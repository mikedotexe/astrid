"""Project an external bounded work selection from Felt Contracts."""

from __future__ import annotations

import json
import re
from datetime import datetime
from pathlib import Path
from typing import Any

try:
    from attention_portfolio.model import BeingImportancePinV1
    from experiential_systems.common import (
        RecordValidationError,
        authority_state,
        deterministic_id,
        event_payload,
        load_jsonl,
        owner_append_jsonl,
        owner_atomic_write,
        owner_atomic_write_json,
        project_events,
        sha256_bytes,
        utc_now,
    )
except ModuleNotFoundError:
    from scripts.attention_portfolio.model import BeingImportancePinV1
    from scripts.experiential_systems.common import (
        RecordValidationError,
        authority_state,
        deterministic_id,
        event_payload,
        load_jsonl,
        owner_append_jsonl,
        owner_atomic_write,
        owner_atomic_write_json,
        project_events,
        sha256_bytes,
        utc_now,
    )

from .model import (
    OwnerPriorityPinV1,
    StewardWorkSelectionEntryV1,
    StewardWorkSelectionReceiptV1,
    StewardWorkSelectionV1,
)

STREAM = "steward_work_selection"
SCHEMA = "steward_work_selection_domain_event_v1"
TIMESTAMP_RE = re.compile(r"_(\d{9,})")
POLICY_SPEC = {
    "schema": "steward_work_selection_policy_v1",
    "source_objection_or_reopen_slots": 4,
    "astrid_priority_slots": 2,
    "minime_priority_slots": 2,
    "steward_work_batch": 16,
    "rank_fields": [
        "source_objection_or_reopen",
        "explicit_owner_priority",
        "claim_recurrence",
        "source_freshness",
        "steward_unaddressed_duration",
        "durable_queue_position",
        "contract_id",
    ],
}
POLICY_ID = deterministic_id("stewardworkpolicy", POLICY_SPEC)


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/steward_work_selection_v1"


def pin_path(workspace: Path) -> Path:
    return state_dir(workspace) / "priority_events.jsonl"


def append_pin(
    workspace: Path, pin: OwnerPriorityPinV1, actor: str
) -> dict[str, Any]:
    if actor != pin.owner:
        raise RecordValidationError("an owner may set priority only for itself")
    core = {"pin": pin.to_dict(), "actor": actor}
    event = {
        "schema": "owner_priority_event_v1",
        "schema_version": 1,
        "event_id": deterministic_id("ownerpriorityevent", core),
        "actor": actor,
        "recorded_at": utc_now(),
        "pin": pin.to_dict(),
        "artifact_authority_state_v1": authority_state(),
    }
    owner_append_jsonl(pin_path(workspace), event)
    return event


def _legacy_pin_paths(workspace: Path) -> tuple[Path, ...]:
    diagnostics = workspace / "diagnostics"
    return (
        diagnostics / "attention_portfolio_v1/pin_events.jsonl",
        diagnostics / "attention_portfolio_v2/pin_events.jsonl",
    )


def _translated_legacy_pin(value: Any) -> OwnerPriorityPinV1:
    legacy = BeingImportancePinV1.from_untrusted(value)
    return OwnerPriorityPinV1.build(
        owner=legacy.being,
        contract_id=legacy.contract_id,
        action=legacy.action,
        source_event_id=legacy.source_event_id,
        source_event_sha256=legacy.source_event_sha256,
    )


def replay_pins(
    workspace: Path,
) -> tuple[dict[str, dict[str, OwnerPriorityPinV1]], list[OwnerPriorityPinV1], list[str]]:
    active: dict[str, dict[str, OwnerPriorityPinV1]] = {"astrid": {}, "minime": {}}
    records: list[OwnerPriorityPinV1] = []
    errors: list[str] = []
    sources = [(path, True) for path in _legacy_pin_paths(workspace)]
    sources.append((pin_path(workspace), False))
    for path, legacy in sources:
        rows, row_errors = load_jsonl(path)
        errors.extend(f"{path.name}:{error}" for error in row_errors)
        for index, event in enumerate(rows, 1):
            try:
                pin = (
                    _translated_legacy_pin(event.get("pin"))
                    if legacy
                    else OwnerPriorityPinV1.from_untrusted(event.get("pin"))
                )
                if event.get("actor") not in {pin.owner, None}:
                    raise RecordValidationError("priority actor mismatch")
                if pin.action == "pin":
                    active[pin.owner][pin.contract_id] = pin
                else:
                    active[pin.owner].pop(pin.contract_id, None)
                records.append(pin)
            except (RecordValidationError, ValueError, TypeError, AttributeError) as error:
                errors.append(f"{path.name}:{index}:{error}")
    return active, records, errors


