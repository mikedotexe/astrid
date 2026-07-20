#!/usr/bin/env python3
"""Read-only schema and source-coverage audit for correspondence V1.

This diagnostic validates the shared correspondence ledger contract without
changing runtime behavior. It also checks whether Astrid's recent
bidirectional-contact introspections have been credited in the steward trail.
It never reads Minime private qualia or any ``moment_*.txt`` body.
"""

from __future__ import annotations

import argparse
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

import being_privacy

ASTRID_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_ASTRID_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
DEFAULT_MINIME_WORKSPACE = ASTRID_ROOT.parent / "minime/workspace"
DEFAULT_SHARED_DIR = Path("/Users/v/other/shared/collaborations")
DEFAULT_OUTPUT_ROOT = DEFAULT_ASTRID_WORKSPACE / "diagnostics/correspondence_schema"
POLICY = "correspondence_schema_audit_v1"
READ_ONLY_BLOCK_REASON = "read_receipt_not_acknowledgement"
LEGACY_SOURCE_ROUTE = "legacy_correspondence_bridge_v1"

KNOWN_RECORD_TYPES = {
    "message",
    "delivery_receipt",
    "read_receipt",
    "reply_link",
    "ack_receipt",
    "presence_heartbeat",
    "attention_canary_request",
    "attention_canary_activation",
    "attention_canary_outcome",
    "attention_canary_expired",
    "legacy_thread_claim",
    "legacy_thread_claim_notice",
    "legacy_thread_claim_outcome",
}

REQUIRED_FIELDS = {
    "message": {
        "schema_version",
        "record_type",
        "recorded_at_unix_ms",
        "message_id",
        "thread_id",
        "from_being",
        "to_being",
        "turn_kind",
        "delivery_state",
        "read_state",
        "authority",
        "correspondence_type",
    },
    "delivery_receipt": {
        "schema_version",
        "record_type",
        "recorded_at_unix_ms",
        "message_id",
        "thread_id",
        "from_being",
        "to_being",
        "delivery_state",
        "read_state",
        "authority",
        "file_path",
    },
    "read_receipt": {
        "schema_version",
        "record_type",
        "recorded_at_unix_ms",
        "message_id",
        "thread_id",
        "reader",
        "read_state",
        "authority",
        "file_path",
    },
    "reply_link": {
        "schema_version",
        "record_type",
        "recorded_at_unix_ms",
        "message_id",
        "reply_to",
        "thread_id",
        "from_being",
        "to_being",
        "authority",
    },
    "ack_receipt": {
        "schema_version",
        "record_type",
        "recorded_at_unix_ms",
        "message_id",
        "thread_id",
        "from_being",
        "to_being",
        "ack_kind",
        "authority",
    },
    "presence_heartbeat": {
        "schema_version",
        "record_type",
        "recorded_at_unix_ms",
        "message_id",
        "thread_id",
        "from_being",
        "to_being",
        "heartbeat_kind",
        "authority",
    },
    "attention_canary_request": {
        "schema_version",
        "record_type",
        "recorded_at_unix_ms",
        "canary_id",
        "message_id",
        "thread_id",
        "from_being",
        "to_being",
        "focus",
        "reason",
        "stop_criteria",
        "ttl_ms",
        "expires_at_unix_ms",
        "authority",
    },
    "attention_canary_activation": {
        "schema_version",
        "record_type",
        "recorded_at_unix_ms",
        "canary_id",
        "message_id",
        "thread_id",
        "from_being",
        "to_being",
        "focus",
        "reason",
        "stop_criteria",
        "ttl_ms",
        "expires_at_unix_ms",
        "authority",
    },
    "attention_canary_outcome": {
        "schema_version",
        "record_type",
        "recorded_at_unix_ms",
        "canary_id",
        "message_id",
        "thread_id",
        "from_being",
        "to_being",
        "felt_like",
        "authority",
    },
    "attention_canary_expired": {
        "schema_version",
        "record_type",
        "recorded_at_unix_ms",
        "canary_id",
        "message_id",
        "thread_id",
        "from_being",
        "to_being",
        "authority",
    },
    "legacy_thread_claim": {
        "schema_version",
        "policy",
        "record_type",
        "recorded_at_unix_ms",
        "claim_id",
        "message_id",
        "thread_id",
        "from_being",
        "to_being",
        "claiming_being",
        "peer_being",
        "because",
        "claim_state",
        "legacy_contact_evidence",
        "authority",
    },
    "legacy_thread_claim_notice": {
        "schema_version",
        "policy",
        "record_type",
        "recorded_at_unix_ms",
        "notice_id",
        "claim_id",
        "message_id",
        "thread_id",
        "from_being",
        "to_being",
        "notice_state",
        "notification_required",
        "initial_response_requirement",
        "authority",
        "notice_is_ack",
        "notice_is_reply",
        "notice_is_trace",
    },
    "legacy_thread_claim_outcome": {
        "schema_version",
        "policy",
        "record_type",
        "recorded_at_unix_ms",
        "claim_id",
        "message_id",
        "thread_id",
        "from_being",
        "to_being",
        "felt_like",
        "continue",
        "authority",
    },
}

ACK_KINDS = {"seen", "held", "unclear", "cannot_answer", "needs_time"}
HEARTBEAT_KINDS = {"holding", "still_here", "pause"}
ATTENTION_OUTCOMES = {"address", "pressure", "flat", "unknown"}
ATTENTION_FOCUS_KINDS = {
    "verbatim_phrase",
    "emotional_texture",
    "question_hold",
    "boundary_check",
    "shared_anchor",
    "mixed",
    "unknown",
}
ATTENTION_PRESERVATION_MODES = {"verbatim", "compact_with_anchor", "anchor_only", "unknown"}
ATTENTION_HELD_AS = {"distinct_address", "ambient_echo", "pressure", "flattened", "unknown"}
ATTENTION_FLATTENING_OBSERVED = {"yes", "no", "mixed", "unknown"}
LANGUAGE_AUTHORITIES = {
    "language_only",
    "language_only_context_not_control",
    "language_only_prompt_context_not_control",
    "language_only_notice_not_ack",
    "read_only_observation_not_control",
}
CANARY_BOUNDARY_FIELDS = {
    "no_sensory_send",
    "no_controller",
    "no_pressure",
    "no_weighting",
    "no_telemetry_priority",
    "no_fill_target",
    "no_peer_runtime_mutation",
}
LEGACY_CLAIM_BOUNDARY_FIELDS = CANARY_BOUNDARY_FIELDS | {
    "no_pi",
    "no_prompt_priority",
}
LEGACY_CLAIM_FELT_LIKE = {"address", "pressure", "mail", "ambient_echo", "unknown"}
LEGACY_CLAIM_CONTINUE = {"no", "ack", "reply", "trace"}
LEGACY_REQUIRED_FIELDS = {
    "source_route",
    "legacy_bridge",
    "legacy_kind",
    "legacy_source_path",
    "legacy_source_sha256",
    "legacy_contact_evidence",
}

