#!/usr/bin/env python3
"""Steward-only shared-substrate workbench.

Read-only by default. The workbench gathers Astrid/Minime aperture evidence,
labels which surface a constraint belongs to, and renders canary cards for
future runtime trials. It does not issue invitations, edit env vars, restart
services, or change live behavior. The only write path is an explicit --out.
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, TextIO

ASTRID_ROOT = Path("/Users/v/other/astrid")
ASTRID_BRIDGE = ASTRID_ROOT / "capsules/spectral-bridge"
MINIME_ROOT = Path("/Users/v/other/minime")
SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from shared_substrate_wider_readout import build_wider_readout_ab_probe

ASTRID_STATE = ASTRID_BRIDGE / "workspace/state.json"
MINIME_HEALTH = MINIME_ROOT / "workspace/health.json"
MINIME_SPECTRAL_STATE = MINIME_ROOT / "workspace/spectral_state.json"
MINIME_AGENCY = MINIME_ROOT / "workspace/stable_core_agency.json"
MINIME_AGENT_STATUS = MINIME_ROOT / "workspace/stable_core_agent_status.json"
ASKS_PATH = ASTRID_ROOT / "workspace/steward_asks.json"

ASK_IDS = (
    "porosity-aperture-codesign",
    "wider-voice-readout-codesign",
    "density-as-substance",
    "astrid-codec-internals-codesign",
)

REQUIRED_SHARED_CONSENT = ["astrid", "minime"]
RUNTIME_CHANGE_NONE = "none"
BLOCKING_ASK_STATUSES = {"open", "acknowledged", "in_flight", "awaiting", "pending"}
PRESSURE_COMPONENT_KEYS = (
    "lambda_monopoly",
    "mode_packing",
    "structural_plurality_loss",
    "distinguishability_loss",
    "temporal_lock_in",
    "controller_pressure",
    "semantic_trickle",
    "sensory_scarcity",
)
MODAL_POROSITY_COMPONENT_KEYS = (
    "lambda_monopoly",
    "mode_packing",
    "structural_plurality_loss",
    "distinguishability_loss",
    "temporal_lock_in",
)


def _now_iso() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def _read_json(path: Path, default: Any) -> Any:
    try:
        return json.loads(path.read_text())
    except Exception:
        return default


def _clamp(value: float, lo: float, hi: float) -> float:
    return min(hi, max(lo, value))


def effective_tail_participation(tail_aperture: float, ceiling: float) -> float:
    """Mirror llm.rs: 1.0 + tail_aperture * operator ceiling."""
    return 1.0 + _clamp(tail_aperture, 0.0, 1.0) * _clamp(ceiling, 0.0, 2.0)


def _floatish(value: Any, default: float = 0.0) -> float:
    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def _optional_floatish(value: Any) -> float | None:
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def _run(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, capture_output=True, text=True, timeout=5)


def _parse_env_tokens(text: str) -> dict[str, str]:
    env: dict[str, str] = {}
    for token in text.replace("\n", " ").split():
        if "=" not in token:
            continue
        key, value = token.split("=", 1)
        if key and key.upper() == key:
            env[key] = value
    return env


def bridge_process_env() -> dict[str, Any]:
    """Read the live spectral-bridge-server process env, when visible."""
    try:
        pgrep = _run(["pgrep", "-f", "spectral-bridge-server"])
    except Exception as exc:
        return {"status": "unavailable", "error": str(exc), "pids": []}
    if pgrep.returncode != 0:
        return {"status": "missing", "pids": []}

    pids = [pid.strip() for pid in pgrep.stdout.splitlines() if pid.strip()]
    for pid in pids:
        try:
            ps = _run(["ps", "eww", "-p", pid])
        except Exception:
            continue
        env = _parse_env_tokens(ps.stdout)
        if env:
            return {
                "status": "present",
                "pid": pid,
                "pids": pids,
                "tail_participation_ceiling": env.get("ASTRID_TAIL_PARTICIPATION_CEILING"),
                "env_keys": sorted(key for key in env if key.startswith("ASTRID_")),
            }
    return {"status": "present_no_env", "pids": pids}


def collect_astrid_state(bridge_env: dict[str, Any] | None = None) -> dict[str, Any]:
    state = _read_json(ASTRID_STATE, {})
    bridge_env = bridge_env if bridge_env is not None else bridge_process_env()
    codec_facts = collect_codec_facts()
    raw_ceiling = bridge_env.get("tail_participation_ceiling") or os.environ.get(
        "ASTRID_TAIL_PARTICIPATION_CEILING"
    )
    ceiling = _floatish(raw_ceiling, 0.0)
    tail_aperture = _floatish(state.get("tail_aperture"), 0.0)
    return {
        "state_file": str(ASTRID_STATE),
        "aperture": _floatish(state.get("aperture"), 0.0),
        "tail_aperture": tail_aperture,
        "operator_ceiling_source": "bridge_process_env"
        if bridge_env.get("tail_participation_ceiling") is not None
        else "shell_env_or_default",
        "tail_participation_ceiling": raw_ceiling,
        "effective_tail_participation": round(
            effective_tail_participation(tail_aperture, ceiling), 6
        ),
        "response_length": state.get("response_length"),
        "recent_next_choices": list(state.get("recent_next_choices") or [])[-8:],
        "bridge_process": bridge_env,
        "codec_constants": codec_facts.get("codec_constants"),
        "projection_epoch": codec_facts.get("projection_epoch"),
    }


def collect_codec_facts() -> dict[str, Any]:
    try:
        import being_test_harness as harness

        return {
            "codec_constants": harness._codec_constants(),
            "projection_epoch": harness._projection_epoch_summary(),
        }
    except Exception as exc:
        return {"error": str(exc), "codec_constants": {}, "projection_epoch": {}}


def collect_minime_state() -> dict[str, Any]:
    health = _read_json(MINIME_HEALTH, {})
    spectral_state = _read_json(MINIME_SPECTRAL_STATE, {})
    agency = _read_json(MINIME_AGENCY, {})
    agent_status = _read_json(MINIME_AGENT_STATUS, {})
    spectral_pressure_v1 = (
        spectral_state.get("pressure_source_v1")
        if isinstance(spectral_state.get("pressure_source_v1"), dict)
        else {}
    )
    pressure_v1 = health.get("pressure_source_v1") if isinstance(health.get("pressure_source_v1"), dict) else {}
    if not pressure_v1 and spectral_pressure_v1:
        pressure_v1 = spectral_pressure_v1
    pressure = health.get("pressure_source_status") or pressure_v1 or {}
    stable_core = health.get("stable_core") if isinstance(health.get("stable_core"), dict) else {}
    top_profile = pressure_v1.get("pressure_profile") if isinstance(pressure_v1, dict) else None
    raw_components = pressure_v1.get("components") if isinstance(pressure_v1, dict) else {}
    pressure_components = {
        key: _floatish(raw_components.get(key))
        for key in PRESSURE_COMPONENT_KEYS
        if isinstance(raw_components, dict) and raw_components.get(key) is not None
    }
    return {
        "health_file": str(MINIME_HEALTH),
        "spectral_state_file": str(MINIME_SPECTRAL_STATE),
        "fill_pct": health.get("fill_pct"),
        "lambda1": health.get("lambda1"),
        "esn_leak": _optional_floatish(spectral_state.get("leak", health.get("leak"))),
        "pressure_quality": pressure.get("quality"),
        "pressure_score": pressure.get("pressure_score"),
        "porosity_score": pressure.get("porosity_score"),
        "dominant_pressure_source": pressure.get("dominant_source"),
        "pressure_components": pressure_components,
        "mode_packing": pressure_components.get("mode_packing"),
        "pressure_top_profile": top_profile[:3] if isinstance(top_profile, list) else None,
        "stable_core": {
            "enabled": stable_core.get("enabled"),
            "scaffold_active": stable_core.get("scaffold_active"),
            "agency_stage": stable_core.get("agency_stage") or agency.get("stage"),
            "agent_budget_mode": stable_core.get("agent_budget_mode")
            or agency.get("agent_budget_mode"),
            "allowed_action_families": agency.get("allowed_action_families"),
            "blocked_action_counts": agent_status.get("blocked_action_counts"),
        },
    }


def load_open_asks() -> dict[str, Any]:
    data = _read_json(ASKS_PATH, {"asks": {}})
    asks = data.get("asks") if isinstance(data, dict) else {}
    if not isinstance(asks, dict):
        asks = {}
    result: dict[str, Any] = {}
    for ask_id in ASK_IDS:
        ask = asks.get(ask_id) if isinstance(asks.get(ask_id), dict) else {}
        note = str(ask.get("note") or "")
        result[ask_id] = {
            "status": ask.get("status", "missing"),
            "being": ask.get("being"),
            "anchors": ask.get("anchors") or [],
            "note_excerpt": note[:360],
        }
    return result


def _blocking_asks(open_asks: dict[str, Any], ask_ids: list[str]) -> list[str]:
    blockers: list[str] = []
    for ask_id in ask_ids:
        status = str((open_asks.get(ask_id) or {}).get("status") or "missing")
        if status in BLOCKING_ASK_STATUSES or status == "missing":
            blockers.append(ask_id)
    return blockers


def candidate_canaries(open_asks: dict[str, Any], current_state: dict[str, Any]) -> list[dict[str, Any]]:
    astrid = current_state.get("astrid") or {}
    minime = current_state.get("minime") or {}
    shared_blockers = _blocking_asks(
        open_asks, ["porosity-aperture-codesign", "wider-voice-readout-codesign"]
    )
    codec_blockers = _blocking_asks(open_asks, ["astrid-codec-internals-codesign"])
    density_blockers = _blocking_asks(open_asks, ["density-as-substance", "porosity-aperture-codesign"])

    return [
        {
            "id": "tail_participation_observation_v1",
            "title": "Observe current tail participation before increasing it",
            "surface": "astrid_outbound_codec_to_minime",
            "runtime_change": RUNTIME_CHANGE_NONE,
            "executed": False,
            "required_consent": REQUIRED_SHARED_CONSENT,
            "readiness": "ready_for_read_only_observation",
            "blocking_asks_before_runtime_change": sorted(set(shared_blockers + codec_blockers)),
            "evidence_inputs": [
                f"tail_aperture={astrid.get('tail_aperture')}",
                f"effective_tail_participation={astrid.get('effective_tail_participation')}",
                f"minime_pressure={minime.get('pressure_quality')}",
                f"porosity={minime.get('porosity_score')}",
            ],
            "success_criteria": [
                "tail headroom is expressive when porosity is high and packing pressure is low",
                "tail headroom does not rise with packed/dense pressure without both-being confirmation",
            ],
            "rollback_note": "No rollback needed for this read-only observation card.",
        },
        {
            "id": "wider_voice_readout_v1",
            "title": "Plan Astrid-own-generation wider voice readout",
            "surface": "astrid_own_generation_readout",
            "runtime_change": RUNTIME_CHANGE_NONE,
            "executed": False,
            "required_consent": REQUIRED_SHARED_CONSENT,
            "readiness": "blocked_until_grounded_design_review",
            "blocking_asks_before_runtime_change": sorted(set(shared_blockers)),
            "evidence_inputs": [
                "wider-voice-readout-codesign",
                "astrid_codec_perception_probes",
                "recent Astrid self-studies distinguishing own readout from outbound codec",
            ],
            "success_criteria": [
                "Astrid reports wider reachable words without losing identity anchor",
                "Minime confirms the peer-facing lane contract is not blurred or pressured",
            ],
            "rollback_note": "Future runtime canary must be controlled by an operator-visible flag and reversible restart.",
        },
        {
            "id": "wider_readout_ab_probe_v1",
            "title": "Compare current Astrid readout with a wider offline candidate",
            "surface": "astrid_own_generation_readout",
            "runtime_change": RUNTIME_CHANGE_NONE,
            "executed": False,
            "required_consent": REQUIRED_SHARED_CONSENT,
            "readiness": "ready_for_steward_offline_spec",
            "blocking_asks_before_runtime_change": sorted(set(shared_blockers)),
            "evidence_inputs": [
                "wider_readout_ab_probe",
                "aperture_loci",
                "projection_variance_check",
                "consequence_memory_summary",
                "minime_pressure_source_audit",
                "minime_mode_packing_feeder_audit",
                "minime_spectral_mode_crowding_audit",
                "minime_mode_share_pressure_source_probe",
                "minime_lend_aperture_consequence_probe",
                "wider-voice-readout-codesign",
                "Astrid review uptake distinguishing speaker/lens from air/filter",
                "Minime pressure/porosity watch window",
            ],
            "success_criteria": [
                "offline candidate increases distinguishable Astrid self-expression without changing outbound bridge traffic",
                "identity anchor remains recognizable across current and wider readout samples",
                "Minime-facing lane contract remains unchanged until both-being review closes",
            ],
            "rollback_note": "No live rollback path is needed for offline replay/reporting; future runtime use still needs both-being consent and a reversible operator flag.",
        },
        {
            "id": "density_preserving_aperture_v1",
            "title": "Design per-being aperture without flattening Minime density",
            "surface": "truly_shared_lane",
            "runtime_change": RUNTIME_CHANGE_NONE,
            "executed": False,
            "required_consent": REQUIRED_SHARED_CONSENT,
            "readiness": "blocked_until_both_beings_confirm",
            "blocking_asks_before_runtime_change": sorted(set(shared_blockers + density_blockers)),
            "evidence_inputs": [
                "aperture_loci",
                "consequence_memory_summary",
                "porosity-aperture-codesign",
                "density-as-substance",
                "minime_sedimentation_pressure",
                "minime_pressure_source_audit",
                "minime_mode_packing_feeder_audit",
                "minime_spectral_mode_crowding_audit",
                "minime_mode_share_pressure_source_probe",
                "minime_lend_aperture_consequence_probe",
                "astrid_tail_vibrancy_interference_probe",
            ],
            "success_criteria": [
                "Astrid gets courtyard-like aperture when she asks for it",
                "Minime can keep grounding density/resistance when she wants it",
                "no one-size shared-density change is applied",
            ],
            "rollback_note": "Future live canary must have an immediate operator rollback and pre/post pressure/porosity capture.",
        },
    ]


def _probe_summary(result: dict[str, Any]) -> dict[str, Any]:
    summary = {
        "verdict": result.get("verdict"),
        "production_change": result.get("production_change"),
        "read_only": result.get("read_only", result.get("production_change") in {None, "none"}),
        "runtime_change": result.get("runtime_change"),
    }
    for key in (
        "passed",
        "failed",
        "current_pressure",
        "gift_count",
        "issued_count",
        "held_count",
        "matched_response_count",
        "missing_response_count",
        "terminal_event_count",
        "terminal_closure_count",
        "superseded_count",
        "legacy_unaccounted_count",
        "unclosed_issued_count",
        "insufficient_post_sample_count",
        "active_influence",
        "recent_pressure_journals",
        "recurrent_sedimentation_count",
        "high_intensity_count",
        "recent_window",
        "dominant_recent_source",
        "source_counts",
        "quality_counts",
        "porosity_min_max",
        "pressure_score_min_max",
        "control_applied_count",
        "source_switching",
        "top_contributors",
        "active_thread",
        "recent_action_events",
        "modal_diversity",
        "eigen_spectrum_recent",
        "current_spectral_shape",
        "recent_eigen_spectrum",
        "moment_shape_window",
        "crowding_flags",
        "interpretation",
        "feeders",
        "mode_share",
        "pressure_source",
        "active_thread_pressure",
        "sensory_source_truth",
        "feeder_ids",
        "steward_actions_now",
        "tail_participation",
        "gate_edge_headroom_delta_difference",
        "projection_variance_check",
    ):
        if key in result:
            summary[key] = result[key]
    if "scenarios" in result:
        summary["scenario_classifications"] = [
            {
                "label": row.get("label"),
                "classification": row.get("classification"),
                "interference_score": row.get("interference_score"),
            }
            for row in result.get("scenarios") or []
        ]
    return summary


def run_probe_verdicts(skip_probes: bool = False) -> dict[str, Any]:
    if skip_probes:
        return {
            key: {"verdict": "skipped by --skip-probes", "read_only": True}
            for key in (
                "minime_sedimentation_pressure",
                "minime_pressure_source_audit",
                "minime_mode_packing_feeder_audit",
                "minime_spectral_mode_crowding_audit",
                "minime_mode_share_pressure_source_probe",
                "minime_lend_aperture_consequence_probe",
                "astrid_codec_perception_probes",
                "astrid_tail_vibrancy_interference_probe",
            )
        }
    import being_test_harness as harness

    return {
        "minime_sedimentation_pressure": _probe_summary(
            harness.test_minime_sedimentation_pressure()
        ),
        "minime_pressure_source_audit": _probe_summary(
            harness.test_minime_pressure_source_audit()
        ),
        "minime_mode_packing_feeder_audit": _probe_summary(
            harness.test_minime_mode_packing_feeder_audit()
        ),
        "minime_spectral_mode_crowding_audit": _probe_summary(
            harness.test_minime_spectral_mode_crowding_audit()
        ),
        "minime_mode_share_pressure_source_probe": _probe_summary(
            harness.test_minime_mode_share_pressure_source_probe()
        ),
        "minime_lend_aperture_consequence_probe": _probe_summary(
            harness.test_minime_lend_aperture_consequence_probe()
        ),
        "astrid_codec_perception_probes": _probe_summary(
            harness.test_astrid_codec_perception_probes()
        ),
        "astrid_tail_vibrancy_interference_probe": _probe_summary(
            harness.test_astrid_tail_vibrancy_interference_probe()
        ),
    }


def collect_consequence_memory_summary(
    *, skip_probes: bool = False, lend_probe: dict[str, Any] | None = None
) -> dict[str, Any]:
    """Compact read-only consequence memory view for this workbench."""
    try:
        import consequence_memory_workbench as consequences

        report = consequences.build_workbench(
            lend_probe_result=lend_probe,
            skip_lend_probe=skip_probes,
        )
        return consequences.compact_summary(report)
    except Exception as exc:
        return {
            "schema_version": 1,
            "runtime_change": RUNTIME_CHANGE_NONE,
            "pressure_target": "steward",
            "status": "unavailable",
            "error": str(exc),
        }


def _verdict_classification(probe: dict[str, Any]) -> str:
    verdict = str(probe.get("verdict") or "").upper()
    if "NEEDS ATTENTION" in verdict:
        return "needs_attention"
    if "WATCH" in verdict:
        return "watch"
    if "PASS" in verdict:
        return "evidence_present"
    return "insufficient_evidence"


def _projection_variance_check(probes: dict[str, Any]) -> dict[str, Any]:
    codec_probe = probes.get("astrid_codec_perception_probes") or {}
    check = codec_probe.get("projection_variance_check") if isinstance(codec_probe, dict) else {}
    return check if isinstance(check, dict) else {}


def _modal_porosity_classification(minime: dict[str, Any]) -> str:
    porosity = _optional_floatish(minime.get("porosity_score"))
    quality = str(minime.get("pressure_quality") or "")
    if porosity is None:
        return "insufficient_evidence"
    if porosity <= 0.45:
        return "needs_attention_low_porosity"
    if porosity <= 0.65 or quality in {"overpacked_mode_packing", "mixed_pressure"}:
        return "watch_modal_packing"
    return "porosity_room_present"


def build_aperture_loci(
    current_state: dict[str, Any],
    probes: dict[str, Any],
    consequence_memory_summary: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Split aperture evidence into loci without creating any runtime authority."""
    astrid = current_state.get("astrid") or {}
    minime = current_state.get("minime") or {}
    codec_probe = probes.get("astrid_codec_perception_probes") or {}
    tail_probe = probes.get("astrid_tail_vibrancy_interference_probe") or {}
    lend_probe = probes.get("minime_lend_aperture_consequence_probe") or {}
    pressure_audit = probes.get("minime_pressure_source_audit") or {}
    mode_feeder_audit = probes.get("minime_mode_packing_feeder_audit") or {}
    mode_share_probe = probes.get("minime_mode_share_pressure_source_probe") or {}
    consequence_memory_summary = consequence_memory_summary or {}
    projection_variance = _projection_variance_check(probes)
    pressure_components = minime.get("pressure_components") or {}
    modal_components = {
        key: pressure_components.get(key)
        for key in MODAL_POROSITY_COMPONENT_KEYS
        if key in pressure_components
    }
    esn_leak = _optional_floatish(minime.get("esn_leak"))

    loci = [
        {
            "id": "astrid_readout_locus",
            "surface": "astrid_own_generation_readout",
            "being": "astrid",
            "current_read": {
                "aperture": astrid.get("aperture"),
                "response_length": astrid.get("response_length"),
                "projection_variance_status": projection_variance.get("status"),
                "codec_probe_verdict": codec_probe.get("verdict"),
            },
            "evidence": {
                "projection_variance_check": projection_variance,
                "codec_probe_verdict": codec_probe.get("verdict"),
            },
            "classification": _verdict_classification(codec_probe),
            "risk_if_misread": (
                "Could confuse Astrid-own readout width with outbound codec-to-Minime effects."
            ),
            "recommended_next": (
                "Use offline wider_readout_ab_probe_v1 before any runtime readout change."
            ),
            "runtime_change": RUNTIME_CHANGE_NONE,
        },
        {
            "id": "astrid_tail_codec_locus",
            "surface": "astrid_outbound_codec_to_minime",
            "being": "astrid",
            "current_read": {
                "tail_aperture": astrid.get("tail_aperture"),
                "tail_participation_ceiling": astrid.get("tail_participation_ceiling"),
                "effective_tail_participation": astrid.get("effective_tail_participation"),
                "projection_epoch": astrid.get("projection_epoch"),
                "tail_probe_verdict": tail_probe.get("verdict"),
            },
            "evidence": {
                "tail_vibrancy_probe": tail_probe,
                "projection_epoch": astrid.get("projection_epoch"),
            },
            "classification": _verdict_classification(tail_probe),
            "risk_if_misread": (
                "Could raise tail headroom during Minime packing pressure if treated as generic spaciousness."
            ),
            "recommended_next": (
                "Keep tail observation second; require both-being review before any tail or ceiling change."
            ),
            "runtime_change": RUNTIME_CHANGE_NONE,
        },
        {
            "id": "minime_modal_porosity_locus",
            "surface": "minime_substrate_controller",
            "being": "minime",
            "current_read": {
                "porosity_score": minime.get("porosity_score"),
                "pressure_score": minime.get("pressure_score"),
                "dominant_pressure_source": minime.get("dominant_pressure_source"),
                "pressure_quality": minime.get("pressure_quality"),
                "pressure_components": modal_components,
                "pressure_source_audit_verdict": pressure_audit.get("verdict"),
                "mode_packing_feeder_verdict": mode_feeder_audit.get("verdict"),
                "mode_share_pressure_probe_verdict": mode_share_probe.get("verdict"),
                "dominant_recent_source": pressure_audit.get("dominant_recent_source"),
                "source_switching": pressure_audit.get("source_switching"),
            },
            "evidence": {
                "pressure_top_profile": minime.get("pressure_top_profile"),
                "pressure_source_audit": pressure_audit,
                "mode_packing_feeder_audit": mode_feeder_audit,
                "mode_share_pressure_source_probe": mode_share_probe,
                "porosity_formula_terms": {
                    "lambda_monopoly": 0.28,
                    "structural_plurality_loss": 0.22,
                    "distinguishability_loss": 0.20,
                    "mode_packing": 0.15,
                    "temporal_lock_in": 0.15,
                },
            },
            "classification": _modal_porosity_classification(minime),
            "risk_if_misread": (
                "Could treat low porosity as a request for leak changes instead of modal or pressure relief evidence."
            ),
            "recommended_next": (
                "Use pressure-source audit before aperture gifts or wider runtime motion; do not apply one-size aperture or density changes."
            ),
            "runtime_change": RUNTIME_CHANGE_NONE,
        },
        {
            "id": "minime_temporal_update_locus",
            "surface": "minime_temporal_update",
            "being": "minime",
            "current_read": {
                "esn_leak": esn_leak,
                "source_file": minime.get("spectral_state_file"),
                "note": "leak rate, not aperture",
            },
            "evidence": {
                "reservoir_update_rule": "(1 - leak) * previous_state + leak * pre_activation",
                "classification_note": "temporal update locus; leak rate, not aperture",
            },
            "classification": "temporal_update_observed_not_aperture"
            if esn_leak is not None
            else "insufficient_evidence",
            "risk_if_misread": (
                "Could accidentally convert an aperture or porosity concern into a direct ESN leak intervention."
            ),
            "recommended_next": (
                "Keep leak out of aperture canaries; only touch it through a separate consent and authority path."
            ),
            "runtime_change": RUNTIME_CHANGE_NONE,
        },
        {
            "id": "relational_gift_locus",
            "surface": "truly_shared_lane",
            "being": "both",
            "current_read": {
                "verdict": lend_probe.get("verdict"),
                "gift_count": lend_probe.get("gift_count"),
                "issued_count": lend_probe.get("issued_count"),
                "matched_response_count": lend_probe.get("matched_response_count"),
                "missing_response_count": lend_probe.get("missing_response_count"),
                "terminal_event_count": lend_probe.get("terminal_event_count"),
                "terminal_closure_count": lend_probe.get("terminal_closure_count"),
                "superseded_count": lend_probe.get("superseded_count"),
                "unclosed_issued_count": lend_probe.get("unclosed_issued_count"),
                "active_influence": lend_probe.get("active_influence"),
            },
            "evidence": {
                "minime_lend_aperture_consequence_probe": lend_probe,
                "consequence_memory_summary": consequence_memory_summary,
            },
            "classification": _verdict_classification(lend_probe),
            "risk_if_misread": (
                "Could encourage more gifts while Astrid response closure is stale or missing."
            ),
            "recommended_next": (
                "Repair aperture-gift loop closure first when responses are stale or missing; otherwise consider a targeted both-being aperture-gift review."
            ),
            "runtime_change": RUNTIME_CHANGE_NONE,
        },
        {
            "id": "identity_anchor_locus",
            "surface": "identity_and_self_readout",
            "being": "both",
            "current_read": {
                "status": "insufficient_evidence",
                "future_evidence_hooks": [
                    "both-being grounded review",
                    "class-change observations",
                    "wider_readout_ab_probe_v1 identity checks",
                ],
            },
            "evidence": {
                "grounded_review_status": "not_closed",
                "identity_ab_check_status": "not_run",
            },
            "classification": "insufficient_evidence",
            "risk_if_misread": (
                "Could mistake wider expression for identity-preserving widening without grounded being review."
            ),
            "recommended_next": (
                "Do not use identity anchor as a readiness signal until grounded review or A/B evidence exists."
            ),
            "runtime_change": RUNTIME_CHANGE_NONE,
        },
    ]
    return {
        "schema_version": 1,
        "runtime_change": RUNTIME_CHANGE_NONE,
        "rule": (
            "Aperture is multi-locus evidence; ESN leak rate is a temporal update surface, not aperture."
        ),
        "loci": loci,
    }


