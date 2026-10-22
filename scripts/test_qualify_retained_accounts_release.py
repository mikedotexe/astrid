"""Fail-closed qualification orchestration; real helpers are tested separately."""
from contextlib import ExitStack
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import qualify_retained_accounts_release as tool


class QualificationTests(unittest.TestCase):
    def setup_inputs(self, root):
        binding = root / "binding.json"
        binding.write_text(json.dumps({"assets": {}}))
        return (root / "old", root / "new", root / "source", binding,
                root / "operator", root / "tests", root / "astrid", root / "out")

    def mocked(self):
        stack = ExitStack()
        for name, result in (
            ("verify_stage", {"verified": True}), ("sha", "a" * 64),
            ("preserve_writing", {"checks": 171, "selected_inputs": {"runtime.py": "b" * 64}}),
            ("account_preservation", {"checks": ["synthetic"]}),
            ("frozen_tests", {"passed": True}), ("inventory", {"runtime.py": "b" * 64}),
        ):
            stack.enter_context(patch.object(tool, name, return_value=result))
        return stack

    def test_receipt_has_no_activation_or_merged_candidate_claim(self):
        with tempfile.TemporaryDirectory() as directory:
            args = self.setup_inputs(Path(directory))
            with self.mocked():
                receipt = tool.qualify(*args)
            self.assertEqual(receipt["checks"], 173)
            self.assertEqual(receipt["selected_inputs"], {"runtime.py": "b" * 64})
            for field in ("activation_performed", "live_eligible_now",
                          "capacity_qualification_included", "navigation_candidate_included"):
                self.assertIs(receipt[field], False)

    def test_every_failed_boundary_retains_failure_without_success(self):
        for boundary in ("verify_stage", "preserve_writing", "account_preservation", "frozen_tests"):
            with self.subTest(boundary=boundary), tempfile.TemporaryDirectory() as directory:
                args = self.setup_inputs(Path(directory))
                with self.mocked(), patch.object(tool, boundary, side_effect=ValueError("synthetic failure")):
                    with self.assertRaisesRegex(ValueError, "synthetic failure"):
                        tool.qualify(*args)
                receipt = json.loads((args[-1] / "failure.json").read_bytes())
                self.assertFalse(receipt["activation_performed"])
                self.assertFalse(receipt["live_eligible_now"])
                self.assertFalse((args[-1] / "qualification.json").exists())

    def test_combined_candidate_scope_does_not_grant_activation_authority(self):
        with tempfile.TemporaryDirectory() as directory:
            args = self.setup_inputs(Path(directory))
            with self.mocked():
                receipt = tool.qualify(*args, navigation_candidate_included=True)
            self.assertTrue(receipt["navigation_candidate_included"])
            self.assertFalse(receipt["activation_performed"])
            self.assertFalse(receipt["live_eligible_now"])
            self.assertFalse(receipt["capacity_qualification_included"])

    def test_input_drift_refuses_success(self):
        with tempfile.TemporaryDirectory() as directory:
            args = self.setup_inputs(Path(directory))
            with self.mocked(), patch.object(tool, "inventory", return_value={}):
                with self.assertRaisesRegex(ValueError, "Minime source drift"):
                    tool.qualify(*args)
            self.assertTrue((args[-1] / "failure.json").exists())

    def test_stage_drift_refuses_success(self):
        with tempfile.TemporaryDirectory() as directory:
            args = self.setup_inputs(Path(directory))
            with self.mocked(), patch.object(tool, "verify_stage", side_effect=[{}, {}, {}, {"changed": True}]):
                with self.assertRaisesRegex(ValueError, "stage drift"):
                    tool.qualify(*args)
            self.assertFalse((args[-1] / "qualification.json").exists())

    def test_existing_packet_is_not_rewritten(self):
        with tempfile.TemporaryDirectory() as directory:
            args = self.setup_inputs(Path(directory))
            args[-1].mkdir()
            retained = args[-1] / "qualification.json"
            retained.write_bytes(b"retained evidence")
            with self.mocked(), self.assertRaises(FileExistsError):
                tool.qualify(*args)
            self.assertEqual(retained.read_bytes(), b"retained evidence")

    def test_position_comparison_excludes_only_navigation_offer(self):
        before = {"questions": {"active": "q2"}, "reading_contexts": {
            "q1": {"position": 7, "pending_navigation": "old"}}}
        after = {"questions": {"active": "q2"}, "reading_contexts": {
            "q1": {"position": 7, "pending_navigation": "new"}}}
        self.assertEqual(tool.preserved_reading(before), tool.preserved_reading(after))
        after["reading_contexts"]["q1"]["position"] = 8
        self.assertNotEqual(tool.preserved_reading(before), tool.preserved_reading(after))
        self.assertEqual(before["reading_contexts"]["q1"]["pending_navigation"], "old")


if __name__ == "__main__":
    unittest.main()
