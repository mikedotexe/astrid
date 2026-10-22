#!/usr/bin/env python3
"""Finite stdlib workers for an owned launchd fixture; no production access."""
from __future__ import annotations

import argparse
import datetime
import json
import os
from pathlib import Path
import re
import signal
import socket
import time


SCHEMA = "reservoir_maintenance_owned_fixture_v1"


def atomic(path: Path, value: dict) -> None:
    temporary = path.with_name(path.name + f".{os.getpid()}.tmp")
    with temporary.open("x") as stream:
        json.dump(value, stream, sort_keys=True)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)
    fd = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def validate_root(root: Path, token: str) -> dict:
    if not re.fullmatch(r"[0-9a-f]{32}", token):
        raise ValueError("invalid fixture token")
    if root.is_symlink() or root.resolve() != root:
        raise ValueError("fixture root must be a resolved owned directory")
    marker = root / "fixture.owner.json"
    if marker.is_symlink() or marker.stat().st_uid != os.getuid():
        raise ValueError("foreign fixture marker")
    data = json.loads(marker.read_text())
    if data != {"schema": SCHEMA, "token": token, "root": str(root), "uid": os.getuid()}:
        raise ValueError("fixture ownership mismatch")
    return data


class Worker:
    def __init__(self, root: Path, token: str, role: str, drain_seconds: float):
        validate_root(root, token)
        if role not in {"provider", "reservoir"}:
            raise ValueError("unknown fixture role")
        if not 5 < drain_seconds <= 30:
            raise ValueError("fixture drain must exceed five seconds and be at most thirty")
        self.root, self.token, self.role = root, token, role
        self.drain_seconds = drain_seconds
        self.pid = os.getpid()
        self.stopping = False
        self.request_id = f"{role}-{self.pid}-{time.monotonic_ns()}"

    def event(self, phase: str, **details) -> None:
        value = {"role": self.role, "pid": self.pid, "token": self.token,
                 "phase": phase, "monotonic": time.monotonic(),
                 "at": datetime.datetime.now(datetime.timezone.utc).isoformat(), **details}
        with (self.root / f"{self.role}.events.jsonl").open("a") as stream:
            stream.write(json.dumps(value, sort_keys=True) + "\n")
            stream.flush()
            os.fsync(stream.fileno())

    def stop(self, *_):
        # Signal handler only sets a flag. All retained work finishes in run().
        self.stopping = True

    def run(self) -> int:
        signal.signal(signal.SIGTERM, self.stop)
        signal.signal(signal.SIGINT, self.stop)
        self.event("started", argv=list(__import__("sys").argv), cwd=os.getcwd())
        ready = self.root / f"{self.role}.ready.json"
        listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        listener.bind(("127.0.0.1", 0))
        listener.listen(4)
        self.listener = listener
        atomic(ready, {"pid": self.pid, "token": self.token, "role": self.role,
                       "port": listener.getsockname()[1], "protocol": "fixture_sentinel_only"})
        try:
            return self.provider() if self.role == "provider" else self.reservoir()
        finally:
            listener.close()
            if ready.exists() and json.loads(ready.read_text()).get("pid") == self.pid:
                ready.unlink()

    def provider(self) -> int:
        self.event("idle")
        while not self.stopping:
            time.sleep(0.025)
        # Model admission racing an earlier idle check, without an actual model.
        self.event("generation_admitted", request_id=self.request_id, ticks=11)
        self.event("stop_received", request_id=self.request_id)
        time.sleep(self.drain_seconds)
        request = {"token": self.token, "request_id": self.request_id,
                   "handle": "fixture_astrid", "ticks": 11, "provider_pid": self.pid}
        atomic(self.root / f"request.{self.request_id}.json", request)
        self.event("checkin_sent", request_id=self.request_id)
        ack_path = self.root / f"ack.{self.request_id}.json"
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            if ack_path.exists():
                ack = json.loads(ack_path.read_text())
                if (ack.get("token") != self.token or ack.get("request_id") != self.request_id
                        or ack.get("ticks") != 11 or ack.get("ok") is not True):
                    self.event("checkin_failed", reason="invalid acknowledgement")
                    return 2
                self.event("checkin_acknowledged", request_id=self.request_id, ticks=11)
                self.event("drained", request_id=self.request_id)
                return 0
            time.sleep(0.025)
        self.event("checkin_failed", reason="acknowledgement timeout")
        return 2

    def reservoir(self) -> int:
        path = self.root / "state.json"
        state = json.loads(path.read_text()) if path.exists() else {
            "fixture_astrid": {"ticks": 0}, "fixture_minime": {"ticks": 7}}
        self.event("restored", state=state)
        seen = set()
        while not self.stopping:
            for request_path in sorted(self.root.glob("request.*.json")):
                request = json.loads(request_path.read_text())
                request_id = request["request_id"]
                ack_path = self.root / f"ack.{request_id}.json"
                if request_id in seen or ack_path.exists():
                    continue
                if request.get("token") != self.token or request.get("handle") != "fixture_astrid":
                    raise ValueError("foreign fixture request")
                state["fixture_astrid"]["ticks"] += request["ticks"]
                seen.add(request_id)
                atomic(ack_path, {"token": self.token, "request_id": request_id,
                                  "ticks": request["ticks"], "ok": True})
                self.event("checkin_applied", request_id=request_id, state=state)
            time.sleep(0.025)
        self.event("stop_received")
        if hasattr(self, "listener"):
            self.listener.close()
        time.sleep(self.drain_seconds)
        self.event("final_snapshot_started", handles=sorted(state))
        atomic(path, state)
        for name, value in sorted(state.items()):
            self.event("handle_saved", name=name, value=value)
        self.event("final_snapshot_complete", state=state)
        self.event("drained")
        return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--token", required=True)
    parser.add_argument("--role", choices=("provider", "reservoir"), required=True)
    parser.add_argument("--drain-seconds", type=float, default=6.0)
    args = parser.parse_args()
    return Worker(args.root, args.token, args.role, args.drain_seconds).run()


if __name__ == "__main__":
    raise SystemExit(main())
