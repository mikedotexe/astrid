#!/usr/bin/env python3
"""Reload only Minime's existing Python launchd job, with an observed idle gate.

The legacy agent has no atomic drain handshake. This wrapper does not claim one:
it requires a quiet mid-cycle window, no active jobs or TCP connections, and
revalidates immediately before one SIGTERM. It never bootouts, forces, cancels
work, edits NEXT, changes launchd settings, or touches another service.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import plistlib
import re
import subprocess
import sys
import time

from deploy_preflight import COMPONENTS, preflight
from steward_control.config import load_config

ROOT = Path("/Users/v/other/minime")
sys.path.insert(0, str(ROOT))
from minime_autonomy.deployment import source_inputs  # noqa: E402

LABEL = "com.minime.autonomous-agent"
PROTECTED = (
    "com.minime.engine", "com.minime.division-gateway",
    "com.minime.division-supervisor", "com.reservoir.coupled-astrid",
    "com.astrid.spectral-bridge",
    "com.minime.mic-to-sensory", "com.minime.camera-client",
    "com.minime.visual-frame-service", "com.minime.host-sensory",
    "com.reservoir.astrid-feeder",
)
ENV_KEYS = (
    "MINIME_MODEL", "MINIME_FALLBACK_MODEL", "MINIME_LLM_BACKEND",
    "MINIME_LLM_TIMEOUT_S", "MINIME_LLM_FALLBACK_TIMEOUT_S",
    "MINIME_LLM_COMPACT_TIMEOUT_S", "MINIME_LLM_COMPACT_FALLBACK_TIMEOUT_S",
    "MINIME_OLLAMA_NUM_CTX", "MINIME_OLLAMA_NUM_PREDICT_CAP",
    "MINIME_OLLAMA_FALLBACK_NUM_CTX", "MINIME_OLLAMA_FALLBACK_NUM_PREDICT_CAP",
    "MINIME_OLLAMA_WRITING_RELEASE", "MINIME_OLLAMA_WRITING_RELEASE_SHA256",
    "AGENT_INTERVAL", "FOCUSED_MODE", "PROVIDER_INPUT_RECEIPT_PIN_V1",
    "MINIME_PUBLIC_WEB_HELPER", "MINIME_PUBLIC_WEB_HELPER_SHA256", "MINIME_PUBLIC_WEB_CONFIG",
)
TERMINAL = {"completed", "thin_output", "timeout", "failed", "canceled", "blocked"}


def read_json(path):
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        raise ValueError(f"not an object: {path}")
    return value


def qualified_inputs(path):
    value = read_json(path)
    inputs = value.get("selected_inputs")
    if (value.get("schema") != "minime_launch_source_reconciliation_v1"
            or not isinstance(inputs, dict) or not inputs or len(inputs) > 2000
            or any(not isinstance(key, str) or not isinstance(digest, str)
                   or not re.fullmatch(r"[0-9a-f]{64}", digest) for key, digest in inputs.items())):
        raise ValueError("invalid qualified launch-source receipt")
    return inputs


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


def idle_diagnostic(before, after, *, elapsed_s=0):
    """Describe failed comparisons, never serialize snapshot values or prose."""
    changed = []
    for field in ("quiet", "source_checked_at", "job_index_sha256", "continuity", "next_handoff"):
        if before and before.get(field) != after.get(field):
            changed.append(field)
    for parent, fields in (
        ("continuity", ("file_sha256", "session_id", "pending_next_sha256",
                        "pending_next_present", "cycle_count")),
        ("next_handoff", ("boot_id", "pid", "started_at", "source_sha256")),
    ):
        left, right = before.get(parent), after.get(parent)
        if isinstance(left, dict) and isinstance(right, dict):
            changed.extend(f"{parent}.{key}" for key in fields if left.get(key) != right.get(key))

    def snapshot(value):
        checks = value.get("quiet_checks", {})
        result = {"quiet": value.get("quiet") if type(value.get("quiet")) is bool else None,
                  "checks": {key: checks.get(key) if type(checks.get(key)) is bool else None
                             for key in ("in_idle_window", "no_tcp_connections",
                                         "indexed_jobs_idle", "summary_jobs_idle")}}
        phase = value.get("lifecycle_phase")
        result["lifecycle_phase"] = (phase if phase in (
            "initializing", "busy", "idle", "draining", "exited", "recovery_blocked") else "unknown")
        contract = value.get("lifecycle_contract")
        result["idle_contract"] = (contract if contract in (
            "agent_drain_v1", "agent_drain_v2", "legacy_no_handshake") else "unknown")
        for key in ("source_status_age_s", "agent_interval_s"):
            number = value.get(key)
            result[key] = (number if type(number) in (int, float)
                           and math.isfinite(number) and abs(number) <= 1e9 else None)
        return result

    return {"schema": "minime_idle_boundary_diagnostic_v1", "changed_fields": changed,
            "comparison_available": bool(before),
            "before": snapshot(before), "after": snapshot(after),
            "boundary_check_elapsed_s": round(elapsed_s, 3),
            "signal_sent": False, "atomic_traffic_quiescence_claimed": False}


def run(args):
    return subprocess.run(args, capture_output=True, text=True, timeout=10)


def job_receipt(root, *, full=False):
    """Read canonical job metadata without the store's recovery/expiry writes."""
    jobs = root / "workspace/llm_jobs"
    index = read_json(jobs / "index.json")
    ids = set(index["recent_jobs"])
    for key in ("active_primary_job_id", "active_background_job_id"):
        if index.get(key):
            ids.add(index[key])
    if full:
        ids.update(p.parent.name for p in (jobs / "jobs").glob("*/job.json"))
    rows = []
    for ident in sorted(ids):
        if not isinstance(ident, str) or not re.fullmatch(r"job_[A-Za-z0-9_-]+", ident):
            raise ValueError("invalid job id")
        job = read_json(jobs / "jobs" / ident / "job.json")
        status = job.get("status")
        if status not in TERMINAL | {"queued", "running", "cancel_requested"}:
            raise ValueError("unrecognized job status")
        row = {k: job.get(k) for k in (
            "job_id", "status", "worker_pid", "action_id", "finished_at", "error",
            "worker_status", "completed_at",
        )}
        row.update(job_outcome_proof(jobs / "jobs" / ident, job))
        rows.append(row)
    active = [r for r in rows if r["status"] not in TERMINAL
              or r.get("worker_status") in {"queued", "running"}]
    return {"active": active, "index_sha256": digest(index), "jobs": rows}


