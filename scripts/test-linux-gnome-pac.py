#!/usr/bin/env python3
"""Verify real GNOME/dconf PAC writes and crash recovery in a private CI session."""
import os
from pathlib import Path
import platform
import subprocess
import tempfile

if platform.system() != "Linux" or os.environ.get("GITHUB_ACTIONS") != "true" or os.geteuid() == 0:
    raise SystemExit("Run only as the ordinary user on an ephemeral Linux GitHub Actions runner.")
root = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="driftnet-gnome-pac-") as temporary:
    folder = Path(temporary)
    (folder / "config").mkdir()
    (folder / "state").mkdir()
    (folder / "runtime").mkdir(mode=0o700)
    environment = dict(os.environ, XDG_CONFIG_HOME=str(folder / "config"),
                       XDG_STATE_HOME=str(folder / "state"), XDG_RUNTIME_DIR=str(folder / "runtime"), XDG_CURRENT_DESKTOP="GNOME",
                       GSETTINGS_BACKEND="dconf", DRIFT_TEST_REAL_GSETTINGS="1")
    subprocess.run(["dbus-run-session", "--", "cargo", "test", "--locked", "-p", "drift-network",
                    "platform::linux::tests::real_gnome_proxy_lease_recovers", "--", "--exact", "--ignored", "--nocapture"],
                   cwd=root, env=environment, check=True, timeout=120)