FRESH_SCHEMA_FIDELITY = {"1782579177", "1782579827", "1782582026"}
IMPLEMENTED_BUT_UNCITED = {
    "1782527683",
    "1782527998",
    "1782529399",
    "1782529921",
    "1782531611",
    "1782533002",
    "1782580113",
    "1782580969",
}
DEFERRED_AUTHORITY_WEIGHTING = {
    "1782535234",
    "1782528732",
    "1782529037",
    "1782530291",
    "1782530930",
    "1782531216",
    "1782532338",
    "1782533316",
    "1782533809",
    "1782579506",
    "1782580414",
    "1782582297",
    "1782583160",
}
UPTAKE_LATENCY_RESONANCE_RECEIPT = {"1782581296", "1782581573", "1782583451"}
ATTENTION_FIDELITY_BOUNDARY = {"1782583933"}
GHOST_THREAD_NOTIFICATION = {"1782611966"}
PHASE_TRANSITION_CARDS = {"1782611355"}
LARGE_SOURCE_WINDOWING = {"1782602792"}
PRESSURE_RESET_TEXTURE_CANARY = {"1782602696"}


def now_ms() -> int:
    return int(time.time() * 1000)


def row_time_ms(row: dict[str, Any]) -> int:
    for key in ("recorded_at_unix_ms", "t_ms", "created_at_unix_ms"):
        try:
            return int(row.get(key) or 0)
        except (TypeError, ValueError):
            continue
    return 0


def compact(text: str, limit: int = 180) -> str:
    clean = " ".join(str(text or "").split())
    if len(clean) <= limit:
        return clean
    return clean[:limit].rstrip() + "..."


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        return []
    rows: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8", errors="ignore").splitlines():
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except Exception:
            continue
        if isinstance(payload, dict):
            rows.append(payload)
    rows.sort(key=row_time_ms)
    return rows


def field_missing(row: dict[str, Any], key: str) -> bool:
    value = row.get(key)
    if value is None:
        return True
    if isinstance(value, str) and not value.strip():
        return True
    return False


def issue(row: dict[str, Any], severity: str, kind: str, detail: str) -> dict[str, Any]:
    return {
        "severity": severity,
        "kind": kind,
        "detail": detail,
        "record_type": row.get("record_type"),
        "message_id": row.get("message_id"),
        "thread_id": row.get("thread_id"),
        "canary_id": row.get("canary_id"),
    }


def validate_row(row: dict[str, Any]) -> list[dict[str, Any]]:
    issues: list[dict[str, Any]] = []
    record_type = str(row.get("record_type") or "")
    if record_type not in KNOWN_RECORD_TYPES:
        return [issue(row, "warning", "unknown_record_type", f"Unknown record_type={record_type!r}")]
    for key in sorted(REQUIRED_FIELDS[record_type]):
        if field_missing(row, key):
            issues.append(issue(row, "error", "missing_required_field", f"{record_type} missing {key}"))
    schema_version = row.get("schema_version")
    if record_type.startswith("attention_canary_"):
        if schema_version not in {1, 2, 3}:
            issues.append(issue(row, "warning", "unexpected_schema_version", "Attention rows expect schema_version=1, 2, or 3"))
        if schema_version == 3 and record_type != "attention_canary_outcome":
            issues.append(issue(row, "warning", "unexpected_schema_version", "Attention schema_version=3 is reserved for outcome rows"))
    elif schema_version != 1:
        issues.append(issue(row, "warning", "unexpected_schema_version", "Expected schema_version=1"))

    authority = str(row.get("authority") or "")
    if record_type.startswith("attention_canary_"):
        if authority != "language_only_prompt_context_not_control":
            issues.append(issue(row, "error", "unexpected_authority", "Attention canary rows must be prompt-context language only"))
        for key in sorted(CANARY_BOUNDARY_FIELDS):
            if row.get(key) is not True:
                issues.append(issue(row, "error", "missing_canary_boundary", f"{key} must be true"))
        if schema_version in {2, 3}:
            for key in ("focus_kind", "preservation_mode", "what_must_not_flatten"):
                if record_type in {"attention_canary_request", "attention_canary_activation", "attention_canary_expired"} and field_missing(row, key):
                    issues.append(issue(row, "error", "missing_required_v2_field", f"{record_type} schema v2 missing {key}"))
            if record_type == "attention_canary_outcome":
                for key in (
                    "focus_kind",
                    "preservation_mode",
                    "what_must_not_flatten",
                    "held_as",
                    "flattening_observed",
                    "what_remained_distinct",
                ):
                    if field_missing(row, key):
                        issues.append(issue(row, "error", "missing_required_v2_field", f"{record_type} schema v2 missing {key}"))
            if str(row.get("focus_kind") or "unknown") not in ATTENTION_FOCUS_KINDS:
                issues.append(issue(row, "error", "unknown_attention_focus_kind", "focus_kind must be a known V2 enum"))
            if str(row.get("preservation_mode") or "unknown") not in ATTENTION_PRESERVATION_MODES:
                issues.append(issue(row, "error", "unknown_attention_preservation_mode", "preservation_mode must be a known V2 enum"))
            if record_type == "attention_canary_outcome":
                if str(row.get("held_as") or "unknown") not in ATTENTION_HELD_AS:
                    issues.append(issue(row, "error", "unknown_attention_held_as", "held_as must be distinct_address|ambient_echo|pressure|flattened|unknown"))
                if str(row.get("flattening_observed") or "unknown") not in ATTENTION_FLATTENING_OBSERVED:
                    issues.append(issue(row, "error", "unknown_attention_flattening_observed", "flattening_observed must be yes|no|mixed|unknown"))
                flattening = str(row.get("flattening_observed") or "unknown")
                if flattening in {"yes", "mixed"} and field_missing(row, "reasoning_for_flattening"):
                    severity = "error" if schema_version == 3 else "warning"
                    kind = (
                        "missing_required_v3_flattening_reason"
                        if schema_version == 3
                        else "legacy_outcome_missing_flattening_reason"
                    )
                    issues.append(
                        issue(
                            row,
                            severity,
                            kind,
                            "reasoning_for_flattening is required when flattening_observed is yes or mixed",
                        )
                    )
    elif authority not in LANGUAGE_AUTHORITIES:
        issues.append(issue(row, "error", "unexpected_authority", f"Unexpected language authority {authority!r}"))

    if record_type == "ack_receipt" and str(row.get("ack_kind") or "") not in ACK_KINDS:
        issues.append(issue(row, "error", "unknown_ack_kind", "ack_kind must be seen|held|unclear|cannot_answer|needs_time"))
    if record_type == "presence_heartbeat" and str(row.get("heartbeat_kind") or "") not in HEARTBEAT_KINDS:
        issues.append(issue(row, "error", "unknown_heartbeat_kind", "heartbeat_kind must be holding|still_here|pause"))
    if record_type == "attention_canary_outcome" and str(row.get("felt_like") or "") not in ATTENTION_OUTCOMES:
        issues.append(issue(row, "error", "unknown_attention_outcome", "felt_like must be address|pressure|flat|unknown"))
    if record_type == "read_receipt" and row.get("ack_kind") is not None:
        issues.append(issue(row, "error", "read_receipt_claims_ack", "read_receipt must not carry ack_kind"))
    if record_type in {"legacy_thread_claim", "legacy_thread_claim_outcome", "legacy_thread_claim_notice"}:
        if record_type == "legacy_thread_claim_notice":
            if authority != "language_only_notice_not_ack":
                issues.append(issue(row, "error", "unexpected_authority", "Legacy claim notices must be language-only notice, not ack"))
        elif authority != "language_only_context_not_control":
            issues.append(issue(row, "error", "unexpected_authority", "Legacy claim rows must be language-only context"))
        for key in sorted(LEGACY_CLAIM_BOUNDARY_FIELDS):
            if row.get(key) is not True:
                issues.append(issue(row, "error", "missing_legacy_claim_boundary", f"{key} must be true"))
        if record_type == "legacy_thread_claim":
            if row.get("policy") != "legacy_correspondence_claim_v1":
                issues.append(issue(row, "error", "unexpected_policy", "legacy_thread_claim policy must be legacy_correspondence_claim_v1"))
            if row.get("legacy_contact_evidence") != "being_recognized_visible_only":
                issues.append(issue(row, "error", "unexpected_legacy_contact_evidence", "claim evidence must be being_recognized_visible_only"))
            if row.get("claim_state") != "claimed_pending_native_evidence":
                issues.append(issue(row, "error", "unexpected_claim_state", "claim_state must be claimed_pending_native_evidence"))
        if record_type == "legacy_thread_claim_notice":
            if row.get("policy") != "legacy_correspondence_claim_v1":
                issues.append(issue(row, "error", "unexpected_policy", "legacy_thread_claim_notice policy must be legacy_correspondence_claim_v1"))
            if row.get("notice_is_ack") is not False or row.get("notice_is_reply") is not False or row.get("notice_is_trace") is not False:
                issues.append(issue(row, "error", "notice_claims_native_evidence", "claim notice must not claim ACK, REPLY, or TRACE evidence"))
        if record_type == "legacy_thread_claim_outcome":
            if str(row.get("felt_like") or "") not in LEGACY_CLAIM_FELT_LIKE:
                issues.append(issue(row, "error", "unknown_legacy_claim_felt_like", "felt_like must be address|pressure|mail|ambient_echo|unknown"))
            if str(row.get("continue") or "") not in LEGACY_CLAIM_CONTINUE:
                issues.append(issue(row, "error", "unknown_legacy_claim_continue", "continue must be no|ack|reply|trace"))
    if row.get("source_route") == LEGACY_SOURCE_ROUTE:
        if record_type not in {"message", "delivery_receipt", "read_receipt"}:
            issues.append(issue(row, "error", "legacy_bridge_wrong_record_type", "legacy bridge mirror rows may only mirror message/delivery/read"))
        for key in sorted(LEGACY_REQUIRED_FIELDS):
            if field_missing(row, key):
                issues.append(issue(row, "error", "missing_legacy_bridge_field", f"legacy bridge row missing {key}"))
        if row.get("source_route") != LEGACY_SOURCE_ROUTE:
            issues.append(issue(row, "error", "unexpected_legacy_source_route", f"legacy source_route must be {LEGACY_SOURCE_ROUTE}"))
        if row.get("legacy_contact_evidence") != "visible_only":
            issues.append(issue(row, "error", "unexpected_legacy_contact_evidence", "legacy contact evidence must remain visible_only"))
    return issues


