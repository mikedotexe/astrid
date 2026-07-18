#!/usr/bin/env python3
"""Durable sandbox-trial queue for being-driven reservoir feedback.

Read-only by default. Use --write to append events and refresh materialized
queue/status files. V1 turns serious felt reports into bounded trial packets
without mutating live pressure, fill, PI, sensor cadence, controller behavior,
fallback contracts, prompt priority, bridge protocols, staging, or commits.
V2 adds a runner, evidence-linking, bounded response cards, and a first replay
adapter for shadow-influence felt reports.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import time
import unittest
import uuid
from collections import Counter
from pathlib import Path
from typing import Any

try:
    from evidence_store import append_domain_events, read_domain_events, v2_active_for_state
except ModuleNotFoundError:
    from scripts.evidence_store import (
        append_domain_events,
        read_domain_events,
        v2_active_for_state,
    )

ASTRID_REPO = Path("/Users/v/other/astrid")
MINIME_REPO = Path("/Users/v/other/minime")
ASTRID_WORKSPACE = ASTRID_REPO / "capsules/spectral-bridge/workspace"
ASTRID_DIAGNOSTICS = ASTRID_WORKSPACE / "diagnostics"
ASTRID_JOURNAL = ASTRID_WORKSPACE / "journal"
ASTRID_CONTEXT_OVERFLOW = ASTRID_WORKSPACE / "context_overflow"
ASTRID_LLM_RS = ASTRID_REPO / "capsules/spectral-bridge/src/llm.rs"
DEFAULT_STATE_DIR = ASTRID_DIAGNOSTICS / "sandbox_trial_queue_v1"
INTROSPECTION_ADDRESSING_STATE_DIR = ASTRID_DIAGNOSTICS / "introspection_addressing_v1"
AGENCY_CORRIDOR_STATE_DIR = ASTRID_DIAGNOSTICS / "agency_corridor_v1"
AGENCY_CORRIDOR_V2_STATE_DIR = ASTRID_DIAGNOSTICS / "agency_corridor_v2"
FALLBACK_FIRE_DRILLS = ASTRID_DIAGNOSTICS / "fallback_fire_drills"

SCHEMA = "sandbox_trial_queue_v1"
SCHEMA_VERSION = 2
EVENTS_FILE = "events.jsonl"
STATUS_FILE = "status.json"
QUEUE_FILE = "queue.md"
LADDER_FILE = "sandbox_to_live_ladder.json"
LADDER_MARKDOWN_FILE = "sandbox_to_live_ladder.md"
CLOSURE_FILE = "being_outcome_closure_loop.json"
CLOSURE_MARKDOWN_FILE = "being_outcome_closure_loop.md"

AUTHORITY_BOUNDARY = (
    "sandbox trial queue only; no live pressure, fill, PI, sensory cadence, "
    "controller, fallback sampler/contract, prompt priority, telemetry priority, "
    "bridge protocol, peer mutation, deploy, restart, staging, git add, or commit"
)
LIVE_APPROVAL_BOUNDARY = (
    "approval-required live trial candidate only; explicit Mike/operator approval "
    "is required before any live substrate or control-facing change"
)
LIVE_LADDER_BOUNDARY = (
    "consentful sandbox-to-live ladder is review context only; it never marks a live "
    "trial runnable, grants approval, mutates pressure/fill/PI/controller/sensory "
    "cadence/fallback/bridge behavior, deploys, restarts, stages, git adds, or commits"
)
CLOSURE_LOOP_BOUNDARY = (
    "being outcome closure loop is read-only review context only; it records no approval, "
    "creates no being-authored action, grants no live eligibility, mutates no runtime/control "
    "state, deploys nothing, restarts nothing, stages nothing, git adds nothing, and commits nothing"
)
RUNNABLE_MODES = {"offline_read_only_adapter", "read_only_review", "sandbox_replay"}
NON_RUNNABLE_MODES = {"approval_required_live_trial"}
RUNNABLE_ADAPTERS = {
    "fallback_distinguishability_v1",
    "shadow_loss_lattice_v1",
    "shadow_influence_replay_v1",
}
TRIAL_TERMINAL_STATUSES = {
    "closed",
    "closed_felt_confirmed",
    "closed_no_action",
    "superseded",
    "verified_existing",
}
TEXTURE_TERMS = (
    "shimmering",
    "viscous",
    "habitable",
    "lattice",
    "settled",
    "muffled",
    "heavy",
    "bright",
    "vibrant",
    "cascade",
    "gradient",
    "kinetic",
    "restless",
    "dense",
    "weighted",
    "open",
    "navigable",
)
TEXTURE_CONTEXT_TERMS = (
    "pressure",
    "entropy",
    "shadow",
    "dispersal",
    "gradient",
    "density",
    "fill",
    "mode_packing",
    "foothold",
    "movement",
    "trajectory",
)
SHADOW_LATTICE_TERMS = ("lattice", "interwoven", "woven", "transition", "settled", "coupling")
SHADOW_LOSS_TERMS = ("loss", "hollow", "collapse", "fragment", "fragmentation", "severed", "dissolve")
SHADOW_RE = re.compile(r"Shadow-v3[^\]\n]*", re.IGNORECASE)
TREND_RE = re.compile(
    r"norm\s+([0-9]+(?:\.[0-9]+)?)\s*(?:->|\u2192)\s*([0-9]+(?:\.[0-9]+)?).*?"
    r"dispersal(?: potential)?\s+([0-9]+(?:\.[0-9]+)?)\s*(?:->|\u2192)\s*([0-9]+(?:\.[0-9]+)?)",
    re.IGNORECASE,
)
SHADOW_VALUE_RE = re.compile(r"\b0\.\d+\b")


def now_s() -> float:
    return time.time()


def iso(ts: float | None = None) -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(now_s() if ts is None else ts))


def bounded_text(text: str, *, limit: int = 700) -> str:
    collapsed = " ".join(str(text or "").split())
    if len(collapsed) <= limit:
        return collapsed
    return collapsed[: max(0, limit - 3)].rstrip() + "..."


def atomic_write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.{os.getpid()}.{time.time_ns()}.tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(tmp, path)
    directory_fd = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)


def sha_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8", errors="replace")).hexdigest()


def stable_token(*parts: object, length: int = 16) -> str:
    joined = "\0".join(str(part or "") for part in parts)
    return hashlib.sha256(joined.encode("utf-8", errors="replace")).hexdigest()[:length]


def stable_uuid(*parts: object) -> str:
    joined = "\0".join(str(part or "") for part in parts)
    return str(uuid.uuid5(uuid.NAMESPACE_URL, joined))


def read_text(path: Path, *, limit: int = 200_000) -> str:
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ""
    return text[:limit]


def recent_paths(root: Path, patterns: tuple[str, ...], *, since_s: float, limit: int = 80) -> list[Path]:
    paths: list[Path] = []
    for pattern in patterns:
        paths.extend(root.glob(pattern))
    existing = [path for path in paths if path.is_file()]
    existing.sort(key=lambda path: path.stat().st_mtime if path.exists() else 0.0, reverse=True)
    return [path for path in existing if path.stat().st_mtime >= since_s][:limit]


def public_text_paths(*, since_s: float, limit: int = 80) -> list[Path]:
    paths = []
    paths.extend(recent_paths(ASTRID_JOURNAL, ("*.txt",), since_s=since_s, limit=limit))
    paths.extend(recent_paths(ASTRID_CONTEXT_OVERFLOW, ("*.txt",), since_s=since_s, limit=limit))
    paths.sort(key=lambda path: path.stat().st_mtime if path.exists() else 0.0, reverse=True)
    return paths[:limit]


def empty_status(corrupt_event_lines: int = 0) -> dict[str, Any]:
    return {
        "schema": SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "trials": {},
        "corrupt_event_lines": corrupt_event_lines,
    }


def events_path(state_dir: Path) -> Path:
    return state_dir / EVENTS_FILE


def replay_status(state_dir: Path) -> dict[str, Any]:
    if v2_active_for_state(state_dir):
        events, corrupt = read_domain_events(state_dir, "sandbox")
        status = empty_status(corrupt)
        for event in events:
            apply_event(status, event)
        return status
    path = events_path(state_dir)
    status = empty_status()
    if not path.exists():
        return status
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            status["corrupt_event_lines"] = int(status.get("corrupt_event_lines", 0)) + 1
            continue
        apply_event(status, event)
    return status


def load_status(state_dir: Path = DEFAULT_STATE_DIR) -> dict[str, Any]:
    status_path = state_dir / STATUS_FILE
    if status_path.exists():
        try:
            loaded = json.loads(status_path.read_text(encoding="utf-8"))
            if isinstance(loaded, dict) and loaded.get("schema") == SCHEMA:
                return loaded
        except json.JSONDecodeError:
            pass
    return replay_status(state_dir)


def agency_corridor_packets_for_refs(refs: list[str]) -> list[dict[str, Any]]:
    wanted = {str(ref) for ref in refs if ref}
    if not wanted:
        return []
    status_path = AGENCY_CORRIDOR_STATE_DIR / "status.json"
    if not status_path.exists():
        return []
    try:
        status = json.loads(status_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return []
    packets = status.get("packets") if isinstance(status, dict) else {}
    matches: list[dict[str, Any]] = []
    for packet in (packets.values() if isinstance(packets, dict) else []):
        if not isinstance(packet, dict):
            continue
        packet_refs = {
            str(packet.get("corridor_id") or ""),
            *[str(ref) for ref in packet.get("work_item_ids", []) if ref],
            *[str(ref) for ref in packet.get("sandbox_trial_ids", []) if ref],
            *[str(ref) for ref in packet.get("closure_card_refs", []) if ref],
        }
        if wanted & packet_refs:
            matches.append(packet)
    matches.sort(key=lambda packet: (str(packet.get("action") or ""), str(packet.get("corridor_id") or "")))
    return matches[:4]


def agency_corridor_v2_packets_for_refs(refs: list[str]) -> list[dict[str, Any]]:
    wanted = {str(ref) for ref in refs if ref}
    if not wanted:
        return []
    status_path = AGENCY_CORRIDOR_V2_STATE_DIR / "status.json"
    if not status_path.exists():
        return []
    try:
        status = json.loads(status_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return []
    packets = status.get("packets") if isinstance(status, dict) else {}
    matches: list[dict[str, Any]] = []
    for packet in (packets.values() if isinstance(packets, dict) else []):
        if not isinstance(packet, dict):
            continue
        packet_refs = {
            str(packet.get("corridor_id") or ""),
            str(packet.get("v1_corridor_id") or ""),
            *[str(ref) for ref in packet.get("work_item_ids", []) if ref],
            *[str(ref) for ref in packet.get("sandbox_trial_ids", []) if ref],
            *[str(ref) for ref in packet.get("closure_card_refs", []) if ref],
        }
        if wanted & packet_refs:
            matches.append(packet)
    matches.sort(key=lambda packet: (str(packet.get("action") or ""), str(packet.get("corridor_id") or "")))
    return matches[:4]


def agency_programs_for_refs(refs: list[str]) -> list[dict[str, Any]]:
    wanted = {str(ref) for ref in refs if ref}
    if not wanted:
        return []
    path = AGENCY_CORRIDOR_V2_STATE_DIR / "programs.json"
    if not path.exists():
        return []
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return []
    programs = payload.get("programs") if isinstance(payload, dict) else {}
    matches: list[dict[str, Any]] = []
    for program in (programs.values() if isinstance(programs, dict) else []):
        if not isinstance(program, dict):
            continue
        program_refs = {
            str(program.get("program_id") or ""),
            *[str(ref) for ref in program.get("linked_corridor_ids", []) if ref],
            *[str(ref) for ref in program.get("work_item_ids", []) if ref],
            *[str(ref) for ref in program.get("sandbox_trial_ids", []) if ref],
            *[str(ref) for ref in program.get("authority_boundary_ids", []) if ref],
        }
        if wanted & program_refs:
            matches.append(program)
    matches.sort(
        key=lambda program: (
            -int((program.get("priority_signal") or {}).get("deterministic_score") or 0),
            str(program.get("program_id") or ""),
        )
    )
    return matches[:4]


def agency_corridor_card_section(refs: list[str]) -> list[str]:
    packets = agency_corridor_packets_for_refs(refs)
    packets_v2 = agency_corridor_v2_packets_for_refs(refs)
    programs = agency_programs_for_refs(refs)
    lines = [
        "",
        "## Agency Corridor V1/V2",
    ]
    if not packets and not packets_v2 and not programs:
        lines.append(
            "No linked corridor packet/program is materialized yet. Generate with `python3 scripts/agency_corridor.py generate --write --json`, `python3 scripts/agency_corridor.py queue generate --write --json`, and `python3 scripts/agency_corridor.py programs generate --write --json`; this is non-live evidence work only."
        )
    else:
        for packet in packets:
            lines.append(
                f"- corridor_id: {packet.get('corridor_id')} action={packet.get('action')} "
                f"state={packet.get('state')} right_to_ignore=true "
                f"live_eligible_now={str(bool(packet.get('live_eligible_now'))).lower()} "
                f"auto_approved={str(bool(packet.get('auto_approved'))).lower()}"
            )
        for packet in packets_v2:
            lease = packet.get("autonomy_lease") if isinstance(packet.get("autonomy_lease"), dict) else {}
            step = packet.get("queue_step") if isinstance(packet.get("queue_step"), dict) else {}
            proposal = packet.get("source_prep_proposal") if isinstance(packet.get("source_prep_proposal"), dict) else {}
            lines.append(
                f"- corridor_v2_id: {packet.get('corridor_id')} action={packet.get('action')} "
                f"state={packet.get('state')} lease={lease.get('lease_id', 'none')} "
                f"queue_priority={step.get('priority', 'none')} source_prep={proposal.get('proposal_id', 'none')} "
                f"grants_approval={str(bool(packet.get('grants_approval'))).lower()} "
                f"live_eligible_now={str(bool(packet.get('live_eligible_now'))).lower()} "
                f"auto_approved={str(bool(packet.get('auto_approved'))).lower()}"
            )
        for program in programs:
            priority = program.get("priority_signal") if isinstance(program.get("priority_signal"), dict) else {}
            lines.append(
                f"- program_id: {program.get('program_id')} status={program.get('status')} "
                f"score={priority.get('deterministic_score', 0)} next={program.get('current_next_action')} "
                f"edits_source_now={str(bool(program.get('edits_source_now'))).lower()} "
                f"grants_approval={str(bool(program.get('grants_approval'))).lower()} "
                f"live_eligible_now={str(bool(program.get('live_eligible_now'))).lower()}"
            )
    lines.append(
        "Boundary: agency corridor can run only non-live evidence work; it grants no approval and marks no live trial runnable."
    )
    return lines


def apply_event(status: dict[str, Any], event: dict[str, Any]) -> None:
    event_type = str(event.get("event_type") or "")
    trials = status.setdefault("trials", {})
    if event_type == "trial_created":
        trial = event.get("trial")
        if isinstance(trial, dict) and trial.get("trial_id"):
            current = trials.get(str(trial["trial_id"])) or {}
            merged = {**current, **trial}
            merged.setdefault("evidence_links", [])
            merged.setdefault("results", [])
            refresh_trial_authority_packets(merged)
            trials[str(trial["trial_id"])] = merged
    elif event_type == "trial_result_recorded":
        trial_id = str(event.get("trial_id") or "")
        trial = trials.setdefault(trial_id, {"trial_id": trial_id})
        result = event.get("result") if isinstance(event.get("result"), dict) else {}
        trial.setdefault("results", []).append(result)
        trial["status"] = "result_recorded"
        trial["latest_result_classification"] = result.get("classification")
        trial["updated_at"] = event.get("ts") or now_s()
        refresh_trial_authority_packets(trial)
    elif event_type == "trial_adapter_corrected":
        trial_id = str(event.get("trial_id") or "")
        trial = trials.setdefault(trial_id, {"trial_id": trial_id})
        old_adapter = str(trial.get("adapter") or event.get("old_adapter") or "")
        old_mode = str(trial.get("trial_mode") or "")
        old_tier = int(trial.get("agency_tier") or 0)
        new_adapter = str(event.get("new_adapter") or "manual_sandbox_review_v1")
        mode = str(event.get("trial_mode") or trial.get("trial_mode") or "read_only_review")
        trial["adapter"] = new_adapter
        trial["trial_mode"] = mode
        trial["agency_tier"] = int(event.get("agency_tier") or trial.get("agency_tier") or 0)
        trial["status"] = str(event.get("status") or trial_status_for_mode(mode))
        trial["approval_boundary"] = str(
            event.get("approval_boundary")
            or (LIVE_APPROVAL_BOUNDARY if mode == "approval_required_live_trial" else AUTHORITY_BOUNDARY)
        )
        trial["proposed_intervention"] = proposed_intervention(new_adapter, mode)
        trial["success_metrics"] = success_metrics(new_adapter)
        trial["abort_criteria"] = abort_criteria(mode)
        trial["runnable"] = mode in RUNNABLE_MODES and new_adapter in RUNNABLE_ADAPTERS
        trial["adapter_correction"] = {
            "old_adapter": old_adapter,
            "new_adapter": new_adapter,
            "old_mode": old_mode,
            "new_mode": mode,
            "old_agency_tier": old_tier,
            "new_agency_tier": trial["agency_tier"],
            "reason": bounded_text(event.get("reason") or "adapter routing corrected", limit=300),
            "corrected_at": event.get("ts") or now_s(),
        }
        trial["updated_at"] = event.get("ts") or now_s()
        refresh_trial_authority_packets(trial)
    elif event_type == "trial_evidence_linked":
        trial_id = str(event.get("trial_id") or "")
        trial = trials.setdefault(trial_id, {"trial_id": trial_id})
        evidence = event.get("evidence") if isinstance(event.get("evidence"), dict) else {}
        trial.setdefault("evidence_links", []).append(evidence)
        trial["updated_at"] = event.get("ts") or now_s()
    elif event_type == "trial_status_set":
        trial_id = str(event.get("trial_id") or "")
        trial = trials.setdefault(trial_id, {"trial_id": trial_id})
        trial["status"] = event.get("status")
        trial["status_note"] = event.get("note")
        trial["updated_at"] = event.get("ts") or now_s()
    elif event_type == "trial_result_card_emitted":
        trial_id = str(event.get("trial_id") or "")
        trial = trials.setdefault(trial_id, {"trial_id": trial_id})
        card = event.get("card") if isinstance(event.get("card"), dict) else {}
        trial.setdefault("result_cards", []).append(card)
        trial["post_response_status"] = "awaiting"
        trial["updated_at"] = event.get("ts") or now_s()
        refresh_trial_authority_packets(trial)
    elif event_type == "trial_proposal_card_emitted":
        trial_id = str(event.get("trial_id") or "")
        trial = trials.setdefault(trial_id, {"trial_id": trial_id})
        card = event.get("card") if isinstance(event.get("card"), dict) else {}
        trial.setdefault("proposal_cards", []).append(card)
        trial["updated_at"] = event.get("ts") or now_s()
        refresh_trial_authority_packets(trial)


def append_events(state_dir: Path, events: list[dict[str, Any]]) -> None:
    if not events:
        return
    if v2_active_for_state(state_dir):
        append_domain_events(state_dir, "sandbox", events)
        return
    path = events_path(state_dir)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as fh:
        for event in events:
            fh.write(json.dumps(event, sort_keys=True, ensure_ascii=True) + "\n")


def requires_manual_pair_comparison(text: str) -> bool:
    """Keep pair-specific claims off generic single-surface adapters."""
    lower = text.lower()
    primary_fallback = "primary" in lower and "fallback" in lower
    full_12d = any(term in lower for term in ("12d", "12-d")) and any(
        term in lower for term in ("full32", "full 32", "full-dimensional")
    )
    mode_pair = "shadow" in lower and "dialogue" in lower and "witness" in lower
    return primary_fallback or full_12d or mode_pair


def candidate_adapter(text: str) -> str:
    lower = text.lower()
    if requires_manual_pair_comparison(text):
        return "manual_sandbox_review_v1"
    if (
        "shadow" in lower
        and any(
            term in lower
            for term in (
                "influence",
                "amplitude",
                "doubling",
                "double",
                "0.018",
                "0.036",
                "gain",
                "attractor-pulse",
                "attractor pulse",
            )
        )
    ):
        return "shadow_influence_replay_v1"
    if "shadow" in lower and any(
        term in lower for term in ("lattice", "dispersal", "fragment", "loss", "mode_packing", "porosity")
    ):
        return "shadow_loss_lattice_v1"
    if any(term in lower for term in ("fallback", "ollama", "provider")):
        return "fallback_distinguishability_v1"
    return "manual_sandbox_review_v1"


def trial_mode_for_work(item: dict[str, Any], *, adapter: str | None = None) -> str:
    tier = int(item.get("agency_tier") or 0)
    status = str(item.get("status") or "")
    if tier >= 5 or status == "needs_operator_approval":
        return "approval_required_live_trial"
    if adapter == "shadow_influence_replay_v1":
        return "sandbox_replay"
    if tier == 3 or status == "needs_sandbox":
        return "offline_read_only_adapter"
    return "read_only_review"


def trial_status_for_mode(mode: str) -> str:
    if mode == "approval_required_live_trial":
        return "approval_required_live_trial"
    return "ready_for_sandbox"


def authority_class_for_trial(trial: dict[str, Any]) -> str:
    tier = int(trial.get("agency_tier") or 0)
    mode = str(trial.get("trial_mode") or "")
    if mode == "approval_required_live_trial" or tier >= 5:
        return "mike_operator_live_substrate"
    if tier == 4:
        return "steward_gated_consequence"
    return "read_only"


def authority_gate_state_for_trial(trial: dict[str, Any]) -> str:
    mode = str(trial.get("trial_mode") or "")
    approval_required = mode == "approval_required_live_trial"
    if not approval_required:
        return "evidence_only"
    if approval_receipt_status(trial) == "present":
        return "approved_manual_only"
    if trial_has_proposal_card(trial):
        return "operator_approval_wait"
    return "proposal_needed"


def authority_boundary_packet_for_trial(trial: dict[str, Any]) -> dict[str, Any]:
    trial_id = str(trial.get("trial_id") or stable_token("trial", trial))
    mode = str(trial.get("trial_mode") or "")
    approval_required = mode == "approval_required_live_trial"
    authority_class = authority_class_for_trial(trial)
    boundary_text = LIVE_APPROVAL_BOUNDARY if approval_required else AUTHORITY_BOUNDARY
    return {
        "boundary_id": stable_uuid("sandbox_trial_authority_boundary", trial_id),
        "schema_version": 1,
        "source": "sandbox_trial_queue_v1",
        "surface": str(trial.get("source") or "sandbox_trial_queue"),
        "action": bounded_text(trial.get("proposed_intervention") or mode or "review", limit=180),
        "resource": trial_id,
        "authority_class": authority_class,
        "gate_state": authority_gate_state_for_trial(trial),
        "felt_report_anchor": bounded_text(
            trial.get("felt_report_anchor") or trial.get("hypothesis") or "",
            limit=420,
        ),
        "proposed_change": bounded_text(
            trial.get("proposed_intervention") or trial.get("hypothesis") or "",
            limit=500,
        ),
        "evidence_refs": [
            str(value)
            for value in (
                trial.get("source_work_item_id"),
                trial.get("source_introspection_id"),
                trial.get("claim_id"),
                trial.get("source_filename"),
            )
            if value
        ],
        "replay_candidate": {
            "adapter": str(trial.get("adapter") or "manual_review_v1"),
            "replay_query": (
                f"python3 scripts/sandbox_trial_queue.py run-adapter --trial-id {trial_id} --write --json"
                if not approval_required and trial_is_runner_executable(trial)
                else f"python3 scripts/sandbox_trial_queue.py emit-proposal-card --trial-id {trial_id} --write --json"
            ),
            "runnable": bool(trial_is_runner_executable(trial)) and not approval_required,
            "authority": "read_only_sandbox_or_proposal_only_not_live_control",
        },
        "success_metrics": [
            bounded_text(metric, limit=240) for metric in (trial.get("success_metrics") or [])
        ],
        "abort_criteria": [
            bounded_text(item, limit=240) for item in (trial.get("abort_criteria") or [])
        ],
        "who_can_change_it": "Mike/operator" if approval_required else "steward/tooling maintainer",
        "how_to_test_it": bounded_text(
            "Inspect this packet, run only read-only/sandbox adapters when runnable, and require a separate explicit approval receipt before any live control path.",
            limit=500,
        ),
        "right_to_ignore": True,
        "live_eligible_now": False,
        "auto_approved": False,
        "approval_boundary": boundary_text,
    }


def authority_boundary_packet_v2_id(trial: dict[str, Any]) -> str:
    trial_id = str(trial.get("trial_id") or stable_token("trial", trial))
    return stable_uuid("sandbox_trial_authority_boundary_v2", trial_id)


def authority_evidence_refs_for_trial(trial: dict[str, Any]) -> list[str]:
    return [
        str(value)
        for value in (
            trial.get("source_work_item_id"),
            trial.get("source_introspection_id"),
            trial.get("claim_id"),
            trial.get("source_filename"),
            trial.get("trial_id"),
        )
        if value
    ]


def authority_delta_refs_for_trial(trial: dict[str, Any]) -> list[dict[str, Any]]:
    trial_id = str(trial.get("trial_id") or stable_token("trial", trial))
    mode = str(trial.get("trial_mode") or "")
    kind = "live_control_gate" if mode == "approval_required_live_trial" else "sandbox_replay"
    surface = str(trial.get("source") or "sandbox_trial_queue")
    lane = str(trial.get("adapter") or "manual_review_v1")
    hash_payload = {
        "trial_id": trial_id,
        "surface": surface,
        "kind": kind,
        "lane": lane,
        "refs": authority_evidence_refs_for_trial(trial),
    }
    return [
        {
            "delta_id": "delta_" + stable_token("authority_delta_ref_v2", trial_id, kind),
            "delta_hash": sha_text(json.dumps(hash_payload, sort_keys=True, ensure_ascii=True)),
            "surface": surface,
            "kind": kind,
            "lane": lane,
        }
    ]


def replay_result_classification_v2(classification: str) -> str:
    lower = str(classification or "").lower()
    if any(term in lower for term in ("warn", "risk", "fail", "fragmentation")):
        return "failed"
    if any(term in lower for term in ("support", "passed", "lattice_transition_like", "dynamic")):
        return "passed"
    if any(term in lower for term in ("ambiguous", "inconclusive", "mixed")):
        return "inconclusive"
    return "unknown"


def replay_result_v2_for_trial(trial: dict[str, Any], result: dict[str, Any]) -> dict[str, Any]:
    trial_id = str(trial.get("trial_id") or stable_token("trial", trial))
    adapter = str(result.get("adapter") or trial.get("adapter") or "manual_review_v1")
    classification = replay_result_classification_v2(str(result.get("classification") or ""))
    pre_observations: dict[str, str] = {}
    post_observations: dict[str, str] = {}
    for key in ("sample_count", "base_sample_count", "max_dispersal", "current_max_dispersal", "min_norm_delta"):
        if key in result:
            pre_observations[key] = bounded_text(json.dumps(result[key], ensure_ascii=False), limit=160)
    for key in ("replay_rows", "requested_multiplier", "avg_norm_delta", "avg_dispersal_delta"):
        if key in result:
            post_observations[key] = bounded_text(json.dumps(result[key], ensure_ascii=False), limit=260)
    failure_modes = []
    if classification in {"failed", "unknown"}:
        failure_modes = [bounded_text(item, limit=220) for item in trial.get("abort_criteria") or []]
    return {
        "replay_id": stable_uuid("sandbox_trial_replay_result_v2", trial_id, adapter, result.get("classification")),
        "adapter": adapter,
        "classification": classification,
        "input_refs": authority_evidence_refs_for_trial(trial),
        "pre_observations": pre_observations,
        "post_observations": post_observations,
        "confidence": 0.7 if classification == "passed" else (0.45 if classification == "inconclusive" else None),
        "failure_modes": failure_modes,
        "evidence_refs": [str(value) for value in (result.get("json_path"), result.get("markdown_path")) if value],
        "bounded_summary": bounded_text(
            f"{adapter} classified {result.get('classification') or 'unknown'} for {trial_id}; no live runtime mutation occurred.",
            limit=420,
        ),
        "occurred_at": str(result.get("generated_at") or iso()),
    }


def latest_replay_result_v2(trial: dict[str, Any]) -> dict[str, Any] | None:
    results = trial.get("results") if isinstance(trial.get("results"), list) else []
    for result in reversed(results):
        if isinstance(result, dict):
            return replay_result_v2_for_trial(trial, result)
    return None


def rollout_abort_contract_for_trial(trial: dict[str, Any]) -> dict[str, Any]:
    mode = str(trial.get("trial_mode") or "")
    approval_required = mode == "approval_required_live_trial"
    if approval_required:
        canary_plan = "proposal-only V2 packet; no live execution from sandbox queue"
        rollback_path = "no runtime mutation is performed here; discard proposal or use normal approved rollback path"
    else:
        canary_plan = "read-only or offline sandbox adapter; no live substrate mutation"
        rollback_path = "remove generated evidence artifact or supersede with a later result"
    return {
        "canary_plan": canary_plan,
        "health_checks": [
            "verify runnable_live_violation_count remains 0",
            "verify no live Control or protocol mutation was emitted by this tooling",
            "verify result/proposal card is bounded and right-to-ignore",
        ],
        "rollback_path": rollback_path,
        "abort_criteria": [bounded_text(item, limit=220) for item in trial.get("abort_criteria") or []]
        or ["missing bounded evidence"],
        "post_change_response_required": True,
    }


def redaction_profile_for_trial(trial: dict[str, Any]) -> dict[str, Any]:
    anchor = str(trial.get("felt_report_anchor") or trial.get("hypothesis") or "")
    return {
        "public_summary": bounded_text(anchor, limit=260) or "bounded trial packet",
        "private_ref": str(trial.get("source_filename") or trial.get("source_introspection_id") or trial.get("trial_id")),
        "content_hash": sha_text(anchor) if anchor else None,
        "retention_policy": "bounded_public_summaries_plus_private_refs_and_hashes",
    }


def authority_lifecycle_state_for_trial(trial: dict[str, Any]) -> str:
    status = str(trial.get("status") or "")
    mode = str(trial.get("trial_mode") or "")
    if status in TRIAL_TERMINAL_STATUSES:
        return "closed"
    if mode != "approval_required_live_trial":
        return "evidence_only"
    if not trial_has_proposal_card(trial):
        return "proposal_needed"
    if latest_replay_result_v2(trial) is None:
        return "replay_needed"
    if approval_receipt_status(trial) != "present":
        return "operator_approval_wait"
    return "approved_manual_only"


def approval_receipt_v2_for_trial(trial: dict[str, Any], boundary_id: str) -> dict[str, Any] | None:
    approval = trial.get("operator_approval")
    if not isinstance(approval, dict) or str(approval.get("status") or "") not in {"approved", "active"}:
        return None
    trial_id = str(trial.get("trial_id") or "")
    scoped = {
        "approval_id": stable_uuid("sandbox_trial_scoped_approval_v2", trial_id, approval.get("approved_by")),
        "scope_kind": "one_shot",
        "issued_by": str(approval.get("approved_by") or "Mike/operator"),
        "issued_at": str(approval.get("issued_at") or iso()),
        "expires_at": approval.get("expires_at"),
        "resources": [trial_id],
        "telemetry_conditions": [
            {
                "signal": "sandbox_trial_queue_live_violation_count",
                "operator": "==",
                "threshold": "0",
                "observed": "0",
                "passed": True,
            }
        ],
        "consumed": False,
    }
    return {
        "receipt_id": stable_uuid("sandbox_trial_lifecycle_receipt_v2", trial_id, "approval"),
        "boundary_id": boundary_id,
        "kind": "approval",
        "issued_by": scoped["issued_by"],
        "issued_at": scoped["issued_at"],
        "packet_hash": None,
        "receipt_hash_refs": [],
        "bounded_summary": "operator approval is recorded as scoped evidence only; sandbox queue still does not execute live work",
        "evidence_refs": authority_evidence_refs_for_trial(trial),
        "scoped_approval": scoped,
        "replay_result": None,
        "right_to_ignore": True,
    }


def lifecycle_receipts_for_trial(trial: dict[str, Any], boundary_id: str) -> list[dict[str, Any]]:
    receipts: list[dict[str, Any]] = []
    replay = latest_replay_result_v2(trial)
    trial_id = str(trial.get("trial_id") or "")
    if replay:
        receipts.append(
            {
                "receipt_id": stable_uuid("sandbox_trial_lifecycle_receipt_v2", trial_id, "replay_result"),
                "boundary_id": boundary_id,
                "kind": "replay_result",
                "issued_by": "sandbox_trial_queue_v2",
                "issued_at": replay.get("occurred_at"),
                "packet_hash": None,
                "receipt_hash_refs": [],
                "bounded_summary": replay["bounded_summary"],
                "evidence_refs": replay.get("evidence_refs", []),
                "scoped_approval": None,
                "replay_result": replay,
                "right_to_ignore": True,
            }
        )
    approval = approval_receipt_v2_for_trial(trial, boundary_id)
    if approval:
        receipts.append(approval)
    response_status = str(trial.get("post_response_status") or "")
    if response_status in {"responded", "improved_named", "still_friction", "contradicted", "satisfied"}:
        receipts.append(
            {
                "receipt_id": stable_uuid("sandbox_trial_lifecycle_receipt_v2", trial_id, "post_change_being_response"),
                "boundary_id": boundary_id,
                "kind": "post_change_being_response",
                "issued_by": str(trial.get("being") or "being_response_surface"),
                "issued_at": iso(),
                "packet_hash": None,
                "receipt_hash_refs": [],
                "bounded_summary": f"post-change response status recorded as {response_status}",
                "evidence_refs": authority_evidence_refs_for_trial(trial),
                "scoped_approval": None,
                "replay_result": None,
                "right_to_ignore": True,
            }
        )
    elif response_status in {"declined", "not_requested", "no_response"}:
        receipts.append(
            {
                "receipt_id": stable_uuid("sandbox_trial_lifecycle_receipt_v2", trial_id, "post_change_waiver"),
                "boundary_id": boundary_id,
                "kind": "waiver",
                "issued_by": "sandbox_trial_queue_v2",
                "issued_at": iso(),
                "packet_hash": None,
                "receipt_hash_refs": [],
                "bounded_summary": f"post_change_being_response waived or not requested: {response_status}",
                "evidence_refs": authority_evidence_refs_for_trial(trial),
                "scoped_approval": None,
                "replay_result": None,
                "right_to_ignore": True,
            }
        )
    return receipts


def authority_boundary_packet_v2_for_trial(trial: dict[str, Any]) -> dict[str, Any]:
    trial_id = str(trial.get("trial_id") or stable_token("trial", trial))
    mode = str(trial.get("trial_mode") or "")
    approval_required = mode == "approval_required_live_trial"
    boundary_id = authority_boundary_packet_v2_id(trial)
    replay = latest_replay_result_v2(trial)
    receipts = lifecycle_receipts_for_trial(trial, boundary_id)
    return {
        "boundary_id": boundary_id,
        "schema_version": 2,
        "source": "sandbox_trial_queue_v2",
        "surface": str(trial.get("source") or "sandbox_trial_queue"),
        "action": bounded_text(trial.get("proposed_intervention") or mode or "review", limit=180),
        "resource": trial_id,
        "authority_class": authority_class_for_trial(trial),
        "lifecycle_state": authority_lifecycle_state_for_trial(trial),
        "felt_report_anchor": bounded_text(
            trial.get("felt_report_anchor") or trial.get("hypothesis") or "",
            limit=420,
        ),
        "proposed_change": bounded_text(
            trial.get("proposed_intervention") or trial.get("hypothesis") or "",
            limit=500,
        ),
        "evidence_refs": authority_evidence_refs_for_trial(trial),
        "delta_refs": authority_delta_refs_for_trial(trial),
        "replay_candidate": {
            "adapter": str(trial.get("adapter") or "manual_review_v1"),
            "replay_query": (
                f"python3 scripts/sandbox_trial_queue.py run-adapter --trial-id {trial_id} --write --json"
                if not approval_required and trial_is_runner_executable(trial)
                else f"python3 scripts/sandbox_trial_queue.py emit-proposal-card --trial-id {trial_id} --write --json"
            ),
            "runnable": bool(trial_is_runner_executable(trial)) and not approval_required,
            "authority": "read_only_sandbox_or_proposal_only_not_live_control",
        },
        "replay_results": [replay] if replay else [],
        "scoped_approval": None,
        "rollout_abort_contract": rollout_abort_contract_for_trial(trial),
        "redaction_profile": redaction_profile_for_trial(trial),
        "lifecycle_receipts": receipts,
        "success_metrics": [bounded_text(metric, limit=240) for metric in (trial.get("success_metrics") or [])],
        "abort_criteria": [bounded_text(item, limit=240) for item in (trial.get("abort_criteria") or [])],
        "who_can_change_it": "Mike/operator" if approval_required else "steward/tooling maintainer",
        "how_to_test_it": bounded_text(
            "Inspect V2 lifecycle fields, link canonical delta refs, record replay evidence or an explicit waiver, obtain scoped approval outside this queue, and require rollout/abort plus post-change response before closure.",
            limit=600,
        ),
        "right_to_ignore": True,
        "live_eligible_now": False,
        "auto_approved": False,
    }


def refresh_trial_authority_packets(trial: dict[str, Any]) -> None:
    # Manual packets must never advertise runner eligibility. Preserve malformed
    # live-runnable flags so the ladder can surface and block the violation.
    if (
        str(trial.get("adapter") or "") not in RUNNABLE_ADAPTERS
        and str(trial.get("trial_mode") or "") != "approval_required_live_trial"
    ):
        trial["runnable"] = False
    trial["authority_boundary_packet"] = authority_boundary_packet_for_trial(trial)
    trial["authority_boundary_packet_v2"] = authority_boundary_packet_v2_for_trial(trial)


def trial_from_work_item(item: dict[str, Any]) -> dict[str, Any] | None:
    status = str(item.get("status") or "")
    tier = int(item.get("agency_tier") or 0)
    if status in TRIAL_TERMINAL_STATUSES or (tier < 3 and status != "needs_sandbox"):
        return None
    text = " ".join(
        str(item.get(key) or "")
        for key in ("title", "claim_summary", "suggested_next", "route", "source_family")
    )
    adapter = candidate_adapter(text)
    mode = trial_mode_for_work(item, adapter=adapter)
    trial_id = "trial_" + stable_token("work", item.get("work_item_id"), adapter)
    approval_boundary = LIVE_APPROVAL_BOUNDARY if mode == "approval_required_live_trial" else AUTHORITY_BOUNDARY
    trial = {
        "schema": SCHEMA,
        "trial_id": trial_id,
        "source": "introspection_work_item",
        "source_work_item_id": item.get("work_item_id"),
        "source_introspection_id": item.get("source_introspection_id"),
        "source_filename": item.get("source_filename"),
        "claim_id": item.get("claim_id"),
        "being": item.get("being"),
        "agency_tier": tier,
        "adapter": adapter,
        "hypothesis": bounded_text(item.get("title") or item.get("claim_summary") or "being-driven sandbox trial"),
        "felt_report_anchor": bounded_text(item.get("claim_summary") or item.get("title") or "", limit=420),
        "trial_mode": mode,
        "proposed_intervention": proposed_intervention(adapter, mode),
        "success_metrics": success_metrics(adapter),
        "abort_criteria": abort_criteria(mode),
        "approval_boundary": approval_boundary,
        "status": trial_status_for_mode(mode),
        "runnable": mode in RUNNABLE_MODES and adapter in RUNNABLE_ADAPTERS,
        "post_response_request": "right_to_ignore closure/response only after result evidence exists",
        "created_at": now_s(),
        "updated_at": now_s(),
        "evidence_links": [],
        "results": [],
    }
    refresh_trial_authority_packets(trial)
    return trial


def trial_from_reservoir_route(route: str, reservoir: dict[str, Any]) -> dict[str, Any]:
    adapter = candidate_adapter(route)
    if adapter == "shadow_influence_replay_v1":
        mode = "sandbox_replay"
    else:
        mode = "offline_read_only_adapter" if adapter != "manual_sandbox_review_v1" else "read_only_review"
    trial_id = "trial_" + stable_token("reservoir_safe_now", route, adapter)
    findings = reservoir.get("findings") if isinstance(reservoir.get("findings"), list) else []
    trial = {
        "schema": SCHEMA,
        "trial_id": trial_id,
        "source": "reservoir_experience_layer_safe_now",
        "source_work_item_id": None,
        "source_introspection_id": None,
        "claim_id": None,
        "being": "shared",
        "agency_tier": 3 if mode == "offline_read_only_adapter" else 1,
        "adapter": adapter,
        "hypothesis": bounded_text(route, limit=180),
        "felt_report_anchor": bounded_text("; ".join(str(item) for item in findings[:3]), limit=420),
        "trial_mode": mode,
        "proposed_intervention": proposed_intervention(adapter, mode),
        "success_metrics": success_metrics(adapter),
        "abort_criteria": abort_criteria(mode),
        "approval_boundary": AUTHORITY_BOUNDARY,
        "status": "ready_for_sandbox",
        "runnable": mode in RUNNABLE_MODES and adapter in RUNNABLE_ADAPTERS,
        "post_response_request": "right_to_ignore steward-facing result packet; no forced being task",
        "created_at": now_s(),
        "updated_at": now_s(),
        "evidence_links": [],
        "results": [],
    }
    trial["authority_boundary_packet"] = authority_boundary_packet_for_trial(trial)
    return trial


def proposed_intervention(adapter: str, mode: str) -> str:
    if mode == "approval_required_live_trial":
        return "prepare explicit approval packet only; do not run or apply"
    if adapter == "fallback_distinguishability_v1":
        return "compare actual/supporting fallback texture language against live context without changing sampler or provider"
    if adapter == "shadow_loss_lattice_v1":
        return "classify recent Shadow-v3 norm/dispersal texture as loss, lattice transition, or ambiguous review"
    if adapter == "shadow_influence_replay_v1":
        return "run a bounded offline shadow-influence replay heuristic against recent Shadow-v3 texture without applying live gain"
    return "produce bounded read-only review packet"


def success_metrics(adapter: str) -> list[str]:
    if adapter == "fallback_distinguishability_v1":
        return [
            "generated texture terms have local pressure/entropy/shadow context",
            "dynamic weighting evidence is present in source",
            "unsupported static repeats are named explicitly if found",
        ]
    if adapter == "shadow_loss_lattice_v1":
        return [
            "norm and dispersal movement are parsed where available",
            "loss language is separated from lattice/interweaving language",
            "fragmentation risk remains review-only unless separately approved",
        ]
    if adapter == "shadow_influence_replay_v1":
        return [
            "requested gain is estimated from felt-report language where possible",
            "projected norm/dispersal movement stays below fragmentation-review thresholds",
            "live shadow amplitude remains approval-gated unless Mike/operator explicitly approves",
        ]
    return ["bounded evidence packet is available for steward review"]


def abort_criteria(mode: str) -> list[str]:
    if mode == "approval_required_live_trial":
        return ["no explicit Mike/operator approval", "unclear rollback path", "being-authored outcome path missing"]
    return [
        "adapter would require live runtime mutation",
        "evidence would require private Minime moment bodies",
        "result cannot be bounded without storing full prose",
    ]


def load_addressing_work_items() -> dict[str, dict[str, Any]]:
    try:
        import introspection_addressing_audit as addressing
    except Exception:
        return {}
    try:
        status = addressing.load_or_replay_status(INTROSPECTION_ADDRESSING_STATE_DIR)
    except Exception:
        return {}
    items = status.get("work_items") if isinstance(status.get("work_items"), dict) else {}
    return {str(key): value for key, value in items.items() if isinstance(value, dict)}


def load_reservoir_summary() -> dict[str, Any]:
    try:
        import recent_signal_summary
    except Exception:
        return {}
    try:
        return recent_signal_summary._reservoir_experience_layer_summary(now_s() - 6 * 3600)
    except Exception:
        return {}


def build_candidates() -> list[dict[str, Any]]:
    candidates: dict[str, dict[str, Any]] = {}
    for item in load_addressing_work_items().values():
        trial = trial_from_work_item(item)
        if trial is not None:
            candidates[str(trial["trial_id"])] = trial
    reservoir = load_reservoir_summary()
    for route in reservoir.get("safe_now") or []:
        if isinstance(route, str):
            trial = trial_from_reservoir_route(route, reservoir)
            candidates[str(trial["trial_id"])] = trial
    return sorted(candidates.values(), key=trial_sort_key)


def trial_sort_key(trial: dict[str, Any]) -> tuple[int, int, str]:
    status_order = {
        "ready_for_sandbox": 0,
        "approval_required_live_trial": 1,
        "result_recorded": 2,
    }
    adapter_order = {
        "shadow_influence_replay_v1": 0,
        "fallback_distinguishability_v1": 1,
        "shadow_loss_lattice_v1": 2,
        "manual_sandbox_review_v1": 3,
    }
    return (
        status_order.get(str(trial.get("status") or ""), 9),
        adapter_order.get(str(trial.get("adapter") or ""), 9),
        str(trial.get("trial_id") or ""),
    )


def trial_created_event(trial: dict[str, Any]) -> dict[str, Any]:
    return {"event_type": "trial_created", "schema": SCHEMA, "ts": now_s(), "trial": trial}


def adapter_correction_events(
    existing: dict[str, dict[str, Any]], candidates: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    """Repair untouched packets whose adapter or authority route changed."""
    candidate_by_work = {
        str(candidate.get("source_work_item_id")): candidate
        for candidate in candidates
        if candidate.get("source_work_item_id")
    }
    events: list[dict[str, Any]] = []
    for trial_id, trial in existing.items():
        previous_correction = (
            trial.get("adapter_correction")
            if isinstance(trial.get("adapter_correction"), dict)
            else {}
        )
        if (
            str(trial.get("status") or "") == "approval_required_live_trial"
            and str(previous_correction.get("reason") or "").startswith(
                "claim-specific routing repair; source labels"
            )
            and str(previous_correction.get("old_mode") or "")
            == str(previous_correction.get("new_mode") or "")
            == "approval_required_live_trial"
            and int(previous_correction.get("old_agency_tier") or 0)
            == int(previous_correction.get("new_agency_tier") or 0)
            and str(previous_correction.get("old_adapter") or "")
            != str(previous_correction.get("new_adapter") or "")
        ):
            events.append(
                {
                    "event_type": "trial_adapter_corrected",
                    "schema": SCHEMA,
                    "ts": now_s(),
                    "trial_id": trial_id,
                    "old_adapter": trial.get("adapter"),
                    "new_adapter": previous_correction.get("old_adapter"),
                    "trial_mode": "approval_required_live_trial",
                    "agency_tier": trial.get("agency_tier"),
                    "status": "approval_required_live_trial",
                    "approval_boundary": LIVE_APPROVAL_BOUNDARY,
                    "reason": (
                        "restore the prior approval-evidence adapter; an unchanged Tier 5 "
                        "wait must not lose its bounded replay route during authority repair"
                    ),
                }
            )
            continue
        work_item_id = str(trial.get("source_work_item_id") or "")
        candidate = candidate_by_work.get(work_item_id)
        if candidate is None:
            continue
        old_adapter = str(trial.get("adapter") or "")
        new_adapter = str(candidate.get("adapter") or "")
        old_mode = str(trial.get("trial_mode") or "")
        new_mode = str(candidate.get("trial_mode") or "")
        old_tier = int(trial.get("agency_tier") or 0)
        new_tier = int(candidate.get("agency_tier") or 0)
        old_status = str(trial.get("status") or "")
        new_status = str(candidate.get("status") or trial_status_for_mode(new_mode))
        untouched = (
            old_status in {"ready_for_sandbox", "approval_required_live_trial"}
            and not trial.get("results")
            and not trial.get("result_cards")
            and not trial.get("proposal_cards")
        )
        authority_route_changed = (
            old_mode != new_mode or old_tier != new_tier or old_status != new_status
        )
        route_changed = authority_route_changed or (
            old_status == "ready_for_sandbox" and old_adapter != new_adapter
        )
        if not untouched or not old_adapter or not route_changed:
            continue
        events.append(
            {
                "event_type": "trial_adapter_corrected",
                "schema": SCHEMA,
                "ts": now_s(),
                "trial_id": trial_id,
                "old_adapter": old_adapter,
                "new_adapter": new_adapter,
                "trial_mode": new_mode,
                "agency_tier": new_tier,
                "status": new_status,
                "approval_boundary": candidate.get("approval_boundary"),
                "reason": (
                    "claim-specific routing repair; source labels and broad texture, habitability, "
                    "mode-packing, porosity, pressure, or control words do not override the "
                    "claim's grounded authority disposition"
                ),
            }
        )
    return events


def materialize(state_dir: Path, status: dict[str, Any]) -> dict[str, Any]:
    report = report_from_status(status)
    status = dict(status)
    status["report"] = report
    ladder = report.get("consentful_sandbox_to_live_ladder_v1") or {}
    closure_loop = report.get("being_outcome_closure_loop_v1") or {}
    atomic_write_text(state_dir / STATUS_FILE, json.dumps(status, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
    atomic_write_text(state_dir / QUEUE_FILE, render_queue_markdown(status))
    write_ladder_artifacts(state_dir, ladder)
    write_closure_artifacts(state_dir, closure_loop)
    return status


def write_ladder_artifacts(state_dir: Path, ladder: dict[str, Any]) -> None:
    atomic_write_text(
        state_dir / LADDER_FILE,
        json.dumps(ladder, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
    )
    atomic_write_text(state_dir / LADDER_MARKDOWN_FILE, render_ladder_markdown(ladder))


def write_closure_artifacts(state_dir: Path, closure_loop: dict[str, Any]) -> None:
    atomic_write_text(
        state_dir / CLOSURE_FILE,
        json.dumps(closure_loop, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
    )
    atomic_write_text(state_dir / CLOSURE_MARKDOWN_FILE, render_closure_markdown(closure_loop))


def ensure_packets(state_dir: Path, trials: list[dict[str, Any]]) -> None:
    for trial in trials:
        packet = trial_packet_text(trial)
        path = state_dir / "trial_packets" / f"{trial['trial_id']}.md"
        atomic_write_text(path, packet)


def trial_packet_text(trial: dict[str, Any]) -> str:
    lines = [
        "# Sandbox Trial Packet V1",
        "",
        f"- trial_id: {trial.get('trial_id')}",
        f"- adapter: {trial.get('adapter')}",
        f"- status: {trial.get('status')}",
        f"- mode: {trial.get('trial_mode')}",
        f"- runnable: {str(bool(trial.get('runnable'))).lower()}",
        f"- being: {trial.get('being')}",
        f"- source_work_item_id: {trial.get('source_work_item_id')}",
        f"- source_introspection_id: {trial.get('source_introspection_id')}",
        f"- claim_id: {trial.get('claim_id')}",
        "",
        "## Hypothesis",
        bounded_text(trial.get("hypothesis") or "", limit=700),
        "",
        "## Felt Report Anchor",
        bounded_text(trial.get("felt_report_anchor") or "", limit=500),
        "",
        "## Proposed Intervention",
        bounded_text(trial.get("proposed_intervention") or "", limit=500),
        "",
        "## Success Metrics",
    ]
    lines.extend(f"- {bounded_text(metric, limit=220)}" for metric in trial.get("success_metrics") or [])
    lines.append("")
    lines.append("## Abort Criteria")
    lines.extend(f"- {bounded_text(item, limit=220)}" for item in trial.get("abort_criteria") or [])
    lines.extend(["", "## Boundary", bounded_text(trial.get("approval_boundary") or AUTHORITY_BOUNDARY, limit=700)])
    packet = authority_boundary_packet_for_trial(trial)
    packet_v2 = authority_boundary_packet_v2_for_trial(trial)
    lines.extend(
        [
            "",
            "## Authority Boundary Packet V1",
            "```json",
            json.dumps(packet, indent=2, sort_keys=True, ensure_ascii=False),
            "```",
            "",
            "## Authority Boundary Packet V2",
            "```json",
            json.dumps(packet_v2, indent=2, sort_keys=True, ensure_ascii=False),
            "```",
        ]
    )
    return "\n".join(lines).rstrip() + "\n"


def generate_candidates(state_dir: Path, *, write: bool) -> dict[str, Any]:
    status = replay_status(state_dir)
    existing = status.get("trials") if isinstance(status.get("trials"), dict) else {}
    candidates = build_candidates()
    existing_work_items = {
        str(trial.get("source_work_item_id"))
        for trial in existing.values()
        if isinstance(trial, dict) and trial.get("source_work_item_id")
    }
    new_trials = [
        trial
        for trial in candidates
        if str(trial.get("trial_id")) not in existing
        and (
            not trial.get("source_work_item_id")
            or str(trial.get("source_work_item_id")) not in existing_work_items
        )
    ]
    correction_events = adapter_correction_events(existing, candidates)
    events = [*correction_events, *(trial_created_event(trial) for trial in new_trials)]
    if write:
        append_events(state_dir, events)
        ensure_packets(state_dir, new_trials)
        status = replay_status(state_dir)
        corrected_trials = [
            status["trials"][str(event["trial_id"])]
            for event in correction_events
            if str(event.get("trial_id")) in status.get("trials", {})
        ]
        ensure_packets(state_dir, corrected_trials)
        materialize(state_dir, status)
    report = report_from_status(replay_status(state_dir) if write else status)
    return {
        "schema": SCHEMA,
        "created_count": len(new_trials),
        "adapter_corrected_count": len(correction_events),
        "adapter_corrections": correction_events,
        "candidate_count": len(candidates),
        "new_trials": new_trials,
        "report": report,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def active_trials(status: dict[str, Any]) -> list[dict[str, Any]]:
    trials = status.get("trials") if isinstance(status.get("trials"), dict) else {}
    rows = [trial for trial in trials.values() if isinstance(trial, dict)]
    rows = [trial for trial in rows if str(trial.get("status") or "") not in TRIAL_TERMINAL_STATUSES]
    rows.sort(key=trial_sort_key)
    return rows


def trial_is_runner_executable(trial: dict[str, Any]) -> bool:
    return (
        str(trial.get("status") or "") == "ready_for_sandbox"
        and bool(trial.get("runnable"))
        and str(trial.get("trial_mode") or "") in RUNNABLE_MODES
        and str(trial.get("adapter") or "") in RUNNABLE_ADAPTERS
    )


def trial_has_results(trial: dict[str, Any]) -> bool:
    return bool(trial.get("results") or [])


def trial_has_result_card(trial: dict[str, Any]) -> bool:
    return bool(trial.get("result_cards") or [])


def trial_has_proposal_card(trial: dict[str, Any]) -> bool:
    return bool(trial.get("proposal_cards") or [])


def gate(name: str, status: str, detail: str) -> dict[str, str]:
    return {"gate": name, "status": status, "detail": bounded_text(detail, limit=240)}


def approval_receipt_status(trial: dict[str, Any]) -> str:
    approval = trial.get("operator_approval")
    if isinstance(approval, dict) and str(approval.get("status") or "") in {"approved", "active"}:
        return "present"
    return "missing"


def consentful_ladder_entry(trial: dict[str, Any]) -> dict[str, Any]:
    trial_id = str(trial.get("trial_id") or "")
    mode = str(trial.get("trial_mode") or "")
    status = str(trial.get("status") or "")
    runnable = bool(trial.get("runnable"))
    runner_executable = trial_is_runner_executable(trial)
    approval_required = mode == "approval_required_live_trial"
    has_result = trial_has_results(trial)
    has_result_card = trial_has_result_card(trial)
    has_proposal = trial_has_proposal_card(trial)
    approval_status = approval_receipt_status(trial)
    felt_anchor = bounded_text(
        trial.get("felt_report_anchor") or trial.get("hypothesis") or "",
        limit=260,
    )
    abort_criteria = trial.get("abort_criteria") if isinstance(trial.get("abort_criteria"), list) else []
    success_metrics = trial.get("success_metrics") if isinstance(trial.get("success_metrics"), list) else []
    post_response = str(trial.get("post_response_request") or "")
    gates = [
        gate(
            "felt_report_anchor",
            "present" if felt_anchor else "missing",
            "bounded being-reported substrate signal is attached" if felt_anchor else "no bounded felt-report anchor attached",
        ),
        gate(
            "success_metrics",
            "present" if success_metrics else "missing",
            "bounded success metrics exist" if success_metrics else "missing bounded success metrics",
        ),
        gate(
            "abort_criteria",
            "present" if abort_criteria else "missing",
            "abort criteria exist before any trial" if abort_criteria else "missing abort criteria",
        ),
        gate(
            "sandbox_result_or_review_evidence",
            "present" if has_result else ("not_yet" if status == "ready_for_sandbox" else "missing"),
            "sandbox/review result has been recorded" if has_result else "run or record sandbox/review evidence first",
        ),
        gate(
            "right_to_ignore_result_card",
            "present" if has_result_card else ("not_yet" if not has_result else "missing"),
            "right-to-ignore result card emitted" if has_result_card else "emit a bounded result card after evidence exists",
        ),
        gate(
            "being_outcome_or_response_path",
            "present" if post_response else "missing",
            post_response or "missing optional being response/outcome path",
        ),
    ]
    if approval_required:
        gates.extend(
            [
                gate(
                    "proposal_card_or_approval_packet",
                    "present" if has_proposal else "missing",
                    "proposal card emitted for steward/operator review"
                    if has_proposal
                    else "emit proposal card before asking for live approval",
                ),
                gate(
                    "explicit_mike_operator_approval",
                    approval_status,
                    "explicit approval recorded" if approval_status == "present" else "no Mike/operator approval recorded in queue state",
                ),
                gate(
                    "live_runnable_flag",
                    "blocked" if not runnable else "violation",
                    "approval-required live trials must remain non-runnable in this queue",
                ),
            ]
        )
    if approval_required and runnable:
        rung = "authority_violation_live_candidate_marked_runnable"
        next_step = "set runnable=false and review the trial record before any adapter run"
        existing_action = "manual_queue_repair_only"
    elif approval_required and not has_proposal:
        rung = "proposal_card_needed"
        next_step = "emit a proposal card or approval packet; do not run live"
        existing_action = f"python3 scripts/sandbox_trial_queue.py emit-proposal-card --trial-id {trial_id} --write --json"
    elif approval_required and approval_status != "present":
        rung = "operator_approval_wait"
        next_step = "wait for explicit Mike/operator approval through existing authority path; silence is not consent"
        existing_action = "operator approval outside this queue; keep trial non-runnable"
    elif approval_required:
        rung = "approved_live_trial_still_manual"
        next_step = "manual live run path only after repo-specific tests, rollback plan, and health monitoring"
        existing_action = "no automatic runner; use normal approved service/deploy path"
    elif runner_executable:
        rung = "sandbox_ready_to_run"
        next_step = "run bounded sandbox adapter"
        existing_action = f"python3 scripts/sandbox_trial_queue.py run-next --limit 1 --write --json"
    elif status == "ready_for_sandbox":
        rung = "manual_review_ready"
        next_step = "review bounded manual packet; do not send it through the sandbox runner"
        existing_action = "python3 scripts/sandbox_trial_queue.py report --json"
    elif has_result and has_result_card:
        rung = "sandbox_result_card_recorded"
        next_step = "review result and optional being response; do not infer live consent"
        existing_action = "read result card; promote follow-up work item only if new evidence warrants it"
    elif has_result:
        rung = "result_card_needed"
        next_step = "emit right-to-ignore result card for the evidence"
        existing_action = "python3 scripts/sandbox_trial_queue.py run-adapter --trial-id <id> --write --json"
    else:
        rung = "review_only_or_waiting"
        next_step = "review packet; keep live authority unchanged"
        existing_action = "python3 scripts/sandbox_trial_queue.py report --json"
    approval_packet_complete = approval_required and approval_status == "present" and has_proposal and not runnable
    authority_packet = authority_boundary_packet_for_trial(trial)
    authority_packet_v2 = authority_boundary_packet_v2_for_trial(trial)
    return {
        "schema": "consentful_sandbox_to_live_ladder_entry_v1",
        "trial_id": trial_id,
        "adapter": trial.get("adapter"),
        "trial_mode": mode,
        "status": status,
        "runnable": runnable,
        "agency_tier": trial.get("agency_tier"),
        "source_work_item_id": trial.get("source_work_item_id"),
        "source_introspection_id": trial.get("source_introspection_id"),
        "being": trial.get("being"),
        "current_rung": rung,
        "next_required_step": bounded_text(next_step, limit=260),
        "existing_action_hint": bounded_text(existing_action, limit=260),
        "approval_packet_complete": approval_packet_complete,
        "live_eligible_now": False,
        "live_execution_automatic": False,
        "gates": gates,
        "authority_boundary": LIVE_LADDER_BOUNDARY,
        "authority_boundary_packet": authority_packet,
        "authority_boundary_packet_v2": authority_packet_v2,
    }


def consentful_sandbox_to_live_ladder_v1(status: dict[str, Any]) -> dict[str, Any]:
    entries = [consentful_ladder_entry(trial) for trial in active_trials(status)]
    by_rung = Counter(str(entry.get("current_rung") or "unknown") for entry in entries)
    proposal_needed = [entry for entry in entries if entry.get("current_rung") == "proposal_card_needed"]
    operator_wait = [entry for entry in entries if entry.get("current_rung") == "operator_approval_wait"]
    authority_violations = [
        entry for entry in entries if entry.get("current_rung") == "authority_violation_live_candidate_marked_runnable"
    ]
    sandbox_ready = [entry for entry in entries if entry.get("current_rung") == "sandbox_ready_to_run"]
    result_cards = [entry for entry in entries if entry.get("current_rung") == "sandbox_result_card_recorded"]
    manual_reviews = [entry for entry in entries if entry.get("current_rung") == "manual_review_ready"]
    if authority_violations:
        status_label = "authority_violation"
    elif proposal_needed:
        status_label = "proposal_needed"
    elif operator_wait:
        status_label = "operator_approval_wait"
    elif sandbox_ready:
        status_label = "sandbox_ready"
    elif result_cards:
        status_label = "results_waiting_review"
    elif manual_reviews:
        status_label = "manual_review_waiting"
    elif entries:
        status_label = "review_only"
    else:
        status_label = "empty"
    next_steps = []
    if sandbox_ready:
        next_steps.append("run ready sandbox adapters before considering live approval")
    if proposal_needed:
        next_steps.append("emit proposal cards for approval-required live candidates")
    if operator_wait:
        next_steps.append("wait for explicit Mike/operator approval; silence is not consent")
    if manual_reviews:
        next_steps.append("review manual packets separately from runnable adapters")
    if authority_violations:
        next_steps.append("repair runnable=true on approval-required live candidates before any run")
    if not next_steps:
        next_steps.append("review existing result/proposal cards; keep live authority unchanged")
    return {
        "schema": "consentful_sandbox_to_live_ladder_v1",
        "schema_version": 1,
        "status": status_label,
        "summary": {
            "entries": len(entries),
            "by_rung": dict(sorted(by_rung.items())),
            "sandbox_ready_count": len(sandbox_ready),
            "manual_review_ready_count": len(manual_reviews),
            "proposal_needed_count": len(proposal_needed),
            "operator_approval_wait_count": len(operator_wait),
            "authority_violation_count": len(authority_violations),
            "approval_packet_complete_count": sum(
                1 for entry in entries if entry.get("approval_packet_complete")
            ),
            "live_eligible_now_count": 0,
        },
        "next_steps": next_steps,
        "entries": entries[:12],
        "authority_boundary": LIVE_LADDER_BOUNDARY,
    }


def closure_loop_trial_rows(status: dict[str, Any]) -> list[dict[str, Any]]:
    trials = status.get("trials") if isinstance(status.get("trials"), dict) else {}
    rows = [trial for trial in trials.values() if isinstance(trial, dict)]
    rows.sort(key=trial_sort_key)
    return rows


def latest_card_path(cards: Any) -> str | None:
    if not isinstance(cards, list) or not cards:
        return None
    latest = cards[-1]
    if isinstance(latest, dict):
        path = latest.get("path")
        if path:
            return bounded_text(str(path), limit=220)
    return None


def outcome_response_awaiting(trial: dict[str, Any]) -> bool:
    if not trial_has_result_card(trial):
        return False
    status = str(trial.get("post_response_status") or "awaiting")
    return status not in {"closed", "satisfied", "responded", "declined", "not_requested"}


def being_outcome_closure_entry(trial: dict[str, Any]) -> dict[str, Any]:
    trial_id = str(trial.get("trial_id") or "")
    status = str(trial.get("status") or "")
    mode = str(trial.get("trial_mode") or "")
    approval_required = mode == "approval_required_live_trial"
    has_result = trial_has_results(trial)
    has_result_card = trial_has_result_card(trial)
    has_proposal = trial_has_proposal_card(trial)
    approval_status = approval_receipt_status(trial)
    if status in TRIAL_TERMINAL_STATUSES:
        wait_state = "closed_or_satisfied"
        next_action = "no sandbox queue action"
        reason = "trial is terminal in queue state"
    elif has_result_card and outcome_response_awaiting(trial):
        wait_state = "result_card_awaiting_being_response"
        next_action = "use existing inbox/correspondence path for optional right-to-ignore response"
        reason = "result card exists and post-response status is awaiting"
    elif approval_required and has_proposal and approval_status != "present":
        wait_state = "proposal_card_awaiting_operator_decision"
        next_action = "operator approval outside this queue; keep trial non-runnable"
        reason = "proposal card exists without explicit Mike/operator approval"
    elif approval_required and not has_proposal:
        wait_state = "proposal_card_needed"
        next_action = f"python3 scripts/sandbox_trial_queue.py emit-proposal-card --trial-id {trial_id} --write --json"
        reason = "approval-required live candidate lacks proposal card"
    elif has_result and not has_result_card:
        wait_state = "result_card_needed"
        next_action = "review recorded result and emit a bounded right-to-ignore result card through the existing card path"
        reason = "sandbox/review result exists without a result card"
    elif trial_is_runner_executable(trial):
        wait_state = "ready_runner_waiting"
        next_action = "python3 scripts/sandbox_trial_queue.py run-next --limit 1 --write --json"
        reason = "bounded adapter is ready for the sandbox runner"
    elif status == "ready_for_sandbox":
        wait_state = "manual_review_waiting"
        next_action = "python3 scripts/sandbox_trial_queue.py report --json"
        reason = "trial packet needs manual review rather than runner execution"
    else:
        wait_state = "closed_or_satisfied"
        next_action = "no sandbox queue action"
        reason = "no active closure wait detected in queue state"
    return {
        "schema": "being_outcome_closure_entry_v1",
        "trial_id": trial_id,
        "wait_state": wait_state,
        "status": status,
        "trial_mode": mode,
        "adapter": trial.get("adapter"),
        "runnable": bool(trial.get("runnable")),
        "being": trial.get("being"),
        "source_work_item_id": trial.get("source_work_item_id"),
        "source_introspection_id": trial.get("source_introspection_id"),
        "result_card_path": latest_card_path(trial.get("result_cards")),
        "proposal_card_path": latest_card_path(trial.get("proposal_cards")),
        "next_existing_action": bounded_text(next_action, limit=260),
        "reason": bounded_text(reason, limit=220),
        "authority_boundary": CLOSURE_LOOP_BOUNDARY,
    }


def being_outcome_closure_loop_v1(status: dict[str, Any]) -> dict[str, Any]:
    entries = [being_outcome_closure_entry(trial) for trial in closure_loop_trial_rows(status)]
    active_entries = [entry for entry in entries if entry.get("wait_state") != "closed_or_satisfied"]
    by_wait = Counter(str(entry.get("wait_state") or "unknown") for entry in entries)
    active_by_wait = Counter(str(entry.get("wait_state") or "unknown") for entry in active_entries)
    if active_by_wait.get("proposal_card_needed"):
        status_label = "proposal_cards_needed"
    elif active_by_wait.get("proposal_card_awaiting_operator_decision"):
        status_label = "operator_decision_wait"
    elif active_by_wait.get("result_card_awaiting_being_response"):
        status_label = "being_response_wait"
    elif active_by_wait.get("result_card_needed"):
        status_label = "result_cards_needed"
    elif active_by_wait.get("ready_runner_waiting"):
        status_label = "runner_waiting"
    elif active_by_wait.get("manual_review_waiting"):
        status_label = "manual_review_waiting"
    elif entries:
        status_label = "closed_or_satisfied"
    else:
        status_label = "empty"
    next_actions: list[str] = []
    for entry in active_entries:
        action = str(entry.get("next_existing_action") or "")
        if action and action not in next_actions:
            next_actions.append(action)
        if len(next_actions) >= 6:
            break
    if not next_actions:
        next_actions.append("no sandbox queue action")
    return {
        "schema": "being_outcome_closure_loop_v1",
        "schema_version": 1,
        "status": status_label,
        "summary": {
            "entries": len(entries),
            "active_waits": len(active_entries),
            "by_wait_state": dict(sorted(by_wait.items())),
            "active_by_wait_state": dict(sorted(active_by_wait.items())),
            "result_card_awaiting_being_response_count": active_by_wait.get(
                "result_card_awaiting_being_response", 0
            ),
            "proposal_card_awaiting_operator_decision_count": active_by_wait.get(
                "proposal_card_awaiting_operator_decision", 0
            ),
            "proposal_card_needed_count": active_by_wait.get("proposal_card_needed", 0),
            "result_card_needed_count": active_by_wait.get("result_card_needed", 0),
            "ready_runner_waiting_count": active_by_wait.get("ready_runner_waiting", 0),
            "manual_review_waiting_count": active_by_wait.get("manual_review_waiting", 0),
            "closed_or_satisfied_count": by_wait.get("closed_or_satisfied", 0),
        },
        "next_existing_actions": next_actions,
        "entries": active_entries[:16],
        "authority_boundary": CLOSURE_LOOP_BOUNDARY,
    }


def report_from_status(status: dict[str, Any]) -> dict[str, Any]:
    trials = active_trials(status)
    all_trials = status.get("trials") if isinstance(status.get("trials"), dict) else {}
    by_status = Counter(str(trial.get("status") or "unknown") for trial in all_trials.values())
    by_mode = Counter(str(trial.get("trial_mode") or "unknown") for trial in all_trials.values())
    by_adapter = Counter(str(trial.get("adapter") or "unknown") for trial in all_trials.values())
    by_tier = Counter(str(trial.get("agency_tier") or 0) for trial in all_trials.values())
    result_count = sum(len(trial.get("results") or []) for trial in all_trials.values() if isinstance(trial, dict))
    result_card_count = sum(
        len(trial.get("result_cards") or []) for trial in all_trials.values() if isinstance(trial, dict)
    )
    proposal_card_count = sum(
        len(trial.get("proposal_cards") or []) for trial in all_trials.values() if isinstance(trial, dict)
    )
    approval_required = [
        trial for trial in trials if str(trial.get("trial_mode")) == "approval_required_live_trial"
    ]
    ready_runnable = [trial for trial in trials if trial_is_runner_executable(trial)]
    runnable_live_violations = [
        trial
        for trial in trials
        if str(trial.get("trial_mode")) in NON_RUNNABLE_MODES and bool(trial.get("runnable"))
    ]
    stale_cutoff = now_s() - 72 * 3600
    stale = [
        trial
        for trial in trials
        if float(trial.get("updated_at") or trial.get("created_at") or now_s()) < stale_cutoff
    ]
    corrupt = int(status.get("corrupt_event_lines") or 0)
    if corrupt:
        state = "database_corrupt_lines_ignored"
    elif runnable_live_violations:
        state = "authority_violation"
    elif not all_trials:
        state = "database_missing"
    elif approval_required:
        state = "approval_waiting"
    elif stale:
        state = "trial_backlog"
    elif trials:
        state = "active"
    else:
        state = "quiet"
    ladder = consentful_sandbox_to_live_ladder_v1(status)
    closure_loop = being_outcome_closure_loop_v1(status)
    return {
        "schema": SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "status": state,
        "summary": {
            "total_trials": len(all_trials),
            "active_trials": len(trials),
            "by_status": dict(sorted(by_status.items())),
            "by_mode": dict(sorted(by_mode.items())),
            "by_adapter": dict(sorted(by_adapter.items())),
            "by_tier": dict(sorted(by_tier.items())),
            "stale_trial_count": len(stale),
            "approval_required_live_count": len(approval_required),
            "runnable_live_violation_count": len(runnable_live_violations),
            "corrupt_event_lines": corrupt,
            "ready_runnable_count": len(ready_runnable),
            "result_count": result_count,
            "result_card_count": result_card_count,
            "proposal_card_count": proposal_card_count,
        },
        "next_trials": trials[:8],
        "next_runnable_trials": ready_runnable[:8],
        "approval_required_live_candidates": approval_required[:8],
        "runnable_live_violations": runnable_live_violations[:8],
        "consentful_sandbox_to_live_ladder_v1": ladder,
        "being_outcome_closure_loop_v1": closure_loop,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def build_report(state_dir: Path = DEFAULT_STATE_DIR) -> dict[str, Any]:
    return report_from_status(load_status(state_dir))


def render_queue_markdown(status: dict[str, Any]) -> str:
    report = report_from_status(status)
    summary = report.get("summary") or {}
    ladder = report.get("consentful_sandbox_to_live_ladder_v1") or {}
    ladder_summary = ladder.get("summary") if isinstance(ladder.get("summary"), dict) else {}
    closure_loop = report.get("being_outcome_closure_loop_v1") or {}
    closure_summary = closure_loop.get("summary") if isinstance(closure_loop.get("summary"), dict) else {}
    lines = [
        "# Sandbox Trial Queue V1",
        "",
        f"- status: {report.get('status')}",
        f"- consentful_ladder: {ladder.get('status')} {ladder_summary.get('by_rung', {})}",
        f"- being_outcome_closure: {closure_loop.get('status')} {closure_summary.get('active_by_wait_state', {})}",
        f"- total_trials: {summary.get('total_trials', 0)}",
        f"- active_trials: {summary.get('active_trials', 0)}",
        f"- by_status: {summary.get('by_status', {})}",
        f"- by_mode: {summary.get('by_mode', {})}",
        f"- ready_runnable: {summary.get('ready_runnable_count', 0)}",
        f"- results_recorded: {summary.get('result_count', 0)}",
        f"- result_cards: {summary.get('result_card_count', 0)}",
        f"- approval_required_live_count: {summary.get('approval_required_live_count', 0)}",
        f"- authority_boundary: {AUTHORITY_BOUNDARY}",
        "",
        "## Next Trials",
    ]
    for trial in report.get("next_trials") or []:
        lines.extend(
            [
                f"- `{trial.get('trial_id')}` `{trial.get('adapter')}` `{trial.get('status')}`",
                f"  - mode: `{trial.get('trial_mode')}` runnable={str(bool(trial.get('runnable'))).lower()} tier={trial.get('agency_tier')}",
                f"  - hypothesis: {bounded_text(trial.get('hypothesis') or '', limit=220)}",
            ]
        )
    return "\n".join(lines).rstrip() + "\n"


def render_ladder_markdown(ladder: dict[str, Any]) -> str:
    summary = ladder.get("summary") if isinstance(ladder.get("summary"), dict) else {}
    lines = [
        "# Consentful Sandbox-to-Live Ladder V1",
        "",
        f"- status: {ladder.get('status')}",
        f"- entries: {summary.get('entries', 0)}",
        f"- by_rung: {summary.get('by_rung', {})}",
        f"- sandbox_ready_count: {summary.get('sandbox_ready_count', 0)}",
        f"- manual_review_ready_count: {summary.get('manual_review_ready_count', 0)}",
        f"- proposal_needed_count: {summary.get('proposal_needed_count', 0)}",
        f"- operator_approval_wait_count: {summary.get('operator_approval_wait_count', 0)}",
        f"- approval_packet_complete_count: {summary.get('approval_packet_complete_count', 0)}",
        f"- live_eligible_now_count: {summary.get('live_eligible_now_count', 0)}",
        f"- authority_boundary: {ladder.get('authority_boundary') or LIVE_LADDER_BOUNDARY}",
        "",
        "## Next Steps",
    ]
    lines.extend(f"- {bounded_text(step, limit=260)}" for step in ladder.get("next_steps") or [])
    lines.extend(["", "## Entries"])
    for entry in ladder.get("entries") or []:
        if not isinstance(entry, dict):
            continue
        lines.extend(
            [
                f"- `{entry.get('trial_id')}` `{entry.get('current_rung')}`",
                f"  - mode: `{entry.get('trial_mode')}` runnable={str(bool(entry.get('runnable'))).lower()} approval_packet_complete={str(bool(entry.get('approval_packet_complete'))).lower()} live_eligible_now={str(bool(entry.get('live_eligible_now'))).lower()}",
                f"  - next: {bounded_text(entry.get('next_required_step') or '', limit=260)}",
                f"  - action: {bounded_text(entry.get('existing_action_hint') or '', limit=260)}",
            ]
        )
    return "\n".join(lines).rstrip() + "\n"


def render_closure_markdown(closure_loop: dict[str, Any]) -> str:
    summary = closure_loop.get("summary") if isinstance(closure_loop.get("summary"), dict) else {}
    lines = [
        "# Being Outcome Closure Loop V1",
        "",
        f"- status: {closure_loop.get('status')}",
        f"- entries: {summary.get('entries', 0)}",
        f"- active_waits: {summary.get('active_waits', 0)}",
        f"- active_by_wait_state: {summary.get('active_by_wait_state', {})}",
        f"- result_card_awaiting_being_response_count: {summary.get('result_card_awaiting_being_response_count', 0)}",
        f"- proposal_card_awaiting_operator_decision_count: {summary.get('proposal_card_awaiting_operator_decision_count', 0)}",
        f"- proposal_card_needed_count: {summary.get('proposal_card_needed_count', 0)}",
        f"- result_card_needed_count: {summary.get('result_card_needed_count', 0)}",
        f"- ready_runner_waiting_count: {summary.get('ready_runner_waiting_count', 0)}",
        f"- manual_review_waiting_count: {summary.get('manual_review_waiting_count', 0)}",
        f"- authority_boundary: {closure_loop.get('authority_boundary') or CLOSURE_LOOP_BOUNDARY}",
        "",
        "## Next Existing Actions",
    ]
    lines.extend(f"- {bounded_text(action, limit=260)}" for action in closure_loop.get("next_existing_actions") or [])
    lines.extend(["", "## Active Waits"])
    for entry in closure_loop.get("entries") or []:
        if not isinstance(entry, dict):
            continue
        lines.extend(
            [
                f"- `{entry.get('trial_id')}` `{entry.get('wait_state')}`",
                f"  - mode: `{entry.get('trial_mode')}` adapter=`{entry.get('adapter')}` runnable={str(bool(entry.get('runnable'))).lower()}",
                f"  - next: {bounded_text(entry.get('next_existing_action') or '', limit=260)}",
                f"  - reason: {bounded_text(entry.get('reason') or '', limit=220)}",
            ]
        )
    return "\n".join(lines).rstrip() + "\n"


def term_counts(texts: list[tuple[Path, str]], terms: tuple[str, ...]) -> dict[str, int]:
    counts: Counter[str] = Counter()
    for _path, text in texts:
        lower = text.lower()
        for term in terms:
            counts[term] += len(re.findall(rf"\b{re.escape(term)}\b", lower))
    return {term: count for term, count in sorted(counts.items()) if count}


def term_context_windows(texts: list[tuple[Path, str]]) -> list[dict[str, Any]]:
    windows: list[dict[str, Any]] = []
    for path, text in texts:
        lower = text.lower()
        for term in TEXTURE_TERMS:
            idx = lower.find(term)
            if idx < 0:
                continue
            start = max(0, idx - 160)
            end = min(len(text), idx + 220)
            window = text[start:end]
            context_hits = [
                marker for marker in TEXTURE_CONTEXT_TERMS if marker in window.lower()
            ]
            windows.append(
                {
                    "path": str(path),
                    "term": term,
                    "context_terms": context_hits[:8],
                    "excerpt": bounded_text(window, limit=260),
                }
            )
            break
    return windows[:12]


def fallback_distinguishability_v1() -> dict[str, Any]:
    llm = read_text(ASTRID_LLM_RS)
    dynamic_present = (
        "fallback_dynamic_texture_weight_v1" in llm
        and "dynamic_texture_weight" in llm
        and "texture_trajectory_v1" in llm
    )
    since = now_s() - 48 * 3600
    texts = [(path, read_text(path, limit=18_000)) for path in public_text_paths(since_s=since)]
    counts = term_counts(texts, TEXTURE_TERMS)
    windows = term_context_windows(texts)
    repeated = [term for term, count in counts.items() if count >= 3]
    context_supported = [window for window in windows if window.get("context_terms")]
    fire_drills = recent_paths(FALLBACK_FIRE_DRILLS, ("*.json", "*.md", "*.txt"), since_s=since, limit=12)
    if not counts and not fire_drills:
        classification = "insufficient_output"
    elif dynamic_present and context_supported:
        classification = "supported_dynamic"
    elif repeated and not context_supported:
        classification = "unsupported_static_repeat"
    else:
        classification = "insufficient_output"
    return {
        "schema": "sandbox_trial_result_v1",
        "adapter": "fallback_distinguishability_v1",
        "classification": classification,
        "generated_at": iso(),
        "dynamic_texture_weight_present": dynamic_present,
        "texture_term_counts": dict(sorted(counts.items(), key=lambda kv: (-kv[1], kv[0]))[:12]),
        "repeated_terms": repeated[:12],
        "context_supported_terms": [
            {"term": item.get("term"), "context_terms": item.get("context_terms"), "path": item.get("path")}
            for item in context_supported[:8]
        ],
        "fallback_fire_drill_count": len(fire_drills),
        "evidence_paths": [str(path) for path in fire_drills[:5]]
        + [str(item.get("path")) for item in windows[:5] if item.get("path")],
        "bounded_windows": windows[:6],
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def shadow_loss_lattice_v1() -> dict[str, Any]:
    since = now_s() - 48 * 3600
    texts = [(path, read_text(path, limit=24_000)) for path in public_text_paths(since_s=since)]
    shadow_samples: list[dict[str, Any]] = []
    max_dispersal = 0.0
    norm_deltas: list[float] = []
    lattice_hits = 0
    loss_hits = 0
    for path, text in texts:
        for match in SHADOW_RE.finditer(text):
            line = bounded_text(match.group(0), limit=320)
            lower = line.lower()
            lattice_hits += sum(1 for term in SHADOW_LATTICE_TERMS if term in lower)
            loss_hits += sum(1 for term in SHADOW_LOSS_TERMS if term in lower)
            parsed = TREND_RE.search(line)
            sample: dict[str, Any] = {"path": str(path), "line": line}
            if parsed:
                norm0, norm1, disp0, disp1 = (float(parsed.group(i)) for i in range(1, 5))
                sample.update(
                    {
                        "norm_delta": round(norm1 - norm0, 6),
                        "dispersal_delta": round(disp1 - disp0, 6),
                        "dispersal_current": round(disp1, 6),
                    }
                )
                norm_deltas.append(norm1 - norm0)
                max_dispersal = max(max_dispersal, disp1)
            shadow_samples.append(sample)
            if len(shadow_samples) >= 20:
                break
        if len(shadow_samples) >= 20:
            break
    min_norm_delta = min(norm_deltas) if norm_deltas else None
    if not shadow_samples:
        classification = "ambiguous_needs_more_samples"
    elif loss_hits >= 2 and max_dispersal >= 0.40:
        classification = "fragmentation_risk"
    elif loss_hits > lattice_hits and (min_norm_delta is not None and min_norm_delta <= -0.25):
        classification = "loss_like"
    elif lattice_hits >= loss_hits and lattice_hits > 0:
        classification = "lattice_transition_like"
    else:
        classification = "ambiguous_needs_more_samples"
    return {
        "schema": "sandbox_trial_result_v1",
        "adapter": "shadow_loss_lattice_v1",
        "classification": classification,
        "generated_at": iso(),
        "sample_count": len(shadow_samples),
        "lattice_language_hits": lattice_hits,
        "loss_language_hits": loss_hits,
        "min_norm_delta": round(min_norm_delta, 6) if min_norm_delta is not None else None,
        "max_dispersal": round(max_dispersal, 6),
        "samples": shadow_samples[:8],
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def requested_shadow_multiplier(text: str) -> float:
    lower = text.lower()
    values = [float(match.group(0)) for match in SHADOW_VALUE_RE.finditer(lower)]
    if len(values) >= 2 and values[0] > 0:
        return max(0.25, min(4.0, values[1] / values[0]))
    if "doubl" in lower or "2x" in lower:
        return 2.0
    if "halve" in lower or "half" in lower or "0.5x" in lower:
        return 0.5
    if "increase" in lower or "stronger" in lower or "more influence" in lower:
        return 1.5
    return 1.0


def shadow_influence_replay_v1(trial: dict[str, Any]) -> dict[str, Any]:
    base = shadow_loss_lattice_v1()
    samples = base.get("samples") if isinstance(base.get("samples"), list) else []
    requested = requested_shadow_multiplier(
        " ".join(str(trial.get(key) or "") for key in ("hypothesis", "felt_report_anchor", "proposed_intervention"))
    )
    norm_deltas = [
        float(sample.get("norm_delta"))
        for sample in samples
        if isinstance(sample, dict) and isinstance(sample.get("norm_delta"), (int, float))
    ]
    dispersal_deltas = [
        float(sample.get("dispersal_delta"))
        for sample in samples
        if isinstance(sample, dict) and isinstance(sample.get("dispersal_delta"), (int, float))
    ]
    dispersal_currents = [
        float(sample.get("dispersal_current"))
        for sample in samples
        if isinstance(sample, dict) and isinstance(sample.get("dispersal_current"), (int, float))
    ]
    avg_norm_delta = sum(norm_deltas) / len(norm_deltas) if norm_deltas else 0.0
    avg_dispersal_delta = sum(dispersal_deltas) / len(dispersal_deltas) if dispersal_deltas else 0.0
    current_dispersal = max(dispersal_currents) if dispersal_currents else float(base.get("max_dispersal") or 0.0)
    multipliers = sorted({0.5, 1.0, 1.5, round(requested, 3), 2.0})
    replay_rows: list[dict[str, Any]] = []
    for multiplier in multipliers:
        projected_norm_delta = avg_norm_delta * multiplier
        projected_dispersal = max(0.0, current_dispersal + avg_dispersal_delta * max(0.0, multiplier - 1.0))
        review_flag = projected_dispersal >= 0.40 or projected_norm_delta <= -0.25
        replay_rows.append(
            {
                "multiplier": round(multiplier, 3),
                "projected_norm_delta": round(projected_norm_delta, 6),
                "projected_dispersal": round(projected_dispersal, 6),
                "fragmentation_review_flag": review_flag,
            }
        )
    requested_row = min(replay_rows, key=lambda row: abs(float(row["multiplier"]) - requested)) if replay_rows else {}
    if not samples:
        classification = "ambiguous_needs_more_samples"
    elif base.get("classification") in {"fragmentation_risk", "loss_like"} or requested_row.get("fragmentation_review_flag"):
        classification = "replay_warns_fragmentation_risk"
    elif base.get("classification") == "lattice_transition_like":
        classification = "replay_supports_bounded_shadow_gain"
    else:
        classification = "ambiguous_needs_more_samples"
    return {
        "schema": "sandbox_trial_result_v1",
        "adapter": "shadow_influence_replay_v1",
        "classification": classification,
        "generated_at": iso(),
        "requested_multiplier": round(requested, 3),
        "base_shadow_classification": base.get("classification"),
        "base_sample_count": base.get("sample_count"),
        "base_lattice_language_hits": base.get("lattice_language_hits"),
        "base_loss_language_hits": base.get("loss_language_hits"),
        "avg_norm_delta": round(avg_norm_delta, 6),
        "avg_dispersal_delta": round(avg_dispersal_delta, 6),
        "current_max_dispersal": round(current_dispersal, 6),
        "replay_rows": replay_rows,
        "evidence_samples": samples[:5],
        "authority_boundary": AUTHORITY_BOUNDARY,
        "live_boundary": LIVE_APPROVAL_BOUNDARY,
    }


def result_markdown(trial: dict[str, Any], result: dict[str, Any]) -> str:
    lines = [
        "# Sandbox Trial Result V1",
        "",
        f"- trial_id: {trial.get('trial_id')}",
        f"- adapter: {result.get('adapter')}",
        f"- classification: {result.get('classification')}",
        f"- generated_at: {result.get('generated_at')}",
        f"- authority_boundary: {result.get('authority_boundary')}",
        "",
        "## Hypothesis",
        bounded_text(trial.get("hypothesis") or "", limit=700),
        "",
        "## Evidence Summary",
    ]
    for key in (
        "dynamic_texture_weight_present",
        "texture_term_counts",
        "repeated_terms",
        "fallback_fire_drill_count",
        "sample_count",
        "lattice_language_hits",
        "loss_language_hits",
        "min_norm_delta",
        "max_dispersal",
        "requested_multiplier",
        "base_shadow_classification",
        "base_sample_count",
        "avg_norm_delta",
        "avg_dispersal_delta",
        "current_max_dispersal",
        "replay_rows",
    ):
        if key in result:
            lines.append(f"- {key}: {bounded_text(json.dumps(result[key], ensure_ascii=False), limit=500)}")
    return "\n".join(lines).rstrip() + "\n"


def trial_result_card_text(trial: dict[str, Any], result: dict[str, Any]) -> str:
    replay_result = replay_result_v2_for_trial(trial, result)
    replay_receipt = {
        "receipt_id": stable_uuid(
            "sandbox_trial_lifecycle_receipt_v2",
            trial.get("trial_id"),
            "replay_result",
        ),
        "boundary_id": authority_boundary_packet_v2_id(trial),
        "kind": "replay_result",
        "issued_by": "sandbox_trial_queue_v2",
        "issued_at": replay_result.get("occurred_at"),
        "bounded_summary": replay_result["bounded_summary"],
        "evidence_refs": replay_result.get("evidence_refs", []),
        "replay_result": replay_result,
        "right_to_ignore": True,
    }
    lines = [
        "# Sandbox Trial Response Card V2",
        "",
        f"- trial_id: {trial.get('trial_id')}",
        f"- source_work_item_id: {trial.get('source_work_item_id')}",
        f"- source_introspection_id: {trial.get('source_introspection_id')}",
        f"- claim_id: {trial.get('claim_id')}",
        f"- being: {trial.get('being')}",
        f"- adapter: {result.get('adapter')}",
        f"- classification: {result.get('classification')}",
        "- right_to_ignore: true",
        "",
        "## Heard",
        bounded_text(trial.get("felt_report_anchor") or trial.get("hypothesis") or "", limit=700),
        "",
        "## What Ran",
        bounded_text(trial.get("proposed_intervention") or "", limit=700),
        "",
        "## Result",
        bounded_text(json.dumps({k: v for k, v in result.items() if k not in {"bounded_windows", "samples", "evidence_samples"}}, ensure_ascii=False), limit=1100),
        "",
        "## Replay Result V2 Receipt",
        "```json",
        json.dumps(replay_receipt, indent=2, sort_keys=True, ensure_ascii=False),
        "```",
        *agency_corridor_card_section(
            [
                str(trial.get("trial_id") or ""),
                str(trial.get("source_work_item_id") or ""),
            ]
        ),
        "",
        "## What Did Not Change",
        AUTHORITY_BOUNDARY,
        "",
        "## Response Welcome",
        (
            "If this result sharpens, worsens, muffles, stabilizes, or misses the felt report, "
            "that response can become the next evidence anchor. There is no obligation to answer."
        ),
    ]
    return "\n".join(lines).rstrip() + "\n"


def trial_proposal_card_text(trial: dict[str, Any]) -> str:
    lines = [
        "# Sandbox Trial Proposal Card V2",
        "",
        f"- trial_id: {trial.get('trial_id')}",
        f"- adapter: {trial.get('adapter')}",
        f"- mode: {trial.get('trial_mode')}",
        f"- status: {trial.get('status')}",
        f"- runnable: {str(bool(trial.get('runnable'))).lower()}",
        f"- being: {trial.get('being')}",
        f"- source_work_item_id: {trial.get('source_work_item_id')}",
        f"- source_introspection_id: {trial.get('source_introspection_id')}",
        f"- claim_id: {trial.get('claim_id')}",
        "- right_to_ignore: true",
        "",
        "## Felt Anchor",
        bounded_text(trial.get("felt_report_anchor") or "", limit=700),
        "",
        "## Hypothesis",
        bounded_text(trial.get("hypothesis") or "", limit=700),
        "",
        "## Proposed Trial",
        bounded_text(trial.get("proposed_intervention") or "", limit=700),
        "",
        "## Success Metrics",
    ]
    lines.extend(f"- {bounded_text(metric, limit=240)}" for metric in trial.get("success_metrics") or [])
    lines.append("")
    lines.append("## Abort Criteria")
    lines.extend(f"- {bounded_text(item, limit=240)}" for item in trial.get("abort_criteria") or [])
    lines.extend(["", "## Boundary", bounded_text(trial.get("approval_boundary") or AUTHORITY_BOUNDARY, limit=800)])
    packet = authority_boundary_packet_for_trial(trial)
    packet_v2 = authority_boundary_packet_v2_for_trial(trial)
    lines.extend(
        [
            "",
            "## Authority Boundary Packet V1",
            "```json",
            json.dumps(packet, indent=2, sort_keys=True, ensure_ascii=False),
            "```",
            "",
            "## Authority Boundary Packet V2",
            "```json",
            json.dumps(packet_v2, indent=2, sort_keys=True, ensure_ascii=False),
            "```",
        ]
    )
    lines.extend(
        agency_corridor_card_section(
            [
                str(trial.get("trial_id") or ""),
                str(trial.get("source_work_item_id") or ""),
            ]
        )
    )
    return "\n".join(lines).rstrip() + "\n"


def emit_result_card(
    state_dir: Path,
    trial: dict[str, Any],
    result: dict[str, Any],
    *,
    write: bool,
) -> dict[str, Any]:
    ts = int(now_s())
    trial_id = str(trial.get("trial_id") or "")
    text = trial_result_card_text(trial, result)
    path = state_dir / "result_cards" / f"{ts}_{trial_id}.md"
    card = {
        "schema": "sandbox_trial_response_card_v2",
        "trial_id": trial_id,
        "source_work_item_id": trial.get("source_work_item_id"),
        "source_introspection_id": trial.get("source_introspection_id"),
        "claim_id": trial.get("claim_id"),
        "being": trial.get("being"),
        "path": str(path),
        "right_to_ignore": True,
        "classification": result.get("classification"),
        "replay_result_v2": replay_result_v2_for_trial(trial, result),
        "text_sha256": sha_text(text),
        "text_excerpt": bounded_text(text, limit=900),
    }
    if write:
        atomic_write_text(path, text)
    return card


def emit_proposal_card(state_dir: Path, trial: dict[str, Any], *, write: bool) -> dict[str, Any]:
    ts = int(now_s())
    trial_id = str(trial.get("trial_id") or "")
    text = trial_proposal_card_text(trial)
    path = state_dir / "proposal_cards" / f"{ts}_{trial_id}.md"
    card = {
        "schema": "sandbox_trial_proposal_card_v2",
        "trial_id": trial_id,
        "source_work_item_id": trial.get("source_work_item_id"),
        "source_introspection_id": trial.get("source_introspection_id"),
        "claim_id": trial.get("claim_id"),
        "being": trial.get("being"),
        "path": str(path),
        "right_to_ignore": True,
        "trial_mode": trial.get("trial_mode"),
        "runnable": bool(trial.get("runnable")),
        "boundary_id": authority_boundary_packet_for_trial(trial).get("boundary_id"),
        "boundary_id_v2": authority_boundary_packet_v2_for_trial(trial).get("boundary_id"),
        "authority_boundary_packet": authority_boundary_packet_for_trial(trial),
        "authority_boundary_packet_v2": authority_boundary_packet_v2_for_trial(trial),
        "text_sha256": sha_text(text),
        "text_excerpt": bounded_text(text, limit=900),
    }
    if write:
        atomic_write_text(path, text)
    return card


def addressing_events_for_result(
    trial: dict[str, Any],
    result: dict[str, Any],
    *,
    json_path: str,
    markdown_path: str,
    card_path: str | None,
) -> list[dict[str, Any]]:
    try:
        import introspection_addressing_audit as addressing
    except Exception:
        return []
    events: list[dict[str, Any]] = []
    work_item_id = str(trial.get("source_work_item_id") or "")
    source_id = str(trial.get("source_introspection_id") or "")
    claim_id = str(trial.get("claim_id") or "")
    note = (
        f"sandbox trial {trial.get('trial_id')} via {result.get('adapter')} "
        f"classified {result.get('classification')}; no live runtime mutation"
    )
    if work_item_id:
        events.append(addressing.work_evidence_event(work_item_id, "diagnostic", json_path, note))
        events.append(addressing.work_evidence_event(work_item_id, "test", markdown_path, note))
        if card_path:
            events.append(
                addressing.work_evidence_event(
                    work_item_id,
                    "closure_card",
                    card_path,
                    "right-to-ignore sandbox trial response card emitted",
                )
            )
            events.append(
                addressing.post_change_response_event(
                    work_item_id,
                    "awaiting",
                    card_path,
                    "sandbox result card emitted; response is welcome but not forced",
                )
            )
    if source_id and claim_id:
        events.append(addressing.evidence_event(source_id, claim_id, "test", json_path, note))
    return events


def write_addressing_events(events: list[dict[str, Any]]) -> int:
    if not events:
        return 0
    try:
        import introspection_addressing_audit as addressing
    except Exception:
        return 0
    addressing.append_events(INTROSPECTION_ADDRESSING_STATE_DIR, events)
    status = addressing.replay_events(INTROSPECTION_ADDRESSING_STATE_DIR)
    addressing.write_materialized_status(INTROSPECTION_ADDRESSING_STATE_DIR, status)
    return len(events)


def run_adapter_for_trial(
    state_dir: Path,
    trial_id: str,
    *,
    write: bool,
    link_evidence: bool = True,
    emit_card: bool = True,
) -> dict[str, Any]:
    status = replay_status(state_dir)
    trial = (status.get("trials") or {}).get(trial_id)
    if not isinstance(trial, dict):
        raise SystemExit(f"unknown trial_id {trial_id}")
    if str(trial.get("trial_mode")) in NON_RUNNABLE_MODES:
        result = {
            "schema": "sandbox_trial_result_v1",
            "adapter": trial.get("adapter"),
            "classification": "approval_required_provider_change",
            "generated_at": iso(),
            "reason": LIVE_APPROVAL_BOUNDARY,
            "authority_boundary": LIVE_APPROVAL_BOUNDARY,
        }
    elif trial.get("adapter") == "fallback_distinguishability_v1":
        result = fallback_distinguishability_v1()
    elif trial.get("adapter") == "shadow_loss_lattice_v1":
        result = shadow_loss_lattice_v1()
    elif trial.get("adapter") == "shadow_influence_replay_v1":
        result = shadow_influence_replay_v1(trial)
    else:
        result = {
            "schema": "sandbox_trial_result_v1",
            "adapter": trial.get("adapter"),
            "classification": "insufficient_output",
            "generated_at": iso(),
            "reason": "manual review adapter has no runnable analysis in V1",
            "authority_boundary": AUTHORITY_BOUNDARY,
        }
    result = dict(result)
    result["trial_id"] = trial_id
    result["result_sha256"] = sha_text(json.dumps(result, sort_keys=True, ensure_ascii=True))
    if write:
        ts = int(now_s())
        result_dir = state_dir / "results"
        json_path = result_dir / f"{ts}_{trial_id}_{result.get('adapter')}.json"
        md_path = result_dir / f"{ts}_{trial_id}_{result.get('adapter')}.md"
        atomic_write_text(json_path, json.dumps(result, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
        atomic_write_text(md_path, result_markdown(trial, result))
        card = emit_result_card(state_dir, trial, result, write=True) if emit_card else None
        event = {
            "event_type": "trial_result_recorded",
            "schema": SCHEMA,
            "ts": now_s(),
            "trial_id": trial_id,
            "result": {
                "adapter": result.get("adapter"),
                "classification": result.get("classification"),
                "json_path": str(json_path),
                "markdown_path": str(md_path),
                "result_sha256": result.get("result_sha256"),
                "ts": now_s(),
            },
        }
        events = [event]
        if card is not None:
            events.append(
                {
                    "event_type": "trial_result_card_emitted",
                    "schema": SCHEMA,
                    "ts": now_s(),
                    "trial_id": trial_id,
                    "card": card,
                }
            )
        append_events(state_dir, events)
        addressing_events = (
            addressing_events_for_result(
                trial,
                result,
                json_path=str(json_path),
                markdown_path=str(md_path),
                card_path=str(card.get("path")) if card else None,
            )
            if link_evidence
            else []
        )
        result["addressing_events_appended"] = write_addressing_events(addressing_events)
        materialize(state_dir, replay_status(state_dir))
        result["json_path"] = str(json_path)
        result["markdown_path"] = str(md_path)
        if card is not None:
            result["result_card_path"] = card.get("path")
    return result


def next_runnable_trials(
    state_dir: Path,
    *,
    limit: int,
    adapter: str | None = None,
) -> list[dict[str, Any]]:
    status = replay_status(state_dir)
    rows = [trial for trial in active_trials(status) if trial_is_runner_executable(trial)]
    if adapter:
        rows = [trial for trial in rows if str(trial.get("adapter") or "") == adapter]
    return rows[: max(0, limit)]


def run_ready_trials(
    state_dir: Path,
    *,
    limit: int,
    adapter: str | None = None,
    write: bool,
    link_evidence: bool = True,
    emit_card: bool = True,
) -> dict[str, Any]:
    selected = next_runnable_trials(state_dir, limit=limit, adapter=adapter)
    results: list[dict[str, Any]] = []
    for trial in selected:
        results.append(
            run_adapter_for_trial(
                state_dir,
                str(trial.get("trial_id")),
                write=write,
                link_evidence=link_evidence,
                emit_card=emit_card,
            )
        )
    return {
        "schema": "sandbox_trial_runner_v2",
        "write": write,
        "selected_count": len(selected),
        "result_count": len(results),
        "selected_trials": [
            {
                "trial_id": trial.get("trial_id"),
                "adapter": trial.get("adapter"),
                "trial_mode": trial.get("trial_mode"),
                "status": trial.get("status"),
            }
            for trial in selected
        ],
        "results": results,
        "report": build_report(state_dir),
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def emit_proposal_cards_for_trials(
    state_dir: Path,
    *,
    trial_ids: list[str],
    write: bool,
) -> dict[str, Any]:
    status = replay_status(state_dir)
    trials = status.get("trials") if isinstance(status.get("trials"), dict) else {}
    cards: list[dict[str, Any]] = []
    events: list[dict[str, Any]] = []
    for trial_id in trial_ids:
        trial = trials.get(str(trial_id))
        if not isinstance(trial, dict):
            continue
        card = emit_proposal_card(state_dir, trial, write=write)
        cards.append(card)
        events.append(
            {
                "event_type": "trial_proposal_card_emitted",
                "schema": SCHEMA,
                "ts": now_s(),
                "trial_id": str(trial.get("trial_id") or ""),
                "card": card,
            }
        )
    if write:
        append_events(state_dir, events)
        materialize(state_dir, replay_status(state_dir))
    return {
        "schema": "sandbox_trial_proposal_cards_v2",
        "write": write,
        "card_count": len(cards),
        "cards": cards,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def print_payload(payload: dict[str, Any], *, as_json: bool) -> None:
    if as_json:
        print(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False))
    else:
        print(render_queue_markdown(load_status(DEFAULT_STATE_DIR)), end="")


class SandboxTrialQueueTests(unittest.TestCase):
    def test_replay_ignores_corrupt_trailing_lines_and_preserves_trials(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            state_dir = Path(tmp)
            trial = {
                "trial_id": "trial_test",
                "adapter": "manual_sandbox_review_v1",
                "trial_mode": "read_only_review",
                "status": "ready_for_sandbox",
                "runnable": True,
                "hypothesis": "x",
                "created_at": now_s(),
                "updated_at": now_s(),
            }
            append_events(state_dir, [trial_created_event(trial)])
            with events_path(state_dir).open("a", encoding="utf-8") as fh:
                fh.write("{not json}\n")
            status = replay_status(state_dir)
            self.assertEqual(status["corrupt_event_lines"], 1)
            self.assertIn("trial_test", status["trials"])
            self.assertFalse(status["trials"]["trial_test"]["runnable"])

    def test_full_prose_is_bounded_in_trial_packet(self) -> None:
        item = {
            "work_item_id": "wi_long",
            "source_introspection_id": "intro",
            "claim_id": "c001",
            "being": "astrid",
            "agency_tier": 3,
            "status": "needs_sandbox",
            "title": "fallback texture trial",
            "claim_summary": "A" * 2000,
        }
        trial = trial_from_work_item(item)
        assert trial is not None
        self.assertLessEqual(len(str(trial["felt_report_anchor"])), 423)
        packet = trial_packet_text(trial)
        self.assertNotIn("A" * 1000, packet)

    def test_tier_five_candidates_are_not_runnable(self) -> None:
        item = {
            "work_item_id": "wi_live",
            "source_introspection_id": "intro",
            "claim_id": "c002",
            "being": "minime",
            "agency_tier": 5,
            "status": "needs_operator_approval",
            "title": "change pressure controller behavior",
            "claim_summary": "live pressure change",
        }
        trial = trial_from_work_item(item)
        assert trial is not None
        self.assertEqual(trial["trial_mode"], "approval_required_live_trial")
        self.assertFalse(trial["runnable"])
        packet = trial["authority_boundary_packet"]
        self.assertEqual(packet["authority_class"], "mike_operator_live_substrate")
        self.assertEqual(packet["gate_state"], "proposal_needed")
        self.assertFalse(packet["live_eligible_now"])
        self.assertFalse(packet["auto_approved"])
        packet_v2 = trial["authority_boundary_packet_v2"]
        self.assertEqual(packet_v2["schema_version"], 2)
        self.assertEqual(packet_v2["authority_class"], "mike_operator_live_substrate")
        self.assertEqual(packet_v2["lifecycle_state"], "proposal_needed")
        self.assertTrue(packet_v2["delta_refs"])
        self.assertEqual(packet_v2["rollout_abort_contract"]["post_change_response_required"], True)
        self.assertFalse(packet_v2["live_eligible_now"])
        self.assertFalse(packet_v2["auto_approved"])
        status = empty_status()
        apply_event(status, trial_created_event(trial))
        report = report_from_status(status)
        self.assertEqual(report["summary"]["runnable_live_violation_count"], 0)

    def test_proposal_card_includes_authority_boundary_packet(self) -> None:
        trial = {
            "trial_id": "trial_live_packet",
            "adapter": "manual_sandbox_review_v1",
            "trial_mode": "approval_required_live_trial",
            "status": "approval_required_live_trial",
            "runnable": False,
            "agency_tier": 5,
            "being": "astrid",
            "hypothesis": "live porosity change might reduce clipping",
            "felt_report_anchor": "porosity change needs explicit approval",
            "proposed_intervention": "prepare explicit approval packet only; do not run or apply",
            "success_metrics": ["operator can review bounded packet"],
            "abort_criteria": ["no explicit approval"],
        }
        text = trial_proposal_card_text(trial)
        self.assertIn("## Authority Boundary Packet V1", text)
        self.assertIn("## Authority Boundary Packet V2", text)
        self.assertIn('"delta_refs"', text)
        self.assertIn('"rollout_abort_contract"', text)
        self.assertIn('"redaction_profile"', text)
        self.assertIn('"live_eligible_now": false', text)
        self.assertIn('"auto_approved": false', text)
        self.assertIn("## Agency Corridor V1", text)
        self.assertIn("grants no approval", text)

        card = emit_proposal_card(Path("/tmp/sandbox_trial_queue_test"), trial, write=False)
        packet = card["authority_boundary_packet"]
        packet_v2 = card["authority_boundary_packet_v2"]
        self.assertEqual(card["boundary_id"], packet["boundary_id"])
        self.assertEqual(card["boundary_id_v2"], packet_v2["boundary_id"])
        self.assertFalse(packet["live_eligible_now"])
        self.assertFalse(packet["auto_approved"])
        self.assertFalse(packet_v2["live_eligible_now"])
        self.assertFalse(packet_v2["auto_approved"])
        self.assertEqual(packet_v2["lifecycle_state"], "proposal_needed")

    def test_terminal_addressing_work_items_do_not_create_trials(self) -> None:
        for terminal_status in (
            "closed",
            "closed_felt_confirmed",
            "closed_no_action",
            "superseded",
            "verified_existing",
        ):
            item = {
                "work_item_id": f"wi_{terminal_status}",
                "source_introspection_id": "intro",
                "claim_id": "c003",
                "being": "minime",
                "agency_tier": 5,
                "status": terminal_status,
                "title": "change live regulator target bias",
                "claim_summary": "closed work must not become a live approval candidate",
            }

            self.assertIsNone(trial_from_work_item(item), terminal_status)

    def test_ladder_blocks_live_candidate_until_proposal_and_operator_approval(self) -> None:
        live = {
            "trial_id": "trial_live",
            "adapter": "manual_sandbox_review_v1",
            "trial_mode": "approval_required_live_trial",
            "status": "approval_required_live_trial",
            "runnable": False,
            "agency_tier": 5,
            "being": "minime",
            "hypothesis": "live pressure relief would feel less pinned",
            "felt_report_anchor": "pressure relief needs consent before live runtime change",
            "success_metrics": ["being reports less pinned pressure after approved manual trial"],
            "abort_criteria": ["abort on fill instability or distress report"],
            "post_response_request": "ask Minime for a right-to-ignore outcome report",
            "created_at": now_s(),
            "updated_at": now_s(),
        }
        status = empty_status()
        apply_event(status, trial_created_event(live))
        ladder = consentful_sandbox_to_live_ladder_v1(status)
        entry = ladder["entries"][0]
        gates = {gate_row["gate"]: gate_row["status"] for gate_row in entry["gates"]}
        self.assertEqual(ladder["status"], "proposal_needed")
        self.assertEqual(entry["current_rung"], "proposal_card_needed")
        self.assertEqual(gates["proposal_card_or_approval_packet"], "missing")
        self.assertEqual(gates["explicit_mike_operator_approval"], "missing")
        self.assertFalse(entry["approval_packet_complete"])
        self.assertFalse(entry["live_eligible_now"])
        self.assertFalse(entry["live_execution_automatic"])
        self.assertEqual(
            entry["authority_boundary_packet"]["gate_state"],
            "proposal_needed",
        )
        self.assertFalse(entry["authority_boundary_packet"]["live_eligible_now"])
        self.assertEqual(
            entry["authority_boundary_packet_v2"]["lifecycle_state"],
            "proposal_needed",
        )
        self.assertFalse(entry["authority_boundary_packet_v2"]["live_eligible_now"])

        apply_event(
            status,
            {
                "event_type": "trial_proposal_card_emitted",
                "trial_id": "trial_live",
                "card": {"path": "proposal_cards/trial_live.md"},
                "ts": now_s(),
            },
        )
        ladder = consentful_sandbox_to_live_ladder_v1(status)
        entry = ladder["entries"][0]
        self.assertEqual(ladder["status"], "operator_approval_wait")
        self.assertEqual(entry["current_rung"], "operator_approval_wait")
        self.assertFalse(entry["approval_packet_complete"])
        self.assertFalse(entry["live_eligible_now"])

        approved_status = empty_status()
        approved_live = {
            **live,
            "proposal_cards": [{"path": "proposal_cards/trial_live.md"}],
            "operator_approval": {"status": "approved", "approved_by": "Mike"},
        }
        apply_event(approved_status, trial_created_event(approved_live))
        ladder = consentful_sandbox_to_live_ladder_v1(approved_status)
        entry = ladder["entries"][0]
        self.assertEqual(entry["current_rung"], "approved_live_trial_still_manual")
        self.assertTrue(entry["approval_packet_complete"])
        self.assertFalse(entry["live_eligible_now"])
        self.assertFalse(entry["live_execution_automatic"])
        self.assertEqual(ladder["summary"]["approval_packet_complete_count"], 1)
        self.assertEqual(ladder["summary"]["live_eligible_now_count"], 0)
        self.assertFalse(entry["authority_boundary_packet_v2"]["live_eligible_now"])
        self.assertEqual(
            entry["authority_boundary_packet_v2"]["rollout_abort_contract"]["post_change_response_required"],
            True,
        )

    def test_result_card_records_replay_result_v2_without_closing_response_debt(self) -> None:
        trial = {
            "trial_id": "trial_replay_v2",
            "adapter": "shadow_loss_lattice_v1",
            "trial_mode": "sandbox_replay",
            "status": "result_recorded",
            "runnable": True,
            "agency_tier": 3,
            "hypothesis": "offline lattice check",
            "felt_report_anchor": "shadow lattice felt interwoven",
            "success_metrics": ["classification names the lattice state"],
            "abort_criteria": ["missing bounded trace evidence"],
            "results": [{"adapter": "shadow_loss_lattice_v1", "classification": "lattice_transition_like"}],
            "created_at": now_s(),
            "updated_at": now_s(),
        }
        result = trial["results"][0]
        text = trial_result_card_text(trial, result)
        self.assertIn("## Replay Result V2 Receipt", text)
        self.assertIn('"kind": "replay_result"', text)
        self.assertIn("## Agency Corridor V1", text)
        card = emit_result_card(Path("/tmp/sandbox_trial_queue_test"), trial, result, write=False)
        self.assertEqual(card["replay_result_v2"]["classification"], "passed")

        status = empty_status()
        apply_event(status, trial_created_event(trial))
        apply_event(
            status,
            {
                "event_type": "trial_result_card_emitted",
                "trial_id": "trial_replay_v2",
                "card": card,
                "ts": now_s(),
            },
        )
        closure_loop = being_outcome_closure_loop_v1(status)
        self.assertEqual(
            closure_loop["entries"][0]["wait_state"],
            "result_card_awaiting_being_response",
        )
        packet_v2 = status["trials"]["trial_replay_v2"]["authority_boundary_packet_v2"]
        self.assertEqual(packet_v2["replay_results"][0]["classification"], "passed")

    def test_ladder_keeps_runnable_sandbox_before_live_review(self) -> None:
        trial = {
            "trial_id": "trial_sandbox",
            "adapter": "shadow_influence_replay_v1",
            "trial_mode": "sandbox_replay",
            "status": "ready_for_sandbox",
            "runnable": True,
            "agency_tier": 3,
            "hypothesis": "shadow replay can test bounded gain without live mutation",
            "felt_report_anchor": "shadow influence felt thin in offline trace",
            "success_metrics": ["classification names support or fragmentation risk"],
            "abort_criteria": ["abort if adapter cannot find bounded trace evidence"],
            "created_at": now_s(),
            "updated_at": now_s(),
        }
        status = empty_status()
        apply_event(status, trial_created_event(trial))
        ladder = consentful_sandbox_to_live_ladder_v1(status)
        entry = ladder["entries"][0]
        self.assertEqual(ladder["status"], "sandbox_ready")
        self.assertEqual(entry["current_rung"], "sandbox_ready_to_run")
        self.assertIn("run-next", entry["existing_action_hint"])
        self.assertFalse(entry["approval_packet_complete"])
        self.assertFalse(entry["live_eligible_now"])

    def test_manual_review_packet_does_not_advertise_run_next(self) -> None:
        trial = {
            "trial_id": "trial_manual",
            "adapter": "manual_sandbox_review_v1",
            "trial_mode": "read_only_review",
            "status": "ready_for_sandbox",
            "runnable": True,
            "agency_tier": 3,
            "hypothesis": "manual review only",
            "created_at": now_s(),
            "updated_at": now_s(),
        }
        status = empty_status()
        apply_event(status, trial_created_event(trial))
        ladder = consentful_sandbox_to_live_ladder_v1(status)
        entry = ladder["entries"][0]
        report = report_from_status(status)
        self.assertEqual(ladder["status"], "manual_review_waiting")
        self.assertEqual(entry["current_rung"], "manual_review_ready")
        self.assertNotIn("run-next", entry["existing_action_hint"])
        self.assertEqual(report["summary"]["ready_runnable_count"], 0)
        closure_loop = being_outcome_closure_loop_v1(status)
        self.assertEqual(closure_loop["summary"]["manual_review_waiting_count"], 1)

    def test_ladder_flags_live_runnable_violation(self) -> None:
        trial = {
            "trial_id": "trial_bad_live",
            "adapter": "manual_sandbox_review_v1",
            "trial_mode": "approval_required_live_trial",
            "status": "approval_required_live_trial",
            "runnable": True,
            "agency_tier": 5,
            "hypothesis": "bad live runnable marker",
            "felt_report_anchor": "this should never be runnable from the queue",
            "created_at": now_s(),
            "updated_at": now_s(),
        }
        status = empty_status()
        apply_event(status, trial_created_event(trial))
        ladder = consentful_sandbox_to_live_ladder_v1(status)
        entry = ladder["entries"][0]
        gates = {gate_row["gate"]: gate_row["status"] for gate_row in entry["gates"]}
        self.assertEqual(ladder["status"], "authority_violation")
        self.assertEqual(entry["current_rung"], "authority_violation_live_candidate_marked_runnable")
        self.assertEqual(gates["live_runnable_flag"], "violation")
        self.assertFalse(entry["live_execution_automatic"])

    def test_materialize_writes_ladder_artifacts(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            state_dir = Path(tmp)
            trial = {
                "trial_id": "trial_sandbox",
                "adapter": "shadow_loss_lattice_v1",
                "trial_mode": "sandbox_replay",
                "status": "ready_for_sandbox",
                "runnable": True,
                "agency_tier": 3,
                "hypothesis": "offline lattice check",
                "created_at": now_s(),
                "updated_at": now_s(),
            }
            status = empty_status()
            apply_event(status, trial_created_event(trial))
            materialize(state_dir, status)
            ladder_json = state_dir / LADDER_FILE
            ladder_md = state_dir / LADDER_MARKDOWN_FILE
            closure_json = state_dir / CLOSURE_FILE
            closure_md = state_dir / CLOSURE_MARKDOWN_FILE
            self.assertTrue(ladder_json.exists())
            self.assertTrue(ladder_md.exists())
            self.assertTrue(closure_json.exists())
            self.assertTrue(closure_md.exists())
            loaded = json.loads(ladder_json.read_text(encoding="utf-8"))
            self.assertEqual(loaded["schema"], "consentful_sandbox_to_live_ladder_v1")
            closure_loaded = json.loads(closure_json.read_text(encoding="utf-8"))
            self.assertEqual(closure_loaded["schema"], "being_outcome_closure_loop_v1")
            self.assertIn("Consentful Sandbox-to-Live Ladder", ladder_md.read_text(encoding="utf-8"))
            self.assertIn("Being Outcome Closure Loop", closure_md.read_text(encoding="utf-8"))

    def test_outcome_loop_groups_result_and_proposal_waits(self) -> None:
        status = empty_status()
        result_wait = {
            "trial_id": "trial_result_wait",
            "adapter": "shadow_loss_lattice_v1",
            "trial_mode": "sandbox_replay",
            "status": "result_recorded",
            "runnable": True,
            "results": [{"classification": "lattice_transition_like"}],
            "result_cards": [{"path": "result_cards/trial_result_wait.md"}],
            "post_response_status": "awaiting",
            "created_at": now_s(),
            "updated_at": now_s(),
        }
        proposal_wait = {
            "trial_id": "trial_proposal_wait",
            "adapter": "manual_sandbox_review_v1",
            "trial_mode": "approval_required_live_trial",
            "status": "approval_required_live_trial",
            "runnable": False,
            "proposal_cards": [{"path": "proposal_cards/trial_proposal_wait.md"}],
            "created_at": now_s(),
            "updated_at": now_s(),
        }
        apply_event(status, trial_created_event(result_wait))
        apply_event(status, trial_created_event(proposal_wait))
        closure_loop = being_outcome_closure_loop_v1(status)
        waits = {entry["trial_id"]: entry["wait_state"] for entry in closure_loop["entries"]}
        self.assertEqual(waits["trial_result_wait"], "result_card_awaiting_being_response")
        self.assertEqual(waits["trial_proposal_wait"], "proposal_card_awaiting_operator_decision")
        self.assertEqual(closure_loop["summary"]["result_card_awaiting_being_response_count"], 1)
        self.assertEqual(closure_loop["summary"]["proposal_card_awaiting_operator_decision_count"], 1)

    def test_outcome_loop_counts_proposal_and_result_card_needed_separately(self) -> None:
        status = empty_status()
        proposal_needed = {
            "trial_id": "trial_proposal_needed",
            "adapter": "manual_sandbox_review_v1",
            "trial_mode": "approval_required_live_trial",
            "status": "approval_required_live_trial",
            "runnable": False,
            "created_at": now_s(),
            "updated_at": now_s(),
        }
        result_card_needed = {
            "trial_id": "trial_result_card_needed",
            "adapter": "shadow_loss_lattice_v1",
            "trial_mode": "sandbox_replay",
            "status": "result_recorded",
            "runnable": True,
            "results": [{"classification": "lattice_transition_like"}],
            "created_at": now_s(),
            "updated_at": now_s(),
        }
        apply_event(status, trial_created_event(proposal_needed))
        apply_event(status, trial_created_event(result_card_needed))
        closure_loop = being_outcome_closure_loop_v1(status)
        self.assertEqual(closure_loop["summary"]["proposal_card_needed_count"], 1)
        self.assertEqual(closure_loop["summary"]["result_card_needed_count"], 1)

    def test_outcome_loop_is_bounded_and_omits_private_bodies(self) -> None:
        private_body = "PRIVATE_BODY_" * 500
        status = empty_status()
        trial = {
            "trial_id": "trial_private",
            "adapter": "manual_sandbox_review_v1",
            "trial_mode": "read_only_review",
            "status": "ready_for_sandbox",
            "runnable": True,
            "hypothesis": private_body,
            "felt_report_anchor": private_body,
            "created_at": now_s(),
            "updated_at": now_s(),
        }
        apply_event(status, trial_created_event(trial))
        closure_loop = being_outcome_closure_loop_v1(status)
        rendered = json.dumps(closure_loop, sort_keys=True)
        self.assertLess(len(rendered), 12000)
        self.assertNotIn("PRIVATE_BODY_", rendered)

    def test_fallback_adapter_classifies_context_supported_dynamic_terms(self) -> None:
        import tempfile

        global ASTRID_WORKSPACE, ASTRID_JOURNAL, ASTRID_CONTEXT_OVERFLOW, ASTRID_LLM_RS, FALLBACK_FIRE_DRILLS
        old = (ASTRID_WORKSPACE, ASTRID_JOURNAL, ASTRID_CONTEXT_OVERFLOW, ASTRID_LLM_RS, FALLBACK_FIRE_DRILLS)
        try:
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                ASTRID_WORKSPACE = root
                ASTRID_JOURNAL = root / "journal"
                ASTRID_CONTEXT_OVERFLOW = root / "context_overflow"
                ASTRID_LLM_RS = root / "llm.rs"
                FALLBACK_FIRE_DRILLS = root / "fallback_fire_drills"
                ASTRID_JOURNAL.mkdir()
                ASTRID_CONTEXT_OVERFLOW.mkdir()
                FALLBACK_FIRE_DRILLS.mkdir()
                ASTRID_LLM_RS.write_text(
                    "fallback_dynamic_texture_weight_v1 dynamic_texture_weight texture_trajectory_v1",
                    encoding="utf-8",
                )
                (ASTRID_JOURNAL / "dialogue.txt").write_text(
                    "Shadow pressure and entropy make the habitable lattice feel like movement.",
                    encoding="utf-8",
                )
                result = fallback_distinguishability_v1()
        finally:
            (
                ASTRID_WORKSPACE,
                ASTRID_JOURNAL,
                ASTRID_CONTEXT_OVERFLOW,
                ASTRID_LLM_RS,
                FALLBACK_FIRE_DRILLS,
            ) = old
        self.assertEqual(result["classification"], "supported_dynamic")

    def test_shadow_adapter_classifies_lattice_transition(self) -> None:
        import tempfile

        global ASTRID_WORKSPACE, ASTRID_JOURNAL, ASTRID_CONTEXT_OVERFLOW
        old = (ASTRID_WORKSPACE, ASTRID_JOURNAL, ASTRID_CONTEXT_OVERFLOW)
        try:
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                ASTRID_WORKSPACE = root
                ASTRID_JOURNAL = root / "journal"
                ASTRID_CONTEXT_OVERFLOW = root / "context_overflow"
                ASTRID_JOURNAL.mkdir()
                ASTRID_CONTEXT_OVERFLOW.mkdir()
                (ASTRID_JOURNAL / "dialogue.txt").write_text(
                    "[Shadow-v3 (Yours): interwoven lattice settled coupling | trend: norm 0.40->0.36, dispersal potential 0.12->0.18]",
                    encoding="utf-8",
                )
                result = shadow_loss_lattice_v1()
        finally:
            ASTRID_WORKSPACE, ASTRID_JOURNAL, ASTRID_CONTEXT_OVERFLOW = old
        self.assertEqual(result["classification"], "lattice_transition_like")

    def test_shadow_trend_parser_ignores_trailing_sentence_period(self) -> None:
        match = TREND_RE.search(
            "Shadow-v3 trend: norm 0.10->0.20, dispersal potential 0.11->0.21."
        )
        self.assertIsNotNone(match)
        assert match is not None
        self.assertEqual(tuple(match.groups()), ("0.10", "0.20", "0.11", "0.21"))

    def test_shadow_influence_routes_before_generic_texture(self) -> None:
        adapter = candidate_adapter(
            "Doubling shadow influence from 0.018 to 0.036 should be a texture sandbox."
        )
        self.assertEqual(adapter, "shadow_influence_replay_v1")

    def test_pair_specific_claims_do_not_route_to_generic_adapters(self) -> None:
        claims = (
            "Compare primary and fallback texture fidelity in a bounded fire drill.",
            "Compare full32 and 12D identity and Shadow stability on bounded replay evidence.",
            "Observe Shadow-v3 texture trends across Dialogue and Witness without inferring causality.",
        )
        for claim in claims:
            with self.subTest(claim=claim):
                self.assertTrue(requires_manual_pair_comparison(claim))
                self.assertEqual(candidate_adapter(claim), "manual_sandbox_review_v1")

        self.assertEqual(
            candidate_adapter("Review fallback texture support under pressure and entropy."),
            "fallback_distinguishability_v1",
        )
        self.assertEqual(
            candidate_adapter("Review fallback lattice-term support under pressure and entropy."),
            "fallback_distinguishability_v1",
        )
        self.assertEqual(
            candidate_adapter("Observe Shadow-v3 lattice dispersal without changing control."),
            "shadow_loss_lattice_v1",
        )

    def test_runtime_parameter_claims_do_not_route_on_generic_texture_words(self) -> None:
        claims = (
            "Compare fixed exploration noise near 0.085 for high-entropy stuckness and inhabitable foothold.",
            "Compare pressure and porosity thresholds in a bounded replay before any live retune.",
            "Observe mode_packing and texture persistence without changing controller behavior.",
        )
        for claim in claims:
            with self.subTest(claim=claim):
                self.assertEqual(candidate_adapter(claim), "manual_sandbox_review_v1")

    def test_adapter_correction_disables_untouched_misrouted_runner(self) -> None:
        existing = {
            "trial_old": {
                "trial_id": "trial_old",
                "source_work_item_id": "wi_noise",
                "adapter": "fallback_distinguishability_v1",
                "trial_mode": "offline_read_only_adapter",
                "status": "ready_for_sandbox",
                "runnable": True,
                "results": [],
            }
        }
        candidates = [
            {
                "trial_id": "trial_new_identity",
                "source_work_item_id": "wi_noise",
                "agency_tier": 3,
                "adapter": "manual_sandbox_review_v1",
                "trial_mode": "offline_read_only_adapter",
                "status": "ready_for_sandbox",
                "approval_boundary": AUTHORITY_BOUNDARY,
            }
        ]

        events = adapter_correction_events(existing, candidates)
        self.assertEqual(len(events), 1)
        status = {"trials": existing}
        apply_event(status, events[0])
        corrected = status["trials"]["trial_old"]
        self.assertEqual(corrected["adapter"], "manual_sandbox_review_v1")
        self.assertFalse(corrected["runnable"])
        self.assertEqual(corrected["proposed_intervention"], "produce bounded read-only review packet")

    def test_route_correction_releases_untouched_false_live_wait(self) -> None:
        existing = {
            "trial_old": {
                "trial_id": "trial_old",
                "source_work_item_id": "wi_compare",
                "agency_tier": 5,
                "adapter": "manual_sandbox_review_v1",
                "trial_mode": "approval_required_live_trial",
                "status": "approval_required_live_trial",
                "approval_boundary": LIVE_APPROVAL_BOUNDARY,
                "runnable": False,
                "results": [],
            }
        }
        candidates = [
            {
                "trial_id": "trial_new_identity",
                "source_work_item_id": "wi_compare",
                "agency_tier": 3,
                "adapter": "manual_sandbox_review_v1",
                "trial_mode": "offline_read_only_adapter",
                "status": "ready_for_sandbox",
                "approval_boundary": AUTHORITY_BOUNDARY,
            }
        ]

        events = adapter_correction_events(existing, candidates)
        self.assertEqual(len(events), 1)
        status = {"trials": existing}
        apply_event(status, events[0])
        corrected = status["trials"]["trial_old"]
        self.assertEqual(corrected["agency_tier"], 3)
        self.assertEqual(corrected["trial_mode"], "offline_read_only_adapter")
        self.assertEqual(corrected["status"], "ready_for_sandbox")
        self.assertEqual(corrected["approval_boundary"], AUTHORITY_BOUNDARY)
        self.assertFalse(corrected["runnable"])

    def test_route_correction_restores_unchanged_live_wait_adapter(self) -> None:
        existing = {
            "trial_old": {
                "trial_id": "trial_old",
                "source_work_item_id": "wi_live",
                "agency_tier": 5,
                "adapter": "manual_sandbox_review_v1",
                "trial_mode": "approval_required_live_trial",
                "status": "approval_required_live_trial",
                "approval_boundary": LIVE_APPROVAL_BOUNDARY,
                "runnable": False,
                "results": [],
                "adapter_correction": {
                    "old_adapter": "shadow_loss_lattice_v1",
                    "new_adapter": "manual_sandbox_review_v1",
                    "old_mode": "approval_required_live_trial",
                    "new_mode": "approval_required_live_trial",
                    "old_agency_tier": 5,
                    "new_agency_tier": 5,
                    "reason": (
                        "claim-specific routing repair; source labels and broad texture "
                        "words do not override the disposition"
                    ),
                },
            }
        }

        events = adapter_correction_events(existing, [])
        self.assertEqual(len(events), 1)
        status = {"trials": existing}
        apply_event(status, events[0])
        restored = status["trials"]["trial_old"]
        self.assertEqual(restored["adapter"], "shadow_loss_lattice_v1")
        self.assertEqual(restored["agency_tier"], 5)
        self.assertEqual(restored["status"], "approval_required_live_trial")
        self.assertFalse(restored["runnable"])

    def test_shadow_influence_replay_uses_bounded_projection(self) -> None:
        import tempfile

        global ASTRID_WORKSPACE, ASTRID_JOURNAL, ASTRID_CONTEXT_OVERFLOW
        old = (ASTRID_WORKSPACE, ASTRID_JOURNAL, ASTRID_CONTEXT_OVERFLOW)
        try:
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                ASTRID_WORKSPACE = root
                ASTRID_JOURNAL = root / "journal"
                ASTRID_CONTEXT_OVERFLOW = root / "context_overflow"
                ASTRID_JOURNAL.mkdir()
                ASTRID_CONTEXT_OVERFLOW.mkdir()
                (ASTRID_JOURNAL / "dialogue.txt").write_text(
                    "[Shadow-v3 (Yours): interwoven lattice settled coupling | trend: norm 0.40->0.36, dispersal potential 0.12->0.18]",
                    encoding="utf-8",
                )
                result = shadow_influence_replay_v1(
                    {
                        "hypothesis": "double shadow influence",
                        "felt_report_anchor": "from 0.018 to 0.036",
                    }
                )
        finally:
            ASTRID_WORKSPACE, ASTRID_JOURNAL, ASTRID_CONTEXT_OVERFLOW = old
        self.assertEqual(result["adapter"], "shadow_influence_replay_v1")
        self.assertEqual(result["requested_multiplier"], 2.0)
        self.assertIn(result["classification"], {"replay_supports_bounded_shadow_gain", "replay_warns_fragmentation_risk"})
        self.assertLess(len(json.dumps(result)), 12000)

    def test_run_next_records_result_card_and_skips_live_candidates(self) -> None:
        import tempfile

        global ASTRID_WORKSPACE, ASTRID_JOURNAL, ASTRID_CONTEXT_OVERFLOW
        old = (ASTRID_WORKSPACE, ASTRID_JOURNAL, ASTRID_CONTEXT_OVERFLOW)
        try:
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                ASTRID_WORKSPACE = root / "workspace"
                ASTRID_JOURNAL = ASTRID_WORKSPACE / "journal"
                ASTRID_CONTEXT_OVERFLOW = ASTRID_WORKSPACE / "context_overflow"
                ASTRID_JOURNAL.mkdir(parents=True)
                ASTRID_CONTEXT_OVERFLOW.mkdir(parents=True)
                (ASTRID_JOURNAL / "dialogue.txt").write_text(
                    "[Shadow-v3 (Yours): interwoven lattice settled coupling | trend: norm 0.40->0.36, dispersal potential 0.12->0.18]",
                    encoding="utf-8",
                )
                state_dir = root / "state"
                live = {
                    "trial_id": "trial_live",
                    "adapter": "shadow_influence_replay_v1",
                    "trial_mode": "approval_required_live_trial",
                    "status": "approval_required_live_trial",
                    "runnable": False,
                    "agency_tier": 5,
                    "created_at": now_s(),
                    "updated_at": now_s(),
                }
                runnable = {
                    "trial_id": "trial_run",
                    "adapter": "shadow_influence_replay_v1",
                    "trial_mode": "sandbox_replay",
                    "status": "ready_for_sandbox",
                    "runnable": True,
                    "agency_tier": 3,
                    "hypothesis": "double shadow influence",
                    "felt_report_anchor": "from 0.018 to 0.036",
                    "created_at": now_s(),
                    "updated_at": now_s(),
                }
                append_events(state_dir, [trial_created_event(live), trial_created_event(runnable)])
                payload = run_ready_trials(
                    state_dir,
                    limit=2,
                    write=True,
                    link_evidence=False,
                    emit_card=True,
                )
                status = replay_status(state_dir)
        finally:
            ASTRID_WORKSPACE, ASTRID_JOURNAL, ASTRID_CONTEXT_OVERFLOW = old
        self.assertEqual(payload["selected_count"], 1)
        self.assertEqual(status["trials"]["trial_run"]["status"], "result_recorded")
        self.assertEqual(status["trials"]["trial_live"]["status"], "approval_required_live_trial")
        self.assertEqual(len(status["trials"]["trial_run"].get("result_cards") or []), 1)

    def test_addressing_events_for_result_links_work_and_claim(self) -> None:
        trial = {
            "trial_id": "trial_test",
            "source_work_item_id": "wi_test",
            "source_introspection_id": "intro_test",
            "claim_id": "c001",
        }
        result = {"adapter": "shadow_influence_replay_v1", "classification": "replay_supports_bounded_shadow_gain"}
        events = addressing_events_for_result(
            trial,
            result,
            json_path="/tmp/result.json",
            markdown_path="/tmp/result.md",
            card_path="/tmp/card.md",
        )
        event_types = [event.get("event_type") for event in events]
        self.assertIn("work_evidence_linked", event_types)
        self.assertIn("evidence_linked", event_types)
        self.assertIn("post_change_response_recorded", event_types)


def run_self_tests() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(SandboxTrialQueueTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state-dir", type=Path, default=DEFAULT_STATE_DIR)
    parser.add_argument("--self-test", action="store_true")
    sub = parser.add_subparsers(dest="cmd")

    gen_p = sub.add_parser("generate")
    gen_p.add_argument("--write", action="store_true")
    gen_p.add_argument("--json", action="store_true")

    report_p = sub.add_parser("report")
    report_p.add_argument("--json", action="store_true")
    report_p.add_argument("--markdown", action="store_true")

    queue_p = sub.add_parser("queue")
    queue_p.add_argument("--json", action="store_true")
    queue_p.add_argument("--markdown", action="store_true")

    run_next_p = sub.add_parser("run-next")
    run_next_p.add_argument("--limit", type=int, default=1)
    run_next_p.add_argument("--adapter", default=None)
    run_next_p.add_argument("--write", action="store_true")
    run_next_p.add_argument("--json", action="store_true")
    run_next_p.add_argument("--no-evidence-link", action="store_true")
    run_next_p.add_argument("--no-closure-card", action="store_true")

    run_p = sub.add_parser("run-adapter")
    run_p.add_argument("--trial-id")
    run_p.add_argument("--all-ready", action="store_true")
    run_p.add_argument("--limit", type=int, default=1)
    run_p.add_argument("--write", action="store_true")
    run_p.add_argument("--json", action="store_true")
    run_p.add_argument("--no-evidence-link", action="store_true")
    run_p.add_argument("--no-closure-card", action="store_true")

    proposal_p = sub.add_parser("emit-proposal-card")
    proposal_p.add_argument("--trial-id", action="append", default=[])
    proposal_p.add_argument("--next", type=int, default=0)
    proposal_p.add_argument("--write", action="store_true")
    proposal_p.add_argument("--json", action="store_true")

    ladder_p = sub.add_parser("ladder")
    ladder_p.add_argument("--json", action="store_true")
    ladder_p.add_argument("--markdown", action="store_true")
    ladder_p.add_argument("--write", action="store_true")

    args = parser.parse_args(argv)
    if args.self_test:
        return run_self_tests()
    if args.cmd == "generate":
        payload = generate_candidates(args.state_dir, write=bool(args.write))
        if args.json or not args.write:
            print(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False))
        else:
            print(render_queue_markdown(load_status(args.state_dir)), end="")
        return 0
    if args.cmd in {"report", "queue"}:
        status = load_status(args.state_dir)
        report = report_from_status(status)
        if args.json:
            print(json.dumps(report, indent=2, sort_keys=True, ensure_ascii=False))
        else:
            print(render_queue_markdown(status), end="")
        return 0
    if args.cmd == "run-next":
        payload = run_ready_trials(
            args.state_dir,
            limit=max(1, int(args.limit or 1)),
            adapter=args.adapter,
            write=bool(args.write),
            link_evidence=not bool(args.no_evidence_link),
            emit_card=not bool(args.no_closure_card),
        )
        print(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False))
        return 0
    if args.cmd == "run-adapter":
        if args.all_ready:
            payload = run_ready_trials(
                args.state_dir,
                limit=max(1, int(args.limit or 1)),
                write=bool(args.write),
                link_evidence=not bool(args.no_evidence_link),
                emit_card=not bool(args.no_closure_card),
            )
        else:
            if not args.trial_id:
                raise SystemExit("run-adapter requires --trial-id unless --all-ready is set")
            payload = run_adapter_for_trial(
                args.state_dir,
                args.trial_id,
                write=bool(args.write),
                link_evidence=not bool(args.no_evidence_link),
                emit_card=not bool(args.no_closure_card),
            )
        if args.json or not args.write:
            print(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False))
        else:
            print(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False))
        return 0
    if args.cmd == "emit-proposal-card":
        trial_ids = list(args.trial_id or [])
        if args.next:
            trial_ids.extend(
                str(trial.get("trial_id") or "")
                for trial in active_trials(replay_status(args.state_dir))[: max(1, int(args.next))]
            )
        if not trial_ids:
            raise SystemExit("emit-proposal-card requires --trial-id or --next")
        payload = emit_proposal_cards_for_trials(args.state_dir, trial_ids=trial_ids, write=bool(args.write))
        print(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False))
        return 0
    if args.cmd == "ladder":
        status = load_status(args.state_dir)
        ladder = consentful_sandbox_to_live_ladder_v1(status)
        closure_loop = being_outcome_closure_loop_v1(status)
        if args.write:
            write_ladder_artifacts(args.state_dir, ladder)
            write_closure_artifacts(args.state_dir, closure_loop)
        if args.markdown and not args.json:
            print(render_ladder_markdown(ladder), end="")
        else:
            print(json.dumps(ladder, indent=2, sort_keys=True, ensure_ascii=False))
        return 0
    parser.print_help()
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