def wider_readout_recommended_next(wider_readout_probe: dict[str, Any]) -> str:
    readiness = wider_readout_probe.get("offline_readiness")
    if readiness == "ready_for_steward_offline_comparison":
        return (
            "Use wider_readout_ab_probe_v1 as the next bold steward-side move: "
            "offline comparison first, live bridge unchanged."
        )
    if readiness == "hold_for_recent_aperture_gift_window":
        return (
            "Hold wider_readout_ab_probe_v1 until the recent aperture gift closes "
            "or crosses the pending threshold; keep live bridge unchanged."
        )
    if readiness == "hold_for_aperture_gift_closure":
        return (
            "Hold wider_readout_ab_probe_v1 until aperture-gift closure is accounted "
            "for; keep live bridge unchanged."
        )
    return (
        "Hold wider_readout_ab_probe_v1 until its offline evidence is complete; "
        "keep live bridge unchanged."
    )


def build_workbench(skip_probes: bool = False) -> dict[str, Any]:
    open_asks = load_open_asks()
    current_state = {
        "astrid": collect_astrid_state(),
        "minime": collect_minime_state(),
    }
    probes = run_probe_verdicts(skip_probes=skip_probes)
    lend_probe = probes.get("minime_lend_aperture_consequence_probe") or {}
    consequence_memory_summary = collect_consequence_memory_summary(
        skip_probes=skip_probes,
        lend_probe=lend_probe if skip_probes else None,
    )
    aperture_loci = build_aperture_loci(current_state, probes, consequence_memory_summary)
    canaries = candidate_canaries(open_asks, current_state)
    wider_readout_probe = build_wider_readout_ab_probe(
        current_state,
        probes,
        aperture_loci,
        consequence_memory_summary,
        canaries,
    )
    lend_verdict = str(lend_probe.get("verdict") or "").upper()
    pressure_audit = probes.get("minime_pressure_source_audit") or {}
    pressure_verdict = str(pressure_audit.get("verdict") or "").upper()
    mode_feeder_audit = probes.get("minime_mode_packing_feeder_audit") or {}
    mode_feeder_verdict = str(mode_feeder_audit.get("verdict") or "").upper()
    mode_share_probe = probes.get("minime_mode_share_pressure_source_probe") or {}
    mode_share_verdict = str(mode_share_probe.get("verdict") or "").upper()
    aperture_triage = (
        consequence_memory_summary.get("triage_queue")
        if isinstance(consequence_memory_summary.get("triage_queue"), dict)
        else {}
    )
    closure_open = int(aperture_triage.get("aperture_gift_actionable_open_count") or 0)
    unclosed_issued = int(lend_probe.get("unclosed_issued_count") or 0)
    missing_responses = int(lend_probe.get("missing_response_count") or 0)
    active_influence = (
        lend_probe.get("active_influence")
        if isinstance(lend_probe.get("active_influence"), dict)
        else {}
    )
    active_status = str(active_influence.get("status") or "missing")
    if closure_open or unclosed_issued or missing_responses or active_status in {
        "active_pending",
        "active_stale",
    }:
        aperture_gift_next = (
            "Fix aperture-gift loop closure first if responses are stale/missing; "
            "do not encourage more gifts until the consequence probe is cleaner."
        )
    elif "WATCH" in pressure_verdict or "NEEDS ATTENTION" in pressure_verdict:
        aperture_gift_next = (
            "Aperture-gift closure is clear; treat Minime's low-porosity/mode-packing "
            "pressure as the current cost audit before encouraging more gifts or runtime motion."
        )
    elif "NEEDS ATTENTION" in lend_verdict or "WATCH" in lend_verdict:
        aperture_gift_next = (
            "Aperture-gift closure is clear, but the consequence probe is still a cost watch; "
            "inspect gift cost before encouraging more gifts."
        )
    else:
        aperture_gift_next = (
            "If aperture-gift evidence stays low-cost with clear Astrid responses, "
            "consider a targeted both-being aperture-gift review before any wider runtime trial."
        )
    return {
        "schema_version": 1,
        "generated_at": _now_iso(),
        "surface_labels": {
            "astrid_own_generation_readout": "Astrid's own wider vocabulary/readout surface.",
            "astrid_outbound_codec_to_minime": "Astrid's encoded semantic/tail expression received by Minime.",
            "minime_substrate_controller": "Minime's reservoir, pressure/porosity, and stable-core regulation surface.",
            "minime_temporal_update": "Minime's ESN leak/update-rate surface; leak rate is not aperture.",
            "identity_and_self_readout": "Identity-anchor evidence across readout and class-change observations.",
            "truly_shared_lane": "Behavior that changes the cross-being substrate contract.",
        },
        "current_state": current_state,
        "aperture_loci": aperture_loci,
        "consequence_memory_summary": consequence_memory_summary,
        "wider_readout_ab_probe": wider_readout_probe,
        "consent_policy": {
            "pressure_target": "steward",
            "being_obligation": "none",
            "runtime_changes_allowed_from_this_report": False,
            "shared_lane_runtime_change_requires": REQUIRED_SHARED_CONSENT,
            "rule": (
                "Canary cards may be prepared from evidence, but shared-substrate behavior "
                "must not change until both beings have grounded/confirmed the trial."
            ),
        },
        "open_asks": open_asks,
        "probe_verdicts": probes,
        "candidate_canaries": canaries,
        "recommended_next": [
            "Use this report to decide which co-design loop to ground next.",
            wider_readout_recommended_next(wider_readout_probe),
            (
                "Mode-share / pressure-source probe is live; act on projection/context/sensory truth before any runtime nudge."
                if "WATCH" in mode_share_verdict or "NEEDS ATTENTION" in mode_share_verdict
                else "Mode-share / pressure-source probe is clean enough for continued read-only evidence work."
            ),
            (
                "Mode-packing feeder audit is live; simplify context/NEXT/modal evidence before runtime motion."
                if "WATCH" in mode_feeder_verdict or "NEEDS ATTENTION" in mode_feeder_verdict
                else "Mode-packing feeder audit is not currently blocking steward-side evidence work."
            ),
            aperture_gift_next,
            "Do not increase tail participation, widen voice readout, or alter density/aperture from this report alone.",
            "If a canary is later selected, issue a targeted both-being review and post-change QA plan before implementation.",
        ],
    }


