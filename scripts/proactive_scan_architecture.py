#!/usr/bin/env python3
"""Architecture-health adapter for proactive_scan.py."""
from __future__ import annotations

import json
import subprocess
from collections import Counter
from pathlib import Path
from typing import Any

ASTRID_REPO = Path("/Users/v/other/astrid")


def _finding(
    name: str,
    severity: str,
    summary: str,
    details: list[str] | None = None,
    snapshot: Any | None = None,
) -> dict[str, Any]:
    return {
        "name": name,
        "severity": severity,
        "summary": summary,
        "details": details,
        "snapshot": snapshot,
    }


def _dict_entries(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [entry for entry in value if isinstance(entry, dict)]


def _run_architecture_health() -> tuple[int, str]:
    script = ASTRID_REPO / "scripts/architecture_health.py"
    if not script.is_file():
        return -1, "architecture_health.py not found"
    try:
        res = subprocess.run(
            ["python3", str(script), "--json"],
            capture_output=True,
            text=True,
            timeout=30,
        )
    except Exception as exc:
        return -1, str(exc)
    return res.returncode, res.stdout


def _architecture_parts(report: dict[str, Any]) -> dict[str, Any]:
    summary_counts = report.get("summary", {})
    if not isinstance(summary_counts, dict):
        summary_counts = {}
    raw_large = _dict_entries(report.get("large_files"))
    raw_long = _dict_entries(report.get("long_functions"))
    action_large = _dict_entries(report.get("actionable_large_files"))
    action_long = _dict_entries(report.get("actionable_long_functions"))
    accepted_large = _dict_entries(report.get("accepted_large_files"))
    accepted_long = _dict_entries(report.get("accepted_long_functions"))
    raw_entries = _dict_entries(report.get("files")) + raw_large + raw_long
    has_actionable = "actionable_large_files" in report or "actionable_long_functions" in report
    return {
        "summary_counts": summary_counts,
        "raw_large_files": raw_large,
        "raw_long_functions": raw_long,
        "raw_entries": raw_entries,
        "entries": action_large + action_long if has_actionable else raw_entries,
        "actionable_large_files": action_large,
        "actionable_long_functions": action_long,
        "accepted_large_files": accepted_large,
        "accepted_long_functions": accepted_long,
    }


def _count(summary_counts: dict[str, Any], key: str, entries: list[dict[str, Any]]) -> int:
    value = summary_counts.get(key)
    return value if isinstance(value, int) else len(entries)


def _snapshot(
    report: dict[str, Any],
    parts: dict[str, Any],
    sev_counts: Counter[str],
    raw_sev_counts: Counter[str],
) -> dict[str, Any]:
    summary = parts["summary_counts"]
    return {
        "sev_counts": dict(sev_counts),
        "raw_sev_counts": dict(raw_sev_counts),
        "critical_signal_count": summary.get(
            "critical_signal_count", report.get("critical_signal_count")
        ),
        "actionable_critical_signal_count": summary.get(
            "actionable_critical_signal_count",
            report.get("actionable_critical_signal_count"),
        ),
        "large_files": _count(summary, "raw_large_files", parts["raw_large_files"]),
        "long_functions": _count(summary, "raw_long_functions", parts["raw_long_functions"]),
        "actionable_large_files": _count(
            summary, "actionable_large_files", parts["actionable_large_files"]
        ),
        "actionable_long_functions": _count(
            summary, "actionable_long_functions", parts["actionable_long_functions"]
        ),
        "accepted_large_files": _count(
            summary, "accepted_large_files", parts["accepted_large_files"]
        ),
        "accepted_long_functions": _count(
            summary, "accepted_long_functions", parts["accepted_long_functions"]
        ),
    }


def _summary(
    prior: dict[str, Any],
    parts: dict[str, Any],
    sev_counts: Counter[str],
    snapshot: dict[str, Any],
) -> str:
    prior_counts = (prior or {}).get("sev_counts", {}) if isinstance(prior, dict) else {}
    deltas = []
    for sev in ("critical", "review", "watch"):
        cur = sev_counts.get(sev, 0)
        old = prior_counts.get(sev, 0)
        if cur != old:
            deltas.append(f"{sev}: {old} -> {cur}")
    summary = (
        f"{sev_counts.get('critical', 0)} actionable critical, "
        f"{sev_counts.get('review', 0)} review, "
        f"{sev_counts.get('watch', 0)} watch"
    )
    if parts["raw_large_files"] or parts["raw_long_functions"]:
        accepted = (snapshot.get("accepted_large_files") or 0) + (
            snapshot.get("accepted_long_functions") or 0
        )
        summary += (
            f" ({snapshot.get('large_files', 0)} raw large files, "
            f"{snapshot.get('long_functions', 0)} raw long functions, "
            f"{accepted} accepted)"
        )
    return summary + (" | delta: " + ", ".join(deltas) if deltas else "")


def probe_architecture_drift(prior: dict[str, Any]) -> dict[str, Any]:
    """Wrap architecture_health.py; report only actionable threshold drift."""
    rc, stdout = _run_architecture_health()
    if rc != 0:
        return _finding("architecture_drift", "notice", "architecture_health.py failed to run")
    try:
        report = json.loads(stdout)
    except Exception:
        return _finding("architecture_drift", "notice", "could not parse architecture_health.py JSON")
    if not isinstance(report, dict):
        return _finding("architecture_drift", "notice", "architecture_health.py JSON was not an object")
    parts = _architecture_parts(report)
    if not parts["raw_entries"] and not parts["entries"]:
        return _finding("architecture_drift", "ok", "architecture_health ran (no signals)")
    sev_counts: Counter[str] = Counter(
        (item.get("severity") or item.get("level") or "ok") for item in parts["entries"]
    )
    raw_sev_counts: Counter[str] = Counter(
        (item.get("severity") or item.get("level") or "ok") for item in parts["raw_entries"]
    )
    snapshot = _snapshot(report, parts, sev_counts, raw_sev_counts)
    severity = "warning" if sev_counts.get("critical", 0) else "ok"
    if severity == "ok" and sev_counts.get("review", 0):
        severity = "notice"
    return _finding(
        "architecture_drift",
        severity,
        _summary(prior, parts, sev_counts, snapshot),
        snapshot=snapshot,
    )
