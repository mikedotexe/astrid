"""Fail-closed packet boundaries; actual staged checks run in the qualifier."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import qualify_open_reflection_release as tool


class QualificationTests(unittest.TestCase):
    def test_failure_is_retained_without_success_or_activation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            out = root / "qualification"
            with patch.object(tool, "verify_stage", return_value={}), \
                 patch.object(tool, "inventory", return_value={}), \
                 patch.object(tool, "verify_launch_binding", return_value={}), \
                 patch.object(tool, "qualify_eof", side_effect=ValueError("synthetic failed check")):
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
