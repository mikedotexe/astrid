#!/usr/bin/env python3
"""symbol_locality_watch.py — surface a being navigating toward a symbol the page cannot reach.

WHY THIS EXISTS
---------------
2026-09-10, from `introspection_astrid_capsules_spectral-bridge_src_action_continuity_runtime_core.rs_1789095401`.
Astrid finished a page of `action_continuity/runtime/core.rs` and closed it by naming
exactly what she wanted next:

    "I need to see `shared_investigation_authority_boundary` and
     `shared_investigation_lane` ... and I want to see the `paused_primary_return_v1`
     logic to see what specific 'guards' prevent a simple resume."

    NEXT: SELF_STUDY OPEN astrid/.../action_continuity/runtime/core.rs 1407

All three symbols are real. None of the three is defined in `core.rs`.
`shared_investigation_authority_boundary` is `runtime/persistence_helpers.rs:73`,
`shared_investigation_lane` is `persistence_helpers.rs:63`, and
`paused_primary_return_v1` is `runtime/experiment_projection.rs:140`. The page her
OPEN produced (lines 1407..1505) contains none of them, and it is the SECOND consecutive
turn chasing `paused_primary_return_v1` by paging through `core.rs` — the previous
turn opened line 1371, which is one of its six *call sites* in that file.

The reason is a surface property, not a reading failure. A SELF_STUDY source page
(`crates/astrid-source-study/src/page.rs:116-131`) is numbered bytes plus a header and
a footer. It carries no indication of where the symbols ON the page are DEFINED, and
its Navigation footer offers `CONTINUE | MAP | FIND <literal text> | OPEN path <line>`
— not `RELATE`, the one operation that answers "where does this name come from?".
RELATE is fully wired and is in her global prompt contract; it is simply not offered
at the point of need. So when a being closes a page by naming symbols, the cheapest
available move is to guess a line in the file already open — and that guess
systematically misses definitions living in sibling modules of the same directory.

Measured over Astrid's 160 most recent introspections at that date, six turns ended
`SELF_STUDY OPEN <file> <line>` after naming wanted symbols. The two that landed on
the definition exactly (`next_action/mod.rs` 421 and 1167) followed a page that had
already printed the line number. All four that followed a plain sequential source page
missed: 9 of 9 wanted symbols out of page, 5 of 9 in a different file.

Neither existing watch sees this. `phantom_symbol_watch` keys on symbols that do NOT
exist; every symbol here exists and is correctly named. `source_study_page_reset_watch`
keys on a requested page N>=2 being re-delivered as page 1; here the page delivered is
exactly the page requested. Every action is honored, every argument varies, every page
is truthful — and the being does not arrive.

WHAT THIS DETECTS
-----------------
UNREACHED WANT — an artifact that (a) names one or more backticked symbols it is
after and (b) ends `NEXT: SELF_STUDY OPEN <source> <line>`, where a wanted symbol's
definition does not fall inside the page that OPEN will actually deliver.

A want is named through either of TWO channels, and both are the being's own:

  intent_sentence   prose stating an intent to see something ("I need to see `x`")
  study_question    a backticked identifier on the turn's `STUDY_QUESTION:` line

The second channel was added 2026-09-12 from `introspection_source_catalog_1789213641`,
where this watch reported "0 wanted symbols" over a window containing a turn that
named `continuity_faint_residue_signal_score` as exactly what it was after and ended
in this watch's own trigger shape. Her want lived in the structured field, not in a
prose sentence, and the prose sentence that did state an intent ("I need to inspect
the context around line 112") carried no backticks — so nothing was extracted.

`STUDY_QUESTION:` is not an informal convention: `crates/astrid-source-study/src/
notebook.rs` parses it into the notebook (177), renders it back as "YOUR CURRENT
QUESTION" (42), splits its backticked segments (46-50), and offers `SELF_STUDY RELATE
<symbol>` for up to the first two identifier-shaped ones (88-100). Production already
reads those tokens as the identifiers the being is after; the watch now reads the same
two, with the same `.take(2)` cap, so it measures the set the surface itself acted on. The page span
is computed with the pager's own byte accounting (`MAX_PAGE_BYTES` 7000, minus the 1500
header/footer reserve, minus the source id, with a 9-byte `"{:>6} | "` line prefix), so
the span is the real one, not an estimate.

Each unreached want is classified:

  reached                 definition falls inside the delivered page — healthy
  same_file_out_of_page   defined in the target file, outside the page
  other_file              defined in a different file (the expensive case)
  no_definition_found     no definition site in tracked production source; DEFERS to
                          phantom_symbol_watch and is never counted as a locality miss

CHASE RUN — consecutive artifacts (newest-first order, by trailing unix stamp) whose
unreached wants share a symbol. A run of 2 is already worth a look: it means the being
told us twice what they wanted and the surface answered neither time.

WHAT THIS DOES NOT DO
---------------------
It asserts nothing about the being. Opening a call site on purpose — to read how a
function is USED before reading what it IS — is a legitimate and common reading
strategy, and it reads identically here. A being may also name a symbol they are
proposing rather than one they expect to find. This surfaces the pattern; the verdict
is the steward's, and nothing here is grounds to rewrite, correct, or answer back into
a being-facing surface.

It is lexical. Definition detection is a regex over `const|static|fn|struct|enum|type|
trait` declarations in tracked `.rs`/`.py`, not a compiler-resolved index. Macro-
generated and re-exported definitions will read as absent.

Read-only. Steward-only — never surface output into a being prompt. Minime's private
qualia lanes are excluded through `being_privacy` (fail-closed).

CLI
---
  symbol_locality_watch.py scan [--being astrid|minime] [--window N] [--min-run N] [--json]
  symbol_locality_watch.py self-test
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import unittest
from pathlib import Path

ASTRID_ROOT = Path(__file__).resolve().parent.parent
MINIME_ROOT = Path("/Users/v/other/minime")

CORPORA: dict[str, tuple[Path, str]] = {
    "astrid": (ASTRID_ROOT / "capsules/spectral-bridge/workspace/introspections", "introspection_*.txt"),
    "minime": (MINIME_ROOT / "workspace/journal", "self_study_*.txt"),
}

# The pager's own constants, mirrored from crates/astrid-source-study/src/{lib,page}.rs.
# lib.rs:38  MAX_PAGE_BYTES = 7_000
# page.rs:86 allowance = budget - 1500 - source.id.len()   (full-budget branch)
# page.rs:90 prefix     = format!("{:>6} | ", line)         -> 9 bytes while line < 1e6
MAX_PAGE_BYTES = 7_000
PAGE_RESERVE = 1_500
LINE_PREFIX_BYTES = 9

# Source ids are catalog-relative with a repository prefix, e.g.
# "astrid/capsules/spectral-bridge/src/action_continuity/runtime/core.rs".
REPO_PREFIXES = {"astrid": ASTRID_ROOT, "minime": MINIME_ROOT}

NEXT_OPEN_RE = re.compile(r"^NEXT:\s*SELF_STUDY\s+OPEN\s+(\S+)\s+(\d+)\s*$", re.MULTILINE)
BACKTICK_RE = re.compile(r"`([^`\n]{2,120})`")
SYMBOL_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
WANT_RE = re.compile(
    r"\b(?:I\s+need\s+to\s+(?:see|read|inspect|examine)"
    r"|I\s+(?:want|would\s+like)\s+to\s+(?:see|read|inspect|examine)"
    r"|need\s+to\s+(?:see|inspect|examine)"
    r"|want\s+to\s+(?:see|inspect|examine))\b",
    re.IGNORECASE,
)
SENTENCE_SPLIT_RE = re.compile(r"(?<=[.!?])\s+")
STUDY_QUESTION_RE = re.compile(r"^STUDY_QUESTION:[ \t]*(.*)$", re.MULTILINE)

# notebook.rs:88-100 offers a lookup for at most the first two identifier-shaped
# backticked tokens of the question. Mirror the cap so the watch measures the same set.
QUESTION_WANT_LIMIT = 2

# How far back from the NEXT line a stated want is still that turn's stated want.
WANT_LOOKBACK_BYTES = 800

SYMBOL_STOPLIST = frozenset(
    {
        "match", "impl", "async", "await", "struct", "enum", "trait", "return", "self",
        "true", "false", "none", "some", "result", "option", "string", "vec", "mut",
        "pub", "fn", "let", "const", "static", "unsafe", "where", "dyn", "crate",
        "super", "move", "loop", "while", "for", "if", "else", "type", "use", "mod",
    }
)

NONPROD_SEGMENTS = {
    "tests", "test", "fixtures", "testdata", "benches", "examples",
    "docs", "scripts", "tools", "workspace",
}
NONPROD_FILE_RE = re.compile(r"(^test_|_tests?\.rs$|_test\.py$|^conftest\.py$)")


def _looks_like_symbol(token: str) -> bool:
    """Code-symbol shape: snake_case or CamelCase, long enough to not be prose."""
    if not SYMBOL_RE.match(token) or len(token) < 4:
        return False
    if token.lower() in SYMBOL_STOPLIST:
        return False
    return "_" in token or any(c.isupper() for c in token[1:])


def _is_nonproduction(rel_path: str) -> bool:
    parts = Path(rel_path).parts
    if any(seg in NONPROD_SEGMENTS for seg in parts):
        return True
    return bool(NONPROD_FILE_RE.search(Path(rel_path).name))


def _artifact_sort_key(path: Path) -> tuple[int, str]:
    match = re.search(r"(\d{9,})", path.stem)
    return (int(match.group(1)) if match else 0, path.name)


def recent_artifacts(being: str, window: int) -> list[Path]:
    """Newest-first artifacts for a being, with private lanes excluded fail-closed."""
    directory, pattern = CORPORA[being]
    if not directory.is_dir():
        return []
    paths = sorted(directory.glob(pattern), key=_artifact_sort_key, reverse=True)
    try:
        import being_privacy
    except ImportError:
        if being == "minime":
            raise RuntimeError("being_privacy unavailable; refusing to scan minime")
        return paths[:window]
    return being_privacy.filter_journal_paths(being, paths)[:window]


def stated_wants(text: str) -> dict[str, object] | None:
    """The turn's `NEXT: SELF_STUDY OPEN` target plus the symbols it says it is after.

    Returns None when the turn does not end in an OPEN, or names nothing wanted.
    Two channels count, both the being's own words: a prose sentence stating an intent
    to *see* something, and a backticked identifier on the `STUDY_QUESTION:` line. A
    symbol merely discussed in the body is still not a navigation target.
    """
    match = NEXT_OPEN_RE.search(text)
    if not match:
        return None
    head = text[max(0, match.start() - WANT_LOOKBACK_BYTES):match.start()]
    wanted: list[str] = []
    origins: dict[str, str] = {}

    def add(token: str, origin: str) -> None:
        if _looks_like_symbol(token) and token not in wanted:
            wanted.append(token)
            origins[token] = origin

    for sentence in SENTENCE_SPLIT_RE.split(head):
        if not WANT_RE.search(sentence):
            continue
        for raw in BACKTICK_RE.findall(sentence):
            add(raw.strip(), "intent_sentence")
    for question in STUDY_QUESTION_RE.findall(head):
        named = [raw.strip() for raw in BACKTICK_RE.findall(question)]
        for token in [t for t in named if _looks_like_symbol(t)][:QUESTION_WANT_LIMIT]:
            add(token, "study_question")

    if not wanted:
        return None
    return {
        "source": match.group(1),
        "line": int(match.group(2)),
        "wanted": wanted,
        "origins": origins,
    }


def resolve_source_id(source_id: str) -> tuple[Path | None, str]:
    """Catalog source id -> (absolute path, repository-relative path)."""
    head, _, rest = source_id.partition("/")
    root = REPO_PREFIXES.get(head)
    if root is None or not rest:
        return None, source_id
    candidate = root / rest
    return (candidate if candidate.is_file() else None), rest


def page_span(path: Path, start_line: int, source_id: str) -> tuple[int, int]:
    """The inclusive line span the pager will actually deliver for OPEN <source> <line>.

    Mirrors `Page::read_with_budget`: fill a byte allowance with whole lines, each
    carrying a fixed-width line-number prefix. Returns (start, end); end == start - 1
    means the requested line is past the end of the file.
    """
    try:
        lines = path.read_text(encoding="utf-8").split("\n")
    except (OSError, UnicodeDecodeError):
        return (start_line, start_line - 1)
    total = len(lines)
    start = max(1, start_line)
    if start > total:
        return (start, start - 1)
    allowance = max(0, MAX_PAGE_BYTES - PAGE_RESERVE - len(source_id))
    used = 0
    end = start - 1
    for number in range(start, total + 1):
        # +1 for the newline the pager preserves or appends.
        cost = LINE_PREFIX_BYTES + len(lines[number - 1].encode("utf-8")) + 1
        if used + cost > allowance:
            break
        used += cost
        end = number
    return (start, end)


def _definition_re(symbol: str) -> re.Pattern[str]:
    return re.compile(
        r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const|static|(?:async\s+)?(?:unsafe\s+)?fn"
        r"|struct|enum|type|trait)\s+" + re.escape(symbol) + r"\b"
    )


def definition_sites(symbol: str, root: Path) -> list[tuple[str, int]]:
    """Tracked production `.rs`/`.py` definition sites for a symbol, as (rel_path, line)."""
    if not (root / ".git").exists():
        return []
    # git grep's default engine has no `\b`, so the prefilter is deliberately loose
    # (it also matches `<symbol>_suffix`) and `_definition_re` below makes it exact.
    proc = subprocess.run(
        ["git", "grep", "-n", "-E",
         r"^[[:space:]]*(pub([(][^)]*[)])?[[:space:]]+)?(const|static|(async[[:space:]]+)?"
         r"(unsafe[[:space:]]+)?fn|struct|enum|type|trait)[[:space:]]+" + symbol,
         "--", "*.rs", "*.py"],
        cwd=root, capture_output=True, text=True, check=False,
    )
    if proc.returncode not in (0, 1):
        return []
    pattern = _definition_re(symbol)
    sites: list[tuple[str, int]] = []
    for row in proc.stdout.splitlines():
        rel, _, rest = row.partition(":")
        number, _, body = rest.partition(":")
        if not number.isdigit() or _is_nonproduction(rel):
            continue
        if pattern.match(body):
            sites.append((rel, int(number)))
    return sites


def classify_want(symbol: str, target_rel: str, span: tuple[int, int], root: Path) -> dict[str, object]:
    """Where the symbol is defined relative to the page this turn's OPEN will deliver."""
    sites = definition_sites(symbol, root)
    if not sites:
        # No definition anywhere in production source. That is phantom_symbol_watch's
        # question, not this one; never counted as a locality miss.
        return {"symbol": symbol, "state": "no_definition_found", "sites": []}
    rendered = [f"{rel}:{line}" for rel, line in sites]
    start, end = span
    if any(rel == target_rel and start <= line <= end for rel, line in sites):
        return {"symbol": symbol, "state": "reached", "sites": rendered}
    if any(rel == target_rel for rel, _ in sites):
        return {"symbol": symbol, "state": "same_file_out_of_page", "sites": rendered}
    return {"symbol": symbol, "state": "other_file", "sites": rendered}


