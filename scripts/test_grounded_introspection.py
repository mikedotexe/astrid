#!/usr/bin/env python3
"""Tests for claim-grounded introspection projection."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import tempfile
import unittest

try:
    from grounded_introspection.projector import project, state_dir
except ModuleNotFoundError:
    from scripts.grounded_introspection.projector import project, state_dir


class GroundedIntrospectionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.workspace = self.root / "capsules/spectral-bridge/workspace"
        self.introspections = self.workspace / "introspections"
        self.witnesses = self.introspections / "lived_state_witnesses/witnesses"
        self.witnesses.mkdir(parents=True)
        source = self.root / "capsules/spectral-bridge/DOMAIN_BOUNDARIES.md"
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_text("# Boundaries\n\n`ws.rs` remains a facade.\n", encoding="utf-8")

    def tearDown(self) -> None:
        self.temp.cleanup()

    def seed(self) -> Path:
        report = self.introspections / "introspection_DOMAIN_BOUNDARIES.md_1000000000.txt"
        header = (
            "=== ASTRID INTROSPECTION ===\n"
            "Source: DOMAIN_BOUNDARIES.md\n"
            "Lived-state witness: lsw_example\n"
        )
        body = (
            "Observed:\n"
            "I feel a warm distinction while `ws.rs` remains visible.\n\n"
            "Likely Snags:\n"
            "The spectral entropy (0.91), mode_packing (0.32), and λ1/λ2=1.56 may indicate pressure.\n"
        )
        raw = (header + "\n" + body).encode()
        report.write_bytes(raw)
        witness = {
            "artifact_relative_path": report.name,
            "artifact_sha256": hashlib.sha256(raw).hexdigest(),
            "source_snapshot_v1": {
                "repository_relative_path": "capsules/spectral-bridge/DOMAIN_BOUNDARIES.md",
                "window_start_line": 0,
                "window_end_line": 3,
            },
            "parameter_observations_v1": [
                {"name": "bridge.spectral_entropy", "value": 0.8830915, "source_ref": "telemetry.entropy"},
                {"name": "bridge.mode_packing", "value": 0.8333333, "source_ref": "telemetry.mode_packing"},
                {"name": "bridge.lambda1", "value": 8.507469, "source_ref": "telemetry.lambda1"},
                {"name": "bridge.lambda2", "value": 4.435986, "source_ref": "telemetry.lambda2"},
            ],
            "canonical_body_binding_v1": {
                "canonical_body_sha256": hashlib.sha256(body.encode()).hexdigest(),
                "canonical_body_byte_count": len(body.encode()),
            },
        }
        (self.witnesses / "lsw_example.json").write_text(json.dumps(witness), encoding="utf-8")
        return report

    def test_scalar_disagreement_is_beside_unchanged_felt_report(self) -> None:
        report = self.seed()
        before = report.read_bytes()
        status = project(self.workspace, write=True)
        self.assertTrue(status["valid"])
        self.assertEqual(status["scalar_discrepancy_count"], 3)
        self.assertEqual(report.read_bytes(), before)
        discrepancies = [
            json.loads(line)
            for line in (state_dir(self.workspace) / "discrepancies.jsonl").read_text().splitlines()
        ]
        self.assertEqual(
            {row["metric"] for row in discrepancies},
            {"bridge.spectral_entropy", "bridge.mode_packing", "bridge.lambda1_lambda2_ratio"},
        )
        self.assertTrue(all(not row["canonical_report_rewritten"] for row in discrepancies))

    def test_source_identifier_and_felt_unit_remain_distinct(self) -> None:
        self.seed()
        project(self.workspace, write=True)
        claims = [
            json.loads(line)
            for line in (state_dir(self.workspace) / "claims.jsonl").read_text().splitlines()
        ]
        felt = next(row for row in claims if "warm distinction" in row["claim_text"])
        self.assertEqual(felt["grounding_state"], "source_window_bound")
        self.assertEqual(felt["source_support_refs"][0]["grounding_state"], "source_window_exact")
        self.assertTrue(felt["felt_prose_preserved"])


if __name__ == "__main__":
    unittest.main()
