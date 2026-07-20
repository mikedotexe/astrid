"""Stable-baseline delta assignment for living claim families."""

from __future__ import annotations

import hashlib
from typing import Any, Callable

from claim_family_matcher import (
    MATCHER_VERSION,
    MATCH_THRESHOLD,
    SUGGESTION_THRESHOLD,
    weighted_similarity,
)

try:
    from evidence_store.model import canonical_json
except ModuleNotFoundError:
    from scripts.evidence_store.model import canonical_json


def _digest(value: Any) -> str:
    return hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest()


def _authority(state: str = "evidence_only") -> dict[str, Any]:
    return {
        "schema": "artifact_authority_state_v1",
        "schema_version": 1,
        "state": state,
        "live_eligible_now": False,
        "auto_approved": False,
        "grants_approval": False,
        "edits_source_now": False,
    }


def _incremental_family_record(claim: Any, family_id: str) -> dict[str, Any]:
    return {
        "schema": "claim_family_v1",
        "schema_version": 1,
        "family_id": family_id,
        "matcher_version": f"{MATCHER_VERSION}+incremental_v3",
        "authority_class": claim.authority_class,
        "target_surface": claim.target_surface,
        "requested_outcome": claim.requested_outcome,
        "polarity": claim.polarity,
        "member_claim_ids": [claim.canonical_claim_id],
        "member_count": 1,
        "minimum_pair_similarity": None,
        "membership_propagates_closure": False,
        "membership_propagates_evidence_sufficiency": False,
        "membership_propagates_supersession": False,
        "membership_propagates_authority": False,
        "artifact_authority_state_v1": _authority(
            "approval_pending"
            if claim.authority_class.startswith("approval_pending")
            else "evidence_only"
        ),
    }


def _candidate_families(
    claim: Any,
    status: dict[str, Any],
) -> tuple[list[tuple[float, str]], list[dict[str, Any]]]:
    qualifying: list[tuple[float, str]] = []
    suggestions: list[dict[str, Any]] = []
    for family_id, family in sorted((status.get("families") or {}).items()):
        if not isinstance(family, dict):
            continue
        criteria = (
            str(family.get("authority_class") or ""),
            str(family.get("target_surface") or ""),
            str(family.get("requested_outcome") or ""),
            str(family.get("polarity") or ""),
        )
        if criteria != claim.match_key:
            continue
        claims = family.get("claims")
        if not isinstance(claims, dict) or not claims:
            continue
        scores = [
            weighted_similarity(claim.summary, str(record.get("text") or ""))
            for record in claims.values()
            if isinstance(record, dict) and record.get("text")
        ]
        if not scores:
            continue
        minimum = min(scores)
        average = sum(scores) / len(scores)
        if minimum >= MATCH_THRESHOLD:
            qualifying.append((average, family_id))
        elif max(scores) >= SUGGESTION_THRESHOLD:
            suggestions.append(
                {
                    "family_id": family_id,
                    "similarity": round(max(scores), 6),
                    "canonical": False,
                }
            )
    return sorted(qualifying, reverse=True), sorted(
        suggestions,
        key=lambda item: (-item["similarity"], item["family_id"]),
    )[:3]


def incremental_events(
    claims: list[Any],
    events: list[dict[str, Any]],
    *,
    project_status: Callable[
        [list[dict[str, Any]], str | None],
        dict[str, Any],
    ],
) -> list[dict[str, Any]]:
    """Return only migration, unseen-claim, or changed-content events."""

    status = project_status(events, None)
    creation_count = sum(
        event.get("event_type") == "claim_family_created"
        for event in events
    )
    membership_count = sum(
        event.get("event_type")
        in {
            "claim_family_membership_assigned",
            "claim_family_membership_corrected",
        }
        for event in events
    )
    has_v3_migration = any(
        event.get("event_type") == "claim_family_v3_migration_completed"
        for event in events
    )
    return _incremental_events_from_status(
        claims,
        status,
        migration_counts=(creation_count, membership_count),
        has_v3_migration=has_v3_migration,
    )


def incremental_events_from_status(
    claims: list[Any],
    status: dict[str, Any],
) -> list[dict[str, Any]]:
    """Produce deltas from an already validated materialized baseline."""

    migration = status.get("migration_receipt")
    has_v3_migration = (
        isinstance(migration, dict)
        and migration.get("schema")
        == "claim_family_v3_migration_receipt_v1"
    )
    if not has_v3_migration:
        raise ValueError(
            "incremental claim-family baseline lacks a V3 migration receipt"
        )
    return _incremental_events_from_status(
        claims,
        status,
        migration_counts=None,
        has_v3_migration=True,
    )


