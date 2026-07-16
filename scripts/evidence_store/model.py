"""Schema objects and canonical serialization for Evidence Event Store V2."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from datetime import UTC, datetime
from typing import Any

EVENT_SCHEMA = "evidence_event_v2"
EVENT_SCHEMA_VERSION = 2
GENESIS_HASH = "0" * 64


def canonical_json(value: Any) -> str:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    )


def sha256_canonical(value: Any) -> str:
    return hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest()


def iso_from_unix_seconds(value: float) -> str:
    return datetime.fromtimestamp(value, tz=UTC).isoformat()


@dataclass(frozen=True)
class ProvenanceSourceV1:
    """Bounded receipt for where an event entered the canonical store."""

    kind: str
    locator: str
    sha256: str | None = None
    line_number: int | None = None

    def to_dict(self) -> dict[str, Any]:
        value: dict[str, Any] = {
            "schema": "evidence_event_source_v1",
            "schema_version": 1,
            "kind": self.kind,
            "locator": self.locator,
        }
        if self.sha256:
            value["sha256"] = self.sha256
        if self.line_number is not None:
            value["line_number"] = self.line_number
        return value


@dataclass(frozen=True)
class EvidenceEventV2:
    """One immutable event envelope.

    The payload remains the domain event understood by the addressing, sandbox,
    or Corridor projector. The envelope owns sequencing, integrity, provenance,
    and non-live authority state.
    """

    event_id: str
    global_seq: int
    stream: str
    stream_seq: int
    event_type: str
    recorded_at: str
    actor: str
    aggregate: dict[str, str]
    correlation_id: str | None
    causation_id: str | None
    artifact_authority_state_v1: dict[str, Any]
    source: ProvenanceSourceV1
    payload: dict[str, Any]
    idempotency_key: str | None
    previous_event_sha256: str
    event_sha256: str = ""

    def unsigned_dict(self) -> dict[str, Any]:
        value: dict[str, Any] = {
            "schema": EVENT_SCHEMA,
            "schema_version": EVENT_SCHEMA_VERSION,
            "event_id": self.event_id,
            "global_seq": self.global_seq,
            "stream": self.stream,
            "stream_seq": self.stream_seq,
            "event_type": self.event_type,
            "recorded_at": self.recorded_at,
            "actor": self.actor,
            "aggregate": self.aggregate,
            "artifact_authority_state_v1": self.artifact_authority_state_v1,
            "source": self.source.to_dict(),
            "payload": self.payload,
            "previous_event_sha256": self.previous_event_sha256,
        }
        if self.correlation_id:
            value["correlation_id"] = self.correlation_id
        if self.causation_id:
            value["causation_id"] = self.causation_id
        if self.idempotency_key:
            value["idempotency_key"] = self.idempotency_key
        return value

    def calculated_sha256(self) -> str:
        return sha256_canonical(self.unsigned_dict())

    def to_dict(self) -> dict[str, Any]:
        value = self.unsigned_dict()
        value["event_sha256"] = self.event_sha256 or self.calculated_sha256()
        return value

    @classmethod
    def from_dict(cls, value: dict[str, Any]) -> EvidenceEventV2:
        source = value.get("source") if isinstance(value.get("source"), dict) else {}
        authority = (
            value.get("artifact_authority_state_v1")
            if isinstance(value.get("artifact_authority_state_v1"), dict)
            else {}
        )
        aggregate = value.get("aggregate") if isinstance(value.get("aggregate"), dict) else {}
        payload = value.get("payload") if isinstance(value.get("payload"), dict) else {}
        return cls(
            event_id=str(value.get("event_id") or ""),
            global_seq=int(value.get("global_seq") or 0),
            stream=str(value.get("stream") or ""),
            stream_seq=int(value.get("stream_seq") or 0),
            event_type=str(value.get("event_type") or ""),
            recorded_at=str(value.get("recorded_at") or ""),
            actor=str(value.get("actor") or ""),
            aggregate={str(key): str(item) for key, item in aggregate.items()},
            correlation_id=(
                str(value["correlation_id"]) if value.get("correlation_id") else None
            ),
            causation_id=str(value["causation_id"]) if value.get("causation_id") else None,
            artifact_authority_state_v1=authority,
            source=ProvenanceSourceV1(
                kind=str(source.get("kind") or ""),
                locator=str(source.get("locator") or ""),
                sha256=str(source["sha256"]) if source.get("sha256") else None,
                line_number=(
                    int(source["line_number"])
                    if source.get("line_number") is not None
                    else None
                ),
            ),
            payload=payload,
            idempotency_key=(
                str(value["idempotency_key"]) if value.get("idempotency_key") else None
            ),
            previous_event_sha256=str(value.get("previous_event_sha256") or ""),
            event_sha256=str(value.get("event_sha256") or ""),
        )
