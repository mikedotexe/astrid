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
    "AGENT_INTERVAL", "FOCUSED_MODE",
)
TERMINAL = {"completed", "thin_output", "timeout", "failed", "canceled", "blocked"}


def read_json(path):
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        raise ValueError(f"not an object: {path}")
    return value


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


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
        rows.append({k: job.get(k) for k in (
            "job_id", "status", "worker_pid", "action_id", "finished_at", "error",
        )})
    active = [r for r in rows if r["status"] not in TERMINAL]
    return {"active": active, "index_sha256": digest(index), "jobs": rows}


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
        if status.get("lifecycle_contract") == "agent_drain_v1":
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
                "jobs": jobs["jobs"], "no_tcp_connections": no_tcp,
                "lifecycle_contract": status.get("lifecycle_contract", "legacy_no_handshake"),
                "legacy_wait_log_sha256": wait_log_sha,
                "continuity": self.continuity()}

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
                and status.get("lifecycle_contract") == "agent_drain_v1")

    def jobs(self):
        return job_receipt(self.root, full=True)


def reload_agent(backend, expected_pid, *, timeout_s=900, quiet_s=15,
                 now=time.monotonic, sleep=time.sleep, emit=lambda event: None):
    if expected_pid <= 0 or not 30 <= timeout_s <= 1800 or not 10 <= quiet_s <= 30:
        raise ValueError("invalid bounded reload request")
    backend.validate()
    inputs, config, protected = backend.inputs(), backend.config(), backend.protected()
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
    while now() < deadline:
        backend.maintenance()
        if backend.pid() != expected_pid or backend.identity(expected_pid) != old_start:
            raise RuntimeError("agent identity changed while waiting; no signal sent")
        observation = backend.observe(expected_pid)
        key = (observation["source_checked_at"], observation["job_index_sha256"],
               observation["continuity"]["file_sha256"])
        if not observation["quiet"]:
            quiet_start = None
        elif quiet_start is None or key != previous_key:
            quiet_start = now()
        previous_key = key
        if quiet_start is not None and now() - quiet_start >= quiet_s:
            break
        sleep(1)
    else:
        raise RuntimeError("no bounded idle window; no signal sent")
    backend.validate()
    if backend.jobs()["active"]:
        raise RuntimeError("canonical active jobs remain; no signal sent")
    if backend.inputs() != inputs or backend.config() != config or backend.protected() != protected:
        raise RuntimeError("source/config/protected process changed; no signal sent")
    final = backend.observe(expected_pid)
    if (not final["quiet"] or final["source_checked_at"] != observation["source_checked_at"]
            or final["job_index_sha256"] != observation["job_index_sha256"]
            or final["continuity"] != observation["continuity"]):
        raise RuntimeError("idle boundary moved; no signal sent")
    if backend.pid() != expected_pid or backend.identity(expected_pid) != old_start:
        raise RuntimeError("PID changed at signal boundary")
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
                return {"schema": "minime_agent_reload_v1", "outcome": "success",
                        "old_pid": expected_pid, "old_started_at": old_start,
                        "new_pid": new_pid, "new_started_at": backend.identity(new_pid),
                        "signal": "SIGTERM", "forced_termination": False,
                        "atomic_traffic_quiescence_claimed": False,
                        "source_inputs": inputs, "config": config, "protected": protected,
                        "pre_signal": final, "post_ready_continuity": backend.continuity(),
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
                "phase", "recorded_at", "outcome", "old_pid", "new_pid", "signal", "error"}}), flush=True)
        try:
            result = reload_agent(LaunchdAgent(args.pause_generation, args.ack),
                                  args.expected_pid, timeout_s=args.timeout_secs, emit=emit)
            emit(result)
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            emit({"outcome": "failed", "error": str(error), "forced_termination": False})
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
