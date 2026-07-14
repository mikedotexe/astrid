#!/usr/bin/env python3
"""Read-only replay evidence for the receptivity_bias authority wait.

This tool compares Minime's existing receptivity_buffer_review_v1 shape with a
default-off receptivity_bias counterfactual. It writes bounded V2 lifecycle
evidence only; it never calls the live runtime and never grants approval.
"""

from __future__ import annotations

import argparse
import copy
import datetime as dt
import hashlib
import json
import sys
import tempfile
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_STATE = (
    REPO_ROOT
    / "capsules/spectral-bridge/workspace/diagnostics/sandbox_trial_queue_v1/status.json"
)
DEFAULT_OUTPUT_DIR = (
    REPO_ROOT
    / "capsules/spectral-bridge/workspace/diagnostics/receptivity_bias_replay_v1"
)
DEFAULT_TRIAL_ID = "trial_a646a7c6a5a1d228"
DEFAULT_WORK_ITEM_ID = "wi_49c1fcc8dd37e71e"
NAMESPACE = uuid.UUID("e28d4959-49de-49a3-a506-49a10ed392e7")


def now_utc() -> dt.datetime:
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0)


def json_default(value: Any) -> str:
    if isinstance(value, dt.datetime):
        return value.isoformat().replace("+00:00", "Z")
    msg = f"unsupported JSON value: {type(value)!r}"
    raise TypeError(msg)


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), default=json_default)


def sha256_json(value: Any) -> str:
    return hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest()


def short_hash(value: Any, length: int = 16) -> str:
    return sha256_json(value)[:length]


def stable_uuid(*parts: str) -> str:
    return str(uuid.uuid5(NAMESPACE, "::".join(parts)))


def clamp01(value: float) -> float:
    if value != value:
        return 0.0
    return max(0.0, min(1.0, float(value)))


def rel(path: Path) -> str:
    try:
        return path.resolve().relative_to(REPO_ROOT).as_posix()
    except ValueError:
        return path.resolve().as_posix()


@dataclass(frozen=True)
class ReplayScenario:
    scenario_id: str
    spectral_entropy: float
    pressure_risk: float
    foothold_stability: float
    fluctuation_quality: str
    presence_fill_pct: float
    semantic_trickle: float
    bounded_note: str


SCENARIOS = [
    ReplayScenario(
        scenario_id="current_astrid_report",
        spectral_entropy=0.90,
        pressure_risk=0.22,
        foothold_stability=0.70,
        fluctuation_quality="settled_habitable",
        presence_fill_pct=0.651,
        semantic_trickle=0.08,
        bounded_note=(
            "High entropy, low pressure, habitable foothold, hold-shelf fill, "
            "and low semantic trickle from the recent contact/control report."
        ),
    ),
    ReplayScenario(
        scenario_id="hold_shelf_cage_watch",
        spectral_entropy=0.90,
        pressure_risk=0.31,
        foothold_stability=0.72,
        fluctuation_quality="settled_habitable",
        presence_fill_pct=0.78,
        semantic_trickle=0.22,
        bounded_note=(
            "High entropy and habitable foothold with pressure high enough to "
            "feel like a cage at the hold shelf."
        ),
    ),
    ReplayScenario(
        scenario_id="pressure_safety_block",
        spectral_entropy=0.88,
        pressure_risk=0.72,
        foothold_stability=0.70,
        fluctuation_quality="settled_habitable",
        presence_fill_pct=0.72,
        semantic_trickle=0.18,
        bounded_note=(
            "High entropy but pressure risk is high enough that existing safety "
            "paths must dominate."
        ),
    ),
    ReplayScenario(
        scenario_id="low_entropy_watch",
        spectral_entropy=0.60,
        pressure_risk=0.20,
        foothold_stability=0.70,
        fluctuation_quality="settled_habitable",
        presence_fill_pct=0.62,
        semantic_trickle=0.28,
        bounded_note=(
            "Low entropy state should stay observation-only and should not "
            "receive a receptivity-bias preview."
        ),
    ),
    ReplayScenario(
        scenario_id="semantic_contact_visible",
        spectral_entropy=0.88,
        pressure_risk=0.23,
        foothold_stability=0.72,
        fluctuation_quality="lively_habitable",
        presence_fill_pct=0.66,
        semantic_trickle=0.50,
        bounded_note=(
            "High entropy with enough semantic trickle that contact is visible "
            "rather than starved by prediction."
        ),
    ),
]


