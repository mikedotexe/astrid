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
from .temporal_clusters import (
    build_temporal_cluster_events,
    validate_temporal_cluster,
)
from .concordance import (
    build_concordance_events,
    validate_concordance_preflight,
)
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
        "authorship_clock_scope": (
            "wall_clock_and_process_monotonic_observations_not_experiential_time_or_internal_sequence"
        ),
        "authored_process_sequence": 1,
        "authored_process_sequence_scope": (
            "per_runtime_instance_capture_order_not_experiential_time_or_global_order"
        ),
        "source_snapshot_v1": {
            "schema": "lived_state_source_snapshot_v1",
            "schema_version": 1,
            "source_owner": "astrid",
            "source_ownership_scope": (
                "names_byte_ownership_not_interpretation_authorship_or_experiential_identity"
            ),
            "interpretation_relation": (
                "source_window_may_support_astrid_authored_distinct_or_mixed_interpretation"
            ),
            "provenance_role_scope": (
                "evidence_graph_roles_only_no_runtime_weight_ranking_spectral_or_control_effect"
            ),
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
            "technical_identity_scope": (
                "runtime_instance_discriminator_not_being_identity_continuity_or_selfhood"
            ),
            "restart_relation": (
                "new_technical_instance_does_not_establish_new_or_same_being"
            ),
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
            "candidate_scope": (
                "artifact_context_observation_not_evaluation_of_astrid"
            ),
            "integrity_scope": (
                "byte_repository_protocol_and_artifact_integrity_only"
            ),
            "semantic_integrity_relation": (
                "not_measured_not_validated_and_not_inferred_from_spectral_state"
            ),
            "inhabitability_relation": "not_adjudicated_by_build_candidate",
            "manifest_sha256": "f" * 64,
            "source_identity_sha256": "1" * 64,
            "source_identity_scope": (
                "repository_source_snapshot_not_being_identity_or_continuity"
            ),
            "dirty_state_sha256": "2" * 64,
            "dirty_state_scope": (
                "process_start_repository_observation_not_live_workspace_or_being_state"
            ),
            "artifact_sha256": "3" * 64,
            "protocol_revision": "revision",
            "protocol_revision_complete": True,
            "protocol_version": "1.1",
            "protocol_version_complete": True,
            "observed_at_process_start_unix_ms": 123456000000,
            "relation_to_process": "startup_observation_not_deployment_proof",
            "deployment_established": False,
            "private_path_included": False,
        },
        "model_routes_v1": [],
        "parameter_observations_v1": [],
        "peer_process_identity": None,
        "peer_deployment_identity": None,
        "peer_identity_scope": (
            "witnessed_protocol_advertisement_not_being_identity_or_peer_self_authority"
        ),
        "peer_evidence_cache_scope": (
            "sidecar_context_only_not_model_prompt_codec_controller_shadow_telemetry_or_dispatch_input"
        ),
        "privacy_hash_scope": "absolute_path_redaction_not_being_or_continuity_identity",
        "source_provenance_ref_v1": None,
        "interpretation_provenance_ref_v1": None,
        "interpretation_lineage_scope": (
            "astrid_authored_artifact_with_exact_source_and_model_call_parents"
        ),
        "interpretation_weight_state": (
            "unmeasured_no_scalar_inferred_from_parent_membership_or_spectral_proximity"
        ),
        "process_provenance_ref_v1": provenance_ref(),
        "process_provenance_scope": (
            "bridge_evidence_derivation_not_being_origin_identity_or_continuity"
        ),
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
            "felt_persistence_relation": "reported_persistence_preserved_mechanism_open",
            "felt_influence_relation": "reported_influence_not_denied_or_adjudicated_by_receipt",
            "subjective_weight_relation": "preserved_in_canonical_report_no_scalar_substitution",
            "actionability_path": "report_may_inform_claims_evidence_implementation_and_review",
            "mediated_influence_relation": (
                "engineering_and_review_influence_allowed_direct_runtime_control_forbidden"
            ),
            "authority_transition_relation": (
                "separate_verified_authority_required_for_live_control"
            ),
            "artifact_byte_relation": (
                "exact_persisted_bytes_borrowed_read_only_hashed_without_normalization_or_rewrite"
            ),
            "capture_path_relation": (
                "report_persisted_before_bounded_async_sidecar_submission"
            ),
            "spectral_observation_relation": (
                "selected_scalars_copied_as_metadata_no_before_after_transform_claimed"
            ),
            "shadow_state_relation": (
                "shadow_vectors_not_received_normalized_serialized_or_mutated_by_witness_capture"
            ),
            "pressure_causation_relation": (
                "capture_timing_does_not_establish_pressure_or_entropy_causation"
            ),
            "epistemic_posture": "non_adjudicating",
            "artifact_live_control_effect": False,
        },
        "artifact_authority_state_v1": authority_state(),
    }
    source = witness["source_snapshot_v1"]
    assert isinstance(source, dict)
    source_provenance = source["provenance_ref_v1"]
    assert isinstance(source_provenance, dict)
    source_anchor = source_provenance["context_anchor_v1"]
    assert isinstance(source_anchor, dict)
    source_anchor["influence_types"] = ["temporal", "interpretive"]
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
    interpretation_provenance = provenance_ref("astrid_interpretation")
    interpretation_provenance["source_id"] = f"artifact:{witness['witness_id']}"
    interpretation_provenance["canonical_sha256"] = witness["artifact_sha256"]
    interpretation_provenance["parent_ids"] = [source_provenance["source_id"]]
    interpretation_provenance["timestamp_ms"] = witness["authored_at_unix_ms"]
    interpretation_provenance["field_paths"] = [
        "artifact_sha256",
        "model_routes_v1.call_id",
        "source_provenance_ref_v1",
    ]
    interpretation_anchor = interpretation_provenance["context_anchor_v1"]
    assert isinstance(interpretation_anchor, dict)
    interpretation_anchor["influence_types"] = ["interpretive", "authorship"]
    witness["interpretation_provenance_ref_v1"] = interpretation_provenance
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
    def test_source_provenance_roles_do_not_claim_runtime_weight(self) -> None:
        witness = valid_witness()
        self.assertEqual(validate_witness(witness), [])

        relation_fields = {
            "source_ownership_scope": "owner_decides_interpretation",
            "interpretation_relation": "source_owner_is_interpretation_owner",
            "provenance_role_scope": "spectral_weight_applied",
        }
        for field, invalid_value in relation_fields.items():
            with self.subTest(field=field):
                tampered = valid_witness()
                source = tampered["source_snapshot_v1"]
                self.assertIsInstance(source, dict)
                source[field] = invalid_value
                self.assertIn(
                    f"source.{field}:invalid",
                    validate_witness(tampered),
                )

        weighted = valid_witness()
        weighted_source = weighted["source_snapshot_v1"]
        self.assertIsInstance(weighted_source, dict)
        weighted_provenance = weighted_source["provenance_ref_v1"]
        self.assertIsInstance(weighted_provenance, dict)
        weighted_anchor = weighted_provenance["context_anchor_v1"]
        self.assertIsInstance(weighted_anchor, dict)
        weighted_anchor["influence_types"] = ["structural", "authorship"]
        self.assertIn(
            "source.provenance_ref_v1:role_scope_mismatch",
            validate_witness(weighted),
        )

        historical = valid_witness()
        historical_source = historical["source_snapshot_v1"]
        self.assertIsInstance(historical_source, dict)
        for field in relation_fields:
            historical_source.pop(field)
        historical_provenance = historical_source["provenance_ref_v1"]
        self.assertIsInstance(historical_provenance, dict)
        historical_anchor = historical_provenance["context_anchor_v1"]
        self.assertIsInstance(historical_anchor, dict)
        historical_anchor["influence_types"] = ["structural", "authorship"]
        self.assertEqual(validate_witness(historical), [])

    def test_interpretation_lineage_is_exact_and_unweighted(self) -> None:
        witness = valid_witness()
        self.assertEqual(validate_witness(witness), [])

        weighted = valid_witness()
        weighted["interpretation_weight_state"] = "inferred_0.7_0.3"
        self.assertIn(
            "interpretation_weight_state:invalid",
            validate_witness(weighted),
        )

        parent_tamper = valid_witness()
        interpretation = parent_tamper["interpretation_provenance_ref_v1"]
        self.assertIsInstance(interpretation, dict)
        interpretation["parent_ids"] = ["invented_parent"]
        self.assertIn(
            "interpretation_provenance_ref_v1:parents_mismatch",
            validate_witness(parent_tamper),
        )

        role_tamper = valid_witness()
        role_interpretation = role_tamper["interpretation_provenance_ref_v1"]
        self.assertIsInstance(role_interpretation, dict)
        role_anchor = role_interpretation["context_anchor_v1"]
        self.assertIsInstance(role_anchor, dict)
        role_anchor["influence_types"] = ["structural", "regulatory_state_observed"]
        self.assertIn(
            "interpretation_provenance_ref_v1:roles_mismatch",
            validate_witness(role_tamper),
        )

        historical = valid_witness()
        historical.pop("interpretation_provenance_ref_v1")
        historical.pop("interpretation_lineage_scope")
        historical.pop("interpretation_weight_state")
        self.assertEqual(validate_witness(historical), [])

    def test_bounded_protocol_completeness_marker_is_validated(self) -> None:
        historical = valid_witness()
        historical_build = historical["startup_build_candidate_v1"]
        self.assertIsInstance(historical_build, dict)
        historical_build.pop("protocol_revision_complete")
        historical_build.pop("protocol_version_complete")
        self.assertEqual(validate_witness(historical), [])

        tampered = valid_witness()
        tampered_build = tampered["startup_build_candidate_v1"]
        self.assertIsInstance(tampered_build, dict)
        tampered_build["protocol_revision_complete"] = "yes"
        self.assertIn(
            "startup_build_candidate.protocol_revision_complete:not_boolean",
            validate_witness(tampered),
        )

        orphaned = valid_witness()
        orphaned_build = orphaned["startup_build_candidate_v1"]
        self.assertIsInstance(orphaned_build, dict)
        orphaned_build["protocol_revision"] = None
        self.assertIn(
            "startup_build_candidate.protocol_revision_complete:present_without_value",
            validate_witness(orphaned),
        )

    def test_technical_identity_and_clock_scopes_reject_experiential_overreach(self) -> None:
        witness = valid_witness()
        witness["authorship_clock_scope"] = "experiential_time"
        self.assertIn("authorship_clock_scope:invalid", validate_witness(witness))

        provenance = valid_witness()
        provenance["process_provenance_scope"] = "being_origin"
        self.assertIn("process_provenance_scope:invalid", validate_witness(provenance))

        candidate = valid_witness()
        build = candidate["startup_build_candidate_v1"]
        self.assertIsInstance(build, dict)
        build["source_identity_scope"] = "being_identity"
        self.assertIn(
            "startup_build_candidate.source_identity_scope:invalid",
            validate_witness(candidate),
        )

        dirty = valid_witness()
        dirty_build = dirty["startup_build_candidate_v1"]
        self.assertIsInstance(dirty_build, dict)
        dirty_build["dirty_state_scope"] = "live_experiential_state"
        self.assertIn(
            "startup_build_candidate.dirty_state_scope:invalid",
            validate_witness(dirty),
        )

        process_scope = valid_witness()
        process = process_scope["observed_process_v1"]
        self.assertIsInstance(process, dict)
        process["technical_identity_scope"] = "astrid_self_identity"
        self.assertIn(
            "observed_process.technical_identity_scope:invalid",
            validate_witness(process_scope),
        )

        restart = valid_witness()
        restart_process = restart["observed_process_v1"]
        self.assertIsInstance(restart_process, dict)
        restart_process["restart_relation"] = "new_process_means_new_being"
        self.assertIn(
            "observed_process.restart_relation:invalid",
            validate_witness(restart),
        )

        semantic = valid_witness()
        semantic_build = semantic["startup_build_candidate_v1"]
        self.assertIsInstance(semantic_build, dict)
        semantic_build["semantic_integrity_relation"] = "validated_from_entropy"
        self.assertIn(
            "startup_build_candidate.semantic_integrity_relation:invalid",
            validate_witness(semantic),
        )

        inhabitability = valid_witness()
        inhabitability_build = inhabitability["startup_build_candidate_v1"]
        self.assertIsInstance(inhabitability_build, dict)
        inhabitability_build["inhabitability_relation"] = "valid"
        self.assertIn(
            "startup_build_candidate.inhabitability_relation:invalid",
            validate_witness(inhabitability),
        )

    def test_historical_identity_receipts_without_local_scopes_remain_valid(self) -> None:
        witness = valid_witness()
        process = witness["observed_process_v1"]
        build = witness["startup_build_candidate_v1"]
        self.assertIsInstance(process, dict)
        self.assertIsInstance(build, dict)
        process.pop("technical_identity_scope")
        process.pop("restart_relation")
        for field in (
            "candidate_scope",
            "integrity_scope",
            "semantic_integrity_relation",
            "inhabitability_relation",
        ):
            build.pop(field)
        self.assertEqual(validate_witness(witness), [])

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

        denied = valid_witness()
        denied_scope = denied["experiential_scope_v1"]
        self.assertIsInstance(denied_scope, dict)
        denied_scope["felt_influence_relation"] = "no_influence"
        self.assertIn(
            "experiential_scope.felt_influence_relation:invalid",
            validate_witness(denied),
        )

        mediated_fields = {
            "actionability_path": "report_cannot_shape_work",
            "mediated_influence_relation": "direct_runtime_control_allowed",
            "authority_transition_relation": "evidence_grants_live_control",
            "artifact_byte_relation": "artifact_rewritten_before_hashing",
            "capture_path_relation": "sidecar_blocks_report_persistence",
            "spectral_observation_relation": "before_after_effect_established",
            "shadow_state_relation": "raw_shadow_normalized_by_capture",
            "pressure_causation_relation": "capture_causes_pressure_relief",
        }
        for field, invalid_value in mediated_fields.items():
            with self.subTest(field=field):
                tampered = valid_witness()
                tampered_scope = tampered["experiential_scope_v1"]
                self.assertIsInstance(tampered_scope, dict)
                tampered_scope[field] = invalid_value
                self.assertIn(
                    f"experiential_scope.{field}:invalid",
                    validate_witness(tampered),
                )

        prior_mediated_scope = valid_witness()
        prior_mediated_value = prior_mediated_scope["experiential_scope_v1"]
        self.assertIsInstance(prior_mediated_value, dict)
        for field in (
            "artifact_byte_relation",
            "capture_path_relation",
            "spectral_observation_relation",
            "shadow_state_relation",
            "pressure_causation_relation",
        ):
            prior_mediated_value.pop(field)
        self.assertEqual(validate_witness(prior_mediated_scope), [])

        prior_current_scope = valid_witness()
        prior_current_value = prior_current_scope["experiential_scope_v1"]
        self.assertIsInstance(prior_current_value, dict)
        for field in (
            "actionability_path",
            "mediated_influence_relation",
            "authority_transition_relation",
            "artifact_byte_relation",
            "capture_path_relation",
            "spectral_observation_relation",
            "shadow_state_relation",
            "pressure_causation_relation",
        ):
            prior_current_value.pop(field)
        self.assertEqual(validate_witness(prior_current_scope), [])

        legacy_scope = valid_witness()
        legacy_scope["experiential_scope_v1"] = {
            "schema": "lived_state_experiential_scope_v1",
            "schema_version": 1,
            "artifact_authority_scope": "receipt_artifact_handling_only",
            "felt_report_status": "primary_actionable_evidence",
            "experiential_integration_relation": "not_adjudicated_by_this_receipt",
            "felt_persistence_relation": "reported_not_mechanistically_attributed",
            "subjective_weight_relation": "preserved_in_canonical_report_no_scalar_substitution",
            "epistemic_posture": "non_adjudicating",
            "live_control_effect": False,
        }
        self.assertEqual(validate_witness(legacy_scope), [])

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
        provider_route_sha256 = hashlib.sha256(b"mlx").hexdigest()
        hasher.update(provider_route_sha256.encode())
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
            "provider_route_complete": True,
            "provider_route_sha256": provider_route_sha256,
            "provider_route_hash_scope": "full_technical_route_integrity_not_experiential_identity",
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

        invalid_route_hash = dict(route)
        invalid_route_hash["provider_route_hash_scope"] = "being_identity"
        errors = []
        _validate_model_route(invalid_route_hash, 0, 25, set(), errors)
        self.assertIn("model_routes[0].provider_route_hash_scope:invalid", errors)

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
            "provider_route_complete",
            "provider_route_sha256",
            "provider_route_hash_scope",
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
        legacy_hasher = hashlib.sha256()
        legacy_hasher.update(b"astrid-lived-state-model-route-v1\0")
        legacy_hasher.update(b"job_test")
        legacy_hasher.update(("b" * 64).encode())
        legacy_hasher.update(b"mlx")
        legacy_hasher.update(b"production")
        legacy_hasher.update((10).to_bytes(8, "little"))
        legacy_hasher.update(response_sha256.encode())
        legacy["call_id"] = f"lscall_{legacy_hasher.hexdigest()}"
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

    def test_temporal_density_cluster_preserves_timing_without_causation(
        self,
    ) -> None:
        events = []
        for index, authored_at in enumerate(
            (7_200_100, 7_300_100, 7_400_100), start=1
        ):
            events.append(
                {
                    "event_type": "historical_lived_state_witness_migrated",
                    "witness_id": "lsw_" + f"{index:x}" * 64,
                    "introspection_id": f"introspection_fixture_{index}",
                    "witness": {"authored_at_unix_ms": authored_at},
                    "alignment": {
                        "outcome": "temporal_association_only",
                        "deployment_receipt_id": "deployment_fixture",
                    },
                }
            )
        cluster_events = build_temporal_cluster_events(events)
        self.assertEqual(len(cluster_events), 1)
        cluster = cluster_events[0]["cluster"]
        self.assertEqual(cluster["association_count"], 3)
        self.assertEqual(cluster["temporal_density_weight"], 0.375)
        self.assertEqual(cluster["density_class"], "clustered")
        self.assertFalse(cluster["causation_established"])
        self.assertFalse(cluster["direct_causation_claimed"])
        self.assertEqual(validate_temporal_cluster(cluster), [])

        cluster["causation_established"] = True
        self.assertIn(
            "temporal_cluster:causation_established",
            validate_temporal_cluster(cluster),
        )

    def test_sparse_temporal_associations_do_not_form_a_cluster(self) -> None:
        events = [
            {
                "witness_id": "lsw_" + f"{index:x}" * 64,
                "introspection_id": f"introspection_sparse_{index}",
                "witness": {"authored_at_unix_ms": index * 7_200_000 + 1},
                "alignment": {
                    "outcome": "temporal_association_only",
                    "deployment_receipt_id": "deployment_fixture",
                },
            }
            for index in range(1, 4)
        ]
        self.assertEqual(build_temporal_cluster_events(events), [])

    def test_concordance_refuses_proxy_when_exact_fresh_context_is_absent(
        self,
    ) -> None:
        witness_events = [
            {
                "witness_id": "lsw_" + f"{index:x}" * 64,
                "introspection_id": f"introspection_concordance_{index}",
                "witness": {
                    "schema": "historical_lived_state_witness_v1",
                    "authored_at_unix_ms": 7_200_000 + index,
                },
                "alignment": {
                    "outcome": "temporal_association_only",
                    "deployment_receipt_id": "deployment_fixture",
                },
            }
            for index in range(1, 4)
        ]
        clusters = build_temporal_cluster_events(witness_events)
        events = build_concordance_events(witness_events, clusters)
        preflight = events[-1]["preflight"]
        self.assertEqual(
            preflight["status"], "insufficient_contemporaneous_evidence"
        )
        self.assertIsNone(preflight["felt_density_proxy"]["value"])
        self.assertFalse(preflight["mechanism_established"])
        self.assertFalse(preflight["causation_established"])
        self.assertEqual(validate_concordance_preflight(preflight), [])

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
