#!/usr/bin/env python3
"""Sign complete installers and prepare latest.json. This never publishes files.

Example:
  python3 scripts/prepare-updates.py --version 1.0.1 --output dist/updates \
    --asset darwin-aarch64=dist/Driftnet-1.0.1-macOS.pkg \
    --asset windows-x86_64=dist/Driftnet-1.0.1-Setup.exe

Keep updater.key outside the repository. Back it up securely: existing clients
trust its public counterpart and cannot accept a replacement signing key.
"""

import argparse
import base64
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    "darwin-aarch64": ".pkg",
    "darwin-x86_64": ".pkg",
    "windows-x86_64": ".exe",
    "windows-aarch64": ".exe",
    "linux-x86_64": ".deb",
    "linux-aarch64": ".deb",
}


def prepare(args):
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", args.version):
        raise ValueError("Use a semantic version without the leading v.")
    key = Path(args.key).expanduser().resolve()
    if not key.is_file():
        raise ValueError("The updater signing key was not found outside the repository.")
    if key.is_relative_to(ROOT):
        raise ValueError("Move the private signing key outside the repository before signing.")
    if os.name != "nt" and key.stat().st_mode & 0o077:
        raise ValueError("The private signing key must have mode 0600.")
    signer = ROOT / "ui/node_modules/.bin/tauri"
    if os.name == "nt":
        signer = signer.with_suffix(".cmd")
    if not signer.exists():
        raise ValueError("Install the locked UI dependencies before signing (npm ci in ui).")
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)

    def sign(destination):
        # Capture signer output; private key material is never logged or printed.
        result = subprocess.run(
            [str(signer), "signer", "sign", "--private-key-path", str(key),
             "--password", "", "--app-version", args.version, str(destination)],
            cwd=ROOT, capture_output=True, text=True,
        )
        if result.returncode:
            raise ValueError(f"Signing failed for {destination.name}. No new manifest was published.")
        signature = Path(str(destination) + ".sig").read_text().strip()
        decoded = base64.b64decode(signature, validate=True).decode()
        trusted = next((line for line in decoded.splitlines() if line.startswith("trusted comment:")), "")
        if trusted.split("\t").count(f"version:{args.version}") != 1:
            raise ValueError("The signer did not authenticate the requested app version.")
        return signature

    platforms = {}
    names = set()
    for item in args.asset:
        target, separator, value = item.partition("=")
        if not separator or target not in TARGETS or target in platforms:
            raise ValueError(f"Invalid or repeated target: {target}")
        source = Path(value).resolve()
        if not source.is_file() or source.suffix.lower() != TARGETS[target]:
            raise ValueError(f"{target} requires a complete {TARGETS[target]} installer.")
        if source.name in names:
            raise ValueError("Each architecture needs a distinct installer filename.")
        names.add(source.name)
        destination = output / source.name
        if source != destination:
            shutil.copyfile(source, destination)
        signature = sign(destination)
        platforms[target] = {
            "url": f"https://github.com/ernilmz/driftnet/releases/download/v{args.version}/{quote(source.name, safe='')}",
            "signature": signature,
        }
    manifest = {"version": args.version, "platforms": platforms}
    manifest_path = output / "latest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    sign(manifest_path)
    print(f"Prepared {len(platforms)} signed installer(s), latest.json and latest.json.sig in {output}; nothing uploaded.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--asset", action="append", required=True, metavar="TARGET=PATH")
    parser.add_argument("--output", default="dist/updates")
    parser.add_argument("--key", default=os.environ.get("DRIFTNET_SIGNING_KEY", "~/.local/share/driftnet/signing/updater.key"))
    try:
        prepare(parser.parse_args())
    except (ValueError, OSError, StopIteration) as error:
        print(str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
