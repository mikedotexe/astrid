"""Immutable validated records for the living felt-contract graph."""

from __future__ import annotations

from dataclasses import InitVar, dataclass
from enum import StrEnum
import re
from typing import Any, Iterable

try:
    from authority_state import (
        ArtifactAuthorityStateV1,
        assert_artifact_authority_tree,
    )
    from evidence_store.model import sha256_canonical
except ModuleNotFoundError:
    from scripts.authority_state import (
        ArtifactAuthorityStateV1,
        assert_artifact_authority_tree,
    )
    from scripts.evidence_store.model import sha256_canonical

_TRUSTED_CONSTRUCTION = object()
_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
_ID_RE = re.compile(r"^[A-Za-z0-9_.:-]{1,240}$")
_FIELD_PATH_RE = re.compile(r"^[A-Za-z0-9_.:\-\[\]]{1,240}$")
_RAW_CONTENT_KEYS = frozenset(
    {
        "claim_summary",
        "content",
        "felt_report_anchor",
        "hypothesis",
        "note",
        "prose",
        "rationale",
        "raw",
        "summary",
        "text",
        "title",
    }
)


class ClaimDispositionV1(StrEnum):
    UNASSESSED = "unassessed"
    READY = "ready"
    GATED = "gated"
    IMPLEMENTED = "implemented"
    DEPLOYED = "deployed"
    VERIFIED = "verified"
    TERMINAL_NO_ACTION = "terminal_no_action"
    SUPERSEDED = "superseded"


TechnicalDispositionV1 = ClaimDispositionV1


class EvidenceSufficiencyV1(StrEnum):
    UNASSESSED = "unassessed"
    INSUFFICIENT = "insufficient"
    PARTIAL = "partial"
    SUFFICIENT = "sufficient"
    CONTRADICTED = "contradicted"


class FeltReviewOutcomeV1(StrEnum):
    NOT_REQUESTED = "not_requested"
    AWAITING = "awaiting"
    NO_RESPONSE = "no_response"
    IMPROVED_NAMED = "improved_named"
    STILL_FRICTION = "still_friction"
    CONTRADICTED = "contradicted"
    FELT_CONFIRMED = "felt_confirmed"
    OBJECTION = "objection"


class ContractActivityV1(StrEnum):
    OPEN = "open"
    WATCHING = "watching"
    REVIEW_PENDING = "review_pending"
    QUIET_ARCHIVED = "quiet_archived"
    ADMINISTRATIVELY_TERMINAL = "administratively_terminal"
    FELT_CLOSED = "felt_closed"
    REOPENED = "reopened"


def _require_trusted(token: object) -> None:
    if token is not _TRUSTED_CONSTRUCTION:
        raise TypeError("trusted graph records must be created by validated builders")


def _identifier(value: str, label: str) -> str:
    clean = str(value or "").strip()
    if not _ID_RE.fullmatch(clean):
        raise ValueError(f"invalid {label}: {value!r}")
    return clean


def _sha256(value: str, label: str) -> str:
    clean = str(value or "").strip().lower()
    if not _SHA256_RE.fullmatch(clean):
        raise ValueError(f"{label} must be a lowercase SHA-256")
    return clean


def _timestamp(value: str, label: str) -> str:
    clean = str(value or "").strip()
    if not clean or len(clean) > 120 or "\n" in clean or "\r" in clean:
        raise ValueError(f"invalid {label}")
    return clean


def _field_paths(values: Iterable[str]) -> tuple[str, ...]:
    paths = tuple(sorted({str(value).strip() for value in values if str(value).strip()}))
    for path in paths:
        if path.startswith("/") or not _FIELD_PATH_RE.fullmatch(path):
            raise ValueError(f"invalid bounded field path: {path!r}")
    return paths


