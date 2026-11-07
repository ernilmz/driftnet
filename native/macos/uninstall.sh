#!/bin/sh
# Installed root-owned, fixed paths only. User rules are intentionally retained.
set -eu
if [ "$(/usr/bin/id -u)" -ne 0 ]; then echo "Use the Driftnet uninstaller to approve removal." >&2; exit 1; fi
if [ -x /Library/PrivilegedHelperTools/app.drift.proxyctl ]; then
 /Library/PrivilegedHelperTools/app.drift.proxyctl recover
fi
/bin/launchctl disable system/app.drift.network >/dev/null 2>&1 || true
/bin/launchctl bootout system/app.drift.network >/dev/null 2>&1 || true
/bin/rm -f /Library/LaunchDaemons/app.drift.network.plist
/bin/rm -f /Library/PrivilegedHelperTools/app.drift.network /Library/PrivilegedHelperTools/app.drift.proxyctl
/bin/rm -f /var/run/app.drift.network.sock /var/run/app.drift.network.lock
/bin/rm -f '/Library/Application Support/Drift/owner.json' '/Library/Application Support/Drift/proxy-journal.json'
/bin/rmdir '/Library/Application Support/Drift' 2>/dev/null || true
/usr/sbin/pkgutil --forget app.drift.desktop.installer >/dev/null 2>&1 || true
for app in /Applications/Driftnet.app /Applications/Drift.app; do
 if [ -d "$app" ] && [ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app/Contents/Info.plist" 2>/dev/null || true)" = app.drift.desktop ]; then
  /bin/rm -rf "$app"
 fi
done
/bin/rm -f /Library/PrivilegedHelperTools/app.drift.uninstall
