use super::App;
use objc2_app_kit::{NSRunningApplication, NSWorkspace};
use objc2_foundation::NSString;
use std::{
    io,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};
pub fn resolve(identity: &str) -> io::Result<PathBuf> {
    let bundle = identity
        .strip_prefix("macos:")
        .ok_or_else(|| io::Error::other("Invalid macOS app identity"))?;
    NSWorkspace::sharedWorkspace()
        .URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle))
        .and_then(|url| url.path())
        .map(|path| PathBuf::from(path.to_string()))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("The selected app is not installed: {bundle}"),
            )
        })
}
pub fn restart(app: &App, proxy: Option<u16>) -> io::Result<()> {
    let bundle = NSString::from_str(
        app.identity
            .strip_prefix("macos:")
            .ok_or_else(|| io::Error::other("Invalid macOS app identity"))?,
    );
    let running = NSRunningApplication::runningApplicationsWithBundleIdentifier(&bundle);
    let discord_pids = if app.discord {
        exact_pids(&app.path)?
    } else {
        Vec::new()
    };
    let had_process =
        running.iter().any(|p| alive(p.processIdentifier())) || !discord_pids.is_empty();
    if proxy.is_none() && !had_process {
        return Ok(());
    }
    for process in &running {
        if alive(process.processIdentifier()) {
            process.terminate();
        }
    }
    // An updater/splash can be absent from AppKit's running-app cache. Match its
    // full executable path and this user's uid, then request normal SIGTERM.
    for pid in &discord_pids {
        // SAFETY: pid came from an exact executable + current-uid process match.
        unsafe {
            libc::kill(*pid, libc::SIGTERM);
        }
    }
    let deadline = Instant::now() + Duration::from_secs(8);
    while running.iter().any(|p| alive(p.processIdentifier()))
        || discord_pids.iter().any(|p| alive(*p))
    {
        if Instant::now() > deadline {
            return Err(io::Error::other(format!(
                "Close {} and try again. Drift will not force-quit it.",
                app.name
            )));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // LaunchServices starts the app with its own macOS data-access identity.
    // Direct exec inherits the helper's responsibility and can trigger App Data
    // Protection (EPERM for Discord's own profile), even with valid signing.
    let mut command = Command::new("/usr/bin/open");
    command
        .arg("-n")
        .arg("-a")
        .arg(&app.path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(port) = proxy {
        let endpoint = format!("http://127.0.0.1:{port}");
        for name in ["HTTP_PROXY", "http_proxy", "HTTPS_PROXY", "https_proxy"] {
            command.arg("--env").arg(format!("{name}={endpoint}"));
        }
        for name in ["NO_PROXY", "no_proxy"] {
            command
                .arg("--env")
                .arg(format!("{name}=localhost,127.0.0.1,::1"));
        }
        command
            .arg("--args")
            .arg(format!("--proxy-server={endpoint}"));
    }
    if !command.status()?.success() {
        return Err(io::Error::other(format!(
            "macOS could not launch {}",
            app.name
        )));
    }
    if app.discord {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if !exact_pids(&app.path)?.is_empty() {
                break;
            }
            if Instant::now() > deadline {
                return Err(io::Error::other("Discord did not remain open after launch"));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    Ok(())
}

fn alive(pid: i32) -> bool {
    // SAFETY: signal zero only checks existence/permission; no signal is delivered.
    unsafe { libc::kill(pid, 0) == 0 }
}

fn exact_pids(bundle: &std::path::Path) -> io::Result<Vec<i32>> {
    let executable = bundle.join("Contents/MacOS/Discord").canonicalize()?;
    // SAFETY: getuid has no preconditions. No shell is used.
    let uid = unsafe { libc::getuid() }.to_string();
    let output = Command::new("/bin/ps")
        .args(["-ww", "-U", &uid, "-o", "pid=,comm="])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other("Could not inspect the selected app"));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let split = trimmed.find(char::is_whitespace)?;
            let (pid, path) = trimmed.split_at(split);
            (std::path::Path::new(path.trim()) == executable)
                .then(|| pid.parse().ok())
                .flatten()
        })
        .collect())
}
