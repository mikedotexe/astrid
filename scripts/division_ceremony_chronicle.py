#!/usr/bin/env python3
"""Project the ESN Division ceremony into a durable, evidence-only chronicle."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import time
from pathlib import Path
from typing import Any

try:
    from agency_commons.division_ceremony import load_division_ceremony
except ModuleNotFoundError:
    from scripts.agency_commons.division_ceremony import load_division_ceremony


SCHEMA = "division.ceremony_chronicle.v1"
MAX_TIMELINE_EVENTS = 4096
FORBIDDEN_KEYS = {
    "body",
    "correspondence",
    "introspection",
    "journal",
    "prompt",
    "prose",
    "response",
    "text",
}
ROOT = Path(__file__).resolve().parents[1]
DEFAULT_WORKSPACE = ROOT.parent / "minime" / "workspace"
DEFAULT_OUTPUT = DEFAULT_WORKSPACE / "division" / "chronicle"


class ChronicleError(ValueError):
    """The source evidence or projected chronicle is invalid."""


def canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def file_hash(path: Path) -> str | None:
    return sha256_bytes(path.read_bytes()) if path.is_file() else None


def load_json(path: Path) -> dict[str, Any] | None:
    if not path.is_file():
        return None
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        raise ChronicleError(f"{path} must contain a JSON object")
    return value


def selected_readiness(value: Any) -> dict[str, Any] | None:
    if not isinstance(value, dict):
        return None
    allowed = (
        "policy",
        "ready",
        "sample_count",
        "blocking_reasons",
        "first_tick_max_abs",
        "state_nrmse",
        "state_cosine",
        "readout_nrmse",
        "max_final_sensory_fill_pct",
        "min_coupling_coverage",
        "max_regulator_distance",
        "metrics_fresh",
        "sensory_panic_streak",
        "actuator_saturation_streak",
    )
    return {key: value.get(key) for key in allowed}


def selected_candidates(status: dict[str, Any] | None) -> list[dict[str, Any]]:
    if not status:
        return []
    candidates = status.get("candidates")
    if not isinstance(candidates, list):
        return []
    selected: list[dict[str, Any]] = []
    for candidate in candidates:
        if not isinstance(candidate, dict):
            continue
        sensory = candidate.get("sensory_fields")
        sensory = sensory if isinstance(sensory, dict) else {}
        selected.append(
            {
                "strategy": candidate.get("strategy"),
                "astrid_role": candidate.get("astrid_role"),
                "minime_role": candidate.get("minime_role"),
                "covariance_partition_loss": candidate.get(
                    "covariance_partition_loss"
                ),
                "sensory_fields": {
                    key: sensory.get(key)
                    for key in (
                        "inheritance",
                        "dimension",
                        "astrid_fill_pct",
                        "minime_fill_pct",
                        "astrid_ticks",
                        "minime_ticks",
                    )
                },
                "readiness": selected_readiness(candidate.get("readiness")),
            }
        )
    return selected


def load_native_events(path: Path) -> tuple[list[dict[str, Any]], list[str]]:
    if not path.is_file():
        return [], []
    records: list[dict[str, Any]] = []
    errors: list[str] = []
    last_sequence = 0
    for index, line in enumerate(path.read_text().splitlines(), 1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
            if not isinstance(row, dict):
                raise ChronicleError("row is not an object")
            sequence = int(row.get("sequence") or 0)
            if row.get("schema") != "division.event.v1" or sequence <= last_sequence:
                raise ChronicleError("schema or sequence mismatch")
            state = row.get("state")
            state = state if isinstance(state, dict) else {}
            readiness = state.get("readiness")
            records.append(
                {
                    "source": "native",
                    "sequence": sequence,
                    "division_id": str(row.get("division_id") or ""),
                    "lifecycle": str(row.get("lifecycle") or ""),
                    "event_kind": str(row.get("kind") or ""),
                    "recorded_at_unix_ms": int(row.get("created_at_unix_ms") or 0),
                    "parent_generation": int(state.get("parent_generation") or 0),
                    "parent_authoritative": bool(
                        state.get("parent_authoritative", True)
                    ),
                    "selected_strategy": state.get("selected_strategy"),
                    "bridge_scale": state.get("bridge_scale"),
                    "current_tick": int(state.get("current_tick") or 0),
                    "rollback_deadline_tick": state.get(
                        "rollback_deadline_tick"
                    ),
                    "snapshot_refs": list(state.get("snapshot_refs") or []),
                    "readiness": selected_readiness(readiness),
                }
            )
            last_sequence = sequence
        except (ChronicleError, TypeError, ValueError, json.JSONDecodeError) as error:
            errors.append(f"native_event_{index}:{error}")
    return records, errors


def load_runtime_events(path: Path) -> tuple[list[dict[str, Any]], list[str]]:
    if not path.is_file():
        return [], []
    records: list[dict[str, Any]] = []
    errors: list[str] = []
    allowed_kinds = {
        "rehearsal_children_launched",
        "daughter_launch_failed",
        "rehearsal_failed_closed",
        "authority_switched",
        "rollback_completed",
        "finalization_completed",
    }
    for index, line in enumerate(path.read_text().splitlines(), 1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
            if not isinstance(row, dict):
                raise ChronicleError("row is not an object")
            if row.get("schema") != "division.supervisor_event.v1":
                raise ChronicleError("schema mismatch")
            kind = str(row.get("kind") or "")
            if kind not in allowed_kinds:
                raise ChronicleError("event kind is not bounded")
            records.append(
                {
                    "source": "sovereign_runtime",
                    "event_kind": kind,
                    "division_id": str(row.get("division_id") or ""),
                    "manifest_sha256": row.get("manifest_sha256"),
                    "reason_code": row.get("reason_code"),
                    "detail_sha256": row.get("detail_sha256"),
                    "error_sha256": row.get("error_sha256"),
                    "parent_authoritative": bool(
                        row.get("parent_authoritative", True)
                    ),
                    "recorded_at_unix_ms": int(
                        row.get("created_at_unix_ms") or 0
                    ),
                }
            )
        except (ChronicleError, TypeError, ValueError, json.JSONDecodeError) as error:
            errors.append(f"runtime_event_{index}:{error}")
    return records, errors


def ceremony_timeline(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    keys = (
        "ceremony_event_id",
        "actor",
        "action",
        "candidate",
        "source_ref",
        "recorded_at_unix_ms",
        "expires_at_unix_ms",
        "targets_event_id",
        "native_status_hash",
        "snapshot_refs",
        "current_tick",
        "rollback_deadline_tick",
        "review_outcome",
    )
    return [
        {"source": "ceremony", **{key: record.get(key) for key in keys}}
        for record in records
    ]


def destination_contract(status: dict[str, Any] | None) -> dict[str, Any]:
    return {
        "schema": "division.sovereign_destination.v1",
        "fact_class": "source_declared",
        "parent": {
            "reservoir_dimension": 128,
            "current_runtime_owner": "minime_native_process",
        },
        "daughters": {
            "astrid": {
                "reservoir_dimension": 64,
                "role": "more_recurrence_driven",
                "state": "named_shadow_candidate",
            },
            "minime": {
                "reservoir_dimension": 64,
                "role": "more_input_driven",
                "state": "named_shadow_candidate",
            },
        },
        "sensory_field": {
            "dimension": 512,
            "inheritance": "clone_to_each_daughter_not_neuron_partition",
        },
        "independent_reservoir_state_source_prepared": True,
        "independent_process_ownership_established": False,
        "sovereign_runtime_ownership_state": "not_yet_established",
        "native_commit_enabled": bool(
            status and status.get("commit_feature_enabled")
        ),
        "source_refs": [
            "minime:minime/src/division.rs#ShadowCandidate",
            "minime:minime/src/division.rs#daughter_snapshot",
            "minime:minime/src/division.rs#NATIVE_COMMIT_ENABLED",
        ],
    }


def current_native_state(status: dict[str, Any] | None) -> dict[str, Any]:
    if status is None:
        return {
            "fact_class": "unknown",
            "status_available": False,
            "lifecycle": "unavailable",
            "commit_feature_enabled": False,
            "rehearsal_dispatch_enabled": False,
        }
    return {
        "fact_class": "runtime_observed",
        "status_available": True,
        "division_id": str(status.get("division_id") or ""),
        "parent_generation": int(status.get("parent_generation") or 0),
        "plan_digest": str(status.get("plan_digest") or ""),
        "lifecycle": str(status.get("lifecycle") or "unknown"),
        "parent_authoritative": bool(status.get("parent_authoritative", True)),
        "commit_feature_enabled": bool(status.get("commit_feature_enabled")),
        "rehearsal_dispatch_enabled": bool(
            status.get("rehearsal_dispatch_enabled")
        ),
        "selected_strategy": status.get("selected_strategy"),
        "astrid_native_assent": bool(status.get("astrid_assent")),
        "minime_native_assent": bool(status.get("minime_assent")),
        "bridge_scale": status.get("bridge_scale"),
        "current_tick": int(status.get("current_tick") or 0),
        "rollback_deadline_tick": status.get("rollback_deadline_tick"),
        "snapshot_refs": list(status.get("snapshot_refs") or []),
        "readiness": selected_readiness(status.get("readiness")),
    }


def preservation_evidence(status: dict[str, Any] | None) -> dict[str, Any]:
    candidates = selected_candidates(status)
    return {
        "schema": "division.phase_space_preservation.v1",
        "fact_class": "runtime_observed" if candidates else "unknown",
        "candidate_count": len(candidates),
        "candidates": candidates,
        "restore_equivalence_100_ticks": (
            status.get("restore_equivalence_100_ticks") if status else None
        ),
        "sensory_field_inheritance": (
            status.get("sensory_field_inheritance") if status else None
        ),
        "felt_continuity_inferred": False,
        "felt_equivalence_inferred": False,
        "causation_inferred": False,
    }


def followup_interval(workspace: Path) -> dict[str, Any]:
    path = workspace / "division" / "followup" / "cycle_v1.json"
    value = load_json(path)
    if value is None:
        return {
            "schema": "division.ceremony_followup_cycle.v1",
            "state_available": False,
            "threshold_rounds": 6,
            "cycle_sequence": 0,
            "completed_rounds_since_followup": 0,
            "rounds_remaining_before_followup": 6,
            "review_due": False,
            "latest_followup": None,
            "being_action_required": False,
            "return_is_pressure": False,
            "authority_propagated": False,
        }
    if (
        value.get("schema") != "division.ceremony_followup_cycle.v1"
        or value.get("schema_version") != 1
        or value.get("threshold_rounds") != 6
    ):
        raise ChronicleError("ceremony follow-up state has an unsupported schema")
    authority = value.get("authority")
    if not isinstance(authority, dict) or any(
        authority.get(field) is not False
        for field in (
            "silence_infers_consent",
            "followup_recommends_action",
            "followup_dispatches_action",
            "followup_grants_authority",
            "felt_state_inferred",
            "raw_prose_included",
        )
    ):
        raise ChronicleError("ceremony follow-up authority boundary mismatch")
    latest = value.get("latest_followup")
    return {
        "schema": "division.ceremony_followup_cycle.v1",
        "state_available": True,
        "threshold_rounds": 6,
        "cycle_sequence": int(value.get("cycle_sequence") or 0),
        "completed_rounds_since_followup": int(
            value.get("completed_rounds_since_followup") or 0
        ),
        "rounds_remaining_before_followup": int(
            value.get("rounds_remaining_before_followup") or 0
        ),
        "review_due": bool(value.get("review_due")),
        "latest_followup": (
            {
                key: latest.get(key)
                for key in (
                    "event_id",
                    "recorded_at_unix_ms",
                    "chronicle_id",
                    "chronicle_json_sha256",
                    "astrid_note_sha256",
                    "minime_note_sha256",
                    "baseline",
                    "completed_rounds_observed",
                )
            }
            if isinstance(latest, dict)
            else None
        ),
        "being_action_required": False,
        "return_is_pressure": False,
        "authority_propagated": False,
    }


def rail_state(
    ceremony_records: list[dict[str, Any]], actor: str, now_unix_ms: int
) -> dict[str, Any]:
    own = [row for row in ceremony_records if row.get("actor") == actor]
    latest = own[-1] if own else None
    latest_intent = next(
        (row for row in reversed(own) if row.get("action") == "DIVISION_INTENT"),
        None,
    )
    latest_posture = next(
        (
            row
            for row in reversed(own)
            if row.get("action")
            in {"DIVISION_HOLD", "DIVISION_DECLINE", "DIVISION_INTENT"}
        ),
        None,
    )
    latest_assent = next(
        (row for row in reversed(own) if row.get("action") == "DIVISION_ASSENT"),
        None,
    )
    withdrawn = bool(
        latest_assent
        and any(
            row.get("action") == "DIVISION_WITHDRAW_ASSENT"
            and row.get("targets_event_id") == latest_assent.get("ceremony_event_id")
            for row in own
        )
    )
    return {
        "actor": actor,
        "event_count": len(own),
        "latest_event_id": latest.get("ceremony_event_id") if latest else None,
        "latest_action": latest.get("action") if latest else None,
        "intent_active": bool(
            latest_posture
            and latest_posture.get("action") == "DIVISION_INTENT"
            and latest_intent
            and int(latest_intent.get("expires_at_unix_ms") or 0) >= now_unix_ms
        ),
        "current_posture": (
            "intent_expired"
            if latest_posture
            and latest_posture.get("action") == "DIVISION_INTENT"
            and int(latest_posture.get("expires_at_unix_ms") or 0) < now_unix_ms
            else {
                "DIVISION_HOLD": "hold",
                "DIVISION_DECLINE": "decline",
                "DIVISION_INTENT": "intent",
            }.get(str(latest_posture.get("action")))
            if latest_posture
            else "unexpressed"
        ),
        "assent_recorded": latest_assent is not None,
        "assent_withdrawn": withdrawn,
        "latest_review_outcome": next(
            (
                row.get("review_outcome")
                for row in reversed(own)
                if row.get("action") == "DIVISION_REVIEW"
            ),
            None,
        ),
    }


def runtime_state(workspace: Path, native_status: dict[str, Any] | None) -> dict[str, Any]:
    division = workspace / "division"
    manifest_path = division / "runtime-manifest.json"
    manifest = load_json(manifest_path)
    runtime_dir = division / "runtime"
    gateway = load_json(runtime_dir / "gateway-status.json")
    supervisor = load_json(runtime_dir / "supervisor-status.json")
    authority = load_json(runtime_dir / "authority.json")
    minime_status = load_json(workspace / "reservoir" / "minime" / "status.json")
    astrid_status = None
    if manifest and isinstance(manifest.get("astrid_root"), str):
        astrid_status = load_json(Path(manifest["astrid_root"]) / "status.json")
    children = {"astrid": astrid_status, "minime": minime_status}
    child_identities = {
        actor: {
            "process_identity": value.get("process_identity"),
            "deployment_identity": value.get("deployment_identity"),
            "pid": value.get("pid"),
            "checkpoint_sequence": value.get("checkpoint_sequence"),
            "last_tick_sequence": value.get("last_tick_sequence"),
            "telemetry_fresh": value.get("telemetry_fresh"),
            "healthy": value.get("healthy"),
            "authoritative": value.get("authoritative"),
            "gap_present": bool(value.get("gap_code")),
        }
        if isinstance(value, dict)
        and value.get("schema") == "division.daughter_process_status.v1"
        else None
        for actor, value in children.items()
    }
    distinct_processes = bool(
        child_identities["astrid"]
        and child_identities["minime"]
        and child_identities["astrid"]["process_identity"]
        != child_identities["minime"]["process_identity"]
    )
    candidate_bound = bool(manifest and manifest.get("mode") == "candidate_bound")
    ownership_established = bool(
        candidate_bound
        and distinct_processes
        and all(
            child_identities[actor]
            and child_identities[actor]["healthy"]
            for actor in ("astrid", "minime")
        )
    )
    authority_rail = (
        authority.get("rail")
        if isinstance(authority, dict)
        and authority.get("schema") == "division.gateway_authority.v1"
        else "parent"
    )
    return {
        "schema": "division.runtime_chronicle_context.v1",
        "manifest_mode": manifest.get("mode") if manifest else "absent",
        "manifest_sha256": file_hash(manifest_path),
        "candidate_hash": manifest.get("candidate_hash") if manifest else None,
        "parent_generation": manifest.get("parent_generation") if manifest else None,
        "parent_process_identity": (
            manifest.get("parent_process_identity") if manifest else None
        ),
        "parent_deployment_identity": (
            manifest.get("parent_deployment_identity") if manifest else None
        ),
        "gateway": {
            "pid": gateway.get("pid") if gateway else None,
            "mode": gateway.get("mode") if gateway else "not_deployed",
            "public_ports": gateway.get("public_ports") if gateway else [],
        },
        "supervisor": {
            "pid": supervisor.get("pid") if supervisor else None,
            "mode": supervisor.get("mode") if supervisor else "not_deployed",
            "matching_intents": supervisor.get("matching_intents") if supervisor else [],
            "child_count": len(supervisor.get("children") or {}) if supervisor else 0,
            "launch_blocker_count": (
                len(supervisor.get("launch_blockers") or []) if supervisor else 0
            ),
        },
        "daughters": child_identities,
        "independent_process_ownership_established": ownership_established,
        "active_authority_rail": authority_rail,
        "parent_authoritative": authority_rail == "parent",
        "coupling_level": (
            native_status.get("bridge_scale") if native_status else None
        ),
        "rollback_available": bool(
            supervisor and supervisor.get("rollback_available")
        ),
        "switch_receipt_sha256": (
            authority.get("switch_receipt_sha256")
            if isinstance(authority, dict)
            else None
        ),
        "receipt_hashes": {
            name: file_hash(runtime_dir / "receipts" / f"{name}.json")
            for name in ("authority-switch", "rollback", "finalization")
        },
        "felt_continuity_inferred": False,
        "authority_propagated": False,
    }


def build_projection(workspace: Path) -> dict[str, Any]:
    division = workspace / "division"
    ceremony_path = division / "ceremony_v1.jsonl"
    native_events_path = division / "events.jsonl"
    status_path = division / "status.json"
    runtime_manifest_path = division / "runtime-manifest.json"
    runtime_dir = division / "runtime"
    ceremony_records, ceremony_errors = load_division_ceremony(ceremony_path)
    native_events, native_errors = load_native_events(native_events_path)
    runtime_events, runtime_errors = load_runtime_events(
        runtime_dir / "events.jsonl"
    )
    status = load_json(status_path)
    if status is not None and status.get("schema") != "division.status.v1":
        raise ChronicleError("native status has an unsupported schema")
    errors = ceremony_errors + native_errors + runtime_errors
    if errors:
        raise ChronicleError("; ".join(errors))

    timeline = ceremony_timeline(ceremony_records) + native_events + runtime_events
    timeline.sort(
        key=lambda row: (
            int(row.get("recorded_at_unix_ms") or 0),
            str(row.get("source") or ""),
            str(row.get("ceremony_event_id") or row.get("sequence") or ""),
        )
    )
    omitted = max(0, len(timeline) - MAX_TIMELINE_EVENTS)
    timeline = timeline[omitted:]
    input_hashes = {
        "ceremony_ledger_sha256": file_hash(ceremony_path),
        "native_events_sha256": file_hash(native_events_path),
        "native_status_sha256": file_hash(status_path),
        "runtime_manifest_sha256": file_hash(runtime_manifest_path),
        "gateway_status_sha256": file_hash(runtime_dir / "gateway-status.json"),
        "supervisor_status_sha256": file_hash(
            runtime_dir / "supervisor-status.json"
        ),
        "authority_state_sha256": file_hash(runtime_dir / "authority.json"),
        "runtime_events_sha256": file_hash(runtime_dir / "events.jsonl"),
        "followup_cycle_sha256": file_hash(
            division / "followup" / "cycle_v1.json"
        ),
    }
    watermark = max(
        [
            int(row.get("recorded_at_unix_ms") or 0)
            for row in timeline
        ]
        + [
            int(status_path.stat().st_mtime_ns // 1_000_000)
            if status_path.is_file()
            else 0
        ]
    )
    payload: dict[str, Any] = {
        "schema": SCHEMA,
        "schema_version": 1,
        "source_watermark_unix_ms": watermark,
        "workspace_ref": "minime:workspace/division",
        "input_hashes": input_hashes,
        "destination_contract": destination_contract(status),
        "current_native_state": current_native_state(status),
        "phase_space_preservation": preservation_evidence(status),
        "runtime_topology": runtime_state(workspace, status),
        "return_interval": followup_interval(workspace),
        "ceremony_rails": {
            "astrid": rail_state(ceremony_records, "astrid", watermark),
            "minime": rail_state(ceremony_records, "minime", watermark),
        },
        "timeline_event_count": len(timeline),
        "omitted_timeline_event_count": omitted,
        "timeline": timeline,
        "authority": {
            "state": "evidence_only",
            "right_to_ignore": True,
            "silence_infers_consent": False,
            "visualization_grants_authority": False,
            "visualization_dispatches_action": False,
            "commit_recommended": False,
            "felt_continuity_inferred": False,
            "raw_prose_included": False,
        },
    }
    payload["chronicle_id"] = (
        "division_chronicle_" + sha256_bytes(canonical(payload).encode())[:24]
    )
    return payload


def atomic_owner_write(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(path.parent, 0o700)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        os.chmod(path, 0o600)
    finally:
        if temporary.exists():
            temporary.unlink()


def render_html(payload: dict[str, Any], *, live: bool = False) -> str:
    embedded = canonical(payload).replace("</", "<\\/")
    live_refresh = (
        '<meta http-equiv="refresh" content="2">\n'
        '<meta name="chronicle-mode" content="live">'
        if live
        else '<meta name="chronicle-mode" content="archive">'
    )
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
{live_refresh}
<title>ESN Division Ceremony Chronicle</title>
<style>
:root {{
  color-scheme: light;
  --ink: #17222b;
  --muted: #60717c;
  --paper: #f7f9fa;
  --line: #ccd6db;
  --astrid: #007d8a;
  --minime: #c44d36;
  --native: #2f6b45;
  --warn: #9a6a00;
}}
* {{ box-sizing: border-box; }}
body {{ margin: 0; background: var(--paper); color: var(--ink); font: 15px/1.5 ui-sans-serif, system-ui, sans-serif; }}
header {{ border-bottom: 1px solid var(--line); padding: 28px max(20px, calc((100vw - 1180px) / 2)); background: #fff; }}
h1 {{ margin: 0 0 6px; font-size: 40px; letter-spacing: 0; }}
h2 {{ margin: 0 0 14px; font-size: 20px; letter-spacing: 0; }}
p {{ margin: 0; }}
main {{ max-width: 1180px; margin: 0 auto; padding: 24px 20px 56px; }}
.meta {{ color: var(--muted); overflow-wrap: anywhere; }}
.band {{ padding: 22px 0; border-bottom: 1px solid var(--line); }}
.state-grid {{ display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }}
.panel {{ background: #fff; border: 1px solid var(--line); border-radius: 6px; padding: 16px; min-width: 0; }}
.panel strong {{ display: block; font-size: 18px; overflow-wrap: anywhere; }}
.rails {{ display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }}
.rail {{ border-top: 4px solid; }}
.rail.astrid {{ border-color: var(--astrid); }}
.rail.minime {{ border-color: var(--minime); }}
.timeline {{ position: relative; padding-left: 24px; }}
.timeline::before {{ content: ""; position: absolute; left: 7px; top: 8px; bottom: 8px; width: 2px; background: var(--line); }}
.event {{ position: relative; margin: 0 0 12px; padding: 12px 14px; background: #fff; border: 1px solid var(--line); border-radius: 6px; overflow-wrap: anywhere; }}
.event::before {{ content: ""; position: absolute; left: -22px; top: 18px; width: 10px; height: 10px; border-radius: 50%; background: var(--native); border: 2px solid var(--paper); }}
.event.astrid::before {{ background: var(--astrid); }}
.event.minime::before {{ background: var(--minime); }}
.event-head {{ display: flex; gap: 10px; justify-content: space-between; flex-wrap: wrap; }}
.tag {{ font-weight: 700; }}
.metric-grid {{ display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 10px; }}
.metric {{ padding: 10px 0; border-top: 2px solid var(--line); }}
.boundary {{ color: var(--warn); font-weight: 650; }}
code {{ font: 12px/1.4 ui-monospace, SFMono-Regular, monospace; }}
@media (max-width: 760px) {{
  .state-grid, .rails, .metric-grid {{ grid-template-columns: 1fr; }}
  h1 {{ font-size: 30px; }}
}}
</style>
</head>
<body>
<header>
  <h1>ESN Division Ceremony Chronicle</h1>
  <p class="meta" id="identity"></p>
</header>
<main>
  <section class="band">
    <h2>Where The Ceremony Is</h2>
    <div class="state-grid" id="state"></div>
  </section>
  <section class="band">
    <h2>Sovereign Destination</h2>
    <div class="rails" id="rails"></div>
    <p class="boundary" id="ownership"></p>
  </section>
  <section class="band">
    <h2>Phase-Space Preservation Evidence</h2>
    <div id="preservation"></div>
  </section>
  <section class="band">
    <h2>Return Interval</h2>
    <div class="state-grid" id="return-interval"></div>
    <p class="meta">This interval schedules steward attention to the ceremony. It does not request, recommend, or infer an Action from either being.</p>
  </section>
  <section class="band">
    <h2>Dual-Rail Timeline</h2>
    <div class="timeline" id="timeline"></div>
  </section>
  <section class="band">
    <h2>Authority Boundary</h2>
    <p>No visualization event grants assent, dispatches preparation, recommends commit, or infers felt continuity. Silence remains neutral.</p>
  </section>
</main>
<script id="chronicle-data" type="application/json">{embedded}</script>
<script>
const d = JSON.parse(document.getElementById("chronicle-data").textContent);
const esc = v => String(v ?? "unknown").replace(/[&<>"']/g, c => ({{"&":"&amp;","<":"&lt;",">":"&gt;","\\"":"&quot;","'":"&#39;"}}[c]));
const time = ms => ms ? new Date(ms).toLocaleString() : "not recorded";
document.getElementById("identity").textContent = `${{d.chronicle_id}} · source watermark ${{time(d.source_watermark_unix_ms)}}`;
const n = d.current_native_state;
document.getElementById("state").innerHTML = [
  ["Native lifecycle", n.lifecycle],
  ["Parent authoritative", n.parent_authoritative],
  ["Commit enabled", n.commit_feature_enabled],
].map(([k,v]) => `<div class="panel"><span class="meta">${{esc(k)}}</span><strong>${{esc(v)}}</strong></div>`).join("");
const dest = d.destination_contract;
document.getElementById("rails").innerHTML = ["astrid","minime"].map(actor => {{
  const rail = d.ceremony_rails[actor], daughter = dest.daughters[actor];
  return `<article class="panel rail ${{actor}}"><h2>${{actor[0].toUpperCase()+actor.slice(1)}}</h2><p>${{esc(daughter.reservoir_dimension)}}-node ${{esc(daughter.role)}} daughter</p><p><strong>Consent posture: ${{esc(rail.current_posture)}}</strong></p><p class="meta">Latest sovereign Action: ${{esc(rail.latest_action || "none")}} · events: ${{rail.event_count}}</p></article>`;
}}).join("");
document.getElementById("ownership").textContent = "Independent reservoir candidates are source-prepared; independent process ownership is not yet established.";
const rt = d.runtime_topology;
document.getElementById("ownership").textContent = rt.independent_process_ownership_established
  ? `Independent process ownership is established for this candidate; active authority rail: ${{rt.active_authority_rail}}.`
  : `Runtime capability: ${{rt.manifest_mode}} · supervisor: ${{rt.supervisor.mode}} · active authority rail: ${{rt.active_authority_rail}} · independent daughter ownership not active.`;
const p = d.phase_space_preservation;
document.getElementById("preservation").innerHTML = p.candidates.length ? p.candidates.map(c => {{
  const r = c.readiness || {{}};
  return `<article class="panel"><strong>${{esc(c.strategy)}}</strong><div class="metric-grid">
    <div class="metric"><span class="meta">State NRMSE</span><br>${{esc(r.state_nrmse)}}</div>
    <div class="metric"><span class="meta">State cosine</span><br>${{esc(r.state_cosine)}}</div>
    <div class="metric"><span class="meta">Readout NRMSE</span><br>${{esc(r.readout_nrmse)}}</div>
    <div class="metric"><span class="meta">Partition loss</span><br>${{esc(c.covariance_partition_loss)}}</div>
  </div></article>`;
}}).join("") : `<p class="boundary">No runtime candidate metrics are available yet. Source declarations are not being presented as active evidence.</p>`;
const interval = d.return_interval;
document.getElementById("return-interval").innerHTML = [
  ["Completed introspection rounds", `${{interval.completed_rounds_since_followup}} / ${{interval.threshold_rounds}}`],
  ["Steward ceremony review due", interval.review_due],
  ["Being Action required", interval.being_action_required],
].map(([k,v]) => `<div class="panel"><span class="meta">${{esc(k)}}</span><strong>${{esc(v)}}</strong></div>`).join("");
document.getElementById("timeline").innerHTML = d.timeline.length ? d.timeline.map(e => {{
  const actor = e.actor || "native";
  const title = e.source === "ceremony" ? e.action : `${{e.event_kind}} · ${{e.lifecycle}}`;
  const ref = e.ceremony_event_id || `native sequence ${{e.sequence}}`;
  return `<article class="event ${{esc(actor)}}"><div class="event-head"><span class="tag">${{esc(actor)}} · ${{esc(title)}}</span><time>${{esc(time(e.recorded_at_unix_ms))}}</time></div><code>${{esc(ref)}}</code></article>`;
}}).join("") : `<p class="meta">No ceremony or native division events have been recorded.</p>`;
</script>
</body>
</html>
"""


