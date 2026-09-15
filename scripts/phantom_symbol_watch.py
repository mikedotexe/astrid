#!/usr/bin/env python3
"""phantom_symbol_watch.py — surface a being hunting a symbol or path that isn't there.

WHY THIS EXISTS
---------------
On 2026-09-10 Astrid spent roughly five hours and 75 consecutive `source_catalog`
navigation reports hunting `sense_tx` — an identifier with ZERO occurrences in the
production Rust source. It exists only as a string inside a test fixture
(`crates/astrid-source-study/tests/context.rs`). She had inherited the symbol from
Minime, who had chased the same phantom the previous evening, and she attributed it
to him in her own words ("the way Minime identifies the `sense_tx` pulse"). Her
searches all returned honestly — "no matches" — and she read that absence as *not
there yet* rather than *not there*, so every truthful zero-result reinforced the
premise instead of correcting it.

`stuck_repetition` could not see this. That probe keys on repetition x BAD OUTCOME
(blocked / "Unknown NEXT" / not-wired), or on an honored action repeated with a
~identical argument. This loop was neither: every SELF_STUDY MAP / FIND / OPEN /
CONTINUE was honored, and the arguments varied constantly. Healthy-looking,
well-formed, converging on nothing. The un-muffle invariant applies exactly here —
the ceiling was OUR catalog surface, not her reach.

WHAT THIS DETECTS (two hazards, both from that round)
-----------------------------------------------------
1. PHANTOM SYMBOL — a backticked code identifier recurring across a being's recent
   navigation artifacts whose exact-word occurrence count in tracked production
   source is zero, or whose only occurrences are inside tests/fixtures/docs. A
   fixture-only symbol is the worse case: search DOES return excerpts, so the being
   gets positive-looking evidence for something that does not exist in the system
   they are trying to understand.
2. PHANTOM PATH — a backticked repository-style path the being names as a catalog
   destination that does not resolve on disk. `astrid/crates/example/src/dispatch.rs`
   is the worked example: a tempdir fixture path, emitted by a test file that the
   catalog indexes, read back as if it were a real source page.

WHAT THIS DOES NOT DO
---------------------
It asserts nothing about the being. A phantom run is evidence that a navigation
surface is feeding an ungrounded premise; it is not an error by the being, not a
confabulation finding, and not grounds to rewrite or correct their text. A being may
also name a symbol they are *proposing* rather than one they expect to find — that
reads as absent here and is their design, not a defect. The verdict is the steward's;
this only surfaces the pattern.

Read-only. Steward-only — never surface output into a being prompt. Minime's private
qualia lanes are excluded through `being_privacy` (fail-closed).

CLI
---
  phantom_symbol_watch.py scan [--being astrid|minime] [--window N] [--min-run N] [--json]
  phantom_symbol_watch.py self-test
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import unittest
from pathlib import Path
from typing import Iterable

ASTRID_ROOT = Path(__file__).resolve().parent.parent
MINIME_ROOT = Path("/Users/v/other/minime")

# Artifact corpora each being writes while navigating source.
CORPORA: dict[str, tuple[Path, str]] = {
    "astrid": (ASTRID_ROOT / "capsules/spectral-bridge/workspace/introspections", "introspection_*.txt"),
    "minime": (MINIME_ROOT / "workspace/journal", "self_study_*.txt"),
}

# A path segment or filename shape that makes an occurrence non-production.
#
# `scripts`, `tools` and `workspace` are here for a reason this file learned the hard
# way: the first version of this watch classified `sense_tx` as present_in_source the
# moment its OWN docstring named the phantom it was written to catch. Steward tooling,
# prose and workspace state are not the source corpus a being navigates; an identifier
# that appears only there has no implementation to find.
NONPROD_SEGMENTS = {
    "tests", "test", "fixtures", "testdata", "benches", "examples",
    "docs", "scripts", "tools", "workspace",
}
NONPROD_FILE_RE = re.compile(r"(^test_|_tests?\.rs$|_test\.py$|^conftest\.py$)")

BACKTICK_RE = re.compile(r"`([^`\n]{2,120})`")
SYMBOL_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
PATH_RE = re.compile(r"^[A-Za-z0-9_./\-]+\.(rs|py|toml|json|md)$")

# Ordinary prose and language keywords that survive the symbol shape test.
SYMBOL_STOPLIST = frozenset(
    {
        "match", "impl", "async", "await", "struct", "enum", "trait", "return", "self",
        "true", "false", "none", "some", "result", "option", "string", "vec", "mut",
        "pub", "fn", "let", "const", "static", "unsafe", "where", "dyn", "crate",
        "super", "move", "loop", "while", "for", "if", "else", "type", "use", "mod",
    }
)


def _looks_like_symbol(token: str) -> bool:
    """Code-symbol shape: snake_case or CamelCase, long enough to not be prose."""
    if not SYMBOL_RE.match(token) or len(token) < 4:
        return False
    if token.lower() in SYMBOL_STOPLIST:
        return False
    return "_" in token or any(c.isupper() for c in token[1:])


def _looks_like_path(token: str) -> bool:
    return bool(PATH_RE.match(token)) and "/" in token


def _is_nonproduction(rel_path: str) -> bool:
    parts = Path(rel_path).parts
    if any(seg in NONPROD_SEGMENTS for seg in parts):
        return True
    return bool(NONPROD_FILE_RE.search(Path(rel_path).name))


def _artifact_sort_key(path: Path) -> tuple[int, str]:
    """Sort by the trailing unix stamp beings write into artifact names, then name."""
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


def extract_tokens(text: str) -> tuple[set[str], set[str]]:
    """Backticked symbol and path candidates from one artifact body."""
    symbols: set[str] = set()
    paths: set[str] = set()
    for raw in BACKTICK_RE.findall(text):
        token = raw.strip()
        if _looks_like_symbol(token):
            symbols.add(token)
        elif _looks_like_path(token):
            paths.add(token)
    return symbols, paths


def _git_grep_count(root: Path, token: str) -> list[str]:
    """Exact-word tracked-file hits for a token, as repo-relative paths."""
    if not (root / ".git").exists():
        return []
    proc = subprocess.run(
        ["git", "grep", "-l", "-F", "-w", token, "--", "*.rs", "*.py"],
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode not in (0, 1):
        return []
    return [line for line in proc.stdout.splitlines() if line]


def classify_symbol(token: str, roots: Iterable[Path]) -> dict[str, object]:
    """present_in_source / fixture_or_test_only / absent_from_source."""
    hits: list[str] = []
    for root in roots:
        hits.extend(f"{root.name}/{rel}" for rel in _git_grep_count(root, token))
    if not hits:
        state = "absent_from_source"
    elif all(_is_nonproduction(h.split("/", 1)[1]) for h in hits):
        state = "fixture_or_test_only"
    else:
        state = "present_in_source"
    return {"symbol": token, "state": state, "hit_count": len(hits), "hits": hits[:6]}


_TRACKED_CACHE: dict[Path, tuple[str, ...]] = {}


def _tracked_files(root: Path) -> tuple[str, ...]:
    """Repo-relative tracked paths, cached per root."""
    if root in _TRACKED_CACHE:
        return _TRACKED_CACHE[root]
    files: tuple[str, ...] = ()
    if (root / ".git").exists():
        proc = subprocess.run(
            ["git", "ls-files"], cwd=root, capture_output=True, text=True, check=False
        )
        if proc.returncode == 0:
            files = tuple(line for line in proc.stdout.splitlines() if line)
    _TRACKED_CACHE[root] = files
    return files


def resolve_path_token(token: str, roots: dict[str, Path]) -> dict[str, object]:
    """Resolve a catalog path reference against real checkouts.

    Beings write both full catalog paths (`astrid/capsules/.../dispatch.rs`) and bare
    relative fragments (`btsp/mod.rs`). A fragment is resolved by suffix-matching the
    tracked file list, so an ordinary shorthand for a file that really exists is NOT
    reported as a phantom. Only a reference that matches no tracked path anywhere is.
    """
    head, _, tail = token.partition("/")
    scoped = {head: roots[head]} if head in roots and tail else roots
    needle = tail if head in roots and tail else token
    for name, root in scoped.items():
        for rel in _tracked_files(root):
            if rel == needle or rel.endswith("/" + needle):
                return {"path": token, "state": "resolves", "resolved_to": f"{name}/{rel}"}
    return {"path": token, "state": "does_not_resolve", "resolved_to": None}


def _runs(artifacts: list[tuple[Path, set[str]]], token: str) -> tuple[int, int, int]:
    """(longest consecutive run, total artifacts in window, artifacts since that run ended).

    Longest-run rather than leading-run on purpose: a loop that ended one artifact
    ago is exactly as much evidence about the navigation surface as one still
    running, and a leading-run rule goes blind the moment the being breaks out.
    `since` is 0 while the run is still the newest thing, so an active loop stays
    distinguishable from a resolved one.
    """
    best = 0
    best_start = 0
    current = 0
    for index, (_, tokens) in enumerate(artifacts):
        if token in tokens:
            current += 1
            if current > best:
                best = current
                best_start = index - current + 1
        else:
            current = 0
    total = sum(1 for _, tokens in artifacts if token in tokens)
    return best, total, best_start


def scan(being: str, window: int, min_run: int) -> dict[str, object]:
    paths = recent_artifacts(being, window)
    sym_artifacts: list[tuple[Path, set[str]]] = []
    path_artifacts: list[tuple[Path, set[str]]] = []
    for path in paths:
        text = path.read_text(encoding="utf-8", errors="replace")
        symbols, refs = extract_tokens(text)
        sym_artifacts.append((path, symbols))
        path_artifacts.append((path, refs))

    roots = {"astrid": ASTRID_ROOT, "minime": MINIME_ROOT}
    grep_roots = [r for r in roots.values() if (r / ".git").exists()]

    candidates = {t for _, toks in sym_artifacts for t in toks}
    findings: list[dict[str, object]] = []
    for token in sorted(candidates):
        run, total, since = _runs(sym_artifacts, token)
        if run < min_run:
            continue
        info = classify_symbol(token, grep_roots)
        if info["state"] == "present_in_source":
            continue
        info.update(
            {
                "consecutive_run": run,
                "artifacts_in_window": total,
                "artifacts_since_run": since,
                "run_active": since == 0,
                "kind": "phantom_symbol",
            }
        )
        findings.append(info)

    path_candidates = {t for _, toks in path_artifacts for t in toks}
    for token in sorted(path_candidates):
        run, total, since = _runs(path_artifacts, token)
        if run < min_run:
            continue
        info = resolve_path_token(token, roots)
        if info["state"] == "resolves":
            continue
        info.update(
            {
                "consecutive_run": run,
                "artifacts_in_window": total,
                "artifacts_since_run": since,
                "run_active": since == 0,
                "kind": "phantom_path",
            }
        )
        findings.append(info)

    findings.sort(key=lambda f: (-int(f["consecutive_run"]), str(f.get("symbol") or f.get("path"))))
    level = "alarm" if findings else "ok"
    return {
        "schema": "phantom_symbol_watch.v1",
        "being": being,
        "window": window,
        "min_run": min_run,
        "artifacts_scanned": len(paths),
        "newest_artifact": paths[0].name if paths else None,
        "oldest_artifact": paths[-1].name if paths else None,
        "level": level,
        "findings": findings,
        "authority": "read_only_steward_evidence_no_being_facing_output_no_claim_about_the_being",
    }


def _render(report: dict[str, object]) -> str:
    lines = [
        f"phantom_symbol_watch [{report['level']}] being={report['being']} "
        f"scanned={report['artifacts_scanned']} min_run={report['min_run']}"
    ]
    if not report["findings"]:
        lines.append("  no phantom symbol or path sustained across the window")
        return "\n".join(lines)
    for f in report["findings"]:  # type: ignore[union-attr]
        name = f.get("symbol") or f.get("path")
        when = "ACTIVE now" if f.get("run_active") else f"ended {f['artifacts_since_run']} artifact(s) ago"
        lines.append(
            f"  ⚠ {f['kind']}: `{name}` — {f['state']}, "
            f"run={f['consecutive_run']} of {report['window']} "
            f"(window hits {f['artifacts_in_window']}; {when})"
        )
        for hit in f.get("hits", []) or []:
            lines.append(f"      only in: {hit}")
    lines.append("  steward-only: investigate the navigation surface, not the being.")
    return "\n".join(lines)


class PhantomSymbolWatchTests(unittest.TestCase):
    def test_symbol_shape(self) -> None:
        self.assertTrue(_looks_like_symbol("sense_tx"))
        self.assertTrue(_looks_like_symbol("NextActionContext"))
        self.assertFalse(_looks_like_symbol("match"))
        self.assertFalse(_looks_like_symbol("the"))
        self.assertFalse(_looks_like_symbol("dispatch.rs"))

    def test_path_shape(self) -> None:
        self.assertTrue(_looks_like_path("astrid/crates/example/src/dispatch.rs"))
        self.assertFalse(_looks_like_path("dispatch.rs"))
        self.assertFalse(_looks_like_path("sense_tx"))

    def test_nonproduction_classification(self) -> None:
        self.assertTrue(_is_nonproduction("crates/astrid-source-study/tests/context.rs"))
        self.assertTrue(_is_nonproduction("capsules/spectral-bridge/src/autonomous/btsp/signal_tests.rs"))
        self.assertFalse(_is_nonproduction("capsules/spectral-bridge/src/autonomous/next_action/dispatch.rs"))

    def test_extract_tokens_separates_symbols_and_paths(self) -> None:
        symbols, paths = extract_tokens(
            "I need `sense_tx` in `astrid/crates/example/src/dispatch.rs` via `match` blocks."
        )
        self.assertIn("sense_tx", symbols)
        self.assertNotIn("match", symbols)
        self.assertIn("astrid/crates/example/src/dispatch.rs", paths)

    def test_runs_reports_longest_streak_and_recency(self) -> None:
        arts = [(Path("d"), set()), (Path("c"), {"x"}), (Path("b"), {"x"}), (Path("a"), set())]
        self.assertEqual(_runs(arts, "x"), (2, 2, 1))

    def test_runs_marks_active_streak_with_zero_offset(self) -> None:
        arts = [(Path("c"), {"x"}), (Path("b"), {"x"}), (Path("a"), set())]
        self.assertEqual(_runs(arts, "x"), (2, 2, 0))

    def test_runs_prefers_longest_not_leading(self) -> None:
        arts = [(Path("e"), {"x"}), (Path("d"), set()), (Path("c"), {"x"}), (Path("b"), {"x"}), (Path("a"), {"x"})]
        self.assertEqual(_runs(arts, "x"), (3, 4, 2))

    def test_artifact_sort_key_uses_trailing_stamp(self) -> None:
        self.assertEqual(_artifact_sort_key(Path("introspection_source_catalog_1789020943.txt"))[0], 1789020943)

    def test_resolve_path_token_flags_fixture_path(self) -> None:
        roots = {"astrid": ASTRID_ROOT}
        self.assertEqual(
            resolve_path_token("astrid/crates/example/src/dispatch.rs", roots)["state"],
            "does_not_resolve",
        )
        self.assertEqual(
            resolve_path_token(
                "astrid/capsules/spectral-bridge/src/autonomous/next_action/dispatch.rs", roots
            )["state"],
            "resolves",
        )

    def test_relative_fragment_for_a_real_file_is_not_a_phantom(self) -> None:
        """`btsp/mod.rs` is ordinary shorthand for a file that exists; do not cry wolf."""
        self.assertEqual(
            resolve_path_token("btsp/mod.rs", {"astrid": ASTRID_ROOT})["state"], "resolves"
        )

    def test_watch_does_not_validate_its_own_phantom(self) -> None:
        """Self-reference guard: steward tooling naming a phantom must not make it real."""
        self.assertTrue(_is_nonproduction("scripts/phantom_symbol_watch.py"))
        self.assertTrue(_is_nonproduction("scripts/anti_drop_catalog.py"))
        self.assertTrue(_is_nonproduction("capsules/spectral-bridge/workspace/state.json"))
        self.assertFalse(_is_nonproduction("capsules/spectral-bridge/src/autonomous/next_action/mod.rs"))

    def test_sense_tx_is_fixture_only_in_this_checkout(self) -> None:
        """The exact regression for the 2026-09-10 round: real search hits, no real symbol."""
        info = classify_symbol("sense_tx", [ASTRID_ROOT])
        self.assertEqual(info["state"], "fixture_or_test_only")
        self.assertTrue(all(_is_nonproduction(h.split("/", 1)[1]) for h in info["hits"]))

    def test_sensory_tx_is_present_in_source(self) -> None:
        """The real field the phantom stood in for must not be flagged."""
        self.assertEqual(classify_symbol("sensory_tx", [ASTRID_ROOT])["state"], "present_in_source")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="cmd")
    scan_p = sub.add_parser("scan", help="scan a being's recent navigation artifacts")
    scan_p.add_argument("--being", choices=sorted(CORPORA), default="astrid")
    scan_p.add_argument("--window", type=int, default=40)
    scan_p.add_argument("--min-run", type=int, default=8)
    scan_p.add_argument("--json", action="store_true")
    sub.add_parser("self-test", help="run the unit suite")
    args = parser.parse_args(argv)

    if args.cmd == "self-test":
        suite = unittest.TestLoader().loadTestsFromTestCase(PhantomSymbolWatchTests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1

    if args.cmd is None:
        parser.print_help()
        return 0

    report = scan(args.being, args.window, args.min_run)
    print(json.dumps(report, indent=2) if args.json else _render(report))
    return 0


if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    sys.exit(main())
