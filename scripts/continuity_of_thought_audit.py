#!/usr/bin/env python3
"""Read-only continuity-of-thought audit for Astrid and Minime action threads."""

from __future__ import annotations

import argparse
import json
import re
import sys
import tempfile
import unittest
from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any


ASTRID_WORKSPACE = Path("/Users/v/other/astrid/capsules/consciousness-bridge/workspace")
MINIME_WORKSPACE = Path("/Users/v/other/minime/workspace")
ASTRID_SOURCE_ROOT = Path("/Users/v/other/astrid/capsules/consciousness-bridge/src")
MINIME_SOURCE = Path("/Users/v/other/minime/autonomous_agent.py")

RUNNING_STATUSES = {"running", "llm_running", "queued", "pending"}
BLOCKED_LIKE_STATUSES = {"blocked", "no_effect", "rehearsal_blocked", "failed"}
RUN_AFTER_CHARTER_STATUSES = {"handled", "rehearsed", "observed", "evidence_recorded"}
RETURN_VERBS = {
    "ACTION_PREFLIGHT",
    "EXAMINE",
    "SHADOW_PREFLIGHT",
    "EXPERIMENT_PLAN",
    "EXPERIMENT_CHARTER",
    "EXPERIMENT_REHEARSE",
    "EXPERIMENT_EVIDENCE",
    "EXPERIMENT_DECIDE",
    "EXPERIMENT_BIND",
}
WIRE_PATTERNS = [
    re.compile(r"""(?:if|elif)\s+base\s*==\s*['"]([A-Z_][A-Z0-9_]*)['"]\s*:"""),
    re.compile(r"""case\s+['"]([A-Z_][A-Z0-9_]*)['"]\s*:"""),
    re.compile(r"""^\s*['"]([A-Z_][A-Z0-9_]+)['"]\s*:\s*(?:['"][a-z_]+['"]|None)\s*,?\s*$"""),
    re.compile(r"""['"]([A-Z_][A-Z0-9_]*)['"]\s*\|"""),
    re.compile(r"""\|\s*['"]([A-Z_][A-Z0-9_]*)['"]"""),
]
WIRE_SET_PATTERN = re.compile(r"""base(?:_action)?\s+in\s+(?:\{|\()(?P<body>[^})]+)(?:\}|\))""")
ACTION_LITERAL_PATTERN = re.compile(r"""['"]([A-Z_][A-Z0-9_]*)['"]""")


def read_json(path: Path, default: Any = None) -> Any:
    try:
        return json.loads(path.read_text())
    except Exception:
        return default


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    rows: list[dict[str, Any]] = []
    for line in path.read_text().splitlines():
        try:
            value = json.loads(line)
        except Exception:
            continue
        if isinstance(value, dict):
            rows.append(value)
    return rows


def parse_time(value: Any) -> datetime | None:
    if not isinstance(value, str) or not value:
        return None
    text = value.replace("Z", "+00:00")
    try:
        parsed = datetime.fromisoformat(text)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def base_action(action: Any) -> str:
    text = str(action or "").strip()
    if not text:
        return ""
    token = text.split(None, 1)[0].strip("`*[](){}<>").strip(".,;:")
    token = re.sub(r"^[^\w]+|[^\w]+$", "", token)
    return token.upper()


def meaningful_text(value: Any) -> bool:
    text = str(value or "").strip()
    if not text:
        return False
    lowered = text.casefold()
    if lowered in {"<structured prose>", "<felt note>", "<reason>", "<note>", "..."}:
        return False
    return not (text.startswith("<") and text.endswith(">"))


def meaningful_list(value: Any) -> bool:
    return isinstance(value, list) and any(meaningful_text(item) for item in value)


def valid_charter(charter: Any) -> bool:
    if not isinstance(charter, dict):
        return False
    return bool(
        meaningful_text(charter.get("hypothesis"))
        or meaningful_text(charter.get("method_intent"))
        or meaningful_text(charter.get("proposed_next_action"))
        or meaningful_list(charter.get("evidence_targets"))
        or meaningful_list(charter.get("stop_criteria"))
    )


def charter_quality_v1(charter: Any) -> dict[str, Any]:
    charter = charter if isinstance(charter, dict) else {}
    missing: list[str] = []
    if not meaningful_text(charter.get("hypothesis")):
        missing.append("hypothesis")
    if not meaningful_text(charter.get("proposed_next_action")):
        missing.append("proposed_next_action")
    if not meaningful_list(charter.get("evidence_targets")):
        missing.append("evidence_targets")
    lifecycle_valid = not missing
    return {
        "schema_version": 1,
        "lifecycle_valid": lifecycle_valid,
        "missing_fields": missing,
        "repair_required": not lifecycle_valid,
    }


def lifecycle_valid_charter(charter: Any) -> bool:
    return valid_charter(charter) and bool(charter_quality_v1(charter).get("lifecycle_valid"))


def charter_proposed_next_action(charter: Any) -> str:
    if not lifecycle_valid_charter(charter) or not isinstance(charter, dict):
        return ""
    return str(charter.get("proposed_next_action") or "").strip()


def evidence_counts(evidence: Any) -> dict[str, int]:
    evidence = evidence if isinstance(evidence, dict) else {}
    return {
        "felt": len(evidence.get("felt_observations") or []),
        "telemetry": len(evidence.get("telemetry_snapshots") or []),
        "artifacts": len(evidence.get("artifact_refs") or []),
        "decisions": len(evidence.get("decisions") or []),
    }


def evidence_meaningful(evidence: Any) -> bool:
    if not isinstance(evidence, dict):
        return False
    felt = evidence.get("felt_observations")
    if isinstance(felt, list):
        for item in felt:
            if isinstance(item, dict) and (
                meaningful_text(item.get("note"))
                or meaningful_text(item.get("felt"))
                or meaningful_text(item.get("summary"))
            ):
                return True
    telemetry = evidence.get("telemetry_snapshots")
    if isinstance(telemetry, list) and telemetry:
        return True
    artifacts = evidence.get("artifact_refs")
    if isinstance(artifacts, list) and artifacts:
        return True
    return False


def latest_by_id(rows: list[dict[str, Any]], key: str) -> dict[str, dict[str, Any]]:
    latest: dict[str, dict[str, Any]] = {}
    for row in rows:
        identifier = row.get(key)
        if isinstance(identifier, str) and identifier:
            latest[identifier] = row
    return latest


