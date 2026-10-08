"""Qualification envelope tests; actual executable checks run separately."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import qualify_private_delivery_recovery as tool


class QualificationTests(unittest.TestCase):
    def test_helper_drift_refuses_before_creating_packet(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            out = root / "packet"
            def digest(path):
                return str(path) if path.name == "astrid-source-study" else "same"
            with patch.object(tool, "verify_stage", return_value={}), \
                 patch.object(tool, "sha", side_effect=digest), patch.object(tool, "probe") as probe:
                with self.assertRaisesRegex(ValueError, "shared helper changed"):
                    tool.qualify(root / "old", root / "new", out)
            self.assertFalse(out.exists())
            probe.assert_not_called()

    def test_failed_probe_is_not_qualified_and_evidence_is_preserved(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            out = root / "packet"
            with patch.object(tool, "verify_stage", return_value={}), \
                 patch.object(tool, "sha", return_value="same"), \
                 patch.object(tool, "probe", side_effect=ValueError("synthetic refusal")):
                with self.assertRaisesRegex(ValueError, "synthetic refusal"):
                    tool.qualify(root / "old", root / "new", out)
            self.assertFalse((out / "qualification.json").exists())
            failure = json.loads((out / "failure.json").read_bytes())
            self.assertFalse(failure["activation_performed"])
            before = (out / "failure.json").read_bytes()
            with patch.object(tool, "verify_stage", return_value={}), \
                 patch.object(tool, "sha", return_value="same"), patch.object(tool, "probe") as probe:
                with self.assertRaises(FileExistsError):
                    tool.qualify(root / "old", root / "new", out)
            self.assertEqual((out / "failure.json").read_bytes(), before)
            probe.assert_not_called()

    def test_success_remains_evidence_not_activation_authority(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(tool, "verify_stage", return_value={}), \
                 patch.object(tool, "sha", return_value="same"), \
                 patch.object(tool, "probe", return_value={"synthetic_only": True}):
                receipt = tool.qualify(root / "old", root / "new", root / "packet")
            for flag in ("activation_performed", "live_authority_granted", "live_eligible_now"):
                self.assertFalse(receipt[flag])


if __name__ == "__main__":
    unittest.main()
