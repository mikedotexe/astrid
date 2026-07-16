"""Deterministic migration from the existing domain JSONL logs."""

from __future__ import annotations

import hashlib
import heapq
import json
import os
import time
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any

try:
    from authority_state import ArtifactAuthorityStateV1
except ModuleNotFoundError:
    from scripts.authority_state import ArtifactAuthorityStateV1

from .model import (
    GENESIS_HASH,
    EvidenceEventV2,
    ProvenanceSourceV1,
    iso_from_unix_seconds,
)
from .store import (
    DEFAULT_ACTOR,
    EvidenceEventStore,
    EvidenceStoreError,
    _aggregate_for_payload,
    _atomic_write_json,
    _normalized_payload,
)

MIGRATION_SCHEMA = "evidence_event_store_migration_receipt_v1"


@dataclass(frozen=True)
class LegacyEventSource:
    stream: str
    path: Path


@dataclass(frozen=True)
class _LegacyLine:
    source_index: int
    stream: str
    source_path: Path
    source_sha256: str
    line_number: int
    raw_sha256: str
    timestamp: float
    payload: dict[str, Any]


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _legacy_lines(source: LegacyEventSource, source_index: int) -> tuple[list[_LegacyLine], int]:
    if not source.path.is_file():
        return [], 0
    source_sha256 = _sha256_file(source.path)
    lines: list[_LegacyLine] = []
    corrupt = 0
    with source.path.open("r", encoding="utf-8", errors="strict") as handle:
        for line_number, raw in enumerate(handle, start=1):
            if not raw.strip():
                continue
            try:
                payload = json.loads(raw)
            except json.JSONDecodeError:
                corrupt += 1
                continue
            if not isinstance(payload, dict):
                corrupt += 1
                continue
            raw_sha256 = hashlib.sha256(raw.rstrip("\n").encode("utf-8")).hexdigest()
            raw_ts = payload.get("ts")
            try:
                timestamp = float(raw_ts)
            except (TypeError, ValueError):
                timestamp = float(source_index)
            if not (timestamp >= 0.0):
                timestamp = float(source_index)
            lines.append(
                _LegacyLine(
                    source_index=source_index,
                    stream=source.stream,
                    source_path=source.path,
                    source_sha256=source_sha256,
                    line_number=line_number,
                    raw_sha256=raw_sha256,
                    timestamp=timestamp,
                    payload=payload,
                )
            )
    return lines, corrupt


def _stable_k_way_merge(streams: list[list[_LegacyLine]]) -> list[_LegacyLine]:
    """Approximate chronology without ever reordering one source stream."""

    heap: list[tuple[float, int, int, int]] = []
    for source_index, lines in enumerate(streams):
        if lines:
            first = lines[0]
            heapq.heappush(
                heap,
                (first.timestamp, source_index, first.line_number, 0),
            )
    merged: list[_LegacyLine] = []
    while heap:
        _timestamp, source_index, _line_number, position = heapq.heappop(heap)
        line = streams[source_index][position]
        merged.append(line)
        next_position = position + 1
        if next_position < len(streams[source_index]):
            next_line = streams[source_index][next_position]
            heapq.heappush(
                heap,
                (
                    next_line.timestamp,
                    source_index,
                    next_line.line_number,
                    next_position,
                ),
            )
    return merged


def _projection_hashes(paths: dict[str, Path] | None) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for name, path in sorted((paths or {}).items()):
        result[name] = {
            "path": str(path),
            "exists": path.is_file(),
            "sha256": _sha256_file(path) if path.is_file() else None,
        }
    return result


def _render_receipt(receipt: dict[str, Any]) -> str:
    verification = receipt["verification"]
    lines = [
        "# Evidence Event Store V2 Migration Receipt",
        "",
        f"- status: {receipt['status']}",
        f"- imported_events: {verification['event_count']}",
        f"- corrupt_legacy_lines: {receipt['corrupt_legacy_lines']}",
        f"- last_global_seq: {verification['last_global_seq']}",
        f"- last_event_sha256: {verification['last_event_sha256']}",
        "- authority: witness-only; migration grants no approval or live authority",
        "",
        "## Streams",
        "",
    ]
    for stream, count in verification["stream_counts"].items():
        lines.append(f"- {stream}: {count}")
    lines.extend(["", "## Sources", ""])
    for source in receipt["sources"]:
        lines.append(
            f"- {source['stream']}: lines={source['event_count']} sha256={source['sha256']} path={source['path']}"
        )
    return "\n".join(lines) + "\n"


