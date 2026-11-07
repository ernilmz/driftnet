#!/usr/bin/env python3
"""Run the pinned GLib iterator regression in an isolated optimized crate build.

A patched registry dependency is not a workspace member and cannot be tested
with cargo -p. Its upstream dev-dependencies belong to its own crate. Copy it
outside the app workspace so test lockfiles/build output never dirty vendor/.
"""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
subprocess.run([sys.executable, str(ROOT / "scripts/verify-glib-patch.py")], check=True)
with tempfile.TemporaryDirectory(prefix="driftnet-glib-regression-") as temporary:
    crate = Path(temporary) / "glib"
    shutil.copytree(ROOT / "vendor/glib", crate)
    environment = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / "target/glib-security-check"))
    subprocess.run(["cargo", "test", "--manifest-path", str(crate / "Cargo.toml"),
                    "--release", "--lib", "variant_iter::tests"],
                   cwd=crate, env=environment, check=True)
# Independently ensure the original vendored source was not modified by Cargo.
subprocess.run([sys.executable, str(ROOT / "scripts/verify-glib-patch.py")], check=True)
