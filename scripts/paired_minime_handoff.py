#!/usr/bin/env python3
"""Sanctioned paired reader transition, with Minime replacement admission held.

Uses the agent idle/reload wrapper and the bridge stage/activation wrapper. No
forced termination, journal inspection, automatic rollback or automation resume.
Failure retains the owned launch hold for explicit operator recovery.
"""
from __future__ import annotations

import argparse
from contextlib import ExitStack
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import uuid

import bridge_stage
from bridge_activate import LaunchdBridge, atomic_bytes
from reconcile_minime_launch import OVERLAY, inventory, sha
from restart_minime_agent import (
    LaunchdAgent, ReviewedInstallation, digest, qualified_inputs, read_json, reload_agent,
)

BRIDGE = "com.astrid.spectral-bridge"
ROOT = Path(__file__).resolve().parents[1]


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def owned_bytes(path, *, private=False, limit=2_000_000):
    """Bounded, no-follow evidence reads; never adopt a detached or foreign inode."""
    require(path.is_absolute() and path.resolve(strict=True) == path,
            "recovery evidence path must be canonical")
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, "rb") as handle:
        before = os.fstat(handle.fileno())
        require(stat.S_ISREG(before.st_mode) and before.st_uid == os.getuid()
                and before.st_nlink == 1 and before.st_size <= limit
                and not before.st_mode & (0o077 if private else 0o022),
                "unsafe recovery evidence file")
        raw = handle.read(limit + 1)
        stamp = lambda value: (value.st_dev, value.st_ino, value.st_size,
                               value.st_mtime_ns, value.st_ctime_ns, value.st_mode)
        require(len(raw) <= limit and stamp(before) == stamp(os.fstat(handle.fileno()))
                == stamp(path.lstat()), "recovery evidence changed while reading")
        return raw


def evidence_json(raw):
    def unique(items):
        result = {}
        for key, value in items:
            require(key not in result, "duplicate recovery evidence field")
            result[key] = value
        return result
    def invalid(_):
        raise ValueError("nonfinite recovery evidence value")
    value = json.loads(raw, object_pairs_hook=unique, parse_constant=invalid)
    require(isinstance(value, dict), "recovery evidence must be an object")
    return value


def evidence_rows(raw):
    require(raw.endswith(b"\n"), "incomplete recovery receipt; explicit review required")
    rows = [evidence_json(line) for line in raw.splitlines()]
    require(0 < len(rows) <= 2000, "invalid recovery receipt length")
    return rows


def sync_parent(path):
    fd = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def verified_protected(before, after, receipt, stage):
    if (receipt.get("status") != "activated_verified"
            or receipt.get("force_used") is not False
            or receipt.get("legacy_transition") is not False
            or receipt.get("drain", {}).get("phase") != "drained"
            or receipt.get("old_pid") != before[BRIDGE]["pid"]
            or receipt.get("stage") != str(stage)):
        raise RuntimeError("paired bridge transition is not an acknowledged graceful activation")
    new = receipt["new_process"]
    expected = {**before, BRIDGE: {"pid": new["pid"], "started_at": new["started_at"]}}
    if after != expected or new.get("binary") != str(stage / "spectral-bridge-server"):
        raise RuntimeError("unreviewed protected process changed during paired activation")
    return expected


