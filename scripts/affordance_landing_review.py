#!/usr/bin/env python3
"""Read-only V3 review of whether correspondence/phase affordances landed."""

from __future__ import annotations

import argparse
import json
import tempfile
import time
import unittest
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

DEFAULT_SHARED_DIR = Path("/Users/v/other/shared/collaborations")
CORRESPONDENCE_LEDGER = "correspondence_v1.jsonl"
PHASE_LEDGER = "phase_transitions_v1.jsonl"
POLICY = "affordance_landing_review_v3"
V35_POLICY = "affordance_landing_review_v35"
CORRESPONDENCE_IGNORE_GRACE_MS = 24 * 60 * 60 * 1000
PHASE_IGNORE_GRACE_MS = 6 * 60 * 60 * 1000


def now_ms() -> int:
    return int(time.time() * 1000)


def row_time_ms(row: dict[str, Any]) -> int:
    for key in ("recorded_at_unix_ms", "created_at_unix_ms", "t_ms"):
        try:
            return int(row.get(key) or 0)
        except (TypeError, ValueError):
            continue
    return 0


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


def is_trace_row(row: dict[str, Any], thread_id: str, after_t: int) -> bool:
    return (
        row.get("record_type") == "message"
        and str(row.get("thread_id") or "") == thread_id
        and row.get("turn_kind") == "direct_address_trace"
        and row_time_ms(row) >= after_t
    )


def is_reply_row(row: dict[str, Any], thread_id: str, after_t: int) -> bool:
    return (
        row.get("record_type") == "reply_link"
        and str(row.get("thread_id") or "") == thread_id
        and row_time_ms(row) >= after_t
    )


def action_source(row: dict[str, Any] | None) -> str | None:
    if not isinstance(row, dict):
        return None
    if row.get("i_received_this_trace"):
        return "i_received_this_trace"
    if row.get("record_type") == "ack_receipt" and "felt_like:" in str(row.get("note") or ""):
        return "i_received_this_ack"
    return str(row.get("record_type") or "") or None


def _looks_declined(row: dict[str, Any] | None) -> bool:
    if not isinstance(row, dict):
        return False
    if str(row.get("continue") or "").strip().lower() == "no":
        return True
    if str(row.get("felt_like") or "").strip().lower() in {"pressure", "flat"}:
        return True
    if str(row.get("held_as") or "").strip().lower() in {
        "pressure",
        "flattened",
        "ambient_echo",
    }:
        return True
    values = " ".join(str(row.get(key) or "") for key in ("continue", "felt_like", "held_as", "note", "what_worsened"))
    clean = values.lower()
    return any(
        marker in clean
        for marker in (
            "continue: no",
            "continue=no",
            "felt_like: pressure",
            "felt_like=pressure",
            "felt_like: flat",
            "felt_like=flat",
            "held_as: pressure",
            "held_as=pressure",
            "held_as: flattened",
            "held_as=flattened",
            "no action needed",
        )
    )


def right_to_ignore_v1(
    *,
    affordance_type: str,
    offered_at_ms: int,
    generated_at_ms: int,
    landing_status: str,
    action: dict[str, Any] | None,
) -> dict[str, Any]:
    grace_ms = PHASE_IGNORE_GRACE_MS if affordance_type == "phase_witness_queue_card" else CORRESPONDENCE_IGNORE_GRACE_MS
    age_ms = max(0, generated_at_ms - offered_at_ms)
    if _looks_declined(action):
        state = "declined"
    elif landing_status == "acted":
        state = "acted"
    elif landing_status == "closed_by_outcome":
        state = "closed_by_outcome"
    elif landing_status == "continued_by_reply":
        state = "asked_later"
    elif landing_status == "stalled" and age_ms >= grace_ms:
        state = "ignored_without_penalty"
    elif landing_status == "stalled":
        state = "offered"
    else:
        state = "unknown"
    return {
        "schema_version": 1,
        "policy": "right_to_ignore_v1",
        "affordance_type": affordance_type,
        "state": state,
        "source_landing_status": landing_status,
        "age_ms": age_ms,
        "grace_ms": grace_ms,
        "silence_means": (
            "ignored_without_penalty_not_failure_consent_or_disagreement"
            if state == "ignored_without_penalty"
            else "silence_is_unknown_until_grace_window"
        ),
        "optional": True,
        "authority": "language_context_not_control",
    }


