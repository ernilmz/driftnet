//! Signed full-installer updates. Privileged helpers are updated with the app.
//! No token is embedded: private or unpublished GitHub feeds fail quietly.
#[path = "update_verify.rs"]
mod verify;

use reqwest::{blocking::Client, redirect::Policy};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{Read, Write},
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tauri::{AppHandle, Manager, State};

const ENDPOINT: &str = "https://github.com/ernilmz/driftnet/releases/latest/download/latest.json";
const MAX_INSTALLER: u64 = 256 * 1024 * 1024;
const MAX_MANIFEST: u64 = 512 * 1024;

#[derive(Clone, Default, Serialize)]
pub struct Snapshot {
    pub state: String,
    pub version: Option<String>,
    pub reason: String,
}
#[derive(Clone, Deserialize)]
struct Asset {
    url: String,
    signature: String,
}
#[derive(Deserialize)]
struct Manifest {
    version: String,
    platforms: BTreeMap<String, Asset>,
}
struct Prepared {
    folder: tempfile::TempDir,
    path: PathBuf,
    signature: String,
    version: String,
}
#[derive(Default)]
struct Inner {
    snapshot: Snapshot,
    prepared: Option<Prepared>,
}
#[derive(Default)]
pub struct Updates {
    inner: Mutex<Inner>,
    busy: AtomicBool,
}

