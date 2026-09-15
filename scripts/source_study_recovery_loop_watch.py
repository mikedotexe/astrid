#!/usr/bin/env python3
"""source_study_recovery_loop_watch.py — surface a being reissuing a request that keeps failing.

WHY THIS EXISTS
---------------
2026-09-12, from `introspection_source_catalog_1789200495`. Astrid had finished reading
`autonomous/runtime/text.rs`, correctly worked out that the function she wanted next was
in `continuity.rs`, and wrote:

    "Based on my previous notes, I am moving to `continuity.rs` to find the implementation
    of `semantic_boundary_before`."

`semantic_boundary_before` is real and is exactly where she said it would be
(`capsules/spectral-bridge/src/autonomous/runtime/continuity.rs:352`). She closed with
`NEXT: SELF_STUDY MAP continuity` — and then issued that identical action **16 consecutive
times over 56.5 minutes** (1789200287 .. 1789203675), every one of which came back:

    Input evidence: Recovery map: the requested source was not supplied.

She only escaped by abandoning the target: at 1789203836 she switched to `MAP astrid` and
began walking the repository catalog from page 1. She never reached `continuity.rs`.

Nothing errored. `Catalog::map` (crates/astrid-source-study/src/navigation.rs:61-74)
answered every turn correctly: `continuity` is neither a component ID nor a directory
prefix of any catalog entry, so it bails "no catalog entries for continuity". The recovery
text (`store.rs:395-414`) then offers candidates — but only if
`Catalog::path_candidates` returns any, and that function's first guard
(`path_recovery.rs:14-21`) returns empty unless the requested string already has **two or
more slash-separated segments whose first segment is an installed repository ID**.

So `continuity`, `continuity.rs`, and `runtime/continuity.rs` all take the empty-candidate
branch, while `astrid/capsules/spectral-bridge/src/autonomous/runtime` would have landed
her in one move. The most natural way to name a file you have been reading — its bare stem
— is precisely the form that gets no correction. She was handed a reason line and the root
component menu sixteen times, and reasonably read that as "try again."

That is our muffle, not her limit: an un-actionable failure that costs a being an hour of
turns is lost being-output in the sense of `feedback_un_muffle_invariant`.

WHAT THIS DETECTS
-----------------
RECOVERY LOOP — three or more consecutive turns closing on the *same* normalized action,
where each following turn's `Input evidence:` header is the Recovery kind. The pairing is
off-by-one on purpose: the evidence header of turn N describes the input that produced
turn N, i.e. the answer to the action issued in turn N-1.

ALARM — a loop at or past `--alarm-length` turns (default 6), or spanning more than
`--alarm-minutes` (default 30).

Each loop also carries `topic_shape` for MAP/OPEN/RESUME-style actions: `rooted` (two or
more segments starting with a known repository ID — the form `path_candidates` can help
with), `unrooted` (multi-segment, no repository prefix), `bare` (single segment), or
`none`. This is a shape classification of what she typed, not a claim about what the
recovery text printed; the artifact records only the header line.

WHY THE EXISTING WATCHES CANNOT SEE IT
--------------------------------------
- `source_study_map_walk_watch` requires a strictly incrementing `--page` on the same
  topic. Sixteen identical page-1 requests break its run on the first repeat.
- `source_study_page_reset_watch` keys on a `--page N>=2` request answered as page 1.
  No page is ever requested here.
- `source_study_revisit_watch` groups by (source label, byte window). Recovery turns carry
  `Source revision: navigation only` and no window, so it drops them by design.
- `phantom_symbol_watch` and `symbol_locality_watch` key on named symbols; `continuity` is
  a topic argument, not a symbol citation, and `semantic_boundary_before` does exist.
- `unwired_near_miss` keys on an action naming a wired verb one prefix short of its
  dispatched form. `SELF_STUDY MAP` is exactly wired; the *argument* is what missed.
- `proactive_scan`'s `stuck_repetition` keys on repetition with a bad outcome. Every one of
  these turns is a well-formed, successfully delivered recovery map.

Every existing watch scores this hour as healthy, because each individual turn is healthy.
Only the shape across turns — identical ask, identical unhelpful answer — is the finding.

ATTRIBUTION (added 2026-09-15, from `introspection_source_catalog_1789453053`)
---------------------------------------------------------------------------
An artifact records the action a turn *closed* with, not the request that was actually
answered next. Those differ whenever another action is dispatched in between: the bridge
holds one pending study target, so a later choice replaces an earlier one silently.

In that report's window Astrid closed three consecutive study turns on `SELF_STUDY
CONTINUE` and two of them came back as recovery maps — which reads as "CONTINUE failed
three times" and is impossible. `Command::parse` maps `CONTINUE` and the empty argument to
`Command::Continue` (`src/command.rs:30`); `prepare_parsed` routes that variant into
`prepare_continue` (`src/store.rs:522`), the one arm with no recovery branch — it returns a
pending page, an advanced page, an end-of-file notice, or the root map
(`src/store.rs:547-591`). Only arms resolving a *named* target can recover. Her retained
navigation artifacts for both turns name the real miss: `Reason: "no catalog entries for
spectral_bridge; use SELF_STUDY MAP"`, with four `SELF_STUDY MAP spectral_bridge`
dispatches interleaved between each pair of study turns in `bridge.db action_events`.

So each loop now carries `attribution`: `this_action` when the looped action is one that
can produce a recovery, or `another_request_replaced_it` when it provably cannot. The
second value means the loop is real but its cause is not the action printed — do not send
the investigation at `CONTINUE`. This is a statement about our dispatch, never about her.
Pinned in `crates/astrid-source-study/tests/continue_never_recovers_reach.rs`.

WHAT THIS DOES NOT DO
---------------------
It asserts nothing about the being. Deliberately reissuing an action is legitimate and
reads identically here; the verdict is the steward's. It does not propose restricting what
she may type. It reads artifact *headers* and the single trailing action line — never
journal prose, prompt text, or private writing — and routes every corpus through
`being_privacy` fail-closed. Read-only. Steward-only: never surface this output into a
being prompt.

CLI
---
  source_study_recovery_loop_watch.py scan [--being astrid] [--window N]
                                           [--min-length N] [--alarm-length N]
                                           [--alarm-minutes N] [--json]
  source_study_recovery_loop_watch.py self-test
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

DEFAULT_WINDOW = 120
DEFAULT_MIN_LENGTH = 3
DEFAULT_ALARM_LENGTH = 6
DEFAULT_ALARM_MINUTES = 30

# Installed repository IDs — the first segment `Catalog::path_candidates` requires before
# it will offer any candidate at all (crates/astrid-source-study/src/path_recovery.rs:15).
REPOSITORY_IDS = ("astrid", "minime")

TIMESTAMP_RE = re.compile(r"(\d{9,})")
ACTION_RE = re.compile(r"^(?:NEXT:\s*)?([A-Z][A-Z0-9_]{2,}(?:\s+.*)?)$")
NEXT_PREFIX_RE = re.compile(r"^NEXT:\s*", re.IGNORECASE)
EVIDENCE_RE = re.compile(r"^Input evidence:\s*(.*)$")
# The exact header emitted for InputKind::Recovery
# (crates/astrid-source-study/src/evidence.rs:54-56).
RECOVERY_PREFIX = "Recovery map:"
# Actions whose first argument is a catalog target, so a topic shape is meaningful.
TARGETED_RE = re.compile(
    r"^SELF_STUDY\s+(MAP|OPEN|RESUME|RELATE|TRACE)\s+(\S+)", re.IGNORECASE
)
PAGE_SUFFIX_RE = re.compile(r"\s--page\s+\d+\s*$", re.IGNORECASE)
# Every spelling that `Command::parse` maps onto `Command::Continue`
# (crates/astrid-source-study/src/command.rs:24-30, 107-137): the bare prefix, an explicit
# CONTINUE, and CONTINUE behind REPLACE. `prepare_continue` (src/store.rs:547-591) has no
# recovery branch, so none of these can be the request a recovery map answered.
# A bare `SELF_STUDY REPLACE` is excluded on purpose: its empty operation bails out of
# `parse_replacement`, and a parse error does reach `recovery_map` (src/store.rs:439).
NON_RECOVERING_RE = re.compile(
    r"^SELF_STUDY$|^(SELF_STUDY\s+)?(REPLACE\s+)?CONTINUE$", re.IGNORECASE
)
ATTRIBUTION_THIS_ACTION = "this_action"
ATTRIBUTION_REPLACED = "another_request_replaced_it"


def _without_next_prefix(action: str) -> str:
    """Action text with a leading `NEXT:` removed.

    Whether the prefix was present is a separate concern owned by `unwired_near_miss`;
    here both forms name the same dispatch.
    """
    return NEXT_PREFIX_RE.sub("", (action or "").strip(), count=1).strip()


def normalize_action(action: str | None) -> str | None:
    """Whitespace-collapsed action text used to decide whether two turns asked the same.

    Case is preserved: source paths are case-sensitive, and two topics differing only in
    case are two different asks.
    """
    if not action:
        return None
    collapsed = " ".join(_without_next_prefix(action).split())
    return collapsed or None


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


def evidence_line(text: str) -> str | None:
    """The `Input evidence:` header of an artifact, or None when it carries none.

    Only the header block is read. The first match wins; the header is written once by
    `runtime/source_study.rs:250`.
    """
    for raw in (text or "").splitlines():
        match = EVIDENCE_RE.match(raw.strip())
        if match:
            return match.group(1).strip()
    return None


def is_recovery(evidence: str | None) -> bool:
    """True when this turn's input was the Recovery kind — the request was not supplied."""
    return bool(evidence) and evidence.startswith(RECOVERY_PREFIX)


