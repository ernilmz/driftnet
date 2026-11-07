#!/usr/bin/env python3
"""Package the app and fixed networking helpers. Never installs the package."""
from pathlib import Path
import argparse
import plistlib
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('--app', type=Path, default=root/'target/debug/bundle/macos/Driftnet.app')
parser.add_argument('--output', type=Path, default=root/'dist/Driftnet-1.0.0-macOS.pkg')
args = parser.parse_args()
app = args.app.resolve()
out = args.output.resolve()
out.parent.mkdir(parents=True, exist_ok=True)
if not app.is_dir():
    raise SystemExit('Build Driftnet.app first')

# Existing privileged directories must already be trusted BEFORE changing their
# mode/owner. Promoting a user-created directory can retain hostile symlinks.
path_checks = '''fail() { echo "Unsafe existing Driftnet helper path: $1" >&2; exit 1; }
safe_acl() {
  acl="$(/bin/ls -lde "$1")" || fail "$1"
  # A previous chmod/chown does not remove extended ACL grants. These fixed
  # privileged paths need no allow ACLs; normal macOS deny-delete entries stay.
  if /usr/bin/printf '%s\\n' "$acl" | /usr/bin/grep -Eq '^[[:space:]]*[0-9]+: .* allow '; then
    fail "$1 (extended ACL grant)"
  fi
}
safe_dir() {
  [ ! -L "$1" ] || fail "$1"
  if [ -e "$1" ]; then
    [ -d "$1" ] || fail "$1"
    [ "$(/usr/bin/stat -f %u "$1")" -eq 0 ] || fail "$1"
    mode="$(/usr/bin/stat -f %Lp "$1")"
    [ $((0$mode & 0022)) -eq 0 ] || fail "$1"
    safe_acl "$1"
  fi
}
safe_file() {
  [ ! -L "$1" ] || fail "$1"
  if [ -e "$1" ]; then
    [ -f "$1" ] || fail "$1"
    [ "$(/usr/bin/stat -f %u "$1")" -eq 0 ] || fail "$1"
    [ "$(/usr/bin/stat -f %l "$1")" -eq 1 ] || fail "$1"
    mode="$(/usr/bin/stat -f %Lp "$1")"
    [ $((0$mode & 0022)) -eq 0 ] || fail "$1"
    safe_acl "$1"
  fi
}
safe_dir /Library
safe_dir "/Library/Application Support"
safe_dir "/Library/Application Support/Drift"
safe_dir /Library/PrivilegedHelperTools
safe_dir /Library/LaunchDaemons
safe_dir /var/log
for file in "/Library/Application Support/Drift/owner.json" "/Library/Application Support/Drift/proxy-journal.json" /Library/PrivilegedHelperTools/app.drift.network /Library/PrivilegedHelperTools/app.drift.proxyctl /Library/PrivilegedHelperTools/app.drift.uninstall /Library/LaunchDaemons/app.drift.network.plist /var/log/drift-network-helper.log; do
  safe_file "$file"
done
'''

