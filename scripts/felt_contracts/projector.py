"""Deterministic projections for the living felt-contract graph."""

from __future__ import annotations

from collections import Counter, defaultdict
import hashlib
import json
import os
from pathlib import Path
import tempfile
from typing import Any, Iterable

try:
    from authority_state import (
        LEGACY_AUTHORITY_MARKERS,
        PROJECTION_KEY,
        assert_artifact_authority_tree,
    )
    from environment_receipts import read_receipts
    from evidence_store.model import canonical_json, sha256_canonical
except ModuleNotFoundError:
    from scripts.authority_state import (
        LEGACY_AUTHORITY_MARKERS,
        PROJECTION_KEY,
        assert_artifact_authority_tree,
    )
    from scripts.environment_receipts import read_receipts
    from scripts.evidence_store.model import canonical_json, sha256_canonical

from .model import (
    ContractActivityV1,
    EvidenceSufficiencyV1,
    FeltReviewOutcomeV1,
    TechnicalDispositionV1,
)
from .sources import graph_state_dir

PROJECTOR_VERSION = 2
GRAPH_INPUT_STREAMS = (
    "addressing",
    "sandbox",
    "corridor_v1",
    "corridor_v2",
    "signal_spine",
    "lived_state_witness",
    "claim_families",
    "felt_contracts",
)


class GraphProjectionError(ValueError):
    """Raised when graph history cannot be replayed safely."""


def _authority(state: str = "evidence_only") -> dict[str, Any]:
    return {
        "schema": "artifact_authority_state_v1",
        "schema_version": 1,
        "state": state,
        "witness_only": True,
    }


def _verify_record_hash(
    record: dict[str, Any],
    hash_key: str,
    *,
    label: str,
) -> None:
    expected = str(record.get(hash_key) or "")
    derived_keys = {PROJECTION_KEY, *LEGACY_AUTHORITY_MARKERS}

    def semantic_value(value: Any) -> Any:
        if isinstance(value, list):
            return [semantic_value(item) for item in value]
        if not isinstance(value, dict):
            return value
        return {
            key: semantic_value(item)
            for key, item in value.items()
            if key not in derived_keys
        }

    unsigned = semantic_value(
        {key: value for key, value in record.items() if key != hash_key}
    )
    if expected != sha256_canonical(unsigned):
        raise GraphProjectionError(f"{label} hash mismatch")


def _change_ref_contracts(receipt: dict[str, Any]) -> set[str]:
    refs = receipt.get("change_refs")
    if not isinstance(refs, list):
        return set()
    return {
        str(ref.get("id"))
        for ref in refs
        if isinstance(ref, dict)
        and ref.get("kind") == "felt_contract"
        and str(ref.get("id") or "").startswith("contract_")
    }


def latest_successful_deployment(workspace: Path) -> dict[str, Any] | None:
    for receipt in reversed(read_receipts(workspace)):
        deployment = receipt.get("deployment")
        if (
            isinstance(deployment, dict)
            and deployment.get("status") == "passed"
            and receipt.get("id")
        ):
            return receipt
    return None


def _initial_contract_state(contract: dict[str, Any]) -> dict[str, Any]:
    return {
        "contract": contract,
        "claims": set(),
        "node_ids": [],
        "edge_ids": [],
        "node_kind_counts": Counter(),
        "technical_by_claim": {},
        "evidence_by_claim": {},
        "felt_review": FeltReviewOutcomeV1.NOT_REQUESTED.value,
        "activity": ContractActivityV1.OPEN.value,
        "last_change_at": contract.get("created_at"),
        "reopen_count": 0,
        "contradiction_count": 0,
        "administrative_terminal": False,
        "felt_closed": False,
    }


