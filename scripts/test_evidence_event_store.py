#!/usr/bin/env python3
"""Failure-mode tests for Evidence Event Store V2."""

from __future__ import annotations

import json
import multiprocessing
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

try:
    from evidence_event_store import counter_audits, effective_aggregate_audit
except ModuleNotFoundError:
    from scripts.evidence_event_store import (
        counter_audits,
        effective_aggregate_audit,
    )

try:
    from evidence_store.adapter import append_domain_events, read_domain_events, v2_active_for_state
    from evidence_store.migration import LegacyEventSource, import_legacy_sources
    from evidence_store.store import EvidenceEventStore, EvidenceStoreError
except ModuleNotFoundError:
    from scripts.evidence_store.adapter import (
        append_domain_events,
        read_domain_events,
        v2_active_for_state,
    )
    from scripts.evidence_store.migration import LegacyEventSource, import_legacy_sources
    from scripts.evidence_store.store import EvidenceEventStore, EvidenceStoreError


def _concurrent_append(root: str, worker: int, count: int) -> None:
    store = EvidenceEventStore(Path(root))
    for index in range(count):
        store.append_payloads(
            "addressing",
            [{"event_type": "concurrent", "worker": worker, "index": index}],
            idempotency_keys=[f"worker:{worker}:{index}"],
        )


