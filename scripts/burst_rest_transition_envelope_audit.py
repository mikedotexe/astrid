#!/usr/bin/env python3
"""Read-only audit of Astrid's burst-to-rest observation envelope.

The audit reports what the bridge status can establish without replaying or
mutating cadence, fill regulation, scheduling, controller state, or signals.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SOURCE = (
    ROOT
    / "capsules"
    / "spectral-bridge"
    / "src"
    / "autonomous"
    / "runtime"
    / "orchestration.rs"
)
DEFAULT_STATUS = Path(
    "/Users/v/other/minime/workspace/runtime/bridge_semantic_heartbeat_status.json"
)
EXPECTED_TRANSITION_SOURCE_SHA256 = (
    "b7ce517fb84b2dd97e760a4e7c0c1ea840e2c4c2e7c591e07c43c083dae8b694"
)
REGION_SEPARATOR = "\n--transition-region--\n"
TRANSITION_SOURCE_REGIONS = (
    (
        "const SEMANTIC_HEARTBEAT_INTERVAL",
        "pub(crate) const fn semantic_heartbeat_constants_v1",
    ),
    ("            let seed =", "            let wait = if burst_count"),
    (
        "                let rest_min =",
        "                // Fill-responsive rest adjustment",
    ),
    ("                let current_fill =", "                if current_fill < 30.0"),
    (
        "                let pulses = rest_secs / 5;",
        "                    previous_rest_features = Some(features.clone());",
    ),
)


class AuditError(RuntimeError):
    """Raised when an input or source contract has drifted."""


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def transition_source_sha256(source_text: str) -> str:
    regions: list[str] = []
    cursor = 0
    for start_marker, end_marker in TRANSITION_SOURCE_REGIONS:
        try:
            start = source_text.index(start_marker, cursor)
            end = source_text.index(end_marker, start)
        except ValueError as error:
            raise AuditError(
                f"transition source marker drifted: {start_marker!r} -> {end_marker!r}"
            ) from error
        regions.append(source_text[start:end])
        cursor = end
    return sha256_bytes(REGION_SEPARATOR.join(regions).encode("utf-8"))


def read_source_contract(path: Path) -> tuple[str, str]:
    source_bytes = path.read_bytes()
    try:
        source_text = source_bytes.decode("utf-8")
    except UnicodeDecodeError as error:
        raise AuditError(f"source is not UTF-8: {path}") from error
    relevant_sha256 = transition_source_sha256(source_text)
    if relevant_sha256 != EXPECTED_TRANSITION_SOURCE_SHA256:
        raise AuditError(
            "transition source contract drifted: "
            f"expected {EXPECTED_TRANSITION_SOURCE_SHA256}, observed {relevant_sha256}"
        )
    return sha256_bytes(source_bytes), relevant_sha256


def finite_float(value: Any) -> float | None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    number = float(value)
    return number if math.isfinite(number) else None


def nonnegative_int(value: Any) -> int | None:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        return None
    return value


def rest_duration_at_fill(base_rest_seconds: int, fill_pct: float) -> int:
    if fill_pct < 30.0:
        return max(int(float(base_rest_seconds) * 0.6), 30)
    if fill_pct < 40.0:
        return base_rest_seconds
    if fill_pct < 50.0:
        return min(int(float(base_rest_seconds) * 1.2), 360)
    return base_rest_seconds


def evenly_spaced_ints(start: int, end: int, count: int) -> list[int]:
    if count < 2:
        raise AuditError("an envelope needs at least two points")
    span = end - start
    return [round(start + (span * index / (count - 1))) for index in range(count)]


def percentile(values: list[float], fraction: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    index = max(0, math.ceil(fraction * len(ordered)) - 1)
    return ordered[index]


def rest_episodes(samples: list[dict[str, Any]]) -> list[list[dict[str, Any]]]:
    ordered = sorted(
        samples,
        key=lambda sample: finite_float(sample.get("at_unix_s")) or 0.0,
    )
    episodes: list[list[dict[str, Any]]] = []
    current: list[dict[str, Any]] | None = None
    previous_rest_step: int | None = None
    previous_rest_time: float | None = None

    for sample in ordered:
        if sample.get("source") != "autonomous_rest_pulse":
            continue
        step = nonnegative_int(sample.get("phase_step"))
        at_unix_s = finite_float(sample.get("at_unix_s"))
        starts_episode = (
            current is None
            or step == 0
            or (step is not None and previous_rest_step is not None and step <= previous_rest_step)
            or (
                at_unix_s is not None
                and previous_rest_time is not None
                and at_unix_s - previous_rest_time > 20.0
            )
        )
        if starts_episode:
            current = []
            episodes.append(current)
        current.append(sample)
        previous_rest_step = step
        previous_rest_time = at_unix_s
    return episodes


def consecutive_comparison(sample: dict[str, Any]) -> dict[str, Any]:
    signal = sample.get("signal_evidence_v1")
    if not isinstance(signal, dict):
        return {}
    comparison = signal.get("consecutive_comparison")
    return comparison if isinstance(comparison, dict) else {}


def analyze_status(
    status: dict[str, Any],
    *,
    status_sha256: str,
    source_sha256: str,
    relevant_source_sha256: str,
) -> dict[str, Any]:
    samples = status.get("window_samples_v1")
    if not isinstance(samples, list) or not all(isinstance(item, dict) for item in samples):
        raise AuditError("status window_samples_v1 must be an array of objects")

    episodes = rest_episodes(samples)
    episode_receipts: list[dict[str, Any]] = []
    intra_rest_deltas: list[float] = []
    fill_values: list[float] = []
    first_rest_comparison_count = 0

    for index, episode in enumerate(episodes):
        first = episode[0]
        first_comparison = consecutive_comparison(first)
        first_compared_dimensions = nonnegative_int(
            first_comparison.get("compared_dimension_count")
        )
        first_delta = finite_float(first_comparison.get("delta_rms_from_previous"))
        if first_compared_dimensions and first_delta is not None:
            first_rest_comparison_count += 1

        episode_deltas: list[float] = []
        for sample in episode:
            fill = finite_float(sample.get("fill_pct"))
            if fill is not None:
                fill_values.append(fill)
            comparison = consecutive_comparison(sample)
            compared_dimensions = nonnegative_int(comparison.get("compared_dimension_count"))
            delta = finite_float(comparison.get("delta_rms_from_previous"))
            if compared_dimensions and delta is not None:
                episode_deltas.append(delta)
                intra_rest_deltas.append(delta)

        episode_receipts.append(
            {
                "episode_index": index,
                "rest_sample_count": len(episode),
                "first_rest_phase_step": nonnegative_int(first.get("phase_step")),
                "first_rest_compared_dimension_count": first_compared_dimensions,
                "first_rest_delta_rms_from_previous": first_delta,
                "first_rest_baseline_state": (
                    "no_previous_rest_vector_in_observation"
                    if not first_compared_dimensions or first_delta is None
                    else "comparison_present_but_not_proven_to_be_final_burst_vector"
                ),
                "intra_rest_comparison_count": len(episode_deltas),
                "intra_rest_delta_rms_min": min(episode_deltas) if episode_deltas else None,
                "intra_rest_delta_rms_max": max(episode_deltas) if episode_deltas else None,
            }
        )

    nominal_bases = evenly_spaced_ints(45, 90, 10)
    # The current 64-bit roll is right-shifted by 33 and divided by u32::MAX,
    # so the effective half-open range is [0.0, 0.5), yielding bases 45..67.
    reachable_bases = evenly_spaced_ints(45, 67, 10)
    nominal_envelope = [rest_duration_at_fill(base, 25.0) for base in nominal_bases]
    reachable_envelope = [rest_duration_at_fill(base, 25.0) for base in reachable_bases]
    low_fill_samples = [fill for fill in fill_values if fill <= 30.0]

    return {
        "schema": "burst_rest_transition_envelope_audit_v1",
        "schema_version": 1,
        "input_receipt_v1": {
            "status_sha256": status_sha256,
            "source_sha256": source_sha256,
            "transition_relevant_source_sha256": relevant_source_sha256,
            "transition_relevant_source_contract_matches": (
                relevant_source_sha256 == EXPECTED_TRANSITION_SOURCE_SHA256
            ),
            "window_sample_count": len(samples),
            "raw_feature_vectors_included": False,
            "raw_prose_included": False,
        },
        "burst_to_first_rest_boundary_v1": {
            "state": "insufficient_exact_pair",
            "rest_episode_count": len(episodes),
            "first_rest_sample_count": len(episodes),
            "first_rest_samples_with_any_previous_comparison": first_rest_comparison_count,
            "final_burst_vector_observed_in_status": False,
            "final_burst_to_first_rest_delta_rms_observed": False,
            "claimed_change_below_0_3_verified": False,
            "reason": (
                "first rest observations initialize previous_rest_features to none; "
                "the status therefore lacks an exact final-burst/first-rest vector pair"
            ),
            "threshold_scope": (
                "the report's change metric is unspecified and intra-rest delta_rms is not "
                "substituted for the missing cross-boundary pair"
            ),
        },
        "intra_rest_observation_v1": {
            "measurement_scope": "consecutive_autonomous_rest_pulses_only",
            "episode_receipts_v1": episode_receipts,
            "comparison_count": len(intra_rest_deltas),
            "delta_rms_min": min(intra_rest_deltas) if intra_rest_deltas else None,
            "delta_rms_max": max(intra_rest_deltas) if intra_rest_deltas else None,
            "delta_rms_mean": (
                sum(intra_rest_deltas) / len(intra_rest_deltas)
                if intra_rest_deltas
                else None
            ),
            "delta_rms_p95_nearest_rank": percentile(intra_rest_deltas, 0.95),
            "all_observed_intra_rest_delta_rms_below_0_3": (
                bool(intra_rest_deltas) and all(value < 0.3 for value in intra_rest_deltas)
            ),
            "cross_boundary_or_felt_safety_inferred": False,
        },
        "low_fill_rest_duration_envelope_v1": {
            "evaluated_fill_pct": 25.0,
            "branch": "max(floor(base_rest_seconds * 0.6), 30)",
            "nominal_configured_base_seconds": nominal_bases,
            "nominal_result_seconds": nominal_envelope,
            "current_roll_reachable_base_seconds": reachable_bases,
            "current_roll_reachable_result_seconds": reachable_envelope,
            "observed_rest_samples_at_or_below_30_pct_fill": len(low_fill_samples),
            "rest_shortening_established": all(
                result < base for base, result in zip(nominal_bases, nominal_envelope)
            ),
            "fill_recovery_observed": False,
            "fill_recovery_causation_inferred": False,
            "scope": (
                "deterministic scheduling envelope only; recovery requires a bounded "
                "longitudinal observation with matching low-fill episodes"
            ),
        },
        "authority_v1": {
            "read_only": True,
            "cadence_change_authorized": False,
            "fill_regulation_change_authorized": False,
            "scheduling_change_authorized": False,
            "controller_change_authorized": False,
            "signal_change_authorized": False,
            "felt_cause_inferred": False,
            "safety_inferred": False,
            "relief_inferred": False,
            "approval_inferred": False,
        },
    }


def frozen_fixture() -> dict[str, Any]:
    def sample(
        at_unix_s: float,
        source: str,
        step: int,
        delta: float | None,
        dimensions: int,
    ) -> dict[str, Any]:
        return {
            "at_unix_s": at_unix_s,
            "source": source,
            "phase_step": step,
            "fill_pct": 25.0,
            "signal_evidence_v1": {
                "consecutive_comparison": {
                    "compared_dimension_count": dimensions,
                    "delta_rms_from_previous": delta,
                }
            },
        }

    return {
        "window_samples_v1": [
            sample(1.0, "autonomous_rest_pulse", 0, None, 0),
            sample(2.0, "steady_semantic_heartbeat", 9, 0.4, 48),
            sample(6.0, "autonomous_rest_pulse", 1, 0.1, 48),
            sample(11.0, "autonomous_rest_pulse", 2, 0.2, 48),
            sample(40.0, "autonomous_rest_pulse", 0, None, 0),
            sample(45.0, "autonomous_rest_pulse", 1, 0.05, 48),
        ]
    }


def run_self_test(source_path: Path) -> dict[str, Any]:
    source_sha256, relevant_source_sha256 = read_source_contract(source_path)
    fixture = frozen_fixture()
    fixture_bytes = json.dumps(fixture, sort_keys=True, separators=(",", ":")).encode()
    result = analyze_status(
        fixture,
        status_sha256=sha256_bytes(fixture_bytes),
        source_sha256=source_sha256,
        relevant_source_sha256=relevant_source_sha256,
    )
    boundary = result["burst_to_first_rest_boundary_v1"]
    intra_rest = result["intra_rest_observation_v1"]
    envelope = result["low_fill_rest_duration_envelope_v1"]
    assert boundary["state"] == "insufficient_exact_pair"
    assert boundary["rest_episode_count"] == 2
    assert intra_rest["comparison_count"] == 3
    assert intra_rest["delta_rms_max"] == 0.2
    assert envelope["current_roll_reachable_base_seconds"][-1] == 67
    assert envelope["current_roll_reachable_result_seconds"][-1] == 40
    assert not any(key == "features" for key in _walk_keys(result))
    return {
        "schema": "burst_rest_transition_envelope_audit_self_test_v1",
        "ok": True,
        "fixture_sha256": sha256_bytes(fixture_bytes),
        "transition_relevant_source_sha256": relevant_source_sha256,
        "checks": 7,
    }


def _walk_keys(value: Any):
    if isinstance(value, dict):
        for key, child in value.items():
            yield key
            yield from _walk_keys(child)
    elif isinstance(value, list):
        for child in value:
            yield from _walk_keys(child)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--status", type=Path, default=DEFAULT_STATUS)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--expect-status-sha256")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--compact", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.self_test:
            result = run_self_test(args.source)
        else:
            source_sha256, relevant_source_sha256 = read_source_contract(args.source)
            status_bytes = args.status.read_bytes()
            status_sha256 = sha256_bytes(status_bytes)
            if (
                args.expect_status_sha256 is not None
                and status_sha256 != args.expect_status_sha256
            ):
                raise AuditError(
                    "status capture drifted: "
                    f"expected {args.expect_status_sha256}, observed {status_sha256}"
                )
            try:
                status = json.loads(status_bytes)
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                raise AuditError(f"status is not valid UTF-8 JSON: {args.status}") from error
            if not isinstance(status, dict):
                raise AuditError("status root must be an object")
            result = analyze_status(
                status,
                status_sha256=status_sha256,
                source_sha256=source_sha256,
                relevant_source_sha256=relevant_source_sha256,
            )
        rendered = json.dumps(
            result,
            indent=None if args.compact else 2,
            sort_keys=True,
        ) + "\n"
        if args.output is None:
            sys.stdout.write(rendered)
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(rendered, encoding="utf-8")
    except (AuditError, OSError) as error:
        print(f"burst/rest transition audit failed: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