def _apply_review(
    state: dict[str, Any],
    outcome: str,
    *,
    authoritative: bool,
) -> None:
    if outcome not in {item.value for item in FeltReviewOutcomeV1}:
        return
    state["felt_review"] = outcome
    if outcome in {
        FeltReviewOutcomeV1.STILL_FRICTION.value,
        FeltReviewOutcomeV1.CONTRADICTED.value,
        FeltReviewOutcomeV1.OBJECTION.value,
    }:
        state["activity"] = ContractActivityV1.REOPENED.value
        state["reopen_count"] += 1
        if outcome == FeltReviewOutcomeV1.CONTRADICTED.value:
            state["contradiction_count"] += 1
        state["felt_closed"] = False
    elif not authoritative:
        return
    elif outcome == FeltReviewOutcomeV1.FELT_CONFIRMED.value:
        state["activity"] = ContractActivityV1.FELT_CLOSED.value
        state["felt_closed"] = True
    elif outcome == FeltReviewOutcomeV1.NO_RESPONSE.value:
        state["activity"] = ContractActivityV1.QUIET_ARCHIVED.value
        state["felt_closed"] = False
    elif outcome == FeltReviewOutcomeV1.AWAITING.value:
        state["activity"] = ContractActivityV1.REVIEW_PENDING.value