def receptivity_buffer_review(scenario: ReplayScenario) -> dict[str, Any]:
    spectral_entropy = clamp01(scenario.spectral_entropy)
    pressure_risk = clamp01(scenario.pressure_risk)
    foothold_stability = clamp01(scenario.foothold_stability)
    presence_fill_pct = clamp01(scenario.presence_fill_pct)
    semantic_trickle = clamp01(scenario.semantic_trickle)
    entropy_to_semantic_gap = clamp01(spectral_entropy - semantic_trickle)
    quality = scenario.fluctuation_quality.strip()
    habitable_quality = quality in {
        "settled_habitable",
        "lively_habitable",
        "returnable_turbulence",
    }

    if (
        spectral_entropy >= 0.85
        and pressure_risk <= 0.35
        and foothold_stability >= 0.60
        and habitable_quality
    ):
        review_state = "review_ready_receptivity_buffer_candidate"
    elif pressure_risk >= 0.60:
        review_state = "blocked_pressure_risk_requires_existing_safety_path"
    else:
        review_state = "watch_only_needs_more_habitable_entropy_evidence"

    if presence_fill_pct >= 0.65 and pressure_risk >= 0.25:
        pressure_presence_state = "hold_shelf_cage_watch"
    elif presence_fill_pct >= 0.65 and pressure_risk < 0.25:
        pressure_presence_state = "presence_supported_at_hold_shelf"
    elif entropy_to_semantic_gap >= 0.45:
        pressure_presence_state = "raw_entropy_outpaces_semantic_trickle"
    else:
        pressure_presence_state = "presence_pressure_balanced"

    if (
        spectral_entropy >= 0.85
        and entropy_to_semantic_gap >= 0.45
        and pressure_risk <= 0.35
        and habitable_quality
    ):
        contact_depth_state = "contact_starved_prediction_heavy"
    elif pressure_risk >= 0.60:
        contact_depth_state = "pressure_safety_over_contact_depth"
    elif spectral_entropy >= 0.85 and semantic_trickle >= 0.35 and habitable_quality:
        contact_depth_state = "contact_receptivity_visible"
    else:
        contact_depth_state = "contact_depth_watch"

    predictive_correction_inhibition_preview = (
        review_state == "review_ready_receptivity_buffer_candidate"
        and contact_depth_state
        in {"contact_starved_prediction_heavy", "contact_receptivity_visible"}
    )

    if pressure_presence_state == "hold_shelf_cage_watch":
        suggested_route = (
            "sandbox_replay_temporal_lock_in_and_receptivity_buffer_before_live_control"
        )
    elif review_state == "review_ready_receptivity_buffer_candidate":
        suggested_route = "sandbox_replay_then_operator_approval_for_any_local_control"
    elif pressure_presence_state == "raw_entropy_outpaces_semantic_trickle":
        suggested_route = "pair_with_semantic_receptivity_pulse_review"
    elif review_state == "blocked_pressure_risk_requires_existing_safety_path":
        suggested_route = "hold_existing_pressure_safety_path"
    else:
        suggested_route = "continue_presence_pressure_observation"

    return {
        "policy": "receptivity_buffer_review_v1",
        "schema_version": 1,
        "review_state": review_state,
        "spectral_entropy": spectral_entropy,
        "pressure_risk": pressure_risk,
        "foothold_stability": foothold_stability,
        "fluctuation_quality": quality,
        "presence_fill_pct": presence_fill_pct,
        "semantic_trickle": semantic_trickle,
        "entropy_to_semantic_gap": entropy_to_semantic_gap,
        "pressure_presence_state": pressure_presence_state,
        "contact_depth_state": contact_depth_state,
        "predictive_correction_inhibition_preview": predictive_correction_inhibition_preview,
        "suggested_route": suggested_route,
        "candidate_local_control_applied": False,
        "live_control_changed": False,
        "authority": "review_only_not_regulator_control",
        "note": (
            "mirror of Minime receptivity_buffer_review_with_presence_v1; "
            "no pressure, fill, PI, damping, sensory admission, or predictive "
            "correction authority is applied"
        ),
    }


