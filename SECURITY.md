# Security

Driftnet includes a privileged local routing helper. Security-relevant changes
need review at the boundary between the desktop client, helper, and OS settings.

## Report a vulnerability

Use the repository's private vulnerability-reporting feature when it is enabled,
or contact a repository maintainer privately. Do not put credentials, private
traffic, personal screenshots, or an unredacted network capture in an issue.
If no private reporting route is available, open an issue asking for a contact
without including the vulnerability details.

Useful reports identify the affected version and platform, the smallest safe
reproduction, expected behavior, and impact. Public endpoint examples are preferred
to private account data.

## Trust boundaries

- The desktop UI runs as the user. The installed helper authenticates its local
  control peer and limits requests to the routing protocol.
- Selected process ownership must match that authenticated user. Unknown ownership
  does not grant a broader routing rule.
- Local loopback data listeners are not a sandbox against code already running
  as the same user.
- System website proxy changes are journaled and restored only while their values
  still belong to the session. Existing conflicting proxies are preserved.
- The application does not install a TLS interception certificate. Diagnostic
  counters do not contain remote addresses or payloads.
- Update artifacts must have a valid trusted signature before installation.
  OS installer approval and OS publisher signatures are separate checks.

Private signing keys and GitHub tokens must remain outside the repository and
application bundle. A private release endpoint must fail safely when authentication
is unavailable; embedding an access token is not a supported workaround.

The [acceptance record](docs/ACCEPTANCE.md) states what has been verified. A build
or a policy unit test alone does not prove native traffic isolation.
