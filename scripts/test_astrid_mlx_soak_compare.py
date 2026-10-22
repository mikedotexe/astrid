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
        "matched_workloads": {"dialogue": {
            "messages_sha256": "same-prompt", "count": 3,
            "p95_total_turn_s": 80.0, "p95_first_token_s": 2.0,
            "steady_tok_s": 10.0, "peak_memory_bytes": 1000000,
            "completion_tokens": [120, 120, 120],
        }},
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


def clean_record():
    return soak_record(run_id="clean", automated_ok=True, failure_reasons=[])


def test_missing_performance_never_passes_even_when_candidate_says_ok():
    baseline, candidate = clean_record(), clean_record()
    del candidate["matched_workloads"]
    report = compare.compare_soaks(baseline, candidate)
    assert not report["strict_promotion_ok"]
    assert not report["baseline_relative_ok"]
    assert report["recommendation"] == "hold_incomplete_performance_evidence"


def test_performance_boundaries_are_inclusive():
    baseline, candidate = clean_record(), clean_record()
    candidate["matched_workloads"]["dialogue"].update({
        "p95_total_turn_s": 96, "p95_first_token_s": 2.4,
        "steady_tok_s": 9, "peak_memory_bytes": 1100000})
    assert compare.compare_soaks(baseline, candidate)["strict_promotion_ok"]


def test_each_performance_regression_blocks_strict_promotion():
    for key, value in (("p95_total_turn_s", 96.1), ("p95_first_token_s", 2.41),
                       ("steady_tok_s", 8.99), ("peak_memory_bytes", 1100001)):
        baseline, candidate = clean_record(), clean_record()
        candidate["matched_workloads"]["dialogue"][key] = value
        report = compare.compare_soaks(baseline, candidate)
        assert not report["strict_promotion_ok"], key
        assert report["recommendation"] == "hold_candidate_regression"


def test_missing_metrics_nonfinite_values_and_mismatched_prompts_block():
    for key, value in (("p95_total_turn_s", None), ("steady_tok_s", float("nan")),
                       ("peak_memory_bytes", 0), ("count", 2),
                       ("messages_sha256", "different-prompt")):
        baseline, candidate = clean_record(), clean_record()
        candidate["matched_workloads"]["dialogue"][key] = value
        report = compare.compare_soaks(baseline, candidate)
        assert not report["strict_promotion_ok"], key
        assert report["performance"]["incomplete_evidence"], key


def test_rendered_prompt_mismatch_and_short_completions_are_incomplete():
    baseline, candidate = clean_record(), clean_record()
    baseline["matched_workloads"]["dialogue"]["rendered_prompt_sha256_12"] = "a" * 12
    candidate["matched_workloads"]["dialogue"]["rendered_prompt_sha256_12"] = "b" * 12
    candidate["matched_workloads"]["dialogue"]["completion_tokens"] = [12, 12, 12]
    report = compare.compare_soaks(baseline, candidate)
    assert not report["strict_promotion_ok"]
    kinds = {item["kind"] for item in report["performance"]["incomplete_evidence"]}
    assert {"workload_setting_mismatch", "completion_length_mismatch"} <= kinds


def test_missing_completion_counts_do_not_pass():
    baseline, candidate = clean_record(), clean_record()
    del candidate["matched_workloads"]["dialogue"]["completion_tokens"]
    assert not compare.compare_soaks(baseline, candidate)["strict_promotion_ok"]
