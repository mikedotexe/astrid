"""Incremental, rebuildable orchestration for the felt-contract projector."""

from __future__ import annotations

import hashlib
from itertools import chain
import json
from pathlib import Path
from typing import Any

try:
    from evidence_store import EvidenceEventStore, EvidenceStoreError
    from evidence_store.adapter import append_domain_events
except ModuleNotFoundError:
    from scripts.evidence_store import EvidenceEventStore, EvidenceStoreError
    from scripts.evidence_store.adapter import append_domain_events

from .projector import (
    GRAPH_INPUT_STREAMS,
    PROJECTOR_VERSION,
    GraphProjectionError,
    payload_hashes,
    project_graph,
    projection_payloads,
    write_projection,
)
from .sources import (
    SOURCE_STREAMS,
    build_source_events,
    claim_family_semantic_sha256,
    graph_state_dir,
    store_root,
)
from .state_index import FeltContractStateIndex


GRAPH_STREAM = "felt_contracts"


def _atomic_source_hash(path: Path) -> str:
    if not path.is_file():
        return hashlib.sha256(b"").hexdigest()
    digest_value = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest_value.update(chunk)
    return digest_value.hexdigest()


def _graph_envelopes(store: EvidenceEventStore) -> list[Any]:
    envelopes, corrupt = store.envelopes_for_stream(GRAPH_STREAM)
    if corrupt:
        raise EvidenceStoreError("V2 store has corrupt felt-contract events")
    return envelopes


def _existing_projection(
    workspace: Path,
) -> tuple[dict[str, Any], list[Any]]:
    store = EvidenceEventStore(store_root(workspace))
    verification = store.verify()
    if not verification.valid:
        raise EvidenceStoreError(
            "invalid V2 store: " + "; ".join(verification.errors)
        )
    state = FeltContractStateIndex(graph_state_dir(workspace))
    projection = state.load_projection()
    state_watermark = state.watermark(GRAPH_STREAM)
    canonical_watermark = int(
        store.stream_watermarks((GRAPH_STREAM,))
        .get(GRAPH_STREAM, {})
        .get("stream_seq", 0)
    )
    if projection is not None and state_watermark == canonical_watermark:
        return projection, state.graph_envelopes()
    envelopes = _graph_envelopes(store)
    return (
        project_graph(
            (envelope.payload for envelope in envelopes),
            workspace=workspace,
        ),
        envelopes,
    )


def _output_source_hashes(
    workspace: Path,
    source_hashes: dict[str, str],
) -> dict[str, str]:
    receipt_log = (
        workspace / "environment_receipts/environment_receipts.jsonl"
    )
    return {
        **source_hashes,
        "environment_receipts": _atomic_source_hash(receipt_log),
    }


def _source_file_hashes(workspace: Path) -> dict[str, str]:
    claim_status_path = (
        workspace / "diagnostics/claim_families_v1/status.json"
    )
    claim_semantic_hash = hashlib.sha256(b"").hexdigest()
    if claim_status_path.is_file():
        try:
            claim_semantic_hash = claim_family_semantic_sha256(workspace)
        except (OSError, json.JSONDecodeError, ValueError):
            pass
    return {
        "claim_family_status": claim_semantic_hash,
        "environment_receipts": _atomic_source_hash(
            workspace / "environment_receipts/environment_receipts.jsonl"
        ),
    }


def _existing_projection_hashes(workspace: Path) -> dict[str, str]:
    root = graph_state_dir(workspace)
    return {
        name: _atomic_source_hash(root / name)
        for name in (
            "status.json",
            "contracts.jsonl",
            "report.md",
            "migration_receipt.json",
        )
    }


def _source_counters(source_build: Any) -> dict[str, Any]:
    return {
        "source_stream_counts": source_build.source_counts,
        "routed_source_event_count": source_build.routed_source_events,
        "unrouted_source_event_count": source_build.unrouted_source_events,
        "ambiguous_new_claim_count": source_build.ambiguous_new_claims,
    }


