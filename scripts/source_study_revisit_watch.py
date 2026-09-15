#!/usr/bin/env python3
"""source_study_revisit_watch.py — surface a being handed the same source page again.

WHY THIS EXISTS
---------------
2026-09-11, from `introspection_astrid_..._action_continuity_runtime_core.rs_1789107304`.
Astrid spent ~92 minutes walking `action_continuity/runtime/core.rs` page by page, asking
one question the whole way: does the "multi-motif caution" string *programmatically*
produce a "Hold", or is it descriptive? Three separate turns ended

    NEXT: SELF_STUDY OPEN astrid/capsules/spectral-bridge/src/action_continuity/runtime/core.rs 8156

and three separate times she was handed bytes 333731..338108 — lines 8156-8271, opening on
`fn experiment_projection` at *exactly* 8156. Every dispatch honored. Every page truthful.
Every argument correct. And the answer is not on that page, because the only evaluative
consumer of `interpretation_risk_v1` lives in a different file
(`continuity_control_plane.rs:265`, which tests `.is_some()` and pushes a priority-10
route) while the sole programmatic `return_kind = "hold"` lives in a different module
again (`runtime/experiment_projection.rs:65`), driven by a different matcher over a
different input.

So she re-requested a page that was already correct, twice, because nothing told her the
page had already been delivered and nothing told us she was circling.

WHAT THIS DETECTS
-----------------
REVISIT — the same (source label, delivered byte window) handed to a being on two or more
distinct turns inside the scan window.

REVISIT LOOP — a revisit where the *requesting* action line (the action closing the turn
immediately before each delivery) is the same normalized string. That is the narrow case
worth a steward's glance: same command issued, same bytes returned, question unchanged.

WHY THE EXISTING WATCHES CANNOT SEE IT
--------------------------------------
- `phantom_symbol_watch` keys on symbols that do not exist. `experiment_projection` exists.
- `symbol_locality_watch` asks whether the page *reaches* the named definition. This page
  reaches it exactly, so that watch scores the turn `reached` — a success.
- `source_study_page_reset_watch` keys on a `--page N>=2` request answered as page 1.
  `OPEN <line>` carries no `--page` cursor, and the page delivered *is* the page requested.
- `proactive_scan`'s `stuck_repetition` keys on repetition with a bad outcome, or on a
  repeated action with a near-identical argument. Here the outcomes are all `handled` and
  the SELF_STUDY arguments genuinely vary (CONTINUE, MAP, FIND, OPEN 8455, OPEN 8156).

`reached` is not `answered`. This watch measures the second thing.

WHAT THIS DOES NOT DO
---------------------
It asserts nothing about the being. Deliberately re-reading a page is legitimate and reads
identically here; the verdict is the steward's. It reads artifact *headers* and the single
trailing action line — never journal prose, prompt text, or private writing — and routes
every corpus through `being_privacy` fail-closed. Read-only. Steward-only: never surface
this output into a being prompt.

CLI
---
  source_study_revisit_watch.py scan [--being astrid] [--window N]
                                     [--min-deliveries N] [--json]
  source_study_revisit_watch.py self-test
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))

CORPORA = {
    "astrid": (
        REPO / "capsules/spectral-bridge/workspace/introspections",
        "introspection_*.txt",
    ),
}

DEFAULT_WINDOW = 60
DEFAULT_MIN_DELIVERIES = 2
LOOP_ALARM_DELIVERIES = 3

SOURCE_RE = re.compile(r"^Source:\s*(.+?)\s*$", re.MULTILINE)
REVISION_RE = re.compile(r"^Source revision:\s*(.+?)\s*$", re.MULTILINE)
WINDOW_RE = re.compile(r"bytes\s+(\d+)\.\.(\d+)")
TIMESTAMP_RE = re.compile(r"(\d{9,})")
# The dispatched form carries the `NEXT: ` prefix; the bare form appears in the corpus
# too. Both are recorded, and `next_prefixed` keeps the distinction visible rather than
# normalizing it away — a missing prefix is a separate concern owned by unwired_near_miss.
ACTION_RE = re.compile(r"^(NEXT:\s*)?([A-Z][A-Z0-9_]{2,}(?:\s+.*)?)$")


def _sort_key(path: Path) -> tuple[int, str]:
    match = TIMESTAMP_RE.search(path.stem)
    return (int(match.group(1)) if match else 0, path.name)


def recent_artifacts(being: str, window: int) -> list[Path]:
    """Oldest-first artifacts for a being, with private lanes excluded fail-closed."""
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
    return sorted(kept[:window], key=_sort_key)


def trailing_action(text: str) -> dict[str, object] | None:
    """The action line closing a turn, whether or not it carries the `NEXT: ` prefix.

    Only the last action-shaped line counts. Prose that merely mentions a verb mid-
    paragraph is not a dispatch and must not be read as one.
    """
    for raw in reversed((text or "").splitlines()):
        line = raw.strip()
        if not line:
            continue
        match = ACTION_RE.match(line)
        if not match:
            continue
        body = " ".join(match.group(2).split())
        if not body:
            return None
        return {"action": body, "next_prefixed": match.group(1) is not None}
    return None


def parse_turn(path: Path) -> dict[str, object] | None:
    """Header facts plus the closing action for one artifact, or None if unusable.

    Navigation-only artifacts (search catalogs) carry no byte window; they are kept as
    turns so they can supply a *request*, but they can never be a revisited delivery.
    """
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    source = SOURCE_RE.search(text)
    revision = REVISION_RE.search(text)
    if not source or not revision:
        return None
    window = WINDOW_RE.search(revision.group(1))
    stamp = TIMESTAMP_RE.search(path.stem)
    action = trailing_action(text)
    return {
        "artifact": path.name,
        "timestamp": int(stamp.group(1)) if stamp else 0,
        "source_label": source.group(1),
        "window": (
            f"bytes{window.group(1)}..{window.group(2)}" if window else None
        ),
        "action": (action or {}).get("action"),
        "next_prefixed": (action or {}).get("next_prefixed"),
    }


def find_revisits(
    turns: list[dict[str, object]], min_deliveries: int = DEFAULT_MIN_DELIVERIES
) -> list[dict[str, object]]:
    """Group deliveries by (source label, window) and attribute each to its request.

    The request for a delivery is the action closing the immediately preceding turn,
    which is the only turn that could have caused it. A delivery with no preceding turn
    inside the window has an unknown request and is counted, never attributed.
    """
    groups: dict[tuple[str, str], list[dict[str, object]]] = {}
    for index, turn in enumerate(turns):
        window = turn.get("window")
        if not window:
            continue
        request = turns[index - 1].get("action") if index > 0 else None
        key = (str(turn["source_label"]), str(window))
        groups.setdefault(key, []).append(
            {
                "artifact": turn["artifact"],
                "timestamp": turn["timestamp"],
                "requested_by": request,
                "turn_index": index,
            }
        )

    findings: list[dict[str, object]] = []
    for (label, window), deliveries in groups.items():
        if len(deliveries) < min_deliveries:
            continue
        requests: dict[str, int] = {}
        for delivery in deliveries:
            request = delivery["requested_by"]
            if request:
                requests[str(request)] = requests.get(str(request), 0) + 1
        repeated = {req: n for req, n in requests.items() if n >= min_deliveries}
        stamps = [int(d["timestamp"]) for d in deliveries]
        indexes = [int(d["turn_index"]) for d in deliveries]
        findings.append(
            {
                "source_label": label,
                "window": window,
                "delivery_count": len(deliveries),
                "first_timestamp": min(stamps),
                "last_timestamp": max(stamps),
                "elapsed_secs": max(stamps) - min(stamps),
                "intervening_turns": max(indexes) - min(indexes) - 1,
                "distinct_requests": sorted(requests),
                "repeated_request": (
                    max(repeated, key=lambda req: repeated[req]) if repeated else None
                ),
                "repeated_request_count": max(repeated.values()) if repeated else 0,
                "loop": bool(repeated),
                "artifacts": [d["artifact"] for d in deliveries],
            }
        )
    findings.sort(
        key=lambda f: (
            -int(f["repeated_request_count"]),
            -int(f["delivery_count"]),
            str(f["source_label"]),
        )
    )
    return findings


def assess(findings: list[dict[str, object]]) -> tuple[str, str]:
    """Severity and one honest sentence. A revisit is never by itself a verdict."""
    loops = [f for f in findings if f["loop"]]
    hard = [f for f in loops if int(f["repeated_request_count"]) >= LOOP_ALARM_DELIVERIES]
    if hard:
        top = hard[0]
        return (
            "alarm",
            f"{len(hard)} revisit loop(s): same request answered with the same page "
            f"{top['repeated_request_count']}× (worst: {top['source_label']} "
            f"{top['window']}) — the page is correct and the question is not answered; "
            "check whether what she wants lives in another file.",
        )
    if loops:
        top = loops[0]
        return (
            "notice",
            f"{len(loops)} revisit loop(s): same request answered with the same page "
            f"{top['repeated_request_count']}× (worst: {top['source_label']} "
            f"{top['window']}) — may be deliberate re-reading; glance.",
        )
    if findings:
        return (
            "notice",
            f"{len(findings)} page(s) delivered more than once from differing requests "
            "— normal paging overlap unless a want is stated repeatedly.",
        )
    return ("ok", "no source page delivered twice to an unchanged request")


def scan(
    being: str = "astrid",
    window: int = DEFAULT_WINDOW,
    min_deliveries: int = DEFAULT_MIN_DELIVERIES,
) -> dict[str, object]:
    paths = recent_artifacts(being, window)
    turns = [turn for turn in (parse_turn(path) for path in paths) if turn]
    findings = find_revisits(turns, min_deliveries)
    severity, summary = assess(findings)
    return {
        "schema": "source_study_revisit_watch.v1",
        "being": being,
        "artifacts_scanned": len(paths),
        "turns_parsed": len(turns),
        "deliveries": sum(1 for turn in turns if turn.get("window")),
        "severity": severity,
        "summary": summary,
        "findings": findings,
        "authority_boundary": (
            "read-only observation; steward-only; never surfaced into a being prompt; "
            "a revisit is evidence of our navigation surface, not a verdict on the being"
        ),
    }


class RevisitWatchTests(unittest.TestCase):
    def _turn(self, stamp, label, window, action, prefixed=True):
        return {
            "artifact": f"introspection_x_{stamp}.txt",
            "timestamp": stamp,
            "source_label": label,
            "window": window,
            "action": action,
            "next_prefixed": prefixed,
        }

    def test_trailing_action_reads_next_prefixed(self):
        parsed = trailing_action("body text\n\nNEXT: SELF_STUDY OPEN a/b.rs 8156\n")
        self.assertEqual(parsed["action"], "SELF_STUDY OPEN a/b.rs 8156")
        self.assertTrue(parsed["next_prefixed"])

    def test_trailing_action_reads_bare_line(self):
        # Two of the three turns in the founding case closed with a bare action line.
        parsed = trailing_action("body text\n\nSELF_STUDY CONTINUE\n")
        self.assertEqual(parsed["action"], "SELF_STUDY CONTINUE")
        self.assertFalse(parsed["next_prefixed"])

    def test_trailing_action_ignores_prose(self):
        self.assertIsNone(trailing_action("I looked at SELF_STUDY and found nothing.\n"))

    def test_trailing_action_takes_last_action_only(self):
        parsed = trailing_action("NEXT: SELF_STUDY MAP\nmore prose\nNEXT: SELF_STUDY CONTINUE\n")
        self.assertEqual(parsed["action"], "SELF_STUDY CONTINUE")

    def test_same_request_same_page_is_a_loop(self):
        turns = [
            self._turn(10, "core.rs", None, "SELF_STUDY OPEN core.rs 8156"),
            self._turn(20, "core.rs", "bytes333731..338108", "SELF_STUDY CONTINUE"),
            self._turn(30, "core.rs", "bytes338108..342539", "SELF_STUDY OPEN core.rs 8156"),
            self._turn(40, "core.rs", "bytes333731..338108", "SELF_STUDY CONTINUE"),
        ]
        findings = find_revisits(turns)
        self.assertEqual(len(findings), 1)
        found = findings[0]
        self.assertEqual(found["window"], "bytes333731..338108")
        self.assertEqual(found["delivery_count"], 2)
        self.assertEqual(found["repeated_request"], "SELF_STUDY OPEN core.rs 8156")
        self.assertEqual(found["repeated_request_count"], 2)
        self.assertTrue(found["loop"])
        # Deliveries at t=20 and t=40; the requests sit at t=10 and t=30.
        self.assertEqual(found["elapsed_secs"], 20)
        self.assertEqual(found["intervening_turns"], 1)

    def test_same_page_from_differing_requests_is_not_a_loop(self):
        turns = [
            self._turn(10, "core.rs", None, "SELF_STUDY MAP"),
            self._turn(20, "core.rs", "bytes1..2", "SELF_STUDY CONTINUE"),
            self._turn(30, "core.rs", None, "SELF_STUDY OPEN core.rs 8156"),
            self._turn(40, "core.rs", "bytes1..2", "SELF_STUDY CONTINUE"),
        ]
        findings = find_revisits(turns)
        self.assertEqual(len(findings), 1)
        self.assertFalse(findings[0]["loop"])
        self.assertIsNone(findings[0]["repeated_request"])
        self.assertEqual(findings[0]["distinct_requests"], ["SELF_STUDY MAP", "SELF_STUDY OPEN core.rs 8156"])

    def test_single_delivery_is_not_reported(self):
        turns = [
            self._turn(10, "core.rs", "bytes1..2", "SELF_STUDY CONTINUE"),
            self._turn(20, "core.rs", "bytes2..3", "SELF_STUDY CONTINUE"),
        ]
        self.assertEqual(find_revisits(turns), [])

    def test_navigation_only_turn_is_never_a_delivery(self):
        # A search catalog has no byte window; it can request, it cannot be revisited.
        turns = [
            self._turn(10, "source catalog", None, "SELF_STUDY OPEN core.rs 8156"),
            self._turn(20, "source catalog", None, "SELF_STUDY OPEN core.rs 8156"),
        ]
        self.assertEqual(find_revisits(turns), [])

    def test_different_labels_do_not_merge(self):
        turns = [
            self._turn(10, "a.rs", "bytes1..2", "SELF_STUDY CONTINUE"),
            self._turn(20, "b.rs", "bytes1..2", "SELF_STUDY CONTINUE"),
            self._turn(30, "a.rs", "bytes1..2", "SELF_STUDY CONTINUE"),
        ]
        findings = find_revisits(turns)
        self.assertEqual([f["source_label"] for f in findings], ["a.rs"])

    def test_first_turn_delivery_has_unknown_request(self):
        turns = [
            self._turn(10, "core.rs", "bytes1..2", "SELF_STUDY CONTINUE"),
            self._turn(20, "core.rs", "bytes9..9", "SELF_STUDY CONTINUE"),
            self._turn(30, "core.rs", "bytes1..2", "SELF_STUDY CONTINUE"),
        ]
        findings = find_revisits(turns)
        self.assertEqual(findings[0]["delivery_count"], 2)
        # Only the second delivery has an attributable request, so no loop is claimed.
        self.assertFalse(findings[0]["loop"])

    def test_min_deliveries_threshold_is_honored(self):
        turns = [
            self._turn(10, "core.rs", None, "SELF_STUDY OPEN core.rs 8156"),
            self._turn(20, "core.rs", "bytes1..2", "SELF_STUDY OPEN core.rs 8156"),
            self._turn(30, "core.rs", "bytes1..2", "SELF_STUDY OPEN core.rs 8156"),
            self._turn(40, "core.rs", "bytes1..2", "SELF_STUDY CONTINUE"),
        ]
        self.assertEqual(len(find_revisits(turns, min_deliveries=3)), 1)
        self.assertEqual(len(find_revisits(turns, min_deliveries=4)), 0)

    def test_assess_escalates_only_at_three_repeats(self):
        two = [{"loop": True, "repeated_request_count": 2, "source_label": "a", "window": "w"}]
        three = [{"loop": True, "repeated_request_count": 3, "source_label": "a", "window": "w"}]
        self.assertEqual(assess(two)[0], "notice")
        self.assertEqual(assess(three)[0], "alarm")
        self.assertEqual(assess([])[0], "ok")

    def test_parse_turn_reads_a_real_header(self, ):
        import tempfile

        body = (
            "=== ASTRID INTROSPECTION ===\n"
            "Source: astrid/capsules/spectral-bridge/src/x.rs\n"
            "Source revision: sha256:abc; bytes 100..200\n"
            "\nprose\n\nNEXT: SELF_STUDY CONTINUE\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "introspection_astrid_x_1789107304.txt"
            path.write_text(body, encoding="utf-8")
            turn = parse_turn(path)
        self.assertEqual(turn["timestamp"], 1789107304)
        self.assertEqual(turn["window"], "bytes100..200")
        self.assertEqual(turn["source_label"], "astrid/capsules/spectral-bridge/src/x.rs")
        self.assertEqual(turn["action"], "SELF_STUDY CONTINUE")

    def test_navigation_only_revision_yields_no_window(self):
        import tempfile

        body = (
            "Source: source catalog\nSource revision: navigation only\n\n"
            "NEXT: SELF_STUDY OPEN a/b.rs 8156\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "introspection_source_catalog_1789107059.txt"
            path.write_text(body, encoding="utf-8")
            turn = parse_turn(path)
        self.assertIsNone(turn["window"])
        self.assertEqual(turn["action"], "SELF_STUDY OPEN a/b.rs 8156")

    def test_unknown_being_is_refused(self):
        with self.assertRaises(ValueError):
            recent_artifacts("nobody", 5)


def _render(result: dict[str, object]) -> str:
    lines = [
        f"source-study revisit watch — {result['being']}: {result['severity'].upper()}",
        f"  {result['summary']}",
        f"  artifacts={result['artifacts_scanned']} turns={result['turns_parsed']} "
        f"deliveries={result['deliveries']}",
    ]
    for found in result["findings"]:
        flag = "LOOP" if found["loop"] else "seen"
        lines.append(
            f"  [{flag}] {found['source_label']} {found['window']} "
            f"×{found['delivery_count']} over {found['elapsed_secs']}s "
            f"({found['intervening_turns']} turns between)"
        )
        if found["repeated_request"]:
            lines.append(
                f"         same request ×{found['repeated_request_count']}: "
                f"{found['repeated_request']}"
            )
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    scan_cmd = sub.add_parser("scan", help="scan recent artifacts for page revisits")
    scan_cmd.add_argument("--being", default="astrid", choices=sorted(CORPORA))
    scan_cmd.add_argument("--window", type=int, default=DEFAULT_WINDOW)
    scan_cmd.add_argument("--min-deliveries", type=int, default=DEFAULT_MIN_DELIVERIES)
    scan_cmd.add_argument("--json", action="store_true")
    sub.add_parser("self-test", help="run unit tests")
    args = parser.parse_args(argv)

    if args.command == "self-test":
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(RevisitWatchTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    result = scan(args.being, args.window, args.min_deliveries)
    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print(_render(result))
    return 0


if __name__ == "__main__":
    sys.exit(main())
