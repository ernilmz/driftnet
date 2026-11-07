use drift_core::Config;
#[cfg(any(windows, target_os = "linux"))]
use drift_network::platform::ProxyLease;
use drift_network::{broker::Servers, pac, router};
use serde::Serialize;
use std::{
    io::{self, Write},
    sync::{Arc, RwLock},
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt};
#[derive(Serialize)]
struct Reply {
    ok: bool,
    app_port: u16,
    site_port: u16,
    pac_url: Option<String>,
    error: Option<String>,
}
fn reply(result: Result<(u16, u16, Option<String>), String>) {
    let value = match result {
        Ok((app_port, site_port, pac_url)) => Reply {
            ok: true,
            app_port,
            site_port,
            pac_url,
            error: None,
        },
        Err(error) => Reply {
            ok: false,
            app_port: 0,
            site_port: 0,
            pac_url: None,
            error: Some(error),
        },
    };
    println!("{}", serde_json::to_string(&value).unwrap());
    let _ = io::stdout().flush();
}
#[tokio::main]
async fn main() {
    #[cfg(any(windows, target_os = "linux"))]
    if std::env::args().nth(1).as_deref() == Some("--recover") {
        if let Err(error) = ProxyLease::recover() {
            eprintln!("Could not restore desktop proxy settings: {error}");
            std::process::exit(1);
        }
        return;
    }
    #[cfg(target_os = "macos")]
    if std::env::args().nth(1).as_deref() == Some("--root-helper") {
        if let Err(error) = drift_network::platform::root_daemon() {
            eprintln!("Drift helper: {error}");
            std::process::exit(1)
        }
        return;
    }
    if let Err(error) = session().await {
        reply(Err(error.to_string()));
        std::process::exit(1)
    }
}
async fn session() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
    let mut line = String::new();
    (&mut stdin).take(65537).read_line(&mut line).await?;
    if line.len() > 65536 {
        return Err("Routing configuration is too large".into());
    }
    let config: Config = serde_json::from_str(&line)?;
    config.validate()?;
    let probe = std::env::args().any(|a| a == "--probe");
    // Monitor the owner during setup as well as after readiness. Cancelling a
    // pending setup must not subsequently activate routing.
    line.clear();
    tokio::select! {
        result = run(config, probe) => result,
        input = stdin.read_line(&mut line) => { input?; Ok(()) },
        _ = tokio::signal::ctrl_c() => Ok(()),
    }
}
async fn run(config: Config, probe: bool) -> Result<(), Box<dyn std::error::Error>> {
    let apps = if probe {
        Vec::new()
    } else {
        router::applications(&config)?
    };
    let interface = if apps.is_empty() {
        None
    } else {
        Some(router::physical_interface()?)
    };
    let sites = pac::has_sites(&config);
    let executable = std::env::current_exe()?
        .parent()
        .ok_or("Missing helper directory")?
        .join(if cfg!(windows) {
            "drift-engine.exe"
        } else {
            "drift-engine"
        });
    let mut engine = drift_engine::Engine::start_bound(&executable, interface)?;
    let servers = Servers::start(engine.endpoint(), Arc::new(RwLock::new(config))).await?;
    let native_lease = if !probe && (!apps.is_empty() || (cfg!(target_os = "macos") && sites)) {
        Some(router::Lease::start(
            if cfg!(target_os = "macos") && sites {
                &servers.pac_url
            } else {
                ""
            },
            engine.endpoint().port(),
            &apps,
            interface,
        )?)
    } else {
        None
    };
    #[cfg(any(windows, target_os = "linux"))]
    let _proxy_lease = if sites && !probe {
        Some(ProxyLease::enable(&servers.pac_url)?)
    } else {
        None
    };
    reply(Ok((
        servers.app_port,
        servers.site_port,
        sites.then(|| servers.pac_url.clone()),
    )));
    let mut health = tokio::time::interval(std::time::Duration::from_secs(2));
    loop {
        health.tick().await;
        if !engine.is_running()?
            || native_lease
                .as_ref()
                .is_some_and(|lease| !lease.is_running())
        {
            // Cleanup runs through the guards below when returning or when
            // the owner's pipe cancels this future.
            #[cfg(any(windows, target_os = "linux"))]
            drop(_proxy_lease);
            drop(native_lease);
            return Err("The network engine stopped".into());
        }
    }
}
