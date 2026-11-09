# Networking architecture

Driftnet 1.0.3 consists of a shared Rust policy and Tauri interface, an unprivileged
session agent, a local TCP/UDP engine, and an installed native routing helper.
macOS uses a LaunchDaemon, Windows a service, and Linux a systemd service. No remote
VPN endpoint is involved.

## Application routing

The helper creates a TUN interface using pinned sing-box/sing-tun code. It attributes
connections to OS process metadata and matches canonical executable paths or macOS
application-bundle prefixes. The client is authenticated before rules are installed;
selected process ownership must match that desktop user. Unknown or unmatched
identities use the direct outbound. Names and shared CDN IPs are not substitutes
for process ownership.

| Platform | Identity and installation |
| --- | --- |
| macOS | Canonical application bundle/executable; local peer uid; installed LaunchDaemon. The Darwin finder checks delegated process uid before using its executable path. |
| Windows | Normalized canonical executable path, socket owner and token SID; authenticated named pipe; installed service and Wintun. The optional Discord preset has a bounded installation family. |
| Linux | Canonical native ELF executable, optionally resolved from a `.desktop` file; local peer credentials; installed systemd helper. Flatpak, Snap, AppImage and script launchers are rejected rather than given an overbroad identity. |

The optional Discord preset looks up an installation when present. Its absence
does not prevent website-only Access. On Windows it selects only `Update.exe`
and `app-<numeric version>/Discord.exe` beneath one canonical Discord installation
root. The pattern also covers a new numeric version directory created while the
same Access lease remains active. It does not match every executable under that
root, a same-named app elsewhere, or a child junction that resolves outside it.
Custom application rules retain exact executable matching; a moved custom app
can require reselecting it. Authenticated socket-owner SID checks still apply to
both exact and Discord-family rules.

Windows 1.0.3 normalizes Rust's extended-length `\\?\` canonical paths and DOS/UNC
process paths to one spelling before matching. Reparse points are resolved before
process selection. The previous mismatch could leave an intended app on direct
egress even though the separate website PAC path worked. These changes address
routing identity; they do not establish that an ISP accepts the resulting TLS
traffic. Native 1.0.3 acceptance is pending.

Native routing does not depend on proxy launch arguments; a selected executable
can close and reopen while its lease stays active. Protected processes and
incomplete metadata may remain direct, rather than broadening the selected rule.

Public traffic enters the local TUN for classification. Unselected traffic therefore
still crosses a local stack before direct egress; this is not a claim of zero local
processing overhead. LAN, loopback, link-local and multicast routes are excluded.

### IPv6 readiness and reopening

The helper only exposes an IPv6 tunnel address when the physical uplink has usable
IPv6. Advertising a local IPv6 address over an IPv4-only uplink can make Electron/Node
reconnects prefer an unusable route. The IPv4-only/dual-stack configuration regression
is covered by a Go test. Three normal Discord quit/reopen cycles passed on installed
macOS 1.0.0 during one continuously active Access session. Other platforms and real
voice/video remain separate acceptance checks.

## Website routing

Selected websites use an OS PAC lease and a local HTTP CONNECT broker. Rules match
exact domains and optional subdomains at label boundaries. Unmatched hosts return
`DIRECT`. The broker checks the hostname again and rejects requests outside the
site policy. A shared CDN address does not establish site ownership.

The optional Discord preset covers `discord.com`, `discordapp.com`, `discordapp.net`
and `discord.gg`, including their subdomains. PAC and broker use the same list.
Other sites may need additional sign-in or media domains added explicitly.

Selected public website names resolve through TLS-validated DoH endpoints with
pinned resolver addresses. DoH connections pass through the same owned local
engine as selected destinations, so resolver TLS traffic also receives the TCP
fragmentation profile. Resolver bootstrap uses the pinned IPs without system DNS;
TLS still authenticates the resolver hostname. Redirects are refused. A/AAAA
answers have bounded caches and staggered connection attempts. The DoH client
explicitly uses rustls and its public CA roots for consistent TLS framing, even
when another workspace component enables native TLS. Resolver requests bypass
inherited proxy environment variables. System DNS settings remain unchanged.
This website resolver does not imply that every DNS
request made by a selected native app uses DoH.

| Platform | Website settings lease |
| --- | --- |
| macOS | SystemConfiguration automatic proxy settings. |
| Windows | Desktop user's Internet Settings registry plus WinINet refresh. |
| Linux | GNOME `org.gnome.system.proxy` settings in the user's desktop session. Other desktop environments are not silently treated as supported. |