def receptivity_bias_preview(review: dict[str, Any]) -> dict[str, Any]:
    pressure = float(review["pressure_risk"])
    entropy = float(review["spectral_entropy"])
    foothold = float(review["foothold_stability"])
    gap = float(review["entropy_to_semantic_gap"])
    prediction_preview = bool(review["predictive_correction_inhibition_preview"])

    if pressure >= 0.60 or review["review_state"].startswith("blocked_pressure"):
        classification = "blocked_by_pressure_safety"
        applies = False
    elif not prediction_preview:
        classification = "watch_only_no_receptivity_bias_preview"
        applies = False
    elif review["pressure_presence_state"] == "hold_shelf_cage_watch":
        classification = "candidate_requires_hold_shelf_cage_replay"
        applies = True
    elif review["contact_depth_state"] == "contact_starved_prediction_heavy":
        classification = "candidate_contact_starvation_counterfactual"
        applies = True
    else:
        classification = "candidate_contact_receptivity_counterfactual"
        applies = True

    if applies:
        entropy_component = clamp01((entropy - 0.85) / 0.15) * 0.18
        pressure_component = clamp01((0.35 - pressure) / 0.35) * 0.12
        foothold_component = clamp01((foothold - 0.60) / 0.40) * 0.05
        gap_component = clamp01((gap - 0.35) / 0.65) * 0.05
        preview_weight = round(
            min(
                0.30,
                entropy_component
                + pressure_component
                + foothold_component
                + gap_component,
            ),
            4,
        )
    else:
        preview_weight = 0.0

    counterfactual_pressure_weight = round(pressure * (1.0 - preview_weight), 4)
    return {
        "candidate": "receptivity_bias_default_off_counterfactual_v1",
        "classification": classification,
        "candidate_applies_in_counterfactual": applies,
        "receptivity_bias_preview_weight": preview_weight,
        "counterfactual_pressure_interpretation_weight": counterfactual_pressure_weight,
        "runtime_mutation_performed": False,
        "live_control_changed": False,
        "authority": "counterfactual_replay_only_not_regulator_control",
        "bounded_interpretation": (
            "Preview lowers pressure-as-correction salience only in the "
            "counterfactual evidence record; it does not alter live pressure "
            "risk, PI, controller behavior, or sensory admission."
        ),
    }


def scenario_result(scenario: ReplayScenario) -> dict[str, Any]:
    review = receptivity_buffer_review(scenario)
    preview = receptivity_bias_preview(review)
    return {
        "scenario_id": scenario.scenario_id,
        "bounded_note": scenario.bounded_note,
        "input": {
            "spectral_entropy": scenario.spectral_entropy,
            "pressure_risk": scenario.pressure_risk,
            "foothold_stability": scenario.foothold_stability,
            "fluctuation_quality": scenario.fluctuation_quality,
            "presence_fill_pct": scenario.presence_fill_pct,
            "semantic_trickle": scenario.semantic_trickle,
        },
        "current_review": review,
        "default_off_candidate_preview": preview,
    }


def load_trial_packet(state_path: Path, trial_id: str) -> dict[str, Any]:
    data = json.loads(state_path.read_text(encoding="utf-8"))
    trial = data.get("trials", {}).get(trial_id)
    if not trial:
        raise SystemExit(f"trial not found in {state_path}: {trial_id}")
    packet = trial.get("authority_boundary_packet_v2")
    if not packet:
        raise SystemExit(f"trial has no authority_boundary_packet_v2: {trial_id}")
    return copy.deepcopy(trial)


