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
#[serde(rename_all = "snake_case")]
enum AppFamily {
    #[cfg(windows)]
    Discord,
}
#[derive(Serialize)]
pub struct App {
    path: PathBuf,
    bundle: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    family: Option<AppFamily>,
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
    #[serde(default)]
    app_families: bool,
    error: Option<String>,
}
#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
impl Reply {
    fn accepts(&self, applications: bool) -> bool {
        self.ok
            && (!applications
                || (self.protocol == 2 && self.router && (!cfg!(windows) || self.app_families)))
    }
}
pub fn applications(config: &Config) -> io::Result<Vec<App>> {
    let mut apps = Vec::new();
    for rule in config.rules.iter().filter(|r| r.enabled) {
        #[cfg(windows)]
        if matches!(rule.target, Target::Discord) {
            let root = PathBuf::from(
                std::env::var_os("LOCALAPPDATA")
                    .ok_or_else(|| io::Error::other("No local app data folder"))?,
            )
            .join("Discord");
            // Select the installation, including Squirrel's updater and future
            // numbered app directories, rather than only today's executable.
            match root.canonicalize() {
                Ok(path) if path.is_dir() => apps.push(App {
                    path,
                    bundle: false,
                    family: Some(AppFamily::Discord),
                }),
                Ok(_) => return Err(io::Error::other("Invalid Discord installation")),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            continue;
        }
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
                family: None,
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
                "Native routing is supported on macOS, Windows and Linux",
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
        if !reply.accepts(!apps.is_empty()) {
            let _ = channel.write_all(b"stop\n");
            return Err(io::Error::other(reply.error.unwrap_or_else(|| {
                "Update Driftnet using the new installer to enable application routing.".into()
            })));
        }
        #[cfg(unix)]
        channel.set_read_timeout(None)?;
        let alive = Arc::new(AtomicBool::new(true));
        let flag = alive.clone();
        #[cfg(unix)]
        let mut reader = channel.try_clone()?;
        #[cfg(windows)]
        let reader = channel.try_clone()?;
        let (send, done) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            #[cfg(windows)]
            wait_for_windows_lease_end(&reader, &flag);
            #[cfg(unix)]
            {
                let mut line = String::new();
                let _ = std::io::Read::take(BufReader::new(&mut reader), 8192).read_line(&mut line);
            }
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
        self.alive.store(false, Ordering::Release);
    }
}

#[cfg(windows)]
fn wait_for_windows_lease_end(channel: &Channel, alive: &AtomicBool) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Pipes::PeekNamedPipe;
    // A pending ReadFile on a synchronous pipe also locks writes through
    // try_clone's shared FILE_OBJECT. Poll without reading so Drop can send
    // stop. The helper emits its next byte only after routing teardown.
    while alive.load(Ordering::Acquire) {
        let mut available = 0;
        // SAFETY: connected live handle, valid output pointer, no data buffer.
        let ok = unsafe {
            PeekNamedPipe(
                channel.as_raw_handle(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 || available > 0 {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
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

#[cfg(test)]
mod tests {
    use super::Reply;

    #[test]
    fn windows_applications_require_the_path_and_family_capability() {
        let legacy: Reply =
            serde_json::from_str(r#"{"ok":true,"protocol":2,"router":true,"error":null}"#).unwrap();
        assert!(legacy.accepts(false));
        // Even an exact generic app needs the new path-normalizing helper.
        assert_eq!(legacy.accepts(true), !cfg!(windows));
        let current: Reply = serde_json::from_str(
            r#"{"ok":true,"protocol":2,"router":true,"app_families":true,"error":null}"#,
        )
        .unwrap();
        assert!(current.accepts(true));
        let wrong_protocol: Reply = serde_json::from_str(
            r#"{"ok":true,"protocol":1,"router":true,"app_families":true,"error":null}"#,
        )
        .unwrap();
        assert!(!wrong_protocol.accepts(true));
    }
}

#[cfg(all(test, windows))]
mod windows_pipe_tests {
    use super::wait_for_windows_lease_end;
    use std::{
        fs::{File, OpenOptions},
        io::{Read, Write},
        os::windows::io::{AsRawHandle, FromRawHandle},
        sync::{Arc, atomic::AtomicBool, mpsc},
        thread,
        time::Duration,
    };
    use windows_sys::Win32::Foundation::{ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE};

    #[link(name = "kernel32")]
    unsafe extern "system" {
        #[link_name = "CreateNamedPipeW"]
        fn create_named_pipe(
            name: *const u16,
            access: u32,
            mode: u32,
            instances: u32,
            output_size: u32,
            input_size: u32,
            timeout: u32,
            security: *const std::ffi::c_void,
        ) -> *mut std::ffi::c_void;
        #[link_name = "ConnectNamedPipe"]
        fn connect_named_pipe(
            pipe: *mut std::ffi::c_void,
            overlapped: *mut std::ffi::c_void,
        ) -> i32;
    }

    #[test]
    fn synchronous_pipe_monitor_does_not_block_stop_writes() {
        let name = format!(r"\\.\pipe\Driftnet.Lease.Test.{}", std::process::id());
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        // SAFETY: terminated name, no security attributes, synchronous duplex
        // byte pipe; the returned handle is immediately transferred to File.
        let handle =
            unsafe { create_named_pipe(wide.as_ptr(), 3, 0, 1, 4096, 4096, 0, std::ptr::null()) };
        assert_ne!(handle, INVALID_HANDLE_VALUE);
        // SAFETY: unique owned valid handle from CreateNamedPipeW.
        let mut server = unsafe { File::from_raw_handle(handle) };
        let server = thread::spawn(move || {
            // SAFETY: live owned pipe, synchronous operation without OVERLAPPED.
            let connected =
                unsafe { connect_named_pipe(server.as_raw_handle(), std::ptr::null_mut()) };
            assert!(
                connected != 0
                    || std::io::Error::last_os_error().raw_os_error()
                        == Some(ERROR_PIPE_CONNECTED as i32)
            );
            let mut stop = [0; 5];
            server.read_exact(&mut stop).unwrap();
            assert_eq!(&stop, b"stop\n");
            server.write_all(b"{\"ok\":true}\n").unwrap();
        });
        let mut client = OpenOptions::new()
            .read(true)
            .write(true)
            .open(name)
            .unwrap();
        let reader = client.try_clone().unwrap();
        let alive = Arc::new(AtomicBool::new(true));
        let (started, ready) = mpsc::channel();
        let monitor = thread::spawn(move || {
            started.send(()).unwrap();
            wait_for_windows_lease_end(&reader, &alive);
        });
        ready.recv_timeout(Duration::from_secs(2)).unwrap();
        thread::sleep(Duration::from_millis(75));
        let (written, done) = mpsc::channel();
        let writer = thread::spawn(move || {
            client.write_all(b"stop\n").unwrap();
            written.send(()).unwrap();
        });
        done.recv_timeout(Duration::from_secs(2))
            .expect("stop write blocked behind the pipe monitor");
        writer.join().unwrap();
        server.join().unwrap();
        monitor.join().unwrap();
    }
}