def _incremental_events_from_status(
    claims: list[Any],
    status: dict[str, Any],
    *,
    migration_counts: tuple[int, int] | None,
    has_v3_migration: bool,
) -> list[dict[str, Any]]:
    assignments = {
        claim_id: (family_id, record)
        for family_id, family in (status.get("families") or {}).items()
        if isinstance(family, dict)
        for claim_id, record in (family.get("claims") or {}).items()
        if isinstance(record, dict)
    }
    generated: list[dict[str, Any]] = []
    changed_family_ids: set[str] = set()
    if not has_v3_migration:
        if migration_counts is None:
            raise ValueError("migration counts are required for V3 bootstrap")
        creation_count, membership_count = migration_counts
        migration = {
            "schema": "claim_family_v3_migration_receipt_v1",
            "schema_version": 1,
            "logical_family_count": len(status.get("families") or {}),
            "logical_assignment_count": len(assignments),
            "historical_family_event_count": creation_count,
            "historical_membership_event_count": membership_count,
            "duplicate_family_restatement_count": max(
                0,
                creation_count - len(status.get("families") or {}),
            ),
            "duplicate_membership_restatement_count": max(
                0,
                membership_count - len(assignments),
            ),
            "history_rewritten": False,
            "family_ids": sorted(status.get("families") or {}),
            "artifact_authority_state_v1": _authority(),
        }
        generated.append(
            {
                "schema": "claim_family_domain_event_v1",
                "schema_version": 1,
                "event_type": "claim_family_v3_migration_completed",
                "aggregate_type": "claim_family_migration",
                "aggregate_id": "incremental_v3_baseline",
                "migration_receipt": migration,
                "idempotency_key": "claim_family_v3_migration:baseline",
                "artifact_authority_state_v1": _authority(),
            }
        )

    for claim in sorted(claims, key=lambda item: item.canonical_claim_id):
        existing = assignments.get(claim.canonical_claim_id)
        if existing:
            family_id, prior_record = existing
            prior_hash = str(
                prior_record.get("canonical_claim_record_sha256") or ""
            )
            current_hash = str(
                claim.record.get("canonical_claim_record_sha256") or ""
            )
            if prior_hash != current_hash:
                generated.append(
                    {
                        "schema": "claim_family_domain_event_v1",
                        "schema_version": 1,
                        "event_type": "claim_content_changed",
                        "aggregate_type": "claim_family",
                        "aggregate_id": family_id,
                        "family_id": family_id,
                        "canonical_claim_id": claim.canonical_claim_id,
                        "previous_claim_record_sha256": prior_hash,
                        "claim": claim.record,
                        "membership_changed": False,
                        "idempotency_key": (
                            f"claim_content_changed:{claim.canonical_claim_id}:"
                            f"{current_hash}"
                        ),
                        "artifact_authority_state_v1": _authority(),
                    }
                )
                changed_family_ids.add(family_id)
            continue

        qualifying, suggestions = _candidate_families(claim, status)
        if len(qualifying) == 1:
            family_id = qualifying[0][1]
            family_record = status["families"][family_id]
        else:
            family_id = (
                "family_"
                + _digest(
                    [
                        "incremental_v3",
                        claim.match_key,
                        claim.canonical_claim_id,
                    ]
                )[:20]
            )
            family = _incremental_family_record(claim, family_id)
            family_record = {**family, "claims": {}}
            status.setdefault("families", {})[family_id] = family_record
            generated.append(
                {
                    "schema": "claim_family_domain_event_v1",
                    "schema_version": 1,
                    "event_type": "claim_family_created",
                    "aggregate_type": "claim_family",
                    "aggregate_id": family_id,
                    "family": family,
                    "idempotency_key": f"claim_family_created:{family_id}",
                    "artifact_authority_state_v1": family[
                        "artifact_authority_state_v1"
                    ],
                }
            )
        generated.append(
            {
                "schema": "claim_family_domain_event_v1",
                "schema_version": 1,
                "event_type": "claim_family_membership_assigned",
                "aggregate_type": "claim_family",
                "aggregate_id": family_id,
                "family_id": family_id,
                "canonical_claim_id": claim.canonical_claim_id,
                "claim": claim.record,
                "authority_class": claim.authority_class,
                "target_surface": claim.target_surface,
                "requested_outcome": claim.requested_outcome,
                "polarity": claim.polarity,
                "noncanonical_suggestions": suggestions,
                "ambiguous_qualifying_family_ids": (
                    [family for _score, family in qualifying]
                    if len(qualifying) > 1
                    else []
                ),
                "closure_propagated": False,
                "evidence_sufficiency_propagated": False,
                "supersession_propagated": False,
                "authority_propagated": False,
                "idempotency_key": (
                    f"claim_membership:{claim.canonical_claim_id}:{family_id}"
                ),
                "artifact_authority_state_v1": _authority(
                    "approval_pending"
                    if claim.authority_class.startswith("approval_pending")
                    else "evidence_only"
                ),
            }
        )
        if len(qualifying) > 1:
            generated[-1]["noncanonical_suggestions"] = [
                {
                    "family_id": candidate_family,
                    "similarity": round(score, 6),
                    "canonical": False,
                }
                for score, candidate_family in qualifying[:3]
            ] + suggestions
        family_record.setdefault("claims", {})[
            claim.canonical_claim_id
        ] = claim.record
        assignments[claim.canonical_claim_id] = (family_id, claim.record)
        changed_family_ids.add(family_id)

    delta_events = [
        event
        for event in generated
        if event.get("event_type") != "claim_family_v3_migration_completed"
    ]
    if delta_events:
        delta_id = _digest(
            sorted(str(event.get("idempotency_key") or "") for event in delta_events)
        )
        generated.append(
            {
                "schema": "claim_family_domain_event_v1",
                "schema_version": 1,
                "event_type": "claim_family_delta_projected",
                "aggregate_type": "claim_family_projection",
                "aggregate_id": delta_id[:20],
                "changed_family_ids": sorted(changed_family_ids),
                "delta_event_count": len(delta_events),
                "idempotency_key": f"claim_family_delta:{delta_id}",
                "artifact_authority_state_v1": _authority(),
            }
        )
    return generated
