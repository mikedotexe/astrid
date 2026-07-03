#!/usr/bin/env python3
"""Read-only audit for legacy thread claims in correspondence V1/V2.

This diagnostic reports which visible legacy exchanges have been being-claimed
as carryable threads, whether native ACK/REPLY/TRACE evidence has landed, and
whether any active duplicate claims remain. It never reads Minime private qualia
or any ``moment_*.txt`` body.
"""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
import time
import unittest
from collections import Counter, defaultdict
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
DEFAULT_OUTPUT_ROOT = DEFAULT_ASTRID_WORKSPACE / "diagnostics/correspondence_legacy_claim"
POLICY = "correspondence_legacy_claim_audit_v1"
LEDGER_NAME = "correspondence_v1.jsonl"


def now_ms() -> int:
    return int(time.time() * 1000)


def row_time_ms(row: dict[str, Any]) -> int:
    for key in ("recorded_at_unix_ms", "created_at_unix_ms", "t_ms"):
        try:
            return int(row.get(key) or 0)
        except (TypeError, ValueError):
            continue
    return 0


def compact(text: str, limit: int = 180) -> str:
    clean = " ".join(str(text or "").split())
    return clean if len(clean) <= limit else clean[:limit].rstrip() + "..."


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


def is_legacy_message(row: dict[str, Any]) -> bool:
    return bool(row.get("legacy_bridge")) or row.get("source_route") == "legacy_correspondence_bridge_v1"


