#!/usr/bin/env python3
"""One native-NEXT v1 -> drain-v2 transition through the unchanged idle gate.

Preparation writes only a new external packet. Activation installs runtime.py
only after old-process exit and a durable native seal. No automatic recovery,
queue import, busy drain, forced fallback, peer restart or source-status repair.
"""
from __future__ import annotations

import argparse
import ast
import hashlib
import json
import os
from pathlib import Path
import re
import sys

# Shared wrappers read the canonical deployment helper, never its runtime.
sys.dont_write_bytecode = True

import bridge_stage
from bridge_activate import atomic_bytes
from finish_minime_work import recover_unsignaled_hold
from minime_legacy_transition import bounded_read, exclusive, immutable, read, sha
from minime_legacy_transition import tools_identity as inherited_tools_identity
from minime_next_handoff import canonical, process, unique
from paired_minime_handoff import AgentOnlyHandoff, sync_parent
from qualify_geometry_release import inventory
from reconcile_minime_launch import OVERLAY, checked_copy
from research_release_compatibility import verify_minime
from restart_minime_agent import LaunchdAgent, reload_agent

SCHEMA = "minime_native_drain_bootstrap_v1"
RUNTIME = "minime_autonomy/runtime.py"
SCRIPTS = Path(__file__).resolve().parent


class BootstrapRefusal(RuntimeError):
    """Static operator reason; never a private exception or worker payload."""


def require(value, reason):
    if not value:
        raise BootstrapRefusal(reason)


def tools_identity():
    return {**inherited_tools_identity(), **{
        str(SCRIPTS / name): sha(SCRIPTS / name) for name in (
            "bootstrap_minime_drain.py", "finish_minime_work.py", "minime_legacy_transition.py",
        )}}


def checked_path(value):
    path = Path(value).absolute()
    require(".." not in path.parts and path.resolve() == path
            and not any(p.is_symlink() for p in (path, *path.parents)), "unsafe_packet_path")
    return path


def inputs(value):
    require(isinstance(value, dict) and 0 < len(value) <= 2000, "invalid_source_inventory")
    for name, digest in value.items():
        require(isinstance(name, str) and not Path(name).is_absolute()
                and all(part not in {"", ".", ".."} for part in name.split("/"))
                and isinstance(digest, str) and re.fullmatch(r"[0-9a-f]{64}", digest),
                "invalid_source_inventory")
    require(RUNTIME in value, "runtime_input_missing")
    return value


def three_inventories(loaded, installed, candidate):
    for value in (loaded, installed, candidate):
        inputs(value)
    require(loaded.keys() == installed.keys() == candidate.keys(), "source_keys_changed")
    for before, after in ((loaded, installed), (installed, candidate)):
        require({name for name in before if before[name] != after[name]} == {RUNTIME},
                "not_runtime_only_transition")


def runtime_contract(path, expected):
    tree = ast.parse(bounded_read(path))
    declarations = [value.value for node in ast.walk(tree) if isinstance(node, ast.Dict)
                    for key, value in zip(node.keys, node.values)
                    if isinstance(key, ast.Constant) and key.value == "lifecycle_contract"
                    and isinstance(value, ast.Constant)]
    require(declarations == [expected], "runtime_contract_mismatch")


def status_identity(backend, loaded, expected_pid):
    status = read(backend.root / "workspace/runtime/autonomous_agent_source_status.json")
    require(status.get("pid") == expected_pid and status.get("source_inputs_at_start") == loaded
            and status.get("source_path") == str(backend.root / RUNTIME)
            and status.get("lifecycle_contract") == "agent_drain_v1", "loaded_v1_identity_changed")
    return status, process(status)


def installation_evidence(path, loaded, installed):
    rows = [json.loads(line, object_pairs_hook=unique) for line in bounded_read(path).splitlines()]
    intents = [row for row in rows if row.get("phase") == "source_install_intent"]
    commits = [row for row in rows if row.get("phase") == "source_installed"]
    launcher = "scripts/launchd_autonomous_agent.sh"
    order = [launcher, *(name for name in OVERLAY if name != launcher)]
    require([row.get("path") for row in intents] == order
            and [row.get("path") for row in commits] == order, "incomplete_prior_installation")
    require(all(row.get("before") == loaded[name] and row.get("after") == installed[name]
                and commit.get("sha256") == installed[name]
                for name, row, commit in zip(order, intents, commits)), "prior_installation_hash_mismatch")


