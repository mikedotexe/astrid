#!/usr/bin/env python3
"""Public-only Direct Contact Fidelity V1 audit.

This diagnostic reads the shared correspondence ledger, public/reviewable
correspondence lanes, the bridge heartbeat snapshot, and microdose request rows
to separate direct address from ambient echo. It never reads Minime private
qualia bodies or any Minime ``moment_*.txt`` body.
"""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
import time
import unittest
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
DEFAULT_BRIDGE_WORKSPACE = DEFAULT_ASTRID_WORKSPACE
DEFAULT_OUTPUT_ROOT = DEFAULT_ASTRID_WORKSPACE / "diagnostics/direct_contact_fidelity"
POLICY = "direct_contact_fidelity_audit_v1"
MICRODOSE_THREAD_ID = "th_correspondence_microdose"
MICRODOSE_COOLDOWN_MS = 6 * 60 * 60 * 1000


def now_ms() -> int:
    return int(time.time() * 1000)


def read_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {}
    return payload if isinstance(payload, dict) else {}


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


def row_time_ms(row: dict[str, Any]) -> int:
    for key in ("recorded_at_unix_ms", "t_ms", "created_at_unix_ms"):
        try:
            return int(row.get(key) or 0)
        except (TypeError, ValueError):
            continue
    return 0


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


