#!/usr/bin/env python3
"""Project capture-first experiment dossiers over existing sandbox trials."""

from __future__ import annotations

import argparse
from collections import Counter
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import tempfile
import time
from typing import Any

try:
    from evidence_store import EvidenceEventStore, EvidenceStoreError
    from evidence_store.adapter import append_domain_events, read_domain_events
    from evidence_store.model import canonical_json
except ModuleNotFoundError:
    from scripts.evidence_store import EvidenceEventStore, EvidenceStoreError
    from scripts.evidence_store.adapter import append_domain_events, read_domain_events
    from scripts.evidence_store.model import canonical_json

try:
    from projection_receipt import projector_receipt
    from projection_cursors import ProjectionInputCursor
except ModuleNotFoundError:
    from scripts.projection_receipt import projector_receipt
    from scripts.projection_cursors import ProjectionInputCursor

DEFAULT_WORKSPACE = Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace")
PROJECTOR_VERSION = 2
DOSSIER_STATES = (
    "draft",
    "capture-ready",
    "baseline-captured",
    "candidate-captured",
    "comparison-ready",
    "result-recorded",
    "review-pending",
    "closed",
)
STATE_INDEX = {state: index for index, state in enumerate(DOSSIER_STATES)}
CAPTURE_REF_RE = re.compile(
    r"^(?:sha256:[0-9a-f]{64}|capture:[A-Za-z0-9_.-]+:sha256:[0-9a-f]{64})$"
)


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/experiment_dossiers_v1"


def family_state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/claim_families_v1"


def sandbox_status_path(workspace: Path) -> Path:
    return workspace / "diagnostics/sandbox_trial_queue_v1/status.json"


def family_status_path(workspace: Path) -> Path:
    return workspace / "diagnostics/claim_families_v1/status.json"


def lived_state_context_path(workspace: Path) -> Path:
    return workspace / "diagnostics/lived_state_witness_v1/context_index.jsonl"


def lived_state_context_index(workspace: Path) -> dict[str, dict[str, Any]]:
    path = lived_state_context_path(workspace)
    result: dict[str, dict[str, Any]] = {}
    if not path.is_file():
        return result
    for raw in path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            row = json.loads(raw)
        except json.JSONDecodeError:
            continue
        if not isinstance(row, dict):
            continue
        introspection_id = str(row.get("introspection_id") or "")
        witness_id = str(row.get("witness_id") or "")
        if introspection_id and witness_id:
            result[introspection_id] = {
                "witness_id": witness_id,
                "introspection_id": introspection_id,
                "alignment": row.get("alignment"),
                "gap_count": int(row.get("gap_count") or 0),
                "reconciliation_ref": row.get("reconciliation_ref"),
                "state_transition_implied": False,
                "authority_propagated": False,
                "raw_prose_included": False,
            }
    return result


def authority_state(state: str) -> dict[str, Any]:
    if state not in {"evidence_only", "approval_pending"}:
        raise ValueError(f"invalid dossier authority state: {state}")
    return {
        "schema": "artifact_authority_state_v1",
        "schema_version": 1,
        "state": state,
        "live_eligible_now": False,
        "auto_approved": False,
        "grants_approval": False,
        "edits_source_now": False,
    }


def digest(value: Any) -> str:
    return hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest()


def agency_tier_value(value: Any) -> int:
    if isinstance(value, bool):
        return 0
    if isinstance(value, int):
        return value
    match = re.search(r"\d+", str(value or ""))
    return int(match.group()) if match else 0


