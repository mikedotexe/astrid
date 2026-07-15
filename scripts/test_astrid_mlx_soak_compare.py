"""Unit tests for scripts/astrid_mlx_soak_compare.py."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

SPEC = importlib.util.spec_from_file_location(
    "astrid_mlx_soak_compare",
    SCRIPT_DIR / "astrid_mlx_soak_compare.py",
)
assert SPEC and SPEC.loader
compare = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(compare)


def soak_record(
    *,
    run_id: str,
    automated_ok: bool,
    failure_reasons: list[str],
    fallback_count: int = 0,
    generated_malformed_next_count: int = 0,
    generated_deprecated_language_count: int = 0,
    max_total_turn_s: float | None = 80.0,
) -> dict:
    return {
        "run_id": run_id,
        "duration_s": 7200.0,
        "summary": {
            "automated_ok": automated_ok,
            "failure_reasons": failure_reasons,
        },
        "monitor": {
            "automated_ok": automated_ok,
            "failure_reasons": failure_reasons,
            "events": {
                "fallback_count": fallback_count,
                "fallback_line_count": fallback_count * 2,
                "bridge_artifact_strip_count": 0,
                "artifact_count": 0,
                "malformed_next_count": 0,
            },
            "generated_output_scan": {
                "counts": {
                    "artifact_count": 0,
                    "deprecated_language_count": generated_deprecated_language_count,
                    "explore_action_count": 0,
                    "malformed_next_count": generated_malformed_next_count,
                }
            },
            "bridge_bad_sample_count": 0,
            "candidate_missing_sample_count": 0,
            "request_metrics": {
                "count": 40,
                "max_total_turn_s": max_total_turn_s,
            },
        },
    }


def test_shared_fallback_failure_can_be_baseline_relative_ok():
    baseline = soak_record(
        run_id="baseline",
        automated_ok=False,
        failure_reasons=["fallback_incidents_exceeded"],
        fallback_count=5,
    )
    candidate = soak_record(
        run_id="candidate",
        automated_ok=False,
        failure_reasons=["fallback_incidents_exceeded"],
        fallback_count=2,
    )

    report = compare.compare_soaks(baseline, candidate)

    assert report["strict_promotion_ok"] is False
    assert report["baseline_relative_ok"] is True
    assert report["recommendation"] == "hold_shared_failures"
    assert report["shared_failure_reasons"] == ["fallback_incidents_exceeded"]
    assert report["regressions"] == []


def test_candidate_output_policy_failure_is_regression_even_when_baseline_has_fallbacks():
    baseline = soak_record(
        run_id="baseline",
        automated_ok=False,
        failure_reasons=["fallback_incidents_exceeded"],
        fallback_count=5,
    )
    candidate = soak_record(
        run_id="candidate",
        automated_ok=False,
        failure_reasons=[
            "fallback_incidents_exceeded",
            "generated_malformed_next_detected",
        ],
        fallback_count=2,
        generated_malformed_next_count=1,
    )

    report = compare.compare_soaks(baseline, candidate)

    assert report["baseline_relative_ok"] is False
    assert report["recommendation"] == "hold_candidate_regression"
    assert "generated_malformed_next_detected" in report["new_failure_reasons"]
    assert any(
        item.get("counter") == "generated.malformed_next_count"
        for item in report["regressions"]
    )


def test_candidate_fallback_count_above_baseline_is_regression_by_default():
    baseline = soak_record(
        run_id="baseline",
        automated_ok=False,
        failure_reasons=["fallback_incidents_exceeded"],
        fallback_count=2,
    )
    candidate = soak_record(
        run_id="candidate",
        automated_ok=False,
        failure_reasons=["fallback_incidents_exceeded"],
        fallback_count=3,
    )

    report = compare.compare_soaks(baseline, candidate)

    assert report["baseline_relative_ok"] is False
    assert any(item["kind"] == "fallback_count_regression" for item in report["regressions"])


def test_clean_candidate_is_strict_promotion_ready():
    baseline = soak_record(
        run_id="baseline",
        automated_ok=False,
        failure_reasons=["fallback_incidents_exceeded"],
        fallback_count=2,
    )
    candidate = soak_record(
        run_id="candidate",
        automated_ok=True,
        failure_reasons=[],
        fallback_count=0,
    )

    report = compare.compare_soaks(baseline, candidate)

    assert report["strict_promotion_ok"] is True
    assert report["baseline_relative_ok"] is True
    assert report["recommendation"] == "strict_promotion_ready"
    assert report["resolved_failure_reasons"] == ["fallback_incidents_exceeded"]
