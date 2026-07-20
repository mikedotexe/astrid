"""Resumable V3 projection identities and atomic generation journals."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any, Iterable, Sequence

from .model import atomic_write_json, canonical_json, load_json, utc_now


def command_identity(commands: Sequence[Sequence[str]]) -> str:
    return hashlib.sha256(canonical_json(list(commands)).encode()).hexdigest()


def plan_identity(plan: dict[str, Any]) -> str:
    stable = {
        "profile": plan.get("profile"),
        "steps": plan.get("steps"),
    }
    return hashlib.sha256(canonical_json(stable).encode()).hexdigest()


def flatten_dependency_hashes(
    dependencies: Iterable[str],
    outputs_by_step: dict[str, dict[str, str]],
) -> dict[str, str]:
    return {
        f"{dependency}:{relative}": digest
        for dependency in sorted(dependencies)
        for relative, digest in sorted(outputs_by_step.get(dependency, {}).items())
    }


def projection_input_identity(
    *,
    input_streams: Iterable[str],
    input_stream_watermarks: dict[str, dict[str, Any]],
    source_hashes: dict[str, str],
    dependency_output_hashes: dict[str, str],
    command_sha256: str,
    config_sha256: str,
    projector_version: int,
) -> dict[str, Any]:
    return {
        "schema": "projection_input_identity_v3",
        "schema_version": 3,
        "input_streams": sorted(input_streams),
        "input_stream_watermarks": input_stream_watermarks,
        "source_hashes": dict(sorted(source_hashes.items())),
        "dependency_output_hashes": dict(
            sorted(dependency_output_hashes.items())
        ),
        "command_sha256": command_sha256,
        "config_sha256": config_sha256,
        "projector_version": projector_version,
    }


class ProjectionJournalStore:
    """Atomic resumability journal; failed histories are retained."""

    SCHEMA = "projection_generation_journal_v1"

    def __init__(self, root: Path):
        self.root = Path(root)

    def path(self, generation_id: str) -> Path:
        return self.root / f"{generation_id}.json"

    def load(self, generation_id: str) -> dict[str, Any] | None:
        return load_json(self.path(generation_id))

    def write(
        self,
        *,
        generation_id: str,
        plan_sha256: str,
        config_sha256: str,
        phase: str,
        run_id: str,
        actor: str,
        status: str,
        step_receipts: list[dict[str, Any]],
        active_step_id: str | None = None,
        command_receipts: list[dict[str, Any]] | None = None,
        resume_source_generation_id: str | None = None,
        reason: str | None = None,
    ) -> dict[str, Any]:
        value = {
            "schema": self.SCHEMA,
            "schema_version": 1,
            "generation_id": generation_id,
            "plan_sha256": plan_sha256,
            "config_sha256": config_sha256,
            "phase": phase,
            "run_id": run_id,
            "actor": actor,
            "status": status,
            "active_step_id": active_step_id,
            "completed_step_count": len(step_receipts),
            "step_receipts": step_receipts,
            "command_receipts": command_receipts or [],
            "resume_source_generation_id": resume_source_generation_id,
            "reason": reason,
            "updated_at": utc_now(),
            "raw_output_included": False,
        }
        atomic_write_json(self.path(generation_id), value)
        return value

    @staticmethod
    def resumable(
        journal: dict[str, Any],
        *,
        plan_sha256: str,
        config_sha256: str,
    ) -> tuple[bool, str]:
        if journal.get("schema") != ProjectionJournalStore.SCHEMA:
            return False, "journal_schema_mismatch"
        if journal.get("plan_sha256") != plan_sha256:
            return False, "plan_changed"
        if journal.get("config_sha256") != config_sha256:
            return False, "config_changed"
        if journal.get("status") == "passed":
            return False, "generation_already_passed"
        receipts = journal.get("step_receipts")
        if not isinstance(receipts, list):
            return False, "journal_receipts_invalid"
        return True, "journal_compatible"


def bounded_receipt(value: dict[str, Any]) -> dict[str, Any]:
    """Keep only stable counters and hashes from a projector's JSON receipt."""

    allowed_scalars = (
        "schema",
        "schema_version",
        "status",
        "valid",
        "event_count",
        "appended_count",
        "reused_count",
        "changed_count",
        "output_sha256",
        "projection_sha256",
        "watermark",
    )
    receipt = {
        key: value[key]
        for key in allowed_scalars
        if key in value
        and isinstance(value[key], (str, int, float, bool, type(None)))
    }
    for key in ("summary", "counters", "output_hashes", "authority_state"):
        child = value.get(key)
        if isinstance(child, dict):
            encoded = json.dumps(child, sort_keys=True, separators=(",", ":"))
            if len(encoded.encode()) <= 32 * 1024:
                receipt[key] = child
    receipt["receipt_sha256"] = hashlib.sha256(
        canonical_json(value).encode()
    ).hexdigest()
    receipt["raw_output_included"] = False
    return receipt
