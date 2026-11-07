#!/usr/bin/env python3
"""Build the system-authorized removal package without running it."""
from pathlib import Path
import subprocess,tempfile,shutil
root=Path(__file__).resolve().parent.parent
out=root/'apps/desktop/resources/Drift-Uninstall.pkg';out.parent.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='drift-uninstall-') as temp:
 scripts=Path(temp);post=scripts/'postinstall'
 post.write_text((root/'native/macos/uninstall.sh').read_text());post.chmod(0o755)
 subprocess.run(['pkgbuild','--nopayload','--scripts',str(scripts),'--identifier','app.drift.desktop.uninstaller','--version','0.5.0',str(out)],check=True)
print(out)
