"""Synthetic processes, real launch admission gate; no live services touched."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import time
import unittest
from unittest.mock import patch

from paired_minime_handoff import BRIDGE, PairedHandoff, verified_protected
from restart_minime_agent import reload_agent
from test_restart_minime_agent import Backend
from reconcile_minime_launch import OVERLAY, inventory

MINIME = Path(__file__).resolve().parents[2] / "minime"


class HandoffTests(unittest.TestCase):
    def install_fixture(self, root):
        canonical, snapshot = root / "canonical", root / "snapshot"
        for tree in (canonical, snapshot):
            for name in OVERLAY:
                path = tree / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("# old\n" if tree == canonical else "# new\n")
            for name in ("scripts/minime_rescue_investigation.py", "launchd/com.minime.autonomous-agent.plist"):
                path = tree / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("# unchanged\n")
        (snapshot / "scripts/launchd_autonomous_agent.sh").write_bytes(
            (MINIME / "scripts/launchd_autonomous_agent.sh").read_bytes())
        (canonical / "workspace/runtime").mkdir(parents=True)
        backend = Backend()
        backend.root = canonical
        backend.inputs = lambda: inventory(canonical)
        packet = root / "reconciliation.json"
        packet.write_text(json.dumps({"schema": "minime_launch_source_reconciliation_v1",
            "selected_inputs": inventory(snapshot), "canonical_inputs": inventory(canonical),
            "overlay_paths": list(OVERLAY), "snapshot_root": str(snapshot),
            "bindings": {"canonical_root": str(canonical)}}))
        return backend, packet, snapshot

    def test_installer_holds_before_first_replace_and_preserves_source_backups(self):
        with tempfile.TemporaryDirectory() as tmp, patch("paired_minime_handoff.time.sleep"), patch(
                "paired_minime_handoff.bridge_stage.verify_stage", return_value={"manifest_sha256": "a"}):
            root = Path(tmp).resolve()
            backend, packet, snapshot = self.install_fixture(root)
            handoff = PairedHandoff(root, root / "receipt.jsonl", "synthetic")
            events = []
            handoff.install(backend, packet, 10, events.append)
            self.assertEqual(backend.inputs(), inventory(snapshot))
            self.assertEqual(backend.signals, [])
            self.assertTrue(handoff.owned)
            self.assertEqual(events[0]["phase"], "installation_held")
            for path in OVERLAY:
                self.assertEqual((root / "receipt.source-before" / path).read_text(), "# old\n")

    def test_installer_source_drift_fails_before_hold_or_write(self):
        with tempfile.TemporaryDirectory() as tmp, patch(
                "paired_minime_handoff.bridge_stage.verify_stage", return_value={"manifest_sha256": "a"}):
            root = Path(tmp).resolve()
            backend, packet, snapshot = self.install_fixture(root)
            (backend.root / OVERLAY[0]).write_text("foreign\n")
            before = backend.inputs()
            handoff = PairedHandoff(root, root / "receipt.jsonl", "synthetic")
            with self.assertRaisesRegex(RuntimeError, "inventory changed"):
                handoff.install(backend, packet, 10, lambda e: None)
            self.assertFalse(handoff.owned)
            self.assertEqual(backend.inputs(), before)

    def test_installer_partial_failure_retains_hold_without_undo(self):
        with tempfile.TemporaryDirectory() as tmp, patch(
                "paired_minime_handoff.bridge_stage.verify_stage", return_value={"manifest_sha256": "a"}):
            root = Path(tmp).resolve()
            backend, packet, snapshot = self.install_fixture(root)
            handoff = PairedHandoff(root, root / "receipt.jsonl", "synthetic")
            def emit(event):
                if event["phase"] == "source_installed":
                    (backend.root / OVERLAY[0]).write_text("foreign\n")
            with self.assertRaisesRegex(RuntimeError, "during installation"):
                handoff.install(backend, packet, 10, emit)
            self.assertTrue(handoff.owned)
            self.assertEqual((backend.root / OVERLAY[0]).read_text(), "foreign\n")
            self.assertEqual((backend.root / "scripts/launchd_autonomous_agent.sh").read_bytes(),
                             (snapshot / "scripts/launchd_autonomous_agent.sh").read_bytes())

    def test_handoff_runs_once_only_after_old_exit_before_readiness(self):
        backend = Backend()
        events = []

        class Handoff:
            def begin(self, b, inputs, config, protected, emit):
                self.assertions = (not b.signals, b.pid() == 10)
                events.append("held")

            def transition(self, b, protected, emit):
                if b.identity(10) is not None:
                    raise AssertionError("old writer still present")
                events.append("transition")
                b.services = {"engine": 100, "bridge": 102}
                return b.services.copy()

        handoff = Handoff()
        ready = backend.ready
        backend.ready = lambda *args: events.append("ready") or ready(*args)
        result = reload_agent(backend, 10, timeout_s=30, quiet_s=10,
            now=lambda: backend.clock, sleep=backend.sleep, handoff=handoff)
        self.assertEqual(events, ["held", "transition", "ready"])
        self.assertEqual(handoff.assertions, (True, True))
        self.assertEqual(result["protected"], backend.services)
        self.assertEqual(backend.signals, [10])

    def test_hold_failure_sends_no_signal(self):
        backend = Backend()
        handoff = unittest.mock.Mock()
        handoff.begin.side_effect = RuntimeError("foreign hold")
        with self.assertRaisesRegex(RuntimeError, "foreign hold"):
            reload_agent(backend, 10, timeout_s=30, quiet_s=10,
                now=lambda: backend.clock, sleep=backend.sleep, handoff=handoff)
        self.assertEqual(backend.signals, [])
        handoff.transition.assert_not_called()

    def test_failed_transition_never_releases_or_accepts_readiness(self):
        backend = Backend()
        handoff = unittest.mock.Mock()
        handoff.transition.side_effect = RuntimeError("bridge refused")
        backend.ready = unittest.mock.Mock()
        with self.assertRaisesRegex(RuntimeError, "bridge refused"):
            reload_agent(backend, 10, timeout_s=30, quiet_s=10,
                now=lambda: backend.clock, sleep=backend.sleep, handoff=handoff)
        self.assertEqual(backend.signals, [10])
        backend.ready.assert_not_called()

    def test_boundary_change_after_hold_aborts_before_signal(self):
        backend = Backend()
        handoff = unittest.mock.Mock()
        handoff.begin.side_effect = lambda *args: setattr(backend, "active_until", 99)
        with self.assertRaisesRegex(RuntimeError, "after launch hold"):
            reload_agent(backend, 10, timeout_s=30, quiet_s=10,
                now=lambda: backend.clock, sleep=backend.sleep, handoff=handoff)
        self.assertEqual(backend.signals, [])

    def test_old_pid_reuse_is_not_treated_as_exit(self):
        backend = Backend()
        original = backend.identity
        backend.identity = lambda pid: "foreign" if pid == 10 and backend.signals else original(pid)
        handoff = unittest.mock.Mock()
        with self.assertRaisesRegex(RuntimeError, "PID reused"):
            reload_agent(backend, 10, timeout_s=30, quiet_s=10,
                now=lambda: backend.clock, sleep=backend.sleep, handoff=handoff)
        handoff.transition.assert_not_called()

    def test_only_verified_bridge_identity_can_change(self):
        stage = Path("/synthetic/stage")
        before = {BRIDGE: {"pid": 10, "started_at": "old"}, "engine": 5}
        after = {BRIDGE: {"pid": 11, "started_at": "new"}, "engine": 5}
        receipt = {"status": "activated_verified", "force_used": False,
            "legacy_transition": False, "drain": {"phase": "drained"}, "old_pid": 10,
            "stage": str(stage), "new_process": {"pid": 11, "started_at": "new",
                "binary": str(stage / "spectral-bridge-server")}}
        self.assertEqual(verified_protected(before, after, receipt, stage), after)
        with self.assertRaisesRegex(RuntimeError, "protected process"):
            verified_protected(before, {**after, "engine": 6}, receipt, stage)
        for changes in ({"status": "failed_requires_review"}, {"force_used": True},
                        {"legacy_transition": True}, {"old_pid": 9}):
            with self.assertRaises(RuntimeError):
                verified_protected(before, after, {**receipt, **changes}, stage)

    def test_actual_launcher_blocks_for_regular_and_dangling_holds(self):
        launcher = (MINIME / "scripts/launchd_autonomous_agent.sh").read_text()
        prefix = launcher.split("launchctl_env()", 1)[0]
        for dangling in (False, True):
            with self.subTest(dangling=dangling), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                runtime = root / "workspace/runtime"
                runtime.mkdir(parents=True)
                hold = runtime / "agent-launch-hold.json"
                if dangling:
                    hold.symlink_to(root / "missing")
                else:
                    hold.write_text("{}")
                code = prefix.replace('PROJECT_DIR="/Users/v/other/minime"', f'PROJECT_DIR="{root}"')
                process = subprocess.Popen(["/bin/bash", "-c", code + "printf admitted"], stdout=subprocess.PIPE)
                try:
                    time.sleep(0.2)
                    self.assertIsNone(process.poll())
                    hold.unlink()
                    output, _ = process.communicate(timeout=3)
                    self.assertEqual(output, b"admitted")
                    self.assertEqual(process.returncode, 0)
                finally:
                    if process.poll() is None:
                        process.terminate()
                        process.communicate(timeout=3)

    def test_owned_hold_is_exclusive_and_tamper_detecting(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "workspace/runtime").mkdir(parents=True)
            (root / "scripts").mkdir()
            launcher = root / "scripts/launchd_autonomous_agent.sh"
            launcher.write_bytes((MINIME / "scripts/launchd_autonomous_agent.sh").read_bytes())
            inputs = {"scripts/launchd_autonomous_agent.sh": hashlib.sha256(launcher.read_bytes()).hexdigest()}
            backend = Backend()
            backend.root = root
            with patch("paired_minime_handoff.bridge_stage.verify_stage", return_value={"manifest_sha256": "a"}):
                handoff = PairedHandoff(root, root / "receipt", "synthetic")
                handoff.begin(backend, inputs, {}, {}, lambda e: None)
                self.assertTrue(handoff.owned)
                handoff.assert_hold()
                original = handoff.hold.read_bytes()
                other = PairedHandoff(root, root / "other", "synthetic")
                with self.assertRaises(FileExistsError):
                    other.begin(backend, inputs, {}, {}, lambda e: None)
                self.assertFalse(other.owned)
                self.assertEqual(handoff.hold.read_bytes(), original)
                handoff.hold.write_text(json.dumps({"foreign": True}))
                with self.assertRaisesRegex(RuntimeError, "ownership"):
                    handoff.assert_hold()


if __name__ == "__main__":
    unittest.main()
