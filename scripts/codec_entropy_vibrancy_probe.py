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
CODEC_REPLAY_LABS_ROOT = ASTRID_WORKSPACE / "diagnostics/codec_replay_labs"

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
        "sample_id": "high_entropy_high_semantic_density",
        "spectral_entropy": 0.96,
        "tail_texture": 0.78,
        "warmth_signal": 0.62,
        "tension_signal": 0.26,
        "content_density": 0.82,
        "fill_pct": 73.0,
        "pressure_risk": 0.19,
        "intent": "Same entropy/tail texture as the low-content case, but with semantic density that should justify tail vibrancy.",
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

NARRATIVE_ARC_SAMPLES = (
    {
        "sample_id": "balanced_valence_flip",
        "segments": [0.78, 0.68, -0.70, -0.82],
        "description": "A clear halfway pivot; current first-half/second-half arc should see it.",
    },
    {
        "sample_id": "late_negative_pivot_after_long_warm_start",
        "segments": [0.72, 0.64, 0.58, -0.88],
        "description": "A late sharp pivot; equal-half averaging may understate the recent negative direction.",
    },
    {
        "sample_id": "steady_warm_no_pivot",
        "segments": [0.46, 0.50, 0.48, 0.52],
        "description": "A steady arc; temporal decay should not invent a pivot.",
    },
)


def run_id() -> str:
    return dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")


def latest_rust_replay_artifact() -> Path | None:
    if not CODEC_REPLAY_LABS_ROOT.exists():
        return None
    candidates = [
        path
        for path in CODEC_REPLAY_LABS_ROOT.glob("*/codec_replay_lab.json")
        if path.is_file()
    ]
    if not candidates:
        return None
    return max(candidates, key=lambda path: path.stat().st_mtime)


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


def build_semantic_density_contrast(samples: list[dict[str, object]]) -> dict[str, object]:
    by_id = {str(sample.get("sample_id")): sample for sample in samples}
    low = by_id.get("high_entropy_low_content")
    high = by_id.get("high_entropy_high_semantic_density")
    if not low or not high:
        return {
            "policy": "semantic_density_contrast_v1",
            "authority": "diagnostic_context_not_command",
            "status": "insufficient_samples",
            "content_blind_lift_risk": False,
        }
    current_delta = abs(
        float(high["current_tail_vibrancy"]) - float(low["current_tail_vibrancy"])
    )
    content_delta = float(high["content_density"]) - float(low["content_density"])
    content_blind = bool(
        content_delta >= 0.5
        and current_delta <= 0.02
        and bool(low.get("current_shimmer_risk"))
    )
    return {
        "policy": "semantic_density_contrast_v1",
        "authority": "diagnostic_context_not_command",
        "status": (
            "content_blind_lift_risk"
            if content_blind
            else "semantic_density_contrast_clear"
        ),
        "content_blind_lift_risk": content_blind,
        "pair": [str(low.get("sample_id")), str(high.get("sample_id"))],
        "spectral_entropy": low.get("spectral_entropy"),
        "tail_texture": low.get("tail_texture"),
        "content_density_delta": round(content_delta, 4),
        "current_tail_vibrancy_delta": round(current_delta, 4),
        "low_content_classification": low.get("classification"),
        "high_content_classification": high.get("classification"),
        "recommended_action": (
            "If entropy/tail lift is identical for low- and high-density samples, "
            "treat this as proposal evidence for content-aware vibrancy gating, not "
            "as permission to change runtime codec math yet."
        ),
    }


def weighted_average(values: list[float], weights: list[float]) -> float:
    total = sum(weights)
    if total <= 0.0:
        return 0.0
    return sum(value * weight for value, weight in zip(values, weights)) / total


def evaluate_narrative_arc_sample(sample: dict[str, object]) -> dict[str, object]:
    segments = [float(value) for value in sample.get("segments", [])]
    if len(segments) < 2:
        return {
            **sample,
            "classification": "insufficient_segments",
            "current_arc_delta": 0.0,
            "temporal_decay_delta": 0.0,
        }
    midpoint = len(segments) // 2
    first = segments[:midpoint]
    second = segments[midpoint:]
    first_avg = sum(first) / max(1, len(first))
    second_avg = sum(second) / max(1, len(second))
    current_delta_raw = second_avg - first_avg
    weights = [0.35 ** (len(segments) - idx - 1) for idx in range(len(segments))]
    recent_weighted = weighted_average(segments, weights)
    temporal_delta_raw = recent_weighted - first_avg
    current_arc_delta = math.tanh(3.0 * current_delta_raw)
    temporal_decay_delta = math.tanh(3.0 * temporal_delta_raw)
    sign_flip = min(first) >= 0.0 and min(second) < 0.0
    late_flip = sign_flip and segments[-1] < -0.5 and second[0] >= 0.0
    decay_strengthens = abs(temporal_delta_raw) > abs(current_delta_raw) + 0.12
    if late_flip and decay_strengthens:
        classification = "temporal_decay_candidate"
    elif sign_flip and abs(current_arc_delta) >= 0.6:
        classification = "current_arc_captures_pivot"
    elif not sign_flip and abs(current_arc_delta) < 0.2 and abs(temporal_decay_delta) < 0.2:
        classification = "stable_no_pivot"
    else:
        classification = "static_average_blur_risk"
    return {
        **sample,
        "first_half_average": round(first_avg, 4),
        "second_half_average": round(second_avg, 4),
        "recent_weighted_average": round(recent_weighted, 4),
        "current_arc_delta": round(current_arc_delta, 4),
        "temporal_decay_delta": round(temporal_decay_delta, 4),
        "current_arc_delta_raw": round(current_delta_raw, 4),
        "temporal_decay_delta_raw": round(temporal_delta_raw, 4),
        "sign_flip": sign_flip,
        "late_flip": late_flip,
        "temporal_decay_strengthens_recent_pivot": decay_strengthens,
        "classification": classification,
    }


