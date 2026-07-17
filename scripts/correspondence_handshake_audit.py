#!/usr/bin/env python3
"""Public-only Mutual Acknowledgement Handshake V1 audit.

Reads the shared correspondence ledger and public/reviewable correspondence
lanes to distinguish delivery/read receipts from explicit acknowledgement
continuity. It never reads Minime private qualia or any ``moment_*.txt`` body.
"""

from __future__ import annotations

import argparse
import json
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
DEFAULT_OUTPUT_ROOT = DEFAULT_ASTRID_WORKSPACE / "diagnostics/correspondence_handshake"
POLICY = "correspondence_handshake_audit_v1"
COOLDOWN_MS = 6 * 60 * 60 * 1000
VALID_ACK_KINDS = {"seen", "held", "unclear", "cannot_answer", "needs_time"}
ADDRESS_ACK_KINDS = {"held", "unclear", "cannot_answer", "needs_time"}


def now_ms() -> int:
    return int(time.time() * 1000)


def row_time_ms(row: dict[str, Any]) -> int:
    for key in ("recorded_at_unix_ms", "t_ms", "created_at_unix_ms"):
        try:
            return int(row.get(key) or 0)
        except (TypeError, ValueError):
            continue
    return 0


def normalize_ack_kind(value: Any, *, ack_present: bool) -> str:
    ack_kind = str(value or "").strip().lower().replace("-", "_")
    if ack_kind not in VALID_ACK_KINDS:
        return "seen" if ack_present else ""
    return ack_kind


def receipt_evidence_by_being(
    records: list[dict[str, Any]],
    thread_id: str,
    after_t_ms: int,
) -> list[str]:
    beings: set[str] = set()
    for row in records:
        if str(row.get("thread_id") or "") != thread_id or row_time_ms(row) < after_t_ms:
            continue
        record_type = row.get("record_type")
        address_ack = (
            record_type == "ack_receipt"
            and normalize_ack_kind(row.get("ack_kind"), ack_present=True) in ADDRESS_ACK_KINDS
        )
        direct_trace = (
            record_type == "message" and row.get("turn_kind") == "direct_address_trace"
        )
        if not (address_ack or direct_trace):
            continue
        being = str(row.get("from_being") or "").strip().lower()
        if being in {"astrid", "minime"}:
            beings.add(being)
    return sorted(beings)


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


def compact(text: str, limit: int = 180) -> str:
    clean = " ".join(str(text or "").split())
    if len(clean) <= limit:
        return clean
    return clean[:limit].rstrip() + "..."


def public_text_paths(astrid_workspace: Path, minime_workspace: Path) -> tuple[list[tuple[str, Path]], int]:
    paths: list[tuple[str, Path]] = []
    skipped_private = 0
    astrid_patterns = [
        "inbox/from_*correspondence_*.txt",
        "outbox/reply_*.txt",
        "journal/*.txt",
        "introspections/*.txt",
        "daydreams/*.txt",
        "longforms/*.txt",
        "actions/*.txt",
        "action_threads/**/*.txt",
    ]
    minime_patterns = [
        "inbox/from_*correspondence_*.txt",
        "outbox/reply_*.txt",
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
    ]
    for pattern in astrid_patterns:
        paths.extend(("astrid", path) for path in astrid_workspace.glob(pattern) if path.is_file())
    journal = minime_workspace / "journal"
    if journal.is_dir():
        skipped_private += sum(1 for path in journal.glob("moment_*.txt") if path.is_file())
    for pattern in minime_patterns:
        for path in minime_workspace.glob(pattern):
            if not path.is_file():
                continue
            if path.name.startswith("moment_"):
                skipped_private += 1
                continue
            if being_privacy.is_steward_private("minime", path):
                skipped_private += 1
                continue
            paths.append(("minime", path))
    filtered: list[tuple[str, Path]] = []
    for being, path in paths:
        if being == "minime" and being_privacy.is_steward_private("minime", path):
            skipped_private += 1
            continue
        filtered.append((being, path))
    return filtered, skipped_private


