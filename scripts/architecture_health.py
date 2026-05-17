#!/usr/bin/env python3
"""Advisory architecture-health report for source shape pressure.

This script intentionally does not enforce a hard 1000-line cap by default.
It surfaces review signals so maintainers can decide whether a large file is
cohesive, should be split, or is an exempt generated/fixture/registry surface.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


SOURCE_SUFFIXES = {
    ".c",
    ".cc",
    ".cpp",
    ".css",
    ".go",
    ".h",
    ".hpp",
    ".html",
    ".js",
    ".jsx",
    ".mjs",
    ".py",
    ".rs",
    ".sh",
    ".ts",
    ".tsx",
    ".toml",
    ".yaml",
    ".yml",
}

DOC_SUFFIXES = {".md", ".rst"}

EXCLUDED_PARTS = {
    ".git",
    ".pytest_cache",
    "__pycache__",
    "node_modules",
    "target",
    "workspace",
}

EXCLUDED_FRAGMENTS = {
    "/review-bundles/",
    "/fixtures/",
    "/fixture/",
    "/snapshots/",
    "/generated/",
    "/vendor/",
}

WARN_LINES = 1_000
REVIEW_LINES = 1_500
CRITICAL_LINES = 2_500
LONG_FUNCTION_LINES = 120
VERY_LONG_FUNCTION_LINES = 220


@dataclass(frozen=True)
class FileSignal:
    path: str
    lines: int
    level: str
    public_items: int
    suggestion: str


@dataclass(frozen=True)
class FunctionSignal:
    path: str
    name: str
    start_line: int
    lines: int
    level: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Report source architecture-health signals."
    )
    parser.add_argument(
        "root",
        nargs="?",
        default=".",
        help="Repository root to scan, default: current directory.",
    )
    parser.add_argument(
        "--include-docs",
        action="store_true",
        help="Include Markdown/RST files. Long-form docs are excluded by default.",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit machine-readable JSON instead of Markdown.",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=20,
        help="Maximum rows per section in Markdown output.",
    )
    parser.add_argument(
        "--fail-on-critical",
        action="store_true",
        help="Exit non-zero when critical file or function signals are present.",
    )
    return parser.parse_args()


def should_scan(path: Path, root: Path, include_docs: bool) -> bool:
    rel = path.relative_to(root)
    if any(part in EXCLUDED_PARTS for part in rel.parts):
        return False
    rel_text = f"/{rel.as_posix()}"
    if any(fragment in rel_text for fragment in EXCLUDED_FRAGMENTS):
        return False
    if path.suffix in SOURCE_SUFFIXES:
        return True
    return include_docs and path.suffix in DOC_SUFFIXES


def source_files(root: Path, include_docs: bool) -> list[Path]:
    paths: list[Path] = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [
            name
            for name in dirnames
            if name not in EXCLUDED_PARTS
            and not any(
                fragment in f"/{Path(dirpath, name).relative_to(root).as_posix()}/"
                for fragment in EXCLUDED_FRAGMENTS
            )
        ]
        for filename in filenames:
            path = Path(dirpath, filename)
            if should_scan(path, root, include_docs):
                paths.append(path)
    return sorted(paths)


def read_lines(path: Path) -> list[str]:
    try:
        return path.read_text(encoding="utf-8").splitlines()
    except UnicodeDecodeError:
        return path.read_text(encoding="utf-8", errors="ignore").splitlines()


def file_level(line_count: int) -> str:
    if line_count >= CRITICAL_LINES:
        return "critical"
    if line_count >= REVIEW_LINES:
        return "review"
    return "watch"


def file_suggestion(line_count: int, public_items: int) -> str:
    if line_count >= CRITICAL_LINES:
        return "write decomposition plan unless exempt"
    if line_count >= REVIEW_LINES:
        return "add review note or split by ownership"
    if public_items >= 25:
        return "check facade/API pressure"
    return "watch cohesion before expanding"


def public_item_count(path: Path, lines: list[str]) -> int:
    if path.suffix == ".rs":
        pattern = re.compile(
            r"^\s*pub(?:\([^)]*\))?\s+(?:async\s+)?"
            r"(?:fn|struct|enum|trait|type|const|static|mod)\b"
        )
    elif path.suffix == ".py":
        pattern = re.compile(r"^\s*(?:async\s+)?def\s+|^\s*class\s+")
    else:
        return 0
    return sum(1 for line in lines if pattern.search(line))


def function_pattern(path: Path) -> re.Pattern[str] | None:
    if path.suffix == ".rs":
        return re.compile(
            r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?"
            r"(?:const\s+)?(?:unsafe\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\b"
        )
    if path.suffix == ".py":
        return re.compile(r"^\s*(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)\b")
    if path.suffix == ".sh":
        return re.compile(
            r"^\s*(?:function\s+)?([A-Za-z_][A-Za-z0-9_-]*)\s*\(\)\s*\{"
        )
    return None


def function_signals(path: Path, rel: str, lines: list[str]) -> list[FunctionSignal]:
    pattern = function_pattern(path)
    if pattern is None:
        return []

    starts: list[tuple[int, str]] = []
    for index, line in enumerate(lines, start=1):
        match = pattern.search(line)
        if match:
            starts.append((index, match.group(1)))

    signals: list[FunctionSignal] = []
    for idx, (start, name) in enumerate(starts):
        end = starts[idx + 1][0] - 1 if idx + 1 < len(starts) else len(lines)
        span = end - start + 1
        if span >= LONG_FUNCTION_LINES:
            level = "critical" if span >= VERY_LONG_FUNCTION_LINES else "review"
            signals.append(FunctionSignal(rel, name, start, span, level))
    return signals


def collect(root: Path, include_docs: bool) -> dict[str, object]:
    files = source_files(root, include_docs)
    file_signals: list[FileSignal] = []
    fn_signals: list[FunctionSignal] = []

    for path in files:
        lines = read_lines(path)
        rel = path.relative_to(root).as_posix()
        public_items = public_item_count(path, lines)
        line_count = len(lines)
        if line_count >= WARN_LINES:
            file_signals.append(
                FileSignal(
                    rel,
                    line_count,
                    file_level(line_count),
                    public_items,
                    file_suggestion(line_count, public_items),
                )
            )
        fn_signals.extend(function_signals(path, rel, lines))

    file_signals.sort(key=lambda item: item.lines, reverse=True)
    fn_signals.sort(key=lambda item: item.lines, reverse=True)
    critical_count = sum(1 for item in file_signals if item.level == "critical")
    critical_count += sum(1 for item in fn_signals if item.level == "critical")

    return {
        "root": root.as_posix(),
        "scanned_files": len(files),
        "thresholds": {
            "file_watch_lines": WARN_LINES,
            "file_review_lines": REVIEW_LINES,
            "file_critical_lines": CRITICAL_LINES,
            "function_review_lines": LONG_FUNCTION_LINES,
            "function_critical_lines": VERY_LONG_FUNCTION_LINES,
        },
        "large_files": [asdict(item) for item in file_signals],
        "long_functions": [asdict(item) for item in fn_signals],
        "critical_signal_count": critical_count,
    }


def markdown(report: dict[str, object], limit: int) -> str:
    large_files = report["large_files"]
    long_functions = report["long_functions"]
    lines = [
        "# Architecture Health Report",
        "",
        f"- Root: `{report['root']}`",
        f"- Source files scanned: `{report['scanned_files']}`",
        f"- Large-file signals: `{len(large_files)}`",
        f"- Long-function signals: `{len(long_functions)}`",
        f"- Critical signals: `{report['critical_signal_count']}`",
        "",
        "This report is advisory by default. A large file is a review prompt, not an automatic failure.",
        "",
    ]

    lines.append("## Large Files")
    if large_files:
        lines.append("")
        lines.append("| Lines | Level | Public Items | Path | Suggestion |")
        lines.append("| ---: | --- | ---: | --- | --- |")
        for item in large_files[:limit]:
            lines.append(
                f"| {item['lines']} | {item['level']} | {item['public_items']} | "
                f"`{item['path']}` | {item['suggestion']} |"
            )
        if len(large_files) > limit:
            lines.append(f"\n_...{len(large_files) - limit} more large files omitted._")
    else:
        lines.append("\nNo large source files found.")

    lines.append("")
    lines.append("## Long Functions")
    if long_functions:
        lines.append("")
        lines.append("| Lines | Level | Function | Path |")
        lines.append("| ---: | --- | --- | --- |")
        for item in long_functions[:limit]:
            lines.append(
                f"| {item['lines']} | {item['level']} | `{item['name']}` "
                f"at line {item['start_line']} | `{item['path']}` |"
            )
        if len(long_functions) > limit:
            lines.append(f"\n_...{len(long_functions) - limit} more long functions omitted._")
    else:
        lines.append("\nNo long function spans found.")

    lines.append("")
    lines.append("## Review Lens")
    lines.append("")
    lines.append("- Split when the file now holds multiple ownership boundaries.")
    lines.append("- Keep a large file when it is cohesive, deliberate, and easier to audit intact.")
    lines.append("- Prefer extracting policy, parsing, rendering, storage, and diagnostics before runtime loop code grows.")
    lines.append("- Treat generated files, fixtures, schema tables, registries, and long-form docs as ordinary exceptions.")
    return "\n".join(lines)


def main() -> int:
    args = parse_args()
    root = Path(args.root).resolve()
    report = collect(root, args.include_docs)
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(markdown(report, args.limit))
    if args.fail_on_critical and report["critical_signal_count"]:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
