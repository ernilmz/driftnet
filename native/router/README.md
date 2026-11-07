Driftnet native router — GPL-3.0-or-later

This separate helper incorporates SagerNet/sing-box 1.14.4, pinned to commit
c005f35300ce76427dc285481ec5695f5b179cd2. Original upstream sources and their
per-file hashes are in vendor/sing-box/DRIFT_UPSTREAM.json. They remain unchanged.
The build copies them into target/router-src and adds the following Driftnet code:

- A bounded, authenticated local session protocol and lifecycle lease.
- macOS/Linux Unix-peer uid checking and Windows named-pipe token SID checking.
- Process-owner filtering before any app-path match. Unknown ownership stays direct.
- Exact executable or application-bundle prefix matching, never basename matching.
- Native TUN startup/stop using upstream's TCP/IP stack and original signed Wintun.
- Windows service install/remove; Linux systemd integration; macOS app-removal
  and preference recovery.
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
license is included in the application resources. Driftnet's independent Rust/UI code
retains its MIT license; this combined router executable is GPL-3.0-or-later.

IPv6 TUN addresses are enabled only when the physical uplink has a global IPv6
address. Windows authenticates pipe clients only after its bounded initial read,
as required by ImpersonateNamedPipeClient.

Dependency security override (2026-11-07)

The generated stage upgrades golang.org/x/net from upstream's v0.57.0 to v0.60.0,
the minimum fixed release for GO-2026-6603, GO-2026-6610, GO-2026-6611,
GO-2026-6612 and GO-2026-6617. These HTTP/2 findings were reported at symbol
level by the official govulncheck v1.9.0 tool. The helper does not expose an
HTTP/2 server or accept arbitrary remote rule-set configuration; several advisory
conditions therefore do not apply to its fixed runtime configuration. Updating
the linked library also removes the client-side framing/CPU-exhaustion findings.
Official reports: https://pkg.go.dev/vuln/GO-2026-6610 and
https://pkg.go.dev/vuln/GO-2026-6611. Upstream vendor files remain untouched.

Go's minimum-version selection also advances required x/crypto, x/mod, x/sync,
x/sys, x/term, x/text and x/tools dependencies and the staged module's minimum
Go version to 1.26. Build and audit use Go 1.27.3. Exact selected versions and
checksums are in the generated go.mod/go.sum and corresponding-source archive.
The pinned github.com/Microsoft/go-winio v0.6.2 requirement is retained for all
build hosts so that the source archive also builds the Windows control pipe.

Post-override source scans with with_gvisor for darwin/arm64, windows/amd64 and
linux/amd64 report zero reachable advisories and zero advisories in imported
packages (Go vulnerability database updated 2026-11-05). Six advisories remain
in unimported upstream module packages: x/net/html (GO-2026-6866, GO-2026-6860),
x/crypto/openpgp (GO-2026-5932), and Chi RealIP middleware (GO-2026-5777,
GO-2026-5775, GO-2026-5774). They are not silently described as fixed. No known
Go database entries were listed for the Sing-box, sing-tun or gVisor fork module
paths at audit time. In particular, the modified main module has no release
version for automatic matching; the module-path database lookup was checked
separately. Static analysis is not proof against unknown issues, reflection,
unsafe/native code, or advisories missing from the database.

Identical unstripped audit builds also pass govulncheck's binary-symbol scan on
all three targets. Release binaries use -s -w; govulncheck v1.9.0 cannot extract
their stripped symbol table and falls back to all module-level advisories. That
fallback must not be presented as a precise reachable-symbol result. Source
analysis and the unstripped audit builds retain the distinction above; no
advisory is suppressed. Tool documentation:
https://pkg.go.dev/golang.org/x/vuln/cmd/govulncheck.
