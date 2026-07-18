#!/usr/bin/env python3
"""Build deterministic living claim families without an external model."""

from __future__ import annotations

import argparse
from collections import Counter
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from typing import Any

from claim_family_matcher import (
    MATCHER_VERSION,
    MATCH_THRESHOLD,
    SUGGESTION_THRESHOLD,
    authority_class,
    polarity,
    requested_outcome,
    weighted_similarity,
)
from claim_family_deployment import latest_successful_deployment

try:
    from evidence_store import EvidenceEventStore, EvidenceStoreError
    from evidence_store.adapter import append_domain_events, read_domain_events
    from evidence_store.model import canonical_json
except ModuleNotFoundError:
    from scripts.evidence_store import EvidenceEventStore, EvidenceStoreError
    from scripts.evidence_store.adapter import append_domain_events, read_domain_events
    from scripts.evidence_store.model import canonical_json

DEFAULT_WORKSPACE = Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace")
PROJECTOR_VERSION = 1


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/claim_families_v1"

def addressing_status_path(workspace: Path) -> Path:
    return workspace / "diagnostics/introspection_addressing_v1/status.json"

def authority_state(state: str = "evidence_only") -> dict[str, Any]:
    if state not in {"evidence_only", "approval_pending"}:
        raise ValueError(f"unsupported artifact authority state: {state}")
    return {
        "schema": "artifact_authority_state_v1",
        "schema_version": 1,
        "state": state,
        "live_eligible_now": False,
        "auto_approved": False,
        "grants_approval": False,
        "edits_source_now": False,
    }

def digest(value: Any) -> str:
    return hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest()

@dataclass(frozen=True)
class ClaimRecord:
    canonical_claim_id: str
    introspection_id: str
    claim_id: str
    summary: str
    authority_class: str
    target_surface: str
    requested_outcome: str
    polarity: str
    record: dict[str, Any]

    @property
    def match_key(self) -> tuple[str, str, str, str]:
        return (
            self.authority_class,
            self.target_surface,
            self.requested_outcome,
            self.polarity,
        )

def load_claims(workspace: Path) -> tuple[list[ClaimRecord], str]:
    path = addressing_status_path(workspace)
    raw = path.read_bytes()
    status = json.loads(raw)
    if not isinstance(status, dict):
        raise ValueError("addressing status must be an object")
    queue_positions = {
        str(item.get("introspection_id")): index
        for index, item in enumerate(status.get("next_queue") or [])
        if isinstance(item, dict) and item.get("introspection_id")
    }
    claims: list[ClaimRecord] = []
    artifacts = status.get("artifacts")
    if not isinstance(artifacts, dict):
        raise ValueError("addressing status artifacts must be an object")
    for artifact_key, artifact in sorted(artifacts.items()):
        if not isinstance(artifact, dict) or not isinstance(artifact.get("claims"), dict):
            continue
        introspection_id = str(artifact.get("introspection_id") or artifact_key)
        target = str(artifact.get("source_family") or "unknown_surface")
        for claim_id, original in sorted(artifact["claims"].items()):
            if not isinstance(original, dict):
                continue
            summary = str(original.get("summary") or "").strip()
            if not summary:
                continue
            evidence_links = [
                {
                    "kind": item.get("kind"),
                    "target": item.get("target"),
                    "ts": item.get("ts"),
                }
                for item in (original.get("evidence") or [])
                if isinstance(item, dict)
            ]
            closure_receipts = [
                {
                    "status": item.get("status"),
                    "ts": item.get("ts"),
                }
                for item in (artifact.get("close_events") or [])
                if isinstance(item, dict)
            ]
            canonical_id = f"{introspection_id}:{claim_id}"
            preserved = {
                "canonical_claim_id": canonical_id,
                "introspection_id": introspection_id,
                "claim_id": str(original.get("claim_id") or claim_id),
                "text": summary,
                "disposition": original.get("disposition"),
                "classification": original.get("classification"),
                "authority_record_sha256": digest(original.get("authority")),
                "evidence_links": evidence_links,
                "evidence_links_sha256": digest(original.get("evidence") or []),
                "rationale_sha256": digest(original.get("rationale")),
                "artifact_status": artifact.get("status"),
                "fully_addressed": artifact.get("fully_addressed"),
                "closure_receipts": closure_receipts,
                "closure_events_sha256": digest(artifact.get("close_events") or []),
                "queue_position": queue_positions.get(introspection_id),
                "source_sha256": artifact.get("sha256"),
                "source_path": artifact.get("relative_path"),
                "source_family": target,
                "canonical_claim_record_sha256": digest(original),
                "canonical_claim_source": (
                    "diagnostics/introspection_addressing_v1/status.json"
                ),
            }
            claims.append(
                ClaimRecord(
                    canonical_claim_id=canonical_id,
                    introspection_id=introspection_id,
                    claim_id=preserved["claim_id"],
                    summary=summary,
                    authority_class=authority_class(original, summary),
                    target_surface=target,
                    requested_outcome=requested_outcome(summary),
                    polarity=polarity(summary),
                    record=preserved,
                )
            )
    return claims, hashlib.sha256(raw).hexdigest()

