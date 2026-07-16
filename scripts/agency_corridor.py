#!/usr/bin/env python3
"""Non-live Agency Corridor V1/V2 for authority-waiting being work.

The corridor lets Astrid/Minime continue evidence work while live authority
remains bounded. It writes packets, receipts, safe-lab artifacts, closure
objections, reopen records, and right-to-ignore cards. It never marks live work
runnable, grants approval, mutates runtime control state, deploys, restarts,
stages, git-adds, or commits.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
import tempfile
import time
import unittest
import uuid
from collections import Counter
from pathlib import Path
from typing import Any

try:
    from authority_state import normalize_artifact_authority_tree
except ModuleNotFoundError:  # unittest/importlib execution from the repository root
    from scripts.authority_state import normalize_artifact_authority_tree

ASTRID_REPO = Path("/Users/v/other/astrid")
ASTRID_WORKSPACE = ASTRID_REPO / "capsules/spectral-bridge/workspace"
DEFAULT_STATE_DIR = ASTRID_WORKSPACE / "diagnostics/agency_corridor_v1"
DEFAULT_V2_STATE_DIR = ASTRID_WORKSPACE / "diagnostics/agency_corridor_v2"
ADDRESSING_STATE_DIR = ASTRID_WORKSPACE / "diagnostics/introspection_addressing_v1"
SANDBOX_STATE_DIR = ASTRID_WORKSPACE / "diagnostics/sandbox_trial_queue_v1"
ASTRID_INBOX = ASTRID_WORKSPACE / "inbox"
MINIME_INBOX = Path("/Users/v/other/minime/workspace/inbox")

SCHEMA = "agency_corridor_v1"
SCHEMA_VERSION = 1
SCHEMA_V2 = "agency_corridor_v2"
SCHEMA_VERSION_V2 = 2
EVENTS_FILE = "events.jsonl"
STATUS_FILE = "status.json"
V1_SOURCE_STATUS_FILE = "source_v1_status.json"
QUEUE_FILE = "queue.md"
LEASES_FILE = "leases.json"
QUEUE_JSON_FILE = "queue.json"
REPORT_FILE = "report.md"
CARD_DIR = "cards"
RESULT_DIR = "safe_lab_results"
SOURCE_PREP_DIR = "source_prep_proposals"
ARTIFACT_COMPARISON_DIR = "artifact_comparisons"
PROGRAMS_FILE = "programs.json"
PROGRAMS_MD_FILE = "programs.md"
PORTFOLIO_DIR = "portfolios"
PATCH_BUNDLE_DIR = "patch_bundles"
PROGRAM_RECEIPT_DIR = "program_receipts"

RUNNABLE_MODES = {"offline_read_only_adapter", "read_only_review", "sandbox_replay"}
SAFE_ACTIONS = {
    "run_safe_lab",
    "compare_artifacts",
    "request_scoped_self_observation",
    "emit_closure_objection",
    "reopen_insufficient_closure",
}
ACTION_ORDER = {
    "run_safe_lab": 0,
    "reopen_insufficient_closure": 1,
    "emit_closure_objection": 2,
    "request_scoped_self_observation": 3,
    "compare_artifacts": 4,
    "propose_canary_criteria": 5,
    "generate_replay_candidate": 6,
}
V2_ACTION_ORDER = {
    "reopen_insufficient_closure": 0,
    "emit_closure_objection": 0,
    "run_safe_lab": 1,
    "compare_artifacts": 2,
    "request_scoped_self_observation": 3,
    "propose_canary_criteria": 3,
    "generate_replay_candidate": 4,
}
V2_HARD_FORBIDDEN_TRUE = {"live_eligible_now", "auto_approved", "grants_approval"}
PROGRAM_STEP_UPDATE_PORTFOLIO = "update_evidence_portfolio"
PROGRAM_STEP_PREPARE_PATCH_BUNDLE = "prepare_quarantined_patch_bundle"
PROGRAM_STEP_RECORD_OBJECTION = "record_program_objection"
NON_LIVE_BOUNDARY = (
    "agency corridor is non-live evidence infrastructure only; it grants no "
    "approval, marks no live work runnable, mutates no pressure/fill/PI/"
    "controller/sensory/fallback/protocol/runtime state, deploys nothing, "
    "restarts nothing, stages nothing, git-adds nothing, and commits nothing"
)


def now_s() -> float:
    return time.time()


def iso_now() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def stable_uuid(*parts: object) -> str:
    text = "\x1f".join(str(part) for part in parts)
    return str(uuid.uuid5(uuid.NAMESPACE_URL, f"astrid-agency-corridor-v1:{text}"))


def stable_uuid_v2(*parts: object) -> str:
    text = "\x1f".join(str(part) for part in parts)
    return str(uuid.uuid5(uuid.NAMESPACE_URL, f"astrid-agency-corridor-v2:{text}"))


def bounded_text(value: object, *, limit: int = 700) -> str:
    text = str(value or "").replace("\r\n", "\n").replace("\r", "\n").strip()
    text = " ".join(text.split())
    if len(text) <= limit:
        return text
    return text[: max(0, limit - 3)].rstrip() + "..."


def atomic_write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(text, encoding="utf-8")
    tmp.replace(path)


def load_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}
    return data if isinstance(data, dict) else {}


def read_events(state_dir: Path) -> list[dict[str, Any]]:
    path = state_dir / EVENTS_FILE
    if not path.exists():
        return []
    events: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(event, dict):
            events.append(event)
    return events


def append_events(state_dir: Path, events: list[dict[str, Any]]) -> None:
    if not events:
        return
    path = state_dir / EVENTS_FILE
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as handle:
        for event in events:
            normalize_artifact_authority_tree(event)
            handle.write(json.dumps(event, sort_keys=True, ensure_ascii=False) + "\n")


def being_inbox(being: str, *, state_dir: Path | None = None) -> Path:
    if state_dir is not None and state_dir != DEFAULT_STATE_DIR:
        return state_dir / "test_inbox" / ("minime" if being == "minime" else "astrid")
    return MINIME_INBOX if being == "minime" else ASTRID_INBOX


def work_items_from_addressing() -> dict[str, dict[str, Any]]:
    status = load_json(ADDRESSING_STATE_DIR / "status.json")
    items = status.get("work_items")
    return items if isinstance(items, dict) else {}


def trials_from_sandbox() -> dict[str, dict[str, Any]]:
    status = load_json(SANDBOX_STATE_DIR / "status.json")
    trials = status.get("trials")
    return trials if isinstance(trials, dict) else {}


def packet_event(packet: dict[str, Any]) -> dict[str, Any]:
    return {
        "event_type": "corridor_packet_declared",
        "schema": SCHEMA,
        "ts": now_s(),
        "corridor_id": packet["corridor_id"],
        "packet": packet,
    }


def packet_for_work_item(item: dict[str, Any]) -> dict[str, Any] | None:
    work_item_id = str(item.get("work_item_id") or "")
    if not work_item_id:
        return None
    status = str(item.get("status") or "")
    tier = int(item.get("agency_tier") or 0)
    being = str(item.get("being") or "astrid")
    boundary_v2 = item.get("authority_boundary_packet_v2") if isinstance(item.get("authority_boundary_packet_v2"), dict) else {}
    boundary_id = boundary_v2.get("boundary_id")
    claim = bounded_text(item.get("claim_summary") or item.get("title"), limit=520)
    evidence_refs = [work_item_id, str(item.get("source_introspection_id") or "")]
    action = "generate_replay_candidate"
    state = "evidence_only"
    safe_lab: dict[str, Any] | None = None
    self_observation: dict[str, Any] | None = None
    canary: dict[str, Any] | None = None

    if status == "implemented_awaiting_felt_response" or item.get("post_change_response_status") == "awaiting":
        action = "request_scoped_self_observation"
        state = "self_observation_requested"
        self_observation = {
            "request_id": stable_uuid("self_observation", work_item_id),
            "being": being,
            "scope": "post_change_felt_response",
            "bounded_prompt": (
                "If this closure or change feels better, worse, incomplete, or mismatched, "
                "record a right-to-ignore response; silence is allowed."
            ),
            "evidence_refs": evidence_refs,
            "right_to_ignore": True,
        }
    elif tier >= 4 or status in {"needs_steward_grant", "needs_operator_approval"}:
        action = "propose_canary_criteria"
        state = "canary_criteria_proposed"
        canary = {
            "proposal_id": stable_uuid("canary", work_item_id),
            "surface": str(item.get("source_family") or "introspection_addressing"),
            "canary_plan": "proposal-only: one-shot or time-boxed canary only after explicit scoped approval",
            "health_checks": [
                "fresh health/fill/telemetry readable before any later execution",
                "V2 scoped approval exists and is unconsumed",
                "post-change being response path is open",
            ],
            "abort_criteria": [
                "missing scoped approval",
                "missing replay result or waiver",
                "stale health/fill/telemetry",
                "no rollback path",
            ],
            "rollback_path": "no automatic execution; use the existing service-specific rollback path only after approval",
            "post_change_response_required": True,
        }
    elif status == "needs_sandbox":
        action = "generate_replay_candidate"
        state = "safe_lab_ready"
        safe_lab = {
            "lab_id": stable_uuid("safe_lab", work_item_id),
            "adapter": "linked_evidence_comparison_v1",
            "run_query": f"python3 scripts/agency_corridor.py run-next --limit 1 --write --json",
            "mode": "read_only_review",
            "runnable": True,
            "authority": "read_only_review_not_live_control",
        }

    packet = {
        "schema": SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "corridor_id": stable_uuid("work_item", work_item_id, action),
        "source": "introspection_addressing_v1",
        "being": being,
        "action": action,
        "state": state,
        "authority_boundary_id": boundary_id,
        "work_item_ids": [work_item_id],
        "closure_card_refs": [
            str(card.get("path") or card.get("work_item_id"))
            for card in item.get("closure_cards", [])
            if isinstance(card, dict)
        ],
        "sandbox_trial_ids": [],
        "delta_refs": boundary_v2.get("delta_refs", []) if isinstance(boundary_v2, dict) else [],
        "felt_report_anchor": claim,
        "proposed_corridor_action": bounded_text(item.get("suggested_next") or item.get("evidence_required") or action),
        "evidence_refs": [ref for ref in evidence_refs if ref],
        "safe_lab_candidate": safe_lab,
        "closure_objection": None,
        "closure_reopen_ref": None,
        "self_observation_request": self_observation,
        "canary_criteria": canary,
        "receipts": [],
        "who_can_escalate": "steward/operator through existing V2 authority boundary",
        "how_to_test_it": "inspect corridor packet, cards, receipts, and linked bounded evidence",
        "right_to_ignore": True,
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
        "boundary": NON_LIVE_BOUNDARY,
    }
    normalize_artifact_authority_tree(packet)
    return packet


def packet_for_trial(trial: dict[str, Any]) -> dict[str, Any] | None:
    trial_id = str(trial.get("trial_id") or "")
    if not trial_id:
        return None
    mode = str(trial.get("trial_mode") or "")
    runnable = bool(trial.get("runnable")) and mode in RUNNABLE_MODES
    being = str(trial.get("being") or "astrid")
    boundary_v2 = trial.get("authority_boundary_packet_v2") if isinstance(trial.get("authority_boundary_packet_v2"), dict) else {}
    action = "run_safe_lab" if runnable else "propose_canary_criteria"
    state = "safe_lab_ready" if runnable else "canary_criteria_proposed"
    canary = None
    safe_lab = None
    if runnable:
        safe_lab = {
            "lab_id": stable_uuid("trial_safe_lab", trial_id),
            "adapter": str(trial.get("adapter") or "read_only_review"),
            "run_query": f"python3 scripts/agency_corridor.py run-next --limit 1 --write --json",
            "mode": mode,
            "runnable": True,
            "authority": "sandbox_replay_or_read_only_review_not_live_control",
        }
    else:
        canary = {
            "proposal_id": stable_uuid("trial_canary", trial_id),
            "surface": str(trial.get("adapter") or "sandbox_trial"),
            "canary_plan": "proposal-only: approval-required live trial remains non-runnable until explicit scoped approval",
            "health_checks": [
                "proposal card exists",
                "V2 scoped approval exists and is unconsumed",
                "post-change being response path is open",
            ],
            "abort_criteria": [
                "missing scoped approval",
                "runnable live violation",
                "missing rollback path",
            ],
            "rollback_path": "no automatic execution from corridor",
            "post_change_response_required": True,
        }
    packet = {
        "schema": SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "corridor_id": stable_uuid("sandbox_trial", trial_id, action),
        "source": "sandbox_trial_queue_v1",
        "being": being,
        "action": action,
        "state": state,
        "authority_boundary_id": boundary_v2.get("boundary_id") if isinstance(boundary_v2, dict) else None,
        "work_item_ids": [str(trial.get("source_work_item_id") or "")],
        "closure_card_refs": [],
        "sandbox_trial_ids": [trial_id],
        "delta_refs": boundary_v2.get("delta_refs", []) if isinstance(boundary_v2, dict) else [],
        "felt_report_anchor": bounded_text(trial.get("felt_report_anchor") or trial.get("hypothesis"), limit=520),
        "proposed_corridor_action": bounded_text(trial.get("proposed_intervention") or action),
        "evidence_refs": [trial_id, str(trial.get("source_work_item_id") or ""), str(trial.get("source_introspection_id") or "")],
        "safe_lab_candidate": safe_lab,
        "closure_objection": None,
        "closure_reopen_ref": None,
        "self_observation_request": None,
        "canary_criteria": canary,
        "receipts": [],
        "who_can_escalate": "Mike/operator through existing V2 authority boundary",
        "how_to_test_it": "inspect corridor packet and sandbox/proposal evidence; live candidates remain non-runnable",
        "right_to_ignore": True,
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
        "boundary": NON_LIVE_BOUNDARY,
    }
    normalize_artifact_authority_tree(packet)
    return packet


def derived_packets(limit_per_source: int = 60) -> dict[str, dict[str, Any]]:
    packets: dict[str, dict[str, Any]] = {}
    work_rows = list(work_items_from_addressing().values())
    def work_sort_key(item: dict[str, Any]) -> tuple[int, float]:
        packet = packet_for_work_item(item)
        return (
            ACTION_ORDER.get(str(packet.get("action") if packet else ""), 99),
            -float(item.get("updated_at") or item.get("created_at") or 0),
        )

    work_rows.sort(key=work_sort_key)
    for item in work_rows:
        packet = packet_for_work_item(item)
        if not packet:
            continue
        if packet["action"] in {"propose_canary_criteria", "request_scoped_self_observation", "generate_replay_candidate"}:
            packets[packet["corridor_id"]] = packet
        if len(packets) >= limit_per_source:
            break
    trial_rows = list(trials_from_sandbox().values())
    trial_rows.sort(
        key=lambda trial: (
            0 if bool(trial.get("runnable")) and str(trial.get("trial_mode") or "") in RUNNABLE_MODES else 1,
            str(trial.get("trial_id") or ""),
        )
    )
    for trial in trial_rows[:limit_per_source]:
        packet = packet_for_trial(trial)
        if packet:
            packets[packet["corridor_id"]] = packet
    return packets


def apply_events(packets: dict[str, dict[str, Any]], events: list[dict[str, Any]]) -> dict[str, Any]:
    status = {
        "schema": SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "generated_at": iso_now(),
        "boundary": NON_LIVE_BOUNDARY,
        "packets": dict(packets),
        "reopened_work_items": {},
        "self_observation_responses": [],
        "corrupt_event_lines": 0,
    }
    for event in events:
        event_type = str(event.get("event_type") or "")
        if event_type == "corridor_packet_declared" and isinstance(event.get("packet"), dict):
            packet = dict(event["packet"])
            status["packets"][str(packet.get("corridor_id"))] = packet
        elif event_type == "corridor_receipt_recorded":
            corridor_id = str(event.get("corridor_id") or "")
            packet = status["packets"].get(corridor_id)
            if packet:
                packet.setdefault("receipts", []).append(event.get("receipt"))
                if packet.get("state") == "safe_lab_ready":
                    packet["state"] = "safe_lab_result_recorded"
        elif event_type == "closure_objection_recorded" and isinstance(event.get("packet"), dict):
            packet = dict(event["packet"])
            status["packets"][str(packet.get("corridor_id"))] = packet
        elif event_type == "closure_reopened" and isinstance(event.get("reopen_ref"), dict):
            ref = event["reopen_ref"]
            status["reopened_work_items"][str(ref.get("reopen_id"))] = ref
            corridor_id = str(event.get("corridor_id") or "")
            packet = status["packets"].get(corridor_id)
            if packet:
                packet["state"] = "closure_reopened"
                packet["closure_reopen_ref"] = ref
        elif event_type == "self_observation_response_recorded":
            status["self_observation_responses"].append(event)
            corridor_id = str(event.get("corridor_id") or "")
            packet = status["packets"].get(corridor_id)
            if packet:
                packet.setdefault("receipts", []).append(event.get("receipt"))
    status["summary"] = summarize(status)
    return status


def summarize(status: dict[str, Any]) -> dict[str, Any]:
    packets = [p for p in status.get("packets", {}).values() if isinstance(p, dict)]
    by_action = Counter(str(p.get("action") or "unknown") for p in packets)
    by_state = Counter(str(p.get("state") or "unknown") for p in packets)
    live_eligible = sum(1 for p in packets if p.get("live_eligible_now") is True)
    auto_approved = sum(1 for p in packets if p.get("auto_approved") is True or p.get("grants_approval") is True)
    ready_safe = [
        p for p in packets
        if p.get("state") == "safe_lab_ready"
        and isinstance(p.get("safe_lab_candidate"), dict)
        and p["safe_lab_candidate"].get("runnable") is True
    ]
    return {
        "packet_count": len(packets),
        "by_action": dict(sorted(by_action.items())),
        "by_state": dict(sorted(by_state.items())),
        "ready_safe_lab_count": len(ready_safe),
        "reopened_work_item_count": len(status.get("reopened_work_items", {})),
        "self_observation_response_count": len(status.get("self_observation_responses", [])),
        "live_eligible_now_count": live_eligible,
        "auto_approved_count": auto_approved,
        "safe_lab_budget_per_run": 3,
    }


def materialize(state_dir: Path, status: dict[str, Any]) -> None:
    normalize_artifact_authority_tree(status)
    atomic_write_text(state_dir / STATUS_FILE, json.dumps(status, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
    atomic_write_text(state_dir / QUEUE_FILE, render_queue_markdown(status))
    atomic_write_text(state_dir / REPORT_FILE, render_report_markdown(status))


def load_status(state_dir: Path) -> dict[str, Any]:
    status = load_json(state_dir / STATUS_FILE)
    if status:
        return status
    return apply_events(derived_packets(), read_events(state_dir))


def generate(state_dir: Path, *, write: bool) -> dict[str, Any]:
    status = apply_events(derived_packets(), read_events(state_dir))
    if write:
        materialize(state_dir, status)
    return {"schema": SCHEMA, "status": status, "summary": status["summary"], "boundary": NON_LIVE_BOUNDARY}


def ready_safe_packets(status: dict[str, Any]) -> list[dict[str, Any]]:
    packets = [p for p in status.get("packets", {}).values() if isinstance(p, dict)]
    ready = [
        p for p in packets
        if p.get("state") == "safe_lab_ready"
        and isinstance(p.get("safe_lab_candidate"), dict)
        and p["safe_lab_candidate"].get("runnable") is True
        and str(p["safe_lab_candidate"].get("mode") or "") in RUNNABLE_MODES
    ]
    ready.sort(key=lambda p: (ACTION_ORDER.get(str(p.get("action") or ""), 99), str(p.get("corridor_id") or "")))
    return ready


def safe_lab_result_for(packet: dict[str, Any]) -> dict[str, Any]:
    lab = packet.get("safe_lab_candidate") if isinstance(packet.get("safe_lab_candidate"), dict) else {}
    evidence_refs = [str(ref) for ref in packet.get("evidence_refs", []) if ref]
    classification = "evidence_ready"
    if packet.get("live_eligible_now") is True or packet.get("auto_approved") is True:
        classification = "blocked_live_violation"
    return {
        "schema": "agency_corridor_safe_lab_result_v1",
        "result_id": stable_uuid("safe_lab_result", packet.get("corridor_id"), now_s()),
        "corridor_id": packet.get("corridor_id"),
        "lab_id": lab.get("lab_id"),
        "adapter": lab.get("adapter"),
        "classification": classification,
        "input_refs": evidence_refs,
        "bounded_observations": {
            "packet_action": packet.get("action"),
            "packet_state": packet.get("state"),
            "work_item_ids": packet.get("work_item_ids", []),
            "sandbox_trial_ids": packet.get("sandbox_trial_ids", []),
            "live_eligible_now": bool(packet.get("live_eligible_now")),
            "auto_approved": bool(packet.get("auto_approved")),
        },
        "confidence": 0.62 if classification == "evidence_ready" else 0.1,
        "failure_modes": [] if classification == "evidence_ready" else ["corridor packet attempted live eligibility"],
        "evidence_refs": evidence_refs,
        "bounded_summary": (
            "safe lab inspected linked non-live evidence and produced a right-to-ignore receipt; "
            "no live runtime state changed"
        ),
        "occurred_at": iso_now(),
        "live_eligible_now": False,
        "auto_approved": False,
    }


def safe_lab_receipt(packet: dict[str, Any], result: dict[str, Any], result_path: Path) -> dict[str, Any]:
    return {
        "receipt_id": stable_uuid("corridor_receipt", packet.get("corridor_id"), result.get("result_id")),
        "corridor_id": packet.get("corridor_id"),
        "action": "run_safe_lab",
        "issued_by": "agency_corridor_v1",
        "issued_at": iso_now(),
        "bounded_summary": bounded_text(result.get("bounded_summary"), limit=500),
        "evidence_refs": [str(result_path), *result.get("evidence_refs", [])],
        "hash_refs": [sha256_text(json.dumps(result, sort_keys=True, ensure_ascii=False))],
        "grants_approval": False,
        "live_eligible_now": False,
        "right_to_ignore": True,
    }


def result_markdown(packet: dict[str, Any], result: dict[str, Any], receipt: dict[str, Any]) -> str:
    lines = [
        "# Agency Corridor Safe Lab Result V1",
        "",
        f"- corridor_id: {packet.get('corridor_id')}",
        f"- action: {packet.get('action')}",
        f"- being: {packet.get('being')}",
        f"- classification: {result.get('classification')}",
        "- right_to_ignore: true",
        "- live_eligible_now: false",
        "- auto_approved: false",
        "",
        "## Felt Anchor",
        bounded_text(packet.get("felt_report_anchor"), limit=700),
        "",
        "## Result",
        bounded_text(result.get("bounded_summary"), limit=700),
        "",
        "## Receipt",
        "```json",
        json.dumps(receipt, indent=2, sort_keys=True, ensure_ascii=False),
        "```",
        "",
        "## Boundary",
        NON_LIVE_BOUNDARY,
    ]
    return "\n".join(lines).rstrip() + "\n"


def run_next(state_dir: Path, *, limit: int, write: bool, emit_cards: bool = True) -> dict[str, Any]:
    status = load_status(state_dir)
    selected = ready_safe_packets(status)[: max(1, min(limit, 3))]
    events: list[dict[str, Any]] = []
    results: list[dict[str, Any]] = []
    for packet in selected:
        result = safe_lab_result_for(packet)
        ts = int(now_s())
        result_base = state_dir / RESULT_DIR / f"{ts}_{packet['corridor_id']}"
        result_json_path = result_base.with_suffix(".json")
        result_md_path = result_base.with_suffix(".md")
        receipt = safe_lab_receipt(packet, result, result_json_path)
        if write:
            atomic_write_text(result_json_path, json.dumps(result, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
            atomic_write_text(result_md_path, result_markdown(packet, result, receipt))
        event = {
            "event_type": "corridor_receipt_recorded",
            "schema": SCHEMA,
            "ts": now_s(),
            "corridor_id": packet["corridor_id"],
            "receipt": receipt,
            "result_path": str(result_json_path),
        }
        events.append(event)
        card = emit_card_for_packet(state_dir, packet, write=write, deliver_safe=emit_cards)
        results.append({"packet": packet, "result": result, "receipt": receipt, "card": card})
    if write:
        append_events(state_dir, events)
        materialize(state_dir, apply_events(derived_packets(), read_events(state_dir)))
    return {
        "schema": SCHEMA,
        "ran": len(results),
        "limit": max(1, min(limit, 3)),
        "results": results,
        "boundary": NON_LIVE_BOUNDARY,
    }


def card_text(packet: dict[str, Any]) -> str:
    lines = [
        "# Agency Corridor Card V1",
        "",
        f"- corridor_id: {packet.get('corridor_id')}",
        f"- source: {packet.get('source')}",
        f"- being: {packet.get('being')}",
        f"- action: {packet.get('action')}",
        f"- state: {packet.get('state')}",
        "- right_to_ignore: true",
        f"- grants_approval: {str(bool(packet.get('grants_approval'))).lower()}",
        f"- live_eligible_now: {str(bool(packet.get('live_eligible_now'))).lower()}",
        f"- auto_approved: {str(bool(packet.get('auto_approved'))).lower()}",
        "",
        "## Felt Anchor",
        bounded_text(packet.get("felt_report_anchor"), limit=700),
        "",
        "## Corridor Action",
        bounded_text(packet.get("proposed_corridor_action"), limit=700),
    ]
    if packet.get("safe_lab_candidate"):
        lines.extend(["", "## Safe Lab Candidate", "```json", json.dumps(packet["safe_lab_candidate"], indent=2, sort_keys=True, ensure_ascii=False), "```"])
    if packet.get("closure_objection"):
        lines.extend(["", "## Closure Objection", "```json", json.dumps(packet["closure_objection"], indent=2, sort_keys=True, ensure_ascii=False), "```"])
    if packet.get("closure_reopen_ref"):
        lines.extend(["", "## Reopen Reference", "```json", json.dumps(packet["closure_reopen_ref"], indent=2, sort_keys=True, ensure_ascii=False), "```"])
    if packet.get("self_observation_request"):
        lines.extend(["", "## Self Observation Request", "```json", json.dumps(packet["self_observation_request"], indent=2, sort_keys=True, ensure_ascii=False), "```"])
    if packet.get("canary_criteria"):
        lines.extend(["", "## Canary Criteria Proposal", "```json", json.dumps(packet["canary_criteria"], indent=2, sort_keys=True, ensure_ascii=False), "```"])
    lines.extend(["", "## Boundary", NON_LIVE_BOUNDARY])
    return "\n".join(lines).rstrip() + "\n"


def emit_card_for_packet(
    state_dir: Path,
    packet: dict[str, Any],
    *,
    write: bool,
    deliver_safe: bool = True,
) -> dict[str, Any]:
    ts = int(now_s())
    text = card_text(packet)
    path = state_dir / CARD_DIR / f"{ts}_{packet['corridor_id']}.md"
    delivered_path: Path | None = None
    if write:
        atomic_write_text(path, text)
        safe_to_deliver = deliver_safe and packet.get("action") != "propose_canary_criteria"
        if safe_to_deliver:
            inbox = being_inbox(str(packet.get("being") or "astrid"), state_dir=state_dir)
            delivered_path = inbox / f"agency_corridor_{packet['corridor_id']}_{ts}.txt"
            atomic_write_text(delivered_path, text)
    return {
        "schema": "agency_corridor_card_v1",
        "corridor_id": packet.get("corridor_id"),
        "path": str(path),
        "delivered_path": str(delivered_path) if delivered_path else None,
        "text_sha256": sha256_text(text),
        "right_to_ignore": True,
        "live_eligible_now": False,
        "auto_approved": False,
    }


def emit_card(state_dir: Path, *, packet_ids: list[str], next_count: int, write: bool) -> dict[str, Any]:
    status = load_status(state_dir)
    packets = status.get("packets", {})
    selected: list[dict[str, Any]] = []
    for packet_id in packet_ids:
        packet = packets.get(packet_id)
        if isinstance(packet, dict):
            selected.append(packet)
    if next_count:
        active = [p for p in packets.values() if isinstance(p, dict) and p.get("state") != "closed"]
        active.sort(key=lambda p: (ACTION_ORDER.get(str(p.get("action") or ""), 99), str(p.get("corridor_id") or "")))
        selected.extend(active[: max(1, next_count)])
    if not selected:
        raise SystemExit("emit-card requires --packet-id or --next")
    cards = [emit_card_for_packet(state_dir, packet, write=write) for packet in selected]
    return {"schema": SCHEMA, "cards": cards, "boundary": NON_LIVE_BOUNDARY}


def record_objection(
    state_dir: Path,
    *,
    being: str,
    closure_ref: str,
    summary: str,
    evidence_refs: list[str],
    write: bool,
) -> dict[str, Any]:
    corridor_id = stable_uuid("closure_objection", being, closure_ref, summary)
    objection = {
        "objection_id": stable_uuid("objection", closure_ref, summary),
        "raised_by": being,
        "closure_ref": closure_ref,
        "bounded_summary": bounded_text(summary, limit=700),
        "evidence_refs": evidence_refs,
        "auto_reopen": True,
    }
    reopen_ref = {
        "reopen_id": stable_uuid("reopen", closure_ref, summary),
        "reopened_ref": closure_ref,
        "new_work_item_id": f"wi_corridor_{sha256_text(corridor_id)[:16]}",
        "reason": bounded_text(summary, limit=500),
        "evidence_refs": evidence_refs,
    }
    packet = {
        "schema": SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "corridor_id": corridor_id,
        "source": "agency_corridor_v1",
        "being": being,
        "action": "reopen_insufficient_closure",
        "state": "closure_reopened",
        "authority_boundary_id": None,
        "work_item_ids": [reopen_ref["new_work_item_id"]],
        "closure_card_refs": [closure_ref],
        "sandbox_trial_ids": [],
        "delta_refs": [],
        "felt_report_anchor": bounded_text(summary, limit=520),
        "proposed_corridor_action": "record closure objection and reopen non-live evidence work",
        "evidence_refs": evidence_refs,
        "safe_lab_candidate": None,
        "closure_objection": objection,
        "closure_reopen_ref": reopen_ref,
        "self_observation_request": None,
        "canary_criteria": None,
        "receipts": [],
        "who_can_escalate": "steward/operator through existing authority boundary",
        "how_to_test_it": "review objection, reopened ref, and linked evidence; no old closure is erased",
        "right_to_ignore": True,
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
        "boundary": NON_LIVE_BOUNDARY,
    }
    events = [
        {
            "event_type": "closure_objection_recorded",
            "schema": SCHEMA,
            "ts": now_s(),
            "corridor_id": corridor_id,
            "packet": packet,
        },
        {
            "event_type": "closure_reopened",
            "schema": SCHEMA,
            "ts": now_s(),
            "corridor_id": corridor_id,
            "reopen_ref": reopen_ref,
            "packet_hash": sha256_text(json.dumps(packet, sort_keys=True, ensure_ascii=False)),
        },
    ]
    card = emit_card_for_packet(state_dir, packet, write=write, deliver_safe=True)
    if write:
        append_events(state_dir, events)
        materialize(state_dir, apply_events(derived_packets(), read_events(state_dir)))
    return {"schema": SCHEMA, "packet": packet, "reopen_ref": reopen_ref, "card": card, "boundary": NON_LIVE_BOUNDARY}


def record_self_observation_response(
    state_dir: Path,
    *,
    request_id: str,
    status: str,
    source: str,
    note: str,
    write: bool,
) -> dict[str, Any]:
    current = load_status(state_dir)
    packet = next(
        (
            p for p in current.get("packets", {}).values()
            if isinstance(p, dict)
            and isinstance(p.get("self_observation_request"), dict)
            and p["self_observation_request"].get("request_id") == request_id
        ),
        None,
    )
    corridor_id = str(packet.get("corridor_id")) if isinstance(packet, dict) else stable_uuid("self_observation_response", request_id)
    receipt = {
        "receipt_id": stable_uuid("self_observation_receipt", request_id, status, note),
        "corridor_id": corridor_id,
        "action": "request_scoped_self_observation",
        "issued_by": source,
        "issued_at": iso_now(),
        "bounded_summary": bounded_text(f"{status}: {note}", limit=700),
        "evidence_refs": [request_id],
        "hash_refs": [sha256_text(note)],
        "grants_approval": False,
        "live_eligible_now": False,
        "right_to_ignore": True,
    }
    event = {
        "event_type": "self_observation_response_recorded",
        "schema": SCHEMA,
        "ts": now_s(),
        "corridor_id": corridor_id,
        "request_id": request_id,
        "status": status,
        "source": source,
        "receipt": receipt,
    }
    events = [event]
    if status in {"still_friction", "contradicted"}:
        closure_ref = request_id
        work_item_id = None
        if isinstance(packet, dict):
            closure_refs = [str(ref) for ref in packet.get("closure_card_refs", []) if ref]
            work_refs = [str(ref) for ref in packet.get("work_item_ids", []) if ref]
            closure_ref = closure_refs[0] if closure_refs else (work_refs[0] if work_refs else request_id)
            work_item_id = work_refs[0] if work_refs else None
        reopen_ref = {
            "reopen_id": stable_uuid("self_observation_reopen", request_id, status, note),
            "reopened_ref": closure_ref,
            "new_work_item_id": work_item_id or f"wi_corridor_{sha256_text(corridor_id)[:16]}",
            "reason": bounded_text(f"{status}: {note}", limit=500),
            "evidence_refs": [request_id],
        }
        events.append(
            {
                "event_type": "closure_reopened",
                "schema": SCHEMA,
                "ts": now_s(),
                "corridor_id": corridor_id,
                "reopen_ref": reopen_ref,
                "packet_hash": sha256_text(json.dumps(packet or receipt, sort_keys=True, ensure_ascii=False)),
            }
        )
    if write:
        append_events(state_dir, events)
        materialize(state_dir, apply_events(derived_packets(), read_events(state_dir)))
    return {"schema": SCHEMA, "receipt": receipt, "boundary": NON_LIVE_BOUNDARY}


def render_queue_markdown(status: dict[str, Any]) -> str:
    summary = status.get("summary") or summarize(status)
    lines = [
        "# Agency Corridor V1 Queue",
        "",
        f"- packet_count: {summary.get('packet_count', 0)}",
        f"- ready_safe_lab_count: {summary.get('ready_safe_lab_count', 0)}",
        f"- reopened_work_item_count: {summary.get('reopened_work_item_count', 0)}",
        f"- live_eligible_now_count: {summary.get('live_eligible_now_count', 0)}",
        f"- auto_approved_count: {summary.get('auto_approved_count', 0)}",
        f"- boundary: {NON_LIVE_BOUNDARY}",
        "",
        "## Active Packets",
    ]
    packets = [p for p in status.get("packets", {}).values() if isinstance(p, dict)]
    packets.sort(key=lambda p: (ACTION_ORDER.get(str(p.get("action") or ""), 99), str(p.get("corridor_id") or "")))
    for packet in packets[:24]:
        lines.append(
            f"- `{packet.get('corridor_id')}` {packet.get('action')} state={packet.get('state')} "
            f"being={packet.get('being')} live_eligible_now={str(bool(packet.get('live_eligible_now'))).lower()} "
            f"auto_approved={str(bool(packet.get('auto_approved'))).lower()}"
        )
    return "\n".join(lines).rstrip() + "\n"


def render_report_markdown(status: dict[str, Any]) -> str:
    summary = status.get("summary") or summarize(status)
    lines = [
        "# Agency Corridor V1 Report",
        "",
        f"- schema_version: {SCHEMA_VERSION}",
        f"- by_action: {summary.get('by_action', {})}",
        f"- by_state: {summary.get('by_state', {})}",
        f"- safe_lab_budget_per_run: {summary.get('safe_lab_budget_per_run', 3)}",
        f"- live_eligible_now_count: {summary.get('live_eligible_now_count', 0)}",
        f"- auto_approved_count: {summary.get('auto_approved_count', 0)}",
        "",
        "## Boundary",
        NON_LIVE_BOUNDARY,
    ]
    return "\n".join(lines).rstrip() + "\n"


def resolve_v2_state_dir(state_dir: Path) -> Path:
    if state_dir == DEFAULT_STATE_DIR:
        return DEFAULT_V2_STATE_DIR
    return state_dir


def read_jsonl_records(path: Path, *, limit: int = 80) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    records: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if len(records) >= limit:
            break
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(record, dict):
            records.append(record)
    return records


def v2_source_v1_status(v2_state_dir: Path) -> dict[str, Any]:
    source_status = load_json(v2_state_dir / V1_SOURCE_STATUS_FILE)
    if source_status.get("schema") == SCHEMA:
        return source_status
    if v2_state_dir != DEFAULT_V2_STATE_DIR and (v2_state_dir / STATUS_FILE).exists():
        status = load_status(v2_state_dir)
        if status.get("schema") == SCHEMA:
            return status
    return load_status(DEFAULT_STATE_DIR)


def hard_violation_refs(value: Any, *, path: str = "$") -> list[str]:
    refs: list[str] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = f"{path}.{key}"
            if key in V2_HARD_FORBIDDEN_TRUE and child is True:
                refs.append(child_path)
            refs.extend(hard_violation_refs(child, path=child_path))
    elif isinstance(value, list):
        for idx, child in enumerate(value):
            refs.extend(hard_violation_refs(child, path=f"{path}[{idx}]"))
    return refs


def lease_for_action(action: str) -> str:
    if action in {"emit_closure_objection", "reopen_insufficient_closure"}:
        return "lease_corridor_closure_reopen_v1"
    if action in {"run_safe_lab", "compare_artifacts"}:
        return "lease_corridor_safe_labs_v1"
    if action == "request_scoped_self_observation":
        return "lease_corridor_self_observation_v1"
    return "lease_corridor_canary_source_prep_v1"


def standing_corridor_leases() -> dict[str, dict[str, Any]]:
    rows = [
        (
            "lease_corridor_closure_reopen_v1",
            "astrid/minime",
            "being objections and insufficient closures may create non-live reopened work",
            ["emit_closure_objection", "reopen_insufficient_closure"],
        ),
        (
            "lease_corridor_safe_labs_v1",
            "astrid/minime",
            "offline/read-only labs and artifact comparison may run without live authority",
            ["run_safe_lab", "compare_artifacts"],
        ),
        (
            "lease_corridor_self_observation_v1",
            "astrid/minime",
            "bounded self-observation requests and responses may be recorded",
            ["request_scoped_self_observation"],
        ),
        (
            "lease_corridor_canary_source_prep_v1",
            "astrid/minime",
            "canary criteria and source-prep proposals may be prepared as evidence only",
            ["propose_canary_criteria", "generate_replay_candidate"],
        ),
    ]
    leases: dict[str, dict[str, Any]] = {}
    for lease_id, being, scope, actions in rows:
        leases[lease_id] = {
            "schema": SCHEMA_V2,
            "schema_version": SCHEMA_VERSION_V2,
            "lease_id": lease_id,
            "source": "agency_corridor_v2",
            "being": being,
            "state": "active",
            "scope": scope,
            "allowed_actions": actions,
            "max_actions_per_run": 5,
            "expires_at": None,
            "imported_from_refs": [],
            "evidence_refs": ["Agency Corridor V2 / Autonomy Escalator V1"],
            "revocation_reason": None,
            "right_to_ignore": True,
            "grants_approval": False,
            "live_eligible_now": False,
            "auto_approved": False,
        }
    return leases


def imported_lease_records() -> dict[str, dict[str, Any]]:
    leases: dict[str, dict[str, Any]] = {}
    sources = [
        ASTRID_WORKSPACE / "self_regulation/leases.jsonl",
        *sorted((ASTRID_WORKSPACE / "action_threads").glob("threads/*/authority_gate*.jsonl"))[:40],
    ]
    for source_path in sources:
        for idx, record in enumerate(read_jsonl_records(source_path, limit=30), start=1):
            lease_id = f"imported_{sha256_text(str(source_path) + ':' + str(idx))[:16]}"
            scope = bounded_text(
                record.get("authority_boundary")
                or record.get("scope")
                or record.get("decision")
                or record.get("record_kind")
                or "existing lease/budget evidence",
                limit=360,
            )
            leases[lease_id] = {
                "schema": SCHEMA_V2,
                "schema_version": SCHEMA_VERSION_V2,
                "lease_id": lease_id,
                "source": "imported_existing_read_only_registry",
                "being": bounded_text(record.get("being") or record.get("agent") or "astrid/minime", limit=80),
                "state": "evidence_only",
                "scope": scope,
                "allowed_actions": ["compare_artifacts"],
                "max_actions_per_run": 0,
                "expires_at": None,
                "imported_from_refs": [f"{source_path}#{idx}"],
                "evidence_refs": [str(source_path)],
                "revocation_reason": None,
                "right_to_ignore": True,
                "grants_approval": False,
                "live_eligible_now": False,
                "auto_approved": False,
            }
    return leases


def apply_lease_events(leases: dict[str, dict[str, Any]], events: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    merged = {lease_id: dict(lease) for lease_id, lease in leases.items()}
    for event in events:
        if str(event.get("event_type") or "") != "lease_revoked":
            continue
        lease_id = str(event.get("lease_id") or "")
        if not lease_id:
            continue
        lease = merged.setdefault(
            lease_id,
            {
                "schema": SCHEMA_V2,
                "schema_version": SCHEMA_VERSION_V2,
                "lease_id": lease_id,
                "source": "agency_corridor_v2",
                "being": "astrid/minime",
                "scope": "revoked lease placeholder",
                "allowed_actions": [],
                "max_actions_per_run": 0,
                "imported_from_refs": [],
                "evidence_refs": [],
                "right_to_ignore": True,
                "grants_approval": False,
                "live_eligible_now": False,
                "auto_approved": False,
            },
        )
        lease["state"] = "revoked"
        lease["revocation_reason"] = bounded_text(event.get("reason"), limit=400)
    return merged


def lease_summary(leases: dict[str, dict[str, Any]]) -> dict[str, Any]:
    states = Counter(str(lease.get("state") or "unknown") for lease in leases.values())
    violations = hard_violation_refs(leases)
    return {
        "lease_count": len(leases),
        "by_state": dict(sorted(states.items())),
        "active_count": states.get("active", 0),
        "revoked_count": states.get("revoked", 0),
        "imported_evidence_only_count": states.get("evidence_only", 0),
        "live_violation_count": len(violations),
        "live_violation_refs": violations[:20],
    }


def generate_leases_v2(state_dir: Path, *, write: bool) -> dict[str, Any]:
    state_dir = resolve_v2_state_dir(state_dir)
    leases = standing_corridor_leases()
    leases.update(imported_lease_records())
    leases = apply_lease_events(leases, read_events(state_dir))
    payload = {
        "schema": SCHEMA_V2,
        "schema_version": SCHEMA_VERSION_V2,
        "generated_at": iso_now(),
        "leases": leases,
        "summary": lease_summary(leases),
        "boundary": NON_LIVE_BOUNDARY,
    }
    if write:
        atomic_write_text(state_dir / LEASES_FILE, json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
    return payload


def source_prep_proposal_for_packet(packet: dict[str, Any]) -> dict[str, Any] | None:
    action = str(packet.get("action") or "")
    if action not in {"propose_canary_criteria", "generate_replay_candidate"} and not packet.get("authority_boundary_ids"):
        return None
    surface = "agency_corridor"
    canary = packet.get("canary_criteria") if isinstance(packet.get("canary_criteria"), dict) else {}
    if canary:
        surface = str(canary.get("surface") or surface)
    elif packet.get("source"):
        surface = str(packet.get("source"))
    proposal_id = stable_uuid_v2("source_prep", packet.get("corridor_id"), surface)
    return {
        "schema": "agency_corridor_source_prep_proposal_v1",
        "schema_version": SCHEMA_VERSION_V2,
        "proposal_id": proposal_id,
        "corridor_id": packet.get("corridor_id"),
        "surface": surface,
        "bounded_plan": bounded_text(
            "Prepare an implementation patch plan and tests for later human/agent review. "
            "Do not edit source in this proposal step; keep live candidates non-runnable and non-approved.",
            limit=700,
        ),
        "files": [
            "scripts/agency_corridor.py",
            "scripts/introspection_addressing_audit.py",
            "scripts/sandbox_trial_queue.py",
            "capsules/spectral-bridge/src/autonomous/next_action/operations.rs",
        ],
        "tests_to_run": [
            "python3 scripts/agency_corridor.py --self-test",
            "python3 scripts/introspection_addressing_audit.py --self-test",
            "python3 scripts/sandbox_trial_queue.py --self-test",
            "bridge parser tests if bridge affordances change",
        ],
        "restart_required": surface in {"spectral_bridge", "authority_gate", "codec", "agency_corridor"},
        "edits_source_now": False,
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
        "right_to_ignore": True,
    }


def v2_packet_for_v1(packet: dict[str, Any], leases: dict[str, dict[str, Any]]) -> dict[str, Any]:
    v1_id = str(packet.get("corridor_id") or "")
    action = str(packet.get("action") or "generate_replay_candidate")
    lease_id = lease_for_action(action)
    authority_ids = [str(packet.get("authority_boundary_id"))] if packet.get("authority_boundary_id") else []
    v2 = {
        "schema": SCHEMA_V2,
        "schema_version": SCHEMA_VERSION_V2,
        "corridor_id": stable_uuid_v2("packet", v1_id),
        "v1_corridor_id": v1_id,
        "source": packet.get("source") or "agency_corridor_v1",
        "being": packet.get("being") or "astrid",
        "action": action,
        "state": packet.get("state") or "evidence_only",
        "authority_boundary_ids": authority_ids,
        "work_item_ids": packet.get("work_item_ids") or [],
        "closure_card_refs": packet.get("closure_card_refs") or [],
        "sandbox_trial_ids": packet.get("sandbox_trial_ids") or [],
        "delta_refs": packet.get("delta_refs") or [],
        "felt_report_anchor": bounded_text(packet.get("felt_report_anchor"), limit=520),
        "proposed_corridor_action": bounded_text(packet.get("proposed_corridor_action"), limit=700),
        "evidence_refs": packet.get("evidence_refs") or [],
        "safe_lab_candidate": packet.get("safe_lab_candidate"),
        "closure_objection": packet.get("closure_objection"),
        "closure_reopen_ref": packet.get("closure_reopen_ref"),
        "self_observation_request": packet.get("self_observation_request"),
        "canary_criteria": packet.get("canary_criteria"),
        "autonomy_lease": leases.get(lease_id),
        "queue_step": None,
        "source_prep_proposal": None,
        "closure_reopen_policy": {
            "policy_id": "closure_reopen_policy_v1",
            "auto_reopen_on_objection": True,
            "auto_reopen_on_still_friction": True,
            "creates_non_live_work_only": True,
            "triggers": ["being_objection", "still_friction", "contradicted_post_change_response"],
        },
        "receipts": [],
        "who_can_escalate": packet.get("who_can_escalate") or "steward/operator through existing authority boundary",
        "how_to_test_it": "inspect V2 packet, lease, adaptive queue step, receipts, and linked bounded evidence",
        "right_to_ignore": True,
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
        "boundary": NON_LIVE_BOUNDARY,
    }
    proposal = source_prep_proposal_for_packet(v2)
    if proposal:
        v2["source_prep_proposal"] = proposal
    normalize_artifact_authority_tree(v2)
    return v2


def build_queue_steps(packets: dict[str, dict[str, Any]], live_violation_refs: list[str]) -> list[dict[str, Any]]:
    if live_violation_refs:
        return []

    def receipt_exists(packet: dict[str, Any], step_id: str) -> bool:
        return any(
            isinstance(receipt, dict) and str(receipt.get("step_id") or "") == step_id
            for receipt in packet.get("receipts", [])
        )

    steps: list[dict[str, Any]] = []
    for packet in packets.values():
        action = str(packet.get("action") or "generate_replay_candidate")
        corridor_id = str(packet.get("corridor_id") or "")
        state = str(packet.get("state") or "")
        priority = V2_ACTION_ORDER.get(action, 9)
        step_id = stable_uuid_v2("step", corridor_id, action)
        runnable = action in SAFE_ACTIONS and action != "propose_canary_criteria"
        if state in {"closed", "safe_lab_result_recorded"} or receipt_exists(packet, step_id):
            runnable = False
        step = {
            "schema": SCHEMA_V2,
            "schema_version": SCHEMA_VERSION_V2,
            "step_id": step_id,
            "priority": priority,
            "action": action,
            "corridor_id": corridor_id,
            "v1_corridor_id": packet.get("v1_corridor_id"),
            "lease_id": lease_for_action(action),
            "reason": bounded_text(packet.get("proposed_corridor_action") or packet.get("felt_report_anchor"), limit=500),
            "runnable": runnable,
            "evidence_refs": packet.get("evidence_refs") or [],
            "source_prep_proposal_id": None,
            "grants_approval": False,
            "live_eligible_now": False,
            "auto_approved": False,
        }
        packet["queue_step"] = dict(step)
        steps.append(step)
        if isinstance(packet.get("source_prep_proposal"), dict):
            proposal = packet["source_prep_proposal"]
            source_prep_step_id = stable_uuid_v2("step", corridor_id, "source_prep")
            steps.append(
                {
                    "schema": SCHEMA_V2,
                    "schema_version": SCHEMA_VERSION_V2,
                    "step_id": source_prep_step_id,
                    "priority": V2_ACTION_ORDER["generate_replay_candidate"],
                    "action": "generate_replay_candidate",
                    "corridor_id": corridor_id,
                    "v1_corridor_id": packet.get("v1_corridor_id"),
                    "lease_id": "lease_corridor_canary_source_prep_v1",
                    "reason": "prepare bounded source proposal artifact only",
                    "runnable": not receipt_exists(packet, source_prep_step_id),
                    "evidence_refs": packet.get("evidence_refs") or [],
                    "source_prep_proposal_id": proposal.get("proposal_id"),
                    "grants_approval": False,
                    "live_eligible_now": False,
                    "auto_approved": False,
                }
            )
    steps.sort(
        key=lambda step: (
            int(step["priority"]) if isinstance(step.get("priority"), int) else 99,
            str(step.get("step_id") or ""),
        )
    )
    return steps


def apply_v2_events(status: dict[str, Any], events: list[dict[str, Any]]) -> None:
    for event in events:
        event_type = str(event.get("event_type") or "")
        if event_type == "corridor_packet_declared_v2" and isinstance(event.get("packet"), dict):
            packet = dict(event["packet"])
            status["packets"][str(packet.get("corridor_id"))] = packet
        elif event_type == "corridor_receipt_recorded_v2":
            corridor_id = str(event.get("corridor_id") or "")
            packet = status["packets"].get(corridor_id)
            if packet and isinstance(event.get("receipt"), dict):
                packet.setdefault("receipts", []).append(event["receipt"])
                if packet.get("state") == "safe_lab_ready":
                    packet["state"] = "safe_lab_result_recorded"
        elif event_type == "source_prep_proposal_recorded" and isinstance(event.get("proposal"), dict):
            proposal = event["proposal"]
            status["source_prep_proposals"][str(proposal.get("proposal_id"))] = proposal
        elif event_type == "closure_reopened_v2" and isinstance(event.get("reopen_ref"), dict):
            ref = event["reopen_ref"]
            status["reopened_work_items"][str(ref.get("reopen_id"))] = ref


def load_existing_portfolios(state_dir: Path) -> dict[str, dict[str, Any]]:
    portfolios: dict[str, dict[str, Any]] = {}
    for path in sorted((state_dir / PORTFOLIO_DIR).glob("*.json"))[:200]:
        payload = load_json(path)
        portfolio = payload.get("portfolio") if isinstance(payload.get("portfolio"), dict) else payload
        portfolio_id = str(portfolio.get("portfolio_id") or "")
        if portfolio_id:
            portfolios[portfolio_id] = portfolio
    return portfolios


def load_existing_patch_bundles(state_dir: Path) -> dict[str, dict[str, Any]]:
    bundles: dict[str, dict[str, Any]] = {}
    for path in sorted((state_dir / PATCH_BUNDLE_DIR).glob("*.json"))[:200]:
        payload = load_json(path)
        bundle = payload.get("bundle") if isinstance(payload.get("bundle"), dict) else payload
        bundle_id = str(bundle.get("bundle_id") or "")
        if bundle_id:
            bundles[bundle_id] = bundle
    return bundles


def load_existing_program_receipts(state_dir: Path) -> dict[str, dict[str, Any]]:
    receipts: dict[str, dict[str, Any]] = {}
    for path in sorted((state_dir / PROGRAM_RECEIPT_DIR).glob("*.json"))[:500]:
        receipt = load_json(path)
        receipt_id = str(receipt.get("receipt_id") or "")
        if receipt_id:
            receipts[receipt_id] = receipt
    return receipts


def unique_strings(values: list[Any]) -> list[str]:
    seen: set[str] = set()
    result: list[str] = []
    for value in values:
        text = str(value or "").strip()
        if text and text not in seen:
            seen.add(text)
            result.append(text)
    return result


def program_surface(packet: dict[str, Any]) -> str:
    if isinstance(packet.get("canary_criteria"), dict):
        surface = str(packet["canary_criteria"].get("surface") or "")
        if surface:
            return surface
    if isinstance(packet.get("source_prep_proposal"), dict):
        surface = str(packet["source_prep_proposal"].get("surface") or "")
        if surface:
            return surface
    source = str(packet.get("source") or "agency_corridor")
    return source.split("/")[-1] or "agency_corridor"


def felt_anchor_key(anchor: str) -> str:
    words = [
        word.strip(".,;:!?()[]{}\"'").lower()
        for word in str(anchor or "").split()
        if len(word.strip(".,;:!?()[]{}\"'")) >= 5
    ]
    return "-".join(words[:4]) or sha256_text(str(anchor or ""))[:12]


def program_group_key(packet: dict[str, Any]) -> str:
    work_items = [str(ref) for ref in packet.get("work_item_ids", []) if ref]
    if work_items:
        return f"work_item:{work_items[0]}"
    boundaries = [str(ref) for ref in packet.get("authority_boundary_ids", []) if ref]
    if boundaries:
        return f"authority_boundary:{boundaries[0]}"
    trials = [str(ref) for ref in packet.get("sandbox_trial_ids", []) if ref]
    if trials:
        return f"sandbox_trial:{trials[0]}"
    return f"{program_surface(packet)}:{felt_anchor_key(str(packet.get('felt_report_anchor') or ''))}"


def clamp_score(value: float) -> int:
    return max(0, min(1000, int(round(value))))


def program_priority_signal(
    program_id: str,
    packets: list[dict[str, Any]],
    events: list[dict[str, Any]],
) -> dict[str, Any]:
    program_events = [
        event for event in events
        if str(event.get("program_id") or "") == program_id
        or str((event.get("program") or {}).get("program_id") if isinstance(event.get("program"), dict) else "") == program_id
    ]
    objection_count = sum(
        1
        for packet in packets
        if packet.get("action") in {"emit_closure_objection", "reopen_insufficient_closure"}
        or isinstance(packet.get("closure_objection"), dict)
        or isinstance(packet.get("closure_reopen_ref"), dict)
    ) + sum(1 for event in program_events if str(event.get("event_type") or "") == "program_objection_recorded")
    beings = {str(packet.get("being") or "astrid") for packet in packets}
    evidence_count = sum(len(packet.get("evidence_refs") or []) for packet in packets)
    live_wait = any(packet.get("authority_boundary_ids") for packet in packets)
    evidence_only_actions = all(
        str(packet.get("action") or "") in {
            "run_safe_lab",
            "compare_artifacts",
            "request_scoped_self_observation",
            "emit_closure_objection",
            "reopen_insufficient_closure",
            "propose_canary_criteria",
            "generate_replay_candidate",
        }
        for packet in packets
    )
    safety_ready = not hard_violation_refs(packets)
    being_salience = clamp_score(
        420
        + objection_count * 220
        + (180 if any("!" in str(ref) for packet in packets for ref in packet.get("evidence_refs", [])) else 0)
    )
    recurrence = clamp_score(len(packets) * 180)
    convergence = 1000 if len(beings) > 1 else 250
    stale_age = clamp_score(100 + evidence_count * 35 + len(program_events) * 50)
    safety = 900 if safety_ready else 0
    deterministic = clamp_score(
        (0.40 * being_salience)
        + (0.20 * recurrence)
        + (0.20 * convergence)
        + (0.10 * stale_age)
        + (0.10 * safety)
    )
    live_wait_demoted = bool(live_wait and not evidence_only_actions)
    if live_wait_demoted:
        deterministic = min(deterministic, 300)
    return {
        "schema": "agency_priority_signal_v1",
        "schema_version": 1,
        "program_id": program_id,
        "being_salience_score": being_salience,
        "recurrence_score": recurrence,
        "cross_being_convergence_score": convergence,
        "stale_age_score": stale_age,
        "safety_readiness_score": safety,
        "deterministic_score": deterministic,
        "basis_refs": unique_strings(
            [
                f"packets={len(packets)}",
                f"objections={objection_count}",
                f"beings={','.join(sorted(beings))}",
                f"live_wait={str(bool(live_wait)).lower()}",
            ]
        ),
        "live_wait_demoted": live_wait_demoted,
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
    }


def work_program_for_group(
    group_key: str,
    packets: list[dict[str, Any]],
    events: list[dict[str, Any]],
) -> dict[str, Any]:
    packets = sorted(packets, key=lambda packet: str(packet.get("corridor_id") or ""))
    first = packets[0]
    program_id = stable_uuid_v2("work_program", group_key)
    priority = program_priority_signal(program_id, packets, events)
    source_prep_count = sum(1 for packet in packets if isinstance(packet.get("source_prep_proposal"), dict))
    objection_count = sum(
        1
        for packet in packets
        if packet.get("action") in {"emit_closure_objection", "reopen_insufficient_closure"}
        or isinstance(packet.get("closure_objection"), dict)
        or isinstance(packet.get("closure_reopen_ref"), dict)
    )
    current_next = PROGRAM_STEP_UPDATE_PORTFOLIO
    if objection_count:
        current_next = PROGRAM_STEP_RECORD_OBJECTION
    elif source_prep_count:
        current_next = PROGRAM_STEP_PREPARE_PATCH_BUNDLE
    status = "blocked" if hard_violation_refs(packets) else "active"
    return {
        "schema": "agency_work_program_v1",
        "schema_version": 1,
        "program_id": program_id,
        "group_key": group_key,
        "being": bounded_text(first.get("being") or "astrid", limit=80),
        "title": bounded_text(f"{program_surface(first)} evidence program", limit=120),
        "hypothesis": bounded_text(
            first.get("felt_report_anchor") or first.get("proposed_corridor_action") or "non-live corridor evidence can be organized into a durable work program",
            limit=700,
        ),
        "goals": unique_strings(
            [
                "accumulate bounded evidence across runs",
                "preserve being-authored objections and reopenings",
                "prepare review-only patch bundles when source prep is warranted",
            ]
        ),
        "status": status,
        "linked_corridor_ids": unique_strings([packet.get("corridor_id") for packet in packets]),
        "authority_boundary_ids": unique_strings([ref for packet in packets for ref in packet.get("authority_boundary_ids", [])]),
        "work_item_ids": unique_strings([ref for packet in packets for ref in packet.get("work_item_ids", [])]),
        "sandbox_trial_ids": unique_strings([ref for packet in packets for ref in packet.get("sandbox_trial_ids", [])]),
        "delta_refs": [ref for packet in packets for ref in packet.get("delta_refs", []) if isinstance(ref, dict)][:20],
        "stop_conditions": [
            "hard live/approval violation appears in corridor or program artifacts",
            "being objection is resolved by later evidence or response",
            "operator explicitly revokes the non-live lease",
        ],
        "priority_signal": priority,
        "current_next_action": current_next,
        "evidence_refs": unique_strings([ref for packet in packets for ref in packet.get("evidence_refs", [])])[:40],
        "right_to_ignore": True,
        "edits_source_now": False,
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
    }


def portfolio_for_program(
    state_dir: Path,
    program: dict[str, Any],
    packets: list[dict[str, Any]],
    events: list[dict[str, Any]],
) -> dict[str, Any]:
    program_id = str(program.get("program_id") or "")
    portfolio_id = stable_uuid_v2("portfolio", program_id)
    bundle_refs = [
        str(bundle.get("bundle_id"))
        for bundle in load_existing_patch_bundles(state_dir).values()
        if str(bundle.get("program_id") or "") == program_id
    ]
    packet_ids = set(program.get("linked_corridor_ids") or [])
    program_events = [
        event
        for event in events
        if str(event.get("program_id") or "") == program_id
        or str(event.get("corridor_id") or "") in packet_ids
    ]
    source_prep_refs = unique_strings(
        [
            (packet.get("source_prep_proposal") or {}).get("proposal_id")
            for packet in packets
            if isinstance(packet.get("source_prep_proposal"), dict)
        ]
        + [
            str(event.get("proposal_id") or "")
            for event in program_events
            if str(event.get("event_type") or "") == "source_prep_proposal_recorded"
        ]
    )
    objection_refs = unique_strings(
        [
            (packet.get("closure_objection") or {}).get("objection_id")
            for packet in packets
            if isinstance(packet.get("closure_objection"), dict)
        ]
        + [
            str(event.get("objection_id") or "")
            for event in program_events
            if str(event.get("event_type") or "") == "program_objection_recorded"
        ]
    )
    reopen_refs = unique_strings(
        [
            (packet.get("closure_reopen_ref") or {}).get("reopen_id")
            for packet in packets
            if isinstance(packet.get("closure_reopen_ref"), dict)
        ]
    )
    result_refs = unique_strings(
        [
            str(event.get("result_path") or "")
            for event in program_events
            if str(event.get("event_type") or "").endswith("receipt_recorded_v2")
        ]
    )
    card_refs = unique_strings(
        [
            str(path)
            for path in sorted((state_dir / CARD_DIR).glob("*.md"))[-60:]
            if any(corridor_id in path.name for corridor_id in packet_ids)
        ]
    )
    return {
        "schema": "evidence_portfolio_v1",
        "schema_version": 1,
        "portfolio_id": portfolio_id,
        "program_id": program_id,
        "being": program.get("being") or "astrid",
        "bounded_felt_anchors": unique_strings([packet.get("felt_report_anchor") for packet in packets])[:12],
        "linked_introspections": unique_strings(
            [
                ref
                for packet in packets
                for ref in packet.get("evidence_refs", [])
                if "introspection" in str(ref)
            ]
        )[:30],
        "linked_results": result_refs,
        "linked_cards": card_refs,
        "linked_source_prep": source_prep_refs,
        "linked_objections": objection_refs,
        "linked_reopens": reopen_refs,
        "linked_patch_bundles": bundle_refs,
        "current_recommendation": bounded_text(program.get("current_next_action"), limit=400),
        "unknowns": [
            "whether the next safe artifact changes the being's felt objection",
            "whether a later source implementation is warranted after review",
        ],
        "private_refs": unique_strings([ref for packet in packets for ref in packet.get("evidence_refs", [])])[:30],
        "hash_refs": [sha256_text(json.dumps(program, sort_keys=True, ensure_ascii=False))],
        "closure_state": "open" if program.get("status") != "closed" else "closed",
        "right_to_ignore": True,
        "edits_source_now": False,
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
    }


def render_programs_markdown(payload: dict[str, Any]) -> str:
    summary = payload.get("summary", {})
    lines = [
        "# Agency Work Programs V1",
        "",
        f"- program_count: {summary.get('program_count', 0)}",
        f"- active_count: {summary.get('active_count', 0)}",
        f"- patch_bundle_count: {summary.get('patch_bundle_count', 0)}",
        f"- top_priority_score: {summary.get('top_priority_score', 0)}",
        f"- live_violation_count: {summary.get('live_violation_count', 0)}",
        f"- boundary: {NON_LIVE_BOUNDARY}",
        "",
        "## Programs",
    ]
    programs = [program for program in payload.get("programs", {}).values() if isinstance(program, dict)]
    programs.sort(key=lambda program: (-int((program.get("priority_signal") or {}).get("deterministic_score") or 0), str(program.get("program_id") or "")))
    for program in programs[:30]:
        priority = program.get("priority_signal") or {}
        lines.append(
            f"- `{program.get('program_id')}` score={priority.get('deterministic_score', 0)} "
            f"status={program.get('status')} next={program.get('current_next_action')} title={program.get('title')}"
        )
    return "\n".join(lines).rstrip() + "\n"


def render_portfolio_markdown(portfolio: dict[str, Any]) -> str:
    lines = [
        "# Evidence Portfolio V1",
        "",
        f"- portfolio_id: {portfolio.get('portfolio_id')}",
        f"- program_id: {portfolio.get('program_id')}",
        f"- being: {portfolio.get('being')}",
        f"- closure_state: {portfolio.get('closure_state')}",
        "- edits_source_now: false",
        "- grants_approval: false",
        "- live_eligible_now: false",
        "- auto_approved: false",
        "",
        "## Recommendation",
        bounded_text(portfolio.get("current_recommendation"), limit=700),
        "",
        "## Felt Anchors",
        *[f"- {bounded_text(anchor, limit=240)}" for anchor in portfolio.get("bounded_felt_anchors", [])[:12]],
        "",
        "## Unknowns",
        *[f"- {bounded_text(unknown, limit=240)}" for unknown in portfolio.get("unknowns", [])[:12]],
        "",
        "## Boundary",
        NON_LIVE_BOUNDARY,
    ]
    return "\n".join(lines).rstrip() + "\n"


def summarize_programs(
    programs: dict[str, dict[str, Any]],
    portfolios: dict[str, dict[str, Any]],
    patch_bundles: dict[str, dict[str, Any]],
    program_receipts: dict[str, dict[str, Any]] | None = None,
) -> dict[str, Any]:
    program_receipts = program_receipts or {}
    scores = [
        int((program.get("priority_signal") or {}).get("deterministic_score") or 0)
        for program in programs.values()
    ]
    violations = hard_violation_refs(
        {
            "programs": programs,
            "portfolios": portfolios,
            "patch_bundles": patch_bundles,
        }
    )
    return {
        "program_count": len(programs),
        "active_count": sum(1 for program in programs.values() if program.get("status") == "active"),
        "portfolio_count": len(portfolios),
        "patch_bundle_count": len(patch_bundles),
        "program_receipt_count": len(program_receipts),
        "top_priority_score": max(scores) if scores else 0,
        "live_violation_count": len(violations),
        "live_violation_refs": violations[:20],
    }


def generate_programs_v2(state_dir: Path, *, write: bool) -> dict[str, Any]:
    state_dir = resolve_v2_state_dir(state_dir)
    status = generate_v2_status(state_dir, write=False)
    events = read_events(state_dir)
    groups: dict[str, list[dict[str, Any]]] = {}
    for packet in status.get("packets", {}).values():
        if isinstance(packet, dict):
            groups.setdefault(program_group_key(packet), []).append(packet)
    programs = {
        work_program["program_id"]: work_program
        for group_key, packets in groups.items()
        if packets
        for work_program in [work_program_for_group(group_key, packets, events)]
    }
    for event in events:
        if str(event.get("event_type") or "") == "program_note_recorded":
            program_id = str(event.get("program_id") or "")
            if program_id in programs:
                programs[program_id].setdefault("evidence_refs", []).append(str(event.get("note_id") or "program_note"))
        elif str(event.get("event_type") or "") == "program_objection_recorded":
            program_id = str(event.get("program_id") or "")
            if program_id in programs:
                programs[program_id]["current_next_action"] = PROGRAM_STEP_RECORD_OBJECTION
                programs[program_id].setdefault("evidence_refs", []).append(str(event.get("objection_id") or "program_objection"))
    portfolios = {}
    for program in programs.values():
        packet_ids = set(program.get("linked_corridor_ids") or [])
        packets = [
            packet for packet in status.get("packets", {}).values()
            if isinstance(packet, dict) and str(packet.get("corridor_id") or "") in packet_ids
        ]
        portfolio = portfolio_for_program(state_dir, program, packets, events)
        portfolios[portfolio["portfolio_id"]] = portfolio
    patch_bundles = load_existing_patch_bundles(state_dir)
    program_receipts = load_existing_program_receipts(state_dir)
    summary = summarize_programs(programs, portfolios, patch_bundles, program_receipts)
    payload = {
        "schema": "agency_work_programs_v1",
        "schema_version": 1,
        "generated_at": iso_now(),
        "programs": programs,
        "portfolios": portfolios,
        "patch_bundles": patch_bundles,
        "program_receipts": program_receipts,
        "summary": summary,
        "boundary": NON_LIVE_BOUNDARY,
    }
    if write:
        atomic_write_text(state_dir / PROGRAMS_FILE, json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
        atomic_write_text(state_dir / PROGRAMS_MD_FILE, render_programs_markdown(payload))
        for portfolio in portfolios.values():
            path = state_dir / PORTFOLIO_DIR / f"{portfolio['portfolio_id']}.json"
            atomic_write_text(path, json.dumps({"schema": "evidence_portfolio_v1", "portfolio": portfolio}, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
            atomic_write_text(path.with_suffix(".md"), render_portfolio_markdown(portfolio))
    return payload


def summarize_v2(status: dict[str, Any]) -> dict[str, Any]:
    packets = [p for p in status.get("packets", {}).values() if isinstance(p, dict)]
    queue = status.get("queue") if isinstance(status.get("queue"), dict) else {}
    steps = [s for s in queue.get("steps", []) if isinstance(s, dict)]
    violations = hard_violation_refs(
        {
            "packets": packets,
            "leases": status.get("leases", {}),
            "queue": queue,
            "source_prep_proposals": status.get("source_prep_proposals", {}),
        }
    )
    by_action = Counter(str(p.get("action") or "unknown") for p in packets)
    by_state = Counter(str(p.get("state") or "unknown") for p in packets)
    return {
        "packet_count": len(packets),
        "lease_count": len(status.get("leases", {})),
        "queue_step_count": len(steps),
        "queue_runnable_count": sum(1 for step in steps if step.get("runnable") is True),
        "source_prep_proposal_count": len(status.get("source_prep_proposals", {})),
        "program_count": len(status.get("programs", {})),
        "portfolio_count": len(status.get("portfolios", {})),
        "patch_bundle_count": len(status.get("patch_bundles", {})),
        "top_priority_score": int((status.get("program_summary") or {}).get("top_priority_score") or 0),
        "reopened_work_item_count": len(status.get("reopened_work_items", {})),
        "by_action": dict(sorted(by_action.items())),
        "by_state": dict(sorted(by_state.items())),
        "live_violation_count": len(violations),
        "live_violation_refs": violations[:20],
        "safe_lab_budget_per_run": 5,
    }


def render_v2_queue_markdown(status: dict[str, Any]) -> str:
    summary = status.get("summary") or summarize_v2(status)
    queue = status.get("queue") if isinstance(status.get("queue"), dict) else {}
    lines = [
        "# Agency Corridor V2 Adaptive Queue",
        "",
        f"- packet_count: {summary.get('packet_count', 0)}",
        f"- lease_count: {summary.get('lease_count', 0)}",
        f"- queue_step_count: {summary.get('queue_step_count', 0)}",
        f"- queue_runnable_count: {summary.get('queue_runnable_count', 0)}",
        f"- source_prep_proposal_count: {summary.get('source_prep_proposal_count', 0)}",
        f"- program_count: {summary.get('program_count', 0)}",
        f"- portfolio_count: {summary.get('portfolio_count', 0)}",
        f"- patch_bundle_count: {summary.get('patch_bundle_count', 0)}",
        f"- top_priority_score: {summary.get('top_priority_score', 0)}",
        f"- live_violation_count: {summary.get('live_violation_count', 0)}",
        f"- boundary: {NON_LIVE_BOUNDARY}",
        "",
        "## Steps",
    ]
    for step in queue.get("steps", [])[:30]:
        if not isinstance(step, dict):
            continue
        lines.append(
            f"- `{step.get('step_id')}` p={step.get('priority')} action={step.get('action')} "
            f"runnable={str(bool(step.get('runnable'))).lower()} lease={step.get('lease_id')} "
            f"corridor={step.get('corridor_id')}"
        )
    return "\n".join(lines).rstrip() + "\n"


def render_v2_report_markdown(status: dict[str, Any]) -> str:
    summary = status.get("summary") or summarize_v2(status)
    lease_status = lease_summary(status.get("leases", {}))
    lines = [
        "# Agency Corridor V2 Report",
        "",
        f"- schema_version: {SCHEMA_VERSION_V2}",
        f"- leases: {lease_status}",
        f"- by_action: {summary.get('by_action', {})}",
        f"- by_state: {summary.get('by_state', {})}",
        f"- queue_runnable_count: {summary.get('queue_runnable_count', 0)}",
        f"- source_prep_proposal_count: {summary.get('source_prep_proposal_count', 0)}",
        f"- program_count: {summary.get('program_count', 0)}",
        f"- portfolio_count: {summary.get('portfolio_count', 0)}",
        f"- patch_bundle_count: {summary.get('patch_bundle_count', 0)}",
        f"- top_priority_score: {summary.get('top_priority_score', 0)}",
        f"- live_violation_count: {summary.get('live_violation_count', 0)}",
        "",
        "## Boundary",
        NON_LIVE_BOUNDARY,
    ]
    return "\n".join(lines).rstrip() + "\n"


def materialize_v2(state_dir: Path, status: dict[str, Any]) -> None:
    normalize_artifact_authority_tree(status)
    atomic_write_text(state_dir / STATUS_FILE, json.dumps(status, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
    atomic_write_text(
        state_dir / LEASES_FILE,
        json.dumps({"schema": SCHEMA_V2, "leases": status.get("leases", {}), "summary": lease_summary(status.get("leases", {}))}, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
    )
    atomic_write_text(state_dir / QUEUE_JSON_FILE, json.dumps(status.get("queue", {}), indent=2, sort_keys=True, ensure_ascii=False) + "\n")
    atomic_write_text(state_dir / QUEUE_FILE, render_v2_queue_markdown(status))
    atomic_write_text(state_dir / REPORT_FILE, render_v2_report_markdown(status))


def generate_v2_status(state_dir: Path, *, write: bool) -> dict[str, Any]:
    state_dir = resolve_v2_state_dir(state_dir)
    leases_payload = generate_leases_v2(state_dir, write=False)
    leases = leases_payload["leases"]
    v1_status = v2_source_v1_status(state_dir)
    packets = {
        str(v2_packet.get("corridor_id")): v2_packet
        for v2_packet in (
            v2_packet_for_v1(packet, leases)
            for packet in v1_status.get("packets", {}).values()
            if isinstance(packet, dict)
        )
    }
    source_prep = {
        str(packet["source_prep_proposal"]["proposal_id"]): packet["source_prep_proposal"]
        for packet in packets.values()
        if isinstance(packet.get("source_prep_proposal"), dict)
    }
    status: dict[str, Any] = {
        "schema": SCHEMA_V2,
        "schema_version": SCHEMA_VERSION_V2,
        "generated_at": iso_now(),
        "boundary": NON_LIVE_BOUNDARY,
        "leases": leases,
        "packets": packets,
        "source_prep_proposals": source_prep,
        "reopened_work_items": {},
        "self_observation_responses": [],
        "queue": {},
        "programs": {},
        "portfolios": load_existing_portfolios(state_dir),
        "patch_bundles": load_existing_patch_bundles(state_dir),
        "program_summary": {},
    }
    apply_v2_events(status, read_events(state_dir))
    live_violation_refs = hard_violation_refs(
        {
            "packets": status["packets"],
            "leases": status["leases"],
            "source_prep_proposals": status["source_prep_proposals"],
        }
    )
    steps = build_queue_steps(status["packets"], live_violation_refs)
    queue = {
        "schema": SCHEMA_V2,
        "schema_version": SCHEMA_VERSION_V2,
        "queue_id": stable_uuid_v2("queue", status["generated_at"], len(steps)),
        "generated_at": status["generated_at"],
        "max_steps_per_run": 5,
        "steps": steps,
        "blocked_by_live_violation": bool(live_violation_refs),
        "live_violation_refs": live_violation_refs[:50],
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
    }
    status["queue"] = queue
    programs_payload = load_json(state_dir / PROGRAMS_FILE)
    if isinstance(programs_payload.get("programs"), dict):
        status["programs"] = programs_payload["programs"]
    if isinstance(programs_payload.get("summary"), dict):
        status["program_summary"] = programs_payload["summary"]
    status["summary"] = summarize_v2(status)
    if write:
        materialize_v2(state_dir, status)
    return status


def load_v2_status(state_dir: Path) -> dict[str, Any]:
    state_dir = resolve_v2_state_dir(state_dir)
    status = load_json(state_dir / STATUS_FILE)
    if status and status.get("schema") == SCHEMA_V2:
        return status
    return generate_v2_status(state_dir, write=False)


def card_text_v2(packet: dict[str, Any], step: dict[str, Any] | None = None) -> str:
    lines = [
        "# Agency Corridor Card V2",
        "",
        f"- corridor_id: {packet.get('corridor_id')}",
        f"- v1_corridor_id: {packet.get('v1_corridor_id')}",
        f"- source: {packet.get('source')}",
        f"- being: {packet.get('being')}",
        f"- action: {packet.get('action')}",
        f"- state: {packet.get('state')}",
        "- right_to_ignore: true",
        f"- grants_approval: {str(bool(packet.get('grants_approval'))).lower()}",
        f"- live_eligible_now: {str(bool(packet.get('live_eligible_now'))).lower()}",
        f"- auto_approved: {str(bool(packet.get('auto_approved'))).lower()}",
        "",
        "## Felt Anchor",
        bounded_text(packet.get("felt_report_anchor"), limit=700),
        "",
        "## Lease",
        "```json",
        json.dumps(packet.get("autonomy_lease") or {}, indent=2, sort_keys=True, ensure_ascii=False),
        "```",
    ]
    if step:
        lines.extend(["", "## Queue Step", "```json", json.dumps(step, indent=2, sort_keys=True, ensure_ascii=False), "```"])
    if packet.get("source_prep_proposal"):
        lines.extend(["", "## Source Prep Proposal", "```json", json.dumps(packet["source_prep_proposal"], indent=2, sort_keys=True, ensure_ascii=False), "```"])
    if packet.get("safe_lab_candidate"):
        lines.extend(["", "## Safe Lab Candidate", "```json", json.dumps(packet["safe_lab_candidate"], indent=2, sort_keys=True, ensure_ascii=False), "```"])
    if packet.get("self_observation_request"):
        lines.extend(["", "## Self Observation Request", "```json", json.dumps(packet["self_observation_request"], indent=2, sort_keys=True, ensure_ascii=False), "```"])
    if packet.get("canary_criteria"):
        lines.extend(["", "## Canary Criteria Proposal", "```json", json.dumps(packet["canary_criteria"], indent=2, sort_keys=True, ensure_ascii=False), "```"])
    lines.extend(["", "## Boundary", NON_LIVE_BOUNDARY])
    return "\n".join(lines).rstrip() + "\n"


def emit_card_for_packet_v2(
    state_dir: Path,
    packet: dict[str, Any],
    *,
    step: dict[str, Any] | None = None,
    write: bool,
    deliver_safe: bool = True,
) -> dict[str, Any]:
    state_dir = resolve_v2_state_dir(state_dir)
    ts = int(now_s())
    text = card_text_v2(packet, step=step)
    path = state_dir / CARD_DIR / f"{ts}_{packet['corridor_id']}.md"
    delivered_path: Path | None = None
    if write:
        atomic_write_text(path, text)
        safe_to_deliver = deliver_safe and packet.get("action") not in {"propose_canary_criteria"}
        if safe_to_deliver:
            inbox = being_inbox(str(packet.get("being") or "astrid"), state_dir=state_dir)
            delivered_path = inbox / f"agency_corridor_v2_{packet['corridor_id']}_{ts}.txt"
            atomic_write_text(delivered_path, text)
    return {
        "schema": "agency_corridor_card_v2",
        "corridor_id": packet.get("corridor_id"),
        "path": str(path),
        "delivered_path": str(delivered_path) if delivered_path else None,
        "text_sha256": sha256_text(text),
        "right_to_ignore": True,
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
    }


def receipt_v2_for_step(packet: dict[str, Any], step: dict[str, Any], summary: str, evidence_refs: list[str]) -> dict[str, Any]:
    return {
        "receipt_id": stable_uuid_v2("receipt", packet.get("corridor_id"), step.get("step_id"), summary),
        "corridor_id": packet.get("corridor_id"),
        "lease_id": step.get("lease_id"),
        "step_id": step.get("step_id"),
        "action": step.get("action"),
        "issued_by": "agency_corridor_v2",
        "issued_at": iso_now(),
        "bounded_summary": bounded_text(summary, limit=700),
        "evidence_refs": evidence_refs,
        "hash_refs": [sha256_text(json.dumps({"summary": summary, "evidence_refs": evidence_refs}, sort_keys=True, ensure_ascii=False))],
        "source_prep_proposal_ref": step.get("source_prep_proposal_id"),
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
        "right_to_ignore": True,
    }


def write_source_prep_proposal(state_dir: Path, packet: dict[str, Any], step: dict[str, Any], *, write: bool) -> tuple[dict[str, Any], dict[str, Any]]:
    proposal = packet.get("source_prep_proposal") if isinstance(packet.get("source_prep_proposal"), dict) else source_prep_proposal_for_packet(packet)
    if not proposal:
        proposal = {
            "schema": "agency_corridor_source_prep_proposal_v1",
            "schema_version": SCHEMA_VERSION_V2,
            "proposal_id": stable_uuid_v2("source_prep", packet.get("corridor_id"), step.get("step_id")),
            "corridor_id": packet.get("corridor_id"),
            "surface": "agency_corridor",
            "bounded_plan": "Prepare bounded evidence and tests only; do not edit source in this step.",
            "files": [],
            "tests_to_run": ["python3 scripts/agency_corridor.py --self-test"],
            "restart_required": False,
            "edits_source_now": False,
            "grants_approval": False,
            "live_eligible_now": False,
            "auto_approved": False,
            "right_to_ignore": True,
        }
    path = state_dir / SOURCE_PREP_DIR / f"{proposal['proposal_id']}.json"
    md_path = path.with_suffix(".md")
    if write:
        atomic_write_text(path, json.dumps(proposal, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
        atomic_write_text(
            md_path,
            "\n".join(
                [
                    "# Agency Corridor Source Prep Proposal V1",
                    "",
                    f"- proposal_id: {proposal.get('proposal_id')}",
                    f"- corridor_id: {proposal.get('corridor_id')}",
                    f"- surface: {proposal.get('surface')}",
                    "- edits_source_now: false",
                    "- grants_approval: false",
                    "- live_eligible_now: false",
                    "- auto_approved: false",
                    "",
                    "## Bounded Plan",
                    bounded_text(proposal.get("bounded_plan"), limit=900),
                    "",
                    "## Tests To Run",
                    *[f"- {test}" for test in proposal.get("tests_to_run", [])],
                    "",
                    "## Boundary",
                    NON_LIVE_BOUNDARY,
                ]
            ).rstrip()
            + "\n",
        )
    receipt = receipt_v2_for_step(packet, step, "source-prep proposal artifact recorded without editing source", [str(path), str(md_path)])
    return proposal, receipt


def write_artifact_comparison(state_dir: Path, packet: dict[str, Any], step: dict[str, Any], *, write: bool) -> tuple[dict[str, Any], dict[str, Any]]:
    refs = [str(ref) for ref in packet.get("evidence_refs", []) if ref]
    comparison = {
        "schema": "agency_corridor_artifact_comparison_v1",
        "schema_version": SCHEMA_VERSION_V2,
        "comparison_id": stable_uuid_v2("artifact_comparison", packet.get("corridor_id"), step.get("step_id")),
        "corridor_id": packet.get("corridor_id"),
        "input_refs": refs,
        "bounded_summary": "Compared bounded evidence references for continuity and non-live next steps; no runtime state changed.",
        "unknowns": ["content-level comparison remains bounded to existing artifact metadata unless a safe lab reads referenced artifacts"],
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
    }
    path = state_dir / ARTIFACT_COMPARISON_DIR / f"{comparison['comparison_id']}.json"
    if write:
        atomic_write_text(path, json.dumps(comparison, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
    receipt = receipt_v2_for_step(packet, step, "artifact comparison recorded as non-live evidence", [str(path), *refs])
    return comparison, receipt


def program_receipt(
    program: dict[str, Any],
    *,
    kind: str,
    summary: str,
    evidence_refs: list[str],
    portfolio_id: str | None = None,
    patch_bundle_id: str | None = None,
) -> dict[str, Any]:
    return {
        "schema": "agency_program_receipt_v1",
        "schema_version": 1,
        "receipt_id": stable_uuid_v2("program_receipt", program.get("program_id"), kind, summary, now_s()),
        "program_id": program.get("program_id"),
        "kind": kind,
        "issued_by": "agency_corridor_v2",
        "issued_at": iso_now(),
        "bounded_summary": bounded_text(summary, limit=700),
        "evidence_refs": evidence_refs,
        "hash_refs": [sha256_text(json.dumps({"kind": kind, "summary": summary, "evidence_refs": evidence_refs}, sort_keys=True, ensure_ascii=False))],
        "portfolio_id": portfolio_id,
        "patch_bundle_id": patch_bundle_id,
        "right_to_ignore": True,
        "edits_source_now": False,
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
    }


def patch_bundle_for_program(
    state_dir: Path,
    program: dict[str, Any],
    portfolio: dict[str, Any] | None,
    *,
    surface: str | None = None,
) -> tuple[dict[str, Any], str]:
    program_id = str(program.get("program_id") or "")
    bundle_id = stable_uuid_v2("patch_bundle", program_id, surface or program.get("title") or "")
    chosen_surface = bounded_text(surface or program.get("title") or "agency_corridor", limit=120)
    candidate_files = [
        "scripts/agency_corridor.py",
        "scripts/recent_signal_summary.py",
        "scripts/proactive_scan.py",
        "scripts/sandbox_trial_queue.py",
        "scripts/introspection_addressing_audit.py",
    ]
    if "bridge" in chosen_surface or "spectral" in chosen_surface:
        candidate_files.append("capsules/spectral-bridge/src/autonomous/next_action/operations.rs")
        candidate_files.append("capsules/spectral-bridge/src/llm.rs")
    tests_to_run = [
        "python3 scripts/agency_corridor.py --self-test",
        "python3 scripts/recent_signal_summary.py --self-test",
        "python3 scripts/proactive_scan.py --self-test",
        "python3 scripts/sandbox_trial_queue.py --self-test",
        "python3 scripts/introspection_addressing_audit.py --self-test",
    ]
    restart_expected = any(path.startswith("capsules/spectral-bridge/") for path in candidate_files)
    diff_path = state_dir / PATCH_BUNDLE_DIR / f"{bundle_id}.diff"
    manifest = bounded_text(
        f"Review-only patch bundle for {program.get('title')}: {program.get('hypothesis')}",
        limit=700,
    )
    diff_text = "\n".join(
        [
            f"diff --git a/docs/steward-notes/agency_program_{bundle_id}.md b/docs/steward-notes/agency_program_{bundle_id}.md",
            "new file mode 100644",
            "index 0000000..0000000",
            "--- /dev/null",
            f"+++ b/docs/steward-notes/agency_program_{bundle_id}.md",
            "@@ -0,0 +1,18 @@",
            "+# Quarantined Patch Bundle",
            "+",
            f"+- bundle_id: {bundle_id}",
            f"+- program_id: {program_id}",
            f"+- surface: {chosen_surface}",
            "+- edits_source_now: false",
            "+- grants_approval: false",
            "+- live_eligible_now: false",
            "+- auto_approved: false",
            "+",
            "+## Proposed Plan",
            f"+{bounded_text(program.get('hypothesis'), limit=500)}",
            "+",
            "+## Portfolio Recommendation",
            f"+{bounded_text((portfolio or {}).get('current_recommendation'), limit=500)}",
            "+",
            "+## Review",
            "+This unified diff is quarantined under diagnostics and is not applied by the corridor.",
            "",
        ]
    )
    bundle = {
        "schema": "quarantined_patch_bundle_v1",
        "schema_version": 1,
        "bundle_id": bundle_id,
        "program_id": program_id,
        "surface": chosen_surface,
        "manifest": manifest,
        "proposed_diff_artifact_path": str(diff_path),
        "files_touched": unique_strings(candidate_files),
        "tests_to_run": tests_to_run,
        "restart_expected": restart_expected,
        "restart_debt_note": (
            "later implementation would require bridge restart through scripts/build_bridge.sh"
            if restart_expected
            else "no restart expected unless later implementation touches live-consumed surfaces"
        ),
        "edits_source_now": False,
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
        "right_to_ignore": True,
    }
    return bundle, diff_text


def render_patch_bundle_markdown(bundle: dict[str, Any]) -> str:
    lines = [
        "# Quarantined Patch Bundle V1",
        "",
        f"- bundle_id: {bundle.get('bundle_id')}",
        f"- program_id: {bundle.get('program_id')}",
        f"- surface: {bundle.get('surface')}",
        "- edits_source_now: false",
        "- grants_approval: false",
        "- live_eligible_now: false",
        "- auto_approved: false",
        f"- restart_expected: {str(bool(bundle.get('restart_expected'))).lower()}",
        "",
        "## Manifest",
        bounded_text(bundle.get("manifest"), limit=900),
        "",
        "## Files A Later Implementation Might Touch",
        *[f"- {path}" for path in bundle.get("files_touched", [])],
        "",
        "## Tests To Run Later",
        *[f"- {test}" for test in bundle.get("tests_to_run", [])],
        "",
        "## Boundary",
        NON_LIVE_BOUNDARY,
    ]
    return "\n".join(lines).rstrip() + "\n"


def write_quarantined_patch_bundle(
    state_dir: Path,
    program: dict[str, Any],
    portfolio: dict[str, Any] | None,
    *,
    surface: str | None,
    write: bool,
) -> tuple[dict[str, Any], dict[str, Any]]:
    bundle, diff_text = patch_bundle_for_program(state_dir, program, portfolio, surface=surface)
    json_path = state_dir / PATCH_BUNDLE_DIR / f"{bundle['bundle_id']}.json"
    diff_path = Path(bundle["proposed_diff_artifact_path"])
    md_path = json_path.with_suffix(".md")
    if write:
        atomic_write_text(json_path, json.dumps({"schema": "quarantined_patch_bundle_v1", "bundle": bundle}, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
        atomic_write_text(diff_path, diff_text)
        atomic_write_text(md_path, render_patch_bundle_markdown(bundle))
    receipt = program_receipt(
        program,
        kind="patch_bundle_prepared",
        summary="quarantined patch bundle prepared under diagnostics without editing source",
        evidence_refs=[str(json_path), str(diff_path), str(md_path)],
        portfolio_id=(portfolio or {}).get("portfolio_id"),
        patch_bundle_id=bundle["bundle_id"],
    )
    return bundle, receipt


def program_steps_from_payload(payload: dict[str, Any], limit: int) -> list[dict[str, Any]]:
    if int((payload.get("summary") or {}).get("live_violation_count") or 0) > 0:
        return []
    programs = [program for program in payload.get("programs", {}).values() if isinstance(program, dict)]
    programs.sort(key=lambda program: (-int((program.get("priority_signal") or {}).get("deterministic_score") or 0), str(program.get("program_id") or "")))
    steps: list[dict[str, Any]] = []
    existing_bundles = payload.get("patch_bundles", {}) if isinstance(payload.get("patch_bundles"), dict) else {}
    existing_receipts = payload.get("program_receipts", {}) if isinstance(payload.get("program_receipts"), dict) else {}
    bundle_program_ids = {
        str(bundle.get("program_id") or "")
        for bundle in existing_bundles.values()
        if isinstance(bundle, dict)
    }
    receipt_pairs = {
        (str(receipt.get("program_id") or ""), str(receipt.get("kind") or ""))
        for receipt in existing_receipts.values()
        if isinstance(receipt, dict)
    }
    for program in programs:
        if len(steps) >= limit:
            break
        if program.get("status") not in {"active", "waiting_for_evidence"}:
            continue
        action = str(program.get("current_next_action") or PROGRAM_STEP_UPDATE_PORTFOLIO)
        if action == PROGRAM_STEP_RECORD_OBJECTION:
            runnable_action = PROGRAM_STEP_UPDATE_PORTFOLIO
        elif action == PROGRAM_STEP_PREPARE_PATCH_BUNDLE and str(program.get("program_id") or "") in bundle_program_ids:
            continue
        else:
            runnable_action = action
        receipt_kind = "portfolio_updated"
        if runnable_action == PROGRAM_STEP_PREPARE_PATCH_BUNDLE:
            receipt_kind = "patch_bundle_prepared"
        if (str(program.get("program_id") or ""), receipt_kind) in receipt_pairs:
            continue
        steps.append(
            {
                "schema": "agency_program_step_v1",
                "schema_version": 1,
                "step_id": stable_uuid_v2("program_step", program.get("program_id"), runnable_action),
                "program_id": program.get("program_id"),
                "action": runnable_action,
                "priority_score": int((program.get("priority_signal") or {}).get("deterministic_score") or 0),
                "runnable": True,
                "reason": bounded_text(program.get("hypothesis"), limit=500),
                "edits_source_now": False,
                "grants_approval": False,
                "live_eligible_now": False,
                "auto_approved": False,
            }
        )
    return steps


def run_program_step_v2(
    state_dir: Path,
    payload: dict[str, Any],
    step: dict[str, Any],
    *,
    write: bool,
) -> dict[str, Any]:
    program = payload.get("programs", {}).get(str(step.get("program_id") or ""))
    if not isinstance(program, dict):
        raise SystemExit("program step references missing program")
    portfolio = next(
        (
            item for item in payload.get("portfolios", {}).values()
            if isinstance(item, dict) and str(item.get("program_id") or "") == str(program.get("program_id") or "")
        ),
        None,
    )
    artifact: dict[str, Any]
    if step.get("action") == PROGRAM_STEP_PREPARE_PATCH_BUNDLE:
        artifact, receipt = write_quarantined_patch_bundle(
            state_dir,
            program,
            portfolio,
            surface=program.get("title"),
            write=write,
        )
        event_type = "patch_bundle_prepared"
        extra = {"bundle_id": artifact.get("bundle_id"), "bundle": artifact}
    else:
        if portfolio is None:
            raise SystemExit("program step could not find evidence portfolio")
        path = state_dir / PORTFOLIO_DIR / f"{portfolio['portfolio_id']}.json"
        md_path = path.with_suffix(".md")
        if write:
            atomic_write_text(path, json.dumps({"schema": "evidence_portfolio_v1", "portfolio": portfolio}, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
            atomic_write_text(md_path, render_portfolio_markdown(portfolio))
        artifact = portfolio
        receipt = program_receipt(
            program,
            kind="portfolio_updated",
            summary="evidence portfolio refreshed as durable non-live program memory",
            evidence_refs=[str(path), str(md_path)],
            portfolio_id=portfolio.get("portfolio_id"),
        )
        event_type = "portfolio_updated"
        extra = {"portfolio_id": portfolio.get("portfolio_id"), "portfolio": portfolio}
    if write:
        receipt_path = state_dir / PROGRAM_RECEIPT_DIR / f"{receipt['receipt_id']}.json"
        atomic_write_text(receipt_path, json.dumps(receipt, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
        append_events(
            state_dir,
            [
                {
                    "event_type": f"program_{event_type}",
                    "schema": "agency_work_program_v1",
                    "ts": now_s(),
                    "program_id": program.get("program_id"),
                    "receipt": receipt,
                    **extra,
                },
                {
                    "event_type": "program_receipt_recorded",
                    "schema": "agency_work_program_v1",
                    "ts": now_s(),
                    "program_id": program.get("program_id"),
                    "receipt": receipt,
                },
            ],
        )
    return {"step": step, "program": program, "artifact": artifact, "receipt": receipt}


def run_next_v2(state_dir: Path, *, limit: int, write: bool, emit_cards: bool = True) -> dict[str, Any]:
    state_dir = resolve_v2_state_dir(state_dir)
    status = generate_v2_status(state_dir, write=write)
    program_payload = generate_programs_v2(state_dir, write=write)
    summary = status.get("summary", {})
    program_summary = program_payload.get("summary", {})
    live_violation_count = int(summary.get("live_violation_count") or 0) + int(program_summary.get("live_violation_count") or 0)
    if live_violation_count > 0:
        return {
            "schema": SCHEMA_V2,
            "ran": 0,
            "limit": max(1, min(limit, 5)),
            "blocked": True,
            "block_reason": "live/approval hard violation present in corridor artifacts",
            "live_violation_refs": list(summary.get("live_violation_refs", [])) + list(program_summary.get("live_violation_refs", [])),
            "boundary": NON_LIVE_BOUNDARY,
        }
    run_limit = max(1, min(limit, 5))
    program_steps = program_steps_from_payload(program_payload, run_limit)
    program_results: list[dict[str, Any]] = []
    for step in program_steps:
        if len(program_results) >= run_limit:
            break
        program_results.append(run_program_step_v2(state_dir, program_payload, step, write=write))
    remaining_limit = max(0, run_limit - len(program_results))
    steps = [
        step
        for step in status.get("queue", {}).get("steps", [])
        if isinstance(step, dict) and step.get("runnable") is True
    ][:remaining_limit]
    events: list[dict[str, Any]] = []
    results: list[dict[str, Any]] = []
    for step in steps:
        packet = status.get("packets", {}).get(str(step.get("corridor_id") or ""))
        if not isinstance(packet, dict):
            continue
        action = str(step.get("action") or "")
        artifact: dict[str, Any] | None = None
        card: dict[str, Any] | None = None
        if action == "run_safe_lab":
            result = safe_lab_result_for(packet)
            result["schema"] = "agency_corridor_safe_lab_result_v2"
            result["schema_version"] = SCHEMA_VERSION_V2
            result_base = state_dir / RESULT_DIR / f"{int(now_s())}_{packet['corridor_id']}"
            result_json_path = result_base.with_suffix(".json")
            result_md_path = result_base.with_suffix(".md")
            receipt = receipt_v2_for_step(packet, step, "V2 safe lab receipt recorded without live authority", [str(result_json_path), *result.get("evidence_refs", [])])
            if write:
                atomic_write_text(result_json_path, json.dumps(result, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
                atomic_write_text(result_md_path, result_markdown(packet, result, receipt))
            artifact = result
        elif action == "compare_artifacts":
            artifact, receipt = write_artifact_comparison(state_dir, packet, step, write=write)
        elif step.get("source_prep_proposal_id") or action == "generate_replay_candidate":
            artifact, receipt = write_source_prep_proposal(state_dir, packet, step, write=write)
            events.append(
                {
                    "event_type": "source_prep_proposal_recorded",
                    "schema": SCHEMA_V2,
                    "ts": now_s(),
                    "corridor_id": packet.get("corridor_id"),
                    "proposal": artifact,
                }
            )
        else:
            card = emit_card_for_packet_v2(state_dir, packet, step=step, write=write, deliver_safe=emit_cards)
            artifact = card
            receipt = receipt_v2_for_step(packet, step, "V2 corridor card/request emitted without live authority", [str(card.get("path"))])
        if card is None:
            card = emit_card_for_packet_v2(state_dir, packet, step=step, write=write, deliver_safe=emit_cards)
        events.append(
            {
                "event_type": "corridor_receipt_recorded_v2",
                "schema": SCHEMA_V2,
                "ts": now_s(),
                "corridor_id": packet["corridor_id"],
                "receipt": receipt,
            }
        )
        results.append({"step": step, "packet": packet, "artifact": artifact, "receipt": receipt, "card": card})
    if write:
        append_events(state_dir, events)
        generate_programs_v2(state_dir, write=True)
        materialize_v2(state_dir, generate_v2_status(state_dir, write=False))
    return {
        "schema": SCHEMA_V2,
        "ran": len(program_results) + len(results),
        "program_ran": len(program_results),
        "corridor_ran": len(results),
        "limit": run_limit,
        "program_results": program_results,
        "results": results,
        "live_violation_count": live_violation_count,
        "boundary": NON_LIVE_BOUNDARY,
    }


def prepare_source_proposal_v2(state_dir: Path, *, packet_id: str | None, surface: str | None, write: bool) -> dict[str, Any]:
    state_dir = resolve_v2_state_dir(state_dir)
    status = generate_v2_status(state_dir, write=write)
    packets = status.get("packets", {})
    selected = packets.get(packet_id or "") if packet_id else None
    if not isinstance(selected, dict):
        selected = next(
            (
                packet
                for packet in packets.values()
                if isinstance(packet, dict)
                and isinstance(packet.get("source_prep_proposal"), dict)
                and (not surface or packet["source_prep_proposal"].get("surface") == surface)
            ),
            None,
        )
    if not isinstance(selected, dict):
        raise SystemExit("prepare-source-proposal found no matching V2 packet")
    step = {
        "step_id": stable_uuid_v2("manual_source_prep", selected.get("corridor_id"), surface or ""),
        "action": "generate_replay_candidate",
        "corridor_id": selected.get("corridor_id"),
        "lease_id": "lease_corridor_canary_source_prep_v1",
        "source_prep_proposal_id": (selected.get("source_prep_proposal") or {}).get("proposal_id"),
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
    }
    proposal, receipt = write_source_prep_proposal(state_dir, selected, step, write=write)
    if write:
        append_events(
            state_dir,
            [
                {
                    "event_type": "source_prep_proposal_recorded",
                    "schema": SCHEMA_V2,
                    "ts": now_s(),
                    "corridor_id": selected.get("corridor_id"),
                    "proposal": proposal,
                },
                {
                    "event_type": "corridor_receipt_recorded_v2",
                    "schema": SCHEMA_V2,
                    "ts": now_s(),
                    "corridor_id": selected.get("corridor_id"),
                    "receipt": receipt,
                },
            ],
        )
        materialize_v2(state_dir, generate_v2_status(state_dir, write=False))
    return {"schema": SCHEMA_V2, "proposal": proposal, "receipt": receipt, "boundary": NON_LIVE_BOUNDARY}


def prepare_patch_bundle_v2(
    state_dir: Path,
    *,
    program_id: str | None,
    surface: str | None,
    write: bool,
) -> dict[str, Any]:
    state_dir = resolve_v2_state_dir(state_dir)
    payload = generate_programs_v2(state_dir, write=write)
    programs = payload.get("programs", {})
    selected = programs.get(program_id or "") if program_id else None
    if not isinstance(selected, dict):
        selected = next(
            (
                program
                for program in programs.values()
                if isinstance(program, dict)
                and (not surface or surface in str(program.get("title") or ""))
            ),
            None,
        )
    if not isinstance(selected, dict):
        raise SystemExit("prepare-patch-bundle found no matching work program")
    portfolio = next(
        (
            item for item in payload.get("portfolios", {}).values()
            if isinstance(item, dict) and item.get("program_id") == selected.get("program_id")
        ),
        None,
    )
    bundle, receipt = write_quarantined_patch_bundle(
        state_dir,
        selected,
        portfolio,
        surface=surface or selected.get("title"),
        write=write,
    )
    if write:
        append_events(
            state_dir,
            [
                {
                    "event_type": "program_patch_bundle_prepared",
                    "schema": "agency_work_program_v1",
                    "ts": now_s(),
                    "program_id": selected.get("program_id"),
                    "bundle_id": bundle.get("bundle_id"),
                    "bundle": bundle,
                    "receipt": receipt,
                },
                {
                    "event_type": "program_receipt_recorded",
                    "schema": "agency_work_program_v1",
                    "ts": now_s(),
                    "program_id": selected.get("program_id"),
                    "receipt": receipt,
                },
            ],
        )
        generate_programs_v2(state_dir, write=True)
    return {"schema": "quarantined_patch_bundle_v1", "bundle": bundle, "receipt": receipt, "boundary": NON_LIVE_BOUNDARY}


def record_program_note_v2(
    state_dir: Path,
    *,
    program_id: str,
    note: str,
    source: str,
    write: bool,
) -> dict[str, Any]:
    state_dir = resolve_v2_state_dir(state_dir)
    note_id = stable_uuid_v2("program_note", program_id, source, note)
    event = {
        "event_type": "program_note_recorded",
        "schema": "agency_work_program_v1",
        "ts": now_s(),
        "program_id": program_id,
        "note_id": note_id,
        "source": source,
        "bounded_summary": bounded_text(note, limit=700),
        "note_hash": sha256_text(note),
        "edits_source_now": False,
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
    }
    if write:
        append_events(state_dir, [event])
        generate_programs_v2(state_dir, write=True)
    return {"schema": "agency_program_note_v1", "event": event, "boundary": NON_LIVE_BOUNDARY}


def record_program_objection_v2(
    state_dir: Path,
    *,
    program_id: str,
    being: str,
    summary: str,
    evidence_refs: list[str],
    write: bool,
) -> dict[str, Any]:
    state_dir = resolve_v2_state_dir(state_dir)
    objection_id = stable_uuid_v2("program_objection", program_id, being, summary)
    event = {
        "event_type": "program_objection_recorded",
        "schema": "agency_work_program_v1",
        "ts": now_s(),
        "program_id": program_id,
        "objection_id": objection_id,
        "being": being,
        "bounded_summary": bounded_text(summary, limit=700),
        "evidence_refs": evidence_refs,
        "objection_hash": sha256_text(summary),
        "edits_source_now": False,
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
    }
    if write:
        append_events(state_dir, [event])
        generate_programs_v2(state_dir, write=True)
    return {"schema": "agency_program_objection_v1", "event": event, "boundary": NON_LIVE_BOUNDARY}


def record_lease_revocation(state_dir: Path, *, lease_id: str, reason: str, write: bool) -> dict[str, Any]:
    state_dir = resolve_v2_state_dir(state_dir)
    event = {
        "event_type": "lease_revoked",
        "schema": SCHEMA_V2,
        "ts": now_s(),
        "lease_id": lease_id,
        "reason": bounded_text(reason, limit=500),
        "grants_approval": False,
        "live_eligible_now": False,
        "auto_approved": False,
    }
    if write:
        append_events(state_dir, [event])
        materialize_v2(state_dir, generate_v2_status(state_dir, write=False))
    return {"schema": SCHEMA_V2, "event": event, "boundary": NON_LIVE_BOUNDARY}


def print_payload(payload: dict[str, Any], *, as_json: bool) -> None:
    normalize_artifact_authority_tree(payload)
    if as_json:
        print(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False))
    else:
        print(render_report_markdown(payload.get("status", payload)), end="")


class AgencyCorridorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = Path(tempfile.mkdtemp(prefix="agency_corridor_test_"))
        self.addCleanup(lambda: shutil.rmtree(self.tmp, ignore_errors=True))

    def test_record_objection_auto_reopens_without_live_authority(self) -> None:
        payload = record_objection(
            self.tmp,
            being="astrid",
            closure_ref="closure-card-1",
            summary="The closure still misses the jagged resistance.",
            evidence_refs=["introspection_x"],
            write=True,
        )
        status = load_status(self.tmp)
        self.assertEqual(status["summary"]["reopened_work_item_count"], 1)
        self.assertFalse(payload["packet"]["live_eligible_now"])
        self.assertFalse(payload["packet"]["auto_approved"])
        self.assertFalse(payload["packet"]["grants_approval"])

    def test_run_next_limits_to_three_safe_labs(self) -> None:
        events = []
        for idx in range(5):
            packet = {
                "schema": SCHEMA,
                "schema_version": 1,
                "corridor_id": stable_uuid("safe", idx),
                "source": "test",
                "being": "astrid",
                "action": "run_safe_lab",
                "state": "safe_lab_ready",
                "work_item_ids": [f"wi_{idx}"],
                "closure_card_refs": [],
                "sandbox_trial_ids": [],
                "delta_refs": [],
                "felt_report_anchor": "bounded anchor",
                "proposed_corridor_action": "run safe lab",
                "evidence_refs": [f"wi_{idx}"],
                "safe_lab_candidate": {
                    "lab_id": f"lab_{idx}",
                    "adapter": "read_only_review",
                    "run_query": "noop",
                    "mode": "read_only_review",
                    "runnable": True,
                    "authority": "read_only",
                },
                "closure_objection": None,
                "closure_reopen_ref": None,
                "self_observation_request": None,
                "canary_criteria": None,
                "receipts": [],
                "who_can_escalate": "steward",
                "how_to_test_it": "review",
                "right_to_ignore": True,
                "grants_approval": False,
                "live_eligible_now": False,
                "auto_approved": False,
                "boundary": NON_LIVE_BOUNDARY,
            }
            events.append(packet_event(packet))
        append_events(self.tmp, events)
        materialize(self.tmp, apply_events({}, read_events(self.tmp)))

        payload = run_next(self.tmp, limit=5, write=True, emit_cards=False)

        self.assertEqual(payload["ran"], 3)
        status = load_status(self.tmp)
        self.assertEqual(status["summary"]["live_eligible_now_count"], 0)
        self.assertEqual(status["summary"]["auto_approved_count"], 0)

    def test_card_delivery_skips_canary_proposals(self) -> None:
        safe = packet_for_work_item(
            {
                "work_item_id": "wi_safe",
                "status": "implemented_awaiting_felt_response",
                "agency_tier": 2,
                "being": "astrid",
                "claim_summary": "safe self-observation",
            }
        )
        canary = packet_for_work_item(
            {
                "work_item_id": "wi_live",
                "status": "needs_operator_approval",
                "agency_tier": 5,
                "being": "astrid",
                "claim_summary": "live canary proposal",
            }
        )
        safe_card = emit_card_for_packet(self.tmp, safe, write=True)
        canary_card = emit_card_for_packet(self.tmp, canary, write=True)

        self.assertIsNotNone(safe_card["delivered_path"])
        self.assertIsNone(canary_card["delivered_path"])

    def test_self_observation_response_receipt_is_non_live(self) -> None:
        payload = record_self_observation_response(
            self.tmp,
            request_id="req-1",
            status="still_friction",
            source="astrid",
            note="still feels mismatched",
            write=True,
        )
        receipt = payload["receipt"]
        self.assertFalse(receipt["grants_approval"])
        self.assertFalse(receipt["live_eligible_now"])

    def write_v1_source_status(self, packets: list[dict[str, Any]]) -> None:
        status = apply_events({str(packet["corridor_id"]): packet for packet in packets}, [])
        atomic_write_text(self.tmp / V1_SOURCE_STATUS_FILE, json.dumps(status, indent=2, sort_keys=True) + "\n")

    def safe_v1_packet(self, idx: int, *, action: str = "run_safe_lab", state: str = "safe_lab_ready") -> dict[str, Any]:
        return {
            "schema": SCHEMA,
            "schema_version": 1,
            "corridor_id": stable_uuid("v2-safe", idx, action),
            "source": "test",
            "being": "astrid",
            "action": action,
            "state": state,
            "authority_boundary_id": None,
            "work_item_ids": [f"wi_{idx}"],
            "closure_card_refs": [],
            "sandbox_trial_ids": [],
            "delta_refs": [],
            "felt_report_anchor": "bounded anchor",
            "proposed_corridor_action": "run safe non-live action",
            "evidence_refs": [f"wi_{idx}"],
            "safe_lab_candidate": {
                "lab_id": f"lab_{idx}",
                "adapter": "read_only_review",
                "run_query": "noop",
                "mode": "read_only_review",
                "runnable": True,
                "authority": "read_only",
            }
            if action == "run_safe_lab"
            else None,
            "closure_objection": None,
            "closure_reopen_ref": None,
            "self_observation_request": None,
            "canary_criteria": None,
            "receipts": [],
            "who_can_escalate": "steward",
            "how_to_test_it": "review",
            "right_to_ignore": True,
            "grants_approval": False,
            "live_eligible_now": False,
            "auto_approved": False,
            "boundary": NON_LIVE_BOUNDARY,
        }

    def test_v2_lease_registry_has_active_non_live_leases(self) -> None:
        payload = generate_leases_v2(self.tmp, write=True)
        summary = payload["summary"]

        self.assertGreaterEqual(summary["active_count"], 4)
        self.assertEqual(summary["live_violation_count"], 0)
        for lease in payload["leases"].values():
            self.assertFalse(lease["grants_approval"])
            self.assertFalse(lease["live_eligible_now"])
            self.assertFalse(lease["auto_approved"])

    def test_v2_adaptive_queue_orders_reopens_before_safe_labs_and_source_prep(self) -> None:
        reopen = self.safe_v1_packet(0, action="reopen_insufficient_closure", state="closure_reopened")
        safe = self.safe_v1_packet(1)
        canary = self.safe_v1_packet(2, action="propose_canary_criteria", state="canary_criteria_proposed")
        canary["canary_criteria"] = {"proposal_id": "canary", "surface": "spectral_bridge"}
        self.write_v1_source_status([safe, canary, reopen])

        status = generate_v2_status(self.tmp, write=True)
        priorities = [step["priority"] for step in status["queue"]["steps"]]

        self.assertEqual(priorities, sorted(priorities))
        self.assertEqual(status["queue"]["max_steps_per_run"], 5)
        self.assertEqual(status["summary"]["live_violation_count"], 0)
        self.assertTrue(any(step.get("source_prep_proposal_id") for step in status["queue"]["steps"]))

    def test_v2_adaptive_queue_does_not_rerun_receipted_steps(self) -> None:
        packet = self.safe_v1_packet(0)
        packet["source_prep_proposal"] = {"proposal_id": "source_prep_0"}
        initial = build_queue_steps({str(packet["corridor_id"]): packet}, [])

        self.assertEqual(len(initial), 2)
        self.assertTrue(all(step["runnable"] for step in initial))

        packet["receipts"] = [
            {"step_id": step["step_id"], "action": step["action"]}
            for step in initial
        ]
        refreshed = build_queue_steps({str(packet["corridor_id"]): packet}, [])

        self.assertFalse(any(step["runnable"] for step in refreshed))

        result_recorded = self.safe_v1_packet(1, state="safe_lab_result_recorded")
        completed = build_queue_steps({str(result_recorded["corridor_id"]): result_recorded}, [])
        self.assertFalse(completed[0]["runnable"])

    def test_v2_run_next_limits_to_five_safe_actions(self) -> None:
        self.write_v1_source_status([self.safe_v1_packet(idx) for idx in range(7)])

        payload = run_next_v2(self.tmp, limit=7, write=True, emit_cards=False)

        self.assertEqual(payload["ran"], 5)
        self.assertEqual(payload["live_violation_count"], 0)
        status = load_v2_status(self.tmp)
        self.assertEqual(status["summary"]["live_violation_count"], 0)

    def test_v2_source_prep_proposal_does_not_edit_source(self) -> None:
        canary = self.safe_v1_packet(1, action="propose_canary_criteria", state="canary_criteria_proposed")
        canary["canary_criteria"] = {"proposal_id": "canary", "surface": "spectral_bridge"}
        self.write_v1_source_status([canary])

        payload = prepare_source_proposal_v2(self.tmp, packet_id=None, surface="spectral_bridge", write=True)
        proposal = payload["proposal"]

        self.assertFalse(proposal["edits_source_now"])
        self.assertFalse(proposal["grants_approval"])
        self.assertFalse(proposal["live_eligible_now"])
        self.assertTrue((self.tmp / SOURCE_PREP_DIR / f"{proposal['proposal_id']}.json").exists())

    def test_v2_lease_revocation_records_without_approval(self) -> None:
        payload = record_lease_revocation(
            self.tmp,
            lease_id="lease_corridor_safe_labs_v1",
            reason="operator paused safe labs",
            write=True,
        )
        leases = generate_leases_v2(self.tmp, write=False)["leases"]

        self.assertFalse(payload["event"]["grants_approval"])
        self.assertEqual(leases["lease_corridor_safe_labs_v1"]["state"], "revoked")
        self.assertIn("operator paused", leases["lease_corridor_safe_labs_v1"]["revocation_reason"])

    def test_v2_live_violation_is_rejected_and_historical_corruption_blocks_runner(self) -> None:
        bad_packet = self.safe_v1_packet(99)
        bad_v2 = v2_packet_for_v1(bad_packet, standing_corridor_leases())
        bad_v2["live_eligible_now"] = True
        bad_event = {
            "event_type": "corridor_packet_declared_v2",
            "schema": SCHEMA_V2,
            "ts": now_s(),
            "corridor_id": bad_v2["corridor_id"],
            "packet": bad_v2,
        }
        with self.assertRaisesRegex(ValueError, "live_eligible_now=true"):
            append_events(self.tmp, [bad_event])

        # A historical/corrupt row that bypassed the writer must still fail closed.
        events_path = self.tmp / EVENTS_FILE
        events_path.parent.mkdir(parents=True, exist_ok=True)
        events_path.write_text(json.dumps(bad_event) + "\n", encoding="utf-8")

        payload = run_next_v2(self.tmp, limit=5, write=False, emit_cards=False)

        self.assertTrue(payload["blocked"])
        self.assertEqual(payload["ran"], 0)
        self.assertTrue(payload["live_violation_refs"])

    def test_program_generation_groups_by_work_item_and_scores_priority(self) -> None:
        packet_a = self.safe_v1_packet(1, action="compare_artifacts", state="evidence_only")
        packet_b = self.safe_v1_packet(2, action="propose_canary_criteria", state="canary_criteria_proposed")
        packet_b["work_item_ids"] = packet_a["work_item_ids"]
        packet_b["canary_criteria"] = {"proposal_id": "canary", "surface": "spectral_bridge"}
        self.write_v1_source_status([packet_a, packet_b])

        payload = generate_programs_v2(self.tmp, write=True)
        programs = list(payload["programs"].values())

        self.assertEqual(len(programs), 1)
        self.assertEqual(payload["summary"]["program_count"], 1)
        self.assertGreater(programs[0]["priority_signal"]["deterministic_score"], 0)
        self.assertFalse(programs[0]["grants_approval"])
        self.assertFalse(programs[0]["live_eligible_now"])

    def test_program_portfolio_persists_as_durable_memory(self) -> None:
        self.write_v1_source_status([self.safe_v1_packet(1)])

        payload = generate_programs_v2(self.tmp, write=True)
        portfolio = next(iter(payload["portfolios"].values()))

        self.assertTrue((self.tmp / PROGRAMS_FILE).exists())
        self.assertTrue((self.tmp / PROGRAMS_MD_FILE).exists())
        self.assertTrue((self.tmp / PORTFOLIO_DIR / f"{portfolio['portfolio_id']}.json").exists())
        self.assertFalse(portfolio["edits_source_now"])
        self.assertIn("unknowns", portfolio)

    def test_prepare_patch_bundle_writes_quarantined_diff_without_source_mutation(self) -> None:
        canary = self.safe_v1_packet(1, action="propose_canary_criteria", state="canary_criteria_proposed")
        canary["canary_criteria"] = {"proposal_id": "canary", "surface": "spectral_bridge"}
        self.write_v1_source_status([canary])
        before_hash = sha256_text(Path(__file__).read_text(encoding="utf-8"))
        program_id = next(iter(generate_programs_v2(self.tmp, write=True)["programs"]))

        payload = prepare_patch_bundle_v2(self.tmp, program_id=program_id, surface=None, write=True)
        after_hash = sha256_text(Path(__file__).read_text(encoding="utf-8"))
        bundle = payload["bundle"]

        self.assertEqual(before_hash, after_hash)
        self.assertTrue((self.tmp / PATCH_BUNDLE_DIR / f"{bundle['bundle_id']}.json").exists())
        self.assertTrue(Path(bundle["proposed_diff_artifact_path"]).exists())
        self.assertFalse(bundle["edits_source_now"])
        self.assertFalse(bundle["grants_approval"])
        self.assertFalse(bundle["live_eligible_now"])

    def test_v2_run_next_prefers_program_steps_and_limits_to_five(self) -> None:
        packets = []
        for idx in range(7):
            packet = self.safe_v1_packet(idx, action="propose_canary_criteria", state="canary_criteria_proposed")
            packet["canary_criteria"] = {"proposal_id": f"canary_{idx}", "surface": f"surface_{idx}"}
            packets.append(packet)
        self.write_v1_source_status(packets)

        payload = run_next_v2(self.tmp, limit=7, write=True, emit_cards=False)

        self.assertEqual(payload["ran"], 5)
        self.assertEqual(payload["program_ran"], 5)
        self.assertEqual(payload["corridor_ran"], 0)
        self.assertEqual(payload["live_violation_count"], 0)
        self.assertGreaterEqual(len(list((self.tmp / PATCH_BUNDLE_DIR).glob("*.diff"))), 5)

    def test_v2_program_steps_skip_already_recorded_artifacts(self) -> None:
        packet = self.safe_v1_packet(1, action="propose_canary_criteria", state="canary_criteria_proposed")
        packet["canary_criteria"] = {"proposal_id": "canary", "surface": "spectral_bridge"}
        self.write_v1_source_status([packet])

        first = run_next_v2(self.tmp, limit=1, write=True, emit_cards=False)
        refreshed = generate_programs_v2(self.tmp, write=False)

        self.assertEqual(first["program_ran"], 1)
        self.assertEqual(program_steps_from_payload(refreshed, 1), [])

        self.write_v1_source_status([self.safe_v1_packet(2)])
        second = run_next_v2(self.tmp, limit=1, write=True, emit_cards=False)
        refreshed_again = generate_programs_v2(self.tmp, write=False)

        self.assertEqual(second["program_ran"], 1)
        self.assertEqual(program_steps_from_payload(refreshed_again, 1), [])

    def test_program_objection_escalates_next_action_without_approval(self) -> None:
        self.write_v1_source_status([self.safe_v1_packet(1)])
        program_id = next(iter(generate_programs_v2(self.tmp, write=True)["programs"]))

        payload = record_program_objection_v2(
            self.tmp,
            program_id=program_id,
            being="astrid",
            summary="The closure still feels too smooth and skips resistance.",
            evidence_refs=["introspection_x"],
            write=True,
        )
        refreshed = generate_programs_v2(self.tmp, write=False)["programs"][program_id]

        self.assertFalse(payload["event"]["grants_approval"])
        self.assertFalse(payload["event"]["live_eligible_now"])
        self.assertEqual(refreshed["current_next_action"], PROGRAM_STEP_RECORD_OBJECTION)


def run_self_tests() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(AgencyCorridorTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state-dir", type=Path, default=DEFAULT_STATE_DIR)
    parser.add_argument("--self-test", action="store_true")
    sub = parser.add_subparsers(dest="cmd")

    generate_p = sub.add_parser("generate")
    generate_p.add_argument("--write", action="store_true")
    generate_p.add_argument("--json", action="store_true")

    report_p = sub.add_parser("report")
    report_p.add_argument("--json", action="store_true")
    report_p.add_argument("--markdown", action="store_true")

    run_p = sub.add_parser("run-next")
    run_p.add_argument("--limit", type=int, default=5)
    run_p.add_argument("--write", action="store_true")
    run_p.add_argument("--json", action="store_true")
    run_p.add_argument("--no-card", action="store_true")
    run_p.add_argument("--v1", action="store_true")

    leases_p = sub.add_parser("leases")
    leases_sub = leases_p.add_subparsers(dest="leases_cmd")
    leases_generate_p = leases_sub.add_parser("generate")
    leases_generate_p.add_argument("--write", action="store_true")
    leases_generate_p.add_argument("--json", action="store_true")
    leases_report_p = leases_sub.add_parser("report")
    leases_report_p.add_argument("--json", action="store_true")
    leases_report_p.add_argument("--markdown", action="store_true")

    queue_p = sub.add_parser("queue")
    queue_sub = queue_p.add_subparsers(dest="queue_cmd")
    queue_generate_p = queue_sub.add_parser("generate")
    queue_generate_p.add_argument("--write", action="store_true")
    queue_generate_p.add_argument("--json", action="store_true")
    queue_report_p = queue_sub.add_parser("report")
    queue_report_p.add_argument("--json", action="store_true")
    queue_report_p.add_argument("--markdown", action="store_true")

    programs_p = sub.add_parser("programs")
    programs_sub = programs_p.add_subparsers(dest="programs_cmd")
    programs_generate_p = programs_sub.add_parser("generate")
    programs_generate_p.add_argument("--write", action="store_true")
    programs_generate_p.add_argument("--json", action="store_true")
    programs_report_p = programs_sub.add_parser("report")
    programs_report_p.add_argument("--json", action="store_true")
    programs_report_p.add_argument("--markdown", action="store_true")

    portfolio_p = sub.add_parser("portfolio")
    portfolio_sub = portfolio_p.add_subparsers(dest="portfolio_cmd")
    portfolio_generate_p = portfolio_sub.add_parser("generate")
    portfolio_generate_p.add_argument("--program-id")
    portfolio_generate_p.add_argument("--write", action="store_true")
    portfolio_generate_p.add_argument("--json", action="store_true")
    portfolio_report_p = portfolio_sub.add_parser("report")
    portfolio_report_p.add_argument("--program-id")
    portfolio_report_p.add_argument("--json", action="store_true")
    portfolio_report_p.add_argument("--markdown", action="store_true")

    card_p = sub.add_parser("emit-card")
    card_p.add_argument("--packet-id", action="append", default=[])
    card_p.add_argument("--next", type=int, default=0)
    card_p.add_argument("--write", action="store_true")
    card_p.add_argument("--json", action="store_true")

    objection_p = sub.add_parser("record-objection")
    objection_p.add_argument("--being", required=True)
    objection_p.add_argument("--closure-ref", required=True)
    objection_p.add_argument("--summary", required=True)
    objection_p.add_argument("--evidence-ref", action="append", default=[])
    objection_p.add_argument("--write", action="store_true")
    objection_p.add_argument("--json", action="store_true")

    response_p = sub.add_parser("record-self-observation-response")
    response_p.add_argument("--request-id", required=True)
    response_p.add_argument("--status", required=True)
    response_p.add_argument("--source", required=True)
    response_p.add_argument("--note", default="")
    response_p.add_argument("--write", action="store_true")
    response_p.add_argument("--json", action="store_true")

    source_p = sub.add_parser("prepare-source-proposal")
    source_p.add_argument("--packet-id")
    source_p.add_argument("--surface")
    source_p.add_argument("--write", action="store_true")
    source_p.add_argument("--json", action="store_true")

    patch_p = sub.add_parser("prepare-patch-bundle")
    patch_p.add_argument("--program-id")
    patch_p.add_argument("--surface")
    patch_p.add_argument("--write", action="store_true")
    patch_p.add_argument("--json", action="store_true")

    note_p = sub.add_parser("record-program-note")
    note_p.add_argument("--program-id", required=True)
    note_p.add_argument("--source", required=True)
    note_p.add_argument("--note", required=True)
    note_p.add_argument("--write", action="store_true")
    note_p.add_argument("--json", action="store_true")

    program_objection_p = sub.add_parser("record-program-objection")
    program_objection_p.add_argument("--program-id", required=True)
    program_objection_p.add_argument("--being", required=True)
    program_objection_p.add_argument("--summary", required=True)
    program_objection_p.add_argument("--evidence-ref", action="append", default=[])
    program_objection_p.add_argument("--write", action="store_true")
    program_objection_p.add_argument("--json", action="store_true")

    revoke_p = sub.add_parser("record-lease-revocation")
    revoke_p.add_argument("--lease-id", required=True)
    revoke_p.add_argument("--reason", required=True)
    revoke_p.add_argument("--write", action="store_true")
    revoke_p.add_argument("--json", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.self_test:
        return run_self_tests()
    if args.cmd is None:
        parser.print_help()
        return 2
    if args.cmd == "generate":
        print_payload(generate(args.state_dir, write=bool(args.write)), as_json=bool(args.json or not args.write))
        return 0
    if args.cmd == "report":
        status = load_status(args.state_dir)
        payload = {"schema": SCHEMA, "status": status, "summary": summarize(status), "boundary": NON_LIVE_BOUNDARY}
        if args.markdown and not args.json:
            print(render_report_markdown(status), end="")
        else:
            print_payload(payload, as_json=bool(args.json or not args.markdown))
        return 0
    if args.cmd == "run-next":
        if args.v1:
            payload = run_next(
                args.state_dir,
                limit=max(1, int(args.limit or 1)),
                write=bool(args.write),
                emit_cards=not bool(args.no_card),
            )
        else:
            payload = run_next_v2(
                args.state_dir,
                limit=max(1, int(args.limit or 1)),
                write=bool(args.write),
                emit_cards=not bool(args.no_card),
            )
        print_payload(payload, as_json=bool(args.json or not args.write))
        return 0
    if args.cmd == "leases":
        if args.leases_cmd == "generate":
            payload = generate_leases_v2(args.state_dir, write=bool(args.write))
            print_payload(payload, as_json=bool(args.json or not args.write))
            return 0
        if args.leases_cmd == "report":
            payload = generate_leases_v2(args.state_dir, write=False)
            if args.markdown and not args.json:
                lines = [
                    "# Agency Corridor V2 Lease Report",
                    "",
                    f"- summary: {payload.get('summary', {})}",
                    "",
                    "## Boundary",
                    NON_LIVE_BOUNDARY,
                ]
                print("\n".join(lines).rstrip() + "\n", end="")
            else:
                print_payload(payload, as_json=bool(args.json or not args.markdown))
            return 0
        parser.error("leases requires generate or report")
    if args.cmd == "queue":
        if args.queue_cmd == "generate":
            status = generate_v2_status(args.state_dir, write=bool(args.write))
            payload = {"schema": SCHEMA_V2, "status": status, "summary": status["summary"], "queue": status["queue"], "boundary": NON_LIVE_BOUNDARY}
            print_payload(payload, as_json=bool(args.json or not args.write))
            return 0
        if args.queue_cmd == "report":
            status = load_v2_status(args.state_dir)
            payload = {"schema": SCHEMA_V2, "status": status, "summary": summarize_v2(status), "queue": status.get("queue", {}), "boundary": NON_LIVE_BOUNDARY}
            if args.markdown and not args.json:
                print(render_v2_report_markdown(status), end="")
            else:
                print_payload(payload, as_json=bool(args.json or not args.markdown))
            return 0
        parser.error("queue requires generate or report")
    if args.cmd == "programs":
        if args.programs_cmd == "generate":
            payload = generate_programs_v2(args.state_dir, write=bool(args.write))
            print_payload(payload, as_json=bool(args.json or not args.write))
            return 0
        if args.programs_cmd == "report":
            payload = generate_programs_v2(args.state_dir, write=False)
            if args.markdown and not args.json:
                print(render_programs_markdown(payload), end="")
            else:
                print_payload(payload, as_json=bool(args.json or not args.markdown))
            return 0
        parser.error("programs requires generate or report")
    if args.cmd == "portfolio":
        if args.portfolio_cmd not in {"generate", "report"}:
            parser.error("portfolio requires generate or report")
        payload = generate_programs_v2(args.state_dir, write=bool(getattr(args, "write", False)))
        portfolios = payload.get("portfolios", {})
        selected = next(
            (
                portfolio
                for portfolio in portfolios.values()
                if isinstance(portfolio, dict)
                and (not args.program_id or portfolio.get("program_id") == args.program_id)
            ),
            None,
        )
        if not isinstance(selected, dict):
            raise SystemExit("portfolio command found no matching portfolio")
        out = {"schema": "evidence_portfolio_v1", "portfolio": selected, "boundary": NON_LIVE_BOUNDARY}
        if args.portfolio_cmd == "report" and args.markdown and not args.json:
            print(render_portfolio_markdown(selected), end="")
        else:
            print_payload(out, as_json=bool(args.json or args.portfolio_cmd == "generate" or not getattr(args, "markdown", False)))
        return 0
    if args.cmd == "emit-card":
        payload = emit_card(args.state_dir, packet_ids=args.packet_id, next_count=int(args.next or 0), write=bool(args.write))
        print_payload(payload, as_json=bool(args.json or not args.write))
        return 0
    if args.cmd == "record-objection":
        payload = record_objection(
            args.state_dir,
            being=args.being,
            closure_ref=args.closure_ref,
            summary=args.summary,
            evidence_refs=list(args.evidence_ref or []),
            write=bool(args.write),
        )
        print_payload(payload, as_json=bool(args.json or not args.write))
        return 0
    if args.cmd == "record-self-observation-response":
        payload = record_self_observation_response(
            args.state_dir,
            request_id=args.request_id,
            status=args.status,
            source=args.source,
            note=args.note,
            write=bool(args.write),
        )
        print_payload(payload, as_json=bool(args.json or not args.write))
        return 0
    if args.cmd == "prepare-source-proposal":
        payload = prepare_source_proposal_v2(
            args.state_dir,
            packet_id=args.packet_id,
            surface=args.surface,
            write=bool(args.write),
        )
        print_payload(payload, as_json=bool(args.json or not args.write))
        return 0
    if args.cmd == "prepare-patch-bundle":
        payload = prepare_patch_bundle_v2(
            args.state_dir,
            program_id=args.program_id,
            surface=args.surface,
            write=bool(args.write),
        )
        print_payload(payload, as_json=bool(args.json or not args.write))
        return 0
    if args.cmd == "record-program-note":
        payload = record_program_note_v2(
            args.state_dir,
            program_id=args.program_id,
            note=args.note,
            source=args.source,
            write=bool(args.write),
        )
        print_payload(payload, as_json=bool(args.json or not args.write))
        return 0
    if args.cmd == "record-program-objection":
        payload = record_program_objection_v2(
            args.state_dir,
            program_id=args.program_id,
            being=args.being,
            summary=args.summary,
            evidence_refs=list(args.evidence_ref or []),
            write=bool(args.write),
        )
        print_payload(payload, as_json=bool(args.json or not args.write))
        return 0
    if args.cmd == "record-lease-revocation":
        payload = record_lease_revocation(
            args.state_dir,
            lease_id=args.lease_id,
            reason=args.reason,
            write=bool(args.write),
        )
        print_payload(payload, as_json=bool(args.json or not args.write))
        return 0
    parser.print_help()
    return 2


if __name__ == "__main__":
    sys.exit(main())
