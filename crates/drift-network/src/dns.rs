use serde::Deserialize;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr, SocketAddrV4},
    sync::Mutex,
    time::{Duration, Instant},
};
#[derive(Deserialize)]
struct Reply {
    #[serde(rename = "Status")]
    status: u32,
    #[serde(rename = "Answer", default)]
    answers: Vec<Answer>,
}
#[derive(Deserialize)]
struct Answer {
    #[serde(rename = "type")]
    kind: u16,
    data: String,
    #[serde(rename = "TTL")]
    ttl: u64,
}
struct Entry {
    addresses: Vec<IpAddr>,
    expires: Instant,
}
pub struct Resolver {
    client: reqwest::Client,
    cache: Mutex<HashMap<String, Entry>>,
}
impl Resolver {
    pub fn new(engine: SocketAddrV4) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .no_proxy()
            // DoH needs the same wire fragmentation as selected destinations.
            // socks5 (not socks5h) keeps bootstrap resolution in this client,
            // where the fixed resolver IPs below avoid a system-DNS dependency.
            .proxy(reqwest::Proxy::all(format!("socks5://{engine}"))?)
            // Keep DoH TLS framing consistent across platforms, including
            // builds where the desktop updater also enables native-tls.
            .use_rustls_tls()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            // Leave time for the bounded TLS strategy chain (four attempts on
            // Windows). A failed resolver still cannot stall indefinitely.
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(10))
            .resolve("cloudflare-dns.com", SocketAddr::from(([1, 1, 1, 1], 443)))
            .resolve("dns.google", SocketAddr::from(([8, 8, 8, 8], 443)))
            .build()?;
        Ok(Self {
            client,
            cache: Mutex::new(HashMap::new()),
        })
    }
    pub async fn addresses(&self, host: &str) -> Result<Vec<IpAddr>, std::io::Error> {
        if let Ok(ip) = host.parse() {
            return Ok(vec![ip]);
        }
        if let Some(entry) = self
            .cache
            .lock()
            .unwrap()
            .get(host)
            .filter(|e| e.expires > Instant::now())
        {
            return Ok(entry.addresses.clone());
        }
        // Internal names belong to the selected app, not to a public-site rule.
        if drift_core::normalize_domain(host).is_err() {
            let addresses: Vec<_> = tokio::net::lookup_host((host, 443))
                .await?
                .map(|a| a.ip())
                .collect();
            return if addresses.is_empty() {
                Err(std::io::Error::other("No address for selected destination"))
            } else {
                Ok(addresses)
            };
        }
        for endpoint in [
            "https://cloudflare-dns.com/dns-query",
            "https://dns.google/resolve",
        ] {
            let lookup = |kind: &'static str| async move {
                self.client
                    .get(endpoint)
                    .query(&[("name", host), ("type", kind)])
                    .header("accept", "application/dns-json")
                    .send()
                    .await?
                    .error_for_status()?
                    .json::<Reply>()
                    .await
            };
            let (v4, v6) = tokio::join!(lookup("A"), lookup("AAAA"));
            let mut addresses = Vec::new();
            let mut ttl = 300;
            for reply in [v4, v6].into_iter().flatten().filter(|r| r.status == 0) {
                for answer in reply
                    .answers
                    .into_iter()
                    .filter(|a| a.kind == 1 || a.kind == 28)
                {
                    if let Ok(ip) = answer.data.parse::<IpAddr>()
                        && !addresses.contains(&ip)
                        && addresses.len() < 8
                    {
                        addresses.push(ip);
                        ttl = ttl.min(answer.ttl);
                    }
                }
            }
            if addresses.is_empty() {
                continue;
            }
            let mut cache = self.cache.lock().unwrap();
            if cache.len() >= 512 {
                cache.retain(|_, e| e.expires > Instant::now());
                if cache.len() >= 512 {
                    cache.clear()
                }
            }
            cache.insert(
                host.to_owned(),
                Entry {
                    addresses: addresses.clone(),
                    expires: Instant::now() + Duration::from_secs(ttl.clamp(5, 300)),
                },
            );
            return Ok(addresses);
        }
        Err(std::io::Error::other(
            "Could not resolve the selected destination over trusted DNS",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        time::timeout,
    };

    #[tokio::test]
    async fn trusted_dns_bootstrap_uses_engine_and_preserves_tls_hostnames() {
        let engine = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let SocketAddr::V4(endpoint) = engine.local_addr().unwrap() else {
            unreachable!()
        };
        let resolver = Resolver::new(endpoint).unwrap();
        for (host, ip) in [
            ("cloudflare-dns.com", [1, 1, 1, 1]),
            ("dns.google", [8, 8, 8, 8]),
        ] {
            let client = resolver.client.clone();
            let request = tokio::spawn(async move {
                client
                    .get(format!("https://{host}/dns-query?name=example.com&type=A"))
                    .send()
                    .await
            });
            // A direct DoH request or proxy-side hostname lookup must fail this
            // regression even on hosts without internet or working system DNS.
            let (mut stream, _) = timeout(Duration::from_secs(3), engine.accept())
                .await
                .expect("DoH bypassed the owned engine")
                .unwrap();
            timeout(Duration::from_secs(3), async {
                let mut greeting = [0; 3];
                stream.read_exact(&mut greeting).await.unwrap();
                assert_eq!(greeting, [5, 1, 0]);
                stream.write_all(&[5, 0]).await.unwrap();
                let mut connect = [0; 10];
                stream.read_exact(&mut connect).await.unwrap();
                assert_eq!(connect[..4], [5, 1, 0, 1]);
                assert_eq!(connect[4..8], ip);
                assert_eq!(connect[8..], 443u16.to_be_bytes());
                stream
                    .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0])
                    .await
                    .unwrap();
                let mut header = [0; 5];
                stream.read_exact(&mut header).await.unwrap();
                assert_eq!(header[0], 22, "resolver connection must use TLS");
                let mut hello = vec![0; u16::from_be_bytes([header[3], header[4]]) as usize];
                stream.read_exact(&mut hello).await.unwrap();
                assert!(
                    hello
                        .windows(host.len())
                        .any(|bytes| bytes == host.as_bytes())
                );
            })
            .await
            .expect("DoH SOCKS/TLS handshake timed out");
            // The mock stops before the server's TLS handshake. It never
            // presents a trusted certificate or provides a DNS answer.
            drop(stream);
            assert!(request.await.unwrap().is_err());
        }
    }
}