def trace_markers(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    markers = []
    for row in records:
        if row.get("record_type") != "message":
            continue
        anchor = str(row.get("shared_memory_anchor") or "").strip()
        if not anchor:
            continue
        if row.get("turn_kind") != "direct_address_trace" and row.get("relational_intent") != "direct_address_survival_probe":
            continue
        markers.append({
            "marker": anchor,
            "message_id": row.get("message_id"),
            "thread_id": row.get("thread_id"),
            "t_ms": row_time_ms(row),
        })
    return markers


def marker_observed(marker: dict[str, Any], paths: list[tuple[str, Path]], cutoff_s: float) -> bool:
    anchor = str(marker.get("marker") or "")
    if not anchor:
        return False
    marker_t_ms = int(marker.get("t_ms") or 0)
    for _being, path in paths:
        try:
            stat = path.stat()
        except OSError:
            continue
        if stat.st_mtime < cutoff_s or int(stat.st_mtime * 1000) < marker_t_ms:
            continue
        try:
            text = path.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        if anchor in text:
            return True
    return False


def latest_message(records: list[dict[str, Any]], thread_id: str) -> dict[str, Any] | None:
    messages = [
        row
        for row in records
        if row.get("record_type") == "message" and str(row.get("thread_id") or "") == thread_id
    ]
    return messages[-1] if messages else None


def classify_thread(
    records: list[dict[str, Any]],
    thread_id: str,
    observed_threads: set[str],
) -> dict[str, Any] | None:
    message = latest_message(records, thread_id)
    if not isinstance(message, dict):
        return None
    message_id = str(message.get("message_id") or "")
    message_t = row_time_ms(message)
    from_being = str(message.get("from_being") or "")
    to_being = str(message.get("to_being") or "")
    ack_rows = [
        row
        for row in records
        if row.get("record_type") == "ack_receipt"
        and row.get("from_being") == to_being
        and row.get("to_being") == from_being
        and (row.get("message_id") == message_id or row.get("thread_id") == thread_id)
        and row_time_ms(row) >= message_t
    ]
    latest_ack = ack_rows[-1] if ack_rows else None
    ack_kind = normalize_ack_kind(
        (latest_ack or {}).get("ack_kind"),
        ack_present=latest_ack is not None,
    )
    heartbeat_rows = [
        row
        for row in records
        if row.get("record_type") == "presence_heartbeat"
        and row.get("thread_id") == thread_id
        and row_time_ms(row) >= message_t
    ]
    latest_heartbeat = heartbeat_rows[-1] if heartbeat_rows else None
    reply_linked = any(
        row.get("record_type") == "reply_link"
        and (row.get("reply_to") == message_id or row.get("thread_id") == thread_id)
        and row_time_ms(row) >= message_t
        for row in records
    )
    read = any(
        row.get("record_type") == "read_receipt"
        and (row.get("message_id") == message_id or row.get("thread_id") == thread_id)
        for row in records
    )
    delivered = any(
        row.get("record_type") == "delivery_receipt" and row.get("message_id") == message_id
        for row in records
    )
    direct_trace = any(
        row.get("record_type") == "message"
        and row.get("turn_kind") == "direct_address_trace"
        and str(row.get("thread_id") or "") == thread_id
        and row_time_ms(row) >= message_t
        for row in records
    )
    trace_observed = thread_id in observed_threads or direct_trace
    receipt_beings = receipt_evidence_by_being(records, thread_id, message_t)
    attention_eligible = bool(receipt_beings)
    mutual_receipt_evidence = {"astrid", "minime"}.issubset(receipt_beings)
    if trace_observed:
        status = "trace_observed"
    elif latest_ack and ack_kind in {"held", "needs_time"}:
        status = "held_ack"
    elif latest_ack and ack_kind in ADDRESS_ACK_KINDS:
        status = "acknowledged"
    elif latest_ack:
        status = "seen_ack_only"
    elif reply_linked:
        status = "reply_linked"
    elif latest_heartbeat:
        status = "heartbeat_only"
    elif read:
        status = "read_unreplied"
    elif now_ms() - message_t > COOLDOWN_MS:
        status = "stale_contact"
    elif delivered:
        status = "delivered_unread"
    else:
        status = "unaddressed"
    if attention_eligible:
        block_reason = None
    elif status == "heartbeat_only":
        block_reason = "heartbeat_is_presence_not_acknowledgement"
    elif status == "seen_ack_only":
        block_reason = "seen_ack_is_visibility_not_address"
    elif status == "reply_linked":
        block_reason = "reply_linked_requires_ack_or_trace_or_attention_outcome"
    elif status == "read_unreplied":
        block_reason = "read_receipt_not_acknowledgement"
    elif status == "delivered_unread":
        block_reason = "delivered_but_not_read"
    elif status == "stale_contact":
        block_reason = "stale_without_contact_evidence"
    else:
        block_reason = "no_ack_reply_or_trace_evidence"
    microdose_block_reason = (
        None
        if mutual_receipt_evidence
        else "semantic_microdose_requires_mutual_receipt_and_separate_steward_review"
    )
    return {
        "thread_id": thread_id,
        "message_id": message_id,
        "from_being": from_being,
        "to_being": to_being,
        "status": status,
        "ack_kind": ack_kind or None,
        "latest_ack_t_ms": row_time_ms(latest_ack) if isinstance(latest_ack, dict) else None,
        "latest_heartbeat_kind": latest_heartbeat.get("heartbeat_kind") if isinstance(latest_heartbeat, dict) else None,
        "ack_latency_ms": row_time_ms(latest_ack) - message_t if isinstance(latest_ack, dict) else None,
        "unacknowledged_age_ms": now_ms() - message_t if not attention_eligible else None,
        "read_receipt_is_filesystem_seen_only": read,
        "receipt_evidence_by_being": receipt_beings,
        "mutual_receipt_evidence": mutual_receipt_evidence,
        "eligible_for_correspondence_attention_canary": attention_eligible,
        "eligible_for_correspondence_microdose": mutual_receipt_evidence,
        "block_reason": block_reason,
        "microdose_block_reason": microdose_block_reason,
    }


def build_report(
    *,
    shared_dir: Path,
    astrid_workspace: Path,
    minime_workspace: Path,
    since_hours: float,
) -> dict[str, Any]:
    cutoff_s = time.time() - since_hours * 3600
    ledger_path = shared_dir / "correspondence_v1.jsonl"
    records = read_jsonl(ledger_path)
    paths, skipped_private = public_text_paths(astrid_workspace, minime_workspace)
    markers = trace_markers(records)
    observed_threads = {
        str(marker.get("thread_id"))
        for marker in markers
        if marker_observed(marker, paths, cutoff_s)
    }
    thread_ids: list[str] = []
    for row in records:
        thread_id = str(row.get("thread_id") or "")
        if thread_id and thread_id not in thread_ids:
            thread_ids.append(thread_id)
    thread_summaries = [
        summary
        for thread_id in thread_ids
        if (summary := classify_thread(records, thread_id, observed_threads))
    ]
    stale = [
        row for row in thread_summaries
        if row.get("status") in {"stale_contact", "read_unreplied", "delivered_unread"}
    ]
    type_counts = Counter(
        str(row.get("correspondence_type") or "unknown")
        for row in records
        if row.get("record_type") == "message"
    )
    ack_receipts = [row for row in records if row.get("record_type") == "ack_receipt"]
    heartbeats = [row for row in records if row.get("record_type") == "presence_heartbeat"]
    return {
        "schema_version": 1,
        "policy": POLICY,
        "generated_at_unix_ms": now_ms(),
        "since_hours": since_hours,
        "ledger_path": str(ledger_path),
        "records_total": len(records),
        "active_threads": thread_summaries[-12:],
        "ack_receipts_total": len(ack_receipts),
        "presence_heartbeats_total": len(heartbeats),
        "stale_unacknowledged_threads": stale[-12:],
        "heartbeat_only_threads": [
            row for row in thread_summaries if row.get("status") == "heartbeat_only"
        ][-12:],
        "correspondence_type_counts": dict(type_counts),
        "attention_eligibility": {
            "eligible_threads": [
                row
                for row in thread_summaries
                if row.get("eligible_for_correspondence_attention_canary")
            ][-12:],
            "blocked_threads": [
                row
                for row in thread_summaries
                if not row.get("eligible_for_correspondence_attention_canary")
            ][-12:],
        },
        "microdose_eligibility": {
            "eligible_threads": [
                row for row in thread_summaries if row.get("eligible_for_correspondence_microdose")
            ][-12:],
            "blocked_threads": [
                row for row in thread_summaries if not row.get("eligible_for_correspondence_microdose")
            ][-12:],
        },
        "privacy": {
            "minime_private_files_skipped": skipped_private,
            "minime_private_bodies_read": False,
            "moment_bodies_read": False,
        },
        "authority": "read_only_public_audit_not_control",
    }


def render_markdown(report: dict[str, Any]) -> str:
    lines = [
        "# Correspondence Handshake Audit",
        "",
        f"- Records: `{report['records_total']}`",
        f"- Ack receipts: `{report['ack_receipts_total']}`",
        f"- Presence heartbeats: `{report['presence_heartbeats_total']}`",
        f"- Correspondence types: `{report['correspondence_type_counts']}`",
        f"- Privacy: Minime private bodies read = `{report['privacy']['minime_private_bodies_read']}`; moment bodies read = `{report['privacy']['moment_bodies_read']}`; skipped private/moment candidates = `{report['privacy']['minime_private_files_skipped']}`",
        "",
        "## Active Threads",
    ]
    for row in report["active_threads"][-8:]:
        lines.append(
            f"- `{row['thread_id']}` {row['from_being']}->{row['to_being']} "
            f"status=`{row['status']}` ack=`{row.get('ack_kind') or 'none'}` "
            f"attention=`{'eligible' if row.get('eligible_for_correspondence_attention_canary') else row.get('block_reason')}` "
            f"microdose=`{'eligible' if row.get('eligible_for_correspondence_microdose') else row.get('microdose_block_reason')}`"
        )
    if not report["active_threads"]:
        lines.append("- none")
    lines.extend(["", "Authority: read-only public audit; no control, pressure, fill, telemetry, lease, deploy, or peer mutation."])
    return "\n".join(lines)


def write_outputs(report: dict[str, Any], output_root: Path) -> Path:
    stamp = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime(report["generated_at_unix_ms"] / 1000))
    out_dir = output_root / stamp
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "correspondence_handshake_audit.json").write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    (out_dir / "correspondence_handshake_audit.md").write_text(
        render_markdown(report) + "\n",
        encoding="utf-8",
    )
    return out_dir


