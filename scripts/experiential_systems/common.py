"""Mechanical primitives shared by reciprocal experiential projectors.

The helpers in this module are deliberately domain-neutral. They persist
bounded metadata, hashes, and source references; they never infer consent,
felt state, causation, closure, or live authority.
"""

from __future__ import annotations

import hashlib
import fcntl
import json
import os
import re
import tempfile
from dataclasses import asdict, is_dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Iterable, Mapping

try:
    from authority_state import ArtifactAuthorityStateV1, assert_artifact_authority_tree
    from evidence_store import EvidenceEventStore
    from evidence_store.model import ProvenanceSourceV1
except ModuleNotFoundError:
    from scripts.authority_state import (
        ArtifactAuthorityStateV1,
        assert_artifact_authority_tree,
    )
    from scripts.evidence_store import EvidenceEventStore
    from scripts.evidence_store.model import ProvenanceSourceV1

SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
IDENTIFIER_RE = re.compile(r"^[A-Za-z0-9_.:@/+\-]{1,240}$")
FORBIDDEN_PROSE_KEYS = frozenset(
    {
        "body",
        "body_preview",
        "content",
        "discrepancy_log",
        "journal",
        "message",
        "phenomenology",
        "prompt",
        "prompts",
        "prose",
        "response",
        "responses",
        "somatic_description",
        "text",
    }
)


class RecordValidationError(ValueError):
    """Untrusted persisted input failed a bounded record contract."""


def canonical_json(value: Any) -> str:
    if is_dataclass(value):
        value = asdict(value)
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    )


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def deterministic_id(prefix: str, value: Any) -> str:
    return f"{prefix}_{sha256_bytes(canonical_json(value).encode('utf-8'))}"


def utc_now() -> str:
    return datetime.now(tz=UTC).isoformat()


def authority_state(state: str = "evidence_only") -> dict[str, Any]:
    if state == "evidence_only":
        return ArtifactAuthorityStateV1.evidence_only().canonical_record()
    if state == "approval_pending":
        return ArtifactAuthorityStateV1.approval_pending().canonical_record()
    raise RecordValidationError(f"unsupported authority state: {state!r}")


def validate_sha256(value: Any, field: str, *, optional: bool = False) -> str | None:
    if optional and value is None:
        return None
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        raise RecordValidationError(f"{field} must be a lowercase SHA-256")
    return value


def validate_bounded_identifier(
    value: Any,
    field: str,
    *,
    optional: bool = False,
    limit: int = 240,
) -> str | None:
    if optional and value is None:
        return None
    if not isinstance(value, str) or not value or len(value) > limit:
        raise RecordValidationError(f"{field} must be a non-empty bounded string")
    if IDENTIFIER_RE.fullmatch(value) is None:
        raise RecordValidationError(f"{field} contains unsupported characters")
    return value


def validate_timestamp(value: Any, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise RecordValidationError(f"{field} must be a positive integer timestamp")
    return value


def reject_private_content(value: Any, *, path: tuple[str, ...] = ()) -> None:
    if isinstance(value, list):
        for index, item in enumerate(value):
            reject_private_content(item, path=(*path, str(index)))
        return
    if not isinstance(value, dict):
        return
    for key, item in value.items():
        normalized = str(key).lower()
        if normalized in FORBIDDEN_PROSE_KEYS or normalized.endswith("_prose"):
            raise RecordValidationError(
                f"private content key forbidden at {'.'.join((*path, str(key)))}"
            )
        reject_private_content(item, path=(*path, str(key)))


def validate_evidence_record(value: Mapping[str, Any]) -> None:
    reject_private_content(dict(value))
    assert_artifact_authority_tree(dict(value))
    authority = value.get("artifact_authority_state_v1")
    if not isinstance(authority, dict) or authority.get("state") not in {
        "evidence_only",
        "approval_pending",
    }:
        raise RecordValidationError("record must carry bounded artifact authority")


def owner_atomic_write(path: Path, content: str | bytes) -> None:
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    os.chmod(path.parent, 0o700)
    mode = "wb" if isinstance(content, bytes) else "w"
    kwargs = {} if isinstance(content, bytes) else {"encoding": "utf-8"}
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", dir=path.parent
    )
    try:
        with os.fdopen(descriptor, mode, **kwargs) as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temporary_name, 0o600)
        os.replace(temporary_name, path)
        directory_fd = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    finally:
        if os.path.exists(temporary_name):
            os.unlink(temporary_name)


