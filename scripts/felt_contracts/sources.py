"""Astrid V2 event adapters for the living felt-contract graph."""

from __future__ import annotations

from collections import defaultdict
from dataclasses import dataclass
from datetime import UTC, datetime
import hashlib
import json
from pathlib import Path
import re
from typing import Any, Iterable

try:
    from claim_family_matcher import MATCH_THRESHOLD, weighted_similarity
    from evidence_store import EvidenceEventStore
    from evidence_store.model import EvidenceEventV2, sha256_canonical
except ModuleNotFoundError:
    from scripts.claim_family_matcher import MATCH_THRESHOLD, weighted_similarity
    from scripts.evidence_store import EvidenceEventStore
    from scripts.evidence_store.model import EvidenceEventV2, sha256_canonical

from .identity import contract_id_for_anchor, edge_id, node_id
from .model import (
    FeltReviewOutcomeV1,
    TechnicalDispositionV1,
    build_contract,
    build_edge,
    build_node,
    build_signal_ref,
)

GRAPH_SCHEMA = "felt_contract_domain_event_v1"
GRAPH_VERSION = 1
SOURCE_STREAMS = (
    "addressing",
    "sandbox",
    "corridor_v1",
    "corridor_v2",
    "signal_spine",
    "lived_state_witness",
    "claim_families",
)
_TIMESTAMP_RE = re.compile(r"_(\d{9,})$")
_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")


@dataclass(frozen=True)
class ClaimSource:
    claim_id: str
    introspection_id: str
    local_claim_id: str
    source_sha256: str
    queue_order: tuple[int, str]
    family_id: str
    authority_class: str
    target_surface: str
    requested_outcome: str
    polarity: str
    text: str
    disposition: str
    classification: str
    record_sha256: str
    source_path_ref: str | None

    @property
    def match_key(self) -> tuple[str, str, str, str]:
        return (
            self.authority_class,
            self.target_surface,
            self.requested_outcome,
            self.polarity,
        )


@dataclass(frozen=True)
class SourceBuild:
    events: tuple[dict[str, Any], ...]
    claim_count: int
    contract_count: int
    source_counts: dict[str, int]
    routed_source_events: int
    unrouted_source_events: int
    ambiguous_new_claims: int
    source_watermarks: dict[str, dict[str, Any]]
    source_hashes: dict[str, str]


def graph_state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/felt_contract_graph_v1"


def store_root(workspace: Path) -> Path:
    return workspace / "diagnostics/evidence_event_store_v2"


def _authority(state: str = "evidence_only") -> dict[str, Any]:
    if state not in {"evidence_only", "approval_pending"}:
        raise ValueError(f"unsupported graph authority state: {state}")
    return {
        "schema": "artifact_authority_state_v1",
        "schema_version": 1,
        "state": state,
        "witness_only": True,
    }


def _event(
    event_type: str,
    aggregate_type: str,
    aggregate_id: str,
    idempotency_key: str,
    *,
    authority_state: str = "evidence_only",
    **values: Any,
) -> dict[str, Any]:
    return {
        "schema": GRAPH_SCHEMA,
        "schema_version": GRAPH_VERSION,
        "event_type": event_type,
        "aggregate_type": aggregate_type,
        "aggregate_id": aggregate_id,
        **values,
        "idempotency_key": idempotency_key,
        "artifact_authority_state_v1": _authority(authority_state),
    }


def _unix_iso(value: Any, fallback: str) -> str:
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        try:
            return datetime.fromtimestamp(float(value), tz=UTC).isoformat()
        except (OSError, OverflowError, ValueError):
            pass
    return fallback


def _event_time(envelope: EvidenceEventV2) -> str:
    return _unix_iso(envelope.payload.get("ts"), envelope.recorded_at)


def _source_event_ref(envelope: EvidenceEventV2) -> dict[str, Any]:
    return {
        "event_id": envelope.event_id,
        "stream": envelope.stream,
        "stream_seq": envelope.stream_seq,
        "global_seq": envelope.global_seq,
        "event_sha256": envelope.event_sha256,
    }


def _owner_for_introspection(introspection_id: str) -> str:
    if introspection_id.startswith("introspection_astrid_"):
        return "astrid"
    if introspection_id.startswith("introspection_minime_"):
        return "minime"
    return "unknown"