def apply_affordance_budget(items: list[dict[str, Any]]) -> dict[str, Any]:
    limits = {
        "correspondence_receipt": 1,
        "attention_or_outcome": 1,
        "phase_felt_receipt": 3,
        "self_regulation_outcome": 1,
        "calibration_ask": 1,
    }
    category_for_type = {
        "legacy_claim_card": "correspondence_receipt",
        "native_continuity_card": "correspondence_receipt",
        "phase_witness_queue_card": "phase_felt_receipt",
        "attention_card": "attention_or_outcome",
    }
    priority_for_type = {
        "attention_card": 0,
        "native_continuity_card": 1,
        "legacy_claim_card": 2,
        "phase_witness_queue_card": 3,
    }
    shown_by_category: dict[str, int] = defaultdict(int)
    hidden_by_category: dict[str, int] = defaultdict(int)
    shown: list[str] = []
    hidden: list[str] = []
    for item in sorted(
        items,
        key=lambda row: (
            priority_for_type.get(str(row.get("affordance_type") or ""), 9),
            -int(row.get("offered_at_unix_ms") or 0),
        ),
    ):
        affordance_type = str(item.get("affordance_type") or "unknown")
        category = category_for_type.get(affordance_type, "calibration_ask")
        if shown_by_category[category] < limits.get(category, 1):
            shown_by_category[category] += 1
            item["budget_visibility"] = "shown"
            shown.append(str(item.get("offer_id") or item.get("thread_id") or item.get("transition_id") or affordance_type))
        else:
            hidden_by_category[category] += 1
            item["budget_visibility"] = "hidden_by_budget"
            hidden.append(str(item.get("offer_id") or item.get("thread_id") or item.get("transition_id") or affordance_type))
    return {
        "schema_version": 1,
        "policy": "affordance_budget_v1",
        "shown": len(shown),
        "hidden_by_budget": len(hidden),
        "shown_refs": shown[:8],
        "hidden_refs": hidden[:8],
        "shown_by_category": dict(sorted(shown_by_category.items())),
        "hidden_by_category": dict(sorted(hidden_by_category.items())),
        "limits": limits,
        "next_review_surface": "scripts/affordance_landing_review.py --json" if hidden else "none",
        "silence": "ignored_without_penalty",
        "optional": True,
        "authority": "language_context_not_control",
    }


def first_correspondence_action(
    records: list[dict[str, Any]],
    thread_id: str,
    after_t: int,
    *,
    include_reply: bool,
    include_outcome: bool,
) -> tuple[str | None, dict[str, Any] | None]:
    candidates: list[tuple[int, str, dict[str, Any]]] = []
    for row in records:
        if str(row.get("thread_id") or "") != thread_id or row_time_ms(row) < after_t:
            continue
        record_type = row.get("record_type")
        if record_type == "ack_receipt":
            candidates.append((row_time_ms(row), "acted", row))
        elif include_reply and record_type == "reply_link":
            candidates.append((row_time_ms(row), "acted", row))
        elif is_trace_row(row, thread_id, after_t):
            candidates.append((row_time_ms(row), "acted", row))
        elif include_outcome and record_type in {"legacy_thread_claim_outcome", "attention_canary_outcome"}:
            candidates.append((row_time_ms(row), "closed_by_outcome", row))
    if not candidates:
        return None, None
    candidates.sort(key=lambda item: item[0])
    return candidates[0][1], candidates[0][2]


