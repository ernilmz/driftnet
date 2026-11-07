#!/usr/bin/env python3
"""Build the GPL router from pinned source with documented owner filtering."""
from pathlib import Path
import argparse,json,subprocess,shutil,os,hashlib
root=Path(__file__).resolve().parent.parent
p=argparse.ArgumentParser();p.add_argument('--target',default='aarch64-apple-darwin');args=p.parse_args()
platform = 'windows' if 'windows' in args.target else 'linux' if 'linux' in args.target else 'darwin' if 'apple-darwin' in args.target else None
if platform is None: raise SystemExit('Unsupported router target: ' + args.target)
arch = 'amd64' if args.target.startswith('x86_64') else 'arm64' if args.target.startswith('aarch64') else None
if arch is None: raise SystemExit('Unsupported router architecture: ' + args.target)
source=root/'vendor/sing-box';build=root/'target/router-src'
manifest=json.loads((source/'DRIFT_UPSTREAM.json').read_text())
for entry in manifest['files']:
 if hashlib.sha256((source/entry['path']).read_bytes()).hexdigest()!=entry['sha256']:raise SystemExit('Pinned router source changed: '+entry['path'])
if not build.exists():shutil.copytree(source,build)
# Always reset and apply this explicit, auditable modification to the original.
original=(source/'common/process/searcher.go').read_text();anchor='\tif info.UserId != -1 && info.UserName == "" {'
assert original.count(anchor)==1
(build/'common/process/searcher.go').write_text(original.replace(anchor,'\tif !driftOwnerAllowed(info) { driftUnknown(); return nil, ErrNotFound }; driftRecord(info)\n'+anchor))
# Kernel-reported socket delegation attributes shared networking helpers to
# the actual responsible app (for example Safari), with a verified process uid.
shared=(source/'common/process/searcher_darwin_shared.go').read_text()
shared=shared.replace('pid        uint32','pid        uint32\n\tdelegatedPID uint32',1)
shared=shared.replace('pid:        binary.NativeEndian.Uint32(so[darwinXsocketLastPID : darwinXsocketLastPID+4]),','pid:        binary.NativeEndian.Uint32(so[darwinXsocketLastPID : darwinXsocketLastPID+4]),\n\t\tdelegatedPID: binary.NativeEndian.Uint32(so[72:76]),',1)
shared=shared.replace('UserId: entry.uid,','UserId: entry.uid,\n\t\t\tProcessID: entry.pid,',1)
shared=shared.replace('\t\tlastOwner = owner','\t\tif entry.delegatedPID>0 { owner.ProcessID=entry.delegatedPID; uid,err:=driftProcessUID(entry.delegatedPID);if err!=nil{return nil,err};owner.UserId=uid }\n\t\tlastOwner = owner',1)
shared=shared.replace('if entry.pid == 0','if owner.ProcessID == 0',1).replace('getExecPathFromPID(entry.pid)','getExecPathFromPID(owner.ProcessID)',1)
(build/'common/process/searcher_darwin_shared.go').write_text(shared)
for file in (root/'native/router/process').glob('*.go'):shutil.copy2(file,build/'common/process'/('drift_'+file.name))
cmd=build/'cmd/drift-router';cmd.mkdir(parents=True,exist_ok=True)
for file in (root/'native/router').glob('*.go'):
 shutil.copy2(file,cmd/file.name)
env=os.environ.copy();env.update(GOOS=platform,GOARCH=arch,CGO_ENABLED='0')
go=env.get('DRIFT_GO','go')
# go-winio is used only for the authenticated local Windows control pipe.
if platform=='windows':subprocess.run([go,'get','github.com/Microsoft/go-winio@v0.6.2'],cwd=build,env=env,check=True)
output=root/'apps/desktop/binaries'/('drift-router-'+args.target+('.exe' if platform=='windows' else ''))
subprocess.run([go,'build','-mod=mod','-trimpath','-tags','with_gvisor',('-ldflags=-s -w -H windowsgui' if platform=='windows' else '-ldflags=-s -w'),'-o',str(output),'./cmd/drift-router'],cwd=build,env=env,check=True)
print(output)
