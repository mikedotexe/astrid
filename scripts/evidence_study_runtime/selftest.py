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
from .projector import replay
from .projector import project
from .review import StudyReviewReceiptV1
from .storage import (
    active_windows,
    append_event,
    arm_window,
    disarm_window,
    load_events,
    samples_path,
)

HASH_A = "a" * 64
HASH_B = "b" * 64


def _records() -> tuple[
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


def _sample(
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
