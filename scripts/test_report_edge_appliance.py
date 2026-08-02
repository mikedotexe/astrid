#!/usr/bin/env python3
"""Regression tests for the CPU-edge appliance report."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
from pathlib import Path
import unittest


SCRIPT = Path(__file__).with_name("report_edge_appliance.py")
SPEC = importlib.util.spec_from_file_location("report_edge_appliance", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
REPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPORT)


class FillSummaryTests(unittest.TestCase):
    def test_observation_only_authority_overrides_measured_profile(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            config = home / ".config/astrid"
            config.mkdir(parents=True)
            (config / "edge-appliance.env").write_text(
                "ASTRID_EDGE_RESERVOIR_TUNING_ENABLED=true\n",
                encoding="utf-8",
            )
            (config / "edge-tuning-authority.env").write_text(
                "ASTRID_EDGE_RESERVOIR_TUNING_ENABLED=false\n",
                encoding="utf-8",
            )
            values = REPORT.effective_profile_values(home)
            self.assertEqual(
                values["ASTRID_EDGE_RESERVOIR_TUNING_ENABLED"], "false"
            )

    def test_loaded_capsule_contract_requires_exact_count_and_essentials(self) -> None:
        essential = sorted(REPORT.ESSENTIAL_EDGE_CAPSULES)
        loaded = essential + [f"base-{index:02d}" for index in range(10)]
        raw = json.dumps({"status": {"loaded_capsules": loaded}})
        parsed = REPORT.loaded_capsules_from_status(raw)
        self.assertEqual(parsed, loaded)
        assert parsed is not None
        self.assertTrue(REPORT.loaded_capsule_contract(parsed))

        missing_essential = loaded.copy()
        missing_essential[0] = "extra-base"
        self.assertFalse(REPORT.loaded_capsule_contract(missing_essential))
        self.assertIsNone(
            REPORT.loaded_capsules_from_status(
                json.dumps(
                    {"status": {"loaded_capsules": loaded[:-1] + [loaded[0]]}}
                )
            )
        )

    def test_signed_tuning_state_uses_payload_and_active_focus(self) -> None:
        envelope = {
            "schema": "astrid_edge_tuning_state_v1",
            "payload": {
                "active_experiment": {
                    "experiment_id": "tuning-1",
                    "candidate_id": "candidate-1",
                    "phase": "trial",
                    "spec": {"parameter": "input_gain", "value": 1.05},
                }
            },
            "payload_sha256": "a" * 64,
            "signing_public_key": "b" * 64,
            "signature": "c" * 128,
        }
        state = REPORT.tuning_state_view(envelope)
        status, focus = REPORT.tuning_state_focus(state)
        self.assertEqual(status, "active_trial")
        self.assertEqual(focus["experiment_id"], "tuning-1")

    def test_reports_both_acceptance_shelves(self) -> None:
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            REPORT.summarize_fill("fill", [64.0, 65.0, 72.0, 73.5, 74.0])

        fields = dict(
            line.split("=", 1)
            for line in output.getvalue().splitlines()
            if "=" in line
        )
        self.assertEqual(fields["fill_inside_65_72_pct"], "40.0")
        self.assertEqual(fields["fill_inside_65_73_5_pct"], "60.0")

    def test_local_header_latency_uses_exact_origin_and_window(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "log").mkdir()
            (root / "log/astrid.2026-08-02.log").write_text(
                "2026-08-02T03:25:43.347748Z  INFO target: HTTP stream response headers received "
                "capsule_id=astrid-capsule-openai-compat origin=http://127.0.0.1:11434 "
                "elapsed_ms=126708\n"
                "2026-08-02T03:26:43.347748Z  INFO target: HTTP stream response headers received "
                "capsule_id=astrid-capsule-http origin=https://example.com:443 elapsed_ms=4\n"
            )
            cutoff = int(
                __import__("datetime")
                .datetime.fromisoformat("2026-08-02T03:25:00+00:00")
                .timestamp()
                * 1_000
            )
            self.assertEqual(
                REPORT.local_provider_header_latencies(root, cutoff), [126708]
            )


if __name__ == "__main__":
    unittest.main()
