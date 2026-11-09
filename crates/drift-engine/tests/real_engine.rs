//! Real local sockets + real compiled engine; never modifies DNS/proxy/routes.
use drift_engine::Engine;
use std::{
    env,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    thread,
    time::Duration,
};
fn binary() -> PathBuf {
    env::var_os("DRIFT_ENGINE_TEST_BINARY")
        .map(PathBuf::from)
        .expect("Build with scripts/build-engine.py and set DRIFT_ENGINE_TEST_BINARY")
}
fn greeting(endpoint: std::net::SocketAddrV4) -> TcpStream {
    let mut socket = TcpStream::connect(endpoint).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    socket.write_all(&[5, 1, 0]).unwrap();
    let mut reply = [0; 2];
    socket.read_exact(&mut reply).unwrap();
    assert_eq!(reply, [5, 0]);
    socket
}

fn connect_target(engine: &Engine, port: u16) -> TcpStream {
    let mut socket = greeting(engine.endpoint());
    let [hi, lo] = port.to_be_bytes();
    socket
        .write_all(&[5, 1, 0, 1, 127, 0, 0, 1, hi, lo])
        .unwrap();
    let mut reply = [0; 10];
    socket.read_exact(&mut reply).unwrap();
    assert_eq!(&reply[..2], &[5, 0]);
    socket
}

fn tls_record(handshake_type: u8, body: &[u8]) -> Vec<u8> {
    let mut handshake = vec![handshake_type];
    handshake.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
    handshake.extend_from_slice(body);
    let mut record = vec![22, 3, 3];
    record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
    record.extend(handshake);
    record
}

fn client_hello() -> Vec<u8> {
    let host = b"fallback.loopback.invalid";
    let mut extensions = vec![0, 0]; // server_name extension
    extensions.extend_from_slice(&((host.len() + 5) as u16).to_be_bytes());
    extensions.extend_from_slice(&((host.len() + 3) as u16).to_be_bytes());
    extensions.push(0);
    extensions.extend_from_slice(&(host.len() as u16).to_be_bytes());
    extensions.extend_from_slice(host);
    extensions.extend_from_slice(&[0, 43, 0, 3, 2, 3, 4]); // TLS 1.3
    extensions.extend_from_slice(&[0, 10, 0, 4, 0, 2, 0, 29]); // X25519 group
    extensions.extend_from_slice(&[0, 13, 0, 4, 0, 2, 4, 3]); // signature algorithm
    extensions.extend_from_slice(&[0, 51, 0, 38, 0, 36, 0, 29, 0, 32]);
    extensions.extend_from_slice(&[0x11; 32]); // synthetic X25519 key share
    let mut body = vec![3, 3];
    body.extend_from_slice(&[0x42; 32]);
    body.push(32);
    body.extend_from_slice(&[0x33; 32]); // legacy_session_id echoed by server
    body.extend_from_slice(&[0, 2, 0x13, 1, 1, 0]);
    body.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
    body.extend(extensions);
    tls_record(1, &body)
}

fn server_hello() -> Vec<u8> {
    let mut body = vec![3, 3];
    body.extend_from_slice(&[0x44; 32]);
    body.push(32);
    body.extend_from_slice(&[0x33; 32]);
    body.extend_from_slice(&[0x13, 1, 0]);
    let mut extensions = vec![0, 43, 0, 2, 3, 4, 0, 51, 0, 36, 0, 29, 0, 32];
    extensions.extend_from_slice(&[0x22; 32]);
    body.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
    body.extend(extensions);
    tls_record(2, &body)
}

fn accept_before(listener: &TcpListener, timeout: Duration) -> Option<TcpStream> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((socket, _)) => {
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                return Some(socket);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() >= deadline {
                    return None;
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("local accept failed: {error}"),
        }
    }
}

// Return the first record's boundary while checking that fragmentation never
// changes the ClientHello handshake bytes or smuggles an application record.
fn read_client_hello(socket: &mut TcpStream, expected: &[u8]) -> usize {
    let mut handshake = Vec::new();
    let mut first_record = 0;
    while handshake.len() < expected.len() - 5 {
        let mut header = [0; 5];
        socket.read_exact(&mut header).unwrap();
        assert_eq!(&header[..2], &[22, 3]);
        let size = usize::from(u16::from_be_bytes([header[3], header[4]]));
        assert!(size > 0 && size <= expected.len());
        if first_record == 0 {
            first_record = size;
        }
        let start = handshake.len();
        handshake.resize(start + size, 0);
        socket.read_exact(&mut handshake[start..]).unwrap();
    }
    assert_eq!(handshake, &expected[5..]);
    first_record
}