def _bounded_metadata(value: Any, *, path: tuple[str, ...] = ()) -> Any:
    if isinstance(value, dict):
        result: dict[str, Any] = {}
        for raw_key, raw_value in sorted(value.items(), key=lambda item: str(item[0])):
            key = str(raw_key)
            if key.lower() in _RAW_CONTENT_KEYS:
                raise ValueError(f"raw private content key is forbidden: {'.'.join((*path, key))}")
            result[key] = _bounded_metadata(raw_value, path=(*path, key))
        assert_artifact_authority_tree(result)
        return result
    if isinstance(value, (list, tuple)):
        return [_bounded_metadata(item, path=(*path, str(index))) for index, item in enumerate(value)]
    if isinstance(value, str):
        if len(value) > 500:
            raise ValueError(f"metadata string exceeds 500 characters at {'.'.join(path)}")
        if value.startswith("/") or re.match(r"^[A-Za-z]:[\\/]", value):
            raise ValueError(f"absolute path is forbidden at {'.'.join(path)}")
        if "\x00" in value:
            raise ValueError(f"NUL is forbidden at {'.'.join(path)}")
        return value
    if value is None or isinstance(value, (bool, int, float)):
        return value
    raise ValueError(f"unsupported metadata type at {'.'.join(path)}")


def _authority(state: str) -> dict[str, Any]:
    return ArtifactAuthorityStateV1(state).canonical_record()


@dataclass(frozen=True)
class FeltSignalRefV1:
    source_kind: str
    source_id: str
    canonical_sha256: str
    owner: str
    observed_at: str
    field_paths: tuple[str, ...]
    _token: InitVar[object] = None

    def __post_init__(self, _token: object) -> None:
        _require_trusted(_token)
        _identifier(self.source_kind, "signal source kind")
        _identifier(self.source_id, "signal source ID")
        _sha256(self.canonical_sha256, "signal canonical SHA-256")
        _identifier(self.owner, "signal owner")
        _timestamp(self.observed_at, "signal timestamp")
        _field_paths(self.field_paths)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "felt_signal_ref_v1",
            "schema_version": 1,
            "source_kind": self.source_kind,
            "source_id": self.source_id,
            "canonical_sha256": self.canonical_sha256,
            "owner": self.owner,
            "observed_at": self.observed_at,
            "field_paths": list(self.field_paths),
            "private_content_copied": False,
        }


@dataclass(frozen=True)
class FeltContractV1:
    contract_id: str
    anchor_claim_id: str
    created_at: str
    authority_state: str
    _token: InitVar[object] = None

    def __post_init__(self, _token: object) -> None:
        _require_trusted(_token)
        _identifier(self.contract_id, "contract ID")
        _identifier(self.anchor_claim_id, "anchor claim ID")
        _timestamp(self.created_at, "contract timestamp")
        ArtifactAuthorityStateV1(self.authority_state)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "felt_contract_v1",
            "schema_version": 1,
            "contract_id": self.contract_id,
            "anchor_claim_id": self.anchor_claim_id,
            "created_at": self.created_at,
            "identity_stable_across_membership_changes": True,
            "artifact_authority_state_v1": _authority(self.authority_state),
        }


@dataclass(frozen=True)
class FeltContractNodeV1:
    node_id: str
    contract_id: str
    kind: str
    source_event_id: str
    occurred_at: str
    source_ref: FeltSignalRefV1 | None
    metadata: dict[str, Any]
    authority_state: str
    _token: InitVar[object] = None

    def __post_init__(self, _token: object) -> None:
        _require_trusted(_token)
        _identifier(self.node_id, "node ID")
        _identifier(self.contract_id, "contract ID")
        _identifier(self.kind, "node kind")
        _identifier(self.source_event_id, "source event ID")
        _timestamp(self.occurred_at, "node timestamp")
        _bounded_metadata(self.metadata)
        ArtifactAuthorityStateV1(self.authority_state)

    def to_dict(self) -> dict[str, Any]:
        value = {
            "schema": "felt_contract_node_v1",
            "schema_version": 1,
            "node_id": self.node_id,
            "contract_id": self.contract_id,
            "kind": self.kind,
            "source_event_id": self.source_event_id,
            "occurred_at": self.occurred_at,
            "metadata": _bounded_metadata(self.metadata),
            "artifact_authority_state_v1": _authority(self.authority_state),
        }
        if self.source_ref is not None:
            value["source_ref"] = self.source_ref.to_dict()
        value["node_sha256"] = sha256_canonical(value)
        return value


