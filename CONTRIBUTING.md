# Contributing to Driftnet

Start with the [README](README.md) and [network architecture](docs/ENGINE.md).
The repository contains three separate parts: Rust policy/session code, a Tauri
interface, and a source-built native routing helper. Their licenses are recorded
in [third-party notices](THIRD_PARTY_NOTICES.md).

## Work locally

Build on the target OS when changing native behavior. Use `scripts/build-sidecars.py`
to prepare the helpers before running the desktop build. The router build shares
`target/router-src`; use separate checkouts for simultaneous target builds.

Keep the installed app and a development bundle distinct. Native tunnel,
installer, and removal checks change host networking or installed files, so run
those checks on a dedicated test host or VM and record the starting configuration.
The public endpoint probe is opt-in and leaves system proxy/DNS settings unchanged.

## Before opening a change

Run the checks relevant to the change:

```sh
cargo fmt --all -- --check
cargo test --locked -p drift-core -p drift-network -p drift-policy-ffi -p drift-agent
cargo clippy --locked --workspace --all-targets -- -D warnings
python3 scripts/test-engine.py
cd ui
npm run format:check
npm run build
```

Describe the trigger, resulting behavior, affected platforms, and observed checks.
For a network change, distinguish local relay tests from native attribution,
installer acceptance, and actual voice. Include a small regression test when it
captures the failure rather than repeating the implementation.

## Preserve the routing contract

Unmatched or unknown identities stay direct. Do not replace exact process paths
with basename matching or domains with shared CDN IP rules. Keep DNS/proxy changes
scoped and journaled, preserve external changes, and clean up on owner loss.
Readiness must come from the working backend; the UI must not simulate a healthy
native connection.

Do not modify pinned vendor files in place. Add a documented build-time change,
verify the original hashes, retain notices, and regenerate the corresponding
router source package when the distributed executable changes.

Do not commit private keys, signing certificates, credentials, personal network
captures, account screenshots, local user paths, or generated installers. Release
assets belong in the release workflow, not source history. Ask before changing
repository visibility or publishing a release.
