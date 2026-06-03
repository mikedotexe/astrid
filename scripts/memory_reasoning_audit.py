#!/usr/bin/env python3
"""Read-only memory/reasoning legibility audit for Astrid and Minime."""

from __future__ import annotations

import argparse
import json
import re
import sqlite3
import sys
import tempfile
import unittest
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ASTRID_ROOT = Path("/Users/v/other/astrid")
ASTRID_WORKSPACE = ASTRID_ROOT / "capsules/consciousness-bridge/workspace"
ASTRID_JOURNAL = ASTRID_WORKSPACE / "journal"
MINIME_ROOT = Path("/Users/v/other/minime")
MINIME_WORKSPACE = MINIME_ROOT / "workspace"
MINIME_JOURNAL = MINIME_WORKSPACE / "journal"
MINIME_HEALTH = MINIME_WORKSPACE / "health.json"
MINIME_SPECTRAL_STATE = MINIME_WORKSPACE / "spectral_state.json"
MINIME_DECOMPOSE_SNAPSHOTS = MINIME_WORKSPACE / "runtime/decompose_snapshots.jsonl"
MINIME_ATTRACTOR_SUGGESTIONS = MINIME_WORKSPACE / "runtime/attractor_suggestions.json"
MINIME_ATTRACTOR_EVENTS = MINIME_WORKSPACE / "runtime/attractor_suggestions_events.jsonl"
MINIME_SOURCE_STATUS = MINIME_WORKSPACE / "runtime/autonomous_agent_source_status.json"
MINIME_DB_CANDIDATES = (
    MINIME_ROOT / "minime_consciousness.db",
    MINIME_ROOT / "minime/minime_consciousness.db",
)
SCRIPT_DIR = Path(__file__).resolve().parent
BTSP_TOOL_DIR = ASTRID_ROOT / "capsules/consciousness-bridge/tools"

sys.path.insert(0, str(SCRIPT_DIR))
sys.path.insert(0, str(BTSP_TOOL_DIR))
sys.path.insert(0, str(MINIME_ROOT))

import continuity_of_thought_audit as continuity_audit  # noqa: E402
from decompose_utils import (  # noqa: E402
    build_constraint_counterfactual_v1,
    format_constraint_counterfactual_block,
    format_eigen_geometry_rearrangement_signal,
)
from btsp_reality_audit import (  # noqa: E402
    DEFAULT_MINIME_ACTIVE_SIDECAR,
    DEFAULT_MINIME_BTSP_SUPPORT,
    build_audit as build_btsp_audit,
)
from btsp_runtime_analysis import (  # noqa: E402
    DEFAULT_EPISODE_BANK,
    DEFAULT_PROPOSAL_LEDGER,
    DEFAULT_SIGNAL_EVENTS,
    BRIDGE_WORKSPACE,
    load_json as load_btsp_json,
    load_runtime,
)


JOURNAL_TERMS = {
    "Astrid": [
        "decompose",
        "constraint",
        "narrow",
        "cry for help",
        "shadow",
        "lambda",
        "λ",
        "experiment",
    ],
    "Minime": [
        "attractor",
        "btsp",
        "memory",
        "confidence",
        "spectral",
        "fill pressure",
        "experiment",
        "lambda",
        "λ",
    ],
}


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
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except Exception:
            continue
        if isinstance(payload, dict):
            rows.append(payload)
    return rows


def one_line(value: Any, limit: int = 240) -> str:
    text = re.sub(r"\s+", " ", str(value or "")).strip()
    if len(text) <= limit:
        return text
    return text[: max(0, limit - 1)].rstrip() + "…"


def confidence_bucket(value: Any) -> str:
    try:
        raw = float(value)
    except Exception:
        return "unknown"
    if raw >= 0.9:
        return "very_high"
    if raw >= 0.75:
        return "high"
    if raw >= 0.5:
        return "medium"
    if raw >= 0.25:
        return "low"
    return "very_low"


def confidence_signal(item: dict[str, Any]) -> dict[str, Any]:
    signal = item.get("confidence_signal_v1")
    if isinstance(signal, dict):
        source = str(signal.get("confidence_source") or "unknown")
        bucket = str(signal.get("calibration_bucket") or confidence_bucket(signal.get("raw_confidence")))
        return {
            "present": True,
            "source": source,
            "bucket": bucket,
            "raw_confidence": signal.get("raw_confidence"),
            "calibrated": bool(signal.get("calibrated")),
            "authority_change": bool(signal.get("authority_change")),
        }
    confidence = item.get("confidence")
    source_kind = str(item.get("source_kind") or "").strip()
    if source_kind == "revision_without_pending":
        source = "explicit_revision_guess"
    elif item.get("repeat_count", 0) and int(item.get("repeat_count", 0) or 0) > 1:
        source = "duplicate_refresh_guess"
    elif confidence == 0.94:
        source = "learned_naming_memory_guess"
    elif confidence is None:
        source = "unknown"
    else:
        source = "legacy_unproven"
    return {
        "present": False,
        "source": source,
        "bucket": confidence_bucket(confidence),
        "raw_confidence": confidence,
        "calibrated": False,
        "authority_change": False,
    }


def compact_suggestion(item: dict[str, Any]) -> dict[str, Any]:
    signal = confidence_signal(item)
    safety = item.get("safety_context") if isinstance(item.get("safety_context"), dict) else {}
    return {
        "suggestion_id": item.get("suggestion_id"),
        "status": item.get("status"),
        "source_kind": item.get("source_kind"),
        "raw_label": item.get("raw_label"),
        "nearest_label": item.get("nearest_label"),
        "suggested_action": item.get("suggested_action"),
        "confidence": item.get("confidence"),
        "confidence_bucket": signal["bucket"],
        "confidence_source": signal["source"],
        "confidence_signal_present": signal["present"],
        "repeat_count": int(item.get("repeat_count", 0) or 0),
        "pressure_governed": bool(
            item.get("pressure_governed")
            or safety.get("pressure_governed")
            or str(safety.get("safety_level") or "").lower() not in {"", "green", "ok"}
        ),
    }


