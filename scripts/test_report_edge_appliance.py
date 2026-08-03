#!/usr/bin/env python3
"""Regression tests for the CPU-edge appliance report."""

from __future__ import annotations

import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("report_edge_appliance.py")
SPEC = importlib.util.spec_from_file_location("report_edge_appliance", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
REPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPORT)


class FillSummaryTests(unittest.TestCase):
    @staticmethod
    def current_react_manifest() -> dict[str, object]:
        return {
            "schema": "astrid_headless_application_capsule_generation_v1",
            "generation_id": "headless-application-capsules-20260803T120000Z-42",
            "capsules": [
                {
                    "capsule_id": "astrid-capsule-react",
                    "archive_sha256": "a" * 64,
                    "normalized_payload_sha256": "b" * 64,
                    "installed_tree_sha256": "c" * 64,
                    "content_objects": [
                        {
                            "kind": "wasm",
                            "digest": "d" * 64,
                            "sha256": "e" * 64,
                        },
                        {
                            "kind": "wit",
                            "digest": "f" * 64,
                            "sha256": "1" * 64,
                        },
                    ],
                }
            ],
        }

    @staticmethod
    def write_current_manifest(
        root: Path,
        manifest: dict[str, object],
        *,
        sidecar_digest: str | None = None,
        write_sidecar: bool = True,
    ) -> str:
        manifest_root = root / "etc/install-manifests"
        manifest_root.mkdir(parents=True, exist_ok=True)
        payload = (json.dumps(manifest, sort_keys=True) + "\n").encode()
        (manifest_root / REPORT.CURRENT_CAPSULE_MANIFEST_NAME).write_bytes(payload)
        digest = hashlib.sha256(payload).hexdigest()
        if write_sidecar:
            (manifest_root / REPORT.CURRENT_CAPSULE_SIDECAR_NAME).write_text(
                f"{sidecar_digest or digest}  {REPORT.CURRENT_CAPSULE_MANIFEST_NAME}\n",
                encoding="utf-8",
            )
        return digest

    @staticmethod
    def write_legacy_react_manifest(root: Path) -> None:
        manifest_root = root / "etc/install-manifests"
        manifest_root.mkdir(parents=True, exist_ok=True)
        (manifest_root / "react-provenance-v1.json").write_text(
            json.dumps(
                {
                    "deployment_state": "legacy-active",
                    "deployment": {"live_content_address": "d" * 64},
                    "artifact": {"sha256": "e" * 64},
                }
            ),
            encoding="utf-8",
        )

    def test_react_provenance_prefers_verified_current_generic_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_legacy_react_manifest(root)
            digest = self.write_current_manifest(root, self.current_react_manifest())

            view = REPORT.react_provenance_view(root)

        self.assertEqual(view["source"], "current_generic_manifest")
        self.assertEqual(view["validation"], "verified")
        self.assertEqual(view["generation_state"], "verified_current_generic_manifest")
        self.assertEqual(view["live_content_address"], "d" * 64)
        self.assertEqual(view["archive_sha256"], "a" * 64)
        self.assertEqual(view["manifest_sha256"], digest)

    def test_react_provenance_sidecar_failure_never_falls_back_to_legacy(self) -> None:
        for write_sidecar, digest in ((True, "f" * 64), (False, None)):
            with self.subTest(write_sidecar=write_sidecar):
                with tempfile.TemporaryDirectory() as temporary:
                    root = Path(temporary)
                    self.write_legacy_react_manifest(root)
                    self.write_current_manifest(
                        root,
                        self.current_react_manifest(),
                        sidecar_digest=digest,
                        write_sidecar=write_sidecar,
                    )
                    view = REPORT.react_provenance_view(root)

                self.assertEqual(view["source"], "current_generic_manifest_invalid")
                self.assertEqual(view["archive_sha256"], "unavailable")
                self.assertNotEqual(view["archive_sha256"], "e" * 64)

    def test_react_provenance_rejects_schema_and_react_entry_mismatch(self) -> None:
        malformed_manifests = []
        wrong_schema = self.current_react_manifest()
        wrong_schema["schema"] = "future_schema"
        malformed_manifests.append(wrong_schema)
        missing_entry = self.current_react_manifest()
        missing_entry["capsules"] = []
        malformed_manifests.append(missing_entry)
        malformed_entry = self.current_react_manifest()
        malformed_entry["capsules"][0]["installed_tree_sha256"] = "not-a-hash"
        malformed_manifests.append(malformed_entry)
        missing_content_objects = self.current_react_manifest()
        del missing_content_objects["capsules"][0]["content_objects"]
        malformed_manifests.append(missing_content_objects)
        missing_wasm = self.current_react_manifest()
        missing_wasm["capsules"][0]["content_objects"] = [
            {
                "kind": "wit",
                "digest": "f" * 64,
                "sha256": "1" * 64,
            }
        ]
        malformed_manifests.append(missing_wasm)
        ambiguous_wasm = self.current_react_manifest()
        ambiguous_wasm["capsules"][0]["content_objects"].append(
            {
                "kind": "wasm",
                "digest": "2" * 64,
                "sha256": "3" * 64,
            }
        )
        malformed_manifests.append(ambiguous_wasm)

        for manifest in malformed_manifests:
            with self.subTest(manifest=manifest):
                with tempfile.TemporaryDirectory() as temporary:
                    root = Path(temporary)
                    self.write_legacy_react_manifest(root)
                    self.write_current_manifest(root, manifest)
                    view = REPORT.react_provenance_view(root)

                self.assertEqual(view["source"], "current_generic_manifest_invalid")
                self.assertEqual(view["live_content_address"], "unavailable")
                self.assertEqual(view["archive_sha256"], "unavailable")

    def test_react_provenance_legacy_only_remains_visible_as_manual(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_legacy_react_manifest(root)
            view = REPORT.react_provenance_view(root)

        self.assertEqual(view["source"], "legacy_manual_manifest")
        self.assertEqual(view["validation"], "legacy_manual_unverified")
        self.assertEqual(view["generation_state"], "legacy-active")
        self.assertEqual(view["live_content_address"], "d" * 64)
        self.assertEqual(view["archive_sha256"], "e" * 64)

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

    def test_modern_non_authored_runs_have_observational_provenance(self) -> None:
        for status in ("transport_recovery", "failed", "interrupted"):
            with self.subTest(status=status):
                run = {
                    "schema": "astrid_edge_autonomy_run_v4",
                    "status": status,
                    "response_provenance": None,
                }
                self.assertEqual(
                    REPORT.run_response_provenance(run),
                    (f"{status}_non_authored", False, False),
                )

        legacy = {
            "schema": "astrid_edge_autonomy_run_v3",
            "status": "failed",
            "response_provenance": None,
        }
        self.assertEqual(
            REPORT.run_response_provenance(legacy),
            ("legacy_unspecified", False, False),
        )

    def test_last_provenance_uses_latest_run_when_state_value_is_null(self) -> None:
        runs = [
            {
                "schema": "astrid_edge_autonomy_run_v4",
                "status": "authored_completed",
                "response_provenance": "model_authored",
            },
            {
                "schema": "astrid_edge_autonomy_run_v4",
                "status": "transport_recovery",
                "response_provenance": None,
            },
        ]
        self.assertEqual(
            REPORT.latest_response_provenance(
                {"last_response_provenance": None}, runs
            ),
            ("transport_recovery_non_authored", False, False),
        )

        counts = REPORT.response_provenance_counts(runs)
        self.assertEqual(list(counts), list(REPORT.RESPONSE_PROVENANCE_LABELS))
        self.assertEqual(counts["model_authored"], 1)
        self.assertEqual(counts["transport_recovery_non_authored"], 1)
        self.assertEqual(counts["failed_non_authored"], 0)
        self.assertEqual(counts["interrupted_non_authored"], 0)

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

    def test_live_process_profile_overrides_staged_rollout_profile(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            config = home / ".config/astrid"
            config.mkdir(parents=True)
            (config / "edge-appliance.env").write_text(
                "ASTRID_EDGE_AUTONOMY_ENABLED=true\n"
                "ASTRID_EDGE_SPECTRAL_ENABLED=true\n",
                encoding="utf-8",
            )
            values = REPORT.effective_profile_values(
                home,
                {
                    "ASTRID_EDGE_AUTONOMY_ENABLED": "false",
                    "ASTRID_EDGE_SPECTRAL_ENABLED": "false",
                },
            )
            self.assertEqual(values["ASTRID_EDGE_AUTONOMY_ENABLED"], "false")
            self.assertEqual(values["ASTRID_EDGE_SPECTRAL_ENABLED"], "false")

    def test_process_profile_parser_excludes_unrelated_and_secret_values(self) -> None:
        values = REPORT.parse_process_profile_values(
            b"ASTRID_EDGE_AUTONOMY_ENABLED=false\0"
            b"ASTRID_OLLAMA_MODEL=qwen3:1.7b\0"
            b"OPENAI_API_KEY=do-not-report\0"
            b"PATH=/usr/bin\0malformed\0"
        )
        self.assertEqual(
            values,
            {
                "ASTRID_EDGE_AUTONOMY_ENABLED": "false",
                "ASTRID_OLLAMA_MODEL": "qwen3:1.7b",
            },
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

    def test_session_retirement_report_counts_completed_exact_schema_only(self) -> None:
        rows = [
            {
                "schema": "astrid_edge_operator_session_retirement_v1",
                "phase": "requested",
            },
            {
                "schema": "astrid_edge_operator_session_retirement_v1",
                "phase": "completed",
                "prior_session_generation": 255,
            },
            {
                "schema": "legacy_session_retirement",
                "phase": "completed",
            },
        ]
        self.assertEqual(
            REPORT.completed_session_retirements(rows),
            [rows[1]],
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

    def test_exact_header_latency_requires_trace_source_request_and_single_count(self) -> None:
        run = {
            "schema": "astrid_edge_autonomy_run_v4",
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
            "trace": {
                "schema_version": 1,
                "trace_id": "00000000-0000-4000-8000-000000000072",
                "turn_id": "00000000-0000-4000-8000-000000000073",
                "span_id": "00000000-0000-4000-8000-000000000074",
                "session_id": "session-one",
            },
        }
        self.assertEqual(
            REPORT.exact_request_header_latency(run),
            (288_001, "00000000-0000-4000-8000-000000000071", 1),
        )

        for field, value in (
            ("request_header_latency_source", None),
            ("provider_request_count", 2),
            ("provider_successful_header_count", 0),
            ("provider_request_id", None),
        ):
            malformed = dict(run)
            malformed[field] = value
            self.assertIsNone(REPORT.exact_request_header_latency(malformed))
        missing_turn = dict(run)
        missing_turn["trace"] = dict(run["trace"])
        missing_turn["trace"].pop("turn_id")
        self.assertIsNone(REPORT.exact_request_header_latency(missing_turn))

        duplicate_attempt = dict(run)
        duplicate_attempt["provider_request_count"] = 2
        duplicate_attempt["provider_successful_header_count"] = 2
        duplicate_attempt["provider_request_id"] = None
        duplicate_attempt["request_header_latency_ms"] = None
        duplicate_attempt["provider_requests"] = [
            dict(run["provider_requests"][0]),
            dict(run["provider_requests"][0]),
        ]
        self.assertIsNone(
            REPORT.exact_provider_request_telemetry(duplicate_attempt)
        )

        tampered_claim = dict(run)
        tampered_claim["provider_requests"] = []
        self.assertIsNone(REPORT.exact_provider_request_telemetry(tampered_claim))
        self.assertIsNone(REPORT.legacy_unattributed_header_latency(tampered_claim))

        unhashable_outcome = dict(run)
        unhashable_outcome["provider_requests"] = [
            dict(run["provider_requests"][0], outcome=["successful_headers"])
        ]
        self.assertIsNone(
            REPORT.exact_provider_request_telemetry(unhashable_outcome)
        )

        boolean_scalar = dict(run)
        boolean_scalar["request_header_latency_ms"] = True
        boolean_scalar["provider_requests"] = [
            dict(run["provider_requests"][0], request_header_latency_ms=1)
        ]
        self.assertIsNone(
            REPORT.exact_provider_request_telemetry(boolean_scalar)
        )

        legacy = {
            "schema": "astrid_edge_autonomy_run_v4",
            "request_header_latency_ms": 7,
        }
        self.assertEqual(REPORT.legacy_unattributed_header_latency(legacy), 7)
        legacy["request_header_latency_ms"] = float("nan")
        self.assertIsNone(REPORT.legacy_unattributed_header_latency(legacy))
        legacy["request_header_latency_ms"] = float("inf")
        self.assertIsNone(REPORT.legacy_unattributed_header_latency(legacy))

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
