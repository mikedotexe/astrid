"""Route exact and temporal deployment receipts into felt contracts."""

from __future__ import annotations

from collections import defaultdict
import hashlib
from pathlib import Path
from typing import Any, Iterable

try:
    from environment_receipts import read_receipts
    from evidence_store.model import EvidenceEventV2, sha256_canonical
except ModuleNotFoundError:
    from scripts.environment_receipts import read_receipts
    from scripts.evidence_store.model import EvidenceEventV2, sha256_canonical

from .identity import edge_id, node_id
from .model import build_edge, build_node


def route_deployments(
    *,
    workspace: Path,
    source_by_stream: dict[str, list[EvidenceEventV2]],
    existing_graph_envelopes: Iterable[EvidenceEventV2],
    existing_implementation_nodes: dict[tuple[str, str], str] | None,
    membership: dict[str, str],
    claim_sources: dict[str, Any],
    claim_nodes: dict[str, str],
    work_contracts: dict[str, str],
    latest_node_by_work: dict[str, str],
    contracts: dict[str, list[Any]],
    contract_created_at: dict[str, str],
    events: list[dict[str, Any]],
) -> tuple[str, int, int, int]:
    from .sources import _change_refs, _event, _unix_iso

    environment_receipts_path = (
        workspace / "environment_receipts/environment_receipts.jsonl"
    )
    environment_receipts_sha256 = (
        hashlib.sha256(environment_receipts_path.read_bytes()).hexdigest()
        if environment_receipts_path.is_file()
        else hashlib.sha256(b"").hexdigest()
    )
    receipts_by_id = {
        str(receipt.get("id") or ""): receipt
        for receipt in read_receipts(workspace)
        if receipt.get("id")
    }
    legacy_deployment_bindings: dict[str, set[str]] = defaultdict(set)
    for envelope in source_by_stream["claim_families"]:
        payload = envelope.payload
        if payload.get("event_type") not in {
            "felt_review_packet_delivered",
            "felt_review_response_recorded",
        }:
            continue
        deployment_id = str(payload.get("deployment_receipt_id") or "")
        family_id = str(payload.get("family_id") or "")
        if not deployment_id or not family_id:
            continue
        legacy_deployment_bindings[deployment_id].update(
            contract_id
            for claim_id, contract_id in membership.items()
            if claim_sources[claim_id].family_id == family_id
        )

    if existing_implementation_nodes is None:
        existing_implementation_nodes = {}
        for envelope in existing_graph_envelopes:
            payload = envelope.payload
            if payload.get("event_type") != "felt_contract_node_recorded":
                continue
            node = payload.get("node")
            metadata = node.get("metadata") if isinstance(node, dict) else None
            if (
                isinstance(node, dict)
                and isinstance(metadata, dict)
                and node.get("kind") == "implementation"
                and metadata.get("implementation_receipt_id")
            ):
                existing_implementation_nodes[
                    (
                        str(metadata["implementation_receipt_id"]),
                        str(node.get("contract_id") or ""),
                    )
                ] = str(node.get("node_id") or "")

    deployment_node_count = 0
    temporal_deployment_count = 0
    for deployment_id, receipt in sorted(receipts_by_id.items()):
        deployment = (
            receipt.get("deployment")
            if isinstance(receipt.get("deployment"), dict)
            else {}
        )
        status = str(deployment.get("status") or "observed")
        refs = _change_refs(receipt)
        exact_contracts = set(refs.get("felt_contract", set()))
        exact_contracts.update(
            membership[claim_id]
            for claim_id in refs.get("claim", set())
            if claim_id in membership
        )
        exact_contracts.update(
            work_contracts[work_id]
            for work_id in refs.get("work_item", set())
            if work_id in work_contracts
        )
        associations = {
            contract_id: True
            for contract_id in exact_contracts
            if contract_id in contracts
        }
        for contract_id in legacy_deployment_bindings.get(deployment_id, set()):
            associations.setdefault(contract_id, False)
        for contract_id, exact in sorted(associations.items()):
            parent = next(
                (
                    existing_implementation_nodes[(implementation_id, contract_id)]
                    for implementation_id in sorted(
                        refs.get("implementation_receipt", set())
                    )
                    if (implementation_id, contract_id)
                    in existing_implementation_nodes
                ),
                None,
            )
            if parent is None:
                exact_work_parents = [
                    latest_node_by_work[work_id]
                    for work_id in sorted(refs.get("work_item", set()))
                    if work_contracts.get(work_id) == contract_id
                    and work_id in latest_node_by_work
                ]
                exact_claim_parents = [
                    claim_nodes[claim_id]
                    for claim_id in sorted(refs.get("claim", set()))
                    if membership.get(claim_id) == contract_id
                    and claim_id in claim_nodes
                ]
                parent = next(iter(exact_work_parents or exact_claim_parents), None)
            claim_level_exact = parent is not None
            if parent is None:
                parent = next(
                    (
                        claim_nodes[claim_id]
                        for claim_id, assigned in sorted(membership.items())
                        if assigned == contract_id and claim_id in claim_nodes
                    ),
                    None,
                )
            if parent is None:
                continue
            occurred_at = str(
                receipt.get("iso_time")
                or _unix_iso(
                    (receipt.get("t_ms") or 0) / 1000
                    if isinstance(receipt.get("t_ms"), (int, float))
                    else None,
                    contract_created_at[contract_id],
                )
            )
            kind = "deployment" if status == "passed" else "deployment_attempt"
            child = node_id(
                deployment_id,
                f"{kind}:{'exact' if exact else 'temporal'}",
                contract_id,
            )
            node = build_node(
                node_id=child,
                contract_id=contract_id,
                kind=kind,
                source_event_id=deployment_id,
                occurred_at=occurred_at,
                source_ref=None,
                metadata={
                    "deployment_receipt_id": deployment_id,
                    "deployment_status": status,
                    "exact_lineage": exact,
                    "temporal_association": not exact,
                    "temporal_association_basis": (
                        None if exact else "legacy_review_packet_binding"
                    ),
                    "receipt_sha256": sha256_canonical(receipt),
                    "process_identity_sha256": sha256_canonical(
                        receipt.get("processes") or {}
                    ),
                    "compatibility_status_sha256": sha256_canonical(
                        receipt.get("compatibility_status") or {}
                    ),
                    "technical_disposition": (
                        "deployed"
                        if exact and claim_level_exact and status == "passed"
                        else "unassessed"
                    ),
                    "private_content_copied": False,
                },
                authority_state="evidence_only",
            ).to_dict()
            relation = (
                "deployed_by"
                if exact and status == "passed"
                else "deployment_attempted"
                if exact
                else "temporally_associated_deployment"
            )
            edge = build_edge(
                edge_id=edge_id(parent, child, relation, deployment_id),
                contract_id=contract_id,
                source_node_id=parent,
                target_node_id=child,
                relation=relation,
                source_event_id=deployment_id,
                occurred_at=occurred_at,
                causal_parent=exact,
            ).to_dict()
            events.append(
                _event(
                    "felt_contract_node_recorded",
                    "felt_contract",
                    contract_id,
                    (
                        f"felt_contract_deployment:{deployment_id}:{contract_id}:"
                        f"{'exact' if exact else 'temporal'}"
                    ),
                    contract_id=contract_id,
                    node=node,
                    edges=[edge],
                    source_event_ref={
                        "event_id": deployment_id,
                        "stream": "environment_receipts",
                        "receipt_sha256": sha256_canonical(receipt),
                    },
                )
            )
            deployment_node_count += 1
            temporal_deployment_count += not exact

    return (
        environment_receipts_sha256,
        len(receipts_by_id),
        deployment_node_count,
        temporal_deployment_count,
    )
