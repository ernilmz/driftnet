#!/usr/bin/env python3
"""Build Drift's pinned engine from source; no end-user compiler is needed."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import subprocess
import tempfile

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('--target', help='Tauri/Rust target triple; defaults to rustc host')
args = parser.parse_args()
if not args.target:
    args.target = next(line.split(': ', 1)[1] for line in subprocess.check_output(['rustc', '-vV'], text=True).splitlines() if line.startswith('host: '))
source = root / 'vendor/byedpi'
manifest = json.loads((source / 'UPSTREAM.json').read_text())
for entry in manifest['files']:
    actual = hashlib.sha256((source / entry['path']).read_bytes()).hexdigest()
    if actual != entry['sha256']:
        raise SystemExit(f"Upstream source changed without updating provenance: {entry['path']}")
windows = 'windows' in args.target
mac = 'apple-darwin' in args.target
if not (windows or mac or 'linux' in args.target):
    raise SystemExit('Unsupported engine target')
compiler = shlex.split(os.environ.get('DRIFT_ENGINE_CC', 'cc'))
flags = ['-D_DEFAULT_SOURCE', '-std=c99', '-O2', '-I', str(source)]
if mac:
    flags += ['-arch', {'aarch64': 'arm64', 'x86_64': 'x86_64'}[args.target.split('-')[0]], '-mmacosx-version-min=13.0']
files = ['packets.c','main.c','conev.c','proxy.c','desync.c','mpool.c','extend.c']
if windows:
    files += ['win_service.c']
output = root / 'apps/desktop/binaries' / ('drift-engine-' + args.target + ('.exe' if windows else ''))
output.parent.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix='drift-engine-build-') as build_dir:
    objects = []
    for index, file in enumerate([source / name for name in files] + [root / 'native/engine/parent_watch.c']):
        obj = Path(build_dir) / (str(index) + '.o')
        fixes = json.loads((root / 'native/engine/fixes.json').read_text())
        for fix in fixes:
            if file.name == fix['source']:
                text = file.read_text()
                if text.count(fix['before']) != 1:
                    raise SystemExit('Engine fix no longer matches the pinned source')
                patched = Path(build_dir) / file.name
                patched.write_text(text.replace(fix['before'], fix['after']))
                file = patched
        cmd = compiler + flags + (['-Dmain=byedpi_main'] if file.name == 'main.c' else []) + ['-c', str(file), '-o', str(obj)]
        subprocess.run(cmd, check=True)
        objects.append(str(obj))
    cmd = compiler + (['-arch', {'aarch64': 'arm64', 'x86_64': 'x86_64'}[args.target.split('-')[0]], '-mmacosx-version-min=13.0'] if mac else []) + objects + ['-o', str(output)]
    if windows:
        cmd += ['-static-libgcc', '-lws2_32', '-lmswsock']
    else:
        cmd += ['-pthread']
    subprocess.run(cmd, check=True)
print(output)
