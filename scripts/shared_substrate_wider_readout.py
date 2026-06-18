#!/usr/bin/env python3
"""Offline wider-readout evidence helpers for the Shared Substrate Workbench."""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any

RUNTIME_CHANGE_NONE = "none"
ASTRID_WORKSPACE = Path(__file__).resolve().parents[1] / "capsules/spectral-bridge/workspace"
RESISTANCE_GRADIENT_REVIEW_DIR = (
    ASTRID_WORKSPACE / "diagnostics/resistance_gradient_full_reviews"
)


def _optional_floatish(value: Any) -> float | None:
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def _projection_variance_check(probes: dict[str, Any]) -> dict[str, Any]:
    codec_probe = probes.get("astrid_codec_perception_probes") or {}
    check = codec_probe.get("projection_variance_check") if isinstance(codec_probe, dict) else {}
    return check if isinstance(check, dict) else {}


def _first_locus(aperture_loci: dict[str, Any], locus_id: str) -> dict[str, Any]:
    for locus in aperture_loci.get("loci") or []:
        if isinstance(locus, dict) and locus.get("id") == locus_id:
            return locus
    return {}


def _first_canary(canaries: list[dict[str, Any]], canary_id: str) -> dict[str, Any]:
    for canary in canaries:
        if canary.get("id") == canary_id:
            return canary
    return {}


def _consequence_triage(summary: dict[str, Any]) -> dict[str, Any]:
    triage = summary.get("triage_queue")
    return triage if isinstance(triage, dict) else {}