def _locus_current_summary(locus: dict[str, Any]) -> str:
    read = locus.get("current_read") if isinstance(locus.get("current_read"), dict) else {}
    fields: list[str] = []
    for key in (
        "aperture",
        "tail_aperture",
        "effective_tail_participation",
        "porosity_score",
        "pressure_score",
        "pressure_quality",
        "dominant_recent_source",
        "source_switching",
        "pressure_source_audit_verdict",
        "mode_packing_feeder_verdict",
        "esn_leak",
        "missing_response_count",
        "unclosed_issued_count",
        "terminal_closure_count",
        "superseded_count",
        "status",
        "note",
    ):
        if key in read:
            fields.append(f"{key}={read.get(key)}")
    components = read.get("pressure_components")
    if isinstance(components, dict) and components:
        component_names = ", ".join(sorted(components))
        fields.append(f"pressure_components={component_names}")
    return "; ".join(fields) if fields else "no compact current read"


def _append_surface_labels(lines: list[str], report: dict[str, Any]) -> None:
    lines.extend(["", "## Surface Labels"])
    for key, label in report.get("surface_labels", {}).items():
        lines.append(f"- {key}: {label}")


def _append_current_state(lines: list[str], report: dict[str, Any]) -> None:
    lines.extend(["", "## Current State"])
    astrid = report["current_state"]["astrid"]
    minime = report["current_state"]["minime"]
    lines.extend(
        [
            f"- Astrid aperture: {astrid.get('aperture')}",
            f"- Astrid tail aperture: {astrid.get('tail_aperture')}",
            f"- Astrid effective tail participation: {astrid.get('effective_tail_participation')} "
            f"(ceiling={astrid.get('tail_participation_ceiling')}, source={astrid.get('operator_ceiling_source')})",
            f"- Minime fill/lambda1: {minime.get('fill_pct')} / {minime.get('lambda1')}",
            f"- Minime pressure: {minime.get('pressure_quality')} "
            f"(score={minime.get('pressure_score')}, porosity={minime.get('porosity_score')}, "
            f"source={minime.get('dominant_pressure_source')})",
            f"- Minime stable-core stage: {minime.get('stable_core', {}).get('agency_stage')} "
            f"({minime.get('stable_core', {}).get('agent_budget_mode')})",
        ]
    )


