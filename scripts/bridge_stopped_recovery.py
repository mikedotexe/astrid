"""One bounded continuation of an acknowledged, stopped bridge transition.

Partial continuation is never replayed. A running replacement can instead use
the separate verification-only path after durable release intent was recorded.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import stat

import bridge_stage as stage_tools
from bridge_release_launch import digest

RUNTIME_FEEDBACK_FILE = "runtime_action_feedback_v1.json"
RUNTIME_FEEDBACK_SNAPSHOT = "runtime-action-feedback.before.json"
RUNTIME_FEEDBACK_SCHEMA = "pending_runtime_action_feedback_v1"


def read_runtime_feedback(path: Path) -> tuple[dict, bytes | None]:
    """Read a private versioned sidecar without following a final symlink."""
    try:
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    except FileNotFoundError:
        # A dangling symlink is unknown state, not an absent queue.
        if os.path.lexists(path):
            raise RuntimeError("runtime feedback sidecar is not a regular file")
        return {"present":False}, None
    with os.fdopen(descriptor, "rb") as handle:
        before = os.fstat(handle.fileno())
        if not stat.S_ISREG(before.st_mode) or before.st_mode & 0o077:
            raise RuntimeError("runtime feedback sidecar is not a private regular file")
        data = handle.read(64 * 1024 * 1024 + 1)
        after = os.fstat(handle.fileno())
    current = path.lstat()
    identity = lambda item: (item.st_dev, item.st_ino, item.st_size,
                             item.st_mtime_ns, item.st_ctime_ns, item.st_mode)
    if identity(before) != identity(after) or identity(after) != identity(current):
        raise RuntimeError("runtime feedback sidecar changed while being read")
    if len(data) > 64 * 1024 * 1024:
        raise RuntimeError("runtime feedback checkpoint exceeds 64 MiB")
    def unique_object(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("duplicate runtime feedback JSON field")
            result[key] = value
        return result
    def invalid_constant(value):
        raise ValueError(f"invalid runtime feedback JSON constant: {value}")
    value = json.loads(data.decode("utf-8"), object_pairs_hook=unique_object,
                       parse_constant=invalid_constant)
    if (not isinstance(value, dict) or set(value) != {"schema", "pending_runtime_feedback"}
            or value["schema"] != RUNTIME_FEEDBACK_SCHEMA
            or not isinstance(value["pending_runtime_feedback"], list)):
        raise RuntimeError("unsupported runtime feedback sidecar schema")
    ids = set()
    for item in value["pending_runtime_feedback"]:
        if (not isinstance(item, dict)
                or any(not isinstance(item.get(key), str) or not item[key].strip()
                       for key in ("id", "requested_action", "status", "message"))
                or any(item.get(key) is not None and not isinstance(item[key], str)
                       for key in ("reason", "suggested_next"))
                or item["id"] in ids):
            raise RuntimeError("invalid runtime feedback record or duplicate identity")
        ids.add(item["id"])
    return {"present":True, "schema":RUNTIME_FEEDBACK_SCHEMA,
            "sha256":hashlib.sha256(data).hexdigest(), "pending_count":len(ids)}, data


def runtime_feedback_descriptor(path: Path) -> dict:
    return read_runtime_feedback(path)[0]


def runtime_feedback_binding(checkpoint: dict, actual: dict) -> dict:
    """Older checkpoint evidence is compatible only with an absent sidecar."""
    declared = checkpoint.get("runtime_action_feedback", {"present":False})
    valid = isinstance(declared, dict) and (
        declared == {"present":False} and declared.get("present") is False
        or set(declared) == {"present", "schema", "sha256", "pending_count"}
        and declared.get("present") is True and declared.get("schema") == RUNTIME_FEEDBACK_SCHEMA
        and isinstance(declared.get("sha256"), str)
        and re.fullmatch(r"[0-9a-f]{64}", declared["sha256"])
        and type(declared.get("pending_count")) is int and declared["pending_count"] >= 0)
    if not valid or declared != actual:
        raise RuntimeError("runtime feedback checkpoint presence or contents changed")
    return declared


def verify_runtime_feedback_snapshot(checkpoint: dict, transaction: Path,
                                     live_path: Path) -> dict:
    saved = runtime_feedback_descriptor(transaction / RUNTIME_FEEDBACK_SNAPSHOT)
    expected = runtime_feedback_binding(checkpoint, saved)
    if "runtime_action_feedback" not in checkpoint:
        runtime_feedback_binding(checkpoint, runtime_feedback_descriptor(live_path))
    return expected


def resume_stopped_transition(backend, *, transaction: Path, expected_pid: int,
                              actor: str, ack: str, timeout: float = 600) -> dict:
    if (expected_pid <= 1 or not actor.strip() or not ack.strip()
            or len(actor) > 128 or len(ack) > 1024 or not 1 <= timeout <= 1800):
        raise ValueError("bounded stopped recovery requires actor, acknowledgement and original PID")
    initial = backend.inspect_stopped(transaction, expected_pid)
    witness = {"schema":"bridge_stopped_transition_recovery_v1", "actor":actor,
        "acknowledgement_sha256":hashlib.sha256(ack.encode()).hexdigest(),
        "original_transaction":str(transaction), **initial, "status":"preparing",
        "old_process_exited":True, "signal_sent":False, "drain_requested":False,
        "force_used":False, "automatic_rollback":False, "release_intent":False,
        "activation_performed":False}
    backend.begin_recovery(witness, history="stopped-transition-recoveries")
    try:
        backend.assert_stopped_ownership(initial)
        if not initial["snapshot_prepared"]:
            backend.snapshot_and_handoff(actor, ack)
        backend.assert_stopped_ownership(initial)
        backend.validate_stopped_snapshot(initial)
        witness["status"] = "snapshot_and_handoff_verified"
        backend.record_recovery(witness)
        backend.select_release(allow_recovery_tool_changes=True)
        backend.assert_stopped_ownership(initial, selected=True)
        backend.validate_stopped_snapshot(initial)
        # This intent admits verification-only recovery if the operator dies
        # after unlinking the hold but before recording the running replacement.
        witness.update(status="release_pending", release_intent=True)
        backend.record_recovery(witness)
        backend.release_hold()
        witness.update(status="verifying", activation_performed=True)
        backend.record_recovery(witness)
        witness["new_process"] = backend.verify_new(expected_pid, timeout)
        backend.publish_manifest(initial["old_identity"])
        witness["status"] = "transition_recovered"
        backend.record_recovery(witness)
        return witness
    except BaseException as error:
        witness.update(status="failed_requires_review", error=str(error)[:2000])
        try:
            backend.retain_hold()
        except Exception as hold_error:
            witness["hold_recovery_error"] = str(hold_error)[:2000]
        backend.record_recovery(witness)
        raise


class StoppedTransitionMixin:
    def inspect_stopped(self, transaction: Path, expected_pid: int) -> dict:
        if (not transaction.is_absolute() or transaction.resolve(strict=True) != transaction
                or transaction.parent != self.control / "transactions"
                or not re.fullmatch(r"[0-9a-f]{32}", transaction.name)):
            raise ValueError("stopped recovery requires a canonical existing activation transaction")
        failed = stage_tools.json_file(transaction / "receipt.json")
        if (failed.get("schema") != "bridge_activation_v1"
                or failed.get("status") != "failed_requires_review"
                or failed.get("activation_performed") is not False
                or failed.get("force_used") is not False
                or failed.get("automatic_rollback") is not False
                or failed.get("legacy_transition") is not False
                or failed.get("signal") != "SIGTERM"
                or failed.get("old_pid") != expected_pid
                or failed.get("drain", {}).get("supported") is not True
                or failed["drain"].get("pid") != expected_pid
                or failed["drain"].get("phase") != "drained"):
            raise ValueError("transaction is not an acknowledged stopped transition")
        baseline = {"receipt.json", "launchd_spectral_bridge.sh.before", "bridge_release_launch.py.before"}
        names = {path.name for path in transaction.iterdir()}
        snapshot_required = {"conversation.before.json", "self-control.before.json",
                             "manifest.before.json", "inputs.before.json"}
        snapshot_allowed = snapshot_required | {RUNTIME_FEEDBACK_SNAPSHOT, "handoff.json"}
        extras = names - baseline
        snapshot_prepared = False
        targets_binary = False
        if extras:
            if not snapshot_required <= extras or not extras <= snapshot_allowed:
                raise RuntimeError("transition already has progress; do not replay, use verification recovery if running")
            before = stage_tools.json_file(transaction / "inputs.before.json")
            checkpoint = before.get("checkpoint", {})
            self_control = before.get("self_control", {})
            if not isinstance(checkpoint, dict) or not isinstance(self_control, dict):
                raise RuntimeError("stopped transition snapshot is partial or internally inconsistent")
            feedback = checkpoint.get("runtime_action_feedback", {"present":False})
            targets_binary = self_control.get("state_targets_this_binary")
            if not isinstance(feedback, dict) or type(targets_binary) is not bool:
                raise RuntimeError("stopped transition snapshot is partial or internally inconsistent")
            feedback_present = feedback.get("present") is True
            required = set(snapshot_required)
            if feedback_present:
                required.add(RUNTIME_FEEDBACK_SNAPSHOT)
            if not targets_binary:
                required.add("handoff.json")
            if extras != required:
                raise RuntimeError("stopped transition snapshot is partial or internally inconsistent")
            snapshot_prepared = True
        self.ready = stage_tools.verify_stage(self.stage)
        if (self.ready["schema"] not in {"bridge_staged_release_v2", "bridge_staged_release_v3"}
                or failed.get("stage") != str(self.stage)
                or failed.get("manifest_sha256") != self.ready["manifest_sha256"]
                or failed.get("binary_sha256") != self.ready["binary_sha256"]):
            raise RuntimeError("failed activation does not bind the requested staged release")
        self.transaction, self.old = transaction, failed["old_identity"]
        self.guard = stage_tools.json_file(self.control / "hold.json")
        if (self.guard.get("schema") != "bridge_launch_hold_v1"
                or self.guard.get("transaction") != str(transaction)):
            raise RuntimeError("stopped recovery does not own the current launch hold")
        checkpoint = failed["drain"].get("checkpoint_sha256", "")
        if not isinstance(checkpoint, str) or not re.fullmatch(r"[0-9a-f]{64}", checkpoint):
            raise ValueError("drained checkpoint evidence is invalid")
        for name, key in (("launchd_spectral_bridge.sh.before", "launcher_sha256"),
                          ("bridge_release_launch.py.before", "selection_helper_sha256")):
            if key in self.old and digest(transaction / name) != self.old[key]:
                raise RuntimeError("original launch helper backup changed")
        pending = self.workspace / "self_control_v2/astrid/deployment_handoff.pending.json"
        pending_expected = snapshot_prepared and not targets_binary
        if os.path.lexists(pending) != pending_expected:
            raise RuntimeError("signed handoff presence does not match the stopped snapshot")
        initial = {"original_failure_sha256":digest(transaction / "receipt.json"),
            "old_pid":expected_pid, "old_identity":self.old, "owned_hold":self.guard,
            "checkpoint_sha256":checkpoint,
            "self_control_sha256":digest(self.workspace / "self_control_v2/astrid/state.json"),
            "runtime_action_feedback":runtime_feedback_descriptor(self.workspace / RUNTIME_FEEDBACK_FILE),
            "snapshot_prepared":snapshot_prepared}
        self.assert_stopped_ownership(initial)
        check = self.native("--verify-deployment-inputs")
        if check.get("checkpoint", {}).get("sha256") != checkpoint:
            raise RuntimeError("native checkpoint does not match the acknowledged drain")
        runtime_feedback_binding(check["checkpoint"], initial["runtime_action_feedback"])
        return initial

    def assert_stopped_ownership(self, initial: dict, *, selected: bool = False) -> None:
        self.verify_bundle(allow_recovery_tool_changes=True)
        if digest(self.transaction / "receipt.json") != initial["original_failure_sha256"]:
            raise RuntimeError("original failed receipt changed during stopped recovery")
        try:
            os.kill(initial["old_pid"], 0)
        except ProcessLookupError:
            pass
        else:
            raise RuntimeError("original PID still exists; no stopped transition is admitted")
        self.verify_launch_configuration(None)
        if stage_tools.json_file(self.control / "hold.json") != initial["owned_hold"]:
            raise RuntimeError("foreign launch hold encountered during stopped recovery")
        old = initial["old_identity"]
        if digest(Path(old["binary"])) != old["binary_sha256"]:
            raise RuntimeError("original executable changed")
        if digest(self.canonical_manifest) != old["manifest_sha256"]:
            raise RuntimeError("original canonical manifest changed")
        artifact = stage_tools.json_file(self.canonical_manifest)["artifacts"]["spectral-bridge"]
        if artifact.get("path") != old["binary"] or artifact.get("sha256") != old["binary_sha256"]:
            raise RuntimeError("original manifest no longer binds the old executable")
        path = self.control / "active.json"
        if selected:
            expected = {"schema":"bridge_release_selection_v1", "stage":str(self.stage),
                "manifest_sha256":self.ready["manifest_sha256"], "transaction":str(self.transaction)}
            if stage_tools.json_file(path) != expected:
                raise RuntimeError("selected release changed during stopped recovery")
        elif (os.path.lexists(path) != (old.get("selection_sha256") is not None)
                or path.exists() and digest(path) != old["selection_sha256"]):
            raise RuntimeError("original release selection changed")
        for name, path in (("release-launcher", self.launcher), ("release-selection", self.selection_helper)):
            if digest(path) != digest(self.stage / stage_tools.ARTIFACTS[name]):
                raise RuntimeError("staged launch helper changed during stopped recovery")
        if digest(self.workspace / "state.json") != initial["checkpoint_sha256"]:
            raise RuntimeError("conversation checkpoint changed after acknowledged drain")
        if digest(self.workspace / "self_control_v2/astrid/state.json") != initial["self_control_sha256"]:
            raise RuntimeError("stopped self-control state changed")
        runtime_feedback_binding(initial, runtime_feedback_descriptor(self.workspace / RUNTIME_FEEDBACK_FILE))

    def validate_stopped_snapshot(self, initial: dict) -> None:
        before = stage_tools.json_file(self.transaction / "inputs.before.json")
        if (digest(self.transaction / "conversation.before.json") != initial["checkpoint_sha256"]
                or before.get("checkpoint", {}).get("sha256") != initial["checkpoint_sha256"]
                or digest(self.transaction / "self-control.before.json") != initial["self_control_sha256"]
                or digest(self.transaction / "manifest.before.json") != self.old["manifest_sha256"]):
            raise RuntimeError("stopped snapshot does not match admitted continuity evidence")
        saved = verify_runtime_feedback_snapshot(before["checkpoint"], self.transaction,
                                                  self.workspace / RUNTIME_FEEDBACK_FILE)
        runtime_feedback_binding(initial, saved)
        if before["self_control"]["state_targets_this_binary"] is not True:
            handoff = stage_tools.json_file(self.transaction / "handoff.json")
            pending = stage_tools.json_file(self.workspace / "self_control_v2/astrid/deployment_handoff.pending.json")
            if (handoff != pending or handoff.get("target_manifest_sha256") != self.ready["manifest_sha256"]
                    or handoff.get("target_binary_sha256") != self.ready["binary_sha256"]):
                raise RuntimeError("signed handoff does not bind the selected release")

    def stopped_release_intent(self, transaction: Path, failure_sha: str, failed: dict) -> dict | None:
        """Allow the existing strict verifier to examine a started recovery only."""
        if failed.get("activation_performed") is not False:
            return None
        history = transaction / "stopped-transition-recoveries"
        if not history.exists():
            return None
        if history.is_symlink() or not history.is_dir():
            raise RuntimeError("invalid stopped recovery history")
        paths = list(history.glob("*/receipt.json"))
        if len(paths) != 1:
            raise RuntimeError("stopped recovery history is not a single bounded attempt")
        path = paths[0]
        if path.resolve(strict=True) != path or not re.fullmatch(r"[0-9a-f]{32}", path.parent.name):
            raise RuntimeError("stopped recovery witness is not canonical")
        record = stage_tools.json_file(path)
        valid = (record.get("schema") == "bridge_stopped_transition_recovery_v1"
            and record.get("original_transaction") == str(transaction)
            and record.get("original_failure_sha256") == failure_sha
            and record.get("old_pid") == failed.get("old_pid")
            and record.get("old_identity") == failed.get("old_identity")
            and record.get("checkpoint_sha256") == failed.get("drain", {}).get("checkpoint_sha256")
            and record.get("owned_hold", {}).get("transaction") == str(transaction)
            and record.get("owned_hold", {}).get("schema") == "bridge_launch_hold_v1"
            and record.get("stage") == str(self.stage)
            and record.get("manifest_sha256") == failed.get("manifest_sha256")
            and record.get("binary_sha256") == failed.get("binary_sha256")
            and record.get("old_process_exited") is True
            and record.get("signal_sent") is False and record.get("drain_requested") is False
            and record.get("force_used") is False and record.get("automatic_rollback") is False
            and record.get("release_intent") is True)
        return {"owned_hold":record["owned_hold"], "path":str(path), "sha256":digest(path)} if valid else None
