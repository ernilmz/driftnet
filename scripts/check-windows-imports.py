#!/usr/bin/env python3
"""Reject external compiler runtimes in shipped PE files, including delay imports.

Works on any build host without dumpbin, Wine, or third-party Python modules.
Windows 10 supplies UCRT and msvcrt.dll; VC++ redistributable and MinGW runtime
DLLs must not be required by a one-package installation.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import re

EXTERNAL_RUNTIME = re.compile(
    r"(?i)^(?:(?:vcruntime|msvcp|msvcr|concrt)\d[^\\/]*\.dll|"
    r"ucrtbased\.dll|libgcc_s_[^\\/]*\.dll|libstdc\+\+-\d+\.dll|libwinpthread-\d+\.dll)$"
)


def imports(path: Path) -> list[str]:
    data = path.read_bytes()

    def number(offset: int, size: int = 4) -> int:
        if offset < 0 or offset + size > len(data):
            raise ValueError(f"Truncated PE field in {path}")
        return int.from_bytes(data[offset:offset + size], "little")

    if data[:2] != b"MZ":
        raise ValueError(f"Not a Windows PE file: {path}")
    pe = number(0x3C)
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError(f"Missing PE signature: {path}")
    optional = pe + 24
    magic = number(optional, 2)
    if magic not in (0x10B, 0x20B):
        raise ValueError(f"Unsupported PE optional header: {path}")
    directories = optional + (112 if magic == 0x20B else 96)
    directory_count = number(directories - 4)
    image_base = number(optional + (24 if magic == 0x20B else 28), 8 if magic == 0x20B else 4)
    sections = []
    section_table = optional + number(pe + 20, 2)
    for index in range(number(pe + 6, 2)):
        section = section_table + index * 40
        sections.append((number(section + 12), max(number(section + 8), number(section + 16)), number(section + 20)))

    def offset(rva: int) -> int:
        for virtual, length, raw in sections:
            if virtual <= rva < virtual + length:
                result = raw + rva - virtual
                if result < len(data):
                    return result
        raise ValueError(f"Unmapped import RVA {rva:#x} in {path}")

    def name(rva: int) -> str:
        start = offset(rva)
        end = data.find(b"\0", start, start + 1024)
        if end < 0:
            raise ValueError(f"Unterminated import name: {path}")
        return data[start:end].decode("ascii").lower()

    found = set()
    for directory, stride in ((1, 20), (13, 32)):
        if directory >= directory_count:
            continue
        address = number(directories + directory * 8)
        length = number(directories + directory * 8 + 4)
        if not address:
            continue
        start = offset(address)
        for index in range(min(length // stride + 1, 4096)):
            entry = start + index * stride
            if data[entry:entry + stride] == bytes(stride):
                break
            if directory == 1:
                found.add(name(number(entry + 12)))
            else:
                dll = number(entry + 4)
                if not (number(entry) & 1):
                    dll -= image_base
                found.add(name(dll))
        else:
            raise ValueError(f"Unterminated import descriptors: {path}")
    return sorted(found)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    files = sorted(path for path in args.directory.rglob("*") if path.is_file() and path.suffix.lower() in (".exe", ".dll"))
    if not files:
        raise SystemExit("No Windows PE files found to audit")
    report = {}
    errors = []
    for path in files:
        dependencies = imports(path)
        report[str(path.relative_to(args.directory))] = dependencies
        errors.extend(f"{path.name} requires {dll}" for dll in dependencies if EXTERNAL_RUNTIME.match(dll))
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    if errors:
        raise SystemExit("External runtime dependencies:\n" + "\n".join(errors))


if __name__ == "__main__":
    main()
