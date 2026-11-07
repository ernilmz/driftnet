//! Signed full-installer updates. Privileged helpers are updated with the app.
//! No token is embedded: private or unpublished GitHub feeds fail quietly.
#[path = "update_verify.rs"]
mod verify;

use reqwest::{blocking::Client, redirect::Policy};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
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
        && if release_asset {
            url.host_str() == Some("github.com")
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
        let available =
            Version::parse(&manifest.version).map_err(|_| "The update version is invalid.")?;
        let current = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|e| e.to_string())?;
        if available <= current {
            self.set("current", None, "");
            return Ok(());
        }
        let asset = manifest
            .platforms
            .get(&platform())
            .ok_or("No update is available for this platform.")?;
        if !trusted_url(&asset.url, true) || asset.signature.len() > 8192 {
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
        let path = folder.path().join(format!(
            "Driftnet-{}.{}",
            manifest.version,
            installer_suffix()
        ));
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
        let mut file = File::create(&path).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
        }
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
}