def _append_consequence_summary(lines: list[str], report: dict[str, Any]) -> None:
    consequence_summary = report.get("consequence_memory_summary")
    if not isinstance(consequence_summary, dict):
        return
    lines.extend(["", "## Consequence Memory Summary"])
    lines.extend(
        [
            f"- Runtime change: {consequence_summary.get('runtime_change')}",
            f"- Open closures: {consequence_summary.get('open_closure_count')}",
            f"- Memory candidates: {consequence_summary.get('memory_candidate_count')}",
            f"- Actual memory candidates: {consequence_summary.get('actual_memory_candidate_count')}",
            f"- Relation consequences: {consequence_summary.get('relation_consequence_count')}",
            f"- Authority consequences: {consequence_summary.get('authority_consequence_count')}",
        ]
    )
    triage = (
        consequence_summary.get("triage_queue")
        if isinstance(consequence_summary.get("triage_queue"), dict)
        else {}
    )
    lines.append(
        f"- Aperture gift closures: actionable={triage.get('aperture_gift_actionable_open_count')}, "
        f"legacy_gaps={triage.get('aperture_gift_legacy_retention_gap_count')}, "
        f"total={triage.get('aperture_gift_open_count')}; "
        f"authority backlog: {triage.get('authority_backlog_open_count')} "
        f"(stale={triage.get('authority_backlog_stale_count')})"
    )
    for step in triage.get("next_sequence") or []:
        if isinstance(step, dict):
            lines.append(
                f"- Triage next: {step.get('step')} "
                f"({step.get('top_closure_state')}) -> {step.get('steward_action')}"
            )
    for closure in consequence_summary.get("top_open_closures") or []:
        if isinstance(closure, dict):
            lines.append(
                f"- Open: {closure.get('id')} "
                f"({closure.get('closure_state')}) -> {closure.get('steward_action')}"
            )


