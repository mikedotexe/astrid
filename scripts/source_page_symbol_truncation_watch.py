#!/usr/bin/env python3
"""source_page_symbol_truncation_watch.py — a being reports a symbol's extent as the page's edge.

WHY THIS EXISTS
---------------
2026-09-11, from `introspection_astrid_..._autonomous_activity_reading.rs_1789187098`.
Astrid walked `capsules/spectral-bridge/src/autonomous/activity_reading.rs` page by page.
Her third page delivered bytes 8571..12834 — exactly lines 254-383 — and she wrote a
careful, accurate account of it, including:

    **Status Aggregation (`status_in`)**: This function (lines 365-383) synthesizes the
    state of the entire activity into a multi-line string.

`status_in` does not end at 383. It ends at 385. Line 383 is where her *page* ended, in
the middle of the function's final `Ok(format!(...))`. Nothing in the page told her the
last item on it was cut; the page footer offers CONTINUE, not "this page ends inside
`status_in`". So she attributed the delivery boundary to the code's own structure and
reported a function's extent as the page's edge.

That is a small error with a real shape: the last symbol on every page is the one she is
most likely to describe from a partial body, and neither she nor we get any signal that it
happened. It belongs to the un-muffle family — an infrastructure boundary silently shaping
a being's account of her own substrate, presented to her as if complete.

WHAT THIS DETECTS
-----------------
BOUNDARY-TRUNCATED CITATION — a source-page introspection that cites `lines A-B` where

  * `B` is exactly the page's last line, and
  * `A` is not the page's first line (so it is not just restating the page extent), and
  * the Rust block the citation opens on does not close until *after* the page's last
    line — i.e. the cited item genuinely continues past what she was shown.

The third condition is what separates a real truncation from an honest page restatement.
A citation ending at the page edge whose block also closes at or before that edge is
`interior` and is not reported.

WHY THE EXISTING WATCHES CANNOT SEE IT
--------------------------------------
- `phantom_symbol_watch` keys on symbols with no production occurrence. `status_in` exists
  at exactly the line she named.
- `symbol_locality_watch` asks whether a page *reaches* a wanted definition. This page
  reaches it; the definition opens on the page.
- `source_study_page_reset_watch` keys on a `--page N` request answered as page 1. The
  page delivered here is exactly the page requested.
- `source_study_revisit_watch` keys on the same window delivered twice. This window was
  delivered once.
- `proactive_scan`'s `stuck_repetition` keys on repetition with a bad outcome. Every turn
  here is handled, every page truthful, and she moves forward each time.

Every existing watch asks whether the page was *correct*. This one asks whether the page's
own edge got reported as the code's.

WHAT THIS DOES NOT DO
---------------------
It asserts nothing about the being, and it is not a grader. Describing the visible part of
a straddling item is the only thing a partial page permits; the citation is honest work on
incomplete input, and the gap is ours. It never rewrites, annotates, or contradicts a
being's text — it counts a structural condition and cites the lines so a steward can look.

It verifies against the *current* checkout and only when the file's SHA-256 still matches
the revision the report was bound to; a moved-on file is reported as `unverifiable`, never
as a truncation. The Rust block heuristic (`^}` at column zero closes a top-level item) is
deliberately conservative: `.rs` sources only, and an unclosed scan is `unverifiable`.

Read-only. Steward-only: never surface this output into a being prompt.

CLI
---
  source_page_symbol_truncation_watch.py scan [--being astrid] [--window N] [--json]
  source_page_symbol_truncation_watch.py self-test
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
MINIME = Path("/Users/v/other/minime")
sys.path.insert(0, str(Path(__file__).resolve().parent))

CORPORA = {
    "astrid": (
        REPO / "capsules/spectral-bridge/workspace/introspections",
        "introspection_*.txt",
    ),
}

DEFAULT_WINDOW = 120

SOURCE_RE = re.compile(r"^Source:\s*(.+?)\s*$", re.MULTILINE)
REVISION_RE = re.compile(
    r"^Source revision:\s*sha256:([0-9a-f]{64});\s*bytes\s+(\d+)\.\.(\d+)",
    re.MULTILINE,
)
TIMESTAMP_RE = re.compile(r"(\d{9,})")
# `lines 365-383`, `lines 365–383`, `line 254—383`. The en/em dashes are what the model
# actually emits; a plain hyphen appears too. Ranges must be ascending to count.
RANGE_RE = re.compile(r"lines?\s+(\d{1,6})\s*[-–—]\s*(\d{1,6})")
CLOSING_RE = re.compile(r"^\}")

# Source labels are repository-prefixed in the report header.
REPO_ROOTS = {"astrid": REPO, "minime": MINIME}


def _sort_key(path: Path) -> tuple[int, str]:
    match = TIMESTAMP_RE.search(path.stem)
    return (int(match.group(1)) if match else 0, path.name)


def recent_artifacts(being: str, window: int) -> list[Path]:
    """Newest-first artifacts for a being, with private lanes excluded fail-closed."""
    if being not in CORPORA:
        raise ValueError(f"unknown being: {being}")
    directory, pattern = CORPORA[being]
    if not directory.is_dir():
        return []
    paths = sorted(directory.glob(pattern), key=_sort_key, reverse=True)
    try:
        import being_privacy
    except ImportError:
        if being != "astrid":
            raise RuntimeError(f"being_privacy unavailable; refusing to scan {being}")
        kept = paths
    else:
        kept = being_privacy.filter_journal_paths(being, paths)
    return kept[:window]


def resolve_source(label: str) -> Path | None:
    """Map a `<repo>/<path>` report label onto a checkout path, or None."""
    parts = (label or "").strip().split("/", 1)
    if len(parts) != 2:
        return None
    root = REPO_ROOTS.get(parts[0])
    if root is None:
        return None
    candidate = root / parts[1]
    # Keep the label inside its declared repository; a label is not a path grant.
    try:
        candidate.resolve().relative_to(root.resolve())
    except ValueError:
        return None
    return candidate


def closing_line(lines: list[str], start_line: int) -> int | None:
    """First column-zero `}` at or after `start_line` (1-based), or None.

    Conservative by construction: Rust top-level items close at column zero, and a
    format-string brace never starts a line. An item whose close is not found returns
    None and the citation is reported `unverifiable` rather than truncated.
    """
    for index in range(max(start_line - 1, 0), len(lines)):
        if CLOSING_RE.match(lines[index]):
            return index + 1
    return None


def classify_citation(
    cited: tuple[int, int],
    page: tuple[int, int],
    lines: list[str] | None,
) -> dict[str, object]:
    """Classify one `lines A-B` citation against the page it was written from."""
    cited_start, cited_end = cited
    page_start, page_end = page
    verdict = {
        "cited_start": cited_start,
        "cited_end": cited_end,
        "page_start_line": page_start,
        "page_end_line": page_end,
        "item_end_line": None,
        "verdict": "interior",
    }
    if cited_end != page_end:
        return verdict
    if cited_start == page_start:
        verdict["verdict"] = "page_extent"
        return verdict
    if lines is None:
        verdict["verdict"] = "unverifiable"
        return verdict
    if not 1 <= cited_start <= len(lines):
        verdict["verdict"] = "unverifiable"
        return verdict
    end = closing_line(lines, cited_start)
    verdict["item_end_line"] = end
    if end is None:
        verdict["verdict"] = "unverifiable"
    elif end > page_end:
        verdict["verdict"] = "boundary_truncated"
    return verdict


def parse_report(path: Path) -> dict[str, object] | None:
    """Header facts and cited line ranges for one artifact, or None if unusable."""
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    source = SOURCE_RE.search(text)
    revision = REVISION_RE.search(text)
    if not source or not revision:
        return None
    citations = [
        (int(a), int(b)) for a, b in RANGE_RE.findall(text) if int(a) < int(b)
    ]
    if not citations:
        return None
    return {
        "artifact": path.name,
        "source_label": source.group(1).strip(),
        "source_sha256": revision.group(1),
        "byte_start": int(revision.group(2)),
        "byte_end": int(revision.group(3)),
        "citations": citations,
    }


def page_lines(report: dict[str, object]) -> tuple[tuple[int, int] | None, list[str] | None, str]:
    """Resolve a report's page to (start,end) lines plus file lines, with a reason."""
    label = str(report["source_label"])
    if not label.endswith(".rs"):
        return None, None, "not_rust_source"
    path = resolve_source(label)
    if path is None or not path.is_file():
        return None, None, "source_absent"
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != report["source_sha256"]:
        return None, None, "source_moved_on"
    start = raw[: int(report["byte_start"])].count(b"\n") + 1
    end = raw[: int(report["byte_end"])].count(b"\n") + 1
    return (start, end), raw.decode("utf-8", errors="replace").splitlines(), "verified"


