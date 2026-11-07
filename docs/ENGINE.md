# Networking architecture

Driftnet 1.0.0 consists of a shared Rust policy and Tauri interface, an unprivileged
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
| Windows | Exact executable path, socket owner and token SID; authenticated named pipe; installed service and Wintun. |
| Linux | Canonical native ELF executable, optionally resolved from a `.desktop` file; local peer credentials; installed systemd helper. Flatpak, Snap, AppImage and script launchers are rejected rather than given an overbroad identity. |

The optional Discord preset looks up an installed application when present. Its
absence does not prevent website-only Access. Native routing does not depend on
proxy launch arguments; the selected executable can close and reopen while the
same lease stays active. A moved executable or changed versioned install path can
require reselecting the app. Protected processes and incomplete metadata may remain
direct, rather than broadening the selected rule.

Public traffic enters the local TUN for classification. Unselected traffic therefore
still crosses a local stack before direct egress; this is not a claim of zero local
processing overhead. LAN, loopback, link-local and multicast routes are excluded.

### IPv6 readiness and reopening

The helper only exposes an IPv6 tunnel address when the physical uplink has usable
IPv6. Advertising a local IPv6 address over an IPv4-only uplink can make Electron/Node
reconnects prefer an unusable route. The IPv4-only/dual-stack configuration regression
is covered by a Go test. One real macOS Discord quit/reopen cycle passed with Access
still on; repeated cycles and other platforms remain separate acceptance checks.

## Website routing

Selected websites use an OS PAC lease and a local HTTP CONNECT broker. Rules match
exact domains and optional subdomains at label boundaries. Unmatched hosts return
`DIRECT`. The broker checks the hostname again and rejects requests outside the
site policy. A shared CDN address does not establish site ownership.

The optional Discord preset covers `discord.com`, `discordapp.com`, `discordapp.net`
and `discord.gg`, including their subdomains. PAC and broker use the same list.
Other sites may need additional sign-in or media domains added explicitly.

Selected public website names resolve through TLS-validated DoH endpoints with
pinned resolver addresses. A/AAAA answers have bounded caches and staggered connection
attempts. Resolver requests bypass inherited proxy environment variables. System
DNS settings remain unchanged. This website resolver does not imply that every DNS
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
documented changes are applied to a temporary build copy. The current TCP profile
uses `--split 2 --tlsrec 1+s`; UDP uses SOCKS5 association and datagram relay.
These capabilities do not prove compatibility with every ISP, voice service or QUIC
connection. Real voice and video require their own end-to-end checks.

Outbound engine sockets bind to the physical interface selected before TUN startup,
preventing a second local stack from reassembling the intended wire fragmentation.
An uplink change ends the lease rather than leaving a stale-bound session shown as
healthy. Reconnect after changing networks.

The short-send correction preserves the partially unsent buffer tail under
backpressure. The regression transfers a deterministic 4 MiB payload through a
slow receiver. Socket tests also cover UDP echo, IPv6 TCP, listener cleanup and
abrupt owner exit; these are local transport checks, not ISP throughput results.

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

Policy tests, loopback relay tests, live endpoint probes, native ownership checks,
installation, real voice and visual behavior are separate evidence. Current results
and remaining device checks are recorded in [ACCEPTANCE.md](ACCEPTANCE.md).
