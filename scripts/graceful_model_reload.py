#!/usr/bin/env python3
"""Reload only the existing coupled-model launchd job, without forced termination."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import subprocess
import time
import urllib.error
import urllib.request


MODEL_REPO = Path("/Users/v/other/neural-triple-reservoir")
LABEL = "com.reservoir.coupled-astrid"


class LaunchdModel:
    def __init__(self):
        self.target = f"gui/{os.getuid()}/{LABEL}"

    def job(self):
        result = subprocess.run(["launchctl", "print", self.target],
                                capture_output=True, text=True, timeout=10)
        if result.returncode:
            raise RuntimeError("coupled-model launchd job is unavailable")
        return result.stdout

    def validate(self):
        source = plistlib.loads((MODEL_REPO / "launchd" / f"{LABEL}.plist").read_bytes())
        installed = plistlib.loads((Path.home() / "Library/LaunchAgents" / f"{LABEL}.plist").read_bytes())
        if source != installed or installed.get("KeepAlive") is not True:
            raise RuntimeError("in-place reload requires identical plists and KeepAlive=true")
        job = self.job()
        match = re.search(r"^\targuments = \{\n(.*?)^\t\}", job, re.M | re.S)
        args = [] if match is None else [line.strip() for line in match[1].splitlines()]
        if args != installed.get("ProgramArguments"):
            raise RuntimeError("loaded launchd arguments differ from the installed plist")

    def pid(self):
        match = re.search(r"^\s*pid = (\d+)\s*$", self.job(), re.M)
        return None if match is None else int(match[1])

    def identity(self, pid):
        result = subprocess.run(["ps", "-o", "lstart=", "-p", str(pid)],
                                capture_output=True, text=True, timeout=10)
        return result.stdout.strip() if result.returncode == 0 else None

    def terminate(self):
        result = subprocess.run(["launchctl", "kill", "SIGTERM", self.target],
                                capture_output=True, text=True, timeout=10)
        if result.returncode:
            raise RuntimeError("launchd refused the graceful termination signal")

    def idle(self):
        try:
            with urllib.request.urlopen("http://127.0.0.1:8090/readyz", timeout=3) as response:
                health = json.load(response)
        except (OSError, ValueError, urllib.error.URLError):
            return False
        return (health.get("ready") is True
                and health.get("worker", {}).get("phase") == "ready"
                and health.get("worker", {}).get("queue_depth") == 0
                and health.get("reservoir", {}).get("status") == "connected")

    def inputs(self):
        names = ("coupled_astrid_server.py", "coupled_http_gateway.py", "mlx_reservoir.py", f"launchd/{LABEL}.plist")
        return {name: hashlib.sha256((MODEL_REPO / name).read_bytes()).hexdigest() for name in names}


def reload_model(backend, expected_pid, *, timeout_s=900, now=time.monotonic, sleep=time.sleep):
    if not 1 <= timeout_s <= 1800 or expected_pid <= 0:
        raise ValueError("invalid bounded reload request")
    backend.validate()
    source_identity = backend.inputs()
    old_start = backend.identity(expected_pid)
    if not old_start or backend.pid() != expected_pid:
        raise RuntimeError("model identity changed before graceful reload")
    idle_deadline = now() + timeout_s
    while not backend.idle():
        if now() >= idle_deadline:
            raise RuntimeError("no idle empty-queue window; no signal sent")
        if backend.identity(expected_pid) != old_start or backend.pid() != expected_pid:
            raise RuntimeError("model identity changed while waiting for idle; no signal sent")
        sleep(1)
    backend.validate()
    if backend.inputs() != source_identity:
        raise RuntimeError("model source changed while waiting for idle; no signal sent")
    # A second observation prevents signaling a replacement found during validation.
    if backend.identity(expected_pid) != old_start or backend.pid() != expected_pid:
        raise RuntimeError("model identity changed at signal boundary")
    backend.terminate()
    deadline = now() + timeout_s
    while now() < deadline:
        if backend.identity(expected_pid) != old_start:
            new_pid = backend.pid()
            if new_pid and new_pid != expected_pid:
                new_start = backend.identity(new_pid)
                if new_start:
                    return {"schema": "graceful_model_reload_v1", "old_pid": expected_pid,
                            "old_started_at": old_start, "new_pid": new_pid,
                            "new_started_at": new_start, "old_process_exited": True,
                            "signal": "SIGTERM", "forced_termination": False,
                            "idle_empty_queue_observed": True,
                            "atomic_traffic_quiescence_claimed": False,
                            "launchd_configuration_changed": False}
        sleep(1)
    raise RuntimeError("model drain/replacement timed out; no forced termination attempted")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--expected-pid", type=int, required=True)
    parser.add_argument("--timeout-secs", type=int, default=900)
    args = parser.parse_args()
    try:
        result = reload_model(LaunchdModel(), args.expected_pid, timeout_s=args.timeout_secs)
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        parser.exit(1, f"graceful_model_reload: {error}\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
