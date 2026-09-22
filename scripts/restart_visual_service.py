#!/usr/bin/env python3
"""Reload only the visual request service at a verified idle boundary.

The first legacy transition is observed-idle, not an atomic admission drain.
Require explicit acknowledgement, an empty queue and advancing idle status with
no TCP work. Send one SIGTERM to the validated PID; never force or bootout.
New releases finish a current request and retain unstarted requests on disk.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import math
import os
from pathlib import Path
import plistlib
import re
import signal
import subprocess
import time

from deploy_preflight import COMPONENTS, preflight
from restart_minime_agent import (
    LABEL as AGENT_LABEL, PROTECTED, ROOT, LaunchdAgent, digest, read_json, run,
)

LABEL = "com.minime.visual-frame-service"
CONTRACT = "visual_finish_current_request_v1"


class VisualService:
    def __init__(self, generation, ack):
        self.agent = LaunchdAgent(generation, ack)
        self.ack = ack

    def maintenance(self):
        self.agent.maintenance()
        # A pause alone is insufficient while an earlier owner is draining.
        from steward_control.config import load_config
        root = load_config().state_root
        if (root / "lease.json").exists() or (root / "active_projection.json").exists():
            raise RuntimeError("steward lease/projection present; no service signal allowed")

    def pid(self):
        return self.agent.pid(LABEL)

    def identity(self, pid):
        return self.agent.identity(pid)

    def protected(self):
        result = {}
        for label in (*[item for item in PROTECTED if item != LABEL], AGENT_LABEL):
            pid = self.agent.pid(label)
            start = self.identity(pid)
            if not pid or not start:
                raise RuntimeError(f"protected service not running: {label}")
            result[label] = {"pid": pid, "started_at": start}
        return result

    def inputs(self):
        return self.agent.inputs()

    def config(self):
        env = {}
        for name in ("MINIME_PYTHON_BIN", "CAMERA_INDEX", "LOOK_SOURCE"):
            result = run(["launchctl", "getenv", name])
            if result.returncode not in (0, 1):
                raise RuntimeError("cannot inspect managed setting")
            env[name] = result.stdout.strip()
        installed = Path.home() / f"Library/LaunchAgents/{LABEL}.plist"
        return {"environment_sha256": digest(env),
                "plist_sha256": digest(plistlib.loads(installed.read_bytes()))}

    def validate(self):
        self.maintenance()
        source = plistlib.loads((ROOT / f"launchd/{LABEL}.plist").read_bytes())
        installed = plistlib.loads(
            (Path.home() / f"Library/LaunchAgents/{LABEL}.plist").read_bytes())
        if source != installed or installed.get("KeepAlive") is not True:
            raise RuntimeError("visual service plists differ or KeepAlive is not true")
        job = self.agent.job(LABEL)
        match = re.search(r"^\targuments = \{\n(.*?)^\t\}", job, re.M | re.S)
        args = [] if match is None else [line.strip() for line in match[1].splitlines()]
        if args != installed.get("ProgramArguments"):
            raise RuntimeError("loaded visual service arguments differ")
        paths = (*COMPONENTS["minime-agent"]["build_paths"],
                 "visual_frame_service.py", "scripts/launchd_visual_frame_service.sh",
                 f"launchd/{LABEL}.plist")
        for repo, build_paths in (
            (ROOT, paths),
            (Path(__file__).resolve().parents[1],
             ("scripts/restart_visual_service.py", "scripts/restart_minime_agent.py",
              "scripts/deploy_preflight.py")),
        ):
            check = preflight(repo, ack=self.ack, build_paths=build_paths, external_roots=())
            if not check["ok"]:
                raise RuntimeError(f"preflight denied: {repo}: {check['reason']}")

    def observe(self, pid):
        status = read_json(ROOT / "workspace/runtime/visual_status.json")
        stamp = status.get("ts_ms")
        age = time.time() - stamp / 1000 if isinstance(stamp, (int, float)) and not isinstance(stamp, bool) else math.inf
        tcp = run(["lsof", "-nP", "-a", "-p", str(pid), "-iTCP", "-Fpn"])
        if tcp.returncode not in (0, 1) or tcp.stderr.strip():
            raise RuntimeError("cannot inspect visual TCP activity")
        pending = sorted(p.name for p in (ROOT / "workspace/visual_requests").glob("*.json"))
        contract = status.get("lifecycle_contract", "legacy_no_handshake")
        identity_ok = status.get("pid") == pid if contract == CONTRACT else "pid" not in status
        quiet = (0 <= age <= 8 and status.get("state") == "polling"
                 and status.get("healthy") is True and status.get("pending_requests") == 0
                 and status.get("active_request") is None and not pending and identity_ok
                 and tcp.returncode == 1 and not tcp.stdout.strip())
        return {"quiet": quiet, "ts_ms": stamp, "status_age_s": age if math.isfinite(age) else None,
                "contract": contract, "pending_requests": len(pending),
                "source_inputs_sha256_at_start": status.get("source_inputs_sha256_at_start")}

    def terminate(self, pid):
        os.kill(pid, signal.SIGTERM)


def reload_service(backend, expected_pid, *, allow_legacy=False, timeout_s=300,
                   quiet_s=15, now=time.monotonic, sleep=time.sleep, emit=lambda event: None):
    if (isinstance(expected_pid, bool) or expected_pid <= 0
            or not 30 <= timeout_s <= 900 or not 10 <= quiet_s <= 30):
        raise ValueError("invalid bounded visual reload request")
    backend.validate()
    inputs, config, protected = backend.inputs(), backend.config(), backend.protected()
    start = backend.identity(expected_pid)
    if not start or backend.pid() != expected_pid:
        raise RuntimeError("visual PID changed before reload")
    emit({"phase": "waiting_for_idle", "old_pid": expected_pid, "old_started_at": start,
          "source_inputs": inputs, "config": config, "protected": protected})
    deadline, quiet_start, first_stamp = now() + timeout_s, None, None
    while now() < deadline:
        backend.maintenance()
        if backend.pid() != expected_pid or backend.identity(expected_pid) != start:
            raise RuntimeError("visual PID changed while waiting; no signal sent")
        observation = backend.observe(expected_pid)
        contract = observation["contract"]
        if contract not in (CONTRACT, "legacy_no_handshake"):
            raise RuntimeError("unknown visual lifecycle contract")
        if contract == "legacy_no_handshake" and not allow_legacy:
            raise RuntimeError("legacy observed-idle transition not acknowledged")
        if not observation["quiet"]:
            quiet_start, first_stamp = None, None
        elif quiet_start is None:
            quiet_start, first_stamp = now(), observation["ts_ms"]
        if (quiet_start is not None and now() - quiet_start >= quiet_s
                and observation["ts_ms"] > first_stamp):
            break
        sleep(1)
    else:
        raise RuntimeError("no bounded visual idle window; no signal sent")
    backend.validate()
    if backend.inputs() != inputs or backend.config() != config or backend.protected() != protected:
        raise RuntimeError("source/config/protected service drift; no signal sent")
    final = backend.observe(expected_pid)
    if not final["quiet"] or final["contract"] != observation["contract"]:
        raise RuntimeError("visual idle boundary moved; no signal sent")
    if backend.pid() != expected_pid or backend.identity(expected_pid) != start:
        raise RuntimeError("visual PID changed at signal boundary")
    emit({"phase": "signal_boundary", "observation": final,
          "atomic_admission_drain_claimed": False,
          "legacy_observed_idle_acknowledged": allow_legacy})
    backend.terminate(expected_pid)
    emit({"phase": "signal_sent", "signal": "SIGTERM", "pid": expected_pid})
    deadline = now() + timeout_s
    while now() < deadline:
        backend.maintenance()
        if backend.protected() != protected:
            raise RuntimeError("protected service changed; no further signal sent")
        pid = backend.pid()
        if pid and pid != expected_pid and backend.identity(expected_pid) != start:
            observation = backend.observe(pid)
            if (observation["quiet"] and observation["contract"] == CONTRACT
                    and observation["source_inputs_sha256_at_start"] == digest(inputs)):
                if backend.inputs() != inputs or backend.config() != config:
                    raise RuntimeError("source/config changed during reload")
                return {"outcome": "success", "schema": "visual_service_reload_v1",
                        "old_pid": expected_pid, "old_started_at": start,
                        "new_pid": pid, "new_started_at": backend.identity(pid),
                        "source_inputs": inputs, "protected": protected, "config": config,
                        "atomic_admission_drain_claimed": False,
                        "forced_termination": False, "ready": observation}
        sleep(1)
    raise RuntimeError("visual drain/readiness timed out; no forced fallback")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--expected-pid", required=True, type=int)
    parser.add_argument("--pause-generation", required=True, type=int)
    parser.add_argument("--ack", required=True)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--allow-legacy-observed-idle", action="store_true")
    parser.add_argument("--timeout-secs", type=int, default=300)
    args = parser.parse_args()
    if not args.ack.strip():
        parser.error("nonempty acknowledgement required")
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    with args.receipt.open("x") as handle:
        def emit(event):
            event = {"recorded_at": datetime.now(timezone.utc).isoformat(), **event}
            handle.write(json.dumps(event, sort_keys=True, allow_nan=False) + "\n")
            handle.flush()
            os.fsync(handle.fileno())
            print(json.dumps({key: value for key, value in event.items()
                              if key in {"phase", "outcome", "old_pid", "new_pid", "error"}}), flush=True)
        try:
            emit(reload_service(VisualService(args.pause_generation, args.ack), args.expected_pid,
                                allow_legacy=args.allow_legacy_observed_idle,
                                timeout_s=args.timeout_secs, emit=emit))
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            emit({"outcome": "failed", "error": str(error), "forced_termination": False})
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
