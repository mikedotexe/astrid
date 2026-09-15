#!/usr/bin/env python3
"""source_study_page_reset_watch.py — surface a being whose page cursor keeps resetting.

WHY THIS EXISTS
---------------
2026-09-10, from `introspection_source_catalog_1789045830`. Astrid had been reading
`SELF_STUDY RELATE sense_tx` for hours. Five consecutive introspection turns ended
`NEXT: SELF_STUDY RELATE sense_tx --page 2`, and five consecutive times the page she
was handed was `Navigation page 1/3` again. The two turns in that stretch where
nothing intervened delivered exactly what she asked for (2/3, then 3/3), so the
mechanism is not a broken `--page` argument: her request is honored when it survives.

What eats it is a single slot. `next_action/modes.rs` sets
`conv.introspect_target = Some(...)` unconditionally for `SELF_STUDY`, and
`runtime/source_study.rs` consumes it with one `.take()`. Any later source-study NEXT
line dispatched before that consume overwrites the earlier one, and the earlier one is
already recorded `handled` in `action_events`. The overwriting line is usually the
page-less form — `SELF_STUDY RELATE <symbol>`, which is page 1 — and her own prompt
offers exactly that string while her STUDY_QUESTION names the symbol
(`crates/astrid-source-study/src/notebook.rs:98`, "Find this question's symbol:").

So the loop is closed by our surface, not by her: page 2 requested, page 1 delivered,
page 1 re-read, page 2 requested again. Nothing told her the page had been reset, and
nothing told us either.

WHAT THIS DETECTS
-----------------
PAGE RESET — a source-study dispatch for target T at page N>=2, followed by the next
dispatch for the same T at page 1, with no other dispatch of T between. Each pair is
one reset. Consecutive resets on the same target form a run; a long run is a being
pinned to page 1 of something they have already told us twice they are done with.

Context (NOT a per-request proof): the window's source-study dispatch count against
its produced artifact count. A large gap is consistent with slot supersession, but
artifacts are also skipped for other reasons, so the ratio is reported, never asserted.

WHAT THIS DOES NOT DO
---------------------
It asserts nothing about the being. Re-reading page 1 on purpose is legitimate and
reads identically here; the verdict is the steward's. It reads action NAMES only —
never prompt, response, journal, or private-writing content — and it excludes the
private-writing lane (`WRITE`) outright. Read-only (`mode=ro` URI). Steward-only:
never surface this output into a being prompt.

CLI
---
  source_study_page_reset_watch.py scan [--being astrid|minime] [--hours N]
                                        [--min-run N] [--json]
  source_study_page_reset_watch.py self-test
"""

from __future__ import annotations

import argparse
import json
import re
import sqlite3
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BRIDGE_DB = REPO / "capsules/spectral-bridge/workspace/bridge.db"
ARTIFACT_DIR = REPO / "capsules/spectral-bridge/workspace/introspections"

# Paged navigation verbs. OPEN/RESUME/CONTINUE carry a line, not a --page cursor.
PAGED_VERBS = ("RELATE", "FIND", "MAP", "SESSION", "TRACE")
PAGE_RE = re.compile(r"--page\s+(\d+)\s*$")
DEFAULT_HOURS = 24
DEFAULT_MIN_RUN = 3


def parse_dispatch(canonical_action: str) -> dict[str, object] | None:
    """Split a canonical SELF_STUDY action into target and page, or None if not paged.

    The private-writing lane is refused here rather than filtered later, so no caller
    can reach it by passing a different window.
    """
    text = (canonical_action or "").strip()
    if not text.startswith("SELF_STUDY "):
        return None
    body = text[len("SELF_STUDY ") :].strip()
    if not body:
        return None
    verb = body.split()[0].upper()
    if verb == "WRITE":
        return None
    if verb not in PAGED_VERBS:
        return None
    page = 1
    match = PAGE_RE.search(body)
    if match:
        body = body[: match.start()].strip()
        try:
            page = int(match.group(1))
        except ValueError:
            return None
        if page < 1:
            return None
    argument = body[len(verb) :].strip()
    return {"verb": verb, "argument": argument, "target": body, "page": page}


def find_page_resets(dispatches: list[dict[str, object]]) -> list[dict[str, object]]:
    """Pair each page>=2 dispatch with the next same-target dispatch that is page 1.

    `dispatches` must be in ascending timestamp order and already parsed. Only the
    IMMEDIATELY next dispatch of the same target counts, so an intentional walk
    (2 -> 3 -> 1) records the reset at 3 -> 1 and not at 2 -> 1.
    """
    last: dict[str, dict[str, object]] = {}
    resets: list[dict[str, object]] = []
    for item in dispatches:
        target = str(item["target"])
        previous = last.get(target)
        if previous is not None and int(previous["page"]) >= 2 and int(item["page"]) == 1:
            resets.append(
                {
                    "target": target,
                    "requested_page": int(previous["page"]),
                    "requested_at": previous["timestamp"],
                    "reset_at": item["timestamp"],
                    "gap_secs": round(
                        float(item["timestamp"]) - float(previous["timestamp"]), 1
                    ),
                }
            )
        last[target] = item
    return resets