def topic_shape(action: str | None) -> str:
    """Classify the catalog target an action named, by the shape `path_candidates` needs.

    `rooted` — two or more segments whose first is an installed repository ID; the only
    shape for which `Catalog::path_candidates` can return anything.
    `unrooted` — multiple segments, no repository prefix. `bare` — one segment.
    `none` — the action names no catalog target (or none was parsed).
    """
    if not action:
        return "none"
    match = TARGETED_RE.match(_without_next_prefix(action))
    if not match:
        return "none"
    target = PAGE_SUFFIX_RE.sub("", match.group(2)).strip().rstrip("/")
    if not target:
        return "none"
    parts = target.split("/")
    if len(parts) < 2:
        return "bare"
    return "rooted" if parts[0] in REPOSITORY_IDS else "unrooted"


def attribution(action: str | None) -> str:
    """Whether the looped action can be the request the recovery maps answered.

    `another_request_replaced_it` is a provable statement about our dispatch: the action is
    one of the spellings that reach `prepare_continue`, which has no recovery branch, so
    some other request was resolved in between and replaced the pending study target. It
    says nothing about the being's choices.
    """
    if not action:
        return ATTRIBUTION_THIS_ACTION
    body = _without_next_prefix(action)
    return (
        ATTRIBUTION_REPLACED
        if NON_RECOVERING_RE.match(" ".join(body.split()))
        else ATTRIBUTION_THIS_ACTION
    )