def prepare(directory, *, launch01, launch02, qualification, receipts, expected_pid,
            pause_generation, ack, backend=None):
    require(type(expected_pid) is int and expected_pid > 0 and 0 < len(ack.strip()) <= 2000,
            "invalid_bootstrap_request")
    directory = checked_path(directory)
    require(not os.path.lexists(directory), "packet_already_exists")
    refs = [checked_path(path) for path in (launch01, launch02, qualification, *receipts)]
    require(len(receipts) == 3 and len(set(refs)) == 6, "exact_three_prior_attempts_required")
    hashes = {str(path): sha(path) for path in refs}
    first, second, qualified = (read(path) for path in refs[:3])
    require(first.get("schema") == second.get("schema") == "minime_launch_source_reconciliation_v1"
            and qualified.get("schema") == "minime_drain_paired_qualification_v1"
            and qualified.get("status") == "passed"
            and qualified.get("activation_performed") is False, "unqualified_source_receipt")
    loaded, installed, candidate = (value["selected_inputs"] for value in (first, second, qualified))
    three_inventories(loaded, installed, candidate)
    require(second.get("canonical_inputs") == loaded, "prior_installation_baseline_changed")
    snapshots = {str(checked_path(first["snapshot_root"])): loaded,
                 str(checked_path(second["snapshot_root"])): installed,
                 str(checked_path(qualified["snapshot"])): candidate}
    require(len(snapshots) == 3 and all(inventory(Path(root)) == expected
            for root, expected in snapshots.items()), "snapshot_inventory_changed")
    candidate_root = checked_path(qualified["source"])
    require(inventory(candidate_root) == candidate, "qualified_candidate_changed")
    runtime_contract(Path(first["snapshot_root"]) / RUNTIME, "agent_drain_v1")
    runtime_contract(Path(qualified["snapshot"]) / RUNTIME, "agent_drain_v2")
    backend = backend or LaunchdAgent(pause_generation, ack)
    root = checked_path(backend.root)
    require(all(not directory.is_relative_to(path) for path in (
        root, candidate_root, SCRIPTS.parent, *(Path(p) for p in snapshots))), "packet_inside_source_tree")
    require(all(value.get("bindings", {}).get("canonical_root") == str(root)
                for value in (first, second)), "canonical_binding_changed")
    stage = checked_path(qualified["stage"])
    require(not directory.is_relative_to(stage), "packet_inside_retained_stage")
    handoff = AgentOnlyHandoff(stage, directory / "events.jsonl", ack)
    require(handoff.ready == qualified["stage_identity"], "qualified_stage_changed")
    tools = tools_identity()
    bindings = recover_unsignaled_hold(backend, handoff, refs[3:], installed)
    installation_evidence(refs[3], loaded, installed)
    status, identity = status_identity(backend, loaded, expected_pid)
    require(status.get("lifecycle_phase") in {"busy", "idle"}, "old_process_not_running")
    old_start = backend.identity(expected_pid)
    require(old_start and backend.pid() == expected_pid, "old_process_changed")
    config, protected = backend.config(), backend.protected()
    verify_minime(Path(qualified["snapshot"]), root / "workspace")
    hold_hash = sha(handoff.hold)
    mode = (root / RUNTIME).stat().st_mode & 0o777
    directory.mkdir(mode=0o700)
    sync_parent(directory)
    checked_copy(root / RUNTIME, directory / "runtime.before.py", installed[RUNTIME])
    checked_copy(Path(qualified["snapshot"]) / RUNTIME, directory / "runtime.v2.py", candidate[RUNTIME])
    sync_parent(directory / "runtime.v2.py")
    plan = {"schema": SCHEMA, "root": str(root), "loaded_inputs": loaded,
            "installed_inputs": installed, "candidate_inputs": candidate,
            "old_pid": expected_pid, "old_started_at": old_start, "old_process": identity,
            "config": config, "protected": protected, "pause_generation": pause_generation,
            "references": hashes, "receipts": bindings, "snapshots": snapshots,
            "candidate_root": str(candidate_root), "candidate_snapshot": qualified["snapshot"],
            "hold": handoff.guard, "hold_sha256": hold_hash, "stage": str(stage),
            "stage_identity": handoff.ready, "tools": tools, "runtime_mode": mode,
            "ack_sha256": hashlib.sha256(ack.encode()).hexdigest()}
    require(backend.inputs() == installed and backend.pid() == expected_pid
            and backend.identity(expected_pid) == old_start
            and status_identity(backend, loaded, expected_pid)[1] == identity
            and backend.config() == config and backend.protected() == protected
            and sha(handoff.hold) == hold_hash and tools_identity() == tools
            and all(sha(Path(p)) == digest for p, digest in hashes.items())
            and all(inventory(Path(p)) == expected for p, expected in snapshots.items())
            and inventory(candidate_root) == candidate
            and bridge_stage.verify_stage(stage) == handoff.ready, "preparation_drift")
    handoff.assert_hold()
    immutable(directory / "manifest.json", plan)
    return plan


