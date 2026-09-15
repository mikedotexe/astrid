#!/usr/bin/env python3
"""negative_assertion_lead_watch.py — surface a being who followed a DISCONFIRMING line as a lead.

WHY THIS EXISTS
---------------
2026-09-11, from `introspection_source_catalog_1789151592`. Astrid ran
`SELF_STUDY FIND multi-motif`, was shown "0 implementation matches" and exactly one
test/fixture hit, and followed it:

    `astrid/capsules/spectral-bridge/src/autonomous/next_action/pressure_agency.rs 765`:
    `"multi-motif",`

    "This is a significant lead because it suggests that the 'multi-motif caution' status
     is likely a defined outcome or a state transition handled by the pressure_agency logic."

Line 765 is the fifth element of `for absent in [...]` inside
`status_render_is_a_telemetry_formatter_not_a_motif_aggregator`, whose body is
`assert!(!report.contains(absent), ...)`. Her single hit is the tree's strongest evidence
*against* the hypothesis it led her to. And the answer she wanted was 16 lines above it, in a
`///` block written to her in a previous round — which FIND does not carry.

The cause is a surface property, not a reading failure. A FIND row
(`crates/astrid-source-study/src/source_search.rs` 139-147) is a single-line horizontal slice,
`line[at-50 .. at+len+120]`, plus a role label and an OPEN hint. Nothing in it distinguishes
`assert!(x.contains(q))` from `for absent in [..]` + `assert!(!x.contains(q))`. Polarity is
exactly the bit that decides whether a hit confirms or refutes, and it is the bit the row drops.

WHAT THIS WATCH DOES NOT DO
---------------------------
It does not judge the being's reasoning — given a row with no polarity, following it is
correct inference. It flags the SURFACE, so a steward can answer the turn. It is read-only:
no source edit, no prompt change, no live control, no delivery into any being's context.

Sibling watches: `symbol_locality_watch` (wanted symbol not on the page), `phantom_symbol_watch`
(symbol that does not exist), `source_study_revisit_watch` (re-reading a delivered page).
None of them look at what a delivered line ASSERTS.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import unittest
from pathlib import Path

ASTRID = Path("/Users/v/other/astrid")
MINIME = Path("/Users/v/other/minime")

CORPORA = {
    "astrid": ASTRID / "capsules/spectral-bridge/workspace/introspections",
    "minime": MINIME / "workspace/journal",
}
REPO_ROOTS = {"astrid": ASTRID, "minime": MINIME}

# A FIND row cited back in prose: "<repo>/<path>.<ext> <line>" or "<path>:<line>".
CITATION = re.compile(
    r"(?P<path>(?:astrid|minime)/[\w./-]+\.(?:rs|py|toml|metal|sh))"
    r"(?:[:\s]+|\s+line\s+)(?P<line>\d{1,6})\b"
)

# Names that mark an array of things asserted ABSENT.
NEGATIVE_BINDERS = (
    "absent",
    "missing",
    "forbidden",
    "banned",
    "excluded",
    "must_not",
    "not_present",
    "unexpected",
    "disallowed",
)
NEGATIVE_PROSE = ("must not", "should not", "never carry", "is not present", "does not contain")

DOC_PREFIXES = ("///", "//!", "#:")


def _read(path: Path) -> list[str] | None:
    try:
        return path.read_text(encoding="utf-8", errors="replace").split("\n")
    except OSError:
        return None


def resolve(cited: str) -> Path | None:
    """Resolve a catalog-style id to a local path without leaving its repository."""
    repo, _, rel = cited.partition("/")
    root = REPO_ROOTS.get(repo)
    if root is None or not rel:
        return None
    target = (root / rel).resolve()
    try:
        target.relative_to(root.resolve())
    except ValueError:
        return None
    return target if target.is_file() else None


def in_test_remainder(lines: list[str], lineno: int) -> bool:
    """Mirror source_search.rs collect_lines: position-based, never restored."""
    return any(
        line.lstrip().startswith("#[cfg(test)]") for line in lines[: max(lineno - 1, 0)]
    )


def nearest_doc_block(lines: list[str], lineno: int, reach: int = 40) -> tuple[int, str] | None:
    """The /// block immediately above the enclosing item — what a FIND row drops."""
    i = lineno - 2
    limit = max(lineno - 2 - reach, -1)
    while i > limit and i >= 0:
        if lines[i].strip().startswith(DOC_PREFIXES):
            end = i
            while i - 1 >= 0 and lines[i - 1].strip().startswith(DOC_PREFIXES):
                i -= 1
            return i + 1, "\n".join(lines[i : end + 1])
        i -= 1
    # Deliberately no early bail on intervening code: the point is to name a line
    # number the being can OPEN, not to prove the block documents this exact item.
    return None