def _queue_order(claim: dict[str, Any], introspection_id: str) -> tuple[int, str]:
    position = claim.get("queue_position")
    if isinstance(position, int) and not isinstance(position, bool):
        return position, introspection_id
    match = _TIMESTAMP_RE.search(introspection_id)
    return (int(match.group(1)) if match else 2**63 - 1), introspection_id


def _repository_ref(path_text: Any, roots: dict[str, Path]) -> str | None:
    text = str(path_text or "").strip()
    if not text:
        return None
    candidate = Path(text).expanduser()
    if candidate.is_absolute():
        resolved = candidate.resolve(strict=False)
        for name, root in roots.items():
            try:
                relative = resolved.relative_to(root.resolve())
            except ValueError:
                continue
            return f"repo:{name}/{relative.as_posix()}"
        return f"external_path_sha256:{hashlib.sha256(text.encode()).hexdigest()}"
    if len(text) > 500:
        return f"opaque_ref_sha256:{hashlib.sha256(text.encode()).hexdigest()}"
    return text


def _hash_text(value: Any) -> str | None:
    text = str(value or "").strip()
    return hashlib.sha256(text.encode()).hexdigest() if text else None


def _claim_sources(workspace: Path) -> tuple[list[ClaimSource], str]:
    path = workspace / "diagnostics/claim_families_v1/status.json"
    raw = path.read_bytes()
    status = json.loads(raw)
    families = status.get("families")
    if not isinstance(families, dict):
        raise ValueError("claim-family status has no families")
    roots = {
        "astrid": Path(__file__).resolve().parents[2],
        "minime": Path(__file__).resolve().parents[3] / "minime",
        "model": Path(__file__).resolve().parents[3] / "neural-triple-reservoir",
    }
    claims: list[ClaimSource] = []
    for family_id, family in sorted(families.items()):
        if not isinstance(family, dict):
            continue
        family_claims = family.get("claims")
        if not isinstance(family_claims, dict):
            continue
        for claim_id, claim in sorted(family_claims.items()):
            if not isinstance(claim, dict):
                continue
            introspection_id = str(claim.get("introspection_id") or claim_id.split(":", 1)[0])
            source_sha256 = str(claim.get("source_sha256") or "")
            if not _SHA256_RE.fullmatch(source_sha256):
                source_sha256 = sha256_canonical(
                    {
                        "canonical_claim_id": claim_id,
                        "canonical_claim_record_sha256": claim.get(
                            "canonical_claim_record_sha256"
                        ),
                    }
                )
            claims.append(
                ClaimSource(
                    claim_id=str(claim_id),
                    introspection_id=introspection_id,
                    local_claim_id=str(
                        claim.get("claim_id") or str(claim_id).split(":")[-1]
                    ),
                    source_sha256=source_sha256,
                    queue_order=_queue_order(claim, introspection_id),
                    family_id=str(family_id),
                    authority_class=str(
                        family.get("authority_class") or "evidence_only_non_live"
                    ),
                    target_surface=str(family.get("target_surface") or "unknown_surface"),
                    requested_outcome=str(
                        family.get("requested_outcome") or "observe_or_verify"
                    ),
                    polarity=str(family.get("polarity") or "neutral"),
                    text=str(claim.get("text") or ""),
                    disposition=str(claim.get("disposition") or ""),
                    classification=str(claim.get("classification") or ""),
                    record_sha256=str(
                        claim.get("canonical_claim_record_sha256")
                        or sha256_canonical(claim)
                    ),
                    source_path_ref=_repository_ref(claim.get("source_path"), roots),
                )
            )
    return claims, sha256_canonical(
        [
            {
                "claim_id": claim.claim_id,
                "introspection_id": claim.introspection_id,
                "local_claim_id": claim.local_claim_id,
                "source_sha256": claim.source_sha256,
                "queue_order": list(claim.queue_order),
                "family_id": claim.family_id,
                "authority_class": claim.authority_class,
                "target_surface": claim.target_surface,
                "requested_outcome": claim.requested_outcome,
                "polarity": claim.polarity,
                "text_sha256": hashlib.sha256(claim.text.encode()).hexdigest(),
                "disposition": claim.disposition,
                "classification": claim.classification,
                "record_sha256": claim.record_sha256,
                "source_path_ref": claim.source_path_ref,
            }
            for claim in claims
        ]
    )


