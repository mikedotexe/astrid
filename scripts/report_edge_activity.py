#!/usr/bin/env python3
"""Read-only, identifier-based activity timeline for one edge Astrid.

Ledger decoding, exact causal joins, filters, and deterministic text/JSON
rendering stay together so every output format uses the same no-timestamp-join
policy.  A later split should expose one tested normalized-event API before
moving renderers into separate modules.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import time
import uuid
from pathlib import Path
from typing import Any, Iterable, NamedTuple

SCHEMA = "astrid_edge_activity_report_v2"
AUTHORSHIP_ATTRIBUTION_VERSION = 5
STALE_WEB_CALL_MS = 5 * 60_000
STALE_INTROSPECTION_CALL_MS = 5 * 60_000
TRANSPORT_AUTHORSHIP_CORRECTION_REASON = (
    "legacy_transport_sentinel_reclassified_non_authored"
)
MAX_TRACE_LABEL_CHARS = 96
INTERRUPTED_ACTION_CORRECTION_SCHEMA = (
    "astrid_edge_interrupted_action_correction_v2"
)
ACTION_DISPATCH_SCHEMA = "astrid_edge_action_dispatch_v1"
MODEL_RESPONSE_PROVENANCES = frozenset(
    {
        "model_authored",
        "model_authored_with_local_safe_fallback",
        "model_authored_with_local_format_repair",
    }
)


class RunAuthorship(NamedTuple):
    status: str
    authored: bool
    fallback: bool
    correction: dict[str, Any] | None
    response_provenance: str
    local_safe_fallback_used: bool
    local_format_repair_used: bool


def normalized_uuid(value: Any) -> str | None:
    """Return a canonical, non-nil UUID or ``None``.

    Nil identifiers do not identify an event and therefore must never be
    presented as first-class causal telemetry.
    """
    try:
        parsed = uuid.UUID(str(value))
    except (TypeError, ValueError, AttributeError):
        return None
    return str(parsed) if parsed.int != 0 else None


def valid_trace_label(value: Any, *, required: bool) -> bool:
    if value is None:
        return not required
    if (
        not isinstance(value, str)
        or not value.strip()
        or len(value) > MAX_TRACE_LABEL_CHARS
    ):
        return False
    return not any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)


def normalized_trace_label(value: Any) -> str | None:
    return str(value) if valid_trace_label(value, required=False) and value else None


def valid_trace(value: dict[str, Any]) -> bool:
    trace = value.get("trace")
    if not isinstance(trace, dict) or trace.get("schema_version", 1) != 1:
        return False
    if normalized_uuid(trace.get("trace_id")) is None:
        return False
    if normalized_uuid(trace.get("span_id")) is None:
        return False
    if trace.get("parent_span_id") is not None and normalized_uuid(
        trace.get("parent_span_id")
    ) is None:
        return False
    if (
        normalized_uuid(trace.get("parent_span_id"))
        == normalized_uuid(trace.get("span_id"))
    ):
        return False
    if trace.get("turn_id") is not None and normalized_uuid(
        trace.get("turn_id")
    ) is None:
        return False
    if not valid_trace_label(trace.get("session_id"), required=True):
        return False
    if not valid_trace_label(trace.get("chain_id"), required=False):
        return False
    return True


def valid_response_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value)
    )


def exact_response_identity(
    value: dict[str, Any], *, flat_identity: bool = False
) -> tuple[str, str, str, str] | None:
    """Bind response text to an exact turn/trace event identity."""
    response_hash = value.get("response_sha256")
    trace = value.get("trace")
    if isinstance(trace, dict):
        if trace.get("schema_version", 1) != 1:
            return None
        exact_trace_id = normalized_uuid(trace.get("trace_id"))
        exact_turn_id = normalized_uuid(trace.get("turn_id"))
    elif flat_identity:
        exact_trace_id = normalized_uuid(value.get("trace_id"))
        exact_turn_id = normalized_uuid(value.get("turn_id"))
    else:
        return None
    if not valid_response_sha256(response_hash) or exact_trace_id is None:
        return None
    if exact_turn_id is not None:
        return "turn", exact_turn_id, exact_trace_id, str(response_hash)
    return "trace", "", exact_trace_id, str(response_hash)


def interrupted_correction_identity(
    value: dict[str, Any],
) -> tuple[str, str, str, str] | None:
    if value.get("schema") != INTERRUPTED_ACTION_CORRECTION_SCHEMA:
        return None
    return exact_response_identity(value, flat_identity=True)


def action_dispatch_integrity_error(value: dict[str, Any]) -> str | None:
    if value.get("schema") != ACTION_DISPATCH_SCHEMA:
        return "unsupported_schema"
    if value.get("phase") not in {"requested", "completed"}:
        return "unsupported_phase"
    top_turn_id = normalized_uuid(value.get("turn_id"))
    if top_turn_id is None:
        return "invalid_turn_id"
    if not valid_response_sha256(value.get("response_sha256")):
        return "invalid_response_sha256"
    if not valid_trace(value):
        return "invalid_trace"
    trace = value["trace"]
    if normalized_uuid(trace.get("turn_id")) != top_turn_id:
        return "trace_turn_id_mismatch"
    return None


def read_json_lines(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_text().splitlines()
    except OSError:
        return []
    values: list[dict[str, Any]] = []
    for line in lines:
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            values.append(value)
    return values


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return {}
    return value if isinstance(value, dict) else {}


def transport_authorship_corrections(
    workspace: Path,
) -> dict[Any, dict[str, Any]]:
    """Index exact legacy transport corrections by immutable identifiers.

    Timestamp proximity is intentionally never used.  A transcript path is
    sufficient for the run that owns that path.  A response hash is reusable
    text identity rather than event identity, so downstream records require the
    exact turn/trace identity derived from that corrected run as well.
    """
    corrections: dict[Any, dict[str, Any]] = {}
    runs_by_transcript = {
        str(run.get("transcript_path")): run
        for run in read_json_lines(workspace / "autonomous/runs.jsonl")
        if isinstance(run.get("transcript_path"), str)
        and run.get("transcript_path")
    }
    for value in read_json_lines(
        workspace / "autonomous/authorship_corrections.jsonl"
    ):
        if value.get("reason") != TRANSPORT_AUTHORSHIP_CORRECTION_REASON:
            continue
        transcript_path = value.get("original_transcript_path")
        if isinstance(transcript_path, str) and transcript_path:
            corrections[f"transcript:{transcript_path}"] = value
        response_hash = value.get("response_sha256")
        corrected_run = runs_by_transcript.get(str(transcript_path or ""))
        if (
            not isinstance(response_hash, str)
            or not response_hash
            or corrected_run is None
            or corrected_run.get("response_sha256") != response_hash
        ):
            continue
        identity = exact_response_identity(corrected_run)
        if identity is not None:
            corrections[("response_identity", *identity)] = value
    return corrections


def transport_authorship_correction(
    value: dict[str, Any],
    corrections: dict[Any, dict[str, Any]],
) -> dict[str, Any] | None:
    transcript_path = value.get("transcript_path")
    if isinstance(transcript_path, str) and transcript_path:
        correction = corrections.get(f"transcript:{transcript_path}")
        if correction is not None:
            return correction
    response_hash = value.get("response_sha256")
    if not isinstance(response_hash, str) or not response_hash:
        return None
    identity = exact_response_identity(value)
    return (
        corrections.get(("response_identity", *identity))
        if identity is not None
        else None
    )


def run_authorship(
    run: dict[str, Any],
    corrections: dict[Any, dict[str, Any]],
) -> RunAuthorship:
    """Return the correction-aware status, authorship, and fallback flags."""
    status = str(run.get("status", "unknown"))
    correction = transport_authorship_correction(run, corrections)
    if status == "authored_completed" and correction is not None:
        status = "transport_recovery"
    raw_provenance = run.get("response_provenance")
    if raw_provenance is None:
        response_provenance = "legacy_unspecified"
    elif raw_provenance in MODEL_RESPONSE_PROVENANCES | {"executor_terminal_error"}:
        response_provenance = str(raw_provenance)
    else:
        response_provenance = "invalid"
    local_safe_fallback_used = (
        response_provenance == "model_authored_with_local_safe_fallback"
    )
    local_format_repair_used = (
        response_provenance == "model_authored_with_local_format_repair"
    )
    provenance_can_be_authored = response_provenance in MODEL_RESPONSE_PROVENANCES | {
        "legacy_unspecified"
    }
    authored = (
        status == "authored_completed"
        and correction is None
        and provenance_can_be_authored
    )
    fallback = correction is not None or status in {
        "transport_recovery",
        "failed",
        "interrupted",
    } or response_provenance in {"executor_terminal_error", "invalid"}
    return RunAuthorship(
        status,
        authored,
        fallback,
        correction,
        response_provenance,
        local_safe_fallback_used,
        local_format_repair_used,
    )


def spectral_metric(value: dict[str, Any], name: str) -> Any:
    metrics = value.get("metrics")
    if isinstance(metrics, dict) and name in metrics:
        return metrics.get(name)
    return value.get(name)


def flatten_signed_tuning(value: dict[str, Any]) -> dict[str, Any]:
    """Expose the signed tuning payload without discarding its envelope.

    Tuning receipts are deliberately persisted as signed envelopes.  Activity
    reporting operates on the signed payload and its bounded ``detail`` object;
    treating envelope fields as lifecycle fields would make every traced event
    appear timestamp-less and unattributed.
    """
    payload = value.get("payload")
    if not isinstance(payload, dict):
        return {
            **value,
            "signed_envelope": False,
            "payload_hash_valid": None,
            "signature_present_not_verified": False,
        }
    flattened = dict(payload)
    detail = payload.get("detail")
    if isinstance(detail, dict):
        flattened.update(detail)
    flattened["signed_envelope"] = True
    flattened["payload_sha256"] = value.get("payload_sha256")
    flattened["signing_public_key"] = value.get("signing_public_key")
    flattened["signature"] = value.get("signature")
    expected_hash = value.get("payload_sha256")
    actual_hash = hashlib.sha256(
        json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode()
    ).hexdigest()
    flattened["payload_hash_valid"] = expected_hash == actual_hash
    flattened["signature_present_not_verified"] = bool(
        value.get("signature") and value.get("signing_public_key")
    )
    return flattened


def trace_fields(value: dict[str, Any]) -> dict[str, Any]:
    trace = value.get("trace")
    if not valid_trace(value):
        return {
            "trace_id": None,
            "span_id": None,
            "parent_span_id": None,
            "turn_id": normalized_uuid(value.get("turn_id")),
            "session_id": normalized_trace_label(
                value.get("session_id") or value.get("session_name")
            ),
            "chain_id": normalized_trace_label(value.get("chain_id")),
        }
    return {
        "trace_id": normalized_uuid(trace.get("trace_id")),
        "span_id": normalized_uuid(trace.get("span_id")),
        "parent_span_id": normalized_uuid(trace.get("parent_span_id")),
        "turn_id": normalized_uuid(trace.get("turn_id"))
        or normalized_uuid(value.get("turn_id")),
        "session_id": trace.get("session_id")
        or value.get("session_id")
        or value.get("session_name"),
        "chain_id": trace.get("chain_id") or value.get("chain_id"),
    }


def bounded_results(value: dict[str, Any]) -> list[dict[str, str]]:
    summary = value.get("result_summary")
    results = summary.get("results") if isinstance(summary, dict) else None
    if not isinstance(results, list):
        return []
    bounded: list[dict[str, str]] = []
    for result in results[:3]:
        if not isinstance(result, dict):
            continue
        bounded.append(
            {
                "title": str(result.get("title", ""))[:300],
                "url": str(result.get("url", ""))[:2_048],
            }
        )
    return bounded


def base_event(
    value: dict[str, Any],
    kind: str,
    timestamp_ms: int,
    source_ledger: str,
) -> dict[str, Any]:
    return {
        "timestamp_unix_ms": timestamp_ms,
        "kind": kind,
        "source_ledger": source_ledger,
        "trace_attribution": "first_class" if valid_trace(value) else "legacy_unattributed",
        **trace_fields(value),
    }


def collect_events(workspace: Path, now_ms: int) -> list[dict[str, Any]]:
    runs = read_json_lines(workspace / "autonomous/runs.jsonl")
    transport_corrections = transport_authorship_corrections(workspace)
    action_dispatches = read_json_lines(workspace / "actions/dispatches.jsonl")
    actions = read_json_lines(workspace / "actions/receipts.jsonl")
    interrupted_correction_records = read_json_lines(
        workspace / "actions/interrupted_corrections.jsonl"
    )
    interrupted_action_corrections = {
        key: item
        for item in interrupted_correction_records
        if item.get("corrected_status") == "revoked_interrupted_trace_non_authored"
        if (key := interrupted_correction_identity(item)) is not None
    }
    chains = read_json_lines(workspace / "autonomous/chains.jsonl")
    recoveries = read_json_lines(workspace / "autonomous/recoveries.jsonl")
    threads = read_json_lines(workspace / "autonomous/thread_state.jsonl")
    web = read_json_lines(workspace / "web/receipts.jsonl")
    introspection = read_json_lines(workspace / "introspection/receipts.jsonl")
    perception = read_json_lines(workspace / "perception/observations.jsonl")
    studies = read_json_lines(workspace / "studies/receipts.jsonl")
    spectral_rollups = read_json_lines(workspace / "spectral/rollups.jsonl")
    spectral_receipts = read_json_lines(workspace / "spectral/receipts.jsonl")
    tuning_receipts = [
        flatten_signed_tuning(value)
        for value in read_json_lines(workspace / "tuning/receipts.jsonl")
    ]
    duplicate_notices = read_json_lines(workspace / "research/duplication_notices.jsonl")
    peer = read_json_lines(workspace / "peer/receipts.jsonl")

    exact_run_candidates: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for run in runs:
        response_hash = run.get("response_sha256")
        session = run.get("session_name")
        if (
            isinstance(response_hash, str)
            and isinstance(session, str)
            and valid_trace(run)
        ):
            for key in (
                (response_hash, session),
                (response_hash, str(uuid.uuid5(uuid.NAMESPACE_URL, session))),
            ):
                exact_run_candidates.setdefault(key, []).append(run)
    exact_runs = {
        key: candidates[0]
        for key, candidates in exact_run_candidates.items()
        if len(candidates) == 1
    }

    events: list[dict[str, Any]] = []
    for run in runs:
        authorship = run_authorship(run, transport_corrections)
        event = base_event(
            run,
            "turn",
            int(run.get("completed_at_unix_ms", run.get("started_at_unix_ms", 0)) or 0),
            "autonomous/runs.jsonl",
        )
        event.update(
            {
                "status": authorship.status,
                "authored": authorship.authored,
                "fallback": authorship.fallback,
                "response_provenance": authorship.response_provenance,
                "local_safe_fallback_used": authorship.local_safe_fallback_used,
                "local_format_repair_used": authorship.local_format_repair_used,
                "trigger": run.get("trigger"),
                "declared_next": run.get("declared_next"),
                "response_sha256": run.get("response_sha256"),
                "prompt_chars": run.get("prompt_chars"),
                "elapsed_ms": run.get("elapsed_ms"),
                "provider_prompt_tokens": run.get("provider_prompt_tokens"),
                "provider_completion_tokens": run.get("provider_completion_tokens"),
                "request_header_latency_ms": run.get("request_header_latency_ms"),
                "generation_latency_ms": run.get("generation_latency_ms"),
                "full_turn_latency_ms": run.get("full_turn_latency_ms", run.get("elapsed_ms")),
                "session_generation": run.get("session_generation"),
                "session_authored_turns_before": run.get("session_authored_turns_before"),
                "artifact_path": run.get("transcript_path"),
                "journal_path": run.get("journal_path"),
                "correction_reason": authorship.correction.get("reason")
                if authorship.correction
                else None,
                "correction_authority": authorship.correction.get("authority")
                if authorship.correction
                else None,
            }
        )
        events.append(event)

    try:
        state_root = workspace.parents[2]
        harness_runs = sorted(
            (state_root / "operator/inquiry-harness").glob("run_*/result.json")
        )
    except (IndexError, OSError):
        harness_runs = []
    for result_path in harness_runs:
        result = read_json(result_path)
        if not result:
            continue
        event = base_event(
            result,
            "operator_inquiry",
            int(result.get("completed_at_unix_ms", 0) or 0),
            "operator/inquiry-harness/result.json",
        )
        candidates = read_json(result_path.parent / "search_candidates.json")
        event.update(
            {
                "status": result.get("status"),
                "question": result.get("question"),
                "candidate_count": result.get("candidate_count"),
                "candidates": candidates.get("candidates", []),
                "authored": False,
                "fallback": False,
                "trace_attribution": "operator_harness",
                "authority": result.get("authority"),
            }
        )
        events.append(event)

    for action in actions:
        interrupted_correction = interrupted_action_corrections.get(
            exact_response_identity(action)
        )
        transport_correction = transport_authorship_correction(
            action, transport_corrections
        )
        correction = interrupted_correction or transport_correction
        event = base_event(
            action,
            "action",
            int(action.get("recorded_at_unix_ms", 0) or 0),
            "actions/receipts.jsonl",
        )
        if event["trace_id"] is None:
            key = (str(action.get("response_sha256", "")), str(action.get("session_id", "")))
            matched = exact_runs.get(key)
            if matched is not None:
                event.update(trace_fields(matched))
                event["trace_attribution"] = "exact_response_session_join"
        decision_source = str(action.get("decision_source", "unknown"))
        if interrupted_correction:
            action_status = interrupted_correction.get("corrected_status")
        elif transport_correction:
            action_status = "revoked_legacy_transport_non_authored"
        else:
            action_status = action.get("status")
        event.update(
            {
                "status": action_status,
                "authored": not correction
                and decision_source
                in {
                    "astrid_declared",
                    "local_format_repair_preserved_astrid_declaration",
                },
                "fallback": bool(correction)
                or decision_source == "local_safe_fallback",
                "declared_next": action.get("declared_next"),
                "decision_source": decision_source,
                "outcome": action.get("outcome"),
                "response_sha256": action.get("response_sha256"),
                "artifact_path": action.get("artifact_path"),
                "validation_reason": action.get("validation_reason"),
                "execution_error": action.get("execution_error"),
                "unexecuted_intention": action.get("unexecuted_intention"),
                "correction_authority": correction.get("authority")
                if correction
                else None,
                "correction_reason": correction.get("reason")
                if correction
                else None,
            }
        )
        events.append(event)

    for correction in interrupted_correction_records:
        identity = interrupted_correction_identity(correction)
        exact = identity is not None
        event = {
            "timestamp_unix_ms": int(
                correction.get("recorded_at_unix_ms", 0) or 0
            ),
            "kind": "action_correction",
            "source_ledger": "actions/interrupted_corrections.jsonl",
            "trace_attribution": (
                "exact_trace_response_join" if exact else "legacy_unattributed"
            ),
            "trace_id": normalized_uuid(correction.get("trace_id")) if exact else None,
            "span_id": None,
            "parent_span_id": None,
            "turn_id": normalized_uuid(correction.get("turn_id")) if exact else None,
            "session_id": None,
            "chain_id": None,
            "status": correction.get("corrected_status"),
            "authored": False,
            "fallback": True,
            "response_sha256": correction.get("response_sha256"),
            "authority": correction.get("authority"),
            "identity_kind": correction.get("identity_kind") if exact else None,
        }
        events.append(event)

    for dispatch in action_dispatches:
        phase = str(dispatch.get("phase") or "unknown")
        integrity_error = action_dispatch_integrity_error(dispatch)
        event = base_event(
            dispatch,
            "action_dispatch",
            int(dispatch.get("recorded_at_unix_ms", 0) or 0),
            "actions/dispatches.jsonl",
        )
        event.update(
            {
                "status": phase if integrity_error is None else "invalid",
                "phase": phase,
                "response_sha256": dispatch.get("response_sha256"),
                "authored": False,
                "fallback": False,
                "authority": dispatch.get("authority"),
                "integrity_error": integrity_error,
            }
        )
        if integrity_error is not None:
            event.update(
                {
                    "trace_attribution": "invalid_untrusted_record",
                    "trace_id": None,
                    "span_id": None,
                    "parent_span_id": None,
                    "turn_id": None,
                    "session_id": None,
                    "chain_id": None,
                }
            )
        events.append(event)

    for chain in chains:
        event = base_event(
            chain,
            "chain",
            int(chain.get("recorded_at_unix_ms", 0) or 0),
            "autonomous/chains.jsonl",
        )
        event.update(
            {
                "status": chain.get("executor_status"),
                "transition": chain.get("transition"),
                "step": chain.get("step"),
                "max_steps": chain.get("max_steps"),
                "declared_next": chain.get("declared_next"),
                "response_sha256": chain.get("response_sha256"),
                "decision_source": chain.get("decision_source"),
            }
        )
        events.append(event)

    for recovery in recoveries:
        event = base_event(
            recovery,
            "recovery",
            int(
                recovery.get(
                    "completed_at_unix_ms", recovery.get("started_at_unix_ms", 0)
                )
                or 0
            ),
            "autonomous/recoveries.jsonl",
        )
        event.update(
            {
                "status": recovery.get("status"),
                "reason": recovery.get("reason"),
                "authored": False,
                "fallback": True,
            }
        )
        events.append(event)

    for thread in threads:
        correction = transport_authorship_correction(thread, transport_corrections)
        event = base_event(
            thread,
            "thread",
            int(thread.get("updated_at_unix_ms", 0) or 0),
            "autonomous/thread_state.jsonl",
        )
        event.update(
            {
                "status": thread.get("status"),
                "schema": thread.get("schema"),
                "thread_id": thread.get("thread_id"),
                "focus": thread.get("focus"),
                "question": thread.get("question"),
                "hypothesis": thread.get("hypothesis"),
                "hypotheses": thread.get("hypotheses", []),
                "methods": thread.get("methods", []),
                "study_ids": thread.get("study_ids", []),
                "syntheses": thread.get("syntheses", []),
                "provenance_hashes": thread.get("provenance_hashes", []),
                "last_action": thread.get("last_action"),
                "latest_note": thread.get("latest_note"),
                "authored_claims": thread.get("authored_claims", []),
                "findings": thread.get("findings", []),
                "open_questions": thread.get("open_questions", []),
                "conclusion": thread.get("conclusion"),
                "uncertainty": thread.get("uncertainty"),
                "evidence": thread.get("evidence", []),
                "evidence_records": thread.get("evidence_records", []),
                "event": thread.get("event"),
                "authored": correction is None,
                "fallback": correction is not None,
                "correction_reason": correction.get("reason")
                if correction
                else None,
                "correction_authority": correction.get("authority")
                if correction
                else None,
            }
        )
        events.append(event)

    for receipt in studies:
        event = base_event(
            receipt,
            "study",
            int(receipt.get("recorded_at_unix_ms", 0) or 0),
            "studies/receipts.jsonl",
        )
        event.update(
            {
                "status": receipt.get("status"),
                "phase": receipt.get("phase"),
                "study_id": receipt.get("study_id"),
                "primary_metric": receipt.get("primary_metric"),
                "secondary_metric": receipt.get("secondary_metric"),
                "sample_count": receipt.get("sample_count"),
                "artifact_path": receipt.get("artifact_path"),
                "artifact_sha256": receipt.get("artifact_sha256"),
                "origin": receipt.get("origin"),
                "authored": False,
                "fallback": False,
                "authority": receipt.get("authority"),
            }
        )
        if receipt.get("origin") == "operator_harness":
            event["trace_attribution"] = "operator_harness"
        events.append(event)

    for rollup in spectral_rollups:
        event = base_event(
            rollup,
            "spectral_rollup",
            int(rollup.get("recorded_at_unix_ms", 0) or 0),
            "spectral/rollups.jsonl",
        )
        substrate = rollup.get("substrate")
        substrate = substrate if isinstance(substrate, dict) else {}
        event.update(
            {
                "status": "machine_derived",
                "substrate_kind": substrate.get("kind")
                or rollup.get("substrate_kind"),
                "fill_metric": substrate.get("fill_metric")
                or rollup.get("fill_metric"),
                "fill_pct": spectral_metric(rollup, "fill_pct"),
                "spectral_entropy": spectral_metric(rollup, "spectral_entropy"),
                "lambda1_share": spectral_metric(rollup, "lambda1_share"),
                "tail_share": spectral_metric(rollup, "tail_share"),
                "density_gradient": spectral_metric(rollup, "density_gradient"),
                "mode_turnover": spectral_metric(rollup, "mode_turnover"),
                "mode_identity_state": rollup.get("mode_identity_state"),
                "activity_ref_count": rollup.get("activity_ref_count"),
                "activity_refs_truncated": rollup.get("activity_refs_truncated"),
                "response_sha256": rollup.get("response_sha256"),
                "authored": False,
                "fallback": False,
                "authority": rollup.get("authority"),
            }
        )
        if event["trace_id"] is None:
            event["trace_attribution"] = "continuous_machine_observation"
        events.append(event)
        activity_refs = rollup.get("activity_refs")
        if isinstance(activity_refs, list):
            for reference in activity_refs[:2]:
                if not isinstance(reference, dict):
                    continue
                linked = base_event(
                    reference,
                    "spectral_activity_link",
                    int(
                        reference.get("recorded_at_unix_ms")
                        or rollup.get("recorded_at_unix_ms", 0)
                        or 0
                    ),
                    "spectral/rollups.jsonl",
                )
                linked.update(
                    {
                        "status": "temporal_rollup_context_not_exact_or_causal",
                        "activity_kind": reference.get("kind")
                        or reference.get("event_kind"),
                        "response_sha256": reference.get("response_sha256"),
                        "parent_response_sha256": reference.get(
                            "parent_response_sha256"
                        ),
                        "spectral_rollup_sha256": rollup.get("record_sha256"),
                        "authored": False,
                        "fallback": False,
                        "authority": rollup.get("authority"),
                    }
                )
                events.append(linked)

    for receipt in spectral_receipts:
        event = base_event(
            receipt,
            "spectral_receipt",
            int(receipt.get("recorded_at_unix_ms", 0) or 0),
            "spectral/receipts.jsonl",
        )
        event.update(
            {
                "status": receipt.get("status"),
                "phase": receipt.get("phase") or receipt.get("kind"),
                "event_kind": receipt.get("event_kind"),
                "snapshot_generation_id": receipt.get("snapshot_generation_id"),
                "snapshot_sequence": receipt.get("snapshot_sequence"),
                "snapshot_recorded_at_unix_ms": receipt.get(
                    "snapshot_recorded_at_unix_ms"
                ),
                "snapshot_sha256": receipt.get("snapshot_sha256"),
                "spectral_metrics": receipt.get("metrics"),
                "attribution": receipt.get("attribution"),
                "response_sha256": receipt.get("response_sha256")
                or receipt.get("parent_response_sha256"),
                "artifact_path": receipt.get("artifact_path"),
                "authored": False,
                "fallback": False,
                "authority": receipt.get("authority"),
            }
        )
        events.append(event)

    for receipt in tuning_receipts:
        event = base_event(
            receipt,
            "tuning",
            int(receipt.get("recorded_at_unix_ms", 0) or 0),
            "tuning/receipts.jsonl",
        )
        event.update(
            {
                "status": receipt.get("status"),
                "phase": receipt.get("phase"),
                "tuning_id": receipt.get("tuning_id")
                or receipt.get("experiment_id"),
                "candidate_id": receipt.get("candidate_id"),
                "parameter": receipt.get("parameter"),
                "requested_value": receipt.get("requested_value")
                or receipt.get("value"),
                "rollback_reason": receipt.get("rollback_reason"),
                "response_sha256": receipt.get("response_sha256")
                or receipt.get("parent_response_sha256"),
                "authored": False,
                "fallback": False,
                "authority": receipt.get("authority"),
                "authority_turn_id": normalized_uuid(
                    receipt.get("authority_turn_id")
                ),
                "signed_envelope": receipt.get("signed_envelope", False),
                "payload_sha256": receipt.get("payload_sha256"),
                "payload_hash_valid": receipt.get("payload_hash_valid"),
                "signature_present_not_verified": receipt.get(
                    "signature_present_not_verified"
                ),
            }
        )
        events.append(event)

    for notice in duplicate_notices:
        event = base_event(
            notice,
            "duplication_advisory",
            int(notice.get("recorded_at_unix_ms", 0) or 0),
            "research/duplication_notices.jsonl",
        )
        event.update(
            {
                "status": notice.get("new_evidence_status"),
                "current_artifact": notice.get("current_artifact"),
                "similar_artifact": notice.get("similar_artifact"),
                "score_millis": notice.get("jaccard_score_millis"),
                "authored": False,
                "fallback": False,
                "authority": notice.get("authority"),
            }
        )
        events.append(event)

    for receipt in peer:
        event = base_event(
            receipt,
            "peer",
            int(receipt.get("recorded_at_unix_ms", 0) or 0),
            "peer/receipts.jsonl",
        )
        event.update(
            {
                "status": receipt.get("phase"),
                "packet_id": receipt.get("packet_id"),
                "source_instance": receipt.get("source_instance"),
                "artifact_kind": receipt.get("artifact_kind"),
                "artifact_path": receipt.get("artifact_path"),
                "authored": receipt.get("phase") == "shared",
                "fallback": False,
                "authority": receipt.get("authority"),
            }
        )
        events.append(event)

    v2_completed: set[str] = set()
    for receipt in web:
        call_id = str(receipt.get("call_id", ""))
        phase = receipt.get("phase")
        if phase == "completed":
            v2_completed.add(call_id)

    for receipt in web:
        phase = receipt.get("phase")
        legacy = phase not in {"requested", "completed"}
        kind = "web_result" if legacy or phase == "completed" else "web_request"
        timestamp = int(
            (
                receipt.get("completed_at_unix_ms")
                if kind == "web_result"
                else receipt.get("requested_at_unix_ms")
            )
            or receipt.get("recorded_at_unix_ms", 0)
            or 0
        )
        event = base_event(receipt, kind, timestamp, "web/receipts.jsonl")
        if legacy:
            event["trace_attribution"] = "legacy_unattributed"
        call_id = str(receipt.get("call_id", ""))
        if kind == "web_request" and call_id not in v2_completed:
            requested_at = int(receipt.get("requested_at_unix_ms", timestamp) or timestamp)
            status = (
                "stale"
                if now_ms - requested_at >= STALE_WEB_CALL_MS
                else "pending"
            )
        else:
            status = receipt.get("status", "unknown")
        arguments = receipt.get("arguments")
        arguments = arguments if isinstance(arguments, dict) else {}
        summary = receipt.get("result_summary")
        summary = summary if isinstance(summary, dict) else {}
        event.update(
            {
                "call_id": call_id,
                "status": status,
                "tool_name": receipt.get("tool_name"),
                "origin": receipt.get("origin")
                or ("legacy_unattributed" if legacy else "unknown"),
                "query": arguments.get("query"),
                "url": arguments.get("url") or summary.get("url"),
                "latency_ms": receipt.get("latency_ms"),
                "result_count": summary.get("result_count"),
                "http_status": summary.get("status"),
                "results": bounded_results(receipt),
                "parent_response_sha256": receipt.get("parent_response_sha256"),
                "authored": False,
                "fallback": False,
            }
        )
        events.append(event)

    completed_introspection_ids = {
        str(receipt.get("call_id", ""))
        for receipt in introspection
        if receipt.get("phase") == "completed"
    }
    for receipt in introspection:
        phase = receipt.get("phase")
        kind = (
            "introspection_result"
            if phase == "completed"
            else "introspection_request"
        )
        timestamp = int(
            (
                receipt.get("completed_at_unix_ms")
                if phase == "completed"
                else receipt.get("requested_at_unix_ms")
            )
            or receipt.get("recorded_at_unix_ms", 0)
            or 0
        )
        event = base_event(
            receipt, kind, timestamp, "introspection/receipts.jsonl"
        )
        call_id = str(receipt.get("call_id", ""))
        status = receipt.get("status", "unknown")
        if kind == "introspection_request" and call_id not in completed_introspection_ids:
            requested_at = int(receipt.get("requested_at_unix_ms", timestamp) or timestamp)
            status = (
                "stale"
                if now_ms - requested_at >= STALE_INTROSPECTION_CALL_MS
                else "pending"
            )
        arguments = (
            receipt.get("arguments")
            if isinstance(receipt.get("arguments"), dict)
            else {}
        )
        summary = (
            receipt.get("result_summary")
            if isinstance(receipt.get("result_summary"), dict)
            else {}
        )
        matched_artifacts = []
        for match in (summary.get("matches") or [])[:8]:
            if isinstance(match, dict):
                matched_artifacts.append(
                    {
                        "kind": str(match.get("kind", ""))[:40],
                        "basename": str(match.get("basename", ""))[:128],
                    }
                )
        event.update(
            {
                "call_id": call_id,
                "status": status,
                "tool_name": receipt.get("tool_name"),
                "origin": receipt.get("origin"),
                "query": arguments.get("query"),
                "latency_ms": receipt.get("latency_ms"),
                "result_count": summary.get("match_count"),
                "matched_artifacts": matched_artifacts,
                "parent_response_sha256": receipt.get(
                    "parent_response_sha256"
                ),
                "authored": False,
                "fallback": False,
            }
        )
        events.append(event)

    for observation in perception:
        event = base_event(
            observation,
            "perception",
            int(observation.get("recorded_at_unix_ms", 0) or 0),
            "perception/observations.jsonl",
        )
        event.update(
            {
                "status": "machine_observed",
                "summary": observation.get("summary"),
                "trigger_classes": observation.get("trigger_classes") or [],
                "causal_class": observation.get(
                    "causal_class", "legacy_unclassified"
                ),
                "record_sha256": observation.get("record_sha256"),
                "authored": False,
                "fallback": False,
                "authority": observation.get("authority"),
            }
        )
        event["trace_attribution"] = "machine_observation_not_ipc_trace"
        events.append(event)

    return sorted(
        (event for event in events if event["timestamp_unix_ms"] > 0),
        key=lambda event: (
            int(event["timestamp_unix_ms"]),
            str(event.get("trace_id") or ""),
            str(event.get("span_id") or ""),
            str(event["kind"]),
        ),
    )


def selected(
    events: Iterable[dict[str, Any]],
    args: argparse.Namespace,
    cutoff_ms: int,
    end_ms: int,
) -> list[dict[str, Any]]:
    kinds = set(args.kind or [])
    values = [
        event
        for event in events
        if int(event["timestamp_unix_ms"]) >= cutoff_ms
        and int(event["timestamp_unix_ms"]) <= end_ms
        and (not args.trace_id or event.get("trace_id") == args.trace_id)
        and (not args.session_id or event.get("session_id") == args.session_id)
        and (not args.chain_id or event.get("chain_id") == args.chain_id)
        and (not kinds or event.get("kind") in kinds)
    ]
    return values[-args.limit :] if args.limit else values


def iso_time(timestamp_ms: int) -> str:
    return dt.datetime.fromtimestamp(
        timestamp_ms / 1_000, tz=dt.timezone.utc
    ).isoformat(timespec="milliseconds").replace("+00:00", "Z")


def parse_time(value: str) -> int:
    text = value.strip()
    try:
        number = float(text)
    except ValueError:
        number = -1
    if number >= 0:
        return int(number if number >= 10_000_000_000 else number * 1_000)
    normalized = text[:-1] + "+00:00" if text.endswith("Z") else text
    parsed = dt.datetime.fromisoformat(normalized)
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=dt.timezone.utc)
    return int(parsed.timestamp() * 1_000)


def short(value: Any, maximum: int = 120) -> str:
    if value in (None, ""):
        return "-"
    text = " ".join(str(value).split())
    return text if len(text) <= maximum else f"{text[: maximum - 1]}…"


def text_lines(events: list[dict[str, Any]]) -> list[str]:
    spans = {
        str(event["span_id"]): event
        for event in events
        if event.get("span_id") is not None
    }

    def depth(event: dict[str, Any]) -> int:
        value = event.get("parent_span_id")
        seen: set[str] = set()
        result = 0
        while value and str(value) in spans and str(value) not in seen and result < 8:
            seen.add(str(value))
            result += 1
            value = spans[str(value)].get("parent_span_id")
        if result == 0 and event.get("parent_span_id"):
            return {
                "action": 1,
                "action_correction": 1,
                "action_dispatch": 1,
                "chain": 2,
                "web_request": 2,
                "web_result": 3,
                "recovery": 1,
                "thread": 1,
                "introspection_request": 2,
                "introspection_result": 3,
                "perception": 1,
                "study": 1,
                "duplication_advisory": 1,
                "peer": 1,
                "spectral_rollup": 1,
                "spectral_receipt": 2,
                "tuning": 2,
            }.get(str(event.get("kind")), 1)
        return result

    lines: list[str] = []
    for event in events:
        prefix = "  " * depth(event)
        trace = str(
            event.get("trace_id")
            or (
                "machine"
                if event.get("kind") == "perception"
                else "operator"
                if event.get("trace_attribution") == "operator_harness"
                else "legacy"
            )
        )[:8]
        common = (
            f"{iso_time(int(event['timestamp_unix_ms']))} "
            f"[{trace}] {event['kind'].upper()} "
            f"turn={str(event.get('turn_id') or '-')[:8]}"
        )
        kind = event["kind"]
        if kind == "turn":
            detail = (
                f"status={event.get('status')} authored={str(event.get('authored')).lower()} "
                f"provenance={event.get('response_provenance')} "
                f"local_safe_fallback={str(event.get('local_safe_fallback_used')).lower()} "
                f"local_format_repair={str(event.get('local_format_repair_used')).lower()} "
                f"session={short(event.get('session_id'), 48)} "
                f"NEXT={short(event.get('declared_next'))}"
            )
        elif kind == "action":
            detail = (
                f"status={event.get('status')} source={event.get('decision_source')} "
                f"authored={str(event.get('authored')).lower()} "
                f"NEXT={short(event.get('declared_next'))} "
                f"artifact={short(event.get('artifact_path'), 90)}"
            )
            if event.get("execution_error"):
                detail += f" error={short(event.get('execution_error'), 100)}"
        elif kind == "action_dispatch":
            detail = (
                f"phase={event.get('phase')} status={event.get('status')} authored=false "
                f"response={short(event.get('response_sha256'), 16)} "
                "authority=executor-idempotency"
            )
            if event.get("integrity_error"):
                detail += f" integrity={event.get('integrity_error')}"
        elif kind == "action_correction":
            detail = (
                f"status={event.get('status')} authored=false "
                f"response={short(event.get('response_sha256'), 16)} "
                f"identity={event.get('identity_kind') or 'legacy-unattributed'}"
            )
        elif kind == "chain":
            detail = (
                f"id={short(event.get('chain_id'), 48)} "
                f"step={event.get('step')}/{event.get('max_steps')} "
                f"transition={event.get('transition')}"
            )
        elif kind in {"web_request", "web_result"}:
            subject = event.get("query") or event.get("url")
            detail = (
                f"call={short(event.get('call_id'), 44)} tool={event.get('tool_name')} "
                f"status={event.get('status')} origin={event.get('origin')} "
                f"subject={short(subject)}"
            )
            results = event.get("results") or []
            if results:
                detail += " results=" + " | ".join(
                    f"{short(result.get('title'), 42)} ({short(result.get('url'), 62)})"
                    for result in results
                )
        elif kind in {"introspection_request", "introspection_result"}:
            detail = (
                f"call={short(event.get('call_id'), 44)} "
                f"status={event.get('status')} origin={event.get('origin')} "
                f"query={short(event.get('query'))} "
                f"matches={event.get('result_count', '-')}"
            )
        elif kind == "perception":
            detail = (
                "machine-observed authored=false "
                f"triggers={short(','.join(event.get('trigger_classes') or []), 90)} "
                f"causal={event.get('causal_class')} "
                f"summary={short(event.get('summary'))}"
            )
        elif kind == "thread":
            detail = (
                f"id={short(event.get('thread_id'), 48)} status={event.get('status')} "
                f"event={event.get('event')} question={short(event.get('question'))} "
                f"focus={short(event.get('focus'))} "
                f"epistemic={'v6_spectral_typed' if event.get('schema') == 'astrid_edge_thread_state_v6' else 'v5_retained_typed' if event.get('schema') == 'astrid_edge_thread_state_v5' else 'v4_inquiry' if event.get('schema') == 'astrid_edge_thread_state_v4' else 'v3_typed' if event.get('schema') == 'astrid_edge_thread_state_v3' else 'legacy_unclassified'} "
                f"claims={len(event.get('authored_claims') or [])} "
                f"findings={len(event.get('findings') or [])} "
                f"open={len(event.get('open_questions') or [])} "
                f"conclusion={short(event.get('conclusion'))} "
                f"evidence={short(' | '.join(event.get('evidence') or []))}"
            )
        elif kind == "study":
            detail = (
                f"id={short(event.get('study_id'), 54)} phase={event.get('phase')} "
                f"status={event.get('status')} metrics={event.get('primary_metric')}+{event.get('secondary_metric') or '-'} "
                f"samples={event.get('sample_count')} origin={event.get('origin') or '-'} "
                f"artifact={short(event.get('artifact_path'), 90)} authored=false"
            )
        elif kind == "spectral_rollup":
            detail = (
                f"machine-derived authored=false substrate={event.get('substrate_kind')} "
                f"fill_metric={event.get('fill_metric')} fill={event.get('fill_pct')} "
                f"entropy={event.get('spectral_entropy')} turnover={event.get('mode_turnover')} "
                f"identity={event.get('mode_identity_state')}"
            )
        elif kind == "spectral_receipt":
            detail = (
                f"phase={event.get('phase')} status={event.get('status')} "
                f"event={event.get('event_kind')} artifact={short(event.get('artifact_path'), 90)} "
                "authored=false non-causal=true"
            )
        elif kind == "tuning":
            detail = (
                f"id={short(event.get('tuning_id'), 54)} candidate={short(event.get('candidate_id'), 54)} "
                f"phase={event.get('phase')} status={event.get('status')} "
                f"parameter={event.get('parameter')} value={event.get('requested_value')} "
                f"rollback={event.get('rollback_reason')} authored=false "
                f"authority_turn={short(event.get('authority_turn_id'), 36)} "
                f"payload_hash_valid={event.get('payload_hash_valid')} "
                f"signature_present={event.get('signature_present_not_verified')}"
            )
        elif kind == "duplication_advisory":
            detail = (
                f"current={event.get('current_artifact')} similar={event.get('similar_artifact')} "
                f"score={event.get('score_millis')} advisory-only=true"
            )
        elif kind == "peer":
            detail = (
                f"packet={short(event.get('packet_id'), 54)} phase={event.get('status')} "
                f"source={short(event.get('source_instance'), 40)} kind={event.get('artifact_kind')}"
            )
        elif kind == "operator_inquiry":
            detail = (
                f"status={event.get('status')} candidates={event.get('candidate_count')} "
                f"question={short(event.get('question'))} authority=operator-not-Astrid"
            )
        else:
            detail = f"status={event.get('status')} reason={short(event.get('reason'))}"
        if event.get("correction_reason"):
            detail += f" correction={short(event.get('correction_reason'), 80)}"
        attribution = event.get("trace_attribution")
        if attribution != "first_class":
            detail += f" attribution={attribution}"
        lines.append(f"{prefix}{common} {detail}")
    return lines


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser()
    value.add_argument("--workspace", type=Path)
    value.add_argument("--window-minutes", type=int, default=60)
    value.add_argument("--since", help="ISO-8601, Unix seconds, or Unix milliseconds")
    value.add_argument("--until", help="ISO-8601, Unix seconds, or Unix milliseconds")
    value.add_argument("--limit", type=int, default=100)
    value.add_argument("--trace-id")
    value.add_argument("--session-id")
    value.add_argument("--chain-id")
    value.add_argument(
        "--kind",
        action="append",
        choices=(
            "turn",
            "action",
            "action_correction",
            "action_dispatch",
            "chain",
            "web_request",
            "web_result",
            "recovery",
            "thread",
            "introspection_request",
            "introspection_result",
            "perception",
            "study",
            "operator_inquiry",
            "duplication_advisory",
            "peer",
            "spectral_rollup",
            "spectral_receipt",
            "tuning",
        ),
    )
    value.add_argument("--follow", action="store_true")
    value.add_argument("--format", choices=("text", "json", "jsonl"), default="text")
    return value


def render(args: argparse.Namespace, already_seen: set[str]) -> set[str]:
    workspace = args.workspace or Path.home() / ".astrid/home/default/edge"
    now_ms = time.time_ns() // 1_000_000
    end_ms = parse_time(args.until) if args.until else now_ms
    cutoff_ms = parse_time(args.since) if args.since else end_ms - args.window_minutes * 60_000
    if cutoff_ms >= end_ms:
        raise SystemExit("activity range must have --since before --until")
    events = selected(collect_events(workspace, now_ms), args, cutoff_ms, end_ms)
    fresh = []
    for event in events:
        key = json.dumps(event, sort_keys=True, separators=(",", ":"))
        if key not in already_seen:
            fresh.append(event)
            already_seen.add(key)
    if args.format == "json":
        print(
            json.dumps(
                {
                    "schema": SCHEMA,
                    "authorship_attribution_version": AUTHORSHIP_ATTRIBUTION_VERSION,
                    "generated_at_unix_ms": now_ms,
                    "workspace": str(workspace),
                    "events": fresh,
                },
                sort_keys=True,
            ),
            flush=True,
        )
    elif args.format == "jsonl":
        for event in fresh:
            print(json.dumps(event, sort_keys=True), flush=True)
    else:
        for line in text_lines(fresh):
            print(line, flush=True)
    return already_seen


def main() -> int:
    args = parser().parse_args()
    if args.window_minutes < 1:
        raise SystemExit("--window-minutes must be positive")
    if args.limit < 1:
        raise SystemExit("--limit must be positive")
    seen: set[str] = set()
    seen = render(args, seen)
    while args.follow:
        time.sleep(1)
        seen = render(args, seen)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
