#!/usr/bin/env python3
"""Developer-only temporal metric qualification on bounded synthetic traces.

No live recorder, private text, engine, inference, network or control imports.
These functions describe frozen coordinates; they do not estimate causal memory.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path

DIM = 128
MAX_FRAMES = 180
RECIPE = "frozen-activation-temporal-distance-v1"
LIMITS = (
    "Endpoint distances, not continuous trajectories across gaps. No interpolation, "
    "automatic threshold search, causal memory duration or experiential inference. "
    "Synthetic fixtures are not recordings or simulations of the running engine."
)


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, allow_nan=False).encode()).hexdigest()


def validate(frames):
    if not isinstance(frames, list) or not 32 <= len(frames) <= MAX_FRAMES:
        raise ValueError("32..180 frames required")
    previous = -1
    for frame in frames:
        if not isinstance(frame, dict) or set(frame) != {"t_ms", "activations"}:
            raise ValueError("exact timestamp and raw activation fields required")
        t, values = frame["t_ms"], frame["activations"]
        if type(t) is not int or not previous < t <= 2**53:
            raise ValueError("strictly increasing exact nonnegative timestamps required")
        if (not isinstance(values, list) or len(values) != DIM
                or any(type(x) not in (float, int) or abs(x) > 1 or not math.isfinite(x) for x in values)):
            raise ValueError("128 finite unsanitized coordinates in [-1,1] required")
        previous = t
    if frames[-1]["t_ms"] - frames[0]["t_ms"] > 180_000:
        raise ValueError("recording exceeds 180 seconds")


def rms(left, right):
    return math.sqrt(math.fsum((float(a) - float(b)) ** 2 for a, b in zip(left, right)) / DIM)


def gaps(frames):
    return [(a["t_ms"], b["t_ms"]) for a, b in zip(frames, frames[1:])
            if b["t_ms"] - a["t_ms"] > 1500]


def lag_profile(frames, lags_ms, tolerance_ms=0):
    """All unordered endpoint pairs in each explicitly selected lag band."""
    validate(frames)
    if (not isinstance(lags_ms, (list, tuple)) or not 1 <= len(lags_ms) <= 8
            or any(type(lag) is not int or not 1 <= lag <= 180_000 for lag in lags_ms)
            or len(set(lags_ms)) != len(lags_ms)):
        raise ValueError("one to eight distinct positive bounded lags required")
    if type(tolerance_ms) is not int or not 0 <= tolerance_ms <= 1000:
        raise ValueError("tolerance must be 0..1000 ms")
    if any(lag <= tolerance_ms for lag in lags_ms):
        raise ValueError("lag band must exclude zero")
    holes = gaps(frames)
    rows = []
    for lag in lags_ms:
        pairs = [(a, b) for i, a in enumerate(frames) for b in frames[i + 1:]
                 if abs((b["t_ms"] - a["t_ms"]) - lag) <= tolerance_ms]
        distances = [rms(a["activations"], b["activations"]) for a, b in pairs]
        elapsed = [b["t_ms"] - a["t_ms"] for a, b in pairs]
        rows.append({"requested_lag_ms": lag, "pair_count": len(pairs),
                     "mean_rms": math.fsum(distances) / len(distances) if distances else None,
                     "min_rms": min(distances) if distances else None,
                     "max_rms": max(distances) if distances else None,
                     "elapsed_range_ms": [min(elapsed), max(elapsed)] if elapsed else None,
                     "pairs_crossing_gap": sum(any(a["t_ms"] <= lo < hi <= b["t_ms"]
                                                   for lo, hi in holes) for a, b in pairs)})
    return {"recipe": RECIPE, "source_sha256": digest(frames), "frame_count": len(frames),
            "coverage_ms": frames[-1]["t_ms"] - frames[0]["t_ms"],
            "gap_intervals_ms": holes, "tolerance_ms": tolerance_ms, "lags": rows,
            "limits": LIMITS}


def paired_curve(left, right):
    """Describe matched fixture clocks; matching does not establish shared identity."""
    validate(left)
    validate(right)
    if [f["t_ms"] for f in left] != [f["t_ms"] for f in right]:
        raise ValueError("matched timestamps required; no alignment or interpolation")
    origin = left[0]["t_ms"]
    return {"recipe": RECIPE, "source_sha256": [digest(left), digest(right)],
            "comparison": "exploratory_separate_traces_no_boot_or_node_identity_attested",
            "gap_intervals_ms": gaps(left), "limits": LIMITS,
            "samples": [{"t_ms": a["t_ms"], "elapsed_ms": a["t_ms"] - origin,
                         "rms": rms(a["activations"], b["activations"])}
                        for a, b in zip(left, right)]}


def fixtures():
    def trace(fn):
        return [{"t_ms": 100_000 + i * 1000,
                 "activations": [fn(i, j) for j in range(DIM)]} for i in range(64)]
    periodic = trace(lambda i, j: .4 * math.sin(2 * math.pi * i / 8 + j / 16))
    reordered = [{"t_ms": periodic[i]["t_ms"],
                  "activations": periodic[(i * 13) % 64]["activations"][:]} for i in range(64)]
    return {"constant": trace(lambda i, j: .2), "zero": trace(lambda i, j: 0.),
            "fading": trace(lambda i, j: .5 * .8**i), "persistent": trace(lambda i, j: .5),
            "periodic": periodic, "reordered": reordered,
            "drift": trace(lambda i, j: -.4 + i * .01),
            "gapped": [f for i, f in enumerate(periodic) if not 24 <= i < 32]}


def plan():
    return {"recipe": RECIPE, "implementation_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "input_origin": "developer_synthetic_fixtures_not_being_history_or_prediction",
            "lags_ms": [1000, 4000, 8000, 16000], "tolerance_ms": 0,
            "identity_tolerance": 1e-12, "limits": LIMITS,
            "expected_controls": ["constant_zero_lag_distance", "identical_trace_zero_distance",
                "analytic_fade_matches", "persistent_does_not_fade", "periodic_return_at_eight_seconds",
                "reordering_changes_short_lag", "gaps_are_visible", "input_bytes_preserved"],
            "live_state_changed": False, "control_authority": False}


def qualify(preregistration):
    if preregistration != plan():
        raise ValueError("plan or implementation identity differs")
    inputs = fixtures()
    before = digest(inputs)
    profiles = {name: lag_profile(value, preregistration["lags_ms"]) for name, value in inputs.items()}
    fade = paired_curve(inputs["fading"], inputs["zero"])
    persist = paired_curve(inputs["persistent"], inputs["zero"])
    identical = paired_curve(inputs["periodic"], inputs["periodic"])
    tolerance = preregistration["identity_tolerance"]
    checks = {
        "constant_zero_lag_distance": all(row["mean_rms"] == 0 for row in profiles["constant"]["lags"]),
        "identical_trace_zero_distance": all(row["rms"] == 0 for row in identical["samples"]),
        "analytic_fade_matches": all(abs(row["rms"] - .5 * .8**i) <= tolerance
                                     for i, row in enumerate(fade["samples"])),
        "persistent_does_not_fade": all(row["rms"] == .5 for row in persist["samples"]),
        "periodic_return_at_eight_seconds": profiles["periodic"]["lags"][2]["max_rms"] <= tolerance,
        "reordering_changes_short_lag": abs(profiles["periodic"]["lags"][0]["mean_rms"]
                                            - profiles["reordered"]["lags"][0]["mean_rms"]) > .01,
        "gaps_are_visible": bool(profiles["gapped"]["gap_intervals_ms"])
                            and profiles["gapped"]["lags"][3]["pairs_crossing_gap"] > 0,
        "input_bytes_preserved": before == digest(inputs),
    }
    return {"schema": "temporal_trace_qualification_v1", "plan_sha256": digest(preregistration),
            "fixture_sha256": before, "checks": checks,
            "outcome": "passed" if all(checks.values()) else "failed",
            "lag_profiles": profiles, "fading_curve": fade, "persistent_curve": persist,
            "live_engine_memory_duration_established": False, "felt_state_conclusion": None,
            "remaining_gate": "isolated production ESN paired-history replay with complete state and common future"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="new offline artifact directory")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    registration = plan()
    def write(name, value):
        with (args.output / name).open("x") as out:
            json.dump(value, out, indent=2, sort_keys=True, allow_nan=False)
            out.write("\n")
    write("plan.json", registration)
    result = qualify(registration)
    write("result.json", result)
    print(json.dumps({"outcome": result["outcome"], "checks": result["checks"],
                      "output": str(args.output), "live_state_changed": False}))
    return 0 if result["outcome"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
