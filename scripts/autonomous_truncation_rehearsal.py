#!/usr/bin/env python3
"""Rehearse priority-preserving truncation without changing Astrid runtime limits."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import re
from pathlib import Path


ASTRID_ROOT = Path(__file__).resolve().parents[1]
ASTRID_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
DEFAULT_OUTPUT_ROOT = ASTRID_WORKSPACE / "diagnostics/autonomous_truncation_rehearsals"
DEFAULT_MAX_BYTES = 500

ANCHOR_TERMS = (
    "NEXT:",
    "SHADOW_TRAJECTORY",
    "Shadow-v3",
    "settled coupling",
    "restless texture",
    "density_gradient",
    "density gradient",
    "pressure_risk",
    "distinguishability_loss",
    "lambda_4",
    "λ4",
    "tail vibrancy",
    "semantic trickle",
    "stable_core_semantic_trickle",
    "telemetry",
    "line ",
)
HIGH_PRIORITY_TERMS = (
    "NEXT:",
    "SHADOW_TRAJECTORY",
    "Shadow-v3",
    "density_gradient",
    "pressure_risk",
    "distinguishability_loss",
    "tail vibrancy",
    "semantic trickle",
    "telemetry",
)


def run_id() -> str:
    return dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")


def compact(text: str, limit: int = 260) -> str:
    clean = " ".join(text.split())
    if len(clean) <= limit:
        return clean
    return clean[: max(0, limit - 1)].rstrip() + "..."


def truncate_bytes(text: str, max_bytes: int) -> str:
    encoded = text.encode("utf-8")
    if len(encoded) <= max_bytes:
        return text
    cut = encoded[:max_bytes]
    while cut:
        try:
            return cut.decode("utf-8")
        except UnicodeDecodeError:
            cut = cut[:-1]
    return ""


def matching_terms(text: str, terms: tuple[str, ...]) -> list[str]:
    lower = text.lower()
    found: list[str] = []
    for term in terms:
        if term.lower() in lower and term not in found:
            found.append(term)
    return found


def split_units(text: str) -> list[str]:
    parts: list[str] = []
    for block in re.split(r"\n{2,}", text):
        for piece in re.split(r"(?<=[.!?])\s+|\n", block):
            clean = piece.strip()
            if clean:
                parts.append(clean)
    return parts or [text.strip()]


def unit_score(unit: str, index: int) -> int:
    score = max(0, 8 - index // 3)
    lower = unit.lower()
    for term in HIGH_PRIORITY_TERMS:
        if term.lower() in lower:
            score += 24
    if re.search(r"\b0\.\d{2,3}\b|\b\d{2,3}\.\d%", unit):
        score += 8
    if "source:" in lower or "lines" in lower or ".rs" in lower:
        score += 5
    return score


def priority_preserve(text: str, max_bytes: int) -> str:
    units = split_units(text)
    scored = sorted(
        enumerate(units),
        key=lambda item: (unit_score(item[1], item[0]), -item[0]),
        reverse=True,
    )
    selected: list[tuple[int, str]] = []
    used = 0
    for index, unit in scored:
        candidate = unit if unit.endswith((".", "!", "?", ":")) else f"{unit}."
        cost = len(candidate.encode("utf-8")) + (2 if selected else 0)
        if used + cost <= max_bytes:
            selected.append((index, candidate))
            used += cost
    selected.sort(key=lambda item: item[0])
    return "\n".join(unit for _index, unit in selected)


def candidate_paths(workspace: Path, limit: int) -> list[Path]:
    roots = [workspace / "introspections", workspace / "journal"]
    paths: list[Path] = []
    for root in roots:
        if root.exists():
            paths.extend(path for path in root.glob("*.txt") if path.is_file())
    paths.sort(key=lambda path: path.stat().st_mtime, reverse=True)
    selected: list[Path] = []
    for path in paths:
        text = path.read_text(encoding="utf-8", errors="replace")
        anchors = matching_terms(text, ANCHOR_TERMS)
        if anchors and any(
            marker in text.lower()
            for marker in ("truncate", "shadow_trajectory", "shadow-v3", "tail vibrancy")
        ):
            selected.append(path)
        if len(selected) >= limit:
            break
    return selected


def evaluate_text(source: str, text: str, max_bytes: int) -> dict[str, object]:
    naive = truncate_bytes(text, max_bytes)
    priority = priority_preserve(text, max_bytes)
    original_anchors = matching_terms(text, ANCHOR_TERMS)
    naive_anchors = matching_terms(naive, tuple(original_anchors))
    priority_anchors = matching_terms(priority, tuple(original_anchors))
    lost_by_naive = [term for term in original_anchors if term not in naive_anchors]
    recovered = [term for term in lost_by_naive if term in priority_anchors]
    return {
        "source": source,
        "original_bytes": len(text.encode("utf-8")),
        "max_bytes": max_bytes,
        "naive_bytes": len(naive.encode("utf-8")),
        "priority_bytes": len(priority.encode("utf-8")),
        "original_anchor_count": len(original_anchors),
        "naive_anchor_count": len(naive_anchors),
        "priority_anchor_count": len(priority_anchors),
        "lost_by_naive": lost_by_naive,
        "recovered_by_priority": recovered,
        "priority_gain": len(priority_anchors) - len(naive_anchors),
        "naive_preview": compact(naive),
        "priority_preview": compact(priority),
    }


def build_record(
    *,
    workspace: Path,
    output_root: Path,
    run: str,
    max_bytes: int,
    limit: int,
    fixture: bool,
) -> dict[str, object]:
    if fixture:
        texts = [
            (
                "fixture_autonomous_shadow",
                "Witness mode begins with texture and context. "
                "The early prose can be beautiful but expendable. "
                "SHADOW_TRAJECTORY witness-thread should survive with Shadow-v3 settled coupling. "
                "density_gradient=0.18 pressure_risk=0.19 distinguishability_loss=0.27. "
                "The lambda_4 tail vibrancy and semantic trickle are the evidence anchors. "
                "NEXT: SHADOW_TRAJECTORY witness-thread",
            )
        ]
    else:
        texts = [
            (str(path), path.read_text(encoding="utf-8", errors="replace"))
            for path in candidate_paths(workspace, limit)
        ]
    candidates = [evaluate_text(source, text, max_bytes) for source, text in texts]
    recovered_count = sum(1 for item in candidates if item["recovered_by_priority"])
    naive_loss_count = sum(1 for item in candidates if item["lost_by_naive"])
    if recovered_count:
        status = "priority_preservation_benefit"
    elif naive_loss_count:
        status = "truncation_risk_without_recovery"
    elif candidates:
        status = "no_priority_loss_detected"
    else:
        status = "no_truncation_candidates"
    record = {
        "policy": "autonomous_truncation_rehearsal_v1",
        "authority": "diagnostic_context_not_command",
        "run_id": run,
        "generated_at": dt.datetime.now(dt.UTC).isoformat(),
        "workspace": str(workspace),
        "mode": "fixture" if fixture else "workspace",
        "max_bytes": max_bytes,
        "status": status,
        "candidate_count": len(candidates),
        "naive_anchor_loss_count": naive_loss_count,
        "priority_recovery_count": recovered_count,
        "candidates": candidates,
        "recommended_action": (
            "Use this rehearsal to decide whether a future runtime tranche should "
            "replace naive byte truncation with priority-preserving compaction. "
            "Do not raise max_bytes or change dispatch behavior from this artifact alone."
        ),
    }
    out_dir = output_root / run
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "autonomous_truncation_rehearsal.json").write_text(
        json.dumps(record, indent=2, sort_keys=True),
        encoding="utf-8",
    )
    (out_dir / "autonomous_truncation_rehearsal.md").write_text(
        render_markdown(record),
        encoding="utf-8",
    )
    return record


def render_markdown(record: dict[str, object]) -> str:
    lines = [
        "# Autonomous Truncation Rehearsal",
        "",
        f"- run_id: `{record['run_id']}`",
        f"- status: `{record['status']}`",
        f"- mode: `{record['mode']}`",
        f"- max_bytes: `{record['max_bytes']}`",
        f"- authority: `{record['authority']}`",
        "",
        "## Candidates",
        "",
    ]
    for item in record.get("candidates") or []:
        if not isinstance(item, dict):
            continue
        lines.append(
            f"- `{item.get('source')}` original={item.get('original_bytes')}B; "
            f"naive_anchors={item.get('naive_anchor_count')}/{item.get('original_anchor_count')}; "
            f"priority_anchors={item.get('priority_anchor_count')}/{item.get('original_anchor_count')}; "
            f"recovered={item.get('recovered_by_priority') or []}"
        )
    lines.extend(
        [
            "",
            "## Boundary",
            "",
            "Diagnostic rehearsal only. It did not replace truncate_str, raise byte limits, write journals, dispatch NEXT actions, tune controllers, or mutate peers.",
        ]
    )
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=ASTRID_WORKSPACE)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--run-id", default=run_id())
    parser.add_argument("--max-bytes", type=int, default=DEFAULT_MAX_BYTES)
    parser.add_argument("--limit", type=int, default=12)
    parser.add_argument("--fixture", action="store_true")
    args = parser.parse_args()
    record = build_record(
        workspace=args.workspace,
        output_root=args.output_root,
        run=args.run_id,
        max_bytes=args.max_bytes,
        limit=args.limit,
        fixture=args.fixture,
    )
    print(f"status={record['status']}")
    print(f"wrote {args.output_root / args.run_id / 'autonomous_truncation_rehearsal.json'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