Proxy settings are journaled before modification and restored only while they
still belong to the lease. External changes are preserved; conflicting proxies
are not overwritten. Startup recovery handles stale lease journals. Linux website
routing requires a GNOME/Ubuntu desktop session with its session bus; native app
routing has its own systemd/TUN path.

Listeners use dynamic loopback ports. They are not an isolation boundary against
hostile code running as the same user. No interception certificate is installed
and Driftnet does not decrypt TLS.

## Local data engine

The C helper incorporates ByeDPI 17.3, pinned to
`e2684772fbda45f2ff4fc220922a010370534216`. Original vendored source is verified before
documented changes are applied to a temporary build copy. The first TCP method
remains `--split 2 --tlsrec 1+s`. Initial TLS failures automatically try:

1. SNI splitting: `--split 1+s --tlsrec 3+s`.
2. On Windows, SNI disorder: `--split 1+s --disorder 3+s --tlsrec 3+s`.
3. TCP-only SNI splitting: `--split 1+s`, without TLS record splitting.

Fallback groups use `--auto=torst,ssl_err --proto tls`. The portable
`--tls-timeout 1.5:1:4096` bounds a silent initial TLS response; every group sets
`--cache-ttl 300` explicitly because group options do not inherit. The learned
method is cached by destination IP and port, not hostname. DoH allows eight
seconds to connect and ten seconds per request to accommodate these attempts.
Successful initial connections keep the original method. UDP continues to use
SOCKS5 association and datagram relay, with no new UDP fake-packet options.

The ClientHello replay guard is not limited to TLS 1.3: a complete single TLS 1.2
ClientHello record is eligible too. The pinned engine compares session IDs only
when ServerHello includes the TLS 1.3 `supported_versions` extension; a new TLS 1.2
session ID alone is not an error. Disabling TLS 1.2 is not part of this follow-up.
A local public Discord updater-manifest request returned HTTP 200 with both normal
TLS negotiation and forced TLS 1.2. This is endpoint evidence on that local
connection, not a Windows process-attribution or Superonline/Vodafone result.

The native integration refuses reconnecting buffered HTTP requests or TLS
early/application data. Only a complete single ClientHello record is replayable;
partial handshakes, coalesced records and additional buffered data fail closed.
This guard is in the temporary source patch and original wrapper, not a change
to the checksum-pinned vendor files.

