#!/usr/bin/env python3
"""Static authority and cadence checks for immutable probation health units."""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from edge_self_change.model import DUE_COALESCE_SECONDS  # noqa: E402


def directives(path: Path) -> dict[str, list[str]]:
    parsed: dict[str, list[str]] = {}
    section = ""
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("[") and line.endswith("]"):
            section = line[1:-1]
            continue
        key, separator, value = line.partition("=")
        if not separator:
            raise AssertionError(f"malformed unit directive: {line}")
        parsed.setdefault(f"{section}.{key}", []).append(value)
    return parsed


def seconds(value: str) -> int:
    suffixes = {"s": 1, "min": 60, "h": 3_600}
    for suffix, factor in suffixes.items():
        if value.endswith(suffix):
            return int(value[: -len(suffix)]) * factor
    return int(value)


class ProbationHealthSystemdTests(unittest.TestCase):
    def setUp(self) -> None:
        systemd = ROOT / "packaging/systemd"
        self.service_path = (
            systemd / "astrid-edge-self-change-probation-health.service"
        )
        self.timer_path = systemd / "astrid-edge-self-change-probation-health.timer"
        self.supervisor_path = systemd / "astrid-edge-self-change-supervisor.service"
        self.steward_timer_path = systemd / "astrid-edge-steward.timer"
        self.supervisor_cli_path = ROOT / "scripts/edge_self_change/cli.py"

    def test_sampler_invokes_only_the_immutable_probation_command(self) -> None:
        unit = directives(self.service_path)
        self.assertEqual(
            unit["Service.ExecStart"],
            [
                "/usr/bin/python3 -I -E -s /usr/libexec/astrid/edge-self-change-supervisor "
                "--config /etc/astrid/edge-self-change.json --execute check-probation"
            ],
        )
        self.assertEqual(unit["Service.NoExecPaths"], ["/"])
        self.assertIn("/usr/bin/python3", unit["Service.ExecPaths"][0].split())
        self.assertNotIn(
            "/usr/libexec/astrid/edge-self-change-supervisor",
            unit["Service.ExecPaths"][0].split(),
        )
        self.assertEqual(
            unit["Unit.Requires"], ["astrid-edge-generation-guard.service"]
        )
        self.assertNotIn("Unit.OnSuccess", unit)
        self.assertNotIn("astrid-edge-steward.service", self.service_path.read_text())
        self.assertEqual(unit["Service.PrivateNetwork"], ["yes"])
        self.assertEqual(
            unit["Unit.JoinsNamespaceOf"], ["astrid-edge-runtime.service"]
        )
        self.assertIn("astrid-edge-runtime.service", unit["Unit.After"][0].split())
        self.assertEqual(unit["Service.IPAddressDeny"], ["any"])
        self.assertEqual(unit["Service.IPAddressAllow"], ["localhost"])
        self.assertEqual(
            unit["Service.RestrictAddressFamilies"], ["AF_UNIX AF_INET"]
        )
        cli = self.supervisor_cli_path.read_text(encoding="utf-8")
        self.assertLess(
            cli.index("with supervisor.locked():"),
            cli.index('elif args.command == "check-probation":'),
        )

    def test_isolated_python_flags_ignore_hostile_startup_environment(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sentinel = root / "loaded"
            (root / "sitecustomize.py").write_text(
                f"from pathlib import Path\nPath({str(sentinel)!r}).write_text('loaded')\n",
                encoding="utf-8",
            )
            startup = root / "startup.py"
            startup.write_text(
                f"from pathlib import Path\nPath({str(sentinel)!r}).write_text('startup')\n",
                encoding="utf-8",
            )
            environment = dict(os.environ)
            environment.update(
                {
                    "PYTHONPATH": str(root),
                    "PYTHONHOME": str(root / "nonexistent-home"),
                    "PYTHONSTARTUP": str(startup),
                }
            )
            completed = subprocess.run(
                [sys.executable, "-I", "-E", "-s", "-c", "import sys; print(sys.prefix)"],
                check=False,
                capture_output=True,
                text=True,
                env=environment,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertFalse(sentinel.exists())

    def test_five_minute_timer_supports_fail_closed_probation_coverage(self) -> None:
        timer = directives(self.timer_path)
        initial = seconds(timer["Timer.OnActiveSec"][0])
        cadence = seconds(timer["Timer.OnUnitActiveSec"][0])
        accuracy = seconds(timer["Timer.AccuracySec"][0])
        self.assertLessEqual(initial, 5 * 60)
        self.assertLessEqual(cadence, 5 * 60)
        self.assertLessEqual(cadence + accuracy, 5 * 60)
        self.assertGreaterEqual(60 * 60 // cadence, 7)
        self.assertEqual(timer["Timer.RandomizedDelaySec"], ["0"])
        self.assertEqual(
            timer["Timer.Unit"],
            ["astrid-edge-self-change-probation-health.service"],
        )

    def test_existing_reflection_cadence_and_coalescing_are_unchanged(self) -> None:
        steward = directives(self.steward_timer_path)
        supervisor = directives(self.supervisor_path)
        self.assertEqual(steward["Timer.OnUnitActiveSec"], ["15min"])
        self.assertEqual(steward["Timer.RandomizedDelaySec"], ["2min"])
        self.assertEqual(
            steward["Timer.Unit"], ["astrid-edge-self-change-supervisor.service"]
        )
        self.assertEqual(
            supervisor["Unit.OnSuccess"], ["astrid-edge-steward.service"]
        )
        self.assertEqual(DUE_COALESCE_SECONDS, 2 * 60 * 60)


if __name__ == "__main__":
    unittest.main()
