use std::{
    io,
    net::{IpAddr, SocketAddrV4},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
pub async fn connect(engine: SocketAddrV4, address: IpAddr, port: u16) -> io::Result<TcpStream> {
    timeout(Duration::from_secs(10), async {
        let mut socket = TcpStream::connect(engine).await?;
        socket.set_nodelay(true)?;
        socket.write_all(&[5, 1, 0]).await?;
        let mut reply = [0; 2];
        socket.read_exact(&mut reply).await?;
        if reply != [5, 0] {
            return Err(io::Error::other("Engine authentication failed"));
        }
        let mut request = vec![5, 1, 0];
        match address {
            IpAddr::V4(ip) => {
                request.push(1);
                request.extend(ip.octets())
            }
            IpAddr::V6(ip) => {
                request.push(4);
                request.extend(ip.octets())
            }
        }
        request.extend(port.to_be_bytes());
        socket.write_all(&request).await?;
        let mut header = [0; 4];
        socket.read_exact(&mut header).await?;
        if header[0] != 5 || header[1] != 0 {
            return Err(io::Error::other(
                "Engine could not open the selected destination",
            ));
        }
        let remaining = match header[3] {
            1 => 6,
            4 => 18,
            3 => socket.read_u8().await? as usize + 2,
            _ => return Err(io::Error::other("Invalid engine response")),
        };
        let mut discard = vec![0; remaining];
        socket.read_exact(&mut discard).await?;
        Ok(socket)
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Engine connection timed out"))?
}