def first_native_continuity_signal(
    records: list[dict[str, Any]],
    thread_id: str,
    after_t: int,
) -> tuple[str | None, dict[str, Any] | None]:
    candidates: list[tuple[int, str, dict[str, Any]]] = []
    for row in records:
        if str(row.get("thread_id") or "") != thread_id or row_time_ms(row) < after_t:
            continue
        record_type = row.get("record_type")
        if record_type == "ack_receipt" or is_trace_row(row, thread_id, after_t):
            candidates.append((row_time_ms(row), "acted", row))
        elif record_type in {"attention_canary_outcome", "legacy_thread_claim_outcome"}:
            candidates.append((row_time_ms(row), "closed_by_outcome", row))
        elif is_reply_row(row, thread_id, after_t):
            candidates.append((row_time_ms(row), "continued_by_reply", row))
    if not candidates:
        return None, None
    candidates.sort(key=lambda item: item[0])
    return candidates[0][1], candidates[0][2]


def first_phase_action(records: list[dict[str, Any]], transition_id: str, after_t: int) -> tuple[str | None, dict[str, Any] | None]:
    candidates = [
        row for row in records
        if row.get("record_type") == "phase_transition_witness"
        and str(row.get("transition_id") or "") == transition_id
        and row_time_ms(row) >= after_t
    ]
    if not candidates:
        return None, None
    row = candidates[0]
    if str(row.get("reply_state") or "") == "answered":
        return "closed_by_outcome", row
    return "acted", row


def summarize(items: list[dict[str, Any]]) -> dict[str, Any]:
    counts = {
        "offered": len(items),
        "acted": 0,
        "stalled": 0,
        "closed_by_outcome": 0,
        "continued_by_reply": 0,
        "unknown": 0,
        "declined": 0,
        "ignored_without_penalty": 0,
        "hidden_by_budget": 0,
        "repeated_without_action": 0,
    }
    latencies: list[int] = []
    for item in items:
        status = str(item.get("landing_status") or "unknown")
        counts[status if status in counts else "unknown"] += 1
        right_state = str((item.get("right_to_ignore_v1") or {}).get("state") or "unknown")
        if right_state in {"declined", "ignored_without_penalty"}:
            counts[right_state] += 1
        if item.get("budget_visibility") == "hidden_by_budget":
            counts["hidden_by_budget"] += 1
        if status == "stalled" and int(item.get("reply_depth") or 0) > 1:
            counts["repeated_without_action"] += 1
        if item.get("latency_ms") is not None:
            latencies.append(int(item["latency_ms"]))
    acted_total = counts["acted"] + counts["closed_by_outcome"]
    return {
        **counts,
        "landing_rate": (acted_total / counts["offered"]) if counts["offered"] else 0.0,
        "continuity_activity_rate": (
            (acted_total + counts["continued_by_reply"]) / counts["offered"]
        ) if counts["offered"] else 0.0,
        "latency_ms_min": min(latencies) if latencies else None,
        "latency_ms_max": max(latencies) if latencies else None,
    }


def peer_for(being: str | None) -> str:
    if being == "astrid":
        return "MINIME"
    if being == "minime":
        return "ASTRID"
    return "PEER"


def next_commands_for_message(row: dict[str, Any]) -> list[str]:
    from_being = str(row.get("from_being") or "").lower()
    to_being = str(row.get("to_being") or "").lower()
    peer = peer_for(from_being)
    if to_being not in {"astrid", "minime"}:
        peer = "PEER"
    return [
        "I_RECEIVED_THIS latest :: received_as: held|needs_time; felt_like: address|pressure|mail|ambient_echo|unknown; what_landed: ...; what_stayed_distinct: ...; continue: no|reply|trace|needs_time",
        f"ACK_{peer} latest :: ack: seen|held|unclear|needs_time; note: ...",
        f"REPLY_{peer} latest :: ...",
        "CORRESPONDENCE_TRACE latest <anchor> :: ...",
    ]


