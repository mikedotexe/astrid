#!/usr/bin/env python3
"""source_page_item_context_watch.py — did a delivered source page begin mid-item?

Steward-only, read-only. No being delivery, no live surface, no git, no deploy.

Why this exists
---------------
`astrid-source-study`'s pager (`crates/astrid-source-study/src/page.rs`) renders a
page header of the form

    SOURCE <id>
    Revision sha256:<sha>; <bytes> bytes; <lines> lines. ...
    Exact source bytes <start>..<end>; line fragments retain their line number.

The header names a byte interval. It never names the *syntactic item* the page
starts inside, nor the doc comment attached to that item. A page can therefore
begin on the second line of a function body, and the being reading it has no way
to see the declaration or the `///` block above the window.

Worked example (2026-09-11, `introspection_source_catalog_1789165450`):
Astrid's page of `capsules/spectral-bridge/src/autonomous/next_action/pressure_agency.rs`
covered bytes 30095..32769, which begins exactly on line 760, `for absent in [`.
Line 757 `#[test]`, line 758 `fn status_render_is_a_telemetry_formatter_not_a_motif_aggregator`
and the ten-line `///` block at 746-756 — written *to her* in an earlier steward
round, and answering her exact question — were all one to fourteen lines above the
window. She read the remaining `for absent in [...]` list as a list of vocabulary
that "is correctly present".

This tool reports that condition so a steward can see it. It asserts nothing about
why a being read a page a particular way, and it never rewrites or judges her text.

Usage
-----
  scan   --file <repo-relative or absolute .rs> --start-byte N [--end-byte M]
  report --introspection <canonical introspection .txt>   (reads the declared
         `Source:` / `Source revision: ...; bytes A..B` header)
  self-test
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import unittest
from pathlib import Path

SCHEMA = "source_page_item_context_watch_v1"
AUTHORITY_BOUNDARY = (
    "read-only steward evidence; no being delivery, no source edit, no live "
    "pressure/fill/PI/controller/sensory/fallback/bridge/peer mutation, no deploy, "
    "no git staging or commit"
)

# A Rust item header we consider a "declaration" for breadcrumb purposes.
ITEM_RE = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+|const\s+|unsafe\s+|extern\s+\"[^\"]*\"\s+)*"
    r"(fn|struct|enum|trait|impl|mod|type|union)\b"
)
ATTR_RE = re.compile(r"^\s*#\[")
DOC_RE = re.compile(r"^\s*///")


def _lines(path: Path) -> list[str]:
    return path.read_text(encoding="utf-8", errors="replace").split("\n")


def line_of_byte(data: bytes, byte_offset: int) -> tuple[int, int]:
    """Return (1-indexed line number, byte offset of that line's start)."""
    if byte_offset <= 0:
        return 1, 0
    prefix = data[:byte_offset]
    line_no = prefix.count(b"\n") + 1
    line_start = prefix.rfind(b"\n") + 1
    return line_no, line_start


def enclosing_item(lines: list[str], line_no: int) -> dict | None:
    """Nearest preceding item declaration at or above `line_no` (1-indexed)."""
    for index in range(min(line_no, len(lines)) - 1, -1, -1):
        text = lines[index]
        match = ITEM_RE.match(text)
        if match:
            return {"line": index + 1, "kind": match.group(1), "text": text.strip()[:200]}
    return None


def attached_doc_block(lines: list[str], decl_line: int) -> dict | None:
    """The `///` block (plus intervening attributes) immediately above a declaration."""
    index = decl_line - 2  # 0-indexed line above the declaration
    first = None
    last = None
    while index >= 0 and (DOC_RE.match(lines[index]) or ATTR_RE.match(lines[index])):
        if DOC_RE.match(lines[index]):
            if last is None:
                last = index
            first = index
        index -= 1
    if first is None or last is None:
        return None
    return {
        "first_line": first + 1,
        "last_line": last + 1,
        "line_count": last - first + 1,
        "first_text": lines[first].strip()[:200],
    }


def scan(path: Path, start_byte: int, end_byte: int | None = None) -> dict:
    data = path.read_bytes()
    total_bytes = len(data)
    lines = data.decode("utf-8", errors="replace").split("\n")
    start_line, line_start_byte = line_of_byte(data, start_byte)
    starts_on_line_boundary = start_byte == line_start_byte
    first_line_text = lines[start_line - 1] if start_line - 1 < len(lines) else ""
    decl = enclosing_item(lines, start_line)
    # A page "begins mid-item" when the enclosing declaration is strictly above
    # the first delivered line.
    begins_mid_item = bool(decl and decl["line"] < start_line)
    doc = attached_doc_block(lines, decl["line"]) if decl else None
    severed_doc_line_count = 0
    if doc and doc["last_line"] < start_line:
        severed_doc_line_count = doc["line_count"]
    result = {
        "schema": SCHEMA,
        "path": str(path),
        "total_bytes": total_bytes,
        "total_lines": len(lines),
        "page_start_byte": start_byte,
        "page_end_byte": end_byte,
        "page_first_line": start_line,
        "page_starts_on_line_boundary": starts_on_line_boundary,
        "page_first_line_text": first_line_text.strip()[:200],
        "enclosing_item": decl,
        "begins_mid_item": begins_mid_item,
        "severed_header_line_count": (start_line - decl["line"]) if begins_mid_item else 0,
        "attached_doc_block": doc,
        "severed_doc_line_count": severed_doc_line_count,
        "suggested_open_line": (doc["first_line"] if doc else decl["line"]) if decl else None,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }
    return result


HEADER_RE = re.compile(r"bytes\s+(\d+)\.\.(\d+)")
SOURCE_RE = re.compile(r"^Source:\s*(.+?)\s*$", re.MULTILINE)


def report(introspection: Path, repo_roots: dict[str, Path]) -> dict:
    text = introspection.read_text(encoding="utf-8", errors="replace")
    source_match = SOURCE_RE.search(text)
    byte_match = HEADER_RE.search(text)
    if not source_match or not byte_match:
        return {
            "schema": SCHEMA,
            "introspection": str(introspection),
            "page_declared": False,
            "note": "report declares no source page (navigation-only or FIND turn)",
            "authority_boundary": AUTHORITY_BOUNDARY,
        }
    declared = source_match.group(1).strip()
    repo, _, relative = declared.partition("/")
    root = repo_roots.get(repo)
    if root is None:
        return {
            "schema": SCHEMA,
            "introspection": str(introspection),
            "page_declared": True,
            "declared_source": declared,
            "resolved": False,
            "note": f"unknown repository prefix {repo!r}",
            "authority_boundary": AUTHORITY_BOUNDARY,
        }
    path = (root / relative).resolve()
    if not str(path).startswith(str(root.resolve())) or not path.is_file():
        return {
            "schema": SCHEMA,
            "introspection": str(introspection),
            "page_declared": True,
            "declared_source": declared,
            "resolved": False,
            "note": "declared source does not resolve inside its own repository",
            "authority_boundary": AUTHORITY_BOUNDARY,
        }
    out = scan(path, int(byte_match.group(1)), int(byte_match.group(2)))
    out["introspection"] = str(introspection)
    out["declared_source"] = declared
    out["page_declared"] = True
    out["resolved"] = True
    return out


DEFAULT_ROOTS = {
    "astrid": Path("/Users/v/other/astrid"),
    "minime": Path("/Users/v/other/minime"),
}


class SourcePageItemContextTests(unittest.TestCase):
    def _write(self, body: str) -> Path:
        import tempfile

        handle = tempfile.NamedTemporaryFile("w", suffix=".rs", delete=False)
        handle.write(body)
        handle.close()
        return Path(handle.name)

    BODY = (
        "/// One.\n"
        "/// Two.\n"
        "#[test]\n"
        "fn demo() {\n"
        '    for absent in [\n'
        '        "multi-motif",\n'
        "    ] {\n"
        "        assert!(!report.contains(absent));\n"
        "    }\n"
        "}\n"
    )

    def test_page_starting_inside_a_function_reports_mid_item(self):
        path = self._write(self.BODY)
        data = path.read_bytes()
        start = data.index(b"    for absent in [")
        out = scan(path, start)
        self.assertEqual(out["page_first_line"], 5)
        self.assertTrue(out["page_starts_on_line_boundary"])
        self.assertTrue(out["begins_mid_item"])
        self.assertEqual(out["enclosing_item"]["line"], 4)
        self.assertEqual(out["enclosing_item"]["kind"], "fn")
        self.assertEqual(out["severed_header_line_count"], 1)
        self.assertEqual(out["attached_doc_block"]["first_line"], 1)
        self.assertEqual(out["severed_doc_line_count"], 2)
        self.assertEqual(out["suggested_open_line"], 1)

    def test_page_starting_at_the_declaration_is_not_mid_item(self):
        path = self._write(self.BODY)
        data = path.read_bytes()
        out = scan(path, data.index(b"fn demo"))
        self.assertFalse(out["begins_mid_item"])
        self.assertEqual(out["severed_header_line_count"], 0)

    def test_byte_offset_mid_line_is_reported_as_not_on_a_line_boundary(self):
        path = self._write(self.BODY)
        data = path.read_bytes()
        out = scan(path, data.index(b"fn demo") + 2)
        self.assertFalse(out["page_starts_on_line_boundary"])

    def test_file_with_no_item_declaration_yields_no_breadcrumb(self):
        path = self._write("let x = 1;\nlet y = 2;\n")
        out = scan(path, 11)
        self.assertIsNone(out["enclosing_item"])
        self.assertFalse(out["begins_mid_item"])
        self.assertIsNone(out["suggested_open_line"])

    def test_worked_example_pressure_agency_page_760(self):
        path = Path(
            "/Users/v/other/astrid/capsules/spectral-bridge/src/autonomous/"
            "next_action/pressure_agency.rs"
        )
        if not path.is_file():
            self.skipTest("worked-example source not present")
        out = scan(path, 30095, 32769)
        self.assertEqual(out["page_first_line"], 760)
        self.assertTrue(out["page_starts_on_line_boundary"])
        self.assertTrue(out["begins_mid_item"])
        self.assertEqual(out["enclosing_item"]["kind"], "fn")
        self.assertIn("status_render_is_a_telemetry_formatter", out["enclosing_item"]["text"])
        self.assertGreaterEqual(out["severed_doc_line_count"], 1)

    def test_report_on_a_navigation_only_introspection_declares_no_page(self):
        import tempfile

        handle = tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False)
        handle.write("Source: source catalog\nSource revision: navigation only\n")
        handle.close()
        out = report(Path(handle.name), DEFAULT_ROOTS)
        self.assertFalse(out["page_declared"])

    def test_report_refuses_a_path_outside_its_repository(self):
        import tempfile

        handle = tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False)
        handle.write(
            "Source: astrid/../../etc/passwd\n"
            "Source revision: sha256:deadbeef; bytes 0..10\n"
        )
        handle.close()
        out = report(Path(handle.name), DEFAULT_ROOTS)
        self.assertFalse(out["resolved"])


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", maxsplit=1)[0])
    sub = parser.add_subparsers(dest="command", required=True)
    scan_cmd = sub.add_parser("scan")
    scan_cmd.add_argument("--file", required=True)
    scan_cmd.add_argument("--start-byte", type=int, required=True)
    scan_cmd.add_argument("--end-byte", type=int)
    report_cmd = sub.add_parser("report")
    report_cmd.add_argument("--introspection", required=True)
    sub.add_parser("self-test")
    args = parser.parse_args(argv)
    if args.command == "self-test":
        suite = unittest.TestLoader().loadTestsFromTestCase(SourcePageItemContextTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    if args.command == "scan":
        print(json.dumps(scan(Path(args.file), args.start_byte, args.end_byte), indent=2))
        return 0
    print(json.dumps(report(Path(args.introspection), DEFAULT_ROOTS), indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