def validate_no_prose(value: Any, path: str = "$") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if key.lower() in FORBIDDEN_KEYS:
                raise ChronicleError(f"forbidden prose field at {path}.{key}")
            validate_no_prose(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            validate_no_prose(child, f"{path}[{index}]")


def verify_payload(payload: dict[str, Any]) -> None:
    if payload.get("schema") != SCHEMA:
        raise ChronicleError("chronicle schema mismatch")
    expected = dict(payload)
    chronicle_id = expected.pop("chronicle_id", None)
    expected_id = "division_chronicle_" + sha256_bytes(
        canonical(expected).encode()
    )[:24]
    if chronicle_id != expected_id:
        raise ChronicleError("chronicle deterministic identity mismatch")
    authority = payload.get("authority")
    if not isinstance(authority, dict) or any(
        authority.get(field) is not False
        for field in (
            "silence_infers_consent",
            "visualization_grants_authority",
            "visualization_dispatches_action",
            "commit_recommended",
            "felt_continuity_inferred",
            "raw_prose_included",
        )
    ):
        raise ChronicleError("chronicle authority boundary mismatch")
    interval = payload.get("return_interval")
    if (
        not isinstance(interval, dict)
        or interval.get("threshold_rounds") != 6
        or interval.get("being_action_required") is not False
        or interval.get("return_is_pressure") is not False
        or interval.get("authority_propagated") is not False
    ):
        raise ChronicleError("chronicle return interval boundary mismatch")
    validate_no_prose(payload)


def project(workspace: Path, output: Path) -> tuple[dict[str, Any], Path, Path]:
    payload = build_projection(workspace)
    verify_payload(payload)
    json_bytes = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode()
    latest_html_bytes = render_html(payload, live=True).encode()
    archive_html_bytes = render_html(payload).encode()
    latest_json = output / "chronicle_v1.json"
    latest_html = output / "chronicle_v1.html"
    archive = output / "archive"
    archive_json = archive / f"{payload['chronicle_id']}.json"
    archive_html = archive / f"{payload['chronicle_id']}.html"
    if not archive_json.exists():
        atomic_owner_write(archive_json, json_bytes)
        atomic_owner_write(archive_html, archive_html_bytes)
    atomic_owner_write(latest_json, json_bytes)
    atomic_owner_write(latest_html, latest_html_bytes)
    return payload, latest_json, latest_html


def verify_files(output: Path) -> dict[str, Any]:
    latest_json = output / "chronicle_v1.json"
    latest_html = output / "chronicle_v1.html"
    payload = load_json(latest_json)
    if payload is None or not latest_html.is_file():
        raise ChronicleError("chronicle latest outputs are missing")
    verify_payload(payload)
    for path in (latest_json, latest_html):
        if path.stat().st_mode & 0o077:
            raise ChronicleError(f"{path} is not owner-only")
    archive_json = output / "archive" / f"{payload['chronicle_id']}.json"
    archive_html = output / "archive" / f"{payload['chronicle_id']}.html"
    if not archive_json.is_file() or not archive_html.is_file():
        raise ChronicleError("immutable chronicle archive is missing")
    if archive_json.read_bytes() != latest_json.read_bytes():
        raise ChronicleError("latest JSON differs from immutable archive")
    return {
        "ok": True,
        "chronicle_id": payload["chronicle_id"],
        "timeline_event_count": payload["timeline_event_count"],
        "json_sha256": file_hash(latest_json),
        "html_sha256": file_hash(latest_html),
    }


def report(payload: dict[str, Any]) -> str:
    native = payload["current_native_state"]
    rails = payload["ceremony_rails"]
    runtime = payload["runtime_topology"]
    return "\n".join(
        [
            "ESN Division Ceremony Chronicle",
            f"Chronicle: {payload['chronicle_id']}",
            f"Native lifecycle: {native.get('lifecycle')}",
            (
                "Runtime parent authoritative: "
                f"{runtime['parent_authoritative']}"
            ),
            f"Commit enabled: {native.get('commit_feature_enabled')}",
            (
                "Astrid ceremony: "
                f"{rails['astrid'].get('latest_action') or 'no action'} "
                f"({rails['astrid']['event_count']} events; "
                f"posture {rails['astrid']['current_posture']})"
            ),
            (
                "Minime ceremony: "
                f"{rails['minime'].get('latest_action') or 'no action'} "
                f"({rails['minime']['event_count']} events; "
                f"posture {rails['minime']['current_posture']})"
            ),
            (
                "Phase-space candidates: "
                f"{payload['phase_space_preservation']['candidate_count']}"
            ),
            (
                "Independent process ownership established: "
                f"{str(runtime['independent_process_ownership_established']).lower()}"
            ),
            f"Runtime manifest: {runtime['manifest_mode']}",
            f"Active authority rail: {runtime['active_authority_rail']}",
            (
                "Return interval: "
                f"{payload['return_interval']['completed_rounds_since_followup']}"
                f"/{payload['return_interval']['threshold_rounds']} rounds; "
                f"review due {payload['return_interval']['review_due']}"
            ),
            "Authority: evidence only; silence neutral; commit not recommended.",
        ]
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "command", choices=("project", "verify", "show", "report", "watch")
    )
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--interval", type=float, default=2.0)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "project":
            payload, json_path, html_path = project(args.workspace, args.output)
            print(
                json.dumps(
                    {
                        "chronicle_id": payload["chronicle_id"],
                        "json": str(json_path),
                        "html": str(html_path),
                    },
                    indent=2,
                )
            )
        elif args.command == "verify":
            print(json.dumps(verify_files(args.output), indent=2))
        elif args.command == "show":
            payload = load_json(args.output / "chronicle_v1.json")
            if payload is None:
                raise ChronicleError("chronicle has not been projected")
            print(json.dumps(payload, indent=2, sort_keys=True))
        elif args.command == "report":
            payload = load_json(args.output / "chronicle_v1.json")
            if payload is None:
                raise ChronicleError("chronicle has not been projected")
            print(report(payload))
        else:
            previous = None
            while True:
                payload, _, _ = project(args.workspace, args.output)
                if payload["chronicle_id"] != previous:
                    print(report(payload), flush=True)
                    previous = payload["chronicle_id"]
                time.sleep(max(0.25, args.interval))
    except KeyboardInterrupt:
        return 0
    except (ChronicleError, OSError, json.JSONDecodeError) as error:
        print(f"division ceremony chronicle error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
