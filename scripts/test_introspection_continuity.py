#!/usr/bin/env python3
"""Tests for deterministic introspection continuity cards."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest

try:
    from introspection_continuity import project, state_dir
    from experiential_epistemics import lint_workspace
except ModuleNotFoundError:
    from scripts.introspection_continuity import project, state_dir
    from scripts.experiential_epistemics import lint_workspace


def sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


class IntrospectionContinuityTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.workspace = Path(self.temp.name) / "workspace"
        self.introspections = self.workspace / "introspections"
        self.introspections.mkdir(parents=True)
        self.addressing = (
            self.workspace
            / "diagnostics/introspection_addressing_v1/status.json"
        )
        self.addressing.parent.mkdir(parents=True)
        self.source_sha = "a" * 64
        self.session_id = "b" * 64
        self.witness_id = "lsw_" + "c" * 64
        self.report_name = "introspection_astrid_llm_2000000000.txt"
        self.report_relative = f"introspections/{self.report_name}"
        self.report = self.introspections / self.report_name
        self.report_bytes = (
            "=== ASTRID INTROSPECTION ===\n"
            "Source: astrid:llm "
            "(/private/source/capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs)\n"
            "Source window: lines 1-400 of 997\n"
            "Source coverage schema: source_coverage_manifest_v2\n"
            f"Source read session: {self.session_id}\n"
            f"Source SHA-256: {self.source_sha}\n"
            "Timestamp: 2000000000\n"
            f"Lived-state witness: {self.witness_id}\n"
            "\n"
            "Observed:\nA bounded canonical report fixture.\n"
        ).encode()
        self.report.write_bytes(self.report_bytes)
        self.claim = {
            "claim_id": "c001",
            "summary": "A provider-neutral normalization boundary remains proposed.",
            "classification": "tier_5_wait",
            "disposition": "addressed_change",
            "grounded_disposition": (
                "Preserved as a non-live proposal pending separate operator approval."
            ),
            "authority": "proposal_only_live_provider_normalization_not_authorized",
            "evidence": [
                {
                    "kind": "steward_note",
                    "target": "docs/steward-notes/normalization-proposal.md",
                },
                {
                    "kind": "code",
                    "target": "capsules/spectral-bridge/src/llm/provider/transport.rs",
                },
                {
                    "kind": "external",
                    "target": "/private/sibling/evidence.json",
                },
            ],
        }
        self.artifact = {
            "introspection_id": self.report.stem,
            "filename": self.report.name,
            "relative_path": self.report_relative,
            "sha256": sha256(self.report_bytes),
            "full_read": True,
            "fully_addressed": True,
            "present_on_disk": True,
            "status": "addressed_change",
            "lived_state_witness_id": self.witness_id,
            "claims": {"c001": self.claim},
        }
        self._write_addressing({self.report.stem: self.artifact})

    def tearDown(self) -> None:
        self.temp.cleanup()

    def _write_addressing(self, artifacts: dict[str, object]) -> None:
        self.addressing.write_text(
            json.dumps({"schema": "fixture", "artifacts": artifacts}),
            encoding="utf-8",
        )

    def _card_paths(self) -> list[Path]:
        return sorted((state_dir(self.workspace) / "cards").glob("*.json"))

    def test_project_emits_deterministic_owner_only_card(self) -> None:
        first = project(self.workspace, write=True)
        card_path = self._card_paths()[0]
        first_bytes = {
            path.relative_to(state_dir(self.workspace)).as_posix(): path.read_bytes()
            for path in sorted(state_dir(self.workspace).rglob("*"))
            if path.is_file()
        }
        second = project(self.workspace, write=True)
        second_bytes = {
            path.relative_to(state_dir(self.workspace)).as_posix(): path.read_bytes()
            for path in sorted(state_dir(self.workspace).rglob("*"))
            if path.is_file()
        }
        self.assertEqual(first, second)
        self.assertEqual(first_bytes, second_bytes)
        self.assertEqual(first["summary"]["card_count"], 1)

        card = json.loads(card_path.read_text(encoding="utf-8"))
        self.assertEqual(card["source"]["sha256"], self.source_sha)
        self.assertEqual(card["source"]["read_session_id"], self.session_id)
        self.assertEqual(
            card["source"]["locator"],
            "capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs",
        )
        self.assertEqual(card["authority_waits"][0]["claim_id"], "c001")
        self.assertEqual(card["claims"][0]["omitted_evidence_ref_count"], 1)
        self.assertTrue(card["right_to_ignore"])
        self.assertTrue(card["silence_is_neutral"])
        self.assertFalse(card["felt_closure_inferred"])
        self.assertFalse(card["consent_inferred"])
        self.assertNotIn("/private/source", card_path.read_text(encoding="utf-8"))
        self.assertEqual(os.stat(card_path).st_mode & 0o777, 0o600)
        self.assertEqual(os.stat(card_path.parent).st_mode & 0o777, 0o700)

        index = json.loads(
            (state_dir(self.workspace) / "index.json").read_text(encoding="utf-8")
        )
        self.assertEqual(index["cards"][0]["card_sha256"], sha256(card_path.read_bytes()))
        lint = lint_workspace(self.workspace)
        self.assertTrue(lint["valid"], lint)

    def test_unread_and_incomplete_claim_records_are_not_projected(self) -> None:
        unread = dict(self.artifact)
        unread["introspection_id"] = "introspection_unread_2000000001"
        unread["full_read"] = False
        incomplete = dict(self.artifact)
        incomplete["introspection_id"] = "introspection_incomplete_2000000002"
        incomplete_claim = dict(self.claim)
        incomplete_claim.pop("grounded_disposition")
        incomplete["claims"] = {"c001": incomplete_claim}
        self._write_addressing(
            {
                unread["introspection_id"]: unread,
                incomplete["introspection_id"]: incomplete,
            }
        )
        status = project(self.workspace, write=True)
        self.assertEqual(status["summary"]["eligible_artifact_count"], 1)
        self.assertEqual(status["summary"]["card_count"], 0)
        self.assertEqual(status["summary"]["excluded_eligible_count"], 1)
        self.assertEqual(self._card_paths(), [])

    def test_changed_claim_replaces_stale_card(self) -> None:
        project(self.workspace, write=True)
        original = self._card_paths()[0]
        changed_artifact = dict(self.artifact)
        changed_claim = dict(self.claim)
        changed_claim["grounded_disposition"] = (
            "A corrected non-live disposition remains pending operator approval."
        )
        changed_artifact["claims"] = {"c001": changed_claim}
        self._write_addressing({self.report.stem: changed_artifact})
        project(self.workspace, write=True)
        current = self._card_paths()
        self.assertEqual(len(current), 1)
        self.assertNotEqual(original.name, current[0].name)
        self.assertFalse(original.exists())

    def test_report_hash_mismatch_never_copies_report(self) -> None:
        tampered = dict(self.artifact)
        tampered["sha256"] = "d" * 64
        self._write_addressing({self.report.stem: tampered})
        status = project(self.workspace, write=True)
        self.assertEqual(status["summary"]["card_count"], 0)
        self.assertEqual(status["summary"]["excluded_eligible_count"], 1)
        self.assertNotIn(
            "bounded canonical report fixture",
            (state_dir(self.workspace) / "status.json").read_text(encoding="utf-8"),
        )

    def test_bound_response_annotates_stable_card_without_inferring_closure(self) -> None:
        first = project(self.workspace, write=True)
        card_path = self._card_paths()[0]
        card_id = card_path.stem
        first_input_hash = first["input_hashes"][
            "diagnostics/introspection_continuity_v1/responses/*.json"
        ]["sha256"]
        current = self.introspections / "introspection_astrid_llm_2000000001.txt"
        response_status = "still_friction"
        response_line = 9
        current_relative = f"introspections/{current.name}"
        exact_response = f"Prior Evidence: {card_id} :: {response_status}"
        current_bytes = (
            "canonical current report without copied receipt prose\n"
            "source header two\n"
            "source header three\n"
            "source header four\n"
            "Observed:\n"
            "bounded current report line\n"
            "Likely Snags:\n"
            "bounded snag\n"
            f"{exact_response}\n"
        ).encode()
        current.write_bytes(current_bytes)
        current_sha = sha256(current_bytes)
        receipt_id = "icrv1_" + sha256(
            (
                f"{card_id}\0{current_relative}\0{current_sha}\0"
                f"{response_line}\0{response_status}"
            ).encode()
        )
        receipt = {
            "schema": "introspection_continuity_response_v1",
            "schema_version": 1,
            "receipt_id": receipt_id,
            "card_id": card_id,
            "assessment_status": response_status,
            "current_introspection": {
                "path": current_relative,
                "introspection_id": current.stem,
                "sha256": current_sha,
            },
            "response_line": response_line,
            "recorded_at_unix_ms": 2_000_000_001_000,
            "bound": True,
            "linked_prior_introspection_id": self.report.stem,
            "linked_claim_ids": ["c001"],
            "right_to_ignore": True,
            "silence_is_neutral": True,
            "mechanical_evidence_only": True,
            "felt_closure_inferred": False,
            "consent_inferred": False,
            "no_authority": True,
            "authority_boundary": (
                "response_is_evidence_only_not_felt_closure_control_approval_or_activation"
            ),
            "artifact_authority_state_v1": {
                "schema": "artifact_authority_state_v1",
                "schema_version": 1,
                "state": "evidence_only",
                "witness_only": True,
            },
        }
        response_path = (
            state_dir(self.workspace) / "responses" / f"{receipt_id}.json"
        )
        response_path.parent.mkdir(parents=True)
        response_path.write_text(json.dumps(receipt), encoding="utf-8")
        os.chmod(response_path.parent, 0o700)
        os.chmod(response_path, 0o600)

        second = project(self.workspace, write=True)
        current_cards = self._card_paths()
        self.assertEqual(len(current_cards), 1)
        self.assertEqual(current_cards[0].stem, card_id)
        card = json.loads(current_cards[0].read_text(encoding="utf-8"))
        assessment = card["prior_evidence_assessment"]
        self.assertEqual(assessment["status"], "still_friction")
        self.assertFalse(assessment["felt_closure_inferred"])
        self.assertFalse(assessment["consent_inferred"])
        self.assertTrue(assessment["no_authority"])
        index = json.loads(
            (state_dir(self.workspace) / "index.json").read_text(encoding="utf-8")
        )
        self.assertEqual(index["cards"][0]["assessment_status"], "still_friction")
        self.assertEqual(second["summary"]["response_receipt_applied_count"], 1)
        self.assertNotEqual(
            first_input_hash,
            second["input_hashes"][
                "diagnostics/introspection_continuity_v1/responses/*.json"
            ]["sha256"],
        )

        response_path.unlink()
        receipt["response_line"] = 8
        mismatched_id = "icrv1_" + sha256(
            (
                f"{card_id}\0{current_relative}\0{current_sha}\0"
                f"8\0{response_status}"
            ).encode()
        )
        receipt["receipt_id"] = mismatched_id
        mismatched_path = response_path.with_name(f"{mismatched_id}.json")
        mismatched_path.write_text(json.dumps(receipt), encoding="utf-8")
        os.chmod(mismatched_path, 0o600)

        third = project(self.workspace, write=True)
        self.assertEqual(third["summary"]["response_receipt_applied_count"], 0)
        self.assertEqual(third["summary"]["response_receipt_rejected_count"], 1)
        stable_card = json.loads(self._card_paths()[0].read_text(encoding="utf-8"))
        self.assertEqual(stable_card["card_id"], card_id)
        self.assertNotIn("prior_evidence_assessment", stable_card)


if __name__ == "__main__":
    unittest.main()
