#!/usr/bin/env bash
# Build the QuickJS WASM kernel from extism/js-pdk and install it.
#
# This script clones the js-pdk repository, builds the QuickJS core crate
# to wasm32-wasip1, and copies the resulting WASM to the kernel directory.
#
# Prerequisites:
#   - Rust with wasm32-wasip1 target: rustup target add wasm32-wasip1
#   - wasi-sdk (for rquickjs C bindings): https://github.com/WebAssembly/wasi-sdk
#   - Node.js + npm (for the JS prelude)
#   - Optional: wasm-opt from Binaryen (for size optimization)
#
# Usage:
#   ./scripts/build-quickjs-kernel.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
KERNEL_DIR="$ROOT_DIR/crates/astrid-openclaw/kernel"
BUILD_ROOT="${TMPDIR:-/tmp}/quickjs-kernel-build"
BUILD_DIR="$BUILD_ROOT/js-pdk"

JS_PDK_REPO="https://github.com/extism/js-pdk.git"
JS_PDK_REV="88eade10a7c6341d5d023cb503962795232fc863"

# Check wasm32-wasip1 target is installed
if ! rustup target list --installed | grep -q wasm32-wasip1; then
    echo "Installing wasm32-wasip1 target..."
    rustup target add wasm32-wasip1
fi

echo "==> Fetching reviewed js-pdk ${JS_PDK_REV}..."
rm -rf "$BUILD_ROOT"
mkdir -p "$BUILD_DIR"
git -C "$BUILD_DIR" init --quiet
git -C "$BUILD_DIR" remote add origin "$JS_PDK_REPO"
git -C "$BUILD_DIR" fetch --quiet --depth 1 origin "$JS_PDK_REV"
git -C "$BUILD_DIR" checkout --quiet --detach FETCH_HEAD
test "$(git -C "$BUILD_DIR" rev-parse HEAD)" = "$JS_PDK_REV"

echo "==> Installing wasi-sdk..."
cd "$BUILD_DIR"
sh install-wasi-sdk.sh

echo "==> Building JS prelude..."
cd "$BUILD_DIR/crates/core/src/prelude"
npm install
npm run build

echo "==> Building QuickJS core (wasm32-wasip1)..."
cd "$BUILD_DIR/crates/core"
WASI_SDK="$BUILD_DIR/wasi-sdk" \
WASI_SDK_PATH="$BUILD_DIR/wasi-sdk" \
    cargo build --release --target=wasm32-wasip1 --target-dir "$BUILD_DIR/target"

BUILT_WASM="$BUILD_DIR/target/wasm32-wasip1/release/js_pdk_core.wasm"
if [ ! -f "$BUILT_WASM" ]; then
    echo "ERROR: Build output not found at $BUILT_WASM"
    exit 1
fi

# Optional: optimize with wasm-opt
if command -v wasm-opt &>/dev/null; then
    echo "==> Optimizing with wasm-opt..."
    wasm-opt --enable-reference-types --enable-bulk-memory --strip -O3 \
        "$BUILT_WASM" -o "$BUILT_WASM"
fi

# A deliberate replacement is reviewable only when the binary and verifier are
# produced together. Refuse to mutate the tracked pair without b3sum.
if ! command -v b3sum &>/dev/null; then
    echo "ERROR: b3sum is required to replace the reviewed QuickJS kernel" >&2
    exit 1
fi
echo "==> Installing kernel..."
mkdir -p "$KERNEL_DIR"
cp "$BUILT_WASM" "$KERNEL_DIR/engine.wasm"
cd "$KERNEL_DIR"
b3sum engine.wasm > engine.wasm.blake3
echo "==> Updated blake3 hash"

SIZE=$(wc -c < "$KERNEL_DIR/engine.wasm" | tr -d ' ')
echo "==> Success: $KERNEL_DIR/engine.wasm ($SIZE bytes)"
echo ""
echo "Rebuild astrid-openclaw to embed the kernel:"
echo "  cargo build -p astrid-openclaw"

# Cleanup
rm -rf "$BUILD_ROOT"
