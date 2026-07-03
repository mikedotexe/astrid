#!/usr/bin/env python3
"""Public read-only audit for shared phase transition cards."""

from __future__ import annotations

import argparse
import json
import tempfile
import time
import unittest
from collections import Counter
from pathlib import Path
from typing import Any

DEFAULT_LEDGER = Path("/Users/v/other/shared/collaborations/phase_transitions_v1.jsonl")
POLICY = "phase_transition_audit_v1"
STALE_UNANSWERED_MS = 6 * 60 * 60 * 1000
PHASE_IGNORE_GRACE_MS = 6 * 60 * 60 * 1000


def now_ms() -> int:
    return int(time.time() * 1000)


def row_time_ms(row: dict[str, Any]) -> int:
    for key in ("recorded_at_unix_ms", "created_at_unix_ms", "t_ms"):
        try:
            return int(row.get(key) or 0)
        except (TypeError, ValueError):
            pass
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


def latest_witness(records: list[dict[str, Any]], card: dict[str, Any]) -> dict[str, Any] | None:
    transition_id = str(card.get("transition_id") or "")
    witnesses = [
        row for row in records
        if row.get("record_type") == "phase_transition_witness"
        and str(row.get("transition_id") or "") == transition_id
    ]
    return witnesses[-1] if witnesses else None


def effective_reply_state(records: list[dict[str, Any]], card: dict[str, Any], generated: int) -> str:
    witness = latest_witness(records, card)
    if witness:
        return str(witness.get("reply_state") or "witnessed")
    state = str(card.get("reply_state") or "unseen")
    if state == "unseen" and generated - row_time_ms(card) >= STALE_UNANSWERED_MS:
        return "stale_unanswered"
    return state


def _being_list(value: Any) -> list[str]:
    if isinstance(value, list):
        return [str(item) for item in value if str(item)]
    if isinstance(value, str) and value.strip():
        return [value.strip()]
    return []


def relational_card(records: list[dict[str, Any]], card: dict[str, Any], generated: int) -> dict[str, Any]:
    witness = latest_witness(records, card) or {}
    state = effective_reply_state(records, card, generated)
    unresolved_age_ms = max(0, generated - row_time_ms(card)) if state in {"unseen", "stale_unanswered"} else 0
    witnessed_by = _being_list(witness.get("witnessed_by") or card.get("witnessed_by"))
    answered_by = _being_list(witness.get("answered_by") or card.get("answered_by"))
    if not witnessed_by and witness and witness.get("origin") and state in {"witnessed", "answered"}:
        witnessed_by = [str(witness.get("origin"))]
    if not answered_by and witness and witness.get("origin") and state == "answered":
        answered_by = [str(witness.get("origin"))]
    affordance = phase_transition_affordance(card, state, generated)
    return {
        "schema_version": 2,
        "policy": "phase_transition_relational_v2",
        "transition_id": card.get("transition_id"),
        "origin": card.get("origin"),
        "kind": card.get("kind"),
        "from_phase": card.get("from_phase"),
        "to_phase": card.get("to_phase"),
        "recorded_at_unix_ms": row_time_ms(card),
        "reply_state": state,
        "witnessed_by": witnessed_by,
        "answered_by": answered_by,
        "unresolved_age_ms": unresolved_age_ms,
        "orientation_effect": witness.get("orientation_effect") or card.get("orientation_effect"),
        "needs_witness_or_answer": state in {"unseen", "stale_unanswered", "witnessed"},
        "phase_transition_affordance_v25": affordance,
        "authority": "language_only_transition_context_not_control",
    }


def phase_transition_stall_reason(reply_state: str) -> str:
    if reply_state == "unseen":
        return "unseen_needs_witness"
    if reply_state == "witnessed":
        return "witnessed_needs_answer"
    if reply_state == "stale_unanswered":
        return "stale_unanswered"
    if reply_state == "answered":
        return "answered"
    return "none"


