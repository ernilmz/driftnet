#!/usr/bin/env python3
"""Exercise the real compiled helper with native paths on every CI platform."""
from pathlib import Path
import os
import subprocess

root = Path(__file__).resolve().parent.parent
host = next(line.split(': ', 1)[1] for line in subprocess.check_output(['rustc', '-vV'], text=True).splitlines() if line.startswith('host: '))
helper = root / 'apps/desktop/binaries' / ('drift-engine-' + host + ('.exe' if os.name == 'nt' else ''))
environment = os.environ.copy()
environment['DRIFT_ENGINE_TEST_BINARY'] = str(helper)
subprocess.run(['cargo', 'test', '--locked', '-p', 'drift-engine', '--', '--include-ignored'], cwd=root, env=environment, check=True)