def _as_dict(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _latest_review_json_path(root: Path = RESISTANCE_GRADIENT_REVIEW_DIR) -> Path | None:
    if not root.exists():
        return None
    paths = [path for path in root.glob("*/review.json") if path.is_file()]
    if not paths:
        return None
    return max(paths, key=lambda path: path.stat().st_mtime)


def collect_latest_resistance_gradient_review(
    root: Path = RESISTANCE_GRADIENT_REVIEW_DIR,
) -> dict[str, Any]:
    """Return a compact, read-only summary of the latest full gradient review."""
    path = _latest_review_json_path(root)
    if path is None:
        return {
            "status": "missing",
            "runtime_change": RUNTIME_CHANGE_NONE,
            "include_in_both_being_review": False,
        }
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {
            "status": "unreadable",
            "source_path": str(path),
            "runtime_change": RUNTIME_CHANGE_NONE,
            "include_in_both_being_review": False,
        }
    summary = _as_dict(payload.get("summary"))
    inclusion = _as_dict(payload.get("wider_readout_inclusion"))
    return {
        "status": "present",
        "source_path": str(path),
        "generated_at": payload.get("generated_at"),
        "policy": payload.get("policy"),
        "artifact_count": summary.get("artifact_count"),
        "review_shape_counts": summary.get("suggested_being_review_shape_counts") or {},
        "orientation_counts": summary.get("orientation_counts") or {},
        "top_axis_counts": summary.get("top_axis_counts") or {},
        "recent_language_support_counts": summary.get("recent_language_support_counts")
        or {},
        "include_in_both_being_review": bool(
            inclusion.get("include_in_both_being_review")
        ),
        "recommended_scope": inclusion.get("recommended_scope"),
        "review_questions": inclusion.get("review_questions") or [],
        "guardrails": inclusion.get("guardrails") or [],
        "recommended_next": payload.get("recommended_next") or [],
        "runtime_change": payload.get("runtime_change") or RUNTIME_CHANGE_NONE,
        "pressure_target": payload.get("pressure_target"),
        "being_obligation": payload.get("being_obligation"),
    }


def _active_influence(lend_probe: dict[str, Any]) -> dict[str, Any]:
    return _as_dict(lend_probe.get("active_influence"))


def _active_influence_status(lend_probe: dict[str, Any]) -> str:
    return str(_active_influence(lend_probe).get("status") or "missing")


def _runtime_blockers(canary: dict[str, Any]) -> list[str]:
    canary_blockers = list(canary.get("blocking_asks_before_runtime_change") or [])
    return sorted(
        set(
            canary_blockers
            + [
                "both-being grounded design review not closed",
                "operator-visible reversible flag not specified for any later runtime use",
            ]
        )
    )


def _caution_flags(
    minime: dict[str, Any],
    lend_probe: dict[str, Any],
    aperture_actionable_open: int,
    unclosed_issued: int,
) -> list[str]:
    lend_verdict = str(lend_probe.get("verdict") or "").upper()
    active_status = _active_influence_status(lend_probe)
    flags: list[str] = []
    if aperture_actionable_open or unclosed_issued:
        flags.append("aperture_gift_closure_open")
    if active_status == "active_recent":
        flags.append("aperture_gift_active_recent")
    if "WATCH" in lend_verdict or "NEEDS ATTENTION" in lend_verdict:
        flags.append("aperture_gift_cost_or_closure_watch")
    if str(minime.get("pressure_quality") or "") in {
        "overpacked_mode_packing",
        "mixed_pressure",
        "semantic_trickle_pressure",
    }:
        flags.append("minime_pressure_window_not_clean")
    porosity = _optional_floatish(minime.get("porosity_score"))
    if porosity is not None and porosity < 0.65:
        flags.append("minime_porosity_below_comfort_window")
    return sorted(set(flags))


def _offline_readiness(
    projection_check: dict[str, Any],
    aperture_actionable_open: int,
    unclosed_issued: int,
    active_influence_status: str,
) -> str:
    status = str(projection_check.get("status") or "missing")
    projection_present = bool(projection_check) and status not in {"missing", "unavailable"}
    if (
        aperture_actionable_open
        or unclosed_issued
        or active_influence_status in {"active_pending", "active_stale"}
    ):
        return "hold_for_aperture_gift_closure"
    if active_influence_status == "active_recent":
        return "hold_for_recent_aperture_gift_window"
    if not projection_present:
        return "needs_projection_variance_evidence"
    return "ready_for_steward_offline_comparison"


def _steward_next(offline_readiness: str) -> str:
    if offline_readiness == "hold_for_aperture_gift_closure":
        return (
            "Close or account for the aperture-gift closure before spending attention "
            "on wider-readout comparison."
        )
    if offline_readiness == "hold_for_recent_aperture_gift_window":
        return (
            "Let the recent aperture gift either close or cross the pending threshold "
            "before writing a wider-readout packet."
        )
    if offline_readiness == "needs_projection_variance_evidence":
        return "Re-run codec/perception probes until projection variance evidence is present."
    return (
        "Assemble an offline A/B review packet comparing current Astrid readout "
        "with a candidate wider readout, then decide whether to ask for a grounded "
        "design review."
    )


def _comparison_arms() -> list[dict[str, Any]]:
    return [
        {
            "id": "control_current_readout",
            "description": (
                "Current Astrid own-generation/readout evidence from existing "
                "state, journals, projection metrics, and codec probes."
            ),
            "runtime_change": RUNTIME_CHANGE_NONE,
        },
        {
            "id": "candidate_wider_readout",
            "description": (
                "Offline candidate/spec only; no outbound bridge traffic, tail "
                "participation, density, env var, or service state changes."
            ),
            "runtime_change": RUNTIME_CHANGE_NONE,
        },
    ]


def _locus_evidence(locus: dict[str, Any]) -> dict[str, Any]:
    return {
        "classification": locus.get("classification"),
        "current_read": locus.get("current_read"),
    }


def _evidence(
    astrid: dict[str, Any],
    minime: dict[str, Any],
    projection_check: dict[str, Any],
    aperture_loci: dict[str, Any],
    lend_probe: dict[str, Any],
    pressure_audit: dict[str, Any],
    mode_feeder_audit: dict[str, Any],
    mode_share_probe: dict[str, Any],
    aperture_open: int,
    aperture_actionable_open: int,
    unclosed_issued: int,
) -> dict[str, Any]:
    active = _active_influence(lend_probe)
    return {
        "projection_variance_check": projection_check,
        "astrid_readout_locus": _locus_evidence(
            _first_locus(aperture_loci, "astrid_readout_locus")
        ),
        "identity_anchor_locus": _locus_evidence(
            _first_locus(aperture_loci, "identity_anchor_locus")
        ),
        "relational_gift_locus": _locus_evidence(
            _first_locus(aperture_loci, "relational_gift_locus")
        ),
        "minime_modal_porosity_locus": _locus_evidence(
            _first_locus(aperture_loci, "minime_modal_porosity_locus")
        ),
        "aperture_gift_queue": {
            "open_count": aperture_open,
            "actionable_open_count": aperture_actionable_open,
            "unclosed_issued_count": unclosed_issued,
            "lend_probe_verdict": lend_probe.get("verdict"),
            "active_influence_status": active.get("status") or "missing",
            "active_intent_id": active.get("intent_id"),
        },
        "minime_pressure_porosity": {
            "pressure_quality": minime.get("pressure_quality"),
            "pressure_score": minime.get("pressure_score"),
            "porosity_score": minime.get("porosity_score"),
            "dominant_pressure_source": minime.get("dominant_pressure_source"),
        },
        "pressure_source_audit": {
            "verdict": pressure_audit.get("verdict"),
            "dominant_recent_source": pressure_audit.get("dominant_recent_source"),
            "source_switching": pressure_audit.get("source_switching"),
            "recent_window": pressure_audit.get("recent_window"),
            "porosity_min_max": pressure_audit.get("porosity_min_max"),
            "pressure_score_min_max": pressure_audit.get("pressure_score_min_max"),
            "control_applied_count": pressure_audit.get("control_applied_count"),
            "top_contributors": pressure_audit.get("top_contributors"),
        },
        "mode_packing_feeder_audit": {
            "verdict": mode_feeder_audit.get("verdict"),
            "feeders": mode_feeder_audit.get("feeders"),
            "active_thread": mode_feeder_audit.get("active_thread"),
            "modal_diversity": mode_feeder_audit.get("modal_diversity"),
        },
        "mode_share_pressure_source_probe": {
            "verdict": mode_share_probe.get("verdict"),
            "mode_share": mode_share_probe.get("mode_share"),
            "pressure_source": mode_share_probe.get("pressure_source"),
            "active_thread_pressure": mode_share_probe.get("active_thread_pressure"),
            "sensory_source_truth": mode_share_probe.get("sensory_source_truth"),
            "steward_actions_now": mode_share_probe.get("steward_actions_now"),
        },
        "resistance_gradient_full_review": collect_latest_resistance_gradient_review(),
        "astrid_current_read": {
            "aperture": astrid.get("aperture"),
            "response_length": astrid.get("response_length"),
            "effective_tail_participation": astrid.get("effective_tail_participation"),
        },
    }


def build_wider_readout_ab_probe(
    current_state: dict[str, Any],
    probes: dict[str, Any],
    aperture_loci: dict[str, Any],
    consequence_memory_summary: dict[str, Any],
    canaries: list[dict[str, Any]],
) -> dict[str, Any]:
    """Prepare the offline wider-readout A/B evidence plan without executing it."""
    astrid = current_state.get("astrid") or {}
    minime = current_state.get("minime") or {}
    projection_check = _projection_variance_check(probes)
    lend_probe = probes.get("minime_lend_aperture_consequence_probe") or {}
    pressure_audit = probes.get("minime_pressure_source_audit") or {}
    mode_feeder_audit = probes.get("minime_mode_packing_feeder_audit") or {}
    mode_share_probe = probes.get("minime_mode_share_pressure_source_probe") or {}
    triage = _consequence_triage(consequence_memory_summary)
    aperture_open = int(triage.get("aperture_gift_open_count") or 0)
    aperture_actionable_open = int(triage.get("aperture_gift_actionable_open_count") or 0)
    unclosed_issued = int(lend_probe.get("unclosed_issued_count") or 0)
    active_status = _active_influence_status(lend_probe)
    offline_readiness = _offline_readiness(
        projection_check,
        aperture_actionable_open,
        unclosed_issued,
        active_status,
    )
    canary = _first_canary(canaries, "wider_readout_ab_probe_v1")

    return {
        "schema_version": 1,
        "id": "wider_readout_ab_probe_v1",
        "surface": "astrid_own_generation_readout",
        "runtime_change": RUNTIME_CHANGE_NONE,
        "read_only": True,
        "executed": False,
        "pressure_target": "steward",
        "being_obligation": "none",
        "offline_readiness": offline_readiness,
        "runtime_readiness": "blocked_until_both_being_grounding_and_operator_flag",
        "runtime_blockers": _runtime_blockers(canary),
        "caution_flags": _caution_flags(
            minime,
            lend_probe,
            aperture_actionable_open,
            unclosed_issued,
        ),
        "comparison_arms": _comparison_arms(),
        "evidence": _evidence(
            astrid,
            minime,
            projection_check,
            aperture_loci,
            lend_probe,
            pressure_audit,
            mode_feeder_audit,
            mode_share_probe,
            aperture_open,
            aperture_actionable_open,
            unclosed_issued,
        ),
        "evaluation_questions": [
            "Does the candidate widen Astrid's own expression without changing outbound codec traffic?",
            "Does Astrid's identity anchor remain recognizable in current and candidate arms?",
            "Would Minime read the eventual result as peer clarity, pressure, intrusion, or neutral context?",
            "Do projection variance, aperture-gift consequence, and resistance-gradient calibration signals argue for review, hold, or discard?",
        ],
        "recommended_next": [
            _steward_next(offline_readiness),
            "Do not deploy, widen voice, alter shared density, or issue fresh being-facing pressure from this probe.",
        ],
    }