def owner_atomic_write_json(path: Path, value: Any) -> None:
    owner_atomic_write(path, json.dumps(value, indent=2, sort_keys=True) + "\n")


def owner_atomic_write_jsonl(path: Path, rows: Iterable[Mapping[str, Any]]) -> None:
    encoded = "".join(canonical_json(dict(row)) + "\n" for row in rows)
    owner_atomic_write(path, encoded)


def owner_append_jsonl(path: Path, value: Mapping[str, Any]) -> None:
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    os.chmod(path.parent, 0o700)
    lock_path = path.with_name(f".{path.name}.lock")
    with lock_path.open("a+", encoding="utf-8") as lock:
        os.chmod(lock_path, 0o600)
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        with path.open("a", encoding="utf-8") as handle:
            os.chmod(path, 0o600)
            handle.write(canonical_json(dict(value)) + "\n")
            handle.flush()
            os.fsync(handle.fileno())
        fcntl.flock(lock.fileno(), fcntl.LOCK_UN)


def load_jsonl(path: Path) -> tuple[list[dict[str, Any]], list[str]]:
    if not path.is_file():
        return [], []
    rows: list[dict[str, Any]] = []
    errors: list[str] = []
    with path.open("r", encoding="utf-8", errors="strict") as handle:
        for line_number, raw in enumerate(handle, 1):
            if not raw.strip():
                continue
            try:
                value = json.loads(raw)
            except json.JSONDecodeError:
                errors.append(f"line_{line_number}:invalid_json")
                continue
            if not isinstance(value, dict):
                errors.append(f"line_{line_number}:not_object")
                continue
            rows.append(value)
    return rows, errors


def source_locator(path: Path, root: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return f"external_sha256:{sha256_bytes(str(path.resolve()).encode())}"


def project_events(
    workspace: Path,
    stream: str,
    payloads: Iterable[dict[str, Any]],
    *,
    actor: str,
    source_kind: str,
    source_locator_value: str,
) -> int:
    bounded = list(payloads)
    for payload in bounded:
        validate_evidence_record(payload)
    if not bounded:
        return 0
    store = EvidenceEventStore(workspace / "diagnostics/evidence_event_store_v2")
    events = store.append_payloads(
        stream,
        bounded,
        actor=actor,
        source=ProvenanceSourceV1(source_kind, source_locator_value),
        idempotency_keys=[str(item["idempotency_key"]) for item in bounded],
        include_existing_idempotent=False,
    )
    return len(events)


def stream_payloads(
    workspace: Path, stream: str
) -> tuple[list[dict[str, Any]], int]:
    store = EvidenceEventStore(workspace / "diagnostics/evidence_event_store_v2")
    return store.payloads_for_stream(stream)


def event_payload(
    *,
    schema: str,
    event_type: str,
    aggregate_type: str,
    aggregate_id: str,
    idempotency_key: str,
    record: Mapping[str, Any],
    authority: str = "evidence_only",
) -> dict[str, Any]:
    payload = {
        "schema": schema,
        "schema_version": 1,
        "event_type": event_type,
        "aggregate_type": aggregate_type,
        "aggregate_id": aggregate_id,
        "idempotency_key": idempotency_key,
        "record": dict(record),
        "raw_prose_included": False,
        "consent_inferred": False,
        "closure_propagated": False,
        "authority_propagated": False,
        "artifact_authority_state_v1": authority_state(authority),
    }
    validate_evidence_record(payload)
    return payload
