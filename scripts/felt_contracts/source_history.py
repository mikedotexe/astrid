"""Route claim-linked V2 history into felt-contract nodes."""

from __future__ import annotations

from pathlib import Path
from typing import Any

try:
    from evidence_store.model import sha256_canonical
except ModuleNotFoundError:
    from scripts.evidence_store.model import sha256_canonical

from .identity import digest, node_id
from .model import (
    FeltReviewOutcomeV1,
    build_intervention_boundary,
    build_node,
)


def route_history(
    *,
    source_by_stream: dict[str, list[Any]],
    membership: dict[str, str],
    claims: list[Any],
    claim_sources: dict[str, Any],
    claim_nodes: dict[str, str],
    signal_nodes: dict[tuple[str, str], str],
    events: list[dict[str, Any]],
    routed_source_ids: set[str],
) -> tuple[dict[str, str], dict[str, str]]:
    from .sources import (
        _collect_exact_refs,
        _edge_record,
        _event_time,
        _hash_text,
        _node_event,
        _repository_ref,
        _review_outcome,
        _technical_disposition,
    )

    work_nodes: dict[str, str] = {}
    work_contracts: dict[str, str] = {}
    work_claims: dict[str, str] = {}
    latest_node_by_work: dict[str, str] = {}
    trial_nodes: dict[str, str] = {}
    trial_contracts: dict[str, str] = {}
    roots = {
        "astrid": Path(__file__).resolve().parents[2],
        "minime": Path(__file__).resolve().parents[3] / "minime",
        "model": Path(__file__).resolve().parents[3] / "neural-triple-reservoir",
    }

    for envelope in source_by_stream["addressing"]:
        payload = envelope.payload
        event_type = str(payload.get("event_type") or "")
        if event_type in {
            "full_read",
            "inventory_run",
            "inventory_artifact",
            "inventory_artifact_absent",
        }:
            continue
        occurred_at = _event_time(envelope)
        if event_type == "work_item_created":
            work = payload.get("work_item")
            if not isinstance(work, dict):
                continue
            claim_id = (
                f"{work.get('source_introspection_id')}:{work.get('claim_id')}"
            )
            contract_id = membership.get(claim_id)
            parent = claim_nodes.get(claim_id)
            work_id = str(
                payload.get("work_item_id") or work.get("work_item_id") or ""
            )
            if not contract_id or not parent or not work_id:
                continue
            tier = int(work.get("agency_tier") or 0)
            authority_state = "approval_pending" if tier >= 4 else "evidence_only"
            boundary = build_intervention_boundary(
                boundary_id=f"boundary_{digest([work_id, 'v1'])[:24]}",
                agency_tier=tier,
                authority_class=str(
                    work.get("agency_tier_label")
                    or ("approval_pending" if tier >= 4 else "evidence_only")
                ).replace(" ", "_"),
                lifecycle_state=str(
                    work.get("status") or "unassessed"
                ).replace(" ", "_"),
                authority_state=authority_state,
            ).to_dict()
            child = node_id(
                envelope.event_id, f"intervention:{work_id}", contract_id
            )
            node = build_node(
                node_id=child,
                contract_id=contract_id,
                kind="intervention",
                source_event_id=envelope.event_id,
                occurred_at=occurred_at,
                source_ref=None,
                metadata={
                    "work_item_id": work_id,
                    "canonical_claim_id": claim_id,
                    "route": str(work.get("route") or "unknown"),
                    "technical_disposition": _technical_disposition(
                        str(work.get("status") or "")
                    ),
                    "boundary": boundary,
                    "claim_summary_sha256": _hash_text(work.get("claim_summary")),
                    "suggested_next_sha256": _hash_text(
                        work.get("suggested_next")
                    ),
                    "private_content_copied": False,
                },
                authority_state=authority_state,
            ).to_dict()
            events.append(
                _node_event(
                    node,
                    [
                        _edge_record(
                            contract_id,
                            parent,
                            child,
                            "proposes",
                            envelope,
                            occurred_at,
                        )
                    ],
                    envelope,
                    authority_state=authority_state,
                )
            )
            work_nodes[work_id] = child
            latest_node_by_work[work_id] = child
            work_contracts[work_id] = contract_id
            work_claims[work_id] = claim_id
            routed_source_ids.add(envelope.event_id)
            continue

        work_id = str(payload.get("work_item_id") or "")
        contract_id = work_contracts.get(work_id)
        parent = latest_node_by_work.get(work_id) or work_nodes.get(work_id)
        if event_type in {
            "work_status_set",
            "work_evidence_linked",
            "agency_tier_requested",
            "agency_tier_corrected",
            "closure_card_emitted",
            "post_change_response_recorded",
        } and contract_id and parent:
            kind = {
                "work_status_set": "disposition",
                "work_evidence_linked": "evidence",
                "agency_tier_requested": "authority_boundary",
                "agency_tier_corrected": "authority_boundary",
                "closure_card_emitted": "review_packet",
                "post_change_response_recorded": "felt_review",
            }[event_type]
            relation = {
                "work_status_set": "changes_disposition",
                "work_evidence_linked": "supported_by",
                "agency_tier_requested": "requires_authority",
                "agency_tier_corrected": "corrects_authority",
                "closure_card_emitted": "delivers_review",
                "post_change_response_recorded": "reviewed_by",
            }[event_type]
            tier = int(payload.get("tier") or 0)
            authority_state = (
                "approval_pending"
                if tier >= 4
                or payload.get("artifact_authority_state_v1", {}).get("state")
                == "approval_pending"
                else "evidence_only"
            )
            metadata: dict[str, Any] = {
                "work_item_id": work_id,
                "canonical_claim_id": work_claims.get(work_id),
                "source_event_type": event_type,
                "private_content_copied": False,
            }
            if event_type == "work_status_set":
                status = str(payload.get("status") or "")
                metadata.update(
                    {
                        "source_status": status,
                        "technical_disposition": _technical_disposition(status),
                        "blocked_by": str(payload.get("blocked_by") or "none"),
                        "note_sha256": _hash_text(payload.get("note")),
                    }
                )
            elif event_type == "work_evidence_linked":
                evidence = payload.get("evidence")
                if isinstance(evidence, dict):
                    metadata.update(
                        {
                            "evidence_kind": str(
                                evidence.get("kind") or "unknown"
                            ),
                            "evidence_ref": _repository_ref(
                                evidence.get("target"), roots
                            ),
                            "evidence_note_sha256": _hash_text(
                                evidence.get("note")
                            ),
                        }
                    )
            elif event_type.startswith("agency_tier"):
                metadata.update(
                    {
                        "agency_tier": tier,
                        "request_status": str(
                            payload.get("request_status")
                            or payload.get("correction_status")
                            or "recorded"
                        ),
                        "reason_sha256": _hash_text(payload.get("reason")),
                    }
                )
            elif event_type == "closure_card_emitted":
                metadata.update(
                    {
                        "card_sha256": sha256_canonical(
                            payload.get("closure_card")
                        ),
                        "card_delivery_is_closure": False,
                    }
                )
            elif event_type == "post_change_response_recorded":
                outcome = _review_outcome(
                    str(payload.get("response_status") or "")
                )
                metadata.update(
                    {
                        "felt_review_outcome": outcome,
                        "source_ref": _repository_ref(
                            payload.get("source"), roots
                        ),
                        "response_note_sha256": _hash_text(
                            payload.get("note")
                        ),
                        "silence_affirms": False,
                        "silence_waives": False,
                        "legacy_no_response_compatibility_only": outcome
                        == FeltReviewOutcomeV1.NO_RESPONSE.value,
                        "reopens_contract": outcome
                        in {
                            FeltReviewOutcomeV1.STILL_FRICTION.value,
                            FeltReviewOutcomeV1.CONTRADICTED.value,
                            FeltReviewOutcomeV1.OBJECTION.value,
                        },
                    }
                )
            child = node_id(envelope.event_id, f"{kind}:{work_id}", contract_id)
            node = build_node(
                node_id=child,
                contract_id=contract_id,
                kind=kind,
                source_event_id=envelope.event_id,
                occurred_at=occurred_at,
                source_ref=None,
                metadata=metadata,
                authority_state=authority_state,
            ).to_dict()
            events.append(
                _node_event(
                    node,
                    [
                        _edge_record(
                            contract_id,
                            parent,
                            child,
                            relation,
                            envelope,
                            occurred_at,
                        )
                    ],
                    envelope,
                    authority_state=authority_state,
                )
            )
            latest_node_by_work[work_id] = child
            routed_source_ids.add(envelope.event_id)
            continue

        if event_type == "evidence_linked":
            claim_id = (
                f"{payload.get('introspection_id')}:{payload.get('claim_id')}"
            )
            contract_id = membership.get(claim_id)
            parent = claim_nodes.get(claim_id)
            evidence = payload.get("evidence")
            if not contract_id or not parent or not isinstance(evidence, dict):
                continue
            child = node_id(
                envelope.event_id, f"evidence:{claim_id}", contract_id
            )
            node = build_node(
                node_id=child,
                contract_id=contract_id,
                kind="evidence",
                source_event_id=envelope.event_id,
                occurred_at=occurred_at,
                source_ref=None,
                metadata={
                    "canonical_claim_id": claim_id,
                    "evidence_kind": str(evidence.get("kind") or "unknown"),
                    "evidence_ref": _repository_ref(
                        evidence.get("target"), roots
                    ),
                    "evidence_note_sha256": _hash_text(evidence.get("note")),
                    "private_content_copied": False,
                },
                authority_state="evidence_only",
            ).to_dict()
            events.append(
                _node_event(
                    node,
                    [
                        _edge_record(
                            contract_id,
                            parent,
                            child,
                            "supported_by",
                            envelope,
                            occurred_at,
                        )
                    ],
                    envelope,
                    authority_state="evidence_only",
                )
            )
            routed_source_ids.add(envelope.event_id)
            continue

        if event_type == "closed":
            introspection_id = str(payload.get("introspection_id") or "")
            contract_ids = sorted(
                {
                    membership[claim.claim_id]
                    for claim in claims
                    if claim.introspection_id == introspection_id
                }
            )
            for contract_id in contract_ids:
                parent = signal_nodes.get((contract_id, introspection_id))
                if not parent:
                    continue
                child = node_id(
                    envelope.event_id,
                    f"compatibility_report_close:{introspection_id}",
                    contract_id,
                )
                node = build_node(
                    node_id=child,
                    contract_id=contract_id,
                    kind="compatibility_report_close",
                    source_event_id=envelope.event_id,
                    occurred_at=occurred_at,
                    source_ref=None,
                    metadata={
                        "report_status": str(
                            payload.get("status") or "unknown"
                        ),
                        "rationale_sha256": _hash_text(
                            payload.get("rationale")
                        ),
                        "closes_claim": False,
                        "closes_contract": False,
                    },
                    authority_state="evidence_only",
                ).to_dict()
                events.append(
                    _node_event(
                        node,
                        [
                            _edge_record(
                                contract_id,
                                parent,
                                child,
                                "compatibility_only",
                                envelope,
                                occurred_at,
                            )
                        ],
                        envelope,
                        authority_state="evidence_only",
                    )
                )
                routed_source_ids.add(envelope.event_id)

    for envelope in source_by_stream["sandbox"]:
        payload = envelope.payload
        event_type = str(payload.get("event_type") or "")
        occurred_at = _event_time(envelope)
        if event_type == "trial_created":
            trial = payload.get("trial")
            if not isinstance(trial, dict):
                continue
            trial_id = str(trial.get("trial_id") or "")
            work_id = str(trial.get("source_work_item_id") or "")
            canonical_claim_id = (
                f"{trial.get('source_introspection_id')}:{trial.get('claim_id')}"
            )
            contract_id = work_contracts.get(work_id) or membership.get(
                canonical_claim_id
            )
            parent = latest_node_by_work.get(work_id) or claim_nodes.get(
                canonical_claim_id
            )
            if not contract_id or not parent or not trial_id:
                continue
            tier = int(trial.get("agency_tier") or 0)
            authority_state = "approval_pending" if tier >= 4 else "evidence_only"
            child = node_id(
                envelope.event_id, f"sandbox_trial:{trial_id}", contract_id
            )
            node = build_node(
                node_id=child,
                contract_id=contract_id,
                kind="sandbox_trial",
                source_event_id=envelope.event_id,
                occurred_at=occurred_at,
                source_ref=None,
                metadata={
                    "trial_id": trial_id,
                    "canonical_claim_id": canonical_claim_id,
                    "adapter": str(trial.get("adapter") or "unknown"),
                    "agency_tier": tier,
                    "status": str(trial.get("status") or "unknown"),
                    "hypothesis_sha256": _hash_text(trial.get("hypothesis")),
                    "proposed_intervention_sha256": _hash_text(
                        trial.get("proposed_intervention")
                    ),
                    "private_content_copied": False,
                },
                authority_state=authority_state,
            ).to_dict()
            events.append(
                _node_event(
                    node,
                    [
                        _edge_record(
                            contract_id,
                            parent,
                            child,
                            "tests_intervention",
                            envelope,
                            occurred_at,
                        )
                    ],
                    envelope,
                    authority_state=authority_state,
                )
            )
            trial_nodes[trial_id] = child
            trial_contracts[trial_id] = contract_id
            routed_source_ids.add(envelope.event_id)
            continue
        trial_id = str(payload.get("trial_id") or "")
        contract_id = trial_contracts.get(trial_id)
        parent = trial_nodes.get(trial_id)
        if not contract_id or not parent:
            continue
        kind = {
            "trial_result_recorded": "evidence",
            "trial_status_set": "disposition",
            "trial_adapter_corrected": "intervention_correction",
            "trial_proposal_card_emitted": "review_packet",
            "trial_result_card_emitted": "review_packet",
        }.get(event_type)
        if kind is None:
            continue
        metadata = {
            "trial_id": trial_id,
            "source_event_type": event_type,
            "source_payload_sha256": sha256_canonical(payload),
            "card_delivery_is_closure": False,
            "private_content_copied": False,
        }
        result = payload.get("result")
        if isinstance(result, dict):
            metadata.update(
                {
                    "classification": str(
                        result.get("classification") or "unknown"
                    ),
                    "result_sha256": str(
                        result.get("result_sha256")
                        or sha256_canonical(result)
                    ),
                    "json_ref": _repository_ref(
                        result.get("json_path"), roots
                    ),
                    "markdown_ref": _repository_ref(
                        result.get("markdown_path"), roots
                    ),
                }
            )
        child = node_id(envelope.event_id, f"{kind}:{trial_id}", contract_id)
        authority_state = str(
            payload.get("artifact_authority_state_v1", {}).get("state")
            or "evidence_only"
        )
        node = build_node(
            node_id=child,
            contract_id=contract_id,
            kind=kind,
            source_event_id=envelope.event_id,
            occurred_at=occurred_at,
            source_ref=None,
            metadata=metadata,
            authority_state=authority_state,
        ).to_dict()
        events.append(
            _node_event(
                node,
                [
                    _edge_record(
                        contract_id,
                        parent,
                        child,
                        "trial_history",
                        envelope,
                        occurred_at,
                    )
                ],
                envelope,
                authority_state=authority_state,
            )
        )
        trial_nodes[trial_id] = child
        routed_source_ids.add(envelope.event_id)

    for stream in (
        "corridor_v1",
        "corridor_v2",
        "signal_spine",
        "claim_families",
    ):
        for envelope in source_by_stream[stream]:
            event_type = str(envelope.payload.get("event_type") or "")
            if stream == "claim_families" and event_type not in {
                "experiment_dossier_projected",
                "experiment_dossier_trial_unrouted",
                "felt_review_packet_delivered",
                "felt_review_response_recorded",
            }:
                continue
            work_refs, claim_refs = _collect_exact_refs(envelope.payload)
            exact_contracts = {
                work_contracts[work_id]
                for work_id in work_refs
                if work_id in work_contracts
            }
            exact_contracts.update(
                membership[claim_id]
                for claim_id in claim_refs
                if claim_id in membership
            )
            if (
                stream == "claim_families"
                and event_type.startswith("felt_review_")
                and not exact_contracts
            ):
                family_id = str(envelope.payload.get("family_id") or "")
                exact_contracts.update(
                    contract_id
                    for claim_id, contract_id in membership.items()
                    if claim_sources[claim_id].family_id == family_id
                )
            if not exact_contracts:
                continue
            occurred_at = _event_time(envelope)
            for contract_id in sorted(exact_contracts):
                parents = sorted(
                    {
                        latest_node_by_work[work_id]
                        for work_id in work_refs
                        if work_contracts.get(work_id) == contract_id
                        and work_id in latest_node_by_work
                    }
                    | {
                        claim_nodes[claim_id]
                        for claim_id in claim_refs
                        if membership.get(claim_id) == contract_id
                        and claim_id in claim_nodes
                    }
                )
                if not parents:
                    parents = sorted(
                        node
                        for claim_id, node in claim_nodes.items()
                        if membership.get(claim_id) == contract_id
                    )[:1]
                if not parents:
                    continue
                kind = (
                    "experiment_dossier"
                    if event_type.startswith("experiment_dossier_")
                    else "felt_review"
                    if event_type.startswith("felt_review_")
                    else "delivery_evidence"
                    if stream == "signal_spine"
                    else "intervention_evidence"
                )
                authority_state = str(
                    envelope.payload.get("artifact_authority_state_v1", {}).get(
                        "state"
                    )
                    or "evidence_only"
                )
                metadata: dict[str, Any] = {
                    "source_stream": stream,
                    "source_event_type": event_type,
                    "source_payload_sha256": sha256_canonical(
                        envelope.payload
                    ),
                    "exact_reference_count": len(work_refs) + len(claim_refs),
                    "private_content_copied": False,
                }
                if event_type == "felt_review_response_recorded":
                    outcome = _review_outcome(
                        str(envelope.payload.get("classification") or "")
                    )
                    metadata.update(
                        {
                            "felt_review_outcome": outcome,
                            "legacy_family_scope": True,
                            "legacy_resolved_is_compatibility_evidence_only": (
                                str(
                                    envelope.payload.get("classification") or ""
                                )
                                == "resolved"
                            ),
                            "reopens_contract": outcome
                            in {
                                FeltReviewOutcomeV1.STILL_FRICTION.value,
                                FeltReviewOutcomeV1.CONTRADICTED.value,
                                FeltReviewOutcomeV1.OBJECTION.value,
                            },
                        }
                    )
                child = node_id(
                    envelope.event_id,
                    f"{kind}:{event_type}",
                    contract_id,
                )
                node = build_node(
                    node_id=child,
                    contract_id=contract_id,
                    kind=kind,
                    source_event_id=envelope.event_id,
                    occurred_at=occurred_at,
                    source_ref=None,
                    metadata=metadata,
                    authority_state=authority_state,
                ).to_dict()
                relation = (
                    "related_to"
                    if stream
                    in {"corridor_v1", "corridor_v2", "signal_spine"}
                    else "reviewed_by"
                )
                events.append(
                    _node_event(
                        node,
                        [
                            _edge_record(
                                contract_id,
                                parent,
                                child,
                                relation,
                                envelope,
                                occurred_at,
                            )
                            for parent in parents
                        ],
                        envelope,
                        authority_state=authority_state,
                    )
                )
                routed_source_ids.add(envelope.event_id)

    return work_contracts, latest_node_by_work
