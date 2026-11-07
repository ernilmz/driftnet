# Networking architecture

Driftnet uses a shared Rust policy and interface, an unprivileged session agent,
a local TCP/UDP engine, and an installed native routing helper. macOS uses a
LaunchDaemon; Windows uses a service. Linux integration is a separate platform
adapter. No remote VPN server is involved.

## Application routing

The native helper creates a TUN interface using pinned sing-box/sing-tun code.
It attributes connections to OS process metadata and evaluates selected executable
paths or application-bundle prefixes. The control client is authenticated before
those rules are installed. Process ownership must match the authenticated desktop
user. Unknown or unmatched identities use the direct outbound.

On macOS, application bundles include executables under their canonical
`Contents/` path. The Darwin finder also reads the kernel-reported delegated
process id and verifies its effective uid before using its executable path. This
supports shared networking helpers without selecting a common helper basename
for every application. On Windows, the helper checks the socket-owning process
and its token SID. Custom executable paths are exact matches.

The optional Discord preset locates an installed app when available. Its absence
must not prevent website access. App attribution currently depends on the path
resolved for the session; changes to versioned install directories require special
care. Protected processes and incomplete OS metadata can remain direct. Rules
must never silently expand to all applications to hide an attribution failure.

The local tunnel receives public traffic for classification. Unselected traffic
still traverses that local stack before direct egress; there is no promise of
zero local processing overhead. LAN, loopback, link-local, and multicast routes
are excluded. Native routing does not require an application to accept proxy
flags and must continue to work when that app closes and reopens.

## Website routing

Selected websites use a PAC lease and a local HTTP CONNECT broker. PAC rules match
exact domains and optional subdomains at label boundaries. Unmatched hosts return
`DIRECT`. The broker checks the hostname again and rejects a request outside the
site policy with HTTP 403. A shared CDN address is not evidence of site ownership.

The optional Discord preset covers `discord.com`, `discordapp.com`,
`discordapp.net`, and `discord.gg`, including their subdomains. The same explicit
list is used by PAC generation and broker policy. Other websites may need
additional sign-in or media domains added by the user.

Selected public website names resolve through TLS-validated DoH endpoints with
pinned resolver addresses. A and AAAA responses are collected, cached with bounded
TTLs, and tried with staggered connection attempts. Resolver calls bypass inherited
proxy environment variables. System-wide DNS settings are unchanged. This website
resolver does not imply that every DNS request from every selected native app
uses DoH.

Local listeners use dynamic loopback ports. They are not an isolation boundary
against hostile code already running as the same user. No MITM certificate is
installed and TLS is not decrypted by Driftnet.

## Data engine

The C helper contains ByeDPI 17.3, pinned to
`e2684772fbda45f2ff4fc220922a010370534216`. Original vendored source is verified
before documented fixes are applied to a temporary build copy. The current TCP
profile uses `--split 2 --tlsrec 1+s`. UDP uses SOCKS5 association and datagram relay.
These capabilities do not prove compatibility with every ISP, site, voice service,
or QUIC connection.

Outbound engine sockets bind to the physical interface selected before the TUN
starts. This prevents a second local TCP stack from reassembling the engine's wire
fragmentation. A default-interface change ends the native lease rather than
leaving a stale-bound session displayed as healthy.

The short-send fix preserves a partially unsent buffer tail under backpressure.
Its regression test transfers a deterministic 4 MiB payload with a deliberately
slow receiver. Other socket tests cover UDP echo, an IPv6-only TCP destination,
listener cleanup, and abrupt owner exit.

## Ownership and cleanup

The desktop owns the agent through a control pipe. The agent owns the engine and
native routing lease. The C engine monitors its inherited stdin lifetime pipe;
EOF terminates it even when Rust destructors cannot run. The agent monitors its
owner during initial setup as well as after readiness.

The macOS helper authenticates Unix peers against the installed owner uid. Windows
uses an authenticated named pipe; the desktop verifies the server PID against the
registered service through the Service Control Manager. The helper accepts bounded
routing requests, not arbitrary commands or arbitrary sing-box configurations.

Website proxy settings are journaled before modification. macOS uses
SystemConfiguration; Windows uses the desktop user's Internet Settings registry
and WinINet refresh. Restore is conditional on the values still belonging to the
lease, preserving changes made by the user or another program. Existing conflicting
proxies are not overwritten. Startup recovery handles a stale website journal.

Normal stop closes the tunnel and restores website preferences. Helper loss is
reported to the desktop; On must reflect a live acknowledged session. The macOS
package includes a removal package and fixed-path cleanup script. The helper also
watches for a removed installation. Windows installer hooks stop and remove the
service during upgrade or uninstall. Actual uninstall acceptance remains separate
from the presence of this code.

## Source and trust

The separate native router incorporates sing-box 1.14.4 at commit
`c005f35300ce76427dc285481ec5695f5b179cd2` and is GPL-3.0-or-later. It is built from
verified source with explicit owner-filter and Darwin delegation changes.
Windows includes the official signed Wintun 0.14.1 binary under its supplied
binary license. App resources contain notices and provenance; a corresponding
router source archive is distributed alongside the installers.

Driftnet's independent Rust/UI code and the C engine retain their own MIT licenses.
See [router integration](../native/router/README.md) and
[third-party notices](../THIRD_PARTY_NOTICES.md).

## Verification boundaries

Policy tests, real loopback relay tests, live public endpoint probes, native
process attribution, installer behavior, and actual voice are distinct evidence.
A passing build or browser design preview does not establish platform parity.
The current record is in [ACCEPTANCE.md](ACCEPTANCE.md).
