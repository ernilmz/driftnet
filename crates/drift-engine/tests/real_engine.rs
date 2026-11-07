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
