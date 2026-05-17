#!/usr/bin/env python3
"""
Render a BTSP agency-recovery scoreboard from the live proposal ledger.
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from btsp_runtime_analysis import (
    DEFAULT_EPISODE_BANK,
    DEFAULT_PROPOSAL_LEDGER,
    format_pct,
    iso_now,
    load_runtime,
    proposal_outcomes,
    write_json,
    write_report,
)


DEFAULT_OUTPUT_DIR = Path(
    "/Users/v/other/astrid/capsules/consciousness-bridge/workspace/diagnostics/btsp_agency_scoreboard"
)
ACTIVE_STATES = {"unseen", "witnessed", "answered", "adopted"}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--proposal-ledger", type=Path, default=DEFAULT_PROPOSAL_LEDGER)
    parser.add_argument("--episode-bank", type=Path, default=DEFAULT_EPISODE_BANK)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    return parser.parse_args()


def summarize_agency(
    proposals: list[dict[str, Any]], episodes: list[dict[str, Any]]
) -> dict[str, Any]:
    by_owner: dict[str, Counter[str]] = defaultdict(Counter)
    recent: list[dict[str, Any]] = []
    recovery_positive = 0
    recovery_total = 0

    for proposal in proposals:
        proposal_id = str(proposal.get("proposal_id", ""))
        reply_state = str(proposal.get("reply_state", ""))
        for adoption in proposal.get("exact_adoptions", []) or []:
            if not isinstance(adoption, dict):
                continue
            owner = str(adoption.get("owner") or "unknown")
            by_owner[owner]["exact_accept"] += 1
            recent.append(
                {
                    "proposal_id": proposal_id,
                    "owner": owner,
                    "agency_outcome": "exact_accept",
                    "response_id": str(adoption.get("response_id") or ""),
                }
            )
        for refusal in proposal.get("refusals", []) or []:
            if not isinstance(refusal, dict):
                continue
            owner = str(refusal.get("owner") or "unknown")
            by_owner[owner]["refusal"] += 1
            recent.append(
                {
                    "proposal_id": proposal_id,
                    "owner": owner,
                    "agency_outcome": "refusal",
                    "reason": str(refusal.get("reason") or ""),
                }
            )
        for counteroffer in proposal.get("counteroffers", []) or []:
            if not isinstance(counteroffer, dict):
                continue
            owner = str(counteroffer.get("owner") or "unknown")
            by_owner[owner]["counteroffer"] += 1
            if counteroffer.get("requested_response_id"):
                by_owner[owner]["parseable_counteroffer"] += 1
            recent.append(
                {
                    "proposal_id": proposal_id,
                    "owner": owner,
                    "agency_outcome": "counteroffer",
                    "state": str(counteroffer.get("state") or ""),
                    "requested_response_id": str(
                        counteroffer.get("requested_response_id") or ""
                    ),
                }
            )
        for interpretation in proposal.get("choice_interpretations", []) or []:
            if not isinstance(interpretation, dict):
                continue
            if str(interpretation.get("relation_to_proposal") or "") == "exact_nominated":
                continue
            owner = str(interpretation.get("owner") or "unknown")
            by_owner[owner]["adjacent_uptake"] += 1
        if reply_state == "expired":
            by_owner["system"]["expiration"] += 1
        if reply_state in ACTIVE_STATES:
            by_owner["system"]["active"] += 1

        for outcome in proposal_outcomes(proposal, episodes):
            if not isinstance(outcome, dict):
                continue
            if str(outcome.get("owner") or "") == "system":
                continue
            recovery_total += 1
            if (
                str(outcome.get("distress_or_recovery") or "") == "recovery"
                or str(outcome.get("target_nearness") or "") == "positive"
                or str(outcome.get("opening_vs_reconcentration") or "") == "opening"
            ):
                recovery_positive += 1

    owner_rows = []
    for owner, counts in sorted(by_owner.items()):
        total_owner_choices = (
            counts["exact_accept"]
            + counts["refusal"]
            + counts["counteroffer"]
            + counts["adjacent_uptake"]
        )
        owner_rows.append(
            {
                "owner": owner,
                "exact_accept": counts["exact_accept"],
                "refusal": counts["refusal"],
                "counteroffer": counts["counteroffer"],
                "parseable_counteroffer": counts["parseable_counteroffer"],
                "adjacent_uptake": counts["adjacent_uptake"],
                "expiration": counts["expiration"],
                "active": counts["active"],
                "agency_expression_total": total_owner_choices,
            }
        )
    owner_rows.sort(
        key=lambda row: (
            row["owner"] == "system",
            -int(row["agency_expression_total"]),
            row["owner"],
        )
    )

    return {
        "generated_at": iso_now(),
        "proposal_count": len(proposals),
        "owner_rows": owner_rows,
        "agency_recovery_positive_rate": format_pct(recovery_positive, recovery_total),
        "agency_recovery_positive": recovery_positive,
        "agency_recovery_total": recovery_total,
        "recent_agency_events": recent[-12:],
    }


def build_report(summary: dict[str, Any]) -> list[str]:
    lines = [
        "# BTSP Agency Scoreboard",
        "",
        f"- Generated: `{summary['generated_at']}`",
        f"- Proposals inspected: `{summary['proposal_count']}`",
        f"- Agency recovery positive rate: `{summary['agency_recovery_positive_rate']}` "
        f"({summary['agency_recovery_positive']}/{summary['agency_recovery_total']})",
        "",
        "## Owner Agency",
    ]
    for row in summary["owner_rows"]:
        lines.append(
            f"- `{row['owner']}`: yes `{row['exact_accept']}`, no `{row['refusal']}`, "
            f"almost `{row['counteroffer']}`, adjacent `{row['adjacent_uptake']}`, "
            f"parseable almost `{row['parseable_counteroffer']}`, expired `{row['expiration']}`"
        )
    lines.extend(["", "## Recent Agency Events"])
    for event in summary["recent_agency_events"]:
        extra = (
            event.get("response_id")
            or event.get("requested_response_id")
            or event.get("reason")
            or event.get("state")
            or ""
        )
        lines.append(
            f"- `{event['proposal_id']}` `{event['owner']}` "
            f"`{event['agency_outcome']}` {extra}".rstrip()
        )
    return lines


def main() -> int:
    args = parse_args()
    proposals, episodes = load_runtime(args.proposal_ledger, args.episode_bank)
    summary = summarize_agency(proposals, episodes)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    write_json(args.output_dir / "summary.json", summary)
    write_report(args.output_dir / "README.md", build_report(summary))
    print(args.output_dir / "README.md")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
