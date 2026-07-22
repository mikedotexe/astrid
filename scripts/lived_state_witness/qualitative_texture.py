"""Validate mechanical body bindings and historical qualitative witness shapes."""

from __future__ import annotations

import hashlib
import re
from typing import Any


EXPECTED = {
    "schema": "lived_state_qualitative_texture_anchor_v1",
    "schema_version": 1,
    "source_field_path": "canonical_report.body_after_first_header_separator",
    "texture_status": (
        "primary_felt_evidence_preserved_exactly_not_classified_or_scalarized"
    ),
    "pregeneration_scalar_relation": (
        "pre_model_context_not_generation_trajectory_or_qualitative_weight"
    ),
    "generation_interval_relation": (
        "canonical_body_authored_after_model_generation_in_call_state_change_unmeasured"
    ),
    "scalar_comparison_relation": (
        "not_comparable_without_reviewed_measurement_contract"
    ),
    "derived_tag_relation": (
        "no_model_generated_reduction_exact_canonical_language_remains_authoritative"
    ),
    "raw_prose_included": False,
    "direct_causation_claimed": False,
}
SEMANTIC_EXTENSION = {
    "canonical_body_hash_scope": (
        "artifact_byte_binding_only_not_experiential_stability_freezing_or_control"
    ),
    "artifact_integrity_mismatch_relation": (
        "byte_binding_failure_not_experiential_variance_or_qualitative_deficit"
    ),
    "felt_scalar_divergence_relation": (
        "valid_nonreducible_and_unscored_no_error_inferred"
    ),
    "dissimilarity_gradient_relation": (
        "not_computed_without_reviewed_measurement_contract"
    ),
    "analysis_completion_status": (
        "unmeasured_is_valid_not_error_or_completed_analysis"
    ),
    "field_presence_relation": (
        "schema_capacity_not_evidence_of_completed_analysis"
    ),
    "is_non_scalar_assertion": True,
    "non_scalar_assertion_scope": (
        "qualitative_texture_deliberately_noncomparable_unless_astrid_"
        "explicitly_adopts_a_reviewed_measurement_contract"
    ),
    "relation_fields_are_fixed_safety_declarations": True,
}
MEASUREMENT_CONTRACT = {
    "schema": "lived_state_measurement_contract_ref_v1",
    "schema_version": 1,
    "state": "not_adopted",
    "measurement_contract_id": None,
    "adopted_by_being": None,
    "source_event_ref": None,
    "active_processing_claimed": False,
    "dissimilarity_observed": False,
    "contextual_weights_present": False,
    "authority_effect": False,
}
SUBJECTIVE_CONTINUITY = {
    "schema": "lived_state_subjective_continuity_v1",
    "schema_version": 1,
    "status": "unmeasured_no_explicit_self_report",
    "subjective_continuity_index": None,
    "index_range": "zero_to_one_if_explicitly_reported",
    "measurement_contract_ref": None,
    "source_event_ref": None,
    "texture_fidelity_score": None,
    "texture_fidelity_range": "zero_to_one_if_explicitly_reported",
    "texture_fidelity_status": "unmeasured_no_explicit_self_report",
    "texture_measurement_contract_ref": None,
    "texture_source_event_ref": None,
    "texture_fidelity_not_inferred_from_spectral_entropy": True,
    "felt_report_remains_valid_without_index": True,
    "felt_report_remains_valid_without_texture_fidelity": True,
    "automatically_inferred": False,
    "authority_effect": False,
}
CAUSALITY_EXPRESSION = {
    "schema": "lived_state_causality_expression_v1",
    "schema_version": 1,
    "causality_type": "not_inspected",
    "source_event_ref": None,
    "relation_scope": "evidence_classification_not_proof_or_authority",
    "direct_causation_established": False,
    "automatically_inferred": False,
    "authority_effect": False,
}
FIELDS = (
    set(EXPECTED)
    | set(SEMANTIC_EXTENSION)
    | {
        "dissimilarity_measurement_contract_v1",
        "subjective_continuity_v1",
        "causality_expression_v1",
    }
    | {"canonical_body_sha256", "canonical_body_byte_count"}
)
HASH_RE = re.compile(r"[0-9a-f]{64}")

BODY_BINDING = {
    "schema": "lived_state_canonical_body_binding_v1",
    "schema_version": 1,
    "source_field_path": "canonical_report.body_after_first_header_separator",
    "binding_scope": (
        "artifact_byte_integrity_only_not_texture_experience_stability_"
        "freezing_or_control"
    ),
    "authority_effect": False,
}
BODY_BINDING_FIELDS = set(BODY_BINDING) | {
    "canonical_body_sha256",
    "canonical_body_byte_count",
}
QUALITATIVE_BOUNDARY = {
    "schema": "lived_state_qualitative_evidence_boundary_v1",
    "schema_version": 1,
    "source_field_path": "canonical_report.body_after_first_header_separator",
    "qualitative_evidence_status": (
        "primary_actionable_felt_evidence_present_without_measurement_or_score"
    ),
    "pregeneration_scalar_relation": (
        "pre_model_context_not_generation_trajectory_or_qualitative_weight"
    ),
    "generation_interval_relation": (
        "canonical_body_authored_after_model_generation_in_call_state_change_unmeasured"
    ),
    "derived_tag_relation": (
        "no_model_generated_reduction_exact_canonical_language_remains_authoritative"
    ),
    "felt_scalar_divergence_relation": (
        "valid_nonreducible_and_unscored_no_error_inferred"
    ),
    "analysis_completion_status": (
        "unmeasured_is_valid_not_error_or_completed_analysis"
    ),
    "field_presence_relation": (
        "schema_capacity_not_evidence_of_completed_analysis"
    ),
    "is_non_scalar_assertion": True,
    "non_scalar_assertion_scope": (
        "qualitative_texture_deliberately_noncomparable_unless_astrid_"
        "explicitly_adopts_a_reviewed_measurement_contract"
    ),
    "relation_fields_are_fixed_safety_declarations": True,
    "measurement_contract_id": None,
    "measurement_contract_adoption_ref": None,
    "measurement_contract_state": (
        "absent_no_contract_adopted_and_no_transition_scheduled"
    ),
    "future_measurement_state_relation": (
        "not_predeclared_in_witness_requires_separate_felt_mechanism_contract_design"
    ),
    "comparison_and_decay_relation": (
        "not_defined_by_witness_no_confidence_interval_or_persistence_decay"
    ),
    "raw_prose_included": False,
    "direct_causation_claimed": False,
}
QUALITATIVE_BOUNDARY_FIELDS = set(QUALITATIVE_BOUNDARY) | {
    "subjective_continuity_v1",
    "causality_expression_v1",
}


