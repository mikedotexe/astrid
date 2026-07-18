"""Focused self-tests for the living felt-contract graph."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import tempfile
import unittest

try:
    from authority_state import normalize_artifact_authority_tree
except ModuleNotFoundError:
    from scripts.authority_state import normalize_artifact_authority_tree

from .identity import contract_id_for_anchor, edge_id, node_id
from .model import (
    ContractActivityV1,
    FeltContractV1,
    FeltReviewOutcomeV1,
    build_contract,
    build_edge,
    build_implementation_receipt,
    build_intervention_boundary,
    build_node,
    build_signal_ref,
)
from .projector import GraphProjectionError, project_graph
from .sources import ClaimSource, _assignment_plan, _repository_ref


def _authority(state: str = "evidence_only") -> dict[str, object]:
    return {
        "schema": "artifact_authority_state_v1",
        "schema_version": 1,
        "state": state,
        "witness_only": True,
    }


class FeltContractGraphTests(unittest.TestCase):
    def setUp(self) -> None:
        self.claim_id = "introspection_astrid_codec_1784301105:c001"
        self.contract_id = contract_id_for_anchor(self.claim_id)
        self.contract = build_contract(
            contract_id=self.contract_id,
            anchor_claim_id=self.claim_id,
            created_at="2026-07-18T00:00:00+00:00",
            authority_state="evidence_only",
        )

    def event(self, event_type: str, **values: object) -> dict[str, object]:
        return {
            "schema": "felt_contract_domain_event_v1",
            "schema_version": 1,
            "event_type": event_type,
            "aggregate_type": "felt_contract",
            "aggregate_id": self.contract_id,
            **values,
            "artifact_authority_state_v1": _authority(),
        }

    def base_events(self) -> tuple[list[dict[str, object]], str]:
        signal_id = node_id("evt_signal", "felt_signal", self.contract_id)
        claim_node_id = node_id("evt_claim", f"claim:{self.claim_id}", self.contract_id)
        signal = build_node(
            node_id=signal_id,
            contract_id=self.contract_id,
            kind="felt_signal",
            source_event_id="evt_signal",
            occurred_at="2026-07-18T00:00:00+00:00",
            source_ref=build_signal_ref(
                source_kind="canonical_introspection",
                source_id="introspection_astrid_codec_1784301105",
                canonical_sha256="a" * 64,
                owner="astrid",
                observed_at="2026-07-18T00:00:00+00:00",
                field_paths=("claims.c001",),
            ),
            metadata={"private_content_copied": False},
            authority_state="evidence_only",
        ).to_dict()
        claim = build_node(
            node_id=claim_node_id,
            contract_id=self.contract_id,
            kind="claim",
            source_event_id="evt_claim",
            occurred_at="2026-07-18T00:00:01+00:00",
            source_ref=None,
            metadata={
                "canonical_claim_id": self.claim_id,
                "canonical_claim_record_sha256": "b" * 64,
                "private_content_copied": False,
            },
            authority_state="evidence_only",
        ).to_dict()
        edge = build_edge(
            edge_id=edge_id(signal_id, claim_node_id, "contains_claim", "evt_claim"),
            contract_id=self.contract_id,
            source_node_id=signal_id,
            target_node_id=claim_node_id,
            relation="contains_claim",
            source_event_id="evt_claim",
            occurred_at="2026-07-18T00:00:01+00:00",
            causal_parent=True,
        ).to_dict()
        return (
            [
                self.event("felt_contract_created", contract=self.contract.to_dict()),
                self.event(
                    "felt_contract_claim_assigned",
                    contract_id=self.contract_id,
                    canonical_claim_id=self.claim_id,
                ),
                self.event(
                    "felt_contract_node_recorded",
                    contract_id=self.contract_id,
                    node=signal,
                    edges=[],
                ),
                self.event(
                    "felt_contract_node_recorded",
                    contract_id=self.contract_id,
                    node=claim,
                    edges=[edge],
                ),
            ],
            claim_node_id,
        )

    def project(self, events: list[dict[str, object]]) -> dict[str, object]:
        with tempfile.TemporaryDirectory() as directory:
            return project_graph(events, workspace=Path(directory))

    def test_trusted_records_reject_direct_construction(self) -> None:
        with self.assertRaises(TypeError):
            FeltContractV1(
                contract_id=self.contract_id,
                anchor_claim_id=self.claim_id,
                created_at="2026-07-18T00:00:00+00:00",
                authority_state="evidence_only",
            )

    def test_contract_identity_is_deterministic(self) -> None:
        self.assertEqual(
            contract_id_for_anchor(self.claim_id),
            contract_id_for_anchor(self.claim_id),
        )
        self.assertNotEqual(
            contract_id_for_anchor(self.claim_id),
            contract_id_for_anchor(f"{self.claim_id}_other"),
        )

    def test_metadata_rejects_raw_prose_and_absolute_paths(self) -> None:
        with self.assertRaises(ValueError):
            build_node(
                node_id="node_test",
                contract_id=self.contract_id,
                kind="claim",
                source_event_id="evt_test",
                occurred_at="now",
                source_ref=None,
                metadata={"text": "private report prose"},
                authority_state="evidence_only",
            )
        with self.assertRaises(ValueError):
            build_node(
                node_id="node_test",
                contract_id=self.contract_id,
                kind="evidence",
                source_event_id="evt_test",
                occurred_at="now",
                source_ref=None,
                metadata={"evidence_ref": "/Users/example/private"},
                authority_state="evidence_only",
            )

    def test_repository_paths_are_relative_or_hashed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            local = root / "src/file.rs"
            local.parent.mkdir()
            local.write_text("source")
            self.assertEqual(
                _repository_ref(str(local), {"fixture": root}),
                "repo:fixture/src/file.rs",
            )
            external = _repository_ref(
                "/private/location/source.rs", {"fixture": root}
            )
            self.assertTrue(str(external).startswith("external_path_sha256:"))

    def test_tampering_and_dangling_parents_are_rejected(self) -> None:
        events, _ = self.base_events()
        tampered = [dict(event) for event in events]
        tampered_node = dict(tampered[-1]["node"])
        tampered_node["kind"] = "evidence"
        tampered[-1] = {**tampered[-1], "node": tampered_node}
        with self.assertRaises(GraphProjectionError):
            self.project(tampered)

        events, _ = self.base_events()
        broken_edge = dict(events[-1]["edges"][0])
        broken_edge["source_node_id"] = "node_missing"
        unsigned = {key: value for key, value in broken_edge.items() if key != "edge_sha256"}
        try:
            from evidence_store.model import sha256_canonical
        except ModuleNotFoundError:
            from scripts.evidence_store.model import sha256_canonical

        broken_edge["edge_sha256"] = sha256_canonical(unsigned)
        events[-1] = {**events[-1], "edges": [broken_edge]}
        with self.assertRaises(GraphProjectionError):
            self.project(events)

    def test_central_authority_projection_does_not_change_semantic_hash(self) -> None:
        events, claim_node_id = self.base_events()
        boundary = build_intervention_boundary(
            boundary_id="boundary_projection_round_trip",
            agency_tier=4,
            authority_class="operator_approval",
            lifecycle_state="waiting",
            authority_state="approval_pending",
        )
        intervention_id = node_id(
            "evt_intervention", "intervention", self.contract_id
        )
        intervention = build_node(
            node_id=intervention_id,
            contract_id=self.contract_id,
            kind="intervention",
            source_event_id="evt_intervention",
            occurred_at="2026-07-18T00:00:02+00:00",
            source_ref=None,
            metadata={
                "boundary": boundary.to_dict(),
                "canonical_claim_id": self.claim_id,
                "private_content_copied": False,
            },
            authority_state="approval_pending",
        ).to_dict()
        intervention_edge = build_edge(
            edge_id=edge_id(
                claim_node_id,
                intervention_id,
                "proposes",
                "evt_intervention",
            ),
            contract_id=self.contract_id,
            source_node_id=claim_node_id,
            target_node_id=intervention_id,
            relation="proposes",
            source_event_id="evt_intervention",
            occurred_at="2026-07-18T00:00:02+00:00",
            causal_parent=True,
        ).to_dict()
        events.append(
            {
                **self.event(
                    "felt_contract_node_recorded",
                    contract_id=self.contract_id,
                    node=intervention,
                    edges=[intervention_edge],
                ),
                "artifact_authority_state_v1": _authority(
                    "approval_pending"
                ),
            }
        )
        normalized = [
            normalize_artifact_authority_tree(dict(event)) for event in events
        ]
        projection = self.project(normalized)
        self.assertEqual(projection["status"]["node_count"], 3)

    def test_silence_is_neutral_and_named_friction_reopens(self) -> None:
        events, _ = self.base_events()
        events.append(
            self.event(
                "felt_contract_review_outcome_recorded",
                contract_id=self.contract_id,
                deployment_receipt_id="deploy_one",
                outcome=FeltReviewOutcomeV1.NO_RESPONSE.value,
            )
        )
        projection = self.project(events)
        contract = projection["contracts"][0]
        self.assertEqual(
            contract["activity"], ContractActivityV1.QUIET_ARCHIVED.value
        )
        self.assertFalse(contract["felt_closed"])

        events.append(
            self.event(
                "felt_contract_review_outcome_recorded",
                contract_id=self.contract_id,
                deployment_receipt_id="deploy_one",
                outcome=FeltReviewOutcomeV1.STILL_FRICTION.value,
            )
        )
        projection = self.project(events)
        self.assertEqual(
            projection["contracts"][0]["activity"],
            ContractActivityV1.REOPENED.value,
        )

    def test_duplicate_assignment_requires_explicit_correction(self) -> None:
        events, _ = self.base_events()
        events.append(
            self.event(
                "felt_contract_claim_assigned",
                contract_id=self.contract_id,
                canonical_claim_id=self.claim_id,
            )
        )
        with self.assertRaisesRegex(
            GraphProjectionError, "duplicate initial membership"
        ):
            self.project(events)

    def test_membership_correction_preserves_identity_and_felt_state(self) -> None:
        events, _ = self.base_events()
        events.append(
            self.event(
                "felt_contract_review_outcome_recorded",
                contract_id=self.contract_id,
                deployment_receipt_id="deploy_one",
                outcome=FeltReviewOutcomeV1.FELT_CONFIRMED.value,
            )
        )
        target_contract_id = contract_id_for_anchor("intro_other:c001")
        target_contract = build_contract(
            contract_id=target_contract_id,
            anchor_claim_id="intro_other:c001",
            created_at="2026-07-18T00:02:00+00:00",
            authority_state="evidence_only",
        )
        events.extend(
            [
                {
                    **self.event(
                        "felt_contract_created",
                        contract=target_contract.to_dict(),
                    ),
                    "aggregate_id": target_contract_id,
                },
                {
                    **self.event(
                        "felt_contract_membership_corrected",
                        contract_id=target_contract_id,
                        canonical_claim_id=self.claim_id,
                        from_contract_id=self.contract_id,
                        to_contract_id=target_contract_id,
                        reason_sha256="c" * 64,
                    ),
                    "aggregate_id": target_contract_id,
                },
            ]
        )
        projection = self.project(events)
        rows = {
            row["contract_id"]: row for row in projection["contracts"]
        }
        self.assertEqual(rows[self.contract_id]["anchor_claim_id"], self.claim_id)
        self.assertTrue(rows[self.contract_id]["felt_closed"])
        self.assertEqual(rows[self.contract_id]["claim_ids"], [])
        self.assertFalse(rows[target_contract_id]["felt_closed"])
        self.assertEqual(
            rows[target_contract_id]["claim_ids"], [self.claim_id]
        )

    def test_review_budget_is_one_per_changed_contract_and_deployment(self) -> None:
        events, _ = self.base_events()
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            receipt_dir = workspace / "environment_receipts"
            receipt_dir.mkdir()
            receipt = {
                "schema": "astrid_environment_receipt_v2",
                "schema_version": 2,
                "id": "deploy_one",
                "t_ms": 1,
                "change_refs": [
                    {"kind": "felt_contract", "id": self.contract_id}
                ],
                "deployment": {"status": "passed"},
            }
            (receipt_dir / "environment_receipts.jsonl").write_text(
                json.dumps(receipt) + "\n",
                encoding="utf-8",
            )
            projection = project_graph(events, workspace=workspace)
            budget = projection["review_budgets"][self.contract_id]
            self.assertTrue(budget["packet_available"])
            self.assertEqual(budget["packet_budget"], 1)

            events.append(
                self.event(
                    "felt_contract_review_outcome_recorded",
                    contract_id=self.contract_id,
                    deployment_receipt_id="deploy_one",
                    outcome=FeltReviewOutcomeV1.NO_RESPONSE.value,
                )
            )
            projection = project_graph(events, workspace=workspace)
            budget = projection["review_budgets"][self.contract_id]
            self.assertFalse(budget["packet_available"])
            self.assertEqual(budget["delivered_or_answered_count"], 1)

    def test_report_close_and_card_delivery_do_not_close_contract(self) -> None:
        events, claim_node_id = self.base_events()
        close_id = node_id(
            "evt_close", "compatibility_report_close", self.contract_id
        )
        close_node = build_node(
            node_id=close_id,
            contract_id=self.contract_id,
            kind="compatibility_report_close",
            source_event_id="evt_close",
            occurred_at="2026-07-18T00:01:00+00:00",
            source_ref=None,
            metadata={
                "closes_claim": False,
                "closes_contract": False,
                "card_delivery_is_closure": False,
            },
            authority_state="evidence_only",
        ).to_dict()
        close_edge = build_edge(
            edge_id=edge_id(
                claim_node_id, close_id, "compatibility_only", "evt_close"
            ),
            contract_id=self.contract_id,
            source_node_id=claim_node_id,
            target_node_id=close_id,
            relation="compatibility_only",
            source_event_id="evt_close",
            occurred_at="2026-07-18T00:01:00+00:00",
            causal_parent=True,
        ).to_dict()
        events.append(
            self.event(
                "felt_contract_node_recorded",
                contract_id=self.contract_id,
                node=close_node,
                edges=[close_edge],
            )
        )
        projection = self.project(events)
        self.assertEqual(
            projection["contracts"][0]["activity"], ContractActivityV1.OPEN.value
        )

    def test_strict_assignment_joins_one_unambiguous_contract(self) -> None:
        first = ClaimSource(
            claim_id="intro_a:c001",
            introspection_id="intro_a",
            local_claim_id="c001",
            source_sha256="a" * 64,
            queue_order=(1, "intro_a"),
            family_id="family_old",
            authority_class="evidence_only_non_live",
            target_surface="astrid_codec",
            requested_outcome="preserve",
            polarity="neutral",
            text="Preserve exact sensory JSON compatibility.",
            disposition="verified",
            classification="",
            record_sha256="b" * 64,
            source_path_ref=None,
        )
        second = ClaimSource(
            **{
                **first.__dict__,
                "claim_id": "intro_b:c001",
                "introspection_id": "intro_b",
                "queue_order": (2, "intro_b"),
                "family_id": "family_new",
                "source_sha256": "c" * 64,
                "record_sha256": "d" * 64,
            }
        )
        existing_contract = contract_id_for_anchor(first.claim_id)
        membership, _, ambiguous = _assignment_plan(
            [first, second], {first.claim_id: existing_contract}
        )
        self.assertEqual(membership[second.claim_id], existing_contract)
        self.assertEqual(ambiguous, 0)

    def test_implementation_receipt_is_evidence_only_and_source_bound(self) -> None:
        value = {
            "schema": "implementation_receipt_v1",
            "schema_version": 1,
            "receipt_id": "implementation_one",
            "actor": "codex",
            "recorded_at": "2026-07-18T00:00:00+00:00",
            "repository": "astrid",
            "source_identity_sha256": "a" * 64,
            "contract_ids": [self.contract_id],
            "claim_ids": [self.claim_id],
            "work_item_ids": ["wi_example"],
            "changed_path_hashes": {"repo:astrid/scripts/example.py": "b" * 64},
            "test_refs": ["test_felt_contract_graph"],
            "artifact_authority_state_v1": _authority(),
        }
        receipt = build_implementation_receipt(value).to_dict()
        self.assertTrue(receipt["witness_only"])
        self.assertEqual(
            receipt["artifact_authority_state_v1"]["state"], "evidence_only"
        )
        with self.assertRaises(ValueError):
            build_implementation_receipt(
                {
                    **value,
                    "changed_path_hashes": {"/private/source.py": "b" * 64},
                }
            )

    def test_tier_four_and_five_interventions_remain_approval_pending(self) -> None:
        with self.assertRaises(ValueError):
            build_intervention_boundary(
                boundary_id="boundary_live_trial",
                agency_tier=4,
                authority_class="live_trial",
                lifecycle_state="waiting",
                authority_state="evidence_only",
            )
        boundary = build_intervention_boundary(
            boundary_id="boundary_live_trial",
            agency_tier=5,
            authority_class="live_trial",
            lifecycle_state="waiting",
            authority_state="approval_pending",
        )
        self.assertEqual(
            boundary.to_dict()["artifact_authority_state_v1"]["state"],
            "approval_pending",
        )


def run() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(
        FeltContractGraphTests
    )
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1
