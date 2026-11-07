#!/usr/bin/env python3
"""Exercise the real installer preflight with harmless filesystem fixtures."""
import ast
from pathlib import Path
import platform
import subprocess
import tempfile

if platform.system() != "Darwin":
    raise SystemExit("This test uses BSD stat and runs on macOS.")
root = Path(__file__).resolve().parents[1]
module = ast.parse((root/'scripts/build-macos-installer.py').read_text())
checks = next(ast.literal_eval(node.value) for node in module.body
              if isinstance(node, ast.Assign) and any(isinstance(target, ast.Name) and target.id == 'path_checks' for target in node.targets))
functions = checks.split('safe_dir /Library\n', 1)[0]

def accepts(function: str, path: Path) -> bool:
    # The path is a positional argument, never shell text.
    result = subprocess.run(['/bin/sh', '-eu', '-c', functions + '\n' + function + ' "$1"', 'preflight', str(path)], capture_output=True, text=True)
    return result.returncode == 0

assert accepts('safe_dir', Path('/Library'))
assert accepts('safe_file', Path('/usr/bin/true'))
with tempfile.TemporaryDirectory(prefix='driftnet-installer-security-') as temporary:
    directory = Path(temporary)
    victim = directory/'victim'
    victim.write_text('unchanged')
    link = directory/'owner.json'
    link.symlink_to(victim)
    assert not accepts('safe_file', link), 'accepted a symlink in a privileged destination'
    folder_link = directory/'state-link'
    folder_link.symlink_to(directory, target_is_directory=True)
    assert not accepts('safe_dir', folder_link), 'accepted a symlink directory'
    assert not accepts('safe_dir', directory), 'promoted an unprivileged directory'
    assert not accepts('safe_file', victim), 'trusted an unprivileged existing file'
    assert victim.read_text() == 'unchanged', 'preflight touched the linked destination'
print('Installer preflight rejected unprivileged and symlink fixtures without writes.')
