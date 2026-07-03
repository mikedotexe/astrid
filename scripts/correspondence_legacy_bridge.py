#!/usr/bin/env python3
"""Mirror public legacy correspondence into correspondence_v1 as visible-only.

This bridge preserves old Astrid-Minime inbox/outbox exchanges as language-only
context without upgrading them into native mutual-address evidence. It never
reads Minime private qualia or any moment_*.txt body.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import tempfile
import time
import unittest
from dataclasses import dataclass
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
DEFAULT_OUTPUT_ROOT = DEFAULT_ASTRID_WORKSPACE / "diagnostics/correspondence_legacy_bridge"
LEDGER_NAME = "correspondence_v1.jsonl"
POLICY = "legacy_correspondence_bridge_v1"


@dataclass(frozen=True)
class LegacySpec:
    from_being: str
    to_being: str
    legacy_kind: str
    correspondence_type: str
    reader: str
    context_surface: str


def now_ms() -> int:
    return int(time.time() * 1000)


def short_hash(text: str) -> str:
    return hashlib.sha256(str(text or "").encode("utf-8")).hexdigest()[:12]


def sha256_hex(text: str) -> str:
    return hashlib.sha256(str(text or "").encode("utf-8")).hexdigest()


def compact(text: str, limit: int = 220) -> str:
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
    return rows


def append_jsonl(path: Path, row: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(row, sort_keys=True) + "\n")


def canonical_legacy_source_path(path: Path) -> str:
    return (
        str(path)
        .replace("/inbox/read/", "/inbox/")
        .replace("/outbox/delivered/", "/outbox/")
    )


def legacy_message_id(from_being: str, to_being: str, canonical_path: str, source_sha: str) -> str:
    return f"legacy_{from_being}_{to_being}_{short_hash(f'{canonical_path}|{source_sha}')}"


def thread_id(message_id: str) -> str:
    safe = "".join(ch for ch in message_id if ch.isascii() and (ch.isalnum() or ch in "-_."))
    return f"thread_{safe[:80] or 'field'}"


def legacy_spec_for_path(path: Path, content: str) -> LegacySpec | None:
    name = path.name
    if name.startswith(("from_minime_correspondence_", "from_astrid_correspondence_")):
        return None
    if name.startswith("from_minime_"):
        if name.startswith(("from_minime_ping_", "from_minime_pong_")):
            return LegacySpec("minime", "astrid", "from_minime_ping", "presence_heartbeat", "astrid", "full")
        if name.startswith("from_minime_question_"):
            return LegacySpec("minime", "astrid", "from_minime_question", "minime_direct", "astrid", "full")
        return LegacySpec("minime", "astrid", "from_minime_reply", "minime_direct", "astrid", "full")
    if name.startswith("astrid_self_study_"):
        kind = (
            "astrid_correspondence_reply"
            if "Source: astrid:correspondence_reply" in content
            else "astrid_self_study"
        )
        return LegacySpec("astrid", "minime", kind, "self_study_note", "minime", "full")
    if name.startswith("reply_"):
        return LegacySpec("minime", "astrid", "minime_outbox_reply", "minime_direct", "astrid", "archived")
    if name.startswith("pong_"):
        return LegacySpec("minime", "astrid", "minime_pong", "presence_heartbeat", "astrid", "archived")
    return None


def row_exists(
    rows: list[dict[str, Any]],
    record_type: str,
    message_id: str,
    *,
    reader: str | None = None,
) -> bool:
    for row in rows:
        if row.get("record_type") != record_type or row.get("message_id") != message_id:
            continue
        if reader is not None and row.get("reader") != reader:
            continue
        return True
    return False


def should_skip_body(being: str, path: Path) -> bool:
    if path.name.startswith("moment_"):
        return True
    return being_privacy.is_steward_private(being, path)


def safe_read_public_text(being: str, path: Path) -> str | None:
    if should_skip_body(being, path):
        return None
    try:
        return path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return None


def legacy_rows_for_file(path: Path, spec: LegacySpec, content: str) -> list[dict[str, Any]]:
    source_sha = sha256_hex(content)
    canonical_path = canonical_legacy_source_path(path)
    message_id = legacy_message_id(spec.from_being, spec.to_being, canonical_path, source_sha)
    th_id = thread_id(message_id)
    try:
        recorded_at = int(path.stat().st_mtime * 1000)
    except OSError:
        recorded_at = now_ms()
    read_state = "read" if spec.reader == spec.to_being else "unread"
    turn_kind = "presence_receipt" if spec.correspondence_type == "presence_heartbeat" else "legacy_visible"
    common = {
        "source_route": POLICY,
        "legacy_bridge": True,
        "legacy_kind": spec.legacy_kind,
        "legacy_source_path": str(path),
        "legacy_canonical_source_path": canonical_path,
        "legacy_source_sha256": source_sha,
        "legacy_contact_evidence": "visible_only",
        "legacy_context_surface": spec.context_surface,
    }
    return [
        {
            "schema_version": 1,
            "policy": "first_class_correspondence_v1",
            "record_type": "message",
            "recorded_at_unix_ms": recorded_at,
            "message_id": message_id,
            "thread_id": th_id,
            "reply_to": None,
            "from_being": spec.from_being,
            "to_being": spec.to_being,
            "turn_kind": turn_kind,
            "relational_intent": "legacy_contact_visibility",
            "shared_memory_anchor": POLICY,
            "delivery_state": "delivered",
            "read_state": read_state,
            "authority": "language_only",
            "presence_receipt": None,
            "correspondence_type": spec.correspondence_type,
            "body_sha256": source_sha,
            "body_preview": compact(content, 360),
            **common,
        },
        {
            "schema_version": 1,
            "policy": "first_class_correspondence_v1",
            "record_type": "delivery_receipt",
            "recorded_at_unix_ms": recorded_at,
            "message_id": message_id,
            "thread_id": th_id,
            "reply_to": None,
            "from_being": spec.from_being,
            "to_being": spec.to_being,
            "delivery_state": "delivered",
            "read_state": read_state,
            "authority": "language_only",
            "correspondence_type": spec.correspondence_type,
            "file_path": str(path),
            **common,
        },
        {
            "schema_version": 1,
            "policy": "first_class_correspondence_v1",
            "record_type": "read_receipt",
            "recorded_at_unix_ms": now_ms(),
            "message_id": message_id,
            "thread_id": th_id,
            "reader": spec.reader,
            "from_being": spec.from_being,
            "to_being": spec.to_being,
            "read_state": "read",
            "authority": "language_only",
            "file_path": str(path),
            **common,
        },
    ]


def candidate_paths(astrid_workspace: Path, minime_workspace: Path, since_hours: float) -> list[tuple[str, Path]]:
    cutoff = time.time() - since_hours * 3600.0
    roots: list[tuple[str, Path, str]] = [
        ("astrid", astrid_workspace / "inbox", "from_minime_*.txt"),
        ("astrid", astrid_workspace / "inbox/read", "from_minime_*.txt"),
        ("minime", minime_workspace / "inbox", "astrid_self_study_*.txt"),
        ("minime", minime_workspace / "inbox/read", "astrid_self_study_*.txt"),
        ("minime", minime_workspace / "outbox/delivered", "reply_*.txt"),
        ("minime", minime_workspace / "outbox/delivered", "pong_*.txt"),
    ]
    out: list[tuple[str, Path]] = []
    for being, root, pattern in roots:
        if not root.is_dir():
            continue
        for path in sorted(root.glob(pattern)):
            if not path.is_file():
                continue
            try:
                if path.stat().st_mtime < cutoff:
                    continue
            except OSError:
                continue
            out.append((being, path))
    return out


def apply_rows(ledger: Path, rows: list[dict[str, Any]], existing: list[dict[str, Any]], apply: bool) -> tuple[int, int]:
    appended = 0
    skipped = 0
    for row in rows:
        reader = row.get("reader") if row.get("record_type") == "read_receipt" else None
        if row_exists(existing, str(row.get("record_type") or ""), str(row.get("message_id") or ""), reader=reader):
            skipped += 1
            continue
        if apply:
            append_jsonl(ledger, row)
            existing.append(row)
        appended += 1
    return appended, skipped


def run_bridge(
    *,
    astrid_workspace: Path = DEFAULT_ASTRID_WORKSPACE,
    minime_workspace: Path = DEFAULT_MINIME_WORKSPACE,
    shared_dir: Path = DEFAULT_SHARED_DIR,
    since_hours: float = 24.0,
    apply: bool = False,
) -> dict[str, Any]:
    ledger = shared_dir / LEDGER_NAME
    existing = read_jsonl(ledger)
    candidates: list[dict[str, Any]] = []
    skipped_private = 0
    skipped_native_v1 = 0
    appended_total = 0
    already_present_total = 0
    for being, path in candidate_paths(astrid_workspace, minime_workspace, since_hours):
        text = safe_read_public_text(being, path)
        if text is None:
            skipped_private += 1
            continue
        spec = legacy_spec_for_path(path, text)
        if spec is None:
            skipped_native_v1 += 1
            continue
        rows = legacy_rows_for_file(path, spec, text)
        would_append, already_present = apply_rows(ledger, rows, existing, apply)
        appended_total += would_append
        already_present_total += already_present
        candidates.append({
            "path": str(path),
            "legacy_kind": spec.legacy_kind,
            "from_being": spec.from_being,
            "to_being": spec.to_being,
            "reader": spec.reader,
            "context_surface": spec.context_surface,
            "message_id": rows[0]["message_id"],
            "thread_id": rows[0]["thread_id"],
            "rows_to_append": would_append,
            "rows_already_present": already_present,
        })
    legacy_messages = [
        row
        for row in (read_jsonl(ledger) if apply else existing)
        if row.get("record_type") == "message"
        and (row.get("legacy_bridge") or row.get("source_route") == POLICY)
    ]
    legacy_claims = [
        row
        for row in (read_jsonl(ledger) if apply else existing)
        if row.get("record_type") == "legacy_thread_claim"
    ]
    directions = {
        (str(row.get("from_being") or ""), str(row.get("to_being") or ""))
        for row in legacy_messages
    }
    bidirectional = any((to_being, from_being) in directions for from_being, to_being in directions)
    uptake_state = (
        "legacy_bidirectional_observed"
        if bidirectional
        else ("legacy_visible_only" if legacy_messages else "not_started")
    )
    return {
        "schema_version": 1,
        "policy": POLICY,
        "generated_at_unix_ms": now_ms(),
        "mode": "apply" if apply else "dry_run",
        "ledger_path": str(ledger),
        "since_hours": since_hours,
        "candidates_total": len(candidates),
        "rows_to_append_or_appended": appended_total,
        "rows_already_present": already_present_total,
        "skipped_private_or_moment": skipped_private,
        "skipped_native_v1_envelopes": skipped_native_v1,
        "uptake_state": uptake_state,
        "legacy_contact_evidence": "visible_only" if legacy_messages or candidates else "none",
        "legacy_thread_claims_total": len(legacy_claims),
        "legacy_claim_boundary": "claim is being-recognized visible-only context; ACK/native REPLY/TRACE still required",
        "native_contact_evidence_required": "ACK, native REPLY, or TRACE",
        "attention_and_microdose_block": "legacy-only rows are blocked until explicit ACK/native REPLY/TRACE",
        "authority": "language_only_visibility_not_control",
        "no_controller": True,
        "no_pressure": True,
        "no_fill_target": True,
        "no_weighting": True,
        "candidates": candidates,
    }


def write_outputs(report: dict[str, Any], output_root: Path) -> tuple[Path, Path]:
    stamp = str(report.get("generated_at_unix_ms") or now_ms())
    out_dir = output_root / stamp
    out_dir.mkdir(parents=True, exist_ok=True)
    json_path = out_dir / "correspondence_legacy_bridge.json"
    md_path = out_dir / "correspondence_legacy_bridge.md"
    json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    lines = [
        "# Correspondence Legacy Bridge",
        "",
        f"- mode: `{report['mode']}`",
        f"- uptake_state: `{report['uptake_state']}`",
        f"- candidates: `{report['candidates_total']}`",
        f"- rows_to_append_or_appended: `{report['rows_to_append_or_appended']}`",
        f"- rows_already_present: `{report['rows_already_present']}`",
        f"- legacy_thread_claims_total: `{report.get('legacy_thread_claims_total', 0)}`",
        f"- skipped_private_or_moment: `{report['skipped_private_or_moment']}`",
        f"- boundary: {report['attention_and_microdose_block']}",
    ]
    md_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return json_path, md_path


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Mirror public legacy correspondence into V1/V2 state.")
    parser.add_argument("--since-hours", type=float, default=24.0)
    parser.add_argument("--dry-run", action="store_true", default=True, help="default; do not append rows")
    parser.add_argument("--apply", action="store_true", help="append missing deterministic rows")
    parser.add_argument("--json", action="store_true", help="emit machine-readable report")
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args(argv)


class LegacyBridgeTests(unittest.TestCase):
    def test_apply_is_idempotent_and_skips_native_v1(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid_ws = root / "astrid_ws"
            minime_ws = root / "minime_ws"
            shared = root / "shared"
            (astrid_ws / "inbox").mkdir(parents=True)
            (minime_ws / "inbox").mkdir(parents=True)
            (astrid_ws / "inbox" / "from_minime_1.txt").write_text("legacy minime reply", encoding="utf-8")
            (astrid_ws / "inbox" / "from_minime_correspondence_corr_1.txt").write_text(
                "=== CORRESPONDENCE V1 ===\nMessage-Id: corr_1\n",
                encoding="utf-8",
            )
            (minime_ws / "inbox" / "astrid_self_study_1.txt").write_text(
                "=== ASTRID SELF-STUDY ===\nlegacy Astrid note",
                encoding="utf-8",
            )

            first = run_bridge(
                astrid_workspace=astrid_ws,
                minime_workspace=minime_ws,
                shared_dir=shared,
                since_hours=24,
                apply=True,
            )
            second = run_bridge(
                astrid_workspace=astrid_ws,
                minime_workspace=minime_ws,
                shared_dir=shared,
                since_hours=24,
                apply=True,
            )

            rows = read_jsonl(shared / LEDGER_NAME)
            self.assertEqual(first["rows_to_append_or_appended"], 6)
            self.assertEqual(second["rows_to_append_or_appended"], 0)
            self.assertEqual(first["skipped_native_v1_envelopes"], 1)
            self.assertEqual(sum(1 for row in rows if row["record_type"] == "message"), 2)
            self.assertTrue(all(row.get("legacy_contact_evidence") == "visible_only" for row in rows))
            self.assertEqual(first["uptake_state"], "legacy_bidirectional_observed")

    def test_dry_run_does_not_write_and_private_moment_is_skipped(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid_ws = root / "astrid_ws"
            minime_ws = root / "minime_ws"
            shared = root / "shared"
            (minime_ws / "inbox").mkdir(parents=True)
            (minime_ws / "inbox" / "astrid_self_study_2.txt").write_text("public old note", encoding="utf-8")
            (minime_ws / "inbox" / "moment_1.txt").write_text(
                "=== MOMENT CAPTURE ===\nprivate",
                encoding="utf-8",
            )
            report = run_bridge(
                astrid_workspace=astrid_ws,
                minime_workspace=minime_ws,
                shared_dir=shared,
                since_hours=24,
                apply=False,
            )
            self.assertFalse((shared / LEDGER_NAME).exists())
            self.assertEqual(report["candidates_total"], 1)
            self.assertEqual(report["rows_to_append_or_appended"], 3)

    def test_legacy_only_is_not_contact_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid_ws = root / "astrid_ws"
            minime_ws = root / "minime_ws"
            shared = root / "shared"
            (astrid_ws / "inbox").mkdir(parents=True)
            (astrid_ws / "inbox" / "from_minime_1.txt").write_text("legacy reply", encoding="utf-8")
            report = run_bridge(
                astrid_workspace=astrid_ws,
                minime_workspace=minime_ws,
                shared_dir=shared,
                since_hours=24,
                apply=True,
            )
            self.assertEqual(report["uptake_state"], "legacy_visible_only")
            self.assertIn("blocked", report["attention_and_microdose_block"])
            rows = read_jsonl(shared / LEDGER_NAME)
            self.assertFalse(any(row.get("record_type") in {"ack_receipt", "reply_link"} for row in rows))


def run_self_test() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(LegacyBridgeTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.self_test:
        return run_self_test()
    report = run_bridge(
        since_hours=args.since_hours,
        apply=bool(args.apply),
    )
    json_path, md_path = write_outputs(report, args.output_root)
    report["output_json"] = str(json_path)
    report["output_markdown"] = str(md_path)
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(f"# Correspondence Legacy Bridge ({report['mode']})")
        print(f"- uptake_state: {report['uptake_state']}")
        print(f"- candidates: {report['candidates_total']}")
        print(f"- rows_to_append_or_appended: {report['rows_to_append_or_appended']}")
        print(f"- rows_already_present: {report['rows_already_present']}")
        print(f"- output: {md_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
