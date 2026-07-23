"""Cohort selection and owner-only scalar fixture assembly."""

from __future__ import annotations

from collections import Counter
import json
import math
from pathlib import Path
from typing import Any

try:
    from experiential_systems.common import (
        canonical_json,
        owner_atomic_write_json,
        sha256_bytes,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        canonical_json,
        owner_atomic_write_json,
        sha256_bytes,
    )

from .codec import gate_samples, narrative_lane_samples
from .config import state_dir
from .model import EvidenceStudyPlanV1, StudyWindowReceiptV1, StudyWindowSpecV1
from .review import StudyCaptureGapReceiptV1
from .storage import load_samples


def _contains_vector(value: Any) -> bool:
    if isinstance(value, list):
        return any(_contains_vector(item) for item in value)
    if not isinstance(value, dict):
        return False
    return any(
        str(key).lower() in {"vector", "features", "embedding"}
        or _contains_vector(item)
        for key, item in value.items()
    )


def _cohort(plan: EvidenceStudyPlanV1, row: dict[str, Any]) -> str | None:
    explicit = row.get("cohort")
    if explicit in {plan.baseline_cohort, plan.candidate_cohort}:
        return str(explicit)
    if plan.sample_kind == "telemetry":
        classification = row.get("classification")
        if classification == "clear_at_latest_sample":
            return plan.baseline_cohort
        if classification in {
            "write_lock_wait_observed",
            "write_lock_hold_observed",
            "prewrite_pipeline_heavy",
        }:
            return plan.candidate_cohort
    if plan.sample_kind == "heartbeat":
        if (
            row.get("admission") == "admitted"
            and row.get("enqueue_outcome") == "enqueued"
        ):
            return plan.baseline_cohort
        if row.get("admission") in {"blocked", "rejected"} or row.get(
            "enqueue_outcome"
        ) in {"channel_closed", "enqueue_rejected", "not_attempted"}:
            return plan.candidate_cohort
    return None


def _identity_pair(row: dict[str, Any]) -> tuple[str, str] | None:
    process = row.get("process_identity_sha256")
    deployment = row.get("deployment_identity_sha256")
    if (
        isinstance(process, str)
        and len(process) == 64
        and all(character in "0123456789abcdef" for character in process)
        and isinstance(deployment, str)
        and len(deployment) == 64
        and all(character in "0123456789abcdef" for character in deployment)
    ):
        return process, deployment
    return None


def _finite_number(value: Any) -> bool:
    return (
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and math.isfinite(float(value))
        and abs(float(value)) < 1.0e12
    )


def _valid_sample(
    plan: EvidenceStudyPlanV1,
    row: dict[str, Any],
    source: StudyWindowSpecV1,
) -> bool:
    if (
        not isinstance(row.get("sample_id"), str)
        or not row["sample_id"]
        or len(row["sample_id"]) > 240
        or row.get("sample_kind") != plan.sample_kind
        or _identity_pair(row) is None
    ):
        return False
    metrics = row.get("metrics")
    if not isinstance(metrics, dict) or any(
        value is not None and not _finite_number(value)
        for value in metrics.values()
    ):
        return False
    if plan.sample_kind in {"telemetry", "heartbeat", "codec_gate"}:
        observed = row.get("observed_at_unix_ms")
        monotonic = row.get("monotonic_time_ns")
        if (
            isinstance(observed, bool)
            or not isinstance(observed, int)
            or not source.started_at_unix_ms
            <= observed
            <= source.expires_at_unix_ms
            or isinstance(monotonic, bool)
            or not isinstance(monotonic, int)
            or monotonic <= 0
        ):
            return False
    required_metrics = {
        "telemetry": {
            "integration_us",
            "prewrite_us",
            "write_lock_wait_us",
            "write_lock_hold_us",
        },
        "heartbeat": {"cadence_seconds", "intensity", "phase_code"},
        "codec_lane": {
            "lane_energy",
            "headroom",
            "clamp_occupancy",
            "representation_loss",
        },
        "codec_gate": {
            "lane_energy",
            "headroom",
            "clamp_occupancy",
            "representation_loss",
        },
    }[plan.sample_kind]
    if any(not _finite_number(metrics.get(name)) for name in required_metrics):
        return False
    if plan.sample_kind == "codec_gate" and (
        row.get("counterfactual_dispatched") is not False
        or row.get("behavior_changed") is not False
        or row.get("felt_outcome_inferred") is not False
    ):
        return False
    return True


