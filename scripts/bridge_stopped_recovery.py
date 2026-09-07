"""One bounded continuation of an acknowledged, stopped bridge transition.

Partial continuation is never replayed. A running replacement can instead use
the separate verification-only path after durable release intent was recorded.
"""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import re

import bridge_stage as stage_tools
from bridge_release_launch import digest


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
        backend.snapshot_and_handoff(actor, ack)
        backend.assert_stopped_ownership(initial)
        backend.validate_stopped_snapshot(initial)
        witness["status"] = "snapshot_and_handoff_verified"
        backend.record_recovery(witness)
        backend.select_release()
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
        allowed = {"receipt.json", "launchd_spectral_bridge.sh.before", "bridge_release_launch.py.before"}
        if any(path.name not in allowed for path in transaction.iterdir()):
            raise RuntimeError("transition already has progress; do not replay, use verification recovery if running")
        self.ready = stage_tools.verify_stage(self.stage)
        if (self.ready["schema"] != "bridge_staged_release_v2"
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
        if os.path.lexists(pending):
            raise RuntimeError("an existing signed handoff requires separate review")
        initial = {"original_failure_sha256":digest(transaction / "receipt.json"),
            "old_pid":expected_pid, "old_identity":self.old, "owned_hold":self.guard,
            "checkpoint_sha256":checkpoint,
            "self_control_sha256":digest(self.workspace / "self_control_v2/astrid/state.json")}
        self.assert_stopped_ownership(initial)
        check = self.native("--verify-deployment-inputs")
        if check.get("checkpoint", {}).get("sha256") != checkpoint:
            raise RuntimeError("native checkpoint does not match the acknowledged drain")
        return initial

    def assert_stopped_ownership(self, initial: dict, *, selected: bool = False) -> None:
        self.verify_bundle()
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

    def validate_stopped_snapshot(self, initial: dict) -> None:
        before = stage_tools.json_file(self.transaction / "inputs.before.json")
        if (digest(self.transaction / "conversation.before.json") != initial["checkpoint_sha256"]
                or before.get("checkpoint", {}).get("sha256") != initial["checkpoint_sha256"]
                or digest(self.transaction / "self-control.before.json") != initial["self_control_sha256"]
                or digest(self.transaction / "manifest.before.json") != self.old["manifest_sha256"]):
            raise RuntimeError("stopped snapshot does not match admitted continuity evidence")
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
