#!/usr/bin/env python3
"""Contract tests for active-generation, explicitly untrusted report modes."""

from __future__ import annotations

import json
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parent
SCHEMA = "astrid.edge_candidate_presentation.content.v1"
INPUT_SCHEMA = "astrid.edge_candidate_presentation.input.v1"


def projection() -> bytes:
    value = {
        "schema": INPUT_SCHEMA,
        "appliance_id": "avado",
        "generated_at_unix_ms": 1,
        "source": "immutable_operator_reports_sanitized_projection",
        "source_sha256": "a" * 64,
        "facts": [
            {"key": "fill_mean_pct", "value": "68.0", "provenance": "trusted"},
            {"key": "status", "value": "safe\u001btext", "provenance": "trusted"},
        ],
        "recent_activity": [
            {
                "recorded_at_unix_ms": 1,
                "kind": "action",
                "status": "completed",
                "summary": "JOURNAL reflection",
            }
        ],
        "projection_sha256": "b" * 64,
    }
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


class CandidatePresentationTests(unittest.TestCase):
    def run_view(self, script: str, view: str) -> dict[str, object]:
        result = subprocess.run(
            [
                sys.executable,
                "-I",
                "-E",
                "-s",
                str(ROOT / script),
                "--candidate-presentation",
                "--input-stdin",
                "--window-minutes",
                "60",
                "--limit",
                "10",
                "--format",
                "json",
            ],
            input=projection(),
            capture_output=True,
            check=True,
            timeout=5,
        )
        value = json.loads(result.stdout)
        self.assertEqual(value["schema"], SCHEMA)
        self.assertEqual(value["view"], view)
        encoded = json.dumps(value)
        self.assertNotIn("\\u001b", encoded)
        return value

    def test_all_three_exact_entrypoints_support_broker_stdin(self) -> None:
        appliance = self.run_view("report_edge_appliance.py", "appliance")
        activity = self.run_view("report_edge_activity.py", "activity")
        glance = self.run_view("astrid_at_a_glance.py", "at_a_glance")
        self.assertTrue(appliance["sections"])
        self.assertTrue(activity["sections"])
        self.assertTrue(glance["sections"])

    def test_candidate_mode_rejects_paths_and_non_json_formats(self) -> None:
        for extra in (("--workspace", "/etc"), ("--format", "text")):
            arguments = [
                sys.executable,
                str(ROOT / "report_edge_appliance.py"),
                "--candidate-presentation",
                "--input-stdin",
                "--window-minutes",
                "60",
                "--limit",
                "10",
            ]
            if extra[0] != "--format":
                arguments.extend(("--format", "json"))
            arguments.extend(extra)
            result = subprocess.run(
                arguments,
                input=projection(),
                capture_output=True,
                check=False,
                timeout=5,
            )
            self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