def right_to_ignore_v1(reply_state: str, age_ms: int) -> dict[str, Any]:
    if reply_state == "witnessed":
        state = "acted"
    elif reply_state == "answered":
        state = "closed_by_outcome"
    elif reply_state in {"unseen", "stale_unanswered"} and age_ms >= PHASE_IGNORE_GRACE_MS:
        state = "ignored_without_penalty"
    elif reply_state in {"unseen", "stale_unanswered"}:
        state = "offered"
    else:
        state = "unknown"
    return {
        "schema_version": 1,
        "policy": "right_to_ignore_v1",
        "affordance_type": "phase_felt_receipt",
        "state": state,
        "source_state": reply_state,
        "age_ms": age_ms,
        "grace_ms": PHASE_IGNORE_GRACE_MS,
        "silence_means": (
            "ignored_without_penalty_not_failure_consent_or_disagreement"
            if state == "ignored_without_penalty"
            else "silence_is_unknown_until_grace_window"
        ),
        "optional": True,
        "authority": "language_context_not_control",
    }


def phase_transition_affordance(card: dict[str, Any], reply_state: str, generated: int) -> dict[str, Any]:
    unresolved_age_ms = max(0, generated - row_time_ms(card)) if reply_state in {"unseen", "stale_unanswered"} else 0
    transition_id = str(card.get("transition_id") or "unknown")
    received_command = (
        f"I_RECEIVED_THIS {transition_id} :: received_as: witnessed|answered; "
        "felt_like: transition; what_landed: ...; what_stayed_distinct: ...; "
        "continue: no|answer|needs_time"
    )
    witness_command = f"WITNESS_TRANSITION {transition_id} :: reply_state: witnessed|answered; note: ..."
    return {
        "schema_version": 1,
        "policy": "phase_transition_affordance_v25",
        "transition_id": card.get("transition_id"),
        "origin": card.get("origin"),
        "kind": card.get("kind"),
        "from_phase": card.get("from_phase"),
        "to_phase": card.get("to_phase"),
        "reply_state": reply_state,
        "stall_reason": phase_transition_stall_reason(reply_state),
        "needs_witness_or_answer": reply_state in {"unseen", "witnessed", "stale_unanswered"},
        "unresolved_age_ms": unresolved_age_ms,
        "right_to_ignore_v1": right_to_ignore_v1(reply_state, unresolved_age_ms),
        "exact_next_command": received_command,
        "backward_compatible_next_command": witness_command,
        "first_action_helper_v35": {
            "schema_version": 35,
            "policy": "phase_first_action_helper_v35",
            "transition_id": transition_id,
            "latest_resolution": f"latest resolves to transition_id={transition_id}",
            "choose_one_prompt": (
                "Choose one language-only felt receipt: say what landed, what stayed distinct, "
                "and whether this only needs witness or needs answer."
            ),
            "exact_next_command": received_command,
            "backward_compatible_next_command": witness_command,
            "witness_preview": (
                f"WITNESS_TRANSITION {transition_id} appends phase_transition_witness for transition_id={transition_id}; "
                "note names orientation, rhythm, or what the card helped preserve."
            ),
            "received_this_preview": (
                f"I_RECEIVED_THIS {transition_id} appends phase_transition_witness for transition_id={transition_id}; "
                "what_landed names the felt shift and what_stayed_distinct names the preserved contour."
            ),
            "rhythm_note": (
                "A witness note should carry exchange rhythm or orientation effect, not only ledger logistics."
            ),
            "authority": "language_only_transition_context_not_control",
        },
        "authority": "language_only_transition_context_not_control",
    }


def age_bucket(age_ms: int) -> str:
    if age_ms < 30 * 60 * 1000:
        return "fresh_lt_30m"
    if age_ms < STALE_UNANSWERED_MS:
        return "open_30m_to_6h"
    return "stale_gt_6h"