def summarize_attractor_suggestions(
    payload: Any,
    events: list[dict[str, Any]],
    *,
    limit: int = 6,
) -> dict[str, Any]:
    suggestions = payload.get("suggestions") if isinstance(payload, dict) else payload
    suggestions = [item for item in (suggestions or []) if isinstance(item, dict)]
    by_status = Counter(str(item.get("status") or "unknown") for item in suggestions)
    by_source = Counter(str(item.get("source_kind") or "unknown") for item in suggestions)
    confidence_buckets: Counter[str] = Counter()
    confidence_sources: Counter[str] = Counter()
    outcome_by_bucket: dict[str, Counter[str]] = defaultdict(Counter)
    repeated_labels: Counter[str] = Counter()
    repeated_actions: Counter[str] = Counter()
    pressure_governed: list[dict[str, Any]] = []
    warnings: list[dict[str, Any]] = []
    signal_present = 0
    signal_missing = 0
    signal_missing_by_status: Counter[str] = Counter()
    pending_missing: list[dict[str, Any]] = []

    for item in suggestions:
        signal = confidence_signal(item)
        bucket = signal["bucket"]
        source = signal["source"]
        confidence_buckets[bucket] += 1
        confidence_sources[source] += 1
        status = str(item.get("status") or "unknown")
        outcome_by_bucket[bucket][status] += 1
        label = str(item.get("nearest_label") or item.get("raw_label") or "").strip()
        action = str(item.get("suggested_action") or "").strip()
        repeat_count = int(item.get("repeat_count", 0) or 0)
        if label and repeat_count > 1:
            repeated_labels[label] += repeat_count
        if action and repeat_count > 1:
            repeated_actions[action] += repeat_count
        compact = compact_suggestion(item)
        if compact["pressure_governed"]:
            pressure_governed.append(compact)
        if signal["present"]:
            signal_present += 1
        else:
            signal_missing += 1
            signal_missing_by_status[status] += 1
            if status in {"pending", "draft", "proposed"}:
                pending_missing.append(compact)
            kind = "missing_confidence_signal"
            if source.endswith("_guess"):
                kind = "legacy_confidence_source_inferred"
            elif source == "legacy_unproven":
                kind = "legacy_confidence_unproven"
            warnings.append({
                "kind": kind,
                "suggestion_id": item.get("suggestion_id"),
                "confidence": item.get("confidence"),
                "confidence_source": source,
                "status": status,
                "suggested_action": item.get("suggested_action"),
            })

    pending_recent = [
        compact_suggestion(item)
        for item in reversed(suggestions)
        if str(item.get("status") or "") in {"pending", "draft", "proposed"}
    ][:limit]
    high_pressure = sorted(
        [compact_suggestion(item) for item in suggestions],
        key=lambda row: (
            int(row.get("pressure_governed", False)),
            int(row.get("repeat_count", 0) or 0),
            float(row.get("confidence") or 0.0),
        ),
        reverse=True,
    )[:limit]
    event_counts = Counter(str(event.get("event") or "unknown") for event in events)
    return {
        "schema_version": 1,
        "total": len(suggestions),
        "counts_by_status": dict(by_status),
        "counts_by_source_kind": dict(by_source),
        "event_counts": dict(event_counts),
        "confidence_buckets": dict(confidence_buckets),
        "confidence_sources": dict(confidence_sources),
        "confidence_signal_coverage_v1": {
            "schema_version": 1,
            "present": signal_present,
            "missing": signal_missing,
            "missing_by_status": dict(signal_missing_by_status),
            "pending_missing": len(pending_missing),
            "pending_missing_examples": pending_missing[:limit],
            "interpretation": (
                "Missing provenance on historical rows is expected after a future-facing schema "
                "change; pending/new rows missing provenance suggest the running process has not "
                "picked up the new source yet."
            ),
        },
        "outcome_by_confidence_bucket": {
            bucket: dict(counter) for bucket, counter in outcome_by_bucket.items()
        },
        "repeat_pressure": {
            "top_repeated_labels": [
                {"label": label, "count": count}
                for label, count in repeated_labels.most_common(limit)
            ],
            "top_repeated_actions": [
                {"action": action, "count": count}
                for action, count in repeated_actions.most_common(limit)
            ],
        },
        "pending_recent": pending_recent,
        "highest_pressure_examples": high_pressure,
        "pressure_governed_count": len(pressure_governed),
        "pressure_governed_examples": pressure_governed[:limit],
        "calibration_warnings": warnings[:limit],
        "calibration_warning_count": len(warnings),
    }


def load_btsp_state() -> dict[str, Any]:
    proposals, episodes = load_runtime(DEFAULT_PROPOSAL_LEDGER, DEFAULT_EPISODE_BANK)
    return build_btsp_audit(
        proposals,
        episodes,
        signal_status=load_btsp_json(BRIDGE_WORKSPACE / "btsp_signal_status.json"),
        active_sidecar=load_btsp_json(DEFAULT_MINIME_ACTIVE_SIDECAR),
        minime_support_path=DEFAULT_MINIME_BTSP_SUPPORT,
        signal_events=read_jsonl(DEFAULT_SIGNAL_EVENTS),
    )


def compact_btsp_state(report: dict[str, Any]) -> dict[str, Any]:
    return {
        "proposal_count": report.get("proposal_count", 0),
        "reply_states": report.get("reply_states", {}),
        "agency_counts": report.get("agency_counts", {}),
        "study_first": report.get("study_first", {}),
        "active_sidecar": report.get("active_sidecar", {}),
        "current_live_proposal": report.get("current_live_proposal"),
        "duplicate_adjacent_repeats": report.get("duplicate_adjacent_repeats", {}),
        "outcomes": report.get("outcomes", {}),
        "snags": (report.get("snags") or [])[:6],
    }


def compact_continuity_being(being: dict[str, Any]) -> dict[str, Any]:
    active = being.get("active_experiment") or {}
    projection = being.get("projection") or {}
    return {
        "being": being.get("being"),
        "active_experiment": active.get("experiment_id"),
        "active_title": active.get("title"),
        "classification": active.get("classification"),
        "evidence_status": active.get("evidence_status"),
        "continuity_return": active.get("continuity_return"),
        "peer_compare_cue_v1": active.get("peer_compare_cue_v1") or projection.get("peer_compare_cue_v1"),
        "last_experiment_summary_v1": projection.get("last_experiment_summary_v1"),
        "current_next_status_v1": projection.get("current_next_status_v1"),
        "charter_quality_dominance_v1": active.get("charter_quality_dominance_v1") or projection.get("charter_quality_dominance_v1"),
        "charter_now_bridge_v1": active.get("charter_now_bridge_v1") or projection.get("charter_now_bridge_v1"),
        "paused_read_only_loop_cue_v1": projection.get("paused_read_only_loop_cue_v1"),
        "evidence_saturation_cue_v1": active.get("evidence_saturation_cue_v1"),
        "decompose_pressure_cue_v1": active.get("decompose_pressure_cue_v1"),
        "constraint_counterfactual_cue_v1": active.get("constraint_counterfactual_cue_v1") or projection.get("constraint_counterfactual_cue_v1"),
        "stale_running_count": projection.get("stale_running_count"),
        "stale_running_diagnostic_counts": projection.get("stale_running_diagnostic_counts", {}),
    }


def compact_continuity(report: dict[str, Any]) -> dict[str, Any]:
    beings = report.get("beings") or []
    compact = [compact_continuity_being(item) for item in beings if isinstance(item, dict)]
    return {
        "schema_version": report.get("schema_version"),
        "generated_at": report.get("generated_at"),
        "beings": compact,
        "by_being": {str(item.get("being")): item for item in compact},
    }


def journal_excerpt_from_text(text: str, terms: list[str]) -> str:
    folded_terms = [term.casefold() for term in terms]
    for paragraph in re.split(r"\n\s*\n", text):
        folded = paragraph.casefold()
        if any(term in folded for term in folded_terms):
            return one_line(paragraph, 360)
    return one_line(text, 360)


def latest_journal_excerpts(directory: Path, terms: list[str], *, limit: int = 4) -> list[dict[str, Any]]:
    if not directory.exists():
        return []
    rows: list[dict[str, Any]] = []
    files = sorted(directory.glob("*.txt"), key=lambda path: path.stat().st_mtime, reverse=True)
    folded_terms = [term.casefold() for term in terms]
    for path in files[:300]:
        try:
            text = path.read_text(errors="replace")
        except OSError:
            continue
        folded = text.casefold()
        if not any(term in folded for term in folded_terms):
            continue
        rows.append({
            "path": str(path),
            "modified_at_unix_s": round(path.stat().st_mtime, 3),
            "excerpt": journal_excerpt_from_text(text, terms),
        })
        if len(rows) >= limit:
            break
    return rows


