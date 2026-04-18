#!/usr/bin/env python3
"""
Render a steward-first BTSP cohort scoreboard from the live proposal ledger.
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from btsp_runtime_analysis import (
    DEFAULT_EPISODE_BANK,
    DEFAULT_PROPOSAL_LEDGER,
    derive_signal_fingerprint,
    format_pct,
    iso_now,
    load_runtime,
    parse_signal_fingerprint,
    proposal_outcomes,
    response_label,
    top_counter_rows,
    write_json,
    write_report,
)


DEFAULT_OUTPUT_DIR = Path(
    "/Users/v/other/astrid/capsules/consciousness-bridge/workspace/diagnostics/btsp_cohort_scoreboard"
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--proposal-ledger", type=Path, default=DEFAULT_PROPOSAL_LEDGER)
    parser.add_argument("--episode-bank", type=Path, default=DEFAULT_EPISODE_BANK)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    return parser.parse_args()


def summarize_cohorts(
    proposals: list[dict[str, Any]], episodes: list[dict[str, Any]]
) -> dict[str, Any]:
    cohorts: dict[str, dict[str, Any]] = {}
    global_exact = Counter()
    global_interpreted = Counter()

    for proposal in proposals:
        fingerprint = derive_signal_fingerprint(proposal)
        components = parse_signal_fingerprint(fingerprint)
        cohort = cohorts.setdefault(
            fingerprint,
            {
                "fingerprint": fingerprint,
                "components": components,
                "proposals": 0,
                "active": 0,
                "resolved": 0,
                "integrated": 0,
                "exact_adoptions": 0,
                "choice_interpretations": 0,
                "outcomes": 0,
                "reconcentrating": 0,
                "recovery": 0,
                "positive_target": 0,
                "exact_response_counts": Counter(),
                "interpreted_choice_counts": Counter(),
                "recent_proposal_ids": [],
            },
        )
        cohort["proposals"] += 1
        reply_state = str(proposal.get("reply_state", ""))
        if reply_state in {"unseen", "witnessed", "answered", "adopted"}:
            cohort["active"] += 1
        else:
            cohort["resolved"] += 1
        if reply_state == "integrated":
            cohort["integrated"] += 1

        exact_adoptions = [
            adoption
            for adoption in proposal.get("exact_adoptions", [])
            if isinstance(adoption, dict)
        ]
        cohort["exact_adoptions"] += len(exact_adoptions)
        for adoption in exact_adoptions:
            response_id = str(adoption.get("response_id", ""))
            if not response_id:
                continue
            cohort["exact_response_counts"][response_id] += 1
            global_exact[response_id] += 1

        interpretations = [
            interpretation
            for interpretation in proposal.get("choice_interpretations", [])
            if isinstance(interpretation, dict)
            and str(interpretation.get("relation_to_proposal", "")) != "exact_nominated"
        ]
        cohort["choice_interpretations"] += len(interpretations)
        for interpretation in interpretations:
            normalized_choice = str(interpretation.get("normalized_choice", ""))
            if not normalized_choice:
                continue
            cohort["interpreted_choice_counts"][normalized_choice] += 1
            global_interpreted[normalized_choice] += 1

        outcomes = proposal_outcomes(proposal, episodes)
        cohort["outcomes"] += len(outcomes)
        for outcome in outcomes:
            if str(outcome.get("opening_vs_reconcentration", "")) == "reconcentrating":
                cohort["reconcentrating"] += 1
            if str(outcome.get("distress_or_recovery", "")) == "recovery":
                cohort["recovery"] += 1
            if str(outcome.get("target_nearness", "")) == "positive":
                cohort["positive_target"] += 1
        cohort["recent_proposal_ids"].append(str(proposal.get("proposal_id", "")))

    cohort_rows = []
    for cohort in cohorts.values():
        exact_counter: Counter[str] = cohort.pop("exact_response_counts")
        interpreted_counter: Counter[str] = cohort.pop("interpreted_choice_counts")
        cohort["exact_response_counts"] = dict(exact_counter)
        cohort["interpreted_choice_counts"] = dict(interpreted_counter)
        cohort["top_exact_responses"] = [
            {"response_id": response_id, "label": response_label(response_id), "count": count}
            for response_id, count in exact_counter.most_common(4)
        ]
        cohort["top_interpreted_choices"] = top_counter_rows(interpreted_counter, limit=4)
        cohort["reconcentrating_rate"] = format_pct(
            cohort["reconcentrating"], cohort["outcomes"]
        )
        cohort["recovery_rate"] = format_pct(cohort["recovery"], cohort["outcomes"])
        cohort["positive_target_rate"] = format_pct(
            cohort["positive_target"], cohort["outcomes"]
        )
        cohort["recent_proposal_ids"] = cohort["recent_proposal_ids"][-5:]
        cohort_rows.append(cohort)

    cohort_rows.sort(
        key=lambda row: (
            -int(row["proposals"]),
            -int(row["outcomes"]),
            row["fingerprint"],
        )
    )
    return {
        "generated_at": iso_now(),
        "proposal_count": len(proposals),
        "resolved_proposals": sum(
            1
            for proposal in proposals
            if str(proposal.get("reply_state", ""))
            not in {"unseen", "witnessed", "answered", "adopted"}
        ),
        "active_proposals": sum(
            1
            for proposal in proposals
            if str(proposal.get("reply_state", ""))
            in {"unseen", "witnessed", "answered", "adopted"}
        ),
        "top_exact_responses": [
            {"response_id": response_id, "label": response_label(response_id), "count": count}
            for response_id, count in global_exact.most_common(8)
        ],
        "top_interpreted_choices": top_counter_rows(global_interpreted, limit=8),
        "cohorts": cohort_rows,
    }


def build_report(summary: dict[str, Any]) -> list[str]:
    cohorts = summary["cohorts"]
    most_reconcentrating = [
        cohort
        for cohort in cohorts
        if int(cohort["outcomes"]) >= 3
    ]
    most_reconcentrating.sort(
        key=lambda row: (
            -int(row["reconcentrating"]),
            -int(row["outcomes"]),
        )
    )
    best_recovery = [
        cohort for cohort in cohorts if int(cohort["outcomes"]) >= 3
    ]
    best_recovery.sort(
        key=lambda row: (
            -int(row["recovery"]),
            int(row["reconcentrating"]),
            -int(row["outcomes"]),
        )
    )

    lines = [
        "# BTSP Cohort Scoreboard",
        "",
        f"- Generated: `{summary['generated_at']}`",
        f"- Proposals inspected: `{summary['proposal_count']}`",
        f"- Active proposals: `{summary['active_proposals']}`",
        f"- Resolved proposals: `{summary['resolved_proposals']}`",
        "",
        "## Top Exact Responses",
    ]
    for row in summary["top_exact_responses"]:
        lines.append(f"- `{row['label']}`: `{row['count']}` exact adoptions")
    lines.extend(["", "## Top Interpreted Choices"])
    for row in summary["top_interpreted_choices"]:
        lines.append(f"- `{row['name']}`: `{row['count']}` interpreted adjacent choices")

    lines.extend(["", "## Largest Cohorts"])
    for cohort in cohorts[:8]:
        components = cohort["components"]
        lines.append(
            f"- `{components['families']}` | transition `{components['transition']}` | "
            f"perturb `{components['perturb']}` | fill `{components['fill_band']}`: "
            f"`{cohort['proposals']}` proposals, `{cohort['outcomes']}` outcomes, "
            f"`{cohort['reconcentrating_rate']}` reconcentrating, "
            f"`{cohort['recovery_rate']}` recovery"
        )
        if cohort["top_exact_responses"]:
            lines.append(
                "  top exact: "
                + ", ".join(
                    f"{row['label']} ({row['count']})"
                    for row in cohort["top_exact_responses"]
                )
            )

    lines.extend(["", "## Strongest Reconcentrating Cohorts"])
    for cohort in most_reconcentrating[:6]:
        components = cohort["components"]
        lines.append(
            f"- `{components['families']}` | `{components['transition']}` | `{components['perturb']}`: "
            f"`{cohort['reconcentrating']}/{cohort['outcomes']}` reconcentrating "
            f"with `{cohort['recovery']}` recovery outcomes"
        )

    lines.extend(["", "## Best Recovery Cohorts"])
    for cohort in best_recovery[:6]:
        components = cohort["components"]
        lines.append(
            f"- `{components['families']}` | `{components['transition']}` | `{components['perturb']}`: "
            f"`{cohort['recovery']}/{cohort['outcomes']}` recovery outcomes and "
            f"`{cohort['reconcentrating']}` reconcentrating"
        )
    return lines


def main() -> None:
    args = parse_args()
    proposals, episodes = load_runtime(args.proposal_ledger, args.episode_bank)
    summary = summarize_cohorts(proposals, episodes)
    report_lines = build_report(summary)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    write_json(args.output_dir / "summary.json", summary)
    write_report(args.output_dir / "report.md", report_lines)


if __name__ == "__main__":
    main()
