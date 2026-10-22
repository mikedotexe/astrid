"""Synthetic processes, real launch admission gate; no live services touched."""
import hashlib
from contextlib import ExitStack
import copy
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest
from unittest.mock import patch

import paired_minime_handoff as paired
from paired_minime_handoff import (
    BRIDGE, AgentOnlyHandoff, InstalledHandoff, PairedHandoff, evidence_rows, verified_protected,
)
from restart_minime_agent import reload_agent
from test_restart_minime_agent import Backend
from reconcile_minime_launch import OVERLAY, inventory

MINIME = Path(os.environ.get("MINIME_SOURCE_ROOT", Path(__file__).resolve().parents[2] / "minime"))


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
        with tempfile.TemporaryDirectory() as tmp, patch("time.sleep") as sleep, patch(
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
            self.assertEqual(events[-1]["phase"], "installed_sources_verified")
            sleep.assert_not_called()
            self.assertEqual(backend.reviewed_installation.validate(backend), inventory(snapshot))
            for path in OVERLAY:
                self.assertEqual((root / "receipt.source-before" / path).read_text(), "# old\n")

    def test_new_reviewed_modules_install_under_hold_without_invented_backups(self):
        with tempfile.TemporaryDirectory() as tmp, patch("time.sleep"), patch(
                "paired_minime_handoff.bridge_stage.verify_stage", return_value={"manifest_sha256": "a"}):
            root = Path(tmp).resolve()
            backend, packet, snapshot = self.install_fixture(root)
            added = ("minime_autonomy/moment_context.py", "minime_autonomy/study_feedback.py",
                     "minime_autonomy/journal_recall.py", "minime_autonomy/source_status.py",
                     "minime_autonomy/mailbox.py", "minime_autonomy/mailbox_fs.py",
                     "minime_autonomy/human_letters.py")
            for name in added:
                (backend.root / name).unlink()
            value = json.loads(packet.read_text())
            value["canonical_inputs"] = backend.inputs()
            packet.write_text(json.dumps(value))
            handoff = PairedHandoff(root, root / "receipt.jsonl", "synthetic")
            events = []
            handoff.install(backend, packet, 10, events.append)
            self.assertEqual(backend.inputs(), inventory(snapshot))
            self.assertEqual(backend.signals, [])
            handoff.assert_hold()
            for name in added:
                self.assertFalse((root / "receipt.source-before" / name).exists())
                intent = next(e for e in events if e["phase"] == "source_install_intent" and e["path"] == name)
                self.assertIsNone(intent["before"])
                self.assertEqual(intent["after"], value["selected_inputs"][name])

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

    def test_installer_refuses_research_downgrade_before_hold_or_source_write(self):
        with tempfile.TemporaryDirectory() as tmp, patch(
                "paired_minime_handoff.bridge_stage.verify_stage", return_value={"manifest_sha256": "a"}):
            root = Path(tmp).resolve()
            backend, packet, _ = self.install_fixture(root)
            requirement = backend.root / "workspace/action_threads/research-transactions-required.json"
            requirement.parent.mkdir(parents=True)
            requirement.write_text(json.dumps(dict(schema_version=1, owner="minime",
                required_research_transaction_version=1)))
            before = backend.inputs()
            handoff = PairedHandoff(root, root / "receipt.jsonl", "synthetic")
            with self.assertRaisesRegex(RuntimeError, "downgrade refused"):
                handoff.install(backend, packet, 10, lambda e: None)
            self.assertFalse(handoff.owned)
            self.assertEqual(backend.signals, [])
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

    def test_post_install_drift_blocks_reload_without_sleeping_or_signaling(self):
        for mutation in ("source", "mode", "hold", "new_input"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as tmp, patch(
                    "paired_minime_handoff.bridge_stage.verify_stage",
                    return_value={"manifest_sha256": "a"}), patch("time.sleep") as sleep:
                root = Path(tmp).resolve()
                backend, packet, snapshot = self.install_fixture(root)
                handoff = AgentOnlyHandoff(root, root / "receipt.jsonl", "synthetic")
                handoff.install(backend, packet, 10, lambda event: None)
                source = backend.root / OVERLAY[0]
                if mutation == "source":
                    source.write_text("# drift\n")
                elif mutation == "mode":
                    source.chmod(0o700)
                elif mutation == "hold":
                    handoff.hold.write_text('{"foreign":true}')
                else:
                    (backend.root / "minime_autonomy/unreviewed.py").write_text("# drift\n")
                with self.assertRaises(RuntimeError):
                    backend.reload(expected_inputs=inventory(snapshot), handoff=handoff)
                self.assertEqual(backend.signals, [])
                self.assertTrue(handoff.hold.exists())
                sleep.assert_not_called()

    def test_agent_only_installs_and_reloads_without_bridge_activation(self):
        with tempfile.TemporaryDirectory() as tmp, patch("time.sleep"), patch(
                "paired_minime_handoff.bridge_stage.verify_stage", return_value={"manifest_sha256": "a"}), patch(
                "paired_minime_handoff.subprocess.run", side_effect=AssertionError("peer activation")):
            root = Path(tmp).resolve()
            backend, packet, snapshot = self.install_fixture(root)
            protected = backend.protected()
            handoff = AgentOnlyHandoff(root, root / "receipt.jsonl", "synthetic")
            events = []
            handoff.install(backend, packet, 10, events.append)
            self.assertTrue(handoff.owned)
            result = reload_agent(backend, 10, timeout_s=30, quiet_s=10,
                now=lambda: backend.clock, sleep=backend.sleep, handoff=handoff,
                expected_inputs=inventory(snapshot), emit=events.append)
            self.assertEqual(backend.signals, [10])
            self.assertEqual(result["protected"], protected)
            self.assertEqual(result["new_pid"], 11)
            self.assertFalse(handoff.owned)
            self.assertFalse(handoff.hold.exists())
            self.assertIn("agent_only_boundary_verified", [event["phase"] for event in events])

    def test_agent_only_refuses_drift_and_retains_owned_hold(self):
        for drift in ("source", "config", "protected", "jobs", "stage", "hold"):
            with self.subTest(drift=drift), tempfile.TemporaryDirectory() as tmp, patch(
                    "paired_minime_handoff.bridge_stage.verify_stage", return_value={"manifest_sha256": "a"}) as stage, patch(
                    "paired_minime_handoff.subprocess.run", side_effect=AssertionError("peer activation")):
                root = Path(tmp).resolve()
                backend, _, _ = self.install_fixture(root)
                # begin pins the canonical launcher, independently of installation.
                launcher = backend.root / "scripts/launchd_autonomous_agent.sh"
                launcher.write_bytes((MINIME / "scripts/launchd_autonomous_agent.sh").read_bytes())
                handoff = AgentOnlyHandoff(root, root / "receipt.jsonl", "synthetic")
                protected = backend.protected()
                handoff.begin(backend, backend.inputs(), backend.config(), protected, lambda e: None)
                if drift == "source":
                    (backend.root / "minime_autonomy/runtime.py").write_text("# drift\n")
                elif drift == "config":
                    backend.settings["changed"] = True
                elif drift == "protected":
                    backend.services["changed"] = True
                elif drift == "jobs":
                    backend.jobs = lambda: {"active": [{"status": "running"}]}
                elif drift == "stage":
                    stage.return_value = {"manifest_sha256": "changed"}
                else:
                    handoff.hold.write_text('{"foreign":true}')
                with self.assertRaises(RuntimeError):
                    handoff.transition(backend, protected, lambda e: None)
                self.assertTrue(handoff.owned)
                self.assertTrue(handoff.hold.exists())
                self.assertEqual(backend.signals, [])

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


class InstalledRecoveryTests(unittest.TestCase):
    def setUp(self):
        temporary = self.enterContext(tempfile.TemporaryDirectory())
        self.root = Path(temporary).resolve()
        self.old_stage, self.stage = self.root / "stage03", self.root / "stage04"
        self.old_stage.mkdir()
        self.stage.mkdir()
        self.old_ready = {"schema": "bridge_staged_release_v4", "status": "staged_verified_not_activated",
            "stage": str(self.old_stage), "manifest_sha256": "a" * 64, "binary_sha256": "e" * 64,
            "source_root": str(self.root / "astrid"), "native_target": "synthetic-target",
            "source_inputs_sha256": "f" * 64, "prepared_at": "2026-10-22T00:00:00+00:00",
            "actor": "synthetic", "acknowledgement_sha256": "0" * 64,
            "authority": "build_manifest_witness_not_deploy_authority", "live_authority_granted": False,
            "live_eligible_now": False, "activation_performed": False}
        self.ready = {**self.old_ready, "manifest_sha256": "b" * 64, "stage": str(self.stage)}
        # The live old release is distinct from the original unactivated stage03.
        self.live_ready = {**self.old_ready, "stage": str(self.root / "live-stage01"),
                           "manifest_sha256": "1" * 64, "binary_sha256": "c" * 64}
        self.enterContext(patch("paired_minime_handoff.bridge_stage.verify_stage",
            side_effect=lambda path: self.ready.copy() if path == self.stage else self.old_ready.copy()))
        self.enterContext(patch("time.sleep"))
        self.enterContext(patch("paired_minime_handoff.subprocess.run",
                               side_effect=AssertionError("live subprocess forbidden")))
        self.bridge_check = self.enterContext(patch("paired_minime_handoff.LaunchdBridge.inspect",
            return_value={"started_at": "bridge-start", "drain_supported": True,
                          "manifest_sha256": "1" * 64, "binary_sha256": "c" * 64}))
        self.backend, self.packet, self.snapshot = HandoffTests().install_fixture(self.root)
        self.backend.pause_generation = 533
        self.backend.settings["installed_plist_sha256"] = "d" * 64
        self.backend.services = {BRIDGE: {"pid": 20, "started_at": "bridge-start"}, "engine": 100}
        # Exercise new overlay modules as well as preserved canonical inputs.
        (self.backend.root / "minime_autonomy/public_web.py").unlink()
        packet = json.loads(self.packet.read_text())
        packet["canonical_inputs"] = inventory(self.backend.root)
        packet["bindings"].update(observed_pid=10, installed_plist_sha256="d" * 64)
        self.packet.write_text(json.dumps(packet))
        self.original = self.root / "original.jsonl"
        original_handoff = PairedHandoff(self.old_stage, self.original, "original approval")
        self.original_handoff = original_handoff
        rows = []
        original_handoff.install(self.backend, self.packet, 10, rows.append)
        self.original.write_text("".join(json.dumps(row) + "\n" for row in rows))
        self.hold = original_handoff.hold
        self.original_guard = original_handoff.guard.copy()
        self.original_bytes = self.original.read_bytes()
        self.original_hold_bytes = self.hold.read_bytes()
        self.status = self.backend.root / "workspace/runtime/autonomous_agent_source_status.json"
        self.status.write_text(json.dumps({"pid": 10, "source_inputs_at_start": packet["canonical_inputs"],
            "source_path": str(self.backend.root / "minime_autonomy/runtime.py")}))
        self.qualification = self.root / "qualification.json"
        self.qualified = {"schema": "retained_accounts_paired_qualification_v1", "status": "passed",
            "stages": {"old": self.live_ready, "new": self.ready}, "qualifier_sha256": "2" * 64,
            "reconciliation_sha256": hashlib.sha256(self.packet.read_bytes()).hexdigest(),
            "selected_inputs": packet["selected_inputs"], "assets": {}, "owners": [],
            "frozen_tests": {}, "preservation_sha256": "3" * 64, "checks": 1,
            "activation_performed": False, "live_eligible_now": False,
            "capacity_qualification_included": False, "navigation_candidate_included": False,
            "scope": "Synthetic old/new state and stubbed providers; no live authored state or services"}
        self.qualification.write_text(json.dumps(self.qualified))
        self.receipt = self.root / "recovery.jsonl"
        self.before_sources = inventory(self.backend.root)
        self.backup = self.original.with_suffix(".source-before")
        self.before_backup = {str(p.relative_to(self.backup)): p.read_bytes()
                              for p in self.backup.rglob("*") if p.is_file()}

    def open(self, stack, *, start="start-10", stage_ack="approve stage04", receipt=None):
        handoff = InstalledHandoff(self.stage, receipt or self.receipt, "resume approved")
        handle = handoff.open_resume(stack, self.backend, self.packet, self.original,
                                    self.qualification, 10, start, stage_ack)
        self.assertIs(handle, handoff.resume_handle)
        return handoff, handoff.write_event

    def assert_preserved(self):
        self.assertEqual(self.original.read_bytes(), self.original_bytes)
        self.assertEqual(inventory(self.backend.root), self.before_sources)
        self.assertEqual({str(p.relative_to(self.backup)): p.read_bytes()
                          for p in self.backup.rglob("*") if p.is_file()}, self.before_backup)

    def test_adoption_preserves_source_backup_original_receipt_and_guard_id(self):
        with ExitStack() as stack, patch.object(PairedHandoff, "install", side_effect=AssertionError("reinstall")):
            handoff, emit = self.open(stack)
            self.assertEqual(self.hold.read_bytes(), self.original_hold_bytes)
            handoff.adopt(emit)
            successor = json.loads(self.hold.read_text())
            self.assertEqual(successor["id"], self.original_guard["id"])
            self.assertEqual(successor["receipt"], str(self.original))
            self.assertEqual(successor["stage"], str(self.stage))
            self.assertEqual(successor["resume"]["receipt"], str(self.receipt))
            self.assertEqual(self.hold.stat().st_mode & 0o777, 0o600)
            self.assertEqual(self.receipt.stat().st_mode & 0o777, 0o600)
            self.assertTrue(handoff.owned)
            self.assertEqual(self.backend.signals, [])
            with self.assertRaisesRegex(RuntimeError, "ownership changed"):
                self.original_handoff.assert_hold()
        self.assert_preserved()

    def test_legacy_completed_install_receipt_remains_recoverable(self):
        rows = evidence_rows(self.original.read_bytes())
        rows[-1] = {"phase": "installed_waiting_for_unchanged_quiet_window", "seconds": 185}
        self.original.write_text("".join(json.dumps(row) + "\n" for row in rows))
        self.original_bytes = self.original.read_bytes()
        with ExitStack() as stack:
            handoff, emit = self.open(stack)
            handoff.adopt(emit)
            self.backend.validate()
            self.assertEqual(self.backend.signals, [])
        self.assert_preserved()

    def test_tampered_completed_install_identity_is_not_recoverable(self):
        rows = evidence_rows(self.original.read_bytes())
        rows[-1]["selected_inputs_sha256"] = "0" * 64
        self.original.write_text("".join(json.dumps(row) + "\n" for row in rows))
        with ExitStack() as stack, self.assertRaisesRegex(RuntimeError, "not completely receipted"):
            self.open(stack)
        self.assertFalse(self.receipt.exists())
        self.assertEqual(self.hold.read_bytes(), self.original_hold_bytes)
        self.assertEqual(self.backend.signals, [])

    def test_normal_reload_and_transition_are_reused_after_adoption(self):
        with ExitStack() as stack:
            handoff, emit = self.open(stack)
            handoff.adopt(emit)
            def transition(owned, backend, protected, output):
                self.assertIs(owned, handoff)
                self.assertEqual(backend.signals, [10])
                self.assertIsNone(backend.identity(10))
                owned.assert_hold()
                self.assertTrue(self.hold.exists())
                self.hold.unlink()
                owned.owned = False
                return protected
            with patch.object(PairedHandoff, "transition", autospec=True, side_effect=transition) as peer:
                result = self.backend.reload(expected_inputs=self.before_sources, handoff=handoff, emit=emit)
            self.assertEqual(result["outcome"], "success")
            self.assertEqual(self.backend.signals, [10])
            peer.assert_called_once()
            self.assertEqual(self.bridge_check.call_count, 2)
        self.assert_preserved()

    def test_exact_retry_spans_intent_and_atomic_adoption_without_rewriting_successor(self):
        with ExitStack() as stack:
            handoff, emit = self.open(stack)
            emit(handoff.intent)  # Simulate interruption before the guard replacement.
        with ExitStack() as stack:
            handoff, emit = self.open(stack)
            handoff.adopt(emit)
        original_successor = self.hold.read_bytes()
        with ExitStack() as stack, patch("paired_minime_handoff.atomic_bytes", side_effect=AssertionError("rewrite")):
            handoff, emit = self.open(stack)
            handoff.adopt(emit)
            self.assertEqual(self.hold.read_bytes(), original_successor)
            self.assertEqual(self.backend.signals, [])
        self.assert_preserved()

    def test_retry_after_pre_signal_refusal_retains_hold(self):
        with ExitStack() as stack:
            handoff, emit = self.open(stack)
            handoff.adopt(emit)
            self.backend.active_until = 99
            with self.assertRaisesRegex(RuntimeError, "no bounded idle"):
                self.backend.reload(expected_inputs=self.before_sources, handoff=handoff, emit=emit)
            emit({"outcome": "failed", "owned_hold_retained": True, "forced_termination": False})
        with ExitStack() as stack:
            handoff, emit = self.open(stack)
            handoff.adopt(emit)
        self.assertTrue(self.hold.exists())
        self.assertEqual(self.backend.signals, [])

    def test_retry_refuses_any_signal_intent_or_later_progress(self):
        with ExitStack() as stack:
            handoff, emit = self.open(stack)
            handoff.adopt(emit)
        prefix = self.receipt.read_bytes()
        for event in ({"phase": "signal_boundary"}, {"phase": "signal_sent"}, {"outcome": "success"},
                      {"phase": "replacement_admission_released"}, {"phase": "unknown"}):
            with self.subTest(event=event):
                self.receipt.write_bytes(prefix + json.dumps(event).encode() + b"\n")
                before = self.receipt.read_bytes()
                with ExitStack() as stack, self.assertRaisesRegex(RuntimeError, "beyond pre-signal"):
                    self.open(stack)
                self.assertEqual(self.receipt.read_bytes(), before)
                self.assertEqual(self.backend.signals, [])

    def test_foreign_guard_and_different_recovery_receipt_are_never_adopted(self):
        self.hold.write_text(json.dumps({**self.original_guard, "id": "f" * 32}))
        foreign = self.hold.read_bytes()
        with ExitStack() as stack, self.assertRaisesRegex(RuntimeError, "foreign"):
            self.open(stack)
        self.assertEqual(self.hold.read_bytes(), foreign)
        self.assertFalse(self.receipt.exists())
        self.hold.write_bytes(self.original_hold_bytes)
        with ExitStack() as stack:
            handoff, emit = self.open(stack)
            handoff.adopt(emit)
        with ExitStack() as stack, self.assertRaisesRegex(RuntimeError, "exact recovery receipt"):
            self.open(stack, receipt=self.root / "other.jsonl")

    def test_process_start_and_native_incompatibility_fail_before_adoption(self):
        with ExitStack() as stack, self.assertRaisesRegex(RuntimeError, "PID/start"):
            self.open(stack, start="reused PID")
        self.bridge_check.side_effect = RuntimeError("legacy queued study incompatible")
        with ExitStack() as stack, self.assertRaisesRegex(RuntimeError, "legacy queued study"):
            self.open(stack)
        self.assertFalse(self.receipt.exists())
        self.assertEqual(self.hold.read_bytes(), self.original_hold_bytes)
        self.assertEqual(self.backend.signals, [])

    def test_qualification_requires_actual_schema_status_and_exact_bindings(self):
        changes = [(("schema",), "geometry_paired_qualification_v1"),
                   (("status",), "offline_checks_passed_not_activatable"),
                   (("status",), "failed"), (("activation_performed",), True),
                   (("activation_performed",), 0), (("live_eligible_now",), True),
                   (("stages",), None), (("stages", "old"), None),
                   (("stages", "new", "stage"), str(self.old_stage)),
                   (("stages", "new", "manifest_sha256"), "0" * 64),
                   (("stages", "new", "binary_sha256"), "0" * 64),
                   (("selected_inputs", "minime_autonomy/runtime.py"), "0" * 64),
                   (("selected_inputs", "unexpected.py"), "0" * 64),
                   (("reconciliation_sha256",), "0" * 64)]
        for keys, value in changes:
            with self.subTest(keys=keys, value=value):
                altered = copy.deepcopy(self.qualified)
                target = altered
                for key in keys[:-1]:
                    target = target[key]
                target[keys[-1]] = value
                self.qualification.write_text(json.dumps(altered))
                with ExitStack() as stack, self.assertRaisesRegex(RuntimeError, "qualification does not bind"):
                    self.open(stack)
                self.assertFalse(self.receipt.exists())
                self.assertEqual(self.hold.read_bytes(), self.original_hold_bytes)
                self.assertEqual(self.backend.signals, [])
        for key in ("schema", "status", "stages", "selected_inputs", "activation_performed",
                    "reconciliation_sha256", "live_eligible_now"):
            with self.subTest(missing=key):
                altered = copy.deepcopy(self.qualified)
                del altered[key]
                self.qualification.write_text(json.dumps(altered))
                with ExitStack() as stack, self.assertRaisesRegex(RuntimeError, "qualification does not bind"):
                    self.open(stack)
                self.assertFalse(self.receipt.exists())
                self.assertEqual(self.hold.read_bytes(), self.original_hold_bytes)
        self.bridge_check.assert_not_called()

    def test_qualification_old_release_must_match_actual_bridge_not_original_hold(self):
        for key in ("manifest_sha256", "binary_sha256"):
            for value in (None, "bad", "0" * 64, self.old_ready[key]):
                with self.subTest(key=key, value=value):
                    altered = copy.deepcopy(self.qualified)
                    altered["stages"]["old"][key] = value
                    self.qualification.write_text(json.dumps(altered))
                    with ExitStack() as stack, self.assertRaisesRegex(RuntimeError, "qualified old stage"):
                        self.open(stack)
                    self.assertFalse(self.receipt.exists())
                    self.assertEqual(self.hold.read_bytes(), self.original_hold_bytes)
                    self.assertEqual(self.backend.signals, [])

    def test_qualification_drift_after_adoption_blocks_before_signal(self):
        with ExitStack() as stack:
            handoff, emit = self.open(stack)
            handoff.adopt(emit)
            self.qualified["status"] = "failed"
            self.qualification.write_text(json.dumps(self.qualified))
            successor = self.hold.read_bytes()
            with self.assertRaisesRegex(RuntimeError, "pinned recovery evidence changed"):
                self.backend.reload(expected_inputs=self.before_sources, handoff=handoff, emit=emit)
            self.assertEqual(self.hold.read_bytes(), successor)
            self.assertEqual(self.backend.signals, [])

    def test_retry_rejects_changed_approval_config_and_protected_peer(self):
        with ExitStack() as stack:
            handoff, emit = self.open(stack)
            handoff.adopt(emit)
        before = self.receipt.read_bytes()
        with ExitStack() as stack, self.assertRaisesRegex(RuntimeError, "stale recovery"):
            self.open(stack, stage_ack="different approval")
        self.backend.settings["changed"] = True
        with ExitStack() as stack, self.assertRaisesRegex(RuntimeError, "stale recovery"):
            self.open(stack)
        del self.backend.settings["changed"]
        self.backend.services["engine"] = 101
        with ExitStack() as stack, self.assertRaisesRegex(RuntimeError, "stale recovery"):
            self.open(stack)
        self.assertEqual(self.receipt.read_bytes(), before)

    def test_only_one_recovery_can_own_the_original_receipt_lock(self):
        with ExitStack() as stack:
            self.open(stack)
            with ExitStack() as other, self.assertRaises(BlockingIOError):
                self.open(other)
        self.assertEqual(self.backend.signals, [])

    def test_pre_signal_revalidation_rejects_native_or_backup_drift(self):
        with ExitStack() as stack:
            handoff, emit = self.open(stack)
            handoff.adopt(emit)
            self.bridge_check.side_effect = RuntimeError("candidate now incompatible")
            with self.assertRaisesRegex(RuntimeError, "candidate now incompatible"):
                self.backend.reload(expected_inputs=self.before_sources, handoff=handoff, emit=emit)
            self.bridge_check.side_effect = None
            saved = self.backup / "minime_autonomy/runtime.py"
            saved.write_text("changed before evidence")
            with self.assertRaisesRegex(RuntimeError, "backup differs"):
                self.backend.reload(expected_inputs=self.before_sources, handoff=handoff, emit=emit)
        self.assertTrue(self.hold.exists())
        self.assertEqual(self.backend.signals, [])

    def test_installation_receipt_requires_complete_exact_pairs(self):
        rows = evidence_rows(self.original_bytes)
        cases = [rows[:-1], rows + [{"phase": "signal_sent"}], [*rows[:1], *rows[2:]],
                 [rows[0], {**rows[1], "before": "0" * 64}, *rows[2:]],
                 [rows[0], rows[1], {**rows[2], "sha256": "0" * 64}, *rows[3:]]]
        for altered in cases:
            with self.subTest(count=len(altered)):
                self.original.write_text("".join(json.dumps(row) + "\n" for row in altered))
                with ExitStack() as stack, self.assertRaises(RuntimeError):
                    self.open(stack)
                self.assertFalse(self.receipt.exists())
                self.assertEqual(self.hold.read_bytes(), self.original_hold_bytes)

    def test_before_backup_source_status_and_qualification_are_not_inferred(self):
        targets = [self.backup / "minime_autonomy/runtime.py", self.status, self.qualification,
                   self.backend.root / "minime_autonomy/runtime.py", self.snapshot / "minime_autonomy/runtime.py"]
        for path in targets:
            with self.subTest(path=path.name):
                before = path.read_bytes()
                path.write_text("{}")
                with ExitStack() as stack, self.assertRaises(RuntimeError):
                    self.open(stack)
                path.write_bytes(before)
                self.assertFalse(self.receipt.exists())
        (self.backup / "unreviewed").write_text("extra")
        with ExitStack() as stack, self.assertRaisesRegex(RuntimeError, "backup differs"):
            self.open(stack)

    def test_symlinked_hold_receipt_or_evidence_is_refused(self):
        for path in (self.hold, self.original, self.packet, self.qualification, self.backup / "minime_autonomy/runtime.py"):
            with self.subTest(path=path):
                saved = path.with_name(path.name + ".saved")
                path.rename(saved)
                path.symlink_to(saved)
                with ExitStack() as stack, self.assertRaises(RuntimeError):
                    self.open(stack)
                path.unlink()
                saved.rename(path)
                self.assertFalse(self.receipt.exists())
        saved = self.root / "foreign-receipt"
        saved.write_text("foreign")
        self.receipt.symlink_to(saved)
        with ExitStack() as stack, self.assertRaises(RuntimeError):
            self.open(stack)
        self.assertEqual(saved.read_text(), "foreign")

    def test_pinned_receipt_replacement_and_foreign_hold_drift_block_before_signal(self):
        with ExitStack() as stack:
            handoff, emit = self.open(stack)
            handoff.adopt(emit)
            self.hold.write_text('{"foreign":true}')
            with self.assertRaisesRegex(RuntimeError, "ownership"):
                self.backend.reload(expected_inputs=self.before_sources, handoff=handoff, emit=emit)
            self.assertEqual(self.hold.read_text(), '{"foreign":true}')
            replacement = self.original.with_suffix(".replacement")
            replacement.write_bytes(self.original_bytes)
            replacement.replace(self.original)
            with self.assertRaisesRegex(RuntimeError, "receipt was replaced"):
                handoff.assert_resume_evidence()
        self.assertEqual(self.backend.signals, [])

    def test_duplicate_or_truncated_receipts_fail_closed(self):
        for raw in (self.original_bytes[:-1], b'{"phase":"x","phase":"y"}\n'):
            self.original.write_bytes(raw)
            with ExitStack() as stack, self.assertRaises(RuntimeError):
                self.open(stack)
            self.assertFalse(self.receipt.exists())

    def test_source_backup_uses_existing_eight_mib_source_limit(self):
        relative = "minime_autonomy/runtime.py"
        data = b"# old runtime\n" * 210_000
        (self.backup / relative).write_bytes(data)
        digest = hashlib.sha256(data).hexdigest()
        packet = json.loads(self.packet.read_text())
        packet["canonical_inputs"][relative] = digest
        self.packet.write_text(json.dumps(packet))
        self.qualified["reconciliation_sha256"] = hashlib.sha256(self.packet.read_bytes()).hexdigest()
        self.qualification.write_text(json.dumps(self.qualified))
        status = json.loads(self.status.read_text())
        status["source_inputs_at_start"][relative] = digest
        self.status.write_text(json.dumps(status))
        rows = evidence_rows(self.original_bytes)
        for row in rows:
            if row.get("phase") == "source_install_intent" and row.get("path") == relative:
                row["before"] = digest
        self.original.write_text("".join(json.dumps(row) + "\n" for row in rows))
        with ExitStack() as stack:
            handoff, emit = self.open(stack)
            handoff.adopt(emit)
        self.assertEqual(self.backend.signals, [])

    def test_in_place_recovery_receipt_edit_refuses_append(self):
        with ExitStack() as stack:
            handoff, emit = self.open(stack)
            handoff.adopt(emit)
            with self.receipt.open("a") as stream:
                stream.write('{"foreign":true}\n')
            before = self.receipt.read_bytes()
            with self.assertRaisesRegex(RuntimeError, "contents changed"):
                emit({"phase": "waiting_for_idle"})
            self.assertEqual(self.receipt.read_bytes(), before)
        self.assertEqual(self.backend.signals, [])

    def test_exact_retry_after_atomic_successor_before_adopted_record(self):
        with ExitStack() as stack:
            handoff, emit = self.open(stack)
            def interrupted(event):
                if event["phase"] == "resume_adopted":
                    raise RuntimeError("synthetic interruption")
                emit(event)
            with self.assertRaisesRegex(RuntimeError, "synthetic interruption"):
                handoff.adopt(interrupted)
        with ExitStack() as stack:
            handoff, emit = self.open(stack)
            handoff.adopt(emit)
        self.assert_preserved()
        self.assertEqual(self.backend.signals, [])

    def test_missing_and_nonprivate_hold_refuse_without_receipt(self):
        self.hold.chmod(0o644)
        with ExitStack() as stack, self.assertRaisesRegex(RuntimeError, "unsafe recovery evidence"):
            self.open(stack)
        self.hold.unlink()
        with ExitStack() as stack, self.assertRaises(OSError):
            self.open(stack)
        self.assertFalse(self.receipt.exists())

    def cli(self):
        return ["paired_minime_handoff.py", "--expected-pid", "10", "--pause-generation", "533",
                "--bridge-stage", str(self.stage), "--expected-inputs", str(self.packet),
                "--receipt", str(self.receipt), "--ack", "resume approved",
                "--resume-installed", str(self.original), "--resume-qualification", str(self.qualification),
                "--resume-stage-ack", "approve stage04", "--expected-started-at", "start-10"]

    def test_cli_adopts_without_install_and_retains_hold_on_bounded_refusal(self):
        with patch("sys.argv", self.cli()), patch("sys.stdout", new_callable=io.StringIO), patch(
                "paired_minime_handoff.LaunchdAgent", return_value=self.backend), patch.object(
                PairedHandoff, "install", side_effect=AssertionError("reinstall")), patch(
                "paired_minime_handoff.reload_agent", side_effect=RuntimeError("synthetic idle refusal")) as reload:
            self.assertEqual(paired.main(), 1)
        self.assertIsInstance(reload.call_args.kwargs["handoff"], InstalledHandoff)
        self.assertEqual(reload.call_args.kwargs["expected_inputs"], self.before_sources)
        self.assertTrue(self.hold.exists())
        self.assertEqual(evidence_rows(self.receipt.read_bytes())[-1]["owned_hold_retained"], True)
        self.assertEqual(self.backend.signals, [])
        self.assert_preserved()

    def test_cli_rejects_install_agent_only_and_missing_resume_arguments(self):
        orphan_options = self.cli()
        position = orphan_options.index("--resume-installed")
        del orphan_options[position:position + 2]
        commands = [self.cli() + ["--install-reviewed-overlay"], self.cli() + ["--agent-only"],
                    self.cli()[:-2], orphan_options]
        for command in commands:
            with self.subTest(command=command[-1]), patch("sys.argv", command), patch(
                    "sys.stderr", new_callable=io.StringIO), self.assertRaises(SystemExit):
                paired.main()
        self.assertFalse(self.receipt.exists())
        self.assertEqual(self.hold.read_bytes(), self.original_hold_bytes)


if __name__ == "__main__":
    unittest.main()
