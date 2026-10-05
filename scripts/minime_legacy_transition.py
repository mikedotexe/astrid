#!/usr/bin/env python3
"""Agent-only first NEXT handoff transition through the sanctioned reload wrapper.

The old runtime cannot author a new-protocol seal. Its completed drain and exact
stopped queue are recorded as operator observations, then acknowledged by the
replacement before scheduling. Only four reviewed maintenance sources change.
No provider, bridge, engine, model, sensory or launch-configuration change.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import fcntl
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import time
import uuid

import bridge_stage
from bridge_activate import atomic_bytes
import minime_next_handoff as proof
from paired_minime_handoff import AgentOnlyHandoff, sync_parent
from qualify_geometry_release import inventory
from reconcile_minime_launch import checked_copy
from restart_minime_agent import LaunchdAgent, ROOT, reload_agent

SCHEMA = "minime_legacy_next_transition_v1"
PATHS = ("minime_autonomy/runtime.py", "minime_autonomy/collaboration_attention.py",
         "minime_autonomy/maintenance_next.py", "minime_autonomy/maintenance_next_legacy.py")
PENDING_KEYS = ("pending_next_action", "pending_next_selection_id", "choice_envelope_v1",
                "pending_next_action_status", "pending_next_action_updated_at",
                "pending_next_action_update_reason")


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def bounded_read(path, maximum=8 * 1024 * 1024):
    path = Path(path)
    require(path.is_absolute() and ".." not in path.parts
            and not any(p.is_symlink() for p in (path, *path.parents)), "unsafe transition path")
    return proof.read_owned(path, maximum)


def read(path):
    value = json.loads(bounded_read(path), object_pairs_hook=proof.unique,
                      parse_constant=lambda _: require(False, "nonfinite transition field"))
    require(isinstance(value, dict), "transition object required")
    return value


def sha(path):
    return hashlib.sha256(bounded_read(path)).hexdigest()


def immutable(path, value):
    raw = proof.canonical(value) + b"\n"
    require(len(raw) <= 4 * 1024 * 1024, "transition receipt capacity")
    if os.path.lexists(path):
        require(bounded_read(path) == raw, "conflicting transition receipt; retained")
        with path.open("rb") as stream:
            os.fsync(stream.fileno())
    else:
        with path.open("xb") as stream:
            os.chmod(path, 0o600)
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
    sync_parent(path)


def tools_identity():
    root = Path(__file__).resolve().parent.parent
    names = set((*bridge_stage.TOOLS, *getattr(bridge_stage, "OPTIONAL_TOOLS", ()),
        *("scripts/" + name for name in ("minime_legacy_transition.py", "minime_next_handoff.py",
        "research_release_compatibility.py", "restart_minime_agent.py", "paired_minime_handoff.py",
        "qualify_geometry_release.py", "reconcile_minime_launch.py"))))
    return {str(root / name): sha(root / name) for name in sorted(names)
            if name.endswith(".py") and (root / name).exists()}


def prepare(candidate, directory, *, expected_pid, pause_generation, stage, ack, backend=None):
    backend = backend or LaunchdAgent(pause_generation, ack)
    backend.validate()
    root, candidate, directory = backend.root, candidate.resolve(strict=True), directory.absolute()
    require(candidate != root and not directory.exists()
            and directory.parent.resolve() == directory.parent, "new canonical transaction required")
    require(not directory.is_relative_to(root) and not directory.is_relative_to(candidate),
            "transaction must be outside input trees")
    before, proposed = inventory(root), inventory(candidate)
    changed = {p for p in before.keys() | proposed.keys() if before.get(p) != proposed.get(p)}
    require(changed <= set(PATHS) and all(p in proposed for p in PATHS),
            "candidate is not maintenance-only")
    status = read(root / "workspace/runtime/autonomous_agent_source_status.json")
    require(status.get("pid") == expected_pid == backend.pid()
            and status.get("source_inputs_at_start") == before
            and status.get("reload_required") is False
            and status.get("lifecycle_contract") == "agent_drain_v1"
            and status.get("next_handoff_contract") is None, "running legacy source mismatch")
    require(not any(os.path.lexists(p) for p in (*proof.legacy_paths(root / "workspace"),
            proof.history_path(root / "workspace"), proof.admission_path(root / "workspace"))),
            "existing NEXT recovery state requires separate review")
    ready = bridge_stage.verify_stage(stage)
    directory.mkdir(mode=0o700)
    sync_parent(directory)
    source, backup = directory / "source", directory / "source-before"
    source.mkdir(mode=0o700)
    backup.mkdir(mode=0o700)
    for path, digest in proposed.items():
        checked_copy(candidate / path, source / path, digest)
    for path in PATHS:
        if path in before:
            checked_copy(root / path, backup / path, before[path])
    require(inventory(root) == before and inventory(source) == proposed,
            "source changed during maintenance snapshot")
    plan = {"schema": SCHEMA, "owner": uuid.uuid4().hex, "root": str(root),
            "before": before, "after": proposed, "old_pid": expected_pid,
            "old_started_at": backend.identity(expected_pid), "old_status_started_at": status["started_at"],
            "protected": backend.protected(), "config": backend.config(), "tools": tools_identity(),
            "stage": str(stage), "stage_ready": ready, "pause_generation": pause_generation,
            "existing_interrupted_jobs": [row["job_id"] for row in backend.jobs()["jobs"]
                if row.get("error") == "worker_restarted_before_completion"],
            "ack_sha256": hashlib.sha256(ack.encode()).hexdigest()}
    require(plan["old_started_at"], "legacy process start unavailable")
    immutable(directory / "manifest.json", plan)
    return plan


class LegacyHandoff(AgentOnlyHandoff):
    def __init__(self, directory, ack):
        self.directory = Path(directory).resolve(strict=True)
        self.plan = read(self.directory / "manifest.json")
        self.plan_sha256 = sha(self.directory / "manifest.json")
        require(self.plan.get("schema") == SCHEMA and self.plan["tools"] == tools_identity(),
                "transition manifest or wrapper source changed")
        super().__init__(Path(self.plan["stage"]), self.directory / "events.jsonl", ack)
        require(self.ready == self.plan["stage_ready"], "retained bridge stage changed")
        self.guard.update(id=self.plan["owner"], mode="legacy_next_transition")
        self.replacement_inputs = self.plan["after"]

    def stopped(self, backend):
        self.assert_hold()
        backend.maintenance()
        require(sha(self.directory / "manifest.json") == self.plan_sha256
                and tools_identity() == self.plan["tools"], "transition source or manifest drift")
        require(backend.identity(self.plan["old_pid"]) is None, "legacy process remains or PID was reused")
        inventory_result = subprocess.run(["/bin/ps", "-axo", "pid=,ppid="],
            capture_output=True, text=True, timeout=10, check=False)
        processes = [row.split() for row in inventory_result.stdout.splitlines()]
        require(inventory_result.returncode == 0 and not inventory_result.stderr.strip() and processes
                and all(len(row) == 2 and all(v.isdigit() for v in row) for row in processes)
                and str(self.plan["old_pid"]) not in {row[0] for row in processes},
                "old process absence not verified")
        replacement = backend.pid()
        if replacement is not None:
            command = subprocess.run(["/bin/ps", "-p", str(replacement), "-o", "command="],
                capture_output=True, text=True, timeout=10, check=False)
            require(command.returncode == 0 and not command.stderr.strip()
                    and command.stdout.strip() == f"/bin/bash {backend.root / 'scripts/launchd_autonomous_agent.sh'}",
                    "unexpected replacement during legacy hold")
        require(backend.protected() == self.plan["protected"] and backend.config() == self.plan["config"],
                "protected process or configuration changed")
        status = read(backend.root / "workspace/runtime/autonomous_agent_source_status.json")
        require(status.get("pid") == self.plan["old_pid"]
                and status.get("started_at") == self.plan["old_status_started_at"]
                and status.get("source_inputs_at_start") == self.plan["before"]
                and status.get("lifecycle_contract") == "agent_drain_v1"
                and status.get("lifecycle_phase") == "exited" and status.get("reason") == "drained"
                and status.get("next_handoff_contract") is None,
                "exact legacy completed-drain observation absent")
        jobs = backend.jobs()
        require(not jobs["active"], "legacy jobs remain")
        require(not any(row.get("error") == "worker_restarted_before_completion"
                        and row["job_id"] not in self.plan.get("existing_interrupted_jobs", [])
                        for row in jobs["jobs"]), "new interrupted legacy job requires review")
        override = backend.root / "workspace/runtime/pending_next_override.json"
        if os.path.lexists(override):
            row = read(override)
            require(row.get("active") is False and row.get("terminal") is True
                    and row.get("status") in {"blocked", "consumed", "expired"}
                    and row.get("pending_next_action") is None,
                    "pending operator override requires separate review")
        return status

    def import_body(self, backend, status):
        path = backend.root / "workspace/sovereignty_state.json"
        raw = bounded_read(path, 4 * 1024 * 1024)
        state = read(path)
        action = state.get("pending_next_action")
        require(action is None or isinstance(action, str) and bool(action), "invalid stopped NEXT")
        require(not action or state.get("pending_next_action_status", "pending") == "pending",
                "stopped NEXT is not pending")
        require(action or (state.get("pending_next_selection_id") is None
                           and state.get("choice_envelope_v1") is None), "orphan NEXT metadata")
        choice = {key: state[key] for key in PENDING_KEYS if key in state} if action else {}
        return {"schema": "minime_next_legacy_import_v1", "version": 1, "owner": self.plan["owner"],
                "old_process": {"pid": self.plan["old_pid"], "started_at": self.plan["old_status_started_at"],
                                "source_sha256": proof.sha(self.plan["before"])},
                "old_status_sha256": proof.sha(status), "expected_source_sha256": proof.sha(self.plan["after"]),
                "state_sha256": hashlib.sha256(raw).hexdigest(), "pending_sha256": proof.sha(choice),
                "pending_present": bool(choice), "runtime_seal_available": False,
                "execution_confirmed": False}

    def transition(self, backend, protected, emit):
        with queue_transaction(backend.root / "workspace"):
            return self.transition_locked(backend, protected, emit)

    def transition_locked(self, backend, protected, emit):
        require(protected == self.plan["protected"], "protected identity differs from preparation")
        status = self.stopped(backend)
        imported = self.import_body(backend, status)
        immutable(self.directory / "stopped-import.json", imported)
        # Retry may finish an exact partial installation, but never reverse it.
        source = self.directory / "source"
        require(inventory(source) == self.plan["after"], "frozen maintenance release changed")
        expected = backend.inputs()
        for name in expected.keys() | self.plan["before"].keys() | self.plan["after"].keys():
            require(expected.get(name) in {self.plan["before"].get(name), self.plan["after"].get(name)},
                    "foreign source change during maintenance")
        require(all(expected.get(name) == self.plan["before"].get(name)
                    for name in expected.keys() | self.plan["before"].keys() if name not in PATHS),
                "unrelated source change during maintenance")
        for name in PATHS:
            self.stopped(backend)
            require(backend.inputs() == expected, "source changed during maintenance installation")
            if expected.get(name) == self.plan["after"][name]:
                continue
            emit({"phase": "maintenance_source_install_intent", "path": name,
                  "before": expected.get(name), "after": self.plan["after"][name]})
            target = backend.root / name
            require(target.resolve() == target and sha(source / name) == self.plan["after"][name],
                    "maintenance source path/hash changed")
            atomic_bytes(target, bounded_read(source / name), 0o644)
            expected[name] = self.plan["after"][name]
        require(backend.inputs() == self.plan["after"], "maintenance installation incomplete")
        require(self.import_body(backend, self.stopped(backend)) == imported, "stopped queue changed")
        imported_path, ack_path = proof.legacy_paths(backend.root / "workspace")
        require(not os.path.lexists(ack_path) and not os.path.lexists(proof.history_path(backend.root / "workspace"))
                and not os.path.lexists(proof.admission_path(backend.root / "workspace")),
                "unexpected NEXT history while legacy process stopped")
        immutable(imported_path, imported)
        from research_release_compatibility import verify_minime
        verify_minime(backend.root, backend.root / "workspace")
        self.stopped(backend)
        require(self.import_body(backend, self.stopped(backend)) == imported,
                "stopped queue drift before release")
        emit({"phase": "legacy_import_ready", "state_sha256": imported["state_sha256"],
              "pending_present": imported["pending_present"], "runtime_seal_available": False})
        immutable(self.directory / "release-intent.json", {"owner": self.plan["owner"],
                  "import_sha256": sha(imported_path)})
        self.assert_hold()
        self.hold.unlink()
        sync_parent(self.hold)
        self.owned = False
        emit({"phase": "maintenance_replacement_released"})
        return protected

    def verify_restoration(self, backend, pid):
        status = read(backend.root / "workspace/runtime/autonomous_agent_source_status.json")
        require(status.get("pid") == pid, "replacement PID changed")
        return proof.verify_legacy_restore(backend.root / "workspace",
            read(self.directory / "stopped-import.json"), status, self.plan["after"])


@contextmanager
def queue_transaction(workspace):
    path = workspace / ".sovereignty-state.lock"
    require(not any(p.is_symlink() for p in (path, *path.parents)), "linked queue lock")
    fd = os.open(path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    try:
        info = os.fstat(fd)
        require(stat.S_ISREG(info.st_mode) and info.st_uid == os.getuid() and info.st_nlink == 1,
                "invalid queue transaction lock")
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        require((path.stat().st_dev, path.stat().st_ino) == (info.st_dev, info.st_ino),
                "queue transaction lock changed")
        yield
    finally:
        os.close(fd)


@contextmanager
def exclusive(directory):
    info = directory.stat()
    require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid() and not info.st_mode & 0o077,
            "private owned transition directory required")
    fd = os.open(directory / "operation.lock", os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        yield
    finally:
        os.close(fd)


def activate(directory, *, ack, recover=False, timeout=900):
    directory = directory.resolve(strict=True)
    with exclusive(directory):
        handoff = LegacyHandoff(directory, ack)
        plan = handoff.plan
        require(plan["root"] == str(ROOT), "fixed canonical Minime root required")
        backend = LaunchdAgent(plan["pause_generation"], ack)
        events = directory / "events.jsonl"
        if os.path.lexists(events):
            raw = bounded_read(events)
            require(not raw or raw.endswith(b"\n"), "interrupted transition event history; retained")
            for line in raw.splitlines():
                require(isinstance(json.loads(line, object_pairs_hook=proof.unique), dict),
                        "corrupt transition event history; retained")
        with (directory / "events.jsonl").open("ab") as stream:
            os.chmod(events, 0o600)
            def emit(event):
                raw = proof.canonical(event) + b"\n"
                require(stream.tell() + len(raw) <= 8 * 1024 * 1024, "transition event capacity")
                stream.write(raw)
                stream.flush()
                os.fsync(stream.fileno())
                sync_parent(directory / "events.jsonl")
                print(json.dumps({key: value for key, value in event.items() if key in
                                  {"phase", "old_pid", "new_pid", "outcome"}}), flush=True)
            if not recover:
                require(not (directory / "started.json").exists(), "transition already started; use explicit recovery")
                require(backend.inputs() == plan["before"] and backend.protected() == plan["protected"]
                        and backend.config() == plan["config"]
                        and backend.pid() == plan["old_pid"]
                        and backend.identity(plan["old_pid"]) == plan["old_started_at"], "legacy preparation drift")
                immutable(directory / "started.json", {"owner": plan["owner"]})
                result = reload_agent(backend, plan["old_pid"], timeout_s=timeout,
                    expected_inputs=plan["before"], handoff=handoff, emit=emit)
            else:
                require((directory / "started.json").exists(), "no started transition to recover")
                backend.maintenance()
                require(backend.identity(plan["old_pid"]) is None, "recovery never re-signals surviving old process")
                handoff.hold = backend.root / "workspace/runtime/agent-launch-hold.json"
                if os.path.lexists(handoff.hold):
                    require(not (directory / "release-intent.json").exists(), "ambiguous release requires operator review")
                    handoff.owned = True
                    handoff.transition(backend, plan["protected"], emit)
                require((directory / "release-intent.json").exists(), "release intent missing")
                deadline = time.monotonic() + timeout
                while time.monotonic() < deadline:
                    backend.maintenance()
                    require(backend.protected() == plan["protected"] and backend.config() == plan["config"]
                            and backend.inputs() == plan["after"], "recovery boundary drift")
                    pid = backend.pid()
                    if pid and pid != plan["old_pid"] and backend.ready(pid, plan["after"]):
                        result = {"outcome": "success", "new_pid": pid,
                                  "legacy_next_handoff": handoff.verify_restoration(backend, pid)}
                        break
                    time.sleep(1)
                else:
                    raise RuntimeError("replacement readiness timeout; no forced fallback")
            immutable(directory / "completed.json", result)
            emit({"phase": "maintenance_transition_verified", "new_pid": result["new_pid"], "outcome": "success"})
            return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("prepare", "activate", "recover"))
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--ack", required=True)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--expected-pid", type=int)
    parser.add_argument("--pause-generation", type=int)
    parser.add_argument("--bridge-stage", type=Path)
    args = parser.parse_args()
    require(0 < len(args.ack.strip()) <= 2000, "bounded explicit acknowledgement required")
    if args.operation == "prepare":
        require(all(value is not None for value in (args.candidate, args.expected_pid,
                args.pause_generation, args.bridge_stage)), "preparation inputs required")
        plan = prepare(args.candidate, args.directory, expected_pid=args.expected_pid,
                       pause_generation=args.pause_generation, stage=args.bridge_stage, ack=args.ack)
        print(json.dumps({"prepared": True, "owner": plan["owner"], "live_changed": False}))
    else:
        activate(args.directory, ack=args.ack, recover=args.operation == "recover")


if __name__ == "__main__":
    main()
