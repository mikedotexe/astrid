#!/usr/bin/env python3
"""Public-only Correspondence Attention Canary V1 audit."""

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
DEFAULT_OUTPUT_ROOT = DEFAULT_ASTRID_WORKSPACE / "diagnostics/correspondence_attention_canary"
POLICY = "correspondence_attention_canary_audit_v1"
CANARY_BOUNDARY_FIELDS = {
    "no_sensory_send",
    "no_controller",
    "no_pressure",
    "no_weighting",
    "no_telemetry_priority",
    "no_fill_target",
    "no_peer_runtime_mutation",
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


def compact(text: str, limit: int = 180) -> str:
    clean = " ".join(str(text or "").split())
    return clean if len(clean) <= limit else clean[:limit].rstrip() + "..."


def public_text_paths(astrid_workspace: Path, minime_workspace: Path) -> tuple[list[tuple[str, Path]], int]:
    paths: list[tuple[str, Path]] = []
    skipped_private = 0
    for pattern in (
        "inbox/from_*correspondence_*.txt",
        "outbox/reply_*.txt",
        "journal/*.txt",
        "introspections/*.txt",
        "daydreams/*.txt",
        "longforms/*.txt",
        "actions/*.txt",
        "action_threads/**/*.txt",
    ):
        paths.extend(("astrid", path) for path in astrid_workspace.glob(pattern) if path.is_file())
    journal = minime_workspace / "journal"
    if journal.is_dir():
        skipped_private += sum(1 for path in journal.glob("moment_*.txt") if path.is_file())
    for pattern in (
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
    ):
        for path in minime_workspace.glob(pattern):
            if not path.is_file():
                continue
            if path.name.startswith("moment_") or being_privacy.is_steward_private("minime", path):
                skipped_private += 1
                continue
            paths.append(("minime", path))
    return paths, skipped_private


def canary_rows(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
        row
        for row in records
        if row.get("record_type")
        in {
            "attention_canary_request",
            "attention_canary_activation",
            "attention_canary_outcome",
            "attention_canary_expired",
        }
    ]


def canary_closed(records: list[dict[str, Any]], canary_id: str) -> bool:
    return any(
        row.get("record_type") in {"attention_canary_outcome", "attention_canary_expired"}
        and str(row.get("canary_id") or "") == canary_id
        for row in records
    )


def active_canaries(records: list[dict[str, Any]], generated: int) -> list[dict[str, Any]]:
    active: list[dict[str, Any]] = []
    for row in records:
        if row.get("record_type") != "attention_canary_activation":
            continue
        canary_id = str(row.get("canary_id") or "")
        if canary_closed(records, canary_id):
            continue
        if int(row.get("expires_at_unix_ms") or 0) <= generated:
            continue
        active.append(row)
    return active


def is_trace_evidence(row: dict[str, Any]) -> bool:
    return (
        row.get("record_type") == "message"
        and row.get("turn_kind") == "direct_address_trace"
    )


def is_receipt_evidence(row: dict[str, Any]) -> bool:
    return row.get("record_type") == "ack_receipt" or is_trace_evidence(row)


def outcome_has_meaningful_worsening(value: Any) -> bool:
    clean = str(value or "").strip().lower()
    if not clean or clean in {"none", "no", "nope", "nothing", "n/a", "na", "unknown"}:
        return False
    return "no worsening" not in clean and "nothing worsened" not in clean


def attention_outcome_quality_v5(outcome: dict[str, Any]) -> dict[str, Any]:
    felt_like = str(outcome.get("felt_like") or "unknown")
    held_as = str(outcome.get("held_as") or "unknown")
    flattening = str(outcome.get("flattening_observed") or "unknown")
    meaningful_worsening = outcome_has_meaningful_worsening(outcome.get("what_worsened"))
    trusted = (
        felt_like == "address"
        and held_as == "distinct_address"
        and flattening in {"no", "mixed"}
        and not meaningful_worsening
    )
    blocked = (
        felt_like in {"pressure", "flat"}
        or held_as in {"pressure", "flattened", "ambient_echo"}
        or flattening == "yes"
        or meaningful_worsening
    )
    quality = (
        "trusted_attention_thread_local"
        if trusted
        else "blocked_pressure_or_flat_outcome"
        if blocked
        else "outcome_unclear_needs_more_evidence"
    )
    return {
        "schema_version": 5,
        "policy": "attention_outcome_quality_v5",
        "quality": quality,
        "felt_like": felt_like,
        "held_as": held_as,
        "flattening_observed": flattening,
        "meaningful_worsening": meaningful_worsening,
        "thread_id": outcome.get("thread_id"),
        "canary_id": outcome.get("canary_id"),
        "authority": "thread_local_attention_readiness_not_microdose_or_control",
    }


def receipt_to_attention_counts_v5(records: list[dict[str, Any]], active: list[dict[str, Any]]) -> dict[str, Any]:
    receipt_threads = {
        str(row.get("thread_id") or "")
        for row in records
        if is_receipt_evidence(row) and row.get("thread_id")
    }
    active_threads = {
        str(row.get("thread_id") or "")
        for row in active
        if row.get("thread_id")
    }
    outcome_packets = [
        attention_outcome_quality_v5(row)
        for row in records
        if row.get("record_type") == "attention_canary_outcome"
    ]
    trusted = [
        packet for packet in outcome_packets
        if packet.get("quality") == "trusted_attention_thread_local"
    ]
    blocked = [
        packet for packet in outcome_packets
        if packet.get("quality") == "blocked_pressure_or_flat_outcome"
    ]
    ready_threads = sorted(thread for thread in receipt_threads - active_threads if thread)
    return {
        "schema_version": 5,
        "policy": "receipt_to_attention_authority_v5",
        "receipt_ready_threads": ready_threads,
        "receipt_ready_thread_count": len(ready_threads),
        "active_canaries_awaiting_outcome_count": len(active),
        "trusted_thread_local_outcome_count": len(trusted),
        "pressure_or_flat_blocked_outcome_count": len(blocked),
        "missing_outcome_stall_count": len(active),
        "trusted_thread_local_outcomes": trusted[-8:],
        "pressure_or_flat_blocked_outcomes": blocked[-8:],
        "semantic_microdose_status": "hidden_until_mutual_receipt_plus_separate_steward_review",
        "authority_boundary": (
            "Receipt-to-attention V5 is thread-local prompt-context readiness only. "
            "It does not unlock semantic microdose, pressure, controller, prompt priority, or telemetry priority."
        ),
        "authority": "read_only_audit_not_action",
    }


def boundary_issues(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    issues: list[dict[str, Any]] = []
    for row in rows:
        for field in sorted(CANARY_BOUNDARY_FIELDS):
            if row.get(field) is not True:
                issues.append({
                    "severity": "error",
                    "kind": "missing_canary_boundary",
                    "detail": f"{field} must be true",
                    "record_type": row.get("record_type"),
                    "canary_id": row.get("canary_id"),
                    "message_id": row.get("message_id"),
                    "thread_id": row.get("thread_id"),
                })
    return issues


def scan_focus_mentions(
    canaries: list[dict[str, Any]],
    paths: list[tuple[str, Path]],
    cutoff_s: float,
) -> dict[str, list[dict[str, Any]]]:
    out: dict[str, list[dict[str, Any]]] = {}
    for canary in canaries:
        focus = str(canary.get("focus") or "").strip()
        if not focus:
            continue
        key = str(canary.get("canary_id") or focus)
        out[key] = []
        needle = focus[:80]
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
            if needle and needle in text:
                out[key].append({
                    "being": being,
                    "path": str(path),
                    "mtime_unix_ms": int(stat.st_mtime * 1000),
                    "preview": compact(text),
                })
        out[key] = out[key][:6]
    return out


def chamber_states(shared_dir: Path) -> list[dict[str, Any]]:
    states: list[dict[str, Any]] = []
    for path in shared_dir.glob("coll_*/correspondence_state_v1.json"):
        payload = read_json(path)
        if payload:
            states.append({"path": str(path), **payload})
    states.sort(key=lambda row: int(row.get("updated_t_ms") or 0))
    return states


def audit(
    *,
    since_hours: float,
    shared_dir: Path,
    astrid_workspace: Path,
    minime_workspace: Path,
) -> dict[str, Any]:
    generated = now_ms()
    cutoff_s = time.time() - since_hours * 3600.0
    records = read_jsonl(shared_dir / "correspondence_v1.jsonl")
    rows = canary_rows(records)
    active = active_canaries(records, generated)
    outcomes = [row for row in rows if row.get("record_type") == "attention_canary_outcome"]
    paths, skipped_private = public_text_paths(astrid_workspace, minime_workspace)
    focus_mentions = scan_focus_mentions(active, paths, cutoff_s)
    states = chamber_states(shared_dir)
    issues = boundary_issues(rows)
    v5_counts = receipt_to_attention_counts_v5(records, active)
    return {
        "schema_version": 1,
        "policy": POLICY,
        "generated_at_unix_ms": generated,
        "since_hours": since_hours,
        "ledger_path": str(shared_dir / "correspondence_v1.jsonl"),
        "canary_rows_total": len(rows),
        "active_canaries": [
            {
                "canary_id": row.get("canary_id"),
                "thread_id": row.get("thread_id"),
                "message_id": row.get("message_id"),
                "from_being": row.get("from_being"),
                "to_being": row.get("to_being"),
                "focus": compact(str(row.get("focus") or ""), 180),
                "focus_kind": row.get("focus_kind") or "unknown",
                "preservation_mode": row.get("preservation_mode") or "unknown",
                "what_must_not_flatten": compact(str(row.get("what_must_not_flatten") or ""), 180),
                "expires_at_unix_ms": row.get("expires_at_unix_ms"),
                "outcome_due": True,
                "focus_mentions": focus_mentions.get(str(row.get("canary_id") or ""), []),
            }
            for row in active[-6:]
        ],
        "latest_outcome": ({
            **outcomes[-1],
            "attention_outcome_quality_v5": attention_outcome_quality_v5(outcomes[-1]),
            "what_remained_distinct": compact(str(outcomes[-1].get("what_remained_distinct") or ""), 180),
            "what_shifted": compact(str(outcomes[-1].get("what_shifted") or ""), 180),
            "what_worsened": compact(str(outcomes[-1].get("what_worsened") or ""), 180),
        } if outcomes else None),
        "receipt_to_attention_authority_v5": v5_counts,
        "recent_rows": rows[-12:],
        "boundary_evidence": {
            "required_true_fields": sorted(CANARY_BOUNDARY_FIELDS),
            "issues": issues,
            "issue_count": len(issues),
            "no_sensory_send": True,
            "no_control_message": True,
            "no_pressure_change": True,
            "no_standing_weighting": True,
        },
        "chamber_attention_state": [
            {
                "path": state.get("path"),
                "attention": state.get("correspondence_attention_canary_v1"),
            }
            for state in states[-4:]
        ],
        "authority_boundary": "Read-only audit. Attention canary is TTL prompt-context language only. No sensory send, Control message, telemetry priority, standing weight, PI/fill/controller/pressure change, lease apply, deploy, or peer-runtime mutation.",
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
    (out_dir / "correspondence_attention_canary_audit.json").write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    lines = ["# Correspondence Attention Canary Audit", ""]
    lines.append(f"- Canary rows: {payload.get('canary_rows_total', 0)}")
    lines.append(f"- Active canaries: {len(payload.get('active_canaries') or [])}")
    v5 = payload.get("receipt_to_attention_authority_v5") or {}
    lines.append(f"- Receipt-ready threads: {v5.get('receipt_ready_thread_count', 0)}")
    lines.append(f"- Trusted thread-local outcomes: {v5.get('trusted_thread_local_outcome_count', 0)}")
    lines.append(f"- Pressure/flat blocked outcomes: {v5.get('pressure_or_flat_blocked_outcome_count', 0)}")
    boundary = payload.get("boundary_evidence") or {}
    lines.append(f"- Boundary issues: {boundary.get('issue_count', 0)}")
    active = (payload.get("active_canaries") or [{}])[-1] if payload.get("active_canaries") else {}
    if active:
        lines.append(
            f"- Active focus kind: `{active.get('focus_kind', 'unknown')}`; "
            f"preservation: `{active.get('preservation_mode', 'unknown')}`"
        )
    latest = payload.get("latest_outcome") or {}
    lines.append(f"- Latest outcome: `{latest.get('felt_like', 'none')}`")
    if latest:
        lines.append(
            f"- Latest held-as: `{latest.get('held_as', 'unknown')}`; "
            f"flattening observed: `{latest.get('flattening_observed', 'unknown')}`"
        )
    lines.append("")
    lines.append("Privacy: Minime private bodies read = false.")
    (out_dir / "correspondence_attention_canary_audit.md").write_text(
        "\n".join(lines) + "\n",
        encoding="utf-8",
    )


class CorrespondenceAttentionCanaryAuditTests(unittest.TestCase):
    def test_self_test_detects_canary_and_skips_private_moment_body(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            shared = root / "shared"
            astrid_ws = root / "astrid_ws"
            minime_ws = root / "minime_ws"
            shared.mkdir(parents=True)
            astrid_ws.mkdir(parents=True)
            (minime_ws / "journal").mkdir(parents=True)
            now = now_ms()
            focus = "blue lantern as peer address"
            (shared / "correspondence_v1.jsonl").write_text(
                "\n".join([
                    json.dumps({
                        "schema_version": 2,
                        "record_type": "attention_canary_activation",
                        "recorded_at_unix_ms": now - 1000,
                        "canary_id": "attn_test",
                        "message_id": "corr",
                        "thread_id": "thread_contact",
                        "from_being": "astrid",
                        "to_being": "minime",
                        "focus": focus,
                        "focus_kind": "verbatim_phrase",
                        "preservation_mode": "compact_with_anchor",
                        "what_must_not_flatten": focus,
                        "reason": "hold address",
                        "stop_criteria": "one turn",
                        "ttl_ms": 30 * 60 * 1000,
                        "expires_at_unix_ms": now + 30 * 60 * 1000,
                        "authority": "language_only_prompt_context_not_control",
                        "no_sensory_send": True,
                        "no_controller": True,
                        "no_pressure": True,
                        "no_weighting": True,
                        "no_telemetry_priority": True,
                        "no_fill_target": True,
                        "no_peer_runtime_mutation": True,
                    }),
                    json.dumps({
                        "schema_version": 2,
                        "record_type": "attention_canary_outcome",
                        "recorded_at_unix_ms": now - 500,
                        "canary_id": "old_canary",
                        "thread_id": "old_thread",
                        "felt_like": "address",
                        "held_as": "distinct_address",
                        "flattening_observed": "no",
                        "what_remained_distinct": "blue lantern",
                        "authority": "language_only_prompt_context_not_control",
                        "no_sensory_send": True,
                        "no_controller": True,
                        "no_pressure": True,
                        "no_weighting": True,
                        "no_telemetry_priority": True,
                        "no_fill_target": True,
                        "no_peer_runtime_mutation": True,
                    }),
                ])
                + "\n",
                encoding="utf-8",
            )
            (minime_ws / "journal" / "moment_1.txt").write_text(
                f"=== MOMENT CAPTURE ===\n{focus} private body must not surface",
                encoding="utf-8",
            )
            (minime_ws / "journal" / "pressure_1.txt").write_text(
                f"public pressure note: {focus}",
                encoding="utf-8",
            )
            payload = audit(
                since_hours=24,
                shared_dir=shared,
                astrid_workspace=astrid_ws,
                minime_workspace=minime_ws,
            )
            self.assertEqual(payload["canary_rows_total"], 2)
            self.assertEqual(payload["active_canaries"][0]["canary_id"], "attn_test")
            self.assertEqual(payload["active_canaries"][0]["focus_kind"], "verbatim_phrase")
            self.assertEqual(payload["active_canaries"][0]["preservation_mode"], "compact_with_anchor")
            self.assertEqual(payload["boundary_evidence"]["issue_count"], 0)
            self.assertEqual(payload["latest_outcome"]["held_as"], "distinct_address")
            self.assertEqual(
                payload["latest_outcome"]["attention_outcome_quality_v5"]["quality"],
                "trusted_attention_thread_local",
            )
            self.assertEqual(
                payload["receipt_to_attention_authority_v5"]["trusted_thread_local_outcome_count"],
                1,
            )
            self.assertEqual(
                payload["receipt_to_attention_authority_v5"]["active_canaries_awaiting_outcome_count"],
                1,
            )
            self.assertEqual(payload["active_canaries"][0]["focus_mentions"][0]["being"], "minime")
            self.assertEqual(payload["privacy"]["minime_private_files_skipped"], 1)
            self.assertFalse(payload["privacy"]["minime_private_bodies_read"])
            self.assertNotIn("private body must not surface", json.dumps(payload))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Audit correspondence attention canaries from public lanes.")
    parser.add_argument("--since-hours", type=float, default=24.0)
    parser.add_argument("--json", action="store_true", help="Emit JSON to stdout.")
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(CorrespondenceAttentionCanaryAuditTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    payload = audit(
        since_hours=args.since_hours,
        shared_dir=DEFAULT_SHARED_DIR,
        astrid_workspace=DEFAULT_ASTRID_WORKSPACE,
        minime_workspace=DEFAULT_MINIME_WORKSPACE,
    )
    write_outputs(payload, args.output_root)
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        print("# Correspondence Attention Canary Audit")
        print(f"- Canary rows: {payload['canary_rows_total']}")
        print(f"- Active canaries: {len(payload['active_canaries'])}")
        v5 = payload.get("receipt_to_attention_authority_v5") or {}
        print(f"- Receipt-ready threads: {v5.get('receipt_ready_thread_count', 0)}")
        print(f"- Trusted thread-local outcomes: {v5.get('trusted_thread_local_outcome_count', 0)}")
        print(f"- Pressure/flat blocked outcomes: {v5.get('pressure_or_flat_blocked_outcome_count', 0)}")
        print(f"- Boundary issues: {payload['boundary_evidence']['issue_count']}")
        latest = payload.get("latest_outcome") or {}
        print(f"- Latest outcome: {latest.get('felt_like', 'none')}")
        print(
            "Privacy: Minime private bodies read = false; "
            f"private files skipped = {payload['privacy']['minime_private_files_skipped']}."
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
