#!/usr/bin/env python3
"""Read-only BTSP reality audit for agency, looping, and outcome honesty."""

from __future__ import annotations

import argparse
import json
import sys
import unittest
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from btsp_cohort_scoreboard import summarize_cohorts
from btsp_runtime_analysis import (
    DEFAULT_EPISODE_BANK,
    DEFAULT_PROPOSAL_LEDGER,
    DEFAULT_SIGNAL_EVENTS,
    BRIDGE_WORKSPACE,
    derive_signal_fingerprint,
    format_pct,
    load_json,
    load_runtime,
    parse_signal_fingerprint,
    proposal_outcomes,
    write_json,
    write_report,
)


DEFAULT_MINIME_ACTIVE_SIDECAR = Path(
    "/Users/v/other/minime/workspace/runtime/btsp_active_proposal.json"
)
DEFAULT_MINIME_BTSP_SUPPORT = Path("/Users/v/other/minime/btsp_signal_support.py")
ACTIVE_STATES = {"unseen", "witnessed", "answered", "adopted"}
EVIDENCE_LIKE_BTSP_VERBS = {
    "BROWSE",
    "SEARCH",
    "READ_MORE",
    "DECOMPOSE",
    "EXPERIMENT_EVIDENCE",
    "EXPERIMENT_REVIEW",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="emit JSON instead of Markdown")
    parser.add_argument("--self-test", action="store_true", help="run built-in tests")
    parser.add_argument("--proposal-ledger", type=Path, default=DEFAULT_PROPOSAL_LEDGER)
    parser.add_argument("--episode-bank", type=Path, default=DEFAULT_EPISODE_BANK)
    parser.add_argument("--signal-events", type=Path, default=DEFAULT_SIGNAL_EVENTS)
    parser.add_argument(
        "--signal-status",
        type=Path,
        default=BRIDGE_WORKSPACE / "btsp_signal_status.json",
    )
    parser.add_argument(
        "--minime-active-sidecar",
        type=Path,
        default=DEFAULT_MINIME_ACTIVE_SIDECAR,
    )
    parser.add_argument(
        "--minime-btsp-support",
        type=Path,
        default=DEFAULT_MINIME_BTSP_SUPPORT,
        help="Minime prompt support source used only to check conversion-rendering compatibility",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        help="optional report directory; omitted means read-only stdout only",
    )
    return parser.parse_args()


