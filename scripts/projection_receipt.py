"""Bounded receipts for source-first projector commands."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import time
from typing import Any

try:
    from authority_state import ArtifactAuthorityStateV1
except ModuleNotFoundError:
    from scripts.authority_state import ArtifactAuthorityStateV1


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _bounded_mapping(value: Any, *, byte_limit: int = 32 * 1024) -> dict[str, Any]:
    if not isinstance(value, dict):
        return {}
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":"))
    return value if len(encoded.encode()) <= byte_limit else {}


def projector_receipt(
    projector: str,
    status: dict[str, Any],
    outputs: dict[str, Path],
    *,
    started_monotonic: float,
) -> dict[str, Any]:
    output_hashes = {
        label: _sha256_file(path)
        for label, path in sorted(outputs.items())
        if path.is_file()
    }
    missing = sorted(set(outputs) - set(output_hashes))
    counters = _bounded_mapping(status.get("summary"))
    if not counters:
        counters = _bounded_mapping(status.get("counter_audit"))
    return {
        "schema": "projection_step_command_receipt_v1",
        "schema_version": 1,
        "projector": projector,
        "status": "passed" if not missing else "failed",
        "valid": not missing and status.get("valid", True) is not False,
        "counters": counters,
        "output_hashes": output_hashes,
        "missing_outputs": missing,
        "duration_ms": int((time.monotonic() - started_monotonic) * 1000),
        "artifact_authority_state_v1": (
            ArtifactAuthorityStateV1.evidence_only().canonical_record()
        ),
        "raw_output_included": False,
    }
