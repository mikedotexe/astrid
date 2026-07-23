"""Operator actions for preregistered capture-first studies."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import time
from typing import Any

try:
    from experiential_systems.common import (
        RecordValidationError,
        canonical_json,
        sha256_bytes,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError,
        canonical_json,
        sha256_bytes,
    )

from felt_mechanism_concordance.model import (
    ConcordanceObservationV2,
    ConcordanceResultV2,
    ConcordanceStudyV1,
    FeltMomentRefV1,
    StudyStateV1,
)
from felt_mechanism_concordance.projector import (
    append_operator_event as append_concordance_event,
    replay as replay_concordance,
    valid_claim_and_witness,
    valid_dossier,
)

from .assembly import assemble as assemble_window
from .assembly import load_fixture
from .config import DEFAULT_MANIFEST, load_manifest
from .context import resolve_observation_context
from .model import (
    EvidenceStudyCampaignV1,
    EvidenceStudyPlanV1,
    MechanicalComparisonReceiptV1,
    ReviewOutcomeV1,
    StudyWindowReceiptV1,
    StudyWindowSpecV1,
)
from .projector import replay
from .review import (
    StudyCaptureGapReceiptV1,
    StudyReviewReceiptV1,
    validate_review_admission,
)
from .storage import (
    active_windows,
    append_event,
    arm_window,
    disarm_window,
)


def _intervention_signature(study: dict[str, Any]) -> str:
    bounded = {
        key: study.get(key)
        for key in (
            "study_key",
            "sample_kind",
            "comparison_kind",
            "baseline_cohort",
            "candidate_cohort",
            "metric_names",
            "thresholds",
        )
    }
    return sha256_bytes(canonical_json(bounded).encode("utf-8"))


def _campaign_records(
    workspace: Path, manifest_path: Path
) -> list[
    tuple[
        EvidenceStudyCampaignV1,
        list[tuple[EvidenceStudyPlanV1, ConcordanceStudyV1]],
    ]
]:
    manifest = load_manifest(manifest_path)
    result = []
    for campaign_value in manifest["campaigns"]:
        prepared: list[tuple[dict[str, Any], ConcordanceStudyV1, str]] = []
        for study_value in campaign_value["studies"]:
            claim_id = str(study_value["canonical_claim_id"])
            witness_id = str(study_value["witness_id"])
            dossier_id = str(study_value["dossier_id"])
            if not valid_claim_and_witness(workspace, claim_id, witness_id):
                raise RecordValidationError(
                    f"claim or witness anchor is unavailable: {claim_id}"
                )
            if not valid_dossier(workspace, dossier_id):
                raise RecordValidationError(f"dossier anchor is unavailable: {dossier_id}")
            signature = _intervention_signature(study_value)
            moment = FeltMomentRefV1.build(
                claim_id,
                witness_id,
                list(study_value["field_refs"]),
            )
            study = ConcordanceStudyV1.build(
                moment=moment,
                intervention_signature_sha256=signature,
                dossier_id=dossier_id,
            )
            prepared.append((study_value, study, signature))
        campaign = EvidenceStudyCampaignV1.build(
            campaign_key=str(campaign_value["campaign_key"]),
            comparison_domain=str(campaign_value["comparison_domain"]),
            study_ids=[study.study_id for _, study, _ in prepared],
            review_opportunity_limit=2,
            follow_up_requires_named_friction=True,
        )
        plans = []
        for study_value, study, signature in prepared:
            plan = EvidenceStudyPlanV1.build(
                plan_version=1,
                frozen_prior_plan_sha256=None,
                campaign_id=campaign.campaign_id,
                concordance_study_id=study.study_id,
                canonical_claim_id=str(study_value["canonical_claim_id"]),
                dossier_id=str(study_value["dossier_id"]),
                witness_id=str(study_value["witness_id"]),
                sample_kind=str(study_value["sample_kind"]),
                comparison_kind=str(study_value["comparison_kind"]),
                baseline_cohort=str(study_value["baseline_cohort"]),
                candidate_cohort=str(study_value["candidate_cohort"]),
                metric_names=list(study_value["metric_names"]),
                thresholds=dict(study_value["thresholds"]),
                minimum_total_samples=int(study_value["minimum_total_samples"]),
                minimum_baseline_samples=int(
                    study_value["minimum_baseline_samples"]
                ),
                minimum_candidate_samples=int(
                    study_value["minimum_candidate_samples"]
                ),
                duration_minutes=int(study_value["duration_minutes"]),
                sample_limit=int(study_value["sample_limit"]),
                extension_limit=int(study_value["extension_limit"]),
                intervention_signature_sha256=signature,
            )
            plans.append((plan, study))
        result.append((campaign, plans))
    return result


def seed(
    workspace: Path,
    *,
    actor: str,
    manifest_path: Path = DEFAULT_MANIFEST,
) -> dict[str, Any]:
    state = replay(workspace)
    studies, _, _, _, concordance_errors = replay_concordance(workspace)
    if state["errors"] or concordance_errors:
        raise RecordValidationError("existing study history must verify before seeding")
    appended = []
    for campaign, plan_studies in _campaign_records(workspace, manifest_path):
        existing_campaign = state["campaigns"].get(campaign.campaign_id)
        if existing_campaign is None:
            appended.append(
                append_event(workspace, "campaign_seeded", campaign.to_dict(), actor)
            )
        elif existing_campaign != campaign:
            raise RecordValidationError("seed campaign conflicts with durable history")
        for plan, study in plan_studies:
            existing_study = studies.get(study.study_id)
            if existing_study is None:
                append_concordance_event(
                    workspace, "study_created", study.to_dict(), actor
                )
            elif existing_study.moment != study.moment:
                raise RecordValidationError("Concordance study identity conflict")
            history = state["plan_history"].get(study.study_id, [])
            if not history:
                appended.append(
                    append_event(
                        workspace, "plan_preregistered", plan.to_dict(), actor
                    )
                )
            elif history[0] != plan:
                raise RecordValidationError("seed plan conflicts with durable history")
    return {
        "valid": True,
        "seeded_event_count": len(appended),
        "campaign_count": 3,
        "study_count": 4,
        "manifest_sha256": load_manifest(manifest_path)["manifest_sha256"],
    }


def _find_plan(state: dict[str, Any], identifier: str) -> EvidenceStudyPlanV1:
    for history in state["plan_history"].values():
        for plan in history:
            if plan.plan_id == identifier:
                return plan
    plan = state["plans"].get(identifier)
    if plan is None:
        raise RecordValidationError("unknown study plan or Concordance study")
    return plan


def revise_plan(
    workspace: Path,
    plan_id: str,
    overrides: dict[str, Any],
    *,
    actor: str,
) -> EvidenceStudyPlanV1:
    state = replay(workspace)
    current = _find_plan(state, plan_id)
    if any(
        window.plan_id == current.plan_id for window in state["windows"].values()
    ):
        raise RecordValidationError("a plan cannot be revised after capture begins")
    values = {
        "plan_version": current.plan_version + 1,
        "frozen_prior_plan_sha256": current.plan_sha256,
        "campaign_id": current.campaign_id,
        "concordance_study_id": current.concordance_study_id,
        "canonical_claim_id": current.canonical_claim_id,
        "dossier_id": current.dossier_id,
        "witness_id": current.witness_id,
        "sample_kind": current.sample_kind,
        "comparison_kind": current.comparison_kind,
        "baseline_cohort": current.baseline_cohort,
        "candidate_cohort": current.candidate_cohort,
        "metric_names": list(current.metric_names),
        "thresholds": current.thresholds,
        "minimum_total_samples": current.minimum_total_samples,
        "minimum_baseline_samples": current.minimum_baseline_samples,
        "minimum_candidate_samples": current.minimum_candidate_samples,
        "duration_minutes": current.duration_minutes,
        "sample_limit": current.sample_limit,
        "extension_limit": current.extension_limit,
        "intervention_signature_sha256": current.intervention_signature_sha256,
    }
    unsupported = sorted(set(overrides) - set(values))
    if unsupported:
        raise RecordValidationError(
            f"unsupported plan revision fields: {', '.join(unsupported)}"
        )
    values.update(overrides)
    revised = EvidenceStudyPlanV1.build(**values)
    append_event(workspace, "plan_preregistered", revised.to_dict(), actor)
    return revised


def start_capture(
    workspace: Path,
    plan_id: str,
    *,
    actor: str,
    duration_minutes: int | None = None,
    sample_limit: int | None = None,
    signal_capture_window_ref: str | None = None,
    extension_of_window_id: str | None = None,
) -> StudyWindowSpecV1:
    state = replay(workspace)
    if state["errors"]:
        raise RecordValidationError("study history must verify before capture")
    plan = _find_plan(state, plan_id)
    started = int(time.time() * 1_000)
    duration = plan.duration_minutes if duration_minutes is None else duration_minutes
    limit = plan.sample_limit if sample_limit is None else sample_limit
    if duration > plan.duration_minutes:
        raise RecordValidationError(
            "capture duration cannot exceed the frozen per-window plan"
        )
    if limit > plan.sample_limit:
        raise RecordValidationError(
            "capture sample limit cannot exceed the frozen plan"
        )
    plan_windows = [
        window
        for window in state["windows"].values()
        if window.plan_id == plan.plan_id
    ]
    if extension_of_window_id is None:
        if any(window.extension_of_window_id is None for window in plan_windows):
            raise RecordValidationError(
                "the plan already has an initial capture window"
            )
    else:
        parent = state["windows"].get(extension_of_window_id)
        if parent is None or parent.plan_id != plan.plan_id:
            raise RecordValidationError(
                "capture extension must name a window from the same frozen plan"
            )
        if parent.extension_of_window_id is not None:
            raise RecordValidationError("capture extensions cannot be chained")
        if parent.window_id not in state["stopped"]:
            raise RecordValidationError(
                "capture extension requires a stopped initial window"
            )
        extension_count = sum(
            window.extension_of_window_id == parent.window_id
            for window in plan_windows
        )
        if extension_count >= plan.extension_limit:
            raise RecordValidationError(
                "capture extension limit has been exhausted"
            )
    spec = StudyWindowSpecV1.build(
        campaign_id=plan.campaign_id,
        study_id=plan.concordance_study_id,
        plan_id=plan.plan_id,
        plan_sha256=plan.plan_sha256,
        sample_kinds=[plan.sample_kind],
        started_at_unix_ms=started,
        expires_at_unix_ms=started + duration * 60 * 1_000,
        sample_limit=limit,
        actor=actor,
        signal_capture_window_ref=signal_capture_window_ref,
        extension_of_window_id=extension_of_window_id,
    )
    arm_window(workspace, spec)
    try:
        append_event(workspace, "window_started", spec.to_dict(), actor)
    except Exception:
        disarm_window(workspace, spec.window_id)
        raise
    return spec


def stop_capture(workspace: Path, window_id: str, *, actor: str) -> StudyWindowSpecV1:
    spec = disarm_window(workspace, window_id)
    append_event(workspace, "window_stopped", spec.to_dict(), actor)
    return spec


def capture_status(workspace: Path) -> dict[str, Any]:
    state = replay(workspace)
    active = active_windows(workspace)
    return {
        "valid": not state["errors"],
        "active_windows": {
            key: value.to_dict() for key, value in sorted(active.items())
        },
        "historical_window_count": len(state["windows"]),
        "errors": state["errors"],
    }


def reconcile(workspace: Path, *, actor: str) -> dict[str, Any]:
    now = int(time.time() * 1_000)
    recovered = []
    for window_id, spec in list(active_windows(workspace).items()):
        if spec.expires_at_unix_ms > now:
            continue
        stopped = disarm_window(workspace, window_id)
        append_event(workspace, "window_stopped", stopped.to_dict(), actor)
        gap = StudyCaptureGapReceiptV1.build(
            window_id=window_id,
            study_id=spec.study_id,
            reason="crash_recovery_gap",
            dropped_sample_count=0,
            observed_at_unix_ms=now,
            source_ref=window_id,
        )
        append_event(workspace, "capture_gap_recorded", gap.to_dict(), actor)
        recovered.append(window_id)
    return {"valid": True, "recovered_window_ids": recovered}


def _capture_ref(receipt: StudyWindowReceiptV1) -> str:
    return (
        f"capture:{receipt.receipt_id}:sha256:"
        f"{receipt.scalar_fixture_sha256}"
    )


def _advance_dossiers(
    workspace: Path, plan: EvidenceStudyPlanV1, receipts: list[StudyWindowReceiptV1]
) -> None:
    if not all(receipt.sufficient for receipt in receipts):
        return
    from experiment_dossiers import transition

    baseline = next(item for item in receipts if item.role == "baseline")
    candidate = next(item for item in receipts if item.role == "candidate")
    try:
        transition(
            workspace,
            plan.dossier_id,
            "baseline-captured",
            _capture_ref(baseline),
            None,
        )
    except ValueError as error:
        if "invalid dossier transition" not in str(error):
            raise
    try:
        transition(
            workspace,
            plan.dossier_id,
            "candidate-captured",
            _capture_ref(candidate),
            None,
        )
    except ValueError as error:
        if "invalid dossier transition" not in str(error):
            raise


def _observation(
    workspace: Path,
    plan: EvidenceStudyPlanV1,
    spec: StudyWindowSpecV1,
    receipt: StudyWindowReceiptV1,
) -> ConcordanceObservationV2:
    context = resolve_observation_context(workspace, plan, spec, receipt)
    return ConcordanceObservationV2.build(
        study_id=plan.concordance_study_id,
        role=receipt.role,
        observation_ref=receipt.receipt_id,
        observation_sha256=receipt.scalar_fixture_sha256,
        telemetry_relation=context["telemetry_relation"],
        mechanical_pass=receipt.sufficient,
        witness_context_refs=context["witness_context_refs"],
        representation_transition_refs=context[
            "representation_transition_refs"
        ],
        model_qos_refs=context["model_qos_refs"],
        reciprocal_state_refs=context["reciprocal_state_refs"],
        signal_stage_refs=context["signal_stage_refs"],
        minime_telemetry_refs=context["minime_telemetry_refs"],
    )


def _advance_concordance(
    workspace: Path,
    plan: EvidenceStudyPlanV1,
    spec: StudyWindowSpecV1,
    receipts: list[StudyWindowReceiptV1],
    *,
    actor: str,
) -> None:
    studies, _, _, _, errors = replay_concordance(workspace)
    if errors:
        raise RecordValidationError("Concordance history is invalid")
    current = studies.get(plan.concordance_study_id)
    if current is None:
        raise RecordValidationError("Concordance study is unavailable")
    baseline = next(item for item in receipts if item.role == "baseline")
    candidate = next(item for item in receipts if item.role == "candidate")
    if current.state == StudyStateV1.DRAFT.value:
        current = ConcordanceStudyV1.build(
            moment=current.moment,
            intervention_signature_sha256=current.intervention_signature_sha256,
            dossier_id=current.dossier_id,
            state=StudyStateV1.CAPTURE_READY.value,
            baseline_capture_ref=baseline.receipt_id,
        )
        append_concordance_event(
            workspace, "study_capture_prepared", current.to_dict(), actor
        )
        append_concordance_event(
            workspace,
            "observation_recorded",
            _observation(workspace, plan, spec, baseline).to_dict(),
            actor,
        )
        current = ConcordanceStudyV1.build(
            moment=current.moment,
            intervention_signature_sha256=current.intervention_signature_sha256,
            dossier_id=current.dossier_id,
            state=StudyStateV1.BASELINE_CAPTURED.value,
            baseline_capture_ref=baseline.receipt_id,
        )
        append_concordance_event(
            workspace, "study_state_changed", current.to_dict(), actor
        )
    if current.state == StudyStateV1.BASELINE_CAPTURED.value:
        prepared = ConcordanceStudyV1.build(
            moment=current.moment,
            intervention_signature_sha256=current.intervention_signature_sha256,
            dossier_id=current.dossier_id,
            state=StudyStateV1.BASELINE_CAPTURED.value,
            baseline_capture_ref=current.baseline_capture_ref,
            candidate_capture_ref=candidate.receipt_id,
        )
        append_concordance_event(
            workspace, "study_capture_prepared", prepared.to_dict(), actor
        )
        append_concordance_event(
            workspace,
            "observation_recorded",
            _observation(workspace, plan, spec, candidate).to_dict(),
            actor,
        )
        candidate_state = ConcordanceStudyV1.build(
            moment=current.moment,
            intervention_signature_sha256=current.intervention_signature_sha256,
            dossier_id=current.dossier_id,
            state=StudyStateV1.CANDIDATE_CAPTURED.value,
            baseline_capture_ref=current.baseline_capture_ref,
            candidate_capture_ref=candidate.receipt_id,
        )
        append_concordance_event(
            workspace, "study_state_changed", candidate_state.to_dict(), actor
        )


def _advance_comparison_concordance(
    workspace: Path,
    state: dict[str, Any],
    plan: EvidenceStudyPlanV1,
    comparison: MechanicalComparisonReceiptV1,
    *,
    actor: str,
) -> None:
    baseline = state["receipts"].get(comparison.baseline_receipt_id)
    candidate = state["receipts"].get(comparison.candidate_receipt_id)
    if baseline is None or candidate is None:
        raise RecordValidationError(
            "mechanical comparison receipts are unavailable"
        )
    spec = state["windows"].get(baseline.window_id)
    if spec is None or candidate.window_id != baseline.window_id:
        raise RecordValidationError(
            "mechanical comparison window lineage is unavailable"
        )
    _advance_concordance(
        workspace, plan, spec, [baseline, candidate], actor=actor
    )
    studies, _, _, _, errors = replay_concordance(workspace)
    current = studies.get(plan.concordance_study_id)
    if errors or current is None:
        raise RecordValidationError(
            "Concordance comparison preparation is invalid"
        )
    if current.state == StudyStateV1.CANDIDATE_CAPTURED.value:
        ready = ConcordanceStudyV1.build(
            moment=current.moment,
            intervention_signature_sha256=current.intervention_signature_sha256,
            dossier_id=current.dossier_id,
            state=StudyStateV1.COMPARISON_READY.value,
            baseline_capture_ref=current.baseline_capture_ref,
            candidate_capture_ref=current.candidate_capture_ref,
        )
        append_concordance_event(
            workspace, "study_state_changed", ready.to_dict(), actor
        )


def assemble(
    workspace: Path, plan_id: str, window_id: str, *, actor: str
) -> dict[str, Any]:
    state = replay(workspace)
    plan = _find_plan(state, plan_id)
    spec = state["windows"].get(window_id)
    if spec is None:
        raise RecordValidationError("unknown capture window")
    if spec.window_id not in state["stopped"]:
        raise RecordValidationError(
            "capture window must be stopped before assembly"
        )
    existing_receipts = [
        receipt
        for receipt in state["receipts"].values()
        if receipt.plan_id == plan.plan_id
        and receipt.window_id == spec.window_id
    ]
    if existing_receipts:
        existing_gaps = [
            gap
            for gap in state["gaps"].values()
            if gap.window_id == spec.window_id
        ]
        return {
            "valid": True,
            "idempotent_reuse": True,
            "receipts": [receipt.to_dict() for receipt in existing_receipts],
            "gaps": [gap.to_dict() for gap in existing_gaps],
        }
    source_specs = [spec]
    if spec.extension_of_window_id is not None:
        parent = state["windows"].get(spec.extension_of_window_id)
        if parent is None or parent.window_id not in state["stopped"]:
            raise RecordValidationError(
                "capture extension lineage is incomplete"
            )
        source_specs = [parent, spec]
    receipts, gaps = assemble_window(
        workspace, plan, spec, source_specs=source_specs
    )
    for gap in gaps:
        append_event(workspace, "capture_gap_recorded", gap.to_dict(), actor)
    for receipt in receipts:
        append_event(workspace, "window_assembled", receipt.to_dict(), actor)
    _advance_dossiers(workspace, plan, receipts)
    _advance_concordance(workspace, plan, spec, receipts, actor=actor)
    return {
        "valid": True,
        "receipts": [receipt.to_dict() for receipt in receipts],
        "gaps": [gap.to_dict() for gap in gaps],
    }


def _mean_metrics(samples: list[dict[str, Any]]) -> dict[str, float]:
    names = sorted(
        {
            name
            for sample in samples
            for name in (sample.get("metrics") or {})
            if isinstance((sample.get("metrics") or {}).get(name), (int, float))
        }
    )
    return {
        name: sum(float(sample["metrics"][name]) for sample in samples if name in sample["metrics"])
        / sum(name in sample["metrics"] for sample in samples)
        for name in names
    }


def compare(workspace: Path, plan_id: str, *, actor: str) -> MechanicalComparisonReceiptV1:
    state = replay(workspace)
    plan = _find_plan(state, plan_id)
    matching = [
        receipt
        for receipt in state["receipts"].values()
        if receipt.plan_id == plan.plan_id
    ]
    baselines = [item for item in matching if item.role == "baseline"]
    candidates = [item for item in matching if item.role == "candidate"]
    if not baselines:
        raise RecordValidationError("comparison refused without a baseline")
    if not candidates:
        raise RecordValidationError("comparison refused without a candidate")
    baseline = baselines[-1]
    candidate = candidates[-1]
    baseline_mean = _mean_metrics(load_fixture(workspace, baseline))
    candidate_mean = _mean_metrics(load_fixture(workspace, candidate))
    common = sorted(set(baseline_mean).intersection(candidate_mean))
    summary: dict[str, float] = {}
    for name in common:
        summary[f"baseline.{name}"] = baseline_mean[name]
        summary[f"candidate.{name}"] = candidate_mean[name]
        summary[f"delta.{name}"] = candidate_mean[name] - baseline_mean[name]
    if not baseline.sufficient or not candidate.sufficient or not common:
        outcome = "insufficient"
    elif any(abs(summary[f"delta.{name}"]) > 1.0e-12 for name in common):
        outcome = "difference_observed"
    else:
        outcome = "no_difference_observed"
    comparison = MechanicalComparisonReceiptV1.build(
        campaign_id=plan.campaign_id,
        study_id=plan.concordance_study_id,
        plan_id=plan.plan_id,
        comparison_kind=plan.comparison_kind,
        baseline_receipt_id=baseline.receipt_id,
        candidate_receipt_id=candidate.receipt_id,
        outcome=outcome,
        metric_summary=summary,
    )
    existing = state["comparisons"].get(comparison.comparison_id)
    if existing is None:
        append_event(
            workspace, "comparison_recorded", comparison.to_dict(), actor
        )
    else:
        comparison = existing
    _advance_comparison_concordance(
        workspace, state, plan, comparison, actor=actor
    )
    if outcome != "insufficient":
        from experiment_dossiers import transition

        try:
            transition(
                workspace,
                plan.dossier_id,
                "comparison-ready",
                comparison.comparison_id,
                None,
            )
        except ValueError as error:
            if "invalid dossier transition" not in str(error):
                raise
    return comparison


def _reviewable_concordance_states(
    prior: list[StudyReviewReceiptV1],
) -> set[str]:
    states = {StudyStateV1.COMPARISON_READY.value}
    if prior and prior[-1].outcome in {
        ReviewOutcomeV1.SMOOTH_FRICTION_REMAINS.value,
        ReviewOutcomeV1.CONTRADICTED.value,
    }:
        states.add(StudyStateV1.RESULT_RECORDED.value)
    return states


def record_review(
    workspace: Path,
    *,
    campaign_id: str,
    study_id: str,
    comparison_id: str,
    outcome: str,
    source_ref: str,
    source_field_refs: list[str] | None = None,
    actor: str,
) -> StudyReviewReceiptV1:
    state = replay(workspace)
    comparison = state["comparisons"].get(comparison_id)
    campaign = state["campaigns"].get(campaign_id)
    plan = state["plans"].get(study_id)
    if comparison is None or comparison.study_id != study_id:
        raise RecordValidationError("review requires a matching mechanical comparison")
    if campaign is None or study_id not in campaign.study_ids:
        raise RecordValidationError("review requires matching campaign membership")
    if plan is None or plan.campaign_id != campaign_id:
        raise RecordValidationError("review study has no current campaign plan")
    review = StudyReviewReceiptV1.build(
        campaign_id=campaign_id,
        study_id=study_id,
        comparison_id=comparison_id,
        outcome=outcome,
        source_ref=source_ref,
        source_field_refs=source_field_refs or [],
        opportunity_completed=True,
    )
    existing = state["reviews"].get(review.review_id)
    if existing is not None and existing != review:
        raise RecordValidationError("review identity was redefined")
    other_reviews = [
        item
        for item in state["reviews"].values()
        if item.campaign_id == campaign_id
        and item.review_id != review.review_id
    ]
    validate_review_admission(campaign, other_reviews, review)
    study_prior = [
        item for item in other_reviews if item.study_id == study_id
    ]
    if review.outcome == ReviewOutcomeV1.NO_RESPONSE.value:
        if existing is None:
            append_event(
                workspace, "review_recorded", review.to_dict(), actor
            )
        return existing or review

    _advance_comparison_concordance(
        workspace, state, plan, comparison, actor=actor
    )
    studies, observations, results, _, errors = replay_concordance(workspace)
    study = studies.get(study_id)
    baseline_observations = [
        item
        for item in observations.values()
        if item.study_id == study_id and item.role == "baseline"
    ]
    candidate_observations = [
        item
        for item in observations.values()
        if item.study_id == study_id and item.role == "candidate"
    ]
    if errors or study is None:
        raise RecordValidationError("felt review requires valid Concordance")
    if not baseline_observations or not candidate_observations:
        raise RecordValidationError(
            "felt review requires baseline and candidate observations"
        )
    baseline = baseline_observations[-1]
    candidate = candidate_observations[-1]
    result = ConcordanceResultV2.build(
        study_id=study_id,
        baseline_observation_id=baseline.observation_id,
        candidate_observation_id=candidate.observation_id,
        outcome=review.outcome,
        felt_source_ref=review.source_ref,
    )
    existing_result = results.get(result.result_id)
    if (
        study.state not in _reviewable_concordance_states(study_prior)
        and existing_result is None
    ):
        raise RecordValidationError(
            "felt review requires comparison-ready Concordance or an "
            "explicit named-friction follow-up"
        )

    if existing is None:
        append_event(workspace, "review_recorded", review.to_dict(), actor)
    if existing_result is None:
        append_concordance_event(
            workspace, "result_recorded", result.to_dict(), actor
        )
    is_follow_up = bool(study_prior)
    if not is_follow_up and study.state == StudyStateV1.COMPARISON_READY.value:
        result_state = ConcordanceStudyV1.build(
            moment=study.moment,
            intervention_signature_sha256=study.intervention_signature_sha256,
            dossier_id=study.dossier_id,
            state=StudyStateV1.RESULT_RECORDED.value,
            baseline_capture_ref=study.baseline_capture_ref,
            candidate_capture_ref=study.candidate_capture_ref,
        )
        append_concordance_event(
            workspace, "study_state_changed", result_state.to_dict(), actor
        )
    if not is_follow_up and comparison.outcome != "insufficient":
        from experiment_dossiers import transition

        transition(
            workspace,
            plan.dossier_id,
            "result-recorded",
            result.result_id,
            None,
        )
        transition(
            workspace,
            plan.dossier_id,
            "review-pending",
            review.review_id,
            None,
        )
    return existing or review