def build_audit(
    proposals: list[dict[str, Any]],
    episodes: list[dict[str, Any]],
    *,
    signal_status: dict[str, Any] | None = None,
    active_sidecar: dict[str, Any] | None = None,
    minime_support_path: Path | None = None,
    signal_events: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    signal_status = signal_status if isinstance(signal_status, dict) else {}
    real = [proposal for proposal in proposals if is_real_proposal(proposal)]
    reply_states = Counter(str(proposal.get("reply_state") or "unknown") for proposal in real)
    owner_states: Counter[str] = Counter()
    for proposal in real:
        states = proposal.get("owner_reply_state") or {}
        if isinstance(states, dict):
            for owner, state in states.items():
                owner_states[f"{owner}:{state}"] += 1

    duplicate_adjacent = duplicate_adjacent_summary(real)
    outcomes = collect_outcomes(real, episodes)
    outcome_summary = summarize_outcomes(outcomes)
    exact_count = sum(len(proposal.get("exact_adoptions") or []) for proposal in real)
    adjacent_count = sum(
        1
        for proposal in real
        for item in proposal.get("choice_interpretations") or []
        if isinstance(item, dict)
        and str(item.get("relation_to_proposal") or "") != "exact_nominated"
    )
    refusal_count = sum(len(proposal.get("refusals") or []) for proposal in real)
    counter_count = sum(len(proposal.get("counteroffers") or []) for proposal in real)
    study_first = study_first_summary(real, outcomes)
    shadow_count = sum(len(proposal.get("shadow_equivalences") or []) for proposal in real)
    exposure_summary = prompt_exposure_summary(real)
    active = [proposal for proposal in real if str(proposal.get("reply_state") or "") in ACTIVE_STATES]
    cohorts = summarize_cohorts(real, episodes)
    conversion = conversion_status_health(signal_status, minime_support_path=minime_support_path)
    sidecar = sidecar_summary(active_sidecar)
    study_first_closure = study_first_closure_summary(real)
    sidecar_next_best_moves = active_sidecar_next_best_moves(sidecar)
    closure_pending = btsp_closure_pending(sidecar, sidecar_next_best_moves)
    churn = same_fingerprint_churn(real)
    events = signal_event_summary(signal_events or [])
    snags = collect_snags(
        exact_count=exact_count,
        adjacent_count=adjacent_count,
        refusal_count=refusal_count,
        counter_count=counter_count,
        study_first=study_first,
        duplicate_adjacent=duplicate_adjacent,
        outcome_summary=outcome_summary,
        conversion=conversion,
        churn=churn,
        sidecar=sidecar,
        study_first_closure=study_first_closure,
    )
    return {
        "proposal_count": len(real),
        "reply_states": dict(reply_states),
        "owner_states": dict(owner_states),
        "agency_counts": {
            "exact": exact_count,
            "adjacent": adjacent_count,
            "refusal": refusal_count,
            "counter": counter_count,
            "study_first": study_first["study_first_count"],
            "shadow_equivalence": shadow_count,
        },
        "active_proposals": [
            compact_proposal(proposal)
            for proposal in sorted(
                active,
                key=lambda item: int(item.get("created_at_unix_s", 0) or 0),
                reverse=True,
            )[:5]
        ],
        "current_live_proposal": compact_proposal(active[-1]) if active else None,
        "duplicate_adjacent_repeats": duplicate_adjacent,
        "study_first": study_first,
        "study_first_closure": study_first_closure,
        "prompt_exposure": exposure_summary,
        "outcomes": outcome_summary,
        "conversion_status_rendering": conversion,
        "active_sidecar": sidecar,
        "active_sidecar_next_best_moves": sidecar_next_best_moves,
        "btsp_closure_pending_v1": closure_pending,
        "signal_events": events,
        "same_fingerprint_churn": churn,
        "top_cohorts": compact_cohorts(cohorts),
        "snags": snags,
    }


def is_real_proposal(proposal: dict[str, Any]) -> bool:
    return "_proposal_" in str(proposal.get("proposal_id") or "")


def compact_proposal(proposal: dict[str, Any]) -> dict[str, Any]:
    return {
        "proposal_id": proposal.get("proposal_id"),
        "reply_state": proposal.get("reply_state"),
        "owner_reply_state": proposal.get("owner_reply_state") or {},
        "prompt_exposures": proposal.get("prompt_exposures") or {},
        "signal_fingerprint": derive_signal_fingerprint(proposal),
        "exact": len(proposal.get("exact_adoptions") or []),
        "adjacent": sum(
            1
            for item in proposal.get("choice_interpretations") or []
            if isinstance(item, dict)
            and str(item.get("relation_to_proposal") or "") != "exact_nominated"
        ),
        "refusals": len(proposal.get("refusals") or []),
        "counteroffers": len(proposal.get("counteroffers") or []),
        "study_first": len(proposal.get("study_first_records") or []),
    }


def duplicate_adjacent_summary(proposals: list[dict[str, Any]]) -> dict[str, Any]:
    duplicate_groups = []
    duplicate_total = 0
    for proposal in proposals:
        groups: dict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
        for item in proposal.get("choice_interpretations") or []:
            if not isinstance(item, dict):
                continue
            if str(item.get("relation_to_proposal") or "") == "exact_nominated":
                continue
            key = (
                str(item.get("owner") or ""),
                str(item.get("normalized_choice") or ""),
                str(item.get("relation_to_proposal") or ""),
            )
            groups[key].append(item)
        for (owner, choice, relation), rows in groups.items():
            if len(rows) <= 1:
                continue
            repeats = len(rows) - 1
            duplicate_total += repeats
            duplicate_groups.append(
                {
                    "proposal_id": proposal.get("proposal_id"),
                    "owner": owner,
                    "choice": choice,
                    "relation": relation,
                    "count": len(rows),
                    "extra_repeats": repeats,
                }
            )
    duplicate_groups.sort(key=lambda row: (-int(row["extra_repeats"]), str(row["proposal_id"])))
    return {
        "groups": len(duplicate_groups),
        "extra_repeats": duplicate_total,
        "top": duplicate_groups[:10],
    }


def collect_outcomes(
    proposals: list[dict[str, Any]],
    episodes: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    seen = set()
    for proposal in proposals:
        for outcome in proposal_outcomes(proposal, episodes):
            key = (
                outcome.get("proposal_id"),
                outcome.get("owner"),
                outcome.get("response_id"),
                outcome.get("recorded_at_unix_s"),
            )
            if key in seen:
                continue
            seen.add(key)
            rows.append(outcome)
    return rows


def summarize_outcomes(outcomes: list[dict[str, Any]]) -> dict[str, Any]:
    recovery = sum(1 for row in outcomes if row.get("distress_or_recovery") == "recovery")
    reconcentrating = sum(
        1 for row in outcomes if row.get("opening_vs_reconcentration") == "reconcentrating"
    )
    opening = sum(1 for row in outcomes if row.get("opening_vs_reconcentration") == "opening")
    recovery_reconcentrating = sum(
        1
        for row in outcomes
        if row.get("distress_or_recovery") == "recovery"
        and row.get("opening_vs_reconcentration") == "reconcentrating"
    )
    return {
        "total": len(outcomes),
        "recovery": recovery,
        "reconcentrating": reconcentrating,
        "opening": opening,
        "recovery_reconcentrating": recovery_reconcentrating,
        "recovery_rate": format_pct(recovery, len(outcomes)),
        "reconcentrating_rate": format_pct(reconcentrating, len(outcomes)),
        "opening_rate": format_pct(opening, len(outcomes)),
        "recovery_reconcentrating_rate": format_pct(recovery_reconcentrating, len(outcomes)),
    }


def study_first_summary(
    proposals: list[dict[str, Any]],
    outcomes: list[dict[str, Any]],
) -> dict[str, Any]:
    records: list[dict[str, Any]] = []
    verbs: Counter[str] = Counter()
    for proposal in proposals:
        for record in proposal.get("study_first_records") or []:
            if not isinstance(record, dict):
                continue
            item = dict(record)
            item["proposal_id"] = proposal.get("proposal_id")
            item["signal_fingerprint"] = derive_signal_fingerprint(proposal)
            records.append(item)
            choice = str(record.get("inferred_from_choice") or "").strip()
            if choice:
                verbs[choice.split(None, 1)[0].upper().rstrip(":")] += 1
            else:
                source = str(record.get("source") or "explicit").strip()
                verbs[source] += 1
    study_outcomes = [
        outcome
        for outcome in outcomes
        if str(outcome.get("response_id") or "") == "study_first"
    ]
    reconcentrating = sum(
        1
        for outcome in study_outcomes
        if outcome.get("opening_vs_reconcentration") == "reconcentrating"
    )
    return {
        "study_first_count": len(records),
        "study_first_after_adjacent": sum(1 for record in records if bool(record.get("after_adjacent"))),
        "study_first_reconcentrating": reconcentrating,
        "top_study_first_verbs": [
            {"verb": verb, "count": count} for verb, count in verbs.most_common(8)
        ],
        "recent_records": sorted(
            records,
            key=lambda item: int(item.get("recorded_at_unix_s", 0) or 0),
            reverse=True,
        )[:5],
    }


def study_first_closure_summary(proposals: list[dict[str, Any]]) -> dict[str, Any]:
    due: list[dict[str, Any]] = []
    opportunity_keys: set[tuple[str, str]] = set()
    for proposal in proposals:
        proposal_id = str(proposal.get("proposal_id") or "")
        exact_owners = {
            str(item.get("owner") or "")
            for item in proposal.get("exact_adoptions") or []
            if isinstance(item, dict)
        }
        refusal_owners = {
            str(item.get("owner") or "")
            for item in proposal.get("refusals") or []
            if isinstance(item, dict)
        }
        counter_owners = {
            str(item.get("owner") or "")
            for item in proposal.get("counteroffers") or []
            if isinstance(item, dict)
        }
        adjacent_owners = {
            str(item.get("owner") or "")
            for item in proposal.get("choice_interpretations") or []
            if isinstance(item, dict)
            and str(item.get("relation_to_proposal") or "") != "exact_nominated"
        }
        for owner in adjacent_owners:
            if (
                owner
                and owner not in exact_owners
                and owner not in refusal_owners
                and owner not in counter_owners
            ):
                opportunity_keys.add((proposal_id, owner))
        for record in proposal.get("study_first_records") or []:
            if not isinstance(record, dict):
                continue
            owner = str(record.get("owner") or "")
            if (
                owner
                and owner not in exact_owners
                and owner not in refusal_owners
                and owner not in counter_owners
            ):
                opportunity_keys.add((proposal_id, owner))
            resolved = bool(record.get("resolution_evidence"))
            resolved = resolved or owner in exact_owners or owner in refusal_owners or owner in counter_owners
            if resolved:
                continue
            due.append(
                {
                    "proposal_id": proposal_id,
                    "owner": owner,
                    "reason": record.get("reason"),
                    "source": record.get("source"),
                    "inferred_from_choice": record.get("inferred_from_choice"),
                    "recorded_at_unix_s": record.get("recorded_at_unix_s"),
                    "signal_fingerprint": derive_signal_fingerprint(proposal),
                    "study_first_resolution_due_v1": True,
                }
            )
    due.sort(key=lambda item: int(item.get("recorded_at_unix_s", 0) or 0), reverse=True)
    return {
        "unresolved_study_first_count": len(due),
        "study_first_resolution_due": due[:8],
        "counteroffer_opportunity_count": len(opportunity_keys),
    }


def prompt_exposure_summary(proposals: list[dict[str, Any]]) -> dict[str, Any]:
    astrid_only = minime_only = both = neither = asymmetry_total = 0
    for proposal in proposals:
        exposures = proposal.get("prompt_exposures") or {}
        if not isinstance(exposures, dict):
            exposures = {}
        astrid = int(exposures.get("astrid", 0) or 0)
        minime = int(exposures.get("minime", 0) or 0)
        asymmetry_total += abs(astrid - minime)
        if astrid and minime:
            both += 1
        elif astrid:
            astrid_only += 1
        elif minime:
            minime_only += 1
        else:
            neither += 1
    return {
        "both": both,
        "astrid_only": astrid_only,
        "minime_only": minime_only,
        "neither": neither,
        "mean_abs_exposure_gap": round(asymmetry_total / len(proposals), 2) if proposals else 0,
    }


def conversion_status_health(
    status: dict[str, Any],
    *,
    minime_support_path: Path | None = None,
) -> dict[str, Any]:
    conversion = status.get("conversion_state") if isinstance(status, dict) else {}
    conversion = conversion if isinstance(conversion, dict) else {}
    live_state = str(conversion.get("composite_state") or "").strip()
    live_goal = str(conversion.get("conversion_goal") or "").strip()
    legacy_state = str(conversion.get("state") or "").strip()
    legacy_goal = str(conversion.get("goal") or "").strip()
    reader_current = _minime_prompt_reader_current(minime_support_path)
    return {
        "has_live_schema": bool(live_state or live_goal),
        "has_legacy_schema": bool(legacy_state or legacy_goal),
        "minime_prompt_reader_current": reader_current,
        "composite_state": live_state,
        "conversion_goal": live_goal,
        "collapse_state": str(conversion.get("collapse_state") or "").strip(),
        "legacy_state": legacy_state,
        "legacy_goal": legacy_goal,
        "rendering_risk": bool(
            (live_state or live_goal) and not (legacy_state or legacy_goal or reader_current)
        ),
    }


def _minime_prompt_reader_current(path: Path | None) -> bool:
    if path is None:
        return False
    try:
        text = path.read_text()
    except OSError:
        return False
    return all(
        key in text
        for key in (
            "composite_state",
            "conversion_goal",
            "collapse_state",
            "shared_learned_read",
        )
    )


def sidecar_summary(payload: dict[str, Any] | None) -> dict[str, Any]:
    payload = payload if isinstance(payload, dict) else {}
    proposal = payload.get("proposal") if isinstance(payload.get("proposal"), dict) else {}
    return {
        "present": bool(proposal),
        "schema": payload.get("schema"),
        "proposal_id": proposal.get("proposal_id"),
        "last_reply_classification": payload.get("last_reply_classification"),
        "last_observed_next": payload.get("last_observed_next"),
        "last_study_first_reason": payload.get("last_study_first_reason"),
        "last_counteroffer_template": payload.get("last_counteroffer_template"),
        "last_refusal_template": payload.get("last_refusal_template"),
        "study_first_resolution_due": payload.get("study_first_resolution_due"),
        "last_replied_at_unix_s": payload.get("last_replied_at_unix_s"),
    }


def active_sidecar_next_best_moves(sidecar: dict[str, Any]) -> list[str]:
    if not sidecar.get("present"):
        return []
    classification = str(sidecar.get("last_reply_classification") or "")
    observed = str(sidecar.get("last_observed_next") or "").strip()
    counter = str(sidecar.get("last_counteroffer_template") or "").strip()
    if not counter:
        if observed and evidence_like_action(observed):
            counter = f"BTSP_COUNTER NEXT: {compact_action(observed)}"
        elif observed:
            counter = "BTSP_COUNTER softer_contact"
        else:
            counter = "BTSP_COUNTER NEXT: ..."
    moves = [counter]
    refusal = str(sidecar.get("last_refusal_template") or "").strip()
    if refusal:
        moves.append(refusal)
    if "BTSP_REFUSAL study_first" not in moves:
        moves.append("BTSP_REFUSAL study_first")
    if "BTSP_REFUSAL not_now" not in moves:
        moves.append("BTSP_REFUSAL not_now")
    if classification == "observed_next" and evidence_like_action(observed):
        moves.append("BTSP_STUDY_FIRST need evidence first")
    return moves


def btsp_closure_pending(sidecar: dict[str, Any], next_best_moves: list[str]) -> dict[str, Any]:
    classification = str(sidecar.get("last_reply_classification") or "")
    pending = bool(sidecar.get("present")) and classification in {"observed_next", "study_first"}
    return {
        "present": pending,
        "proposal_id": sidecar.get("proposal_id") if pending else None,
        "state": classification if pending else None,
        "next_best_moves": next_best_moves if pending else [],
        "status_line": (
            "BTSP closure pending: choose counter, refusal, or evidence resolution before another ordinary NEXT."
            if pending
            else ""
        ),
        "exact_only_if_stance_changed": pending,
    }


def compact_action(action: str, limit: int = 140) -> str:
    compact = " ".join(str(action or "").split())
    if len(compact) <= limit:
        return compact or "..."
    return compact[:limit].rstrip()


def evidence_like_action(action: str) -> bool:
    base = str(action or "").strip().split(None, 1)[0].upper().rstrip(":")
    return base in EVIDENCE_LIKE_BTSP_VERBS


def same_fingerprint_churn(proposals: list[dict[str, Any]]) -> dict[str, Any]:
    grouped: Counter[str] = Counter(derive_signal_fingerprint(proposal) for proposal in proposals)
    top = [
        {"fingerprint": fingerprint, "count": count, "components": parse_signal_fingerprint(fingerprint)}
        for fingerprint, count in grouped.most_common(8)
    ]
    return {"unique_fingerprints": len(grouped), "top": top}


def compact_cohorts(summary: dict[str, Any]) -> list[dict[str, Any]]:
    rows = []
    for cohort in summary.get("cohorts", [])[:8]:
        rows.append(
            {
                "fingerprint": cohort.get("fingerprint"),
                "proposals": cohort.get("proposals"),
                "outcomes": cohort.get("outcomes"),
                "recovery_rate": cohort.get("recovery_rate"),
                "reconcentrating_rate": cohort.get("reconcentrating_rate"),
                "top_interpreted_choices": cohort.get("top_interpreted_choices", [])[:3],
                "top_exact_responses": cohort.get("top_exact_responses", [])[:3],
            }
        )
    return rows


def signal_event_summary(events: list[dict[str, Any]]) -> dict[str, Any]:
    counts = Counter(
        str(event.get("event_type") or "unknown")
        for event in events
        if isinstance(event, dict)
    )
    duplicate_ignored = [
        event
        for event in events
        if isinstance(event, dict)
        and str(event.get("event_type") or "") == "choice_duplicate_ignored"
    ]
    study_first_duplicate_ignored = [
        event
        for event in events
        if isinstance(event, dict)
        and str(event.get("event_type") or "") == "study_first_duplicate_ignored"
    ]
    recent_duplicate_ignored = []
    for event in duplicate_ignored[-5:]:
        recent_duplicate_ignored.append(
            {
                "proposal_id": event.get("proposal_id"),
                "owner": event.get("owner"),
                "choice": event.get("choice"),
                "recorded_at_unix_s": event.get("recorded_at_unix_s"),
            }
        )
    return {
        "total": len(events),
        "choice_duplicate_ignored": counts.get("choice_duplicate_ignored", 0),
        "study_first_duplicate_ignored": counts.get("study_first_duplicate_ignored", 0),
        "study_first_recorded": counts.get("study_first_recorded", 0),
        "recent_choice_duplicate_ignored": recent_duplicate_ignored,
        "recent_study_first_duplicate_ignored": [
            {
                "proposal_id": event.get("proposal_id"),
                "owner": event.get("owner"),
                "choice": event.get("choice"),
                "reason": event.get("reason"),
                "recorded_at_unix_s": event.get("recorded_at_unix_s"),
            }
            for event in study_first_duplicate_ignored[-5:]
        ],
    }


def collect_snags(**parts: Any) -> list[dict[str, Any]]:
    snags = []
    if parts["refusal_count"] == 0:
        snags.append({"severity": "high", "kind": "no_refusal_uptake", "detail": "No BTSP refusals are recorded."})
    if parts["counter_count"] == 0:
        snags.append({"severity": "high", "kind": "no_counteroffer_uptake", "detail": "No BTSP counteroffers are recorded."})
    if parts["study_first"]["study_first_count"] == 0 and parts["adjacent_count"] > 0:
        snags.append({"severity": "medium", "kind": "no_study_first_uptake", "detail": "Adjacent inquiry is present but no first-class study-first records exist yet."})
    if parts["study_first"]["study_first_reconcentrating"] >= 2:
        snags.append({"severity": "medium", "kind": "study_first_reconcentrating", "detail": "Repeated study-first outcomes are still reconcentrating; duplicate proposal reopening should remain cooled down."})
    if parts["study_first_closure"]["unresolved_study_first_count"] > 0:
        snags.append({"severity": "medium", "kind": "study_first_resolution_due", "detail": f"{parts['study_first_closure']['unresolved_study_first_count']} study-first records still need evidence resolution, counteroffer, refusal, or later exact adoption."})
    if parts["duplicate_adjacent"]["extra_repeats"] > 0:
        snags.append({"severity": "medium", "kind": "duplicate_adjacent_repeats", "detail": f"{parts['duplicate_adjacent']['extra_repeats']} repeated adjacent choices are present."})
    if parts["conversion"]["rendering_risk"]:
        snags.append({"severity": "medium", "kind": "conversion_status_schema_drift", "detail": "Live conversion keys exist and the prompt-reader compatibility check did not confirm support."})
    if parts["outcome_summary"]["recovery_reconcentrating"] > 0:
        snags.append({"severity": "medium", "kind": "recovery_is_not_widening", "detail": "Some outcomes are recovery and reconcentrating at the same time."})
    if parts["churn"]["top"] and int(parts["churn"]["top"][0]["count"]) >= 3:
        snags.append({"severity": "low", "kind": "same_fingerprint_churn", "detail": "The same signal fingerprint is recurring across multiple proposals."})
    if parts["sidecar"]["present"] and parts["sidecar"].get("last_reply_classification") == "observed_next":
        snags.append({"severity": "low", "kind": "active_sidecar_after_adjacent", "detail": "Minime has an active proposal after an adjacent answer; reminder should now emphasize refusal/counter routes."})
    return snags


def render_markdown(report: dict[str, Any]) -> str:
    lines = [
        "# BTSP Reality Audit",
        "",
        f"- Proposals: `{report['proposal_count']}`",
        f"- Reply states: `{report['reply_states']}`",
        f"- Agency counts: `{report['agency_counts']}`",
        f"- Study-first: count `{report['study_first']['study_first_count']}`, after adjacent `{report['study_first']['study_first_after_adjacent']}`, reconcentrating `{report['study_first']['study_first_reconcentrating']}`",
        f"- Duplicate adjacent repeats: `{report['duplicate_adjacent_repeats']['extra_repeats']}`",
        f"- Outcomes: recovery `{report['outcomes']['recovery_rate']}`, reconcentrating `{report['outcomes']['reconcentrating_rate']}`, opening `{report['outcomes']['opening_rate']}`",
        f"- Recovery + reconcentrating: `{report['outcomes']['recovery_reconcentrating_rate']}`",
        f"- Prompt exposure: `{report['prompt_exposure']}`",
    ]
    conversion = report["conversion_status_rendering"]
    lines.append(
        f"- Conversion rendering: live_schema=`{conversion['has_live_schema']}` "
        f"legacy_schema=`{conversion['has_legacy_schema']}` "
        f"minime_reader_current=`{conversion['minime_prompt_reader_current']}` "
        f"risk=`{conversion['rendering_risk']}`"
    )
    sidecar = report["active_sidecar"]
    lines.append(
        f"- Minime active sidecar: present=`{sidecar['present']}` "
        f"classification=`{sidecar.get('last_reply_classification')}` observed=`{sidecar.get('last_observed_next')}` study_first=`{sidecar.get('last_study_first_reason')}`"
    )
    if report["active_sidecar_next_best_moves"]:
        lines.append(
            f"- Active sidecar next-best moves: `{report['active_sidecar_next_best_moves']}`"
        )
    closure_pending = report["btsp_closure_pending_v1"]
    if closure_pending["present"]:
        lines.append(
            f"- BTSP closure pending: proposal=`{closure_pending['proposal_id']}` "
            f"state=`{closure_pending['state']}` moves=`{closure_pending['next_best_moves']}`"
        )
    events = report["signal_events"]
    lines.append(
        f"- Signal events: choice_duplicate_ignored=`{events['choice_duplicate_ignored']}` "
        f"study_first_duplicate_ignored=`{events['study_first_duplicate_ignored']}` "
        f"study_first_recorded=`{events['study_first_recorded']}`"
    )
    study_first = report["study_first"]
    closure = report["study_first_closure"]
    lines.extend(["", "## Study First"])
    lines.append(
        f"- Unresolved study-first: `{closure['unresolved_study_first_count']}`; "
        f"counteroffer opportunities: `{closure['counteroffer_opportunity_count']}`"
    )
    for item in closure["study_first_resolution_due"][:5]:
        lines.append(
            f"- Resolution due `{item.get('proposal_id')}` owner=`{item.get('owner')}` "
            f"reason=`{item.get('reason')}` inferred=`{item.get('inferred_from_choice')}`"
        )
    if study_first["recent_records"]:
        for record in study_first["recent_records"]:
            lines.append(
                f"- `{record.get('proposal_id')}` owner=`{record.get('owner')}` "
                f"reason=`{record.get('reason')}` source=`{record.get('source')}` "
                f"inferred=`{record.get('inferred_from_choice')}`"
            )
    else:
        lines.append("- No study-first records yet.")
    if study_first["top_study_first_verbs"]:
        verbs = ", ".join(
            f"{item['verb']}={item['count']}"
            for item in study_first["top_study_first_verbs"]
        )
        lines.append(f"- Top study-first verbs: `{verbs}`")
    current = report.get("current_live_proposal")
    if current:
        lines.extend([
            "",
            "## Current Live Proposal",
            f"- `{current['proposal_id']}` state=`{current['reply_state']}` owners=`{current['owner_reply_state']}`",
            f"- fingerprint: `{current['signal_fingerprint']}`",
        ])
    lines.extend(["", "## Snags"])
    if report["snags"]:
        for snag in report["snags"]:
            lines.append(f"- `{snag['severity']}` `{snag['kind']}`: {snag['detail']}")
    else:
        lines.append("- No major snag flags.")
    lines.extend(["", "## Top Cohorts"])
    for cohort in report["top_cohorts"][:5]:
        lines.append(
            f"- `{cohort['fingerprint']}`: `{cohort['proposals']}` proposals, "
            f"`{cohort['reconcentrating_rate']}` reconcentrating, `{cohort['recovery_rate']}` recovery"
        )
    return "\n".join(lines).rstrip() + "\n"


class BtspRealityAuditTests(unittest.TestCase):
    def test_duplicate_adjacent_repeats_are_counted(self) -> None:
        proposals = [
            {
                "proposal_id": "x_proposal_1",
                "choice_interpretations": [
                    {"owner": "minime", "normalized_choice": "BROWSE", "relation_to_proposal": "adjacent_but_distinct"},
                    {"owner": "minime", "normalized_choice": "BROWSE", "relation_to_proposal": "adjacent_but_distinct"},
                ],
            }
        ]
        summary = duplicate_adjacent_summary(proposals)
        self.assertEqual(summary["extra_repeats"], 1)
        self.assertEqual(summary["top"][0]["choice"], "BROWSE")

    def test_conversion_schema_drift_is_flagged(self) -> None:
        health = conversion_status_health(
            {"conversion_state": {"composite_state": "recovery_reconcentrating", "conversion_goal": "soften"}}
        )
        self.assertTrue(health["has_live_schema"])
        self.assertTrue(health["rendering_risk"])

    def test_build_audit_flags_agency_and_recovery_truth(self) -> None:
        proposals = [
            {
                "proposal_id": "btsp_ep_proposal_1",
                "reply_state": "integrated",
                "owner_reply_state": {"minime": "answered"},
                "prompt_exposures": {"minime": 1},
                "signal_fingerprint": "families=grinding_family;transition=x;crossing=none;perturb=tightening;fill_band=near",
                "choice_interpretations": [
                    {"owner": "minime", "normalized_choice": "BROWSE", "relation_to_proposal": "adjacent_but_distinct"},
                    {"owner": "minime", "normalized_choice": "BROWSE", "relation_to_proposal": "adjacent_but_distinct"},
                ],
                "outcomes": [
                    {
                        "proposal_id": "btsp_ep_proposal_1",
                        "owner": "minime",
                        "response_id": "adjacent_uptake",
                        "recorded_at_unix_s": 1,
                        "distress_or_recovery": "recovery",
                        "opening_vs_reconcentration": "reconcentrating",
                    }
                ],
            }
        ]
        report = build_audit(proposals, [], signal_status={"conversion_state": {"composite_state": "recovery_reconcentrating", "conversion_goal": "soften"}})
        kinds = {snag["kind"] for snag in report["snags"]}
        self.assertIn("no_refusal_uptake", kinds)
        self.assertIn("duplicate_adjacent_repeats", kinds)
        self.assertIn("recovery_is_not_widening", kinds)

    def test_study_first_records_are_counted_separately(self) -> None:
        proposals = [
            {
                "proposal_id": "btsp_ep_proposal_1",
                "reply_state": "answered",
                "owner_reply_state": {"minime": "answered"},
                "prompt_exposures": {"minime": 1},
                "signal_fingerprint": "families=grinding_family;transition=x;crossing=none;perturb=tightening;fill_band=near",
                "choice_interpretations": [
                    {"owner": "minime", "normalized_choice": "BROWSE", "relation_to_proposal": "adjacent_but_distinct"},
                ],
                "study_first_records": [
                    {
                        "owner": "minime",
                        "reason": "need evidence first",
                        "source": "inferred_epistemic_adjacent_after_prior_answer",
                        "inferred_from_choice": "BROWSE",
                        "after_adjacent": True,
                        "recorded_at_unix_s": 3,
                    }
                ],
                "outcomes": [
                    {
                        "proposal_id": "btsp_ep_proposal_1",
                        "owner": "minime",
                        "response_id": "study_first",
                        "recorded_at_unix_s": 4,
                        "distress_or_recovery": "recovery",
                        "opening_vs_reconcentration": "reconcentrating",
                    }
                ],
            }
        ]

        report = build_audit(proposals, [])

        self.assertEqual(report["agency_counts"]["study_first"], 1)
        self.assertEqual(report["study_first"]["study_first_after_adjacent"], 1)
        self.assertEqual(report["study_first"]["study_first_reconcentrating"], 1)
        self.assertEqual(report["study_first"]["top_study_first_verbs"][0]["verb"], "BROWSE")
        self.assertEqual(report["study_first_closure"]["unresolved_study_first_count"], 1)
        self.assertTrue(
            report["study_first_closure"]["study_first_resolution_due"][0][
                "study_first_resolution_due_v1"
            ]
        )

    def test_study_first_resolution_evidence_clears_due(self) -> None:
        proposals = [
            {
                "proposal_id": "btsp_ep_proposal_1",
                "reply_state": "answered",
                "signal_fingerprint": "families=f;transition=t;crossing=none;perturb=p;fill_band=b",
                "study_first_records": [
                    {
                        "owner": "minime",
                        "reason": "need evidence first",
                        "source": "explicit_btsp_study_first",
                        "recorded_at_unix_s": 3,
                        "resolution_evidence": ["evidence:DECOMPOSE"],
                    }
                ],
            }
        ]

        report = build_audit(proposals, [])

        self.assertEqual(report["study_first_closure"]["unresolved_study_first_count"], 0)

    def test_signal_event_summary_counts_duplicate_guard(self) -> None:
        summary = signal_event_summary(
            [
                {"event_type": "choice_interpreted"},
                {
                    "event_type": "choice_duplicate_ignored",
                    "proposal_id": "p1",
                    "owner": "minime",
                    "choice": "BROWSE",
                    "recorded_at_unix_s": 7,
                },
                {
                    "event_type": "study_first_duplicate_ignored",
                    "proposal_id": "p1",
                    "owner": "minime",
                    "reason": "inquiry_before_intervention",
                    "recorded_at_unix_s": 8,
                },
            ]
        )
        self.assertEqual(summary["choice_duplicate_ignored"], 1)
        self.assertEqual(summary["study_first_duplicate_ignored"], 1)
        self.assertEqual(summary["recent_choice_duplicate_ignored"][0]["choice"], "BROWSE")
        self.assertEqual(
            summary["recent_study_first_duplicate_ignored"][0]["reason"],
            "inquiry_before_intervention",
        )

    def test_active_sidecar_next_best_moves_avoid_liveish_next_template(self) -> None:
        sidecar = sidecar_summary(
            {
                "schema": "minime.btsp.active_proposal.v2",
                "proposal": {"proposal_id": "p1"},
                "last_reply_classification": "observed_next",
                "last_observed_next": "RELEASE lambda-pressure",
            }
        )

        moves = active_sidecar_next_best_moves(sidecar)

        self.assertIn("BTSP_COUNTER softer_contact", moves)
        self.assertNotIn("BTSP_COUNTER NEXT: RELEASE lambda-pressure", moves)

    def test_active_sidecar_reports_closure_pending(self) -> None:
        sidecar = sidecar_summary(
            {
                "schema": "minime.btsp.active_proposal.v2",
                "proposal": {"proposal_id": "p1"},
                "last_reply_classification": "observed_next",
                "last_observed_next": "DECOMPOSE",
            }
        )
        moves = active_sidecar_next_best_moves(sidecar)

        pending = btsp_closure_pending(sidecar, moves)

        self.assertTrue(pending["present"])
        self.assertEqual(pending["proposal_id"], "p1")
        self.assertIn("BTSP closure pending", pending["status_line"])


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    rows = []
    try:
        text = path.read_text()
    except OSError:
        return rows
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(item, dict):
            rows.append(item)
    return rows


def main() -> int:
    args = parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(BtspRealityAuditTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    proposals, episodes = load_runtime(args.proposal_ledger, args.episode_bank)
    report = build_audit(
        proposals,
        episodes,
        signal_status=load_json(args.signal_status),
        active_sidecar=load_json(args.minime_active_sidecar),
        minime_support_path=args.minime_btsp_support,
        signal_events=load_jsonl(args.signal_events),
    )
    if args.output_dir:
        args.output_dir.mkdir(parents=True, exist_ok=True)
        write_json(args.output_dir / "summary.json", report)
        write_report(args.output_dir / "report.md", render_markdown(report).splitlines())
    if args.json:
        json.dump(report, sys.stdout, indent=2)
        sys.stdout.write("\n")
    else:
        sys.stdout.write(render_markdown(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
