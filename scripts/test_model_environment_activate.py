"""No production subprocesses: provider transition ordering and refusal gates."""
from __future__ import annotations

import copy
import contextlib
import json
import io
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import types
import unittest
import urllib.error
from unittest.mock import patch

import model_environment_activate as activate
import model_environment as environments


class FakeBackend:
    def __init__(self, fail=None, error=RuntimeError):
        self.events = []
        self.receipts = []
        self.fail = fail
        self.error = error
        self.clock = 0
        self.idle_value = True
        self.still_running = False

    def event(self, name):
        self.events.append(name)
        if self.fail == name:
            raise self.error(name)

    def inspect(self, pid, *, migrate):
        self.event("inspect")
        return {"started_at": "original", "pid": pid}

    def begin(self, receipt):
        self.event("begin")

    def record(self, receipt):
        self.receipts.append(copy.deepcopy(receipt))

    def pre_stack_receipt(self): self.event("pre_stack")
    def post_stack_receipt(self): self.event("post_stack")
    def hold(self): self.event("hold")
    def retain_hold(self): self.event("retain_hold")
    def arm_next_spawn(self): self.event("arm")
    def idle(self):
        self.event("idle")
        return self.idle_value
    def assert_old(self, *_): self.event("assert_old")
    def revalidate(self): self.event("revalidate")
    def signal(self): self.event("SIGTERM")
    def old_exists(self, *_):
        self.event("old_exists")
        return self.still_running
    def wait_held(self, *_):
        self.event("wait_held")
        return {"pid": 200, "started_at": "new"}
    def migrate_held(self, *_): self.event("migrate_held")
    def restore_held_job(self): self.event("restore_held_job")
    def select(self): self.event("select")
    def release(self): self.event("release")
    def verify_ready(self, *_):
        self.event("verify_ready")
        return {"pid": 200}
    def publish(self): self.event("publish")
    def sleep(self, elapsed): self.clock += elapsed

    def transition(self, **kwargs):
        return activate.transition(self, expected_pid=100, actor="fixture", ack="owned test",
                                   migrate=kwargs.pop("migrate", True), timeout=2,
                                   now=lambda: self.clock, sleep=self.sleep, **kwargs)