def family_claims(
    claims: list[ClaimRecord],
) -> tuple[list[dict[str, Any]], dict[str, list[dict[str, Any]]]]:
    groups: dict[tuple[str, str, str, str], list[ClaimRecord]] = {}
    for claim in claims:
        groups.setdefault(claim.match_key, []).append(claim)
    families: list[dict[str, Any]] = []
    suggestions: dict[str, list[dict[str, Any]]] = {}
    for key, group in sorted(groups.items()):
        complete_link: list[list[ClaimRecord]] = []
        for claim in sorted(group, key=lambda item: item.canonical_claim_id):
            candidate_indexes: list[tuple[float, int]] = []
            for index, family in enumerate(complete_link):
                scores = [
                    weighted_similarity(claim.summary, member.summary)
                    for member in family
                ]
                if scores and min(scores) >= MATCH_THRESHOLD:
                    candidate_indexes.append((sum(scores) / len(scores), index))
            if candidate_indexes:
                _, selected = max(candidate_indexes, key=lambda item: (item[0], -item[1]))
                complete_link[selected].append(claim)
            else:
                complete_link.append([claim])
        for family in complete_link:
            member_ids = sorted(item.canonical_claim_id for item in family)
            family_id = f"family_{digest([MATCHER_VERSION, key, member_ids])[:20]}"
            pair_scores = [
                weighted_similarity(left.summary, right.summary)
                for index, left in enumerate(family)
                for right in family[index + 1 :]
            ]
            families.append(
                {
                    "schema": "claim_family_v1",
                    "schema_version": 1,
                    "family_id": family_id,
                    "matcher_version": MATCHER_VERSION,
                    "authority_class": key[0],
                    "target_surface": key[1],
                    "requested_outcome": key[2],
                    "polarity": key[3],
                    "member_claim_ids": member_ids,
                    "member_count": len(member_ids),
                    "minimum_pair_similarity": min(pair_scores) if pair_scores else None,
                    "membership_propagates_closure": False,
                    "membership_propagates_evidence_sufficiency": False,
                    "membership_propagates_supersession": False,
                    "membership_propagates_authority": False,
                    "artifact_authority_state_v1": authority_state(
                        "approval_pending"
                        if key[0].startswith("approval_pending")
                        else "evidence_only"
                    ),
                }
            )
    by_id = {claim.canonical_claim_id: claim for claim in claims}
    family_by_claim = {
        member: family
        for family in families
        for member in family["member_claim_ids"]
    }
    for claim in claims:
        own_family = family_by_claim[claim.canonical_claim_id]
        if own_family["member_count"] != 1:
            continue
        candidates = []
        for other in claims:
            if other.canonical_claim_id == claim.canonical_claim_id:
                continue
            if other.match_key != claim.match_key:
                continue
            score = weighted_similarity(claim.summary, other.summary)
            if SUGGESTION_THRESHOLD <= score < MATCH_THRESHOLD:
                candidates.append(
                    {
                        "claim_id": other.canonical_claim_id,
                        "family_id": family_by_claim[other.canonical_claim_id]["family_id"],
                        "similarity": round(score, 6),
                        "canonical": False,
                    }
                )
        if candidates:
            suggestions[claim.canonical_claim_id] = sorted(
                candidates, key=lambda item: (-item["similarity"], item["claim_id"])
            )[:3]
    assert set(family_by_claim) == set(by_id)
    return sorted(families, key=lambda item: item["family_id"]), suggestions