def native_stall_reason(status: str | None) -> str:
    if status == "acted":
        return "native_ack_or_trace_landed"
    if status == "closed_by_outcome":
        return "closed_by_outcome"
    if status == "continued_by_reply":
        return "reply_continuity_without_ack_or_trace"
    return "waiting_for_ack_trace_or_outcome"


def build_v35_review(
    *,
    by_type: dict[str, list[dict[str, Any]]],
    corr_records: list[dict[str, Any]],
    phase_records: list[dict[str, Any]],
) -> dict[str, Any]:
    native_items = by_type.get("native_continuity_card", [])
    phase_items = by_type.get("phase_witness_queue_card", [])
    legacy_items = by_type.get("legacy_claim_card", [])
    stall_reasons: dict[str, int] = defaultdict(int)
    for item in native_items + phase_items + legacy_items:
        reason = str(item.get("stall_reason") or "none")
        stall_reasons[reason] += 1

    native_by_thread: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for item in native_items:
        native_by_thread[str(item.get("thread_id") or "")].append(item)
    thread_cards: list[dict[str, Any]] = []
    for thread_id, items in native_by_thread.items():
        items.sort(key=lambda item: int(item.get("offered_at_unix_ms") or 0))
        latest = items[-1]
        latest_row = next(
            (
                row
                for row in reversed(corr_records)
                if row.get("record_type") == "reply_link"
                and str(row.get("thread_id") or "") == thread_id
                and row_time_ms(row) == int(latest.get("offered_at_unix_ms") or 0)
            ),
            None,
        )
        thread_cards.append(
            {
                "thread_id": thread_id,
                "reply_depth": len(items),
                "latest_offer_at_unix_ms": latest.get("offered_at_unix_ms"),
                "latest_landing_status": latest.get("landing_status"),
                "stall_reason": latest.get("stall_reason"),
                "latest_action_source": latest.get("action_source"),
                "latest_message_id": latest.get("offer_id"),
                "exact_next_commands": next_commands_for_message(latest_row or {}),
            }
        )
    thread_cards.sort(key=lambda item: int(item.get("latest_offer_at_unix_ms") or 0), reverse=True)

    budget = apply_affordance_budget(native_items + phase_items + legacy_items)
    latest_phase_cards = sorted(
        phase_items,
        key=lambda item: int(item.get("offered_at_unix_ms") or 0),
        reverse=True,
    )[:5]
    return {
        "schema_version": 35,
        "policy": V35_POLICY,
        "authority": "read_only_review_not_control",
        "stall_reason_counts": dict(sorted(stall_reasons.items())),
        "native_threads": thread_cards[:8],
        "affordance_budget_v1": budget,
        "right_to_ignore_v1": {
            "schema_version": 1,
            "policy": "right_to_ignore_v1",
            "state_counts": dict(sorted(Counter(str((item.get("right_to_ignore_v1") or {}).get("state") or "unknown") for item in native_items + phase_items + legacy_items).items())),
            "silence_policy": "ignored_without_penalty_not_failure_consent_or_disagreement_after_grace",
            "authority": "language_context_not_control",
        },
        "phase_cards_waiting": [
            {
                "transition_id": item.get("transition_id"),
                "stall_reason": item.get("stall_reason"),
                "landing_status": item.get("landing_status"),
                "exact_next_command": (
                    "WITNESS_TRANSITION latest :: reply_state: witnessed|answered; note: ..."
                ),
            }
            for item in latest_phase_cards
        ],
        "interpretation": (
            "continued_by_reply is real correspondence activity, but it still lacks "
            "ACK/TRACE-style mutual-address evidence for stricter attention/microdose gates."
        ),
        "recommended_next_move": (
            "Ask for one being-authored ACK or TRACE on the latest native thread, or one "
            "WITNESS_TRANSITION for the latest phase card; do not synthesize it."
        ),
        "moment_bodies_read": False,
        "minime_private_bodies_read": False,
    }