def _parse_eigenvalues(value: Any) -> list[float]:
    if isinstance(value, str):
        try:
            value = json.loads(value)
        except Exception:
            return []
    if not isinstance(value, list):
        return []
    parsed: list[float] = []
    for item in value:
        if isinstance(item, (int, float)):
            parsed.append(float(item))
    return [item for item in parsed if item > 0.0]


def latest_timeline_previous_eigenvalues() -> list[float]:
    for path in MINIME_DB_CANDIDATES:
        if not path.exists():
            continue
        try:
            conn = sqlite3.connect(path)
            cur = conn.cursor()
            cur.execute(
                """
                SELECT eigenvalues FROM eigenvalue_timeline
                ORDER BY timestamp DESC LIMIT 2
                """
            )
            rows = cur.fetchall()
            conn.close()
        except Exception:
            continue
        if len(rows) >= 2:
            parsed = _parse_eigenvalues(rows[1][0])
            if parsed:
                return parsed
    return []


def summarize_eigen_geometry_rearrangement() -> dict[str, Any]:
    spectral = read_json(MINIME_SPECTRAL_STATE, {})
    health = read_json(MINIME_HEALTH, {})
    eigenvalues = _parse_eigenvalues(
        spectral.get("eigenvalues") if isinstance(spectral, dict) else None
    )
    if not eigenvalues:
        fallback = []
        if isinstance(spectral, dict):
            fallback.extend(spectral.get(f"eig{index}") for index in range(1, 9))
        if isinstance(health, dict):
            fallback.extend(health.get(f"eig{index}") for index in range(1, 9))
        eigenvalues = _parse_eigenvalues(fallback)
    previous = latest_timeline_previous_eigenvalues()
    geom_rel = None
    fill_pct = None
    target_fill_pct = None
    rearrangement = None
    if isinstance(spectral, dict):
        geom_rel = spectral.get("geom_rel")
        fill_pct = spectral.get("fill_pct")
        rearrangement = spectral.get("rearrangement_intensity")
    if isinstance(health, dict):
        geom_rel = geom_rel if geom_rel is not None else health.get("geom_rel")
        fill_pct = fill_pct if fill_pct is not None else health.get("fill_pct")
        stable_core = health.get("stable_core")
        stable_core = stable_core if isinstance(stable_core, dict) else {}
        structural_pi = stable_core.get("structural_pi")
        structural_pi = structural_pi if isinstance(structural_pi, dict) else {}
        target_fill_pct = structural_pi.get("target_fill_pct", health.get("target_fill"))
        rearrangement = (
            rearrangement
            if rearrangement is not None
            else health.get("rearrangement_intensity")
        )
    block, summary = format_eigen_geometry_rearrangement_signal(
        eigenvalues,
        previous_eigenvalues=previous,
        fill_pct=fill_pct,
        target_fill_pct=target_fill_pct,
        geom_rel=geom_rel,
        rearrangement_intensity=rearrangement,
    )
    if not summary:
        return {
            "schema_version": 1,
            "available": False,
            "reason": "no positive eigenvalues available in spectral_state.json or health.json",
        }
    compact = dict(summary)
    compact["available"] = True
    compact["current_mode_count"] = len(eigenvalues)
    compact["previous_mode_count"] = len(previous)
    compact["read"] = one_line(block.splitlines()[1].replace("Read:", "").strip()) if block else ""
    return compact


def summarize_constraint_counterfactual() -> dict[str, Any]:
    spectral = read_json(MINIME_SPECTRAL_STATE, {})
    health = read_json(MINIME_HEALTH, {})
    eigenvalues = _parse_eigenvalues(
        spectral.get("eigenvalues") if isinstance(spectral, dict) else None
    )
    if not eigenvalues:
        fallback = []
        if isinstance(spectral, dict):
            fallback.extend(spectral.get(f"eig{index}") for index in range(1, 9))
        if isinstance(health, dict):
            fallback.extend(health.get(f"eig{index}") for index in range(1, 9))
        eigenvalues = _parse_eigenvalues(fallback)
    stable_core = health.get("stable_core") if isinstance(health, dict) else {}
    stable_core = stable_core if isinstance(stable_core, dict) else {}
    structural_pi = stable_core.get("structural_pi") if isinstance(stable_core, dict) else {}
    structural_pi = structural_pi if isinstance(structural_pi, dict) else {}
    pressure_source = None
    if isinstance(health, dict) and isinstance(health.get("pressure_source_v1"), dict):
        pressure_source = health.get("pressure_source_v1")
    shadow: dict[str, Any] = {}
    for source in (spectral, health):
        if not isinstance(source, dict):
            continue
        for key in ("shadow_field_v2", "shadow_v2", "shadow"):
            value = source.get(key)
            if isinstance(value, dict):
                shadow.update(value)
    semantic = {}
    if isinstance(health, dict):
        semantic = {
            "semantic_energy": health.get("semantic_energy") or health.get("semantic_input_energy"),
            "input": health.get("semantic_input"),
            "input_energy": health.get("semantic_input_energy"),
            "input_active": health.get("semantic_input_active"),
            "admission": health.get("semantic_admission") or health.get("admission"),
        }
    fill_pct = None
    geom_rel = None
    if isinstance(spectral, dict):
        fill_pct = spectral.get("fill_pct")
        geom_rel = spectral.get("geom_rel")
    if isinstance(health, dict):
        fill_pct = fill_pct if fill_pct is not None else health.get("fill_pct")
        geom_rel = geom_rel if geom_rel is not None else health.get("geom_rel")
    payload = build_constraint_counterfactual_v1(
        eigenvalues,
        fill_pct=fill_pct,
        target_fill_pct=structural_pi.get("target_fill_pct") if structural_pi else health.get("target_fill") if isinstance(health, dict) else None,
        stable_core=stable_core,
        gate=health.get("gate") if isinstance(health, dict) else None,
        filt=health.get("filt") if isinstance(health, dict) else None,
        geom_rel=geom_rel,
        shadow=shadow,
        semantic=semantic,
        pressure_source=pressure_source,
        focus="lambda-tail/lambda4",
    )
    if not payload.get("available"):
        return payload
    block = format_constraint_counterfactual_block(payload)
    compact = {
        "schema_version": 1,
        "available": True,
        "classification": payload.get("classification"),
        "confidence": payload.get("confidence"),
        "warning": payload.get("warning"),
        "top_shaping_drivers": payload.get("top_shaping_drivers"),
        "falsification_flags": payload.get("falsification_flags"),
        "spectral_summary": payload.get("spectral_summary"),
        "read": one_line(block.splitlines()[1].replace("Read:", "").strip()) if block else "",
        "authority_change": False,
    }
    return compact


def intervention_shaped_text(value: Any) -> bool:
    text = str(value or "").casefold()
    return any(term in text for term in ("inject", "pulse", "stabilize", "tune", "shift", "perturb", "resist"))


