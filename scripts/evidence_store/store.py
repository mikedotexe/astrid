"""Locked JSONL storage and verification for Evidence Event Store V2."""

from __future__ import annotations

import copy
import fcntl
import hashlib
import json
import os
import re
import time
import uuid
from collections import Counter
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Iterable

try:
    from authority_state import (
        ARTIFACT_STATE_KEY,
        ArtifactAuthorityStateV1,
        assert_artifact_authority_tree,
        normalize_artifact_authority_tree,
    )
except ModuleNotFoundError:
    from scripts.authority_state import (
        ARTIFACT_STATE_KEY,
        ArtifactAuthorityStateV1,
        assert_artifact_authority_tree,
        normalize_artifact_authority_tree,
    )

from .model import (
    EVENT_SCHEMA,
    EVENT_SCHEMA_VERSION,
    GENESIS_HASH,
    EvidenceEventV2,
    ProvenanceSourceV1,
    canonical_json,
)

DEFAULT_ACTOR = "interactive-agent"
HEAD_SCHEMA = "evidence_event_store_head_v1"
ACTIVATION_SCHEMA = "evidence_event_store_activation_v1"
CHECKPOINT_SCHEMA = "evidence_event_projection_checkpoint_v1"
COMPATIBILITY_EXPORT_SCHEMA = "evidence_event_store_compatibility_export_v1"
SAFE_NAME_RE = re.compile(r"[^a-zA-Z0-9_.-]+")


class EvidenceStoreError(RuntimeError):
    """Raised when integrity, authority, or lifecycle checks fail."""


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


def _utc_now() -> str:
    return datetime.now(tz=UTC).isoformat()


def _atomic_write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.{os.getpid()}.{time.time_ns()}.tmp")
    encoded = json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(encoded)
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(tmp, path)
    try:
        directory_fd = os.open(path.parent, os.O_RDONLY)
    except OSError:
        return
    try:
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _aggregate_for_payload(payload: dict[str, Any]) -> dict[str, str]:
    candidates = (
        ("work_item", "work_item_id"),
        ("introspection", "introspection_id"),
        ("trial", "trial_id"),
        ("packet", "packet_id"),
        ("lease", "lease_id"),
        ("program", "program_id"),
        ("proposal", "proposal_id"),
        ("artifact", "artifact_id"),
    )
    for kind, key in candidates:
        if payload.get(key):
            return {"kind": kind, "id": str(payload[key])}
    for key in ("work_item", "trial", "packet", "lease", "program"):
        child = payload.get(key)
        if not isinstance(child, dict):
            continue
        id_key = f"{key}_id"
        if child.get(id_key):
            return {"kind": key, "id": str(child[id_key])}
    event_type = str(payload.get("event_type") or "unknown")
    return {"kind": "event", "id": event_type}


def _normalized_payload(payload: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any]]:
    normalized = copy.deepcopy(payload)
    normalize_artifact_authority_tree(normalized)
    assert_artifact_authority_tree(normalized)
    authority = normalized.get(ARTIFACT_STATE_KEY)
    if not isinstance(authority, dict):
        authority = ArtifactAuthorityStateV1.evidence_only().canonical_record()
    state = ArtifactAuthorityStateV1(str(authority.get("state") or ""))
    return normalized, state.canonical_record()