def job_outcome_proof(directory, job):
    """Verify terminal worker evidence without returning its private content."""
    outcome = job.get("outcome")
    proof = {"outcome_status": None, "outcome_verified": False}
    if not isinstance(outcome, dict) or outcome.get("status") not in TERMINAL:
        return proof
    proof["outcome_status"] = outcome["status"]
    evidence = {key: value for key, value in outcome.items() if key != "result_path"}
    try:
        raw = json.dumps(evidence, sort_keys=True, allow_nan=False).encode()
        if hashlib.sha256(raw).hexdigest() != job.get("outcome_sha256"):
            return proof
        retained = outcome.get("result_path")
        if retained is not None:
            path = Path(retained)
            allowed = {directory / "result.txt", directory / "late_result.txt"}
            if path not in allowed or path.resolve() != path:
                return proof
            from minime_next_handoff import read_owned
            result = read_owned(path, 64 * 1024 * 1024)
            if hashlib.sha256(result).hexdigest() != outcome.get("result_sha256"):
                return proof
        elif outcome.get("result_sha256") is not None:
            return proof
        proof["outcome_verified"] = (job.get("worker_status") == outcome["status"]
                                     and bool(job.get("completed_at")))
    except (OSError, ValueError, RuntimeError, TypeError):
        pass
    return proof