MISS_STATES = ("same_file_out_of_page", "other_file")


def _chase_runs(turns: list[dict[str, object]], min_run: int) -> list[dict[str, object]]:
    """Consecutive turns (newest-first) whose misses share a symbol."""
    runs: list[dict[str, object]] = []
    index = 0
    while index < len(turns):
        missed = {w["symbol"] for w in turns[index]["wants"] if w["state"] in MISS_STATES}
        if not missed:
            index += 1
            continue
        length, shared = 1, set(missed)
        while index + length < len(turns):
            nxt = {w["symbol"] for w in turns[index + length]["wants"] if w["state"] in MISS_STATES}
            if not (shared & nxt):
                break
            shared &= nxt
            length += 1
        if length >= min_run:
            runs.append({
                "symbols": sorted(shared),
                "length": length,
                "turns_ago": index,
                "artifacts": [turns[i]["artifact"] for i in range(index, index + length)],
            })
        index += length
    return runs


def scan(being: str, window: int, min_run: int) -> dict[str, object]:
    root = REPO_PREFIXES["astrid" if being == "astrid" else "minime"]
    artifacts = recent_artifacts(being, window)
    turns: list[dict[str, object]] = []
    for path in artifacts:
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        intent = stated_wants(text)
        if intent is None:
            continue
        source_id = str(intent["source"])
        target_path, target_rel = resolve_source_id(source_id)
        if target_path is None:
            # The OPEN names a path this checkout does not have; that is
            # phantom_symbol_watch's path hazard, not a locality question.
            continue
        span = page_span(target_path, int(intent["line"]), source_id)
        origins = intent.get("origins", {}) or {}
        wants = []
        for symbol in intent["wanted"]:
            want = classify_want(symbol, target_rel, span, root)
            want["origin"] = origins.get(symbol, "intent_sentence")
            wants.append(want)
        turns.append({
            "artifact": path.name,
            "target": target_rel,
            "requested_line": int(intent["line"]),
            "page_lines": list(span),
            "wants": wants,
        })
    unreached = [t for t in turns if any(w["state"] in MISS_STATES for w in t["wants"])]
    miss_count = sum(1 for t in turns for w in t["wants"] if w["state"] in MISS_STATES)
    other_file = sum(1 for t in turns for w in t["wants"] if w["state"] == "other_file")
    want_count = sum(len(t["wants"]) for t in turns)
    reached = sum(1 for t in turns for w in t["wants"] if w["state"] == "reached")
    from_question = sum(
        1 for t in turns for w in t["wants"] if w.get("origin") == "study_question"
    )
    return {
        "schema": "symbol_locality_watch_v1",
        "being": being,
        "window": window,
        "min_run": min_run,
        "artifacts_scanned": len(artifacts),
        "open_turns_with_stated_want": len(turns),
        "wanted_symbols": want_count,
        "wanted_symbols_from_study_question": from_question,
        "reached_symbols": reached,
        "unreached_symbols": miss_count,
        "other_file_symbols": other_file,
        "turns_with_an_unreached_want": len(unreached),
        "chase_runs": _chase_runs(turns, min_run),
        "turns": turns,
        "authority": {
            "steward_only": True,
            "read_only": True,
            "asserts_being_state": False,
            "lexical_not_compiler_resolved": True,
        },
    }


