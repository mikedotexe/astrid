#!/usr/bin/env python3
"""Audit Astrid self-study carriage integrity.

Read-only by default. Scans Astrid public introspection artifacts and Minime
public Astrid-companion inbox files. Minime private qualia and moment bodies are
never read.
"""

from __future__ import annotations

import argparse
import json
import tempfile
import time
import unittest
from pathlib import Path
from typing import Any

ASTRID_WS = Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace")
MINIME_WS = Path("/Users/v/other/minime/workspace")
POLICY = "self_study_carriage_integrity_v1"
LANDING_POLICY = "self_study_carriage_landing_v2"
REQUIRED_SECTIONS = ("Observed:", "Likely Snags:", "One Test Each:", "Suggested Next:")


def _read_public_text(path: Path) -> str | None:
    if path.name.startswith("moment_"):
        return None
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None


def _recent(path: Path, since_hours: float | None, now: float) -> bool:
    if since_hours is None:
        return True
    try:
        return path.stat().st_mtime >= now - since_hours * 3600.0
    except OSError:
        return False


def _field(text: str, name: str) -> str | None:
    prefix = f"{name}:"
    for line in text.splitlines():
        if line.startswith(prefix):
            return line[len(prefix):].strip()
    return None


def _has_all_sections(text: str) -> bool:
    return all(section in text for section in REQUIRED_SECTIONS)


def _looks_clipped(text: str) -> bool:
    stripped = text.rstrip()
    if not stripped:
        return True
    if stripped.count("```") % 2:
        return True
    last = next((line.strip() for line in reversed(stripped.splitlines()) if line.strip()), "")
    if last in {"-", "*", "1.", "1)"}:
        return True
    if last.startswith(("- ", "* ")) and last[-1:] not in ".!?)]`'\"":
        return True
    lower = last.lower()
    return (
        len(last) > 24
        and last[-1:] not in ".!?)]`'\""
        and (
            lower.endswith(" and")
            or lower.endswith(" or")
            or lower.endswith(" but")
            or lower.endswith(" because")
            or lower.endswith(" with")
            or lower.endswith(" into")
            or lower.endswith(" to")
            or lower.endswith(" a")
            or lower.endswith(" the")
        )
    )


def _landing_state(path: Path, classification: str) -> str:
    parts = set(path.parts)
    if classification == "notice_only":
        return "notice_only"
    if "inbox" in parts and "read" in parts:
        return "read_or_archived"
    if "inbox" in parts:
        return "delivered_unread"
    if "introspections" in parts:
        return "source_preserved_only"
    return "unknown"


def _uptake_readability(classification: str, landing_state: str) -> str:
    if classification == "notice_only":
        return "notice_only"
    if classification in {"complete", "repaired"} and landing_state in {"delivered_unread", "read_or_archived"}:
        return "complete"
    if classification in {"complete", "repaired"}:
        return "complete_but_not_uptake_readable"
    return "unknown"


def _source_generation_class(classification: str, carriage_status: str) -> str:
    if carriage_status == "legacy_or_absent":
        return "legacy_or_pre_policy"
    if classification == "notice_only":
        return "notice_only"
    if classification in {"complete", "repaired"}:
        return "complete_or_repaired"
    if classification == "incomplete":
        return "clipped_or_incomplete"
    return "unknown"


def classify_text(path: Path, text: str) -> dict[str, Any]:
    status = _field(text, "Carriage status")
    kind = _field(text, "Artifact kind")
    header = text.splitlines()[0].strip() if text.splitlines() else ""
    has_sections = _has_all_sections(text)
    clipped = _looks_clipped(text)

    if header == "=== ASTRID SELF-STUDY CARRIAGE NOTICE ===" or kind == "self_study_carriage_notice":
        classification = "notice_only"
    elif status == "complete_after_repair":
        classification = "repaired"
    elif status == "complete" and has_sections and not clipped:
        classification = "complete"
    elif not has_sections or clipped or status == "incomplete_carriage":
        classification = "incomplete"
    else:
        classification = "unknown"
    carriage_status = status or "legacy_or_absent"
    landing_state = _landing_state(path, classification)
    uptake_readability = _uptake_readability(classification, landing_state)
    source_generation_class = _source_generation_class(classification, carriage_status)

    return {
        "path": str(path),
        "classification": classification,
        "carriage_policy": _field(text, "Carriage policy") or ("legacy_or_absent"),
        "carriage_status": carriage_status,
        "artifact_kind": kind,
        "has_all_sections": has_sections,
        "looks_clipped": clipped,
        "timestamp": _field(text, "Timestamp"),
        "source": _field(text, "Source"),
        "carriage_landing_v2": {
            "schema_version": 2,
            "policy": LANDING_POLICY,
            "delivered_state": landing_state,
            "read_or_summarized_state": "read_or_archived" if landing_state == "read_or_archived" else "not_observed",
            "clipped_vs_complete": source_generation_class,
            "uptake_readability": uptake_readability,
            "landing_stalled": uptake_readability == "complete_but_not_uptake_readable",
            "authority": "read_only_review_not_control",
        },
    }