class LaunchdAgent:
    def __init__(self, pause_generation, ack):
        self.pause_generation = pause_generation
        self.ack = ack
        self.root = ROOT
        self.domain = f"gui/{os.getuid()}"

    def job(self, label=LABEL):
        result = run(["launchctl", "print", f"{self.domain}/{label}"])
        if result.returncode:
            raise RuntimeError(f"launchd job unavailable: {label}")
        return result.stdout

    def pid(self, label=LABEL):
        match = re.search(r"^\s*pid = (\d+)\s*$", self.job(label), re.M)
        return int(match[1]) if match else None

    def identity(self, pid):
        if not pid:
            return None
        result = run(["ps", "-o", "lstart=", "-p", str(pid)])
        return result.stdout.strip() if result.returncode == 0 else None

    def protected(self):
        result = {}
        for label in PROTECTED:
            pid = self.pid(label)
            start = self.identity(pid)
            if not pid or not start:
                raise RuntimeError(f"protected service not running: {label}")
            result[label] = {"pid": pid, "started_at": start}
        return result

    def maintenance(self):
        state = read_json(load_config().state_root / "control.json")
        if (state.get("paused") is not True
                or state.get("pause_generation") != self.pause_generation
                or state.get("actor") != "codex-astra-interactive"):
            raise RuntimeError("interactive maintenance hold changed or absent")

    def validate(self):
        self.maintenance()
        from research_release_compatibility import verify_minime
        verify_minime(self.root, self.root / "workspace")
        source = plistlib.loads((self.root / f"launchd/{LABEL}.plist").read_bytes())
        installed = plistlib.loads((Path.home() / f"Library/LaunchAgents/{LABEL}.plist").read_bytes())
        if source != installed or installed.get("KeepAlive") is not True:
            raise RuntimeError("plists differ or KeepAlive is not true")
        job = self.job()
        match = re.search(r"^\targuments = \{\n(.*?)^\t\}", job, re.M | re.S)
        args = [] if match is None else [line.strip() for line in match[1].splitlines()]
        if args != installed.get("ProgramArguments"):
            raise RuntimeError("loaded launchd arguments differ from installed plist")
        for repo in (self.root, Path(__file__).resolve().parents[1]):
            paths = (COMPONENTS["minime-agent"]["build_paths"] if repo == self.root else
                     ("scripts/restart_minime_agent.py", "scripts/deploy_preflight.py"))
            check = preflight(repo, ack=self.ack, build_paths=paths, external_roots=())
            if not check["ok"]:
                raise RuntimeError(f"deployment preflight denied: {repo}: {check['reason']}")

    def inputs(self):
        return source_inputs(self.root)

    def config(self):
        env = {}
        for key in ENV_KEYS:
            result = run(["launchctl", "getenv", key])
            if result.returncode not in (0, 1):
                raise RuntimeError("cannot read managed launchd setting")
            env[key] = result.stdout.strip()
        profile = self.root / "workspace/rescue_profile.json"
        return {"managed_env_sha256": digest(env),
                "profile_sha256": hashlib.sha256(profile.read_bytes()).hexdigest(),
                "installed_plist_sha256": hashlib.sha256(
                    (Path.home() / f"Library/LaunchAgents/{LABEL}.plist").read_bytes()).hexdigest()}

    def continuity(self):
        state = read_json(self.root / "workspace/sovereignty_state.json")
        return {"file_sha256": digest(state), "session_id": state.get("session_id"),
                "pending_next_sha256": digest(state.get("pending_next_action")),
                "pending_next_present": bool(state.get("pending_next_action")),
                "cycle_count": state.get("cycle_count")}

    def observe(self, expected_pid):
        status = read_json(self.root / "workspace/runtime/autonomous_agent_source_status.json")
        lock = read_json(self.root / "workspace/runtime/autonomous_agent.lock")
        if status.get("pid") != expected_pid or lock.get("pid") != expected_pid:
            raise RuntimeError("source-status or singleton identity mismatch")
        jobs = job_receipt(self.root)
        checked = datetime.fromisoformat(status["checked_at"]).astimezone()
        age = time.time() - checked.timestamp()
        interval = float(lock["interval"])
        # Avoid the loop-entry and next-wakeup edges in the legacy process.
        in_window = 8 <= age <= interval - 8
        tcp = run(["lsof", "-nP", "-a", "-p", str(expected_pid), "-iTCP", "-Fpn"])
        if tcp.returncode not in (0, 1) or tcp.stderr.strip():
            raise RuntimeError("cannot inspect agent TCP activity")
        no_tcp = tcp.returncode == 1 and not tcp.stdout.strip()
        phase = status.get("lifecycle_phase")
        wait_log_sha = None
        if status.get("lifecycle_contract") in {"agent_drain_v1", "agent_drain_v2"}:
            in_window = phase == "idle" and 0 <= age <= interval - 8
        else:
            # The legacy active-job branch logs this line, writes status, then
            # waits on the stop event and continues directly to the loop guard.
            # Wait for that job to finalize while the main loop is still asleep.
            log = self.root / "logs/autonomous-agent.log"
            with log.open("rb") as handle:
                handle.seek(max(0, log.stat().st_size - 8192))
                lines = handle.read().decode("utf-8", errors="replace").splitlines()
            last = lines[-1] if lines else ""
            match = re.match(r"^(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d,\d{3}) - LLM job active: ", last)
            if match:
                logged = datetime.strptime(match[1], "%Y-%m-%d %H:%M:%S,%f").astimezone()
                in_window = in_window and checked.timestamp() <= logged.timestamp() <= time.time()
                wait_log_sha = hashlib.sha256(last.encode()).hexdigest()
            else:
                in_window = False
        summary = read_json(self.root / "workspace/runtime/llm_jobs_status.json")
        quiet = (in_window and no_tcp and not jobs["active"]
                 and summary.get("active_count") == 0)
        return {"quiet": quiet, "source_checked_at": status["checked_at"],
                "source_status_age_s": round(age, 3), "job_index_sha256": jobs["index_sha256"],
                "agent_interval_s": interval,
                "quiet_checks": {"in_idle_window": in_window, "no_tcp_connections": no_tcp,
                                 "indexed_jobs_idle": not jobs["active"],
                                 "summary_jobs_idle": summary.get("active_count") == 0},
                "jobs": jobs["jobs"], "no_tcp_connections": no_tcp,
                "lifecycle_contract": status.get("lifecycle_contract", "legacy_no_handshake"),
                "lifecycle_phase": phase,
                "legacy_wait_log_sha256": wait_log_sha,
                "continuity": self.continuity(),
                "next_handoff": self.next_handoff_identity(status)}

    @staticmethod
    def next_handoff_identity(status):
        if status.get("next_handoff_contract") is None:
            return None  # Legacy runtime: never infer a restoration guarantee.
        from minime_next_handoff import process
        return process(status)

    def verify_next_handoff(self, before, pid, expected_inputs):
        from minime_next_handoff import verify_restore
        status = read_json(self.root / "workspace/runtime/autonomous_agent_source_status.json")
        if status.get("pid") != pid:
            raise RuntimeError("NEXT restoration process changed")
        return verify_restore(self.root / "workspace", before, status, expected_inputs)

    def verify_next_seal(self, before):
        from minime_next_handoff import verify_seal
        status = read_json(self.root / "workspace/runtime/autonomous_agent_source_status.json")
        if status.get("reason") != "drained" or status.get("lifecycle_phase") != "exited":
            raise RuntimeError("NEXT seal lacks completed-drain acknowledgement")
        return verify_seal(self.root / "workspace", before, status)

    def terminate(self, expected_pid):
        # Target the validated PID, never an unexpected launchd replacement.
        import signal
        os.kill(expected_pid, signal.SIGTERM)

    def ready(self, pid, expected_inputs):
        try:
            status = read_json(self.root / "workspace/runtime/autonomous_agent_source_status.json")
            lock = read_json(self.root / "workspace/runtime/autonomous_agent.lock")
        except (OSError, ValueError):
            return False
        return (status.get("pid") == pid == lock.get("pid")
                and status.get("source_inputs_at_start") == expected_inputs
                and status.get("reload_required") is False
                and status.get("reason") in {"loop", "idle"}
                and status.get("lifecycle_contract") in {"agent_drain_v1", "agent_drain_v2"})

    def jobs(self):
        return job_receipt(self.root, full=True)


