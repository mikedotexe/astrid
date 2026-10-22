"""Synthetic snapshot safety tests; no services, live data, or native builds."""
import json
from pathlib import Path
import shutil
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

    def launch_receipt(self, *, distinct=False):
        before = qualify.inventory(self.source)
        inputs = before
        if distinct:
            self.selected = self.root / "selected"
            shutil.copytree(self.source, self.selected)
            for name in ("autonomous_agent.py", "minime_autonomy/source_study.py",
                         "minime_autonomy/selected_only.py"):
                (self.selected / name).write_text("# selected synthetic input\n")
            inputs = qualify.inventory(self.selected)
        receipt = self.root / "reconciliation.json"
        receipt.write_text(json.dumps({
            "schema": "minime_launch_source_reconciliation_v1",
            "selected_inputs": inputs,
            "canonical_inputs": before,
            "bindings": {"canonical_root": str(self.source)},
        }))
        return receipt, inputs

    def assert_source_only_binding(self, binding):
        self.assertTrue(binding["exact_selected_inputs"])
        self.assertEqual(binding["observation_scope"], "canonical_source_inventory_only")
        for key in ("canonical_writes", "installer_provenance_verified", "drain_verified",
                    "readiness_verified", "activation_performed"):
            self.assertIs(binding[key], False)
        self.assertNotIn("canonical_applied", binding)

    def test_launch_binding_matches_exact_frozen_inputs_without_claiming_installation(self):
        receipt, inputs = self.launch_receipt()
        original = receipt.read_bytes()
        binding = qualify.verify_launch_binding(receipt, inputs)
        self.assertEqual(binding["receipt_sha256"], qualify.sha(receipt))
        self.assertEqual(binding["observed_boundary"], "original_and_selected_inputs")
        self.assert_source_only_binding(binding)
        self.assertEqual(receipt.read_bytes(), original)

    def test_launch_binding_accepts_exact_before_or_fully_selected_inventory(self):
        receipt, inputs = self.launch_receipt(distinct=True)
        original = receipt.read_bytes()
        before = qualify.inventory(self.source)
        binding_before = qualify.verify_launch_binding(receipt, inputs)
        self.assertEqual(binding_before["observed_boundary"], "original_canonical_inputs")
        self.assert_source_only_binding(binding_before)
        self.assertEqual(qualify.inventory(self.source), before)
        shutil.copytree(self.selected, self.source, dirs_exist_ok=True)
        binding_selected = qualify.verify_launch_binding(receipt, inputs)
        self.assertEqual(binding_selected["observed_boundary"], "selected_inputs")
        self.assert_source_only_binding(binding_selected)
        self.assertEqual(qualify.inventory(self.source), inputs)
        self.assertEqual(receipt.read_bytes(), original)
        self.assertEqual(binding_before["receipt_sha256"], binding_selected["receipt_sha256"])
        # Existing callers compare the two observations to reject mid-run changes.
        self.assertNotEqual(binding_before, binding_selected)

    def test_launch_binding_rejects_partial_install_and_partial_rollback(self):
        receipt, inputs = self.launch_receipt(distinct=True)
        original = receipt.read_bytes()
        old = (self.source / "autonomous_agent.py").read_bytes()
        shutil.copyfile(self.selected / "autonomous_agent.py", self.source / "autonomous_agent.py")
        with self.assertRaisesRegex(ValueError, "canonical sources changed"):
            qualify.verify_launch_binding(receipt, inputs)
        shutil.copytree(self.selected, self.source, dirs_exist_ok=True)
        (self.source / "autonomous_agent.py").write_bytes(old)
        with self.assertRaisesRegex(ValueError, "canonical sources changed"):
            qualify.verify_launch_binding(receipt, inputs)
        self.assertEqual(receipt.read_bytes(), original)

    def test_launch_binding_rejects_missing_extra_and_drifted_selected_inputs(self):
        receipt, inputs = self.launch_receipt(distinct=True)
        shutil.copytree(self.selected, self.source, dirs_exist_ok=True)
        original = receipt.read_bytes()
        missing = self.source / "mikemind/__init__.py"
        saved = missing.read_bytes()
        missing.unlink()
        with self.assertRaisesRegex(ValueError, "canonical sources changed"):
            qualify.verify_launch_binding(receipt, inputs)
        missing.write_bytes(saved)
        extra = self.source / "extra.py"
        extra.write_text("# not selected\n")
        with self.assertRaisesRegex(ValueError, "canonical sources changed"):
            qualify.verify_launch_binding(receipt, inputs)
        extra.unlink()
        (self.source / "autonomous_agent.py").write_text("# drift after installation\n")
        with self.assertRaisesRegex(ValueError, "canonical sources changed"):
            qualify.verify_launch_binding(receipt, inputs)
        self.assertEqual(receipt.read_bytes(), original)

    def test_launch_binding_rejects_malformed_inventory_even_at_other_valid_boundary(self):
        receipt, inputs = self.launch_receipt(distinct=True)
        shutil.copytree(self.selected, self.source, dirs_exist_ok=True)
        valid = json.loads(receipt.read_bytes())
        malformed = (None, [], {}, {"autonomous_agent.py": "g" * 64},
                     {**inputs, "": "a" * 64}, {**inputs, ".": "a" * 64},
                     {**inputs, "nul\0.py": "a" * 64},
                     {**inputs, "../escape.py": "a" * 64},
                     {**inputs, "/absolute.py": "a" * 64},
                     {**inputs, "alias/../escape.py": "a" * 64},
                     {**inputs, "extra.py": True}, {**inputs, "extra.py": "a" * 63},
                     {**inputs, "extra.py": "A" * 64},
                     {k: v for k, v in inputs.items() if k != qualify.LAUNCH_INPUTS_REQUIRED[0]})
        for field in ("canonical_inputs", "selected_inputs"):
            for value in malformed:
                with self.subTest(field=field, value=value):
                    receipt.write_text(json.dumps({**valid, field: value}))
                    supplied = value if field == "selected_inputs" else inputs
                    with self.assertRaisesRegex(ValueError, "launch reconciliation"):
                        qualify.verify_launch_binding(receipt, supplied)
        for bindings in (None, [], {}, {"canonical_root": None},
                         {"canonical_root": "relative"}, {"canonical_root": ""}):
            with self.subTest(bindings=bindings):
                receipt.write_text(json.dumps({**valid, "bindings": bindings}))
                with self.assertRaisesRegex(ValueError, "launch reconciliation"):
                    qualify.verify_launch_binding(receipt, inputs)

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
