"""Bounded, explicitly non-causal density context for temporal witnesses."""

from __future__ import annotations

from collections import defaultdict
import re
from typing import Any, Iterable

from .model import (
    WITNESS_ID_RE,
    authority_state,
    deterministic_id,
    sha256_bytes,
)

try:
    from evidence_store.model import canonical_json
except ModuleNotFoundError:
    from scripts.evidence_store.model import canonical_json


CLUSTER_ID_RE = re.compile(r"^lstc_[0-9a-f]{64}$")
WINDOW_MS = 2 * 60 * 60 * 1_000
MIN_MEMBERS = 3
MAX_MEMBER_REFS = 256
DENSITY_SATURATION_COUNT = 8


def _density_class(count: int) -> str:
    if count >= DENSITY_SATURATION_COUNT:
        return "high_density"
    if count >= 5:
        return "dense"
    return "clustered"


def _cluster_event(cluster: dict[str, Any]) -> dict[str, Any]:
    anchor = cluster["member_refs"][0]
    membership_sha256 = str(cluster["membership_sha256"])
    cluster_id = str(cluster["cluster_id"])
    return {
        "schema": "lived_state_witness_domain_event_v1",
        "schema_version": 1,
        "event_type": "lived_state_temporal_cluster_observed",
        "aggregate_type": "lived_state_temporal_cluster",
        "aggregate_id": cluster_id,
        "cluster_id": cluster_id,
        "witness_id": anchor["witness_id"],
        "introspection_id": anchor["introspection_id"],
        "cluster": cluster,
        "idempotency_key": (
            f"lived-state-temporal-cluster:{cluster_id}:{membership_sha256}"
        ),
        "artifact_authority_state_v1": authority_state(),
    }


def build_temporal_cluster_events(
    witness_events: Iterable[dict[str, Any]],
) -> list[dict[str, Any]]:
    """Group temporal-only authorship evidence into fixed, reviewable windows."""

    grouped: dict[tuple[str, int], list[dict[str, Any]]] = defaultdict(list)
    for event in witness_events:
        alignment = event.get("alignment")
        witness = event.get("witness")
        if not isinstance(alignment, dict) or not isinstance(witness, dict):
            continue
        if alignment.get("outcome") not in {
            "temporal_association_only",
            "same_deployment",
        }:
            continue
        receipt_id = alignment.get("deployment_receipt_id")
        authored_at_ms = witness.get("authored_at_unix_ms")
        witness_id = event.get("witness_id")
        introspection_id = event.get("introspection_id")
        if (
            not isinstance(receipt_id, str)
            or not receipt_id
            or len(receipt_id) > 300
            or not isinstance(authored_at_ms, int)
            or isinstance(authored_at_ms, bool)
            or authored_at_ms < 1
            or not isinstance(witness_id, str)
            or WITNESS_ID_RE.fullmatch(witness_id) is None
            or not isinstance(introspection_id, str)
            or not introspection_id
            or len(introspection_id) > 300
        ):
            continue
        window_start_ms = authored_at_ms - (authored_at_ms % WINDOW_MS)
        grouped[(receipt_id, window_start_ms)].append(
            {
                "witness_id": witness_id,
                "introspection_id": introspection_id,
                "authored_at_unix_ms": authored_at_ms,
            }
        )

    events: list[dict[str, Any]] = []
    for (receipt_id, window_start_ms), members in sorted(grouped.items()):
        members.sort(
            key=lambda row: (
                int(row["authored_at_unix_ms"]),
                str(row["witness_id"]),
            )
        )
        if len(members) < MIN_MEMBERS:
            continue
        bounded_members = members[:MAX_MEMBER_REFS]
        membership_sha256 = sha256_bytes(canonical_json(members).encode())
        cluster_id = deterministic_id(
            "lstc", (receipt_id, window_start_ms, WINDOW_MS)
        )
        count = len(members)
        cluster = {
            "schema": "lived_state_temporal_cluster_v1",
            "schema_version": 1,
            "cluster_id": cluster_id,
            "deployment_receipt_id": receipt_id,
            "window_start_unix_ms": window_start_ms,
            "window_end_exclusive_unix_ms": window_start_ms + WINDOW_MS,
            "window_policy": "fixed_utc_epoch_two_hour_window",
            "first_authored_at_unix_ms": int(
                members[0]["authored_at_unix_ms"]
            ),
            "last_authored_at_unix_ms": int(
                members[-1]["authored_at_unix_ms"]
            ),
            "association_count": count,
            "member_refs": bounded_members,
            "member_overflow_count": count - len(bounded_members),
            "membership_sha256": membership_sha256,
            "associations_per_hour": round(count / 2.0, 6),
            "temporal_density_weight": round(
                min(1.0, count / DENSITY_SATURATION_COUNT), 6
            ),
            "density_class": _density_class(count),
            "density_scope": (
                "review_salience_only_not_causal_strength_or_felt_weight"
            ),
            "relationship_scope": (
                "repeated_temporal_association_not_mechanism_or_causation"
            ),
            "causation_established": False,
            "direct_causation_claimed": False,
            "closure_propagated": False,
            "evidence_sufficiency_propagated": False,
            "authority_propagated": False,
            "felt_resolution_propagated": False,
            "raw_prose_included": False,
            "artifact_authority_state_v1": authority_state(),
        }
        events.append(_cluster_event(cluster))
    return events


