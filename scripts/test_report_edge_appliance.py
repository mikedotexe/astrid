#!/usr/bin/env python3
"""Regression tests for the CPU-edge appliance report."""

from __future__ import annotations

import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import tempfile
import types
import unittest
from unittest import mock


SCRIPT = Path(__file__).with_name("report_edge_appliance.py")
SPEC = importlib.util.spec_from_file_location("report_edge_appliance", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
REPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPORT)
GLANCE_SCRIPT = Path(__file__).with_name("astrid_at_a_glance.py")
GLANCE_SPEC = importlib.util.spec_from_file_location(
    "astrid_at_a_glance", GLANCE_SCRIPT
)
assert GLANCE_SPEC is not None and GLANCE_SPEC.loader is not None
GLANCE = importlib.util.module_from_spec(GLANCE_SPEC)
GLANCE_SPEC.loader.exec_module(GLANCE)
RETIRE_SCRIPT = Path(__file__).with_name("retire_edge_origin_mac_affordance.py")
RETIRE_SPEC = importlib.util.spec_from_file_location(
    "retire_edge_origin_mac_affordance_for_report", RETIRE_SCRIPT
)
assert RETIRE_SPEC is not None and RETIRE_SPEC.loader is not None
RETIRE = importlib.util.module_from_spec(RETIRE_SPEC)
RETIRE_SPEC.loader.exec_module(RETIRE)


def operator_lifecycle_event(**overrides: object) -> dict[str, object]:
    value: dict[str, object] = {
        "schema": "astrid.edge_self_change.operator_lifecycle_event.v1",
        "recorded_at": 1,
        "source_ledger": "candidate",
        "sequence": 1,
        "event_id": "candidate-one",
        "status": "candidate_recorded",
        "facets": ["candidate"],
        "record_sha256": "1" * 64,
        "candidate_id": "candidate-2",
        "candidate_sha256": "a" * 64,
        "build_id": None,
        "generation_id": None,
        "from_generation": None,
        "trace_id": None,
        "session_id": None,
        "turn_id": None,
        "response_sha256": None,
        "terminal_declaration_sha256": None,
        "terminal_reason_sha256": None,
        "terminal_authority": None,
        "automatic_retry": None,
        "tests_sha256": None,
        "bundle_sha256": None,
        "manifest_sha256": None,
        "invariant_candidate_replay_sha256": None,
        "invariant_package_replay_sha256": None,
        "shadow_evidence_sha256": None,
        "shadow_status": None,
        "command_profile": None,
        "command_executable_sha256": None,
        "command_argv_sha256": None,
        "command_stdout_sha256": None,
        "command_stderr_sha256": None,
        "command_exit_code": None,
        "command_timed_out": None,
        "provenance": "immutable_supervisor_signed_ledger_sanitized_metadata",
        "authority": "observation_only_not_deployment_or_astrid_authorship",
        "authored": False,
        "fallback": False,
    }
    value.update(overrides)
    return value