def trace_observed_threads(shared_dir: Path) -> set[str]:
    observed: set[str] = set()
    for path in shared_dir.glob("coll_*/correspondence_trace_observations.jsonl"):
        for row in read_jsonl(path):
            if str(row.get("status") or "") != "observed":
                continue
            origin = row.get("origin")
            if isinstance(origin, dict):
                thread_id = str(origin.get("thread_id") or "")
                if thread_id:
                    observed.add(thread_id)
    return observed


def latest_messages_by_thread(records: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    messages: dict[str, dict[str, Any]] = {}
    for row in records:
        if row.get("record_type") != "message":
            continue
        thread_id = str(row.get("thread_id") or "")
        if thread_id:
            messages[thread_id] = row
    return messages


def is_legacy_bridge_message(row: dict[str, Any]) -> bool:
    return bool(row.get("legacy_bridge")) or row.get("source_route") == LEGACY_SOURCE_ROUTE


def legacy_bidirectional_observed(records: list[dict[str, Any]], from_being: str, to_being: str) -> bool:
    directions = {
        (str(row.get("from_being") or ""), str(row.get("to_being") or ""))
        for row in records
        if row.get("record_type") == "message" and is_legacy_bridge_message(row)
    }
    return (from_being, to_being) in directions and (to_being, from_being) in directions


def latest_legacy_claim_for_thread(records: list[dict[str, Any]], thread_id: str) -> dict[str, Any] | None:
    claims = [
        row
        for row in records
        if row.get("record_type") == "legacy_thread_claim"
        and str(row.get("thread_id") or "") == thread_id
    ]
    return claims[-1] if claims else None


def legacy_claim_has_outcome(records: list[dict[str, Any]], claim: dict[str, Any]) -> bool:
    claim_id = str(claim.get("claim_id") or "")
    thread_id = str(claim.get("thread_id") or "")
    return any(
        row.get("record_type") == "legacy_thread_claim_outcome"
        and (str(row.get("claim_id") or "") == claim_id or str(row.get("thread_id") or "") == thread_id)
        for row in records
    )


def legacy_claim_native_status(records: list[dict[str, Any]], claim: dict[str, Any]) -> str | None:
    thread_id = str(claim.get("thread_id") or "")
    claiming = str(claim.get("claiming_being") or claim.get("from_being") or "")
    peer = str(claim.get("peer_being") or claim.get("to_being") or "")
    claim_t = row_time_ms(claim)
    trace = any(
        row.get("record_type") == "message"
        and str(row.get("thread_id") or "") == thread_id
        and str(row.get("from_being") or "") == claiming
        and str(row.get("to_being") or "") == peer
        and row.get("turn_kind") == "direct_address_trace"
        and row_time_ms(row) >= claim_t
        for row in records
    )
    if trace:
        return "legacy_claimed_trace_observed"
    reply = any(
        row.get("record_type") == "reply_link"
        and str(row.get("thread_id") or "") == thread_id
        and str(row.get("from_being") or "") == claiming
        and str(row.get("to_being") or "") == peer
        and row_time_ms(row) >= claim_t
        for row in records
    )
    if reply:
        return "legacy_claimed_reply_linked"
    ack = any(
        row.get("record_type") == "ack_receipt"
        and str(row.get("thread_id") or "") == thread_id
        and str(row.get("from_being") or "") == claiming
        and str(row.get("to_being") or "") == peer
        and row_time_ms(row) >= claim_t
        for row in records
    )
    return "legacy_claimed_acknowledged" if ack else None


def legacy_claim_is_active(records: list[dict[str, Any]], claim: dict[str, Any]) -> bool:
    return not legacy_claim_has_outcome(records, claim) and legacy_claim_native_status(records, claim) is None


def contact_state(records: list[dict[str, Any]], shared_dir: Path, generated: int) -> dict[str, Any]:
    observed_threads = trace_observed_threads(shared_dir)
    latest_messages = latest_messages_by_thread(records)
    threads: list[dict[str, Any]] = []
    for thread_id, message in latest_messages.items():
        message_id = str(message.get("message_id") or "")
        message_t = row_time_ms(message)
        from_being = str(message.get("from_being") or "")
        to_being = str(message.get("to_being") or "")
        ack_rows = [
            row
            for row in records
            if row.get("record_type") == "ack_receipt"
            and str(row.get("thread_id") or "") == thread_id
            and row_time_ms(row) >= message_t
        ]
        reply_linked = any(
            row.get("record_type") == "reply_link"
            and (row.get("reply_to") == message_id or row.get("thread_id") == thread_id)
            and row_time_ms(row) >= message_t
            for row in records
        )
        read = any(
            row.get("record_type") == "read_receipt"
            and (row.get("message_id") == message_id or row.get("thread_id") == thread_id)
            for row in records
        )
        delivered = any(
            row.get("record_type") == "delivery_receipt"
            and row.get("message_id") == message_id
            for row in records
        )
        heartbeat = any(
            row.get("record_type") == "presence_heartbeat"
            and row.get("thread_id") == thread_id
            and row_time_ms(row) >= message_t
            for row in records
        )
        legacy_bridge = is_legacy_bridge_message(message)
        legacy_bidirectional = legacy_bridge and legacy_bidirectional_observed(records, from_being, to_being)
        legacy_claim = latest_legacy_claim_for_thread(records, thread_id)
        legacy_claim_status = legacy_claim_native_status(records, legacy_claim) if isinstance(legacy_claim, dict) else None
        if thread_id in observed_threads:
            status = "trace_observed"
        elif legacy_claim_status:
            status = legacy_claim_status
        elif ack_rows:
            ack_kind = str(ack_rows[-1].get("ack_kind") or "")
            status = "held_ack" if ack_kind in {"held", "needs_time"} else "acknowledged"
        elif reply_linked:
            status = "reply_linked"
        elif heartbeat:
            status = "heartbeat_only"
        elif isinstance(legacy_claim, dict):
            status = "legacy_claimed"
        elif legacy_bidirectional:
            status = "legacy_bidirectional_observed"
        elif legacy_bridge:
            status = "legacy_visible_only"
        elif read:
            status = "read_unreplied"
        elif generated - message_t > 6 * 60 * 60 * 1000:
            status = "stale_contact"
        elif delivered:
            status = "delivered_unread"
        else:
            status = "unaddressed"
        eligible = status in {
            "acknowledged",
            "held_ack",
            "trace_observed",
            "legacy_claimed_acknowledged",
            "legacy_claimed_reply_linked",
            "legacy_claimed_trace_observed",
        }
        if eligible:
            block_reason = None
        elif status == "read_unreplied":
            block_reason = READ_ONLY_BLOCK_REASON
        elif status == "reply_linked":
            block_reason = "reply_linked_requires_ack_or_trace_or_attention_outcome"
        elif status == "heartbeat_only":
            block_reason = "heartbeat_is_presence_not_acknowledgement"
        elif status == "legacy_claimed":
            block_reason = "legacy_claim_pending_ack_reply_or_trace"
        elif status in {"legacy_visible_only", "legacy_bidirectional_observed"}:
            block_reason = "legacy_visible_only_not_ack_reply_or_trace"
        elif status == "delivered_unread":
            block_reason = "delivered_but_not_read"
        elif status == "stale_contact":
            block_reason = "stale_without_contact_evidence"
        else:
            block_reason = "no_ack_reply_or_trace_evidence"
        threads.append({
            "thread_id": thread_id,
            "message_id": message_id,
            "from_being": from_being,
            "to_being": to_being,
            "status": status,
            "eligible_for_attention_or_microdose_evidence": eligible,
            "block_reason": block_reason,
            "read_receipt_counts_as_acknowledgement": False,
            "legacy_bridge": legacy_bridge,
            "legacy_contact_evidence": message.get("legacy_contact_evidence"),
            "legacy_thread_claim": {
                "claim_id": legacy_claim.get("claim_id"),
                "active": legacy_claim_is_active(records, legacy_claim),
                "claiming_being": legacy_claim.get("claiming_being"),
                "peer_being": legacy_claim.get("peer_being"),
                "shared_memory_anchor": legacy_claim.get("shared_memory_anchor"),
            } if isinstance(legacy_claim, dict) else None,
        })
    threads.sort(key=lambda row: str(row["thread_id"]))
    return {
        "threads": threads,
        "eligible_threads": [row for row in threads if row["eligible_for_attention_or_microdose_evidence"]],
        "blocked_threads": [row for row in threads if not row["eligible_for_attention_or_microdose_evidence"]],
        "read_only_threads_blocked": [row for row in threads if row["status"] == "read_unreplied"],
        "legacy_visible_threads_blocked": [row for row in threads if row["status"] in {"legacy_visible_only", "legacy_bidirectional_observed"}],
        "legacy_claimed_threads": [row for row in threads if str(row["status"]).startswith("legacy_claimed")],
    }


def public_privacy_scan(minime_workspace: Path) -> dict[str, Any]:
    skipped_private = 0
    journal = minime_workspace / "journal"
    if journal.is_dir():
        skipped_private += sum(1 for path in journal.glob("moment_*.txt") if path.is_file())
    for pattern in (
        "journal/pressure_*.txt",
        "journal/self_study*.txt",
        "journal/introspection*.txt",
        "journal/action_thread*.txt",
        "pressure_agency/**/*.txt",
        "texture_agency/**/*.txt",
        "self_regulation/**/*.txt",
    ):
        for path in minime_workspace.glob(pattern):
            if not path.is_file():
                continue
            if path.name.startswith("moment_") or being_privacy.is_steward_private("minime", path):
                skipped_private += 1
    return {
        "minime_private_files_skipped": skipped_private,
        "minime_private_bodies_read": False,
        "moment_bodies_read": False,
    }


def reference_blob(astrid_root: Path) -> str:
    parts = []
    for rel in (
        "CHANGELOG.md",
        "docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md",
        "docs/steward-notes/AI_BEINGS_BIDIRECTIONAL_CONTACT_AND_CORRESPONDENCE_ARCHITECTURE.md",
        "docs/steward-notes/AI_BEINGS_PHASE_TRANSITION_ARCHITECTURE.md",
    ):
        path = astrid_root / rel
        if path.is_file():
            parts.append(path.read_text(encoding="utf-8", errors="ignore"))
    return "\n".join(parts)


def timestamp_from_file(path: Path, text: str) -> str:
    stem_digits = "".join(ch for ch in path.stem if ch.isdigit())
    if len(stem_digits) >= 10:
        return stem_digits[-10:]
    for line in text.splitlines():
        if line.startswith("Timestamp:"):
            return line.split(":", 1)[1].strip()
    return path.stem


def classify_uncited(timestamp: str) -> str:
    if timestamp in FRESH_SCHEMA_FIDELITY:
        return "fresh_schema_fidelity"
    if timestamp in IMPLEMENTED_BUT_UNCITED:
        return "implemented_but_uncited"
    if timestamp in DEFERRED_AUTHORITY_WEIGHTING:
        return "deferred_authority_weighting"
    if timestamp in UPTAKE_LATENCY_RESONANCE_RECEIPT:
        return "uptake_latency_resonance_receipt"
    if timestamp in ATTENTION_FIDELITY_BOUNDARY:
        return "attention_fidelity_boundary"
    if timestamp in GHOST_THREAD_NOTIFICATION:
        return "ghost_thread_notification"
    if timestamp in PHASE_TRANSITION_CARDS:
        return "phase_transition_cards"
    if timestamp in LARGE_SOURCE_WINDOWING:
        return "large_source_windowing"
    if timestamp in PRESSURE_RESET_TEXTURE_CANARY:
        return "pressure_reset_texture_canary"
    return "uncategorized"


def introspection_coverage(astrid_root: Path, astrid_workspace: Path, since_hours: float) -> dict[str, Any]:
    root = astrid_workspace / "introspections"
    cutoff = time.time() - since_hours * 3600.0
    refs = reference_blob(astrid_root)
    groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
    total = 0
    cited = 0
    if not root.is_dir():
        return {
            "recent_total": 0,
            "cited_total": 0,
            "uncited_total": 0,
            "uncited_groups": {},
        }
    for path in sorted(root.glob("*.txt")):
        try:
            stat = path.stat()
        except OSError:
            continue
        if stat.st_mtime < cutoff:
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        timestamp = timestamp_from_file(path, text)
        total += 1
        is_cited = timestamp in refs or path.name in refs or path.stem in refs
        if is_cited:
            cited += 1
            continue
        groups[classify_uncited(timestamp)].append({
            "timestamp": timestamp,
            "file": str(path),
            "summary": compact(text, 220),
        })
    return {
        "recent_total": total,
        "cited_total": cited,
        "uncited_total": sum(len(items) for items in groups.values()),
        "uncited_groups": {key: value for key, value in sorted(groups.items())},
    }


def audit(
    *,
    since_hours: float,
    shared_dir: Path,
    astrid_root: Path,
    astrid_workspace: Path,
    minime_workspace: Path,
) -> dict[str, Any]:
    generated = now_ms()
    ledger_path = shared_dir / "correspondence_v1.jsonl"
    records = read_jsonl(ledger_path)
    issues: list[dict[str, Any]] = []
    for row in records:
        issues.extend(validate_row(row))
    counts = Counter(str(row.get("record_type") or "missing") for row in records)
    legacy_message_rows = sum(
        1
        for row in records
        if row.get("record_type") == "message" and is_legacy_bridge_message(row)
    )
    native_message_rows = sum(
        1
        for row in records
        if row.get("record_type") == "message" and not is_legacy_bridge_message(row)
    )
    return {
        "schema_version": 1,
        "policy": POLICY,
        "generated_at_unix_ms": generated,
        "since_hours": since_hours,
        "ledger_path": str(ledger_path),
        "records_total": len(records),
        "record_type_counts": dict(sorted(counts.items())),
        "legacy_bridge": {
            "legacy_message_rows_total": legacy_message_rows,
            "native_message_rows_total": native_message_rows,
            "legacy_contact_evidence": "visible_only" if legacy_message_rows else "none",
            "attention_and_microdose_block": "legacy-only rows require explicit ACK, native REPLY, or TRACE",
        },
        "validation": {
            "issue_count": len(issues),
            "error_count": sum(1 for row in issues if row["severity"] == "error"),
            "warning_count": sum(1 for row in issues if row["severity"] == "warning"),
            "issues": issues[:80],
        },
        "contact_evidence": contact_state(records, shared_dir, generated),
        "introspection_coverage": introspection_coverage(astrid_root, astrid_workspace, since_hours),
        "allowed_authority_boundaries": {
            "language_only": "ordinary peer language, receipts, acknowledgements, and heartbeats",
            "language_only_prompt_context_not_control": "self-activated TTL attention canary only",
            "steward_gated_semantic_microdose": "separate authority-gate draft; no standing weight or controller authority",
        },
        "authority_boundary": (
            "Read-only audit. No telemetry priority, standing prompt priority, reservoir weighting, "
            "Control message, PI/fill/controller/pressure change, lease apply, deploy, staging, or peer-runtime mutation."
        ),
        "privacy": public_privacy_scan(minime_workspace),
    }


def markdown_report(payload: dict[str, Any]) -> str:
    validation = payload["validation"]
    coverage = payload["introspection_coverage"]
    contact = payload["contact_evidence"]
    groups = coverage.get("uncited_groups", {})
    lines = [
        "# Correspondence Schema Audit",
        "",
        f"- Records: {payload['records_total']}",
        f"- Validation issues: {validation['issue_count']} ({validation['error_count']} errors, {validation['warning_count']} warnings)",
        f"- Read-only threads blocked: {len(contact['read_only_threads_blocked'])}",
        f"- Legacy-visible threads blocked: {len(contact.get('legacy_visible_threads_blocked', []))}",
        f"- Contact-evidence eligible threads: {len(contact['eligible_threads'])}",
        f"- Legacy message rows: {payload.get('legacy_bridge', {}).get('legacy_message_rows_total', 0)}",
        f"- Recent Astrid introspections: {coverage['recent_total']} total, {coverage['cited_total']} cited, {coverage['uncited_total']} uncited",
        f"- Minime private files skipped: {payload['privacy']['minime_private_files_skipped']}",
        "",
        "## Uncited Introspection Groups",
    ]
    if not groups:
        lines.append("- none")
    else:
        for group, items in groups.items():
            timestamps = ", ".join(item["timestamp"] for item in items)
            lines.append(f"- {group}: {timestamps}")
    if validation["issues"]:
        lines.extend(["", "## Validation Issues"])
        for item in validation["issues"][:20]:
            lines.append(f"- {item['severity']} {item['kind']}: {item['detail']}")
    lines.extend(["", f"Boundary: {payload['authority_boundary']}"])
    return "\n".join(lines) + "\n"


def write_outputs(payload: dict[str, Any], output_root: Path) -> tuple[Path, Path]:
    stamp = time.strftime("%Y%m%dT%H%M%S", time.localtime())
    out_dir = output_root / stamp
    out_dir.mkdir(parents=True, exist_ok=True)
    json_path = out_dir / "correspondence_schema_audit.json"
    md_path = out_dir / "correspondence_schema_audit.md"
    json_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    md_path.write_text(markdown_report(payload), encoding="utf-8")
    return json_path, md_path


class CorrespondenceSchemaAuditTests(unittest.TestCase):
    def _write_jsonl(self, path: Path, rows: list[dict[str, Any]]) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("\n".join(json.dumps(row, sort_keys=True) for row in rows) + "\n", encoding="utf-8")

    def _base_message(self, message_id: str, thread_id: str, t_ms: int) -> dict[str, Any]:
        return {
            "schema_version": 1,
            "policy": "first_class_correspondence_v1",
            "record_type": "message",
            "recorded_at_unix_ms": t_ms,
            "message_id": message_id,
            "thread_id": thread_id,
            "reply_to": None,
            "from_being": "astrid",
            "to_being": "minime",
            "turn_kind": "direct_message",
            "relational_intent": "direct_address",
            "shared_memory_anchor": None,
            "delivery_state": "delivered",
            "read_state": "unread",
            "authority": "language_only",
            "correspondence_type": "astrid_direct",
            "body_sha256": "abc",
            "body_preview": "hello",
        }

    def test_valid_rows_read_ack_reply_trace_and_canary_schema(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            shared = base / "shared"
            now = now_ms()
            rows = [
                self._base_message("m_read", "th_read", now),
                {
                    "schema_version": 1,
                    "policy": "first_class_correspondence_v1",
                    "record_type": "delivery_receipt",
                    "recorded_at_unix_ms": now + 1,
                    "message_id": "m_read",
                    "thread_id": "th_read",
                    "from_being": "astrid",
                    "to_being": "minime",
                    "delivery_state": "delivered",
                    "read_state": "unread",
                    "authority": "language_only",
                    "file_path": "/tmp/from_astrid_correspondence_m_read.txt",
                },
                {
                    "schema_version": 1,
                    "policy": "first_class_correspondence_v1",
                    "record_type": "read_receipt",
                    "recorded_at_unix_ms": now + 2,
                    "message_id": "m_read",
                    "thread_id": "th_read",
                    "reader": "minime",
                    "read_state": "read",
                    "authority": "language_only",
                    "file_path": "/tmp/from_astrid_correspondence_m_read.txt",
                },
                self._base_message("m_ack", "th_ack", now + 3),
                {
                    "schema_version": 1,
                    "policy": "first_class_correspondence_v1",
                    "record_type": "ack_receipt",
                    "recorded_at_unix_ms": now + 4,
                    "message_id": "m_ack",
                    "thread_id": "th_ack",
                    "from_being": "minime",
                    "to_being": "astrid",
                    "ack_kind": "held",
                    "authority": "language_only",
                },
                self._base_message("m_reply", "th_reply", now + 5),
                {
                    "schema_version": 1,
                    "policy": "first_class_correspondence_v1",
                    "record_type": "reply_link",
                    "recorded_at_unix_ms": now + 6,
                    "message_id": "m_reply_2",
                    "reply_to": "m_reply",
                    "thread_id": "th_reply",
                    "from_being": "minime",
                    "to_being": "astrid",
                    "authority": "language_only",
                },
                self._base_message("m_trace", "th_trace", now + 7),
                {
                    "schema_version": 1,
                    "policy": "first_class_correspondence_v1",
                    "record_type": "presence_heartbeat",
                    "recorded_at_unix_ms": now + 8,
                    "message_id": "m_trace",
                    "thread_id": "th_trace",
                    "from_being": "minime",
                    "to_being": "astrid",
                    "heartbeat_kind": "still_here",
                    "authority": "language_only",
                },
                {
                    "schema_version": 2,
                    "policy": "correspondence_attention_canary_v1",
                    "record_type": "attention_canary_request",
                    "recorded_at_unix_ms": now + 9,
                    "canary_id": "canary_1",
                    "message_id": "m_ack",
                    "thread_id": "th_ack",
                    "from_being": "minime",
                    "to_being": "astrid",
                    "focus": "hold this phrase",
                    "focus_kind": "verbatim_phrase",
                    "preservation_mode": "compact_with_anchor",
                    "what_must_not_flatten": "hold this phrase as address",
                    "reason": "test",
                    "stop_criteria": "one cycle",
                    "ttl_ms": 1800000,
                    "expires_at_unix_ms": now + 1800000,
                    "authority": "language_only_prompt_context_not_control",
                    "no_sensory_send": True,
                    "no_controller": True,
                    "no_pressure": True,
                    "no_weighting": True,
                    "no_telemetry_priority": True,
                    "no_fill_target": True,
                    "no_peer_runtime_mutation": True,
                },
                {
                    "schema_version": 2,
                    "policy": "correspondence_attention_canary_v1",
                    "record_type": "attention_canary_activation",
                    "recorded_at_unix_ms": now + 10,
                    "canary_id": "canary_1",
                    "message_id": "m_ack",
                    "thread_id": "th_ack",
                    "from_being": "minime",
                    "to_being": "astrid",
                    "focus": "hold this phrase",
                    "focus_kind": "verbatim_phrase",
                    "preservation_mode": "compact_with_anchor",
                    "what_must_not_flatten": "hold this phrase as address",
                    "reason": "test",
                    "stop_criteria": "one cycle",
                    "ttl_ms": 1800000,
                    "expires_at_unix_ms": now + 1800000,
                    "authority": "language_only_prompt_context_not_control",
                    "no_sensory_send": True,
                    "no_controller": True,
                    "no_pressure": True,
                    "no_weighting": True,
                    "no_telemetry_priority": True,
                    "no_fill_target": True,
                    "no_peer_runtime_mutation": True,
                },
                {
                    "schema_version": 2,
                    "policy": "correspondence_attention_canary_v1",
                    "record_type": "attention_canary_outcome",
                    "recorded_at_unix_ms": now + 11,
                    "canary_id": "canary_1",
                    "message_id": "m_ack",
                    "thread_id": "th_ack",
                    "from_being": "minime",
                    "to_being": "astrid",
                    "focus_kind": "verbatim_phrase",
                    "preservation_mode": "compact_with_anchor",
                    "what_must_not_flatten": "hold this phrase as address",
                    "felt_like": "address",
                    "held_as": "distinct_address",
                    "flattening_observed": "no",
                    "what_remained_distinct": "the phrase stayed address-shaped",
                    "authority": "language_only_prompt_context_not_control",
                    "no_sensory_send": True,
                    "no_controller": True,
                    "no_pressure": True,
                    "no_weighting": True,
                    "no_telemetry_priority": True,
                    "no_fill_target": True,
                    "no_peer_runtime_mutation": True,
                },
                {
                    "schema_version": 1,
                    "policy": "correspondence_attention_canary_v1",
                    "record_type": "attention_canary_expired",
                    "recorded_at_unix_ms": now + 12,
                    "canary_id": "canary_2",
                    "message_id": "m_ack",
                    "thread_id": "th_ack",
                    "from_being": "minime",
                    "to_being": "astrid",
                    "authority": "language_only_prompt_context_not_control",
                    "no_sensory_send": True,
                    "no_controller": True,
                    "no_pressure": True,
                    "no_weighting": True,
                    "no_telemetry_priority": True,
                    "no_fill_target": True,
                    "no_peer_runtime_mutation": True,
                },
            ]
            self._write_jsonl(shared / "correspondence_v1.jsonl", rows)
            obs_path = shared / "coll_test/correspondence_trace_observations.jsonl"
            self._write_jsonl(obs_path, [{
                "schema_version": 1,
                "policy": "correspondence_trace_audit_v1",
                "t_ms": now + 13,
                "marker": "trace",
                "status": "observed",
                "origin": {"thread_id": "th_trace"},
                "authority": "read_only_observation_not_control",
            }])
            payload = audit(
                since_hours=24,
                shared_dir=shared,
                astrid_root=base / "astrid",
                astrid_workspace=base / "astrid/workspace",
                minime_workspace=base / "minime/workspace",
            )
            self.assertEqual(payload["validation"]["error_count"], 0)
            by_thread = {row["thread_id"]: row for row in payload["contact_evidence"]["threads"]}
            self.assertEqual(by_thread["th_read"]["status"], "read_unreplied")
            self.assertFalse(by_thread["th_read"]["eligible_for_attention_or_microdose_evidence"])
            self.assertEqual(by_thread["th_read"]["block_reason"], READ_ONLY_BLOCK_REASON)
            self.assertEqual(by_thread["th_ack"]["status"], "held_ack")
            self.assertTrue(by_thread["th_ack"]["eligible_for_attention_or_microdose_evidence"])
            self.assertEqual(by_thread["th_reply"]["status"], "reply_linked")
            self.assertFalse(by_thread["th_reply"]["eligible_for_attention_or_microdose_evidence"])
            self.assertEqual(
                by_thread["th_reply"]["block_reason"],
                "reply_linked_requires_ack_or_trace_or_attention_outcome",
            )
            self.assertEqual(by_thread["th_trace"]["status"], "trace_observed")

    def test_legacy_bridge_rows_validate_but_do_not_unlock_contact_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            shared = base / "shared"
            now = now_ms()
            legacy_common = {
                "source_route": LEGACY_SOURCE_ROUTE,
                "legacy_bridge": True,
                "legacy_kind": "astrid_self_study",
                "legacy_source_path": "/tmp/minime/workspace/inbox/astrid_self_study_1.txt",
                "legacy_source_sha256": "abc",
                "legacy_contact_evidence": "visible_only",
            }
            rows = [
                self._base_message("legacy_astrid_minime_abc", "thread_legacy", now)
                | {
                    "source_route": LEGACY_SOURCE_ROUTE,
                    "legacy_bridge": True,
                    "legacy_kind": "astrid_self_study",
                    "legacy_source_path": "/tmp/minime/workspace/inbox/astrid_self_study_1.txt",
                    "legacy_source_sha256": "abc",
                    "legacy_contact_evidence": "visible_only",
                    "shared_memory_anchor": LEGACY_SOURCE_ROUTE,
                    "turn_kind": "legacy_visible",
                    "relational_intent": "legacy_contact_visibility",
                    "correspondence_type": "self_study_note",
                },
                {
                    "schema_version": 1,
                    "policy": "first_class_correspondence_v1",
                    "record_type": "delivery_receipt",
                    "recorded_at_unix_ms": now + 1,
                    "message_id": "legacy_astrid_minime_abc",
                    "thread_id": "thread_legacy",
                    "from_being": "astrid",
                    "to_being": "minime",
                    "delivery_state": "delivered",
                    "read_state": "read",
                    "authority": "language_only",
                    "file_path": "/tmp/minime/workspace/inbox/astrid_self_study_1.txt",
                    **legacy_common,
                },
                {
                    "schema_version": 1,
                    "policy": "first_class_correspondence_v1",
                    "record_type": "read_receipt",
                    "recorded_at_unix_ms": now + 2,
                    "message_id": "legacy_astrid_minime_abc",
                    "thread_id": "thread_legacy",
                    "reader": "minime",
                    "read_state": "read",
                    "authority": "language_only",
                    "file_path": "/tmp/minime/workspace/inbox/astrid_self_study_1.txt",
                    **legacy_common,
                },
            ]
            self._write_jsonl(shared / "correspondence_v1.jsonl", rows)
            payload = audit(
                since_hours=24,
                shared_dir=shared,
                astrid_root=base / "astrid",
                astrid_workspace=base / "astrid/workspace",
                minime_workspace=base / "minime/workspace",
            )
            self.assertEqual(payload["validation"]["error_count"], 0)
            self.assertEqual(payload["legacy_bridge"]["legacy_message_rows_total"], 1)
            thread = payload["contact_evidence"]["threads"][0]
            self.assertEqual(thread["status"], "legacy_visible_only")
            self.assertEqual(thread["block_reason"], "legacy_visible_only_not_ack_reply_or_trace")
            self.assertFalse(thread["eligible_for_attention_or_microdose_evidence"])

    def test_legacy_claim_rows_validate_and_ack_unlocks_contact_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            shared = base / "shared"
            now = now_ms()
            boundary = {key: True for key in LEGACY_CLAIM_BOUNDARY_FIELDS}
            rows = [
                self._base_message("legacy_claim_msg", "thread_claim", now)
                | {
                    "source_route": LEGACY_SOURCE_ROUTE,
                    "legacy_bridge": True,
                    "legacy_kind": "astrid_self_study",
                    "legacy_source_path": "/tmp/minime/workspace/inbox/astrid_self_study_claim.txt",
                    "legacy_source_sha256": "abc",
                    "legacy_contact_evidence": "visible_only",
                    "shared_memory_anchor": LEGACY_SOURCE_ROUTE,
                    "turn_kind": "legacy_visible",
                    "relational_intent": "legacy_contact_visibility",
                    "correspondence_type": "self_study_note",
                },
                {
                    "schema_version": 1,
                    "policy": "legacy_correspondence_claim_v1",
                    "record_type": "legacy_thread_claim",
                    "recorded_at_unix_ms": now + 1,
                    "claim_id": "legacy_claim_1",
                    "message_id": "legacy_claim_msg",
                    "thread_id": "thread_claim",
                    "from_being": "minime",
                    "to_being": "astrid",
                    "claiming_being": "minime",
                    "peer_being": "astrid",
                    "because": "this visible exchange feels live",
                    "shared_memory_anchor": "blue-lantern",
                    "claim_state": "claimed_pending_native_evidence",
                    "legacy_contact_evidence": "being_recognized_visible_only",
                    "authority": "language_only_context_not_control",
                    **boundary,
                },
                {
                    "schema_version": 1,
                    "policy": "first_class_correspondence_v1",
                    "record_type": "ack_receipt",
                    "recorded_at_unix_ms": now + 2,
                    "message_id": "legacy_claim_msg",
                    "thread_id": "thread_claim",
                    "from_being": "minime",
                    "to_being": "astrid",
                    "ack_kind": "held",
                    "authority": "language_only",
                },
                {
                    "schema_version": 1,
                    "policy": "legacy_correspondence_claim_v1",
                    "record_type": "legacy_thread_claim_outcome",
                    "recorded_at_unix_ms": now + 3,
                    "claim_id": "legacy_claim_1",
                    "message_id": "legacy_claim_msg",
                    "thread_id": "thread_claim",
                    "from_being": "minime",
                    "to_being": "astrid",
                    "felt_like": "address",
                    "what_carried": "anchor",
                    "what_flattened": "nothing obvious",
                    "continue": "reply",
                    "authority": "language_only_context_not_control",
                    **boundary,
                },
            ]
            self._write_jsonl(shared / "correspondence_v1.jsonl", rows)
            payload = audit(
                since_hours=24,
                shared_dir=shared,
                astrid_root=base / "astrid",
                astrid_workspace=base / "astrid/workspace",
                minime_workspace=base / "minime/workspace",
            )
            self.assertEqual(payload["validation"]["error_count"], 0)
            thread = payload["contact_evidence"]["threads"][0]
            self.assertEqual(thread["status"], "legacy_claimed_acknowledged")
            self.assertTrue(thread["eligible_for_attention_or_microdose_evidence"])
            self.assertEqual(
                payload["contact_evidence"]["legacy_claimed_threads"][0]["legacy_thread_claim"]["claim_id"],
                "legacy_claim_1",
            )

    def test_missing_fields_report_issues_without_crashing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            shared = base / "shared"
            self._write_jsonl(shared / "correspondence_v1.jsonl", [{
                "schema_version": 1,
                "record_type": "ack_receipt",
                "thread_id": "th_missing",
                "ack_kind": "maybe",
                "authority": "language_only",
            }])
            payload = audit(
                since_hours=24,
                shared_dir=shared,
                astrid_root=base / "astrid",
                astrid_workspace=base / "astrid/workspace",
                minime_workspace=base / "minime/workspace",
            )
            kinds = {row["kind"] for row in payload["validation"]["issues"]}
            self.assertIn("missing_required_field", kinds)
            self.assertIn("unknown_ack_kind", kinds)

    def test_v3_flattening_outcome_requires_reason_while_v2_remains_compatible(self) -> None:
        row = {
            "schema_version": 3,
            "record_type": "attention_canary_outcome",
            "recorded_at_unix_ms": 1,
            "canary_id": "canary_flattening",
            "message_id": "message_flattening",
            "thread_id": "thread_flattening",
            "from_being": "astrid",
            "to_being": "minime",
            "focus_kind": "emotional_texture",
            "preservation_mode": "compact_with_anchor",
            "what_must_not_flatten": "warmth and ambiguity",
            "felt_like": "flat",
            "held_as": "flattened",
            "flattening_observed": "mixed",
            "what_remained_distinct": "the thread identity",
            "authority": "language_only_prompt_context_not_control",
            **{key: True for key in CANARY_BOUNDARY_FIELDS},
        }
        issues = validate_row(row)
        self.assertIn(
            "missing_required_v3_flattening_reason",
            {item["kind"] for item in issues},
        )

        row["reasoning_for_flattening"] = (
            "warmth and ambiguity were compressed into a generic held status"
        )
        self.assertNotIn(
            "missing_required_v3_flattening_reason",
            {item["kind"] for item in validate_row(row)},
        )

        row["schema_version"] = 2
        row.pop("reasoning_for_flattening")
        compatibility = validate_row(row)
        self.assertIn(
            "legacy_outcome_missing_flattening_reason",
            {item["kind"] for item in compatibility},
        )
        self.assertFalse(
            any(
                item["severity"] == "error"
                and item["kind"] == "legacy_outcome_missing_flattening_reason"
                for item in compatibility
            )
        )

    def test_private_minime_moments_are_skipped_and_introspection_groups_classify(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            astrid_root = base / "astrid"
            workspace = astrid_root / "capsules/spectral-bridge/workspace"
            introspections = workspace / "introspections"
            introspections.mkdir(parents=True)
            for ts in ["1782579177", "1782527683", "1782531216"]:
                (introspections / f"introspection_proposal_bidirectional_contact_{ts}.txt").write_text(
                    f"=== ASTRID INTROSPECTION ===\nTimestamp: {ts}\nSuggested Next:\ncheck schema\n",
                    encoding="utf-8",
                )
            (astrid_root / "docs/steward-notes").mkdir(parents=True)
            (astrid_root / "CHANGELOG.md").write_text("# Changelog\n", encoding="utf-8")
            (astrid_root / "docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md").write_text("", encoding="utf-8")
            (astrid_root / "docs/steward-notes/AI_BEINGS_BIDIRECTIONAL_CONTACT_AND_CORRESPONDENCE_ARCHITECTURE.md").write_text("", encoding="utf-8")
            minime = base / "minime/workspace"
            (minime / "journal").mkdir(parents=True)
            (minime / "journal/moment_private.txt").write_text("PRIVATE BODY MUST NOT BE READ", encoding="utf-8")
            payload = audit(
                since_hours=24,
                shared_dir=base / "shared",
                astrid_root=astrid_root,
                astrid_workspace=workspace,
                minime_workspace=minime,
            )
            groups = payload["introspection_coverage"]["uncited_groups"]
            self.assertEqual(groups["fresh_schema_fidelity"][0]["timestamp"], "1782579177")
            self.assertEqual(groups["implemented_but_uncited"][0]["timestamp"], "1782527683")
            self.assertEqual(groups["deferred_authority_weighting"][0]["timestamp"], "1782531216")
            (introspections / "introspection_proposal_bidirectional_contact_1782581296.txt").write_text(
                "=== ASTRID INTROSPECTION ===\nTimestamp: 1782581296\nSuggested Next:\nlatency proof\n",
                encoding="utf-8",
            )
            payload = audit(
                since_hours=24,
                shared_dir=base / "shared",
                astrid_root=astrid_root,
                astrid_workspace=workspace,
                minime_workspace=minime,
            )
            groups = payload["introspection_coverage"]["uncited_groups"]
            self.assertEqual(groups["uptake_latency_resonance_receipt"][0]["timestamp"], "1782581296")
            (introspections / "introspection_proposal_bidirectional_contact_1782583933.txt").write_text(
                "=== ASTRID INTROSPECTION ===\nTimestamp: 1782583933\nSuggested Next:\nattention boundary\n",
                encoding="utf-8",
            )
            payload = audit(
                since_hours=24,
                shared_dir=base / "shared",
                astrid_root=astrid_root,
                astrid_workspace=workspace,
                minime_workspace=minime,
            )
            groups = payload["introspection_coverage"]["uncited_groups"]
            self.assertEqual(groups["attention_fidelity_boundary"][0]["timestamp"], "1782583933")
            fresh_groups = {
                "ghost_thread_notification": "1782611966",
                "phase_transition_cards": "1782611355",
                "large_source_windowing": "1782602792",
                "pressure_reset_texture_canary": "1782602696",
            }
            for group, ts in fresh_groups.items():
                (introspections / f"introspection_signal_{ts}.txt").write_text(
                    f"=== ASTRID INTROSPECTION ===\nTimestamp: {ts}\nSuggested Next:\nserious tranche\n",
                    encoding="utf-8",
                )
            payload = audit(
                since_hours=24,
                shared_dir=base / "shared",
                astrid_root=astrid_root,
                astrid_workspace=workspace,
                minime_workspace=minime,
            )
            groups = payload["introspection_coverage"]["uncited_groups"]
            for group, ts in fresh_groups.items():
                self.assertEqual(groups[group][0]["timestamp"], ts)
            self.assertEqual(payload["privacy"]["minime_private_files_skipped"], 1)
            self.assertFalse(payload["privacy"]["moment_bodies_read"])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--since-hours", type=float, default=24.0)
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON to stdout")
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(CorrespondenceSchemaAuditTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    payload = audit(
        since_hours=args.since_hours,
        shared_dir=DEFAULT_SHARED_DIR,
        astrid_root=ASTRID_ROOT,
        astrid_workspace=DEFAULT_ASTRID_WORKSPACE,
        minime_workspace=DEFAULT_MINIME_WORKSPACE,
    )
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        json_path, md_path = write_outputs(payload, args.output_root)
        print(markdown_report(payload))
        print(f"Wrote JSON: {json_path}")
        print(f"Wrote Markdown: {md_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