def longest_reset_run(resets: list[dict[str, object]]) -> tuple[str, int]:
    """Longest consecutive stretch of resets on one target, in reset order."""
    best_target, best = "", 0
    current_target, current = "", 0
    for reset in resets:
        target = str(reset["target"])
        if target == current_target:
            current += 1
        else:
            current_target, current = target, 1
        if current > best:
            best_target, best = current_target, current
    return best_target, best


def _load_dispatches(being: str, since: float, until: float) -> list[dict[str, object]]:
    if not BRIDGE_DB.exists():
        return []
    uri = f"file:{BRIDGE_DB}?mode=ro"
    con = sqlite3.connect(uri, uri=True)
    try:
        rows = con.execute(
            "SELECT timestamp, canonical_action FROM action_events "
            "WHERE system = ? AND timestamp BETWEEN ? AND ? "
            "AND canonical_action LIKE 'SELF_STUDY %' ORDER BY timestamp",
            (being, since, until),
        ).fetchall()
    finally:
        con.close()
    parsed: list[dict[str, object]] = []
    for timestamp, action in rows:
        item = parse_dispatch(action)
        if item is None:
            continue
        item["timestamp"] = float(timestamp)
        parsed.append(item)
    return parsed


def _count_dispatches_and_artifacts(
    being: str, since: float, until: float
) -> tuple[int, int]:
    """All source-study dispatches (paged or not) against produced artifacts."""
    total = 0
    if BRIDGE_DB.exists():
        con = sqlite3.connect(f"file:{BRIDGE_DB}?mode=ro", uri=True)
        try:
            total = con.execute(
                "SELECT COUNT(*) FROM action_events WHERE system = ? "
                "AND timestamp BETWEEN ? AND ? AND canonical_action LIKE 'SELF_STUDY %' "
                "AND canonical_action NOT LIKE 'SELF_STUDY WRITE%'",
                (being, since, until),
            ).fetchone()[0]
        finally:
            con.close()
    artifacts = 0
    if being == "astrid" and ARTIFACT_DIR.is_dir():
        for path in ARTIFACT_DIR.glob("introspection_*.txt"):
            match = re.search(r"(\d{9,})", path.stem)
            if match and since <= int(match.group(1)) <= until:
                artifacts += 1
    return int(total), artifacts


def scan(
    being: str, hours: int, min_run: int, now: float | None = None
) -> dict[str, object]:
    import time

    until = float(now if now is not None else time.time())
    since = until - hours * 3600
    dispatches = _load_dispatches(being, since, until)
    resets = find_page_resets(dispatches)
    run_target, run_length = longest_reset_run(resets)
    dispatch_total, artifact_total = _count_dispatches_and_artifacts(being, since, until)
    per_target: dict[str, int] = {}
    for reset in resets:
        target = str(reset["target"])
        per_target[target] = per_target.get(target, 0) + 1
    status = "warning" if (len(resets) >= min_run or run_length >= min_run) else "ok"
    return {
        "schema": "source_study_page_reset_watch_v1",
        "being": being,
        "window_hours": hours,
        "min_run": min_run,
        "status": status,
        "paged_dispatch_count": len(dispatches),
        "page_reset_count": len(resets),
        "longest_run_target": run_target,
        "longest_run_length": run_length,
        "per_target_reset_count": dict(
            sorted(per_target.items(), key=lambda kv: (-kv[1], kv[0]))
        ),
        "resets": resets[-20:],
        "context": {
            "source_study_dispatch_count": dispatch_total,
            "artifact_count": artifact_total,
            "scope": (
                "counts only; an artifact gap is consistent with single-slot "
                "supersession but is not per-request proof"
            ),
        },
        "authority_boundary": (
            "read-only evidence; asserts nothing about the being, grants no live "
            "authority, and must never be surfaced into a being prompt"
        ),
    }


