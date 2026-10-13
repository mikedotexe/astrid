#!/usr/bin/env python3
"""Qualified agent-only drain: finish accepted work without waiting for idle.

Only for reviewed, exact-hash runtimes with agent_drain_v2 and native NEXT
sealing. The legacy idle wrapper is unchanged. No cancellation, forced exit,
queue replay, source installation, peer restart or automatic rollback.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import time

from minime_next_handoff import process, read_owned, unique
from paired_minime_handoff import AgentOnlyHandoff
from restart_minime_agent import LaunchdAgent, TERMINAL, qualified_inputs, read_json


class DrainRefusal(RuntimeError):
    """Bounded operator reason authored here, never a worker exception payload."""


def require(condition, reason):
    if not condition:
        raise DrainRefusal(reason)


class DrainAgent(LaunchdAgent):
    def drain_identity(self, expected_pid, running_inputs, *, stopped=False):
        status = read_json(self.root / "workspace/runtime/autonomous_agent_source_status.json")
        require(status.get("lifecycle_contract") == "agent_drain_v2",
                "qualified agent_drain_v2 required; old runtime needs idle-only bootstrap")
        require(status.get("pid") == expected_pid
                and status.get("source_inputs_at_start") == running_inputs,
                "loaded drain implementation differs from qualified running inputs")
        if stopped:
            require(status.get("lifecycle_phase") == "exited" and status.get("reason") == "drained",
                    "old process did not acknowledge completed drain")
        else:
            lock = read_json(self.root / "workspace/runtime/autonomous_agent.lock")
            require(lock.get("pid") == expected_pid
                    and status.get("lifecycle_phase") in {"busy", "idle"}
                    and status.get("reason") in {"loop", "idle"},
                    "running drain process is not an admitted lifecycle")
        return process(status)


def safe_jobs(snapshot):
    """Only bounded identities/statuses, never errors, commands or artifacts."""
    result = {}
    for job in snapshot["jobs"]:
        ident, status = job["job_id"], job["status"]
        require(isinstance(ident, str) and ident.startswith("job_") and len(ident) <= 256
                and status in TERMINAL | {"queued", "running", "cancel_requested"}
                and ident not in result, "invalid or duplicate drain job identity")
        result[ident] = status
    return result


def recover_unsignaled_hold(backend, handoff, receipts, expected_inputs):
    """Claim only this exact owned, never-signalled agent-only failure chain.

    Retain and recheck the validated bytes; never hash a second unvalidated read.
    A signalled/ambiguous failure is deliberately not recoverable by this path.
    """
    require(receipts and len(receipts) <= 8, "bounded prior hold receipts required")
    retained, original, guard = [], None, None
    for index, path in enumerate(receipts):
        path = path.resolve(strict=True)
        raw = read_owned(path, 4 * 1024 * 1024)
        require(raw.endswith(b"\n"), "incomplete prior handoff receipt")
        rows = [json.loads(line, object_pairs_hook=unique) for line in raw.splitlines()]
        allowed = {"waiting_for_idle", "idle_boundary_refused"}
        allowed |= ({"installation_held", "source_install_intent", "source_installed",
                     "installed_waiting_for_unchanged_quiet_window"} if index == 0 else
                    {"owned_unsignaled_hold_recovered"})
        require(rows and all(isinstance(row, dict) for row in rows)
                and all(row.get("phase") in allowed for row in rows[:-1])
                and rows[-1].get("outcome") == "failed"
                and rows[-1].get("owned_hold_retained") is True
                and rows[-1].get("forced_termination") is False
                and not any(row.get("phase") in {
                    "signal_boundary", "signal_sent", "replacement_admission_released",
                    "accepted_work_signal_boundary", "accepted_work_signal_sent"} for row in rows),
                "prior handoff is not an unsignaled owned failure")
        waits = [row for row in rows if row.get("phase") == "waiting_for_idle"]
        require(len(waits) == 1, "ambiguous prior process binding")
        if index == 0:
            holds = [row for row in rows if row.get("phase") == "installation_held"]
            require(len(holds) == 1, "missing original hold ownership")
            original, guard = waits[0], holds[0]["hold"]
            require(guard.get("receipt") == str(path), "original hold receipt differs")
        else:
            links = [row for row in rows if row.get("phase") == "owned_unsignaled_hold_recovered"]
            require(len(links) == 1 and links[0].get("previous_receipt") == str(retained[0][0])
                    and links[0].get("previous_sha256") == retained[0][2],
                    "prior recovery does not reference original hold")
            intermediate = links[0].get("intervening_receipt")
            if index > 1 or intermediate is not None or "intervening_sha256" in links[0]:
                require(index > 1 and intermediate == str(retained[-1][0])
                        and links[0].get("intervening_sha256") == retained[-1][2],
                        "intervening recovery evidence is missing or changed")
            require(all(waits[0].get(key) == original.get(key) for key in
                        ("old_pid", "old_started_at", "source_inputs", "config", "protected")),
                    "prior recovery changed pinned identities")
        retained.append((path, raw, hashlib.sha256(raw).hexdigest()))
    require(guard.get("schema") == "minime_paired_launch_hold_v1"
            and guard.get("mode") == "agent_only"
            and guard.get("stage") == str(handoff.stage)
            and guard.get("manifest_sha256") == handoff.ready["manifest_sha256"],
            "prior hold scope differs")
    backend.validate()
    require(backend.pid() == original["old_pid"]
            and backend.identity(original["old_pid"]) == original["old_started_at"]
            and backend.inputs() == original["source_inputs"] == expected_inputs
            and backend.config() == original["config"]
            and backend.protected() == original["protected"],
            "prior hold source, configuration or process identities changed")
    handoff.hold = backend.root / "workspace/runtime/agent-launch-hold.json"
    handoff.guard = guard
    handoff.assert_hold()
    require(all(read_owned(path, 4 * 1024 * 1024) == raw for path, raw, _ in retained),
            "prior recovery evidence changed")
    handoff.owned = True
    return [{"path": str(path), "sha256": digest} for path, _, digest in retained]


def finish_accepted_work(backend, expected_pid, *, running_inputs, expected_inputs,
                         handoff, timeout_s=900, now=time.monotonic, sleep=time.sleep,
                         emit=lambda event: None):
    require(type(expected_pid) is int and expected_pid > 0 and 30 <= timeout_s <= 1800,
            "invalid bounded drain request")
    require(running_inputs and expected_inputs and handoff is not None,
            "qualified running/replacement inputs and held handoff required")
    backend.validate()
    inputs, config, protected = backend.inputs(), backend.config(), backend.protected()
    require(inputs == expected_inputs, "replacement source differs; no signal sent")
    old_start = backend.identity(expected_pid)
    require(old_start and backend.pid() == expected_pid, "old process changed; no signal sent")
    identity = backend.drain_identity(expected_pid, running_inputs)
    initial = backend.jobs()
    initial_status = safe_jobs(initial)
    accepted_ids = {row["job_id"] for row in initial["active"]}
    require(not any(row["status"] == "cancel_requested" for row in initial["active"]),
            "cancellation already pending; no signal sent")
    interrupted = {row["job_id"] for row in initial["jobs"]
                   if row.get("error") == "worker_restarted_before_completion"}
    handoff.begin(backend, inputs, config, protected, emit)
    handoff.assert_hold()
    backend.validate()
    require(backend.pid() == expected_pid and backend.identity(expected_pid) == old_start
            and backend.drain_identity(expected_pid, running_inputs) == identity
            and backend.inputs() == inputs and backend.config() == config
            and backend.protected() == protected, "pre-signal boundary changed; hold retained")
    emit({"phase": "accepted_work_signal_boundary", "old_pid": expected_pid,
          "old_started_at": old_start, "running_process": identity,
          "source_inputs": inputs, "config": config, "protected": protected,
          "job_statuses": initial_status, "atomic_traffic_quiescence_claimed": False})
    backend.terminate(expected_pid)
    emit({"phase": "accepted_work_signal_sent", "pid": expected_pid, "signal": "SIGTERM"})
    deadline = now() + timeout_s
    while now() < deadline:
        backend.maintenance()
        handoff.assert_hold()
        require(backend.inputs() == inputs and backend.config() == config
                and backend.protected() == protected, "drain boundary changed; hold retained")
        old = backend.identity(expected_pid)
        if old is None:
            break
        require(old == old_start and backend.pid() == expected_pid,
                "old process identity changed during drain; hold retained")
        sleep(1)
    else:
        raise DrainRefusal("accepted-work drain timed out; hold retained; no forced fallback")

    require(backend.drain_identity(expected_pid, running_inputs, stopped=True) == identity,
            "stopped process identity changed; hold retained")
    sealed = backend.verify_next_seal(identity)
    drained = backend.jobs()
    statuses = safe_jobs(drained)
    require(not drained["active"] and initial_status.keys() <= statuses.keys(),
            "accepted jobs missing or unfinished; hold retained")
    for row in drained["jobs"]:
        ident = row["job_id"]
        require(not (row.get("error") == "worker_restarted_before_completion" and ident not in interrupted),
                "interrupted job detected; hold retained")
        if ident in accepted_ids or ident not in initial_status:
            require(row["status"] != "canceled", "accepted job cancelled; hold retained")
            require(row.get("worker_status") in TERMINAL
                    and row.get("outcome_status") == row["worker_status"]
                    and row.get("outcome_verified") is True,
                    "accepted worker outcome unverified; hold retained")
    emit({"phase": "accepted_work_drained", "old_pid": expected_pid,
          "next_seal": sealed, "job_statuses": statuses,
          "active_jobs": 0, "forced_termination": False})
    # No replacement may inspect/modify the queue until this seal is verified.
    require(handoff.transition(backend, protected, emit) == protected,
            "agent-only transition changed protected identities")
    deadline = now() + timeout_s
    while now() < deadline:
        backend.maintenance()
        require(backend.inputs() == inputs and backend.config() == config
                and backend.protected() == protected, "replacement boundary changed")
        pid = backend.pid()
        if pid and pid != expected_pid and backend.ready(pid, inputs):
            restored = backend.verify_next_handoff(identity, pid, inputs)
            require(restored["seal_sha256"] == sealed["seal_sha256"],
                    "replacement restored a different NEXT seal")
            return {"schema": "minime_finish_accepted_work_v1", "outcome": "success",
                    "old_pid": expected_pid, "old_started_at": old_start,
                    "new_pid": pid, "new_started_at": backend.identity(pid),
                    "source_inputs": inputs, "config": config, "protected": protected,
                    "next_handoff": restored, "signal": "SIGTERM", "forced_termination": False,
                    "job_statuses_at_drain": statuses, "execution_confirmed": False}
        sleep(1)
    raise DrainRefusal("replacement readiness timed out; no forced fallback or queue replay")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--expected-pid", type=int, required=True)
    parser.add_argument("--pause-generation", type=int, required=True)
    parser.add_argument("--qualified-running-inputs", type=Path, required=True)
    parser.add_argument("--expected-inputs", type=Path, required=True)
    parser.add_argument("--bridge-stage", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--recover-unsignaled-receipt", type=Path, action="append", default=[])
    parser.add_argument("--timeout-secs", type=int, default=900)
    parser.add_argument("--ack", required=True)
    args = parser.parse_args()
    if not args.ack.strip():
        parser.error("nonempty explicit reviewed-drain acknowledgement required")
    backend = DrainAgent(args.pause_generation, args.ack)
    handoff = AgentOnlyHandoff(args.bridge_stage, args.receipt, args.ack)
    with args.receipt.open("x") as stream:
        os.chmod(args.receipt, 0o600)
        def emit(event):
            event = {"recorded_at": datetime.now(timezone.utc).isoformat(), **event}
            stream.write(json.dumps(event, sort_keys=True, allow_nan=False) + "\n")
            stream.flush()
            os.fsync(stream.fileno())
            print(json.dumps({key: event[key] for key in
                  ("recorded_at", "phase", "outcome", "old_pid", "new_pid") if key in event}), flush=True)
        try:
            replacement = qualified_inputs(args.expected_inputs)
            running = qualified_inputs(args.qualified_running_inputs)
            if args.recover_unsignaled_receipt:
                bindings = recover_unsignaled_hold(backend, handoff,
                    args.recover_unsignaled_receipt, replacement)
                emit({"phase": "owned_unsignaled_hold_recovered", "receipts": bindings})
            emit(finish_accepted_work(backend, args.expected_pid,
                running_inputs=running, expected_inputs=replacement, handoff=handoff,
                timeout_s=args.timeout_secs, emit=emit))
        except Exception as error:
            emit({"outcome": "failed", "error_type": type(error).__name__,
                  "reason": str(error) if isinstance(error, DrainRefusal) else "external_gate_refused",
                  "owned_hold_retained": handoff.owned, "forced_termination": False})
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