def load_object(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain an object")
    return value


def claim_family_index(family_status: dict[str, Any]) -> dict[str, str]:
    result: dict[str, str] = {}
    families = family_status.get("families")
    if not isinstance(families, dict):
        return result
    for family_id, family in families.items():
        claims = family.get("claims") if isinstance(family, dict) else None
        if isinstance(claims, dict):
            for claim_id in claims:
                result[str(claim_id)] = str(family_id)
    return result


def intervention_signature(trial: dict[str, Any]) -> str:
    bounded = {
        "adapter": trial.get("adapter"),
        "trial_mode": trial.get("trial_mode"),
        "proposed_intervention": trial.get("proposed_intervention"),
        "agency_tier": trial.get("agency_tier"),
        "surface": (
            trial.get("authority_boundary_packet_v2", {}).get("surface")
            if isinstance(trial.get("authority_boundary_packet_v2"), dict)
            else None
        ),
    }
    return digest(bounded)


def initial_dossier_events(workspace: Path) -> list[dict[str, Any]]:
    sandbox_raw = sandbox_status_path(workspace).read_bytes()
    family_raw = family_status_path(workspace).read_bytes()
    sandbox = json.loads(sandbox_raw)
    family_status = json.loads(family_raw)
    families = claim_family_index(family_status)
    lived_contexts = lived_state_context_index(workspace)
    lived_context_raw = (
        lived_state_context_path(workspace).read_bytes()
        if lived_state_context_path(workspace).is_file()
        else b""
    )
    trials = sandbox.get("trials")
    if not isinstance(trials, dict):
        raise ValueError("sandbox status trials must be an object")
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = {}
    unrouted: list[dict[str, Any]] = []
    for trial_id, trial in sorted(trials.items()):
        if not isinstance(trial, dict):
            continue
        introspection_id = str(trial.get("source_introspection_id") or "")
        claim_id = str(trial.get("claim_id") or "")
        canonical_claim_id = f"{introspection_id}:{claim_id}"
        family_id = families.get(canonical_claim_id)
        if family_id is None:
            unrouted.append(
                {
                    "trial_id": str(trial.get("trial_id") or trial_id),
                    "canonical_claim_id": (
                        canonical_claim_id
                        if introspection_id and claim_id
                        else None
                    ),
                    "routing_reason": (
                        "claim_family_missing"
                        if introspection_id and claim_id
                        else "claim_identity_missing"
                    ),
                    "status": trial.get("status"),
                    "adapter": trial.get("adapter"),
                    "agency_tier": trial.get("agency_tier"),
                }
            )
            continue
        signature = intervention_signature(trial)
        grouped.setdefault((family_id, signature), []).append(
            {
                "trial_id": str(trial.get("trial_id") or trial_id),
                "canonical_claim_id": canonical_claim_id,
                "status": trial.get("status"),
                "runnable": bool(trial.get("runnable")),
                "agency_tier": trial.get("agency_tier"),
                "adapter": trial.get("adapter"),
            }
        )
    source_sha256 = digest(
        {
            "sandbox": hashlib.sha256(sandbox_raw).hexdigest(),
            "families": hashlib.sha256(family_raw).hexdigest(),
            "lived_state_context": hashlib.sha256(
                lived_context_raw
            ).hexdigest(),
        }
    )
    events = []
    for trial in unrouted:
        events.append(
            {
                "schema": "experiment_dossier_domain_event_v1",
                "schema_version": 1,
                "event_type": "experiment_dossier_trial_unrouted",
                "aggregate_type": "sandbox_trial",
                "aggregate_id": trial["trial_id"],
                "trial_id": trial["trial_id"],
                "unrouted_trial": trial,
                "source_receipt": {
                    "sandbox_status_sha256": hashlib.sha256(sandbox_raw).hexdigest(),
                    "claim_family_status_sha256": hashlib.sha256(family_raw).hexdigest(),
                    "lived_state_context_sha256": hashlib.sha256(
                        lived_context_raw
                    ).hexdigest(),
                    "combined_source_sha256": source_sha256,
                },
                "idempotency_key": (
                    f"experiment_dossier_unrouted:{trial['trial_id']}:"
                    f"{digest(trial)}"
                ),
                "artifact_authority_state_v1": authority_state("evidence_only"),
            }
        )
    for (family_id, signature), trial_refs in sorted(grouped.items()):
        approval_pending = any(
            agency_tier_value(trial.get("agency_tier")) >= 4
            or "approval_required" in str(trial.get("status") or "")
            for trial in trial_refs
        )
        dossier_id = f"dossier_{digest([family_id, signature])[:20]}"
        context_refs = {
            canonical_json(lived_contexts[introspection_id]): lived_contexts[
                introspection_id
            ]
            for trial in trial_refs
            for introspection_id in [
                str(trial.get("canonical_claim_id") or "").rsplit(":", 1)[0]
            ]
            if introspection_id in lived_contexts
        }
        bounded_context_refs = [
            context_refs[key] for key in sorted(context_refs)[:64]
        ]
        dossier = {
            "schema": "experiment_dossier_v1",
            "schema_version": 1,
            "dossier_id": dossier_id,
            "claim_family_id": family_id,
            "intervention_signature": signature,
            "state": "draft" if approval_pending else "capture-ready",
            "trial_refs": trial_refs,
            "baseline_capture_ref": None,
            "candidate_capture_ref": None,
            "comparison_ref": None,
            "result_ref": None,
            "review_ref": None,
            "closure_ref": None,
            "lived_state_context_refs": bounded_context_refs,
            "lived_state_context_ref_count": len(context_refs),
            "lived_state_context_refs_truncated": len(context_refs) > 64,
            "dossier_sufficient": False,
            "candidate_comparison_allowed": False,
            "live_authority_granted": False,
            "artifact_authority_state_v1": authority_state(
                "approval_pending" if approval_pending else "evidence_only"
            ),
        }
        events.append(
            {
                "schema": "experiment_dossier_domain_event_v1",
                "schema_version": 1,
                "event_type": "experiment_dossier_projected",
                "aggregate_type": "experiment_dossier",
                "aggregate_id": dossier_id,
                "dossier_id": dossier_id,
                "dossier": dossier,
                "source_receipt": {
                    "sandbox_status_sha256": hashlib.sha256(sandbox_raw).hexdigest(),
                    "claim_family_status_sha256": hashlib.sha256(family_raw).hexdigest(),
                    "lived_state_context_sha256": hashlib.sha256(
                        lived_context_raw
                    ).hexdigest(),
                    "combined_source_sha256": source_sha256,
                },
                "idempotency_key": (
                    f"experiment_dossier:{dossier_id}:{digest(dossier)}"
                ),
                "artifact_authority_state_v1": dossier[
                    "artifact_authority_state_v1"
                ],
            }
        )
    return events


def delta_dossier_events(
    candidates: list[dict[str, Any]],
    existing_events: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    """Emit only new or source-relevant dossier records."""

    return delta_dossier_events_from_status(
        candidates,
        project(existing_events),
    )


def delta_dossier_events_from_status(
    candidates: list[dict[str, Any]],
    current: dict[str, Any],
) -> list[dict[str, Any]]:
    """Compare candidates with a verified materialized dossier projection."""

    dossiers = current.get("dossiers") or {}
    unrouted = current.get("unrouted_trials") or {}
    stateful_fields = {
        "state",
        "baseline_capture_ref",
        "candidate_capture_ref",
        "comparison_ref",
        "result_ref",
        "review_ref",
        "closure_ref",
        "dossier_sufficient",
        "candidate_comparison_allowed",
        "live_authority_granted",
    }
    authority_projection_fields = {
        "authority_projection_v2",
        "auto_approved",
        "edits_source_now",
        "grants_approval",
        "live_eligible_now",
    }

    def source_identity(dossier: dict[str, Any]) -> dict[str, Any]:
        source = {
            key: value
            for key, value in dossier.items()
            if key not in stateful_fields
            and key not in authority_projection_fields
            and key != "artifact_authority_state_v1"
        }
        authority = dossier.get("artifact_authority_state_v1")
        source["authority_state"] = (
            authority.get("state") if isinstance(authority, dict) else None
        )
        return source

    result: list[dict[str, Any]] = []
    for event in candidates:
        event_type = event.get("event_type")
        if event_type == "experiment_dossier_projected":
            dossier_id = str(event.get("dossier_id") or "")
            candidate = event.get("dossier")
            existing = dossiers.get(dossier_id)
            if not isinstance(candidate, dict):
                continue
            if isinstance(existing, dict):
                candidate_source = source_identity(candidate)
                existing_source = source_identity(existing)
                if candidate_source == existing_source:
                    continue
        elif event_type == "experiment_dossier_trial_unrouted":
            trial = event.get("unrouted_trial")
            if (
                isinstance(trial, dict)
                and unrouted.get(str(trial.get("trial_id") or "")) == trial
            ):
                continue
        result.append(event)
    return result


def validate_transition(
    dossier: dict[str, Any],
    target_state: str,
    evidence_ref: str,
    approval_receipt: str | None,
) -> None:
    if target_state not in DOSSIER_STATES:
        raise ValueError(f"unknown dossier state: {target_state}")
    current = str(dossier.get("state") or "draft")
    if STATE_INDEX[target_state] != STATE_INDEX[current] + 1:
        raise ValueError(f"invalid dossier transition: {current} -> {target_state}")
    if not evidence_ref.strip():
        raise ValueError("transition evidence reference must not be empty")
    if target_state in {"baseline-captured", "candidate-captured"} and not (
        CAPTURE_REF_RE.fullmatch(evidence_ref.strip())
    ):
        raise ValueError(
            "capture transition requires a content-addressed sha256 reference"
        )
    baseline = dossier.get("baseline_capture_ref")
    if target_state in {"candidate-captured", "comparison-ready"} and not (
        isinstance(baseline, str) and CAPTURE_REF_RE.fullmatch(baseline)
    ):
        raise ValueError("candidate capture/comparison requires a valid baseline")
    candidate = dossier.get("candidate_capture_ref")
    if target_state == "comparison-ready" and not (
        isinstance(candidate, str) and CAPTURE_REF_RE.fullmatch(candidate)
    ):
        raise ValueError("comparison requires a captured candidate")
    authority = dossier.get("artifact_authority_state_v1")
    if (
        isinstance(authority, dict)
        and authority.get("state") == "approval_pending"
        and target_state in {"candidate-captured", "comparison-ready"}
        and not approval_receipt
    ):
        raise ValueError("approval-pending intervention requires an external approval receipt")


def apply_transition(
    dossier: dict[str, Any], event: dict[str, Any]
) -> dict[str, Any]:
    updated = dict(dossier)
    target = str(event.get("target_state") or "")
    evidence_ref = str(event.get("evidence_ref") or "")
    field_by_state = {
        "baseline-captured": "baseline_capture_ref",
        "candidate-captured": "candidate_capture_ref",
        "comparison-ready": "comparison_ref",
        "result-recorded": "result_ref",
        "review-pending": "review_ref",
        "closed": "closure_ref",
    }
    if target in field_by_state:
        updated[field_by_state[target]] = evidence_ref
    updated["state"] = target
    updated["candidate_comparison_allowed"] = bool(
        updated.get("baseline_capture_ref") and updated.get("candidate_capture_ref")
    )
    updated["dossier_sufficient"] = bool(
        updated.get("baseline_capture_ref")
        and updated.get("candidate_capture_ref")
        and updated.get("comparison_ref")
    )
    return updated


def _status_from_state(
    dossiers: dict[str, dict[str, Any]],
    history: list[dict[str, Any]],
    unrouted: dict[str, dict[str, Any]],
    transition_violations: list[dict[str, Any]],
) -> dict[str, Any]:
    counts = Counter(
        str(dossier.get("state") or "draft")
        for dossier in dossiers.values()
    )
    baseline_missing = sum(
        1
        for dossier in dossiers.values()
        if dossier.get("state")
        in {
            "candidate-captured",
            "comparison-ready",
            "result-recorded",
            "review-pending",
            "closed",
        }
        and not dossier.get("baseline_capture_ref")
    )
    return {
        "schema": "experiment_dossier_status_v1",
        "schema_version": 1,
        "dossier_count": len(dossiers),
        "state_counts": dict(sorted(counts.items())),
        "baseline_missing_violation_count": baseline_missing,
        "transition_violation_count": len(transition_violations),
        "unrouted_trial_count": len(unrouted),
        "counter_audit": {
            "all_dossiers_have_one_state": sum(counts.values())
            == len(dossiers),
            "candidate_or_later_has_baseline": baseline_missing == 0,
            "transition_replay_valid": not transition_violations,
        },
        "dossiers": dict(sorted(dossiers.items())),
        "transition_history": history,
        "transition_violations": transition_violations,
        "unrouted_trials": dict(sorted(unrouted.items())),
        "artifact_authority_state_v1": authority_state("evidence_only"),
    }


def apply_dossier_events(
    status: dict[str, Any] | None,
    events: list[dict[str, Any]],
) -> dict[str, Any]:
    current = status or {}
    dossiers = copy.deepcopy(current.get("dossiers") or {})
    history = copy.deepcopy(current.get("transition_history") or [])
    unrouted = copy.deepcopy(current.get("unrouted_trials") or {})
    transition_violations = copy.deepcopy(
        current.get("transition_violations") or []
    )
    stateful_fields = {
        "state",
        "baseline_capture_ref",
        "candidate_capture_ref",
        "comparison_ref",
        "result_ref",
        "review_ref",
        "closure_ref",
        "dossier_sufficient",
        "candidate_comparison_allowed",
        "live_authority_granted",
    }
    for event in events:
        event_type = event.get("event_type")
        dossier_id = str(event.get("dossier_id") or "")
        if event_type == "experiment_dossier_projected":
            dossier = event.get("dossier")
            if isinstance(dossier, dict):
                if str(dossier.get("claim_family_id") or "").startswith(
                    "family_unresolved_"
                ):
                    for trial in dossier.get("trial_refs") or []:
                        if isinstance(trial, dict) and trial.get("trial_id"):
                            unrouted[str(trial["trial_id"])] = {
                                **trial,
                                "routing_reason": "legacy_unresolved_family_projection",
                            }
                    continue
                previous = dossiers.get(dossier_id)
                if previous is None:
                    dossiers[dossier_id] = dict(dossier)
                else:
                    refreshed = dict(dossier)
                    for field in stateful_fields:
                        if field in previous:
                            refreshed[field] = previous[field]
                    old_authority = previous.get("artifact_authority_state_v1")
                    new_authority = refreshed.get("artifact_authority_state_v1")
                    old_pending = (
                        isinstance(old_authority, dict)
                        and old_authority.get("state") == "approval_pending"
                    )
                    new_pending = (
                        isinstance(new_authority, dict)
                        and new_authority.get("state") == "approval_pending"
                    )
                    if old_pending or new_pending:
                        refreshed["artifact_authority_state_v1"] = authority_state(
                            "approval_pending"
                        )
                    dossiers[dossier_id] = refreshed
        elif event_type == "experiment_dossier_transitioned":
            if dossier_id in dossiers:
                try:
                    validate_transition(
                        dossiers[dossier_id],
                        str(event.get("target_state") or ""),
                        str(event.get("evidence_ref") or ""),
                        (
                            str(event["approval_receipt"])
                            if event.get("approval_receipt")
                            else None
                        ),
                    )
                except ValueError as error:
                    transition_violations.append(
                        {
                            "dossier_id": dossier_id,
                            "event_type": event_type,
                            "error": str(error),
                        }
                    )
                else:
                    dossiers[dossier_id] = apply_transition(
                        dossiers[dossier_id], event
                    )
                    history.append(event)
        elif event_type == "experiment_dossier_trial_unrouted":
            trial = event.get("unrouted_trial")
            if isinstance(trial, dict) and trial.get("trial_id"):
                unrouted[str(trial["trial_id"])] = trial
    return _status_from_state(
        dossiers,
        history,
        unrouted,
        transition_violations,
    )


def project(events: list[dict[str, Any]]) -> dict[str, Any]:
    return apply_dossier_events(None, events)


def render_report(status: dict[str, Any]) -> str:
    lines = [
        "# Experiment Dossiers",
        "",
        f"- Dossiers: {status['dossier_count']}",
        f"- Baseline gate violations: {status['baseline_missing_violation_count']}",
        f"- Transition replay violations: {status['transition_violation_count']}",
        f"- Unrouted legacy trials: {status['unrouted_trial_count']}",
    ]
    lines.extend(
        f"- {state}: {status['state_counts'].get(state, 0)}"
        for state in DOSSIER_STATES
    )
    lines.extend(
        [
            "- Candidate comparison requires baseline: enforced",
            "- Live interventions remain approval pending",
            "",
        ]
    )
    return "\n".join(lines)


def atomic_write_text(path: Path, payload: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    handle = tempfile.NamedTemporaryFile(
        "w",
        encoding="utf-8",
        dir=path.parent,
        prefix=f".{path.name}.",
        delete=False,
    )
    temporary = Path(handle.name)
    try:
        with handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        directory_fd = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    finally:
        temporary.unlink(missing_ok=True)


def write_projection(workspace: Path, status: dict[str, Any]) -> dict[str, str]:
    root = state_dir(workspace)
    root.mkdir(parents=True, exist_ok=True)
    outputs = {
        "status.json": json.dumps(status, indent=2, sort_keys=True) + "\n",
        "report.md": render_report(status),
    }
    hashes = {}
    for name, payload in outputs.items():
        atomic_write_text(root / name, payload)
        hashes[name] = hashlib.sha256(payload.encode()).hexdigest()
    return hashes


def _cursor_path(workspace: Path) -> Path:
    return state_dir(workspace) / "projection_cursor_v1.json"


def _status_sha256(workspace: Path) -> str | None:
    path = state_dir(workspace) / "status.json"
    if not path.is_file():
        return None
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _incremental_base(
    workspace: Path,
    store: EvidenceEventStore,
) -> tuple[
    dict[str, Any],
    ProjectionInputCursor,
    int,
    int,
    bool,
]:
    cursor = ProjectionInputCursor(
        _cursor_path(workspace),
        "experiment_dossiers_v2",
    )
    metadata = cursor.jsonl_metadata("claim_families_v2")
    watermark = store.stream_watermarks(("claim_families",))[
        "claim_families"
    ]
    current_stream_seq = int(watermark.get("stream_seq") or 0)
    cursor_stream_seq = int(metadata.get("stream_seq") or 0)
    cached_hash = _status_sha256(workspace)
    cache_valid = (
        cursor_stream_seq <= current_stream_seq
        and cursor_stream_seq > 0
        and cached_hash is not None
        and cached_hash == metadata.get("status_sha256")
    )
    if cache_valid:
        status = load_object(state_dir(workspace) / "status.json")
        envelopes, corrupt = store.envelopes_for_stream(
            "claim_families",
            after_stream_seq=cursor_stream_seq,
        )
        if corrupt:
            raise EvidenceStoreError("claim family tail is corrupt")
        tail = [
            envelope.payload
            for envelope in envelopes
            if str(envelope.payload.get("event_type") or "").startswith(
                "experiment_dossier_"
            )
        ]
        return (
            apply_dossier_events(status, tail),
            cursor,
            current_stream_seq,
            len(tail),
            False,
        )

    events, corrupt = read_domain_events(
        family_state_dir(workspace),
        "claim_families",
    )
    if corrupt:
        raise EvidenceStoreError("claim family stream is corrupt")
    return project(events), cursor, current_stream_seq, len(events), True


def generate(workspace: Path, *, write: bool) -> dict[str, Any]:
    candidates = initial_dossier_events(workspace)
    if write:
        store = EvidenceEventStore(
            workspace / "diagnostics/evidence_event_store_v2"
        )
        store.verify_indexed_tail()
        current, cursor, prior_stream_seq, consumed_count, full_replay = (
            _incremental_base(workspace, store)
        )
        generated = delta_dossier_events_from_status(candidates, current)
        append_domain_events(
            family_state_dir(workspace), "claim_families", generated
        )
        appended, corrupt = store.envelopes_for_stream(
            "claim_families",
            after_stream_seq=prior_stream_seq,
        )
        if corrupt:
            raise EvidenceStoreError("appended claim family tail is corrupt")
        canonical_generated = [
            envelope.payload
            for envelope in appended
            if str(envelope.payload.get("event_type") or "").startswith(
                "experiment_dossier_"
            )
        ]
        status = apply_dossier_events(current, canonical_generated)
    else:
        generated = candidates
        canonical_generated = generated
        status = project(candidates)
        full_replay = False
        consumed_count = len(candidates)
        prior_stream_seq = 0
    status["generated_event_count"] = len(generated)
    status["canonical_applied_event_count"] = len(canonical_generated)
    status["incremental_consumed_event_count"] = consumed_count
    status["full_reference_replay"] = full_replay
    if write:
        hashes = write_projection(workspace, status)
        final_watermark = store.stream_watermarks(("claim_families",))[
            "claim_families"
        ]
        store.write_checkpoint(
            "experiment_dossiers_v1",
            PROJECTOR_VERSION,
            hashes,
            input_streams=("claim_families",),
            source_hashes={
                "sandbox_status": hashlib.sha256(
                    sandbox_status_path(workspace).read_bytes()
                ).hexdigest(),
                "claim_family_status": hashlib.sha256(
                    family_status_path(workspace).read_bytes()
                ).hexdigest(),
            },
        )
        cursor.commit_jsonl(
            {
                "claim_families_v2": {
                    "stream_seq": int(final_watermark.get("stream_seq") or 0),
                    "last_global_seq": int(
                        final_watermark.get("last_global_seq") or 0
                    ),
                    "last_event_id": final_watermark.get("last_event_id"),
                    "last_event_sha256": final_watermark.get(
                        "last_event_sha256"
                    ),
                    "status_sha256": _status_sha256(workspace),
                    "prior_stream_seq": prior_stream_seq,
                    "full_reference_replay": full_replay,
                }
            }
        )
        status["projection_hashes"] = hashes
    return status


def transition(
    workspace: Path,
    dossier_id: str,
    target_state: str,
    evidence_ref: str,
    approval_receipt: str | None,
) -> dict[str, Any]:
    events, corrupt = read_domain_events(
        family_state_dir(workspace), "claim_families"
    )
    if corrupt:
        raise EvidenceStoreError("claim family stream is corrupt")
    status = project(events)
    dossier = status["dossiers"].get(dossier_id)
    if not isinstance(dossier, dict):
        raise ValueError(f"unknown dossier: {dossier_id}")
    validate_transition(dossier, target_state, evidence_ref, approval_receipt)
    authority = dossier.get("artifact_authority_state_v1")
    authority = authority if isinstance(authority, dict) else authority_state("evidence_only")
    event = {
        "schema": "experiment_dossier_domain_event_v1",
        "schema_version": 1,
        "event_type": "experiment_dossier_transitioned",
        "aggregate_type": "experiment_dossier",
        "aggregate_id": dossier_id,
        "dossier_id": dossier_id,
        "from_state": dossier.get("state"),
        "target_state": target_state,
        "evidence_ref": evidence_ref,
        "approval_receipt": approval_receipt,
        "idempotency_key": (
            f"dossier_transition:{dossier_id}:{target_state}:"
            f"{digest([evidence_ref, approval_receipt])}"
        ),
        "artifact_authority_state_v1": authority,
    }
    append_domain_events(family_state_dir(workspace), "claim_families", [event])
    return event


def cli_summary(status: dict[str, Any]) -> dict[str, Any]:
    return {
        key: value
        for key, value in status.items()
        if key
        not in {
            "dossiers",
            "transition_history",
            "transition_violations",
            "unrouted_trials",
        }
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    commands = parser.add_subparsers(dest="command")
    generate_parser = commands.add_parser("generate")
    generate_parser.add_argument("--write", action="store_true")
    project_parser = commands.add_parser("project")
    project_parser.add_argument("--write", action="store_true")
    project_parser.add_argument("--receipt-json", action="store_true")
    commands.add_parser("report")
    advance = commands.add_parser("advance")
    advance.add_argument("--dossier-id", required=True)
    advance.add_argument("--state", required=True, choices=DOSSIER_STATES)
    advance.add_argument("--evidence-ref", required=True)
    advance.add_argument("--approval-receipt")
    advance.add_argument("--write", action="store_true")
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    if args.self_test:
        from experiment_dossiers_selftest import run

        return run()
    workspace = args.workspace.resolve()
    try:
        if args.command in {"generate", "project", "report"}:
            started = time.monotonic()
            status = generate(
                workspace,
                write=args.command in {"generate", "project"} and bool(args.write),
            )
            if args.command == "project":
                root = state_dir(workspace)
                print(
                    json.dumps(
                        projector_receipt(
                            "experiment_dossiers",
                            cli_summary(status),
                            {
                                "status.json": root / "status.json",
                                "report.md": root / "report.md",
                            },
                            started_monotonic=started,
                        ),
                        indent=2,
                        sort_keys=True,
                    )
                )
                return 0
            print(json.dumps(cli_summary(status), indent=2, sort_keys=True))
            return 0
        if args.command == "advance":
            if not args.write:
                raise ValueError("advance requires --write")
            event = transition(
                workspace,
                args.dossier_id,
                args.state,
                args.evidence_ref,
                args.approval_receipt,
            )
            print(json.dumps(event, indent=2, sort_keys=True))
            return 0
    except (EvidenceStoreError, OSError, ValueError) as error:
        print(json.dumps({"status": "failed", "error": str(error)}), file=sys.stderr)
        return 1
    parser.print_help()
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
