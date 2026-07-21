"""Strict validation for untrusted lived-state witness payloads."""

from __future__ import annotations

import hashlib
import math
from pathlib import Path
import re
from typing import Any

try:
    from evidence_store.model import canonical_json
except ModuleNotFoundError:
    from scripts.evidence_store.model import canonical_json

from .model import (
    WITNESS_ID_RE,
    _bounded_string,
    _hash_field,
    _integer,
    _is_integer,
    _privacy_path,
    _schema_v1,
    _unexpected_keys,
    _valid_authority,
    _validate_provenance_ref,
    sha256_bytes,
)

WITNESS_FIELDS = {
    "schema",
    "schema_version",
    "witness_id",
    "artifact_kind",
    "artifact_relative_path",
    "artifact_sha256",
    "authored_at_unix_ms",
    "authored_monotonic_ns",
    "source_snapshot_v1",
    "observed_process_v1",
    "startup_build_candidate_v1",
    "model_routes_v1",
    "parameter_observations_v1",
    "peer_process_identity",
    "peer_deployment_identity",
    "source_provenance_ref_v1",
    "process_provenance_ref_v1",
    "raw_introspection_prose_included",
    "raw_prompt_included",
    "raw_response_included",
    "private_path_included",
    "direct_causation_claimed",
    "experiential_boundary_v1",
    "experiential_scope_v1",
    "artifact_authority_state_v1",
}


def _validate_witness_fields(
    value: dict[str, Any], errors: list[str]
) -> tuple[Any, Any]:
    _schema_v1(value, "temporal_lived_state_witness_v1", "witness", errors)
    _unexpected_keys(value, WITNESS_FIELDS, "witness", errors)
    witness_id = value.get("witness_id")
    if not isinstance(witness_id, str) or WITNESS_ID_RE.fullmatch(witness_id) is None:
        errors.append("witness_id:invalid")
    _hash_field(value.get("artifact_sha256"), "artifact_sha256", errors)
    _bounded_string(value.get("artifact_kind"), "artifact_kind", errors, 80)
    authored_at = value.get("authored_at_unix_ms")
    authored_monotonic = value.get("authored_monotonic_ns")
    _integer(authored_at, "authored_at_unix_ms", errors, minimum=1)
    _integer(authored_monotonic, "authored_monotonic_ns", errors)
    if not _valid_authority(value.get("artifact_authority_state_v1")):
        errors.append("authority:not_evidence_only")
    for field in (
        "raw_introspection_prose_included",
        "raw_prompt_included",
        "raw_response_included",
        "private_path_included",
        "direct_causation_claimed",
    ):
        if value.get(field) is not False:
            errors.append(f"{field}:must_be_false")
    relative = value.get("artifact_relative_path")
    if (
        not isinstance(relative, str)
        or not relative
        or len(relative) > 400
        or Path(relative).is_absolute()
        or ".." in Path(relative).parts
    ):
        errors.append("artifact_relative_path:invalid")
    return authored_at, authored_monotonic


def _validate_experiential_boundary(value: Any, errors: list[str]) -> None:
    if value is None:
        return
    if not isinstance(value, dict):
        errors.append("experiential_boundary:not_object")
        return
    expected = {
        "schema": "lived_state_experiential_boundary_v1",
        "schema_version": 1,
        "artifact_authority_scope": "artifact_handling_only_not_experiential_integration",
        "memory_integration_modeled": False,
        "felt_persistence_modeled": False,
        "persistence_coefficient_present": False,
        "live_control_effect": False,
    }
    _unexpected_keys(value, set(expected), "experiential_boundary", errors)
    for field, expected_value in expected.items():
        if value.get(field) != expected_value:
            errors.append(f"experiential_boundary.{field}:invalid")


