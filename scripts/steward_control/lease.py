"""Cooperative, token-authenticated steward lease and pause state."""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
import time
from typing import Any
import uuid

from .config import ControlConfig
from .errors import BusyError, LeaseError, PausedError
from .model import (
    atomic_write_json,
    exclusive_file_lock,
    host_identity,
    load_json,
    random_token,
    utc_now,
)


def token_hash(token: str) -> str:
    return hashlib.sha256(token.encode("utf-8")).hexdigest()


def pid_alive(pid: int) -> bool:
    if pid <= 0:
        return False
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


class LeaseManager:
    def __init__(self, config: ControlConfig):
        self.config = config
        self.lock_path = config.state_root / ".control.lock"
        self.state_path = config.state_root / "control.json"
        self.lease_path = config.state_root / "lease.json"

    def _default_state(self) -> dict[str, Any]:
        return {
            "schema": "steward_control_state_v1",
            "schema_version": 1,
            "paused": True,
            "pause_generation": 1,
            "actor": "bootstrap",
            "reason": "control plane starts paused until explicitly resumed",
            "recorded_at": utc_now(),
        }

    def state(self) -> dict[str, Any]:
        return load_json(self.state_path) or self._default_state()

    def lease(self) -> dict[str, Any] | None:
        return load_json(self.lease_path)

    def is_stale(self, lease: dict[str, Any], now: float | None = None) -> bool:
        current = time.time() if now is None else now
        if float(lease.get("expires_at_unix") or 0) <= current:
            return True
        if lease.get("adapter_kind") == "subprocess":
            return not pid_alive(int(lease.get("pid") or -1))
        return False

    def pause(self, actor: str, reason: str) -> dict[str, Any]:
        if not actor.strip() or not reason.strip():
            raise ValueError("pause requires non-empty actor and reason")
        with exclusive_file_lock(self.lock_path):
            state = self.state()
            generation = int(state.get("pause_generation") or 0)
            if not state.get("paused"):
                generation += 1
            updated = {
                "schema": "steward_control_state_v1",
                "schema_version": 1,
                "paused": True,
                "pause_generation": max(1, generation),
                "actor": actor,
                "reason": reason,
                "recorded_at": utc_now(),
            }
            atomic_write_json(self.state_path, updated)
            return updated

    def resume(self, actor: str, acknowledgement: str) -> dict[str, Any]:
        if not actor.strip() or not acknowledgement.strip():
            raise ValueError("resume requires actor and acknowledgement")
        with exclusive_file_lock(self.lock_path):
            state = self.state()
            lease = self.lease()
            if lease and not self.is_stale(lease):
                raise BusyError(f"run {lease.get('run_id')} still holds the lease")
            if lease:
                self.lease_path.unlink(missing_ok=True)
            updated = {
                "schema": "steward_control_state_v1",
                "schema_version": 1,
                "paused": False,
                "pause_generation": int(state.get("pause_generation") or 0) + 1,
                "actor": actor,
                "acknowledgement_sha256": hashlib.sha256(
                    acknowledgement.encode("utf-8")
                ).hexdigest(),
                "recorded_at": utc_now(),
            }
            atomic_write_json(self.state_path, updated)
            return updated

    def acquire(
        self,
        *,
        actor: str,
        adapter_kind: str,
        repositories: dict[str, dict[str, Any]],
        pid: int | None = None,
    ) -> tuple[dict[str, Any], str, dict[str, Any] | None]:
        if adapter_kind not in {"session", "subprocess", "project"}:
            raise ValueError(f"unsupported adapter kind: {adapter_kind}")
        now = time.time()
        with exclusive_file_lock(self.lock_path):
            state = self.state()
            if state.get("paused"):
                raise PausedError(str(state.get("reason") or "control plane paused"))
            stale_lease: dict[str, Any] | None = None
            lease = self.lease()
            if lease:
                if not self.is_stale(lease, now):
                    raise BusyError(f"run {lease.get('run_id')} holds the lease")
                stale_lease = lease
                self.lease_path.unlink(missing_ok=True)
            token = random_token()
            run_id = f"run_{time.time_ns()}_{uuid.uuid4().hex[:10]}"
            record = {
                "schema": "steward_control_lease_v1",
                "schema_version": 1,
                "run_id": run_id,
                "actor": actor or "interactive-agent",
                "adapter_kind": adapter_kind,
                "token_sha256": token_hash(token),
                "host": host_identity(),
                "pid": int(pid or os.getpid()),
                "process_started_at_unix": now,
                "acquired_at": utc_now(),
                "heartbeat_at_unix": now,
                "expires_at_unix": now + self.config.lease_ttl_secs,
                "pause_generation": int(state.get("pause_generation") or 0),
                "config_sha256": self.config.config_sha256,
                "repositories": repositories,
            }
            atomic_write_json(self.lease_path, record)
            return record, token, stale_lease

    def _owned(self, run_id: str, token: str) -> dict[str, Any]:
        lease = self.lease()
        if not lease or lease.get("run_id") != run_id:
            raise LeaseError(f"run {run_id} does not own the active lease")
        if lease.get("token_sha256") != token_hash(token):
            raise LeaseError("lease token does not match")
        if self.is_stale(lease):
            raise LeaseError("lease expired")
        return lease

    def heartbeat(self, run_id: str, token: str) -> dict[str, Any]:
        now = time.time()
        with exclusive_file_lock(self.lock_path):
            lease = self._owned(run_id, token)
            state = self.state()
            lease["heartbeat_at_unix"] = now
            lease["expires_at_unix"] = now + self.config.lease_ttl_secs
            lease["stop_requested"] = bool(state.get("paused"))
            atomic_write_json(self.lease_path, lease)
            return {
                "run_id": run_id,
                "expires_at_unix": lease["expires_at_unix"],
                "stop_requested": lease["stop_requested"],
                "pause_generation": state.get("pause_generation"),
            }

    def release(self, run_id: str, token: str) -> dict[str, Any]:
        with exclusive_file_lock(self.lock_path):
            lease = self._owned(run_id, token)
            self.lease_path.unlink(missing_ok=True)
            return lease

    def reap_stale(self) -> dict[str, Any] | None:
        with exclusive_file_lock(self.lock_path):
            lease = self.lease()
            if not lease or not self.is_stale(lease):
                return None
            self.lease_path.unlink(missing_ok=True)
            return lease

    def wait_for_release(self, wait_secs: float) -> bool:
        deadline = time.monotonic() + max(0.0, wait_secs)
        while True:
            lease = self.lease()
            if not lease or self.is_stale(lease):
                return True
            if time.monotonic() >= deadline:
                return False
            time.sleep(0.1)