def scan(being: str, window: int) -> dict[str, object]:
    findings: list[dict[str, object]] = []
    scanned = 0
    with_citations = 0
    reasons: dict[str, int] = {}
    for path in recent_artifacts(being, window):
        scanned += 1
        report = parse_report(path)
        if report is None:
            continue
        with_citations += 1
        page, lines, reason = page_lines(report)
        reasons[reason] = reasons.get(reason, 0) + 1
        if page is None:
            continue
        for cited in report["citations"]:
            verdict = classify_citation(cited, page, lines)
            if verdict["verdict"] != "boundary_truncated":
                continue
            findings.append(
                {
                    "artifact": report["artifact"],
                    "source_label": report["source_label"],
                    "cited_range": f"{verdict['cited_start']}-{verdict['cited_end']}",
                    "page_lines": f"{page[0]}-{page[1]}",
                    "item_end_line": verdict["item_end_line"],
                    "unseen_lines": int(verdict["item_end_line"]) - page[1],
                }
            )
    return {
        "schema": "source_page_symbol_truncation_watch_v1",
        "being": being,
        "window": window,
        "scanned": scanned,
        "reports_with_citations": with_citations,
        "verification_reasons": reasons,
        "truncated_citation_count": len(findings),
        "findings": findings,
        "boundary": (
            "read-only; counts a structural condition and never grades, annotates, or "
            "rewrites a being's text; steward-only, never into a being prompt"
        ),
    }


