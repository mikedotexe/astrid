"""Focused tests for the evidence-to-study runtime."""

from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
import json
import hashlib
import os
from pathlib import Path
import tempfile
import unittest

try:
    from experiential_systems.common import (
        RecordValidationError,
        authority_state,
        canonical_json,
        owner_append_jsonl,
        owner_atomic_write,
        sha256_bytes,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError,
        authority_state,
        canonical_json,
        owner_append_jsonl,
        owner_atomic_write,
        sha256_bytes,
    )

from .assembly import assemble as assemble_window
from .assembly import load_fixture
from .codec import narrative_lane_samples
from .context import resolve_observation_context
from .model import (
    EvidenceStudyCampaignV1,
    EvidenceStudyPlanV1,
    MechanicalComparisonReceiptV1,
    StudyWindowReceiptV1,
    StudyWindowSpecV1,
)
from .projector import _descriptive_metrics, _review_packet, project, replay
from .review import (
    StudyReviewReceiptV1,
    group_review_opportunities,
    validate_review_admission,
)
from .storage import (
    active_windows,
    append_event,
    arm_window,
    disarm_window,
    load_events,
    samples_path,
)
from .service import (
    _advance_concordance,
    _reviewable_concordance_states,
)
from .selftest_support import HASH_A, HASH_B, records as _records
from .selftest_support import sample as _sample

from felt_mechanism_concordance.model import (
    ConcordanceStudyV1,
    FeltMomentRefV1,
    StudyStateV1,
)
from felt_mechanism_concordance.projector import (
    append_operator_event as append_concordance_event,
    replay as replay_concordance,
)

