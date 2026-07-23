"""Deterministic offline codec fixtures over explicitly captured Signal Spine vectors."""

from __future__ import annotations

import json
import math
from pathlib import Path
from typing import Any

try:
    from experiential_systems.common import (
        authority_state,
        canonical_json,
        sha256_bytes,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        authority_state,
        canonical_json,
        sha256_bytes,
    )


def _metrics(vector: list[float], *, dropped: int, representation_loss: float) -> dict[str, float]:
    lane = vector[40:44]
    lane_energy = math.sqrt(sum(value * value for value in lane) / len(lane))
    total_energy = math.sqrt(
        sum(value * value for value in vector) / max(1, len(vector))
    )
    max_abs = max((abs(value) for value in vector), default=0.0)
    clamp_occupancy = sum(abs(value) >= 4.999 for value in vector) / max(
        1, len(vector)
    )
    return {
        "lane_energy": lane_energy,
        "total_energy": total_energy,
        "headroom": max(0.0, 5.0 - max_abs),
        "clamp_occupancy": clamp_occupancy,
        "dropped_dimension_count": float(dropped),
        "representation_loss": representation_loss,
    }


def _fixture_path(root: Path, receipt: dict[str, Any]) -> Path | None:
    ref = receipt.get("capture_fixture_ref_v1")
    if not isinstance(ref, dict):
        return None
    relative = ref.get("relative_path")
    if not isinstance(relative, str):
        return None
    path = (root / relative).resolve()
    try:
        path.relative_to(root.resolve())
    except ValueError:
        return None
    return path if path.is_file() else None


def _identity(receipt: dict[str, Any]) -> tuple[str, str]:
    process = receipt.get("process_identity_v1")
    process_hash = sha256_bytes(canonical_json(process or {}).encode("utf-8"))
    deployment = process.get("deployment_identity") if isinstance(process, dict) else None
    deployment_hash = sha256_bytes(str(deployment or "unavailable").encode("utf-8"))
    return process_hash, deployment_hash


def narrative_lane_samples(workspace: Path, window_id: str) -> list[dict[str, Any]]:
    root = workspace / "diagnostics/signal_spine_v1"
    rows: list[dict[str, Any]] = []
    for journey_path in sorted((root / "journeys").glob("*.json")):
        journey = json.loads(journey_path.read_text(encoding="utf-8"))
        feedback = [
            receipt
            for receipt in journey.get("receipts") or []
            if receipt.get("stage_kind") == "feedback"
            and receipt.get("capture_fixture_ref_v1")
        ]
        if not feedback:
            continue
        receipt = feedback[-1]
        fixture_path = _fixture_path(root, receipt)
        if fixture_path is None:
            continue
        fixture = json.loads(fixture_path.read_text(encoding="utf-8"))
        vector = fixture.get("vector")
        if not isinstance(vector, list) or len(vector) != 48:
            continue
        current = [float(value) for value in vector]
        candidate = list(current)
        candidate[40:44] = [0.0] * 4
        loss = math.sqrt(sum(value * value for value in current[40:44]) / 4)
        process_hash, deployment_hash = _identity(receipt)
        common = {
            "schema": "codec_study_sample_v1",
            "schema_version": 1,
            "window_id": window_id,
            "sample_kind": "codec_lane",
            "journey_id": journey.get("journey_id"),
            "process_identity_sha256": process_hash,
            "deployment_identity_sha256": deployment_hash,
            "source_fixture_sha256": fixture.get("vector_sha256"),
            "raw_prose_included": False,
            "counterfactual_dispatched": False,
            "causation_established": False,
            "artifact_authority_state_v1": authority_state(),
        }
        for cohort, values, dropped, representation_loss in (
            ("current_codec", current, 0, 0.0),
            ("leave_narrative_lane_40_44_out", candidate, 4, loss),
        ):
            core = {
                "journey_id": journey.get("journey_id"),
                "cohort": cohort,
                "source_fixture_sha256": fixture.get("vector_sha256"),
            }
            rows.append(
                {
                    **common,
                    "sample_id": (
                        "codecsample_"
                        + sha256_bytes(canonical_json(core).encode("utf-8"))
                    ),
                    "cohort": cohort,
                    "metrics": _metrics(
                        values,
                        dropped=dropped,
                        representation_loss=representation_loss,
                    ),
                }
            )
    return rows


def gate_samples(workspace: Path, window_id: str) -> list[dict[str, Any]]:
    """Return exact runtime-captured gate pairs, never an inferred historical ablation."""

    sample_path = (
        workspace
        / "diagnostics/evidence_study_runtime_v1/samples"
        / f"{window_id}.jsonl"
    )
    if not sample_path.is_file():
        return []
    rows: list[dict[str, Any]] = []
    for raw in sample_path.read_text(encoding="utf-8").splitlines():
        if not raw.strip():
            continue
        value = json.loads(raw)
        if value.get("sample_kind") == "codec_gate":
            rows.append(value)
    return rows