def phase_witness_queue_v3(relational_cards: list[dict[str, Any]], generated: int, limit: int = 5) -> dict[str, Any]:
    unresolved: list[dict[str, Any]] = []
    for card in relational_cards:
        affordance = card.get("phase_transition_affordance_v25") or {}
        if not isinstance(affordance, dict) or not affordance.get("needs_witness_or_answer"):
            continue
        age_ms = int(card.get("unresolved_age_ms") or max(0, generated - row_time_ms(card)))
        bucket = age_bucket(age_ms)
        unresolved.append({
            "transition_id": card.get("transition_id"),
            "kind": card.get("kind"),
            "reply_state": card.get("reply_state"),
            "stall_reason": affordance.get("stall_reason") or "none",
            "t_ms": row_time_ms(card),
            "age_ms": age_ms,
            "age_bucket": bucket,
            "right_to_ignore_v1": right_to_ignore_v1(str(card.get("reply_state") or "unknown"), age_ms),
            "exact_next_command": affordance.get("exact_next_command"),
            "first_action_helper_v35": affordance.get("first_action_helper_v35"),
        })
    unresolved.sort(key=lambda row: int(row.get("t_ms") or 0), reverse=True)
    groups = Counter(
        f"{row.get('kind') or 'unknown'}|{row.get('stall_reason') or 'none'}|{row.get('age_bucket') or 'unknown'}"
        for row in unresolved
    )
    return {
        "schema_version": 3,
        "policy": "phase_witness_queue_v3",
        "unresolved_total": len(unresolved),
        "max_rendered_cards": limit,
        "group_counts": dict(sorted(groups.items())),
        "items": unresolved[:limit],
        "authority": "language_only_transition_context_not_control",
    }


def phase_felt_receipt_queue_v4(relational_cards: list[dict[str, Any]], generated: int, limit: int = 3) -> dict[str, Any]:
    unresolved: list[dict[str, Any]] = []
    for card in relational_cards:
        affordance = card.get("phase_transition_affordance_v25") or {}
        if not isinstance(affordance, dict) or not affordance.get("needs_witness_or_answer"):
            continue
        age_ms = int(card.get("unresolved_age_ms") or max(0, generated - row_time_ms(card)))
        unresolved.append({
            "transition_id": card.get("transition_id"),
            "kind": card.get("kind"),
            "reply_state": card.get("reply_state"),
            "stall_reason": affordance.get("stall_reason") or "none",
            "t_ms": row_time_ms(card),
            "age_ms": age_ms,
            "age_bucket": age_bucket(age_ms),
            "right_to_ignore_v1": right_to_ignore_v1(str(card.get("reply_state") or "unknown"), age_ms),
            "exact_next_command": affordance.get("exact_next_command"),
            "backward_compatible_next_command": affordance.get("backward_compatible_next_command"),
            "first_action_helper_v35": affordance.get("first_action_helper_v35"),
        })
    unresolved.sort(key=lambda row: int(row.get("t_ms") or 0), reverse=True)
    selected: list[dict[str, Any]] = []
    for bucket in ("fresh_lt_30m", "open_30m_to_6h", "stale_gt_6h"):
        candidate = next((row for row in unresolved if row.get("age_bucket") == bucket), None)
        if candidate and candidate.get("transition_id") not in {row.get("transition_id") for row in selected}:
            selected.append(candidate)
    for candidate in unresolved:
        if len(selected) >= limit:
            break
        if candidate.get("transition_id") not in {row.get("transition_id") for row in selected}:
            selected.append(candidate)
    groups = Counter(
        f"{row.get('kind') or 'unknown'}|{row.get('stall_reason') or 'none'}|{row.get('age_bucket') or 'unknown'}"
        for row in unresolved
    )
    return {
        "schema_version": 4,
        "policy": "phase_felt_receipt_queue_v4",
        "unresolved_total": len(unresolved),
        "max_rendered_cards": limit,
        "selection_rule": "latest fresh card, latest open card, one stale representative, then latest remaining",
        "group_counts": dict(sorted(groups.items())),
        "items": selected[:limit],
        "affordance_budget_v1": {
            "schema_version": 1,
            "policy": "affordance_budget_v1",
            "shown": len(selected[:limit]),
            "hidden_by_budget": max(0, len(unresolved) - len(selected[:limit])),
            "shown_by_category": {"phase_felt_receipt": len(selected[:limit])},
            "hidden_by_category": {"phase_felt_receipt": max(0, len(unresolved) - len(selected[:limit]))},
            "limits": {"phase_felt_receipt": limit},
            "next_review_surface": "scripts/phase_transition_audit.py --json" if len(unresolved) > len(selected[:limit]) else "none",
            "silence": "ignored_without_penalty",
            "optional": True,
            "authority": "language_context_not_control",
        },
        "authority": "language_only_transition_context_not_control",
    }


