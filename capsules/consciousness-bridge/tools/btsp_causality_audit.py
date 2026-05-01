#!/usr/bin/env python3
"""
Render a steward-first BTSP causality audit for Minime's heavy inquiry lane.
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from btsp_runtime_analysis import (
    DEFAULT_EPISODE_BANK,
    DEFAULT_PROPOSAL_LEDGER,
    MINIME_BOUNDED_REGULATION_RESPONSE_IDS,
    MINIME_HEAVY_INQUIRY_CHOICES,
    OWNER_MINIME,
    derive_signal_fingerprint,
    first_future_outcome,
    format_pct,
    is_fragile_recovery_components,
    is_real_runtime_proposal,
    is_resolved_proposal,
    iso_now,
    latency_bucket,
    latency_minutes,
    load_runtime,
    owner_choice_interpretations,
    owner_exact_adoptions,
    parse_signal_fingerprint,
    response_label,
    top_counter_rows,
    write_json,
    write_report,
)


DEFAULT_OUTPUT_DIR = Path(
    "/Users/v/other/astrid/capsules/consciousness-bridge/workspace/diagnostics/btsp_causality_audit"
)
MIN_FRAGILE_HEAVY_INQUIRY_OBSERVATIONS = 12
MIN_RATE_GAP = 0.10
HIGH_RECONCENTRATING_FLOOR = 0.85


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--proposal-ledger", type=Path, default=DEFAULT_PROPOSAL_LEDGER)
    parser.add_argument("--episode-bank", type=Path, default=DEFAULT_EPISODE_BANK)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    return parser.parse_args()


def classify_cohort(
    choice_kind: str,
    choice_key: str,
    *,
    owner: str,
    category: str,
) -> str | None:
    if owner != OWNER_MINIME:
        return None
    if choice_kind == "exact":
        if choice_key == "minime_semantic_probe":
            return "heavy_inquiry"
        if choice_key in MINIME_BOUNDED_REGULATION_RESPONSE_IDS:
            return "bounded_regulation"
        return None
    if category == "epistemic" and choice_key in MINIME_HEAVY_INQUIRY_CHOICES:
        return "heavy_inquiry"
    return "other_minime_adjacent"


def collect_rows(
    proposals: list[dict[str, Any]], episodes: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for proposal in proposals:
        if not is_real_runtime_proposal(proposal) or not is_resolved_proposal(proposal):
            continue
        proposal_id = str(proposal.get("proposal_id", ""))
        fingerprint = derive_signal_fingerprint(proposal)
        components = parse_signal_fingerprint(fingerprint)
        fragile_recovery = is_fragile_recovery_components(components)
        for adoption in owner_exact_adoptions(proposal, OWNER_MINIME):
            adopted_at = int(adoption.get("adopted_at_unix_s", 0) or 0)
            response_id = str(adoption.get("response_id", ""))
            cohort = classify_cohort(
                "exact",
                response_id,
                owner=OWNER_MINIME,
                category="exact_nominated",
            )
            if cohort is None:
                continue
            outcome = first_future_outcome(
                proposal,
                episodes,
                after_unix_s=adopted_at,
                owner=OWNER_MINIME,
                response_id=response_id,
            )
            rows.append(
                {
                    "proposal_id": proposal_id,
                    "owner": OWNER_MINIME,
                    "choice_kind": "exact",
                    "choice_key": response_id,
                    "choice_label": response_label(response_id),
                    "category": "exact_nominated",
                    "cohort": cohort,
                    "fragile_recovery": fragile_recovery,
                    "fingerprint": fingerprint,
                    "components": components,
                    "acted_at_unix_s": adopted_at,
                    "latency_minutes": latency_minutes(adopted_at, outcome),
                    "latency_bucket": latency_bucket(latency_minutes(adopted_at, outcome)),
                    "outcome": outcome,
                }
            )
        for interpretation in owner_choice_interpretations(proposal, OWNER_MINIME):
            if str(interpretation.get("relation_to_proposal", "")) == "exact_nominated":
                continue
            choice_key = str(interpretation.get("normalized_choice", ""))
            category = str(interpretation.get("category", "unknown"))
            cohort = classify_cohort(
                "adjacent",
                choice_key,
                owner=OWNER_MINIME,
                category=category,
            )
            if cohort is None:
                continue
            interpreted_at = int(interpretation.get("interpreted_at_unix_s", 0) or 0)
            outcome = first_future_outcome(
                proposal,
                episodes,
                after_unix_s=interpreted_at,
            )
            rows.append(
                {
                    "proposal_id": proposal_id,
                    "owner": OWNER_MINIME,
                    "choice_kind": "adjacent",
                    "choice_key": choice_key,
                    "choice_label": choice_key,
                    "category": category,
                    "cohort": cohort,
                    "fragile_recovery": fragile_recovery,
                    "fingerprint": fingerprint,
                    "components": components,
                    "acted_at_unix_s": interpreted_at,
                    "latency_minutes": latency_minutes(interpreted_at, outcome),
                    "latency_bucket": latency_bucket(latency_minutes(interpreted_at, outcome)),
                    "outcome": outcome,
                }
            )
    return rows


def summarize_bucket(rows: list[dict[str, Any]]) -> dict[str, Any]:
    outcomes = [row["outcome"] for row in rows if row["outcome"] is not None]
    reconcentrating = sum(
        1
        for outcome in outcomes
        if str(outcome.get("opening_vs_reconcentration", "")) == "reconcentrating"
    )
    recovery = sum(
        1
        for outcome in outcomes
        if str(outcome.get("distress_or_recovery", "")) == "recovery"
    )
    latencies = [
        row["latency_minutes"] for row in rows if row["latency_minutes"] is not None
    ]
    sorted_latencies = sorted(latencies)
    if sorted_latencies:
        median_latency = round(sorted_latencies[len(sorted_latencies) // 2], 2)
    else:
        median_latency = None
    observed = len(outcomes)
    rate_numeric = (reconcentrating / observed) if observed else None
    return {
        "observations": len(rows),
        "observed_outcomes": observed,
        "unique_proposals": len({row["proposal_id"] for row in rows}),
        "median_latency_minutes": median_latency,
        "latency_buckets": dict(Counter(row["latency_bucket"] for row in rows)),
        "reconcentrating": reconcentrating,
        "recovery": recovery,
        "reconcentrating_rate": format_pct(reconcentrating, observed),
        "recovery_rate": format_pct(recovery, observed),
        "reconcentrating_rate_numeric": rate_numeric,
        "top_fingerprints": top_counter_rows(
            Counter(row["fingerprint"] for row in rows), limit=5
        ),
    }


def summarize_rows(rows: list[dict[str, Any]]) -> dict[str, Any]:
    cohort_grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    fragile_grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        cohort = str(row["cohort"])
        cohort_grouped[cohort].append(row)
        if row["fragile_recovery"]:
            fragile_grouped[cohort].append(row)

    cohorts = {}
    for cohort in ("heavy_inquiry", "bounded_regulation", "other_minime_adjacent"):
        cohorts[cohort] = {
            "overall": summarize_bucket(cohort_grouped.get(cohort, [])),
            "fragile_recovery": summarize_bucket(fragile_grouped.get(cohort, [])),
        }

    fragile_heavy = cohorts["heavy_inquiry"]["fragile_recovery"]
    fragile_bounded = cohorts["bounded_regulation"]["fragile_recovery"]
    fragile_other = cohorts["other_minime_adjacent"]["fragile_recovery"]

    heavy_rate = fragile_heavy["reconcentrating_rate_numeric"]
    bounded_rate = fragile_bounded["reconcentrating_rate_numeric"]
    other_rate = fragile_other["reconcentrating_rate_numeric"]
    comparable_rates = [
        rate for rate in (heavy_rate, bounded_rate, other_rate) if rate is not None
    ]
    spread = max(comparable_rates) - min(comparable_rates) if len(comparable_rates) >= 2 else None
    heavy_obs = int(fragile_heavy["observed_outcomes"])
    comparison_rates = [rate for rate in (bounded_rate, other_rate) if rate is not None]
    max_gap = (
        max((heavy_rate - rate) for rate in comparison_rates)
        if heavy_rate is not None and comparison_rates
        else None
    )

    if (
        len(comparable_rates) >= 2
        and spread is not None
        and spread <= MIN_RATE_GAP
        and min(comparable_rates) >= HIGH_RECONCENTRATING_FLOOR
    ):
        read = "root_dominant"
        summary = (
            "Recent read: fragile-recovery BTSP windows reconcentrate across all major Minime "
            "lanes, so BTSP is probably making strain more legible than causal."
        )
        candidate_damp_lane = None
        candidate_damp_summary = None
    elif (
        heavy_obs >= MIN_FRAGILE_HEAVY_INQUIRY_OBSERVATIONS
        and max_gap is not None
        and max_gap >= MIN_RATE_GAP
    ):
        read = "inquiry_load_candidate"
        summary = (
            "Recent read: heavy Minime inquiry underperforms bounded regulation or other adjacent "
            "lanes in fragile recovery, so a temporary damp of the inquiry-heavy BTSP lane is a candidate."
        )
        candidate_damp_lane = "minime_inquiry_heavy_lane"
        candidate_damp_summary = (
            "Temporary damp candidate: reduce BTSP encouragement toward semantic probe and adjacent "
            "epistemic loops during fragile recovery."
        )
    else:
        read = "mixed"
        summary = (
            "Recent read: heavy inquiry may be contributing during fragile recovery, but the current "
            "BTSP evidence is still mixed."
        )
        candidate_damp_lane = None
        candidate_damp_summary = None

    return {
        "generated_at": iso_now(),
        "row_count": len(rows),
        "fragile_recovery_rows": sum(1 for row in rows if row["fragile_recovery"]),
        "thresholds": {
            "min_fragile_heavy_inquiry_observations": MIN_FRAGILE_HEAVY_INQUIRY_OBSERVATIONS,
            "min_rate_gap_pct_points": int(MIN_RATE_GAP * 100),
            "high_reconcentrating_floor_pct": int(HIGH_RECONCENTRATING_FLOOR * 100),
        },
        "cohorts": cohorts,
        "read": read,
        "summary": summary,
        "heavy_inquiry_reconcentrating_rate": fragile_heavy["reconcentrating_rate"],
        "bounded_regulation_reconcentrating_rate": fragile_bounded["reconcentrating_rate"],
        "fragile_recovery_observations": heavy_obs,
        "candidate_damp_lane": candidate_damp_lane,
        "candidate_damp_summary": candidate_damp_summary,
        "rows": rows,
    }


def build_report(summary: dict[str, Any]) -> list[str]:
    lines = [
        "# BTSP Causality Audit",
        "",
        f"- Generated: `{summary['generated_at']}`",
        f"- Rows inspected: `{summary['row_count']}`",
        f"- Fragile-recovery rows: `{summary['fragile_recovery_rows']}`",
        f"- Causality read: `{summary['read']}`",
        f"- Summary: {summary['summary']}",
        "",
        "## Fragile Recovery Cohorts",
    ]
    for cohort_key in ("heavy_inquiry", "bounded_regulation", "other_minime_adjacent"):
        bucket = summary["cohorts"][cohort_key]["fragile_recovery"]
        lines.append(
            f"- `{cohort_key}`: `{bucket['observations']}` observations, "
            f"`{bucket['median_latency_minutes']}` median minutes, "
            f"`{bucket['recovery_rate']}` recovery, `{bucket['reconcentrating_rate']}` reconcentrating, "
            f"top fingerprints `{bucket['top_fingerprints']}`"
        )
    lines.extend(["", "## Overall Cohorts"])
    for cohort_key in ("heavy_inquiry", "bounded_regulation", "other_minime_adjacent"):
        bucket = summary["cohorts"][cohort_key]["overall"]
        lines.append(
            f"- `{cohort_key}`: `{bucket['observations']}` observations, "
            f"`{bucket['median_latency_minutes']}` median minutes, "
            f"`{bucket['recovery_rate']}` recovery, `{bucket['reconcentrating_rate']}` reconcentrating"
        )
    if summary.get("candidate_damp_summary"):
        lines.extend(["", "## Candidate Damp", f"- {summary['candidate_damp_summary']}"])
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