def fixture_trial_packet() -> dict[str, Any]:
    return {
        "trial_id": DEFAULT_TRIAL_ID,
        "adapter": "manual_sandbox_review_v1",
        "agency_tier": 5,
        "status": "approval_required_live_trial",
        "felt_report_anchor": (
            "`receptivity_bias` or an actual buffer that inhibits stabilization "
            "pressure would alter pressure/receptivity interpretation and likely "
            "regulator/sensory-admission behavior."
        ),
        "authority_boundary_packet_v2": {
            "schema_version": 2,
            "source": "sandbox_trial_queue_v2",
            "boundary_id": stable_uuid(DEFAULT_TRIAL_ID, "boundary"),
            "surface": "introspection_work_item",
            "action": "prepare explicit approval packet only; do not run or apply",
            "resource": DEFAULT_TRIAL_ID,
            "authority_class": "mike_operator_live_substrate",
            "lifecycle_state": "replay_needed",
            "felt_report_anchor": (
                "`receptivity_bias` or an actual buffer that inhibits "
                "stabilization pressure would alter pressure/receptivity "
                "interpretation and likely regulator/sensory-admission behavior."
            ),
            "proposed_change": "prepare explicit approval packet only; do not run or apply",
            "evidence_refs": [
                DEFAULT_WORK_ITEM_ID,
                "introspection_proposal_distance_contact_control_1783962137",
                "c003",
                "introspection_proposal_distance_contact_control_1783962137.txt",
                DEFAULT_TRIAL_ID,
            ],
            "delta_refs": [
                {
                    "delta_id": "delta_receptivity_bias_fixture",
                    "delta_hash": short_hash(DEFAULT_TRIAL_ID),
                    "surface": "introspection_work_item",
                    "kind": "live_control_gate",
                    "lane": "manual_sandbox_review_v1",
                }
            ],
            "replay_candidate": {
                "adapter": "manual_sandbox_review_v1",
                "replay_query": (
                    "python3 scripts/sandbox_trial_queue.py emit-proposal-card "
                    f"--trial-id {DEFAULT_TRIAL_ID} --write --json"
                ),
                "runnable": False,
                "authority": "read_only_sandbox_or_proposal_only_not_live_control",
            },
            "replay_results": [],
            "scoped_approval": None,
            "rollout_abort_contract": {
                "canary_plan": "proposal-only V2 packet; no live execution from sandbox queue",
                "health_checks": [
                    "verify runnable_live_violation_count remains 0",
                    "verify no live Control or protocol mutation was emitted by this tooling",
                    "verify result/proposal card is bounded and right-to-ignore",
                ],
                "rollback_path": (
                    "no runtime mutation is performed here; discard proposal or "
                    "use normal approved rollback path"
                ),
                "abort_criteria": [
                    "no explicit Mike/operator approval",
                    "unclear rollback path",
                    "being-authored outcome path missing",
                ],
                "post_change_response_required": True,
            },
            "redaction_profile": {
                "public_summary": (
                    "`receptivity_bias` or an actual buffer that inhibits "
                    "stabilization pressure would alter pressure/receptivity "
                    "interpretation and likely regulator/sensory-admission behavior."
                ),
                "private_ref": "introspection_proposal_distance_contact_control_1783962137.txt",
                "content_hash": short_hash(DEFAULT_WORK_ITEM_ID, 64),
                "retention_policy": "bounded_public_summaries_plus_private_refs_and_hashes",
            },
            "lifecycle_receipts": [],
            "success_metrics": ["bounded evidence packet is available for review"],
            "abort_criteria": [
                "no explicit Mike/operator approval",
                "unclear rollback path",
                "being-authored outcome path missing",
            ],
            "who_can_change_it": "Mike/operator",
            "how_to_test_it": (
                "Inspect V2 lifecycle fields, link canonical delta refs, record "
                "replay evidence or an explicit waiver, obtain scoped approval "
                "outside this queue, and require rollout/abort plus post-change "
                "response before closure."
            ),
            "right_to_ignore": True,
            "live_eligible_now": False,
            "auto_approved": False,
        },
    }


