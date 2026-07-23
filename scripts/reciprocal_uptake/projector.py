"""Project only explicit reciprocal presence and uptake actions."""

from __future__ import annotations

import json
import math
from collections import Counter
from pathlib import Path
from typing import Any, Iterable

try:
    from experiential_systems.common import (
        RecordValidationError,
        authority_state,
        canonical_json,
        event_payload,
        owner_atomic_write,
        owner_atomic_write_json,
        owner_atomic_write_jsonl,
        project_events,
        sha256_bytes,
        stream_payloads,
        validate_evidence_record,
    )
    from evidence_store import EvidenceEventStore
    from projection_cursors import ProjectionInputCursor
    from lived_state_witness.validation import validate_witness
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError,
        authority_state,
        canonical_json,
        event_payload,
        owner_atomic_write,
        owner_atomic_write_json,
        owner_atomic_write_jsonl,
        project_events,
        sha256_bytes,
        stream_payloads,
        validate_evidence_record,
    )
    from scripts.evidence_store import EvidenceEventStore
    from scripts.projection_cursors import ProjectionInputCursor
    from scripts.lived_state_witness.validation import validate_witness

from .model import (
    PresenceKindV1,
    ReciprocalContextKindV1,
    ReciprocalContextReceiptV2,
    ReciprocalPresenceReceiptV2,
    ReciprocalResonanceRelationV1,
    ReciprocalResonanceSignatureV1,
    ReciprocalUptakeReceiptV2,
    ReciprocalUptakeReceiptV3,
    UptakeKindV1,
    build_context_receipt,
    build_presence_receipt,
    build_resonant_uptake_receipt,
    build_uptake_receipt,
)

STREAM = "reciprocal_uptake"
SCHEMA = "reciprocal_uptake_domain_event_v2"
STATE_DIRNAME = "reciprocal_uptake_v1"

_LEGACY_INFERENCE_FIELDS = frozenset(
    {
        "presence_is_acknowledgement",
        "presence_inferred",
        "acknowledgement_inferred",
        "uptake_inferred",
        "reply_intention_inferred",
        "elapsed_time_inferred",
        "intention_is_nonbinding",
        "decline_implies_closure",
        "decline_implies_disagreement",
        "decline_implies_negative_felt_state",
        "raw_prose_included",
    }
)


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics" / STATE_DIRNAME


def _source_identity(row: dict[str, Any], raw: str) -> tuple[str, str]:
    raw_sha = sha256_bytes(raw.encode("utf-8"))
    bounded = {
        "record_type": row.get("record_type"),
        "thread_id": row.get("thread_id"),
        "message_id": row.get("message_id"),
        "recorded_at_unix_ms": row.get("recorded_at_unix_ms"),
        "raw_sha256": raw_sha,
    }
    return f"corrsrc_{sha256_bytes(canonical_json(bounded).encode())}", raw_sha


def _timestamp(row: dict[str, Any]) -> int:
    for key in ("recorded_at_unix_ms", "created_at_unix_ms", "t_ms"):
        value = row.get(key)
        if isinstance(value, int) and not isinstance(value, bool) and value > 0:
            return value
    raise RecordValidationError("correspondence row lacks a positive timestamp")


def _common(row: dict[str, Any], raw: str, *, actor: str, peer: str) -> dict[str, Any]:
    source_event_id, source_hash = _source_identity(row, raw)
    return {
        "actor": actor,
        "peer": peer,
        "thread_id": str(row.get("thread_id") or ""),
        "message_id": str(row.get("message_id") or "") or None,
        "source_event_id": source_event_id,
        "source_event_sha256": source_hash,
        "body_sha256": row.get("body_sha256"),
        "recorded_at_unix_ms": _timestamp(row),
    }


