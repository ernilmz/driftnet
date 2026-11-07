# Windows installer identity

`installer-template.nsi` is the official Tauri CLI 2.12.4 NSIS template under its MIT license. `NSIS_UPSTREAM.json` records the pinned upstream file and SHA-256. The only behavioral changes preserve the previously shipped `Drift` registry product name and `Software\drift` manufacturer key while allowing the visible product name to be `Driftnet`.

Do not change those registry constants when renaming the displayed product. The original installer uses them to recover the installation directory, identify upgrades, and remove the existing uninstall entry. Keeping them avoids duplicate installations and never executes an arbitrary legacy uninstall command read from a differently named registry entry. Per-user rules retain the `app.drift.desktop` configuration directory. Hooks remove former shortcuts only when their targets match this installation, stop the existing service, and install the replacement service.

When updating `@tauri-apps/cli`, refresh this template from that exact upstream tag, reapply the three registry identity lines, and run the native Windows installer integration job. Its legacy registry/shortcut fixture verifies in-place migration, configuration preservation, and one uninstall entry. That fixture is a migration test, not a claim that an older signed installer or Windows 10 GUI was exercised.