def claim_family_semantic_sha256(workspace: Path) -> str:
    """Hash only the stable family/claim fields consumed by this graph."""

    _, semantic_sha256 = _claim_sources(workspace)
    return semantic_sha256


def _current_graph_membership(
    graph_events: Iterable[EvidenceEventV2],
) -> tuple[dict[str, str], dict[str, dict[str, Any]]]:
    membership: dict[str, str] = {}
    contracts: dict[str, dict[str, Any]] = {}
    for envelope in graph_events:
        payload = envelope.payload
        event_type = payload.get("event_type")
        if event_type == "felt_contract_created" and isinstance(
            payload.get("contract"), dict
        ):
            contracts[str(payload["contract"].get("contract_id") or "")] = payload[
                "contract"
            ]
        elif event_type in {
            "felt_contract_claim_assigned",
            "felt_contract_membership_corrected",
        }:
            claim_id = str(payload.get("canonical_claim_id") or "")
            contract_id = str(payload.get("contract_id") or "")
            if claim_id and contract_id:
                membership[claim_id] = contract_id
    return membership, contracts


def _strict_assignment(
    claim: ClaimSource,
    membership: dict[str, str],
    sources: dict[str, ClaimSource],
) -> tuple[str | None, list[dict[str, Any]]]:
    members_by_contract: dict[str, list[ClaimSource]] = defaultdict(list)
    for member_id, contract_id in membership.items():
        member = sources.get(member_id)
        if member is not None and member.match_key == claim.match_key:
            members_by_contract[contract_id].append(member)
    candidates: list[tuple[float, str]] = []
    suggestions: list[dict[str, Any]] = []
    for contract_id, members in sorted(members_by_contract.items()):
        scores = [weighted_similarity(claim.text, member.text) for member in members]
        if scores and min(scores) >= MATCH_THRESHOLD:
            candidates.append((sum(scores) / len(scores), contract_id))
        elif scores:
            suggestions.append(
                {
                    "contract_id": contract_id,
                    "similarity": round(max(scores), 6),
                    "canonical": False,
                }
            )
    if len(candidates) == 1:
        return candidates[0][1], sorted(
            suggestions, key=lambda item: (-item["similarity"], item["contract_id"])
        )[:3]
    suggestions.extend(
        {
            "contract_id": contract_id,
            "similarity": round(score, 6),
            "canonical": False,
        }
        for score, contract_id in candidates
    )
    return None, sorted(
        suggestions, key=lambda item: (-item["similarity"], item["contract_id"])
    )[:3]


def _assignment_plan(
    claims: list[ClaimSource],
    existing_membership: dict[str, str],
) -> tuple[dict[str, str], dict[str, list[dict[str, Any]]], int]:
    sources = {claim.claim_id: claim for claim in claims}
    membership = {
        claim_id: contract_id
        for claim_id, contract_id in existing_membership.items()
        if claim_id in sources
    }
    suggestions: dict[str, list[dict[str, Any]]] = {}
    ambiguous = 0
    if not membership:
        grouped: dict[str, list[ClaimSource]] = defaultdict(list)
        for claim in claims:
            grouped[claim.family_id].append(claim)
        for family_claims in grouped.values():
            anchor = min(family_claims, key=lambda item: (item.queue_order, item.claim_id))
            contract_id = contract_id_for_anchor(anchor.claim_id)
            for claim in family_claims:
                membership[claim.claim_id] = contract_id
        return membership, suggestions, ambiguous

    for claim in sorted(claims, key=lambda item: (item.queue_order, item.claim_id)):
        if claim.claim_id in membership:
            continue
        contract_id, claim_suggestions = _strict_assignment(claim, membership, sources)
        if contract_id is None:
            contract_id = contract_id_for_anchor(claim.claim_id)
            ambiguous += bool(claim_suggestions)
        membership[claim.claim_id] = contract_id
        if claim_suggestions:
            suggestions[claim.claim_id] = claim_suggestions
    return membership, suggestions, ambiguous