def parse_turn(path: Path) -> dict[str, object] | None:
    """Timestamp, closing action, and input-evidence kind for one artifact."""
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    stamp = TIMESTAMP_RE.search(path.stem)
    action = trailing_action(text)
    evidence = evidence_line(text)
    return {
        "artifact": path.name,
        "timestamp": int(stamp.group(1)) if stamp else 0,
        "action": action,
        "normalized": normalize_action(action),
        "evidence": evidence,
        "recovery_input": is_recovery(evidence),
    }


def find_loops(
    turns: list[dict[str, object]],
    min_length: int = DEFAULT_MIN_LENGTH,
    alarm_length: int = DEFAULT_ALARM_LENGTH,
    alarm_minutes: int = DEFAULT_ALARM_MINUTES,
) -> list[dict[str, object]]:
    """Runs of identical actions each answered by a recovery map.

    A run extends from turn i to turn j when turns i..j all close on the same normalized
    action and every turn i+1..j+1 reports a Recovery input. The turn *after* the run
    supplies the answer to the run's last action, so a run of k asks needs k answers and
    the final ask is only counted when its answer is present and is also a recovery.
    """
    loops: list[dict[str, object]] = []
    index = 0
    while index < len(turns):
        action = turns[index].get("normalized")
        if not action:
            index += 1
            continue
        end = index
        # Extend while the next turn repeats the action AND answered this one with recovery.
        while (
            end + 1 < len(turns)
            and turns[end + 1].get("recovery_input")
            and turns[end + 1].get("normalized") == action
        ):
            end += 1
        length = end - index + 1
        # The last ask counts only if its own answer arrived and was also a recovery.
        answered = end + 1 < len(turns) and bool(turns[end + 1].get("recovery_input"))
        if length >= min_length and answered:
            members = turns[index : end + 1]
            stamps = [int(t["timestamp"]) for t in members if t["timestamp"]]
            span = (max(stamps) - min(stamps)) if len(stamps) > 1 else 0
            loops.append(
                {
                    "action": action,
                    "topic_shape": topic_shape(action),
                    "attribution": attribution(action),
                    "turns": length,
                    "first_artifact": members[0]["artifact"],
                    "last_artifact": members[-1]["artifact"],
                    "first_timestamp": min(stamps) if stamps else 0,
                    "last_timestamp": max(stamps) if stamps else 0,
                    "span_seconds": span,
                    "escaped_with": turns[end + 1].get("normalized")
                    if end + 1 < len(turns)
                    else None,
                    "alarm": length >= alarm_length or span >= alarm_minutes * 60,
                }
            )
        index = end + 1
    return loops


