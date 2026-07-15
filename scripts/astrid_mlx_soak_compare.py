#!/usr/bin/env python3
"""Compare Astrid MLX soak results against a live-version baseline.

The live soak is intentionally strict: any fallback incident blocks a direct
promotion. This companion report answers a different question for future MLX
intake work: did a candidate introduce a new regression relative to the current
baseline, or are the observed failures shared bridge/load debt?
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


EVENT_COUNTERS = (
    "fallback_count",
    "fallback_line_count",
    "bridge_artifact_strip_count",
    "artifact_count",
    "malformed_next_count",
)
GENERATED_COUNTERS = (
    "artifact_count",
    "deprecated_language_count",
    "explore_action_count",
    "malformed_next_count",
)
SAMPLE_COUNTERS = (
    "bridge_bad_sample_count",
    "candidate_missing_sample_count",
)
REQUEST_METRICS = (
    "count",
    "p95_total_turn_s",
    "max_total_turn_s",
    "p95_first_token_s",
    "max_first_token_s",
    "max_prompt_chars",
    "max_generated_tokens",
)
POLICY_REGRESSION_COUNTERS = (
    "events.bridge_artifact_strip_count",
    "events.artifact_count",
    "events.malformed_next_count",
    "generated.artifact_count",
    "generated.deprecated_language_count",
    "generated.explore_action_count",
    "generated.malformed_next_count",
    "samples.bridge_bad_sample_count",
    "samples.candidate_missing_sample_count",
)


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"{path} did not contain a JSON object")
    return value


def nested_int(data: dict[str, Any], *keys: str) -> int:
    value: Any = data
    for key in keys:
        if not isinstance(value, dict):
            return 0
        value = value.get(key)
    return value if isinstance(value, int) else 0


def nested_float(data: dict[str, Any], *keys: str) -> float | None:
    value: Any = data
    for key in keys:
        if not isinstance(value, dict):
            return None
        value = value.get(key)
    return float(value) if isinstance(value, (int, float)) else None


def failure_reasons(record: dict[str, Any]) -> list[str]:
    monitor = record.get("monitor") if isinstance(record.get("monitor"), dict) else {}
    summary = record.get("summary") if isinstance(record.get("summary"), dict) else {}
    reasons = monitor.get("failure_reasons") or summary.get("failure_reasons") or []
    return sorted(reason for reason in reasons if isinstance(reason, str))


def automated_ok(record: dict[str, Any]) -> bool:
    monitor = record.get("monitor") if isinstance(record.get("monitor"), dict) else {}
    summary = record.get("summary") if isinstance(record.get("summary"), dict) else {}
    if isinstance(monitor.get("automated_ok"), bool):
        return bool(monitor["automated_ok"])
    return bool(summary.get("automated_ok"))


def counter_map(record: dict[str, Any]) -> dict[str, int]:
    monitor = record.get("monitor") if isinstance(record.get("monitor"), dict) else {}
    events = monitor.get("events") if isinstance(monitor.get("events"), dict) else {}
    generated_scan = (
        monitor.get("generated_output_scan")
        if isinstance(monitor.get("generated_output_scan"), dict)
        else {}
    )
    generated = (
        generated_scan.get("counts")
        if isinstance(generated_scan.get("counts"), dict)
        else {}
    )
    counters: dict[str, int] = {}
    for key in EVENT_COUNTERS:
        counters[f"events.{key}"] = events.get(key) if isinstance(events.get(key), int) else 0
    for key in GENERATED_COUNTERS:
        counters[f"generated.{key}"] = (
            generated.get(key) if isinstance(generated.get(key), int) else 0
        )
    for key in SAMPLE_COUNTERS:
        counters[f"samples.{key}"] = monitor.get(key) if isinstance(monitor.get(key), int) else 0
    return counters


def request_metric_map(record: dict[str, Any]) -> dict[str, float | int | None]:
    monitor = record.get("monitor") if isinstance(record.get("monitor"), dict) else {}
    metrics = (
        monitor.get("request_metrics")
        if isinstance(monitor.get("request_metrics"), dict)
        else {}
    )
    return {
        key: metrics.get(key) if isinstance(metrics.get(key), (int, float)) else None
        for key in REQUEST_METRICS
    }


def run_summary(record: dict[str, Any]) -> dict[str, Any]:
    return {
        "run_id": record.get("run_id"),
        "started_at": record.get("started_at"),
        "candidate_model": record.get("candidate_model"),
        "candidate_url": record.get("candidate_url"),
        "candidate_port": record.get("candidate_port"),
        "duration_s": record.get("duration_s"),
        "automated_ok": automated_ok(record),
        "failure_reasons": failure_reasons(record),
        "counters": counter_map(record),
        "request_metrics": request_metric_map(record),
    }


def per_hour(count: int, duration_s: Any) -> float | None:
    if not isinstance(duration_s, (int, float)) or duration_s <= 0:
        return None
    return round((float(count) / float(duration_s)) * 3600.0, 3)


def compare_counter_deltas(
    baseline: dict[str, int],
    candidate: dict[str, int],
) -> dict[str, dict[str, int]]:
    keys = sorted(set(baseline) | set(candidate))
    return {
        key: {
            "baseline": baseline.get(key, 0),
            "candidate": candidate.get(key, 0),
            "delta": candidate.get(key, 0) - baseline.get(key, 0),
        }
        for key in keys
    }


def compare_metric_deltas(
    baseline: dict[str, float | int | None],
    candidate: dict[str, float | int | None],
) -> dict[str, dict[str, float | int | None]]:
    keys = sorted(set(baseline) | set(candidate))
    deltas: dict[str, dict[str, float | int | None]] = {}
    for key in keys:
        left = baseline.get(key)
        right = candidate.get(key)
        delta: float | int | None = None
        if isinstance(left, (int, float)) and isinstance(right, (int, float)):
            delta = round(float(right) - float(left), 3)
        deltas[key] = {"baseline": left, "candidate": right, "delta": delta}
    return deltas


def compare_soaks(
    baseline_record: dict[str, Any],
    candidate_record: dict[str, Any],
    *,
    max_fallback_delta: int = 0,
    max_fallback_rate_delta: float | None = None,
) -> dict[str, Any]:
    baseline = run_summary(baseline_record)
    candidate = run_summary(candidate_record)
    baseline_failures = set(baseline["failure_reasons"])
    candidate_failures = set(candidate["failure_reasons"])
    counter_deltas = compare_counter_deltas(baseline["counters"], candidate["counters"])
    metric_deltas = compare_metric_deltas(
        baseline["request_metrics"],
        candidate["request_metrics"],
    )

    regressions: list[dict[str, Any]] = []
    new_failure_reasons = sorted(candidate_failures - baseline_failures)
    if new_failure_reasons:
        regressions.append(
            {
                "kind": "new_failure_reasons",
                "items": new_failure_reasons,
            }
        )

    for key in POLICY_REGRESSION_COUNTERS:
        delta = counter_deltas.get(key, {})
        baseline_count = int(delta.get("baseline") or 0)
        candidate_count = int(delta.get("candidate") or 0)
        if candidate_count > baseline_count:
            regressions.append(
                {
                    "kind": "policy_counter_increased",
                    "counter": key,
                    "baseline": baseline_count,
                    "candidate": candidate_count,
                    "delta": candidate_count - baseline_count,
                }
            )

    fallback_delta = counter_deltas.get("events.fallback_count", {})
    baseline_fallbacks = int(fallback_delta.get("baseline") or 0)
    candidate_fallbacks = int(fallback_delta.get("candidate") or 0)
    if candidate_fallbacks > baseline_fallbacks + max_fallback_delta:
        regressions.append(
            {
                "kind": "fallback_count_regression",
                "baseline": baseline_fallbacks,
                "candidate": candidate_fallbacks,
                "delta": candidate_fallbacks - baseline_fallbacks,
                "max_allowed_delta": max_fallback_delta,
            }
        )

    baseline_fallback_rate = per_hour(
        baseline_fallbacks,
        baseline.get("duration_s"),
    )
    candidate_fallback_rate = per_hour(
        candidate_fallbacks,
        candidate.get("duration_s"),
    )
    if (
        max_fallback_rate_delta is not None
        and baseline_fallback_rate is not None
        and candidate_fallback_rate is not None
        and candidate_fallback_rate > baseline_fallback_rate + max_fallback_rate_delta
    ):
        regressions.append(
            {
                "kind": "fallback_rate_regression",
                "baseline_per_hour": baseline_fallback_rate,
                "candidate_per_hour": candidate_fallback_rate,
                "delta_per_hour": round(candidate_fallback_rate - baseline_fallback_rate, 3),
                "max_allowed_delta_per_hour": max_fallback_rate_delta,
            }
        )

    strict_promotion_ok = bool(candidate["automated_ok"])
    baseline_relative_ok = not regressions
    if strict_promotion_ok:
        recommendation = "strict_promotion_ready"
    elif not baseline_relative_ok:
        recommendation = "hold_candidate_regression"
    elif candidate_failures & baseline_failures:
        recommendation = "hold_shared_failures"
    else:
        recommendation = "hold_operator_review"

    return {
        "baseline": baseline,
        "candidate": candidate,
        "strict_promotion_ok": strict_promotion_ok,
        "baseline_relative_ok": baseline_relative_ok,
        "recommendation": recommendation,
        "shared_failure_reasons": sorted(baseline_failures & candidate_failures),
        "new_failure_reasons": new_failure_reasons,
        "resolved_failure_reasons": sorted(baseline_failures - candidate_failures),
        "regressions": regressions,
        "counter_deltas": counter_deltas,
        "request_metric_deltas": metric_deltas,
        "fallback_rates_per_hour": {
            "baseline": baseline_fallback_rate,
            "candidate": candidate_fallback_rate,
            "delta": (
                round(candidate_fallback_rate - baseline_fallback_rate, 3)
                if baseline_fallback_rate is not None and candidate_fallback_rate is not None
                else None
            ),
        },
    }


def render_markdown(report: dict[str, Any]) -> str:
    baseline = report["baseline"]
    candidate = report["candidate"]
    lines = [
        "# Astrid MLX Soak Comparison",
        "",
        f"- Baseline run: `{baseline.get('run_id')}`",
        f"- Candidate run: `{candidate.get('run_id')}`",
        f"- Strict promotion: `{'PASS' if report['strict_promotion_ok'] else 'HOLD'}`",
        f"- Baseline-relative result: `{'OK' if report['baseline_relative_ok'] else 'REGRESSION'}`",
        f"- Recommendation: `{report['recommendation']}`",
        "",
        "## Failure Reasons",
        "",
        f"- Shared: `{', '.join(report['shared_failure_reasons']) or 'none'}`",
        f"- New in candidate: `{', '.join(report['new_failure_reasons']) or 'none'}`",
        f"- Resolved by candidate: `{', '.join(report['resolved_failure_reasons']) or 'none'}`",
        "",
        "## Key Counters",
        "",
        "| Counter | Baseline | Candidate | Delta |",
        "| --- | ---: | ---: | ---: |",
    ]
    for key in (
        "events.fallback_count",
        "events.fallback_line_count",
        "events.bridge_artifact_strip_count",
        "events.artifact_count",
        "events.malformed_next_count",
        "generated.artifact_count",
        "generated.deprecated_language_count",
        "generated.explore_action_count",
        "generated.malformed_next_count",
        "samples.bridge_bad_sample_count",
        "samples.candidate_missing_sample_count",
    ):
        delta = report["counter_deltas"].get(key, {})
        lines.append(
            f"| `{key}` | {delta.get('baseline', 0)} | "
            f"{delta.get('candidate', 0)} | {delta.get('delta', 0)} |"
        )
    lines.extend(
        [
            "",
            "## Regressions",
            "",
        ]
    )
    if report["regressions"]:
        lines.extend(f"- `{item['kind']}`: `{item}`" for item in report["regressions"])
    else:
        lines.append("- none")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--max-fallback-delta", type=int, default=0)
    parser.add_argument("--max-fallback-rate-delta", type=float)
    parser.add_argument("--json-out", type=Path)
    parser.add_argument("--markdown-out", type=Path)
    parser.add_argument("--fail-on-regression", action="store_true")
    parser.add_argument("--fail-unless-strict-promotion", action="store_true")
    args = parser.parse_args()

    report = compare_soaks(
        load_json(args.baseline),
        load_json(args.candidate),
        max_fallback_delta=args.max_fallback_delta,
        max_fallback_rate_delta=args.max_fallback_rate_delta,
    )

    text = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.json_out:
        args.json_out.write_text(text, encoding="utf-8")
    else:
        sys.stdout.write(text)
    if args.markdown_out:
        args.markdown_out.write_text(render_markdown(report), encoding="utf-8")

    if args.fail_unless_strict_promotion and not report["strict_promotion_ok"]:
        return 1
    if args.fail_on_regression and not report["baseline_relative_ok"]:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