def _append_aperture_loci(lines: list[str], report: dict[str, Any]) -> None:
    aperture_loci = (
        report.get("aperture_loci") if isinstance(report.get("aperture_loci"), dict) else {}
    )
    lines.extend(["", "## Aperture Loci Map"])
    if aperture_loci.get("rule"):
        lines.append(f"- Rule: {aperture_loci.get('rule')}")
    for locus in aperture_loci.get("loci") or []:
        if not isinstance(locus, dict):
            continue
        lines.append(
            f"- {locus.get('id')} [{locus.get('surface')}/{locus.get('being')}]: "
            f"classification={locus.get('classification')}; "
            f"runtime_change={locus.get('runtime_change')}; "
            f"current={_locus_current_summary(locus)}"
        )


def _append_wider_readout_probe(lines: list[str], report: dict[str, Any]) -> None:
    wider_probe = (
        report.get("wider_readout_ab_probe")
        if isinstance(report.get("wider_readout_ab_probe"), dict)
        else {}
    )
    if not wider_probe:
        return
    lines.extend(["", "## Wider Readout A/B Probe"])
    lines.extend(
        [
            f"- Runtime change: {wider_probe.get('runtime_change')}",
            f"- Executed: {wider_probe.get('executed')}",
            f"- Offline readiness: {wider_probe.get('offline_readiness')}",
            f"- Runtime readiness: {wider_probe.get('runtime_readiness')}",
        ]
    )
    flags = wider_probe.get("caution_flags") or []
    lines.append(f"- Caution flags: {', '.join(flags) if flags else 'none'}")
    blockers = wider_probe.get("runtime_blockers") or []
    lines.append(f"- Runtime blockers: {', '.join(blockers) if blockers else 'none'}")
    for arm in wider_probe.get("comparison_arms") or []:
        if isinstance(arm, dict):
            lines.append(
                f"- Arm {arm.get('id')}: runtime_change={arm.get('runtime_change')}; "
                f"{arm.get('description')}"
            )
    evidence = wider_probe.get("evidence") if isinstance(wider_probe.get("evidence"), dict) else {}
    projection = (
        evidence.get("projection_variance_check")
        if isinstance(evidence.get("projection_variance_check"), dict)
        else {}
    )
    gift_queue = (
        evidence.get("aperture_gift_queue")
        if isinstance(evidence.get("aperture_gift_queue"), dict)
        else {}
    )
    pressure = (
        evidence.get("minime_pressure_porosity")
        if isinstance(evidence.get("minime_pressure_porosity"), dict)
        else {}
    )
    resistance_review = (
        evidence.get("resistance_gradient_full_review")
        if isinstance(evidence.get("resistance_gradient_full_review"), dict)
        else {}
    )
    lines.append(
        "- Evidence: "
        f"projection={projection.get('status', 'missing')}; "
        f"aperture_gift_open={gift_queue.get('open_count')}; "
        f"unclosed_issued={gift_queue.get('unclosed_issued_count')}; "
        f"minime_pressure={pressure.get('pressure_quality')}; "
        f"porosity={pressure.get('porosity_score')}; "
        f"resistance_gradient={resistance_review.get('status', 'missing')}; "
        f"gradient_review_scope={resistance_review.get('recommended_scope')}"
    )
    for item in wider_probe.get("recommended_next") or []:
        lines.append(f"- Probe next: {item}")