def build_narrative_arc_temporal_decay() -> dict[str, object]:
    samples = [
        evaluate_narrative_arc_sample(dict(sample))
        for sample in NARRATIVE_ARC_SAMPLES
    ]
    temporal_candidates = [
        sample
        for sample in samples
        if sample.get("classification") == "temporal_decay_candidate"
    ]
    blur_risks = [
        sample
        for sample in samples
        if sample.get("classification") == "static_average_blur_risk"
    ]
    pivot_capture = [
        sample
        for sample in samples
        if sample.get("classification") == "current_arc_captures_pivot"
    ]
    return {
        "policy": "narrative_arc_temporal_decay_v1",
        "authority": "diagnostic_context_not_command",
        "status": (
            "temporal_decay_candidate"
            if temporal_candidates
            else "static_average_blur_risk"
            if blur_risks
            else "current_arc_captures_clean_pivots"
            if pivot_capture
            else "quiet"
        ),
        "temporal_decay_candidate_count": len(temporal_candidates),
        "static_average_blur_risk_count": len(blur_risks),
        "current_arc_capture_count": len(pivot_capture),
        "samples": samples,
        "recommended_action": (
            "Use these fixture deltas to decide whether a future Rust replay should "
            "add temporal-decay or pivot-detection evidence to narrative arc dims. "
            "Do not change live codec narrative_arc math from this probe alone."
        ),
    }


def build_record(*, output_root: Path, run: str) -> dict[str, object]:
    samples = [evaluate_sample(dict(sample)) for sample in SAMPLES]
    semantic_density_contrast = build_semantic_density_contrast(samples)
    narrative_arc_temporal_decay = build_narrative_arc_temporal_decay()
    rust_replay = latest_rust_replay_artifact()
    shimmer_count = sum(1 for sample in samples if sample["current_shimmer_risk"])
    improved_count = sum(
        1
        for sample in samples
        if sample["classification"] == "current_overload_candidate_improves"
    )
    temporal_decay_count = int(
        narrative_arc_temporal_decay.get("temporal_decay_candidate_count", 0) or 0
    )
    status = (
        "semantic_density_and_temporal_decay_probe_needed"
        if semantic_density_contrast.get("content_blind_lift_risk") and temporal_decay_count
        else "content_blind_vibrancy_probe_needed"
        if semantic_density_contrast.get("content_blind_lift_risk")
        else "narrative_temporal_decay_probe_needed"
        if temporal_decay_count
        else "current_overload_candidate_improves"
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
        "rust_replay_available": rust_replay is not None,
        "rust_replay_artifact_path": str(rust_replay) if rust_replay else None,
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
        "semantic_density_contrast_v1": semantic_density_contrast,
        "narrative_arc_temporal_decay_v1": narrative_arc_temporal_decay,
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
        f"- rust_replay_available: `{record.get('rust_replay_available')}`",
        "",
        "## Samples",
        "",
    ]
    if record.get("rust_replay_artifact_path"):
        lines.insert(
            7,
            f"- rust_replay_artifact: `{record.get('rust_replay_artifact_path')}`",
        )
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
    contrast = record.get("semantic_density_contrast_v1") or {}
    if isinstance(contrast, dict):
        lines.extend(
            [
                "",
                "## Semantic Density Contrast",
                "",
                f"- status: `{contrast.get('status')}`",
                f"- pair: `{contrast.get('pair')}`",
                f"- content_density_delta: `{contrast.get('content_density_delta')}`",
                f"- current_tail_vibrancy_delta: `{contrast.get('current_tail_vibrancy_delta')}`",
                f"- content_blind_lift_risk: `{contrast.get('content_blind_lift_risk')}`",
            ]
        )
    narrative = record.get("narrative_arc_temporal_decay_v1") or {}
    if isinstance(narrative, dict):
        lines.extend(["", "## Narrative Arc Temporal Decay", ""])
        lines.append(
            f"- status: `{narrative.get('status')}`; "
            f"temporal_decay_candidates={narrative.get('temporal_decay_candidate_count')}; "
            f"blur_risks={narrative.get('static_average_blur_risk_count')}; "
            f"current_arc_capture={narrative.get('current_arc_capture_count')}"
        )
        for sample in (narrative.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            lines.append(
                f"- `{sample.get('sample_id')}` class=`{sample.get('classification')}`; "
                f"current_delta={sample.get('current_arc_delta')}; "
                f"temporal_decay_delta={sample.get('temporal_decay_delta')}; "
                f"late_flip={sample.get('late_flip')}"
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