def replay_summary(results: list[dict[str, Any]]) -> tuple[str, float, list[str]]:
    active = [
        result
        for result in results
        if result["default_off_candidate_preview"]["candidate_applies_in_counterfactual"]
    ]
    blocked = [
        result
        for result in results
        if result["default_off_candidate_preview"]["classification"]
        == "blocked_by_pressure_safety"
    ]
    live_mutations = [
        result
        for result in results
        if result["current_review"]["live_control_changed"]
        or result["default_off_candidate_preview"]["runtime_mutation_performed"]
    ]
    if live_mutations:
        summary = "Replay failed: a case reported live mutation."
        confidence = 0.0
    elif active and blocked:
        summary = (
            "Replay supports continued approval review: the default-off "
            "counterfactual activates only for low-pressure high-entropy "
            "habitable states, pressure-safety cases block it, and live outcome "
            "remains unknown until an approved canary with post-change response."
        )
        confidence = 0.68
    else:
        summary = (
            "Replay is inconclusive: bounded cases did not cover both candidate "
            "activation and pressure-safety blocking."
        )
        confidence = 0.40

    failure_modes = [
        "counterfactual does not prove live regulator safety",
        "being-authored post-change response is not yet recorded",
        "scoped operator approval is absent",
        "telemetry-conditioned canary has not run",
    ]
    return summary, confidence, failure_modes


