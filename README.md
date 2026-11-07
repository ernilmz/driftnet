<div align="center">

<img src="apps/desktop/icons/icon.png" width="112" height="112" alt="Driftnet icon" />

# Driftnet

**Selected apps. Selected sites. One switch.**

A small menu bar and tray utility with a quiet amber interface.
Local routing, an optional Discord preset, and no remote VPN account.

[Install](#install) · [How it works](#how-it-works) · [Build](#build-from-source) · [Contribute](CONTRIBUTING.md)

</div>

## What it does

- Turns access on or off with one switch.
- Lets you add websites and desktop applications in Settings.
- Includes an optional Discord preset for the app, website, downloads, and media. Discord does not have to be installed.
- Routes selected application TCP and UDP connections through a local engine, without requiring each app to support a proxy.
- Keeps unmatched connections on direct egress and excludes local network and loopback destinations from the application tunnel.
- Restores the website proxy settings when the session ends. Rules stay on your device.

Driftnet sends traffic to its actual destination. It does not provide a remote VPN
server, change your public IP address, or require a subscription. Its effectiveness
depends on the protocol, destination, and network restrictions.

## Install

Download the appropriate installer from the repository’s
[Releases](https://github.com/ernilmz/driftnet/releases). While the repository is
private, downloads require access to it.

| Platform | Package | Notes |
| --- | --- | --- |
| macOS 13+ | Combined `.pkg` | Installs the app and networking helper together. Apple Silicon is the locally built target; this routing mode does not require Apple Developer membership. |
| Windows 10/11 x64 | `Setup.exe` | Installs the app and service; handles WebView2 when needed. |
| Linux | Ubuntu/Debian `.deb` target | Linux integration is being prepared; use only a package explicitly included in a release. |

Install once and approve the operating system’s normal administrator request.
Open Driftnet, click its menu bar or tray icon, and turn **Access** on. There are
no ports, proxy addresses, browser extensions, or separate engine programs to set up.

In **Settings → Websites**, add a domain such as `example.com`. Its subdomains
are included. Sites can use other domains for sign-in, images, or media; add those
explicitly when needed. In **Settings → Apps**, choose an application. The Discord
preset appears under both tabs because it covers the app and its web services.
You can disable or remove it and use Driftnet only with your own rules.

Turn Access off or choose **Quit Driftnet** to end the session. Use **Uninstall
Driftnet…** from the tray menu for complete removal, including the networking helper.
An update should preserve your existing rules.

Current development packages do not have production Apple notarization or Windows
Authenticode signatures. First-open approval may be required. Keep OS security
protections enabled.

## How it works

The interface and policy are shared Rust/Tauri code. A local privileged helper
creates a TUN interface and attributes connections to operating-system process
identities. Only selected application paths for the authenticated desktop user
enter the local SOCKS engine. Unknown or unmatched identities use direct egress.

The TUN receives public traffic locally to classify it, so “direct” describes its
egress route; it does not mean that those packets bypass all local processing.
LAN and loopback routes are excluded. Protected processes, unavailable ownership
metadata, and unusual helper processes can limit app attribution. Driftnet does
not broaden a rule to every browser or shared CDN IP to compensate.

Selected websites use an operating-system automatic proxy configuration (PAC)
and a hostname-checking broker. Unmatched hosts return `DIRECT`. Selected public
website names are resolved with TLS-validated DNS over HTTPS, including A and AAAA
answers and connection failover. System-wide DNS settings remain unchanged.

The engine supports TCP fragmentation and UDP relay. UDP relay support alone
is not evidence that every network will allow voice or QUIC. See
[network architecture](docs/ENGINE.md) for lifecycle, attribution, and protocol details.

## Release status

The repository is being prepared privately. Native routing, package generation,
and local socket checks exist; a compiled installer is not equivalent to a tested
installation on every platform. In particular, Windows device acceptance, Linux
desktop acceptance, and current-version Discord voice are tracked separately.

The reported Discord close/reopen issue is being investigated before final release.
Signed update delivery is also being integrated. A private GitHub release cannot
be fetched anonymously; no GitHub access token is embedded in the application.

See the [acceptance record](docs/ACCEPTANCE.md) for what has actually been checked.

## Build from source

Use the target OS for normal distribution. Developers need stable Rust, Node 22+,
Python 3, Go 1.27+, and the [Tauri native prerequisites](https://v2.tauri.app/start/prerequisites/).
macOS needs Command Line Tools. Windows needs the Microsoft C++ build tools and a
C compiler for the C engine. End users need none of these development tools.

From the repository root:

```sh
python3 scripts/build-sidecars.py --release
cd ui
npm ci
npm run build
cd ../apps/desktop
../../ui/node_modules/.bin/tauri build
```

On Windows, set `DRIFT_ENGINE_CC` to the compiler executable; the workflow uses
MinGW GCC. `DRIFT_GO` can select a Go executable outside `PATH`. Do not build
multiple router targets concurrently in the same checkout: they share a temporary
source directory.

To create the combined macOS development installer after bundling:

```sh
python3 scripts/sign-dev-macos.py --app target/release/bundle/macos/Driftnet.app
python3 scripts/build-macos-installer.py --app target/release/bundle/macos/Driftnet.app
```

The build verifies pinned engine and router source hashes before applying the
documented changes to temporary copies. It does not publish anything.

## Development checks

```sh
cargo fmt --all -- --check
cargo test --locked -p drift-core -p drift-network -p drift-policy-ffi -p drift-agent
cargo clippy --locked --workspace --all-targets -- -D warnings
python3 scripts/test-engine.py
cd ui
npm run format:check
npm run build
```

`probe-network.py` is an opt-in test against public endpoints. It does not launch
desktop apps or change system proxy/DNS settings. Native routing and real voice
checks require separate evidence.

## Source and licenses

The independent Rust/UI code is [MIT licensed](LICENSE). The separate router
executable incorporates GPL-3.0-or-later sing-box code; the corresponding source
archive accompanies the installers. The C engine incorporates MIT-licensed
ByeDPI 17.3 with documented build-time fixes. Windows includes the official signed
Wintun binary under its supplied binary license. Fonts, icons, and other
dependencies retain their respective licenses.

See [third-party notices](THIRD_PARTY_NOTICES.md), [router integration](native/router/README.md),
and the licenses packaged with the app. Driftnet is not affiliated with Discord,
SagerNet, WireGuard, or the websites and apps selected by users.
