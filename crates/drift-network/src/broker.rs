use crate::{dns::Resolver, pac, socks};
use drift_core::{Config, Decision, Flow};
use std::{
    io,
    net::{Ipv4Addr, SocketAddrV4},
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Semaphore, watch},
    task::JoinHandle,
    time::timeout,
};
const MAX_HEADER: usize = 16 * 1024;
#[derive(Clone, Copy)]
pub enum Scope {
    Sites,
    App,
}
struct Request {
    host: String,
    port: u16,
    connect: bool,
    header: Vec<u8>,
    tail: Vec<u8>,
}
fn parse(bytes: &[u8], header_end: usize) -> io::Result<Request> {
    let text = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| io::Error::other("Invalid proxy request"))?;
    let mut lines = text.split("\r\n");
    let first = lines
        .next()
        .ok_or_else(|| io::Error::other("Missing request line"))?;
    let parts: Vec<_> = first.split(' ').collect();
    if parts.len() != 3 || !matches!(parts[2], "HTTP/1.1" | "HTTP/1.0") {
        return Err(io::Error::other("Invalid proxy request line"));
    }
    let connect = parts[0] == "CONNECT";
    let url = url::Url::parse(&if connect {
        format!("https://{}", parts[1])
    } else {
        parts[1].to_owned()
    })
    .map_err(|_| io::Error::other("Invalid proxy destination"))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(io::Error::other("Invalid proxy destination"));
    }
    if connect
        && (url.path() != "/"
            || url.query().is_some()
            || parts[1]
                .rsplit(':')
                .next()
                .and_then(|p| p.parse::<u16>().ok())
                .is_none())
    {
        return Err(io::Error::other("CONNECT needs an explicit host and port"));
    }
    let host = url
        .host_str()
        .ok_or_else(|| io::Error::other("Missing proxy host"))?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    let port = url
        .port_or_known_default()
        .ok_or_else(|| io::Error::other("Missing proxy port"))?;
    if port == 0 {
        return Err(io::Error::other("Invalid proxy port"));
    }
    let mut header = Vec::new();
    if !connect {
        if url.scheme() != "http" {
            return Err(io::Error::other("HTTPS requires CONNECT"));
        }
        let target = format!(
            "{}{}",
            url.path(),
            url.query().map(|q| format!("?{q}")).unwrap_or_default()
        );
        header.extend_from_slice(format!("{} {} {}\r\n", parts[0], target, parts[2]).as_bytes());
        for line in lines.filter(|s| !s.is_empty()) {
            let Some((name, _)) = line.split_once(':') else {
                return Err(io::Error::other("Malformed request header"));
            };
            if !name.eq_ignore_ascii_case("proxy-connection")
                && !name.eq_ignore_ascii_case("proxy-authorization")
                && !name.eq_ignore_ascii_case("connection")
            {
                header.extend_from_slice(line.as_bytes());
                header.extend_from_slice(b"\r\n");
            }
        }
        header.extend_from_slice(b"Connection: close\r\n\r\n");
    }
    Ok(Request {
        host,
        port,
        connect,
        header,
        tail: bytes[header_end..].to_vec(),
    })
}
async fn headers(socket: &mut TcpStream) -> io::Result<(Vec<u8>, usize)> {
    timeout(Duration::from_secs(8), async {
        let mut bytes = Vec::with_capacity(2048);
        let mut chunk = [0; 2048];
        loop {
            let read = socket.read(&mut chunk).await?;
            if read == 0 {
                return Err(io::Error::other("Proxy client closed"));
            }
            bytes.extend_from_slice(&chunk[..read]);
            if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                if i + 4 > MAX_HEADER {
                    return Err(io::Error::other("Proxy headers too large"));
                }
                return Ok((bytes, i + 4));
            }
            if bytes.len() > MAX_HEADER {
                return Err(io::Error::other("Proxy headers too large"));
            }
        }
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Proxy request timed out"))?
}
async fn proxy(
    mut client: TcpStream,
    scope: Scope,
    engine: SocketAddrV4,
    config: Arc<RwLock<Config>>,
    resolver: Arc<Resolver>,
) -> io::Result<()> {
    let (bytes, end) = headers(&mut client).await?;
    let request = match parse(&bytes, end) {
        Ok(r) => r,
        Err(_) => {
            client
                .write_all(b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n")
                .await?;
            return Ok(());
        }
    };
    if matches!(scope, Scope::Sites)
        && !matches!(
            config.read().unwrap().decide(&Flow {
                hostname: Some(&request.host),
                ..Flow::default()
            }),
            Decision::Selected { .. }
        )
    {
        client
            .write_all(b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n")
            .await?;
        return Ok(());
    }
    let result = async {
        let addresses = resolver.addresses(&request.host).await?;
        let mut attempts = tokio::task::JoinSet::new();
        for (i, address) in addresses.into_iter().enumerate() {
            let port = request.port;
            attempts.spawn(async move {
                tokio::time::sleep(Duration::from_millis(150 * i as u64)).await;
                socks::connect(engine, address, port).await
            });
        }
        while let Some(result) = attempts.join_next().await {
            if let Ok(Ok(stream)) = result {
                attempts.abort_all();
                return Ok(stream);
            }
        }
        Err(io::Error::other(
            "All destination addresses were unreachable",
        ))
    }
    .await;
    let mut remote = match result {
        Ok(s) => s,
        Err(_) => {
            client
                .write_all(b"HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\n\r\n")
                .await?;
            return Ok(());
        }
    };
    if request.connect {
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?
    } else {
        remote.write_all(&request.header).await?
    }
    remote.write_all(&request.tail).await?;
    tokio::io::copy_bidirectional(&mut client, &mut remote).await?;
    Ok(())
}
pub struct Servers {
    pub site_port: u16,
    pub app_port: u16,
    pub pac_url: String,
    stop: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}
impl Servers {
    pub async fn start(engine: SocketAddrV4, config: Arc<RwLock<Config>>) -> io::Result<Self> {
        let site = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let app = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let pac_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let site_port = site.local_addr()?.port();
        let app_port = app.local_addr()?.port();
        let pac_port = pac_listener.local_addr()?.port();
        let resolver = Arc::new(Resolver::new(engine).map_err(io::Error::other)?);
        let permits = Arc::new(Semaphore::new(256));
        let (stop, rx) = watch::channel(false);
        let mut tasks = Vec::new();
        for (listener, scope) in [(site, Scope::Sites), (app, Scope::App)] {
            let (config, resolver, permits, mut rx) = (
                config.clone(),
                resolver.clone(),
                permits.clone(),
                rx.clone(),
            );
            tasks.push(tokio::spawn(async move {
                loop {tokio::select! {
                    _=rx.changed()=>break,
                    accepted=listener.accept()=>{let Ok((socket,_))=accepted else {break};
                        let Ok(permit)=permits.clone().try_acquire_owned() else {drop(socket);continue};
                        let (config,resolver)=(config.clone(),resolver.clone());
                        tokio::spawn(async move {let _permit=permit;let _=proxy(socket,scope,engine,config,resolver).await;});
                    }
                }}
            }));
        }
        let mut rx = rx;
        tasks.push(tokio::spawn(async move {
            loop {tokio::select! {
                _=rx.changed()=>break,
                accepted=pac_listener.accept()=>{let Ok((mut socket,_))=accepted else {break};let config=config.clone();
                    let Ok(permit)=permits.clone().try_acquire_owned() else {drop(socket);continue};
                    tokio::spawn(async move {
                        let _permit=permit;
                        let Ok((bytes,_))=headers(&mut socket).await else {return};
                        if !bytes.starts_with(b"GET /proxy.pac HTTP/1.") {let _=socket.write_all(b"HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n").await;return}
                        let script=pac::script(&config.read().unwrap(),site_port);
                        let response=format!("HTTP/1.1 200 OK\r\nContent-Type: application/x-ns-proxy-autoconfig\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",script.len(),script);
                        let _=socket.write_all(response.as_bytes()).await;
                    });
                }
            }}
        }));
        Ok(Self {
            site_port,
            app_port,
            pac_url: format!("http://127.0.0.1:{pac_port}/proxy.pac"),
            stop,
            tasks,
        })
    }
}
impl Drop for Servers {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        for task in &self.tasks {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_connect_ipv6_and_refuses_ambiguous_targets() {
        let input = b"CONNECT [::1]:8443 HTTP/1.1\r\nHost: [::1]:8443\r\n\r\nhello";
        let end = input.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        let r = parse(input, end).unwrap();
        assert_eq!(r.host, "::1");
        assert_eq!(r.port, 8443);
        assert_eq!(r.tail, b"hello");
        for input in [
            "CONNECT example.com HTTP/1.1\r\n\r\n",
            "CONNECT user@example.com:443 HTTP/1.1\r\n\r\n",
            "CONNECT example.com:443/path HTTP/1.1\r\n\r\n",
        ] {
            assert!(parse(input.as_bytes(), input.len()).is_err())
        }
    }
    #[test]
    fn rewrites_absolute_http_target_without_forwarding_proxy_credentials() {
        let input=b"GET http://example.com/a?q=1 HTTP/1.1\r\nHost: example.com\r\nProxy-Authorization: secret\r\n\r\n";
        let request = parse(input, input.len()).unwrap();
        assert_eq!(
            request.header,
            b"GET /a?q=1 HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n"
        );
    }
}