class EvidenceEventStoreTests(unittest.TestCase):
    def test_counter_audits_follow_real_projector_schemas(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            diagnostics = workspace / "diagnostics"
            addressing = diagnostics / "introspection_addressing_v1"
            sandbox = diagnostics / "sandbox_trial_queue_v1"
            corridor = diagnostics / "agency_corridor_v2"
            signal = diagnostics / "signal_spine_v1"
            families = diagnostics / "claim_families_v1"
            dossiers = diagnostics / "experiment_dossiers_v1"
            for directory in (addressing, sandbox, corridor, signal, families, dossiers):
                directory.mkdir(parents=True)
            (addressing / "status.json").write_text(
                json.dumps(
                    {
                        "summary": {"canonical_indexed": 4},
                        "counter_audit": {
                            "checks": {"indexed_matches": True, "remaining_matches": True}
                        },
                    }
                ),
                encoding="utf-8",
            )
            (sandbox / "status.json").write_text(
                json.dumps(
                    {
                        "corrupt_event_lines": 0,
                        "trials": {
                            "one": {"state": "ready"},
                            "two": {"status": "approval_required"},
                        },
                    }
                ),
                encoding="utf-8",
            )
            (corridor / "status.json").write_text(
                json.dumps(
                    {
                        "summary": {"live_violation_count": 0, "packet_count": 1},
                        "packets": {"one": {}},
                    }
                ),
                encoding="utf-8",
            )
            (signal / "projection_status.json").write_text(
                json.dumps(
                    {
                        "journey_count": 20,
                        "complete_journey_count": 20,
                        "lineage_mismatch_count": 0,
                        "parity_mismatch_count": 0,
                        "tampered_journey_count": 0,
                        "tampered_association_count": 0,
                        "dispatch_failed_journey_count": 0,
                        "capture_gap_count": 0,
                        "zero_mismatch": True,
                    }
                ),
                encoding="utf-8",
            )
            (families / "status.json").write_text(
                json.dumps(
                    {
                        "counter_audit": {
                            "every_claim_assigned_once": True,
                            "membership_count_equals_claim_count": True,
                            "unassigned_claim_count": 0,
                        }
                    }
                ),
                encoding="utf-8",
            )
            (dossiers / "status.json").write_text(
                json.dumps(
                    {
                        "counter_audit": {
                            "all_dossiers_have_one_state": True,
                            "candidate_or_later_has_baseline": True,
                        }
                    }
                ),
                encoding="utf-8",
            )
            audits = counter_audits(workspace)
            self.assertTrue(audits["addressing"]["consistent"])
            self.assertEqual(
                audits["addressing"]["summary"]["canonical_indexed"], 4
            )
            self.assertEqual(audits["sandbox"]["trial_count"], 2)
            self.assertEqual(audits["sandbox"]["trial_state_counts"]["ready"], 1)
            self.assertTrue(audits["corridor_v2"]["consistent"])
            self.assertEqual(audits["corridor_v2"]["packets_count"], 1)
            self.assertTrue(audits["signal_spine"]["consistent"])
            self.assertEqual(audits["signal_spine"]["complete_journey_count"], 20)
            self.assertTrue(audits["claim_families"]["consistent"])
            self.assertTrue(audits["experiment_dossiers"]["consistent"])

    def test_append_idempotency_and_authority_rejection(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = EvidenceEventStore(Path(tmp) / "store")
            result = store.append_payloads(
                "addressing",
                [{"event_type": "read"}, {"event_type": "read"}],
                idempotency_keys=["same", "same"],
            )
            self.assertEqual(result[0].event_id, result[1].event_id)
            self.assertEqual(store.verify().event_count, 1)
            aggregate_event = store.append_payloads(
                "signal_spine",
                [
                    {
                        "event_type": "signal_journey_recorded",
                        "aggregate_type": "causal_signal_journey",
                        "aggregate_id": "journey_one",
                        "journey_id": "journey_one",
                    }
                ],
            )[0]
            self.assertEqual(
                aggregate_event.aggregate,
                {"kind": "causal_signal_journey", "id": "journey_one"},
            )
            aggregate_audit = effective_aggregate_audit(store)
            self.assertTrue(aggregate_audit["effective_aggregate_valid"])
            self.assertEqual(aggregate_audit["envelope_exact_count"], 1)
            self.assertEqual(
                aggregate_audit["historical_payload_fallback_count"], 0
            )
            with self.assertRaises(ValueError):
                store.append_payloads(
                    "addressing",
                    [{"event_type": "bad", "grants_approval": True}],
                )

    def test_concurrent_writers_keep_one_valid_chain(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = str(Path(tmp) / "store")
            processes = [
                multiprocessing.Process(target=_concurrent_append, args=(root, worker, 8))
                for worker in range(4)
            ]
            for process in processes:
                process.start()
            for process in processes:
                process.join(20)
                self.assertEqual(process.exitcode, 0)
            verification = EvidenceEventStore(Path(root)).verify()
            self.assertTrue(verification.valid, verification.errors)
            self.assertEqual(verification.event_count, 32)

    def test_interrupted_tail_and_tampering_are_detected(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = EvidenceEventStore(Path(tmp) / "store")
            store.append_payloads("sandbox", [{"event_type": "trial_created"}])
            with store.events_path.open("a", encoding="utf-8") as handle:
                handle.write('{"partial":')
            verification = store.verify()
            self.assertFalse(verification.valid)
            self.assertEqual(verification.corrupt_lines, 1)
        with tempfile.TemporaryDirectory() as tmp:
            store = EvidenceEventStore(Path(tmp) / "store")
            store.append_payloads("sandbox", [{"event_type": "trial_created"}])
            text = store.events_path.read_text(encoding="utf-8")
            store.events_path.write_text(text.replace("trial_created", "trial_changed"), encoding="utf-8")
            self.assertIn("event_hash_mismatch", ";".join(store.verify().errors))

    def test_migration_is_deterministic_and_preserves_stream_order(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            first = base / "addressing.jsonl"
            second = base / "sandbox.jsonl"
            first.write_text(
                '\n'.join(
                    json.dumps(value)
                    for value in (
                        {"event_type": "a1", "ts": 20.0},
                        {"event_type": "a2", "ts": 10.0},
                    )
                ) + "\n",
                encoding="utf-8",
            )
            second.write_text(
                json.dumps({"event_type": "s1", "ts": 15.0}) + "\n",
                encoding="utf-8",
            )
            sources = [
                LegacyEventSource("addressing", first),
                LegacyEventSource("sandbox", second),
            ]
            stores = [EvidenceEventStore(base / "one"), EvidenceEventStore(base / "two")]
            for store in stores:
                receipt = import_legacy_sources(store, sources, write=True)
                self.assertEqual(receipt["status"], "passed")
            self.assertEqual(
                stores[0].events_path.read_bytes(), stores[1].events_path.read_bytes()
            )
            addressing, corrupt = stores[0].payloads_for_stream("addressing")
            self.assertEqual(corrupt, 0)
            self.assertEqual([event["event_type"] for event in addressing], ["a1", "a2"])

    def test_checkpoint_invalidation_activation_adapter_and_guarded_rollback(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            diagnostics = base / "diagnostics"
            state_dir = diagnostics / "introspection_addressing_v1"
            state_dir.mkdir(parents=True)
            legacy = state_dir / "events.jsonl"
            legacy.write_text(json.dumps({"event_type": "legacy", "ts": 1.0}) + "\n", encoding="utf-8")
            store = EvidenceEventStore(diagnostics / "evidence_event_store_v2")
            import_legacy_sources(
                store, [LegacyEventSource("addressing", legacy)], write=True
            )
            store.write_checkpoint("addressing", 1, {"status": "abc"})
            self.assertTrue(store.checkpoint_current("addressing", 1))
            store.activate(actor="test", acknowledgement="test activation")
            with patch.dict(os.environ, {}, clear=False):
                os.environ.pop("ASTRID_EVIDENCE_STORE_MODE", None)
                os.environ.pop("ASTRID_EVIDENCE_STORE_ROOT", None)
                self.assertTrue(v2_active_for_state(state_dir))
                append_domain_events(
                    state_dir,
                    "addressing",
                    [{"event_type": "v2_only"}],
                    actor="test",
                )
                append_domain_events(
                    diagnostics / "signal_spine_v1",
                    "signal_spine",
                    [{"event_type": "signal_journey_recorded"}],
                    actor="test",
                )
                append_domain_events(
                    diagnostics / "claim_families_v1",
                    "claim_families",
                    [{"event_type": "claim_family_created"}],
                    actor="test",
                )
                payloads, corrupt = read_domain_events(state_dir, "addressing")
                signal_payloads, signal_corrupt = read_domain_events(
                    diagnostics / "signal_spine_v1", "signal_spine"
                )
                family_payloads, family_corrupt = read_domain_events(
                    diagnostics / "claim_families_v1", "claim_families"
                )
            self.assertEqual(corrupt, 0)
            self.assertEqual([item["event_type"] for item in payloads], ["legacy", "v2_only"])
            self.assertEqual(signal_corrupt, 0)
            self.assertEqual(
                [item["event_type"] for item in signal_payloads],
                ["signal_journey_recorded"],
            )
            self.assertEqual(family_corrupt, 0)
            self.assertEqual(
                [item["event_type"] for item in family_payloads],
                ["claim_family_created"],
            )
            self.assertFalse(store.checkpoint_current("addressing", 1))
            with self.assertRaises(EvidenceStoreError):
                store.rollback_to_v1(actor="test", acknowledgement="no export")
            export_root = base / "compatibility"
            store.export_v1_compatibility(
                export_root, actor="test", acknowledgement="verified export"
            )
            rolled_back = store.rollback_to_v1(
                actor="test",
                acknowledgement="verified rollback",
                compatibility_export=export_root / "compatibility_export_receipt.json",
            )
            self.assertEqual(rolled_back["active_store"], "v1")

    def test_v2_checkpoint_ignores_unrelated_stream_append(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = EvidenceEventStore(Path(tmp) / "store")
            store.initialize_from_envelopes([], legacy_imported=False)
            store.append_payloads(
                "addressing",
                [{"event_type": "claim_recorded", "claim_id": "claim_one"}],
            )
            store.write_checkpoint(
                "addressing_projection",
                2,
                {"status": "abc"},
                input_streams=["addressing"],
                source_hashes={"source": "one"},
            )
            store.append_payloads(
                "sandbox",
                [{"event_type": "trial_recorded", "trial_id": "trial_one"}],
            )
            self.assertTrue(
                store.checkpoint_current_for_inputs(
                    "addressing_projection",
                    2,
                    input_streams=["addressing"],
                    source_hashes={"source": "one"},
                )
            )

    def test_v2_checkpoint_invalidates_declared_stream_or_source(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = EvidenceEventStore(Path(tmp) / "store")
            store.initialize_from_envelopes([], legacy_imported=False)
            store.write_checkpoint(
                "addressing_projection",
                2,
                {"status": "abc"},
                input_streams=["addressing"],
                source_hashes={"source": "one"},
            )
            self.assertFalse(
                store.checkpoint_current_for_inputs(
                    "addressing_projection",
                    2,
                    input_streams=["addressing"],
                    source_hashes={"source": "two"},
                )
            )
            store.append_payloads(
                "addressing",
                [{"event_type": "claim_recorded", "claim_id": "claim_one"}],
            )
            self.assertFalse(
                store.checkpoint_current_for_inputs(
                    "addressing_projection",
                    2,
                    input_streams=["addressing"],
                    source_hashes={"source": "one"},
                )
            )


if __name__ == "__main__":
    raise SystemExit(
        0
        if unittest.TextTestRunner(verbosity=2)
        .run(unittest.defaultTestLoader.loadTestsFromTestCase(EvidenceEventStoreTests))
        .wasSuccessful()
        else 1
    )
