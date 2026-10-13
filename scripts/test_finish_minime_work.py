"""Busy drain qualification with fake time/processes; no live service calls."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock

from finish_minime_work import DrainAgent, finish_accepted_work, recover_unsignaled_hold
from restart_minime_agent import job_receipt, reload_agent
from test_restart_minime_agent import Backend


class BusyBackend(Backend):
    def __init__(self):
        super().__init__()
        self.exit_at = 4
        self.released = False
        self.identity_value = {"boot_id": "old"}
        self.outcome = "completed"
        self.missing = False
        self.seal_calls = 0

    def identity(self, pid):
        if pid == 10 and self.signals and self.clock >= self.exit_at:
            return None
        return f"start-{pid}"

    def pid(self):
        if self.released:
            return 11
        return None if self.identity(10) is None else 10

    def drain_identity(self, pid, inputs, *, stopped=False):
        if inputs != self.source:
            raise RuntimeError("unqualified running inputs")
        if stopped and self.identity(10) is not None:
            raise AssertionError("read stopped acknowledgement while alive")
        return self.identity_value.copy()

    def verify_next_seal(self, identity):
        self.seal_calls += 1
        assert self.identity(10) is None and not self.released
        return {"seal_sha256": "exact", "execution_confirmed": False}

    def verify_next_handoff(self, identity, pid, inputs):
        assert self.released and self.seal_calls == 1
        return {"seal_sha256": "exact", "restore_sha256": "restored"}

    def jobs(self):
        stopped = self.identity(10) is None
        jobs = [] if stopped and self.missing else [{
            "job_id": "job_accepted", "status": self.outcome if stopped else "running",
            "worker_pid": 10, "error": "private synthetic content",
            "worker_status": self.outcome if stopped else "running",
            "outcome_status": self.outcome if stopped else None,
            "outcome_verified": stopped}]
        return {"jobs": jobs, "active": [j for j in jobs if j["status"] == "running"]}


class Hold:
    def __init__(self):
        self.owned = False
        self.transitions = 0

    def begin(self, *args):
        self.owned = True

    def assert_hold(self):
        if not self.owned:
            raise RuntimeError("hold changed")

    def transition(self, backend, protected, emit):
        assert backend.identity(10) is None and backend.seal_calls == 1
        self.transitions += 1
        backend.released = True
        self.owned = False
        return protected


def drain(backend, hold, **kwargs):
    return finish_accepted_work(backend, 10, running_inputs=backend.source,
        expected_inputs=backend.source, handoff=hold, timeout_s=30,
        now=lambda: backend.clock, sleep=backend.sleep, **kwargs)


class DrainTests(unittest.TestCase):
    def test_legacy_idle_bootstrap_also_verifies_seal_before_releasing(self):
        backend, events = Backend(), []
        observe = backend.observe
        backend.observe = lambda pid: {**observe(pid), "next_handoff": {"boot_id": "old"}}
        hold = Mock()
        backend.verify_next_seal = Mock(side_effect=RuntimeError("invalid native seal"))
        with self.assertRaisesRegex(RuntimeError, "invalid native seal"):
            reload_agent(backend, 10, timeout_s=30, quiet_s=10,
                now=lambda: backend.clock, sleep=backend.sleep, handoff=hold, emit=events.append)
        self.assertEqual(backend.signals, [10])
        hold.transition.assert_not_called()
        backend.verify_next_seal.assert_called_once_with({"boot_id": "old"})

    def test_busy_work_finishes_once_before_seal_release_and_restore(self):
        backend, hold, events = BusyBackend(), Hold(), []
        result = drain(backend, hold, emit=events.append)
        self.assertEqual(backend.clock, 4)
        self.assertEqual(backend.signals, [10])
        self.assertEqual(hold.transitions, 1)
        self.assertEqual(result["job_statuses_at_drain"], {"job_accepted": "completed"})
        self.assertEqual(result["new_pid"], 11)
        self.assertFalse(result["forced_termination"])
        self.assertNotIn("private synthetic", json.dumps(events + [result]))

    def test_execution_failure_is_not_invented_success(self):
        for status in ("failed", "timeout", "blocked", "thin_output"):
            backend, hold = BusyBackend(), Hold()
            backend.outcome = status
            result = drain(backend, hold)
            self.assertEqual(result["job_statuses_at_drain"]["job_accepted"], status)
            self.assertFalse(result["execution_confirmed"])

    def test_timeout_retains_hold_without_second_signal(self):
        backend, hold = BusyBackend(), Hold()
        backend.exit_at = 100
        with self.assertRaisesRegex(RuntimeError, "drain timed out"):
            drain(backend, hold)
        self.assertEqual(backend.signals, [10])
        self.assertTrue(hold.owned)
        self.assertEqual(hold.transitions, 0)

    def test_unqualified_loaded_identity_never_signals(self):
        backend, hold = BusyBackend(), Hold()
        backend.drain_identity = Mock(side_effect=RuntimeError("unqualified"))
        with self.assertRaisesRegex(RuntimeError, "unqualified"):
            drain(backend, hold)
        self.assertFalse(hold.owned)
        self.assertEqual(backend.signals, [])

    def test_missing_ack_seal_jobs_or_cancelled_work_never_releases(self):
        for failure in ("ack", "seal", "missing", "active", "canceled"):
            with self.subTest(failure=failure):
                backend, hold = BusyBackend(), Hold()
                if failure == "ack":
                    original = backend.drain_identity
                    def identity(*args, **kwargs):
                        if kwargs.get("stopped"):
                            raise RuntimeError("no drained acknowledgement")
                        return original(*args, **kwargs)
                    backend.drain_identity = identity
                elif failure == "seal":
                    backend.verify_next_seal = Mock(side_effect=RuntimeError("bad seal"))
                elif failure == "missing":
                    backend.missing = True
                else:
                    backend.outcome = "running" if failure == "active" else "canceled"
                with self.assertRaises(RuntimeError):
                    drain(backend, hold)
                self.assertEqual(backend.signals, [10])
                self.assertTrue(hold.owned)
                self.assertEqual(hold.transitions, 0)

    def test_boundary_drift_before_or_after_signal_never_forces(self):
        for when in ("before", "after"):
            for field in ("source", "settings", "services"):
                with self.subTest(when=when, field=field):
                    backend, hold = BusyBackend(), Hold()
                    if when == "before":
                        begin = hold.begin
                        def change(*args):
                            begin(*args)
                            getattr(backend, field)["drift"] = True
                        hold.begin = change
                    else:
                        sleep = backend.sleep
                        def change(amount):
                            sleep(amount)
                            getattr(backend, field)["drift"] = True
                        backend.sleep = change
                    with self.assertRaises(RuntimeError):
                        drain(backend, hold)
                    self.assertEqual(backend.signals, [] if when == "before" else [10])
                    self.assertTrue(hold.owned)
                    self.assertEqual(hold.transitions, 0)

    def test_failed_readiness_never_replays_or_restarts_again(self):
        backend, hold = BusyBackend(), Hold()
        backend.ready_value = False
        with self.assertRaisesRegex(RuntimeError, "readiness timed out"):
            drain(backend, hold)
        self.assertEqual(backend.signals, [10])
        self.assertEqual(hold.transitions, 1)

    def test_timeout_then_failed_outcome_persistence_cannot_release(self):
        backend, hold = BusyBackend(), Hold()
        original = backend.jobs
        def jobs():
            value = original()
            if backend.identity(10) is None:
                row = value["jobs"][0]
                row.update(status="timeout", worker_status="running", outcome_status=None,
                           outcome_verified=False)
                # Even a legacy active-list false negative cannot grant release.
                value["active"] = []
            return value
        backend.jobs = jobs
        with self.assertRaisesRegex(RuntimeError, "outcome unverified"):
            drain(backend, hold)
        self.assertTrue(hold.owned)
        self.assertEqual(hold.transitions, 0)

    def test_native_status_requires_exact_implementation_and_drained_phase(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            runtime = root / "workspace/runtime"
            runtime.mkdir(parents=True)
            status = {"pid": 10, "boot_id": "a" * 32, "started_at": "synthetic",
                      "source_inputs_at_start": {"runtime.py": "a" * 64},
                      "lifecycle_contract": "agent_drain_v2", "lifecycle_phase": "busy",
                      "reason": "loop", "next_handoff_contract": "minime_next_handoff_v1"}
            path = runtime / "autonomous_agent_source_status.json"
            (runtime / "autonomous_agent.lock").write_text('{"pid":10}')
            backend = DrainAgent(1, "synthetic")
            backend.root = root
            path.write_text(json.dumps(status))
            expected = status["source_inputs_at_start"]
            backend.drain_identity(10, expected)
            with self.assertRaises(RuntimeError):
                backend.drain_identity(10, {"runtime.py": "b" * 64})
            with self.assertRaises(RuntimeError):
                backend.drain_identity(10, expected, stopped=True)
            status.update(lifecycle_phase="exited", reason="drained")
            path.write_text(json.dumps(status))
            backend.drain_identity(10, expected, stopped=True)
            status["lifecycle_contract"] = "agent_drain_v1"
            path.write_text(json.dumps(status))
            with self.assertRaises(RuntimeError):
                backend.drain_identity(10, expected, stopped=True)


class RecoveryTests(unittest.TestCase):
    def fixture(self, root):
        backend = BusyBackend()
        backend.root = root
        hold = Hold()
        hold.stage = root
        hold.ready = {"manifest_sha256": "stage"}
        first = root / "first.jsonl"
        guard = {"schema": "minime_paired_launch_hold_v1", "mode": "agent_only",
                 "receipt": str(first), "stage": str(root), "manifest_sha256": "stage"}
        pinned = {"phase": "waiting_for_idle", "old_pid": 10, "old_started_at": "start-10",
                  "source_inputs": backend.source, "config": backend.settings,
                  "protected": backend.services}
        rows = [{"phase": "installation_held", "hold": guard}, pinned,
                {"outcome": "failed", "owned_hold_retained": True, "forced_termination": False}]
        first.write_text("".join(json.dumps(row) + "\n" for row in rows))
        disk = root / "workspace/runtime/agent-launch-hold.json"
        disk.parent.mkdir(parents=True)
        disk.write_text(json.dumps(guard))
        def check():
            if json.loads(disk.read_bytes()) != hold.guard:
                raise RuntimeError("foreign hold")
        hold.assert_hold = check
        return backend, hold, first, rows

    def test_owned_recovery_binds_validated_bytes_without_signal(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            backend, hold, first, rows = self.fixture(root)
            raw = first.read_bytes()
            follow = root / "follow.jsonl"
            follow.write_text("".join(json.dumps(row) + "\n" for row in [
                {"phase": "owned_unsignaled_hold_recovered", "previous_receipt": str(first),
                 "previous_sha256": hashlib.sha256(raw).hexdigest()}, rows[1], rows[-1]]))
            result = recover_unsignaled_hold(backend, hold, [first, follow], backend.source)
            self.assertTrue(hold.owned)
            self.assertEqual(result[0]["sha256"], hashlib.sha256(raw).hexdigest())
            self.assertEqual(backend.signals, [])

    def test_signalled_unknown_changed_or_corrupt_attempt_cannot_be_claimed(self):
        for failure in ("signal", "boundary", "released", "incomplete", "outcome", "mode", "pid", "drift", "unknown"):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp).resolve()
                backend, hold, first, rows = self.fixture(root)
                if failure in {"signal", "boundary", "released"}:
                    phase = {"signal": "signal_sent", "boundary": "signal_boundary",
                             "released": "replacement_admission_released"}[failure]
                    rows.insert(2, {"phase": phase})
                elif failure == "unknown":
                    rows.insert(2, {"phase": "unreviewed_operation"})
                elif failure == "outcome":
                    rows[-1]["outcome"] = "success"
                elif failure == "mode":
                    rows[0]["hold"]["mode"] = "paired"
                elif failure == "pid":
                    rows[1]["old_pid"] = 99
                elif failure == "drift":
                    validate = backend.validate
                    def mutate():
                        validate()
                        first.write_text(first.read_text() + "\n")
                    backend.validate = mutate
                raw = "".join(json.dumps(row) + "\n" for row in rows)
                first.write_text(raw.rstrip() if failure == "incomplete" else raw)
                with self.assertRaises(RuntimeError):
                    recover_unsignaled_hold(backend, hold, [first], backend.source)
                self.assertFalse(hold.owned)
                self.assertEqual(backend.signals, [])


class WorkerEvidenceTests(unittest.TestCase):
    def test_timeout_is_active_until_worker_commits_verified_outcome(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            jobs = root / "workspace/llm_jobs"
            directory = jobs / "jobs/job_test"
            directory.mkdir(parents=True)
            (jobs / "index.json").write_text('{"recent_jobs":["job_test"]}')
            path = directory / "job.json"
            job = {"job_id": "job_test", "status": "timeout", "worker_status": "running",
                   "worker_pid": 10, "outcome": None}
            path.write_text(json.dumps(job))
            before = job_receipt(root, full=True)
            self.assertEqual(len(before["active"]), 1)
            self.assertFalse(before["jobs"][0]["outcome_verified"])
            result = directory / "late_result.txt"
            result.write_text("private synthetic completed result")
            outcome = {"status": "completed", "summary": "private synthetic summary", "error": None,
                       "result_sha256": hashlib.sha256(result.read_bytes()).hexdigest(),
                       "artifact_refs": [], "phase_timings": {}}
            fingerprint = hashlib.sha256(json.dumps(outcome, sort_keys=True).encode()).hexdigest()
            job.update(worker_status="completed", completed_at="synthetic", outcome_sha256=fingerprint,
                       outcome={**outcome, "result_path": str(result)})
            path.write_text(json.dumps(job))
            final = job_receipt(root, full=True)
            self.assertEqual(final["active"], [])
            self.assertTrue(final["jobs"][0]["outcome_verified"])
            self.assertEqual(final["jobs"][0]["outcome_status"], "completed")
            self.assertNotIn("private synthetic", json.dumps(final))
            result.write_text("tampered")
            self.assertFalse(job_receipt(root, full=True)["jobs"][0]["outcome_verified"])
            job["outcome"]["result_path"] = str(root / "outside.txt")
            path.write_text(json.dumps(job))
            self.assertFalse(job_receipt(root, full=True)["jobs"][0]["outcome_verified"])


if __name__ == "__main__":
    unittest.main()
