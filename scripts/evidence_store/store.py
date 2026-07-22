"""Locked JSONL storage and verification for Evidence Event Store V2."""

from __future__ import annotations

import copy
import fcntl
import hashlib
import json
import os
import re
import stat
import time
import uuid
from collections import Counter
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Iterable, Iterator

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
from .index import EvidenceReadIndex, EvidenceReadIndexError
from .verification import StoreVerification, verify_canonical_events

DEFAULT_ACTOR = "interactive-agent"
VERIFIED_SESSION_ENV = "ASTRID_EVIDENCE_PROJECTION_SESSION"
HEAD_SCHEMA = "evidence_event_store_head_v1"
ACTIVATION_SCHEMA = "evidence_event_store_activation_v1"
CHECKPOINT_SCHEMA = "evidence_event_projection_checkpoint_v1"
CHECKPOINT_SCHEMA_V2 = "evidence_event_projection_checkpoint_v2"
CHECKPOINT_SCHEMA_V3 = "evidence_event_projection_checkpoint_v3"
COMPATIBILITY_EXPORT_SCHEMA = "evidence_event_store_compatibility_export_v1"
SAFE_NAME_RE = re.compile(r"[^a-zA-Z0-9_.-]+")


class EvidenceStoreError(RuntimeError):
    """Raised when integrity, authority, or lifecycle checks fail."""


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
    explicit_kind = str(payload.get("aggregate_type") or "").strip()
    explicit_id = str(payload.get("aggregate_id") or "").strip()
    if explicit_kind and explicit_id:
        return {"kind": explicit_kind, "id": explicit_id}
    candidates = (
        ("work_item", "work_item_id"),
        ("introspection", "introspection_id"),
        ("trial", "trial_id"),
        ("packet", "packet_id"),
        ("lease", "lease_id"),
        ("program", "program_id"),
        ("proposal", "proposal_id"),
        ("artifact", "artifact_id"),
        ("causal_signal_journey", "journey_id"),
        ("temporal_lived_state_witness", "witness_id"),
        ("reciprocal_receipt", "receipt_id"),
        ("representation_contract", "contract_id"),
        ("representation_transition", "transition_id"),
        ("concordance_study", "study_id"),
        ("agency_commons_record", "record_id"),
        ("claim_family", "family_id"),
        ("experiment_dossier", "dossier_id"),
        ("attention_portfolio", "portfolio_id"),
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
        self.read_index = EvidenceReadIndex(
            self.root,
            self.events_path,
            self.head_path,
        )
        self._verified_anchor: tuple[int, str] | None = None
        self._last_verification_mode: str | None = None

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
        verification = verify_canonical_events(
            self.events_path,
            self.read_head,
            self._empty_head,
        )
        self._verified_anchor = (
            (verification.last_global_seq, verification.last_event_sha256)
            if verification.valid
            else None
        )
        self._last_verification_mode = "full_chain"
        return verification

    def verify_indexed_tail(self) -> StoreVerification:
        """Validate only canonical bytes after an already verified chain anchor."""

        had_anchor = self._verified_anchor is not None
        if not had_anchor:
            session_anchor = self._session_anchor()
            had_anchor = (
                session_anchor is not None
                and self.read_index.has_anchor(*session_anchor)
            )
        index = self._prepare_read_index()
        status = index.status(
            include_details=True,
            include_logical_digest=False,
        )
        errors: list[str] = []
        if not status.get("valid_schema"):
            errors.append("index_schema_invalid")
        if not status.get("matches_head"):
            errors.append("index_head_mismatch")
        if status.get("permissions") != "0o600":
            errors.append("index_permissions_not_owner_only")
        verification = StoreVerification(
            valid=not errors,
            event_count=int(status.get("event_count") or 0),
            stream_counts={
                str(stream): int(count)
                for stream, count in (status.get("stream_counts") or {}).items()
            },
            corrupt_lines=0,
            errors=tuple(errors),
            last_global_seq=int(status.get("last_global_seq") or 0),
            last_event_sha256=str(
                status.get("last_event_sha256") or GENESIS_HASH
            ),
        )
        self._verified_anchor = (
            (verification.last_global_seq, verification.last_event_sha256)
            if verification.valid
            else None
        )
        self._last_verification_mode = (
            "indexed_tail" if had_anchor else "full_chain"
        )
        return verification

    def _session_anchor(self) -> tuple[int, str] | None:
        raw_path = os.environ.get(VERIFIED_SESSION_ENV)
        if not raw_path:
            return None
        path = Path(raw_path)
        try:
            if stat.S_IMODE(path.stat().st_mode) & 0o077:
                return None
            value = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            return None
        if (
            not isinstance(value, dict)
            or value.get("schema") != "evidence_projection_session_v1"
            or Path(str(value.get("store_root") or "")).resolve()
            != self.root.resolve()
            or float(value.get("expires_at_unix") or 0) <= time.time()
        ):
            return None
        return (
            int(value.get("verified_global_seq") or 0),
            str(value.get("verified_event_sha256") or GENESIS_HASH),
        )

    def _prepare_read_index(
        self,
        *,
        append_lock_held: bool = False,
    ) -> EvidenceReadIndex:
        """Verify canonical history once per store instance, then reconcile its tail."""

        def prepare_locked() -> None:
            head = self.read_head()
            current_anchor = (
                int(head.get("last_global_seq") or 0),
                str(head.get("last_event_sha256") or GENESIS_HASH),
            )
            session_anchor = self._session_anchor()
            can_validate_tail = self._verified_anchor is not None or (
                session_anchor is not None
                and self.read_index.has_anchor(*session_anchor)
            )
            if self._verified_anchor != current_anchor and not can_validate_tail:
                verification = self.verify()
                if not verification.valid:
                    raise EvidenceStoreError(
                        "cannot index invalid store: "
                        + "; ".join(verification.errors)
                    )
            try:
                self.read_index.reconcile()
            except EvidenceReadIndexError as error:
                raise EvidenceStoreError(
                    f"invalid evidence read index: {error}"
                ) from error
            self._verified_anchor = current_anchor

        if append_lock_held:
            prepare_locked()
        else:
            self.root.mkdir(parents=True, exist_ok=True)
            with self.lock_path.open("a+", encoding="utf-8") as lock_handle:
                fcntl.flock(lock_handle.fileno(), fcntl.LOCK_EX)
                prepare_locked()
                fcntl.flock(lock_handle.fileno(), fcntl.LOCK_UN)
        return self.read_index

    def prepare_read_index(self) -> dict[str, Any]:
        """Prepare and return the derived index status after canonical verification."""

        self._prepare_read_index()
        return self.read_index.status(include_details=False)

    def append_payloads(
        self,
        stream: str,
        payloads: Iterable[dict[str, Any]],
        *,
        actor: str = DEFAULT_ACTOR,
        source: ProvenanceSourceV1 | None = None,
        idempotency_keys: Iterable[str | None] | None = None,
        include_existing_idempotent: bool = True,
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
            read_index = self._prepare_read_index(append_lock_held=True)
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
            idempotency_index = {
                key: read_index.event_for_idempotency(stream, key)
                for key in {key for key in keys if key}
            }

            self.events_path.parent.mkdir(parents=True, exist_ok=True)
            with self.events_path.open("a", encoding="utf-8") as event_handle:
                for payload, idempotency_key in zip(payload_list, keys, strict=True):
                    if idempotency_key:
                        existing = idempotency_index.get(idempotency_key)
                        if existing is not None:
                            if include_existing_idempotent:
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
                        idempotency_index[idempotency_key] = envelope
                event_handle.flush()
                os.fsync(event_handle.fileno())

            stream_sequences[stream] = stream_seq
            head["last_global_seq"] = global_seq
            head["last_event_sha256"] = previous_hash
            head["stream_sequences"] = dict(sorted(stream_sequences.items()))
            self._write_head(head)
            self._verified_anchor = (global_seq, previous_hash)
            try:
                self.read_index.reconcile()
            except EvidenceReadIndexError:
                # Canonical JSONL and head are already durable. The derived index
                # remains safely rebuildable on the next controlled read.
                pass
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
        try:
            index = self._prepare_read_index()
            return [
                copy.deepcopy(event.payload)
                for event in index.iter_stream(stream)
            ], 0
        except (EvidenceStoreError, EvidenceReadIndexError):
            return [], 1

    def envelopes_for_stream(
        self,
        stream: str,
        *,
        after_stream_seq: int = 0,
    ) -> tuple[list[EvidenceEventV2], int]:
        """Read one stream from the derived offset index.

        Callers that need durable source identities should use envelopes rather
        than reconstructing them from payloads. The canonical JSONL remains the
        source of every returned record.
        """

        try:
            index = self._prepare_read_index()
            return [
                copy.deepcopy(event)
                for event in index.iter_stream(
                    stream,
                    after_stream_seq=after_stream_seq,
                )
            ], 0
        except (EvidenceStoreError, EvidenceReadIndexError):
            return [], 1

    def iter_envelopes_for_stream(
        self,
        stream: str,
        *,
        after_stream_seq: int = 0,
    ) -> Iterator[EvidenceEventV2]:
        """Stream one canonical V2 stream without accumulating its payloads."""

        try:
            index = self._prepare_read_index()
            yield from index.iter_stream(
                stream,
                after_stream_seq=after_stream_seq,
            )
        except EvidenceReadIndexError as error:
            raise EvidenceStoreError(
                f"cannot stream indexed events for {stream}: {error}"
            ) from error

    def payloads_for_stream_after(
        self,
        stream: str,
        *,
        after_stream_seq: int,
    ) -> tuple[list[dict[str, Any]], int]:
        try:
            index = self._prepare_read_index()
            return [
                copy.deepcopy(event.payload)
                for event in index.iter_stream(
                    stream,
                    after_stream_seq=after_stream_seq,
                )
            ], 0
        except (EvidenceStoreError, EvidenceReadIndexError):
            return [], 1

    def stream_watermarks(
        self,
        streams: Iterable[str],
    ) -> dict[str, dict[str, Any]]:
        """Return exact high-water identities for only the declared streams."""

        requested = sorted({str(stream) for stream in streams if str(stream).strip()})
        try:
            return self._prepare_read_index().stream_watermarks(requested)
        except EvidenceReadIndexError as error:
            raise EvidenceStoreError(f"cannot read indexed watermarks: {error}") from error

    def idempotency_keys(self, stream: str) -> set[str]:
        try:
            return self._prepare_read_index().idempotency_keys(stream)
        except EvidenceReadIndexError as error:
            raise EvidenceStoreError(
                f"cannot read idempotency keys for {stream}: {error}"
            ) from error

    def write_checkpoint(
        self,
        projector: str,
        projector_version: int,
        output_hashes: dict[str, str],
        *,
        input_streams: Iterable[str] | None = None,
        source_hashes: dict[str, str] | None = None,
        dependency_output_hashes: dict[str, str] | None = None,
        command_sha256: str | None = None,
        config_sha256: str | None = None,
    ) -> Path:
        self._prepare_read_index()
        head = self.read_head()
        observed_global_seq = int(head.get("last_global_seq") or 0)
        observed_event_sha256 = str(
            head.get("last_event_sha256") or GENESIS_HASH
        )
        safe_name = SAFE_NAME_RE.sub("_", projector).strip("_") or "projector"
        path = self.checkpoints_dir / f"{safe_name}.json"
        if any(
            value is not None
            for value in (
                dependency_output_hashes,
                command_sha256,
                config_sha256,
            )
        ):
            declared_streams = sorted(
                {
                    str(stream)
                    for stream in (input_streams or ())
                    if str(stream).strip()
                }
            )
            input_identity = {
                "schema": "projection_input_identity_v3",
                "schema_version": 3,
                "input_streams": declared_streams,
                "input_stream_watermarks": self.stream_watermarks(
                    declared_streams
                ),
                "source_hashes": dict(sorted((source_hashes or {}).items())),
                "dependency_output_hashes": dict(
                    sorted((dependency_output_hashes or {}).items())
                ),
                "command_sha256": command_sha256,
                "config_sha256": config_sha256,
                "projector_version": projector_version,
            }
            _atomic_write_json(
                path,
                {
                    "schema": CHECKPOINT_SCHEMA_V3,
                    "schema_version": 3,
                    "projector": projector,
                    "projector_version": projector_version,
                    "input_identity": input_identity,
                    "store_observed_at": {
                        "last_global_seq": observed_global_seq,
                        "last_event_sha256": observed_event_sha256,
                    },
                    "output_hashes": dict(sorted(output_hashes.items())),
                    "recorded_at": _utc_now(),
                },
            )
            return path
        if input_streams is not None:
            declared_streams = sorted(
                {str(stream) for stream in input_streams if str(stream).strip()}
            )
            _atomic_write_json(
                path,
                {
                    "schema": CHECKPOINT_SCHEMA_V2,
                    "schema_version": 2,
                    "projector": projector,
                    "projector_version": projector_version,
                    "input_streams": declared_streams,
                    "input_stream_watermarks": self.stream_watermarks(
                        declared_streams
                    ),
                    "source_hashes": dict(sorted((source_hashes or {}).items())),
                    "store_observed_at": {
                        "last_global_seq": observed_global_seq,
                        "last_event_sha256": observed_event_sha256,
                    },
                    "output_hashes": dict(sorted(output_hashes.items())),
                    "recorded_at": _utc_now(),
                },
            )
            return path
        _atomic_write_json(
            path,
            {
                "schema": CHECKPOINT_SCHEMA,
                "schema_version": 1,
                "projector": projector,
                "projector_version": projector_version,
                "last_global_seq": observed_global_seq,
                "last_event_sha256": observed_event_sha256,
                "output_hashes": dict(sorted(output_hashes.items())),
                "recorded_at": _utc_now(),
            },
        )
        return path

    def checkpoint_current(self, projector: str, projector_version: int) -> bool:
        return self.checkpoint_current_for_inputs(projector, projector_version)

    def read_checkpoint(self, projector: str) -> dict[str, Any] | None:
        safe_name = SAFE_NAME_RE.sub("_", projector).strip("_") or "projector"
        path = self.checkpoints_dir / f"{safe_name}.json"
        try:
            value = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            return None
        return value if isinstance(value, dict) else None

    def checkpoint_current_for_inputs(
        self,
        projector: str,
        projector_version: int,
        *,
        input_streams: Iterable[str] | None = None,
        source_hashes: dict[str, str] | None = None,
        dependency_output_hashes: dict[str, str] | None = None,
        command_sha256: str | None = None,
        config_sha256: str | None = None,
        output_hashes: dict[str, str] | None = None,
    ) -> bool:
        checkpoint = self.read_checkpoint(projector)
        if checkpoint is None:
            return False
        try:
            self._prepare_read_index()
            head = self.read_head()
            store_valid = True
        except EvidenceStoreError:
            head = self._empty_head()
            store_valid = False
        if (
            store_valid
            and checkpoint.get("schema") == CHECKPOINT_SCHEMA_V3
            and int(checkpoint.get("projector_version") or 0) == projector_version
        ):
            identity = checkpoint.get("input_identity")
            if not isinstance(identity, dict):
                return False
            stored_streams = identity.get("input_streams")
            stored_sources = identity.get("source_hashes")
            stored_dependencies = identity.get("dependency_output_hashes")
            if (
                not isinstance(stored_streams, list)
                or not isinstance(stored_sources, dict)
                or not isinstance(stored_dependencies, dict)
            ):
                return False
            expected_streams = sorted(
                {
                    str(stream)
                    for stream in (
                        stored_streams if input_streams is None else input_streams
                    )
                    if str(stream).strip()
                }
            )
            expected_identity = {
                "schema": "projection_input_identity_v3",
                "schema_version": 3,
                "input_streams": expected_streams,
                "input_stream_watermarks": self.stream_watermarks(
                    expected_streams
                ),
                "source_hashes": dict(
                    sorted(
                        (
                            stored_sources
                            if source_hashes is None
                            else source_hashes
                        )
                        .items()
                    )
                ),
                "dependency_output_hashes": dict(
                    sorted(
                        (
                            stored_dependencies
                            if dependency_output_hashes is None
                            else dependency_output_hashes
                        )
                        .items()
                    )
                ),
                "command_sha256": (
                    identity.get("command_sha256")
                    if command_sha256 is None
                    else command_sha256
                ),
                "config_sha256": (
                    identity.get("config_sha256")
                    if config_sha256 is None
                    else config_sha256
                ),
                "projector_version": projector_version,
            }
            if expected_identity != identity:
                return False
            if output_hashes is not None and dict(
                sorted(output_hashes.items())
            ) != checkpoint.get("output_hashes"):
                return False
            return True
        if (
            store_valid
            and checkpoint.get("schema") == CHECKPOINT_SCHEMA_V2
            and int(checkpoint.get("projector_version") or 0) == projector_version
        ):
            stored_streams = checkpoint.get("input_streams")
            if not isinstance(stored_streams, list):
                return False
            expected_streams = sorted(
                {
                    str(stream)
                    for stream in (
                        stored_streams if input_streams is None else input_streams
                    )
                    if str(stream).strip()
                }
            )
            if expected_streams != sorted(str(stream) for stream in stored_streams):
                return False
            expected_sources = (
                checkpoint.get("source_hashes")
                if source_hashes is None
                else dict(sorted(source_hashes.items()))
            )
            return bool(
                self.stream_watermarks(expected_streams)
                == checkpoint.get("input_stream_watermarks")
                and expected_sources == checkpoint.get("source_hashes")
            )
        return bool(
            store_valid
            and checkpoint.get("schema") == CHECKPOINT_SCHEMA
            and int(checkpoint.get("projector_version") or 0) == projector_version
            and int(checkpoint.get("last_global_seq") or 0)
            == int(head.get("last_global_seq") or 0)
            and checkpoint.get("last_event_sha256")
            == str(head.get("last_event_sha256") or GENESIS_HASH)
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
