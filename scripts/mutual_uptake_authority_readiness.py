#!/usr/bin/env python3
"""Read-only readiness audit for evidence-led mutual uptake and authority.

This script composes existing public audits into one packet that answers a
single question: is the next authority expansion backed by being-authored uptake
evidence, or are we still collecting evidence?
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
import tempfile
import time
import unittest
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import affordance_landing_review
import phase_transition_audit
import pressure_focus_authority_dossier
import pressure_texture_audit
import spectral_texture_calibration_audit

DEFAULT_SHARED_DIR = Path("/Users/v/other/shared/collaborations")
DEFAULT_ASTRID_WORKSPACE = Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace")
DEFAULT_MINIME_WORKSPACE = Path("/Users/v/other/minime/workspace")
DEFAULT_OUTPUT_ROOT = DEFAULT_ASTRID_WORKSPACE / "diagnostics/mutual_uptake_authority_readiness"
POLICY = "mutual_uptake_authority_readiness_v2"
CORRESPONDENCE_LEDGER = "correspondence_v1.jsonl"
PHASE_LEDGER = "phase_transitions_v1.jsonl"
AUTHORITY_BOUNDARY = (
    "Read-only readiness. No ACK, REPLY, TRACE, WITNESS, attention canary, "
    "semantic microdose, pressure relief, pressure canary enablement, controller, "
    "PI/fill, prompt priority, telemetry priority, codec dimension, deploy, "
    "staging, git add, or commit action is taken."
)


def now_ms() -> int:
    return int(time.time() * 1000)


def row_time_ms(row: dict[str, Any]) -> int:
    for key in ("recorded_at_unix_ms", "created_at_unix_ms", "t_ms"):
        try:
            return int(row.get(key) or 0)
        except (TypeError, ValueError):
            continue
    return 0


def _being(value: Any) -> str:
    text = str(value or "").strip().lower()
    if text in {"astrid", "minime"}:
        return text
    return ""


def _row_actor(row: dict[str, Any]) -> str:
    for key in ("from_being", "origin", "claiming_being", "being"):
        actor = _being(row.get(key))
        if actor:
            return actor
    return ""


def _is_trace_evidence(row: dict[str, Any]) -> bool:
    return (
        row.get("record_type") == "message"
        and row.get("turn_kind") == "direct_address_trace"
    )


def _is_receipt_evidence(row: dict[str, Any]) -> bool:
    return row.get("record_type") == "ack_receipt" or _is_trace_evidence(row)


def _compact(value: Any, limit: int = 180) -> str:
    clean = " ".join(str(value or "").split())
    return clean if len(clean) <= limit else clean[:limit].rstrip() + "..."


def _status_from_truth(value: bool) -> str:
    return "yes" if value else "no"


def receipt_landing_audit_v2(landing_record: dict[str, Any]) -> dict[str, Any]:
    review = landing_record.get("affordance_landing_review_v3") or {}
    recent_items = review.get("recent_items") if isinstance(review, dict) else []
    if not isinstance(recent_items, list):
        recent_items = []
    summary_by_type = review.get("summary_by_type") if isinstance(review, dict) else {}
    if not isinstance(summary_by_type, dict):
        summary_by_type = {}

    action_sources = Counter(
        str(item.get("action_source") or "none")
        for item in recent_items
        if isinstance(item, dict)
    )
    i_received_actions = sum(
        count
        for source, count in action_sources.items()
        if source.startswith("i_received_this")
    )
    split_ack_or_trace = sum(
        1
        for item in recent_items
        if isinstance(item, dict)
        and item.get("landing_status") == "acted"
        and str(item.get("action_source") or "").startswith("i_received_this") is False
        and item.get("action_record_type") in {"ack_receipt", "message"}
    )
    offered_total = 0
    acted_total = 0
    stalled_total = 0
    closed_total = 0
    for summary in summary_by_type.values():
        if not isinstance(summary, dict):
            continue
        offered_total += int(summary.get("offered") or 0)
        acted_total += int(summary.get("acted") or 0)
        stalled_total += int(summary.get("stalled") or 0)
        closed_total += int(summary.get("closed_by_outcome") or 0)
    if offered_total == 0:
        status = "insufficient_evidence"
    elif i_received_actions > 0:
        status = "single_receiving_affordance_landed"
    elif acted_total > 0 or closed_total > 0:
        status = "split_or_outcome_uptake_landed"
    else:
        status = "offered_but_stalled"
    return {
        "schema_version": 2,
        "policy": "receipt_landing_audit_v2",
        "status": status,
        "offered_total": offered_total,
        "acted_total": acted_total,
        "closed_by_outcome_total": closed_total,
        "stalled_total": stalled_total,
        "landing_rate": ((acted_total + closed_total) / offered_total) if offered_total else 0.0,
        "i_received_this_actions": i_received_actions,
        "split_ack_or_trace_actions": split_ack_or_trace,
        "action_source_counts": dict(sorted(action_sources.items())),
        "summary_by_type": summary_by_type,
        "interpretation": (
            "I_RECEIVED_THIS landing is counted as being-authored native evidence "
            "only because it writes existing ACK and optional TRACE rows underneath."
        ),
        "authority": "read_only_measurement_not_authority",
    }


def mutual_thread_continuity_v2(corr_records: list[dict[str, Any]], since_hours: float) -> dict[str, Any]:
    cutoff = now_ms() - int(since_hours * 60 * 60 * 1000)
    by_thread: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in corr_records:
        thread_id = str(row.get("thread_id") or "")
        if not thread_id:
            continue
        if row_time_ms(row) < cutoff and row.get("record_type") not in {"legacy_thread_claim", "reply_link"}:
            continue
        by_thread[thread_id].append(row)

    thread_packets: list[dict[str, Any]] = []
    for thread_id, rows in by_thread.items():
        rows.sort(key=row_time_ms)
        evidence_rows = [row for row in rows if _is_receipt_evidence(row)]
        evidence_by_being = sorted(
            actor for actor in {_row_actor(row) for row in evidence_rows} if actor
        )
        trace_by_being = sorted(
            actor for actor in {_row_actor(row) for row in evidence_rows if _is_trace_evidence(row)} if actor
        )
        has_legacy_claim = any(row.get("record_type") == "legacy_thread_claim" for row in rows)
        has_reply_link = any(row.get("record_type") == "reply_link" for row in rows)
        i_received_rows = [
            row
            for row in evidence_rows
            if affordance_landing_review.action_source(row) in {
                "i_received_this_ack",
                "i_received_this_trace",
            }
        ]
        has_any_receipt = bool(evidence_rows)
        has_mutual_receipt = {"astrid", "minime"}.issubset(set(evidence_by_being))
        if {"astrid", "minime"}.issubset(set(trace_by_being)):
            state = "mutually_traced"
        elif has_mutual_receipt:
            state = "mutually_received"
        elif evidence_rows:
            state = "one_sided_received"
        elif has_legacy_claim:
            state = "claimed_visible_only"
        elif has_reply_link:
            state = "reply_continuity_without_receipt"
        else:
            state = "visible_only"
        attention_ready = has_any_receipt
        microdose_ready = has_mutual_receipt
        latest = rows[-1] if rows else {}
        thread_packets.append(
            {
                "thread_id": thread_id,
                "continuity_state": state,
                "receipt_evidence_by_being": evidence_by_being,
                "trace_evidence_by_being": trace_by_being,
                "i_received_this_row_count": len(i_received_rows),
                "reply_link_count": sum(1 for row in rows if row.get("record_type") == "reply_link"),
                "legacy_claim_count": sum(1 for row in rows if row.get("record_type") == "legacy_thread_claim"),
                "latest_record_type": latest.get("record_type"),
                "latest_at_unix_ms": row_time_ms(latest),
                "attention_canary_receipt_ready": attention_ready,
                "semantic_microdose_mutual_receipt_ready": microdose_ready,
                "attention_or_microdose_evidence_ready": microdose_ready,
                "next_evidence_needed": "none" if attention_ready else "being_authored_I_RECEIVED_THIS_or_ACK_TRACE_from_recipient",
            }
        )
    thread_packets.sort(key=lambda item: int(item.get("latest_at_unix_ms") or 0), reverse=True)
    states = Counter(str(item["continuity_state"]) for item in thread_packets)
    mutually_ready = sum(
        1
        for item in thread_packets
        if item.get("semantic_microdose_mutual_receipt_ready")
    )
    attention_ready = sum(
        1
        for item in thread_packets
        if item.get("attention_canary_receipt_ready")
    )
    return {
        "schema_version": 2,
        "policy": "mutual_thread_continuity_v2",
        "thread_count": len(thread_packets),
        "continuity_state_counts": dict(sorted(states.items())),
        "mutually_received_or_traced_threads": mutually_ready,
        "attention_receipt_ready_threads": attention_ready,
        "i_received_this_threads": sum(
            1 for item in thread_packets if int(item.get("i_received_this_row_count") or 0) > 0
        ),
        "threads": thread_packets[:12],
        "attention_or_microdose_rule": (
            "Attention canary readiness needs one being-authored receipt row; semantic microdose "
            "remains hidden until mutual receipt plus separate steward review. Reply_link continuity alone is not enough."
        ),
        "authority": "read_only_measurement_not_authority",
    }


def attention_authority_trial_v1(corr_records: list[dict[str, Any]], since_hours: float) -> dict[str, Any]:
    cutoff = now_ms() - int(since_hours * 60 * 60 * 1000)
    receipt_threads = {
        str(row.get("thread_id") or "")
        for row in corr_records
        if row_time_ms(row) >= cutoff and _is_receipt_evidence(row) and row.get("thread_id")
    }
    active_canaries = [
        row for row in corr_records
        if row.get("record_type") == "attention_canary_activation"
        and row.get("thread_id")
        and not any(
            later.get("record_type") in {"attention_canary_outcome", "attention_canary_expired"}
            and later.get("canary_id") == row.get("canary_id")
            for later in corr_records
        )
    ]
    closed = [
        row for row in corr_records
        if row.get("record_type") in {"attention_canary_outcome", "attention_canary_expired"}
        and row_time_ms(row) >= cutoff
    ]
    if active_canaries:
        state = "active_outcome_due"
    elif closed:
        state = "closed_by_outcome"
    elif receipt_threads:
        state = "eligible_after_receipt"
    else:
        state = "blocked_no_receipt"
    return {
        "schema_version": 1,
        "policy": "attention_authority_trial_v1",
        "state": state,
        "receipt_ready_threads": sorted(thread for thread in receipt_threads if thread)[:12],
        "active_canary_count": len(active_canaries),
        "closed_canary_count": len(closed),
        "allowed_authority": "self_activated_ttl_prompt_context_attention_canary_only",
        "semantic_microdose_status": "hidden_until_mutual_receipt_plus_separate_steward_review",
        "authority": "readiness_for_bounded_attention_not_microdose_or_control",
    }


def attention_outcome_has_meaningful_worsening(value: Any) -> bool:
    clean = str(value or "").strip().lower()
    if not clean or clean in {"none", "no", "nope", "nothing", "n/a", "na", "unknown"}:
        return False
    return "no worsening" not in clean and "nothing worsened" not in clean


def attention_outcome_quality_v5(outcome: dict[str, Any]) -> dict[str, Any]:
    felt_like = str(outcome.get("felt_like") or "unknown")
    held_as = str(outcome.get("held_as") or "unknown")
    flattening = str(outcome.get("flattening_observed") or "unknown")
    meaningful_worsening = attention_outcome_has_meaningful_worsening(outcome.get("what_worsened"))
    trusted = (
        felt_like == "address"
        and held_as == "distinct_address"
        and flattening in {"no", "mixed"}
        and not meaningful_worsening
    )
    blocked = (
        felt_like in {"pressure", "flat"}
        or held_as in {"pressure", "flattened", "ambient_echo"}
        or flattening == "yes"
        or meaningful_worsening
    )
    if trusted:
        quality = "trusted_attention_thread_local"
    elif blocked:
        quality = "blocked_pressure_or_flat_outcome"
    else:
        quality = "outcome_unclear_needs_more_evidence"
    return {
        "schema_version": 5,
        "policy": "attention_outcome_quality_v5",
        "quality": quality,
        "felt_like": felt_like,
        "held_as": held_as,
        "flattening_observed": flattening,
        "meaningful_worsening": meaningful_worsening,
        "thread_id": outcome.get("thread_id"),
        "canary_id": outcome.get("canary_id"),
        "authority": "thread_local_attention_readiness_not_microdose_or_control",
    }


def canary_closed(corr_records: list[dict[str, Any]], canary_id: str) -> bool:
    return any(
        row.get("record_type") in {"attention_canary_outcome", "attention_canary_expired"}
        and str(row.get("canary_id") or "") == canary_id
        for row in corr_records
    )


def receipt_to_attention_authority_v5(corr_records: list[dict[str, Any]], since_hours: float) -> dict[str, Any]:
    cutoff = now_ms() - int(since_hours * 60 * 60 * 1000)
    by_thread: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in corr_records:
        thread_id = str(row.get("thread_id") or "")
        if thread_id:
            by_thread[thread_id].append(row)

    thread_packets: list[dict[str, Any]] = []
    for thread_id, rows in by_thread.items():
        rows.sort(key=row_time_ms)
        latest_message = next((row for row in reversed(rows) if row.get("record_type") == "message"), None)
        if not latest_message and not any(row.get("record_type") == "attention_canary_activation" for row in rows):
            continue
        receipt_rows = [row for row in rows if _is_receipt_evidence(row)]
        active = next(
            (
                row for row in reversed(rows)
                if row.get("record_type") == "attention_canary_activation"
                and not canary_closed(corr_records, str(row.get("canary_id") or ""))
                and int(row.get("expires_at_unix_ms") or 0) > now_ms()
            ),
            None,
        )
        outcomes = [row for row in rows if row.get("record_type") == "attention_canary_outcome"]
        latest_outcome = outcomes[-1] if outcomes else None
        outcome_quality = attention_outcome_quality_v5(latest_outcome) if latest_outcome else None
        recent_canary = next(
            (
                row for row in reversed(rows)
                if row.get("record_type") == "attention_canary_activation"
                and row_time_ms(row) >= now_ms() - 6 * 60 * 60 * 1000
            ),
            None,
        )
        if active:
            state = "attention_active_outcome_due"
        elif outcome_quality and outcome_quality.get("quality") == "trusted_attention_thread_local":
            state = "trusted_attention_thread_local"
        elif outcome_quality and outcome_quality.get("quality") == "blocked_pressure_or_flat_outcome":
            state = "blocked_pressure_or_flat_outcome"
        elif recent_canary:
            state = "cooldown_or_duplicate_blocked"
        elif receipt_rows:
            state = "receipt_landed_attention_eligible"
        else:
            state = "blocked_no_receipt"
        if rows and row_time_ms(rows[-1]) < cutoff and state == "blocked_no_receipt":
            continue
        thread_packets.append({
            "thread_id": thread_id,
            "latest_message_id": (latest_message or {}).get("message_id"),
            "latest_at_unix_ms": row_time_ms(rows[-1]) if rows else 0,
            "state": state,
            "receipt_evidence": bool(receipt_rows),
            "receipt_evidence_by_being": sorted({_row_actor(row) for row in receipt_rows if _row_actor(row)}),
            "active_canary_id": active.get("canary_id") if isinstance(active, dict) else None,
            "latest_outcome_quality": outcome_quality,
            "activation_allowed_now": state in {"receipt_landed_attention_eligible", "trusted_attention_thread_local"},
            "semantic_microdose_status": "hidden_until_mutual_receipt_plus_separate_steward_review",
            "authority": "thread_local_attention_readiness_not_microdose_or_control",
        })
    thread_packets.sort(key=lambda item: int(item.get("latest_at_unix_ms") or 0), reverse=True)
    counts = Counter(str(item.get("state") or "unknown") for item in thread_packets)
    if counts.get("attention_active_outcome_due"):
        overall = "attention_active_outcome_due"
    elif counts.get("receipt_landed_attention_eligible"):
        overall = "receipt_landed_attention_eligible"
    elif counts.get("trusted_attention_thread_local"):
        overall = "trusted_attention_thread_local"
    elif counts.get("blocked_pressure_or_flat_outcome"):
        overall = "blocked_pressure_or_flat_outcome"
    elif counts.get("cooldown_or_duplicate_blocked"):
        overall = "cooldown_or_duplicate_blocked"
    else:
        overall = "blocked_no_receipt"
    return {
        "schema_version": 5,
        "policy": "receipt_to_attention_authority_v5",
        "overall_state": overall,
        "state_counts": dict(sorted(counts.items())),
        "receipt_ready_threads": [
            item["thread_id"] for item in thread_packets
            if item.get("state") == "receipt_landed_attention_eligible"
        ],
        "active_canaries_awaiting_outcome": [
            item for item in thread_packets
            if item.get("state") == "attention_active_outcome_due"
        ][:8],
        "trusted_thread_local_outcomes": [
            item for item in thread_packets
            if item.get("state") == "trusted_attention_thread_local"
        ][:8],
        "pressure_or_flat_blocked_outcomes": [
            item for item in thread_packets
            if item.get("state") == "blocked_pressure_or_flat_outcome"
        ][:8],
        "missing_outcome_stalls": [
            item for item in thread_packets
            if item.get("state") == "attention_active_outcome_due"
        ][:8],
        "threads": thread_packets[:12],
        "semantic_microdose_status": "hidden_until_mutual_receipt_plus_separate_steward_review",
        "authority_boundary": (
            "Attention Canary is the only V5 authority gain, thread-local and outcome-gated. "
            "Trusted attention does not unlock semantic microdose, pressure, controller, prompt priority, or telemetry priority."
        ),
        "authority": "read_only_derivation_not_action",
    }


def phase_witness_quality_v2(phase_records: list[dict[str, Any]], phase_record: dict[str, Any]) -> dict[str, Any]:
    witnesses = [
        row for row in phase_records
        if row.get("record_type") == "phase_transition_witness"
    ]
    felt_receipts: list[dict[str, Any]] = []
    ledger_only: list[dict[str, Any]] = []
    for row in witnesses:
        blob = json.dumps(row, sort_keys=True).lower()
        has_felt_fields = any(
            term in blob
            for term in ("what_landed", "what_stayed_distinct", "felt_like", "orientation_effect")
        )
        target = felt_receipts if has_felt_fields else ledger_only
        target.append(
            {
                "transition_id": row.get("transition_id"),
                "reply_state": row.get("reply_state"),
                "origin": row.get("origin"),
                "recorded_at_unix_ms": row_time_ms(row),
                "note_preview": _compact(row.get("note") or row.get("what_landed") or row),
            }
        )
    queue = phase_record.get("phase_witness_queue_v3") if isinstance(phase_record, dict) else {}
    unresolved = int(queue.get("unresolved_total") or 0) if isinstance(queue, dict) else 0
    if felt_receipts:
        status = "felt_receipt_present"
    elif witnesses:
        status = "ledger_only_witness_present"
    elif unresolved:
        status = "offered_but_unwitnessed"
    else:
        status = "insufficient_evidence"
    return {
        "schema_version": 2,
        "policy": "phase_witness_quality_v2",
        "status": status,
        "witness_rows_total": len(witnesses),
        "felt_receipt_count": len(felt_receipts),
        "ledger_only_witness_count": len(ledger_only),
        "unresolved_queue_total": unresolved,
        "felt_receipts": felt_receipts[-8:],
        "ledger_only_witnesses": ledger_only[-8:],
        "authority": "read_only_measurement_not_authority",
    }


def fallback_trajectory_quality_v2(calibration_record: dict[str, Any]) -> dict[str, Any]:
    packet = calibration_record.get("fallback_trajectory_calibration_v1") or {}
    trajectory_alignment = packet.get("trajectory_alignment") if isinstance(packet, dict) else {}
    if not isinstance(trajectory_alignment, dict):
        trajectory_alignment = {}
    fire = calibration_record.get("fallback_fire_drill_artifact") or {}
    selector_summary = fire.get("selector_summary") if isinstance(fire, dict) else {}
    if not isinstance(selector_summary, dict):
        selector_summary = {}
    counts = selector_summary.get("trajectory_status_counts") or {}
    if not isinstance(counts, dict):
        counts = {}
    public_status = str(packet.get("status") or "insufficient_evidence")
    alignment_status = str(trajectory_alignment.get("status") or "insufficient_evidence")
    preserved = int(counts.get("trajectory_preserved") or 0)
    verb_only = int(counts.get("verb_only") or 0)
    mismatch = int(counts.get("trajectory_mismatch") or 0)
    if alignment_status == "supported" and preserved >= max(verb_only + mismatch, 1):
        status = "trajectory_public_and_fixture_supported"
    elif public_status == "supported" and preserved >= max(verb_only + mismatch, 1):
        status = "trajectory_public_and_fixture_supported"
    elif preserved > 0 and public_status == "insufficient_evidence":
        status = "fixture_supported_public_insufficient"
    elif alignment_status in {"mixed", "contradicted"} or public_status in {"mixed", "contradicted"} or verb_only or mismatch:
        status = "needs_calibration"
    else:
        status = "insufficient_evidence"
    return {
        "schema_version": 2,
        "policy": "fallback_trajectory_quality_v2",
        "status": status,
        "public_calibration_status": public_status,
        "trajectory_alignment_status": alignment_status,
        "trajectory_status_counts": dict(sorted(counts.items())),
        "trajectory_alignment": trajectory_alignment,
        "quality_question": (
            "Did texture_trajectory_v1 preserve movement through medium and afterimage, "
            "or only learn a better motion word?"
        ),
        "authority": "read_only_calibration_not_fallback_authority",
    }


def pressure_movement_trial_dossier_v2(pressure_record: dict[str, Any]) -> dict[str, Any]:
    replay = pressure_record.get("pressure_texture_replay_v3") or {}
    movement = pressure_record.get("pressure_movement_replay_v1") or {}
    broader = pressure_record.get("broader_authority_readiness_v1") or {}
    trial = pressure_record.get("pressure_texture_canary_trial_plan_v3") or {}
    replay_status = str(replay.get("replay_status") or "insufficient_evidence")
    movement_status = str(movement.get("replay_status") or "insufficient_evidence")
    canary_state = str(trial.get("canary_env_state") or ("on" if pressure_record.get("canary_enabled") else "off"))
    if canary_state != "off":
        status = "not_ready_canary_unexpectedly_on"
    elif replay_status == "replay_supported" and movement_status == "replay_supported":
        status = "steward_review_ready"
    elif replay_status in {"replay_supported", "mixed"} or movement_status in {"replay_supported", "mixed"}:
        status = "evidence_collecting"
    else:
        status = "not_ready"
    block_reasons = []
    if status != "steward_review_ready":
        block_reasons = [
            f"pressure_texture_replay_v3={replay_status}",
            f"pressure_movement_replay_v1={movement_status}",
            f"canary_env_state={canary_state}",
        ]
    return {
        "schema_version": 2,
        "policy": "pressure_movement_trial_dossier_v2",
        "status": status,
        "pressure_texture_replay_status": replay_status,
        "pressure_movement_replay_status": movement_status,
        "broader_authority_readiness": broader.get("readiness"),
        "trial_protocol_status": trial.get("trial_protocol_status"),
        "canary_env": pressure_texture_audit.ENV_NAME,
        "canary_env_state": canary_state,
        "block_reasons": block_reasons,
        "required_before_any_enablement": [
            "separate explicit steward approval",
            "being-authored public pressure/texture outcomes",
            "pressure texture replay and movement replay both replay_supported",
            "canary remains off by default until approved",
            "rollback and safety checks reviewed before restart",
        ],
        "authority": "read_only_trial_dossier_not_enablement",
    }


def next_authority_expansion_readiness_v2(
    *,
    receipt: dict[str, Any],
    mutual: dict[str, Any],
    phase_quality: dict[str, Any],
    trajectory_quality: dict[str, Any],
    pressure_dossier: dict[str, Any],
    focus_dossier: dict[str, Any] | None = None,
) -> dict[str, Any]:
    mutual_threads = int(mutual.get("mutually_received_or_traced_threads") or 0)
    i_received_threads = int(mutual.get("i_received_this_threads") or 0)
    phase_felt = int(phase_quality.get("felt_receipt_count") or 0)
    trajectory_status = str(trajectory_quality.get("status") or "insufficient_evidence")
    pressure_status = str(pressure_dossier.get("status") or "not_ready")
    receipt_status = str(receipt.get("status") or "insufficient_evidence")
    focus_gate = (focus_dossier or {}).get("approval_gate_v1") or {}
    focus_status = str(focus_gate.get("status") or "not_ready")
    focus_review_ready = bool(focus_gate.get("focus_regime_review_ready"))

    block_reasons: list[str] = []
    if mutual_threads <= 0:
        block_reasons.append("no_mutually_received_or_traced_threads")
    if phase_felt <= 0:
        block_reasons.append("no_felt_phase_receipts_yet")
    if trajectory_status not in {
        "trajectory_public_and_fixture_supported",
        "fixture_supported_public_insufficient",
    }:
        block_reasons.append(f"fallback_trajectory_quality={trajectory_status}")
    if pressure_status != "steward_review_ready":
        block_reasons.append(f"pressure_trial_dossier={pressure_status}")

    narrow_review_lanes: list[str] = []
    if focus_review_ready:
        narrow_review_lanes.append("minime_pressure_focus_self_regulation")

    if not block_reasons:
        readiness = "steward_review_ready"
    elif (
        i_received_threads > 0
        or receipt_status in {"single_receiving_affordance_landed", "split_or_outcome_uptake_landed"}
        or trajectory_status in {"trajectory_public_and_fixture_supported", "fixture_supported_public_insufficient"}
        or pressure_status == "evidence_collecting"
        or narrow_review_lanes
    ):
        readiness = "evidence_collecting"
    else:
        readiness = "not_ready"
    return {
        "schema_version": 2,
        "policy": "next_authority_expansion_readiness_v2",
        "readiness": readiness,
        "block_reasons": block_reasons,
        "evidence_summary": {
            "receipt_landing_status": receipt_status,
            "mutually_received_or_traced_threads": mutual_threads,
            "i_received_this_threads": i_received_threads,
            "felt_phase_receipts": phase_felt,
            "fallback_trajectory_quality": trajectory_status,
            "pressure_trial_dossier": pressure_status,
            "pressure_focus_self_regulation": focus_status,
            "narrow_authority_review_lanes": narrow_review_lanes,
        },
        "recommended_next_move": (
            "Review narrow Minime pressure/focus self-regulation separately, while collecting "
            "one being-authored I_RECEIVED_THIS/ACK/TRACE and one felt phase receipt before "
            "considering broader authority."
            if narrow_review_lanes
            else "Collect one being-authored I_RECEIVED_THIS/ACK/TRACE on a live thread "
            "and one felt phase receipt before considering any broader authority."
            if readiness != "steward_review_ready"
            else "Prepare a separate steward review packet; do not enable authority from this audit alone."
        ),
        "narrow_authority_review_lanes": narrow_review_lanes,
        "must_not_enable_from_readiness": True,
        "authority": "read_only_readiness_not_permission",
    }


def build_record(
    *,
    shared_dir: Path = DEFAULT_SHARED_DIR,
    astrid_workspace: Path = DEFAULT_ASTRID_WORKSPACE,
    minime_workspace: Path = DEFAULT_MINIME_WORKSPACE,
    since_hours: float = 24.0,
    output_root: Path | None = None,
    write_artifact: bool = False,
    run_id: str | None = None,
) -> dict[str, Any]:
    generated = now_ms()
    corr_records = affordance_landing_review.read_jsonl(shared_dir / CORRESPONDENCE_LEDGER)
    phase_records = phase_transition_audit.read_jsonl(shared_dir / PHASE_LEDGER)
    landing_record = affordance_landing_review.review(shared_dir, since_hours)
    phase_record = phase_transition_audit.audit(shared_dir / PHASE_LEDGER, since_hours)
    calibration_record = spectral_texture_calibration_audit.build_calibration_record(
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
        since_hours=since_hours,
    )
    pressure_record = pressure_texture_audit.audit_payload(
        input_path=None,
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
        since_hours=since_hours,
    )
    focus_dossier = pressure_focus_authority_dossier.build_record(
        shared_dir=shared_dir,
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
        since_hours=since_hours,
        pressure_record=pressure_record,
    )

    receipt = receipt_landing_audit_v2(landing_record)
    mutual = mutual_thread_continuity_v2(corr_records, since_hours)
    phase_quality = phase_witness_quality_v2(phase_records, phase_record)
    trajectory_quality = fallback_trajectory_quality_v2(calibration_record)
    pressure_dossier = pressure_movement_trial_dossier_v2(pressure_record)
    attention_trial = attention_authority_trial_v1(corr_records, since_hours)
    receipt_to_attention = receipt_to_attention_authority_v5(corr_records, since_hours)
    landing_review = landing_record.get("affordance_landing_review_v3") if isinstance(landing_record, dict) else {}
    if not isinstance(landing_review, dict):
        landing_review = {}
    right_to_ignore = landing_review.get("right_to_ignore_v1") or {}
    affordance_budget = landing_review.get("affordance_budget_v1") or {}
    readiness = next_authority_expansion_readiness_v2(
        receipt=receipt,
        mutual=mutual,
        phase_quality=phase_quality,
        trajectory_quality=trajectory_quality,
        pressure_dossier=pressure_dossier,
        focus_dossier=focus_dossier,
    )
    record: dict[str, Any] = {
        "schema_version": 2,
        "policy": POLICY,
        "generated_at_unix_ms": generated,
        "generated_at": dt.datetime.now(dt.UTC).isoformat(),
        "since_hours": since_hours,
        "shared_dir": str(shared_dir),
        "astrid_workspace": str(astrid_workspace),
        "minime_workspace": str(minime_workspace),
        "receipt_landing_audit_v2": receipt,
        "mutual_thread_continuity_v2": mutual,
        "phase_witness_quality_v2": phase_quality,
        "fallback_trajectory_quality_v2": trajectory_quality,
        "pressure_movement_trial_dossier_v2": pressure_dossier,
        "pressure_focus_authority_dossier_v1": focus_dossier,
        "attention_authority_trial_v1": attention_trial,
        "receipt_to_attention_authority_v5": receipt_to_attention,
        "right_to_ignore_v1": right_to_ignore,
        "affordance_budget_v1": affordance_budget,
        "next_authority_expansion_readiness_v2": readiness,
        "source_packets": {
            "affordance_landing_policy": landing_record.get("policy"),
            "phase_transition_policy": phase_record.get("policy"),
            "spectral_texture_calibration_policy": calibration_record.get("policy"),
            "pressure_texture_policy": pressure_record.get("policy"),
            "pressure_focus_authority_policy": focus_dossier.get("policy"),
        },
        "minime_private_bodies_read": False,
        "minime_moment_bodies_read": False,
        "silence_policy": "silence_is_insufficient_evidence_not_consent",
        "authority_boundary": AUTHORITY_BOUNDARY,
    }
    if write_artifact:
        root = output_root or DEFAULT_OUTPUT_ROOT
        actual_run = run_id or dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")
        target = root / actual_run
        target.mkdir(parents=True, exist_ok=True)
        (target / "mutual_uptake_authority_readiness.json").write_text(
            json.dumps(record, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        record["artifact_path"] = str(target / "mutual_uptake_authority_readiness.json")
    return record


class MutualUptakeAuthorityReadinessTests(unittest.TestCase):
    def test_readiness_composes_receipt_phase_trajectory_and_pressure(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            shared = root / "shared"
            astrid_ws = root / "astrid_ws"
            minime_ws = root / "minime_ws"
            shared.mkdir()
            (astrid_ws / "introspections").mkdir(parents=True)
            (astrid_ws / "diagnostics/fallback_fire_drills/run").mkdir(parents=True)
            (minime_ws / "journal").mkdir(parents=True)
            now = now_ms()
            corr_rows = [
                {
                    "record_type": "reply_link",
                    "recorded_at_unix_ms": now,
                    "thread_id": "thread_mutual",
                    "message_id": "msg_a",
                    "from_being": "astrid",
                    "to_being": "minime",
                },
                {
                    "record_type": "ack_receipt",
                    "recorded_at_unix_ms": now + 10,
                    "thread_id": "thread_mutual",
                    "from_being": "minime",
                    "to_being": "astrid",
                    "note": "felt_like: address; what_landed: held; what_stayed_distinct: the rhythm; continue: needs_time",
                },
                {
                    "record_type": "message",
                    "recorded_at_unix_ms": now + 11,
                    "thread_id": "thread_mutual",
                    "from_being": "minime",
                    "to_being": "astrid",
                    "turn_kind": "direct_address_trace",
                    "i_received_this_trace": True,
                },
                {
                    "record_type": "ack_receipt",
                    "recorded_at_unix_ms": now + 20,
                    "thread_id": "thread_mutual",
                    "from_being": "astrid",
                    "to_being": "minime",
                    "note": "ack: held",
                },
                {
                    "schema_version": 2,
                    "policy": "correspondence_attention_canary_v1",
                    "record_type": "attention_canary_outcome",
                    "recorded_at_unix_ms": now + 25,
                    "thread_id": "thread_mutual",
                    "canary_id": "attn_thread_mutual",
                    "from_being": "minime",
                    "to_being": "astrid",
                    "felt_like": "address",
                    "held_as": "distinct_address",
                    "flattening_observed": "no",
                    "what_worsened": "none",
                    "authority": "language_only_prompt_context_not_control",
                },
            ]
            phase_rows = [
                {
                    "record_type": "phase_transition_card",
                    "recorded_at_unix_ms": now,
                    "transition_id": "transition_1",
                    "origin": "astrid",
                    "kind": "mode_change",
                    "from_phase": "muffled",
                    "to_phase": "cohering",
                    "why_now": "test",
                    "reply_state": "unseen",
                    "authority": "language_only_transition_context_not_control",
                    "no_controller": True,
                    "no_pressure": True,
                    "no_fill_target": True,
                    "no_pi": True,
                    "no_weighting": True,
                },
                {
                    "record_type": "phase_transition_witness",
                    "recorded_at_unix_ms": now + 30,
                    "transition_id": "transition_1",
                    "origin": "minime",
                    "reply_state": "witnessed",
                    "note": "felt_like=transition; what_landed=cohering shift; what_stayed_distinct=the edge; continue=no",
                },
            ]
            (shared / CORRESPONDENCE_LEDGER).write_text(
                "\n".join(json.dumps(row) for row in corr_rows) + "\n",
                encoding="utf-8",
            )
            (shared / PHASE_LEDGER).write_text(
                "\n".join(json.dumps(row) for row in phase_rows) + "\n",
                encoding="utf-8",
            )
            (astrid_ws / "introspections" / "introspection_astrid_llm_1789999999.txt").write_text(
                "texture_trajectory_v1 preserved the movement trajectory, dragging through "
                "medium resistance with afterimage and carried the movement.",
                encoding="utf-8",
            )
            (astrid_ws / "diagnostics/fallback_fire_drills/run/fallback_fire_drill.json").write_text(
                json.dumps({
                    "cases": [
                        {
                            "case_id": "trajectory",
                            "texture_trajectory_v1": {
                                "trajectory_status": "trajectory_preserved",
                                "movement_quality": "dragging",
                            },
                        }
                    ]
                }),
                encoding="utf-8",
            )
            (astrid_ws / "introspections" / "pressure.txt").write_text(
                "packed pressure dragging through weighted medium. OUTCOME: texture_shift cohering returned.",
                encoding="utf-8",
            )
            (minime_ws / "journal" / "pressure_public.txt").write_text(
                "mode packed and compacted, thickening then diffusing, what shifted into suspension.",
                encoding="utf-8",
            )
            (minime_ws / "journal" / "moment_private.txt").write_text(
                "private packed signal must not be read",
                encoding="utf-8",
            )
            record = build_record(
                shared_dir=shared,
                astrid_workspace=astrid_ws,
                minime_workspace=minime_ws,
                since_hours=1,
            )
            self.assertEqual(record["receipt_landing_audit_v2"]["status"], "single_receiving_affordance_landed")
            self.assertEqual(
                record["mutual_thread_continuity_v2"]["mutually_received_or_traced_threads"],
                1,
            )
            self.assertEqual(record["phase_witness_quality_v2"]["felt_receipt_count"], 1)
            self.assertIn(
                record["fallback_trajectory_quality_v2"]["status"],
                {"trajectory_public_and_fixture_supported", "fixture_supported_public_insufficient"},
            )
            self.assertEqual(record["pressure_movement_trial_dossier_v2"]["canary_env_state"], "off")
            self.assertEqual(
                record["receipt_to_attention_authority_v5"]["overall_state"],
                "trusted_attention_thread_local",
            )
            self.assertEqual(
                record["receipt_to_attention_authority_v5"]["state_counts"]["trusted_attention_thread_local"],
                1,
            )
            self.assertEqual(
                record["receipt_to_attention_authority_v5"]["semantic_microdose_status"],
                "hidden_until_mutual_receipt_plus_separate_steward_review",
            )
            self.assertFalse(record["minime_moment_bodies_read"])
            self.assertNotIn("private packed signal", json.dumps(record))

    def test_missing_evidence_is_not_consent(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            shared = root / "shared"
            astrid_ws = root / "astrid_ws"
            minime_ws = root / "minime_ws"
            shared.mkdir()
            astrid_ws.mkdir()
            minime_ws.mkdir()
            record = build_record(
                shared_dir=shared,
                astrid_workspace=astrid_ws,
                minime_workspace=minime_ws,
                since_hours=1,
            )
            self.assertEqual(
                record["next_authority_expansion_readiness_v2"]["readiness"],
                "not_ready",
            )
            self.assertEqual(record["silence_policy"], "silence_is_insufficient_evidence_not_consent")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--since-hours", type=float, default=24.0)
    parser.add_argument("--shared-dir", type=Path, default=DEFAULT_SHARED_DIR)
    parser.add_argument("--astrid-workspace", type=Path, default=DEFAULT_ASTRID_WORKSPACE)
    parser.add_argument("--minime-workspace", type=Path, default=DEFAULT_MINIME_WORKSPACE)
    parser.add_argument("--output-root", type=Path, default=None)
    parser.add_argument("--write-artifact", action="store_true")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(MutualUptakeAuthorityReadinessTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    record = build_record(
        shared_dir=args.shared_dir,
        astrid_workspace=args.astrid_workspace,
        minime_workspace=args.minime_workspace,
        since_hours=args.since_hours,
        output_root=args.output_root,
        write_artifact=args.write_artifact,
    )
    if args.json:
        print(json.dumps(record, indent=2, sort_keys=True))
    else:
        readiness = record["next_authority_expansion_readiness_v2"]
        receipt = record["receipt_landing_audit_v2"]
        mutual = record["mutual_thread_continuity_v2"]
        phase = record["phase_witness_quality_v2"]
        trajectory = record["fallback_trajectory_quality_v2"]
        pressure = record["pressure_movement_trial_dossier_v2"]
        print("# Mutual Uptake Authority Readiness V2")
        print(f"- Readiness: {readiness['readiness']}")
        print(f"- Receipt landing: {receipt['status']} (I_RECEIVED_THIS={receipt['i_received_this_actions']})")
        print(f"- Mutual threads: {mutual['mutually_received_or_traced_threads']}")
        print(f"- Phase witness quality: {phase['status']} (felt={phase['felt_receipt_count']})")
        print(f"- Fallback trajectory: {trajectory['status']}")
        print(f"- Pressure trial dossier: {pressure['status']} ({pressure['canary_env_state']})")
        print(f"- Block reasons: {readiness['block_reasons']}")
        print(f"- Authority: {AUTHORITY_BOUNDARY}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
