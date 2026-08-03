#!/usr/bin/env python3
"""Regression tests for bounded CPU-edge session retirement."""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("retire_edge_autonomy_session.py")
SPEC = importlib.util.spec_from_file_location("retire_edge_autonomy_session", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
RETIRE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RETIRE)


def initial_state() -> dict[str, object]:
    return {
        "schema": "astrid_edge_autonomy_state_v3",
        "attempts_today": 1,
        "authored_turns_today": 0,
        "transport_recoveries_today": 0,
        "total_attempts": 259,
        "total_authored_turns": 242,
        "total_transport_recoveries": 15,
        "consecutive_failures": 8,
        "ordinary_session_generation": 255,
        "ordinary_session_authored_turns": 0,
        "chain_session_generation": 1,
        "chain_session_authored_turns": 0,
        "active_chain_id": None,
        "active_chain_step": 0,
        "last_session_name": "edge-autonomous-g255",
        "last_trace_id": "ec240c6c-bd93-4b82-92ac-d3bbc3eaddb3",
        "last_status": "waiting_for_salient_machine_observation",
        "last_started_at_unix_ms": 100,
        "last_completed_at_unix_ms": 200,
        "next_due_at_unix_ms": 300,
        "run_receipt_pending": False,
        "chain_receipt_pending": False,
        "action_dispatch_pending": False,
    }


class RetirementTests(unittest.TestCase):
    def workspace(self, root: str, state: dict[str, object]) -> Path:
        workspace = Path(root) / "edge"
        autonomous = workspace / "autonomous"
        autonomous.mkdir(parents=True)
        state_path = autonomous / "state.json"
        state_path.write_text(json.dumps(state), encoding="utf-8")
        os.chmod(state_path, 0o600)
        return workspace

    def test_retires_only_generation_and_writes_two_phase_receipt(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            before = initial_state()
            workspace = self.workspace(temporary, before)
            result = RETIRE.retire(
                workspace,
                expected_generation=255,
                expected_session_name="edge-autonomous-g255",
                expected_trace_id="ec240c6c-bd93-4b82-92ac-d3bbc3eaddb3",
                reason="fail-closed provenance upgrade",
                dry_run=False,
                require_inactive_service=False,
            )
            after = json.loads((workspace / "autonomous/state.json").read_text())
            self.assertEqual(after["ordinary_session_generation"], 256)
            for key, value in before.items():
                if key != "ordinary_session_generation":
                    self.assertEqual(after[key], value)
            receipts = [
                json.loads(line)
                for line in (
                    workspace / "autonomous/session_retirements.jsonl"
                ).read_text().splitlines()
            ]
            self.assertEqual([item["phase"] for item in receipts], ["requested", "completed"])
            self.assertEqual(receipts[0]["transition_id"], receipts[1]["transition_id"])
            self.assertEqual(result["phase"], "completed")
            self.assertEqual(
                (workspace / "autonomous/state.json").stat().st_mode & 0o777,
                0o600,
            )

    def test_dry_run_changes_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = self.workspace(temporary, initial_state())
            before = (workspace / "autonomous/state.json").read_bytes()
            result = RETIRE.retire(
                workspace,
                expected_generation=255,
                expected_session_name="edge-autonomous-g255",
                expected_trace_id="ec240c6c-bd93-4b82-92ac-d3bbc3eaddb3",
                reason="audit",
                dry_run=True,
                require_inactive_service=False,
            )
            self.assertEqual(result["phase"], "dry_run")
            self.assertEqual((workspace / "autonomous/state.json").read_bytes(), before)
            self.assertFalse((workspace / "autonomous/session_retirements.jsonl").exists())

    def test_completed_retirement_is_idempotent(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = self.workspace(temporary, initial_state())
            arguments = {
                "expected_generation": 255,
                "expected_session_name": "edge-autonomous-g255",
                "expected_trace_id": "ec240c6c-bd93-4b82-92ac-d3bbc3eaddb3",
                "reason": "audit",
                "dry_run": False,
                "require_inactive_service": False,
            }
            RETIRE.retire(workspace, **arguments)
            ledger = workspace / "autonomous/session_retirements.jsonl"
            before = ledger.read_bytes()
            replay = RETIRE.retire(workspace, **arguments)
            self.assertTrue(replay["idempotent_replay"])
            self.assertEqual(ledger.read_bytes(), before)

    def test_requested_receipt_recovers_completion_after_state_replace(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = self.workspace(temporary, initial_state())
            RETIRE.retire(
                workspace,
                expected_generation=255,
                expected_session_name="edge-autonomous-g255",
                expected_trace_id="ec240c6c-bd93-4b82-92ac-d3bbc3eaddb3",
                reason="audit",
                dry_run=False,
                require_inactive_service=False,
            )
            ledger = workspace / "autonomous/session_retirements.jsonl"
            requested = ledger.read_text().splitlines()[0]
            ledger.write_text(requested + "\n", encoding="utf-8")
            os.chmod(ledger, 0o600)
            recovered = RETIRE.retire(
                workspace,
                expected_generation=255,
                expected_session_name="edge-autonomous-g255",
                expected_trace_id="ec240c6c-bd93-4b82-92ac-d3bbc3eaddb3",
                reason="audit",
                dry_run=False,
                require_inactive_service=False,
            )
            self.assertTrue(recovered["recovered_after_interrupted_receipt"])
            self.assertEqual(
                [json.loads(line)["phase"] for line in ledger.read_text().splitlines()],
                ["requested", "completed"],
            )

    def test_refuses_stale_or_pending_state(self) -> None:
        for field, value in (
            ("ordinary_session_generation", 256),
            ("ordinary_session_authored_turns", 1),
            ("action_dispatch_pending", True),
            ("active_chain_id", "chain-1"),
        ):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as temporary:
                state = initial_state()
                state[field] = value
                workspace = self.workspace(temporary, state)
                with self.assertRaises(RuntimeError):
                    RETIRE.retire(
                        workspace,
                        expected_generation=255,
                        expected_session_name="edge-autonomous-g255",
                        expected_trace_id="ec240c6c-bd93-4b82-92ac-d3bbc3eaddb3",
                        reason="audit",
                        dry_run=True,
                        require_inactive_service=False,
                    )


if __name__ == "__main__":
    unittest.main()
