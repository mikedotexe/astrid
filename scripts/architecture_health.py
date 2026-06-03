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
BASELINE_DIR = Path(__file__).resolve().parent / "baselines"
DEFAULT_BASELINE = BASELINE_DIR / "architecture_health.json"
LEVEL_RANK = {"watch": 1, "review": 2, "critical": 3}
DEFAULT_GROWTH_TOLERANCE_LINES = 50
DEFAULT_GROWTH_TOLERANCE_RATIO = 0.05


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
        "--baseline",
        type=Path,
        default=DEFAULT_BASELINE,
        help=f"Accepted-debt baseline JSON (default: {DEFAULT_BASELINE}).",
    )
    parser.add_argument(
        "--no-baseline",
        action="store_true",
        help="Disable accepted-debt baseline classification.",
    )
    parser.add_argument(
        "--show-accepted",
        action="store_true",
        help="Show accepted baseline entries in Markdown output.",
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


def load_baseline(path: Path | None) -> dict[str, object] | None:
    if path is None or not path.is_file():
        return None
    with path.open(encoding="utf-8") as f:
        data = json.load(f)
    if not isinstance(data, dict):
        raise ValueError(f"baseline must be a JSON object: {path}")
    return data


def baseline_entries(value: object) -> list[dict[str, object]]:
    if isinstance(value, list):
        return [item for item in value if isinstance(item, dict)]
    if isinstance(value, dict):
        return [item for item in value.values() if isinstance(item, dict)]
    return []


def function_key(item: dict[str, object]) -> tuple[str, str]:
    return (str(item.get("path", "")), str(item.get("name", "")))


def line_count(item: dict[str, object]) -> int:
    try:
        return int(item.get("lines", 0))
    except (TypeError, ValueError):
        return 0


def material_growth(
    current: dict[str, object],
    accepted: dict[str, object],
    tolerance_lines: int,
    tolerance_ratio: float,
) -> bool:
    accepted_lines = line_count(accepted)
    current_lines = line_count(current)
    tolerated = max(
        accepted_lines + tolerance_lines,
        int(accepted_lines * (1.0 + tolerance_ratio)),
    )
    return current_lines > tolerated


def classify_item(
    current: dict[str, object],
    accepted: dict[str, object] | None,
    tolerance_lines: int,
    tolerance_ratio: float,
) -> dict[str, object]:
    out = dict(current)
    if accepted is None:
        out["baseline_status"] = "new"
        out["baseline_reason"] = "not present in accepted baseline"
        return out

    accepted_level = str(accepted.get("level") or accepted.get("severity") or "watch")
    current_level = str(current.get("level") or current.get("severity") or "watch")
    out["baseline_level"] = accepted_level
    out["baseline_lines"] = line_count(accepted)

    if LEVEL_RANK.get(current_level, 0) > LEVEL_RANK.get(accepted_level, 0):
        out["baseline_status"] = "worsened"
        out["baseline_reason"] = f"level rose from {accepted_level} to {current_level}"
    elif material_growth(current, accepted, tolerance_lines, tolerance_ratio):
        out["baseline_status"] = "worsened"
        out["baseline_reason"] = (
            f"grew from {line_count(accepted)} to {line_count(current)} lines"
        )
    else:
        out["baseline_status"] = "accepted"
        out["baseline_reason"] = "within accepted baseline"
    return out


def set_unbaselined_report(
    report: dict[str, object],
    large_files: list[dict[str, object]],
    long_functions: list[dict[str, object]],
    reason: str,
    baseline_path: Path | None = None,
) -> dict[str, object]:
    report["baseline"] = {
        "enabled": False,
        "reason": reason,
        "path": baseline_path.as_posix() if baseline_path else None,
    }
    report["actionable_large_files"] = large_files
    report["actionable_long_functions"] = long_functions
    report["accepted_large_files"] = []
    report["accepted_long_functions"] = []
    report["actionable_critical_signal_count"] = report["critical_signal_count"]
    return report


def partition_classified(
    items: list[dict[str, object]],
) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
    actionable = [item for item in items if item.get("baseline_status") != "accepted"]
    accepted = [item for item in items if item.get("baseline_status") == "accepted"]
    return actionable, accepted


def attach_summary(report: dict[str, object]) -> dict[str, object]:
    report["summary"] = {
        "raw_large_files": len(report.get("large_files", [])),
        "raw_long_functions": len(report.get("long_functions", [])),
        "accepted_large_files": len(report.get("accepted_large_files", [])),
        "accepted_long_functions": len(report.get("accepted_long_functions", [])),
        "actionable_large_files": len(report.get("actionable_large_files", [])),
        "actionable_long_functions": len(report.get("actionable_long_functions", [])),
        "critical_signal_count": report.get("critical_signal_count", 0),
        "actionable_critical_signal_count": report.get(
            "actionable_critical_signal_count", 0
        ),
    }
    return report


def apply_baseline(
    report: dict[str, object],
    baseline_path: Path | None,
    disabled: bool,
) -> dict[str, object]:
    large_files = [item for item in report["large_files"] if isinstance(item, dict)]
    long_functions = [
        item for item in report["long_functions"] if isinstance(item, dict)
    ]

    if disabled:
        return attach_summary(
            set_unbaselined_report(report, large_files, long_functions, "disabled")
        )

    baseline = load_baseline(baseline_path)
    if baseline is None:
        return attach_summary(
            set_unbaselined_report(
                report, large_files, long_functions, "not found", baseline_path
            )
        )

    tolerance_lines = int(
        baseline.get("growth_tolerance_lines", DEFAULT_GROWTH_TOLERANCE_LINES)
    )
    tolerance_ratio = float(
        baseline.get("growth_tolerance_ratio", DEFAULT_GROWTH_TOLERANCE_RATIO)
    )
    accepted_large_by_path = {
        str(item.get("path", "")): item
        for item in baseline_entries(baseline.get("large_files"))
    }
    accepted_fn_by_key = {
        function_key(item): item
        for item in baseline_entries(baseline.get("long_functions"))
    }

    classified_large = [
        classify_item(
            item,
            accepted_large_by_path.get(str(item.get("path", ""))),
            tolerance_lines,
            tolerance_ratio,
        )
        for item in large_files
    ]
    classified_functions = [
        classify_item(
            item,
            accepted_fn_by_key.get(function_key(item)),
            tolerance_lines,
            tolerance_ratio,
        )
        for item in long_functions
    ]

    actionable_large, accepted_large = partition_classified(classified_large)
    actionable_functions, accepted_functions = partition_classified(classified_functions)
    actionable_critical_count = sum(
        1
        for item in [*actionable_large, *actionable_functions]
        if item.get("level") == "critical"
    )

    current_large_paths = {str(item.get("path", "")) for item in large_files}
    current_function_keys = {function_key(item) for item in long_functions}
    resolved_large = sorted(set(accepted_large_by_path) - current_large_paths)
    resolved_functions = sorted(
        f"{path}::{name}"
        for path, name in set(accepted_fn_by_key) - current_function_keys
    )

    report["large_files"] = classified_large
    report["long_functions"] = classified_functions
    report["actionable_large_files"] = actionable_large
    report["actionable_long_functions"] = actionable_functions
    report["accepted_large_files"] = accepted_large
    report["accepted_long_functions"] = accepted_functions
    report["actionable_critical_signal_count"] = actionable_critical_count
    report["baseline"] = {
        "enabled": True,
        "path": baseline_path.as_posix() if baseline_path else None,
        "growth_tolerance_lines": tolerance_lines,
        "growth_tolerance_ratio": tolerance_ratio,
        "accepted_large_files": len(accepted_large),
        "accepted_long_functions": len(accepted_functions),
        "actionable_large_files": len(actionable_large),
        "actionable_long_functions": len(actionable_functions),
        "resolved_large_files": len(resolved_large),
        "resolved_long_functions": len(resolved_functions),
    }
    report["resolved_baseline_large_files"] = resolved_large
    report["resolved_baseline_long_functions"] = resolved_functions
    return attach_summary(report)


def markdown(report: dict[str, object], limit: int, show_accepted: bool) -> str:
    baseline = report.get("baseline")
    baseline_enabled = isinstance(baseline, dict) and bool(baseline.get("enabled"))
    large_files = report["actionable_large_files"] if baseline_enabled else report["large_files"]
    long_functions = (
        report["actionable_long_functions"] if baseline_enabled else report["long_functions"]
    )
    lines = [
        "# Architecture Health Report",
        "",
        f"- Root: `{report['root']}`",
        f"- Source files scanned: `{report['scanned_files']}`",
        f"- Actionable large-file signals: `{len(large_files)}`",
        f"- Actionable long-function signals: `{len(long_functions)}`",
        f"- Actionable critical signals: `{report['actionable_critical_signal_count']}`",
        f"- Raw accepted/current signals: `{len(report['large_files'])}` large files, `{len(report['long_functions'])}` long functions",
        "",
        "This report is advisory by default. A large file is a review prompt, not an automatic failure.",
        "",
    ]
    if baseline_enabled:
        lines.insert(
            8,
            f"- Accepted baseline: `{baseline['accepted_large_files']}` large files, `{baseline['accepted_long_functions']}` long functions",
        )

    lines.append("## Actionable Large Files")
    if large_files:
        lines.append("")
        lines.append("| Lines | Level | Public Items | Path | Reason | Suggestion |")
        lines.append("| ---: | --- | ---: | --- | --- | --- |")
        for item in large_files[:limit]:
            lines.append(
                f"| {item['lines']} | {item['level']} | {item['public_items']} | "
                f"`{item['path']}` | {item.get('baseline_reason', 'current signal')} | "
                f"{item['suggestion']} |"
            )
        if len(large_files) > limit:
            lines.append(f"\n_...{len(large_files) - limit} more large files omitted._")
    else:
        lines.append("\nNo new or worsened large source files found.")

    lines.append("")
    lines.append("## Actionable Long Functions")
    if long_functions:
        lines.append("")
        lines.append("| Lines | Level | Function | Path | Reason |")
        lines.append("| ---: | --- | --- | --- | --- |")
        for item in long_functions[:limit]:
            lines.append(
                f"| {item['lines']} | {item['level']} | `{item['name']}` "
                f"at line {item['start_line']} | `{item['path']}` | "
                f"{item.get('baseline_reason', 'current signal')} |"
            )
        if len(long_functions) > limit:
            lines.append(f"\n_...{len(long_functions) - limit} more long functions omitted._")
    else:
        lines.append("\nNo new or worsened long function spans found.")

    if baseline_enabled and show_accepted:
        lines.append("")
        lines.append("## Accepted Baseline")
        lines.append("")
        lines.append(
            f"- Large files: `{len(report['accepted_large_files'])}` accepted, "
            f"`{len(report['resolved_baseline_large_files'])}` resolved since baseline."
        )
        lines.append(
            f"- Long functions: `{len(report['accepted_long_functions'])}` accepted, "
            f"`{len(report['resolved_baseline_long_functions'])}` resolved since baseline."
        )

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
    report = apply_baseline(report, args.baseline.resolve(), args.no_baseline)
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(markdown(report, args.limit, args.show_accepted))
    if args.fail_on_critical and report["actionable_critical_signal_count"]:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
