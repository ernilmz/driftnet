#!/usr/bin/env python3
from pathlib import Path
import argparse,subprocess,shutil
root=Path(__file__).resolve().parent.parent
parser=argparse.ArgumentParser();parser.add_argument('--target');parser.add_argument('--release',action='store_true');args=parser.parse_args()
host=next(line.split(': ',1)[1] for line in subprocess.check_output(['rustc','-vV'],text=True).splitlines() if line.startswith('host: '))
target=args.target or host
subprocess.run(['python3' if __import__('os').name!='nt' else 'python',str(root/'scripts/build-engine.py'),'--target',target],cwd=root,check=True)
command=['cargo','build','--locked','-p','drift-agent']+(['--release'] if args.release else [])+(['--target',target] if target!=host else [])
subprocess.run(command,cwd=root,check=True)
name='drift-agent'+('.exe' if 'windows' in target else '')
source=root/'target'/('' if target==host else target)/('release' if args.release else 'debug')/name
output=root/'apps/desktop/binaries'/('drift-agent-'+target+('.exe' if 'windows' in target else ''))
shutil.copy2(source,output);print(output)

if 'apple-darwin' in target:
    subprocess.run(['python3',str(root/'scripts/build-uninstaller.py')],check=True)
    arch={'aarch64':'arm64','x86_64':'x86_64'}[target.split('-')[0]]
    subprocess.run(['xcrun','swiftc','-swift-version','5','-O','-target',arch+'-apple-macos13','-framework','SystemConfiguration',str(root/'native/macos/ProxyHelper/main.swift'),'-o',str(root/'apps/desktop/binaries'/('drift-proxyctl-'+target))],check=True)

subprocess.run(["python3" if __import__("os").name!="nt" else "python",str(root/"scripts/build-router.py"),"--target",target],cwd=root,check=True)
