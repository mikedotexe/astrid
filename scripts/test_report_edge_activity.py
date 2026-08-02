#!/usr/bin/env python3
"""Tests for the read-only correlated edge activity view."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import report_edge_activity as activity


class EdgeActivityTest(unittest.TestCase):
    TRACE_ONE = "00000000-0000-4000-8000-000000000001"
    TRACE_WEB = "00000000-0000-4000-8000-000000000002"
    SPAN_ROOT = "00000000-0000-4000-8000-000000000011"
    SPAN_ACTION = "00000000-0000-4000-8000-000000000012"
    SPAN_REQUEST = "00000000-0000-4000-8000-000000000013"
    SPAN_RESULT = "00000000-0000-4000-8000-000000000014"
    SPAN_THREAD = "00000000-0000-4000-8000-000000000015"

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.workspace = Path(self.temporary.name)
        for relative in (
            "autonomous",
            "actions",
            "web",
            "introspection",
            "perception",
            "spectral",
            "studies",
            "tuning",
        ):
            (self.workspace / relative).mkdir()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_lines(self, relative: str, values: list[dict[str, object]]) -> None:
        (self.workspace / relative).write_text(
            "".join(json.dumps(value, sort_keys=True) + "\n" for value in values)
        )

    @staticmethod
    def trace(
        trace_id: str,
        span_id: str,
        parent_span_id: str | None = None,
    ) -> dict[str, object]:
        return {
            "schema_version": 1,
            "trace_id": trace_id,
            "span_id": span_id,
            "parent_span_id": parent_span_id,
            "session_id": "session-one",
            "chain_id": "chain-one",
        }

    def test_exact_identifiers_join_legacy_action_but_timestamps_never_join_web(self) -> None:
        self.write_lines(
            "autonomous/runs.jsonl",
            [
                {
                    "completed_at_unix_ms": 100,
                    "status": "authored_completed",
                    "session_name": "session-one",
                    "response_sha256": "exact-hash",
                    "trace": self.trace(self.TRACE_ONE, self.SPAN_ROOT),
                }
            ],
        )
        self.write_lines(
            "actions/receipts.jsonl",
            [
                {
                    "recorded_at_unix_ms": 101,
                    "session_id": "session-one",
                    "response_sha256": "exact-hash",
                    "decision_source": "astrid_declared",
                    "declared_next": "JOURNAL exact",
                }
            ],
        )
        self.write_lines(
            "web/receipts.jsonl",
            [
                {
                    "recorded_at_unix_ms": 101,
                    "call_id": "legacy-web",
                    "tool_name": "search_web",
                    "status": "success",
                    "arguments": {"query": "same timestamp is insufficient"},
                    "result_summary": {"results": []},
                }
            ],
        )

        events = activity.collect_events(self.workspace, 1_000)
        action = next(event for event in events if event["kind"] == "action")
        web = next(event for event in events if event["kind"] == "web_result")
        self.assertEqual(action["trace_id"], self.TRACE_ONE)
        self.assertEqual(action["trace_attribution"], "exact_response_session_join")
        self.assertIsNone(web["trace_id"])
        self.assertEqual(web["trace_attribution"], "legacy_unattributed")

    def test_request_completion_and_stale_state_are_deterministic_and_bounded(self) -> None:
        request_trace = self.trace(
            self.TRACE_WEB,
            self.SPAN_REQUEST,
            self.SPAN_ACTION,
        )
        self.write_lines(
            "web/receipts.jsonl",
            [
                {
                    "schema": "astrid_edge_web_tool_receipt_v2",
                    "phase": "requested",
                    "recorded_at_unix_ms": 100,
                    "requested_at_unix_ms": 100,
                    "call_id": "pending",
                    "tool_name": "search_web",
                    "arguments": {
                        "query": "bounded query",
                        "headers": {"Authorization": "never render"},
                    },
                    "origin": "react_model_tool",
                    "trace": request_trace,
                },
                {
                    "schema": "astrid_edge_web_tool_receipt_v2",
                    "phase": "completed",
                    "recorded_at_unix_ms": 200,
                    "requested_at_unix_ms": 150,
                    "completed_at_unix_ms": 200,
                    "latency_ms": 50,
                    "call_id": "done",
                    "tool_name": "fetch_url",
                    "arguments": {"url": "https://example.com"},
                    "status": "success",
                    "result_summary": {
                        "status": 200,
                        "body": "must not be rendered",
                    },
                    "origin": "action_executor_read_source",
                    "trace": self.trace(
                        self.TRACE_WEB,
                        self.SPAN_RESULT,
                        self.SPAN_REQUEST,
                    ),
                },
            ],
        )

        first = activity.collect_events(self.workspace, activity.STALE_WEB_CALL_MS + 101)
        second = activity.collect_events(self.workspace, activity.STALE_WEB_CALL_MS + 101)
        self.assertEqual(first, second)
        pending = next(event for event in first if event.get("call_id") == "pending")
        completed = next(event for event in first if event.get("call_id") == "done")
        self.assertEqual(pending["status"], "stale")
        self.assertEqual(completed["http_status"], 200)
        rendered = "\n".join(activity.text_lines(first))
        self.assertNotIn("Authorization", rendered)
        self.assertNotIn("must not be rendered", rendered)
        self.assertIn("bounded query", rendered)

    def test_malformed_trace_is_never_displayed_as_first_class(self) -> None:
        self.write_lines(
            "autonomous/runs.jsonl",
            [
                {
                    "completed_at_unix_ms": 100,
                    "status": "authored_completed",
                    "session_name": "session-one",
                    "trace": self.trace("not-a-uuid", self.SPAN_ROOT),
                },
                {
                    "completed_at_unix_ms": 200,
                    "status": "authored_completed",
                    "session_name": "session-two",
                    "trace": {
                        **self.trace(self.TRACE_ONE, self.SPAN_ROOT),
                        "schema_version": 99,
                    },
                },
            ],
        )

        events = activity.collect_events(self.workspace, 1_000)
        self.assertEqual(len(events), 2)
        for event in events:
            self.assertIsNone(event["trace_id"])
            self.assertEqual(event["trace_attribution"], "legacy_unattributed")

    def test_interrupted_action_correction_overrides_false_authorship(self) -> None:
        self.write_lines(
            "actions/receipts.jsonl",
            [
                {
                    "recorded_at_unix_ms": 100,
                    "response_sha256": "interrupted-hash",
                    "decision_source": "astrid_declared",
                    "status": "executed",
                    "declared_next": "JOURNAL stale",
                    "trace": self.trace(self.TRACE_ONE, self.SPAN_ACTION),
                }
            ],
        )
        self.write_lines(
            "actions/interrupted_corrections.jsonl",
            [
                {
                    "recorded_at_unix_ms": 200,
                    "response_sha256": "interrupted-hash",
                    "corrected_status": "revoked_interrupted_trace_non_authored",
                    "authority": "operator_reconciliation_non_authored_no_action_authority",
                }
            ],
        )

        event = next(
            value
            for value in activity.collect_events(self.workspace, 1_000)
            if value["kind"] == "action"
        )
        self.assertEqual(event["status"], "revoked_interrupted_trace_non_authored")
        self.assertFalse(event["authored"])
        self.assertTrue(event["fallback"])
        self.assertIn(
            "status=revoked_interrupted_trace_non_authored",
            "\n".join(activity.text_lines([event])),
        )

    def test_operator_study_is_explicitly_non_authored_and_attributed(self) -> None:
        self.write_lines(
            "studies/receipts.jsonl",
            [
                {
                    "schema": "astrid_edge_study_receipt_v1",
                    "phase": "started",
                    "recorded_at_unix_ms": 250,
                    "study_id": "study_operator",
                    "status": "active",
                    "primary_metric": "fill",
                    "secondary_metric": "generation_latency",
                    "sample_count": 0,
                    "origin": "operator_harness",
                    "authority": "deterministic_machine_study_not_astrid_authorship_or_causal_proof",
                }
            ],
        )

        event = next(
            value
            for value in activity.collect_events(self.workspace, 1_000)
            if value["kind"] == "study"
        )
        self.assertFalse(event["authored"])
        self.assertEqual(event["origin"], "operator_harness")
        self.assertEqual(event["trace_attribution"], "operator_harness")
        rendered = "\n".join(activity.text_lines([event]))
        self.assertIn("[operator] STUDY", rendered)
        self.assertIn("origin=operator_harness", rendered)
        self.assertIn("authored=false", rendered)

    def test_thread_capsule_is_a_bounded_causal_activity_event(self) -> None:
        self.write_lines(
            "autonomous/thread_state.jsonl",
            [
                {
                    "schema": "astrid_edge_thread_state_v2",
                    "updated_at_unix_ms": 300,
                    "thread_id": "chain-one",
                    "status": "active",
                    "event": "action_research",
                    "focus": "bounded question",
                    "question": "bounded question",
                    "findings": ["one bounded finding"],
                    "open_questions": ["what remains unknown"],
                    "conclusion": "not established",
                    "evidence": ["web status=success"],
                    "evidence_records": [
                        {
                            "kind": "web",
                            "reference": "bounded question",
                            "summary": "results=1",
                            "source": "action_executor_research",
                        }
                    ],
                    "trace": self.trace(
                        self.TRACE_ONE,
                        self.SPAN_THREAD,
                        self.SPAN_ACTION,
                    ),
                }
            ],
        )
        events = activity.collect_events(self.workspace, 1_000)
        thread = next(event for event in events if event["kind"] == "thread")
        self.assertEqual(thread["trace_id"], self.TRACE_ONE)
        self.assertEqual(thread["trace_attribution"], "first_class")
        self.assertEqual(thread["focus"], "bounded question")
        self.assertEqual(thread["question"], "bounded question")
        self.assertEqual(thread["findings"], ["one bounded finding"])
        self.assertEqual(thread["open_questions"], ["what remains unknown"])
        self.assertIn("THREAD", "\n".join(activity.text_lines(events)))

    def test_introspection_and_perception_remain_non_authored_and_body_free(self) -> None:
        self.write_lines(
            "introspection/receipts.jsonl",
            [
                {
                    "phase": "completed",
                    "recorded_at_unix_ms": 400,
                    "completed_at_unix_ms": 400,
                    "call_id": "intro-1",
                    "tool_name": "search_owned_text",
                    "arguments": {"query": "heat"},
                    "status": "success",
                    "origin": "action_executor_self_study",
                    "parent_response_sha256": "parent",
                    "result_summary": {
                        "match_count": 1,
                        "matches": [
                            {
                                "kind": "journal",
                                "basename": "journal_1.md",
                                "content": "must never render",
                            }
                        ],
                    },
                    "trace": self.trace(
                        self.TRACE_ONE,
                        self.SPAN_RESULT,
                        self.SPAN_REQUEST,
                    ),
                }
            ],
        )
        self.write_lines(
            "perception/observations.jsonl",
            [
                {
                    "recorded_at_unix_ms": 500,
                    "trigger_classes": ["baseline"],
                    "summary": "machine-observed numeric state",
                    "authority": (
                        "deterministic_machine_observation_not_astrid_authorship"
                    ),
                    "record_sha256": "hash",
                }
            ],
        )
        events = activity.collect_events(self.workspace, 1_000)
        introspection = next(
            event for event in events if event["kind"] == "introspection_result"
        )
        perception = next(
            event for event in events if event["kind"] == "perception"
        )
        self.assertFalse(introspection["authored"])
        self.assertFalse(perception["authored"])
        rendered = "\n".join(activity.text_lines(events))
        self.assertIn("machine-observed authored=false", rendered)
        self.assertNotIn("must never render", rendered)

    def test_spectral_and_tuning_events_are_machine_evidence_with_exact_trace(self) -> None:
        trace = self.trace(self.TRACE_ONE, self.SPAN_RESULT, self.SPAN_ACTION)
        self.write_lines(
            "spectral/rollups.jsonl",
            [
                {
                    "schema": "astrid_edge_spectral_rollup_v1",
                    "recorded_at_unix_ms": 600,
                    "substrate": {
                        "kind": "cpu_edge_covariance_effective_rank",
                        "fill_metric": "normalized_covariance_effective_rank",
                    },
                    "metrics": {
                        "fill_pct": 68.0,
                        "spectral_entropy": 0.9,
                        "mode_turnover": 0.2,
                    },
                    "activity_refs": [
                        {
                            "kind": "sovereign_action_outcome",
                            "recorded_at_unix_ms": 590,
                            "trace": trace,
                            "response_sha256": "a" * 64,
                        }
                    ],
                    "activity_ref_count": 1,
                    "activity_refs_truncated": False,
                    "authority": "deterministic_machine_derivation_not_authorship_or_causal_proof",
                }
            ],
        )
        self.write_lines(
            "tuning/receipts.jsonl",
            [
                {
                    "payload": {
                        "schema": "astrid_edge_tuning_receipt_v1",
                        "recorded_at_unix_ms": 700,
                        "phase": "rolled_back",
                        "status": "completed",
                        "trace": trace,
                        "detail": {
                            "tuning_id": "tuning-1",
                            "parameter": "input_gain",
                            "requested_value": 1.05,
                            "rollback_reason": "expired",
                        },
                        "authority": "signed_private_tuning_manager",
                    },
                    "payload_sha256": "b" * 64,
                    "signing_public_key": "c" * 64,
                    "signature": "d" * 128,
                }
            ],
        )
        events = activity.collect_events(self.workspace, 1_000)
        spectral = next(event for event in events if event["kind"] == "spectral_rollup")
        spectral_link = next(
            event for event in events if event["kind"] == "spectral_activity_link"
        )
        tuning = next(event for event in events if event["kind"] == "tuning")
        self.assertFalse(spectral["authored"])
        self.assertFalse(tuning["authored"])
        self.assertIsNone(spectral["trace_id"])
        self.assertEqual(spectral_link["trace_id"], self.TRACE_ONE)
        self.assertEqual(tuning["trace_id"], self.TRACE_ONE)
        self.assertEqual(tuning["timestamp_unix_ms"], 700)
        self.assertEqual(tuning["tuning_id"], "tuning-1")
        self.assertEqual(spectral["spectral_entropy"], 0.9)
        rendered = "\n".join(activity.text_lines([spectral, tuning]))
        self.assertIn("machine-derived authored=false", rendered)
        self.assertIn("rollback=expired", rendered)


if __name__ == "__main__":
    unittest.main()