class PairedHandoff:
    def __init__(self, stage, receipt, ack, *, root=ROOT):
        self.stage = stage.resolve(strict=True)
        self.receipt = receipt.resolve()
        self.root = root
        self.ack = ack
        self.ready = bridge_stage.verify_stage(self.stage)
        self.guard = {"schema": "minime_paired_launch_hold_v1", "id": uuid.uuid4().hex,
                      "receipt": str(self.receipt), "stage": str(self.stage),
                      "manifest_sha256": self.ready["manifest_sha256"]}
        self.owned = False

    def create_hold(self, backend):
        self.hold = backend.root / "workspace/runtime/agent-launch-hold.json"
        if self.owned:
            self.assert_hold()
            return
        with self.hold.open("xb") as handle:
            os.chmod(self.hold, 0o600)
            handle.write(json.dumps(self.guard, sort_keys=True).encode())
            handle.flush()
            os.fsync(handle.fileno())
        self.owned = True
        sync_parent(self.hold)

    def install(self, backend, reconciliation, expected_pid, emit):
        """Mechanical explicit-path installation bound to the reviewed snapshot.

        Save original source bytes, never native authored state. A partial install
        retains the hold and exact per-file receipts; there is no automatic undo.
        """
        packet = read_json(reconciliation)
        selected = qualified_inputs(reconciliation)
        if (packet.get("overlay_paths") != list(OVERLAY)
                or packet.get("bindings", {}).get("canonical_root") != str(backend.root)):
            raise RuntimeError("installation is not the exact reviewed overlay")
        snapshot = Path(packet["snapshot_root"])
        from research_release_compatibility import verify_minime
        verify_minime(snapshot, backend.root / "workspace")
        expected = packet["canonical_inputs"].copy()
        if inventory(snapshot) != selected or backend.inputs() != expected:
            raise RuntimeError("installation source inventory changed")
        launcher = "scripts/launchd_autonomous_agent.sh"
        if b'while [ -e "$HOLD_PATH" ] || [ -L "$HOLD_PATH" ]; do' not in (snapshot / launcher).read_bytes():
            raise RuntimeError("qualified launcher has no admission hold")
        backend.validate()
        old_start = backend.identity(expected_pid)
        if not old_start or backend.pid() != expected_pid:
            raise RuntimeError("old agent changed before installation")
        backup = self.receipt.with_suffix(".source-before")
        backup.mkdir(mode=0o700)
        self.create_hold(backend)
        emit({"phase": "installation_held", "hold": self.guard, "backup": str(backup)})
        for path in [launcher, *(p for p in OVERLAY if p != launcher)]:
            self.assert_hold()
            backend.maintenance()
            if (backend.inputs() != expected or backend.pid() != expected_pid
                    or backend.identity(expected_pid) != old_start):
                raise RuntimeError("source/process changed during installation; hold retained")
            source, target = snapshot / path, backend.root / path
            if (source.resolve() != source or target.resolve() != target
                    or sha(source) != selected[path]):
                raise RuntimeError("overlay path or source changed")
            old = target.read_bytes() if target.exists() else None
            if old is not None:
                saved = backup / path
                saved.parent.mkdir(parents=True, exist_ok=True)
                atomic_bytes(saved, old)
            emit({"phase": "source_install_intent", "path": path,
                  "before": expected.get(path), "after": selected[path]})
            atomic_bytes(target, source.read_bytes(), target.stat().st_mode & 0o777 if old is not None else 0o644)
            expected[path] = selected[path]
            if backend.inputs() != expected:
                raise RuntimeError("source drift after file installation; hold retained")
            emit({"phase": "source_installed", "path": path, "sha256": selected[path]})
        if expected != selected:
            raise RuntimeError("installed inventory differs from qualified source")
        backend.reviewed_installation = ReviewedInstallation(backend, selected, self.assert_hold)
        emit({"phase": "installed_sources_verified", "schema": "minime_reviewed_install_v1",
              "selected_inputs_sha256": digest(selected)})
        # Exact inventory + owned hold replace the fixed post-write aging delay.
        # reload_agent still validates process identity, pending work and real idle.

    def assert_hold(self):
        if self.hold.is_symlink() or read_json(self.hold) != self.guard:
            raise RuntimeError("paired launch hold ownership changed")

    def begin(self, backend, inputs, config, protected, emit):
        self.hold = backend.root / "workspace/runtime/agent-launch-hold.json"
        launcher = backend.root / "scripts/launchd_autonomous_agent.sh"
        # Pin the reviewed launcher, not an arbitrary script containing a marker.
        expected = inputs["scripts/launchd_autonomous_agent.sh"]
        if hashlib.sha256(launcher.read_bytes()).hexdigest() != expected:
            raise RuntimeError("paired launcher changed")
        if b'while [ -e "$HOLD_PATH" ] || [ -L "$HOLD_PATH" ]; do' not in launcher.read_bytes():
            raise RuntimeError("paired launcher has no admission hold")
        backend.maintenance()
        if bridge_stage.verify_stage(self.stage) != self.ready:
            raise RuntimeError("paired stage changed")
        self.inputs, self.config = inputs, config
        self.create_hold(backend)
        emit({"phase": "replacement_admission_held", "hold": self.guard})

    def transition(self, backend, protected, emit):
        self.assert_hold()
        backend.maintenance()
        if backend.jobs()["active"]:
            raise RuntimeError("old agent left active jobs; paired hold retained")
        if (backend.inputs() != self.inputs or backend.config() != self.config
                or backend.protected() != protected):
            raise RuntimeError("paired boundary changed; hold retained")
        output = self.receipt.with_suffix(".bridge-output.txt")
        emit({"phase": "old_agent_exited_bridge_transition", "bridge_output": str(output)})
        with output.open("x") as handle:
            result = subprocess.run([
                "/bin/bash", str(self.root / "scripts/build_bridge.sh"),
                "--activate-stage", str(self.stage), "--expected-pid", str(protected[BRIDGE]["pid"]),
                "--actor", "codex-astra-interactive", "--ack", self.ack,
                "--timeout-secs", "900"], stdout=handle, stderr=subprocess.STDOUT,
                timeout=2100, check=False)
            handle.flush()
            os.fsync(handle.fileno())
        if result.returncode:
            raise RuntimeError("bridge activation failed; paired launch hold retained; inspect bridge output")
        control = Path("/Users/v/other/astrid/.runtime/bridge-deployment")
        selection = read_json(control / "active.json")
        transaction = Path(selection["transaction"])
        if (transaction.parent != control / "transactions" or transaction.resolve() != transaction
                or selection.get("stage") != str(self.stage)
                or selection.get("manifest_sha256") != self.ready["manifest_sha256"]):
            raise RuntimeError("paired release selection changed")
        receipt = read_json(transaction / "receipt.json")
        protected = verified_protected(protected, backend.protected(), receipt, self.stage)
        self.activation_receipt = receipt
        self.assert_hold()
        backend.maintenance()
        if backend.inputs() != self.inputs or backend.config() != self.config:
            raise RuntimeError("Minime launch source/config changed during bridge activation")
        emit({"phase": "paired_bridge_verified", "transaction": str(transaction),
              "protected": protected, "manifest_sha256": self.ready["manifest_sha256"]})
        self.hold.unlink()
        sync_parent(self.hold)
        self.owned = False
        emit({"phase": "replacement_admission_released"})
        return protected