def reload_agent(backend, expected_pid, *, timeout_s=900, quiet_s=15,
                 now=time.monotonic, sleep=time.sleep, emit=lambda event: None,
                 expected_inputs=None, handoff=None, check_idle_only=False):
    if expected_pid <= 0 or not 30 <= timeout_s <= 1800 or not 10 <= quiet_s <= 30:
        raise ValueError("invalid bounded reload request")
    backend.validate()
    inputs, config, protected = backend.inputs(), backend.config(), backend.protected()
    if expected_inputs is not None and inputs != expected_inputs:
        raise RuntimeError("canonical launch sources differ from qualified inputs; no signal sent")
    old_start = backend.identity(expected_pid)
    if not old_start or backend.pid() != expected_pid:
        raise RuntimeError("agent identity changed before reload")
    initial_jobs = backend.jobs()
    initially_interrupted = {j["job_id"] for j in initial_jobs["jobs"]
                            if j.get("error") == "worker_restarted_before_completion"}
    emit({"phase": "waiting_for_idle", "old_pid": expected_pid,
          "old_started_at": old_start, "source_inputs": inputs,
          "config": config, "protected": protected})
    deadline = now() + timeout_s
    quiet_start = None
    previous_key = None
    completed_handoff = None
    observation = {}
    while now() < deadline:
        backend.maintenance()
        if backend.pid() != expected_pid or backend.identity(expected_pid) != old_start:
            raise RuntimeError("agent identity changed while waiting; no signal sent")
        observation = backend.observe(expected_pid)
        key = (observation["source_checked_at"], observation["job_index_sha256"],
               observation["continuity"]["file_sha256"], observation.get("next_handoff"))
        if not observation["quiet"]:
            quiet_start = None
        elif quiet_start is None or key != previous_key:
            quiet_start = now()
        previous_key = key
        if quiet_start is not None and now() - quiet_start >= quiet_s:
            break
        sleep(1)
    else:
        emit({"phase": "idle_boundary_refused", "boundary": "waiting_timeout",
              "diagnostic": idle_diagnostic({}, observation)})
        raise RuntimeError("no bounded idle window; no signal sent")
    boundary_started = now()
    backend.validate()
    if backend.jobs()["active"]:
        raise RuntimeError("canonical active jobs remain; no signal sent")
    if backend.inputs() != inputs or backend.config() != config or backend.protected() != protected:
        raise RuntimeError("source/config/protected process changed; no signal sent")
    final = backend.observe(expected_pid)
    if (not final["quiet"] or final["source_checked_at"] != observation["source_checked_at"]
            or final["job_index_sha256"] != observation["job_index_sha256"]
            or final["continuity"] != observation["continuity"]
            or final.get("next_handoff") != observation.get("next_handoff")):
        emit({"phase": "idle_boundary_refused", "boundary": "pre_signal",
              "diagnostic": idle_diagnostic(observation, final, elapsed_s=now() - boundary_started)})
        raise RuntimeError("idle boundary moved; no signal sent")
    if backend.pid() != expected_pid or backend.identity(expected_pid) != old_start:
        raise RuntimeError("PID changed at signal boundary")
    if check_idle_only:
        return {"schema": "minime_agent_idle_check_v1", "outcome": "idle_verified_no_signal",
                "old_pid": expected_pid, "forced_termination": False,
                "launch_hold_created": False,
                "diagnostic": idle_diagnostic(observation, final, elapsed_s=now() - boundary_started)}
    if handoff is not None:
        boundary_started = now()
        handoff.begin(backend, inputs, config, protected, emit)
        # Hold installation is I/O; repeat the actual admission boundary checks.
        boundary = backend.observe(expected_pid)
        if (not boundary["quiet"] or any(boundary[k] != final[k] for k in
                ("source_checked_at", "job_index_sha256", "continuity"))
                or boundary.get("next_handoff") != final.get("next_handoff")
                or backend.pid() != expected_pid
                or backend.identity(expected_pid) != old_start):
            diagnostic = idle_diagnostic(final, boundary, elapsed_s=now() - boundary_started)
            diagnostic["process_identity_changed"] = (
                backend.pid() != expected_pid or backend.identity(expected_pid) != old_start)
            emit({"phase": "idle_boundary_refused", "boundary": "after_launch_hold",
                  "diagnostic": diagnostic})
            raise RuntimeError("idle boundary moved after launch hold; no signal sent")
    # Retain evidence before any signal, including failures after this point.
    emit({"phase": "signal_boundary", "observation": final,
          "atomic_traffic_quiescence_claimed": False})
    backend.terminate(expected_pid)
    emit({"phase": "signal_sent", "signal": "SIGTERM", "pid": expected_pid})
    deadline = now() + timeout_s
    while now() < deadline:
        if backend.protected() != protected:
            raise RuntimeError("protected process changed during reload; no further signal sent")
        if backend.identity(expected_pid) != old_start:
            if handoff is not None:
                if backend.identity(expected_pid) is not None:
                    raise RuntimeError("old PID reused during paired handoff")
                if final.get("next_handoff"):
                    backend.verify_next_seal(final["next_handoff"])
                protected = handoff.transition(backend, protected, emit)
                completed_handoff = handoff
                replacement_inputs = getattr(handoff, "replacement_inputs", None)
                if replacement_inputs is not None:
                    # Only the reviewed handoff may replace the source inventory
                    # after old-process exit; readiness remains exact-hash bound.
                    inputs = replacement_inputs
                handoff = None
                deadline = now() + timeout_s
            new_pid = backend.pid()
            if new_pid and new_pid != expected_pid and backend.ready(new_pid, inputs):
                if backend.inputs() != inputs or backend.config() != config:
                    raise RuntimeError("source/config changed during reload")
                final_jobs = backend.jobs()
                old_active = [j for j in final_jobs["jobs"]
                              if j.get("worker_pid") == expected_pid and j["status"] not in TERMINAL]
                if old_active:
                    raise RuntimeError("old process left unfinished jobs")
                if any(j.get("error") == "worker_restarted_before_completion"
                       and j["job_id"] not in initially_interrupted for j in final_jobs["jobs"]):
                    raise RuntimeError("startup recovered an interrupted job; rollout is not clean")
                before_ids = {j["job_id"] for j in initial_jobs["jobs"]}
                handoff_proof = (backend.verify_next_handoff(final["next_handoff"], new_pid, inputs)
                                 if final.get("next_handoff") else None)
                legacy_proof = (completed_handoff.verify_restoration(backend, new_pid)
                                if completed_handoff is not None
                                and hasattr(completed_handoff, "verify_restoration") else None)
                return {"schema": "minime_agent_reload_v1", "outcome": "success",
                        "old_pid": expected_pid, "old_started_at": old_start,
                        "new_pid": new_pid, "new_started_at": backend.identity(new_pid),
                        "signal": "SIGTERM", "forced_termination": False,
                        "atomic_traffic_quiescence_claimed": False,
                        "source_inputs": inputs, "config": config, "protected": protected,
                        "pre_signal": final, "post_ready_continuity": backend.continuity(),
                        "next_handoff": handoff_proof,
                        "legacy_next_handoff": legacy_proof,
                        "new_jobs_during_rollout": [j for j in final_jobs["jobs"] if j["job_id"] not in before_ids]}
        sleep(1)
    raise RuntimeError("drain/readiness timed out; no forced fallback; inspect live PID")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--expected-pid", required=True, type=int)
    parser.add_argument("--pause-generation", required=True, type=int)
    parser.add_argument("--ack", required=True)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--timeout-secs", default=900, type=int)
    parser.add_argument("--check-idle-only", action="store_true",
                        help="exercise pre-signal checks only; never install a hold or send a signal")
    parser.add_argument("--expected-inputs", type=Path,
                        help="reconciliation receipt whose selected_inputs must match canonical launch sources")
    args = parser.parse_args()
    if not args.ack.strip():
        parser.error("nonempty acknowledgement required")
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    with args.receipt.open("x") as handle:
        def emit(event):
            event = {"recorded_at": datetime.now(timezone.utc).isoformat(), **event}
            handle.write(json.dumps(event, sort_keys=True) + "\n")
            handle.flush()
            os.fsync(handle.fileno())
            print(json.dumps({k: v for k, v in event.items() if k in {
                "phase", "recorded_at", "outcome", "old_pid", "new_pid", "signal", "error",
                "boundary", "diagnostic"}}), flush=True)
        try:
            result = reload_agent(LaunchdAgent(args.pause_generation, args.ack),
                                  args.expected_pid, timeout_s=args.timeout_secs, emit=emit,
                                  check_idle_only=args.check_idle_only,
                                  expected_inputs=(qualified_inputs(args.expected_inputs)
                                                   if args.expected_inputs else None))
            emit(result)
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            emit({"outcome": "failed", "error": str(error), "forced_termination": False})
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
