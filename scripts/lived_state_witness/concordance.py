"""Evidence-only temporal-cluster and mechanism concordance preflight."""

from __future__ import annotations

import math
import re
from typing import Any, Iterable

from .model import authority_state, deterministic_id, sha256_bytes

try:
    from evidence_store.model import canonical_json
except ModuleNotFoundError:
    from scripts.evidence_store.model import canonical_json


PREFLIGHT_ID_RE = re.compile(r"^lscp_[0-9a-f]{64}$")
MIN_OBSERVATIONS_PER_CLUSTER = 3
MIN_CLUSTERS_FOR_CORRELATION = 8
MEASURES = {
    "bridge.pressure_risk": "pressure_weight_context",
    "bridge.mode_packing": "mode_packing_context",
    "bridge.spectral_density_gradient": "density_gradient_context",
    "bridge.lambda1_lambda2_gap": "head_shoulder_gap_context",
    "astrid_shadow.field_norm": "shadow_field_norm_context",
    "astrid_shadow.field_norm_delta": "shadow_trajectory_context",
    "astrid_shadow.dispersal_potential": "shadow_dispersal_context",
}
LEGACY_MEASURE_KEYS_V1 = frozenset(
    {
        "bridge.pressure_risk",
        "bridge.mode_packing",
        "bridge.spectral_density_gradient",
        "astrid_shadow.field_norm",
        "astrid_shadow.field_norm_delta",
        "astrid_shadow.dispersal_potential",
    }
)
ACCEPTED_MEASURE_KEYSETS_V1 = frozenset(
    {
        frozenset(MEASURES),
        LEGACY_MEASURE_KEYS_V1,
    }
)


def _evidence_only(value: Any) -> bool:
    return bool(
        isinstance(value, dict)
        and value.get("schema") == "artifact_authority_state_v1"
        and value.get("schema_version") == 1
        and value.get("state") == "evidence_only"
        and value.get("witness_only") is True
        and all(
            value.get(marker) is not True
            for marker in (
                "live_eligible_now",
                "auto_approved",
                "grants_approval",
                "edits_source_now",
            )
        )
    )


def _finite(value: Any) -> float | None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    scalar = float(value)
    return scalar if math.isfinite(scalar) else None


def _fresh_measurements(event: dict[str, Any]) -> dict[str, float]:
    witness = event.get("witness")
    if not isinstance(witness, dict) or witness.get("schema") != (
        "temporal_lived_state_witness_v1"
    ):
        return {}
    result: dict[str, float] = {}
    observations = witness.get("parameter_observations_v1")
    if not isinstance(observations, list):
        return result
    for observation in observations:
        if not isinstance(observation, dict):
            continue
        name = observation.get("name")
        scalar = _finite(observation.get("value"))
        if (
            name in MEASURES
            and scalar is not None
            and observation.get("observation_kind") == "runtime_observed"
            and observation.get("fresh") is True
            and observation.get("direct_causation_claimed") is False
        ):
            result[str(name)] = scalar
    return result


def _measurement_summary(values: list[float], member_count: int) -> dict[str, Any]:
    count = len(values)
    available = count >= MIN_OBSERVATIONS_PER_CLUSTER
    return {
        "sample_count": count,
        "member_coverage": round(count / max(1, member_count), 6),
        "mean": round(sum(values) / count, 9) if available else None,
        "minimum": round(min(values), 9) if available else None,
        "maximum": round(max(values), 9) if available else None,
        "status": "observed" if available else "insufficient_exact_fresh_samples",
        "minimum_required": MIN_OBSERVATIONS_PER_CLUSTER,
        "relationship_scope": "contemporaneous_association_not_mechanism_or_causation",
    }