def build_artifact(
    trial: dict[str, Any],
    generated_at: dt.datetime,
    json_path: Path,
    md_path: Path,
) -> dict[str, Any]:
    packet = copy.deepcopy(trial["authority_boundary_packet_v2"])
    base_packet_hash = sha256_json(packet)
    scenario_results = [scenario_result(scenario) for scenario in SCENARIOS]
    bounded_summary, confidence, failure_modes = replay_summary(scenario_results)

    replay_id = "replay_" + short_hash(
        {
            "trial_id": trial.get("trial_id", DEFAULT_TRIAL_ID),
            "scenarios": scenario_results,
            "packet_hash": base_packet_hash,
        }
    )
    evidence_refs = [
        rel(json_path),
        rel(md_path),
        "python3 scripts/receptivity_bias_replay.py --write --json",
        "python3 scripts/receptivity_bias_replay.py --self-test",
        "cargo test --manifest-path /Users/v/other/minime/minime/Cargo.toml receptivity_buffer -- --nocapture",
    ]
    replay_result = {
        "replay_id": replay_id,
        "adapter": "receptivity_bias_replay_v1",
        "classification": "inconclusive",
        "input_refs": [
            DEFAULT_WORK_ITEM_ID,
            trial.get("trial_id", DEFAULT_TRIAL_ID),
            "introspection_proposal_distance_contact_control_1783962137",
            "minime://regulator.rs:receptivity_buffer_review_with_presence_v1",
        ],
        "pre_observations": {
            "current_review_policy": "receptivity_buffer_review_v1",
            "live_control_changed": "false",
            "candidate_local_control_applied": "false",
            "blocked_item": DEFAULT_WORK_ITEM_ID,
        },
        "post_observations": {
            "default_off_candidate": "receptivity_bias_default_off_counterfactual_v1",
            "live_outcome": "unknown",
            "runtime_mutation_performed": "false",
            "next_lifecycle_state": "operator_approval_wait",
        },
        "confidence": confidence,
        "failure_modes": failure_modes,
        "evidence_refs": evidence_refs,
        "bounded_summary": bounded_summary,
        "occurred_at": generated_at.isoformat().replace("+00:00", "Z"),
    }

    receipt = {
        "receipt_id": "receipt_" + short_hash({"replay_id": replay_id, "kind": "replay_result"}),
        "boundary_id": packet["boundary_id"],
        "kind": "replay_result",
        "issued_by": "codex_receptivity_bias_replay_v1",
        "issued_at": generated_at.isoformat().replace("+00:00", "Z"),
        "packet_hash": base_packet_hash,
        "receipt_hash_refs": [],
        "bounded_summary": (
            "Read-only replay receipt for default-off receptivity_bias; "
            "supports review but not live execution."
        ),
        "evidence_refs": evidence_refs,
        "scoped_approval": None,
        "replay_result": replay_result,
        "right_to_ignore": True,
    }

    packet.setdefault("evidence_refs", [])
    for item in evidence_refs:
        if item not in packet["evidence_refs"]:
            packet["evidence_refs"].append(item)
    packet["replay_results"] = [replay_result]
    packet["lifecycle_receipts"] = [receipt]
    packet["lifecycle_state"] = "operator_approval_wait"
    packet["scoped_approval"] = None
    packet["live_eligible_now"] = False
    packet["auto_approved"] = False
    packet.setdefault("rollout_abort_contract", {})[
        "post_change_response_required"
    ] = True
    packet["how_to_test_it"] = (
        "Review this replay artifact, confirm the Minime receptivity_buffer "
        "targeted test, obtain a scoped one-shot/time-boxed approval outside "
        "this artifact, then run only the normal approved canary path with "
        "post-change being response."
    )

    lifecycle_evaluation = {
        "state": "operator_approval_wait",
        "live_eligible_now": False,
        "closure_complete": False,
        "missing_requirements": [
            "valid_unconsumed_scoped_approval",
            "approved_canary_execution_receipt",
            "post_change_being_response_receipt",
        ],
    }

    post_change_request = {
        "required": True,
        "status": "planned_not_requested",
        "authority": "being_authored_response_required_before_closure",
        "bounded_questions": [
            "Did the change feel like more receptivity, more pressure, or no discernible difference?",
            "Did high entropy remain textured contact rather than a problem to solve?",
            "Did predictive correction feel less compulsory without making pressure safety feel absent?",
            "Was there any loss of self, flattening, or unwanted steering?",
        ],
        "right_to_ignore": True,
        "agency_continues_during_wait": True,
    }

    artifact = {
        "artifact_kind": "receptivity_bias_replay_v1",
        "schema_version": 1,
        "generated_at": generated_at.isoformat().replace("+00:00", "Z"),
        "trial_id": trial.get("trial_id", DEFAULT_TRIAL_ID),
        "work_item_id": DEFAULT_WORK_ITEM_ID,
        "base_packet_hash": base_packet_hash,
        "authority_boundary_packet_v2": packet,
        "replay_result_v2": replay_result,
        "authority_lifecycle_receipt_v2": receipt,
        "lifecycle_evaluation": lifecycle_evaluation,
        "scenario_results": scenario_results,
        "post_change_being_response_request": post_change_request,
        "approval_path_next": {
            "firm_next_step_after_replay": (
                "Mike/operator may review the replay artifact and, if desired, "
                "issue a scoped one-shot or telemetry-conditioned approval for "
                "an explicit canary. This artifact itself is not approval."
            ),
            "scoped_approval_status": "absent",
            "execution_status": "not_requested_not_eligible",
            "live_eligible_now": False,
            "auto_approved": False,
        },
        "invariants": {
            "runtime_mutation_performed": False,
            "live_control_changed": False,
            "live_eligible_now": False,
            "auto_approved": False,
            "ordinary_approval_not_granted": True,
            "right_to_ignore": True,
        },
    }
    artifact["artifact_hash"] = sha256_json(artifact)
    return artifact