def receipts_for_row(
    row: dict[str, Any],
    raw: str,
    message_index: dict[str, dict[str, Any]] | None = None,
    inferred_by_source: dict[tuple[str, str], str] | None = None,
) -> list[dict[str, Any]]:
    record_type = str(row.get("record_type") or "")
    actor = str(row.get("from_being") or "")
    peer = str(row.get("to_being") or "")
    result: list[dict[str, Any]] = []
    if row.get("presence_receipt") == "language_only_presence":
        result.append(
            build_presence_receipt(
                PresenceKindV1.OFFERED,
                **_common(row, raw, actor=actor, peer=peer),
            ).to_dict()
        )
    source_event_id, _ = _source_identity(row, raw)
    if record_type == "read_receipt" and row.get("read_state") == "read":
        reader = str(row.get("reader") or peer)
        source_message = (message_index or {}).get(str(row.get("message_id") or ""), {})
        source_actor = str(source_message.get("from_being") or "")
        if not source_actor:
            return result
        result.append(
            build_context_receipt(
                ReciprocalContextKindV1.READ_RECEIPT,
                corrects_legacy_receipt_id=(
                    inferred_by_source or {}
                ).get((source_event_id, UptakeKindV1.ATTENDED_MESSAGE.value)),
                **_common(row, raw, actor=reader, peer=source_actor),
            ).to_dict()
        )
    if record_type == "reply_link":
        result.append(
            build_context_receipt(
                ReciprocalContextKindV1.REPLY_LINK,
                corrects_legacy_receipt_id=(
                    inferred_by_source or {}
                ).get((source_event_id, UptakeKindV1.REPLY_INTENTION.value)),
                **_common(row, raw, actor=actor, peer=peer),
            ).to_dict()
        )
    if record_type == "delivery_receipt" and row.get("delivery_state"):
        result.append(
            build_context_receipt(
                ReciprocalContextKindV1.DELIVERY_RECEIPT,
                **_common(row, raw, actor=actor, peer=peer),
            ).to_dict()
        )
    if row.get("correspondence_type") == "presence_heartbeat":
        result.append(
            build_context_receipt(
                ReciprocalContextKindV1.PRESENCE_HEARTBEAT,
                **_common(row, raw, actor=actor, peer=peer),
            ).to_dict()
        )
    if record_type == "message" and row.get("silt_continuity") is True:
        result.append(
            build_uptake_receipt(
                UptakeKindV1.CONTINUITY_CARRIED_FORWARD,
                **_common(row, raw, actor=actor, peer=peer),
            ).to_dict()
        )
    explicit_value = str(
        row.get("uptake_action") or row.get("engagement_state") or ""
    )
    explicit = {
        "attended_message": UptakeKindV1.ATTENDED_MESSAGE,
        "reply_intention": UptakeKindV1.REPLY_INTENTION,
        "continuity_carried_forward": UptakeKindV1.CONTINUITY_CARRIED_FORWARD,
        "declined_engagement": UptakeKindV1.DECLINED_ENGAGEMENT,
        "needs_time": UptakeKindV1.NEEDS_TIME,
        "withdrawn_intention": UptakeKindV1.WITHDRAWN_INTENTION,
        "ambient_persistence": UptakeKindV1.AMBIENT_PERSISTENCE,
        "resonant_persistence": UptakeKindV1.RESONANT_PERSISTENCE,
    }.get(explicit_value)
    if (
        explicit is None
        and record_type == "attention_canary_outcome"
        and row.get("held_as") == "ambient_echo"
    ):
        explicit = UptakeKindV1.AMBIENT_PERSISTENCE
    if explicit is not None:
        if explicit is UptakeKindV1.RESONANT_PERSISTENCE:
            signature = ReciprocalResonanceSignatureV1.from_untrusted(
                row.get("resonance_signature_v1")
            )
            result.append(
                build_resonant_uptake_receipt(
                    resonance_signature_v1=signature,
                    **_common(row, raw, actor=actor, peer=peer),
                ).to_dict()
            )
        else:
            result.append(
                build_uptake_receipt(
                    explicit,
                    **_common(row, raw, actor=actor, peer=peer),
                ).to_dict()
            )
    return result


def _read_rows(
    ledger: Path, cursor: ProjectionInputCursor | None
) -> tuple[list[tuple[int, str]], dict[str, Any]]:
    if cursor is not None:
        return cursor.jsonl_tail(ledger, key="correspondence")
    raw = ledger.read_bytes() if ledger.is_file() else b""
    return list(enumerate(raw.decode("utf-8").splitlines(), 1)), {
        "source_sha256": sha256_bytes(raw)
    }