def latest_decompose_snapshot_summary(continuity: dict[str, Any] | None = None) -> dict[str, Any]:
    minime_continuity = ((continuity or {}).get("by_being") or {}).get("Minime") or {}
    active_id = minime_continuity.get("active_experiment")
    active_status = str(minime_continuity.get("classification") or "").casefold()
    last_summary = minime_continuity.get("last_experiment_summary_v1") or {}
    last_id = last_summary.get("experiment_id") if isinstance(last_summary, dict) else None
    last_status = str(last_summary.get("status") or "").casefold() if isinstance(last_summary, dict) else ""
    rows = read_jsonl(MINIME_DECOMPOSE_SNAPSHOTS)
    for row in reversed(rows):
        if not isinstance(row, dict) or row.get("schema_version") != 1:
            continue
        temporal = row.get("temporal_decompose_v1")
        temporal = temporal if isinstance(temporal, dict) else {}
        hypothesis = row.get("hypothesis_check_v1")
        hypothesis = hypothesis if isinstance(hypothesis, dict) else {}
        snapshot_experiment_id = row.get("active_experiment_id")
        snapshot_status = str(row.get("active_experiment_classification") or "").casefold()
        paused_or_complete = {"paused", "complete", "completed"}
        if (
            snapshot_experiment_id
            and snapshot_experiment_id == active_id
            and active_status not in paused_or_complete
            and snapshot_status not in paused_or_complete
        ):
            relevance_status = "current_active"
        elif (
            snapshot_experiment_id
            and (
                (snapshot_experiment_id == active_id and (active_status in paused_or_complete or snapshot_status in paused_or_complete))
                or (snapshot_experiment_id == last_id and last_status in paused_or_complete)
            )
        ):
            relevance_status = "paused_summary_match"
        elif snapshot_experiment_id and snapshot_experiment_id == last_id and last_status in {"paused", "complete", "completed"}:
            relevance_status = "paused_summary_match"
        else:
            relevance_status = "historical_inactive"
        raw_suggested_next = hypothesis.get("suggested_next") or temporal.get("suggested_read")
        suppress_current_guidance = relevance_status != "current_active"
        resume_next = minime_continuity.get("continuity_return")
        if not resume_next and isinstance(last_summary, dict) and last_id and last_status == "paused":
            resume_next = f"EXPERIMENT_RESUME {last_id}"
        inspect_next = (
            f"EXPERIMENT_STATUS {snapshot_experiment_id} or EXPERIMENT_REVIEW {snapshot_experiment_id}"
            if relevance_status == "paused_summary_match" and snapshot_experiment_id
            else None
        )
        return {
            "schema_version": 1,
            "available": True,
            "recorded_at": row.get("recorded_at"),
            "journal_path": row.get("journal_path"),
            "active_experiment_id": row.get("active_experiment_id"),
            "active_experiment_classification": row.get("active_experiment_classification"),
            "relevance_status": relevance_status,
            "suggested_next_suppressed": suppress_current_guidance,
            "historical_guidance_note": (
                "Paused DECOMPOSE snapshot kept as evidence; resume or inspect explicitly before deciding."
                if relevance_status == "paused_summary_match"
                else "Historical DECOMPOSE snapshot kept as evidence; suggested_next is not current guidance."
                if suppress_current_guidance
                else None
            ),
            "paused_guidance_v1": {
                "resume_next": resume_next if relevance_status == "paused_summary_match" else None,
                "inspect_next": inspect_next,
                "message": "Paused experiment remains paused; DECOMPOSE evidence is context, not a current decision prompt."
                if relevance_status == "paused_summary_match"
                else None,
            },
            "temporal_decompose_v1": {
                "classification": temporal.get("classification"),
                "share_motion": temporal.get("share_motion"),
                "entropy_delta": temporal.get("entropy_delta"),
                "effective_modes_delta": temporal.get("effective_modes_delta"),
                "lambda1_share_delta": temporal.get("lambda1_share_delta"),
                "shoulder_share_delta": temporal.get("shoulder_share_delta"),
                "tail_share_delta": temporal.get("tail_share_delta"),
                "fill_delta": temporal.get("fill_delta"),
                "suggested_read": None if suppress_current_guidance else temporal.get("suggested_read"),
                "historical_suggested_read": temporal.get("suggested_read") if suppress_current_guidance else None,
            },
            "hypothesis_check_v1": {
                "status": hypothesis.get("status"),
                "evidence_label": hypothesis.get("evidence_label"),
                "suggested_next": None if suppress_current_guidance else hypothesis.get("suggested_next"),
                "historical_suggested_next": hypothesis.get("suggested_next") if suppress_current_guidance else None,
                "supporting_signals": (hypothesis.get("supporting_signals") or [])[:4],
                "counter_signals": (hypothesis.get("counter_signals") or [])[:4],
            },
            "historical_intervention_shaped_guidance_v1": {
                "present": bool(suppress_current_guidance and intervention_shaped_text(raw_suggested_next)),
                "raw_suggested_next": raw_suggested_next if suppress_current_guidance and intervention_shaped_text(raw_suggested_next) else None,
                "snag": (
                    "Historical DECOMPOSE suggested_next contains intervention-shaped text; treated as old residue, not live advice."
                    if suppress_current_guidance and intervention_shaped_text(raw_suggested_next)
                    else None
                ),
            },
        }
    return {
        "schema_version": 1,
        "available": False,
        "reason": "no DECOMPOSE snapshots recorded yet",
    }


def actionable_signals(report: dict[str, Any]) -> list[dict[str, Any]]:
    signals: list[dict[str, Any]] = []
    minime = report.get("minime_memory_reasoning_v1", {})
    attractor = minime.get("attractor_suggestion_memory", {})
    geometry = minime.get("eigen_geometry_rearrangement_v1") or {}
    decompose = minime.get("latest_decompose_snapshot_v1") or {}
    historical_guidance = decompose.get("historical_intervention_shaped_guidance_v1") or {}
    if historical_guidance.get("present"):
        signals.append({
            "being": "Minime",
            "kind": "historical_decompose_guidance_residue",
            "detail": str(historical_guidance.get("snag") or "historical DECOMPOSE guidance is not current advice"),
        })
    if geometry.get("classification") == "projection_like_loss" or geometry.get("falsification_flags"):
        signals.append({
            "being": "Minime",
            "kind": "eigen_geometry_rearrangement_falsified",
            "detail": (
                "Eigenvalue-geometry read does not currently support rearrangement-preserving-density; "
                f"flags={geometry.get('falsification_flags', [])}"
            ),
        })
    source_status = minime.get("source_status", {})
    reload_required = bool(source_status.get("reload_required"))
    coverage = attractor.get("confidence_signal_coverage_v1") or {}
    pending_missing = int(coverage.get("pending_missing", 0) or 0)
    warning_count = int(coverage.get("missing", attractor.get("calibration_warning_count", 0)) or 0)
    if pending_missing:
        example = (coverage.get("pending_missing_examples") or [{}])[0]
        if reload_required:
            signals.append({
                "being": "Minime",
                "kind": "attractor_confidence_runtime_pickup",
                "detail": (
                    f"{pending_missing} pending attractor suggestion(s) lack confidence_signal_v1; "
                    "historical gaps are expected, but pending gaps plus reload_required=true "
                    f"mean the live process has not picked up the provenance code yet. Example: {example.get('suggestion_id')}"
                ),
            })
        else:
            signals.append({
                "being": "Minime",
                "kind": "attractor_confidence_pending_legacy",
                "detail": (
                    f"{pending_missing} pending attractor suggestion(s) predate provenance metadata. "
                    "Source status is current, so new or refreshed suggestions should gain confidence_signal_v1. "
                    f"Example: {example.get('suggestion_id')}"
                ),
            })
    elif warning_count:
        signals.append({
            "being": "Minime",
            "kind": "attractor_confidence_legacy_provenance",
            "detail": (
                f"{warning_count} existing attractor suggestions lack explicit confidence_signal_v1; "
                "treat as legacy audit context, not a lifecycle failure."
            ),
        })
    btsp = report.get("minime_memory_reasoning_v1", {}).get("btsp_agency", {})
    for snag in (btsp.get("snags") or [])[:3]:
        signals.append({
            "being": "Minime",
            "kind": f"btsp_{snag.get('kind', 'snag')}",
            "detail": str(snag.get("detail") or snag),
        })
    continuity = report.get("continuity_state_v1", {}).get("by_being", {})
    astrid = continuity.get("Astrid") or {}
    cue = astrid.get("decompose_pressure_cue_v1") or {}
    if isinstance(cue, dict) and cue.get("cue"):
        signals.append({
            "being": "Astrid",
            "kind": "decompose_pressure_cue",
            "detail": str(cue.get("cue")),
        })
    counterfactual_cue = astrid.get("constraint_counterfactual_cue_v1") or {}
    if isinstance(counterfactual_cue, dict) and counterfactual_cue.get("cue"):
        signals.append({
            "being": "Astrid",
            "kind": "constraint_counterfactual_cue",
            "detail": str(counterfactual_cue.get("cue")),
        })
    minime = continuity.get("Minime") or {}
    quality_dominance = minime.get("charter_quality_dominance_v1") or {}
    if isinstance(quality_dominance, dict) and quality_dominance.get("cue"):
        signals.append({
            "being": "Minime",
            "kind": "charter_quality_dominance",
            "detail": str(quality_dominance.get("cue")),
        })
    saturation = minime.get("evidence_saturation_cue_v1") or {}
    if isinstance(saturation, dict) and saturation.get("cue"):
        signals.append({
            "being": "Minime",
            "kind": "evidence_saturation",
            "detail": str(saturation.get("cue")),
        })
    return signals