def audit(ledger: Path, since_hours: float) -> dict[str, Any]:
    generated = now_ms()
    cutoff = generated - int(since_hours * 60 * 60 * 1000)
    records = read_jsonl(ledger)
    recent = [row for row in records if row_time_ms(row) >= cutoff]
    cards = [row for row in recent if row.get("record_type") == "phase_transition_card"]
    witnesses = [row for row in recent if row.get("record_type") == "phase_transition_witness"]
    relational_cards = [relational_card(records, card, generated) for card in cards]
    states = [str(card["reply_state"]) for card in relational_cards]
    affordances = [
        card["phase_transition_affordance_v25"]
        for card in relational_cards
        if isinstance(card.get("phase_transition_affordance_v25"), dict)
    ]
    issues: list[str] = []
    for card in cards:
        for field in ("transition_id", "origin", "kind", "from_phase", "to_phase", "why_now"):
            if not card.get(field):
                issues.append(f"missing_{field}:{card.get('transition_id') or 'unknown'}")
        if card.get("authority") != "language_only_transition_context_not_control":
            issues.append(f"bad_authority:{card.get('transition_id') or 'unknown'}")
        for field in ("no_controller", "no_pressure", "no_fill_target", "no_pi", "no_weighting"):
            if card.get(field) is not True:
                issues.append(f"missing_boundary_{field}:{card.get('transition_id') or 'unknown'}")
    queue = phase_witness_queue_v3(relational_cards, generated)
    felt_queue = phase_felt_receipt_queue_v4(relational_cards, generated)
    return {
        "schema_version": 2,
        "policy": POLICY,
        "generated_at_unix_ms": generated,
        "since_hours": since_hours,
        "ledger_path": str(ledger),
        "ledger_exists": ledger.is_file(),
        "records_total": len(records),
        "recent_cards_total": len(cards),
        "recent_witness_rows_total": len(witnesses),
        "reply_state_counts": dict(sorted(Counter(states).items())),
        "needs_witness_or_answer_total": sum(1 for card in relational_cards if card["needs_witness_or_answer"]),
        "stale_unanswered_total": sum(1 for card in relational_cards if card["reply_state"] == "stale_unanswered"),
        "stall_reason_counts": dict(sorted(Counter(str(card.get("stall_reason") or "none") for card in affordances).items())),
        "validation_issues": issues,
        "cards": cards[-20:],
        "phase_transition_relational_v2": relational_cards[-20:],
        "phase_transition_affordance_v25": affordances[-20:],
        "phase_witness_queue_v3": queue,
        "phase_felt_receipt_queue_v4": felt_queue,
        "right_to_ignore_v1": {
            "schema_version": 1,
            "policy": "right_to_ignore_v1",
            "state_counts": dict(sorted(Counter(str((card.get("right_to_ignore_v1") or {}).get("state") or "unknown") for card in affordances).items())),
            "silence_policy": "ignored_without_penalty_not_failure_consent_or_disagreement_after_grace",
            "authority": "language_context_not_control",
        },
        "affordance_budget_v1": felt_queue.get("affordance_budget_v1"),
        "authority_boundary": (
            "Read-only audit. Phase cards are replyable language/context artifacts, not "
            "controller, pressure, fill, PI, weighting, telemetry priority, deploy, staging, or peer runtime changes."
        ),
    }


