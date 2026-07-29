#!/usr/bin/env python3
"""Tests for the Living Problem Registry projector."""

from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

try:
    from living_problem_registry.projector import project, state_dir
    from living_problem_registry.v2 import state_dir_v2
except ModuleNotFoundError:
    from scripts.living_problem_registry.projector import project, state_dir
    from scripts.living_problem_registry.v2 import state_dir_v2


AUTHORITY = {
    "schema": "artifact_authority_state_v1",
    "schema_version": 1,
    "state": "evidence_only",
    "witness_only": True,
}


class LivingProblemRegistryTests(unittest.TestCase):
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
        path.write_text(
            "".join(json.dumps(row) + "\n" for row in rows),
            encoding="utf-8",
        )

    @staticmethod
    def contract(
        suffix: str,
        *,
        activity: str = "open",
        felt_review: str = "not_requested",
        felt_closed: bool = False,
        reopen_count: int = 0,
        technical: dict[str, int] | None = None,
    ) -> dict[str, object]:
        claim = f"introspection_example_{suffix}:c001"
        return {
            "schema": "felt_contract_projection_v1",
            "schema_version": 1,
            "contract_id": f"contract_{suffix}",
            "anchor_claim_id": claim,
            "claim_ids": [claim],
            "claim_count": 1,
            "activity": activity,
            "felt_review": felt_review,
            "felt_closed": felt_closed,
            "administrative_terminal": activity == "administratively_terminal",
            "reopen_count": reopen_count,
            "technical_state_counts": technical or {},
            "evidence_state_counts": {},
            "last_change_at": "2026-07-28T00:00:00+00:00",
            "artifact_authority_state_v1": AUTHORITY,
        }

    def seed(self) -> None:
        contracts = [
            self.contract("1000000001"),
            self.contract(
                "1000000002",
                activity="administratively_terminal",
                technical={"verified": 1},
            ),
            self.contract(
                "1000000003",
                activity="felt_closed",
                felt_review="felt_confirmed",
                felt_closed=True,
            ),
            self.contract(
                "1000000004",
                activity="reopened",
                felt_review="objection",
                reopen_count=1,
            ),
        ]
        self.write_jsonl("felt_contract_graph_v1/contracts.jsonl", contracts)
        self.write_json(
            "introspection_addressing_v1/status.json",
            {
                "artifacts": {
                    "introspection_example_1000000001": {
                        "status": "unread",
                        "full_read": False,
                    },
                    "introspection_example_1000000002": {
                        "status": "addressed_change",
                        "full_read": True,
                    },
                    "introspection_example_1000000003": {
                        "status": "addressed_change",
                        "full_read": True,
                    },
                    "introspection_example_1000000004": {
                        "status": "addressed_change",
                        "full_read": True,
                    },
                },
                "work_items": {
                    "wi_verified": {
                        "source_introspection_id": "introspection_example_1000000002",
                        "claim_id": "c001",
                        "status": "verified_existing",
                    },
                    "wi_closed": {
                        "source_introspection_id": "introspection_example_1000000003",
                        "claim_id": "c001",
                        "status": "closed_felt_confirmed",
                    },
                },
            },
        )
        self.write_json(
            "sandbox_trial_queue_v1/status.json",
            {"trials": {}},
        )
        self.write_json(
            "steward_work_selection_v1/selection.json",
            {"selected_entries": [{"contract_id": "contract_1000000001"}]},
        )

    def test_one_problem_per_contract_and_contract_local_closure(self) -> None:
        self.seed()
        status = project(self.workspace, write=True)
        self.assertTrue(status["valid"])
        self.assertEqual(status["problem_count"], 4)
        rows = {
            row["problem_id"]: row
            for row in (
                json.loads(line)
                for line in (
                    state_dir(self.workspace) / "problems.jsonl"
                ).read_text(encoding="utf-8").splitlines()
            )
        }
        self.assertEqual(
            rows["contract_1000000001"]["current_wait"],
            "source_read",
        )
        self.assertTrue(
            rows["contract_1000000001"]["selected_for_steward_work"]
        )
        self.assertEqual(
            rows["contract_1000000002"]["current_wait"],
            "being_review",
        )
        self.assertEqual(
            rows["contract_1000000003"]["current_wait"],
            "closed",
        )
        self.assertEqual(
            rows["contract_1000000004"]["current_wait"],
            "claim_disposition",
        )
        self.assertFalse(rows["contract_1000000004"]["felt_closed"])
        rows_v2 = {
            row["problem_id"]: row
            for row in (
                json.loads(line)
                for line in (
                    state_dir_v2(self.workspace) / "problems.jsonl"
                ).read_text(encoding="utf-8").splitlines()
            )
        }
        self.assertEqual(
            rows_v2["contract_1000000001"]["current_wait"],
            "source_read",
        )
        self.assertEqual(
            rows_v2["contract_1000000003"]["current_wait"],
            "closure",
        )
        self.assertEqual(
            rows_v2["contract_1000000003"]["lifecycle_state"],
            "closed",
        )
        self.assertFalse(
            rows_v2["contract_1000000001"]["authority"][
                "registry_schedules_work"
            ]
        )
        self.assertFalse(
            rows_v2["contract_1000000001"]["authority"][
                "registry_grants_authority"
            ]
        )

    def test_second_identical_projection_has_no_changed_packets(self) -> None:
        self.seed()
        project(self.workspace, write=True)
        status = project(self.workspace, write=True)
        self.assertEqual(status["changed_problem_count"], 0)
        self.assertEqual(
            (state_dir(self.workspace) / "changed_problems.jsonl").read_text(),
            "",
        )
        self.assertEqual(
            (
                state_dir_v2(self.workspace) / "changed_problems.jsonl"
            ).read_text(),
            "",
        )


if __name__ == "__main__":
    unittest.main()