def project_graph(
    events: Iterable[dict[str, Any]],
    *,
    workspace: Path,
) -> dict[str, Any]:
    contracts: dict[str, dict[str, Any]] = {}
    membership: dict[str, str] = {}
    membership_history: list[dict[str, Any]] = []
    nodes: dict[str, dict[str, Any]] = {}
    edges: dict[str, dict[str, Any]] = {}
    node_to_claim: dict[str, str] = {}
    migration_receipt: dict[str, Any] = {}
    implementation_receipts: dict[str, dict[str, Any]] = {}
    review_events: set[tuple[str, str]] = set()
    event_count = 0

    for event_index, event in enumerate(events):
        event_count += 1
        assert_artifact_authority_tree(event)
        event_type = str(event.get("event_type") or "")
        if event_type == "felt_contract_created":
            contract = event.get("contract")
            if not isinstance(contract, dict):
                raise GraphProjectionError("contract creation has no contract record")
            contract_id = str(contract.get("contract_id") or "")
            existing = contracts.get(contract_id)
            if existing and existing["contract"] != contract:
                raise GraphProjectionError(f"contract identity changed: {contract_id}")
            contracts.setdefault(contract_id, _initial_contract_state(contract))
            continue

        if event_type in {
            "felt_contract_claim_assigned",
            "felt_contract_membership_corrected",
        }:
            claim_id = str(event.get("canonical_claim_id") or "")
            contract_id = str(event.get("contract_id") or "")
            if contract_id not in contracts:
                raise GraphProjectionError(
                    f"claim assignment references unknown contract: {contract_id}"
                )
            if not claim_id:
                raise GraphProjectionError("claim assignment has no canonical claim ID")
            previous = membership.get(claim_id)
            if event_type == "felt_contract_claim_assigned" and previous is not None:
                raise GraphProjectionError(
                    f"claim has duplicate initial membership: {claim_id}"
                )
            if event_type == "felt_contract_membership_corrected":
                declared_previous = str(event.get("from_contract_id") or "")
                if previous is None:
                    raise GraphProjectionError(
                        f"membership correction has no prior assignment: {claim_id}"
                    )
                if declared_previous != previous:
                    raise GraphProjectionError(
                        f"membership correction prior contract mismatch: {claim_id}"
                    )
                if previous == contract_id:
                    raise GraphProjectionError(
                        f"membership correction does not move claim: {claim_id}"
                    )
            membership[claim_id] = contract_id
            if event_type == "felt_contract_membership_corrected":
                membership_history.append(
                    {
                        "claim_id": claim_id,
                        "from_contract_id": previous,
                        "to_contract_id": contract_id,
                        "event_index": event_index,
                        "reason_sha256": event.get("reason_sha256"),
                    }
                )
            continue

        if event_type == "felt_contract_node_recorded":
            node = event.get("node")
            node_edges = event.get("edges")
            if not isinstance(node, dict) or not isinstance(node_edges, list):
                raise GraphProjectionError("node event is malformed")
            _verify_record_hash(node, "node_sha256", label="contract node")
            node_id = str(node.get("node_id") or "")
            contract_id = str(node.get("contract_id") or "")
            if contract_id not in contracts:
                raise GraphProjectionError(
                    f"node references unknown contract: {contract_id}"
                )
            if node_id in nodes:
                raise GraphProjectionError(f"duplicate node event: {node_id}")
            nodes[node_id] = node
            contract = contracts[contract_id]
            if node_id not in contract["node_ids"]:
                contract["node_ids"].append(node_id)
                contract["node_kind_counts"][str(node.get("kind") or "unknown")] += 1
            contract["last_change_at"] = node.get("occurred_at")

            parent_claims: set[str] = set()
            for edge in node_edges:
                if not isinstance(edge, dict):
                    raise GraphProjectionError("edge record must be an object")
                _verify_record_hash(edge, "edge_sha256", label="contract edge")
                edge_id = str(edge.get("edge_id") or "")
                source_node = str(edge.get("source_node_id") or "")
                target_node = str(edge.get("target_node_id") or "")
                if target_node != node_id:
                    raise GraphProjectionError(
                        f"edge target does not match enclosing node: {edge_id}"
                    )
                if source_node not in nodes:
                    raise GraphProjectionError(
                        f"edge has dangling or forward parent: {edge_id}"
                    )
                if edge_id in edges and edges[edge_id] != edge:
                    raise GraphProjectionError(f"edge identity collision: {edge_id}")
                edges[edge_id] = edge
                if edge_id not in contract["edge_ids"]:
                    contract["edge_ids"].append(edge_id)
                if source_node in node_to_claim:
                    parent_claims.add(node_to_claim[source_node])

            metadata = node.get("metadata")
            metadata = metadata if isinstance(metadata, dict) else {}
            claim_id = str(metadata.get("canonical_claim_id") or "")
            if claim_id:
                node_to_claim[node_id] = claim_id
            elif len(parent_claims) == 1:
                claim_id = next(iter(parent_claims))
                node_to_claim[node_id] = claim_id

            kind = str(node.get("kind") or "")
            if claim_id:
                contract["technical_by_claim"].setdefault(
                    claim_id, TechnicalDispositionV1.UNASSESSED.value
                )
                contract["evidence_by_claim"].setdefault(
                    claim_id, EvidenceSufficiencyV1.UNASSESSED.value
                )
                technical = str(metadata.get("technical_disposition") or "")
                if technical in {item.value for item in TechnicalDispositionV1}:
                    contract["technical_by_claim"][claim_id] = technical
                if kind == "evidence":
                    current = contract["evidence_by_claim"][claim_id]
                    if current == EvidenceSufficiencyV1.UNASSESSED.value:
                        contract["evidence_by_claim"][
                            claim_id
                        ] = EvidenceSufficiencyV1.PARTIAL.value
                if technical == TechnicalDispositionV1.VERIFIED.value:
                    if contract["evidence_by_claim"][claim_id] in {
                        EvidenceSufficiencyV1.UNASSESSED.value,
                        EvidenceSufficiencyV1.PARTIAL.value,
                    }:
                        contract["evidence_by_claim"][
                            claim_id
                        ] = EvidenceSufficiencyV1.SUFFICIENT.value
                if str(metadata.get("source_status") or "") == "closed_felt_confirmed":
                    _apply_review(
                        contract,
                        FeltReviewOutcomeV1.FELT_CONFIRMED.value,
                        authoritative=True,
                    )

            review_outcome = str(metadata.get("felt_review_outcome") or "")
            if review_outcome:
                compatibility_only = bool(
                    metadata.get("legacy_resolved_is_compatibility_evidence_only")
                    or metadata.get("legacy_no_response_compatibility_only")
                )
                if not compatibility_only:
                    _apply_review(
                        contract,
                        review_outcome,
                        authoritative=bool(
                            metadata.get("authoritative_contract_review")
                        ),
                    )
                if review_outcome == FeltReviewOutcomeV1.CONTRADICTED.value and claim_id:
                    contract["evidence_by_claim"][
                        claim_id
                    ] = EvidenceSufficiencyV1.CONTRADICTED.value
            continue

        if event_type == "felt_contract_implementation_recorded":
            receipt = event.get("implementation_receipt")
            if not isinstance(receipt, dict):
                raise GraphProjectionError("implementation event has no receipt")
            receipt_id = str(receipt.get("receipt_id") or "")
            existing = implementation_receipts.get(receipt_id)
            if existing and existing != receipt:
                raise GraphProjectionError(
                    f"implementation receipt identity collision: {receipt_id}"
                )
            implementation_receipts[receipt_id] = receipt
            continue

        if event_type == "felt_contract_review_outcome_recorded":
            contract_id = str(event.get("contract_id") or "")
            deployment_id = str(event.get("deployment_receipt_id") or "")
            outcome = str(event.get("outcome") or "")
            if contract_id not in contracts:
                raise GraphProjectionError(
                    f"review references unknown contract: {contract_id}"
                )
            review_events.add((contract_id, deployment_id))
            _apply_review(contracts[contract_id], outcome, authoritative=True)
            continue

        if event_type == "felt_contract_migration_completed":
            receipt = event.get("migration_receipt")
            if isinstance(receipt, dict):
                migration_receipt = receipt

    for state in contracts.values():
        state["claims"].clear()
    for claim_id, contract_id in membership.items():
        if contract_id not in contracts:
            raise GraphProjectionError(
                f"current membership references unknown contract: {contract_id}"
            )
        contracts[contract_id]["claims"].add(claim_id)
        contracts[contract_id]["technical_by_claim"].setdefault(
            claim_id, TechnicalDispositionV1.UNASSESSED.value
        )
        contracts[contract_id]["evidence_by_claim"].setdefault(
            claim_id, EvidenceSufficiencyV1.UNASSESSED.value
        )

    for state in contracts.values():
        claim_states = [
            state["technical_by_claim"].get(
                claim_id, TechnicalDispositionV1.UNASSESSED.value
            )
            for claim_id in state["claims"]
        ]
        if claim_states and all(
            value
            in {
                TechnicalDispositionV1.TERMINAL_NO_ACTION.value,
                TechnicalDispositionV1.SUPERSEDED.value,
            }
            for value in claim_states
        ):
            state["administrative_terminal"] = True
            if state["activity"] not in {
                ContractActivityV1.FELT_CLOSED.value,
                ContractActivityV1.REOPENED.value,
            }:
                state[
                    "activity"
                ] = ContractActivityV1.ADMINISTRATIVELY_TERMINAL.value

    deployment = latest_successful_deployment(workspace)
    deployment_id = str(deployment.get("id") or "") if deployment else ""
    changed_contracts = _change_ref_contracts(deployment or {})
    review_budgets: dict[str, dict[str, Any]] = {}
    for contract_id in sorted(changed_contracts):
        if contract_id not in contracts:
            continue
        consumed = (contract_id, deployment_id) in review_events
        review_budgets[contract_id] = {
            "schema": "felt_contract_review_budget_v1",
            "schema_version": 1,
            "contract_id": contract_id,
            "deployment_receipt_id": deployment_id,
            "packet_budget": 1,
            "delivered_or_answered_count": 1 if consumed else 0,
            "packet_available": not consumed,
            "individual_claim_cards_queryable": True,
            "duplicate_delivery_held": True,
            "objection_or_still_friction_bypasses_hold": True,
            "silence_classification": FeltReviewOutcomeV1.NO_RESPONSE.value,
            "silence_affirms": False,
            "silence_closes": False,
        }

    contract_rows = []
    for contract_id, state in sorted(contracts.items()):
        technical_counts = Counter(
            state["technical_by_claim"].get(
                claim_id, TechnicalDispositionV1.UNASSESSED.value
            )
            for claim_id in state["claims"]
        )
        evidence_counts = Counter(
            state["evidence_by_claim"].get(
                claim_id, EvidenceSufficiencyV1.UNASSESSED.value
            )
            for claim_id in state["claims"]
        )
        contract_rows.append(
            {
                "schema": "felt_contract_projection_v1",
                "schema_version": 1,
                "contract_id": contract_id,
                "anchor_claim_id": state["contract"].get("anchor_claim_id"),
                "claim_ids": sorted(state["claims"]),
                "claim_count": len(state["claims"]),
                "node_count": len(state["node_ids"]),
                "edge_count": len(state["edge_ids"]),
                "node_kind_counts": dict(sorted(state["node_kind_counts"].items())),
                "technical_state_counts": dict(sorted(technical_counts.items())),
                "evidence_state_counts": dict(sorted(evidence_counts.items())),
                "felt_review": state["felt_review"],
                "activity": state["activity"],
                "administrative_terminal": state["administrative_terminal"],
                "felt_closed": state["felt_closed"],
                "reopen_count": state["reopen_count"],
                "contradiction_count": state["contradiction_count"],
                "last_change_at": state["last_change_at"],
                "identity_stable_across_membership_changes": True,
                "membership_propagates_closure": False,
                "membership_propagates_evidence_sufficiency": False,
                "membership_propagates_supersession": False,
                "membership_propagates_authority": False,
                "artifact_authority_state_v1": state["contract"].get(
                    "artifact_authority_state_v1", _authority()
                ),
            }
        )

    activity_counts = Counter(row["activity"] for row in contract_rows)
    felt_counts = Counter(row["felt_review"] for row in contract_rows)
    checks = {
        "every_claim_assigned_once": len(membership)
        == len(set(membership)),
        "every_membership_has_contract": all(
            contract_id in contracts for contract_id in membership.values()
        ),
        "contract_ids_unique": len(contracts) == len(set(contracts)),
        "node_ids_unique": len(nodes) == len(set(nodes)),
        "edge_ids_unique": len(edges) == len(set(edges)),
        "no_dangling_edges": all(
            edge.get("source_node_id") in nodes
            and edge.get("target_node_id") in nodes
            for edge in edges.values()
        ),
        "family_membership_does_not_propagate_closure": True,
        "family_membership_does_not_propagate_evidence": True,
        "family_membership_does_not_propagate_supersession": True,
        "family_membership_does_not_propagate_authority": True,
        "silence_does_not_affirm": True,
        "silence_does_not_close": True,
    }
    status = {
        "schema": "felt_contract_graph_status_v1",
        "schema_version": 1,
        "projector_version": PROJECTOR_VERSION,
        "contract_count": len(contracts),
        "claim_count": len(membership),
        "node_count": len(nodes),
        "edge_count": len(edges),
        "event_count": event_count,
        "implementation_receipt_count": len(implementation_receipts),
        "membership_correction_count": len(membership_history),
        "activity_counts": dict(sorted(activity_counts.items())),
        "felt_review_counts": dict(sorted(felt_counts.items())),
        "latest_successful_deployment_receipt_id": deployment_id or None,
        "latest_deployment_exact_change_ref_count": len(changed_contracts),
        "review_budget_count": len(review_budgets),
        "review_budgets": review_budgets,
        "counter_audit": {
            "schema": "felt_contract_counter_audit_v1",
            "schema_version": 1,
            "status": "consistent" if all(checks.values()) else "inconsistent",
            "checks": checks,
        },
        "migration_receipt": migration_receipt,
        "private_content_copied": False,
        "artifact_authority_state_v1": _authority(),
    }
    assert_artifact_authority_tree(status)
    return {
        "status": status,
        "contracts": contract_rows,
        "nodes": nodes,
        "edges": edges,
        "membership": membership,
        "review_budgets": review_budgets,
    }


