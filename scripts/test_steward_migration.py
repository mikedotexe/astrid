#!/usr/bin/env python3
"""Portability and compatibility tests for steward control migration."""

from __future__ import annotations

from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
CORE_ROOT = REPO_ROOT / "scripts/steward_control"
CONFIG_EXAMPLE = REPO_ROOT / "scripts/steward_control.example.toml"
SCHEDULER_ROOT = REPO_ROOT / "scripts/scheduler_examples"


class StewardMigrationTests(unittest.TestCase):
    def test_core_and_default_config_have_no_machine_or_vendor_dependency(
        self,
    ) -> None:
        forbidden = (
            "/users/" + "v",
            "." + "claude",
            "clau" + "de",
            "op" + "us",
        )
        paths = sorted(CORE_ROOT.glob("*.py")) + [CONFIG_EXAMPLE]
        for path in paths:
            text = path.read_text(encoding="utf-8").lower()
            for marker in forbidden:
                self.assertNotIn(marker, text, path)

    def test_scheduler_examples_are_portable_and_parse(self) -> None:
        for path in SCHEDULER_ROOT.iterdir():
            self.assertNotIn("/Users/", path.read_text(encoding="utf-8"))
        subprocess.run(
            ["bash", "-n", str(SCHEDULER_ROOT / "run_projection.sh")],
            check=True,
            capture_output=True,
        )
        with (
            SCHEDULER_ROOT / "steward-control.launchd.plist.example"
        ).open("rb") as handle:
            plist = plistlib.load(handle)
        self.assertEqual(plist["Label"], "org.example.steward-control")

    def test_retired_entrypoints_are_inert_warning_facades(self) -> None:
        loop = subprocess.run(
            ["bash", str(REPO_ROOT / "scripts/steward_loop_run.sh")],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(loop.returncode, 2)
        self.assertIn('"retired":true', loop.stderr)
        for name in ("install_steward_hooks.py", "verify_mutex_hooks.py"):
            result = subprocess.run(
                [sys.executable, str(REPO_ROOT / "scripts" / name), "--self-test"],
                check=False,
                capture_output=True,
            )
            self.assertEqual(result.returncode, 0, name)

    def test_legacy_lock_command_does_not_create_lock_state(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            lock = Path(raw) / "legacy.lock"
            result = subprocess.run(
                [
                    sys.executable,
                    str(REPO_ROOT / "scripts/steward_mutex.py"),
                    "acquire",
                    "--holder",
                    "legacy:1",
                    "--lock-dir",
                    str(lock),
                ],
                check=False,
                capture_output=True,
            )
            self.assertEqual(result.returncode, 2)
            self.assertFalse(lock.exists())

    def test_hard_coded_legacy_service_definition_is_removed(self) -> None:
        self.assertFalse(
            (REPO_ROOT / "scripts/com.astrid.steward-loop.plist").exists()
        )


if __name__ == "__main__":
    unittest.main()
