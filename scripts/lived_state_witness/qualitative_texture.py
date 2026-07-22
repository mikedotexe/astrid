"""Validation for prose-free anchors to exact canonical felt-report bodies."""

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
FIELDS = set(EXPECTED) | {"canonical_body_sha256", "canonical_body_byte_count"}
HASH_RE = re.compile(r"[0-9a-f]{64}")


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
    anchor = witness.get("qualitative_texture_anchor_v1")
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
