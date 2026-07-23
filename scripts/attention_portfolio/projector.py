"""Read-only compatibility support for historical attention artifacts."""

from __future__ import annotations

import json
import re
from collections import defaultdict
from datetime import datetime
from pathlib import Path
from typing import Any

try:
    from experiential_systems.common import (
        RecordValidationError, authority_state, load_jsonl, sha256_bytes,
        stream_payloads,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError, authority_state, load_jsonl, sha256_bytes,
        stream_payloads,
    )

from .model import (
    AttentionPortfolioEntryV2,
    AttentionPortfolioV1,
    AttentionPortfolioV2,
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
    del workspace, pin, actor
    raise RecordValidationError(
        "attention portfolio mutation is retired; use steward_work_selection"
    )


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
    events_path = workspace / "diagnostics/evidence_event_store_v2/events.jsonl"
    payloads, corrupt = (
        stream_payloads(workspace, STREAM) if events_path.is_file() else ([], 0)
    )
    return {
        "schema": "attention_portfolio_compatibility_status_v1",
        "schema_version": 1,
        "valid": corrupt == 0,
        "retired": True,
        "requested_write": write,
        "appended_event_count": 0,
        "historical_event_count": len(payloads),
        "corrupt_event_lines": corrupt,
        "successor": "steward_work_selection",
        "historical_records_rewritten": False,
        "artifact_authority_state_v1": authority_state(),
    }


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
