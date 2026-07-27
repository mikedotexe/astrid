#!/usr/bin/env python3

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from division_ceremony_chronicle import (
    ChronicleError,
    build_projection,
    project,
    verify_files,
    verify_payload,
)


def native_status() -> dict:
    return {
        "schema": "division.status.v1",
        "division_id": "divide-one",
        "parent_generation": 7,
        "plan_digest": "b" * 64,
        "lifecycle": "shadowing",
        "parent_authoritative": True,
        "commit_feature_enabled": False,
        "rehearsal_dispatch_enabled": False,
        "selected_strategy": None,
        "astrid_assent": False,
        "minime_assent": False,
        "bridge_scale": 1.0,
        "current_tick": 12,
        "rollback_deadline_tick": None,
        "snapshot_refs": ["sha256:parent"],
        "readiness": {
            "policy": "division.readiness.v1",
            "ready": False,
            "sample_count": 12,
            "blocking_reasons": ["shadow_window_incomplete"],
            "metrics_fresh": True,
            "sensory_panic_streak": 0,
            "actuator_saturation_streak": 0,
        },
        "visual_evidence_advisory_only": True,
        "candidates": [
            {
                "strategy": "input_recurrence",
                "minime_role": "more_input_driven",
                "astrid_role": "more_recurrence_driven",
                "covariance_partition_loss": 0.1,
                "sensory_fields": {
                    "inheritance": "independent_clones",
                    "dimension": 512,
                    "minime_fill_pct": 68.0,
                    "astrid_fill_pct": 67.5,
                    "minime_ticks": 12,
                    "astrid_ticks": 12,
                },
                "readiness": {
                    "policy": "division.readiness.v1",
                    "ready": False,
                    "sample_count": 12,
                    "blocking_reasons": ["shadow_window_incomplete"],
                    "state_nrmse": 0.12,
                    "state_cosine": 0.91,
                    "readout_nrmse": 0.08,
                    "metrics_fresh": True,
                },
            }
        ],
    }


class DivisionCeremonyChronicleTests(unittest.TestCase):
    def test_empty_projection_keeps_source_and_runtime_distinct(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            payload = build_projection(Path(raw))
            self.assertEqual(
                payload["current_native_state"]["fact_class"], "unknown"
            )
            self.assertTrue(
                payload["destination_contract"][
                    "independent_reservoir_state_source_prepared"
                ]
            )
            self.assertFalse(
                payload["destination_contract"][
                    "independent_process_ownership_established"
                ]
            )
            self.assertEqual(payload["timeline"], [])
            verify_payload(payload)

    def test_projection_archives_deterministically_and_owner_only(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            workspace = Path(raw) / "workspace"
            division = workspace / "division"
            division.mkdir(parents=True)
            (division / "status.json").write_text(json.dumps(native_status()))
            output = Path(raw) / "output"

            first, _, _ = project(workspace, output)
            second, _, _ = project(workspace, output)

            self.assertEqual(first["chronicle_id"], second["chronicle_id"])
            self.assertEqual(
                first["phase_space_preservation"]["candidate_count"], 1
            )
            self.assertFalse(
                first["authority"]["visualization_grants_authority"]
            )
            receipt = verify_files(output)
            self.assertTrue(receipt["ok"])
            self.assertEqual(
                (output / "chronicle_v1.json").stat().st_mode & 0o077, 0
            )
            self.assertIn(
                'name="chronicle-mode" content="live"',
                (output / "chronicle_v1.html").read_text(),
            )
            self.assertIn(
                'name="chronicle-mode" content="archive"',
                (
                    output
                    / "archive"
                    / f"{first['chronicle_id']}.html"
                ).read_text(),
            )

    def test_tampering_and_prose_keys_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            payload = build_projection(Path(raw))
            payload["authority"]["commit_recommended"] = True
            with self.assertRaisesRegex(ChronicleError, "identity mismatch"):
                verify_payload(payload)

            payload = build_projection(Path(raw))
            payload["prompt"] = "forbidden"
            payload["chronicle_id"] = "division_chronicle_invalid"
            with self.assertRaises(ChronicleError):
                verify_payload(payload)


if __name__ == "__main__":
    unittest.main()
