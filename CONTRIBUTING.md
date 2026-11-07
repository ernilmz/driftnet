# Contributing to Driftnet

Start with the [README](README.md), [network architecture](docs/ENGINE.md) and
[acceptance record](docs/ACCEPTANCE.md). The project contains shared Rust policy
and session code, a Tauri/React interface, a C engine and a separate GPL native
router. Their licenses are recorded in [third-party notices](THIRD_PARTY_NOTICES.md).

## Work locally

Build on the target OS when changing native behavior. Run
`scripts/build-sidecars.py --release` before bundling the desktop. Normal developer
requirements and platform packaging commands are in the README. The router build
shares `target/router-src`; use separate checkouts for simultaneous target builds.

Keep the installed app and a development bundle distinct. Tunnel, installer and
removal checks affect host networking or installed files; use a suitable test host
or VM and record its starting configuration. `scripts/probe-network.py` is an
opt-in endpoint probe that leaves system proxy/DNS settings unchanged.

## Checks

Run the checks relevant to the change:

```sh
cargo fmt --all -- --check
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
python3 scripts/test-engine.py
cd ui
npm run format:check
npm run build
```

Desktop tests need native Tauri dependencies. The CI matrix runs macOS, Windows and
Ubuntu 24.04 desktop builds; a job definition is not evidence that its current run
passed. After generating the router source, its native Go tests are available in
`target/router-src`:

```sh
go test -tags with_gvisor ./cmd/drift-router ./common/process
```

Describe the concrete trigger, changed behavior, affected platforms and observed
checks. For networking changes, distinguish relay tests from process attribution,
installer acceptance and real voice. Add a regression when it captures the failure;
avoid a test that merely repeats the implementation.

## Preserve the routing contract

Unknown and unselected identities stay on direct egress. Do not replace canonical
process paths with basename matches or domain rules with shared CDN IPs. Authenticate
ownership before application matching. Keep proxy changes scoped and journaled,
preserve external changes, and clean up on owner/helper loss. Readiness must come
from the backend rather than a simulated healthy UI state.

Display names may change; `app.drift.desktop` and migration/helper identities are
intentionally stable. Rule storage belongs outside the installation directory.
Test an upgrade before changing paths, service names or installer identities.

Do not edit pinned vendor source in place. Document a build-time change, verify
original hashes, retain notices and regenerate the corresponding router source
archive whenever the distributed router changes.

## Prepare signed updates

Update signing authenticates payloads; it is separate from Apple Developer ID,
notarization and Windows Authenticode. Use the existing maintainer Minisign key.
Keep its private file outside the checkout with directory mode 0700 and file mode
0600 on Unix. The application embeds only
`apps/desktop/resources/updater-public-key.txt`. Back the private key up securely:
a replacement key is not automatically trusted by already-installed clients.

Build and, where credentials are available, apply OS publisher signatures first.
Then sign the final complete installers, not app-only bundles:

```sh
python3 scripts/prepare-updates.py --version 1.0.0 \
  --asset darwin-aarch64=dist/Driftnet-1.0.0-macOS.pkg \
  --output dist/updates
```

Add `--asset windows-x86_64=PATH_TO_SETUP_EXE` and
`--asset linux-x86_64=PATH_TO_DEB` for the available packages. Other supported target
keys are listed by the script. Use distinct installer filenames for distinct
architectures. `DRIFTNET_SIGNING_KEY` can point to a private key outside the checkout;
never put the key contents in an issue, command transcript or source file.

The script produces complete installers, their `.sig` files, `latest.json` and
`latest.json.sig`. It does not upload anything. Attach the matching set to the
`vVERSION` release; both signatures authenticate the same version. Do not modify
installer bytes, manifest URLs or filenames afterward without regenerating the
relevant signatures. Preserve the corresponding GPL router source and notices
alongside distributed binaries.

The application's anonymous updater cannot read private GitHub releases. Keep
manual installation available to repository members; never solve private-feed
access by embedding a token. Only make the repository/release public with the
owner's explicit direction.

## Repository hygiene

Do not commit private keys, certificates, credentials, personal captures/account
screenshots, local user paths or generated installers. Release binaries belong in
release assets, not source history. Keep documentation candid about observed tests;
update [ACCEPTANCE.md](docs/ACCEPTANCE.md) and [status.json](docs/status.json) from
actual runs rather than inferred platform parity.
