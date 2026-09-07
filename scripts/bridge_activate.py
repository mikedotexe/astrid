#!/usr/bin/env python3
"""Sanctioned stage activation. No forced termination or automatic rollback.

A durable launch hold prevents KeepAlive from admitting a replacement while
the old process exits and its exact persisted state is handed off. A legacy
SIGINT transition requires separate acknowledgement of unconfirmed in-flight work.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import signal
import subprocess
import time
import urllib.request
import uuid

import bridge_drain as drain
import bridge_stage as stage_tools
from bridge_release_launch import digest, runtime_arguments

ROOT = Path("/Users/v/other/astrid")
LABEL = "com.astrid.spectral-bridge"


def sync_directory(path: Path) -> None:
    directory = os.open(path, os.O_RDONLY)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


def atomic_bytes(path: Path, data: bytes, mode: int = 0o600) -> None:
    temporary = path.with_name(f".{path.name}.{uuid.uuid4().hex}.pending")
    with temporary.open("xb") as handle:
        os.chmod(temporary, mode)
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temporary, path)
    sync_directory(path.parent)


def activate(backend, *, expected_pid: int, actor: str, ack: str, legacy_ack: str,
             timeout: float = 600, now=time.monotonic, sleep=time.sleep) -> dict:
    if expected_pid <= 1 or not actor.strip() or not ack.strip() or not 1 <= timeout <= 1800:
        raise ValueError("bounded activation requires actor, acknowledgement and expected PID")
    if len(actor) > 128 or len(ack) > 2000 or len(legacy_ack) > 2000:
        raise ValueError("operator evidence exceeds the reviewed bound")
    initial = backend.inspect(expected_pid)
    legacy = not initial["drain_supported"]
    if legacy and not legacy_ack.strip():
        raise ValueError("legacy bridge requires explicit acknowledgement of unconfirmed in-flight work")
    receipt = {"schema":"bridge_activation_v1", "actor":actor,
               "acknowledgement_sha256":hashlib.sha256(ack.encode()).hexdigest(),
               "legacy_acknowledgement_sha256":hashlib.sha256(legacy_ack.encode()).hexdigest() if legacy else None,
               "old_pid":expected_pid, "old_identity":initial, "legacy_transition":legacy,
               "lossless_drain_claimed":False, "force_used":False, "automatic_rollback":False,
               "status":"preparing", "activation_performed":False}
    backend.begin(receipt)
    try:
        backend.install_hold_and_launcher(initial)
        backend.assert_old(expected_pid, initial)
        if not legacy:
            receipt["drain"] = backend.drain(expected_pid, timeout)
        else:
            deadline = now() + timeout
            while not backend.model_idle():
                backend.assert_old(expected_pid, initial)
                if now() >= deadline:
                    raise RuntimeError("no idle model window; launch hold retained, no signal sent")
                sleep(1)
            receipt["model_idle_observed_not_atomic_barrier"] = True
        backend.assert_old(expected_pid, initial)
        backend.signal(expected_pid, signal.SIGINT if legacy else signal.SIGTERM)
        receipt["signal"] = "SIGINT" if legacy else "SIGTERM"
        backend.record(receipt)
        deadline = now() + timeout
        while backend.old_exists(expected_pid, initial):
            if now() >= deadline:
                raise RuntimeError("old process did not exit; no force, launch hold retained")
            sleep(0.25)
        receipt["old_process_exited"] = True
        backend.snapshot_and_handoff(actor, ack)
        backend.select_release()
        backend.release_hold()
        receipt["activation_performed"] = True
        backend.record(receipt)
        receipt["new_process"] = backend.verify_new(expected_pid, timeout)
        backend.publish_manifest(initial)
        receipt["status"] = "activated_verified"
        backend.record(receipt)
        return receipt
    except BaseException as error:
        receipt.update(status="failed_requires_review", error=str(error)[:2000])
        try:
            backend.retain_hold()
        except Exception as recovery_error:
            receipt["hold_recovery_error"] = str(recovery_error)[:2000]
        try:
            backend.record(receipt)
        except Exception as receipt_error:
            raise RuntimeError(f"activation failed ({error}); receipt also failed ({receipt_error})") from error
        raise


class LaunchdBridge:
    def __init__(self, stage: Path, root: Path = ROOT):
        self.stage = stage.resolve(strict=True)
        self.root = root.resolve(strict=True)
        self.workspace = self.root / "capsules/spectral-bridge/workspace"
        self.control = self.root / ".runtime/bridge-deployment"
        self.target = f"gui/{os.getuid()}/{LABEL}"
        self.launcher = self.root / "scripts/launchd_spectral_bridge.sh"
        self.selection_helper = self.root / "scripts/bridge_release_launch.py"
        self.canonical_manifest = self.workspace / "deployment_manifests/spectral-bridge.json"
        self.transaction = self.control / "transactions" / uuid.uuid4().hex
        self.guard = {"schema":"bridge_launch_hold_v1", "transaction":str(self.transaction),
                      "created_at":dt.datetime.now(dt.UTC).isoformat()}
        self.ready = None
        self.old = None

    def job(self) -> str:
        return subprocess.run(["launchctl", "print", self.target], capture_output=True,
                              text=True, check=True, timeout=10).stdout

    def pid(self) -> int | None:
        match = re.search(r"^\s*pid = (\d+)\s*$", self.job(), re.M)
        return None if match is None else int(match[1])

    def args(self) -> list[str]:
        return runtime_arguments(self.root, Path(self.ready["source_root"]))

    def native(self, *options: str) -> dict:
        result = subprocess.run([str(self.stage / "spectral-bridge-server"), "--deployment-manifest",
                                 str(self.stage / "manifest.json"), *self.args(), *options],
                                capture_output=True, text=True, check=True, timeout=60)
        return json.loads(result.stdout)

    def verify_bundle(self) -> None:
        if stage_tools.verify_stage(self.stage) != self.ready:
            raise RuntimeError("release witness changed during activation")
        # Source-facing runtime tools must still describe the tree that was built.
        source = Path(self.ready["source_root"])
        expected = stage_tools.json_file(self.stage / "source-inputs.json")
        current = stage_tools.input_snapshot(source, stage_tools.local_packages(source))
        if current != expected:
            raise RuntimeError("staged source inputs changed; rebuild before activation")

    def inspect(self, expected_pid: int) -> dict:
        self.ready = stage_tools.verify_stage(self.stage)
        if self.ready["schema"] != "bridge_staged_release_v2":
            raise ValueError("activation requires the startup-gated V2 release bundle")
        self.verify_bundle()
        if os.path.lexists(self.control / "hold.json"):
            raise RuntimeError("an existing operator launch hold must be resolved first")
        installed = plistlib.loads((Path.home() / "Library/LaunchAgents" / f"{LABEL}.plist").read_bytes())
        source = plistlib.loads((self.root / "launchd" / f"{LABEL}.plist").read_bytes())
        if installed != source or installed.get("KeepAlive") is not True or installed.get("ProgramArguments") != ["/bin/bash", str(self.launcher)]:
            raise RuntimeError("launchd configuration requires separate reconciliation")
        job = self.job()
        match = re.search(r"^\targuments = \{\n(.*?)^\t\}", job, re.M | re.S)
        loaded = [] if match is None else [line.strip() for line in match[1].splitlines()]
        if loaded != installed["ProgramArguments"] or self.pid() != expected_pid:
            raise RuntimeError("loaded launchd identity does not match the expected job")
        identity = drain.process_identity(expected_pid)
        binary = Path(identity[1]).resolve(strict=True)
        manifest = stage_tools.json_file(self.canonical_manifest)
        artifact = manifest["artifacts"]["spectral-bridge"]
        if artifact["path"] != str(binary) or artifact["sha256"] != stage_tools.sha(binary):
            raise RuntimeError("live binary is not bound by its canonical manifest")
        status_path = self.root / ".runtime/bridge-lifecycle" / f"{expected_pid}.json"
        supported = status_path.exists()
        if supported:
            drain.verify(drain.read_status(status_path), expected_pid, binary, identity)
        # The real target binary checks the complete SavedState schema, without mutation.
        self.native("--verify-deployment-inputs")
        self.old = {"started_at":identity[0], "binary":str(binary), "binary_sha256":stage_tools.sha(binary),
                    "manifest_sha256":stage_tools.sha(self.canonical_manifest),
                    "launcher_sha256":stage_tools.sha(self.launcher), "drain_supported":supported}
        if os.path.lexists(self.selection_helper):
            self.old["selection_helper_sha256"] = digest(self.selection_helper)
        if os.path.lexists(self.control / "active.json"):
            self.old["selection_sha256"] = digest(self.control / "active.json")
        return self.old

    def begin(self, receipt: dict) -> None:
        if self.control.resolve() != self.control:
            raise RuntimeError("deployment control path is not canonical")
        self.control.mkdir(parents=True, mode=0o700, exist_ok=True)
        (self.control / "transactions").mkdir(mode=0o700, exist_ok=True)
        self.transaction.mkdir(parents=True, mode=0o700)
        self.record(receipt)

    def record(self, receipt: dict) -> None:
        stage_tools.atomic_json(self.transaction / "receipt.json", {**receipt,
            "recorded_at":dt.datetime.now(dt.UTC).isoformat(), "stage":str(self.stage),
            "manifest_sha256":self.ready["manifest_sha256"], "binary_sha256":self.ready["binary_sha256"]})

    def retain_hold(self) -> None:
        path = self.control / "hold.json"
        if os.path.lexists(path):
            if stage_tools.json_file(path) != self.guard:
                raise RuntimeError("foreign launch hold encountered; not overwritten")
            return
        with path.open("xb") as handle:
            os.chmod(path, 0o600)
            handle.write(stage_tools.encoded(self.guard))
            handle.flush()
            os.fsync(handle.fileno())
        sync_directory(path.parent)

    def install_hold_and_launcher(self, initial: dict) -> None:
        self.verify_bundle()
        if digest(self.launcher) != initial["launcher_sha256"]:
            raise RuntimeError("launcher changed before installation")
        self.retain_hold()
        for name, destination in (("release-launcher", self.launcher), ("release-selection", self.selection_helper)):
            original = self.stage / stage_tools.ARTIFACTS[name]
            expected_old = initial.get("launcher_sha256" if name == "release-launcher" else "selection_helper_sha256")
            if os.path.lexists(destination):
                if expected_old is None or digest(destination) != expected_old:
                    raise RuntimeError("live launch helper changed concurrently")
                atomic_bytes(self.transaction / (destination.name + ".before"), destination.read_bytes())
            data = original.read_bytes()
            manifest = stage_tools.json_file(self.stage / "manifest.json")
            if hashlib.sha256(data).hexdigest() != manifest["artifacts"][name]["sha256"]:
                raise RuntimeError("staged launch helper changed before installation")
            atomic_bytes(destination, data, 0o755)

    def assert_old(self, pid: int, initial: dict) -> None:
        if self.pid() != pid or drain.process_identity(pid) != (initial["started_at"], initial["binary"]):
            raise RuntimeError("old process identity changed; no signal sent")
        if stage_tools.sha(Path(initial["binary"])) != initial["binary_sha256"]:
            raise RuntimeError("old executable changed")
        if digest(self.canonical_manifest) != initial["manifest_sha256"]:
            raise RuntimeError("old manifest changed before signal")
        if stage_tools.json_file(self.control / "hold.json") != self.guard:
            raise RuntimeError("launch hold ownership changed before signal")
        for name, destination in (("release-launcher", self.launcher), ("release-selection", self.selection_helper)):
            if digest(destination) != digest(self.stage / stage_tools.ARTIFACTS[name]):
                raise RuntimeError("installed launch helper changed before signal")

    def drain(self, pid: int, timeout: float) -> dict:
        return drain.drain(pid, Path(self.old["binary"]), self.root / ".runtime/bridge-lifecycle", timeout, True)

    def model_idle(self) -> bool:
        try:
            with urllib.request.urlopen("http://127.0.0.1:8090/readyz", timeout=3) as response:
                state = json.load(response)
            return (state.get("ready") is True and state.get("worker", {}).get("phase") == "ready"
                    and state.get("worker", {}).get("queue_depth") == 0
                    and state.get("reservoir", {}).get("status") == "connected")
        except (OSError, ValueError):
            return False

    def signal(self, pid: int, sig: int) -> None:
        os.kill(pid, sig)

    def old_exists(self, pid: int, initial: dict) -> bool:
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return False
        try:
            identity = drain.process_identity(pid)
        except drain.DrainError:
            # A failed ps lookup alone is not proof of exit.
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                return False
            raise
        if identity != (initial["started_at"], initial["binary"]):
            raise RuntimeError("old PID was reused during transition")
        return True

    def snapshot_and_handoff(self, actor: str, ack: str) -> None:
        # No private keys are copied; only the persisted conversation and control state.
        for name, path in (("conversation.before.json", self.workspace / "state.json"),
                           ("self-control.before.json", self.workspace / "self_control_v2/astrid/state.json"),
                           ("manifest.before.json", self.canonical_manifest)):
            before = stage_tools.sha(path)
            atomic_bytes(self.transaction / name, path.read_bytes())
            if stage_tools.sha(path) != before or stage_tools.sha(self.transaction / name) != before:
                raise RuntimeError("persisted input changed while taking transition snapshot")
        check = self.native("--verify-deployment-inputs")
        stage_tools.atomic_json(self.transaction / "inputs.before.json", check)
        if not check["self_control"]["state_targets_this_binary"]:
            result = self.native("--prepare-self-control-deployment-handoff", "--operator-actor", actor,
                                 "--operator-ack", ack)
            stage_tools.atomic_json(self.transaction / "handoff.json", result)

    def select_release(self) -> None:
        self.verify_bundle()
        path = self.control / "active.json"
        previous = self.old.get("selection_sha256")
        if os.path.lexists(path) and (previous is None or digest(path) != previous):
            raise RuntimeError("release selection changed concurrently")
        if previous is not None and not path.exists():
            raise RuntimeError("prior release selection disappeared")
        if path.exists():
            atomic_bytes(self.transaction / "selection.before.json", path.read_bytes())
        stage_tools.atomic_json(path, {"schema":"bridge_release_selection_v1", "stage":str(self.stage),
            "manifest_sha256":self.ready["manifest_sha256"], "transaction":str(self.transaction)})

    def release_hold(self) -> None:
        path = self.control / "hold.json"
        if stage_tools.json_file(path) != self.guard:
            raise RuntimeError("launch hold ownership changed")
        path.unlink()
        sync_directory(path.parent)

    def verify_new(self, old_pid: int, timeout: float) -> dict:
        deadline = time.monotonic() + timeout
        expected = self.native("--verify-deployment-manifest")["deployment_identity"]
        while time.monotonic() < deadline:
            pid = self.pid()
            if pid and pid != old_pid:
                identity = drain.process_identity(pid)
                if Path(identity[1]) == self.stage / "spectral-bridge-server":
                    directory = self.root / ".runtime/bridge-lifecycle"
                    if (directory / f"{pid}.json").exists() and (directory / f"{pid}.startup.json").exists():
                        status = drain.read_status(directory / f"{pid}.json")
                        drain.verify(status, pid, Path(identity[1]), identity)
                        startup = stage_tools.json_file(directory / f"{pid}.startup.json")
                        control = startup.get("self_control", {})
                        if startup.get("pid") != pid or control.get("state_deployment_identity") != expected or control.get("state_targets_this_binary") is not True:
                            raise RuntimeError("new process did not pass the exact state-lineage startup gate")
                        current = self.native("--verify-deployment-inputs")
                        before = stage_tools.json_file(self.transaction / "inputs.before.json")
                        if startup.get("checkpoint", {}).get("sha256") != before["checkpoint"]["sha256"]:
                            raise RuntimeError("startup checkpoint was not the exact stopped-state snapshot")
                        if status["phase"] == "running" and current["checkpoint"]["exchange_count"] > before["checkpoint"]["exchange_count"]:
                            if current["self_control"]["state_targets_this_binary"] is not True or self.pid() != pid:
                                raise RuntimeError("new process or state lineage changed during verification")
                            if not self.model_idle():
                                time.sleep(1)
                                continue
                            return {"pid":pid, "started_at":identity[0], "binary":identity[1],
                                    "deployment_identity":expected, "startup":startup,
                                    "new_saved_exchange_observed":True}
            time.sleep(1)
        raise RuntimeError("new release did not complete startup and save a fresh exchange in time")

    def publish_manifest(self, initial: dict) -> None:
        if stage_tools.sha(self.canonical_manifest) != initial["manifest_sha256"]:
            raise RuntimeError("canonical manifest changed concurrently; not overwritten")
        data = (self.stage / "manifest.json").read_bytes()
        if hashlib.sha256(data).hexdigest() != self.ready["manifest_sha256"]:
            raise RuntimeError("selected manifest changed before publication")
        atomic_bytes(self.canonical_manifest, data)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage-dir", type=Path, required=True)
    parser.add_argument("--expected-pid", type=int, required=True)
    parser.add_argument("--actor", required=True)
    parser.add_argument("--ack", required=True)
    parser.add_argument("--legacy-stop-ack", default="")
    parser.add_argument("--timeout-secs", type=int, default=600)
    args = parser.parse_args()
    try:
        if os.environ.get("ASTRID_SANCTIONED_BRIDGE_ACTIVATION") != "1":
            raise ValueError("activate only through scripts/build_bridge.sh --activate-stage")
        result = activate(LaunchdBridge(args.stage_dir), expected_pid=args.expected_pid,
                          actor=args.actor, ack=args.ack, legacy_ack=args.legacy_stop_ack, timeout=args.timeout_secs)
        print(json.dumps(result, indent=2))
        return 0
    except (OSError, ValueError, RuntimeError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(json.dumps({"ok":False, "error":str(error)[:2000], "force_used":False, "automatic_rollback":False}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