def classify(lines: list[str], lineno: int, reach: int = 25) -> dict:
    """Classify the assertion polarity of the context enclosing a cited line."""
    if lineno < 1 or lineno > len(lines):
        return {"verdict": "out_of_range", "evidence": None, "evidence_line": None}
    own = lines[lineno - 1]
    window_start = max(lineno - 1 - reach, 0)
    above = [(n + 1, lines[n]) for n in range(window_start, lineno)]

    for number, text in reversed(above):
        low = text.lower()
        binder = re.search(r"\bfor\s+(\w+)\s+in\s*\[", text)
        if binder and binder.group(1).lower() in NEGATIVE_BINDERS:
            return {"verdict": "negative_assertion", "evidence": text.strip(), "evidence_line": number}
        if "assert!(!" in text.replace(" ", "") or "assert_ne!" in text:
            return {"verdict": "negative_assertion", "evidence": text.strip(), "evidence_line": number}
        if any(phrase in low for phrase in NEGATIVE_PROSE) and (
            "assert" in low or number == lineno
        ):
            return {"verdict": "negative_assertion", "evidence": text.strip(), "evidence_line": number}
        if re.search(r"\bassert(_eq)?!\(", text) and "!(" not in text.replace("assert!(", ""):
            return {"verdict": "positive_assertion", "evidence": text.strip(), "evidence_line": number}

    if own.strip().startswith(DOC_PREFIXES):
        return {"verdict": "documentation", "evidence": own.strip(), "evidence_line": lineno}
    return {"verdict": "unclassified", "evidence": None, "evidence_line": None}


def inspect(cited_path: str, lineno: int) -> dict | None:
    target = resolve(cited_path)
    if target is None:
        return None
    lines = _read(target)
    if lines is None:
        return None
    verdict = classify(lines, lineno)
    doc = nearest_doc_block(lines, lineno)
    return {
        "path": cited_path,
        "line": lineno,
        "text": lines[lineno - 1].strip()[:160] if 0 < lineno <= len(lines) else None,
        "verdict": verdict["verdict"],
        "context_evidence": verdict["evidence"],
        "context_line": verdict["evidence_line"],
        "in_test_remainder": in_test_remainder(lines, lineno),
        "doc_block_line": doc[0] if doc else None,
        "doc_block_excerpt": (doc[1][:400] if doc else None),
    }


def scan(being: str, window: int) -> dict:
    directory = CORPORA[being]
    files = sorted(
        (p for p in directory.glob("introspection_*.txt") if p.is_file()),
        key=lambda p: p.stat().st_mtime,
        reverse=True,
    )[:window]
    findings = []
    for path in files:
        text = path.read_text(encoding="utf-8", errors="replace")
        seen = set()
        for match in CITATION.finditer(text):
            key = (match.group("path"), int(match.group("line")))
            if key in seen:
                continue
            seen.add(key)
            row = inspect(*key)
            if row and row["verdict"] == "negative_assertion":
                findings.append({"artifact": path.name, **row})
    return {
        "schema": "negative_assertion_lead_watch_v1",
        "being": being,
        "scanned": len(files),
        "alarm": bool(findings),
        "finding_count": len(findings),
        "findings": findings,
        "authority": "read_only_evidence_no_live_change_no_being_delivery",
    }


def _render(result: dict) -> str:
    head = f"negative-assertion lead watch — {result['being']}, {result['scanned']} artifacts"
    if not result["alarm"]:
        return f"{head}\n  OK — no cited line resolved to a negative assertion."
    out = [f"{head}\n  ALARM — {result['finding_count']} cited line(s) refute the lead they gave:"]
    for f in result["findings"]:
        out.append(f"  - {f['artifact']}")
        out.append(f"      cited {f['path']} {f['line']}: {f['text']}")
        out.append(f"      context line {f['context_line']}: {f['context_evidence']}")
        if f["doc_block_line"]:
            out.append(f"      answer already at line {f['doc_block_line']} (FIND does not carry it)")
    return "\n".join(out)


