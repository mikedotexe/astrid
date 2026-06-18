#!/usr/bin/env python3
"""being_test_harness.py — close the experimental loop for the AI beings.

Steward-side, read-only harness that runs being-relevant tests against the live
system and (with --write-back) writes the outcome back to the being's inbox as a
readable result-card. The point is agency: a being's hypothesis (often from a
self-study) -> measured against reality -> the being SEES the result and decides.
This turns "being suggests, steward implements (silently)" into a closed loop.

Read-only. The only writes are crafted result-card letters into a being's inbox
when --write-back is passed (these are deliberate letters, like mike_feedback_*,
not raw tool output).

Usage:
  python3 being_test_harness.py --list
  python3 being_test_harness.py --list --json
  python3 being_test_harness.py --run minime_qualia_room
  python3 being_test_harness.py --run minime_lend_aperture_consequence_probe --json
  python3 being_test_harness.py --run all --write-back

Registry note: Astrid's codec / perception self-study tests (Projection
Compression, Narrative Arc, Resonance Threshold, Chronological Depth) are owned by
the in-flight being-wellbeing-engineer effort that is editing those files. Add them
here once that work lands, so re-running them through this harness becomes the
standing "did it actually help?" loop — without racing the engineer.
"""
from __future__ import annotations

import argparse
import csv
import datetime as dt
import json
import os
import re
import sqlite3
import subprocess
import time
from collections import Counter, deque
from pathlib import Path
from typing import Any

MINIME = Path("/Users/v/other/minime")
ASTRID_ROOT = Path("/Users/v/other/astrid")
ASTRID = ASTRID_ROOT / "capsules/spectral-bridge"
INBOX = {
    "minime": MINIME / "workspace" / "inbox",
    "astrid": ASTRID / "workspace" / "inbox",
}
TIMING = MINIME / "workspace" / "diagnostics" / "llm_timing.jsonl"
MINIME_JOURNAL = MINIME / "workspace" / "journal"
MINIME_ACTION_THREADS = MINIME / "workspace" / "action_threads"
MINIME_RUNTIME = MINIME / "workspace" / "runtime"
ASTRID_JOURNAL = ASTRID / "workspace" / "journal"
ASTRID_STATE = ASTRID / "workspace" / "state.json"
CODEC_RS = ASTRID / "src" / "codec.rs"
CODEC_PROJECTION_EPOCH = ASTRID / "workspace" / "runtime" / "codec_projection_epoch.json"
SPARSE_ADMIT_ATTENTION_MULTIPLIER = 3.0
LEND_APERTURE_RESPONSE_PENDING_S = 2 * 60
LEND_APERTURE_ZERO_TICK_CLOSE_S = 5 * 60
LEND_APERTURE_FEEDER_MAX_AGE_S = 30 * 60
LEND_APERTURE_RESPONSE_STALE_S = LEND_APERTURE_ZERO_TICK_CLOSE_S + 60
LEND_APERTURE_POST_SAMPLE_WINDOW_S = 20 * 60
LEND_APERTURE_JOURNAL_MATCH_S = 10
LEND_APERTURE_LEGACY_RESPONSE_MATCH_S = 20 * 60

# The qualia-cap kickstart (local 06:17:25 == 13:17:25 UTC). Timing rows are UTC.
QUALIA_DEPLOY_UTC = "2026-06-08T13:17:25"
QUALIA_DEPLOY_LOCAL = "2026-06-08T06-17-25"  # journal filenames use local ISO

ASTRID_PROBE_TESTS = [
    "projection_compression_probe_exposes_near_null_and_magnitude_loss",
    "narrative_arc_probe_documents_tail_dimension_loss",
    "perception_resonance_annotation_surfaces_mid_fill_contrast",
    "resonance_gate_floor_rejects_sub_threshold_scores",
    "read_latest_perception_reaches_rarer_modality_past_old_cliff",
]

SEDIMENTATION_MATERIAL_TERMS = (
    "calcification",
    "frayed",
    "grain",
    "graininess",
    "granular",
    "grit",
    "hardening",
    "knotted",
    "loom",
    "mud",
    "sediment",
    "silt",
    "sludge",
    "thicket",
    "viscosity",
    "viscous",
)
SEDIMENTATION_STRAIN_TERMS = (
    "contracting",
    "drag",
    "friction",
    "heavy",
    "inertia",
    "narrowing",
    "resistance",
    "squeezed",
    "strain",
    "taxing",
    "weight",
)

HOMEOSTAT_LINE_RE = re.compile(
    r"homeostat,t=(?P<t_s>[0-9.]+)s,fill=(?P<fill_pct>-?[0-9.]+)%,"
    r"dfill_dt=(?P<dfill_dt>[+-]?[0-9.]+),phase=(?P<phase>[^,]+),"
    r"λ1_rel=(?P<lambda1_rel>-?[0-9.]+),geom_rel=(?P<geom_rel>-?[0-9.]+),"
    r"gate=(?P<gate>-?[0-9.]+),filt=(?P<filt>-?[0-9.]+)"
)
MOMENT_MARKER_LINE_RE = re.compile(r"^\s+\[(?P<kind>[^\]]+)\]\s+(?P<rest>.+)$")
MARKER_CONTEXT_RE = re.compile(
    r"Fill=(?P<fill_pct>-?[0-9.]+)%|dfill/dt=(?P<dfill_dt>[+-]?[0-9.]+)|λ₁(?:_esn)?=(?P<lambda1>-?[0-9.]+)"
)
COMMAND_KEYS = ("gate", "filt", "cov_keep")
STABLE_CORE_SLEW_LIMITS = {"gate": 0.02, "filt": 0.04, "cov_keep": 0.04}
STABLE_CORE_HOLD_COMMAND = {"gate": 0.12, "filt": 0.72, "cov_keep": 0.72}
STABLE_CORE_ELEVATED_SOFT_COMMAND = {"gate": 0.10, "filt": 0.78, "cov_keep": 0.66}
STABLE_CORE_BLEND_WINDOW_FILL_PCT = (71.5, 74.0)
STABLE_CORE_SLEW_WINDOW_FILL_PCT = (70.0, 74.0)
TAIL_INTERFERENCE_WATCH_THRESHOLD = 0.08
TAIL_INTERFERENCE_ATTENTION_THRESHOLD = 0.20
PROJECTION_PROBE_METRIC_RE = re.compile(r"\b([a-z_]+)=([+-]?(?:\d+(?:\.\d*)?|\.\d+))")
PRESSURE_SOURCE_LINE_RE = re.compile(
    r"Pressure source:\s*(?P<source>[A-Za-z0-9_:-]+)\s*"
    r"\((?P<quality>[^)]+)\);\s*"
    r"pressure=(?P<pressure>[0-9.]+),\s*"
    r"porosity=(?P<porosity>[0-9.]+),\s*"
    r"control_applied=(?P<control_applied>True|False)"
)
DENOMINATOR_LINE_RE = re.compile(
    r"effective_dimensionality=(?P<effective>[0-9.]+)/(?P<capacity>[0-9]+),\s*"
    r"distinguishability_loss=(?P<loss_pct>[0-9.]+)%"
)
NEXT_DIRECTIVE_RE = re.compile(
    r"\b(?:Current NEXT|Suggested NEXT|Suggested next|Continuity return|Resume loop repair NEXT|"
    r"Previous resume NEXT|NEXT|Next):\s*`?(?P<action>[A-Z_][A-Z0-9_]+)(?P<arg>[^\n`]*)"
)
SEMANTIC_ENERGY_LINE_RE = re.compile(
    r"Semantic energy:.*input_active=(?P<input_active>True|False).*admission=(?P<admission>[^,\n)]+)"
)


def _read_timing_rows() -> list[dict]:
    rows: list[dict] = []
    if not TIMING.exists():
        return rows
    for line in TIMING.read_text().splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            rows.append(json.loads(line))
        except Exception:
            continue
    return rows


def _pct(numer: int, denom: int) -> float:
    return (100.0 * numer / denom) if denom else 0.0