def _bounded_row(
    row: dict[str, Any], cohort: str, metric_names: tuple[str, ...]
) -> dict[str, Any]:
    metrics = row.get("metrics")
    metrics = metrics if isinstance(metrics, dict) else {}
    return {
        "sample_id": row.get("sample_id"),
        "sample_kind": row.get("sample_kind"),
        "cohort": cohort,
        "observed_at_unix_ms": row.get("observed_at_unix_ms"),
        "monotonic_time_ns": row.get("monotonic_time_ns"),
        "classification": row.get("classification"),
        "connection_id": row.get("connection_id"),
        "telemetry_t_ms": row.get("telemetry_t_ms"),
        "admission": row.get("admission"),
        "enqueue_outcome": row.get("enqueue_outcome"),
        "journey_id": row.get("journey_id"),
        "source_fixture_sha256": row.get("source_fixture_sha256"),
        "process_identity_sha256": row.get("process_identity_sha256"),
        "deployment_identity_sha256": row.get("deployment_identity_sha256"),
        "metrics": {
            name: float(metrics[name])
            for name in metric_names
            if isinstance(metrics.get(name), (int, float))
            and not isinstance(metrics.get(name), bool)
        },
    }


def _gap(
    spec: StudyWindowSpecV1,
    reason: str,
    *,
    dropped: int = 0,
) -> StudyCaptureGapReceiptV1:
    return StudyCaptureGapReceiptV1.build(
        window_id=spec.window_id,
        study_id=spec.study_id,
        reason=reason,
        dropped_sample_count=dropped,
        observed_at_unix_ms=max(spec.started_at_unix_ms, spec.expires_at_unix_ms),
        source_ref=spec.window_id,
    )


