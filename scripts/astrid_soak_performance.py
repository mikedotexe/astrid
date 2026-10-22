"""Strict, matched workload gates for MLX qualification evidence."""

from __future__ import annotations

import math
import statistics
from typing import Any


def positive(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value) and value > 0


def percentile(values: list[float], fraction: float = .95) -> float:
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * fraction) - 1)]


def workloads(record: dict[str, Any]) -> dict[str, Any]:
    if isinstance(record.get("matched_workloads"), dict):
        return record["matched_workloads"]
    groups: dict[str, list[dict[str, Any]]] = {}
    for sample in record.get("performance_samples", []):
        if not isinstance(sample, dict):
            continue
        key = sample.get("scenario")
        if isinstance(key, str) and key:
            groups.setdefault(key, []).append(sample)
    result = {}
    for key, samples in groups.items():
        first = samples[0]
        group = {"count": len(samples), "messages_sha256": first.get("prompt_sha256"),
                 "lane": first.get("lane"), "max_tokens": first.get("max_tokens"),
                 "rendered_prompt_sha256_12": first.get("rendered_prompt_sha256_12"),
                 "completion_tokens": [sample.get("completion_tokens") for sample in samples]}
        identities = {(s.get("prompt_sha256"), s.get("lane"), s.get("max_tokens"),
                       s.get("rendered_prompt_sha256_12")) for s in samples}
        group["consistent_workload"] = len(identities) == 1
        for source, target, aggregate in (
            ("total_turn_s", "p95_total_turn_s", percentile),
            ("first_token_s", "p95_first_token_s", percentile),
            ("decode_tok_s", "steady_tok_s", statistics.median),
            ("peak_memory_bytes", "peak_memory_bytes", max),
        ):
            values = [s.get(source) for s in samples]
            group[target] = aggregate(values) if all(positive(v) for v in values) else None
        result[key] = group
    return result


def compare_performance(
    baseline_record: dict[str, Any], candidate_record: dict[str, Any], *,
    max_p95_increase: float = .20, max_decode_decline: float = .10,
    max_peak_memory_increase: float = .10, min_trials: int = 3,
) -> dict[str, Any]:
    """Absence or unmatched data is incomplete evidence, never a passing gate."""
    for name, value in (("max_p95_increase", max_p95_increase),
                        ("max_decode_decline", max_decode_decline),
                        ("max_peak_memory_increase", max_peak_memory_increase)):
        if not isinstance(value, (int, float)) or not math.isfinite(value) or value < 0:
            raise ValueError(f"{name} must be finite and nonnegative")
    if min_trials < 3:
        raise ValueError("matched performance requires at least three trials")
    baseline, candidate = workloads(baseline_record), workloads(candidate_record)
    incomplete, regressions, comparisons = [], [], {}
    if not baseline or not candidate:
        incomplete.append({"kind": "matched_performance_missing"})
    for key in sorted(set(baseline) | set(candidate)):
        left, right = baseline.get(key), candidate.get(key)
        if not isinstance(left, dict) or not isinstance(right, dict):
            incomplete.append({"kind": "unmatched_workload", "workload": key})
            continue
        if (not left.get("messages_sha256") or left.get("messages_sha256") != right.get("messages_sha256")
                or left.get("consistent_workload") is False or right.get("consistent_workload") is False):
            incomplete.append({"kind": "workload_identity_mismatch", "workload": key})
        for field in ("lane", "max_tokens", "temperature", "prompt_tokens", "rendered_prompt_sha256_12"):
            if left.get(field) != right.get(field):
                incomplete.append({"kind": "workload_setting_mismatch", "workload": key, "field": field})
        counts = [left.get("count"), right.get("count")]
        if any(not isinstance(n, int) or isinstance(n, bool) or n < min_trials for n in counts) or counts[0] != counts[1]:
            incomplete.append({"kind": "insufficient_matched_trials", "workload": key,
                               "baseline": counts[0], "candidate": counts[1], "minimum": min_trials})
        metrics = {}
        completions = [left.get("completion_tokens"), right.get("completion_tokens")]
        if any(not isinstance(values, list) or not values or len(values) != count
               or not all(isinstance(v, int) and not isinstance(v, bool) and v > 0 for v in values)
               for values, count in zip(completions, counts)):
            incomplete.append({"kind": "completion_counts_missing", "workload": key})
        else:
            total_left, total_right = map(sum, completions)
            ratio = total_right / total_left
            metrics["total_completion_tokens"] = {"baseline": total_left, "candidate": total_right,
                                                   "ratio": ratio, "min_ratio": .8, "max_ratio": 1.2}
            if ratio < .8 - 1e-12 or ratio > 1.2 + 1e-12:
                incomplete.append({"kind": "completion_length_mismatch", "workload": key,
                                   "baseline": total_left, "candidate": total_right, "ratio": ratio})
        for field, allowance, lower_is_better in (
            ("p95_total_turn_s", max_p95_increase, True),
            ("p95_first_token_s", max_p95_increase, True),
            ("steady_tok_s", max_decode_decline, False),
            ("peak_memory_bytes", max_peak_memory_increase, True),
        ):
            a, b = left.get(field), right.get(field)
            if not positive(a) or not positive(b):
                incomplete.append({"kind": "performance_metric_missing", "workload": key, "metric": field})
                continue
            ratio = b / a
            boundary = 1 + allowance if lower_is_better else 1 - allowance
            failed = ratio > boundary + 1e-12 if lower_is_better else ratio < boundary - 1e-12
            metrics[field] = {"baseline": a, "candidate": b, "ratio": ratio,
                              "threshold_ratio": boundary, "ok": not failed}
            if failed:
                regressions.append({"kind": "performance_regression", "workload": key,
                                    "metric": field, **metrics[field]})
        comparisons[key] = metrics
    return {"ok": not incomplete and not regressions, "complete": not incomplete,
            "incomplete_evidence": incomplete, "regressions": regressions,
            "workloads": comparisons, "thresholds": {
                "max_p95_increase": max_p95_increase, "max_decode_decline": max_decode_decline,
                "max_peak_memory_increase": max_peak_memory_increase, "min_trials": min_trials,
                "max_completion_variation": .20}}