def _sync_derived_events(
    store: EvidenceEventStore,
    state: FeltContractStateIndex,
    *,
    full_rebuild: bool,
) -> tuple[int, int, int]:
    state.initialize(replace=full_rebuild)
    source_added = 0
    source_effects = 0
    for stream in SOURCE_STREAMS:
        added, effects = state.ingest_source_events_with_effects(
            store.iter_envelopes_for_stream(
                stream,
                after_stream_seq=state.watermark(stream),
            )
        )
        source_added += added
        source_effects += effects
    graph_added = state.ingest_graph_events(
        store.iter_envelopes_for_stream(
            GRAPH_STREAM,
            after_stream_seq=state.watermark(GRAPH_STREAM),
        )
    )
    return source_added, source_effects, graph_added


def _reference_projection(
    store: EvidenceEventStore,
    workspace: Path,
) -> tuple[dict[str, Any], list[Any]]:
    envelopes = _graph_envelopes(store)
    return (
        project_graph(
            (envelope.payload for envelope in envelopes),
            workspace=workspace,
        ),
        envelopes,
    )


def generate(
    workspace: Path,
    *,
    write: bool,
    actor: str,
    full_replay: bool = False,
) -> dict[str, Any]:
    store = EvidenceEventStore(store_root(workspace))
    verification_before = store.verify_indexed_tail()
    if not verification_before.valid:
        raise EvidenceStoreError(
            "invalid V2 store: " + "; ".join(verification_before.errors)
        )
    state = FeltContractStateIndex(graph_state_dir(workspace))
    current_source_hashes = _source_file_hashes(workspace)
    source_added = 0
    source_effects = 0
    graph_added = 0
    cached_status: dict[str, Any] | None = None
    cached_hashes: dict[str, str] = {}
    cached_counters: dict[str, Any] = {}
    if write:
        source_added, source_effects, graph_added = _sync_derived_events(
            store,
            state,
            full_rebuild=full_replay,
        )
        cached_status = state.cached_status()
        cached_hashes, cached_counters = state.source_metadata()
    else:
        existing_envelopes = _graph_envelopes(store)
        indexed_source_envelopes = []
        for stream in SOURCE_STREAMS:
            envelopes, corrupt = store.envelopes_for_stream(stream)
            if corrupt:
                raise EvidenceStoreError(
                    f"corrupt indexed source stream: {stream}"
                )
            indexed_source_envelopes.extend(envelopes)
        indexed_source_envelopes.sort(
            key=lambda envelope: envelope.global_seq
        )

    existing_event_count = (
        state.count("graph_events") if write else len(existing_envelopes)
    )
    unchanged = (
        write
        and not full_replay
        and source_effects == 0
        and graph_added == 0
        and cached_status is not None
        and cached_hashes == current_source_hashes
    )
    metadata_only = False
    if unchanged:
        source_build = None
        planned: list[dict[str, Any]] = []
        projection: dict[str, Any] | None = None
        status = cached_status
        counters = cached_counters
    else:
        if write:
            indexed_source_envelopes = state.source_envelopes(SOURCE_STREAMS)
        source_watermarks = {
            stream: watermark
            for stream, watermark in state.watermarks().items()
            if stream in SOURCE_STREAMS
        }
        source_build = build_source_events(
            workspace,
            existing_graph_envelopes=(
                existing_envelopes if not write else ()
            ),
            existing_membership=(
                state.current_membership()
                if write and cached_status
                else None
            ),
            existing_contract_ids=(
                state.current_contract_ids()
                if write and cached_status
                else None
            ),
            existing_implementation_nodes=(
                state.implementation_nodes()
                if write and cached_status
                else None
            ),
            source_envelopes=indexed_source_envelopes,
            source_watermarks=(
                source_watermarks
                if write
                else store.stream_watermarks(SOURCE_STREAMS)
            ),
        )
        del indexed_source_envelopes
        existing_keys = store.idempotency_keys(GRAPH_STREAM)
        planned = [
            event
            for event in source_build.events
            if str(event.get("idempotency_key") or "") not in existing_keys
        ]
        counters = _source_counters(source_build)
        metadata_only = bool(
            write
            and cached_status is not None
            and not planned
            and not full_replay
        )
        if metadata_only:
            projection = None
            status = cached_status
        else:
            if write:
                existing_envelopes = state.graph_envelopes()
            projection = project_graph(
                chain(
                    (
                        envelope.payload
                        for envelope in existing_envelopes
                    ),
                    planned,
                ),
                workspace=workspace,
            )
            hashes_for_receipt = payload_hashes(
                projection_payloads(projection)
            )
            for event in planned:
                if (
                    event.get("event_type")
                    == "felt_contract_migration_completed"
                ):
                    receipt = event.get("migration_receipt")
                    if isinstance(receipt, dict):
                        receipt["projection_hashes"] = hashes_for_receipt
            if hashes_for_receipt and planned:
                projection = project_graph(
                    chain(
                        (
                            envelope.payload
                            for envelope in existing_envelopes
                        ),
                        planned,
                    ),
                    workspace=workspace,
                )
            status = projection["status"]

    projection_hashes: dict[str, str] = {}
    checkpoint_path: str | None = None
    parity: dict[str, Any] | None = None
    if write:
        if unchanged:
            projection_hashes = _existing_projection_hashes(workspace)
        elif metadata_only:
            state.update_source_metadata(
                source_hashes=current_source_hashes,
                source_counters=counters,
            )
            projection_hashes = _existing_projection_hashes(workspace)
        else:
            append_domain_events(
                graph_state_dir(workspace),
                GRAPH_STREAM,
                planned,
                actor=actor,
            )
            graph_added += state.ingest_graph_events(
                store.iter_envelopes_for_stream(
                    GRAPH_STREAM,
                    after_stream_seq=state.watermark(GRAPH_STREAM),
                )
            )
            if projection is None:
                raise GraphProjectionError(
                    "changed generation has no projection"
                )
            state.materialize(
                projection,
                source_hashes=current_source_hashes,
                source_counters=counters,
            )
            projection_hashes = write_projection(workspace, projection)
        checkpoint = store.write_checkpoint(
            "felt_contract_graph_v1",
            PROJECTOR_VERSION,
            projection_hashes,
            input_streams=GRAPH_INPUT_STREAMS,
            source_hashes=_output_source_hashes(
                workspace,
                (
                    source_build.source_hashes
                    if source_build is not None
                    else current_source_hashes
                ),
            ),
        )
        checkpoint_path = str(checkpoint)
        if full_replay:
            if projection is None:
                raise GraphProjectionError("full replay has no projection")
            reference, _ = _reference_projection(store, workspace)
            incremental_payloads = projection_payloads(projection)
            reference_payloads = projection_payloads(reference)
            parity = {
                "exact": incremental_payloads == reference_payloads,
                "incremental_hashes": payload_hashes(incremental_payloads),
                "full_replay_hashes": payload_hashes(reference_payloads),
            }
            if not parity["exact"]:
                raise GraphProjectionError(
                    "incremental felt-contract state diverged "
                    "from full replay"
                )

    result = dict(status)
    result.update(
        {
            "write": write,
            "planned_event_count": len(planned),
            "existing_event_count": existing_event_count,
            **counters,
            "incremental_source_event_count": source_added,
            "incremental_relevant_source_event_count": source_effects,
            "incremental_graph_event_count": graph_added,
            "derived_state": state.status() if write else None,
            "full_replay": full_replay,
            "full_replay_parity": parity,
            "projection_hashes": projection_hashes,
            "checkpoint_path": checkpoint_path,
            "v2_before": {
                "global_seq": verification_before.last_global_seq,
                "head_sha256": verification_before.last_event_sha256,
            },
        }
    )
    if write:
        verification_after = store.verify_indexed_tail()
        result["v2_after"] = {
            "valid": verification_after.valid,
            "global_seq": verification_after.last_global_seq,
            "head_sha256": verification_after.last_event_sha256,
            "felt_contract_event_count": (
                verification_after.stream_counts.get(GRAPH_STREAM, 0)
            ),
        }
    return result