with tempfile.TemporaryDirectory(prefix='drift-package-') as temp:
    stage = Path(temp)/'root'
    scripts = Path(temp)/'scripts'
    scripts.mkdir()
    (stage/'Applications').mkdir(parents=True)
    staged_app = stage/'Applications/Driftnet.app'
    shutil.copytree(app, staged_app, symlinks=True)
    # The payload installs these directly into a root-only parent. Never copy
    # privileged executable code back out of /Applications during postinstall.
    helpers = stage/'Library/PrivilegedHelperTools'
    helpers.mkdir(parents=True)
    helpers.chmod(0o755)
    for source, name, mode in [
        (app/'Contents/MacOS/drift-router', 'app.drift.network', 0o755),
        (app/'Contents/MacOS/drift-proxyctl', 'app.drift.proxyctl', 0o755),
        (app/'Contents/Resources/drift-uninstall.sh', 'app.drift.uninstall', 0o700),
    ]:
        shutil.copyfile(source, helpers/name)
        (helpers/name).chmod(mode)
    scripts.joinpath('preinstall').write_text('''#!/bin/sh
set -eu
''' + path_checks + '''if /usr/bin/pgrep -x drift-desktop >/dev/null 2>&1; then
  echo "Quit Driftnet before installing or updating it." >&2
  exit 1
fi
''')
    scripts.joinpath('postinstall').write_text('''#!/bin/sh
set -eu
''' + path_checks + '''DIR="/Library/Application Support/Drift"
OWNER_UID="$(/usr/bin/stat -f %u /dev/console)"
case "$OWNER_UID" in ''|*[!0-9]*) exit 1;; esac
if [ "$OWNER_UID" -eq 0 ] || [ "$OWNER_UID" -eq 4294967295 ]; then
  echo "Install Driftnet while a desktop user is logged in." >&2
  exit 1
fi
/bin/launchctl bootout system/app.drift.network >/dev/null 2>&1 || true
/usr/bin/install -d -m 700 -o root -g wheel "$DIR"
OWNER_TEMP="$(/usr/bin/mktemp "$DIR/.owner.XXXXXXXX")"
trap '/bin/rm -f "$OWNER_TEMP"' EXIT HUP INT TERM
/usr/bin/printf '{"uid":%s}\\n' "$OWNER_UID" > "$OWNER_TEMP"
/bin/chmod 600 "$OWNER_TEMP"
/bin/mv -f "$OWNER_TEMP" "$DIR/owner.json"
for helper in /Library/PrivilegedHelperTools/app.drift.network /Library/PrivilegedHelperTools/app.drift.proxyctl; do
  /bin/chmod 755 "$helper"
  /usr/sbin/chown root:wheel "$helper"
done
/bin/chmod 700 /Library/PrivilegedHelperTools/app.drift.uninstall
/usr/sbin/chown root:wheel /Library/PrivilegedHelperTools/app.drift.uninstall
/bin/chmod 644 /Library/LaunchDaemons/app.drift.network.plist
/usr/sbin/chown root:wheel /Library/LaunchDaemons/app.drift.network.plist
/bin/launchctl bootstrap system /Library/LaunchDaemons/app.drift.network.plist
/bin/launchctl enable system/app.drift.network
# Migrate the former product name, preserving rules and helper identities.
if [ -d /Applications/Drift.app ] && [ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' /Applications/Drift.app/Contents/Info.plist 2>/dev/null || true)" = app.drift.desktop ]; then
  /bin/rm -rf /Applications/Drift.app
fi
''')
    for path in scripts.iterdir():
        path.chmod(0o755)
    daemon = {'Label':'app.drift.network', 'ProgramArguments':['/Library/PrivilegedHelperTools/app.drift.network','--serve'], 'RunAtLoad':True, 'KeepAlive':True, 'ProcessType':'Background', 'ThrottleInterval':5, 'StandardErrorPath':'/var/log/drift-network-helper.log'}
    path = stage/'Library/LaunchDaemons/app.drift.network.plist'
    path.parent.mkdir(parents=True)
    path.parent.chmod(0o755)
    path.write_bytes(plistlib.dumps(daemon))
    path.chmod(0o644)
    # Do not distribute developer provenance/quarantine/resource-fork metadata.
    # Signature blobs live in Mach-O/_CodeSignature and remain verified below.
    subprocess.run(['/usr/bin/xattr', '-cr', str(stage)], check=True)
    subprocess.run(['/usr/bin/codesign', '--verify', '--deep', '--strict', str(staged_app)], check=True)
    subprocess.run(['pkgbuild', '--root', str(stage), '--ownership', 'recommended', '--scripts', str(scripts), '--identifier', 'app.drift.desktop.installer', '--version', '1.0.0', '--install-location', '/', str(out)], check=True)
print(out)
