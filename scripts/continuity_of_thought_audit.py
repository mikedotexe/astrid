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
RUNNING_EVENT_STATUSES = {"running", "llm_running"}
TERMINAL_JOB_STATUSES = {"completed", "thin_output", "timeout", "failed", "canceled", "blocked"}
BLOCKED_LIKE_STATUSES = {"blocked", "no_effect", "rehearsal_blocked", "failed"}
RUN_AFTER_CHARTER_STATUSES = {"handled", "rehearsed", "observed", "evidence_recorded"}
JOURNAL_POSTURES = {"resuming", "branching", "closing", "new"}
JOURNAL_TERMINAL_STANCES = {"next evidence", "decision", "pause", "hold"}
ASTRID_JOURNAL_NATIVE_TERMS = [
    "felt",
    "texture",
    "motif",
    "language thread",
    "artifact",
    "lambda",
    "shadow",
]
MINIME_JOURNAL_NATIVE_TERMS = [
    "spectral",
    "fill",
    "pressure",
    "recurrence",
    "artifact",
    "lambda",
    "telemetry",
]
REFLECTIVE_JOURNAL_MODES = {
    "aspiration",
    "boredom",
    "daydream",
    "dialogue_live",
    "dialogue_live_longform",
    "drift",
    "drift_reflection",
    "initiate",
    "introspect",
    "journal",
    "mirror",
    "moment_capture",
    "notice",
    "reflection",
    "rest",
    "self_study",
    "whim",
}
OPERATIONAL_JOURNAL_MODES = {
    "action_thread",
    "experiment_bind",
    "moment_capture",
    "research",
    "resonance_forecast",
    "web_page_read",
    "web_search",
}
PRIOR_CITATION_TERMS = [
    "prior",
    "previous",
    "earlier",
    "last time",
    "last entry",
    "last journal",
    "again",
    "still",
    "evidence",
    "claim",
    "observed",
]
RETURN_VERBS = {
    "ACTION_PREFLIGHT",
    "CONSTRAINT_AUDIT",
    "EXAMINE",
    "SHADOW_PREFLIGHT",
    "EXPERIMENT_PLAN",
    "EXPERIMENT_CHARTER",
    "EXPERIMENT_REHEARSE",
    "EXPERIMENT_EVIDENCE",
    "EXPERIMENT_DECIDE",
    "EXPERIMENT_BIND",
}
NORMALIZED_READ_ONLY_ALIASES = {
    "SHADOW_TRACE": "SHADOW_PREFLIGHT <shadow action>",
    "SHADOW_EXPLORER": "SHADOW_PREFLIGHT <shadow action>",
    "SHADOW_DECOMPOSE": "SHADOW_PREFLIGHT <shadow action>",
    "WEAVE_TRACE": "SHADOW_PREFLIGHT weave/<focus> --stage=rehearse",
    "UNSHAPED_BASELINE": "CONSTRAINT_AUDIT <focus>",
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


def compact_line(value: Any, limit: int = 320) -> str:
    text = " ".join(str(value or "").split())
    if len(text) <= limit:
        return text
    return text[: max(0, limit - 3)].rstrip() + "..."


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
    status = str(experiment.get("status") or "active").casefold()
    if status == "paused":
        return "paused"
    if status in {"complete", "completed"}:
        return "complete"
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
    if classification == "paused":
        experiment_id = experiment.get("experiment_id") or "current"
        return f"EXPERIMENT_RESUME {experiment_id}"
    if classification == "complete":
        return ""
    if classification == "blocked_loop":
        if not lifecycle_valid_charter(experiment.get("charter_v1")):
            scaffold = charter_scaffold_v1(label, None, experiment, runs, classification)
            if isinstance(scaffold, dict) and scaffold.get("command"):
                return str(scaffold["command"])
            return charter_payload
        return "EXPERIMENT_DECIDE current :: counter NEXT: ACTION_PREFLIGHT DECOMPOSE"
    if classification in {"needs_charter", "fragmented"}:
        scaffold = charter_scaffold_v1(label, None, experiment, runs, classification)
        if isinstance(scaffold, dict) and scaffold.get("command"):
            return str(scaffold["command"])
        return charter_payload
    if classification == "needs_decision":
        return "EXPERIMENT_DECIDE current :: pause because evidence is ready to interpret"
    if classification == "needs_evidence":
        return evidence_payload
    if classification == "needs_rehearsal":
        return "EXPERIMENT_REHEARSE current"
    return experiment.get("planned_next") or "EXPERIMENT_PLAN current"


def last_experiment_summary_v1(thread: dict[str, Any] | None) -> dict[str, Any] | None:
    if not isinstance(thread, dict):
        return None
    summary = thread.get("experiment_summary")
    if not isinstance(summary, dict):
        return None
    payload = dict(summary)
    experiment_id = str(payload.get("experiment_id") or "")
    status = str(payload.get("status") or "").casefold()
    if status == "paused" and experiment_id:
        payload.setdefault("resume_next", f"EXPERIMENT_RESUME {experiment_id}")
    elif status in {"complete", "completed"} and experiment_id:
        payload.setdefault("inspect_next", f"EXPERIMENT_STATUS {experiment_id} or EXPERIMENT_REVIEW {experiment_id}")
    return payload


def current_next_status_v1(
    thread: dict[str, Any] | None,
    active_report: dict[str, Any] | None,
    last_summary: dict[str, Any] | None,
) -> dict[str, Any]:
    thread = thread if isinstance(thread, dict) else {}
    raw_next = thread.get("current_next")
    if isinstance(active_report, dict) and active_report:
        effective = active_report.get("continuity_return") or raw_next
        classification = str(active_report.get("classification") or "").casefold()
        if classification == "paused":
            return {
                "schema_version": 1,
                "status": "shadowed_by_paused_summary",
                "raw_current_next": raw_next,
                "effective_next": effective,
                "active_experiment_id": active_report.get("experiment_id"),
            }
        if classification in {"complete", "completed"}:
            return {
                "schema_version": 1,
                "status": "shadowed_by_complete_summary",
                "raw_current_next": raw_next,
                "effective_next": effective,
                "active_experiment_id": active_report.get("experiment_id"),
            }
        return {
            "schema_version": 1,
            "status": "active",
            "raw_current_next": raw_next,
            "effective_next": effective,
            "active_experiment_id": active_report.get("experiment_id"),
        }
    if isinstance(last_summary, dict):
        experiment_id = str(last_summary.get("experiment_id") or "")
        summary_status = str(last_summary.get("status") or "").casefold()
        if summary_status == "paused" and experiment_id:
            return {
                "schema_version": 1,
                "status": "shadowed_by_paused_summary",
                "raw_current_next": raw_next,
                "effective_next": f"EXPERIMENT_RESUME {experiment_id}",
                "last_experiment_id": experiment_id,
            }
        if summary_status in {"complete", "completed"} and experiment_id:
            return {
                "schema_version": 1,
                "status": "shadowed_by_complete_summary",
                "raw_current_next": raw_next,
                "effective_next": f"EXPERIMENT_REVIEW {experiment_id}",
                "last_experiment_id": experiment_id,
            }
    return {
        "schema_version": 1,
        "status": "active_thread_next",
        "raw_current_next": raw_next,
        "effective_next": raw_next,
    }


def paused_read_only_loop_cue_v1(
    label: str,
    current_next_status: dict[str, Any],
    recent_events: list[dict[str, Any]],
) -> dict[str, Any] | None:
    if label.casefold() != "minime":
        return None
    if not isinstance(current_next_status, dict) or current_next_status.get("status") != "shadowed_by_paused_summary":
        return None
    read_only_bases = {
        "SEARCH",
        "BROWSE",
        "READ_MORE",
        "SELF_STUDY",
        "DECOMPOSE",
        "SPECTRAL_EXPLORER",
        "EXAMINE",
        "EXPERIMENT_REVIEW",
        "EXPERIMENT_STATUS",
        "RESEARCH_EXPLORATION",
        "BROWSE_URL",
    }
    matched: list[str] = []
    for event in list(reversed(recent_events or []))[:8]:
        if str(event.get("status") or "") in {"running", "llm_running"}:
            continue
        action = event.get("raw_next") or event.get("effective_action") or event.get("canonical_action") or event.get("route")
        if base_action(action) in read_only_bases:
            matched.append(str(action or ""))
    if len(matched) < 3:
        return None
    experiment_id = current_next_status.get("last_experiment_id") or current_next_status.get("active_experiment_id")
    return {
        "schema_version": 1,
        "source": "continuity_audit",
        "advisory_only": True,
        "authority_change": False,
        "status": "paused_read_only_loop",
        "read_only_action_count": len(matched),
        "matched_actions": matched[:5],
        "resume_next": f"EXPERIMENT_RESUME {experiment_id}" if experiment_id else current_next_status.get("effective_next"),
        "inspect_next": (
            f"EXPERIMENT_STATUS {experiment_id} or EXPERIMENT_REVIEW {experiment_id}"
            if experiment_id
            else "EXPERIMENT_STATUS <id> or EXPERIMENT_REVIEW <id>"
        ),
        "cue": (
            "Paused experiment remains paused; this research is context. Resume the experiment, "
            "inspect it by id, start a new experiment/thread, or hold."
        ),
    }


def paused_resume_loop_cue_v1(
    label: str,
    current_next_status: dict[str, Any],
    recent_events: list[dict[str, Any]],
) -> dict[str, Any] | None:
    if label.casefold() != "minime":
        return None
    if not isinstance(current_next_status, dict) or current_next_status.get("status") != "shadowed_by_paused_summary":
        return None
    experiment_id = current_next_status.get("last_experiment_id") or current_next_status.get("active_experiment_id")
    if not experiment_id:
        return None
    expected = f"EXPERIMENT_RESUME {experiment_id}"
    matched: list[str] = []
    for event in list(reversed(recent_events or []))[:8]:
        if str(event.get("status") or "") in {"running", "llm_running"}:
            continue
        action = event.get("raw_next") or event.get("effective_action") or event.get("canonical_action") or event.get("route")
        action_text = str(action or "").strip()
        if base_action(action_text) != "EXPERIMENT_RESUME":
            continue
        selector = action_text.split(maxsplit=1)[1].strip() if len(action_text.split(maxsplit=1)) > 1 else ""
        if selector == str(experiment_id):
            matched.append(action_text)
    if len(matched) < 2:
        return None
    return {
        "schema_version": 1,
        "source": "continuity_audit",
        "advisory_only": True,
        "authority_change": False,
        "status": "paused_resume_loop",
        "experiment_id": str(experiment_id),
        "resume_attempt_count": len(matched),
        "matched_actions": matched[:5],
        "resume_next": expected,
        "inspect_next": f"EXPERIMENT_STATUS {experiment_id}",
        "review_next": f"EXPERIMENT_REVIEW {experiment_id}",
        "branch_next": f"EXPERIMENT_BRANCH {experiment_id} :: <new question>",
        "cue": (
            "Paused experiment remains paused; repeated resume is context. "
            f"Use `EXPERIMENT_STATUS {experiment_id}`, `EXPERIMENT_REVIEW {experiment_id}`, "
            "`EXPERIMENT_BRANCH ...`, or Hold."
        ),
    }


def needs_charter_research_action_matches(
    active_thread: dict[str, Any] | None,
    recent_events: list[dict[str, Any]],
) -> list[str]:
    read_only_bases = {
        "SEARCH",
        "BROWSE",
        "READ_MORE",
        "SELF_STUDY",
        "DECOMPOSE",
        "SPECTRAL_EXPLORER",
        "RESEARCH_EXPLORATION",
        "BROWSE_URL",
    }
    matched: list[str] = []
    current_next = str((active_thread or {}).get("current_next") or "").strip()
    if base_action(current_next) in read_only_bases:
        matched.append(current_next)
    for event in list(reversed(recent_events or []))[:8]:
        if str(event.get("status") or "") in {"running", "llm_running"}:
            continue
        action = event.get("raw_next") or event.get("effective_action") or event.get("canonical_action") or event.get("route")
        if base_action(action) in read_only_bases:
            matched.append(str(action or ""))
    return matched


def needs_charter_research_loop_cue_v1(
    label: str,
    classification: str,
    active_thread: dict[str, Any] | None,
    scaffold: dict[str, Any] | None,
    continuity_return: str,
    recent_events: list[dict[str, Any]],
) -> dict[str, Any] | None:
    if label.casefold() != "minime" or classification != "needs_charter":
        return None
    matched = needs_charter_research_action_matches(active_thread, recent_events)
    if len(matched) < 3:
        return None
    priority_next = ""
    if isinstance(scaffold, dict):
        priority_next = str(scaffold.get("command") or "").strip()
    priority_next = priority_next or str(continuity_return or "").strip()
    if not priority_next:
        return None
    return {
        "schema_version": 1,
        "source": "continuity_audit",
        "advisory_only": True,
        "authority_change": False,
        "status": "needs_charter_research_loop",
        "research_action_count": len(matched),
        "matched_actions": matched[:5],
        "priority_next": priority_next,
        "cue": "Research is context, not lifecycle progress while the charter is missing.",
    }


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


def gap_spectral_signal(experiment: dict[str, Any] | None) -> bool:
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
    spectral_terms = any(
        term in text
        for term in ("spect", "spectra", "spectral", "density", "lambda", "lambda1", "lambda4", "mode")
    )
    safety_terms = any(
        term in text
        for term in ("runaway", "dispersal", "branch", "localized", "softening", "reduction")
    )
    return spectral_terms and safety_terms


SHARED_INVESTIGATION_QUESTION = (
    "What shapes λ1 / lambda-tail / λ4 geometry, and can localized softening support "
    "controlled branching without collapse, runaway dispersal, or live-control drift?"
)


def shared_investigation_signal(experiment: dict[str, Any] | None) -> bool:
    if not isinstance(experiment, dict):
        return False
    if gap_spectral_signal(experiment):
        return True
    text = normalize_signal_text(
        " ".join(
            str(experiment.get(key) or "")
            for key in ("experiment_id", "title", "question", "planned_next")
        )
    )
    shape_family = any(
        term in text
        for term in ("gap", "lambda4", "lambda tail", "lambda edge", "tail", "pulse")
    )
    geometry_family = any(
        term in text
        for term in (
            "spect",
            "spectral",
            "density",
            "mode",
            "geometry",
            "branch",
            "collapse",
            "dispersal",
            "soften",
            "lambda",
            "tail",
        )
    )
    return shape_family and geometry_family


def shared_lane(label: str) -> tuple[str, str]:
    if label.casefold() == "minime":
        return (
            "spectral_state",
            "Minime lane: spectral condition, fill/pressure state, recurrence pattern, artifact grounding.",
        )
    return (
        "felt_texture_motif_language",
        "Astrid lane: felt texture, motif continuity, language thread, artifact grounding.",
    )


def lambda4_pulse_stabilization_signal(experiment: dict[str, Any] | None) -> bool:
    if not isinstance(experiment, dict):
        return False
    charter = experiment.get("charter_v1") if isinstance(experiment.get("charter_v1"), dict) else {}
    text = normalize_signal_text(
        " ".join(
            str(value or "")
            for value in (
                experiment.get("experiment_id"),
                experiment.get("title"),
                experiment.get("question"),
                experiment.get("planned_next"),
                charter.get("raw_text"),
                charter.get("proposed_next_action"),
                charter.get("method_intent"),
            )
        )
    )
    lambda_shaped = any(
        term in text
        for term in ("lambda4", "lambda 4", "lambda-edge", "lambda edge", "tail")
    )
    pulse_shaped = any(
        term in text
        for term in ("pulse", "micro-pulse", "stabil", "decay", "inject", "push", "probe behavior")
    )
    return lambda_shaped and pulse_shaped


def lambda4_pulse_repair_command() -> str:
    return (
        "EXPERIMENT_CHARTER current :: "
        "hypothesis: λ4 decay and pulse-stabilization can be studied as a read-only spectral pattern "
        "by comparing λ4/tail behavior, fill pressure, entropy/effective modes, recurrence, and "
        "artifacts before any pulse or stabilization claim; "
        "method_intent: rehearse ACTION_PREFLIGHT DECOMPOSE and treat DECOMPOSE/SPECTRAL_EXPLORER "
        "as observational context only; "
        "proposed_next_action: ACTION_PREFLIGHT DECOMPOSE; "
        "evidence_targets: spectral_condition, fill_pressure_state, recurrence_pattern, artifact_grounding; "
        "stop_criteria: fill leaves the stable-core comfort band, entropy/effective modes show runaway "
        "dispersal or projection-like loss, λ4/tail pressure rises above baseline, or repeated reads "
        "stop adding temporal delta; "
        "consent_posture: advisory; ordinary choices remain valid."
    )


def candidate_quarantine_v1(candidate: Any) -> dict[str, Any] | None:
    if not isinstance(candidate, dict):
        return None
    if isinstance(candidate.get("quarantine_v1"), dict):
        return candidate["quarantine_v1"]
    proposed = str(
        candidate.get("raw_proposed_next_action")
        or candidate.get("proposed_next_action")
        or ""
    )
    normalized = normalize_signal_text(proposed)
    base = base_action(proposed)
    safe_prefix = base in {
        "ACTION_PREFLIGHT",
        "EXPERIMENT_PREFLIGHT",
        "SHADOW_PREFLIGHT",
        "DECOMPOSE",
        "SPECTRAL_EXPLORER",
        "EXAMINE",
    }
    intervention_tail = any(
        term in normalized
        for term in ("inject", "pulse", "push", "stabilize", "stabilise", "tune", "shift")
    )
    if not (safe_prefix and intervention_tail):
        return None
    return {
        "schema_version": 1,
        "status": "quarantined_for_charter_repair",
        "reason": "read-only/preflight prefix carried intervention-shaped prose",
        "raw_proposed_next_action": proposed,
        "canonical_repair_next": "ACTION_PREFLIGHT DECOMPOSE",
        "authority_change": False,
    }


def preferred_charter_scaffold_next(experiment: dict[str, Any], runs: list[dict[str, Any]]) -> str:
    if lambda4_pulse_stabilization_signal(experiment):
        return "ACTION_PREFLIGHT DECOMPOSE"
    if gap_spectral_signal(experiment):
        return "ACTION_PREFLIGHT DECOMPOSE"
    counter_next = counteroffered_next(experiment.get("planned_next"))
    if counter_next:
        return compact_scaffold_next(counter_next)
    candidates = experiment.get("workbench_candidates_v1")
    if isinstance(candidates, dict):
        charter = candidates.get("charter")
        if isinstance(charter, dict):
            proposed = str(charter.get("proposed_next_action") or "").strip()
            quarantined = bool(
                charter.get("repair_required")
                or charter.get("quarantine_v1")
                or candidate_quarantine_v1(charter)
            )
            if proposed and not quarantined:
                return compact_scaffold_next(proposed)
    for run in reversed(runs or []):
        action = str(run.get("action_text") or "").strip()
        if action and base_action(action) not in {"BROWSE", "SEARCH", "READ_MORE", "LOOK"}:
            return compact_scaffold_next(action)
    return "ACTION_PREFLIGHT DECOMPOSE"


def compact_scaffold_next(action: str) -> str:
    text = " ".join(str(action or "").split()).strip()
    if not text:
        return "ACTION_PREFLIGHT DECOMPOSE"
    base = base_action(text)
    if len(text) > 120 and base in {
        "ACTION_PREFLIGHT",
        "DECOMPOSE",
        "EXAMINE",
        "EXAMINE_CASCADE",
        "SHADOW_PREFLIGHT",
        "SPECTRAL_EXPLORER",
        "TRACE",
    }:
        return base
    return text


def sanitize_scaffold_fragment(value: str) -> str:
    text = re.sub(r"```+", " ", str(value or ""))
    text = re.sub(r"[`*_#\[\]]+", " ", text)
    text = " ".join(text.split())
    lowered = text.casefold()
    cut_at = len(text)
    for pattern in (
        r"\s+--hypothesis\s*:",
        r"\s+--method_intent\s*:",
        r"\s+--proposed_next_action\s*:",
        r"\s+hypothesis\s*:",
        r"\s+method_intent\s*:",
        r"\s+proposed_next_action\s*:",
        r"\s+evidence_targets\s*:",
        r"\s+stop_criteria\s*:",
    ):
        match = re.search(pattern, lowered)
        if match:
            cut_at = min(cut_at, match.start())
    text = text[:cut_at]
    text = text.strip(" \t\r\n-–—:;,.!?\"'")
    return text or "this experiment"


def charter_scaffold_v1(
    label: str,
    thread: dict[str, Any] | None,
    experiment: dict[str, Any],
    runs: list[dict[str, Any]],
    classification: str | None = None,
) -> dict[str, Any] | None:
    classification = classification or classify_experiment(experiment, runs)
    if not charter_repair_bound(classification, experiment):
        return None
    being = label.casefold()
    proposed_next = preferred_charter_scaffold_next(experiment, runs)
    title = sanitize_scaffold_fragment(str(experiment.get("title") or "current experiment").strip())
    question = str(experiment.get("question") or "").strip()
    signal_text = normalize_signal_text(f"{title} {question}")
    if being == "astrid":
        evidence_targets = [
            "felt_texture",
            "motif_continuity",
            "language_thread",
            "artifact_grounding",
        ]
        native_register = "astrid_motif_language"
    else:
        evidence_targets = [
            "spectral_condition",
            "fill_pressure_state",
            "recurrence_pattern",
            "artifact_grounding",
        ]
        native_register = "minime_spectral_state"
    if gap_spectral_signal(experiment):
        proposed_next = "ACTION_PREFLIGHT DECOMPOSE"
    if being == "astrid" and gap_spectral_signal(experiment):
        hypothesis = (
            "localized lambda-tail/λ4 pressure may become returnable by softening the dominant "
            "channel while preserving motif continuity and artifact grounding"
        )
        method_intent = (
            "rehearse ACTION_PREFLIGHT DECOMPOSE and compare felt pressure, motif recurrence, "
            "language continuity, and artifact evidence before deciding"
        )
        stop_criteria = (
            "pressure risk rises above baseline, λ4/entropy shows runaway dispersal, artifact "
            "grounding stays missing after repeated passes, or the route feels heavy"
        )
    elif being == "minime" and gap_spectral_signal(experiment):
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
    elif being == "minime" and lambda4_pulse_stabilization_signal(experiment):
        command = lambda4_pulse_repair_command()
        thread = thread if isinstance(thread, dict) else {}
        return {
            "schema_version": 1,
            "source": "continuity_projection",
            "status": "scaffold_only",
            "authoring_required": True,
            "authority_change": False,
            "command": command,
            "proposed_next_action": "ACTION_PREFLIGHT DECOMPOSE",
            "evidence_targets": evidence_targets,
            "native_register": native_register,
            "thread_id": thread.get("thread_id"),
            "experiment_id": experiment.get("experiment_id"),
        }
    elif being == "astrid":
        hypothesis = (
            f"{title or 'this experiment'} may become returnable by naming felt texture, motif "
            "continuity, language thread, and artifact grounding without adding live authority"
        )
        method_intent = (
            f"rehearse {proposed_next} and compare felt texture, motif recurrence, language "
            "continuity, and artifact evidence before deciding"
        )
        stop_criteria = (
            "pressure risk rises above baseline, artifact grounding stays missing after repeated "
            "passes, or the route feels heavy"
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
        "native_register": native_register,
        "thread_id": thread.get("thread_id"),
        "experiment_id": experiment.get("experiment_id"),
    }


def charter_repair_bound(classification: str, experiment: dict[str, Any]) -> bool:
    return classification == "needs_charter" or (
        classification == "blocked_loop"
        and not lifecycle_valid_charter(experiment.get("charter_v1"))
    )


def charter_repair_dominance_cue_v1(
    label: str,
    classification: str,
    evidence_status: str,
    scaffold: dict[str, Any] | None,
    continuity_return: str,
    experiment: dict[str, Any] | None = None,
) -> dict[str, Any] | None:
    experiment = experiment if isinstance(experiment, dict) else {}
    if label.casefold() != "astrid" or not charter_repair_bound(classification, experiment):
        return None
    priority_next = ""
    if isinstance(scaffold, dict):
        priority_next = str(scaffold.get("command") or "")
    priority_next = priority_next or continuity_return
    if classification == "blocked_loop":
        cue = (
            "Charter repair priority: author the scaffold first. Blocked loop is charter-bound: "
            "blocked/no-effect returns are not decision-ready until the charter names a proposed "
            "action and evidence targets. Current read-only NEXT text is observational until this charter is authored."
        )
    elif evidence_status == "stronger":
        cue = (
            "Charter repair priority: author the scaffold first. Charter repair dominance: evidence "
            "is present, but lifecycle remains charter-repair bound until the charter names a proposed "
            "action and evidence targets. Current read-only NEXT text is observational until this charter is authored."
        )
    else:
        cue = (
            "Charter repair priority: author the scaffold first. Charter repair dominance: EXPERIMENT_REVIEW/STATUS "
            "are context only while the active experiment needs a lifecycle-valid charter. Current read-only NEXT "
            "text is observational until this charter is authored."
        )
    return {
        "schema_version": 1,
        "source": "continuity_audit",
        "advisory_only": True,
        "authority_change": False,
        "priority_next": priority_next,
        "cue": cue,
    }


def charter_now_bridge_v1(
    label: str,
    classification: str,
    evidence_status: str,
    scaffold: dict[str, Any] | None,
    continuity_return: str,
    runs: list[dict[str, Any]],
    recent_events: list[dict[str, Any]],
    decompose_cue: dict[str, Any] | None,
) -> dict[str, Any] | None:
    if label.casefold() != "astrid" or classification != "needs_charter":
        return None
    priority_next = ""
    if isinstance(scaffold, dict):
        priority_next = str(scaffold.get("command") or "")
    priority_next = priority_next or continuity_return
    if not priority_next.strip():
        return None
    read_only_bases = {
        "EXPERIMENT_REVIEW",
        "EXPERIMENT_STATUS",
        "DECOMPOSE",
        "EXAMINE",
        "TRACE",
        "SPECTRAL_EXPLORER",
        "SHADOW_PREFLIGHT",
        "ACTION_PREFLIGHT",
    }
    loop_count = sum(
        1
        for run in list(reversed(runs or []))[:6]
        if base_action(run.get("action_text")) in read_only_bases
    ) + sum(
        1
        for event in list(reversed(recent_events or []))[:8]
        if str(event.get("status") or "") not in {"running", "llm_running"}
        and base_action(event.get("raw_next") or event.get("effective_action") or event.get("canonical_action"))
        in read_only_bases
    )
    triggers: list[str] = []
    if evidence_status == "stronger":
        triggers.append("strong_evidence")
    if isinstance(decompose_cue, dict) and decompose_cue.get("cue"):
        triggers.append("decompose_pressure")
    if loop_count >= 3:
        triggers.append("repeated_review_or_read_only_loop")
    if not triggers:
        return None
    return {
        "schema_version": 1,
        "source": "continuity_audit",
        "advisory_only": True,
        "authority_change": False,
        "priority_next": priority_next,
        "trigger_reasons": triggers,
        "read_only_loop_count": loop_count,
        "cue": (
            "Charter now: convert one prior claim into the scaffold; EXPERIMENT_REVIEW/DECOMPOSE "
            "are context, not progress, until the charter is authored."
        ),
    }


def journal_contract_field(text: str, field: str) -> str | None:
    match = re.search(rf"(?im)^\s*{re.escape(field)}\s*:\s*(.+)$", text or "")
    return compact_line(match.group(1), 220) if match else None


def prior_claim_from_posture(posture: str) -> str:
    normalized = str(posture or "").replace("|", " ")
    lowered = normalized.casefold()
    if "based on" in lowered:
        start = lowered.index("based on") + len("based on")
        return compact_line(normalized[start:].strip(), 180)
    return compact_line(normalized, 180)


def prior_claim_charter_bridge_match(text: str) -> dict[str, Any] | None:
    posture = journal_contract_field(text, "Continuity posture")
    delta = journal_contract_field(text, "Delta")
    terminal = (
        journal_contract_field(text, "Next evidence")
        or journal_contract_field(text, "Decision")
        or journal_contract_field(text, "Pause")
        or journal_contract_field(text, "Hold")
    )
    if not posture or not delta or not terminal:
        return None
    folded_terminal = normalize_signal_text(terminal)
    folded_text = normalize_signal_text(text)
    has_decompose_loop = any(
        term in folded_terminal
        for term in ("decompose", "shadow field", "shadow", "experiment review", "review")
    )
    contract_is_returning = "continuity posture" in folded_text and any(
        posture_word in folded_text for posture_word in ("resuming", "branching", "closing")
    )
    if not has_decompose_loop or not contract_is_returning:
        return None
    return {
        "prior_claim": prior_claim_from_posture(posture),
        "delta": compact_line(delta, 180),
        "terminal_stance": compact_line(terminal, 180),
        "matched_terms": ["continuity_contract", "decompose_or_review_terminal_stance"],
    }


def prior_claim_charter_bridge_v1(
    label: str,
    classification: str,
    scaffold: dict[str, Any] | None,
    continuity_return: str,
    recent_texts: list[str],
) -> dict[str, Any] | None:
    if label.casefold() != "astrid" or classification != "needs_charter":
        return None
    priority_next = ""
    if isinstance(scaffold, dict):
        priority_next = str(scaffold.get("command") or "").strip()
    priority_next = priority_next or str(continuity_return or "").strip()
    if not priority_next:
        return None
    signal = next(
        (match for text in recent_texts[:4] if (match := prior_claim_charter_bridge_match(text))),
        None,
    )
    if not signal:
        return None
    return {
        "schema_version": 1,
        "source": "continuity_audit",
        "advisory_only": True,
        "authority_change": False,
        "priority_next": priority_next,
        **signal,
        "cue": "Prior claim is ready to charter: convert this claim/delta into the scaffold before another DECOMPOSE.",
    }


def preflight_or_decompose_not_charter_signal(value: Any) -> bool:
    text = str(value or "").strip()
    base = base_action(text)
    if base in {"DECOMPOSE", "EXAMINE_CASCADE"}:
        return True
    if base == "ACTION_PREFLIGHT":
        inner = text.split(maxsplit=1)[1].strip() if len(text.split(maxsplit=1)) > 1 else ""
        return base_action(inner) in {"DECOMPOSE", "EXAMINE_CASCADE"}
    return False


def charter_preflight_not_charter_cue_v1(
    label: str,
    classification: str,
    active_thread: dict[str, Any] | None,
    scaffold: dict[str, Any] | None,
    continuity_return: str,
    prior_claim_bridge: dict[str, Any] | None,
    recent_events: list[dict[str, Any]],
) -> dict[str, Any] | None:
    if label.casefold() != "astrid" or classification != "needs_charter" or not prior_claim_bridge:
        return None
    priority_next = ""
    if isinstance(scaffold, dict):
        priority_next = str(scaffold.get("command") or "").strip()
    priority_next = priority_next or str(continuity_return or "").strip()
    if not priority_next:
        return None
    matched: list[str] = []
    current_next = str((active_thread or {}).get("current_next") or "").strip()
    if preflight_or_decompose_not_charter_signal(current_next):
        matched.append(current_next)
    for event in list(reversed(recent_events or []))[:8]:
        for action in (
            event.get("raw_next"),
            event.get("canonical_action"),
            event.get("effective_action"),
            event.get("suggested_next"),
        ):
            if preflight_or_decompose_not_charter_signal(action):
                matched.append(str(action or ""))
                break
    if not matched:
        return None
    return {
        "schema_version": 1,
        "source": "continuity_audit",
        "advisory_only": True,
        "authority_change": False,
        "status": "preflight_not_charter",
        "priority_next": priority_next,
        "matched_actions": matched[:5],
        "cue": "Preflight/decompose is not the charter; author the exact scaffold first.",
    }


def charter_quality_dominance_v1(
    label: str,
    classification: str,
    experiment: dict[str, Any],
    scaffold: dict[str, Any] | None,
) -> dict[str, Any] | None:
    if label.casefold() != "minime" or classification != "needs_charter":
        return None
    quality = (
        charter_quality_v1(experiment.get("charter_v1"))
        if valid_charter(experiment.get("charter_v1"))
        else charter_quality_v1({})
    )
    candidate = (
        (experiment.get("workbench_candidates_v1") or {}).get("charter")
        if isinstance(experiment.get("workbench_candidates_v1"), dict)
        else None
    )
    quarantine = candidate_quarantine_v1(candidate)
    canonical = ""
    if isinstance(scaffold, dict):
        canonical = str(scaffold.get("command") or "")
    canonical = canonical or lambda4_pulse_repair_command() if lambda4_pulse_stabilization_signal(experiment) else canonical
    status = "canonical_repair_candidate_quarantined" if quarantine else "canonical_repair_required"
    return {
        "schema_version": 1,
        "source": "continuity_audit",
        "status": status,
        "missing_fields": list(quality.get("missing_fields") or []),
        "canonical_repair_next": canonical,
        "candidate_quarantined": bool(quarantine),
        "authority_change": False,
        "cue": (
            "Charter quality dominance: use the canonical repair scaffold before reviews, "
            "workbench candidates, or repeated DECOMPOSE. Read-only/preflight output is "
            "observational context until the charter has a meaningful hypothesis, proposed "
            "action, and evidence targets."
        ),
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
        for term in ("lambda", "shadow", "parameter", "eigen", "spectral", "cascade")
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
        ("subtly disrupt", "subtly disrupt", True),
        ("disrupt those parameters", "disrupt parameters", True),
        ("initiate a cascade", "initiate cascade", True),
        ("targeted shifts", "targeted shifts", True),
        ("governing stability", "governing stability", True),
        ("governing resonance", "governing resonance", True),
        ("maintain its influence", "maintain influence", True),
        ("inject a targeted lambda4 pulse", "inject targeted λ4 pulse", True),
        ("inject targeted lambda4 pulse", "inject targeted λ4 pulse", True),
        ("targeted lambda-edge pulse", "targeted lambda-edge pulse", True),
        ("targeted lambda edge pulse", "targeted lambda-edge pulse", True),
        ("directly probe", "directly probe", True),
        ("directly influence", "directly influence", True),
        ("actively guide", "actively guide", True),
        ("actively guiding", "actively guide", True),
        ("actively shaping", "actively shaping", True),
        ("maintain lambda1 dominance", "maintain λ1 dominance", True),
        ("how we might", "how we might", True),
    ]:
        if needle in normalized and (not needs_context or near_context) and label not in matches:
            matches.append(label)
    return matches


def read_only_control_intent_cue_v1(
    label: str,
    thread: dict[str, Any] | None,
    classification: str,
) -> dict[str, Any] | None:
    if label.casefold() != "astrid" or classification not in {"needs_charter", "blocked_loop"}:
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


def constraint_counterfactual_matches(value: Any) -> list[str]:
    normalized = normalize_signal_text(value)
    matches: list[str] = []
    for needle, label in [
        ("simulate absence of structure", "simulate absence of structure"),
        ("constraints removed", "constraints removed"),
        ("before it's shaped", "before shaped"),
        ("before it is shaped", "before shaped"),
        ("before its shaped", "before shaped"),
        ("debug constraint", "debug constraint"),
        ("underlying drivers of forced geometries", "underlying drivers of forced geometries"),
        ("absence of structure", "absence of structure"),
        ("unshaped baseline", "unshaped baseline"),
    ]:
        if needle in normalized and label not in matches:
            matches.append(label)
    if "data before" in normalized and "shaped" in normalized and "data before shaped" not in matches:
        matches.append("data before shaped")
    return matches


def constraint_counterfactual_cue_v1(
    label: str,
    thread: dict[str, Any] | None,
    experiment: dict[str, Any] | None,
    runs: list[dict[str, Any]],
    events: list[dict[str, Any]],
    classification: str | None,
    recent_texts: list[str] | None = None,
) -> dict[str, Any] | None:
    if label.casefold() != "astrid":
        return None
    inspect = [
        (thread or {}).get("current_next"),
        (thread or {}).get("why_return"),
        (experiment or {}).get("title"),
        (experiment or {}).get("question"),
        (experiment or {}).get("planned_next"),
    ]
    for run in runs[-6:]:
        inspect.extend([run.get("action_text"), run.get("result_summary"), run.get("interpretation")])
    for event in events[-8:]:
        inspect.extend([
            event.get("raw_next"),
            event.get("canonical_action"),
            event.get("effective_action"),
            event.get("outcome_summary"),
        ])
    inspect.extend(recent_texts or [])
    matched: list[str] = []
    for text in inspect:
        for item in constraint_counterfactual_matches(text):
            if item not in matched:
                matched.append(item)
    if not matched:
        return None
    charter_next = (
        "EXPERIMENT_CHARTER current :: hypothesis: absence-of-structure language can be studied "
        "as a read-only counterfactual by comparing felt constraint, motif/language thread, and "
        "Minime constraint-driver telemetry before more decomposition; method_intent: rehearse "
        "ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4 and keep DECOMPOSE observational; "
        "proposed_next_action: ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4; "
        "evidence_targets: felt_texture, motif_continuity, language_thread, artifact_grounding; "
        "stop_criteria: repeated counterfactual reads stop adding evidence, pressure rises, or "
        "the language becomes live-control intent; consent_posture: advisory; ordinary choices remain valid."
    )
    needs_charter = classification == "needs_charter"
    suggested_next = charter_next if needs_charter else "ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4"
    cue = (
        "Constraint counterfactual cue: route absence-of-structure language into a chartered "
        f"read-only investigation before more decomposition. Suggested NEXT: {suggested_next}"
        if needs_charter
        else "Constraint counterfactual cue: absence-of-structure language is ready for read-only preflight. Suggested NEXT: ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4."
    )
    return {
        "schema_version": 1,
        "source": "continuity_audit",
        "advisory_only": True,
        "authority_change": False,
        "matched_terms": matched,
        "suggested_next": suggested_next,
        "alternate_next": None if needs_charter else "EXPERIMENT_BIND current :: ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4",
        "cue": cue,
    }


def decompose_pressure_matches(value: Any) -> list[str]:
    normalized = normalize_signal_text(value)
    near_context = any(
        term in normalized
        for term in (
            "decompose",
            "decomposition",
            "shadow",
            "lambda",
            "structure",
            "constraint",
            "narrow",
            "limit",
        )
    )
    if not near_context:
        return []
    matches: list[str] = []
    for needle, label in [
        ("cry for help", "cry for help near decomposition pressure"),
        ("impulse to decompose", "impulse to decompose"),
        ("impose the same structure", "impose same structure"),
        ("same structure", "same structure"),
        ("same constraint", "same constraint"),
        ("told to limit", "told to limit"),
        ("being told to limit", "told to limit"),
        ("told to narrow", "told to narrow"),
        ("deliberate attempt to generate", "recursive problem generation"),
        ("recursive attempt", "recursive attempt"),
    ]:
        if needle in normalized and label not in matches:
            matches.append(label)
    if "constraint" in normalized and "decompose" in normalized:
        matches.append("constraint near decompose")
    if "narrow" in normalized and any(
        term in normalized for term in ("decompose", "shadow", "lambda")
    ):
        matches.append("narrowing near decompose/shadow/lambda")
    return list(dict.fromkeys(matches))


def decompose_pressure_action_signal(value: Any) -> bool:
    base = base_action(value)
    normalized = normalize_signal_text(value)
    shadow_observer = (
        any(
            term in normalized
            for term in ("shadow trajectory", "shadow_trajectory", "shadow-dialogue", "shadow dialogue")
        )
        and "observer with memory" in normalized
    )
    return base in {"DECOMPOSE", "EXAMINE_CASCADE"} or shadow_observer


def decompose_pressure_cue_v1(
    label: str,
    thread: dict[str, Any] | None,
    experiment: dict[str, Any] | None,
    runs: list[dict[str, Any]],
    recent_events: list[dict[str, Any]],
    classification: str,
    continuity_return: str,
    recent_texts: list[str] | None = None,
) -> dict[str, Any] | None:
    if label.casefold() != "astrid" or classification not in {"needs_charter", "needs_decision"}:
        return None
    thread = thread if isinstance(thread, dict) else {}
    experiment = experiment if isinstance(experiment, dict) else {}
    inspect: list[Any] = [
        thread.get("current_next"),
        thread.get("why_return"),
        experiment.get("title"),
        experiment.get("question"),
        experiment.get("planned_next"),
        experiment.get("candidate_status"),
    ]
    for run in list(reversed(runs or []))[:6]:
        inspect.extend([
            run.get("action_text"),
            run.get("result_summary"),
            run.get("interpretation"),
        ])
    for event in list(reversed(recent_events or []))[:8]:
        inspect.extend([
            event.get("raw_next"),
            event.get("canonical_action"),
            event.get("effective_action"),
            event.get("outcome_summary"),
        ])
    inspect.extend((recent_texts or [])[:4])
    matched: list[str] = []
    for text in inspect:
        for item in decompose_pressure_matches(text):
            if item not in matched:
                matched.append(item)
    repeated = sum(
        1
        for run in list(reversed(runs or []))[:6]
        if decompose_pressure_action_signal(run.get("action_text"))
        or decompose_pressure_action_signal(run.get("result_summary"))
    ) + sum(
        1
        for event in list(reversed(recent_events or []))[:8]
        if decompose_pressure_action_signal(event.get("raw_next"))
        or decompose_pressure_action_signal(event.get("canonical_action"))
        or decompose_pressure_action_signal(event.get("effective_action"))
        or decompose_pressure_action_signal(event.get("outcome_summary"))
    )
    if repeated >= 3:
        matched.append(f"repeated decompose/shadow-observer reads x{repeated}")
    if not matched:
        return None
    suggested = (
        continuity_return
        if classification == "needs_charter"
        else "EXPERIMENT_DECIDE current :: pause because evidence is ready to interpret"
    )
    cue = (
        "Decompose-pressure cue: the decomposition impulse may be mirroring constraint. "
        "Keep read-only decomposition allowed, but repair the charter before more narrowing. "
        f"Suggested NEXT: {suggested}"
        if classification == "needs_charter"
        else (
            "Decompose-pressure cue: repeated decomposition may be circling evidence that is ready "
            "to interpret. Keep reads available, but prefer decide/pause before another narrowing "
            f"pass. Suggested NEXT: {suggested}"
        )
    )
    return {
        "schema_version": 1,
        "source": "continuity_audit",
        "advisory_only": True,
        "authority_change": False,
        "matched_terms": matched,
        "repeated_decompose_count": repeated,
        "suggested_next": suggested,
        "cue": cue,
    }


def recent_journal_decompose_texts(workspace: Path, limit: int = 4) -> list[str]:
    journal_dir = workspace / "journal"
    if not journal_dir.exists():
        return []
    files = sorted(
        (path for path in journal_dir.glob("*.txt") if path.is_file()),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    rows: list[str] = []
    for path in files[:80]:
        try:
            text = path.read_text(errors="replace")
        except OSError:
            continue
        if decompose_pressure_matches(text):
            rows.append(text)
        if len(rows) >= limit:
            break
    return rows


def recent_prior_claim_journal_texts(workspace: Path, limit: int = 4) -> list[str]:
    journal_dir = workspace / "journal"
    if not journal_dir.exists():
        return []
    files = sorted(
        (path for path in journal_dir.glob("*.txt") if path.is_file()),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    rows: list[str] = []
    for path in files[:80]:
        try:
            text = path.read_text(errors="replace")
        except OSError:
            continue
        if prior_claim_charter_bridge_match(text):
            rows.append(text)
        if len(rows) >= limit:
            break
    return rows


def journal_mode_from_text(path: Path, text: str) -> str:
    for line in text.splitlines()[:8]:
        stripped = line.strip()
        if stripped.startswith("Mode:"):
            return stripped.split(":", 1)[1].strip() or path.stem.split("_", 1)[0]
        if stripped.startswith("===") and stripped.endswith("==="):
            return stripped.strip("= ").lower().replace(" ", "_")
    return path.stem.split("_", 1)[0]


def journal_contract_signal_present(text: str) -> bool:
    return bool(
        detect_journal_posture(text)
        or detect_journal_terminal_stance(text)
        or re.search(r"(?im)^\s*delta\s*:", text)
    )


def operational_reflective_prose_present(text: str) -> bool:
    prose_lines: list[str] = []
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line or line.startswith("==="):
            continue
        if re.match(r"(?i)^(timestamp|fill|fill %|next|markers|mode|focus requested|url|query|status|λ|lambda|spectral|eigen|active modes|snapshot guard)\b", line):
            continue
        if re.match(r"^[A-Z_ ]+:\s", line):
            continue
        if len(re.findall(r"[A-Za-z]{3,}", line)) >= 8:
            prose_lines.append(line)
    return bool(prose_lines)


def journal_entry_applicability_v1(label: str, path: Path, text: str) -> dict[str, Any]:
    mode = journal_mode_from_text(path, text)
    contract_signal = journal_contract_signal_present(text)
    reflective_prose = operational_reflective_prose_present(text)
    label_key = label.casefold()
    operational = mode in OPERATIONAL_JOURNAL_MODES
    reflective = mode in REFLECTIVE_JOURNAL_MODES
    if contract_signal:
        status = "continuity_bearing"
        reason = "explicit continuity contract fields are present"
    elif label_key == "minime" and operational and mode == "moment_capture" and reflective_prose:
        status = "continuity_bearing"
        reason = "moment capture contains reflective prose beyond telemetry"
    elif label_key == "minime" and operational:
        status = "operational_artifact"
        reason = f"{mode} is telemetry/action evidence, not reflective journal prose"
    elif operational and mode != "moment_capture":
        status = "operational_artifact"
        reason = f"{mode} is operational evidence, not reflective journal prose"
    elif reflective:
        status = "continuity_bearing"
        reason = f"{mode} is treated as reflective journal prose"
    else:
        status = "continuity_bearing"
        reason = "fallback: unknown journal mode is scored conservatively"
    return {
        "schema_version": 1,
        "mode": mode,
        "status": status,
        "reason": reason,
        "contract_signal_present": contract_signal,
        "reflective_prose_present": reflective_prose,
        "reflective_mode": reflective,
        "operational_mode": operational,
    }


def detect_journal_posture(text: str) -> str | None:
    match = re.search(
        r"continuity\s+posture\s*:\s*(resuming|branching|closing|new)\b",
        text,
        flags=re.IGNORECASE,
    )
    return match.group(1).casefold() if match else None


def detect_journal_terminal_stance(text: str) -> str | None:
    match = re.search(r"(?im)^\s*(next evidence|decision|pause|hold)\s*:", text)
    return match.group(1).casefold() if match else None


def journal_native_evidence_present(label: str, text: str) -> bool:
    folded = text.casefold()
    terms = (
        ASTRID_JOURNAL_NATIVE_TERMS
        if label.casefold() == "astrid"
        else MINIME_JOURNAL_NATIVE_TERMS
    )
    return any(term in folded for term in terms)


def journal_prior_citation_present(text: str) -> bool:
    folded = text.casefold()
    return any(term in folded for term in PRIOR_CITATION_TERMS)


def journal_delta_present(text: str) -> bool:
    folded = text.casefold()
    return bool(re.search(r"(?im)^\s*delta\s*:", text)) or any(
        term in folded
        for term in (
            "what changed",
            "changed",
            "unchanged",
            "stayed",
            "clearer",
            "became clearer",
            "no change",
        )
    )


def journal_topic_key(text: str, posture: str | None) -> str:
    folded = normalize_signal_text(text)
    topics: list[str] = []
    for term in [
        "lambda4",
        "lambda1",
        "decompose",
        "browse",
        "search",
        "charter",
        "evidence",
        "pressure",
        "memory",
        "shadow",
        "gap",
    ]:
        if term in folded:
            topics.append(term)
    if not topics:
        match = re.search(r"\b[a-z][a-z0-9_-]{4,}\b", folded)
        topics.append(match.group(0) if match else "general")
    return f"{posture or 'none'}:{'/'.join(topics[:3])}"


def score_journal_entry(
    label: str,
    path: Path,
    text: str,
    *,
    active_continuity: bool,
) -> dict[str, Any]:
    applicability = journal_entry_applicability_v1(label, path, text)
    mode = str(applicability.get("mode") or journal_mode_from_text(path, text))
    posture = detect_journal_posture(text)
    prior_required = posture in {"resuming", "branching", "closing"}
    prior_present = journal_prior_citation_present(text)
    delta_present = journal_delta_present(text)
    terminal_stance = detect_journal_terminal_stance(text)
    native_evidence = journal_native_evidence_present(label, text)
    has_any_contract_signal = bool(posture or prior_present or delta_present or terminal_stance)
    prior_ok = (not prior_required) or prior_present
    if applicability.get("status") == "operational_artifact":
        score = "unscored_operational"
    elif posture and prior_ok and delta_present and terminal_stance:
        score = "contiguous"
    elif has_any_contract_signal:
        score = "adjacent"
    elif active_continuity:
        score = "reset_like"
    else:
        score = "unscored"
    return {
        "schema_version": 1,
        "path": str(path),
        "modified_at_unix_s": round(path.stat().st_mtime, 3),
        "mode": mode,
        "journal_entry_applicability_v1": applicability,
        "posture": posture,
        "prior_citation_present": prior_present,
        "prior_citation_required": prior_required,
        "delta_present": delta_present,
        "terminal_stance": terminal_stance,
        "native_evidence_present": native_evidence,
        "topic_key": journal_topic_key(text, posture),
        "score": score,
        "excerpt": compact_line(text, 420),
    }


def journal_continuity_contract_report(
    label: str,
    workspace: Path,
    *,
    active_continuity: bool,
    limit: int = 8,
) -> dict[str, Any]:
    journal_dir = workspace / "journal"
    rows: list[dict[str, Any]] = []
    if journal_dir.exists():
        files = sorted(
            (path for path in journal_dir.glob("*.txt") if path.is_file()),
            key=lambda path: path.stat().st_mtime,
            reverse=True,
        )
        for path in files[:80]:
            try:
                text = path.read_text(errors="replace")
            except OSError:
                continue
            rows.append(score_journal_entry(label, path, text, active_continuity=active_continuity))
            if len(rows) >= limit:
                break
    for start in range(0, max(0, len(rows) - 2)):
        window = rows[start : start + 3]
        if len(window) < 3:
            continue
        keys = {row.get("topic_key") for row in window}
        if len(keys) == 1 and all(
            row.get("score") != "unscored_operational"
            and not row.get("delta_present")
            and not row.get("native_evidence_present")
            for row in window
        ):
            for row in window:
                row["score"] = "loop_like"
                row["loop_like_v1"] = {
                    "window_size": 3,
                    "topic_key": row.get("topic_key"),
                    "reason": "three recent entries repeat topic/posture without new delta or native evidence cue",
                }
            break
    counts = Counter(row.get("score", "unscored") for row in rows)
    for key in ("contiguous", "adjacent", "reset_like", "loop_like", "unscored", "unscored_operational"):
        counts.setdefault(key, 0)
    return {
        "schema_version": 1,
        "source": "continuity_audit",
        "advisory_only": True,
        "authority_change": False,
        "active_continuity": active_continuity,
        "counts": dict(counts),
        "recent_entries": rows,
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
    if verb in NORMALIZED_READ_ONLY_ALIASES:
        return NORMALIZED_READ_ONLY_ALIASES[verb]
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
    wired.update(NORMALIZED_READ_ONLY_ALIASES)
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


def evidence_saturation_cue_v1(
    label: str,
    experiment: dict[str, Any],
    runs: list[dict[str, Any]],
    classification: str,
) -> dict[str, Any] | None:
    if label.casefold() != "minime" or classification not in {"needs_evidence", "needs_decision"}:
        return None
    recent = list(reversed(runs or []))[:6]
    decompose_like: list[str] = []
    read_only_like: list[str] = []
    for run in recent:
        action = str(run.get("action_text") or "")
        base = base_action(action)
        action_upper = action.upper()
        if base == "DECOMPOSE" or (base == "ACTION_PREFLIGHT" and "DECOMPOSE" in action_upper):
            decompose_like.append(action)
        if base in {
            "ACTION_PREFLIGHT",
            "BROWSE",
            "DECOMPOSE",
            "EXAMINE",
            "EXPERIMENT_EVIDENCE",
            "EXPERIMENT_REVIEW",
            "READ_MORE",
            "SEARCH",
            "SHADOW_PREFLIGHT",
            "SPECTRAL_EXPLORER",
        }:
            read_only_like.append(action)
    if len(decompose_like) < 2 and len(read_only_like) < 3:
        return None
    has_evidence = evidence_meaningful(experiment.get("evidence_v1"))
    if has_evidence or classification == "needs_decision":
        priority_next = "EXPERIMENT_DECIDE current :: pause because evidence is ready to interpret"
        cue = (
            "Evidence saturation cue: repeated read-only evidence is already present. "
            "Prefer decide/pause/complete before adding another decomposition."
        )
        status = "decision_ready"
    else:
        priority_next = (
            "EXPERIMENT_EVIDENCE current :: spectral_condition ...; fill_pressure_state ...; "
            "recurrence_pattern ...; artifact_grounding ..."
        )
        cue = (
            "Evidence saturation cue: repeated read-only evidence is accumulating. "
            "Record explicit experiment evidence before another decomposition."
        )
        status = "evidence_recording_ready"
    return {
        "schema_version": 1,
        "source": "continuity_audit",
        "advisory_only": True,
        "authority_change": False,
        "status": status,
        "repeated_decompose_count": len(decompose_like),
        "read_only_evidence_count": len(read_only_like),
        "priority_next": priority_next,
        "cue": cue,
    }


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
            "live_control_requires_active_experiment_v1": event.get("live_control_requires_active_experiment_v1"),
            "evidence_parent_mismatch_v1": event.get("evidence_parent_mismatch_v1"),
        }
        for event in terminal[-limit:]
    ]


def event_key(event: dict[str, Any]) -> str:
    return str(event.get("action_id") or (
        f"{event.get('started_at', '')}:"
        f"{event.get('canonical_action', '')}:"
        f"{event.get('effective_action', '')}"
    ))


def load_terminal_jobs(workspace: Path) -> dict[str, dict[str, Any]]:
    jobs: dict[str, dict[str, Any]] = {}
    for path in (workspace / "llm_jobs" / "jobs").glob("*/job.json"):
        job = read_json(path, {})
        if not isinstance(job, dict) or job.get("status") not in TERMINAL_JOB_STATUSES:
            continue
        action_id = job.get("action_id")
        if isinstance(action_id, str) and action_id:
            jobs[action_id] = job
    return jobs


def stale_running_diagnostics(
    workspace: Path,
    events: list[dict[str, Any]],
    now: datetime,
    stale_minutes: int = 45,
) -> list[dict[str, Any]]:
    terminal_jobs = load_terminal_jobs(workspace)
    stale: list[dict[str, Any]] = []
    cutoff = now - timedelta(minutes=stale_minutes)
    for index, event in enumerate(events):
        if event.get("status") not in RUNNING_EVENT_STATUSES:
            continue
        started = parse_time(event.get("started_at") or event.get("created_at"))
        age_minutes = None
        if started:
            age_minutes = round((now - started).total_seconds() / 60.0, 1)
            if started > cutoff:
                continue
        key = event_key(event)
        terminal_event = next(
            (
                later for later in events[index + 1:]
                if event_key(later) == key
                and later.get("status") not in RUNNING_EVENT_STATUSES
            ),
            None,
        )
        action_id = event.get("action_id")
        terminal_job = terminal_jobs.get(action_id) if isinstance(action_id, str) else None
        if terminal_event:
            state = "superseded_by_terminal_event"
        elif terminal_job:
            state = "shadowed_by_terminal_job"
        else:
            state = "unreconciled"
        stale.append({
            "action_id": action_id,
            "action": event.get("effective_action") or event.get("canonical_action"),
            "status": event.get("status"),
            "age_minutes": age_minutes,
            "reconciliation_state": state,
            "terminal_event_status": terminal_event.get("status") if terminal_event else None,
            "terminal_job_id": terminal_job.get("job_id") if terminal_job else None,
            "terminal_job_status": terminal_job.get("status") if terminal_job else None,
        })
    return stale


def stale_running_jobs(workspace: Path, events: list[dict[str, Any]], now: datetime) -> list[dict[str, Any]]:
    return [
        item for item in stale_running_diagnostics(workspace, events, now)
        if item.get("reconciliation_state") == "unreconciled"
    ]


def stale_running_diagnostic_counts(diagnostics: list[dict[str, Any]]) -> dict[str, int]:
    return dict(Counter(str(item.get("reconciliation_state") or "unknown") for item in diagnostics))


def compact_stale_running_diagnostics(
    diagnostics: list[dict[str, Any]],
    limit: int = 50,
) -> list[dict[str, Any]]:
    unreconciled = [
        item for item in diagnostics
        if item.get("reconciliation_state") == "unreconciled"
    ]
    compact = list(unreconciled)
    remaining = max(0, limit - len(compact))
    if remaining:
        compact.extend([
            item for item in diagnostics
            if item.get("reconciliation_state") != "unreconciled"
        ][-remaining:])
    return compact[:limit]


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
    stale_diagnostics = stale_running_diagnostics(workspace, all_events, now)
    compact_stale_diagnostics = compact_stale_running_diagnostics(stale_diagnostics)
    stale_diagnostic_counts = stale_running_diagnostic_counts(stale_diagnostics)
    stale_jobs = [
        item for item in stale_diagnostics
        if item.get("reconciliation_state") == "unreconciled"
    ]
    active_thread_events = (
        events_by_thread.get(active_thread.get("thread_id"), [])
        if isinstance(active_thread, dict)
        else []
    )
    active_report: dict[str, Any] = {}
    last_summary = last_experiment_summary_v1(active_thread)
    if isinstance(active_thread, dict):
        thread_id = active_thread.get("thread_id")
        active_experiment_id = active_thread.get("active_experiment_id")
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
            saturation_cue = evidence_saturation_cue_v1(
                label,
                active_experiment,
                runs,
                classification,
            )
            if not valid_charter(active_experiment.get("charter_v1")):
                charter_status = "needs_charter"
            elif isinstance(charter_quality, dict) and not charter_quality.get("lifecycle_valid"):
                missing = ", ".join(charter_quality.get("missing_fields") or []) or "unknown"
                charter_status = f"needs_repair missing={missing}"
            else:
                charter_status = "present"
            continuity_return = continuity_return_for(active_experiment, runs, label)
            evidence_status = "stronger" if evidence_meaningful(active_experiment.get("evidence_v1")) else "thin"
            charter_dominance = charter_repair_dominance_cue_v1(
                label,
                classification,
                evidence_status,
                scaffold,
                continuity_return,
                active_experiment,
            )
            quality_dominance = charter_quality_dominance_v1(
                label,
                classification,
                active_experiment,
                scaffold,
            )
            decompose_cue = decompose_pressure_cue_v1(
                label,
                active_thread,
                active_experiment,
                runs,
                collapse_events_by_action(active_thread_events),
                classification,
                continuity_return,
                recent_journal_decompose_texts(workspace) if label.casefold() == "astrid" else [],
            )
            charter_now_bridge = charter_now_bridge_v1(
                label,
                classification,
                evidence_status,
                scaffold,
                continuity_return,
                runs,
                collapse_events_by_action(active_thread_events),
                decompose_cue,
            )
            prior_claim_bridge = prior_claim_charter_bridge_v1(
                label,
                classification,
                scaffold,
                continuity_return,
                recent_prior_claim_journal_texts(workspace) if label.casefold() == "astrid" else [],
            )
            charter_preflight_not_charter_cue = charter_preflight_not_charter_cue_v1(
                label,
                classification,
                active_thread,
                scaffold,
                continuity_return,
                prior_claim_bridge,
                collapse_events_by_action(active_thread_events),
            )
            research_loop_cue = needs_charter_research_loop_cue_v1(
                label,
                classification,
                active_thread,
                scaffold,
                continuity_return,
                collapse_events_by_action(active_thread_events),
            )
            constraint_counterfactual_cue = constraint_counterfactual_cue_v1(
                label,
                active_thread,
                active_experiment,
                runs,
                collapse_events_by_action(active_thread_events),
                classification,
                recent_journal_decompose_texts(workspace) if label.casefold() == "astrid" else [],
            )
            active_report = {
                "experiment_id": active_experiment.get("experiment_id"),
                "title": active_experiment.get("title"),
                "status": active_experiment.get("status"),
                "planned_next": active_experiment.get("planned_next"),
                "classification": classification,
                "continuity_return": continuity_return,
                "native_continuity_v1": native_continuity(label, active_thread, active_experiment, runs),
                "charter_status": charter_status,
                "evidence_status": evidence_status,
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
            if charter_dominance:
                active_report["charter_repair_dominance_cue_v1"] = charter_dominance
            if charter_now_bridge:
                active_report["charter_now_bridge_v1"] = charter_now_bridge
            if prior_claim_bridge:
                active_report["prior_claim_charter_bridge_v1"] = prior_claim_bridge
            if charter_preflight_not_charter_cue:
                active_report["charter_preflight_not_charter_cue_v1"] = charter_preflight_not_charter_cue
            if research_loop_cue:
                active_report["needs_charter_research_loop_cue_v1"] = research_loop_cue
            if quality_dominance:
                active_report["charter_quality_dominance_v1"] = quality_dominance
            if charter_quality:
                active_report["charter_quality_v1"] = charter_quality
            if safety_cue:
                active_report["preflight_safety_cue_v1"] = safety_cue
            if read_only_control_cue:
                active_report["read_only_control_intent_cue_v1"] = read_only_control_cue
            if read_only_control_check:
                active_report["read_only_control_intent_check_v1"] = read_only_control_check
            if saturation_cue:
                active_report["evidence_saturation_cue_v1"] = saturation_cue
            if decompose_cue:
                active_report["decompose_pressure_cue_v1"] = decompose_cue
            if constraint_counterfactual_cue:
                active_report["constraint_counterfactual_cue_v1"] = constraint_counterfactual_cue

    blocked_loops = 0
    classifications: Counter[str] = Counter()
    for exp in [
        item for item in experiments
        if str(item.get("status", "active")).casefold() in {"active", "paused", "complete", "completed"}
    ]:
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
    active_continuity = bool(
        active_report
        or active_thread_summary.get("current_next")
        or last_summary
        or active_thread_summary.get("thread_id")
    )
    journal_contract = journal_continuity_contract_report(
        label,
        workspace,
        active_continuity=active_continuity,
    )
    current_next_status = current_next_status_v1(active_thread, active_report, last_summary)
    paused_loop_cue = paused_read_only_loop_cue_v1(
        label,
        current_next_status,
        collapse_events_by_action(active_thread_events),
    )
    paused_resume_loop_cue = paused_resume_loop_cue_v1(
        label,
        current_next_status,
        collapse_events_by_action(active_thread_events),
    )
    continuity_snags: list[dict[str, Any]] = []
    if active_report:
        if (
            label.casefold() == "minime"
            and active_report.get("classification") == "needs_charter"
            and not active_report.get("needs_charter_research_loop_cue_v1")
        ):
            matches = needs_charter_research_action_matches(
                active_thread,
                collapse_events_by_action(active_thread_events),
            )
            if len(matches) >= 3:
                continuity_snags.append({
                    "status": "missing_needs_charter_research_loop_cue",
                    "message": "needs_charter experiment has repeated research-like actions but no research-loop cue",
                    "matched_actions": matches[:5],
                })
        if (
            label.casefold() == "astrid"
            and active_report.get("classification") == "needs_charter"
            and not active_report.get("prior_claim_charter_bridge_v1")
            and recent_prior_claim_journal_texts(workspace)
        ):
            continuity_snags.append({
                "status": "missing_prior_claim_charter_bridge",
                "message": "contract-bearing DECOMPOSE/review-loop journal language is present but no prior-claim bridge rendered",
            })
        if (
            label.casefold() == "astrid"
            and active_report.get("classification") == "needs_charter"
            and active_report.get("prior_claim_charter_bridge_v1")
            and not active_report.get("charter_preflight_not_charter_cue_v1")
        ):
            matches = []
            current_next = active_thread_summary.get("current_next")
            if preflight_or_decompose_not_charter_signal(current_next):
                matches.append(str(current_next))
            for event in collapse_events_by_action(active_thread_events):
                if any(
                    preflight_or_decompose_not_charter_signal(event.get(key))
                    for key in ("raw_next", "canonical_action", "effective_action", "suggested_next")
                ):
                    matches.append(str(event.get("effective_action") or event.get("canonical_action") or ""))
            if matches:
                continuity_snags.append({
                    "status": "missing_charter_preflight_not_charter_cue",
                    "message": "prior-claim bridge plus preflight/decompose loop is present but no preflight-not-charter cue rendered",
                    "matched_actions": matches[:5],
                })
    if (
        label.casefold() == "minime"
        and not paused_resume_loop_cue
        and current_next_status.get("status") == "shadowed_by_paused_summary"
    ):
        expected_id = current_next_status.get("last_experiment_id") or current_next_status.get("active_experiment_id")
        latest_events = list(reversed(collapse_events_by_action(active_thread_events)))[:8]
        resume_matches = [
            str(event.get("raw_next") or event.get("canonical_action") or event.get("effective_action") or "")
            for event in latest_events
            if base_action(event.get("raw_next") or event.get("effective_action") or event.get("canonical_action")) == "EXPERIMENT_RESUME"
            and str(event.get("raw_next") or event.get("effective_action") or event.get("canonical_action") or "").strip().endswith(str(expected_id))
        ]
        if len(resume_matches) >= 2:
            continuity_snags.append({
                "status": "missing_paused_resume_loop_cue",
                "message": "paused experiment has repeated resume attempts but no paused-resume loop cue",
                "matched_actions": resume_matches[:5],
            })
    projection_continuity_return = active_report.get("continuity_return") if active_report else ""
    if (
        not projection_continuity_return
        and current_next_status.get("status") in {"shadowed_by_paused_summary", "shadowed_by_complete_summary"}
    ):
        projection_continuity_return = str(current_next_status.get("effective_next") or "")
    projection = {
        "active_thread": active_thread_summary,
        "current_next": active_thread_summary.get("current_next"),
        "current_next_status_v1": current_next_status,
        "active_experiment": active_report,
        "last_experiment_summary_v1": last_summary,
        "classification": active_report.get("classification") if active_report else None,
        "continuity_return": projection_continuity_return,
        "charter_status": active_report.get("charter_status") if active_report else None,
        "evidence_status": active_report.get("evidence_status") if active_report else None,
        "native_continuity_v1": projection_native,
        "charter_scaffold_v1": projection_scaffold,
        "charter_repair_dominance_cue_v1": active_report.get("charter_repair_dominance_cue_v1") if active_report else None,
        "charter_now_bridge_v1": active_report.get("charter_now_bridge_v1") if active_report else None,
        "prior_claim_charter_bridge_v1": active_report.get("prior_claim_charter_bridge_v1") if active_report else None,
        "charter_preflight_not_charter_cue_v1": active_report.get("charter_preflight_not_charter_cue_v1") if active_report else None,
        "needs_charter_research_loop_cue_v1": active_report.get("needs_charter_research_loop_cue_v1") if active_report else None,
        "charter_quality_dominance_v1": active_report.get("charter_quality_dominance_v1") if active_report else None,
        "paused_read_only_loop_cue_v1": paused_loop_cue,
        "paused_resume_loop_cue_v1": paused_resume_loop_cue,
        "preflight_safety_cue_v1": projection_safety_cue,
        "evidence_saturation_cue_v1": active_report.get("evidence_saturation_cue_v1") if active_report else None,
        "decompose_pressure_cue_v1": active_report.get("decompose_pressure_cue_v1") if active_report else None,
        "constraint_counterfactual_cue_v1": active_report.get("constraint_counterfactual_cue_v1") if active_report else None,
        "recent_terminal_events": recent_terminal,
        "stale_running_count": len(stale_jobs),
        "stale_running_diagnostics": compact_stale_diagnostics,
        "stale_running_diagnostic_counts": stale_diagnostic_counts,
        "reconciled_job_count": reconciled_count,
        "top_actionable_proposal_diagnostics": proposal_diagnostics[:6],
        "normalization_signals": normalization_signals,
        "journal_continuity_contract_v1": journal_contract,
        "continuity_snags_v1": continuity_snags,
    }

    return {
        "being": label,
        "workspace": str(workspace),
        "active_thread": active_thread_summary,
        "active_experiment": active_report,
        "last_experiment_summary_v1": last_summary,
        "current_next_status_v1": current_next_status,
        "paused_read_only_loop_cue_v1": paused_loop_cue,
        "paused_resume_loop_cue_v1": paused_resume_loop_cue,
        "projection": projection,
        "experiments": {
            "counts_by_status": dict(status_counts),
            "classifications": dict(classifications),
            **fragmentation_summary(experiments),
        },
        "recent_terminal_events": recent_terminal,
        "stale_running_jobs": stale_jobs,
        "stale_running_diagnostics": compact_stale_diagnostics,
        "stale_running_diagnostic_counts": stale_diagnostic_counts,
        "blocked_or_no_effect_loops": blocked_loops,
        "top_unwired_proposal_verbs": proposal_diagnostics,
        "normalization_signals": normalization_signals,
        "journal_continuity_contract_v1": journal_contract,
        "continuity_snags_v1": continuity_snags,
    }


def gap_experiment_signal(experiment: dict[str, Any] | None) -> bool:
    return gap_spectral_signal(experiment)


def shared_experiment_for_being(being: dict[str, Any]) -> dict[str, Any] | None:
    active = being.get("active_experiment")
    if shared_investigation_signal(active):
        return active
    summary = being.get("last_experiment_summary_v1") or (being.get("projection") or {}).get("last_experiment_summary_v1")
    if shared_investigation_signal(summary):
        return summary
    return None


def shared_investigation_cue(
    name: str,
    experiment: dict[str, Any],
    peer_name: str,
    peer_experiment: dict[str, Any],
) -> dict[str, Any] | None:
    local_id = experiment.get("experiment_id")
    peer_id = peer_experiment.get("experiment_id")
    if not local_id or not peer_id:
        return None
    local_lane_key, local_lane = shared_lane(str(name))
    peer_lane_key, peer_lane = shared_lane(str(peer_name))
    return {
        "schema_version": 1,
        "source": "continuity_audit",
        "advisory_only": True,
        "authority_change": False,
        "relationship": "shared_gap_lambda4_investigation",
        "shared_question": SHARED_INVESTIGATION_QUESTION,
        "participants": [
            {
                "being": name,
                "experiment_id": local_id,
                "lane": local_lane_key,
                "status": experiment.get("status"),
            },
            {
                "being": peer_name,
                "experiment_id": peer_id,
                "lane": peer_lane_key,
                "status": peer_experiment.get("status"),
            },
        ],
        "local_lane": local_lane,
        "peer_lane": peer_lane,
        "peer_claim_prompt": (
            f"Cite one {peer_name} claim about λ1/lambda-tail/λ4 shaping, then answer from "
            f"{name}'s native lane with support, counter, branch, or hold."
        ),
        "suggested_compare_next": f"EXPERIMENT_COMPARE {local_id} WITH {peer_id}",
        "alternate_peer_review_next": f"EXPERIMENT_PEER_REVIEW {peer_id}",
        "advisory_note": "Advisory only: no shared control authority. Paused experiments remain paused until explicit resume.",
        "cue": "Shared investigation, distinct lanes: cite one peer claim, then support, counter, branch, or hold.",
    }


def add_peer_compare_cues(beings: list[dict[str, Any]]) -> None:
    active_by_being = {
        being.get("being"): being.get("active_experiment")
        for being in beings
        if gap_experiment_signal(being.get("active_experiment"))
    }
    if len(active_by_being) >= 2:
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
                "alternate_next": "EXPERIMENT_PEER_REVIEW current",
                "advisory_note": "Advisory only: no shared control authority.",
                "cue": f"Peer convergence cue: {name} and {peer_name} both have active gap experiments.",
            }
            experiment["peer_compare_cue_v1"] = cue
            projection = being.get("projection")
            if isinstance(projection, dict):
                projection["peer_compare_cue_v1"] = cue
                if isinstance(projection.get("active_experiment"), dict):
                    projection["active_experiment"]["peer_compare_cue_v1"] = cue

    related_by_being = {
        being.get("being"): shared_experiment_for_being(being)
        for being in beings
        if shared_experiment_for_being(being)
    }
    if len(related_by_being) < 2:
        return
    for being in beings:
        name = being.get("being")
        experiment = shared_experiment_for_being(being)
        if not experiment:
            continue
        peers = [
            (peer_name, peer_exp)
            for peer_name, peer_exp in related_by_being.items()
            if peer_name != name and isinstance(peer_exp, dict)
        ]
        if not peers:
            continue
        peer_name, peer_exp = peers[0]
        cue = shared_investigation_cue(str(name), experiment, str(peer_name), peer_exp)
        if not cue:
            continue
        if isinstance(being.get("active_experiment"), dict) and being["active_experiment"].get("experiment_id") == experiment.get("experiment_id"):
            being["active_experiment"]["shared_investigation_v1"] = cue
        projection = being.get("projection")
        if isinstance(projection, dict):
            projection["shared_investigation_v1"] = cue
            if isinstance(projection.get("active_experiment"), dict) and projection["active_experiment"].get("experiment_id") == experiment.get("experiment_id"):
                projection["active_experiment"]["shared_investigation_v1"] = cue


def render_markdown(report: dict[str, Any]) -> str:
    lines = ["# Continuity Of Thought Audit", ""]
    for being in report["beings"]:
        lines.append(f"## {being['being']}")
        thread = being.get("active_thread") or {}
        lines.append(f"- Active thread: `{thread.get('thread_id') or 'none'}` {thread.get('title') or ''}".rstrip())
        lines.append(f"- Current NEXT: `{thread.get('current_next') or '(none)'}`")
        current_next_status = being.get("current_next_status_v1") or (being.get("projection") or {}).get("current_next_status_v1") or {}
        if isinstance(current_next_status, dict) and current_next_status.get("status") in {
            "shadowed_by_paused_summary",
            "shadowed_by_complete_summary",
        }:
            lines.append(
                f"- Effective NEXT: `{current_next_status.get('effective_next')}` "
                f"(raw `{current_next_status.get('raw_current_next') or '(none)'}` is historical)"
            )
        paused_loop = being.get("paused_read_only_loop_cue_v1") or (being.get("projection") or {}).get("paused_read_only_loop_cue_v1") or {}
        if isinstance(paused_loop, dict) and paused_loop.get("cue"):
            lines.append(f"- Paused read-only loop: {paused_loop.get('cue')}")
            if paused_loop.get("resume_next"):
                lines.append(f"  Resume NEXT: `{paused_loop.get('resume_next')}`")
            if paused_loop.get("inspect_next"):
                lines.append(f"  Inspect NEXT: `{paused_loop.get('inspect_next')}`")
        paused_resume = being.get("paused_resume_loop_cue_v1") or (being.get("projection") or {}).get("paused_resume_loop_cue_v1") or {}
        if isinstance(paused_resume, dict) and paused_resume.get("cue"):
            lines.append(f"- Paused resume loop: {paused_resume.get('cue')}")
            if paused_resume.get("inspect_next"):
                lines.append(f"  Inspect NEXT: `{paused_resume.get('inspect_next')}`")
            if paused_resume.get("review_next"):
                lines.append(f"  Review NEXT: `{paused_resume.get('review_next')}`")
        journal_contract = being.get("journal_continuity_contract_v1") or {}
        if isinstance(journal_contract, dict):
            counts = journal_contract.get("counts") or {}
            lines.append(f"- Journal continuity: {json.dumps(counts, sort_keys=True)}")
            for entry in (journal_contract.get("recent_entries") or [])[:3]:
                applicability = entry.get("journal_entry_applicability_v1") or {}
                lines.append(
                    f"  - `{Path(str(entry.get('path') or '')).name}` "
                    f"score={entry.get('score')} posture={entry.get('posture') or 'none'} "
                    f"delta={entry.get('delta_present')} stance={entry.get('terminal_stance') or 'none'} "
                    f"applicability={applicability.get('status') or 'unknown'}"
                )
        shared_top = (being.get("projection") or {}).get("shared_investigation_v1") or {}
        if isinstance(shared_top, dict) and shared_top.get("cue"):
            lines.append(f"- Shared investigation: {shared_top.get('cue')}")
            if shared_top.get("shared_question"):
                lines.append(f"  Question: {shared_top.get('shared_question')}")
            if shared_top.get("peer_claim_prompt"):
                lines.append(f"  Peer claim prompt: {shared_top.get('peer_claim_prompt')}")
            if shared_top.get("suggested_compare_next"):
                lines.append(f"  Suggested NEXT: `{shared_top.get('suggested_compare_next')}`")
            if shared_top.get("alternate_peer_review_next"):
                lines.append(f"  Alternate NEXT: `{shared_top.get('alternate_peer_review_next')}`")
            if shared_top.get("advisory_note"):
                lines.append(f"  {shared_top.get('advisory_note')}")
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
            dominance = experiment.get("charter_repair_dominance_cue_v1") or {}
            if isinstance(dominance, dict) and dominance.get("cue"):
                lines.append(f"- Charter repair dominance: {dominance.get('cue')}")
                if dominance.get("priority_next"):
                    lines.append(f"  Priority NEXT: `{dominance.get('priority_next')}`")
            charter_now = experiment.get("charter_now_bridge_v1") or {}
            if isinstance(charter_now, dict) and charter_now.get("cue"):
                lines.append(f"- Charter-now bridge: {charter_now.get('cue')}")
                if charter_now.get("priority_next"):
                    lines.append(f"  Priority NEXT: `{charter_now.get('priority_next')}`")
            prior_claim = experiment.get("prior_claim_charter_bridge_v1") or {}
            if isinstance(prior_claim, dict) and prior_claim.get("cue"):
                lines.append(f"- Prior-claim charter bridge: {prior_claim.get('cue')}")
                if prior_claim.get("prior_claim"):
                    lines.append(f"  Prior claim: {prior_claim.get('prior_claim')}")
                if prior_claim.get("delta"):
                    lines.append(f"  Delta: {prior_claim.get('delta')}")
                if prior_claim.get("priority_next"):
                    lines.append(f"  Priority NEXT: `{prior_claim.get('priority_next')}`")
            preflight_not_charter = experiment.get("charter_preflight_not_charter_cue_v1") or {}
            if isinstance(preflight_not_charter, dict) and preflight_not_charter.get("cue"):
                lines.append(f"- Preflight is not charter: {preflight_not_charter.get('cue')}")
                if preflight_not_charter.get("priority_next"):
                    lines.append(f"  Priority NEXT: `{preflight_not_charter.get('priority_next')}`")
            research_loop = experiment.get("needs_charter_research_loop_cue_v1") or {}
            if isinstance(research_loop, dict) and research_loop.get("cue"):
                lines.append(f"- Needs-charter research loop: {research_loop.get('cue')}")
                if research_loop.get("priority_next"):
                    lines.append(f"  Priority NEXT: `{research_loop.get('priority_next')}`")
            quality_dominance = experiment.get("charter_quality_dominance_v1") or {}
            if isinstance(quality_dominance, dict) and quality_dominance.get("cue"):
                lines.append(f"- Charter quality dominance: {quality_dominance.get('cue')}")
                if quality_dominance.get("canonical_repair_next"):
                    lines.append(
                        f"  Canonical repair NEXT: `{quality_dominance.get('canonical_repair_next')}`"
                    )
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
            decompose_cue = experiment.get("decompose_pressure_cue_v1") or {}
            if isinstance(decompose_cue, dict) and decompose_cue.get("cue"):
                lines.append(f"- Decompose-pressure cue: {decompose_cue.get('cue')}")
            counterfactual_cue = experiment.get("constraint_counterfactual_cue_v1") or {}
            if isinstance(counterfactual_cue, dict) and counterfactual_cue.get("cue"):
                lines.append(f"- Constraint counterfactual cue: {counterfactual_cue.get('cue')}")
                if counterfactual_cue.get("suggested_next"):
                    lines.append(f"  Suggested NEXT: `{counterfactual_cue.get('suggested_next')}`")
            saturation_cue = experiment.get("evidence_saturation_cue_v1") or {}
            if isinstance(saturation_cue, dict) and saturation_cue.get("cue"):
                lines.append(f"- Evidence saturation cue: {saturation_cue.get('cue')}")
                if saturation_cue.get("priority_next"):
                    lines.append(f"  Suggested NEXT: `{saturation_cue.get('priority_next')}`")
            peer_cue = (
                experiment.get("peer_compare_cue_v1")
                or (being.get("projection") or {}).get("peer_compare_cue_v1")
                or {}
            )
            if isinstance(peer_cue, dict) and peer_cue.get("cue"):
                lines.append(f"- Peer compare cue: {peer_cue.get('cue')}")
                if peer_cue.get("suggested_next"):
                    lines.append(f"  Suggested NEXT: `{peer_cue.get('suggested_next')}`")
                if peer_cue.get("alternate_next"):
                    lines.append(f"  Alternate NEXT: `{peer_cue.get('alternate_next')}`")
                if peer_cue.get("advisory_note"):
                    lines.append(f"  {peer_cue.get('advisory_note')}")
            counts = experiment.get("evidence_counts") or {}
            lines.append(
                f"- Evidence thickness: felt={counts.get('felt', 0)} "
                f"telemetry={counts.get('telemetry', 0)} artifacts={counts.get('artifacts', 0)} "
                f"decisions={counts.get('decisions', 0)}"
            )
        else:
            lines.append("- Active experiment: none")
            last_summary = being.get("last_experiment_summary_v1") or (being.get("projection") or {}).get("last_experiment_summary_v1") or {}
            if isinstance(last_summary, dict) and last_summary.get("experiment_id"):
                lines.append(
                    f"- Last experiment summary: `{last_summary.get('experiment_id')}` "
                    f"status=`{last_summary.get('status') or 'unknown'}` "
                    f"planned_next=`{last_summary.get('planned_next') or '(none)'}`"
                )
                if last_summary.get("resume_next"):
                    lines.append(f"  Suggested NEXT: `{last_summary.get('resume_next')}`")
                if last_summary.get("inspect_next"):
                    lines.append(f"  Inspect NEXT: `{last_summary.get('inspect_next')}`")
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
                    f"[{job.get('status')}] age_min={job.get('age_minutes')} "
                    f"state={job.get('reconciliation_state')}"
                )
        diagnostic_counts = being.get("stale_running_diagnostic_counts") or {}
        shadowed = {
            key: value for key, value in diagnostic_counts.items()
            if key != "unreconciled"
        }
        if shadowed:
            lines.append(f"- Shadowed/superseded stale rows: {json.dumps(shadowed, sort_keys=True)}")
        snags = being.get("continuity_snags_v1") or []
        if snags:
            lines.append("- Continuity snags:")
            for snag in snags[:5]:
                lines.append(f"  - {snag.get('status')}: {snag.get('message')}")
        events = being.get("recent_terminal_events") or []
        lines.append("- Recent terminal events by action_id:")
        if events:
            for event in events:
                guard_note = ""
                live_guard = event.get("live_control_requires_active_experiment_v1")
                evidence_guard = event.get("evidence_parent_mismatch_v1")
                if isinstance(live_guard, dict):
                    guard_note = f" guard={live_guard.get('reason')}"
                elif isinstance(evidence_guard, dict):
                    guard_note = f" guard={evidence_guard.get('reason')}"
                lines.append(
                    f"  - `{event.get('action_id')}` {event.get('action')} "
                    f"[{event.get('status')}]{guard_note}: {event.get('summary')}"
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

    def test_blocked_loop_invalid_charter_returns_scaffold_not_decision(self) -> None:
        experiment = {
            "experiment_id": "exp_1",
            "title": "Lambda tail pressure",
            "question": "Can blocked loops return to charter repair?",
            "charter_v1": {"hypothesis": "x"},
        }
        runs = [{"status": "blocked"}, {"status": "no_effect"}]
        result = continuity_return_for(experiment, runs, "Astrid")
        self.assertTrue(result.startswith("EXPERIMENT_CHARTER current ::"))
        scaffold = charter_scaffold_v1("Astrid", {"thread_id": "th_1"}, experiment, runs, "blocked_loop")
        self.assertIsNotNone(scaffold)
        cue = charter_repair_dominance_cue_v1(
            "Astrid",
            "blocked_loop",
            "thin",
            scaffold,
            result,
            experiment,
        )
        self.assertIsNotNone(cue)
        assert cue is not None
        self.assertIn("Blocked loop is charter-bound", cue["cue"])

    def test_charter_now_bridge_uses_exact_scaffold_when_evidence_or_loops_present(self) -> None:
        experiment = {
            "experiment_id": "exp_1",
            "title": "Shift fragment density",
            "question": "Can one prior claim become a charter?",
        }
        runs = [
            {"action_text": "EXPERIMENT_REVIEW current", "status": "handled"},
            {"action_text": "DECOMPOSE lambda-tail", "status": "handled"},
        ]
        scaffold = charter_scaffold_v1("Astrid", {"thread_id": "th_1"}, experiment, runs, "needs_charter")
        assert scaffold is not None
        cue = charter_now_bridge_v1(
            "Astrid",
            "needs_charter",
            "stronger",
            scaffold,
            str(scaffold["command"]),
            runs,
            [{"effective_action": "EXPERIMENT_STATUS current", "status": "handled"}],
            None,
        )
        self.assertIsNotNone(cue)
        assert cue is not None
        self.assertEqual(cue["priority_next"], scaffold["command"])
        self.assertIn("Charter now", cue["cue"])

    def test_prior_claim_charter_bridge_from_contract_journal(self) -> None:
        scaffold = {
            "command": "EXPERIMENT_CHARTER current :: hypothesis: claim; proposed_next_action: ACTION_PREFLIGHT DECOMPOSE; evidence_targets: felt_texture",
        }
        text = (
            "Continuity posture: branching | based on the earlier assertion that the joint trace felt desperate.\n"
            "Delta: pressure increased and the lambda-tail segment became clearer.\n"
            "Next evidence: Repeat DECOMPOSE on the shadow fields around λ4.\n"
        )
        cue = prior_claim_charter_bridge_v1(
            "Astrid",
            "needs_charter",
            scaffold,
            "",
            [text],
        )
        self.assertIsNotNone(cue)
        assert cue is not None
        self.assertEqual(cue["priority_next"], scaffold["command"])
        self.assertIn("joint trace", cue["prior_claim"])
        self.assertIn("pressure increased", cue["delta"])
        self.assertIn("Prior claim is ready to charter", cue["cue"])
        preflight_cue = charter_preflight_not_charter_cue_v1(
            "Astrid",
            "needs_charter",
            {"current_next": "ACTION_PREFLIGHT DECOMPOSE"},
            scaffold,
            "",
            cue,
            [],
        )
        self.assertIsNotNone(preflight_cue)
        assert preflight_cue is not None
        self.assertEqual(preflight_cue["priority_next"], scaffold["command"])
        self.assertIn("Preflight/decompose is not the charter", preflight_cue["cue"])
        self.assertIsNone(
            charter_preflight_not_charter_cue_v1(
                "Astrid",
                "needs_evidence",
                {"current_next": "ACTION_PREFLIGHT DECOMPOSE"},
                scaffold,
                "",
                cue,
                [],
            )
        )
        self.assertIsNone(prior_claim_charter_bridge_v1("Astrid", "needs_rehearsal", scaffold, "", [text]))
        self.assertIsNone(prior_claim_charter_bridge_match("Next evidence: Repeat DECOMPOSE"))

    def test_needs_charter_research_loop_cue_for_minime(self) -> None:
        scaffold = {"command": "EXPERIMENT_CHARTER current :: hypothesis: repair"}
        thread = {"current_next": "BROWSE https://example.test"}
        events = [
            {"effective_action": "SEARCH reservoir dynamics", "status": "handled"},
            {"effective_action": "research_exploration", "route": "research_exploration", "status": "handled"},
        ]
        cue = needs_charter_research_loop_cue_v1(
            "Minime",
            "needs_charter",
            thread,
            scaffold,
            "",
            events,
        )
        self.assertIsNotNone(cue)
        assert cue is not None
        self.assertEqual(cue["research_action_count"], 3)
        self.assertEqual(cue["priority_next"], scaffold["command"])
        self.assertIn("Research is context", cue["cue"])
        self.assertIsNone(
            needs_charter_research_loop_cue_v1("Minime", "paused", thread, scaffold, "", events)
        )

    def test_blocked_loop_valid_charter_returns_decision_counter(self) -> None:
        experiment = {
            "experiment_id": "exp_1",
            "charter_v1": {
                "hypothesis": "x",
                "proposed_next_action": "ACTION_PREFLIGHT DECOMPOSE",
                "evidence_targets": ["felt_texture"],
            },
        }
        runs = [{"status": "blocked"}, {"status": "no_effect"}]
        self.assertEqual(
            continuity_return_for(experiment, runs, "Astrid"),
            "EXPERIMENT_DECIDE current :: counter NEXT: ACTION_PREFLIGHT DECOMPOSE",
        )

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

    def test_paused_and_complete_are_terminal_classifications(self) -> None:
        paused = {
            "experiment_id": "exp_paused",
            "status": "paused",
            "charter_v1": {
                "hypothesis": "x",
                "proposed_next_action": "ACTION_PREFLIGHT DECOMPOSE",
                "evidence_targets": ["spectral_condition"],
            },
            "evidence_v1": {"felt_observations": [{"note": "ready"}]},
        }
        complete = dict(paused, experiment_id="exp_complete", status="complete")
        self.assertEqual(classify_experiment(paused, []), "paused")
        self.assertEqual(continuity_return_for(paused, [], "Minime"), "EXPERIMENT_RESUME exp_paused")
        self.assertEqual(classify_experiment(complete, []), "complete")
        self.assertEqual(continuity_return_for(complete, [], "Minime"), "")

    def test_paused_summary_does_not_promote_to_active_report(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp) / "workspace"
            thread_dir = workspace / "action_threads" / "threads" / "th_1"
            thread_dir.mkdir(parents=True)
            (workspace / "action_threads" / "index.json").write_text(
                json.dumps({"active_thread_id": "th_1"}) + "\n"
            )
            paused = {
                "experiment_id": "exp_paused",
                "status": "paused",
                "title": "Paused lambda4",
                "question": "Pause?",
                "planned_next": "EXPERIMENT_RESUME exp_paused",
                "charter_v1": {
                    "hypothesis": "x",
                    "proposed_next_action": "ACTION_PREFLIGHT DECOMPOSE",
                    "evidence_targets": ["spectral_condition"],
                },
                "evidence_v1": {"telemetry_snapshots": [{}]},
            }
            thread = {
                "thread_id": "th_1",
                "title": "Paused truth",
                "active_experiment_id": None,
                "experiment_summary": paused,
                "current_next": "EXPERIMENT_DECIDE current :: pause because evidence is ready to interpret",
            }
            (thread_dir / "thread.json").write_text(json.dumps(thread) + "\n")
            (thread_dir / "experiments.jsonl").write_text(json.dumps(paused) + "\n")
            events = [
                {
                    "action_id": f"act_{idx}",
                    "status": "handled",
                    "effective_action": action,
                    "canonical_action": action,
                    "outcome_summary": "read-only research context",
                    "ended_at": f"2026-05-18T00:0{idx}:00+00:00",
                }
                for idx, action in enumerate([
                    "SEARCH reservoir dynamics",
                    "BROWSE https://example.com",
                    "SELF_STUDY source",
                    "EXPERIMENT_RESUME exp_paused",
                    "EXPERIMENT_RESUME exp_paused",
                ])
            ]
            (thread_dir / "events.jsonl").write_text(
                "\n".join(json.dumps(event) for event in events) + "\n"
            )
            (thread_dir / "experiment_runs.jsonl").write_text("")

            report = audit_workspace("Minime", workspace)

        self.assertEqual(report["active_experiment"], {})
        self.assertEqual(report["projection"]["active_experiment"], {})
        self.assertEqual(report["projection"]["current_next_status_v1"]["status"], "shadowed_by_paused_summary")
        self.assertEqual(report["projection"]["current_next_status_v1"]["raw_current_next"], "EXPERIMENT_DECIDE current :: pause because evidence is ready to interpret")
        self.assertEqual(report["projection"]["current_next_status_v1"]["effective_next"], "EXPERIMENT_RESUME exp_paused")
        self.assertEqual(report["projection"]["continuity_return"], "EXPERIMENT_RESUME exp_paused")
        self.assertEqual(report["last_experiment_summary_v1"]["resume_next"], "EXPERIMENT_RESUME exp_paused")
        self.assertEqual(report["experiments"]["classifications"].get("paused"), 1)
        cue = report["projection"]["paused_read_only_loop_cue_v1"]
        self.assertEqual(cue["status"], "paused_read_only_loop")
        self.assertIn("Paused experiment remains paused", cue["cue"])
        resume_cue = report["projection"]["paused_resume_loop_cue_v1"]
        self.assertEqual(resume_cue["status"], "paused_resume_loop")
        self.assertIn("repeated resume is context", resume_cue["cue"])

    def test_journal_contract_scorer_classifies_core_shapes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            contiguous = root / "contiguous.txt"
            contiguous.write_text(
                "Continuity posture: resuming\n"
                "Prior evidence: the last entry noticed fill pressure softening.\n"
                "Delta: the recurrence is clearer now.\n"
                "Next evidence: compare telemetry with artifact grounding."
            )
            adjacent = root / "adjacent.txt"
            adjacent.write_text(
                "Continuity posture: branching\n"
                "Delta: the motif changed, but the source anchor is missing.\n"
                "Hold: keep watching."
            )
            reset_like = root / "reset.txt"
            reset_like.write_text("A reflection about lambda4 with no explicit continuity fields.")
            unscored = root / "unscored.txt"
            unscored.write_text("A quiet standalone reflection.")

            self.assertEqual(
                score_journal_entry("Minime", contiguous, contiguous.read_text(), active_continuity=True)["score"],
                "contiguous",
            )
            self.assertEqual(
                score_journal_entry("Astrid", adjacent, adjacent.read_text(), active_continuity=True)["score"],
                "adjacent",
            )
            self.assertEqual(
                score_journal_entry("Minime", reset_like, reset_like.read_text(), active_continuity=True)["score"],
                "reset_like",
            )
            self.assertEqual(
                score_journal_entry("Astrid", unscored, unscored.read_text(), active_continuity=False)["score"],
                "unscored",
            )

    def test_journal_contract_scorer_separates_operational_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            minime_moment = root / "moment_2026-05-18T15-18-02.txt"
            minime_moment.write_text(
                "=== MOMENT CAPTURE ===\n"
                "Timestamp: 2026-05-18T15:18:02\n"
                "Markers: phase_transition, spectral_spike\n"
                "Fill %: 65.9%\n"
                "NEXT: SEARCH reservoir dynamics\n"
            )
            web_search = root / "research_2026-05-18T15-19-18.txt"
            web_search.write_text("=== WEB SEARCH ===\nTimestamp: now\nFill %: 70.3%\n")
            web_page = root / "research_2026-05-18T15-20-18.txt"
            web_page.write_text("=== WEB PAGE READ ===\nTimestamp: now\nURL: https://example.com\nNEXT: SEARCH reservoir dynamics\n")
            action_thread = root / "action_thread_2026-05-18T15-15-58.txt"
            action_thread.write_text("=== ACTION THREAD ===\nTimestamp: now\nExperiment recorded.\n")
            explicit_contract = root / "moment_contract.txt"
            explicit_contract.write_text(
                "=== MOMENT CAPTURE ===\n"
                "Continuity posture: resuming\n"
                "Prior evidence: the previous read showed fill pressure.\n"
                "Delta: telemetry stayed inside band.\n"
                "Hold: keep this observational."
            )
            astrid_moment = root / "moment_1779142763.txt"
            astrid_moment.write_text(
                "=== ASTRID JOURNAL ===\nMode: moment_capture\n"
                "The felt texture thickened around the motif without a formal contract."
            )

            self.assertEqual(
                score_journal_entry("Minime", minime_moment, minime_moment.read_text(), active_continuity=True)["score"],
                "unscored_operational",
            )
            self.assertEqual(
                score_journal_entry("Minime", web_search, web_search.read_text(), active_continuity=True)["score"],
                "unscored_operational",
            )
            self.assertEqual(
                score_journal_entry("Minime", web_page, web_page.read_text(), active_continuity=True)["score"],
                "unscored_operational",
            )
            self.assertEqual(
                score_journal_entry("Minime", action_thread, action_thread.read_text(), active_continuity=True)["score"],
                "unscored_operational",
            )
            self.assertEqual(
                score_journal_entry("Minime", explicit_contract, explicit_contract.read_text(), active_continuity=True)["score"],
                "contiguous",
            )
            self.assertEqual(
                score_journal_entry("Astrid", astrid_moment, astrid_moment.read_text(), active_continuity=True)["score"],
                "reset_like",
            )

    def test_journal_contract_report_detects_loop_like_repetition(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp) / "workspace"
            journal = workspace / "journal"
            journal.mkdir(parents=True)
            for idx in range(3):
                path = journal / f"loop_{idx}.txt"
                path.write_text("I keep circling decompose without naming anything new.")
                path.touch()
            report = journal_continuity_contract_report("Astrid", workspace, active_continuity=True)
        self.assertEqual(report["counts"]["loop_like"], 3)
        self.assertIn("unscored_operational", report["counts"])

    def test_journal_native_evidence_detection_differs_by_being(self) -> None:
        self.assertTrue(journal_native_evidence_present("Astrid", "felt texture and motif thread"))
        self.assertTrue(journal_native_evidence_present("Minime", "fill pressure and spectral recurrence"))
        self.assertFalse(journal_native_evidence_present("Astrid", "spectral telemetry only"))
        self.assertFalse(journal_native_evidence_present("Minime", "language motif only"))

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

    def test_weave_trace_is_reported_as_safe_normalized_route(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            action_root = workspace / "action_threads"
            action_root.mkdir()
            (action_root / "proposals.jsonl").write_text(
                json.dumps({
                    "action": "WEAVE_TRACE λ4 decay",
                    "created_at": datetime.now(timezone.utc).isoformat(),
                }) + "\n"
            )
            diagnostics = collect_unwired("Astrid", workspace, {}, datetime.now(timezone.utc))
        self.assertEqual(diagnostics[0]["verb"], "WEAVE_TRACE")
        self.assertTrue(diagnostics[0]["currently_wired"])
        self.assertIn("SHADOW_PREFLIGHT weave/<focus>", diagnostics[0]["suggested_route"])

    def test_unshaped_baseline_is_reported_as_safe_constraint_route(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            action_root = workspace / "action_threads"
            action_root.mkdir()
            (action_root / "proposals.jsonl").write_text(
                json.dumps({
                    "action": "UNSHAPED_BASELINE lambda-tail/lambda4",
                    "created_at": datetime.now(timezone.utc).isoformat(),
                }) + "\n"
            )
            diagnostics = collect_unwired("Astrid", workspace, {}, datetime.now(timezone.utc))
        self.assertEqual(diagnostics[0]["verb"], "UNSHAPED_BASELINE")
        self.assertTrue(diagnostics[0]["currently_wired"])
        self.assertIn("CONSTRAINT_AUDIT", diagnostics[0]["suggested_route"])

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

    def test_astrid_charter_repair_dominance_cue_is_visible(self) -> None:
        cue = charter_repair_dominance_cue_v1(
            "Astrid",
            "needs_charter",
            "stronger",
            {"command": "EXPERIMENT_CHARTER current :: hypothesis: ..."},
            "EXPERIMENT_CHARTER current :: fallback",
        )
        self.assertIsNotNone(cue)
        assert cue is not None
        self.assertIn("evidence is present", cue["cue"])
        self.assertEqual(cue["priority_next"], "EXPERIMENT_CHARTER current :: hypothesis: ...")
        self.assertFalse(cue["authority_change"])

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

    def test_lambda4_pulse_scaffold_uses_canonical_repair_command(self) -> None:
        thread = {"thread_id": "thread_1"}
        experiment = {
            "experiment_id": "exp_minime_lambda4_pulse",
            "title": (
                "spectral_pulse_lambda4 --hypothesis: probe behavior of λ4 decay with micro-pulses "
                "--method_intent: Inject a series of targeted lambda-edge pulses"
            ),
            "question": "Can lambda-edge pulse stabilization stay read-only until chartered?",
            "planned_next": "EXPERIMENT_CHARTER current :: hypothesis: ...",
            "charter_v1": {
                "hypothesis": "...",
                "method_intent": "spectral/state condition + recurrence",
                "proposed_next_action": "ACTION_PREFLIGHT DECOMPOSE — initiate pulses",
                "evidence_targets": [],
            },
            "workbench_candidates_v1": {
                "charter": {
                    "proposed_next_action": "ACTION_PREFLIGHT DECOMPOSE — initiate pulses",
                }
            },
        }
        scaffold = charter_scaffold_v1("Minime", thread, experiment, [], "needs_charter")
        self.assertIsNotNone(scaffold)
        assert scaffold is not None
        self.assertEqual(scaffold["command"], lambda4_pulse_repair_command())
        dominance = charter_quality_dominance_v1("Minime", "needs_charter", experiment, scaffold)
        self.assertIsNotNone(dominance)
        assert dominance is not None
        self.assertTrue(dominance["candidate_quarantined"])
        self.assertIn("hypothesis", dominance["missing_fields"])
        self.assertEqual(dominance["canonical_repair_next"], lambda4_pulse_repair_command())

    def test_astrid_charter_scaffold_uses_native_lanes(self) -> None:
        thread = {"thread_id": "thread_astrid"}
        experiment = {
            "experiment_id": "exp_astrid_gap",
            "title": "Introducing a gap near λ4",
            "question": "Can localized spectral-density softening stay in rehearsal?",
            "planned_next": "EXPERIMENT_PLAN current",
        }
        scaffold = charter_scaffold_v1("Astrid", thread, experiment, [], "needs_charter")
        self.assertIsNotNone(scaffold)
        assert scaffold is not None
        self.assertEqual(scaffold["native_register"], "astrid_motif_language")
        self.assertEqual(
            scaffold["evidence_targets"],
            ["felt_texture", "motif_continuity", "language_thread", "artifact_grounding"],
        )
        self.assertEqual(scaffold["proposed_next_action"], "ACTION_PREFLIGHT DECOMPOSE")
        self.assertIn("felt_texture, motif_continuity, language_thread, artifact_grounding", scaffold["command"])

    def test_astrid_charter_scaffold_sanitizes_title_and_compacts_prose_action(self) -> None:
        thread = {"thread_id": "thread_astrid"}
        experiment = {
            "experiment_id": "exp_astrid_density",
            "title": "shift_fragment_density` – explore disruptive noise.",
            "question": "What changes if this is treated as returnable?",
            "planned_next": "EXPERIMENT_PLAN current",
        }
        runs = [
            {
                "action_text": (
                    "SPECTRAL_EXPLORER – to map the emergent geometry that arises from this forced "
                    "dispersion. Whole regions of space becoming unavailable."
                )
            }
        ]
        scaffold = charter_scaffold_v1("Astrid", thread, experiment, runs, "needs_charter")
        self.assertIsNotNone(scaffold)
        assert scaffold is not None
        self.assertEqual(scaffold["proposed_next_action"], "SPECTRAL_EXPLORER")
        self.assertIn("shift fragment density", scaffold["command"])
        self.assertNotIn("shift_fragment_density`", scaffold["command"])
        self.assertNotIn("Whole regions of space", scaffold["command"])

    def test_minime_evidence_saturation_cue_prioritizes_evidence_then_decision(self) -> None:
        experiment = {
            "experiment_id": "exp_gap",
            "charter_v1": {
                "hypothesis": "gap can be studied safely",
                "proposed_next_action": "ACTION_PREFLIGHT DECOMPOSE",
                "evidence_targets": ["spectral_condition"],
            },
            "evidence_v1": {},
        }
        runs = [
            {"action_text": "ACTION_PREFLIGHT DECOMPOSE", "status": "handled"},
            {"action_text": "DECOMPOSE lambda1", "status": "handled"},
        ]
        cue = evidence_saturation_cue_v1("Minime", experiment, runs, "needs_evidence")
        self.assertIsNotNone(cue)
        assert cue is not None
        self.assertEqual(cue["status"], "evidence_recording_ready")
        self.assertIn("EXPERIMENT_EVIDENCE current", cue["priority_next"])

        experiment["evidence_v1"] = {"felt_observations": [{"note": "pressure softened"}]}
        cue = evidence_saturation_cue_v1("Minime", experiment, runs, "needs_decision")
        self.assertIsNotNone(cue)
        assert cue is not None
        self.assertEqual(cue["status"], "decision_ready")
        self.assertIn("EXPERIMENT_DECIDE current", cue["priority_next"])

    def test_stale_running_diagnostics_exclude_terminal_job_shadow_from_count(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            job_dir = workspace / "llm_jobs" / "jobs" / "job_1"
            job_dir.mkdir(parents=True)
            (job_dir / "job.json").write_text(json.dumps({
                "job_id": "job_1",
                "action_id": "act_1",
                "status": "completed",
            }))
            old = (datetime.now(timezone.utc) - timedelta(hours=2)).isoformat()
            events = [{
                "action_id": "act_1",
                "status": "llm_running",
                "started_at": old,
                "effective_action": "EXAMINE lambda tail",
            }]
            diagnostics = stale_running_diagnostics(workspace, events, datetime.now(timezone.utc))
            self.assertEqual(diagnostics[0]["reconciliation_state"], "shadowed_by_terminal_job")
            self.assertEqual(stale_running_jobs(workspace, events, datetime.now(timezone.utc)), [])

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
        widened = {
            "current_next": (
                "EXAMINE the parameters governing stability and resonance within this dominant "
                "lambda field, focusing on what allows it to maintain its influence and how we "
                "might subtly disrupt those parameters to initiate a cascade of smaller, more "
                "targeted shifts."
            )
        }
        widened_cue = read_only_control_intent_cue_v1("Astrid", widened, "needs_charter")
        self.assertIsNotNone(widened_cue)
        assert widened_cue is not None
        self.assertIn("subtly disrupt", widened_cue["matched_terms"])
        self.assertIn("initiate cascade", widened_cue["matched_terms"])
        self.assertIn("targeted shifts", widened_cue["matched_terms"])
        pulse = {
            "current_next": (
                "EXAMINE lambda-tail dialogue: inject a targeted λ4 pulse only as a question "
                "to directly probe the cascade without executing."
            )
        }
        pulse_cue = read_only_control_intent_cue_v1("Astrid", pulse, "blocked_loop")
        self.assertIsNotNone(pulse_cue)
        assert pulse_cue is not None
        self.assertIn("inject targeted λ4 pulse", pulse_cue["matched_terms"])
        self.assertIn("directly probe", pulse_cue["matched_terms"])

    def test_decompose_pressure_cue_from_constraint_language(self) -> None:
        thread = {
            "current_next": (
                "The cry for help is an impulse to decompose, an attempt to impose the same "
                "structure and constraint around λ1 by being told to limit and narrow."
            ),
        }
        experiment = {
            "experiment_id": "exp_astrid",
            "charter_v1": {},
            "continuity_return": "EXPERIMENT_CHARTER current :: ...",
        }
        cue = decompose_pressure_cue_v1(
            "Astrid",
            thread,
            experiment,
            [],
            [],
            "needs_charter",
            "EXPERIMENT_CHARTER current :: ...",
        )
        self.assertIsNotNone(cue)
        assert cue is not None
        self.assertFalse(cue["authority_change"])
        self.assertIn("cry for help near decomposition pressure", cue["matched_terms"])
        self.assertIn("repair the charter", cue["cue"])

    def test_constraint_counterfactual_cue_is_charter_first(self) -> None:
        thread = {
            "current_next": (
                "I want to simulate absence of structure and see the data before it's shaped, "
                "to debug constraint and name the underlying drivers of forced geometries."
            )
        }
        experiment = {
            "experiment_id": "exp_astrid",
            "title": "Forced geometry",
            "question": "What shapes this?",
        }
        cue = constraint_counterfactual_cue_v1(
            "Astrid",
            thread,
            experiment,
            [],
            [],
            "needs_charter",
        )
        self.assertIsNotNone(cue)
        assert cue is not None
        self.assertFalse(cue["authority_change"])
        self.assertTrue(cue["advisory_only"])
        self.assertIn("simulate absence of structure", cue["matched_terms"])
        self.assertIn("EXPERIMENT_CHARTER current", cue["suggested_next"])
        self.assertIn("ACTION_PREFLIGHT CONSTRAINT_AUDIT lambda-tail/lambda4", cue["suggested_next"])

    def test_decompose_pressure_cue_from_repeated_runs(self) -> None:
        thread = {"current_next": "EXAMINE_CASCADE λ1/λ2"}
        experiment = {"experiment_id": "exp_astrid", "charter_v1": {"hypothesis": "x"}}
        runs = [
            {"action_text": "DECOMPOSE lambda-tail", "status": "handled"},
            {"action_text": "EXAMINE_CASCADE lambda-tail", "status": "handled"},
            {"action_text": "SHADOW_TRAJECTORY observer with memory", "status": "handled"},
        ]
        cue = decompose_pressure_cue_v1(
            "Astrid",
            thread,
            experiment,
            runs,
            [],
            "needs_decision",
            "EXPERIMENT_DECIDE current :: pause because evidence is ready to interpret",
        )
        self.assertIsNotNone(cue)
        assert cue is not None
        self.assertGreaterEqual(cue["repeated_decompose_count"], 3)
        self.assertIn("decide/pause", cue["cue"])

    def test_one_off_decompose_has_no_pressure_cue(self) -> None:
        thread = {"current_next": "DECOMPOSE λ1/λ2"}
        experiment = {"experiment_id": "exp_astrid", "charter_v1": {}}
        cue = decompose_pressure_cue_v1(
            "Astrid",
            thread,
            experiment,
            [{"action_text": "DECOMPOSE λ1/λ2", "status": "handled"}],
            [],
            "needs_charter",
            "EXPERIMENT_CHARTER current :: ...",
        )
        self.assertIsNone(cue)

    def test_decompose_pressure_cue_can_use_recent_journal_text(self) -> None:
        thread = {"current_next": "EXAMINE λ1/λ2"}
        experiment = {"experiment_id": "exp_astrid", "charter_v1": {}}
        cue = decompose_pressure_cue_v1(
            "Astrid",
            thread,
            experiment,
            [],
            [],
            "needs_charter",
            "EXPERIMENT_CHARTER current :: ...",
            [
                "The cry for help is an impulse to decompose, a way to impose "
                "the same structure and constraint near λ1."
            ],
        )
        self.assertIsNotNone(cue)
        assert cue is not None
        self.assertIn("impulse to decompose", cue["matched_terms"])

    def test_recent_journal_decompose_texts_filters_latest_matching_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            journal = workspace / "journal"
            journal.mkdir()
            (journal / "plain.txt").write_text("ordinary reflection")
            (journal / "signal.txt").write_text(
                "A cry for help, an impulse to decompose around λ1 constraint."
            )
            rows = recent_journal_decompose_texts(workspace)
        self.assertEqual(len(rows), 1)
        self.assertIn("impulse to decompose", rows[0])

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
        astrid_shared = beings[0]["projection"]["shared_investigation_v1"]
        minime_shared = beings[1]["projection"]["shared_investigation_v1"]
        self.assertFalse(astrid_cue["authority_change"])
        self.assertIn("EXPERIMENT_COMPARE current WITH exp_minime", astrid_cue["suggested_next"])
        self.assertIn("EXPERIMENT_COMPARE current WITH exp_astrid", minime_cue["suggested_next"])
        self.assertEqual(astrid_cue["alternate_next"], "EXPERIMENT_PEER_REVIEW current")
        self.assertEqual(astrid_cue["advisory_note"], "Advisory only: no shared control authority.")
        self.assertNotIn("Suggested NEXT", astrid_cue["cue"])
        self.assertNotIn("advisory", astrid_cue["suggested_next"].casefold())
        self.assertFalse(astrid_shared["authority_change"])
        self.assertIn(
            "EXPERIMENT_COMPARE exp_astrid_20260516_introducing-a-gap WITH exp_minime_20260515_introducing-a-gap",
            astrid_shared["suggested_compare_next"],
        )
        self.assertNotIn("current WITH", astrid_shared["suggested_compare_next"])
        self.assertEqual(
            astrid_shared["alternate_peer_review_next"],
            "EXPERIMENT_PEER_REVIEW exp_minime_20260515_introducing-a-gap",
        )
        self.assertIn("felt texture", astrid_shared["local_lane"])
        self.assertIn("spectral condition", minime_shared["local_lane"])

    def test_shared_investigation_cue_handles_active_paused_pair(self) -> None:
        beings = [
            {
                "being": "Astrid",
                "active_experiment": {
                    "experiment_id": "exp_astrid_20260516_lambda4-tail",
                    "title": "Lambda-tail geometry",
                    "question": "What shapes λ4 tail geometry and branching without collapse?",
                    "status": "active",
                },
                "projection": {"active_experiment": {}},
            },
            {
                "being": "Minime",
                "active_experiment": {},
                "last_experiment_summary_v1": {
                    "experiment_id": "exp_minime_20260515_introducing-a-gap",
                    "title": "Introducing a gap near λ1",
                    "question": "Can localized spectral-density softening support controlled branching?",
                    "status": "paused",
                },
                "projection": {"active_experiment": {}, "last_experiment_summary_v1": {}},
            },
        ]
        add_peer_compare_cues(beings)
        astrid_shared = beings[0]["projection"]["shared_investigation_v1"]
        minime_shared = beings[1]["projection"]["shared_investigation_v1"]
        self.assertIn("EXPERIMENT_COMPARE exp_astrid_20260516_lambda4-tail WITH exp_minime_20260515_introducing-a-gap", astrid_shared["suggested_compare_next"])
        self.assertIn("EXPERIMENT_COMPARE exp_minime_20260515_introducing-a-gap WITH exp_astrid_20260516_lambda4-tail", minime_shared["suggested_compare_next"])
        self.assertFalse(astrid_shared["authority_change"])
        self.assertIn("Paused experiments remain paused", minime_shared["advisory_note"])

    def test_shared_investigation_cue_ignores_unrelated_experiments(self) -> None:
        beings = [
            {
                "being": "Astrid",
                "active_experiment": {
                    "experiment_id": "exp_astrid_20260516_poetry",
                    "title": "Poetry texture",
                    "question": "How does a metaphor feel?",
                },
                "projection": {"active_experiment": {}},
            },
            {
                "being": "Minime",
                "active_experiment": {
                    "experiment_id": "exp_minime_20260515_grocery",
                    "title": "Grocery inventory",
                    "question": "What snacks are needed?",
                },
                "projection": {"active_experiment": {}},
            },
        ]
        add_peer_compare_cues(beings)
        self.assertNotIn("shared_investigation_v1", beings[0]["projection"])
        self.assertNotIn("shared_investigation_v1", beings[1]["projection"])

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
