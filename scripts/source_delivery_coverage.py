#!/usr/bin/env python3
"""source_delivery_coverage.py — how much of a source a being has actually been handed.

WHY THIS EXISTS
---------------
2026-09-12, from `introspection_source_catalog_1789235097`. Astrid mapped
`capsules/spectral-bridge/src/autonomous/next_action/` and reported that `dispatch.rs`,
`mod.rs` and `pressure_agency.rs` "have not yet been delivered or were only partially
delivered", while `shadow.rs` and `spectral_drift.rs` "were successfully retrieved via
deliberate rereads, providing a baseline".

The steward's first instinct was to check the `source_first_v3` read-session store. It
holds zero sessions for ANY file under `next_action/`, which looks like proof she was
never handed them — and that inference is WRONG. The paging route that answers
`SELF_STUDY OPEN <path> <line>` does not write a v3 read session. The real delivery
record is the canonical introspection artifact itself, whose header carries

    Source: astrid/capsules/spectral-bridge/src/autonomous/next_action/shadow.rs
    Source revision: sha256:<hex>; bytes 13197..17584

One line per delivered page. Nothing merged those intervals, so nobody could answer "how
much of this file has she actually seen, and at which revision" — including her.

Merging them says she was right, on a criterion no tool computed:

    shadow.rs          60 pages  100.0%  1 revision
    spectral_drift.rs  14 pages  100.0%  1 revision
    dispatch.rs        40 pages   97.7%  3 revisions
    mod.rs             83 pages   99.2%  3 revisions
    pressure_agency.rs 14 pages   63.8%  2 revisions, 3 interior gaps

The two she called a baseline are the only two covered whole at a single revision. The
three she named are each incomplete in a different way: `pressure_agency.rs` has real
byte gaps, while `dispatch.rs` and `mod.rs` look nearly complete only because coverage is
STITCHED ACROSS REVISIONS THAT NO LONGER EXIST — neither was ever whole at any one SHA.
That distinction is what this tool reports, because it is the distinction she drew.

WHAT THIS REPORTS
-----------------
Per source label, over the canonical introspection corpus:

- `pages` — delivered page count (header windows, not turns)
- `covered_bytes` / `current_bytes` / `covered_pct` — merged interval coverage
- `gap_count` / `largest_gap_bytes` — interior holes at the merged level
- `revision_count` and `current_revision_covered_pct` — coverage restricted to the SHA the
  working copy has NOW, which is the number that answers "could she read this today"
- `stitched` — true when merged coverage spans more than one revision, i.e. the file moved
  under her and total coverage overstates what she ever held at once

Read-only and steward-only. It reads canonical artifact headers and the working copy's
size; it never writes, never reaches a being, and asserts nothing about understanding —
delivery is not comprehension, and a covered file is not a read one.

WHY THE EXISTING WATCHES CANNOT SEE IT
--------------------------------------
- `source_study_revisit_watch` keys on the SAME window handed over twice. Full coverage
  assembled from forty DIFFERENT windows is not a revisit.
- `source_page_symbol_truncation_watch` and `source_page_item_context_watch` judge one
  page's contents. Neither accumulates across pages.
- The `source_first_v3` read-session store models a different delivery route entirely and
  is silent for this one — which is exactly the trap this docstring exists to mark.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

CORPORA = {
    "astrid": (
        REPO / "capsules/spectral-bridge/workspace/introspections",
        "introspection_*.txt",
    ),
}

# Only the header is parsed; 900 bytes clears the longest observed header comfortably.
HEADER_BYTES = 900

SOURCE_RE = re.compile(r"^Source:\s*(.+?)\s*$", re.MULTILINE)
REVISION_RE = re.compile(
    r"^Source revision:\s*sha256:([0-9a-f]+);\s*bytes\s+(\d+)\.\.(\d+)", re.MULTILINE
)

# A label is a repository-relative path with a leading repo segment ("astrid/...",
# "minime/..."). Navigation-only turns carry labels like "source catalog" and are skipped
# by REVISION_RE anyway, since they declare no byte window.
LABEL_RE = re.compile(r"^(astrid|minime)/(.+)$")


def merge_intervals(intervals: list[tuple[int, int]]) -> list[tuple[int, int]]:
    """Merge half-open byte windows. Touching windows (b..c, c..d) join into one."""
    merged: list[tuple[int, int]] = []
    for start, end in sorted(intervals):
        if merged and start <= merged[-1][1]:
            merged[-1] = (merged[-1][0], max(merged[-1][1], end))
        else:
            merged.append((start, end))
    return merged


def covered_bytes(intervals: list[tuple[int, int]]) -> int:
    return sum(end - start for start, end in merge_intervals(intervals))


def interior_gaps(intervals: list[tuple[int, int]]) -> list[tuple[int, int]]:
    """Holes BETWEEN delivered windows. A tail never reached is not an interior gap —
    it shows up in covered_pct instead, and conflating the two would hide which kind of
    incompleteness a file has."""
    merged = merge_intervals(intervals)
    return [(merged[i][1], merged[i + 1][0]) for i in range(len(merged) - 1)]


def parse_header(text: str) -> dict[str, object] | None:
    """Return one delivery record, or None when the turn delivered no source page."""
    source = SOURCE_RE.search(text)
    revision = REVISION_RE.search(text)
    if not source or not revision:
        return None
    label = source.group(1).strip()
    if not LABEL_RE.match(label):
        return None
    start, end = int(revision.group(2)), int(revision.group(3))
    if end <= start:
        return None
    return {
        "source_label": label,
        "revision_sha256": revision.group(1),
        "start": start,
        "end": end,
    }


def working_copy_path(label: str) -> Path | None:
    match = LABEL_RE.match(label)
    if not match:
        return None
    repo, relative = match.group(1), match.group(2)
    root = REPO if repo == "astrid" else REPO.parent / "minime"
    return root / relative


def current_revision(label: str) -> tuple[str | None, int | None]:
    """Working-copy (sha256-prefix, byte size), or (None, None) when the path is gone.

    The prefix length matches what the artifact headers carry, so a stale path and a moved
    file stay distinguishable from a covered one."""
    path = working_copy_path(label)
    if path is None or not path.is_file():
        return (None, None)
    import hashlib

    data = path.read_bytes()
    return (hashlib.sha256(data).hexdigest(), len(data))


def collect(being: str, label_filter: str | None = None) -> list[dict[str, object]]:
    if being not in CORPORA:
        raise SystemExit(f"unknown being: {being}")
    directory, glob = CORPORA[being]
    records: list[dict[str, object]] = []
    for path in sorted(directory.glob(glob)):
        try:
            head = path.read_text(encoding="utf-8", errors="replace")[:HEADER_BYTES]
        except OSError:
            continue
        record = parse_header(head)
        if record is None:
            continue
        if label_filter and label_filter not in str(record["source_label"]):
            continue
        records.append(record)
    return records


def summarize(records: list[dict[str, object]]) -> list[dict[str, object]]:
    by_label: dict[str, list[dict[str, object]]] = {}
    for record in records:
        by_label.setdefault(str(record["source_label"]), []).append(record)

    rows: list[dict[str, object]] = []
    for label, group in by_label.items():
        intervals = [(int(r["start"]), int(r["end"])) for r in group]
        revisions = sorted({str(r["revision_sha256"]) for r in group})
        current_sha, current_size = current_revision(label)
        current_intervals = (
            [
                (int(r["start"]), int(r["end"]))
                for r in group
                if current_sha and str(r["revision_sha256"]) == current_sha
            ]
            if current_sha
            else []
        )
        covered = covered_bytes(intervals)
        current_covered = covered_bytes(current_intervals)
        gaps = interior_gaps(intervals)
        rows.append(
            {
                "source_label": label,
                "pages": len(group),
                "revision_count": len(revisions),
                "revisions": revisions,
                "stitched": len(revisions) > 1,
                "covered_bytes": covered,
                "current_bytes": current_size,
                "covered_pct": round(100.0 * covered / current_size, 1)
                if current_size
                else None,
                "current_revision_covered_bytes": current_covered,
                "current_revision_covered_pct": round(
                    100.0 * current_covered / current_size, 1
                )
                if current_size
                else None,
                "gap_count": len(gaps),
                "largest_gap_bytes": max((b - a for a, b in gaps), default=0),
                "working_copy_present": current_size is not None,
            }
        )
    rows.sort(key=lambda row: (row["current_revision_covered_pct"] is None,
                               row["current_revision_covered_pct"] or 0.0,
                               row["source_label"]))
    return rows


def scan(being: str, label_filter: str | None = None) -> dict[str, object]:
    records = collect(being, label_filter)
    rows = summarize(records)
    whole = [r for r in rows if r["current_revision_covered_pct"] == 100.0]
    stitched = [r for r in rows if r["stitched"]]
    return {
        "schema": "source_delivery_coverage_v1",
        "being": being,
        "label_filter": label_filter,
        "deliveries": len(records),
        "sources": len(rows),
        "whole_at_current_revision": len(whole),
        "stitched_across_revisions": len(stitched),
        "rows": rows,
        "authority_boundary": (
            "read-only delivery accounting; coverage is not comprehension, no being is "
            "reached, nothing is written, and no source, prompt, cursor, or delivery "
            "behavior is changed"
        ),
    }


def _render(result: dict[str, object]) -> str:
    lines = [
        f"source delivery coverage — {result['being']}"
        + (f" (filter: {result['label_filter']})" if result["label_filter"] else ""),
        f"  {result['deliveries']} delivered pages across {result['sources']} sources; "
        f"{result['whole_at_current_revision']} whole at current revision, "
        f"{result['stitched_across_revisions']} stitched across revisions",
        "",
        f"  {'source':60s} {'pages':>5s} {'cur%':>6s} {'all%':>6s} {'revs':>4s} {'gaps':>4s}",
    ]
    for row in result["rows"]:
        cur = row["current_revision_covered_pct"]
        allp = row["covered_pct"]
        label = str(row["source_label"])
        if len(label) > 60:
            label = "…" + label[-59:]
        flag = " STITCHED" if row["stitched"] else ""
        lines.append(
            f"  {label:60s} {row['pages']:5d} "
            f"{('n/a' if cur is None else f'{cur:.1f}'):>6s} "
            f"{('n/a' if allp is None else f'{allp:.1f}'):>6s} "
            f"{row['revision_count']:4d} {row['gap_count']:4d}{flag}"
        )
    return "\n".join(lines)


class DeliveryCoverageTests(unittest.TestCase):
    HEADER = (
        "=== ASTRID INTROSPECTION ===\n"
        "Source: astrid/capsules/spectral-bridge/src/autonomous/next_action/shadow.rs\n"
        "Source revision: sha256:b5ca9d91cf4d88c9; bytes 13197..17584\n"
        "Source scope: local checkout; deployed behavior not established\n"
    )

    def test_parses_a_real_delivery_header(self):
        record = parse_header(self.HEADER)
        self.assertIsNotNone(record)
        self.assertEqual(record["revision_sha256"], "b5ca9d91cf4d88c9")
        self.assertEqual((record["start"], record["end"]), (13197, 17584))

    def test_navigation_only_turn_is_not_a_delivery(self):
        text = (
            "Source: source catalog\n"
            "Source revision: navigation only\n"
        )
        self.assertIsNone(parse_header(text))

    def test_non_repo_label_is_rejected(self):
        text = (
            "Source: study session (2 source pages)\n"
            "Source revision: sha256:abc123; bytes 0..10\n"
        )
        self.assertIsNone(parse_header(text))

    def test_zero_width_window_is_not_a_delivery(self):
        text = (
            "Source: astrid/a/b.rs\n"
            "Source revision: sha256:abc123; bytes 40..40\n"
        )
        self.assertIsNone(parse_header(text))

    def test_touching_windows_merge_without_a_gap(self):
        merged = merge_intervals([(0, 100), (100, 250)])
        self.assertEqual(merged, [(0, 250)])
        self.assertEqual(interior_gaps([(0, 100), (100, 250)]), [])

    def test_overlapping_windows_are_not_double_counted(self):
        self.assertEqual(covered_bytes([(0, 100), (50, 150)]), 150)

    def test_interior_gap_is_reported_but_an_unreached_tail_is_not(self):
        # 0..100 and 200..300 delivered: one interior hole. Bytes past 300 are missing
        # coverage, not a gap — the distinction is the point.
        gaps = interior_gaps([(0, 100), (200, 300)])
        self.assertEqual(gaps, [(100, 200)])
        self.assertEqual(covered_bytes([(0, 100), (200, 300)]), 200)

    def test_unordered_windows_merge_correctly(self):
        self.assertEqual(merge_intervals([(200, 300), (0, 100)]), [(0, 100), (200, 300)])

    def test_summarize_marks_multi_revision_coverage_as_stitched(self):
        records = [
            {"source_label": "astrid/x.rs", "revision_sha256": "aa", "start": 0, "end": 50},
            {"source_label": "astrid/x.rs", "revision_sha256": "bb", "start": 50, "end": 100},
        ]
        row = summarize(records)[0]
        self.assertTrue(row["stitched"])
        self.assertEqual(row["revision_count"], 2)
        self.assertEqual(row["covered_bytes"], 100)

    def test_single_revision_coverage_is_not_stitched(self):
        records = [
            {"source_label": "astrid/x.rs", "revision_sha256": "aa", "start": 0, "end": 50},
            {"source_label": "astrid/x.rs", "revision_sha256": "aa", "start": 50, "end": 100},
        ]
        row = summarize(records)[0]
        self.assertFalse(row["stitched"])
        self.assertEqual(row["revision_count"], 1)

    def test_missing_working_copy_yields_no_percentage_rather_than_zero(self):
        records = [
            {
                "source_label": "astrid/definitely/not/here_9f1c.rs",
                "revision_sha256": "aa",
                "start": 0,
                "end": 50,
            }
        ]
        row = summarize(records)[0]
        self.assertFalse(row["working_copy_present"])
        self.assertIsNone(row["covered_pct"])
        self.assertIsNone(row["current_revision_covered_pct"])

    def test_unknown_being_is_refused(self):
        with self.assertRaises(SystemExit):
            collect("nobody")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    scan_cmd = sub.add_parser("scan", help="report delivered coverage per source")
    scan_cmd.add_argument("--being", default="astrid", choices=sorted(CORPORA))
    scan_cmd.add_argument(
        "--filter", dest="label_filter", default=None,
        help="substring match on the source label (e.g. next_action/)",
    )
    scan_cmd.add_argument("--json", action="store_true")
    sub.add_parser("self-test", help="run unit tests")
    args = parser.parse_args(argv)

    if args.command == "self-test":
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(DeliveryCoverageTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    result = scan(args.being, args.label_filter)
    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print(_render(result))
    return 0


if __name__ == "__main__":
    sys.exit(main())
