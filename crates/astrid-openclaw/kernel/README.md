# astrid-openclaw kernel

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](../../../LICENSE-MIT)
[![MSRV: 1.94](https://img.shields.io/badge/MSRV-1.94-blue)](https://www.rust-lang.org)

**The QuickJS WASM engine that powers Tier 1 plugin compilation.**

This directory holds `engine.wasm`, a QuickJS build targeting `wasm32-wasip1`. The `astrid-openclaw` crate embeds it via `include_bytes!` and feeds it to Wizer for pre-initialization. Without this file, Tier 1 compilation fails at runtime with build instructions.

## Why it exists

Tier 1 plugins compile TypeScript into a WASM module by pre-initializing a QuickJS engine with the plugin source. That engine is this file. The reviewed `engine.wasm` and `engine.wasm.blake3` pair are checked into git as immutable signed-release inputs so a clean checkout and the offline CPU-edge source bundle consume identical bytes. `build.rs` verifies the BLAKE3 hash at compile time, and a mismatch fails the build.

The current kernel was built from [`extism/js-pdk`](https://github.com/extism/js-pdk) v1.6.0, commit `88eade10a7c6341d5d023cb503962795232fc863`. Its reviewed identities are:

- BLAKE3: `8c1685a206c32633d364701e6bd90b6658f1d92959f8136c82ad9a309c114862`
- SHA-256: `318c3b10c3f7dea63ba532bbe055a62b6c0d965688769d4f7bc4ca5fbfc8313f`
- Size: 1,568,372 bytes

When the engine is absent, `build.rs` generates a placeholder stub so workspace compilation succeeds. The stub errors at runtime, not compile time, pointing the developer at the build command.

## Building

```bash
# Deliberate replacement build (requires wasi-sdk + wasm32-wasip1 target)
./scripts/build-quickjs-kernel.sh

# Developer-only bootstrap when no reviewed kernel is present
ASTRID_AUTO_BUILD_KERNEL=1 cargo build -p astrid-openclaw
```

Any deliberate replacement must update and review the binary and hash together:

```bash
cd crates/astrid-openclaw/kernel
b3sum engine.wasm > engine.wasm.blake3
```

## Development

```bash
cargo test -p astrid-openclaw
```

## License

Astrid is dual MIT/Apache-2.0. The bundled `js-pdk`-derived kernel retains its BSD-3-Clause notice in [`LICENSE-js-pdk`](../../../LICENSE-js-pdk).