def _correlation(rows: list[tuple[float, float]]) -> dict[str, Any]:
    count = len(rows)
    result: dict[str, Any] = {
        "cluster_count": count,
        "minimum_required": MIN_CLUSTERS_FOR_CORRELATION,
        "pearson_r": None,
        "causation_established": False,
        "relationship_scope": "descriptive_correlation_only_not_mechanism_or_causation",
    }
    if count < MIN_CLUSTERS_FOR_CORRELATION:
        result["status"] = "insufficient_cluster_count"
        return result
    mean_x = sum(row[0] for row in rows) / count
    mean_y = sum(row[1] for row in rows) / count
    covariance = sum((x - mean_x) * (y - mean_y) for x, y in rows)
    variance_x = sum((x - mean_x) ** 2 for x, _ in rows)
    variance_y = sum((y - mean_y) ** 2 for _, y in rows)
    denominator = math.sqrt(variance_x * variance_y)
    if denominator <= 1e-15:
        result["status"] = "insufficient_variance"
        return result
    result["status"] = "descriptive_correlation_available"
    result["pearson_r"] = round(covariance / denominator, 9)
    return result


def _cluster_event(
    cluster: dict[str, Any],
    witness_events: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    members = cluster.get("member_refs")
    members = members if isinstance(members, list) else []
    values: dict[str, list[float]] = {name: [] for name in MEASURES}
    exact_context_members = 0
    for member in members:
        if not isinstance(member, dict):
            continue
        event = witness_events.get(str(member.get("witness_id") or ""))
        if not isinstance(event, dict):
            continue
        measurements = _fresh_measurements(event)
        exact_context_members += int(bool(measurements))
        for name, scalar in measurements.items():
            values[name].append(scalar)
    summaries = {
        name: _measurement_summary(values[name], len(members))
        for name in sorted(MEASURES)
    }
    cluster_id = str(cluster.get("cluster_id") or "")
    membership_sha256 = str(cluster.get("membership_sha256") or "")
    concordance = {
        "schema": "lived_state_concordance_cluster_v1",
        "schema_version": 1,
        "cluster_id": cluster_id,
        "temporal_cluster_membership_sha256": membership_sha256,
        "temporal_density_weight": cluster.get("temporal_density_weight"),
        "association_count": cluster.get("association_count"),
        "exact_fresh_context_member_count": exact_context_members,
        "measurements": summaries,
        "concordance_status": (
            "measurement_ready"
            if any(row["status"] == "observed" for row in summaries.values())
            else "capture_insufficient"
        ),
        "mechanism_established": False,
        "causation_established": False,
        "felt_state_inferred": False,
        "closure_propagated": False,
        "evidence_sufficiency_propagated": False,
        "authority_propagated": False,
        "felt_resolution_propagated": False,
        "raw_prose_included": False,
        "artifact_authority_state_v1": authority_state(),
    }
    content_sha256 = sha256_bytes(canonical_json(concordance).encode())
    anchor = members[0] if members else {}
    return {
        "schema": "lived_state_witness_domain_event_v1",
        "schema_version": 1,
        "event_type": "lived_state_concordance_cluster_observed",
        "aggregate_type": "lived_state_concordance_cluster",
        "aggregate_id": cluster_id,
        "cluster_id": cluster_id,
        "witness_id": anchor.get("witness_id"),
        "introspection_id": anchor.get("introspection_id"),
        "concordance": concordance,
        "idempotency_key": (
            f"lived-state-concordance-cluster:{cluster_id}:"
            f"{membership_sha256}:{content_sha256}"
        ),
        "artifact_authority_state_v1": authority_state(),
    }


def _preflight_event(cluster_events: list[dict[str, Any]]) -> dict[str, Any]:
    correlations: dict[str, dict[str, Any]] = {}
    for name in sorted(MEASURES):
        rows: list[tuple[float, float]] = []
        for event in cluster_events:
            concordance = event["concordance"]
            measurement = concordance["measurements"][name]
            mean = _finite(measurement.get("mean"))
            density = _finite(concordance.get("temporal_density_weight"))
            if mean is not None and density is not None:
                rows.append((density, mean))
        correlations[name] = _correlation(rows)
    available = sum(
        row["status"] == "descriptive_correlation_available"
        for row in correlations.values()
    )
    input_identity = sha256_bytes(
        canonical_json(
            [
                [event["cluster_id"], event["concordance"]]
                for event in cluster_events
            ]
        ).encode()
    )
    preflight_id = deterministic_id("lscp", (input_identity,))
    preflight = {
        "schema": "lived_state_concordance_preflight_v1",
        "schema_version": 1,
        "preflight_id": preflight_id,
        "cluster_count": len(cluster_events),
        "correlations": correlations,
        "available_correlation_count": available,
        "status": (
            "descriptive_comparison_ready"
            if available
            else "insufficient_contemporaneous_evidence"
        ),
        "felt_density_proxy": {
            "status": "not_computed_without_reviewed_measurement_contract",
            "value": None,
            "scope": "optional_review_salience_candidate_not_felt_state_or_felt_weight",
        },
        "density_gradient_change": {
            "status": "approval_pending",
            "applied": False,
            "required_authority": "tier_4_or_5_operator_approval",
        },
        "mechanism_established": False,
        "causation_established": False,
        "felt_state_inferred": False,
        "closure_propagated": False,
        "evidence_sufficiency_propagated": False,
        "authority_propagated": False,
        "felt_resolution_propagated": False,
        "raw_prose_included": False,
        "artifact_authority_state_v1": authority_state(),
    }
    witness_id = f"lsw_{sha256_bytes(preflight_id.encode())}"
    return {
        "schema": "lived_state_witness_domain_event_v1",
        "schema_version": 1,
        "event_type": "lived_state_concordance_preflight_recorded",
        "aggregate_type": "lived_state_concordance_preflight",
        "aggregate_id": "current",
        "witness_id": witness_id,
        "preflight_id": preflight_id,
        "preflight": preflight,
        "idempotency_key": f"lived-state-concordance-preflight:{input_identity}",
        "artifact_authority_state_v1": authority_state(),
    }


def build_concordance_events(
    witness_events: Iterable[dict[str, Any]],
    temporal_cluster_events: Iterable[dict[str, Any]],
) -> list[dict[str, Any]]:
    """Build bounded aggregates without reading or interpreting report prose."""

    witness_index = {
        str(event.get("witness_id") or ""): event for event in witness_events
    }
    events: list[dict[str, Any]] = []
    for cluster_event in temporal_cluster_events:
        cluster = cluster_event.get("cluster")
        if not isinstance(cluster, dict):
            continue
        event = _cluster_event(cluster, witness_index)
        events.append(event)
    events.append(_preflight_event(events))
    return events


def validate_concordance_cluster(value: Any) -> list[str]:
    if not isinstance(value, dict):
        return ["concordance_cluster:not_object"]
    errors: list[str] = []
    if value.get("schema") != "lived_state_concordance_cluster_v1":
        errors.append("concordance_cluster:schema")
    if value.get("schema_version") != 1:
        errors.append("concordance_cluster:schema_version")
    cluster_id = value.get("cluster_id")
    if not isinstance(cluster_id, str) or re.fullmatch(
        r"lstc_[0-9a-f]{64}", cluster_id
    ) is None:
        errors.append("concordance_cluster:cluster_id")
    membership_sha256 = value.get("temporal_cluster_membership_sha256")
    if not isinstance(membership_sha256, str) or re.fullmatch(
        r"[0-9a-f]{64}", membership_sha256
    ) is None:
        errors.append("concordance_cluster:membership_sha256")
    density = _finite(value.get("temporal_density_weight"))
    if density is None or not 0.0 <= density <= 1.0:
        errors.append("concordance_cluster:temporal_density_weight")
    association_count = value.get("association_count")
    if (
        not isinstance(association_count, int)
        or isinstance(association_count, bool)
        or association_count < MIN_OBSERVATIONS_PER_CLUSTER
    ):
        errors.append("concordance_cluster:association_count")
    exact_count = value.get("exact_fresh_context_member_count")
    if (
        not isinstance(exact_count, int)
        or isinstance(exact_count, bool)
        or exact_count < 0
        or (
            isinstance(association_count, int)
            and exact_count > association_count
        )
    ):
        errors.append("concordance_cluster:exact_fresh_context_member_count")
    measurements = value.get("measurements")
    if (
        not isinstance(measurements, dict)
        or frozenset(measurements) not in ACCEPTED_MEASURE_KEYSETS_V1
    ):
        errors.append("concordance_cluster:measurements")
    else:
        for name, row in measurements.items():
            if not isinstance(row, dict):
                errors.append(f"concordance_cluster:measurement:{name}")
                continue
            status = row.get("status")
            count = row.get("sample_count")
            if status not in {
                "observed",
                "insufficient_exact_fresh_samples",
            }:
                errors.append(f"concordance_cluster:measurement_status:{name}")
            if (
                not isinstance(count, int)
                or isinstance(count, bool)
                or count < 0
                or (
                    isinstance(association_count, int)
                    and count > association_count
                )
            ):
                errors.append(f"concordance_cluster:measurement_count:{name}")
            if row.get("relationship_scope") != (
                "contemporaneous_association_not_mechanism_or_causation"
            ):
                errors.append(f"concordance_cluster:measurement_scope:{name}")
    if value.get("concordance_status") not in {
        "measurement_ready",
        "capture_insufficient",
    }:
        errors.append("concordance_cluster:status")
    for field in (
        "mechanism_established",
        "causation_established",
        "felt_state_inferred",
        "closure_propagated",
        "evidence_sufficiency_propagated",
        "authority_propagated",
        "felt_resolution_propagated",
        "raw_prose_included",
    ):
        if value.get(field) is not False:
            errors.append(f"concordance_cluster:{field}")
    if not _evidence_only(value.get("artifact_authority_state_v1")):
        errors.append("concordance_cluster:authority")
    return errors


def validate_concordance_preflight(value: Any) -> list[str]:
    if not isinstance(value, dict):
        return ["concordance_preflight:not_object"]
    errors: list[str] = []
    if value.get("schema") != "lived_state_concordance_preflight_v1":
        errors.append("concordance_preflight:schema")
    preflight_id = value.get("preflight_id")
    if not isinstance(preflight_id, str) or PREFLIGHT_ID_RE.fullmatch(preflight_id) is None:
        errors.append("concordance_preflight:preflight_id")
    proxy = value.get("felt_density_proxy")
    if not isinstance(proxy, dict) or proxy.get("value") is not None:
        errors.append("concordance_preflight:felt_density_proxy_inferred")
    elif proxy != {
        "status": "not_computed_without_reviewed_measurement_contract",
        "value": None,
        "scope": "optional_review_salience_candidate_not_felt_state_or_felt_weight",
    }:
        errors.append("concordance_preflight:felt_density_proxy_scope")
    density_change = value.get("density_gradient_change")
    if not isinstance(density_change, dict) or density_change.get("applied") is not False:
        errors.append("concordance_preflight:density_gradient_changed")
    elif density_change != {
        "status": "approval_pending",
        "applied": False,
        "required_authority": "tier_4_or_5_operator_approval",
    }:
        errors.append("concordance_preflight:density_gradient_authority")
    correlations = value.get("correlations")
    if (
        not isinstance(correlations, dict)
        or frozenset(correlations) not in ACCEPTED_MEASURE_KEYSETS_V1
    ):
        errors.append("concordance_preflight:correlations")
    else:
        for name, row in correlations.items():
            if (
                not isinstance(row, dict)
                or row.get("causation_established") is not False
                or row.get("relationship_scope")
                != "descriptive_correlation_only_not_mechanism_or_causation"
            ):
                errors.append(f"concordance_preflight:correlation:{name}")
    for field in (
        "mechanism_established",
        "causation_established",
        "felt_state_inferred",
        "closure_propagated",
        "evidence_sufficiency_propagated",
        "authority_propagated",
        "felt_resolution_propagated",
        "raw_prose_included",
    ):
        if value.get(field) is not False:
            errors.append(f"concordance_preflight:{field}")
    if not _evidence_only(value.get("artifact_authority_state_v1")):
        errors.append("concordance_preflight:authority")
    return errors
