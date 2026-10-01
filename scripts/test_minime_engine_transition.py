import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from unittest.mock import Mock
from types import SimpleNamespace

from minime_engine_transition_host import Host

from minime_engine_transition import transition
from minime_engine_transition_io import (
    Holds, LABELS, atomic, health_point, hold_block, read_json, stop_exact, validate_launcher,
)


class FakeHost:
    names = ("preflight", "prepare", "hold", "stop_old", "checkpoint", "install", "handoff",
             "start_engine", "start_companions", "observe", "publish", "rollback")

    def __init__(self, fail=None):
        self.calls, self.records, self.fail = [], [], fail

    def record(self, value):
        self.records.append(copy.deepcopy(value))

    def __getattr__(self, name):
        if name not in self.names:
            raise AttributeError(name)
        def operation(*_):
            self.calls.append(name)
            if name == self.fail:
                raise ValueError("fixture boundary " + name)
            return {"ok": True}
        return operation


class TransitionTests(unittest.TestCase):
    def test_order_and_durable_intent_before_every_boundary(self):
        host = FakeHost()
        result = transition(host)
        self.assertEqual(host.calls, list(FakeHost.names[:-1]))
        self.assertEqual(result["status"], "activated_verified")
        self.assertFalse(result["complete_input_drain_claimed"])
        for phase in ("launch_holds", "stopped_checkpoint", "candidate_installed", "signed_handoff", "engine_ready"):
            records = [r for r in host.records if r["phase"] == phase]
            self.assertNotIn(phase, records[0])
            self.assertIn(phase, records[1])

    def test_no_automatic_restart_without_verified_stopped_checkpoint(self):
        for failure in FakeHost.names[:5]:
            with self.subTest(failure=failure):
                host = FakeHost(failure)
                with self.assertRaises(ValueError):
                    transition(host)
                self.assertNotIn("rollback", host.calls)
                self.assertNotIn("install", host.calls)

    def test_later_failures_request_owned_rollback_not_state_restore(self):
        for failure in FakeHost.names[5:-1]:
            with self.subTest(failure=failure):
                host = FakeHost(failure)
                with self.assertRaises(ValueError):
                    transition(host)
                self.assertEqual(host.calls[-1], "rollback")
                self.assertEqual(host.records[-1]["status"], "rolled_back_requires_review")

    def test_failed_rollback_is_explicit(self):
        host = FakeHost("observe")
        with patch.object(host, "rollback", side_effect=ValueError("foreign PID")):
            with self.assertRaises(ValueError):
                transition(host)
        self.assertEqual(host.records[-1]["status"], "failed_requires_review")
        self.assertEqual(host.records[-1]["rollback_error"], "foreign PID")

    def test_only_exact_process_is_signalled_and_timeout_does_not_escalate(self):
        row = {"pid": 999999, "started_at": "original"}
        with patch("minime_engine_transition_io.started", return_value="different"), patch("os.kill") as kill:
            with self.assertRaises(ValueError):
                stop_exact(row, 0)
            kill.assert_not_called()
        with patch("minime_engine_transition_io.started", return_value="original"), patch("os.kill") as kill:
            with self.assertRaises(ValueError):
                stop_exact(row, 0)
            self.assertEqual(kill.call_count, 1)
            self.assertEqual(int(kill.call_args.args[1]), 15)

    def test_holds_survive_foreign_retries_and_corruption(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve() / "holds"
            first, second = Holds(root, "one"), Holds(root, "two")
            first.create(LABELS[0])
            with self.assertRaises(FileExistsError):
                first.create(LABELS[0])
            with self.assertRaises(ValueError):
                second.release(LABELS[0])
            first.verify(LABELS[0])
            path = root / (LABELS[0] + ".json")
            path.write_text("broken")
            with self.assertRaises(ValueError):
                first.release(LABELS[0])
            self.assertEqual(path.read_text(), "broken")

    def test_holds_release_only_owned_record_and_refuse_symlinks(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve() / "holds"
            holds = Holds(root, "one")
            holds.create(LABELS[0])
            holds.release(LABELS[0])
            victim = Path(temporary).resolve() / "victim"
            victim.write_text("keep")
            (root / (LABELS[0] + ".json")).symlink_to(victim)
            with self.assertRaises(ValueError):
                holds.release(LABELS[0])
            self.assertEqual(victim.read_text(), "keep")

    def test_only_reviewed_launcher_addition_allowed(self):
        for label in LABELS:
            anchor = 'cd "$PROJECT_DIR"\n\n' if label == LABELS[0] else 'MANIFEST="$ROOT/workspace/division/runtime-manifest.json"\n\n'
            old = anchor + 'exec "$ENGINE"\n'
            new = anchor + hold_block(label) + 'exec "$ENGINE"\n'
            validate_launcher(old.encode(), new.encode(), label)
            validate_launcher(new.encode(), new.encode(), label)
            for bad in (new.replace('exec', 'rm'), new + hold_block(label), hold_block(label) + old):
                with self.assertRaises(ValueError):
                    validate_launcher(old.encode(), bad.encode(), label)

    def test_atomic_replacement_preserves_destination_before_rename_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary).resolve() / "state"
            path.write_bytes(b"old")
            with patch("os.replace", side_effect=OSError("fixture")):
                with self.assertRaises(OSError):
                    atomic(path, b"new")
            self.assertEqual(path.read_bytes(), b"old")
            self.assertEqual(list(path.parent.iterdir()), [path])


class HealthTests(unittest.TestCase):
    def fixture(self):
        return {"provenance": {"wall_clock_unix_ms": 100000, "session_id": 7, "snapshot_sequence": 2},
                "fill_pct": 68, "t_s": 10, "startup_restore": {"state": "restored", "resume_mode": "pi_only_resume"},
                "stable_core": {"enabled": True, "checkpoint_lineage_enabled": False, "neural_bundle_enabled": False,
                                "stage": "hold", "restart_gate": {"phase": "settled"},
                                "structural_pi": {"target_fill_pct": 68}},
                "fill_rate_v1": {}, "measurement_basis_v1": {}}

    def test_fresh_valid_repaired_health(self):
        point = health_point(self.fixture(), now=101, earliest_ms=90000, candidate=True)
        self.assertEqual(point["target_fill_pct"], 68)

    def test_stale_future_warning_nonfinite_and_wrong_restore_refused(self):
        cases = [({"fill_pct": 80}, 101), ({"fill_pct": float("nan")}, 101), ({}, 116), ({}, 99),
                 ({"startup_restore": {"state": "partial", "resume_mode": "pi_only_resume"}}, 101)]
        for changes, now in cases:
            value = self.fixture()
            value.update(changes)
            with self.assertRaises(ValueError):
                health_point(value, now=now, earliest_ms=90000, candidate=True)

    def test_missing_candidate_diagnostics_not_silently_certified(self):
        value = self.fixture()
        del value["measurement_basis_v1"]
        with self.assertRaises(ValueError):
            health_point(value, now=101, earliest_ms=90000, candidate=True)
        health_point(value, now=101, earliest_ms=90000, candidate=False)


class HostBoundaryTests(unittest.TestCase):
    def host(self, root):
        h = object.__new__(Host)
        h.engine = root / "engine"
        h.engine.write_bytes(b"candidate")
        h.transaction = root
        h.control_root = root / "control"
        h.args = SimpleNamespace(actor="fixture", ack="approved fixture only")
        h.env = {}
        h.candidate = True
        h.expected_sha = "a" * 64
        h.stopped_control = {"state_sha256": "stopped"}
        h.stopped = Mock()
        h.control = Mock(return_value={"state_sha256": "stopped"})
        return h

    def test_handoff_refuses_missing_state_wrong_path_hash_and_moved_state(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            h = self.host(root)
            h.control_root.mkdir()
            (h.control_root / "deployment_handoff.pending.json").write_text("fixture pending")
            good = {"status": "prepared", "handoff": {"target_binary_sha256": h.expected_sha,
                "target_executable_path": str(h.engine), "from_state_sha256": "stopped",
                "handoff_id": "h1", "from_deployment_identity": "old", "to_deployment_identity": "new"}}
            for bad in ({"status": "not_needed"}, {"status": "already_current"},
                        {**good, "handoff": {**good["handoff"], "target_binary_sha256": "wrong"}},
                        {**good, "handoff": {**good["handoff"], "target_executable_path": "/other"}},
                        {**good, "handoff": {**good["handoff"], "from_state_sha256": "moved"}}):
                with patch("minime_engine_transition_host.command", return_value=json.dumps(bad)):
                    with self.assertRaises(ValueError):
                        h.handoff()
            with patch("minime_engine_transition_host.command", return_value=json.dumps(good)) as command:
                result = h.handoff()
                self.assertEqual(result["handoff_id"], "h1")
                self.assertIn("prepare-deployment-handoff", command.call_args.args[0])
                self.assertEqual(read_json(root / "handoff.json"), good)
            h.control.return_value = {"state_sha256": "moved"}
            with patch("minime_engine_transition_host.command") as command:
                with self.assertRaises(ValueError):
                    h.handoff()
                command.assert_not_called()

    def test_control_validation_keeps_pending_and_identity_failures_separate(self):
        with tempfile.TemporaryDirectory() as temporary:
            h = self.host(Path(temporary).resolve())
            del h.control
            good = {"integrity_verified": True, "pending_transition": False,
                    "pending_deployment_handoff": None, "state_targets_this_binary": True,
                    "state_sha256": "s", "revision_by_family": {"r": 2}, "deployment_identity": "new",
                    "cli_deployment_identity": "new", "binary_sha256": "a" * 64}
            for bad in ({"integrity_verified": False}, {"pending_transition": True},
                        {"pending_deployment_handoff": {"id": "pending"}}, {"state_targets_this_binary": False}):
                with patch("minime_engine_transition_host.command", return_value=json.dumps({**good, **bad})):
                    with self.assertRaises(ValueError):
                        h.control()
            with patch("minime_engine_transition_host.command", return_value=json.dumps(good)):
                self.assertEqual(h.control()["revision_by_family"], {"r": 2})

    def test_rollback_stops_only_released_owned_processes_and_uses_latest_state(self):
        with tempfile.TemporaryDirectory() as temporary:
            h = self.host(Path(temporary).resolve())
            h.guards = Mock()
            h.old_sha = "old-sha"
            h.stage_manifest = {"artifacts": {"minime": {"sha256": "new-sha"}}}
            h.holds = Mock()
            h.released = set(LABELS)
            h.current = {label: {"pid": n, "started_at": "fixture", "label": label} for n, label in enumerate(LABELS, 10)}
            h.log = h.transaction / "log"
            h.log.write_text("fixture")
            (h.transaction / "old-minime").write_bytes(b"old binary only")
            h.wait_held = Mock()
            h.checkpoint = Mock()
            h.control.return_value = {"state_sha256": "newest-authored-state", "state_targets_this_binary": False}
            h.handoff = Mock()
            h.start_engine = Mock(return_value={"ok": True})
            h.start_companions = Mock(return_value={"ok": True})
            h.observe = Mock(return_value={"ok": True})
            h.publish = Mock(return_value={"ok": True})
            h.rollback_pending_retained = False
            with patch("minime_engine_transition_host.digest", side_effect=["new-sha", "old-sha"]), \
                 patch("minime_engine_transition_host.binding_tool.process", side_effect=lambda label: h.current[label]), \
                 patch("minime_engine_transition_host.stop_exact") as stop:
                result = h.rollback(120)
            self.assertEqual([c.args[0]["label"] for c in stop.call_args_list], list(reversed(LABELS)))
            self.assertEqual(h.stopped_control["state_sha256"], "newest-authored-state")
            self.assertEqual(h.engine.read_bytes(), b"old binary only")
            h.handoff.assert_called_once()
            self.assertFalse(result["prestart_pending_evidence_retained_for_review"])

    def test_unknown_rollback_binary_never_signals_or_installs(self):
        with tempfile.TemporaryDirectory() as temporary:
            h = self.host(Path(temporary).resolve())
            h.guards = Mock()
            h.old_sha = "old"
            h.stage_manifest = {"artifacts": {"minime": {"sha256": "new"}}}
            with patch("minime_engine_transition_host.digest", return_value="foreign"), \
                 patch("minime_engine_transition_host.stop_exact") as stop:
                with self.assertRaises(ValueError):
                    h.rollback(120)
                stop.assert_not_called()
            self.assertEqual(h.engine.read_bytes(), b"candidate")


if __name__ == "__main__":
    unittest.main()
