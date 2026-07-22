"""Project only explicit reciprocal presence and uptake actions."""

from __future__ import annotations

import json
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
    )
    from projection_cursors import ProjectionInputCursor
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
    )
    from scripts.projection_cursors import ProjectionInputCursor

from .model import (
    PresenceKindV1,
    ReciprocalContextKindV1,
    ReciprocalContextReceiptV1,
    ReciprocalPresenceReceiptV1,
    ReciprocalUptakeReceiptV1,
    UptakeKindV1,
    build_context_receipt,
    build_presence_receipt,
    build_uptake_receipt,
)

STREAM = "reciprocal_uptake"
SCHEMA = "reciprocal_uptake_domain_event_v1"
STATE_DIRNAME = "reciprocal_uptake_v1"


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
                corrects_inferred_uptake_receipt_id=(
                    inferred_by_source or {}
                ).get((source_event_id, UptakeKindV1.ATTENDED_MESSAGE.value)),
                **_common(row, raw, actor=reader, peer=source_actor),
            ).to_dict()
        )
    if record_type == "reply_link":
        result.append(
            build_context_receipt(
                ReciprocalContextKindV1.REPLY_LINK,
                corrects_inferred_uptake_receipt_id=(
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
    }.get(explicit_value)
    if explicit is not None:
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


def _all_records(workspace: Path) -> tuple[list[dict[str, Any]], int]:
    if not (
        workspace / "diagnostics/evidence_event_store_v2/events.jsonl"
    ).is_file():
        return [], 0
    payloads, corrupt = stream_payloads(workspace, STREAM)
    records = [
        dict(payload["record"])
        for payload in payloads
        if isinstance(payload.get("record"), dict)
    ]
    records.sort(key=lambda item: (item.get("recorded_at_unix_ms", 0), item["receipt_id"]))
    return records, corrupt


def _message_index_path(workspace: Path) -> Path:
    return state_dir(workspace) / "message_index_v1.json"


def _migration_path(workspace: Path) -> Path:
    return state_dir(workspace) / "technical_context_migration_v1.json"


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
                if value.get("schema") == "reciprocal_presence_receipt_v1":
                    ReciprocalPresenceReceiptV1.from_untrusted(value)
                elif value.get("schema") == "reciprocal_context_receipt_v1":
                    ReciprocalContextReceiptV1.from_untrusted(value)
                else:
                    ReciprocalUptakeReceiptV1.from_untrusted(value)
                records.append(value)
            except (json.JSONDecodeError, RecordValidationError, ValueError) as error:
                errors.append(f"line_{line_number}:{error}")
    return {
        "schema": "reciprocal_uptake_verification_v1",
        "schema_version": 1,
        "valid": not errors,
        "receipt_count": len(records),
        "errors": errors,
        "artifact_authority_state_v1": authority_state(),
    }


def project(workspace: Path, ledger: Path, *, write: bool) -> dict[str, Any]:
    root = state_dir(workspace)
    cursor = ProjectionInputCursor(root / "input_cursor_v1.json", STREAM) if write else None
    rows, next_cursor = _read_rows(ledger, cursor)
    migration_required = not _migration_path(workspace).is_file()
    if migration_required and ledger.is_file():
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
    message_index = {} if migration_required else _load_message_index(workspace)
    _update_message_index(message_index, parsed_rows)
    existing_records, existing_corrupt = _all_records(workspace)
    if existing_corrupt:
        errors.append(f"stream_corrupt:{existing_corrupt}")
    inferred_by_source = {
        (str(record.get("source_event_id") or ""), str(record.get("uptake_kind") or "")): str(record.get("receipt_id") or "")
        for record in existing_records
        if record.get("schema") == "reciprocal_uptake_receipt_v1"
        and record.get("uptake_kind") in {
            UptakeKindV1.ATTENDED_MESSAGE.value,
            UptakeKindV1.REPLY_INTENTION.value,
        }
    }
    for line_number, raw, value in parsed_rows:
        try:
            receipts = receipts_for_row(
                value,
                raw,
                message_index,
                inferred_by_source if migration_required else None,
            )
            generated.extend(receipts)
            if receipts:
                source_types[str(value.get("record_type") or "unknown")] += 1
        except (RecordValidationError, ValueError) as error:
            errors.append(f"line_{line_number}:{error}")
    payloads = [
        event_payload(
            schema=SCHEMA,
            event_type={
                "reciprocal_presence_receipt_v1": "reciprocal_presence_recorded",
                "reciprocal_context_receipt_v1": "reciprocal_context_recorded",
            }.get(record["schema"], "reciprocal_uptake_recorded"),
            aggregate_type="reciprocal_thread",
            aggregate_id=str(record["thread_id"]),
            idempotency_key=f"{STREAM}:{record['receipt_id']}",
            record=record,
        )
        for record in generated
    ]
    appended = 0
    if write and not errors:
        appended = project_events(
            workspace,
            STREAM,
            payloads,
            actor="reciprocal-uptake-projector",
            source_kind="append_only_correspondence_projection",
            source_locator_value="shared/collaborations/correspondence_v1.jsonl",
        )
        records, corrupt = _all_records(workspace)
        if corrupt:
            errors.append(f"stream_corrupt:{corrupt}")
    else:
        records = generated
        corrupt = 0
    corrected_ids = {
        str(item.get("corrects_inferred_uptake_receipt_id"))
        for item in records
        if item.get("schema") == "reciprocal_context_receipt_v1"
        and item.get("corrects_inferred_uptake_receipt_id")
    }
    current_records = [
        item for item in records if item.get("receipt_id") not in corrected_ids
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
        "schema": "reciprocal_uptake_projection_status_v1",
        "schema_version": 1,
        "valid": not errors,
        "write": write,
        "source_present": ledger.is_file(),
        "source_sha256": next_cursor.get("source_sha256"),
        "source_delta_line_count": len(rows),
        "delta_receipt_count": len(generated),
        "appended_event_count": appended,
        "receipt_count": len(records),
        "receipt_counts": dict(sorted(counts.items())),
        "current_receipt_count": len(current_records),
        "current_receipt_counts": dict(sorted(current_counts.items())),
        "technical_context_receipt_count": sum(
            item.get("schema") == "reciprocal_context_receipt_v1"
            for item in current_records
        ),
        "historical_inferred_uptake_corrected_count": len(corrected_ids),
        "technical_context_migration_performed": migration_required,
        "source_record_counts": dict(sorted(source_types.items())),
        "presence_infers_uptake": False,
        "silence_infers_uptake": False,
        "elapsed_time_infers_uptake": False,
        "raw_prose_included": False,
        "errors": errors,
        "counter_audit": {
            "status": "consistent" if not errors else "inconsistent",
            "checks": {
                "records_unique": len({item["receipt_id"] for item in records}) == len(records),
                "presence_distinct_from_uptake": all(
                    item.get("uptake_inferred") is False
                    for item in records
                    if item.get("schema") == "reciprocal_presence_receipt_v1"
                ),
                "technical_context_does_not_infer_uptake": all(
                    item.get("uptake_inferred") is False
                    and item.get("reply_intention_inferred") is False
                    for item in records
                    if item.get("schema") == "reciprocal_context_receipt_v1"
                ),
                "corrected_inferences_not_current": not corrected_ids.intersection(
                    item.get("receipt_id") for item in current_records
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
        owner_atomic_write_json(root / "status.json", status)
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
                        item.get("schema") == "reciprocal_context_receipt_v1"
                        for item in records
                    ),
                    "history_rewritten": False,
                    "raw_prose_included": False,
                    "artifact_authority_state_v1": authority_state(),
                },
            )
        lines = [
            "# Reciprocal Presence And Uptake",
            "",
            f"Receipts: {len(records)}",
            "",
            "Presence, delivery, acknowledgement, uptake, and elapsed time remain separate facts.",
            "Silence never supplies uptake or consent.",
            "",
            "## Counts",
        ]
        lines.extend(f"- {key}: {value}" for key, value in sorted(counts.items()))
        owner_atomic_write(root / "report.md", "\n".join(lines) + "\n")
        assert cursor is not None
        cursor.commit_jsonl({"correspondence": next_cursor})
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
    corrected = current.get("corrects_inferred_uptake_receipt_id")
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
                item.get("corrects_inferred_uptake_receipt_id") in selected
                and item["receipt_id"] not in selected
            ):
                selected.add(item["receipt_id"])
                changed = True
    return [item for item in records if item.get("receipt_id") in selected]