def assemble(
    workspace: Path,
    plan: EvidenceStudyPlanV1,
    spec: StudyWindowSpecV1,
    source_specs: list[StudyWindowSpecV1] | None = None,
) -> tuple[list[StudyWindowReceiptV1], list[StudyCaptureGapReceiptV1]]:
    if plan.plan_id != spec.plan_id or plan.plan_sha256 != spec.plan_sha256:
        raise ValueError("capture window does not match frozen plan")
    sources = source_specs or [spec]
    if not sources or sources[-1] != spec:
        raise ValueError("capture source lineage must end at the assembled window")
    if any(
        item.plan_id != plan.plan_id or item.plan_sha256 != plan.plan_sha256
        for item in sources
    ):
        raise ValueError("capture source lineage mixes frozen plans")
    errors: list[str] = []
    if plan.sample_kind == "codec_lane":
        rows = narrative_lane_samples(workspace, spec.window_id)
    elif plan.sample_kind == "codec_gate":
        rows = [
            row
            for source in sources
            for row in gate_samples(workspace, source.window_id)
        ]
    else:
        rows = []
        for source in sources:
            source_rows, source_errors = load_samples(
                workspace, source.window_id
            )
            rows.extend(source_rows)
            errors.extend(
                f"{source.window_id}:{error}" for error in source_errors
            )
    writer_gap_rows = [
        row for row in rows if row.get("schema") == "study_capture_gap_receipt_v1"
    ]
    rows = [
        row for row in rows if row.get("schema") != "study_capture_gap_receipt_v1"
    ]
    observed_row_count = len(rows)
    source_by_window = {item.window_id: item for item in sources}
    validated_rows = []
    for row in rows[: plan.sample_limit]:
        source = source_by_window.get(str(row.get("window_id") or ""))
        if source is None and plan.sample_kind == "codec_lane":
            source = spec
        if (
            source is None
            or _contains_vector(row)
            or not _valid_sample(plan, row, source)
        ):
            errors.append("invalid_or_out_of_window_sample")
            continue
        validated_rows.append(row)
    rows = validated_rows
    gaps: list[StudyCaptureGapReceiptV1] = []
    for row in writer_gap_rows:
        reason = str(row.get("reason") or "asynchronous_write_failed")
        if reason not in {
            "queue_exhausted",
            "writer_disconnected",
            "asynchronous_write_failed",
            "crash_recovery_gap",
            "sample_limit_reached",
            "identity_mismatch",
            "required_cohort_missing",
        }:
            reason = "asynchronous_write_failed"
        gaps.append(
            _gap(
                spec,
                reason,
                dropped=int(row.get("dropped_sample_count") or 0),
            )
        )
    if errors:
        gaps.append(_gap(spec, "asynchronous_write_failed", dropped=len(errors)))
    if observed_row_count > plan.sample_limit:
        gaps.append(
            _gap(
                spec,
                "sample_limit_reached",
                dropped=observed_row_count - plan.sample_limit,
            )
        )

    grouped: dict[tuple[str, str], list[tuple[dict[str, Any], str]]] = {}
    for row in rows:
        cohort = _cohort(plan, row)
        identity = _identity_pair(row)
        if cohort is not None and identity is not None:
            grouped.setdefault(identity, []).append((row, cohort))
    eligible = [
        (identity, values)
        for identity, values in grouped.items()
        if {cohort for _, cohort in values}
        == {plan.baseline_cohort, plan.candidate_cohort}
    ]
    if eligible:
        identity, selected = max(
            eligible, key=lambda item: (len(item[1]), item[0])
        )
        relation = "exact_identity"
    else:
        identity = (None, None)
        selected = []
        relation = "identity_unavailable"
        gaps.append(_gap(spec, "identity_mismatch", dropped=len(rows)))

    cohort_rows = {
        cohort: [
            _bounded_row(row, resolved, plan.metric_names)
            for row, resolved in selected
            if resolved == cohort
        ]
        for cohort in (plan.baseline_cohort, plan.candidate_cohort)
    }
    counts = Counter(resolved for _, resolved in selected)
    total_sufficient = (
        len(selected) >= plan.minimum_total_samples
        and counts[plan.baseline_cohort] >= plan.minimum_baseline_samples
        and counts[plan.candidate_cohort] >= plan.minimum_candidate_samples
    )
    if not total_sufficient and not any(
        item.reason == "required_cohort_missing" for item in gaps
    ):
        gaps.append(_gap(spec, "required_cohort_missing"))

    receipts: list[StudyWindowReceiptV1] = []
    fixture_root = state_dir(workspace) / "scalar_fixtures"
    for role, cohort in (
        ("baseline", plan.baseline_cohort),
        ("candidate", plan.candidate_cohort),
    ):
        fixture = {
            "schema": "study_scalar_fixture_v1",
            "schema_version": 1,
            "window_id": spec.window_id,
            "source_window_ids": [item.window_id for item in sources],
            "plan_id": plan.plan_id,
            "role": role,
            "cohort": cohort,
            "samples": cohort_rows[cohort],
            "raw_prose_included": False,
            "full_vector_included": False,
        }
        encoded = (canonical_json(fixture) + "\n").encode("utf-8")
        fixture_hash = sha256_bytes(encoded)
        fixture_path = fixture_root / f"{fixture_hash}.json"
        try:
            from experiential_systems.common import owner_atomic_write
        except ModuleNotFoundError:
            from scripts.experiential_systems.common import owner_atomic_write
        owner_atomic_write(fixture_path, encoded)
        sample_hash = sha256_bytes(
            canonical_json(cohort_rows[cohort]).encode("utf-8")
        )
        receipts.append(
            StudyWindowReceiptV1.build(
                window_id=spec.window_id,
                campaign_id=spec.campaign_id,
                study_id=spec.study_id,
                plan_id=plan.plan_id,
                role=role,
                comparison_kind=plan.comparison_kind,
                cohort=cohort,
                sample_count=len(cohort_rows[cohort]),
                qualifying_sample_count=len(cohort_rows[cohort]),
                sample_set_sha256=sample_hash,
                scalar_fixture_ref=(
                    f"scalar_fixtures/{fixture_hash}.json"
                ),
                scalar_fixture_sha256=fixture_hash,
                process_identity_sha256=identity[0],
                deployment_identity_sha256=identity[1],
                identity_relation=relation,
                gap_refs=[item.gap_id for item in gaps],
                sufficient=total_sufficient and not gaps,
            )
        )
    return receipts, gaps


def load_fixture(workspace: Path, receipt: StudyWindowReceiptV1) -> list[dict[str, Any]]:
    path = state_dir(workspace) / receipt.scalar_fixture_ref
    raw = path.read_bytes()
    value = json.loads(raw)
    if sha256_bytes(raw) != receipt.scalar_fixture_sha256:
        raise ValueError("scalar fixture hash mismatch")
    samples = value.get("samples")
    if not isinstance(samples, list):
        raise ValueError("scalar fixture samples are invalid")
    return samples