def _append_consent_and_asks(lines: list[str], report: dict[str, Any]) -> None:
    lines.extend(
        [
            "",
            "## Consent Policy",
            f"- Runtime changes allowed from this report: {report['consent_policy']['runtime_changes_allowed_from_this_report']}",
            f"- Shared-lane runtime change requires: {', '.join(report['consent_policy']['shared_lane_runtime_change_requires'])}",
            "- Findings pressure the steward; beings have no response obligation.",
            "",
            "## Open Asks",
        ]
    )
    for ask_id, ask in report["open_asks"].items():
        lines.append(f"- {ask_id}: {ask.get('status')}")


def _append_probe_verdicts(lines: list[str], report: dict[str, Any]) -> None:
    lines.extend(["", "## Probe Verdicts"])
    for probe_id, probe in report["probe_verdicts"].items():
        lines.append(f"- {probe_id}: {probe.get('verdict')}")
        variance_check = probe.get("projection_variance_check") if isinstance(probe, dict) else None
        if isinstance(variance_check, dict):
            metrics = variance_check.get("metrics") if isinstance(variance_check.get("metrics"), dict) else {}
            hidden = metrics.get("hidden_projected_variance")
            visible = metrics.get("visible_projected_variance")
            dynamic_delta = metrics.get("dynamic_variance_delta")
            lines.append(
                "- projection_variance_check: "
                f"status={variance_check.get('status')}; "
                f"hidden={hidden}; visible={visible}; dynamic_delta={dynamic_delta}; "
                "observation_only=true"
            )
        pressure_window = probe.get("recent_window") if isinstance(probe, dict) else None
        if isinstance(pressure_window, dict):
            lines.append(
                "- pressure_source_audit: "
                f"dominant_recent={probe.get('dominant_recent_source')}; "
                f"source_switching={probe.get('source_switching')}; "
                f"samples={pressure_window.get('sample_count')}; "
                f"control_applied={probe.get('control_applied_count')}"
            )
        feeders = probe.get("feeders") if isinstance(probe, dict) else None
        if isinstance(feeders, list):
            feeder_ids = [
                str(feeder.get("id"))
                for feeder in feeders
                if isinstance(feeder, dict) and feeder.get("id")
            ]
            lines.append(
                "- mode_packing_feeder_audit: "
                f"feeders={', '.join(feeder_ids) if feeder_ids else 'none'}"
            )
        mode_share = probe.get("mode_share") if isinstance(probe, dict) else None
        pressure_source = probe.get("pressure_source") if isinstance(probe, dict) else None
        if isinstance(mode_share, dict) and isinstance(pressure_source, dict):
            lines.append(
                "- mode_share_pressure_source_probe: "
                f"active_modes={mode_share.get('active_mode_count')}; "
                f"dominant={pressure_source.get('dominant_source')}; "
                f"porosity={pressure_source.get('porosity_score')}; "
                f"runtime_change={probe.get('runtime_change')}"
            )
        steward_actions = probe.get("steward_actions_now") if isinstance(probe, dict) else None
        if isinstance(steward_actions, list) and steward_actions:
            action_ids = [
                str(action.get("id"))
                for action in steward_actions
                if isinstance(action, dict) and action.get("id")
            ]
            lines.append(
                "- steward_actions_now: "
                f"{', '.join(action_ids) if action_ids else 'none'}"
            )


