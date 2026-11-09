# Changelog

## 1.0.3 — unreleased

- Normalize Windows extended-length canonical paths (`\\?\`) to the same DOS/UNC
  spelling used by process metadata. Previously a selected app could miss its
  routing rule and use direct egress while website PAC rules still worked.
- Scope the optional Windows Discord preset to one canonical installation root:
  `Update.exe` and `app-<numeric version>/Discord.exe`, including a version created
  during an existing Access session. Custom apps retain exact executable matching.
- Preserve the authenticated process SID check and resolve reparse points before
  matching; another directory or a redirected child outside the selected root
  does not become a Discord match.
- Record a local updater-manifest HTTP 200 probe with normal TLS negotiation and
  forced TLS 1.2. Native 1.0.3 CI, affected Windows devices and provider-specific
  Discord startup/voice acceptance remain pending.

## 1.0.2 — 2026-11-10

- Preserve the original connection method and automatically try SNI-based TLS
  alternatives after an initial handshake reset, invalid response or timeout.
  Windows includes an additional SNI disorder method for filtered fiber uplinks,
  including Superonline compatibility testing. Provider-specific acceptance is
  still required; no universal ISP result is claimed.
- Bound TLS handshake retries and expire learned destination methods after five
  minutes. Retry only a complete buffered ClientHello, preventing automatic replay
  of HTTP requests or buffered TLS early/application data. Preserve UDP relay.
- Allow encrypted DNS enough time to use the bounded compatibility chain.
- Send selected-site encrypted DNS through the local engine too, fixing broker
  failures on networks that reset direct DNS-over-HTTPS connections. Keep pinned
  resolver bootstrap addresses and TLS hostname/certificate verification.
- Use rustls explicitly for the DNS client so its TLS handshake works through
  the engine on macOS as well as other platforms; the updater keeps native TLS.

## 1.0.1 — 2026-11-07

- Pin the local router to the fixed `golang.org/x/net` v0.60.0 dependency set.
  Official source and unstripped-binary vulnerability scans pass on macOS,
  Windows and Linux; unused module metadata findings remain documented.
- Ship the reviewed protected installer paths, bounded control replies,
  signed full-installer updates, GLib safety backport and neutral Driftnet credits.
- Provide the first published security release as an upgrade from the installed
  1.0.0 development build, preserving existing rules and identifiers.
- Validate native selected/direct traffic and graceful/crash cleanup on Windows,
  plus complete macOS/Ubuntu jobs, GNOME proxy recovery and the optimized GLib
  backport tests. The final Windows installer passed full native install, upgrade,
  shortcut cleanup and uninstall acceptance without recompiling its four payloads.
- Verify the actual final manifest, all three installer signatures and signed
  release checksums; reject corrupted files and signed-version relabeling.
- Normalize router source archive ownership/timestamps and verify offline builds
  for macOS arm64, Windows x64 and Linux x64/arm64.

## 1.0.0 — 2026-11-07

### Application

- Adopt the Driftnet name and amber `d_` icon, preserving `app.drift.desktop` and existing rule storage for upgrades.
- Provide one Access switch, website/application rule editors, available app icons and favicons, menu bar/tray controls, and animated edge lighting.
- Keep Discord optional: website access and bootstrap downloads work without an installed Discord app.

### Networking and lifecycle

- Route selected native applications' TCP and UDP traffic using a local TUN helper on macOS, Windows, and Linux.
- Authenticate helper clients and bind selected process ownership to the desktop user's identity.
- Keep unmatched connections on direct egress and exclude local network destinations.
- Resolve selected website A/AAAA records through DoH and try available destination addresses with failover.
- Correct IPv6 readiness detection on IPv4-only uplinks and add a Go regression test.
- Preserve the native routing session when Discord closes and reopens; three normal macOS quit/reopen cycles passed in the same continuously active Access session.
- Journal website proxy leases and preserve external proxy changes during cleanup.

### Installation and updates

- Package macOS app/helper/removal together, Windows app/service lifecycle together, and Ubuntu native app/systemd helper in a `.deb`.
- Add Linux native ELF and `.desktop` app selection; website rules target GNOME desktop proxy settings.
- Check for updates automatically and expose a manual tray check.
- Verify the complete manifest and full installer with Minisign, including the signed version, before opening the OS installer.
- Retain normal OS administrator approval and update app/helper together. Private releases remain inaccessible to anonymous update checks.

### Verification

Updater signature/tamper tests and desktop checks passed locally on macOS.
The macOS 1.0.0 package, receipt, running helper and migration were verified after
installation. Native OS proxy selection and gateway HTTP 200 passed. The Linux
policy/engine CI job passed; desktop jobs are tracked separately.
The final corresponding router source archive built offline for Darwin arm64,
Windows amd64, Linux amd64 and Linux arm64 from its vendored dependencies.
Current-version voice/video, Windows device acceptance, and Linux desktop acceptance
remain explicitly recorded in [ACCEPTANCE.md](docs/ACCEPTANCE.md). Compiling a package
is not recorded as a successful installation test.

## Earlier development builds

Early compatibility builds used explicit proxy settings for supported Electron
apps and PAC rules for selected websites. They established the rule editor,
menu bar/tray panel, local relay lifecycle, optional Discord preset, and website
bootstrap path. Their successful voice calls are not evidence for later native
routing builds.
