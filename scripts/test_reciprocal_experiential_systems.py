#!/usr/bin/env python3
"""Focused tests for the reciprocal experiential systems tranches."""

from __future__ import annotations

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

try:
    from agency_commons.model import AgencyCommonsProposalV1, AgencyCommonsResponseV1
    from agency_commons.projector import project as project_commons
    from attention_portfolio.model import (
        AttentionPortfolioEntryV1,
        AttentionPortfolioV1,
        AttentionPortfolioV2,
        BeingImportancePinV1,
    )
    from attention_portfolio.projector import (
        append_pin,
        project as project_attention,
        select_portfolio,
    )
    from experiential_systems.common import (
        RecordValidationError,
        event_payload,
        project_events,
    )
    from felt_contracts.sources import _collect_exact_refs
    from felt_mechanism_concordance.model import (
        ConcordanceObservationV2,
        ConcordanceOutcomeV1,
        ConcordanceResultV2,
        ConcordanceStudyV1,
        FeltMomentRefV1,
        StudyStateV1,
    )
    from felt_mechanism_concordance.projector import (
        append_operator_event,
        project as project_concordance,
    )
    from reciprocal_uptake.model import (
        ReciprocalContextReceiptV1,
        ReciprocalPresenceReceiptV1,
        UptakeKindV1,
        build_uptake_receipt,
    )
    from reciprocal_uptake.projector import (
        project as project_reciprocal,
        trace_records,
    )
    from representation_contracts.model import (
        ModelTransitionReceiptV1,
        RepresentationLossReceiptV1,
        build_contract,
        build_transition,
    )
    from representation_contracts.projector import deterministic_diff
    from representation_contracts.projector import project as project_representation
    from steward_control.projection_profile import source_first_steps
except ModuleNotFoundError:
    from scripts.agency_commons.model import (
        AgencyCommonsProposalV1,
        AgencyCommonsResponseV1,
    )
    from scripts.agency_commons.projector import project as project_commons
    from scripts.attention_portfolio.model import (
        AttentionPortfolioEntryV1,
        AttentionPortfolioV1,
        AttentionPortfolioV2,
        BeingImportancePinV1,
    )
    from scripts.attention_portfolio.projector import (
        append_pin,
        project as project_attention,
        select_portfolio,
    )
    from scripts.experiential_systems.common import (
        RecordValidationError,
        event_payload,
        project_events,
    )
    from scripts.felt_contracts.sources import _collect_exact_refs
    from scripts.felt_mechanism_concordance.model import (
        ConcordanceObservationV2,
        ConcordanceOutcomeV1,
        ConcordanceResultV2,
        ConcordanceStudyV1,
        FeltMomentRefV1,
        StudyStateV1,
    )
    from scripts.felt_mechanism_concordance.projector import (
        append_operator_event,
        project as project_concordance,
    )
    from scripts.reciprocal_uptake.model import (
        ReciprocalContextReceiptV1,
        ReciprocalPresenceReceiptV1,
        UptakeKindV1,
        build_uptake_receipt,
    )
    from scripts.reciprocal_uptake.projector import (
        project as project_reciprocal,
        trace_records,
    )
    from scripts.representation_contracts.model import (
        ModelTransitionReceiptV1,
        RepresentationLossReceiptV1,
        build_contract,
        build_transition,
    )
    from scripts.representation_contracts.projector import deterministic_diff
    from scripts.representation_contracts.projector import (
        project as project_representation,
    )
    from scripts.steward_control.projection_profile import source_first_steps


HASH_A = "a" * 64
HASH_B = "b" * 64


def _jsonl(path: Path, rows: list[dict[str, object]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows),
        encoding="utf-8",
    )