class TransitionTests(unittest.TestCase):
    def test_absent_job_recovery_bootstraps_held_without_signalling_or_bootout(self):
        backend = FakeBackend()
        with patch.object(backend, "inspect", return_value={"interrupted_migration_job_absent": True}):
            result = activate.transition(backend, expected_pid=0, actor="fixture", ack="recover owned absent job",
                                         migrate=True, rollback=True, timeout=2,
                                         now=lambda: backend.clock, sleep=backend.sleep)
        for forbidden in ("SIGTERM", "arm", "migrate_held", "idle", "pre_stack", "hold"):
            self.assertNotIn(forbidden, backend.events)
        self.assertLess(backend.events.index("revalidate"), backend.events.index("restore_held_job"))
        self.assertLess(backend.events.index("restore_held_job"), backend.events.index("select"))
        self.assertFalse(result["signal_sent"])

    def test_pid_zero_cannot_start_ordinary_activation_or_nonmigration_rollback(self):
        for rollback, migrate in ((False, True), (False, False), (True, False)):
            backend = FakeBackend()
            with self.subTest(rollback=rollback, migrate=migrate), self.assertRaises(ValueError):
                activate.transition(backend, expected_pid=0, actor="fixture", ack="test", migrate=migrate, rollback=rollback)
            self.assertEqual(backend.events, [])

    def test_migration_orders_hold_signal_exit_selection_and_readiness(self):
        backend = FakeBackend()
        result = backend.transition()
        self.assertEqual(result["status"], "activated_verified")
        ordering = ["hold", "arm", "SIGTERM", "old_exists", "wait_held", "migrate_held",
                    "select", "release", "verify_ready", "publish", "post_stack"]
        indexes = [backend.events.index(event) for event in ordering]
        self.assertEqual(indexes, sorted(indexes))
        self.assertEqual(backend.events.count("SIGTERM"), 1)
        self.assertFalse(result["force_used"])
        self.assertFalse(result["runtime_state_rollback"])
        self.assertFalse(result["atomic_traffic_quiescence_claimed"])
        self.assertLess(backend.events.index("revalidate"), backend.events.index("idle"))

    def test_idle_rechecked_after_expensive_validation_before_signal(self):
        backend = FakeBackend()
        backend.transition()
        before_signal = backend.events[:backend.events.index("SIGTERM")]
        self.assertEqual(before_signal[-4:], ["idle", "assert_old", "idle", "assert_old"])

    def test_explicit_recovery_of_held_process_never_signals_again(self):
        backend = FakeBackend()
        with patch.object(backend, "inspect", return_value={"already_held": True}):
            result = backend.transition(migrate=False, rollback=True)
        for action in ("SIGTERM", "arm", "old_exists", "pre_stack", "hold"):
            self.assertNotIn(action, backend.events)
        self.assertFalse(result["signal_sent"])
        self.assertIn("select", backend.events)

    def test_rollback_uses_existing_launcher_without_bootout_debug_or_state_restore(self):
        backend = FakeBackend()
        result = backend.transition(migrate=False, rollback=True)
        self.assertNotIn("arm", backend.events)
        self.assertNotIn("migrate_held", backend.events)
        self.assertTrue(result["rollback"])
        self.assertFalse(result["runtime_state_rollback"])

    def test_failed_host_gate_is_before_any_mutation(self):
        backend = FakeBackend(fail="inspect")
        with self.assertRaises(RuntimeError): backend.transition()
        self.assertEqual(backend.events, ["inspect"])
        self.assertFalse(backend.receipts)

    def test_every_failure_boundary_retains_hold_and_durable_failure(self):
        for boundary in ("pre_stack", "hold", "arm", "idle", "assert_old", "revalidate",
                         "SIGTERM", "old_exists", "wait_held", "migrate_held", "select",
                         "release", "verify_ready", "publish", "post_stack"):
            with self.subTest(boundary=boundary):
                backend = FakeBackend(fail=boundary)
                with self.assertRaises(RuntimeError): backend.transition()
                self.assertEqual(backend.events[-1], "retain_hold")
                self.assertEqual(backend.receipts[-1]["status"], "failed_requires_review")
                self.assertLessEqual(backend.events.count("SIGTERM"), 1)
                self.assertFalse(backend.receipts[-1]["force_used"])

    def test_interrupt_after_arming_is_not_clean_no_change(self):
        backend = FakeBackend(fail="idle", error=KeyboardInterrupt)
        with self.assertRaises(KeyboardInterrupt): backend.transition()
        receipt = backend.receipts[-1]
        self.assertTrue(receipt["debug_override_attempted"])
        self.assertTrue(receipt["debug_override_armed"])
        self.assertFalse(receipt["signal_sent"])
        self.assertEqual(receipt["status"], "failed_requires_review")
        self.assertIn("retain_hold", backend.events)

    def test_no_idle_window_never_signals(self):
        backend = FakeBackend()
        backend.idle_value = False
        with self.assertRaisesRegex(RuntimeError, "no idle"):
            backend.transition()
        self.assertNotIn("SIGTERM", backend.events)

    def test_drain_timeout_never_migrates_or_escalates(self):
        backend = FakeBackend()
        backend.still_running = True
        with self.assertRaisesRegex(RuntimeError, "no forced termination"):
            backend.transition()
        self.assertEqual(backend.events.count("SIGTERM"), 1)
        self.assertNotIn("migrate_held", backend.events)
        self.assertNotIn("select", backend.events)

    def test_readiness_failure_retains_hold_without_automatic_rollback(self):
        backend = FakeBackend(fail="verify_ready")
        with self.assertRaises(RuntimeError): backend.transition()
        self.assertNotIn("publish", backend.events)
        self.assertEqual(backend.events.count("select"), 1)
        self.assertTrue(backend.receipts[-1]["hold_released"])
        self.assertIn("retain_hold", backend.events)

    def test_all_interrupt_boundaries_retain_ownership(self):
        for boundary in ("arm", "SIGTERM", "wait_held", "migrate_held", "select", "release", "verify_ready"):
            with self.subTest(boundary=boundary):
                backend = FakeBackend(fail=boundary, error=KeyboardInterrupt)
                with self.assertRaises(KeyboardInterrupt): backend.transition()
                self.assertIn("retain_hold", backend.events)
                self.assertEqual(backend.receipts[-1]["status"], "failed_requires_review")