#[test]
#[ignore = "requires pinned native engine; exercised by CI after building it"]
fn tls_close_retries_only_the_hello_and_caches_the_working_profile() {
    let engine = Engine::start(&binary()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let hello = client_hello();
    let response = server_hello();
    let worker_hello = hello.clone();
    let worker_response = response.clone();
    let worker = thread::spawn(move || {
        // Reject every record-splitting method, including Windows' additional
        // disorder method, to exercise the complete chain before TCP-only TLS.
        for _ in 0..if cfg!(windows) { 3 } else { 2 } {
            let mut rejected = accept_before(&listener, Duration::from_secs(5)).unwrap();
            assert!(read_client_hello(&mut rejected, &worker_hello) < worker_hello.len() - 5);
            drop(rejected);
        }
        let mut retry = accept_before(&listener, Duration::from_secs(5)).unwrap();
        let fallback = read_client_hello(&mut retry, &worker_hello);
        assert_eq!(
            fallback,
            worker_hello.len() - 5,
            "final method must omit TLS record splitting"
        );
        retry.write_all(&worker_response).unwrap();
        let mut warm = accept_before(&listener, Duration::from_secs(5)).unwrap();
        assert_eq!(read_client_hello(&mut warm, &worker_hello), fallback);
        warm.write_all(&worker_response).unwrap();
    });
    for _ in 0..2 {
        let mut client = connect_target(&engine, port);
        client.write_all(&hello).unwrap();
        let mut received = vec![0; response.len()];
        client.read_exact(&mut received).unwrap();
        assert_eq!(received, response);
    }
    worker.join().unwrap();
}

#[test]
#[ignore = "requires pinned native engine; exercised by CI after building it"]
fn tls_silent_first_hello_falls_back_before_the_client_timeout() {
    let engine = Engine::start(&binary()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let hello = client_hello();
    let response = server_hello();
    let worker_hello = hello.clone();
    let worker_response = response.clone();
    let worker = thread::spawn(move || {
        let mut silent = accept_before(&listener, Duration::from_secs(5)).unwrap();
        read_client_hello(&mut silent, &worker_hello);
        // Retain the first connection without replying, so only the TLS timer
        // (rather than an EOF/reset) can make a new outbound connection.
        let mut retry = accept_before(&listener, Duration::from_secs(4)).unwrap();
        read_client_hello(&mut retry, &worker_hello);
        retry.write_all(&worker_response).unwrap();
        drop(silent);
    });
    let mut client = connect_target(&engine, port);
    let started = std::time::Instant::now();
    client.write_all(&hello).unwrap();
    let mut received = vec![0; response.len()];
    client.read_exact(&mut received).unwrap();
    assert_eq!(received, response);
    assert!(started.elapsed() < Duration::from_secs(5));
    worker.join().unwrap();
}

#[test]
#[ignore = "requires pinned native engine; exercised by CI after building it"]
fn http_post_and_tls_early_data_are_never_replayed() {
    let mut early = client_hello();
    early.extend_from_slice(&[23, 3, 3, 0, 4, 1, 2, 3, 4]);
    for payload in [
        b"POST /transfer HTTP/1.1\r\nHost: fallback.loopback.invalid\r\nContent-Length: 4\r\n\r\npaid".to_vec(),
        early,
    ] {
        let engine = Engine::start(&binary()).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let expected = payload.clone();
        let worker = thread::spawn(move || {
            let mut first = accept_before(&listener, Duration::from_secs(5)).unwrap();
            if expected[0] == 22 {
                let hello = client_hello();
                read_client_hello(&mut first, &hello);
                let mut application = [0; 9];
                first.read_exact(&mut application).unwrap();
                assert_eq!(application, [23, 3, 3, 0, 4, 1, 2, 3, 4]);
            } else {
                let mut received = vec![0; expected.len()];
                first.read_exact(&mut received).unwrap();
                assert_eq!(received, expected);
            }
            drop(first);
            assert!(
                accept_before(&listener, Duration::from_secs(3)).is_none(),
                "a non-idempotent payload opened a replay connection"
            );
        });
        let mut client = connect_target(&engine, port);
        client.write_all(&payload).unwrap();
        // Keep the owner and client alive throughout the server's observation
        // window; an early client drop must not be what prevents a replay.
        worker.join().unwrap();
        drop(client);
        drop(engine);
    }
}
#[test]
#[ignore = "requires pinned native engine; exercised by CI after building it"]
fn relays_payload_and_releases_port_on_drop() {
    let engine = Engine::start(&binary()).unwrap();
    assert!(engine.endpoint().ip().is_loopback());
    let endpoint = engine.endpoint();
    let target = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = target.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut incoming, _) = target.accept().unwrap();
        incoming
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        // Let the sender fill the kernel buffers before reading. This catches
        // short-send data loss in the engine under TCP backpressure.
        thread::sleep(Duration::from_millis(100));
        let mut bytes = vec![0u8; 4_194_304];
        incoming.read_exact(&mut bytes).unwrap();
        assert!(bytes.iter().enumerate().all(|(i, &b)| b == (i % 251) as u8));
        incoming.write_all(b"drift-relay-ok").unwrap();
    });
    let mut socket = greeting(endpoint);
    let [hi, lo] = port.to_be_bytes();
    socket
        .write_all(&[5, 1, 0, 1, 127, 0, 0, 1, hi, lo])
        .unwrap();
    let mut reply = [0u8; 10];
    socket.read_exact(&mut reply).unwrap();
    assert_eq!(&reply[..2], &[5, 0]);
    let payload: Vec<u8> = (0..4_194_304).map(|i| (i % 251) as u8).collect();
    socket.write_all(&payload).unwrap();
    let mut echoed = [0; 14];
    socket.read_exact(&mut echoed).unwrap();
    assert_eq!(&echoed, b"drift-relay-ok");
    server.join().unwrap();
    drop(socket);
    drop(engine);
    assert!(TcpStream::connect_timeout(&endpoint.into(), Duration::from_millis(200)).is_err());
}
#[test]
#[ignore = "requires pinned native engine; exercised by CI after building it"]
fn relays_udp_datagrams_and_releases_association() {
    use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
    let mut engine = Engine::start(&binary()).unwrap();
    let mut control = greeting(engine.endpoint());
    control
        .write_all(&[5, 3, 0, 1, 127, 0, 0, 1, 0, 0])
        .unwrap();
    let mut header = [0; 4];
    control.read_exact(&mut header).unwrap();
    assert_eq!(&header[..3], &[5, 0, 0]);
    let endpoint = match header[3] {
        1 => {
            let mut b = [0; 6];
            control.read_exact(&mut b).unwrap();
            SocketAddr::from((
                Ipv4Addr::new(b[0], b[1], b[2], b[3]),
                u16::from_be_bytes([b[4], b[5]]),
            ))
        }
        other => panic!("Unexpected local association address {other}"),
    };
    let target = UdpSocket::bind("127.0.0.1:0").unwrap();
    target
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let port = target.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let mut b = [0; 4096];
        let (n, from) = target.recv_from(&mut b).unwrap();
        target.send_to(&b[..n], from).unwrap();
    });
    let client = UdpSocket::bind("127.0.0.1:0").unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let body: Vec<u8> = (0..1024).map(|i| (i % 251) as u8).collect();
    let mut packet = vec![0, 0, 0, 1, 127, 0, 0, 1];
    packet.extend(port.to_be_bytes());
    packet.extend(&body);
    client.send_to(&packet, endpoint).unwrap();
    let mut response = [0; 4096];
    let (n, _) = client.recv_from(&mut response).unwrap();
    let offset = match response[3] {
        1 => 10,
        4 => 22,
        _ => panic!("Invalid UDP address"),
    };
    assert_eq!(&response[..3], &[0, 0, 0]);
    assert_eq!(&response[offset..n], body.as_slice());
    server.join().unwrap();
    drop(control);
    engine.stop().unwrap();
    assert!(!engine.is_running().unwrap());
}
#[test]
#[ignore = "requires pinned native engine; exercised by CI after building it"]
fn reaches_ipv6_only_tcp_destination() {
    let engine = Engine::start(&binary()).unwrap();
    let server = TcpListener::bind("[::1]:0").unwrap();
    let port = server.local_addr().unwrap().port();
    let worker = thread::spawn(move || {
        let (mut s, _) = server.accept().unwrap();
        s.write_all(b"ipv6-drift").unwrap();
    });
    let mut client = greeting(engine.endpoint());
    let mut request = vec![5, 1, 0, 4];
    request.extend(std::net::Ipv6Addr::LOCALHOST.octets());
    request.extend(port.to_be_bytes());
    client.write_all(&request).unwrap();
    let mut header = [0; 4];
    client.read_exact(&mut header).unwrap();
    assert_eq!(&header[..3], &[5, 0, 0]);
    let mut address = vec![0; if header[3] == 4 { 18 } else { 6 }];
    client.read_exact(&mut address).unwrap();
    let mut body = [0; 10];
    client.read_exact(&mut body).unwrap();
    assert_eq!(&body, b"ipv6-drift");
    worker.join().unwrap();
}

#[test]
fn missing_engine_is_reported_without_starting_a_listener() {
    assert!(Engine::start(std::path::Path::new("/drift-test-nonexistent-engine")).is_err());
}

#[test]
#[ignore = "requires pinned native engine; exercised by CI after building it"]
fn abrupt_owner_crash_releases_listener_without_rust_drop() {
    use std::io::BufRead;
    let mut owner = std::process::Command::new(env!("CARGO_BIN_EXE_drift-engine-owner-fixture"))
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    std::io::BufReader::new(owner.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let endpoint: std::net::SocketAddr = line.trim().parse().unwrap();
    assert!(TcpStream::connect(endpoint).is_ok());
    owner.kill().unwrap();
    owner.wait().unwrap(); // no destructor executes in owner
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while TcpStream::connect_timeout(&endpoint, Duration::from_millis(100)).is_ok() {
        assert!(
            std::time::Instant::now() < deadline,
            "engine survived its owner"
        );
        thread::sleep(Duration::from_millis(20));
    }
}
