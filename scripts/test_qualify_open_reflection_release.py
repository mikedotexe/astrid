"""Fail-closed packet boundaries; actual staged checks run in the qualifier."""
import json
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

import qualify_open_reflection_release as tool


class QualificationTests(unittest.TestCase):
    def test_old_recall_contract_is_explicit_and_recorded(self):
        for expected in ("present", "absent"):
            with self.subTest(expected=expected), tempfile.TemporaryDirectory() as temporary:
                with patch.object(tool.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as run:
                    receipt = tool.reflection_preservation(Path("old"), Path("new"),
                                                           Path(temporary), expected)
                self.assertEqual(run.call_args.kwargs["env"]["QUALIFICATION_OLD_REFLECTION_RECALL"], expected)
                self.assertEqual(receipt["old_reflection_recall"], expected)

    def test_default_preserves_legacy_transition_contract(self):
        with tempfile.TemporaryDirectory() as temporary:
            with patch.object(tool.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as run:
                receipt = tool.reflection_preservation(Path("old"), Path("new"), Path(temporary))
            self.assertEqual(run.call_args.kwargs["env"]["QUALIFICATION_OLD_REFLECTION_RECALL"], "present")
            self.assertEqual(receipt["old_reflection_recall"], "present")

    def test_unknown_recall_contract_fails_before_running_or_writing(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(tool.subprocess, "run") as run:
                with self.assertRaisesRegex(ValueError, "unknown old reflection recall"):
                    tool.reflection_preservation(Path("old"), Path("new"), root, "automatic")
                with self.assertRaisesRegex(ValueError, "unknown old reflection recall"):
                    tool.qualify(root, root, root, root, root, old_reflection_recall="automatic")
            run.assert_not_called()
            self.assertEqual(list(root.iterdir()), [])

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
