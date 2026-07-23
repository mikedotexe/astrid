"""Select a bounded steward work view over Felt Contracts."""

from __future__ import annotations

import json
import re
from collections import defaultdict
from datetime import datetime
from pathlib import Path
from typing import Any

try:
    from experiential_systems.common import (
        RecordValidationError, authority_state, canonical_json, deterministic_id,
        event_payload, load_jsonl, owner_append_jsonl, owner_atomic_write,
        owner_atomic_write_json, project_events, sha256_bytes, stream_payloads,
        utc_now,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError, authority_state, canonical_json, deterministic_id,
        event_payload, load_jsonl, owner_append_jsonl, owner_atomic_write,
        owner_atomic_write_json, project_events, sha256_bytes, stream_payloads,
        utc_now,
    )

from .model import (
    AttentionPortfolioEntryV2,
    AttentionPortfolioV1,
    AttentionPortfolioV2,
    AttentionSelectionReceiptV2,
    BeingImportancePinV1,
    steward_unaddressed_age_band,
)

STREAM = "attention_portfolio"
SCHEMA = "attention_portfolio_domain_event_v1"
TIMESTAMP_RE = re.compile(r"_(\d{9,})")


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/attention_portfolio_v2"


def legacy_state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/attention_portfolio_v1"


def pin_path(workspace: Path) -> Path:
    return state_dir(workspace) / "pin_events.jsonl"


def append_pin(workspace: Path, pin: BeingImportancePinV1, actor: str) -> dict[str, Any]:
    if actor != pin.being: raise RecordValidationError("a being may pin only for itself")
    core = {"pin": pin.to_dict(), "actor": actor}
    event = {"schema": "attention_pin_event_v1", "schema_version": 1,
             "event_id": deterministic_id("attentionpinevent", core),
             "actor": actor, "recorded_at": utc_now(), "pin": pin.to_dict(),
             "artifact_authority_state_v1": authority_state()}
    owner_append_jsonl(pin_path(workspace), event)
    return event


def replay_pins(workspace: Path) -> tuple[dict[str, set[str]], list[dict[str, Any]], list[str]]:
    legacy_rows, legacy_errors = load_jsonl(legacy_state_dir(workspace) / "pin_events.jsonl")
    rows, errors = load_jsonl(pin_path(workspace))
    rows = legacy_rows + rows
    errors = legacy_errors + errors
    active: dict[str, set[str]] = {"astrid": set(), "minime": set()}
    valid_events: list[dict[str, Any]] = []
    for index, event in enumerate(rows, 1):
        try:
            value = event.get("pin")
            pin = BeingImportancePinV1.from_untrusted(value)
            if event.get("actor") != pin.being: raise RecordValidationError("pin actor mismatch")
            if pin.action == "pin": active[pin.being].add(pin.contract_id)
            else: active[pin.being].discard(pin.contract_id)
            valid_events.append(event)
        except (RecordValidationError, ValueError, TypeError, AttributeError) as error:
            errors.append(f"pin_{index}:{error}")
    return active, valid_events, errors


def _contracts(workspace: Path) -> tuple[list[dict[str, Any]], bytes]:
    path = workspace / "diagnostics/felt_contract_graph_v1/contracts.jsonl"
    raw = path.read_bytes()
    rows = [json.loads(line) for line in raw.decode().splitlines() if line.strip()]
    return rows, raw


def _unix_ms(value: Any) -> int:
    try: return int(datetime.fromisoformat(str(value)).timestamp() * 1000)
    except (TypeError, ValueError): return 0


def _queue_position(contract: dict[str, Any]) -> int:
    match = TIMESTAMP_RE.search(str(contract.get("anchor_claim_id") or ""))
    return int(match.group(1)) if match else 2**63 - 1


def _review_priority(contract: dict[str, Any]) -> tuple[int, str]:
    review = str(contract.get("felt_review") or "")
    activity = str(contract.get("activity") or "")
    if review == "contradicted" or int(contract.get("contradiction_count") or 0) > 0:
        return 6, "contradiction_or_named_objection"
    if (
        review in {"still_friction", "objection"}
        or activity == "reopened"
        or int(contract.get("reopen_count") or 0) > 0
    ):
        return 5, "reopened_or_still_friction"
    if activity == "review_pending" or review == "awaiting":
        return 3, "review_pending"
    if activity == "open":
        return 1, "open"
    return 0, "quiet"


