#!/usr/bin/env python3
"""Pre-commit WIP hygiene report — distinguish today's intent from
prior accumulated WIP on a savepoint branch.

Today's commits across the v5.1 phases routinely show "2000+ insertions
for ~150 lines of intent" because savepoint branches accumulate
modifications across many sessions. The diffs are recoverable, but
`git log` becomes unreadable for "what shipped when."

This script lists git status entries (M / A / D / ??) bucketed by file
mtime against a configurable cutoff (default: today's start at local
00:00). Files modified since the cutoff are likely the steward's
current intent; files modified before are prior session WIP that the
steward should consciously decide whether to include in this commit.

Usage:
  python3 scripts/wip_hygiene.py                     # cwd, cutoff=today 00:00
  python3 scripts/wip_hygiene.py /Users/v/other/minime
  python3 scripts/wip_hygiene.py --since "08:00"     # cutoff=8am today
  python3 scripts/wip_hygiene.py --since "2026-05-13 17:00"  # explicit datetime
  python3 scripts/wip_hygiene.py --json              # machine-readable output

Advisory tool. The steward decides what to stage. Mirrors the shape of
scripts/architecture_health.py.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from dataclasses import asdict, dataclass
from datetime import datetime, time as dt_time
from pathlib import Path


@dataclass(frozen=True)
class FileEntry:
    status: str  # "M" | "A" | "D" | "??" | "R" | etc.
    path: str
    mtime_unix: float
    mtime_iso: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Bucket git status entries by mtime to surface today's intent vs prior WIP.",
        epilog="Advisory only. Steward decides what to stage.",
    )
    parser.add_argument(
        "root",
        nargs="?",
        default=".",
        help="Repository root to scan (default: current directory).",
    )
    parser.add_argument(
        "--since",
        type=str,
        default=None,
        help=(
            "Cutoff time for 'today's intent' bucket. "
            "Accepts 'HH:MM' (today at that local time) or "
            "'YYYY-MM-DD HH:MM' (explicit datetime). "
            "Default: today's local 00:00."
        ),
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit machine-readable JSON instead of Markdown.",
    )
    return parser.parse_args()


def parse_cutoff(arg: str | None) -> datetime:
    """Convert --since argument to a datetime. Returns today's start (local 00:00) if None."""
    today = datetime.now().date()
    if arg is None:
        return datetime.combine(today, dt_time.min)
    arg = arg.strip()
    # Try HH:MM
    if len(arg) <= 5 and ":" in arg:
        try:
            hh, mm = arg.split(":")
            return datetime.combine(today, dt_time(int(hh), int(mm)))
        except (ValueError, IndexError):
            pass
    # Try ISO-ish "YYYY-MM-DD HH:MM" or "YYYY-MM-DDTHH:MM"
    for fmt in ("%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M:%S"):
        try:
            return datetime.strptime(arg, fmt)
        except ValueError:
            continue
    print(f"warning: could not parse --since '{arg}', falling back to today's 00:00", file=sys.stderr)
    return datetime.combine(today, dt_time.min)


def git_status(root: Path) -> list[tuple[str, str]]:
    """Returns list of (status, path) tuples from `git status --porcelain`.
    Status is the porcelain code (M, A, D, ??, R, etc., possibly two chars
    for dual-state). Path is relative to repo root."""
    try:
        result = subprocess.run(
            ["git", "status", "--porcelain"],
            cwd=str(root),
            capture_output=True,
            text=True,
            check=True,
        )
    except subprocess.CalledProcessError as exc:
        print(f"git status failed: {exc.stderr}", file=sys.stderr)
        return []
    except FileNotFoundError:
        print("git not found in PATH", file=sys.stderr)
        return []

    entries: list[tuple[str, str]] = []
    for line in result.stdout.splitlines():
        if not line.strip():
            continue
        # Porcelain format: "XY path" where XY is two-char status, then space, then path
        if len(line) < 4:
            continue
        status = line[:2].strip() or line[:2]
        path = line[3:].strip()
        # Handle rename: "R  old -> new"
        if " -> " in path:
            path = path.split(" -> ", 1)[1]
        # Strip surrounding quotes (porcelain sometimes quotes paths with spaces)
        if path.startswith('"') and path.endswith('"'):
            path = path[1:-1]
        entries.append((status, path))
    return entries


