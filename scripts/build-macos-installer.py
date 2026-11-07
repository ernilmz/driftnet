#!/usr/bin/env python3
"""Package the app + narrowly scoped networking helper. Never installs it."""
from pathlib import Path
import argparse,plistlib,shutil,subprocess,tempfile
root=Path(__file__).resolve().parent.parent
parser=argparse.ArgumentParser();parser.add_argument('--app',type=Path,default=root/'target/debug/bundle/macos/Driftnet.app');parser.add_argument('--output',type=Path,default=root/'dist/Driftnet-1.0.0-macOS.pkg');args=parser.parse_args()
app=args.app.resolve();out=args.output.resolve();out.parent.mkdir(parents=True,exist_ok=True)
if not app.is_dir():raise SystemExit('Build Driftnet.app first')
with tempfile.TemporaryDirectory(prefix='drift-package-') as temp:
    stage=Path(temp)/'root';scripts=Path(temp)/'scripts';scripts.mkdir();(stage/'Applications').mkdir(parents=True)
    shutil.copytree(app,stage/'Applications/Driftnet.app',symlinks=True)
    scripts.joinpath('preinstall').write_text('''#!/bin/sh
set -eu
if /usr/bin/pgrep -x drift-desktop >/dev/null 2>&1; then
  echo "Quit Driftnet before installing or updating it." >&2
  exit 1
fi
''')
    scripts.joinpath('postinstall').write_text('''#!/bin/sh
set -eu
APP="/Applications/Driftnet.app"
DIR="/Library/Application Support/Drift"
OWNER_UID="$(/usr/bin/stat -f %u /dev/console)"
if [ "$OWNER_UID" -eq 0 ] || [ "$OWNER_UID" -eq 4294967295 ]; then
  echo "Install Driftnet while a desktop user is logged in." >&2
  exit 1
fi
/bin/launchctl bootout system/app.drift.network >/dev/null 2>&1 || true
/usr/bin/install -d -m 700 -o root -g wheel "$DIR"
case "$OWNER_UID" in ''|*[!0-9]*) exit 1;; esac
/usr/bin/printf '{"uid":%s}\\n' "$OWNER_UID" > "$DIR/owner.new.json"
/bin/chmod 600 "$DIR/owner.new.json"
/bin/mv -f "$DIR/owner.new.json" "$DIR/owner.json"
/usr/bin/install -d -m 755 -o root -g wheel /Library/PrivilegedHelperTools
/usr/bin/install -m 755 -o root -g wheel "$APP/Contents/MacOS/drift-router" /Library/PrivilegedHelperTools/app.drift.network
/usr/bin/install -m 755 -o root -g wheel "$APP/Contents/MacOS/drift-proxyctl" /Library/PrivilegedHelperTools/app.drift.proxyctl
/usr/bin/install -m 700 -o root -g wheel "$APP/Contents/Resources/drift-uninstall.sh" /Library/PrivilegedHelperTools/app.drift.uninstall
/bin/chmod 644 /Library/LaunchDaemons/app.drift.network.plist
/usr/sbin/chown root:wheel /Library/LaunchDaemons/app.drift.network.plist
/bin/launchctl bootstrap system /Library/LaunchDaemons/app.drift.network.plist
/bin/launchctl enable system/app.drift.network
# Migrate the former product name, preserving rules and helper identities.
if [ -d /Applications/Drift.app ] && [ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' /Applications/Drift.app/Contents/Info.plist 2>/dev/null || true)" = app.drift.desktop ]; then
  /bin/rm -rf /Applications/Drift.app
fi
''')
    for path in scripts.iterdir():path.chmod(0o755)
    daemon={'Label':'app.drift.network','ProgramArguments':['/Library/PrivilegedHelperTools/app.drift.network','--serve'],'RunAtLoad':True,'KeepAlive':True,'ProcessType':'Background','ThrottleInterval':5,'StandardErrorPath':'/var/log/drift-network-helper.log'}
    path=stage/'Library/LaunchDaemons/app.drift.network.plist';path.parent.mkdir(parents=True);path.write_bytes(plistlib.dumps(daemon))
    subprocess.run(['pkgbuild','--root',str(stage),'--scripts',str(scripts),'--identifier','app.drift.desktop.installer','--version','1.0.0','--install-location','/',str(out)],check=True)
print(out)
