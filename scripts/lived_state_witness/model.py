"""Validation and bounded identities for lived-state witness receipts."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import re
from typing import Any, Iterable

try:
    from evidence_store.model import canonical_json
except ModuleNotFoundError:
    from scripts.evidence_store.model import canonical_json

WITNESS_ID_RE = re.compile(r"^lsw_[0-9a-f]{64}$")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
REPORT_TIMESTAMP_RE = re.compile(r"_(\d{9,})\.txt$")
WITNESS_HEADER_RE = re.compile(
    r"(?m)^Lived-state witness:\s*(lsw_[0-9a-f]{64})\s*$"
)
RECONCILIATION_OUTCOMES = frozenset(
    {
        "same_deployment",
        "same_source_new_process",
        "source_changed_not_deployed",
        "deployed_changed",
        "temporal_association_only",
        "deployment_unknown",
        "historical_unrecoverable",
    }
)


def authority_state() -> dict[str, Any]:
    return {
        "schema": "artifact_authority_state_v1",
        "schema_version": 1,
        "state": "evidence_only",
        "witness_only": True,
        "live_eligible_now": False,
        "auto_approved": False,
        "grants_approval": False,
        "edits_source_now": False,
    }


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def deterministic_id(namespace: str, values: Iterable[Any]) -> str:
    identity = canonical_json([namespace, *values]).encode()
    return f"{namespace}_{sha256_bytes(identity)}"


def report_timestamp(path: Path) -> int | None:
    match = REPORT_TIMESTAMP_RE.search(path.name)
    return int(match.group(1)) if match else None


def canonical_report_paths(workspace: Path) -> list[Path]:
    root = workspace / "introspections"
    rows = [
        (report_timestamp(path), path.name, path)
        for path in root.glob("introspection_*.txt")
        if report_timestamp(path) is not None
    ]
    rows.sort(key=lambda row: (int(row[0] or 0), row[1]))
    return [row[2] for row in rows]


def report_header(path: Path) -> dict[str, str]:
    metadata: dict[str, str] = {}
    with path.open("r", encoding="utf-8", errors="replace") as handle:
        for _ in range(40):
            line = handle.readline()
            if not line:
                break
            if line.strip() in {"Observed:", "Likely Snags:"}:
                break
            if ":" not in line:
                continue
            key, value = line.split(":", 1)
            normalized = key.strip().lower().replace("-", "_").replace(" ", "_")
            if normalized:
                metadata[normalized] = value.strip()
    return metadata


def witness_pointer(path: Path) -> str | None:
    with path.open("r", encoding="utf-8", errors="replace") as handle:
        header = "".join(handle.readline() for _ in range(20))
    match = WITNESS_HEADER_RE.search(header)
    return match.group(1) if match else None


def _privacy_path(value: Any, path: str = "$") -> str | None:
    if isinstance(value, dict):
        for key, child in value.items():
            rejection = _privacy_path(child, f"{path}.{key}")
            if rejection:
                return rejection
    elif isinstance(value, list):
        for index, child in enumerate(value):
            rejection = _privacy_path(child, f"{path}[{index}]")
            if rejection:
                return rejection
    elif isinstance(value, str):
        if (
            value.startswith("/")
            or re.match(r"^[A-Za-z]:[\\/]", value)
        ):
            return f"{path}:private_absolute_path"
        if len(value) > 500:
            return f"{path}:unbounded_string"
    return None


def _valid_authority(value: Any) -> bool:
    allowed = {
        "schema",
        "schema_version",
        "state",
        "witness_only",
        "live_eligible_now",
        "auto_approved",
        "grants_approval",
        "edits_source_now",
    }
    return (
        isinstance(value, dict)
        and set(value) == allowed
        and value.get("schema") == "artifact_authority_state_v1"
        and value.get("schema_version") == 1
        and value.get("state") == "evidence_only"
        and value.get("witness_only") is True
        and value.get("live_eligible_now") is False
        and value.get("auto_approved") is False
        and value.get("grants_approval") is False
        and value.get("edits_source_now") is False
    )


def _schema_v1(value: dict[str, Any], schema: str, field: str, errors: list[str]) -> None:
    if value.get("schema") != schema:
        errors.append(f"{field}:schema")
    if value.get("schema_version") != 1:
        errors.append(f"{field}:schema_version")


def _integer(
    value: Any,
    field: str,
    errors: list[str],
    *,
    minimum: int = 0,
    optional: bool = False,
) -> None:
    if optional and value is None:
        return
    if not _is_integer(value, minimum=minimum):
        errors.append(f"{field}:invalid_integer")


def _is_integer(value: Any, *, minimum: int = 0) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= minimum


def _hash_field(value: Any, field: str, errors: list[str], *, optional: bool = False) -> None:
    if optional and value is None:
        return
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        errors.append(f"{field}:invalid_sha256")


def _unexpected_keys(
    value: dict[str, Any], allowed: set[str], field: str, errors: list[str]
) -> None:
    for key in sorted(set(value) - allowed):
        errors.append(f"{field}.{key}:unexpected_field")


def _bounded_string(
    value: Any, field: str, errors: list[str], maximum: int, *, optional: bool = False
) -> None:
    if optional and value is None:
        return
    if not isinstance(value, str) or not value or len(value) > maximum:
        errors.append(f"{field}:invalid_bounded_string")


def _validate_provenance_ref(
    value: Any, field: str, errors: list[str], *, optional: bool = False
) -> None:
    if optional and value is None:
        return
    if not isinstance(value, dict):
        errors.append(f"{field}:not_object")
        return
    _unexpected_keys(
        value,
        {
            "origin",
            "source_id",
            "canonical_sha256",
            "parent_ids",
            "timestamp_ms",
            "field_paths",
            "context_anchor_v1",
        },
        field,
        errors,
    )
    if value.get("origin") not in {
        "minime_observation",
        "bridge_derived",
        "astrid_interpretation",
        "mixed",
        "unknown",
    }:
        errors.append(f"{field}.origin:invalid")
    _bounded_string(value.get("source_id"), f"{field}.source_id", errors, 300)
    _hash_field(
        value.get("canonical_sha256"), f"{field}.canonical_sha256", errors
    )
    _integer(value.get("timestamp_ms"), f"{field}.timestamp_ms", errors, minimum=1)
    parent_ids = value.get("parent_ids")
    if not isinstance(parent_ids, list) or len(parent_ids) > 32:
        errors.append(f"{field}.parent_ids:invalid")
    else:
        for index, parent in enumerate(parent_ids):
            _bounded_string(
                parent, f"{field}.parent_ids[{index}]", errors, 160
            )
    field_paths = value.get("field_paths")
    if not isinstance(field_paths, list) or len(field_paths) > 64:
        errors.append(f"{field}.field_paths:invalid")
    else:
        for index, path in enumerate(field_paths):
            _bounded_string(
                path, f"{field}.field_paths[{index}]", errors, 200
            )
        if field_paths != sorted(set(field_paths)):
            errors.append(f"{field}.field_paths:not_canonical")
    anchor = value.get("context_anchor_v1")
    if not isinstance(anchor, dict):
        errors.append(f"{field}.context_anchor_v1:not_object")
        return
    _unexpected_keys(
        anchor,
        {
            "descriptor",
            "structural_signature_sha256",
            "influence_types",
            "private_payload_included",
        },
        f"{field}.context_anchor_v1",
        errors,
    )
    _bounded_string(
        anchor.get("descriptor"),
        f"{field}.context_anchor_v1.descriptor",
        errors,
        120,
    )
    expected_descriptor = {
        "minime_observation": "producer_telemetry_shape",
        "bridge_derived": "bridge_evidence_shape",
        "astrid_interpretation": "astrid_interpretive_context_shape",
        "mixed": "composed_witness_shape",
        "unknown": "unknown_context_shape",
    }.get(value.get("origin"))
    if expected_descriptor and anchor.get("descriptor") != expected_descriptor:
        errors.append(f"{field}.context_anchor_v1.descriptor:mismatch")
    _hash_field(
        anchor.get("structural_signature_sha256"),
        f"{field}.context_anchor_v1.structural_signature_sha256",
        errors,
    )
    influence_types = anchor.get("influence_types")
    if not isinstance(influence_types, list) or len(influence_types) > 8:
        errors.append(f"{field}.context_anchor_v1.influence_types:invalid")
    else:
        allowed_influences = {
            "regulatory_state_observed",
            "structural",
            "temporal",
            "interpretive",
            "stylistic_context",
            "authorship",
        }
        if (
            any(item not in allowed_influences for item in influence_types)
            or len(influence_types) != len(set(influence_types))
        ):
            errors.append(
                f"{field}.context_anchor_v1.influence_types:invalid_values"
            )
    if anchor.get("private_payload_included") is not False:
        errors.append(
            f"{field}.context_anchor_v1.private_payload_included:must_be_false"
        )


def validate_witness(value: Any) -> list[str]:
    from .validation import validate_witness as validate_untrusted_witness

    return validate_untrusted_witness(value)
def validate_gap(value: Any) -> list[str]:
    errors: list[str] = []
    if not isinstance(value, dict):
        return ["gap:not_object"]
    _unexpected_keys(
        value,
        {
            "schema",
            "schema_version",
            "gap_id",
            "witness_id",
            "reason",
            "detected_at_unix_ms",
            "sidecar_expected",
            "report_persistence_blocked",
            "artifact_authority_state_v1",
        },
        "gap",
        errors,
    )
    _schema_v1(value, "lived_state_gap_receipt_v1", "gap", errors)
    if (
        not isinstance(value.get("witness_id"), str)
        or WITNESS_ID_RE.fullmatch(value["witness_id"]) is None
    ):
        errors.append("gap:witness_id")
    if (
        not isinstance(value.get("gap_id"), str)
        or re.fullmatch(r"lsgap_[0-9a-f]{64}", value["gap_id"]) is None
    ):
        errors.append("gap:gap_id")
    _bounded_string(value.get("reason"), "gap.reason", errors, 160)
    _integer(value.get("detected_at_unix_ms"), "gap.detected_at_unix_ms", errors, minimum=1)
    if value.get("sidecar_expected") is not True:
        errors.append("gap:sidecar_expected")
    if value.get("report_persistence_blocked") is not False:
        errors.append("gap:report_persistence_blocked")
    if not _valid_authority(value.get("artifact_authority_state_v1")):
        errors.append("gap:authority")
    if (
        isinstance(value.get("gap_id"), str)
        and isinstance(value.get("witness_id"), str)
        and isinstance(value.get("reason"), str)
        and _is_integer(value.get("detected_at_unix_ms"), minimum=1)
    ):
        expected_gap_id = "lsgap_" + sha256_bytes(
            (
                f"{value['witness_id']}\0{value['reason']}\0"
                f"{value['detected_at_unix_ms']}"
            ).encode()
        )
        if value["gap_id"] != expected_gap_id:
            errors.append("gap:gap_id_mismatch")
    privacy_error = _privacy_path(value)
    if privacy_error:
        errors.append(privacy_error)
    return errors