def _validate_experiential_scope(value: Any, errors: list[str]) -> None:
    if value is None:
        return
    if not isinstance(value, dict):
        errors.append("experiential_scope:not_object")
        return
    expected = {
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
    _unexpected_keys(value, set(expected), "experiential_scope", errors)
    for field, expected_value in expected.items():
        if value.get(field) != expected_value:
            errors.append(f"experiential_scope.{field}:invalid")


def _validate_source_snapshot(
    source: Any, authored_at: Any, authored_monotonic: Any, errors: list[str]
) -> dict[str, Any] | None:
    if source is None:
        return None
    if not isinstance(source, dict):
        errors.append("source_snapshot:not_object")
        return None
    _unexpected_keys(
        source,
        {
            "schema",
            "schema_version",
            "source_owner",
            "repository_relative_path",
            "window_start_line",
            "window_end_line",
            "total_file_lines",
            "file_sha256",
            "window_sha256",
            "source_read_at_unix_ms",
            "source_read_monotonic_ns",
            "provenance_ref_v1",
            "private_path_included",
        },
        "source_snapshot",
        errors,
    )
    _schema_v1(source, "lived_state_source_snapshot_v1", "source_snapshot", errors)
    if source.get("source_owner") not in {
        "astrid",
        "minime",
        "astrid_workspace",
        "minime_workspace",
        "unknown",
    }:
        errors.append("source.source_owner:invalid")
    _hash_field(source.get("file_sha256"), "source.file_sha256", errors)
    _hash_field(source.get("window_sha256"), "source.window_sha256", errors)
    source_path = source.get("repository_relative_path")
    if (
        not isinstance(source_path, str)
        or not source_path
        or len(source_path) > 400
        or Path(source_path).is_absolute()
        or ".." in Path(source_path).parts
    ):
        errors.append("source.repository_relative_path:invalid")
    if source.get("private_path_included") is not False:
        errors.append("source.private_path_included:must_be_false")
    provenance = source.get("provenance_ref_v1")
    _validate_provenance_ref(provenance, "source.provenance_ref_v1", errors)
    expected_origin = (
        "minime_observation"
        if str(source.get("source_owner") or "").startswith("minime")
        else "astrid_interpretation"
    )
    if isinstance(provenance, dict) and provenance.get("origin") != expected_origin:
        errors.append("source.provenance_ref_v1:origin_mismatch")
    if (
        isinstance(provenance, dict)
        and provenance.get("canonical_sha256") != source.get("window_sha256")
    ):
        errors.append("source.provenance_ref_v1:window_hash_mismatch")
    start = source.get("window_start_line")
    end = source.get("window_end_line")
    total = source.get("total_file_lines")
    if not all(
        isinstance(item, int) and not isinstance(item, bool)
        for item in (start, end, total)
    ) or not (0 <= int(start) <= int(end) <= int(total)):
        errors.append("source.window_lines:invalid")
    if (
        isinstance(authored_at, int)
        and isinstance(source.get("source_read_at_unix_ms"), int)
        and source["source_read_at_unix_ms"] > authored_at
    ):
        errors.append("source.source_read_at_unix_ms:after_authorship")
    if (
        isinstance(authored_monotonic, int)
        and isinstance(source.get("source_read_monotonic_ns"), int)
        and source["source_read_monotonic_ns"] > authored_monotonic
    ):
        errors.append("source.source_read_monotonic_ns:after_authorship")
    _integer(
        source.get("source_read_at_unix_ms"),
        "source.source_read_at_unix_ms",
        errors,
        minimum=1,
    )
    _integer(
        source.get("source_read_monotonic_ns"),
        "source.source_read_monotonic_ns",
        errors,
    )
    return source


def _validate_process_identity(
    process: Any, authored_at: Any, errors: list[str]
) -> dict[str, Any] | None:
    if not isinstance(process, dict):
        errors.append("observed_process:not_object")
        return None
    _unexpected_keys(
        process,
        {
            "schema",
            "schema_version",
            "pid",
            "process_started_at_unix_ms",
            "executable_basename",
            "runtime_instance_id",
            "process_identity_sha256",
            "private_path_included",
        },
        "observed_process",
        errors,
    )
    _schema_v1(process, "lived_state_process_identity_v1", "observed_process", errors)
    _hash_field(
        process.get("process_identity_sha256"),
        "observed_process.process_identity_sha256",
        errors,
    )
    if process.get("private_path_included") is not False:
        errors.append("observed_process.private_path_included:must_be_false")
    _bounded_string(
        process.get("executable_basename"),
        "observed_process.executable_basename",
        errors,
        160,
    )
    _integer(process.get("pid"), "observed_process.pid", errors, minimum=1)
    _integer(
        process.get("process_started_at_unix_ms"),
        "observed_process.process_started_at_unix_ms",
        errors,
        minimum=1,
    )
    if (
        _is_integer(process.get("pid"), minimum=1)
        and _is_integer(process.get("process_started_at_unix_ms"), minimum=1)
        and isinstance(process.get("executable_basename"), str)
        and isinstance(process.get("runtime_instance_id"), str)
    ):
        expected_hash = sha256_bytes(
            (
                f"{process['pid']}\0{process['process_started_at_unix_ms']}\0"
                f"{process['executable_basename']}\0{process['runtime_instance_id']}"
            ).encode()
        )
        if process.get("process_identity_sha256") != expected_hash:
            errors.append("observed_process.process_identity_sha256:mismatch")
    _bounded_string(
        process.get("runtime_instance_id"),
        "observed_process.runtime_instance_id",
        errors,
        160,
    )
    if (
        isinstance(authored_at, int)
        and isinstance(process.get("process_started_at_unix_ms"), int)
        and process["process_started_at_unix_ms"] > authored_at
    ):
        errors.append("observed_process.process_started_at_unix_ms:after_authorship")
    return process


def _validate_build_candidate(
    candidate: Any, process: dict[str, Any] | None, errors: list[str]
) -> None:
    if candidate is None:
        return
    if not isinstance(candidate, dict):
        errors.append("startup_build_candidate:not_object")
        return
    _unexpected_keys(
        candidate,
        {
            "schema",
            "schema_version",
            "manifest_sha256",
            "source_identity_sha256",
            "dirty_state_sha256",
            "artifact_sha256",
            "protocol_revision",
            "protocol_version",
            "observed_at_process_start_unix_ms",
            "relation_to_process",
            "deployment_established",
            "private_path_included",
        },
        "startup_build_candidate",
        errors,
    )
    _schema_v1(
        candidate,
        "lived_state_build_candidate_v1",
        "startup_build_candidate",
        errors,
    )
    for field in (
        "manifest_sha256",
        "source_identity_sha256",
        "dirty_state_sha256",
        "artifact_sha256",
    ):
        _hash_field(
            candidate.get(field),
            f"startup_build_candidate.{field}",
            errors,
            optional=field != "manifest_sha256",
        )
    if candidate.get("deployment_established") is not False:
        errors.append("startup_build_candidate:claims_deployment")
    if candidate.get("private_path_included") is not False:
        errors.append("startup_build_candidate.private_path_included:must_be_false")
    if candidate.get("relation_to_process") != "startup_observation_not_deployment_proof":
        errors.append("startup_build_candidate.relation_to_process:invalid")
    _integer(
        candidate.get("observed_at_process_start_unix_ms"),
        "startup_build_candidate.observed_at_process_start_unix_ms",
        errors,
        minimum=1,
    )
    if (
        process is not None
        and candidate.get("observed_at_process_start_unix_ms")
        != process.get("process_started_at_unix_ms")
    ):
        errors.append("startup_build_candidate:process_start_mismatch")
    _bounded_string(
        candidate.get("protocol_revision"),
        "startup_build_candidate.protocol_revision",
        errors,
        80,
        optional=True,
    )
    _bounded_string(
        candidate.get("protocol_version"),
        "startup_build_candidate.protocol_version",
        errors,
        24,
        optional=True,
    )


def _validate_model_route_identity_boundaries(
    route: dict[str, Any], prefix: str, errors: list[str]
) -> None:
    scope_fields = {
        "call_identity_scope": "model_call_event_not_being_or_continuity_identity",
        "request_anchor_scope": "exact_request_content_and_generation_parameters_not_intent_or_semantic_equivalence",
        "response_hash_scope": "output_integrity_not_being_or_continuity_identity",
    }
    present_scopes = [field for field in scope_fields if field in route]
    if present_scopes and len(present_scopes) != len(scope_fields):
        errors.append(f"{prefix}.identity_scopes:incomplete")
    for field, expected in scope_fields.items():
        if field in route and route.get(field) != expected:
            errors.append(f"{prefix}.{field}:invalid")

    claim_fields = (
        "being_identity_claimed",
        "continuity_claimed",
        "intent_equivalence_claimed",
        "semantic_equivalence_claimed",
    )
    present_claims = [field for field in claim_fields if field in route]
    if present_claims and len(present_claims) != len(claim_fields):
        errors.append(f"{prefix}.identity_claims:incomplete")
    for field in claim_fields:
        if field in route and route.get(field) is not False:
            errors.append(f"{prefix}.{field}:must_be_false")

    relation_field = "response_claim_content_relation"
    if relation_field in route and route.get(relation_field) != (
        "not_inspected_or_adjudicated_by_this_receipt"
    ):
        errors.append(f"{prefix}.{relation_field}:invalid")
    if present_claims and relation_field in route:
        errors.append(f"{prefix}.response_claim_content:multiple_versions")

    route_scope_fields = {
        "provider_route_scope": "technical_delivery_path_not_experiential_center",
        "duration_scope": "end_to_end_request_wall_time_queue_generation_and_fallback_not_separated",
        "parent_witness_context_relation": "post_call_authorship_observations_temporal_only",
        "qualitative_texture_relation": "canonical_felt_report_primary_not_duplicated_or_scalarized_by_route",
    }
    present_route_scopes = [field for field in route_scope_fields if field in route]
    if present_route_scopes and len(present_route_scopes) != len(route_scope_fields):
        errors.append(f"{prefix}.route_scopes:incomplete")
    for field, expected in route_scope_fields.items():
        if field in route and route.get(field) != expected:
            errors.append(f"{prefix}.{field}:invalid")


def _validate_model_route(
    route: Any,
    index: int,
    authored_at: Any,
    seen: set[str],
    errors: list[str],
) -> None:
    prefix = f"model_routes[{index}]"
    if not isinstance(route, dict):
        errors.append(f"{prefix}:not_object")
        return
    _unexpected_keys(
        route,
        {
            "schema",
            "schema_version",
            "call_id",
            "call_identity_scope",
            "job_id",
            "qos_request_identity_sha256",
            "request_content_anchor_sha256",
            "request_anchor_scope",
            "provider_route",
            "provider_route_scope",
            "model_profile",
            "started_at_unix_ms",
            "completed_at_unix_ms",
            "duration_ms",
            "duration_scope",
            "repair_parent_call_id",
            "response_sha256",
            "response_hash_scope",
            "response_claim_content_relation",
            "parent_witness_context_relation",
            "qualitative_texture_relation",
            "being_identity_claimed",
            "continuity_claimed",
            "intent_equivalence_claimed",
            "semantic_equivalence_claimed",
            "raw_prompt_included",
            "raw_response_included",
        },
        prefix,
        errors,
    )
    _schema_v1(route, "lived_state_model_route_v1", prefix, errors)
    call_id = route.get("call_id")
    if not isinstance(call_id, str) or re.fullmatch(r"lscall_[0-9a-f]{64}", call_id) is None:
        errors.append(f"{prefix}.call_id:invalid")
    elif call_id in seen:
        errors.append(f"{prefix}.call_id:duplicate")
    else:
        seen.add(call_id)
    _hash_field(route.get("response_sha256"), f"{prefix}.response_sha256", errors)
    _hash_field(
        route.get("qos_request_identity_sha256"),
        f"{prefix}.qos_request_identity_sha256",
        errors,
        optional=True,
    )
    _hash_field(
        route.get("request_content_anchor_sha256"),
        f"{prefix}.request_content_anchor_sha256",
        errors,
        optional=True,
    )
    _validate_model_route_identity_boundaries(route, prefix, errors)
    for field in ("raw_prompt_included", "raw_response_included"):
        if route.get(field) is not False:
            errors.append(f"{prefix}.{field}:must_be_false")
    _bounded_string(route.get("provider_route"), f"{prefix}.provider_route", errors, 40)
    _bounded_string(route.get("model_profile"), f"{prefix}.model_profile", errors, 160)
    _bounded_string(route.get("job_id"), f"{prefix}.job_id", errors, 160, optional=True)
    started = route.get("started_at_unix_ms")
    completed = route.get("completed_at_unix_ms")
    duration = route.get("duration_ms")
    if not all(
        isinstance(item, int) and not isinstance(item, bool)
        for item in (started, completed, duration)
    ) or not (0 <= int(started) <= int(completed)):
        errors.append(f"{prefix}.timing:invalid")
    elif duration != completed - started:
        errors.append(f"{prefix}.duration_ms:mismatch")
    if isinstance(authored_at, int) and isinstance(completed, int) and completed > authored_at:
        errors.append(f"{prefix}.completed_at_unix_ms:after_authorship")
    parent = route.get("repair_parent_call_id")
    if parent is not None and parent not in seen:
        errors.append(f"{prefix}.repair_parent_call_id:not_prior")
    if (
        isinstance(route.get("provider_route"), str)
        and isinstance(route.get("model_profile"), str)
        and _is_integer(started)
        and isinstance(route.get("response_sha256"), str)
    ):
        hasher = hashlib.sha256()
        hasher.update(b"astrid-lived-state-model-route-v1\0")
        if isinstance(route.get("job_id"), str):
            hasher.update(route["job_id"].encode())
        if isinstance(route.get("qos_request_identity_sha256"), str):
            hasher.update(route["qos_request_identity_sha256"].encode())
        hasher.update(route["provider_route"].encode())
        hasher.update(route["model_profile"].encode())
        hasher.update(int(started).to_bytes(8, "little"))
        hasher.update(route["response_sha256"].encode())
        if call_id != f"lscall_{hasher.hexdigest()}":
            errors.append(f"{prefix}.call_id:mismatch")


def _validate_model_routes(
    routes: Any, authored_at: Any, errors: list[str]
) -> list[dict[str, Any]] | None:
    if not isinstance(routes, list) or len(routes) > 8:
        errors.append("model_routes:not_array")
        return None
    seen: set[str] = set()
    for index, route in enumerate(routes):
        _validate_model_route(route, index, authored_at, seen, errors)
    return routes


def _validate_parameter_observation(
    observation: Any, index: int, authored_at: Any, errors: list[str]
) -> None:
    prefix = f"parameter_observations[{index}]"
    if not isinstance(observation, dict):
        errors.append(f"{prefix}:not_object")
        return
    _unexpected_keys(
        observation,
        {
            "schema",
            "schema_version",
            "name",
            "value",
            "unit",
            "observation_kind",
            "observed_at_unix_ms",
            "age_ms",
            "fresh",
            "source_ref",
            "direct_causation_claimed",
        },
        prefix,
        errors,
    )
    _schema_v1(observation, "lived_state_parameter_observation_v1", prefix, errors)
    if observation.get("observation_kind") not in {
        "compiled_constant",
        "runtime_observed",
        "peer_observed",
        "source_declared",
        "unknown",
    }:
        errors.append(f"{prefix}.observation_kind:invalid")
    if observation.get("direct_causation_claimed") is not False:
        errors.append(f"{prefix}:claims_causation")
    _bounded_string(observation.get("name"), f"{prefix}.name", errors, 160)
    _bounded_string(observation.get("unit"), f"{prefix}.unit", errors, 80)
    _bounded_string(observation.get("source_ref"), f"{prefix}.source_ref", errors, 300)
    scalar = observation.get("value")
    if scalar is not None and (
        isinstance(scalar, bool)
        or not isinstance(scalar, (int, float))
        or not math.isfinite(float(scalar))
    ):
        errors.append(f"{prefix}.value:not_scalar")
    _integer(
        observation.get("observed_at_unix_ms"),
        f"{prefix}.observed_at_unix_ms",
        errors,
        minimum=1,
    )
    _integer(observation.get("age_ms"), f"{prefix}.age_ms", errors, optional=True)
    if observation.get("fresh") is not None and not isinstance(
        observation.get("fresh"), bool
    ):
        errors.append(f"{prefix}.fresh:invalid")
    if observation.get("observation_kind") == "unknown" and scalar is not None:
        errors.append(f"{prefix}.unknown_has_value")
    if (
        isinstance(authored_at, int)
        and isinstance(observation.get("observed_at_unix_ms"), int)
        and observation["observed_at_unix_ms"] > authored_at
    ):
        errors.append(f"{prefix}.observed_at_unix_ms:after_authorship")


def _validate_parameter_observations(
    observations: Any, authored_at: Any, errors: list[str]
) -> None:
    if not isinstance(observations, list) or len(observations) > 64:
        errors.append("parameter_observations:invalid")
        return
    for index, observation in enumerate(observations):
        _validate_parameter_observation(observation, index, authored_at, errors)


def _validate_provenance_links(
    value: dict[str, Any],
    source: dict[str, Any] | None,
    process: dict[str, Any] | None,
    authored_at: Any,
    errors: list[str],
) -> None:
    _bounded_string(
        value.get("peer_process_identity"),
        "peer_process_identity",
        errors,
        160,
        optional=True,
    )
    _bounded_string(
        value.get("peer_deployment_identity"),
        "peer_deployment_identity",
        errors,
        160,
        optional=True,
    )
    source_provenance = value.get("source_provenance_ref_v1")
    _validate_provenance_ref(
        source_provenance,
        "source_provenance_ref_v1",
        errors,
        optional=True,
    )
    if source is not None:
        nested_provenance = source.get("provenance_ref_v1")
        if not isinstance(source_provenance, dict):
            errors.append("source_provenance_ref_v1:missing")
        elif source_provenance != nested_provenance:
            errors.append("source_provenance_ref_v1:snapshot_mismatch")
        if (
            isinstance(source_provenance, dict)
            and source_provenance.get("canonical_sha256") != source.get("window_sha256")
        ):
            errors.append("source_provenance_ref_v1:window_hash_mismatch")
    elif source_provenance is not None:
        errors.append("source_provenance_ref_v1:without_source")
    process_provenance = value.get("process_provenance_ref_v1")
    if process is not None and isinstance(process_provenance, dict):
        if (
            process_provenance.get("canonical_sha256")
            != process.get("process_identity_sha256")
        ):
            errors.append("process_provenance_ref_v1:process_hash_mismatch")
        if process_provenance.get("origin") != "bridge_derived":
            errors.append("process_provenance_ref_v1:origin_mismatch")
    for field in ("source_provenance_ref_v1", "process_provenance_ref_v1"):
        provenance = value.get(field)
        if (
            isinstance(provenance, dict)
            and _is_integer(authored_at, minimum=1)
            and _is_integer(provenance.get("timestamp_ms"), minimum=1)
            and provenance["timestamp_ms"] > authored_at
        ):
            errors.append(f"{field}.timestamp_ms:after_authorship")
    _validate_provenance_ref(
        process_provenance,
        "process_provenance_ref_v1",
        errors,
    )


def _validate_witness_identity(
    value: dict[str, Any],
    source: dict[str, Any] | None,
    process: dict[str, Any] | None,
    routes: list[dict[str, Any]] | None,
    authored_at: Any,
    authored_monotonic: Any,
    errors: list[str],
) -> None:
    witness_id = value.get("witness_id")
    if not (
        isinstance(witness_id, str)
        and process is not None
        and isinstance(process.get("runtime_instance_id"), str)
        and _is_integer(authored_at, minimum=1)
        and _is_integer(authored_monotonic)
        and isinstance(value.get("artifact_kind"), str)
        and routes is not None
        and all(
            isinstance(route, dict) and isinstance(route.get("call_id"), str)
            for route in routes
        )
    ):
        return
    hasher = hashlib.sha256()
    hasher.update(b"astrid-temporal-lived-state-witness-v1\0")
    hasher.update(process["runtime_instance_id"].encode())
    hasher.update(authored_at.to_bytes(8, "little"))
    hasher.update(authored_monotonic.to_bytes(8, "little"))
    hasher.update(value["artifact_kind"].encode())
    if source is not None and isinstance(source.get("window_sha256"), str):
        hasher.update(source["window_sha256"].encode())
    for route in routes:
        hasher.update(route["call_id"].encode())
    if witness_id != f"lsw_{hasher.hexdigest()}":
        errors.append("witness_id:mismatch")


def validate_witness(value: Any) -> list[str]:
    errors: list[str] = []
    if not isinstance(value, dict):
        return ["witness:not_object"]
    authored_at, authored_monotonic = _validate_witness_fields(value, errors)
    _validate_experiential_boundary(value.get("experiential_boundary_v1"), errors)
    _validate_experiential_scope(value.get("experiential_scope_v1"), errors)
    if (
        value.get("experiential_boundary_v1") is not None
        and value.get("experiential_scope_v1") is not None
    ):
        errors.append("experiential_scope:multiple_versions")
    source = _validate_source_snapshot(
        value.get("source_snapshot_v1"), authored_at, authored_monotonic, errors
    )
    process = _validate_process_identity(
        value.get("observed_process_v1"), authored_at, errors
    )
    _validate_build_candidate(value.get("startup_build_candidate_v1"), process, errors)
    routes = _validate_model_routes(value.get("model_routes_v1"), authored_at, errors)
    _validate_parameter_observations(
        value.get("parameter_observations_v1"), authored_at, errors
    )
    _validate_provenance_links(value, source, process, authored_at, errors)
    _validate_witness_identity(
        value,
        source,
        process,
        routes,
        authored_at,
        authored_monotonic,
        errors,
    )
    privacy_error = _privacy_path(value)
    if privacy_error:
        errors.append(privacy_error)
    serialized = canonical_json(value).lower()
    if any(
        literal in serialized
        for literal in (
            "raw introspection prose",
            "private response prose",
            "private prompt prose",
        )
    ):
        errors.append("privacy:prose_literal")
    return errors