def validate_temporal_cluster(value: Any) -> list[str]:
    """Revalidate a persisted cluster as untrusted evidence input."""

    errors: list[str] = []
    if not isinstance(value, dict):
        return ["temporal_cluster:not_object"]
    expected_keys = {
        "schema",
        "schema_version",
        "cluster_id",
        "deployment_receipt_id",
        "window_start_unix_ms",
        "window_end_exclusive_unix_ms",
        "window_policy",
        "first_authored_at_unix_ms",
        "last_authored_at_unix_ms",
        "association_count",
        "member_refs",
        "member_overflow_count",
        "membership_sha256",
        "associations_per_hour",
        "temporal_density_weight",
        "density_class",
        "density_scope",
        "relationship_scope",
        "causation_established",
        "direct_causation_claimed",
        "closure_propagated",
        "evidence_sufficiency_propagated",
        "authority_propagated",
        "felt_resolution_propagated",
        "raw_prose_included",
        "artifact_authority_state_v1",
    }
    normalized_authority_keys = {
        "authority_projection_v2",
        "live_eligible_now",
        "auto_approved",
        "grants_approval",
        "edits_source_now",
    }
    if not expected_keys.issubset(value) or not set(value).issubset(
        expected_keys | normalized_authority_keys
    ):
        errors.append("temporal_cluster:fields")
    if value.get("schema") != "lived_state_temporal_cluster_v1":
        errors.append("temporal_cluster:schema")
    if value.get("schema_version") != 1:
        errors.append("temporal_cluster:schema_version")
    cluster_id = value.get("cluster_id")
    receipt_id = value.get("deployment_receipt_id")
    start = value.get("window_start_unix_ms")
    end = value.get("window_end_exclusive_unix_ms")
    count = value.get("association_count")
    overflow = value.get("member_overflow_count")
    if not isinstance(cluster_id, str) or CLUSTER_ID_RE.fullmatch(cluster_id) is None:
        errors.append("temporal_cluster:cluster_id")
    if not isinstance(receipt_id, str) or not receipt_id or len(receipt_id) > 300:
        errors.append("temporal_cluster:deployment_receipt_id")
    if not isinstance(start, int) or isinstance(start, bool) or start < 0:
        errors.append("temporal_cluster:window_start")
    if (
        not isinstance(end, int)
        or isinstance(end, bool)
        or not isinstance(start, int)
        or end != start + WINDOW_MS
    ):
        errors.append("temporal_cluster:window_end")
    if value.get("window_policy") != "fixed_utc_epoch_two_hour_window":
        errors.append("temporal_cluster:window_policy")
    if (
        isinstance(cluster_id, str)
        and isinstance(receipt_id, str)
        and isinstance(start, int)
        and cluster_id != deterministic_id("lstc", (receipt_id, start, WINDOW_MS))
    ):
        errors.append("temporal_cluster:cluster_id_mismatch")
    members = value.get("member_refs")
    if not isinstance(members, list) or not members or len(members) > MAX_MEMBER_REFS:
        errors.append("temporal_cluster:member_refs")
        members = []
    canonical_members: list[dict[str, Any]] = []
    for index, member in enumerate(members):
        if not isinstance(member, dict) or set(member) != {
            "witness_id",
            "introspection_id",
            "authored_at_unix_ms",
        }:
            errors.append(f"temporal_cluster:member:{index}")
            continue
        witness_id = member.get("witness_id")
        introspection_id = member.get("introspection_id")
        authored_at_ms = member.get("authored_at_unix_ms")
        if not isinstance(witness_id, str) or WITNESS_ID_RE.fullmatch(witness_id) is None:
            errors.append(f"temporal_cluster:member_witness:{index}")
        if (
            not isinstance(introspection_id, str)
            or not introspection_id
            or len(introspection_id) > 300
            or introspection_id.startswith("/")
        ):
            errors.append(f"temporal_cluster:member_introspection:{index}")
        if (
            not isinstance(authored_at_ms, int)
            or isinstance(authored_at_ms, bool)
            or not isinstance(start, int)
            or not isinstance(end, int)
            or authored_at_ms < start
            or authored_at_ms >= end
        ):
            errors.append(f"temporal_cluster:member_time:{index}")
        canonical_members.append(member)
    ordered = sorted(
        canonical_members,
        key=lambda row: (
            int(row.get("authored_at_unix_ms") or 0),
            str(row.get("witness_id") or ""),
        ),
    )
    if canonical_members != ordered:
        errors.append("temporal_cluster:member_order")
    if not isinstance(count, int) or isinstance(count, bool) or count < MIN_MEMBERS:
        errors.append("temporal_cluster:association_count")
    if not isinstance(overflow, int) or isinstance(overflow, bool) or overflow < 0:
        errors.append("temporal_cluster:member_overflow_count")
    if isinstance(count, int) and isinstance(overflow, int) and count != len(members) + overflow:
        errors.append("temporal_cluster:member_count_mismatch")
    if isinstance(count, int) and overflow == 0:
        expected_membership = sha256_bytes(canonical_json(members).encode())
        if value.get("membership_sha256") != expected_membership:
            errors.append("temporal_cluster:membership_sha256_mismatch")
    elif not isinstance(value.get("membership_sha256"), str) or re.fullmatch(
        r"[0-9a-f]{64}", str(value.get("membership_sha256") or "")
    ) is None:
        errors.append("temporal_cluster:membership_sha256")
    if isinstance(count, int):
        if value.get("associations_per_hour") != round(count / 2.0, 6):
            errors.append("temporal_cluster:association_rate")
        if value.get("temporal_density_weight") != round(
            min(1.0, count / DENSITY_SATURATION_COUNT), 6
        ):
            errors.append("temporal_cluster:density_weight")
        if value.get("density_class") != _density_class(count):
            errors.append("temporal_cluster:density_class")
    for field in (
        "causation_established",
        "direct_causation_claimed",
        "closure_propagated",
        "evidence_sufficiency_propagated",
        "authority_propagated",
        "felt_resolution_propagated",
        "raw_prose_included",
    ):
        if value.get(field) is not False:
            errors.append(f"temporal_cluster:{field}")
    if value.get("density_scope") != (
        "review_salience_only_not_causal_strength_or_felt_weight"
    ):
        errors.append("temporal_cluster:density_scope")
    if value.get("relationship_scope") != (
        "repeated_temporal_association_not_mechanism_or_causation"
    ):
        errors.append("temporal_cluster:relationship_scope")
    nested_authority = value.get("artifact_authority_state_v1")
    normalized_nested_authority = {
        "schema": "artifact_authority_state_v1",
        "schema_version": 1,
        "state": "evidence_only",
        "witness_only": True,
    }
    if nested_authority not in (authority_state(), normalized_nested_authority):
        errors.append("temporal_cluster:authority")
    present_authority_keys = normalized_authority_keys & set(value)
    if present_authority_keys:
        projection = value.get("authority_projection_v2")
        expected_projection = {
            "schema": "artifact_authority_projection_v2",
            "schema_version": 2,
            "source_state": "evidence_only",
            "live_eligible_now": False,
            "auto_approved": False,
            "grants_approval": False,
            "edits_source_now": False,
        }
        if projection != expected_projection:
            errors.append("temporal_cluster:authority_projection_v2")
        for marker in (
            "live_eligible_now",
            "auto_approved",
            "grants_approval",
            "edits_source_now",
        ):
            if value.get(marker) is not False:
                errors.append(f"temporal_cluster:{marker}")
    first = value.get("first_authored_at_unix_ms")
    last = value.get("last_authored_at_unix_ms")
    if members:
        if first != members[0].get("authored_at_unix_ms"):
            errors.append("temporal_cluster:first_authored_at")
        if last != members[-1].get("authored_at_unix_ms") and overflow == 0:
            errors.append("temporal_cluster:last_authored_at")
    return errors