def select_portfolio(workspace: Path) -> tuple[AttentionPortfolioV2, list[dict[str, Any]], list[dict[str, Any]], list[str]]:
    contracts, raw = _contracts(workspace)
    pins, pin_events, errors = replay_pins(workspace)
    latest_ms = max((_unix_ms(item.get("last_change_at")) for item in contracts), default=0)
    eligible = []
    for contract in contracts:
        contract_id = str(contract.get("contract_id") or "")
        pinned_by = tuple(being for being in ("astrid", "minime") if contract_id in pins[being])
        if contract.get("felt_closed") and str(contract.get("activity")) != "reopened": continue
        if str(contract.get("activity")) == "quiet_archived" and not pinned_by: continue
        changed = _unix_ms(contract.get("last_change_at"))
        review_priority, review_state_class = _review_priority(contract)
        eligible.append({"contract": contract, "contract_id": contract_id,
                         "review_priority": review_priority,
                         "review_state_class": review_state_class,
                         "claim_recurrence_count": int(contract.get("claim_count") or 0),
                         "source_signal_recent": bool(latest_ms and latest_ms - changed <= 24 * 60 * 60 * 1000),
                         "steward_unaddressed_age_ms": max(0, latest_ms - changed),
                         "queue": _queue_position(contract), "pinned_by": pinned_by})
    def rank(item: dict[str, Any]) -> tuple[Any, ...]:
        return (-item["review_priority"], -int(bool(item["pinned_by"])),
                -item["claim_recurrence_count"], -int(item["source_signal_recent"]),
                -item["steward_unaddressed_age_ms"], item["queue"], item["contract_id"])
    ranked = sorted(eligible, key=rank)
    urgent = [item for item in ranked if item["review_priority"] >= 5]
    selected: list[tuple[dict[str, Any], str]] = [(item, "urgent") for item in urgent[:4]]
    selected_ids = {item["contract_id"] for item, _ in selected}
    overflow = [item["contract_id"] for item in urgent[4:]]
    overflow_ids = set(overflow)
    for being, slot_class in (("astrid", "astrid_pin"), ("minime", "minime_pin")):
        candidates = [
            item
            for item in ranked
            if item["contract_id"] in pins[being]
            and item["contract_id"] not in selected_ids
            and item["contract_id"] not in overflow_ids
        ]
        for item in candidates[:2]:
            selected.append((item, slot_class)); selected_ids.add(item["contract_id"])
    for item in ranked:
        if len(selected) >= 16: break
        if item["contract_id"] not in selected_ids and item["contract_id"] not in overflow_ids:
            selected.append((item, "ranked")); selected_ids.add(item["contract_id"])
    entries = [AttentionPortfolioEntryV2.build(
        contract_id=item["contract_id"], steward_slot_class=slot_class,
        selection_rank=index,
        contract_review_state_class=item["review_state_class"],
        claim_recurrence_count=item["claim_recurrence_count"],
        source_signal_recency_class=(
            "recent_24h" if item["source_signal_recent"] else "older"
        ),
        steward_unaddressed_age_band=steward_unaddressed_age_band(
            item["steward_unaddressed_age_ms"]
        ),
        canonical_queue_tiebreaker=item["queue"], pinned_by=item["pinned_by"])
        for index, (item, slot_class) in enumerate(selected, 1)]
    return AttentionPortfolioV2.build(
        source_contracts_sha256=sha256_bytes(raw),
        selected_entries=entries,
        visible_urgent_alert_contract_ids=overflow,
    ), pin_events, contracts, errors


