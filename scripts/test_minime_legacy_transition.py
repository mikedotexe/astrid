"""Synthetic stopped-state installation. Never signals a real service."""
import hashlib
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import minime_legacy_transition as t
import minime_next_handoff as proof


class LegacyTransitionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name).resolve()
        self.root, self.directory = self.home / "root", self.home / "transaction"
        self.root.mkdir()
        self.directory.mkdir(mode=0o700)
        (self.root / "workspace/runtime").mkdir(parents=True)
        self.before = {}
        self.after = {}
        for name in t.PATHS:
            content = ("HANDOFF_VERSION = 1\n" if name.endswith("/maintenance_next.py") else
                       "LEGACY_HANDOFF_VERSION = 1\n" if name.endswith("/maintenance_next_legacy.py") else
                       "# synthetic new source\n")
            source = self.directory / "source" / name
            source.parent.mkdir(parents=True, exist_ok=True)
            source.write_text(content)
            self.after[name] = t.sha(source)
            if "maintenance_next" not in name:
                target = self.root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text("# synthetic old source\n")
                self.before[name] = t.sha(target)
        self.status = dict(pid=200, started_at="old", source_inputs_at_start=self.before,
                           lifecycle_contract="agent_drain_v1", lifecycle_phase="exited", reason="drained")
        t.immutable(self.root / "workspace/runtime/autonomous_agent_source_status.json", self.status)
        t.immutable(self.root / "workspace/sovereignty_state.json", {
            "pending_next_action": 'WRITE CONTINUE "private synthetic"',
            "pending_next_selection_id": "choice", "pending_next_action_updated_at": "2020-01-01"})
        self.plan = dict(schema=t.SCHEMA, owner="a" * 32, root=str(self.root), before=self.before,
            after=self.after, old_pid=200, old_status_started_at="old", protected={"engine": 300},
            config={"unchanged": True}, tools={}, stage=str(self.home), stage_ready={"manifest_sha256": "b" * 64})
        t.immutable(self.directory / "manifest.json", self.plan)
        self.backend = SimpleNamespace(root=self.root, maintenance=Mock(), identity=lambda pid: None,
            pid=lambda: None, protected=lambda: self.plan["protected"], config=lambda: self.plan["config"],
            jobs=lambda: {"active": [], "jobs": []}, inputs=lambda: {
                str(p.relative_to(self.root)): t.sha(p) for p in (self.root / "minime_autonomy").glob("*.py")})
        self.patches = [patch.object(t, "tools_identity", return_value={}),
            patch.object(t.bridge_stage, "verify_stage", return_value=self.plan["stage_ready"]),
            patch.object(t, "inventory", side_effect=lambda root: {
                str(p.relative_to(root)): t.sha(p) for p in (root / "minime_autonomy").glob("*.py")}),
            patch.object(t.subprocess, "run", return_value=SimpleNamespace(returncode=0, stdout="1 0\n300 1\n", stderr=""))]
        for p in self.patches:
            p.start()
            self.addCleanup(p.stop)
        self.handoff = t.LegacyHandoff(self.directory, "synthetic qualification")
        self.handoff.hold = self.root / "workspace/runtime/agent-launch-hold.json"
        self.handoff.owned = True
        t.immutable(self.handoff.hold, self.handoff.guard)

    def test_installs_exact_maintenance_sources_and_import_without_rewriting_queue(self):
        state = self.root / "workspace/sovereignty_state.json"
        before = state.read_bytes()
        events = []
        self.handoff.transition(self.backend, self.plan["protected"], events.append)
        self.assertEqual(self.backend.inputs(), self.after)
        self.assertEqual(state.read_bytes(), before)
        self.assertFalse(self.handoff.hold.exists())
        imported = proof.read_legacy(proof.legacy_paths(self.root / "workspace")[0])
        self.assertEqual(imported["state_sha256"], hashlib.sha256(before).hexdigest())
        self.assertFalse(imported["runtime_seal_available"])
        self.assertNotIn("private synthetic", json.dumps(imported))
        self.assertTrue((self.directory / "release-intent.json").exists())

    def test_exact_partial_install_recovers_without_restoring_old_source(self):
        original = t.atomic_bytes
        calls = []
        def crash(path, data, mode):
            calls.append(path)
            if len(calls) == 2:
                raise OSError("synthetic disk full")
            original(path, data, mode)
        with patch.object(t, "atomic_bytes", side_effect=crash), self.assertRaises(OSError):
            self.handoff.transition(self.backend, self.plan["protected"], lambda row: None)
        self.assertTrue(self.handoff.hold.exists())
        self.assertEqual(t.sha(self.root / t.PATHS[0]), self.after[t.PATHS[0]])
        self.handoff.transition(self.backend, self.plan["protected"], lambda row: None)
        self.assertFalse(self.handoff.hold.exists())

    def test_stale_state_after_partial_install_does_not_silently_import_new_queue(self):
        with patch.object(t, "atomic_bytes", side_effect=OSError("disk full")), self.assertRaises(OSError):
            self.handoff.transition(self.backend, self.plan["protected"], lambda row: None)
        state = self.root / "workspace/sovereignty_state.json"
        state.write_text('{"pending_next_action":"REST"}')
        with self.assertRaisesRegex(RuntimeError, "conflicting transition"):
            self.handoff.transition(self.backend, self.plan["protected"], lambda row: None)
        self.assertTrue(self.handoff.hold.exists())
        self.assertEqual(json.loads(state.read_text())["pending_next_action"], "REST")

    def test_foreign_edit_or_live_process_or_failed_ps_refuses_before_install(self):
        for mode in ("foreign", "alive", "ps_failed"):
            with self.subTest(mode=mode):
                target = self.root / t.PATHS[0]
                original = target.read_bytes()
                if mode == "foreign":
                    target.write_text("# foreign\n")
                identity = (lambda pid: "alive") if mode == "alive" else (lambda pid: None)
                result = SimpleNamespace(returncode=1 if mode == "ps_failed" else 0,
                    stdout="1 0\n", stderr="")
                with patch.object(self.backend, "identity", identity), patch.object(t.subprocess, "run", return_value=result):
                    with self.assertRaises(RuntimeError):
                        self.handoff.transition(self.backend, self.plan["protected"], lambda row: None)
                self.assertTrue(self.handoff.hold.exists())
                target.write_bytes(original)

    def test_restore_verifies_real_ack_shape_and_schema_floor(self):
        self.handoff.transition(self.backend, self.plan["protected"], lambda row: None)
        imported_path, ack_path = proof.legacy_paths(self.root / "workspace")
        imported = proof.read_legacy(imported_path)
        status = dict(pid=201, started_at="new", boot_id="c" * 32, source_inputs_at_start=self.after,
                      next_handoff_contract=proof.CONTRACT)
        ack = dict(schema="minime_next_legacy_restored_v1", version=1, kind="legacy_restored",
            import_sha256=t.sha(imported_path), process=proof.process(status),
            **{k: imported[k] for k in ("state_sha256", "pending_sha256", "pending_present")},
            runtime_seal_available=False, execution_confirmed=False, scheduling_authorized=False)
        t.immutable(ack_path, ack)
        status["next_handoff_restore"] = ack
        result = proof.verify_legacy_restore(self.root / "workspace", imported, status, self.after)
        self.assertFalse(result["execution_confirmed"])
        from research_release_compatibility import verify_minime
        verify_minime(self.root, self.root / "workspace")
        (self.root / "minime_autonomy/maintenance_next_legacy.py").unlink()
        with self.assertRaisesRegex(RuntimeError, "downgrade"):
            verify_minime(self.root, self.root / "workspace")
        status["next_handoff_restore"] = dict(ack, scheduling_authorized=True)
        with self.assertRaisesRegex(RuntimeError, "acknowledgement"):
            proof.verify_legacy_restore(self.root / "workspace", imported, status, self.after)

    def test_real_reload_switches_inventory_only_after_exit_and_requires_ack(self):
        from test_restart_minime_agent import Backend
        from restart_minime_agent import reload_agent
        for fail in (False, True):
            backend = Backend()
            new_inputs = {"runtime.py": "b"}
            def transition(b, protected, emit):
                self.assertIsNone(b.identity(10))
                b.source = new_inputs
                return protected
            def verify(b, pid):
                self.assertEqual(pid, 11)
                self.assertEqual(b.inputs(), new_inputs)
                if fail:
                    raise RuntimeError("missing real acknowledgement")
                return {"execution_confirmed": False}
            handoff = SimpleNamespace(begin=lambda *args: None, transition=transition,
                replacement_inputs=new_inputs, verify_restoration=verify)
            def run():
                return reload_agent(backend, 10, timeout_s=30, quiet_s=10,
                    now=lambda: backend.clock, sleep=backend.sleep, handoff=handoff)
            if fail:
                with self.assertRaisesRegex(RuntimeError, "missing real"):
                    run()
            else:
                result = run()
                self.assertEqual(result["source_inputs"], new_inputs)
                self.assertFalse(result["legacy_next_handoff"]["execution_confirmed"])
            self.assertEqual(backend.signals, [10])


if __name__ == "__main__":
    unittest.main()
