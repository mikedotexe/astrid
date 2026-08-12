#!/usr/bin/env python3
"""Tests for origin-aware contact-to-capacity projections."""

from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

try:
    from contact_capacity_trace.projector import (
        _canonical_sha256,
        project,
        projection_dir,
        state_dir,
        validate_contact_receipt,
    )
except ModuleNotFoundError:
    from scripts.contact_capacity_trace.projector import (
        _canonical_sha256,
        project,
        projection_dir,
        state_dir,
        validate_contact_receipt,
    )


def bounded_id(prefix: str, number: int) -> str:
    return f"{prefix}{number:024x}"


def jsonl(path: Path) -> list[dict[str, object]]:
    if not path.is_file():
        return []
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]


class ContactCapacityTraceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.workspace = Path(self.temp.name)
        (self.workspace / "diagnostics/signal_spine_v1/journeys").mkdir(parents=True)

    def tearDown(self) -> None:
        self.temp.cleanup()

    @staticmethod
    def process(*, pid: int = 7, clock_scope: str = "clockscope_bridge_one") -> dict[str, object]:
        return {
            "pid": pid,
            "executable": "bridge",
            "deployment_identity": "build-one",
            "process_started_at_unix_ms": 900,
            "clock_scope_id": clock_scope,
        }

    @staticmethod
    def source_refs(*, mixed: bool = False) -> list[dict[str, object]]:
        rows: list[dict[str, object]] = [
            {
                "source_ref_id_sha256": "1" * 64,
                "source_bytes_sha256": "2" * 64,
                "source_body_sha256": "3" * 64,
                "principal_identity_sha256": "4" * 64,
                "thread_id_sha256": None,
                "provenance_state": "provenance_backed",
                "provenance_basis": "sanctioned_owner_note_v1",
                "local_provenance_validated": True,
                "cryptographic_sender_authenticated": False,
            }
        ]
        if mixed:
            rows.append(
                {
                    "source_ref_id_sha256": "5" * 64,
                    "source_bytes_sha256": "6" * 64,
                    "source_body_sha256": None,
                    "principal_identity_sha256": None,
                    "thread_id_sha256": None,
                    "provenance_state": "unverified",
                    "provenance_basis": "unverified_inbox_source",
                    "local_provenance_validated": False,
                    "cryptographic_sender_authenticated": False,
                }
            )
        return rows

    def contact(
        self,
        number: int,
        journey_number: int,
        *,
        admission: str = "admitted",
        input_hash: str = "a" * 64,
        raw_payload: bool = False,
        tamper: bool = False,
    ) -> dict[str, object]:
        contact_id = bounded_id("contact_", number)
        journey_id = bounded_id("journey_", journey_number)
        process_hash = _canonical_sha256(self.process())
        sources = self.source_refs()
        value: dict[str, object] = {
            "schema": "contact_input_receipt_v1",
            "schema_version": 1,
            "contact_id": contact_id,
            "journey_id": journey_id,
            "cutoff_unix_ms": 1_000,
            "clock_scope_id": "clockscope_bridge_one",
            "input_identity_sha256": input_hash,
            "principal_identity_sha256": "b" * 64,
            "process_identity_sha256": process_hash,
            "received_at_unix_ms": 1_100,
            "received_monotonic_ns": 1_000_000,
            "admitted_context_sha256": "c" * 64,
            "admission_outcome": admission,
            "admission_basis": "prompt_context" if admission == "admitted" else "pre_prompt_policy",
            "provenance_state": "provenance_backed",
            "source_count": len(sources),
            "source_refs": sources,
            "local_provenance_validation_only": True,
            "cryptographic_sender_authentication": False,
            "raw_input_included": False,
            "live_control_authority": False,
        }
        if raw_payload:
            value["prompt"] = "private raw prose"
        value["receipt_sha256"] = _canonical_sha256(value)
        if tamper:
            value["input_identity_sha256"] = "d" * 64
        directory = state_dir(self.workspace) / "contact_receipts"
        directory.mkdir(parents=True, exist_ok=True)
        (directory / f"{contact_id}.json").write_text(json.dumps(value), encoding="utf-8")
        return value

    def ingress_gap(
        self,
        number: int,
        journey_number: int,
        *,
        reason: str = "mixed_or_unverified_source",
    ) -> dict[str, object]:
        gap_id = bounded_id("ingress_gap_", number)
        journey_id = bounded_id("journey_", journey_number)
        sources = self.source_refs(mixed=reason == "mixed_or_unverified_source")
        value: dict[str, object] = {
            "schema": "contact_ingress_gap_v1",
            "schema_version": 1,
            "ingress_gap_id": gap_id,
            "journey_id": journey_id,
            "attempted_contact_id": (
                bounded_id("contact_", number) if reason == "contact_capture_failed" else None
            ),
            "reason": reason,
            "capture_error_kind": "already_exists" if reason == "contact_capture_failed" else None,
            "cutoff_unix_ms": 1_000,
            "clock_scope_id": "clockscope_bridge_one",
            "input_identity_sha256": "a" * 64,
            "process_identity_sha256": _canonical_sha256(self.process()),
            "received_at_unix_ms": 1_100,
            "received_monotonic_ns": 1_000_000,
            "admitted_context_sha256": "c" * 64,
            "admission_outcome": "admitted",
            "provenance_state": (
                "provenance_backed" if reason == "contact_capture_failed" else "mixed_or_unverified"
            ),
            "source_count": len(sources),
            "source_refs": sources,
            "local_provenance_validation_only": True,
            "cryptographic_sender_authentication": False,
            "raw_input_included": False,
            "live_control_authority": False,
        }
        value["receipt_sha256"] = _canonical_sha256(value)
        directory = state_dir(self.workspace) / "ingress_gaps"
        directory.mkdir(parents=True, exist_ok=True)
        (directory / f"{gap_id}.json").write_text(json.dumps(value), encoding="utf-8")
        return value

    def journey(
        self,
        number: int,
        *,
        origin_kind: str | None,
        origin_id: str | None = None,
        origin_evidence: str | None = None,
        response_origin: str = "model_authored",
        terminal: str = "delivered",
        process: dict[str, object] | None = None,
    ) -> dict[str, object]:
        journey_id = bounded_id("journey_", number)
        process = process or self.process()
        receipts: list[dict[str, object]] = [
            {
                "stage_id": bounded_id("stage_", number * 10),
                "stage_index": 0,
                "stage_kind": "authored",
                "effect": "produced",
                "process_identity_v1": process,
                "temporal_envelope_v1": {
                    "stage_time_unix_ms": 1_200,
                    "monotonic_time_ns": 2_000_000,
                },
            }
        ]
        if terminal == "delivered":
            receipts.append(
                {
                    "stage_id": bounded_id("stage_", number * 10 + 1),
                    "stage_index": 1,
                    "stage_kind": "delivery_evidence",
                    "effect": "evidence_recorded",
                    "process_identity_v1": process,
                    "temporal_envelope_v1": {
                        "stage_time_unix_ms": 1_400,
                        "monotonic_time_ns": 4_000_000,
                    },
                }
            )
        elif terminal == "outbound_blocked":
            receipts.append(
                {
                    "stage_id": bounded_id("stage_", number * 10 + 1),
                    "stage_index": 1,
                    "stage_kind": "blocked",
                    "effect": "blocked",
                    "process_identity_v1": process,
                    "temporal_envelope_v1": {
                        "stage_time_unix_ms": 1_300,
                        "monotonic_time_ns": 3_000_000,
                    },
                }
            )
        value: dict[str, object] = {
            "schema": "causal_signal_journey_v1",
            "journey_id": journey_id,
            "lineage_valid": True,
            "response_origin_v1": response_origin,
            "receipts": receipts,
        }
        if origin_kind is not None:
            origin: dict[str, object] = {
                "kind": origin_kind,
                "input_root_expected": origin_kind == "contact",
                "local_provenance_validation_only": origin_kind in {"contact", "ingress_gap"},
                "cryptographic_sender_authentication": False,
            }
            if origin_kind == "contact":
                origin["contact_id"] = origin_id
            if origin_kind == "ingress_gap":
                origin["ingress_gap_id"] = origin_id
                origin["ingress_gap_reason"] = "mixed_or_unverified_source"
            if origin_evidence is not None:
                origin["evidence_sha256"] = origin_evidence
            value["journey_origin_v1"] = origin
        path = self.workspace / f"diagnostics/signal_spine_v1/journeys/{journey_id}.json"
        path.write_text(json.dumps(value), encoding="utf-8")
        return value

    def test_proven_root_joins_with_same_process_durations_and_v1_compatibility(self) -> None:
        contact = self.contact(1, 1)
        self.journey(
            1,
            origin_kind="contact",
            origin_id=str(contact["contact_id"]),
            origin_evidence=str(contact["receipt_sha256"]),
        )
        status = project(self.workspace, write=True)
        self.assertTrue(status["valid"])
        self.assertEqual(status["complete_trace_count"], 1)
        trace = jsonl(projection_dir(self.workspace) / "traces.jsonl")[0]
        self.assertEqual(trace["terminal_state"], "delivered")
        self.assertEqual(trace["input_to_first_response_ms"], 1.0)
        self.assertEqual(trace["input_to_terminal_ms"], 3.0)
        self.assertFalse(trace["felt_capacity_inferred"])
        compat = jsonl(state_dir(self.workspace) / "traces.jsonl")[0]
        self.assertEqual(compat["schema"], "contact_capacity_trace_v1")

    def test_mixed_input_is_an_ingress_gap_and_never_a_contact_root(self) -> None:
        gap = self.ingress_gap(2, 2)
        self.journey(
            2,
            origin_kind="ingress_gap",
            origin_id=str(gap["ingress_gap_id"]),
            origin_evidence=str(gap["receipt_sha256"]),
        )
        status = project(self.workspace, write=True)
        self.assertTrue(status["valid"])
        self.assertEqual(status["contact_receipt_count"], 0)
        self.assertEqual(status["ingress_gap_count"], 1)
        row = jsonl(projection_dir(self.workspace) / "ingress_gaps.jsonl")[0]
        self.assertFalse(row["contact_root_emitted"])
        self.assertFalse(row["verified_subset_promoted_to_contact"])

    def test_capture_failure_is_explicit_without_blocking_response_lineage(self) -> None:
        gap = self.ingress_gap(3, 3, reason="contact_capture_failed")
        self.journey(
            3,
            origin_kind="ingress_gap",
            origin_id=str(gap["ingress_gap_id"]),
            origin_evidence=str(gap["receipt_sha256"]),
            response_origin="static_fallback",
        )
        status = project(self.workspace, write=True)
        self.assertTrue(status["valid"])
        row = jsonl(projection_dir(self.workspace) / "ingress_gaps.jsonl")[0]
        self.assertEqual(row["reason"], "contact_capture_failed")
        self.assertEqual(row["response_origin_v1"], "static_fallback")

    def test_autonomous_and_historical_journeys_are_not_missing_contacts(self) -> None:
        self.journey(4, origin_kind="autonomous", response_origin="mirror")
        self.journey(5, origin_kind=None)
        status = project(self.workspace, write=True)
        self.assertEqual(status["autonomous_journey_count"], 1)
        self.assertEqual(status["legacy_unknown_journey_count"], 1)
        self.assertEqual(status["true_contact_root_missing_count"], 0)
        origins = jsonl(projection_dir(self.workspace) / "journey_origins.jsonl")
        self.assertEqual({row["origin"]["kind"] for row in origins}, {"autonomous", "legacy_unknown"})

    def test_held_and_denied_are_pre_prompt_terminals_without_journeys(self) -> None:
        self.contact(6, 6, admission="held")
        self.contact(7, 7, admission="denied")
        status = project(self.workspace, write=True)
        self.assertTrue(status["valid"])
        self.assertEqual(status["pre_prompt_terminal_count"], 2)
        traces = jsonl(projection_dir(self.workspace) / "traces.jsonl")
        self.assertEqual(
            {row["terminal_state"] for row in traces},
            {"held_before_prompt", "denied_before_prompt"},
        )
        self.assertTrue(all(row["journey_present"] is False for row in traces))

    def test_outbound_block_is_terminal_and_fallback_ownership_is_preserved(self) -> None:
        contact = self.contact(8, 8)
        self.journey(
            8,
            origin_kind="contact",
            origin_id=str(contact["contact_id"]),
            origin_evidence=str(contact["receipt_sha256"]),
            response_origin="static_fallback",
            terminal="outbound_blocked",
        )
        status = project(self.workspace, write=True)
        self.assertEqual(status["complete_trace_count"], 1)
        trace = jsonl(projection_dir(self.workspace) / "traces.jsonl")[0]
        self.assertEqual(trace["terminal_state"], "outbound_blocked")
        self.assertEqual(trace["response_origin_v1"], "static_fallback")

    def test_split_process_keeps_exact_link_but_withholds_duration(self) -> None:
        contact = self.contact(9, 9)
        self.journey(
            9,
            origin_kind="contact",
            origin_id=str(contact["contact_id"]),
            origin_evidence=str(contact["receipt_sha256"]),
            process=self.process(pid=8, clock_scope="clockscope_bridge_two"),
        )
        project(self.workspace, write=True)
        trace = jsonl(projection_dir(self.workspace) / "traces.jsonl")[0]
        self.assertIsNone(trace["input_to_first_response_ms"])
        self.assertEqual(trace["input_to_first_response_clock_relation"], "split_process_identity")
        reasons = {row["reason"] for row in jsonl(projection_dir(self.workspace) / "capture_gaps.jsonl")}
        self.assertIn("split_process_identity", reasons)

    def test_retries_with_same_input_hash_remain_distinct_events(self) -> None:
        first = self.contact(10, 10, input_hash="e" * 64)
        second = self.contact(11, 11, input_hash="e" * 64)
        self.journey(
            10,
            origin_kind="contact",
            origin_id=str(first["contact_id"]),
            origin_evidence=str(first["receipt_sha256"]),
        )
        self.journey(
            11,
            origin_kind="contact",
            origin_id=str(second["contact_id"]),
            origin_evidence=str(second["receipt_sha256"]),
        )
        status = project(self.workspace, write=True)
        self.assertTrue(status["valid"])
        self.assertEqual(status["trace_count"], 2)
        self.assertEqual(status["complete_trace_count"], 2)

    def test_duplicate_journey_claims_are_rejected(self) -> None:
        self.contact(12, 12)
        self.contact(13, 12)
        status = project(self.workspace, write=False)
        self.assertFalse(status["valid"])
        self.assertIn("multiple_contact_roots_claim_same_journey", status["errors"])

    def test_tampered_and_raw_payload_receipts_are_rejected(self) -> None:
        tampered = self.contact(14, 14, tamper=True)
        raw = self.contact(15, 15, raw_payload=True)
        self.assertIn("receipt_sha256_mismatch", validate_contact_receipt(tampered))
        self.assertIn("forbidden_raw_payload", validate_contact_receipt(raw))
        status = project(self.workspace, write=False)
        self.assertFalse(status["valid"])
        self.assertEqual(status["rejected_contact_receipt_count"], 2)

    def test_contact_origin_without_receipt_is_the_only_true_root_gap(self) -> None:
        self.journey(
            16,
            origin_kind="contact",
            origin_id=bounded_id("contact_", 16),
            origin_evidence="f" * 64,
        )
        status = project(self.workspace, write=True)
        self.assertEqual(status["true_contact_root_missing_count"], 1)
        compat = json.loads((state_dir(self.workspace) / "status.json").read_text())
        self.assertEqual(compat["orphan_journey_count"], 1)


if __name__ == "__main__":
    unittest.main()