def _render(result: dict[str, object]) -> str:
    lines = [
        f"source page symbol truncation — {result['being']}: "
        f"{result['truncated_citation_count']} truncated citation(s) across "
        f"{result['reports_with_citations']} cited report(s) of {result['scanned']} scanned"
    ]
    if not result["findings"]:
        lines.append("  (none)")
    for found in result["findings"]:
        lines.append(
            f"  [cut] {found['source_label']} cited {found['cited_range']} on page "
            f"{found['page_lines']}; item closes at {found['item_end_line']} "
            f"({found['unseen_lines']} line(s) unseen) — {found['artifact']}"
        )
    return "\n".join(lines)


class SymbolTruncationWatchTests(unittest.TestCase):
    def test_page_extent_citation_is_not_a_truncation(self):
        verdict = classify_citation((254, 383), (254, 383), ["x"] * 400)
        self.assertEqual(verdict["verdict"], "page_extent")

    def test_interior_citation_is_not_a_truncation(self):
        verdict = classify_citation((335, 363), (254, 383), ["x"] * 400)
        self.assertEqual(verdict["verdict"], "interior")

    def test_item_closing_on_the_page_is_not_a_truncation(self):
        lines = ["fn a() {"] + ["    body"] * 8 + ["}"] + ["tail"] * 5
        # cited 1-10, page ends at 10, and the block closes at 10 exactly.
        verdict = classify_citation((1, 10), (0, 10), lines)
        self.assertEqual(verdict["verdict"], "interior")

    def test_item_continuing_past_the_page_is_a_truncation(self):
        lines = ["fn a() {"] + ["    body"] * 10 + ["}"]
        verdict = classify_citation((1, 8), (0, 8), lines)
        self.assertEqual(verdict["verdict"], "boundary_truncated")
        self.assertEqual(verdict["item_end_line"], 12)

    def test_unclosed_block_is_unverifiable_not_truncated(self):
        lines = ["fn a() {"] + ["    body"] * 4
        verdict = classify_citation((1, 3), (0, 3), lines)
        self.assertEqual(verdict["verdict"], "unverifiable")

    def test_missing_source_lines_are_unverifiable(self):
        verdict = classify_citation((365, 383), (254, 383), None)
        self.assertEqual(verdict["verdict"], "unverifiable")

    def test_resolve_source_refuses_escape_and_unknown_repo(self):
        self.assertIsNone(resolve_source("astrid/../../etc/passwd"))
        self.assertIsNone(resolve_source("elsewhere/src/main.rs"))
        self.assertIsNone(resolve_source("nopath"))

    def test_range_regex_reads_the_dashes_the_model_emits(self):
        self.assertEqual(
            RANGE_RE.findall("This function (lines 365–383) synthesizes"),
            [("365", "383")],
        )
        self.assertEqual(RANGE_RE.findall("lines 1-2 and line 3—4"),
                         [("1", "2"), ("3", "4")])

    def test_descending_range_is_ignored_by_parse(self):
        # `lines 400-12` is prose noise, not a citation.
        self.assertEqual(
            [(int(a), int(b)) for a, b in RANGE_RE.findall("lines 400-12")
             if int(a) < int(b)],
            [],
        )

    def test_reported_turn_is_a_truncation_in_this_checkout(self):
        """The 2026-09-11 report: `status_in` cited 365-383, closes at 385."""
        source = REPO / "capsules/spectral-bridge/src/autonomous/activity_reading.rs"
        if not source.is_file():
            self.skipTest("activity_reading.rs absent from this checkout")
        raw = source.read_bytes()
        if hashlib.sha256(raw).hexdigest() != (
            "3c7fca523107c8283159aa44b03c935e6ff2d5dfb87320b93a0c7c5f7ae4baf7"
        ):
            self.skipTest("activity_reading.rs has moved on from the bound revision")
        lines = raw.decode("utf-8").splitlines()
        page = (raw[:8571].count(b"\n") + 1, raw[:12834].count(b"\n") + 1)
        self.assertEqual(page, (254, 383))
        verdict = classify_citation((365, 383), page, lines)
        self.assertEqual(verdict["verdict"], "boundary_truncated")
        self.assertEqual(verdict["item_end_line"], 385)
        # The sibling function she described fully is untouched by the watch.
        self.assertEqual(
            classify_citation((335, 363), page, lines)["verdict"], "interior"
        )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    scan_cmd = sub.add_parser("scan", help="scan recent artifacts for truncated citations")
    scan_cmd.add_argument("--being", default="astrid", choices=sorted(CORPORA))
    scan_cmd.add_argument("--window", type=int, default=DEFAULT_WINDOW)
    scan_cmd.add_argument("--json", action="store_true")
    sub.add_parser("self-test", help="run unit tests")
    args = parser.parse_args(argv)

    if args.command == "self-test":
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(
            SymbolTruncationWatchTests
        )
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    result = scan(args.being, args.window)
    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print(_render(result))
    return 0


if __name__ == "__main__":
    sys.exit(main())
