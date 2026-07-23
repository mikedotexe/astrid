"""Owner-only persistence for study events, capture windows, and scalar samples."""

from __future__ import annotations

import fcntl
import json
import os
from pathlib import Path
from typing import Any

try:
    from experiential_systems.common import (
        RecordValidationError,
        authority_state,
        deterministic_id,
        load_jsonl,
        owner_append_jsonl,
        owner_atomic_write_json,
        reject_private_content,
        utc_now,
        validate_bounded_identifier,
        validate_evidence_record,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError,
        authority_state,
        deterministic_id,
        load_jsonl,
        owner_append_jsonl,
        owner_atomic_write_json,
        reject_private_content,
        utc_now,
        validate_bounded_identifier,
        validate_evidence_record,
    )

from .config import state_dir
from .model import StudyWindowSpecV1


def operator_path(workspace: Path) -> Path:
    return state_dir(workspace) / "operator_events.jsonl"


def active_windows_path(workspace: Path) -> Path:
    return state_dir(workspace) / "active_windows.json"


def samples_path(workspace: Path, window_id: str) -> Path:
    bounded = validate_bounded_identifier(window_id, "window_id") or ""
    return state_dir(workspace) / "samples" / f"{bounded}.jsonl"


def append_event(
    workspace: Path,
    event_type: str,
    record: dict[str, Any],
    actor: str,
) -> dict[str, Any]:
    validate_evidence_record(record)
    bounded_type = validate_bounded_identifier(event_type, "event_type") or ""
    bounded_actor = validate_bounded_identifier(actor, "actor", limit=120) or ""
    core = {
        "event_type": bounded_type,
        "record": record,
        "actor": bounded_actor,
    }
    value = {
        "schema": "evidence_study_operator_event_v1",
        "schema_version": 1,
        "event_id": deterministic_id("studyevent", core),
        "event_type": bounded_type,
        "actor": bounded_actor,
        "recorded_at": utc_now(),
        "record": record,
        "raw_prose_included": False,
        "artifact_authority_state_v1": authority_state(),
    }
    owner_append_jsonl(operator_path(workspace), value)
    return value


def load_events(workspace: Path) -> tuple[list[dict[str, Any]], list[str]]:
    return load_jsonl(operator_path(workspace))


def _load_active(path: Path) -> dict[str, Any]:
    if not path.is_file():
        return {
            "schema": "active_study_windows_v1",
            "schema_version": 1,
            "windows": {},
            "artifact_authority_state_v1": authority_state(),
        }
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict) or not isinstance(value.get("windows"), dict):
        raise RecordValidationError("active study window registry is invalid")
    validate_evidence_record(value)
    return value


def active_windows(workspace: Path) -> dict[str, StudyWindowSpecV1]:
    value = _load_active(active_windows_path(workspace))
    result: dict[str, StudyWindowSpecV1] = {}
    for window_id, record in value["windows"].items():
        item = StudyWindowSpecV1.from_untrusted(record)
        if item.window_id != window_id:
            raise RecordValidationError("active window registry identity mismatch")
        result[window_id] = item
    return result


def arm_window(workspace: Path, spec: StudyWindowSpecV1) -> None:
    path = active_windows_path(workspace)
    path.parent.mkdir(parents=True, exist_ok=True)
    os.chmod(path.parent, 0o700)
    lock_path = path.with_name(".active_windows.lock")
    with lock_path.open("a+", encoding="utf-8") as lock:
        os.chmod(lock_path, 0o600)
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        value = _load_active(path)
        windows = dict(value["windows"])
        for existing in windows.values():
            current = StudyWindowSpecV1.from_untrusted(existing)
            if set(current.sample_kinds).intersection(spec.sample_kinds):
                raise RecordValidationError(
                    "an active study window already owns this sample kind"
                )
        windows[spec.window_id] = spec.to_dict()
        value["windows"] = dict(sorted(windows.items()))
        owner_atomic_write_json(path, value)
        fcntl.flock(lock.fileno(), fcntl.LOCK_UN)


def disarm_window(workspace: Path, window_id: str) -> StudyWindowSpecV1:
    path = active_windows_path(workspace)
    path.parent.mkdir(parents=True, exist_ok=True)
    lock_path = path.with_name(".active_windows.lock")
    with lock_path.open("a+", encoding="utf-8") as lock:
        os.chmod(lock_path, 0o600)
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        value = _load_active(path)
        windows = dict(value["windows"])
        raw = windows.pop(window_id, None)
        if raw is None:
            raise RecordValidationError("study capture window is not active")
        owner_atomic_write_json(path, {**value, "windows": windows})
        fcntl.flock(lock.fileno(), fcntl.LOCK_UN)
    return StudyWindowSpecV1.from_untrusted(raw)


def load_samples(
    workspace: Path, window_id: str
) -> tuple[list[dict[str, Any]], list[str]]:
    rows, errors = load_jsonl(samples_path(workspace, window_id))
    validated: list[dict[str, Any]] = []
    for index, row in enumerate(rows, 1):
        try:
            reject_private_content(row)
            validate_evidence_record(row)
            if row.get("window_id") != window_id:
                raise RecordValidationError("sample belongs to another window")
            if row.get("schema") == "study_capture_gap_receipt_v1":
                validated.append(row)
                continue
            if row.get("sample_kind") not in {
                "telemetry",
                "heartbeat",
                "codec_lane",
                "codec_gate",
            }:
                raise RecordValidationError("unsupported sample kind")
            validated.append(row)
        except (RecordValidationError, TypeError, ValueError) as error:
            errors.append(f"sample_{index}:{error}")
    return validated, errors