class NegativeAssertionLeadWatchTests(unittest.TestCase):
    FIXTURE = [
        "fn render() -> String { String::new() }",
        "",
        "#[cfg(test)]",
        "mod tests {",
        "    /// Astrid asked whether render performs the synthesis.",
        "    /// It does not; the terms are assembled elsewhere.",
        "    #[test]",
        "    fn render_is_not_an_aggregator() {",
        "        let report = render();",
        "        for absent in [",
        '            "matched_terms",',
        '            "multi-motif",',
        "        ] {",
        "            assert!(!report.contains(absent), \"must not carry: {absent}\");",
        "        }",
        "    }",
        "}",
    ]

    def test_negative_binder_array_is_flagged(self):
        # line 12 (1-based) is the "multi-motif" element
        self.assertEqual(classify(self.FIXTURE, 12)["verdict"], "negative_assertion")

    def test_positive_assertion_is_not_flagged(self):
        lines = ["fn f() {}", "    let r = render();", '    assert!(r.contains("multi-motif"));']
        self.assertEqual(classify(lines, 3)["verdict"], "positive_assertion")

    def test_test_remainder_is_position_based(self):
        self.assertTrue(in_test_remainder(self.FIXTURE, 12))
        self.assertFalse(in_test_remainder(self.FIXTURE, 1))

    def test_doc_block_above_the_item_is_recovered(self):
        found = nearest_doc_block(self.FIXTURE, 12)
        self.assertIsNotNone(found)
        self.assertIn("It does not", found[1])

    def test_out_of_range_line_is_reported_not_raised(self):
        self.assertEqual(classify(self.FIXTURE, 9999)["verdict"], "out_of_range")

    def test_citation_regex_matches_a_find_row_shape(self):
        text = "`astrid/capsules/spectral-bridge/src/autonomous/next_action/pressure_agency.rs 765`: ok"
        match = CITATION.search(text)
        self.assertIsNotNone(match)
        self.assertEqual(int(match.group("line")), 765)

    def test_resolve_refuses_escape_from_the_repository(self):
        self.assertIsNone(resolve("astrid/../../etc/passwd"))
        self.assertIsNone(resolve("nowhere/foo.rs"))

    def test_live_worked_example_still_reads_as_negative(self):
        row = inspect(
            "astrid/capsules/spectral-bridge/src/autonomous/next_action/pressure_agency.rs", 765
        )
        if row is None:
            self.skipTest("pressure_agency.rs not available in this checkout")
        self.assertEqual(row["verdict"], "negative_assertion")
        self.assertTrue(row["in_test_remainder"])


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="cmd")
    scan_p = sub.add_parser("scan", help="scan a being's recent navigation artifacts")
    scan_p.add_argument("--being", choices=sorted(CORPORA), default="astrid")
    scan_p.add_argument("--window", type=int, default=60)
    scan_p.add_argument("--json", action="store_true")
    ex_p = sub.add_parser("explain", help="classify one cited line")
    ex_p.add_argument("--path", required=True, help="catalog id, e.g. astrid/capsules/.../file.rs")
    ex_p.add_argument("--line", required=True, type=int)
    ex_p.add_argument("--json", action="store_true")
    sub.add_parser("self-test", help="run the unit suite")
    args = parser.parse_args(argv)

    if args.cmd == "self-test":
        suite = unittest.TestLoader().loadTestsFromTestCase(NegativeAssertionLeadWatchTests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1

    if args.cmd == "explain":
        row = inspect(args.path, args.line)
        if row is None:
            print("citation did not resolve inside a known repository")
            return 1
        print(json.dumps(row, indent=2) if args.json else _render(
            {"being": "explain", "scanned": 1, "alarm": row["verdict"] == "negative_assertion",
             "finding_count": 1, "findings": [{"artifact": "(explicit)", **row}]}))
        return 0

    if args.cmd is None:
        parser.print_help()
        return 0

    result = scan(args.being, args.window)
    print(json.dumps(result, indent=2) if args.json else _render(result))
    return 0


if __name__ == "__main__":
    sys.exit(main())