def write_operator_projection(
    path: Path,
    events: list[dict[str, object]],
    *,
    pipeline_phase: str = "idle",
    restart_phase: str = "none",
    restart_seconds: int = 0,
) -> dict[str, object]:
    path.parent.chmod(0o2750)
    heads = {name: None for name in ("candidate", "build", "activation", "operator")}
    for event in events:
        heads[str(event["source_ledger"])] = event["record_sha256"]
    core: dict[str, object] = {
        "schema": "astrid.edge_self_change.operator_status.v3",
        "appliance_id": "avado-test",
        "generated_at": 2,
        "state_revision": 7,
        "mode": "running",
        "active_generation": "generation-2",
        "previous_generation": "generation-1",
        "pipeline_phase": pipeline_phase,
        "latest_transition": {"operation": "activate", "status": "started"},
        "restart_expectation": {
            "phase": restart_phase,
            "maximum_seconds": restart_seconds,
            "basis": "immutable_command_profile_timeout_upper_bound",
        },
        "lifecycle": {
            "schema": "astrid.edge_self_change.operator_lifecycle.v1",
            "events": events,
            "included": len(events),
            "total": len(events),
            "truncated": False,
            "maximum_events": 64,
            "ledger_heads": heads,
        },
        "provenance": "immutable_supervisor_sanitized_projection",
        "authority": "observation_only_not_deployment_authority",
    }
    encoded = json.dumps(
        core,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")
    path.write_text(
        json.dumps(
            {
                "schema": "astrid.edge_self_change.operator_status_envelope.v1",
                "core": core,
                "core_sha256": hashlib.sha256(encoded).hexdigest(),
            }
        ),
        encoding="utf-8",
    )
    path.chmod(0o640)
    return core


def _encode_ed25519_point(point: tuple[int, int, int, int]) -> bytes:
    """Encode one extended Edwards point for deterministic test signatures."""
    x, y, z, _t = point
    inverse = pow(z, REPORT._ED25519_P - 2, REPORT._ED25519_P)
    affine_x = x * inverse % REPORT._ED25519_P
    affine_y = y * inverse % REPORT._ED25519_P
    encoded = affine_y | ((affine_x & 1) << 255)
    return encoded.to_bytes(32, "little")


def _sign_ed25519(seed: bytes, message: bytes) -> tuple[bytes, bytes]:
    """Create a deterministic Ed25519 signature without a test dependency."""
    expanded = hashlib.sha512(seed).digest()
    scalar_bytes = bytearray(expanded[:32])
    scalar_bytes[0] &= 248
    scalar_bytes[31] &= 63
    scalar_bytes[31] |= 64
    scalar = int.from_bytes(scalar_bytes, "little")
    public = _encode_ed25519_point(
        REPORT._ed25519_scalar(REPORT._ED25519_BASE, scalar)
    )
    nonce = int.from_bytes(hashlib.sha512(expanded[32:] + message).digest(), "little")
    nonce %= REPORT._ED25519_L
    encoded_r = _encode_ed25519_point(
        REPORT._ed25519_scalar(REPORT._ED25519_BASE, nonce)
    )
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public + message).digest(), "little"
    ) % REPORT._ED25519_L
    signature = encoded_r + ((nonce + challenge * scalar) % REPORT._ED25519_L).to_bytes(
        32, "little"
    )
    return public, signature


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
                "schema": (
                    "astrid_edge_action_receipt_v5"
                    if turn_id == turn_two
                    else "astrid_edge_action_receipt_v4"
                ),
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

    def test_scheduled_introspection_summary_separates_exact_authorship_from_fallback(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            (workspace / "runtime").mkdir()
            (workspace / "runtime/scheduled-introspection/admission").mkdir(
                parents=True
            )
            (workspace / "introspection/scheduled").mkdir(parents=True)
            (workspace / "introspections/scheduled").mkdir(parents=True)
            trace = {
                "schema_version": 1,
                "trace_id": "00000000-0000-4000-8000-000000000001",
                "span_id": "00000000-0000-4000-8000-000000000002",
                "turn_id": "00000000-0000-4000-8000-000000000003",
                "session_id": "scheduled-session",
                "chain_id": None,
            }
            reflection_path = "introspections/scheduled/reflection_due-700.md"
            reflection_body = b"I noticed a bounded pattern.\nI will inspect it carefully."
            response_sha256 = hashlib.sha256(reflection_body).hexdigest()
            prompt_sha256 = "b" * 64
            continuity_summary = "A bounded pattern merits careful inspection."
            context_provenance = {
                "schema": "astrid.edge.scheduled_context_provenance.v1",
                "candidate_authoring_eligible": True,
                "untrusted_external_content": False,
                "taint_causes": [],
            }
            context_provenance_sha256 = hashlib.sha256(
                REPORT.canonical_json_bytes(context_provenance)
            ).hexdigest()
            (workspace / "runtime/scheduled_introspection_state.json").write_text(
                json.dumps(
                    {
                        "schema": "astrid_edge_scheduled_introspection_state_v1",
                        "running": False,
                        "last_status": "transport_recovery",
                        "last_completed_at_unix_ms": 800,
                        "next_due_at_unix_ms": 2_000,
                        "total_attempts": 2,
                        "total_authored": 1,
                        "consecutive_failures": 1,
                    }
                )
            )
            (workspace / "runtime/scheduled_introspection_continuity.json").write_text(
                json.dumps(
                    {
                        "schema": "astrid_edge_scheduled_introspection_continuity_v1",
                        "appliance_id": "avado-test",
                        "model": "qwen-test",
                        "due_nonce": "due-700",
                        "recorded_at_unix_ms": 700,
                        "summary": continuity_summary,
                        "summary_sha256": hashlib.sha256(
                            continuity_summary.encode()
                        ).hexdigest(),
                        "response_sha256": response_sha256,
                        "prompt_sha256": prompt_sha256,
                        "trace": trace,
                        "provenance": "model_authored_runtime_scheduled",
                        "authority": (
                            "bounded_continuity_projection_not_voluntary_journal"
                        ),
                        "reflection_path": reflection_path,
                        "context_provenance": context_provenance,
                        "context_provenance_sha256": context_provenance_sha256,
                        "candidate_authoring_eligible": True,
                        "reflection_lane": "candidate_authoring_eligible",
                        "taint_causes": [],
                    }
                )
            )
            (
                workspace
                / "runtime/scheduled-introspection/admission/state.json"
            ).write_text(
                json.dumps(
                    {
                        "schema": "astrid.edge.scheduled_introspection.admission.v1",
                        "continuity_admitted": True,
                        "provenance": "model_authored_runtime_scheduled",
                        "authority": "runtime_verified_projection_observational_only",
                        "last_response_sha256": response_sha256,
                        "last_summary_sha256": hashlib.sha256(
                            continuity_summary.encode()
                        ).hexdigest(),
                        "last_trace_id": trace["trace_id"],
                        "last_due_nonce": "due-700",
                        "admitted_at_unix_ms": 710,
                    }
                )
            )
            receipts = [
                {
                    "schema": "astrid_edge_scheduled_introspection_v1",
                    "completed_at_unix_ms": 700,
                    "status": "authored_completed",
                    "provenance": "model_authored_runtime_scheduled",
                    "continuity_admitted": True,
                    "response_sha256": response_sha256,
                    "reflection_path": reflection_path,
                    "trace": trace,
                },
                {
                    "schema": "astrid_edge_scheduled_introspection_v1",
                    "completed_at_unix_ms": 800,
                    "status": "transport_recovery",
                    "provenance": "local_safe_fallback",
                    "continuity_admitted": False,
                    "response_sha256": None,
                    "reflection_path": None,
                },
            ]
            (workspace / "introspections/scheduled/receipts.jsonl").write_text(
                "".join(json.dumps(value) + "\n" for value in receipts)
            )
            (workspace / "introspection/scheduled/receipts.jsonl").write_text(
                "".join(json.dumps(value) + "\n" for value in receipts)
            )
            reflection = workspace / reflection_path
            reflection.write_bytes(reflection_body)
            reflection.with_suffix(".json").write_text(
                json.dumps(
                    {
                        "schema": "astrid.edge.scheduled_introspection.model_reflection.v1",
                        "provenance": "model_authored_runtime_scheduled",
                        "appliance_id": "avado-test",
                        "due_nonce": "due-700",
                        "trace_id": trace["trace_id"],
                        "session_id": trace["session_id"],
                        "turn_id": trace["turn_id"],
                        "model": "qwen-test",
                        "prompt_sha256": prompt_sha256,
                        "response_sha256": response_sha256,
                        "exact_response_path": reflection.name,
                        "context_provenance": context_provenance,
                        "context_provenance_sha256": context_provenance_sha256,
                        "reflection_lane": "candidate_authoring_eligible",
                        "taint_causes": [],
                    }
                )
            )

            summary = REPORT.scheduled_introspection_summary(
                workspace, cutoff_ms=500, now_ms=1_000
            )

        self.assertTrue(summary["state_present"])
        self.assertEqual(summary["window_receipts"], 2)
        self.assertEqual(summary["window_current_ledger_records"], 2)
        self.assertEqual(summary["window_legacy_ledger_records"], 2)
        self.assertEqual(summary["window_exact_duplicates_merged"], 2)
        self.assertEqual(
            summary["latest_receipt_source_ledgers"],
            "introspections/scheduled/receipts.jsonl,"
            "introspection/scheduled/receipts.jsonl",
        )
        self.assertEqual(summary["window_authored"], 0)
        self.assertEqual(summary["window_non_authored_excluded"], 2)
        self.assertEqual(summary["window_transport_recoveries"], 1)
        self.assertEqual(summary["continuity_provenance"], "model_authored_runtime_scheduled")
        self.assertFalse(summary["continuity_integrity_valid"])
        self.assertFalse(summary["continuity_actual_admitted"])
        self.assertEqual(summary["continuity_summary"], "unavailable")
        self.assertEqual(
            summary["continuity_validation"],
            "immutable_authorship_attestation_join_failed",
        )
        self.assertEqual(summary["authorship_attestation_status"], "verify_key_absent")
        self.assertEqual(
            summary["verified_reflection_excerpt"],
            "unavailable",
        )
        self.assertEqual(
            summary["reflection_text_authority"],
            "unavailable",
        )
        self.assertEqual(summary["reflection_artifact_count"], 1)

    def test_ed25519_verifier_matches_rfc8032_and_rejects_tampering(self) -> None:
        public_key = bytes.fromhex(
            "d75a980182b10ab7d54bfed3c964073a"
            "0ee172f3daa62325af021a68f707511a"
        )
        signature = bytes.fromhex(
            "e5564300c360ac729086e2cc806e828a"
            "84877f1eb8e5d974d873e06522490155"
            "5fb8821590a33bacc61e39701cf9b46bd"
            "25bf5f0595bbe24655141438e7a100b"
        )
        self.assertTrue(REPORT.verify_ed25519(public_key, b"", signature))
        self.assertFalse(REPORT.verify_ed25519(public_key, b"changed", signature))
        self.assertFalse(
            REPORT.verify_ed25519(public_key, b"", signature[:-1] + b"\x00")
        )

    def test_scheduled_introspection_requires_signed_exact_authorship_join(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary) / "workspace"
            projection = workspace / "runtime/scheduled-introspection/projection"
            admission_root = workspace / "runtime/scheduled-introspection/admission"
            artifacts = workspace / "introspections/scheduled"
            projection.mkdir(parents=True)
            admission_root.mkdir(parents=True)
            artifacts.mkdir(parents=True)
            key_path = Path(temporary) / "scheduled-authorship.pub"
            public_key, _unused = _sign_ed25519(b"scheduled-authorship-test-seed", b"")
            key_path.write_bytes(public_key)

            due_nonce = "due-10000"
            started_at = 10_000_001
            completed_at = 10_000_700
            trace = {
                "schema_version": 1,
                "trace_id": "10000000-0000-4000-8000-000000000001",
                "span_id": "10000000-0000-4000-8000-000000000002",
                "turn_id": "10000000-0000-4000-8000-000000000003",
                "session_id": "scheduled-session-10000",
            }
            reflection_path = (
                "introspections/scheduled/"
                "reflection_due-10000_10000000-0000-4000-8000-000000000003.md"
            )
            reflection_body = b"I verified a local pattern without claiming causation."
            response_sha256 = hashlib.sha256(reflection_body).hexdigest()
            prompt_sha256 = "a" * 64
            summary_text = "A verified local pattern remains non-causal evidence."
            context_provenance = {
                "schema": "astrid.edge.context_provenance.v1",
                "candidate_authoring_eligible": True,
                "untrusted_external_content": False,
                "taint_causes": [],
            }
            context_provenance_sha256 = hashlib.sha256(
                REPORT.canonical_json_bytes(context_provenance)
            ).hexdigest()
            continuity = {
                "schema": REPORT.SCHEDULED_INTROSPECTION_CONTINUITY_SCHEMA,
                "appliance_id": "avado-test",
                "model": "qwen-test",
                "due_nonce": due_nonce,
                "recorded_at_unix_ms": completed_at,
                "summary": summary_text,
                "summary_sha256": hashlib.sha256(summary_text.encode()).hexdigest(),
                "response_sha256": response_sha256,
                "prompt_sha256": prompt_sha256,
                "reflection_path": reflection_path,
                "trace": trace,
                "provenance": REPORT.SCHEDULED_INTROSPECTION_PROVENANCE,
                "authority": "bounded_continuity_projection_not_voluntary_journal",
                "context_provenance": context_provenance,
                "context_provenance_sha256": context_provenance_sha256,
                "candidate_authoring_eligible": True,
                "reflection_lane": "candidate_authoring_eligible",
                "taint_causes": [],
            }
            state = {
                "schema": REPORT.SCHEDULED_INTROSPECTION_STATE_SCHEMA,
                "running": False,
                "last_status": "authored_completed",
                "last_started_at_unix_ms": started_at,
                "last_completed_at_unix_ms": completed_at,
                "next_due_at_unix_ms": completed_at + 7_200_000,
                "total_attempts": 1,
                "total_authored": 1,
                "consecutive_failures": 0,
            }
            metadata = {
                "schema": "astrid.edge.scheduled_introspection.model_reflection.v1",
                "provenance": REPORT.SCHEDULED_INTROSPECTION_PROVENANCE,
                "appliance_id": continuity["appliance_id"],
                "due_nonce": due_nonce,
                "trace_id": trace["trace_id"],
                "session_id": trace["session_id"],
                "turn_id": trace["turn_id"],
                "model": continuity["model"],
                "prompt_sha256": prompt_sha256,
                "response_sha256": response_sha256,
                "exact_response_path": Path(reflection_path).name,
                "context_provenance": context_provenance,
                "context_provenance_sha256": context_provenance_sha256,
                "reflection_lane": "candidate_authoring_eligible",
                "taint_causes": [],
            }
            receipt = {
                "schema": REPORT.SCHEDULED_INTROSPECTION_RECEIPT_SCHEMA,
                "appliance": "avado-test",
                "due_nonce": due_nonce,
                "due_at_unix_ms": 10_000_000,
                "started_at_unix_ms": started_at,
                "completed_at_unix_ms": completed_at,
                "status": "authored_completed",
                "provenance": REPORT.SCHEDULED_INTROSPECTION_PROVENANCE,
                "model_id": "qwen-test",
                "prompt_sha256": prompt_sha256,
                "response_sha256": response_sha256,
                "reflection_path": reflection_path,
                "continuity_projection_written": True,
                "trace": trace,
            }
            continuity_bytes = REPORT.canonical_json_bytes(continuity)
            state_bytes = REPORT.canonical_json_bytes(state)
            metadata_bytes = REPORT.canonical_json_bytes(metadata)
            receipt_bytes = REPORT.canonical_json_bytes(receipt)
            (projection / "continuity.json").write_bytes(continuity_bytes)
            (projection / "state.json").write_bytes(state_bytes)
            reflection = workspace / reflection_path
            reflection.write_bytes(reflection_body)
            reflection.with_suffix(".json").write_bytes(metadata_bytes)
            (artifacts / "receipts.jsonl").write_bytes(receipt_bytes + b"\n")
            (admission_root / "state.json").write_text(
                json.dumps(
                    {
                        "schema": REPORT.SCHEDULED_INTROSPECTION_ADMISSION_SCHEMA,
                        "continuity_admitted": True,
                        "provenance": REPORT.SCHEDULED_INTROSPECTION_PROVENANCE,
                        "authority": "runtime_verified_projection_observational_only",
                        "last_response_sha256": response_sha256,
                        "last_summary_sha256": continuity["summary_sha256"],
                        "last_trace_id": trace["trace_id"],
                        "last_due_nonce": due_nonce,
                        "admitted_at_unix_ms": completed_at + 1,
                    }
                )
            )
            core = {
                "schema": REPORT.SCHEDULED_AUTHORSHIP_CORE_SCHEMA,
                "appliance_id": "avado-test",
                "due_nonce": due_nonce,
                "due_at_unix_ms": 10_000_000,
                "started_at_unix_ms": started_at,
                "completed_at_unix_ms": completed_at,
                "terminal_status": "authored_completed",
                "model": "qwen-test",
                "prompt_sha256": prompt_sha256,
                "response_sha256": response_sha256,
                "reflection_path": reflection_path,
                "reflection_sha256": response_sha256,
                "reflection_metadata_sha256": hashlib.sha256(metadata_bytes).hexdigest(),
                "continuity_projection_sha256": hashlib.sha256(
                    continuity_bytes
                ).hexdigest(),
                "state_projection_sha256": hashlib.sha256(state_bytes).hexdigest(),
                "terminal_receipt_sha256": hashlib.sha256(receipt_bytes).hexdigest(),
                "context_provenance_sha256": context_provenance_sha256,
                "candidate_id": None,
                "candidate_digest": None,
                "trace": trace,
                "provenance": REPORT.SCHEDULED_INTROSPECTION_PROVENANCE,
                "authority": "immutable_steward_signed_exact_authorship_join",
            }
            unsigned = {
                "schema": REPORT.SCHEDULED_AUTHORSHIP_ENVELOPE_SCHEMA,
                "core": core,
            }
            _public, signature = _sign_ed25519(
                b"scheduled-authorship-test-seed",
                REPORT.canonical_json_bytes(unsigned),
            )
            envelope = unsigned | {
                "auth": {
                    "algorithm": "ed25519",
                    "key_id": (
                        "ed25519:" + hashlib.sha256(public_key).hexdigest()[:16]
                    ),
                    "signature": signature.hex(),
                }
            }
            attestation = artifacts / (
                f"authorship_attestation_{due_nonce}_{response_sha256}.json"
            )
            attestation.write_bytes(REPORT.canonical_json_bytes(envelope))

            summary = REPORT.scheduled_introspection_summary(
                workspace,
                cutoff_ms=10_000_000,
                now_ms=10_001_000,
                verify_key_path=key_path,
            )

            self.assertEqual(summary["window_authored"], 1)
            self.assertEqual(summary["window_non_authored_excluded"], 0)
            self.assertTrue(summary["continuity_integrity_valid"])
            self.assertTrue(summary["continuity_actual_admitted"])
            self.assertEqual(summary["authorship_attestation_status"], "verified")
            self.assertEqual(summary["authorship_attestations_valid"], 1)
            self.assertEqual(summary["authorship_attestations_invalid"], 0)
            self.assertEqual(summary["continuity_summary"], summary_text)

            envelope["core"]["response_sha256"] = "f" * 64
            attestation.write_bytes(REPORT.canonical_json_bytes(envelope))
            rejected = REPORT.scheduled_introspection_summary(
                workspace,
                cutoff_ms=10_000_000,
                now_ms=10_001_000,
                verify_key_path=key_path,
            )

        self.assertEqual(rejected["window_authored"], 0)
        self.assertFalse(rejected["continuity_integrity_valid"])
        self.assertEqual(rejected["authorship_attestations_valid"], 0)
        self.assertEqual(rejected["authorship_attestations_invalid"], 1)

    def test_v2_evidence_integration_attestation_uses_trigger_provenance(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary) / "workspace"
            artifacts = workspace / "introspections/scheduled"
            artifacts.mkdir(parents=True)
            key_path = Path(temporary) / "scheduled-authorship.pub"
            public_key, _unused = _sign_ed25519(b"v2-evidence-attestation", b"")
            key_path.write_bytes(public_key)
            due_nonce = "due-9223372036854776000"
            response_sha256 = "1" * 64
            trigger_nonce = "evidence-integration-" + "2" * 64
            trace = {
                "schema_version": 1,
                "trace_id": "20000000-0000-4000-8000-000000000001",
                "turn_id": "20000000-0000-4000-8000-000000000002",
                "span_id": "20000000-0000-4000-8000-000000000003",
                "session_id": "integration-session",
            }
            core = {
                "schema": REPORT.SCHEDULED_AUTHORSHIP_CORE_SCHEMA_V2,
                "appliance_id": "avado-test",
                "due_nonce": due_nonce,
                "trigger_kind": "evidence_integration",
                "trigger_nonce": trigger_nonce,
                "due_at_unix_ms": None,
                "started_at_unix_ms": 20_000_000,
                "completed_at_unix_ms": 20_001_000,
                "terminal_status": "model_authored_structured",
                "model": "qwen-test",
                "prompt_sha256": "3" * 64,
                "response_sha256": response_sha256,
                "reflection_path": (
                    f"introspections/scheduled/reflection_{due_nonce}_"
                    f"{trace['turn_id']}.md"
                ),
                "reflection_sha256": response_sha256,
                "reflection_metadata_sha256": "4" * 64,
                "continuity_projection_sha256": "5" * 64,
                "inquiry_current_projection_sha256": "6" * 64,
                "signed_entry_id": "inquiry-entry-one",
                "step_id": "inquiry-step-one",
                "admission_id": "inquiry-admission-one",
                "inquiry_step_sha256": "7" * 64,
                "inquiry_declaration_sha256": "8" * 64,
                "state_projection_sha256": "9" * 64,
                "terminal_receipt_sha256": "a" * 64,
                "context_provenance_sha256": "b" * 64,
                "candidate_id": None,
                "candidate_digest": None,
                "trace": trace,
                "provenance": (
                    REPORT.EVIDENCE_INTEGRATION_INTROSPECTION_PROVENANCE
                ),
                "authority": "immutable_steward_signed_exact_authorship_join",
            }
            unsigned = {
                "schema": REPORT.SCHEDULED_AUTHORSHIP_ENVELOPE_SCHEMA_V2,
                "core": core,
            }
            _public, signature = _sign_ed25519(
                b"v2-evidence-attestation",
                REPORT.canonical_json_bytes(unsigned),
            )
            envelope = unsigned | {
                "auth": {
                    "algorithm": "ed25519",
                    "key_id": (
                        "ed25519:" + hashlib.sha256(public_key).hexdigest()[:16]
                    ),
                    "signature": signature.hex(),
                }
            }
            path = artifacts / (
                f"authorship_attestation_{due_nonce}_{response_sha256}.json"
            )
            path.write_bytes(REPORT.canonical_json_bytes(envelope))
            attestations, invalid, status = REPORT.scheduled_authorship_attestations(
                workspace, key_path
            )
            self.assertEqual(status, "verified")
            self.assertEqual(invalid, 0)
            self.assertEqual(len(attestations), 1)
            self.assertEqual(
                attestations[0]["provenance"],
                REPORT.EVIDENCE_INTEGRATION_INTROSPECTION_PROVENANCE,
            )

            envelope["core"]["provenance"] = REPORT.SCHEDULED_INTROSPECTION_PROVENANCE
            tampered_unsigned = {
                "schema": REPORT.SCHEDULED_AUTHORSHIP_ENVELOPE_SCHEMA_V2,
                "core": envelope["core"],
            }
            _public, signature = _sign_ed25519(
                b"v2-evidence-attestation",
                REPORT.canonical_json_bytes(tampered_unsigned),
            )
            envelope["auth"]["signature"] = signature.hex()
            path.write_bytes(REPORT.canonical_json_bytes(envelope))
            attestations, invalid, status = REPORT.scheduled_authorship_attestations(
                workspace, key_path
            )
            self.assertEqual(attestations, [])
            self.assertEqual(invalid, 1)
            self.assertEqual(status, "no_valid_attestations")

    def test_v2_semantic_admission_requires_exact_ack_state(self) -> None:
        trace_id = "30000000-0000-4000-8000-000000000001"
        continuity = {
            "trigger_kind": "evidence_integration",
            "signed_entry_id": "inquiry-entry-one",
            "admission_id": "inquiry-admission-one",
            "response_sha256": "1" * 64,
            "summary_sha256": "2" * 64,
            "due_nonce": "due-9223372036854776001",
            "trace": {"trace_id": trace_id},
        }
        admission = {
            "schema": REPORT.SCHEDULED_INTROSPECTION_ADMISSION_SCHEMA_V2,
            "continuity_admitted": True,
            "admitted_at_unix_ms": 1_000,
            "signed_entry_id": continuity["signed_entry_id"],
            "admission_id": continuity["admission_id"],
            "last_response_sha256": continuity["response_sha256"],
            "last_summary_sha256": continuity["summary_sha256"],
            "last_trace_id": trace_id,
            "last_due_nonce": continuity["due_nonce"],
            "reservoir_delivery": "acknowledged",
            "queued_at_unix_ms": 1_001,
            "terminal_at_unix_ms": 1_002,
            "reservoir_generation": "reservoir-generation-one",
            "reservoir_sequence": 0,
            "vector_sha256": "3" * 64,
            "source_class": "evidence_integration",
            "migrated_legacy_schema": None,
            "provenance": REPORT.EVIDENCE_INTEGRATION_INTROSPECTION_PROVENANCE,
            "authority": "verified_signed_inquiry_observational_only",
        }
        self.assertTrue(
            REPORT.valid_v2_semantic_admission(
                admission, continuity, continuity_valid=True
            )
        )
        malformed = dict(admission, reservoir_sequence=None)
        self.assertFalse(
            REPORT.valid_v2_semantic_admission(
                malformed, continuity, continuity_valid=True
            )
        )
        queued = dict(
            admission,
            reservoir_delivery="queued",
            terminal_at_unix_ms=None,
            reservoir_generation=None,
            reservoir_sequence=None,
        )
        self.assertTrue(
            REPORT.valid_v2_semantic_admission(
                queued, continuity, continuity_valid=True
            )
        )
        self.assertFalse(
            REPORT.valid_v2_semantic_admission(
                dict(queued, reservoir_generation="forged"),
                continuity,
                continuity_valid=True,
            )
        )

    def test_v2_summary_attributes_evidence_integration_and_exact_ack(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            projection = workspace / "runtime/scheduled-introspection/projection"
            artifacts = workspace / "introspections/scheduled"
            projection.mkdir(parents=True)
            artifacts.mkdir(parents=True)
            due_nonce = "due-9223372036854776002"
            trigger_nonce = "evidence-integration-" + "4" * 64
            trace = {
                "schema_version": 1,
                "trace_id": "40000000-0000-4000-8000-000000000001",
                "turn_id": "40000000-0000-4000-8000-000000000002",
                "span_id": "40000000-0000-4000-8000-000000000003",
                "session_id": "integration-session-two",
            }
            reflection_path = (
                f"introspections/scheduled/reflection_{due_nonce}_"
                f"{trace['turn_id']}.md"
            )
            reflection_bytes = b"Integrated exact evidence into a bounded inquiry."
            sidecar_bytes = b'{"schema":"test-sidecar"}'
            current_bytes = b'{"schema":"test-current"}'
            response_sha256 = hashlib.sha256(reflection_bytes).hexdigest()
            summary_text = "Exact evidence was integrated without automatic belief change."
            summary_sha256 = hashlib.sha256(summary_text.encode()).hexdigest()
            signed_entry_id = "inquiry-entry-evidence-two"
            step_id = "inquiry-step-evidence-two"
            admission_id = "inquiry-admission-evidence-two"
            continuity = {
                "schema": REPORT.SCHEDULED_INTROSPECTION_CONTINUITY_SCHEMA_V2,
                "appliance_id": "avado-test",
                "model": "qwen-test",
                "trigger_kind": "evidence_integration",
                "trigger_nonce": trigger_nonce,
                "due_nonce": due_nonce,
                "recorded_at_unix_ms": 40_001_000,
                "summary": summary_text,
                "summary_sha256": summary_sha256,
                "response_sha256": response_sha256,
                "prompt_sha256": "5" * 64,
                "reflection_path": reflection_path,
                "signed_entry_id": signed_entry_id,
                "step_id": step_id,
                "admission_id": admission_id,
                "inquiry_current_projection_sha256": hashlib.sha256(
                    current_bytes
                ).hexdigest(),
                "trace": trace,
                "provenance": (
                    REPORT.EVIDENCE_INTEGRATION_INTROSPECTION_PROVENANCE
                ),
                "authority": (
                    "bounded_signed_inquiry_continuity_projection_not_code_or_action_authority"
                ),
            }
            state = {
                "schema": REPORT.SCHEDULED_INTROSPECTION_STATE_SCHEMA_V2,
                "running": False,
                "last_status": "model_authored_structured",
                "last_started_at_unix_ms": 40_000_000,
                "last_completed_at_unix_ms": 40_001_000,
                "next_due_at_unix_ms": 47_200_000,
                "total_attempts": 1,
                "total_authored": 1,
                "total_structured": 1,
                "total_unstructured": 0,
                "consecutive_failures": 0,
            }
            receipt = {
                "schema": REPORT.SCHEDULED_INTROSPECTION_RECEIPT_SCHEMA_V2,
                "status": "model_authored_structured",
                "completed_at_unix_ms": 40_001_000,
                "trigger_kind": "evidence_integration",
                "trigger_nonce": trigger_nonce,
                "due_nonce": due_nonce,
                "provenance": (
                    REPORT.EVIDENCE_INTEGRATION_INTROSPECTION_PROVENANCE
                ),
                "prompt_sha256": continuity["prompt_sha256"],
                "response_sha256": response_sha256,
                "reflection_path": reflection_path,
                "trace": trace,
                "continuity_projection_written": True,
                "reservoir_admission_eligible": True,
                "continuity_admitted": False,
                "signed_entry_id": signed_entry_id,
                "step_id": step_id,
                "admission_id": admission_id,
                "inquiry_step_sha256": "6" * 64,
                "inquiry_declaration_sha256": "7" * 64,
                "inquiry_current_projection_sha256": hashlib.sha256(
                    current_bytes
                ).hexdigest(),
                "continuity_projection_sha256": hashlib.sha256(
                    REPORT.canonical_json_bytes(continuity)
                ).hexdigest(),
            }
            attestation = {
                "schema": REPORT.SCHEDULED_AUTHORSHIP_CORE_SCHEMA_V2,
                "terminal_status": "model_authored_structured",
                "terminal_receipt_sha256": hashlib.sha256(
                    REPORT.canonical_json_bytes(receipt)
                ).hexdigest(),
                "trigger_kind": "evidence_integration",
                "trigger_nonce": trigger_nonce,
                "due_nonce": due_nonce,
                "provenance": (
                    REPORT.EVIDENCE_INTEGRATION_INTROSPECTION_PROVENANCE
                ),
                "prompt_sha256": continuity["prompt_sha256"],
                "response_sha256": response_sha256,
                "reflection_path": reflection_path,
                "reflection_sha256": response_sha256,
                "reflection_metadata_sha256": hashlib.sha256(
                    sidecar_bytes
                ).hexdigest(),
                "trace": trace,
                "continuity_projection_sha256": receipt[
                    "continuity_projection_sha256"
                ],
                "state_projection_sha256": hashlib.sha256(
                    REPORT.canonical_json_bytes(state)
                ).hexdigest(),
                "signed_entry_id": signed_entry_id,
                "step_id": step_id,
                "admission_id": admission_id,
                "inquiry_step_sha256": receipt["inquiry_step_sha256"],
                "inquiry_declaration_sha256": receipt[
                    "inquiry_declaration_sha256"
                ],
                "inquiry_current_projection_sha256": receipt[
                    "inquiry_current_projection_sha256"
                ],
                "key_id": "ed25519:test",
                "attestation_path": "attestation.json",
            }
            admission = {
                "schema": REPORT.SCHEDULED_INTROSPECTION_ADMISSION_SCHEMA_V2,
                "continuity_admitted": True,
                "admitted_at_unix_ms": 40_001_001,
                "signed_entry_id": signed_entry_id,
                "admission_id": admission_id,
                "last_response_sha256": response_sha256,
                "last_summary_sha256": summary_sha256,
                "last_trace_id": trace["trace_id"],
                "last_due_nonce": due_nonce,
                "reservoir_delivery": "acknowledged",
                "queued_at_unix_ms": 40_001_002,
                "terminal_at_unix_ms": 40_001_003,
                "reservoir_generation": "reservoir-two",
                "reservoir_sequence": 9,
                "vector_sha256": "8" * 64,
                "source_class": "evidence_integration",
                "migrated_legacy_schema": None,
                "provenance": (
                    REPORT.EVIDENCE_INTEGRATION_INTROSPECTION_PROVENANCE
                ),
                "authority": "verified_signed_inquiry_observational_only",
            }
            reflection = workspace / reflection_path
            reflection.write_bytes(reflection_bytes)
            reflection.with_suffix(".json").write_bytes(sidecar_bytes)
            (projection / "inquiry-current.json").write_bytes(current_bytes)
            train_report = {
                "integrity": "full_signed_hash_chain_verified",
                "events": [
                    {
                        "kind": "inquiry_step",
                        "step_id": step_id,
                        "signed_entry_id": signed_entry_id,
                        "response_sha256": response_sha256,
                        "declaration_sha256": receipt[
                            "inquiry_declaration_sha256"
                        ],
                        "trigger_kind": "evidence_integration",
                    }
                ],
            }
            summary = REPORT.scheduled_introspection_summary_v2(
                workspace,
                cutoff_ms=40_000_000,
                now_ms=40_002_000,
                state=state,
                continuity=continuity,
                sourced_receipts=[
                    (
                        receipt,
                        ("introspections/scheduled/receipts.jsonl",),
                        1,
                    )
                ],
                admission=admission,
                attestations=[attestation],
                invalid_attestations=0,
                attestation_status="verified",
                train_report=train_report,
            )
        self.assertEqual(summary["window_authored"], 1)
        self.assertEqual(summary["window_authored_scheduled"], 0)
        self.assertEqual(summary["window_authored_evidence_integrations"], 1)
        self.assertTrue(summary["continuity_integrity_valid"])
        self.assertTrue(summary["continuity_actual_admitted"])
        self.assertEqual(summary["reservoir_delivery"], "acknowledged")
        self.assertEqual(summary["continuity_trigger_kind"], "evidence_integration")
        self.assertEqual(
            summary["reflection_text_authority"],
            "owner_private_hash_verified_model_authored_runtime_evidence_integration",
        )

    def test_self_change_summary_uses_only_sanitized_operator_projection(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary) / "workspace"
            root = Path(temporary) / "supervisor"
            operator_path = Path(temporary) / "operator-status.json"
            (workspace / "self-change/outbox").mkdir(parents=True)
            (workspace / "self-change/patch-outbox").mkdir(parents=True)
            (root / "ledgers").mkdir(parents=True)
            private = root / "ledgers/build.jsonl"
            private.write_text("SECRET_PRIVATE_BUILD_LEDGER", encoding="utf-8")
            private.chmod(0o000)
            (workspace / "self-change/outbox/intent_1_a.json").write_text(
                json.dumps(
                    {
                        "recorded_at_unix_ms": 1_100,
                        "candidate_id": "candidate-2",
                        "candidate_digest": "a" * 64,
                        "provenance": "exact_model_scheduled_introspection",
                        "source_body": "must never render",
                    }
                )
            )
            patch_core = {
                "schema": "astrid.edge.steward_helper.owner_patch_export_summary.v1",
                "recorded_at": 2,
                "appliance_id": "avado-astrid",
                "candidate_id": "candidate-2",
                "candidate_sha256": "a" * 64,
                "patch_sha256": "b" * 64,
                "source_id": "source-1",
                "base_generation": "generation-1",
                "terminal_status": "accepted",
                "terminal_reason_sha256": "c" * 64,
                "touched_paths": ["scripts/report_edge_appliance.py"],
                "file_count": 1,
                "added_lines": 8,
                "removed_lines": 3,
                "changed_lines": 11,
                "full_export_sha256": "d" * 64,
                "source_bodies_retained": False,
                "authority": "reporting_summary_only_never_reingested_or_authorizing",
            }
            encoded = json.dumps(
                patch_core,
                sort_keys=True,
                separators=(",", ":"),
                ensure_ascii=True,
                allow_nan=False,
            ).encode("ascii")
            patch_path = (
                workspace
                / "self-change/patch-outbox"
                / f"candidate-change-candidate-2-{'a' * 64}.summary.json"
            )
            patch_path.write_text(
                json.dumps(
                    {
                        "schema": "astrid.edge.steward_helper.owner_patch_export_summary_envelope.v1",
                        "core": patch_core,
                        "core_sha256": hashlib.sha256(encoded).hexdigest(),
                        "auth": {
                            "algorithm": "hmac-sha256",
                            "key_id": "key-1",
                            "signature": "e" * 64,
                        },
                    }
                )
            )

            events = [
                operator_lifecycle_event(),
                operator_lifecycle_event(
                    recorded_at=2,
                    source_ledger="operator",
                    sequence=1,
                    event_id="reflection-one",
                    status="steward_profile_completed",
                    facets=["reflection"],
                    record_sha256="2" * 64,
                    response_sha256="f" * 64,
                ),
                operator_lifecycle_event(
                    recorded_at=3,
                    source_ledger="build",
                    sequence=1,
                    event_id="build-one",
                    status="build_recorded",
                    facets=["build", "invariant", "shadow", "test"],
                    record_sha256="3" * 64,
                    build_id="build-2",
                    generation_id="generation-2",
                    tests_sha256="4" * 64,
                    bundle_sha256="5" * 64,
                    invariant_candidate_replay_sha256="6" * 64,
                    invariant_package_replay_sha256="7" * 64,
                    shadow_evidence_sha256="7" * 64,
                    shadow_status="package_replay_hash_only_no_detailed_shadow_claim",
                ),
                operator_lifecycle_event(
                    recorded_at=4,
                    source_ledger="activation",
                    sequence=1,
                    event_id="probation-one",
                    status="probation_started",
                    facets=["activation", "probation", "restart"],
                    record_sha256="8" * 64,
                    build_id="build-2",
                    generation_id="generation-2",
                ),
                operator_lifecycle_event(
                    recorded_at=5,
                    source_ledger="activation",
                    sequence=2,
                    event_id="rollback-one",
                    status="rolled_back",
                    facets=["restart", "rollback"],
                    record_sha256="9" * 64,
                    generation_id="generation-1",
                    from_generation="generation-2",
                ),
            ]
            write_operator_projection(
                operator_path,
                events,
                pipeline_phase="probation",
                restart_phase="activation",
                restart_seconds=3_600,
            )
            try:
                summary = REPORT.self_change_summary(
                    workspace,
                    root,
                    cutoff_ms=500,
                    operator_status_path=operator_path,
                    test_only_allow_unprivileged_operator_status=True,
                )
            finally:
                private.chmod(0o600)

        self.assertEqual(summary["mode"], "running")
        self.assertEqual(summary["state_revision"], 7)
        self.assertEqual(summary["probation_status"], "active")
        self.assertEqual(summary["latest_intent_candidate_id"], "candidate-2")
        self.assertEqual(summary["latest_build_status"], "build_recorded")
        self.assertEqual(summary["latest_reflection_status"], "steward_profile_completed")
        self.assertEqual(summary["latest_reflection_response_sha256"], "f" * 64)
        self.assertEqual(summary["latest_tests_sha256"], "4" * 64)
        self.assertEqual(summary["latest_shadow_evidence_sha256"], "7" * 64)
        self.assertEqual(summary["latest_probation_status"], "probation_started")
        self.assertEqual(summary["latest_rollback_status"], "rolled_back")
        self.assertEqual(summary["expected_restart_phase"], "activation")
        self.assertEqual(summary["expected_restart_maximum_seconds"], 3_600)
        self.assertEqual(
            summary["expected_restart_basis"],
            "immutable_command_profile_timeout_upper_bound",
        )
        self.assertEqual(summary["patch_export_summary_total"], 1)
        self.assertEqual(summary["latest_patch_candidate_id"], "candidate-2")
        self.assertEqual(summary["latest_patch_changed_lines"], 11)
        self.assertFalse(summary["latest_patch_source_bodies_retained"])
        self.assertEqual(summary["ledger_candidate_legacy_records"], 0)
        self.assertEqual(summary["ledger_build_reported_valid"], "projection_hash_verified")
        self.assertFalse(summary["private_root_read"])
        self.assertEqual(
            summary["private_ledger_policy"],
            "0600_secret_not_read_by_operator_reports",
        )
        self.assertNotIn("must never render", json.dumps(summary))
        self.assertNotIn("SECRET_PRIVATE", json.dumps(summary))

    def test_operator_self_change_projection_is_narrow_and_hash_verified(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "operator-status.json"
            core = write_operator_projection(
                path,
                [operator_lifecycle_event()],
                pipeline_phase="probation",
            )
            self.assertEqual(REPORT.read_self_change_operator_status(path), {})
            self.assertEqual(
                REPORT.read_self_change_operator_status(
                    path, test_only_allow_unprivileged_owner=True
                )["pipeline_phase"],
                "probation",
            )

            legacy_v2 = {key: value for key, value in core.items() if key != "lifecycle"}
            legacy_v2["schema"] = "astrid.edge_self_change.operator_status.v2"
            encoded = json.dumps(
                legacy_v2,
                sort_keys=True,
                separators=(",", ":"),
                ensure_ascii=True,
                allow_nan=False,
            ).encode("ascii")
            envelope = {
                "schema": "astrid.edge_self_change.operator_status_envelope.v1",
                "core": legacy_v2,
                "core_sha256": hashlib.sha256(encoded).hexdigest(),
            }
            path.write_text(json.dumps(envelope), encoding="utf-8")
            path.chmod(0o640)

            self.assertEqual(
                REPORT.read_self_change_operator_status(
                    path, test_only_allow_unprivileged_owner=True
                )["pipeline_phase"],
                "probation",
            )
            legacy = dict(legacy_v2)
            legacy["schema"] = "astrid.edge_self_change.operator_status.v1"
            legacy.pop("restart_expectation")
            legacy_encoded = json.dumps(
                legacy,
                sort_keys=True,
                separators=(",", ":"),
                ensure_ascii=True,
                allow_nan=False,
            ).encode("ascii")
            path.write_text(
                json.dumps(
                    {
                        "schema": (
                            "astrid.edge_self_change.operator_status_envelope.v1"
                        ),
                        "core": legacy,
                        "core_sha256": hashlib.sha256(
                            legacy_encoded
                        ).hexdigest(),
                    }
                ),
                encoding="utf-8",
            )
            path.chmod(0o640)
            self.assertEqual(
                REPORT.read_self_change_operator_status(
                    path, test_only_allow_unprivileged_owner=True
                )["pipeline_phase"],
                "probation",
            )

            path.write_text(json.dumps(envelope), encoding="utf-8")
            path.chmod(0o640)
            envelope["core"]["pipeline_phase"] = "accepted"
            path.write_text(json.dumps(envelope), encoding="utf-8")
            path.chmod(0o640)
            self.assertEqual(
                REPORT.read_self_change_operator_status(
                    path, test_only_allow_unprivileged_owner=True
                ),
                {},
            )

            write_operator_projection(path, [operator_lifecycle_event()])
            path.chmod(0o600)
            self.assertEqual(
                REPORT.read_self_change_operator_status(
                    path, test_only_allow_unprivileged_owner=True
                ),
                {},
            )

    def test_configured_self_change_root_resolves_relative_to_appliance_home(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            workspace = home / ".astrid-icp/state/home/default/edge"
            workspace.mkdir(parents=True)
            resolved = REPORT.configured_self_change_root(
                workspace,
                {
                    "ASTRID_EDGE_SELF_CHANGE_ROOT": (
                        ".astrid-icp/state/self-change"
                    )
                },
                home,
            )
        self.assertEqual(
            resolved, home / ".astrid-icp/state/self-change"
        )

    def test_at_a_glance_hides_verified_source_bodies(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            research = workspace / "research"
            research.mkdir()
            (research / "source_1.md").write_text(
                "secret fetched source body must never render"
            )
            (research / "research_1.md").write_text("Astrid-authored synthesis")

            artifacts = GLANCE.newest_artifacts(workspace, maximum=2)

        rendered = "\n".join(artifacts)
        self.assertIn("source_1.md — verified source artifact (body hidden)", rendered)
        self.assertNotIn("secret fetched source body", rendered)
        self.assertIn("Astrid-authored synthesis", rendered)

    def test_at_a_glance_artifacts_reject_links_and_oversized_content(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workspace = root / "workspace"
            research = workspace / "research"
            research.mkdir(parents=True)
            outside = root / "operator-secret.txt"
            outside.write_text("operator secret must not render", encoding="utf-8")
            (research / "escape.md").symlink_to(outside)
            os.link(outside, research / "hardlink.md")
            (research / "oversized.md").write_bytes(
                b"x" * (GLANCE.ARTIFACT_READ_MAX_BYTES + 1)
            )
            (research / "valid.md").write_text(
                "bounded Astrid-owned artifact", encoding="utf-8"
            )
            outside_directory = root / "outside-journal"
            outside_directory.mkdir()
            (outside_directory / "entry.md").write_text(
                "directory escape must not render", encoding="utf-8"
            )
            (workspace / "journal").symlink_to(
                outside_directory, target_is_directory=True
            )

            artifacts = GLANCE.newest_artifacts(workspace, maximum=10)

        rendered = "\n".join(artifacts)
        self.assertIn("research/valid.md — bounded Astrid-owned artifact", rendered)
        for forbidden in (
            "escape.md",
            "hardlink.md",
            "oversized.md",
            "operator secret",
            "directory escape",
        ):
            self.assertNotIn(forbidden, rendered)

    def test_at_a_glance_allows_trusted_workspace_ancestor_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            real_workspace = root / "ssd/workspace"
            (real_workspace / "journal").mkdir(parents=True)
            (real_workspace / "journal/entry.md").write_text(
                "SSD-backed owned artifact", encoding="utf-8"
            )
            workspace_link = root / "workspace"
            workspace_link.symlink_to(real_workspace, target_is_directory=True)

            artifacts = GLANCE.newest_artifacts(workspace_link, maximum=1)

        self.assertEqual(
            artifacts,
            ["journal/entry.md — SSD-backed owned artifact"],
        )

    def test_at_a_glance_rejects_artifact_changed_during_bounded_read(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            artifact = directory / "entry.md"
            artifact.write_text("stable before race", encoding="utf-8")
            before = artifact.stat()
            changed = types.SimpleNamespace(
                st_dev=before.st_dev,
                st_ino=before.st_ino,
                st_mode=before.st_mode,
                st_nlink=before.st_nlink,
                st_uid=before.st_uid,
                st_gid=before.st_gid,
                st_size=before.st_size,
                st_mtime_ns=before.st_mtime_ns + 1,
                st_ctime_ns=before.st_ctime_ns,
            )
            directory_descriptor = os.open(
                directory,
                os.O_RDONLY | getattr(os, "O_DIRECTORY", 0),
            )
            try:
                with mock.patch.object(
                    GLANCE.os,
                    "fstat",
                    side_effect=(before, changed),
                ):
                    body = GLANCE._read_stable_artifact(
                        directory_descriptor, "entry.md", before
                    )
            finally:
                os.close(directory_descriptor)

        self.assertIsNone(body)

    def test_operator_reports_use_only_the_immutable_launcher_root(self) -> None:
        self.assertEqual(
            GLANCE.IMMUTABLE_OPERATOR_ROOT,
            Path("/usr/libexec/astrid-edge/operator"),
        )
        self.assertEqual(
            GLANCE.workspace_for(Path("/home/avado")),
            ("AVADO", Path("/home/avado/.astrid/home/default/edge")),
        )

    def test_system_manager_marker_requires_root_owned_exact_identity(self) -> None:
        marker = mock.Mock()
        marker.stat.return_value = types.SimpleNamespace(
            st_mode=stat.S_IFREG | 0o640,
            st_uid=0,
            st_nlink=1,
        )
        marker.read_text.return_value = json.dumps(
            {
                "schema": "astrid.edge.service_manager.v2",
                "manager": "system",
                "runtime_user": "avado",
            }
        )
        with mock.patch.object(
            REPORT.pwd,
            "getpwuid",
            return_value=types.SimpleNamespace(pw_name="avado"),
        ):
            self.assertTrue(REPORT.system_service_manager_enabled(marker))
            marker.stat.return_value = types.SimpleNamespace(
                st_mode=stat.S_IFREG | 0o660,
                st_uid=0,
                st_nlink=1,
            )
            self.assertFalse(REPORT.system_service_manager_enabled(marker))

    def test_report_subprocesses_ignore_mutable_path_and_python_imports(self) -> None:
        completed = types.SimpleNamespace(stdout="active\n")
        with mock.patch.dict(
            REPORT.os.environ,
            {"PATH": "/tmp/hostile", "PYTHONPATH": "/tmp/inject"},
            clear=True,
        ), mock.patch.object(
            REPORT.subprocess, "run", return_value=completed
        ) as run:
            self.assertEqual(
                REPORT.command("systemctl", "show", "astrid.service"),
                "active",
            )
        arguments = run.call_args.args[0]
        environment = run.call_args.kwargs["env"]
        self.assertEqual(arguments[0], "/usr/bin/systemctl")
        self.assertEqual(environment["PATH"], "/usr/bin:/bin")
        self.assertNotIn("PYTHONPATH", environment)

    def test_owner_text_surfaces_neutralize_terminal_controls(self) -> None:
        hostile = "value\x1b]52;c;Zm9v\x07\x1b[2J\x9b31m\u202ereversed"
        with mock.patch("sys.stdout", new_callable=io.StringIO) as output:
            REPORT.emit("field", hostile + "\ninjected=value")
        rendered = output.getvalue()
        self.assertEqual(rendered.count("\n"), 1)
        for control in ("\x1b", "\x07", "\x9b", "\u202e"):
            self.assertNotIn(control, rendered)
            self.assertNotIn(control, GLANCE.compact(hostile))

        decoded = {"summary": hostile}
        self.assertEqual(decoded["summary"], hostile)
        safe = GLANCE.terminal_safe_value(decoded)
        self.assertEqual(decoded["summary"], hostile)
        self.assertNotIn("\x1b", safe["summary"])

    def test_origin_mac_retirement_receipt_is_exact_scope_and_hash_bound(self) -> None:
        # The retirement migration verifies every ancestor and correctly
        # rejects Linux's world-writable /tmp.  Exercise it below the private
        # checkout instead of weakening that production invariant.
        with tempfile.TemporaryDirectory(
            dir=Path(__file__).resolve().parent
        ) as temporary:
            root = Path(temporary).resolve()
            workspace = root / "home/default"
            operator = root / "operator"
            retirement = root / "retirement"
            workspace.mkdir(parents=True)
            legacy = (
                Path(__file__).parents[1]
                / "packaging/headless/introspection-memory.md"
            ).read_bytes()
            digest = hashlib.sha256(legacy).hexdigest()
            (workspace / "MEMORY.md").write_bytes(legacy)
            RETIRE.migrate(workspace, operator, retirement)
            path = operator / RETIRE.OPERATOR_RECEIPT_NAME
            valid = REPORT.origin_mac_retirement_summary(
                path, root_uid=os.geteuid(), root_gid=os.getegid()
            )
            self.assertTrue(valid["valid"])
            self.assertEqual(valid["retired_count"], 1)
            self.assertEqual(valid["already_retired_count"], 1)
            self.assertEqual(valid["artifacts_verified"], 1)
            self.assertTrue(valid["transaction_valid"])
            self.assertEqual(valid["status"], "verified_exact_scope_already_retired")
            current = workspace / "AGENTS.md"
            current.write_text("A newly revised appliance-local prompt.\n")
            still_valid = REPORT.origin_mac_retirement_summary(
                path, root_uid=os.geteuid(), root_gid=os.getegid()
            )
            self.assertTrue(still_valid["valid"])
            current.write_text("Read introspections/origin-mac/reintroduced.md\n")
            reintroduced = REPORT.origin_mac_retirement_summary(
                path, root_uid=os.geteuid(), root_gid=os.getegid()
            )
            self.assertFalse(reintroduced["valid"])
            self.assertEqual(
                reintroduced["status"], "invalid_current_origin_mac_affordance"
            )
            current.write_text("A safely revised appliance-local prompt.\n")
            artifact = retirement / f"MEMORY.md.{digest}"
            artifact.chmod(0o600)
            artifact.write_bytes(b"tampered")
            invalid = REPORT.origin_mac_retirement_summary(
                path, root_uid=os.geteuid(), root_gid=os.getegid()
            )
            self.assertFalse(invalid["valid"])
            self.assertEqual(invalid["status"], "invalid_durable_retirement")

    def test_inquiry_train_summary_keeps_authored_and_machine_events_distinct(self) -> None:
        report = {
            "schema": REPORT.INQUIRY_TRAIN_REPORT_SCHEMA,
            "integrity": "full_signed_hash_chain_verified",
            "appliance_id": "avado-edge",
            "key_id": "ed25519:test",
            "events": [
                {
                    "timestamp_unix_ms": 1_000,
                    "kind": "inquiry_step",
                    "step_id": "step-one",
                    "thread_id": "thread-one",
                    "thread_operation": "open",
                    "confidence": "tentative",
                    "observation": "A bounded observation.",
                    "interpretation": "A bounded interpretation.",
                    "uncertainty": "A bounded uncertainty.",
                    "decision": "A bounded decision.",
                    "belief_operation": "propose",
                    "belief_id": "belief-one",
                    "trigger_kind": "scheduled",
                },
                {
                    "timestamp_unix_ms": 1_001,
                    "kind": "evidence_arrival",
                    "evidence_id": "evidence-one",
                    "authored": False,
                },
                {
                    "timestamp_unix_ms": 1_002,
                    "kind": "semantic_admission",
                    "status": "acknowledged",
                    "authored": False,
                },
            ],
        }
        summary = REPORT.inquiry_train_summary(report, cutoff_ms=900)
        self.assertEqual(summary["window_inquiry_step_count"], 1)
        self.assertEqual(summary["window_evidence_arrival_count"], 1)
        self.assertEqual(summary["latest_step_id"], "step-one")
        self.assertEqual(summary["latest_reservoir_delivery"], "acknowledged")

    def test_inquiry_train_summary_exposes_fail_closed_path(self) -> None:
        report = {
            "schema": REPORT.INQUIRY_TRAIN_REPORT_SCHEMA,
            "integrity": "invalid_protected_history",
            "invalid_records": [
                {
                    "path": "/protected/segment.jsonl",
                    "reason": "torn tail",
                }
            ],
            "events": [
                {
                    "timestamp_unix_ms": 1_000,
                    "kind": "integrity_violation",
                    "authored": False,
                }
            ],
        }
        summary = REPORT.inquiry_train_summary(report, cutoff_ms=900)
        self.assertEqual(summary["invalid_record_count"], 1)
        self.assertEqual(
            summary["latest_invalid_path"], "/protected/segment.jsonl"
        )
        self.assertEqual(summary["latest_invalid_reason"], "torn tail")
        self.assertEqual(summary["window_integrity_violation_count"], 1)

    def test_at_a_glance_train_requires_exact_sealed_schema(self) -> None:
        completed = types.SimpleNamespace(
            returncode=0,
            stdout=json.dumps({"schema": "older", "events": []}),
        )
        with mock.patch.object(
            GLANCE.subprocess, "run", return_value=completed
        ) as run:
            result = GLANCE.inquiry_train(
                Path("/sealed/astrid-train"), Path("/workspace"), 60, 20
            )
        self.assertNotIn("--workspace", run.call_args.args[0])
        self.assertEqual(
            result["integrity"], "pre-bootstrap/untrusted-report-surface"
        )

    def test_at_a_glance_preserves_sealed_fail_closed_diagnostics(self) -> None:
        completed = types.SimpleNamespace(
            returncode=2,
            stdout=json.dumps(
                {
                    "schema": GLANCE.INQUIRY_TRAIN_SCHEMA,
                    "integrity": "invalid_protected_history",
                    "invalid_records": [
                        {"path": "/protected/head.json", "reason": "bad signature"}
                    ],
                    "events": [],
                }
            ),
        )
        with mock.patch.object(GLANCE.subprocess, "run", return_value=completed):
            result = GLANCE.inquiry_train(
                Path("/sealed/astrid-train"), Path("/workspace"), 60, 20
            )
        self.assertEqual(result["integrity"], "invalid_protected_history")
        self.assertEqual(result["invalid_records"][0]["path"], "/protected/head.json")


if __name__ == "__main__":
    unittest.main()
