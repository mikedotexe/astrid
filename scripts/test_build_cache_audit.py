"""Synthetic, read-only cache-budget qualification; no real cache cleanup."""

from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import build_cache_audit as audit


class BuildCacheAuditTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve()
        self.target = self.root / "target"
        self.target.mkdir()
        (self.target / "CACHEDIR.TAG").write_bytes(audit.TAG + b"\n")

    def add(self, relative):
        path = self.target / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"synthetic fixture\n")
        return path

    def report(self):
        with patch.object(audit.shutil, "disk_usage", return_value=shutil._ntuple_diskusage(
            200 * audit.GIB, 100 * audit.GIB, 100 * audit.GIB
        )):
            return audit.audit([self.target], 40, 50)

    def test_host_cross_target_and_trybuild_without_release(self):
        paths = [self.add(p) for p in (
            "debug/deps/a", "aarch64-apple-darwin/debug/deps/b",
            "tests/trybuild/debug/deps/c", "tests/trybuild/aarch64-apple-darwin/debug/deps/d",
            "release/live-binary", "tests/trybuild/release/rollback",
            "tests/trybuild/synthetic-project/src/main.rs",
        )]
        before = {p: (p.read_bytes(), p.stat().st_mtime_ns) for p in paths}
        result = self.report()
        self.assertEqual(len(result["targets"][0]["scopes"]), 4)
        self.assertGreater(result["targets"][0]["debug_allocated_bytes"], 0)
        self.assertEqual(result["status"], "within_budget")
        self.assertFalse(result["cleanup_authorized"])
        self.assertEqual(before, {p: (p.read_bytes(), p.stat().st_mtime_ns) for p in paths})

    def test_budgets_and_shared_device_are_explicit(self):
        self.add("debug/deps/a")
        with patch.object(audit, "directory_bytes", return_value=41 * audit.GIB), patch.object(
            audit.shutil, "disk_usage", return_value=shutil._ntuple_diskusage(100, 90, 10)
        ):
            result = audit.audit([self.target, self.target], 40, 50)
        self.assertEqual(len(result["targets"]), 1)
        self.assertEqual(len(result["volumes"]), 1)
        self.assertEqual({w["reason"] for w in result["warnings"]},
                         {"debug_cache_over_budget", "free_space_below_reserve"})

    def test_missing_cache_is_not_created(self):
        missing = self.root / "new/target"
        result = audit.audit([missing], 40, 0.001)
        self.assertFalse(missing.exists())
        self.assertFalse(result["targets"][0]["exists"])

    def test_invalid_tag_and_untagged_directory_fail(self):
        tag = self.target / "CACHEDIR.TAG"
        for value in (b"not Cargo", None):
            if value is None:
                tag.unlink()
            else:
                tag.write_bytes(value)
            with self.assertRaises(ValueError):
                self.report()

    def test_symlinked_target_or_debug_scope_fails(self):
        alias = self.root / "alias"
        alias.symlink_to(self.target, target_is_directory=True)
        with self.assertRaises(ValueError):
            audit.audit([alias], 40, 50)
        (self.target / "debug").symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(ValueError):
            self.report()

    def test_symlinked_trybuild_parent_fails(self):
        (self.target / "tests").symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(ValueError):
            self.report()

    def test_nested_targets_fail(self):
        nested = self.target / "nested"
        nested.mkdir()
        (nested / "CACHEDIR.TAG").write_bytes(audit.TAG)
        with self.assertRaises(ValueError):
            audit.audit([self.target, nested], 40, 50)

    def test_symlinked_cross_target_cannot_silently_disappear(self):
        elsewhere = self.root / "elsewhere"
        (elsewhere / "debug").mkdir(parents=True)
        for relative in ("aarch64-unknown-linux-gnu", "tests/trybuild/aarch64-unknown-linux-gnu"):
            link = self.target / relative
            link.parent.mkdir(parents=True, exist_ok=True)
            link.symlink_to(elsewhere, target_is_directory=True)
            with self.subTest(relative=relative), self.assertRaises(ValueError):
                self.report()
            link.unlink()

    def test_failed_or_incomplete_measurement_is_not_zero(self):
        self.add("debug/a")
        with patch.object(audit.subprocess, "run", side_effect=subprocess.TimeoutExpired("du", 300)):
            with self.assertRaises(subprocess.TimeoutExpired):
                self.report()
        with patch.object(audit.subprocess, "run", return_value=subprocess.CompletedProcess(
            ["du"], 0, "1\t/path\n", "permission denied"
        )):
            with self.assertRaises(ValueError):
                self.report()

    def test_cli_check_and_incomplete_status(self):
        with patch.object(audit, "audit", return_value={"warnings": ["synthetic"]}), patch("builtins.print"):
            self.assertEqual(audit.main(["--check"]), 2)
            self.assertEqual(audit.main([]), 0)
        with patch.object(audit, "audit", side_effect=ValueError("synthetic")), patch("builtins.print"):
            self.assertEqual(audit.main(["--check"]), 3)

    def test_nonfinite_or_nonpositive_budgets_rejected(self):
        for value in ("0", "-1", "nan", "inf"):
            with self.subTest(value=value), self.assertRaises(SystemExit), patch("sys.stderr"):
                audit.main(["--max-debug-gib", value])


if __name__ == "__main__":
    unittest.main()
