"""Reload orchestration with fake processes and temporary durable job records."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from datetime import datetime, timedelta
from types import SimpleNamespace

from restart_minime_agent import LaunchdAgent, job_receipt, reload_agent


class Backend:
    def __init__(self):
        self.signals = []
        self.clock = 0
        self.source = {"runtime.py": "a"}
        self.settings = {"settings": "same"}
        self.services = {"engine": 100}
        self.observations = 0
        self.validations = 0
        self.active_until = 0
        self.replace = True
        self.ready_value = True
        self.post_jobs = []

    def validate(self):
        self.validations += 1

    def maintenance(self):
        pass

    def inputs(self):
        return dict(self.source)

    def config(self):
        return dict(self.settings)

    def protected(self):
        return dict(self.services)

    def identity(self, pid):
        return None if pid == 10 and self.signals and self.replace else f"start-{pid}"

    def pid(self):
        return 11 if self.signals and self.replace else 10

    def jobs(self):
        return {"active": [], "jobs": copy.deepcopy(self.post_jobs) if self.signals else []}

    def continuity(self):
        return {"file_sha256": "same", "pending_next_sha256": "retained"}

    def observe(self, pid):
        self.observations += 1
        return {"quiet": self.clock >= self.active_until, "source_checked_at": "cycle",
                "job_index_sha256": "jobs", "continuity": self.continuity()}

    def terminate(self, pid):
        self.signals.append(pid)

    def ready(self, pid, inputs):
        return self.ready_value

    def sleep(self, amount):
        self.clock += amount

    def reload(self):
        return reload_agent(self, 10, timeout_s=30, quiet_s=10,
                            now=lambda: self.clock, sleep=self.sleep)


class ReloadTests(unittest.TestCase):
    def test_waits_for_quiet_and_only_signals_agent_once(self):
        backend = Backend()
        backend.active_until = 3
        result = backend.reload()
        self.assertGreaterEqual(backend.clock, 13)
        self.assertEqual(backend.signals, [10])
        self.assertEqual(result["new_pid"], 11)
        self.assertFalse(result["atomic_traffic_quiescence_claimed"])
        self.assertFalse(result["forced_termination"])

    def test_active_timeout_does_not_signal(self):
        backend = Backend()
        backend.active_until = 100
        with self.assertRaisesRegex(RuntimeError, "idle window"):
            backend.reload()
        self.assertEqual(backend.signals, [])

    def test_missing_new_process_or_readiness_never_forces(self):
        for field in ("replace", "ready_value"):
            backend = Backend()
            setattr(backend, field, False)
            with self.assertRaisesRegex(RuntimeError, "no forced fallback"):
                backend.reload()
            self.assertEqual(backend.signals, [10])

    def test_changed_sources_settings_or_protected_service_abort(self):
        for field in ("source", "settings", "services"):
            backend = Backend()
            def validate():
                backend.validations += 1
                if backend.validations == 2:
                    getattr(backend, field)["changed"] = True
            backend.validate = validate
            with self.assertRaisesRegex(RuntimeError, "changed; no signal"):
                backend.reload()
            self.assertEqual(backend.signals, [])

    def test_moved_signal_boundary_aborts(self):
        backend = Backend()
        original = backend.observe
        def observe(pid):
            value = original(pid)
            if backend.validations == 2:
                value["job_index_sha256"] = "new-job"
            return value
        backend.observe = observe
        with self.assertRaisesRegex(RuntimeError, "idle boundary moved"):
            backend.reload()
        self.assertEqual(backend.signals, [])

    def test_changed_pid_aborts(self):
        backend = Backend()
        backend.pid = lambda: 99
        with self.assertRaisesRegex(RuntimeError, "identity changed"):
            backend.reload()
        self.assertEqual(backend.signals, [])

    def test_lost_maintenance_hold_aborts(self):
        backend = Backend()
        def lost():
            raise RuntimeError("maintenance lost")
        backend.maintenance = lost
        with self.assertRaisesRegex(RuntimeError, "maintenance lost"):
            backend.reload()
        self.assertEqual(backend.signals, [])

    def test_startup_recovery_is_not_misreported_as_graceful(self):
        backend = Backend()
        backend.post_jobs = [{"job_id": "job_late", "status": "failed", "worker_pid": 10,
                             "error": "worker_restarted_before_completion"}]
        with self.assertRaisesRegex(RuntimeError, "interrupted job"):
            backend.reload()
        self.assertEqual(backend.signals, [10])

    def test_job_observer_is_read_only_and_finds_unindexed_jobs(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            jobs = root / "workspace/llm_jobs"
            job_dir = jobs / "jobs/job_test"
            job_dir.mkdir(parents=True)
            path = job_dir / "job.json"
            path.write_text(json.dumps({"job_id": "job_test", "worker_pid": 10, "status": "running"}))
            (jobs / "index.json").write_text(json.dumps({"recent_jobs": []}))
            before = path.read_bytes()
            result = job_receipt(root, full=True)
            self.assertEqual(result["active"][0]["job_id"], "job_test")
            self.assertEqual(path.read_bytes(), before)

    def test_job_observer_rejects_unknown_status_or_path_escape(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            jobs = root / "workspace/llm_jobs"
            jobs.mkdir(parents=True)
            (jobs / "index.json").write_text(json.dumps({"recent_jobs": ["../private"]}))
            with self.assertRaises(ValueError):
                job_receipt(root)

    def test_observation_requires_pid_midcycle_and_no_connections(self):
        with tempfile.TemporaryDirectory() as tmp:
            backend = LaunchdAgent(1, "test")
            backend.root = Path(tmp)
            runtime = backend.root / "workspace/runtime"
            runtime.mkdir(parents=True)
            (backend.root / "workspace/sovereignty_state.json").write_text("{}")
            (runtime / "autonomous_agent.lock").write_text(json.dumps({"pid": 10, "interval": 60}))
            (runtime / "llm_jobs_status.json").write_text('{"active_count": 0}')
            status = {"pid": 10, "checked_at": (datetime.now() - timedelta(seconds=20)).isoformat()}
            logs = backend.root / "logs"
            logs.mkdir()
            (logs / "autonomous-agent.log").write_text(
                datetime.now().strftime("%Y-%m-%d %H:%M:%S,%f")[:-3]
                + " - LLM job active: job_fixture [running]\n")
            path = runtime / "autonomous_agent_source_status.json"
            path.write_text(json.dumps(status))
            with patch("restart_minime_agent.job_receipt", return_value={"active": [], "jobs": [], "index_sha256": "j"}), patch("restart_minime_agent.run") as command:
                command.return_value = SimpleNamespace(returncode=1, stdout="", stderr="")
                self.assertTrue(backend.observe(10)["quiet"])
                command.return_value = SimpleNamespace(returncode=0, stdout="p10\nnlocalhost:11434", stderr="")
                self.assertFalse(backend.observe(10)["quiet"])
                command.return_value = SimpleNamespace(returncode=1, stdout="", stderr="")
                (logs / "autonomous-agent.log").write_text("main loop could still be generating\n")
                self.assertFalse(backend.observe(10)["quiet"])
                status["pid"] = 99
                path.write_text(json.dumps(status))
                with self.assertRaisesRegex(RuntimeError, "identity mismatch"):
                    backend.observe(10)

    def test_new_readiness_requires_actual_pid_and_import_hashes(self):
        with tempfile.TemporaryDirectory() as tmp:
            backend = LaunchdAgent(1, "test")
            backend.root = Path(tmp)
            runtime = backend.root / "workspace/runtime"
            runtime.mkdir(parents=True)
            (runtime / "autonomous_agent.lock").write_text('{"pid": 11}')
            path = runtime / "autonomous_agent_source_status.json"
            status = {"pid": 11, "source_inputs_at_start": {"helper.py": "new"},
                      "reload_required": False, "reason": "loop", "lifecycle_contract": "agent_drain_v1"}
            path.write_text(json.dumps(status))
            self.assertTrue(backend.ready(11, {"helper.py": "new"}))
            self.assertFalse(backend.ready(11, {"helper.py": "old"}))
            self.assertFalse(backend.ready(10, {"helper.py": "new"}))


if __name__ == "__main__":
    unittest.main()
