use drift_core::{Config, DISCORD_DOMAINS, Target};
pub fn has_sites(config: &Config) -> bool {
    config
        .rules
        .iter()
        .any(|r| r.enabled && matches!(r.target, Target::Site { .. } | Target::Discord))
}
/// Host comparisons only: no DNS lookup, no IP/CDN rule and no catch-all proxy.
pub fn script(config: &Config, port: u16) -> String {
    let sites: Vec<_> = config
        .rules
        .iter()
        .filter(|r| r.enabled)
        .flat_map(|r| match &r.target {
            Target::Site { domain, subdomains } => vec![(domain.as_str(), *subdomains)],
            Target::Discord => DISCORD_DOMAINS
                .iter()
                .map(|domain| (*domain, true))
                .collect(),
            _ => Vec::new(),
        })
        .collect();
    format!(
        "function FindProxyForURL(url, host) {{\n  host = host.toLowerCase().replace(/\\.$/, '');\n  var sites = {};\n  for (var i = 0; i < sites.length; i++) {{\n    var domain = sites[i][0];\n    if (host === domain || (sites[i][1] && host.slice(-(domain.length + 1)) === '.' + domain)) return 'PROXY 127.0.0.1:{}';\n  }}\n  return 'DIRECT';\n}}\n",
        serde_json::to_string(&sites).unwrap(),
        port
    )
}