def _full_read_index(
    envelopes: Iterable[EvidenceEventV2],
) -> dict[str, EvidenceEventV2]:
    result: dict[str, EvidenceEventV2] = {}
    for envelope in envelopes:
        if envelope.stream != "addressing":
            continue
        if envelope.payload.get("event_type") == "full_read":
            introspection_id = str(envelope.payload.get("introspection_id") or "")
            if introspection_id:
                result[introspection_id] = envelope
    return result


def _node_event(
    node: dict[str, Any],
    edges: list[dict[str, Any]],
    source: EvidenceEventV2,
    *,
    authority_state: str,
) -> dict[str, Any]:
    contract_id = str(node["contract_id"])
    return _event(
        "felt_contract_node_recorded",
        "felt_contract",
        contract_id,
        f"felt_contract_node:{node['node_id']}:{source.event_sha256}",
        authority_state=authority_state,
        contract_id=contract_id,
        node=node,
        edges=edges,
        source_event_ref=_source_event_ref(source),
    )


def _edge_record(
    contract_id: str,
    parent_node_id: str,
    child_node_id: str,
    relation: str,
    source: EvidenceEventV2,
    occurred_at: str,
) -> dict[str, Any]:
    return build_edge(
        edge_id=edge_id(parent_node_id, child_node_id, relation, source.event_id),
        contract_id=contract_id,
        source_node_id=parent_node_id,
        target_node_id=child_node_id,
        relation=relation,
        source_event_id=source.event_id,
        occurred_at=occurred_at,
        causal_parent=relation
        not in {
            "related_to",
            "context_exactly_observed_by",
            "context_temporally_associated_with",
            "context_witness_gap_for",
            "context_unresolved_for",
        },
    ).to_dict()


def _technical_disposition(status: str) -> str:
    normalized = status.strip().lower()
    if normalized in {"ready_for_implementation", "needs_sandbox", "needs_steward_grant"}:
        return TechnicalDispositionV1.READY.value
    if normalized in {"needs_operator_approval", "approval_required_live_trial"}:
        return TechnicalDispositionV1.GATED.value
    if normalized == "implemented_awaiting_felt_response":
        return TechnicalDispositionV1.IMPLEMENTED.value
    if normalized == "verified_existing":
        return TechnicalDispositionV1.VERIFIED.value
    if normalized == "closed_no_action":
        return TechnicalDispositionV1.TERMINAL_NO_ACTION.value
    if normalized == "superseded":
        return TechnicalDispositionV1.SUPERSEDED.value
    if normalized == "closed_felt_confirmed":
        return TechnicalDispositionV1.VERIFIED.value
    return TechnicalDispositionV1.UNASSESSED.value


def _review_outcome(value: str) -> str:
    normalized = value.strip().lower()
    aliases = {
        "resolved": FeltReviewOutcomeV1.FELT_CONFIRMED.value,
        "improved_named": FeltReviewOutcomeV1.IMPROVED_NAMED.value,
        "still_friction": FeltReviewOutcomeV1.STILL_FRICTION.value,
        "contradicted": FeltReviewOutcomeV1.CONTRADICTED.value,
        "objection": FeltReviewOutcomeV1.OBJECTION.value,
        "no_response": FeltReviewOutcomeV1.NO_RESPONSE.value,
        "awaiting": FeltReviewOutcomeV1.AWAITING.value,
        "not_requested": FeltReviewOutcomeV1.NOT_REQUESTED.value,
    }
    return aliases.get(normalized, FeltReviewOutcomeV1.NOT_REQUESTED.value)


def _collect_exact_refs(value: Any) -> tuple[set[str], set[str]]:
    work_items: set[str] = set()
    claims: set[str] = set()

    def walk(item: Any, parent: dict[str, Any] | None = None) -> None:
        if isinstance(item, list):
            for child in item:
                walk(child, parent)
            return
        if not isinstance(item, dict):
            return
        work_id = item.get("source_work_item_id") or item.get("work_item_id")
        if isinstance(work_id, str) and work_id.startswith("wi_"):
            work_items.add(work_id)
        canonical = item.get("canonical_claim_id")
        if isinstance(canonical, str) and ":" in canonical:
            claims.add(canonical)
        introspection_id = item.get("source_introspection_id") or item.get(
            "introspection_id"
        )
        claim_id = item.get("claim_id")
        if (
            isinstance(introspection_id, str)
            and introspection_id.startswith("introspection_")
            and isinstance(claim_id, str)
            and claim_id.startswith("c")
        ):
            claims.add(f"{introspection_id}:{claim_id}")
        for child in item.values():
            walk(child, item)

    walk(value)
    return work_items, claims


