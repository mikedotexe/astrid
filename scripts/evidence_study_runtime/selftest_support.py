"""Shared fixtures for evidence-study runtime self-tests."""

from __future__ import annotations

try:
    from experiential_systems.common import authority_state
except ModuleNotFoundError:
    from scripts.experiential_systems.common import authority_state

from .model import (
    EvidenceStudyCampaignV1,
    EvidenceStudyPlanV1,
    StudyWindowReceiptV1,
    StudyWindowSpecV1,
)

HASH_A = "a" * 64
HASH_B = "b" * 64


def records() -> tuple[
    EvidenceStudyCampaignV1,
    EvidenceStudyPlanV1,
    StudyWindowSpecV1,
    StudyWindowReceiptV1,
    StudyWindowReceiptV1,
]:
    campaign = EvidenceStudyCampaignV1.build(
        campaign_key="fixture",
        comparison_domain="mechanical_fixture",
        study_ids=["concordance_fixture"],
        review_opportunity_limit=2,
    )
    plan = EvidenceStudyPlanV1.build(
        plan_version=1,
        frozen_prior_plan_sha256=None,
        campaign_id=campaign.campaign_id,
        concordance_study_id="concordance_fixture",
        canonical_claim_id="introspection_fixture:c001",
        dossier_id="dossier_fixture",
        witness_id="lsw_fixture",
        sample_kind="telemetry",
        comparison_kind="observational_context",
        baseline_cohort="clear",
        candidate_cohort="delayed",
        metric_names=["integration_us"],
        thresholds={"wait_ms": 5.0},
        minimum_total_samples=2,
        minimum_baseline_samples=1,
        minimum_candidate_samples=1,
        duration_minutes=30,
        sample_limit=32,
        extension_limit=1,
        intervention_signature_sha256=HASH_A,
    )
    spec = StudyWindowSpecV1.build(
        campaign_id=campaign.campaign_id,
        study_id=plan.concordance_study_id,
        plan_id=plan.plan_id,
        plan_sha256=plan.plan_sha256,
        sample_kinds=["telemetry"],
        started_at_unix_ms=1_000,
        expires_at_unix_ms=2_000,
        sample_limit=32,
        actor="test",
    )
    baseline = StudyWindowReceiptV1.build(
        window_id=spec.window_id,
        campaign_id=campaign.campaign_id,
        study_id=plan.concordance_study_id,
        plan_id=plan.plan_id,
        role="baseline",
        comparison_kind=plan.comparison_kind,
        cohort=plan.baseline_cohort,
        sample_count=1,
        qualifying_sample_count=1,
        sample_set_sha256=HASH_A,
        scalar_fixture_ref="scalar_fixtures/a.json",
        scalar_fixture_sha256=HASH_A,
        process_identity_sha256=HASH_A,
        deployment_identity_sha256=HASH_B,
        identity_relation="exact_identity",
        gap_refs=[],
        sufficient=True,
    )
    candidate = StudyWindowReceiptV1.build(
        window_id=spec.window_id,
        campaign_id=campaign.campaign_id,
        study_id=plan.concordance_study_id,
        plan_id=plan.plan_id,
        role="candidate",
        comparison_kind=plan.comparison_kind,
        cohort=plan.candidate_cohort,
        sample_count=1,
        qualifying_sample_count=1,
        sample_set_sha256=HASH_B,
        scalar_fixture_ref="scalar_fixtures/b.json",
        scalar_fixture_sha256=HASH_B,
        process_identity_sha256=HASH_A,
        deployment_identity_sha256=HASH_B,
        identity_relation="exact_identity",
        gap_refs=[],
        sufficient=True,
    )
    return campaign, plan, spec, baseline, candidate


def sample(
    window_id: str,
    *,
    sample_id: str,
    classification: str,
    process_hash: str = HASH_A,
    deployment_hash: str = HASH_B,
    observed_at_unix_ms: int = 1_500,
) -> dict[str, object]:
    return {
        "schema": "telemetry_study_sample_v1",
        "schema_version": 1,
        "sample_id": sample_id,
        "window_id": window_id,
        "sample_kind": "telemetry",
        "classification": classification,
        "connection_id": 7,
        "telemetry_t_ms": 42,
        "observed_at_unix_ms": observed_at_unix_ms,
        "monotonic_time_ns": 500,
        "process_identity_sha256": process_hash,
        "deployment_identity_sha256": deployment_hash,
        "metrics": {
            "integration_us": 10.0,
            "prewrite_us": 2.0,
            "write_lock_wait_us": 3.0,
            "write_lock_hold_us": 5.0,
        },
        "timing_establishes_causation": False,
        "raw_prose_included": False,
        "artifact_authority_state_v1": authority_state(),
    }
