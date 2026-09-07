"""Activation ordering and failure tests never signal a live service."""
import copy
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import bridge_activate as activation


class Backend:
    def __init__(self, *, legacy=True, fail=None, idle=True, exits=True):
        self.legacy, self.fail, self.idle, self.exits = legacy, fail, idle, exits
        self.events, self.receipts = [], []
        self.clock = 0

    def call(self, name):
        self.events.append(name)
        if self.fail == name:
            raise RuntimeError("injected " + name)

    def inspect(self, pid):
        self.call("inspect")
        return {"drain_supported":not self.legacy}

    def begin(self, receipt):
        self.call("begin")

    def install_hold_and_launcher(self, initial):
        self.call("hold_and_launcher")

    def assert_old(self, pid, initial):
        self.call("assert_old")

    def drain(self, pid, timeout):
        self.call("drain")
        return {"phase":"drained"}

    def model_idle(self):
        self.call("model_idle")
        return self.idle

    def signal(self, pid, sig):
        self.call(signal.Signals(sig).name)

    def record(self, receipt):
        self.receipts.append(copy.deepcopy(receipt))

    def old_exists(self, pid, initial):
        self.call("old_exists")
        return not self.exits

    def snapshot_and_handoff(self, actor, ack):
        self.call("snapshot_and_handoff")

    def select_release(self):
        self.call("select_release")

    def release_hold(self):
        self.call("release_hold")

    def verify_new(self, pid, timeout):
        self.call("verify_new")
        return {"pid":12346}

    def publish_manifest(self, initial):
        self.call("publish_manifest")

    def retain_hold(self):
        self.call("retain_hold")

    def sleep(self, duration):
        self.clock += duration