def collapse_events_by_action(events: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Keep only the newest row for each action_id-like key."""
    rows: list[dict[str, Any]] = []
    seen: set[str] = set()
    for event in reversed(events):
        key = event.get("action_id") or (
            f"{event.get('started_at', '')}:"
            f"{event.get('canonical_action', '')}:"
            f"{event.get('effective_action', '')}"
        )
        if key in seen:
            continue
        seen.add(str(key))
        rows.append(event)
    return list(reversed(rows))


def classify_experiment(experiment: dict[str, Any], runs: list[dict[str, Any]]) -> str:
    recent = list(reversed(runs))[:4]
    blocked_like = sum(1 for run in recent if run.get("status") in BLOCKED_LIKE_STATUSES)
    if blocked_like >= 2:
        return "blocked_loop"
    if not lifecycle_valid_charter(experiment.get("charter_v1")):
        return "needs_charter"
    if evidence_meaningful(experiment.get("evidence_v1")):
        return "needs_decision"
    if any(run.get("status") in RUN_AFTER_CHARTER_STATUSES for run in runs):
        return "needs_evidence"
    if base_action(experiment.get("planned_next")) == "EXPERIMENT_PLAN":
        return "fragmented"
    if lifecycle_valid_charter(experiment.get("charter_v1")):
        return "needs_rehearsal"
    return "returnable"


def continuity_return_for(
    experiment: dict[str, Any],
    runs: list[dict[str, Any]],
    label: str = "",
) -> str:
    classification = classify_experiment(experiment, runs)
    being = label.casefold()
    if being == "astrid":
        charter_payload = (
            "EXPERIMENT_CHARTER current :: hypothesis: ...; method_intent: felt texture + motif continuity; "
            "proposed_next_action: ACTION_PREFLIGHT ...; evidence_targets: felt_texture, motif_continuity, language_thread, artifact_grounding; "
            "stop_criteria: ..."
        )
        evidence_payload = (
            "EXPERIMENT_EVIDENCE current :: felt_texture ...; motif_continuity ...; "
            "language_thread ...; artifact_grounding ..."
        )
    elif being == "minime":
        charter_payload = (
            "EXPERIMENT_CHARTER current :: hypothesis: ...; method_intent: spectral/state condition + recurrence; "
            "proposed_next_action: ACTION_PREFLIGHT ...; evidence_targets: spectral_condition, fill_pressure_state, recurrence_pattern, artifact_grounding; "
            "stop_criteria: ..."
        )
        evidence_payload = (
            "EXPERIMENT_EVIDENCE current :: spectral_condition ...; fill_pressure_state ...; "
            "recurrence_pattern ...; artifact_grounding ..."
        )
    else:
        charter_payload = (
            "EXPERIMENT_CHARTER current :: hypothesis: ...; method_intent: ...; "
            "proposed_next_action: ACTION_PREFLIGHT ...; evidence_targets: felt, telemetry, artifact; "
            "stop_criteria: ..."
        )
        evidence_payload = "EXPERIMENT_EVIDENCE current :: felt ...; telemetry ...; artifact ..."
    if classification == "blocked_loop":
        return "EXPERIMENT_DECIDE current :: counter NEXT: ACTION_PREFLIGHT DECOMPOSE"
    if classification in {"needs_charter", "fragmented"}:
        return charter_payload
    if classification == "needs_decision":
        return "EXPERIMENT_DECIDE current :: pause because evidence is ready to interpret"
    if classification == "needs_evidence":
        return evidence_payload
    if classification == "needs_rehearsal":
        return "EXPERIMENT_REHEARSE current"
    return experiment.get("planned_next") or "EXPERIMENT_PLAN current"


def normalize_signal_text(value: Any) -> str:
    return (
        str(value or "")
        .casefold()
        .replace("λ", "lambda")
        .replace("₁", "1")
        .replace("₂", "2")
        .replace("₃", "3")
        .replace("₄", "4")
        .replace("–", "-")
        .replace("—", "-")
    )


def counteroffered_next(value: Any) -> str:
    text = str(value or "")
    match = re.search(r"counter\s+NEXT\s*:\s*(.+)", text, flags=re.IGNORECASE)
    if not match:
        return ""
    return match.group(1).splitlines()[0].strip().strip("`")


def preferred_charter_scaffold_next(experiment: dict[str, Any], runs: list[dict[str, Any]]) -> str:
    counter_next = counteroffered_next(experiment.get("planned_next"))
    if counter_next:
        return counter_next
    candidates = experiment.get("workbench_candidates_v1")
    if isinstance(candidates, dict):
        charter = candidates.get("charter")
        if isinstance(charter, dict):
            proposed = str(charter.get("proposed_next_action") or "").strip()
            if proposed:
                return proposed
    for run in reversed(runs or []):
        action = str(run.get("action_text") or "").strip()
        if action and base_action(action) not in {"BROWSE", "SEARCH", "READ_MORE", "LOOK"}:
            return action
    return "ACTION_PREFLIGHT DECOMPOSE"


def charter_scaffold_v1(
    label: str,
    thread: dict[str, Any] | None,
    experiment: dict[str, Any],
    runs: list[dict[str, Any]],
    classification: str | None = None,
) -> dict[str, Any] | None:
    if label.casefold() != "minime":
        return None
    classification = classification or classify_experiment(experiment, runs)
    if classification != "needs_charter":
        return None
    proposed_next = preferred_charter_scaffold_next(experiment, runs)
    title = str(experiment.get("title") or "current experiment").strip()
    question = str(experiment.get("question") or "").strip()
    signal_text = normalize_signal_text(f"{title} {question}")
    evidence_targets = [
        "spectral_condition",
        "fill_pressure_state",
        "recurrence_pattern",
        "artifact_grounding",
    ]
    if "gap" in signal_text and any(
        term in signal_text
        for term in ("spect", "spectra", "spectral", "density", "lambda", "mode")
    ):
        hypothesis = (
            "localized λ1 spectral-density softening near the dominant mode may reduce "
            "mode-packing/lambda-monopoly and support controlled branching without premature "
            "λ4 dominance or runaway dispersal"
        )
        method_intent = (
            "rehearse ACTION_PREFLIGHT DECOMPOSE and compare pressure/resonance telemetry before and after"
        )
        proposed_next = "ACTION_PREFLIGHT DECOMPOSE"
        stop_criteria = (
            "pressure_risk rises above baseline, fill leaves the comfort band, "
            "λ4/entropy shows runaway dispersal, or repeated research stops adding evidence"
        )
    else:
        hypothesis = (
            f"{title or 'this experiment'} may become returnable by comparing the current spectral "
            "condition, pressure state, recurrence pattern, and artifacts without adding live authority"
        )
        method_intent = f"rehearse {proposed_next} and compare pressure/resonance telemetry before and after"
        stop_criteria = (
            "pressure_risk rises above baseline, fill leaves the comfort band, runaway dispersal appears, "
            "or repeated research stops adding evidence"
        )
    command = (
        "EXPERIMENT_CHARTER current :: "
        f"hypothesis: {hypothesis}; "
        f"method_intent: {method_intent}; "
        f"proposed_next_action: {proposed_next}; "
        f"evidence_targets: {', '.join(evidence_targets)}; "
        f"stop_criteria: {stop_criteria}; "
        "consent_posture: advisory; ordinary choices remain valid."
    )
    thread = thread if isinstance(thread, dict) else {}
    return {
        "schema_version": 1,
        "source": "continuity_projection",
        "status": "scaffold_only",
        "authoring_required": True,
        "authority_change": False,
        "command": command,
        "proposed_next_action": proposed_next,
        "evidence_targets": evidence_targets,
        "native_register": "minime_spectral_state",
        "thread_id": thread.get("thread_id"),
        "experiment_id": experiment.get("experiment_id"),
    }


def directed_shift_matches(value: Any) -> list[str]:
    normalized = normalize_signal_text(value)
    matches: list[str] = []
    for phrase in [
        "directed shift",
        "initiate shift",
        "localized dispersal",
        "reciprocal shadow-trace",
    ]:
        if phrase in normalized:
            matches.append(phrase)
    if (
        "centered on lambda4" in normalized
        or "centered on lambda 4" in normalized
        or "centered on lambda2" in normalized
        or "centered on lambda 2" in normalized
    ):
        matches.append("centered on lambda")
    if ("lambda" in normalized or "shadow" in normalized) and (
        "steer" in normalized or "steering" in normalized
    ):
        matches.append("steer/steering near lambda/shadow")
    if "lambda" in normalized or "shadow" in normalized:
        for needle, label in [
            ("guiding", "guiding near lambda/shadow"),
            ("actively shaping", "actively shaping near lambda/shadow"),
            ("controlled distortion", "controlled distortion near lambda/shadow"),
            ("deliberate narrowing", "deliberate narrowing near lambda/shadow"),
            ("let lambda4 become", "let lambda4 become"),
            ("let lambda 4 become", "let lambda4 become"),
        ]:
            if needle in normalized and label not in matches:
                matches.append(label)
    return matches


def preflight_safety_cue_v1(
    label: str,
    thread: dict[str, Any] | None,
    experiment: dict[str, Any] | None,
    recent_events: list[dict[str, Any]],
) -> dict[str, Any] | None:
    if label.casefold() != "astrid":
        return None
    thread = thread if isinstance(thread, dict) else {}
    experiment = experiment if isinstance(experiment, dict) else {}
    inspect = [
        thread.get("current_next"),
        thread.get("why_return"),
        experiment.get("title"),
        experiment.get("question"),
        experiment.get("planned_next"),
        experiment.get("candidate_status"),
    ]
    for event in list(reversed(recent_events))[:5]:
        inspect.extend([
            event.get("raw_next"),
            event.get("canonical_action"),
            event.get("effective_action"),
            event.get("outcome_summary"),
        ])
    matched: list[str] = []
    for text in inspect:
        for item in directed_shift_matches(text):
            if item not in matched:
                matched.append(item)
    if not matched:
        return None
    return {
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": True,
        "authority_change": False,
        "matched_terms": matched,
        "suggested_next": "SHADOW_PREFLIGHT lambda-tail/lambda4 --stage=rehearse or ACTION_PREFLIGHT DECOMPOSE",
        "cue": (
            "Directed-shift cue: keep this in rehearsal/preflight. Suggested NEXT: "
            "SHADOW_PREFLIGHT lambda-tail/lambda4 --stage=rehearse or ACTION_PREFLIGHT DECOMPOSE."
        ),
    }


READ_ONLY_CONTROL_INTENT_BASES = {
    "EXAMINE",
    "EXAMINE_CASCADE",
    "TRACE",
    "DECOMPOSE",
    "SPECTRAL_EXPLORER",
}


def read_only_control_intent_matches(value: Any) -> list[str]:
    normalized = normalize_signal_text(value)
    near_context = any(
        term in normalized
        for term in ("lambda", "shadow", "parameter", "eigen", "spectral")
    )
    matches: list[str] = []
    for needle, label, needs_context in [
        ("[control]", "[control]", False),
        ("active parameter glyphs", "active parameter glyphs", False),
        ("delta_lambda", "delta_lambda", False),
        ("delta lambda", "delta_lambda", False),
        ("epsilon=", "epsilon parameter", False),
        ("how to influence", "influence intent", True),
        ("influence its spread", "influence spread", True),
        ("influence it's spread", "influence spread", True),
        ("influence the spread", "influence spread", True),
    ]:
        if needle in normalized and (not needs_context or near_context) and label not in matches:
            matches.append(label)
    return matches


def read_only_control_intent_cue_v1(
    label: str,
    thread: dict[str, Any] | None,
    classification: str,
) -> dict[str, Any] | None:
    if label.casefold() != "astrid" or classification != "needs_charter":
        return None
    thread = thread if isinstance(thread, dict) else {}
    current_next = str(thread.get("current_next") or "")
    if base_action(current_next) not in READ_ONLY_CONTROL_INTENT_BASES:
        return None
    matched = read_only_control_intent_matches(current_next)
    if not matched:
        return None
    return {
        "schema_version": 1,
        "source": "continuity_projection",
        "advisory_only": True,
        "authority_change": False,
        "matched_terms": matched,
        "suggested_next": "EXPERIMENT_CHARTER current :: ... or ACTION_PREFLIGHT <read-only focus>",
        "cue": (
            "Read-only control cue: keep this observational while the charter is missing. "
            "Author a charter or preflight before influence/control intent."
        ),
    }


def read_only_control_intent_check_v1(
    label: str,
    thread: dict[str, Any] | None,
    classification: str,
    cue: dict[str, Any] | None = None,
) -> dict[str, Any] | None:
    if label.casefold() != "astrid":
        return None
    thread = thread if isinstance(thread, dict) else {}
    current_next = str(thread.get("current_next") or "")
    current_base = base_action(current_next)
    eligible_base = current_base in READ_ONLY_CONTROL_INTENT_BASES
    matched = read_only_control_intent_matches(current_next) if eligible_base else []
    cue_rendered = isinstance(cue, dict) and bool(cue.get("cue"))
    cue_expected = classification == "needs_charter" and eligible_base and bool(matched)
    if cue_rendered:
        status = "cue_rendered"
        reason = "control-shaped read-only language matched and the advisory cue rendered"
    elif cue_expected:
        status = "cue_missing"
        reason = "control-shaped read-only language matched but no advisory cue rendered"
    elif classification != "needs_charter":
        status = "not_applicable"
        reason = "active experiment is not needs_charter"
    elif not eligible_base:
        status = "not_applicable"
        reason = "current NEXT is not a read-only control-cue action"
    else:
        status = "no_trigger"
        reason = "current NEXT is read-only but has no control-shaped trigger language"
    return {
        "schema_version": 1,
        "source": "continuity_audit",
        "status": status,
        "reason": reason,
        "current_next_base": current_base,
        "eligible_read_only_base": eligible_base,
        "matched_terms": matched,
        "cue_expected": cue_expected,
        "cue_rendered": cue_rendered,
    }


def fragmentation_summary(experiments: list[dict[str, Any]]) -> dict[str, Any]:
    active = [exp for exp in experiments if exp.get("status", "active") == "active"]
    plan_stage = [
        exp for exp in active if base_action(exp.get("planned_next")) == "EXPERIMENT_PLAN"
    ]
    ratio = len(plan_stage) / len(active) if active else 0.0
    return {
        "active_experiments": len(active),
        "plan_stage_active": len(plan_stage),
        "fragmented": len(plan_stage) >= 8 and ratio >= 0.6,
        "plan_stage_ratio": round(ratio, 3),
    }


def suggest_route(verb: str) -> str:
    verb = verb.upper()
    if verb.startswith("INVESTIGATE") or verb.startswith("EXPLORE"):
        return "EXAMINE <target> or EXPERIMENT_PLAN current"
    if verb.startswith("INJECT") or verb in {"PERTURB", "DELIVER", "RUN"}:
        return "ACTION_PREFLIGHT <proposed action>"
    if verb.startswith("SHADOW"):
        return "SHADOW_PREFLIGHT <shadow action>"
    if verb.startswith("EXEXPERIMENT") or verb.startswith("EXPERIENCE"):
        return "EXPERIMENT_PLAN current"
    if verb.startswith("EXPERIMENT"):
        return "EXPERIMENT_PLAN current"
    return "ACTION_PREFLIGHT <proposed action>"


def scan_wired_actions(label: str) -> set[str]:
    if label.lower() == "minime":
        paths = [MINIME_SOURCE] if MINIME_SOURCE.exists() else []
    else:
        paths = [
            path
            for path in [
                ASTRID_SOURCE_ROOT / "action_continuity.rs",
                ASTRID_SOURCE_ROOT / "autonomous" / "next_action.rs",
                ASTRID_SOURCE_ROOT / "autonomous" / "next_action" / "shadow.rs",
                ASTRID_SOURCE_ROOT / "autonomous" / "next_action" / "operations.rs",
            ]
            if path.exists()
        ]
    wired: set[str] = set()
    for path in paths:
        source = path.read_text(errors="replace")
        for line in source.splitlines():
            for pattern in WIRE_PATTERNS:
                for match in pattern.finditer(line):
                    wired.add(match.group(1))
            for match in WIRE_SET_PATTERN.finditer(line):
                for literal in ACTION_LITERAL_PATTERN.finditer(match.group("body")):
                    wired.add(literal.group(1))
    return wired


def collect_unwired(
    label: str,
    workspace: Path,
    events_by_thread: dict[str, list[dict[str, Any]]],
    now: datetime,
) -> list[dict[str, Any]]:
    counts: Counter[str] = Counter()
    first_seen: dict[str, datetime] = {}
    latest_seen: dict[str, datetime] = {}
    wired = scan_wired_actions(label)

    def observe(verb: str, stamp: datetime | None) -> None:
        counts[verb] += 1
        if stamp is None:
            return
        first_seen[verb] = min(first_seen.get(verb, stamp), stamp)
        latest_seen[verb] = max(latest_seen.get(verb, stamp), stamp)

    proposals = read_jsonl(workspace / "action_threads" / "proposals.jsonl")
    for proposal in proposals:
        verb = base_action(proposal.get("action") or proposal.get("raw_action"))
        if re.match(r"^[A-Z]+-SUGG-\d+$", verb):
            continue
        if verb and verb not in RETURN_VERBS:
            observe(verb, parse_time(proposal.get("created_at")))
    for events in events_by_thread.values():
        for event in events:
            status = str(event.get("status") or "")
            route = str(event.get("route") or "")
            if "unwired" not in status and "unwired" not in route:
                continue
            verb = base_action(event.get("canonical_action") or event.get("raw_action"))
            if re.match(r"^[A-Z]+-SUGG-\d+$", verb):
                continue
            if verb and verb not in RETURN_VERBS:
                observe(verb, parse_time(event.get("ended_at") or event.get("started_at") or event.get("created_at")))
    return [
        {
            "verb": verb,
            "count": count,
            "currently_wired": verb in wired,
            "historical_only": bool(
                latest_seen.get(verb) and (now - latest_seen[verb]).total_seconds() > 86400
            ),
            "first_seen_at": first_seen.get(verb).isoformat() if first_seen.get(verb) else None,
            "latest_seen_at": latest_seen.get(verb).isoformat() if latest_seen.get(verb) else None,
            "suggested_route": suggest_route(verb),
        }
        for verb, count in counts.most_common(12)
    ]


def native_continuity(
    label: str,
    thread: dict[str, Any] | None,
    experiment: dict[str, Any] | None,
    runs: list[dict[str, Any]],
) -> dict[str, Any]:
    thread = thread if isinstance(thread, dict) else {}
    experiment = experiment if isinstance(experiment, dict) else {}
    evidence = experiment.get("evidence_v1") if isinstance(experiment.get("evidence_v1"), dict) else {}
    artifact_count = len(evidence.get("artifact_refs") or []) + sum(
        len(run.get("artifacts") or []) for run in runs if isinstance(run, dict)
    )
    if label.casefold() == "astrid":
        motif = experiment.get("motif_allowance_v1") or thread.get("motif_allowance_v1") or {}
        motif = motif if isinstance(motif, dict) else {}
        felt_count = len(evidence.get("felt_observations") or [])
        dominant = motif.get("dominant_motif") or "open inquiry"
        quality = motif.get("quality") or "open_basin"
        language_present = bool(
            str(experiment.get("title") or "").strip()
            or str(experiment.get("question") or "").strip()
            or str(experiment.get("planned_next") or "").strip()
            or str(thread.get("why_return") or "").strip()
        )
        return {
            "schema_version": 1,
            "native_register": "astrid_motif_language",
            "native_return_cue": (
                f"Astrid native return: name felt texture, motif continuity ({dominant}), "
                "language thread, and artifact grounding."
            ),
            "evidence_lanes": {
                "felt_texture": {"status": "present" if felt_count else "missing", "count": felt_count},
                "motif_continuity": {
                    "status": "present" if dominant != "open inquiry" or quality != "open_basin" else "missing",
                    "dominant_motif": dominant,
                    "quality": quality,
                },
                "language_thread": {"status": "present" if language_present else "missing"},
                "artifact_grounding": {"status": "present" if artifact_count else "missing", "count": artifact_count},
            },
        }
    resonance = thread.get("thread_resonance_density_v1") or {}
    pressure = thread.get("thread_pressure_source_v1") or {}
    resonance = resonance if isinstance(resonance, dict) else {}
    pressure = pressure if isinstance(pressure, dict) else {}
    telemetry_count = len(evidence.get("telemetry_snapshots") or [])
    actions = Counter(base_action(run.get("action_text")) for run in runs if base_action(run.get("action_text")))
    recurrence_count = max(actions.values(), default=0)
    return {
        "schema_version": 1,
        "native_register": "minime_spectral_state",
        "native_return_cue": (
            "Minime native return: state spectral condition, fill/pressure state, "
            "recurrence pattern, and artifact grounding."
        ),
        "evidence_lanes": {
            "spectral_condition": {
                "status": "present" if resonance or telemetry_count else "missing",
                "quality": resonance.get("quality", "unknown"),
                "telemetry_snapshots": telemetry_count,
            },
            "fill_pressure_state": {
                "status": "present" if pressure or telemetry_count else "missing",
                "quality": pressure.get("quality", "unknown"),
                "dominant_source": pressure.get("dominant_source", "unknown"),
                "aggregate": pressure.get("aggregate"),
            },
            "recurrence_pattern": {
                "status": "present" if recurrence_count >= 2 else "missing",
                "max_repeated_action_count": recurrence_count,
                "dominant_action": actions.most_common(1)[0][0] if actions else None,
            },
            "artifact_grounding": {"status": "present" if artifact_count else "missing", "count": artifact_count},
        },
    }


def collect_normalization_signals(
    workspace: Path,
    events_by_thread: dict[str, list[dict[str, Any]]],
) -> list[dict[str, Any]]:
    signals: list[dict[str, Any]] = []
    for events in events_by_thread.values():
        for event in events:
            signal = event.get("normalization_signal_v1")
            if isinstance(signal, dict):
                signals.append({
                    "source": "event",
                    "raw_verb": signal.get("raw_verb"),
                    "normalized_verb": signal.get("normalized_verb"),
                    "reason": signal.get("reason"),
                    "authority_change": signal.get("authority_change", False),
                    "action_id": event.get("action_id"),
                    "created_at": event.get("ended_at") or event.get("started_at"),
                })
    for proposal in read_jsonl(workspace / "action_threads" / "proposals.jsonl"):
        signal = proposal.get("normalization_signal_v1")
        if isinstance(signal, dict):
            signals.append({
                "source": "proposal",
                "raw_verb": signal.get("raw_verb"),
                "normalized_verb": signal.get("normalized_verb"),
                "reason": signal.get("reason"),
                "authority_change": signal.get("authority_change", False),
                "created_at": proposal.get("created_at"),
            })
    return signals[-12:]


def terminal_event_summary(events: list[dict[str, Any]], limit: int = 8) -> list[dict[str, Any]]:
    terminal = [
        event for event in collapse_events_by_action(events)
        if event.get("status") not in RUNNING_STATUSES
    ]
    return [
        {
            "action_id": event.get("action_id"),
            "action": event.get("effective_action") or event.get("canonical_action"),
            "status": event.get("status"),
            "summary": event.get("outcome_summary") or event.get("result_summary") or "",
            "ended_at": event.get("ended_at") or event.get("finished_at") or event.get("started_at"),
        }
        for event in terminal[-limit:]
    ]


def stale_running_jobs(events: list[dict[str, Any]], now: datetime) -> list[dict[str, Any]]:
    stale: list[dict[str, Any]] = []
    for event in collapse_events_by_action(events):
        if event.get("status") not in {"llm_running", "running"}:
            continue
        started = parse_time(event.get("started_at") or event.get("created_at"))
        age_minutes = None
        if started:
            age_minutes = round((now - started).total_seconds() / 60.0, 1)
        if age_minutes is None or age_minutes >= 45:
            stale.append({
                "action_id": event.get("action_id"),
                "action": event.get("effective_action") or event.get("canonical_action"),
                "status": event.get("status"),
                "age_minutes": age_minutes,
            })
    return stale


def audit_workspace(label: str, workspace: Path) -> dict[str, Any]:
    root = workspace / "action_threads"
    index = read_json(root / "index.json", {}) or {}
    thread_dirs = sorted((root / "threads").glob("*")) if (root / "threads").exists() else []
    threads = [
        thread for path in thread_dirs
        if isinstance((thread := read_json(path / "thread.json", None)), dict)
    ]
    active_thread_id = index.get("active_thread_id")
    active_thread = next(
        (thread for thread in threads if thread.get("thread_id") == active_thread_id),
        threads[-1] if threads else None,
    )
    events_by_thread: dict[str, list[dict[str, Any]]] = {}
    runs_by_thread: dict[str, list[dict[str, Any]]] = {}
    experiments: list[dict[str, Any]] = []
    for thread in threads:
        thread_id = thread.get("thread_id")
        if not thread_id:
            continue
        tdir = root / "threads" / thread_id
        events_by_thread[thread_id] = read_jsonl(tdir / "events.jsonl")
        runs_by_thread[thread_id] = read_jsonl(tdir / "experiment_runs.jsonl")
        experiments.extend(latest_by_id(read_jsonl(tdir / "experiments.jsonl"), "experiment_id").values())

    status_counts = Counter(exp.get("status", "active") for exp in experiments)
    now = datetime.now(timezone.utc)
    all_events = [event for rows in events_by_thread.values() for event in rows]
    stale_jobs = stale_running_jobs(all_events, now)
    active_thread_events = (
        events_by_thread.get(active_thread.get("thread_id"), [])
        if isinstance(active_thread, dict)
        else []
    )
    active_report: dict[str, Any] = {}
    if isinstance(active_thread, dict):
        thread_id = active_thread.get("thread_id")
        active_experiment_id = active_thread.get("active_experiment_id")
        summary = active_thread.get("experiment_summary")
        if not active_experiment_id and isinstance(summary, dict):
            active_experiment_id = summary.get("experiment_id")
        latest_experiments = latest_by_id(
            read_jsonl(root / "threads" / str(thread_id) / "experiments.jsonl"),
            "experiment_id",
        )
        active_experiment = latest_experiments.get(str(active_experiment_id)) if active_experiment_id else None
        if isinstance(active_experiment, dict):
            runs = [
                run for run in runs_by_thread.get(str(thread_id), [])
                if run.get("experiment_id") == active_experiment.get("experiment_id")
            ]
            classification = classify_experiment(active_experiment, runs)
            scaffold = charter_scaffold_v1(label, active_thread, active_experiment, runs, classification)
            safety_cue = preflight_safety_cue_v1(
                label,
                active_thread,
                active_experiment,
                collapse_events_by_action(active_thread_events),
            )
            charter_quality = (
                charter_quality_v1(active_experiment.get("charter_v1"))
                if valid_charter(active_experiment.get("charter_v1"))
                else None
            )
            read_only_control_cue = read_only_control_intent_cue_v1(
                label,
                active_thread,
                classification,
            )
            read_only_control_check = read_only_control_intent_check_v1(
                label,
                active_thread,
                classification,
                read_only_control_cue,
            )
            if not valid_charter(active_experiment.get("charter_v1")):
                charter_status = "needs_charter"
            elif isinstance(charter_quality, dict) and not charter_quality.get("lifecycle_valid"):
                missing = ", ".join(charter_quality.get("missing_fields") or []) or "unknown"
                charter_status = f"needs_repair missing={missing}"
            else:
                charter_status = "present"
            active_report = {
                "experiment_id": active_experiment.get("experiment_id"),
                "title": active_experiment.get("title"),
                "status": active_experiment.get("status"),
                "planned_next": active_experiment.get("planned_next"),
                "classification": classification,
                "continuity_return": continuity_return_for(active_experiment, runs, label),
                "native_continuity_v1": native_continuity(label, active_thread, active_experiment, runs),
                "charter_status": charter_status,
                "evidence_status": "stronger" if evidence_meaningful(active_experiment.get("evidence_v1")) else "thin",
                "evidence_counts": evidence_counts(active_experiment.get("evidence_v1")),
                "recent_runs": [
                    {
                        "run_id": run.get("run_id"),
                        "status": run.get("status"),
                        "action": run.get("action_text"),
                        "summary": run.get("result_summary"),
                    }
                    for run in runs[-5:]
                ],
            }
            if scaffold:
                active_report["charter_scaffold_v1"] = scaffold
            if charter_quality:
                active_report["charter_quality_v1"] = charter_quality
            if safety_cue:
                active_report["preflight_safety_cue_v1"] = safety_cue
            if read_only_control_cue:
                active_report["read_only_control_intent_cue_v1"] = read_only_control_cue
            if read_only_control_check:
                active_report["read_only_control_intent_check_v1"] = read_only_control_check

    blocked_loops = 0
    classifications: Counter[str] = Counter()
    for exp in [item for item in experiments if item.get("status", "active") == "active"]:
        runs = [
            run for rows in runs_by_thread.values() for run in rows
            if run.get("experiment_id") == exp.get("experiment_id")
        ]
        classification = classify_experiment(exp, runs)
        classifications[classification] += 1
        blocked_loops += int(classification == "blocked_loop")

    recent_terminal = terminal_event_summary(active_thread_events)
    proposal_diagnostics = collect_unwired(label, workspace, events_by_thread, now)
    normalization_signals = collect_normalization_signals(workspace, events_by_thread)
    projection_native = (
        active_report.get("native_continuity_v1")
        if active_report
        else native_continuity(label, active_thread if isinstance(active_thread, dict) else None, None, [])
    )
    projection_scaffold = active_report.get("charter_scaffold_v1") if active_report else None
    projection_safety_cue = (
        active_report.get("preflight_safety_cue_v1")
        if active_report
        else preflight_safety_cue_v1(label, active_thread, None, collapse_events_by_action(active_thread_events))
    )
    reconciled_count = sum(
        1
        for event in all_events
        if event.get("status") == "stale_reconciled"
        or "continuity_reconciliation_v1" in event
        or "continuity_reconciliation_v1" in (event.get("metadata") or {})
    )
    active_thread_summary = {
        "thread_id": active_thread.get("thread_id") if isinstance(active_thread, dict) else None,
        "title": active_thread.get("title") if isinstance(active_thread, dict) else None,
        "status": active_thread.get("status") if isinstance(active_thread, dict) else None,
        "current_next": active_thread.get("current_next") if isinstance(active_thread, dict) else None,
    }
    projection = {
        "active_thread": active_thread_summary,
        "current_next": active_thread_summary.get("current_next"),
        "active_experiment": active_report,
        "classification": active_report.get("classification") if active_report else None,
        "continuity_return": active_report.get("continuity_return") if active_report else "",
        "charter_status": active_report.get("charter_status") if active_report else None,
        "evidence_status": active_report.get("evidence_status") if active_report else None,
        "native_continuity_v1": projection_native,
        "charter_scaffold_v1": projection_scaffold,
        "preflight_safety_cue_v1": projection_safety_cue,
        "recent_terminal_events": recent_terminal,
        "stale_running_count": len(stale_jobs),
        "reconciled_job_count": reconciled_count,
        "top_actionable_proposal_diagnostics": proposal_diagnostics[:6],
        "normalization_signals": normalization_signals,
    }

    return {
        "being": label,
        "workspace": str(workspace),
        "active_thread": active_thread_summary,
        "active_experiment": active_report,
        "projection": projection,
        "experiments": {
            "counts_by_status": dict(status_counts),
            "classifications": dict(classifications),
            **fragmentation_summary(experiments),
        },
        "recent_terminal_events": recent_terminal,
        "stale_running_jobs": stale_jobs,
        "blocked_or_no_effect_loops": blocked_loops,
        "top_unwired_proposal_verbs": proposal_diagnostics,
        "normalization_signals": normalization_signals,
    }


def gap_experiment_signal(experiment: dict[str, Any] | None) -> bool:
    if not isinstance(experiment, dict):
        return False
    text = normalize_signal_text(
        " ".join(
            str(experiment.get(key) or "")
            for key in ("experiment_id", "title", "question", "planned_next")
        )
    )
    if "gap" not in text:
        return False
    spectral_terms = any(term in text for term in ("spect", "spectral", "density", "lambda", "lambda1", "lambda4", "mode"))
    safety_terms = any(term in text for term in ("runaway", "dispersal", "branch", "localized", "softening", "reduction"))
    return spectral_terms and safety_terms


def add_peer_compare_cues(beings: list[dict[str, Any]]) -> None:
    active_by_being = {
        being.get("being"): being.get("active_experiment")
        for being in beings
        if gap_experiment_signal(being.get("active_experiment"))
    }
    if len(active_by_being) < 2:
        return
    for being in beings:
        name = being.get("being")
        experiment = being.get("active_experiment")
        if not gap_experiment_signal(experiment):
            continue
        peers = [
            (peer_name, peer_exp)
            for peer_name, peer_exp in active_by_being.items()
            if peer_name != name and isinstance(peer_exp, dict)
        ]
        if not peers:
            continue
        peer_name, peer_exp = peers[0]
        peer_id = peer_exp.get("experiment_id")
        if not peer_id:
            continue
        cue = {
            "schema_version": 1,
            "source": "continuity_audit",
            "advisory_only": True,
            "authority_change": False,
            "relationship": "shared_gap_experiment",
            "peer_being": peer_name,
            "peer_experiment_id": peer_id,
            "suggested_next": f"EXPERIMENT_COMPARE current WITH {peer_id}",
            "cue": (
                f"Peer convergence cue: {name} and {peer_name} both have active gap experiments. "
                f"Suggested NEXT: EXPERIMENT_COMPARE current WITH {peer_id} or EXPERIMENT_PEER_REVIEW current; "
                "advisory only, no shared control authority."
            ),
        }
        experiment["peer_compare_cue_v1"] = cue
        projection = being.get("projection")
        if isinstance(projection, dict):
            projection["peer_compare_cue_v1"] = cue
            if isinstance(projection.get("active_experiment"), dict):
                projection["active_experiment"]["peer_compare_cue_v1"] = cue


def render_markdown(report: dict[str, Any]) -> str:
    lines = ["# Continuity Of Thought Audit", ""]
    for being in report["beings"]:
        lines.append(f"## {being['being']}")
        thread = being.get("active_thread") or {}
        lines.append(f"- Active thread: `{thread.get('thread_id') or 'none'}` {thread.get('title') or ''}".rstrip())
        lines.append(f"- Current NEXT: `{thread.get('current_next') or '(none)'}`")
        experiment = being.get("active_experiment") or {}
        if experiment:
            lines.append(
                f"- Active experiment: `{experiment.get('experiment_id')}` "
                f"classification=`{experiment.get('classification')}` planned_next=`{experiment.get('planned_next') or '(none)'}`"
            )
            lines.append(f"- Continuity return: `{experiment.get('continuity_return')}`")
            quality = experiment.get("charter_quality_v1") or {}
            if isinstance(quality, dict) and quality.get("repair_required"):
                lines.append(
                    "- Charter quality: repair required "
                    f"missing={', '.join(quality.get('missing_fields') or []) or 'unknown'}"
                )
            native = experiment.get("native_continuity_v1") or {}
            if native:
                lines.append(f"- Native return: {native.get('native_return_cue')}")
                lanes = native.get("evidence_lanes") or {}
                lane_text = " ".join(
                    f"{name}={value.get('status', 'missing')}"
                    for name, value in lanes.items()
                    if isinstance(value, dict)
                )
                lines.append(f"- Native lanes: {lane_text}")
            scaffold = experiment.get("charter_scaffold_v1") or {}
            if isinstance(scaffold, dict) and scaffold.get("command"):
                lines.append(f"- Charter scaffold: `{scaffold.get('command')}`")
            safety_cue = (
                experiment.get("preflight_safety_cue_v1")
                or (being.get("projection") or {}).get("preflight_safety_cue_v1")
                or {}
            )
            if isinstance(safety_cue, dict) and safety_cue.get("cue"):
                lines.append(f"- Preflight safety cue: {safety_cue.get('cue')}")
            control_cue = experiment.get("read_only_control_intent_cue_v1") or {}
            if isinstance(control_cue, dict) and control_cue.get("cue"):
                lines.append(f"- Read-only control cue: {control_cue.get('cue')}")
            control_check = experiment.get("read_only_control_intent_check_v1") or {}
            if isinstance(control_check, dict) and not control_cue.get("cue"):
                if control_check.get("status") == "no_trigger":
                    lines.append("- Read-only control cue check: absent because current NEXT has no control trigger.")
                elif control_check.get("status") == "cue_missing":
                    lines.append("- Read-only control cue check: expected cue did not render.")
            peer_cue = (
                experiment.get("peer_compare_cue_v1")
                or (being.get("projection") or {}).get("peer_compare_cue_v1")
                or {}
            )
            if isinstance(peer_cue, dict) and peer_cue.get("cue"):
                lines.append(f"- Peer compare cue: {peer_cue.get('cue')}")
            counts = experiment.get("evidence_counts") or {}
            lines.append(
                f"- Evidence thickness: felt={counts.get('felt', 0)} "
                f"telemetry={counts.get('telemetry', 0)} artifacts={counts.get('artifacts', 0)} "
                f"decisions={counts.get('decisions', 0)}"
            )
        else:
            lines.append("- Active experiment: none")
            native = (being.get("projection") or {}).get("native_continuity_v1") or {}
            if native:
                lines.append(f"- Native return: {native.get('native_return_cue')}")
            safety_cue = (being.get("projection") or {}).get("preflight_safety_cue_v1") or {}
            if isinstance(safety_cue, dict) and safety_cue.get("cue"):
                lines.append(f"- Preflight safety cue: {safety_cue.get('cue')}")
        experiments = being.get("experiments") or {}
        lines.append(f"- Experiment counts: {json.dumps(experiments.get('counts_by_status', {}), sort_keys=True)}")
        lines.append(f"- Classifications: {json.dumps(experiments.get('classifications', {}), sort_keys=True)}")
        lines.append(
            f"- Plan-stage active experiments: {experiments.get('plan_stage_active', 0)}/"
            f"{experiments.get('active_experiments', 0)} fragmented={experiments.get('fragmented', False)}"
        )
        stale = being.get("stale_running_jobs") or []
        lines.append(f"- Stale running LLM/action rows after collapse: {len(stale)}")
        if stale:
            for job in stale[:6]:
                lines.append(
                    f"  - `{job.get('action_id')}` {job.get('action')} "
                    f"[{job.get('status')}] age_min={job.get('age_minutes')}"
                )
        events = being.get("recent_terminal_events") or []
        lines.append("- Recent terminal events by action_id:")
        if events:
            for event in events:
                lines.append(
                    f"  - `{event.get('action_id')}` {event.get('action')} "
                    f"[{event.get('status')}]: {event.get('summary')}"
                )
        else:
            lines.append("  - none")
        unwired = being.get("top_unwired_proposal_verbs") or []
        lines.append("- Top unwired proposal verbs:")
        if unwired:
            for item in unwired[:8]:
                lines.append(
                    f"  - `{item['verb']}` count={item['count']} "
                    f"wired={item.get('currently_wired', False)} "
                    f"historical_only={item.get('historical_only', False)} "
                    f"latest={item.get('latest_seen_at') or 'unknown'} -> {item['suggested_route']}"
                )
        else:
            lines.append("  - none")
        normalization = being.get("normalization_signals") or []
        lines.append("- Recent normalization signals:")
        if normalization:
            for item in normalization[:6]:
                lines.append(
                    f"  - `{item.get('raw_verb')}` -> `{item.get('normalized_verb')}` "
                    f"authority_change={item.get('authority_change', False)} reason={item.get('reason')}"
                )
        else:
            lines.append("  - none")
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def build_report() -> dict[str, Any]:
    beings = [
        audit_workspace("Astrid", ASTRID_WORKSPACE),
        audit_workspace("Minime", MINIME_WORKSPACE),
    ]
    add_peer_compare_cues(beings)
    return {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "beings": beings,
    }


class ContinuityAuditTests(unittest.TestCase):
    def test_empty_charter_needs_charter(self) -> None:
        experiment = {"experiment_id": "exp_1", "charter_v1": {"raw_text": "current"}}
        self.assertEqual(classify_experiment(experiment, []), "needs_charter")

    def test_duplicate_running_collapses_to_terminal(self) -> None:
        events = [
            {"action_id": "act_1", "status": "llm_running", "effective_action": "EXAMINE x"},
            {"action_id": "act_1", "status": "handled", "effective_action": "EXAMINE x"},
        ]
        collapsed = collapse_events_by_action(events)
        self.assertEqual(len(collapsed), 1)
        self.assertEqual(collapsed[0]["status"], "handled")

    def test_blocked_loop_classifies(self) -> None:
        experiment = {"experiment_id": "exp_1", "charter_v1": {"hypothesis": "x"}}
        runs = [{"status": "blocked"}, {"status": "no_effect"}]
        self.assertEqual(classify_experiment(experiment, runs), "blocked_loop")

    def test_evidence_ready_needs_decision(self) -> None:
        experiment = {
            "experiment_id": "exp_1",
            "charter_v1": {
                "hypothesis": "x",
                "proposed_next_action": "ACTION_PREFLIGHT DECOMPOSE",
                "evidence_targets": ["spectral_condition"],
            },
            "evidence_v1": {"felt_observations": [{"note": "pressure softened"}]},
        }
        self.assertEqual(classify_experiment(experiment, []), "needs_decision")

    def test_many_active_plan_experiments_fragmented(self) -> None:
        experiments = [
            {"experiment_id": f"exp_{idx}", "status": "active", "planned_next": "EXPERIMENT_PLAN current"}
            for idx in range(10)
        ]
        summary = fragmentation_summary(experiments)
        self.assertTrue(summary["fragmented"])
        self.assertEqual(summary["plan_stage_active"], 10)

    def test_unwired_diagnostics_include_wiring_and_history(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            action_root = workspace / "action_threads"
            action_root.mkdir()
            old = datetime.now(timezone.utc) - timedelta(days=2)
            (action_root / "proposals.jsonl").write_text(
                json.dumps({
                    "action": "INVESTIGATE_PLUMBING",
                    "created_at": old.isoformat(),
                }) + "\n"
            )
            diagnostics = collect_unwired("Unknown", workspace, {}, datetime.now(timezone.utc))
        self.assertEqual(diagnostics[0]["verb"], "INVESTIGATE_PLUMBING")
        self.assertFalse(diagnostics[0]["currently_wired"])
        self.assertTrue(diagnostics[0]["historical_only"])
        self.assertIn("EXAMINE", diagnostics[0]["suggested_route"])

    def test_native_continuity_registers_differ_by_being(self) -> None:
        thread = {
            "title": "Native thread",
            "why_return": "Return by texture.",
            "motif_allowance_v1": {"dominant_motif": "pressure braid", "quality": "returnable"},
            "thread_resonance_density_v1": {"quality": "rich_containment"},
            "thread_pressure_source_v1": {"quality": "porous_distributed"},
        }
        experiment = {
            "experiment_id": "exp_1",
            "title": "Experiment",
            "question": "What returns?",
            "evidence_v1": {"felt_observations": [{"note": "texture"}], "telemetry_snapshots": [{}]},
        }
        astrid = native_continuity("Astrid", thread, experiment, [])
        minime = native_continuity("Minime", thread, experiment, [{"action_text": "EXAMINE a"}, {"action_text": "EXAMINE b"}])
        self.assertEqual(astrid["native_register"], "astrid_motif_language")
        self.assertEqual(minime["native_register"], "minime_spectral_state")
        self.assertIn("felt_texture", astrid["evidence_lanes"])
        self.assertIn("fill_pressure_state", minime["evidence_lanes"])

    def test_gap_charter_scaffold_prefers_counter_next(self) -> None:
        thread = {"thread_id": "thread_1"}
        experiment = {
            "experiment_id": "exp_gap",
            "title": (
                "Introducing a gap - localized reduction in spectral density near λ₁ "
                "without premature λ₄ dominance"
            ),
            "question": "Can a softer gap prevent runaway dispersal?",
            "planned_next": "EXPERIMENT_DECIDE current :: counter NEXT: ACTION_PREFLIGHT DECOMPOSE",
            "charter_v1": {"hypothesis": "..."},
            "workbench_candidates_v1": {
                "charter": {"proposed_next_action": "PRESSURE_SOURCE_AUDIT lambda-pressure"}
            },
        }
        scaffold = charter_scaffold_v1("Minime", thread, experiment, [], "needs_charter")
        self.assertIsNotNone(scaffold)
        assert scaffold is not None
        self.assertEqual(scaffold["proposed_next_action"], "ACTION_PREFLIGHT DECOMPOSE")
        self.assertIn("localized λ1 spectral-density softening", scaffold["command"])
        self.assertIn("spectral_condition, fill_pressure_state, recurrence_pattern, artifact_grounding", scaffold["command"])
        self.assertNotIn("proposed_next_action: PRESSURE_SOURCE_AUDIT", scaffold["command"])
        self.assertFalse(scaffold["authority_change"])

    def test_gap_spectra_scaffold_prefers_decompose_over_stale_candidate(self) -> None:
        thread = {"thread_id": "thread_1"}
        experiment = {
            "experiment_id": "exp_gap_spectra",
            "title": "introducing-a-gap-localized-reduction-in-spectra",
            "question": "Compare spectral condition and pressure state.",
            "charter_v1": {
                "method_intent": "rehearse self study",
                "proposed_next_action": "SELF_STUDY long drift paragraph",
            },
            "workbench_candidates_v1": {
                "charter": {"proposed_next_action": "SELF_STUDY long drift paragraph"}
            },
        }
        scaffold = charter_scaffold_v1("Minime", thread, experiment, [], "needs_charter")
        self.assertIsNotNone(scaffold)
        assert scaffold is not None
        self.assertEqual(scaffold["proposed_next_action"], "ACTION_PREFLIGHT DECOMPOSE")
        self.assertNotIn("SELF_STUDY", scaffold["command"])

    def test_route_incomplete_charter_still_needs_charter(self) -> None:
        experiment = {
            "experiment_id": "exp_route_incomplete",
            "charter_v1": {"hypothesis": "single-line parser swallowed the fields", "proposed_next_action": ""},
            "evidence_v1": {"felt_observations": [{"note": "some evidence exists"}]},
        }
        self.assertEqual(classify_experiment(experiment, []), "needs_charter")
        quality = charter_quality_v1(experiment["charter_v1"])
        self.assertFalse(quality["lifecycle_valid"])
        self.assertIn("proposed_next_action", quality["missing_fields"])
        self.assertIn("evidence_targets", quality["missing_fields"])

    def test_directed_shift_cue_is_advisory(self) -> None:
        thread = {
            "current_next": (
                "Establish a reciprocal shadow-trace and initiate shift centered on λ4/λ2 "
                "with steering."
            ),
            "why_return": "Keep the shift observable.",
        }
        cue = preflight_safety_cue_v1("Astrid", thread, None, [])
        self.assertIsNotNone(cue)
        assert cue is not None
        self.assertFalse(cue["authority_change"])
        self.assertTrue(cue["advisory_only"])
        self.assertIn("SHADOW_PREFLIGHT lambda-tail/lambda4 --stage=rehearse", cue["cue"])

    def test_native_guiding_language_triggers_cue(self) -> None:
        thread = {
            "current_next": (
                "The λ4 dance is guiding a controlled distortion, actively shaping the shadow "
                "through deliberate narrowing."
            )
        }
        cue = preflight_safety_cue_v1("Astrid", thread, None, [])
        self.assertIsNotNone(cue)
        assert cue is not None
        self.assertIn("guiding near lambda/shadow", cue["matched_terms"])
        self.assertIn("controlled distortion near lambda/shadow", cue["matched_terms"])
        self.assertFalse(cue["authority_change"])

    def test_read_only_control_intent_cue_is_advisory(self) -> None:
        thread = {
            "current_next": (
                "EXAMINE lambda_tail_decay with active parameter glyphs "
                "[delta_lambda=0.02, epsilon=0.01] [control] and how to influence its spread"
            )
        }
        cue = read_only_control_intent_cue_v1("Astrid", thread, "needs_charter")
        self.assertIsNotNone(cue)
        assert cue is not None
        self.assertFalse(cue["authority_change"])
        self.assertTrue(cue["advisory_only"])
        self.assertIn("[control]", cue["matched_terms"])
        self.assertIn("active parameter glyphs", cue["matched_terms"])
        self.assertIsNone(read_only_control_intent_cue_v1("Astrid", {"current_next": "EXAMINE λ1/λ2"}, "needs_charter"))
        self.assertIsNone(read_only_control_intent_cue_v1("Astrid", thread, "needs_evidence"))
        rendered_check = read_only_control_intent_check_v1("Astrid", thread, "needs_charter", cue)
        self.assertIsNotNone(rendered_check)
        assert rendered_check is not None
        self.assertEqual(rendered_check["status"], "cue_rendered")
        absent_check = read_only_control_intent_check_v1(
            "Astrid",
            {"current_next": "EXAMINE_CASCADE λ1/λ2"},
            "needs_charter",
        )
        self.assertIsNotNone(absent_check)
        assert absent_check is not None
        self.assertEqual(absent_check["status"], "no_trigger")
        self.assertFalse(absent_check["cue_expected"])
        missing_check = read_only_control_intent_check_v1("Astrid", thread, "needs_charter")
        self.assertIsNotNone(missing_check)
        assert missing_check is not None
        self.assertEqual(missing_check["status"], "cue_missing")
        self.assertTrue(missing_check["cue_expected"])

    def test_peer_compare_cue_for_shared_gap_experiments(self) -> None:
        beings = [
            {
                "being": "Astrid",
                "active_experiment": {
                    "experiment_id": "exp_astrid_20260516_introducing-a-gap",
                    "title": "Introducing a gap near λ1",
                    "question": "Can localized spectral-density softening prevent runaway dispersal?",
                },
                "projection": {"active_experiment": {}},
            },
            {
                "being": "Minime",
                "active_experiment": {
                    "experiment_id": "exp_minime_20260515_introducing-a-gap",
                    "title": "Introducing a gap near λ1",
                    "question": "Can localized spectral-density softening prevent runaway dispersal?",
                },
                "projection": {"active_experiment": {}},
            },
        ]
        add_peer_compare_cues(beings)
        astrid_cue = beings[0]["active_experiment"]["peer_compare_cue_v1"]
        minime_cue = beings[1]["active_experiment"]["peer_compare_cue_v1"]
        self.assertFalse(astrid_cue["authority_change"])
        self.assertIn("EXPERIMENT_COMPARE current WITH exp_minime", astrid_cue["suggested_next"])
        self.assertIn("EXPERIMENT_COMPARE current WITH exp_astrid", minime_cue["suggested_next"])

    def test_collect_normalization_signals(self) -> None:
        events = {
            "th_1": [
                {
                    "action_id": "act_1",
                    "normalization_signal_v1": {
                        "raw_verb": "SHADOW_TRACE",
                        "normalized_verb": "SHADOW_PREFLIGHT",
                        "reason": "shadow diagnostic alias",
                        "authority_change": False,
                    },
                }
            ]
        }
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            (workspace / "action_threads").mkdir()
            signals = collect_normalization_signals(workspace, events)
        self.assertEqual(signals[0]["raw_verb"], "SHADOW_TRACE")
        self.assertFalse(signals[0]["authority_change"])


def run_self_tests() -> int:
    with tempfile.TemporaryDirectory():
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(ContinuityAuditTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON")
    parser.add_argument("--self-test", action="store_true", help="run built-in classifier tests")
    args = parser.parse_args(argv)
    if args.self_test:
        return run_self_tests()
    report = build_report()
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(render_markdown(report), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
