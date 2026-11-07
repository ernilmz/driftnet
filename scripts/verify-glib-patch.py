#!/usr/bin/env python3
"""Verify the GTK3 GLib safety backport against pinned upstream file hashes."""
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
VENDOR = ROOT / "vendor/glib"


def main():
    metadata = json.loads((VENDOR / "DRIFTNET_PATCH.json").read_text())
    original = metadata["original_file_sha256"]
    omitted = set(metadata["omitted_registry_metadata"])
    modified = metadata["patched_file_sha256"]
    assert set(modified) == {"Cargo.toml", "src/variant_iter.rs"}
    expected_files = set(original) - omitted
    actual_files = {str(path.relative_to(VENDOR)) for path in VENDOR.rglob("*") if path.is_file()}
    assert actual_files == expected_files | {"DRIFTNET_PATCH.json"}, "Unexpected or missing vendor files"
    for name in expected_files:
        path = VENDOR / name
        assert not path.is_symlink(), f"Unexpected symlink: {name}"
        assert hashlib.sha256(path.read_bytes()).hexdigest() == modified.get(name, original[name]), f"Changed vendor file: {name}"
    # Reverse the exact upstream fix, then independently confirm original bytes.
    variant = (VENDOR / "src/variant_iter.rs").read_bytes()
    original_variant = variant.replace(b"let mut p: *mut libc::c_char = std::ptr::null_mut();", b"let p: *mut libc::c_char = std::ptr::null_mut();", 1)
    original_variant = original_variant.replace(b"                &mut p,", b"                &p,", 1)
    assert original_variant != variant
    assert hashlib.sha256(original_variant).hexdigest() == original["src/variant_iter.rs"], "Backport differs from exact two-line upstream fix"
    manifest = (VENDOR / "Cargo.toml").read_bytes().replace(b'0.18.5+driftnet.1', b'0.18.5', 1)
    assert hashlib.sha256(manifest).hexdigest() == original["Cargo.toml"], "Unexpected package-manifest edits"
    print(f"GLib backport verified: {len(expected_files)} pinned files; exact upstream safety fix; MIT license retained")


if __name__ == "__main__":
    main()
