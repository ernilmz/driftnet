Drift native router — GPL-3.0-or-later

This separate helper incorporates SagerNet/sing-box 1.14.4, pinned to commit
c005f35300ce76427dc285481ec5695f5b179cd2. Original upstream sources and their
per-file hashes are in vendor/sing-box/DRIFT_UPSTREAM.json. They remain unchanged.
The build copies them into target/router-src and adds the following Drift code:

- A bounded, authenticated local session protocol and lifecycle lease.
- macOS Unix-peer uid checking and Windows named-pipe token SID checking.
- Process-owner filtering before any app-path match. Unknown ownership stays direct.
- Exact executable or application-bundle prefix matching, never basename matching.
- Native TUN startup/stop using upstream's TCP/IP stack and original signed Wintun.
- Windows service install/remove; macOS app-removal and preference recovery.
- Aggregate diagnostic counters, without remote addresses or traffic payload logs.

common/process/searcher.go:FindProcessInfo is patched to apply owner filtering
and increment the counters. The Darwin connection finder also reads the kernel’s
delegated process id (xsocket_n.so_e_pid), verifies that process uid and uses its
executable path, covering shared networking helpers without global helper matches. Original source is verified
before applying the patch. native/router/process contains those additions.

The Rust/C engine remains a separate process over SOCKS5, licensed separately.
Its outbound sockets bind to the pre-tunnel interface so the routing stack cannot
reassemble its DPI fragmentation. All unselected traffic uses the direct outbound.
The TUN leaves DNS settings unchanged and excludes LAN/loopback destinations.
No remote VPN endpoint or subscription is used.

The corresponding source archive beside the installers contains the exact patched
router and its dependency sources. Build recipe: scripts/build-router.py. The GPL
license is included in the application resources. Drift's independent Rust/UI code
retains its MIT license; this combined router executable is GPL-3.0-or-later.