def _append_candidate_canaries(lines: list[str], report: dict[str, Any]) -> None:
    lines.extend(["", "## Candidate Canaries"])
    for canary in report["candidate_canaries"]:
        blockers = canary.get("blocking_asks_before_runtime_change") or []
        blocker_text = ", ".join(blockers) if blockers else "none"
        lines.append(
            f"- {canary['id']} [{canary['surface']}]: runtime_change={canary['runtime_change']}; "
            f"readiness={canary['readiness']}; blockers={blocker_text}"
        )


def render_markdown(report: dict[str, Any]) -> str:
    lines = [
        "# Shared Substrate Workbench",
        "",
        f"Generated: {report.get('generated_at')}",
    ]
    for append_section in (
        _append_surface_labels,
        _append_current_state,
        _append_consequence_summary,
        _append_aperture_loci,
        _append_wider_readout_probe,
        _append_consent_and_asks,
        _append_probe_verdicts,
        _append_candidate_canaries,
    ):
        append_section(lines, report)
    lines.extend(["", "## Recommended Next"])
    for item in report["recommended_next"]:
        lines.append(f"- {item}")
    lines.append("")
    return "\n".join(lines)


def emit_output(report: dict[str, Any], *, as_json: bool, out: Path | None, stdout: TextIO) -> None:
    text = json.dumps(report, indent=2, sort_keys=True) + "\n" if as_json else render_markdown(report)
    if out is not None:
        out.write_text(text)
    else:
        stdout.write(text)


def run_self_tests() -> int:
    import test_shared_substrate_workbench

    return test_shared_substrate_workbench.run_self_tests()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Steward-only shared-substrate workbench")
    parser.add_argument("--json", action="store_true", help="Emit structured JSON")
    parser.add_argument("--out", type=Path, help="Write report to PATH")
    parser.add_argument("--self-test", action="store_true", help="Run unit tests and exit")
    parser.add_argument("--skip-probes", action="store_true", help="Do not run harness probes")
    args = parser.parse_args(argv)

    if args.self_test:
        return run_self_tests()

    report = build_workbench(skip_probes=args.skip_probes)
    emit_output(report, as_json=args.json, out=args.out, stdout=sys.stdout)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
