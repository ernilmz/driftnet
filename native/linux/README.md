# Linux backend

Initial Linux target: **Ubuntu 24.04 or later, x86-64/ARM64, systemd, native apps**.
GNOME is required for website rules. Other desktop environments can route selected
native applications through the TUN service but must not claim website support.
Flatpak, Snap, AppImage and interpreted-script applications are rejected because
their launcher path does not reliably identify the actual network process.

## One package

Build the native desktop, agent, engine and router on the matching Linux host,
then run `python3 scripts/package-linux.py`. The generated `.deb` contains all
four binaries, the launcher/icon, and the root-owned systemd service. Install it
through the desktop's package installer or `sudo apt install ./Driftnet-*.deb`.
Installation requires the normal system administrator approval. Subsequent access
toggling runs as the desktop user with no manual proxy or terminal setup.

The router opens `/run/driftnet/network.sock`; SO_PEERCRED supplies the real caller
UID, and selected-app rules only match sockets belonging to that UID. The service
has a narrow capability set and may not accept arbitrary commands. The engine's
root-owned executable receives `cap_net_raw=ep` so its outbound sockets can bind
to the original physical interface and avoid recirculating through the TUN.
Uninstall stops the system service; user rules are preserved.

Website rules use GNOME's per-user PAC setting. Driftnet saves both prior settings
in a user-only recovery journal before writing either, holds a process lock,
restores on normal shutdown, and recovers after a crash at next startup. It
preserves proxy changes made by another application. Existing active desktop
proxies are never replaced. DNS settings are not modified. Browsers using their
own proxy configuration may ignore GNOME settings; live browser verification is
required before the release is advertised as supported.

## Backend integration

The shared integration now connects these components:

1. `apps/mod.rs`: cfg Linux module and dispatch `resolve` to `linux::resolve`;
   exclude Linux from the unsupported fallback. App IDs are `linux:/absolute/path`
   (a native ELF or `.desktop` entry). `desktop_entry` exposes name/icon metadata.
2. `platform/mod.rs`: export Linux `ProxyLease`; exclude Linux from fallback.
3. `router.rs`: add Linux Unix client socket `/run/driftnet/network.sock`, verify
   server `SO_PEERCRED.uid == 0`, then send the same root protocol. Website URL in
   the root request must be empty. Use the user's Linux ProxyLease separately.
4. `drift-agent`: keep Linux ProxyLease alive for website rules like Windows;
   build it before reporting On, drop it before ending the root-router lease.
   Do not enable GNOME PAC when selected website list is empty.
5. Desktop startup: invoke `ProxyLease::recover()` early, before connecting a new
   agent, as Windows does. A held lock means an agent is active and recovery is a
   no-op. Surface a stale journal recovery failure instead of silently routing
   browsers to a dead local PAC server.
6. UI picker: offer native executables and `.desktop` entries on Linux, obtain
   names/icons through `desktop_entry`, store the canonical resolved binary path.
   Linux tray support requires AppIndicator on the target desktop.
7. `build-router.py`: GOOS linux for Linux targets (not darwin); amd64/arm64 as
   appropriate. New Linux Go files are already copied by its glob logic. Keep
   Darwin upstream delegated-PID patch Darwin-only as it is today.
8. `build-sidecars.py`: select native Linux compiler, no macOS framework flags.
   `native/engine/parent_watch.c`: Linux `drift_bind_outbound` must map numeric
   interface index with `if_indextoname` and set `SO_BINDTODEVICE`; reject lookup
   or bind failures. The package supplies the required capability.
9. Release CI: build/test natively on Ubuntu 24.04 (plus ARM64 if distributing it),
   then call the package builder and upload the single `.deb`. AppImage does not
   install this required root service, and must not be advertised as equivalent.

## Verification status

Rust module tests cover desktop Exec parsing, unsafe launcher rejection, PAC
endpoint validation, restore after partial writes, and preservation of external
proxy changes. Go tests exercise actual kernel peer credentials and reject
non-Unix clients. Shell maintainer scripts and Python package syntax are checked.
The Go router can be cross-compiled for Linux from macOS. Native GNOME install,
TUN routing, browser PAC, voice, systemd and removal must be smoke-tested on a real
Linux host; they cannot be honestly verified on the macOS development machine.
`scripts/test-linux-installer.py` runs the disposable GitHub runner package install,
root-service startup, real selected/unselected HTTPS TUN flows, clean stop,
crash cleanup, repair and uninstall checks. It writes JSON evidence; it never
claims to test a GNOME browser or Discord voice.