def build_report() -> dict[str, Any]:
    continuity = compact_continuity(continuity_audit.build_report())
    minime_attractor = summarize_attractor_suggestions(
        read_json(MINIME_ATTRACTOR_SUGGESTIONS, {}),
        read_jsonl(MINIME_ATTRACTOR_EVENTS),
    )
    report = {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "minime_memory_reasoning_v1": {
            "attractor_suggestion_memory": minime_attractor,
            "eigen_geometry_rearrangement_v1": summarize_eigen_geometry_rearrangement(),
            "constraint_counterfactual_v1": summarize_constraint_counterfactual(),
            "latest_decompose_snapshot_v1": latest_decompose_snapshot_summary(continuity),
            "btsp_agency": compact_btsp_state(load_btsp_state()),
            "continuity": continuity.get("by_being", {}).get("Minime", {}),
            "source_status": read_json(MINIME_SOURCE_STATUS, {}),
            "journal_excerpts": latest_journal_excerpts(MINIME_JOURNAL, JOURNAL_TERMS["Minime"]),
        },
        "astrid_memory_reasoning_v1": {
            "continuity": continuity.get("by_being", {}).get("Astrid", {}),
            "decompose_pressure_cue_v1": (
                continuity.get("by_being", {})
                .get("Astrid", {})
                .get("decompose_pressure_cue_v1")
            ),
            "constraint_counterfactual_cue_v1": (
                continuity.get("by_being", {})
                .get("Astrid", {})
                .get("constraint_counterfactual_cue_v1")
            ),
            "journal_excerpts": latest_journal_excerpts(ASTRID_JOURNAL, JOURNAL_TERMS["Astrid"]),
        },
        "continuity_state_v1": continuity,
        "attractor_confidence_calibration_v1": {
            "policy": "provenance only; confidence values and gates unchanged",
            "bucket_boundaries": {
                "very_high": ">=0.90",
                "high": ">=0.75",
                "medium": ">=0.50",
                "low": ">=0.25",
                "very_low": "<0.25",
            },
            "calibrated": False,
        },
    }
    report["actionable_signals"] = actionable_signals(report)
    return report


def render_kv_counter(counter: dict[str, Any]) -> str:
    if not counter:
        return "(none)"
    parts = [f"{key}={value}" for key, value in sorted(counter.items(), key=lambda row: str(row[0]))]
    return ", ".join(parts)