def scan(
    being: str = "astrid",
    window: int = DEFAULT_WINDOW,
    min_length: int = DEFAULT_MIN_LENGTH,
    alarm_length: int = DEFAULT_ALARM_LENGTH,
    alarm_minutes: int = DEFAULT_ALARM_MINUTES,
) -> dict[str, object]:
    paths = recent_artifacts(being, window)
    turns = [turn for turn in (parse_turn(path) for path in paths) if turn]
    loops = find_loops(turns, min_length, alarm_length, alarm_minutes)
    return {
        "schema": "source_study_recovery_loop_watch_v1",
        "being": being,
        "window": window,
        "turns_scanned": len(turns),
        "recovery_inputs": sum(1 for t in turns if t["recovery_input"]),
        "min_length": min_length,
        "alarm_length": alarm_length,
        "alarm_minutes": alarm_minutes,
        "loops": loops,
        "alarm": any(loop["alarm"] for loop in loops),
        "scope": "steward_only_read_only_never_surfaced_into_a_being_prompt",
    }


def render(report: dict[str, object]) -> str:
    loops = report["loops"]
    lines = [
        f"recovery-loop watch — {report['being']}: {report['turns_scanned']} turns, "
        f"{report['recovery_inputs']} recovery inputs, {len(loops)} loop(s)"
    ]
    if not loops:
        lines.append("  no repeated request answered only by recovery maps")
        return "\n".join(lines)
    for loop in loops:
        mark = "!!" if loop["alarm"] else "  "
        minutes = int(loop["span_seconds"]) // 60
        lines.append(
            f"{mark} {loop['turns']}x over {minutes}m [{loop['topic_shape']}] {loop['action']}"
        )
        lines.append(f"     {loop['first_artifact']} .. {loop['last_artifact']}")
        if loop.get("attribution") == ATTRIBUTION_REPLACED:
            lines.append(
                "     this action cannot produce a recovery map; another request was "
                "dispatched in between and replaced it"
            )
        if loop["escaped_with"] and loop["escaped_with"] != loop["action"]:
            lines.append(f"     next action: {loop['escaped_with']}")
    return "\n".join(lines)