def collect(root: Path, cutoff: datetime) -> dict[str, object]:
    cutoff_unix = cutoff.timestamp()
    raw_entries = git_status(root)
    files_today: list[FileEntry] = []
    files_prior: list[FileEntry] = []
    files_missing: list[str] = []  # files in status but not on disk (e.g., D status)

    for status, rel_path in raw_entries:
        full = root / rel_path
        try:
            mtime = full.stat().st_mtime
        except (FileNotFoundError, NotADirectoryError):
            files_missing.append(f"{status}  {rel_path}")
            continue
        entry = FileEntry(
            status=status,
            path=rel_path,
            mtime_unix=mtime,
            mtime_iso=datetime.fromtimestamp(mtime).strftime("%Y-%m-%d %H:%M:%S"),
        )
        if mtime >= cutoff_unix:
            files_today.append(entry)
        else:
            files_prior.append(entry)

    files_today.sort(key=lambda e: e.mtime_unix, reverse=True)
    files_prior.sort(key=lambda e: e.mtime_unix, reverse=True)

    return {
        "root": root.as_posix(),
        "cutoff_iso": cutoff.strftime("%Y-%m-%d %H:%M:%S"),
        "cutoff_unix": cutoff_unix,
        "scanned_at_iso": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
        "files_today": [asdict(e) for e in files_today],
        "files_prior": [asdict(e) for e in files_prior],
        "files_missing": files_missing,
    }


def render_table(entries: list[dict], indent: str = "") -> list[str]:
    if not entries:
        return [f"{indent}_(none)_"]
    lines = [
        f"{indent}| Status | Path | Last modified |",
        f"{indent}| --- | --- | --- |",
    ]
    for e in entries:
        lines.append(f"{indent}| `{e['status']}` | `{e['path']}` | {e['mtime_iso']} |")
    return lines


def markdown(report: dict[str, object]) -> str:
    today = report["files_today"]
    prior = report["files_prior"]
    missing = report["files_missing"]
    assert isinstance(today, list)
    assert isinstance(prior, list)

    lines = [
        "# WIP Hygiene Report",
        "",
        f"- Repo: `{report['root']}`",
        f"- Cutoff: `{report['cutoff_iso']}` (files modified at or after this time = 'today's intent')",
        f"- Scanned at: `{report['scanned_at_iso']}`",
        f"- Today's intent: `{len(today)}` file(s)",
        f"- Prior WIP: `{len(prior)}` file(s)",
        "",
        "Advisory only. The steward decides what to stage. Files in the 'prior WIP'",
        "bucket may belong in this commit (carry-forward changes from earlier",
        "sessions you intend to ship now) or may be unrelated WIP that should be",
        "left alone. Inspect each before `git add`.",
        "",
        "## Modified since cutoff (likely today's intent)",
        "",
    ]
    lines.extend(render_table(today))
    lines.append("")
    lines.append("## Modified before cutoff (verify before staging)")
    lines.append("")
    lines.extend(render_table(prior))

    if missing:
        lines.append("")
        lines.append("## Files in status but missing on disk")
        lines.append("")
        lines.append("These are likely deletions or renames; mtime cannot be checked.")
        lines.append("")
        for m in missing:
            lines.append(f"- `{m}`")

    lines.append("")
    lines.append("## Suggested workflow")
    lines.append("")
    lines.append("```bash")
    lines.append("# Stage only today's intent (review the list above first):")
    lines.append("# git add <path1> <path2> ...")
    lines.append("# Or stage selectively with hunks:")
    lines.append("# git add -p <path>")
    lines.append("```")
    return "\n".join(lines)


def main() -> int:
    args = parse_args()
    root = Path(args.root).resolve()
    cutoff = parse_cutoff(args.since)
    report = collect(root, cutoff)
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(markdown(report))
    return 0


if __name__ == "__main__":
    sys.exit(main())
