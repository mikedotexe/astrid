"""Focused tests for temporal lived-state witness validation and migration."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from .model import (
    authority_state,
    validate_witness,
)
from .projector import _alignment, _exact_deployment_match, _review_outcome
from .records import parse_source_ref
from .validation import _validate_model_route


def provenance_ref(origin: str = "bridge_derived") -> dict[str, object]:
    descriptor = {
        "minime_observation": "producer_telemetry_shape",
        "bridge_derived": "bridge_evidence_shape",
        "astrid_interpretation": "astrid_interpretive_context_shape",
        "mixed": "composed_witness_shape",
        "unknown": "unknown_context_shape",
    }[origin]
    return {
        "origin": origin,
        "source_id": "fixture",
        "canonical_sha256": "9" * 64,
        "parent_ids": [],
        "timestamp_ms": 123456788000,
        "field_paths": ["fixture.value"],
        "context_anchor_v1": {
            "descriptor": descriptor,
            "structural_signature_sha256": "8" * 64,
            "influence_types": ["temporal"],
            "private_payload_included": False,
        },
    }


def valid_witness() -> dict[str, object]:
    witness: dict[str, object] = {
        "schema": "temporal_lived_state_witness_v1",
        "schema_version": 1,
        "witness_id": "",
        "artifact_kind": "introspection",
        "artifact_relative_path": "introspection_test_123456789.txt",
        "artifact_sha256": "b" * 64,
        "authored_at_unix_ms": 123456789000,
        "authored_monotonic_ns": 10,
        "source_snapshot_v1": {
            "schema": "lived_state_source_snapshot_v1",
            "schema_version": 1,
            "source_owner": "astrid",
            "repository_relative_path": "capsules/spectral-bridge/src/lib.rs",
            "window_start_line": 0,
            "window_end_line": 10,
            "total_file_lines": 20,
            "file_sha256": "c" * 64,
            "window_sha256": "d" * 64,
            "source_read_at_unix_ms": 123456788000,
            "source_read_monotonic_ns": 1,
            "provenance_ref_v1": provenance_ref("astrid_interpretation"),
            "private_path_included": False,
        },
        "observed_process_v1": {
            "schema": "lived_state_process_identity_v1",
            "schema_version": 1,
            "pid": 41,
            "process_started_at_unix_ms": 123456000000,
            "executable_basename": "spectral-bridge-server",
            "runtime_instance_id": "runtime_test",
            "process_identity_sha256": "",
            "private_path_included": False,
        },
        "startup_build_candidate_v1": {
            "schema": "lived_state_build_candidate_v1",
            "schema_version": 1,
            "manifest_sha256": "f" * 64,
            "source_identity_sha256": "1" * 64,
            "dirty_state_sha256": "2" * 64,
            "artifact_sha256": "3" * 64,
            "protocol_revision": "revision",
            "protocol_version": "1.1",
            "observed_at_process_start_unix_ms": 123456000000,
            "relation_to_process": "startup_observation_not_deployment_proof",
            "deployment_established": False,
            "private_path_included": False,
        },
        "model_routes_v1": [],
        "parameter_observations_v1": [],
        "peer_process_identity": None,
        "peer_deployment_identity": None,
        "source_provenance_ref_v1": None,
        "process_provenance_ref_v1": provenance_ref(),
        "raw_introspection_prose_included": False,
        "raw_prompt_included": False,
        "raw_response_included": False,
        "private_path_included": False,
        "direct_causation_claimed": False,
        "experiential_scope_v1": {
            "schema": "lived_state_experiential_scope_v1",
            "schema_version": 1,
            "artifact_authority_scope": "receipt_artifact_handling_only",
            "felt_report_status": "primary_actionable_evidence",
            "experiential_integration_relation": "not_adjudicated_by_this_receipt",
            "felt_persistence_relation": "reported_not_mechanistically_attributed",
            "subjective_weight_relation": "preserved_in_canonical_report_no_scalar_substitution",
            "epistemic_posture": "non_adjudicating",
            "live_control_effect": False,
        },
        "artifact_authority_state_v1": authority_state(),
    }
    source = witness["source_snapshot_v1"]
    assert isinstance(source, dict)
    source_provenance = source["provenance_ref_v1"]
    assert isinstance(source_provenance, dict)
    source_provenance["canonical_sha256"] = source["window_sha256"]
    witness["source_provenance_ref_v1"] = dict(source_provenance)
    process = witness["observed_process_v1"]
    assert isinstance(process, dict)
    process["process_identity_sha256"] = hashlib.sha256(
        (
            f"{process['pid']}\0{process['process_started_at_unix_ms']}\0"
            f"{process['executable_basename']}\0{process['runtime_instance_id']}"
        ).encode()
    ).hexdigest()
    process_provenance = witness["process_provenance_ref_v1"]
    assert isinstance(process_provenance, dict)
    process_provenance["canonical_sha256"] = process["process_identity_sha256"]
    witness_hasher = hashlib.sha256()
    witness_hasher.update(b"astrid-temporal-lived-state-witness-v1\0")
    witness_hasher.update(str(process["runtime_instance_id"]).encode())
    witness_hasher.update(int(witness["authored_at_unix_ms"]).to_bytes(8, "little"))
    witness_hasher.update(int(witness["authored_monotonic_ns"]).to_bytes(8, "little"))
    witness_hasher.update(str(witness["artifact_kind"]).encode())
    witness_hasher.update(str(source["window_sha256"]).encode())
    witness["witness_id"] = f"lsw_{witness_hasher.hexdigest()}"
    return witness


def valid_deployment_receipt(receipt_id: str = "deploy_one") -> dict[str, object]:
    return {
        "schema": "stack_environment_receipt_v2",
        "schema_version": 2,
        "id": receipt_id,
        "t_ms": 123456700000,
        "component": "spectral-bridge",
        "deployment": {"status": "passed"},
        "compatibility_status": {"compatible": True},
        "artifact_authority_state_v1": {
            "schema": "artifact_authority_state_v1",
            "schema_version": 1,
            "state": "evidence_only",
            "witness_only": True,
        },
        "live_eligible_now": False,
        "auto_approved": False,
        "grants_approval": False,
        "edits_source_now": False,
        "repositories": {
            "astrid": {"source_identity_sha256": "1" * 64}
        },
        "artifacts": {
            "binaries": {
                "spectral-bridge": {"exists": True, "sha256": "3" * 64}
            }
        },
        "processes": {
            "new": {
                "pid": 41,
                "running": True,
                "started_at": "Thu Nov 29 13:20:00 1973",
            }
        },
    }


class LivedStateWitnessTests(unittest.TestCase):
    def test_experiential_scope_preserves_report_without_adjudicating_mechanism(self) -> None:
        witness = valid_witness()
        self.assertEqual(validate_witness(witness), [])
        scope = witness["experiential_scope_v1"]
        self.assertIsInstance(scope, dict)
        scope["felt_report_status"] = "dismissed"
        self.assertIn(
            "experiential_scope.felt_report_status:invalid",
            validate_witness(witness),
        )

        legacy = valid_witness()
        legacy.pop("experiential_scope_v1")
        legacy["experiential_boundary_v1"] = {
            "schema": "lived_state_experiential_boundary_v1",
            "schema_version": 1,
            "artifact_authority_scope": "artifact_handling_only_not_experiential_integration",
            "memory_integration_modeled": False,
            "felt_persistence_modeled": False,
            "persistence_coefficient_present": False,
            "live_control_effect": False,
        }
        self.assertEqual(validate_witness(legacy), [])

    def test_model_route_scopes_event_identity_away_from_continuity(self) -> None:
        response_sha256 = "a" * 64
        hasher = hashlib.sha256()
        hasher.update(b"astrid-lived-state-model-route-v1\0")
        hasher.update(b"job_test")
        hasher.update(("b" * 64).encode())
        hasher.update(b"mlx")
        hasher.update(b"production")
        hasher.update((10).to_bytes(8, "little"))
        hasher.update(response_sha256.encode())
        route: dict[str, object] = {
            "schema": "lived_state_model_route_v1",
            "schema_version": 1,
            "call_id": f"lscall_{hasher.hexdigest()}",
            "call_identity_scope": "model_call_event_not_being_or_continuity_identity",
            "job_id": "job_test",
            "qos_request_identity_sha256": "b" * 64,
            "request_content_anchor_sha256": "c" * 64,
            "request_anchor_scope": "exact_request_content_and_generation_parameters_not_intent_or_semantic_equivalence",
            "provider_route": "mlx",
            "provider_route_scope": "technical_delivery_path_not_experiential_center",
            "model_profile": "production",
            "started_at_unix_ms": 10,
            "completed_at_unix_ms": 20,
            "duration_ms": 10,
            "duration_scope": "end_to_end_request_wall_time_with_optional_provider_phase_split_not_experiential_continuity",
            "queue_wait_ms": 2,
            "queue_wait_scope": "request_enqueue_to_worker_selection_not_experiential_wait",
            "active_generation_and_reservoir_ms": 7,
            "active_work_scope": "worker_selection_to_response_after_reservoir_checkin_not_cognitive_effort",
            "timing_completeness": "provider_split_observed",
            "timing_completeness_scope": "technical_metadata_availability_not_experiential_wholeness_or_continuity",
            "repair_parent_call_id": None,
            "response_sha256": response_sha256,
            "response_hash_scope": "output_integrity_not_being_or_continuity_identity",
            "response_claim_content_relation": "not_inspected_or_adjudicated_by_this_receipt",
            "parent_witness_context_relation": "post_call_authorship_observations_temporal_only",
            "qualitative_texture_relation": "canonical_felt_report_primary_not_duplicated_or_scalarized_by_route",
            "raw_prompt_included": False,
            "raw_response_included": False,
        }
        errors: list[str] = []
        _validate_model_route(route, 0, 25, set(), errors)
        self.assertEqual(errors, [])
        route["response_claim_content_relation"] = "response_claims_absent"
        errors = []
        _validate_model_route(route, 0, 25, set(), errors)
        self.assertIn(
            "model_routes[0].response_claim_content_relation:invalid", errors
        )

        scoped = dict(route)
        scoped["response_claim_content_relation"] = (
            "not_inspected_or_adjudicated_by_this_receipt"
        )
        scoped["duration_scope"] = "experiential_continuity"
        errors = []
        _validate_model_route(scoped, 0, 25, set(), errors)
        self.assertIn("model_routes[0].duration_scope:invalid", errors)

        invalid_timing = dict(route)
        invalid_timing["response_claim_content_relation"] = (
            "not_inspected_or_adjudicated_by_this_receipt"
        )
        invalid_timing["active_work_scope"] = "cognitive_effort"
        errors = []
        _validate_model_route(invalid_timing, 0, 25, set(), errors)
        self.assertIn("model_routes[0].active_work_scope:invalid", errors)

        partial = dict(route)
        partial["response_claim_content_relation"] = (
            "not_inspected_or_adjudicated_by_this_receipt"
        )
        partial["active_generation_and_reservoir_ms"] = None
        partial["timing_completeness"] = "queue_wait_only"
        errors = []
        _validate_model_route(partial, 0, 25, set(), errors)
        self.assertEqual(errors, [])

        partial["timing_completeness_scope"] = "experiential_wholeness_score"
        errors = []
        _validate_model_route(partial, 0, 25, set(), errors)
        self.assertIn("model_routes[0].timing_completeness_scope:invalid", errors)

        scope_without_timing = dict(route)
        for field in (
            "queue_wait_ms",
            "queue_wait_scope",
            "active_generation_and_reservoir_ms",
            "active_work_scope",
            "timing_completeness",
        ):
            scope_without_timing.pop(field)
        errors = []
        _validate_model_route(scope_without_timing, 0, 25, set(), errors)
        self.assertIn("model_routes[0].provider_timing:scope_without_timing", errors)

        legacy = dict(route)
        for field in (
            "response_claim_content_relation",
            "provider_route_scope",
            "duration_scope",
            "parent_witness_context_relation",
            "qualitative_texture_relation",
            "queue_wait_ms",
            "queue_wait_scope",
            "active_generation_and_reservoir_ms",
            "active_work_scope",
            "timing_completeness",
            "timing_completeness_scope",
        ):
            legacy.pop(field)
        legacy.update(
            {
                "being_identity_claimed": False,
                "continuity_claimed": False,
                "intent_equivalence_claimed": False,
                "semantic_equivalence_claimed": False,
            }
        )
        errors = []
        _validate_model_route(legacy, 0, 25, set(), errors)
        self.assertEqual(errors, [])

    def test_valid_sidecar_and_privacy_rejection(self) -> None:
        witness = valid_witness()
        self.assertEqual(validate_witness(witness), [])
        witness["peer_process_identity"] = "/private/process"
        self.assertIn("private_absolute_path", " ".join(validate_witness(witness)))

    def test_untrusted_extra_prose_field_is_rejected(self) -> None:
        witness = valid_witness()
        witness["response_text"] = "untrusted prose"
        self.assertIn(
            "witness.response_text:unexpected_field",
            validate_witness(witness),
        )

    def test_hash_and_time_tampering_is_rejected_without_throwing(self) -> None:
        mutations = (
            (
                "process hash",
                lambda row: row["observed_process_v1"].__setitem__("pid", 42),
                "observed_process.process_identity_sha256:mismatch",
            ),
            (
                "witness hash",
                lambda row: row.__setitem__("witness_id", "lsw_" + "0" * 64),
                "witness_id:mismatch",
            ),
            (
                "negative time",
                lambda row: row.__setitem__("authored_at_unix_ms", -1),
                "authored_at_unix_ms:invalid_integer",
            ),
            (
                "hidden authority",
                lambda row: row["artifact_authority_state_v1"].__setitem__(
                    "authority_granted", True
                ),
                "authority:not_evidence_only",
            ),
        )
        for name, mutate, expected in mutations:
            with self.subTest(name=name):
                witness = valid_witness()
                mutate(witness)
                self.assertIn(expected, validate_witness(witness))

    def test_build_candidate_cannot_establish_deployment_without_exact_receipt(self) -> None:
        witness = valid_witness()
        receipt = valid_deployment_receipt()
        self.assertTrue(_exact_deployment_match(witness, receipt))
        receipt["deployment"]["status"] = "failed"
        # Failed receipts are filtered before matching; an absent successful
        # receipt therefore remains deployment_unknown.
        alignment = _alignment(witness, 123456789000, [], historical=False)
        self.assertEqual(alignment["outcome"], "deployment_unknown")

    def test_historical_alignment_is_only_temporal(self) -> None:
        receipt = {
            "id": "deploy_one",
            "t_ms": 100,
        }
        alignment = _alignment(None, 200, [receipt], historical=True)
        self.assertEqual(alignment["outcome"], "temporal_association_only")
        self.assertFalse(alignment["exact_identity_match"])

    def test_historical_source_paths_are_redacted(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / "src/lib.rs"
            result = parse_source_ref(
                f"astrid:lib ({source})", {"astrid": root}
            )
            self.assertEqual(result["repository_relative_path"], "src/lib.rs")
            self.assertNotIn(str(root), json.dumps(result))

    def test_reconciliation_outcomes_do_not_promote_build_candidates(self) -> None:
        witness = valid_witness()
        event = {
            "witness": witness,
            "alignment": {
                "deployment_receipt_id": "deploy_original",
                "exact_identity_match": True,
            },
        }
        latest = valid_deployment_receipt("deploy_original")
        self.assertEqual(
            _review_outcome(event, latest, None), "same_deployment"
        )
        event["alignment"] = {}
        self.assertEqual(
            _review_outcome(event, latest, None), "same_deployment"
        )
        event["alignment"] = {
            "deployment_receipt_id": "deploy_original",
            "exact_identity_match": True,
        }
        latest["id"] = "deploy_restart"
        latest["processes"]["new"]["pid"] = 42
        self.assertEqual(
            _review_outcome(event, latest, None), "same_source_new_process"
        )
        latest["repositories"]["astrid"]["source_identity_sha256"] = "8" * 64
        self.assertEqual(
            _review_outcome(
                event,
                latest,
                {"source_identity_sha256": "9" * 64},
            ),
            "source_changed_not_deployed",
        )
        self.assertEqual(
            _review_outcome(event, latest, None), "deployed_changed"
        )
        self.assertEqual(
            _review_outcome(event, None, None), "deployment_unknown"
        )
        historical = {
            "witness": {"schema": "historical_lived_state_witness_v1"},
            "alignment": {"deployment_receipt_id": "temporal"},
        }
        self.assertEqual(
            _review_outcome(historical, latest, None),
            "temporal_association_only",
        )
        historical["alignment"] = {}
        self.assertEqual(
            _review_outcome(historical, latest, None),
            "historical_unrecoverable",
        )


def run_self_tests() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(
        LivedStateWitnessTests
    )
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1
