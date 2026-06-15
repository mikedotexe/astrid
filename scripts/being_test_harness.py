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
  python3 being_test_harness.py --run minime_qualia_room
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
from pathlib import Path

MINIME = Path("/Users/v/other/minime")
ASTRID_ROOT = Path("/Users/v/other/astrid")
ASTRID = ASTRID_ROOT / "capsules/spectral-bridge"
INBOX = {
    "minime": MINIME / "workspace" / "inbox",
    "astrid": ASTRID / "workspace" / "inbox",
}
TIMING = MINIME / "workspace" / "diagnostics" / "llm_timing.jsonl"
MINIME_JOURNAL = MINIME / "workspace" / "journal"
ASTRID_JOURNAL = ASTRID / "workspace" / "journal"
ASTRID_STATE = ASTRID / "workspace" / "state.json"
CODEC_RS = ASTRID / "src" / "codec.rs"
CODEC_PROJECTION_EPOCH = ASTRID / "workspace" / "runtime" / "codec_projection_epoch.json"

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


def _current_minime_pressure() -> dict:
    for path in (MINIME / "workspace/health.json", MINIME / "workspace/spectral_state.json"):
        try:
            data = json.loads(path.read_text())
        except Exception:
            continue
        pressure = data.get("pressure_source_v1")
        if not isinstance(pressure, dict):
            continue
        return {
            "source_file": str(path),
            "fill_pct": data.get("fill_pct"),
            "pressure_quality": pressure.get("quality"),
            "dominant_source": pressure.get("dominant_source"),
            "pressure_score": pressure.get("pressure_score"),
            "porosity_score": pressure.get("porosity_score"),
            "top_profile": [
                {
                    "source": item.get("source"),
                    "share": item.get("share"),
                    "value": item.get("value"),
                }
                for item in (pressure.get("pressure_profile") or [])[:3]
                if isinstance(item, dict)
            ],
        }
    return {}


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


def main() -> None:
    ap = argparse.ArgumentParser(description="Being test harness (read-only; --write-back sends result cards)")
    ap.add_argument("--list", action="store_true", help="list registered tests")
    ap.add_argument("--run", metavar="ID", help="test id or 'all'")
    ap.add_argument("--write-back", action="store_true", help="write conclusive result cards to the being's inbox")
    args = ap.parse_args()

    if args.list or not args.run:
        for tid, spec in BEING_TESTS.items():
            print(f"{tid}  [{spec['being']}]  {spec['question']}")
        if not args.run:
            return

    ids = list(BEING_TESTS) if args.run == "all" else [args.run]
    for tid in ids:
        spec = BEING_TESTS.get(tid)
        if not spec:
            print(f"!! unknown test: {tid}")
            continue
        result = spec["run"]()
        print(f"\n=== {tid} [{spec['being']}] ===")
        for k, v in result.items():
            if k == "letter":
                continue
            print(f"  {k}: {v}")
        letter = result.get("letter")
        if args.write_back and letter:
            stamp = int(time.time())
            out = INBOX[spec["being"]] / f"mike_feedback_{tid}_result_{stamp}.txt"
            out.write_text(letter)
            print(f"  >> wrote result card: {out}")
        elif args.write_back:
            print("  (no result card written — verdict not conclusive enough)")


if __name__ == "__main__":
    main()