def render_report(projection: dict[str, Any]) -> str:
    status = projection["status"]
    counter = status["counter_audit"]
    return "\n".join(
        [
            "# Living Felt Contract Graph",
            "",
            f"- Contracts: {status['contract_count']}",
            f"- Claims: {status['claim_count']}",
            f"- Nodes: {status['node_count']}",
            f"- Edges: {status['edge_count']}",
            f"- Membership corrections: {status['membership_correction_count']}",
            f"- Review budgets: {status['review_budget_count']}",
            f"- Counter audit: {counter['status']}",
            "- Contract identity: stable concern anchored to earliest canonical claim",
            "- Technical, evidence, felt-review, and activity states: orthogonal",
            "- Family propagation of closure/evidence/supersession/authority: disabled",
            "- Silence: no_response; never affirmation, waiver, approval, or closure",
            "- Authority: evidence_only or approval_pending; never live",
            "",
        ]
    )


def projection_payloads(projection: dict[str, Any]) -> dict[str, str]:
    status_payload = json.dumps(
        projection["status"], indent=2, sort_keys=True, ensure_ascii=False
    ) + "\n"
    contracts_payload = "".join(
        canonical_json(row) + "\n" for row in projection["contracts"]
    )
    migration_payload = json.dumps(
        projection["status"].get("migration_receipt") or {},
        indent=2,
        sort_keys=True,
        ensure_ascii=False,
    ) + "\n"
    return {
        "status.json": status_payload,
        "contracts.jsonl": contracts_payload,
        "report.md": render_report(projection),
        "migration_receipt.json": migration_payload,
    }