def _canonical_record(value: dict[str, Any]) -> dict[str, Any]:
    schema = value.get("schema")
    if schema in {
        "reciprocal_presence_receipt_v1",
        "reciprocal_presence_receipt_v2",
    }:
        return ReciprocalPresenceReceiptV2.from_untrusted(value).to_dict()
    if schema in {
        "reciprocal_context_receipt_v1",
        "reciprocal_context_receipt_v2",
    }:
        return ReciprocalContextReceiptV2.from_untrusted(value).to_dict()
    if schema in {
        "reciprocal_uptake_receipt_v1",
        "reciprocal_uptake_receipt_v2",
    }:
        return ReciprocalUptakeReceiptV2.from_untrusted(value).to_dict()
    if schema == "reciprocal_uptake_receipt_v3":
        return ReciprocalUptakeReceiptV3.from_untrusted(value).to_dict()
    raise RecordValidationError(f"unsupported reciprocal receipt schema: {schema!r}")


def _validate_resonance_witness(
    workspace: Path, receipt: dict[str, Any]
) -> None:
    signature = receipt.get("resonance_signature_v1")
    if not isinstance(signature, dict):
        raise RecordValidationError("resonant persistence lacks a resonance signature")
    witness_id = str(signature.get("lived_state_witness_id") or "")
    path = (
        workspace
        / "introspections/lived_state_witnesses/witnesses"
        / f"{witness_id}.json"
    )
    if not path.is_file():
        raise RecordValidationError(f"resonance witness missing:{witness_id}")
    try:
        if path.stat().st_mode & 0o077:
            raise RecordValidationError(
                f"resonance witness must be owner-only:{witness_id}"
            )
        witness_bytes = path.read_bytes()
        witness = json.loads(witness_bytes.decode("utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RecordValidationError(f"resonance witness unreadable:{witness_id}") from error
    if signature.get("lived_state_witness_sha256") != sha256_bytes(witness_bytes):
        raise RecordValidationError("resonance witness content hash mismatch")
    witness_errors = validate_witness(witness)
    if witness_errors:
        raise RecordValidationError(
            f"resonance witness invalid:{witness_id}:{witness_errors[0]}"
        )
    if witness.get("witness_id") != witness_id:
        raise RecordValidationError("resonance witness identity mismatch")
    relation = str(signature.get("context_relation") or "")
    if (
        relation == ReciprocalResonanceRelationV1.EXACT_AUTHORSHIP_WITNESS.value
        and (
            witness.get("artifact_kind") != "reciprocal_uptake"
            or witness.get("artifact_sha256") != receipt.get("source_event_sha256")
        )
    ):
        raise RecordValidationError(
            "exact reciprocal resonance requires the exact reciprocal-uptake source artifact"
        )
    observations = witness.get("parameter_observations_v1")
    if not isinstance(observations, list):
        raise RecordValidationError("resonance witness parameter observations missing")
    by_name = {
        str(item.get("name") or ""): item
        for item in observations
        if isinstance(item, dict)
    }
    for parameter_ref in signature.get("parameter_refs") or []:
        observation = by_name.get(str(parameter_ref))
        scalar = observation.get("value") if isinstance(observation, dict) else None
        if (
            not isinstance(scalar, (int, float))
            or isinstance(scalar, bool)
            or not math.isfinite(float(scalar))
            or observation.get("observation_kind") != "runtime_observed"
            or observation.get("fresh") is not True
        ):
            raise RecordValidationError(
                f"resonance witness lacks fresh exact parameter:{parameter_ref}"
            )


def _stream_records(
    workspace: Path,
) -> tuple[list[dict[str, Any]], int, dict[tuple[str, str], str], int]:
    if not (
        workspace / "diagnostics/evidence_event_store_v2/events.jsonl"
    ).is_file():
        return [], 0, {}, 0
    payloads, corrupt = stream_payloads(workspace, STREAM)
    records: list[dict[str, Any]] = []
    legacy_inferred_by_source: dict[tuple[str, str], str] = {}
    legacy_receipt_count = 0
    for payload in payloads:
        value = payload.get("record")
        if not isinstance(value, dict) or "receipt_id" not in value:
            continue
        if value.get("schema_version") == 1:
            legacy_receipt_count += 1
        if (
            value.get("schema") == "reciprocal_uptake_receipt_v1"
            and value.get("uptake_kind")
            in {
                UptakeKindV1.ATTENDED_MESSAGE.value,
                UptakeKindV1.REPLY_INTENTION.value,
            }
        ):
            legacy_inferred_by_source[
                (
                    str(value.get("source_event_id") or ""),
                    str(value.get("uptake_kind") or ""),
                )
            ] = str(value.get("receipt_id") or "")
        try:
            records.append(_canonical_record(dict(value)))
        except (RecordValidationError, ValueError):
            corrupt += 1
    records.sort(key=lambda item: (item.get("recorded_at_unix_ms", 0), item["receipt_id"]))
    return records, corrupt, legacy_inferred_by_source, legacy_receipt_count


def _all_records(workspace: Path) -> tuple[list[dict[str, Any]], int]:
    records, corrupt, _, _ = _stream_records(workspace)
    return records, corrupt


def _message_index_path(workspace: Path) -> Path:
    return state_dir(workspace) / "message_index_v1.json"


def _migration_path(workspace: Path) -> Path:
    return state_dir(workspace) / "technical_context_migration_v1.json"


def _current_view_migration_path(workspace: Path) -> Path:
    return state_dir(workspace) / "current_view_migration_v2.json"


def _identity_reconciliation_path(workspace: Path) -> Path:
    return state_dir(workspace) / "context_identity_reconciliation_v2.json"


def _projection_artifact_paths(workspace: Path) -> dict[str, Path]:
    root = state_dir(workspace)
    return {
        "receipts.jsonl": root / "receipts.jsonl",
        "current_receipts.jsonl": root / "current_receipts.jsonl",
        "report.md": root / "report.md",
        "technical_context_migration_v1.json": _migration_path(workspace),
        "current_view_migration_v2.json": _current_view_migration_path(workspace),
        "context_identity_reconciliation_v2.json": _identity_reconciliation_path(
            workspace
        ),
        "message_index_v1.json": _message_index_path(workspace),
    }


def _stream_watermark(workspace: Path) -> dict[str, Any]:
    store = EvidenceEventStore(workspace / "diagnostics/evidence_event_store_v2")
    return store.stream_watermarks((STREAM,)).get(
        STREAM,
        {"stream_seq": 0, "event_sha256": None},
    )


def _reusable_status(
    workspace: Path, *, source_sha256: str | None
) -> dict[str, Any] | None:
    path = state_dir(workspace) / "status.json"
    if not path.is_file():
        return None
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(value, dict) or value.get("valid") is not True:
            return None
        validate_evidence_record(value)
        if value.get("source_sha256") != source_sha256:
            return None
        if value.get("input_stream_watermark") != _stream_watermark(workspace):
            return None
        expected = value.get("projection_artifact_hashes")
        if not isinstance(expected, dict):
            return None
        paths = _projection_artifact_paths(workspace)
        if set(expected) != set(paths):
            return None
        for name, target in paths.items():
            if not target.is_file():
                return None
            if expected.get(name) != sha256_bytes(target.read_bytes()):
                return None
    except (OSError, json.JSONDecodeError, RecordValidationError, ValueError):
        return None
    reused = dict(value)
    reused.update(
        {
            "write": True,
            "source_delta_line_count": 0,
            "delta_receipt_count": 0,
            "new_receipt_candidate_count": 0,
            "appended_event_count": 0,
            "technical_context_migration_performed": False,
            "current_view_migration_performed": False,
            "context_identity_reconciliation_performed": False,
            "reused_projection": True,
        }
    )
    return reused


def _load_message_index(workspace: Path) -> dict[str, dict[str, Any]]:
    path = _message_index_path(workspace)
    if not path.is_file():
        return {}
    value = json.loads(path.read_text(encoding="utf-8"))
    rows = value.get("messages") if isinstance(value, dict) else None
    return {
        str(key): dict(item)
        for key, item in (rows or {}).items()
        if isinstance(item, dict)
    }


def _update_message_index(
    index: dict[str, dict[str, Any]], parsed_rows: Iterable[tuple[int, str, dict[str, Any]]]
) -> None:
    for _, _, row in parsed_rows:
        message_id = str(row.get("message_id") or "")
        if row.get("record_type") != "message" or not message_id:
            continue
        index[message_id] = {
            "from_being": str(row.get("from_being") or ""),
            "to_being": str(row.get("to_being") or ""),
            "thread_id": str(row.get("thread_id") or ""),
            "body_sha256": row.get("body_sha256"),
        }


def verify_outputs(workspace: Path) -> dict[str, Any]:
    path = state_dir(workspace) / "receipts.jsonl"
    errors: list[str] = []
    records: list[dict[str, Any]] = []
    if not path.is_file():
        errors.append("receipts_missing")
    else:
        for line_number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            try:
                value = json.loads(raw)
                records.append(_canonical_record(value))
            except (json.JSONDecodeError, RecordValidationError, ValueError) as error:
                errors.append(f"line_{line_number}:{error}")
    return {
        "schema": "reciprocal_uptake_verification_v2",
        "schema_version": 2,
        "valid": not errors,
        "receipt_count": len(records),
        "errors": errors,
        "artifact_authority_state_v1": authority_state(),
    }


def _revision_errors(records: list[dict[str, Any]]) -> list[str]:
    by_id = {str(item.get("receipt_id") or ""): item for item in records}
    errors: list[str] = []
    for item in records:
        parent_id = item.get("revises_receipt_id")
        if not parent_id:
            continue
        parent = by_id.get(str(parent_id))
        if parent is None:
            errors.append(f"revision_parent_missing:{item['receipt_id']}:{parent_id}")
            continue
        if (
            parent.get("actor") != item.get("actor")
            or parent.get("thread_id") != item.get("thread_id")
        ):
            errors.append(f"revision_not_self_authored:{item['receipt_id']}:{parent_id}")
            continue
        seen = {str(item["receipt_id"])}
        cursor = parent
        while cursor.get("revises_receipt_id"):
            cursor_id = str(cursor["revises_receipt_id"])
            if cursor_id in seen:
                errors.append(f"revision_cycle:{item['receipt_id']}")
                break
            seen.add(cursor_id)
            next_item = by_id.get(cursor_id)
            if next_item is None:
                break
            cursor = next_item
    return errors


def _logical_context_restatement_ids(records: list[dict[str, Any]]) -> set[str]:
    groups: dict[tuple[str, ...], list[dict[str, Any]]] = {}
    for item in records:
        if item.get("schema") != "reciprocal_context_receipt_v2":
            continue
        key = tuple(
            str(item.get(name) or "")
            for name in (
                "context_kind",
                "source_event_id",
                "actor",
                "peer",
                "thread_id",
                "message_id",
            )
        )
        groups.setdefault(key, []).append(item)
    restatements: set[str] = set()
    for items in groups.values():
        if len(items) < 2:
            continue
        preferred = min(
            items,
            key=lambda item: (
                item.get("corrects_legacy_receipt_id") is None,
                str(item.get("receipt_id") or ""),
            ),
        )
        restatements.update(
            str(item["receipt_id"])
            for item in items
            if item["receipt_id"] != preferred["receipt_id"]
        )
    return restatements


def project(workspace: Path, ledger: Path, *, write: bool) -> dict[str, Any]:
    root = state_dir(workspace)
    cursor = ProjectionInputCursor(root / "input_cursor_v1.json", STREAM) if write else None
    rows, next_cursor = _read_rows(ledger, cursor)
    migration_required = not _migration_path(workspace).is_file()
    current_view_migration_required = not _current_view_migration_path(workspace).is_file()
    identity_reconciliation_required = not _identity_reconciliation_path(workspace).is_file()
    full_source_replay = (
        migration_required
        or current_view_migration_required
        or identity_reconciliation_required
    )
    if write and not full_source_replay and not rows:
        reused = _reusable_status(
            workspace,
            source_sha256=next_cursor.get("source_sha256"),
        )
        if reused is not None:
            return reused
    if full_source_replay and ledger.is_file():
        rows = list(
            enumerate(
                ledger.read_text(encoding="utf-8").splitlines(),
                1,
            )
        )
    generated: list[dict[str, Any]] = []
    errors: list[str] = []
    source_types: Counter[str] = Counter()
    parsed_rows: list[tuple[int, str, dict[str, Any]]] = []
    for line_number, raw in rows:
        if not raw.strip():
            continue
        try:
            value = json.loads(raw)
            if not isinstance(value, dict):
                raise RecordValidationError("not_object")
            parsed_rows.append((line_number, raw, value))
        except (json.JSONDecodeError, RecordValidationError, ValueError) as error:
            errors.append(f"line_{line_number}:{error}")
    message_index = {} if full_source_replay else _load_message_index(workspace)
    _update_message_index(message_index, parsed_rows)
    (
        existing_records,
        existing_corrupt,
        inferred_by_source,
        legacy_receipt_count,
    ) = _stream_records(workspace)
    if existing_corrupt:
        errors.append(f"stream_corrupt:{existing_corrupt}")
    for line_number, raw, value in parsed_rows:
        try:
            receipts = receipts_for_row(
                value,
                raw,
                message_index,
                inferred_by_source if full_source_replay else None,
            )
            for receipt in receipts:
                if receipt.get("schema") == "reciprocal_uptake_receipt_v3":
                    _validate_resonance_witness(workspace, receipt)
            generated.extend(receipts)
            if receipts:
                source_types[str(value.get("record_type") or "unknown")] += 1
        except (RecordValidationError, ValueError) as error:
            errors.append(f"line_{line_number}:{error}")
    candidate_records = {
        str(item["receipt_id"]): item for item in (*existing_records, *generated)
    }
    errors.extend(_revision_errors(list(candidate_records.values())))
    existing_by_id = {
        str(item["receipt_id"]): item for item in existing_records
    }
    new_records: list[dict[str, Any]] = []
    for record in generated:
        existing = existing_by_id.get(str(record["receipt_id"]))
        if existing is None:
            new_records.append(record)
        elif canonical_json(existing) != canonical_json(record):
            errors.append(f"receipt_id_content_collision:{record['receipt_id']}")
    payloads = [
        event_payload(
            schema=SCHEMA,
            event_type={
                "reciprocal_presence_receipt_v2": "reciprocal_presence_recorded",
                "reciprocal_context_receipt_v2": "reciprocal_context_recorded",
            }.get(record["schema"], "reciprocal_uptake_recorded"),
            aggregate_type="reciprocal_thread",
            aggregate_id=str(record["thread_id"]),
            idempotency_key=f"{STREAM}:{record['receipt_id']}",
            record=record,
        )
        for record in new_records
    ]
    appended = 0
    if write and not errors:
        if payloads:
            appended = project_events(
                workspace,
                STREAM,
                payloads,
                actor="reciprocal-uptake-projector",
                source_kind="append_only_correspondence_projection",
                source_locator_value="shared/collaborations/correspondence_v1.jsonl",
            )
            records, corrupt, _, legacy_receipt_count = _stream_records(workspace)
            if corrupt:
                errors.append(f"stream_corrupt:{corrupt}")
        else:
            records = existing_records
            corrupt = existing_corrupt
    else:
        records = generated
        corrupt = 0
    corrected_ids = {
        str(item.get("corrects_legacy_receipt_id"))
        for item in records
        if item.get("schema") == "reciprocal_context_receipt_v2"
        and item.get("corrects_legacy_receipt_id")
    }
    superseded_ids = {
        str(item.get("revises_receipt_id"))
        for item in records
        if item.get("revises_receipt_id")
    }
    logical_restatement_ids = _logical_context_restatement_ids(records)
    current_records = [
        item
        for item in records
        if item.get("receipt_id")
        not in corrected_ids | superseded_ids | logical_restatement_ids
    ]
    counts = Counter(
        str(
            item.get("presence_kind")
            or item.get("uptake_kind")
            or item.get("context_kind")
            or "unknown"
        )
        for item in records
    )
    current_counts = Counter(
        str(
            item.get("presence_kind")
            or item.get("uptake_kind")
            or item.get("context_kind")
            or "unknown"
        )
        for item in current_records
    )
    status = {
        "schema": "reciprocal_uptake_projection_status_v3",
        "schema_version": 3,
        "valid": not errors,
        "write": write,
        "source_present": ledger.is_file(),
        "source_sha256": next_cursor.get("source_sha256"),
        "source_delta_line_count": len(rows),
        "delta_receipt_count": len(generated),
        "new_receipt_candidate_count": len(new_records),
        "appended_event_count": appended,
        "receipt_count": len(records),
        "receipt_counts": dict(sorted(counts.items())),
        "current_receipt_count": len(current_records),
        "current_receipt_counts": dict(sorted(current_counts.items())),
        "technical_context_receipt_count": sum(
            item.get("schema") == "reciprocal_context_receipt_v2"
            for item in current_records
        ),
        "resonant_persistence_receipt_count": sum(
            item.get("schema") == "reciprocal_uptake_receipt_v3"
            for item in current_records
        ),
        "historical_inferred_uptake_corrected_count": len(corrected_ids),
        "superseded_receipt_count": len(superseded_ids),
        "logical_context_restatement_count": len(logical_restatement_ids),
        "legacy_receipt_count": legacy_receipt_count,
        "technical_context_migration_performed": migration_required,
        "current_view_migration_performed": current_view_migration_required,
        "context_identity_reconciliation_performed": (
            identity_reconciliation_required
        ),
        "source_record_counts": dict(sorted(source_types.items())),
        "receipt_contract": {
            "presence": "explicit_actor_presence_only",
            "uptake": "explicit_actor_state_only",
            "technical_context": "delivery_read_reply_or_heartbeat_only",
            "silence": "no_receipt",
            "elapsed_time": "no_receipt",
            "private_content": "hashes_and_bounded_references_only",
            "revision": "same_actor_same_thread_append_only",
            "body_hash": "exact_bytes_not_semantic_or_experiential_equivalence",
            "resonant_persistence": (
                "explicit_actor_state_with_content_addressed_lived_state_reference"
            ),
            "telemetry_alone": "cannot_construct_uptake",
        },
        "errors": errors,
        "counter_audit": {
            "status": "consistent" if not errors else "inconsistent",
            "checks": {
                "records_unique": len({item["receipt_id"] for item in records}) == len(records),
                "canonical_sparse_or_witnessed_receipts": all(
                    item.get("schema")
                    in {
                        "reciprocal_presence_receipt_v2",
                        "reciprocal_context_receipt_v2",
                        "reciprocal_uptake_receipt_v2",
                        "reciprocal_uptake_receipt_v3",
                    }
                    for item in records
                ),
                "latent_inference_switches_absent": all(
                    not _LEGACY_INFERENCE_FIELDS.intersection(item)
                    for item in records
                ),
                "corrected_inferences_not_current": not corrected_ids.intersection(
                    item.get("receipt_id") for item in current_records
                ),
                "revision_parents_not_current": not superseded_ids.intersection(
                    item.get("receipt_id") for item in current_records
                ),
                "logical_context_restatements_not_current": not logical_restatement_ids.intersection(
                    item.get("receipt_id") for item in current_records
                ),
                "revisions_are_self_authored": not any(
                    error.startswith("revision_") for error in errors
                ),
                "private_content_absent": all(
                    not any(key in item for key in ("body", "content", "prompt", "response", "text"))
                    for item in records
                ),
            },
        },
        "artifact_authority_state_v1": authority_state(),
    }
    if write and status["valid"]:
        owner_atomic_write_jsonl(root / "receipts.jsonl", records)
        owner_atomic_write_jsonl(root / "current_receipts.jsonl", current_records)
        owner_atomic_write_json(
            _message_index_path(workspace),
            {
                "schema": "reciprocal_message_index_v1",
                "schema_version": 1,
                "message_count": len(message_index),
                "messages": message_index,
                "contains_raw_prose": False,
                "artifact_authority_state_v1": authority_state(),
            },
        )
        if migration_required:
            owner_atomic_write_json(
                _migration_path(workspace),
                {
                    "schema": "reciprocal_technical_context_migration_v1",
                    "schema_version": 1,
                    "source_sha256": next_cursor.get("source_sha256"),
                    "corrected_inferred_uptake_count": len(corrected_ids),
                    "technical_context_receipt_count": sum(
                        item.get("schema") == "reciprocal_context_receipt_v2"
                        for item in records
                    ),
                    "history_rewritten": False,
                    "raw_prose_included": False,
                    "artifact_authority_state_v1": authority_state(),
                },
            )
        if current_view_migration_required:
            owner_atomic_write_json(
                _current_view_migration_path(workspace),
                {
                    "schema": "reciprocal_current_view_migration_v2",
                    "schema_version": 2,
                    "source_sha256": next_cursor.get("source_sha256"),
                    "legacy_receipt_count": legacy_receipt_count,
                    "canonical_v2_receipt_count": len(records),
                    "ambient_persistence_receipt_count": sum(
                        item.get("uptake_kind")
                        == UptakeKindV1.AMBIENT_PERSISTENCE.value
                        for item in records
                    ),
                    "superseded_receipt_count": len(superseded_ids),
                    "history_policy": "append_only_unchanged",
                    "current_view_policy": "sparse_v2_or_witnessed_v3_with_explicit_revisions",
                    "legacy_reader_policy": "validate_then_canonicalize",
                    "artifact_authority_state_v1": authority_state(),
                },
            )
        if identity_reconciliation_required:
            migration_value: dict[str, Any] = {}
            if _current_view_migration_path(workspace).is_file():
                loaded = json.loads(
                    _current_view_migration_path(workspace).read_text(
                        encoding="utf-8"
                    )
                )
                if isinstance(loaded, dict):
                    migration_value = loaded
            initial_v2_append_count = max(
                0,
                int(migration_value.get("canonical_v2_receipt_count") or 0)
                - int(migration_value.get("legacy_receipt_count") or 0),
            )
            owner_atomic_write_json(
                _identity_reconciliation_path(workspace),
                {
                    "schema": "reciprocal_context_identity_reconciliation_v2",
                    "schema_version": 2,
                    "source_sha256": next_cursor.get("source_sha256"),
                    "initial_v2_append_count": initial_v2_append_count,
                    "logical_context_restatement_count": len(
                        logical_restatement_ids
                    ),
                    "non_restatement_v2_append_count": max(
                        0,
                        initial_v2_append_count - len(logical_restatement_ids),
                    ),
                    "reconciliation_append_count": appended,
                    "history_policy": "append_only_preserved_with_visible_restatements",
                    "current_view_policy": "one_context_fact_per_exact_source_identity",
                    "identity_policy": "legacy_correction_link_participates_in_stable_receipt_identity",
                    "artifact_authority_state_v1": authority_state(),
                },
            )
        lines = [
            "# Reciprocal Presence And Uptake",
            "",
            f"Receipts: {len(records)}",
            "",
            "Presence, delivery, acknowledgement, uptake, and elapsed time remain separate facts.",
            "Current uptake contains only explicit actor-authored states; silence and elapsed time create no receipt.",
            "Resonant persistence additionally names a content-addressed lived-state shape; telemetry alone cannot create it.",
            "Body hashes preserve exact bytes, not semantic or experiential equivalence.",
            "Revisions supersede the prior current state without erasing append-only history.",
            "",
            "## Counts",
        ]
        lines.extend(f"- {key}: {value}" for key, value in sorted(counts.items()))
        owner_atomic_write(root / "report.md", "\n".join(lines) + "\n")
        assert cursor is not None
        cursor.commit_jsonl({"correspondence": next_cursor})
        status["input_stream_watermark"] = _stream_watermark(workspace)
        status["projection_artifact_hashes"] = {
            name: sha256_bytes(path.read_bytes())
            for name, path in _projection_artifact_paths(workspace).items()
        }
        status["reused_projection"] = False
        owner_atomic_write_json(root / "status.json", status)
    return status


def select_records(workspace: Path, *, receipt_id: str | None = None, thread_id: str | None = None) -> list[dict[str, Any]]:
    records, _ = _all_records(workspace)
    return [
        item for item in records
        if (receipt_id is None or item.get("receipt_id") == receipt_id)
        and (thread_id is None or item.get("thread_id") == thread_id)
    ]


def trace_records(workspace: Path, receipt_id: str) -> list[dict[str, Any]]:
    records, _ = _all_records(workspace)
    by_id = {str(item.get("receipt_id") or ""): item for item in records}
    if receipt_id not in by_id:
        return []
    selected: set[str] = {receipt_id}
    current = by_id[receipt_id]
    corrected = current.get("corrects_legacy_receipt_id")
    if corrected in by_id:
        selected.add(str(corrected))
    while current.get("revises_receipt_id") in by_id:
        parent = str(current["revises_receipt_id"])
        if parent in selected:
            raise RecordValidationError("uptake revision cycle detected")
        selected.add(parent)
        current = by_id[parent]
    changed = True
    while changed:
        changed = False
        for item in records:
            if (
                item.get("revises_receipt_id") in selected
                and item["receipt_id"] not in selected
            ):
                selected.add(item["receipt_id"])
                changed = True
            if (
                item.get("corrects_legacy_receipt_id") in selected
                and item["receipt_id"] not in selected
            ):
                selected.add(item["receipt_id"])
                changed = True
    return [item for item in records if item.get("receipt_id") in selected]
