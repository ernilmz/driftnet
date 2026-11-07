use super::App;
use base64::Engine as _;
use std::{
    io,
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Command, Stdio},
};
pub fn resolve(identity: &str) -> io::Result<PathBuf> {
    if identity == "discord" {
        let root = PathBuf::from(
            std::env::var_os("LOCALAPPDATA")
                .ok_or_else(|| io::Error::other("No local app data folder"))?,
        )
        .join("Discord");
        let mut paths = std::fs::read_dir(&root)?
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let version = name
                    .strip_prefix("app-")?
                    .split('.')
                    .map(str::parse::<u32>)
                    .collect::<Result<Vec<_>, _>>()
                    .ok()?;
                let path = e.path().join("Discord.exe");
                path.is_file().then_some((version, path))
            })
            .collect::<Vec<_>>();
        paths.sort_by(|a, b| a.0.cmp(&b.0));
        return paths.pop().map(|(_, p)| p).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Discord is not installed for this Windows user",
            )
        });
    }
    let path = PathBuf::from(
        identity
            .strip_prefix("windows:")
            .ok_or_else(|| io::Error::other("Invalid Windows app identity"))?,
    );
    if path.is_file() {
        Ok(path)
    } else {
        Err(io::Error::other("The selected app is not installed"))
    }
}
pub fn restart(app: &App, proxy: Option<u16>) -> io::Result<()> {
    // Windows 10 includes PowerShell. Exact path + current session matching avoids
    // generic Discord/Update.exe basename kills. Only Discord gets a forced close;
    // other apps may refuse a graceful close to preserve unsaved work.
    let path = app.path.to_string_lossy().replace('\'', "''");
    let force = if app.discord { "$true" } else { "$false" };
    let script = format!(
        r#"$ErrorActionPreference='Stop'; $path='{path}'; $session=(Get-Process -Id $PID).SessionId; $items=@(Get-Process | Where-Object {{ $_.SessionId -eq $session -and $_.Path -ieq $path }}); if($items.Count -eq 0){{exit 10}}; foreach($p in $items){{[void]$p.CloseMainWindow()}}; $until=[DateTime]::UtcNow.AddSeconds(4); do{{$left=@($items | Where-Object {{-not $_.HasExited}}); if($left.Count -eq 0){{exit 0}}; Start-Sleep -Milliseconds 100}}while([DateTime]::UtcNow -lt $until); if({force}){{foreach($p in $left){{if(-not $p.HasExited){{Stop-Process -InputObject $p -Force}}}}; exit 0}}; exit 12"#
    );
    let bytes: Vec<_> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let result = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded])
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    match result.code() {
        Some(0) => {}
        Some(10) if proxy.is_none() => return Ok(()),
        Some(10) => {}
        _ => {
            return Err(io::Error::other(format!(
                "Close {} and try again. Drift will not force-quit it.",
                app.name
            )));
        }
    }
    let mut command = Command::new(&app.path);
    command
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(port) = proxy {
        let proxy = format!("http://127.0.0.1:{port}");
        command.arg(format!("--proxy-server={proxy}"));
        for name in ["HTTP_PROXY", "http_proxy", "HTTPS_PROXY", "https_proxy"] {
            command.env(name, &proxy);
        }
        for name in ["NO_PROXY", "no_proxy"] {
            command.env(name, "localhost,127.0.0.1,::1");
        }
    }
    command.spawn()?;
    Ok(())
}
