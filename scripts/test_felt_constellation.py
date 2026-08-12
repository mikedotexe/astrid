#!/usr/bin/env python3
"""Tests for the read-only Felt Constellation projector."""

from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

try:
    from felt_constellation.projector import project, state_dir
except ModuleNotFoundError:
    from scripts.felt_constellation.projector import project, state_dir


class FeltConstellationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.workspace = Path(self.temp.name)
        self.diagnostics = self.workspace / "diagnostics"

    def tearDown(self) -> None:
        self.temp.cleanup()

    def write_json(self, relative: str, value: object) -> None:
        path = self.diagnostics / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value) + "\n", encoding="utf-8")

    def write_jsonl(self, relative: str, rows: list[dict[str, object]]) -> None:
        path = self.diagnostics / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")

    def seed(self) -> None:
        claims = {
            "intro_one:c001": {
                "canonical_claim_id": "intro_one:c001",
                "text": "The codec projection needs exact source provenance and an offline test.",
                "classification": "study_preregistered",
                "source_family": "codec",
            },
            "intro_two:c001": {
                "canonical_claim_id": "intro_two:c001",
                "text": "A singular unfamiliar statement remains visible.",
                "classification": "primary_qualitative_evidence",
                "source_family": "other",
            },
        }
        self.write_json(
            "claim_families_v1/status.json",
            {
                "families": {
                    "family_one": {"target_surface": "codec", "claims": {"intro_one:c001": claims["intro_one:c001"]}},
                    "family_two": {"target_surface": "other", "claims": {"intro_two:c001": claims["intro_two:c001"]}},
                }
            },
        )
        contracts = []
        for index, claim_id in enumerate(claims, 1):
            contracts.append(
                {
                    "contract_id": f"contract_{index}",
                    "claim_ids": [claim_id],
                    "felt_closed": False,
                    "felt_review": "still_friction" if index == 1 else "not_requested",
                    "contradiction_count": 0,
                    "reopen_count": 1 if index == 1 else 0,
                    "evidence_state_counts": {"partial": 1},
                    "technical_state_counts": {"unassessed": 1},
                }
            )
        self.write_jsonl("felt_contract_graph_v1/contracts.jsonl", contracts)
        self.write_jsonl(
            "living_problem_registry_v2/problems.jsonl",
            [
                {"contract_id": "contract_1", "current_wait": "replay"},
                {"contract_id": "contract_2", "current_wait": "disposition"},
            ],
        )

    def test_exact_claims_remain_visible_with_multi_membership(self) -> None:
        self.seed()
        status = project(self.workspace, write=True)
        self.assertTrue(status["valid"])
        self.assertEqual(status["canonical_claim_count"], 2)
        self.assertEqual(status["unmatched_claim_count"], 1)
        memberships = [
            json.loads(line)
            for line in (state_dir(self.workspace) / "memberships.jsonl").read_text().splitlines()
        ]
        first = next(row for row in memberships if row["canonical_claim_id"] == "intro_one:c001")
        self.assertIn("representation_codec_and_compression", first["constellation_ids"])
        self.assertIn("source_provenance_and_grounding", first["constellation_ids"])
        self.assertIn("study_observation_and_verification", first["constellation_ids"])
        self.assertEqual(first["artifact_authority_state_v1"]["state"], "evidence_only")

    def test_second_projection_is_byte_stable(self) -> None:
        self.seed()
        project(self.workspace, write=True)
        before = (state_dir(self.workspace) / "constellations.jsonl").read_bytes()
        project(self.workspace, write=True)
        self.assertEqual(before, (state_dir(self.workspace) / "constellations.jsonl").read_bytes())


if __name__ == "__main__":
    unittest.main()