fn platform() -> String {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    format!("{os}-{}", std::env::consts::ARCH)
}
fn installer_suffix() -> &'static str {
    if cfg!(target_os = "macos") {
        "pkg"
    } else if cfg!(windows) {
        "exe"
    } else {
        "deb"
    }
}
fn trusted_url(value: &str, release_asset: bool) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none_or(|port| port == 443)
        && url.fragment().is_none()
        && if release_asset {
            url.host_str() == Some("github.com")
                && url.query().is_none()
                && url
                    .path()
                    .starts_with("/ernilmz/driftnet/releases/download/")
                && url.path().ends_with(&format!(".{}", installer_suffix()))
        } else {
            matches!(
                url.host_str(),
                Some(
                    "github.com"
                        | "release-assets.githubusercontent.com"
                        | "objects.githubusercontent.com"
                        | "github-releases.githubusercontent.com"
                )
            )
        }
}
fn trusted_asset_url(value: &str, version: &str) -> bool {
    if !trusted_url(value, true) {
        return false;
    }
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    let prefix = format!("/ernilmz/driftnet/releases/download/v{version}/");
    let Some(filename) = url.path().strip_prefix(&prefix) else {
        return false;
    };
    !filename.is_empty()
        && filename
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}
fn release_version(value: &str) -> Result<Version, String> {
    if value.len() > 128 {
        return Err("The update version is too long.".into());
    }
    Version::parse(value).map_err(|_| "The update version is invalid.".into())
}
fn create_installer(path: &std::path::Path) -> Result<std::fs::File, String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|error| error.to_string())
}
fn client() -> Result<Client, String> {
    Client::builder()
        .user_agent(concat!("Driftnet/", env!("CARGO_PKG_VERSION")))
        .https_only(true)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(180))
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !trusted_url(attempt.url().as_str(), false) {
                attempt.error("Untrusted update redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|e| e.to_string())
}
impl Updates {
    pub fn snapshot(&self) -> Snapshot {
        self.inner.lock().unwrap().snapshot.clone()
    }
    fn set(&self, state: &str, version: Option<&str>, reason: &str) {
        self.inner.lock().unwrap().snapshot = Snapshot {
            state: state.into(),
            version: version.map(str::to_owned),
            reason: reason.into(),
        };
    }
    fn check(&self, manual: bool) -> Result<Snapshot, String> {
        if self.busy.swap(true, Ordering::AcqRel) {
            return Ok(self.snapshot());
        }
        struct Busy<'a>(&'a AtomicBool);
        impl Drop for Busy<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _busy = Busy(&self.busy);
        if self.inner.lock().unwrap().prepared.is_some() {
            return Ok(self.snapshot());
        }
        self.set("checking", None, "");
        let result = self.prepare();
        if let Err(error) = result {
            // No pop-up, network change, restart or forced authentication on an
            // inaccessible private feed. Manual checks expose a short status.
            self.set("unavailable", None, if manual { &error } else { "" });
        }
        Ok(self.snapshot())
    }
    fn prepare(&self) -> Result<(), String> {
        let client = client()?;
        let response = client
            .get(ENDPOINT)
            .send()
            .map_err(|_| "Could not reach the update feed.")?;
        if !response.status().is_success() {
            return Err("The update feed is not accessible yet. Private releases require publication before automatic updates are available.".into());
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_MANIFEST + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_MANIFEST {
            return Err("The update manifest is too large.".into());
        }
        let manifest: Manifest =
            serde_json::from_slice(&bytes).map_err(|_| "The update manifest is invalid.")?;
        // Authenticate the entire metadata too, including each platform URL.
        // The manifest and full installer have independent Minisign signatures.
        let signature_response = client
            .get(format!("{ENDPOINT}.sig"))
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|_| "The signed update manifest is not accessible yet.")?;
        let mut signature = String::new();
        signature_response
            .take(8193)
            .read_to_string(&mut signature)
            .map_err(|_| "The update manifest signature is invalid.")?;
        if signature.len() > 8192 {
            return Err("The update manifest signature is too large.".into());
        }
        verify::verify_bytes(&bytes, &signature, &manifest.version)?;
        let available = release_version(&manifest.version)?;
        let current = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|e| e.to_string())?;
        if available <= current {
            self.set("current", None, "");
            return Ok(());
        }
        let asset = manifest
            .platforms
            .get(&platform())
            .ok_or("No update is available for this platform.")?;
        if !trusted_asset_url(&asset.url, &manifest.version) || asset.signature.len() > 8192 {
            return Err("The update asset is invalid.".into());
        }
        self.set("downloading", Some(&manifest.version), "");
        let folder = tempfile::Builder::new()
            .prefix("driftnet-update-")
            .tempdir()
            .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(folder.path(), std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        // Network metadata never becomes a local path component.
        let path = folder
            .path()
            .join(format!("Driftnet-update.{}", installer_suffix()));
        let response = client
            .get(&asset.url)
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|_| "Could not download the update.")?;
        if response
            .content_length()
            .is_some_and(|size| size > MAX_INSTALLER)
        {
            return Err("The update installer is too large.".into());
        }
        let mut file = create_installer(&path)?;
        let mut response = response.take(MAX_INSTALLER + 1);
        let copied = std::io::copy(&mut response, &mut file).map_err(|e| e.to_string())?;
        if copied > MAX_INSTALLER {
            return Err("The update installer is too large.".into());
        }
        file.flush()
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        verify::verify_file(&path, &asset.signature, &manifest.version)?;
        self.inner.lock().unwrap().prepared = Some(Prepared {
            folder,
            path,
            signature: asset.signature.clone(),
            version: manifest.version.clone(),
        });
        self.set("ready", Some(&manifest.version), "");
        Ok(())
    }
}
#[tauri::command]
pub fn update_status(updates: State<'_, Updates>) -> Snapshot {
    updates.snapshot()
}
#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<Updates>().check(true))
        .await
        .map_err(|e| e.to_string())?
}
/// Call after stopping the network runtime, then exit the GUI once this succeeds.
/// A complete installer updates both app and root/service helper. OS consent is
/// left to the platform installer; this never executes an unchecked privileged command.
pub fn launch_ready(app: &AppHandle) -> Result<(), String> {
    let updates = app.state::<Updates>();
    let mut inner = updates.inner.lock().unwrap();
    let prepared = inner
        .prepared
        .as_ref()
        .ok_or("No verified update is ready.")?;
    verify::verify_file(&prepared.path, &prepared.signature, &prepared.version)?;
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("/usr/bin/open")
            .arg(&prepared.path)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("macOS could not open the update installer.".into());
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let path: Vec<_> = prepared
            .path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let verb: Vec<_> = "open".encode_utf16().chain(Some(0)).collect();
        let result = unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result as isize <= 32 {
            return Err("Windows could not open the update installer.".into());
        }
    }
    #[cfg(target_os = "linux")]
    {
        // A desktop polkit agent supplies the normal administrator approval.
        // No terminal, shell expansion or password handling is involved.
        let child = std::process::Command::new("/usr/bin/pkexec")
            .args(["/usr/bin/dpkg", "--install"])
            .arg(&prepared.path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        drop(child);
    }
    // Preserve the verified installer after GUI exit; the OS installer can still
    // be reading it. This is a private per-user temp directory, not a root write.
    let prepared = inner.prepared.take().unwrap();
    let _ = prepared.folder.keep();
    inner.snapshot = Snapshot {
        state: "installing".into(),
        version: Some(prepared.version),
        reason: String::new(),
    };
    Ok(())
}
pub fn start_background(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(20));
        loop {
            let _ = app.state::<Updates>().check(false);
            std::thread::sleep(Duration::from_secs(6 * 60 * 60));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_release_installers_for_this_platform_are_downloadable() {
        let suffix = installer_suffix();
        assert!(trusted_url(
            &format!(
                "https://github.com/ernilmz/driftnet/releases/download/v1.0.1/Driftnet.{suffix}"
            ),
            true
        ));
        for url in [
            "http://github.com/ernilmz/driftnet/releases/download/v1/a.pkg",
            "https://github.com/other/driftnet/releases/download/v1/a.pkg",
            "https://evil.example/a.pkg",
            "https://github.com/ernilmz/driftnet.evil/releases/download/v1/a.pkg",
        ] {
            assert!(!trusted_url(url, true));
        }
        assert!(!trusted_url(
            "https://github.com/ernilmz/driftnet/releases/download/v1/readme.txt",
            true
        ));
    }
    #[test]
    fn asset_tag_credentials_and_path_cannot_escape_release_policy() {
        let suffix = installer_suffix();
        let base = "https://github.com/ernilmz/driftnet/releases/download/v1.0.1";
        assert!(trusted_asset_url(
            &format!("{base}/Driftnet-1.0.1.{suffix}"),
            "1.0.1"
        ));
        for value in [
            format!("{base}/../other/Driftnet.{suffix}"),
            format!("{base}/nested/Driftnet.{suffix}"),
            format!("{base}/%2e%2e%2fDriftnet.{suffix}"),
            format!("{base}/Driftnet.{suffix}?redirect=elsewhere"),
            format!("{base}/Driftnet.{suffix}#ignored"),
            format!(
                "https://user:password@github.com/ernilmz/driftnet/releases/download/v1.0.1/Driftnet.{suffix}"
            ),
            format!(
                "https://github.com:8443/ernilmz/driftnet/releases/download/v1.0.1/Driftnet.{suffix}"
            ),
        ] {
            assert!(!trusted_asset_url(&value, "1.0.1"));
        }
        assert!(!trusted_asset_url(
            &format!("{base}/Driftnet.{suffix}"),
            "1.0.2"
        ));
    }
    #[test]
    fn version_cannot_be_a_path_or_unbounded_string() {
        assert!(release_version("1.0.1-rc.2").is_ok());
        for value in [
            "../../elsewhere",
            "1.0.1/../../../file",
            "1.0.1\\file",
            "1.0.1:alternate-stream",
        ] {
            assert!(release_version(value).is_err());
        }
        assert!(release_version(&format!("1.0.1-{}", "a".repeat(129))).is_err());
    }
    #[test]
    fn staged_installer_never_overwrites_an_existing_path() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("installer.pkg");
        std::fs::write(&path, b"existing bytes").unwrap();
        assert!(create_installer(&path).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"existing bytes");
    }
    #[cfg(unix)]
    #[test]
    fn unix_staging_rejects_symlinks_and_is_private_at_creation() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target");
        std::fs::write(&target, b"untouched").unwrap();
        let link = directory.path().join("installer.pkg");
        symlink(&target, &link).unwrap();
        assert!(create_installer(&link).is_err());
        assert_eq!(std::fs::read(target).unwrap(), b"untouched");
        let file = create_installer(&directory.path().join("new.pkg")).unwrap();
        assert_eq!(file.metadata().unwrap().permissions().mode() & 0o777, 0o600);
    }
}
