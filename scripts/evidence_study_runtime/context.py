"""Resolve bounded study context without turning association into causation."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Iterable

try:
    from experiential_systems.common import (
        RecordValidationError,
        sha256_bytes,
        validate_bounded_identifier,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError,
        sha256_bytes,
        validate_bounded_identifier,
    )

from .config import state_dir
from .model import EvidenceStudyPlanV1, StudyWindowReceiptV1, StudyWindowSpecV1


def _jsonl_rows(path: Path) -> Iterable[dict[str, Any]]:
    if not path.is_file():
        return ()
    rows = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        if not raw.strip():
            continue
        try:
            value = json.loads(raw)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            rows.append(value)
    return rows


def _bounded_refs(values: Iterable[str], *, fallback: str = "unavailable") -> list[str]:
    result: list[str] = []
    for value in values:
        try:
            bounded = validate_bounded_identifier(value, "context_ref", limit=200)
        except RecordValidationError:
            continue
        if bounded and bounded not in result:
            result.append(bounded)
    return result or [fallback]


def _contains_exact(value: Any, target: str) -> bool:
    if isinstance(value, dict):
        return any(_contains_exact(item, target) for item in value.values())
    if isinstance(value, list):
        return any(_contains_exact(item, target) for item in value)
    return value == target


def _witness_record(workspace: Path, witness_id: str) -> dict[str, Any] | None:
    path = workspace / "diagnostics/lived_state_witness_v1/witnesses.jsonl"
    for row in _jsonl_rows(path):
        if row.get("aggregate_id") == witness_id:
            return row
        witness = row.get("witness")
        if isinstance(witness, dict) and witness.get("witness_id") == witness_id:
            return row
    return None


def _fixture_rows(
    workspace: Path, receipt: StudyWindowReceiptV1
) -> list[dict[str, Any]]:
    path = state_dir(workspace) / receipt.scalar_fixture_ref
    if not path.is_file():
        return []
    raw = path.read_bytes()
    if sha256_bytes(raw) != receipt.scalar_fixture_sha256:
        return []
    try:
        value = json.loads(raw)
    except json.JSONDecodeError:
        return []
    samples = value.get("samples") if isinstance(value, dict) else None
    return [item for item in samples or [] if isinstance(item, dict)]


def resolve_observation_context(
    workspace: Path,
    plan: EvidenceStudyPlanV1,
    spec: StudyWindowSpecV1,
    receipt: StudyWindowReceiptV1,
) -> dict[str, Any]:
    """Return exact receipt refs where available and explicit temporal refs otherwise."""

    witness_row = _witness_record(workspace, plan.witness_id)
    witness = witness_row.get("witness") if isinstance(witness_row, dict) else None
    witness = witness if isinstance(witness, dict) else {}
    alignment = witness_row.get("alignment") if isinstance(witness_row, dict) else None
    alignment = alignment if isinstance(alignment, dict) else {}
    process = witness.get("observed_process_v1")
    process = process if isinstance(process, dict) else {}

    witness_refs = [plan.witness_id]
    deployment_receipt = alignment.get("deployment_receipt_id")
    if isinstance(deployment_receipt, str):
        witness_refs.append(f"deployment:{deployment_receipt}")
    process_identity = process.get("process_identity_sha256")
    if isinstance(process_identity, str):
        witness_refs.append(f"process:{process_identity}")

    model_refs: list[str] = []
    for route in witness.get("model_routes_v1") or []:
        if not isinstance(route, dict):
            continue
        qos_identity = route.get("qos_request_identity_sha256")
        call_id = route.get("call_id")
        if isinstance(qos_identity, str):
            model_refs.append(f"qos_request:{qos_identity}")
        if isinstance(call_id, str):
            model_refs.append(f"model_call:{call_id}")

    representation_refs = []
    transitions = (
        workspace
        / "diagnostics/representation_contracts_v1/transitions.jsonl"
    )
    for row in _jsonl_rows(transitions):
        if row.get("source_witness_id") != plan.witness_id:
            continue
        receipt_id = row.get("receipt_id")
        if isinstance(receipt_id, str):
            representation_refs.append(receipt_id)

    reciprocal_refs = []
    reciprocal = (
        workspace
        / "diagnostics/reciprocal_uptake_v1/current_receipts.jsonl"
    )
    for row in _jsonl_rows(reciprocal):
        if not _contains_exact(row, plan.witness_id):
            continue
        receipt_id = row.get("receipt_id")
        if isinstance(receipt_id, str):
            reciprocal_refs.append(receipt_id)

    samples = _fixture_rows(workspace, receipt)
    signal_refs = []
    if spec.signal_capture_window_ref:
        signal_refs.append(spec.signal_capture_window_ref)
    signal_refs.extend(
        f"signal_journey:{journey_id}"
        for journey_id in (
            sample.get("journey_id") for sample in samples
        )
        if isinstance(journey_id, str)
    )

    telemetry_refs: list[str] = []
    if plan.sample_kind in {"telemetry", "heartbeat"} and samples:
        telemetry_refs.append(f"temporal_window:{spec.window_id}")
        telemetry_refs.extend(
            f"temporal_connection:{connection_id}"
            for connection_id in (
                sample.get("connection_id") for sample in samples
            )
            if isinstance(connection_id, int) and not isinstance(connection_id, bool)
        )

    return {
        "witness_context_refs": _bounded_refs(witness_refs)[:8],
        "representation_transition_refs": _bounded_refs(representation_refs)[:8],
        "model_qos_refs": _bounded_refs(model_refs)[:8],
        "reciprocal_state_refs": _bounded_refs(reciprocal_refs)[:8],
        "signal_stage_refs": _bounded_refs(signal_refs)[:8],
        "telemetry_relation": (
            "temporal_window" if telemetry_refs else "unavailable"
        ),
        "minime_telemetry_refs": _bounded_refs(telemetry_refs)[:8],
    }
