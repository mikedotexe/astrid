#!/usr/bin/env python3
"""Bounded recent introspection signal across Astrid and Minime public lanes.

This is deliberately shallow: it scans only known artifact directories and caps
both candidate count and bytes read so a steward check cannot drift into a broad
history sweep.
"""

from __future__ import annotations

import argparse
import json
import re
import time
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ASTRID_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_ASTRID_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
DEFAULT_MINIME_WORKSPACE = ASTRID_ROOT.parent / "minime" / "workspace"
SCHEMA_VERSION = 1


@dataclass(frozen=True)
class ScanSpec:
    channel: str
    root: Path
    pattern: str


@dataclass(frozen=True)
class Candidate:
    channel: str
    path: Path
    mtime: float


SIGNALS: tuple[tuple[str, re.Pattern[str], str], ...] = (
    (
        "pressure_relief",
        re.compile(r"\b(PRESSURE_RELIEF|RELIEF|pressure relief|fill pressure)\b", re.I),
        "Keep the pressure-relief route and tests close at hand when overpacked/fill pressure recurs.",
    ),
    (
        "rewrite_latency",
        re.compile(r"\b(rewrite_seconds|rewrite stage|rewrite latency|rewrite budget|rewrite cap|timeout)\b", re.I),
        "Use the bounded rewrite digest before changing generation behavior.",
    ),
    (
        "muffled_action",
        re.compile(r"\b(un-?muffle|muffled|silently drop|fallthrough|fall through|unknown NEXT|unwired)\b", re.I),
        "Run the action-surface coverage guard before trusting a new being-invokable verb.",
    ),
    (
        "continuity_deficit",
        re.compile(r"\b(continuity_deficit|continuity deficit|re-entry|reentry)\b", re.I),
        "Prefer compact re-entry/context repair before adding more live behavior.",
    ),
    (
        "mode_packing",
        re.compile(r"\b(overpacked|mode[_ -]?packing|modal crowding|compression pressure)\b", re.I),
        "Separate spectral crowding from stale context before applying runtime nudges.",
    ),
    (
        "chamber_uptake",
        re.compile(r"\b(CHAMBER_SEEN|CHAMBER_ANNOTATE|presence protocol|annotation lane|triadic chamber)\b", re.I),
        "Check public chamber receipts/annotations for uptake before reading private lanes.",
    ),
)


def scan_specs(astrid_workspace: Path, minime_workspace: Path) -> list[ScanSpec]:
    return [
        ScanSpec(
            "astrid_introspection_digest_markdown",
            astrid_workspace,
            "diagnostics/introspection_feedback_digest/latest.md",
        ),
        ScanSpec(
            "astrid_introspection_digest_json",
            astrid_workspace,
            "diagnostics/introspection_feedback_digest/latest.json",
        ),
        ScanSpec(
            "astrid_autonomous_introspection",
            astrid_workspace,
            "introspections/controller_astrid:autonomous_*.json",
        ),
        ScanSpec(
            "minime_astrid_self_study_inbox",
            minime_workspace,
            "inbox/astrid_self_study_*.txt",
        ),
        ScanSpec(
            "minime_astrid_self_study_read",
            minime_workspace,
            "inbox/read/astrid_self_study_*.txt",
        ),
        ScanSpec(
            "minime_self_study_action",
            minime_workspace,
            "actions/*_self_study.json",
        ),
        ScanSpec(
            "minime_introspect_action",
            minime_workspace,
            "actions/*_introspect.json",
        ),
        ScanSpec(
            "minime_public_introspection",
            minime_workspace,
            "introspections/*.txt",
        ),
    ]


def _safe_stat(path: Path) -> Any | None:
    try:
        return path.stat()
    except OSError:
        return None


def collect_candidates(
    astrid_workspace: Path,
    minime_workspace: Path,
    *,
    per_location_limit: int = 8,
    since_hours: float | None = None,
) -> tuple[list[Candidate], list[dict[str, Any]]]:
    cutoff = time.time() - (since_hours * 3600.0) if since_hours is not None else None
    candidates: list[Candidate] = []
    locations: list[dict[str, Any]] = []
    for spec in scan_specs(astrid_workspace, minime_workspace):
        paths: list[Path] = []
        if spec.root.is_dir():
            paths = [path for path in spec.root.glob(spec.pattern) if path.is_file()]
        scoped: list[Candidate] = []
        for path in paths:
            stat = _safe_stat(path)
            if stat is None:
                continue
            if cutoff is not None and stat.st_mtime < cutoff:
                continue
            scoped.append(Candidate(spec.channel, path, stat.st_mtime))
        scoped.sort(key=lambda item: item.mtime, reverse=True)
        scoped = scoped[: max(per_location_limit, 1)]
        candidates.extend(scoped)
        locations.append(
            {
                "channel": spec.channel,
                "root": str(spec.root),
                "pattern": spec.pattern,
                "matched": len(scoped),
            }
        )
    deduped = {str(candidate.path): candidate for candidate in candidates}
    ordered = sorted(deduped.values(), key=lambda item: item.mtime, reverse=True)
    return ordered, locations


