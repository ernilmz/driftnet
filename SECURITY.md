# Security

Driftnet includes a privileged local routing helper. Its trust boundaries are the
desktop user, helper control channel, process attribution, OS proxy lease, update
signing key and installer.

## Report a vulnerability

Use GitHub's private vulnerability reporting when enabled, or contact a repository
maintainer privately. Do not include credentials, private traffic, account screenshots
or an unredacted capture in a public issue. If no private reporting route is
available, ask for a contact without disclosing vulnerability details.

Useful reports identify the version, platform, smallest safe reproduction, expected
behavior and impact. Prefer public test endpoints over private account data.

## Trust boundaries

- The desktop UI runs as the user. The installed helper authenticates the local
  control client and accepts a bounded routing protocol, rather than arbitrary
  privileged commands or arbitrary router configuration.
- Selected process ownership must match the authenticated user. Unknown ownership
  does not grant a broader application rule. Canonical paths and exact domain
  boundaries are used; display names and shared CDN IPs are insufficient.
- Local loopback listeners are not a sandbox against hostile code already running
  as the same user. This application is not a defense against a compromised account.
- Website proxy settings are journaled and conditionally restored. Conflicting
  proxies and external changes are preserved. System DNS configuration is unchanged.
- Driftnet does not install a TLS interception certificate or decrypt TLS. Aggregate
  diagnostic counters do not contain remote addresses or traffic payloads.
- The TUN classifies public traffic locally. Unmatched traffic uses direct egress;
  it is not a promise that those packets avoid every local stack.

## Update authentication

The updater verifies a Minisign signature on the full manifest and a separate
signature on the complete platform installer. Signed versions must match. The
manifest covers platform selection and installer URLs; a changed URL, architecture,
version or installer invalidates verification. Files are checked again immediately
before the OS installer is opened. HTTPS endpoints, redirects, download sizes and
timeouts are restricted. Unix staging uses a private 0700 directory and 0600 file.

App and privileged helper are upgraded through the complete installer. The desktop
stops routing first and retains normal OS administrator consent. Update-payload
signatures do not replace Apple notarization, Windows publisher trust, package-manager
policy or administrator approval.

Private signing keys and GitHub access tokens must remain outside the repository
and bundle. A private release feed is expected to fail quietly without credentials;
embedding a GitHub token is not a supported workaround. Protect and back up the
signing key: installed clients cannot automatically trust an unrelated replacement.

## Scope of evidence

The current release target is 1.0.1. The [acceptance record](docs/ACCEPTANCE.md) distinguishes
local tests from native device and packaged-flow verification. A passing policy test
or build alone does not prove native isolation, crash cleanup, successful installation
or real voice/video on another platform or network.
