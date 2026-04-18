#!/usr/bin/env python3
"""
Render a BTSP Astrid indirect-uptake board from the live proposal ledger.
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from btsp_runtime_analysis import (
    DEFAULT_EPISODE_BANK,
    DEFAULT_PROPOSAL_LEDGER,
    DEFAULT_SIGNAL_EVENTS,
    OWNER_ASTRID,
    derive_signal_fingerprint,
    first_future_outcome,
    format_pct,
    iso_now,
    is_real_runtime_proposal,
    is_resolved_proposal,
    latency_bucket,
    latency_minutes,
    load_signal_events,
    load_runtime,
    owner_choice_interpretations,
    owner_exact_adoptions,
    parse_signal_fingerprint,
    proposal_seen_by_owner,
    top_counter_rows,
    write_json,
    write_report,
)


DEFAULT_OUTPUT_DIR = Path(
    "/Users/v/other/astrid/capsules/consciousness-bridge/workspace/diagnostics/btsp_astrid_indirect_uptake"
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--proposal-ledger", type=Path, default=DEFAULT_PROPOSAL_LEDGER)
    parser.add_argument("--episode-bank", type=Path, default=DEFAULT_EPISODE_BANK)
    parser.add_argument("--signal-events", type=Path, default=DEFAULT_SIGNAL_EVENTS)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    return parser.parse_args()


def collect_rows(
    proposals: list[dict[str, Any]], episodes: list[dict[str, Any]]
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    shadow_rows: list[dict[str, Any]] = []
    seen_by_astrid = 0
    exact_astrid = 0
    indirect_proposals = 0
    for proposal in proposals:
        is_real = is_real_runtime_proposal(proposal)
        is_resolved = is_resolved_proposal(proposal)
        if proposal_seen_by_owner(proposal, OWNER_ASTRID):
            seen_by_astrid += 1
        astrid_exact = owner_exact_adoptions(proposal, OWNER_ASTRID)
        if astrid_exact:
            exact_astrid += 1
        astrid_choices = [
            choice
            for choice in owner_choice_interpretations(proposal, OWNER_ASTRID)
            if str(choice.get("relation_to_proposal", "")) != "exact_nominated"
        ]
        for shadow in proposal.get("shadow_equivalences", []):
            if str(shadow.get("owner", "")) != OWNER_ASTRID:
                continue
            recorded_at = int(shadow.get("recorded_at_unix_s", 0) or 0)
            outcome = first_future_outcome(
                proposal,
                episodes,
                after_unix_s=recorded_at,
            )
            fingerprint = derive_signal_fingerprint(proposal)
            shadow_rows.append(
                {
                    "proposal_id": str(proposal.get("proposal_id", "")),
                    "choice_key": str(shadow.get("normalized_choice", "")),
                    "shadow_key": str(shadow.get("shadow_key", "")),
                    "confidence": str(shadow.get("confidence", "unknown")),
                    "preference_key": (
                        str(shadow.get("preference_key", ""))
                        if shadow.get("preference_key") is not None
                        else ""
                    ),
                    "equivalent_response_family": (
                        str(shadow.get("equivalent_response_family", ""))
                        if shadow.get("equivalent_response_family") is not None
                        else ""
                    ),
                    "acted_at_unix_s": recorded_at,
                    "latency_minutes": latency_minutes(recorded_at, outcome),
                    "latency_bucket": latency_bucket(latency_minutes(recorded_at, outcome)),
                    "outcome": outcome,
                    "fingerprint": fingerprint,
                    "components": parse_signal_fingerprint(fingerprint),
                    "proposal_is_real_runtime": is_real,
                    "proposal_is_resolved": is_resolved,
                }
            )
        if astrid_choices:
            indirect_proposals += 1
        for choice in astrid_choices:
            interpreted_at = int(choice.get("interpreted_at_unix_s", 0) or 0)
            outcome = first_future_outcome(
                proposal,
                episodes,
                after_unix_s=interpreted_at,
            )
            fingerprint = derive_signal_fingerprint(proposal)
            rows.append(
                {
                    "proposal_id": str(proposal.get("proposal_id", "")),
                    "choice_key": str(choice.get("normalized_choice", "")),
                    "category": str(choice.get("category", "unknown")),
                    "relation_to_proposal": str(choice.get("relation_to_proposal", "")),
                    "likely_intent": str(choice.get("likely_intent", "")),
                    "acted_at_unix_s": interpreted_at,
                    "latency_minutes": latency_minutes(interpreted_at, outcome),
                    "latency_bucket": latency_bucket(latency_minutes(interpreted_at, outcome)),
                    "outcome": outcome,
                    "fingerprint": fingerprint,
                    "components": parse_signal_fingerprint(fingerprint),
                    "matched_signal_families": proposal.get("matched_signal_families", []),
                    "proposal_is_real_runtime": is_real,
                    "proposal_is_resolved": is_resolved,
                }
            )
    overview = {
        "proposals_seen_by_astrid": seen_by_astrid,
        "proposals_with_astrid_exact_adoption": exact_astrid,
        "proposals_with_astrid_indirect_uptake": indirect_proposals,
    }
    return rows, shadow_rows, overview


def summarize_rows(
    rows: list[dict[str, Any]],
    shadow_rows: list[dict[str, Any]],
    overview: dict[str, Any],
    shadow_policy_windows: list[dict[str, Any]],
) -> dict[str, Any]:
    choice_grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    category_grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    fingerprint_counter = Counter()
    for row in rows:
        choice_grouped[row["choice_key"]].append(row)
        category_grouped[row["category"]].append(row)
        fingerprint_counter[row["fingerprint"]] += 1

    choice_summaries = []
    for choice_key, choice_rows in choice_grouped.items():
        outcomes = [row["outcome"] for row in choice_rows if row["outcome"] is not None]
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
            row["latency_minutes"]
            for row in choice_rows
            if row["latency_minutes"] is not None
        ]
        choice_summaries.append(
            {
                "choice_key": choice_key,
                "category": choice_rows[0]["category"],
                "relation_to_proposal": choice_rows[0]["relation_to_proposal"],
                "likely_intent": choice_rows[0]["likely_intent"],
                "observations": len(choice_rows),
                "unique_proposals": len({row["proposal_id"] for row in choice_rows}),
                "median_latency_minutes": round(
                    sorted(latencies)[len(latencies) // 2], 2
                )
                if latencies
                else None,
                "latency_buckets": dict(
                    Counter(row["latency_bucket"] for row in choice_rows)
                ),
                "reconcentrating": reconcentrating,
                "recovery": recovery,
                "reconcentrating_rate": format_pct(reconcentrating, len(outcomes)),
                "recovery_rate": format_pct(recovery, len(outcomes)),
                "top_fingerprints": top_counter_rows(
                    Counter(row["fingerprint"] for row in choice_rows), limit=3
                ),
            }
        )
    choice_summaries.sort(key=lambda row: (-int(row["observations"]), row["choice_key"]))

    category_summaries = []
    for category, category_rows in category_grouped.items():
        outcomes = [row["outcome"] for row in category_rows if row["outcome"] is not None]
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
        category_summaries.append(
            {
                "category": category,
                "observations": len(category_rows),
                "unique_proposals": len({row["proposal_id"] for row in category_rows}),
                "reconcentrating_rate": format_pct(reconcentrating, len(outcomes)),
                "recovery_rate": format_pct(recovery, len(outcomes)),
            }
        )
    category_summaries.sort(key=lambda row: (-int(row["observations"]), row["category"]))

    shadow_key_grouped: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in shadow_rows:
        shadow_key_grouped[(row["shadow_key"], row["confidence"])].append(row)
    shadow_summaries = []
    for (shadow_key, confidence), grouped_rows in shadow_key_grouped.items():
        outcomes = [row["outcome"] for row in grouped_rows if row["outcome"] is not None]
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
        shadow_summaries.append(
            {
                "shadow_key": shadow_key,
                "confidence": confidence,
                "observations": len(grouped_rows),
                "unique_proposals": len({row["proposal_id"] for row in grouped_rows}),
                "preference_keys": sorted(
                    {
                        row["preference_key"]
                        for row in grouped_rows
                        if row["preference_key"]
                    }
                ),
                "equivalent_response_families": sorted(
                    {
                        row["equivalent_response_family"]
                        for row in grouped_rows
                        if row["equivalent_response_family"]
                    }
                ),
                "reconcentrating_rate": format_pct(reconcentrating, len(outcomes)),
                "recovery_rate": format_pct(recovery, len(outcomes)),
            }
        )
    shadow_summaries.sort(
        key=lambda row: (
            row["confidence"] != "high",
            -int(row["observations"]),
            row["shadow_key"],
        )
    )

    resolved_real_shadow_rows = [
        row
        for row in shadow_rows
        if row["proposal_is_real_runtime"] and row["proposal_is_resolved"]
    ]
    resolved_real_high_confidence = [
        row for row in resolved_real_shadow_rows if row["confidence"] == "high"
    ]
    progress_grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in resolved_real_high_confidence:
        progress_grouped[row["shadow_key"]].append(row)
    shadow_progress = []
    for shadow_key, grouped_rows in progress_grouped.items():
        unique_proposals = len({row["proposal_id"] for row in grouped_rows})
        outcomes = [row["outcome"] for row in grouped_rows if row["outcome"] is not None]
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
        shadow_progress.append(
            {
                "shadow_key": shadow_key,
                "resolved_live_observations": len(grouped_rows),
                "resolved_live_unique_proposals": unique_proposals,
                "progress_current": unique_proposals,
                "progress_target": 3,
                "remaining_for_preference_memory": max(0, 3 - unique_proposals),
                "preference_keys": sorted(
                    {
                        row["preference_key"]
                        for row in grouped_rows
                        if row["preference_key"]
                    }
                ),
                "equivalent_response_families": sorted(
                    {
                        row["equivalent_response_family"]
                        for row in grouped_rows
                        if row["equivalent_response_family"]
                    }
                ),
                "reconcentrating_rate": format_pct(reconcentrating, len(outcomes)),
                "recovery_rate": format_pct(recovery, len(outcomes)),
            }
        )
    shadow_progress.sort(
        key=lambda row: (-int(row["resolved_live_unique_proposals"]), row["shadow_key"])
    )

    lead_frequency = top_counter_rows(
        Counter(
            row["lead_preference_key"]
            for row in shadow_policy_windows
            if row["lead_preference_key"]
        ),
        limit=8,
    )
    grouping_by_goal = []
    grouped_windows: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in shadow_policy_windows:
        grouped_windows[row["conversion_goal"]].append(row)
    for conversion_goal, grouped in grouped_windows.items():
        grouping_by_goal.append(
            {
                "conversion_goal": conversion_goal,
                "windows": len(grouped),
                "closest_fit_groups": top_counter_rows(
                    Counter(row["closest_fit_group"] for row in grouped), limit=4
                ),
            }
        )
    grouping_by_goal.sort(
        key=lambda row: (-int(row["windows"]), str(row["conversion_goal"]))
    )

    closest_fit_group_frequency = top_counter_rows(
        Counter(row["closest_fit_group"] for row in shadow_policy_windows), limit=8
    )
    closest_fit_outcome_mix = []
    closest_fit_grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in shadow_policy_windows:
        closest_fit_grouped[row["closest_fit_group"]].append(row)
    for closest_fit_group, grouped in closest_fit_grouped.items():
        outcomes = [row["outcome"] for row in grouped if row["outcome"] is not None]
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
        closest_fit_outcome_mix.append(
            {
                "closest_fit_group": closest_fit_group,
                "windows": len(grouped),
                "recovery_rate": format_pct(recovery, len(outcomes)),
                "reconcentrating_rate": format_pct(reconcentrating, len(outcomes)),
            }
        )
    closest_fit_outcome_mix.sort(
        key=lambda row: (-int(row["windows"]), str(row["closest_fit_group"]))
    )

    summary = {
        "generated_at": iso_now(),
        **overview,
        "indirect_uptake_rate_of_seen": format_pct(
            overview["proposals_with_astrid_indirect_uptake"],
            overview["proposals_seen_by_astrid"],
        ),
        "exact_uptake_rate_of_seen": format_pct(
            overview["proposals_with_astrid_exact_adoption"],
            overview["proposals_seen_by_astrid"],
        ),
        "shadow_confidence_counts": dict(
            Counter(row["confidence"] for row in shadow_rows)
        ),
        "resolved_live_shadow_confidence_counts": dict(
            Counter(row["confidence"] for row in resolved_real_shadow_rows)
        ),
        "top_fingerprints": top_counter_rows(fingerprint_counter, limit=8),
        "choice_summaries": choice_summaries,
        "category_summaries": category_summaries,
        "shadow_summaries": shadow_summaries,
        "resolved_live_shadow_progress": shadow_progress,
        "formed_lead_preference_frequency": lead_frequency,
        "shadow_policy_grouping_by_conversion_goal": grouping_by_goal,
        "closest_fit_group_frequency": closest_fit_group_frequency,
        "closest_fit_outcome_mix": closest_fit_outcome_mix,
        "shadow_policy_windows": shadow_policy_windows,
        "preference_eligible_shadow_keys": [
            row
            for row in shadow_progress
            if row["resolved_live_unique_proposals"] >= 3
        ],
        "tentative_observational_shadow_keys": [
            row for row in shadow_summaries if row["confidence"] != "high"
        ],
        "rows": rows,
        "shadow_rows": shadow_rows,
    }
    return summary


def build_report(summary: dict[str, Any]) -> list[str]:
    lines = [
        "# BTSP Astrid Indirect Uptake",
        "",
        f"- Generated: `{summary['generated_at']}`",
        f"- Proposals seen by Astrid: `{summary['proposals_seen_by_astrid']}`",
        f"- Proposals with Astrid indirect uptake: `{summary['proposals_with_astrid_indirect_uptake']}`",
        f"- Indirect uptake rate of seen proposals: `{summary['indirect_uptake_rate_of_seen']}`",
        f"- Proposals with Astrid exact bounded adoption: `{summary['proposals_with_astrid_exact_adoption']}`",
        f"- Exact uptake rate of seen proposals: `{summary['exact_uptake_rate_of_seen']}`",
        f"- Shadow-equivalence observations: `{len(summary['shadow_rows'])}`",
        f"- High-confidence shadow observations: `{summary['shadow_confidence_counts'].get('high', 0)}`",
        f"- Tentative shadow observations: `{summary['shadow_confidence_counts'].get('tentative', 0)}`",
        f"- Resolved live high-confidence shadow observations: `{summary['resolved_live_shadow_confidence_counts'].get('high', 0)}`",
        "- Preference-progress tracking excludes synthetic/test-like rows and only counts real resolved live proposals.",
        "",
        "## Category Mix",
    ]
    for row in summary["category_summaries"]:
        lines.append(
            f"- `{row['category']}`: `{row['observations']}` observations across "
            f"`{row['unique_proposals']}` proposals, `{row['recovery_rate']}` recovery, "
            f"`{row['reconcentrating_rate']}` reconcentrating"
        )
    lines.extend(["", "## Top Astrid Indirect Choices"])
    for row in summary["choice_summaries"][:12]:
        lines.append(
            f"- `{row['choice_key']}` ({row['category']}, {row['relation_to_proposal']}): "
            f"`{row['observations']}` observations, `{row['median_latency_minutes']}` median minutes, "
            f"`{row['recovery_rate']}` recovery, `{row['reconcentrating_rate']}` reconcentrating"
        )
        if row["top_fingerprints"]:
            lines.append(
                "  top cohorts: "
                + ", ".join(
                    f"{item['name']} ({item['count']})" for item in row["top_fingerprints"]
                )
            )
    lines.extend(["", "## Shadow Equivalence"])
    for row in summary["shadow_summaries"][:12]:
        lines.append(
            f"- `{row['shadow_key']}` ({row['confidence']}): `{row['observations']}` observations across "
            f"`{row['unique_proposals']}` proposals, `{row['recovery_rate']}` recovery, "
            f"`{row['reconcentrating_rate']}` reconcentrating"
        )
        if row["preference_keys"]:
            lines.append("  preference-eligible: " + ", ".join(row["preference_keys"]))
        if row["equivalent_response_families"]:
            lines.append(
                "  bounded analogue: " + ", ".join(row["equivalent_response_families"])
            )
    lines.extend(["", "## Preference Progress"])
    if summary["resolved_live_shadow_progress"]:
        for row in summary["resolved_live_shadow_progress"]:
            lines.append(
                f"- `{row['shadow_key']}`: `{row['progress_current']} / {row['progress_target']}` distinct resolved live proposals "
                f"toward preference memory, `{row['recovery_rate']}` recovery, `{row['reconcentrating_rate']}` reconcentrating"
            )
            if row["preference_keys"]:
                lines.append("  preference target: " + ", ".join(row["preference_keys"]))
            if row["equivalent_response_families"]:
                lines.append(
                    "  bounded analogue: " + ", ".join(row["equivalent_response_families"])
                )
            if row["remaining_for_preference_memory"] > 0:
                lines.append(
                    f"  remaining: `{row['remaining_for_preference_memory']}` more distinct resolved live proposals"
                )
    else:
        lines.append("- No real resolved high-confidence Astrid shadow observations yet.")
    lines.extend(["", "## Shadow Policy Reflection"])
    if summary["formed_lead_preference_frequency"]:
        lines.append("- Formed lead preference frequency:")
        for row in summary["formed_lead_preference_frequency"]:
            lines.append(f"  - `{row['name']}`: `{row['count']}` live Astrid windows")
    else:
        lines.append("- No live Astrid prompt windows carried shadow-policy reflection yet.")
    if summary["shadow_policy_grouping_by_conversion_goal"]:
        lines.append("- Grouping decisions by conversion goal:")
        for row in summary["shadow_policy_grouping_by_conversion_goal"]:
            groups = ", ".join(
                f"{item['name']} ({item['count']})" for item in row["closest_fit_groups"]
            )
            lines.append(
                f"  - `{row['conversion_goal']}`: `{row['windows']}` windows; closest fit groups: {groups}"
            )
    if summary["closest_fit_group_frequency"]:
        lines.append("- Closest-fit bounded group frequency:")
        for row in summary["closest_fit_group_frequency"]:
            lines.append(f"  - `{row['name']}`: `{row['count']}` live Astrid windows")
    if summary["closest_fit_outcome_mix"]:
        lines.append("- Outcome mix by closest-fit group:")
        for row in summary["closest_fit_outcome_mix"]:
            lines.append(
                f"  - `{row['closest_fit_group']}`: `{row['windows']}` windows, "
                f"`{row['recovery_rate']}` recovery, `{row['reconcentrating_rate']}` reconcentrating"
            )
    lines.extend(["", "## Preference-Eligible Shadow Keys"])
    if summary["preference_eligible_shadow_keys"]:
        for row in summary["preference_eligible_shadow_keys"]:
            lines.append(
                f"- `{row['shadow_key']}` is approaching or meeting preference-memory eligibility with "
                f"`{row['resolved_live_unique_proposals']}` distinct resolved live proposals."
            )
    else:
        lines.append("- No high-confidence Astrid shadow key has reached preference-memory eligibility yet.")
    lines.extend(["", "## Tentative Observational Keys"])
    if summary["tentative_observational_shadow_keys"]:
        for row in summary["tentative_observational_shadow_keys"]:
            lines.append(
                f"- `{row['shadow_key']}` remains observational only at `{row['observations']}` observations."
            )
    else:
        lines.append("- No tentative Astrid shadow keys were observed.")
    lines.extend(["", "## Most Common Indirect Cohorts"])
    for row in summary["top_fingerprints"]:
        lines.append(f"- `{row['name']}`: `{row['count']}` Astrid indirect observations")
    return lines


def collect_shadow_policy_windows(
    proposals: list[dict[str, Any]],
    episodes: list[dict[str, Any]],
    signal_events: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    proposal_lookup = {
        str(proposal.get("proposal_id", "")): proposal for proposal in proposals
    }
    first_windows: dict[str, dict[str, Any]] = {}
    for event in signal_events:
        if str(event.get("event_type", "")) != "prompt_rendered":
            continue
        if str(event.get("owner", "")) != OWNER_ASTRID:
            continue
        proposal_id = str(event.get("proposal_id", ""))
        if not proposal_id or proposal_id in first_windows:
            continue
        proposal = proposal_lookup.get(proposal_id)
        if proposal is None or not is_real_runtime_proposal(proposal):
            continue
        policy = event.get("astrid_shadow_policy")
        if not isinstance(policy, dict):
            continue
        candidate_groups = policy.get("candidate_groups", {})
        if not isinstance(candidate_groups, dict):
            candidate_groups = {}
        recorded_at = int(event.get("recorded_at_unix_s", 0) or 0)
        first_windows[proposal_id] = {
            "proposal_id": proposal_id,
            "lead_preference_key": str(policy.get("lead_preference_key", "")),
            "conversion_goal": str(policy.get("conversion_goal", "")),
            "collapse_state": str(policy.get("collapse_state", "")),
            "closest_fit_group": classify_closest_fit_group(
                candidate_groups.get("closest_fit_response_ids", [])
            ),
            "reason": str(policy.get("reason", "")),
            "recorded_at_unix_s": recorded_at,
            "outcome": first_future_outcome(
                proposal,
                episodes,
                after_unix_s=recorded_at,
            ),
        }
    return sorted(
        first_windows.values(),
        key=lambda row: int(row.get("recorded_at_unix_s", 0) or 0),
    )


def classify_closest_fit_group(response_ids: Any) -> str:
    if not isinstance(response_ids, list):
        return "unknown"
    normalized = sorted(str(response_id) for response_id in response_ids if str(response_id))
    if normalized == ["astrid_dampen"]:
        return "astrid_dampen"
    if normalized == ["astrid_breathe_alone", "astrid_echo_off"]:
        return "astrid_breathe_alone+astrid_echo_off"
    return "+".join(normalized) if normalized else "unknown"


def main() -> None:
    args = parse_args()
    proposals, episodes = load_runtime(args.proposal_ledger, args.episode_bank)
    signal_events = load_signal_events(args.signal_events)
    rows, shadow_rows, overview = collect_rows(proposals, episodes)
    shadow_policy_windows = collect_shadow_policy_windows(
        proposals,
        episodes,
        signal_events,
    )
    summary = summarize_rows(rows, shadow_rows, overview, shadow_policy_windows)
    report_lines = build_report(summary)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    write_json(args.output_dir / "summary.json", summary)
    write_report(args.output_dir / "report.md", report_lines)


if __name__ == "__main__":
    main()
