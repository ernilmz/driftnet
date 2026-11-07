# Security review — Driftnet 1.0.0

Review date: 7 November 2026. This is a code and artifact review, not a security
certification or a claim that a compromised desktop account can be contained.
The first history scan covered six reachable commits through `83bbab8` and 1,760
historical file blobs. Subsequent working-tree hardening requires rebuilt packages;
a previously signed package does not acquire later source fixes automatically.

## Public key and private key

`apps/desktop/resources/updater-public-key.txt` is deliberately public. It verifies
signatures; it cannot create a valid release signature. Publishing this file, the
public signatures or their key identifier does not disclose the private signing
key. This is the normal [Minisign public-key model](https://jedisct1.github.io/minisign/),
also documented by [Tauri's update signing guidance](https://v2.tauri.app/plugin/updater/).

The repository, local public `.pub` and installed application contain the same
public key. Only file existence/location/permissions of the actual private key
were inspected: it is outside the checkout, with Unix file mode 0600 and directory
mode 0700. Its contents were not opened or printed. These permissions protect
against other unprivileged users; they do not protect a developer's key against
malware already running as that developer or against root. Protect its backups
and signing host. Never upload the private key or embed a GitHub access token.

## Findings and disposition

| ID | Severity | Finding | Disposition |
| --- | --- | --- | --- |
| INST-MAC-01 | High, conditional local privilege escalation | A pre-existing user-owned helper state directory could contain a symlink followed by an elevated installer write; helper binaries were copied from an admin-writable application bundle. | Fixed in source: validate root ownership, write permissions and fixed-path symlinks before installation; exclusive temporary owner record and atomic replacement; helper binaries are direct protected payload entries. Guard fixtures, script syntax, package build and strict staged signature checks passed. Final installed package acceptance remains separate. |
| INST-WIN-01 | High, conditional local privilege escalation | Elevated hooks / LocalSystem service registration could execute a helper from an arbitrarily writable custom install directory. | Fixed in source: only protected Program Files product roots, owner/DACL/reparse-point validation in installer and service registration, SCM operations through trusted OS APIs, and a negative unsafe-location regression. Native installer acceptance is required before release. |
| IPC-01 | Low, local availability | An authenticated slow reader could keep a reply/lease operation blocked without a write bound. | Fixed: bounded control reply writes and teardown when readiness cannot be delivered. A real blocked socket regression passed. A malicious local user can still occupy the one global routing lease. |
| UPD-01 | Low, defense in depth | Initial release URL checking accepted an extra path/query within the trusted GitHub repository; staging used ordinary file creation. Signatures still prevented a demonstrated remote substitution. | Hardened: exact signed `vVERSION` path, one ASCII filename, no query/fragment/credentials/nonstandard port, fixed local filename, bounded semantic version, exclusive 0600 creation and symlink rejection. |
| LOCAL-01 | Medium, conditional local attack | The verified installer remains owned by the desktop user until the OS installer reads it. Code already running as that same uid can modify user-owned staging after verification; normal OS admin consent is still required. | Explicit residual risk. 0700/0600 and immediate re-verification are not kernel-enforced root-side authentication. No new general-purpose privileged install IPC was added. |
| PRIV-01 | Low, privacy | Initial macOS release agent/desktop binaries contained build-machine home/source paths. Package metadata included removable attributes and OS-managed `com.apple.provenance`. | Release path remapping and stage metadata cleanup are applied during final rebuilding; rebuilt artifacts must be scanned again. macOS can reattach a generic, non-identifying provenance attribute even to new files, so no claim of zero extended attributes is made. No credential or private-key pattern was found in the inspected payload. |
| PRIV-02 | Low, privacy | Initial corresponding-source tar headers exposed the builder account name/uid/gid and timestamps. | Fixed in the source packager: normalize uid/gid to 0, clear account names/PAX metadata, use `SOURCE_DATE_EPOCH` (default 0), omit gzip filename and set gzip timestamp to 0. Final archive regeneration/reinspection is required. |
| DEP-01 | Informational unsoundness, potentially a crash if reached | RustSec flags `glib` 0.18.5 `VariantStrIter` output-pointer mutability, RUSTSEC-2024-0429. Tauri's Linux GTK3 chain uses this version. | Exact two-line upstream fix backported as `0.18.5+driftnet.1` without changing the GTK3 API family. Official crate and all retained files are checksum-pinned; the backport guard passes. Optimized upstream iterator tests and native desktop compilation are required in Ubuntu CI before final acceptance. |
| REL-01 | Low, release integrity usability | The initial `dist/SHA256SUMS.txt` named older development artifacts. | Do not publish it unchanged. Regenerate checksums after the final packages, source archive and signatures are complete. Checksums alone are not authenticity signatures. |

## Repository, history and package privacy

Observed checks, without printing matching values:

- GitHub metadata reported `ernilmz/driftnet` **PRIVATE**; the remote URL contained
  no credentials and only `main` was advertised at the time of the scan.
- All 1,760 reachable historical blobs were checked for private-key blocks, GitHub,
  AWS and Discord token patterns, credential-bearing URLs and personal home paths.
  Additional bounded Base64 decoding checked for wrapped private-key envelopes;
  Slack/OpenAI token patterns were also checked.
- No production credential/private-key match was identified. Credential-bearing
  URLs were synthetic policy/upstream test data. The two source home-path matches
  were in pinned upstream benchmark/tests, rather than private user files.
- The three Base64 signature matches were public updater fixtures. Their comment
  says that a signature came from a secret key; the files contain signatures, not
  the secret key. Fixture version `0.0.0-fixture` is below shipping versions.
- One unreachable local blob was separately scanned without a secret pattern hit.
  Git authors used a GitHub noreply address. No tracked `.p12`, `.pfx`, private
  `.key`, account capture, packet capture or log artifact was identified.
- The inspected macOS package expanded to 24 files with expected app/helper/license
  resources, not user profiles, browser databases or a private signing key.
  Build paths and provenance metadata are recorded above as privacy findings.
- The initial corresponding-source archive contained 4,411 members, no absolute
  or parent-traversing member/link paths, and no actual builder-home, token or
  private-key content matches. Owner metadata still needed normalization.

Pattern scanning cannot establish that every possible secret is absent. It does
not inspect GitHub's hidden infrastructure, external developer backups, previously
shared screenshots/chats or a compromised account. No private credential values
were uploaded as part of these checks. Final Windows/Linux payload inspection and
post-rebuild checks must be recorded separately.

## Update attack surface

The entire manifest is authenticated before selecting a platform installer. The
installer has an independent signature, and both authenticated trusted comments
must name the expected version. The updater accepts only a version newer than the
running application. A replay of an old signed installer cannot be relabeled as a
new version, and changing a platform key/URL invalidates the manifest signature.

Downloads use HTTPS, a small GitHub redirect-host allowlist, redirect-count limits,
10-second connection and 180-second request timeouts, a 512 KiB manifest limit and
256 MiB installer limit. An asset must come from the configured repository's exact
release tag and use the platform's `.pkg`, `.exe` or `.deb` suffix. The URL is never
passed to a shell. Untrusted metadata is not used as a local path component.

Staging uses a random temporary directory; Unix permissions are 0700/0600. File
creation is exclusive, so an existing file or symlink cannot be overwritten. The
signature is checked again before launching the complete installer. The app stops
routing first and preserves normal OS administrator consent. macOS opens its full
package, Windows invokes the full Setup executable, and Linux uses fixed
`pkexec`/`dpkg` executable paths and argument arrays. The existing root network
helper is not an arbitrary update-command endpoint.

The remaining same-user race is material: an unprivileged application cannot make
its own user-owned file immutable against other code running as that same user.
A stronger future design needs a separately reviewed, fixed-purpose privileged
staging/verifier or an enforced expected OS publisher identity. It must not become
an arbitrary file-copy or command-execution interface. Root compromise and a
stolen signing key remain outside the current updater's protection.

Initial installation is a separate trust event. The packages currently lack
production Apple notarization/Windows Authenticode credentials. A malicious fork
can change its own public key and source, but it cannot produce updates accepted
by an existing official installation without the official signing key. Users must
still obtain their initial installer from a trusted source; a copied name/icon is
not cryptographic identity. The anonymous updater cannot read private releases and
quietly leaves Access unchanged; no token-based workaround is bundled.

## Tests and dependency checks

- Eleven integrated updater tests passed on macOS after hardening: valid artifact
  and metadata, byte tampering, version relabeling, trusted-comment forgery,
  architecture/URL changes, host/path policy, malicious version strings, existing
  file/symlink rejection and Unix creation permissions.
- Desktop all-target clippy with warnings denied passed after the updater changes.
- An isolated harness using the exact production `update_verify.rs` verified the
  actual initial 1.0.0 manifest and complete macOS/Windows installer signatures.
  Corrupted copies and version `1.0.1` relabeling were rejected. Original artifacts
  were unchanged; private-key contents were not needed. Rebuilt artifacts must be
  signed and checked again.
- Archive-normalization checks cover dependency headers, `.git` exclusion, zeroed
  gzip filename/time metadata and round-trip tar headers.
- The GLib dependency was extracted from the official crates.io archive only after
  checking its published Cargo.lock checksum. Its MIT license and 120 retained
  original files are preserved; registry-only `.cargo_vcs_info.json` was omitted.
  `scripts/verify-glib-patch.py` checks every file and reverses the exact two-line
  fix to reproduce the upstream source hash. See [backport provenance](../native/patches/README.md).
- `npm audit` including development dependencies and production-only each reported
  zero advisories for the locked UI dependency set at review time.
- `cargo-audit` 0.22.2 checked 512 locked dependencies against RustSec database
  commit `309392b18f6b17022f82c701f300433126901068` (1,422 advisories, updated
  6 November 2026). Its vulnerability list was empty, but it also reported the
  `glib` unsoundness above and four unmaintained dependencies: `proc-macro-error`,
  `zbus_names`, `zvariant` and `zvariant_derive`. After the backport, the path-based
  GLib package no longer appears in the scanner's warning list; this scanner
  behavior is not the evidence of remediation. The exact patch/provenance check
  and optimized native tests establish the mitigation. Four unmaintained warnings
  remain recorded rather than silently ignored. No advisory ignore list was added.
  Advisory scans are not a source-code proof of safety.

## Reuse, branding and licensing boundary

Publishing source enables inspection and forks; it cannot prevent someone from
modifying a fork for harmful purposes. This review did not find an update route
that lets an unauthenticated remote site choose and execute an unsigned privileged
payload. That finding is narrower than a guarantee against all malicious reuse.

The inspected package/repository uses Driftnet as the product identity. Stable
`app.drift.desktop` and internal Drift helper names are intentional migration
identifiers. No legacy Discord Local executable was found among tracked files or
the inspected macOS payload. A legacy name in migration code is not evidence that
a separate commercial product's code or rights have been acquired.

The independent project code, C engine and separate GPL router have different
license obligations; redistribution must preserve their notices and corresponding
source requirements. See [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) and
[LICENSE](../LICENSE). This technical review is not trademark clearance or a legal
opinion that every third-party license obligation has been independently certified.

## Local routing and operating-system privileges

The reviewed custom protocol uses kernel-authenticated local users, bounded JSON
and app counts, exact escaped paths and per-owner process filtering. Unknown
ownership remains direct. It does not accept an arbitrary executable, shell
command or upstream router JSON to execute with privileges. Control sockets and
traffic listeners are local; the latter are not an authentication boundary against
other processes on the same host.

The macOS installer registers a desktop uid. That account can request system PAC
changes pointing at its own loopback server, so a compromised application under
that account can misuse the delegated routing capability. Linux accepts local
non-root users and Windows interactive users; one global routing session means a
local account can occupy it and deny another user access. This utility does not
provide isolation from a hostile local account.

The root helper intentionally has networking and process-inspection privileges.
Linux's engine additionally receives CAP_NET_RAW for physical-interface binding.
Native packet parsers and the upstream TUN stack remain security-sensitive. The
review covered custom glue and boundaries; it did not fuzz or formally verify all
upstream native code and does not establish that no exploitable defect exists.

There is no separate analytics or account backend. Expected outbound requests
include the configured GitHub update feed, DNS-over-HTTPS resolvers for selected
website names, favicons fetched from entered websites, and the destinations of
routed traffic. Their servers can observe ordinary connection metadata. Driftnet
does not change the public IP address or promise anonymity. GitHub account/noreply
author attribution remains associated with the project, even though no private
key or production credential was found.

## Native presentation verification

A separate unprivileged macOS preview tested initial display, Escape dismissal,
reopening with retained Settings state, and native About version/copyright. The
installed Access session and its agent were left running during this test.
The surface uses a short opacity/position/scale transition, reverses a queued
hide on reopening, and respects the reduced-motion media preference. This does
not establish Windows/Linux compositor behavior or a measured animation frame rate.