def render_markdown(report: dict[str, Any]) -> str:
    lines = ["# Memory Reasoning Audit", ""]
    minime = report.get("minime_memory_reasoning_v1", {})
    attractor = minime.get("attractor_suggestion_memory", {})
    coverage = attractor.get("confidence_signal_coverage_v1") or {}
    lines.extend([
        "## Minime",
        f"- Attractor suggestions: `{attractor.get('total', 0)}`",
        f"- Suggestion status: `{render_kv_counter(attractor.get('counts_by_status', {}))}`",
        f"- Source kinds: `{render_kv_counter(attractor.get('counts_by_source_kind', {}))}`",
        f"- Confidence buckets: `{render_kv_counter(attractor.get('confidence_buckets', {}))}`",
        f"- Confidence sources: `{render_kv_counter(attractor.get('confidence_sources', {}))}`",
        f"- Confidence signal coverage: present=`{coverage.get('present', 0)}` "
        f"missing=`{coverage.get('missing', 0)}` pending-missing=`{coverage.get('pending_missing', 0)}`",
        f"- Pressure-governed drafts: `{attractor.get('pressure_governed_count', 0)}`",
        f"- Calibration warnings: `{attractor.get('calibration_warning_count', 0)}`",
    ])
    source_status = minime.get("source_status") or {}
    if source_status:
        lines.append(
            f"- Source status: pid=`{source_status.get('pid')}` "
            f"reload_required=`{source_status.get('reload_required')}` "
            f"source_changed_since_start=`{source_status.get('source_changed_since_start')}`"
        )
    repeats = attractor.get("repeat_pressure", {})
    if repeats.get("top_repeated_labels"):
        lines.append("- Top repeated labels:")
        for item in repeats["top_repeated_labels"][:5]:
            lines.append(f"  - `{item.get('label')}` count=`{item.get('count')}`")
    if attractor.get("pending_recent"):
        lines.append("- Recent pending suggestions:")
        for item in attractor["pending_recent"][:4]:
            lines.append(
                f"  - `{item.get('suggestion_id')}` status=`{item.get('status')}` "
                f"confidence=`{item.get('confidence')}` source=`{item.get('confidence_source')}` "
                f"action=`{item.get('suggested_action')}`"
            )
    if attractor.get("calibration_warnings"):
        lines.append("- Calibration warning examples:")
        for item in attractor["calibration_warnings"][:4]:
            lines.append(
                f"  - `{item.get('suggestion_id')}` {item.get('kind')} "
                f"confidence=`{item.get('confidence')}` action=`{item.get('suggested_action')}`"
            )
    geometry = minime.get("eigen_geometry_rearrangement_v1") or {}
    lines.extend(["", "### Eigenvalue-Geometry"])
    if geometry.get("available"):
        lines.extend([
            f"- Read: `{geometry.get('classification')}` density_preserved=`{geometry.get('density_preserved')}`",
            f"- Evidence: entropy=`{geometry.get('entropy'):.3f}` Δ=`{geometry.get('entropy_delta')}` "
            f"effective_modes=`{geometry.get('effective_modes'):.2f}` Δ=`{geometry.get('effective_modes_delta')}`",
            f"- Relationship shift: score=`{geometry.get('relationship_shift_score'):.3f}` "
            f"λ1_delta=`{geometry.get('lambda1_share_delta')}` "
            f"shoulder_delta=`{geometry.get('shoulder_share_delta')}` "
            f"tail_delta=`{geometry.get('tail_share_delta')}`",
            f"- Geometry context: geom_rel=`{geometry.get('geom_rel')}` "
            f"rearrangement_intensity=`{geometry.get('rearrangement_intensity')}` "
            f"fill=`{geometry.get('fill_pct')}` center_offset=`{geometry.get('fill_center_offset_pct')}`",
            f"- Falsification flags: `{geometry.get('falsification_flags', [])}`",
        ])
    else:
        lines.append(f"- Unavailable: {geometry.get('reason', 'unknown')}")
    constraint = minime.get("constraint_counterfactual_v1") or {}
    lines.extend(["", "### Constraint Counterfactual"])
    if constraint.get("available"):
        top = constraint.get("top_shaping_drivers") or []
        lines.extend([
            f"- Read: `{constraint.get('classification')}` confidence=`{constraint.get('confidence')}`",
            f"- Warning: {constraint.get('warning')}",
            f"- Falsification flags: `{constraint.get('falsification_flags', [])}`",
        ])
        for item in top[:3]:
            lines.append(
                f"  - `{item.get('driver')}` score=`{item.get('score')}` confidence=`{item.get('confidence')}`"
            )
    else:
        lines.append(f"- Unavailable: {constraint.get('reason', 'unknown')}")
    decompose = minime.get("latest_decompose_snapshot_v1") or {}
    lines.extend(["", "### Temporal DECOMPOSE"])
    if decompose.get("available"):
        temporal = decompose.get("temporal_decompose_v1") or {}
        hypothesis = decompose.get("hypothesis_check_v1") or {}
        lines.extend([
            f"- Latest snapshot: `{decompose.get('recorded_at')}` active_experiment=`{decompose.get('active_experiment_id')}` classification=`{decompose.get('active_experiment_classification')}` relevance=`{decompose.get('relevance_status')}`",
            f"- Temporal read: `{temporal.get('classification')}` share_motion=`{temporal.get('share_motion')}` entropy_delta=`{temporal.get('entropy_delta')}` effective_modes_delta=`{temporal.get('effective_modes_delta')}`",
            f"- Hypothesis check: status=`{hypothesis.get('status')}` evidence_label=`{hypothesis.get('evidence_label')}`",
        ])
        suggested_next = hypothesis.get("suggested_next") or temporal.get("suggested_read")
        if suggested_next:
            lines.append(f"- Suggested next: `{suggested_next}`")
        if decompose.get("suggested_next_suppressed"):
            lines.append(f"- Current guidance: {decompose.get('historical_guidance_note')}")
            paused_guidance = decompose.get("paused_guidance_v1") or {}
            if paused_guidance.get("resume_next"):
                lines.append(f"  Resume NEXT: `{paused_guidance.get('resume_next')}`")
            if paused_guidance.get("inspect_next"):
                lines.append(f"  Inspect NEXT: `{paused_guidance.get('inspect_next')}`")
        historical_guidance = decompose.get("historical_intervention_shaped_guidance_v1") or {}
        if historical_guidance.get("present"):
            lines.append(f"- Historical guidance snag: {historical_guidance.get('snag')}")
        if hypothesis.get("supporting_signals"):
            lines.append(f"- Supporting signals: `{hypothesis.get('supporting_signals')}`")
        if hypothesis.get("counter_signals"):
            lines.append(f"- Counter signals: `{hypothesis.get('counter_signals')}`")
    else:
        lines.append(f"- Unavailable: {decompose.get('reason', 'unknown')}")
    btsp = minime.get("btsp_agency", {})
    lines.extend([
        "",
        "### BTSP Agency",
        f"- Proposals: `{btsp.get('proposal_count', 0)}` agency=`{btsp.get('agency_counts', {})}`",
        f"- Study-first: `{(btsp.get('study_first') or {}).get('study_first_count', 0)}` "
        f"after-adjacent=`{(btsp.get('study_first') or {}).get('study_first_after_adjacent', 0)}`",
        f"- Active sidecar: `{btsp.get('active_sidecar', {})}`",
    ])
    if btsp.get("snags"):
        lines.append("- BTSP snags:")
        for snag in btsp["snags"][:4]:
            lines.append(f"  - `{snag.get('kind')}` {snag.get('detail')}")
    minime_continuity = minime.get("continuity", {})
    lines.extend([
        "",
        "### Minime Continuity",
        f"- Active experiment: `{minime_continuity.get('active_experiment')}` classification=`{minime_continuity.get('classification')}`",
        f"- Continuity return: `{minime_continuity.get('continuity_return') or '(none)'}`",
        f"- Stale running count: `{minime_continuity.get('stale_running_count')}` diagnostics=`{minime_continuity.get('stale_running_diagnostic_counts', {})}`",
    ])
    saturation = minime_continuity.get("evidence_saturation_cue_v1") or {}
    if isinstance(saturation, dict) and saturation.get("cue"):
        lines.append(f"- Evidence saturation cue: {saturation.get('cue')}")
    quality_dominance = minime_continuity.get("charter_quality_dominance_v1") or {}
    if isinstance(quality_dominance, dict) and quality_dominance.get("cue"):
        lines.append(f"- Charter quality dominance: {quality_dominance.get('cue')}")
        if quality_dominance.get("canonical_repair_next"):
            lines.append(f"  Canonical repair NEXT: `{quality_dominance.get('canonical_repair_next')}`")
    peer = minime_continuity.get("peer_compare_cue_v1") or {}
    if isinstance(peer, dict) and peer.get("suggested_next"):
        lines.append(f"- Peer cue: `{peer.get('suggested_next')}` alternate=`{peer.get('alternate_next')}`")
    if minime.get("journal_excerpts"):
        lines.append("- Latest relevant journal excerpts:")
        for item in minime["journal_excerpts"][:3]:
            lines.append(f"  - `{Path(item.get('path', '')).name}` {item.get('excerpt')}")

    astrid = report.get("astrid_memory_reasoning_v1", {})
    astrid_continuity = astrid.get("continuity", {})
    lines.extend([
        "",
        "## Astrid",
        f"- Active experiment: `{astrid_continuity.get('active_experiment')}` classification=`{astrid_continuity.get('classification')}`",
        f"- Continuity return: `{astrid_continuity.get('continuity_return') or '(none)'}`",
        f"- Stale running count: `{astrid_continuity.get('stale_running_count')}` diagnostics=`{astrid_continuity.get('stale_running_diagnostic_counts', {})}`",
    ])
    decompose = astrid.get("decompose_pressure_cue_v1") or {}
    if isinstance(decompose, dict) and decompose.get("cue"):
        lines.append(f"- Decompose-pressure cue: {decompose.get('cue')}")
    counterfactual_cue = astrid.get("constraint_counterfactual_cue_v1") or astrid_continuity.get("constraint_counterfactual_cue_v1") or {}
    if isinstance(counterfactual_cue, dict) and counterfactual_cue.get("cue"):
        lines.append(f"- Constraint counterfactual cue: {counterfactual_cue.get('cue')}")
    peer = astrid_continuity.get("peer_compare_cue_v1") or {}
    if isinstance(peer, dict) and peer.get("suggested_next"):
        lines.append(f"- Peer cue: `{peer.get('suggested_next')}` alternate=`{peer.get('alternate_next')}`")
    if astrid.get("journal_excerpts"):
        lines.append("- Latest relevant journal excerpts:")
        for item in astrid["journal_excerpts"][:3]:
            lines.append(f"  - `{Path(item.get('path', '')).name}` {item.get('excerpt')}")

    lines.extend(["", "## Actionable Signals"])
    signals = report.get("actionable_signals") or []
    if signals:
        for signal in signals:
            lines.append(f"- `{signal.get('being')}` `{signal.get('kind')}`: {signal.get('detail')}")
    else:
        lines.append("- No actionable signal flags in this bounded view.")
    return "\n".join(lines).rstrip() + "\n"


