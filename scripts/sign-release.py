#!/usr/bin/env python3
"""Use the owner's OS certificate store; never fetch/export private keys."""
import argparse,os,shutil,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser()
p.add_argument('artifact',type=Path)
p.add_argument('--app-identity',help='Developer ID Application identity in the Mac keychain')
p.add_argument('--installer-identity',help='Developer ID Installer identity in the Mac keychain')
p.add_argument('--notary-profile',help='Existing notarytool keychain profile, never a password')
p.add_argument('--windows-thumbprint',help='Existing Authenticode certificate thumbprint in the Windows certificate store')
a=p.parse_args();artifact=a.artifact.resolve()
if not artifact.exists():raise SystemExit('Artifact does not exist')
def run(args):subprocess.run(args,check=True)
if artifact.suffix.lower()=='.exe':
 if not a.windows_thumbprint:raise SystemExit('Provide an existing Windows signing certificate thumbprint; no certificate is included in source.')
 signer=shutil.which('signtool')
 if not signer:raise SystemExit('Run on a Windows signing host with signtool installed.')
 run([signer,'sign','/sha1',a.windows_thumbprint,'/fd','SHA256','/tr','http://timestamp.digicert.com','/td','SHA256',str(artifact)])
 run([signer,'verify','/pa',str(artifact)])
elif artifact.suffix=='.app':
 if not a.app_identity:raise SystemExit('Provide an existing Developer ID Application identity; ad-hoc development signing uses sign-dev-macos.py.')
 for name in ['drift-router','drift-engine','drift-agent','drift-proxyctl']:
  run(['codesign','--force','--options','runtime','--timestamp','--sign',a.app_identity,str(artifact/'Contents/MacOS'/name)])
 run(['codesign','--force','--options','runtime','--timestamp','--sign',a.app_identity,str(artifact)])
 run(['codesign','--verify','--deep','--strict',str(artifact)])
elif artifact.suffix=='.pkg':
 if not a.installer_identity:raise SystemExit('Provide an existing Developer ID Installer identity.')
 with tempfile.TemporaryDirectory(prefix='drift-signed-') as temp:
  signed=Path(temp)/artifact.name
  run(['productsign','--sign',a.installer_identity,str(artifact),str(signed)])
  run(['pkgutil','--check-signature',str(signed)]);shutil.copy2(signed,artifact)
 if a.notary_profile:
  run(['xcrun','notarytool','submit',str(artifact),'--keychain-profile',a.notary_profile,'--wait'])
  run(['xcrun','stapler','staple',str(artifact)])
  run(['xcrun','stapler','validate',str(artifact)])
else:raise SystemExit('Sign a .app, .pkg or Windows .exe artifact')
print('Signature verified:',artifact.name)
