# Acceptance record

Release target: **1.0.1**. Updated: **7 November 2026**.

The installed local macOS app is **1.0.0**. It remains in the user's working Access
session; a built 1.0.1 installer is not counted as an already installed upgrade.
This record separates observed results from remaining acceptance.

## Implemented contract

- One Access switch backed by a live agent acknowledgment.
- Independent website and native application rules; Discord is an optional preset,
  and neither an installed Discord app nor a remote health check is required.
- Authenticated process ownership, exact paths/domain boundaries, and direct egress
  for unmatched connections. LAN/loopback routes are excluded.
- Native app reopening while the same session remains active.
- Combined app/helper installation, rule-preserving upgrades, lease cleanup and
  clear unhealthy-state reporting after owner/helper/network loss.
- English amber UI, animated edges, tray/menu bar controls and native About credits.
- Complete signed update manifest and full installer, normal OS admin consent,
  and no bundled private signing key or GitHub token.
- Linux native ELF/`.desktop` selection, systemd helper and GNOME website settings.

## Observed evidence

| Area | Observation | Boundary |
| --- | --- | --- |
| Policy, broker and optional app | Core/domain-boundary, missing Discord and independent website tests passed | Pure tests do not establish GUI or ISP behavior. |
| C engine | Five real TCP/UDP/IPv6/lifecycle tests passed; twenty consecutive 4 MiB backpressure regressions passed previously | Local sockets are not a voice or ISP throughput test. |
| macOS installed 1.0.0 | Package receipt and helper version verified; only Driftnet remains after migration; user reports Access works | 1.0.1 is not yet installed. |
| macOS native routing | IPv4-only TUN matches the uplink; selected/direct ownership counters observed | One user's machine and network. |
| macOS website routing | Actual CFNetwork selected Discord proxy and unrelated DIRECT; OS URLSession gateway returned HTTP 200 | HTTP success does not establish voice. |
| Discord reopen | Three normal Quit/reopen cycles reached the main screen in the same continuously active 1.0.0 session | Access was never toggled; other versions/platforms require their own result. |
| Website/bootstrap probes | Gateway/avatar/update/download HTTP 200; Windows download redirect/range HTTP 206, 65,536 bytes; selected example 200/unselected 403; no-Discord website-only and owner EOF checks passed | The downloaded Windows range was not executed by the probe. |
| macOS cleanup/UI | Full proxy baseline restored on tested stop; corner/shadow/picker/icon, Escape, outside dismissal, single instance and separate preview About checked | No physical Windows/Linux mixed-DPI GUI evidence. |
| macOS 1.0.1 package | New combined package built and bundled `--check-engine` passed | Final package byte/privacy and signature checks are recorded separately; the user installation remains 1.0.0. |
| Signed updater | Eleven integrated hardening/cryptographic tests passed; initial 1.0.0 real manifest/Mac/Windows signatures verified | New 1.0.1 installers must be signed together and reverified; actual OS-approved 1.0.0→1.0.1 update remains separate. |
| Windows native routing | Real selected/direct TUN traffic, graceful stop and crash cleanup passed with the fixed dependency set | Current final installer shortcut cleanup rerun is pending; no physical Windows 10/voice pass is inferred. |
| Linux native CI | Ubuntu package/install, systemd/TUN, selected/direct traffic, helper cleanup, GNOME PAC restoration and optimized GLib tests passed | CI runner/session evidence; not every desktop environment or compositor. |
| Native CI references | macOS and Ubuntu jobs passed in [42373554549](https://github.com/ernilmz/driftnet/actions/runs/42373554549) and [42373905313](https://github.com/ernilmz/driftnet/actions/runs/42373905313) | Overall runs were not green because Windows installer acceptance still failed. Latest shortcut correction is tracked in [42375361885](https://github.com/ernilmz/driftnet/actions/runs/42375361885). |
| GLib security backport | Official crate hash, all retained-file hashes and exact two-line upstream fix verified; optimized iterator regressions passed | Cargo's path-package scanner behavior is not the evidence of the fix. |
| Go dependency security | After exact `x/net` v0.60.0 pin, official source and identical unstripped-binary scans passed on Darwin arm64, Windows amd64 and Linux amd64 with zero reachable/imported-package findings; Go 1.27.3 standard library scan clean | Six advisories remain in unimported package metadata; source and stripped-symbol fallback results must not be conflated. |
| 1.0.1 corresponding source | 4,420 normalized archive members, 4,219 regular files; offline vendored builds passed Darwin arm64, Windows amd64, Linux amd64 and Linux arm64 | Reproducible helper source availability does not establish GUI/voice behavior. |

A previous compatibility backend had working voice, but that result is not counted
for current native routing builds. See [SECURITY-REVIEW.md](SECURITY-REVIEW.md) for
artifact/history scanning, installer findings, mitigation evidence and residual risks.

## Remaining release gates

- Complete the final Windows installer/shortcut upgrade/removal rerun and retain its
  evidence; do not call the complete platform matrix green before this passes.
- Collect the final 1.0.1 Mac/Windows/Linux installers and source, regenerate checksums,
  sign both full manifest and installers, and reverify actual final bytes.
- Publish only the completed release set. The owner authorized public visibility
  after the checks/packages are complete; visibility is not changed by this record.

## Remaining end-to-end observations

- A real signed, OS-approved 1.0.0→1.0.1 upgrade preserving the user's rules and
  replacing both GUI and helper. A signature unit test is not this installation.
- Current-version Discord audio in both directions and video separately. No
  unsolicited automated calls are made.
- Physical Windows 10 user acceptance, repeated app reopen, and mixed-DPI displays.
- Physical Linux desktop interaction beyond native CI, and multi-monitor popup
  behavior on each platform.
- Additional macOS uninstall/crash/network-change and IPv6-only/dual-stack networks
  beyond the installed IPv4 path and tested cleanup fixtures.

Production Apple notarization and Windows Authenticode certificates are not
present. OS publisher trust, administrator consent and Minisign update authenticity
are separate controls. Automatic delivery requires a publicly accessible feed;
a private or inaccessible feed quietly leaves manual installation available.
