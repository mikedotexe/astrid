#!/usr/bin/env python3
"""Bounded read-only audit of dialogue budgets and spectral pressure context.

The audit uses fields already co-located in ``dialogue_prompt_budget_v3``.
It does not induce pressure, generate dialogue, score private prose, change a
token budget, or turn an observational association into a felt or causal claim.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import tempfile
import unittest
from collections import Counter, defaultdict, deque
from pathlib import Path
from typing import Any, Callable

DEFAULT_DIAGNOSTIC = Path(
    "/Users/v/other/astrid/capsules/spectral-bridge/workspace/diagnostics/"
    "dialogue_prompt_budget.jsonl"
)
DEFAULT_ATTEMPTS = Path(
    "/Users/v/other/astrid/capsules/spectral-bridge/workspace/diagnostics/"
    "dialogue_live_attempts.jsonl"
)
POLICY = "dialogue_token_pressure_audit_v1"
LATENCY_FIELDS = (
    ("elapsed_ms", 0.001),
    ("latency_ms", 0.001),
    ("duration_ms", 0.001),
    ("elapsed_seconds", 1.0),
    ("latency_seconds", 1.0),
    ("duration_seconds", 1.0),
    ("first_token_seconds", 1.0),
)
AUTHORITY = {
    "scope": "bounded_existing_log_observation",
    "pressure_induced": False,
    "dialogue_generated": False,
    "runtime_mutated": False,
    "token_budget_changed": False,
    "model_or_sampler_changed": False,
    "felt_effect_established": False,
    "causal_effect_established": False,
    "silence_infers_uptake_or_resolution": False,
    "live_behavior_change_requires_separate_authority": True,
}


def _finite_float(value: object) -> float | None:
    if isinstance(value, bool):
        return None
    try:
        parsed = float(value)
    except (TypeError, ValueError):
        return None
    return parsed if math.isfinite(parsed) else None


def _integer(value: object) -> int | None:
    parsed = _finite_float(value)
    if parsed is None or not parsed.is_integer():
        return None
    return int(parsed)


def _sha256(path: Path) -> str | None:
    if not path.is_file():
        return None
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _read_window(
    path: Path,
    *,
    timestamp: Callable[[dict[str, Any]], int | None],
    from_ts: int,
    to_ts: int | None,
    limit: int,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    retained: deque[dict[str, Any]] = deque(maxlen=limit)
    source_lines = 0
    valid_records = 0
    matching_records = 0
    source_timestamps: list[int] = []
    matching_timestamps: list[int] = []
    if path.is_file():
        with path.open("r", encoding="utf-8", errors="ignore") as handle:
            for line in handle:
                source_lines += 1
                if not line.strip():
                    continue
                try:
                    payload = json.loads(line)
                except (TypeError, ValueError):
                    continue
                if not isinstance(payload, dict):
                    continue
                valid_records += 1
                ts = timestamp(payload)
                if ts is None:
                    continue
                source_timestamps.append(ts)
                if ts < from_ts or (to_ts is not None and ts > to_ts):
                    continue
                matching_records += 1
                matching_timestamps.append(ts)
                retained.append(payload)
    retained_records = list(retained)
    retained_timestamps = [
        ts
        for record in retained_records
        if (ts := timestamp(record)) is not None
    ]
    return retained_records, {
        "source_lines": source_lines,
        "valid_records": valid_records,
        "source_timestamp_min": min(source_timestamps, default=None),
        "source_timestamp_max": max(source_timestamps, default=None),
        "matching_records": matching_records,
        "matching_timestamp_range": _timestamp_range(matching_timestamps),
        "retained_records": len(retained_records),
        "retained_timestamp_range": _timestamp_range(retained_timestamps),
        "earlier_matching_records_omitted": max(
            0, matching_records - len(retained_records)
        ),
    }


def _prompt_timestamp(record: dict[str, Any]) -> int | None:
    return _integer(record.get("timestamp") or record.get("ts"))


def _attempt_timestamp(record: dict[str, Any]) -> int | None:
    return _integer(record.get("timestamp_unix_s") or record.get("timestamp"))


def _pressure_observation(record: dict[str, Any]) -> dict[str, Any]:
    context = record.get("prompt_context_observation_v3")
    if not isinstance(context, dict):
        return {}
    pressure = context.get("felt_pressure_observation_v3")
    return pressure if isinstance(pressure, dict) else {}


def _removed_fraction(record: dict[str, Any]) -> float | None:
    context = record.get("prompt_context_observation_v3")
    if isinstance(context, dict):
        value = _finite_float(context.get("removed_fraction"))
        if value is not None:
            return min(max(value, 0.0), 1.0)
    report = record.get("budget_report")
    if not isinstance(report, dict):
        return None
    before = _finite_float(report.get("total_before"))
    after = _finite_float(report.get("total_after"))
    if before is None or before <= 0.0 or after is None:
        return None
    return min(max((before - after) / before, 0.0), 1.0)


def _latency_seconds(record: dict[str, Any]) -> tuple[str, float] | None:
    for name, scale in LATENCY_FIELDS:
        value = _finite_float(record.get(name))
        if value is not None and value >= 0.0:
            return name, value * scale
    return None


def _mean(values: list[float]) -> float | None:
    return sum(values) / len(values) if values else None


def _rounded(value: float | None) -> float | None:
    return round(value, 6) if value is not None else None


def _timestamp_range(timestamps: list[int]) -> dict[str, int] | None:
    if not timestamps:
        return None
    return {"min": min(timestamps), "max": max(timestamps)}


def _window_relation(
    prompt_range: dict[str, int] | None,
    attempt_range: dict[str, int] | None,
) -> str:
    if not prompt_range or not attempt_range:
        return "unknown"
    if attempt_range["max"] < prompt_range["min"]:
        return "precedes"
    if attempt_range["min"] > prompt_range["max"]:
        return "follows"
    return "overlaps"


def _atomic_write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary_path = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(payload, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary_path, path)
    finally:
        temporary_path.unlink(missing_ok=True)


def audit(
    *,
    prompt_budget_path: Path,
    attempts_path: Path,
    from_ts: int,
    to_ts: int | None,
    limit: int,
) -> dict[str, Any]:
    if from_ts < 0:
        raise ValueError("from_ts must be non-negative")
    if to_ts is not None and to_ts < from_ts:
        raise ValueError("to_ts must be at or after from_ts")
    if limit <= 0:
        raise ValueError("limit must be positive")

    records, prompt_coverage = _read_window(
        prompt_budget_path,
        timestamp=_prompt_timestamp,
        from_ts=from_ts,
        to_ts=to_ts,
        limit=limit,
    )
    attempts, attempt_coverage = _read_window(
        attempts_path,
        timestamp=_attempt_timestamp,
        from_ts=from_ts,
        to_ts=to_ts,
        limit=limit,
    )

    budget_groups: Counter[tuple[int | None, int | None]] = Counter()
    mode_groups: dict[float | None, list[float]] = defaultdict(list)
    mode_counts: Counter[float | None] = Counter()
    suffocation_states: Counter[str] = Counter()
    clamp_count = 0
    runtime_effect_true = 0
    removed_fractions: list[float] = []
    timestamps: list[int] = []

    for record in records:
        requested = _integer(record.get("requested_tokens"))
        effective = _integer(record.get("effective_tokens"))
        budget_groups[(requested, effective)] += 1
        if requested is not None and effective is not None and effective < requested:
            clamp_count += 1
        if record.get("diagnostic_runtime_effect") is True:
            runtime_effect_true += 1
        ts = _prompt_timestamp(record)
        if ts is not None:
            timestamps.append(ts)

        pressure = _pressure_observation(record)
        mode = _finite_float(pressure.get("mode_packing"))
        fraction = _removed_fraction(record)
        mode_counts[mode] += 1
        if fraction is not None:
            removed_fractions.append(fraction)
            mode_groups[mode].append(fraction)
        context = record.get("prompt_context_observation_v3")
        if isinstance(context, dict):
            state = str(context.get("suffocation_risk") or "unavailable")
            suffocation_states[state] += 1

    prompt_range = prompt_coverage["retained_timestamp_range"]
    attempt_range = attempt_coverage["retained_timestamp_range"]
    attempt_prompt_relation = _window_relation(prompt_range, attempt_range)
    overlap_range = None
    if attempt_prompt_relation == "overlaps":
        overlap_range = {
            "min": max(prompt_range["min"], attempt_range["min"]),
            "max": min(prompt_range["max"], attempt_range["max"]),
        }
    overlapping_attempts = [
        record
        for record in attempts
        if overlap_range is not None
        and (ts := _attempt_timestamp(record)) is not None
        and overlap_range["min"] <= ts <= overlap_range["max"]
    ]
    overlapping_prompts = [
        record
        for record in records
        if overlap_range is not None
        and (ts := _prompt_timestamp(record)) is not None
        and overlap_range["min"] <= ts <= overlap_range["max"]
    ]

    latency_rows: list[tuple[str, float]] = []
    for record in overlapping_attempts:
        latency = _latency_seconds(record)
        if latency is not None:
            latency_rows.append(latency)

    constant_budget = len(budget_groups) == 1 and len(records) > 1
    distinct_mode_values = sorted(mode for mode in mode_counts if mode is not None)
    mode_varied = len(distinct_mode_values) > 1
    continuity_evictions = sum(
        count
        for state, count in suffocation_states.items()
        if "continuity_eviction" in state
    )
    prompt_packing_observed = any(value > 0.0 for value in removed_fractions)
    budget_rows = [
        {
            "requested_tokens": requested,
            "effective_tokens": effective,
            "count": count,
        }
        for (requested, effective), count in sorted(
            budget_groups.items(),
            key=lambda item: (
                item[0][0] is None,
                item[0][0] or 0,
                item[0][1] is None,
                item[0][1] or 0,
            ),
        )
    ]
    mode_rows = [
        {
            "mode_packing": mode,
            "count": mode_counts[mode],
            "removed_fraction_count": len(mode_groups[mode]),
            "mean_removed_fraction": _rounded(_mean(mode_groups[mode])),
        }
        for mode in sorted(
            mode_counts,
            key=lambda value: (value is None, value if value is not None else 0.0),
        )
    ]
    latency_fields = Counter(name for name, _ in latency_rows)
    latency_values = [value for _, value in latency_rows]

    return {
        "schema": POLICY,
        "schema_version": 1,
        "window": {
            "from_timestamp_unix_s": from_ts,
            "to_timestamp_unix_s": to_ts,
            "retention_limit": limit,
        },
        "sources": {
            "dialogue_prompt_budget": {
                "path": str(prompt_budget_path),
                "sha256": _sha256(prompt_budget_path),
                **prompt_coverage,
            },
            "dialogue_live_attempts": {
                "path": str(attempts_path),
                "sha256": _sha256(attempts_path),
                **attempt_coverage,
            },
        },
        "prompt_budget_observation": {
            "timestamp_min": min(timestamps, default=None),
            "timestamp_max": max(timestamps, default=None),
            "budget_groups": budget_rows,
            "constant_requested_and_effective_budget": constant_budget,
            "clamp_count": clamp_count,
            "mode_packing_present_count": sum(
                count for mode, count in mode_counts.items() if mode is not None
            ),
            "mode_packing_missing_count": mode_counts[None],
            "distinct_mode_packing_values": distinct_mode_values,
            "mode_packing_varied": mode_varied,
            "mode_packing_groups": mode_rows,
            "removed_fraction_count": len(removed_fractions),
            "removed_fraction_min": _rounded(min(removed_fractions, default=None)),
            "removed_fraction_max": _rounded(max(removed_fractions, default=None)),
            "removed_fraction_mean": _rounded(_mean(removed_fractions)),
            "prompt_context_packing_observed": prompt_packing_observed,
            "suffocation_states": [
                {"state": state, "count": count}
                for state, count in sorted(suffocation_states.items())
            ],
            "continuity_eviction_count": continuity_evictions,
            "diagnostic_runtime_effect_true_count": runtime_effect_true,
        },
        "machine_latency_observation": {
            "attempt_records_in_window": len(attempts),
            "attempt_records_in_overlap": len(overlapping_attempts),
            "prompt_records_in_overlap": len(overlapping_prompts),
            "latency_records_in_window": len(latency_rows),
            "latency_fields": dict(sorted(latency_fields.items())),
            "latency_seconds_min": _rounded(min(latency_values, default=None)),
            "latency_seconds_max": _rounded(max(latency_values, default=None)),
            "latency_seconds_mean": _rounded(_mean(latency_values)),
            "correlation_available": bool(latency_rows and overlapping_prompts),
            "attempt_prompt_window_relation": attempt_prompt_relation,
            "relation_basis": "retained_timestamp_ranges",
            "overlap_timestamp_range": overlap_range,
            "attempt_stream_precedes_prompt_window": (
                attempt_prompt_relation == "precedes"
            ),
        },
        "disposition": {
            "requested_token_band_within_window_explanation": (
                "constant_budget_cannot_explain_within_window_variation"
                if constant_budget
                else "budget_varied_or_sample_insufficient"
            ),
            "mode_packing_association_state": (
                "mode_varied_with_observational_prompt_packing_evidence"
                if mode_varied and prompt_packing_observed
                else "mode_or_prompt_packing_variation_insufficient"
            ),
            "latency_claim_state": (
                "bounded_machine_latency_comparison_available"
                if latency_rows
                else "machine_latency_evidence_unavailable_in_window"
            ),
            "causal_result": "not_established_by_observational_logs",
            "felt_result": "not_inferred_from_machine_records",
            "live_retuning_result": "not_authorized_or_performed",
        },
        "authority": AUTHORITY,
    }


class DialogueTokenPressureAuditTests(unittest.TestCase):
    def _write_jsonl(self, path: Path, rows: list[dict[str, Any]]) -> None:
        path.write_text(
            "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows),
            encoding="utf-8",
        )

    def test_constant_budget_separates_band_from_prompt_packing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            budgets = root / "budgets.jsonl"
            attempts = root / "attempts.jsonl"
            rows = []
            for ts, mode, removed, state in (
                (100, 0.30, 0.52, "observed_continuity_eviction"),
                (101, 0.33, 0.61, "not_high_entropy_specific"),
                (102, None, 0.57, "observed_continuity_eviction"),
            ):
                rows.append(
                    {
                        "schema": "dialogue_prompt_budget_v3",
                        "timestamp": str(ts),
                        "requested_tokens": 768,
                        "effective_tokens": 768,
                        "diagnostic_runtime_effect": False,
                        "prompt_context_observation_v3": {
                            "removed_fraction": removed,
                            "suffocation_risk": state,
                            "felt_pressure_observation_v3": {
                                "mode_packing": mode,
                            },
                        },
                    }
                )
            self._write_jsonl(budgets, rows)
            self._write_jsonl(attempts, [])

            payload = audit(
                prompt_budget_path=budgets,
                attempts_path=attempts,
                from_ts=100,
                to_ts=102,
                limit=50,
            )

            observation = payload["prompt_budget_observation"]
            self.assertTrue(observation["constant_requested_and_effective_budget"])
            self.assertTrue(observation["mode_packing_varied"])
            self.assertEqual(observation["continuity_eviction_count"], 2)
            self.assertEqual(observation["clamp_count"], 0)
            self.assertEqual(
                payload["disposition"]["requested_token_band_within_window_explanation"],
                "constant_budget_cannot_explain_within_window_variation",
            )
            self.assertFalse(payload["authority"]["causal_effect_established"])
            self.assertFalse(payload["authority"]["runtime_mutated"])

    def test_latency_requires_an_explicit_numeric_field(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            budgets = root / "budgets.jsonl"
            attempts = root / "attempts.jsonl"
            self._write_jsonl(
                budgets,
                [
                    {
                        "timestamp": "200",
                        "requested_tokens": 768,
                        "effective_tokens": 512,
                        "budget_report": {"total_before": 1_000, "total_after": 400},
                        "prompt_context_observation_v3": {
                            "suffocation_risk": "bounded_overflow",
                            "felt_pressure_observation_v3": {"mode_packing": 0.32},
                        },
                    }
                ],
            )
            self._write_jsonl(
                attempts,
                [
                    {"timestamp_unix_s": 200, "outcome": "success"},
                    {"timestamp_unix_s": 200, "outcome": "success", "elapsed_ms": 2_500},
                ],
            )

            payload = audit(
                prompt_budget_path=budgets,
                attempts_path=attempts,
                from_ts=200,
                to_ts=200,
                limit=50,
            )

            self.assertEqual(payload["prompt_budget_observation"]["clamp_count"], 1)
            latency = payload["machine_latency_observation"]
            self.assertTrue(latency["correlation_available"])
            self.assertEqual(latency["latency_records_in_window"], 1)
            self.assertEqual(latency["latency_seconds_mean"], 2.5)

    def test_window_is_bounded_and_discloses_omitted_records(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            budgets = root / "budgets.jsonl"
            attempts = root / "attempts.jsonl"
            self._write_jsonl(
                budgets,
                [
                    {
                        "timestamp": str(ts),
                        "requested_tokens": 768,
                        "effective_tokens": 768,
                    }
                    for ts in range(10)
                ],
            )
            self._write_jsonl(attempts, [])

            payload = audit(
                prompt_budget_path=budgets,
                attempts_path=attempts,
                from_ts=0,
                to_ts=9,
                limit=3,
            )

            source = payload["sources"]["dialogue_prompt_budget"]
            self.assertEqual(source["matching_records"], 10)
            self.assertEqual(source["retained_records"], 3)
            self.assertEqual(source["earlier_matching_records_omitted"], 7)
            self.assertEqual(source["source_timestamp_min"], 0)
            self.assertEqual(source["source_timestamp_max"], 9)
            self.assertEqual(source["matching_timestamp_range"], {"min": 0, "max": 9})
            self.assertEqual(source["retained_timestamp_range"], {"min": 7, "max": 9})
            self.assertEqual(payload["prompt_budget_observation"]["timestamp_min"], 7)

    def _relation_fixture(
        self,
        *,
        prompt_timestamps: list[int],
        attempt_rows: list[dict[str, Any]],
    ) -> dict[str, Any]:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            budgets = root / "budgets.jsonl"
            attempts = root / "attempts.jsonl"
            self._write_jsonl(
                budgets,
                [
                    {
                        "timestamp": ts,
                        "requested_tokens": 768,
                        "effective_tokens": 768,
                    }
                    for ts in prompt_timestamps
                ],
            )
            self._write_jsonl(attempts, attempt_rows)
            return audit(
                prompt_budget_path=budgets,
                attempts_path=attempts,
                from_ts=0,
                to_ts=None,
                limit=50,
            )

    def test_attempt_window_precedes_prompt_window(self) -> None:
        payload = self._relation_fixture(
            prompt_timestamps=[200, 201],
            attempt_rows=[{"timestamp_unix_s": 100, "elapsed_ms": 2500}],
        )
        latency = payload["machine_latency_observation"]
        self.assertEqual(latency["attempt_prompt_window_relation"], "precedes")
        self.assertTrue(latency["attempt_stream_precedes_prompt_window"])
        self.assertEqual(latency["attempt_records_in_overlap"], 0)
        self.assertEqual(latency["latency_records_in_window"], 0)
        self.assertFalse(latency["correlation_available"])

    def test_attempt_window_follows_prompt_window(self) -> None:
        payload = self._relation_fixture(
            prompt_timestamps=[100, 101],
            attempt_rows=[{"timestamp_unix_s": 200, "elapsed_ms": 2500}],
        )
        latency = payload["machine_latency_observation"]
        self.assertEqual(latency["attempt_prompt_window_relation"], "follows")
        self.assertFalse(latency["attempt_stream_precedes_prompt_window"])
        self.assertEqual(latency["attempt_records_in_overlap"], 0)

    def test_latency_uses_only_overlapping_attempts(self) -> None:
        payload = self._relation_fixture(
            prompt_timestamps=[100, 110],
            attempt_rows=[
                {"timestamp_unix_s": 90, "elapsed_ms": 9000},
                {"timestamp_unix_s": 105, "elapsed_ms": 2000},
            ],
        )
        latency = payload["machine_latency_observation"]
        self.assertEqual(latency["attempt_prompt_window_relation"], "overlaps")
        self.assertEqual(latency["overlap_timestamp_range"], {"min": 100, "max": 105})
        self.assertEqual(latency["attempt_records_in_overlap"], 1)
        self.assertEqual(latency["latency_records_in_window"], 1)
        self.assertEqual(latency["latency_seconds_mean"], 2.0)
        self.assertTrue(latency["correlation_available"])

    def test_empty_attempt_window_has_unknown_relation(self) -> None:
        payload = self._relation_fixture(
            prompt_timestamps=[100],
            attempt_rows=[],
        )
        latency = payload["machine_latency_observation"]
        self.assertEqual(latency["attempt_prompt_window_relation"], "unknown")
        self.assertIsNone(latency["overlap_timestamp_range"])
        self.assertFalse(latency["attempt_stream_precedes_prompt_window"])
        self.assertFalse(latency["correlation_available"])

    def test_output_path_is_atomic_json_projection(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "nested" / "audit.json"
            payload = {"schema": POLICY, "value": 3}
            _atomic_write_json(path, payload)
            self.assertEqual(json.loads(path.read_text(encoding="utf-8")), payload)
            self.assertEqual(list(path.parent.glob(f".{path.name}.*.tmp")), [])


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Read-only audit of dialogue token budgets and spectral pressure context."
    )
    parser.add_argument("--prompt-budget-path", type=Path, default=DEFAULT_DIAGNOSTIC)
    parser.add_argument("--attempts-path", type=Path, default=DEFAULT_ATTEMPTS)
    parser.add_argument("--from-ts", type=int, default=0)
    parser.add_argument("--to-ts", type=int)
    parser.add_argument("--limit", type=int, default=500)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(
            DialogueTokenPressureAuditTests
        )
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    payload = audit(
        prompt_budget_path=args.prompt_budget_path,
        attempts_path=args.attempts_path,
        from_ts=args.from_ts,
        to_ts=args.to_ts,
        limit=args.limit,
    )
    if args.output:
        _atomic_write_json(args.output, payload)
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        observation = payload["prompt_budget_observation"]
        disposition = payload["disposition"]
        print("# Dialogue Token Pressure Audit")
        print(f"- records: {payload['sources']['dialogue_prompt_budget']['retained_records']}")
        print(f"- budget groups: {observation['budget_groups']}")
        print(
            "- mode packing values: "
            f"{observation['distinct_mode_packing_values']}"
        )
        print(
            "- mean removed fraction: "
            f"{observation['removed_fraction_mean']}"
        )
        print(f"- band disposition: {disposition['requested_token_band_within_window_explanation']}")
        print(f"- latency disposition: {disposition['latency_claim_state']}")
        print("- authority: observational evidence only; no live retuning or felt inference")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