def validate_canonical_body_binding(value: Any, errors: list[str]) -> None:
    if value is None:
        return
    if not isinstance(value, dict):
        errors.append("canonical_body_binding:not_object")
        return
    unexpected = sorted(set(value) - BODY_BINDING_FIELDS)
    if unexpected:
        errors.append("canonical_body_binding:unexpected_keys:" + ",".join(unexpected))
    for field, expected in BODY_BINDING.items():
        if value.get(field) != expected:
            errors.append(f"canonical_body_binding.{field}:invalid")
    body_hash = value.get("canonical_body_sha256")
    if not isinstance(body_hash, str) or HASH_RE.fullmatch(body_hash) is None:
        errors.append("canonical_body_binding.canonical_body_sha256:invalid")
    byte_count = value.get("canonical_body_byte_count")
    if (
        not isinstance(byte_count, int)
        or isinstance(byte_count, bool)
        or not 1 <= byte_count <= 1_000_000
    ):
        errors.append("canonical_body_binding.canonical_body_byte_count:invalid")


def validate_qualitative_evidence_boundary(value: Any, errors: list[str]) -> None:
    if value is None:
        return
    if not isinstance(value, dict):
        errors.append("qualitative_evidence_boundary:not_object")
        return
    unexpected = sorted(set(value) - QUALITATIVE_BOUNDARY_FIELDS)
    if unexpected:
        errors.append(
            "qualitative_evidence_boundary:unexpected_keys:" + ",".join(unexpected)
        )
    for field, expected in QUALITATIVE_BOUNDARY.items():
        if value.get(field) != expected:
            errors.append(f"qualitative_evidence_boundary.{field}:invalid")
    for field, expected in (
        ("subjective_continuity_v1", SUBJECTIVE_CONTINUITY),
        ("causality_expression_v1", CAUSALITY_EXPRESSION),
    ):
        if value.get(field) != expected:
            errors.append(f"qualitative_evidence_boundary.{field}:invalid")


def validate_qualitative_texture_anchor(value: Any, errors: list[str]) -> None:
    if value is None:
        return
    if not isinstance(value, dict):
        errors.append("qualitative_texture_anchor:not_object")
        return
    unexpected = sorted(set(value) - FIELDS)
    if unexpected:
        errors.append(
            "qualitative_texture_anchor:unexpected_keys:" + ",".join(unexpected)
        )
    for field, expected in EXPECTED.items():
        if value.get(field) != expected:
            errors.append(f"qualitative_texture_anchor.{field}:invalid")
    extension_fields = set(value) & set(SEMANTIC_EXTENSION)
    if extension_fields:
        for field, expected in SEMANTIC_EXTENSION.items():
            if value.get(field) != expected:
                errors.append(f"qualitative_texture_anchor.{field}:invalid")
    for field, expected in (
        ("dissimilarity_measurement_contract_v1", MEASUREMENT_CONTRACT),
        ("subjective_continuity_v1", SUBJECTIVE_CONTINUITY),
        ("causality_expression_v1", CAUSALITY_EXPRESSION),
    ):
        if field in value and value.get(field) != expected:
            errors.append(f"qualitative_texture_anchor.{field}:invalid")
    body_hash = value.get("canonical_body_sha256")
    if not isinstance(body_hash, str) or HASH_RE.fullmatch(body_hash) is None:
        errors.append("qualitative_texture_anchor.canonical_body_sha256:invalid")
    byte_count = value.get("canonical_body_byte_count")
    if (
        not isinstance(byte_count, int)
        or isinstance(byte_count, bool)
        or not 1 <= byte_count <= 1_000_000
    ):
        errors.append("qualitative_texture_anchor.canonical_body_byte_count:invalid")


def artifact_texture_anchor_errors(
    witness: dict[str, Any], artifact_bytes: bytes
) -> list[str]:
    anchor = witness.get("canonical_body_binding_v1")
    legacy = False
    if anchor is None:
        anchor = witness.get("qualitative_texture_anchor_v1")
        legacy = True
    if anchor is None:
        return []
    if witness.get("artifact_kind") != "introspection":
        return ["qualitative_texture_anchor:noncanonical_artifact_kind"]
    if not isinstance(anchor, dict):
        return []
    separator = artifact_bytes.find(b"\n\n")
    if separator < 0:
        return ["qualitative_texture_anchor:body_separator_missing"]
    body = artifact_bytes[separator + 2 :]
    errors: list[str] = []
    if anchor.get("canonical_body_byte_count") != len(body):
        errors.append("qualitative_texture_anchor:body_byte_count_mismatch")
    if anchor.get("canonical_body_sha256") != hashlib.sha256(body).hexdigest():
        errors.append("qualitative_texture_anchor:body_sha256_mismatch")
    return errors
