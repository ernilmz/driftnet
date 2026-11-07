use serde::Deserialize;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Mutex,
    time::{Duration, Instant},
};
#[derive(Deserialize)]
struct Reply {
    #[serde(rename = "Status")]
    status: u32,
    #[serde(rename = "Answer", default)]
    answers: Vec<Answer>,
}
#[derive(Deserialize)]
struct Answer {
    #[serde(rename = "type")]
    kind: u16,
    data: String,
    #[serde(rename = "TTL")]
    ttl: u64,
}
struct Entry {
    addresses: Vec<IpAddr>,
    expires: Instant,
}
pub struct Resolver {
    client: reqwest::Client,
    cache: Mutex<HashMap<String, Entry>>,
}
impl Resolver {
    pub fn new() -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .https_only(true)
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(4))
            .resolve("cloudflare-dns.com", SocketAddr::from(([1, 1, 1, 1], 443)))
            .resolve("dns.google", SocketAddr::from(([8, 8, 8, 8], 443)))
            .build()?;
        Ok(Self {
            client,
            cache: Mutex::new(HashMap::new()),
        })
    }
    pub async fn addresses(&self, host: &str) -> Result<Vec<IpAddr>, std::io::Error> {
        if let Ok(ip) = host.parse() {
            return Ok(vec![ip]);
        }
        if let Some(entry) = self
            .cache
            .lock()
            .unwrap()
            .get(host)
            .filter(|e| e.expires > Instant::now())
        {
            return Ok(entry.addresses.clone());
        }
        // Internal names belong to the selected app, not to a public-site rule.
        if drift_core::normalize_domain(host).is_err() {
            let addresses: Vec<_> = tokio::net::lookup_host((host, 443))
                .await?
                .map(|a| a.ip())
                .collect();
            return if addresses.is_empty() {
                Err(std::io::Error::other("No address for selected destination"))
            } else {
                Ok(addresses)
            };
        }
        for endpoint in [
            "https://cloudflare-dns.com/dns-query",
            "https://dns.google/resolve",
        ] {
            let lookup = |kind: &'static str| async move {
                self.client
                    .get(endpoint)
                    .query(&[("name", host), ("type", kind)])
                    .header("accept", "application/dns-json")
                    .send()
                    .await?
                    .error_for_status()?
                    .json::<Reply>()
                    .await
            };
            let (v4, v6) = tokio::join!(lookup("A"), lookup("AAAA"));
            let mut addresses = Vec::new();
            let mut ttl = 300;
            for reply in [v4, v6].into_iter().flatten().filter(|r| r.status == 0) {
                for answer in reply
                    .answers
                    .into_iter()
                    .filter(|a| a.kind == 1 || a.kind == 28)
                {
                    if let Ok(ip) = answer.data.parse::<IpAddr>()
                        && !addresses.contains(&ip)
                        && addresses.len() < 8
                    {
                        addresses.push(ip);
                        ttl = ttl.min(answer.ttl);
                    }
                }
            }
            if addresses.is_empty() {
                continue;
            }
            let mut cache = self.cache.lock().unwrap();
            if cache.len() >= 512 {
                cache.retain(|_, e| e.expires > Instant::now());
                if cache.len() >= 512 {
                    cache.clear()
                }
            }
            cache.insert(
                host.to_owned(),
                Entry {
                    addresses: addresses.clone(),
                    expires: Instant::now() + Duration::from_secs(ttl.clamp(5, 300)),
                },
            );
            return Ok(addresses);
        }
        Err(std::io::Error::other(
            "Could not resolve the selected destination over trusted DNS",
        ))
    }
}
