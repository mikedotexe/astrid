#!/usr/bin/env python3
"""Tests for interrupted autonomous Action reconciliation."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).with_name("reconcile_edge_interrupted_actions.py")
SPEC = importlib.util.spec_from_file_location("reconcile_interrupted", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class InterruptedActionReconciliationTests(unittest.TestCase):
    @staticmethod
    def seed_candidate(workspace: Path, artifact_path: str) -> None:
        for relative in ("actions", "autonomous"):
            (workspace / relative).mkdir(parents=True, exist_ok=True)
        trace = {
            "schema_version": 1,
            "trace_id": "00000000-0000-4000-8000-000000000401",
            "span_id": "00000000-0000-4000-8000-000000000402",
            "turn_id": "00000000-0000-4000-8000-000000000403",
            "session_id": "session-confinement",
        }
        (workspace / "autonomous/recoveries.jsonl").write_text(
            json.dumps(
                {
                    "status": "interrupted",
                    "completed_at_unix_ms": 100,
                    "trace": trace,
                }
            )
            + "\n"
        )
        (workspace / "actions/receipts.jsonl").write_text(
            json.dumps(
                {
                    "recorded_at_unix_ms": 200,
                    "response_sha256": "d" * 64,
                    "declared_next": "JOURNAL confinement",
                    "status": "executed",
                    "artifact_path": artifact_path,
                    "trace": trace,
                }
            )
            + "\n"
        )
        (workspace / "autonomous/runs.jsonl").write_text("")

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
                "turn_id": "00000000-0000-4000-8000-000000000003",
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
            correction = result["corrections"][0]
            self.assertEqual(correction["schema"], MODULE.SCHEMA)
            self.assertEqual(correction["turn_id"], trace["turn_id"])
            self.assertEqual(correction["trace_id"], trace["trace_id"])

    def test_fsynced_correction_is_reapplied_once_after_reconciliation_crash(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workspace = root / "edge"
            operator = root / "operator"
            for relative in ("actions", "autonomous"):
                (workspace / relative).mkdir(parents=True)
            trace = {
                "trace_id": "00000000-0000-4000-8000-000000000301",
                "span_id": "00000000-0000-4000-8000-000000000302",
                "turn_id": "00000000-0000-4000-8000-000000000303",
                "chain_id": "chain-crash",
            }
            action = {
                "recorded_at_unix_ms": 200,
                "response_sha256": "c" * 64,
                "declared_next": "JOURNAL crash-boundary claim",
                "status": "executed",
                "trace": trace,
            }
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
                json.dumps(action) + "\n"
            )
            (workspace / "autonomous/runs.jsonl").write_text("")
            (workspace / "autonomous/thread_state.json").write_text(
                json.dumps(
                    {
                        "revision": 4,
                        "authored_claims": ["older", "crash-boundary claim"],
                        "evidence_records": [],
                        "provenance_hashes": [],
                        "next_options": ["JOURNAL crash-boundary claim"],
                    }
                )
            )
            (workspace / "autonomous/thread_state.jsonl").write_text("")
            (workspace / "autonomous/state.json").write_text(
                json.dumps(
                    {
                        "active_chain_id": "chain-crash",
                        "active_chain_step": 3,
                    }
                )
            )

            with mock.patch.object(
                MODULE,
                "reconcile_thread",
                side_effect=RuntimeError("simulated crash after correction fsync"),
            ):
                with self.assertRaisesRegex(RuntimeError, "simulated crash"):
                    MODULE.apply(workspace, operator)

            correction_path = workspace / "actions/interrupted_corrections.jsonl"
            self.assertEqual(len(MODULE.read_jsonl(correction_path)), 1)
            self.assertEqual(
                MODULE.read_json(workspace / "autonomous/thread_state.json")[
                    "revision"
                ],
                4,
            )
            self.assertEqual(
                MODULE.read_json(workspace / "autonomous/state.json")[
                    "active_chain_step"
                ],
                3,
            )

            original_append = MODULE.append_owner_jsonl

            def fail_thread_history(path: Path, value: dict[str, object]) -> None:
                if path.name == "thread_state.jsonl":
                    raise RuntimeError("simulated crash before thread history fsync")
                original_append(path, value)

            with mock.patch.object(
                MODULE, "append_owner_jsonl", side_effect=fail_thread_history
            ):
                with self.assertRaisesRegex(RuntimeError, "thread history"):
                    MODULE.apply(workspace, operator)
            self.assertEqual(
                MODULE.read_json(workspace / "autonomous/thread_state.json")[
                    "revision"
                ],
                5,
            )
            self.assertEqual(
                MODULE.read_json(workspace / "autonomous/state.json")[
                    "active_chain_step"
                ],
                3,
            )

            recovered = MODULE.apply(workspace, operator)
            self.assertEqual(recovered["detected"], 0)
            self.assertEqual(recovered["existing_exact_corrections"], 1)
            self.assertTrue(recovered["thread_reconciled"])
            self.assertTrue(recovered["autonomy_state_reconciled"])
            thread = MODULE.read_json(workspace / "autonomous/thread_state.json")
            state = MODULE.read_json(workspace / "autonomous/state.json")
            self.assertEqual(thread["revision"], 5)
            self.assertEqual(thread["authored_claims"], ["older"])
            self.assertEqual(state["active_chain_step"], 2)
            self.assertEqual(len(MODULE.read_jsonl(correction_path)), 1)
            history_path = workspace / "autonomous/thread_state.jsonl"
            self.assertEqual(len(MODULE.read_jsonl(history_path)), 1)
            self.assertIn(
                MODULE.causal_response_key(action),
                MODULE.completed_reconciliation_keys(operator),
            )

            # Runtime serialization may later omit the operator-only markers.
            # The fsynced completion manifest remains the durable idempotency
            # authority and prevents another decrement or interpretation edit.
            thread.pop("reconciled_interrupted_responses")
            (workspace / "autonomous/thread_state.json").write_text(
                json.dumps(thread)
            )
            state.pop("reconciled_interrupted_responses")
            (workspace / "autonomous/state.json").write_text(json.dumps(state))
            repaired = MODULE.apply(workspace, operator)
            self.assertEqual(repaired["pending_reconciliation"], 0)
            self.assertNotIn("thread_reconciled", repaired)
            self.assertNotIn("autonomy_state_reconciled", repaired)
            self.assertEqual(
                MODULE.read_json(workspace / "autonomous/thread_state.json")[
                    "revision"
                ],
                5,
            )
            self.assertEqual(len(MODULE.read_jsonl(history_path)), 1)
            self.assertEqual(
                MODULE.read_json(workspace / "autonomous/state.json")[
                    "active_chain_step"
                ],
                2,
            )

            again = MODULE.apply(workspace, operator)
            self.assertEqual(again["pending_reconciliation"], 0)
            self.assertEqual(
                MODULE.read_json(workspace / "autonomous/thread_state.json")[
                    "revision"
                ],
                5,
            )
            self.assertEqual(
                MODULE.read_json(workspace / "autonomous/state.json")[
                    "active_chain_step"
                ],
                2,
            )
            self.assertEqual(len(MODULE.read_jsonl(history_path)), 1)
            self.assertEqual(len(MODULE.read_jsonl(correction_path)), 1)

    def test_private_persistence_helpers_reject_symlinks_and_sync_parents(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            outside = root / "outside.json"
            outside.write_text("outside\n")
            append_path = root / "private/ledger.jsonl"
            append_path.parent.mkdir()
            append_path.symlink_to(outside)
            with self.assertRaisesRegex(RuntimeError, "non-regular"):
                MODULE.append_owner_jsonl(append_path, {"value": 1})
            self.assertEqual(outside.read_text(), "outside\n")

            atomic_path = root / "private/state.json"
            atomic_path.symlink_to(outside)
            with self.assertRaisesRegex(RuntimeError, "non-regular"):
                MODULE.atomic_json(atomic_path, {"value": 1})
            self.assertEqual(outside.read_text(), "outside\n")

            append_path.unlink()
            atomic_path.unlink()
            append_path.write_bytes(b'{"partial":')
            with self.assertRaisesRegex(RuntimeError, "partial JSONL"):
                MODULE.append_owner_jsonl(append_path, {"value": 1})
            self.assertEqual(append_path.read_bytes(), b'{"partial":')
            append_path.unlink()
            with mock.patch.object(MODULE, "fsync_directory") as sync:
                MODULE.append_owner_jsonl(append_path, {"value": 1})
                MODULE.atomic_json(atomic_path, {"value": 1})
            self.assertEqual(
                sync.call_args_list,
                [
                    mock.call(append_path.parent.resolve()),
                    mock.call(atomic_path.parent.resolve()),
                ],
            )

            redirected = root / "redirected"
            redirected.mkdir()
            linked_parent = root / "linked-parent"
            linked_parent.symlink_to(redirected, target_is_directory=True)
            with self.assertRaisesRegex(RuntimeError, "symlink root"):
                MODULE.append_owner_jsonl(
                    linked_parent / "ledger.jsonl", {"value": 2}
                )
            with self.assertRaisesRegex(RuntimeError, "symlink root"):
                MODULE.atomic_json(linked_parent / "state.json", {"value": 2})
            self.assertEqual(list(redirected.iterdir()), [])

    def test_only_complete_authoritative_v2_corrections_are_replayable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            (workspace / "actions").mkdir()
            correction = {
                "schema": MODULE.SCHEMA,
                "trace_id": "00000000-0000-4000-8000-000000000401",
                "turn_id": "00000000-0000-4000-8000-000000000402",
                "response_sha256": "d" * 64,
                "corrected_status": "revoked_interrupted_trace_non_authored",
                "authority": (
                    "operator_reconciliation_non_authored_no_action_authority"
                ),
            }
            ledger = workspace / "actions/interrupted_corrections.jsonl"
            ledger.write_text(json.dumps(correction))
            self.assertEqual(MODULE.exact_correction_keys(workspace), set())

            ledger.write_text(json.dumps(correction) + "\n")
            self.assertEqual(
                MODULE.exact_correction_keys(workspace),
                {MODULE.causal_response_key(correction)},
            )
            correction.pop("corrected_status")
            ledger.write_text(json.dumps(correction) + "\n")
            self.assertEqual(MODULE.exact_correction_keys(workspace), set())

    def test_artifact_symlink_ancestor_cannot_move_external_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workspace = root / "edge"
            outside = root / "outside"
            outside.mkdir()
            external_file = outside / "journal_200.md"
            external_file.write_text("external must remain\n")
            self.seed_candidate(
                workspace, "home://edge/journal/journal_200.md"
            )
            (workspace / "journal").symlink_to(outside, target_is_directory=True)

            result = MODULE.apply(workspace, root / "operator")

            self.assertEqual(result["detected"], 1)
            self.assertEqual(external_file.read_text(), "external must remain\n")
            self.assertIsNone(result["corrections"][0]["quarantined_artifact"])
            self.assertIsNone(result["corrections"][0]["artifact_sha256"])

    def test_quarantine_symlink_escape_aborts_before_any_move(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workspace = root / "edge"
            artifact = workspace / "journal/journal_200.md"
            artifact.parent.mkdir(parents=True)
            artifact.write_text("owned artifact\n")
            self.seed_candidate(
                workspace, "home://edge/journal/journal_200.md"
            )
            operator = root / "operator"
            operator.mkdir()
            outside = root / "outside-quarantine"
            outside.mkdir()
            (operator / "interrupted-actions").symlink_to(
                outside, target_is_directory=True
            )

            with self.assertRaisesRegex(RuntimeError, "unsafe quarantine component"):
                MODULE.apply(workspace, operator)

            self.assertEqual(artifact.read_text(), "owned artifact\n")
            self.assertEqual(list(outside.iterdir()), [])
            self.assertFalse(
                (workspace / "actions/interrupted_corrections.jsonl").exists()
            )

    def test_repeated_response_text_on_another_trace_is_never_corrected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            for relative in ("actions", "autonomous"):
                (workspace / relative).mkdir(parents=True)

            response_hash = "a" * 64
            first_trace = {
                "trace_id": "00000000-0000-4000-8000-000000000101",
                "turn_id": "00000000-0000-4000-8000-000000000102",
            }
            second_trace = {
                "trace_id": "00000000-0000-4000-8000-000000000201",
                "turn_id": "00000000-0000-4000-8000-000000000202",
            }
            (workspace / "autonomous/recoveries.jsonl").write_text(
                json.dumps(
                    {
                        "status": "interrupted",
                        "completed_at_unix_ms": 100,
                        "trace": first_trace,
                    }
                )
                + "\n"
            )
            actions = [
                {
                    "recorded_at_unix_ms": 200,
                    "response_sha256": response_hash,
                    "trace": first_trace,
                },
                {
                    "recorded_at_unix_ms": 200,
                    "response_sha256": response_hash,
                    "trace": second_trace,
                },
            ]
            (workspace / "actions/receipts.jsonl").write_text(
                "".join(json.dumps(action) + "\n" for action in actions)
            )
            (workspace / "autonomous/runs.jsonl").write_text("")
            # A v1 record is retained history, not an attribution authority.
            (workspace / "actions/interrupted_corrections.jsonl").write_text(
                json.dumps(
                    {
                        "schema": "astrid_edge_interrupted_action_correction_v1",
                        "trace_id": first_trace["trace_id"],
                        "response_sha256": response_hash,
                    }
                )
                + "\n"
            )

            detected = MODULE.interrupted_actions(workspace)
            self.assertEqual(len(detected), 1)
            self.assertEqual(MODULE.trace_id(detected[0]), first_trace["trace_id"])


if __name__ == "__main__":
    unittest.main()
