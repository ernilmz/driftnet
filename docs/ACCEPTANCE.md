# Acceptance record

This record distinguishes implemented behavior from verified behavior. It must
be updated from observed results before a release is described as final.

## Required behavior

- One Access switch, with a live agent acknowledgment behind On.
- An optional Discord preset; no installed Discord app, account, or remote Discord
  health endpoint is required to start a website-only session.
- Independent website rules and native application selection.
- Exact process or domain boundaries; unknown identities use direct egress.
- Reopening a selected application while Access stays on must work without toggling
  Access or retaining obsolete proxy endpoints.
- One package per platform, preserving rules during an in-place upgrade.
- Normal stop, owner loss, service failure, uninstall, and network changes must
  release routing or report a clear unhealthy state.
- English UI, readable tray branding, rounded transparent corners, and correct
  popup placement on the icon's display.
- Update authenticity must be verified. No private signing key or GitHub access
  token may be embedded in source, app resources, or release metadata.

## Evidence available

| Area | Observed result | Boundary |
| --- | --- | --- |
| Policy and popup geometry | 15 Rust core tests passed | Pure tests do not establish native Windows geometry. |
| Broker and optional-app handling | 4 network tests passed | Missing Discord and independent site rules are covered. |
| FFI policy bridge | 2 tests passed | Experimental Network Extension source is not the installed backend. |
| C engine | Five real socket tests passed | Includes TCP backpressure, UDP echo, IPv6 TCP, missing helper, and abrupt owner cleanup. |
| Backpressure regression | Twenty consecutive 4 MiB relay/cleanup runs passed after the short-send fix | Local socket transfer, not an ISP throughput benchmark. |
| Native router configuration | App-boundary and invalid-request checks passed | No privileged tunnel is started by these tests. |
| macOS package | Installed receipt and running native helper observed | Current installed package uses the legacy Drift display name pending Driftnet migration. |
| Native helper activity | Selected and direct connection counters observed | Aggregate activity does not verify every process or protocol. |
| Website policy on macOS | CFNetwork chose proxy for a selected host and DIRECT for an unrelated host | One development machine and connection. |
| System website request | URLSession using OS proxy settings received HTTP 200 from the public gateway | No account or voice call involved. |
| Website shutdown | Complete recorded system proxy baseline restored; local listeners closed | Observed for the tested stop path. |
| Public endpoints | Gateway, avatar, update endpoint, and download page returned HTTP 200 | Endpoint success is separate from app relaunch and voice. |
| Download bootstrap | Official Windows redirect followed; 65,536 bytes received with HTTP 206 | Downloaded bytes were not executed by the probe. |
| Generic website-only session | Worked without the Discord preset and refused an unrelated host | Probe mode does not start native apps or change system proxy settings. |
| Native UI on macOS | Rounded corners, bounded shadow, picker/icon add/remove, Escape, outside dismiss, and single instance observed | Windows/Linux UI still require their own acceptance. |
| Windows package | Optimized x64 executable and NSIS installer cross-built; helper/loader resources inspected | Not a real Windows installer/runtime test. |

The earlier Windows attempt found a missing-Discord dependency. The optional-preset
handling was corrected afterward; a fresh device result is still needed.

## Current regression: Discord relaunch

A user reported that Discord could not be reopened while Access remained on and
needed an Off/On cycle. This remains an explicit release gate until the corrected
build passes a real quit/reopen cycle with the same running session. Reaching the
main window once does not satisfy this gate.

## Still requiring acceptance

- Discord quit/reopen while Access remains on, including repeated cycles.
- New-version Discord voice and video; a previous version's working call cannot
  be counted as evidence for the current native routing path.
- Windows clean install without Discord, browser download, later app install,
  normal stop, service crash recovery, in-place update, and uninstall.
- Linux package install, application selection, website routing, and desktop UI
  on the supported distribution once integration is complete.
- Multi-monitor and mixed-DPI popup placement on Windows/Linux.
- Update signature rejection, combined-helper replacement, and rule preservation
  through the actual packaged update flow.
- Current branding/package migration without leaving two active installations.

Production notarization and Authenticode certificates are not present in this
workspace. Development package consent and update-payload signing are separate
from OS publisher trust.
