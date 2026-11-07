use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::Path,
    process::Command,
    sync::{Arc, Mutex},
};
const SOCKET: &str = "/var/run/app.drift.network.sock";
const DIRECTORY: &str = "/Library/Application Support/Drift";
#[derive(Serialize, Deserialize)]
struct Request {
    url: String,
}
#[derive(Serialize, Deserialize)]
struct Reply {
    ok: bool,
    error: Option<String>,
}
pub struct ProxyLease {
    socket: UnixStream,
}
impl ProxyLease {
    pub fn enable(url: &str) -> io::Result<Self> {
        let mut socket = UnixStream::connect(SOCKET).map_err(|_| {
            io::Error::other(
                "Install Drift's networking helper to enable websites. Discord works without it.",
            )
        })?;
        socket.set_read_timeout(Some(std::time::Duration::from_secs(20)))?;
        socket.write_all(&serde_json::to_vec(&Request { url: url.into() })?)?;
        socket.write_all(b"\n")?;
        let mut response = String::new();
        BufReader::new(socket.try_clone()?)
            .take(4096)
            .read_line(&mut response)?;
        let reply: Reply = serde_json::from_str(&response)?;
        if !reply.ok {
            return Err(io::Error::other(
                reply
                    .error
                    .unwrap_or_else(|| "Could not configure selected websites".into()),
            ));
        }
        socket.set_read_timeout(None)?;
        Ok(Self { socket })
    }
}
impl Drop for ProxyLease {
    fn drop(&mut self) {
        let _ = self
            .socket
            .set_read_timeout(Some(std::time::Duration::from_secs(15)));
        let _ = self.socket.write_all(b"stop\n");
        let mut reply = String::new();
        let _ = BufReader::new(&self.socket)
            .take(4096)
            .read_line(&mut reply);
        let _ = self.socket.shutdown(std::net::Shutdown::Both);
    }
}
struct Journal {
    url: String,
}
fn native(action: &str, url: Option<&str>) -> io::Result<()> {
    let mut command = Command::new("/Library/PrivilegedHelperTools/app.drift.proxyctl");
    command.arg(action);
    if let Some(url) = url {
        command.arg(url);
    }
    let result = command.output()?;
    if !result.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&result.stderr).trim().to_owned(),
        ));
    }
    Ok(())
}
fn restore(journal: &Journal) -> io::Result<()> {
    native("restore", Some(&journal.url))
}
fn apply(url: &str) -> io::Result<Journal> {
    let parsed = url::Url::parse(url).map_err(io::Error::other)?;
    if parsed.scheme() != "http"
        || parsed.host_str() != Some("127.0.0.1")
        || parsed.port().is_none_or(|p| p < 1024)
        || parsed.path() != "/proxy.pac"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(io::Error::other("Invalid local PAC endpoint"));
    }
    if let Err(error) = native("apply", Some(url)) {
        let _ = native("recover", None);
        return Err(error);
    }
    Ok(Journal { url: url.into() })
}
pub fn root_daemon() -> io::Result<()> {
    // SAFETY: libc identity calls have no preconditions.
    if unsafe { libc::geteuid() } != 0 {
        return Err(io::Error::other(
            "The networking helper must be installed by the Drift installer",
        ));
    }
    let lock = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open("/var/run/app.drift.network.lock")?;
    // SAFETY: the file descriptor is live and flock flags are valid.
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(io::Error::other("Networking helper already running"));
    }
    fs::create_dir_all(DIRECTORY)?;
    fs::set_permissions(DIRECTORY, fs::Permissions::from_mode(0o700))?;
    #[derive(Deserialize)]
    struct Owner {
        uid: u32,
    }
    let owner: Owner = serde_json::from_slice(&fs::read(Path::new(DIRECTORY).join("owner.json"))?)?;
    if owner.uid == 0 || owner.uid == u32::MAX {
        return Err(io::Error::other(
            "No desktop user was registered by the installer",
        ));
    }
    let journal_path = Path::new(DIRECTORY).join("proxy-journal.json");
    if journal_path.exists() {
        native("recover", None)?;
    }
    if let Ok(meta) = fs::symlink_metadata(SOCKET) {
        if !meta.file_type().is_socket() || meta.uid() != 0 {
            return Err(io::Error::other("Invalid helper socket"));
        }
        fs::remove_file(SOCKET)?;
    }
    let listener = UnixListener::bind(SOCKET)?;
    fs::set_permissions(SOCKET, fs::Permissions::from_mode(0o666))?;
    let occupied = Arc::new(Mutex::new(false));
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let occupied = occupied.clone();

        std::thread::spawn(move || {
            let mut uid = 0;
            let mut gid = 0;
            // SAFETY: the descriptor and both output pointers are valid.
            if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0
                || uid != owner.uid
            {
                return;
            }
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(8)));
            let mut input = String::new();
            if BufReader::new(&stream)
                .take(4096)
                .read_line(&mut input)
                .is_err()
            {
                return;
            }
            let result = (|| -> io::Result<Journal> {
                let request: Request = serde_json::from_str(&input)?;
                let mut active = occupied.lock().unwrap();
                if *active {
                    return Err(io::Error::other(
                        "Another Drift session already owns website routing",
                    ));
                }
                let journal = apply(&request.url)?;
                *active = true;
                Ok(journal)
            })();
            let reply = Reply {
                ok: result.is_ok(),
                error: result.as_ref().err().map(ToString::to_string),
            };
            let _ = writeln!(stream, "{}", serde_json::to_string(&reply).unwrap());
            if let Ok(journal) = result {
                let _ = stream.set_read_timeout(None);
                let mut reader = BufReader::new(&stream);
                let mut ignore = String::new();
                let _ = reader.read_line(&mut ignore);
                let restored = restore(&journal);
                let acknowledgement = Reply {
                    ok: restored.is_ok(),
                    error: restored.err().map(|e| e.to_string()),
                };
                let _ = writeln!(
                    stream,
                    "{}",
                    serde_json::to_string(&acknowledgement).unwrap()
                );
                *occupied.lock().unwrap() = false;
            }
        });
    }
    Ok(())
}
