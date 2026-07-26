"""Validate the shared, evidence-only ESN Division ceremony ledger."""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any, Mapping

try:
    from experiential_systems.common import (
        RecordValidationError,
        load_jsonl,
        validate_evidence_record,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError,
        load_jsonl,
        validate_evidence_record,
    )


WRITE_ACTIONS = {
    "DIVISION_INTENT",
    "DIVISION_ASSENT",
    "DIVISION_WITHDRAW_ASSENT",
    "DIVISION_RETURN_REQUEST",
    "DIVISION_REVIEW",
}
FALSE_BOUNDARIES = {
    "presence_inferred",
    "peer_consent_inferred",
    "silence_infers_consent",
    "native_assent_changed",
    "division_stage_changed",
    "prepare_dispatched",
    "commit_recommended",
    "commit_dispatched",
    "rollback_dispatched",
    "return_transition_dispatched",
    "scheduler_effect",
    "model_qos_effect",
    "substrate_effect",
    "dispatch_effect",
    "live_control_effect",
    "raw_prose_included",
}


def _event_id(value: Mapping[str, Any]) -> str:
    candidate = value.get("candidate")
    candidate = candidate if isinstance(candidate, Mapping) else {}
    ordered = (
        value.get("actor"),
        value.get("action"),
        candidate.get("division_id"),
        candidate.get("parent_generation"),
        candidate.get("plan_digest"),
        candidate.get("selected_strategy"),
        value.get("source_ref"),
        value.get("recorded_at_unix_ms"),
        value.get("expires_at_unix_ms"),
        value.get("previous_actor_event_id"),
        value.get("targets_event_id"),
        value.get("native_status_hash"),
        value.get("readiness_receipt_ref"),
        value.get("readiness_receipt_hash"),
        ",".join(str(item) for item in (value.get("snapshot_refs") or [])),
        value.get("current_tick"),
        value.get("rollback_deadline_tick"),
        value.get("review_outcome"),
    )
    identity = "|".join("" if item is None else str(item) for item in ordered)
    digest = hashlib.sha256(identity.encode()).hexdigest()[:24]
    return f"division_ceremony_{digest}"


def validate_division_ceremony_event(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise RecordValidationError("division ceremony row must be an object")
    validate_evidence_record(value)
    if (
        value.get("schema") != "division.ceremony_event.v1"
        or value.get("record_type") != "division_ceremony_event"
        or value.get("action") not in WRITE_ACTIONS
        or value.get("owner_language_action") != value.get("action")
    ):
        raise RecordValidationError("division ceremony schema or action mismatch")
    event_id = str(value.get("ceremony_event_id") or "")
    if (
        not event_id
        or value.get("record_id") != event_id
        or event_id != _event_id(value)
    ):
        raise RecordValidationError("division ceremony deterministic identity mismatch")
    if (
        value.get("actor") not in {"astrid", "minime"}
        or value.get("self_authored_only") is not True
        or value.get("response_revisable") is not True
        or value.get("right_to_ignore") is not True
    ):
        raise RecordValidationError("division ceremony self-authorship mismatch")
    for field in FALSE_BOUNDARIES:
        if value.get(field) is not False:
            raise RecordValidationError(
                f"division ceremony contains forbidden {field}"
            )
    candidate = value.get("candidate")
    if not isinstance(candidate, dict) or any(
        not candidate.get(field)
        for field in ("division_id", "plan_digest", "selected_strategy")
    ):
        raise RecordValidationError("division ceremony candidate is incomplete")
    if not isinstance(candidate.get("parent_generation"), int):
        raise RecordValidationError("division candidate generation is invalid")
    return dict(value)


def load_division_ceremony(
    ledger: Path | None,
) -> tuple[list[dict[str, Any]], list[str]]:
    if ledger is None or not ledger.is_file():
        return [], []
    rows, errors = load_jsonl(ledger)
    records: list[dict[str, Any]] = []
    latest_by_actor: dict[str, str] = {}
    assent_by_id: dict[str, dict[str, Any]] = {}
    for index, row in enumerate(rows, 1):
        try:
            record = validate_division_ceremony_event(row)
            actor = str(record["actor"])
            if record.get("previous_actor_event_id") != latest_by_actor.get(actor):
                raise RecordValidationError(
                    "division ceremony actor lineage mismatch"
                )
            if record["action"] == "DIVISION_ASSENT":
                assent_by_id[record["ceremony_event_id"]] = record
            if record["action"] == "DIVISION_WITHDRAW_ASSENT":
                target = assent_by_id.get(str(record.get("targets_event_id") or ""))
                if target is None or target.get("actor") != actor:
                    raise RecordValidationError(
                        "division assent withdrawal is not self-owned"
                    )
            latest_by_actor[actor] = record["ceremony_event_id"]
            records.append(record)
        except (RecordValidationError, TypeError, ValueError) as error:
            errors.append(f"division_ceremony_{index}:{error}")
    return records, errors
