"""Deterministic identities for living felt-contract records."""

from __future__ import annotations

import hashlib
from typing import Any

try:
    from evidence_store.model import canonical_json
except ModuleNotFoundError:
    from scripts.evidence_store.model import canonical_json

IDENTITY_NAMESPACE = "astrid_living_felt_contract_graph_v1"


def digest(value: Any) -> str:
    return hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest()


def contract_id_for_anchor(anchor_claim_id: str) -> str:
    if not anchor_claim_id.strip():
        raise ValueError("anchor claim ID must not be empty")
    return f"contract_{digest([IDENTITY_NAMESPACE, 'contract', anchor_claim_id])[:24]}"


def node_id(source_event_id: str, semantic_kind: str, contract_id: str) -> str:
    if not source_event_id.strip() or not semantic_kind.strip() or not contract_id.strip():
        raise ValueError("node identity inputs must not be empty")
    return (
        "node_"
        + digest(
            [
                IDENTITY_NAMESPACE,
                "node",
                source_event_id,
                semantic_kind,
                contract_id,
            ]
        )[:24]
    )


def edge_id(
    source_node_id: str,
    target_node_id: str,
    relation: str,
    source_event_id: str,
) -> str:
    if not all(
        value.strip()
        for value in (source_node_id, target_node_id, relation, source_event_id)
    ):
        raise ValueError("edge identity inputs must not be empty")
    return (
        "edge_"
        + digest(
            [
                IDENTITY_NAMESPACE,
                "edge",
                source_node_id,
                target_node_id,
                relation,
                source_event_id,
            ]
        )[:24]
    )