def project(workspace: Path, *, write: bool) -> dict[str, Any]:
    portfolio, pin_events, contracts, errors = select_portfolio(workspace)
    selection = AttentionSelectionReceiptV2.from_portfolio(portfolio)
    records = [event.get("pin") for event in pin_events] + [portfolio.to_dict(), selection.to_dict()]
    payloads = []
    for record in records:
        if not isinstance(record, dict): continue
        identifier = str(
            record.get("pin_id")
            or record.get("receipt_id")
            or record.get("portfolio_id")
        )
        payloads.append(event_payload(
            schema=SCHEMA, event_type=f"{record['schema']}_recorded",
            aggregate_type="attention_portfolio", aggregate_id=portfolio.portfolio_id,
            idempotency_key=f"{STREAM}:{identifier}", record=record,
        ))
    appended = project_events(workspace, STREAM, payloads,
                              actor="attention-portfolio-projector",
                              source_kind="felt_contract_bounded_view",
                              source_locator_value="diagnostics/felt_contract_graph_v1/contracts.jsonl") if write and not errors else 0
    active = portfolio.to_dict()
    contracts_by_id = {
        str(contract.get("contract_id") or ""): contract for contract in contracts
    }
    status = {"schema": "attention_portfolio_status_v2", "schema_version": 2,
              "valid": not errors, "write": write, "contract_count": len(contracts),
              "steward_selected_count": len(portfolio.selected_entries),
              "steward_selected_work_limit": 16,
              "urgent_selected_count": sum(item.steward_slot_class == "urgent" for item in portfolio.selected_entries),
              "astrid_pin_selected_count": sum(item.steward_slot_class == "astrid_pin" for item in portfolio.selected_entries),
              "minime_pin_selected_count": sum(item.steward_slot_class == "minime_pin" for item in portfolio.selected_entries),
              "visible_urgent_alert_count": len(portfolio.visible_urgent_alert_contract_ids),
              "visible_urgent_alert": bool(portfolio.visible_urgent_alert_contract_ids),
              "appended_event_count": appended,
              "selection_scope": "steward_work_view_not_being_attention",
              "contract_state_relation": "selection_does_not_change_contract_or_felt_state",
              "runtime_relation": "not_consumed_by_bridge_minime_model_or_control_runtime",
              "authority_relation": "cannot_grant_or_propagate_authority",
              "errors": errors,
              "counter_audit": {"status": "consistent" if not errors else "inconsistent",
                                "checks": {"steward_work_limit_respected": len(portfolio.selected_entries) <= 16,
                                           "selected_ids_unique": len({item.contract_id for item in portfolio.selected_entries}) == len(portfolio.selected_entries),
                                           "urgent_selection_limit_respected": sum(item.steward_slot_class == "urgent" for item in portfolio.selected_entries) <= 4,
                                           "closed_not_selected": all(
                                               not contracts_by_id[item.contract_id].get("felt_closed")
                                               or contracts_by_id[item.contract_id].get("activity") == "reopened"
                                               for item in portfolio.selected_entries
                                           ),
                                           "visible_urgent_alert_not_selected": not set(portfolio.visible_urgent_alert_contract_ids).intersection(
                                               item.contract_id for item in portfolio.selected_entries
                                           )}},
              "artifact_authority_state_v1": authority_state()}
    if write and status["valid"]:
        output = state_dir(workspace)
        owner_atomic_write_json(output / "active.json", active)
        owner_atomic_write_json(output / "status.json", status)
        owner_atomic_write(
            output / "report.md",
            "# Contract-Centered Steward Work View\n\n"
            "This selects at most 16 contracts for steward work. It is not a model of "
            "Astrid's or Minime's attention, felt state, capacity, pressure, permissions, "
            "or runtime behavior. Every unselected contract remains queryable; urgent "
            "unselected contracts remain visible in the alert list.\n\n"
            + "\n".join(
                f"- {item.selection_rank}. {item.contract_id} ({item.steward_slot_class})"
                for item in portfolio.selected_entries
            )
            + "\n",
        )
    return status


def query(workspace: Path, contract_id: str | None) -> dict[str, Any]:
    path = state_dir(workspace) / "active.json"
    if not path.is_file():
        path = legacy_state_dir(workspace) / "active.json"
    value = json.loads(path.read_text()) if path.is_file() else {}
    if value and value.get("schema") == "attention_portfolio_v2":
        value = AttentionPortfolioV2.from_untrusted(value).to_dict()
    elif value:
        value = AttentionPortfolioV2.from_legacy_v1(
            AttentionPortfolioV1.from_untrusted(value).to_dict()
        ).to_dict()
    if contract_id:
        value["selected_entries"] = [
            item
            for item in value.get("selected_entries", [])
            if item.get("contract_id") == contract_id
        ]
    return value