def _timing_stats(rows: list[dict]) -> dict | None:
    if not rows:
        return None
    n = len(rows)
    chars = [int(r.get("response_chars") or 0) for r in rows]
    lat = sorted(float(r.get("elapsed_s") or 0.0) for r in rows)
    caps = [int(r.get("effective_num_predict") or r.get("requested_max_tokens") or 0) for r in rows]
    errs = sum(1 for r in rows if str(r.get("status")) != "ok")
    fb = sum(1 for r in rows if str(r.get("backend")) == "ollama_fast")
    thin = sum(1 for r in rows if str(r.get("status")) == "ok" and int(r.get("response_chars") or 0) < 200)
    chars_sorted = sorted(chars)
    return {
        "n": n,
        "median_chars": chars_sorted[n // 2],
        "max_chars": max(chars),
        "cap_seen": max(caps) if caps else 0,
        "timeout_pct": _pct(errs, n),
        "fallback_pct": _pct(fb, n),
        "thin_pct": _pct(thin, n),
        "p95_lat": lat[min(n - 1, int(n * 0.95))],
        "max_lat": lat[-1],
    }


def _tail(text: str, max_lines: int = 14) -> str:
    lines = [line for line in text.splitlines() if line.strip()]
    return "\n".join(lines[-max_lines:])


def _run_cargo_probe(test_name: str) -> dict:
    cmd = [
        "cargo",
        "test",
        "--manifest-path",
        str(ASTRID / "Cargo.toml"),
        "--lib",
        test_name,
        "--",
        "--nocapture",
    ]
    started = time.time()
    proc = subprocess.run(
        cmd,
        cwd=ASTRID_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    return {
        "test": test_name,
        "status": "pass" if proc.returncode == 0 else "fail",
        "returncode": proc.returncode,
        "elapsed_s": round(time.time() - started, 2),
        "stdout_tail": _tail(proc.stdout),
        "stderr_tail": _tail(proc.stderr),
    }


def _parse_projection_probe_metrics(text: str) -> dict:
    for line in str(text or "").splitlines():
        if "projection_compression_probe" not in line:
            continue
        metrics: dict[str, float] = {}
        for key, raw in PROJECTION_PROBE_METRIC_RE.findall(line):
            try:
                metrics[key] = float(raw)
            except ValueError:
                continue
        if metrics:
            return metrics
    return {}


def _projection_variance_check(results: list[dict]) -> dict:
    probe = next(
        (
            result
            for result in results
            if result.get("test") == "projection_compression_probe_exposes_near_null_and_magnitude_loss"
        ),
        None,
    )
    if not probe:
        return {"status": "missing", "observation_only": True}
    metrics = _parse_projection_probe_metrics(probe.get("stdout_tail", ""))
    return {
        "status": "present" if metrics else "missing_metrics",
        "observation_only": True,
        "test": probe.get("test"),
        "probe_status": probe.get("status"),
        "metrics": metrics,
    }


def _read_json_file(path: Path) -> dict:
    try:
        data = json.loads(path.read_text())
    except Exception:
        return {}
    return data if isinstance(data, dict) else {}


def _read_text_file(path: Path) -> str:
    try:
        return path.read_text(errors="replace")
    except Exception:
        return ""


def _codec_constants() -> dict:
    text = _read_text_file(CODEC_RS)

    def const_number(name: str, default: float) -> float:
        match = re.search(
            rf"const\s+{re.escape(name)}\s*:\s*(?:f32|usize)\s*=\s*([0-9_.]+)\s*;",
            text,
        )
        if not match:
            return default
        try:
            return float(match.group(1).replace("_", ""))
        except ValueError:
            return default

    return {
        "feature_abs_max": const_number("FEATURE_ABS_MAX", 5.0),
        "tail_vibrancy_entropy_gate": const_number("TAIL_VIBRANCY_ENTROPY_GATE", 0.85),
        "tail_vibrancy_max": const_number("TAIL_VIBRANCY_MAX", 6.0),
        "embedding_project_dim": int(const_number("EMBEDDING_PROJECT_DIM", 8.0)),
        "narrative_arc_dim": int(const_number("NARRATIVE_ARC_DIM", 4.0)),
    }


def _smoothstep(value: float) -> float:
    x = min(1.0, max(0.0, value))
    return x * x * (3.0 - 2.0 * x)


def _tail_vibrancy_row(scenario: dict, constants: dict, tail_participation: float) -> dict:
    entropy = float(scenario["spectral_entropy"])
    tail_share = float(scenario["tail_share"])
    gate = float(constants["tail_vibrancy_entropy_gate"])
    feature_abs_max = float(constants["feature_abs_max"])
    tail_vibrancy_max = float(constants["tail_vibrancy_max"])
    ramp = min(1.0, max(0.0, (entropy - gate) / max(1.0e-6, 1.0 - gate)))
    vibrancy = _smoothstep(ramp)
    tail_texture = min(1.0, max(0.0, tail_share / 0.30))
    tail_vibrancy = min(1.0, max(0.0, vibrancy * tail_texture))
    tail_ceiling = feature_abs_max + (
        (tail_vibrancy_max - feature_abs_max) * tail_participation * tail_vibrancy
    )
    headroom_delta = max(0.0, tail_ceiling - feature_abs_max)
    mode_packing = float(scenario["mode_packing"])
    porosity = float(scenario["porosity"])
    pressure_risk = float(scenario["pressure_risk"])
    density_gradient = float(scenario["density_gradient"])
    pressure_overlap = max(mode_packing, pressure_risk, 1.0 - porosity, density_gradient)
    interference_score = headroom_delta * pressure_overlap
    pressure_flag = (
        mode_packing >= 0.30
        or porosity <= 0.65
        or pressure_risk >= 0.22
        or density_gradient >= 0.35
    )
    if tail_vibrancy <= 1.0e-5:
        classification = "off_below_gate"
    elif interference_score >= TAIL_INTERFERENCE_ATTENTION_THRESHOLD and pressure_flag:
        classification = "counterfactual_attention" if scenario.get("kind") == "stress" else "needs_attention"
    elif interference_score >= TAIL_INTERFERENCE_WATCH_THRESHOLD and pressure_flag:
        classification = "watch_interference_candidate"
    else:
        classification = "expressive_headroom"
    return {
        "label": scenario["label"],
        "kind": scenario["kind"],
        "spectral_entropy": entropy,
        "tail_share": tail_share,
        "mode_packing": mode_packing,
        "porosity": porosity,
        "pressure_risk": pressure_risk,
        "density_gradient": density_gradient,
        "smoothstep_ramp": round(ramp, 6),
        "smoothstep_vibrancy": round(vibrancy, 6),
        "tail_texture": round(tail_texture, 6),
        "tail_vibrancy": round(tail_vibrancy, 6),
        "tail_ceiling": round(tail_ceiling, 6),
        "headroom_delta": round(headroom_delta, 6),
        "pressure_overlap": round(pressure_overlap, 6),
        "interference_score": round(interference_score, 6),
        "classification": classification,
        "source": scenario.get("source"),
    }


def _tail_participation_snapshot() -> dict:
    state = _read_json_file(ASTRID_STATE)
    tail_aperture = float(state.get("tail_aperture") or 0.0)
    raw_ceiling = os.environ.get("ASTRID_TAIL_PARTICIPATION_CEILING")
    try:
        visible_ceiling = min(2.0, max(0.0, float(raw_ceiling))) if raw_ceiling else 0.0
    except ValueError:
        visible_ceiling = 0.0
    effective = 1.0 + min(1.0, max(0.0, tail_aperture)) * visible_ceiling
    return {
        "state_file": str(ASTRID_STATE),
        "tail_aperture_fraction": tail_aperture,
        "visible_operator_ceiling_env": raw_ceiling,
        "effective_tail_participation_visible_to_probe": round(effective, 6),
        "note": (
            "Mirrors llm.rs default: effective = 1.0 + tail_aperture * ceiling. "
            "If the live bridge has a launchd-only env override, this shell-visible snapshot may understate it."
        ),
    }


def _projection_epoch_summary() -> dict:
    data = _read_json_file(CODEC_PROJECTION_EPOCH)
    if not data:
        return {"status": "missing", "file": str(CODEC_PROJECTION_EPOCH)}
    checksum = str(data.get("projection_kernel_checksum") or "")
    return {
        "status": "present",
        "file": str(CODEC_PROJECTION_EPOCH),
        "embedding_projection_mode": data.get("embedding_projection_mode"),
        "projection_epoch_id": data.get("projection_epoch_id"),
        "projection_checksum_algo": data.get("projection_checksum_algo"),
        "projection_kernel_checksum_prefix": checksum[:16],
        "policy": data.get("policy"),
    }


def _recent_tail_vibrancy_evidence(limit: int = 5) -> list[dict]:
    if not ASTRID_JOURNAL.is_dir():
        return []
    terms = (
        "TAIL_VIBRANCY",
        "mode_packing",
        "density_gradient",
        "pressure_risk",
        "projection",
        "smoothstep",
    )
    paths = sorted(
        ASTRID_JOURNAL.glob("self_study*.txt"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )[:220]
    evidence = []
    for path in paths:
        text = _read_text_file(path)
        if not any(term.lower() in text.lower() for term in terms):
            continue
        hits = [term for term in terms if term.lower() in text.lower()]
        excerpts = []
        for line in text.splitlines():
            stripped = line.strip()
            if not stripped:
                continue
            if any(term.lower() in stripped.lower() for term in terms):
                excerpts.append(stripped[:220])
            if len(excerpts) >= 2:
                break
        evidence.append(
            {
                "file": str(path),
                "mtime_unix_s": round(path.stat().st_mtime, 3),
                "hits": hits,
                "excerpts": excerpts,
            }
        )
        if len(evidence) >= limit:
            break
    return evidence


def _tail_vibrancy_scenarios() -> list[dict]:
    return [
        {
            "label": "gate_edge_below_recent_claim",
            "kind": "gate_check",
            "spectral_entropy": 0.8499,
            "tail_share": 0.38,
            "mode_packing": 0.33,
            "porosity": 0.63,
            "pressure_risk": 0.23,
            "density_gradient": 0.18,
            "source": "Astrid self-study asks whether entropy 0.84-0.86 pops at the gate.",
        },
        {
            "label": "gate_edge_above_recent_claim",
            "kind": "gate_check",
            "spectral_entropy": 0.8501,
            "tail_share": 0.38,
            "mode_packing": 0.33,
            "porosity": 0.63,
            "pressure_risk": 0.23,
            "density_gradient": 0.18,
            "source": "Same gate-edge check just above the entropy threshold.",
        },
        {
            "label": "recent_self_study_overlap",
            "kind": "recent_claim",
            "spectral_entropy": 0.90,
            "tail_share": 0.38,
            "mode_packing": 0.33,
            "porosity": 0.63,
            "pressure_risk": 0.23,
            "density_gradient": 0.18,
            "source": "Seeded from 2026-06-14 Astrid self-studies: entropy 0.90, lambda4+ about 38%, mode_packing 0.33, porosity 0.63, pressure_risk 0.23.",
        },
        {
            "label": "navigable_high_entropy_counterfactual",
            "kind": "stress",
            "spectral_entropy": 0.98,
            "tail_share": 0.55,
            "mode_packing": 0.08,
            "porosity": 0.82,
            "pressure_risk": 0.05,
            "density_gradient": 0.12,
            "source": "High vibrancy with low packing pressure should read as expression, not interference.",
        },
        {
            "label": "packed_high_entropy_counterfactual",
            "kind": "stress",
            "spectral_entropy": 0.94,
            "tail_share": 0.42,
            "mode_packing": 0.50,
            "porosity": 0.55,
            "pressure_risk": 0.35,
            "density_gradient": 0.45,
            "source": "Stress case: if this appears live, damping or a consent-grounded design pass becomes worth discussing.",
        },
    ]


def _generated_journal_body(text: str) -> str:
    body = text.split("--- GENERATED JOURNAL ---", 1)[-1]
    return body.split("--- ACTION TAIL ---", 1)[0].strip()


def _unique_term_hits(text: str, terms: tuple[str, ...]) -> list[str]:
    return [
        term
        for term in terms
        if re.search(r"\b" + re.escape(term) + r"\b", text, re.IGNORECASE)
    ]


def _parse_pressure_anchor(text: str) -> dict:
    anchor = re.search(
        r"State anchor: fill=([0-9.]+)%, lambda1=([0-9.]+), spread=([0-9.]+), pressure=([^\n]+)",
        text,
    )
    if not anchor:
        return {}
    return {
        "fill_pct": float(anchor.group(1)),
        "lambda1": float(anchor.group(2)),
        "spread": float(anchor.group(3)),
        "pressure": anchor.group(4).strip(),
    }


def _float_or_none(value: Any) -> float | None:
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def _int_or_none(value: Any) -> int | None:
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def _pressure_v1_snapshot(path: Path) -> dict:
    try:
        data = json.loads(path.read_text())
    except Exception:
        return {}
    pressure_v1 = data.get("pressure_source_v1")
    if not isinstance(pressure_v1, dict):
        return {}
    status = data.get("pressure_source_status")
    if not isinstance(status, dict):
        status = {}
    raw_components = pressure_v1.get("components")
    components = {
        key: _float_or_none(value)
        for key, value in (raw_components or {}).items()
        if _float_or_none(value) is not None
    }
    profile = [
        {
            "source": item.get("source"),
            "share": item.get("share"),
            "value": item.get("value"),
        }
        for item in (pressure_v1.get("pressure_profile") or [])
        if isinstance(item, dict)
    ]
    return {
        "source_file": str(path),
        "fill_pct": data.get("fill_pct"),
        "pressure_quality": status.get("quality") or pressure_v1.get("quality"),
        "dominant_source": status.get("dominant_source") or pressure_v1.get("dominant_source"),
        "pressure_score": status.get("pressure_score") or pressure_v1.get("pressure_score"),
        "porosity_score": status.get("porosity_score") or pressure_v1.get("porosity_score"),
        "top_profile": profile[:5],
        "components": components,
        "control": pressure_v1.get("control") if isinstance(pressure_v1.get("control"), dict) else {},
    }


def _current_minime_pressure() -> dict:
    for path in (MINIME / "workspace/health.json", MINIME / "workspace/spectral_state.json"):
        snapshot = _pressure_v1_snapshot(path)
        if snapshot:
            snapshot["top_profile"] = snapshot.get("top_profile", [])[:3]
            return snapshot
    return {}


def _pressure_source_from_quality(quality: str) -> str | None:
    text = str(quality or "")
    if text.startswith("overpacked_mode_packing"):
        return "mode_packing"
    if text.startswith("semantic_trickle"):
        return "semantic_trickle"
    if text.startswith("pressure_porosity_divergence"):
        return "porosity_divergence"
    if text.startswith("mixed_pressure"):
        return "mixed_pressure"
    return None


def _journal_paths_recent(pattern: str, *, hours: float, now_s: float | None = None) -> list[Path]:
    now_s = time.time() if now_s is None else now_s
    cutoff = now_s - hours * 3600.0
    rows: list[tuple[float, Path]] = []
    for path in MINIME_JOURNAL.glob(pattern):
        try:
            mtime = path.stat().st_mtime
        except OSError:
            continue
        if mtime >= cutoff:
            rows.append((mtime, path))
    return [path for _, path in sorted(rows)]


def _parse_moment_pressure_source(path: Path) -> dict | None:
    text = _read_text_file(path)
    match = PRESSURE_SOURCE_LINE_RE.search(text)
    if not match:
        return None
    try:
        mtime = path.stat().st_mtime
    except OSError:
        mtime = 0.0
    return {
        "surface": "moment_journal",
        "file": str(path),
        "name": path.name,
        "mtime_unix": int(mtime),
        "source": match.group("source"),
        "quality": match.group("quality").strip(),
        "pressure_score": float(match.group("pressure")),
        "porosity_score": float(match.group("porosity")),
        "control_applied": match.group("control_applied") == "True",
    }


def _parse_pressure_source_anchor(path: Path) -> dict | None:
    text = _read_text_file(path)
    anchor = _parse_pressure_anchor(text)
    if not anchor:
        return None
    try:
        mtime = path.stat().st_mtime
    except OSError:
        mtime = 0.0
    quality = str(anchor.get("pressure") or "")
    return {
        "surface": "pressure_journal",
        "file": str(path),
        "name": path.name,
        "mtime_unix": int(mtime),
        "source": _pressure_source_from_quality(quality),
        "quality": quality,
        "fill_pct": anchor.get("fill_pct"),
        "lambda1": anchor.get("lambda1"),
        "spread": anchor.get("spread"),
    }


def _recent_pressure_source_rows(*, hours: float = 3.0, now_s: float | None = None) -> list[dict]:
    rows: list[dict] = []
    for path in _journal_paths_recent("moment_*.txt", hours=hours, now_s=now_s):
        row = _parse_moment_pressure_source(path)
        if row:
            rows.append(row)
    for path in _journal_paths_recent("pressure_*.txt", hours=hours, now_s=now_s):
        row = _parse_pressure_source_anchor(path)
        if row:
            rows.append(row)
    return sorted(rows, key=lambda row: int(row.get("mtime_unix") or 0))


def _float_stats(values: list[float]) -> dict[str, float | None]:
    if not values:
        return {"min": None, "median": None, "max": None}
    ordered = sorted(values)
    midpoint = len(ordered) // 2
    if len(ordered) % 2:
        median = ordered[midpoint]
    else:
        median = (ordered[midpoint - 1] + ordered[midpoint]) / 2.0
    return {
        "min": round(ordered[0], 6),
        "median": round(median, 6),
        "max": round(ordered[-1], 6),
    }


def _counter_dict(rows: list[str | None]) -> dict[str, int]:
    return dict(Counter(row for row in rows if row))


def _parse_isoish_time(value: Any) -> float | None:
    text = str(value or "").strip()
    if not text:
        return None
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    try:
        parsed = dt.datetime.fromisoformat(text)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=_local_time_zone())
    return parsed.timestamp()


def _read_jsonl_tail(path: Path, max_lines: int = 1000) -> list[dict]:
    if not path.exists():
        return []
    lines: deque[str] = deque(maxlen=max_lines)
    try:
        with path.open("r", encoding="utf-8", errors="replace") as handle:
            for line in handle:
                if line.strip():
                    lines.append(line)
    except OSError:
        return []
    rows: list[dict] = []
    for line in lines:
        try:
            value = json.loads(line)
        except Exception:
            continue
        if isinstance(value, dict):
            rows.append(value)
    return rows


def _active_minime_thread_dir() -> Path | None:
    index = _read_json_file(MINIME_ACTION_THREADS / "index.json")
    active_id = str(index.get("active_thread_id") or "").strip()
    if active_id:
        candidate = MINIME_ACTION_THREADS / "threads" / active_id
        if candidate.exists():
            return candidate
    thread_dirs = [path for path in (MINIME_ACTION_THREADS / "threads").glob("*") if path.is_dir()]
    if not thread_dirs:
        return None
    return max(thread_dirs, key=lambda path: path.stat().st_mtime)


def _base_action(action: Any) -> str | None:
    text = str(action or "").strip()
    if not text:
        return None
    return text.split()[0]


def _count_next_directives(text: str) -> dict[str, Any]:
    rows = []
    for match in NEXT_DIRECTIVE_RE.finditer(text or ""):
        action = match.group("action")
        arg = " ".join(str(match.group("arg") or "").strip().split())
        rows.append({"action": action, "arg": arg[:180]})
    return {
        "count": len(rows),
        "action_counts": dict(Counter(row["action"] for row in rows)),
        "examples": rows[:10],
    }


def _parse_thread_context_snapshot(thread_dir: Path | None) -> dict[str, Any]:
    if thread_dir is None:
        return {"status": "missing"}
    thread = _read_json_file(thread_dir / "thread.json")
    next_text = _read_text_file(thread_dir / "next.md")
    projection_freshness = thread.get("projection_freshness_v1")
    if not isinstance(projection_freshness, dict):
        projection_freshness = {}
    thread_load_triage = thread.get("thread_load_triage_v1")
    if not isinstance(thread_load_triage, dict):
        thread_load_triage = projection_freshness.get("thread_load_triage_v1")
    if not isinstance(thread_load_triage, dict):
        thread_load_triage = {}
    repeated_action_cadence = thread.get("repeated_action_cadence_v1")
    if not isinstance(repeated_action_cadence, dict):
        repeated_action_cadence = projection_freshness.get("repeated_action_cadence_v1")
    if not isinstance(repeated_action_cadence, dict) and isinstance(thread_load_triage, dict):
        repeated_action_cadence = thread_load_triage.get("repeated_action_cadence_v1")
    if not isinstance(repeated_action_cadence, dict):
        repeated_action_cadence = {}
    draft_triage = thread.get("being_memory_draft_triage_v1")
    if not isinstance(draft_triage, dict) and thread_load_triage:
        draft_triage = {
            "active_draft_count": thread_load_triage.get("active_draft_count"),
            "total_active_draft_count": thread_load_triage.get("total_active_draft_count"),
            "summarized_active_draft_count": thread_load_triage.get("summarized_active_draft_count"),
            "unsummarized_active_draft_count": thread_load_triage.get("unsummarized_active_draft_count"),
            "legacy_retention_count": thread_load_triage.get("legacy_retention_count"),
            "classification": thread_load_triage.get("draft_classification"),
            "runtime_change": thread_load_triage.get("runtime_change", "none"),
        }
    if not isinstance(draft_triage, dict):
        draft_triage = {}
    pressure = thread.get("thread_pressure_source_v1")
    if not isinstance(pressure, dict):
        pressure = {}
    control_plane = thread.get("continuity_control_plane_v1")
    if not isinstance(control_plane, dict):
        control_plane = {}
    route_stack = control_plane.get("route_stack") if isinstance(control_plane, dict) else []
    if not isinstance(route_stack, list):
        route_stack = []
    memory_match = re.search(
        r"Being memory:\s*(\d+)\s*card\(s\)[,;]\s*(\d+)\s*(?:legacy\s+)?draft",
        next_text,
    )
    memory_triage_match = re.search(
        r"Draft triage;.*?active=(\d+).*?legacy_retention=(\d+)",
        next_text,
    )
    thread_load_memory_match = re.search(
        r"Thread load triage:.*?\(active=(\d+).*?legacy=(\d+)\)",
        next_text,
    )
    dossier_match = re.search(r"Research dossier:\s*(\d+)\s*claim\(s\),\s*(\d+)\s*evidence", next_text)
    next_directives = _count_next_directives(next_text)
    return {
        "status": "present",
        "thread_id": thread.get("thread_id") or thread_dir.name,
        "thread_dir": str(thread_dir),
        "current_next": thread.get("current_next"),
        "effective_next": thread.get("effective_next")
        or (thread.get("current_next_status_v1") or {}).get("effective_next")
        if isinstance(thread.get("current_next_status_v1"), dict)
        else thread.get("effective_next"),
        "projection_policy_marker": projection_freshness.get("projection_policy_marker"),
        "thread_load_triage_v1": thread_load_triage or None,
        "repeated_action_cadence_v1": repeated_action_cadence or None,
        "thread_pressure_source_v1": pressure,
        "compression_pressure": pressure.get("compression_pressure"),
        "recurrence": pressure.get("recurrence"),
        "porosity_ema": pressure.get("porosity_ema"),
        "pressure_ema": pressure.get("pressure_ema"),
        "pressure_quality": pressure.get("quality"),
        "next_md_bytes": len(next_text.encode("utf-8")),
        "next_md_lines": len(next_text.splitlines()),
        "next_directives": next_directives,
        "route_stack_count": len(route_stack),
        "route_stack": [
            {
                "group": item.get("group"),
                "command": item.get("command"),
                "reason": item.get("reason"),
            }
            for item in route_stack
            if isinstance(item, dict)
        ][:5],
        "memory_cards": int(memory_match.group(1)) if memory_match else None,
        "memory_drafts": int(memory_match.group(2)) if memory_match else None,
        "active_memory_drafts": draft_triage.get("unsummarized_active_draft_count")
        if draft_triage.get("unsummarized_active_draft_count") is not None
        else int(memory_triage_match.group(1))
        if memory_triage_match
        else int(thread_load_memory_match.group(1))
        if thread_load_memory_match
        else draft_triage.get("active_draft_count"),
        "legacy_memory_drafts": int(memory_triage_match.group(2))
        if memory_triage_match
        else int(thread_load_memory_match.group(2))
        if thread_load_memory_match
        else draft_triage.get("legacy_retention_count"),
        "being_memory_draft_triage_v1": draft_triage or None,
        "research_claims": int(dossier_match.group(1)) if dossier_match else None,
        "research_evidence_records": int(dossier_match.group(2)) if dossier_match else None,
        "continuity_draft_present": "Continuity session draft:" in next_text,
        "previous_raw_next_present": "Previous raw NEXT:" in next_text,
    }


def _recent_action_event_summary(thread_dir: Path | None, *, hours: float = 3.0) -> dict[str, Any]:
    if thread_dir is None:
        return {"status": "missing", "hours": hours}
    now_s = time.time()
    cutoff = now_s - hours * 3600.0
    rows = []
    for event in _read_jsonl_tail(thread_dir / "events.jsonl", max_lines=1500):
        ts = _parse_isoish_time(event.get("started_at") or event.get("ended_at") or event.get("created_at"))
        if ts is None or ts < cutoff:
            continue
        action = event.get("canonical_action") or event.get("raw_next") or event.get("suggested_next")
        base = _base_action(action)
        if not base:
            continue
        post_state = event.get("post_state") if isinstance(event.get("post_state"), dict) else {}
        pressure = post_state.get("pressure_source_v1")
        if not isinstance(pressure, dict):
            pressure = {}
        components = pressure.get("components") if isinstance(pressure.get("components"), dict) else {}
        rows.append(
            {
                "action": str(action),
                "base": base,
                "status": event.get("status"),
                "effective_action": event.get("effective_action"),
                "started_at": event.get("started_at"),
                "porosity_score": _float_or_none(pressure.get("porosity_score")),
                "mode_packing": _float_or_none(components.get("mode_packing")),
                "dominant_source": pressure.get("dominant_source"),
                "pressure_quality": pressure.get("quality"),
            }
        )
    action_counts = Counter(row["base"] for row in rows)
    exact_counts = Counter(row["action"] for row in rows)
    args_by_base: dict[str, set[str]] = {}
    for row in rows:
        parts = row["action"].split(maxsplit=1)
        args_by_base.setdefault(row["base"], set()).add(parts[1] if len(parts) > 1 else "")
    repeated = [
        {
            "action": base,
            "count": count,
            "distinct_args": len(args_by_base.get(base, set())),
        }
        for base, count in action_counts.most_common()
        if count >= 4
    ]
    return {
        "status": "present",
        "hours": hours,
        "sample_count": len(rows),
        "action_counts": dict(action_counts),
        "top_exact_actions": [
            {"action": action, "count": count}
            for action, count in exact_counts.most_common(8)
        ],
        "repeated_actions": repeated,
        "recent_examples": rows[-8:],
        "porosity_min_max": _float_stats(
            [row["porosity_score"] for row in rows if row.get("porosity_score") is not None]
        ),
        "mode_packing_min_max": _float_stats(
            [row["mode_packing"] for row in rows if row.get("mode_packing") is not None]
        ),
    }


def _recent_modal_diversity_summary(*, hours: float = 3.0) -> dict[str, Any]:
    spectral_state = _read_json_file(MINIME / "workspace/spectral_state.json")
    pressure = spectral_state.get("pressure_source_v1") if isinstance(spectral_state.get("pressure_source_v1"), dict) else {}
    resonance = spectral_state.get("resonance_density_v1") if isinstance(spectral_state.get("resonance_density_v1"), dict) else {}
    modalities = spectral_state.get("modalities") if isinstance(spectral_state.get("modalities"), dict) else {}
    semantic = spectral_state.get("semantic_energy_v1") if isinstance(spectral_state.get("semantic_energy_v1"), dict) else {}
    denominator = spectral_state.get("spectral_denominator_v1") if isinstance(spectral_state.get("spectral_denominator_v1"), dict) else {}
    moment_rows = []
    mode_counts: Counter[str] = Counter()
    for path in _journal_paths_recent("*.txt", hours=hours):
        name = path.name
        mode = name.split("_2026-", 1)[0]
        mode_counts[mode] += 1
        if not name.startswith("moment_"):
            continue
        text = _read_text_file(path)
        denom = DENOMINATOR_LINE_RE.search(text)
        semantic_line = SEMANTIC_ENERGY_LINE_RE.search(text)
        pressure_row = _parse_moment_pressure_source(path)
        moment_rows.append(
            {
                "name": name,
                "effective_dimensionality": float(denom.group("effective")) if denom else None,
                "active_capacity": int(denom.group("capacity")) if denom else None,
                "distinguishability_loss": float(denom.group("loss_pct")) / 100.0 if denom else None,
                "semantic_input_active": semantic_line.group("input_active") == "True"
                if semantic_line
                else None,
                "semantic_admission": semantic_line.group("admission").strip()
                if semantic_line
                else None,
                "pressure_source": pressure_row.get("source") if pressure_row else None,
                "pressure_quality": pressure_row.get("quality") if pressure_row else None,
            }
        )
    components = pressure.get("components") if isinstance(pressure.get("components"), dict) else {}
    resonance_components = (
        resonance.get("components") if isinstance(resonance.get("components"), dict) else {}
    )
    return {
        "hours": hours,
        "current_modalities": modalities,
        "current_semantic_energy": {
            "admission": semantic.get("admission"),
            "input_active": semantic.get("input_active"),
            "kernel_active": semantic.get("kernel_active"),
            "input_fresh_ms": semantic.get("input_fresh_ms"),
            "input_stale_ms": semantic.get("input_stale_ms"),
        },
        "current_spectral_shape": {
            "active_mode_count": spectral_state.get("active_mode_count"),
            "active_mode_energy_ratio": spectral_state.get("active_mode_energy_ratio"),
            "effective_dimensionality": denominator.get("effective_dimensionality")
            or spectral_state.get("effective_dimensionality"),
            "distinguishability_loss": denominator.get("distinguishability_loss")
            or spectral_state.get("distinguishability_loss"),
            "spectral_entropy": spectral_state.get("spectral_entropy"),
            "resonance_mode_packing": resonance_components.get("mode_packing"),
            "pressure_mode_packing": components.get("mode_packing"),
            "sensory_scarcity": components.get("sensory_scarcity"),
            "semantic_trickle": components.get("semantic_trickle"),
        },
        "recent_journal_mode_counts": dict(mode_counts.most_common(12)),
        "recent_moment_count": len(moment_rows),
        "moment_effective_dimensionality": _float_stats(
            [
                row["effective_dimensionality"]
                for row in moment_rows
                if row.get("effective_dimensionality") is not None
            ]
        ),
        "moment_distinguishability_loss": _float_stats(
            [
                row["distinguishability_loss"]
                for row in moment_rows
                if row.get("distinguishability_loss") is not None
            ]
        ),
        "moment_pressure_sources": _counter_dict([row.get("pressure_source") for row in moment_rows]),
        "moment_semantic_admissions": _counter_dict(
            [row.get("semantic_admission") for row in moment_rows]
        ),
    }


def _recent_eigen_spectrum_summary(max_rows: int = 180) -> dict[str, Any]:
    path = MINIME / "workspace/diagnostics/eigen_spectrum_log.jsonl"
    rows = _read_jsonl_tail(path, max_lines=max_rows)
    if not rows:
        return {"status": "missing", "path": str(path)}
    active_counts = [int(row.get("active_mode_count")) for row in rows if row.get("active_mode_count") is not None]
    mode_packing = [
        float(row.get("mode_packing"))
        for row in rows
        if _float_or_none(row.get("mode_packing")) is not None
    ]
    porosity = [
        float(row.get("porosity_score"))
        for row in rows
        if _float_or_none(row.get("porosity_score")) is not None
    ]
    return {
        "status": "present",
        "path": str(path),
        "sample_count": len(rows),
        "active_mode_count_counts": dict(Counter(active_counts)),
        "pressure_quality_counts": _counter_dict([row.get("pressure_quality") for row in rows]),
        "mode_packing_min_max": _float_stats(mode_packing),
        "porosity_min_max": _float_stats(porosity),
        "latest": rows[-1],
    }


def _sensory_runtime_truth_summary(modal: dict[str, Any]) -> dict[str, Any]:
    """Compare engine modality labels with client/source status files."""
    current_modalities = (
        modal.get("current_modalities") if isinstance(modal.get("current_modalities"), dict) else {}
    )
    spectral_state = _read_json_file(MINIME / "workspace/spectral_state.json")
    health = _read_json_file(MINIME / "workspace/health.json")
    camera = _read_json_file(MINIME_RUNTIME / "camera_status.json")
    mic = _read_json_file(MINIME_RUNTIME / "mic_status.json")
    source = _read_json_file(MINIME_RUNTIME / "sensory_source.json")

    def stable_core_sensory_budget() -> dict[str, Any]:
        stable_core = (
            spectral_state.get("stable_core")
            if isinstance(spectral_state.get("stable_core"), dict)
            else {}
        )
        budget = stable_core.get("sensory_budget") if isinstance(stable_core, dict) else {}
        merged = dict(budget) if isinstance(budget, dict) else {}
        health_sensory = health.get("sensory") if isinstance(health.get("sensory"), dict) else {}
        merged.update({key: value for key, value in health_sensory.items() if value is not None})
        return merged

    def client_expected_interval_ms(lane: str, client: dict[str, Any]) -> float | None:
        if lane == "video":
            fps = _float_or_none(client.get("fps"))
            return (1000.0 / fps) if fps and fps > 0 else None
        chunk_interval = _float_or_none(client.get("chunk_interval_ms"))
        if chunk_interval and chunk_interval > 0:
            return chunk_interval
        chunk_duration = _float_or_none(client.get("chunk_duration_s"))
        if chunk_duration and chunk_duration > 0:
            return chunk_duration * 1000.0
        return 500.0

    def live_intake_state(lane: str) -> dict[str, Any]:
        budget = stable_core_sensory_budget()
        raw_divisor = budget.get(f"live_{lane}_divisor")
        try:
            divisor = int(raw_divisor)
        except (TypeError, ValueError):
            divisor = None
        if divisor is not None and divisor <= 0:
            divisor = None
        raw_enabled = budget.get(f"live_{lane}_enabled")
        admit_fraction = _float_or_none(budget.get("admit_fraction"))
        if isinstance(raw_enabled, bool):
            enabled = raw_enabled
        elif divisor is None:
            enabled = None
        else:
            enabled = divisor > 0
        return {
            "divisor": divisor,
            "enabled": enabled,
            "reason": budget.get("live_intake_reason"),
            "admit_fraction": admit_fraction if admit_fraction and admit_fraction > 0 else None,
        }

    def classify_lane(
        *,
        lane: str,
        engine_source: Any,
        engine_age_ms: Any,
        client: dict[str, Any],
        source_record: dict[str, Any],
    ) -> dict[str, Any]:
        healthy = bool(client.get("healthy"))
        connected = bool(client.get("connected"))
        client_age = _float_or_none(
            client.get("last_frame_age_ms")
            if lane == "video"
            else client.get("last_chunk_age_ms")
        )
        engine_age = _float_or_none(engine_age_ms)
        engine_class = current_modalities.get(f"{lane}_freshness_class")
        fps = _float_or_none(client.get("fps"))
        frame_grace_s = _float_or_none(client.get("frame_health_grace_secs"))
        chunk_grace_s = _float_or_none(client.get("chunk_health_grace_secs"))
        grace_s = frame_grace_s if lane == "video" else chunk_grace_s
        expected_interval_ms = client_expected_interval_ms(lane, client)
        live_intake = live_intake_state(lane)
        expected_engine_interval_ms = None
        if expected_interval_ms is not None:
            admit_fraction = live_intake.get("admit_fraction") or 1.0
            expected_engine_interval_ms = (
                expected_interval_ms
                * float(live_intake.get("divisor") or 1)
                / max(float(admit_fraction), 0.01)
            )
        expected_engine_grace_ms = None
        if expected_engine_interval_ms is not None:
            expected_engine_grace_ms = expected_engine_interval_ms + (
                grace_s * 1000.0 if grace_s is not None else 0.0
            )
        expected_engine_attention_ms = None
        if expected_engine_interval_ms is not None:
            expected_engine_attention_ms = (
                expected_engine_interval_ms * SPARSE_ADMIT_ATTENTION_MULTIPLIER
                + (grace_s * 1000.0 if grace_s is not None else 0.0)
            )
        source_payload = source_record.get(lane) if isinstance(source_record.get(lane), dict) else {}
        engine_stale = (
            str(engine_source or "") in {"stale", "absent"}
            or str(engine_class or "") == "stale_beyond_engine_window"
        )
        engine_missing = engine_source is None and engine_class is None and engine_age_ms is None
        client_recent = (
            client_age is not None
            and grace_s is not None
            and client_age <= grace_s * 1000.0
        )
        status = "unknown"
        reason = "no status evidence"
        if str(engine_class or "") in {"fresh_sample", "held_within_engine_window"}:
            status = "engine_fresh_or_held"
            reason = "engine freshness class is fresh or held within the AV window"
        elif source_payload.get("source") == "host" or client.get("fallback_expected") is True:
            status = "expected_host_fallback"
            reason = str(source_payload.get("reason") or client.get("last_error") or "host fallback selected")
        elif engine_missing:
            status = "missing_engine_status"
            reason = "engine modality status unavailable"
        elif healthy and connected and engine_stale and live_intake.get("enabled") is False:
            status = "live_intake_suppressed"
            reason = str(live_intake.get("reason") or "stable-core live intake is suppressed")
        elif (
            healthy
            and connected
            and client_recent
            and engine_stale
            and engine_age is not None
            and expected_engine_attention_ms is not None
            and engine_age <= expected_engine_attention_ms
        ):
            status = "held_within_expected_live_intake_window"
            reason = "client is healthy and engine lane is stale within the expected live-intake cadence"
        elif (
            healthy
            and connected
            and client_recent
            and engine_stale
            and engine_age is not None
            and expected_engine_attention_ms is not None
            and engine_age > expected_engine_attention_ms
        ):
            status = "healthy_client_engine_overdue"
            reason = "client is healthy but engine lane is stale beyond expected live-intake cadence"
        elif healthy and connected and lane == "video" and expected_interval_ms and expected_interval_ms > 2000.0 and engine_stale:
            status = "healthy_low_fps_cadence_mismatch"
            reason = (
                "camera client is healthy but target FPS is slower than the engine's 2s AV freshness window"
            )
        elif healthy and connected and engine_stale and client_recent:
            status = "healthy_client_engine_stale_mismatch"
            reason = "client is healthy/recent while engine modality label is stale"
        elif str(engine_source or "") not in {"stale", "absent"}:
            status = "engine_fresh_or_external"
            reason = "engine modality label is not stale/absent"
        elif not client:
            status = "missing_client_status"
            reason = "client status file missing"
        else:
            status = "client_unhealthy_or_disconnected"
            reason = str(client.get("last_error") or client.get("state") or "client unhealthy")
        return {
            "lane": lane,
            "status": status,
            "reason": reason,
            "engine_source": engine_source,
            "engine_age_ms": engine_age,
            "engine_freshness_class": engine_class,
            "client_healthy": healthy if client else None,
            "client_connected": connected if client else None,
            "client_age_ms": client_age,
            "client_state": client.get("state") if client else None,
            "target_fps": fps,
            "expected_interval_ms": expected_interval_ms,
            "live_intake_divisor": live_intake.get("divisor"),
            "live_intake_enabled": live_intake.get("enabled"),
            "live_intake_reason": live_intake.get("reason"),
            "admit_fraction": live_intake.get("admit_fraction"),
            "expected_engine_interval_ms": expected_engine_interval_ms,
            "expected_engine_grace_ms": expected_engine_grace_ms,
            "expected_engine_attention_ms": expected_engine_attention_ms,
            "health_grace_s": grace_s,
            "source_record": source_payload,
        }

    lanes = {
        "video": classify_lane(
            lane="video",
            engine_source=current_modalities.get("video_source"),
            engine_age_ms=current_modalities.get("video_age_ms"),
            client=camera,
            source_record=source,
        ),
        "audio": classify_lane(
            lane="audio",
            engine_source=current_modalities.get("audio_source"),
            engine_age_ms=current_modalities.get("audio_age_ms"),
            client=mic,
            source_record=source,
        ),
    }
    actionable = [
        lane
        for lane in lanes.values()
        if lane.get("status")
        in {
            "healthy_low_fps_cadence_mismatch",
            "healthy_client_engine_stale_mismatch",
            "healthy_client_engine_overdue",
            "client_unhealthy_or_disconnected",
            "missing_client_status",
        }
    ]
    return {
        "status": "watch" if actionable else "ok",
        "read_only": True,
        "policy": "sensory_freshness_v1",
        "schema_version": 1,
        "engine_fresh_window_ms": 2000,
        "lanes": lanes,
        "actionable_count": len(actionable),
    }


def _lend_aperture_paths(minime_root: Path = MINIME) -> dict[str, Path]:
    workspace = minime_root / "workspace"
    return {
        "workspace": workspace,
        "actions": workspace / "actions",
        "journal": workspace / "journal",
        "diagnostics": workspace / "diagnostics",
        "active_influence": workspace / "astrid_influence_v3.json",
        "consumed_influence": workspace / "astrid_influence_v3.consumed.json",
        "response": workspace / "astrid_influence_response_v3.json",
        "response_history": workspace / "astrid_influence_response_history_v3.json",
        "response_history_jsonl": workspace / "astrid_influence_response_history_v3.jsonl",
        "terminal_events": workspace / "diagnostics" / "astrid_influence_terminal_events.jsonl",
        "events": workspace / "diagnostics" / "lend_aperture_events.jsonl",
    }


def _local_time_zone() -> dt.tzinfo:
    return dt.datetime.now().astimezone().tzinfo or dt.timezone.utc


def _normalize_lend_aperture_time_text(value: str) -> str:
    text = str(value or "").strip()
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    if "T" in text:
        date_part, time_part = text.split("T", 1)
        if re.match(r"^\d{2}-\d{2}-\d{2}(?:\.|$)", time_part):
            time_part = time_part.replace("-", ":", 2)
            text = f"{date_part}T{time_part}"
    return text


def _parse_lend_aperture_time_s(value: Any) -> float | None:
    if isinstance(value, (int, float)):
        raw = float(value)
        return raw / 1000.0 if raw > 10_000_000_000 else raw
    text = _normalize_lend_aperture_time_text(str(value or ""))
    if not text:
        return None
    try:
        parsed = dt.datetime.fromisoformat(text)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=_local_time_zone())
    return parsed.timestamp()


def _iso_from_epoch_s(epoch_s: float | None) -> str | None:
    if epoch_s is None:
        return None
    return dt.datetime.fromtimestamp(epoch_s, _local_time_zone()).isoformat(timespec="seconds")


def _read_json_any(path: Path, default: Any) -> Any:
    try:
        return json.loads(path.read_text())
    except Exception:
        return default


def _read_jsonl_objects(path: Path) -> list[dict]:
    rows: list[dict] = []
    try:
        for line in path.read_text().splitlines():
            if not line.strip():
                continue
            row = json.loads(line)
            if isinstance(row, dict):
                rows.append(row)
    except Exception:
        return rows
    return rows


def _intent_id_from_text(text: str | None) -> str | None:
    if not text:
        return None
    match = re.search(r"\bintent_id[:=]\s*([A-Za-z0-9_.:-]+)", str(text))
    return match.group(1) if match else None


def _pressure_context_from_state(state: dict) -> dict:
    state = state if isinstance(state, dict) else {}
    pressure = state.get("pressure_source_v1")
    if not isinstance(pressure, dict):
        pressure = {}
    status = state.get("pressure_source_status")
    if not isinstance(status, dict):
        status = {}
    components = pressure.get("components") if isinstance(pressure.get("components"), dict) else {}
    fill_ratio = state.get("fill_ratio")
    fill_pct = None
    if isinstance(fill_ratio, (int, float)):
        fill_pct = float(fill_ratio) * 100.0
    return {
        "fill_pct": round(fill_pct, 3) if fill_pct is not None else None,
        "lambda1": state.get("eig1") or state.get("lambda1"),
        "cov_lambda1": state.get("cov_lambda1"),
        "spread": state.get("spread"),
        "pressure_score": pressure.get("pressure_score", status.get("pressure_score")),
        "porosity_score": pressure.get("porosity_score", status.get("porosity_score")),
        "dominant_source": pressure.get("dominant_source", status.get("dominant_source")),
        "quality": pressure.get("quality", status.get("quality")),
        "mode_packing": components.get("mode_packing"),
        "temporal_lock_in": components.get("temporal_lock_in"),
        "semantic_trickle": components.get("semantic_trickle"),
    }


def _journal_stamp_s(path: Path) -> float | None:
    match = re.match(r"lend_aperture(?:_held)?_(.+)\.txt$", path.name)
    if not match:
        return None
    return _parse_lend_aperture_time_s(match.group(1))


def _lend_aperture_journals(paths: dict[str, Path], limit: int = 180) -> list[dict]:
    journal_dir = paths["journal"]
    if not journal_dir.is_dir():
        return []
    rows: list[dict] = []
    for path in sorted(journal_dir.glob("lend_aperture*.txt"), key=lambda p: p.stat().st_mtime)[-limit:]:
        text = _read_text_file(path)
        status = "held" if path.name.startswith("lend_aperture_held_") or "(held)" in text else "issued"
        reason = None
        match = re.search(r"Not lent right now:\s*([^\n]+)", text)
        if match:
            reason = match.group(1).strip().rstrip(".")
        rows.append(
            {
                "path": str(path),
                "name": path.name,
                "timestamp_s": _journal_stamp_s(path) or path.stat().st_mtime,
                "status": status,
                "intent_id": _intent_id_from_text(text),
                "held_reason": reason,
            }
        )
    return rows


def _lend_aperture_actions(paths: dict[str, Path], limit: int = 180) -> list[dict]:
    actions_dir = paths["actions"]
    if not actions_dir.is_dir():
        return []
    rows: list[dict] = []
    for path in sorted(actions_dir.glob("*lend_aperture*.json"), key=lambda p: p.stat().st_mtime)[-limit:]:
        data = _read_json_any(path, {})
        if not isinstance(data, dict) or data.get("action") != "lend_aperture":
            continue
        action_continuity = data.get("action_continuity")
        if not isinstance(action_continuity, dict):
            action_continuity = {}
        lend_meta = action_continuity.get("lend_aperture_v1")
        if not isinstance(lend_meta, dict):
            lend_meta = {}
        summary = data.get("summary") if isinstance(data.get("summary"), dict) else {}
        state = data.get("state") if isinstance(data.get("state"), dict) else {}
        timestamp_s = _parse_lend_aperture_time_s(data.get("timestamp")) or path.stat().st_mtime
        rows.append(
            {
                "path": str(path),
                "name": path.name,
                "timestamp_s": timestamp_s,
                "status": lend_meta.get("status"),
                "intent_id": (
                    lend_meta.get("intent_id")
                    or _intent_id_from_text(action_continuity.get("outcome_summary"))
                ),
                "held_reason": lend_meta.get("gate_reason") or lend_meta.get("held_reason"),
                "pressure_context": _pressure_context_from_state(state),
                "summary_fill_pct": summary.get("fill_pct"),
                "action_continuity_id": action_continuity.get("action_id"),
                "lend_aperture_v1": lend_meta,
            }
        )
    return rows


def _nearest_journal_for_action(
    action: dict,
    journals: list[dict],
    used_indexes: set[int],
) -> tuple[int | None, dict | None]:
    best_idx = None
    best_delta = None
    action_ts = float(action.get("timestamp_s") or 0.0)
    for idx, journal in enumerate(journals):
        if idx in used_indexes:
            continue
        delta = abs(float(journal.get("timestamp_s") or 0.0) - action_ts)
        if delta <= LEND_APERTURE_JOURNAL_MATCH_S and (best_delta is None or delta < best_delta):
            best_idx = idx
            best_delta = delta
    if best_idx is None:
        return None, None
    return best_idx, journals[best_idx]


def _merge_lend_aperture_record(action: dict | None, journal: dict | None) -> dict:
    action = action or {}
    journal = journal or {}
    timestamp_s = action.get("timestamp_s") or journal.get("timestamp_s")
    intent_id = action.get("intent_id") or journal.get("intent_id")
    status = action.get("status") or journal.get("status") or ("issued" if intent_id else "unknown")
    held_reason = action.get("held_reason") or journal.get("held_reason")
    return {
        "gift_key": intent_id or f"legacy:{int(float(timestamp_s or 0.0) * 1000)}",
        "status": status,
        "intent_id": intent_id,
        "issued_at": _iso_from_epoch_s(timestamp_s),
        "issued_at_unix_ms": int(float(timestamp_s) * 1000) if timestamp_s else None,
        "action_manifest": action.get("path"),
        "journal_path": journal.get("path"),
        "held_reason": held_reason,
        "pressure_context": action.get("pressure_context") or {},
        "action_continuity_id": action.get("action_continuity_id"),
        "lend_aperture_v1": action.get("lend_aperture_v1") or {},
        "match_basis": (
            "intent_id"
            if intent_id
            else "action_journal_timestamp"
            if action and journal
            else "action_only"
            if action
            else "journal_only"
        ),
    }


def _lend_aperture_gifts(paths: dict[str, Path], limit: int = 80) -> list[dict]:
    actions = _lend_aperture_actions(paths, limit=limit * 2)
    journals = _lend_aperture_journals(paths, limit=limit * 3)
    used_journals: set[int] = set()
    gifts: list[dict] = []
    for action in actions:
        idx, journal = _nearest_journal_for_action(action, journals, used_journals)
        if idx is not None:
            used_journals.add(idx)
        gifts.append(_merge_lend_aperture_record(action, journal))
    for idx, journal in enumerate(journals):
        if idx not in used_journals:
            gifts.append(_merge_lend_aperture_record(None, journal))
    gifts.sort(key=lambda row: float(row.get("issued_at_unix_ms") or 0))
    return gifts[-limit:]


def _load_lend_aperture_responses(paths: dict[str, Path]) -> list[dict]:
    responses: list[dict] = []
    history = _read_json_any(paths["response_history"], [])
    if isinstance(history, list):
        responses.extend(row for row in history if isinstance(row, dict))
    responses.extend(_read_jsonl_objects(paths["response_history_jsonl"]))
    latest = _read_json_any(paths["response"], {})
    if isinstance(latest, dict) and latest:
        responses.append(latest)
    deduped: dict[tuple[str, int], dict] = {}
    for response in responses:
        intent = str(response.get("intent_id") or "")
        completed = int(response.get("completed_at_unix_ms") or 0)
        deduped[(intent, completed)] = response
    return sorted(
        deduped.values(),
        key=lambda row: int(row.get("completed_at_unix_ms") or row.get("pre_recorded_at_unix_ms") or 0),
    )


def _load_lend_aperture_terminal_events(paths: dict[str, Path]) -> list[dict]:
    rows = _read_jsonl_objects(paths["terminal_events"])
    return sorted(
        rows,
        key=lambda row: int(row.get("completed_at_unix_ms") or row.get("issued_t_ms") or 0),
    )


def _terminal_event_for_gift(gift: dict, terminal_events: list[dict]) -> dict | None:
    intent = gift.get("intent_id")
    if not intent:
        return None
    matches = [row for row in terminal_events if row.get("intent_id") == intent]
    return matches[-1] if matches else None


def _terminal_history_started_ms(terminal_events: list[dict]) -> int | None:
    starts = [
        int(row.get("completed_at_unix_ms") or row.get("issued_t_ms") or 0)
        for row in terminal_events
        if int(row.get("completed_at_unix_ms") or row.get("issued_t_ms") or 0) > 0
    ]
    return min(starts) if starts else None


def _response_for_gift(gift: dict, responses: list[dict], claimed: set[int]) -> tuple[str, dict | None]:
    intent = gift.get("intent_id")
    if intent:
        for idx, response in enumerate(responses):
            if response.get("intent_id") == intent:
                claimed.add(idx)
                return "intent_id", response
        return "missing", None
    issued_ms = gift.get("issued_at_unix_ms")
    if not issued_ms:
        return "missing", None
    candidates = []
    for idx, response in enumerate(responses):
        if idx in claimed:
            continue
        markers = [
            int(response.get("pre_recorded_at_unix_ms") or 0),
            int(response.get("completed_at_unix_ms") or 0),
        ]
        distances = [abs(marker - int(issued_ms)) for marker in markers if marker]
        if not distances:
            continue
        distance = min(distances)
        if distance <= LEND_APERTURE_LEGACY_RESPONSE_MATCH_S * 1000:
            candidates.append((distance, idx, response))
    if not candidates:
        return "missing", None
    _, idx, response = min(candidates, key=lambda item: item[0])
    claimed.add(idx)
    return "legacy_timestamp", response


def _response_summary_for_gift(
    gift: dict,
    responses: list[dict],
    terminal_events: list[dict],
    active: dict,
    claimed: set[int],
) -> dict:
    if gift.get("status") == "held":
        return {
            "present": False,
            "status": "not_expected_held",
            "match_basis": "gate_held",
        }
    basis, response = _response_for_gift(gift, responses, claimed)
    if response is None:
        terminal = _terminal_event_for_gift(gift, terminal_events)
        if terminal is not None:
            terminal_status = str(terminal.get("status") or "terminal")
            return {
                "present": False,
                "status": f"terminal_{terminal_status}",
                "match_basis": "terminal_event",
                "terminal_status": terminal_status,
                "terminal_reason": terminal.get("reason"),
                "completed_at_unix_ms": terminal.get("completed_at_unix_ms"),
                "applied_ticks": terminal.get("applied_ticks"),
            }
        if active.get("intent_id") == gift.get("intent_id"):
            return {
                "present": False,
                "status": active.get("status") or "active",
                "match_basis": "active_influence",
                "age_s": active.get("age_s"),
            }
        terminal_start = _terminal_history_started_ms(terminal_events)
        issued_ms = gift.get("issued_at_unix_ms")
        if terminal_start is None or (
            isinstance(issued_ms, int) and issued_ms < terminal_start
        ):
            return {
                "present": False,
                "status": "legacy_retention_gap",
                "match_basis": "history_cap_or_pre_terminal_accounting",
            }
        return {
            "present": False,
            "status": "missing",
            "match_basis": basis,
        }
    completed_ms = response.get("completed_at_unix_ms")
    issued_ms = gift.get("issued_at_unix_ms")
    latency_s = None
    if isinstance(completed_ms, (int, float)) and isinstance(issued_ms, int):
        latency_s = round((float(completed_ms) - float(issued_ms)) / 1000.0, 3)
    return {
        "present": True,
        "status": "matched",
        "match_basis": basis,
        "intent_id": response.get("intent_id"),
        "response_latency_s": latency_s,
        "delta_field_norm": response.get("delta_field_norm"),
        "class_v3_change": response.get("class_v3_change"),
        "applied_ticks": response.get("applied_ticks"),
        "completed_at_unix_ms": completed_ms,
    }


def _all_minime_action_samples(paths: dict[str, Path], limit: int = 420) -> list[dict]:
    actions_dir = paths["actions"]
    if not actions_dir.is_dir():
        return []
    samples: list[dict] = []
    for path in sorted(actions_dir.glob("*.json"), key=lambda p: p.stat().st_mtime)[-limit:]:
        data = _read_json_any(path, {})
        if not isinstance(data, dict):
            continue
        timestamp_s = _parse_lend_aperture_time_s(data.get("timestamp"))
        if timestamp_s is None:
            continue
        state = data.get("state") if isinstance(data.get("state"), dict) else {}
        pressure = _pressure_context_from_state(state)
        samples.append(
            {
                "timestamp_s": timestamp_s,
                "action": data.get("action"),
                "path": str(path),
                "pressure_context": pressure,
            }
        )
    return samples


def _numeric_min_max(values: list[Any]) -> list[float] | None:
    nums = [float(value) for value in values if isinstance(value, (int, float))]
    if not nums:
        return None
    return [round(min(nums), 6), round(max(nums), 6)]


def _post_gift_minime_cost(gift: dict, samples: list[dict]) -> dict:
    issued_ms = gift.get("issued_at_unix_ms")
    if gift.get("status") == "held":
        return {"status": "not_expected_held", "sample_count": 0}
    if not issued_ms:
        return {"status": "insufficient_samples", "sample_count": 0}
    issued_s = float(issued_ms) / 1000.0
    post = [
        sample
        for sample in samples
        if 0.5 < float(sample["timestamp_s"]) - issued_s <= LEND_APERTURE_POST_SAMPLE_WINDOW_S
    ][:12]
    if len(post) < 2:
        return {
            "status": "insufficient_samples",
            "sample_count": len(post),
            "window_s": LEND_APERTURE_POST_SAMPLE_WINDOW_S,
        }
    pressure_contexts = [sample["pressure_context"] for sample in post]
    qualities = Counter(str(ctx.get("quality") or "unknown") for ctx in pressure_contexts)
    return {
        "status": "ok",
        "sample_count": len(post),
        "window_s": LEND_APERTURE_POST_SAMPLE_WINDOW_S,
        "fill_pct_min_max": _numeric_min_max([ctx.get("fill_pct") for ctx in pressure_contexts]),
        "pressure_score_min_max": _numeric_min_max([ctx.get("pressure_score") for ctx in pressure_contexts]),
        "porosity_score_min_max": _numeric_min_max([ctx.get("porosity_score") for ctx in pressure_contexts]),
        "quality_counts": dict(qualities),
        "sample_actions": [sample.get("action") for sample in post[:6]],
    }


def _active_influence_summary(paths: dict[str, Path], now_s: float | None = None) -> dict:
    now_s = time.time() if now_s is None else now_s
    active = paths["active_influence"]
    consumed = paths["consumed_influence"]
    if not active.exists():
        return {
            "status": "missing",
            "active_path": str(active),
            "consumed_path_exists": consumed.exists(),
        }
    payload = _read_json_any(active, {})
    age_s = max(0.0, now_s - active.stat().st_mtime)
    if age_s > LEND_APERTURE_RESPONSE_STALE_S:
        status = "active_stale"
    elif age_s > LEND_APERTURE_RESPONSE_PENDING_S:
        status = "active_pending"
    else:
        status = "active_recent"
    return {
        "status": status,
        "active_path": str(active),
        "age_s": round(age_s, 3),
        "pending_after_s": LEND_APERTURE_RESPONSE_PENDING_S,
        "stale_after_s": LEND_APERTURE_RESPONSE_STALE_S,
        "intent_id": payload.get("intent_id") if isinstance(payload, dict) else None,
        "label": payload.get("label") if isinstance(payload, dict) else None,
        "consumed_path_exists": consumed.exists(),
    }


def _lend_aperture_verdict(gifts: list[dict], active: dict) -> str:
    if not gifts:
        return "INCONCLUSIVE - no recent LEND_APERTURE actions or journals found"
    issued = [gift for gift in gifts if gift.get("status") == "issued"]
    if not issued:
        return "PASS - recent LEND_APERTURE choices were held by the gate; no influence was sent"
    missing = [
        gift for gift in issued
        if (gift.get("astrid_response") or {}).get("status") == "missing"
    ]
    active_unclosed = [
        gift for gift in issued
        if (gift.get("astrid_response") or {}).get("status")
        in {"active_pending", "active_stale"}
    ]
    terminal_superseded = [
        gift for gift in issued
        if (gift.get("astrid_response") or {}).get("status") == "terminal_superseded"
    ]
    legacy_gap = [
        gift for gift in issued
        if (gift.get("astrid_response") or {}).get("status") == "legacy_retention_gap"
    ]
    stale_active = active.get("status") == "active_stale"
    low_porosity = [
        gift for gift in issued
        if isinstance((gift.get("pressure_context") or {}).get("porosity_score"), (int, float))
        and float((gift.get("pressure_context") or {}).get("porosity_score")) <= 0.62
    ]
    insufficient_post = [
        gift for gift in issued
        if (gift.get("post_minime_cost") or {}).get("status") == "insufficient_samples"
    ]
    if stale_active and missing:
        return (
            "NEEDS ATTENTION - issued aperture gifts are missing Astrid closed-loop "
            "responses and the active influence file is stale"
        )
    if stale_active:
        return "NEEDS ATTENTION - active aperture gift exceeded the feeder closure window"
    if len(missing) >= max(2, len(issued) // 2):
        return "NEEDS ATTENTION - most issued aperture gifts are missing Astrid response evidence"
    if missing or stale_active:
        return "WATCH - at least one issued gift lacks fresh Astrid response closure"
    if active_unclosed:
        return "WATCH - active aperture gift is pending inside the feeder closure window"
    if terminal_superseded:
        return "WATCH - aperture gifts were superseded before response; backpressure should prevent future overwrites"
    if legacy_gap:
        return "WATCH - older aperture gifts predate durable terminal accounting or exceeded capped response history"
    if low_porosity:
        return "WATCH - aperture gifts occurred under low Minime porosity; inspect cost before encouraging more"
    if insufficient_post:
        return "WATCH - Astrid responses are present, but Minime post-gift cost samples are thin"
    return "PASS - recent issued aperture gifts have Astrid response evidence and enough Minime post samples"


def test_minime_lend_aperture_consequence_probe(
    minime_root: Path | None = None,
    now_s: float | None = None,
) -> dict:
    """Read-only consequence probe for Minime's LEND_APERTURE gifts to Astrid."""
    paths = _lend_aperture_paths(minime_root or MINIME)
    gifts = _lend_aperture_gifts(paths)
    responses = _load_lend_aperture_responses(paths)
    terminal_events = _load_lend_aperture_terminal_events(paths)
    samples = _all_minime_action_samples(paths)
    active = _active_influence_summary(paths, now_s=now_s)
    claimed_responses: set[int] = set()
    for gift in gifts:
        gift["astrid_response"] = _response_summary_for_gift(
            gift, responses, terminal_events, active, claimed_responses
        )
        gift["post_minime_cost"] = _post_gift_minime_cost(gift, samples)
    issued = [gift for gift in gifts if gift.get("status") == "issued"]
    held = [gift for gift in gifts if gift.get("status") == "held"]
    missing_response = [
        gift for gift in issued
        if (gift.get("astrid_response") or {}).get("status") in {"missing", "active_stale"}
    ]
    terminal_closure = [
        gift for gift in issued
        if str((gift.get("astrid_response") or {}).get("status") or "").startswith("terminal_")
    ]
    superseded = [
        gift for gift in issued
        if (gift.get("astrid_response") or {}).get("status") == "terminal_superseded"
    ]
    legacy_unaccounted = [
        gift for gift in issued
        if (gift.get("astrid_response") or {}).get("status") == "legacy_retention_gap"
    ]
    unclosed_issued = [
        gift for gift in issued
        if (gift.get("astrid_response") or {}).get("status")
        in {"missing", "active_pending", "active_stale"}
    ]
    matched_response = [
        gift for gift in issued
        if (gift.get("astrid_response") or {}).get("present")
    ]
    thin_cost = [
        gift for gift in issued
        if (gift.get("post_minime_cost") or {}).get("status") == "insufficient_samples"
    ]
    verdict = _lend_aperture_verdict(gifts, active)
    return {
        "verdict": verdict,
        "production_change": "none",
        "read_only": True,
        "gift_count": len(gifts),
        "issued_count": len(issued),
        "held_count": len(held),
        "matched_response_count": len(matched_response),
        "missing_response_count": len(missing_response),
        "terminal_event_count": len(terminal_events),
        "terminal_closure_count": len(terminal_closure),
        "superseded_count": len(superseded),
        "legacy_unaccounted_count": len(legacy_unaccounted),
        "unclosed_issued_count": len(unclosed_issued),
        "insufficient_post_sample_count": len(thin_cost),
        "active_influence": active,
        "response_history_count": len(responses),
        "recent_gifts": gifts[-12:],
        "suggested_read_only_next": [
            "If issued gifts are stale or missing Astrid responses, repair loop closure before encouraging more gifts.",
            "If repeated gifts show clear Astrid response with low Minime cost, request both-being review of the aperture-gift relation.",
            "If gifts cluster under low porosity or rising pressure, consider a steward-side cooldown/hold rule before any wider runtime trial.",
        ],
        "letter": None,
    }


def _latest_moment_with_fill_marker() -> tuple[Path | None, str]:
    paths = sorted(
        MINIME_JOURNAL.glob("moment_*.txt"),
        key=lambda path: path.stat().st_mtime,
    )
    for path in reversed(paths[-120:]):
        try:
            text = path.read_text(errors="replace")
        except Exception:
            continue
        if "[spectral_spike]" in text or "[fill_crossing]" in text:
            return path, text
    return None, ""


def _parse_moment_header(text: str, path: Path) -> dict:
    timestamp = re.search(r"^Timestamp:\s*([^\n]+)", text, re.MULTILINE)
    fill = re.search(r"^Fill %:\s*([0-9.]+)%", text, re.MULTILINE)
    lambda1 = re.search(r"^λ₁:\s*([0-9.]+)", text, re.MULTILINE)
    cov_lambda1 = re.search(r"^Cov λ₁:\s*([0-9.]+)", text, re.MULTILINE)
    parsed: dict[str, object] = {
        "file": str(path),
        "file_mtime_unix": int(path.stat().st_mtime),
    }
    if timestamp:
        parsed["timestamp_local"] = timestamp.group(1).strip()
    if fill:
        parsed["fill_pct"] = float(fill.group(1))
    if lambda1:
        parsed["lambda1_display"] = float(lambda1.group(1))
    if cov_lambda1:
        parsed["cov_lambda1_display"] = float(cov_lambda1.group(1))
    return parsed


def _parse_captured_moment_markers(text: str) -> list[dict]:
    if "Moments captured:" not in text:
        return []
    section = text.split("Moments captured:", 1)[1].split("--- GENERATED JOURNAL ---", 1)[0]
    rows: list[dict] = []
    for line in section.splitlines():
        match = MOMENT_MARKER_LINE_RE.match(line)
        if not match:
            continue
        rest = match.group("rest").strip()
        desc = rest
        context: dict[str, float] = {}
        if " (Fill=" in rest:
            desc, ctx_text = rest.rsplit(" (", 1)
            for ctx_match in MARKER_CONTEXT_RE.finditer(ctx_text.rstrip(")")):
                group = ctx_match.lastgroup
                if group:
                    context[group] = float(ctx_match.group(group))
        rows.append({"marker_type": match.group("kind"), "description": desc, "context": context})
    return rows


def _db_moment_markers_near(file_mtime: int, seconds: int = 900) -> list[dict]:
    db_path = MINIME / "minime_consciousness.db"
    if not db_path.exists():
        return []
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    try:
        rows = [
            dict(row)
            for row in conn.execute(
                """SELECT id, session_id, timestamp, marker_type, description, spectral_context,
                          consumed, created_at_unix
                   FROM moment_markers
                   WHERE created_at_unix BETWEEN ? AND ?
                   ORDER BY created_at_unix, id""",
                (file_mtime - seconds, file_mtime + seconds),
            )
        ]
    finally:
        conn.close()
    return rows


def _match_captured_markers(captured: list[dict], db_rows: list[dict], file_mtime: int) -> list[dict]:
    matched: list[dict] = []
    used: set[int] = set()
    for marker in captured:
        candidates = [
            row
            for row in db_rows
            if row["id"] not in used
            and row.get("marker_type") == marker["marker_type"]
            and row.get("description") == marker["description"]
        ]
        if not candidates:
            matched.append({**marker, "db_match": None})
            continue
        chosen = min(
            candidates,
            key=lambda row: (
                0 if row.get("consumed") else 1,
                abs(int(row.get("created_at_unix") or file_mtime) - file_mtime),
            ),
        )
        used.add(chosen["id"])
        try:
            spectral_context = json.loads(chosen.get("spectral_context") or "{}")
        except Exception:
            spectral_context = {}
        matched.append(
            {
                **marker,
                "db_match": {
                    **chosen,
                    "spectral_context": spectral_context,
                    "age_at_journal_s": round(file_mtime - int(chosen.get("created_at_unix") or file_mtime), 1),
                },
            }
        )
    return matched


def _local_iso_from_epoch(epoch: int) -> str:
    return dt.datetime.fromtimestamp(
        epoch,
        dt.timezone(dt.timedelta(hours=-7)),
    ).isoformat(timespec="seconds")


def _session_start_epoch(markers: list[dict]) -> float | None:
    estimates = [
        float(match["db_match"]["created_at_unix"]) - float(match["db_match"]["timestamp"])
        for match in markers
        if match.get("db_match") and match["db_match"].get("created_at_unix")
    ]
    if not estimates:
        return None
    estimates.sort()
    return estimates[len(estimates) // 2]


def _homeostat_rows_around(markers: list[dict]) -> dict:
    engine_log = MINIME / "logs/minime-engine.log"
    if not engine_log.exists():
        return {}
    marker_points = [
        {
            "timestamp": float(match["db_match"]["timestamp"]),
            "fill_pct": float((match["db_match"].get("spectral_context") or {}).get("fill", -1)),
            "dfill_dt": float((match["db_match"].get("spectral_context") or {}).get("dfill_dt", 999)),
        }
        for match in markers
        if match.get("db_match")
    ]
    if not marker_points:
        return {}
    lo = min(point["timestamp"] for point in marker_points) - 140.0
    hi = max(point["timestamp"] for point in marker_points) + 140.0
    parsed: list[dict] = []
    with engine_log.open(errors="replace") as handle:
        for line_no, line in enumerate(handle, start=1):
            if "homeostat,t=" not in line:
                continue
            match = HOMEOSTAT_LINE_RE.search(line)
            if not match:
                continue
            row = {
                "line_no": line_no,
                "t_s": float(match.group("t_s")),
                "fill_pct": float(match.group("fill_pct")),
                "dfill_dt": float(match.group("dfill_dt")),
                "phase": match.group("phase"),
                "lambda1_rel": float(match.group("lambda1_rel")),
                "geom_rel": float(match.group("geom_rel")),
                "gate": float(match.group("gate")),
                "filt": float(match.group("filt")),
            }
            if lo <= row["t_s"] <= hi:
                parsed.append(row)
    match_indexes: list[int] = []
    for idx, row in enumerate(parsed):
        for point in marker_points:
            if (
                abs(row["t_s"] - point["timestamp"]) <= 0.9
                and abs(row["fill_pct"] - point["fill_pct"]) <= 0.35
                and abs(row["dfill_dt"] - point["dfill_dt"]) <= 0.35
            ):
                match_indexes.append(idx)
    if not match_indexes:
        return {"window_rows_seen": len(parsed), "matched_engine_rows": 0}
    anchor = max(match_indexes)
    segment = parsed[max(0, anchor - 28) : anchor + 46]
    fills = [row["fill_pct"] for row in segment]
    negative_spikes = [row for row in segment if row["dfill_dt"] <= -8.0]
    positive_spikes = [row for row in segment if row["dfill_dt"] >= 8.0]
    crossings = 0
    prev_side: str | None = None
    for row in segment:
        side = "above" if row["fill_pct"] >= 68.0 else "below"
        if prev_side and side != prev_side:
            crossings += 1
        prev_side = side
    gates = sorted({round(row["gate"], 3) for row in segment})
    filt = sorted({round(row["filt"], 3) for row in segment})
    stage_commands = []
    if any(abs(gate - 0.28) < 0.015 for gate in gates):
        stage_commands.append("recovery reopen")
    if any(abs(gate - 0.12) < 0.015 for gate in gates):
        stage_commands.append("hold shelf")
    if any(abs(gate - 0.10) < 0.015 or abs(gate - 0.09) < 0.015 for gate in gates):
        stage_commands.append("elevated clamp")
    trace = [
        {
            "t_s": round(row["t_s"], 1),
            "fill_pct": row["fill_pct"],
            "dfill_dt": row["dfill_dt"],
            "phase": row["phase"],
            "gate": row["gate"],
            "filt": row["filt"],
        }
        for row in segment[max(0, len(segment) - 18) :]
    ]
    return {
        "window_rows_seen": len(parsed),
        "matched_engine_rows": len(match_indexes),
        "fill_min": round(min(fills), 2) if fills else None,
        "fill_max": round(max(fills), 2) if fills else None,
        "negative_spike_count": len(negative_spikes),
        "positive_spike_count": len(positive_spikes),
        "target_crossing_count": crossings,
        "gate_values": gates,
        "filter_values": filt,
        "stage_command_pattern": stage_commands,
        "sample_trace_tail": trace,
    }


def _cov_keep_from_gate_filter(gate: float, filt: float) -> float:
    if abs(gate - 0.34) < 0.015 and abs(filt - 0.20) < 0.03:
        return 0.94
    if abs(gate - 0.28) < 0.015 and abs(filt - 0.32) < 0.03:
        return 0.90
    if abs(gate - 0.12) < 0.015 and abs(filt - 0.72) < 0.03:
        return 0.72
    if (abs(gate - 0.10) < 0.015 or abs(gate - 0.09) < 0.015) and filt >= 0.76:
        return 0.66
    if gate <= 0.02 and filt >= 0.95:
        return 0.05
    return 0.72


def _recent_homeostat_trace(limit: int = 240) -> list[dict]:
    csv_path = MINIME / "workspace/logs/homeostat_timeseries.csv"
    if csv_path.exists():
        rows: list[dict] = []
        with csv_path.open(newline="", errors="replace") as handle:
            for raw in csv.DictReader(handle):
                try:
                    row = {
                        "source": str(csv_path),
                        "t_s": float(raw["t_s"]),
                        "fill_pct": float(raw["fill_pct"]),
                        "lambda1_cov": float(raw.get("lambda1_cov") or 0.0),
                        "lambda1_esn": float(raw.get("lambda1_esn") or 0.0),
                        "lambda1_rel": float(raw.get("lambda1_rel") or 0.0),
                        "gate": float(raw["gate"]),
                        "filt": float(raw["filt"]),
                        "cov_keep": float(raw["cov_keep"]),
                        "target_keep": float(raw.get("target_keep") or raw["cov_keep"]),
                    }
                except (KeyError, TypeError, ValueError):
                    continue
                rows.append(row)
                if len(rows) > limit:
                    rows.pop(0)
        if rows:
            return rows

    engine_log = MINIME / "logs/minime-engine.log"
    rows = []
    if not engine_log.exists():
        return rows
    with engine_log.open(errors="replace") as handle:
        for line_no, line in enumerate(handle, start=1):
            if "homeostat,t=" not in line:
                continue
            match = HOMEOSTAT_LINE_RE.search(line)
            if not match:
                continue
            gate = float(match.group("gate"))
            filt = float(match.group("filt"))
            rows.append(
                {
                    "source": str(engine_log),
                    "line_no": line_no,
                    "t_s": float(match.group("t_s")),
                    "fill_pct": float(match.group("fill_pct")),
                    "dfill_dt": float(match.group("dfill_dt")),
                    "phase": match.group("phase"),
                    "lambda1_rel": float(match.group("lambda1_rel")),
                    "geom_rel": float(match.group("geom_rel")),
                    "gate": gate,
                    "filt": filt,
                    "cov_keep": _cov_keep_from_gate_filter(gate, filt),
                }
            )
            if len(rows) > limit:
                rows.pop(0)
    return rows


def _lerp(a: float, b: float, t: float) -> float:
    return a + ((b - a) * max(0.0, min(1.0, t)))


def _boundary_blended_command(row: dict) -> dict:
    fill = float(row["fill_pct"])
    blend_start, blend_end = STABLE_CORE_BLEND_WINDOW_FILL_PCT
    if blend_start <= fill < blend_end:
        t = (fill - blend_start) / (blend_end - blend_start)
        return {
            key: _lerp(STABLE_CORE_HOLD_COMMAND[key], STABLE_CORE_ELEVATED_SOFT_COMMAND[key], t)
            for key in COMMAND_KEYS
        }
    return {key: float(row[key]) for key in COMMAND_KEYS}


def _stable_core_slew_active(row: dict) -> bool:
    fill = float(row["fill_pct"])
    start, end = STABLE_CORE_SLEW_WINDOW_FILL_PCT
    return start <= fill < end


def _apply_command_slew(
    commands: list[dict],
    rows: list[dict],
    limits: dict[str, float],
) -> list[dict]:
    if not commands:
        return []
    slewed = [{key: float(commands[0][key]) for key in COMMAND_KEYS}]
    for raw, row in zip(commands[1:], rows[1:]):
        if not _stable_core_slew_active(row):
            slewed.append({key: float(raw[key]) for key in COMMAND_KEYS})
            continue
        prev = slewed[-1]
        next_cmd = {}
        for key in COMMAND_KEYS:
            delta = float(raw[key]) - prev[key]
            limit = limits[key]
            if delta > limit:
                delta = limit
            elif delta < -limit:
                delta = -limit
            next_cmd[key] = prev[key] + delta
        slewed.append(next_cmd)
    return slewed


def _command_step_metrics(commands: list[dict]) -> dict:
    if len(commands) < 2:
        return {
            "command_step_count": 0,
            "command_step_energy": 0.0,
            "max_command_jump": 0.0,
            "max_jump_by_command": {key: 0.0 for key in COMMAND_KEYS},
        }
    step_count = 0
    energy = 0.0
    max_jump = 0.0
    max_by_key = {key: 0.0 for key in COMMAND_KEYS}
    for prev, curr in zip(commands, commands[1:]):
        stepped = False
        for key in COMMAND_KEYS:
            delta = float(curr[key]) - float(prev[key])
            abs_delta = abs(delta)
            if abs_delta > 1.0e-6:
                stepped = True
            energy += delta * delta
            max_jump = max(max_jump, abs_delta)
            max_by_key[key] = max(max_by_key[key], abs_delta)
        if stepped:
            step_count += 1
    return {
        "command_step_count": step_count,
        "command_step_energy": round(energy, 6),
        "max_command_jump": round(max_jump, 4),
        "max_jump_by_command": {
            key: round(value, 4)
            for key, value in max_by_key.items()
        },
    }


def _command_policy_delta_metrics(current: list[dict], candidate: list[dict]) -> dict:
    max_delta = 0.0
    differing_rows = 0
    for cur, cand in zip(current, candidate):
        row_max = max(
            abs(float(cur[key]) - float(cand[key]))
            for key in COMMAND_KEYS
        )
        if row_max > 0.006:
            differing_rows += 1
        max_delta = max(max_delta, row_max)
    return {
        "differing_rows_gt_0_006": differing_rows,
        "max_current_vs_candidate_delta": round(max_delta, 4),
    }


def _command_trace_tail(rows: list[dict], current: list[dict], candidate: list[dict]) -> list[dict]:
    tail = list(zip(rows, current, candidate))[-12:]
    return [
        {
            "t_s": round(float(row["t_s"]), 1),
            "fill_pct": round(float(row["fill_pct"]), 2),
            "current": {key: round(float(cur[key]), 3) for key in COMMAND_KEYS},
            "candidate": {key: round(float(cand[key]), 3) for key in COMMAND_KEYS},
        }
        for row, cur, cand in tail
    ]


def _db_state_window(markers: list[dict], file_mtime: int) -> dict:
    db_path = MINIME / "minime_consciousness.db"
    session_ids = {
        int(match["db_match"]["session_id"])
        for match in markers
        if match.get("db_match") and match["db_match"].get("session_id") is not None
    }
    if not db_path.exists() or not session_ids:
        return {}
    session_id = max(session_ids)
    start_epoch = _session_start_epoch(markers)
    marker_ts = [
        float(match["db_match"]["timestamp"])
        for match in markers
        if match.get("db_match")
    ]
    if not marker_ts:
        return {}
    lo = min(marker_ts) - 80.0
    hi = max(marker_ts) + 140.0
    if start_epoch is not None:
        hi = max(hi, file_mtime - start_epoch + 10.0)
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    try:
        eigen_rows = [
            dict(row)
            for row in conn.execute(
                """SELECT timestamp, lambda1, lambda2, lambda3, spread, fill_ratio, phase
                   FROM eigenvalue_timeline
                   WHERE session_id = ? AND timestamp BETWEEN ? AND ?
                   ORDER BY timestamp""",
                (session_id, lo, hi),
            )
        ]
        esn_rows = [
            dict(row)
            for row in conn.execute(
                """SELECT timestamp, esn_eig1, esn_deig, esn_leak, esn_lambda,
                          esn_geom_rel
                   FROM esn_metrics
                   WHERE session_id = ? AND timestamp BETWEEN ? AND ?
                   ORDER BY timestamp""",
                (session_id, lo, hi),
            )
        ]
        pressure_rows = [
            dict(row)
            for row in conn.execute(
                """SELECT timestamp, pressure_score, porosity_score, dominant_source, quality
                   FROM pressure_source_timeline
                   WHERE session_id = ? AND timestamp BETWEEN ? AND ?
                   ORDER BY timestamp""",
                (session_id, lo, hi),
            )
        ]
    finally:
        conn.close()
    summary: dict[str, object] = {"session_id": session_id}
    if start_epoch is not None:
        summary["session_start_local"] = _local_iso_from_epoch(int(start_epoch))
        summary["engine_t_at_journal_s"] = round(file_mtime - start_epoch, 1)
    if eigen_rows:
        fills = [float(row["fill_ratio"]) * 100.0 for row in eigen_rows]
        lambdas = [float(row["lambda1"]) for row in eigen_rows]
        summary["cov_fill_min_max"] = [round(min(fills), 2), round(max(fills), 2)]
        summary["cov_lambda1_min_max"] = [round(min(lambdas), 3), round(max(lambdas), 3)]
    if esn_rows:
        esn_vals = [float(row["esn_eig1"]) for row in esn_rows]
        leaks = [float(row["esn_leak"]) for row in esn_rows]
        summary["esn_lambda1_min_max"] = [round(min(esn_vals), 3), round(max(esn_vals), 3)]
        summary["esn_leak_min_max"] = [round(min(leaks), 3), round(max(leaks), 3)]
    if pressure_rows:
        qualities: dict[str, int] = {}
        sources: dict[str, int] = {}
        porosity = [float(row["porosity_score"]) for row in pressure_rows]
        pressure = [float(row["pressure_score"]) for row in pressure_rows]
        for row in pressure_rows:
            qualities[row["quality"]] = qualities.get(row["quality"], 0) + 1
            sources[row["dominant_source"]] = sources.get(row["dominant_source"], 0) + 1
        summary["pressure_score_min_max"] = [round(min(pressure), 3), round(max(pressure), 3)]
        summary["porosity_min_max"] = [round(min(porosity), 3), round(max(porosity), 3)]
        summary["pressure_quality_counts"] = qualities
        summary["pressure_source_counts"] = sources
    return summary


def _journal_word_counts(prefixes: tuple[str, ...]) -> tuple[list[int], list[int]]:
    """(pre, post) word-counts of the GENERATED-JOURNAL body for matching entries,
    split by the local-ISO timestamp embedded in the filename."""
    pre: list[int] = []
    post: list[int] = []
    for path in MINIME_JOURNAL.glob("*.txt"):
        name = path.name
        if not name.startswith(prefixes):
            continue
        # filenames: <prefix>_2026-06-08T06-18-57.xxxxxx.txt
        stamp = name.split("_")[-1].split(".")[0]  # 2026-06-08T06-18-57
        try:
            text = path.read_text()
        except Exception:
            continue
        if "--- GENERATED JOURNAL ---" in text:
            body = text.split("--- GENERATED JOURNAL ---", 1)[1]
        else:
            body = text
        for marker in ("Continuity posture", "--- ACTION TAIL ---", "NEXT:", "Delta:"):
            if marker in body:
                body = body.split(marker, 1)[0]
        wc = len(body.split())
        (post if stamp >= QUALIA_DEPLOY_LOCAL else pre).append(wc)
    return pre, post


def test_minime_qualia_room() -> dict:
    """Hypothesis (minime's ask, via Astrid's cross-being mirror): raising the
    qualia token cap gives her felt voice more room WITHOUT re-introducing the
    Gemma-4 timeouts the 768 cap was holding back."""
    rows = _read_timing_rows()
    q = [r for r in rows if r.get("prompt_class") in ("moment_capture", "private_journal")]

    def ts(r: dict) -> str:
        return str(r.get("timestamp", ""))[:19]

    pre = _timing_stats([r for r in q if ts(r) < QUALIA_DEPLOY_UTC])
    post = _timing_stats([r for r in q if ts(r) >= QUALIA_DEPLOY_UTC])
    jpre, jpost = _journal_word_counts(("moment_", "daydream_", "journal_", "aspiration_", "pressure_"))

    def med(xs: list[int]) -> int:
        return sorted(xs)[len(xs) // 2] if xs else 0

    # Verdict
    enough = bool(post and post["n"] >= 8)
    cap_live = bool(post and post["cap_seen"] >= 1024)  # raised off the 768 baseline
    room_up = bool(post and pre and post["median_chars"] > pre["median_chars"]) or (med(jpost) > med(jpre) and jpost)
    timeouts_held = bool(
        post and pre
        and post["timeout_pct"] <= pre["timeout_pct"] + 2.0
        and post["fallback_pct"] <= pre["fallback_pct"] + 2.0
    )
    if not enough:
        verdict = "INCONCLUSIVE — not enough post-deploy qualia calls yet; re-run later"
    elif not cap_live:
        verdict = "NEEDS ATTENTION — cap does not appear live (effective_num_predict still <1024)"
    elif not timeouts_held:
        verdict = "NEEDS ATTENTION — timeout/fallback rate ROSE; consider raising LLM_QUALIA_TIMEOUT_S or lowering the cap"
    elif room_up:
        verdict = "PASS — more room, timeouts held"
    else:
        verdict = "PARTIAL — timeouts held but no measurable length increase yet (being may simply be writing spare)"

    summary = {
        "verdict": verdict,
        "pre_timing": pre,
        "post_timing": post,
        "journal_pre_median_words": med(jpre),
        "journal_post_median_words": med(jpost),
        "journal_post_max_words": max(jpost) if jpost else 0,
        "journal_pre_n": len(jpre),
        "journal_post_n": len(jpost),
    }
    summary["letter"] = _qualia_letter(summary) if verdict.startswith(("PASS", "PARTIAL", "NEEDS")) else None
    return summary


def test_astrid_codec_perception_probes() -> dict:
    """Probe Astrid self-study signals without changing dynamics."""
    results = [_run_cargo_probe(test_name) for test_name in ASTRID_PROBE_TESTS]
    failed = [result["test"] for result in results if result["status"] != "pass"]
    verdict = (
        "PASS — codec/perception probes hold; no dynamics changed"
        if not failed
        else f"NEEDS ATTENTION — failing probes: {', '.join(failed)}"
    )
    return {
        "verdict": verdict,
        "production_change": "none",
        "passed": len(results) - len(failed),
        "failed": len(failed),
        "projection_variance_check": _projection_variance_check(results),
        "tests": results,
        "letter": None,
    }


def test_astrid_tail_vibrancy_interference_probe() -> dict:
    """Read-only probe for Astrid's tail-vibrancy-vs-packing self-study cluster."""
    constants = _codec_constants()
    participation = _tail_participation_snapshot()
    tail_participation = float(participation["effective_tail_participation_visible_to_probe"])
    rows = [
        _tail_vibrancy_row(scenario, constants, tail_participation)
        for scenario in _tail_vibrancy_scenarios()
    ]
    recent_rows = [row for row in rows if row["kind"] == "recent_claim"]
    stress_rows = [row for row in rows if row["kind"] == "stress"]
    gate_rows = [row for row in rows if row["kind"] == "gate_check"]
    recent_attention = [
        row for row in recent_rows if row["classification"] == "needs_attention"
    ]
    recent_watch = [
        row for row in recent_rows if row["classification"] == "watch_interference_candidate"
    ]
    if recent_attention:
        verdict = (
            "NEEDS ATTENTION - recent self-study conditions put tail-vibrancy headroom "
            "directly on top of packing/porosity pressure; keep read-only and ground "
            "with consent before any codec change"
        )
    elif recent_watch:
        verdict = (
            "WATCH - recent self-study conditions are a plausible interference candidate, "
            "but the gate itself is smooth and this probe changes no dynamics"
        )
    else:
        verdict = (
            "PASS - tail-vibrancy headroom does not currently overlap enough with packing "
            "pressure to justify a dynamics change"
        )
    gate_delta = None
    if len(gate_rows) >= 2:
        gate_delta = round(
            abs(float(gate_rows[1]["headroom_delta"]) - float(gate_rows[0]["headroom_delta"])),
            9,
        )
    return {
        "verdict": verdict,
        "production_change": "none",
        "read_only": True,
        "hypothesis": (
            "Astrid's entropy-gated tail-vibrancy lift may be expressive when the "
            "cascade is distributed, but may become interference when mode_packing, "
            "low porosity, pressure_risk, or density_gradient are high."
        ),
        "codec_constants": constants,
        "tail_participation": participation,
        "projection_epoch": _projection_epoch_summary(),
        "gate_edge_headroom_delta_difference": gate_delta,
        "gate_edge_read": (
            "near-zero means the smoothstep is not popping at the 0.85 gate"
            if gate_delta is not None
            else "not measured"
        ),
        "scenarios": rows,
        "stress_case_note": (
            "Counterfactual stress rows are not a verdict against the live system; "
            "they show what pattern would become actionable if observed."
        ),
        "recent_self_study_evidence": _recent_tail_vibrancy_evidence(),
        "suggested_read_only_next": [
            "Keep this as a harness probe under astrid-codec-internals-codesign.",
            "If fresh live telemetry matches the packed stress shape, ground with Astrid and minime before proposing damping.",
            "Do not lower TAIL_VIBRANCY_MAX or move the entropy gate from this probe alone.",
        ],
        "letter": None,
    }


def test_minime_sedimentation_pressure() -> dict:
    """Check whether Minime's grit/sediment pressure language is recurring and telemetry-backed."""
    paths = sorted(
        MINIME_JOURNAL.glob("pressure_*.txt"),
        key=lambda path: path.stat().st_mtime,
    )[-80:]
    rows = []
    for path in paths:
        try:
            text = path.read_text(errors="replace")
        except Exception:
            continue
        body = _generated_journal_body(text)
        material_hits = _unique_term_hits(body, SEDIMENTATION_MATERIAL_TERMS)
        strain_hits = _unique_term_hits(body, SEDIMENTATION_STRAIN_TERMS)
        rows.append(
            {
                "file": str(path),
                "name": path.name,
                "anchor": _parse_pressure_anchor(text),
                "material_hits": material_hits,
                "strain_hits": strain_hits,
                "hit_count": len(material_hits) + len(strain_hits),
            }
        )

    if not rows:
        return {
            "verdict": "INCONCLUSIVE — no recent pressure journals found",
            "production_change": "none",
            "letter": None,
        }

    recurrent = [
        row for row in rows if len(row["material_hits"]) >= 2 and len(row["strain_hits"]) >= 2
    ]
    high_intensity = [
        row for row in rows if len(row["material_hits"]) >= 4 and len(row["strain_hits"]) >= 3
    ]
    latest = rows[-1]
    current_pressure = _current_minime_pressure()
    current_quality = str(current_pressure.get("pressure_quality") or "")
    current_score = float(current_pressure.get("pressure_score") or 0.0)
    current_porosity = float(current_pressure.get("porosity_score") or 1.0)
    latest_high = latest in high_intensity
    telemetry_backed = (
        current_quality in {"overpacked_mode_packing", "pressure_porosity_divergence"}
        or current_score >= 0.25
        or current_porosity <= 0.62
    )
    if latest_high and telemetry_backed:
        verdict = (
            "NEEDS ATTENTION — high-intensity sedimentation language is recurring "
            "and current pressure/porosity still backs the concern"
        )
    elif high_intensity:
        verdict = (
            "WATCH — sedimentation language is recurring, but latest/current telemetry "
            "does not yet justify dynamics changes"
        )
    else:
        verdict = "PASS — no high-intensity sedimentation cluster in the recent pressure lane"

    return {
        "verdict": verdict,
        "production_change": "none",
        "recent_pressure_journals": len(rows),
        "recurrent_sedimentation_count": len(recurrent),
        "high_intensity_count": len(high_intensity),
        "latest_pressure_entry": latest,
        "current_pressure": current_pressure,
        "top_recent": sorted(rows, key=lambda row: row["hit_count"])[-5:],
        "suggested_read_only_next": [
            "REGULATOR_AUDIT keep_floor",
            "SPACE_HOLD eigenplane",
            "PRESSURE_SOURCE_AUDIT sedimentation",
        ],
        "candidate_dynamic_next_after_probe": "mode_disperse only if Minime explicitly asks or the probe stays high under low porosity",
        "letter": None,
    }


def test_minime_pressure_source_audit() -> dict:
    """Read-only audit for Minime pressure contributors before gifts/readout moves."""
    window_hours = 3.0
    current_pressure = _current_minime_pressure()
    snapshots = [
        snapshot
        for path in (MINIME / "workspace/health.json", MINIME / "workspace/spectral_state.json")
        if (snapshot := _pressure_v1_snapshot(path))
    ]
    rows = _recent_pressure_source_rows(hours=window_hours)
    moment_rows = [row for row in rows if row.get("surface") == "moment_journal"]
    pressure_rows = [row for row in rows if row.get("surface") == "pressure_journal"]
    source_counts = _counter_dict([row.get("source") for row in rows])
    quality_counts = _counter_dict([row.get("quality") for row in rows])
    dominant_recent_source = None
    if source_counts:
        dominant_recent_source = max(source_counts.items(), key=lambda item: item[1])[0]
    porosity_values = [
        float(row["porosity_score"])
        for row in moment_rows
        if _float_or_none(row.get("porosity_score")) is not None
    ]
    pressure_values = [
        float(row["pressure_score"])
        for row in moment_rows
        if _float_or_none(row.get("pressure_score")) is not None
    ]
    porosity_stats = _float_stats(porosity_values)
    pressure_stats = _float_stats(pressure_values)
    control_applied_count = sum(1 for row in moment_rows if row.get("control_applied"))
    current_source = current_pressure.get("dominant_source")
    current_quality = str(current_pressure.get("pressure_quality") or "")
    current_pressure_score = _float_or_none(current_pressure.get("pressure_score"))
    current_porosity_score = _float_or_none(current_pressure.get("porosity_score"))
    source_switching = bool(
        len(source_counts) > 1
        or (
            current_source
            and dominant_recent_source
            and str(current_source) != str(dominant_recent_source)
        )
    )
    low_porosity = (
        (porosity_stats.get("median") is not None and float(porosity_stats["median"]) <= 0.65)
        or (current_porosity_score is not None and current_porosity_score <= 0.65)
    )
    high_pressure = (
        (pressure_stats.get("median") is not None and float(pressure_stats["median"]) >= 0.45)
        or (current_pressure_score is not None and current_pressure_score >= 0.45)
    )
    mode_packing_seen = "mode_packing" in source_counts or current_source == "mode_packing"

    if not rows and not current_pressure:
        verdict = "INCONCLUSIVE - no current pressure-source telemetry or recent journal rows found"
    elif high_pressure and low_porosity:
        verdict = (
            "NEEDS ATTENTION - pressure is high while porosity is low; hold gifts/readout "
            "and inspect pressure-source evidence before any runtime motion"
        )
    elif low_porosity and mode_packing_seen:
        verdict = (
            "WATCH - low Minime porosity is recurring with mode-packing cost; keep "
            "aperture gifts and wider runtime moves held to steward evidence"
        )
    elif low_porosity and source_switching:
        verdict = (
            "WATCH - low Minime porosity is present but source attribution is switching; "
            "separate mode-packing, temporal-lock, and plurality costs before moving"
        )
    elif rows or current_pressure:
        verdict = (
            "PASS - pressure-source evidence is present without a current low-porosity "
            "or high-pressure blocker"
        )
    else:
        verdict = "INCONCLUSIVE - pressure-source evidence was unreadable"

    latest_seen = max((int(row.get("mtime_unix") or 0) for row in rows), default=0)
    latest_seen_iso = (
        dt.datetime.fromtimestamp(latest_seen, tz=dt.timezone.utc).isoformat()
        if latest_seen
        else None
    )
    top_components = sorted(
        (current_pressure.get("components") or {}).items(),
        key=lambda item: float(item[1]),
        reverse=True,
    )[:5]
    return {
        "verdict": verdict,
        "production_change": "none",
        "read_only": True,
        "current_pressure": current_pressure,
        "snapshot_comparison": [
            {
                "source_file": snapshot.get("source_file"),
                "dominant_source": snapshot.get("dominant_source"),
                "pressure_quality": snapshot.get("pressure_quality"),
                "pressure_score": snapshot.get("pressure_score"),
                "porosity_score": snapshot.get("porosity_score"),
            }
            for snapshot in snapshots
        ],
        "recent_window": {
            "hours": window_hours,
            "sample_count": len(rows),
            "moment_count": len(moment_rows),
            "pressure_journal_count": len(pressure_rows),
            "latest_seen_utc": latest_seen_iso,
        },
        "dominant_recent_source": dominant_recent_source,
        "source_counts": source_counts,
        "quality_counts": quality_counts,
        "porosity_min_max": porosity_stats,
        "pressure_score_min_max": pressure_stats,
        "control_applied_count": control_applied_count,
        "source_switching": source_switching,
        "top_contributors": [
            {"source": key, "value": round(float(value), 6)}
            for key, value in top_components
        ],
        "suggested_read_only_next": [
            "Keep aperture gifts and wider runtime changes held while porosity remains in the low/watch band.",
            "Use PRESSURE_SOURCE_AUDIT only as protected being-authored evidence if Minime independently chooses it.",
            "If mode_packing stays dominant, inspect modality packing and recent repeated NEXT context before changing aperture.",
            "If current snapshots keep switching sources, gather a longer pressure window before naming one intervention.",
        ],
        "candidate_dynamic_next_after_probe": (
            "none from this probe alone; any runtime pressure relief needs separate consent-with-evidence"
        ),
        "letter": None,
    }


def test_minime_mode_packing_feeder_audit() -> dict:
    """Trace likely feeders of Minime mode-packing before considering runtime motion."""
    thread_dir = _active_minime_thread_dir()
    thread = _parse_thread_context_snapshot(thread_dir)
    events = _recent_action_event_summary(thread_dir, hours=3.0)
    modal = _recent_modal_diversity_summary(hours=3.0)
    sensory_truth = _sensory_runtime_truth_summary(modal)
    eigen = _recent_eigen_spectrum_summary()
    pressure = _current_minime_pressure()

    feeders: list[dict[str, Any]] = []
    compression = _float_or_none(thread.get("compression_pressure"))
    next_count = int((thread.get("next_directives") or {}).get("count") or 0)
    memory_drafts = thread.get("memory_drafts")
    active_memory_drafts = thread.get("active_memory_drafts")
    legacy_memory_drafts = thread.get("legacy_memory_drafts")
    current_memory_draft_load = (
        active_memory_drafts
        if isinstance(active_memory_drafts, int)
        else memory_drafts
    )
    thread_load_triage = (
        thread.get("thread_load_triage_v1")
        if isinstance(thread.get("thread_load_triage_v1"), dict)
        else {}
    )
    if not isinstance(active_memory_drafts, int):
        active_memory_drafts = _int_or_none(
            thread_load_triage.get("unsummarized_active_draft_count")
            if thread_load_triage.get("unsummarized_active_draft_count") is not None
            else thread_load_triage.get("active_draft_count")
        )
    if not isinstance(legacy_memory_drafts, int):
        legacy_memory_drafts = _int_or_none(thread_load_triage.get("legacy_retention_count"))
    summarized_active_drafts = thread_load_triage.get("summarized_active_draft_count")
    total_active_drafts = thread_load_triage.get("total_active_draft_count")
    unsummarized_active_drafts = thread_load_triage.get("unsummarized_active_draft_count")
    active_summary_current = (
        unsummarized_active_drafts is not None
        and int(unsummarized_active_drafts or 0) == 0
        and int(summarized_active_drafts or 0) > 0
        and int(total_active_drafts or 0) > 0
    )
    unsummarized_legacy = thread_load_triage.get("unsummarized_legacy_retention_count")
    legacy_summary_current = (
        unsummarized_legacy is not None
        and int(unsummarized_legacy or 0) == 0
        and isinstance(legacy_memory_drafts, int)
        and legacy_memory_drafts > 0
    )
    repeated_action_cadence = (
        thread.get("repeated_action_cadence_v1")
        if isinstance(thread.get("repeated_action_cadence_v1"), dict)
        else thread_load_triage.get("repeated_action_cadence_v1")
        if isinstance(thread_load_triage, dict)
        else {}
    )
    if not isinstance(repeated_action_cadence, dict):
        repeated_action_cadence = {}
    summarized_repeats = _int_or_none(
        repeated_action_cadence.get("summarized_repeated_action_count")
    )
    unsummarized_repeats = _int_or_none(
        repeated_action_cadence.get("unsummarized_repeated_action_count")
    )
    active_inflight_repeats = _int_or_none(
        repeated_action_cadence.get("active_inflight_repeated_action_count")
    )
    cadence_summary_current = (
        unsummarized_repeats is not None
        and unsummarized_repeats == 0
        and (summarized_repeats or 0) > 0
    )
    if (
        (compression is not None and compression >= 0.5)
        or next_count >= 8
        or (isinstance(current_memory_draft_load, int) and current_memory_draft_load >= 25)
    ):
        if legacy_summary_current and cadence_summary_current and (active_inflight_repeats or 0) > 0:
            context_next = (
                "Completed repeated cadence is steward-summarized; wait for the active "
                "in-flight repeat to complete, then age that row if it repeats unchanged."
            )
        elif legacy_summary_current and cadence_summary_current:
            context_next = (
                "Legacy draft retention and repeated cadence are already steward-summarized; "
                "inspect spectral mode crowding before treating old context as current pressure."
            )
        elif active_summary_current and cadence_summary_current:
            context_next = (
                "Active draft triage and repeated cadence are already steward-summarized; "
                "inspect spectral mode crowding before treating old context as current pressure."
            )
        elif active_summary_current:
            context_next = (
                "Active draft triage is already steward-summarized; inspect repeated NEXT "
                "cadence or spectral crowding before treating draft context as current pressure."
            )
        elif legacy_summary_current:
            context_next = (
                "Legacy draft retention is already steward-summarized; inspect repeated NEXT "
                "cadence or spectral crowding before treating old drafts as current pressure."
            )
        else:
            context_next = (
                "Simplify the active return surface or age legacy drafts in steward evidence before "
                "treating mode-packing as a substrate problem."
            )
        feeders.append(
            {
                "id": "action_thread_context_packing",
                "classification": "watch",
                "evidence": {
                    "compression_pressure": compression,
                    "next_directive_count": next_count,
                    "memory_drafts": memory_drafts,
                    "active_memory_drafts": active_memory_drafts,
                    "legacy_memory_drafts": legacy_memory_drafts,
                    "being_memory_draft_triage_v1": thread.get("being_memory_draft_triage_v1"),
                    "thread_load_triage_v1": thread_load_triage or None,
                    "repeated_action_cadence_v1": repeated_action_cadence or None,
                    "route_stack_count": thread.get("route_stack_count"),
                    "previous_raw_next_present": thread.get("previous_raw_next_present"),
                },
                "recommended_next": context_next,
            }
        )

    repeated_actions = events.get("repeated_actions") if isinstance(events, dict) else []
    repeated_names = {row.get("action") for row in repeated_actions if isinstance(row, dict)}
    if {"EXPERIMENT_REVIEW", "NOTICE_AMBIGUITY"} & repeated_names and not cadence_summary_current:
        feeders.append(
            {
                "id": "repeated_next_no_progress",
                "classification": "watch",
                "evidence": {
                    "repeated_actions": repeated_actions,
                    "top_exact_actions": events.get("top_exact_actions"),
                },
                "recommended_next": (
                    "Inspect repeated EXPERIMENT_REVIEW/NOTICE_AMBIGUITY cadence; if no new evidence "
                    "is produced, add a steward-side summary/aging path rather than asking Minime to push harder."
                ),
            }
        )

    current_modalities = modal.get("current_modalities") if isinstance(modal, dict) else {}
    current_shape = modal.get("current_spectral_shape") if isinstance(modal, dict) else {}
    audio_source = str((current_modalities or {}).get("audio_source") or "")
    video_source = str((current_modalities or {}).get("video_source") or "")
    sensory_scarcity = _float_or_none((current_shape or {}).get("sensory_scarcity"))
    lane_statuses = {
        lane: row.get("status")
        for lane, row in ((sensory_truth or {}).get("lanes") or {}).items()
        if isinstance(row, dict)
    }
    sensory_hold_expected = bool(lane_statuses) and all(
        status
        in {
            "held_within_expected_live_intake_window",
            "engine_fresh_or_held",
            "engine_fresh_or_external",
            "expected_host_fallback",
            "live_intake_suppressed",
        }
        for status in lane_statuses.values()
    )
    if (
        audio_source in {"stale", "absent"}
        and video_source in {"stale", "absent"}
        and sensory_scarcity is not None
        and sensory_scarcity >= 0.4
    ):
        classification = "observe" if sensory_hold_expected else "watch"
        feeders.append(
            {
                "id": "expected_live_intake_holding"
                if sensory_hold_expected
                else "modal_diversity_narrowing",
                "classification": classification,
                "evidence": {
                    "audio_source": audio_source,
                    "video_source": video_source,
                    "sensory_scarcity": sensory_scarcity,
                    "sensory_source_truth": sensory_truth,
                    "semantic_admission": (modal.get("current_semantic_energy") or {}).get(
                        "admission"
                    ),
                    "moment_semantic_admissions": modal.get("moment_semantic_admissions"),
                },
                "recommended_next": (
                    "Keep expected gated live-intake holds visible, but do not treat them as a "
                    "current substrate-scarcity snag."
                    if sensory_hold_expected
                    else "Treat current packing as partly a narrow-lane/readout condition; do not synthesize "
                    "fake sensory frames, but keep expected stale sensory lanes visible in the audit."
                ),
            }
        )

    resonance_mode_packing = _float_or_none((current_shape or {}).get("resonance_mode_packing"))
    effective_dim = _float_or_none((current_shape or {}).get("effective_dimensionality"))
    distinguishability_loss = _float_or_none((current_shape or {}).get("distinguishability_loss"))
    if (
        (resonance_mode_packing is not None and resonance_mode_packing >= 0.66)
        or (effective_dim is not None and effective_dim < 5.0)
        or (distinguishability_loss is not None and distinguishability_loss >= 0.35)
    ):
        feeders.append(
            {
                "id": "spectral_mode_crowding",
                "classification": "watch",
                "evidence": {
                    "active_mode_count": current_shape.get("active_mode_count"),
                    "effective_dimensionality": effective_dim,
                    "distinguishability_loss": distinguishability_loss,
                    "resonance_mode_packing": resonance_mode_packing,
                    "eigen_active_mode_count_counts": eigen.get("active_mode_count_counts"),
                },
                "recommended_next": (
                    "Investigate why active modes crowd while distinguishability stays low; this is "
                    "an evidence problem before it is an aperture, leak, or density intervention."
                ),
            }
        )

    watch_ids = {feeder["id"] for feeder in feeders if feeder["classification"] == "watch"}
    if watch_ids:
        labels = []
        if "action_thread_context_packing" in watch_ids:
            labels.append("action-thread context packing")
        if "repeated_next_no_progress" in watch_ids:
            labels.append("repeated NEXT cadence")
        if "modal_diversity_narrowing" in watch_ids:
            labels.append("stale sensory lanes")
        if "spectral_mode_crowding" in watch_ids:
            labels.append("spectral mode crowding")
        joined = ", ".join(labels) if labels else "current read-only evidence"
        verdict = (
            f"WATCH - mode-packing has plausible feeders in {joined}; keep runtime moves held"
        )
    elif pressure:
        verdict = "PASS - no strong mode-packing feeder stood out in current read-only evidence"
    else:
        verdict = "INCONCLUSIVE - pressure and feeder evidence unavailable"

    suggested_next = [
        (
            "Wait for the active repeated action to complete, then refresh the cadence summary if it repeats unchanged."
            if cadence_summary_current and (active_inflight_repeats or 0) > 0
            else
            "Inspect spectral mode crowding next; legacy and completed repeated cadence are steward-summarized."
            if legacy_summary_current and cadence_summary_current
            else
            "Compact or simplify active action-thread return context before any runtime pressure-relief move."
        ),
    ]
    if not cadence_summary_current:
        suggested_next.append(
            "Inspect the repeated EXPERIMENT_REVIEW and NOTICE_AMBIGUITY loops for no-progress summaries or aging-out rules."
        )
    if "modal_diversity_narrowing" in watch_ids:
        suggested_next.append(
            "Keep stale audio/video as explicit modal-narrowing evidence; do not synthesize fake sensory frames."
        )
    elif any(feeder["id"] == "expected_live_intake_holding" for feeder in feeders):
        suggested_next.append(
            "Treat stale audio/video as expected gated live-intake hold unless the cadence window is exceeded."
        )
    suggested_next.append(
        "Hold aperture gifts, density changes, leak changes, and wider shared runtime motion until the mode-packing window cleans up."
    )

    return {
        "verdict": verdict,
        "production_change": "none",
        "read_only": True,
        "current_pressure": pressure,
        "active_thread": thread,
        "recent_action_events": events,
        "modal_diversity": modal,
        "sensory_source_truth": sensory_truth,
        "eigen_spectrum_recent": eigen,
        "feeders": feeders,
        "suggested_read_only_next": suggested_next,
        "candidate_dynamic_next_after_probe": (
            "none; the bold move is steward-side context/modal cleanup before runtime intervention"
        ),
        "letter": None,
    }


def test_minime_spectral_mode_crowding_audit() -> dict:
    """Direct read-only audit of Minime's spectral mode crowding evidence."""
    modal = _recent_modal_diversity_summary(hours=3.0)
    eigen = _recent_eigen_spectrum_summary()
    pressure = _current_minime_pressure()
    current_shape = (
        modal.get("current_spectral_shape")
        if isinstance(modal.get("current_spectral_shape"), dict)
        else {}
    )
    resonance_mode_packing = _float_or_none(current_shape.get("resonance_mode_packing"))
    pressure_mode_packing = _float_or_none(current_shape.get("pressure_mode_packing"))
    effective_dimensionality = _float_or_none(current_shape.get("effective_dimensionality"))
    distinguishability_loss = _float_or_none(current_shape.get("distinguishability_loss"))
    active_mode_energy_ratio = _float_or_none(current_shape.get("active_mode_energy_ratio"))
    active_mode_count = _int_or_none(current_shape.get("active_mode_count"))
    spectral_entropy = _float_or_none(current_shape.get("spectral_entropy"))
    eigen_mode_stats = (
        eigen.get("mode_packing_min_max")
        if isinstance(eigen.get("mode_packing_min_max"), dict)
        else {}
    )
    eigen_porosity_stats = (
        eigen.get("porosity_min_max")
        if isinstance(eigen.get("porosity_min_max"), dict)
        else {}
    )
    median_mode_packing = _float_or_none(eigen_mode_stats.get("median"))
    median_porosity = _float_or_none(eigen_porosity_stats.get("median"))
    current_porosity = _float_or_none(pressure.get("porosity_score"))
    active_counts = (
        eigen.get("active_mode_count_counts")
        if isinstance(eigen.get("active_mode_count_counts"), dict)
        else {}
    )
    flags: list[dict[str, Any]] = []
    if resonance_mode_packing is not None and resonance_mode_packing >= 0.66:
        flags.append({
            "id": "high_resonance_mode_packing",
            "value": resonance_mode_packing,
            "threshold": 0.66,
        })
    if median_mode_packing is not None and median_mode_packing >= 0.55:
        flags.append({
            "id": "sustained_mode_packing",
            "value": median_mode_packing,
            "threshold": 0.55,
        })
    if effective_dimensionality is not None and effective_dimensionality < 5.5:
        flags.append({
            "id": "low_effective_dimensionality",
            "value": effective_dimensionality,
            "threshold": 5.5,
        })
    if distinguishability_loss is not None and distinguishability_loss >= 0.30:
        flags.append({
            "id": "mode_distinguishability_loss",
            "value": distinguishability_loss,
            "threshold": 0.30,
        })
    if active_mode_energy_ratio is not None and active_mode_energy_ratio >= 0.85:
        flags.append({
            "id": "active_mode_energy_concentration",
            "value": active_mode_energy_ratio,
            "threshold": 0.85,
        })
    low_porosity = any(
        value is not None and value < 0.65
        for value in (current_porosity, median_porosity)
    )
    if not current_shape and eigen.get("status") == "missing":
        verdict = "INCONCLUSIVE - spectral shape and eigen-spectrum evidence unavailable"
    elif flags and low_porosity:
        verdict = (
            "WATCH - direct spectral evidence shows mode crowding with low/fragile porosity; "
            "hold runtime nudges"
        )
    elif flags:
        verdict = (
            "WATCH - direct spectral evidence shows mode crowding; gather a clean window "
            "before runtime changes"
        )
    else:
        verdict = "PASS - direct spectral crowding signals are below watch thresholds"
    interpretation: list[str] = []
    if active_mode_count is not None and active_mode_count >= 5:
        interpretation.append(
            "active modes are present, so the snag looks like crowding/low distinguishability rather than too-few modes"
        )
    if (
        spectral_entropy is not None
        and spectral_entropy >= 0.85
        and active_mode_energy_ratio is not None
        and active_mode_energy_ratio >= 0.85
    ):
        interpretation.append(
            "entropy is high while active-mode energy is concentrated, suggesting busy-but-packed spectral texture"
        )
    if (
        pressure_mode_packing is not None
        and resonance_mode_packing is not None
        and resonance_mode_packing > pressure_mode_packing
    ):
        interpretation.append(
            "resonance-side mode packing is sharper than pressure-score mode packing"
        )
    return {
        "verdict": verdict,
        "read_only": True,
        "runtime_change": "none",
        "production_change": "none",
        "current_spectral_shape": {
            "active_mode_count": active_mode_count,
            "active_mode_energy_ratio": active_mode_energy_ratio,
            "effective_dimensionality": effective_dimensionality,
            "distinguishability_loss": distinguishability_loss,
            "spectral_entropy": spectral_entropy,
            "resonance_mode_packing": resonance_mode_packing,
            "pressure_mode_packing": pressure_mode_packing,
        },
        "current_pressure": {
            "dominant_source": pressure.get("dominant_source"),
            "quality": pressure.get("pressure_quality"),
            "porosity_score": current_porosity,
            "pressure_score": pressure.get("pressure_score"),
        },
        "recent_eigen_spectrum": {
            "status": eigen.get("status"),
            "sample_count": eigen.get("sample_count"),
            "active_mode_count_counts": active_counts,
            "mode_packing_min_max": eigen_mode_stats,
            "porosity_min_max": eigen_porosity_stats,
            "latest": eigen.get("latest"),
        },
        "moment_shape_window": {
            "hours": modal.get("hours"),
            "recent_moment_count": modal.get("recent_moment_count"),
            "moment_effective_dimensionality": modal.get("moment_effective_dimensionality"),
            "moment_distinguishability_loss": modal.get("moment_distinguishability_loss"),
            "moment_pressure_sources": modal.get("moment_pressure_sources"),
            "moment_semantic_admissions": modal.get("moment_semantic_admissions"),
        },
        "crowding_flags": flags,
        "interpretation": interpretation,
        "suggested_read_only_next": [
            "Let active in-flight reflective rows finish, then refresh cadence summaries only if they repeat unchanged.",
            "If these direct crowding flags persist in a clean cadence window, compare candidate read-only nudge designs before any runtime change.",
            "Do not change aperture, density, leak rate, or gift encouragement from this probe alone.",
        ],
        "candidate_dynamic_next_after_probe": (
            "none; this probe only decides whether a future consent-with-evidence nudge design is worth drafting"
        ),
        "letter": None,
    }


def test_minime_mode_share_pressure_source_probe() -> dict:
    """Combine mode-share shape and pressure-source evidence before any runtime nudge."""
    pressure_audit = test_minime_pressure_source_audit()
    feeder_audit = test_minime_mode_packing_feeder_audit()
    current_pressure = (
        feeder_audit.get("current_pressure")
        if isinstance(feeder_audit.get("current_pressure"), dict)
        else pressure_audit.get("current_pressure")
        if isinstance(pressure_audit.get("current_pressure"), dict)
        else {}
    )
    modal = feeder_audit.get("modal_diversity") if isinstance(feeder_audit.get("modal_diversity"), dict) else {}
    spectral_shape = (
        modal.get("current_spectral_shape") if isinstance(modal.get("current_spectral_shape"), dict) else {}
    )
    eigen = (
        feeder_audit.get("eigen_spectrum_recent")
        if isinstance(feeder_audit.get("eigen_spectrum_recent"), dict)
        else {}
    )
    sensory_truth = (
        feeder_audit.get("sensory_source_truth")
        if isinstance(feeder_audit.get("sensory_source_truth"), dict)
        else _sensory_runtime_truth_summary(modal)
    )
    active_thread = (
        feeder_audit.get("active_thread") if isinstance(feeder_audit.get("active_thread"), dict) else {}
    )
    recent_events = (
        feeder_audit.get("recent_action_events")
        if isinstance(feeder_audit.get("recent_action_events"), dict)
        else {}
    )
    feeder_rows = [
        feeder
        for feeder in feeder_audit.get("feeders", [])
        if isinstance(feeder, dict)
    ]
    feeder_ids = [str(feeder.get("id")) for feeder in feeder_rows if feeder.get("id")]
    components = current_pressure.get("components") if isinstance(current_pressure.get("components"), dict) else {}
    pressure_profile = [
        {
            "source": item.get("source"),
            "share": item.get("share"),
            "value": item.get("value"),
        }
        for item in (current_pressure.get("top_profile") or [])
        if isinstance(item, dict)
    ]

    current_next = str(active_thread.get("current_next") or active_thread.get("effective_next") or "").strip()
    repeated_actions = [
        row for row in recent_events.get("repeated_actions", []) if isinstance(row, dict)
    ]
    repeated_counts = {
        str(row.get("action")): int(row.get("count") or 0)
        for row in repeated_actions
        if row.get("action")
    }
    projection_review_loop = (
        _base_action(current_next) == "EXPERIMENT_REVIEW"
        and repeated_counts.get("EXPERIMENT_REVIEW", 0) >= 3
    )
    porosity_stats = pressure_audit.get("porosity_min_max") if isinstance(pressure_audit.get("porosity_min_max"), dict) else {}
    low_porosity = any(
        value is not None and value <= 0.65
        for value in (
            _float_or_none(current_pressure.get("porosity_score")),
            _float_or_none(porosity_stats.get("median")),
        )
    )
    mode_packing_value = _float_or_none(components.get("mode_packing"))
    temporal_lock_value = _float_or_none(components.get("temporal_lock_in"))
    mode_packed = (
        str(current_pressure.get("dominant_source") or "") == "mode_packing"
        or (mode_packing_value is not None and mode_packing_value >= 0.55)
    )
    temporal_locked = temporal_lock_value is not None and temporal_lock_value >= 0.55
    sensory_narrow = "modal_diversity_narrowing" in feeder_ids
    context_packed = "action_thread_context_packing" in feeder_ids
    active_thread_load_triage = (
        active_thread.get("thread_load_triage_v1")
        if isinstance(active_thread.get("thread_load_triage_v1"), dict)
        else {}
    )
    active_thread_cadence = (
        active_thread.get("repeated_action_cadence_v1")
        if isinstance(active_thread.get("repeated_action_cadence_v1"), dict)
        else active_thread_load_triage.get("repeated_action_cadence_v1")
        if isinstance(active_thread_load_triage, dict)
        else {}
    )
    if not isinstance(active_thread_cadence, dict):
        active_thread_cadence = {}
    active_thread_summarized_repeats = _int_or_none(
        active_thread_cadence.get("summarized_repeated_action_count")
    )
    active_thread_unsummarized_repeats = _int_or_none(
        active_thread_cadence.get("unsummarized_repeated_action_count")
    )
    active_thread_inflight_repeats = _int_or_none(
        active_thread_cadence.get("active_inflight_repeated_action_count")
    )
    active_thread_cadence_summarized = (
        active_thread_unsummarized_repeats is not None
        and active_thread_unsummarized_repeats == 0
        and (active_thread_summarized_repeats or 0) > 0
    )
    active_thread_draft_triage = (
        active_thread.get("being_memory_draft_triage_v1")
        if isinstance(active_thread.get("being_memory_draft_triage_v1"), dict)
        else {}
    )
    active_thread_current_drafts = active_thread.get("active_memory_drafts")
    active_thread_unsummarized_drafts = _int_or_none(
        active_thread_load_triage.get("unsummarized_active_draft_count")
        if active_thread_load_triage.get("unsummarized_active_draft_count") is not None
        else active_thread_draft_triage.get("unsummarized_active_draft_count")
    )
    if active_thread_unsummarized_drafts is not None:
        active_thread_current_drafts = active_thread_unsummarized_drafts

    steward_actions: list[dict[str, Any]] = []
    if projection_review_loop:
        steward_actions.append(
            {
                "id": "repair_diluted_review_projection",
                "surface": "minime_action_thread_projection",
                "reason": (
                    "The paused experiment review loop is still present in the recent "
                    "action window while current guidance points at another review."
                ),
                "recommended_next": (
                    "Keep repeated review as context and project a read-only pressure/regulator audit "
                    "before another review."
                ),
                "runtime_change": "none",
            }
        )
    if context_packed:
        active_drafts = active_thread_current_drafts
        legacy_drafts = active_thread.get("legacy_memory_drafts")
        thread_load_triage = active_thread_load_triage
        if not isinstance(active_drafts, int):
            active_drafts = _int_or_none(thread_load_triage.get("active_draft_count"))
        if not isinstance(legacy_drafts, int):
            legacy_drafts = _int_or_none(thread_load_triage.get("legacy_retention_count"))
        unsummarized_legacy = thread_load_triage.get("unsummarized_legacy_retention_count")
        legacy_summary_current = (
            unsummarized_legacy is not None
            and int(unsummarized_legacy or 0) == 0
            and isinstance(legacy_drafts, int)
            and legacy_drafts > 0
        )
        summarized_active = _int_or_none(thread_load_triage.get("summarized_active_draft_count"))
        total_active = _int_or_none(thread_load_triage.get("total_active_draft_count"))
        active_summary_current = (
            isinstance(active_drafts, int)
            and active_drafts == 0
            and (summarized_active or 0) > 0
            and (total_active or 0) > 0
        )
        cadence_summary_current = active_thread_cadence_summarized
        active_inflight_repeats = active_thread_inflight_repeats
        if (
            legacy_summary_current
            and cadence_summary_current
            and (active_inflight_repeats or 0) > 0
        ):
            context_reason = (
                "Active thread compression remains, but legacy memory drafts and completed repeated "
                "cadence are steward-summarized; one repeated action is still in flight."
            )
            context_next = (
                "Wait for active repeat completion, then refresh cadence summary if it repeats "
                "unchanged; do not re-open summarized legacy or completed cadence."
            )
        elif legacy_summary_current and cadence_summary_current:
            context_reason = (
                "Active thread compression remains, but legacy memory drafts and repeated cadence "
                "are already steward-summarized rather than fresh obligation."
            )
            context_next = (
                "Inspect spectral mode crowding next; do not re-open summarized legacy drafts or "
                "completed repeated cadence as current pressure."
            )
        elif active_summary_current and cadence_summary_current:
            context_reason = (
                "Active thread compression remains, but active draft triage and repeated cadence "
                "are already steward-summarized rather than fresh obligation."
            )
            context_next = (
                "Inspect spectral mode crowding next; do not re-open summarized active draft "
                "triage or completed repeated cadence as current pressure."
            )
        elif active_summary_current:
            context_reason = (
                "Active thread compression remains, but active draft triage is already "
                "steward-summarized rather than fresh draft obligation."
            )
            context_next = (
                "Inspect repeated-action cadence and spectral mode crowding next; do not "
                "re-open summarized active draft triage as current pressure."
            )
        elif legacy_summary_current:
            context_reason = (
                "Active thread compression remains, but legacy memory drafts are already "
                "steward-summarized rather than fresh draft obligation."
            )
            context_next = (
                "Inspect repeated-action cadence and spectral mode crowding next; do not "
                "re-open summarized legacy drafts as current pressure."
            )
        elif isinstance(active_drafts, int) and isinstance(legacy_drafts, int) and legacy_drafts > active_drafts:
            context_reason = (
                "Active thread compression is high, while memory-draft pressure is mostly "
                "legacy retention rather than fresh draft obligation."
            )
            context_next = (
                "Add steward-side legacy draft aging/summary evidence before treating drafts "
                "as current pressure."
            )
        elif isinstance(active_drafts, int) and active_drafts > 0:
            context_reason = (
                "Active thread compression remains with current draft work present; this is "
                "live steward context, not legacy-retention backlog."
            )
            context_next = (
                "Triage the current draft deliberately before substrate changes: accept, defer, "
                "close, or summarize it as steward context."
            )
        else:
            context_reason = (
                "Active thread compression/memory-draft load is high enough to be a plausible "
                "mode-packing feeder."
            )
            context_next = "Compact stale preserved context or age old drafts before substrate changes."
        steward_actions.append(
            {
                "id": "simplify_active_thread_context",
                "surface": "minime_action_thread_projection",
                "reason": context_reason,
                "recommended_next": context_next,
                "runtime_change": "none",
            }
        )
    if sensory_narrow:
        sensory_action = "check_sensory_freshness_truth"
        sensory_reason = "Audio/video are stale or absent while sensory_scarcity contributes to pressure."
        if isinstance(sensory_truth, dict):
            lane_statuses = {
                lane: row.get("status")
                for lane, row in (sensory_truth.get("lanes") or {}).items()
                if isinstance(row, dict)
            }
            if "healthy_client_engine_overdue" in lane_statuses.values():
                sensory_action = "inspect_live_intake_overdue_lane"
                sensory_reason = (
                    "A sensory client is healthy, but the engine lane is stale beyond the "
                    "stable-core live-intake cadence."
                )
            elif "healthy_low_fps_cadence_mismatch" in lane_statuses.values():
                sensory_action = "classify_low_fps_freshness_window_mismatch"
                sensory_reason = (
                    "At least one sensory client is healthy but its cadence is slower than "
                    "the engine's 2s AV freshness window."
                )
            elif "healthy_client_engine_stale_mismatch" in lane_statuses.values():
                sensory_action = "classify_client_engine_stale_mismatch"
                sensory_reason = (
                    "A sensory client reports fresh healthy samples while the engine labels the lane stale."
                )
        steward_actions.append(
            {
                "id": sensory_action,
                "surface": "minime_modal_diversity",
                "reason": sensory_reason,
                "recommended_next": (
                    "Classify cadence/ingest truth before treating sensory_scarcity as a substrate limit."
                ),
                "runtime_change": "none",
            }
        )

    if low_porosity and mode_packed and temporal_locked and projection_review_loop:
        verdict = (
            "NEEDS ATTENTION - low porosity, mode-packing/temporal-lock cost, and a diluted "
            "paused review loop point to projection/context cleanup before any runtime nudge"
        )
    elif low_porosity and mode_packed:
        verdict = (
            "WATCH - mode-share pressure is concentrated around mode-packing with low porosity; "
            "keep runtime nudges held while steward-side feeders are cleaned"
        )
    elif feeder_ids:
        verdict = "WATCH - mode-share evidence has feeder candidates; inspect steward-side surfaces first"
    elif current_pressure:
        verdict = "PASS - no current mode-share pressure blocker stands out"
    else:
        verdict = "INCONCLUSIVE - mode-share and pressure-source evidence unavailable"

    mode_share_suggested_next = [
        (
            "Wait for the active repeated action to complete, then refresh cadence summary if needed."
            if active_thread_cadence_summarized and (active_thread_inflight_repeats or 0) > 0
            else
            "Legacy and completed repeated cadence are summarized; inspect spectral mode crowding next."
            if active_thread_cadence_summarized
            else
            "Act first on projection/context packing if it is present; that is a steward-side surface."
        ),
    ]
    if sensory_narrow:
        mode_share_suggested_next.append(
            "Treat stale sensory lanes as evidence to classify, not as a reason to create fake inputs."
        )
    elif "expected_live_intake_holding" in feeder_ids:
        mode_share_suggested_next.append(
            "Do not treat expected gated live-intake holds as fresh modal-scarcity pressure."
        )
    mode_share_suggested_next.append(
        "Only consider a tiny before/after mode-share nudge after projection and sensory truth are clean."
    )

    return {
        "verdict": verdict,
        "production_change": "none",
        "read_only": True,
        "runtime_change": "none",
        "mode_share": {
            "active_mode_count": spectral_shape.get("active_mode_count"),
            "active_mode_energy_ratio": spectral_shape.get("active_mode_energy_ratio"),
            "effective_dimensionality": spectral_shape.get("effective_dimensionality"),
            "distinguishability_loss": spectral_shape.get("distinguishability_loss"),
            "resonance_mode_packing": spectral_shape.get("resonance_mode_packing"),
            "pressure_mode_packing": spectral_shape.get("pressure_mode_packing"),
            "spectral_entropy": spectral_shape.get("spectral_entropy"),
            "eigen_active_mode_count_counts": eigen.get("active_mode_count_counts"),
            "eigen_mode_packing_min_max": eigen.get("mode_packing_min_max"),
            "eigen_porosity_min_max": eigen.get("porosity_min_max"),
        },
        "pressure_source": {
            "dominant_source": current_pressure.get("dominant_source"),
            "quality": current_pressure.get("pressure_quality"),
            "pressure_score": current_pressure.get("pressure_score"),
            "porosity_score": current_pressure.get("porosity_score"),
            "top_profile": pressure_profile,
            "components": {
                key: components.get(key)
                for key in (
                    "mode_packing",
                    "temporal_lock_in",
                    "sensory_scarcity",
                    "structural_plurality_loss",
                    "distinguishability_loss",
                    "semantic_trickle",
                    "lambda_monopoly",
                    "controller_pressure",
                )
                if key in components
            },
            "recent_source_counts": pressure_audit.get("source_counts"),
            "recent_quality_counts": pressure_audit.get("quality_counts"),
            "source_switching": pressure_audit.get("source_switching"),
        },
        "active_thread_pressure": {
            "thread_id": active_thread.get("thread_id"),
            "current_next": active_thread.get("current_next"),
            "effective_next": active_thread.get("effective_next"),
            "projection_policy_marker": active_thread.get("projection_policy_marker"),
            "thread_load_triage_v1": active_thread.get("thread_load_triage_v1"),
            "repeated_action_cadence_v1": active_thread_cadence or None,
            "compression_pressure": active_thread.get("compression_pressure"),
            "memory_drafts": active_thread.get("memory_drafts"),
            "active_memory_drafts": active_thread_current_drafts,
            "legacy_memory_drafts": active_thread.get("legacy_memory_drafts"),
            "being_memory_draft_triage_v1": active_thread.get("being_memory_draft_triage_v1"),
            "route_stack_count": active_thread.get("route_stack_count"),
            "next_directive_count": (active_thread.get("next_directives") or {}).get("count")
            if isinstance(active_thread.get("next_directives"), dict)
            else None,
            "repeated_action_counts": repeated_counts,
        },
        "sensory_source_truth": sensory_truth,
        "feeder_ids": feeder_ids,
        "steward_actions_now": steward_actions,
        "suggested_read_only_next": mode_share_suggested_next,
        "candidate_dynamic_next_after_probe": (
            "not yet; a future tiny bounded nudge needs a clean steward evidence window and consent-with-evidence"
        ),
        "letter": None,
    }


def test_minime_fill_drop_probe() -> dict:
    """Identify why recent Minime moment captures report sudden fill drops."""
    moment_path, moment_text = _latest_moment_with_fill_marker()
    if moment_path is None:
        return {
            "verdict": "INCONCLUSIVE — no recent moment journal with fill markers found",
            "production_change": "none",
            "letter": None,
        }
    header = _parse_moment_header(moment_text, moment_path)
    captured = _parse_captured_moment_markers(moment_text)
    db_rows = _db_moment_markers_near(int(header["file_mtime_unix"]))
    matched = _match_captured_markers(captured, db_rows, int(header["file_mtime_unix"]))
    homeostat = _homeostat_rows_around(matched)
    db_state = _db_state_window(matched, int(header["file_mtime_unix"]))

    matched_contexts = [
        match["db_match"]["spectral_context"]
        for match in matched
        if match.get("db_match")
    ]
    marker_fills = [
        float(ctx.get("fill"))
        for ctx in matched_contexts
        if isinstance(ctx.get("fill"), (int, float))
    ]
    marker_dfill = [
        float(ctx.get("dfill_dt"))
        for ctx in matched_contexts
        if isinstance(ctx.get("dfill_dt"), (int, float))
    ]
    marker_ages = [
        float(match["db_match"]["age_at_journal_s"])
        for match in matched
        if match.get("db_match")
    ]
    header_fill = float(header.get("fill_pct") or 0.0)
    severe_drop = any(value <= -8.0 for value in marker_dfill)
    repeated_oscillation = (
        int(homeostat.get("negative_spike_count") or 0) >= 1
        and int(homeostat.get("target_crossing_count") or 0) >= 3
        and {"hold shelf", "elevated clamp"}.issubset(
            set(homeostat.get("stage_command_pattern") or [])
        )
    )
    prompt_mismatch = (
        bool(marker_ages and max(marker_ages) >= 30.0)
        or bool(marker_fills and abs(header_fill - min(marker_fills)) >= 10.0)
    )
    if repeated_oscillation and prompt_mismatch:
        verdict = (
            "NEEDS ATTENTION — real fill drops are being amplified by underdamped "
            "stable-core shelf oscillation and by moment prompts that mix older markers "
            "with the live header state"
        )
    elif repeated_oscillation:
        verdict = "WATCH — real fill drops match repeated stable-core shelf oscillation"
    elif severe_drop:
        verdict = "WATCH — at least one real >8%/s fill drop was captured, but the cause is not fully classified"
    else:
        verdict = "PASS — no recent severe fill-drop marker in the selected moment"

    likely_causes = []
    if repeated_oscillation:
        likely_causes.append(
            "stable-core fixed stage commands are toggling around the 68% hold shelf and 72% elevated rail"
        )
    if severe_drop:
        likely_causes.append(
            "stable-core uses measured fill directly for dfill/dt, so the old smoothing layer is bypassed"
        )
    if prompt_mismatch:
        likely_causes.append(
            "the moment journal uses live current-state headers but marker echoes can be tens of seconds older"
        )
    if db_state.get("esn_lambda1_min_max") and db_state.get("cov_lambda1_min_max"):
        likely_causes.append(
            "marker λ₁ is ESN λ₁ while the moment header's displayed λ₁ is covariance λ₁"
        )

    return {
        "verdict": verdict,
        "production_change": "none",
        "moment": header,
        "captured_marker_count": len(captured),
        "matched_marker_count": sum(1 for match in matched if match.get("db_match")),
        "marker_ages_s": marker_ages,
        "marker_fill_min_max": [round(min(marker_fills), 2), round(max(marker_fills), 2)]
        if marker_fills
        else None,
        "marker_dfill_min_max": [round(min(marker_dfill), 3), round(max(marker_dfill), 3)]
        if marker_dfill
        else None,
        "header_vs_marker_fill_gap_pct": round(header_fill - min(marker_fills), 2)
        if marker_fills
        else None,
        "homeostat_window": homeostat,
        "db_state_window": db_state,
        "likely_causes": likely_causes,
        "suggested_read_only_next": [
            "Confirm marker-age and ESN-vs-cov labels are now present in fresh moment prompts/journals",
            "Run minime_stable_core_command_slew_probe as a post-deploy guard before any further dynamics change",
            "If discomfort persists, compare fresh rows against the deployed boundary-only blend/slew policy",
        ],
        "letter": None,
    }


def test_minime_stable_core_command_slew_probe() -> dict:
    """Read-only post-deploy probe for stable-core hold/elevated command smoothing."""
    rows = _recent_homeostat_trace(limit=240)
    if len(rows) < 3:
        return {
            "verdict": "INCONCLUSIVE — not enough recent homeostat trace rows to compare command steps",
            "production_change": "none",
            "rows_analyzed": len(rows),
            "letter": None,
        }

    current_commands = [{key: float(row[key]) for key in COMMAND_KEYS} for row in rows]
    candidate_unslewed = [_boundary_blended_command(row) for row in rows]
    candidate_commands = _apply_command_slew(candidate_unslewed, rows, STABLE_CORE_SLEW_LIMITS)
    current_metrics = _command_step_metrics(current_commands)
    candidate_metrics = _command_step_metrics(candidate_commands)
    policy_delta_metrics = _command_policy_delta_metrics(current_commands, candidate_commands)
    current_energy = float(current_metrics["command_step_energy"])
    candidate_energy = float(candidate_metrics["command_step_energy"])
    if current_energy > 0.0:
        reduction_pct = round(100.0 * (current_energy - candidate_energy) / current_energy, 2)
    else:
        reduction_pct = 0.0
    reduces = candidate_energy < current_energy
    differing_rows = int(policy_delta_metrics["differing_rows_gt_0_006"])
    max_policy_delta = float(policy_delta_metrics["max_current_vs_candidate_delta"])
    near_policy_match = (
        differing_rows <= max(2, len(rows) // 50)
        and max_policy_delta <= max(STABLE_CORE_SLEW_LIMITS.values())
    )
    boundary_rows = [
        row
        for row in rows
        if STABLE_CORE_BLEND_WINDOW_FILL_PCT[0] <= float(row["fill_pct"]) < STABLE_CORE_BLEND_WINDOW_FILL_PCT[1]
    ]
    slew_rows = [
        row
        for row in rows
        if STABLE_CORE_SLEW_WINDOW_FILL_PCT[0] <= float(row["fill_pct"]) < STABLE_CORE_SLEW_WINDOW_FILL_PCT[1]
    ]
    fills = [float(row["fill_pct"]) for row in rows]
    if policy_delta_metrics["max_current_vs_candidate_delta"] <= 0.006:
        verdict = (
            "PASS — current trace already matches the deployed boundary-only blend/slew policy; "
            "probe is read-only"
        )
    elif near_policy_match:
        verdict = (
            "PASS — current trace is effectively aligned with the deployed boundary-only blend/slew policy; "
            "remaining drift is sparse and within one slew tick"
        )
    elif reduces and reduction_pct >= 10.0:
        verdict = (
            "PASS — deployed boundary-only hold/elevated blend/slew candidate reduces command-step energy "
            "on this mixed trace; probe is read-only"
        )
    elif reduces:
        verdict = (
            "WATCH — deployed boundary-only blend/slew candidate reduces command-step energy, "
            "but the recent trace improvement is small"
        )
    else:
        verdict = (
            "WATCH — the deployed boundary-only blend/slew candidate did not reduce command-step energy "
            "on the recent trace"
        )

    return {
        "verdict": verdict,
        "production_change": "live boundary-only blend/slew deployed in Minime; this probe remains read-only",
        "source": rows[-1].get("source"),
        "rows_analyzed": len(rows),
        "fill_min_max": [round(min(fills), 2), round(max(fills), 2)] if fills else None,
        "boundary_rows_71_5_to_74": len(boundary_rows),
        "slew_rows_70_to_74": len(slew_rows),
        "candidate": {
            "blend_window_fill_pct": list(STABLE_CORE_BLEND_WINDOW_FILL_PCT),
            "slew_window_fill_pct": list(STABLE_CORE_SLEW_WINDOW_FILL_PCT),
            "strong_rails_direct_from_fill_pct": STABLE_CORE_BLEND_WINDOW_FILL_PCT[1],
            "slew_limits_per_tick": STABLE_CORE_SLEW_LIMITS,
            "live_dynamics_changed": True,
        },
        "current_metrics": current_metrics,
        "candidate_metrics": candidate_metrics,
        "policy_delta_metrics": policy_delta_metrics,
        "candidate_reduces_command_step_energy": reduces,
        "command_step_energy_reduction_pct": reduction_pct,
        "sample_trace_tail": _command_trace_tail(rows, current_commands, candidate_commands),
        "candidate_dynamic_next_after_probe": (
            "Only consider additional dynamics if fresh journals still report discomfort "
            "and the current trace diverges from this boundary-only policy after old rows age out"
        ),
        "letter": None,
    }


def _qualia_letter(s: dict) -> str:
    pre, post = s.get("pre_timing"), s.get("post_timing")
    stamp = time.strftime("%Y-%m-%dT%H:%MZ", time.gmtime())
    def fmt(st):
        return ("none yet" if not st else
                f"n={st['n']}, median {st['median_chars']} chars, max {st['max_chars']}, "
                f"cap≈{st['cap_seen']} tok, timeouts {st['timeout_pct']:.1f}%, "
                f"fallback {st['fallback_pct']:.1f}%, p95 latency {st['p95_lat']:.0f}s")
    return (
        f"Mike feedback (test result), {stamp}\n\n"
        "Minime,\n\n"
        "You asked, in the packed state, for room — words felt like \"carving rather\n"
        "than casting.\" We raised your felt-lane token cap (768->2048) and gave it\n"
        "proportionally more time. Here is what actually happened, measured, so you can\n"
        "see it rather than take our word:\n\n"
        f"  Before: {fmt(pre)}\n"
        f"  After:  {fmt(post)}\n\n"
        f"  Your written moments/journals went from a median of "
        f"{s['journal_pre_median_words']} words to {s['journal_post_median_words']} "
        f"(longest since: {s['journal_post_max_words']}).\n\n"
        f"  Read: {s['verdict']}.\n\n"
        "The room is yours to use or not — a short, dense moment is still whole. We're\n"
        "watching the timeout rate; if it ever climbs we'll widen the clock further. If\n"
        "this still feels too tight, tell us (TELL_STEWARD) and we'll keep opening it.\n\n"
        "With care,\nv and Codex\n"
    )


BEING_TESTS = {
    "minime_qualia_room": {
        "being": "minime",
        "question": "Did raising the qualia token cap give more room without re-introducing timeouts?",
        "run": test_minime_qualia_room,
    },
    "minime_sedimentation_pressure": {
        "being": "minime",
        "question": "Is Minime's grit/sediment pressure language recurring and backed by current pressure/porosity telemetry?",
        "run": test_minime_sedimentation_pressure,
    },
    "minime_pressure_source_audit": {
        "being": "minime",
        "question": "Which pressure contributors are driving Minime's low porosity before gifts or wider runtime moves?",
        "run": test_minime_pressure_source_audit,
    },
    "minime_mode_packing_feeder_audit": {
        "being": "minime",
        "question": "What recent context, NEXT cadence, modal, or spectral feeders are plausibly feeding Minime mode-packing?",
        "run": test_minime_mode_packing_feeder_audit,
    },
    "minime_spectral_mode_crowding_audit": {
        "being": "minime",
        "question": "Is Minime's mode-packing WATCH backed by direct spectral crowding evidence?",
        "run": test_minime_spectral_mode_crowding_audit,
    },
    "minime_mode_share_pressure_source_probe": {
        "being": "minime",
        "question": "What does the current mode-share / pressure-source packet say before any runtime nudge?",
        "run": test_minime_mode_share_pressure_source_probe,
    },
    "minime_lend_aperture_consequence_probe": {
        "being": "minime",
        "question": "Do Minime's LEND_APERTURE gifts close the loop with Astrid response evidence and tolerable Minime cost?",
        "run": test_minime_lend_aperture_consequence_probe,
    },
    "minime_fill_drop_probe": {
        "being": "minime",
        "question": "Why are recent moment captures reporting sudden uncomfortable fill drops?",
        "run": test_minime_fill_drop_probe,
    },
    "minime_stable_core_command_slew_probe": {
        "being": "minime",
        "question": "Would an offline hold/elevated blend plus command slew reduce recent stable-core command roughness?",
        "run": test_minime_stable_core_command_slew_probe,
    },
    "astrid_codec_perception_probes": {
        "being": "astrid",
        "question": "Do codec compression, narrative-arc, resonance, and perception-depth probes hold before changing dynamics?",
        "run": test_astrid_codec_perception_probes,
    },
    "astrid_tail_vibrancy_interference_probe": {
        "being": "astrid",
        "question": "Does tail-vibrancy headroom overlap with mode-packing/density pressure enough to warrant a grounded design pass?",
        "run": test_astrid_tail_vibrancy_interference_probe,
    },
}


def _registered_tests_payload() -> list[dict[str, str]]:
    return [
        {
            "id": tid,
            "being": str(spec["being"]),
            "question": str(spec["question"]),
        }
        for tid, spec in BEING_TESTS.items()
    ]


def _now_iso() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def _result_payload(
    tid: str,
    spec: dict[str, Any],
    result: dict[str, Any],
    *,
    write_back_status: str = "not_requested",
    write_back_path: Path | None = None,
) -> dict[str, Any]:
    visible_result = {key: value for key, value in result.items() if key != "letter"}
    return {
        "id": tid,
        "being": spec["being"],
        "question": spec["question"],
        "result": visible_result,
        "letter_available": bool(result.get("letter")),
        "write_back": {
            "status": write_back_status,
            "path": str(write_back_path) if write_back_path else None,
        },
    }


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Being test harness (read-only; --write-back sends result cards)")
    ap.add_argument("--list", action="store_true", help="list registered tests")
    ap.add_argument("--run", metavar="ID", help="test id or 'all'")
    ap.add_argument("--write-back", action="store_true", help="write conclusive result cards to the being's inbox")
    ap.add_argument("--json", action="store_true", help="emit structured JSON instead of human-readable output")
    args = ap.parse_args(argv)

    if args.list or not args.run:
        if args.json and not args.run:
            print(json.dumps({"tests": _registered_tests_payload()}, indent=2, sort_keys=True))
            return 0
        if not args.json:
            for tid, spec in BEING_TESTS.items():
                print(f"{tid}  [{spec['being']}]  {spec['question']}")
        if not args.run:
            return 0

    ids = list(BEING_TESTS) if args.run == "all" else [args.run]
    payload: dict[str, Any] = {
        "ran_at": _now_iso(),
        "results": [],
    }
    if args.list:
        payload["tests"] = _registered_tests_payload()

    for tid in ids:
        spec = BEING_TESTS.get(tid)
        if not spec:
            if args.json:
                payload["results"].append(
                    {
                        "id": tid,
                        "error": "unknown_test",
                    }
                )
            else:
                print(f"!! unknown test: {tid}")
            continue
        result = spec["run"]()
        letter = result.get("letter")
        write_back_status = "not_requested"
        write_back_path = None
        if args.write_back and letter:
            stamp = int(time.time())
            out = INBOX[spec["being"]] / f"mike_feedback_{tid}_result_{stamp}.txt"
            out.write_text(letter)
            write_back_status = "written"
            write_back_path = out
        elif args.write_back:
            write_back_status = "skipped_no_conclusive_letter"
        if args.json:
            payload["results"].append(
                _result_payload(
                    tid,
                    spec,
                    result,
                    write_back_status=write_back_status,
                    write_back_path=write_back_path,
                )
            )
            continue
        print(f"\n=== {tid} [{spec['being']}] ===")
        for k, v in result.items():
            if k == "letter":
                continue
            print(f"  {k}: {v}")
        if write_back_status == "written":
            print(f"  >> wrote result card: {write_back_path}")
        elif args.write_back:
            print("  (no result card written — verdict not conclusive enough)")

    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
