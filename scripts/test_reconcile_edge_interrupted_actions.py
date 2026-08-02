#!/usr/bin/env python3
"""Tests for interrupted autonomous Action reconciliation."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("reconcile_edge_interrupted_actions.py")
SPEC = importlib.util.spec_from_file_location("reconcile_interrupted", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class InterruptedActionReconciliationTests(unittest.TestCase):
    def test_quarantines_artifact_and_removes_false_authored_continuity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workspace = root / "edge"
            operator = root / "operator"
            for relative in ("actions", "autonomous", "journal"):
                (workspace / relative).mkdir(parents=True, exist_ok=True)
            trace = {
                "schema_version": 1,
                "trace_id": "00000000-0000-4000-8000-000000000001",
                "span_id": "00000000-0000-4000-8000-000000000002",
                "chain_id": "chain-1",
            }
            artifact = workspace / "journal/journal_200.md"
            artifact.write_text("not valid authored continuity\n")
            (workspace / "autonomous/recoveries.jsonl").write_text(
                json.dumps(
                    {
                        "status": "interrupted",
                        "completed_at_unix_ms": 150,
                        "trace": trace,
                    }
                )
                + "\n"
            )
            (workspace / "actions/receipts.jsonl").write_text(
                json.dumps(
                    {
                        "recorded_at_unix_ms": 200,
                        "response_sha256": "a" * 64,
                        "declared_next": "JOURNAL invalid claim",
                        "status": "executed",
                        "artifact_path": "home://edge/journal/journal_200.md",
                        "trace": trace,
                    }
                )
                + "\n"
            )
            (workspace / "autonomous/runs.jsonl").write_text("")
            thread = {
                "schema": "astrid_edge_thread_state_v5",
                "revision": 2,
                "authored_claims": ["older", "invalid claim", "invalid claim"],
                "provenance_hashes": ["artifact-hash", "older-hash"],
                "evidence_records": [
                    {
                        "reference": "journal_200.md",
                        "captured_at_unix_ms": 200,
                        "sha256": "artifact-hash",
                    }
                ],
                "next_options": ["JOURNAL invalid claim"],
                "response_sha256": "b" * 64,
            }
            (workspace / "autonomous/thread_state.json").write_text(json.dumps(thread))
            (workspace / "autonomous/thread_state.jsonl").write_text("")
            (workspace / "autonomous/state.json").write_text(
                json.dumps({"active_chain_id": "chain-1", "active_chain_step": 3})
            )

            result = MODULE.apply(workspace, operator)

            self.assertEqual(result["detected"], 1)
            self.assertFalse(artifact.exists())
            self.assertTrue(Path(result["corrections"][0]["quarantined_artifact"]).exists())
            corrected_thread = MODULE.read_json(
                workspace / "autonomous/thread_state.json"
            )
            self.assertEqual(
                corrected_thread["authored_claims"], ["older", "invalid claim"]
            )
            self.assertEqual(corrected_thread["evidence_records"], [])
            self.assertEqual(corrected_thread["provenance_hashes"], ["older-hash"])
            self.assertEqual(corrected_thread["next_options"], [])
            self.assertEqual(
                MODULE.read_json(workspace / "autonomous/state.json")[
                    "active_chain_step"
                ],
                2,
            )
            self.assertEqual(MODULE.apply(workspace, operator)["detected"], 0)


if __name__ == "__main__":
    unittest.main()
