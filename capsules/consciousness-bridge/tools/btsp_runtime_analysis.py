#!/usr/bin/env python3
"""
Shared helpers for steward-first BTSP runtime diagnostics.
"""

from __future__ import annotations

import json
from collections import Counter
from datetime import datetime
from pathlib import Path
from typing import Any


BRIDGE_WORKSPACE = Path(
    "/Users/v/other/astrid/capsules/consciousness-bridge/workspace"
)
DEFAULT_PROPOSAL_LEDGER = BRIDGE_WORKSPACE / "sovereignty_proposals.json"
DEFAULT_EPISODE_BANK = BRIDGE_WORKSPACE / "btsp_episode_bank.json"
DEFAULT_SIGNAL_EVENTS = BRIDGE_WORKSPACE / "btsp_signal_events.jsonl"
OWNER_ASTRID = "astrid"
OWNER_MINIME = "minime"


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text())
    except Exception:
        return {}


def write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=False))


def load_runtime(
    proposal_path: Path = DEFAULT_PROPOSAL_LEDGER,
    episode_bank_path: Path = DEFAULT_EPISODE_BANK,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    proposal_ledger = load_json(proposal_path)
    episode_bank = load_json(episode_bank_path)
    proposals = [
        proposal
        for proposal in proposal_ledger.get("proposals", [])
        if isinstance(proposal, dict)
    ]
    episodes = [
        episode
        for episode in episode_bank.get("episodes", [])
        if isinstance(episode, dict)
    ]
    return proposals, episodes


def load_signal_events(path: Path = DEFAULT_SIGNAL_EVENTS) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    events: list[dict[str, Any]] = []
    for line in path.read_text().splitlines():
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except Exception:
            continue
        if isinstance(payload, dict):
            events.append(payload)
    return events


def parse_signal_fingerprint(fingerprint: str) -> dict[str, str]:
    parsed = {
        "families": "",
        "transition": "unknown",
        "crossing": "none",
        "perturb": "unknown",
        "fill_band": "unknown",
    }
    if not fingerprint:
        return parsed
    for part in fingerprint.split(";"):
        if "=" not in part:
            continue
        key, value = part.split("=", 1)
        parsed[key] = value
    return parsed


def derive_signal_fingerprint(proposal: dict[str, Any]) -> str:
    fingerprint = str(proposal.get("signal_fingerprint", "")).strip()
    if fingerprint:
        return fingerprint
    families = sorted(
        {
            str(item)
            for item in proposal.get("matched_signal_families", [])
            if str(item).strip()
        }
    )
    transition = "unknown"
    crossing = "none"
    perturb = "unknown"
    fill_band = "unknown"
    for signal in proposal.get("matched_live_signals", []) or []:
        value = str(signal)
        if value.startswith("phase_transition:"):
            transition = normalize_component(value.split(":", 1)[1])
        elif value.startswith("fill_band_crossing:"):
            crossing = normalize_component(value.split(":", 1)[1])
            fill_band = crossing
        elif value.startswith("perturb_visibility:"):
            perturb = normalize_component(value.split(":", 1)[1])
    return (
        f"families={'+'.join(families)};"
        f"transition={transition};crossing={crossing};"
        f"perturb={perturb};fill_band={fill_band}"
    )


def normalize_component(value: str) -> str:
    return value.strip().lower().replace(" -> ", "->").replace(" ", "_")


def proposal_outcomes(
    proposal: dict[str, Any], episodes: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    outcomes = [
        outcome
        for outcome in proposal.get("outcomes", [])
        if isinstance(outcome, dict)
    ]
    if outcomes:
        return sorted(
            outcomes,
            key=lambda outcome: int(outcome.get("recorded_at_unix_s", 0) or 0),
        )
    proposal_id = str(proposal.get("proposal_id", ""))
    fallback: list[dict[str, Any]] = []
    for episode in episodes:
        for outcome in episode.get("response_outcomes", []):
            if not isinstance(outcome, dict):
                continue
            if str(outcome.get("proposal_id", "")) == proposal_id:
                fallback.append(outcome)
    return sorted(
        fallback,
        key=lambda outcome: int(outcome.get("recorded_at_unix_s", 0) or 0),
    )


def first_future_outcome(
    proposal: dict[str, Any],
    episodes: list[dict[str, Any]],
    *,
    after_unix_s: int,
    owner: str | None = None,
    response_id: str | None = None,
) -> dict[str, Any] | None:
    for outcome in proposal_outcomes(proposal, episodes):
        recorded = int(outcome.get("recorded_at_unix_s", 0) or 0)
        if recorded < after_unix_s:
            continue
        if owner is not None and str(outcome.get("owner", "")) != owner:
            continue
        if response_id is not None and str(outcome.get("response_id", "")) != response_id:
            continue
        return outcome
    return None


def latency_minutes(start_unix_s: int, outcome: dict[str, Any] | None) -> float | None:
    if outcome is None:
        return None
    end_unix_s = int(outcome.get("recorded_at_unix_s", 0) or 0)
    if end_unix_s < start_unix_s:
        return None
    return round((end_unix_s - start_unix_s) / 60.0, 2)


def latency_bucket(minutes: float | None) -> str:
    if minutes is None:
        return "none"
    if minutes <= 1.0:
        return "<=1m"
    if minutes <= 3.0:
        return "<=3m"
    if minutes <= 10.0:
        return "<=10m"
    return ">10m"


def format_pct(numerator: int, denominator: int) -> str:
    if denominator <= 0:
        return "0.0%"
    return f"{(100.0 * numerator / denominator):.1f}%"


def top_counter_rows(counter: Counter[str], limit: int = 5) -> list[dict[str, Any]]:
    return [
        {"name": name, "count": count}
        for name, count in counter.most_common(limit)
    ]


def owner_exact_adoptions(proposal: dict[str, Any], owner: str) -> list[dict[str, Any]]:
    return [
        adoption
        for adoption in proposal.get("exact_adoptions", [])
        if isinstance(adoption, dict) and str(adoption.get("owner", "")) == owner
    ]


def owner_choice_interpretations(
    proposal: dict[str, Any], owner: str
) -> list[dict[str, Any]]:
    return [
        choice
        for choice in proposal.get("choice_interpretations", [])
        if isinstance(choice, dict) and str(choice.get("owner", "")) == owner
    ]


def proposal_seen_by_owner(proposal: dict[str, Any], owner: str) -> bool:
    exposures = proposal.get("prompt_exposures", {})
    if not isinstance(exposures, dict):
        return False
    return int(exposures.get(owner, 0) or 0) > 0


def is_real_runtime_proposal(proposal: dict[str, Any]) -> bool:
    proposal_id = str(proposal.get("proposal_id", "")).strip()
    return "_proposal_" in proposal_id


def is_resolved_proposal(proposal: dict[str, Any]) -> bool:
    reply_state = str(proposal.get("reply_state", "")).strip()
    outcome_status = str(proposal.get("outcome_status", "")).strip()
    return reply_state in {"declined", "expired", "integrated"} or outcome_status == "integrated"


def iso_now() -> str:
    return datetime.now().isoformat(timespec="seconds")


def response_label(response_id: str) -> str:
    return {
        "minime_notice_first": "NOTICE",
        "minime_recover_regime": "recover",
        "minime_semantic_probe": "semantic probe",
        "astrid_dampen": "DAMPEN",
        "astrid_breathe_alone": "BREATHE_ALONE",
        "astrid_echo_off": "ECHO_OFF",
        "continue_current_course": "continue current course",
        "proposal_expired": "proposal expired",
    }.get(response_id, response_id)


def write_report(path: Path, lines: list[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines).rstrip() + "\n")