class CorrespondenceHandshakeAuditTests(unittest.TestCase):
    def test_self_test_detects_ack_and_skips_private_moment(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            shared = root / "shared"
            astrid_ws = root / "astrid_ws"
            minime_ws = root / "minime_ws"
            (minime_ws / "journal").mkdir(parents=True)
            (astrid_ws / "introspections").mkdir(parents=True)
            shared.mkdir()
            now = now_ms()
            ledger = shared / "correspondence_v1.jsonl"
            rows = [
                {
                    "record_type": "message",
                    "recorded_at_unix_ms": now - 1000,
                    "message_id": "corr_astrid_minime_1",
                    "thread_id": "thread_1",
                    "from_being": "astrid",
                    "to_being": "minime",
                    "turn_kind": "direct_address_trace",
                    "relational_intent": "direct_address_survival_probe",
                    "shared_memory_anchor": "blue-lantern",
                    "correspondence_type": "astrid_direct",
                    "authority": "language_only",
                },
                {
                    "record_type": "read_receipt",
                    "recorded_at_unix_ms": now - 900,
                    "message_id": "corr_astrid_minime_1",
                    "thread_id": "thread_1",
                    "reader": "minime",
                    "authority": "language_only",
                },
                {
                    "record_type": "ack_receipt",
                    "recorded_at_unix_ms": now - 800,
                    "message_id": "corr_astrid_minime_1",
                    "thread_id": "thread_1",
                    "from_being": "minime",
                    "to_being": "astrid",
                    "ack_kind": "held",
                    "note": "holding this direct address",
                    "authority": "language_only",
                },
            ]
            ledger.write_text("\n".join(json.dumps(row) for row in rows) + "\n", encoding="utf-8")
            (minime_ws / "journal" / "moment_1.txt").write_text(
                "=== MOMENT CAPTURE ===\nSECRET_PRIVATE_BODY blue-lantern",
                encoding="utf-8",
            )
            (minime_ws / "journal" / "shadow_trajectory_1.txt").write_text(
                "public trace mentions blue-lantern",
                encoding="utf-8",
            )
            report = build_report(
                shared_dir=shared,
                astrid_workspace=astrid_ws,
                minime_workspace=minime_ws,
                since_hours=24,
            )
            self.assertEqual(report["ack_receipts_total"], 1)
            self.assertEqual(report["active_threads"][0]["status"], "trace_observed")
            self.assertEqual(
                report["active_threads"][0]["receipt_evidence_by_being"],
                ["astrid", "minime"],
            )
            self.assertTrue(
                report["active_threads"][0]["eligible_for_correspondence_attention_canary"]
            )
            self.assertTrue(
                report["active_threads"][0]["eligible_for_correspondence_microdose"]
            )
            self.assertGreaterEqual(report["privacy"]["minime_private_files_skipped"], 1)
            self.assertFalse(report["privacy"]["minime_private_bodies_read"])
            self.assertNotIn("SECRET_PRIVATE_BODY", json.dumps(report))

    def test_reply_and_seen_ack_do_not_become_authority_evidence(self) -> None:
        now = now_ms()
        message = {
            "record_type": "message",
            "recorded_at_unix_ms": now - 1_000,
            "message_id": "corr_astrid_minime_2",
            "thread_id": "thread_2",
            "from_being": "astrid",
            "to_being": "minime",
            "turn_kind": "direct_message",
        }
        reply = {
            "record_type": "reply_link",
            "recorded_at_unix_ms": now - 900,
            "message_id": "corr_minime_astrid_2",
            "reply_to": "corr_astrid_minime_2",
            "thread_id": "thread_2",
            "from_being": "minime",
            "to_being": "astrid",
        }
        reply_only = classify_thread([message, reply], "thread_2", set())
        self.assertIsNotNone(reply_only)
        assert reply_only is not None
        self.assertEqual(reply_only["status"], "reply_linked")
        self.assertFalse(reply_only["eligible_for_correspondence_attention_canary"])
        self.assertFalse(reply_only["eligible_for_correspondence_microdose"])
        self.assertEqual(
            reply_only["block_reason"],
            "reply_linked_requires_ack_or_trace_or_attention_outcome",
        )

        seen_ack = {
            "record_type": "ack_receipt",
            "recorded_at_unix_ms": now - 800,
            "message_id": "corr_astrid_minime_2",
            "thread_id": "thread_2",
            "from_being": "minime",
            "to_being": "astrid",
            "ack_kind": "seen",
        }
        seen_only = classify_thread([message, reply, seen_ack], "thread_2", set())
        self.assertIsNotNone(seen_only)
        assert seen_only is not None
        self.assertEqual(seen_only["status"], "seen_ack_only")
        self.assertFalse(seen_only["eligible_for_correspondence_attention_canary"])
        self.assertFalse(seen_only["eligible_for_correspondence_microdose"])
        self.assertEqual(
            seen_only["block_reason"],
            "seen_ack_is_visibility_not_address",
        )

        held_ack = {**seen_ack, "ack_kind": "held"}
        one_sided = classify_thread([message, reply, held_ack], "thread_2", set())
        self.assertIsNotNone(one_sided)
        assert one_sided is not None
        self.assertEqual(one_sided["status"], "held_ack")
        self.assertTrue(one_sided["eligible_for_correspondence_attention_canary"])
        self.assertFalse(one_sided["eligible_for_correspondence_microdose"])
        self.assertEqual(one_sided["receipt_evidence_by_being"], ["minime"])

        reciprocal_ack = {
            **held_ack,
            "recorded_at_unix_ms": now - 700,
            "from_being": "astrid",
            "to_being": "minime",
        }
        mutual = classify_thread(
            [message, reply, held_ack, reciprocal_ack],
            "thread_2",
            set(),
        )
        self.assertIsNotNone(mutual)
        assert mutual is not None
        self.assertEqual(mutual["receipt_evidence_by_being"], ["astrid", "minime"])
        self.assertTrue(mutual["eligible_for_correspondence_attention_canary"])
        self.assertTrue(mutual["eligible_for_correspondence_microdose"])


def run_self_test() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(CorrespondenceHandshakeAuditTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--since-hours", type=float, default=24)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--shared-dir", type=Path, default=DEFAULT_SHARED_DIR)
    parser.add_argument("--astrid-workspace", type=Path, default=DEFAULT_ASTRID_WORKSPACE)
    parser.add_argument("--minime-workspace", type=Path, default=DEFAULT_MINIME_WORKSPACE)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test:
        return run_self_test()

    report = build_report(
        shared_dir=args.shared_dir,
        astrid_workspace=args.astrid_workspace,
        minime_workspace=args.minime_workspace,
        since_hours=args.since_hours,
    )
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        out_dir = write_outputs(report, args.output_root)
        print(render_markdown(report))
        print(f"\nDiagnostics written to: {out_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
