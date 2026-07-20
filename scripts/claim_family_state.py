"""Incremental materialization for append-only claim-family events."""

from __future__ import annotations

from typing import Any, Iterable

from claim_family_matcher import MATCHER_VERSION, MATCH_THRESHOLD


def _authority() -> dict[str, Any]:
    return {
        "schema": "artifact_authority_state_v1",
        "schema_version": 1,
        "state": "evidence_only",
        "live_eligible_now": False,
        "auto_approved": False,
        "grants_approval": False,
        "edits_source_now": False,
    }


def apply_claim_family_events(
    status: dict[str, Any] | None,
    events: Iterable[dict[str, Any]],
    deployment_receipt_id: str | None,
) -> dict[str, Any]:
    """Apply canonical event tails to a validated materialized status."""

    current = status or {}
    family_records: dict[str, dict[str, Any]] = {}
    assignments: dict[str, dict[str, Any]] = {}
    for family_id, family in (current.get("families") or {}).items():
        if not isinstance(family, dict):
            continue
        family_records[str(family_id)] = {
            key: value
            for key, value in family.items()
            if key not in {"claims", "member_claim_ids", "member_count"}
        }
        for claim_id, claim in (family.get("claims") or {}).items():
            assignments[str(claim_id)] = {
                "family_id": str(family_id),
                "claim": claim,
            }

    migration_receipt = dict(current.get("migration_receipt") or {})
    history = list(current.get("membership_history") or [])
    responses = list(current.get("felt_review_responses") or [])
    delivered: set[tuple[str, str]] = set()
    for family_id, budget in (
        current.get("felt_review_budget_v1") or {}
    ).items():
        if (
            isinstance(budget, dict)
            and budget.get("deployment_receipt_id")
            and budget.get("delivered_packet_count")
        ):
            delivered.add(
                (
                    str(family_id),
                    str(budget["deployment_receipt_id"]),
                )
            )

    for event in events:
        event_type = event.get("event_type")
        if event_type == "claim_family_created" and isinstance(
            event.get("family"), dict
        ):
            family = event["family"]
            family_records[str(family.get("family_id") or "")] = dict(family)
        elif event_type in {
            "claim_family_membership_assigned",
            "claim_family_membership_corrected",
        }:
            claim_id = str(event.get("canonical_claim_id") or "")
            if claim_id:
                assignments[claim_id] = {
                    "family_id": str(event.get("family_id") or ""),
                    "claim": event.get("claim"),
                }
            if event_type == "claim_family_membership_corrected":
                history.append(event)
        elif event_type == "claim_content_changed":
            claim_id = str(event.get("canonical_claim_id") or "")
            if claim_id in assignments:
                assignments[claim_id]["claim"] = event.get("claim")
            history.append(event)
        elif event_type in {
            "claim_family_migration_completed",
            "claim_family_v3_migration_completed",
        }:
            if isinstance(event.get("migration_receipt"), dict):
                migration_receipt = dict(event["migration_receipt"])
        elif event_type == "felt_review_packet_delivered":
            delivered.add(
                (
                    str(event.get("family_id") or ""),
                    str(event.get("deployment_receipt_id") or ""),
                )
            )
        elif event_type == "felt_review_response_recorded":
            responses.append(event)
        elif event_type in {
            "claim_family_merged",
            "claim_family_split",
            "claim_family_delta_projected",
        }:
            history.append(event)

    families: dict[str, dict[str, Any]] = {}
    for claim_id, assignment in sorted(assignments.items()):
        family_id = str(assignment.get("family_id") or "")
        family = families.setdefault(
            family_id,
            {
                **family_records.get(family_id, {}),
                "family_id": family_id,
                "claims": {},
            },
        )
        family["claims"][claim_id] = assignment.get("claim")
    for family in families.values():
        member_ids = sorted(family["claims"])
        family["member_claim_ids"] = member_ids
        family["member_count"] = len(member_ids)

    changed_family_ids = set(migration_receipt.get("family_ids") or [])
    for event in history:
        for key in (
            "family_id",
            "from_family_id",
            "to_family_id",
            "new_family_id",
        ):
            if event.get(key):
                changed_family_ids.add(str(event[key]))
    review_budget: dict[str, dict[str, Any]] = {}
    if deployment_receipt_id:
        for family_id in sorted(changed_family_ids):
            was_delivered = (
                family_id,
                deployment_receipt_id,
            ) in delivered
            review_budget[family_id] = {
                "schema": "felt_review_budget_v1",
                "schema_version": 1,
                "family_id": family_id,
                "deployment_receipt_id": deployment_receipt_id,
                "packet_budget": 1,
                "delivered_packet_count": 1 if was_delivered else 0,
                "packet_available": not was_delivered,
                "individual_cards_queryable": True,
                "duplicate_delivery_held": True,
                "objection_or_still_friction_bypasses_hold": True,
                "silence_classification": "no_response",
            }

    claim_count = len(assignments)
    membership_count = sum(
        len(family["claims"]) for family in families.values()
    )
    return {
        "schema": "claim_family_status_v1",
        "schema_version": 1,
        "matcher_version": MATCHER_VERSION,
        "match_threshold": MATCH_THRESHOLD,
        "claim_count": claim_count,
        "family_count": len(families),
        "singleton_family_count": sum(
            1 for family in families.values() if len(family["claims"]) == 1
        ),
        "counter_audit": {
            "every_claim_assigned_once": membership_count == claim_count,
            "membership_count_equals_claim_count": (
                membership_count == claim_count
            ),
            "unassigned_claim_count": 0,
        },
        "families": families,
        "migration_receipt": migration_receipt,
        "membership_history": history,
        "felt_review_budget_v1": review_budget,
        "felt_review_responses": responses,
        "closure_propagated_by_family": False,
        "evidence_sufficiency_propagated_by_family": False,
        "supersession_propagated_by_family": False,
        "authority_propagated_by_family": False,
        "artifact_authority_state_v1": _authority(),
    }