def import_legacy_sources(
    store: EvidenceEventStore,
    sources: list[LegacyEventSource],
    *,
    projection_paths: dict[str, Path] | None = None,
    counter_audits: dict[str, Any] | None = None,
    write: bool,
) -> dict[str, Any]:
    source_lines: list[list[_LegacyLine]] = []
    source_records: list[dict[str, Any]] = []
    corrupt_total = 0
    for index, source in enumerate(sources):
        lines, corrupt = _legacy_lines(source, index)
        source_lines.append(lines)
        corrupt_total += corrupt
        source_records.append(
            {
                "stream": source.stream,
                "path": str(source.path),
                "exists": source.path.is_file(),
                "event_count": len(lines),
                "corrupt_lines": corrupt,
                "sha256": _sha256_file(source.path) if source.path.is_file() else None,
            }
        )
    if corrupt_total:
        raise EvidenceStoreError(
            f"legacy import refused because {corrupt_total} corrupt line(s) were found"
        )

    merged = _stable_k_way_merge(source_lines)
    stream_sequences: Counter[str] = Counter()
    previous_hash = GENESIS_HASH
    envelopes: list[EvidenceEventV2] = []
    for global_seq, legacy in enumerate(merged, start=1):
        stream_sequences[legacy.stream] += 1
        payload, authority = _normalized_payload(legacy.payload)
        identity_material = (
            f"{legacy.source_sha256}:{legacy.line_number}:{legacy.raw_sha256}"
        ).encode("utf-8")
        event_id = "legacy_" + hashlib.sha256(identity_material).hexdigest()[:32]
        envelope = EvidenceEventV2(
            event_id=event_id,
            global_seq=global_seq,
            stream=legacy.stream,
            stream_seq=stream_sequences[legacy.stream],
            event_type=str(payload.get("event_type") or "unknown"),
            recorded_at=iso_from_unix_seconds(legacy.timestamp),
            actor=str(payload.get("reader") or payload.get("actor") or "legacy-import"),
            aggregate=_aggregate_for_payload(payload),
            correlation_id=(
                str(payload["correlation_id"]) if payload.get("correlation_id") else None
            ),
            causation_id=(
                str(payload["causation_id"]) if payload.get("causation_id") else None
            ),
            artifact_authority_state_v1=authority,
            source=ProvenanceSourceV1(
                kind="legacy_jsonl_import",
                locator=str(legacy.source_path),
                sha256=legacy.source_sha256,
                line_number=legacy.line_number,
            ),
            payload=payload,
            idempotency_key=(
                f"legacy:{legacy.stream}:{legacy.source_sha256}:{legacy.line_number}"
            ),
            previous_event_sha256=previous_hash,
        )
        envelope = EvidenceEventV2(
            **{
                **envelope.__dict__,
                "event_sha256": envelope.calculated_sha256(),
            }
        )
        previous_hash = envelope.event_sha256
        envelopes.append(envelope)

    preview_verification = {
        "valid": True,
        "event_count": len(envelopes),
        "stream_counts": dict(sorted(stream_sequences.items())),
        "corrupt_lines": 0,
        "errors": [],
        "last_global_seq": len(envelopes),
        "last_event_sha256": previous_hash,
    }
    receipt: dict[str, Any] = {
        "schema": MIGRATION_SCHEMA,
        "schema_version": 1,
        "status": "preview",
        "write": write,
        "created_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "store_root": str(store.root),
        "sources": source_records,
        "corrupt_legacy_lines": corrupt_total,
        "projection_hashes": _projection_hashes(projection_paths),
        "counter_audits": counter_audits or {},
        "verification": preview_verification,
        "artifact_authority_state_v1": (
            ArtifactAuthorityStateV1.evidence_only().canonical_record()
        ),
        "authority": "migration_witness_only_not_approval_or_live_control",
        "actor": DEFAULT_ACTOR,
    }
    if not write:
        return receipt

    verification = store.initialize_from_envelopes(envelopes, legacy_imported=True)
    receipt["status"] = "passed" if verification.valid else "failed"
    receipt["verification"] = verification.to_dict()
    _atomic_write_json(store.root / "migration_receipt.json", receipt)
    markdown_path = store.root / "migration_receipt.md"
    tmp = markdown_path.with_name(
        f".{markdown_path.name}.{os.getpid()}.{time.time_ns()}.tmp"
    )
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(_render_receipt(receipt))
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(tmp, markdown_path)
    return receipt
