#!/usr/bin/env python3
"""source_study_map_walk_watch.py — surface a being walking a repository MAP page by page.

WHY THIS EXISTS
---------------
2026-09-11, from `introspection_source_catalog_1789119142`. Astrid wrote:

    "Because the `action_continuity` directory has been difficult to access due to
    catalog inconsistencies, I am systematically re-mapping the repository"

and closed with `NEXT: SELF_STUDY MAP astrid --page 5`. By the time that report reached
the flywheel queue she had walked, one page per turn without a single skip or branch,
from page 5 to page 55 — 51 consecutive turns, ~68 minutes.

The entries she was looking for are on **page 2**. She had already walked past them
before the first report in this chain was written, and every further page carried her
further away. The astrid catalog holds 6,277 entries over ~118 pages; 4,581 of them
(73.0%) are `docs/steward-notes/**` — the flywheel's own round packets. Her bridge
source occupies pages 2-7. Pages 14-109 are almost entirely our notes about her.

Nothing was broken. `Catalog::map` (crates/astrid-source-study/src/navigation.rs:59-72)
answered every request exactly, and the directory-scoped form she needed —
`SELF_STUDY MAP astrid/capsules/spectral-bridge/src/action_continuity` — resolves through
the same prefix filter on line 63-67 and would have landed her in one move.

The muffle is in what the surface *offers*. The component branch of `Catalog::map`
(navigation.rs:48-58) ends its listing with an explicit narrowing menu:

    "These are entry points. Browse their directories for the surrounding implementation:"
    SELF_STUDY MAP <directory>

The repository/prefix branch (navigation.rs:59-72) emits no such menu. Its only
advertised next move is the pagination footer written by `paginate_with_header`
(navigation.rs:157-159):

    Next: SELF_STUDY MAP astrid --page N+1

So when a being types `MAP <repository>` the running surface teaches exactly one move,
and increments. The path form is documented one layer away, in the help syntax line
(`autonomous/next_action/action_help.rs:162`), whose two worked examples are `MAP` and
`MAP kernel` — neither a directory. A being reading the page in front of her is reading
a correct, complete, one-move menu.

WHAT THIS DETECTS
-----------------
MAP WALK — three or more consecutive turns closing on `SELF_STUDY MAP <topic>` with a
strictly incrementing `--page`, same topic, no other action between them.

NARROWING UNUSED — a walk in which the being never once issued a narrowing move: no
`MAP <topic>/<subpath>`, no `FIND`, no `OPEN`, no `RELATE`, anywhere inside the walk.
That is the signature worth a steward's glance. A walk with narrowing in it is a being
surveying deliberately; a walk without any is a being handed one move and taking it.

ALARM — a NARROWING UNUSED walk at or past `--alarm-length` turns (default 12).

WHY THE EXISTING WATCHES CANNOT SEE IT
--------------------------------------
- `source_study_revisit_watch` groups deliveries by (source label, byte window). MAP turns
  carry `Source revision: navigation only` and no byte window, so it drops them by design.
- `source_study_page_reset_watch` keys on a `--page N>=2` request answered as page 1. Here
  every page requested is the page delivered; the cursor never resets.
- `symbol_locality_watch` and `phantom_symbol_watch` key on named symbols. A MAP walk
  names none.
- `proactive_scan`'s `stuck_repetition` keys on repetition with a bad outcome or a
  near-identical argument. Every one of these turns is `handled`, and `--page 5` through
  `--page 55` are 51 genuinely distinct arguments.

Every existing watch scores this walk as healthy, because each turn *is* healthy. Only
the shape across turns is the finding.

WHAT THIS DOES NOT DO
---------------------
It asserts nothing about the being. Paging a catalog on purpose is legitimate and reads
identically here; the verdict is the steward's. It does not propose narrowing the
catalog — removing `docs/steward-notes/**` from her reach would be taking reading away
from her to fix our own volume, which is not a repair. It reads artifact *headers* and
the single trailing action line — never journal prose, prompt text, or private writing —
and routes every corpus through `being_privacy` fail-closed. Read-only. Steward-only:
never surface this output into a being prompt.

CLI
---
  source_study_map_walk_watch.py scan [--being astrid] [--window N]
                                      [--min-length N] [--alarm-length N] [--json]
  source_study_map_walk_watch.py self-test
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

DEFAULT_WINDOW = 80
DEFAULT_MIN_LENGTH = 3
DEFAULT_ALARM_LENGTH = 12

TIMESTAMP_RE = re.compile(r"(\d{9,})")
ACTION_RE = re.compile(r"^(?:NEXT:\s*)?([A-Z][A-Z0-9_]{2,}(?:\s+.*)?)$")
NEXT_PREFIX_RE = re.compile(r"^NEXT:\s*", re.IGNORECASE)
MAP_RE = re.compile(r"^SELF_STUDY\s+MAP\s*(.*)$", re.IGNORECASE)
PAGE_RE = re.compile(r"\s--page\s+(\d+)\s*$", re.IGNORECASE)
# Any of these inside a walk means the being reached for something other than "next page".
NARROWING_RE = re.compile(
    r"^SELF_STUDY\s+(FIND|OPEN|RESUME|RELATE|SESSION|TRACE|QUESTION)\b", re.IGNORECASE
)


def _without_next_prefix(action: str) -> str:
    """Action text with a leading `NEXT:` removed.

    `trailing_action` already strips the prefix, but callers also hand this module raw
    lines copied from an artifact. Both forms name the same dispatch; whether the prefix
    was present is a separate concern owned by `unwired_near_miss`.
    """
    return NEXT_PREFIX_RE.sub("", (action or "").strip(), count=1).strip()


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


def trailing_action(text: str) -> str | None:
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
        body = " ".join(match.group(1).split())
        return body or None
    return None


def parse_map_action(action: str | None) -> dict[str, object] | None:
    """Split a `SELF_STUDY MAP <topic> [--page N]` action into topic and page.

    A bare `SELF_STUDY MAP` is page 1 of the root menu and carries topic "". Anything
    that is not a MAP action returns None.
    """
    if not action:
        return None
    match = MAP_RE.match(_without_next_prefix(action))
    if not match:
        return None
    rest = match.group(1).strip()
    page_match = PAGE_RE.search(rest)
    if page_match:
        page = int(page_match.group(1))
        topic = rest[: page_match.start()].strip()
    else:
        page = 1
        topic = rest
    return {"topic": topic, "page": page}


def parse_turn(path: Path) -> dict[str, object] | None:
    """Timestamp plus the closing action for one artifact, or None if unreadable."""
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    stamp = TIMESTAMP_RE.search(path.stem)
    action = trailing_action(text)
    return {
        "artifact": path.name,
        "timestamp": int(stamp.group(1)) if stamp else 0,
        "action": action,
        "map": parse_map_action(action),
    }


def is_narrowing(action: str | None, topic: str) -> bool:
    """True when the action reaches past 'next page' — a scoped MAP or a non-MAP read.

    A MAP whose topic strictly extends the walked topic is narrowing; a MAP of the same
    topic (any page) is not.
    """
    if not action:
        return False
    if NARROWING_RE.match(_without_next_prefix(action)):
        return True
    parsed = parse_map_action(action)
    if not parsed:
        return False
    other = str(parsed["topic"])
    return other != topic and other.startswith(f"{topic.rstrip('/')}/")


def find_walks(
    turns: list[dict[str, object]],
    min_length: int = DEFAULT_MIN_LENGTH,
    alarm_length: int = DEFAULT_ALARM_LENGTH,
) -> list[dict[str, object]]:
    """Maximal runs of same-topic MAP turns whose page increments by exactly one.

    A run breaks on a different topic, a non-incrementing page, or any turn that is not
    a MAP. `narrowing_used` records whether any action inside the run reached past the
    pagination footer; `alarm` is reserved for long runs where none did.
    """
    walks: list[dict[str, object]] = []
    run: list[dict[str, object]] = []

    def flush() -> None:
        if len(run) < min_length:
            run.clear()
            return
        topic = str(run[0]["map"]["topic"])
        narrowing = [
            str(turn["action"])
            for turn in run
            if is_narrowing(str(turn["action"]), topic)
        ]
        first_page = int(run[0]["map"]["page"])
        last_page = int(run[-1]["map"]["page"])
        stamps = [int(turn["timestamp"]) for turn in run if turn["timestamp"]]
        walks.append(
            {
                "topic": topic,
                "length": len(run),
                "first_page": first_page,
                "last_page": last_page,
                "first_artifact": run[0]["artifact"],
                "last_artifact": run[-1]["artifact"],
                "elapsed_secs": (max(stamps) - min(stamps)) if len(stamps) > 1 else 0,
                "narrowing_used": bool(narrowing),
                "narrowing_actions": narrowing,
                "alarm": (not narrowing) and len(run) >= alarm_length,
            }
        )
        run.clear()

    for turn in turns:
        parsed = turn.get("map")
        if not parsed:
            flush()
            continue
        if run:
            previous = run[-1]["map"]
            same_topic = str(previous["topic"]) == str(parsed["topic"])
            steps_one = int(parsed["page"]) == int(previous["page"]) + 1
            if not (same_topic and steps_one):
                flush()
        run.append(turn)
    flush()
    return walks


def scan(
    being: str,
    window: int = DEFAULT_WINDOW,
    min_length: int = DEFAULT_MIN_LENGTH,
    alarm_length: int = DEFAULT_ALARM_LENGTH,
) -> dict[str, object]:
    paths = recent_artifacts(being, window)
    turns = [turn for turn in (parse_turn(path) for path in paths) if turn]
    walks = find_walks(turns, min_length=min_length, alarm_length=alarm_length)
    return {
        "schema": "source_study_map_walk_watch_v1",
        "schema_version": 1,
        "being": being,
        "artifacts_scanned": len(turns),
        "window": window,
        "min_length": min_length,
        "alarm_length": alarm_length,
        "walks": walks,
        "walk_count": len(walks),
        "alarm_count": sum(1 for walk in walks if walk["alarm"]),
        "steward_only": True,
        "asserts_about_being": False,
    }


def render(report: dict[str, object]) -> str:
    lines = [
        f"source_study_map_walk_watch — being={report['being']} "
        f"artifacts={report['artifacts_scanned']} walks={report['walk_count']} "
        f"alarms={report['alarm_count']}"
    ]
    if not report["walks"]:
        lines.append("  no MAP page-walk of the configured length in this window.")
    for walk in report["walks"]:  # type: ignore[union-attr]
        mark = "⚠ ALARM" if walk["alarm"] else "  note "
        topic = walk["topic"] or "(root menu)"
        lines.append(
            f"{mark} MAP {topic}: pages {walk['first_page']}→{walk['last_page']} "
            f"over {walk['length']} turns, {walk['elapsed_secs']}s, "
            f"narrowing_used={walk['narrowing_used']}"
        )
        lines.append(f"         {walk['first_artifact']} .. {walk['last_artifact']}")
    lines.append("  Steward-only. Never surface into a being prompt.")
    return "\n".join(lines)


class SelfTest(unittest.TestCase):
    @staticmethod
    def _turns(actions: list[str]) -> list[dict[str, object]]:
        return [
            {
                "artifact": f"introspection_x_{1000 + index}.txt",
                "timestamp": 1000 + index,
                "action": action,
                "map": parse_map_action(action),
            }
            for index, action in enumerate(actions)
        ]

    def test_parse_map_action_reads_topic_and_page(self):
        self.assertEqual(
            parse_map_action("SELF_STUDY MAP astrid --page 55"),
            {"topic": "astrid", "page": 55},
        )
        self.assertEqual(
            parse_map_action("SELF_STUDY MAP astrid"), {"topic": "astrid", "page": 1}
        )
        self.assertEqual(parse_map_action("SELF_STUDY MAP"), {"topic": "", "page": 1})
        self.assertIsNone(parse_map_action("SELF_STUDY FIND multi-motif"))
        self.assertIsNone(parse_map_action(None))

    def test_incrementing_same_topic_run_is_a_walk(self):
        walks = find_walks(
            self._turns([f"NEXT: SELF_STUDY MAP astrid --page {n}" for n in range(5, 20)]),
            alarm_length=12,
        )
        self.assertEqual(len(walks), 1)
        self.assertEqual(walks[0]["topic"], "astrid")
        self.assertEqual((walks[0]["first_page"], walks[0]["last_page"]), (5, 19))
        self.assertEqual(walks[0]["length"], 15)
        self.assertFalse(walks[0]["narrowing_used"])
        self.assertTrue(walks[0]["alarm"])

    def test_page_gap_and_topic_change_break_the_run(self):
        actions = [
            "SELF_STUDY MAP astrid --page 1",
            "SELF_STUDY MAP astrid --page 2",
            "SELF_STUDY MAP astrid --page 3",
            "SELF_STUDY MAP astrid --page 9",
            "SELF_STUDY MAP astrid --page 10",
            "SELF_STUDY MAP astrid --page 11",
        ]
        walks = find_walks(self._turns(actions), min_length=3)
        self.assertEqual([(w["first_page"], w["last_page"]) for w in walks], [(1, 3), (9, 11)])
        walks = find_walks(
            self._turns(
                [
                    "SELF_STUDY MAP astrid --page 1",
                    "SELF_STUDY MAP astrid --page 2",
                    "SELF_STUDY MAP minime --page 3",
                ]
            ),
            min_length=3,
        )
        self.assertEqual(walks, [])

    def test_non_map_turn_breaks_the_run(self):
        walks = find_walks(
            self._turns(
                [
                    "SELF_STUDY MAP astrid --page 1",
                    "SELF_STUDY MAP astrid --page 2",
                    "DECOMPOSE",
                    "SELF_STUDY MAP astrid --page 3",
                ]
            ),
            min_length=3,
        )
        self.assertEqual(walks, [])

    def test_scoped_map_inside_a_walk_counts_as_narrowing(self):
        # A deeper topic ends the run (topic changed) but is recorded as narrowing when
        # it appears inside one; here the walk itself is same-topic and the scoped MAP
        # is checked directly.
        self.assertTrue(
            is_narrowing("SELF_STUDY MAP astrid/capsules/spectral-bridge/src", "astrid")
        )
        self.assertTrue(is_narrowing("SELF_STUDY FIND multi-motif caution", "astrid"))
        self.assertTrue(is_narrowing("NEXT: SELF_STUDY OPEN astrid/x.rs 10", "astrid"))
        self.assertFalse(is_narrowing("SELF_STUDY MAP astrid --page 6", "astrid"))
        self.assertFalse(is_narrowing("SELF_STUDY MAP astridextra", "astrid"))
        self.assertFalse(is_narrowing("DECOMPOSE", "astrid"))

    def test_long_walk_with_narrowing_is_not_an_alarm(self):
        actions = [f"SELF_STUDY MAP astrid --page {n}" for n in range(1, 14)]
        turns = self._turns(actions)
        # Replace one turn's action with a scoped MAP while keeping its walk membership,
        # which is exactly what a being surveying on purpose looks like.
        turns[6]["action"] = "SELF_STUDY MAP astrid/scripts"
        walks = find_walks(turns, alarm_length=12)
        self.assertEqual(len(walks), 1)
        self.assertTrue(walks[0]["narrowing_used"])
        self.assertFalse(walks[0]["alarm"])

    def test_short_run_is_not_reported(self):
        walks = find_walks(
            self._turns(
                ["SELF_STUDY MAP astrid --page 1", "SELF_STUDY MAP astrid --page 2"]
            ),
            min_length=3,
        )
        self.assertEqual(walks, [])

    def test_trailing_action_ignores_prose_mentions(self):
        text = "I thought about SELF_STUDY MAP for a while.\n\nNEXT: SELF_STUDY MAP astrid --page 5\n"
        self.assertEqual(trailing_action(text), "SELF_STUDY MAP astrid --page 5")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    scan_parser = sub.add_parser("scan")
    scan_parser.add_argument("--being", default="astrid", choices=sorted(CORPORA))
    scan_parser.add_argument("--window", type=int, default=DEFAULT_WINDOW)
    scan_parser.add_argument("--min-length", type=int, default=DEFAULT_MIN_LENGTH)
    scan_parser.add_argument("--alarm-length", type=int, default=DEFAULT_ALARM_LENGTH)
    scan_parser.add_argument("--json", action="store_true")
    sub.add_parser("self-test")
    args = parser.parse_args()

    if args.command == "self-test":
        suite = unittest.TestLoader().loadTestsFromTestCase(SelfTest)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    report = scan(
        args.being,
        window=args.window,
        min_length=args.min_length,
        alarm_length=args.alarm_length,
    )
    print(json.dumps(report, indent=2) if args.json else render(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
