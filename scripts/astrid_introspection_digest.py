#!/usr/bin/env python3
"""Summarize Astrid autonomous introspection pressure and latency signals."""

from __future__ import annotations

import argparse
import json
import re
from collections import Counter
from pathlib import Path
from typing import Any

ASTRID_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
DEFAULT_OUTPUT_DIR = DEFAULT_WORKSPACE / "diagnostics/introspection_feedback_digest"
INTROSPECTION_RE = re.compile(r"controller_astrid:autonomous_(\d+)\.json$")
SCHEMA_VERSION = 1
SLOW_REWRITE_SECONDS = 120.0


def read_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text())
    except Exception:
        return {}
    return payload if isinstance(payload, dict) else {}


def atomic_write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.tmp")
    tmp.write_text(text)
    tmp.replace(path)


def introspection_files(workspace: Path) -> list[Path]:
    root = workspace / "introspections"
    if not root.is_dir():
        return []
    paths = [
        path
        for path in root.glob("controller_astrid:autonomous_*.json")
        if INTROSPECTION_RE.search(path.name)
    ]
    paths.sort(key=lambda path: int(INTROSPECTION_RE.search(path.name).group(1)), reverse=True)
    return paths


def _round(value: Any, places: int = 3) -> float | None:
    try:
        return round(float(value), places)
    except (TypeError, ValueError):
        return None


