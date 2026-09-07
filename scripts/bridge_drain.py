#!/usr/bin/env python3
"""Request a producer drain only from a process advertising the new lifecycle.

No restart, SIGKILL, traffic generation, being Action, or automatic resumption.
A timeout leaves the process running (possibly still completing admitted work).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time


class DrainError(RuntimeError):
    pass


def process_identity(pid: int) -> tuple[str, str]:
    def ps(field: str) -> str:
        result = subprocess.run(
            ["ps", "-p", str(pid), "-o", field + "="],
            capture_output=True, text=True, timeout=5, check=False,
        )
        if result.returncode or not result.stdout.strip():
            raise DrainError("target process is not running")
        return result.stdout.strip()
    return ps("lstart"), ps("comm")


def read_status(path: Path) -> dict:
    if not path.is_file() or path.is_symlink():
        raise DrainError("unsupported process: no regular lifecycle acknowledgement; no signal sent")
    with path.open("rb") as handle:
        raw = handle.read(16385)
    if len(raw) > 16384:
        raise DrainError("oversized lifecycle status")
    try:
        value = json.loads(raw)
    except (ValueError, UnicodeError) as error:
        raise DrainError("invalid lifecycle status") from error
    if not isinstance(value, dict):
        raise DrainError("invalid lifecycle status object")
    return value


def digest(path: Path) -> str:
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def verify(status: dict, pid: int, binary: Path, identity: tuple[str, str]) -> None:
    if status.get("schema") != "bridge_operator_drain_v1" or status.get("pid") != pid:
        raise DrainError("lifecycle protocol or PID mismatch")
    instance = status.get("instance")
    if not isinstance(instance, str) or len(instance) != 32 or any(c not in "0123456789abcdef" for c in instance):
        raise DrainError("invalid lifecycle instance")
    if status.get("authority") != "operator_maintenance_witness_only":
        raise DrainError("unexpected lifecycle authority")
    if status.get("phase") not in {"running", "draining", "drained"}:
        raise DrainError("process reports failed or unknown drain phase")
    if Path(identity[1]).resolve() != binary.resolve() or status.get("executable") != str(binary.resolve()):
        raise DrainError("target executable mismatch")
    if status.get("executable_sha256") != digest(binary):
        raise DrainError("executable changed since lifecycle registration")
    # PID alone is reusable. A status created before this process is stale.
    started = time.mktime(time.strptime(identity[0], "%a %b %d %H:%M:%S %Y"))
    registered = status.get("started_at_unix_ms")
    if type(registered) is not int or not started <= registered / 1000 <= time.time() + 1:
        raise DrainError("stale or future lifecycle registration")


def drain(pid: int, binary: Path, directory: Path, timeout: float, request: bool) -> dict:
    path = directory / f"{pid}.json"
    identity = process_identity(pid)
    initial = read_status(path)
    verify(initial, pid, binary, identity)
    if process_identity(pid) != identity:
        raise DrainError("process changed during validation")
    if not request:
        return {"supported": True, "pid": pid, "phase": initial["phase"], "signal_sent": False}
    if initial["phase"] == "running":
        os.kill(pid, signal.SIGUSR1)
    deadline = time.monotonic() + timeout
    while True:
        if process_identity(pid) != identity:
            raise DrainError("process exited or changed during drain")
        current = read_status(path)
        if current.get("instance") != initial["instance"]:
            raise DrainError("lifecycle instance changed")
        verify(current, pid, binary, identity)
        if current["phase"] == "drained":
            checkpoint = current.get("checkpoint")
            if not isinstance(checkpoint, dict) or not isinstance(checkpoint.get("path"), str):
                raise DrainError("drained process has no conversation checkpoint")
            if digest(Path(checkpoint["path"])) != checkpoint.get("sha256"):
                raise DrainError("conversation checkpoint changed after drain")
            return {"supported": True, "pid": pid, "instance": current["instance"],
                    "phase": "drained", "checkpoint_sha256": checkpoint["sha256"],
                    "remote_delivery_confirmed": False, "restart_performed": False}
        if time.monotonic() >= deadline:
            raise DrainError("drain timeout: process left running; no escalation or automatic resume")
        time.sleep(0.25)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--status-dir", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=600)
    parser.add_argument("--request", action="store_true")
    parser.add_argument("--ack", default="")
    args = parser.parse_args()
    if args.pid <= 1 or not 0 < args.timeout <= 1800:
        parser.error("PID must exceed 1 and timeout must be within (0, 1800]")
    if args.request and not args.ack.strip():
        parser.error("--request requires an explicit --ack")
    try:
        result = drain(args.pid, args.binary, args.status_dir, args.timeout, args.request)
    except (DrainError, OSError, subprocess.SubprocessError) as error:
        print(json.dumps({"ok": False, "error": str(error), "force_used": False}))
        return 1
    print(json.dumps({"ok": True, **result}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