def migration_events(
    claims: list[ClaimRecord],
    families: list[dict[str, Any]],
    suggestions: dict[str, list[dict[str, Any]]],
    source_sha256: str,
) -> list[dict[str, Any]]:
    family_by_claim = {
        claim_id: family
        for family in families
        for claim_id in family["member_claim_ids"]
    }
    events: list[dict[str, Any]] = []
    for family in families:
        events.append(
            {
                "schema": "claim_family_domain_event_v1",
                "schema_version": 1,
                "event_type": "claim_family_created",
                "aggregate_type": "claim_family",
                "aggregate_id": family["family_id"],
                "family": family,
                "idempotency_key": (
                    f"claim_family_created:{family['family_id']}:{source_sha256}"
                ),
                "artifact_authority_state_v1": family[
                    "artifact_authority_state_v1"
                ],
            }
        )
    for claim in sorted(claims, key=lambda item: item.canonical_claim_id):
        family = family_by_claim[claim.canonical_claim_id]
        events.append(
            {
                "schema": "claim_family_domain_event_v1",
                "schema_version": 1,
                "event_type": "claim_family_membership_assigned",
                "aggregate_type": "claim_family",
                "aggregate_id": family["family_id"],
                "family_id": family["family_id"],
                "canonical_claim_id": claim.canonical_claim_id,
                "claim": claim.record,
                "authority_class": claim.authority_class,
                "target_surface": claim.target_surface,
                "requested_outcome": claim.requested_outcome,
                "polarity": claim.polarity,
                "noncanonical_suggestions": suggestions.get(
                    claim.canonical_claim_id, []
                ),
                "closure_propagated": False,
                "evidence_sufficiency_propagated": False,
                "supersession_propagated": False,
                "authority_propagated": False,
                "idempotency_key": (
                    f"claim_membership:{claim.canonical_claim_id}:"
                    f"{family['family_id']}:{source_sha256}"
                ),
                "artifact_authority_state_v1": family[
                    "artifact_authority_state_v1"
                ],
            }
        )
    counts = Counter(family["member_count"] for family in families)
    receipt = {
        "schema": "claim_family_migration_receipt_v1",
        "schema_version": 1,
        "matcher_version": MATCHER_VERSION,
        "match_threshold": MATCH_THRESHOLD,
        "source_status_sha256": source_sha256,
        "claim_count": len(claims),
        "family_count": len(families),
        "singleton_family_count": counts[1],
        "multi_claim_family_count": len(families) - counts[1],
        "family_ids": [family["family_id"] for family in families],
        "counter_audit": {
            "every_claim_assigned_once": (
                sum(family["member_count"] for family in families) == len(claims)
            ),
            "claim_ids_unique": (
                len({claim.canonical_claim_id for claim in claims}) == len(claims)
            ),
            "family_members_unique": (
                len(
                    {
                        member
                        for family in families
                        for member in family["member_claim_ids"]
                    }
                )
                == len(claims)
            ),
        },
        "artifact_authority_state_v1": authority_state(),
    }
    events.append(
        {
            "schema": "claim_family_domain_event_v1",
            "schema_version": 1,
            "event_type": "claim_family_migration_completed",
            "aggregate_type": "claim_family_migration",
            "aggregate_id": source_sha256[:20],
            "migration_receipt": receipt,
            "idempotency_key": f"claim_family_migration:{source_sha256}",
            "artifact_authority_state_v1": authority_state(),
        }
    )
    return events