class InstalledHandoff(PairedHandoff):
    """Resume only the completed-install, pre-signal boundary of an owned run.

    The original receipt is never amended. Its inode is also the exclusive
    recovery lock; a successor guard invalidates the original runner's guard.
    """

    def __init__(self, stage, receipt, ack, **kwargs):
        require(stage.is_absolute() and stage.resolve(strict=True) == stage
                and receipt.is_absolute() and receipt.resolve() == receipt,
                "recovery stage/receipt paths must be canonical, without symlinks")
        super().__init__(stage, receipt, ack, **kwargs)

    def installed_evidence(self, backend, reconciliation, original, expected_pid):
        raw = owned_bytes(original)
        rows = evidence_rows(raw)
        packet_raw = owned_bytes(reconciliation)
        packet = evidence_json(packet_raw)
        selected = qualified_inputs(reconciliation)
        before = packet.get("canonical_inputs")
        require(isinstance(before, dict) and before and all(
            isinstance(p, str) and p and not Path(p).is_absolute() and ".." not in Path(p).parts
            and isinstance(h, str) and re.fullmatch(r"[0-9a-f]{64}", h)
            for p, h in before.items()), "invalid canonical-before source inventory")
        require(packet.get("overlay_paths") == list(OVERLAY)
                and packet.get("bindings", {}).get("canonical_root") == str(backend.root)
                and packet["bindings"].get("observed_pid") == expected_pid,
                "recovery does not bind the reviewed installation and old PID")
        require(set(OVERLAY) <= selected.keys()
                and {**before, **{p: selected[p] for p in OVERLAY}} == selected,
                "reconciliation changes paths outside the reviewed overlay")
        snapshot = Path(packet["snapshot_root"])
        require(snapshot.is_absolute() and snapshot.resolve(strict=True) == snapshot
                and inventory(snapshot) == selected and inventory(backend.root) == selected
                and backend.inputs() == selected,
                "installed or frozen sources differ from reviewed after digests")
        guard = rows[0].get("hold", {})
        require(set(guard) == {"schema", "id", "receipt", "stage", "manifest_sha256"}
                and guard.get("schema") == "minime_paired_launch_hold_v1"
                and isinstance(guard.get("id"), str) and re.fullmatch(r"[0-9a-f]{32}", guard["id"])
                and guard.get("receipt") == str(original), "invalid original paired hold")
        old_stage = Path(guard["stage"])
        require(old_stage.is_absolute() and old_stage.resolve(strict=True) == old_stage
                and bridge_stage.verify_stage(old_stage)["manifest_sha256"] == guard["manifest_sha256"],
                "original stage identity changed")
        backup = original.with_suffix(".source-before")
        installed_count = 2 * len(OVERLAY) + 2
        installed = rows[:installed_count]
        terminal = {k: v for k, v in installed[-1].items() if k != "recorded_at"}
        complete = terminal in (
            {"phase": "installed_waiting_for_unchanged_quiet_window", "seconds": 185},
            {"phase": "installed_sources_verified", "schema": "minime_reviewed_install_v1",
             "selected_inputs_sha256": digest(selected)},
        )
        require(rows[0].get("phase") == "installation_held" and rows[0].get("backup") == str(backup)
                and len(installed) == installed_count and complete,
                "original installation is not completely receipted")
        tail = rows[installed_count:]
        if tail:
            require(len(tail) == 3, "original failure is not a bounded pre-signal refusal")
            wait, refusal, failure = tail
            reasons = {"pre_signal": "idle boundary moved; no signal sent",
                       "waiting_timeout": "no bounded idle window; no signal sent"}
            require(set(wait) - {"recorded_at"} == {
                        "phase", "old_pid", "old_started_at", "source_inputs", "config", "protected"}
                    and wait.get("phase") == "waiting_for_idle"
                    and wait.get("old_pid") == expected_pid
                    and wait.get("old_started_at") == backend.identity(expected_pid)
                    and wait.get("source_inputs") == selected
                    and wait.get("config") == backend.config()
                    and wait.get("protected") == backend.protected(),
                    "original pre-signal process/source/config boundary changed")
            require(refusal.get("phase") == "idle_boundary_refused"
                    and refusal.get("boundary") in reasons
                    and isinstance(refusal.get("diagnostic"), dict)
                    and refusal["diagnostic"].get("schema") == "minime_idle_boundary_diagnostic_v1"
                    and refusal["diagnostic"].get("signal_sent") is False
                    and {k: v for k, v in failure.items() if k != "recorded_at"} == {
                        "outcome": "failed", "error": reasons[refusal["boundary"]],
                        "owned_hold_retained": True, "forced_termination": False},
                    "original failure lacks exact no-signal and retained-hold evidence")
        order = ["scripts/launchd_autonomous_agent.sh", *(p for p in OVERLAY
                 if p != "scripts/launchd_autonomous_agent.sh")]
        for index, path in enumerate(order):
            intent, done = rows[1 + 2 * index:3 + 2 * index]
            require({k: v for k, v in intent.items() if k != "recorded_at"} == {
                "phase": "source_install_intent", "path": path,
                "before": before.get(path), "after": selected[path]}
                and {k: v for k, v in done.items() if k != "recorded_at"} == {
                    "phase": "source_installed", "path": path, "sha256": selected[path]},
                "source installation receipt is partial or inconsistent")
        require(backup.resolve(strict=True) == backup and backup.is_dir()
                and backup.stat().st_uid == os.getuid() and not backup.stat().st_mode & 0o077,
                "unsafe canonical-before backup directory")
        saved = {}
        for path in backup.rglob("*"):
            require(not path.is_symlink(), "symlink in canonical-before backup")
            if not path.is_dir():
                saved[str(path.relative_to(backup))] = hashlib.sha256(
                    owned_bytes(path, limit=8 * 1024 * 1024)).hexdigest()
        require(saved == {p: before[p] for p in OVERLAY if p in before},
                "canonical-before backup differs from installation receipt")
        status = evidence_json(owned_bytes(backend.root / "workspace/runtime/autonomous_agent_source_status.json"))
        require(status.get("pid") == expected_pid and status.get("source_inputs_at_start") == before
                and status.get("source_path") == str(backend.root / "minime_autonomy/runtime.py"),
                "running agent no longer binds the canonical-before source identity")
        return guard, selected, packet, {str(original): hashlib.sha256(raw).hexdigest(),
                                        str(reconciliation): hashlib.sha256(packet_raw).hexdigest()}

    def bridge_boundary(self, protected):
        peer = protected[BRIDGE]
        old = LaunchdBridge(self.stage).inspect(peer["pid"])
        require(old.get("started_at") == peer["started_at"] and old.get("drain_supported") is True,
                "recovery requires the exact graceful bridge peer")
        return old

    def open_resume(self, stack, backend, reconciliation, original, qualification,
                    expected_pid, expected_start, stage_ack):
        require(expected_pid > 1 and expected_start.strip() and 0 < len(stage_ack.strip()) <= 2000,
                "recovery requires explicit process start and reviewed stage approval")
        require(original.is_absolute() and original.resolve(strict=True) == original,
                "original receipt must use its canonical path")
        fd = os.open(original, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        self.original_lock = stack.enter_context(os.fdopen(fd, "rb"))
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        self.original_inode = (os.fstat(fd).st_dev, os.fstat(fd).st_ino)
        require(self.original_inode == (original.stat().st_dev, original.stat().st_ino),
                "original recovery receipt inode changed")
        self.hold = backend.root / "workspace/runtime/agent-launch-hold.json"
        original_guard, selected, packet, references = self.installed_evidence(
            backend, reconciliation, original, expected_pid)
        qualification_raw = owned_bytes(qualification)
        qualified = evidence_json(qualification_raw)
        stages = qualified.get("stages")
        require(qualified.get("schema") == "retained_accounts_paired_qualification_v1"
                and qualified.get("status") == "passed"
                and isinstance(stages, dict) and set(stages) == {"old", "new"}
                and stages["new"] == self.ready and isinstance(stages["old"], dict)
                and qualified.get("selected_inputs") == selected
                and qualified.get("reconciliation_sha256") == references[str(reconciliation)]
                and qualified.get("activation_performed") is False
                and qualified.get("live_eligible_now") is False,
                "qualification does not bind the explicit stage and installed Minime inputs")
        references[str(qualification)] = hashlib.sha256(qualification_raw).hexdigest()
        require(backend.pid() == expected_pid and backend.identity(expected_pid) == expected_start,
                "old agent PID/start identity changed")
        config, protected = backend.config(), backend.protected()
        require(config.get("installed_plist_sha256") == packet["bindings"].get("installed_plist_sha256")
                and config.get("installed_plist_sha256") is not None,
                "current launch configuration differs from reconciliation")
        boundary = self.bridge_boundary(protected)
        # The qualified old release is the running bridge, not the unactivated
        # stage retained in the original installation hold.
        require(all(isinstance(stages["old"].get(key), str)
                    and re.fullmatch(r"[0-9a-f]{64}", stages["old"][key])
                    and stages["old"][key] == boundary.get(key)
                    for key in ("manifest_sha256", "binary_sha256")),
                "qualified old stage does not match the current bridge manifest and binary")
        require(self.receipt.is_absolute() and self.receipt.parent == original.parent
                and self.receipt.parent.resolve(strict=True) == self.receipt.parent
                and self.receipt != original and not self.receipt.is_relative_to(backend.root)
                and not self.receipt.is_relative_to(self.stage), "unsafe recovery receipt destination")
        binding = {"schema": "minime_installed_resume_v1", "receipt": str(self.receipt),
                   "references": references, "old_pid": expected_pid, "old_started_at": expected_start,
                   "config": config, "protected": protected, "bridge_before": boundary,
                   "stage_approval_sha256": hashlib.sha256(stage_ack.encode()).hexdigest(),
                   "acknowledgement_sha256": hashlib.sha256(self.ack.encode()).hexdigest(),
                   "pause_generation": backend.pause_generation,
                   "boundary_observed_at_resume_not_original_install": True}
        self.guard = {**original_guard, "stage": str(self.stage),
                      "manifest_sha256": self.ready["manifest_sha256"], "resume": binding}
        self.original_guard = original_guard
        self.resume_binding = binding
        self.resume_args = (reconciliation, original, qualification, expected_pid)
        self.intent = {"phase": "resume_adoption_intent", "original_hold": original_guard,
                       "successor_hold": self.guard, "selected_stage": self.ready}
        # Recovery authenticates complete receipts and the installed inventory
        # anew, including an exact successor on retry, before bypassing edit age.
        def verify_install_hold():
            current = evidence_json(owned_bytes(self.hold, private=True))
            require(current in (original_guard, self.guard),
                    "installation hold ownership changed before recovery")
        def validate_installed():
            backend.reviewed_installation = ReviewedInstallation(backend, selected, verify_install_hold)
            backend.validate()
        current = evidence_json(owned_bytes(self.hold, private=True))
        self.new_receipt = not os.path.lexists(self.receipt)
        if self.new_receipt:
            require(current == original_guard, "foreign or already adopted hold; use its exact recovery receipt")
            validate_installed()
            fd = os.open(self.receipt, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        else:
            raw = owned_bytes(self.receipt, private=True)
            rows = evidence_rows(raw)
            require({k: v for k, v in rows[0].items() if k != "recorded_at"} == self.intent,
                    "stale recovery request; exact original inputs and approval required")
            for row in rows[1:]:
                phase = row.get("phase")
                require(phase in {"resume_adopted", "waiting_for_idle", "replacement_admission_held",
                                  "idle_boundary_refused"}
                        or (phase is None and row.get("outcome") == "failed"
                            and row.get("forced_termination") is False),
                        "recovery progressed beyond pre-signal boundary; do not replay")
                if phase == "waiting_for_idle":
                    require(all(row.get(k) == v for k, v in {
                        "old_pid": expected_pid, "old_started_at": expected_start, "source_inputs": selected,
                        "config": config, "protected": protected}.items()), "recovery boundary receipt changed")
                if phase in {"resume_adopted", "replacement_admission_held"}:
                    require(row.get("hold") == self.guard, "recovery successor receipt changed")
            require(current == self.guard or (current == original_guard and all(
                row.get("outcome") == "failed" for row in rows[1:])),
                    "foreign, missing or stale successor hold")
            validate_installed()
            fd = os.open(self.receipt, os.O_WRONLY | os.O_APPEND | os.O_NOFOLLOW | os.O_NONBLOCK)
        self.resume_handle = stack.enter_context(os.fdopen(fd, "a"))
        if not self.new_receipt:
            require(owned_bytes(self.receipt, private=True) == raw
                    and (os.fstat(fd).st_dev, os.fstat(fd).st_ino)
                    == (self.receipt.stat().st_dev, self.receipt.stat().st_ino),
                    "recovery receipt changed before append")
        self.recovery_inode = (os.fstat(fd).st_dev, os.fstat(fd).st_ino)
        self.recovery_bytes = b"" if self.new_receipt else raw
        return self.resume_handle

    def assert_resume_evidence(self):
        original = self.resume_args[1]
        require((original.stat().st_dev, original.stat().st_ino) == self.original_inode,
                "original recovery receipt was replaced")
        for path, expected in self.resume_binding["references"].items():
            require(hashlib.sha256(owned_bytes(Path(path))).hexdigest() == expected,
                    "pinned recovery evidence changed")
        require((self.receipt.stat().st_dev, self.receipt.stat().st_ino) == self.recovery_inode,
                "recovery receipt was replaced")
        require(owned_bytes(self.receipt, private=True) == self.recovery_bytes,
                "recovery receipt contents changed")

    def write_event(self, event):
        self.assert_resume_evidence()
        line = json.dumps(event, sort_keys=True) + "\n"
        require(len(self.recovery_bytes) + len(line.encode()) <= 2_000_000,
                "recovery receipt exceeds retry evidence bound")
        self.resume_handle.write(line)
        self.resume_handle.flush()
        os.fsync(self.resume_handle.fileno())
        self.recovery_bytes += line.encode()

    def adopt(self, emit):
        self.assert_resume_evidence()
        if self.new_receipt:
            emit(self.intent)
            sync_parent(self.receipt)
        current = evidence_json(owned_bytes(self.hold, private=True))
        require(current in (self.original_guard, self.guard), "hold changed before recovery adoption")
        if current == self.original_guard:
            atomic_bytes(self.hold, json.dumps(self.guard, sort_keys=True).encode())
        self.owned = True
        self.assert_hold()
        emit({"phase": "resume_adopted", "hold": self.guard, "source_install_performed": False})

    def begin(self, backend, inputs, config, protected, emit):
        self.assert_resume_evidence()
        reconciliation, original, _, expected_pid = self.resume_args
        self.installed_evidence(backend, reconciliation, original, expected_pid)
        require(backend.pid() == expected_pid
                and backend.identity(expected_pid) == self.resume_binding["old_started_at"]
                and config == self.resume_binding["config"] and protected == self.resume_binding["protected"]
                and self.bridge_boundary(protected) == self.resume_binding["bridge_before"],
                "recovery process/config/protected boundary changed; no signal sent")
        super().begin(backend, inputs, config, protected, emit)
        require(backend.inputs() == inputs and backend.config() == config and backend.protected() == protected,
                "recovery source/config/protected boundary moved during preflight; no signal sent")

    def transition(self, backend, protected, emit):
        self.assert_resume_evidence()
        return super().transition(backend, protected, emit)


class AgentOnlyHandoff(PairedHandoff):
    """Use the qualified installer/reload gates while retaining every peer process."""

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.guard["mode"] = "agent_only"

    def transition(self, backend, protected, emit):
        # reload_agent invokes this only after the validated old PID has exited.
        self.assert_hold()
        backend.maintenance()
        if backend.jobs()["active"]:
            raise RuntimeError("old agent left active jobs; agent-only hold retained")
        if (backend.inputs() != self.inputs or backend.config() != self.config
                or backend.protected() != protected):
            raise RuntimeError("agent-only boundary changed; hold retained")
        if bridge_stage.verify_stage(self.stage) != self.ready:
            raise RuntimeError("retained bridge stage changed; agent-only hold retained")
        self.assert_hold()
        emit({"phase": "agent_only_boundary_verified", "protected": protected,
              "retained_bridge_stage": str(self.stage), "manifest_sha256": self.ready["manifest_sha256"]})
        self.hold.unlink()
        sync_parent(self.hold)
        self.owned = False
        emit({"phase": "replacement_admission_released", "mode": "agent_only"})
        return protected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--expected-pid", type=int, required=True)
    parser.add_argument("--pause-generation", type=int, required=True)
    parser.add_argument("--bridge-stage", type=Path, required=True)
    parser.add_argument("--expected-inputs", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--ack", required=True)
    parser.add_argument("--install-reviewed-overlay", action="store_true")
    parser.add_argument("--agent-only", action="store_true",
                        help="retain the current bridge stage and all protected processes; reload only Minime")
    parser.add_argument("--resume-installed", type=Path,
                        help="original completed-install receipt; never reinstall or restore source")
    parser.add_argument("--resume-qualification", type=Path,
                        help="offline paired qualification binding the explicit stage and installed inputs")
    parser.add_argument("--resume-stage-ack",
                        help="explicit operator approval of the qualified successor stage")
    parser.add_argument("--expected-started-at",
                        help="operator-reviewed ps lstart identity of the still-running old Minime PID")
    parser.add_argument("--finish-accepted-work", action="store_true",
                        help="use qualified native v2 drain rather than waiting for an idle gap")
    parser.add_argument("--qualified-running-inputs", type=Path,
                        help="exact prior launch-source reconciliation for native v2 drain")
    args = parser.parse_args()
    if not args.ack.strip():
        parser.error("nonempty acknowledgement required")
    resume_options = (args.resume_qualification, args.resume_stage_ack, args.expected_started_at)
    if args.resume_installed:
        if args.install_reviewed_overlay or args.agent_only or not all(resume_options):
            parser.error("installed recovery requires qualification/start/stage approval and excludes install/agent-only")
    elif any(resume_options):
        parser.error("recovery options require --resume-installed")
    if args.finish_accepted_work:
        if not args.resume_installed or not args.qualified_running_inputs:
            parser.error("paired native drain requires installed recovery and qualified running inputs")
    elif args.qualified_running_inputs:
        parser.error("qualified running inputs require --finish-accepted-work")
    handoff = None
    with ExitStack() as stack:
        emit = lambda event: print(json.dumps(event), flush=True)
        try:
            handoff_type = (InstalledHandoff if args.resume_installed else
                            AgentOnlyHandoff if args.agent_only else PairedHandoff)
            handoff = handoff_type(args.bridge_stage, args.receipt, args.ack)
            if args.finish_accepted_work:
                from finish_minime_work import DrainAgent, finish_accepted_work
                backend = DrainAgent(args.pause_generation, args.ack)
                running = qualified_inputs(args.qualified_running_inputs)
                backend.drain_identity(args.expected_pid, running)
            else:
                backend = LaunchdAgent(args.pause_generation, args.ack)
            if args.resume_installed:
                handle = handoff.open_resume(stack, backend, args.expected_inputs, args.resume_installed,
                    args.resume_qualification, args.expected_pid, args.expected_started_at, args.resume_stage_ack)
            else:
                handle = stack.enter_context(args.receipt.open("x"))
            def emit(event):
                event = {"recorded_at": datetime.now(timezone.utc).isoformat(), **event}
                if args.resume_installed:
                    handoff.write_event(event)
                else:
                    handle.write(json.dumps(event, sort_keys=True) + "\n")
                    handle.flush()
                    os.fsync(handle.fileno())
                print(json.dumps({k: v for k, v in event.items() if k in
                    {"phase", "outcome", "old_pid", "new_pid", "error", "recorded_at",
                     "boundary", "diagnostic"}}), flush=True)
            if args.resume_installed:
                handoff.adopt(emit)
            if args.install_reviewed_overlay:
                handoff.install(backend, args.expected_inputs, args.expected_pid, emit)
            if args.finish_accepted_work:
                result = finish_accepted_work(backend, args.expected_pid,
                    running_inputs=running, expected_inputs=qualified_inputs(args.expected_inputs),
                    emit=emit, handoff=handoff, paired=True)
            else:
                result = reload_agent(backend, args.expected_pid,
                    expected_inputs=qualified_inputs(args.expected_inputs), emit=emit, handoff=handoff)
            emit(result)
        except (OSError, ValueError, RuntimeError, KeyError, TypeError, subprocess.SubprocessError) as error:
            failure = {"outcome": "failed", "error": str(error),
                       "owned_hold_retained": bool(handoff and handoff.owned), "forced_termination": False}
            try:
                emit(failure)
            except (OSError, ValueError, RuntimeError):
                print(json.dumps({**failure, "receipt_append_failed": True}), flush=True)
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
