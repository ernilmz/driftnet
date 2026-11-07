# Acceptance record

Version: **1.0.0**. Last consolidated: **7 November 2026**.

This document separates implemented behavior from observed acceptance. Update a
result only after the relevant run, and retain its platform/build boundary.

## Implemented contract

- One Access switch backed by a live agent acknowledgment.
- Optional Discord preset; an installed app, account or Discord health endpoint is
  not required to start a website-only session.
- Independent domain rules and native application selection.
- Authenticated process ownership, exact app/domain boundaries and direct egress
  for unknown or unselected connections.
- Selected native applications can close and reopen without toggling Access.
- Combined app/helper installation and in-place upgrades preserving the rule store.
- Lease cleanup and unhealthy-state reporting for owner/helper/network loss.
- English amber UI, readable tray branding, rounded popup and animated edge lighting.
- Signed complete update manifest and full installer; no private keys or GitHub
  tokens embedded in the repository, bundle or release metadata.
- Linux native ELF/`.desktop` selection, systemd helper and GNOME website settings.

## Observed evidence

| Area | Observation | Boundary |
| --- | --- | --- |
| Policy and popup geometry | Core tests passed in earlier checks | Pure geometry is not Windows/Linux display acceptance. |
| Website broker and optional preset | Missing Discord and independent site policy tests passed | No installed-app dependency for website Access. |
| FFI bridge | Tests passed in earlier checks | Experimental Network Extension code is not the installed backend. |
| 1.0.0 C engine sockets | Five real TCP/UDP/IPv6/lifecycle socket tests passed; release bundle `--check-engine` passed | Local sockets do not prove ISP or voice behavior. |
| Backpressure correction | Twenty consecutive 4 MiB relay/cleanup runs passed | Local transport regression; no external speed claim. |
| Native routing configuration | App boundaries and invalid-request checks passed | These tests do not start every platform's privileged tunnel. |
| macOS 1.0.0 package/migration | User installed package; receipt and running root helper report 1.0.0; only Driftnet remains installed | Clean uninstall and the packaged signed-update path still need their own checks. |
| macOS native ownership/TUN | 1.0.0 Access On observed, IPv4-only TUN matches the physical uplink; selected/direct ownership counters observed | One machine/connection; other platforms need native evidence. |
| macOS website policy | Actual CFNetwork settings selected proxy for Discord HTTPS and DIRECT for an unrelated host; OS URLSession gateway returned HTTP 200 | One machine/connection. |
| macOS HTTP requests | System URLSession succeeded previously; 1.0.0 release probes returned HTTP 200 for gateway, avatar, update and download targets | Account activity and voice not tested by HTTP probes. |
| Website bootstrap | Windows download redirect followed; 65,536 bytes received with HTTP 206 | Downloaded content was not executed by the probe. |
| 1.0.0 website-only session | Selected example returned 200, unselected host 403, PAC DIRECT boundary and no-Discord website-only checks passed; owner EOF cleaned up | Probe mode does not start native apps or prove privileged TUN attribution. |
| macOS shutdown | Recorded complete proxy baseline restored; listeners closed | Observed stop path, not every crash or removal path. |
| macOS UI | Corner/shadow behavior, picker/icon add/remove, Escape, outside dismiss and single instance observed | Native Windows/Linux GUI acceptance remains separate. |
| Discord reopen | Three normal Quit/reopen cycles reached the main screen during the same continuously active 1.0.0 Access session; Access was never toggled | macOS relaunch regression passed; voice and other platforms remain separate. |
| IPv4-only uplink correction | Unusable IPv6 tunnel advertisement corrected; three macOS native-router Go tests passed including its regression | Real IPv6-only/dual-stack networks need separate runs. |
| Signed updates | Seven integrated desktop tests passed: valid files/metadata, byte tampering, version relabeling, forged comment, architecture/URL tampering and URL boundaries | Signature correctness does not establish packaged helper replacement. |
| Desktop integration | `cargo check -p drift-desktop`, UI TypeScript and formatting passed locally | macOS development host. |
| Windows packaging | Optimized x64 executable and NSIS installer cross-built; helper resources inspected | Native desktop CI still running; physical-device acceptance pending. |
| Linux integration | Native ELF/desktop selection, GNOME PAC lease, systemd packaging and updater path implemented | Native desktop CI still running; Ubuntu/GNOME device acceptance pending. |
| Linux policy/engine CI | Job passed in [run 42367749112](https://github.com/ernilmz/driftnet/actions/runs/42367749112) after lint corrections | macOS, Windows and Ubuntu desktop jobs remain in progress; this is not an all-jobs pass. |
| Corresponding router source | Final 6.6 MB archive extracted to a temporary directory and built offline with `GOPROXY=off`, `GOSUMDB=off` and `-mod=vendor` for Darwin arm64, Windows amd64, Linux amd64 and Linux arm64 | Verifies the distributed helper source/dependency package can build without network access; not a native GUI or runtime test. |

A previous Windows attempt exposed a mandatory-Discord dependency; that dependency
was removed. A previous compatibility backend had working voice, but it is not
counted as current-version voice evidence.

## Remaining device and packaged-flow checks

| Platform/area | Acceptance to record |
| --- | --- |
| macOS lifecycle | Clean uninstall and crash/network-change paths beyond the already observed install/migration and normal session behavior. |
| Voice/video | A real current-version call with audio in both directions; optional video separately. No automated unsolicited calls. |
| Windows | Native CI, clean machine without Discord, website download, later app installation, selected app traffic, reopen, stop/service failure, upgrade and uninstall. |
| Linux | Native CI, Ubuntu 24.04+ GNOME/systemd install, native app selection, website rules, restoration, helper loss, upgrade and package removal. |
| Popup placement | Multi-monitor/mixed-DPI placement and native corner/shadow behavior on each supported platform. |
| Packaged updater | Publish a correctly signed test release, reject modified metadata/installer, complete OS-approved app/helper upgrade and preserve rules. |
| Network changes | Wi-Fi/interface changes, IPv4-only/dual-stack/IPv6-only paths and session health reporting. |

Production Apple notarization and Windows Authenticode certificates are not present.
OS publisher trust, administrator consent and Minisign update authentication are
separate controls. Private GitHub releases cannot be fetched anonymously; failure
to check a private feed is expected, not proof of a broken signature verifier.

## Installed macOS 1.0.0 verification

- Installed package receipt `app.drift.desktop.installer` reports 1.0.0; the new
  root helper and the Driftnet-branded app run from their installed locations.
- `/Applications/Drift.app` was removed by migration, leaving Driftnet.app.
  The separate legacy Discord Local app was stopped and moved to Trash.
- The IPv4-only physical connection now receives an IPv4-only application TUN.
- Three normal Discord Quit/reopen cycles reached its main screen in the same
  continuously active Access session; Access was never toggled during the cycles.
- Native helper counters attributed selected and direct connections.
- CFNetwork chose the local proxy for discord.com and DIRECT for example.org;
  URLSession using the actual OS settings received HTTP 200 from the public gateway.
- Discord voice/video was not tested; these results do not establish call quality.

The Linux policy/engine job in run 42367749112 passed after the documented lint
fixes. Native desktop packaging and service jobs are tracked separately.
