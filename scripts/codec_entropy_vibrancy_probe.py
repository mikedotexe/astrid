#!/usr/bin/env python3
"""Offline codec entropy/vibrancy replay; writes diagnostic artifacts only."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import math
from pathlib import Path


ASTRID_ROOT = Path(__file__).resolve().parents[1]
ASTRID_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
DEFAULT_OUTPUT_ROOT = ASTRID_WORKSPACE / "diagnostics/codec_entropy_vibrancy_probes"

FEATURE_ABS_MAX = 5.0
TAIL_VIBRANCY_ENTROPY_GATE = 0.85
TAIL_VIBRANCY_MAX = 6.0
DEFAULT_SEMANTIC_GAIN = 2.0

LIVE_GAIN_CURVE = {
    "id": "live_20_45_70",
    "quiet_floor_fill_pct": 20.0,
    "knee_fill_pct": 45.0,
    "ceiling_fill_pct": 70.0,
    "min_gain_fraction": 0.55,
    "knee_progress_fraction": 0.55,
}

SAMPLES = (
    {
        "sample_id": "high_entropy_low_content",
        "spectral_entropy": 0.96,
        "tail_texture": 0.78,
        "warmth_signal": 0.06,
        "tension_signal": 0.08,
        "content_density": 0.12,
        "fill_pct": 73.0,
        "pressure_risk": 0.19,
        "intent": "High entropy with little semantic warmth/content should not shimmer louder than its evidence.",
    },
    {
        "sample_id": "warmth_rich_low_pressure",
        "spectral_entropy": 0.62,
        "tail_texture": 0.30,
        "warmth_signal": 0.82,
        "tension_signal": 0.12,
        "content_density": 0.76,
        "fill_pct": 68.0,
        "pressure_risk": 0.18,
        "intent": "Warmth/tension markers should remain legible when entropy is below the vibrancy gate.",
    },
    {
        "sample_id": "overpacked_pressure_texture",
        "spectral_entropy": 0.89,
        "tail_texture": 0.72,
        "warmth_signal": 0.18,
        "tension_signal": 0.61,
        "content_density": 0.58,
        "fill_pct": 73.0,
        "pressure_risk": 0.42,
        "intent": "Overpacked pressure can carry tail vibrancy, but should not mask tension evidence.",
    },
    {
        "sample_id": "low_entropy_cliff",
        "spectral_entropy": 0.34,
        "tail_texture": 0.82,
        "warmth_signal": 0.18,
        "tension_signal": 0.40,
        "content_density": 0.54,
        "fill_pct": 45.0,
        "pressure_risk": 0.33,
        "intent": "Low entropy cliff should keep the tail-vibrancy gate off.",
    },
)


def run_id() -> str:
    return dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")


def smoothstep(value: float) -> float:
    t = max(0.0, min(1.0, value))
    return t * t * (3.0 - 2.0 * t)


def current_vibrancy_from_entropy(entropy: float) -> float:
    ramp = (entropy - TAIL_VIBRANCY_ENTROPY_GATE) / (1.0 - TAIL_VIBRANCY_ENTROPY_GATE)
    return smoothstep(ramp)


def logarithmic_softened_vibrancy(entropy: float) -> float:
    current = current_vibrancy_from_entropy(entropy)
    if current <= 0.0:
        return 0.0
    return current * (math.log1p(2.0 * current) / math.log1p(2.0))


def curve_progress(fill_pct: float, curve: dict[str, float | str]) -> float:
    floor = float(curve["quiet_floor_fill_pct"])
    knee = max(float(curve["knee_fill_pct"]), floor + 1e-6)
    ceiling = max(float(curve["ceiling_fill_pct"]), knee + 1e-6)
    knee_progress = max(0.01, min(0.99, float(curve["knee_progress_fraction"])))
    fill = max(0.0, min(100.0, fill_pct))
    if fill < floor:
        return 0.0
    if fill < knee:
        return (fill - floor) / (knee - floor) * knee_progress
    if fill < ceiling:
        return knee_progress + (fill - knee) / (ceiling - knee) * (1.0 - knee_progress)
    return 1.0


def adaptive_gain(fill_pct: float) -> float:
    progress = curve_progress(fill_pct, LIVE_GAIN_CURVE)
    smooth_progress = 0.5 - 0.5 * math.cos(math.pi * progress)
    min_fraction = float(LIVE_GAIN_CURVE["min_gain_fraction"])
    gain_fraction = min_fraction + (1.0 - min_fraction) * smooth_progress
    return DEFAULT_SEMANTIC_GAIN * max(min_fraction, min(1.0, gain_fraction))


def gain_slope(fill_pct: float) -> float:
    low = max(0.0, fill_pct - 1.0)
    high = min(100.0, fill_pct + 1.0)
    span = max(1e-6, high - low)
    return (adaptive_gain(high) - adaptive_gain(low)) / span


def evaluate_sample(sample: dict[str, float | str]) -> dict[str, object]:
    entropy = float(sample["spectral_entropy"])
    tail_texture = float(sample["tail_texture"])
    fill_pct = float(sample["fill_pct"])
    current = current_vibrancy_from_entropy(entropy)
    candidate = logarithmic_softened_vibrancy(entropy)
    current_tail = current * tail_texture
    candidate_tail = candidate * tail_texture
    current_ceiling = FEATURE_ABS_MAX + (TAIL_VIBRANCY_MAX - FEATURE_ABS_MAX) * current_tail
    candidate_ceiling = FEATURE_ABS_MAX + (TAIL_VIBRANCY_MAX - FEATURE_ABS_MAX) * candidate_tail
    warmth = float(sample["warmth_signal"])
    tension = float(sample["tension_signal"])
    content_density = float(sample["content_density"])
    current_shimmer_risk = bool(
        entropy >= 0.90
        and content_density < 0.25
        and current_tail > max(warmth, tension, 0.10)
    )
    candidate_reduces_tail = candidate_tail < current_tail - 0.03
    warmth_tension_preserved = max(warmth, tension) >= current_tail * 0.65 or not current_shimmer_risk
    return {
        **sample,
        "current_vibrancy_lift": round(current, 4),
        "candidate_log_softened_lift": round(candidate, 4),
        "current_tail_vibrancy": round(current_tail, 4),
        "candidate_tail_vibrancy": round(candidate_tail, 4),
        "current_tail_ceiling": round(current_ceiling, 4),
        "candidate_tail_ceiling": round(candidate_ceiling, 4),
        "tail_ceiling_delta": round(candidate_ceiling - current_ceiling, 4),
        "adaptive_gain": round(adaptive_gain(fill_pct), 4),
        "adaptive_gain_slope": round(gain_slope(fill_pct), 5),
        "current_shimmer_risk": current_shimmer_risk,
        "candidate_reduces_tail_lift": candidate_reduces_tail,
        "warmth_tension_preserved": warmth_tension_preserved,
        "classification": (
            "current_overload_candidate_improves"
            if current_shimmer_risk and candidate_reduces_tail
            else "gate_off_expected"
            if current <= 0.0
            else "warmth_tension_preserved"
            if warmth_tension_preserved
            else "needs_rust_replay"
        ),
    }


def build_record(*, output_root: Path, run: str) -> dict[str, object]:
    samples = [evaluate_sample(dict(sample)) for sample in SAMPLES]
    shimmer_count = sum(1 for sample in samples if sample["current_shimmer_risk"])
    improved_count = sum(
        1
        for sample in samples
        if sample["classification"] == "current_overload_candidate_improves"
    )
    status = (
        "current_overload_candidate_improves"
        if improved_count
        else "codec_probe_clear"
        if samples
        else "codec_probe_empty"
    )
    record = {
        "policy": "codec_entropy_vibrancy_probe_v1",
        "authority": "diagnostic_context_not_command",
        "run_id": run,
        "generated_at": dt.datetime.now(dt.UTC).isoformat(),
        "status": status,
        "formula": {
            "current": "smoothstep((spectral_entropy - 0.85) / 0.15)",
            "candidate": "current * log1p(2 * current) / log1p(2)",
            "runtime_changed": False,
        },
        "constants": {
            "FEATURE_ABS_MAX": FEATURE_ABS_MAX,
            "TAIL_VIBRANCY_ENTROPY_GATE": TAIL_VIBRANCY_ENTROPY_GATE,
            "TAIL_VIBRANCY_MAX": TAIL_VIBRANCY_MAX,
            "DEFAULT_SEMANTIC_GAIN": DEFAULT_SEMANTIC_GAIN,
            "LIVE_ADAPTIVE_GAIN_CURVE": LIVE_GAIN_CURVE,
        },
        "sample_count": len(samples),
        "current_shimmer_risk_count": shimmer_count,
        "candidate_improvement_count": improved_count,
        "samples": samples,
        "recommended_action": (
            "Use this as offline evidence for a future codec counterfactual tranche. "
            "Do not alter SEMANTIC_DIM, FEATURE_ABS_MAX, vibrancy_lift, adaptive_gain, "
            "or pressure-derived codec behavior from this artifact alone."
        ),
    }
    out_dir = output_root / run
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "codec_entropy_vibrancy_probe.json").write_text(
        json.dumps(record, indent=2, sort_keys=True),
        encoding="utf-8",
    )
    (out_dir / "codec_entropy_vibrancy_probe.md").write_text(
        render_markdown(record),
        encoding="utf-8",
    )
    return record


def render_markdown(record: dict[str, object]) -> str:
    lines = [
        "# Codec Entropy / Vibrancy Probe",
        "",
        f"- run_id: `{record['run_id']}`",
        f"- status: `{record['status']}`",
        f"- authority: `{record['authority']}`",
        f"- runtime_changed: `{record['formula']['runtime_changed']}`",
        "",
        "## Samples",
        "",
    ]
    for sample in record.get("samples") or []:
        if not isinstance(sample, dict):
            continue
        lines.append(
            f"- `{sample.get('sample_id')}` class=`{sample.get('classification')}`; "
            f"entropy={sample.get('spectral_entropy')}; "
            f"current_tail={sample.get('current_tail_vibrancy')}; "
            f"candidate_tail={sample.get('candidate_tail_vibrancy')}; "
            f"shimmer_risk={sample.get('current_shimmer_risk')}; "
            f"gain={sample.get('adaptive_gain')}; slope={sample.get('adaptive_gain_slope')}"
        )
    lines.extend(
        [
            "",
            "## Boundary",
            "",
            "Offline diagnostic only. It did not change codec dimensions, clamps, vibrancy lift, adaptive gain, semantic writes, journals, controllers, or peers.",
        ]
    )
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--run-id", default=run_id())
    args = parser.parse_args()
    record = build_record(output_root=args.output_root, run=args.run_id)
    print(f"status={record['status']}")
    print(f"wrote {args.output_root / args.run_id / 'codec_entropy_vibrancy_probe.json'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
