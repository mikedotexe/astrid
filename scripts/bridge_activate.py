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
from bridge_stopped_recovery import (
    StoppedTransitionMixin, resume_stopped_transition, RUNTIME_FEEDBACK_FILE,
    RUNTIME_FEEDBACK_SNAPSHOT, read_runtime_feedback, runtime_feedback_descriptor,
    runtime_feedback_binding, verify_runtime_feedback_snapshot,
)

ROOT = Path("/Users/v/other/astrid")
LABEL = "com.astrid.spectral-bridge"
RECOVERY_TOOL_PATHS = frozenset({
    "scripts/bridge_activate.py",
    "scripts/bridge_stage.py",
    "scripts/bridge_stopped_recovery.py",
})


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


def resume_verification(backend, *, transaction: Path, expected_pid: int, actor: str,
                        ack: str, timeout: float = 600) -> dict:
    """Verify an already running replacement; never drain, signal or restart."""
    if expected_pid <= 1 or not actor.strip() or not ack.strip() or not 1 <= timeout <= 1800:
        raise ValueError("bounded verification recovery requires actor, acknowledgement and expected PID")
    if len(actor) > 128 or len(ack) > 2000:
        raise ValueError("operator evidence exceeds the reviewed bound")
    initial = backend.inspect_resume(transaction, expected_pid)
    witness = {"schema":"bridge_activation_verification_recovery_v1", "actor":actor,
               "acknowledgement_sha256":hashlib.sha256(ack.encode()).hexdigest(),
               "original_transaction":str(transaction),
               "original_failure_sha256":initial["original_failure_sha256"],
               "expected_pid":expected_pid, "expected_process":initial["process"],
               "owned_hold":initial["owned_hold"], "status":"verifying",
               "activation_performed":False, "signal_sent":False, "drain_requested":False,
               "restart_performed":False, "force_used":False, "automatic_rollback":False}
    backend.begin_recovery(witness)
    try:
        witness["new_process"] = backend.verify_new(initial["old_pid"], timeout,
            expected_pid=expected_pid, expected_identity=tuple(initial["process"]))
        backend.assert_resume_ownership(initial, expected_pid)
        backend.publish_manifest(initial["old_identity"], allow_current=True)
        witness["status"] = "verified_manifest_published"
        backend.record_recovery(witness)
        backend.assert_resume_ownership(initial, expected_pid)
        # Durable intent spans a crash after unlink but before the final record.
        # A retry still re-verifies this exact process and the published manifest.
        witness["status"] = "verified_release_pending"
        backend.record_recovery(witness)
        backend.release_resume_hold(initial)
        witness["status"] = "verification_recovered"
        backend.record_recovery(witness)
        return witness
    except BaseException as error:
        witness.update(status="failed_requires_review", error=str(error)[:2000])
        if initial["hold_present"]:
            try:
                backend.retain_hold()
            except Exception as hold_error:
                witness["hold_recovery_error"] = str(hold_error)[:2000]
        backend.record_recovery(witness)
        raise


