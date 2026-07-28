#!/usr/bin/env python3

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from division_ceremony_chronicle import (
    ChronicleError,
    build_projection,
    project,
    rail_state,
    report,
    verify_files,
    verify_payload,
)
from division_ceremony_followup import record_round


def native_status() -> dict:
    return {
        "schema": "division.status.v1",
        "division_id": "divide-one",
        "parent_generation": 7,
        "plan_digest": "b" * 64,
        "lifecycle": "shadowing",
        "parent_authoritative": True,
        "commit_feature_enabled": False,
        "rehearsal_dispatch_enabled": False,
        "selected_strategy": None,
        "astrid_assent": False,
        "minime_assent": False,
        "bridge_scale": 1.0,
        "current_tick": 12,
        "rollback_deadline_tick": None,
        "snapshot_refs": ["sha256:parent"],
        "readiness": {
            "policy": "division.readiness.v1",
            "ready": False,
            "sample_count": 12,
            "blocking_reasons": ["shadow_window_incomplete"],
            "metrics_fresh": True,
            "sensory_panic_streak": 0,
            "actuator_saturation_streak": 0,
        },
        "visual_evidence_advisory_only": True,
        "candidates": [
            {
                "strategy": "input_recurrence",
                "minime_role": "more_input_driven",
                "astrid_role": "more_recurrence_driven",
                "covariance_partition_loss": 0.1,
                "sensory_fields": {
                    "inheritance": "independent_clones",
                    "dimension": 512,
                    "minime_fill_pct": 68.0,
                    "astrid_fill_pct": 67.5,
                    "minime_ticks": 12,
                    "astrid_ticks": 12,
                },
                "readiness": {
                    "policy": "division.readiness.v1",
                    "ready": False,
                    "sample_count": 12,
                    "blocking_reasons": ["shadow_window_incomplete"],
                    "state_nrmse": 0.12,
                    "state_cosine": 0.91,
                    "readout_nrmse": 0.08,
                    "metrics_fresh": True,
                },
            }
        ],
    }