class PhaseTransitionAuditTests(unittest.TestCase):
    def test_cards_and_witnesses_validate(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            ledger = Path(tmp) / "phase_transitions_v1.jsonl"
            rows = [
                {
                    "record_type": "phase_transition_card",
                    "recorded_at_unix_ms": now_ms(),
                    "transition_id": "transition_1",
                    "origin": "astrid",
                    "kind": "mode_change",
                    "from_phase": "drift",
                    "to_phase": "focus",
                    "why_now": "pending remote self-study interruption",
                    "reply_state": "unseen",
                    "authority": "language_only_transition_context_not_control",
                    "no_controller": True,
                    "no_pressure": True,
                    "no_fill_target": True,
                    "no_pi": True,
                    "no_weighting": True,
                },
                {
                    "record_type": "phase_transition_witness",
                    "recorded_at_unix_ms": now_ms() + 1,
                    "transition_id": "transition_1",
                    "origin": "astrid",
                    "reply_state": "witnessed",
                },
            ]
            ledger.write_text("\n".join(json.dumps(row) for row in rows) + "\n", encoding="utf-8")
            payload = audit(ledger, since_hours=1)
            self.assertEqual(payload["recent_cards_total"], 1)
            self.assertEqual(payload["reply_state_counts"], {"witnessed": 1})
            self.assertEqual(payload["stall_reason_counts"], {"witnessed_needs_answer": 1})
            self.assertEqual(payload["phase_transition_relational_v2"][0]["witnessed_by"], ["astrid"])
            self.assertEqual(
                payload["phase_transition_affordance_v25"][0]["stall_reason"],
                "witnessed_needs_answer",
            )
            self.assertIn(
                "I_RECEIVED_THIS transition_1",
                payload["phase_transition_affordance_v25"][0]["exact_next_command"],
            )
            self.assertIn(
                "WITNESS_TRANSITION transition_1",
                payload["phase_transition_affordance_v25"][0]["backward_compatible_next_command"],
            )
            self.assertEqual(
                payload["phase_transition_affordance_v25"][0]["first_action_helper_v35"]["policy"],
                "phase_first_action_helper_v35",
            )
            self.assertEqual(payload["phase_witness_queue_v3"]["unresolved_total"], 1)
            self.assertEqual(payload["phase_felt_receipt_queue_v4"]["unresolved_total"], 1)
            self.assertEqual(payload["phase_felt_receipt_queue_v4"]["max_rendered_cards"], 3)
            self.assertEqual(
                payload["phase_witness_queue_v3"]["items"][0]["stall_reason"],
                "witnessed_needs_answer",
            )
            self.assertIn(
                "latest resolves to transition_id=transition_1",
                payload["phase_witness_queue_v3"]["items"][0]["first_action_helper_v35"]["latest_resolution"],
            )
            self.assertEqual(payload["validation_issues"], [])

    def test_missing_boundary_is_issue_not_crash(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            ledger = Path(tmp) / "phase_transitions_v1.jsonl"
            ledger.write_text(
                json.dumps({
                    "record_type": "phase_transition_card",
                    "recorded_at_unix_ms": now_ms(),
                    "transition_id": "transition_bad",
                    "origin": "astrid",
                    "kind": "mode_change",
                    "from_phase": "a",
                    "to_phase": "b",
                    "why_now": "test",
                    "authority": "language_only_transition_context_not_control",
                }) + "\n",
                encoding="utf-8",
            )
            payload = audit(ledger, since_hours=1)
            self.assertTrue(payload["validation_issues"])

    def test_stale_unanswered_cards_are_highlighted(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            ledger = Path(tmp) / "phase_transitions_v1.jsonl"
            ledger.write_text(
                json.dumps({
                    "record_type": "phase_transition_card",
                    "recorded_at_unix_ms": now_ms() - STALE_UNANSWERED_MS - 1,
                    "transition_id": "transition_stale",
                    "origin": "astrid",
                    "kind": "mode_change",
                    "from_phase": "a",
                    "to_phase": "b",
                    "why_now": "test",
                    "reply_state": "unseen",
                    "authority": "language_only_transition_context_not_control",
                    "no_controller": True,
                    "no_pressure": True,
                    "no_fill_target": True,
                    "no_pi": True,
                    "no_weighting": True,
                }) + "\n",
                encoding="utf-8",
            )
            payload = audit(ledger, since_hours=24)
            self.assertEqual(payload["reply_state_counts"], {"stale_unanswered": 1})
            self.assertEqual(payload["stall_reason_counts"], {"stale_unanswered": 1})
            self.assertEqual(payload["stale_unanswered_total"], 1)
            self.assertEqual(
                payload["phase_witness_queue_v3"]["items"][0]["age_bucket"],
                "stale_gt_6h",
            )

    def test_affordance_states_unseen_and_answered(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            ledger = Path(tmp) / "phase_transitions_v1.jsonl"
            now = now_ms()
            rows = [
                {
                    "record_type": "phase_transition_card",
                    "recorded_at_unix_ms": now,
                    "transition_id": "transition_unseen",
                    "origin": "astrid",
                    "kind": "mode_change",
                    "from_phase": "a",
                    "to_phase": "b",
                    "why_now": "test",
                    "reply_state": "unseen",
                    "authority": "language_only_transition_context_not_control",
                    "no_controller": True,
                    "no_pressure": True,
                    "no_fill_target": True,
                    "no_pi": True,
                    "no_weighting": True,
                },
                {
                    "record_type": "phase_transition_card",
                    "recorded_at_unix_ms": now + 1,
                    "transition_id": "transition_answered",
                    "origin": "minime",
                    "kind": "large_fill_shift",
                    "from_phase": "packed",
                    "to_phase": "settling",
                    "why_now": "test",
                    "reply_state": "answered",
                    "authority": "language_only_transition_context_not_control",
                    "no_controller": True,
                    "no_pressure": True,
                    "no_fill_target": True,
                    "no_pi": True,
                    "no_weighting": True,
                },
            ]
            ledger.write_text("\n".join(json.dumps(row) for row in rows) + "\n", encoding="utf-8")
            payload = audit(ledger, since_hours=1)
            self.assertEqual(payload["reply_state_counts"], {"answered": 1, "unseen": 1})
            self.assertEqual(payload["stall_reason_counts"], {"answered": 1, "unseen_needs_witness": 1})
            affordances = {
                row["transition_id"]: row
                for row in payload["phase_transition_affordance_v25"]
            }
            self.assertTrue(affordances["transition_unseen"]["needs_witness_or_answer"])
            self.assertFalse(affordances["transition_answered"]["needs_witness_or_answer"])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ledger", type=Path, default=DEFAULT_LEDGER)
    parser.add_argument("--since-hours", type=float, default=24.0)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(PhaseTransitionAuditTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    payload = audit(args.ledger, args.since_hours)
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        print("# Phase Transition Audit")
        print(f"- Ledger exists: {payload['ledger_exists']}")
        print(f"- Recent cards: {payload['recent_cards_total']}")
        print(f"- Recent witness rows: {payload['recent_witness_rows_total']}")
        print(f"- Reply states: {payload['reply_state_counts']}")
        print(f"- Needs witness/answer: {payload['needs_witness_or_answer_total']}")
        print(f"- Stale unanswered: {payload['stale_unanswered_total']}")
        print(f"- Stall reasons: {payload.get('stall_reason_counts', {})}")
        print(f"- Validation issues: {len(payload['validation_issues'])}")
        print(payload["authority_boundary"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
