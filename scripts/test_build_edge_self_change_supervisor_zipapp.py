#!/usr/bin/env python3
"""Tests for the immutable supervisor zipapp builder."""

from __future__ import annotations

import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("build_edge_self_change_supervisor_zipapp.py")
SPEC = importlib.util.spec_from_file_location("edge_supervisor_zipapp", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
builder = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = builder
SPEC.loader.exec_module(builder)


class ZipappTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.source = self.root / "edge_self_change"
        self.source.mkdir()
        for name in ("__init__.py", "cli.py", "model.py", "profiles.py", "supervisor.py"):
            (self.source / name).write_text("\n", encoding="utf-8")
        (self.source / "cli.py").write_text("def main():\n    print('ok')\n    return 0\n")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_build_is_deterministic_executable_and_non_overwriting(self) -> None:
        first = self.root / "first.pyz"
        second = self.root / "second.pyz"
        one = builder.build(self.source, first)
        two = builder.build(self.source, second)
        self.assertEqual(one["sha256"], two["sha256"])
        self.assertEqual(first.read_bytes(), second.read_bytes())
        self.assertEqual(subprocess.run([str(first)], check=False, capture_output=True, text=True).stdout, "ok\n")
        with self.assertRaisesRegex(builder.BuildError, "overwrite"):
            builder.build(self.source, first)

    def test_incomplete_and_linked_sources_fail(self) -> None:
        (self.source / "model.py").unlink()
        with self.assertRaisesRegex(builder.BuildError, "incomplete"):
            builder.build(self.source, self.root / "missing.pyz")
        (self.source / "model.py").write_text("\n")
        os.symlink("model.py", self.source / "linked.py")
        with self.assertRaisesRegex(builder.BuildError, "regular unlinked"):
            builder.build(self.source, self.root / "linked.pyz")


if __name__ == "__main__":
    unittest.main()