def _contracts(workspace: Path) -> tuple[list[dict[str, Any]], bytes]:
    path = workspace / "diagnostics/felt_contract_graph_v1/contracts.jsonl"
    raw = path.read_bytes()
    rows = [json.loads(line) for line in raw.decode().splitlines() if line.strip()]
    return rows, raw


def _unix_ms(value: Any) -> int:
    try:
        return int(datetime.fromisoformat(str(value)).timestamp() * 1000)
    except (TypeError, ValueError):
        return 0


def _queue_position(contract: dict[str, Any]) -> int:
    match = TIMESTAMP_RE.search(str(contract.get("anchor_claim_id") or ""))
    return int(match.group(1)) if match else 2**63 - 1


def _source_priority(contract: dict[str, Any]) -> int:
    review = str(contract.get("felt_review") or "")
    activity = str(contract.get("activity") or "")
    if review in {"contradicted", "still_friction", "objection"}:
        return 2
    if activity == "reopened" or int(contract.get("reopen_count") or 0) > 0:
        return 2
    if int(contract.get("contradiction_count") or 0) > 0:
        return 2
    if activity == "review_pending" or review == "awaiting":
        return 1
    return 0


def select_work(
    workspace: Path,
) -> tuple[StewardWorkSelectionV1, list[OwnerPriorityPinV1], list[dict[str, Any]], list[str]]:
    contracts, raw = _contracts(workspace)
    pins, pin_records, errors = replay_pins(workspace)
    latest_ms = max((_unix_ms(item.get("last_change_at")) for item in contracts), default=0)
    eligible: list[dict[str, Any]] = []
    for contract in contracts:
        contract_id = str(contract.get("contract_id") or "")
        pinned_by = tuple(
            owner for owner in ("astrid", "minime") if contract_id in pins[owner]
        )
        if contract.get("felt_closed") and str(contract.get("activity")) != "reopened":
            continue
        if str(contract.get("activity")) == "quiet_archived" and not pinned_by:
            continue
        changed = _unix_ms(contract.get("last_change_at"))
        eligible.append(
            {
                "contract": contract,
                "contract_id": contract_id,
                "source_priority": _source_priority(contract),
                "claim_recurrence": int(contract.get("claim_count") or 0),
                "source_recent": bool(
                    latest_ms and latest_ms - changed <= 24 * 60 * 60 * 1000
                ),
                "unaddressed_ms": max(0, latest_ms - changed),
                "queue": _queue_position(contract),
                "pinned_by": pinned_by,
            }
        )

    def rank(item: dict[str, Any]) -> tuple[Any, ...]:
        return (
            -item["source_priority"],
            -int(bool(item["pinned_by"])),
            -item["claim_recurrence"],
            -int(item["source_recent"]),
            -item["unaddressed_ms"],
            item["queue"],
            item["contract_id"],
        )

    ranked = sorted(eligible, key=rank)
    source_urgent = [item for item in ranked if item["source_priority"] >= 2]
    selected: list[tuple[dict[str, Any], str, str]] = []
    for item in source_urgent[:4]:
        selected.append(
            (
                item,
                "source_objection_or_reopen",
                str(item["contract"].get("anchor_claim_id") or item["contract_id"]),
            )
        )
    selected_ids = {item["contract_id"] for item, _, _ in selected}
    alerts = [item["contract_id"] for item in source_urgent[4:]]
    alert_ids = set(alerts)
    for owner in ("astrid", "minime"):
        candidates = [
            item
            for item in ranked
            if item["contract_id"] in pins[owner]
            and item["contract_id"] not in selected_ids
            and item["contract_id"] not in alert_ids
        ]
        for item in candidates[:2]:
            pin = pins[owner][item["contract_id"]]
            selected.append((item, f"{owner}_priority_pin", pin.pin_id))
            selected_ids.add(item["contract_id"])
    for item in ranked:
        if len(selected) >= 16:
            break
        if item["contract_id"] in selected_ids or item["contract_id"] in alert_ids:
            continue
        selected.append(
            (
                item,
                "deterministic_steward_policy",
                str(item["contract"].get("anchor_claim_id") or item["contract_id"]),
            )
        )
        selected_ids.add(item["contract_id"])
    entries = [
        StewardWorkSelectionEntryV1.build(
            contract_id=item["contract_id"],
            selection_source=selection_source,
            source_ref=source_ref,
        )
        for item, selection_source, source_ref in selected
    ]
    selection = StewardWorkSelectionV1.build(
        policy_id=POLICY_ID,
        source_contracts_sha256=sha256_bytes(raw),
        selected_entries=entries,
        visible_urgent_alert_contract_ids=alerts,
    )
    return selection, pin_records, contracts, errors