@dataclass(frozen=True)
class FeltContractEdgeV1:
    edge_id: str
    contract_id: str
    source_node_id: str
    target_node_id: str
    relation: str
    source_event_id: str
    occurred_at: str
    causal_parent: bool
    _token: InitVar[object] = None

    def __post_init__(self, _token: object) -> None:
        _require_trusted(_token)
        for value, label in (
            (self.edge_id, "edge ID"),
            (self.contract_id, "contract ID"),
            (self.source_node_id, "source node ID"),
            (self.target_node_id, "target node ID"),
            (self.relation, "edge relation"),
            (self.source_event_id, "source event ID"),
        ):
            _identifier(value, label)
        _timestamp(self.occurred_at, "edge timestamp")
        if self.source_node_id == self.target_node_id:
            raise ValueError("self-referential contract edges are forbidden")

    def to_dict(self) -> dict[str, Any]:
        value = {
            "schema": "felt_contract_edge_v1",
            "schema_version": 1,
            "edge_id": self.edge_id,
            "contract_id": self.contract_id,
            "source_node_id": self.source_node_id,
            "target_node_id": self.target_node_id,
            "relation": self.relation,
            "source_event_id": self.source_event_id,
            "occurred_at": self.occurred_at,
            "causal_parent": self.causal_parent,
        }
        value["edge_sha256"] = sha256_canonical(value)
        return value


@dataclass(frozen=True)
class InterventionBoundaryV1:
    boundary_id: str
    agency_tier: int
    authority_class: str
    lifecycle_state: str
    authority_state: str
    _token: InitVar[object] = None

    def __post_init__(self, _token: object) -> None:
        _require_trusted(_token)
        _identifier(self.boundary_id, "boundary ID")
        if self.agency_tier < 0 or self.agency_tier > 5:
            raise ValueError("agency tier must be between 0 and 5")
        _identifier(self.authority_class, "authority class")
        _identifier(self.lifecycle_state, "lifecycle state")
        ArtifactAuthorityStateV1(self.authority_state)
        if self.agency_tier >= 4 and self.authority_state != "approval_pending":
            raise ValueError("Tier 4/5 intervention boundaries must remain approval_pending")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "intervention_boundary_v1",
            "schema_version": 1,
            "boundary_id": self.boundary_id,
            "agency_tier": self.agency_tier,
            "authority_class": self.authority_class,
            "lifecycle_state": self.lifecycle_state,
            "artifact_authority_state_v1": _authority(self.authority_state),
        }


@dataclass(frozen=True)
class ContractContradictionV1:
    contradiction_id: str
    contract_id: str
    source_ref: str
    recorded_at: str
    previous_activity: ContractActivityV1
    _token: InitVar[object] = None

    def __post_init__(self, _token: object) -> None:
        _require_trusted(_token)
        _identifier(self.contradiction_id, "contradiction ID")
        _identifier(self.contract_id, "contract ID")
        _identifier(self.source_ref, "contradiction source ref")
        _timestamp(self.recorded_at, "contradiction timestamp")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "contract_contradiction_v1",
            "schema_version": 1,
            "contradiction_id": self.contradiction_id,
            "contract_id": self.contract_id,
            "source_ref": self.source_ref,
            "recorded_at": self.recorded_at,
            "previous_activity": self.previous_activity.value,
            "next_activity": ContractActivityV1.REOPENED.value,
            "history_erased": False,
        }


