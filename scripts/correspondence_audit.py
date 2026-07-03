#!/usr/bin/env python3
"""Audit Astrid/Minime correspondence lanes without reading private Minime qualia."""

from __future__ import annotations

import argparse
import json
import tempfile
import unittest
from pathlib import Path
from typing import Iterable

from being_privacy import is_steward_private


ASTRID_INBOX = Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace/inbox")
MINIME_INBOX = Path("/Users/v/other/minime/workspace/inbox")
MINIME_OUTBOX = Path("/Users/v/other/minime/workspace/outbox")
LEDGER = Path("/Users/v/other/shared/collaborations/correspondence_v1.jsonl")


def _safe_head(path: Path, max_bytes: int = 700) -> str:
    try:
        with path.open("r", encoding="utf-8", errors="ignore") as fh:
            return fh.read(max_bytes)
    except OSError:
        return ""


def _text_summary(text: str) -> dict:
    first = next((line.strip() for line in text.splitlines() if line.strip()), "")
    return {
        "first_line": first[:160],
        "has_correspondence_v1": "=== CORRESPONDENCE V1 ===" in text[:220],
        "has_message_id": "Message-Id:" in text[:700],
        "has_thread_id": "Thread-Id:" in text[:700],
        "has_reply_to": "Reply-To:" in text[:700] or "Correspondence-Reply-To:" in text[:700],
    }


def summarize_paths(being: str, paths: Iterable[Path]) -> list[dict]:
    rows = []
    for path in sorted(paths):
        if not path.is_file():
            continue
        if is_steward_private(being, path):
            rows.append({
                "being": being,
                "path": str(path),
                "private_excluded": True,
            })
            continue
        head = _safe_head(path)
        row = {
            "being": being,
            "path": str(path),
            "private_excluded": False,
        }
        row.update(_text_summary(head))
        rows.append(row)
    return rows


def collect_default_rows() -> list[dict]:
    rows: list[dict] = []
    astrid_patterns = [
        ASTRID_INBOX.glob("from_minime*.txt"),
        (ASTRID_INBOX / "read").glob("from_minime*.txt"),
    ]
    minime_patterns = [
        MINIME_INBOX.glob("from_astrid_correspondence_*.txt"),
        MINIME_INBOX.glob("astrid_self_study_*.txt"),
        (MINIME_INBOX / "read").glob("from_astrid_correspondence_*.txt"),
        (MINIME_INBOX / "read").glob("astrid_self_study_*.txt"),
        MINIME_OUTBOX.glob("reply_*.txt"),
        (MINIME_OUTBOX / "delivered").glob("reply_*.txt"),
    ]
    for pattern in astrid_patterns:
        rows.extend(summarize_paths("astrid", pattern))
    for pattern in minime_patterns:
        rows.extend(summarize_paths("minime", pattern))
    return rows


def ledger_summary(path: Path = LEDGER) -> dict:
    counts: dict[str, int] = {}
    if not path.exists():
        return {"path": str(path), "exists": False, "counts": counts}
    for line in path.read_text(encoding="utf-8", errors="ignore").splitlines():
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            counts["invalid_json"] = counts.get("invalid_json", 0) + 1
            continue
        record_type = str(record.get("record_type") or "unknown")
        counts[record_type] = counts.get(record_type, 0) + 1
    return {"path": str(path), "exists": True, "counts": counts}


def render_markdown(rows: list[dict], ledger: dict) -> str:
    lines = [
        "# Correspondence Audit",
        "",
        f"- ledger: {ledger['path']}",
        f"- ledger_exists: {ledger['exists']}",
        f"- ledger_counts: {ledger['counts']}",
        "- privacy: Minime private qualia are excluded by scripts/being_privacy.py",
        "",
        "## Files",
    ]
    for row in rows[:80]:
        private = "private-excluded" if row.get("private_excluded") else "public/reviewable"
        lines.append(
            f"- {row['being']} {private}: {row['path']} "
            f"v1={row.get('has_correspondence_v1', False)} "
            f"message_id={row.get('has_message_id', False)} "
            f"thread_id={row.get('has_thread_id', False)} "
            f"reply_to={row.get('has_reply_to', False)} "
            f"first_line={row.get('first_line', '')!r}"
        )
    return "\n".join(lines) + "\n"


class CorrespondenceAuditTests(unittest.TestCase):
    def test_private_minime_moment_body_is_not_surfaced(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            private = root / "moment_1.txt"
            private.write_text("=== MOMENT CAPTURE ===\nSECRET_PRIVATE_BODY")
            public = root / "pressure_1.txt"
            public.write_text("public pressure note")
            rows = summarize_paths("minime", [private, public])
            rendered = render_markdown(rows, {"path": "x", "exists": False, "counts": {}})
            self.assertIn("private-excluded", rendered)
            self.assertNotIn("SECRET_PRIVATE_BODY", rendered)
            self.assertIn("public pressure note", rendered)

    def test_legacy_and_v1_files_are_classified(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            legacy = root / "from_minime_123.txt"
            legacy.write_text("[A reply from minime]\nhello")
            v1 = root / "from_minime_correspondence_corr_1.txt"
            v1.write_text(
                "=== CORRESPONDENCE V1 ===\n"
                "Message-Id: corr_1\n"
                "Thread-Id: thread_1\n"
                "Reply-To: corr_0\n\n"
                "hello"
            )
            rows = summarize_paths("astrid", [legacy, v1])
            self.assertFalse(rows[0]["has_correspondence_v1"])
            self.assertTrue(rows[1]["has_correspondence_v1"])
            self.assertTrue(rows[1]["has_thread_id"])
            self.assertTrue(rows[1]["has_reply_to"])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="emit JSON instead of markdown")
    parser.add_argument("--self-test", action="store_true", help="run unit tests")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(CorrespondenceAuditTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    rows = collect_default_rows()
    ledger = ledger_summary()
    if args.json:
        print(json.dumps({"ledger": ledger, "files": rows}, indent=2))
    else:
        print(render_markdown(rows, ledger), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