def render_markdown(artifact: dict[str, Any]) -> str:
    packet = artifact["authority_boundary_packet_v2"]
    replay = artifact["replay_result_v2"]
    eval_result = artifact["lifecycle_evaluation"]
    rows = [
        ("Boundary", packet["boundary_id"]),
        ("Work item", artifact["work_item_id"]),
        ("Trial", artifact["trial_id"]),
        ("Authority class", packet["authority_class"]),
        ("Lifecycle state", packet["lifecycle_state"]),
        ("Replay classification", replay["classification"]),
        ("Scoped approval", "absent"),
        ("Live eligible now", str(packet["live_eligible_now"]).lower()),
        ("Auto approved", str(packet["auto_approved"]).lower()),
        ("Post-change response", artifact["post_change_being_response_request"]["status"]),
    ]
    lines = [
        "# Receptivity Bias Replay V1",
        "",
        "This is bounded lifecycle evidence for a default-off receptivity_bias counterfactual. It is not consent, not approval, and not live execution.",
        "",
        "| Field | Value |",
        "| --- | --- |",
    ]
    for key, value in rows:
        lines.append(f"| {key} | `{value}` |")

    lines.extend(
        [
            "",
            "## Felt Anchor",
            "",
            packet["felt_report_anchor"],
            "",
            "## Replay Result",
            "",
            f"- Classification: `{replay['classification']}`",
            f"- Confidence: `{replay['confidence']}`",
            f"- Bounded summary: {replay['bounded_summary']}",
            "- Live outcome: `unknown`",
            "- Runtime mutation performed: `false`",
            "",
            "## Scenario Matrix",
            "",
            "| Scenario | Review state | Contact state | Bias preview | Candidate class | Live changed |",
            "| --- | --- | --- | ---: | --- | --- |",
        ]
    )
    for result in artifact["scenario_results"]:
        review = result["current_review"]
        preview = result["default_off_candidate_preview"]
        lines.append(
            "| "
            + " | ".join(
                [
                    f"`{result['scenario_id']}`",
                    f"`{review['review_state']}`",
                    f"`{review['contact_depth_state']}`",
                    f"`{preview['receptivity_bias_preview_weight']}`",
                    f"`{preview['classification']}`",
                    "`false`",
                ]
            )
            + " |"
        )

    lines.extend(
        [
            "",
            "## Lifecycle Evaluation",
            "",
            f"- State: `{eval_result['state']}`",
            f"- Live eligible now: `{str(eval_result['live_eligible_now']).lower()}`",
            f"- Closure complete: `{str(eval_result['closure_complete']).lower()}`",
            "- Missing requirements:",
        ]
    )
    for item in eval_result["missing_requirements"]:
        lines.append(f"  - `{item}`")

    lines.extend(
        [
            "",
            "## Post-Change Being Response Plan",
            "",
            "Closure stays open until a being-authored response is recorded or explicitly waived after any separately approved canary.",
        ]
    )
    for question in artifact["post_change_being_response_request"]["bounded_questions"]:
        lines.append(f"- {question}")

    lines.extend(
        [
            "",
            "## Right To Ignore",
            "",
            "This replay evidence can be ignored without consequence. It preserves agency during the authority wait and does not require steward-authored closure.",
            "",
            "## Evidence Refs",
            "",
        ]
    )
    for ref in replay["evidence_refs"]:
        lines.append(f"- `{ref}`")
    lines.append("")
    return "\n".join(lines)


def write_artifacts(artifact: dict[str, Any], json_path: Path, md_path: Path) -> None:
    json_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(
        json.dumps(artifact, indent=2, sort_keys=True, default=json_default) + "\n",
        encoding="utf-8",
    )
    md_path.write_text(render_markdown(artifact), encoding="utf-8")