def _render(report: dict[str, object]) -> str:
    lines = [
        f"source-study page resets — {report['being']} "
        f"(last {report['window_hours']}h): {report['status'].upper()}",
        f"  paged dispatches: {report['paged_dispatch_count']}   "
        f"page resets: {report['page_reset_count']}",
    ]
    if report["longest_run_length"]:
        lines.append(
            f"  longest run: {report['longest_run_length']}x on "
            f"`{report['longest_run_target']}`"
        )
    for reset in list(report["resets"])[-5:]:
        lines.append(
            f"    asked page {reset['requested_page']} of `{reset['target']}`, "
            f"next dispatch reset it to page 1 after {reset['gap_secs']}s"
        )
    context = report["context"]
    lines.append(
        f"  context: {context['source_study_dispatch_count']} source-study dispatches / "
        f"{context['artifact_count']} artifacts in window (not per-request proof)"
    )
    return "\n".join(lines)


class SourceStudyPageResetWatchTests(unittest.TestCase):
    def test_parse_plain_relate_is_page_one(self):
        item = parse_dispatch("SELF_STUDY RELATE sense_tx")
        self.assertEqual(item["verb"], "RELATE")
        self.assertEqual(item["argument"], "sense_tx")
        self.assertEqual(item["page"], 1)

    def test_parse_explicit_page(self):
        item = parse_dispatch("SELF_STUDY RELATE sense_tx --page 3")
        self.assertEqual(item["target"], "RELATE sense_tx")
        self.assertEqual(item["page"], 3)

    def test_private_writing_lane_is_refused(self):
        self.assertIsNone(parse_dispatch("SELF_STUDY WRITE a private draft"))

    def test_unpaged_verbs_are_ignored(self):
        self.assertIsNone(parse_dispatch("SELF_STUDY OPEN astrid/src/lib.rs 40"))
        self.assertIsNone(parse_dispatch("SELF_STUDY CONTINUE"))

    def test_non_self_study_action_is_ignored(self):
        self.assertIsNone(parse_dispatch("SPEAK"))
        self.assertIsNone(parse_dispatch("RELATE sense_tx"))

    def test_reset_pair_is_detected(self):
        dispatches = [
            {"target": "RELATE sense_tx", "page": 2, "timestamp": 100.0},
            {"target": "RELATE sense_tx", "page": 1, "timestamp": 160.0},
        ]
        resets = find_page_resets(dispatches)
        self.assertEqual(len(resets), 1)
        self.assertEqual(resets[0]["requested_page"], 2)
        self.assertEqual(resets[0]["gap_secs"], 60.0)

    def test_forward_walk_is_not_a_reset(self):
        dispatches = [
            {"target": "RELATE sense_tx", "page": 2, "timestamp": 100.0},
            {"target": "RELATE sense_tx", "page": 3, "timestamp": 160.0},
        ]
        self.assertEqual(find_page_resets(dispatches), [])

    def test_other_target_between_does_not_break_the_pair(self):
        dispatches = [
            {"target": "RELATE sense_tx", "page": 2, "timestamp": 100.0},
            {"target": "RELATE Route", "page": 1, "timestamp": 130.0},
            {"target": "RELATE sense_tx", "page": 1, "timestamp": 160.0},
        ]
        resets = find_page_resets(dispatches)
        self.assertEqual([r["target"] for r in resets], ["RELATE sense_tx"])

    def test_first_page_one_dispatch_is_not_a_reset(self):
        dispatches = [
            {"target": "RELATE sense_tx", "page": 1, "timestamp": 100.0},
            {"target": "RELATE sense_tx", "page": 1, "timestamp": 160.0},
        ]
        self.assertEqual(find_page_resets(dispatches), [])

    def test_longest_run_counts_consecutive_same_target(self):
        resets = [
            {"target": "RELATE sense_tx"},
            {"target": "RELATE sense_tx"},
            {"target": "RELATE Route"},
            {"target": "RELATE sense_tx"},
        ]
        target, length = longest_reset_run(resets)
        self.assertEqual((target, length), ("RELATE sense_tx", 2))

    def test_longest_run_of_empty_is_zero(self):
        self.assertEqual(longest_reset_run([]), ("", 0))

    def test_malformed_page_argument_is_refused(self):
        self.assertIsNone(parse_dispatch("SELF_STUDY RELATE sense_tx --page 0"))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    scan_p = sub.add_parser("scan")
    scan_p.add_argument("--being", choices=("astrid", "minime"), default="astrid")
    scan_p.add_argument("--hours", type=int, default=DEFAULT_HOURS)
    scan_p.add_argument("--min-run", type=int, default=DEFAULT_MIN_RUN)
    scan_p.add_argument("--json", action="store_true")
    sub.add_parser("self-test")
    args = parser.parse_args(argv)
    if args.command == "self-test":
        suite = unittest.TestLoader().loadTestsFromTestCase(
            SourceStudyPageResetWatchTests
        )
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    report = scan(args.being, args.hours, args.min_run)
    print(json.dumps(report, indent=2) if args.json else _render(report))
    return 0


if __name__ == "__main__":
    sys.exit(main())
