"""Integration tests for temporal lived-state witness projection."""

from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
from datetime import datetime
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from evidence_store import EvidenceEventStore
from evidence_store.model import ProvenanceSourceV1
from lived_state_witness.model import authority_state
from lived_state_witness.projector import (
    PROJECTOR_VERSION,
    STREAM,
    project,
    reconcile,
    show,
    state_dir,
    verify,
)


def sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def provenance_ref(origin: str = "bridge_derived") -> dict[str, object]:
    descriptor = {
        "minime_observation": "producer_telemetry_shape",
        "bridge_derived": "bridge_evidence_shape",
        "astrid_interpretation": "astrid_interpretive_context_shape",
        "mixed": "composed_witness_shape",
        "unknown": "unknown_context_shape",
    }[origin]
    return {
        "origin": origin,
        "source_id": "fixture",
        "canonical_sha256": "9" * 64,
        "parent_ids": [],
        "timestamp_ms": 1_700_000_000_000,
        "field_paths": ["fixture.value"],
        "context_anchor_v1": {
            "descriptor": descriptor,
            "structural_signature_sha256": "8" * 64,
            "influence_types": ["temporal"],
            "private_payload_included": False,
        },
    }


class LivedStateWitnessProjectionTests(unittest.TestCase):
    maxDiff = None

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.workspace = Path(self.temporary.name) / "workspace"
        self.introspections = self.workspace / "introspections"
        self.introspections.mkdir(parents=True)
        self.store = EvidenceEventStore(
            self.workspace / "diagnostics/evidence_event_store_v2"
        )
        self.store.initialize_from_envelopes([], legacy_imported=False)
        self.store.activation_path.write_text(
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

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _write_exact_fixture(self) -> tuple[str, str]:
        witness_id = "lsw_" + "a" * 64
        timestamp = 1_700_000_100
        filename = f"introspection_fixture_{timestamp}.txt"
        report = (
            "=== ASTRID INTROSPECTION ===\n"
            "Source: fixture\n"
            f"Timestamp: {timestamp}\n"
            f"Lived-state witness: {witness_id}\n"
            "Fill: 68.0%\n\n"
            "Observed:\n- bounded fixture\n"
        ).encode()
        (self.introspections / filename).write_bytes(report)
        process_started_ms = 1_699_999_999_000
        process_started = datetime.fromtimestamp(
            process_started_ms / 1_000
        ).strftime("%a %b %d %H:%M:%S %Y")
        source_identity = "1" * 64
        artifact_identity = "2" * 64
        receipt = {
            "schema": "stack_environment_receipt_v2",
            "schema_version": 2,
            "id": "deployment_fixture",
            "t_ms": 1_700_000_000_000,
            "component": "spectral-bridge",
            "deployment": {"status": "passed"},
            "compatibility_status": {"compatible": True},
            "artifact_authority_state_v1": {
                "schema": "artifact_authority_state_v1",
                "schema_version": 1,
                "state": "evidence_only",
                "witness_only": True,
            },
            "live_eligible_now": False,
            "auto_approved": False,
            "grants_approval": False,
            "edits_source_now": False,
            "repositories": {
                "astrid": {"source_identity_sha256": source_identity}
            },
            "artifacts": {
                "binaries": {
                    "spectral-bridge": {
                        "exists": True,
                        "sha256": artifact_identity,
                    }
                }
            },
            "processes": {
                "new": {
                    "pid": 41,
                    "running": True,
                    "started_at": process_started,
                }
            },
        }
        receipt_path = self.workspace / "environment_receipts"
        receipt_path.mkdir()
        (receipt_path / "environment_receipts.jsonl").write_text(
            json.dumps(receipt, sort_keys=True) + "\n", encoding="utf-8"
        )
        sidecar = {
            "schema": "temporal_lived_state_witness_v1",
            "schema_version": 1,
            "witness_id": witness_id,
            "artifact_kind": "introspection",
            "artifact_relative_path": filename,
            "artifact_sha256": sha256(report),
            "authored_at_unix_ms": timestamp * 1_000,
            "authored_monotonic_ns": 10,
            "source_snapshot_v1": {
                "schema": "lived_state_source_snapshot_v1",
                "schema_version": 1,
                "source_owner": "astrid",
                "repository_relative_path": "src/lib.rs",
                "window_start_line": 0,
                "window_end_line": 1,
                "total_file_lines": 1,
                "file_sha256": "3" * 64,
                "window_sha256": "4" * 64,
                "source_read_at_unix_ms": timestamp * 1_000 - 1,
                "source_read_monotonic_ns": 1,
                "provenance_ref_v1": provenance_ref(
                    "astrid_interpretation"
                ),
                "private_path_included": False,
            },
            "observed_process_v1": {
                "schema": "lived_state_process_identity_v1",
                "schema_version": 1,
                "pid": 41,
                "process_started_at_unix_ms": process_started_ms,
                "executable_basename": "spectral-bridge-server",
                "runtime_instance_id": "runtime_fixture",
                "process_identity_sha256": "5" * 64,
                "private_path_included": False,
            },
            "startup_build_candidate_v1": {
                "schema": "lived_state_build_candidate_v1",
                "schema_version": 1,
                "manifest_sha256": "6" * 64,
                "source_identity_sha256": source_identity,
                "dirty_state_sha256": "7" * 64,
                "artifact_sha256": artifact_identity,
                "protocol_revision": "revision",
                "protocol_revision_complete": True,
                "protocol_version": "1.1",
                "protocol_version_complete": True,
                "observed_at_process_start_unix_ms": process_started_ms,
                "relation_to_process": "startup_observation_not_deployment_proof",
                "deployment_established": False,
                "private_path_included": False,
            },
            "model_routes_v1": [],
            "parameter_observations_v1": [],
            "peer_process_identity": None,
            "peer_deployment_identity": None,
            "source_provenance_ref_v1": None,
            "process_provenance_ref_v1": provenance_ref(),
            "raw_introspection_prose_included": False,
            "raw_prompt_included": False,
            "raw_response_included": False,
            "private_path_included": False,
            "direct_causation_claimed": False,
            "artifact_authority_state_v1": authority_state(),
        }
        source_snapshot = sidecar["source_snapshot_v1"]
        source_snapshot["provenance_ref_v1"]["canonical_sha256"] = source_snapshot[
            "window_sha256"
        ]
        sidecar["source_provenance_ref_v1"] = dict(
            source_snapshot["provenance_ref_v1"]
        )
        process = sidecar["observed_process_v1"]
        process["process_identity_sha256"] = sha256(
            (
                f"{process['pid']}\0{process['process_started_at_unix_ms']}\0"
                f"{process['executable_basename']}\0{process['runtime_instance_id']}"
            ).encode()
        )
        sidecar["process_provenance_ref_v1"]["canonical_sha256"] = process[
            "process_identity_sha256"
        ]
        witness_hasher = hashlib.sha256()
        witness_hasher.update(b"astrid-temporal-lived-state-witness-v1\0")
        witness_hasher.update(process["runtime_instance_id"].encode())
        witness_hasher.update(sidecar["authored_at_unix_ms"].to_bytes(8, "little"))
        witness_hasher.update(sidecar["authored_monotonic_ns"].to_bytes(8, "little"))
        witness_hasher.update(sidecar["artifact_kind"].encode())
        witness_hasher.update(source_snapshot["window_sha256"].encode())
        bound_witness_id = f"lsw_{witness_hasher.hexdigest()}"
        report = report.replace(witness_id.encode(), bound_witness_id.encode())
        witness_id = bound_witness_id
        sidecar["witness_id"] = witness_id
        sidecar["artifact_sha256"] = sha256(report)
        (self.introspections / filename).write_bytes(report)
        sidecar_root = self.introspections / "lived_state_witnesses/witnesses"
        sidecar_root.mkdir(parents=True)
        (sidecar_root / f"{witness_id}.json").write_text(
            json.dumps(sidecar, sort_keys=True) + "\n", encoding="utf-8"
        )
        return witness_id, filename

    def test_exact_projection_is_idempotent_and_checkpoint_selective(self) -> None:
        witness_id, _ = self._write_exact_fixture()
        first = project(self.workspace, write=True)
        self.assertTrue(first["valid"])
        self.assertEqual(first["migration_counters"]["canonical"], 1)
        self.assertEqual(first["migration_counters"]["exact"], 1)
        self.assertEqual(
            first["migration_counters"]["exact_deployment_match"], 1
        )
        self.assertEqual(
            first["witnesses"][witness_id]["alignment"]["outcome"],
            "same_deployment",
        )
        first_watermark = self.store.stream_watermarks((STREAM,))[STREAM][
            "stream_seq"
        ]
        first_hashes = dict(first["projection_hashes"])

        second = project(self.workspace, write=True)
        self.assertEqual(
            self.store.stream_watermarks((STREAM,))[STREAM]["stream_seq"],
            first_watermark,
        )
        self.assertEqual(second["projection_hashes"], first_hashes)

        self.store.append_payloads(
            "sandbox",
            [
                {
                    "event_type": "unrelated_fixture",
                    "idempotency_key": "unrelated_fixture",
                    "artifact_authority_state_v1": authority_state(),
                }
            ],
            actor="test",
            source=ProvenanceSourceV1("test", "unrelated"),
            idempotency_keys=["unrelated_fixture"],
        )
        self.assertTrue(
            self.store.checkpoint_current_for_inputs(
                "lived_state_witness_v1", PROJECTOR_VERSION
            )
        )
        self.store.append_payloads(
            "addressing",
            [
                {
                    "event_type": "declared_input_fixture",
                    "idempotency_key": "declared_input_fixture",
                    "artifact_authority_state_v1": authority_state(),
                }
            ],
            actor="test",
            source=ProvenanceSourceV1("test", "declared"),
            idempotency_keys=["declared_input_fixture"],
        )
        self.assertFalse(
            self.store.checkpoint_current_for_inputs(
                "lived_state_witness_v1", PROJECTOR_VERSION
            )
        )

    def test_exact_receipt_may_complete_after_authorship(self) -> None:
        witness_id, _ = self._write_exact_fixture()
        receipt_path = (
            self.workspace
            / "environment_receipts/environment_receipts.jsonl"
        )
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        receipt["t_ms"] = 1_700_000_100_500
        receipt_path.write_text(json.dumps(receipt) + "\n", encoding="utf-8")
        status = project(self.workspace, write=True)
        self.assertEqual(
            status["witnesses"][witness_id]["alignment"]["outcome"],
            "same_deployment",
        )

    def test_delayed_exact_receipt_reconciles_without_propagating_state(self) -> None:
        witness_id, _ = self._write_exact_fixture()
        receipt_path = (
            self.workspace
            / "environment_receipts/environment_receipts.jsonl"
        )
        receipt = receipt_path.read_text(encoding="utf-8")
        receipt_path.unlink()
        first = project(self.workspace, write=True)
        self.assertEqual(
            first["witnesses"][witness_id]["alignment"]["outcome"],
            "deployment_unknown",
        )
        receipt_path.write_text(receipt, encoding="utf-8")
        result = reconcile(self.workspace, write=True)
        self.assertEqual(result["outcome_counts"], {"same_deployment": 1})
        shown = show(self.workspace, witness_id)
        self.assertTrue(shown["reconciliation"]["exact_identity_match"])
        self.assertFalse(shown["reconciliation"]["closure_propagated"])
        self.assertTrue(verify(self.workspace)["valid"])

    def test_concurrent_retry_does_not_duplicate_events(self) -> None:
        self._write_exact_fixture()
        with ThreadPoolExecutor(max_workers=2) as executor:
            results = list(
                executor.map(
                    lambda _: project(self.workspace, write=True), range(2)
                )
            )
        self.assertTrue(all(result["valid"] for result in results))
        events, corrupt = self.store.payloads_for_stream(STREAM)
        self.assertEqual(corrupt, 0)
        self.assertEqual(len(events), 2)

    def test_missing_sidecar_is_gap_and_tampered_output_fails_verify(self) -> None:
        witness_id = "lsw_" + "b" * 64
        timestamp = 1_700_000_200
        (self.introspections / f"introspection_gap_{timestamp}.txt").write_text(
            "=== ASTRID INTROSPECTION ===\n"
            f"Timestamp: {timestamp}\n"
            f"Lived-state witness: {witness_id}\n\n"
            "Observed:\n- report remains primary\n",
            encoding="utf-8",
        )
        status = project(self.workspace, write=True)
        self.assertEqual(status["migration_counters"]["gap"], 1)
        self.assertEqual(status["witness_count"], 0)
        context = json.loads(
            (state_dir(self.workspace) / "context_index.jsonl")
            .read_text(encoding="utf-8")
            .strip()
        )
        self.assertEqual(context["witness_id"], witness_id)
        self.assertEqual(context["gap_count"], 1)
        self.assertEqual(context["alignment"]["outcome"], "witness_gap")
        self.assertTrue(verify(self.workspace)["valid"])
        (state_dir(self.workspace) / "context_index.jsonl").write_text(
            "tampered\n", encoding="utf-8"
        )
        verification = verify(self.workspace)
        self.assertFalse(verification["valid"])
        self.assertIn(
            "output_hash_mismatch:context_index.jsonl",
            verification["output_errors"],
        )

    def test_invalid_writer_gap_never_copies_untrusted_payload(self) -> None:
        gap_root = self.introspections / "lived_state_witnesses/gaps"
        gap_root.mkdir(parents=True)
        private_text = "/private/raw response prose"
        (gap_root / "tampered.json").write_text(
            json.dumps(
                {
                    "schema": "lived_state_gap_receipt_v1",
                    "schema_version": 1,
                    "witness_id": "lsw_" + "a" * 64,
                    "reason": private_text,
                    "raw_response": private_text,
                }
            ),
            encoding="utf-8",
        )
        project(self.workspace, write=True)
        events, corrupt = self.store.payloads_for_stream(STREAM)
        self.assertEqual(corrupt, 0)
        gap = next(
            event
            for event in events
            if event.get("event_type") == "lived_state_writer_gap_recorded"
        )
        self.assertIsNone(gap["gap_receipt"])
        self.assertFalse(gap["invalid_payload_copied"])
        self.assertNotIn(private_text, json.dumps(gap, sort_keys=True))


if __name__ == "__main__":
    unittest.main()
