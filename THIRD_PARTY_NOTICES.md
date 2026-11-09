# Third-party notices

Original Driftnet code is MIT licensed. Dependency and asset licenses remain separate.

- IBM Plex Mono: SIL Open Font License, bundled copy in `licenses/IBM-Plex-Mono-OFL.txt`.
- Phosphor Icons: MIT, supplied through `@phosphor-icons/react`.
- Tauri and Rust/npm dependencies: their respective upstream licenses; pinned
  resolution is recorded in Cargo.lock and ui/package-lock.json.
- Application icons and website favicons are fetched/extracted at runtime for
  identification. They belong to their respective owners and are not relicensed
  by Driftnet's MIT license. Driftnet is not affiliated with Discord or those websites.
- ByeDPI 17.3 by hufrea: MIT, original source vendored in `vendor/byedpi`, pinned
  commit `e2684772fbda45f2ff4fc220922a010370534216`. `UPSTREAM.json` records
  source hashes. Built as the bundled `drift-engine` helper. Its license and
  provenance are included in application resources. The separate routing helper manages TUN routing; this C engine itself
  does not install interception or change network settings.

The original vendored files remain unchanged. During compilation, Driftnet applies
the documented `native/engine/fixes.json` patch to a temporary copy of desync.c.
It preserves the unsent tail when a nonblocking send succeeds partially, preventing
data loss under TCP backpressure. Additional documented integration patches bind
outbound sockets to the physical interface and restrict automatic reconnection to
a complete buffered TLS ClientHello. HTTP requests and buffered TLS early/application
data cannot be automatically replayed. Patches and upstream notices ship in resources.

- sing-box 1.14.4 by SagerNet: GPL-3.0-or-later, separate `drift-router`
  executable built from the pinned `vendor/sing-box` source. Its per-file hash
  manifest is `DRIFT_UPSTREAM.json`; integration and source patches are documented
  in `native/router/README.md` and `scripts/build-router.py`. The router's license
  is included with the app and corresponding source accompanies releases.
- Wintun 0.14.1: official signed Windows binary embedded by the router dependency,
  distributed under the upstream binary license in `licenses/Wintun-LICENSE.txt`.
  Official archive SHA-256: `07c256185d6ee3652e09fa55c0b673e2624b565e02c4b9091c79ca7d2f24ef51`.
- Update verification uses the MIT-licensed `minisign-verify` crate; the public
  update key is bundled, while the private signing key is never distributed.

`native/router` is GPL-3.0-or-later; its license is `native/router/LICENSE`.
This does not relicense third-party code under the root MIT license.

- gtk-rs `glib` 0.18.5: MIT, exact upstream pointer-mutability security backport
  for `VariantStrIter` (upstream PR 1343 / RUSTSEC-2024-0429). The patch, original
  crate digest and per-file hashes are documented in `vendor/glib/DRIFTNET_PATCH.json`
  and `native/patches`; `scripts/verify-glib-patch.py` verifies original and patched
  bytes. Original copyright and license are preserved in `vendor/glib/LICENSE`.
  The pinned path package is `0.18.5+driftnet.1`; it does not imply a GTK major upgrade.
- The Windows NSIS template is a pinned, minimally modified Tauri MIT template;
  provenance and its license are under `native/windows` and ship with Windows.