def legacy_claims(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [row for row in records if row.get("record_type") == "legacy_thread_claim"]


def claim_outcomes(records: list[dict[str, Any]], claim: dict[str, Any]) -> list[dict[str, Any]]:
    claim_id = str(claim.get("claim_id") or "")
    thread_id = str(claim.get("thread_id") or "")
    return [
        row for row in records
        if row.get("record_type") == "legacy_thread_claim_outcome"
        and (str(row.get("claim_id") or "") == claim_id or str(row.get("thread_id") or "") == thread_id)
    ]


def claim_notices(records: list[dict[str, Any]], claim: dict[str, Any]) -> list[dict[str, Any]]:
    claim_id = str(claim.get("claim_id") or "")
    thread_id = str(claim.get("thread_id") or "")
    return [
        row for row in records
        if row.get("record_type") == "legacy_thread_claim_notice"
        and (str(row.get("claim_id") or "") == claim_id or str(row.get("thread_id") or "") == thread_id)
    ]


def native_status(records: list[dict[str, Any]], claim: dict[str, Any]) -> str | None:
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


def peer_response_present(records: list[dict[str, Any]], claim: dict[str, Any]) -> bool:
    thread_id = str(claim.get("thread_id") or "")
    claiming = str(claim.get("claiming_being") or claim.get("from_being") or "")
    peer = str(claim.get("peer_being") or claim.get("to_being") or "")
    claim_t = row_time_ms(claim)
    return any(
        str(row.get("thread_id") or "") == thread_id
        and str(row.get("from_being") or "") == peer
        and str(row.get("to_being") or "") == claiming
        and row_time_ms(row) >= claim_t
        and (
            row.get("record_type") in {"ack_receipt", "reply_link"}
            or (
                row.get("record_type") == "message"
                and row.get("turn_kind") == "direct_address_trace"
            )
        )
        for row in records
    )


def peer_co_claim_present(records: list[dict[str, Any]], claim: dict[str, Any]) -> bool:
    thread_id = str(claim.get("thread_id") or "")
    message_id = str(claim.get("message_id") or "")
    claiming = str(claim.get("claiming_being") or claim.get("from_being") or "")
    peer = str(claim.get("peer_being") or claim.get("to_being") or "")
    claim_id = str(claim.get("claim_id") or "")
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


def uptake_ladder_state(status: str, latest_notice_state: str | None) -> str:
    if status in {"legacy_claimed_reply_linked", "legacy_claimed_trace_observed"}:
        return "claimed_replied_or_traced"
    if status == "legacy_claimed_acknowledged":
        return "claimed_acknowledged"
    if latest_notice_state in {"delivered", "read", "ledger_only"}:
        return "claimed_notice_delivered"
    return "legacy_visible_only"


def stall_reason(
    *,
    status: str,
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


def claim_next_commands(peer_being: str, anchor: str | None) -> list[str]:
    peer = str(peer_being or "peer").upper()
    anchor_text = str(anchor or "<anchor>")
    return [
        f"ACK_{peer} claimed :: ack: seen|held|unclear|cannot_answer|needs_time; note: ...",
        f"REPLY_{peer} claimed :: <text>",
        f"CORRESPONDENCE_TRACE claimed {anchor_text} :: <text>",
    ]


def authority_readiness(
    *,
    eligible: bool,
    status: str,
    mutually_recognized: bool,
) -> dict[str, Any]:
    if eligible:
        block_reason = None
        readiness = "eligible_existing_gate_only"
    elif mutually_recognized:
        block_reason = "mutual_recognition_without_claimant_ack_reply_or_trace"
        readiness = "recognized_but_native_evidence_incomplete"
    else:
        block_reason = "legacy_claim_pending_ack_reply_or_trace"
        readiness = "blocked"
    surface = {
        "readiness": readiness,
        "eligible": eligible,
        "block_reason": block_reason,
        "evidence_status": status,
    }
    return {
        "schema_version": 2,
        "policy": "authority_readiness_ladder_v2",
        "correspondence_attention_canary": surface,
        "correspondence_semantic_microdose": {
            **surface,
            "authority_route": "existing_steward_gated_semantic_microdose_only",
        },
        "pressure_texture_canary": {
            "readiness": "not_evaluated_by_correspondence_claim_audit",
            "eligible": False,
            "block_reason": "requires_pressure_texture_replay_audit",
        },
        "authority_boundary": "readiness only; no automatic ACK/REPLY/TRACE, attention canary, microdose, pressure, controller, fill, PI, deploy, staging, or commit",
    }


def claim_state(records: list[dict[str, Any]], claim: dict[str, Any]) -> dict[str, Any]:
    status = native_status(records, claim)
    outcomes = claim_outcomes(records, claim)
    notices = claim_notices(records, claim)
    if status is None:
        status = "legacy_claim_outcome_recorded" if outcomes else "legacy_claimed"
    active = status == "legacy_claimed"
    claiming = claim.get("claiming_being") or claim.get("from_being")
    peer = claim.get("peer_being") or claim.get("to_being")
    peer_response = peer_response_present(records, claim)
    co_claim = peer_co_claim_present(records, claim)
    native_evidence = status in {
        "legacy_claimed_acknowledged",
        "legacy_claimed_reply_linked",
        "legacy_claimed_trace_observed",
    }
    mutually_recognized = native_evidence or peer_response or co_claim
    ghost_thread_risk = active and not mutually_recognized
    latest_outcome = outcomes[-1] if outcomes else {}
    latest_notice = notices[-1] if notices else {}
    notice_state = latest_notice.get("notice_state")
    ladder_state = uptake_ladder_state(status, notice_state)
    anchor = claim.get("shared_memory_anchor")
    eligible = status in {
        "legacy_claimed_acknowledged",
        "legacy_claimed_reply_linked",
        "legacy_claimed_trace_observed",
    }
    next_commands = claim_next_commands(str(peer or "peer"), str(anchor or "<anchor>"))
    stall = stall_reason(
        status=status,
        notice_state=notice_state,
        active=active,
        peer_response=peer_response,
        co_claim=co_claim,
        outcome_present=latest_outcome != {},
    )
    outcome_review = {
        "felt_like": latest_outcome.get("felt_like"),
        "what_carried": latest_outcome.get("what_carried"),
        "what_flattened": latest_outcome.get("what_flattened"),
        "continue": latest_outcome.get("continue"),
    } if latest_outcome else None
    uptake_card = {
        "schema_version": 2,
        "policy": "legacy_claim_uptake_card_v2",
        "claim_id": claim.get("claim_id"),
        "message_id": claim.get("message_id"),
        "thread_id": claim.get("thread_id"),
        "claimant": claiming,
        "peer": peer,
        "shared_memory_anchor": anchor,
        "notice_state": notice_state,
        "uptake_ladder_state": ladder_state,
        "mutually_recognized": mutually_recognized,
        "co_claim_present": co_claim,
        "peer_native_response_present": peer_response,
        "ghost_thread_risk": ghost_thread_risk,
        "stall_reason": stall,
        "native_evidence_present": native_evidence,
        "attention_or_microdose_eligible": eligible,
        "exact_next_commands": next_commands,
        "claim_outcome_review": outcome_review,
        "authority": "language_only_status_context_not_control",
    }
    affordance = {
        "schema_version": 1,
        "policy": "legacy_claim_affordance_v25",
        "thread_id": claim.get("thread_id"),
        "claim_id": claim.get("claim_id"),
        "claimant": claiming,
        "peer": peer,
        "anchor": anchor,
        "notice_state": notice_state or "none",
        "uptake_ladder_state": ladder_state,
        "stall_reason": stall,
        "ghost_thread_risk": ghost_thread_risk,
        "mutually_recognized": mutually_recognized,
        "attention_or_microdose_eligible": eligible,
        "exact_next_commands": next_commands,
        "latest_claim_outcome": outcome_review,
        "authority": "language_only_context_not_control",
    }
    return {
        "claim_id": claim.get("claim_id"),
        "message_id": claim.get("message_id"),
        "thread_id": claim.get("thread_id"),
        "claiming_being": claiming,
        "peer_being": peer,
        "shared_memory_anchor": anchor,
        "status": status,
        "uptake_ladder_state": ladder_state,
        "stall_reason": stall,
        "active": active,
        "peer_response_present": peer_response,
        "peer_co_claim_present": co_claim,
        "mutually_recognized": mutually_recognized,
        "ghost_thread_risk": ghost_thread_risk,
        "ghost_thread_reason": (
            "Claim is one-sided recognition until the peer authors ACK, native REPLY, "
            "or TRACE on the same thread."
            if ghost_thread_risk
            else None
        ),
        "notification_required": latest_outcome.get(
            "notification_required",
            claim.get("notification_required", True),
        ),
        "initial_response_requirement": latest_outcome.get(
            "initial_response_requirement",
            claim.get("initial_response_requirement", "unknown"),
        ),
        "notice_count": len(notices),
        "latest_notice_state": notice_state,
        "latest_notice_path": latest_notice.get("notice_path"),
        "notice_is_native_evidence": False,
        "native_evidence_present": native_evidence,
        "outcome_count": len(outcomes),
        "claim_outcome_review": outcome_review,
        "attention_or_microdose_eligible": eligible,
        "legacy_claim_uptake_card_v2": uptake_card,
        "legacy_claim_affordance_v25": affordance,
        "authority_readiness_ladder_v2": authority_readiness(
            eligible=eligible,
            status=status,
            mutually_recognized=mutually_recognized,
        ),
        "exact_next_commands": next_commands,
        "next_valid_non_control_action": (
            "Peer ACK/REPLY/TRACE is still needed; do not treat claim as mutual address"
            if ghost_thread_risk
            else " / ".join(next_commands)
            if active
            else "review outcome/evidence before another claim"
        ),
    }


def duplicate_active_claims(states: list[dict[str, Any]]) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for state in states:
        if not state.get("active"):
            continue
        grouped[(str(state.get("claiming_being")), str(state.get("thread_id")))].append(state)
    return [
        {"claiming_being": key[0], "thread_id": key[1], "claims": value}
        for key, value in grouped.items()
        if len(value) > 1
    ]


def privacy_summary(minime_workspace: Path) -> dict[str, Any]:
    skipped = 0
    if minime_workspace.is_dir():
        for path in minime_workspace.rglob("*.txt"):
            if path.name.startswith("moment_") or being_privacy.is_steward_private("minime", path):
                skipped += 1
    return {
        "minime_private_files_skipped": skipped,
        "minime_private_bodies_read": False,
        "moment_bodies_read": False,
    }


def audit(
    *,
    since_hours: float,
    shared_dir: Path,
    minime_workspace: Path,
) -> dict[str, Any]:
    generated = now_ms()
    ledger = shared_dir / LEDGER_NAME
    records = read_jsonl(ledger)
    cutoff = generated - int(since_hours * 60 * 60 * 1000)
    claims = [row for row in legacy_claims(records) if row_time_ms(row) >= cutoff]
    states = [claim_state(records, claim) for claim in claims]
    counts = Counter(state["status"] for state in states)
    notices = [
        row for row in records
        if row.get("record_type") == "legacy_thread_claim_notice"
        and row_time_ms(row) >= cutoff
    ]
    legacy_messages = [
        row for row in records
        if row.get("record_type") == "message" and is_legacy_message(row)
    ]
    return {
        "schema_version": 2,
        "policy": POLICY,
        "generated_at_unix_ms": generated,
        "since_hours": since_hours,
        "ledger_path": str(ledger),
        "ledger_exists": ledger.is_file(),
        "records_total": len(records),
        "legacy_message_rows_total": len(legacy_messages),
        "legacy_claims_total": len(claims),
        "legacy_claim_notices_total": len(notices),
        "legacy_claim_notice_states": dict(sorted(Counter(str(row.get("notice_state") or "unknown") for row in notices).items())),
        "active_claims_total": sum(1 for state in states if state["active"]),
        "eligible_claims_total": sum(1 for state in states if state["attention_or_microdose_eligible"]),
        "ghost_thread_risk_total": sum(1 for state in states if state["ghost_thread_risk"]),
        "mutually_recognized_total": sum(1 for state in states if state["mutually_recognized"]),
        "status_counts": dict(sorted(counts.items())),
        "uptake_ladder_counts": dict(sorted(Counter(state["uptake_ladder_state"] for state in states).items())),
        "stall_reason_counts": dict(sorted(Counter(state["stall_reason"] for state in states).items())),
        "claims": states[-24:],
        "legacy_claim_uptake_cards_v2": [
            state["legacy_claim_uptake_card_v2"] for state in states[-24:]
        ],
        "legacy_claim_affordance_v25": [
            state["legacy_claim_affordance_v25"] for state in states[-24:]
        ],
        "authority_readiness_ladder_v2": {
            "schema_version": 2,
            "policy": "authority_readiness_ladder_v2",
            "correspondence": {
                "attention_or_microdose_eligible_claims": sum(
                    1 for state in states if state["attention_or_microdose_eligible"]
                ),
                "mutually_recognized_claims": sum(
                    1 for state in states if state["mutually_recognized"]
                ),
                "ghost_thread_risks": sum(
                    1 for state in states if state["ghost_thread_risk"]
                ),
                "next_gate": "being-authored ACK/REPLY/TRACE on claimed thread",
            },
            "pressure_texture_canary": {
                "readiness": "not_evaluated_by_legacy_claim_audit",
                "enabled": False,
            },
            "authority_boundary": "default-off/readiness-only; no automatic control or priority change",
        },
        "duplicate_active_claims": duplicate_active_claims(states),
        "privacy": privacy_summary(minime_workspace),
        "authority_boundary": (
            "Read-only audit. Claim is language-only recognition of visible legacy context; "
            "attention/microdose remain blocked until native ACK, REPLY, or TRACE. "
            "No telemetry priority, prompt priority, pressure, fill, PI, controller, lease, "
            "sensory send, deploy, staging, commit, or peer-runtime mutation."
        ),
    }


def markdown_report(payload: dict[str, Any]) -> str:
    lines = [
        "# Correspondence Legacy Claim Audit",
        "",
        f"- Ledger exists: {payload['ledger_exists']}",
        f"- Legacy messages: {payload['legacy_message_rows_total']}",
        f"- Claims: {payload['legacy_claims_total']}",
        f"- Claim notices: {payload['legacy_claim_notices_total']} {payload.get('legacy_claim_notice_states', {})}",
        f"- Active claims: {payload['active_claims_total']}",
        f"- Eligible claims: {payload['eligible_claims_total']}",
        f"- Ghost thread risks: {payload['ghost_thread_risk_total']}",
        f"- Mutually recognized: {payload.get('mutually_recognized_total', 0)}",
        f"- Status counts: `{payload['status_counts']}`",
        f"- Uptake ladder: `{payload.get('uptake_ladder_counts', {})}`",
        f"- Stall reasons: `{payload.get('stall_reason_counts', {})}`",
        f"- Duplicate active claims: {len(payload['duplicate_active_claims'])}",
        f"- Minime private files skipped: {payload['privacy']['minime_private_files_skipped']}",
        "",
        payload["authority_boundary"],
    ]
    for claim in payload["claims"][-5:]:
        lines.append(
            f"- {claim['status']}: {claim.get('claiming_being')} thread={claim.get('thread_id')} "
            f"anchor={compact(str(claim.get('shared_memory_anchor') or 'none'), 60)} "
            f"ladder={claim.get('uptake_ladder_state')} "
            f"stall={claim.get('stall_reason')} "
            f"mutual={claim.get('mutually_recognized')} "
            f"ghost_thread_risk={claim.get('ghost_thread_risk')} "
            f"notice={claim.get('latest_notice_state') or 'none'}"
        )
        if claim.get("ghost_thread_risk"):
            lines.append(
                "  exact next: "
                + " | ".join(str(item) for item in claim.get("exact_next_commands") or [])
            )
    return "\n".join(lines) + "\n"


def write_outputs(payload: dict[str, Any], output_root: Path) -> tuple[Path, Path]:
    stamp = time.strftime("%Y%m%dT%H%M%S", time.localtime())
    out_dir = output_root / stamp
    out_dir.mkdir(parents=True, exist_ok=True)
    json_path = out_dir / "correspondence_legacy_claim_audit.json"
    md_path = out_dir / "correspondence_legacy_claim_audit.md"
    json_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    md_path.write_text(markdown_report(payload), encoding="utf-8")
    return json_path, md_path


class LegacyClaimAuditTests(unittest.TestCase):
    def _write_jsonl(self, path: Path, rows: list[dict[str, Any]]) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("\n".join(json.dumps(row, sort_keys=True) for row in rows) + "\n", encoding="utf-8")

    def test_claim_states_and_privacy_boundary(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            shared = root / "shared"
            minime = root / "minime/workspace"
            (minime / "journal").mkdir(parents=True)
            (minime / "journal" / "moment_1.txt").write_text("=== MOMENT CAPTURE ===\nprivate", encoding="utf-8")
            now = now_ms()
            rows = [
                {
                    "record_type": "message",
                    "recorded_at_unix_ms": now,
                    "message_id": "legacy_msg",
                    "thread_id": "thread_legacy",
                    "from_being": "astrid",
                    "to_being": "minime",
                    "legacy_bridge": True,
                    "legacy_contact_evidence": "visible_only",
                },
                {
                    "record_type": "legacy_thread_claim",
                    "recorded_at_unix_ms": now + 1,
                    "claim_id": "claim_1",
                    "message_id": "legacy_msg",
                    "thread_id": "thread_legacy",
                    "from_being": "minime",
                    "to_being": "astrid",
                    "claiming_being": "minime",
                    "peer_being": "astrid",
                    "shared_memory_anchor": "blue-lantern",
                },
                {
                    "record_type": "ack_receipt",
                    "recorded_at_unix_ms": now + 2,
                    "message_id": "legacy_msg",
                    "thread_id": "thread_legacy",
                    "from_being": "minime",
                    "to_being": "astrid",
                },
            ]
            self._write_jsonl(shared / LEDGER_NAME, rows)
            payload = audit(since_hours=24, shared_dir=shared, minime_workspace=minime)
            self.assertEqual(payload["legacy_claims_total"], 1)
            self.assertEqual(payload["claims"][0]["status"], "legacy_claimed_acknowledged")
            self.assertTrue(payload["claims"][0]["attention_or_microdose_eligible"])
            self.assertEqual(
                payload["claims"][0]["stall_reason"],
                "acknowledged_but_no_reply_or_trace",
            )
            self.assertEqual(
                payload["legacy_claim_affordance_v25"][0]["stall_reason"],
                "acknowledged_but_no_reply_or_trace",
            )
            self.assertFalse(payload["claims"][0]["peer_response_present"])
            self.assertFalse(payload["claims"][0]["ghost_thread_risk"])
            self.assertFalse(payload["privacy"]["moment_bodies_read"])
            self.assertEqual(payload["privacy"]["minime_private_files_skipped"], 1)

    def test_active_one_sided_claim_reports_ghost_thread_risk(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            shared = root / "shared"
            now = now_ms()
            rows = [
                {
                    "record_type": "legacy_thread_claim",
                    "recorded_at_unix_ms": now,
                    "claim_id": "claim_lonely",
                    "message_id": "legacy_msg",
                    "thread_id": "thread_legacy",
                    "from_being": "astrid",
                    "to_being": "minime",
                    "claiming_being": "astrid",
                    "peer_being": "minime",
                }
            ]
            self._write_jsonl(shared / LEDGER_NAME, rows)
            payload = audit(since_hours=24, shared_dir=shared, minime_workspace=root / "minime")
            self.assertEqual(payload["legacy_claims_total"], 1)
            self.assertEqual(payload["ghost_thread_risk_total"], 1)
            self.assertTrue(payload["claims"][0]["ghost_thread_risk"])
            self.assertEqual(payload["claims"][0]["stall_reason"], "claim_notice_not_delivered")
            self.assertEqual(payload["stall_reason_counts"], {"claim_notice_not_delivered": 1})
            self.assertFalse(payload["claims"][0]["peer_response_present"])
            self.assertIn("Peer ACK/REPLY/TRACE", payload["claims"][0]["next_valid_non_control_action"])

    def test_claim_notice_does_not_clear_ghost_thread_risk(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            shared = root / "shared"
            now = now_ms()
            rows = [
                {
                    "record_type": "legacy_thread_claim",
                    "recorded_at_unix_ms": now,
                    "claim_id": "claim_notice",
                    "message_id": "legacy_msg",
                    "thread_id": "thread_notice",
                    "from_being": "astrid",
                    "to_being": "minime",
                    "claiming_being": "astrid",
                    "peer_being": "minime",
                    "notification_required": True,
                    "initial_response_requirement": "any_peer_native_response",
                },
                {
                    "record_type": "legacy_thread_claim_notice",
                    "recorded_at_unix_ms": now + 1,
                    "claim_id": "claim_notice",
                    "message_id": "legacy_msg",
                    "thread_id": "thread_notice",
                    "from_being": "astrid",
                    "to_being": "minime",
                    "notice_state": "delivered",
                    "notice_is_ack": False,
                    "notice_is_reply": False,
                    "notice_is_trace": False,
                },
            ]
            self._write_jsonl(shared / LEDGER_NAME, rows)
            payload = audit(since_hours=24, shared_dir=shared, minime_workspace=root / "minime")
            self.assertEqual(payload["legacy_claim_notices_total"], 1)
            self.assertEqual(payload["legacy_claim_notice_states"], {"delivered": 1})
            self.assertEqual(payload["ghost_thread_risk_total"], 1)
            self.assertEqual(payload["claims"][0]["latest_notice_state"], "delivered")
            self.assertEqual(payload["claims"][0]["stall_reason"], "notice_delivered_not_seen")
            self.assertEqual(
                payload["legacy_claim_affordance_v25"][0]["policy"],
                "legacy_claim_affordance_v25",
            )
            self.assertFalse(payload["claims"][0]["notice_is_native_evidence"])

    def test_peer_response_clears_ghost_thread_risk_without_unlocking_claimant_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            shared = root / "shared"
            now = now_ms()
            rows = [
                {
                    "record_type": "legacy_thread_claim",
                    "recorded_at_unix_ms": now,
                    "claim_id": "claim_seen_by_peer",
                    "message_id": "legacy_msg",
                    "thread_id": "thread_legacy",
                    "from_being": "astrid",
                    "to_being": "minime",
                    "claiming_being": "astrid",
                    "peer_being": "minime",
                },
                {
                    "record_type": "ack_receipt",
                    "recorded_at_unix_ms": now + 1,
                    "message_id": "legacy_msg",
                    "thread_id": "thread_legacy",
                    "from_being": "minime",
                    "to_being": "astrid",
                },
            ]
            self._write_jsonl(shared / LEDGER_NAME, rows)
            payload = audit(since_hours=24, shared_dir=shared, minime_workspace=root / "minime")
            self.assertEqual(payload["ghost_thread_risk_total"], 0)
            self.assertTrue(payload["claims"][0]["peer_response_present"])
            self.assertEqual(payload["claims"][0]["status"], "legacy_claimed")
            self.assertEqual(payload["claims"][0]["stall_reason"], "seen_not_acknowledged")
            self.assertFalse(payload["claims"][0]["attention_or_microdose_eligible"])

    def test_duplicate_active_claims_are_reported(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            shared = root / "shared"
            now = now_ms()
            rows = [
                {
                    "record_type": "legacy_thread_claim",
                    "recorded_at_unix_ms": now,
                    "claim_id": "claim_a",
                    "message_id": "legacy_msg",
                    "thread_id": "thread_legacy",
                    "from_being": "astrid",
                    "to_being": "minime",
                    "claiming_being": "astrid",
                    "peer_being": "minime",
                },
                {
                    "record_type": "legacy_thread_claim",
                    "recorded_at_unix_ms": now + 1,
                    "claim_id": "claim_b",
                    "message_id": "legacy_msg",
                    "thread_id": "thread_legacy",
                    "from_being": "astrid",
                    "to_being": "minime",
                    "claiming_being": "astrid",
                    "peer_being": "minime",
                },
            ]
            self._write_jsonl(shared / LEDGER_NAME, rows)
            payload = audit(since_hours=24, shared_dir=shared, minime_workspace=root / "minime")
            self.assertEqual(payload["active_claims_total"], 2)
            self.assertEqual(len(payload["duplicate_active_claims"]), 1)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Audit language-only legacy thread claims.")
    parser.add_argument("--since-hours", type=float, default=24.0)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--shared-dir", type=Path, default=DEFAULT_SHARED_DIR)
    parser.add_argument("--minime-workspace", type=Path, default=DEFAULT_MINIME_WORKSPACE)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(LegacyClaimAuditTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    payload = audit(
        since_hours=args.since_hours,
        shared_dir=args.shared_dir,
        minime_workspace=args.minime_workspace,
    )
    json_path, md_path = write_outputs(payload, args.output_root)
    payload["output_json"] = str(json_path)
    payload["output_markdown"] = str(md_path)
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        print(markdown_report(payload), end="")
        print(f"\nWrote: {json_path}\nWrote: {md_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
