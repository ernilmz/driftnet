//! Platform-independent policy. Unknown identities never broaden a rule.
pub mod panel;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, net::IpAddr, path::Path};
use thiserror::Error;
use uuid::Uuid;

/// The optional Discord preset covers its app, website, downloads and media.
/// Match owned domain labels, never a shared CDN IP or a hostname substring.
pub const DISCORD_DOMAINS: &[&str] = &[
    "discord.com",
    "discordapp.com",
    "discordapp.net",
    "discord.gg",
];

#[derive(Debug, Error)]
pub enum Error {
    #[error("Enter a valid website, such as example.com.")]
    InvalidDomain,
    #[error("This item is already in your list.")]
    Duplicate,
    #[error("Item not found.")]
    NotFound,
    #[error("Unsupported configuration version.")]
    Schema,
    #[error("A valid app identity is required.")]
    InvalidApp,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn normalize_domain(input: &str) -> Result<String, Error> {
    let input = input.trim();
    if input.is_empty() || input.contains('*') || input.chars().any(char::is_whitespace) {
        return Err(Error::InvalidDomain);
    }
    let with_scheme = if input.contains("://") {
        input.to_owned()
    } else {
        format!("https://{input}")
    };
    let parsed = url::Url::parse(&with_scheme).map_err(|_| Error::InvalidDomain)?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(Error::InvalidDomain);
    }
    let host = parsed
        .host_str()
        .ok_or(Error::InvalidDomain)?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.parse::<IpAddr>().is_ok()
        || host.starts_with('[')
        || host.len() > 253
        || psl::domain_str(&host).is_none()
        || host.split('.').any(|s| {
            s.is_empty()
                || s.len() > 63
                || s.starts_with('-')
                || s.ends_with('-')
                || !s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
    {
        return Err(Error::InvalidDomain);
    }
    Ok(host)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    Discord,
    Site {
        domain: String,
        subdomains: bool,
    },
    App {
        identity: String,
        display_name: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: Uuid,
    pub enabled: bool,
    pub target: Target,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub schema_version: u32,
    pub rules: Vec<Rule>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: 1,
            rules: vec![Rule {
                id: Uuid::from_u128(1),
                enabled: true,
                target: Target::Discord,
            }],
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<(), Error> {
        if self.schema_version != 1 {
            return Err(Error::Schema);
        }
        let mut ids = std::collections::HashSet::new();
        let mut targets = std::collections::HashSet::new();
        for rule in &self.rules {
            if !ids.insert(rule.id) {
                return Err(Error::Duplicate);
            }
            match &rule.target {
                Target::Site { domain, .. } => {
                    if normalize_domain(domain)? != *domain {
                        return Err(Error::InvalidDomain);
                    }
                }
                Target::App {
                    identity,
                    display_name,
                } if identity.is_empty() || display_name.trim().is_empty() => {
                    return Err(Error::InvalidApp);
                }
                _ => {}
            }
            let key = match &rule.target {
                Target::Discord => "discord".into(),
                Target::Site { domain, .. } => format!("site:{domain}"),
                Target::App { identity, .. } => format!("app:{identity}"),
            };
            if !targets.insert(key) {
                return Err(Error::Duplicate);
            }
        }
        Ok(())
    }
    pub fn add_site(&mut self, input: &str) -> Result<(), Error> {
        self.add(Target::Site {
            domain: normalize_domain(input)?,
            subdomains: true,
        })
    }
    pub fn add_app(&mut self, identity: String, display_name: String) -> Result<(), Error> {
        self.add(Target::App {
            identity,
            display_name,
        })
    }
    fn add(&mut self, target: Target) -> Result<(), Error> {
        let mut candidate = self.clone();
        candidate.rules.push(Rule {
            id: Uuid::new_v4(),
            enabled: true,
            target,
        });
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
    pub fn set_enabled(&mut self, id: Uuid, value: bool) -> Result<(), Error> {
        self.rules
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or(Error::NotFound)?
            .enabled = value;
        Ok(())
    }
    pub fn remove(&mut self, id: Uuid) -> Result<(), Error> {
        let index = self
            .rules
            .iter()
            .position(|r| r.id == id)
            .ok_or(Error::NotFound)?;
        self.rules.remove(index);
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self, Error> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let config: Self = serde_json::from_slice(&fs::read(path)?)?;
        config.validate()?;
        Ok(config)
    }
    pub fn save(&self, path: &Path) -> Result<(), Error> {
        self.validate()?;
        let parent = path
            .parent()
            .ok_or_else(|| std::io::Error::other("Missing config directory"))?;
        fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        }
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(&serde_json::to_vec_pretty(self)?)?;
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|e| e.error)?;
        #[cfg(unix)]
        {
            fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
    }
    pub fn decide(&self, flow: &Flow<'_>) -> Decision {
        for rule in self.rules.iter().filter(|r| r.enabled) {
            let matched = match &rule.target {
                Target::Discord => {
                    flow.app_identity == Some("macos:com.hnc.Discord")
                        || flow.verified_discord_installation
                        || flow
                            .hostname
                            .and_then(|h| normalize_domain(h).ok())
                            .is_some_and(|h| {
                                DISCORD_DOMAINS.iter().any(|domain| {
                                    h == *domain || h.ends_with(&format!(".{domain}"))
                                })
                            })
                }
                Target::App { identity, .. } => flow.app_identity == Some(identity.as_str()),
                Target::Site { domain, subdomains } => flow
                    .hostname
                    .and_then(|h| normalize_domain(h).ok())
                    .is_some_and(|h| {
                        h == *domain || (*subdomains && h.ends_with(&format!(".{domain}")))
                    }),
            };
            if matched {
                return Decision::Selected { rule_id: rule.id };
            }
        }
        Decision::Direct
    }
}
/// Caller must provide identity from trusted OS metadata, never from UI text.
/// Windows Discord detection must validate the installation root, not just Update.exe.
#[derive(Default)]
pub struct Flow<'a> {
    pub app_identity: Option<&'a str>,
    pub hostname: Option<&'a str>,
    pub verified_discord_installation: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Direct,
    Selected { rule_id: Uuid },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_discord_preset_covers_website_and_download_domain_boundaries() {
        let mut config = Config::default();
        for host in [
            "discord.com",
            "cdn.discordapp.com",
            "stable.dl2.discordapp.net",
            "discord.gg",
        ] {
            assert!(matches!(
                config.decide(&Flow {
                    hostname: Some(host),
                    ..Default::default()
                }),
                Decision::Selected { .. }
            ));
        }
        for host in ["discord.com.evil.org", "evil-discord.com", "example.com"] {
            assert_eq!(
                config.decide(&Flow {
                    hostname: Some(host),
                    ..Default::default()
                }),
                Decision::Direct
            );
        }
        config.rules[0].enabled = false;
        assert_eq!(
            config.decide(&Flow {
                hostname: Some("discord.com"),
                ..Default::default()
            }),
            Decision::Direct
        );
    }
    #[test]
    fn normalizes_url_idn_and_case() {
        assert_eq!(
            normalize_domain(" HTTPS://EXAMPLE.COM/path?q=x ").unwrap(),
            "example.com"
        );
        assert_eq!(normalize_domain("bücher.de").unwrap(), "xn--bcher-kva.de");
    }
    #[test]
    fn rejects_broad_and_ambiguous_rules() {
        for input in [
            "com",
            "co.uk",
            "*.com",
            "127.0.0.1",
            "[::1]",
            "https://user:pass@example.com",
            "ftp://example.com",
            "evil example.com",
            "-example.com",
        ] {
            assert!(normalize_domain(input).is_err(), "{input}");
        }
    }
    #[test]
    fn exact_app_identity_and_default_direct() {
        let c = Config::default();
        assert!(matches!(
            c.decide(&Flow {
                app_identity: Some("macos:com.hnc.Discord"),
                ..Default::default()
            }),
            Decision::Selected { .. }
        ));
        for id in [
            "macos:com.hnc.Discord.fake",
            "Update.exe",
            "Discord.exe",
            "macos:com.google.Chrome",
        ] {
            assert_eq!(
                c.decide(&Flow {
                    app_identity: Some(id),
                    ..Default::default()
                }),
                Decision::Direct
            );
        }
        assert_eq!(c.decide(&Flow::default()), Decision::Direct);
    }
    #[test]
    fn domain_boundaries_do_not_capture_shared_cdn_neighbors() {
        let mut c = Config {
            schema_version: 1,
            rules: vec![],
        };
        c.add_site("example.com").unwrap();
        for host in ["example.com", "cdn.example.com", "a.b.example.com"] {
            assert!(matches!(
                c.decide(&Flow {
                    hostname: Some(host),
                    ..Default::default()
                }),
                Decision::Selected { .. }
            ));
        }
        for host in [
            "evilexample.com",
            "example.com.evil.org",
            "example.net",
            "1.2.3.4",
        ] {
            assert_eq!(
                c.decide(&Flow {
                    hostname: Some(host),
                    ..Default::default()
                }),
                Decision::Direct
            );
        }
    }
    #[test]
    fn disabled_and_removed_rules_are_direct() {
        let mut c = Config::default();
        c.add_site("example.com").unwrap();
        let id = c.rules[1].id;
        c.set_enabled(id, false).unwrap();
        assert_eq!(
            c.decide(&Flow {
                hostname: Some("example.com"),
                ..Default::default()
            }),
            Decision::Direct
        );
        c.remove(id).unwrap();
        assert_eq!(c.rules.len(), 1);
    }
    #[test]
    fn saves_atomically_and_retains_existing_config_on_validation_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut c = Config::default();
        c.save(&path).unwrap();
        c.add_site("example.com").unwrap();
        c.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap().rules.len(), 2);
        c.schema_version = 99;
        assert!(c.save(&path).is_err());
        assert_eq!(Config::load(&path).unwrap().schema_version, 1);
    }
    #[test]
    fn duplicate_update_does_not_mutate_config() {
        let mut c = Config::default();
        c.add_site("example.com").unwrap();
        assert!(c.add_site("https://EXAMPLE.COM/foo").is_err());
        assert_eq!(c.rules.len(), 2);
    }
}
