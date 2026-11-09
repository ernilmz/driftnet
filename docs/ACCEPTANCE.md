# Acceptance record

Release target: **1.0.2**. Updated: **10 November 2026**.

The release rows below retain the **7 November** evidence and its installation
limits. The follow-up below records the later Vodafone compatibility work
separately. This record separates observed results from remaining acceptance.

## Filtered fiber / Superonline compatibility — 1.0.2

The new engine keeps its existing successful method and adds bounded TLS-only
fallback, including a Windows SNI disorder method and a final method without TLS
record splitting. No physical Superonline uplink is available here; provider
acceptance and Windows 10/11 Discord voice/video remain user checks.

Local macOS workspace tests and clippy passed. All eight compiled-engine socket
tests passed, including the complete fallback chain/warm cache, silent TLS
timeout, no HTTP POST or buffered early-data replay, TCP backpressure, UDP, IPv6
and owner cleanup. The public endpoint probe passed Discord gateway, avatar,
update and website HTTP 200, the Windows installer redirect/range HTTP 206 with
65,536 bytes, selected/unselected site policy, PAC boundaries, owner EOF and
website-only operation. The probe did not acquire an OS routing/proxy/DNS lease.

Native Windows, macOS and Ubuntu follow-up runs are tracked in
[checks 42710358310](https://github.com/ernilmz/driftnet/actions/runs/42710358310).
Their results must be checked before final installer publication. Earlier 1.0.1
package and installer results below remain historical evidence for those bytes.

## Vodafone DNS compatibility follow-up — 10 November 2026

The local installed bundle was verified as **1.0.1** before this fix. On one
Vodafone Net IPv4 uplink, both pinned DoH resolvers reset direct TLS connections.
Three attempts per resolver failed both directly and through an unmodified local
SOCKS relay (0/6 for each mode); the existing engine fragmentation profile succeeded
6/6. The installed agent's app/site brokers returned CONNECT 502, including for an
otherwise reachable control website. This does not establish a Vodafone-wide
policy or compare other providers on the same device.

The corrected macOS arm64 release agent routes DoH through its owned engine and
uses rustls explicitly. `scripts/probe-network.py --release` passed Discord
gateway/avatar/update/website HTTP 200, the Windows download redirect/range
HTTP 206 with 65,536 bytes, selected-site HTTP 200, unselected-site CONNECT 403,
PAC boundaries, listener cleanup, owner EOF during setup and a generic website-only
session. Probe mode did not acquire an OS routing/proxy/DNS lease. An offline
SOCKS/TLS regression verifies both pinned bootstrap IPs and resolver TLS hostnames.
All 38 macOS workspace tests passed, including the five compiled-engine checks.
Native Discord application routing and actual audio/video remain separate checks.

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
| macOS 1.0.1 package | Combined package, engine smoke, 29-file privacy/license scan, root-helper BOM and actual signed bytes passed | User explicitly deferred installation; the working installed app remains 1.0.0. |
| Signed updater | Eleven integrated tests passed; production verifier accepted the actual final 1.0.1 manifest and all three complete installers; corrupted copies and version 99.0.0 relabeling were rejected; signed checksums verified too | Actual OS-approved 1.0.0→1.0.1 installation remains deferred, not inferred from cryptographic checks. |
| Windows final installer | Full native installer/runtime/import, protected-root, selected/direct TUN, graceful/crash, repair/upgrade, legacy/current/unrelated shortcut and uninstall checks passed in [42379006434](https://github.com/ernilmz/driftnet/actions/runs/42379006434) | The four compiled payload hashes match the known-good baseline. Physical Windows 10 and voice remain separate. |
| Linux native CI | Ubuntu package/install, systemd/TUN, selected/direct traffic, helper cleanup, GNOME PAC restoration and optimized GLib tests passed | CI runner/session evidence; not every desktop environment or compositor. |
| Native CI references | Mac/Ubuntu/policy jobs passed at `f108eee` in [42375361885](https://github.com/ernilmz/driftnet/actions/runs/42375361885); final installer-only Windows run [42379006434](https://github.com/ernilmz/driftnet/actions/runs/42379006434) passed at `0f044d4` | The older single matrix had a Windows installer failure; final acceptance combines the unchanged compiled baseline and tested corrected installer. |
| GLib security backport | Official crate hash, all retained-file hashes and exact two-line upstream fix verified; optimized iterator regressions passed | Cargo's path-package scanner behavior is not the evidence of the fix. |
| Go dependency security | After exact `x/net` v0.60.0 pin, official source and identical unstripped-binary scans passed on Darwin arm64, Windows amd64 and Linux amd64 with zero reachable/imported-package findings; Go 1.27.3 standard library scan clean | Six advisories remain in unimported package metadata; source and stripped-symbol fallback results must not be conflated. |
| 1.0.1 corresponding source | 4,420 normalized archive members, 4,219 regular files; offline vendored builds passed Darwin arm64, Windows amd64, Linux amd64 and Linux arm64 | Reproducible helper source availability does not establish GUI/voice behavior. |

A previous compatibility backend had working voice, but that result is not counted
for current native routing builds. See [SECURITY-REVIEW.md](SECURITY-REVIEW.md) for
artifact/history scanning, installer findings, mitigation evidence and residual risks.

## Final artifact checks

The actual 1.0.1 Mac/Linux packages and source were inspected for private-key/token
patterns, personal developer paths, unsafe archive paths, ownership and license
content. No production-secret or personal developer-path match was found. Linux
binaries retain generic public CI-runner source paths; macOS may reattach opaque
OS provenance attributes. Neither is represented as private account data or as
zero build metadata. Exact file counts and limits are in the security review.

The complete release set has independent installer/manifest signatures and signed
SHA-256 checksums covering the source and supporting assets. The production verifier
checked the actual files, including corruption and version mismatch rejection.
Public distribution is authorized after verified assets are uploaded; this record
does not install software or change repository visibility.

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
