"""Activation ordering and failure tests never signal a live service."""
import copy
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
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


class VerificationRecoveryTests(unittest.TestCase):
    """Real transaction/manifest/hold files; every process surface is synthetic."""

    def setUp(self):
        TransactionFilesTests.setUp(self)
        TransactionFilesTests.launcher_fixture(self)
        self.pid = TransactionFilesTests.verify_fixture(self)
        self.binary = self.stage / "spectral-bridge-server"
        self.binary.write_bytes(b"synthetic executable, never run")
        self.process = (time.strftime("%a %b %d %H:%M:%S %Y"), str(self.binary))
        self.backend.ready = {"schema":"bridge_staged_release_v2", "source_root":str(self.root),
            "manifest_sha256":activation.digest(self.stage / "manifest.json"),
            "binary_sha256":activation.digest(self.binary)}
        self.backend.canonical_manifest.parent.mkdir(parents=True)
        self.backend.canonical_manifest.write_text('{"previous":"manifest"}')
        self.old_manifest = self.backend.canonical_manifest.read_bytes()
        old_sha = activation.digest(self.backend.canonical_manifest)
        (self.backend.transaction / "manifest.before.json").write_bytes(self.old_manifest)
        checkpoint = self.backend.transaction / "conversation.before.json"
        checkpoint.write_text('{"exchange_count":10,"history":"synthetic"}')
        checkpoint_sha = activation.digest(checkpoint)
        activation.stage_tools.atomic_json(self.backend.transaction / "inputs.before.json", {
            "checkpoint":{"sha256":checkpoint_sha, "exchange_count":10}})
        self.status_path = self.root / ".runtime/bridge-lifecycle" / f"{self.pid}.json"
        self.startup_path = self.status_path.with_name(f"{self.pid}.startup.json")
        self.status = {"schema":"bridge_operator_drain_v1", "pid":self.pid,
            "instance":"a" * 32, "authority":"operator_maintenance_witness_only", "phase":"running",
            "executable":str(self.binary), "executable_sha256":activation.digest(self.binary),
            "started_at_unix_ms":int(time.time() * 1000)}
        activation.stage_tools.atomic_json(self.status_path, self.status)
        self.startup = {"pid":self.pid, "checkpoint":{"sha256":checkpoint_sha},
            "self_control":{"state_deployment_identity":"new-deployment", "state_targets_this_binary":True}}
        activation.stage_tools.atomic_json(self.startup_path, self.startup)
        self.current = {"checkpoint":{"exchange_count":11}, "self_control":{
            "state_targets_this_binary":True, "state_deployment_identity":"new-deployment"}}
        failed = {"schema":"bridge_activation_v1", "status":"failed_requires_review",
            "error":"process reports failed or unknown drain phase", "activation_performed":True,
            "old_process_exited":True, "force_used":False, "automatic_rollback":False,
            "legacy_transition":False, "signal":"SIGTERM", "old_pid":12345,
            "old_identity":{"manifest_sha256":old_sha},
            "drain":{"phase":"drained", "checkpoint_sha256":checkpoint_sha}}
        self.backend.record(failed)
        self.failure_path = self.backend.transaction / "receipt.json"
        self.failure_bytes = self.failure_path.read_bytes()
        self.backend.launcher.write_bytes((self.stage / activation.stage_tools.ARTIFACTS["release-launcher"]).read_bytes())
        self.backend.selection_helper.write_bytes((self.stage / activation.stage_tools.ARTIFACTS["release-selection"]).read_bytes())
        self.selection = {"schema":"bridge_release_selection_v1", "stage":str(self.stage),
            "manifest_sha256":self.backend.ready["manifest_sha256"], "transaction":str(self.backend.transaction)}
        activation.stage_tools.atomic_json(self.backend.control / "active.json", self.selection)
        self.backend.retain_hold()
        self.hold = self.backend.control / "hold.json"
        self.hold_bytes = self.hold.read_bytes()
        self.mocks = {}
        for target, name, options in (
            (self.backend, "pid", {"return_value":self.pid}),
            (self.backend, "verify_launch_configuration", {}),
            (self.backend, "verify_bundle", {}),
            (activation.stage_tools, "verify_stage", {"return_value":self.backend.ready}),
            (activation.drain, "process_identity", {"return_value":self.process}),
            (self.backend, "native", {"side_effect":self.native}),
            (self.backend, "model_idle", {"return_value":True}),
        ):
            patcher = patch.object(target, name, **options)
            self.mocks[name] = patcher.start()
            self.addCleanup(patcher.stop)
        # Even accidental calls to an activation-only method fail this test.
        for name in ("begin", "install_hold_and_launcher", "assert_old", "drain", "signal",
                     "old_exists", "snapshot_and_handoff", "select_release"):
            patcher = patch.object(self.backend, name, side_effect=AssertionError("forbidden live control: " + name))
            patcher.start()
            self.addCleanup(patcher.stop)

    def native(self, *args):
        if args == ("--verify-deployment-manifest",):
            return {"deployment_identity":"new-deployment"}
        self.assertEqual(args, ("--verify-deployment-inputs",))
        return self.current

    def resume(self):
        return activation.resume_verification(self.backend, transaction=self.backend.transaction,
            expected_pid=self.pid, actor="fixture", ack="verify synthetic existing replacement", timeout=1)

    def recovery_records(self):
        return [activation.stage_tools.json_file(path) for path in
            (self.backend.transaction / "verification-recoveries").glob("*/receipt.json")]

    def assert_no_publication(self):
        self.assertEqual(self.backend.canonical_manifest.read_bytes(), self.old_manifest)
        self.assertEqual(self.failure_path.read_bytes(), self.failure_bytes)
        self.assertEqual(self.hold.read_bytes(), self.hold_bytes)

    def test_recovery_preserves_failure_and_publishes_before_owned_release(self):
        release = self.backend.release_hold
        def release_after_durable_intent():
            self.assertEqual(self.backend.canonical_manifest.read_bytes(), (self.stage / "manifest.json").read_bytes())
            self.assertEqual(self.recovery_records()[0]["status"], "verified_release_pending")
            release()
        with patch.object(self.backend, "release_hold", side_effect=release_after_durable_intent):
            receipt = self.resume()
        self.assertEqual(receipt["status"], "verification_recovered")
        self.assertEqual(self.failure_path.read_bytes(), self.failure_bytes)
        self.assertFalse(self.hold.exists())
        for name in ("activation_performed", "signal_sent", "drain_requested", "restart_performed", "force_used", "automatic_rollback"):
            self.assertIs(receipt[name], False)
        self.assertTrue(receipt["new_process"]["new_saved_exchange_observed"])
        self.assertEqual(receipt["new_process"]["stopped_exchange_count"], 10)
        self.assertEqual(receipt["new_process"]["observed_exchange_count"], 11)
        self.assertTrue(receipt["new_process"]["model_idle_observed"])

    def test_recovery_history_directories_are_synced_before_publication(self):
        history = self.backend.transaction / "verification-recoveries"
        with patch.object(activation, "sync_directory", wraps=activation.sync_directory) as sync:
            publish = self.backend.publish_manifest
            def publish_after_directory_sync(*args, **kwargs):
                synced = [call.args[0] for call in sync.call_args_list]
                self.assertIn(self.backend.transaction, synced)
                self.assertIn(history, synced)
                publish(*args, **kwargs)
            with patch.object(self.backend, "publish_manifest", side_effect=publish_after_directory_sync):
                self.resume()

    def test_already_published_manifest_is_idempotent(self):
        self.backend.canonical_manifest.write_bytes((self.stage / "manifest.json").read_bytes())
        with patch.object(activation, "atomic_bytes", side_effect=AssertionError("manifest republished")):
            self.resume()
        self.assertFalse(self.hold.exists())
        self.assertEqual(self.failure_path.read_bytes(), self.failure_bytes)

    def test_completed_retry_has_no_manifest_or_hold_mutation(self):
        self.resume()
        with patch.object(activation, "atomic_bytes", side_effect=AssertionError("manifest republished")), \
             patch.object(self.backend, "retain_hold", side_effect=AssertionError("hold introduced")), \
             patch.object(self.backend, "release_hold", side_effect=AssertionError("hold released again")):
            self.resume()
        self.assertEqual(len(self.recovery_records()), 2)
        self.assertFalse(self.hold.exists())

    def test_crash_after_unlink_has_durable_release_intent_for_retry(self):
        self.resume()
        # Model abrupt death at the exact persisted pre-unlink state: final
        # receipt write never happened, but its release intent remains durable.
        path = self.backend.recovery_transaction / "receipt.json"
        intent = activation.stage_tools.json_file(path)
        intent["status"] = "verified_release_pending"
        activation.stage_tools.atomic_json(path, intent)
        self.resume()
        self.assertFalse(self.hold.exists())
        self.assertEqual(len(self.recovery_records()), 2)

    def test_failed_completed_retry_does_not_introduce_hold(self):
        self.resume()
        with patch.object(self.backend, "verify_new", side_effect=RuntimeError("model idle timeout")):
            with self.assertRaisesRegex(RuntimeError, "model idle timeout"):
                self.resume()
        self.assertFalse(self.hold.exists())
        self.assertEqual(self.failure_path.read_bytes(), self.failure_bytes)
        self.assertIn("failed_requires_review", {r["status"] for r in self.recovery_records()})

    def test_missing_hold_without_prior_recovery_is_refused(self):
        self.hold.unlink()
        with self.assertRaisesRegex(RuntimeError, "missing without completed"):
            self.resume()
        self.assertFalse(self.hold.exists())
        self.assertEqual(self.recovery_records(), [])

    def test_prior_recovery_cannot_authorize_reused_pid(self):
        self.resume()
        self.mocks["process_identity"].return_value = ("different-start", str(self.binary))
        with self.assertRaisesRegex(RuntimeError, "missing without completed"):
            self.resume()
        self.assertFalse(self.hold.exists())

    def test_changed_source_bundle_blocks_before_recovery_record(self):
        self.mocks["verify_bundle"].side_effect = RuntimeError("staged source inputs changed")
        with self.assertRaisesRegex(RuntimeError, "source inputs changed"):
            self.resume()
        self.assert_no_publication()
        self.assertEqual(self.recovery_records(), [])

    def test_selected_release_change_is_not_overwritten(self):
        path = self.backend.control / "active.json"
        path.write_text('{"foreign":true}')
        with self.assertRaisesRegex(RuntimeError, "release selection changed"):
            self.resume()
        self.assert_no_publication()
        self.assertEqual(path.read_text(), '{"foreign":true}')

    def test_foreign_hold_is_not_adopted_or_released(self):
        self.hold.write_text('{"schema":"bridge_launch_hold_v1","transaction":"foreign"}')
        before = self.hold.read_bytes()
        with self.assertRaisesRegex(RuntimeError, "does not own"):
            self.resume()
        self.assertEqual(self.hold.read_bytes(), before)
        self.assertEqual(self.backend.canonical_manifest.read_bytes(), self.old_manifest)

    def test_unrelated_manifest_is_not_overwritten(self):
        self.backend.canonical_manifest.write_text('{"foreign":true}')
        with self.assertRaisesRegex(RuntimeError, "canonical manifest changed"):
            self.resume()
        self.assertEqual(self.backend.canonical_manifest.read_text(), '{"foreign":true}')
        self.assertEqual(self.hold.read_bytes(), self.hold_bytes)

    def test_checkpoint_mismatch_blocks_recovery(self):
        self.startup["checkpoint"]["sha256"] = "another checkpoint"
        activation.stage_tools.atomic_json(self.startup_path, self.startup)
        with self.assertRaisesRegex(RuntimeError, "not the exact stopped-state"):
            self.resume()
        self.assert_no_publication()

    def test_wrong_signed_lineage_blocks_recovery(self):
        self.current["self_control"]["state_deployment_identity"] = "another deployment"
        with self.assertRaisesRegex(RuntimeError, "signed state does not target"):
            self.resume()
        self.assert_no_publication()

    def test_legacy_or_preactivation_failure_is_refused(self):
        for key, value in (("legacy_transition", True), ("activation_performed", False), ("old_process_exited", False)):
            with self.subTest(key=key):
                failed = json.loads(self.failure_bytes)
                failed[key] = value
                activation.stage_tools.atomic_json(self.failure_path, failed)
                with self.assertRaisesRegex(ValueError, "not an acknowledged post-activation"):
                    self.resume()
        self.assertEqual(self.recovery_records(), [])

    def test_recovery_failure_retains_exact_owned_hold(self):
        with patch.object(self.backend, "verify_new", side_effect=RuntimeError("no saved exchange")):
            with self.assertRaisesRegex(RuntimeError, "no saved exchange"):
                self.resume()
        self.assert_no_publication()
        self.assertEqual(self.recovery_records()[0]["status"], "failed_requires_review")

    def test_foreign_hold_arriving_during_verification_is_preserved(self):
        foreign = {"schema":"bridge_launch_hold_v1", "transaction":"other operator"}
        verify = self.backend.verify_new
        def verify_then_change_hold(*args, **kwargs):
            result = verify(*args, **kwargs)
            activation.stage_tools.atomic_json(self.hold, foreign)
            return result
        with patch.object(self.backend, "verify_new", side_effect=verify_then_change_hold):
            with self.assertRaisesRegex(RuntimeError, "foreign launch hold"):
                self.resume()
        self.assertEqual(activation.stage_tools.json_file(self.hold), foreign)
        self.assertEqual(self.backend.canonical_manifest.read_bytes(), self.old_manifest)
        self.assertIn("foreign launch hold", self.recovery_records()[0]["hold_recovery_error"])

    def test_no_fresh_saved_exchange_or_busy_model_cannot_publish(self):
        for saved, idle in ((10, True), (11, False)):
            with self.subTest(saved=saved, idle=idle):
                self.current["checkpoint"]["exchange_count"] = saved
                self.mocks["model_idle"].return_value = idle
                with patch.object(activation.time, "monotonic", side_effect=[0, 0, 2]), \
                     patch.object(activation.time, "sleep"):
                    with self.assertRaisesRegex(RuntimeError, "fresh exchange in time"):
                        self.resume()
                self.assert_no_publication()

    def test_process_change_after_verification_prevents_publication(self):
        verify = self.backend.verify_new
        def verify_then_reuse_pid(*args, **kwargs):
            result = verify(*args, **kwargs)
            self.mocks["process_identity"].return_value = ("reused", str(self.binary))
            return result
        with patch.object(self.backend, "verify_new", side_effect=verify_then_reuse_pid):
            with self.assertRaisesRegex(RuntimeError, "recovery process identity changed"):
                self.resume()
        self.assert_no_publication()

    def test_starting_phase_waits_for_running_without_relaxing_identity(self):
        self.status["phase"] = "starting"
        activation.stage_tools.atomic_json(self.status_path, self.status)
        def start_runtime(_):
            self.status["phase"] = "running"
            activation.stage_tools.atomic_json(self.status_path, self.status)
        with patch.object(activation.time, "sleep", side_effect=start_runtime) as sleep:
            result = self.resume()
        sleep.assert_called_once_with(1)
        self.assertEqual(result["status"], "verification_recovered")

    def test_failed_unknown_and_draining_phases_are_not_waited_through(self):
        for phase in ("failed", "initializing", "unknown", "draining", "drained"):
            with self.subTest(phase=phase):
                self.status["phase"] = phase
                activation.stage_tools.atomic_json(self.status_path, self.status)
                with patch.object(activation.time, "sleep", side_effect=AssertionError("fatal state was swallowed")):
                    with self.assertRaises(RuntimeError):
                        self.resume()
                self.assert_no_publication()

    def test_starting_phase_still_rejects_wrong_lifecycle_identity(self):
        self.status.update(phase="starting", executable_sha256="forged")
        activation.stage_tools.atomic_json(self.status_path, self.status)
        with self.assertRaisesRegex(RuntimeError, "executable changed"):
            self.resume()
        self.assert_no_publication()

    def test_verification_refuses_pid_or_start_identity_change(self):
        for pid, identity in ((12347, self.process), (self.pid, ("reused", str(self.binary)))):
            with self.subTest(pid=pid, identity=identity):
                self.mocks["pid"].return_value = pid
                self.mocks["process_identity"].return_value = identity
                with self.assertRaisesRegex(RuntimeError, "expected replacement"):
                    self.backend.verify_new(12345, 1, expected_pid=self.pid, expected_identity=self.process)
        self.assert_no_publication()


