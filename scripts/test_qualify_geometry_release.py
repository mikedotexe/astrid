"""Synthetic snapshot safety tests; no services, live data, or native builds."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import qualify_geometry_release as qualify


class SnapshotTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="geometry-release-test-")
        self.root = Path(self.temp.name).resolve()
        self.addCleanup(self.cleanup)
        self.source = self.root / "source"
        for name in ("autonomous_agent.py", "minime_autonomy/source_study.py", "mikemind/__init__.py",
                     "scripts/launchd_autonomous_agent.sh", "scripts/minime_rescue_investigation.py",
                     "launchd/com.minime.autonomous-agent.plist"):
            path = self.source / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("# synthetic fixture\n")

    def cleanup(self):
        for path in self.root.rglob("*"):
            if path.is_dir() and not path.is_symlink():
                path.chmod(0o700)
        self.temp.cleanup()

    def test_snapshot_is_exact_read_only_and_omits_live_state(self):
        private = self.source / "workspace/private.py"
        private.parent.mkdir()
        private.write_text("synthetic not-an-input")
        destination = self.root / "snapshot"
        before = qualify.inventory(self.source)
        self.assertEqual(qualify.freeze_python(self.source, destination), before)
        self.assertEqual(qualify.inventory(destination), before)
        self.assertFalse((destination / "workspace").exists())
        self.assertTrue(all(not ((destination / name).stat().st_mode & 0o222) for name in before))
        self.assertFalse(destination.stat().st_mode & 0o222)

    def test_missing_required_input_fails(self):
        (self.source / "scripts/launchd_autonomous_agent.sh").unlink()
        with self.assertRaisesRegex(ValueError, "missing"):
            qualify.freeze_python(self.source, self.root / "snapshot")

    def test_symlink_is_refused_even_when_internal(self):
        (self.source / "linked.py").symlink_to(self.source / "autonomous_agent.py")
        with self.assertRaisesRegex(ValueError, "symlink"):
            qualify.inventory(self.source)

    def test_parent_symlink_is_refused(self):
        directory = self.source / "minime_autonomy"
        directory.rename(self.source / "saved")
        directory.symlink_to(self.source / "saved", target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symlink"):
            qualify.inventory(self.source)

    def test_existing_output_is_not_overwritten(self):
        destination = self.root / "snapshot"
        destination.mkdir()
        (destination / "evidence.json").write_text("prior")
        with self.assertRaises(FileExistsError):
            qualify.freeze_python(self.source, destination)
        self.assertEqual((destination / "evidence.json").read_text(), "prior")

    def test_mid_copy_change_is_rejected(self):
        original = qualify.shutil.copyfile
        def changed(source, destination):
            result = original(source, destination)
            Path(source).write_text("changed concurrently")
            return result
        with patch.object(qualify.shutil, "copyfile", side_effect=changed):
            with self.assertRaisesRegex(ValueError, "inventory changed"):
                qualify.freeze_python(self.source, self.root / "snapshot")

    def test_copy_corruption_is_rejected(self):
        def corrupt(_source, destination):
            Path(destination).write_text("wrong bytes")
        with patch.object(qualify.shutil, "copyfile", side_effect=corrupt):
            with self.assertRaisesRegex(ValueError, "changed during copying"):
                qualify.freeze_python(self.source, self.root / "snapshot")

    def test_evidence_write_is_exclusive_and_private(self):
        path = self.root / "receipt.json"
        qualify.write_json(path, {"live_eligible_now": False})
        original = path.read_bytes()
        self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        with self.assertRaises(FileExistsError):
            qualify.write_json(path, {"changed": True})
        self.assertEqual(path.read_bytes(), original)

    def launch_receipt(self):
        inputs = qualify.inventory(self.source)
        receipt = self.root / "reconciliation.json"
        receipt.write_text(json.dumps({
            "schema": "minime_launch_source_reconciliation_v1",
            "selected_inputs": inputs,
            "canonical_inputs": inputs,
            "bindings": {"canonical_root": str(self.source)},
        }))
        return receipt, inputs

    def test_launch_binding_matches_exact_frozen_inputs_without_claiming_installation(self):
        receipt, inputs = self.launch_receipt()
        binding = qualify.verify_launch_binding(receipt, inputs)
        self.assertEqual(binding["receipt_sha256"], qualify.sha(receipt))
        self.assertTrue(binding["exact_selected_inputs"])
        self.assertFalse(binding["canonical_applied"])

    def test_launch_binding_rejects_candidate_and_canonical_drift(self):
        receipt, inputs = self.launch_receipt()
        with self.assertRaisesRegex(ValueError, "qualified Python inputs"):
            qualify.verify_launch_binding(receipt, {**inputs, "unexpected.py": "0" * 64})
        (self.source / "autonomous_agent.py").write_text("# later canonical edit\n")
        with self.assertRaisesRegex(ValueError, "canonical sources changed"):
            qualify.verify_launch_binding(receipt, inputs)

    def test_null_or_unrecognized_launch_receipt_is_not_an_opt_out(self):
        receipt, inputs = self.launch_receipt()
        for value in (None, [], {"schema": "future", "selected_inputs": inputs}):
            receipt.write_text(json.dumps(value))
            with self.assertRaisesRegex(ValueError, "launch reconciliation"):
                qualify.verify_launch_binding(receipt, inputs)


if __name__ == "__main__":
    unittest.main()