def project(
    events: list[dict[str, Any]],
    deployment_receipt_id: str | None,
) -> dict[str, Any]:
    family_records: dict[str, dict[str, Any]] = {}
    assignments: dict[str, dict[str, Any]] = {}
    migration_receipt: dict[str, Any] = {}
    delivered: set[tuple[str, str]] = set()
    responses: list[dict[str, Any]] = []
    history: list[dict[str, Any]] = []
    for event in events:
        event_type = event.get("event_type")
        if event_type == "claim_family_created" and isinstance(event.get("family"), dict):
            family_records[str(event["family"]["family_id"])] = event["family"]
        elif event_type in {
            "claim_family_membership_assigned",
            "claim_family_membership_corrected",
        }:
            claim_id = str(event.get("canonical_claim_id") or "")
            if claim_id:
                assignments[claim_id] = event
            if event_type == "claim_family_membership_corrected":
                history.append(event)
        elif event_type == "claim_family_migration_completed":
            if isinstance(event.get("migration_receipt"), dict):
                migration_receipt = event["migration_receipt"]
        elif event_type == "felt_review_packet_delivered":
            delivered.add(
                (
                    str(event.get("family_id") or ""),
                    str(event.get("deployment_receipt_id") or ""),
                )
            )
        elif event_type == "felt_review_response_recorded":
            responses.append(event)
        elif event_type in {"claim_family_merged", "claim_family_split"}:
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
    changed_family_ids = set(migration_receipt.get("family_ids") or [])
    for event in history:
        for key in ("family_id", "from_family_id", "to_family_id", "new_family_id"):
            if event.get(key):
                changed_family_ids.add(str(event[key]))
    review_budget = {}
    if deployment_receipt_id:
        for family_id in sorted(changed_family_ids):
            review_budget[family_id] = {
                "schema": "felt_review_budget_v1",
                "schema_version": 1,
                "family_id": family_id,
                "deployment_receipt_id": deployment_receipt_id,
                "packet_budget": 1,
                "delivered_packet_count": (
                    1 if (family_id, deployment_receipt_id) in delivered else 0
                ),
                "packet_available": (
                    (family_id, deployment_receipt_id) not in delivered
                ),
                "individual_cards_queryable": True,
                "duplicate_delivery_held": True,
                "objection_or_still_friction_bypasses_hold": True,
                "silence_classification": "no_response",
            }
    claim_count = len(assignments)
    membership_count = sum(len(family["claims"]) for family in families.values())
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
            "membership_count_equals_claim_count": membership_count == claim_count,
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
        "artifact_authority_state_v1": authority_state(),
    }

def render_report(status: dict[str, Any]) -> str:
    audit = status["counter_audit"]
    return "\n".join(
        [
            "# Living Claim Families",
            "",
            f"- Matcher: {status['matcher_version']}",
            f"- Threshold: {status['match_threshold']}",
            f"- Claims: {status['claim_count']}",
            f"- Families: {status['family_count']}",
            f"- Singleton families: {status['singleton_family_count']}",
            f"- Every claim assigned once: {audit['every_claim_assigned_once']}",
            "- Closure/evidence/supersession/authority propagation: disabled",
            "- External model dependency: none",
            "- Authority: evidence only or approval pending, never live",
            "",
        ]
    )

