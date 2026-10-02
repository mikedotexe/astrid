"""Synthetic qualification and receipt-boundary tests; no live state or inference."""
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import qualify_reflection_continuity_release as tool


class QualificationTests(unittest.TestCase):
    @unittest.skipUnless(os.environ.get("QUALIFICATION_OLD_READER") and
                         os.environ.get("ASTRID_SOURCE_STUDY_BIN"), "actual helper paths required")
    def test_actual_helpers_preserve_both_owners(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            for owner in ("astrid", "minime"):
                with self.subTest(owner=owner):
                    result = tool.owner_continuity(root / owner, owner,
                        Path(os.environ["QUALIFICATION_OLD_READER"]),
                        Path(os.environ["ASTRID_SOURCE_STUDY_BIN"]))
                    self.assertGreaterEqual(len(result["checks"]), 24)
                    self.assertTrue(result["state_hashes"])

    def test_failure_is_retained_without_success_or_activation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            out = root / "qualification"
            with patch.object(tool, "verify_stage", return_value={}), \
                 patch.object(tool, "inventory", return_value={}), \
                 patch.object(tool, "verify_launch_binding", return_value={}), \
                 patch.object(tool, "qualify_baseline", side_effect=ValueError("synthetic failed check")):
                with self.assertRaisesRegex(ValueError, "synthetic failed check"):
                    tool.qualify(root / "old", root / "new", root / "minime", root / "binding", out)
            receipt = json.loads((out / "failure.json").read_bytes())
            self.assertFalse(receipt["activation_performed"])
            self.assertFalse((out / "qualification.json").exists())

    def test_existing_packet_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            out = root / "qualification"
            out.mkdir()
            receipt = out / "qualification.json"
            receipt.write_text("retained receipt")
            with patch.object(tool, "verify_stage", return_value={}), \
                 patch.object(tool, "inventory", return_value={}), \
                 patch.object(tool, "verify_launch_binding", return_value={}):
                with self.assertRaises(FileExistsError):
                    tool.qualify(root / "old", root / "new", root / "minime", root / "binding", out)
            self.assertEqual(receipt.read_text(), "retained receipt")


if __name__ == "__main__":
    unittest.main()