def payload_hashes(payloads: dict[str, str]) -> dict[str, str]:
    return {
        name: hashlib.sha256(payload.encode("utf-8")).hexdigest()
        for name, payload in sorted(payloads.items())
        if name in {"contracts.jsonl", "report.md"}
    }


def _atomic_write_text(path: Path, payload: str) -> None:
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


def write_projection(workspace: Path, projection: dict[str, Any]) -> dict[str, str]:
    root = graph_state_dir(workspace)
    payloads = projection_payloads(projection)
    hashes: dict[str, str] = {}
    for name, payload in payloads.items():
        _atomic_write_text(root / name, payload)
        hashes[name] = hashlib.sha256(payload.encode("utf-8")).hexdigest()
    for contract_id, budget in sorted(projection["review_budgets"].items()):
        packet = {
            "schema": "felt_contract_review_packet_v1",
            "schema_version": 1,
            "contract_id": contract_id,
            "claim_ids": next(
                row["claim_ids"]
                for row in projection["contracts"]
                if row["contract_id"] == contract_id
            ),
            "deployment_receipt_id": budget["deployment_receipt_id"],
            "right_to_ignore": True,
            "silence_classification": FeltReviewOutcomeV1.NO_RESPONSE.value,
            "silence_affirms": False,
            "silence_closes": False,
            "private_content_copied": False,
            "artifact_authority_state_v1": _authority(),
        }
        payload = json.dumps(packet, indent=2, sort_keys=True) + "\n"
        relative = f"review_packets/{contract_id}_{budget['deployment_receipt_id']}.json"
        _atomic_write_text(root / relative, payload)
        hashes[relative] = hashlib.sha256(payload.encode()).hexdigest()
    return dict(sorted(hashes.items()))