def atomic_write_text(path: Path, payload: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    handle = tempfile.NamedTemporaryFile(
        "w",
        encoding="utf-8",
        dir=path.parent,
        prefix=f".{path.name}.",
        delete=False,
    )
    temporary = Path(handle.name)
    try:
        with handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        directory_fd = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    finally:
        temporary.unlink(missing_ok=True)

def write_projection(workspace: Path, status: dict[str, Any]) -> dict[str, str]:
    root = state_dir(workspace)
    root.mkdir(parents=True, exist_ok=True)
    outputs = {
        "status.json": json.dumps(status, indent=2, sort_keys=True) + "\n",
        "report.md": render_report(status),
        "migration_receipt.json": (
            json.dumps(status["migration_receipt"], indent=2, sort_keys=True) + "\n"
        ),
    }
    hashes = {}
    for name, payload in outputs.items():
        atomic_write_text(root / name, payload)
        hashes[name] = hashlib.sha256(payload.encode()).hexdigest()
    return hashes

def generate(workspace: Path, *, write: bool) -> dict[str, Any]:
    claims, source_sha256 = load_claims(workspace)
    families, suggestions = family_claims(claims)
    generated = migration_events(claims, families, suggestions, source_sha256)
    directory = state_dir(workspace)
    if write:
        append_domain_events(directory, "claim_families", generated)
        events, corrupt = read_domain_events(directory, "claim_families")
        if corrupt:
            raise EvidenceStoreError(f"claim family stream has {corrupt} corrupt events")
    else:
        events = generated
    status = project(events, latest_successful_deployment(workspace))
    status["source_status_sha256"] = source_sha256
    status["generated_event_count"] = len(generated)
    if write:
        hashes = write_projection(workspace, status)
        EvidenceEventStore(
            workspace / "diagnostics/evidence_event_store_v2"
        ).write_checkpoint("claim_families_v1", PROJECTOR_VERSION, hashes)
        status["projection_hashes"] = hashes
    return status

def append_review_event(
    workspace: Path,
    *,
    family_id: str,
    deployment_receipt_id: str,
    event_type: str,
    classification: str | None = None,
) -> dict[str, Any]:
    events, corrupt = read_domain_events(state_dir(workspace), "claim_families")
    if corrupt:
        raise EvidenceStoreError("claim family stream is corrupt")
    status = project(events, latest_successful_deployment(workspace))
    if family_id not in status["families"]:
        raise ValueError(f"unknown family: {family_id}")
    if event_type == "felt_review_packet_delivered":
        budget = status["felt_review_budget_v1"].get(family_id)
        if not budget or budget["deployment_receipt_id"] != deployment_receipt_id:
            raise ValueError("family has no budget under this deployment receipt")
        if not budget["packet_available"]:
            raise ValueError("felt-review packet budget already consumed")
    if event_type == "felt_review_response_recorded" and classification not in {
        "objection",
        "still_friction",
        "contradicted",
        "resolved",
        "no_response",
    }:
        raise ValueError("unsupported felt-review classification")
    event = {
        "schema": "claim_family_domain_event_v1",
        "schema_version": 1,
        "event_type": event_type,
        "aggregate_type": "claim_family",
        "aggregate_id": family_id,
        "family_id": family_id,
        "deployment_receipt_id": deployment_receipt_id,
        "right_to_ignore": True,
        "classification": classification,
        "immediate_surface": classification
        in {"objection", "still_friction", "contradicted"},
        "idempotency_key": (
            f"{event_type}:{family_id}:{deployment_receipt_id}:"
            f"{classification or 'packet'}"
        ),
        "artifact_authority_state_v1": authority_state(),
    }
    append_domain_events(state_dir(workspace), "claim_families", [event])
    return event

def append_membership_history(
    workspace: Path,
    *,
    action: str,
    source_family_id: str,
    target_family_id: str | None,
    claim_ids: list[str],
    reason: str,
) -> list[dict[str, Any]]:
    if not reason.strip():
        raise ValueError("family history reason must not be empty")
    events, corrupt = read_domain_events(state_dir(workspace), "claim_families")
    if corrupt:
        raise EvidenceStoreError("claim family stream is corrupt")
    status = project(events, latest_successful_deployment(workspace))
    families = status["families"]
    source = families.get(source_family_id)
    if not isinstance(source, dict):
        raise ValueError(f"unknown source family: {source_family_id}")
    source_claims = source.get("claims")
    if not isinstance(source_claims, dict):
        raise ValueError("source family has no claims")
    history_events: list[dict[str, Any]] = []
    if action == "correct":
        if len(claim_ids) != 1 or not target_family_id:
            raise ValueError("correction requires one claim and a target family")
        target = families.get(target_family_id)
        if not isinstance(target, dict):
            raise ValueError(f"unknown target family: {target_family_id}")
        criteria = ("authority_class", "target_surface", "requested_outcome", "polarity")
        if any(source.get(key) != target.get(key) for key in criteria):
            raise ValueError(
                "corrected membership must preserve all canonical match classes"
            )
        selected = claim_ids
    elif action == "merge":
        if not target_family_id or target_family_id == source_family_id:
            raise ValueError("merge requires a different target family")
        target = families.get(target_family_id)
        if not isinstance(target, dict):
            raise ValueError(f"unknown target family: {target_family_id}")
        criteria = ("authority_class", "target_surface", "requested_outcome", "polarity")
        if any(source.get(key) != target.get(key) for key in criteria):
            raise ValueError("merge families must agree on all canonical match classes")
        selected = sorted(source_claims)
        history_events.append(
            {
                "schema": "claim_family_domain_event_v1",
                "schema_version": 1,
                "event_type": "claim_family_merged",
                "aggregate_type": "claim_family",
                "aggregate_id": target_family_id,
                "from_family_id": source_family_id,
                "to_family_id": target_family_id,
                "claim_ids": selected,
                "reason": reason.strip(),
                "idempotency_key": (
                    f"claim_family_merge:{source_family_id}:{target_family_id}:"
                    f"{digest([selected, reason])}"
                ),
                "artifact_authority_state_v1": authority_state(),
            }
        )
    elif action == "split":
        selected = sorted(set(claim_ids))
        if not selected or len(selected) >= len(source_claims):
            raise ValueError("split must select a non-empty proper subset of the family")
        target_family_id = (
            f"family_{digest(['manual_split_v1', source_family_id, selected, reason])[:20]}"
        )
        family_record = {
            key: value
            for key, value in source.items()
            if key not in {"claims", "family_id", "member_claim_ids", "member_count"}
        }
        family_record.update(
            {
                "schema": "claim_family_v1",
                "schema_version": 1,
                "family_id": target_family_id,
                "matcher_version": f"{MATCHER_VERSION}+manual_split",
                "member_claim_ids": selected,
                "member_count": len(selected),
                "artifact_authority_state_v1": authority_state(),
            }
        )
        history_events.extend(
            [
                {
                    "schema": "claim_family_domain_event_v1",
                    "schema_version": 1,
                    "event_type": "claim_family_created",
                    "aggregate_type": "claim_family",
                    "aggregate_id": target_family_id,
                    "family": family_record,
                    "idempotency_key": (
                        f"claim_family_created:{target_family_id}:manual_split"
                    ),
                    "artifact_authority_state_v1": authority_state(),
                },
                {
                    "schema": "claim_family_domain_event_v1",
                    "schema_version": 1,
                    "event_type": "claim_family_split",
                    "aggregate_type": "claim_family",
                    "aggregate_id": source_family_id,
                    "family_id": source_family_id,
                    "new_family_id": target_family_id,
                    "claim_ids": selected,
                    "reason": reason.strip(),
                    "idempotency_key": (
                        f"claim_family_split:{source_family_id}:{target_family_id}"
                    ),
                    "artifact_authority_state_v1": authority_state(),
                },
            ]
        )
    else:
        raise ValueError(f"unsupported family history action: {action}")
    assert target_family_id is not None
    for claim_id in selected:
        claim = source_claims.get(claim_id)
        if not isinstance(claim, dict):
            raise ValueError(f"claim {claim_id} is not in {source_family_id}")
        history_events.append(
            {
                "schema": "claim_family_domain_event_v1",
                "schema_version": 1,
                "event_type": "claim_family_membership_corrected",
                "aggregate_type": "claim_family",
                "aggregate_id": target_family_id,
                "canonical_claim_id": claim_id,
                "family_id": target_family_id,
                "from_family_id": source_family_id,
                "to_family_id": target_family_id,
                "claim": claim,
                "reason": reason.strip(),
                "closure_propagated": False,
                "evidence_sufficiency_propagated": False,
                "supersession_propagated": False,
                "authority_propagated": False,
                "idempotency_key": (
                    f"claim_family_correction:{claim_id}:{target_family_id}:"
                    f"{digest(reason)}"
                ),
                "artifact_authority_state_v1": authority_state(),
            }
        )
    append_domain_events(state_dir(workspace), "claim_families", history_events)
    return history_events


class ClaimFamilyTests(unittest.TestCase):
    def claim(self, claim_id: str, summary: str, target: str = "astrid_codec") -> ClaimRecord:
        return ClaimRecord(
            canonical_claim_id=claim_id,
            introspection_id=claim_id.split(":")[0],
            claim_id=claim_id.split(":")[-1],
            summary=summary,
            authority_class="evidence_only_non_live",
            target_surface=target,
            requested_outcome=requested_outcome(summary),
            polarity=polarity(summary),
            record={"canonical_claim_id": claim_id, "text": summary},
        )

    def test_strict_match_joins_duplicates_and_keeps_ambiguous_singleton(self) -> None:
        claims = [
            self.claim("a:c001", "Preserve exact sensory JSON compatibility."),
            self.claim("b:c001", "Preserve exact sensory JSON compatibility."),
            self.claim("c:c001", "Preserve sensory compatibility where practical."),
        ]
        families, suggestions = family_claims(claims)
        sizes = sorted(family["member_count"] for family in families)
        self.assertEqual(sizes, [1, 2])
        assigned = [
            member for family in families for member in family["member_claim_ids"]
        ]
        self.assertEqual(sorted(assigned), sorted(claim.canonical_claim_id for claim in claims))
        self.assertTrue(
            all(
                family["membership_propagates_closure"] is False
                for family in families
            )
        )
        self.assertIsInstance(suggestions, dict)

    def test_authority_or_target_mismatch_never_auto_joins(self) -> None:
        left = self.claim("a:c001", "Preserve exact sensory JSON compatibility.")
        right = self.claim(
            "b:c001",
            "Preserve exact sensory JSON compatibility.",
            target="minime_regulator",
        )
        families, _ = family_claims([left, right])
        self.assertEqual(len(families), 2)

    def test_review_budget_allows_one_packet_and_immediate_objection(self) -> None:
        events = [
            {
                "event_type": "claim_family_created",
                "family": {"family_id": "family_one"},
            },
            {
                "event_type": "claim_family_membership_assigned",
                "family_id": "family_one",
                "canonical_claim_id": "a:c001",
                "claim": {"text": "claim"},
            },
            {
                "event_type": "claim_family_migration_completed",
                "migration_receipt": {"family_ids": ["family_one"]},
            },
            {
                "event_type": "felt_review_response_recorded",
                "family_id": "family_one",
                "classification": "objection",
                "immediate_surface": True,
            },
        ]
        status = project(events, "receipt_one")
        self.assertTrue(
            status["felt_review_budget_v1"]["family_one"]["packet_available"]
        )
        self.assertTrue(status["felt_review_responses"][0]["immediate_surface"])

    def test_membership_correction_replays_without_propagating_closure(self) -> None:
        events = [
            {
                "event_type": "claim_family_created",
                "family": {"family_id": "family_one"},
            },
            {
                "event_type": "claim_family_created",
                "family": {"family_id": "family_two"},
            },
            {
                "event_type": "claim_family_membership_assigned",
                "family_id": "family_one",
                "canonical_claim_id": "a:c001",
                "claim": {"text": "claim"},
            },
            {
                "event_type": "claim_family_membership_corrected",
                "family_id": "family_two",
                "from_family_id": "family_one",
                "to_family_id": "family_two",
                "canonical_claim_id": "a:c001",
                "claim": {"text": "claim"},
                "closure_propagated": False,
            },
        ]
        status = project(events, None)
        self.assertNotIn("a:c001", status["families"].get("family_one", {}).get("claims", {}))
        self.assertIn("a:c001", status["families"]["family_two"]["claims"])
        self.assertFalse(status["membership_history"][0]["closure_propagated"])

    def test_realistic_status_preserves_claim_identity_and_queue_position(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            path = addressing_status_path(workspace)
            path.parent.mkdir(parents=True)
            path.write_text(
                json.dumps(
                    {
                        "next_queue": [{"introspection_id": "intro_one"}],
                        "artifacts": {
                            "intro_one": {
                                "introspection_id": "intro_one",
                                "source_family": "astrid_codec",
                                "status": "read",
                                "fully_addressed": False,
                                "sha256": "source",
                                "claims": {
                                    "c001": {
                                        "claim_id": "c001",
                                        "summary": "Preserve exact output.",
                                        "disposition": "verified",
                                        "evidence": [{"target": "test"}],
                                    }
                                },
                            }
                        },
                    }
                ),
                encoding="utf-8",
            )
            claims, _ = load_claims(workspace)
            self.assertEqual(claims[0].canonical_claim_id, "intro_one:c001")
            self.assertEqual(claims[0].record["claim_id"], "c001")
            self.assertEqual(claims[0].record["queue_position"], 0)
            self.assertEqual(
                claims[0].record["evidence_links"],
                [{"kind": None, "target": "test", "ts": None}],
            )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    commands = parser.add_subparsers(dest="command")
    generate_parser = commands.add_parser("generate")
    generate_parser.add_argument("--write", action="store_true")
    commands.add_parser("report")
    deliver = commands.add_parser("deliver-review")
    deliver.add_argument("--family-id", required=True)
    deliver.add_argument("--deployment-receipt-id", required=True)
    deliver.add_argument("--write", action="store_true")
    response = commands.add_parser("record-response")
    response.add_argument("--family-id", required=True)
    response.add_argument("--deployment-receipt-id", required=True)
    response.add_argument("--classification", required=True)
    response.add_argument("--write", action="store_true")
    correct = commands.add_parser("correct-membership")
    correct.add_argument("--source-family-id", required=True)
    correct.add_argument("--target-family-id", required=True)
    correct.add_argument("--claim-id", required=True)
    correct.add_argument("--reason", required=True)
    correct.add_argument("--write", action="store_true")
    merge = commands.add_parser("merge-families")
    merge.add_argument("--source-family-id", required=True)
    merge.add_argument("--target-family-id", required=True)
    merge.add_argument("--reason", required=True)
    merge.add_argument("--write", action="store_true")
    split = commands.add_parser("split-family")
    split.add_argument("--source-family-id", required=True)
    split.add_argument("--claim-id", action="append", required=True)
    split.add_argument("--reason", required=True)
    split.add_argument("--write", action="store_true")
    return parser


def cli_summary(status: dict[str, Any]) -> dict[str, Any]:
    summary = {
        key: value
        for key, value in status.items()
        if key
        not in {
            "families",
            "felt_review_budget_v1",
            "felt_review_responses",
            "membership_history",
        }
    }
    migration = summary.get("migration_receipt")
    if isinstance(migration, dict):
        summary["migration_receipt"] = {
            key: value for key, value in migration.items() if key != "family_ids"
        }
    return summary


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(ClaimFamilyTests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    workspace = args.workspace.resolve()
    try:
        if args.command in {"generate", "report"}:
            status = generate(
                workspace,
                write=args.command == "generate" and bool(args.write),
            )
            print(json.dumps(cli_summary(status), indent=2, sort_keys=True))
            return 0
        if args.command in {"deliver-review", "record-response"}:
            if not args.write:
                raise ValueError(f"{args.command} requires --write")
            event = append_review_event(
                workspace,
                family_id=args.family_id,
                deployment_receipt_id=args.deployment_receipt_id,
                event_type=(
                    "felt_review_packet_delivered"
                    if args.command == "deliver-review"
                    else "felt_review_response_recorded"
                ),
                classification=(
                    args.classification
                    if args.command == "record-response"
                    else None
                ),
            )
            print(json.dumps(event, indent=2, sort_keys=True))
            return 0
        if args.command in {
            "correct-membership",
            "merge-families",
            "split-family",
        }:
            if not args.write:
                raise ValueError(f"{args.command} requires --write")
            history = append_membership_history(
                workspace,
                action={
                    "correct-membership": "correct",
                    "merge-families": "merge",
                    "split-family": "split",
                }[args.command],
                source_family_id=args.source_family_id,
                target_family_id=getattr(args, "target_family_id", None),
                claim_ids=(
                    [getattr(args, "claim_id")]
                    if isinstance(getattr(args, "claim_id", None), str)
                    else list(getattr(args, "claim_id", []) or [])
                ),
                reason=args.reason,
            )
            print(json.dumps(history, indent=2, sort_keys=True))
            return 0
    except (EvidenceStoreError, OSError, ValueError) as error:
        print(json.dumps({"status": "failed", "error": str(error)}), file=sys.stderr)
        return 1
    parser.print_help()
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