def read_bounded(path: Path, *, max_chars: int) -> str:
    try:
        with path.open("r", encoding="utf-8", errors="replace") as handle:
            return handle.read(max(max_chars, 1))
    except OSError:
        return ""


def signal_hits(text: str) -> dict[str, int]:
    return {
        name: len(pattern.findall(text))
        for name, pattern, _action in SIGNALS
        if pattern.search(text)
    }


def excerpt_for_hits(text: str, *, max_lines: int = 3, max_line_chars: int = 220) -> list[str]:
    lines: list[str] = []
    patterns = [pattern for _name, pattern, _action in SIGNALS]
    for line in text.splitlines():
        stripped = " ".join(line.strip().split())
        if not stripped:
            continue
        if any(pattern.search(stripped) for pattern in patterns):
            lines.append(stripped[:max_line_chars])
        if len(lines) >= max_lines:
            break
    if lines:
        return lines
    for line in text.splitlines():
        stripped = " ".join(line.strip().split())
        if stripped:
            return [stripped[:max_line_chars]]
    return []


def build_signal(
    astrid_workspace: Path,
    minime_workspace: Path,
    *,
    limit: int = 12,
    per_location_limit: int = 8,
    max_chars: int = 8000,
    since_hours: float | None = None,
) -> dict[str, Any]:
    candidates, locations = collect_candidates(
        astrid_workspace,
        minime_workspace,
        per_location_limit=per_location_limit,
        since_hours=since_hours,
    )
    entries: list[dict[str, Any]] = []
    counts: Counter[str] = Counter()
    for candidate in candidates[: max(limit, 1)]:
        text = read_bounded(candidate.path, max_chars=max_chars)
        hits = signal_hits(text)
        counts.update(hits)
        entries.append(
            {
                "channel": candidate.channel,
                "path": str(candidate.path),
                "mtime": round(candidate.mtime, 3),
                "bytes_read_cap": max_chars,
                "signals": hits,
                "excerpt": excerpt_for_hits(text),
            }
        )
    actions = suggested_actions(counts)
    return {
        "schema_version": SCHEMA_VERSION,
        "policy": "bounded_recent_introspection_signal_v1",
        "authority": "diagnostic_context_not_command",
        "window": {
            "source_limit": max(limit, 1),
            "per_location_limit": max(per_location_limit, 1),
            "max_chars_per_file": max(max_chars, 1),
            "since_hours": since_hours,
        },
        "locations": locations,
        "source_count": len(entries),
        "signal_counts": dict(counts),
        "suggested_next": actions,
        "sources": entries,
    }


def suggested_actions(counts: Counter[str]) -> list[str]:
    actions: list[str] = []
    for name, _pattern, action in SIGNALS:
        if counts.get(name, 0) > 0 and action not in actions:
            actions.append(action)
    if not actions:
        actions.append("No repeated introspection signal crossed the bounded scan window.")
    return actions


def render_markdown(report: dict[str, Any]) -> str:
    window = report.get("window") if isinstance(report.get("window"), dict) else {}
    counts = report.get("signal_counts") if isinstance(report.get("signal_counts"), dict) else {}
    lines = [
        "# Recent Introspection Signal",
        "",
        "Read-only bounded diagnostic context, not a command.",
        "",
        f"- Sources scanned: {report.get('source_count', 0)}",
        f"- Source limit: {window.get('source_limit')} files",
        f"- Per-location limit: {window.get('per_location_limit')} files",
        f"- Per-file read cap: {window.get('max_chars_per_file')} chars",
        "",
        "## Actionable Signals",
        "",
    ]
    for action in report.get("suggested_next") or []:
        lines.append(f"- {action}")
    lines.extend(["", "## Signal Counts", ""])
    if counts:
        for name, count in sorted(counts.items(), key=lambda item: (-int(item[1]), item[0])):
            lines.append(f"- {name}: {count}")
    else:
        lines.append("- none")
    lines.extend(["", "## Recent Sources", ""])
    for entry in report.get("sources") or []:
        hit_text = ", ".join(f"{name}={count}" for name, count in sorted((entry.get("signals") or {}).items()))
        lines.append(f"- {entry.get('channel')}: `{entry.get('path')}`")
        lines.append(f"  - Signals: {hit_text or 'none'}")
        for excerpt in entry.get("excerpt") or []:
            lines.append(f"  - {excerpt}")
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description="Build a bounded recent introspection signal report")
    parser.add_argument("--astrid-workspace", type=Path, default=DEFAULT_ASTRID_WORKSPACE)
    parser.add_argument("--minime-workspace", type=Path, default=DEFAULT_MINIME_WORKSPACE)
    parser.add_argument("--limit", type=int, default=12)
    parser.add_argument("--per-location-limit", type=int, default=8)
    parser.add_argument("--max-chars", type=int, default=8000)
    parser.add_argument("--since-hours", type=float)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    report = build_signal(
        args.astrid_workspace,
        args.minime_workspace,
        limit=args.limit,
        per_location_limit=args.per_location_limit,
        max_chars=args.max_chars,
        since_hours=args.since_hours,
    )
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(render_markdown(report), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
