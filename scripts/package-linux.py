#!/usr/bin/env python3
"""Build one native .deb containing UI, engine and systemd routing helper.

Run on Ubuntu 24.04+ after the release desktop and Linux sidecars are built.
Never packages a macOS/Windows executable as a Linux helper.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
ARCHES = {"x86_64": ("amd64", "x86_64-unknown-linux-gnu", 62), "aarch64": ("arm64", "aarch64-unknown-linux-gnu", 183)}

def elf(path: Path, machine: int) -> None:
    header = path.read_bytes()[:20]
    if len(header) != 20 or header[:4] != b"\x7fELF" or header[4] != 2 or header[5] != 1 or int.from_bytes(header[18:20], "little") != machine:
        raise SystemExit(f"Expected a native 64-bit Linux ELF: {path}")

def build(args: argparse.Namespace) -> Path:
    if platform.system() != "Linux":
        raise SystemExit("Build this package on Linux (the GUI requires native WebKitGTK libraries).")
    architecture, target, machine = ARCHES[args.arch]
    config = json.loads((ROOT / "apps/desktop/tauri.conf.json").read_text())
    version = args.version or config["version"]
    if not re.fullmatch(r"[0-9][A-Za-z0-9.+~:-]*", version):
        raise SystemExit("Invalid Debian package version")
    binary_dir = args.binary_dir or ROOT / "apps/desktop/binaries"
    desktop = args.desktop or ROOT / "target/release/drift-desktop"
    sources = {"drift-desktop": desktop}
    for name in ["drift-agent", "drift-engine", "drift-router"]:
        source = binary_dir / f"{name}-{target}"
        if not source.is_file(): source = binary_dir / name
        sources[name] = source
    for source in sources.values(): elf(source, machine)
    output = args.output or ROOT / "dist" / f"Driftnet-{version}-linux-{architecture}.deb"
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="driftnet-deb-") as temporary:
        stage = Path(temporary)
        def install(source: Path, destination: str, executable: bool = False) -> None:
            path = stage / destination
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, path)
            path.chmod(0o755 if executable else 0o644)
        for name, source in sources.items(): install(source, f"usr/lib/driftnet/{name}", True)
        (stage / "usr/bin").mkdir(parents=True)
        (stage / "usr/bin/driftnet").symlink_to("../lib/driftnet/drift-desktop")
        install(ROOT / "native/linux/driftnet-network.service", "lib/systemd/system/driftnet-network.service")
        install(ROOT / "native/linux/driftnet.desktop", "usr/share/applications/app.drift.desktop.desktop")
        icon = ROOT / "apps/desktop/icons/128x128@2x.png"
        if not icon.exists(): icon = ROOT / "apps/desktop/icons/128x128.png"
        install(icon, "usr/share/icons/hicolor/256x256/apps/driftnet.png")
        for name in ["LICENSE", "THIRD_PARTY_NOTICES.md"]:
            if (ROOT / name).is_file(): install(ROOT / name, f"usr/share/doc/driftnet/{name}")
        install(ROOT / "native/router/LICENSE", "usr/share/doc/driftnet/router-LICENSE")
        # Mirror the fixed license/provenance files in the base desktop bundle.
        # Installer-only macOS resources are deliberately outside this namespace.
        for relative, destination in config["bundle"].get("resources", {}).items():
            if not destination.startswith("licenses/"):
                continue
            source = (ROOT / "apps/desktop" / relative).resolve()
            parts = Path(destination).parts
            if not source.is_relative_to(ROOT) or not source.is_file() or ".." in parts:
                raise SystemExit(f"Invalid license resource: {relative}")
            install(source, f"usr/share/doc/driftnet/{destination}")
        for name in ["postinst", "prerm", "postrm"]: install(ROOT / "native/linux" / name, f"DEBIAN/{name}", True)
        (stage / "DEBIAN/control").write_text(f"""Package: driftnet
Version: {version}
Architecture: {architecture}
Maintainer: Driftnet contributors
Section: net
Priority: optional
Depends: libc6 (>= 2.39), libwebkit2gtk-4.1-0, libgtk-3-0, libayatana-appindicator3-1, librsvg2-2, libssl3t64 | libssl3, libcap2-bin, systemd, gsettings-desktop-schemas, pkexec
Description: Selective application and website access
 Native application routing and GNOME website rules, controlled from the tray.
""")
        subprocess.run(["dpkg-deb", "--build", "--root-owner-group", str(stage), str(output)], check=True)
    return output

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--arch", choices=ARCHES, default=platform.machine())
    parser.add_argument("--desktop", type=Path)
    parser.add_argument("--binary-dir", type=Path)
    parser.add_argument("--version")
    parser.add_argument("--output", type=Path)
    print(build(parser.parse_args()))