class EvidenceEventStore:
    """One globally ordered, multi-stream JSONL event store."""

    def __init__(self, root: Path):
        self.root = Path(root)
        self.events_path = self.root / "events.jsonl"
        self.head_path = self.root / "head.json"
        self.lock_path = self.root / ".append.lock"
        self.activation_path = self.root / "active_store.json"
        self.checkpoints_dir = self.root / "checkpoints"

    def _empty_head(self) -> dict[str, Any]:
        return {
            "schema": HEAD_SCHEMA,
            "schema_version": 1,
            "last_global_seq": 0,
            "last_event_sha256": GENESIS_HASH,
            "stream_sequences": {},
            "legacy_imported_global_seq": 0,
            "updated_at": None,
        }

    def read_head(self) -> dict[str, Any]:
        if not self.head_path.is_file():
            return self._empty_head()
        try:
            value = json.loads(self.head_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise EvidenceStoreError(f"invalid head manifest: {error}") from error
        if not isinstance(value, dict) or value.get("schema") != HEAD_SCHEMA:
            raise EvidenceStoreError("invalid head manifest schema")
        return value

    def _write_head(self, head: dict[str, Any]) -> None:
        value = dict(head)
        value["schema"] = HEAD_SCHEMA
        value["schema_version"] = 1
        value["updated_at"] = _utc_now()
        _atomic_write_json(self.head_path, value)

    def read_envelopes(self) -> tuple[list[EvidenceEventV2], int]:
        if not self.events_path.is_file():
            return [], 0
        events: list[EvidenceEventV2] = []
        corrupt = 0
        with self.events_path.open("r", encoding="utf-8", errors="strict") as handle:
            for line in handle:
                if not line.strip():
                    continue
                try:
                    value = json.loads(line)
                    if not isinstance(value, dict):
                        raise ValueError("event line is not an object")
                    events.append(EvidenceEventV2.from_dict(value))
                except (json.JSONDecodeError, TypeError, ValueError):
                    corrupt += 1
        return events, corrupt

    def verify(self) -> StoreVerification:
        events, corrupt = self.read_envelopes()
        errors: list[str] = []
        stream_counts: Counter[str] = Counter()
        expected_previous = GENESIS_HASH
        expected_global_seq = 1
        stream_sequences: Counter[str] = Counter()
        seen_ids: set[str] = set()
        seen_idempotency: set[tuple[str, str]] = set()

        for event in events:
            stream_counts[event.stream] += 1
            stream_sequences[event.stream] += 1
            prefix = f"event[{event.global_seq}]"
            if event.event_id in seen_ids:
                errors.append(f"{prefix}:duplicate_event_id:{event.event_id}")
            seen_ids.add(event.event_id)
            if event.global_seq != expected_global_seq:
                errors.append(
                    f"{prefix}:global_seq_expected_{expected_global_seq}_got_{event.global_seq}"
                )
            if event.stream_seq != stream_sequences[event.stream]:
                errors.append(
                    f"{prefix}:stream_seq_expected_{stream_sequences[event.stream]}_got_{event.stream_seq}"
                )
            if event.previous_event_sha256 != expected_previous:
                errors.append(f"{prefix}:previous_hash_mismatch")
            calculated = event.calculated_sha256()
            if event.event_sha256 != calculated:
                errors.append(f"{prefix}:event_hash_mismatch")
            if event.idempotency_key:
                identity = (event.stream, event.idempotency_key)
                if identity in seen_idempotency:
                    errors.append(f"{prefix}:duplicate_idempotency_key")
                seen_idempotency.add(identity)
            try:
                state = str(event.artifact_authority_state_v1.get("state") or "")
                ArtifactAuthorityStateV1(state)
                assert_artifact_authority_tree(event.payload)
            except (ValueError, TypeError) as error:
                errors.append(f"{prefix}:authority:{error}")
            if event.to_dict().get("schema") != EVENT_SCHEMA:
                errors.append(f"{prefix}:schema_mismatch")
            expected_previous = event.event_sha256
            expected_global_seq += 1

        if corrupt:
            errors.append(f"corrupt_lines:{corrupt}")
        last_seq = events[-1].global_seq if events else 0
        last_hash = events[-1].event_sha256 if events else GENESIS_HASH
        try:
            head = self.read_head()
        except EvidenceStoreError as error:
            errors.append(str(error))
            head = self._empty_head()
        if int(head.get("last_global_seq") or 0) != last_seq:
            errors.append("head_last_global_seq_mismatch")
        if str(head.get("last_event_sha256") or GENESIS_HASH) != last_hash:
            errors.append("head_last_event_sha256_mismatch")
        expected_streams = {key: int(value) for key, value in stream_counts.items()}
        head_streams = {
            str(key): int(value)
            for key, value in (head.get("stream_sequences") or {}).items()
        }
        if head_streams != expected_streams:
            errors.append("head_stream_sequences_mismatch")
        return StoreVerification(
            valid=not errors,
            event_count=len(events),
            stream_counts=dict(sorted(stream_counts.items())),
            corrupt_lines=corrupt,
            errors=tuple(errors),
            last_global_seq=last_seq,
            last_event_sha256=last_hash,
        )

    def append_payloads(
        self,
        stream: str,
        payloads: Iterable[dict[str, Any]],
        *,
        actor: str = DEFAULT_ACTOR,
        source: ProvenanceSourceV1 | None = None,
        idempotency_keys: Iterable[str | None] | None = None,
    ) -> list[EvidenceEventV2]:
        payload_list = list(payloads)
        if not payload_list:
            return []
        keys = list(idempotency_keys or [None] * len(payload_list))
        if len(keys) != len(payload_list):
            raise EvidenceStoreError("idempotency key count must match payload count")
        if not stream.strip():
            raise EvidenceStoreError("stream must not be empty")

        self.root.mkdir(parents=True, exist_ok=True)
        with self.lock_path.open("a+", encoding="utf-8") as lock_handle:
            fcntl.flock(lock_handle.fileno(), fcntl.LOCK_EX)
            verification = self.verify()
            if not verification.valid:
                raise EvidenceStoreError(
                    "refusing append to invalid store: " + "; ".join(verification.errors)
                )
            head = self.read_head()
            stream_sequences = {
                str(key): int(value)
                for key, value in (head.get("stream_sequences") or {}).items()
            }
            global_seq = int(head.get("last_global_seq") or 0)
            previous_hash = str(head.get("last_event_sha256") or GENESIS_HASH)
            stream_seq = int(stream_sequences.get(stream, 0))
            appended: list[EvidenceEventV2] = []
            event_source = source or ProvenanceSourceV1("runtime_append", stream)
            existing_events, corrupt = self.read_envelopes()
            if corrupt:
                raise EvidenceStoreError("cannot search idempotency in a corrupt store")
            idempotency_index = {
                (event.stream, event.idempotency_key): event
                for event in existing_events
                if event.idempotency_key
            }

            self.events_path.parent.mkdir(parents=True, exist_ok=True)
            with self.events_path.open("a", encoding="utf-8") as event_handle:
                for payload, idempotency_key in zip(payload_list, keys, strict=True):
                    if idempotency_key:
                        existing = idempotency_index.get((stream, idempotency_key))
                        if existing is not None:
                            appended.append(existing)
                            continue
                    normalized, authority = _normalized_payload(payload)
                    global_seq += 1
                    stream_seq += 1
                    event_type = str(normalized.get("event_type") or "unknown")
                    envelope = EvidenceEventV2(
                        event_id=f"evt_{time.time_ns()}_{uuid.uuid4().hex[:12]}",
                        global_seq=global_seq,
                        stream=stream,
                        stream_seq=stream_seq,
                        event_type=event_type,
                        recorded_at=_utc_now(),
                        actor=actor or DEFAULT_ACTOR,
                        aggregate=_aggregate_for_payload(normalized),
                        correlation_id=(
                            str(normalized["correlation_id"])
                            if normalized.get("correlation_id")
                            else None
                        ),
                        causation_id=(
                            str(normalized["causation_id"])
                            if normalized.get("causation_id")
                            else None
                        ),
                        artifact_authority_state_v1=authority,
                        source=event_source,
                        payload=normalized,
                        idempotency_key=idempotency_key,
                        previous_event_sha256=previous_hash,
                    )
                    envelope = EvidenceEventV2(
                        **{
                            **envelope.__dict__,
                            "event_sha256": envelope.calculated_sha256(),
                        }
                    )
                    event_handle.write(canonical_json(envelope.to_dict()) + "\n")
                    previous_hash = envelope.event_sha256
                    appended.append(envelope)
                    if idempotency_key:
                        idempotency_index[(stream, idempotency_key)] = envelope
                event_handle.flush()
                os.fsync(event_handle.fileno())

            stream_sequences[stream] = stream_seq
            head["last_global_seq"] = global_seq
            head["last_event_sha256"] = previous_hash
            head["stream_sequences"] = dict(sorted(stream_sequences.items()))
            self._write_head(head)
            fcntl.flock(lock_handle.fileno(), fcntl.LOCK_UN)
        return appended

    def initialize_from_envelopes(
        self,
        events: Iterable[EvidenceEventV2],
        *,
        legacy_imported: bool,
    ) -> StoreVerification:
        event_list = list(events)
        self.root.mkdir(parents=True, exist_ok=True)
        with self.lock_path.open("a+", encoding="utf-8") as lock_handle:
            fcntl.flock(lock_handle.fileno(), fcntl.LOCK_EX)
            if self.events_path.exists() and self.events_path.stat().st_size:
                raise EvidenceStoreError("refusing to replace a non-empty V2 event store")
            stream_counts: Counter[str] = Counter()
            previous = GENESIS_HASH
            with self.events_path.open("w", encoding="utf-8") as event_handle:
                for expected_seq, event in enumerate(event_list, start=1):
                    if event.global_seq != expected_seq:
                        raise EvidenceStoreError("import event global sequence is not contiguous")
                    if event.previous_event_sha256 != previous:
                        raise EvidenceStoreError("import event hash chain is not contiguous")
                    if event.event_sha256 != event.calculated_sha256():
                        raise EvidenceStoreError("import event hash is invalid")
                    event_handle.write(canonical_json(event.to_dict()) + "\n")
                    stream_counts[event.stream] += 1
                    previous = event.event_sha256
                event_handle.flush()
                os.fsync(event_handle.fileno())
            head = self._empty_head()
            head["last_global_seq"] = len(event_list)
            head["last_event_sha256"] = previous
            head["stream_sequences"] = dict(sorted(stream_counts.items()))
            if legacy_imported:
                head["legacy_imported_global_seq"] = len(event_list)
            self._write_head(head)
            fcntl.flock(lock_handle.fileno(), fcntl.LOCK_UN)
        verification = self.verify()
        if not verification.valid:
            raise EvidenceStoreError(
                "initialized store failed verification: " + "; ".join(verification.errors)
            )
        return verification

    def payloads_for_stream(self, stream: str) -> tuple[list[dict[str, Any]], int]:
        verification = self.verify()
        if not verification.valid:
            return [], max(1, verification.corrupt_lines)
        events, _ = self.read_envelopes()
        return [copy.deepcopy(event.payload) for event in events if event.stream == stream], 0

    def write_checkpoint(
        self,
        projector: str,
        projector_version: int,
        output_hashes: dict[str, str],
    ) -> Path:
        verification = self.verify()
        if not verification.valid:
            raise EvidenceStoreError("cannot checkpoint an invalid store")
        safe_name = SAFE_NAME_RE.sub("_", projector).strip("_") or "projector"
        path = self.checkpoints_dir / f"{safe_name}.json"
        _atomic_write_json(
            path,
            {
                "schema": CHECKPOINT_SCHEMA,
                "schema_version": 1,
                "projector": projector,
                "projector_version": projector_version,
                "last_global_seq": verification.last_global_seq,
                "last_event_sha256": verification.last_event_sha256,
                "output_hashes": dict(sorted(output_hashes.items())),
                "recorded_at": _utc_now(),
            },
        )
        return path

    def checkpoint_current(self, projector: str, projector_version: int) -> bool:
        safe_name = SAFE_NAME_RE.sub("_", projector).strip("_") or "projector"
        path = self.checkpoints_dir / f"{safe_name}.json"
        if not path.is_file():
            return False
        try:
            checkpoint = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            return False
        verification = self.verify()
        return bool(
            verification.valid
            and checkpoint.get("schema") == CHECKPOINT_SCHEMA
            and int(checkpoint.get("projector_version") or 0) == projector_version
            and int(checkpoint.get("last_global_seq") or 0) == verification.last_global_seq
            and checkpoint.get("last_event_sha256") == verification.last_event_sha256
        )

    def export_v1_compatibility(
        self,
        export_root: Path,
        *,
        actor: str,
        acknowledgement: str,
    ) -> dict[str, Any]:
        if not acknowledgement.strip():
            raise EvidenceStoreError("compatibility export requires an acknowledgement")
        export_root = Path(export_root)
        self.root.mkdir(parents=True, exist_ok=True)
        with self.lock_path.open("a+", encoding="utf-8") as lock_handle:
            fcntl.flock(lock_handle.fileno(), fcntl.LOCK_EX)
            verification = self.verify()
            if not verification.valid:
                raise EvidenceStoreError("cannot export an invalid V2 store")
            events, corrupt = self.read_envelopes()
            if corrupt:
                raise EvidenceStoreError("cannot export a store with corrupt lines")
            export_root.mkdir(parents=True, exist_ok=True)
            by_stream: dict[str, list[EvidenceEventV2]] = {}
            for event in events:
                by_stream.setdefault(event.stream, []).append(event)
            streams: dict[str, dict[str, Any]] = {}
            for stream, stream_events in sorted(by_stream.items()):
                safe_stream = SAFE_NAME_RE.sub("_", stream).strip("_") or "stream"
                path = export_root / f"{safe_stream}.events.jsonl"
                tmp = path.with_name(f".{path.name}.{os.getpid()}.{time.time_ns()}.tmp")
                with tmp.open("w", encoding="utf-8") as handle:
                    for event in stream_events:
                        handle.write(canonical_json(event.payload) + "\n")
                    handle.flush()
                    os.fsync(handle.fileno())
                os.replace(tmp, path)
                streams[stream] = {
                    "path": str(path),
                    "event_count": len(stream_events),
                    "sha256": _sha256_file(path),
                }
            receipt = {
                "schema": COMPATIBILITY_EXPORT_SCHEMA,
                "schema_version": 1,
                "actor": actor or DEFAULT_ACTOR,
                "acknowledgement": acknowledgement,
                "recorded_at": _utc_now(),
                "source_store": str(self.root),
                "last_global_seq": verification.last_global_seq,
                "last_event_sha256": verification.last_event_sha256,
                "streams": streams,
                "artifact_authority_state_v1": (
                    ArtifactAuthorityStateV1.evidence_only().canonical_record()
                ),
                "witness_only": True,
            }
            _atomic_write_json(export_root / "compatibility_export_receipt.json", receipt)
            fcntl.flock(lock_handle.fileno(), fcntl.LOCK_UN)
        return receipt

    def _verify_compatibility_export(
        self,
        receipt_path: Path,
        verification: StoreVerification,
    ) -> None:
        try:
            receipt = json.loads(Path(receipt_path).read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise EvidenceStoreError(f"invalid compatibility export receipt: {error}") from error
        if not isinstance(receipt, dict) or receipt.get("schema") != COMPATIBILITY_EXPORT_SCHEMA:
            raise EvidenceStoreError("invalid compatibility export receipt schema")
        if int(receipt.get("last_global_seq") or 0) != verification.last_global_seq:
            raise EvidenceStoreError("compatibility export sequence does not cover current V2 head")
        if receipt.get("last_event_sha256") != verification.last_event_sha256:
            raise EvidenceStoreError("compatibility export hash does not cover current V2 head")
        streams = receipt.get("streams")
        if not isinstance(streams, dict):
            raise EvidenceStoreError("compatibility export stream manifest is missing")
        exported_count = 0
        for stream, record in streams.items():
            if not isinstance(record, dict):
                raise EvidenceStoreError(f"invalid compatibility export record for {stream}")
            path = Path(str(record.get("path") or ""))
            if not path.is_file() or _sha256_file(path) != record.get("sha256"):
                raise EvidenceStoreError(f"compatibility export hash mismatch for {stream}")
            exported_count += int(record.get("event_count") or 0)
        if exported_count != verification.event_count:
            raise EvidenceStoreError("compatibility export event count mismatch")

    def activate(self, *, actor: str, acknowledgement: str) -> dict[str, Any]:
        if not acknowledgement.strip():
            raise EvidenceStoreError("activation requires a non-empty acknowledgement")
        self.root.mkdir(parents=True, exist_ok=True)
        with self.lock_path.open("a+", encoding="utf-8") as lock_handle:
            fcntl.flock(lock_handle.fileno(), fcntl.LOCK_EX)
            verification = self.verify()
            if not verification.valid:
                raise EvidenceStoreError("cannot activate an invalid V2 store")
            head = self.read_head()
            imported_seq = int(head.get("legacy_imported_global_seq") or 0)
            if imported_seq <= 0:
                raise EvidenceStoreError("cannot activate before a verified legacy import")
            value = {
                "schema": ACTIVATION_SCHEMA,
                "schema_version": 1,
                "active_store": "v2",
                "actor": actor or DEFAULT_ACTOR,
                "acknowledgement": acknowledgement,
                "activated_at": _utc_now(),
                "legacy_imported_global_seq": imported_seq,
                "last_global_seq": verification.last_global_seq,
                "last_event_sha256": verification.last_event_sha256,
                "artifact_authority_state_v1": (
                    ArtifactAuthorityStateV1.evidence_only().canonical_record()
                ),
                "witness_only": True,
            }
            _atomic_write_json(self.activation_path, value)
            fcntl.flock(lock_handle.fileno(), fcntl.LOCK_UN)
        return value

    def rollback_to_v1(
        self,
        *,
        actor: str,
        acknowledgement: str,
        compatibility_export: Path | None = None,
    ) -> dict[str, Any]:
        if not acknowledgement.strip():
            raise EvidenceStoreError("rollback requires a non-empty acknowledgement")
        self.root.mkdir(parents=True, exist_ok=True)
        with self.lock_path.open("a+", encoding="utf-8") as lock_handle:
            fcntl.flock(lock_handle.fileno(), fcntl.LOCK_EX)
            verification = self.verify()
            if not verification.valid:
                raise EvidenceStoreError("cannot roll back from an invalid V2 store")
            head = self.read_head()
            imported_seq = int(head.get("legacy_imported_global_seq") or 0)
            if verification.last_global_seq > imported_seq:
                if compatibility_export is None:
                    raise EvidenceStoreError(
                        "rollback refused: V2-only events exist and no verified compatibility export was supplied"
                    )
                self._verify_compatibility_export(compatibility_export, verification)
            value = {
                "schema": ACTIVATION_SCHEMA,
                "schema_version": 1,
                "active_store": "v1",
                "actor": actor or DEFAULT_ACTOR,
                "acknowledgement": acknowledgement,
                "rolled_back_at": _utc_now(),
                "legacy_imported_global_seq": imported_seq,
                "last_global_seq": verification.last_global_seq,
                "artifact_authority_state_v1": (
                    ArtifactAuthorityStateV1.evidence_only().canonical_record()
                ),
                "witness_only": True,
            }
            _atomic_write_json(self.activation_path, value)
            fcntl.flock(lock_handle.fileno(), fcntl.LOCK_UN)
        return value