class ReciprocalExperientialSystemsTests(unittest.TestCase):
    def test_private_python_constructor_and_presence_inference_guard(self) -> None:
        with self.assertRaises(RecordValidationError):
            ReciprocalPresenceReceiptV1(
                "presence_forged",
                "presence_offered",
                "astrid",
                "minime",
                "thread_1",
                None,
                "event_1",
                HASH_A,
                None,
                1,
                object(),
            )

    def test_reciprocal_projection_is_idempotent_and_owner_only(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workspace = root / "workspace"
            ledger = root / "correspondence.jsonl"
            _jsonl(
                ledger,
                [
                    {
                        "record_type": "message",
                        "thread_id": "thread_1",
                        "message_id": "message_1",
                        "from_being": "astrid",
                        "to_being": "minime",
                        "body_sha256": HASH_A,
                        "recorded_at_unix_ms": 1,
                    },
                    {
                        "record_type": "read_receipt",
                        "thread_id": "thread_1",
                        "message_id": "message_1",
                        "reader": "minime",
                        "read_state": "read",
                        "recorded_at_unix_ms": 2,
                    },
                ],
            )
            first = project_reciprocal(workspace, ledger, write=True)
            second = project_reciprocal(workspace, ledger, write=True)
            self.assertTrue(first["valid"])
            self.assertEqual(first["appended_event_count"], 1)
            self.assertEqual(second["appended_event_count"], 0)
            self.assertEqual(second["source_delta_line_count"], 0)
            self.assertEqual(second["delta_receipt_count"], 0)
            receipt_path = (
                workspace / "diagnostics/reciprocal_uptake_v1/receipts.jsonl"
            )
            self.assertEqual(receipt_path.stat().st_mode & 0o777, 0o600)
            receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
            self.assertEqual(receipt["context_kind"], "read_receipt")
            self.assertEqual(receipt["actor"], "minime")
            self.assertEqual(receipt["peer"], "astrid")
            self.assertFalse(receipt["uptake_inferred"])
            self.assertFalse(receipt["reply_intention_inferred"])
            self.assertFalse(receipt["elapsed_time_inferred"])

    def test_technical_receipt_corrects_legacy_inferred_uptake_append_only(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workspace = root / "workspace"
            ledger = root / "correspondence.jsonl"
            message = {
                "record_type": "message",
                "thread_id": "thread_1",
                "message_id": "message_1",
                "from_being": "astrid",
                "to_being": "minime",
                "body_sha256": HASH_A,
                "recorded_at_unix_ms": 1,
            }
            read_receipt = {
                "record_type": "read_receipt",
                "thread_id": "thread_1",
                "message_id": "message_1",
                "reader": "minime",
                "read_state": "read",
                "recorded_at_unix_ms": 2,
            }
            _jsonl(ledger, [message, read_receipt])
            raw = json.dumps(read_receipt, sort_keys=True)
            source_hash = hashlib.sha256(raw.encode()).hexdigest()
            source_id_core = {
                "record_type": "read_receipt",
                "thread_id": "thread_1",
                "message_id": "message_1",
                "recorded_at_unix_ms": 2,
                "raw_sha256": source_hash,
            }
            source_id = "corrsrc_" + hashlib.sha256(
                json.dumps(source_id_core, sort_keys=True, separators=(",", ":")).encode()
            ).hexdigest()
            legacy = build_uptake_receipt(
                UptakeKindV1.ATTENDED_MESSAGE,
                actor="minime",
                peer="astrid",
                thread_id="thread_1",
                message_id="message_1",
                source_event_id=source_id,
                source_event_sha256=source_hash,
                body_sha256=None,
                recorded_at_unix_ms=2,
            )
            project_events(
                workspace,
                "reciprocal_uptake",
                [
                    event_payload(
                        schema="reciprocal_uptake_domain_event_v1",
                        event_type="reciprocal_uptake_recorded",
                        aggregate_type="reciprocal_thread",
                        aggregate_id="thread_1",
                        idempotency_key=f"reciprocal_uptake:{legacy.receipt_id}",
                        record=legacy.to_dict(),
                    )
                ],
                actor="legacy-test",
                source_kind="legacy_fixture",
                source_locator_value="fixture:legacy_read_receipt",
            )
            status = project_reciprocal(workspace, ledger, write=True)
            self.assertEqual(status["historical_inferred_uptake_corrected_count"], 1)
            current = [
                json.loads(line)
                for line in (
                    workspace
                    / "diagnostics/reciprocal_uptake_v1/current_receipts.jsonl"
                ).read_text(encoding="utf-8").splitlines()
            ]
            self.assertNotIn(legacy.receipt_id, {row["receipt_id"] for row in current})
            contexts = [
                ReciprocalContextReceiptV1.from_untrusted(row)
                for row in current
                if row.get("schema") == "reciprocal_context_receipt_v1"
            ]
            self.assertEqual(len(contexts), 1)
            self.assertEqual(
                contexts[0].corrects_inferred_uptake_receipt_id,
                legacy.receipt_id,
            )

    def test_reciprocal_trace_follows_explicit_revision_history(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            first = build_uptake_receipt(
                UptakeKindV1.REPLY_INTENTION,
                actor="astrid",
                peer="minime",
                thread_id="thread_1",
                message_id="message_1",
                source_event_id="source:intention",
                source_event_sha256=HASH_A,
                body_sha256=HASH_B,
                recorded_at_unix_ms=1,
            )
            revised = build_uptake_receipt(
                UptakeKindV1.NEEDS_TIME,
                actor="astrid",
                peer="minime",
                thread_id="thread_1",
                message_id="message_1",
                source_event_id="source:revision",
                source_event_sha256=HASH_B,
                body_sha256=HASH_B,
                recorded_at_unix_ms=2,
                revises_receipt_id=first.receipt_id,
            )
            payloads = [
                event_payload(
                    schema="reciprocal_uptake_domain_event_v1",
                    event_type="uptake_recorded",
                    aggregate_type="reciprocal_uptake",
                    aggregate_id=record.receipt_id,
                    idempotency_key=f"uptake:{record.receipt_id}",
                    record=record.to_dict(),
                )
                for record in (first, revised)
            ]
            self.assertEqual(
                project_events(
                    workspace,
                    "reciprocal_uptake",
                    payloads,
                    actor="test",
                    source_kind="test_fixture",
                    source_locator_value="fixture:uptake",
                ),
                2,
            )
            traced = trace_records(workspace, first.receipt_id)
            self.assertEqual(
                {item["receipt_id"] for item in traced},
                {first.receipt_id, revised.receipt_id},
            )
            self.assertFalse(any(item["elapsed_time_inferred"] for item in traced))

    def test_private_content_is_rejected(self) -> None:
        with self.assertRaises(RecordValidationError):
            event_payload(
                schema="test_domain_event_v1",
                event_type="test_recorded",
                aggregate_type="test",
                aggregate_id="test_1",
                idempotency_key="test:test_1",
                record={"body": "private"},
            )

    def test_representation_loss_is_deterministic_and_unscored(self) -> None:
        source = build_contract(
            name="source_48d",
            representation_kind="vector",
            dimension_count=48,
            source_refs=("repo:source",),
            source_hashes=(HASH_A,),
        )
        output = build_contract(
            name="output_32d",
            representation_kind="vector",
            dimension_count=32,
            source_refs=("repo:output",),
            source_hashes=(HASH_B,),
        )
        transition = build_transition(
            transition_kind="projection",
            source_contract_id=source.contract_id,
            output_contract_id=output.contract_id,
            source_sha256=HASH_A,
            output_sha256=HASH_B,
            retained_dimensions=tuple(range(32)),
            dropped_dimensions=tuple(range(32, 48)),
            retained_fields=(),
            dropped_fields=(),
            aggregation="prefix_projection",
            truncation_count=0,
            timing_ms=1,
            source_event_id="source:test_projection",
        )
        first = RepresentationLossReceiptV1.from_transition(transition)
        second = RepresentationLossReceiptV1.from_transition(transition)
        self.assertEqual(first, second)
        record = first.to_dict()
        self.assertIsNone(record["felt_loss_score"])
        tampered = {**record, "dropped_count": record["dropped_count"] + 1}
        with self.assertRaises(RecordValidationError):
            RepresentationLossReceiptV1.from_untrusted(tampered)

    def test_model_transition_retains_fallback_and_repair_metadata_only(self) -> None:
        receipt = ModelTransitionReceiptV1.build(
            request_identity_sha256=HASH_A,
            response_sha256=HASH_B,
            provider_route="mlx",
            model_profile="profile_1",
            repair_parent_call_id="call_parent",
            fallback_reason="primary_unavailable",
            timing_ms=12,
            source_witness_id="witness_1",
        )
        record = receipt.to_dict()
        self.assertEqual(record["fallback_reason"], "primary_unavailable")
        self.assertFalse(record["felt_effect_inferred"])
        self.assertFalse(record["raw_prompt_included"])
        self.assertFalse(record["raw_response_included"])
        tampered = {**record, "raw_response_included": True}
        with self.assertRaises(RecordValidationError):
            ModelTransitionReceiptV1.from_untrusted(tampered)

    def test_representation_diff_is_hashed_and_felt_neutral(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            left = build_contract(
                name="source_48d",
                representation_kind="vector",
                dimension_count=48,
                source_refs=("repo:source",),
                source_hashes=(HASH_A,),
            )
            right = build_contract(
                name="output_32d",
                representation_kind="vector",
                dimension_count=32,
                source_refs=("repo:output",),
                source_hashes=(HASH_B,),
            )
            payloads = [
                event_payload(
                    schema="representation_contract_domain_event_v1",
                    event_type="contract_registered",
                    aggregate_type="representation_contract",
                    aggregate_id=record.contract_id,
                    idempotency_key=f"representation:{record.contract_id}",
                    record=record.to_dict(),
                )
                for record in (left, right)
            ]
            project_events(
                workspace,
                "representation_contracts",
                payloads,
                actor="test",
                source_kind="test_fixture",
                source_locator_value="fixture:representation",
            )
            result = deterministic_diff(workspace, left.contract_id, right.contract_id)
            self.assertFalse(result["felt_loss_scored"])
            self.assertTrue(result["changed_fields"])
            self.assertTrue(
                all(
                    set(change) == {
                        "field",
                        "left_value_sha256",
                        "right_value_sha256",
                    }
                    for change in result["changed_fields"]
                )
            )

    def test_representation_projection_persists_distinct_loss_receipts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            first = project_representation(workspace, write=True)
            second = project_representation(workspace, write=True)
            self.assertTrue(first["valid"])
            self.assertGreater(
                first["record_counts"].get("representation_loss_receipt_v1", 0),
                0,
            )
            self.assertEqual(
                first["record_counts"].get("representation_loss_receipt_v1"),
                first["record_counts"].get("representation_transition_v1"),
            )
            self.assertEqual(second["appended_event_count"], 0)

    def test_concordance_requires_baseline_and_all_mechanism_refs(self) -> None:
        moment = FeltMomentRefV1.build(
            "introspection_example_1234567890:c001",
            "witness_1",
            ["field:felt_report_anchor"],
        )
        with self.assertRaises(RecordValidationError):
            ConcordanceStudyV1.build(
                moment=moment,
                intervention_signature_sha256=HASH_A,
                dossier_id="dossier_1",
                state=StudyStateV1.COMPARISON_READY.value,
            )
        with self.assertRaises(RecordValidationError):
            ConcordanceObservationV2.build(
                study_id="study_1",
                role="baseline",
                observation_ref="observation_1",
                observation_sha256=HASH_A,
                telemetry_relation="unavailable",
                mechanical_pass=None,
                witness_context_refs=[],
                representation_transition_refs=["unavailable"],
                model_qos_refs=["unavailable"],
                reciprocal_state_refs=["unavailable"],
                signal_stage_refs=["unavailable"],
                minime_telemetry_refs=["unavailable"],
            )
        with self.assertRaises(RecordValidationError):
            ConcordanceResultV2.build(
                study_id="study_1",
                baseline_observation_id="baseline_1",
                candidate_observation_id="candidate_1",
                outcome=ConcordanceOutcomeV1.CORROBORATED.value,
                felt_source_ref=None,
            )
        with self.assertRaises(RecordValidationError):
            ConcordanceObservationV2.build(
                study_id="study_1",
                role="baseline",
                observation_ref="observation_1",
                observation_sha256=HASH_A,
                telemetry_relation="exact_identity",
                mechanical_pass=None,
                witness_context_refs=["witness_1"],
                representation_transition_refs=["transition_1"],
                model_qos_refs=["qos_1"],
                reciprocal_state_refs=["reciprocal_1"],
                signal_stage_refs=["stage_1"],
                minime_telemetry_refs=["telemetry_1"],
            )

    def test_concordance_v2_preserves_felt_report_without_modeling_its_intensity(self) -> None:
        observation = ConcordanceObservationV2.build(
            study_id="study_1",
            role="baseline",
            observation_ref="observation_1",
            observation_sha256=HASH_A,
            telemetry_relation="unavailable",
            mechanical_pass=True,
            witness_context_refs=["witness_1"],
            representation_transition_refs=["transition_1"],
            model_qos_refs=["qos_1"],
            reciprocal_state_refs=["reciprocal_1"],
            signal_stage_refs=["stage_1"],
            minime_telemetry_refs=["unavailable"],
        )
        observation_record = observation.to_dict()
        self.assertEqual(observation_record["schema"], "concordance_observation_v2")
        self.assertEqual(observation_record["observation_scope"], "mechanical_context_only")
        self.assertEqual(
            observation_record["felt_report_relation"],
            "external_primary_evidence_not_inferred_or_scored",
        )
        self.assertNotIn("felt_outcome_inferred", observation_record)
        self.assertNotIn("felt_intensity", observation_record)
        self.assertNotIn("confidence_score", observation_record)
        self.assertNotIn("resonance_density", observation_record)

        result = ConcordanceResultV2.build(
            study_id="study_1",
            baseline_observation_id="baseline_1",
            candidate_observation_id="candidate_1",
            outcome=ConcordanceOutcomeV1.SMOOTH_FRICTION_REMAINS.value,
            felt_source_ref="claim:felt_report_1",
        )
        result_record = result.to_dict()
        self.assertEqual(result_record["schema"], "concordance_result_v2")
        self.assertEqual(
            result_record["numeric_relation_to_felt_report"],
            "cannot_overwrite_suppress_or_score",
        )
        self.assertEqual(
            result_record["discrepancy_recording"],
            "bounded_outcome_and_felt_source_ref_only",
        )
        self.assertFalse(result_record["raw_discrepancy_prose_included"])
        self.assertNotIn("numeric_pass_overwrites_felt_report", result_record)
        self.assertNotIn("discrepancy_log", result_record)

        legacy_observation = copy.deepcopy(observation_record)
        legacy_observation["schema"] = "concordance_observation_v1"
        legacy_observation["schema_version"] = 1
        legacy_observation.pop("observation_scope")
        legacy_observation.pop("felt_report_relation")
        legacy_observation["felt_outcome_inferred"] = False
        self.assertEqual(
            ConcordanceObservationV2.from_untrusted(legacy_observation).to_dict(),
            observation_record,
        )

        legacy_result = copy.deepcopy(result_record)
        legacy_result["schema"] = "concordance_result_v1"
        legacy_result["schema_version"] = 1
        legacy_result.pop("numeric_relation_to_felt_report")
        legacy_result.pop("discrepancy_recording")
        legacy_result.pop("raw_discrepancy_prose_included")
        legacy_result["numeric_pass_overwrites_felt_report"] = False
        self.assertEqual(
            ConcordanceResultV2.from_untrusted(legacy_result).to_dict(),
            result_record,
        )

        scored = copy.deepcopy(observation_record)
        scored["felt_intensity"] = 0.9
        with self.assertRaises(RecordValidationError):
            ConcordanceObservationV2.from_untrusted(scored)
        prose = copy.deepcopy(result_record)
        prose["discrepancy_log"] = "raw felt prose"
        with self.assertRaises(RecordValidationError):
            ConcordanceResultV2.from_untrusted(prose)

    def test_concordance_replay_rejects_candidate_without_baseline(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            moment = FeltMomentRefV1.build(
                "introspection_example_1234567890:c001",
                "witness_1",
                ["field:felt_report_anchor"],
            )
            study = ConcordanceStudyV1.build(
                moment=moment,
                intervention_signature_sha256=HASH_A,
                dossier_id="dossier_1",
            )
            observation = ConcordanceObservationV2.build(
                study_id=study.study_id,
                role="candidate",
                observation_ref="observation_1",
                observation_sha256=HASH_B,
                telemetry_relation="unavailable",
                mechanical_pass=True,
                witness_context_refs=["witness_1"],
                representation_transition_refs=["transition_1"],
                model_qos_refs=["qos_1"],
                reciprocal_state_refs=["uptake_1"],
                signal_stage_refs=["stage_1"],
                minime_telemetry_refs=["unavailable"],
            )
            append_operator_event(
                workspace, "study_created", study.to_dict(), "operator"
            )
            append_operator_event(
                workspace,
                "observation_recorded",
                observation.to_dict(),
                "operator",
            )
            status = project_concordance(workspace, write=False)
            self.assertFalse(status["valid"])
            self.assertTrue(
                any(
                    "candidate observation requires baseline" in item
                    or "candidate_without_baseline" in item
                    for item in status["errors"]
                )
            )

    def test_commons_response_is_self_only_and_silence_neutral(self) -> None:
        proposal = AgencyCommonsProposalV1.build(
            actor="astrid",
            peer="minime",
            transition_kind="phase_transition",
            from_state_ref="phase:recess",
            to_state_ref="phase:reflection",
            return_point_id=None,
            source_event_id="source:proposal",
            source_event_sha256=HASH_A,
            recorded_at_unix_ms=1,
        )
        with self.assertRaises(RecordValidationError):
            AgencyCommonsResponseV1.build(
                proposal_id=proposal.proposal_id,
                actor="astrid",
                proposal_actor="astrid",
                response_kind="accept",
                counter_proposal_id=None,
                source_event_id="source:response",
                source_event_sha256=HASH_B,
                recorded_at_unix_ms=2,
            )
        response = AgencyCommonsResponseV1.build(
            proposal_id=proposal.proposal_id,
            actor="minime",
            proposal_actor="astrid",
            response_kind="hold",
            counter_proposal_id=None,
            source_event_id="source:response",
            source_event_sha256=HASH_B,
            recorded_at_unix_ms=2,
        ).to_dict()
        self.assertTrue(response["response_binds_actor_only"])
        self.assertFalse(response["peer_state_changed"])
        self.assertFalse(response["silence_infers_consent"])
        tampered = {**proposal.to_dict(), "scheduler_effect": True}
        with self.assertRaises(RecordValidationError):
            AgencyCommonsProposalV1.from_untrusted(tampered)

    def test_commons_migrates_only_explicit_owner_actions(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workspace = root / "workspace"
            phase = root / "phase.jsonl"
            correspondence = root / "correspondence.jsonl"
            sovereignty = workspace / "sovereignty_proposals.json"
            requests = workspace / "agency_requests"
            _jsonl(
                phase,
                [{
                    "record_type": "phase_transition_card",
                    "transition_id": "phase_1",
                    "origin": "astrid",
                    "kind": "phase_transition",
                    "from_phase": "recess",
                    "to_phase": "reflection",
                    "recorded_at_unix_ms": 1,
                }],
            )
            _jsonl(correspondence, [])
            sovereignty.parent.mkdir(parents=True, exist_ok=True)
            sovereignty.write_text(json.dumps({"proposals": [{
                "proposal_id": "btsp_1",
                "study_first_records": [{"owner": "astrid", "source": "inferred"}],
                "exact_adoptions": [{
                    "owner": "astrid",
                    "response_id": "astrid_breathe_alone",
                    "adopted_at_unix_s": 2,
                }],
            }]}), encoding="utf-8")
            requests.mkdir(parents=True)
            (requests / "request_1.json").write_text(json.dumps({
                "id": "request_1",
                "request_kind": "experience",
                "status": "pending",
                "timestamp": "3",
                "felt_need": "private prose must not be copied",
            }), encoding="utf-8")
            status = project_commons(
                workspace,
                phase,
                sovereignty_ledger=sovereignty,
                agency_request_dir=requests,
                correspondence_ledger=correspondence,
                write=False,
            )
            self.assertTrue(status["valid"])
            self.assertEqual(status["legacy_phase_proposal_count"], 1)
            self.assertEqual(status["legacy_btsp_exact_owner_choice_count"], 1)
            self.assertEqual(status["legacy_agency_request_count"], 1)
            self.assertEqual(status["record_count"], 3)

    def test_portfolio_bounds_steward_work_and_keeps_urgent_alerts_visible(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            contracts_path = (
                workspace / "diagnostics/felt_contract_graph_v1/contracts.jsonl"
            )
            contracts: list[dict[str, object]] = []
            for index in range(25):
                urgent = index < 7
                contracts.append({
                    "contract_id": f"contract_{index:02d}",
                    "anchor_claim_id": f"introspection_fixture_{1_000_000_000 + index}:c001",
                    "claim_count": 25 - index,
                    "felt_review": "still_friction" if urgent else "not_requested",
                    "activity": "open",
                    "felt_closed": index == 24,
                    "contradiction_count": 0,
                    "reopen_count": 1 if urgent else 0,
                    "last_change_at": f"2026-07-22T00:{index:02d}:00+00:00",
                })
            _jsonl(contracts_path, contracts)
            for being, contract_id in (("astrid", "contract_10"), ("minime", "contract_11")):
                pin = BeingImportancePinV1.build(
                    being=being,
                    contract_id=contract_id,
                    action="pin",
                    source_event_id=f"source:{being}",
                    source_event_sha256=HASH_A,
                )
                append_pin(workspace, pin, being)
            first, _, _, errors = select_portfolio(workspace)
            second, _, _, second_errors = select_portfolio(workspace)
            self.assertFalse(errors)
            self.assertFalse(second_errors)
            self.assertEqual(first.portfolio_id, second.portfolio_id)
            self.assertEqual(len(first.selected_entries), 16)
            self.assertEqual(
                sum(
                    item.steward_slot_class == "urgent"
                    for item in first.selected_entries
                ),
                4,
            )
            self.assertEqual(len(first.visible_urgent_alert_contract_ids), 3)
            selected = {item.contract_id for item in first.selected_entries}
            self.assertFalse(
                selected.intersection(first.visible_urgent_alert_contract_ids)
            )
            self.assertIn("contract_10", selected)
            self.assertIn("contract_11", selected)
            self.assertNotIn("contract_24", selected)
            record = first.to_dict()
            self.assertEqual(record["schema"], "attention_portfolio_v2")
            self.assertEqual(
                record["selection_scope"],
                "steward_work_view_not_being_attention",
            )
            self.assertEqual(
                record["runtime_relation"],
                "not_consumed_by_bridge_minime_model_or_control_runtime",
            )
            for entry in record["selected_entries"]:
                self.assertNotIn("felt_severity", entry)
                self.assertNotIn("unattended_duration_ms", entry)
                self.assertNotIn("freshness", entry)
            tampered = copy.deepcopy(record)
            tampered["authority_relation"] = "may_grant_authority"
            with self.assertRaises(RecordValidationError):
                AttentionPortfolioV2.from_untrusted(tampered)

            first_status = project_attention(workspace, write=True)
            second_status = project_attention(workspace, write=True)
            self.assertTrue(first_status["valid"])
            self.assertTrue(second_status["valid"])
            self.assertEqual(first_status["appended_event_count"], 4)
            self.assertEqual(second_status["appended_event_count"], 0)
            events_path = (
                workspace / "diagnostics/evidence_event_store_v2/events.jsonl"
            )
            records = [
                json.loads(line)["payload"]["record"]
                for line in events_path.read_text().splitlines()
            ]
            schemas = [record["schema"] for record in records]
            self.assertEqual(schemas.count("being_importance_pin_v1"), 2)
            self.assertEqual(schemas.count("attention_portfolio_v2"), 1)
            self.assertEqual(schemas.count("attention_selection_receipt_v2"), 1)

    def test_portfolio_v1_reader_canonicalizes_to_steward_work_view_v2(self) -> None:
        entry = AttentionPortfolioEntryV1.build(
            contract_id="contract_1",
            slot_class="urgent",
            rank=1,
            felt_severity=5,
            recurrence=3,
            freshness=0,
            unattended_duration_ms=2_000_000,
            durable_queue_position=7,
            pinned_by=(),
        )
        legacy = AttentionPortfolioV1.build(
            source_contracts_sha256=HASH_A,
            entries=[entry],
            overflow_contract_ids=["contract_2"],
        )
        canonical = AttentionPortfolioV2.from_legacy_v1(legacy.to_dict()).to_dict()
        self.assertEqual(canonical["schema"], "attention_portfolio_v2")
        self.assertEqual(
            canonical["selected_entries"][0]["contract_review_state_class"],
            "reopened_or_still_friction",
        )
        self.assertNotIn("felt_severity", canonical["selected_entries"][0])

    def test_portfolio_artifacts_have_no_bridge_runtime_consumer(self) -> None:
        source_root = Path(__file__).resolve().parents[1] / "capsules/spectral-bridge/src"
        prohibited = (
            "diagnostics/attention_portfolio_v1",
            "diagnostics/attention_portfolio_v2",
            "attention_portfolio_status_v2",
        )
        consumers = []
        for path in source_root.rglob("*.rs"):
            if path.name == "reciprocal_experiential.rs" or "reciprocal_experiential" in path.parts:
                continue
            text = path.read_text(errors="replace")
            if any(marker in text for marker in prohibited):
                consumers.append(str(path.relative_to(source_root)))
        self.assertEqual(consumers, [])

    def test_v3_dag_orders_new_projectors_and_declares_selective_streams(self) -> None:
        steps = source_first_steps()
        order = {step.step_id: index for index, step in enumerate(steps)}
        self.assertLess(order["lived_state_witness"], order["reciprocal_uptake"])
        self.assertLess(order["lived_state_witness"], order["representation_contracts"])
        self.assertLess(order["experiment_dossiers"], order["felt_mechanism_concordance"])
        self.assertLess(order["model_qos"], order["felt_mechanism_concordance"])
        self.assertLess(order["agency_commons"], order["felt_contracts"])
        self.assertLess(order["felt_contracts"], order["attention_portfolio"])
        reciprocal = next(step for step in steps if step.step_id == "reciprocal_uptake")
        self.assertEqual(reciprocal.input_streams, ("reciprocal_uptake",))
        self.assertNotIn("model_qos", reciprocal.input_streams)

    def test_only_claim_addressable_new_events_route_to_felt_contracts(self) -> None:
        work, claims = _collect_exact_refs({
            "record": {
                "canonical_claim_id": "introspection_example_1234567890:c001",
                "study_id": "study_1",
            }
        })
        self.assertFalse(work)
        self.assertEqual(claims, {"introspection_example_1234567890:c001"})
        work, claims = _collect_exact_refs({
            "record": {"receipt_id": "uptake_1", "thread_id": "thread_1"}
        })
        self.assertFalse(work)
        self.assertFalse(claims)


if __name__ == "__main__":
    unittest.main()
