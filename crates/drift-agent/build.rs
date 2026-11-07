fn main() {
    // Match Tauri's deployment model: Windows 10 supplies UCRT, while the VC
    // runtime must be linked into this standalone helper for a clean machine.
    // The upstream hook is a no-op for GNU, macOS, and Linux targets.
    static_vcruntime::metabuild();
}
