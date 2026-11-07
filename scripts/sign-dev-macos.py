#!/usr/bin/env python3
from pathlib import Path
import subprocess,argparse
root=Path(__file__).resolve().parent.parent
p=argparse.ArgumentParser();p.add_argument('--app',type=Path,default=root/'target/debug/bundle/macos/Driftnet.app');args=p.parse_args();app=args.app.resolve()
for name,identifier in [('drift-router','app.drift.desktop.router'),('drift-engine','app.drift.desktop.engine'),('drift-agent','app.drift.desktop.agent'),('drift-proxyctl','app.drift.desktop.proxyctl')]:
    subprocess.run(['codesign','--force','--sign','-','--timestamp=none','--identifier',identifier,str(app/'Contents/MacOS'/name)],check=True)
subprocess.run(['codesign','--force','--sign','-','--timestamp=none',str(app)],check=True)
subprocess.run(['codesign','--verify','--deep','--strict',str(app)],check=True)
