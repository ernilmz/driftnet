# Changelog

## Unreleased — Driftnet

- Adopt the Driftnet name while preserving existing application identity and rules.
- Add native process-based routing through a local TUN helper for macOS and Windows.
- Add UDP association/relay and IPv6 destination support to the local engine.
- Resolve selected website A and AAAA records with staggered address failover.
- Add authenticated helper leases, process-owner filtering, and conditional proxy
  restoration.
- Include combined macOS installation/removal and Windows service lifecycle hooks.
- Refresh the application icon while retaining the amber `d_` identity.
- Track Discord quit/reopen, signed full-package updates, and Linux integration
  as release work requiring acceptance.

## Earlier development builds

The initial compatibility backend used explicit proxy settings for supported
Electron apps and PAC rules for selected websites. It established the rule editor,
menu bar/tray panel, local relay lifecycle, optional Discord preset, and website
bootstrap download path. Those results do not imply native routing or voice parity
for subsequent builds.