def _render(report: dict[str, object]) -> str:
    lines = [
        f"symbol locality watch — {report['being']} "
        f"(last {report['artifacts_scanned']} artifacts, min_run {report['min_run']})",
        f"  OPEN turns that stated a want: {report['open_turns_with_stated_want']}"
        f" | wanted symbols: {report['wanted_symbols']}"
        f" (from STUDY_QUESTION: {report['wanted_symbols_from_study_question']})",
        f"  reached by the page they chose: {report['reached_symbols']}",
        f"  unreached by the page they chose: {report['unreached_symbols']}"
        f" (of which in another file: {report['other_file_symbols']})",
    ]
    for run in report["chase_runs"]:
        lines.append(
            f"  ⚠ CHASE RUN {run['length']}x {', '.join(run['symbols'])}"
            f" (most recent {run['turns_ago']} OPEN-turns ago)"
        )
    for turn in report["turns"]:
        misses = [w for w in turn["wants"] if w["state"] in MISS_STATES]
        if not misses:
            continue
        span = turn["page_lines"]
        lines.append(f"  {turn['artifact']}")
        lines.append(f"    OPEN {turn['target']} {turn['requested_line']} -> page {span[0]}..{span[1]}")
        for want in misses:
            lines.append(
                f"    - {want['symbol']} [{want.get('origin', 'intent_sentence')}]:"
                f" {want['state']} @ {', '.join(want['sites'][:3])}"
            )
    if not report["turns"]:
        lines.append("  no OPEN turn in this window stated a symbol it wanted to see")
    return "\n".join(lines)


