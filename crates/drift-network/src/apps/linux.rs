//! Native Linux executable selection. Sandboxed launchers require a separate backend.
use std::{
    env, fs, io,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

pub struct DesktopEntry {
    pub name: String,
    pub icon: Option<String>,
    pub executable: PathBuf,
}

pub fn resolve(identity: &str) -> io::Result<PathBuf> {
    if identity == "discord" {
        for path in [
            "/usr/share/discord/Discord",
            "/opt/Discord/Discord",
            "/usr/lib/discord/Discord",
        ] {
            if let Ok(path) = native_executable(Path::new(path)) {
                return Ok(path);
            }
        }
        return find_command("discord").and_then(|p| native_executable(&p));
    }
    let path = Path::new(
        identity
            .strip_prefix("linux:")
            .ok_or_else(|| io::Error::other("Invalid Linux app identity"))?,
    );
    if !path.is_absolute() {
        return Err(io::Error::other("Select an absolute application path"));
    }
    if path.extension().is_some_and(|e| e == "desktop") {
        return desktop_entry(path).map(|e| e.executable);
    }
    native_executable(path)
}

fn find_command(command: &str) -> io::Result<PathBuf> {
    if command.contains('/') {
        let path = PathBuf::from(command);
        return if path.is_absolute() {
            Ok(path)
        } else {
            Err(io::Error::other(
                "Relative application paths are unsupported",
            ))
        };
    }
    // Ignore relative PATH components, which would make the selected process depend on cwd.
    env::split_paths(&env::var_os("PATH").unwrap_or_default())
        .filter(|p| p.is_absolute())
        .map(|p| p.join(command))
        .find(|p| p.is_file())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "This native application is not installed",
            )
        })
}
fn native_executable(path: &Path) -> io::Result<PathBuf> {
    if path
        .extension()
        .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("AppImage"))
    {
        return Err(io::Error::other(
            "AppImage executables are unsupported; install the native package instead.",
        ));
    }
    let path = fs::canonicalize(path)?;
    let metadata = fs::metadata(&path)?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(io::Error::other("Select an executable application"));
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if [
        "flatpak",
        "snap",
        "env",
        "sh",
        "bash",
        "dash",
        "zsh",
        "python",
        "python3",
        "node",
        "perl",
        "ruby",
        "gtk-launch",
        "xdg-open",
    ]
    .contains(&name)
    {
        return Err(io::Error::other(
            "Select the app's native executable. Shared interpreters and sandbox launchers are unsupported.",
        ));
    }
    use io::Read;
    let mut magic = [0u8; 4];
    fs::File::open(&path)?.read_exact(&mut magic)?;
    if magic == *b"\x7fELF" {
        return Ok(path);
    }
    // Common distribution wrappers launch these dedicated native processes.
    let mut alternatives = vec![];
    if let Some(parent) = path.parent() {
        alternatives.push(parent.join(match name {
            "google-chrome" | "google-chrome-stable" => "chrome",
            "firefox" => "firefox-bin",
            _ => "__unsupported_wrapper__",
        }));
    }
    match name {
        "google-chrome" | "google-chrome-stable" => {
            alternatives.push("/opt/google/chrome/chrome".into())
        }
        "firefox" => alternatives.push("/usr/lib/firefox/firefox".into()),
        "firefox-esr" => alternatives.push("/usr/lib/firefox-esr/firefox-esr".into()),
        "chromium" | "chromium-browser" => alternatives.push("/usr/lib/chromium/chromium".into()),
        "discord" => alternatives.push("/usr/share/discord/Discord".into()),
        _ => {}
    }
    for alternative in alternatives {
        if alternative == path {
            continue;
        }
        if let Ok(meta) = fs::metadata(&alternative) {
            if meta.is_file() && meta.permissions().mode() & 0o111 != 0 {
                let mut magic = [0u8; 4];
                if fs::File::open(&alternative)
                    .and_then(|mut f| f.read_exact(&mut magic))
                    .is_ok()
                    && magic == *b"\x7fELF"
                {
                    return fs::canonicalize(alternative);
                }
            }
        }
    }
    Err(io::Error::other(
        "This launcher is unsupported. Choose the installed native executable; Flatpak, Snap, scripts and AppImage are not supported yet.",
    ))
}