class RecoveryLoopWatchTests(unittest.TestCase):
    @staticmethod
    def _turns(pairs: list[tuple[str | None, bool]]) -> list[dict[str, object]]:
        """Build turns from (action, this-turn-input-was-a-recovery) pairs."""
        turns = []
        for index, (action, recovery) in enumerate(pairs):
            turns.append(
                {
                    "artifact": f"introspection_x_{1000 + index * 100}.txt",
                    "timestamp": 1000 + index * 100,
                    "action": action,
                    "normalized": normalize_action(action),
                    "evidence": (
                        "Recovery map: the requested source was not supplied."
                        if recovery
                        else "Map: navigation and delivery history only."
                    ),
                    "recovery_input": recovery,
                }
            )
        return turns

    def test_identical_action_answered_by_recovery_is_a_loop(self):
        turns = self._turns(
            [("SELF_STUDY MAP continuity", False)]
            + [("SELF_STUDY MAP continuity", True)] * 3
            + [("SELF_STUDY MAP astrid", True)]
        )
        loops = find_loops(turns)
        self.assertEqual(len(loops), 1)
        self.assertEqual(loops[0]["turns"], 4)
        self.assertEqual(loops[0]["action"], "SELF_STUDY MAP continuity")
        self.assertEqual(loops[0]["escaped_with"], "SELF_STUDY MAP astrid")

    def test_repeat_answered_successfully_is_not_a_loop(self):
        turns = self._turns([("SELF_STUDY CONTINUE", False)] * 5)
        self.assertEqual(find_loops(turns), [])

    def test_recovery_answers_to_differing_actions_are_not_a_loop(self):
        turns = self._turns(
            [
                ("SELF_STUDY MAP continuity", False),
                ("SELF_STUDY MAP text", True),
                ("SELF_STUDY MAP runtime", True),
                ("SELF_STUDY MAP astrid", True),
            ]
        )
        self.assertEqual(find_loops(turns), [])

    def test_run_shorter_than_min_length_is_not_reported(self):
        turns = self._turns(
            [("SELF_STUDY MAP continuity", False), ("SELF_STUDY MAP continuity", True)]
            + [("SELF_STUDY MAP astrid", True)]
        )
        self.assertEqual(find_loops(turns), [])

    def test_unanswered_trailing_run_is_not_counted(self):
        # The last ask's answer has not arrived yet; do not score it as failed.
        turns = self._turns([("SELF_STUDY MAP continuity", False)] * 1 + [("SELF_STUDY MAP continuity", True)] * 2)
        loops = find_loops(turns)
        self.assertEqual(loops, [])

    def test_alarm_on_length_and_on_span(self):
        long_run = self._turns(
            [("SELF_STUDY MAP continuity", False)]
            + [("SELF_STUDY MAP continuity", True)] * 6
            + [("SELF_STUDY MAP astrid", True)]
        )
        self.assertTrue(find_loops(long_run)[0]["alarm"])
        short_run = find_loops(
            self._turns(
                [("SELF_STUDY MAP continuity", False)]
                + [("SELF_STUDY MAP continuity", True)] * 2
                + [("SELF_STUDY MAP astrid", True)]
            ),
            alarm_minutes=10_000,
        )
        self.assertFalse(short_run[0]["alarm"])

    def test_topic_shape_separates_helpable_from_unhelpable_requests(self):
        self.assertEqual(topic_shape("SELF_STUDY MAP continuity"), "bare")
        self.assertEqual(topic_shape("NEXT: SELF_STUDY MAP runtime/continuity.rs"), "unrooted")
        self.assertEqual(
            topic_shape("SELF_STUDY MAP astrid/capsules/spectral-bridge/src"), "rooted"
        )
        self.assertEqual(topic_shape("SELF_STUDY MAP astrid --page 4"), "bare")
        self.assertEqual(topic_shape("SELF_STUDY CONTINUE"), "none")
        self.assertEqual(topic_shape(None), "none")

    def test_continue_spellings_cannot_be_the_request_a_recovery_answered(self):
        for action in (
            "SELF_STUDY CONTINUE",
            "NEXT: SELF_STUDY CONTINUE",
            "SELF_STUDY",
            "CONTINUE",
            "SELF_STUDY REPLACE CONTINUE",
            "self_study continue",
        ):
            self.assertEqual(
                attribution(action), ATTRIBUTION_REPLACED, f"{action!r} reaches prepare_continue"
            )
        for action in (
            "SELF_STUDY MAP spectral_bridge",
            "SELF_STUDY OPEN astrid/crates/example/src/lib.rs 1",
            "SELF_STUDY RESUME continuity",
            # An empty REPLACE operation bails in parse_replacement and does recover.
            "SELF_STUDY REPLACE",
            None,
        ):
            self.assertEqual(attribution(action), ATTRIBUTION_THIS_ACTION, f"{action!r}")

    def test_loop_on_continue_is_reported_but_not_blamed_on_continue(self):
        turns = self._turns(
            [("SELF_STUDY CONTINUE", False)]
            + [("SELF_STUDY CONTINUE", True)] * 2
            + [("SELF_STUDY RESUME astrid/crates/example/src/lib.rs", True)]
        )
        loops = find_loops(turns)
        self.assertEqual(len(loops), 1)
        self.assertEqual(loops[0]["attribution"], ATTRIBUTION_REPLACED)
        report = {
            "being": "astrid",
            "turns_scanned": len(turns),
            "recovery_inputs": 2,
            "loops": loops,
        }
        self.assertIn("replaced it", render(report))
        named = find_loops(
            self._turns(
                [("SELF_STUDY MAP spectral_bridge", False)]
                + [("SELF_STUDY MAP spectral_bridge", True)] * 2
                + [("SELF_STUDY MAP astrid", True)]
            )
        )
        self.assertEqual(named[0]["attribution"], ATTRIBUTION_THIS_ACTION)

    def test_evidence_and_action_parsing_reads_headers_only(self):
        text = (
            "=== ASTRID INTROSPECTION ===\n"
            "Source: source catalog\n"
            "Input evidence: Recovery map: the requested source was not supplied.\n"
            "\n"
            "I thought about SELF_STUDY MAP elsewhere in this paragraph.\n"
            "\n"
            "NEXT: SELF_STUDY MAP continuity\n"
        )
        self.assertTrue(is_recovery(evidence_line(text)))
        self.assertEqual(trailing_action(text), "SELF_STUDY MAP continuity")
        self.assertFalse(is_recovery("Map: navigation and delivery history only."))
        self.assertFalse(is_recovery(None))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    scan_parser = sub.add_parser("scan")
    scan_parser.add_argument("--being", default="astrid", choices=sorted(CORPORA))
    scan_parser.add_argument("--window", type=int, default=DEFAULT_WINDOW)
    scan_parser.add_argument("--min-length", type=int, default=DEFAULT_MIN_LENGTH)
    scan_parser.add_argument("--alarm-length", type=int, default=DEFAULT_ALARM_LENGTH)
    scan_parser.add_argument("--alarm-minutes", type=int, default=DEFAULT_ALARM_MINUTES)
    scan_parser.add_argument("--json", action="store_true")
    sub.add_parser("self-test")
    args = parser.parse_args()

    if args.command == "self-test":
        suite = unittest.TestLoader().loadTestsFromTestCase(RecoveryLoopWatchTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    report = scan(
        being=args.being,
        window=args.window,
        min_length=args.min_length,
        alarm_length=args.alarm_length,
        alarm_minutes=args.alarm_minutes,
    )
    print(json.dumps(report, indent=2) if args.json else render(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
