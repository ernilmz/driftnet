//! Authenticated local control lease for the installed native application router.
use drift_core::{Config, Target};
#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
use serde::Deserialize;
use serde::Serialize;
#[cfg(windows)]
use std::fs::File as Channel;
#[cfg(unix)]
use std::os::unix::net::UnixStream as Channel;
use std::{
    io::{self, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
#[derive(Serialize)]
pub struct App {
    path: PathBuf,
    bundle: bool,
}
#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
#[derive(Serialize)]
struct Request<'a> {
    url: &'a str,
    engine_port: u16,
    interface_index: u32,
    apps: &'a [App],
}
#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
#[derive(Deserialize)]
struct Reply {
    ok: bool,
    #[serde(default)]
    protocol: u32,
    #[serde(default)]
    router: bool,
    error: Option<String>,
}
pub fn applications(config: &Config) -> io::Result<Vec<App>> {
    let mut apps = Vec::new();
    for rule in config.rules.iter().filter(|r| r.enabled) {
        let (identity, optional) = match &rule.target {
            Target::Discord => (
                if cfg!(target_os = "macos") {
                    "macos:com.hnc.Discord"
                } else {
                    "discord"
                },
                true,
            ),
            Target::App { identity, .. } => (identity.as_str(), false),
            _ => continue,
        };
        match crate::apps::resolve(identity) {
            Ok(path) => apps.push(App {
                path: path.canonicalize()?,
                bundle: cfg!(target_os = "macos"),
            }),
            Err(e) if optional && e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(apps)
}
pub fn physical_interface() -> io::Result<u32> {
    let helper = std::env::current_exe()?
        .parent()
        .ok_or_else(|| io::Error::other("Missing helpers"))?
        .join(if cfg!(windows) {
            "drift-router.exe"
        } else {
            "drift-router"
        });
    let mut command = std::process::Command::new(helper);
    command.arg("--interface-index");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|i| *i > 0)
        .ok_or_else(|| io::Error::other("Invalid physical interface"))
}
pub struct Lease {
    channel: Channel,
    alive: Arc<AtomicBool>,
    done: std::sync::mpsc::Receiver<()>,
}
impl Lease {
    pub fn start(
        url: &str,
        engine_port: u16,
        apps: &[App],
        interface: Option<u32>,
    ) -> io::Result<Self> {
        #[cfg(target_os = "macos")]
        let mut channel = Channel::connect("/var/run/app.drift.network.sock")?;
        #[cfg(target_os = "linux")]
        let mut channel = Channel::connect("/run/driftnet/network.sock")?;
        #[cfg(windows)]
        let mut channel = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(r"\\.\pipe\Drift.Network")?;
        #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
        {
            let _ = (url, engine_port, apps, interface);
            return Err(io::Error::other(
                "Native routing is supported on macOS and Windows",
            ));
        }
        #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
        {
            Self::connect(&mut channel, url, engine_port, apps, interface)
        }
    }
    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    fn connect(
        channel: &mut Channel,
        url: &str,
        engine_port: u16,
        apps: &[App],
        interface: Option<u32>,
    ) -> io::Result<Self> {
        use std::io::{BufRead, BufReader};
        #[cfg(unix)]
        channel.set_read_timeout(Some(Duration::from_secs(30)))?;
        #[cfg(target_os = "macos")]
        {
            use std::os::fd::AsRawFd;
            let (mut uid, mut gid) = (u32::MAX, 0);
            // SAFETY: live connected socket and valid output pointers.
            if unsafe { libc::getpeereid(channel.as_raw_fd(), &mut uid, &mut gid) } != 0 || uid != 0
            {
                return Err(io::Error::other(
                    "The installed networking helper could not be authenticated",
                ));
            }
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::AsRawFd;
            let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
            let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
            // SAFETY: live connected socket, valid output buffer and length.
            let result = unsafe {
                libc::getsockopt(
                    channel.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_PEERCRED,
                    (&mut credentials as *mut libc::ucred).cast(),
                    &mut size,
                )
            };
            if result != 0
                || size as usize != std::mem::size_of::<libc::ucred>()
                || credentials.uid != 0
            {
                return Err(io::Error::other(
                    "The installed networking service could not be authenticated",
                ));
            }
        }
        #[cfg(windows)]
        verify_service(channel)?;
        let request = serde_json::to_vec(&Request {
            url,
            engine_port,
            interface_index: interface.unwrap_or_default(),
            apps,
        })?;
        if request.len() > 65535 {
            return Err(io::Error::other("Too many routing rules"));
        }
        channel.write_all(&request)?;
        channel.write_all(b"\n")?;
        let mut line = String::new();
        std::io::Read::take(BufReader::new(channel.try_clone()?), 8192).read_line(&mut line)?;
        let reply: Reply = serde_json::from_str(&line)?;
        if !reply.ok || (!apps.is_empty() && (reply.protocol != 2 || !reply.router)) {
            let _ = channel.write_all(b"stop\n");
            return Err(io::Error::other(reply.error.unwrap_or_else(|| {
                "Update Drift using the new installer to enable application routing.".into()
            })));
        }
        #[cfg(unix)]
        channel.set_read_timeout(None)?;
        let alive = Arc::new(AtomicBool::new(true));
        let flag = alive.clone();
        let mut reader = channel.try_clone()?;
        let (send, done) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let _ = std::io::Read::take(BufReader::new(&mut reader), 8192).read_line(&mut line);
            flag.store(false, Ordering::Release);
            let _ = send.send(());
        });
        Ok(Self {
            channel: channel.try_clone()?,
            alive,
            done,
        })
    }
    pub fn is_running(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        let _ = self.channel.write_all(b"stop\n");
        let _ = self.done.recv_timeout(Duration::from_secs(15));
    }
}

#[cfg(windows)]
fn verify_service(channel: &Channel) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::{Pipes::GetNamedPipeServerProcessId, Services::*};
    // Querying the SCM is available to ordinary desktop users and avoids asking
    // them for permission to inspect a SYSTEM process token.
    unsafe {
        let mut pid = 0;
        if GetNamedPipeServerProcessId(channel.as_raw_handle(), &mut pid) == 0 {
            return Err(io::Error::last_os_error());
        }
        let manager = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
        if manager.is_null() {
            return Err(io::Error::last_os_error());
        }
        let name: Vec<u16> = "DriftNetwork".encode_utf16().chain(Some(0)).collect();
        let service = OpenServiceW(manager, name.as_ptr(), SERVICE_QUERY_STATUS);
        CloseServiceHandle(manager);
        if service.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut status: SERVICE_STATUS_PROCESS = std::mem::zeroed();
        let mut needed = 0;
        let ok = QueryServiceStatusEx(
            service,
            SC_STATUS_PROCESS_INFO,
            (&mut status as *mut SERVICE_STATUS_PROCESS).cast(),
            std::mem::size_of::<SERVICE_STATUS_PROCESS>() as u32,
            &mut needed,
        );
        CloseServiceHandle(service);
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        if status.dwProcessId != pid || status.dwCurrentState != SERVICE_RUNNING {
            return Err(io::Error::other(
                "The installed network service could not be authenticated",
            ));
        }
        Ok(())
    }
}