def _change_refs(receipt: dict[str, Any]) -> dict[str, set[str]]:
    result: dict[str, set[str]] = defaultdict(set)
    values = receipt.get("change_refs")
    if not isinstance(values, list):
        return result
    for value in values:
        if not isinstance(value, dict):
            continue
        kind = str(value.get("kind") or "")
        identifier = str(value.get("id") or "")
        if kind and identifier:
            result[kind].add(identifier)
    return result


def build_source_events(
    workspace: Path,
    *,
    existing_graph_envelopes: Iterable[EvidenceEventV2] = (),
    existing_membership: dict[str, str] | None = None,
    existing_contract_ids: set[str] | None = None,
    existing_implementation_nodes: dict[tuple[str, str], str] | None = None,
    source_envelopes: Iterable[EvidenceEventV2] | None = None,
    source_watermarks: dict[str, dict[str, Any]] | None = None,
) -> SourceBuild:
    existing_graph_envelopes = tuple(existing_graph_envelopes)
    store = EvidenceEventStore(store_root(workspace))
    if source_envelopes is None:
        selected: list[EvidenceEventV2] = []
        for stream in SOURCE_STREAMS:
            stream_events, corrupt = store.envelopes_for_stream(stream)
            if corrupt:
                raise ValueError(f"V2 store has corrupt events in {stream}")
            selected.extend(stream_events)
        source_envelopes = sorted(selected, key=lambda envelope: envelope.global_seq)
    else:
        source_envelopes = sorted(
            (
                envelope
                for envelope in source_envelopes
                if envelope.stream in SOURCE_STREAMS
            ),
            key=lambda envelope: envelope.global_seq,
        )
    source_by_stream: dict[str, list[EvidenceEventV2]] = defaultdict(list)
    for envelope in source_envelopes:
        source_by_stream[envelope.stream].append(envelope)

    claims, family_status_sha256 = _claim_sources(workspace)
    claim_sources = {claim.claim_id: claim for claim in claims}
    if existing_membership is None or existing_contract_ids is None:
        replay_membership, replay_contracts = _current_graph_membership(
            existing_graph_envelopes
        )
        existing_membership = (
            replay_membership
            if existing_membership is None
            else existing_membership
        )
        existing_contract_ids = (
            set(replay_contracts)
            if existing_contract_ids is None
            else existing_contract_ids
        )
    membership, suggestions, ambiguous = _assignment_plan(
        claims, existing_membership
    )
    contracts: dict[str, list[ClaimSource]] = defaultdict(list)
    for claim in claims:
        contracts[membership[claim.claim_id]].append(claim)

    events: list[dict[str, Any]] = []
    contract_created_at: dict[str, str] = {}
    family_assignment_envelopes: dict[str, EvidenceEventV2] = {}
    for envelope in source_by_stream["claim_families"]:
        payload = envelope.payload
        if payload.get("event_type") in {
            "claim_family_membership_assigned",
            "claim_family_membership_corrected",
        }:
            claim_id = str(payload.get("canonical_claim_id") or "")
            if claim_id:
                family_assignment_envelopes[claim_id] = envelope

    for contract_id, contract_claims in sorted(contracts.items()):
        anchor = min(contract_claims, key=lambda item: (item.queue_order, item.claim_id))
        source = family_assignment_envelopes.get(anchor.claim_id)
        created_at = source.recorded_at if source else datetime.now(UTC).isoformat()
        contract_created_at[contract_id] = created_at
        authority_state = (
            "approval_pending"
            if any(
                claim.authority_class.startswith("approval_pending")
                for claim in contract_claims
            )
            else "evidence_only"
        )
        if contract_id not in existing_contract_ids:
            contract = build_contract(
                contract_id=contract_id,
                anchor_claim_id=anchor.claim_id,
                created_at=created_at,
                authority_state=authority_state,
            ).to_dict()
            events.append(
                _event(
                    "felt_contract_created",
                    "felt_contract",
                    contract_id,
                    f"felt_contract_created:{contract_id}",
                    authority_state=authority_state,
                    contract=contract,
                    source_family_id=anchor.family_id,
                )
            )

    full_reads = _full_read_index(source_envelopes)
    claim_nodes: dict[str, str] = {}
    signal_nodes: dict[tuple[str, str], str] = {}
    routed_source_ids: set[str] = set()

    for claim in sorted(claims, key=lambda item: (item.queue_order, item.claim_id)):
        contract_id = membership[claim.claim_id]
        authority_state = (
            "approval_pending"
            if claim.authority_class.startswith("approval_pending")
            else "evidence_only"
        )
        assignment_source = family_assignment_envelopes.get(claim.claim_id)
        source_id = (
            assignment_source.event_id
            if assignment_source is not None
            else f"claim_status_{claim.record_sha256[:20]}"
        )
        events.append(
            _event(
                "felt_contract_claim_assigned",
                "felt_contract",
                contract_id,
                f"felt_contract_claim_assignment:{claim.claim_id}:{contract_id}",
                authority_state=authority_state,
                contract_id=contract_id,
                canonical_claim_id=claim.claim_id,
                claim_ref={
                    "canonical_claim_id": claim.claim_id,
                    "introspection_id": claim.introspection_id,
                    "claim_id": claim.local_claim_id,
                    "canonical_claim_record_sha256": claim.record_sha256,
                    "source_sha256": claim.source_sha256,
                    "source_path_ref": claim.source_path_ref,
                    "queue_order": claim.queue_order[0],
                    "private_content_copied": False,
                },
                match_classes={
                    "authority_class": claim.authority_class,
                    "target_surface": claim.target_surface,
                    "requested_outcome": claim.requested_outcome,
                    "polarity": claim.polarity,
                },
                noncanonical_suggestions=suggestions.get(claim.claim_id, []),
                propagation={
                    "closure": False,
                    "evidence_sufficiency": False,
                    "supersession": False,
                    "authority": False,
                },
                source_event_ref=(
                    _source_event_ref(assignment_source)
                    if assignment_source is not None
                    else {"event_id": source_id, "stream": "claim_family_projection"}
                ),
            )
        )
        full_read = full_reads.get(claim.introspection_id)
        if full_read is None:
            continue
        routed_source_ids.add(full_read.event_id)
        occurred_at = _event_time(full_read)
        signal_key = (contract_id, claim.introspection_id)
        signal_node_id = signal_nodes.get(signal_key)
        if signal_node_id is None:
            signal_node_id = node_id(
                full_read.event_id,
                f"felt_signal:{claim.introspection_id}",
                contract_id,
            )
            signal_nodes[signal_key] = signal_node_id
            signal_ref = build_signal_ref(
                source_kind="canonical_introspection",
                source_id=claim.introspection_id,
                canonical_sha256=claim.source_sha256,
                owner=_owner_for_introspection(claim.introspection_id),
                observed_at=occurred_at,
                field_paths=(f"claims.{claim.local_claim_id}",),
            )
            signal_node = build_node(
                node_id=signal_node_id,
                contract_id=contract_id,
                kind="felt_signal",
                source_event_id=full_read.event_id,
                occurred_at=occurred_at,
                source_ref=signal_ref,
                metadata={
                    "summary_sha256": str(
                        full_read.payload.get("summary_sha256")
                        or sha256_canonical(full_read.payload.get("summary_excerpt"))
                    ),
                    "private_content_copied": False,
                },
                authority_state="evidence_only",
            ).to_dict()
            events.append(
                _node_event(
                    signal_node,
                    [],
                    full_read,
                    authority_state="evidence_only",
                )
            )
        claim_node_id = node_id(
            full_read.event_id,
            f"claim:{claim.claim_id}",
            contract_id,
        )
        claim_nodes[claim.claim_id] = claim_node_id
        claim_node = build_node(
            node_id=claim_node_id,
            contract_id=contract_id,
            kind="claim",
            source_event_id=full_read.event_id,
            occurred_at=occurred_at,
            source_ref=None,
            metadata={
                "canonical_claim_id": claim.claim_id,
                "canonical_claim_record_sha256": claim.record_sha256,
                "disposition": claim.disposition,
                "classification": claim.classification,
                "private_content_copied": False,
            },
            authority_state=authority_state,
        ).to_dict()
        events.append(
            _node_event(
                claim_node,
                [
                    _edge_record(
                        contract_id,
                        signal_node_id,
                        claim_node_id,
                        "contains_claim",
                        full_read,
                        occurred_at,
                    )
                ],
                full_read,
                authority_state=authority_state,
            )
        )

    from .source_history import route_history

    work_contracts, latest_node_by_work = route_history(
        source_by_stream=source_by_stream,
        membership=membership,
        claims=claims,
        claim_sources=claim_sources,
        claim_nodes=claim_nodes,
        signal_nodes=signal_nodes,
        events=events,
        routed_source_ids=routed_source_ids,
    )

    from .source_deployments import route_deployments

    (
        environment_receipts_sha256,
        environment_receipt_count,
        deployment_node_count,
        temporal_deployment_count,
    ) = route_deployments(
        workspace=workspace,
        source_by_stream=source_by_stream,
        existing_graph_envelopes=existing_graph_envelopes,
        existing_implementation_nodes=existing_implementation_nodes,
        membership=membership,
        claim_sources=claim_sources,
        claim_nodes=claim_nodes,
        work_contracts=work_contracts,
        latest_node_by_work=latest_node_by_work,
        contracts=contracts,
        contract_created_at=contract_created_at,
        events=events,
    )

    canonical_source_counts = {
        stream: int(
            (source_watermarks or {}).get(stream, {}).get(
                "stream_seq",
                len(source_by_stream[stream]),
            )
        )
        for stream in SOURCE_STREAMS
    }
    canonical_source_total = sum(canonical_source_counts.values())
    migration_receipt = {
        "schema": "felt_contract_migration_receipt_v1",
        "schema_version": 1,
        "claim_family_status_sha256": family_status_sha256,
        "claim_count": len(claims),
        "contract_count": len(contracts),
        "singleton_contract_count": sum(
            1 for contract_claims in contracts.values() if len(contract_claims) == 1
        ),
        "multi_claim_contract_count": sum(
            1 for contract_claims in contracts.values() if len(contract_claims) > 1
        ),
        "ambiguous_new_claim_count": ambiguous,
        "routed_source_event_count": len(routed_source_ids),
        "unrouted_source_event_count": (
            canonical_source_total - len(routed_source_ids)
        ),
        "environment_receipt_count": environment_receipt_count,
        "deployment_node_count": deployment_node_count,
        "temporal_deployment_node_count": temporal_deployment_count,
        "source_stream_counts": canonical_source_counts,
        "counter_audit": {
            "every_claim_assigned_once": len(membership) == len(claims),
            "claim_ids_unique": len({claim.claim_id for claim in claims}) == len(claims),
            "contract_ids_stable": all(
                contract_id.startswith("contract_") for contract_id in contracts
            ),
            "family_membership_does_not_propagate_closure": True,
            "family_membership_does_not_propagate_evidence": True,
            "family_membership_does_not_propagate_supersession": True,
            "family_membership_does_not_propagate_authority": True,
            "private_content_not_copied": True,
        },
        "source_watermarks": (
            source_watermarks
            if source_watermarks is not None
            else store.stream_watermarks(SOURCE_STREAMS)
        ),
        "source_hashes": {
            "claim_family_status": family_status_sha256,
            "environment_receipts": environment_receipts_sha256,
        },
        "artifact_authority_state_v1": _authority(),
    }
    events.append(
        _event(
            "felt_contract_migration_completed",
            "felt_contract_migration",
            family_status_sha256[:24],
            f"felt_contract_migration:{family_status_sha256}",
            migration_receipt=migration_receipt,
        )
    )
    return SourceBuild(
        events=tuple(events),
        claim_count=len(claims),
        contract_count=len(contracts),
        source_counts={
            stream: canonical_source_counts[stream] for stream in SOURCE_STREAMS
        },
        routed_source_events=len(routed_source_ids),
        unrouted_source_events=canonical_source_total - len(routed_source_ids),
        ambiguous_new_claims=ambiguous,
        source_watermarks=(
            source_watermarks
            if source_watermarks is not None
            else store.stream_watermarks(SOURCE_STREAMS)
        ),
        source_hashes={
            "claim_family_status": family_status_sha256,
            "environment_receipts": environment_receipts_sha256,
        },
    )