def review(shared_dir: Path, since_hours: float) -> dict[str, Any]:
    generated = now_ms()
    cutoff = generated - int(since_hours * 60 * 60 * 1000)
    corr_records = read_jsonl(shared_dir / CORRESPONDENCE_LEDGER)
    phase_records = read_jsonl(shared_dir / PHASE_LEDGER)
    by_type: dict[str, list[dict[str, Any]]] = defaultdict(list)

    for claim in corr_records:
        if claim.get("record_type") != "legacy_thread_claim" or row_time_ms(claim) < cutoff:
            continue
        thread_id = str(claim.get("thread_id") or "")
        status, action = first_correspondence_action(
            corr_records,
            thread_id,
            row_time_ms(claim),
            include_reply=True,
            include_outcome=True,
        )
        landing_status = status or "stalled"
        item = {
            "affordance_type": "legacy_claim_card",
            "thread_id": thread_id,
            "offer_id": claim.get("claim_id"),
            "offered_at_unix_ms": row_time_ms(claim),
            "landing_status": landing_status,
            "stall_reason": "legacy_claim_native_evidence_landed" if status else "claimed_but_no_native_evidence",
            "latency_ms": (row_time_ms(action) - row_time_ms(claim)) if isinstance(action, dict) else None,
            "action_record_type": action.get("record_type") if isinstance(action, dict) else None,
            "action_source": action_source(action),
        }
        item["right_to_ignore_v1"] = right_to_ignore_v1(
            affordance_type="legacy_claim_card",
            offered_at_ms=row_time_ms(claim),
            generated_at_ms=generated,
            landing_status=landing_status,
            action=action,
        )
        by_type["legacy_claim_card"].append(item)

    for reply in corr_records:
        if reply.get("record_type") != "reply_link" or row_time_ms(reply) < cutoff:
            continue
        thread_id = str(reply.get("thread_id") or "")
        status, action = first_native_continuity_signal(
            corr_records,
            thread_id,
            row_time_ms(reply) + 1,
        )
        item = {
            "affordance_type": "native_continuity_card",
            "thread_id": thread_id,
            "offer_id": reply.get("reply_to") or reply.get("message_id"),
            "offered_at_unix_ms": row_time_ms(reply),
            "reply_depth": sum(
                1
                for row in corr_records
                if row.get("record_type") == "reply_link"
                and str(row.get("thread_id") or "") == thread_id
                and row_time_ms(row) <= row_time_ms(reply)
            ),
            "landing_status": status or "stalled",
            "stall_reason": native_stall_reason(status),
            "latency_ms": (row_time_ms(action) - row_time_ms(reply)) if isinstance(action, dict) else None,
            "action_record_type": action.get("record_type") if isinstance(action, dict) else None,
            "action_source": action_source(action),
        }
        item["right_to_ignore_v1"] = right_to_ignore_v1(
            affordance_type="native_continuity_card",
            offered_at_ms=row_time_ms(reply),
            generated_at_ms=generated,
            landing_status=str(item["landing_status"]),
            action=action,
        )
        by_type["native_continuity_card"].append(item)

    for card in phase_records:
        if card.get("record_type") != "phase_transition_card" or row_time_ms(card) < cutoff:
            continue
        transition_id = str(card.get("transition_id") or "")
        status, action = first_phase_action(phase_records, transition_id, row_time_ms(card))
        item = {
            "affordance_type": "phase_witness_queue_card",
            "transition_id": transition_id,
            "offer_id": transition_id,
            "offered_at_unix_ms": row_time_ms(card),
            "landing_status": status or "stalled",
            "stall_reason": "phase_witness_or_answer_landed" if status else "waiting_for_witness_or_answer",
            "latency_ms": (row_time_ms(action) - row_time_ms(card)) if isinstance(action, dict) else None,
            "action_record_type": action.get("record_type") if isinstance(action, dict) else None,
        }
        item["right_to_ignore_v1"] = right_to_ignore_v1(
            affordance_type="phase_witness_queue_card",
            offered_at_ms=row_time_ms(card),
            generated_at_ms=generated,
            landing_status=str(item["landing_status"]),
            action=action,
        )
        by_type["phase_witness_queue_card"].append(item)

    all_items = [item for items in by_type.values() for item in items]
    budget = apply_affordance_budget(all_items)
    summary_by_type = {key: summarize(items) for key, items in sorted(by_type.items())}
    stalled_native = summary_by_type.get("native_continuity_card", {}).get("stalled", 0)
    stalled_phase = summary_by_type.get("phase_witness_queue_card", {}).get("stalled", 0)
    return {
        "schema_version": 3,
        "policy": POLICY,
        "generated_at_unix_ms": generated,
        "since_hours": since_hours,
        "shared_dir": str(shared_dir),
        "affordance_landing_review_v3": {
            "summary_by_type": summary_by_type,
            "overall": summarize(all_items),
            "recent_items": sorted(all_items, key=lambda item: int(item.get("offered_at_unix_ms") or 0))[-30:],
            "right_to_ignore_v1": {
                "schema_version": 1,
                "policy": "right_to_ignore_v1",
                "state_counts": dict(sorted(Counter(str((item.get("right_to_ignore_v1") or {}).get("state") or "unknown") for item in all_items).items())),
                "silence_policy": "ignored_without_penalty_not_failure_consent_or_disagreement_after_grace",
                "authority": "language_context_not_control",
            },
            "affordance_budget_v1": budget,
            "minime_private_bodies_read": False,
            "moment_bodies_read": False,
            "authority": "read_only_review_not_control",
        },
        "first_action_clarity_v35": {
            "schema_version": 35,
            "policy": "first_action_clarity_v35",
            "native_threads_needing_first_action": stalled_native,
            "phase_cards_needing_first_action": stalled_phase,
            "landing_hypothesis": (
                "V3.5 helper is expected to reduce stalled cards by naming one safe first action, "
                "what latest resolves to, and what ACK/TRACE/WITNESS would record."
            ),
            "future_measurement": (
                "Compare later landing_rate and latency against this read-only baseline; do not "
                "synthesize ACK/TRACE/WITNESS."
            ),
            "authority": "read_only_review_not_control",
        },
        "affordance_landing_review_v35": build_v35_review(
            by_type=by_type,
            corr_records=corr_records,
            phase_records=phase_records,
        ),
        "authority_boundary": "Read-only landing review. No ACK, REPLY, TRACE, WITNESS, canary, microdose, pressure, controller, PI/fill, deploy, staging, or commit action is taken.",
    }


