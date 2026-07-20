"""Integration tests for the rebuildable felt-contract projection state."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import stat
import tempfile
import unittest

from evidence_store import EvidenceEventStore
from felt_contract_graph import generate
from felt_contracts.sources import (
    claim_family_semantic_sha256,
    graph_state_dir,
    store_root,
)
from felt_contracts.state_index import FeltContractStateIndex


class FeltContractIncrementalTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.workspace = Path(self.temporary.name) / "workspace"
        self.family_path = (
            self.workspace / "diagnostics/claim_families_v1/status.json"
        )
        self.family_path.parent.mkdir(parents=True)
        store = EvidenceEventStore(store_root(self.workspace))
        store.initialize_from_envelopes([], legacy_imported=False)
        store.activation_path.write_text(
            json.dumps(
                {
                    "schema": "evidence_store_activation_v1",
                    "schema_version": 1,
                    "active_store": "v2",
                }
            )
            + "\n",
            encoding="utf-8",
        )
        self._write_claims(("c1",))

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _claim(self, claim_id: str) -> dict[str, object]:
        canonical_id = f"introspection_astrid_100:{claim_id}"
        record_hash = hashlib.sha256(canonical_id.encode()).hexdigest()
        return {
            "claim_id": claim_id,
            "introspection_id": "introspection_astrid_100",
            "source_sha256": hashlib.sha256(b"source").hexdigest(),
            "canonical_claim_record_sha256": record_hash,
            "queue_position": 1,
            "text": "Preserve the exact felt review boundary.",
            "disposition": "verified_existing",
            "classification": "evidence",
        }

    def _write_claims(self, claim_ids: tuple[str, ...]) -> None:
        claims = {
            f"introspection_astrid_100:{claim_id}": self._claim(claim_id)
            for claim_id in claim_ids
        }
        status = {
            "schema": "claim_family_status_v1",
            "schema_version": 1,
            "families": {
                "family_fixture": {
                    "authority_class": "evidence_only_non_live",
                    "target_surface": "felt_contract_graph",
                    "requested_outcome": "preserve_boundary",
                    "polarity": "affirm",
                    "claims": claims,
                }
            },
        }
        self.family_path.write_text(
            json.dumps(status, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    def test_no_input_is_event_free_and_full_replay_matches(self) -> None:
        first = generate(
            self.workspace,
            write=True,
            actor="test",
        )
        self.assertGreater(first["planned_event_count"], 0)
        store = EvidenceEventStore(store_root(self.workspace))
        first_count = store.stream_watermarks(("felt_contracts",))[
            "felt_contracts"
        ]["stream_seq"]

        second = generate(
            self.workspace,
            write=True,
            actor="test",
        )
        second_count = store.stream_watermarks(("felt_contracts",))[
            "felt_contracts"
        ]["stream_seq"]
        self.assertEqual(second["planned_event_count"], 0)
        self.assertEqual(second["incremental_source_event_count"], 0)
        self.assertEqual(first_count, second_count)

        replay = generate(
            self.workspace,
            write=True,
            actor="test",
            full_replay=True,
        )
        self.assertEqual(replay["planned_event_count"], 0)
        self.assertTrue(replay["full_replay_parity"]["exact"])
        self.assertEqual(first_count, store.stream_watermarks(("felt_contracts",))[
            "felt_contracts"
        ]["stream_seq"])

        index = FeltContractStateIndex(graph_state_dir(self.workspace))
        index_status = index.status()
        self.assertTrue(index_status["materialized"])
        self.assertEqual(index_status["counts"]["contracts"], 1)
        self.assertEqual(index_status["counts"]["membership"], 1)
        self.assertEqual(
            stat.S_IMODE(index.path.stat().st_mode),
            0o600,
        )

    def test_one_new_claim_is_bounded_and_does_not_move_prior_membership(self) -> None:
        generate(self.workspace, write=True, actor="test")
        index = FeltContractStateIndex(graph_state_dir(self.workspace))
        original = index.load_projection()
        self.assertIsNotNone(original)
        original_contract = original["membership"]["introspection_astrid_100:c1"]

        self._write_claims(("c1", "c2"))
        changed = generate(self.workspace, write=True, actor="test")
        updated = index.load_projection()
        self.assertIsNotNone(updated)
        self.assertLessEqual(changed["planned_event_count"], 3)
        self.assertEqual(
            updated["membership"]["introspection_astrid_100:c1"],
            original_contract,
        )
        self.assertEqual(
            updated["membership"]["introspection_astrid_100:c2"],
            original_contract,
        )
        contract = next(
            row
            for row in updated["contracts"]
            if row["contract_id"] == original_contract
        )
        self.assertFalse(contract["felt_closed"])
        self.assertFalse(contract["membership_propagates_closure"])
        self.assertFalse(contract["membership_propagates_authority"])

    def test_source_identity_history_compacts_duplicate_family_payloads(
        self,
    ) -> None:
        store = EvidenceEventStore(store_root(self.workspace))
        events = store.append_payloads(
            "claim_families",
            [
                {
                    "event_type": "claim_family_created",
                    "family_id": "family_fixture",
                },
                {
                    "event_type": "claim_family_membership_assigned",
                    "canonical_claim_id": "introspection_astrid_100:c1",
                    "family_id": "family_fixture",
                },
                {
                    "event_type": "claim_family_membership_assigned",
                    "canonical_claim_id": "introspection_astrid_100:c1",
                    "family_id": "family_fixture",
                },
                {
                    "event_type": "experiment_dossier_projected",
                    "dossier_id": "dossier_fixture",
                },
                {
                    "event_type": "experiment_dossier_projected",
                    "dossier_id": "dossier_fixture",
                },
            ],
        )
        index = FeltContractStateIndex(graph_state_dir(self.workspace))
        inserted = index.ingest_source_events(
            store.iter_envelopes_for_stream("claim_families")
        )
        self.assertEqual(inserted, 5)
        status = index.status()
        self.assertEqual(status["counts"]["source_events"], 5)
        self.assertEqual(status["counts"]["retained_source_events"], 2)
        retained = index.source_envelopes(("claim_families",))
        self.assertEqual(
            [event.event_id for event in retained],
            [events[2].event_id, events[4].event_id],
        )

    def test_unseen_irrelevant_source_tail_preserves_cached_projection(
        self,
    ) -> None:
        generate(self.workspace, write=True, actor="test")
        contracts_path = graph_state_dir(self.workspace) / "contracts.jsonl"
        before = contracts_path.read_bytes()
        store = EvidenceEventStore(store_root(self.workspace))
        store.append_payloads(
            "addressing",
            [{"event_type": "inventory_run", "run_id": "fixture"}],
        )

        result = generate(self.workspace, write=True, actor="test")

        self.assertEqual(result["planned_event_count"], 0)
        self.assertEqual(result["incremental_source_event_count"], 1)
        self.assertEqual(result["incremental_relevant_source_event_count"], 0)
        self.assertEqual(contracts_path.read_bytes(), before)

    def test_family_semantic_hash_ignores_projection_metadata(self) -> None:
        before = claim_family_semantic_sha256(self.workspace)
        status = json.loads(self.family_path.read_text(encoding="utf-8"))
        status["generated_at"] = "later"
        status["families"]["family_fixture"]["projection_receipt"] = {
            "source_status_sha256": "changed"
        }
        self.family_path.write_text(
            json.dumps(status, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        self.assertEqual(
            claim_family_semantic_sha256(self.workspace),
            before,
        )


if __name__ == "__main__":
    unittest.main()
