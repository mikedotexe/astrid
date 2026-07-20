"""Lease renewal and bounded progress state for long projection generations."""

from __future__ import annotations

from pathlib import Path
import time
from typing import Any

from .config import ControlConfig
from .lease import LeaseManager
from .model import atomic_write_json, utc_now


class ProjectionLeaseGuard:
    """Keep an owned lease alive while exposing token-free progress."""

    SCHEMA = "projection_run_control_v1"

    def __init__(
        self,
        config: ControlConfig,
        leases: LeaseManager,
        *,
        run_id: str,
        lease_token: str,
        actor: str,
        phase: str,
    ):
        self.config = config
        self.leases = leases
        self.run_id = run_id
        self._lease_token = lease_token
        self.actor = actor
        self.phase = phase
        self.path = config.state_root / "active_projection.json"
        self.started_monotonic = time.monotonic()
        self.started_at = utc_now()
        self.last_renewed_monotonic = 0.0
        self.last_renewed_at: str | None = None
        self.expires_at_unix: float | None = None
        self.renewal_count = 0
        self.stop_requested = False
        self.progress: dict[str, Any] = {}
        self.renewal_interval_secs = min(
            float(config.heartbeat_interval_secs),
            max(0.25, float(config.lease_ttl_secs) / 3.0),
        )
        self.poll(force=True)

    @property
    def poll_interval_secs(self) -> float:
        return min(1.0, max(0.05, self.renewal_interval_secs / 4.0))

    def _record(self) -> None:
        generation_id = self.progress.get("generation_id")
        step_id = self.progress.get("step_id")
        command_id = self.progress.get("command_id")
        atomic_write_json(
            self.path,
            {
                "schema": self.SCHEMA,
                "schema_version": 1,
                "run_id": self.run_id,
                "actor": self.actor,
                "phase": self.phase,
                "started_at": self.started_at,
                "elapsed_ms": int(
                    (time.monotonic() - self.started_monotonic) * 1000
                ),
                "last_renewed_at": self.last_renewed_at,
                "expires_at_unix": self.expires_at_unix,
                "renewal_count": self.renewal_count,
                "stop_requested": self.stop_requested,
                "generation_id": generation_id,
                "step_id": step_id,
                "command_id": command_id,
                "completed_step_count": int(
                    self.progress.get("completed_step_count") or 0
                ),
                "reused_step_count": int(
                    self.progress.get("reused_step_count") or 0
                ),
                "progress": self.progress,
                "raw_output_included": False,
                "lease_token_included": False,
            },
        )

    def poll(
        self,
        progress: dict[str, Any] | None = None,
        *,
        force: bool = False,
    ) -> bool:
        if progress:
            self.progress.update(progress)
        now = time.monotonic()
        state = self.leases.state()
        paused = bool(state.get("paused"))
        if (
            force
            or paused
            or now - self.last_renewed_monotonic >= self.renewal_interval_secs
        ):
            heartbeat = self.leases.heartbeat(self.run_id, self._lease_token)
            self.last_renewed_monotonic = now
            self.last_renewed_at = utc_now()
            self.expires_at_unix = float(heartbeat["expires_at_unix"])
            self.renewal_count += 1
            self.stop_requested = bool(heartbeat["stop_requested"])
            self.progress["pause_generation"] = heartbeat.get("pause_generation")
            self._record()
        elif progress:
            self.stop_requested = paused
            self._record()
        return self.stop_requested

    def close(self, *, status: str) -> None:
        self.progress["status"] = status
        self._record()
        try:
            self.path.unlink()
        except FileNotFoundError:
            pass