class LaunchdBridge(StoppedTransitionMixin):
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

    def verify_bundle(self, *, allow_recovery_tool_changes: bool = False) -> None:
        if stage_tools.verify_stage(self.stage) != self.ready:
            raise RuntimeError("release witness changed during activation")
        # Source-facing runtime tools must still describe the tree that was built.
        source = Path(self.ready["source_root"])
        expected = stage_tools.json_file(self.stage / "source-inputs.json")
        current = stage_tools.input_snapshot(source, stage_tools.local_packages(source))
        allowed = (frozenset(str(source / path) for path in RECOVERY_TOOL_PATHS)
                   if allow_recovery_tool_changes else frozenset())
        if not stage_tools.same_input_contents(expected, current, allowed):
            raise RuntimeError("staged source inputs changed; rebuild before activation")

    def inspect(self, expected_pid: int) -> dict:
        self.ready = stage_tools.verify_stage(self.stage)
        if self.ready["schema"] != "bridge_staged_release_v2":
            raise ValueError("activation requires the startup-gated V2 release bundle")
        self.verify_bundle()
        if os.path.lexists(self.control / "hold.json"):
            raise RuntimeError("an existing operator launch hold must be resolved first")
        self.verify_launch_configuration(expected_pid)
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

    def verify_launch_configuration(self, expected_pid: int | None) -> None:
        installed = plistlib.loads((Path.home() / "Library/LaunchAgents" / f"{LABEL}.plist").read_bytes())
        source = plistlib.loads((self.root / "launchd" / f"{LABEL}.plist").read_bytes())
        if installed != source or installed.get("KeepAlive") is not True or installed.get("ProgramArguments") != ["/bin/bash", str(self.launcher)]:
            raise RuntimeError("launchd configuration requires separate reconciliation")
        job = self.job()
        match = re.search(r"^\targuments = \{\n(.*?)^\t\}", job, re.M | re.S)
        loaded = [] if match is None else [line.strip() for line in match[1].splitlines()]
        if loaded != installed["ProgramArguments"] or self.pid() != expected_pid:
            raise RuntimeError("loaded launchd identity does not match the expected job")

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
        manifest = stage_tools.json_file(self.stage / "manifest.json")
        for name, destination in (("release-launcher", self.launcher), ("release-selection", self.selection_helper)):
            original = self.stage / stage_tools.ARTIFACTS[name]
            expected_old = initial.get("launcher_sha256" if name == "release-launcher" else "selection_helper_sha256")
            if os.path.lexists(destination):
                if expected_old is None or digest(destination) != expected_old:
                    raise RuntimeError("live launch helper changed concurrently")
                atomic_bytes(self.transaction / (destination.name + ".before"), destination.read_bytes())
            data = original.read_bytes()
            if hashlib.sha256(data).hexdigest() != manifest["artifacts"][name]["sha256"]:
                raise RuntimeError("staged launch helper changed before installation")
            if os.path.lexists(destination) and digest(destination) == manifest["artifacts"][name]["sha256"]:
                continue
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
            # The process can exit between kill(0), lstart and comm observations.
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                return False
            if identity[0] == initial["started_at"]:
                status = subprocess.run(["ps", "-p", str(pid), "-o", "lstart=", "-o", "stat="],
                    capture_output=True, text=True, check=False, timeout=5)
                fields = status.stdout.strip().rsplit(None, 1)
                if (status.returncode == 0 and len(fields) == 2
                        and fields[0] == initial["started_at"] and fields[1].startswith("Z")):
                    return True  # Wait for confirmed kernel absence; never signal again.
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                return False
            raise RuntimeError("old PID was reused during transition")
        return True

    def snapshot_and_handoff(self, actor: str, ack: str) -> None:
        feedback_path = self.workspace / RUNTIME_FEEDBACK_FILE
        check = self.native("--verify-deployment-inputs")
        feedback, feedback_bytes = read_runtime_feedback(feedback_path)
        runtime_feedback_binding(check["checkpoint"], feedback)
        # No private keys are copied; these are the persisted continuity inputs.
        for name, path in (("conversation.before.json", self.workspace / "state.json"),
                           ("self-control.before.json", self.workspace / "self_control_v2/astrid/state.json"),
                           ("manifest.before.json", self.canonical_manifest)):
            before = stage_tools.sha(path)
            atomic_bytes(self.transaction / name, path.read_bytes())
            if stage_tools.sha(path) != before or stage_tools.sha(self.transaction / name) != before:
                raise RuntimeError("persisted input changed while taking transition snapshot")
        if feedback_bytes is not None:
            atomic_bytes(self.transaction / RUNTIME_FEEDBACK_SNAPSHOT, feedback_bytes)
        runtime_feedback_binding(check["checkpoint"], runtime_feedback_descriptor(feedback_path))
        after = self.native("--verify-deployment-inputs")
        runtime_feedback_binding(after["checkpoint"], feedback)
        if (check["checkpoint"]["sha256"] != stage_tools.sha(self.transaction / "conversation.before.json")
                or after["checkpoint"]["sha256"] != check["checkpoint"]["sha256"]):
            raise RuntimeError("conversation checkpoint changed while taking transition snapshot")
        # New receipts make absence explicit even for an older no-sidecar input.
        check["checkpoint"]["runtime_action_feedback"] = feedback
        verify_runtime_feedback_snapshot(check["checkpoint"], self.transaction, feedback_path)
        stage_tools.atomic_json(self.transaction / "inputs.before.json", check)
        self._stopped_feedback_binding = check["checkpoint"]
        if not check["self_control"]["state_targets_this_binary"]:
            result = self.native("--prepare-self-control-deployment-handoff", "--operator-actor", actor,
                                 "--operator-ack", ack)
            stage_tools.atomic_json(self.transaction / "handoff.json", result)

    def select_release(self, *, allow_recovery_tool_changes: bool = False) -> None:
        self.assert_stopped_feedback()
        self.verify_bundle(allow_recovery_tool_changes=allow_recovery_tool_changes)
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
        self.assert_stopped_feedback()
        path = self.control / "hold.json"
        if stage_tools.json_file(path) != self.guard:
            raise RuntimeError("launch hold ownership changed")
        path.unlink()
        sync_directory(path.parent)
        self.__dict__.pop("_stopped_feedback_binding", None)

    def assert_stopped_feedback(self) -> None:
        binding = getattr(self, "_stopped_feedback_binding", None)
        if binding is not None:
            runtime_feedback_binding(binding,
                runtime_feedback_descriptor(self.workspace / RUNTIME_FEEDBACK_FILE))
            verify_runtime_feedback_snapshot(binding, self.transaction,
                self.workspace / RUNTIME_FEEDBACK_FILE)

    def lifecycle_startup(self, pid: int, identity: tuple[str, str], expected: str) -> tuple[dict, dict, dict]:
        directory = self.root / ".runtime/bridge-lifecycle"
        status = drain.read_status(directory / f"{pid}.json")
        # `starting` is the exact producer phase before runtime admission. Reuse
        # all drain identity checks with a validation-only phase copy; the real
        # status stays unchanged and is never certified as running by this copy.
        validation = {**status, "phase":"running"} if status.get("phase") == "starting" else status
        drain.verify(validation, pid, Path(identity[1]), identity)
        if status["phase"] not in {"starting", "running"}:
            raise RuntimeError("replacement is no longer starting or running")
        startup = stage_tools.json_file(directory / f"{pid}.startup.json")
        control = startup.get("self_control", {})
        if startup.get("pid") != pid or control.get("state_deployment_identity") != expected or control.get("state_targets_this_binary") is not True:
            raise RuntimeError("new process did not pass the exact state-lineage startup gate")
        before = stage_tools.json_file(self.transaction / "inputs.before.json")
        if startup.get("checkpoint", {}).get("sha256") != before["checkpoint"]["sha256"]:
            raise RuntimeError("startup checkpoint was not the exact stopped-state snapshot")
        feedback = verify_runtime_feedback_snapshot(before["checkpoint"], self.transaction,
                                                     self.workspace / RUNTIME_FEEDBACK_FILE)
        runtime_feedback_binding(startup.get("checkpoint", {}), feedback)
        if "runtime_action_feedback" not in startup.get("checkpoint", {}):
            runtime_feedback_binding(startup.get("checkpoint", {}),
                runtime_feedback_descriptor(self.workspace / RUNTIME_FEEDBACK_FILE))
        return status, startup, before

    def verify_new(self, old_pid: int, timeout: float, *, expected_pid: int | None = None,
                   expected_identity: tuple[str, str] | None = None) -> dict:
        deadline = time.monotonic() + timeout
        expected = self.native("--verify-deployment-manifest")["deployment_identity"]
        while time.monotonic() < deadline:
            pid = self.pid()
            if expected_pid is not None and pid != expected_pid:
                raise RuntimeError("expected replacement PID changed during verification")
            if pid and pid != old_pid:
                identity = drain.process_identity(pid)
                if expected_identity is not None and identity != expected_identity:
                    raise RuntimeError("expected replacement process identity changed during verification")
                if Path(identity[1]) == self.stage / "spectral-bridge-server":
                    directory = self.root / ".runtime/bridge-lifecycle"
                    if (directory / f"{pid}.json").exists() and (directory / f"{pid}.startup.json").exists():
                        status, startup, before = self.lifecycle_startup(pid, identity, expected)
                        if status["phase"] == "starting":
                            time.sleep(1)
                            continue
                        current = self.native("--verify-deployment-inputs")
                        if current["checkpoint"]["exchange_count"] > before["checkpoint"]["exchange_count"]:
                            if current["self_control"]["state_targets_this_binary"] is not True or self.pid() != pid:
                                raise RuntimeError("new process or state lineage changed during verification")
                            if not self.model_idle():
                                time.sleep(1)
                                continue
                            return {"pid":pid, "started_at":identity[0], "binary":identity[1],
                                    "deployment_identity":expected, "startup":startup,
                                    "new_saved_exchange_observed":True, "model_idle_observed":True,
                                    "stopped_exchange_count":before["checkpoint"]["exchange_count"],
                                    "observed_exchange_count":current["checkpoint"]["exchange_count"]}
                elif expected_pid is not None:
                    raise RuntimeError("expected replacement executable does not match the selected stage")
            time.sleep(1)
        raise RuntimeError("new release did not complete startup and save a fresh exchange in time")

    def publish_manifest(self, initial: dict, *, allow_current: bool = False) -> None:
        current = stage_tools.sha(self.canonical_manifest)
        permitted = {initial["manifest_sha256"]}
        if allow_current:
            permitted.add(self.ready["manifest_sha256"])
        if current not in permitted:
            raise RuntimeError("canonical manifest changed concurrently; not overwritten")
        data = (self.stage / "manifest.json").read_bytes()
        if hashlib.sha256(data).hexdigest() != self.ready["manifest_sha256"]:
            raise RuntimeError("selected manifest changed before publication")
        if current != self.ready["manifest_sha256"]:
            atomic_bytes(self.canonical_manifest, data)

    def inspect_resume(self, transaction: Path, expected_pid: int) -> dict:
        if (not transaction.is_absolute() or transaction.resolve(strict=True) != transaction
                or transaction.parent != self.control / "transactions"
                or not re.fullmatch(r"[0-9a-f]{32}", transaction.name)):
            raise ValueError("verification recovery requires a canonical existing activation transaction")
        failed_path = transaction / "receipt.json"
        failed = stage_tools.json_file(failed_path)
        failure_sha = digest(failed_path)
        stopped_evidence = self.stopped_release_intent(transaction, failure_sha, failed)
        if (failed.get("schema") != "bridge_activation_v1"
                or failed.get("status") != "failed_requires_review"
                or not ((failed.get("activation_performed") is True
                         and failed.get("old_process_exited") is True)
                        or stopped_evidence)
                or failed.get("force_used") is not False
                or failed.get("automatic_rollback") is not False
                or failed.get("legacy_transition") is not False
                or failed.get("drain", {}).get("phase") != "drained"
                or failed.get("signal") != "SIGTERM"):
            raise ValueError("transaction is not an acknowledged post-activation verification failure")
        if type(failed.get("old_pid")) is not int or failed["old_pid"] <= 1 or failed["old_pid"] == expected_pid:
            raise ValueError("recovery PID must identify the replacement, not the old process")
        self.ready = stage_tools.verify_stage(self.stage)
        if (self.ready["schema"] != "bridge_staged_release_v2"
                or failed.get("stage") != str(self.stage)
                or failed.get("manifest_sha256") != self.ready["manifest_sha256"]
                or failed.get("binary_sha256") != self.ready["binary_sha256"]):
            raise RuntimeError("failed activation does not bind the requested staged release")
        self.verify_bundle()
        self.transaction = transaction
        self.old = failed["old_identity"]
        self.verify_launch_configuration(expected_pid)
        process = drain.process_identity(expected_pid)
        if Path(process[1]) != self.stage / "spectral-bridge-server":
            raise RuntimeError("recovery PID is not executing the selected staged binary")
        hold_path = self.control / "hold.json"
        hold_present = os.path.lexists(hold_path)
        if hold_present:
            guard = stage_tools.json_file(hold_path)
        else:
            guard = (stopped_evidence["owned_hold"] if stopped_evidence else
                     self.completed_recovery_guard(failure_sha, expected_pid, process))
        if guard.get("schema") != "bridge_launch_hold_v1" or guard.get("transaction") != str(transaction):
            raise RuntimeError("verification recovery does not own the current launch hold")
        self.guard = guard
        if stopped_evidence and guard != stopped_evidence["owned_hold"]:
            raise RuntimeError("stopped recovery launch hold changed")
        before = stage_tools.json_file(transaction / "inputs.before.json")
        checkpoint_sha = before["checkpoint"]["sha256"]
        if (digest(transaction / "conversation.before.json") != checkpoint_sha
                or failed["drain"].get("checkpoint_sha256") != checkpoint_sha
                or digest(transaction / "manifest.before.json") != self.old["manifest_sha256"]):
            raise RuntimeError("stopped checkpoint or prior manifest witness changed")
        expected = self.native("--verify-deployment-manifest")["deployment_identity"]
        self.lifecycle_startup(expected_pid, process, expected)
        current = self.native("--verify-deployment-inputs")
        if (current.get("self_control", {}).get("state_targets_this_binary") is not True
                or current["self_control"].get("state_deployment_identity") != expected):
            raise RuntimeError("current signed state does not target the selected release")
        initial = {"original_failure_sha256":failure_sha, "old_pid":failed["old_pid"],
                   "old_identity":self.old, "process":list(process), "owned_hold":guard,
                   "hold_present":hold_present, "stopped_recovery_evidence":stopped_evidence}
        self.assert_resume_ownership(initial, expected_pid)
        return initial

    def completed_recovery_guard(self, failure_sha: str, expected_pid: int,
                                 process: tuple[str, str]) -> dict:
        directory = self.transaction / "verification-recoveries"
        if not directory.is_dir() or directory.is_symlink():
            raise RuntimeError("owned launch hold is missing without completed recovery evidence")
        if digest(self.canonical_manifest) != self.ready["manifest_sha256"]:
            raise RuntimeError("missing hold recovery requires the already published selected manifest")
        paths = sorted(directory.glob("*/receipt.json"))
        if len(paths) > 1_000:
            raise RuntimeError("verification recovery history exceeds the reviewed bound")
        for path in paths:
            if path.resolve(strict=True) != path or not re.fullmatch(r"[0-9a-f]{32}", path.parent.name):
                raise RuntimeError("verification recovery history path is not canonical")
            record = stage_tools.json_file(path)
            if (record.get("schema") == "bridge_activation_verification_recovery_v1"
                    and record.get("status") in {"verification_recovered", "verified_release_pending"}
                    and record.get("original_transaction") == str(self.transaction)
                    and record.get("original_failure_sha256") == failure_sha
                    and record.get("expected_pid") == expected_pid
                    and record.get("expected_process") == list(process)
                    and record.get("stage") == str(self.stage)
                    and record.get("binary_sha256") == self.ready["binary_sha256"]
                    and record.get("new_process", {}).get("new_saved_exchange_observed") is True
                    and record.get("manifest_sha256") == self.ready["manifest_sha256"]):
                return record["owned_hold"]
        raise RuntimeError("owned launch hold is missing without completed recovery evidence")

    def assert_resume_ownership(self, initial: dict, expected_pid: int) -> None:
        self.verify_bundle()
        if digest(self.transaction / "receipt.json") != initial["original_failure_sha256"]:
            raise RuntimeError("original failure receipt changed during verification recovery")
        stopped = initial.get("stopped_recovery_evidence")
        if stopped and digest(Path(stopped["path"])) != stopped["sha256"]:
            raise RuntimeError("stopped recovery release intent changed during verification")
        if self.pid() != expected_pid or drain.process_identity(expected_pid) != tuple(initial["process"]):
            raise RuntimeError("recovery process identity changed")
        self.verify_launch_configuration(expected_pid)
        expected_selection = {"schema":"bridge_release_selection_v1", "stage":str(self.stage),
            "manifest_sha256":self.ready["manifest_sha256"], "transaction":str(self.transaction)}
        if stage_tools.json_file(self.control / "active.json") != expected_selection:
            raise RuntimeError("release selection changed during verification recovery")
        for name, path in (("release-launcher", self.launcher), ("release-selection", self.selection_helper)):
            if digest(path) != digest(self.stage / stage_tools.ARTIFACTS[name]):
                raise RuntimeError("selected launch helper changed during verification recovery")
        hold = self.control / "hold.json"
        if os.path.lexists(hold) != initial["hold_present"]:
            raise RuntimeError("launch hold presence changed during verification recovery")
        if os.path.lexists(hold):
            if stage_tools.json_file(hold) != self.guard:
                raise RuntimeError("foreign launch hold encountered during verification recovery")
        if digest(self.canonical_manifest) not in {self.old["manifest_sha256"], self.ready["manifest_sha256"]}:
            raise RuntimeError("canonical manifest changed concurrently; not overwritten")

    def begin_recovery(self, witness: dict, *, history: str = "verification-recoveries") -> None:
        if history not in {"verification-recoveries", "stopped-transition-recoveries"}:
            raise ValueError("unknown recovery history")
        directory = self.transaction / history
        if directory.exists() and (directory.is_symlink() or not directory.is_dir()):
            raise RuntimeError("invalid verification recovery history directory")
        directory.mkdir(mode=0o700, exist_ok=history != "stopped-transition-recoveries")
        sync_directory(self.transaction)
        self.recovery_transaction = directory / uuid.uuid4().hex
        self.recovery_transaction.mkdir(mode=0o700)
        sync_directory(directory)
        self.record_recovery(witness)

    def record_recovery(self, witness: dict) -> None:
        stage_tools.atomic_json(self.recovery_transaction / "receipt.json", {**witness,
            "recorded_at":dt.datetime.now(dt.UTC).isoformat(), "stage":str(self.stage),
            "manifest_sha256":self.ready["manifest_sha256"], "binary_sha256":self.ready["binary_sha256"]})

    def release_resume_hold(self, initial: dict) -> None:
        if initial["hold_present"]:
            self.release_hold()
        elif os.path.lexists(self.control / "hold.json"):
            raise RuntimeError("launch hold appeared during verification recovery")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage-dir", type=Path, required=True)
    parser.add_argument("--expected-pid", type=int, required=True)
    parser.add_argument("--resume-verification", type=Path)
    parser.add_argument("--resume-stopped-transition", type=Path)
    parser.add_argument("--actor", required=True)
    parser.add_argument("--ack", required=True)
    parser.add_argument("--legacy-stop-ack", default="")
    parser.add_argument("--timeout-secs", type=int, default=600)
    args = parser.parse_args()
    try:
        if os.environ.get("ASTRID_SANCTIONED_BRIDGE_ACTIVATION") != "1":
            raise ValueError("activate only through scripts/build_bridge.sh --activate-stage")
        if args.resume_stopped_transition is not None:
            if args.legacy_stop_ack or args.resume_verification is not None:
                raise ValueError("stopped recovery cannot accompany legacy or verification recovery")
            result = resume_stopped_transition(LaunchdBridge(args.stage_dir), transaction=args.resume_stopped_transition,
                expected_pid=args.expected_pid, actor=args.actor, ack=args.ack, timeout=args.timeout_secs)
        elif args.resume_verification is not None:
            if args.legacy_stop_ack:
                raise ValueError("verification-only recovery cannot acknowledge a legacy stop")
            result = resume_verification(LaunchdBridge(args.stage_dir), transaction=args.resume_verification,
                expected_pid=args.expected_pid, actor=args.actor, ack=args.ack, timeout=args.timeout_secs)
        else:
            result = activate(LaunchdBridge(args.stage_dir), expected_pid=args.expected_pid,
                              actor=args.actor, ack=args.ack, legacy_ack=args.legacy_stop_ack, timeout=args.timeout_secs)
        print(json.dumps(result, indent=2))
        return 0
    except (OSError, ValueError, RuntimeError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(json.dumps({"ok":False, "error":str(error)[:2000], "force_used":False, "automatic_rollback":False}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
