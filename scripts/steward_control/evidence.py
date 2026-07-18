"""Canonical store and legacy-source verification."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

from .config import ControlConfig
from .errors import EvidenceInvalidError


def _sha256_file(path: Path) -> str | None:
    try:
        handle = path.open("rb")
    except OSError:
        return None
    digest = hashlib.sha256()
    with handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def verify_evidence(config: ControlConfig, *, require_active: bool = True) -> dict[str, Any]:
    try:
        from evidence_store import EvidenceEventStore
    except ModuleNotFoundError:
        from scripts.evidence_store import EvidenceEventStore

    store = EvidenceEventStore(config.store_root)
    verification = store.verify()
    activation: dict[str, Any] = {}
    try:
        raw_activation = json.loads(store.activation_path.read_text(encoding="utf-8"))
        if isinstance(raw_activation, dict):
            activation = raw_activation
    except (OSError, json.JSONDecodeError):
        pass
    migration: dict[str, Any] = {}
    try:
        raw_migration = json.loads(
            (config.store_root / "migration_receipt.json").read_text(encoding="utf-8")
        )
        if isinstance(raw_migration, dict):
            migration = raw_migration
    except (OSError, json.JSONDecodeError):
        pass

    immutable_sources: list[dict[str, Any]] = []
    for source in migration.get("sources") or []:
        if not isinstance(source, dict):
            continue
        path = Path(str(source.get("path") or ""))
        expected = str(source.get("sha256") or "")
        actual = _sha256_file(path)
        immutable_sources.append(
            {
                "stream": source.get("stream"),
                "exists": path.is_file(),
                "expected_sha256": expected or None,
                "actual_sha256": actual,
                "immutable": bool(expected and actual == expected),
            }
        )
    active = activation.get("active_store") == "v2"
    immutable = bool(immutable_sources) and all(
        source["immutable"] for source in immutable_sources
    )
    result = {
        "schema": "steward_control_evidence_verification_v1",
        "schema_version": 1,
        "active_store": activation.get("active_store"),
        "active": active,
        "valid": verification.valid,
        "errors": list(verification.errors),
        "event_count": verification.event_count,
        "last_global_seq": verification.last_global_seq,
        "last_event_sha256": verification.last_event_sha256,
        "stream_counts": verification.stream_counts,
        "legacy_sources": immutable_sources,
        "v1_immutable": immutable,
    }
    if require_active and (not active or not verification.valid or not immutable):
        reasons = []
        if not active:
            reasons.append("active_store_not_v2")
        if not verification.valid:
            reasons.append("invalid_hash_chain")
        if not immutable:
            reasons.append("v1_source_hash_changed")
        raise EvidenceInvalidError(",".join(reasons))
    return result
