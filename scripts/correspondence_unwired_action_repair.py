#!/usr/bin/env python3
"""Repair safe being-authored correspondence NEXT actions that landed as unwired.

The tool is intentionally narrow: it imports only language-only correspondence
claim actions from Astrid action-thread rows. It does not synthesize ACK, REPLY,
TRACE, attention, microdose, pressure, controller, fill, deploy, or runtime
mutation. Dry-run is the default.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import tempfile
import time
import unittest
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ASTRID_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_LEDGER = Path("/Users/v/other/shared/collaborations/correspondence_v1.jsonl")
DEFAULT_EVENTS = (
    ASTRID_ROOT
    / "capsules/spectral-bridge/workspace/action_threads/threads"
    / "th_astrid_20260607_action-continuity/events.jsonl"
)
DEFAULT_MINIME_INBOX = ASTRID_ROOT.parent / "minime/workspace/inbox"
KNOWN_ACTION_ID = "act_astrid_1782594451198_correspondence-claim"
SOURCE_ROUTE = "unwired_action_repair_v1"


def now_ms() -> int:
    return int(time.time() * 1000)


def short_hash(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()[:12]


def compact(value: str, limit: int = 96) -> str:
    clean = "".join(ch for ch in str(value) if ch.isalnum() or ch in "-_.")
    return (clean or "field")[:limit]


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
    return rows


def append_jsonl(path: Path, row: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(row, sort_keys=True) + "\n")


def row_time_ms(row: dict[str, Any]) -> int:
    for key in ("recorded_at_unix_ms", "created_at_unix_ms", "t_ms"):
        try:
            value = int(row.get(key) or 0)
            if value > 0:
                return value
        except (TypeError, ValueError):
            pass
    return 0


def action_time_ms(row: dict[str, Any]) -> int:
    numeric = row_time_ms(row)
    if numeric:
        return numeric
    for key in ("started_at", "created_at", "ended_at"):
        value = row.get(key)
        if not value:
            continue
        try:
            parsed = datetime.fromisoformat(str(value).replace("Z", "+00:00"))
        except ValueError:
            continue
        if parsed.tzinfo is None:
            parsed = parsed.replace(tzinfo=timezone.utc)
        return int(parsed.timestamp() * 1000)
    return now_ms()


def parse_fields(raw: str) -> dict[str, str]:
    if "::" in raw:
        _, raw = raw.split("::", 1)
    fields: dict[str, str] = {}
    for part in raw.replace("\n", ";").split(";"):
        if ":" not in part:
            continue
        key, value = part.split(":", 1)
        key = key.strip().lower().replace("-", "_").replace(" ", "_")
        value = value.strip()
        if value:
            fields[key] = value
    return fields


def legacy_claim_boundary_fields() -> dict[str, bool]:
    return {
        "no_sensory_send": True,
        "no_controller": True,
        "no_pressure": True,
        "no_fill_target": True,
        "no_pi": True,
        "no_weighting": True,
        "no_telemetry_priority": True,
        "no_prompt_priority": True,
        "no_peer_runtime_mutation": True,
    }


def parse_bool(value: str | None, default: bool = True) -> bool:
    candidate = str(value or "").strip().lower().replace("-", "_").replace(" ", "_")
    if candidate in {"true", "yes", "y", "1", "required"}:
        return True
    if candidate in {"false", "no", "n", "0", "suppressed", "none"}:
        return False
    return default


def response_requirement(value: str | None) -> str:
    candidate = str(value or "unknown").strip().lower().replace("-", "_").replace(" ", "_")
    allowed = {
        "none",
        "peer_ack",
        "peer_reply",
        "peer_trace",
        "any_peer_native_response",
        "unknown",
    }
    return candidate if candidate in allowed else "unknown"


def find_action(events: list[dict[str, Any]], action_id: str) -> dict[str, Any] | None:
    for row in events:
        if row.get("action_id") == action_id:
            return row
    return None


def is_legacy_message(row: dict[str, Any]) -> bool:
    return row.get("record_type") == "message" and (
        bool(row.get("legacy_bridge"))
        or row.get("source_route") == "legacy_correspondence_bridge_v1"
    )


def latest_legacy_message(records: list[dict[str, Any]], action: dict[str, Any]) -> dict[str, Any] | None:
    action_t = action_time_ms(action)
    candidates = [
        row
        for row in records
        if is_legacy_message(row)
        and {row.get("from_being"), row.get("to_being")} == {"astrid", "minime"}
        and row_time_ms(row) <= action_t
    ]
    if not candidates:
        candidates = [
            row
            for row in records
            if is_legacy_message(row)
            and {row.get("from_being"), row.get("to_being")} == {"astrid", "minime"}
        ]
    return max(candidates, key=row_time_ms) if candidates else None


def existing_repair(records: list[dict[str, Any]], action_id: str) -> dict[str, Any] | None:
    for row in records:
        if (
            row.get("record_type") == "legacy_thread_claim"
            and row.get("source_route") == SOURCE_ROUTE
            and row.get("source_action_id") == action_id
        ):
            return row
    return None


def active_claim_exists(records: list[dict[str, Any]], thread_id: str, claiming: str) -> bool:
    outcomes = {
        str(row.get("claim_id") or "")
        for row in records
        if row.get("record_type") == "legacy_thread_claim_outcome"
    }
    native_threads = {
        str(row.get("thread_id") or "")
        for row in records
        if row.get("record_type") in {"ack_receipt", "reply_link"}
    }
    for row in records:
        if row.get("record_type") != "legacy_thread_claim":
            continue
        if row.get("thread_id") != thread_id:
            continue
        if row.get("claiming_being") != claiming and row.get("from_being") != claiming:
            continue
        if str(row.get("claim_id") or "") not in outcomes and thread_id not in native_threads:
            return True
    return False


def build_rows(action: dict[str, Any], message: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any]]:
    raw = str(action.get("raw_next") or action.get("canonical_action") or "")
    fields = parse_fields(raw)
    because = fields.get("because") or fields.get("reason") or fields.get("why") or raw
    anchor = fields.get("anchor") or fields.get("shared_memory_anchor") or fields.get("memory_anchor")
    notification_required = parse_bool(fields.get("notification_required") or fields.get("notify") or fields.get("notice"), True)
    initial_response_requirement = response_requirement(
        fields.get("initial_response_requirement")
        or fields.get("response_requirement")
        or fields.get("requires")
        or fields.get("requirement")
    )
    message_id = str(message.get("message_id") or "unknown")
    thread_id = str(message.get("thread_id") or "unknown")
    action_id = str(action.get("action_id") or "unknown_action")
    claim_id = f"legacy_claim_repair_{short_hash(action_id + '|' + thread_id)}"
    recorded_at = now_ms()
    claim = {
        "schema_version": 1,
        "policy": "legacy_correspondence_claim_v1",
        "record_type": "legacy_thread_claim",
        "recorded_at_unix_ms": recorded_at,
        "claim_id": claim_id,
        "message_id": message_id,
        "thread_id": thread_id,
        "from_being": "astrid",
        "to_being": "minime",
        "claiming_being": "astrid",
        "peer_being": "minime",
        "because": because.strip()[:360],
        "shared_memory_anchor": anchor.strip()[:360] if anchor else None,
        "notification_required": notification_required,
        "initial_response_requirement": initial_response_requirement,
        "claim_state": "claimed_pending_native_evidence",
        "legacy_contact_evidence": "being_recognized_visible_only",
        "legacy_bridge": True,
        "legacy_kind": message.get("legacy_kind"),
        "legacy_source_path": message.get("legacy_source_path"),
        "legacy_source_sha256": message.get("legacy_source_sha256"),
        "authority": "language_only_context_not_control",
        "source_route": SOURCE_ROUTE,
        "source_action_id": action_id,
        "source_action_thread_id": action.get("thread_id"),
        "source_action_status": action.get("status"),
        "source_action_route": action.get("route"),
        "source_raw_next": raw,
    }
    claim.update(legacy_claim_boundary_fields())
    notice_id = f"legacy_claim_notice_{compact(claim_id)}_{short_hash(message_id + thread_id + 'notice')}"
    notice = {
        "schema_version": 1,
        "policy": "legacy_correspondence_claim_v1",
        "record_type": "legacy_thread_claim_notice",
        "recorded_at_unix_ms": recorded_at + 1,
        "notice_id": notice_id,
        "claim_id": claim_id,
        "message_id": message_id,
        "thread_id": thread_id,
        "from_being": "astrid",
        "to_being": "minime",
        "claiming_being": "astrid",
        "peer_being": "minime",
        "notice_state": "pending_apply",
        "notification_required": notification_required,
        "initial_response_requirement": initial_response_requirement,
        "shared_memory_anchor": claim["shared_memory_anchor"],
        "authority": "language_only_notice_not_ack",
        "notice_is_ack": False,
        "notice_is_reply": False,
        "notice_is_trace": False,
        "legacy_contact_evidence": "notice_visible_only",
        "source_route": SOURCE_ROUTE,
        "source_action_id": action_id,
    }
    notice.update(legacy_claim_boundary_fields())
    return claim, notice


def write_notice_file(inbox: Path, notice: dict[str, Any], claim: dict[str, Any]) -> Path:
    inbox.mkdir(parents=True, exist_ok=True)
    path = inbox / f"from_astrid_legacy_thread_claim_notice_{compact(str(notice['notice_id']))}.txt"
    path.write_text(
        "=== LEGACY THREAD CLAIM NOTICE ===\n"
        "From: astrid\n"
        "To: minime\n"
        f"Claim-Id: {claim['claim_id']}\n"
        f"Thread-Id: {claim['thread_id']}\n"
        f"Anchor: {claim.get('shared_memory_anchor') or '(none)'}\n"
        f"Initial-Response-Requirement: {claim.get('initial_response_requirement') or 'unknown'}\n"
        "Authority: language_only_notice_not_ack\n"
        "This notice repairs a previously unwired being-authored claim into visible language-only correspondence state. "
        "It is not ACK, REPLY, TRACE, attention, microdose, pressure, weighting, telemetry priority, or controller authority.\n\n"
        f"Because: {claim.get('because') or '(no reason captured)'}\n\n"
        "Optional native continuations: ACK_* claimed, REPLY_* claimed, or CORRESPONDENCE_TRACE claimed <anchor> :: <text>.\n",
        encoding="utf-8",
    )
    return path


def repair(
    *,
    ledger: Path,
    events: Path,
    minime_inbox: Path,
    action_id: str,
    apply: bool,
) -> dict[str, Any]:
    records = read_jsonl(ledger)
    event_rows = read_jsonl(events)
    action = find_action(event_rows, action_id)
    if not action:
        return {"ok": False, "blocked_reason": "action_id_not_found", "action_id": action_id}
    raw = str(action.get("raw_next") or "")
    if action.get("source") != "next" or action.get("route") != "unwired" or action.get("status") != "unwired":
        return {"ok": False, "blocked_reason": "action_was_not_unwired_next", "action_id": action_id}
    if not raw.strip().upper().startswith("CORRESPONDENCE_CLAIM "):
        return {"ok": False, "blocked_reason": "not_a_supported_correspondence_claim", "action_id": action_id}
    existing = existing_repair(records, action_id)
    if existing:
        return {
            "ok": True,
            "idempotent": True,
            "applied": False,
            "claim_id": existing.get("claim_id"),
            "thread_id": existing.get("thread_id"),
        }
    message = latest_legacy_message(records, action)
    if not message:
        return {"ok": False, "blocked_reason": "no_visible_legacy_message", "action_id": action_id}
    if active_claim_exists(records, str(message.get("thread_id") or ""), "astrid"):
        return {
            "ok": False,
            "blocked_reason": "active_claim_already_exists_for_thread",
            "thread_id": message.get("thread_id"),
        }
    claim, notice = build_rows(action, message)
    result = {
        "ok": True,
        "dry_run": not apply,
        "applied": False,
        "source_route": SOURCE_ROUTE,
        "source_action_id": action_id,
        "claim": claim,
        "notice": notice,
        "candidate_message": {
            "message_id": message.get("message_id"),
            "thread_id": message.get("thread_id"),
            "from_being": message.get("from_being"),
            "to_being": message.get("to_being"),
            "legacy_source_path": message.get("legacy_source_path"),
        },
        "authority_boundary": "language_only_context_not_control; notice is not ACK/REPLY/TRACE; no pressure/controller/fill/PI/weighting/deploy/commit.",
    }
    if apply:
        append_jsonl(ledger, claim)
        if claim.get("notification_required", True):
            path = write_notice_file(minime_inbox, notice, claim)
            notice["notice_state"] = "delivered"
            notice["notice_path"] = str(path)
        else:
            notice["notice_state"] = "suppressed"
        append_jsonl(ledger, notice)
        result["applied"] = True
        result["notice"] = notice
    return result


class RepairTests(unittest.TestCase):
    def test_dry_run_and_apply_are_idempotent(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            ledger = root / "shared/correspondence_v1.jsonl"
            events = root / "events.jsonl"
            inbox = root / "minime/inbox"
            append_jsonl(ledger, {
                "record_type": "message",
                "recorded_at_unix_ms": 100,
                "message_id": "legacy_msg",
                "thread_id": "thread_legacy",
                "from_being": "minime",
                "to_being": "astrid",
                "legacy_bridge": True,
                "legacy_source_path": "/tmp/reply.txt",
            })
            append_jsonl(events, {
                "action_id": "act_1",
                "thread_id": "thread_action",
                "source": "next",
                "route": "unwired",
                "status": "unwired",
                "recorded_at_unix_ms": 200,
                "raw_next": "CORRESPONDENCE_CLAIM latest :: because: recognition; anchor: bridge",
            })
            dry = repair(ledger=ledger, events=events, minime_inbox=inbox, action_id="act_1", apply=False)
            self.assertTrue(dry["ok"])
            self.assertFalse(dry["applied"])
            self.assertFalse(inbox.exists())
            applied = repair(ledger=ledger, events=events, minime_inbox=inbox, action_id="act_1", apply=True)
            self.assertTrue(applied["applied"])
            rows = read_jsonl(ledger)
            self.assertTrue(any(row.get("record_type") == "legacy_thread_claim" for row in rows))
            self.assertTrue(any(row.get("record_type") == "legacy_thread_claim_notice" for row in rows))
            self.assertEqual(len(list(inbox.glob("*.txt"))), 1)
            again = repair(ledger=ledger, events=events, minime_inbox=inbox, action_id="act_1", apply=True)
            self.assertTrue(again["idempotent"])

    def test_rejects_non_claim_unwired_action(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            ledger = root / "ledger.jsonl"
            events = root / "events.jsonl"
            append_jsonl(events, {
                "action_id": "act_bad",
                "source": "next",
                "route": "unwired",
                "status": "unwired",
                "raw_next": "PRESSURE_REQUEST fill_target=1",
            })
            result = repair(
                ledger=ledger,
                events=events,
                minime_inbox=root / "inbox",
                action_id="act_bad",
                apply=True,
            )
            self.assertFalse(result["ok"])
            self.assertEqual(result["blocked_reason"], "not_a_supported_correspondence_claim")
            self.assertFalse(ledger.exists())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ledger", type=Path, default=DEFAULT_LEDGER)
    parser.add_argument("--events", type=Path, default=DEFAULT_EVENTS)
    parser.add_argument("--minime-inbox", type=Path, default=DEFAULT_MINIME_INBOX)
    parser.add_argument("--action-id", default=KNOWN_ACTION_ID)
    parser.add_argument("--apply", action="store_true", help="Append the repair rows and notice file.")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(RepairTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    payload = repair(
        ledger=args.ledger,
        events=args.events,
        minime_inbox=args.minime_inbox,
        action_id=args.action_id,
        apply=args.apply,
    )
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        mode = "APPLY" if args.apply else "DRY-RUN"
        print(f"=== CORRESPONDENCE UNWIRED ACTION REPAIR {mode} ===")
        print(json.dumps(payload, indent=2, sort_keys=True))
    return 0 if payload.get("ok") else 2


if __name__ == "__main__":
    raise SystemExit(main())