/// Parse Exec without invoking a shell or expanding command substitutions.
fn exec_words(value: &str) -> io::Result<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for c in value.chars() {
        if escaped {
            word.push(c);
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            c => word.push(c),
        }
    }
    if quoted || escaped {
        return Err(io::Error::other("Malformed application Exec entry"));
    }
    if !word.is_empty() {
        words.push(word);
    }
    Ok(words)
}
fn exec_command(value: &str) -> io::Result<PathBuf> {
    let words = exec_words(value)?;
    let mut index = 0;
    if words
        .first()
        .is_some_and(|v| Path::new(v).file_name().is_some_and(|n| n == "env"))
    {
        index = 1;
        while let Some(word) = words.get(index) {
            if word == "--" {
                index += 1;
                break;
            }
            if word == "-i"
                || word == "--ignore-environment"
                || word.contains('=') && !word.starts_with('-')
            {
                index += 1;
                continue;
            }
            if word.starts_with('-') {
                return Err(io::Error::other("Complex env launchers are unsupported"));
            }
            break;
        }
    }
    let command = words
        .get(index)
        .filter(|v| !v.contains('%'))
        .ok_or_else(|| io::Error::other("Missing application executable"))?;
    find_command(command).and_then(|p| native_executable(&p))
}
pub fn desktop_entry(path: &Path) -> io::Result<DesktopEntry> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > 1024 * 1024 {
        return Err(io::Error::other("Application entry is too large"));
    }
    let text = fs::read_to_string(path)?;
    let mut section = false;
    let mut name = None;
    let mut icon = None;
    let mut exec = None;
    let mut kind = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line == "[Desktop Entry]";
            continue;
        }
        if !section || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "Name" => name = Some(value.to_string()),
            "Icon" => icon = Some(value.to_string()),
            "Exec" => exec = Some(value.to_string()),
            "Type" => kind = Some(value.to_string()),
            _ => {}
        }
    }
    if kind.as_deref() != Some("Application") {
        return Err(io::Error::other("Select a desktop application entry"));
    }
    Ok(DesktopEntry {
        name: name.unwrap_or_else(|| "Application".into()),
        icon,
        executable: exec_command(
            &exec.ok_or_else(|| io::Error::other("No executable in desktop entry"))?,
        )?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn words_preserve_quoted_spaces() {
        assert_eq!(
            exec_words("\"/opt/My App/bin\" --flag %U").unwrap(),
            ["/opt/My App/bin", "--flag", "%U"]
        );
        assert!(exec_words("\"unterminated").is_err());
    }
    #[test]
    fn never_runs_shell_or_relative_path() {
        assert!(exec_command("sh -c touch /tmp/surprise").is_err());
        assert!(exec_command("env -S 'sh -c anything'").is_err());
        assert!(resolve("linux:relative-path").is_err());
        assert!(resolve("windows:/bin/sh").is_err());
    }
    #[test]
    fn desktop_entry_resolves_binary_and_metadata_without_executing_it() {
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("A Native App");
        fs::write(&binary, b"\x7fELFfixture, never executed").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        let entry = directory.path().join("application.desktop");
        fs::write(&entry, format!("[Desktop Entry]\nType=Application\nName=Native App\nIcon=application-icon\nExec=env TEST=1 \"{}\" --argument %U\n[Desktop Action Ignore]\nExec=/bad\n", binary.display())).unwrap();
        let parsed = desktop_entry(&entry).unwrap();
        assert_eq!(parsed.name, "Native App");
        assert_eq!(parsed.icon.as_deref(), Some("application-icon"));
        assert_eq!(parsed.executable, fs::canonicalize(binary).unwrap());
    }
    #[test]
    fn rejects_nonexecutable_scripts_and_appimage() {
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("program");
        fs::write(&binary, b"\x7fELFfixture").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(native_executable(&binary).is_err());
        fs::write(&binary, b"#!/bin/sh\necho should-not-run").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(native_executable(&binary).is_err());
        let image = directory.path().join("program.AppImage");
        fs::write(&image, b"\x7fELFfixture").unwrap();
        fs::set_permissions(&image, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(native_executable(&image).is_err());
    }
    #[test]
    fn missing_discord_is_not_fabricated() {
        assert!(find_command("driftnet-certainly-not-an-installed-app-43932").is_err());
    }
}

/// Locate an installed PNG app icon without running desktop entry commands.
/// SVG-only themes retain the application's generic icon until vector rendering
/// is available in the desktop shell.
pub fn icon_path(icon: &str) -> Option<PathBuf> {
    let path = Path::new(icon);
    if path.is_absolute() {
        return (path.extension().is_some_and(|e| e == "png") && path.is_file())
            .then(|| path.to_owned());
    }
    if icon.is_empty() || icon.contains('/') || icon.contains('\\') || icon == "." || icon == ".." {
        return None;
    }
    let filename = if icon.ends_with(".png") {
        icon.to_owned()
    } else {
        format!("{icon}.png")
    };
    let mut roots = Vec::new();
    if let Some(home) = env::var_os("HOME") {
        roots.push(PathBuf::from(home).join(".icons"));
    }
    if let Some(data) = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
    {
        roots.push(data.join("icons"));
    } else if let Some(home) = env::var_os("HOME") {
        roots.push(PathBuf::from(home).join(".local/share/icons"));
    }
    let system =
        env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    for data in env::split_paths(&system).filter(|p| p.is_absolute()) {
        roots.push(data.join("icons"));
        let pixmap = data.join("pixmaps").join(&filename);
        if pixmap.is_file() {
            return Some(pixmap);
        }
    }
    for root in roots {
        let direct = root.join(&filename);
        if direct.is_file() {
            return Some(direct);
        }
        for theme in ["hicolor", "Adwaita", "gnome"] {
            for size in ["256x256", "128x128", "64x64", "48x48", "32x32", "512x512"] {
                let candidate = root.join(theme).join(size).join("apps").join(&filename);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}
