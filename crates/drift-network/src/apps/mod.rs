use drift_core::{Config, Target};
use std::{io, path::PathBuf};
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
pub use linux::{
    DesktopEntry as LinuxDesktopEntry, desktop_entry as linux_desktop_entry,
    icon_path as linux_icon_path,
};
pub fn resolve(identity: &str) -> io::Result<PathBuf> {
    #[cfg(target_os = "macos")]
    return macos::resolve(identity);
    #[cfg(windows)]
    return windows::resolve(identity);
    #[cfg(target_os = "linux")]
    return linux::resolve(identity);
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    {
        let _ = identity;
        Err(io::Error::other("Desktop apps are unavailable"))
    }
}
#[derive(Clone)]
pub struct App {
    pub identity: String,
    pub path: PathBuf,
    pub name: String,
    pub discord: bool,
}
pub struct Launches {
    apps: Vec<App>,
    started: std::sync::atomic::AtomicBool,
}
impl Launches {
    pub fn prepare(config: &Config) -> io::Result<Self> {
        Self::prepare_with(config, resolve)
    }

    fn prepare_with(
        config: &Config,
        mut resolve: impl FnMut(&str) -> io::Result<PathBuf>,
    ) -> io::Result<Self> {
        let mut apps = Vec::new();
        for rule in config.rules.iter().filter(|r| r.enabled) {
            let (identity, name, discord) = match &rule.target {
                Target::Discord => (
                    if cfg!(target_os = "macos") {
                        "macos:com.hnc.Discord"
                    } else {
                        "discord"
                    },
                    "Discord",
                    true,
                ),
                Target::App {
                    identity,
                    display_name,
                } => (identity.as_str(), display_name.as_str(), false),
                _ => continue,
            };
            let path = match resolve(identity) {
                Ok(path) => path,
                // The included preset is optional. A fresh computer must be
                // able to reach the website/download before Discord is installed.
                Err(error) if discord && error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            if !discord && !compatible(&path) {
                return Err(io::Error::other(format!(
                    "{name} needs system routing. This build supports proxy-compatible desktop apps."
                )));
            }
            apps.push(App {
                identity: identity.into(),
                path,
                name: name.into(),
                discord,
            });
        }
        Ok(Self {
            apps,
            started: std::sync::atomic::AtomicBool::new(false),
        })
    }
    pub fn start(&self, port: u16) -> io::Result<()> {
        for (index, app) in self.apps.iter().enumerate() {
            if let Err(error) = restart(app, Some(port)) {
                for app in &self.apps[..index] {
                    let _ = restart(app, None);
                }
                return Err(error);
            }
        }
        self.started
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
    pub fn restore(&self) {
        if !self
            .started
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
        for app in &self.apps {
            let _ = restart(app, None);
        }
    }
}

#[cfg(target_os = "macos")]
fn compatible(path: &std::path::Path) -> bool {
    path.join("Contents/Frameworks/Electron Framework.framework")
        .exists()
}
#[cfg(windows)]
fn compatible(path: &std::path::Path) -> bool {
    path.parent()
        .is_some_and(|p| p.join("resources/app.asar").exists() || p.join("chrome.dll").exists())
}
#[cfg(not(any(target_os = "macos", windows)))]
fn compatible(_path: &std::path::Path) -> bool {
    false
}
fn restart(app: &App, proxy: Option<u16>) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    return macos::restart(app, proxy);
    #[cfg(windows)]
    return windows::restart(app, proxy);
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (app, proxy);
        Err(io::Error::other(
            "Desktop launching is supported on macOS and Windows",
        ))
    }
}

impl Drop for Launches {
    fn drop(&mut self) {
        self.restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_optional_discord_does_not_block_website_access() {
        let config = Config::default();
        let launches =
            Launches::prepare_with(&config, |_| Err(io::Error::from(io::ErrorKind::NotFound)))
                .unwrap();
        assert!(launches.apps.is_empty());
        assert!(crate::pac::has_sites(&config));
        assert!(matches!(
            config.decide(&drift_core::Flow {
                hostname: Some("stable.dl2.discordapp.net"),
                ..Default::default()
            }),
            drift_core::Decision::Selected { .. }
        ));
    }
    #[test]
    fn websites_do_not_require_any_discord_rule_or_app_lookup() {
        let mut config = Config::default();
        config.rules.clear();
        config.add_site("example.com").unwrap();
        let launches = Launches::prepare_with(&config, |_| {
            panic!("A website must not resolve a desktop app")
        })
        .unwrap();
        assert!(launches.apps.is_empty());
        assert!(crate::pac::has_sites(&config));
        assert!(matches!(
            config.decide(&drift_core::Flow {
                hostname: Some("example.com"),
                ..Default::default()
            }),
            drift_core::Decision::Selected { .. }
        ));
        assert_eq!(
            config.decide(&drift_core::Flow {
                hostname: Some("discord.com"),
                ..Default::default()
            }),
            drift_core::Decision::Direct
        );
    }
}
