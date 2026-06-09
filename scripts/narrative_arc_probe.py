#!/usr/bin/env python3
"""Write a probe-only report for Astrid's 4D narrative-arc projection.

The bridge currently keeps `NARRATIVE_ARC_DIM=4` while the embedding projection
surface is 8D. This diagnostic documents whether synthetic shifts in projected
dims 4-7 are invisible to the current arc calculation before any dimension
expansion is considered.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import math
from pathlib import Path


ASTRID_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT_ROOT = (
    ASTRID_ROOT
    / "capsules/spectral-bridge/workspace/diagnostics/narrative_arc_probe"
)
EMBEDDING_PROJECT_DIM = 8
NARRATIVE_ARC_DIM = 4


def run_id() -> str:
    return dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")


def tanh(value: float) -> float:
    return math.tanh(value)


def compute_current_arc(first: list[float], second: list[float]) -> list[float]:
    return [
        tanh(3.0 * (second[idx] - first[idx]))
        for idx in range(NARRATIVE_ARC_DIM)
    ]


def rms(values: list[float]) -> float:
    if not values:
        return 0.0
    return math.sqrt(sum(value * value for value in values) / len(values))


def build_report() -> dict[str, object]:
    cases = []
    synthetic_cases = [
        {
            "case": "tail_dims_only",
            "first": [0.0] * EMBEDDING_PROJECT_DIM,
            "second": [0.0, 0.0, 0.0, 0.0, 0.24, -0.18, 0.12, -0.30],
        },
        {
            "case": "head_and_tail_dims",
            "first": [0.0] * EMBEDDING_PROJECT_DIM,
            "second": [0.12, -0.08, 0.05, 0.10, 0.24, -0.18, 0.12, -0.30],
        },
    ]
    for item in synthetic_cases:
        first = list(item["first"])
        second = list(item["second"])
        delta = [second[idx] - first[idx] for idx in range(EMBEDDING_PROJECT_DIM)]
        arc = compute_current_arc(first, second)
        lost_tail = delta[NARRATIVE_ARC_DIM:]
        cases.append(
            {
                "case": item["case"],
                "current_arc": arc,
                "captured_arc_rms": round(rms(arc), 6),
                "lost_dims_4_7_rms": round(rms(lost_tail), 6),
                "lost_dims_4_7_max_abs": round(max((abs(v) for v in lost_tail), default=0.0), 6),
                "captured_by_current_4d_arc": bool(rms(arc) > 0.0),
                "lost_in_dims_4_7": bool(rms(lost_tail) > 0.0),
            }
        )
    return {
        "generated_at": dt.datetime.now(dt.UTC).isoformat(),
        "policy": "narrative_arc_probe_v1",
        "embedding_project_dim": EMBEDDING_PROJECT_DIM,
        "narrative_arc_dim": NARRATIVE_ARC_DIM,
        "production_change": "none",
        "cases": cases,
        "recommendation": (
            "Keep NARRATIVE_ARC_DIM=4 for now. Consider an 8D expansion only "
            "after live entries show recurring useful signal in projected dims 4-7."
        ),
    }


def render_markdown(report: dict[str, object]) -> str:
    lines = [
        "# Narrative Arc Probe",
        "",
        f"- generated_at: `{report['generated_at']}`",
        f"- current narrative arc dims: `{report['narrative_arc_dim']}`",
        f"- projected embedding dims: `{report['embedding_project_dim']}`",
        f"- production change: `{report['production_change']}`",
        "",
        "## Cases",
        "",
    ]
    for case in report["cases"]:  # type: ignore[index]
        lines.append(
            f"- {case['case']}: captured_rms={case['captured_arc_rms']}, "
            f"lost_dims_4_7_rms={case['lost_dims_4_7_rms']}, "
            f"lost_dims_4_7_max_abs={case['lost_dims_4_7_max_abs']}"
        )
    lines.extend(["", "## Recommendation", "", str(report["recommendation"]), ""])
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--print-summary", action="store_true")
    args = parser.parse_args()

    report = build_report()
    target = args.output_root / (args.run_id or run_id())
    target.mkdir(parents=True, exist_ok=True)
    json_path = target / "report.json"
    md_path = target / "report.md"
    json_path.write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")
    md_path.write_text(render_markdown(report), encoding="utf-8")
    print(f"narrative arc probe: {md_path}")
    if args.print_summary:
        print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
