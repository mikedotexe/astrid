"""Offline synthetic qualification; actual executables are opt-in, never built."""
import json
import os
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

import qualify_mailbox_context_release as tool


class ReceiptTests(unittest.TestCase):
    def patches(self, root):
        from contextlib import ExitStack
        stack = ExitStack()
        inputs = dict.fromkeys(tool.ADAPTER_MODULES, "a" * 64)
        for name, value in (
            ("verify_stage", {}), ("inventory", inputs), ("verify_launch_binding", {}),
            ("freeze_python", inputs), ("owner_preservation", {"checks": ["baseline"]}),
            ("owner_context", {"checks": ["context"]}), ("bridge_checkpoint_probe", {}),
            ("adapter_probe", {}), ("invalid_state_probes", []), ("preparation_retry_probe", {}),
            ("sha", "b" * 64),
        ):
            stack.enter_context(patch.object(tool, name, return_value=value))
        return stack

    def call(self, root):
        return tool.qualify(root / "old", root / "new", root / "minime", root / "binding", root / "out")

    def test_receipt_is_evidence_only_and_binds_exact_inventory(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.patches(root):
                receipt = self.call(root)
            self.assertEqual(receipt["status"], "passed")
            self.assertFalse(receipt["activation_performed"])
            self.assertFalse(receipt["live_eligible_now"])
            self.assertEqual(set(receipt["selected_inputs"]), set(tool.ADAPTER_MODULES))
            self.assertEqual(set(receipt["qualifier_inputs"]), set(tool.QUALIFIER_INPUTS))
            self.assertEqual(receipt["checks"], 4)

    def test_missing_mailbox_module_refuses_before_creating_packet(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.patches(root), patch.object(tool, "inventory", return_value={}):
                with self.assertRaisesRegex(ValueError, "mailbox modules missing"):
                    self.call(root)
            self.assertFalse((root / "out").exists())

    def test_failure_is_retained_without_success_or_activation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.patches(root), patch.object(tool, "owner_context", side_effect=ValueError("synthetic failed check")):
                with self.assertRaisesRegex(ValueError, "synthetic failed check"):
                    self.call(root)
            receipt = json.loads((root / "out/failure.json").read_bytes())
            self.assertFalse(receipt["activation_performed"])
            self.assertFalse(receipt["live_eligible_now"])
            self.assertFalse((root / "out/qualification.json").exists())

    def test_stage_drift_retains_failed_packet(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.patches(root), patch.object(tool, "verify_stage", side_effect=[{}, {}, {}, {"changed": True}]):
                with self.assertRaisesRegex(ValueError, "stage drift"):
                    self.call(root)
            self.assertTrue((root / "out/failure.json").exists())
            self.assertFalse((root / "out/qualification.json").exists())

    def test_source_drift_refuses_success(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs = dict.fromkeys(tool.ADAPTER_MODULES, "a" * 64)
            with self.patches(root), patch.object(tool, "inventory", side_effect=[inputs, {}]):
                with self.assertRaisesRegex(ValueError, "Minime input drift"):
                    self.call(root)
            self.assertFalse((root / "out/qualification.json").exists())

    def test_existing_packet_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "out").mkdir()
            receipt = root / "out/qualification.json"
            receipt.write_text("retained exact receipt")
            with self.patches(root):
                with self.assertRaises(FileExistsError):
                    self.call(root)
            self.assertEqual(receipt.read_text(), "retained exact receipt")


class ActualArtifactTests(unittest.TestCase):
    @unittest.skipUnless(os.environ.get("QUALIFICATION_OLD_READER") and
                         os.environ.get("ASTRID_SOURCE_STUDY_BIN"), "actual old/new helper paths required")
    def test_schema_thirteen_byte_preservation_and_changed_choices(self):
        old = Path(os.environ["QUALIFICATION_OLD_READER"]).resolve(strict=True)
        new = Path(os.environ["ASTRID_SOURCE_STUDY_BIN"]).resolve(strict=True)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            for owner in ("astrid", "minime"):
                with self.subTest(owner=owner):
                    baseline = tool.owner_preservation(root / (owner + "-baseline"), owner, old, new)
                    self.assertGreater(len(baseline["checks"]), 20)
                    result = tool.owner_context(root / owner, owner, old, new)
                    self.assertEqual(result["schema_version"], 13)
                    self.assertGreaterEqual(len(result["checks"]), 18)

    @unittest.skipUnless(os.environ.get("MINIME_QUALIFICATION_SOURCE") and
                         os.environ.get("ASTRID_SOURCE_STUDY_BIN"), "actual frozen-source input and helper required")
    def test_actual_frozen_mailbox_adapter_and_pending_next(self):
        source = Path(os.environ["MINIME_QUALIFICATION_SOURCE"]).resolve(strict=True)
        helper = Path(os.environ["ASTRID_SOURCE_STUDY_BIN"]).resolve(strict=True)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            inputs = tool.freeze_python(source, root / "snapshot")
            stage = root / "fixture-stage"
            reader = stage / "helpers/astrid-source-study"
            reader.parent.mkdir(parents=True)
            shutil.copyfile(helper, reader)
            reader.chmod(0o555)
            # Reader-selection fixture only, not a verified bridge release stage.
            tool.write_json(stage / "manifest.json", {"artifacts": {"source-study-reader": {
                "path": str(reader), "sha256": tool.sha(reader)}}})
            try:
                result = tool.adapter_probe(root / "snapshot", stage, root / "adapter")
            except ValueError as error:
                self.fail(str(error) + "\n" + (root / "adapter/adapter.log").read_text())
            self.assertEqual(result["selected_inputs"], inputs)
            self.assertTrue(result["old_pending_next_bytes_preserved"])
            self.assertTrue(result["uncertain_completion_no_retry"])
            self.assertFalse(result["scheduling_authorized"])
            self.assertEqual(tool.inventory(source), inputs)

    @unittest.skipUnless(os.environ.get("QUALIFICATION_OLD_STAGE") and
                         os.environ.get("QUALIFICATION_NEW_STAGE"), "actual old/new bridge stages required")
    def test_both_staged_checkpoint_decoders_are_read_only(self):
        old = Path(os.environ["QUALIFICATION_OLD_STAGE"]).resolve(strict=True)
        new = Path(os.environ["QUALIFICATION_NEW_STAGE"]).resolve(strict=True)
        tool.verify_stage(old)
        tool.verify_stage(new)
        with tempfile.TemporaryDirectory() as temporary:
            result = tool.bridge_checkpoint_probe(old, new, Path(temporary).resolve() / "bridge")
            self.assertTrue(result["both_runtime_schemas_decoded"])
            self.assertTrue(result["bytes_unchanged"])
            self.assertFalse(result["services_started"])


if __name__ == "__main__":
    unittest.main()