@dataclass(frozen=True)
class ImplementationReceiptV1:
    receipt_id: str
    actor: str
    recorded_at: str
    repository: str
    source_identity_sha256: str
    contract_ids: tuple[str, ...]
    claim_ids: tuple[str, ...]
    work_item_ids: tuple[str, ...]
    changed_path_hashes: dict[str, str]
    test_refs: tuple[str, ...]
    _token: InitVar[object] = None

    def __post_init__(self, _token: object) -> None:
        _require_trusted(_token)
        for value, label in (
            (self.receipt_id, "implementation receipt ID"),
            (self.actor, "implementation actor"),
            (self.repository, "implementation repository"),
        ):
            _identifier(value, label)
        _timestamp(self.recorded_at, "implementation timestamp")
        _sha256(self.source_identity_sha256, "implementation source identity")
        if not self.contract_ids or not self.claim_ids:
            raise ValueError("implementation receipt requires contract and claim references")
        for value in (*self.contract_ids, *self.claim_ids, *self.work_item_ids):
            _identifier(value, "implementation reference")
        for path_ref, path_sha256 in self.changed_path_hashes.items():
            if path_ref.startswith("/") or not path_ref.startswith("repo:"):
                raise ValueError("changed paths must be repository-relative refs")
            _sha256(path_sha256, "changed path SHA-256")
        for ref in self.test_refs:
            _identifier(ref, "test reference")

    def to_dict(self) -> dict[str, Any]:
        value = {
            "schema": "implementation_receipt_v1",
            "schema_version": 1,
            "receipt_id": self.receipt_id,
            "actor": self.actor,
            "recorded_at": self.recorded_at,
            "repository": self.repository,
            "source_identity_sha256": self.source_identity_sha256,
            "contract_ids": list(self.contract_ids),
            "claim_ids": list(self.claim_ids),
            "work_item_ids": list(self.work_item_ids),
            "changed_path_hashes": dict(sorted(self.changed_path_hashes.items())),
            "test_refs": list(self.test_refs),
            "witness_only": True,
            "artifact_authority_state_v1": _authority("evidence_only"),
        }
        value["receipt_sha256"] = sha256_canonical(value)
        return value


def build_signal_ref(**values: Any) -> FeltSignalRefV1:
    values["field_paths"] = _field_paths(values.get("field_paths") or ())
    return FeltSignalRefV1(**values, _token=_TRUSTED_CONSTRUCTION)


def build_contract(**values: Any) -> FeltContractV1:
    return FeltContractV1(**values, _token=_TRUSTED_CONSTRUCTION)


def build_node(**values: Any) -> FeltContractNodeV1:
    values["metadata"] = _bounded_metadata(values.get("metadata") or {})
    return FeltContractNodeV1(**values, _token=_TRUSTED_CONSTRUCTION)


def build_edge(**values: Any) -> FeltContractEdgeV1:
    return FeltContractEdgeV1(**values, _token=_TRUSTED_CONSTRUCTION)


def build_intervention_boundary(**values: Any) -> InterventionBoundaryV1:
    return InterventionBoundaryV1(**values, _token=_TRUSTED_CONSTRUCTION)


def build_contradiction(**values: Any) -> ContractContradictionV1:
    return ContractContradictionV1(**values, _token=_TRUSTED_CONSTRUCTION)


def build_implementation_receipt(value: dict[str, Any]) -> ImplementationReceiptV1:
    if value.get("schema") != "implementation_receipt_v1":
        raise ValueError("implementation receipt schema must be implementation_receipt_v1")
    assert_artifact_authority_tree(value)
    authority = value.get("artifact_authority_state_v1")
    if not isinstance(authority, dict) or authority.get("state") != "evidence_only":
        raise ValueError("implementation receipts must remain evidence_only")
    receipt = ImplementationReceiptV1(
        receipt_id=str(value.get("receipt_id") or ""),
        actor=str(value.get("actor") or ""),
        recorded_at=str(value.get("recorded_at") or ""),
        repository=str(value.get("repository") or ""),
        source_identity_sha256=str(value.get("source_identity_sha256") or ""),
        contract_ids=tuple(str(item) for item in (value.get("contract_ids") or ())),
        claim_ids=tuple(str(item) for item in (value.get("claim_ids") or ())),
        work_item_ids=tuple(str(item) for item in (value.get("work_item_ids") or ())),
        changed_path_hashes={
            str(key): str(item)
            for key, item in (value.get("changed_path_hashes") or {}).items()
        },
        test_refs=tuple(str(item) for item in (value.get("test_refs") or ())),
        _token=_TRUSTED_CONSTRUCTION,
    )
    expected = value.get("receipt_sha256")
    if expected and expected != receipt.to_dict()["receipt_sha256"]:
        raise ValueError("implementation receipt hash mismatch")
    return receipt
