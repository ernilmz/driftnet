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
| macOS native ownership/TUN | Selected application routing observed on the earlier 0.5 native build | The final 1.0.0 combined package needs its own installed run. |
| macOS website policy | Selected host used proxy; unrelated host returned DIRECT | One machine/connection. |
| macOS HTTP requests | System URLSession succeeded previously; 1.0.0 release probes returned HTTP 200 for gateway, avatar, update and download targets | Account activity and voice not tested by HTTP probes. |
| Website bootstrap | Windows download redirect followed; 65,536 bytes received with HTTP 206 | Downloaded content was not executed by the probe. |
| 1.0.0 website-only session | Selected example returned 200, unselected host 403, PAC DIRECT boundary and no-Discord website-only checks passed; owner EOF cleaned up | Probe mode does not start native apps or prove privileged TUN attribution. |
| macOS shutdown | Recorded complete proxy baseline restored; listeners closed | Observed stop path, not every crash or removal path. |
| macOS UI | Corner/shadow behavior, picker/icon add/remove, Escape, outside dismiss and single instance observed | Native Windows/Linux GUI acceptance remains separate. |
| Discord reopen | One real macOS quit/reopen cycle passed while the same Access session stayed on | Repeat cycles and voice still needed; do not label the bug wholly untested. |
| IPv4-only uplink correction | Unusable IPv6 tunnel advertisement corrected; three macOS native-router Go tests passed including its regression | Real IPv6-only/dual-stack networks need separate runs. |
| Signed updates | Seven integrated desktop tests passed: valid files/metadata, byte tampering, version relabeling, forged comment, architecture/URL tampering and URL boundaries | Signature correctness does not establish packaged helper replacement. |
| Desktop integration | `cargo check -p drift-desktop`, UI TypeScript and formatting passed locally | macOS development host. |
| Windows packaging | Optimized x64 executable and NSIS installer cross-built; helper resources inspected | Native CI and physical-device runtime are not yet recorded here. |
| Linux integration | Native ELF/desktop selection, GNOME PAC lease, systemd packaging and updater path implemented | Native Ubuntu CI/installation results are not yet recorded here. |
| Initial native CI | A Linux clippy failure was identified and corrected locally; updated push/rerun pending | Do not record the workflow as passed before the rerun completes. |

A previous Windows attempt exposed a mandatory-Discord dependency; that dependency
was removed. A previous compatibility backend had working voice, but it is not
counted as current-version voice evidence.

## Remaining device and packaged-flow checks

| Platform/area | Acceptance to record |
| --- | --- |
| macOS 1.0.0 package | New package opened and awaits user administrator installation; then verify one app and matching helper, preserve rules, stop/uninstall cleanly. |
| Discord relaunch | Repeat quit/reopen cycles with Access still on; verify the session does not need Off/On. |
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