class ActivationTests(unittest.TestCase):
    def run_activation(self, backend, **kwargs):
        return activation.activate(backend, expected_pid=12345, actor="test", ack="fixture only",
                                   legacy_ack=kwargs.get("legacy_ack", "explicit fixture legacy stop"),
                                   timeout=1, now=lambda:backend.clock, sleep=backend.sleep)

    def test_legacy_requires_separate_ack_before_any_write(self):
        backend = Backend()
        with self.assertRaisesRegex(ValueError, "legacy bridge requires"):
            self.run_activation(backend, legacy_ack="")
        self.assertEqual(backend.events, ["inspect"])
        self.assertEqual(backend.receipts, [])

    def test_legacy_transition_order_and_truthful_receipt(self):
        backend = Backend()
        receipt = self.run_activation(backend)
        self.assertEqual(backend.events, ["inspect", "begin", "hold_and_launcher", "assert_old",
            "model_idle", "assert_old", "SIGINT", "old_exists", "snapshot_and_handoff",
            "select_release", "release_hold", "verify_new", "publish_manifest"])
        self.assertEqual(receipt["status"], "activated_verified")
        self.assertTrue(receipt["activation_performed"])
        self.assertFalse(receipt["lossless_drain_claimed"])
        self.assertTrue(receipt["model_idle_observed_not_atomic_barrier"])
        self.assertFalse(receipt["force_used"])

    def test_new_protocol_drains_before_normal_exit(self):
        backend = Backend(legacy=False)
        receipt = self.run_activation(backend, legacy_ack="")
        self.assertLess(backend.events.index("drain"), backend.events.index("SIGTERM"))
        self.assertNotIn("SIGINT", backend.events)
        self.assertNotIn("model_idle", backend.events)
        self.assertFalse(receipt["legacy_transition"])

    def test_idle_timeout_does_not_signal(self):
        backend = Backend(idle=False)
        with self.assertRaisesRegex(RuntimeError, "no idle model window"):
            self.run_activation(backend)
        self.assertFalse(any(event.startswith("SIG") for event in backend.events))
        self.assertEqual(backend.events[-1], "retain_hold")

    def test_exit_timeout_never_escalates_or_selects(self):
        backend = Backend(exits=False)
        with self.assertRaisesRegex(RuntimeError, "no force"):
            self.run_activation(backend)
        self.assertEqual([event for event in backend.events if event.startswith("SIG")], ["SIGINT"])
        self.assertNotIn("snapshot_and_handoff", backend.events)
        self.assertNotIn("select_release", backend.events)

    def test_identity_change_does_not_signal(self):
        backend = Backend(fail="assert_old")
        with self.assertRaisesRegex(RuntimeError, "injected assert_old"):
            self.run_activation(backend)
        self.assertNotIn("SIGINT", backend.events)

    def test_handoff_failure_cannot_select_or_release_hold(self):
        backend = Backend(fail="snapshot_and_handoff")
        with self.assertRaisesRegex(RuntimeError, "injected snapshot"):
            self.run_activation(backend)
        self.assertNotIn("select_release", backend.events)
        self.assertNotIn("release_hold", backend.events)
        self.assertFalse(backend.receipts[-1]["activation_performed"])

    def test_post_start_failure_retains_hold_without_rollback_or_publication(self):
        backend = Backend(fail="verify_new")
        with self.assertRaisesRegex(RuntimeError, "injected verify_new"):
            self.run_activation(backend)
        self.assertNotIn("publish_manifest", backend.events)
        self.assertTrue(backend.receipts[-1]["activation_performed"])
        self.assertFalse(backend.receipts[-1]["automatic_rollback"])
        self.assertEqual(backend.events[-1], "retain_hold")

    def test_interruption_is_recorded_and_holds_next_launch(self):
        backend = Backend()
        with patch.object(backend, "snapshot_and_handoff", side_effect=KeyboardInterrupt):
            with self.assertRaises(KeyboardInterrupt):
                self.run_activation(backend)
        self.assertEqual(backend.receipts[-1]["status"], "failed_requires_review")
        self.assertIn("retain_hold", backend.events)

    def test_failed_hold_recovery_does_not_erase_original_error(self):
        backend = Backend(fail="verify_new")
        with patch.object(backend, "retain_hold", side_effect=RuntimeError("foreign hold")):
            with self.assertRaisesRegex(RuntimeError, "injected verify_new"):
                self.run_activation(backend)
        self.assertEqual(backend.receipts[-1]["hold_recovery_error"], "foreign hold")

    def test_standalone_activation_is_refused(self):
        result = subprocess.run(["python3", "-B", activation.__file__, "--stage-dir", "/unused",
                                 "--expected-pid", "12345", "--actor", "test", "--ack", "fixture"],
                                env={k:v for k,v in os.environ.items() if k != "ASTRID_SANCTIONED_BRIDGE_ACTIVATION"},
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn("activate only through", result.stdout)


class TransactionFilesTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="bridge-activation-files-")
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name).resolve()
        self.stage = self.root / "stage"
        self.stage.mkdir()
        self.backend = activation.LaunchdBridge(self.stage, self.root)
        self.backend.ready = {"manifest_sha256":"fixture", "binary_sha256":"fixture"}
        self.backend.begin({"status":"preparing"})

    def test_foreign_hold_is_never_replaced(self):
        path = self.backend.control / "hold.json"
        path.write_text('{"owner":"foreign"}')
        before = path.read_bytes()
        with self.assertRaisesRegex(RuntimeError, "foreign launch hold"):
            self.backend.retain_hold()
        self.assertEqual(before, path.read_bytes())
        with self.assertRaisesRegex(RuntimeError, "ownership changed"):
            self.backend.release_hold()

    def test_owned_hold_is_durable_private_and_releasable(self):
        self.backend.retain_hold()
        self.backend.retain_hold()
        path = self.backend.control / "hold.json"
        self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        self.assertEqual(json.loads(path.read_bytes()), self.backend.guard)
        self.backend.release_hold()
        self.assertFalse(path.exists())

    def test_failed_process_lookup_is_not_proof_of_exit(self):
        with patch.object(activation.os, "kill"), \
             patch.object(activation.drain, "process_identity", side_effect=activation.drain.DrainError("lookup failed")):
            with self.assertRaisesRegex(RuntimeError, "lookup failed"):
                self.backend.old_exists(12345, {})

    def test_observed_process_exit_is_accepted(self):
        with patch.object(activation.os, "kill", side_effect=ProcessLookupError):
            self.assertFalse(self.backend.old_exists(12345, {}))

    def test_reused_pid_is_not_the_old_process(self):
        with patch.object(activation.os, "kill"), \
             patch.object(activation.drain, "process_identity", return_value=("new", "/new")):
            with self.assertRaisesRegex(RuntimeError, "reused"):
                self.backend.old_exists(12345, {"started_at":"old", "binary":"/old"})

    def test_canonical_publication_refuses_concurrent_change(self):
        self.backend.canonical_manifest.parent.mkdir(parents=True)
        self.backend.canonical_manifest.write_text("foreign")
        with self.assertRaisesRegex(RuntimeError, "changed concurrently"):
            self.backend.publish_manifest({"manifest_sha256":"old"})
        self.assertEqual(self.backend.canonical_manifest.read_text(), "foreign")

    def launcher_fixture(self):
        (self.stage / "helpers").mkdir()
        self.backend.launcher.parent.mkdir()
        self.backend.launcher.write_bytes(b"old launcher")
        artifacts = {}
        for name in ("release-launcher", "release-selection"):
            path = self.stage / activation.stage_tools.ARTIFACTS[name]
            path.write_bytes(("new " + name).encode())
            artifacts[name] = {"sha256":activation.digest(path)}
        (self.stage / "manifest.json").write_text(json.dumps({"artifacts":artifacts}))
        return {"launcher_sha256":activation.digest(self.backend.launcher)}

    def test_launcher_install_preserves_original_and_holds_first(self):
        initial = self.launcher_fixture()
        with patch.object(self.backend, "verify_bundle"):
            self.backend.install_hold_and_launcher(initial)
        self.assertEqual((self.backend.transaction / "launchd_spectral_bridge.sh.before").read_bytes(), b"old launcher")
        self.assertEqual(self.backend.launcher.read_bytes(), b"new release-launcher")
        self.assertEqual(self.backend.selection_helper.read_bytes(), b"new release-selection")
        self.assertEqual(activation.stage_tools.json_file(self.backend.control / "hold.json"), self.backend.guard)

    def test_unexpected_helper_is_not_overwritten(self):
        initial = self.launcher_fixture()
        self.backend.selection_helper.write_text("foreign helper")
        with patch.object(self.backend, "verify_bundle"):
            with self.assertRaisesRegex(RuntimeError, "changed concurrently"):
                self.backend.install_hold_and_launcher(initial)
        self.assertEqual(self.backend.selection_helper.read_text(), "foreign helper")
        self.assertTrue((self.backend.control / "hold.json").exists())

    def test_handoff_uses_only_stopped_state_backups(self):
        self.backend.workspace.mkdir(parents=True)
        (self.backend.workspace / "state.json").write_text('{"history":"synthetic"}')
        control = self.backend.workspace / "self_control_v2/astrid/state.json"
        control.parent.mkdir(parents=True)
        control.write_text('{"synthetic":"control"}')
        self.backend.canonical_manifest.parent.mkdir()
        self.backend.canonical_manifest.write_text('{"synthetic":"manifest"}')
        calls = []
        def native(*args):
            calls.append(args)
            if args == ("--verify-deployment-inputs",):
                return {"self_control":{"state_targets_this_binary":False}}
            self.assertTrue((self.backend.transaction / "conversation.before.json").is_file())
            self.assertTrue((self.backend.transaction / "self-control.before.json").is_file())
            return {"synthetic":"handoff"}
        with patch.object(self.backend, "native", side_effect=native):
            self.backend.snapshot_and_handoff("test", "fixture")
        self.assertEqual(calls[-1], ("--prepare-self-control-deployment-handoff", "--operator-actor", "test", "--operator-ack", "fixture"))
        self.assertEqual((self.backend.transaction / "self-control.before.json").stat().st_mode & 0o777, 0o600)
        self.assertFalse(any("key" in path.name for path in self.backend.transaction.iterdir()))

    def test_concurrent_selection_is_not_overwritten(self):
        self.backend.old = {}
        path = self.backend.control / "active.json"
        path.write_text('{"foreign":true}')
        with patch.object(self.backend, "verify_bundle"):
            with self.assertRaisesRegex(RuntimeError, "selection changed concurrently"):
                self.backend.select_release()
        self.assertEqual(path.read_text(), '{"foreign":true}')

    def verify_fixture(self, checkpoint="stopped-checkpoint"):
        pid = 12346
        directory = self.root / ".runtime/bridge-lifecycle"
        directory.mkdir()
        (directory / f"{pid}.json").write_text('{"phase":"running"}')
        (directory / f"{pid}.startup.json").write_text(json.dumps({"pid":pid,
            "checkpoint":{"sha256":checkpoint}, "self_control":{
                "state_deployment_identity":"new-deployment", "state_targets_this_binary":True}}))
        (self.backend.transaction / "inputs.before.json").write_text(json.dumps({
            "checkpoint":{"sha256":"stopped-checkpoint", "exchange_count":10}}))
        return pid

    def test_verifies_exact_startup_checkpoint_and_fresh_saved_exchange(self):
        pid = self.verify_fixture()
        def native(*args):
            if args == ("--verify-deployment-manifest",):
                return {"deployment_identity":"new-deployment"}
            return {"checkpoint":{"exchange_count":11}, "self_control":{"state_targets_this_binary":True}}
        with patch.object(self.backend, "pid", return_value=pid), \
             patch.object(activation.drain, "process_identity", return_value=("new-start", str(self.stage / "spectral-bridge-server"))), \
             patch.object(activation.drain, "verify"), \
             patch.object(self.backend, "model_idle", return_value=True), \
             patch.object(self.backend, "native", side_effect=native):
            result = self.backend.verify_new(12345, 1)
        self.assertEqual(result["pid"], pid)
        self.assertTrue(result["new_saved_exchange_observed"])

    def test_different_startup_checkpoint_is_refused_even_with_new_exchange(self):
        pid = self.verify_fixture("different-checkpoint")
        def native(*args):
            if args == ("--verify-deployment-manifest",):
                return {"deployment_identity":"new-deployment"}
            return {"checkpoint":{"exchange_count":11}, "self_control":{"state_targets_this_binary":True}}
        with patch.object(self.backend, "pid", return_value=pid), \
             patch.object(activation.drain, "process_identity", return_value=("new-start", str(self.stage / "spectral-bridge-server"))), \
             patch.object(activation.drain, "verify"), \
             patch.object(self.backend, "native", side_effect=native):
            with self.assertRaisesRegex(RuntimeError, "not the exact stopped-state"):
                self.backend.verify_new(12345, 1)


if __name__ == "__main__":
    unittest.main()