class FixtureProofTests(unittest.TestCase):
    def test_check_only_failed_fixture_never_reaches_preflight_or_writes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            backend = activate.LaunchdBackend(control_root=root / "control", manifest=root / "candidate.json",
                                              baseline=root / "baseline.json", fixture_proof=root / "proof.json",
                                              actor="fixture", ack="model-free refusal check")
            with patch.object(activate, "verify_fixture", side_effect=RuntimeError("unsupported host")), \
                 patch.object(backend, "preflight") as preflight:
                with self.assertRaisesRegex(RuntimeError, "unsupported host"):
                    backend.inspect(12345, migrate=True)
                preflight.assert_not_called()
            self.assertEqual(list(root.iterdir()), [])

    def test_bootout_refused_without_exact_verified_held_identity(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            backend = activate.LaunchdBackend(control_root=root / "control", manifest=root / "candidate.json",
                                              baseline=root / "baseline.json", fixture_proof=root / "proof.json",
                                              actor="fixture", ack="model-free refusal check")
            with patch.object(backend, "held", return_value={"pid": 2, "hold_only": True}), \
                 patch.object(activate, "run") as run:
                with self.assertRaisesRegex(RuntimeError, "exact one-shot"):
                    backend.migrate_held({"pid": 3, "hold_only": True})
                run.assert_not_called()

    def test_arbitrary_passed_json_is_not_host_qualification(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "proof.json"
            environments.atomic_json(path, {"passed": True})
            with patch.object(activate, "run") as run:
                run.return_value.stdout = "fixture-host"
                with self.assertRaisesRegex(RuntimeError, "fixture not qualified"):
                    activate.verify_fixture(path)

    def test_real_failed_fixture_cannot_admit_transition(self):
        path = Path(__file__).resolve().parents[1] / "docs/steward-notes/2026-10-15-mlx-upgrade/launchd-handoff-fixture-01.json"
        if not path.exists(): self.skipTest("host-specific evidence not present")
        with patch.object(activate, "run") as run:
            run.return_value.stdout = "fixture-host"
            with self.assertRaisesRegex(RuntimeError, "fixture not qualified"):
                activate.verify_fixture(path)


class ReadinessTests(unittest.TestCase):
    def setUp(self):
        self.good = {"ready": True, "worker": {"phase": "ready", "queue_depth": 0,
                                               "last_generation_error": None},
                     "reservoir": {"status": "connected"}}

    def test_error_cannot_pass_even_when_gateway_ready_is_true(self):
        for change in (lambda h: h["worker"].update(last_generation_error="failed generation"),
                       lambda h: h.update(error="worker failure"),
                       lambda h: h["worker"].update(phase="starting"),
                       lambda h: h["reservoir"].update(status="degraded")):
            health = copy.deepcopy(self.good)
            change(health)
            self.assertFalse(activate.healthy_readiness(health))
            self.assertFalse(activate.healthy_readiness(health, idle=True))

    def test_generating_is_ready_but_not_idle(self):
        self.good["worker"]["phase"] = "generating"
        self.assertTrue(activate.healthy_readiness(self.good))
        self.assertFalse(activate.healthy_readiness(self.good, idle=True))


class QualificationBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()

    def backend(self, **kwargs):
        return activate.LaunchdBackend(control_root=self.root / "control",
                                       manifest=self.root / "candidate.json", baseline=self.root / "baseline.json",
                                       fixture_proof=self.root / "fixture.json", actor="fixture", ack="test", **kwargs)

    def test_missing_quality_proof_refuses_before_preflight_or_writes(self):
        backend = self.backend()
        with patch.object(activate, "verify_fixture"), patch.object(backend, "preflight") as preflight:
            with self.assertRaisesRegex(ValueError, "qualification proof"):
                backend.inspect(12345, migrate=True)
            preflight.assert_not_called()
        self.assertEqual(list(self.root.iterdir()), [])

    def test_quality_proof_failure_refuses_before_preflight_or_writes(self):
        backend = self.backend(qualification_proof=self.root / "proof.json")
        module = types.SimpleNamespace(verify_proof=lambda *_: (_ for _ in ()).throw(ValueError("review missing")))
        with patch.dict(sys.modules, model_qualification=module), patch.object(activate, "verify_fixture"), \
             patch.object(backend, "preflight") as preflight:
            with self.assertRaisesRegex(ValueError, "review missing"):
                backend.inspect(12345, migrate=True)
            preflight.assert_not_called()
        self.assertEqual(list(self.root.iterdir()), [])

    def test_changed_proof_before_selection_does_not_write_selection(self):
        backend = self.backend()
        backend.initial = {"qualification": {"proof_sha256": "original"}}
        with patch.object(environments, "assert_hold"), \
             patch.object(backend, "qualification", return_value={"proof_sha256": "changed"}), \
             patch.object(environments, "atomic_json") as write:
            with self.assertRaisesRegex(RuntimeError, "qualification evidence changed"):
                backend.select()
            write.assert_not_called()

    def test_rollback_cannot_relabel_candidate_as_preserved_baseline(self):
        backend = self.backend(rollback=True)
        backend.baseline = backend.manifest
        backend.manifest.write_text("{}")
        with self.assertRaisesRegex(ValueError, "invalid record"):
            backend.qualification()
        environments.atomic_json(backend.root / "rollback-baseline.json",
                                 {"schema": "model_environment_rollback_baseline_v1",
                                  "manifest": str(self.root / "original.json"), "manifest_sha256": "original"})
        with self.assertRaisesRegex(ValueError, "invalid record"):
            backend.qualification()

    def test_rollback_quality_exemption_requires_exact_anchored_manifest(self):
        backend = self.backend(rollback=True)
        backend.manifest = backend.baseline
        backend.baseline.write_text("{}")
        environments.atomic_json(backend.root / "rollback-baseline.json", backend.baseline_anchor())
        self.assertEqual(backend.qualification()["exempt"], "explicit_preserved_baseline_rollback")
        backend.baseline.write_text('{"changed":true}')
        with self.assertRaisesRegex(ValueError, "invalid record"):
            backend.qualification()

    def test_full_stage_rollback_accepts_only_recorded_core_predecessor(self):
        backend = self.backend(rollback=True)
        backend.manifest = backend.baseline
        backend.baseline.write_text('{"release":"core"}')
        environments.atomic_json(backend.root / "rollback-baseline.json",
                                 {"schema": "model_environment_rollback_baseline_v1",
                                  "manifest": str(self.root / "original.json"), "manifest_sha256": "original"})
        full = str(self.root / "full.json")
        environments.atomic_json(backend.root / "active.json",
                                 {"schema": environments.SELECTION_SCHEMA, "manifest": full,
                                  "manifest_sha256": "full"})
        receipt_path = backend.root / "transactions" / "prior" / "receipt.json"
        receipt = {"status": "activated_verified", "manifest": full,
                   "baseline_manifest": str(backend.baseline),
                   "old_identity": {"baseline_manifest_sha256": environments.digest(backend.baseline)},
                   "new_process": {"manifest_sha256": "full"},
                   "force_used": False, "runtime_state_rollback": False}
        environments.atomic_json(receipt_path, receipt)
        self.assertEqual(backend.qualification()["baseline"]["accepted_transaction"], str(receipt_path))
        receipt["old_identity"]["baseline_manifest_sha256"] = "different"
        environments.atomic_json(receipt_path, receipt)
        with self.assertRaisesRegex(ValueError, "accepted predecessor"):
            backend.qualification()

    def test_later_rollback_requires_verified_receipt_status(self):
        backend = self.backend(rollback=True)
        backend.manifest = backend.baseline
        backend.baseline.write_text("{}")
        environments.atomic_json(backend.root / "rollback-baseline.json", {"original": "preserved"})
        environments.atomic_json(backend.root / "active.json",
                                 {"schema": environments.SELECTION_SCHEMA, "manifest": "full",
                                  "manifest_sha256": "full"})
        environments.atomic_json(backend.root / "transactions/prior/receipt.json",
                                 {"status": "failed_requires_review", "manifest": "full",
                                  "new_process": {"manifest_sha256": "full"}})
        with self.assertRaisesRegex(ValueError, "verified activation receipt"):
            backend.qualification()

    def test_core_retry_after_original_rollback_needs_no_core_predecessor(self):
        backend = self.backend()
        backend.baseline.write_text("{}")
        backend.require_predecessor_acceptance({"predecessor_acceptance": None},
                                               backend.baseline_anchor(), environments.digest(backend.baseline))

    def test_full_stage_requires_exact_accepted_core_observation_binding(self):
        backend = self.backend()
        backend.baseline.write_text("{}")
        for proof in ({}, {"predecessor_acceptance": None},
                      {"predecessor_acceptance": {"manifest": {"path": "other", "sha256": "core"}}}):
            with self.subTest(proof=proof), self.assertRaisesRegex(ValueError, "postobservations"):
                backend.require_predecessor_acceptance(proof, {"original": "preserved"}, "core")
        backend.require_predecessor_acceptance(
            {"predecessor_acceptance": {"manifest": {"path": str(backend.baseline), "sha256": "core"}}},
            {"original": "preserved"}, "core")

    def test_core_promotion_requires_same_passively_observed_process(self):
        observed = {"samples": [{"provider": {"pids": [123], "identity": {
            "stdout": "123 Thu Oct 15 10:00:00 2026 /exact/python server.py"}}}]}
        with patch("model_qualification.load_reference", return_value=observed):
            self.assertEqual(activate.qualified_process({}, 123, "Thu Oct 15 10:00:00 2026"),
                             {"pid": 123, "started_at": "Thu Oct 15 10:00:00 2026"})
            for pid, started_at in ((124, "Thu Oct 15 10:00:00 2026"), (123, "Thu Oct 15 11:00:00 2026")):
                with self.subTest(pid=pid, started_at=started_at), self.assertRaisesRegex(ValueError, "repeat the baseline"):
                    activate.qualified_process({}, pid, started_at)

    def test_full_promotion_requires_same_observed_core_pid_and_start(self):
        proof = {"predecessor_acceptance": {"provider": {"pid": 123, "started_at": "original start"}}}
        self.assertEqual(activate.qualified_process(proof, 123, "original start"), proof["predecessor_acceptance"]["provider"])
        for pid, started_at in ((124, "original start"), (123, "later start")):
            with self.subTest(pid=pid, started_at=started_at), self.assertRaisesRegex(ValueError, "repeat the baseline"):
                activate.qualified_process(proof, pid, started_at)

    def test_immediate_presignal_check_retains_qualified_process_binding(self):
        backend = self.backend()
        initial = {"started_at": "same", "qualified_process": {"pid": 123, "started_at": "same"}}
        with patch.object(environments, "assert_hold"), patch.object(backend, "pid", return_value=123), \
             patch.object(environments, "process_start", return_value="same"):
            backend.assert_old(123, initial)
            initial["qualified_process"]["started_at"] = "different"
            with self.assertRaisesRegex(RuntimeError, "qualified observation process"):
                backend.assert_old(123, initial)


class PrivilegeBoundaryTests(unittest.TestCase):
    def backend(self, root, **kwargs):
        backend = activate.LaunchdBackend(control_root=root / "control", manifest=root / "candidate.json",
                                          baseline=root / "baseline.json", fixture_proof=root / "fixture.json",
                                          actor="fixture", ack="test", **kwargs)
        backend.migrate = True
        return backend

    def test_sudo_debug_scopes_privilege_to_exact_service_and_inherits_terminal(self):
        with tempfile.TemporaryDirectory() as temporary:
            backend = self.backend(Path(temporary).resolve(), sudo_debug=True)
            with patch.object(environments, "assert_hold"), patch.object(sys.stdin, "isatty", return_value=True), \
                 patch.object(activate.subprocess, "run") as run:
                backend.arm_next_spawn()
            args, kwargs = run.call_args
            self.assertEqual(args[0][:5], ["/usr/bin/sudo", "--", "/bin/launchctl", "debug",
                                         f"gui/{os.getuid()}/{activate.LABEL}"])
            self.assertEqual(kwargs, {"check": True, "timeout": 180})

    def test_sudo_debug_refuses_other_targets_nonmigration_and_nonterminal(self):
        with tempfile.TemporaryDirectory() as temporary:
            backend = self.backend(Path(temporary).resolve(), sudo_debug=True)
            with patch.object(environments, "assert_hold"), patch.object(activate.subprocess, "run") as run:
                with patch.object(sys.stdin, "isatty", return_value=False):
                    with self.assertRaisesRegex(ValueError, "interactive Terminal"):
                        backend.arm_next_spawn()
                with patch.object(sys.stdin, "isatty", return_value=True):
                    backend.target = f"gui/{os.getuid()}/another.service"
                    with self.assertRaisesRegex(ValueError, "validated first"):
                        backend.arm_next_spawn()
                    backend.migrate = False
                    with self.assertRaisesRegex(ValueError, "first launcher migration"):
                        backend.arm_next_spawn()
                run.assert_not_called()

    def test_whole_controller_cannot_run_as_root(self):
        with tempfile.TemporaryDirectory() as temporary:
            backend = self.backend(Path(temporary).resolve())
            with patch.object(os, "geteuid", return_value=0):
                with self.assertRaisesRegex(ValueError, "never sudo the whole"):
                    backend.validate_privilege_boundary(migrate=True)


class RecoveryBackendTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.failed = self.root / "control/transactions/failed"
        self.failed.mkdir(parents=True)
        self.backend = activate.LaunchdBackend(control_root=self.root / "control", manifest=self.root / "baseline.json",
                                              baseline=self.root / "baseline.json", fixture_proof=self.root / "fixture.json",
                                              actor="recovery-actor", ack="owned CPU fixture", rollback=True,
                                              resume_transaction=self.failed)
        backend = self.backend
        backend.guard = environments.create_hold(backend.root, "failed", "original-owner")
        backend.source_plist, backend.installed_plist = self.root / "source.plist", self.root / "installed.plist"
        python = str(self.root / ".venv/bin/python")
        self.settings = {"Label": activate.LABEL, "KeepAlive": True,
                         "ProgramArguments": [python, "coupled_astrid_server.py", "--port", "8090"]}
        payload = plistlib.dumps(self.settings)
        for path in (backend.source_plist, backend.installed_plist,
                     self.failed / "previous-source.plist", self.failed / "previous-installed.plist"):
            path.write_bytes(payload)
        old = {"pid": 111, "started_at": "old start", "settings": self.settings,
               "source_plist_sha256": environments.digest(backend.source_plist),
               "installed_plist_sha256": environments.digest(backend.installed_plist)}
        self.receipt = {"status": "failed_requires_review", "migration": True, "old_process_exited": True,
                        "migration_phase": "held_job_removed", "old_identity": old,
                        "held_process": {"pid": 222, "started_at": "held start", "hold_only": True,
                                         "hold": backend.guard}}
        environments.atomic_json(self.failed / "receipt.json", self.receipt)
        backend.checked = {"manifest": {"python": python}}
        backend.initial = {"interrupted_migration_job_absent": True, "settings": self.settings,
                           "source_plist_sha256": old["source_plist_sha256"],
                           "installed_plist_sha256": old["installed_plist_sha256"]}
        backend.current_receipt = {"status": "preparing"}

    def inspect_absent(self):
        with patch.object(environments, "process_start", return_value=None), \
             patch.object(self.backend, "job_loaded", return_value=False), \
             patch.object(self.backend, "listeners", return_value=[]):
            return self.backend.inspect_absent_migration(self.receipt)

    def test_absent_recovery_accepts_only_exact_old_or_permanent_plist_bytes(self):
        self.assertEqual(self.inspect_absent(), self.settings)
        permanent = dict(self.settings, ProgramArguments=self.backend.launcher_arguments())
        self.backend.source_plist.write_bytes(plistlib.dumps(permanent))
        self.assertEqual(self.inspect_absent(), self.settings)
        self.backend.installed_plist.write_bytes(plistlib.dumps(permanent))
        self.assertEqual(self.inspect_absent(), self.settings)
        self.backend.source_plist.write_bytes(plistlib.dumps(dict(permanent, UnownedSetting=True)))
        with self.assertRaisesRegex(ValueError, "neither exact original"):
            self.inspect_absent()

    def test_absent_recovery_rejects_changed_saved_snapshot_and_missing_phase(self):
        self.receipt["old_identity"]["source_plist_sha256"] = "changed"
        with self.assertRaisesRegex(ValueError, "preserved original"):
            self.inspect_absent()
        self.receipt["migration_phase"] = "unknown"
        with self.assertRaisesRegex(ValueError, "interrupted owned"):
            self.inspect_absent()

    def test_absent_recovery_rejects_any_surviving_original_or_held_pid(self):
        for current in ("old start", "held start", "foreign recycled process"):
            for survivor in (111, 222):
                with self.subTest(current=current, survivor=survivor), \
                     patch.object(environments, "process_start", side_effect=lambda pid: current if pid == survivor else None), \
                     self.assertRaisesRegex(RuntimeError, "PID still exists"):
                    self.backend.inspect_absent_migration(self.receipt)

    def test_absent_recovery_rejects_loaded_job_and_every_listener(self):
        for loaded, listeners in ((True, []), (False, ["111"]), (False, ["99999"])):
            with self.subTest(loaded=loaded, listeners=listeners), \
                 patch.object(environments, "process_start", return_value=None), \
                 patch.object(self.backend, "job_loaded", return_value=loaded), \
                 patch.object(self.backend, "listeners", return_value=listeners), \
                 self.assertRaisesRegex(RuntimeError, "no launchd job and no model listener"):
                self.backend.inspect_absent_migration(self.receipt)

    def test_absent_recovery_requires_owned_hold_and_rollback(self):
        self.backend.rollback = False
        with self.assertRaisesRegex(ValueError, "interrupted owned"):
            self.inspect_absent()
        self.backend.rollback = True
        self.receipt["held_process"]["hold"] = {"transaction_id": "foreign"}
        with self.assertRaisesRegex(ValueError, "ownership"):
            self.inspect_absent()

    def test_absence_probe_does_not_treat_permissions_or_unknown_errors_as_absent(self):
        absent = f'Could not find service "{activate.LABEL}" in domain for user gui: {os.getuid()}'
        for code, stderr, expected in ((113, absent, False), (0, "", True)):
            with patch.object(activate, "run", return_value=types.SimpleNamespace(returncode=code, stderr=stderr)):
                self.assertEqual(self.backend.job_loaded(), expected)
        for code, stderr in ((1, "permission denied"), (113, "different service"), (113, "unknown domain")):
            with patch.object(activate, "run", return_value=types.SimpleNamespace(returncode=code, stderr=stderr)), \
                 self.assertRaisesRegex(RuntimeError, "cannot prove"):
                self.backend.job_loaded()

    def test_resumed_failure_after_release_recreates_original_guard(self):
        environments.release_hold(self.backend.root, self.backend.guard)
        self.backend.retain_hold()
        self.assertEqual(environments.hold_record(self.backend.root), self.backend.guard)
        self.assertEqual(self.backend.guard["actor"], "original-owner")
        self.assertEqual(self.backend.guard["transaction_id"], "failed")

    def test_resume_never_replaces_foreign_hold(self):
        environments.release_hold(self.backend.root, self.backend.guard)
        foreign = environments.create_hold(self.backend.root, "foreign", "other-owner")
        with self.assertRaisesRegex(RuntimeError, "ownership changed"):
            self.backend.retain_hold()
        self.assertEqual(environments.hold_record(self.backend.root), foreign)

    def test_recovery_installs_atomically_then_bootstraps_under_existing_hold(self):
        with patch.object(self.backend, "job_loaded", return_value=False), \
             patch.object(self.backend, "listeners", return_value=[]), \
             patch.object(self.backend, "loaded_arguments", return_value=self.backend.launcher_arguments()), \
             patch.object(activate, "run") as run:
            self.backend.install_held_launcher()
        for path in (self.backend.source_plist, self.backend.installed_plist):
            self.assertEqual(plistlib.loads(path.read_bytes())["ProgramArguments"], self.backend.launcher_arguments())
        self.assertEqual(environments.hold_record(self.backend.root), self.backend.guard)
        run.assert_called_once_with("launchctl", "bootstrap", self.backend.domain, str(self.backend.installed_plist))
        self.assertEqual(self.backend.current_receipt["migration_phase"], "bootstrap_pending")

    def test_every_atomic_plist_replace_failure_leaves_only_old_or_permanent_bytes(self):
        old = plistlib.dumps(self.settings)
        permanent = plistlib.dumps(dict(self.settings, ProgramArguments=self.backend.launcher_arguments()))
        real_replace = os.replace
        for fail_at in (1, 2):
            for path in (self.backend.source_plist, self.backend.installed_plist):
                path.write_bytes(old)
            calls = 0
            def replace(source, target):
                nonlocal calls
                if Path(target) in {self.backend.source_plist, self.backend.installed_plist}:
                    calls += 1
                    if calls == fail_at:
                        raise OSError("injected atomic replacement failure")
                return real_replace(source, target)
            with self.subTest(fail_at=fail_at), patch.object(self.backend, "job_loaded", return_value=False), \
                 patch.object(self.backend, "listeners", return_value=[]), patch.object(os, "replace", side_effect=replace), \
                 patch.object(activate, "run") as run, self.assertRaises(OSError):
                self.backend.install_held_launcher()
            run.assert_not_called()
            for path in (self.backend.source_plist, self.backend.installed_plist):
                self.assertIn(path.read_bytes(), (old, permanent))
            for path in self.root.glob(".*.pending"):
                path.unlink()  # Owned temporary CPU fixture only.

    def test_rollback_idle_requires_exact_listener_and_ready_empty_worker(self):
        self.backend.initial = {"pid": 123}
        unhealthy = {"model": "coupled-astrid", "ready": False, "reservoir": {"status": "degraded"},
                     "worker": {"phase": "ready", "queue_depth": 0, "last_generation_error": "failed generation"}}
        with patch.object(self.backend, "health", return_value=unhealthy), \
             patch.object(self.backend, "listeners", return_value=["123"]):
            self.assertTrue(self.backend.idle())
            self.backend.rollback = False
            self.assertFalse(self.backend.idle())
            self.backend.rollback = True
            for phase, depth in (("generating", 0), ("starting", 0), ("failed", 0), ("ready", 1), ("ready", None)):
                unhealthy["worker"].update(phase=phase, queue_depth=depth)
                self.assertFalse(self.backend.idle())
            unhealthy["worker"].update(phase="ready", queue_depth=0)
            with patch.object(self.backend, "listeners", return_value=["999"]):
                self.assertFalse(self.backend.idle())

    def test_http_503_retains_worker_state_without_promoting_it_to_healthy(self):
        payload = {"ready": False, "worker": {"phase": "ready", "queue_depth": 0}, "model": "coupled-astrid"}
        error = urllib.error.HTTPError("http://fixture/readyz", 503, "not ready", {}, io.BytesIO(json.dumps(payload).encode()))
        with patch.object(activate.urllib.request, "urlopen", side_effect=error):
            self.assertEqual(self.backend.health(), payload)
        self.assertFalse(activate.healthy_readiness(payload))

    def test_explicit_unhealthy_rollback_keeps_failed_preinventory_without_blocking_recovery(self):
        self.backend.resume_transaction = None
        with patch.object(activate, "run", return_value=types.SimpleNamespace(returncode=1, stdout="failed", stderr="unready")) as run:
            self.backend.pre_stack_receipt()
        self.assertFalse(run.call_args.kwargs["check"])
        inventory = environments.read_json(self.backend.transaction / "recovery-pre-stack.json")
        self.assertEqual(inventory["returncode"], 1)
        self.assertTrue(inventory["readiness_waived_for_explicit_rollback_only"])

    def prepare_full_recovery(self):
        self.backend.baseline.write_text("{}")
        self.backend.fixture_proof.write_text("{}")
        self.backend.deployment_manifest = self.root / "deployment.json"
        baseline_hash = environments.digest(self.backend.baseline)
        self.receipt.update(transaction_id="failed", actor="original-owner",
                            baseline_manifest=str(self.backend.baseline))
        self.receipt["old_identity"]["baseline_manifest_sha256"] = baseline_hash
        environments.atomic_json(self.failed / "receipt.json", self.receipt)
        environments.atomic_json(self.backend.root / "rollback-baseline.json", self.backend.baseline_anchor())
        checked = {"manifest_sha256": baseline_hash, "release_id": "original", "manifest": {
            "python": self.settings["ProgramArguments"][0], "working_directory": str(activate.MODEL_ROOT),
            "argv": ["--port", "8090"], "runtime_paths": {}}}
        return checked

    def run_full_recovery(self, *, fail_ready=False):
        checked = self.prepare_full_recovery()
        held = {"pid": 333, "started_at": "restored start", "hold": self.backend.guard, "hold_only": False}
        with contextlib.ExitStack() as mocks:
            mocks.enter_context(patch.object(activate, "verify_fixture"))
            mocks.enter_context(patch.object(self.backend, "preflight"))
            mocks.enter_context(patch.object(environments, "validate_manifest", return_value=checked))
            mocks.enter_context(patch.object(environments, "process_start", return_value=None))
            mocks.enter_context(patch.object(self.backend, "job_loaded", return_value=False))
            mocks.enter_context(patch.object(self.backend, "listeners", return_value=[]))
            mocks.enter_context(patch.object(self.backend, "loaded_arguments", return_value=self.backend.launcher_arguments()))
            mocks.enter_context(patch.object(self.backend, "wait_held", return_value=held))
            ready = mocks.enter_context(patch.object(self.backend, "verify_ready", return_value={
                **held, "manifest_sha256": checked["manifest_sha256"]}))
            if fail_ready:
                ready.side_effect = RuntimeError("injected restored provider failure")
            mocks.enter_context(patch.object(self.backend, "publish"))
            mocks.enter_context(patch.object(self.backend, "post_stack_receipt"))
            subprocess_run = mocks.enter_context(patch.object(activate, "run"))
            signal = mocks.enter_context(patch.object(self.backend, "signal"))
            try:
                return activate.transition(self.backend, expected_pid=0, actor="recovery-actor", ack="owned test",
                                           migrate=True, rollback=True, timeout=2)
            finally:
                signal.assert_not_called()
                subprocess_run.assert_called_once_with("launchctl", "bootstrap", self.backend.domain,
                                                       str(self.backend.installed_plist))

    def test_real_backend_recovers_absent_job_under_hold_without_signal(self):
        result = self.run_full_recovery()
        self.assertEqual(result["status"], "activated_verified")
        self.assertFalse(result["signal_sent"])
        self.assertIsNone(environments.hold_record(self.backend.root))
        self.assertEqual(environments.read_json(self.failed / "receipt.json")["status"], "failed_requires_review")
        self.assertEqual(environments.read_json(self.backend.root / "active.json")["manifest"], str(self.backend.baseline))

    def test_real_backend_resumed_readiness_failure_reinstates_owned_hold(self):
        with self.assertRaisesRegex(RuntimeError, "restored provider failure"):
            self.run_full_recovery(fail_ready=True)
        self.assertEqual(environments.hold_record(self.backend.root), self.backend.guard)
        failure = environments.read_json(self.backend.transaction / "receipt.json")
        self.assertEqual(failure["status"], "failed_requires_review")
        self.assertTrue(failure["hold_released"])
        self.assertNotIn("hold_recovery_error", failure)


if __name__ == "__main__":
    unittest.main()