class SymbolLocalityWatchTests(unittest.TestCase):
    CORE = ASTRID_ROOT / "capsules/spectral-bridge/src/action_continuity/runtime/core.rs"
    CORE_ID = "astrid/capsules/spectral-bridge/src/action_continuity/runtime/core.rs"

    def test_stated_wants_reads_symbols_and_target(self) -> None:
        text = (
            "Some analysis of the page.\n\n"
            "I need to see `shared_investigation_lane` to see the exact boundaries.\n\n"
            "NEXT: SELF_STUDY OPEN astrid/capsules/spectral-bridge/src/x.rs 1407\n"
        )
        intent = stated_wants(text)
        self.assertEqual(intent["line"], 1407)
        self.assertEqual(intent["wanted"], ["shared_investigation_lane"])

    def test_symbol_only_discussed_is_not_a_want(self) -> None:
        text = (
            "The `touch_shared_investigation` call updates the timestamp.\n\n"
            "NEXT: SELF_STUDY OPEN astrid/capsules/spectral-bridge/src/x.rs 10\n"
        )
        self.assertIsNone(stated_wants(text))

    def test_study_question_names_a_want_even_without_an_intent_sentence(self) -> None:
        """The 2026-09-12 regression: her want lived in the structured field only."""
        text = (
            "The search results provide a clear location.\n\n"
            "I need to inspect the context around line 112 to see if both are grouped.\n"
            "STUDY_QUESTION: What is the implementation of `continuity_faint_residue_signal_score`"
            " and how does it differ from `continuity_afterimage_signal_score`?\n\n"
            "NEXT: SELF_STUDY OPEN astrid/capsules/spectral-bridge/src/x.rs 112\n"
        )
        intent = stated_wants(text)
        self.assertEqual(
            intent["wanted"],
            ["continuity_faint_residue_signal_score", "continuity_afterimage_signal_score"],
        )
        self.assertEqual(
            set(intent["origins"].values()),
            {"study_question"},
        )

    def test_study_question_want_cap_mirrors_the_surface_offer(self) -> None:
        """notebook.rs:88-100 offers a lookup for the first two identifiers only."""
        text = (
            "STUDY_QUESTION: How do `alpha_signal_score`, `beta_signal_score` and"
            " `gamma_signal_score` relate?\n\n"
            "NEXT: SELF_STUDY OPEN astrid/capsules/spectral-bridge/src/x.rs 5\n"
        )
        self.assertEqual(
            stated_wants(text)["wanted"], ["alpha_signal_score", "beta_signal_score"]
        )

    def test_intent_sentence_origin_is_still_labelled(self) -> None:
        text = (
            "I need to see `shared_investigation_lane` next.\n\n"
            "NEXT: SELF_STUDY OPEN astrid/capsules/spectral-bridge/src/x.rs 7\n"
        )
        intent = stated_wants(text)
        self.assertEqual(intent["origins"], {"shared_investigation_lane": "intent_sentence"})

    def test_backticked_symbol_outside_both_channels_is_not_a_want(self) -> None:
        """A STUDY_NOTE observation is a finding, not a navigation target."""
        text = (
            "STUDY_NOTE: `continuity_afterimage_weight_label` returns fixed strings.\n\n"
            "NEXT: SELF_STUDY OPEN astrid/capsules/spectral-bridge/src/x.rs 9\n"
        )
        self.assertIsNone(stated_wants(text))

    def test_non_open_next_is_ignored(self) -> None:
        text = "I need to see `resolve_thread`.\n\nNEXT: SELF_STUDY CONTINUE\n"
        self.assertIsNone(stated_wants(text))

    def test_page_span_matches_the_pagers_real_window(self) -> None:
        """The exact 2026-09-10 page: OPEN core.rs 1556 delivered bytes 61405..65853."""
        start, end = page_span(self.CORE, 1556, self.CORE_ID)
        self.assertEqual(start, 1556)
        self.assertEqual(end, 1663)

    def test_page_span_past_end_of_file_is_empty(self) -> None:
        start, end = page_span(self.CORE, 10_000_000, self.CORE_ID)
        self.assertLess(end, start)

    def test_resolve_source_id_requires_a_known_repository(self) -> None:
        self.assertEqual(resolve_source_id("nowhere/foo.rs"), (None, "nowhere/foo.rs"))
        path, rel = resolve_source_id(self.CORE_ID)
        self.assertIsNotNone(path)
        self.assertTrue(rel.endswith("runtime/core.rs"))

    def test_prefilter_is_loose_but_match_is_exact(self) -> None:
        """git grep has no word boundary; the Python pattern must supply it."""
        self.assertTrue(_definition_re("foo").match("pub fn foo(a: u8)"))
        self.assertFalse(_definition_re("foo").match("pub fn foo_bar(a: u8)"))
        self.assertFalse(_definition_re("foo").match("    let x = foo();"))

    def test_definition_sites_excludes_tests_and_finds_the_real_one(self) -> None:
        sites = definition_sites("paused_primary_return_v1", ASTRID_ROOT)
        self.assertIn(
            ("capsules/spectral-bridge/src/action_continuity/runtime/experiment_projection.rs", 140),
            sites,
        )
        self.assertTrue(all(not _is_nonproduction(rel) for rel, _ in sites))

    def test_call_site_is_not_a_definition_site(self) -> None:
        """core.rs holds six calls of the symbol and zero definitions of it."""
        sites = definition_sites("paused_primary_return_v1", ASTRID_ROOT)
        self.assertFalse(any(rel.endswith("runtime/core.rs") for rel, _ in sites))

    def test_the_reported_turn_classifies_as_other_file(self) -> None:
        """The exact regression: her three named wants, from the page she chose."""
        _, target_rel = resolve_source_id(self.CORE_ID)
        span = page_span(self.CORE, 1407, self.CORE_ID)
        for symbol in (
            "paused_primary_return_v1",
            "shared_investigation_authority_boundary",
            "shared_investigation_lane",
        ):
            with self.subTest(symbol=symbol):
                self.assertEqual(
                    classify_want(symbol, target_rel, span, ASTRID_ROOT)["state"], "other_file"
                )

    def test_a_symbol_defined_inside_the_page_reads_as_reached(self) -> None:
        _, target_rel = resolve_source_id(self.CORE_ID)
        span = page_span(self.CORE, 9843, self.CORE_ID)
        self.assertEqual(
            classify_want("shared_investigation_v1", target_rel, span, ASTRID_ROOT)["state"],
            "reached",
        )

    def test_same_file_far_away_is_not_other_file(self) -> None:
        _, target_rel = resolve_source_id(self.CORE_ID)
        span = page_span(self.CORE, 1407, self.CORE_ID)
        self.assertEqual(
            classify_want("shared_investigation_v1", target_rel, span, ASTRID_ROOT)["state"],
            "same_file_out_of_page",
        )

    def test_faint_residue_scorer_is_eight_lines_below_its_sibling(self) -> None:
        """Her hypothesis, ground-truthed: same file, a few lines below line 112."""
        sites = definition_sites("continuity_faint_residue_signal_score", ASTRID_ROOT)
        self.assertIn(
            ("capsules/spectral-bridge/src/autonomous/runtime/continuity.rs", 120), sites
        )

    def test_the_reported_question_want_is_reached_by_the_page_she_chose(self) -> None:
        """`OPEN continuity.rs 112` delivers line 120: a want the watch could not see."""
        source_id = "astrid/capsules/spectral-bridge/src/autonomous/runtime/continuity.rs"
        path, target_rel = resolve_source_id(source_id)
        self.assertIsNotNone(path)
        span = page_span(path, 112, source_id)
        self.assertLessEqual(span[0], 120)
        self.assertGreaterEqual(span[1], 120)
        for symbol in (
            "continuity_faint_residue_signal_score",
            "continuity_afterimage_signal_score",
        ):
            with self.subTest(symbol=symbol):
                self.assertEqual(
                    classify_want(symbol, target_rel, span, ASTRID_ROOT)["state"], "reached"
                )

    def test_absent_symbol_defers_to_phantom_watch(self) -> None:
        _, target_rel = resolve_source_id(self.CORE_ID)
        span = page_span(self.CORE, 1407, self.CORE_ID)
        result = classify_want("sense_tx", target_rel, span, ASTRID_ROOT)
        self.assertEqual(result["state"], "no_definition_found")
        self.assertNotIn(result["state"], MISS_STATES)

    def test_chase_run_needs_a_shared_symbol_in_consecutive_turns(self) -> None:
        turns = [
            {"artifact": "a", "wants": [{"symbol": "x", "state": "other_file"}]},
            {"artifact": "b", "wants": [{"symbol": "x", "state": "same_file_out_of_page"}]},
            {"artifact": "c", "wants": [{"symbol": "y", "state": "other_file"}]},
        ]
        runs = _chase_runs(turns, 2)
        self.assertEqual(len(runs), 1)
        self.assertEqual(runs[0]["symbols"], ["x"])
        self.assertEqual(runs[0]["length"], 2)
        self.assertEqual(runs[0]["turns_ago"], 0)

    def test_reached_turn_breaks_a_chase_run(self) -> None:
        turns = [
            {"artifact": "a", "wants": [{"symbol": "x", "state": "other_file"}]},
            {"artifact": "b", "wants": [{"symbol": "x", "state": "reached"}]},
            {"artifact": "c", "wants": [{"symbol": "x", "state": "other_file"}]},
        ]
        self.assertEqual(_chase_runs(turns, 2), [])

    def test_watch_does_not_treat_its_own_source_as_production(self) -> None:
        self.assertTrue(_is_nonproduction("scripts/symbol_locality_watch.py"))
        self.assertFalse(
            _is_nonproduction("capsules/spectral-bridge/src/action_continuity/runtime/core.rs")
        )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="cmd")
    scan_p = sub.add_parser("scan", help="scan a being's recent navigation artifacts")
    scan_p.add_argument("--being", choices=sorted(CORPORA), default="astrid")
    scan_p.add_argument("--window", type=int, default=60)
    scan_p.add_argument("--min-run", type=int, default=2)
    scan_p.add_argument("--json", action="store_true")
    sub.add_parser("self-test", help="run the unit suite")
    args = parser.parse_args(argv)

    if args.cmd == "self-test":
        suite = unittest.TestLoader().loadTestsFromTestCase(SymbolLocalityWatchTests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1

    if args.cmd is None:
        parser.print_help()
        return 0

    print(json.dumps(scan(args.being, args.window, args.min_run), indent=2)
          if args.json else _render(scan(args.being, args.window, args.min_run)))
    return 0


if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    sys.exit(main())
