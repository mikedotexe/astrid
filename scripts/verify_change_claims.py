#!/usr/bin/env python3
"""verify_change_claims.py — catch CHANGELOG/ledger claims that NAME a test which
does not exist in the code (the "claim exceeds evidence" class).

Twice this session a claim outran the code: a CHANGELOG entry asserted "a
regression test covers the exact 80-file edge" before that test existed, and an
ad-hoc verification grep was itself buggy (it searched only Rust `fn`, silently
false-flagging every Python `def`). This tool makes the named-test class LOUD
and handles BOTH languages.

What it does (read-only):
  - Pull the `[Unreleased]` section of CHANGELOG.md (+ optionally the ledger).
  - HARD check: every backticked identifier that matches a test-name pattern
    (`test_*`, `*_test`, `*_tests`, CamelCase `*Tests`) must be DEFINED somewhere
    as `fn NAME` / `def NAME` / `mod NAME` / `class NAME`. A named-but-missing
    test => a likely overclaim => exit 3.
  - SOFT surface: UNNAMED test claims ("a regression test", "characterization
    test", "lock test", "N/0 tests") are listed for a human glance — they can't
    be auto-verified, and the perception overclaim was exactly this shape.

Known limit (documented, not hidden): Rust tests with descriptive names and no
`test_` prefix aren't auto-extracted as HARD claims; the SOFT unnamed-claim list
is the backstop for those. THE RULE remains: name your tests in the CHANGELOG so
this tool can verify them.

Usage:
  verify_change_claims.py [--changelog PATH] [--ledger PATH] [--repo PATH]
  verify_change_claims.py --self-test
Exit: 0 = all named test-claims resolve; 3 = a named test-claim is MISSING.
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
import unittest
from pathlib import Path

ASTRID_ROOT = Path(__file__).resolve().parent.parent

# A backticked identifier that looks like a TEST name (high confidence).
TEST_NAME_RE = re.compile(r"`(test_[A-Za-z0-9_]+|[A-Za-z0-9_]+_tests?|[A-Za-z0-9]+Tests)`")
# Unnamed test-existence assertions we can only surface for manual review.
UNNAMED_CLAIM_RE = re.compile(
    r"\b(a regression test|regression test covers|characterization test|lock test|"
    r"\d+/0(?: lib)? tests?|\+\d+ [a-z]* ?tests?)\b",
    re.IGNORECASE,
)


def extract_unreleased(changelog_text: str) -> str:
    """The text between `## [Unreleased]` and the next `## [` heading (or EOF)."""
    lines = changelog_text.splitlines()
    out: list[str] = []
    inside = False
    for line in lines:
        if line.strip().startswith("## [Unreleased]"):
            inside = True
            continue
        if inside and line.startswith("## ["):
            break
        if inside:
            out.append(line)
    return "\n".join(out)


def extract_named_test_claims(section: str) -> list[str]:
    """Backticked test-pattern identifiers claimed in `section` (deduped, sorted)."""
    return sorted({m.group(1) for m in TEST_NAME_RE.finditer(section)})


def extract_unnamed_claims(section: str) -> list[str]:
    """Unnamed test-existence phrases — surfaced for manual review (can't auto-verify)."""
    return sorted({m.group(0).strip() for m in UNNAMED_CLAIM_RE.finditer(section)})


def symbol_defined_in_text(name: str, text: str) -> bool:
    """True if `text` DEFINES `name` as a fn/mod/def/class (word-bounded). Pure —
    handles BOTH Rust (fn/mod) and Python (def/class), the bug the ad-hoc grep had."""
    return re.search(rf"\b(?:fn|mod|def|class)\s+{re.escape(name)}\b", text) is not None


# Backticked tokens that LOOK like test names but are concepts, not definitions
# (so they legitimately resolve to no fn/def/mod/class). Documented, not hidden.
KNOWN_NON_TEST_TOKENS = frozenset({"test_gap"})  # anti_drop_catalog's "has a test OR a gap" concept


def sibling_repos(astrid_repo: Path) -> list[Path]:
    """The coupled-stack repos a CHANGELOG entry may name tests in (astrid +
    minime + neural-triple-reservoir). An astrid CHANGELOG spans the whole stack
    and references tests in all three; searching only astrid false-flags the rest
    (e.g. the feeder's `MinimeApertureJitterTests`, minime's co-regulation tests)."""
    parent = astrid_repo.parent
    candidates = [astrid_repo, parent / "minime", parent / "neural-triple-reservoir"]
    return [p for p in candidates if (p / ".git").exists()]


def find_missing(names: list[str], repos: list[Path]) -> list[str]:
    """Names NOT defined as a fn/mod/def/class in any tracked .rs/.py file across
    `repos`. Reuses the tested pure `symbol_defined_in_text` (Python `re`, so `\\b`
    works for BOTH languages) over `git ls-files` — NOT `git grep -E`, whose POSIX
    regex silently drops `\\b` and false-flagged everything."""
    remaining = {n for n in names if n not in KNOWN_NON_TEST_TOKENS}
    for repo in repos:
        if not remaining:
            break
        try:
            files = subprocess.run(
                ["git", "-C", str(repo), "ls-files", "*.rs", "*.py"],
                capture_output=True, text=True, timeout=20, check=False,
            ).stdout.splitlines()
        except (OSError, subprocess.SubprocessError):
            continue
        for rel in files:
            if not remaining:
                break
            try:
                text = (repo / rel).read_text(encoding="utf-8", errors="ignore")
            except OSError:
                continue
            remaining -= {n for n in remaining if symbol_defined_in_text(n, text)}
    return sorted(remaining)


def verify(changelog: Path, ledger: Path | None, repos: list[Path]) -> dict:
    sections: list[str] = []
    if changelog.exists():
        sections.append(extract_unreleased(changelog.read_text(encoding="utf-8")))
    if ledger is not None and ledger.exists():
        sections.append(ledger.read_text(encoding="utf-8"))
    blob = "\n".join(sections)
    named = extract_named_test_claims(blob)
    missing = find_missing(named, repos)
    return {
        "named_claims": named,
        "missing": missing,
        "unnamed_claims": extract_unnamed_claims(blob),
    }


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--changelog", default=str(ASTRID_ROOT / "CHANGELOG.md"))
    ap.add_argument("--ledger", default=str(
        ASTRID_ROOT / "docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md"))
    ap.add_argument("--repo", default=str(ASTRID_ROOT),
                    help="astrid repo anchor; minime + neural-triple-reservoir siblings auto-searched")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _run_self_test()

    r = verify(Path(args.changelog), Path(args.ledger), sibling_repos(Path(args.repo)))
    print(f"# verify-change-claims — {len(r['named_claims'])} named test-claims checked")
    if r["missing"]:
        print(f"\n✗ MISSING ({len(r['missing'])}) — named in a changelog/ledger claim but NOT defined in code:")
        for n in r["missing"]:
            print(f"    {n}")
        print("\n  A named test that does not exist is a claim-exceeds-evidence overclaim.")
    else:
        print("✓ every named test-claim resolves to a fn/mod/def/class in the tree.")
    if r["unnamed_claims"]:
        print(f"\n  manual-verify ({len(r['unnamed_claims'])} unnamed test claims — can't auto-check, glance once):")
        for c in r["unnamed_claims"]:
            print(f"    \"{c}\"")
    return 3 if r["missing"] else 0


# ── self-test ────────────────────────────────────────────────────────────────
class VerifyChangeClaimsTests(unittest.TestCase):
    def test_extract_unreleased_stops_at_next_heading(self):
        text = "## [Unreleased]\n- in `test_a`\n\n## [0.2.0]\n- old `test_b`\n"
        sec = extract_unreleased(text)
        self.assertIn("test_a", sec)
        self.assertNotIn("test_b", sec)  # released section excluded

    def test_extract_named_test_claims_patterns(self):
        sec = ("Added `test_foo` and `bar_tests` and a `BeingPrivacyTests` class; "
               "also touched `pressure_risk` (a field, not a test).")
        named = extract_named_test_claims(sec)
        self.assertIn("test_foo", named)
        self.assertIn("bar_tests", named)
        self.assertIn("BeingPrivacyTests", named)
        self.assertNotIn("pressure_risk", named)  # not a test-name pattern

    def test_symbol_defined_handles_rust_and_python(self):
        self.assertTrue(symbol_defined_in_text("test_foo", "    fn test_foo() {"))
        self.assertTrue(symbol_defined_in_text("test_foo", "def test_foo(self):"))
        self.assertTrue(symbol_defined_in_text("BeingPrivacyTests", "class BeingPrivacyTests(unittest.TestCase):"))
        self.assertTrue(symbol_defined_in_text("tests", "mod tests;"))
        self.assertFalse(symbol_defined_in_text("test_foo", "// mentions test_foo in a comment only"))
        self.assertFalse(symbol_defined_in_text("test_foo", "let x = test_foo_helper();"))  # word-bounded

    def test_unnamed_claims_surfaced(self):
        sec = "a regression test covers the edge; 866/0 lib tests; +3 governor tests."
        claims = extract_unnamed_claims(sec)
        self.assertTrue(any("regression test" in c for c in claims))
        self.assertTrue(any("0" in c and "test" in c.lower() for c in claims))

    def test_find_missing_denylist_and_no_repo(self):
        # test_gap is the catalog's "has a test OR a gap" concept, not a test.
        self.assertEqual(find_missing(["test_gap"], []), [])
        # With no repos to search, a real-looking name is reported missing.
        self.assertEqual(
            find_missing(["test_does_not_exist_xyz"], []), ["test_does_not_exist_xyz"]
        )


def _run_self_test() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(VerifyChangeClaimsTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
