"""Evidence-only event append and outage spool."""

from __future__ import annotations

import json
from pathlib import Path
import time
from typing import Any
import uuid

from .config import ControlConfig
from .model import atomic_write_json, authority_state, load_json, utc_now


class EventSink:
    def __init__(self, config: ControlConfig):
        self.config = config
        self.spool_root = config.state_root / "pending_events"

    def domain_event(
        self,
        event_type: str,
        *,
        aggregate_type: str,
        aggregate_id: str,
        payload: dict[str, Any],
        idempotency_key: str,
    ) -> dict[str, Any]:
        return {
            "schema": "steward_control_domain_event_v1",
            "schema_version": 1,
            "event_type": event_type,
            "aggregate_type": aggregate_type,
            "aggregate_id": aggregate_id,
            "recorded_at": utc_now(),
            "payload": payload,
            "idempotency_key": idempotency_key,
            "artifact_authority_state_v1": authority_state(),
        }

    def _append(self, event: dict[str, Any]) -> None:
        try:
            from evidence_store.adapter import append_domain_events
        except ModuleNotFoundError:
            from scripts.evidence_store.adapter import append_domain_events

        append_domain_events(
            self.config.state_root,
            "steward_control",
            [event],
            actor=str(event.get("payload", {}).get("actor") or "interactive-agent"),
        )

    def _spool(self, event: dict[str, Any], error: Exception) -> Path:
        event = dict(event)
        event["spool"] = {
            "schema": "steward_control_event_spool_v1",
            "schema_version": 1,
            "error_type": type(error).__name__,
            "spooled_at": utc_now(),
        }
        path = self.spool_root / (
            f"{time.time_ns()}_{uuid.uuid4().hex[:10]}.json"
        )
        atomic_write_json(path, event)
        return path

    def emit(self, event: dict[str, Any]) -> dict[str, Any]:
        try:
            self._append(event)
        except Exception as error:  # Evidence outages must not break pause.
            path = self._spool(event, error)
            return {"appended": False, "spooled": str(path)}
        return {"appended": True, "spooled": None}

    def pending(self) -> list[Path]:
        return sorted(self.spool_root.glob("*.json"))

    def reconcile(self) -> dict[str, Any]:
        appended = 0
        failed: list[str] = []
        for path in self.pending():
            event = load_json(path)
            if event is None:
                failed.append(path.name)
                continue
            event.pop("spool", None)
            try:
                self._append(event)
            except Exception:
                failed.append(path.name)
                break
            path.unlink(missing_ok=True)
            appended += 1
        return {
            "schema": "steward_control_reconcile_v1",
            "schema_version": 1,
            "appended": appended,
            "failed": failed,
            "pending": len(self.pending()),
        }