class BootstrapHandoff(AgentOnlyHandoff):
    def __init__(self, directory, ack):
        self.directory = checked_path(directory)
        self.plan = read(self.directory / "manifest.json")
        self.manifest_sha256 = sha(self.directory / "manifest.json")
        require(self.plan.get("schema") == SCHEMA, "invalid_bootstrap_packet")
        super().__init__(Path(self.plan["stage"]), self.directory / "events.jsonl", ack)
        require(self.ready == self.plan["stage_identity"], "retained_stage_changed")
        self.replacement_inputs = self.plan["candidate_inputs"]
        self.sealed = None

    def evidence(self):
        plan = self.plan
        require(sha(self.directory / "manifest.json") == self.manifest_sha256
                and tools_identity() == plan["tools"]
                and all(sha(Path(p)) == digest for p, digest in plan["references"].items())
                and all(inventory(Path(p)) == expected for p, expected in plan["snapshots"].items())
                and inventory(Path(plan["candidate_root"])) == plan["candidate_inputs"]
                and sha(self.directory / "runtime.before.py") == plan["installed_inputs"][RUNTIME]
                and sha(self.directory / "runtime.v2.py") == plan["candidate_inputs"][RUNTIME]
                and bridge_stage.verify_stage(self.stage) == self.ready, "bootstrap_evidence_changed")

    def boundary(self, backend, expected, *, stopped=False):
        self.evidence()
        backend.maintenance()
        self.assert_hold()
        plan = self.plan
        require(str(checked_path(backend.root)) == plan["root"]
                and self.guard == plan["hold"] and sha(self.hold) == plan["hold_sha256"]
                and backend.inputs() == expected and backend.config() == plan["config"]
                and backend.protected() == plan["protected"], "bootstrap_boundary_changed")
        status, identity = status_identity(backend, plan["loaded_inputs"], plan["old_pid"])
        require(identity == plan["old_process"], "native_process_identity_changed")
        if stopped:
            require(backend.identity(plan["old_pid"]) is None
                    and status.get("lifecycle_phase") == "exited" and status.get("reason") == "drained"
                    and not backend.jobs()["active"], "old_process_not_drained")
            seal = backend.verify_next_seal(plan["old_process"])
            require(self.sealed is None or self.sealed == seal, "native_seal_changed")
            self.sealed = seal
        else:
            require(backend.pid() == plan["old_pid"]
                    and backend.identity(plan["old_pid"]) == plan["old_started_at"], "old_process_changed")

    def begin(self, backend, inputs, config, protected, emit):
        require(inputs == self.plan["installed_inputs"] and config == self.plan["config"]
                and protected == self.plan["protected"], "idle_gate_binding_changed")
        self.boundary(backend, inputs)
        require(backend.observe(self.plan["old_pid"]).get("next_handoff") == self.plan["old_process"],
                "idle_gate_native_identity_changed")
        super().begin(backend, inputs, config, protected, emit)
        self.boundary(backend, inputs)

    def transition(self, backend, protected, emit):
        plan = self.plan
        require(protected == plan["protected"], "protected_binding_changed")
        self.boundary(backend, plan["installed_inputs"], stopped=True)
        verify_minime(Path(plan["candidate_snapshot"]), backend.root / "workspace")
        raw = bounded_read(self.directory / "runtime.v2.py")
        require(hashlib.sha256(raw).hexdigest() == plan["candidate_inputs"][RUNTIME], "candidate_runtime_changed")
        target = checked_path(backend.root / RUNTIME)
        require(bounded_read(target) == bounded_read(self.directory / "runtime.before.py"), "installed_source_changed")
        emit({"phase": "native_seal_verified", "next_handoff": self.sealed})
        emit({"phase": "runtime_install_intent", "before": plan["installed_inputs"][RUNTIME],
              "after": plan["candidate_inputs"][RUNTIME]})
        self.boundary(backend, plan["installed_inputs"], stopped=True)
        atomic_bytes(target, raw, plan["runtime_mode"])
        self.boundary(backend, plan["candidate_inputs"], stopped=True)
        verify_minime(backend.root, backend.root / "workspace")
        emit({"phase": "runtime_installed", "sha256": plan["candidate_inputs"][RUNTIME]})
        self.inputs, self.config = plan["candidate_inputs"], plan["config"]
        self.boundary(backend, self.inputs, stopped=True)
        return super().transition(backend, protected, emit)


