"""Streaming canonical verification for Evidence Event Store V2."""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
import json
from pathlib import Path
from typing import Any, Callable

try:
    from authority_state import (
        ArtifactAuthorityStateV1,
        assert_artifact_authority_tree,
    )
except ModuleNotFoundError:
    from scripts.authority_state import (
        ArtifactAuthorityStateV1,
        assert_artifact_authority_tree,
    )

from .model import EVENT_SCHEMA, EvidenceEventV2, GENESIS_HASH


@dataclass(frozen=True)
class StoreVerification:
    valid: bool
    event_count: int
    stream_counts: dict[str, int]
    corrupt_lines: int
    errors: tuple[str, ...]
    last_global_seq: int
    last_event_sha256: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "evidence_event_store_verification_v1",
            "schema_version": 1,
            "valid": self.valid,
            "event_count": self.event_count,
            "stream_counts": self.stream_counts,
            "corrupt_lines": self.corrupt_lines,
            "errors": list(self.errors),
            "last_global_seq": self.last_global_seq,
            "last_event_sha256": self.last_event_sha256,
        }


def verify_canonical_events(
    events_path: Path,
    read_head: Callable[[], dict[str, Any]],
    empty_head: Callable[[], dict[str, Any]],
) -> StoreVerification:
    """Verify the full canonical chain without retaining parsed event payloads."""

    errors: list[str] = []
    stream_counts: Counter[str] = Counter()
    expected_previous = GENESIS_HASH
    expected_global_seq = 1
    stream_sequences: Counter[str] = Counter()
    seen_ids: set[str] = set()
    seen_idempotency: set[tuple[str, str]] = set()
    corrupt = 0
    event_count = 0
    last_seq = 0
    last_hash = GENESIS_HASH
    if events_path.is_file():
        with events_path.open("r", encoding="utf-8", errors="strict") as handle:
            for line in handle:
                if not line.strip():
                    continue
                try:
                    value = json.loads(line)
                    if not isinstance(value, dict):
                        raise ValueError("event line is not an object")
                    event = EvidenceEventV2.from_dict(value)
                except (json.JSONDecodeError, TypeError, ValueError):
                    corrupt += 1
                    continue
                event_count += 1
                stream_counts[event.stream] += 1
                stream_sequences[event.stream] += 1
                prefix = f"event[{event.global_seq}]"
                if event.event_id in seen_ids:
                    errors.append(
                        f"{prefix}:duplicate_event_id:{event.event_id}"
                    )
                seen_ids.add(event.event_id)
                if event.global_seq != expected_global_seq:
                    errors.append(
                        f"{prefix}:global_seq_expected_"
                        f"{expected_global_seq}_got_{event.global_seq}"
                    )
                if event.stream_seq != stream_sequences[event.stream]:
                    errors.append(
                        f"{prefix}:stream_seq_expected_"
                        f"{stream_sequences[event.stream]}_got_"
                        f"{event.stream_seq}"
                    )
                if event.previous_event_sha256 != expected_previous:
                    errors.append(f"{prefix}:previous_hash_mismatch")
                if event.event_sha256 != event.calculated_sha256():
                    errors.append(f"{prefix}:event_hash_mismatch")
                if event.idempotency_key:
                    identity = (event.stream, event.idempotency_key)
                    if identity in seen_idempotency:
                        errors.append(f"{prefix}:duplicate_idempotency_key")
                    seen_idempotency.add(identity)
                try:
                    state = str(
                        event.artifact_authority_state_v1.get("state") or ""
                    )
                    ArtifactAuthorityStateV1(state)
                    assert_artifact_authority_tree(event.payload)
                except (ValueError, TypeError) as error:
                    errors.append(f"{prefix}:authority:{error}")
                if event.to_dict().get("schema") != EVENT_SCHEMA:
                    errors.append(f"{prefix}:schema_mismatch")
                expected_previous = event.event_sha256
                expected_global_seq += 1
                last_seq = event.global_seq
                last_hash = event.event_sha256
    if corrupt:
        errors.append(f"corrupt_lines:{corrupt}")
    try:
        head = read_head()
    except Exception as error:
        errors.append(str(error))
        head = empty_head()
    if int(head.get("last_global_seq") or 0) != last_seq:
        errors.append("head_last_global_seq_mismatch")
    if str(head.get("last_event_sha256") or GENESIS_HASH) != last_hash:
        errors.append("head_last_event_sha256_mismatch")
    expected_streams = {
        key: int(value) for key, value in stream_counts.items()
    }
    head_streams = {
        str(key): int(value)
        for key, value in (head.get("stream_sequences") or {}).items()
    }
    if head_streams != expected_streams:
        errors.append("head_stream_sequences_mismatch")
    return StoreVerification(
        valid=not errors,
        event_count=event_count,
        stream_counts=dict(sorted(stream_counts.items())),
        corrupt_lines=corrupt,
        errors=tuple(errors),
        last_global_seq=last_seq,
        last_event_sha256=last_hash,
    )
