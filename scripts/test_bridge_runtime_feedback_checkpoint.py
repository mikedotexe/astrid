"""Private temporary sidecars only; no process, network or deployment access."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import bridge_activate as activation


class RuntimeFeedbackCheckpointTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="feedback-checkpoint-fixture-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.stage = self.root / "stage"
        self.stage.mkdir()
        self.backend = activation.LaunchdBridge(self.stage, self.root)
        self.backend.workspace.mkdir(parents=True)
        self.path = self.backend.workspace / activation.RUNTIME_FEEDBACK_FILE
        (self.backend.workspace / "state.json").write_text('{"exchange_count":3}')
        control = self.backend.workspace / "self_control_v2/astrid/state.json"
        control.parent.mkdir(parents=True)
        control.write_text('{"synthetic":"control"}')
        self.backend.canonical_manifest.parent.mkdir()
        self.backend.canonical_manifest.write_text('{"synthetic":"manifest"}')
        self.backend.ready = {"manifest_sha256":"fixture", "binary_sha256":"fixture"}
        self.backend.begin({"synthetic":True})
        self.backend.retain_hold()
        patcher = patch.object(self.backend, "native", side_effect=self.native)
        patcher.start()
        self.addCleanup(patcher.stop)

    def write_feedback(self, pending=True):
        items = [{"id":"fixture-result-1", "requested_action":"READ_MORE", "status":"blocked",
                  "message":"Runtime guard result", "reason":"budget", "suggested_next":None}] if pending else []
        activation.atomic_bytes(self.path, json.dumps({
            "schema":"pending_runtime_action_feedback_v1", "pending_runtime_feedback":items}).encode())
        return activation.runtime_feedback_descriptor(self.path)

    def native(self, *args):
        self.assertEqual(args, ("--verify-deployment-inputs",))
        return {"checkpoint":{"sha256":activation.digest(self.backend.workspace / "state.json"),
                "runtime_action_feedback":activation.runtime_feedback_descriptor(self.path)},
                "self_control":{"state_targets_this_binary":True}}

    def snapshot(self):
        self.backend.snapshot_and_handoff("fixture", "offline")
        return self.backend.transaction / activation.RUNTIME_FEEDBACK_SNAPSHOT

    def startup(self, feedback):
        directory = self.root / ".runtime/bridge-lifecycle"
        directory.mkdir(exist_ok=True)
        checkpoint = {"sha256":activation.digest(self.backend.workspace / "state.json")}
        if feedback is not None:
            checkpoint["runtime_action_feedback"] = feedback
        activation.stage_tools.atomic_json(directory / "12346.startup.json", {
            "pid":12346, "checkpoint":checkpoint, "self_control":{
                "state_deployment_identity":"fixture", "state_targets_this_binary":True}})
        with patch.object(activation.drain, "read_status", return_value={"phase":"running"}), \
             patch.object(activation.drain, "verify"):
            return self.backend.lifecycle_startup(12346, ("synthetic", "/never-executed"), "fixture")

    def test_optional_file_is_exact_private_snapshot_and_startup_bound(self):
        original = self.write_feedback()
        saved = self.snapshot()
        self.assertEqual(saved.read_bytes(), self.path.read_bytes())
        self.assertEqual(saved.stat().st_mode & 0o777, 0o600)
        self.assertEqual(original["sha256"], activation.digest(saved))
        self.assertEqual(original["pending_count"], 1)
        self.startup(original)
        # The queue may legitimately drain after a startup bound to the old bytes.
        self.write_feedback(pending=False)
        self.startup(original)
        saved.write_bytes(b"{}")
        with self.assertRaises((RuntimeError, ValueError)):
            self.startup(original)

    def test_startup_missing_or_different_feedback_binding_is_refused(self):
        original = self.write_feedback()
        self.snapshot()
        for descriptor in (None, {"present":False}, {**original, "sha256":"0" * 64}):
            with self.subTest(descriptor=descriptor), self.assertRaisesRegex(RuntimeError, "runtime feedback"):
                self.startup(descriptor)

    def test_explicit_absence_at_startup_allows_later_new_feedback(self):
        self.snapshot()
        self.write_feedback()
        self.startup({"present":False})

    def test_legacy_missing_metadata_requires_absence(self):
        saved = self.snapshot()
        self.assertFalse(saved.exists())
        before_path = self.backend.transaction / "inputs.before.json"
        before = activation.stage_tools.json_file(before_path)
        del before["checkpoint"]["runtime_action_feedback"]
        activation.stage_tools.atomic_json(before_path, before)
        self.startup(None)
        self.write_feedback(pending=False)
        with self.assertRaisesRegex(RuntimeError, "runtime feedback"):
            self.startup(None)

    def test_creation_deletion_and_content_changes_keep_hold(self):
        for initial, mutation in ((False, "create"), (True, "delete"), (True, "change")):
            with self.subTest(mutation=mutation):
                if self.path.exists():
                    self.path.unlink()
                if initial:
                    self.write_feedback()
                binding = activation.runtime_feedback_descriptor(self.path)
                self.backend._stopped_feedback_binding = {"runtime_action_feedback":binding}
                if mutation == "delete":
                    self.path.unlink()
                else:
                    self.write_feedback(pending=False)
                with self.assertRaisesRegex(RuntimeError, "runtime feedback"):
                    self.backend.release_hold()
                self.assertTrue((self.backend.control / "hold.json").exists())

    def test_change_during_snapshot_does_not_admit_handoff(self):
        original_write = activation.atomic_bytes
        def racing_write(path, data, mode=0o600):
            original_write(path, data, mode)
            if path.name == "conversation.before.json":
                self.write_feedback()
        with patch.object(activation, "atomic_bytes", side_effect=racing_write):
            with self.assertRaisesRegex(RuntimeError, "runtime feedback"):
                self.snapshot()
        self.assertFalse((self.backend.transaction / "inputs.before.json").exists())

    def test_snapshot_tampering_before_release_keeps_hold(self):
        self.write_feedback()
        saved = self.snapshot()
        saved.unlink()
        with self.assertRaisesRegex(RuntimeError, "runtime feedback"):
            self.backend.release_hold()
        self.assertTrue((self.backend.control / "hold.json").exists())

    def test_descriptor_refuses_unknown_private_state_and_hashes_exact_bytes(self):
        original = self.write_feedback()
        self.path.write_bytes(self.path.read_bytes() + b"\n")
        self.assertNotEqual(original["sha256"], activation.runtime_feedback_descriptor(self.path)["sha256"])
        self.path.chmod(0o644)
        with self.assertRaisesRegex(RuntimeError, "private"):
            activation.runtime_feedback_descriptor(self.path)
        self.path.chmod(0o600)
        for bad in (b"{}", b'{"schema":"future","pending_runtime_feedback":[]}',
                    b'{"schema":"pending_runtime_action_feedback_v1","pending_runtime_feedback":[],"extra":0}',
                    b'{"schema":"pending_runtime_action_feedback_v1","schema":"pending_runtime_action_feedback_v1","pending_runtime_feedback":[]}'):
            with self.subTest(bad=bad):
                self.path.write_bytes(bad)
                with self.assertRaises((RuntimeError, ValueError)):
                    activation.runtime_feedback_descriptor(self.path)
        self.write_feedback()
        value = json.loads(self.path.read_bytes())
        value["pending_runtime_feedback"] *= 2
        self.path.write_text(json.dumps(value))
        with self.assertRaisesRegex(RuntimeError, "duplicate identity"):
            activation.runtime_feedback_descriptor(self.path)
        self.path.unlink()
        self.path.symlink_to(self.root / "missing")
        with self.assertRaises((RuntimeError, OSError)):
            activation.runtime_feedback_descriptor(self.path)


if __name__ == "__main__":
    unittest.main()
