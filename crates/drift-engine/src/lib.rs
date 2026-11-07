//! Owned, loopback-only engine process. This does not install OS interception.
//! The platform service must retain this owner until it has removed its redirects.
use std::{
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("The bundled network engine could not be started: {0}")]
    Io(#[from] std::io::Error),
    #[error("The bundled network engine exited before it was ready.")]
    EarlyExit,
    #[error("The bundled network engine did not become ready in time.")]
    Timeout,
}

pub struct Engine {
    child: Child,
    endpoint: SocketAddrV4,
}
impl Engine {
    /// The caller supplies an absolute packaged resource, never a PATH lookup or
    /// a user-supplied executable. No daemon mode and no inherited SS_* options.
    pub fn start(executable: &Path) -> Result<Self, Error> {
        Self::start_bound(executable, None)
    }
    pub fn start_bound(executable: &Path, interface: Option<u32>) -> Result<Self, Error> {
        let executable = executable.canonicalize()?;
        let port = {
            let reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
            reservation.local_addr()?.port()
        };
        let endpoint = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
        let mut command = Command::new(executable);
        command
            .args([
                "--ip",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--max-conn",
                "256",
                "--split",
                "2",
                "--tlsrec",
                "1+s",
            ])
            .env_remove("SS_LOCAL_PORT")
            .env_remove("SS_PLUGIN_OPTIONS")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        if let Some(index) = interface {
            command.env("DRIFT_OUTBOUND_IFINDEX", index.to_string());
        } else {
            command.env_remove("DRIFT_OUTBOUND_IFINDEX");
        }
        let mut engine = Self {
            child: command.spawn()?,
            endpoint,
        };
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            if engine.child.try_wait()?.is_some() {
                return Err(Error::EarlyExit);
            }
            if socks_ready(endpoint) && engine.child.try_wait()?.is_none() {
                return Ok(engine);
            }
            if Instant::now() >= deadline {
                return Err(Error::Timeout);
            }
            thread::sleep(Duration::from_millis(15));
        }
    }
    pub fn process_id(&self) -> u32 {
        self.child.id()
    }
    pub fn endpoint(&self) -> SocketAddrV4 {
        self.endpoint
    }
    pub fn is_running(&mut self) -> Result<bool, Error> {
        Ok(self.child.try_wait()?.is_none())
    }
    pub fn stop(&mut self) -> Result<(), Error> {
        if self.child.try_wait()?.is_none() {
            self.child.kill()?;
        }
        self.child.wait()?;
        Ok(())
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
fn socks_ready(endpoint: SocketAddrV4) -> bool {
    let Ok(mut socket) =
        TcpStream::connect_timeout(&SocketAddr::V4(endpoint), Duration::from_millis(100))
    else {
        return false;
    };
    let timeout = Some(Duration::from_millis(100));
    let _ = socket.set_read_timeout(timeout);
    let _ = socket.set_write_timeout(timeout);
    let mut reply = [0u8; 2];
    socket.write_all(&[5, 1, 0]).is_ok() && socket.read_exact(&mut reply).is_ok() && reply == [5, 0]
}
