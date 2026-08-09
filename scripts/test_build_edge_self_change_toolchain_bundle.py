#!/usr/bin/env python3
"""Tests for the signed CPU-edge Rust toolchain bundle."""

from __future__ import annotations

import argparse
import importlib.util
import os
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("build_edge_self_change_toolchain_bundle.py")
SPEC = importlib.util.spec_from_file_location("edge_toolchain_bundle", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
bundle = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = bundle
SPEC.loader.exec_module(bundle)


class ToolchainBundleTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.toolchain = self.root / "toolchain"
        (self.toolchain / "bin").mkdir(parents=True)
        (self.toolchain / "lib/rustlib/wasm32-wasip2/lib").mkdir(parents=True)
        rustc = f"""#!/bin/sh
cat <<'EOF'
rustc {bundle.RUST_RELEASE} ({bundle.RUST_COMMIT[:9]} {bundle.RUST_DATE})
binary: rustc
commit-hash: {bundle.RUST_COMMIT}
commit-date: {bundle.RUST_DATE}
host: x86_64-unknown-linux-gnu
release: {bundle.RUST_RELEASE}
LLVM version: {bundle.LLVM_VERSION}
EOF
"""
        cargo = "#!/bin/sh\nprintf '%s\\n' 'cargo 1.94.1 (29ea6fb6a 2026-03-24)'\n"
        self._write(self.toolchain / "bin/rustc", rustc, 0o755)
        self._write(self.toolchain / "bin/cargo", cargo, 0o755)
        self._write(self.toolchain / "lib/rustlib/wasm32-wasip2/lib/libstd.rlib", b"wasm")
        self.key = self.root / "key"
        self._write(self.key, b"K" * 32, 0o600)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    @staticmethod
    def _write(path: Path, value: str | bytes, mode: int = 0o644) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(value.encode() if isinstance(value, str) else value)
        path.chmod(mode)

    def build_args(self, output: Path) -> argparse.Namespace:
        return argparse.Namespace(
            toolchain_dir=self.toolchain,
            target="x86_64-unknown-linux-gnu",
            signing_key=self.key,
            output=output,
        )

    def verify_args(self, output: Path, key: Path | None = None) -> argparse.Namespace:
        return argparse.Namespace(bundle=output, signing_key=key or self.key)

    def test_bundle_is_deterministic_and_verifies(self) -> None:
        first = self.root / "first.tar.gz"
        second = self.root / "second.tar.gz"
        bundle.build(self.build_args(first))
        bundle.build(self.build_args(second))
        self.assertEqual(first.read_bytes(), second.read_bytes())
        result = bundle.verify(self.verify_args(first))
        self.assertEqual(result["target"], "x86_64-unknown-linux-gnu")
        self.assertEqual(result["file_count"], 3)

    def test_wrong_identity_or_missing_wasm_target_fails(self) -> None:
        rustc = self.toolchain / "bin/rustc"
        self._write(rustc, rustc.read_text().replace("1.94.1", "1.95.0"), 0o755)
        with self.assertRaisesRegex(bundle.BundleError, "reviewed 1.94.1"):
            bundle.build(self.build_args(self.root / "wrong.tar.gz"))
        self.setUp_repair_rustc()
        os.rename(
            self.toolchain / "lib/rustlib/wasm32-wasip2",
            self.toolchain / "lib/rustlib/wasm32-wasip2-missing",
        )
        with self.assertRaisesRegex(bundle.BundleError, "wasm32-wasip2"):
            bundle.build(self.build_args(self.root / "missing.tar.gz"))

    def setUp_repair_rustc(self) -> None:
        rustc = self.toolchain / "bin/rustc"
        self._write(rustc, rustc.read_text().replace("1.95.0", "1.94.1"), 0o755)

    def test_links_special_files_and_existing_output_fail(self) -> None:
        link = self.toolchain / "bin/linked"
        os.symlink("cargo", link)
        with self.assertRaisesRegex(bundle.BundleError, "link or special"):
            bundle.build(self.build_args(self.root / "link.tar.gz"))
        link.unlink()
        fifo = self.toolchain / "fifo"
        os.mkfifo(fifo)
        with self.assertRaisesRegex(bundle.BundleError, "link or special"):
            bundle.build(self.build_args(self.root / "fifo.tar.gz"))
        fifo.unlink()
        output = self.root / "existing.tar.gz"
        output.write_bytes(b"keep")
        with self.assertRaisesRegex(bundle.BundleError, "overwrite"):
            bundle.build(self.build_args(output))
        self.assertEqual(output.read_bytes(), b"keep")

    def test_wrong_key_and_corrupt_archive_fail(self) -> None:
        output = self.root / "bundle.tar.gz"
        bundle.build(self.build_args(output))
        wrong = self.root / "wrong"
        self._write(wrong, b"W" * 32, 0o600)
        with self.assertRaisesRegex(bundle.BundleError, "signature"):
            bundle.verify(self.verify_args(output, wrong))
        corrupted = self.root / "corrupt.tar.gz"
        data = bytearray(output.read_bytes())
        data[len(data) // 2] ^= 1
        corrupted.write_bytes(data)
        with self.assertRaises((bundle.BundleError, OSError, EOFError)):
            bundle.verify(self.verify_args(corrupted))

    def test_key_must_be_owner_only_and_target_exact(self) -> None:
        self.key.chmod(0o644)
        with self.assertRaisesRegex(bundle.BundleError, "owner-only"):
            bundle.build(self.build_args(self.root / "key.tar.gz"))
        self.key.chmod(0o600)
        args = self.build_args(self.root / "target.tar.gz")
        args.target = "x86_64-apple-darwin"
        with self.assertRaisesRegex(bundle.BundleError, "unsupported"):
            bundle.build(args)


if __name__ == "__main__":
    unittest.main()