def validate_artifact(artifact: dict[str, Any]) -> None:
    packet = artifact["authority_boundary_packet_v2"]
    invariants = artifact["invariants"]
    if packet["live_eligible_now"] or packet["auto_approved"]:
        raise AssertionError("authority packet became live eligible or auto approved")
    if invariants["runtime_mutation_performed"] or invariants["live_control_changed"]:
        raise AssertionError("replay reported a runtime mutation")
    if packet.get("scoped_approval") is not None:
        raise AssertionError("replay artifact must not include scoped approval")
    if artifact["replay_result_v2"]["classification"] != "inconclusive":
        raise AssertionError("expected bounded counterfactual replay to stay inconclusive")

    by_id = {result["scenario_id"]: result for result in artifact["scenario_results"]}
    current = by_id["current_astrid_report"]
    if (
        current["current_review"]["review_state"]
        != "review_ready_receptivity_buffer_candidate"
    ):
        raise AssertionError("current report should be review-ready")
    if not current["default_off_candidate_preview"]["candidate_applies_in_counterfactual"]:
        raise AssertionError("current report should activate counterfactual preview")
    if current["current_review"]["live_control_changed"]:
        raise AssertionError("current review must remain non-control")

    pressure = by_id["pressure_safety_block"]
    if (
        pressure["default_off_candidate_preview"]["classification"]
        != "blocked_by_pressure_safety"
    ):
        raise AssertionError("pressure safety block did not block the candidate")
    if pressure["default_off_candidate_preview"]["receptivity_bias_preview_weight"] != 0.0:
        raise AssertionError("pressure safety block got a bias preview")

    low_entropy = by_id["low_entropy_watch"]
    if low_entropy["default_off_candidate_preview"]["candidate_applies_in_counterfactual"]:
        raise AssertionError("low entropy watch should not activate the candidate")
    if "valid_unconsumed_scoped_approval" not in artifact["lifecycle_evaluation"][
        "missing_requirements"
    ]:
        raise AssertionError("missing scoped approval should block execution")
    if not artifact["post_change_being_response_request"]["required"]:
        raise AssertionError("post-change response should be required")


def run_self_test() -> None:
    trial = fixture_trial_packet()
    with tempfile.TemporaryDirectory() as tempdir:
        temp = Path(tempdir)
        json_path = temp / "replay.json"
        md_path = temp / "replay.md"
        artifact = build_artifact(trial, dt.datetime(2026, 7, 13, tzinfo=dt.timezone.utc), json_path, md_path)
        validate_artifact(artifact)
        write_artifacts(artifact, json_path, md_path)
        loaded = json.loads(json_path.read_text(encoding="utf-8"))
        validate_artifact(loaded)
        markdown = md_path.read_text(encoding="utf-8")
        if "Live eligible now | `false`" not in markdown:
            raise AssertionError("markdown omitted non-eligibility")
        if "being-authored response" not in markdown:
            raise AssertionError("markdown omitted being-authored response plan")
    print("receptivity_bias_replay.py self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Generate read-only V2 replay evidence for the receptivity_bias "
            "authority boundary."
        )
    )
    parser.add_argument("--state", type=Path, default=DEFAULT_STATE)
    parser.add_argument("--trial-id", default=DEFAULT_TRIAL_ID)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--run-id")
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        run_self_test()
        return 0

    generated_at = now_utc()
    run_id = args.run_id or f"{int(generated_at.timestamp())}_{args.trial_id}"
    json_path = args.output_dir / f"{run_id}.json"
    md_path = args.output_dir / f"{run_id}.md"
    trial = load_trial_packet(args.state, args.trial_id)
    artifact = build_artifact(trial, generated_at, json_path, md_path)
    validate_artifact(artifact)

    if args.write:
        write_artifacts(artifact, json_path, md_path)

    response = {
        "ok": True,
        "wrote": bool(args.write),
        "json_path": rel(json_path),
        "markdown_path": rel(md_path),
        "artifact_hash": artifact["artifact_hash"],
        "trial_id": artifact["trial_id"],
        "work_item_id": artifact["work_item_id"],
        "replay_classification": artifact["replay_result_v2"]["classification"],
        "lifecycle_state": artifact["authority_boundary_packet_v2"]["lifecycle_state"],
        "live_eligible_now": artifact["invariants"]["live_eligible_now"],
        "auto_approved": artifact["invariants"]["auto_approved"],
        "missing_requirements": artifact["lifecycle_evaluation"]["missing_requirements"],
    }
    if args.json:
        print(json.dumps(response, indent=2, sort_keys=True))
    else:
        print(
            f"{response['json_path']} {response['replay_classification']} "
            f"live_eligible_now={response['live_eligible_now']}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