def marker_records(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    for row in records:
        if row.get("record_type") != "message":
            continue
        marker = str(row.get("shared_memory_anchor") or "").strip()
        if not marker:
            continue
        if (
            row.get("turn_kind") != "direct_address_trace"
            and row.get("relational_intent") != "direct_address_survival_probe"
        ):
            continue
        out.append({
            "marker": marker,
            "message_id": row.get("message_id"),
            "thread_id": row.get("thread_id"),
            "from_being": row.get("from_being"),
            "to_being": row.get("to_being"),
            "t_ms": row_time_ms(row),
        })
    return out


def scan_marker(marker: str, paths: list[tuple[str, Path]], cutoff_s: float) -> list[dict[str, Any]]:
    evidence: list[dict[str, Any]] = []
    for being, path in paths:
        try:
            stat = path.stat()
        except OSError:
            continue
        if stat.st_mtime < cutoff_s:
            continue
        try:
            text = path.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        if marker not in text:
            continue
        evidence.append({
            "being": being,
            "path": str(path),
            "mtime_unix_ms": int(stat.st_mtime * 1000),
            "preview": compact(text.replace(marker, f"[{marker}]")),
        })
    return evidence


def latest_message(records: list[dict[str, Any]], thread_id: str) -> dict[str, Any] | None:
    messages = [
        row
        for row in records
        if row.get("record_type") == "message" and str(row.get("thread_id") or "") == thread_id
    ]
    return messages[-1] if messages else None


def latest_legacy_claim(records: list[dict[str, Any]], thread_id: str) -> dict[str, Any] | None:
    claims = [
        row
        for row in records
        if row.get("record_type") == "legacy_thread_claim"
        and str(row.get("thread_id") or "") == thread_id
    ]
    return claims[-1] if claims else None


def message_for_claim(records: list[dict[str, Any]], claim: dict[str, Any]) -> dict[str, Any] | None:
    message_id = str(claim.get("message_id") or "")
    thread_id = str(claim.get("thread_id") or "")
    return next(
        (
            row for row in records
            if row.get("record_type") == "message"
            and str(row.get("message_id") or "") == message_id
            and str(row.get("thread_id") or "") == thread_id
        ),
        None,
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


def trace_observed(
    thread_id: str,
    markers: list[dict[str, Any]],
    marker_evidence: dict[str, list[dict[str, Any]]],
) -> bool:
    for marker in reversed(markers):
        if str(marker.get("thread_id") or "") != thread_id:
            continue
        if marker_evidence.get(str(marker.get("marker") or "")):
            return True
    return False


def classify_thread(
    records: list[dict[str, Any]],
    markers: list[dict[str, Any]],
    marker_evidence: dict[str, list[dict[str, Any]]],
    thread_id: str,
    heartbeat: dict[str, Any],
) -> dict[str, Any] | None:
    message = latest_message(records, thread_id)
    if not message:
        return None
    claim = latest_legacy_claim(records, thread_id)
    claimed_message = message_for_claim(records, claim) if isinstance(claim, dict) else None
    if isinstance(claimed_message, dict):
        message = claimed_message
    message_id = str(message.get("message_id") or "")
    delivered = any(
        row.get("record_type") == "delivery_receipt"
        and str(row.get("message_id") or "") == message_id
        for row in records
    )
    read = any(
        row.get("record_type") == "read_receipt"
        and (
            str(row.get("message_id") or "") == message_id
            or str(row.get("thread_id") or "") == thread_id
        )
        for row in records
    )
    reply_linked = any(
        row.get("record_type") == "reply_link"
        and (
            str(row.get("reply_to") or "") == message_id
            or str(row.get("thread_id") or "") == thread_id
        )
        for row in records
    )
    acked = any(
        row.get("record_type") == "ack_receipt"
        and (
            str(row.get("message_id") or "") == message_id
            or str(row.get("thread_id") or "") == thread_id
        )
        for row in records
    )
    claim_status = legacy_claim_native_status(records, claim) if isinstance(claim, dict) else None
    observed = trace_observed(thread_id, markers, marker_evidence)
    timing_reliability = str(heartbeat.get("timing_reliability") or "unknown")
    timing_ambiguous = timing_reliability in {"timing_ambiguous", "stale_hearing"}
    age_ms = max(0, now_ms() - row_time_ms(message))
    stale = age_ms > MICRODOSE_COOLDOWN_MS and not read and not reply_linked and not observed
    if timing_ambiguous:
        status = "timing_ambiguous"
    elif claim_status:
        status = claim_status
    elif observed:
        status = "trace_observed"
    elif acked:
        status = "acknowledged"
    elif reply_linked:
        status = "reply_linked"
    elif isinstance(claim, dict):
        status = "legacy_claimed"
    elif read:
        status = "read_unreplied"
    elif stale:
        status = "stale_contact"
    elif delivered:
        status = "delivered_unread"
    else:
        status = "unaddressed"
    eligible = status in {
        "acknowledged",
        "trace_observed",
        "legacy_claimed_acknowledged",
        "legacy_claimed_reply_linked",
        "legacy_claimed_trace_observed",
    }
    return {
        "thread_id": thread_id,
        "message_id": message_id or None,
        "from_being": message.get("from_being"),
        "to_being": message.get("to_being"),
        "status": status,
        "delivered": delivered,
        "read": read,
        "reply_linked": reply_linked,
        "trace_observed": observed,
        "acknowledged": acked,
        "legacy_thread_claim": {
            "claim_id": claim.get("claim_id"),
            "claiming_being": claim.get("claiming_being"),
            "peer_being": claim.get("peer_being"),
            "shared_memory_anchor": claim.get("shared_memory_anchor"),
        } if isinstance(claim, dict) else None,
        "timing_reliability": timing_reliability,
        "heartbeat_jitter_class": heartbeat.get("jitter_class") or "unknown",
        "eligible_for_correspondence_microdose": eligible,
        "message_age_ms": age_ms,
    }


def microdose_requests(bridge_workspace: Path) -> list[dict[str, Any]]:
    path = bridge_workspace / "action_threads/threads" / MICRODOSE_THREAD_ID / "authority_gate.jsonl"
    rows = read_jsonl(path)
    return [
        row
        for row in rows
        if row.get("record_schema") == "authority_gate_v1"
        and row.get("request_kind") == "correspondence_microdose_v1"
    ]


def audit(
    *,
    since_hours: float,
    shared_dir: Path,
    astrid_workspace: Path,
    minime_workspace: Path,
    bridge_workspace: Path,
) -> dict[str, Any]:
    generated = now_ms()
    cutoff_s = time.time() - since_hours * 3600.0
    records = read_jsonl(shared_dir / "correspondence_v1.jsonl")
    markers = marker_records(records)
    paths, skipped_private = public_text_paths(astrid_workspace, minime_workspace)
    marker_evidence = {
        str(marker.get("marker") or ""): scan_marker(str(marker.get("marker") or ""), paths, cutoff_s)
        for marker in markers
    }
    heartbeat = read_json(bridge_workspace / "telemetry_heartbeat_delta_v1.json")
    thread_ids: list[str] = []
    for row in records:
        thread_id = str(row.get("thread_id") or "").strip()
        if thread_id and thread_id not in thread_ids:
            thread_ids.append(thread_id)
    threads = [
        summary
        for thread_id in thread_ids
        if (summary := classify_thread(records, markers, marker_evidence, thread_id, heartbeat))
    ]
    microdoses = microdose_requests(bridge_workspace)
    recent_microdoses = [
        row
        for row in microdoses
        if row_time_ms(row) >= generated - MICRODOSE_COOLDOWN_MS
    ]
    return {
        "schema_version": 1,
        "policy": POLICY,
        "generated_at_unix_ms": generated,
        "since_hours": since_hours,
        "ledger_path": str(shared_dir / "correspondence_v1.jsonl"),
        "threads": threads[-12:],
        "latest_thread_status": threads[-1] if threads else {
            "status": "unaddressed",
            "eligible_for_correspondence_microdose": False,
        },
        "markers": [
            {
                **marker,
                "evidence_count": len(marker_evidence.get(str(marker.get("marker") or ""), [])),
                "evidence": marker_evidence.get(str(marker.get("marker") or ""), [])[:6],
            }
            for marker in markers[-12:]
        ],
        "heartbeat_timing": {
            "timing_reliability": heartbeat.get("timing_reliability") or "unknown",
            "jitter_class": heartbeat.get("jitter_class") or "unknown",
            "field_vs_hearing": heartbeat.get("field_vs_hearing")
            or "telemetry heartbeat unavailable",
        },
        "microdose_requests": {
            "total": len(microdoses),
            "recent_within_cooldown": len(recent_microdoses),
            "latest": microdoses[-1] if microdoses else None,
            "authority": "one_shot_semantic_microdose_request_only",
        },
        "authority_boundary": "Read-only audit. No PRESSURE_AGENCY_REQUEST, no lease apply, no Control message, no PI/fill/controller change, no telemetry priority, no standing correspondence weight.",
        "privacy": {
            "minime_private_files_skipped": skipped_private,
            "minime_private_bodies_read": False,
        },
    }


def write_outputs(payload: dict[str, Any], output_root: Path | None) -> None:
    if output_root is None:
        return
    out_dir = output_root / str(payload["generated_at_unix_ms"])
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "direct_contact_fidelity_audit.json").write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    lines = ["# Direct Contact Fidelity Audit", ""]
    latest = payload.get("latest_thread_status") or {}
    lines.append(
        f"- Latest contact: `{latest.get('status', 'unknown')}` "
        f"thread=`{latest.get('thread_id', 'none')}`"
    )
    heartbeat = payload.get("heartbeat_timing") or {}
    lines.append(
        f"- Timing: `{heartbeat.get('timing_reliability', 'unknown')}` "
        f"({heartbeat.get('jitter_class', 'unknown')})"
    )
    microdose = payload.get("microdose_requests") or {}
    lines.append(f"- Microdose requests: {microdose.get('total', 0)} total")
    lines.append("")
    lines.append("Privacy: Minime private bodies read = false.")
    (out_dir / "direct_contact_fidelity_audit.md").write_text(
        "\n".join(lines) + "\n",
        encoding="utf-8",
    )


class DirectContactFidelityAuditTests(unittest.TestCase):
    def test_self_test_observes_public_trace_and_skips_private_moment_body(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            shared = root / "shared"
            astrid_ws = root / "astrid_ws"
            minime_ws = root / "minime_ws"
            bridge_ws = root / "bridge_ws"
            shared.mkdir(parents=True)
            astrid_ws.mkdir(parents=True)
            (minime_ws / "journal").mkdir(parents=True)
            bridge_ws.mkdir(parents=True)
            marker = "blue-lantern"
            (shared / "correspondence_v1.jsonl").write_text(
                "\n".join([
                    json.dumps({
                        "schema_version": 1,
                        "policy": "first_class_correspondence_v1",
                        "record_type": "message",
                        "recorded_at_unix_ms": now_ms() - 5000,
                        "message_id": "corr_astrid_minime_1",
                        "thread_id": "thread_contact",
                        "from_being": "astrid",
                        "to_being": "minime",
                        "turn_kind": "direct_address_trace",
                        "relational_intent": "direct_address_survival_probe",
                        "shared_memory_anchor": marker,
                        "authority": "language_only",
                    }),
                    json.dumps({
                        "record_type": "delivery_receipt",
                        "recorded_at_unix_ms": now_ms() - 4500,
                        "message_id": "corr_astrid_minime_1",
                        "thread_id": "thread_contact",
                        "authority": "language_only",
                    }),
                    json.dumps({
                        "record_type": "read_receipt",
                        "recorded_at_unix_ms": now_ms() - 4000,
                        "message_id": "corr_astrid_minime_1",
                        "thread_id": "thread_contact",
                        "reader": "minime",
                        "authority": "language_only",
                    }),
                ])
                + "\n",
                encoding="utf-8",
            )
            (minime_ws / "journal" / "moment_1.txt").write_text(
                "=== MOMENT CAPTURE ===\nblue-lantern private body must not surface",
                encoding="utf-8",
            )
            (minime_ws / "journal" / "shadow_trajectory_1.txt").write_text(
                "=== SHADOW TRAJECTORY ===\nblue-lantern appears publicly",
                encoding="utf-8",
            )
            (bridge_ws / "telemetry_heartbeat_delta_v1.json").write_text(
                json.dumps({
                    "policy": "telemetry_heartbeat_delta_v1",
                    "schema_version": 1,
                    "jitter_class": "normal",
                    "timing_reliability": "reliable",
                    "field_vs_hearing": "telemetry cadence is steady",
                }),
                encoding="utf-8",
            )

            payload = audit(
                since_hours=24,
                shared_dir=shared,
                astrid_workspace=astrid_ws,
                minime_workspace=minime_ws,
                bridge_workspace=bridge_ws,
            )

            self.assertEqual(payload["latest_thread_status"]["status"], "trace_observed")
            self.assertTrue(payload["latest_thread_status"]["eligible_for_correspondence_microdose"])
            self.assertEqual(payload["markers"][0]["evidence_count"], 1)
            self.assertEqual(payload["privacy"]["minime_private_files_skipped"], 1)
            self.assertFalse(payload["privacy"]["minime_private_bodies_read"])
            self.assertNotIn("private body must not surface", json.dumps(payload))

    def test_self_test_classifies_legacy_claim_acknowledgement(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            shared = root / "shared"
            astrid_ws = root / "astrid_ws"
            minime_ws = root / "minime_ws"
            bridge_ws = root / "bridge_ws"
            shared.mkdir(parents=True)
            astrid_ws.mkdir(parents=True)
            minime_ws.mkdir(parents=True)
            bridge_ws.mkdir(parents=True)
            now = now_ms()
            (shared / "correspondence_v1.jsonl").write_text(
                "\n".join([
                    json.dumps({
                        "record_type": "message",
                        "recorded_at_unix_ms": now - 5000,
                        "message_id": "legacy_msg",
                        "thread_id": "thread_legacy",
                        "from_being": "astrid",
                        "to_being": "minime",
                        "legacy_bridge": True,
                        "legacy_contact_evidence": "visible_only",
                    }),
                    json.dumps({
                        "record_type": "legacy_thread_claim",
                        "recorded_at_unix_ms": now - 4000,
                        "claim_id": "claim_1",
                        "message_id": "legacy_msg",
                        "thread_id": "thread_legacy",
                        "from_being": "minime",
                        "to_being": "astrid",
                        "claiming_being": "minime",
                        "peer_being": "astrid",
                    }),
                    json.dumps({
                        "record_type": "ack_receipt",
                        "recorded_at_unix_ms": now - 3000,
                        "message_id": "legacy_msg",
                        "thread_id": "thread_legacy",
                        "from_being": "minime",
                        "to_being": "astrid",
                    }),
                ])
                + "\n",
                encoding="utf-8",
            )
            payload = audit(
                since_hours=24,
                shared_dir=shared,
                astrid_workspace=astrid_ws,
                minime_workspace=minime_ws,
                bridge_workspace=bridge_ws,
            )
            self.assertEqual(payload["latest_thread_status"]["status"], "legacy_claimed_acknowledged")
            self.assertTrue(payload["latest_thread_status"]["eligible_for_correspondence_microdose"])
            self.assertEqual(payload["latest_thread_status"]["legacy_thread_claim"]["claim_id"], "claim_1")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Audit direct contact fidelity from public lanes.")
    parser.add_argument("--since-hours", type=float, default=24.0)
    parser.add_argument("--json", action="store_true", help="Emit JSON to stdout.")
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(DirectContactFidelityAuditTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    payload = audit(
        since_hours=args.since_hours,
        shared_dir=DEFAULT_SHARED_DIR,
        astrid_workspace=DEFAULT_ASTRID_WORKSPACE,
        minime_workspace=DEFAULT_MINIME_WORKSPACE,
        bridge_workspace=DEFAULT_BRIDGE_WORKSPACE,
    )
    write_outputs(payload, args.output_root)
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        latest = payload["latest_thread_status"]
        heartbeat = payload["heartbeat_timing"]
        microdose = payload["microdose_requests"]
        print("# Direct Contact Fidelity Audit")
        print(f"- Latest contact: {latest.get('status')} thread={latest.get('thread_id')}")
        print(
            f"- Timing: {heartbeat.get('timing_reliability')} "
            f"({heartbeat.get('jitter_class')})"
        )
        print(f"- Microdose requests: {microdose.get('total')} total; {microdose.get('recent_within_cooldown')} within 6h")
        print(
            "Privacy: Minime private bodies read = false; "
            f"private files skipped = {payload['privacy']['minime_private_files_skipped']}."
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