The [pinned ByeDPI parser](https://github.com/hufrea/byedpi/blob/e2684772fbda45f2ff4fc220922a010370534216/main.c)
and [retry/cache implementation](https://github.com/hufrea/byedpi/blob/e2684772fbda45f2ff4fc220922a010370534216/extend.c)
define the actual options. Superonline-oriented methods also appear in
[ByeDPI Android presets](https://github.com/barkeser2002/BarisKeser-ByeDPI/blob/master/app/src/main/res/values/arrays.xml)
and [GoodbyeDPI-Turkey Windows alternatives](https://github.com/cagritaskn/GoodbyeDPI-Turkey/tree/master/windows).
GoodbyeDPI/WinDivert flags cannot be copied into this engine unchanged. The
Windows disorder method temporarily changes the outbound socket's TTL internally;
it does not change the machine's global TTL, DNS settings or routing scope.
There is no physical Superonline line in the current validation environment.
These capabilities do not prove compatibility with every ISP, voice service or QUIC
connection. Real voice and video require their own end-to-end checks.

Outbound engine sockets bind to the physical interface selected before TUN startup,
preventing a second local stack from reassembling the intended wire fragmentation.
An uplink change ends the lease rather than leaving a stale-bound session shown as
healthy. Reconnect after changing networks.

The short-send correction preserves the partially unsent buffer tail under
backpressure. The regression transfers a deterministic 4 MiB payload through a
slow receiver. Socket tests also cover UDP echo, IPv6 TCP, listener cleanup and
abrupt owner exit. Additional real-engine regressions force the complete TLS
fallback chain and its warm cache, a silent handshake timeout, and absence of
HTTP POST/early-data replay. These are local transport checks, not ISP throughput results.

## Ownership, cleanup and installation

The desktop owns the agent through a control pipe. The agent owns the engine and
native lease. The engine monitors inherited stdin; EOF ends it even when Rust
destructors cannot run. The agent monitors its owner during setup and after readiness.

macOS authenticates local peers against the installed owner uid. Windows verifies
the named-pipe server PID against the registered service and authenticates the
client's token. Linux uses Unix peer credentials and a bounded local service
protocol. The helper accepts routing requests, not arbitrary commands or unbounded
sing-box configurations.

Normal stop removes the tunnel and restores website preferences. Helper loss is
reported to the desktop; On represents a live acknowledged session. The macOS
package includes fixed-path removal and recovery. Windows installer hooks stop
and remove the service on upgrade/uninstall. Linux package scripts manage the
systemd unit. Code for cleanup and an observed device cleanup test are distinct.

The display name is Driftnet. `app.drift.desktop`, internal helper filenames and
migration identifiers are retained intentionally, so renaming does not create a
second rule store or an incompatible helper identity.

## Signed full-installer updates

Checks start approximately 20 seconds after app launch, repeat every six hours,
and can be requested from the tray menu. The stable feed is:

`https://github.com/ernilmz/driftnet/releases/latest/download/latest.json`

The updater also fetches `latest.json.sig`. It verifies the complete manifest,
including architecture keys and URLs, with the embedded Minisign public key. It
then downloads the matching platform's full installer and verifies its independent
signature. Both signatures must authenticate the same semantic app version. Old
signed bytes cannot be relabeled as a newer version. Local files are verified
again immediately before opening the installer.

Only HTTPS GitHub release assets and the expected installer suffix are accepted;
redirects have a small GitHub-host allowlist and a maximum count. Manifest and
installer downloads are bounded in size and time. Unix staging directories use
0700 and installer files 0600. No GitHub token or private signing key is bundled.

An update-ready icon opens the normal OS installer after stopping the current
routing session. macOS uses the combined `.pkg`, Windows the full Setup executable,
and Linux `pkexec dpkg --install` for the native `.deb`. OS administrator consent
is retained. App-only replacement is deliberately avoided because it could leave
the privileged helper at an incompatible version. Failed installer launch restores
the prior active session where possible.

The actual 1.0.1 manifest and all three installer signatures passed the production
verifier, including corrupted-byte and signed-version mismatch rejection. Signed
release checksums also authenticate the corresponding source and supporting assets.
This establishes artifact authenticity, not an observed user installation.

An inaccessible private release feed produces a quiet automatic failure. It cannot
be made anonymously accessible by packaging a token. Private repository members
can install manually; automatic anonymous delivery requires a public feed.

## Source and verification

The separate router incorporates sing-box 1.14.4 at
`c005f35300ce76427dc285481ec5695f5b179cd2`, GPL-3.0-or-later, with documented owner
filtering, Darwin delegation and lifecycle additions. Windows includes the official
signed Wintun 0.14.1 binary under its supplied binary license. Corresponding router
source accompanies installers. Independent Rust/UI and C-engine components retain
their own MIT licenses. See [router integration](../native/router/README.md) and
[third-party notices](../THIRD_PARTY_NOTICES.md).

The 1.0.1 corresponding-source archive contains the exact staged module and
vendored dependency set. Its 4,420 member headers are normalized to root ids,
empty account names and fixed timestamps; gzip omits its source filename/time.
It built offline with `GOPROXY=off`, `GOSUMDB=off`, `-mod=vendor` for Darwin arm64,
Windows amd64, Linux amd64 and Linux arm64. This checks source completeness,
rather than native desktop installation or traffic behavior on those systems.

### Dependency security in 1.0.1

The generated router stage pins `golang.org/x/net` to v0.60.0, retaining the
verified upstream source while applying the dependency update in the build copy.
Go 1.27.3 source and unstripped-binary vulnerability scans passed for macOS,
Windows and Linux with zero reachable advisories and zero imported-package
advisories. Six findings remain in unimported upstream packages and are listed in
[router provenance](../native/router/README.md). Stripped release binaries do not
retain enough symbols for the same precise binary scan; the identical unstripped
audit build provides that evidence. No advisory is globally suppressed.

Linux's GTK3 dependency uses the exact reviewed upstream `VariantStrIter` safety
backport in GLib `0.18.5+driftnet.1`. The source and every retained crate file are
checksum-pinned; optimized upstream iterator tests passed in native Ubuntu CI.
See [backport provenance](../native/patches/README.md). Remaining unmaintained Rust
dependency warnings and the limits of scanner results are recorded in the
[security review](SECURITY-REVIEW.md).

Policy tests, loopback relay tests, live endpoint probes, native ownership checks,
installation, real voice and visual behavior are separate evidence. Current results
and remaining device checks are recorded in [ACCEPTANCE.md](ACCEPTANCE.md).
