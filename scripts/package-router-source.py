#!/usr/bin/env python3
"""Bundle the exact GPL router and only dependencies used by its supported targets."""
from pathlib import Path
import gzip,json,os,subprocess,tarfile,tempfile
root=Path(__file__).resolve().parent.parent
version=json.loads((root/'apps/desktop/tauri.conf.json').read_text())['version']
source=root/'target/router-src'
if not (source/'cmd/drift-router/main.go').is_file():raise SystemExit('Build the router first')
go=os.environ.get('DRIFT_GO','go')
subprocess.run([go,'mod','vendor'],cwd=source,check=True)
files=set();vendor=source/'vendor'
for system,arch in [('darwin','arm64'),('windows','amd64'),('linux','amd64'),('linux','arm64')]:
 environment=os.environ.copy();environment.update(GOOS=system,GOARCH=arch,CGO_ENABLED='0')
 output=subprocess.check_output([go,'list','-mod=vendor','-deps','-tags','with_gvisor','-json','./cmd/drift-router'],cwd=source,env=environment,text=True)
 decoder=json.JSONDecoder();offset=0
 while offset<len(output):
  while offset<len(output) and output[offset].isspace():offset+=1
  if offset==len(output):break
  package,offset=decoder.raw_decode(output,offset)
  directory=Path(package.get('Dir','/'))
  if not directory.is_relative_to(vendor):continue
  for path in directory.iterdir():
   if path.is_file():files.add(path)
  for name in package.get('EmbedFiles',[]):files.add(directory/name)
# Keep license notices at module ancestors, including notices not imported as Go.
for path in vendor.rglob('*'):
 if path.is_file() and path.name.upper().startswith(('LICENSE','COPYING','NOTICE','PATENTS')):files.add(path)
files.add(vendor/'modules.txt')
out=root/'dist'/f'Driftnet-{version}-Router-Source.tar.gz';out.parent.mkdir(exist_ok=True)
prefix='Driftnet-router-source'
temporary=out.with_name(out.name+'.'+str(os.getpid())+'.tmp')
try:epoch=int(os.environ.get('SOURCE_DATE_EPOCH','0'))
except ValueError:raise SystemExit('SOURCE_DATE_EPOCH must be a nonnegative integer')
if epoch<0:raise SystemExit('SOURCE_DATE_EPOCH must be a nonnegative integer')
def normalized(entry):
 if entry.name==prefix+'/vendor' or '.git' in Path(entry.name).parts:return None
 # Do not publish the build machine's account name, uid, gid or file timestamps.
 entry.uid=entry.gid=0;entry.uname=entry.gname='';entry.mtime=epoch
 entry.pax_headers={}
 return entry
with temporary.open('wb') as raw:
 with gzip.GzipFile(filename='',mode='wb',fileobj=raw,mtime=0) as compressed:
  with tarfile.open(fileobj=compressed,mode='w',format=tarfile.PAX_FORMAT) as archive:
   archive.add(source,arcname=prefix,filter=normalized)
   for path in sorted(files):archive.add(path,arcname=prefix+'/'+str(path.relative_to(source)),recursive=False,filter=normalized)
   archive.add(root/'native/router/README.md',arcname=prefix+'/DRIFT-INTEGRATION.md',filter=normalized)
temporary.replace(out)
print(out, out.stat().st_size, 'bytes;',len(files),'dependency files')
