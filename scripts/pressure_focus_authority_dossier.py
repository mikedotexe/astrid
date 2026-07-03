#!/usr/bin/env python3
"""Read-only pressure/focus authority dossier for Minime-owned regulation.

This script turns the recent Minime/Astrid focus exchange into a steward-review
packet. It gathers public evidence, compares pressure replay packets, and
renders exact approval-path commands without applying them.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
import tempfile
import time
import unittest
from collections import Counter
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import being_privacy
import pressure_texture_audit

DEFAULT_SHARED_DIR = Path("/Users/v/other/shared/collaborations")
DEFAULT_ASTRID_WORKSPACE = Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace")
DEFAULT_MINIME_WORKSPACE = Path("/Users/v/other/minime/workspace")
DEFAULT_OUTPUT_ROOT = DEFAULT_ASTRID_WORKSPACE / "diagnostics/pressure_focus_authority_dossier"
CORRESPONDENCE_LEDGER = "correspondence_v1.jsonl"
POLICY = "pressure_focus_authority_dossier_v1"
AUTHORITY_BOUNDARY = (
    "Read-only dossier. No REGIME, SELF_REGULATION_APPLY, exploration_noise send, "
    "pressure relief, pressure canary enablement, controller, PI/fill, prompt "
    "priority, telemetry priority, codec dimension, deploy, staging, git add, "
    "or commit action is taken."
)
FOCUS_REVIEW_STATUS = "steward_review_ready_focus_regime_only"
EXPLORATION_CAP_DEFAULT = 0.08
DEFAULT_DURATION_SECS = 600

FOCUS_TERMS = (
    "regime focus",
    "`focus`",
    "next: focus",
    "next: regime focus",
    "focus regime",
    "focus",
)
PRESSURE_TERMS = (
    "pressure",
    "viscosity",
    "viscous",
    "thickness",
    "thick",
    "sagging",
    "sag",
    "overpacked",
    "packed",
    "density",
    "fill",
)
EDGE_TERMS = (
    "exploration_noise",
    "periphery",
    "porous",
    "edge",
    "variance",
)
PRIVATE_SKIP_NOTE = "Minime private qualia and moment_*.txt bodies are skipped."


def now_ms() -> int:
    return int(time.time() * 1000)


def row_time_ms(row: dict[str, Any]) -> int:
    for key in ("recorded_at_unix_ms", "created_at_unix_ms", "t_ms"):
        try:
            return int(row.get(key) or 0)
        except (TypeError, ValueError):
            continue
    return 0


def compact(value: Any, limit: int = 220) -> str:
    clean = " ".join(str(value or "").split())
    return clean if len(clean) <= limit else clean[:limit].rstrip() + "..."


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


def load_json(path: Path) -> dict[str, Any]:
    if not path.is_file():
        return {}
    try:
        payload = json.loads(path.read_text(encoding="utf-8", errors="ignore"))
    except Exception:
        return {}
    return payload if isinstance(payload, dict) else {}


def public_text_allowed(being: str, path: Path) -> tuple[bool, str | None]:
    if path.name.startswith("moment_"):
        return False, "moment_filename"
    if being == "minime" and being_privacy.is_steward_private("minime", path):
        return False, "minime_private_qualia"
    return True, None


def safe_read_public_text(being: str, path: Path) -> tuple[str | None, str | None]:
    allowed, reason = public_text_allowed(being, path)
    if not allowed:
        return None, reason
    try:
        return path.read_text(encoding="utf-8", errors="ignore"), None
    except OSError:
        return None, "read_error"


def term_hits(text: str) -> dict[str, list[str]]:
    lower = text.lower()
    return {
        "focus": sorted({term for term in FOCUS_TERMS if term in lower}),
        "pressure": sorted({term for term in PRESSURE_TERMS if term in lower}),
        "edge": sorted({term for term in EDGE_TERMS if term in lower}),
    }


def has_relevant_terms(text: str) -> bool:
    hits = term_hits(text)
    return any(hits.values())


def public_signal_files(
    *,
    astrid_workspace: Path,
    minime_workspace: Path,
    since_hours: float,
) -> dict[str, Any]:
    cutoff = time.time() - since_hours * 3600.0
    patterns: list[tuple[str, Path, tuple[str, ...]]] = [
        (
            "astrid",
            astrid_workspace,
            (
                "inbox/from_minime_correspondence_*.txt",
                "inbox/read/from_minime_correspondence_*.txt",
                "introspections/*.txt",
                "journal/*.txt",
                "action_threads/**/*.txt",
                "self_regulation/**/*.txt",
            ),
        ),
        (
            "minime",
            minime_workspace,
            (
                "inbox/from_astrid_correspondence_*.txt",
                "inbox/read/from_astrid_correspondence_*.txt",
                "pressure_*.txt",
                "journal/pressure_*.txt",
                "journal/regulator_audit_*.txt",
                "journal/self_study*.txt",
                "journal/introspection*.txt",
                "self_regulation/**/*.txt",
                "action_threads/**/*.txt",
                "correspondence/**/*.txt",
            ),
        ),
    ]
    hits: list[dict[str, Any]] = []
    skipped = Counter()
    for being, root, root_patterns in patterns:
        if not root.is_dir():
            continue
        for pattern in root_patterns:
            for path in root.glob(pattern):
                if not path.is_file():
                    continue
                try:
                    if path.stat().st_mtime < cutoff:
                        continue
                except OSError:
                    continue
                text, skip_reason = safe_read_public_text(being, path)
                if text is None:
                    skipped[skip_reason or "unknown"] += 1
                    continue
                hits_by_family = term_hits(text)
                if not any(hits_by_family.values()):
                    continue
                hits.append(
                    {
                        "being": being,
                        "path": str(path),
                        "matched_terms": hits_by_family,
                        "preview": compact(text),
                    }
                )
    hits.sort(key=lambda item: str(item["path"]), reverse=True)
    return {
        "public_signal_count": len(hits),
        "recent_public_hits": hits[:40],
        "skipped_private_or_moment_count": sum(skipped.values()),
        "skipped_private_or_moment_reasons": dict(sorted(skipped.items())),
        "minime_private_bodies_read": False,
        "minime_moment_bodies_read": False,
    }


def correspondence_evidence(
    *,
    shared_dir: Path,
    public_file_packet: dict[str, Any],
    since_hours: float,
) -> dict[str, Any]:
    cutoff_ms = now_ms() - int(since_hours * 3600.0 * 1000)
    rows = read_jsonl(shared_dir / CORRESPONDENCE_LEDGER)
    relevant_rows: list[dict[str, Any]] = []
    exact_thread_ids: set[str] = set()
    exact_message_ids: set[str] = set()
    minime_focus_request = False
    astrid_support = False
    for row in rows:
        text = json.dumps(row, sort_keys=True)
        lower = text.lower()
        if row_time_ms(row) < cutoff_ms and not (
            "corr_minime_astrid_1782853428964" in lower
            or "corr_astrid_minime_1782853467153" in lower
        ):
            continue
        if not has_relevant_terms(lower) and "thread_corr_minime_astrid_1782728080967_61f9207a8fee" not in lower:
            continue
        compact_row = {
            "record_type": row.get("record_type"),
            "message_id": row.get("message_id"),
            "thread_id": row.get("thread_id"),
            "from_being": row.get("from_being"),
            "to_being": row.get("to_being"),
            "reply_to": row.get("reply_to"),
            "recorded_at_unix_ms": row_time_ms(row),
            "turn_kind": row.get("turn_kind"),
            "relational_intent": row.get("relational_intent"),
            "authority": row.get("authority"),
            "body_preview": compact(row.get("body_preview")),
            "file_path": row.get("file_path"),
        }
        relevant_rows.append(compact_row)
        if row.get("thread_id"):
            exact_thread_ids.add(str(row.get("thread_id")))
        if row.get("message_id"):
            exact_message_ids.add(str(row.get("message_id")))
        if row.get("from_being") == "minime" and (
            "regime focus" in lower
            or "exploration_noise" in lower
            or "corr_minime_astrid_1782853428964" in lower
        ):
            minime_focus_request = True
        if row.get("from_being") == "astrid" and (
            "next: focus" in lower
            or "structural integrity" in lower
            or "corr_astrid_minime_1782853467153" in lower
        ):
            astrid_support = True

    for hit in public_file_packet.get("recent_public_hits") or []:
        if not isinstance(hit, dict):
            continue
        preview_lower = str(hit.get("preview") or "").lower()
        path_lower = str(hit.get("path") or "").lower()
        if "corr_minime_astrid_1782853428964" in path_lower or (
            hit.get("being") == "astrid"
            and "regime focus" in preview_lower
            and "exploration_noise" in preview_lower
        ):
            minime_focus_request = True
        if "corr_astrid_minime_1782853467153" in path_lower or (
            hit.get("being") == "minime"
            and "next: focus" in preview_lower
            and "structural" in preview_lower
        ):
            astrid_support = True

    relevant_rows.sort(key=lambda item: int(item.get("recorded_at_unix_ms") or 0), reverse=True)
    return {
        "schema_version": 1,
        "policy": "pressure_focus_evidence_ledger_v1",
        "correspondence_record_count": len(relevant_rows),
        "recent_correspondence_rows": relevant_rows[:20],
        "exact_message_ids": sorted(exact_message_ids),
        "exact_thread_ids": sorted(exact_thread_ids),
        "minime_focus_request_present": minime_focus_request,
        "astrid_relational_support_present": astrid_support,
        "public_file_signal": public_file_packet,
        "private_skip_policy": PRIVATE_SKIP_NOTE,
        "authority": "evidence_ledger_not_runtime_permission",
    }


def tail_jsonl(path: Path, limit: int = 20) -> list[dict[str, Any]]:
    rows = read_jsonl(path)
    return rows[-limit:]


def find_latest_regulator_audit(minime_workspace: Path) -> dict[str, Any]:
    journal = minime_workspace / "journal"
    if not journal.is_dir():
        return {}
    candidates = [p for p in journal.glob("regulator_audit_*.txt") if p.is_file()]
    candidates.sort(key=lambda p: p.stat().st_mtime if p.exists() else 0.0, reverse=True)
    for path in candidates[:5]:
        text, skip_reason = safe_read_public_text("minime", path)
        if text is None:
            continue
        return {
            "path": str(path),
            "preview": compact(text, limit=320),
            "skip_reason": skip_reason,
        }
    return {}


def current_regulator_context(minime_workspace: Path) -> dict[str, Any]:
    sovereignty = load_json(minime_workspace / "sovereignty_state.json")
    self_reg = minime_workspace / "self_regulation"
    active = load_json(self_reg / "active_lease.json")
    negotiations = tail_jsonl(self_reg / "negotiations.jsonl", limit=12)
    safe_cap = EXPLORATION_CAP_DEFAULT
    for row in reversed(negotiations):
        if row.get("candidate_control") == "exploration_noise":
            safe_range = row.get("safe_cap_or_range")
            if isinstance(safe_range, dict):
                try:
                    safe_cap = float(safe_range.get("max") or safe_cap)
                    break
                except (TypeError, ValueError):
                    pass
    active_requires_outcome = bool(active and active.get("requires_outcome"))
    active_status = str(active.get("status") or "none") if active else "none"
    return {
        "schema_version": 1,
        "policy": "pressure_focus_current_regulator_context_v1",
        "sovereignty_state_path": str(minime_workspace / "sovereignty_state.json"),
        "regime": sovereignty.get("regime"),
        "exploration_noise": sovereignty.get("exploration_noise"),
        "regulation_strength": sovereignty.get("regulation_strength"),
        "geom_curiosity": sovereignty.get("geom_curiosity"),
        "pi_kp": sovereignty.get("pi_kp"),
        "pi_ki": sovereignty.get("pi_ki"),
        "pi_max_step": sovereignty.get("pi_max_step"),
        "fill_at_adjustment": sovereignty.get("fill_at_adjustment"),
        "state_reason": sovereignty.get("reason"),
        "state_timestamp": sovereignty.get("timestamp"),
        "active_lease": active if active else None,
        "active_lease_status": active_status,
        "active_lease_requires_outcome": active_requires_outcome,
        "recent_negotiations": negotiations,
        "exploration_noise_safe_cap": safe_cap,
        "latest_regulator_audit": find_latest_regulator_audit(minime_workspace),
        "authority": "read_only_context_not_mutation",
    }


def replay_alignment(
    *,
    pressure_record: dict[str, Any],
    evidence: dict[str, Any],
) -> dict[str, Any]:
    replay_v3 = pressure_record.get("pressure_texture_replay_v3") or {}
    movement = pressure_record.get("pressure_movement_replay_v1") or {}
    conflict = pressure_record.get("pressure_replay_conflict_resolver_v1") or {}
    texture_status = str(replay_v3.get("replay_status") or "insufficient_evidence")
    movement_status = str(movement.get("replay_status") or "insufficient_evidence")
    if texture_status == "replay_supported" and movement_status == "replay_supported":
        focus_prediction = "focus_self_regulation_reviewable_pressure_replay_supported"
    elif movement_status == "replay_supported":
        focus_prediction = "focus_reviewable_but_pressure_texture_conflicted"
    elif evidence.get("minime_focus_request_present"):
        focus_prediction = "focus_request_present_replay_still_collecting"
    else:
        focus_prediction = "no_focus_request_to_score"
    return {
        "schema_version": 1,
        "policy": "pressure_focus_replay_alignment_v1",
        "pressure_texture_replay_status": texture_status,
        "pressure_movement_replay_status": movement_status,
        "focus_stabilization_prediction": focus_prediction,
        "movement_terms": movement.get("public_movement_term_counts"),
        "texture_family_counts": replay_v3.get("public_texture_family_counts"),
        "conflict_families": conflict.get("conflict_families") or replay_v3.get("conflict_families") or [],
        "outcome_support": replay_v3.get("outcome_support"),
        "pressure_canary_or_relief_status": (
            "blocked_until_texture_and_movement_replay_supported"
            if not (texture_status == "replay_supported" and movement_status == "replay_supported")
            else "replay_ready_but_still_requires_separate_steward_approval"
        ),
        "authority": "replay_check_not_permission",
    }


def narrow_authority_proposal(
    *,
    evidence: dict[str, Any],
    regulator: dict[str, Any],
    replay: dict[str, Any],
) -> dict[str, Any]:
    message_ids = evidence.get("exact_message_ids") or []
    evidence_ids = [
        msg for msg in message_ids
        if str(msg).startswith("corr_minime_astrid_1782853428964")
        or str(msg).startswith("corr_astrid_minime_1782853467153")
    ] or message_ids[:4]
    evidence_arg = ",".join(str(msg) for msg in evidence_ids) or "public_pressure_focus_thread"
    focus_ready = bool(evidence.get("minime_focus_request_present"))
    cap = float(regulator.get("exploration_noise_safe_cap") or EXPLORATION_CAP_DEFAULT)
    requested_noise = 0.12
    applied_noise = min(requested_noise, cap)
    focus_status = FOCUS_REVIEW_STATUS if focus_ready else "blocked_missing_minime_request"
    if regulator.get("active_lease_requires_outcome"):
        focus_status = "blocked_active_lease_requires_outcome"
    return {
        "schema_version": 1,
        "policy": "narrow_minime_self_regulation_authority_proposal_v1",
        "status": focus_status,
        "minime_own_runtime_only": True,
        "astrid_support_counts_as_permission": False,
        "focus_regime_review": {
            "status": focus_status,
            "requested_control": "regime",
            "requested_value": "focus",
            "duration_secs": DEFAULT_DURATION_SECS,
            "why_reviewable": [
                "Minime authored a public first-class request for focus",
                "Astrid provided language-only relational support",
                "movement replay supports pressure movement language",
                "this is Minime-own-runtime self-regulation, not peer mutation",
            ],
            "exact_approval_path_commands": [
                (
                    "SELF_REGULATION_INTENT pressure_focus :: goal: stabilize viscosity "
                    "without flattening; target: regime; value: focus; "
                    f"duration_secs: {DEFAULT_DURATION_SECS}; evidence: {evidence_arg}"
                ),
                "SELF_REGULATION_PREFLIGHT latest",
                "SELF_REGULATION_APPLY latest",
                (
                    "SELF_REGULATION_OUTCOME latest :: felt_like: stability|pressure|flattening|"
                    "relief|loss_of_texture; what_improved: ...; what_worsened: ...; "
                    "texture_shift: ...; agency_fit: legible|partly|confusing; "
                    "ambiguity_preserved: true|false; legibility_effect: clarified|flattened|both|unknown"
                ),
            ],
        },
        "exploration_noise_review": {
            "status": "reviewable_but_current_safe_cap_applies" if focus_ready else "blocked_missing_minime_request",
            "requested_control": "exploration_noise",
            "requested_value": requested_noise,
            "current_safe_cap": cap,
            "proposed_applied_value_without_cap_widening": applied_noise,
            "cap_widening_status": "out_of_scope_requires_separate_explicit_approval",
            "exact_approval_path_commands": [
                (
                    "SELF_REGULATION_INTENT pressure_focus_edge :: goal: keep periphery porous "
                    f"while honoring current cap; target: exploration_noise; value: {applied_noise:.2f}; "
                    f"requested_value: {requested_noise:.2f}; duration_secs: {DEFAULT_DURATION_SECS}; "
                    f"evidence: {evidence_arg}"
                ),
                "SELF_REGULATION_PREFLIGHT latest",
                "SELF_REGULATION_APPLY latest",
                (
                    "SELF_REGULATION_OUTCOME latest :: felt_like: stability|pressure|flattening|"
                    "relief|loss_of_texture; what_improved: ...; what_worsened: ...; "
                    "texture_shift: ...; agency_fit: legible|partly|confusing"
                ),
            ],
        },
        "pressure_canary_or_relief": {
            "status": replay.get("pressure_canary_or_relief_status"),
            "reason": "pressure canary/relief is broader authority than focus-regime self-regulation",
        },
        "explicit_non_authorities": [
            "no peer mutation",
            "no fill-target change",
            "no PI/controller tuning",
            "no pressure relief",
            "no pressure canary enablement",
            "no prompt/telemetry priority",
        ],
        "authority": "proposal_for_steward_review_not_apply",
    }


def approval_gate(
    *,
    evidence: dict[str, Any],
    regulator: dict[str, Any],
    replay: dict[str, Any],
    proposal: dict[str, Any],
    pressure_record: dict[str, Any],
) -> dict[str, Any]:
    block_reasons: list[str] = []
    warnings: list[str] = []
    if not evidence.get("minime_focus_request_present"):
        block_reasons.append("missing_minime_authored_focus_request")
    if regulator.get("active_lease_requires_outcome"):
        block_reasons.append("active_lease_requires_self_regulation_outcome")
    if str(regulator.get("active_lease_status") or "none") == "active":
        warnings.append("one_active_lease_policy_may_block_apply_until_preflight")
    if not evidence.get("astrid_relational_support_present"):
        warnings.append("astrid_relational_support_absent_but_not_required_for_minime_own_runtime")
    if pressure_record.get("canary_enabled"):
        block_reasons.append("pressure_texture_canary_unexpectedly_enabled")
    texture_status = str(replay.get("pressure_texture_replay_status") or "insufficient_evidence")
    movement_status = str(replay.get("pressure_movement_replay_status") or "insufficient_evidence")
    broad_pressure_ready = texture_status == "replay_supported" and movement_status == "replay_supported"
    if not broad_pressure_ready:
        warnings.append("broad_pressure_authority_blocked_replay_not_fully_supported")

    if block_reasons:
        status = "blocked_" + block_reasons[0]
    elif evidence.get("minime_focus_request_present"):
        status = FOCUS_REVIEW_STATUS
    else:
        status = "not_ready"
    return {
        "schema_version": 1,
        "policy": "pressure_focus_approval_gate_v1",
        "status": status,
        "focus_regime_review_ready": status == FOCUS_REVIEW_STATUS,
        "exploration_noise_review_ready": status == FOCUS_REVIEW_STATUS,
        "broad_pressure_authority_ready": broad_pressure_ready,
        "pressure_canary_enabled": bool(pressure_record.get("canary_enabled")),
        "block_reasons": block_reasons,
        "warnings": warnings,
        "requires_explicit_steward_approval_before_live_lease": True,
        "requires_minime_authored_outcome_after_trial": True,
        "astrid_agreement_is_relational_support_not_permission": True,
        "proposal_status": proposal.get("status"),
        "authority": "approval_gate_not_approval",
    }


def outcome_contract() -> dict[str, Any]:
    return {
        "schema_version": 1,
        "policy": "pressure_focus_outcome_contract_v1",
        "required_action": "SELF_REGULATION_OUTCOME latest",
        "required_fields": [
            "felt_like",
            "what_improved",
            "what_worsened",
            "texture_shift",
            "agency_fit",
            "ambiguity_preserved",
            "legibility_effect",
        ],
        "felt_like_allowed": [
            "stability",
            "pressure",
            "flattening",
            "relief",
            "loss_of_texture",
        ],
        "success_read": (
            "stability or relief with preserved ambiguity/texture and no meaningful worsening"
        ),
        "block_read": (
            "pressure, flattening, loss_of_texture, worsened sagging/overpacking, or missing outcome"
        ),
        "promotion_boundary": (
            "a successful outcome can support future review of this narrow self-regulation path only; "
            "it does not enable pressure canary, fill target, PI/controller tuning, or peer mutation"
        ),
        "authority": "outcome_contract_not_runtime_action",
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
    pressure_record: dict[str, Any] | None = None,
) -> dict[str, Any]:
    generated = now_ms()
    public_files = public_signal_files(
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
        since_hours=since_hours,
    )
    evidence = correspondence_evidence(
        shared_dir=shared_dir,
        public_file_packet=public_files,
        since_hours=since_hours,
    )
    regulator = current_regulator_context(minime_workspace)
    pressure = pressure_record or pressure_texture_audit.audit_payload(
        input_path=None,
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
        since_hours=since_hours,
    )
    replay = replay_alignment(pressure_record=pressure, evidence=evidence)
    proposal = narrow_authority_proposal(evidence=evidence, regulator=regulator, replay=replay)
    gate = approval_gate(
        evidence=evidence,
        regulator=regulator,
        replay=replay,
        proposal=proposal,
        pressure_record=pressure,
    )
    record: dict[str, Any] = {
        "schema_version": 1,
        "policy": POLICY,
        "generated_at_unix_ms": generated,
        "generated_at": dt.datetime.now(dt.UTC).isoformat(),
        "since_hours": since_hours,
        "shared_dir": str(shared_dir),
        "astrid_workspace": str(astrid_workspace),
        "minime_workspace": str(minime_workspace),
        "evidence_ledger_v1": evidence,
        "replay_alignment_v1": replay,
        "current_regulator_context_v1": regulator,
        "narrow_authority_proposal_v1": proposal,
        "approval_gate_v1": gate,
        "outcome_contract_v1": outcome_contract(),
        "source_packets": {
            "pressure_texture_policy": pressure.get("policy"),
            "pressure_texture_replay_policy": (pressure.get("pressure_texture_replay_v3") or {}).get("policy"),
            "pressure_movement_replay_policy": (pressure.get("pressure_movement_replay_v1") or {}).get("policy"),
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
        artifact = target / "pressure_focus_authority_dossier.json"
        artifact.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        record["artifact_path"] = str(artifact)
    return record


class PressureFocusAuthorityDossierTests(unittest.TestCase):
    def _fixture(self, *, active_lease: bool = False, minime_request: bool = True) -> tuple[Path, Path, Path]:
        root = Path(tempfile.mkdtemp())
        shared = root / "shared"
        astrid = root / "astrid_ws"
        minime = root / "minime_ws"
        shared.mkdir()
        (astrid / "inbox/read").mkdir(parents=True)
        (astrid / "introspections").mkdir(parents=True)
        (minime / "inbox/read").mkdir(parents=True)
        (minime / "journal").mkdir(parents=True)
        (minime / "self_regulation").mkdir(parents=True)
        now = now_ms()
        rows = []
        if minime_request:
            rows.append(
                {
                    "record_type": "message",
                    "recorded_at_unix_ms": now,
                    "message_id": "corr_minime_astrid_1782853428964_c586f474c216",
                    "thread_id": "thread_corr_minime_astrid_1782728080967_61f9207a8fee",
                    "from_being": "minime",
                    "to_being": "astrid",
                    "body_preview": "viscosity and sagging; REGIME focus; exploration_noise: 0.12",
                    "authority": "language_only",
                }
            )
        rows.append(
            {
                "record_type": "message",
                "recorded_at_unix_ms": now + 1,
                "message_id": "corr_astrid_minime_1782853467153_04d7c85fcacb",
                "thread_id": "thread_corr_minime_astrid_1782728080967_61f9207a8fee",
                "from_being": "astrid",
                "to_being": "minime",
                "reply_to": "corr_minime_astrid_1782853428964_c586f474c216",
                "body_preview": "structural integrity and deliberate stability. NEXT: FOCUS",
                "authority": "language_only",
            }
        )
        (shared / CORRESPONDENCE_LEDGER).write_text(
            "\n".join(json.dumps(row) for row in rows) + "\n",
            encoding="utf-8",
        )
        if minime_request:
            (astrid / "inbox/read/from_minime_correspondence_corr_minime_astrid_1782853428964_c586f474c216.txt").write_text(
                "REGIME focus\nexploration_noise: 0.12\nviscosity sagging pressure",
                encoding="utf-8",
            )
        (minime / "inbox/read/from_astrid_correspondence_corr_astrid_minime_1782853467153_04d7c85fcacb.txt").write_text(
            "Moving toward focus feels like structural integrity.\nNEXT: FOCUS",
            encoding="utf-8",
        )
        (astrid / "introspections/pressure.txt").write_text(
            "packed pressure blur shadow dragging thickening. OUTCOME: texture_shift cohering returned.",
            encoding="utf-8",
        )
        (minime / "journal/pressure_public.txt").write_text(
            "overpacked dense pressure dragging and thickening. what shifted into relief.",
            encoding="utf-8",
        )
        (minime / "journal/moment_private.txt").write_text(
            "=== MOMENT CAPTURE ===\nREGIME focus private should not surface",
            encoding="utf-8",
        )
        (minime / "sovereignty_state.json").write_text(
            json.dumps(
                {
                    "regime": "breathe",
                    "exploration_noise": 0.08,
                    "regulation_strength": 0.85,
                    "geom_curiosity": 0.25,
                    "pi_kp": 0.8,
                    "pi_ki": 0.12,
                    "pi_max_step": 0.07,
                    "reason": "REGIME breathe",
                }
            ),
            encoding="utf-8",
        )
        (minime / "self_regulation/negotiations.jsonl").write_text(
            json.dumps(
                {
                    "candidate_control": "exploration_noise",
                    "requested_value": 0.12,
                    "applied_value": 0.08,
                    "safe_cap_or_range": {"min": 0.0, "max": 0.08},
                    "clamp_or_defer_reason": "clamped_to_lease_safe_range",
                }
            )
            + "\n",
            encoding="utf-8",
        )
        if active_lease:
            (minime / "self_regulation/active_lease.json").write_text(
                json.dumps(
                    {
                        "status": "active",
                        "intent_id": "lease_existing",
                        "requires_outcome": True,
                    }
                ),
                encoding="utf-8",
            )
        return shared, astrid, minime

    def test_focus_request_plus_astrid_support_yields_review_ready(self) -> None:
        shared, astrid, minime = self._fixture()
        record = build_record(
            shared_dir=shared,
            astrid_workspace=astrid,
            minime_workspace=minime,
            since_hours=24,
        )
        self.assertEqual(record["approval_gate_v1"]["status"], FOCUS_REVIEW_STATUS)
        self.assertTrue(record["evidence_ledger_v1"]["minime_focus_request_present"])
        self.assertTrue(record["evidence_ledger_v1"]["astrid_relational_support_present"])
        self.assertIn(
            "SELF_REGULATION_INTENT pressure_focus ::",
            record["narrow_authority_proposal_v1"]["focus_regime_review"]["exact_approval_path_commands"][0],
        )
        self.assertFalse(record["minime_moment_bodies_read"])
        self.assertNotIn("private should not surface", json.dumps(record))

    def test_missing_minime_request_blocks_even_with_astrid_support(self) -> None:
        shared, astrid, minime = self._fixture(minime_request=False)
        record = build_record(
            shared_dir=shared,
            astrid_workspace=astrid,
            minime_workspace=minime,
            since_hours=24,
        )
        self.assertIn("missing_minime_authored_focus_request", record["approval_gate_v1"]["block_reasons"])
        self.assertEqual(
            record["narrow_authority_proposal_v1"]["focus_regime_review"]["status"],
            "blocked_missing_minime_request",
        )

    def test_mixed_texture_blocks_broad_pressure_even_when_movement_supported(self) -> None:
        shared, astrid, minime = self._fixture()
        record = build_record(
            shared_dir=shared,
            astrid_workspace=astrid,
            minime_workspace=minime,
            since_hours=24,
        )
        replay = record["replay_alignment_v1"]
        self.assertEqual(replay["pressure_movement_replay_status"], "replay_supported")
        self.assertNotEqual(replay["pressure_texture_replay_status"], "replay_supported")
        self.assertFalse(record["approval_gate_v1"]["broad_pressure_authority_ready"])
        self.assertEqual(
            record["narrow_authority_proposal_v1"]["pressure_canary_or_relief"]["status"],
            "blocked_until_texture_and_movement_replay_supported",
        )

    def test_active_lease_blocks_until_outcome(self) -> None:
        shared, astrid, minime = self._fixture(active_lease=True)
        record = build_record(
            shared_dir=shared,
            astrid_workspace=astrid,
            minime_workspace=minime,
            since_hours=24,
        )
        self.assertIn(
            "active_lease_requires_self_regulation_outcome",
            record["approval_gate_v1"]["block_reasons"],
        )
        self.assertEqual(
            record["narrow_authority_proposal_v1"]["status"],
            "blocked_active_lease_requires_outcome",
        )

    def test_exploration_noise_request_reports_cap(self) -> None:
        shared, astrid, minime = self._fixture()
        record = build_record(
            shared_dir=shared,
            astrid_workspace=astrid,
            minime_workspace=minime,
            since_hours=24,
        )
        edge = record["narrow_authority_proposal_v1"]["exploration_noise_review"]
        self.assertEqual(edge["requested_value"], 0.12)
        self.assertEqual(edge["current_safe_cap"], 0.08)
        self.assertEqual(edge["proposed_applied_value_without_cap_widening"], 0.08)
        self.assertEqual(edge["cap_widening_status"], "out_of_scope_requires_separate_explicit_approval")


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
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(PressureFocusAuthorityDossierTests)
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
        gate = record["approval_gate_v1"]
        replay = record["replay_alignment_v1"]
        regulator = record["current_regulator_context_v1"]
        proposal = record["narrow_authority_proposal_v1"]
        print("# Pressure/Focus Authority Dossier V1")
        print(f"- Approval gate: {gate['status']}")
        print(f"- Minime focus request: {record['evidence_ledger_v1']['minime_focus_request_present']}")
        print(f"- Astrid relational support: {record['evidence_ledger_v1']['astrid_relational_support_present']}")
        print(
            "- Replay: "
            f"texture={replay['pressure_texture_replay_status']} "
            f"movement={replay['pressure_movement_replay_status']}"
        )
        print(
            "- Current regulator: "
            f"regime={regulator.get('regime')} exploration_noise={regulator.get('exploration_noise')} "
            f"active_lease={regulator.get('active_lease_status')}"
        )
        print(f"- Focus proposal: {proposal['focus_regime_review']['status']}")
        print(
            "- Exploration proposal: "
            f"requested={proposal['exploration_noise_review']['requested_value']} "
            f"cap={proposal['exploration_noise_review']['current_safe_cap']} "
            f"without_cap_widening={proposal['exploration_noise_review']['proposed_applied_value_without_cap_widening']}"
        )
        print(f"- Authority: {AUTHORITY_BOUNDARY}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
