"""Synthetic idle bootstrap: real reload ordering and native NEXT receipts."""

import copy
import hashlib
import io
import json
import os
from contextlib import redirect_stdout
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch


MINIME = Path(os.environ.get(
    "MINIME_SOURCE_ROOT",
    "/Users/v/other/worktrees/finish-accepted-drain-20261012/minime",
))
sys.path.insert(0, str(MINIME))
from minime_autonomy import maintenance_next as native

import minime_next_handoff as proof
import bootstrap_minime_drain as bootstrap
from qualify_geometry_release import inventory
from reconcile_minime_launch import OVERLAY
from restart_minime_agent import LaunchdAgent, job_receipt, reload_agent


RUNTIME = "minime_autonomy/runtime.py"
LAUNCHER = "scripts/launchd_autonomous_agent.sh"


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, sort_keys=True) + "\n")


def write_rows(path, rows):
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows))


def file_sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class BootstrapBackend:
    """Only process/time/config are fake; source and NEXT checks read real files."""

    def __init__(self, root, loaded):
        self.root = root
        self.loaded = dict(loaded)
        self.clock = 0
        self.signals = []
        self.events = []
        self.validations = 0
        self.exit_old = True
        self.started_new = False
        self.quiet = True
        self.on_observe = None
        self.on_terminate = None
        self.on_ready = None
        self.observations = 0
        self.status_path = root / "workspace/runtime/autonomous_agent_source_status.json"
        self.lock_path = root / "workspace/runtime/autonomous_agent.lock"
        self.state_path = root / "workspace/sovereignty_state.json"
        self.hold_path = root / "workspace/runtime/agent-launch-hold.json"
        self.settings = {"configuration": "unchanged"}
        self.services = {"engine": {"pid": 90, "started_at": "same"},
                         "bridge": {"pid": 91, "started_at": "same"}}
        self.old_agent = self.native_agent(10, "1" * 32, self.loaded)
        self.old_status = self.status(self.old_agent, "agent_drain_v1", "idle", "idle")
        write_json(self.status_path, self.old_status)
        write_json(self.lock_path, {"pid": 10, "interval": 60})
        native.write_state(self.state_path, {
            "session_id": 7, "pending_next_action": "SELF_STUDY CONTINUE",
            "pending_next_selection_id": "synthetic-selection",
            "choice_envelope_v1": {"exact": "synthetic authored context"},
            "pending_next_action_status": "pending",
            "pending_next_action_updated_at": "2026-01-01T00:00:00",
        })
        write_json(root / "workspace/llm_jobs/index.json", {"recent_jobs": []})
        self.initial_state = self.state_path.read_bytes()
        self.initial_state_stat = self.state_path.stat()

    def native_agent(self, pid, boot, inputs):
        return SimpleNamespace(
            _agent_pid=pid, _agent_boot_id=boot, _agent_started_at=f"start-{pid}",
            _agent_inputs_at_start=dict(inputs), _pending_next_action="SELF_STUDY CONTINUE",
            _pending_next_selection_id="synthetic-selection",
            _pending_choice_envelope_v1={"exact": "synthetic authored context"},
            _sovereignty_state_path=lambda: str(self.state_path),
        )

    def status(self, agent, contract, phase, reason):
        return {"pid": agent._agent_pid, "boot_id": agent._agent_boot_id,
                "started_at": agent._agent_started_at,
                "source_inputs_at_start": agent._agent_inputs_at_start,
                "source_path": str(self.root / RUNTIME), "reload_required": phase != "exited",
                "lifecycle_contract": contract, "lifecycle_phase": phase,
                "reason": reason, "next_handoff_contract": native.CONTRACT}

    def validate(self):
        self.validations += 1

    def maintenance(self):
        pass

    def inputs(self):
        return inventory(self.root)

    def config(self):
        return dict(self.settings)

    def protected(self):
        return copy.deepcopy(self.services)

    def identity(self, pid):
        if pid == 10 and self.signals and self.exit_old:
            return None
        return f"start-{pid}"

    def pid(self):
        if self.signals and self.exit_old:
            return None if self.hold_path.exists() else 11
        return 10

    def jobs(self):
        return job_receipt(self.root, full=True)

    def continuity(self):
        return {"file_sha256": file_sha(self.state_path),
                "pending_next_sha256": native.digest(b"SELF_STUDY CONTINUE")}

    def observe(self, pid):
        self.observations += 1
        result = {"quiet": self.quiet and not self.jobs()["active"],
                  "source_checked_at": "same-idle-cycle", "job_index_sha256": "same-jobs",
                  "continuity": self.continuity(), "next_handoff": proof.process(self.old_status)}
        if self.on_observe:
            self.on_observe(result)
        return result

    def terminate(self, pid):
        assert pid == 10 and self.hold_path.is_file()
        self.events.append("signal")
        self.signals.append(pid)
        if self.exit_old:
            self.events.append("old_exit")
            seal = native.seal(self.old_agent)
            self.old_status.update(lifecycle_phase="exited", reason="drained", next_handoff_seal=seal)
            write_json(self.status_path, self.old_status)
        if self.on_terminate:
            self.on_terminate()

    def verify_next_seal(self, before):
        assert self.identity(10) is None
        result = LaunchdAgent.verify_next_seal(self, before)
        self.events.append("seal_verified")
        return result

    def ready(self, pid, expected_inputs):
        assert not self.hold_path.exists() and self.identity(10) is None
        if not self.started_new:
            self.started_new = True
            self.events.append("replacement_start")
            new = self.native_agent(11, "2" * 32, self.inputs())
            self.replacement = new
            restored = native.restore(new)
            status = self.status(new, "agent_drain_v2", "idle", "idle")
            status.update(reload_required=False, next_handoff_restore=restored)
            write_json(self.status_path, status)
            write_json(self.lock_path, {"pid": 11, "interval": 60})
            if self.on_ready:
                self.on_ready(status)
                write_json(self.status_path, status)
        return LaunchdAgent.ready(self, pid, expected_inputs)

    def verify_next_handoff(self, before, pid, inputs):
        result = LaunchdAgent.verify_next_handoff(self, before, pid, inputs)
        self.events.append("restore_verified")
        return result

    def sleep(self, amount):
        self.clock += amount


class BootstrapTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name).resolve()
        self.root = self.directory / "canonical"
        self.loaded_tree = self.directory / "loaded-snapshot"
        self.installed_tree = self.directory / "installed-snapshot"
        self.candidate = self.directory / "qualified-snapshot"
        for root, version, comment in (
            (self.loaded_tree, "v1", "loaded before mailbox repair"),
            (self.root, "v1", "installed mailbox repair"),
            (self.installed_tree, "v1", "installed mailbox repair"),
            (self.candidate, "v2", "qualified drain repair"),
        ):
            files = {
                **{name: "# unchanged overlay\n" for name in OVERLAY},
                RUNTIME: f'STATUS = {{"lifecycle_contract": "agent_drain_{version}"}}\n# {comment}\n',
                "minime_autonomy/maintenance_next.py": 'HANDOFF_VERSION = 1\n',
                LAUNCHER: 'while [ -e "$HOLD_PATH" ] || [ -L "$HOLD_PATH" ]; do\n  sleep 1\ndone\n',
                "scripts/minime_rescue_investigation.py": '# unchanged\n',
                "launchd/com.minime.autonomous-agent.plist": '<plist>unchanged</plist>\n',
            }
            for name, content in files.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
        for tree in (self.loaded_tree, self.installed_tree, self.candidate):
            for path in tree.rglob("*"):
                path.chmod(0o555 if path.is_dir() else 0o444)
            tree.chmod(0o555)
        self.loaded = inventory(self.loaded_tree)
        self.installed = inventory(self.root)
        self.selected = inventory(self.candidate)
        self.backend = BootstrapBackend(self.root, self.loaded)
        self.stage = self.directory / "stage"
        self.stage.mkdir()
        self.stage_identity = {"manifest_sha256": "a" * 64}
        self.loaded_packet = self.directory / "loaded-reconciliation.json"
        self.installed_packet = self.directory / "installed-reconciliation.json"
        for packet, inputs, snapshot in (
            (self.loaded_packet, self.loaded, self.loaded_tree),
            (self.installed_packet, self.installed, self.installed_tree),
        ):
            write_json(packet, {"schema": "minime_launch_source_reconciliation_v1",
                               "selected_inputs": inputs, "canonical_inputs": self.loaded,
                               "snapshot_root": str(snapshot),
                               "bindings": {"canonical_root": str(self.root)}})
        self.qualification = self.directory / "qualification.json"
        write_json(self.qualification, {
            "schema": "minime_drain_paired_qualification_v1", "status": "passed",
            "snapshot": str(self.candidate), "selected_inputs": self.selected,
            "source": str(self.candidate),
            "stage": str(self.stage), "stage_identity": self.stage_identity,
            "checks": 54, "activation_performed": False, "live_eligible_now": False,
        })
        self.receipt = self.directory / "bootstrap.jsonl"
        self.packet = self.directory / "bootstrap-packet"
        self.prior_receipts = [self.directory / f"attempt-{i}.jsonl" for i in (2, 3, 4)]
        self.guard = {"schema": "minime_paired_launch_hold_v1", "id": "retained-owner",
                      "mode": "agent_only", "receipt": str(self.prior_receipts[0]),
                      "stage": str(self.stage), "manifest_sha256": "a" * 64}
        waiting = {"phase": "waiting_for_idle", "old_pid": 10, "old_started_at": "start-10",
                   "source_inputs": self.installed, "config": self.backend.config(),
                   "protected": self.backend.protected()}
        failed = {"outcome": "failed", "owned_hold_retained": True, "forced_termination": False}
        installed_rows = [{"phase": "installation_held", "hold": self.guard}]
        for name in (LAUNCHER, *(name for name in OVERLAY if name != LAUNCHER)):
            installed_rows.extend([
                {"phase": "source_install_intent", "path": name,
                 "before": self.loaded[name], "after": self.installed[name]},
                {"phase": "source_installed", "path": name, "sha256": self.installed[name]},
            ])
        write_rows(self.prior_receipts[0], [*installed_rows, waiting, failed])
        for index in (1, 2):
            recovery = {"phase": "owned_unsignaled_hold_recovered",
                        "previous_receipt": str(self.prior_receipts[0]),
                        "previous_sha256": file_sha(self.prior_receipts[0])}
            if index == 2:
                recovery.update(intervening_receipt=str(self.prior_receipts[1]),
                                intervening_sha256=file_sha(self.prior_receipts[1]))
            write_rows(self.prior_receipts[index], [recovery, waiting, failed])
        write_json(self.backend.hold_path, self.guard)
        self.hold_before = self.backend.hold_path.read_bytes()
        self.events = []
        stage = patch("bridge_stage.verify_stage", return_value=self.stage_identity)
        self.stage_check = stage.start()
        self.addCleanup(stage.stop)
        tools = patch.object(bootstrap, "tools_identity", return_value={"synthetic-tool.py": "f" * 64})
        self.tools_check = tools.start()
        self.addCleanup(tools.stop)
        for name in ("subprocess.run", "subprocess.Popen", "os.kill", "socket.create_connection"):
            guard = patch(name, side_effect=AssertionError("live/service/Git operation forbidden"))
            guard.start()
            self.addCleanup(guard.stop)

    def change_candidate(self, name, raw):
        path = self.candidate / name
        path.chmod(0o644)
        path.write_bytes(raw)
        path.chmod(0o444)

    def prepare(self):
        return bootstrap.prepare(
            self.packet, launch01=self.loaded_packet, launch02=self.installed_packet,
            qualification=self.qualification, receipts=self.prior_receipts,
            expected_pid=10, pause_generation=1, ack="synthetic approved bootstrap", backend=self.backend,
        )

    def activate(self, *, cli=False):
        atomic = bootstrap.atomic_bytes

        def install(path, raw, mode):
            self.assertEqual(path, self.root / RUNTIME)
            self.assertIsNone(self.backend.identity(10))
            self.assert_original_hold()
            self.assertIn("seal_verified", self.backend.events)
            atomic(path, raw, mode)
            self.backend.events.append("source_installed")

        def real_reload(*args, **kwargs):
            self.assertNotIn("quiet_s", kwargs)
            self.assertIs(kwargs["handoff"].__class__, bootstrap.BootstrapHandoff)
            self.assertEqual(kwargs["expected_inputs"], self.installed)
            emit = kwargs["emit"]

            def record(event):
                self.events.append(copy.deepcopy(event))
                emit(event)

            kwargs.update(now=lambda: self.backend.clock, sleep=self.backend.sleep, emit=record)
            self.reload_result = reload_agent(*args, **kwargs)
            return self.reload_result

        with patch.object(bootstrap, "reload_agent", side_effect=real_reload), \
                patch.object(bootstrap, "atomic_bytes", side_effect=install):
            if cli:
                output = io.StringIO()
                argv = [bootstrap.__file__, "activate", "--directory", str(self.packet),
                        "--ack", "synthetic approved bootstrap", "--timeout-secs", "30"]
                with patch.object(sys, "argv", argv), \
                        patch.object(bootstrap, "LaunchdAgent", return_value=self.backend), \
                        redirect_stdout(output):
                    result = bootstrap.main()
                self.cli_stdout = output.getvalue()
                return result
            return bootstrap.activate(self.packet, ack="synthetic approved bootstrap",
                                      timeout_s=30, backend=self.backend)

    def execute(self):
        self.prepare()
        return self.activate()

    def mutate_json(self, path, change):
        value = json.loads(path.read_bytes())
        change(value)
        write_json(path, value)

    def assert_state_preserved(self):
        self.assertEqual(self.backend.state_path.read_bytes(), self.backend.initial_state)
        current = self.backend.state_path.stat()
        self.assertEqual((current.st_ino, current.st_mtime_ns),
                         (self.backend.initial_state_stat.st_ino,
                          self.backend.initial_state_stat.st_mtime_ns))

    def assert_original_hold(self):
        self.assertEqual(self.backend.hold_path.read_bytes(), self.hold_before)

    def assert_not_installed(self):
        self.assertEqual(inventory(self.root), self.installed)
        self.assert_state_preserved()

    def assert_pre_signal_refusal(self, reason=None):
        with self.assertRaises((OSError, ValueError, RuntimeError)) as raised:
            self.execute()
        if reason is not None:
            self.assertRegex(str(raised.exception), reason)
        self.assertEqual(self.backend.signals, [])
        self.assert_original_hold()
        self.assert_not_installed()

    def assert_post_signal_refusal(self, reason=None):
        with self.assertRaises((OSError, ValueError, RuntimeError)) as raised:
            self.execute()
        if reason is not None:
            self.assertRegex(str(raised.exception), reason)
        self.assertEqual(self.backend.signals, [10])
        self.assert_original_hold()
        self.assert_not_installed()

    def test_native_v1_mismatched_loaded_source_bootstraps_to_v2(self):
        self.assertNotEqual(self.loaded, self.installed)
        self.assertNotEqual(self.installed, self.selected)
        result = self.execute()
        self.assertEqual(result["outcome"], "success")
        self.assertEqual(self.reload_result["source_inputs"], self.selected)
        self.assertEqual(self.backend.signals, [10])
        self.assertGreaterEqual(self.backend.clock, 15)
        self.assertGreaterEqual(self.backend.validations, 2)
        self.assertEqual(inventory(self.root), self.selected)
        self.assertEqual(inventory(self.candidate), self.selected)
        self.assertFalse(self.backend.hold_path.exists())
        self.assert_state_preserved()
        self.assertEqual(self.backend.replacement._pending_next_action, "SELF_STUDY CONTINUE")
        self.assertEqual(self.backend.replacement._pending_next_selection_id, "synthetic-selection")
        self.assertEqual(self.backend.replacement._pending_choice_envelope_v1,
                         {"exact": "synthetic authored context"})
        status = json.loads(self.backend.status_path.read_bytes())
        self.assertEqual(status["lifecycle_contract"], "agent_drain_v2")
        self.assertEqual(status["next_handoff_restore"]["seal_sha256"],
                         self.backend.old_status["next_handoff_seal"]["record_sha256"])
        self.assertFalse(result["next_handoff"]["execution_confirmed"])
        order = self.backend.events
        self.assertLess(order.index("old_exit"), order.index("seal_verified"))
        self.assertLess(order.index("seal_verified"), order.index("source_installed"))
        self.assertLess(order.index("source_installed"), order.index("replacement_start"))
        self.assertLess(order.index("replacement_start"), order.index("restore_verified"))

    def test_loaded_inventory_drift_refused(self):
        self.mutate_json(self.loaded_packet,
                         lambda value: value["selected_inputs"].update({RUNTIME: "0" * 64}))
        self.assert_pre_signal_refusal()

    def test_loaded_status_identity_drift_refused(self):
        self.prepare()
        self.mutate_json(self.backend.status_path,
                         lambda value: value.update(boot_id="3" * 32))
        with self.assertRaisesRegex(RuntimeError, "native_process_identity_changed"):
            self.activate()
        self.assertEqual(self.backend.signals, [])
        self.assert_original_hold()
        self.assert_not_installed()

    def test_installed_receipt_inventory_drift_refused(self):
        self.mutate_json(self.installed_packet,
                         lambda value: value["selected_inputs"].update({RUNTIME: "0" * 64}))
        self.assert_pre_signal_refusal()

    def test_candidate_change_outside_runtime_refused(self):
        name = "minime_autonomy/maintenance_next.py"
        self.change_candidate(name, b"HANDOFF_VERSION = 2\n")
        changed = inventory(self.candidate)
        self.mutate_json(self.qualification, lambda value: value.update(selected_inputs=changed))
        self.assert_pre_signal_refusal()

    def test_candidate_missing_key_refused(self):
        self.mutate_json(self.qualification,
                         lambda value: value["selected_inputs"].pop("minime_autonomy/maintenance_next.py"))
        self.assert_pre_signal_refusal()

    def test_candidate_extra_key_refused(self):
        self.mutate_json(self.qualification,
                         lambda value: value["selected_inputs"].update({"extra.py": "0" * 64}))
        self.assert_pre_signal_refusal()

    def test_candidate_bytes_drift_refused(self):
        self.change_candidate(RUNTIME, b"CHANGED_AFTER_QUALIFICATION = True\n")
        self.assert_pre_signal_refusal()

    def test_candidate_runtime_missing_refused(self):
        parent = (self.candidate / RUNTIME).parent
        parent.chmod(0o755)
        (self.candidate / RUNTIME).unlink()
        parent.chmod(0o555)
        self.assert_pre_signal_refusal()

    def test_qualification_not_passed_refused(self):
        self.mutate_json(self.qualification, lambda value: value.update(status="failed"))
        self.assert_pre_signal_refusal()

    def test_qualification_wrong_schema_refused(self):
        self.mutate_json(self.qualification, lambda value: value.update(schema="unknown_v9"))
        self.assert_pre_signal_refusal()

    def test_qualification_wrong_stage_refused(self):
        self.mutate_json(self.qualification,
                         lambda value: value["stage_identity"].update(manifest_sha256="b" * 64))
        self.assert_pre_signal_refusal()

    def test_unknown_hold_retained(self):
        self.mutate_json(self.backend.hold_path, lambda value: value.update(id="another-owner"))
        self.hold_before = self.backend.hold_path.read_bytes()
        self.assert_pre_signal_refusal()

    def test_original_hold_receipt_cannot_be_replaced_by_latest_attempt(self):
        self.mutate_json(self.backend.hold_path,
                         lambda value: value.update(receipt=str(self.prior_receipts[-1])))
        self.hold_before = self.backend.hold_path.read_bytes()
        self.assert_pre_signal_refusal()

    def test_recovery_chain_hash_drift_refused(self):
        rows = [json.loads(line) for line in self.prior_receipts[-1].read_text().splitlines()]
        rows[0]["intervening_sha256"] = "0" * 64
        write_rows(self.prior_receipts[-1], rows)
        self.assert_pre_signal_refusal()

    def test_previously_signaled_hold_not_recoverable(self):
        path = self.prior_receipts[-1]
        rows = [json.loads(line) for line in path.read_text().splitlines()]
        rows.insert(-1, {"phase": "signal_sent", "signal": "SIGTERM", "pid": 10})
        write_rows(path, rows)
        self.assert_pre_signal_refusal()

    def test_active_worker_timeout_is_not_idle(self):
        write_json(self.root / "workspace/llm_jobs/jobs/job_active/job.json", {
            "job_id": "job_active", "status": "timeout", "worker_status": "running",
            "worker_pid": 10, "action_id": "self-study",
        })
        write_json(self.root / "workspace/llm_jobs/index.json", {"recent_jobs": ["job_active"]})
        self.assertEqual(self.backend.jobs()["active"][0]["worker_status"], "running")
        self.assert_pre_signal_refusal("no bounded idle window; no signal sent")
        self.assertEqual(self.backend.clock, 30)

    def test_no_quiet_window_never_signals_or_installs(self):
        self.backend.quiet = False
        self.assert_pre_signal_refusal()

    def test_moving_idle_boundary_never_signals_or_installs(self):
        def move(observation):
            observation["source_checked_at"] = str(self.backend.clock)
        self.backend.on_observe = move
        self.assert_pre_signal_refusal()

    def test_config_change_at_signal_boundary_refused(self):
        def change(_):
            if self.backend.clock >= 15:
                self.backend.settings["configuration"] = "drift"
        self.backend.on_observe = change
        self.assert_pre_signal_refusal()

    def test_protected_process_change_at_signal_boundary_refused(self):
        def change(_):
            if self.backend.clock >= 15:
                self.backend.services["engine"]["pid"] = 99
        self.backend.on_observe = change
        self.assert_pre_signal_refusal()

    def test_old_process_timeout_retains_hold_and_source(self):
        self.backend.exit_old = False
        self.assert_post_signal_refusal()

    def test_exited_without_drained_status_retains_hold(self):
        self.backend.on_terminate = lambda: self.mutate_json(
            self.backend.status_path, lambda value: value.update(reason="failed"))
        self.assert_post_signal_refusal()

    def test_drained_without_exited_status_retains_hold(self):
        self.backend.on_terminate = lambda: self.mutate_json(
            self.backend.status_path, lambda value: value.update(lifecycle_phase="draining"))
        self.assert_post_signal_refusal()

    def test_missing_real_seal_retains_hold(self):
        self.backend.on_terminate = lambda: proof.history_path(self.root / "workspace").unlink()
        self.assert_post_signal_refusal()

    def test_false_seal_receipt_retains_hold(self):
        self.backend.on_terminate = lambda: self.mutate_json(
            self.backend.status_path,
            lambda value: value["next_handoff_seal"].update(record_sha256="0" * 64))
        self.assert_post_signal_refusal()

    def test_candidate_drift_after_old_exit_retains_hold(self):
        self.backend.on_terminate = lambda: self.change_candidate(RUNTIME, b"POST_EXIT_DRIFT = True\n")
        self.assert_post_signal_refusal()

    def test_wrong_native_restoration_ack_cannot_succeed(self):
        self.backend.on_ready = lambda status: status["next_handoff_restore"].update(seal_sha256="0" * 64)
        with self.assertRaises((ValueError, RuntimeError)):
            self.execute()
        self.assertEqual(self.backend.signals, [10])
        self.assert_state_preserved()

    def test_prepare_only_saves_source_never_state_or_releases_hold(self):
        plan = self.prepare()
        self.assertEqual(plan["loaded_inputs"], self.loaded)
        self.assertEqual(plan["installed_inputs"], self.installed)
        self.assertEqual(plan["candidate_inputs"], self.selected)
        self.assertEqual(plan["hold"], self.guard)
        self.assertEqual(plan["hold_sha256"], file_sha(self.backend.hold_path))
        self.assertEqual(set(path.name for path in self.packet.iterdir()),
                         {"runtime.before.py", "runtime.v2.py", "manifest.json"})
        self.assertEqual(file_sha(self.packet / "runtime.before.py"), self.installed[RUNTIME])
        self.assertEqual(file_sha(self.packet / "runtime.v2.py"), self.selected[RUNTIME])
        self.assertEqual((self.packet / "runtime.v2.py").stat().st_mode & 0o777, 0o444)
        self.assertEqual(self.backend.signals, [])
        self.assert_original_hold()
        self.assert_not_installed()

    def test_qualification_bytes_drift_after_prepare_refused(self):
        self.prepare()
        self.mutate_json(self.qualification, lambda value: value.update(checks=55))
        with self.assertRaisesRegex(RuntimeError, "bootstrap_evidence_changed"):
            self.activate()
        self.assertEqual(self.backend.signals, [])
        self.assert_original_hold()
        self.assert_not_installed()

    def test_candidate_bytes_drift_after_prepare_refused(self):
        self.prepare()
        self.change_candidate(RUNTIME, b"POST_PREPARATION_DRIFT = True\n")
        with self.assertRaisesRegex(RuntimeError, "bootstrap_evidence_changed"):
            self.activate()
        self.assertEqual(self.backend.signals, [])
        self.assert_original_hold()
        self.assert_not_installed()

    def test_candidate_copy_bytes_drift_after_prepare_refused(self):
        self.prepare()
        copied = self.packet / "runtime.v2.py"
        copied.chmod(0o644)
        copied.write_text("CORRUPT_STAGED_RUNTIME = True\n")
        with self.assertRaisesRegex(RuntimeError, "bootstrap_evidence_changed"):
            self.activate()
        self.assertEqual(self.backend.signals, [])
        self.assert_original_hold()
        self.assert_not_installed()

    def test_installed_source_drift_after_prepare_not_overwritten(self):
        self.prepare()
        target = self.root / RUNTIME
        target.write_text("EXTERNAL_SOURCE_CHANGE = True\n")
        before = target.read_bytes()
        with self.assertRaises(RuntimeError):
            self.activate()
        self.assertEqual(target.read_bytes(), before)
        self.assertEqual(self.backend.signals, [])
        self.assert_original_hold()
        self.assert_state_preserved()

    def test_tool_inventory_drift_after_prepare_refused(self):
        self.prepare()
        self.tools_check.return_value = {"synthetic-tool.py": "0" * 64}
        with self.assertRaisesRegex(RuntimeError, "bootstrap_evidence_changed"):
            self.activate()
        self.assertEqual(self.backend.signals, [])
        self.assert_original_hold()
        self.assert_not_installed()

    def test_partial_installation_retains_hold_backup_and_no_automatic_retry(self):
        atomic = bootstrap.atomic_bytes

        def partial(path, raw, mode):
            atomic(path, raw, mode)
            raise OSError("synthetic post-replace persistence failure")

        with patch.object(bootstrap, "atomic_bytes", side_effect=partial):
            with self.assertRaisesRegex(OSError, "synthetic post-replace"):
                self.execute()
        self.assertEqual(self.backend.signals, [10])
        self.assertEqual(inventory(self.root), self.selected)
        self.assertEqual(file_sha(self.packet / "runtime.before.py"), self.installed[RUNTIME])
        self.assertEqual(file_sha(self.packet / "runtime.v2.py"), self.selected[RUNTIME])
        self.assert_original_hold()
        self.assert_state_preserved()
        self.assertFalse((self.packet / "completed.json").exists())
        failure = json.loads((self.packet / "failure.json").read_bytes())
        self.assertTrue(failure["owned_hold_retained"])
        self.assertFalse(failure["forced_termination"])
        with self.assertRaisesRegex(RuntimeError, "bootstrap_already_attempted"):
            self.activate()
        self.assertEqual(self.backend.signals, [10])

    def test_active_worker_after_old_exit_retains_hold(self):
        def active():
            write_json(self.root / "workspace/llm_jobs/jobs/job_late/job.json", {
                "job_id": "job_late", "status": "timeout", "worker_status": "running", "worker_pid": 10,
            })
            write_json(self.root / "workspace/llm_jobs/index.json", {"recent_jobs": ["job_late"]})
            self.assertEqual(self.backend.jobs()["active"][0]["worker_status"], "running")
        self.backend.on_terminate = active
        self.assert_post_signal_refusal("old_process_not_drained")

    def test_replacement_v1_cannot_claim_v2_success(self):
        self.backend.on_ready = lambda status: status.update(lifecycle_contract="agent_drain_v1")
        with self.assertRaisesRegex(RuntimeError, "replacement_v2_not_verified"):
            self.execute()
        self.assertEqual(self.backend.signals, [10])
        self.assertFalse((self.packet / "completed.json").exists())
        self.assert_state_preserved()

    def test_receipt_and_stdout_omit_observation_and_new_job_payloads(self):
        secret = "PRIVATE_SYNTHETIC_JOB_ERROR_AND_COMMAND_DO_NOT_RECORD"

        def observation(value):
            value["jobs"] = {"jobs": [{"error": secret, "command": secret}], "active": []}

        def completed(_):
            write_json(self.root / "workspace/llm_jobs/jobs/job_completed/job.json", {
                "job_id": "job_completed", "status": "failed", "worker_status": "failed",
                "worker_pid": 11, "error": secret, "command": secret,
            })
            write_json(self.root / "workspace/llm_jobs/index.json", {"recent_jobs": ["job_completed"]})

        self.backend.on_observe = observation
        self.backend.on_ready = completed
        self.prepare()
        self.assertEqual(self.activate(cli=True), 0)
        boundary = next(event for event in self.events if event.get("phase") == "signal_boundary")
        self.assertIn(secret, json.dumps(boundary["observation"]["jobs"]))
        self.assertIn(secret, json.dumps(self.reload_result["new_jobs_during_rollout"]))
        for path in self.packet.iterdir():
            if path.suffix in {".json", ".jsonl"}:
                self.assertNotIn(secret, path.read_text(), path.name)
        self.assertNotIn(secret, self.cli_stdout)
        self.assertEqual(json.loads(self.cli_stdout)["outcome"], "success")
        self.assert_state_preserved()

    def test_failure_receipt_and_stdout_omit_exception_payload(self):
        secret = "PRIVATE_SYNTHETIC_WORKER_EXCEPTION_DO_NOT_RECORD"

        def fail():
            raise RuntimeError(secret)

        self.backend.on_terminate = fail
        self.prepare()
        self.assertEqual(self.activate(cli=True), 1)
        for path in self.packet.iterdir():
            if path.suffix in {".json", ".jsonl"}:
                self.assertNotIn(secret, path.read_text(), path.name)
        self.assertNotIn(secret, self.cli_stdout)
        self.assertEqual(json.loads(self.cli_stdout)["reason"], "external_gate_refused")
        self.assert_original_hold()
        self.assert_not_installed()


if __name__ == "__main__":
    unittest.main()
