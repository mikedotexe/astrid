#!/usr/bin/env python3
"""Tests for exact-only Temporal Bearing projection."""

from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

from temporal_bearing.projector import project, projection_dir


def _write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def _write_jsonl(path: Path, rows: list[dict[str, object]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows))


def _stage(
    journey_id: str,
    index: int,
    kind: str,
    effect: str,
    output_sha256: str,
    *,
    stage_id: str | None = None,
    time_ms: int = 1_000,
) -> dict[str, object]:
    return {
        "stage_id": stage_id or f"stage_{index:024x}",
        "stage_index": index,
        "stage_kind": kind,
        "relation": "root" if index == 0 else "exact_transformation",
        "effect": effect,
        "ownership_domain": "astrid_authored" if index == 0 else "bridge_evidence",
        "source_sha256": output_sha256,
        "output_sha256": output_sha256,
        "receipt_integrity_sha256": f"{index + 10:064x}",
        "process_identity_v1": {
            "clock_scope_id": "clock-test",
            "deployment_identity": "test-deployment",
        },
        "temporal_envelope_v1": {
            "stage_time_unix_ms": time_ms + index,
            "monotonic_time_ns": 10_000 + index,
        },
        "journey_id": journey_id,
    }


def _journey(
    journey_id: str,
    response_sha256: str,
    *,
    contact_id: str | None = None,
) -> dict[str, object]:
    origin = (
        {
            "kind": "contact",
            "contact_id": contact_id,
            "evidence_sha256": "a" * 64,
        }
        if contact_id
        else {"kind": "autonomous"}
    )
    return {
        "schema": "causal_signal_journey_v1",
        "schema_version": 1,
        "journey_id": journey_id,
        "journey_origin_v1": origin,
        "response_origin_v1": "model_authored",
        "lineage_valid": True,
        "receipts": [
            _stage(journey_id, 0, "authored", "produced", response_sha256),
            _stage(journey_id, 1, "dispatched", "dispatched", "b" * 64),
            _stage(journey_id, 2, "delivery_evidence", "evidence_recorded", "c" * 64),
        ],
    }


class TemporalBearingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.workspace = self.root / "astrid-workspace"
        self.minime = self.root / "minime-workspace"
        self.workspace.mkdir()
        self.minime.mkdir()

    def tearDown(self) -> None:
        self.temp.cleanup()

    def _seed_contact(
        self,
        journey_id: str,
        contact_id: str,
        response_sha256: str,
    ) -> None:
        _write_json(
            self.workspace
            / f"diagnostics/signal_spine_v1/journeys/{journey_id}.json",
            _journey(journey_id, response_sha256, contact_id=contact_id),
        )
        contact = {
            "schema": "contact_input_receipt_v1",
            "schema_version": 1,
            "contact_id": contact_id,
            "journey_id": journey_id,
            "receipt_sha256": "a" * 64,
            "cutoff_unix_ms": 900,
            "received_at_unix_ms": 901,
            "received_monotonic_ns": 9_000,
            "clock_scope_id": "clock-test",
            "admission_outcome": "admitted",
        }
        _write_json(
            self.workspace
            / f"diagnostics/contact_capacity_trace_v1/contact_receipts/{contact_id}.json",
            contact,
        )
        _write_jsonl(
            self.workspace / "diagnostics/contact_capacity_trace_v2/traces.jsonl",
            [
                {
                    "contact_id": contact_id,
                    "journey_id": journey_id,
                    "terminal_state": "delivered",
                    "input_to_first_response_ms": 1.0,
                    "input_to_first_response_clock_relation": "same_process_monotonic",
                    "input_to_terminal_ms": 3.0,
                    "input_to_terminal_clock_relation": "same_process_monotonic",
                }
            ],
        )

    def _record(self, journey_id: str) -> dict[str, object]:
        path = projection_dir(self.workspace) / f"by_journey/{journey_id}.json"
        return json.loads(path.read_text())

    def test_exact_contact_response_stage_and_passage_links_bind_both_rails(self) -> None:
        journey_id = "journey_" + "1" * 24
        contact_id = "contact_" + "2" * 24
        passage_id = "passage_1000_" + "3" * 24
        response_sha = "4" * 64
        self._seed_contact(journey_id, contact_id, response_sha)

        witness = {
            "witness_id": "lsw_" + "5" * 64,
            "event_type": "temporal_lived_state_witness_recorded",
            "introspection_id": "introspection_exact",
            "evidence_completeness": "exact_authorship_sidecar",
            "witness": {
                "witness_id": "lsw_" + "5" * 64,
                "authored_at_unix_ms": 1_020,
                "provenance_ref_v1": {"parent_ids": [journey_id]},
            },
            "report_ref": {"sha256": "6" * 64},
            "private_prose": "must never reach the projection",
        }
        _write_jsonl(
            self.workspace / "diagnostics/lived_state_witness_v1/witnesses.jsonl",
            [witness],
        )
        passage = {
            "passage_id": passage_id,
            "transition_id": "transition_exact",
            "actor": "astrid",
            "latest_stage": "held",
            "latest_stage_event_id": "passage_event_exact",
            "latest_felt_review_outcome": "still_friction",
            "stage_history": [
                {
                    "passage_event_id": "passage_event_exact",
                    "recorded_at_unix_ms": 1_030,
                    "continuity_anchor_ref": contact_id,
                }
            ],
            "anchors": [
                {
                    "role": "continuity",
                    "current": {
                        "passage_context_event_id": "passage_context_exact",
                        "anchor_kind": "signal_spine",
                        "anchor_association": "receipt_linked",
                        "anchor_ref": journey_id,
                        "recorded_at_unix_ms": 1_031,
                    },
                }
            ],
            "strands": [
                {
                    "strand": "continuity",
                    "expression_state": "self_authored",
                    "current": {
                        "passage_context_event_id": "bearing_exact",
                        "movement_resistance": "changing",
                        "persistence_tendency": "carried",
                        "witness_fit": "holding",
                        "recorded_at_unix_ms": 1_032,
                    },
                }
            ],
        }
        _write_json(
            self.minime / "division/passage-observatory/observatory_v1.json",
            {"phase_passages": {"passages": [passage]}},
        )
        _write_json(
            self.workspace / "shadow_cartography/trajectory_exact.json",
            {
                "schema": "shadow_trajectory_v1",
                "recorded_at_unix_ms": 1_040,
                "source_response_sha256": response_sha,
                "history": [{"field_norm": 0.2}],
                "history_bearing_v1": {
                    "schema": "shadow_history_bearing_v1",
                    "bearing": "recent_window_only",
                    "applies_temporal_decay": False,
                    "infers_felt_causation": False,
                },
            },
        )
        texture = {
            "schema": "texture_dynamics_snapshot_v1",
            "schema_version": 1,
            "inquiry_id": "inquiry-exact",
            "strand_count": 1,
            "unordered_pair_count": 0,
            "strands": [
                {
                    "strand_id": "strand-a",
                    "source": {
                        "source_evidence_id": "texture-source-a",
                        "source_response_interval_sha256": response_sha,
                        "sampled_at_unix_ms": 1_050,
                    },
                }
            ],
            "raw_reservoir_mode_packing_state": "missing_no_exact_raw_reservoir_source",
            "shadow_dispersal_state": "missing_no_exact_shadow_history_source",
            "felt_status": {"state": "unreported", "author_domain": "owner_authored_only"},
        }
        _write_json(
            self.workspace
            / "volition_v1/astrid/inquiries/receipts/inquiry-exact.json",
            {"analysis_receipts": [{"result": {"texture_dynamics_snapshot_v1": texture}}]},
        )

        status = project(
            self.workspace,
            minime_workspace=self.minime,
            write=True,
            now_unix_ms=2_000,
        )
        self.assertTrue(status["valid"], status["errors"])
        record = self._record(journey_id)
        self.assertEqual(record["contact_id"], contact_id)
        self.assertEqual(record["passage_ids"], [passage_id])
        self.assertEqual(len(record["owner_rail"]["lived_state_witnesses"]), 1)
        self.assertEqual(len(record["owner_rail"]["phase_passages"]), 1)
        self.assertEqual(len(record["machine_rail"]["shadow_history_evidence"]), 1)
        self.assertEqual(len(record["machine_rail"]["texture_dynamics_snapshots"]), 1)
        self.assertIsNone(record["owner_rail"]["felt_weight_scalar"])
        self.assertFalse(record["timestamp_proximity_edges_created"])
        self.assertNotIn("must never reach", json.dumps(record))

    def test_timestamp_proximity_never_links_and_owner_state_stays_unreported(self) -> None:
        journey_id = "journey_" + "7" * 24
        response_sha = "8" * 64
        _write_json(
            self.workspace
            / f"diagnostics/signal_spine_v1/journeys/{journey_id}.json",
            _journey(journey_id, response_sha),
        )
        _write_jsonl(
            self.workspace / "diagnostics/lived_state_witness_v1/witnesses.jsonl",
            [
                {
                    "witness_id": "lsw_" + "9" * 64,
                    "event_type": "temporal_lived_state_witness_recorded",
                    "introspection_id": "same_time_only",
                    "witness": {
                        "witness_id": "lsw_" + "9" * 64,
                        "authored_at_unix_ms": 1_000,
                    },
                    "report_ref": {"sha256": "a" * 64},
                }
            ],
        )
        status = project(
            self.workspace,
            minime_workspace=self.minime,
            write=True,
            now_unix_ms=2_000,
        )
        self.assertTrue(status["valid"], status["errors"])
        record = self._record(journey_id)
        self.assertEqual(
            record["owner_rail"]["lived_state_witness_state"],
            "owner_unreported_for_exact_lineage",
        )
        self.assertEqual(status["summary"]["unlinked_evidence_count"], 1)
        unlinked = json.loads(
            (projection_dir(self.workspace) / "unlinked_evidence.jsonl")
            .read_text()
            .strip()
        )
        self.assertEqual(unlinked["reason"], "no_exact_lineage_identity")
        self.assertFalse(unlinked["timestamp_proximity_considered"])

    def test_duplicate_response_hash_is_ambiguous_not_a_join(self) -> None:
        response_sha = "b" * 64
        for digit in ("c", "d"):
            journey_id = "journey_" + digit * 24
            _write_json(
                self.workspace
                / f"diagnostics/signal_spine_v1/journeys/{journey_id}.json",
                _journey(journey_id, response_sha),
            )
        _write_json(
            self.workspace / "shadow_cartography/trajectory_ambiguous.json",
            {
                "schema": "shadow_trajectory_v1",
                "recorded_at_unix_ms": 1_000,
                "source_response_sha256": response_sha,
                "history": [],
            },
        )
        status = project(
            self.workspace,
            minime_workspace=self.minime,
            write=True,
            now_unix_ms=2_000,
        )
        self.assertTrue(status["valid"], status["errors"])
        self.assertEqual(status["summary"]["shadow_binding_count"], 0)
        unlinked = (
            projection_dir(self.workspace) / "unlinked_evidence.jsonl"
        ).read_text()
        self.assertIn("ambiguous_exact_response_hash", unlinked)

    def test_rerun_is_deterministic_for_records_and_indexes(self) -> None:
        journey_id = "journey_" + "e" * 24
        _write_json(
            self.workspace
            / f"diagnostics/signal_spine_v1/journeys/{journey_id}.json",
            _journey(journey_id, "f" * 64),
        )
        first = project(
            self.workspace,
            minime_workspace=self.minime,
            write=True,
            now_unix_ms=2_000,
        )
        record_before = (
            projection_dir(self.workspace) / f"by_journey/{journey_id}.json"
        ).read_bytes()
        indexes_before = (projection_dir(self.workspace) / "indexes.json").read_bytes()
        second = project(
            self.workspace,
            minime_workspace=self.minime,
            write=True,
            now_unix_ms=2_000,
        )
        self.assertEqual(first, second)
        self.assertEqual(
            record_before,
            (projection_dir(self.workspace) / f"by_journey/{journey_id}.json").read_bytes(),
        )
        self.assertEqual(
            indexes_before, (projection_dir(self.workspace) / "indexes.json").read_bytes()
        )


if __name__ == "__main__":
    unittest.main()