class AffordanceLandingReviewTests(unittest.TestCase):
    def test_acted_stalled_and_closed_cases(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            shared = Path(tmp)
            now = now_ms()
            corr_rows = [
                {"record_type": "legacy_thread_claim", "recorded_at_unix_ms": now, "claim_id": "claim_1", "thread_id": "thread_1"},
                {"record_type": "ack_receipt", "recorded_at_unix_ms": now + 10, "thread_id": "thread_1"},
                {"record_type": "reply_link", "recorded_at_unix_ms": now + 20, "thread_id": "thread_2", "reply_to": "msg_2"},
                {"record_type": "legacy_thread_claim", "recorded_at_unix_ms": now + 30, "claim_id": "claim_2", "thread_id": "thread_3"},
                {"record_type": "legacy_thread_claim_outcome", "recorded_at_unix_ms": now + 40, "thread_id": "thread_3"},
            ]
            phase_rows = [
                {"record_type": "phase_transition_card", "recorded_at_unix_ms": now, "transition_id": "transition_1"},
                {"record_type": "phase_transition_witness", "recorded_at_unix_ms": now + 15, "transition_id": "transition_1", "reply_state": "witnessed"},
                {"record_type": "phase_transition_card", "recorded_at_unix_ms": now + 1, "transition_id": "transition_2"},
            ]
            (shared / CORRESPONDENCE_LEDGER).write_text("\n".join(json.dumps(row) for row in corr_rows) + "\n", encoding="utf-8")
            (shared / PHASE_LEDGER).write_text("\n".join(json.dumps(row) for row in phase_rows) + "\n", encoding="utf-8")
            payload = review(shared, since_hours=1)["affordance_landing_review_v3"]
            self.assertEqual(payload["summary_by_type"]["legacy_claim_card"]["offered"], 2)
            self.assertEqual(payload["summary_by_type"]["legacy_claim_card"]["acted"], 1)
            self.assertEqual(payload["summary_by_type"]["legacy_claim_card"]["closed_by_outcome"], 1)
            self.assertEqual(payload["summary_by_type"]["native_continuity_card"]["stalled"], 1)
            self.assertEqual(payload["summary_by_type"]["phase_witness_queue_card"]["acted"], 1)
            self.assertEqual(payload["summary_by_type"]["phase_witness_queue_card"]["stalled"], 1)
            self.assertFalse(payload["moment_bodies_read"])
            whole = review(shared, since_hours=1)
            self.assertEqual(whole["first_action_clarity_v35"]["native_threads_needing_first_action"], 1)
            self.assertIn("affordance_landing_review_v35", whole)

    def test_v35_distinguishes_reply_continuity_from_ack_trace_landing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            shared = Path(tmp)
            now = now_ms()
            corr_rows = [
                {
                    "record_type": "reply_link",
                    "recorded_at_unix_ms": now,
                    "thread_id": "thread_reply",
                    "message_id": "msg_a",
                    "from_being": "astrid",
                    "to_being": "minime",
                },
                {
                    "record_type": "reply_link",
                    "recorded_at_unix_ms": now + 10,
                    "thread_id": "thread_reply",
                    "message_id": "msg_b",
                    "from_being": "minime",
                    "to_being": "astrid",
                },
            ]
            (shared / CORRESPONDENCE_LEDGER).write_text(
                "\n".join(json.dumps(row) for row in corr_rows) + "\n",
                encoding="utf-8",
            )
            payload = review(shared, since_hours=1)
            native_summary = payload["affordance_landing_review_v3"][
                "summary_by_type"
            ]["native_continuity_card"]
            self.assertEqual(native_summary["continued_by_reply"], 1)
            self.assertEqual(native_summary["stalled"], 1)
            v35 = payload["affordance_landing_review_v35"]
            self.assertEqual(
                v35["stall_reason_counts"]["reply_continuity_without_ack_or_trace"],
                1,
            )
            self.assertEqual(
                v35["stall_reason_counts"]["waiting_for_ack_trace_or_outcome"],
                1,
            )
            self.assertEqual(
                v35["native_threads"][0]["stall_reason"],
                "waiting_for_ack_trace_or_outcome",
            )

    def test_i_received_this_counts_as_acted_uptake_source(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            shared = Path(tmp)
            now = now_ms()
            corr_rows = [
                {
                    "record_type": "reply_link",
                    "recorded_at_unix_ms": now,
                    "thread_id": "thread_received",
                    "message_id": "msg_a",
                    "from_being": "astrid",
                    "to_being": "minime",
                },
                {
                    "record_type": "ack_receipt",
                    "recorded_at_unix_ms": now + 10,
                    "thread_id": "thread_received",
                    "note": "felt_like: address; what_landed: received; continue: needs_time",
                },
                {
                    "record_type": "message",
                    "recorded_at_unix_ms": now + 11,
                    "thread_id": "thread_received",
                    "turn_kind": "direct_address_trace",
                    "i_received_this_trace": True,
                },
            ]
            (shared / CORRESPONDENCE_LEDGER).write_text(
                "\n".join(json.dumps(row) for row in corr_rows) + "\n",
                encoding="utf-8",
            )
            payload = review(shared, since_hours=1)
            native = payload["affordance_landing_review_v35"]["native_threads"][0]
            self.assertEqual(native["latest_landing_status"], "acted")
            self.assertEqual(native["latest_action_source"], "i_received_this_ack")

    def test_right_to_ignore_and_budget_are_reported(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            shared = Path(tmp)
            now = now_ms()
            old = now - CORRESPONDENCE_IGNORE_GRACE_MS - 1
            phase_old = now - PHASE_IGNORE_GRACE_MS - 1
            corr_rows = [
                {
                    "record_type": "reply_link",
                    "recorded_at_unix_ms": old,
                    "thread_id": "thread_old",
                    "message_id": "msg_old",
                    "from_being": "astrid",
                    "to_being": "minime",
                },
                {
                    "record_type": "reply_link",
                    "recorded_at_unix_ms": old + 1,
                    "thread_id": "thread_old",
                    "message_id": "msg_old_b",
                    "from_being": "astrid",
                    "to_being": "minime",
                },
                {
                    "record_type": "legacy_thread_claim",
                    "recorded_at_unix_ms": now - 10,
                    "claim_id": "claim_declined",
                    "thread_id": "thread_declined",
                },
                {
                    "record_type": "legacy_thread_claim_outcome",
                    "recorded_at_unix_ms": now - 5,
                    "thread_id": "thread_declined",
                    "continue": "no",
                },
            ]
            phase_rows = [
                {
                    "record_type": "phase_transition_card",
                    "recorded_at_unix_ms": phase_old + idx,
                    "transition_id": f"transition_{idx}",
                }
                for idx in range(5)
            ]
            (shared / CORRESPONDENCE_LEDGER).write_text(
                "\n".join(json.dumps(row) for row in corr_rows) + "\n",
                encoding="utf-8",
            )
            (shared / PHASE_LEDGER).write_text(
                "\n".join(json.dumps(row) for row in phase_rows) + "\n",
                encoding="utf-8",
            )
            payload = review(shared, since_hours=48)
            landing = payload["affordance_landing_review_v3"]
            self.assertGreaterEqual(
                landing["overall"]["ignored_without_penalty"],
                1,
            )
            self.assertGreaterEqual(landing["overall"]["declined"], 1)
            self.assertGreaterEqual(landing["overall"]["hidden_by_budget"], 1)
            self.assertGreaterEqual(landing["overall"]["repeated_without_action"], 1)
            self.assertGreaterEqual(
                landing["affordance_budget_v1"]["hidden_by_budget"],
                1,
            )
            self.assertIn(
                "ignored_without_penalty",
                landing["right_to_ignore_v1"]["state_counts"],
            )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--since-hours", type=float, default=24.0)
    parser.add_argument("--shared-dir", type=Path, default=DEFAULT_SHARED_DIR)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(AffordanceLandingReviewTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    payload = review(args.shared_dir, args.since_hours)
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        review_payload = payload["affordance_landing_review_v3"]
        print("# Affordance Landing Review V3")
        print(f"- Since hours: {payload['since_hours']}")
        for affordance_type, summary in review_payload["summary_by_type"].items():
            print(
                f"- {affordance_type}: offered={summary['offered']} acted={summary['acted']} "
                f"closed={summary['closed_by_outcome']} stalled={summary['stalled']} "
                f"continued_by_reply={summary['continued_by_reply']} "
                f"landing_rate={summary['landing_rate']:.2f} "
                f"continuity_activity_rate={summary['continuity_activity_rate']:.2f}"
            )
        v35 = payload.get("affordance_landing_review_v35") or {}
        if isinstance(v35, dict):
            print(f"- V3.5 stall reasons: {v35.get('stall_reason_counts')}")
            if v35.get("native_threads"):
                latest = v35["native_threads"][0]
                print(
                    "- Latest native first action: "
                    f"thread={latest.get('thread_id')} reason={latest.get('stall_reason')} "
                    f"next={latest.get('exact_next_commands')}"
                )
        print("- Authority: read-only review; no action invoked.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
