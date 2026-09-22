#!/usr/bin/env python3
"""Sanctioned paired reader transition, with Minime replacement admission held.

Uses the agent idle/reload wrapper and the bridge stage/activation wrapper. No
forced termination, journal inspection, automatic rollback or automation resume.
Failure retains the owned launch hold for explicit operator recovery.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
import uuid

import bridge_stage
from bridge_activate import atomic_bytes
from reconcile_minime_launch import OVERLAY, inventory, sha
from restart_minime_agent import LaunchdAgent, qualified_inputs, read_json, reload_agent

BRIDGE = "com.astrid.spectral-bridge"
ROOT = Path(__file__).resolve().parents[1]


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
        """Mechanical seven-file installation bound to the reviewed snapshot.

        Save original source bytes, never native authored state. A partial install
        retains the hold and exact per-file receipts; there is no automatic undo.
        """
        packet = read_json(reconciliation)
        selected = qualified_inputs(reconciliation)
        if (packet.get("overlay_paths") != list(OVERLAY)
                or packet.get("bindings", {}).get("canonical_root") != str(backend.root)):
            raise RuntimeError("installation is not the exact reviewed overlay")
        snapshot = Path(packet["snapshot_root"])
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
        emit({"phase": "installed_waiting_for_unchanged_quiet_window", "seconds": 185})
        # Preserve the ordinary preflight's full 180-second quiet requirement.
        time.sleep(185)

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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--expected-pid", type=int, required=True)
    parser.add_argument("--pause-generation", type=int, required=True)
    parser.add_argument("--bridge-stage", type=Path, required=True)
    parser.add_argument("--expected-inputs", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--ack", required=True)
    parser.add_argument("--install-reviewed-overlay", action="store_true")
    args = parser.parse_args()
    if not args.ack.strip():
        parser.error("nonempty acknowledgement required")
    handoff = PairedHandoff(args.bridge_stage, args.receipt, args.ack)
    with args.receipt.open("x") as handle:
        def emit(event):
            event = {"recorded_at": datetime.now(timezone.utc).isoformat(), **event}
            handle.write(json.dumps(event, sort_keys=True) + "\n")
            handle.flush()
            os.fsync(handle.fileno())
            print(json.dumps({k: v for k, v in event.items() if k in
                {"phase", "outcome", "old_pid", "new_pid", "error", "recorded_at"}}), flush=True)
        try:
            backend = LaunchdAgent(args.pause_generation, args.ack)
            if args.install_reviewed_overlay:
                handoff.install(backend, args.expected_inputs, args.expected_pid, emit)
            result = reload_agent(backend, args.expected_pid,
                expected_inputs=qualified_inputs(args.expected_inputs), emit=emit, handoff=handoff)
            emit(result)
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            emit({"outcome": "failed", "error": str(error), "owned_hold_retained": handoff.owned,
                  "forced_termination": False})
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
