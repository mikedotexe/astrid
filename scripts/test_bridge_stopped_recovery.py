"""Stopped recovery uses real temporary files and only synthetic process/native calls."""
import json
from pathlib import Path
import tempfile
import time
import unittest
from unittest.mock import patch

import bridge_activate as activation


class StoppedRecoveryTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="bridge-stopped-fixture-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.stage = self.root / "stage"
        (self.stage / "helpers").mkdir(parents=True)
        self.backend = activation.LaunchdBridge(self.stage, self.root)
        self.backend.launcher.parent.mkdir()
        artifacts = {}
        for name, destination in (("release-launcher", self.backend.launcher),
                                  ("release-selection", self.backend.selection_helper)):
            source = self.stage / activation.stage_tools.ARTIFACTS[name]
            source.write_text("synthetic " + name)
            destination.write_bytes(source.read_bytes())
            artifacts[name] = {"sha256":activation.digest(source)}
        self.binary = self.stage / "spectral-bridge-server"
        self.binary.write_text("synthetic new executable; never run")
        self.old_binary = self.root / "old-binary"
        self.old_binary.write_text("synthetic old executable; never run")
        artifacts["spectral-bridge"] = {"path":str(self.binary), "sha256":activation.digest(self.binary)}
        activation.stage_tools.atomic_json(self.stage / "manifest.json", {"artifacts":artifacts})
        self.backend.ready = {"schema":"bridge_staged_release_v2", "source_root":str(self.root),
            "manifest_sha256":activation.digest(self.stage / "manifest.json"),
            "binary_sha256":activation.digest(self.binary)}
        self.backend.canonical_manifest.parent.mkdir(parents=True)
        activation.stage_tools.atomic_json(self.backend.canonical_manifest, {"artifacts":{"spectral-bridge":{
            "path":str(self.old_binary), "sha256":activation.digest(self.old_binary)}}})
        self.old_manifest = self.backend.canonical_manifest.read_bytes()
        self.state = self.backend.workspace / "state.json"
        self.state.write_text('{"exchange_count":10,"history":"synthetic"}')
        self.checkpoint = activation.digest(self.state)
        self.control_state = self.backend.workspace / "self_control_v2/astrid/state.json"
        self.control_state.parent.mkdir(parents=True)
        self.control_state.write_text('{"deployment":"old","synthetic":true}')
        self.pending = self.control_state.with_name("deployment_handoff.pending.json")
        self.backend.begin({"status":"preparing"})
        self.selection = self.backend.control / "active.json"
        self.selection.write_text('{"stage":"synthetic-old-selection"}')
        old = {"started_at":"synthetic-old-start", "binary":str(self.old_binary),
            "binary_sha256":activation.digest(self.old_binary),
            "manifest_sha256":activation.digest(self.backend.canonical_manifest),
            "launcher_sha256":activation.digest(self.backend.launcher),
            "selection_helper_sha256":activation.digest(self.backend.selection_helper),
            "selection_sha256":activation.digest(self.selection), "drain_supported":True}
        for path in (self.backend.launcher, self.backend.selection_helper):
            (self.backend.transaction / (path.name + ".before")).write_bytes(path.read_bytes())
        self.failed = {"schema":"bridge_activation_v1", "status":"failed_requires_review",
            "activation_performed":False, "force_used":False, "automatic_rollback":False,
            "legacy_transition":False, "signal":"SIGTERM", "old_pid":12345, "old_identity":old,
            "drain":{"supported":True, "pid":12345, "phase":"drained", "checkpoint_sha256":self.checkpoint}}
        self.backend.record(self.failed)
        self.failure = self.backend.transaction / "receipt.json"
        self.failure_bytes = self.failure.read_bytes()
        self.backend.retain_hold()
        self.hold = self.backend.control / "hold.json"
        self.hold_bytes = self.hold.read_bytes()
        self.started = False
        self.process = (time.strftime("%a %b %d %H:%M:%S %Y"), str(self.binary))
        self.mocks = {}
        for target, name, options in (
            (activation.os, "kill", {"side_effect":ProcessLookupError}),
            (self.backend, "pid", {"side_effect":lambda:12346 if self.started else None}),
            (self.backend, "verify_launch_configuration", {"side_effect":lambda pid:self.assertEqual(pid, 12346 if self.started else None)}),
            (self.backend, "verify_bundle", {}),
            (activation.stage_tools, "verify_stage", {"return_value":self.backend.ready}),
            (activation.drain, "process_identity", {"return_value":self.process}),
            (self.backend, "native", {"side_effect":self.native}),
            (self.backend, "model_idle", {"return_value":True}),
        ):
            patcher = patch.object(target, name, **options)
            self.mocks[name] = patcher.start()
            self.addCleanup(patcher.stop)
        for name in ("drain", "signal", "old_exists", "install_hold_and_launcher"):
            patcher = patch.object(self.backend, name, side_effect=AssertionError("forbidden control " + name))
            patcher.start()
            self.addCleanup(patcher.stop)
        self.real_release = self.backend.release_hold
        patcher = patch.object(self.backend, "release_hold", side_effect=self.start_replacement)
        patcher.start()
        self.addCleanup(patcher.stop)

    def native(self, *args):
        if args == ("--verify-deployment-manifest",):
            return {"deployment_identity":"new-deployment"}
        if args == ("--verify-deployment-inputs",):
            return {"checkpoint":{"sha256":activation.digest(self.state), "exchange_count":11 if self.started else 10,
                    "runtime_action_feedback":activation.runtime_feedback_descriptor(self.backend.workspace / activation.RUNTIME_FEEDBACK_FILE)},
                "self_control":{"state_targets_this_binary":self.started,
                                "state_deployment_identity":"new-deployment" if self.started else "old"}}
        self.assertEqual(args[0], "--prepare-self-control-deployment-handoff")
        handoff = {"synthetic":True, "target_manifest_sha256":self.backend.ready["manifest_sha256"],
                   "target_binary_sha256":self.backend.ready["binary_sha256"]}
        activation.stage_tools.atomic_json(self.pending, handoff)
        return handoff

    def start_replacement(self):
        self.assertEqual(self.records()[0]["status"], "release_pending")
        self.assertTrue(self.records()[0]["release_intent"])
        self.real_release()
        self.started = True
        directory = self.root / ".runtime/bridge-lifecycle"
        directory.mkdir(exist_ok=True)
        activation.stage_tools.atomic_json(directory / "12346.json", {
            "schema":"bridge_operator_drain_v1", "pid":12346, "instance":"a" * 32,
            "authority":"operator_maintenance_witness_only", "phase":"running",
            "executable":str(self.binary), "executable_sha256":activation.digest(self.binary),
            "started_at_unix_ms":int(time.time() * 1000)})
        activation.stage_tools.atomic_json(directory / "12346.startup.json", {
            "pid":12346, "checkpoint":{"sha256":self.checkpoint,
                "runtime_action_feedback":activation.runtime_feedback_descriptor(self.backend.workspace / activation.RUNTIME_FEEDBACK_FILE)},
            "self_control":{"state_targets_this_binary":True,"state_deployment_identity":"new-deployment"}})
        self.state.write_text('{"exchange_count":11,"history":"synthetic"}')
        self.control_state.write_text('{"deployment":"new-deployment","synthetic":true}')
        self.pending.unlink()

    def records(self):
        return [activation.stage_tools.json_file(path) for path in
                (self.backend.transaction / "stopped-transition-recoveries").glob("*/receipt.json")]

    def resume(self):
        return activation.resume_stopped_transition(self.backend, transaction=self.backend.transaction,
            expected_pid=12345, actor="fixture", ack="synthetic stopped recovery", timeout=1)

    def test_real_file_pipeline_preserves_failure_and_exact_continuity(self):
        result = self.resume()
        self.assertEqual(result["status"], "transition_recovered")
        self.assertEqual(self.failure.read_bytes(), self.failure_bytes)
        self.assertFalse(self.hold.exists())
        self.assertEqual(self.backend.canonical_manifest.read_bytes(), (self.stage / "manifest.json").read_bytes())
        self.assertEqual(activation.digest(self.backend.transaction / "conversation.before.json"), self.checkpoint)
        self.assertTrue(result["new_process"]["new_saved_exchange_observed"])
        for key in ("signal_sent", "drain_requested", "force_used", "automatic_rollback"):
            self.assertIs(result[key], False)
        with self.assertRaisesRegex(RuntimeError, "already has progress"):
            self.resume()

    def test_v3_shared_reader_release_recovers_with_exact_continuity(self):
        self.backend.ready["schema"] = "bridge_staged_release_v3"
        result = self.resume()
        self.assertEqual(result["status"], "transition_recovered")
        self.assertEqual(self.failure.read_bytes(), self.failure_bytes)
        self.assertEqual(activation.digest(self.backend.transaction / "conversation.before.json"), self.checkpoint)
        self.assertFalse(self.hold.exists())

    def test_v3_mismatched_release_binding_never_releases_hold(self):
        self.backend.ready["schema"] = "bridge_staged_release_v3"
        for key in ("manifest_sha256", "binary_sha256"):
            original = self.backend.ready[key]
            with self.subTest(key=key):
                self.backend.ready[key] = "0" * 64
                with self.assertRaisesRegex(RuntimeError, "does not bind"):
                    self.resume()
                self.backend.ready[key] = original
                self.assertEqual(self.records(), [])
                self.assertEqual(self.hold.read_bytes(), self.hold_bytes)

    def test_legacy_and_unknown_release_formats_never_release_hold(self):
        for schema in ("bridge_staged_release_v1", "bridge_staged_release_v99"):
            with self.subTest(schema=schema):
                self.backend.ready["schema"] = schema
                with self.assertRaisesRegex(RuntimeError, "does not bind"):
                    self.resume()
                self.assertEqual(self.records(), [])
                self.assertEqual(self.hold.read_bytes(), self.hold_bytes)

    def test_complete_original_activation_snapshot_is_validated_without_replay(self):
        self.backend.snapshot_and_handoff("fixture", "original activation snapshot")
        with patch.object(self.backend, "snapshot_and_handoff", side_effect=AssertionError("snapshot replayed")):
            result = self.resume()
        self.assertTrue(result["snapshot_prepared"])
        self.assertEqual(result["status"], "transition_recovered")
        self.assertFalse(self.hold.exists())

    def test_live_pid_is_refused_without_progress(self):
        self.mocks["kill"].side_effect = None
        with self.assertRaisesRegex(RuntimeError, "PID still exists"):
            self.resume()
        self.assertEqual(self.records(), [])

    def test_pending_feedback_survives_stopped_recovery_with_exact_private_snapshot(self):
        sidecar = self.backend.workspace / activation.RUNTIME_FEEDBACK_FILE
        activation.atomic_bytes(sidecar, b'{"schema":"pending_runtime_action_feedback_v1","pending_runtime_feedback":[{"id":"one","requested_action":"READ_MORE","status":"blocked","message":"Runtime result"}]}')
        original = sidecar.read_bytes()
        result = self.resume()
        saved = self.backend.transaction / activation.RUNTIME_FEEDBACK_SNAPSHOT
        self.assertEqual(saved.read_bytes(), original)
        self.assertEqual(saved.stat().st_mode & 0o777, 0o600)
        self.assertEqual(result["runtime_action_feedback"]["sha256"], activation.digest(saved))
        self.assertEqual(result["new_process"]["startup"]["checkpoint"]["runtime_action_feedback"]["pending_count"], 1)

    def test_feedback_creation_after_stopped_inspection_keeps_hold(self):
        original = self.backend.snapshot_and_handoff
        def changed(*args):
            sidecar = self.backend.workspace / activation.RUNTIME_FEEDBACK_FILE
            activation.atomic_bytes(sidecar, b'{"schema":"pending_runtime_action_feedback_v1","pending_runtime_feedback":[]}')
            original(*args)
        with patch.object(self.backend, "snapshot_and_handoff", side_effect=changed):
            with self.assertRaisesRegex(RuntimeError, "runtime feedback"):
                self.resume()
        self.assertFalse(self.started)
        self.assertEqual(self.hold.read_bytes(), self.hold_bytes)

    def test_tampered_preconditions_never_release_hold(self):
        for path in (self.state, self.selection, self.backend.canonical_manifest, self.backend.launcher,
                     self.backend.transaction / "launchd_spectral_bridge.sh.before"):
            original = path.read_bytes()
            with self.subTest(path=path):
                path.write_bytes(b"tampered")
                with self.assertRaises((RuntimeError, ValueError)):
                    self.resume()
                path.write_bytes(original)
                self.assertEqual(self.records(), [])
                self.assertEqual(self.hold.read_bytes(), self.hold_bytes)

    def test_foreign_hold_and_existing_handoff_are_refused(self):
        self.hold.write_text('{"schema":"bridge_launch_hold_v1","transaction":"foreign"}')
        with self.assertRaisesRegex(RuntimeError, "does not own"):
            self.resume()
        self.hold.write_bytes(self.hold_bytes)
        self.pending.write_text('{"foreign":true}')
        with self.assertRaisesRegex(RuntimeError, "signed handoff presence"):
            self.resume()
        self.assertEqual(self.records(), [])

    def test_partial_handoff_failure_is_not_replayed(self):
        with patch.object(self.backend, "select_release", side_effect=RuntimeError("injected selection failure")):
            with self.assertRaisesRegex(RuntimeError, "injected"):
                self.resume()
        self.assertEqual(self.hold.read_bytes(), self.hold_bytes)
        self.assertFalse(self.records()[0]["release_intent"])
        with self.assertRaisesRegex(RuntimeError, "already has progress"):
            self.resume()
        self.assertEqual(self.failure.read_bytes(), self.failure_bytes)

    def test_stopped_history_claim_is_exclusive(self):
        self.backend.begin_recovery({"fixture":True}, history="stopped-transition-recoveries")
        with self.assertRaises(FileExistsError):
            self.backend.begin_recovery({"fixture":True}, history="stopped-transition-recoveries")
        self.assertEqual(len(self.records()), 1)

    def test_after_release_failure_uses_existing_verification_recovery(self):
        with patch.object(self.backend, "verify_new", side_effect=RuntimeError("injected verification failure")):
            with self.assertRaisesRegex(RuntimeError, "injected"):
                self.resume()
        self.assertTrue(self.records()[0]["release_intent"])
        self.assertEqual(self.failure.read_bytes(), self.failure_bytes)
        with patch.object(self.backend, "release_hold", side_effect=self.real_release):
            result = activation.resume_verification(self.backend, transaction=self.backend.transaction,
                expected_pid=12346, actor="fixture", ack="verify synthetic replacement", timeout=1)
        self.assertEqual(result["status"], "verification_recovered")
        self.assertEqual(self.failure.read_bytes(), self.failure_bytes)
        self.assertFalse(self.hold.exists())

    def test_abrupt_exit_after_release_can_verify_without_reintroducing_hold(self):
        self.resume()
        self.backend.canonical_manifest.write_bytes(self.old_manifest)
        with patch.object(self.backend, "retain_hold", side_effect=AssertionError("new hold")), \
             patch.object(self.backend, "release_hold", side_effect=AssertionError("second release")):
            result = activation.resume_verification(self.backend, transaction=self.backend.transaction,
                expected_pid=12346, actor="fixture", ack="verify synthetic replacement", timeout=1)
        self.assertEqual(result["status"], "verification_recovered")
        self.assertEqual(self.failure.read_bytes(), self.failure_bytes)


if __name__ == "__main__":
    unittest.main()
