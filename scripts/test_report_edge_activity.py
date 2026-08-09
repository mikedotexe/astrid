#!/usr/bin/env python3
"""Tests for the read-only correlated edge activity view."""

from __future__ import annotations

import contextlib
import io
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
    TURN_ONE = "00000000-0000-4000-8000-000000000021"

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
        turn_id: str | None = None,
    ) -> dict[str, object]:
        return {
            "schema_version": 1,
            "trace_id": trace_id,
            "span_id": span_id,
            "parent_span_id": parent_span_id,
            "turn_id": turn_id or EdgeActivityTest.TURN_ONE,
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

    def test_ambiguous_response_session_join_remains_unattributed(self) -> None:
        second_trace = "00000000-0000-4000-8000-000000000031"
        self.write_lines(
            "autonomous/runs.jsonl",
            [
                {
                    "status": "authored_completed",
                    "session_name": "session-one",
                    "response_sha256": "a" * 64,
                    "trace": self.trace(self.TRACE_ONE, self.SPAN_ROOT),
                },
                {
                    "status": "authored_completed",
                    "session_name": "session-one",
                    "response_sha256": "a" * 64,
                    "trace": self.trace(
                        second_trace,
                        "00000000-0000-4000-8000-000000000032",
                        turn_id="00000000-0000-4000-8000-000000000033",
                    ),
                },
            ],
        )
        self.write_lines(
            "actions/receipts.jsonl",
            [
                {
                    "recorded_at_unix_ms": 100,
                    "session_id": "session-one",
                    "response_sha256": "a" * 64,
                    "decision_source": "astrid_declared",
                }
            ],
        )

        action = next(
            event
            for event in activity.collect_events(self.workspace, 1_000)
            if event["kind"] == "action"
        )
        self.assertIsNone(action["trace_id"])
        self.assertEqual(action["trace_attribution"], "legacy_unattributed")

    def test_run_authorship_surfaces_local_response_provenance(self) -> None:
        cases = (
            ("model_authored", True, False, False, False),
            (
                "model_authored_with_local_safe_fallback",
                True,
                False,
                True,
                False,
            ),
            (
                "model_authored_with_local_format_repair",
                True,
                False,
                False,
                True,
            ),
            ("executor_terminal_error", False, True, False, False),
            ("unknown_provenance", False, True, False, False),
        )
        for provenance, authored, fallback, safe_used, repair_used in cases:
            with self.subTest(provenance=provenance):
                result = activity.run_authorship(
                    {
                        "status": "authored_completed",
                        "response_provenance": provenance,
                    },
                    {},
                )
                expected_provenance = (
                    "invalid" if provenance == "unknown_provenance" else provenance
                )
                self.assertEqual(result.response_provenance, expected_provenance)
                self.assertEqual(result.authored, authored)
                self.assertEqual(result.fallback, fallback)
                self.assertEqual(result.local_safe_fallback_used, safe_used)
                self.assertEqual(result.local_format_repair_used, repair_used)

        legacy = activity.run_authorship({"status": "authored_completed"}, {})
        self.assertEqual(legacy.response_provenance, "legacy_unspecified")
        self.assertTrue(legacy.authored)
        self.assertFalse(legacy.local_safe_fallback_used)
        self.assertFalse(legacy.local_format_repair_used)

    def test_modern_terminal_runs_without_provenance_are_explicitly_non_authored(
        self,
    ) -> None:
        for status in ("transport_recovery", "failed", "interrupted"):
            expected = f"{status}_non_authored"
            for explicit_null in (False, True):
                run = {
                    "schema": "astrid_edge_autonomy_run_v4",
                    "status": status,
                }
                if explicit_null:
                    run["response_provenance"] = None
                with self.subTest(status=status, explicit_null=explicit_null):
                    result = activity.run_authorship(run, {})
                    self.assertEqual(result.response_provenance, expected)
                    self.assertFalse(result.authored)
                    self.assertTrue(result.fallback)

        corrected = activity.run_authorship(
            {
                "schema": "astrid_edge_autonomy_run_v4",
                "status": "authored_completed",
                "transcript_path": "transcripts/corrected.md",
            },
            {"transcript:transcripts/corrected.md": {"reason": "corrected"}},
        )
        self.assertEqual(corrected.status, "transport_recovery")
        self.assertEqual(
            corrected.response_provenance, "transport_recovery_non_authored"
        )
        self.assertFalse(corrected.authored)

    def test_missing_provenance_stays_legacy_when_not_a_modern_terminal_run(
        self,
    ) -> None:
        cases = (
            {"status": "failed"},
            {
                "schema": "astrid_edge_autonomy_run_v3",
                "status": "interrupted",
                "response_provenance": None,
            },
            {
                "schema": "astrid_edge_autonomy_run_v4",
                "status": "authored_completed",
            },
            {
                "schema": "astrid_edge_autonomy_run_v4",
                "status": "otherwise_unmarked",
            },
        )
        for run in cases:
            with self.subTest(run=run):
                result = activity.run_authorship(run, {})
                self.assertEqual(result.response_provenance, "legacy_unspecified")

    def test_malformed_or_claimed_derived_raw_provenance_is_invalid(self) -> None:
        for raw_provenance in (
            ["model_authored"],
            {"kind": "model_authored"},
            "failed_non_authored",
        ):
            with self.subTest(raw_provenance=raw_provenance):
                result = activity.run_authorship(
                    {
                        "schema": "astrid_edge_autonomy_run_v4",
                        "status": "failed",
                        "response_provenance": raw_provenance,
                    },
                    {},
                )
                self.assertEqual(result.response_provenance, "invalid")
                self.assertFalse(result.authored)
                self.assertTrue(result.fallback)

    def test_response_provenance_counters_are_fixed_and_zero_filled(self) -> None:
        events = [
            {
                "kind": "turn",
                "response_provenance": "failed_non_authored",
            },
            {
                "kind": "turn",
                "response_provenance": "failed_non_authored",
            },
            {
                "kind": "turn",
                "response_provenance": "legacy_unspecified",
            },
            {"kind": "action", "response_provenance": "model_authored"},
        ]
        counts = activity.response_provenance_counts(events)
        self.assertEqual(
            tuple(counts), activity.RESPONSE_PROVENANCE_COUNTER_KEYS
        )
        self.assertEqual(counts["failed_non_authored"], 2)
        self.assertEqual(counts["legacy_unspecified"], 1)
        self.assertEqual(counts["transport_recovery_non_authored"], 0)
        self.assertEqual(sum(counts.values()), 3)
        self.assertEqual(
            activity.response_provenance_summary_line(events),
            "RESPONSE_PROVENANCE_COUNTS "
            + " ".join(
                f"{key}={counts[key]}"
                for key in activity.RESPONSE_PROVENANCE_COUNTER_KEYS
            ),
        )

    def test_json_output_includes_terminal_provenance_counters(self) -> None:
        self.write_lines(
            "autonomous/runs.jsonl",
            [
                {
                    "schema": "astrid_edge_autonomy_run_v4",
                    "completed_at_unix_ms": timestamp,
                    "status": status,
                    "response_provenance": None,
                }
                for timestamp, status in (
                    (100, "transport_recovery"),
                    (200, "failed"),
                    (300, "interrupted"),
                )
            ],
        )
        args = activity.parser().parse_args(
            [
                "--workspace",
                str(self.workspace),
                "--since",
                "0",
                "--until",
                "1",
                "--format",
                "json",
            ]
        )
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            activity.render(args, set())
        report = json.loads(output.getvalue())
        self.assertEqual(report["authorship_attribution_version"], 6)
        self.assertEqual(
            [event["response_provenance"] for event in report["events"]],
            [
                "transport_recovery_non_authored",
                "failed_non_authored",
                "interrupted_non_authored",
            ],
        )
        counts = report["response_provenance_counts"]
        self.assertEqual(
            tuple(counts), activity.RESPONSE_PROVENANCE_COUNTER_KEYS
        )
        self.assertEqual(counts["transport_recovery_non_authored"], 1)
        self.assertEqual(counts["failed_non_authored"], 1)
        self.assertEqual(counts["interrupted_non_authored"], 1)
        self.assertEqual(counts["model_authored"], 0)

        text_args = activity.parser().parse_args(
            [
                "--workspace",
                str(self.workspace),
                "--since",
                "0",
                "--until",
                "1",
            ]
        )
        text_output = io.StringIO()
        with contextlib.redirect_stdout(text_output):
            activity.render(text_args, set())
        self.assertEqual(
            text_output.getvalue().splitlines()[0],
            activity.response_provenance_summary_line(report["events"]),
        )

    def test_turn_activity_renders_local_modification_flags(self) -> None:
        self.write_lines(
            "autonomous/runs.jsonl",
            [
                {
                    "completed_at_unix_ms": 100,
                    "status": "authored_completed",
                    "response_provenance": (
                        "model_authored_with_local_format_repair"
                    ),
                    "trace": self.trace(self.TRACE_ONE, self.SPAN_ROOT),
                }
            ],
        )
        turn = next(
            event
            for event in activity.collect_events(self.workspace, 1_000)
            if event["kind"] == "turn"
        )
        self.assertEqual(
            turn["response_provenance"],
            "model_authored_with_local_format_repair",
        )
        self.assertFalse(turn["local_safe_fallback_used"])
        self.assertTrue(turn["local_format_repair_used"])
        rendered = "\n".join(activity.text_lines([turn]))
        self.assertIn(
            "provenance=model_authored_with_local_format_repair", rendered
        )
        self.assertIn("local_format_repair=true", rendered)

    def test_turn_activity_separates_exact_and_legacy_header_latency(self) -> None:
        exact_trace = self.trace(self.TRACE_ONE, self.SPAN_ROOT)
        self.write_lines(
            "autonomous/runs.jsonl",
            [
                {
                    "schema": "astrid_edge_autonomy_run_v4",
                    "completed_at_unix_ms": 100,
                    "status": "authored_completed",
                    "trigger": "exact",
                    "request_header_latency_ms": 288_001,
                    "request_header_latency_source": "kernel_http_host_trace_v1",
                    "provider_request_id": "00000000-0000-4000-8000-000000000071",
                    "provider_request_count": 1,
                    "provider_successful_header_count": 1,
                    "provider_requests": [
                        {
                            "attempt_id": "00000000-0000-4000-8000-000000000075",
                            "request_id": "00000000-0000-4000-8000-000000000071",
                            "outcome": "successful_headers",
                            "request_header_latency_ms": 288_001,
                        }
                    ],
                    "trace": exact_trace,
                },
                {
                    "schema": "astrid_edge_autonomy_run_v4",
                    "completed_at_unix_ms": 102,
                    "status": "authored_completed",
                    "trigger": "tampered-exact",
                    "request_header_latency_ms": 9,
                    "request_header_latency_source": "kernel_http_host_trace_v1",
                    "provider_request_id": "00000000-0000-4000-8000-000000000091",
                    "provider_request_count": 1,
                    "provider_successful_header_count": 1,
                    "provider_requests": [
                        {
                            "attempt_id": "00000000-0000-4000-8000-000000000095",
                            "request_id": "00000000-0000-4000-8000-000000000091",
                            "outcome": ["successful_headers"],
                            "request_header_latency_ms": 9,
                        }
                    ],
                    "trace": self.trace(
                        "00000000-0000-4000-8000-000000000092",
                        "00000000-0000-4000-8000-000000000093",
                        turn_id="00000000-0000-4000-8000-000000000094",
                    ),
                },
                {
                    "schema": "astrid_edge_autonomy_run_v4",
                    "completed_at_unix_ms": 103,
                    "status": "authored_completed",
                    "trigger": "multi",
                    "request_header_latency_ms": None,
                    "request_header_latency_source": "kernel_http_host_trace_v1",
                    "provider_request_count": 2,
                    "provider_successful_header_count": 1,
                    "provider_requests": [
                        {
                            "attempt_id": "00000000-0000-4000-8000-0000000000a1",
                            "request_id": "00000000-0000-4000-8000-0000000000a2",
                            "outcome": "timeout",
                        },
                        {
                            "attempt_id": "00000000-0000-4000-8000-0000000000a3",
                            "request_id": "00000000-0000-4000-8000-0000000000a4",
                            "outcome": "successful_headers",
                            "request_header_latency_ms": 17,
                        },
                    ],
                    "trace": self.trace(
                        "00000000-0000-4000-8000-0000000000a5",
                        "00000000-0000-4000-8000-0000000000a6",
                        turn_id="00000000-0000-4000-8000-0000000000a7",
                    ),
                },
                {
                    "schema": "astrid_edge_autonomy_run_v4",
                    "completed_at_unix_ms": 101,
                    "status": "authored_completed",
                    "trigger": "legacy",
                    "request_header_latency_ms": 7,
                    "trace": self.trace(
                        "00000000-0000-4000-8000-000000000081",
                        "00000000-0000-4000-8000-000000000082",
                        turn_id="00000000-0000-4000-8000-000000000083",
                    ),
                },
            ],
        )
        turns = {
            event["trigger"]: event
            for event in activity.collect_events(self.workspace, 1_000)
            if event["kind"] == "turn"
        }
        exact = turns["exact"]
        self.assertEqual(exact["request_header_latency_ms"], 288_001)
        self.assertEqual(
            exact["request_header_latency_source"], "kernel_http_host_trace_v1"
        )
        self.assertEqual(
            exact["provider_request_id"],
            "00000000-0000-4000-8000-000000000071",
        )
        self.assertEqual(exact["provider_request_count"], 1)
        self.assertEqual(exact["provider_successful_header_count"], 1)
        self.assertEqual(len(exact["provider_requests"]), 1)
        self.assertFalse(exact["provider_metrics_invalid_claimed_exact"])
        self.assertIsNone(exact["request_header_latency_ms_legacy_unattributed"])

        legacy = turns["legacy"]
        self.assertIsNone(legacy["request_header_latency_ms"])
        self.assertIsNone(legacy["request_header_latency_source"])
        self.assertEqual(legacy["request_header_latency_ms_legacy_unattributed"], 7)

        tampered = turns["tampered-exact"]
        self.assertIsNone(tampered["request_header_latency_ms"])
        self.assertIsNone(tampered["request_header_latency_ms_legacy_unattributed"])
        self.assertIsNone(tampered["provider_request_count"])
        self.assertTrue(tampered["provider_metrics_invalid_claimed_exact"])

        multi = turns["multi"]
        self.assertEqual(multi["provider_request_count"], 2)
        self.assertEqual(multi["provider_successful_header_count"], 1)
        self.assertEqual(len(multi["provider_requests"]), 2)
        self.assertIsNone(multi["provider_request_id"])
        self.assertIsNone(multi["request_header_latency_ms"])
        self.assertFalse(multi["provider_metrics_invalid_claimed_exact"])
        self.assertIsNone(
            activity.legacy_unattributed_header_latency(
                {"request_header_latency_ms": float("nan")}
            )
        )

    def test_completed_operator_session_retirement_is_visible_and_non_authored(self) -> None:
        self.write_lines(
            "autonomous/session_retirements.jsonl",
            [
                {
                    "schema": "astrid_edge_operator_session_retirement_v1",
                    "phase": "requested",
                    "recorded_at_unix_ms": 99,
                    "prior_session_generation": 255,
                    "new_session_generation": 256,
                },
                {
                    "schema": "astrid_edge_operator_session_retirement_v1",
                    "phase": "completed",
                    "recorded_at_unix_ms": 100,
                    "transition_id": "retirement-one",
                    "prior_session_generation": 255,
                    "new_session_generation": 256,
                    "reason": "retire unverified legacy session history",
                    "authority": "operator_compatibility_repair_no_turn",
                },
            ],
        )

        events = [
            event
            for event in activity.collect_events(self.workspace, 1_000)
            if event["kind"] == "session_retirement"
        ]
        self.assertEqual(len(events), 1)
        self.assertFalse(events[0]["authored"])
        self.assertEqual(events[0]["trace_attribution"], "operator_session_retirement")
        rendered = "\n".join(activity.text_lines(events))
        self.assertIn("SESSION_RETIREMENT", rendered)
        self.assertIn("generation=255->256", rendered)
        self.assertIn("counters-preserved=true", rendered)

    def test_action_dispatch_phases_are_non_authored_first_class_events(self) -> None:
        trace = self.trace(self.TRACE_ONE, self.SPAN_ACTION, self.SPAN_ROOT)
        self.write_lines(
            "actions/dispatches.jsonl",
            [
                {
                    "schema": "astrid_edge_action_dispatch_v1",
                    "phase": phase,
                    "recorded_at_unix_ms": timestamp,
                    "turn_id": self.TURN_ONE,
                    "response_sha256": "a" * 64,
                    "trace": trace,
                    "authority": "executor_idempotency_record_not_astrid_authorship",
                }
                for phase, timestamp in (("requested", 100), ("completed", 102))
            ],
        )

        events = [
            event
            for event in activity.collect_events(self.workspace, 1_000)
            if event["kind"] == "action_dispatch"
        ]
        self.assertEqual([event["phase"] for event in events], ["requested", "completed"])
        self.assertTrue(all(event["authored"] is False for event in events))
        self.assertTrue(
            all(event["trace_attribution"] == "first_class" for event in events)
        )
        self.assertTrue(
            all(event["source_ledger"] == "actions/dispatches.jsonl" for event in events)
        )
        rendered = "\n".join(activity.text_lines(events))
        self.assertIn("ACTION_DISPATCH", rendered)
        self.assertIn("authority=executor-idempotency", rendered)

    def test_invalid_action_dispatch_is_explicitly_untrusted(self) -> None:
        trace = self.trace(self.TRACE_ONE, self.SPAN_ACTION, self.SPAN_ROOT)
        self.write_lines(
            "actions/dispatches.jsonl",
            [
                {
                    "schema": "astrid_edge_action_dispatch_v1",
                    "phase": "requested",
                    "recorded_at_unix_ms": 100,
                    "turn_id": "00000000-0000-4000-8000-000000000099",
                    "response_sha256": "a" * 64,
                    "trace": trace,
                },
                {
                    "schema": "astrid_edge_action_dispatch_v1",
                    "phase": "requested",
                    "recorded_at_unix_ms": 101,
                    "turn_id": self.TURN_ONE,
                    "response_sha256": "not-a-hash",
                    "trace": trace,
                },
            ],
        )

        events = [
            event
            for event in activity.collect_events(self.workspace, 1_000)
            if event["kind"] == "action_dispatch"
        ]
        self.assertEqual(len(events), 2)
        self.assertTrue(all(event["status"] == "invalid" for event in events))
        self.assertTrue(
            all(
                event["trace_attribution"] == "invalid_untrusted_record"
                for event in events
            )
        )
        self.assertEqual(
            {event["integrity_error"] for event in events},
            {"trace_turn_id_mismatch", "invalid_response_sha256"},
        )
        self.assertTrue(all(event["trace_id"] is None for event in events))

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
                {
                    "completed_at_unix_ms": 300,
                    "status": "authored_completed",
                    "session_name": "session-three",
                    "trace": {
                        **self.trace(self.TRACE_ONE, self.SPAN_ROOT),
                        "turn_id": "00000000-0000-0000-0000-000000000000",
                    },
                },
                {
                    "completed_at_unix_ms": 400,
                    "status": "authored_completed",
                    "session_name": "session-four",
                    "trace": {
                        **self.trace(self.TRACE_ONE, self.SPAN_ROOT),
                        "session_id": "session\nspoof",
                    },
                },
                {
                    "completed_at_unix_ms": 500,
                    "status": "authored_completed",
                    "session_name": "session-five",
                    "trace": {
                        **self.trace(self.TRACE_ONE, self.SPAN_ROOT),
                        "parent_span_id": self.SPAN_ROOT,
                    },
                },
                {
                    "completed_at_unix_ms": 600,
                    "status": "authored_completed",
                    "session_name": "session-six",
                    "trace": {
                        **self.trace(self.TRACE_ONE, self.SPAN_ROOT),
                        "session_id": "   ",
                    },
                },
            ],
        )

        events = activity.collect_events(self.workspace, 1_000)
        self.assertEqual(len(events), 6)
        for event in events:
            self.assertIsNone(event["trace_id"])
            self.assertEqual(event["trace_attribution"], "legacy_unattributed")

    def test_interrupted_action_correction_overrides_false_authorship(self) -> None:
        second_trace = "00000000-0000-4000-8000-000000000041"
        second_turn = "00000000-0000-4000-8000-000000000042"
        self.write_lines(
            "actions/receipts.jsonl",
            [
                {
                    "recorded_at_unix_ms": 100,
                    "response_sha256": "a" * 64,
                    "decision_source": "astrid_declared",
                    "status": "executed",
                    "declared_next": "JOURNAL stale",
                    "trace": self.trace(self.TRACE_ONE, self.SPAN_ACTION),
                },
                {
                    "recorded_at_unix_ms": 101,
                    "response_sha256": "a" * 64,
                    "decision_source": "astrid_declared",
                    "status": "executed",
                    "declared_next": "JOURNAL same text, different turn",
                    "trace": self.trace(
                        second_trace,
                        "00000000-0000-4000-8000-000000000043",
                        turn_id=second_turn,
                    ),
                },
            ],
        )
        self.write_lines(
            "actions/interrupted_corrections.jsonl",
            [
                {
                    "schema": "astrid_edge_interrupted_action_correction_v2",
                    "recorded_at_unix_ms": 200,
                    "response_sha256": "a" * 64,
                    "trace_id": self.TRACE_ONE,
                    "turn_id": self.TURN_ONE,
                    "identity_kind": "turn_id",
                    "corrected_status": "revoked_interrupted_trace_non_authored",
                    "authority": "operator_reconciliation_non_authored_no_action_authority",
                }
            ],
        )

        events = activity.collect_events(self.workspace, 1_000)
        actions = [value for value in events if value["kind"] == "action"]
        self.assertEqual(actions[0]["status"], "revoked_interrupted_trace_non_authored")
        self.assertFalse(actions[0]["authored"])
        self.assertTrue(actions[0]["fallback"])
        self.assertEqual(actions[1]["status"], "executed")
        self.assertTrue(actions[1]["authored"])
        self.assertFalse(actions[1]["fallback"])
        correction = next(
            value for value in events if value["kind"] == "action_correction"
        )
        self.assertEqual(correction["trace_attribution"], "exact_trace_response_join")
        self.assertIn(
            "status=revoked_interrupted_trace_non_authored",
            "\n".join(activity.text_lines(actions)),
        )

    def test_legacy_interrupted_correction_remains_unattributed(self) -> None:
        self.write_lines(
            "actions/receipts.jsonl",
            [
                {
                    "recorded_at_unix_ms": 100,
                    "response_sha256": "a" * 64,
                    "decision_source": "astrid_declared",
                    "status": "executed",
                    "trace": self.trace(self.TRACE_ONE, self.SPAN_ACTION),
                }
            ],
        )
        self.write_lines(
            "actions/interrupted_corrections.jsonl",
            [
                {
                    "schema": "astrid_edge_interrupted_action_correction_v1",
                    "recorded_at_unix_ms": 200,
                    "response_sha256": "a" * 64,
                    "trace_id": self.TRACE_ONE,
                    "corrected_status": "revoked_interrupted_trace_non_authored",
                }
            ],
        )

        events = activity.collect_events(self.workspace, 1_000)
        action = next(value for value in events if value["kind"] == "action")
        correction = next(
            value for value in events if value["kind"] == "action_correction"
        )
        self.assertEqual(action["status"], "executed")
        self.assertTrue(action["authored"])
        self.assertEqual(correction["trace_attribution"], "legacy_unattributed")

    def test_transport_authorship_correction_reclassifies_turn_and_action(self) -> None:
        transcript = "autonomous/turns/autonomous_100.md"
        response_hash = "b" * 64
        independent_trace = "00000000-0000-4000-8000-000000000051"
        independent_turn = "00000000-0000-4000-8000-000000000052"
        self.write_lines(
            "autonomous/runs.jsonl",
            [
                {
                    "completed_at_unix_ms": 100,
                    "status": "authored_completed",
                    "session_name": "session-one",
                    "response_sha256": response_hash,
                    "transcript_path": transcript,
                    "trace": self.trace(self.TRACE_ONE, self.SPAN_ROOT),
                }
            ],
        )
        self.write_lines(
            "actions/receipts.jsonl",
            [
                {
                    "recorded_at_unix_ms": 101,
                    "response_sha256": response_hash,
                    "decision_source": "astrid_declared",
                    "status": "executed",
                    "declared_next": "JOURNAL false transport output",
                    "trace": self.trace(self.TRACE_ONE, self.SPAN_ACTION),
                },
                {
                    "recorded_at_unix_ms": 103,
                    "session_id": "session-one",
                    "response_sha256": response_hash,
                    "decision_source": "astrid_declared",
                    "status": "executed",
                    "declared_next": "JOURNAL independently authored same bytes",
                    "trace": self.trace(
                        independent_trace,
                        "00000000-0000-4000-8000-000000000053",
                        turn_id=independent_turn,
                    ),
                }
            ],
        )
        self.write_lines(
            "autonomous/thread_state.jsonl",
            [
                {
                    "updated_at_unix_ms": 102,
                    "response_sha256": response_hash,
                    "status": "active",
                    "event": "action_journal",
                    "trace": self.trace(self.TRACE_ONE, self.SPAN_THREAD),
                }
            ],
        )
        self.write_lines(
            "autonomous/authorship_corrections.jsonl",
            [
                {
                    "schema": "astrid_edge_authorship_correction_v2",
                    "recorded_at_unix_ms": 200,
                    "original_transcript_path": transcript,
                    "response_sha256": response_hash,
                    "reason": activity.TRANSPORT_AUTHORSHIP_CORRECTION_REASON,
                    "authority": (
                        "deterministic_provenance_correction_no_model_or_action_invocation"
                    ),
                }
            ],
        )

        events = activity.collect_events(self.workspace, 1_000)
        turn = next(value for value in events if value["kind"] == "turn")
        actions = [value for value in events if value["kind"] == "action"]
        action = next(
            value
            for value in actions
            if value.get("trace_id") == self.TRACE_ONE
        )
        independent_action = next(
            value
            for value in actions
            if value.get("trace_id") == independent_trace
        )
        thread = next(value for value in events if value["kind"] == "thread")
        self.assertEqual(turn["status"], "transport_recovery")
        self.assertFalse(turn["authored"])
        self.assertTrue(turn["fallback"])
        self.assertEqual(
            turn["correction_reason"],
            activity.TRANSPORT_AUTHORSHIP_CORRECTION_REASON,
        )
        self.assertEqual(
            action["status"], "revoked_legacy_transport_non_authored"
        )
        self.assertFalse(action["authored"])
        self.assertTrue(action["fallback"])
        self.assertTrue(independent_action["authored"])
        self.assertFalse(independent_action["fallback"])
        self.assertFalse(thread["authored"])
        self.assertTrue(thread["fallback"])
        rendered = "\n".join(activity.text_lines([turn, action, thread]))
        self.assertIn("authored=false", rendered)
        self.assertIn(activity.TRANSPORT_AUTHORSHIP_CORRECTION_REASON, rendered)

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
        self.assertNotIn("payload_hash_valid", event)
        self.assertNotIn("signature_present_not_verified", event)
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
                            "attribution": "temporal_rollup_context_not_exact_or_causal",
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
                            "authority_turn_id": self.TURN_ONE,
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
        self.assertEqual(tuning["turn_id"], self.TURN_ONE)
        self.assertEqual(tuning["authority_turn_id"], self.TURN_ONE)
        self.assertEqual(tuning["timestamp_unix_ms"], 700)
        self.assertEqual(tuning["tuning_id"], "tuning-1")
        self.assertEqual(spectral["spectral_entropy"], 0.9)
        self.assertNotIn("payload_hash_valid", spectral)
        self.assertFalse(tuning["payload_hash_valid"])
        self.assertTrue(tuning["signature_present_not_verified"])
        rendered = "\n".join(activity.text_lines([spectral, tuning]))
        self.assertIn("machine-derived authored=false", rendered)
        self.assertIn("rollback=expired", rendered)


if __name__ == "__main__":
    unittest.main()
