//! Per-user GNOME PAC integration. The root TUN service never changes desktop settings.
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
    },
    path::PathBuf,
    process::Command,
};
const SCHEMA: &str = "org.gnome.system.proxy";
#[derive(Serialize, Deserialize)]
struct Journal {
    ours: String,
    mode: String,
    url: String,
}
trait Settings {
    fn get(&self, key: &str) -> io::Result<String>;
    fn set(&self, key: &str, value: &str) -> io::Result<()>;
}
struct GSettings;
impl Settings for GSettings {
    fn get(&self, key: &str) -> io::Result<String> {
        let output = Command::new("gsettings")
            .args(["get", SCHEMA, key])
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(
                "GNOME proxy settings are unavailable in this desktop session",
            ));
        }
        String::from_utf8(output.stdout)
            .map(|s| s.trim().to_owned())
            .map_err(io::Error::other)
    }
    fn set(&self, key: &str, value: &str) -> io::Result<()> {
        let output = Command::new("gsettings")
            .args(["set", SCHEMA, key, value])
            .output()?;
        if !output.status.success() || !output.stderr.is_empty() {
            return Err(io::Error::other(
                "Could not update GNOME proxy settings. Check that the user session permits proxy changes.",
            ));
        }
        if self.get(key)? != value {
            return Err(io::Error::other("GNOME did not save the proxy setting"));
        }
        Ok(())
    }
}
fn session_available() -> io::Result<()> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !desktop
        .split(':')
        .any(|part| part == "gnome" || part == "ubuntu")
        || std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none()
    {
        return Err(io::Error::other(
            "Website routing currently requires a GNOME desktop session. Native application routing is supported separately.",
        ));
    }
    Ok(())
}
fn validate_url(url: &str) -> io::Result<()> {
    let parsed = url::Url::parse(url).map_err(io::Error::other)?;
    if parsed.scheme() != "http"
        || parsed.host_str() != Some("127.0.0.1")
        || parsed.port().is_none_or(|p| p < 1024)
        || parsed.path() != "/proxy.pac"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(io::Error::other("Invalid local proxy endpoint"));
    }
    Ok(())
}
fn variant(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}
fn state_dir() -> io::Result<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/state")))
        .ok_or_else(|| io::Error::other("No user state directory"))?;
    let directory = base.join("driftnet");
    fs::create_dir_all(&directory)?;
    let metadata = fs::symlink_metadata(&directory)?;
    if !metadata.is_dir() {
        return Err(io::Error::other("Invalid proxy state directory"));
    }
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    Ok(directory)
}
fn lock(directory: &std::path::Path) -> io::Result<fs::File> {
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(directory.join("proxy.lock"))?;
    // SAFETY: flock only receives an open file descriptor; the guard lives for the lease.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(file)
}
fn restore(settings: &impl Settings, journal: &Journal) -> io::Result<()> {
    // If another tool changed the URL, leave its settings alone. If only mode
    // changed, preserve that mode while removing our obsolete PAC address.
    if settings.get("autoconfig-url")? == journal.ours {
        if settings.get("mode")? == "'auto'" {
            settings.set("mode", &journal.mode)?;
        }
        settings.set("autoconfig-url", &journal.url)?;
    }
    Ok(())
}
fn recover_locked(path: &std::path::Path, settings: &impl Settings) -> io::Result<()> {
    let bytes = match fs::read(path) {
        Ok(v) => v,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if bytes.len() > 65536 {
        return Err(io::Error::other("Invalid proxy recovery journal"));
    }
    let journal: Journal = serde_json::from_slice(&bytes)?;
    restore(settings, &journal)?;
    fs::remove_file(path)
}
pub struct ProxyLease {
    journal: Journal,
    path: PathBuf,
    _lock: fs::File,
}
impl ProxyLease {
    pub fn recover() -> io::Result<()> {
        let directory = state_dir()?;
        let path = directory.join("proxy-journal.json");
        if !path.exists() {
            return Ok(());
        }
        let _lock = match lock(&directory) {
            Ok(v) => v,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(e) => return Err(e),
        };
        session_available()?;
        recover_locked(&path, &GSettings)
    }
    pub fn enable(url: &str) -> io::Result<Self> {
        validate_url(url)?;
        session_available()?;
        let directory = state_dir()?;
        let lock = lock(&directory)
            .map_err(|_| io::Error::other("Website routing is already running for this user"))?;
        let path = directory.join("proxy-journal.json");
        let settings = GSettings;
        recover_locked(&path, &settings)?;
        let journal = Journal {
            ours: variant(url),
            mode: settings.get("mode")?,
            url: settings.get("autoconfig-url")?,
        };
        if journal.mode != "'none'" {
            return Err(io::Error::other(
                "Another desktop proxy is active. Driftnet kept it unchanged.",
            ));
        }
        let mut temporary = tempfile::NamedTempFile::new_in(&directory)?;
        fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o600))?;
        use io::Write;
        temporary.write_all(&serde_json::to_vec(&journal)?)?;
        temporary.as_file().sync_all()?;
        temporary.persist(&path).map_err(|e| e.error)?;
        let lease = Self {
            journal,
            path,
            _lock: lock,
        };
        // A failed partial write still owns the journal and Drop restores it.
        settings.set("autoconfig-url", &lease.journal.ours)?;
        settings.set("mode", "'auto'")?;
        Ok(lease)
    }
}
impl Drop for ProxyLease {
    fn drop(&mut self) {
        if restore(&GSettings, &self.journal).is_ok() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    struct Fake(RefCell<(String, String)>);
    impl Settings for Fake {
        fn get(&self, key: &str) -> io::Result<String> {
            let v = self.0.borrow();
            Ok(if key == "mode" { &v.0 } else { &v.1 }.clone())
        }
        fn set(&self, key: &str, value: &str) -> io::Result<()> {
            let mut v = self.0.borrow_mut();
            if key == "mode" {
                v.0 = value.into();
            } else {
                v.1 = value.into();
            }
            Ok(())
        }
    }
    fn journal() -> Journal {
        Journal {
            ours: "'ours'".into(),
            mode: "'none'".into(),
            url: "'before'".into(),
        }
    }
    #[test]
    fn restores_previous_settings_and_partial_writes() {
        for mode in ["'auto'", "'none'"] {
            let fake = Fake(RefCell::new((mode.into(), "'ours'".into())));
            restore(&fake, &journal()).unwrap();
            assert_eq!(*fake.0.borrow(), ("'none'".into(), "'before'".into()));
        }
    }
    #[test]
    fn preserves_external_proxy_changes() {
        let fake = Fake(RefCell::new(("'auto'".into(), "'someone-else'".into())));
        restore(&fake, &journal()).unwrap();
        assert_eq!(fake.get("autoconfig-url").unwrap(), "'someone-else'");
        assert_eq!(fake.get("mode").unwrap(), "'auto'");
        let fake = Fake(RefCell::new(("'manual'".into(), "'ours'".into())));
        restore(&fake, &journal()).unwrap();
        assert_eq!(fake.get("mode").unwrap(), "'manual'");
    }
    #[test]
    fn rejects_nonlocal_and_ambiguous_urls() {
        assert!(validate_url("http://127.0.0.1:42000/proxy.pac").is_ok());
        for value in [
            "https://example.com/proxy.pac",
            "http://127.0.0.1:80/proxy.pac",
            "http://user@127.0.0.1:42000/proxy.pac",
            "http://127.0.0.1:42000/proxy.pac?a=b",
        ] {
            assert!(validate_url(value).is_err());
        }
    }
    fn require_disposable_gnome_session() {
        assert_eq!(std::env::var("GITHUB_ACTIONS").as_deref(), Ok("true"));
        assert_eq!(
            std::env::var("DRIFT_TEST_REAL_GSETTINGS").as_deref(),
            Ok("1")
        );
        let config = PathBuf::from(std::env::var_os("XDG_CONFIG_HOME").expect("isolated config"));
        let state = PathBuf::from(std::env::var_os("XDG_STATE_HOME").expect("isolated state"));
        assert_eq!(config.parent(), state.parent());
        assert!(config.is_absolute());
        assert!(
            config
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("driftnet-gnome-pac-")
        );
    }
    #[test]
    #[ignore = "requires the isolated disposable Linux CI DBus session"]
    fn real_gnome_proxy_crash_child() {
        require_disposable_gnome_session();
        let _lease = ProxyLease::enable("http://127.0.0.1:42001/proxy.pac").unwrap();
        assert_eq!(GSettings.get("mode").unwrap(), "'auto'");
        // Deliberately skip Rust destructors, exactly as an abruptly killed agent.
        std::process::exit(0);
    }
    #[test]
    #[ignore = "requires the isolated disposable Linux CI DBus session"]
    fn real_gnome_proxy_lease_recovers() {
        require_disposable_gnome_session();
        let settings = GSettings;
        settings.set("mode", "'none'").unwrap();
        let before = variant("http://before.example/proxy.pac");
        settings.set("autoconfig-url", &before).unwrap();
        {
            let _lease = ProxyLease::enable("http://127.0.0.1:42001/proxy.pac").unwrap();
            assert_eq!(settings.get("mode").unwrap(), "'auto'");
            assert_eq!(
                settings.get("autoconfig-url").unwrap(),
                variant("http://127.0.0.1:42001/proxy.pac")
            );
        }
        assert_eq!(settings.get("mode").unwrap(), "'none'");
        assert_eq!(settings.get("autoconfig-url").unwrap(), before);
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "platform::linux::tests::real_gnome_proxy_crash_child",
                "--ignored",
                "--nocapture",
            ])
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(settings.get("mode").unwrap(), "'auto'");
        ProxyLease::recover().unwrap();
        assert_eq!(settings.get("mode").unwrap(), "'none'");
        assert_eq!(settings.get("autoconfig-url").unwrap(), before);
        assert!(!state_dir().unwrap().join("proxy-journal.json").exists());
    }
}