class RecoveryWrapperTests(unittest.TestCase):
    def invoke(self, *args):
        # Both preflights and the eventual Python helper are replaced with an
        # argv recorder. No installed launchd service or runtime file is read.
        with tempfile.TemporaryDirectory(prefix="bridge-wrapper-recovery-") as directory:
            root = Path(directory)
            executable = root / "python3"
            executable.write_text(f"#!{sys.executable}\nimport json, os, sys\n"
                "with open(os.environ['FIXTURE_LOG'], 'a') as f:\n"
                " f.write(json.dumps({'argv':sys.argv[1:], 'sanctioned':os.environ.get('ASTRID_SANCTIONED_BRIDGE_ACTIVATION')})+'\\n')\n")
            executable.chmod(0o700)
            log = root / "calls.jsonl"
            result = subprocess.run(["bash", str(Path(activation.__file__).with_name("build_bridge.sh")), *args],
                env={**os.environ, "PATH":str(root) + os.pathsep + os.environ["PATH"], "FIXTURE_LOG":str(log)},
                capture_output=True, text=True)
            calls = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
            return result, calls

    def test_resume_option_is_forwarded_only_after_standard_preflight(self):
        result, calls = self.invoke("--activate-stage", "/fixture-stage", "--resume-verification", "/fixture-transaction",
            "--expected-pid", "12346", "--actor", "fixture", "--ack", "synthetic recovery")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertGreaterEqual(len(calls), 2)
        self.assertTrue(all("deploy_preflight.py" in call["argv"][1] for call in calls[:-1]))
        self.assertTrue(calls[-1]["argv"][1].endswith("bridge_activate.py"))
        self.assertEqual(calls[-1]["sanctioned"], "1")
        self.assertEqual(calls[-1]["argv"][-2:], ["--resume-verification", "/fixture-transaction"])

    def test_empty_missing_or_mixed_resume_options_never_fall_into_activation(self):
        base = ("--activate-stage", "/fixture-stage", "--expected-pid", "12346", "--ack", "fixture")
        cases = [("--resume-verification", ""), ("--resume-verification",),
            ("--resume-verification", "--actor", "fixture"),
            ("--resume-verification", "/fixture-transaction", "--legacy-stop-ack", "fixture"),
            ("--resume-verification", "/fixture-transaction", "--restart")]
        for args in cases:
            with self.subTest(args=args):
                result, calls = self.invoke(*base, *args)
                self.assertEqual(result.returncode, 64, result.stderr)
                self.assertEqual(calls, [])
        result, calls = self.invoke("--resume-verification", "/fixture-transaction")
        self.assertEqual(result.returncode, 64)
        self.assertEqual(calls, [])


if __name__ == "__main__":
    unittest.main()