def _dict(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def summarize_entry(path: Path) -> dict[str, Any] | None:
    payload = read_json(path)
    if not payload:
        return None
    match = INTROSPECTION_RE.search(path.name)
    t_s = int(match.group(1)) if match else None
    observer = _dict(payload.get("observer_report"))
    condition = _dict(payload.get("condition_vector"))
    profiling = _dict(payload.get("profiling"))
    runtime = _dict(profiling.get("runtime_audit"))
    generation = _dict(runtime.get("generation"))
    rewrite_budget = _dict(profiling.get("rewrite_budget")) or _dict(
        generation.get("rewrite_budget")
    )
    budget_seconds = _round(rewrite_budget.get("budget_seconds"))
    elapsed_seconds = _round(rewrite_budget.get("elapsed_seconds"))
    elapsed_over_budget = (
        budget_seconds is not None
        and elapsed_seconds is not None
        and elapsed_seconds > budget_seconds
    )
    return {
        "path": str(path),
        "t_s": t_s,
        "controller_regime": payload.get("controller_regime"),
        "controller_reason": observer.get("controller_reason") or payload.get("controller_regime_reason"),
        "dominant_pressure": observer.get("dominant_pressure"),
        "geometry_regime": observer.get("geometry_regime"),
        "predicted_top_anchor": observer.get("predicted_top_anchor"),
        "rewrite_issue_count": observer.get("rewrite_issue_count"),
        "stability_score": _round(observer.get("stability_score")),
        "severity": _round(condition.get("severity")),
        "continuity_deficit": _round(condition.get("continuity_deficit")),
        "truncation_pressure": _round(condition.get("truncation_pressure")),
        "structure_strain": _round(condition.get("structure_strain")),
        "rewrite_seconds": _round(profiling.get("rewrite_seconds")),
        "candidate_generation_seconds": _round(profiling.get("candidate_generation_seconds")),
        "rewrite_budget_seconds": budget_seconds,
        "rewrite_elapsed_seconds": elapsed_seconds,
        "rewrite_budget_cap_applied": bool(rewrite_budget.get("cap_applied")),
        "rewrite_budget_cap_reason": rewrite_budget.get("cap_reason"),
        "rewrite_elapsed_over_budget": elapsed_over_budget,
        "first_token_seconds": _round(generation.get("first_token_seconds")),
        "total_turn_seconds": _round(generation.get("total_turn_seconds")),
    }


def build_digest(workspace: Path, *, limit: int = 12) -> dict[str, Any]:
    entries = [
        entry
        for path in introspection_files(workspace)[:limit]
        if (entry := summarize_entry(path)) is not None
    ]
    pressures = Counter(
        str(entry.get("dominant_pressure") or "unknown")
        for entry in entries
    )
    regimes = Counter(str(entry.get("controller_regime") or "unknown") for entry in entries)
    rewrite_values = [
        float(entry["rewrite_seconds"])
        for entry in entries
        if entry.get("rewrite_seconds") is not None
    ]
    total_values = [
        float(entry["total_turn_seconds"])
        for entry in entries
        if entry.get("total_turn_seconds") is not None
    ]
    candidate_values = [
        float(entry["candidate_generation_seconds"])
        for entry in entries
        if entry.get("candidate_generation_seconds") is not None
    ]
    continuity_values = [
        float(entry["continuity_deficit"])
        for entry in entries
        if entry.get("continuity_deficit") is not None
    ]
    top_pressure, top_pressure_count = pressures.most_common(1)[0] if pressures else ("unknown", 0)
    summary = {
        "entry_count": len(entries),
        "window_limit": limit,
        "dominant_pressure": top_pressure,
        "dominant_pressure_count": top_pressure_count,
        "controller_regimes": dict(regimes),
        "avg_rewrite_seconds": _round(sum(rewrite_values) / len(rewrite_values)) if rewrite_values else None,
        "max_rewrite_seconds": _round(max(rewrite_values)) if rewrite_values else None,
        "slow_rewrite_count": sum(
            1
            for value in rewrite_values
            if value >= SLOW_REWRITE_SECONDS
        ),
        "avg_candidate_generation_seconds": (
            _round(sum(candidate_values) / len(candidate_values))
            if candidate_values
            else None
        ),
        "max_candidate_generation_seconds": (
            _round(max(candidate_values))
            if candidate_values
            else None
        ),
        "rewrite_budget_cap_count": sum(
            1
            for entry in entries
            if entry.get("rewrite_budget_cap_applied") is True
        ),
        "rewrite_elapsed_over_budget_count": sum(
            1
            for entry in entries
            if entry.get("rewrite_elapsed_over_budget") is True
        ),
        "avg_total_turn_seconds": _round(sum(total_values) / len(total_values)) if total_values else None,
        "max_total_turn_seconds": _round(max(total_values)) if total_values else None,
        "avg_continuity_deficit": (
            _round(sum(continuity_values) / len(continuity_values))
            if continuity_values
            else None
        ),
    }
    actions = []
    if top_pressure == "continuity_deficit" and top_pressure_count >= max(2, len(entries) // 2):
        actions.append(
            "Investigate why autonomous introspections keep warming up with continuity_deficit dominance."
        )
    if summary["avg_rewrite_seconds"] is not None and summary["avg_rewrite_seconds"] >= SLOW_REWRITE_SECONDS:
        actions.append(
            "Profile or cap the rewrite stage before changing generation behavior."
        )
    if summary["rewrite_budget_cap_count"]:
        actions.append(
            "Review rewrite-budget caps before raising sidecar budgets; the digest is read-only."
        )
    if not actions:
        actions.append("Keep watching; no repeated pressure crossed the action threshold.")
    return {
        "schema_version": SCHEMA_VERSION,
        "source": "controller_astrid_autonomous_introspections",
        "summary": summary,
        "suggested_next": actions,
        "entries": entries,
        "authority": "diagnostic_context_not_command",
    }


def write_digest(digest: dict[str, Any], output_dir: Path) -> dict[str, str]:
    output_dir.mkdir(parents=True, exist_ok=True)
    json_path = output_dir / "latest.json"
    md_path = output_dir / "latest.md"
    artifacts = {"json": str(json_path), "markdown": str(md_path)}
    payload = digest | {"artifacts": artifacts}
    atomic_write_text(
        json_path,
        json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
    )
    atomic_write_text(md_path, render_markdown(payload))
    return artifacts


def render_markdown(digest: dict[str, Any]) -> str:
    summary = digest.get("summary") if isinstance(digest.get("summary"), dict) else {}
    lines = [
        "# Astrid Introspection Feedback Digest",
        "",
        "Read-only diagnostic context, not a command.",
        "",
        f"- Entries: {summary.get('entry_count', 0)}",
        f"- Dominant pressure: {summary.get('dominant_pressure', 'unknown')} ({summary.get('dominant_pressure_count', 0)})",
        f"- Avg continuity deficit: {summary.get('avg_continuity_deficit', 'n/a')}",
        f"- Avg rewrite seconds: {summary.get('avg_rewrite_seconds', 'n/a')}",
        f"- Max rewrite seconds: {summary.get('max_rewrite_seconds', 'n/a')}",
        f"- Slow rewrites (>= {SLOW_REWRITE_SECONDS:.0f}s): {summary.get('slow_rewrite_count', 0)}",
        f"- Avg candidate-generation seconds: {summary.get('avg_candidate_generation_seconds', 'n/a')}",
        f"- Max candidate-generation seconds: {summary.get('max_candidate_generation_seconds', 'n/a')}",
        f"- Rewrite budget caps: {summary.get('rewrite_budget_cap_count', 0)}",
        f"- Rewrite elapsed over budget: {summary.get('rewrite_elapsed_over_budget_count', 0)}",
        f"- Avg total turn seconds: {summary.get('avg_total_turn_seconds', 'n/a')}",
        "",
        "## Suggested Next",
        "",
    ]
    lines.extend(f"- {item}" for item in digest.get("suggested_next") or [])
    lines.extend(["", "## Recent Entries", ""])
    for entry in digest.get("entries") or []:
        lines.append(
            "- "
            f"{entry.get('t_s')}: pressure={entry.get('dominant_pressure')} "
            f"rewrite={entry.get('rewrite_seconds')}s "
            f"candidate={entry.get('candidate_generation_seconds')}s "
            f"budget={entry.get('rewrite_elapsed_seconds')}/{entry.get('rewrite_budget_seconds')}s "
            f"cap={entry.get('rewrite_budget_cap_reason') or entry.get('rewrite_budget_cap_applied')} "
            f"total={entry.get('total_turn_seconds')}s "
            f"anchor={entry.get('predicted_top_anchor')}"
        )
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description="Build Astrid introspection feedback digest")
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--limit", type=int, default=12)
    parser.add_argument("--no-write", action="store_true")
    args = parser.parse_args()

    digest = build_digest(args.workspace, limit=max(args.limit, 1))
    if not args.no_write:
        paths = write_digest(digest, args.output_dir)
        digest["artifacts"] = paths
    print(render_markdown(digest), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