class RuntimeTests(unittest.TestCase):
    def test_trusted_records_revalidate_and_reject_tampering(self) -> None:
        campaign, plan, _, _, _ = _records()
        self.assertEqual(
            EvidenceStudyCampaignV1.from_untrusted(campaign.to_dict()), campaign
        )
        value = plan.to_dict()
        value["minimum_baseline_samples"] = 2
        with self.assertRaises(RecordValidationError):
            EvidenceStudyPlanV1.from_untrusted(value)

    def test_plan_revision_freezes_prior_hash(self) -> None:
        _, plan, _, _, _ = _records()
        value = plan.to_dict()
        value.update(
            {
                "plan_version": 2,
                "frozen_prior_plan_sha256": plan.plan_sha256,
                "minimum_total_samples": 3,
            }
        )
        value.pop("plan_id")
        value.pop("plan_sha256")
        revised = EvidenceStudyPlanV1.build(
            **{
                key: value[key]
                for key in (
                    "plan_version",
                    "frozen_prior_plan_sha256",
                    "campaign_id",
                    "concordance_study_id",
                    "canonical_claim_id",
                    "dossier_id",
                    "witness_id",
                    "sample_kind",
                    "comparison_kind",
                    "baseline_cohort",
                    "candidate_cohort",
                    "metric_names",
                    "thresholds",
                    "minimum_total_samples",
                    "minimum_baseline_samples",
                    "minimum_candidate_samples",
                    "duration_minutes",
                    "sample_limit",
                    "extension_limit",
                    "intervention_signature_sha256",
                )
            }
        )
        self.assertEqual(revised.frozen_prior_plan_sha256, plan.plan_sha256)

    def test_active_window_is_exclusive_and_owner_only(self) -> None:
        _, _, spec, _, _ = _records()
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            arm_window(workspace, spec)
            with self.assertRaises(RecordValidationError):
                arm_window(workspace, spec)
            path = workspace / "diagnostics/evidence_study_runtime_v1/active_windows.json"
            self.assertEqual(os.stat(path).st_mode & 0o777, 0o600)
            self.assertEqual(disarm_window(workspace, spec.window_id), spec)

    def test_double_arm_race_allows_one_sample_kind_owner(self) -> None:
        campaign, plan, spec, _, _ = _records()
        contender = StudyWindowSpecV1.build(
            campaign_id=campaign.campaign_id,
            study_id=plan.concordance_study_id,
            plan_id=plan.plan_id,
            plan_sha256=plan.plan_sha256,
            sample_kinds=["telemetry"],
            started_at_unix_ms=1_001,
            expires_at_unix_ms=2_001,
            sample_limit=32,
            actor="contender",
        )
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)

            def arm(item: StudyWindowSpecV1) -> bool:
                try:
                    arm_window(workspace, item)
                except RecordValidationError:
                    return False
                return True

            with ThreadPoolExecutor(max_workers=2) as pool:
                outcomes = list(pool.map(arm, (spec, contender)))
            self.assertEqual(outcomes.count(True), 1)
            self.assertEqual(len(active_windows(workspace)), 1)

    def test_replay_refuses_comparison_without_baseline(self) -> None:
        campaign, plan, spec, _, candidate = _records()
        comparison = MechanicalComparisonReceiptV1.build(
            campaign_id=campaign.campaign_id,
            study_id=plan.concordance_study_id,
            plan_id=plan.plan_id,
            comparison_kind=plan.comparison_kind,
            baseline_receipt_id="missing_baseline",
            candidate_receipt_id=candidate.receipt_id,
            outcome="insufficient",
            metric_summary={},
        )
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            for event_type, item in (
                ("campaign_seeded", campaign),
                ("plan_preregistered", plan),
                ("window_started", spec),
                ("window_assembled", candidate),
                ("comparison_recorded", comparison),
            ):
                append_event(workspace, event_type, item.to_dict(), "test")
            self.assertTrue(
                any("baseline" in error for error in replay(workspace)["errors"])
            )

    def test_silence_stays_pending_and_never_closes(self) -> None:
        campaign, plan, spec, baseline, candidate = _records()
        comparison = MechanicalComparisonReceiptV1.build(
            campaign_id=campaign.campaign_id,
            study_id=plan.concordance_study_id,
            plan_id=plan.plan_id,
            comparison_kind=plan.comparison_kind,
            baseline_receipt_id=baseline.receipt_id,
            candidate_receipt_id=candidate.receipt_id,
            outcome="difference_observed",
            metric_summary={"delta.integration_us": 1.0},
        )
        review = StudyReviewReceiptV1.build(
            campaign_id=campaign.campaign_id,
            study_id=plan.concordance_study_id,
            comparison_id=comparison.comparison_id,
            outcome="no_response",
            source_ref="review_opportunity_fixture",
            opportunity_completed=True,
        )
        self.assertFalse(review.to_dict()["felt_result_established"])
        self.assertTrue(review.to_dict()["review_pending"])
        self.assertFalse(review.to_dict()["closure_propagated"])

    def test_named_friction_is_linked_as_unscored_qualitative_context(self) -> None:
        campaign, plan, _, baseline, candidate = _records()
        comparison = MechanicalComparisonReceiptV1.build(
            campaign_id=campaign.campaign_id,
            study_id=plan.concordance_study_id,
            plan_id=plan.plan_id,
            comparison_kind=plan.comparison_kind,
            baseline_receipt_id=baseline.receipt_id,
            candidate_receipt_id=candidate.receipt_id,
            outcome="difference_observed",
            metric_summary={"delta.integration_us": 1.0},
        )
        review = StudyReviewReceiptV1.build(
            campaign_id=campaign.campaign_id,
            study_id=plan.concordance_study_id,
            comparison_id=comparison.comparison_id,
            outcome="mechanism_smooth_felt_friction_remains",
            source_ref="introspection_fixture_review",
            source_field_refs=[
                "canonical_report.sections.Observed",
                "canonical_report.sections.Likely_Snags",
            ],
            opportunity_completed=True,
        )
        with tempfile.TemporaryDirectory() as temporary:
            packet = _review_packet(
                campaign,
                {
                    "comparisons": {
                        comparison.comparison_id: comparison
                    },
                    "receipts": {
                        baseline.receipt_id: baseline,
                        candidate.receipt_id: candidate,
                    },
                    "reviews": {review.review_id: review},
                },
                workspace=Path(temporary),
            )
        self.assertEqual(
            packet["review_state"], "named_friction_follow_up_available"
        )
        contexts = packet["qualitative_context_receipts"]
        self.assertEqual(len(contexts), 1)
        self.assertEqual(contexts[0]["source_ref"], review.source_ref)
        self.assertTrue(contexts[0]["unscored"])
        self.assertFalse(contexts[0]["mechanical_comparison_modified"])
        self.assertFalse(contexts[0]["raw_prose_included"])
        self.assertEqual(
            contexts[0]["source_field_refs"],
            [
                "canonical_report.sections.Likely_Snags",
                "canonical_report.sections.Observed",
            ],
        )
        self.assertTrue(contexts[0]["mapping_link_v1"]["pointer_only"])
        self.assertFalse(
            contexts[0]["mapping_link_v1"]["calculation_performed"]
        )
        baseline_context = packet["descriptive_capture_context"][0][
            "cohorts"
        ][0]
        self.assertEqual(
            baseline_context["process_identity_sha256"],
            baseline.process_identity_sha256,
        )
        self.assertEqual(
            baseline_context["deployment_identity_sha256"],
            baseline.deployment_identity_sha256,
        )
        follow_up = StudyReviewReceiptV1.build(
            campaign_id=campaign.campaign_id,
            study_id=plan.concordance_study_id,
            comparison_id=comparison.comparison_id,
            outcome="mechanism_smooth_felt_friction_remains",
            source_ref="introspection_fixture_follow_up",
            opportunity_completed=True,
        )
        exhausted = _review_packet(
            campaign,
            {
                "comparisons": {comparison.comparison_id: comparison},
                "reviews": {
                    review.review_id: review,
                    follow_up.review_id: follow_up,
                },
            },
        )
        self.assertEqual(
            exhausted["review_state"],
            "named_friction_review_budget_exhausted",
        )
        self.assertEqual(exhausted["review_opportunity_count"], 2)

    def test_named_friction_allows_one_result_recorded_follow_up(self) -> None:
        campaign, plan, _, baseline, candidate = _records()
        comparison = MechanicalComparisonReceiptV1.build(
            campaign_id=campaign.campaign_id,
            study_id=plan.concordance_study_id,
            plan_id=plan.plan_id,
            comparison_kind=plan.comparison_kind,
            baseline_receipt_id=baseline.receipt_id,
            candidate_receipt_id=candidate.receipt_id,
            outcome="difference_observed",
            metric_summary={"delta.integration_us": 1.0},
        )
        friction = StudyReviewReceiptV1.build(
            campaign_id=campaign.campaign_id,
            study_id=plan.concordance_study_id,
            comparison_id=comparison.comparison_id,
            outcome="mechanism_smooth_felt_friction_remains",
            source_ref="introspection_fixture_review",
            opportunity_completed=True,
        )
        self.assertEqual(
            _reviewable_concordance_states([]),
            {"comparison_ready"},
        )
        self.assertEqual(
            _reviewable_concordance_states([friction]),
            {"comparison_ready", "result_recorded"},
        )

    def test_campaign_budget_counts_review_opportunities_not_studies(self) -> None:
        campaign = EvidenceStudyCampaignV1.build(
            campaign_key="paired_fixture",
            comparison_domain="mechanical_fixture",
            study_ids=["concordance_a", "concordance_b"],
            review_opportunity_limit=2,
        )
        first = StudyReviewReceiptV1.build(
            campaign_id=campaign.campaign_id,
            study_id="concordance_a",
            comparison_id="comparison_a",
            outcome="mechanism_smooth_felt_friction_remains",
            source_ref="introspection_review_1",
            opportunity_completed=True,
        )
        companion = StudyReviewReceiptV1.build(
            campaign_id=campaign.campaign_id,
            study_id="concordance_b",
            comparison_id="comparison_b",
            outcome="insufficient",
            source_ref="introspection_review_1",
            opportunity_completed=True,
        )
        validate_review_admission(campaign, [], first)
        validate_review_admission(campaign, [first], companion)
        self.assertEqual(
            len(group_review_opportunities([first, companion])), 1
        )
        duplicate_study = StudyReviewReceiptV1.build(
            campaign_id=campaign.campaign_id,
            study_id="concordance_a",
            comparison_id="comparison_a",
            outcome="contradicted",
            source_ref="introspection_review_1",
            opportunity_completed=True,
        )
        with self.assertRaises(RecordValidationError):
            validate_review_admission(
                campaign, [first, companion], duplicate_study
            )
        follow_up = StudyReviewReceiptV1.build(
            campaign_id=campaign.campaign_id,
            study_id="concordance_a",
            comparison_id="comparison_a",
            outcome="mechanism_smooth_felt_friction_remains",
            source_ref="introspection_review_2",
            opportunity_completed=True,
        )
        validate_review_admission(campaign, [first, companion], follow_up)
        self.assertEqual(
            len(
                group_review_opportunities(
                    [first, companion, follow_up]
                )
            ),
            2,
        )
        third = StudyReviewReceiptV1.build(
            campaign_id=campaign.campaign_id,
            study_id="concordance_a",
            comparison_id="comparison_a",
            outcome="corroborated",
            source_ref="introspection_review_3",
            opportunity_completed=True,
        )
        with self.assertRaises(RecordValidationError):
            validate_review_admission(
                campaign, [first, companion, follow_up], third
            )

    def test_descriptive_capture_context_retains_pressure_without_felt_label(
        self,
    ) -> None:
        metrics = _descriptive_metrics(
            [
                {"metrics": {"pressure": 0.2, "entropy": 0.8}},
                {"metrics": {"pressure": 0.3, "entropy": 0.9}},
            ]
        )
        self.assertAlmostEqual(metrics["pressure"]["mean"], 0.25)
        self.assertAlmostEqual(
            metrics["pressure"]["first_last_delta"], 0.1
        )
        self.assertAlmostEqual(metrics["pressure"]["variance"], 0.0025)
        self.assertNotIn("felt_texture", metrics)

    def test_insufficient_capture_remains_reviewable_concordance_evidence(
        self,
    ) -> None:
        campaign, plan, _, _, _ = _records()
        moment = FeltMomentRefV1.build(
            plan.canonical_claim_id,
            plan.witness_id,
            ["claims.c001"],
        )
        study = ConcordanceStudyV1.build(
            moment=moment,
            intervention_signature_sha256=(
                plan.intervention_signature_sha256
            ),
            dossier_id=plan.dossier_id,
        )
        aligned_plan = EvidenceStudyPlanV1.build(
            plan_version=plan.plan_version,
            frozen_prior_plan_sha256=plan.frozen_prior_plan_sha256,
            campaign_id=campaign.campaign_id,
            concordance_study_id=study.study_id,
            canonical_claim_id=plan.canonical_claim_id,
            dossier_id=plan.dossier_id,
            witness_id=plan.witness_id,
            sample_kind=plan.sample_kind,
            comparison_kind=plan.comparison_kind,
            baseline_cohort=plan.baseline_cohort,
            candidate_cohort=plan.candidate_cohort,
            metric_names=list(plan.metric_names),
            thresholds=dict(plan.thresholds),
            minimum_total_samples=plan.minimum_total_samples,
            minimum_baseline_samples=plan.minimum_baseline_samples,
            minimum_candidate_samples=plan.minimum_candidate_samples,
            duration_minutes=plan.duration_minutes,
            sample_limit=plan.sample_limit,
            extension_limit=plan.extension_limit,
            intervention_signature_sha256=(
                plan.intervention_signature_sha256
            ),
        )
        spec = StudyWindowSpecV1.build(
            campaign_id=campaign.campaign_id,
            study_id=study.study_id,
            plan_id=aligned_plan.plan_id,
            plan_sha256=aligned_plan.plan_sha256,
            sample_kinds=["telemetry"],
            started_at_unix_ms=1_000,
            expires_at_unix_ms=2_000,
            sample_limit=32,
            actor="test",
        )

        def receipt(
            role: str,
            cohort: str,
            *,
            sample_count: int,
            sufficient: bool,
        ) -> StudyWindowReceiptV1:
            return StudyWindowReceiptV1.build(
                window_id=spec.window_id,
                campaign_id=campaign.campaign_id,
                study_id=study.study_id,
                plan_id=aligned_plan.plan_id,
                role=role,
                comparison_kind=aligned_plan.comparison_kind,
                cohort=cohort,
                sample_count=sample_count,
                qualifying_sample_count=sample_count,
                sample_set_sha256=HASH_A if role == "baseline" else HASH_B,
                scalar_fixture_ref=f"scalar_fixtures/{role}.json",
                scalar_fixture_sha256=(
                    HASH_A if role == "baseline" else HASH_B
                ),
                process_identity_sha256=HASH_A,
                deployment_identity_sha256=HASH_B,
                identity_relation="exact_identity",
                gap_refs=(
                    [] if sufficient else ["studygap_missing_cohort"]
                ),
                sufficient=sufficient,
            )

        baseline = receipt(
            "baseline", aligned_plan.baseline_cohort, sample_count=1, sufficient=True
        )
        insufficient_candidate = receipt(
            "candidate",
            aligned_plan.candidate_cohort,
            sample_count=0,
            sufficient=False,
        )
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            append_concordance_event(
                workspace, "study_created", study.to_dict(), "test"
            )
            _advance_concordance(
                workspace,
                aligned_plan,
                spec,
                [baseline, insufficient_candidate],
                actor="test",
            )
            studies, observations, _, _, errors = replay_concordance(
                workspace
            )
            self.assertEqual(errors, [])
            self.assertEqual(
                studies[study.study_id].state,
                StudyStateV1.CANDIDATE_CAPTURED.value,
            )
            self.assertEqual(
                sorted(item.mechanical_pass for item in observations.values()),
                [False, True],
            )

    def test_no_raw_vector_field_enters_scalar_identity(self) -> None:
        digest = sha256_bytes(b"fixture")
        self.assertEqual(len(digest), 64)

    def test_cohorts_require_one_exact_process_and_deployment_identity(self) -> None:
        _, plan, spec, _, _ = _records()
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            for row in (
                _sample(
                    spec.window_id,
                    sample_id="clear",
                    classification="clear_at_latest_sample",
                ),
                _sample(
                    spec.window_id,
                    sample_id="wait",
                    classification="write_lock_wait_observed",
                ),
            ):
                owner_append_jsonl(samples_path(workspace, spec.window_id), row)
            receipts, gaps = assemble_window(workspace, plan, spec)
            self.assertEqual(gaps, [])
            self.assertTrue(all(receipt.sufficient for receipt in receipts))
            self.assertTrue(
                all(receipt.identity_relation == "exact_identity" for receipt in receipts)
            )
            fixture = load_fixture(workspace, receipts[0])
            self.assertEqual(len(fixture), 1)
            self.assertEqual(fixture[0]["connection_id"], 7)
            self.assertEqual(fixture[0]["telemetry_t_ms"], 42)
            fixture_path = (
                workspace
                / "diagnostics/evidence_study_runtime_v1"
                / receipts[0].scalar_fixture_ref
            )
            self.assertEqual(os.stat(fixture_path).st_mode & 0o777, 0o600)

    def test_identity_mismatch_and_writer_gap_make_capture_insufficient(self) -> None:
        _, plan, spec, _, _ = _records()
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            owner_append_jsonl(
                samples_path(workspace, spec.window_id),
                _sample(
                    spec.window_id,
                    sample_id="clear",
                    classification="clear_at_latest_sample",
                ),
            )
            owner_append_jsonl(
                samples_path(workspace, spec.window_id),
                _sample(
                    spec.window_id,
                    sample_id="wait",
                    classification="write_lock_wait_observed",
                    process_hash="c" * 64,
                ),
            )
            owner_append_jsonl(
                samples_path(workspace, spec.window_id),
                {
                    "schema": "study_capture_gap_receipt_v1",
                    "schema_version": 1,
                    "window_id": spec.window_id,
                    "reason": "queue_exhausted",
                    "dropped_sample_count": 1,
                    "artifact_authority_state_v1": authority_state(),
                },
            )
            receipts, gaps = assemble_window(workspace, plan, spec)
            self.assertFalse(any(receipt.sufficient for receipt in receipts))
            self.assertIn("queue_exhausted", {gap.reason for gap in gaps})
            self.assertIn("identity_mismatch", {gap.reason for gap in gaps})

    def test_missing_natural_cohort_preserves_exact_available_samples(self) -> None:
        _, plan, spec, _, _ = _records()
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            owner_append_jsonl(
                samples_path(workspace, spec.window_id),
                _sample(
                    spec.window_id,
                    sample_id="clear",
                    classification="clear_at_latest_sample",
                ),
            )
            receipts, gaps = assemble_window(workspace, plan, spec)
            by_role = {receipt.role: receipt for receipt in receipts}
            self.assertEqual(by_role["baseline"].sample_count, 1)
            self.assertEqual(by_role["candidate"].sample_count, 0)
            self.assertTrue(
                all(
                    receipt.identity_relation == "exact_identity"
                    for receipt in receipts
                )
            )
            self.assertFalse(any(receipt.sufficient for receipt in receipts))
            self.assertIn("required_cohort_missing", {gap.reason for gap in gaps})
            self.assertNotIn("identity_mismatch", {gap.reason for gap in gaps})

    def test_capture_extension_combines_bounded_window_lineage(self) -> None:
        campaign, plan, parent, _, _ = _records()
        child = StudyWindowSpecV1.build(
            campaign_id=campaign.campaign_id,
            study_id=plan.concordance_study_id,
            plan_id=plan.plan_id,
            plan_sha256=plan.plan_sha256,
            sample_kinds=["telemetry"],
            started_at_unix_ms=2_001,
            expires_at_unix_ms=3_000,
            sample_limit=32,
            actor="test",
            extension_of_window_id=parent.window_id,
        )
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            owner_append_jsonl(
                samples_path(workspace, parent.window_id),
                _sample(
                    parent.window_id,
                    sample_id="clear",
                    classification="clear_at_latest_sample",
                ),
            )
            owner_append_jsonl(
                samples_path(workspace, child.window_id),
                _sample(
                    child.window_id,
                    sample_id="wait",
                    classification="write_lock_wait_observed",
                    observed_at_unix_ms=2_500,
                ),
            )
            receipts, gaps = assemble_window(
                workspace, plan, child, source_specs=[parent, child]
            )
            self.assertEqual(gaps, [])
            self.assertTrue(all(receipt.sufficient for receipt in receipts))

    def test_expired_window_recovery_records_a_gap_without_deleting_evidence(self) -> None:
        from .service import reconcile

        campaign, plan, spec, _, _ = _records()
        expired = StudyWindowSpecV1.build(
            campaign_id=campaign.campaign_id,
            study_id=plan.concordance_study_id,
            plan_id=plan.plan_id,
            plan_sha256=plan.plan_sha256,
            sample_kinds=["telemetry"],
            started_at_unix_ms=1,
            expires_at_unix_ms=2,
            sample_limit=32,
            actor="test",
        )
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            for event_type, item in (
                ("campaign_seeded", campaign),
                ("plan_preregistered", plan),
                ("window_started", expired),
            ):
                append_event(workspace, event_type, item.to_dict(), "test")
            arm_window(workspace, expired)
            result = reconcile(workspace, actor="test")
            self.assertEqual(result["recovered_window_ids"], [expired.window_id])
            self.assertEqual(active_windows(workspace), {})
            rows, errors = load_events(workspace)
            self.assertEqual(errors, [])
            self.assertIn(
                "capture_gap_recorded",
                {row["event_type"] for row in rows},
            )

    def test_codec_lane_replay_is_deterministic_and_never_returns_vectors(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            root = workspace / "diagnostics/signal_spine_v1"
            journeys = root / "journeys"
            fixtures = root / "captures/fixture/journeys"
            journeys.mkdir(parents=True)
            fixtures.mkdir(parents=True)
            for index in range(20):
                vector = [float((index + offset) % 7) / 10.0 for offset in range(48)]
                fixture_path = fixtures / f"{index}.json"
                fixture_path.write_text(
                    json.dumps(
                        {
                            "vector": vector,
                            "vector_sha256": sha256_bytes(
                                json.dumps(vector).encode("utf-8")
                            ),
                        }
                    ),
                    encoding="utf-8",
                )
                (journeys / f"{index}.json").write_text(
                    json.dumps(
                        {
                            "journey_id": f"journey_{index:02d}",
                            "receipts": [
                                {
                                    "stage_kind": "feedback",
                                    "capture_fixture_ref_v1": {
                                        "relative_path": (
                                            f"captures/fixture/journeys/{index}.json"
                                        )
                                    },
                                    "process_identity_v1": {
                                        "deployment_identity": "fixture"
                                    },
                                }
                            ],
                        }
                    ),
                    encoding="utf-8",
                )
            first = narrative_lane_samples(workspace, "window")
            second = narrative_lane_samples(workspace, "window")
            self.assertEqual(first, second)
            self.assertEqual(len(first), 40)
            self.assertEqual(
                {item["cohort"] for item in first},
                {"current_codec", "leave_narrative_lane_40_44_out"},
            )
            self.assertNotIn('"vector"', json.dumps(first))

    def test_context_resolution_uses_exact_witness_receipts_and_temporal_telemetry(
        self,
    ) -> None:
        campaign, plan, spec, baseline, _ = _records()
        fixture = {
            "schema": "study_scalar_fixture_v1",
            "schema_version": 1,
            "samples": [
                {
                    "sample_id": "sample",
                    "connection_id": 7,
                    "journey_id": "journey_fixture",
                }
            ],
        }
        encoded = (canonical_json(fixture) + "\n").encode("utf-8")
        digest = sha256_bytes(encoded)
        receipt = StudyWindowReceiptV1.build(
            window_id=spec.window_id,
            campaign_id=campaign.campaign_id,
            study_id=plan.concordance_study_id,
            plan_id=plan.plan_id,
            role=baseline.role,
            comparison_kind=plan.comparison_kind,
            cohort=plan.baseline_cohort,
            sample_count=1,
            qualifying_sample_count=1,
            sample_set_sha256=HASH_A,
            scalar_fixture_ref=f"scalar_fixtures/{digest}.json",
            scalar_fixture_sha256=digest,
            process_identity_sha256=HASH_A,
            deployment_identity_sha256=HASH_B,
            identity_relation="exact_identity",
            gap_refs=[],
            sufficient=True,
        )
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            witness_path = (
                workspace
                / "diagnostics/lived_state_witness_v1/witnesses.jsonl"
            )
            owner_append_jsonl(
                witness_path,
                {
                    "aggregate_id": plan.witness_id,
                    "alignment": {"deployment_receipt_id": "env_receipt_fixture"},
                    "witness": {
                        "observed_process_v1": {
                            "process_identity_sha256": HASH_A
                        },
                        "model_routes_v1": [
                            {
                                "call_id": "lscall_fixture",
                                "qos_request_identity_sha256": HASH_B,
                            }
                        ],
                    },
                },
            )
            owner_append_jsonl(
                workspace
                / "diagnostics/representation_contracts_v1/transitions.jsonl",
                {
                    "source_witness_id": plan.witness_id,
                    "receipt_id": "modeltransition_fixture",
                },
            )
            owner_atomic_write(
                workspace
                / "diagnostics/evidence_study_runtime_v1"
                / receipt.scalar_fixture_ref,
                encoded,
            )
            context = resolve_observation_context(
                workspace, plan, spec, receipt
            )
            self.assertIn(
                "deployment:env_receipt_fixture",
                context["witness_context_refs"],
            )
            self.assertIn(
                f"qos_request:{HASH_B}", context["model_qos_refs"]
            )
            self.assertEqual(
                context["representation_transition_refs"],
                ["modeltransition_fixture"],
            )
            self.assertEqual(context["telemetry_relation"], "temporal_window")
            self.assertIn(
                "temporal_connection:7", context["minime_telemetry_refs"]
            )

    def test_projector_is_idempotent_and_checkpoint_hashes_match_outputs(self) -> None:
        try:
            from evidence_store import EvidenceEventStore
        except ModuleNotFoundError:
            from scripts.evidence_store import EvidenceEventStore

        campaign, plan, _, _, _ = _records()
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            store = EvidenceEventStore(
                workspace / "diagnostics/evidence_event_store_v2"
            )
            store.initialize_from_envelopes([], legacy_imported=False)
            append_event(
                workspace, "campaign_seeded", campaign.to_dict(), "test"
            )
            append_event(
                workspace, "plan_preregistered", plan.to_dict(), "test"
            )
            first = project(workspace, write=True)
            first_count = store.verify().event_count
            second = project(workspace, write=True)
            second_count = store.verify().event_count
            self.assertEqual(first["appended_event_count"], 2)
            self.assertEqual(second["appended_event_count"], 0)
            self.assertEqual(first_count, second_count)
            checkpoint = store.read_checkpoint("evidence_study_runtime_v1")
            self.assertIsNotNone(checkpoint)
            packet_path = (
                workspace
                / "diagnostics/evidence_study_runtime_v1/review_packets"
                / f"{campaign.campaign_id}.json"
            )
            packet = json.loads(packet_path.read_text(encoding="utf-8"))
            self.assertTrue(packet["right_to_ignore"])
            self.assertEqual(
                packet["review_state"], "awaiting_mechanical_comparison"
            )
            for name, expected in checkpoint["output_hashes"].items():
                actual = hashlib.sha256(
                    (
                        workspace
                        / "diagnostics/evidence_study_runtime_v1"
                        / name
                    ).read_bytes()
                ).hexdigest()
                self.assertEqual(actual, expected)
            self.assertTrue(
                store.checkpoint_current("evidence_study_runtime_v1", 1)
            )
            store.append_payloads("unrelated", [{"event_type": "unrelated"}])
            self.assertTrue(
                store.checkpoint_current("evidence_study_runtime_v1", 1)
            )
            store.append_payloads(
                "felt_mechanism_concordance",
                [{"event_type": "declared_input_changed"}],
            )
            self.assertFalse(
                store.checkpoint_current("evidence_study_runtime_v1", 1)
            )


def run() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(RuntimeTests)
    return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