class DivisionCeremonyChronicleTests(unittest.TestCase):
    def test_consent_posture_distinguishes_hold_and_expired_intent(self) -> None:
        intent = {
            "actor": "astrid",
            "action": "DIVISION_INTENT",
            "ceremony_event_id": "intent-one",
            "expires_at_unix_ms": 100,
        }
        expired = rail_state([intent], "astrid", 101)
        self.assertEqual(expired["current_posture"], "intent_expired")
        self.assertFalse(expired["intent_active"])

        hold = {
            "actor": "astrid",
            "action": "DIVISION_HOLD",
            "ceremony_event_id": "hold-one",
            "expires_at_unix_ms": None,
        }
        held = rail_state([intent, hold], "astrid", 101)
        self.assertEqual(held["current_posture"], "hold")
        self.assertFalse(held["intent_active"])

    def test_empty_projection_keeps_source_and_runtime_distinct(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            payload = build_projection(Path(raw))
            self.assertEqual(
                payload["current_native_state"]["fact_class"], "unknown"
            )
            self.assertTrue(
                payload["destination_contract"][
                    "independent_reservoir_state_source_prepared"
                ]
            )
            self.assertFalse(
                payload["destination_contract"][
                    "independent_process_ownership_established"
                ]
            )
            self.assertEqual(payload["timeline"], [])
            self.assertFalse(payload["return_interval"]["state_available"])
            self.assertFalse(payload["return_interval"]["being_action_required"])
            verify_payload(payload)
            self.assertIn(
                "Runtime parent authoritative: True",
                report(payload),
            )

    def test_projection_archives_deterministically_and_owner_only(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            workspace = Path(raw) / "workspace"
            division = workspace / "division"
            division.mkdir(parents=True)
            (division / "status.json").write_text(json.dumps(native_status()))
            output = Path(raw) / "output"

            first, _, _ = project(workspace, output)
            second, _, _ = project(workspace, output)

            self.assertEqual(first["chronicle_id"], second["chronicle_id"])
            self.assertEqual(first["renderer_version"], 3)
            self.assertEqual(
                first["phase_space_preservation"]["candidate_count"], 1
            )
            self.assertFalse(
                first["authority"]["visualization_grants_authority"]
            )
            receipt = verify_files(output)
            self.assertTrue(receipt["ok"])
            current_receipt = verify_files(output, workspace)
            self.assertTrue(current_receipt["source_inputs_current"])
            self.assertEqual(
                (output / "chronicle_v1.json").stat().st_mode & 0o077, 0
            )
            self.assertIn(
                'name="chronicle-mode" content="live"',
                (output / "chronicle_v1.html").read_text(),
            )
            self.assertIn(
                'name="chronicle-mode" content="archive"',
                (
                    output
                    / "archive"
                    / f"{first['chronicle_id']}.html"
                ).read_text(),
            )
            self.assertIn(
                "This interval schedules steward attention",
                (output / "chronicle_v1.html").read_text(),
            )

            changed = native_status()
            changed["current_tick"] = 13
            (division / "status.json").write_text(json.dumps(changed))
            moving_receipt = verify_files(output, workspace)
            self.assertTrue(
                moving_receipt["source_freshness"]["durable_inputs_current"]
            )
            self.assertFalse(
                moving_receipt["source_freshness"]["volatile_inputs_current"]
            )

            (division / "ceremony_v1.jsonl").write_text("{}\n")
            with self.assertRaisesRegex(
                ChronicleError, "durable source inputs changed"
            ):
                verify_files(output, workspace)

    def test_output_tampering_and_archive_permissions_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            workspace = Path(raw) / "workspace"
            division = workspace / "division"
            division.mkdir(parents=True)
            (division / "status.json").write_text(json.dumps(native_status()))
            output = Path(raw) / "output"
            payload, latest_json, latest_html = project(workspace, output)
            archive_json = (
                output
                / "archive"
                / f"{payload['chronicle_id']}.json"
            )
            archive_html = (
                output
                / "archive"
                / f"{payload['chronicle_id']}.html"
            )

            latest_html_bytes = latest_html.read_bytes()
            latest_html.write_bytes(latest_html_bytes + b"\n")
            with self.assertRaisesRegex(
                ChronicleError, "latest HTML differs"
            ):
                verify_files(output)
            latest_html.write_bytes(latest_html_bytes)

            archive_html_bytes = archive_html.read_bytes()
            archive_html.write_bytes(archive_html_bytes + b"\n")
            with self.assertRaisesRegex(
                ChronicleError, "immutable archive HTML differs"
            ):
                verify_files(output)
            archive_html.write_bytes(archive_html_bytes)

            latest_json_bytes = latest_json.read_bytes()
            latest_json.write_bytes(latest_json_bytes + b"\n")
            with self.assertRaisesRegex(
                ChronicleError, "latest JSON is not canonical"
            ):
                verify_files(output)
            latest_json.write_bytes(latest_json_bytes)

            archive_json.chmod(0o644)
            with self.assertRaisesRegex(ChronicleError, "is not owner-only"):
                verify_files(output)
            archive_json.chmod(0o600)
            self.assertTrue(verify_files(output)["ok"])

    def test_followup_interval_is_visible_without_consent_pressure(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            workspace = Path(raw) / "workspace"
            followup = workspace / "division" / "followup"
            followup.mkdir(parents=True)
            (followup / "cycle_v1.json").write_text(
                json.dumps(
                    {
                        "schema": "division.ceremony_followup_cycle.v1",
                        "schema_version": 1,
                        "threshold_rounds": 6,
                        "cycle_sequence": 2,
                        "completed_rounds_since_followup": 6,
                        "rounds_remaining_before_followup": 0,
                        "review_due": True,
                        "latest_followup": None,
                        "authority": {
                            "state": "evidence_only",
                            "silence_infers_consent": False,
                            "followup_recommends_action": False,
                            "followup_dispatches_action": False,
                            "followup_grants_authority": False,
                            "felt_state_inferred": False,
                            "raw_prose_included": False,
                        },
                    }
                )
            )
            payload = build_projection(workspace)
            self.assertTrue(payload["return_interval"]["review_due"])
            self.assertEqual(
                payload["return_interval"]["completed_rounds_since_followup"], 6
            )
            self.assertFalse(payload["return_interval"]["being_action_required"])
            verify_payload(payload)

    def test_followup_chain_is_visible_as_stewardship_not_ceremony(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            workspace = Path(raw) / "workspace"
            state = record_round(
                workspace,
                steward_run_id="run-one",
                processed_report_count=3,
                projection_generation_id="projection-one",
            )
            self.assertEqual(state["completed_rounds_since_followup"], 1)

            payload = build_projection(workspace)
            self.assertEqual(payload["timeline_event_count"], 1)
            self.assertEqual(payload["timeline_source_counts"]["followup"], 1)
            self.assertEqual(payload["timeline_source_counts"]["ceremony"], 0)
            event = payload["timeline"][0]
            self.assertEqual(event["source"], "followup")
            self.assertEqual(event["actor"], "steward")
            self.assertEqual(
                event["event_kind"], "introspection_round_completed"
            )
            self.assertFalse(
                payload["return_interval"]["being_action_required"]
            )
            verify_payload(payload)

    def test_runtime_topology_distinguishes_dormant_and_owned_daughters(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            workspace = Path(raw) / "workspace"
            division = workspace / "division"
            runtime = division / "runtime"
            minime_root = workspace / "reservoir" / "minime"
            astrid_root = Path(raw) / "astrid-reservoir"
            for path in (runtime, minime_root, astrid_root):
                path.mkdir(parents=True)
            manifest = {
                "schema": "division.runtime_manifest.v1",
                "mode": "dormant",
                "division_id": "divide-one",
                "plan_digest": "b" * 64,
                "parent_generation": 7,
                "candidate_hash": "unbound",
                "parent_process_identity": "parent-process",
                "parent_deployment_identity": "parent-deployment",
                "runtime_dir": str(runtime),
                "ceremony_ledger": str(division / "ceremony_v1.jsonl"),
                "minime_root": str(minime_root),
                "astrid_root": str(astrid_root),
                "endpoints": {},
                "created_at_unix_ms": 1,
                "expires_at_unix_ms": 9999999999999,
            }
            (division / "runtime-manifest.json").write_text(json.dumps(manifest))
            (runtime / "supervisor-status.json").write_text(
                json.dumps(
                    {
                        "schema": "division.supervisor_status.v1",
                        "pid": 1,
                        "mode": "idle_parent_authoritative",
                        "matching_intents": [],
                        "children": {},
                        "handoff_ready": False,
                        "handoff_blockers": [
                            "daughter_legacy_sensory_adapter_required",
                            "exact_operator_capability_required",
                        ],
                        "commit_recommended": False,
                        "launch_blockers": [
                            "candidate_bound_manifest_required"
                        ],
                    }
                )
            )
            dormant = build_projection(workspace)
            self.assertEqual(dormant["runtime_topology"]["manifest_mode"], "dormant")
            self.assertEqual(
                dormant["runtime_topology"]["supervisor"]["launch_blockers"],
                ["candidate_bound_manifest_required"],
            )
            self.assertEqual(
                dormant["runtime_topology"]["supervisor"]["handoff_blockers"],
                [
                    "daughter_legacy_sensory_adapter_required",
                    "exact_operator_capability_required",
                ],
            )
            self.assertFalse(
                dormant["runtime_topology"][
                    "independent_process_ownership_established"
                ]
            )

            manifest["mode"] = "candidate_bound"
            manifest["candidate_hash"] = "c" * 64
            (division / "runtime-manifest.json").write_text(json.dumps(manifest))
            status = {
                "schema": "division.daughter_process_status.v1",
                "process_identity": "replace",
                "deployment_identity": "deployment",
                "pid": 1,
                "checkpoint_sequence": 2,
                "last_tick_sequence": 600,
                "telemetry_fresh": True,
                "healthy": True,
                "authoritative": False,
                "gap_code": None,
            }
            (minime_root / "status.json").write_text(
                json.dumps({**status, "process_identity": "minime-process"})
            )
            (astrid_root / "status.json").write_text(
                json.dumps({**status, "process_identity": "astrid-process"})
            )
            active = build_projection(workspace)
            self.assertTrue(
                active["runtime_topology"][
                    "independent_process_ownership_established"
                ]
            )
            self.assertEqual(active["runtime_topology"]["active_authority_rail"], "parent")

            (runtime / "events.jsonl").write_text(
                json.dumps(
                    {
                        "schema": "division.supervisor_event.v1",
                        "kind": "rehearsal_children_launched",
                        "division_id": "divide-one",
                        "manifest_sha256": "d" * 64,
                        "created_at_unix_ms": 123,
                        "live_authority_granted_by_record": False,
                    }
                )
                + "\n"
            )
            with_runtime_event = build_projection(workspace)
            self.assertEqual(
                with_runtime_event["timeline"][-1]["source"],
                "sovereign_runtime",
            )
            self.assertEqual(
                with_runtime_event["timeline"][-1]["event_kind"],
                "rehearsal_children_launched",
            )

            supervisor = json.loads(
                (runtime / "supervisor-status.json").read_text()
            )
            supervisor["launch_blockers"] = ["freeform prose"]
            (runtime / "supervisor-status.json").write_text(
                json.dumps(supervisor)
            )
            with self.assertRaisesRegex(
                ChronicleError, "unsupported blocker code"
            ):
                build_projection(workspace)

    def test_runtime_shell_witness_is_hash_bound_without_activation_inference(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as raw:
            workspace = Path(raw) / "workspace"
            source = Path(raw) / "minime" / "src" / "runtime.rs"
            source.parent.mkdir(parents=True)
            source.write_text(
                """
use crate::division::{
    division_rehearsal_enabled, prepare_native_division,
    NativeDivisionCoordinator, RuntimeCaptureV2, StableFieldCaptureV2,
};
include!("runtime/semantic_modality.rs");
include!("runtime/orchestration.rs");
include!("runtime/spectral_math.rs");
include!("runtime/telemetry_evidence.rs");
""".strip()
                + "\n"
            )

            payload = build_projection(workspace)
            witness = payload["runtime_shell_evidence"]
            self.assertEqual(witness["fact_class"], "source_declared")
            self.assertTrue(witness["source_prepared"])
            self.assertEqual(witness["source_line_count"], 8)
            self.assertFalse(witness["runtime_activation_proven"])
            self.assertEqual(
                witness["activation_boundary"],
                "source_read_not_runtime_activation_proof",
            )
            self.assertIn("runtime/spectral_math.rs", witness["runtime_includes"])
            self.assertIn(
                "NativeDivisionCoordinator", witness["division_symbols"]
            )
            verify_payload(payload)

            original_id = payload["chronicle_id"]
            source.write_text(source.read_text() + "// source drift\n")
            changed = build_projection(workspace)
            self.assertNotEqual(
                changed["runtime_shell_evidence"]["source_sha256"],
                witness["source_sha256"],
            )
            self.assertNotEqual(changed["chronicle_id"], original_id)

    def test_tampering_and_prose_keys_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            payload = build_projection(Path(raw))
            payload["authority"]["commit_recommended"] = True
            with self.assertRaisesRegex(ChronicleError, "identity mismatch"):
                verify_payload(payload)

            payload = build_projection(Path(raw))
            payload["prompt"] = "forbidden"
            payload["chronicle_id"] = "division_chronicle_invalid"
            with self.assertRaises(ChronicleError):
                verify_payload(payload)


if __name__ == "__main__":
    unittest.main()