def project(workspace: Path, *, write: bool) -> dict[str, Any]:
    selection, pin_records, contracts, errors = select_work(workspace)
    receipt = StewardWorkSelectionReceiptV1.from_selection(selection)
    records = [pin.to_dict() for pin in pin_records]
    records.extend((selection.to_dict(), receipt.to_dict()))
    payloads = []
    for record in records:
        identifier = str(
            record.get("pin_id")
            or record.get("receipt_id")
            or record.get("selection_id")
        )
        payloads.append(
            event_payload(
                schema=SCHEMA,
                event_type=f"{record['schema']}_recorded",
                aggregate_type="steward_work_selection",
                aggregate_id=selection.selection_id,
                idempotency_key=f"{STREAM}:{identifier}",
                record=record,
            )
        )
    appended = (
        project_events(
            workspace,
            STREAM,
            payloads,
            actor="steward-work-selection-projector",
            source_kind="external_steward_work_selection",
            source_locator_value="diagnostics/felt_contract_graph_v1/contracts.jsonl",
        )
        if write and not errors
        else 0
    )
    selected_ids = [item.contract_id for item in selection.selected_entries]
    status = {
        "schema": "steward_work_selection_status_v1",
        "schema_version": 1,
        "valid": not errors,
        "write": write,
        "contract_count": len(contracts),
        "selected_contract_count": len(selected_ids),
        "visible_urgent_alert_count": len(
            selection.visible_urgent_alert_contract_ids
        ),
        "appended_event_count": appended,
        "policy_id": POLICY_ID,
        "selection_scope": "steward_work_selection_not_being_state",
        "experiential_relation": "no_claim_about_being_felt_effect",
        "runtime_relation": (
            "not_consumed_by_bridge_minime_model_scheduler_or_control_runtime"
        ),
        "authority_relation": "selection_grants_no_runtime_authority",
        "historical_attention_stream_relation": "read_only_compatibility_evidence",
        "errors": errors,
        "counter_audit": {
            "status": "consistent" if not errors else "inconsistent",
            "checks": {
                "external_steward_batch_respected": len(selected_ids) <= 16,
                "selected_ids_unique": len(set(selected_ids)) == len(selected_ids),
                "urgent_alerts_not_selected": not set(
                    selection.visible_urgent_alert_contract_ids
                ).intersection(selected_ids),
            },
        },
        "artifact_authority_state_v1": authority_state(),
    }
    if write and status["valid"]:
        output = state_dir(workspace)
        owner_atomic_write_json(output / "selection.json", selection.to_dict())
        owner_atomic_write_json(output / "status.json", status)
        owner_atomic_write(
            output / "report.md",
            "# External Steward Work Selection\n\n"
            "This artifact lists contracts selected for steward work. It does not "
            "describe a being's attention, orientation, capacity, pressure, or felt "
            "effects. Historical attention-portfolio artifacts remain read-only "
            "compatibility evidence.\n\n"
            + "\n".join(
                f"- {item.contract_id} ({item.selection_source})"
                for item in selection.selected_entries
            )
            + "\n",
        )
    return status


def query(workspace: Path, contract_id: str | None) -> dict[str, Any]:
    path = state_dir(workspace) / "selection.json"
    if not path.is_file():
        return {"valid": False, "error": "selection_missing"}
    value = StewardWorkSelectionV1.from_untrusted(json.loads(path.read_text())).to_dict()
    if contract_id:
        value["selected_entries"] = [
            item
            for item in value.get("selected_entries", [])
            if item.get("contract_id") == contract_id
        ]
    return value
