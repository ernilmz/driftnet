# GLib safety backport

The Linux Tauri/GTK3 dependency chain requires GLib Rust bindings 0.18. Updating
only this dependency to 0.20 would not preserve that API family. The project
therefore vendors the official `glib` 0.18.5 crate with the exact two-line upstream
fix from [gtk-rs-core PR 1343](https://github.com/gtk-rs/gtk-rs-core/pull/1343),
addressing [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html).

The C output pointer in `VariantStrIter::impl_get` is now declared mutable and
passed as `&mut p`. No other upstream source behavior is changed. Cargo labels the
local package `0.18.5+driftnet.1`; this remains compatible with GTK3's 0.18 dependency.
The original MIT license is retained at `vendor/glib/LICENSE`.

`vendor/glib/DRIFTNET_PATCH.json` records the crates.io archive checksum, every
original file checksum, the two modified-file checksums, omitted registry metadata,
and the upstream merge commit. The accompanying patch describes only the source
fix; the other change is the package's local build metadata in `Cargo.toml`.

Run `python3 scripts/verify-glib-patch.py` to verify all retained original bytes and
the declared modifications. On a Linux host with native GLib development libraries,
run the upstream iterator tests with optimization enabled:

```sh
cargo test --release -p glib --lib variant_iter::tests
```

`cargo audit` may continue to flag the original version range: Cargo build metadata
does not remove an advisory. Record the reviewed backport rather than globally
ignoring the advisory or claiming that the upstream version was upgraded.