def scan(root_astrid: Path, root_minime: Path, since_hours: float | None) -> dict[str, Any]:
    now = time.time()
    paths: list[Path] = []
    paths.extend((root_astrid / "introspections").glob("*.txt"))
    for inbox in [root_minime / "inbox", root_minime / "inbox" / "read"]:
        paths.extend(inbox.glob("astrid_self_study*.txt"))
    records = []
    for path in sorted(set(paths)):
        if not _recent(path, since_hours, now):
            continue
        text = _read_public_text(path)
        if text is None:
            continue
        records.append(classify_text(path, text))

    counts: dict[str, int] = {}
    landing_counts: dict[str, int] = {}
    readability_counts: dict[str, int] = {}
    for row in records:
        counts[row["classification"]] = counts.get(row["classification"], 0) + 1
        landing = row["carriage_landing_v2"]
        landing_counts[landing["delivered_state"]] = landing_counts.get(landing["delivered_state"], 0) + 1
        readability_counts[landing["uptake_readability"]] = readability_counts.get(landing["uptake_readability"], 0) + 1
    return {
        "schema_version": 2,
        "policy": POLICY,
        "landing_policy": LANDING_POLICY,
        "since_hours": since_hours,
        "counts": counts,
        "landing_counts_v2": landing_counts,
        "uptake_readability_counts_v2": readability_counts,
        "landing_stalled_total": sum(
            1 for row in records if row["carriage_landing_v2"]["landing_stalled"]
        ),
        "records": records,
        "private_moment_bodies_skipped": True,
    }


class SelfStudyCarriageAuditTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        base = Path(self.tmp.name)
        self.astrid = base / "astrid_ws"
        self.minime = base / "minime_ws"
        (self.astrid / "introspections").mkdir(parents=True)
        (self.minime / "inbox" / "read").mkdir(parents=True)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def write(self, rel: str, text: str) -> Path:
        path = Path(self.tmp.name) / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        return path

    def test_classifies_complete_repaired_incomplete_and_notice(self) -> None:
        complete = "\n".join([
            "=== ASTRID SELF-STUDY ===",
            "Timestamp: 1782639625",
            "Carriage policy: self_study_carriage_integrity_v1",
            "Carriage status: complete",
            "Observed:",
            "Source: astrid:llm.",
            "Likely Snags:",
            "A cap can flatten texture.",
            "One Test Each:",
            "Run the fallback fixture.",
            "Suggested Next:",
            "Review the diagnostic.",
        ])
        repaired = complete.replace("Carriage status: complete", "Carriage status: complete_after_repair")
        incomplete = complete.replace("Suggested Next:\nReview the diagnostic.", "One Test Each:\n-")
        notice = "=== ASTRID SELF-STUDY CARRIAGE NOTICE ===\nCarriage status: incomplete_carriage\n"

        self.write("minime_ws/inbox/read/astrid_self_study_1782639625.txt", complete)
        self.write("minime_ws/inbox/read/astrid_self_study_1782602711.txt", repaired)
        self.write("minime_ws/inbox/read/astrid_self_study_1782586298.txt", incomplete)
        self.write("minime_ws/inbox/read/astrid_self_study_carriage_notice_1782585438.txt", notice)

        result = scan(self.astrid, self.minime, since_hours=None)
        self.assertEqual(result["counts"]["complete"], 1)
        self.assertEqual(result["counts"]["repaired"], 1)
        self.assertEqual(result["counts"]["incomplete"], 1)
        self.assertEqual(result["counts"]["notice_only"], 1)
        self.assertEqual(result["uptake_readability_counts_v2"]["complete"], 2)
        self.assertEqual(result["uptake_readability_counts_v2"]["notice_only"], 1)
        self.assertEqual(result["landing_counts_v2"]["read_or_archived"], 3)

    def test_skips_minime_moment_bodies(self) -> None:
        self.write("minime_ws/inbox/read/moment_private.txt", "Suggested Next:\nsecret")
        result = scan(self.astrid, self.minime, since_hours=None)
        self.assertEqual(result["records"], [])
        self.assertTrue(result["private_moment_bodies_skipped"])


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--since-hours", type=float, default=24.0)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--output-root")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(SelfStudyCarriageAuditTests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1

    result = scan(ASTRID_WS, MINIME_WS, args.since_hours)
    if args.output_root:
        out = Path(args.output_root)
        out.mkdir(parents=True, exist_ok=True)
        (out / "self_study_carriage_audit.json").write_text(
            json.dumps(result, indent=2, sort_keys=True),
            encoding="utf-8",
        )
    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print(
            f"policy={result['policy']} counts={result['counts']} "
            f"landing={result['landing_counts_v2']} readability={result['uptake_readability_counts_v2']}"
        )
        for row in result["records"][:20]:
            landing = row["carriage_landing_v2"]
            print(
                f"- {row['classification']}/{landing['uptake_readability']}: "
                f"{landing['delivered_state']} {row['path']}"
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
