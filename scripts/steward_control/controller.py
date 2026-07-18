"""High-level steward lifecycle orchestration."""

from __future__ import annotations

import hashlib
from pathlib import Path
import time
from typing import Any

from .config import ControlConfig
from .errors import EvidenceInvalidError, LeaseError, ProjectionError
from .events import EventSink
from .evidence import verify_evidence
from .git_state import (
    git_policy_violations,
    repository_identities,
)
from .lease import LeaseManager, token_hash
from .model import atomic_write_json, load_json, sha256_canonical, utc_now
from .projection import ProjectionCoordinator


class StewardController:
    def __init__(
        self,
        config: ControlConfig,
        *,
        projection_coordinator: ProjectionCoordinator | None = None,
    ):
        self.config = config
        self.leases = LeaseManager(config)
        self.events = EventSink(config)
        self.runs_root = config.state_root / "runs"
        self.projections = projection_coordinator or ProjectionCoordinator(config)

    def _emit(
        self,
        event_type: str,
        *,
        aggregate_type: str,
        aggregate_id: str,
        payload: dict[str, Any],
        idempotency_key: str,
    ) -> dict[str, Any]:
        event = self.events.domain_event(
            event_type,
            aggregate_type=aggregate_type,
            aggregate_id=aggregate_id,
            payload=payload,
            idempotency_key=idempotency_key,
        )
        return self.events.emit(event)

    def _run_path(self, run_id: str) -> Path:
        return self.runs_root / f"{run_id}.json"

    def _source_lag(self) -> dict[str, Any]:
        introspections = self.config.workspace / "introspections"
        newest_name = None
        newest_timestamp = None
        for path in introspections.glob("introspection_*.txt"):
            try:
                timestamp = int(path.stem.rsplit("_", 1)[-1])
            except ValueError:
                continue
            if newest_timestamp is None or timestamp > newest_timestamp:
                newest_timestamp = timestamp
                newest_name = path.name
        status = load_json(
            self.config.workspace
            / "diagnostics/introspection_addressing_v1/status.json"
        )
        cutoff = status.get("cutoff") if isinstance(status, dict) else {}
        cutoff = cutoff if isinstance(cutoff, dict) else {}
        durable_timestamp = cutoff.get("cutoff_timestamp")
        return {
            "schema": "steward_source_lag_v1",
            "schema_version": 1,
            "newest_canonical_filename": newest_name,
            "newest_canonical_timestamp": newest_timestamp,
            "durable_cutoff_filename": cutoff.get("cutoff"),
            "durable_cutoff_timestamp": durable_timestamp,
            "timestamp_lag": (
                newest_timestamp - int(durable_timestamp)
                if newest_timestamp is not None and durable_timestamp is not None
                else None
            ),
            "cutoff_current": newest_timestamp == durable_timestamp,
        }

    def status(self) -> dict[str, Any]:
        state = self.leases.state()
        lease = self.leases.lease()
        if lease:
            lease = {
                key: value
                for key, value in lease.items()
                if key != "token_sha256"
            }
            lease["stale"] = self.leases.is_stale(lease)
        evidence_error = None
        try:
            evidence = verify_evidence(self.config, require_active=False)
        except Exception as error:
            evidence = {"valid": False}
            evidence_error = type(error).__name__
        return {
            "schema": "steward_control_status_v1",
            "schema_version": 1,
            "state": state,
            "lease": lease,
            "pending_event_count": len(self.events.pending()),
            "evidence": evidence,
            "evidence_error": evidence_error,
            "source_lag": self._source_lag(),
            "config_sha256": self.config.config_sha256,
        }

    def verify(self) -> dict[str, Any]:
        evidence = verify_evidence(self.config)
        return {
            "schema": "steward_control_verify_v1",
            "schema_version": 1,
            "evidence": evidence,
            "source_lag": self._source_lag(),
            "pending_event_count": len(self.events.pending()),
            "valid": evidence["valid"] and evidence["v1_immutable"],
        }

    def pause(
        self,
        *,
        actor: str,
        reason: str,
        wait_secs: float = 0,
    ) -> dict[str, Any]:
        state = self.leases.pause(actor, reason)
        active = self.leases.lease()
        event_result = self._emit(
            "steward_control_paused",
            aggregate_type="steward_control",
            aggregate_id="singleton",
            payload={
                "actor": actor,
                "reason_sha256": hashlib.sha256(reason.encode()).hexdigest(),
                "pause_generation": state["pause_generation"],
                "active_run_id": active.get("run_id") if active else None,
                "receipt": {
                    "schema": "steward_pause_receipt_v1",
                    "schema_version": 1,
                    "actor": actor,
                    "reason_sha256": hashlib.sha256(
                        reason.encode()
                    ).hexdigest(),
                    "pause_generation": state["pause_generation"],
                    "active_run_id": active.get("run_id") if active else None,
                    "recorded_at": state["recorded_at"],
                    "raw_reason_included": False,
                },
            },
            idempotency_key=(
                f"pause:{state['pause_generation']}:"
                f"{hashlib.sha256(reason.encode()).hexdigest()}"
            ),
        )
        released = self.leases.wait_for_release(wait_secs) if wait_secs else not active
        return {
            "state": state,
            "active_run_id": active.get("run_id") if active else None,
            "active_run_released": released,
            "event": event_result,
        }

    def resume(self, *, actor: str, acknowledgement: str) -> dict[str, Any]:
        verify_evidence(self.config)
        reconciliation = self.reconcile()
        if reconciliation["events"]["pending"]:
            raise EvidenceInvalidError("pending steward events could not be reconciled")
        state = self.leases.resume(actor, acknowledgement)
        event_result = self._emit(
            "steward_control_resumed",
            aggregate_type="steward_control",
            aggregate_id="singleton",
            payload={
                "actor": actor,
                "pause_generation": state["pause_generation"],
                "acknowledgement_sha256": state["acknowledgement_sha256"],
            },
            idempotency_key=f"resume:{state['pause_generation']}",
        )
        return {"state": state, "event": event_result}

    def begin(
        self,
        *,
        actor: str,
        adapter_kind: str = "session",
        pid: int | None = None,
        project_before: bool = True,
    ) -> dict[str, Any]:
        evidence = verify_evidence(self.config)
        identities = repository_identities(self.config.repositories)
        lease, token, stale = self.leases.acquire(
            actor=actor,
            adapter_kind=adapter_kind,
            repositories=identities,
            pid=pid,
        )
        if stale:
            self._emit(
                "steward_run_abandoned",
                aggregate_type="steward_run",
                aggregate_id=str(stale.get("run_id") or "unknown"),
                payload={
                    "actor": actor,
                    "prior_actor": stale.get("actor"),
                    "reason": "stale_lease_reaped",
                },
                idempotency_key=f"abandoned:{stale.get('run_id')}",
            )
        start_receipt = {
            "schema": "steward_run_receipt_v1",
            "schema_version": 1,
            "run_id": lease["run_id"],
            "actor": lease["actor"],
            "adapter_kind": lease["adapter_kind"],
            "status": "running",
            "started_at": lease["acquired_at"],
            "pause_generation": lease["pause_generation"],
            "config_sha256": lease["config_sha256"],
            "lease_token_sha256": lease["token_sha256"],
            "process": {
                "host": lease["host"],
                "pid": lease["pid"],
                "started_at_unix": lease["process_started_at_unix"],
            },
            "repositories_before": identities,
            "evidence_before": {
                "last_global_seq": evidence["last_global_seq"],
                "last_event_sha256": evidence["last_event_sha256"],
            },
            "raw_prompt_included": False,
            "raw_output_included": False,
        }
        atomic_write_json(self._run_path(lease["run_id"]), start_receipt)
        event_result = self._emit(
            "steward_run_started",
            aggregate_type="steward_run",
            aggregate_id=lease["run_id"],
            payload={
                "actor": actor,
                "adapter_kind": adapter_kind,
                "config_sha256": self.config.config_sha256,
                "pause_generation": lease["pause_generation"],
                "receipt": start_receipt,
            },
            idempotency_key=f"run_started:{lease['run_id']}",
        )
        lease_event = self._emit(
            "steward_lease_acquired",
            aggregate_type="steward_lease",
            aggregate_id=lease["run_id"],
            payload={
                "actor": actor,
                "run_id": lease["run_id"],
                "adapter_kind": adapter_kind,
                "expires_at_unix": lease["expires_at_unix"],
                "token_sha256": lease["token_sha256"],
                "pause_generation": lease["pause_generation"],
            },
            idempotency_key=f"lease_acquired:{lease['run_id']}",
        )
        result = {
            "run_id": lease["run_id"],
            "lease_token": token,
            "expires_at_unix": lease["expires_at_unix"],
            "heartbeat_interval_secs": self.config.heartbeat_interval_secs,
            "event": event_result,
            "lease_event": lease_event,
        }
        if project_before and self.config.profile != "none":
            try:
                generation = self.project(
                    run_id=lease["run_id"],
                    lease_token=token,
                    actor=actor,
                    phase="pre",
                )
            except ProjectionError:
                self.finish(
                    run_id=lease["run_id"],
                    lease_token=token,
                    outcome="failed",
                    summary_ref="pre_projection_failed",
                    project_after=False,
                )
                raise
            result["projection_generation_id"] = generation["generation_id"]
        return result

    def heartbeat(self, *, run_id: str, lease_token: str) -> dict[str, Any]:
        heartbeat = self.leases.heartbeat(run_id, lease_token)
        lease = self.leases.lease() or {}
        event = self._emit(
            "steward_lease_heartbeat",
            aggregate_type="steward_lease",
            aggregate_id=run_id,
            payload={
                "actor": lease.get("actor"),
                "run_id": run_id,
                "expires_at_unix": heartbeat["expires_at_unix"],
                "stop_requested": heartbeat["stop_requested"],
                "pause_generation": heartbeat["pause_generation"],
            },
            idempotency_key=(
                f"lease_heartbeat:{run_id}:"
                f"{float(heartbeat['expires_at_unix']):.6f}:"
                f"{heartbeat['pause_generation']}:"
                f"{heartbeat['stop_requested']}"
            ),
        )
        return {**heartbeat, "event": event}

    def finish(
        self,
        *,
        run_id: str,
        lease_token: str,
        outcome: str,
        exit_code: int | None = None,
        summary_ref: str | None = None,
        project_after: bool = True,
    ) -> dict[str, Any]:
        if outcome not in {"success", "failed", "cancelled", "policy_violation"}:
            raise ValueError(f"unsupported outcome: {outcome}")
        receipt_path = self._run_path(run_id)
        existing = load_json(receipt_path)
        if (
            existing
            and existing.get("status") == "finished"
            and existing.get("lease_token_sha256") == token_hash(lease_token)
        ):
            return {"receipt": existing, "idempotent": True}
        if existing is None:
            raise LeaseError(f"missing run receipt for {run_id}")

        projection_generation_id = None
        projection_error = None
        if (
            project_after
            and outcome == "success"
            and self.config.profile != "none"
        ):
            try:
                generation = self.project(
                    run_id=run_id,
                    lease_token=lease_token,
                    actor=str(existing.get("actor") or "interactive-agent"),
                    phase="post",
                )
                projection_generation_id = generation["generation_id"]
            except ProjectionError as error:
                outcome = "failed"
                projection_error = {
                    "error_type": type(error).__name__,
                    "error_sha256": hashlib.sha256(str(error).encode()).hexdigest(),
                }

        after = repository_identities(self.config.repositories)
        violations = git_policy_violations(
            existing.get("repositories_before") or {},
            after,
        )
        final_outcome = "policy_violation" if violations else outcome
        evidence_after: dict[str, Any]
        try:
            verified = verify_evidence(self.config)
            evidence_after = {
                "valid": True,
                "last_global_seq": verified["last_global_seq"],
                "last_event_sha256": verified["last_event_sha256"],
            }
        except EvidenceInvalidError as error:
            evidence_after = {"valid": False, "error": str(error)}
            if final_outcome == "success":
                final_outcome = "failed"
        released_lease = self.leases.release(run_id, lease_token)
        receipt = {
            **existing,
            "status": "finished",
            "outcome": final_outcome,
            "requested_outcome": outcome,
            "exit_code": exit_code,
            "summary_ref_sha256": (
                hashlib.sha256(summary_ref.encode()).hexdigest()
                if summary_ref
                else None
            ),
            "finished_at": utc_now(),
            "repositories_after": after,
            "git_policy_violations": violations,
            "evidence_after": evidence_after,
            "projection_generation_id": projection_generation_id,
            "projection_error": projection_error,
        }
        atomic_write_json(receipt_path, receipt)
        event_result = self._emit(
            "steward_run_finished",
            aggregate_type="steward_run",
            aggregate_id=run_id,
            payload={
                "actor": existing.get("actor"),
                "outcome": final_outcome,
                "exit_code": exit_code,
                "git_policy_violations": violations,
                "evidence_valid": evidence_after.get("valid"),
                "receipt": receipt,
            },
            idempotency_key=f"run_finished:{run_id}:{final_outcome}",
        )
        lease_event = self._emit(
            "steward_lease_released",
            aggregate_type="steward_lease",
            aggregate_id=run_id,
            payload={
                "actor": existing.get("actor"),
                "run_id": run_id,
                "outcome": final_outcome,
                "token_sha256": released_lease.get("token_sha256"),
            },
            idempotency_key=f"lease_released:{run_id}",
        )
        return {
            "receipt": receipt,
            "event": event_result,
            "lease_event": lease_event,
            "idempotent": False,
        }

    def project(
        self,
        *,
        run_id: str,
        lease_token: str,
        actor: str,
        phase: str = "manual",
    ) -> dict[str, Any]:
        heartbeat = self.leases.heartbeat(run_id, lease_token)
        if heartbeat["stop_requested"]:
            raise ProjectionError("projection denied after pause was requested")
        lease = self.leases.lease()
        if lease is None or lease.get("actor") != actor:
            raise LeaseError("projection actor does not own the active lease")
        manifest = self.projections.run(actor=actor, run_id=run_id, phase=phase)
        event_result = self._emit(
            "projection_generation_published",
            aggregate_type="projection_generation",
            aggregate_id=str(manifest["generation_id"]),
            payload={
                "actor": actor,
                "run_id": run_id,
                "phase": phase,
                "status": manifest["status"],
                "manifest_sha256": sha256_canonical(manifest),
            },
            idempotency_key=f"projection:{manifest['generation_id']}",
        )
        return {**manifest, "event": event_result}

    def reconcile(self) -> dict[str, Any]:
        stale = self.leases.reap_stale()
        if stale:
            self._emit(
                "steward_run_abandoned",
                aggregate_type="steward_run",
                aggregate_id=str(stale.get("run_id") or "unknown"),
                payload={
                    "actor": "reconciler",
                    "prior_actor": stale.get("actor"),
                    "reason": "stale_lease_reaped",
                },
                idempotency_key=f"abandoned:{stale.get('run_id')}",
            )
        events = self.events.reconcile()
        return {"stale_lease": stale, "events": events}