def contract_view(projection: dict[str, Any], contract_id: str) -> dict[str, Any]:
    row = next(
        (
            candidate
            for candidate in projection["contracts"]
            if candidate["contract_id"] == contract_id
        ),
        None,
    )
    if row is None:
        raise KeyError(f"unknown contract: {contract_id}")
    node_ids = {
        node_id
        for node_id, node in projection["nodes"].items()
        if node.get("contract_id") == contract_id
    }
    return {
        **row,
        "nodes": [
            projection["nodes"][node_id] for node_id in sorted(node_ids)
        ],
        "edges": [
            edge
            for _, edge in sorted(projection["edges"].items())
            if edge.get("contract_id") == contract_id
        ],
    }


def claim_view(projection: dict[str, Any], claim_id: str) -> dict[str, Any]:
    contract_id = projection["membership"].get(claim_id)
    if not contract_id:
        raise KeyError(f"unknown claim: {claim_id}")
    contract = contract_view(projection, contract_id)
    return {
        "schema": "felt_contract_claim_view_v1",
        "schema_version": 1,
        "canonical_claim_id": claim_id,
        "contract_id": contract_id,
        "contract": contract,
    }


def trace_view(
    projection: dict[str, Any],
    node_id: str,
    *,
    direction: str,
    depth: int,
) -> dict[str, Any]:
    if node_id not in projection["nodes"]:
        raise KeyError(f"unknown node: {node_id}")
    if direction not in {"parents", "children", "both"}:
        raise ValueError("trace direction must be parents, children, or both")
    if depth < 0 or depth > 64:
        raise ValueError("trace depth must be between 0 and 64")
    selected = {node_id}
    frontier = {node_id}
    for _ in range(depth):
        next_frontier: set[str] = set()
        for edge in projection["edges"].values():
            source = str(edge.get("source_node_id") or "")
            target = str(edge.get("target_node_id") or "")
            if direction in {"children", "both"} and source in frontier:
                next_frontier.add(target)
            if direction in {"parents", "both"} and target in frontier:
                next_frontier.add(source)
        next_frontier -= selected
        if not next_frontier:
            break
        selected.update(next_frontier)
        frontier = next_frontier
    return {
        "schema": "felt_contract_trace_v1",
        "schema_version": 1,
        "root_node_id": node_id,
        "direction": direction,
        "depth": depth,
        "nodes": [
            projection["nodes"][selected_id] for selected_id in sorted(selected)
        ],
        "edges": [
            edge
            for _, edge in sorted(projection["edges"].items())
            if edge.get("source_node_id") in selected
            and edge.get("target_node_id") in selected
        ],
    }
