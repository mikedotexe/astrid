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
    def test_run_provenance_keeps_local_fallback_and_format_repair_distinct(self) -> None:
        self.assertEqual(
            REPORT.run_response_provenance(
                {
                    "response_provenance": (
                        "model_authored_with_local_safe_fallback"
                    )
                }
            ),
            ("model_authored_with_local_safe_fallback", True, False),
        )
        self.assertEqual(
            REPORT.run_response_provenance(
                {
                    "response_provenance": (
                        "model_authored_with_local_format_repair"
                    )
                }
            ),
            ("model_authored_with_local_format_repair", False, True),
        )
        self.assertEqual(
            REPORT.run_response_provenance({}),
            ("legacy_unspecified", False, False),
        )
        self.assertEqual(
            REPORT.run_response_provenance({"response_provenance": "spoofed"}),
            ("invalid", False, False),
        )

    def test_action_dispatch_summary_exposes_pending_orphan_and_corrupt_records(self) -> None:
        dispatch_span = "00000000-0000-4000-8000-000000000090"

        def trace(
            turn_id: str,
            trace_id: str,
            *,
            span_id: str = dispatch_span,
            parent_span_id: str | None = None,
        ) -> dict[str, object]:
            return {
                "schema_version": 1,
                "trace_id": trace_id,
                "span_id": span_id,
                "parent_span_id": parent_span_id,
                "turn_id": turn_id,
                "session_id": "session-one",
                "chain_id": None,
            }

        def dispatch(
            phase: str, turn_id: str, response_hash: str, trace_id: str
        ) -> dict[str, object]:
            return {
                "schema": "astrid_edge_action_dispatch_v1",
                "phase": phase,
                "turn_id": turn_id,
                "response_sha256": response_hash,
                "trace": trace(turn_id, trace_id),
            }

        turn_one = "00000000-0000-4000-8000-000000000001"
        turn_two = "00000000-0000-4000-8000-000000000002"
        turn_three = "00000000-0000-4000-8000-000000000003"
        turn_four = "00000000-0000-4000-8000-000000000004"
        trace_one = "00000000-0000-4000-8000-000000000011"
        trace_two = "00000000-0000-4000-8000-000000000012"
        trace_three = "00000000-0000-4000-8000-000000000013"
        trace_four = "00000000-0000-4000-8000-000000000014"
        first = dispatch("requested", turn_one, "a" * 64, trace_one)
        action_receipts = [
            {
                "schema": "astrid_edge_action_receipt_v4",
                "response_sha256": response_hash,
                "session_id": "session-one",
                "trace": trace(
                    turn_id,
                    trace_id,
                    span_id="00000000-0000-4000-8000-000000000091",
                    parent_span_id=dispatch_span,
                ),
            }
            for turn_id, response_hash, trace_id in (
                (turn_one, "a" * 64, trace_one),
                (turn_one, "a" * 64, trace_one),
                (turn_two, "b" * 64, trace_two),
                ("00000000-0000-4000-8000-000000000005", "e" * 64, trace_four),
            )
        ]
        action_receipts.append(
            {
                "schema": "astrid_edge_action_receipt_v3",
                "response_sha256": "f" * 64,
            }
        )
        summary = REPORT.summarize_action_dispatches(
            [
                first,
                dict(first),
                dispatch("completed", turn_one, "a" * 64, trace_one),
                dispatch("requested", turn_two, "b" * 64, trace_two),
                dispatch("completed", turn_three, "c" * 64, trace_three),
                dispatch("requested", turn_four, "d" * 64, trace_four),
                dispatch("completed", turn_four, "d" * 64, trace_four),
                dispatch("unknown", turn_four, "not-a-hash", trace_four),
            ],
            action_receipts,
        )
        self.assertEqual(
            summary,
            {
                "records_total": 8,
                "requested_total": 4,
                "completed_total": 3,
                "fully_correlated_total": 1,
                "pending_total": 1,
                "orphan_completion_total": 1,
                "completed_without_action_receipt_total": 1,
                "action_receipt_without_intent_total": 1,
                "action_receipt_without_completion_total": 1,
                "duplicate_phase_total": 1,
                "duplicate_action_receipt_total": 1,
                "unattributed_action_receipt_total": 1,
                "malformed_total": 1,
            },
        )

    def test_dispatch_correlation_rejects_session_chain_and_parent_mismatch(self) -> None:
        turn_id = "00000000-0000-4000-8000-000000000001"
        trace_id = "00000000-0000-4000-8000-000000000011"
        dispatch_span = "00000000-0000-4000-8000-000000000021"
        response_hash = "a" * 64

        def trace(
            *,
            session_id: str = "session-one",
            chain_id: str | None = "chain-one",
            span_id: str = dispatch_span,
            parent_span_id: str | None = None,
        ) -> dict[str, object]:
            return {
                "schema_version": 1,
                "trace_id": trace_id,
                "span_id": span_id,
                "parent_span_id": parent_span_id,
                "turn_id": turn_id,
                "session_id": session_id,
                "chain_id": chain_id,
            }

        dispatches = [
            {
                "schema": "astrid_edge_action_dispatch_v1",
                "phase": phase,
                "turn_id": turn_id,
                "response_sha256": response_hash,
                "trace": trace(),
            }
            for phase in ("requested", "completed")
        ]

        def receipt(**trace_changes: object) -> dict[str, object]:
            receipt_trace = trace(
                span_id="00000000-0000-4000-8000-000000000022",
                parent_span_id=dispatch_span,
            )
            receipt_trace.update(trace_changes)
            return {
                "schema": "astrid_edge_action_receipt_v4",
                "response_sha256": response_hash,
                "session_id": receipt_trace["session_id"],
                "trace": receipt_trace,
            }

        for mismatch in (
            {"session_id": "session-two"},
            {"chain_id": "chain-two"},
            {"parent_span_id": "00000000-0000-4000-8000-000000000099"},
        ):
            with self.subTest(mismatch=mismatch):
                summary = REPORT.summarize_action_dispatches(
                    dispatches, [receipt(**mismatch)]
                )
                self.assertEqual(summary["fully_correlated_total"], 0)
                self.assertEqual(
                    summary["completed_without_action_receipt_total"], 1
                )
                self.assertEqual(summary["action_receipt_without_intent_total"], 1)

        exact = REPORT.summarize_action_dispatches(dispatches, [receipt()])
        self.assertEqual(exact["fully_correlated_total"], 1)
        self.assertEqual(exact["completed_without_action_receipt_total"], 0)

    def test_exact_correction_identity_never_matches_repeated_text_on_other_trace(self) -> None:
        response_hash = "a" * 64
        correction = {
            "schema": "astrid_edge_interrupted_action_correction_v2",
            "trace_id": "00000000-0000-4000-8000-000000000011",
            "turn_id": "00000000-0000-4000-8000-000000000001",
            "response_sha256": response_hash,
        }
        other = {
            "response_sha256": response_hash,
            "trace": {
                "schema_version": 1,
                "trace_id": "00000000-0000-4000-8000-000000000012",
                "span_id": "00000000-0000-4000-8000-000000000013",
                "turn_id": "00000000-0000-4000-8000-000000000002",
                "session_id": "session-two",
                "chain_id": None,
            },
        }
        self.assertNotEqual(
            REPORT.interrupted_correction_identity(correction),
            REPORT.exact_response_identity(other),
        )
        legacy = dict(correction, schema="astrid_edge_interrupted_action_correction_v1")
        self.assertIsNone(REPORT.interrupted_correction_identity(legacy))

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