def public_event(event):
    # In particular, omit observation.jobs, raw errors and new_jobs_during_rollout.
    allowed = {"phase", "outcome", "old_pid", "new_pid", "pid", "signal", "boundary",
               "forced_termination", "before", "after", "sha256"}
    return {key: value for key, value in event.items() if key in allowed
            and type(value) in (str, int, bool) and (not isinstance(value, str) or len(value) <= 128)}


def activate(directory, *, ack, timeout_s=900, backend=None):
    require(0 < len(ack.strip()) <= 2000 and 30 <= timeout_s <= 1800, "invalid_bootstrap_request")
    directory = checked_path(directory)
    with exclusive(directory):
        handoff = BootstrapHandoff(directory, ack)
        plan = handoff.plan
        require(not any(os.path.lexists(directory / name) for name in ("started.json", "events.jsonl")),
                "bootstrap_already_attempted_no_automatic_recovery")
        backend = backend or LaunchdAgent(plan["pause_generation"], ack)
        handoff.evidence()
        bindings = recover_unsignaled_hold(backend, handoff,
            [Path(row["path"]) for row in plan["receipts"]], plan["installed_inputs"])
        require(bindings == plan["receipts"], "prior_receipts_changed")
        handoff.boundary(backend, plan["installed_inputs"])
        immutable(directory / "started.json", {"manifest_sha256": handoff.manifest_sha256})
        with (directory / "events.jsonl").open("xb") as stream:
            os.chmod(directory / "events.jsonl", 0o600)
            sync_parent(directory / "events.jsonl")

            def emit(event):
                raw = canonical(public_event(event)) + b"\n"
                require(stream.tell() + len(raw) <= 1024 * 1024, "bootstrap_event_capacity")
                stream.write(raw)
                stream.flush()
                os.fsync(stream.fileno())

            try:
                result = reload_agent(backend, plan["old_pid"], timeout_s=timeout_s,
                    expected_inputs=plan["installed_inputs"], handoff=handoff, emit=emit)
                status = read(backend.root / "workspace/runtime/autonomous_agent_source_status.json")
                require(status.get("lifecycle_contract") == "agent_drain_v2"
                        and status.get("source_inputs_at_start") == plan["candidate_inputs"]
                        and status.get("pid") == result["new_pid"], "replacement_v2_not_verified")
                restored = backend.verify_next_handoff(plan["old_process"], result["new_pid"], plan["candidate_inputs"])
                require(handoff.sealed is not None and restored["seal_sha256"] == handoff.sealed["seal_sha256"],
                        "replacement_seal_mismatch")
                receipt = {"schema": SCHEMA, "outcome": "success", "old_pid": plan["old_pid"],
                           "new_pid": result["new_pid"], "manifest_sha256": handoff.manifest_sha256,
                           "source_inputs": plan["candidate_inputs"],
                           "next_handoff": restored, "forced_termination": False, "execution_confirmed": False}
                immutable(directory / "completed.json", receipt)
                emit(receipt)
                return receipt
            except Exception as error:
                failure = {"outcome": "failed", "reason": str(error) if isinstance(error, BootstrapRefusal)
                           else "external_gate_refused", "error_type": type(error).__name__,
                           "owned_hold_retained": handoff.owned, "forced_termination": False}
                immutable(directory / "failure.json", failure)
                emit(failure)
                raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("prepare", "activate"))
    parser.add_argument("--directory", required=True, type=Path)
    parser.add_argument("--ack", required=True)
    for name in ("launch01", "launch02", "qualification"):
        parser.add_argument("--" + name, type=Path)
    parser.add_argument("--receipt", dest="receipts", action="append", type=Path, default=[])
    parser.add_argument("--expected-pid", type=int)
    parser.add_argument("--pause-generation", type=int)
    parser.add_argument("--timeout-secs", type=int, default=900)
    args = parser.parse_args()
    try:
        if args.operation == "prepare":
            require(all(value is not None for value in (args.launch01, args.launch02, args.qualification,
                    args.expected_pid, args.pause_generation)), "preparation_inputs_required")
            prepare(args.directory, launch01=args.launch01, launch02=args.launch02,
                    qualification=args.qualification, receipts=args.receipts, expected_pid=args.expected_pid,
                    pause_generation=args.pause_generation, ack=args.ack)
            print(json.dumps({"outcome": "prepared", "live_changed": False}))
        else:
            print(json.dumps(public_event(activate(args.directory, ack=args.ack, timeout_s=args.timeout_secs))))
        return 0
    except Exception as error:
        print(json.dumps({"outcome": "failed", "reason": str(error) if isinstance(error, BootstrapRefusal)
                         else "external_gate_refused", "error_type": type(error).__name__}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
