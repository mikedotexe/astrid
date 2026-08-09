#!/usr/bin/env python3
"""Tests for the sealed authored-inquiry train viewer."""

from __future__ import annotations

import hashlib
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import astrid_train as train


def encode_point(point: tuple[int, int, int, int]) -> bytes:
    x, y, z, _ = point
    inverse = pow(z, train._P - 2, train._P)
    affine_x, affine_y = x * inverse % train._P, y * inverse % train._P
    value = affine_y | ((affine_x & 1) << 255)
    return value.to_bytes(32, "little")


def keypair(seed: bytes) -> tuple[bytes, callable]:
    expanded = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(
        bytes([expanded[0] & 248]) + expanded[1:31] + bytes([(expanded[31] & 63) | 64]),
        "little",
    )
    public = encode_point(train._scalar(train._BASE, scalar))

    def sign(message: bytes) -> bytes:
        nonce = int.from_bytes(hashlib.sha512(expanded[32:] + message).digest(), "little") % train._L
        encoded_r = encode_point(train._scalar(train._BASE, nonce))
        challenge = int.from_bytes(
            hashlib.sha512(encoded_r + public + message).digest(), "little"
        ) % train._L
        return encoded_r + ((nonce + challenge * scalar) % train._L).to_bytes(32, "little")

    return public, sign


class AstridTrainTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.workspace = self.root / "workspace"
        self.inquiry = self.root / "immutable-inquiry"
        (self.workspace / "introspections/scheduled").mkdir(parents=True)
        (self.workspace / "runtime/scheduled-introspection/admission").mkdir(parents=True)
        (self.workspace / "runtime/scheduled-introspection/admission").chmod(0o700)
        (self.workspace / "runtime/scheduled-introspection/projection").mkdir(parents=True)
        (self.workspace / "autonomous").mkdir()
        (self.inquiry / "segments").mkdir(parents=True)
        self.inquiry.chmod(0o750)
        (self.inquiry / "segments").chmod(0o750)
        self.public, self.sign = keypair(bytes(range(32)))
        self.key = self.root / "verify.pub"
        self.key.write_bytes(self.public)
        self.key.chmod(0o644)
        self.appliance = "avado-edge"

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def signed(self, schema: str, core: dict[str, object]) -> dict[str, object]:
        message = train.canonical(core)
        return {
            "schema": schema,
            "core": core,
            "core_sha256": train.sha256(message),
            "auth": {
                "algorithm": "ed25519",
                "key_id": f"ed25519:{train.sha256(self.public)[:16]}",
                "signature": self.sign(message).hex(),
            },
        }

    def step(self, *, parent: str | None = None) -> dict[str, object]:
        return {
            "schema": "astrid.edge.inquiry.step.v1",
            "thread_operation": "open" if parent is None else "continue",
            "thread_id": "thread-latency",
            "parent_step_id": parent,
            "observation": "Warm requests cross a recurring latency shelf.",
            "interpretation": "Prompt growth may be part of the constraint.",
            "uncertainty": "Only one hardware profile has enough samples.",
            "decision": "Measure another independent warm interval.",
            "counterpoint": "Thermal pressure may explain the same shelf.",
            "next_test": "Compare matched prompt sizes.",
            "evidence_ids": ["study-latency-1"],
            "confidence": "tentative",
            "belief_operation": "propose",
            "belief_id": "belief-latency",
            "belief_claim": "Prompt size affects feasible inquiry depth.",
        }

    def add_entry(
        self,
        *,
        prior_hash: str = train.GENESIS_HASH,
        prior_id: str = "genesis",
        parent: str | None = None,
        recorded_at: int = 1_800_000,
        trigger_kind: str = "scheduled",
    ) -> tuple[dict[str, object], str, bytes]:
        step = self.step(parent=parent)
        declaration = "INQUIRY_STEP: " + json.dumps(
            step, sort_keys=True, separators=(",", ":")
        )
        reflection = (
            "I am testing whether latency constrains inquiry depth.\n"
            + declaration
            + "\nSOURCE_REVIEW: NONE\n"
        ).encode()
        response_hash = train.sha256(reflection)
        reflection_path = "introspections/scheduled/reflection_due-1800_test.md"
        path = self.workspace / reflection_path
        path.write_bytes(reflection)
        path.chmod(0o640)
        trace = {
            "schema_version": 1,
            "trace_id": "00000000-0000-4000-8000-000000000001",
            "turn_id": "00000000-0000-4000-8000-000000000002",
            "span_id": "00000000-0000-4000-8000-000000000003",
            "session_id": "scheduled-session",
        }
        provisional = {
            "appliance_id": self.appliance,
            "trigger_kind": trigger_kind,
            "trigger_nonce": "scheduled-due-1800",
            "trace": trace,
            "response_sha256": response_hash,
            "declaration_sha256": train.sha256(declaration.encode()),
        }
        signed_entry_id = train.derive_entry_id(provisional)
        step_id = "inquiry-step-" + train.derive_id(
            train.STEP_ID_DOMAIN, self.appliance, signed_entry_id
        )
        core = {
            "schema": train.ENTRY_SCHEMA,
            "appliance_id": self.appliance,
            "signed_entry_id": signed_entry_id,
            "step_id": step_id,
            "admission_id": "inquiry-admission-"
            + train.derive_id(train.ADMISSION_ID_DOMAIN, self.appliance, signed_entry_id),
            "recorded_at_unix_ms": recorded_at,
            "trigger_kind": trigger_kind,
            "due_nonce": "due-1800",
            "trigger_nonce": "scheduled-due-1800",
            "trace": trace,
            "prompt_sha256": "1" * 64,
            "response_sha256": response_hash,
            "context_provenance_sha256": "2" * 64,
            "reflection_path": reflection_path,
            "reflection_sha256": response_hash,
            "declaration": declaration,
            "declaration_sha256": train.sha256(declaration.encode()),
            "inquiry_step": step,
            "inquiry_step_sha256": train.sha256(train.canonical(step)),
            "summary": train.inquiry_summary(step),
            "summary_sha256": train.sha256(train.inquiry_summary(step).encode()),
            "prior_entry_sha256": prior_hash,
            "mechanical_predecessor": prior_id,
            "semantic_parent_step_id": parent,
            "provenance": train.authored_provenance(trigger_kind),
            "authority": "signed_authored_inquiry_not_hidden_chain_of_thought_not_code_authority",
        }
        envelope = self.signed(train.ENTRY_ENVELOPE_SCHEMA, core)
        entry_hash = train.sha256(train.canonical(envelope))
        return envelope, entry_hash, reflection

    def write_ledger(
        self, *, trigger_kind: str = "scheduled"
    ) -> tuple[dict[str, object], str, bytes]:
        entry, entry_hash, reflection = self.add_entry(trigger_kind=trigger_kind)
        raw = train.canonical(entry) + b"\n"
        segment = self.inquiry / "segments/segment-00000000000000000001.jsonl"
        segment.write_bytes(raw)
        segment.chmod(0o640)
        core = entry["core"]
        head_core = {
            "schema": train.HEAD_SCHEMA,
            "appliance_id": self.appliance,
            "entry_count": 1,
            "segment": 1,
            "entry_index": 1,
            "signed_entry_id": core["signed_entry_id"],
            "entry_sha256": entry_hash,
            "segment_bytes": len(raw),
        }
        head = self.signed(train.HEAD_ENVELOPE_SCHEMA, head_core)
        (self.inquiry / "head.json").write_bytes(train.canonical(head))
        (self.inquiry / "head.json").chmod(0o640)
        current_core = {
            "schema": train.CURRENT_SCHEMA,
            "appliance_id": self.appliance,
            "signed_entry_id": core["signed_entry_id"],
            "step_id": core["step_id"],
            "admission_id": core["admission_id"],
            "recorded_at_unix_ms": core["recorded_at_unix_ms"],
            "summary": core["summary"],
            "summary_sha256": core["summary_sha256"],
            "inquiry_step": core["inquiry_step"],
            "inquiry_step_sha256": core["inquiry_step_sha256"],
            "declaration_sha256": core["declaration_sha256"],
            "response_sha256": core["response_sha256"],
            "trace": core["trace"],
            "trigger_kind": core["trigger_kind"],
            "due_nonce": core["due_nonce"],
            "trigger_nonce": core["trigger_nonce"],
            "reflection_path": core["reflection_path"],
            "reflection_sha256": core["reflection_sha256"],
            "ledger": {
                "segment": 1,
                "entry_index": 1,
                "prior_entry_sha256": core["prior_entry_sha256"],
                "entry_sha256": entry_hash,
                "key_id": entry["auth"]["key_id"],
                "signature_algorithm": entry["auth"]["algorithm"],
                "signature": entry["auth"]["signature"],
            },
            "provenance": core["provenance"],
            "authority": "immutable_steward_signed_bounded_inquiry_projection_observational_only",
        }
        message = train.canonical(current_core)
        current = {
            **current_core,
            "core_sha256": train.sha256(message),
            "auth": {
                "algorithm": "ed25519",
                "key_id": f"ed25519:{train.sha256(self.public)[:16]}",
                "signature": self.sign(message).hex(),
            },
        }
        current_path = (
            self.workspace
            / "runtime/scheduled-introspection/projection/inquiry-current.json"
        )
        current_path.write_bytes(train.canonical(current))
        current_path.chmod(0o640)
        return entry, entry_hash, reflection

    def write_attestation(
        self, entry: dict[str, object], receipt: dict[str, object]
    ) -> None:
        core = entry["core"]
        reflection = self.workspace / str(core["reflection_path"])
        metadata = reflection.with_suffix(".json")
        metadata.write_bytes(b"{}")
        metadata.chmod(0o640)
        attestation_core = {
            "schema": train.AUTHORSHIP_CORE_SCHEMA,
            "appliance_id": self.appliance,
            "due_nonce": core["due_nonce"],
            "trigger_kind": core["trigger_kind"],
            "trigger_nonce": core["trigger_nonce"],
            "due_at_unix_ms": core["recorded_at_unix_ms"],
            "started_at_unix_ms": int(core["recorded_at_unix_ms"]) - 1_000,
            "completed_at_unix_ms": core["recorded_at_unix_ms"],
            "terminal_status": "model_authored_structured",
            "model": "qwen-test",
            "prompt_sha256": core["prompt_sha256"],
            "response_sha256": core["response_sha256"],
            "reflection_path": core["reflection_path"],
            "reflection_sha256": core["reflection_sha256"],
            "reflection_metadata_sha256": train.sha256(metadata.read_bytes()),
            "continuity_projection_sha256": "3" * 64,
            "inquiry_current_projection_sha256": "4" * 64,
            "signed_entry_id": core["signed_entry_id"],
            "step_id": core["step_id"],
            "admission_id": core["admission_id"],
            "inquiry_step_sha256": core["inquiry_step_sha256"],
            "inquiry_declaration_sha256": core["declaration_sha256"],
            "state_projection_sha256": "5" * 64,
            "terminal_receipt_sha256": train.sha256(train.canonical(receipt)),
            "context_provenance_sha256": core["context_provenance_sha256"],
            "candidate_id": None,
            "candidate_digest": None,
            "trace": core["trace"],
            "provenance": core["provenance"],
            "authority": "immutable_steward_signed_exact_authorship_join",
        }
        unsigned = {
            "schema": train.AUTHORSHIP_ENVELOPE_SCHEMA,
            "core": attestation_core,
        }
        envelope = {
            **unsigned,
            "auth": {
                "algorithm": "ed25519",
                "key_id": f"ed25519:{train.sha256(self.public)[:16]}",
                "signature": self.sign(train.canonical(unsigned)).hex(),
            },
        }
        path = (
            self.workspace
            / "introspections/scheduled"
            / f"authorship_attestation_{core['due_nonce']}_{core['response_sha256']}.json"
        )
        path.write_bytes(train.canonical(envelope))
        path.chmod(0o640)

    def admission_state(
        self,
        inquiry_event: dict[str, object],
        *,
        status: str,
        admitted_at: int,
        generation: str | None = None,
        sequence: int | None = None,
    ) -> dict[str, object]:
        terminal = status != "queued"
        trace = inquiry_event.get("trace")
        trace_id = (
            trace.get("trace_id")
            if isinstance(trace, dict)
            else inquiry_event.get("trace_id")
        )
        return {
            "schema": train.ADMISSION_SCHEMA,
            "continuity_admitted": True,
            "admitted_at_unix_ms": admitted_at,
            "admission_id": inquiry_event["admission_id"],
            "signed_entry_id": inquiry_event["signed_entry_id"],
            "last_response_sha256": inquiry_event["response_sha256"],
            "last_summary_sha256": inquiry_event["summary_sha256"],
            "last_trace_id": trace_id,
            "last_due_nonce": inquiry_event["due_nonce"],
            "source_class": (
                "scheduled_inquiry"
                if inquiry_event["trigger_kind"] == "scheduled"
                else "evidence_integration"
            ),
            "reservoir_delivery": status,
            "queued_at_unix_ms": admitted_at + 10,
            "terminal_at_unix_ms": admitted_at + 20 if terminal else None,
            "reservoir_generation": generation if status == "acknowledged" else None,
            "reservoir_sequence": sequence if status == "acknowledged" else None,
            "vector_sha256": "b" * 64,
            "migrated_legacy_schema": None,
            "provenance": train.authored_provenance(
                inquiry_event["trigger_kind"]
            ),
            "authority": "verified_signed_inquiry_observational_only",
        }

    def write_admission_receipts(
        self, records: list[tuple[str, int, dict[str, object]]]
    ) -> None:
        path = (
            self.workspace
            / "runtime/scheduled-introspection/admission/receipts.jsonl"
        )
        path.write_text(
            "".join(
                json.dumps(
                    {
                        "schema": train.ADMISSION_RECEIPT_SCHEMA,
                        "event": event,
                        "recorded_at_unix_ms": recorded_at,
                        "state": state,
                    }
                )
                + "\n"
                for event, recorded_at, state in records
            )
        )
        path.chmod(0o600)

    def test_full_chain_exact_prose_and_derived_events(self) -> None:
        entry, _entry_hash, reflection = self.write_ledger()
        core = entry["core"]
        clean_trace = {
            "schema_version": 1,
            "trace_id": "00000000-0000-4000-8000-000000000061",
            "turn_id": "00000000-0000-4000-8000-000000000062",
            "span_id": "00000000-0000-4000-8000-000000000063",
            "session_id": "clean-source-session",
        }
        receipt = {
            "schema": "astrid_edge_scheduled_introspection_v2",
            "status": "model_authored_structured",
            "completed_at_unix_ms": 1_800_000,
            "signed_entry_id": core["signed_entry_id"],
            "step_id": core["step_id"],
            "admission_id": core["admission_id"],
            "response_sha256": core["response_sha256"],
            "trace": core["trace"],
            "provenance": core["provenance"],
            "tools_used": ["read_owned_continuity"],
            "source_review_relation": "separate_clean_source_review",
            "source_review": {
                "status": "candidate_attested",
                "trace": clean_trace,
                "response_sha256": "6" * 64,
                "prompt_sha256": "7" * 64,
                "candidate_attested": True,
                "failure_class": None,
                "authority": (
                    "separate_clean_source_review_fresh_context_"
                    "candidate_authority_only_when_attested"
                ),
            },
            "candidate_id": "candidate-clean-only-canary",
            "candidate_digest": "a" * 64,
        }
        (self.workspace / "introspections/scheduled/receipts.jsonl").write_text(
            json.dumps(receipt) + "\n"
        )
        (self.workspace / "introspections/scheduled/receipts.jsonl").chmod(0o640)
        self.write_attestation(entry, receipt)
        queued = self.admission_state(
            core, status="queued", admitted_at=1_800_050
        )
        admission = self.admission_state(
            core,
            status="acknowledged",
            admitted_at=1_800_050,
            generation="reservoir-one",
            sequence=12,
        )
        self.write_admission_receipts(
            [
                ("queued", 1_800_060, queued),
                ("acknowledged", 1_800_100, admission),
            ]
        )
        admission_path = self.workspace / "runtime/scheduled-introspection/admission/state.json"
        admission_path.write_text(json.dumps(admission))
        admission_path.chmod(0o600)
        report = train.collect_train(
            self.workspace,
            inquiry_root=self.inquiry,
            verify_key=self.key,
            appliance_id=self.appliance,
            full=True,
        )
        self.assertEqual(report["integrity"], "full_signed_hash_chain_verified")
        self.assertIsNone(report["degraded_reason"])
        step = next(event for event in report["events"] if event["kind"] == "inquiry_step")
        self.assertEqual(step["reflection_text"], reflection.decode())
        self.assertEqual(step["provenance_class"], "astrid_authored")
        self.assertIn("model_tool_request", {event["kind"] for event in report["events"]})
        clean = next(
            event
            for event in report["events"]
            if event["kind"] == "clean_source_review"
        )
        self.assertEqual(clean["trace_id"], clean_trace["trace_id"])
        self.assertEqual(clean["turn_id"], clean_trace["turn_id"])
        self.assertEqual(clean["span_id"], clean_trace["span_id"])
        self.assertEqual(clean["session_id"], clean_trace["session_id"])
        self.assertEqual(clean["response_sha256"], "6" * 64)
        self.assertEqual(clean["prompt_sha256"], "7" * 64)
        self.assertEqual(clean["candidate_id"], "candidate-clean-only-canary")
        self.assertNotIn("thread_id", clean)
        self.assertNotIn("step_id", clean)
        clean_json = json.dumps(clean, sort_keys=True)
        self.assertNotIn(core["trace"]["trace_id"], clean_json)
        self.assertNotIn(core["trace"]["turn_id"], clean_json)
        self.assertNotIn(core["trace"]["span_id"], clean_json)
        self.assertNotIn(core["trace"]["session_id"], clean_json)
        self.assertNotIn(core["step_id"], clean_json)
        self.assertNotIn("thread-latency", clean_json)
        ack = next(
            event
            for event in report["events"]
            if event["kind"] == "semantic_admission"
            and event["status"] == "acknowledged"
        )
        self.assertEqual(ack["status"], "acknowledged")
        self.assertEqual(ack["provenance_class"], "executor_outcome")

    def test_full_text_mode_is_reversible_and_does_not_truncate_exact_prose(self) -> None:
        prose = "first line\n" + "x" * 900 + "\nlast exact line é\u202e"
        event = {
            "timestamp_unix_ms": 1_800_000,
            "kind": "scheduled_reflection",
            "response_sha256": "a" * 64,
            "reflection_text": prose,
            "provenance_class": "astrid_authored",
        }
        report = {
            "integrity": "full_signed_hash_chain_verified",
            "inquiry_step_count": 0,
        }
        line = next(
            item
            for item in train.text_lines(report, [event])
            if "exact_prose_json=" in item
        )
        encoded = line.split(" exact_prose_json=", 1)[1].rsplit(" class=", 1)[0]
        self.assertEqual(json.loads(encoded), prose)

    def test_malformed_clean_review_never_borrows_rich_trace(self) -> None:
        entry, _entry_hash, _reflection = self.write_ledger()
        core = entry["core"]
        receipt = {
            "schema": "astrid_edge_scheduled_introspection_v2",
            "status": "model_authored_structured",
            "completed_at_unix_ms": 1_800_000,
            "signed_entry_id": core["signed_entry_id"],
            "step_id": core["step_id"],
            "admission_id": core["admission_id"],
            "response_sha256": core["response_sha256"],
            "trace": core["trace"],
            "provenance": core["provenance"],
            "tools_used": [],
            "source_review_relation": "separate_clean_source_review",
            "source_review": {
                "status": "candidate_attested",
                "trace": {"trace_id": core["trace"]["trace_id"]},
                "response_sha256": "6" * 64,
                "prompt_sha256": "7" * 64,
                "candidate_attested": True,
                "failure_class": None,
                "authority": (
                    "separate_clean_source_review_fresh_context_"
                    "candidate_authority_only_when_attested"
                ),
            },
            "candidate_id": "candidate-rich-confusion",
            "candidate_digest": "a" * 64,
        }
        receipts = self.workspace / "introspections/scheduled/receipts.jsonl"
        receipts.write_text(json.dumps(receipt) + "\n")
        receipts.chmod(0o640)
        self.write_attestation(entry, receipt)

        report = train.collect_train(
            self.workspace,
            inquiry_root=self.inquiry,
            verify_key=self.key,
            appliance_id=self.appliance,
        )
        clean = next(
            event
            for event in report["events"]
            if event["kind"] == "clean_source_review"
        )

        self.assertEqual(
            clean["status"], "unattributed_invalid_source_review_projection"
        )
        self.assertEqual(
            clean["authority"],
            "unattributed_source_review_projection_no_causal_or_candidate_claim",
        )
        self.assertIsNone(clean["trace_id"])
        self.assertIsNone(clean["turn_id"])
        self.assertIsNone(clean["span_id"])
        self.assertIsNone(clean["session_id"])
        self.assertIsNone(clean["response_sha256"])
        self.assertIsNone(clean["candidate_id"])
        self.assertNotIn("step_id", clean)
        self.assertNotIn("thread_id", clean)

    def test_admission_ledger_preserves_multiple_inquiry_terminal_states(self) -> None:
        inquiry: list[dict[str, object]] = []
        for index in (1, 2):
            inquiry.append(
                {
                    "signed_entry_id": f"inquiry-entry-{index}",
                    "admission_id": f"inquiry-admission-{index}",
                    "response_sha256": str(index) * 64,
                    "summary_sha256": str(index + 2) * 64,
                    "trace_id": f"00000000-0000-4000-8000-{index:012d}",
                    "turn_id": f"00000000-0000-4000-8001-{index:012d}",
                    "span_id": f"00000000-0000-4000-8002-{index:012d}",
                    "session_id": f"scheduled-session-{index}",
                    "due_nonce": f"due-{index}",
                    "trigger_kind": "scheduled",
                    "step_id": f"inquiry-step-{index}",
                    "thread_id": f"thread-{index}",
                }
            )
        queued_one = self.admission_state(
            inquiry[0], status="queued", admitted_at=3_000_000
        )
        acknowledged = self.admission_state(
            inquiry[0],
            status="acknowledged",
            admitted_at=3_000_000,
            generation="reservoir-one",
            sequence=21,
        )
        queued_two = self.admission_state(
            inquiry[1], status="queued", admitted_at=3_100_000
        )
        failed = self.admission_state(
            inquiry[1], status="failed", admitted_at=3_100_000
        )
        self.write_admission_receipts(
            [
                ("queued", 3_000_010, queued_one),
                ("acknowledged", 3_000_020, acknowledged),
                ("queued", 3_100_010, queued_two),
                ("failed", 3_100_020, failed),
            ]
        )

        events, index = train.semantic_admission_receipt_history(
            self.workspace, inquiry
        )

        self.assertEqual(
            [event["status"] for event in events],
            ["queued", "acknowledged", "queued", "failed"],
        )
        self.assertEqual(
            index["inquiry-admission-1"]["state"]["reservoir_delivery"],
            "acknowledged",
        )
        self.assertEqual(
            index["inquiry-admission-2"]["state"]["reservoir_delivery"],
            "failed",
        )
        acknowledged_event = events[1]
        self.assertEqual(acknowledged_event["reservoir_generation"], "reservoir-one")
        self.assertEqual(acknowledged_event["reservoir_sequence"], 21)
        self.assertEqual(acknowledged_event["thread_id"], "thread-1")

    def test_admission_ledger_tamper_and_torn_tail_fail_closed(self) -> None:
        inquiry = {
            "signed_entry_id": "inquiry-entry-one",
            "admission_id": "inquiry-admission-one",
            "response_sha256": "1" * 64,
            "summary_sha256": "3" * 64,
            "trace_id": "00000000-0000-4000-8000-000000000071",
            "turn_id": "00000000-0000-4000-8000-000000000072",
            "span_id": "00000000-0000-4000-8000-000000000073",
            "session_id": "scheduled-session-one",
            "due_nonce": "due-one",
            "trigger_kind": "scheduled",
            "step_id": "inquiry-step-one",
            "thread_id": "thread-one",
        }
        queued = self.admission_state(
            inquiry, status="queued", admitted_at=3_200_000
        )
        self.write_admission_receipts([("queued", 3_200_010, queued)])
        ledger = (
            self.workspace
            / "runtime/scheduled-introspection/admission/receipts.jsonl"
        )
        ledger.write_bytes(ledger.read_bytes().removesuffix(b"\n"))
        with self.assertRaisesRegex(train.TrainError, "torn tail"):
            train.semantic_admission_receipt_history(self.workspace, [inquiry])

        tampered = dict(queued)
        tampered["last_response_sha256"] = "9" * 64
        self.write_admission_receipts([("queued", 3_200_010, tampered)])
        with self.assertRaisesRegex(train.TrainError, "does not bind"):
            train.semantic_admission_receipt_history(self.workspace, [inquiry])

    def test_tampered_chain_never_claims_validity(self) -> None:
        self.write_ledger()
        segment = self.inquiry / "segments/segment-00000000000000000001.jsonl"
        value = json.loads(segment.read_text())
        value["core"]["inquiry_step"]["decision"] = "Tampered"
        segment.write_text(json.dumps(value) + "\n")
        segment.chmod(0o640)
        with self.assertRaises(train.TrainError):
            train.full_chain_events(
                self.workspace,
                self.inquiry,
                self.appliance,
                self.public,
                f"ed25519:{train.sha256(self.public)[:16]}",
                full=False,
            )

    def test_evidence_integration_provenance_is_distinct_and_verified(self) -> None:
        entry, _entry_hash, _reflection = self.write_ledger(
            trigger_kind="evidence_integration"
        )
        events = train.full_chain_events(
            self.workspace,
            self.inquiry,
            self.appliance,
            self.public,
            f"ed25519:{train.sha256(self.public)[:16]}",
            full=False,
        )
        self.assertEqual(events[0]["trigger_kind"], "evidence_integration")
        self.assertEqual(
            entry["core"]["provenance"],
            "model_authored_runtime_evidence_integration",
        )

    def test_v7_evidence_and_belief_are_distinct_provenance(self) -> None:
        trace = {
            "schema_version": 1,
            "trace_id": "00000000-0000-4000-8000-000000000021",
            "turn_id": "00000000-0000-4000-8000-000000000022",
            "span_id": "00000000-0000-4000-8000-000000000023",
            "parent_span_id": "00000000-0000-4000-8000-000000000024",
            "session_id": "ordinary-session",
        }
        row = {
            "schema": "astrid_edge_thread_state_v7",
            "revision": 3,
            "thread_id": "thread-latency",
            "status": "active",
            "event": "thread_belief_updated",
            "updated_at_unix_ms": 1_900_000,
            "last_admitted_inquiry_step_id": "inquiry-step-" + "a" * 64,
            "trace": trace,
            "evidence_records": [
                {
                    "evidence_id": "study-latency-1",
                    "kind": "completed_study",
                    "epistemic_status": "verified",
                    "reference": "study-latency-1.md",
                    "summary": "Latency remained bounded.",
                    "source": "machine_study",
                    "captured_at_unix_ms": 1_850_000,
                    "sha256": "c" * 64,
                    "eligible_for_belief_update": True,
                }
            ],
            "beliefs": [
                {
                    "revision_id": "belief-revision-1",
                    "belief_id": "belief-latency",
                    "thread_id": "thread-latency",
                    "operation": "supported",
                    "claim": "Latency bounds inquiry depth.",
                    "evidence_ids": ["study-latency-1"],
                    "prior_revision_id": "belief-latency-r1",
                    "recorded_at_unix_ms": 1_900_000,
                    "response_sha256": "d" * 64,
                    "source": "exact_unrepaired_update_belief_action",
                }
            ],
        }
        path = self.workspace / "autonomous/thread_state.jsonl"
        path.write_text(json.dumps(row) + "\n")
        path.chmod(0o600)
        events = train.thread_events(self.workspace)
        evidence = next(event for event in events if event["kind"] == "evidence_arrival")
        belief = next(event for event in events if event["kind"] == "belief_revision")
        self.assertFalse(evidence["authored"])
        self.assertEqual(evidence["provenance_class"], "machine_evidence")
        self.assertFalse(belief["authored"])
        self.assertEqual(
            belief["authority"], "unverified_thread_projection_not_authorship"
        )
        self.assertEqual(belief["evidence_ids"], ["study-latency-1"])
        receipt = {
            "schema": "astrid_edge_action_receipt_v5",
            "recorded_at_unix_ms": 1_900_000,
            "session_id": "ordinary-session",
            "response_sha256": "d" * 64,
            "declared_next": (
                "UPDATE_BELIEF belief-latency WITH study-latency-1 :: supported :: "
                "Latency bounds inquiry depth."
            ),
            "decision_source": "astrid_declared",
            "status": "executed",
            "outcome": "inquiry_belief_updated",
            "recovery_reason": None,
            "trace": trace,
            "authority": "validated_model_next_with_optional_syntax_only_repair_owned_workspace_only",
        }
        receipts = self.workspace / "actions/receipts.jsonl"
        receipts.parent.mkdir()
        receipts.write_text(json.dumps(receipt) + "\n")
        receipts.chmod(0o600)
        verified = next(
            event
            for event in train.thread_events(self.workspace)
            if event["kind"] == "belief_revision"
        )
        self.assertTrue(verified["authored"])
        self.assertEqual(
            verified["authority"], "exact_unrepaired_action_receipt_projection"
        )

    def test_evidence_arrival_uses_exact_child_trace_and_parent_response(self) -> None:
        row_trace = {
            "schema_version": 1,
            "trace_id": "00000000-0000-4000-8000-000000000031",
            "turn_id": "00000000-0000-4000-8000-000000000032",
            "span_id": "00000000-0000-4000-8000-000000000033",
            "session_id": "unrelated-thread-snapshot",
            "chain_id": "unrelated-chain",
        }
        evidence_trace = {
            "schema_version": 1,
            "trace_id": "00000000-0000-4000-8000-000000000041",
            "turn_id": "00000000-0000-4000-8000-000000000042",
            "span_id": "00000000-0000-4000-8000-000000000043",
            "parent_span_id": "00000000-0000-4000-8000-000000000044",
            "session_id": "async-study-session",
            "chain_id": "study-chain",
        }
        parent_response_sha256 = "e" * 64
        row = {
            "schema": "astrid_edge_thread_state_v7",
            "revision": 4,
            "thread_id": "thread-latency",
            "status": "active",
            "event": "evidence_recorded",
            "updated_at_unix_ms": 2_000_000,
            "trace": row_trace,
            "evidence_records": [
                {
                    "evidence_id": "study-latency-async",
                    "kind": "completed_study",
                    "epistemic_status": "verified",
                    "captured_at_unix_ms": 1_990_000,
                    "trace": evidence_trace,
                    "parent_response_sha256": parent_response_sha256,
                    "eligible_for_belief_update": True,
                }
            ],
        }
        path = self.workspace / "autonomous/thread_state.jsonl"
        path.write_text(json.dumps(row) + "\n")
        path.chmod(0o600)

        evidence = next(
            event
            for event in train.thread_events(self.workspace)
            if event["kind"] == "evidence_arrival"
        )

        self.assertEqual(evidence["trace_id"], evidence_trace["trace_id"])
        self.assertEqual(evidence["turn_id"], evidence_trace["turn_id"])
        self.assertEqual(evidence["span_id"], evidence_trace["span_id"])
        self.assertEqual(evidence["session_id"], evidence_trace["session_id"])
        self.assertEqual(evidence["chain_id"], evidence_trace["chain_id"])
        self.assertNotEqual(evidence["trace_id"], row_trace["trace_id"])
        self.assertEqual(
            evidence["parent_response_sha256"], parent_response_sha256
        )

    def test_legacy_or_malformed_evidence_trace_is_unattributed(self) -> None:
        unrelated_trace = {
            "schema_version": 1,
            "trace_id": "00000000-0000-4000-8000-000000000051",
            "turn_id": "00000000-0000-4000-8000-000000000052",
            "span_id": "00000000-0000-4000-8000-000000000053",
            "session_id": "unrelated-current-turn",
            "chain_id": "unrelated-chain",
        }
        row = {
            "schema": "astrid_edge_thread_state_v7",
            "revision": 5,
            "thread_id": "thread-latency",
            "status": "active",
            "event": "evidence_recorded",
            "updated_at_unix_ms": 2_100_000,
            "trace": unrelated_trace,
            "evidence_records": [
                {
                    "evidence_id": "legacy-evidence",
                    "kind": "verified_source",
                    "captured_at_unix_ms": 2_080_000,
                },
                {
                    "evidence_id": "malformed-trace-evidence",
                    "kind": "completed_measurement",
                    "captured_at_unix_ms": 2_090_000,
                    "trace": {
                        "schema_version": 1,
                        "trace_id": unrelated_trace["trace_id"],
                        "span_id": "not-a-uuid",
                        "session_id": "malformed-evidence",
                    },
                },
            ],
        }
        path = self.workspace / "autonomous/thread_state.jsonl"
        path.write_text(json.dumps(row) + "\n")
        path.chmod(0o600)

        evidence_events = {
            event["evidence_id"]: event
            for event in train.thread_events(self.workspace)
            if event["kind"] == "evidence_arrival"
        }

        for evidence_id in ("legacy-evidence", "malformed-trace-evidence"):
            event = evidence_events[evidence_id]
            self.assertIsNone(event["trace_id"])
            self.assertIsNone(event["turn_id"])
            self.assertIsNone(event["span_id"])
            self.assertIsNone(event["session_id"])
            self.assertIsNone(event["chain_id"])
            self.assertIsNone(event["parent_response_sha256"])

    def test_same_response_hash_keeps_distinct_exact_action_traces(self) -> None:
        response_sha256 = "f" * 64
        declaration = "PROPOSE Repeated prose can still have distinct causal turns."
        rows: list[dict[str, object]] = []
        receipts: list[dict[str, object]] = []
        for index in (1, 2):
            trace = {
                "schema_version": 1,
                "trace_id": f"00000000-0000-4000-8000-{index:012d}",
                "turn_id": f"00000000-0000-4000-8001-{index:012d}",
                "span_id": f"00000000-0000-4000-8002-{index:012d}",
                "session_id": f"ordinary-session-{index}",
            }
            rows.append(
                {
                    "schema": "astrid_edge_thread_state_v7",
                    "revision": 10 + index,
                    "thread_id": "thread-repeat",
                    "status": "active",
                    "event": "action_belief_proposed",
                    "last_action": declaration.removeprefix("PROPOSE "),
                    "updated_at_unix_ms": 2_200_000 + index,
                    "response_sha256": response_sha256,
                    "trace": trace,
                    "beliefs": [
                        {
                            "revision_id": f"belief-revision-repeat-{index}",
                            "belief_id": f"belief-repeat-{index}",
                            "thread_id": "thread-repeat",
                            "operation": "propose",
                            "claim": declaration.removeprefix("PROPOSE "),
                            "evidence_ids": [],
                            "recorded_at_unix_ms": 2_200_000 + index,
                            "response_sha256": response_sha256,
                            "source": "authored_propose_action",
                        }
                    ],
                }
            )
            receipts.append(
                {
                    "schema": "astrid_edge_action_receipt_v5",
                    "recorded_at_unix_ms": 2_200_000 + index,
                    "session_id": trace["session_id"],
                    "response_sha256": response_sha256,
                    "declared_next": declaration,
                    "decision_source": "astrid_declared",
                    "status": "executed",
                    "outcome": "inquiry_belief_proposed",
                    "recovery_reason": None,
                    "trace": trace,
                    "authority": "validated_model_next_with_optional_syntax_only_repair_owned_workspace_only",
                }
            )
        thread_path = self.workspace / "autonomous/thread_state.jsonl"
        thread_path.write_text("".join(json.dumps(row) + "\n" for row in rows))
        thread_path.chmod(0o600)
        receipt_path = self.workspace / "actions/receipts.jsonl"
        receipt_path.parent.mkdir()
        receipt_path.write_text(
            "".join(json.dumps(receipt) + "\n" for receipt in receipts)
        )
        receipt_path.chmod(0o600)

        beliefs = [
            event
            for event in train.thread_events(self.workspace)
            if event["kind"] == "belief_revision"
        ]

        self.assertEqual(len(beliefs), 2)
        self.assertTrue(all(event["authored"] for event in beliefs))
        self.assertEqual(
            {event["authority"] for event in beliefs},
            {"exact_unrepaired_action_receipt_projection"},
        )

    def test_signed_unstructured_reflection_has_no_continuity(self) -> None:
        body = b"I noticed something, but my terminal declaration was malformed.\n"
        response_hash = train.sha256(body)
        relative = "introspections/scheduled/reflection_due-2000_unstructured.md"
        reflection = self.workspace / relative
        reflection.write_bytes(body)
        reflection.chmod(0o640)
        metadata = reflection.with_suffix(".json")
        metadata.write_bytes(b"{}")
        metadata.chmod(0o640)
        core = {
            "schema": train.AUTHORSHIP_CORE_SCHEMA,
            "appliance_id": self.appliance,
            "due_nonce": "due-2000",
            "trigger_kind": "scheduled",
            "trigger_nonce": "scheduled-due-2000",
            "due_at_unix_ms": 2_000_000,
            "started_at_unix_ms": 1_999_000,
            "terminal_status": "model_authored_unstructured",
            "completed_at_unix_ms": 2_000_000,
            "model": "qwen-test",
            "prompt_sha256": "1" * 64,
            "response_sha256": response_hash,
            "reflection_path": relative,
            "reflection_sha256": response_hash,
            "reflection_metadata_sha256": train.sha256(metadata.read_bytes()),
            "continuity_projection_sha256": None,
            "inquiry_current_projection_sha256": None,
            "signed_entry_id": None,
            "step_id": None,
            "admission_id": None,
            "inquiry_step_sha256": None,
            "inquiry_declaration_sha256": None,
            "state_projection_sha256": "2" * 64,
            "terminal_receipt_sha256": "3" * 64,
            "context_provenance_sha256": "4" * 64,
            "candidate_id": None,
            "candidate_digest": None,
            "trace": {
                "schema_version": 1,
                "trace_id": "00000000-0000-4000-8000-000000000011",
                "turn_id": "00000000-0000-4000-8000-000000000012",
                "span_id": "00000000-0000-4000-8000-000000000013",
                "session_id": "scheduled-session",
            },
            "provenance": "model_authored_runtime_scheduled",
            "authority": "immutable_steward_signed_exact_authorship_join",
        }
        unsigned = {"schema": train.AUTHORSHIP_ENVELOPE_SCHEMA, "core": core}
        envelope = {
            **unsigned,
            "auth": {
                "algorithm": "ed25519",
                "key_id": f"ed25519:{train.sha256(self.public)[:16]}",
                "signature": self.sign(train.canonical(unsigned)).hex(),
            },
        }
        attestation = (
            self.workspace
            / f"introspections/scheduled/authorship_attestation_due-2000_{response_hash}.json"
        )
        attestation.write_bytes(train.canonical(envelope))
        attestation.chmod(0o640)
        report = train.collect_train(
            self.workspace,
            inquiry_root=self.inquiry,
            verify_key=self.key,
            appliance_id=self.appliance,
            full=True,
        )
        event = next(
            item
            for item in report["events"]
            if item["kind"] == "scheduled_reflection"
        )
        self.assertEqual(report["inquiry_step_count"], 0)
        self.assertTrue(event["authored"])
        self.assertFalse(event["continuity_admitted"])
        self.assertEqual(event["reflection_text"].encode(), body)

    def test_filters_are_identifier_based_and_deterministic(self) -> None:
        event = {
            "timestamp_unix_ms": 100,
            "kind": "inquiry_step",
            "thread_id": "thread-one",
            "step_id": "step-one",
        }
        report = {"events": [event, {**event, "thread_id": "thread-two", "step_id": "step-two"}]}
        args = train.parser().parse_args(
            ["--thread-id", "thread-one", "--step-id", "step-one", "--kind", "inquiry_step"]
        )
        self.assertEqual(train.selected(report, args, 0, 200), [event])

    def test_invalid_attestation_fails_closed_with_exact_path(self) -> None:
        entry, _entry_hash, _reflection = self.write_ledger()
        core = entry["core"]
        receipt = {
            "schema": "astrid_edge_scheduled_introspection_v2",
            "status": "model_authored_structured",
            "completed_at_unix_ms": 1_800_000,
            "signed_entry_id": core["signed_entry_id"],
            "step_id": core["step_id"],
            "admission_id": core["admission_id"],
            "response_sha256": core["response_sha256"],
            "trace": core["trace"],
            "provenance": core["provenance"],
            "tools_used": [],
        }
        receipts = self.workspace / "introspections/scheduled/receipts.jsonl"
        receipts.write_text(json.dumps(receipt) + "\n")
        receipts.chmod(0o640)
        self.write_attestation(entry, receipt)
        invalid = (
            self.workspace
            / "introspections/scheduled"
            / f"authorship_attestation_due-bad_{'f' * 64}.json"
        )
        invalid.write_text("{not-json")
        invalid.chmod(0o640)
        report = train.collect_train(
            self.workspace,
            inquiry_root=self.inquiry,
            verify_key=self.key,
            appliance_id=self.appliance,
        )
        self.assertEqual(report["integrity"], "invalid_protected_history")
        self.assertEqual(report["invalid_record_count"], 1)
        self.assertEqual(report["invalid_records"][0]["path"], str(invalid))
        self.assertFalse(
            any(event.get("authored") is True for event in report["events"])
        )

    def test_malformed_thread_jsonl_invalidates_projection_not_authorship(self) -> None:
        entry, _entry_hash, _reflection = self.write_ledger()
        core = entry["core"]
        receipt = {
            "schema": "astrid_edge_scheduled_introspection_v2",
            "status": "model_authored_structured",
            "completed_at_unix_ms": 1_800_000,
            "signed_entry_id": core["signed_entry_id"],
            "step_id": core["step_id"],
            "admission_id": core["admission_id"],
            "response_sha256": core["response_sha256"],
            "trace": core["trace"],
            "provenance": core["provenance"],
            "tools_used": [],
        }
        receipts = self.workspace / "introspections/scheduled/receipts.jsonl"
        receipts.write_text(json.dumps(receipt) + "\n")
        receipts.chmod(0o640)
        self.write_attestation(entry, receipt)
        thread = self.workspace / "autonomous/thread_state.jsonl"
        thread.write_text('{"schema":"astrid_edge_thread_state_v7"}')
        thread.chmod(0o600)
        report = train.collect_train(
            self.workspace,
            inquiry_root=self.inquiry,
            verify_key=self.key,
            appliance_id=self.appliance,
        )
        self.assertEqual(report["integrity"], "invalid_protected_history")
        self.assertIn("torn tail", report["invalid_records"][0]["reason"])
        self.assertEqual(report["invalid_records"][0]["path"], str(thread))

    def test_stable_read_detects_atomic_path_replacement(self) -> None:
        path = self.root / "protected.json"
        replacement = self.root / "replacement.json"
        path.write_bytes(b"first")
        replacement.write_bytes(b"other")
        path.chmod(0o600)
        replacement.chmod(0o600)
        original_read = os.read
        replaced = False

        def replacing_read(descriptor: int, maximum: int) -> bytes:
            nonlocal replaced
            block = original_read(descriptor, maximum)
            if block and not replaced:
                replaced = True
                os.replace(replacement, path)
            return block

        with mock.patch.object(os, "read", side_effect=replacing_read):
            with self.assertRaisesRegex(train.TrainError, "replaced"):
                train.stable_regular(path, 64)

    def test_full_reflection_read_rejects_intermediate_symlink(self) -> None:
        relative = "introspections/scheduled/reflection_safe.md"
        reflection = self.workspace / relative
        reflection.write_bytes(b"private authored prose")
        reflection.chmod(0o640)
        original = self.workspace / "introspections"
        moved = self.workspace / "introspections-real"
        original.rename(moved)
        original.symlink_to(moved, target_is_directory=True)

        with self.assertRaisesRegex(train.TrainError, "descriptor-anchored"):
            train.stable_scheduled_artifact(
                self.workspace,
                relative,
                ".md",
                train.REFLECTION_MAX_BYTES,
                private=True,
            )

    def test_full_reflection_read_detects_intermediate_path_replacement(self) -> None:
        relative = "introspections/scheduled/reflection_safe.md"
        reflection = self.workspace / relative
        reflection.write_bytes(b"private authored prose")
        reflection.chmod(0o640)
        scheduled = reflection.parent
        moved = scheduled.with_name("scheduled-original")
        original_read = os.read
        replaced = False

        def replacing_read(descriptor: int, maximum: int) -> bytes:
            nonlocal replaced
            block = original_read(descriptor, maximum)
            if block and not replaced:
                replaced = True
                scheduled.rename(moved)
                scheduled.mkdir(mode=0o750)
                replacement = scheduled / reflection.name
                replacement.write_bytes(b"replacement prose")
                replacement.chmod(0o640)
            return block

        with mock.patch.object(os, "read", side_effect=replacing_read):
            with self.assertRaisesRegex(train.TrainError, "replaced"):
                train.stable_scheduled_artifact(
                    self.workspace,
                    relative,
                    ".md",
                    train.REFLECTION_MAX_BYTES,
                    private=True,
                )


if __name__ == "__main__":
    unittest.main()
