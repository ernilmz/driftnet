#!/usr/bin/env python3
"""Embed the readable validator in both NSIS installer and uninstaller code."""
from pathlib import Path
import argparse
import hashlib

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--check', action='store_true')
args = parser.parse_args()
source = (root / 'native/windows/validate-install.ps1').read_text()
lines = [
    '; Generated from validate-install.ps1. Keep the readable source in sync.',
    '; SHA256: ' + hashlib.sha256(source.encode()).hexdigest(),
    '!macro DRIFT_WRITE_VALIDATOR',
    '  FileOpen $2 "$PLUGINSDIR\\driftnet-validate-install.ps1" w',
    '  ${If} $2 == ""',
    '    Abort "Could not prepare the Driftnet installer validation."',
    '  ${EndIf}',
]
for line in source.splitlines():
    escaped = line.replace('$', '$$').replace('"', '$\\"')
    lines.append('  FileWrite $2 "' + escaped + '$\\r$\\n"')
lines += ['  FileClose $2', '!macroend', '']
rendered = '\n'.join(lines)
destination = root / 'native/windows/validator-embedded.nsh'
if args.check:
    if destination.read_text() != rendered:
        raise SystemExit('Run python3 scripts/embed-windows-validator.py after editing the validator')
else:
    destination.write_text(rendered)
