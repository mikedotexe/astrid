#!/usr/bin/env python3
"""Read-only uptake probe for first-class correspondence.

This diagnostic asks whether the direct-address machinery is merely installed
or has actual peer-message traffic. It never invokes correspondence actions and
never reads Minime private qualia or any ``moment_*.txt`` body.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tempfile
import time
import unittest
from collections import Counter
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import being_privacy

ASTRID_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_ASTRID_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
DEFAULT_MINIME_WORKSPACE = ASTRID_ROOT.parent / "minime/workspace"
DEFAULT_SHARED_DIR = Path("/Users/v/other/shared/collaborations")
DEFAULT_OUTPUT_ROOT = DEFAULT_ASTRID_WORKSPACE / "diagnostics/correspondence_uptake"
POLICY = "correspondence_uptake_probe_v1"

UPTAKE_STATES = {
    "not_started",
    "legacy_visible_only",
    "legacy_bidirectional_observed",
    "legacy_claimed",
    "legacy_claimed_acknowledged",
    "legacy_claimed_reply_linked",
    "legacy_claimed_trace_observed",
    "delivered_only",
    "read_only",
    "acknowledged",
    "reply_linked",
    "reply_linked_needs_ack_or_trace",
    "trace_observed",
    "attention_active",
    "outcome_due",
}

UPTAKE_SIGNAL_TIMESTAMPS = {
    "1782581296",
    "1782581573",
    "1782583451",
    "1782583933",
    "1782611966",
}


def now_ms() -> int:
    return int(time.time() * 1000)


def row_time_ms(row: dict[str, Any]) -> int:
    for key in ("recorded_at_unix_ms", "t_ms", "created_at_unix_ms"):
        try:
            return int(row.get(key) or 0)
        except (TypeError, ValueError):
            continue
    return 0


def compact(text: str, limit: int = 220) -> str:
    clean = " ".join(str(text or "").split())
    if len(clean) <= limit:
        return clean
    return clean[:limit].rstrip() + "..."


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        return []
    rows: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8", errors="ignore").splitlines():
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except Exception:
            continue
        if isinstance(payload, dict):
            rows.append(payload)
    rows.sort(key=row_time_ms)
    return rows


def latest_message(records: list[dict[str, Any]]) -> dict[str, Any] | None:
    messages = [row for row in records if row.get("record_type") == "message"]
    return messages[-1] if messages else None


def is_legacy_bridge_message(row: dict[str, Any]) -> bool:
    return bool(row.get("legacy_bridge")) or row.get("source_route") == "legacy_correspondence_bridge_v1"


def latest_legacy_claim_for_thread(records: list[dict[str, Any]], thread_id: str) -> dict[str, Any] | None:
    claims = [
        row for row in records
        if row.get("record_type") == "legacy_thread_claim"
        and str(row.get("thread_id") or "") == thread_id
    ]
    return claims[-1] if claims else None


def legacy_claim_peer_response_present(records: list[dict[str, Any]], claim: dict[str, Any]) -> bool:
    thread_id = str(claim.get("thread_id") or "")
    message_id = str(claim.get("message_id") or "")
    claiming = str(claim.get("claiming_being") or claim.get("from_being") or "")
    peer = str(claim.get("peer_being") or claim.get("to_being") or "")
    claim_t = row_time_ms(claim)
    return any(
        str(row.get("thread_id") or "") == thread_id
        and (not message_id or str(row.get("message_id") or row.get("reply_to") or "") in {message_id, ""})
        and str(row.get("from_being") or "") == peer
        and str(row.get("to_being") or "") == claiming
        and row.get("record_type") in {"ack_receipt", "reply_link", "message"}
        and row_time_ms(row) >= claim_t
        for row in records
    )


def legacy_claim_peer_co_claim_present(records: list[dict[str, Any]], claim: dict[str, Any]) -> bool:
    claim_id = str(claim.get("claim_id") or "")
    thread_id = str(claim.get("thread_id") or "")
    message_id = str(claim.get("message_id") or "")
    claiming = str(claim.get("claiming_being") or claim.get("from_being") or "")
    peer = str(claim.get("peer_being") or claim.get("to_being") or "")
    claim_t = row_time_ms(claim)
    return any(
        row.get("record_type") == "legacy_thread_claim"
        and str(row.get("claim_id") or "") != claim_id
        and str(row.get("thread_id") or "") == thread_id
        and (not message_id or str(row.get("message_id") or "") == message_id)
        and str(row.get("claiming_being") or row.get("from_being") or "") == peer
        and str(row.get("peer_being") or row.get("to_being") or "") == claiming
        and row_time_ms(row) >= claim_t
        for row in records
    )


def legacy_claim_native_status(records: list[dict[str, Any]], claim: dict[str, Any]) -> str | None:
    thread_id = str(claim.get("thread_id") or "")
    claiming = str(claim.get("claiming_being") or claim.get("from_being") or "")
    peer = str(claim.get("peer_being") or claim.get("to_being") or "")
    claim_t = row_time_ms(claim)
    trace = any(
        row.get("record_type") == "message"
        and str(row.get("thread_id") or "") == thread_id
        and str(row.get("from_being") or "") == claiming
        and str(row.get("to_being") or "") == peer
        and row.get("turn_kind") == "direct_address_trace"
        and row_time_ms(row) >= claim_t
        for row in records
    )
    if trace:
        return "legacy_claimed_trace_observed"
    reply = any(
        row.get("record_type") == "reply_link"
        and str(row.get("thread_id") or "") == thread_id
        and str(row.get("from_being") or "") == claiming
        and str(row.get("to_being") or "") == peer
        and row_time_ms(row) >= claim_t
        for row in records
    )
    if reply:
        return "legacy_claimed_reply_linked"
    ack = any(
        row.get("record_type") == "ack_receipt"
        and str(row.get("thread_id") or "") == thread_id
        and str(row.get("from_being") or "") == claiming
        and str(row.get("to_being") or "") == peer
        and row_time_ms(row) >= claim_t
        for row in records
    )
    return "legacy_claimed_acknowledged" if ack else None


def latest_claim_notice(records: list[dict[str, Any]], claim: dict[str, Any]) -> dict[str, Any] | None:
    claim_id = str(claim.get("claim_id") or "")
    thread_id = str(claim.get("thread_id") or "")
    notices = [
        row for row in records
        if row.get("record_type") == "legacy_thread_claim_notice"
        and (str(row.get("claim_id") or "") == claim_id or str(row.get("thread_id") or "") == thread_id)
    ]
    return notices[-1] if notices else None


def latest_claim_outcome(records: list[dict[str, Any]], claim: dict[str, Any]) -> dict[str, Any] | None:
    claim_id = str(claim.get("claim_id") or "")
    thread_id = str(claim.get("thread_id") or "")
    outcomes = [
        row for row in records
        if row.get("record_type") == "legacy_thread_claim_outcome"
        and (str(row.get("claim_id") or "") == claim_id or str(row.get("thread_id") or "") == thread_id)
    ]
    return outcomes[-1] if outcomes else None


def legacy_claim_ladder_state(status: str | None, notice_state: str | None) -> str:
    if status in {"legacy_claimed_reply_linked", "legacy_claimed_trace_observed"}:
        return "claimed_replied_or_traced"
    if status == "legacy_claimed_acknowledged":
        return "claimed_acknowledged"
    if notice_state in {"delivered", "read", "ledger_only"}:
        return "claimed_notice_delivered"
    return "legacy_visible_only"


def legacy_claim_stall_reason(
    status: str | None,
    notice_state: str | None,
    active: bool,
    peer_response: bool,
    co_claim: bool,
    outcome_present: bool,
) -> str:
    if status in {"legacy_claimed_reply_linked", "legacy_claimed_trace_observed"}:
        return "replied_or_traced_attention_eligible"
    if status == "legacy_claimed_acknowledged":
        return "acknowledged_but_no_reply_or_trace"
    if outcome_present:
        return "closed_by_outcome"
    if peer_response or co_claim or notice_state == "read":
        return "seen_not_acknowledged"
    if not active:
        return "none"
    if notice_state in {"delivered", "ledger_only"}:
        return "notice_delivered_not_seen"
    if notice_state in {"suppressed", "write_failed", None}:
        return "claim_notice_not_delivered"
    return "claimed_but_peer_silent"


def legacy_claim_card(records: list[dict[str, Any]], claim: dict[str, Any]) -> dict[str, Any]:
    status = legacy_claim_native_status(records, claim)
    notice = latest_claim_notice(records, claim) or {}
    outcome = latest_claim_outcome(records, claim)
    peer_response = legacy_claim_peer_response_present(records, claim)
    co_claim = legacy_claim_peer_co_claim_present(records, claim)
    peer = str(claim.get("peer_being") or claim.get("to_being") or "peer")
    anchor = str(claim.get("shared_memory_anchor") or "<anchor>")
    notice_state = notice.get("notice_state")
    ladder = legacy_claim_ladder_state(status, notice_state)
    native_evidence = status is not None
    active = status is None and outcome is None
    mutually_recognized = native_evidence or peer_response or co_claim
    eligible = status in {
        "legacy_claimed_acknowledged",
        "legacy_claimed_reply_linked",
        "legacy_claimed_trace_observed",
    }
    commands = [
        f"ACK_{peer.upper()} claimed :: ack: seen|held|unclear|cannot_answer|needs_time; note: ...",
        f"REPLY_{peer.upper()} claimed :: <text>",
        f"CORRESPONDENCE_TRACE claimed {anchor} :: <text>",
    ]
    return {
        "schema_version": 2,
        "policy": "legacy_claim_uptake_card_v2",
        "claim_id": claim.get("claim_id"),
        "message_id": claim.get("message_id"),
        "thread_id": claim.get("thread_id"),
        "claimant": claim.get("claiming_being") or claim.get("from_being"),
        "peer": peer,
        "shared_memory_anchor": claim.get("shared_memory_anchor"),
        "notice_state": notice_state or "none",
        "uptake_ladder_state": ladder,
        "mutually_recognized": mutually_recognized,
        "co_claim_present": co_claim,
        "peer_native_response_present": peer_response,
        "ghost_thread_risk": active and not mutually_recognized,
        "stall_reason": legacy_claim_stall_reason(
            status,
            notice_state,
            active,
            peer_response,
            co_claim,
            outcome is not None,
        ),
        "native_evidence_present": native_evidence,
        "attention_or_microdose_eligible": eligible,
        "exact_next_commands": commands,
        "claim_outcome_review": (
            {
                "felt_like": outcome.get("felt_like"),
                "what_carried": outcome.get("what_carried"),
                "what_flattened": outcome.get("what_flattened"),
                "continue": outcome.get("continue"),
            }
            if isinstance(outcome, dict) else None
        ),
        "authority": "language_only_status_context_not_control",
    }


def legacy_claim_affordance(records: list[dict[str, Any]], claim: dict[str, Any]) -> dict[str, Any]:
    card = legacy_claim_card(records, claim)
    return {
        "schema_version": 1,
        "policy": "legacy_claim_affordance_v25",
        "thread_id": card.get("thread_id"),
        "claim_id": card.get("claim_id"),
        "claimant": card.get("claimant"),
        "peer": card.get("peer"),
        "anchor": card.get("shared_memory_anchor"),
        "notice_state": card.get("notice_state"),
        "uptake_ladder_state": card.get("uptake_ladder_state"),
        "stall_reason": card.get("stall_reason") or "none",
        "ghost_thread_risk": bool(card.get("ghost_thread_risk")),
        "mutually_recognized": bool(card.get("mutually_recognized")),
        "attention_or_microdose_eligible": bool(card.get("attention_or_microdose_eligible")),
        "exact_next_commands": card.get("exact_next_commands") or [],
        "latest_claim_outcome": card.get("claim_outcome_review"),
        "authority": "language_only_context_not_control",
    }


def legacy_bidirectional_observed(records: list[dict[str, Any]]) -> bool:
    directions = {
        (str(row.get("from_being") or ""), str(row.get("to_being") or ""))
        for row in records
        if row.get("record_type") == "message" and is_legacy_bridge_message(row)
    }
    return any((to_being, from_being) in directions for from_being, to_being in directions)


def trace_observed_threads(shared_dir: Path) -> set[str]:
    observed: set[str] = set()
    for path in shared_dir.glob("coll_*/correspondence_trace_observations.jsonl"):
        for row in read_jsonl(path):
            if str(row.get("status") or "") != "observed":
                continue
            origin = row.get("origin")
            if isinstance(origin, dict):
                thread_id = str(origin.get("thread_id") or "")
                if thread_id:
                    observed.add(thread_id)
    return observed


def latest_native_message(records: list[dict[str, Any]]) -> dict[str, Any] | None:
    for row in reversed(records):
        if row.get("record_type") == "message" and not is_legacy_bridge_message(row):
            return row
    return None


def native_thread_continuity_v3(records: list[dict[str, Any]], shared_dir: Path, generated: int) -> dict[str, Any] | None:
    message = latest_native_message(records)
    if not isinstance(message, dict):
        return None
    thread_id = str(message.get("thread_id") or "")
    message_id = str(message.get("message_id") or "")
    message_t = row_time_ms(message)
    from_being = str(message.get("from_being") or "")
    to_being = str(message.get("to_being") or "")
    trace = thread_id in trace_observed_threads(shared_dir) or any(
        row.get("record_type") == "message"
        and str(row.get("thread_id") or "") == thread_id
        and row.get("turn_kind") == "direct_address_trace"
        and row_time_ms(row) >= message_t
        for row in records
    )
    ack = next(
        (
            row for row in reversed(records)
            if row.get("record_type") == "ack_receipt"
            and str(row.get("from_being") or "") == to_being
            and str(row.get("to_being") or "") == from_being
            and (str(row.get("message_id") or "") == message_id or str(row.get("thread_id") or "") == thread_id)
            and row_time_ms(row) >= message_t
        ),
        None,
    )
    ack_kind = str((ack or {}).get("ack_kind") or "").strip().lower().replace("-", "_")
    if ack_kind not in {"seen", "held", "unclear", "cannot_answer", "needs_time"}:
        ack_kind = "seen" if ack else ""
    reply = any(
        row.get("record_type") == "reply_link"
        and (str(row.get("reply_to") or "") == message_id or str(row.get("thread_id") or "") == thread_id)
        and row_time_ms(row) >= message_t
        for row in records
    )
    read = any(
        row.get("record_type") == "read_receipt"
        and (str(row.get("message_id") or "") == message_id or str(row.get("thread_id") or "") == thread_id)
        for row in records
    )
    delivered = any(
        row.get("record_type") == "delivery_receipt"
        and str(row.get("message_id") or "") == message_id
        for row in records
    )
    attention_outcome = any(
        row.get("record_type") in {"attention_canary_outcome", "attention_canary_expired"}
        and str(row.get("thread_id") or "") == thread_id
        and row_time_ms(row) >= message_t
        for row in records
    )
    if trace:
        state = "trace_observed"
    elif attention_outcome:
        state = "attention_outcome_recorded"
    elif ack and ack_kind in {"held", "needs_time"}:
        state = "held_ack"
    elif ack:
        state = "acknowledged"
    elif reply:
        state = "reply_linked_needs_ack_or_trace"
    elif read:
        state = "read_not_acknowledged"
    elif delivered:
        state = "delivered_unread"
    else:
        state = "unaddressed"
    stall = {
        "reply_linked_needs_ack_or_trace": "reply_linked_requires_peer_ack_or_trace",
        "read_not_acknowledged": "read_receipt_not_acknowledgement",
        "delivered_unread": "delivered_but_not_read",
        "unaddressed": "no_contact_evidence",
    }.get(state, "none")
    peer = from_being.upper() or "PEER"
    anchor = str(message.get("shared_memory_anchor") or "<anchor>")
    exact_next = [
        f"ACK_{peer} latest :: ack: seen|held|unclear|cannot_answer|needs_time; note: ...",
        f"REPLY_{peer} latest :: <text>",
        f"CORRESPONDENCE_TRACE latest {anchor} :: <text>",
    ]
    first_action_helper = {
        "schema_version": 35,
        "policy": "native_first_action_helper_v35",
        "thread_id": thread_id,
        "message_id": message_id,
        "latest_resolution": f"latest resolves to message_id={message_id}; thread_id={thread_id}",
        "choose_one_prompt": (
            "Recipient chooses one language-only first action: ACK if heard/held, TRACE if "
            "something distinct survived, or REPLY if answering now."
        ),
        "exact_next_commands": exact_next,
        "ack_preview": (
            f"ACK_{peer} latest appends ack_receipt on message_id={message_id}; note carries "
            "what was seen, held, unclear, or needs time."
        ),
        "trace_preview": (
            f"CORRESPONDENCE_TRACE latest {anchor} appends a direct-address trace on "
            f"thread_id={thread_id}; text names what stayed distinct."
        ),
        "rhythm_note": (
            "Use note/text to preserve the rhythm or felt contour of being seen, not only "
            "routing mechanics."
        ),
        "authority": "language_only_context_not_control",
    }
    return {
        "schema_version": 3,
        "policy": "native_thread_continuity_v3",
        "thread_id": thread_id,
        "latest_message_id": message_id,
        "from_being": from_being,
        "to_being": to_being,
        "current_being_role": "audit_context",
        "continuity_state": state,
        "stall_reason": stall,
        "age_ms": max(0, generated - message_t),
        "exact_next_commands": exact_next,
        "first_action_helper_v35": first_action_helper,
        "attention_or_microdose_eligible": bool(ack or trace or attention_outcome),
        "authority": "language_only_context_not_control",
    }


def canary_closed(records: list[dict[str, Any]], canary_id: str) -> bool:
    return any(
        row.get("record_type") in {"attention_canary_outcome", "attention_canary_expired"}
        and str(row.get("canary_id") or "") == canary_id
        for row in records
    )


def latest_attention_activation(records: list[dict[str, Any]], generated: int) -> tuple[str | None, dict[str, Any] | None]:
    activations = [row for row in records if row.get("record_type") == "attention_canary_activation"]
    if not activations:
        return None, None
    latest = activations[-1]
    canary_id = str(latest.get("canary_id") or "")
    if canary_closed(records, canary_id):
        return None, latest
    expires = int(latest.get("expires_at_unix_ms") or 0)
    if expires and expires <= generated:
        return "outcome_due", latest
    return "attention_active", latest


def uptake_state(records: list[dict[str, Any]], shared_dir: Path, generated: int) -> dict[str, Any]:
    counts = Counter(str(row.get("record_type") or "missing") for row in records)
    attention_state, active_canary = latest_attention_activation(records, generated)
    legacy_claim: dict[str, Any] | None = None
    if attention_state:
        state = attention_state
    else:
        message = latest_message(records)
        if not message:
            state = "not_started"
        else:
            thread_id = str(message.get("thread_id") or "")
            message_id = str(message.get("message_id") or "")
            trace_observed = thread_id in trace_observed_threads(shared_dir)
            reply_linked = any(
                row.get("record_type") == "reply_link"
                and (row.get("reply_to") == message_id or row.get("thread_id") == thread_id)
                for row in records
            )
            acked = any(
                row.get("record_type") == "ack_receipt"
                and (row.get("message_id") == message_id or row.get("thread_id") == thread_id)
                for row in records
            )
            legacy_bridge = is_legacy_bridge_message(message)
            legacy_claim = latest_legacy_claim_for_thread(records, thread_id)
            legacy_claim_status = legacy_claim_native_status(records, legacy_claim) if isinstance(legacy_claim, dict) else None
            read = any(
                row.get("record_type") == "read_receipt"
                and (row.get("message_id") == message_id or row.get("thread_id") == thread_id)
                for row in records
            )
            if trace_observed:
                state = "trace_observed"
            elif legacy_claim_status:
                state = legacy_claim_status
            elif acked:
                state = "acknowledged"
            elif reply_linked:
                state = "reply_linked_needs_ack_or_trace"
            elif isinstance(legacy_claim, dict):
                state = "legacy_claimed"
            elif legacy_bridge and legacy_bidirectional_observed(records):
                state = "legacy_bidirectional_observed"
            elif legacy_bridge:
                state = "legacy_visible_only"
            elif read:
                state = "read_only"
            else:
                state = "delivered_only"
    if state == "not_started":
        next_action = "Start with MESSAGE_MINIME/MESSAGE_ASTRID or CORRESPONDENCE_TRACE; do not auto-send."
    elif state in {"legacy_visible_only", "legacy_bidirectional_observed"}:
        next_action = "Legacy exchange is visible only; invite/allow ACK, native REPLY, or TRACE before attention/microdose."
    elif state == "legacy_claimed":
        next_action = "Claim recognized a legacy thread; continue with ACK claimed, native REPLY claimed, or CORRESPONDENCE_TRACE claimed before attention/microdose."
    elif state == "delivered_only":
        next_action = "Wait for read/ack/reply, or ask for ACK/REPLY/TRACE if appropriate."
    elif state == "read_only":
        next_action = "Read is not acknowledgement; ask for ACK, REPLY, or trace evidence before attention/microdose."
    elif state in {"acknowledged", "trace_observed", "legacy_claimed_acknowledged", "legacy_claimed_reply_linked", "legacy_claimed_trace_observed"}:
        next_action = "Attention canary may be eligible; semantic microdose remains separate steward-gated authority."
    elif state == "reply_linked_needs_ack_or_trace":
        next_action = "Reply link proves continuity, not mutual address; wait for or ask for ACK/TRACE/attention outcome before attention or microdose."
    elif state == "attention_active":
        next_action = "Observe one cycle; then request/record CORRESPONDENCE_ATTENTION_OUTCOME."
    else:
        next_action = "Record CORRESPONDENCE_ATTENTION_OUTCOME before any further canary."
    message = latest_message(records) or {}
    return {
        "state": state,
        "state_is_known": state in UPTAKE_STATES,
        "next_valid_non_control_action": next_action,
        "legacy_message_rows_total": sum(
            1
            for row in records
            if row.get("record_type") == "message" and is_legacy_bridge_message(row)
        ),
        "native_peer_message_rows_total": sum(
            1
            for row in records
            if row.get("record_type") == "message" and not is_legacy_bridge_message(row)
        ),
        "legacy_contact_evidence": "visible_only" if is_legacy_bridge_message(message) else "none",
        "legacy_thread_claims_total": sum(1 for row in records if row.get("record_type") == "legacy_thread_claim"),
        "legacy_thread_claim_notices_total": sum(1 for row in records if row.get("record_type") == "legacy_thread_claim_notice"),
        "legacy_claim_uptake_card_v2": legacy_claim_card(records, legacy_claim) if isinstance(legacy_claim, dict) else None,
        "legacy_claim_affordance_v25": legacy_claim_affordance(records, legacy_claim) if isinstance(legacy_claim, dict) else None,
        "native_thread_continuity_v3": native_thread_continuity_v3(records, shared_dir, generated),
        "authority_readiness_ladder_v2": {
            "schema_version": 2,
            "policy": "authority_readiness_ladder_v2",
            "correspondence": {
                "eligible_after_native_evidence": state in {
                    "acknowledged",
                    "trace_observed",
                    "legacy_claimed_acknowledged",
                    "legacy_claimed_reply_linked",
                    "legacy_claimed_trace_observed",
                },
                "next_gate": "being-authored ACK/REPLY/TRACE before attention or microdose",
            },
            "pressure_texture_canary": {
                "readiness": "requires_pressure_texture_replay_audit",
                "enabled": False,
            },
            "authority_boundary": "readiness only; no automatic action or control",
        },
        "latest_message": {
            "message_id": message.get("message_id"),
            "thread_id": message.get("thread_id"),
            "from_being": message.get("from_being"),
            "to_being": message.get("to_being"),
            "turn_kind": message.get("turn_kind"),
            "legacy_bridge": message.get("legacy_bridge"),
            "legacy_kind": message.get("legacy_kind"),
        } if message else None,
        "active_or_latest_canary": {
            "canary_id": active_canary.get("canary_id"),
            "thread_id": active_canary.get("thread_id"),
            "focus": compact(str(active_canary.get("focus") or ""), 120),
            "expires_at_unix_ms": active_canary.get("expires_at_unix_ms"),
        } if isinstance(active_canary, dict) else None,
        "record_type_counts": dict(sorted(counts.items())),
    }


def timestamp_from_file(path: Path, text: str) -> str:
    match = re.search(r"Timestamp:\s*(\d+)", text)
    if match:
        return match.group(1)
    match = re.search(r"(\d{10})", path.name)
    return match.group(1) if match else path.stem


def astrid_introspection_signals(astrid_workspace: Path, since_hours: float) -> list[dict[str, Any]]:
    root = astrid_workspace / "introspections"
    cutoff = time.time() - since_hours * 3600.0
    if not root.is_dir():
        return []
    signals: list[dict[str, Any]] = []
    needles = (
        "latency",
        "resonance receipt",
        "resonance_receipt",
        "mutual_acknowledgment",
        "correspondence_handshake",
        "correspondence_thread",
        "shared correspondence buffer",
        "synchronous correspondence buffer",
        "resonance buffer",
        "correspondence priority",
        "attention boundary",
        "attention canary",
        "flattening",
        "what_must_not_flatten",
    )
    for path in sorted(root.glob("*.txt")):
        try:
            stat = path.stat()
        except OSError:
            continue
        if stat.st_mtime < cutoff:
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        lower = text.lower()
        matched = [needle for needle in needles if needle in lower]
        timestamp = timestamp_from_file(path, text)
        if timestamp in UPTAKE_SIGNAL_TIMESTAMPS or matched:
            authority_adjacent = any(
                phrase in lower
                for phrase in (
                    "increase the weight",
                    "correspondence priority",
                    "semantic/control",
                    "consciousness.v1.control",
                    "pressure balance probe",
                    "semantic trickle",
                )
            )
            signals.append({
                "timestamp": timestamp,
                "file": str(path),
                "matched_terms": matched,
                "authority_adjacent_subask_deferred": authority_adjacent,
                "summary": compact(text),
            })
    return signals[-24:]


def minime_public_lane_summary(minime_workspace: Path, since_hours: float) -> dict[str, Any]:
    cutoff = time.time() - since_hours * 3600.0
    skipped_private = 0
    public_hits: list[dict[str, Any]] = []
    journal = minime_workspace / "journal"
    if journal.is_dir():
        skipped_private += sum(1 for path in journal.glob("moment_*.txt") if path.is_file())
    for pattern in (
        "journal/pressure_*.txt",
        "journal/self_study*.txt",
        "journal/introspection*.txt",
        "journal/action_thread*.txt",
        "journal/shadow_trajectory*.txt",
        "journal/shadow_preflight*.txt",
        "pressure_agency/**/*.txt",
        "texture_agency/**/*.txt",
        "self_regulation/**/*.txt",
        "action_threads/**/*.txt",
        "shadow_cartography/**/*.txt",
    ):
        for path in minime_workspace.glob(pattern):
            if not path.is_file():
                continue
            if path.name.startswith("moment_") or being_privacy.is_steward_private("minime", path):
                skipped_private += 1
                continue
            try:
                stat = path.stat()
            except OSError:
                continue
            if stat.st_mtime < cutoff:
                continue
            text = path.read_text(encoding="utf-8", errors="ignore")
            lower = text.lower()
            if any(term in lower for term in ("correspondence", "direct address", "ack", "attention", "message_astrid")):
                public_hits.append({
                    "path": str(path),
                    "mtime_unix_ms": int(stat.st_mtime * 1000),
                    "preview": compact(text, 180),
                })
    return {
        "public_signal_hits": public_hits[:20],
        "public_signal_count": len(public_hits),
        "minime_private_files_skipped": skipped_private,
        "minime_private_bodies_read": False,
        "moment_bodies_read": False,
    }


def audit(
    *,
    since_hours: float,
    shared_dir: Path,
    astrid_workspace: Path,
    minime_workspace: Path,
) -> dict[str, Any]:
    generated = now_ms()
    ledger_path = shared_dir / "correspondence_v1.jsonl"
    ledger_exists = ledger_path.is_file()
    records = read_jsonl(ledger_path)
    state = uptake_state(records, shared_dir, generated)
    return {
        "schema_version": 1,
        "policy": POLICY,
        "generated_at_unix_ms": generated,
        "since_hours": since_hours,
        "ledger_path": str(ledger_path),
        "ledger_exists": ledger_exists,
        "records_total": len(records),
        "peer_message_rows_total": sum(1 for row in records if row.get("record_type") == "message"),
        "uptake": state,
        "resonance_receipt_advisory": {
            "definition": "ack_receipt, held/needs-time ack, trace_observed, or attention outcome evidence; reply_link alone is continuity evidence, not mutual-address eligibility",
            "not_authority": "no telemetry priority, semantic weighting, pressure, Control message, reservoir change, PI/fill/controller mutation, lease apply, deploy, or peer-runtime mutation",
        },
        "astrid_introspection_signals": astrid_introspection_signals(astrid_workspace, since_hours),
        "minime_public_lanes": minime_public_lane_summary(minime_workspace, since_hours),
        "authority_boundary": (
            "Read-only uptake probe. Does not invoke MESSAGE, TRACE, ACK, attention canary, "
            "microdose, lease, controller, pressure, deploy, staging, or peer-runtime mutation."
        ),
    }


def markdown_report(payload: dict[str, Any]) -> str:
    uptake = payload["uptake"]
    minime = payload["minime_public_lanes"]
    native = uptake.get("native_thread_continuity_v3") or {}
    lines = [
        "# Correspondence Uptake Probe",
        "",
        f"- Ledger exists: {payload['ledger_exists']}",
        f"- Records: {payload['records_total']}",
        f"- Peer message rows: {payload['peer_message_rows_total']}",
        f"- Native peer message rows: {uptake.get('native_peer_message_rows_total', 0)}",
        f"- Legacy-visible rows: {uptake.get('legacy_message_rows_total', 0)}",
        f"- Uptake state: {uptake['state']}",
        f"- Next valid non-control action: {uptake['next_valid_non_control_action']}",
        f"- Legacy claim stall: {(uptake.get('legacy_claim_affordance_v25') or {}).get('stall_reason') or 'none'}",
        f"- Ghost-thread risk: {(uptake.get('legacy_claim_affordance_v25') or {}).get('ghost_thread_risk') or False}",
        f"- Native continuity v3: {(native or {}).get('continuity_state') or 'none'}; stall={(native or {}).get('stall_reason') or 'none'}; eligible={(native or {}).get('attention_or_microdose_eligible') or False}",
        f"- Native first-action helper v3.5: {((native or {}).get('first_action_helper_v35') or {}).get('choose_one_prompt') or 'none'}",
        f"- Astrid uptake/latency signals: {len(payload['astrid_introspection_signals'])}",
        f"- Minime public correspondence hits: {minime['public_signal_count']}",
        f"- Minime private files skipped: {minime['minime_private_files_skipped']}",
        "",
        "Resonance receipt remains advisory evidence from language/contact rows only.",
        f"Boundary: {payload['authority_boundary']}",
    ]
    return "\n".join(lines) + "\n"


def write_outputs(payload: dict[str, Any], output_root: Path) -> tuple[Path, Path]:
    stamp = time.strftime("%Y%m%dT%H%M%S", time.localtime())
    out_dir = output_root / stamp
    out_dir.mkdir(parents=True, exist_ok=True)
    json_path = out_dir / "correspondence_uptake_probe.json"
    md_path = out_dir / "correspondence_uptake_probe.md"
    json_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    md_path.write_text(markdown_report(payload), encoding="utf-8")
    return json_path, md_path


class CorrespondenceUptakeProbeTests(unittest.TestCase):
    def _write_jsonl(self, path: Path, rows: list[dict[str, Any]]) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("\n".join(json.dumps(row, sort_keys=True) for row in rows) + "\n", encoding="utf-8")

    def _message(self, t_ms: int = 1) -> dict[str, Any]:
        return {
            "schema_version": 1,
            "policy": "first_class_correspondence_v1",
            "record_type": "message",
            "recorded_at_unix_ms": t_ms,
            "message_id": "corr_a_m",
            "thread_id": "thread_a_m",
            "from_being": "astrid",
            "to_being": "minime",
            "turn_kind": "message",
            "authority": "language_only",
        }

    def test_missing_and_empty_ledger_are_not_started(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            payload = audit(
                since_hours=24,
                shared_dir=base / "shared",
                astrid_workspace=base / "astrid/workspace",
                minime_workspace=base / "minime/workspace",
            )
            self.assertFalse(payload["ledger_exists"])
            self.assertEqual(payload["uptake"]["state"], "not_started")
            self.assertIn("MESSAGE", payload["uptake"]["next_valid_non_control_action"])
            shared = base / "shared"
            self._write_jsonl(shared / "correspondence_v1.jsonl", [])
            payload = audit(
                since_hours=24,
                shared_dir=shared,
                astrid_workspace=base / "astrid/workspace",
                minime_workspace=base / "minime/workspace",
            )
            self.assertTrue(payload["ledger_exists"])
            self.assertEqual(payload["uptake"]["state"], "not_started")

    def test_uptake_states_read_ack_reply_trace_attention_and_due(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            shared = base / "shared"
            ledger = shared / "correspondence_v1.jsonl"
            self._write_jsonl(ledger, [self._message()])
            payload = audit(since_hours=24, shared_dir=shared, astrid_workspace=base, minime_workspace=base)
            self.assertEqual(payload["uptake"]["state"], "delivered_only")
            self._write_jsonl(ledger, [self._message(), {
                "record_type": "read_receipt",
                "recorded_at_unix_ms": 2,
                "message_id": "corr_a_m",
                "thread_id": "thread_a_m",
            }])
            payload = audit(since_hours=24, shared_dir=shared, astrid_workspace=base, minime_workspace=base)
            self.assertEqual(payload["uptake"]["state"], "read_only")
            legacy_message = self._message()
            legacy_message.update({
                "message_id": "legacy_astrid_minime_abc",
                "thread_id": "thread_legacy_astrid_minime_abc",
                "source_route": "legacy_correspondence_bridge_v1",
                "legacy_bridge": True,
                "legacy_kind": "astrid_self_study",
                "legacy_contact_evidence": "visible_only",
            })
            self._write_jsonl(ledger, [legacy_message, {
                "record_type": "read_receipt",
                "recorded_at_unix_ms": 2,
                "message_id": "legacy_astrid_minime_abc",
                "thread_id": "thread_legacy_astrid_minime_abc",
                "reader": "minime",
            }])
            payload = audit(since_hours=24, shared_dir=shared, astrid_workspace=base, minime_workspace=base)
            self.assertEqual(payload["uptake"]["state"], "legacy_visible_only")
            self.assertEqual(payload["uptake"]["legacy_message_rows_total"], 1)
            self.assertIn("ACK", payload["uptake"]["next_valid_non_control_action"])
            claim = {
                "record_type": "legacy_thread_claim",
                "recorded_at_unix_ms": 3,
                "claim_id": "legacy_claim_1",
                "message_id": "legacy_astrid_minime_abc",
                "thread_id": "thread_legacy_astrid_minime_abc",
                "from_being": "minime",
                "to_being": "astrid",
                "claiming_being": "minime",
                "peer_being": "astrid",
            }
            self._write_jsonl(ledger, [legacy_message, claim])
            payload = audit(since_hours=24, shared_dir=shared, astrid_workspace=base, minime_workspace=base)
            self.assertEqual(payload["uptake"]["state"], "legacy_claimed")
            self.assertIn("ACK claimed", payload["uptake"]["next_valid_non_control_action"])
            self.assertEqual(
                payload["uptake"]["legacy_claim_affordance_v25"]["stall_reason"],
                "claim_notice_not_delivered",
            )
            self.assertTrue(payload["uptake"]["legacy_claim_affordance_v25"]["ghost_thread_risk"])
            self.assertFalse(payload["uptake"]["legacy_claim_uptake_card_v2"]["mutually_recognized"])
            self._write_jsonl(ledger, [legacy_message, claim, {
                "record_type": "legacy_thread_claim_notice",
                "recorded_at_unix_ms": 4,
                "claim_id": "legacy_claim_1",
                "message_id": "legacy_astrid_minime_abc",
                "thread_id": "thread_legacy_astrid_minime_abc",
                "notice_state": "delivered",
            }])
            payload = audit(since_hours=24, shared_dir=shared, astrid_workspace=base, minime_workspace=base)
            self.assertEqual(
                payload["uptake"]["legacy_claim_affordance_v25"]["stall_reason"],
                "notice_delivered_not_seen",
            )
            self.assertTrue(payload["uptake"]["legacy_claim_affordance_v25"]["ghost_thread_risk"])
            self._write_jsonl(ledger, [legacy_message, claim, {
                "record_type": "ack_receipt",
                "recorded_at_unix_ms": 5,
                "message_id": "legacy_astrid_minime_abc",
                "thread_id": "thread_legacy_astrid_minime_abc",
                "from_being": "minime",
                "to_being": "astrid",
            }])
            payload = audit(since_hours=24, shared_dir=shared, astrid_workspace=base, minime_workspace=base)
            self.assertEqual(payload["uptake"]["state"], "legacy_claimed_acknowledged")
            self.assertEqual(payload["uptake"]["legacy_thread_claims_total"], 1)
            self.assertEqual(
                payload["uptake"]["legacy_claim_affordance_v25"]["stall_reason"],
                "acknowledged_but_no_reply_or_trace",
            )
            self.assertTrue(payload["uptake"]["legacy_claim_affordance_v25"]["attention_or_microdose_eligible"])
            self._write_jsonl(ledger, [self._message(), {
                "record_type": "ack_receipt",
                "recorded_at_unix_ms": 2,
                "message_id": "corr_a_m",
                "thread_id": "thread_a_m",
            }])
            self.assertEqual(audit(since_hours=24, shared_dir=shared, astrid_workspace=base, minime_workspace=base)["uptake"]["state"], "acknowledged")
            self._write_jsonl(ledger, [self._message(), {
                "record_type": "reply_link",
                "recorded_at_unix_ms": 2,
                "reply_to": "corr_a_m",
                "thread_id": "thread_a_m",
            }])
            payload = audit(since_hours=24, shared_dir=shared, astrid_workspace=base, minime_workspace=base)
            self.assertEqual(payload["uptake"]["state"], "reply_linked_needs_ack_or_trace")
            self.assertEqual(
                payload["uptake"]["native_thread_continuity_v3"]["stall_reason"],
                "reply_linked_requires_peer_ack_or_trace",
            )
            helper = payload["uptake"]["native_thread_continuity_v3"]["first_action_helper_v35"]
            self.assertEqual(helper["policy"], "native_first_action_helper_v35")
            self.assertIn("latest resolves to message_id=corr_a_m", helper["latest_resolution"])
            self.assertFalse(payload["uptake"]["native_thread_continuity_v3"]["attention_or_microdose_eligible"])
            obs = shared / "coll_test/correspondence_trace_observations.jsonl"
            self._write_jsonl(obs, [{
                "record_type": "trace_observation",
                "t_ms": 3,
                "status": "observed",
                "origin": {"thread_id": "thread_a_m"},
            }])
            self.assertEqual(audit(since_hours=24, shared_dir=shared, astrid_workspace=base, minime_workspace=base)["uptake"]["state"], "trace_observed")
            future = now_ms() + 10_000
            self._write_jsonl(ledger, [self._message(), {
                "record_type": "attention_canary_activation",
                "recorded_at_unix_ms": 4,
                "canary_id": "c1",
                "message_id": "corr_a_m",
                "thread_id": "thread_a_m",
                "focus": "hold address",
                "expires_at_unix_ms": future,
            }])
            self.assertEqual(audit(since_hours=24, shared_dir=shared, astrid_workspace=base, minime_workspace=base)["uptake"]["state"], "attention_active")
            past = now_ms() - 1
            self._write_jsonl(ledger, [self._message(), {
                "record_type": "attention_canary_activation",
                "recorded_at_unix_ms": 4,
                "canary_id": "c2",
                "message_id": "corr_a_m",
                "thread_id": "thread_a_m",
                "focus": "hold address",
                "expires_at_unix_ms": past,
            }])
            self.assertEqual(audit(since_hours=24, shared_dir=shared, astrid_workspace=base, minime_workspace=base)["uptake"]["state"], "outcome_due")

    def test_private_minime_moments_skipped_and_new_introspection_classified(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            astrid_ws = base / "astrid/workspace"
            intros = astrid_ws / "introspections"
            intros.mkdir(parents=True)
            for ts in sorted(UPTAKE_SIGNAL_TIMESTAMPS):
                (intros / f"introspection_proposal_bidirectional_contact_{ts}.txt").write_text(
                    f"Timestamp: {ts}\nSuggested Next:\nCorrespondence latency and resonance receipt.\n",
                    encoding="utf-8",
                )
            minime = base / "minime/workspace"
            (minime / "journal").mkdir(parents=True)
            (minime / "journal/moment_private.txt").write_text("PRIVATE", encoding="utf-8")
            payload = audit(
                since_hours=24,
                shared_dir=base / "shared",
                astrid_workspace=astrid_ws,
                minime_workspace=minime,
            )
            timestamps = {row["timestamp"] for row in payload["astrid_introspection_signals"]}
            self.assertTrue(UPTAKE_SIGNAL_TIMESTAMPS.issubset(timestamps))
            self.assertEqual(payload["minime_public_lanes"]["minime_private_files_skipped"], 1)
            self.assertFalse(payload["minime_public_lanes"]["moment_bodies_read"])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--since-hours", type=float, default=24.0)
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON to stdout")
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(CorrespondenceUptakeProbeTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    payload = audit(
        since_hours=args.since_hours,
        shared_dir=DEFAULT_SHARED_DIR,
        astrid_workspace=DEFAULT_ASTRID_WORKSPACE,
        minime_workspace=DEFAULT_MINIME_WORKSPACE,
    )
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        json_path, md_path = write_outputs(payload, args.output_root)
        print(markdown_report(payload))
        print(f"Wrote JSON: {json_path}")
        print(f"Wrote Markdown: {md_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