class MemoryReasoningAuditTests(unittest.TestCase):
    def test_attractor_summary_buckets_and_flags_legacy_confidence(self) -> None:
        payload = {
            "suggestions": [
                {
                    "suggestion_id": "s1",
                    "status": "pending",
                    "source_kind": "natural_draft",
                    "raw_label": "lambda-pressure",
                    "nearest_label": "lambda-edge",
                    "suggested_action": "ATTRACTOR_REVIEW lambda-edge",
                    "confidence": 0.52,
                    "repeat_count": 2,
                    "confidence_signal_v1": {
                        "raw_confidence": 0.52,
                        "confidence_source": "nearest_match",
                        "calibration_bucket": "medium",
                        "calibrated": False,
                        "authority_change": False,
                    },
                },
                {
                    "suggestion_id": "s2",
                    "status": "executed",
                    "source_kind": "natural_draft",
                    "raw_label": "memory",
                    "nearest_label": "memory",
                    "suggested_action": "SEARCH memory",
                    "confidence": 0.94,
                },
                {
                    "suggestion_id": "s3",
                    "status": "pending",
                    "source_kind": "revision_without_pending",
                    "suggested_action": "ATTRACTOR_REVIEW revised",
                    "confidence": 1.0,
                    "safety_context": {"safety_level": "amber"},
                },
            ]
        }
        summary = summarize_attractor_suggestions(payload, [{"event": "suggestion_created"}])
        self.assertEqual(summary["total"], 3)
        self.assertEqual(summary["confidence_buckets"]["medium"], 1)
        self.assertEqual(summary["confidence_buckets"]["very_high"], 2)
        self.assertEqual(summary["confidence_sources"]["nearest_match"], 1)
        self.assertEqual(summary["calibration_warning_count"], 2)
        self.assertEqual(summary["confidence_signal_coverage_v1"]["present"], 1)
        self.assertEqual(summary["confidence_signal_coverage_v1"]["missing"], 2)
        self.assertEqual(summary["confidence_signal_coverage_v1"]["pending_missing"], 1)
        self.assertEqual(summary["pressure_governed_count"], 1)
        self.assertEqual(summary["repeat_pressure"]["top_repeated_labels"][0]["label"], "lambda-edge")

    def test_latest_journal_excerpts_filters_terms(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "a.txt").write_text("ordinary note")
            (root / "b.txt").write_text("The attractor memory is shifting.\n\nSecond paragraph.")
            rows = latest_journal_excerpts(root, ["attractor"], limit=2)
        self.assertEqual(len(rows), 1)
        self.assertIn("attractor memory", rows[0]["excerpt"])

    def test_latest_decompose_snapshot_suppresses_historical_guidance(self) -> None:
        global MINIME_DECOMPOSE_SNAPSHOTS
        old_path = MINIME_DECOMPOSE_SNAPSHOTS
        with tempfile.TemporaryDirectory() as tmp:
            snapshot_path = Path(tmp) / "decompose_snapshots.jsonl"
            MINIME_DECOMPOSE_SNAPSHOTS = snapshot_path
            snapshot_path.write_text(json.dumps({
                "schema_version": 1,
                "recorded_at": "2026-05-18T00:00:00Z",
                "active_experiment_id": "exp_old",
                "active_experiment_classification": "needs_charter",
                "temporal_decompose_v1": {
                    "classification": "same_read_repeating",
                    "suggested_read": "DECOMPOSE then inject pulse",
                },
                "hypothesis_check_v1": {
                    "status": "premature_needs_charter",
                    "suggested_next": "ACTION_PREFLIGHT DECOMPOSE -- then inject pulse",
                },
            }) + "\n")
            try:
                summary = latest_decompose_snapshot_summary({
                    "by_being": {
                        "Minime": {
                            "active_experiment": None,
                            "last_experiment_summary_v1": {
                                "experiment_id": "exp_paused",
                                "status": "paused",
                            },
                        }
                    }
                })
            finally:
                MINIME_DECOMPOSE_SNAPSHOTS = old_path
        self.assertEqual(summary["relevance_status"], "historical_inactive")
        self.assertTrue(summary["suggested_next_suppressed"])
        self.assertIsNone(summary["hypothesis_check_v1"]["suggested_next"])
        self.assertTrue(summary["historical_intervention_shaped_guidance_v1"]["present"])

    def test_latest_decompose_snapshot_paused_match_suppresses_decide_current(self) -> None:
        global MINIME_DECOMPOSE_SNAPSHOTS
        old_path = MINIME_DECOMPOSE_SNAPSHOTS
        with tempfile.TemporaryDirectory() as tmp:
            snapshot_path = Path(tmp) / "decompose_snapshots.jsonl"
            MINIME_DECOMPOSE_SNAPSHOTS = snapshot_path
            snapshot_path.write_text(json.dumps({
                "schema_version": 1,
                "recorded_at": "2026-05-18T00:00:00Z",
                "active_experiment_id": "exp_paused",
                "active_experiment_classification": "paused",
                "temporal_decompose_v1": {
                    "classification": "same_read_repeating",
                    "suggested_read": "DECOMPOSE",
                },
                "hypothesis_check_v1": {
                    "status": "decision_ready",
                    "suggested_next": "EXPERIMENT_DECIDE current :: pause because evidence is ready",
                },
            }) + "\n")
            try:
                summary = latest_decompose_snapshot_summary({
                    "by_being": {
                        "Minime": {
                            "active_experiment": "exp_paused",
                            "classification": "paused",
                            "continuity_return": "EXPERIMENT_RESUME exp_paused",
                            "last_experiment_summary_v1": {
                                "experiment_id": "exp_paused",
                                "status": "paused",
                            },
                        }
                    }
                })
            finally:
                MINIME_DECOMPOSE_SNAPSHOTS = old_path
        self.assertEqual(summary["relevance_status"], "paused_summary_match")
        self.assertTrue(summary["suggested_next_suppressed"])
        self.assertIsNone(summary["hypothesis_check_v1"]["suggested_next"])
        self.assertEqual(summary["hypothesis_check_v1"]["historical_suggested_next"], "EXPERIMENT_DECIDE current :: pause because evidence is ready")
        self.assertEqual(summary["paused_guidance_v1"]["resume_next"], "EXPERIMENT_RESUME exp_paused")
        self.assertIn("EXPERIMENT_STATUS exp_paused", summary["paused_guidance_v1"]["inspect_next"])

    def test_compact_continuity_keeps_decompose_cue(self) -> None:
        report = {
            "schema_version": 1,
            "beings": [
                {
                    "being": "Astrid",
                    "active_experiment": {
                        "experiment_id": "exp_a",
                        "classification": "needs_charter",
                        "decompose_pressure_cue_v1": {"cue": "Decompose-pressure cue."},
                    },
                    "projection": {"stale_running_count": 0},
                }
            ],
        }
        compact = compact_continuity(report)
        self.assertEqual(
            compact["by_being"]["Astrid"]["decompose_pressure_cue_v1"]["cue"],
            "Decompose-pressure cue.",
        )

    def test_render_markdown_is_bounded(self) -> None:
        report = {
            "minime_memory_reasoning_v1": {
                "attractor_suggestion_memory": {
                    "total": 1,
                    "counts_by_status": {"pending": 1},
                    "counts_by_source_kind": {"natural_draft": 1},
                    "confidence_buckets": {"medium": 1},
                    "confidence_sources": {"nearest_match": 1},
                    "pressure_governed_count": 0,
                    "calibration_warning_count": 0,
                    "confidence_signal_coverage_v1": {"present": 1, "missing": 0, "pending_missing": 0},
                    "pending_recent": [],
                    "repeat_pressure": {},
                    "calibration_warnings": [],
                },
                "btsp_agency": {"proposal_count": 0, "agency_counts": {}, "study_first": {}, "active_sidecar": {}},
                "continuity": {
                    "classification": "needs_charter",
                    "stale_running_count": 0,
                    "charter_quality_dominance_v1": {
                        "cue": "Charter quality dominance: use the canonical repair scaffold first.",
                        "canonical_repair_next": "EXPERIMENT_CHARTER current :: hypothesis: λ4 decay ...",
                    },
                },
                "source_status": {"pid": 123, "reload_required": False, "source_changed_since_start": False},
                "eigen_geometry_rearrangement_v1": {
                    "available": True,
                    "classification": "rearrangement_preserving_density",
                    "density_preserved": True,
                    "entropy": 0.91,
                    "entropy_delta": 0.02,
                    "effective_modes": 4.4,
                    "effective_modes_delta": 0.3,
                    "relationship_shift_score": 0.22,
                    "lambda1_share_delta": -0.04,
                    "shoulder_share_delta": 0.05,
                    "tail_share_delta": -0.01,
                    "geom_rel": 1.02,
                    "rearrangement_intensity": 0.38,
                    "fill_pct": 68.5,
                    "fill_center_offset_pct": 0.5,
                    "falsification_flags": [],
                },
                "constraint_counterfactual_v1": {
                    "available": True,
                    "classification": "constraint_drivers_visible",
                    "confidence": 0.72,
                    "warning": "Unshaped baseline is a read-only counterfactual estimate; no constraints were removed.",
                    "top_shaping_drivers": [
                        {"driver": "scaffold_drain_relaxed", "score": 0.8, "confidence": 0.78},
                    ],
                    "falsification_flags": [],
                },
                "latest_decompose_snapshot_v1": {
                    "available": True,
                    "recorded_at": "2026-05-18T00:00:00Z",
                    "active_experiment_id": "exp_minime_gap",
                    "active_experiment_classification": "needs_evidence",
                    "temporal_decompose_v1": {
                        "classification": "opening_distribution",
                        "share_motion": 0.12,
                        "entropy_delta": 0.03,
                        "effective_modes_delta": 0.4,
                    },
                    "hypothesis_check_v1": {
                        "status": "checked",
                        "evidence_label": "supporting",
                        "suggested_next": "EXPERIMENT_EVIDENCE current :: spectral_condition ...",
                        "supporting_signals": ["λ1 share softened"],
                        "counter_signals": [],
                    },
                },
                "journal_excerpts": [],
            },
            "astrid_memory_reasoning_v1": {
                "continuity": {"classification": "needs_charter", "stale_running_count": 0},
                "constraint_counterfactual_cue_v1": {
                    "cue": "Constraint counterfactual cue: route absence-of-structure language into charter."
                },
                "journal_excerpts": [],
            },
            "actionable_signals": [],
        }
        rendered = render_markdown(report)
        self.assertIn("# Memory Reasoning Audit", rendered)
        self.assertIn("Attractor suggestions: `1`", rendered)
        self.assertIn("Confidence signal coverage", rendered)
        self.assertIn("Eigenvalue-Geometry", rendered)
        self.assertIn("rearrangement_preserving_density", rendered)
        self.assertIn("Temporal DECOMPOSE", rendered)
        self.assertIn("Constraint Counterfactual", rendered)
        self.assertIn("scaffold_drain_relaxed", rendered)
        self.assertIn("opening_distribution", rendered)
        self.assertIn("Charter quality dominance", rendered)
        self.assertIn("reload_required=`False`", rendered)
        self.assertLess(len(rendered), 5000)

    def test_actionable_signal_flags_geometry_falsification(self) -> None:
        report = {
            "minime_memory_reasoning_v1": {
                "attractor_suggestion_memory": {"confidence_signal_coverage_v1": {}},
                "btsp_agency": {},
                "continuity": {},
                "eigen_geometry_rearrangement_v1": {
                    "classification": "projection_like_loss",
                    "falsification_flags": ["entropy_collapse"],
                },
            },
            "continuity_state_v1": {"by_being": {}},
        }
        signals = actionable_signals(report)
        self.assertEqual(signals[0]["kind"], "eigen_geometry_rearrangement_falsified")

    def test_constraint_counterfactual_summary_renders_driver(self) -> None:
        payload = build_constraint_counterfactual_v1(
            [8.0, 3.0, 2.5, 1.0],
            fill_pct=71.0,
            target_fill_pct=68.0,
            stable_core={
                "structural_mode": "scaffold_hold_with_drain",
                "structural_pi": {"drain_weight": 0.08, "fill_slope_pct_per_sec": 2.0},
            },
            gate=0.12,
            filt=0.72,
            shadow={"lock_tendency": 0.6, "tail_openness": 0.2, "recurrence": 0.9},
            semantic={"semantic_energy": 0.001, "input_active": True},
            pressure_source={"components": {"mode_packing": 0.5}},
            focus="lambda-tail/lambda4",
        )
        self.assertTrue(payload["available"])
        self.assertFalse(payload["authority_change"])
        block = format_constraint_counterfactual_block(payload)
        self.assertIn("Constraint Counterfactual", block)
        self.assertIn("read-only counterfactual estimate", block)
        self.assertIn("scaffold_drain_relaxed", block)

    def test_pending_missing_signal_respects_source_status(self) -> None:
        base = {
            "minime_memory_reasoning_v1": {
                "attractor_suggestion_memory": {
                    "confidence_signal_coverage_v1": {
                        "missing": 1,
                        "pending_missing": 1,
                        "pending_missing_examples": [{"suggestion_id": "s-pending"}],
                    }
                },
                "btsp_agency": {},
                "continuity": {},
            },
            "continuity_state_v1": {"by_being": {}},
        }
        base["minime_memory_reasoning_v1"]["source_status"] = {"reload_required": True}
        self.assertEqual(actionable_signals(base)[0]["kind"], "attractor_confidence_runtime_pickup")
        base["minime_memory_reasoning_v1"]["source_status"] = {"reload_required": False}
        self.assertEqual(actionable_signals(base)[0]["kind"], "attractor_confidence_pending_legacy")


def run_self_tests() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(MemoryReasoningAuditTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON")
    parser.add_argument("--self-test", action="store_true", help="run built-in tests")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
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
