#!/usr/bin/env python3
"""
Render a BTSP choice-to-outcome latency view from the live proposal ledger.
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from btsp_runtime_analysis import (
    DEFAULT_EPISODE_BANK,
    DEFAULT_PROPOSAL_LEDGER,
    OWNER_ASTRID,
    OWNER_MINIME,
    first_future_outcome,
    format_pct,
    is_real_runtime_proposal,
    is_resolved_proposal,
    iso_now,
    latency_bucket,
    latency_minutes,
    load_runtime,
    owner_choice_interpretations,
    owner_exact_adoptions,
    response_label,
    write_json,
    write_report,
)


DEFAULT_OUTPUT_DIR = Path(
    "/Users/v/other/astrid/capsules/consciousness-bridge/workspace/diagnostics/btsp_choice_outcome_latency"
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--proposal-ledger", type=Path, default=DEFAULT_PROPOSAL_LEDGER)
    parser.add_argument("--episode-bank", type=Path, default=DEFAULT_EPISODE_BANK)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    return parser.parse_args()


def collect_rows(
    proposals: list[dict[str, Any]], episodes: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for proposal in proposals:
        if not is_real_runtime_proposal(proposal) or not is_resolved_proposal(proposal):
            continue
        proposal_id = str(proposal.get("proposal_id", ""))
        for owner in (OWNER_MINIME, OWNER_ASTRID):
            for adoption in owner_exact_adoptions(proposal, owner):
                adopted_at = int(adoption.get("adopted_at_unix_s", 0) or 0)
                response_id = str(adoption.get("response_id", ""))
                outcome = first_future_outcome(
                    proposal,
                    episodes,
                    after_unix_s=adopted_at,
                    owner=owner,
                    response_id=response_id,
                )
                rows.append(
                    {
                        "proposal_id": proposal_id,
                        "owner": owner,
                        "choice_kind": "exact",
                        "choice_key": response_id,
                        "choice_label": response_label(response_id),
                        "category": "exact_nominated",
                        "relation_to_proposal": "exact_nominated",
                        "acted_at_unix_s": adopted_at,
                        "latency_minutes": latency_minutes(adopted_at, outcome),
                        "latency_bucket": latency_bucket(latency_minutes(adopted_at, outcome)),
                        "outcome": outcome,
                    }
                )
            for interpretation in owner_choice_interpretations(proposal, owner):
                if str(interpretation.get("relation_to_proposal", "")) == "exact_nominated":
                    continue
                interpreted_at = int(interpretation.get("interpreted_at_unix_s", 0) or 0)
                outcome = first_future_outcome(
                    proposal,
                    episodes,
                    after_unix_s=interpreted_at,
                )
                choice_key = str(interpretation.get("normalized_choice", ""))
                rows.append(
                    {
                        "proposal_id": proposal_id,
                        "owner": owner,
                        "choice_kind": "adjacent",
                        "choice_key": choice_key,
                        "choice_label": choice_key,
                        "category": str(interpretation.get("category", "unknown")),
                        "relation_to_proposal": str(
                            interpretation.get("relation_to_proposal", "")
                        ),
                        "acted_at_unix_s": interpreted_at,
                        "latency_minutes": latency_minutes(interpreted_at, outcome),
                        "latency_bucket": latency_bucket(latency_minutes(interpreted_at, outcome)),
                        "outcome": outcome,
                    }
                )
    return rows


def summarize_rows(rows: list[dict[str, Any]]) -> dict[str, Any]:
    grouped: dict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        grouped[(row["owner"], row["choice_kind"], row["choice_key"])].append(row)

    summaries: list[dict[str, Any]] = []
    for (owner, choice_kind, choice_key), bucket_rows in grouped.items():
        latency_counter = Counter(row["latency_bucket"] for row in bucket_rows)
        reconcentrating = sum(
            1
            for row in bucket_rows
            if row["outcome"]
            and str(row["outcome"].get("opening_vs_reconcentration", ""))
            == "reconcentrating"
        )
        recovery = sum(
            1
            for row in bucket_rows
            if row["outcome"]
            and str(row["outcome"].get("distress_or_recovery", "")) == "recovery"
        )
        observed = sum(1 for row in bucket_rows if row["outcome"] is not None)
        latencies = [
            row["latency_minutes"]
            for row in bucket_rows
            if row["latency_minutes"] is not None
        ]
        summaries.append(
            {
                "owner": owner,
                "choice_kind": choice_kind,
                "choice_key": choice_key,
                "choice_label": bucket_rows[0]["choice_label"],
                "category": bucket_rows[0]["category"],
                "relation_to_proposal": bucket_rows[0]["relation_to_proposal"],
                "observations": len(bucket_rows),
                "observed_outcomes": observed,
                "median_latency_minutes": round(
                    sorted(latencies)[len(latencies) // 2], 2
                )
                if latencies
                else None,
                "latency_buckets": dict(latency_counter),
                "reconcentrating": reconcentrating,
                "recovery": recovery,
                "reconcentrating_rate": format_pct(reconcentrating, observed),
                "recovery_rate": format_pct(recovery, observed),
            }
        )
    summaries.sort(
        key=lambda row: (
            row["owner"],
            row["choice_kind"],
            -int(row["observations"]),
            row["choice_key"],
        )
    )
    exact_choice_summaries = sorted(
        [row for row in summaries if row["choice_kind"] == "exact"],
        key=lambda row: (
            row["owner"],
            -int(row["observations"]),
            row["choice_key"],
        ),
    )
    adjacent_choice_summaries = sorted(
        [row for row in summaries if row["choice_kind"] == "adjacent"],
        key=lambda row: (
            row["owner"],
            -int(row["observations"]),
            row["choice_key"],
        ),
    )
    return {
        "generated_at": iso_now(),
        "row_count": len(rows),
        "exact_rows": sum(1 for row in rows if row["choice_kind"] == "exact"),
        "adjacent_rows": sum(1 for row in rows if row["choice_kind"] == "adjacent"),
        "exact_choice_summaries": exact_choice_summaries,
        "adjacent_choice_summaries": adjacent_choice_summaries,
        "summaries": summaries,
        "rows": rows,
    }


def build_report(summary: dict[str, Any]) -> list[str]:
    exact = list(summary["exact_choice_summaries"])
    adjacent = list(summary["adjacent_choice_summaries"])
    lines = [
        "# BTSP Choice-to-Outcome Latency",
        "",
        f"- Generated: `{summary['generated_at']}`",
        f"- Rows inspected: `{summary['row_count']}`",
        f"- Exact rows: `{summary['exact_rows']}`",
        f"- Adjacent rows: `{summary['adjacent_rows']}`",
        "",
        "## Exact Responses",
    ]
    for row in exact[:12]:
        lines.append(
            f"- `{row['owner']}` `{row['choice_label']}`: `{row['observations']}` observations, "
            f"`{row['median_latency_minutes']}` median minutes, "
            f"`{row['recovery_rate']}` recovery, `{row['reconcentrating_rate']}` reconcentrating, "
            f"buckets `{row['latency_buckets']}`"
        )
    lines.extend(["", "## Adjacent / Interpreted Choices"])
    for row in adjacent[:16]:
        lines.append(
            f"- `{row['owner']}` `{row['choice_label']}` ({row['category']}, {row['relation_to_proposal']}): "
            f"`{row['observations']}` observations, `{row['median_latency_minutes']}` median minutes, "
            f"`{row['recovery_rate']}` recovery, `{row['reconcentrating_rate']}` reconcentrating, "
            f"buckets `{row['latency_buckets']}`"
        )
    return lines


def main() -> None:
    args = parse_args()
    proposals, episodes = load_runtime(args.proposal_ledger, args.episode_bank)
    rows = collect_rows(proposals, episodes)
    summary = summarize_rows(rows)
    report_lines = build_report(summary)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    write_json(args.output_dir / "summary.json", summary)
    write_report(args.output_dir / "report.md", report_lines)


if __name__ == "__main__":
    main()
