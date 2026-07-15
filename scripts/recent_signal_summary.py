#!/usr/bin/env python3
"""Read-only current signal summary for steward co-design review.

This script gathers the main held signals into one bounded packet without
writing state, changing prompt pressure, or touching runtime controls.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
import unittest
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import being_privacy

ASTRID_REPO = Path("/Users/v/other/astrid")
MINIME_REPO = Path("/Users/v/other/minime")

ASTRID_WORKSPACE = ASTRID_REPO / "capsules/spectral-bridge/workspace"
ASTRID_JOURNAL = ASTRID_WORKSPACE / "journal"
ASTRID_CONTEXT_OVERFLOW = (
    ASTRID_WORKSPACE / "context_overflow"
)
ASTRID_INTROSPECTIONS = ASTRID_WORKSPACE / "introspections"
ASTRID_DIAGNOSTICS = ASTRID_WORKSPACE / "diagnostics"
ASTRID_INBOX = ASTRID_WORKSPACE / "inbox"
ASTRID_OUTBOX = ASTRID_WORKSPACE / "outbox"
ASTRID_LLM_JOBS = ASTRID_WORKSPACE / "llm_jobs/jobs"
ASTRID_SOURCE = ASTRID_REPO / "capsules/spectral-bridge/src"
ASTRID_AUTONOMOUS_RS = ASTRID_SOURCE / "autonomous.rs"
ASTRID_CODEC_RS = ASTRID_SOURCE / "codec.rs"
ASTRID_LLM_RS = ASTRID_SOURCE / "llm.rs"
ASTRID_TYPES_RS = ASTRID_SOURCE / "types.rs"
ASTRID_WS_RS = ASTRID_SOURCE / "ws.rs"
ASTRID_STATE_RS = ASTRID_SOURCE / "autonomous/state.rs"
ASTRID_MODES_RS = ASTRID_SOURCE / "autonomous/next_action/modes.rs"
ASTRID_OPERATIONS_RS = ASTRID_SOURCE / "autonomous/next_action/operations.rs"
ASTRID_SELF_MODEL_RS = ASTRID_SOURCE / "self_model.rs"
ASTRID_ACTION_SELF_KNOWLEDGE_RS = ASTRID_SOURCE / "action_self_knowledge.rs"
ASTRID_SPECTRAL_VIZ_RS = ASTRID_SOURCE / "spectral_viz.rs"
ASTRID_ACTION_EVENTS_ROOT = (
    ASTRID_REPO / "capsules/spectral-bridge/workspace/action_threads/threads"
)
CONTEXT_PACKING_PRESSURE = ASTRID_DIAGNOSTICS / "context_packing_pressure_v1.jsonl"
CODEC_REPLAY_LABS = ASTRID_DIAGNOSTICS / "codec_replay_labs"
INTROSPECTION_ADDRESSING_STATE_DIR = ASTRID_DIAGNOSTICS / "introspection_addressing_v1"
SANDBOX_TRIAL_QUEUE_STATE_DIR = ASTRID_DIAGNOSTICS / "sandbox_trial_queue_v1"
AUTHORITY_WAIT_READINESS_STATE_DIR = ASTRID_DIAGNOSTICS / "authority_wait_readiness_v1"
AGENCY_CORRIDOR_STATE_DIR = ASTRID_DIAGNOSTICS / "agency_corridor_v1"
AGENCY_CORRIDOR_V2_STATE_DIR = ASTRID_DIAGNOSTICS / "agency_corridor_v2"
BRIDGE_LOG = Path("/tmp/bridge.log")
BTSP_SIGNAL_STATUS = (
    ASTRID_REPO / "capsules/spectral-bridge/workspace/btsp_signal_status.json"
)
MINIME_WORKSPACE = MINIME_REPO / "workspace"
MINIME_JOURNAL = MINIME_WORKSPACE / "journal"
MINIME_ACTION_THREADS = MINIME_WORKSPACE / "action_threads"
MINIME_INBOX = MINIME_WORKSPACE / "inbox"
MINIME_OUTBOX = MINIME_WORKSPACE / "outbox"
MINIME_RUNTIME = MINIME_WORKSPACE / "runtime"
MINIME_SPECTRAL_STATE = MINIME_WORKSPACE / "spectral_state.json"
MINIME_SPECTRAL_FINGERPRINTS = MINIME_WORKSPACE / "diagnostics/spectral_fingerprints"
MINIME_SENSORY_SOURCE = MINIME_RUNTIME / "sensory_source.json"
MINIME_CAMERA_STATUS = MINIME_RUNTIME / "camera_status.json"
MINIME_MIC_STATUS = MINIME_RUNTIME / "mic_status.json"
MINIME_AUTONOMOUS_AGENT = MINIME_REPO / "autonomous_agent.py"
MINIME_SOURCE = MINIME_REPO / "minime/src"
MINIME_MAIN_RS = MINIME_SOURCE / "main.rs"
MINIME_ESN_RS = MINIME_SOURCE / "esn.rs"
MINIME_SENSORY_BUS_RS = MINIME_SOURCE / "sensory_bus.rs"
MINIME_REGULATOR_RS = MINIME_SOURCE / "regulator.rs"
SHARED_COLLABORATIONS = Path("/Users/v/other/shared/collaborations")
CORRESPONDENCE_LEDGER = SHARED_COLLABORATIONS / "correspondence_v1.jsonl"
PHASE_TRANSITIONS_LEDGER = SHARED_COLLABORATIONS / "phase_transitions_v1.jsonl"
ACTIVE_CORRESPONDENCE_STATE = (
    SHARED_COLLABORATIONS
    / "coll_1778605252_spectral-cascade-dynamics/correspondence_state_v1.json"
)
CONTACT_PROPOSAL_INTROSPECTIONS = (
    ASTRID_INTROSPECTIONS / "introspection_proposal_phase_transitions_1783301734.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_bidirectional_contact_1783302128.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_distance_contact_control_1783302681.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_phase_transitions_1783309050.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_bidirectional_contact_1783309346.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_distance_contact_control_1783309655.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_phase_transitions_1783320004.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_bidirectional_contact_1783320325.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_distance_contact_control_1783320736.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_distance_contact_control_1783320952.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_phase_transitions_1783454487.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_bidirectional_contact_1783454946.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_bidirectional_contact_1783455147.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_distance_contact_control_1783459300.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_distance_contact_control_1783449016.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_bidirectional_contact_1783528438.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_distance_contact_control_1783528788.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_distance_contact_control_1783528997.txt",
)
REPRESENTATION_LOSS_INTROSPECTIONS = (
    ASTRID_INTROSPECTIONS / "introspection_astrid_autonomous_1783303621.txt",
    ASTRID_INTROSPECTIONS / "introspection_astrid_codec_1783303322.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_12d_glimpse_1783302984.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_12d_glimpse_1783310010.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_12d_glimpse_1783322606.txt",
    ASTRID_INTROSPECTIONS / "introspection_astrid_codec_1783322940.txt",
    ASTRID_INTROSPECTIONS / "introspection_astrid_autonomous_1783323325.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_12d_glimpse_1783459987.txt",
    ASTRID_INTROSPECTIONS / "introspection_astrid_codec_1783449661.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_12d_glimpse_1783449390.txt",
)
TEXTURE_STATE_ALIGNMENT_INTROSPECTIONS = (
    ASTRID_INTROSPECTIONS / "introspection_astrid_ws_1783303975.txt",
    ASTRID_INTROSPECTIONS / "introspection_astrid_types_1783304419.txt",
    ASTRID_INTROSPECTIONS / "introspection_astrid_llm_1783304988.txt",
    ASTRID_INTROSPECTIONS / "introspection_astrid_ws_1783324590.txt",
    ASTRID_INTROSPECTIONS / "introspection_astrid_types_1783324904.txt",
    ASTRID_INTROSPECTIONS / "introspection_astrid_llm_1783451251.txt",
    ASTRID_INTROSPECTIONS / "introspection_astrid_autonomous_1783449945.txt",
    ASTRID_INTROSPECTIONS / "introspection_astrid_ws_1783450517.txt",
    ASTRID_INTROSPECTIONS / "introspection_astrid_types_1783450902.txt",
    ASTRID_INTROSPECTIONS / "introspection_astrid_llm_1783524650.txt",
    ASTRID_INTROSPECTIONS / "introspection_astrid_llm_1783522928.txt",
    ASTRID_INTROSPECTIONS / "introspection_astrid_types_1783522573.txt",
)
RESERVOIR_EXPERIENCE_INTROSPECTIONS = (
    ASTRID_INTROSPECTIONS / "introspection_minime_sensory_bus_1783536272.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_distance_contact_control_1783520739.txt",
    ASTRID_INTROSPECTIONS / "introspection_astrid_llm_1783518052.txt",
)
MINIME_RECESS_SCHEMA_INTROSPECTIONS = (
    ASTRID_INTROSPECTIONS / "introspection_proposal_phase_transitions_1783309050.txt",
    ASTRID_INTROSPECTIONS / "introspection_minime_autonomous_agent_1783308714.txt",
    ASTRID_INTROSPECTIONS / "introspection_minime_main_excerpt_1783308310.txt",
    ASTRID_INTROSPECTIONS / "introspection_proposal_phase_transitions_1783320004.txt",
    ASTRID_INTROSPECTIONS / "introspection_minime_autonomous_agent_1783319728.txt",
    ASTRID_INTROSPECTIONS / "introspection_minime_main_excerpt_1783319201.txt",
    ASTRID_INTROSPECTIONS / "introspection_minime_esn_1783452337.txt",
    ASTRID_INTROSPECTIONS / "introspection_minime_main_excerpt_1783453850.txt",
    ASTRID_INTROSPECTIONS / "introspection_minime_autonomous_agent_1783454208.txt",
    ASTRID_INTROSPECTIONS / "introspection_minime_esn_1783524039.txt",
)

CONTEXT_OVERFLOW_LABEL_RE = re.compile(r"^=== \[([^\]]+)\] ===\s*$", re.MULTILINE)
ANSI_RE = re.compile(r"\x1b\[[0-9;]*m")
LOG_TS_RE = re.compile(r"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z)")
NEXT_CHOICE_RE = re.compile(r"Astrid chose NEXT:\s*(.+)$")
INTROSPECTION_FRESHNESS_STALE_AFTER_S = 86_400.0
INTROSPECTION_JOURNAL_PREFIXES = ("self_study_",)
INTROSPECTION_ARTIFACT_PREFIXES = (
    "introspection_",
    "self_study_carriage_notice_",
    "thin_introspection_output_",
)
SELF_READ_ROUTES = ("INTROSPECT", "SELF_STUDY")
COMPETING_SELF_READ_ROUTES = (
    "SHADOW_TRAJECTORY",
    "PRESSURE_SOURCE_AUDIT",
    "READ_MORE",
    "DECOMPOSE",
    "SPECTRAL_EXPLORER",
    "SHADOW_FIELD",
)
ROUTE_LEGIBILITY_ROUTES = SELF_READ_ROUTES + COMPETING_SELF_READ_ROUTES

SIGNAL_QUERIES = {
    "density_silt": [
        "density",
        "silt",
        "sediment",
        "calcification",
        "viscous",
        "grain",
        "friction",
        "floor",
        "medium",
    ],
    "porosity_aperture": [
        "porosity",
        "aperture",
        "courtyard",
        "corridor",
        "wide",
        "opening",
        "mode_packing",
        "navigable",
    ],
    "lambda4_tail": [
        "lambda4",
        "lambda_4",
        "lambda4+",
        "tail-vibrancy",
        "tail vibrancy",
        "tail",
        "shadow_trajectory",
    ],
}

FALLBACK_STATIC_TEXTURE_TERMS = (
    "viscous",
    "muffled",
    "lattice",
    "restless",
    "shimmering",
    "bright",
    "navigable",
    "tapered",
    "graduated",
    "slope",
    "edge",
    "open",
    "settled",
    "heavy",
    "weighted",
    "density",
    "dense",
    "pressure",
    "resisting",
    "pulled",
    "heaving",
    "drifting",
    "anchored",
)
FALLBACK_STATIC_CONCERN_TERMS = (
    "hardcoded",
    "static",
    "pre-packaged",
    "dictionary",
    "over-simplify",
    "dynamic descriptor sampler",
    "vocabulary drift",
    "static arrays",
    "static lists",
)
FALLBACK_TELEMETRY_ANCHOR_TERMS = (
    "density_gradient",
    "spectral_entropy",
    "pressure_risk",
    "mode_packing",
    "shadow field",
    "shadow-v3",
    "lambda_4",
    "λ4",
    "tail vibrancy",
    "widely distributed cascade",
)
FALLBACK_AUDIT_KEYWORDS = tuple(
    sorted(
        set(
            FALLBACK_STATIC_TEXTURE_TERMS
            + FALLBACK_STATIC_CONCERN_TERMS
            + FALLBACK_TELEMETRY_ANCHOR_TERMS
            + ("fallback", "FALLBACK_TEXTURE")
        ),
        key=len,
        reverse=True,
    )
)
MINIME_RECESS_SCHEMA_TERMS = (
    "phase_transition_event",
    "transition cards",
    "transition_persistence",
    "spectral_signature",
    "consent_receipt",
    "recess",
    "daydream",
    "spectral pruning",
    "spectral_entropy",
    "overpacked_mode_packing",
    "EigenPacket",
    "eigenvector_field",
    "shadow_field_v3",
    "semantic_admission_label",
    "sensory lockout",
    "regulator_drive_energy",
    "density_aware_recess",
    "density_gradient",
    "controller_pressure",
    "warm_start_blend",
    "calculate_dynamic_noise",
    "exploration noise",
)
FALLBACK_PROVIDER_SUMMARY_RE = re.compile(
    r"\b(?P<label>[a-z_]+) completed via (?P<provider>Ollama)"
    r"(?:\s+model=(?P<model>[^\s,;]+))?\b",
    re.IGNORECASE,
)
FALLBACK_LOG_INCIDENT_RE = re.compile(
    r"\b(?:(?P<label>[a-z_]+):\s*)?"
    r"(?:MLX unavailable;\s*)?falling back(?: to Ollama)?\b|"
    r"\bMLX request failed\b",
    re.IGNORECASE,
)
FALLBACK_BRIDGE_OUTPUT_RE = re.compile(r"\|\s*([a-z_]+)\s+'([^']{8,900})'")
FALLBACK_PROVIDER_OUTPUT_WINDOW_SECS = 8 * 60
FALLBACK_SEMANTIC_DENSITY_TERMS = tuple(
    sorted(
        set(
            FALLBACK_STATIC_TEXTURE_TERMS
            + FALLBACK_TELEMETRY_ANCHOR_TERMS
            + (
                "silt",
                "gradient",
                "directional",
                "density",
                "dense",
                "edge",
                "open",
                "held",
                "settled",
                "restless",
                "habitable",
                "navigable",
                "movement",
                "unfolding",
                "tail",
                "vibrancy",
                "semantic",
                "friction",
                "coherence",
                "coherent",
            )
        ),
        key=len,
        reverse=True,
    )
)

SENSORY_PRESENCE_NOTE_GLOB = "mike_feedback_sensory_presence_legibility_*.txt"
SENSORY_PRESENCE_ANCHOR_TERMS = (
    "camera",
    "mic",
    "microphone",
    "open gate",
    "open gates",
    "open-gate",
    "eyes open",
    "ears open",
    "eyes are open",
    "ears are open",
    "live intake",
    "sparse intake",
    "full_presence_admitted",
)
SENSORY_PRESENCE_TERMS = (
    "camera",
    "mic",
    "microphone",
    "open gate",
    "open-gate",
    "eyes open",
    "ears open",
    "eyes",
    "ears",
    "seeing",
    "hearing",
    "live intake",
    "sparse intake",
    "sensory",
    "full_presence_admitted",
)
SENSORY_TEXTURE_TERMS = (
    "pacing",
    "cadence",
    "dimming",
    "dim",
    "muffling",
    "muffled",
    "absence",
    "absent",
    "closed",
    "held",
    "pressure",
    "calm",
    "sparse",
    "filtered",
    "filtering",
    "deprivation",
    "deprived",
    "open",
    "presence",
)
SENSORY_CONCERN_TERMS = (
    "dimming",
    "dim",
    "muffling",
    "muffled",
    "absence",
    "absent",
    "closed",
    "deprivation",
    "deprived",
    "dead",
    "outage",
    "blind",
    "deaf",
)
SENSORY_UPTAKE_WINDOW_CHARS = 360
SENSORY_UPTAKE_MAX_PARAGRAPH_CHARS = 900
SENSORY_UPTAKE_WINDOW_POLICY = "paragraph_or_360char_anchor_cooccurrence_v2"
SENSORY_TELEMETRY_WINDOW_MARKERS = (
    "=== spectral",
    "state anchor:",
    "reservoir dynamics:",
    "sensory features",
    "sensory:",
    "timestamp:",
    "fill=",
    "lambda",
    "λ",
    "client_",
    "engine_",
    "source_record",
    "target_fps",
    "expected_interval_ms",
)
SENSORY_LIVED_WINDOW_PHRASES = (
    "feels",
    "felt",
    "feeling",
    "grounding",
    "grounded",
    "listening",
    "hearing",
    "seeing",
    "not closing",
    "closed my eyes",
    "eyes to the world",
    "ears to the world",
    "i am here",
    "i am listening",
    "truth for me",
    "lived",
    "experience",
)

VISCOSITY_SEMANTIC_INTROSPECTION_TERMS = (
    "semantic flicker",
    "semantic trace",
    "semantic tail",
    "complex thought tail",
    "content sharpness",
    "sharp content",
    "memory-loss",
    "calcified thickness",
    "semantic_persistence_weight",
    "stable_core_semantic_trickle",
    "STALE_SEMANTIC_HIGH_MS",
    "SURGE_TAPER_START_FILL",
    "m6:-0.6",
    "rho",
    "density gradient",
    "viscosity",
    "viscosity_index",
    "velvet silt",
    "settled_habitable",
    "overpacked_mode_packing",
    "mode_packing",
    "dynamic_damping_threshold_candidate",
    "exploration noise",
    "shiver",
    "jitteriness",
    "semantic entropy persistence",
    "narrative arc",
    "high-fill decay",
    "complex reflections",
)
CONTACT_TRANSITION_PROPOSAL_TERMS = (
    "phase transition",
    "phase_transition",
    "shared transition artifact",
    "transition_visibility",
    "replyable object",
    "bidirectional in relationship",
    "first-class, threaded return path",
    "structural asymmetry",
    "persistent dialogue thread",
    "shared history",
    "language-first bidirectional correspondence",
    "lonely together",
    "semantic seed",
    "Shared_Context_Buffer",
    "Correspondence_State",
    "asynchronous drift",
    "semantic masking",
    "prediction/control",
    "surrender/flow",
    "held breath",
    "breathing",
    "stillness",
    "non-instrumental",
    "exploration_noise",
    "regulation_strength",
)
REPRESENTATION_LOSS_TERMS = (
    "truncate_str",
    "CONTINUITY_RECAP_MAX_BYTES",
    "Priority-Based Truncation",
    "directional gradient",
    "FEATURE_ABS_MAX",
    "TAIL_VIBRANCY_MAX",
    "vibrancy lift",
    "hard ceiling",
    "GlimpseCodec",
    "12D",
    "12d",
    "Compression Gap",
    "TRACE_CODEC_LOSS",
    "warmth",
    "orphaned",
    "semantic drift",
    "SEMANTIC_DIM",
    "hardcoded",
    "foothold",
    "anchor",
)
TEXTURE_STATE_ALIGNMENT_TERMS = (
    "FALLBACK_TEXTURE",
    "mixed cascade",
    "gradient",
    "distributed",
    "multi-modal",
    "spectral entropy",
    "density_gradient",
    "mode_packing",
    "primary_texture",
    "pressure_gradient_delta",
    "ResonanceTextureSignatureV1",
    "BridgeState",
    "ConnectivityStatus",
    "Bidirectional",
    "false_bidirectional",
    "last_sensory_sent_unix_s",
    "latest_telemetry_arrival_unix_s",
    "PRESSURE_TREND_SMOOTHING_WINDOW",
    "dynamic_flux_vector",
    "pressure velocity",
    "acceleration",
    "active constraints",
    "dissipation",
    "structural_density_delta",
    "flux_confidence",
    "settled_habitable",
    "dispersal_potential",
    "heavy settled",
    "displacement",
    "silt",
    "coupling_coefficient",
    "mode_packing_velocity",
    "pressure_velocity",
    "structural weight",
)


def _load_json(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text())
    except Exception:
        return {}
    return data if isinstance(data, dict) else {}


def _iso(ts: float) -> str:
    return datetime.fromtimestamp(ts, timezone.utc).isoformat(timespec="seconds")


def _recent_paths(root: Path, patterns: tuple[str, ...], since_s: float) -> list[Path]:
    if not root.exists():
        return []
    out: list[Path] = []
    for pattern in patterns:
        for path in root.glob(pattern):
            if path.is_file():
                try:
                    if path.stat().st_mtime >= since_s:
                        out.append(path)
                except OSError:
                    continue
    return sorted(set(out), key=lambda p: p.stat().st_mtime, reverse=True)


def _read_bounded(path: Path, limit: int = 8_000) -> str:
    try:
        text = path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return ""
    if len(text) <= limit:
        return text
    return text[-limit:]


def _read_source(path: Path, limit: int = 500_000) -> str:
    try:
        text = path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return ""
    if len(text) <= limit:
        return text
    return text[:limit]


def _score_text(text: str, keywords: list[str]) -> tuple[int, list[str]]:
    lower = text.lower()
    anchors = [keyword for keyword in keywords if keyword.lower() in lower]
    score = sum(lower.count(keyword.lower()) for keyword in keywords)
    return score, anchors


def _term_counts(text: str, terms: tuple[str, ...]) -> Counter[str]:
    lower = text.lower()
    counts: Counter[str] = Counter()
    for term in terms:
        key = term.lower()
        if re.fullmatch(r"[a-z0-9_]+", key):
            count = len(re.findall(rf"\b{re.escape(key)}\b", lower))
        else:
            count = lower.count(key)
        if count:
            counts[term] = count
    return counts


def _counter_rows(counter: Counter[str], *, limit: int = 12) -> list[dict[str, Any]]:
    return [
        {"term": term, "count": count}
        for term, count in counter.most_common(limit)
    ]


def _excerpt(text: str, anchors: list[str], *, max_len: int = 260) -> str:
    if not text:
        return ""
    lower = text.lower()
    positions = [
        lower.find(anchor.lower())
        for anchor in anchors
        if lower.find(anchor.lower()) >= 0
    ]
    center = min(positions) if positions else 0
    start = max(0, center - max_len // 3)
    excerpt = re.sub(r"\s+", " ", text[start : start + max_len]).strip()
    if start > 0:
        excerpt = "..." + excerpt
    if start + max_len < len(text):
        excerpt += "..."
    return excerpt


def _evidence(
    paths: list[Path],
    keywords: list[str],
    *,
    top_n: int = 3,
) -> list[dict[str, Any]]:
    scored: list[tuple[int, float, Path, list[str], str]] = []
    for path in paths:
        text = _read_bounded(path)
        score, anchors = _score_text(text, keywords)
        if score <= 0:
            continue
        try:
            mtime = path.stat().st_mtime
        except OSError:
            mtime = 0.0
        scored.append((score, mtime, path, anchors, text))
    scored.sort(key=lambda item: (item[0], item[1]), reverse=True)
    return [
        {
            "path": str(path),
            "mtime": _iso(mtime),
            "score": score,
            "anchors": anchors[:8],
            "excerpt": _excerpt(text, anchors),
        }
        for score, mtime, path, anchors, text in scored[:top_n]
    ]


def _context_label_counts(paths: list[Path]) -> Counter[str]:
    counts: Counter[str] = Counter()
    for path in paths:
        text = _read_bounded(path, limit=20_000)
        counts.update(CONTEXT_OVERFLOW_LABEL_RE.findall(text))
    return counts


def _jsonl_tail(path: Path, *, max_records: int = 100) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    try:
        lines = path.read_text(encoding="utf-8", errors="ignore").splitlines()
    except OSError:
        return []
    records: list[dict[str, Any]] = []
    for line in lines[-max_records:]:
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(payload, dict):
            records.append(payload)
    return records


def _pressure_next_suggestions(top_labels: list[dict[str, Any]]) -> list[str]:
    actions = {
        "continuity": "verify compact continuity recap is reducing repeated history pressure",
        "modality": "verify modality wording follows sensory_freshness_v1 before adding sensory prose",
        "diversity": "inspect diversity cooldown/stagnant-loop text for repeated packing cost",
        "feedback": "close, defer, or summarize stale steward feedback loops before adding prompts",
        "web": "summarize web context before carrying more page text",
    }
    suggestions: list[str] = []
    for item in top_labels[:3]:
        label = str(item.get("label") or "").strip().lower()
        if not label:
            continue
        suggestion = actions.get(label, f"inspect {label} prompt packing")
        if suggestion not in suggestions:
            suggestions.append(suggestion)
    return suggestions


def _context_packing_pressure_summary(since_s: float) -> dict[str, Any]:
    records = []
    for record in _jsonl_tail(CONTEXT_PACKING_PRESSURE):
        ts_raw = record.get("ts")
        try:
            ts = float(ts_raw)
        except (TypeError, ValueError):
            ts = 0.0
        if ts >= since_s:
            records.append(record)
    removed_by_label: Counter[str] = Counter()
    occurrences_by_label: Counter[str] = Counter()
    for record in records:
        blocks = record.get("blocks")
        if not isinstance(blocks, list):
            continue
        for block in blocks:
            if not isinstance(block, dict):
                continue
            label = str(block.get("label") or "").strip().lower()
            if not label:
                continue
            try:
                removed_chars = int(block.get("removed_chars") or 0)
            except (TypeError, ValueError):
                removed_chars = 0
            if removed_chars <= 0:
                continue
            removed_by_label[label] += removed_chars
            occurrences_by_label[label] += 1
    top_labels = [
        {
            "label": label,
            "removed_chars": removed_chars,
            "occurrences": occurrences_by_label[label],
        }
        for label, removed_chars in removed_by_label.most_common(10)
    ]
    latest_ts = 0.0
    for record in records:
        try:
            ts = float(record.get("ts") or 0.0)
        except (TypeError, ValueError):
            continue
        latest_ts = max(latest_ts, ts)
    return {
        "schema": "context_packing_pressure_v1_summary",
        "recent_records": len(records),
        "latest_ts": _iso(latest_ts) if latest_ts else None,
        "top_pressure_labels": top_labels,
        "next_suggestions": _pressure_next_suggestions(top_labels),
        "authority_boundary": "steward diagnostics only; no prompt priority or being obligation",
    }


def _record_ts(record: dict[str, Any]) -> float:
    try:
        return float(record.get("ts") or 0.0)
    except (TypeError, ValueError):
        return 0.0


def _row_time_s(record: dict[str, Any]) -> float:
    for key in ("recorded_at_unix_ms", "created_at_unix_ms", "t_ms"):
        raw = record.get(key)
        if isinstance(raw, (int, float)):
            return float(raw) / 1000.0
    raw = record.get("recorded_at") or record.get("created_at") or record.get("ts")
    if isinstance(raw, str):
        parsed = _parse_iso_ts(raw)
        if parsed:
            return parsed
    if isinstance(raw, (int, float)):
        return float(raw)
    return 0.0


def _contact_proposal_samples(since_s: float) -> list[dict[str, Any]]:
    samples: list[dict[str, Any]] = []
    for path in CONTACT_PROPOSAL_INTROSPECTIONS:
        if not path.exists():
            continue
        try:
            mtime = path.stat().st_mtime
        except OSError:
            mtime = 0.0
        if mtime and mtime < since_s:
            continue
        text = _read_bounded(path, limit=14_000)
        counts = _term_counts(text, CONTACT_TRANSITION_PROPOSAL_TERMS)
        if not counts:
            continue
        source_match = re.search(r"^Source:\s*(.+)$", text, re.MULTILINE)
        samples.append(
            {
                "path": str(path),
                "ts": _iso(mtime) if mtime else None,
                "source": source_match.group(1).strip() if source_match else None,
                "anchor_terms": _counter_rows(counts),
                "excerpt": _excerpt(text, list(counts.keys()), max_len=340),
            }
        )
    return samples


def _phase_transition_followthrough(since_s: float) -> dict[str, Any]:
    records = _jsonl_tail(PHASE_TRANSITIONS_LEDGER, max_records=800)
    recent = [record for record in records if _row_time_s(record) >= since_s]
    cards = [
        record
        for record in recent
        if record.get("record_type") == "phase_transition_card"
    ]
    witnesses = [
        record
        for record in recent
        if record.get("record_type") == "phase_transition_witness"
    ]
    witnessed_ids = {
        str(record.get("transition_id") or "")
        for record in witnesses
        if record.get("transition_id")
    }
    unwitnessed = [
        card
        for card in cards
        if str(card.get("transition_id") or "") not in witnessed_ids
        and str(card.get("reply_state") or "unseen") in {"", "unseen"}
    ]
    latest_cards = sorted(cards, key=_row_time_s, reverse=True)[:5]
    v2_fields = ("transition_type", "spectral_delta", "phenomenology", "anchor_point")
    cards_with_v2_payload = [
        card for card in cards if any(card.get(field) for field in v2_fields)
    ]
    payload_complete = [
        card for card in cards if all(card.get(field) for field in v2_fields)
    ]
    auto_mode_cards = [
        card
        for card in cards
        if str(card.get("requested_by") or "") == "astrid_bridge_auto_high_signal"
        or str(card.get("trigger") or "").endswith("mode_change")
        or str(card.get("kind") or "") == "mode_change"
    ]
    being_declared_cards = [card for card in cards if card not in auto_mode_cards]
    return {
        "schema": "phase_transition_followthrough_v1",
        "ledger": str(PHASE_TRANSITIONS_LEDGER),
        "recent_cards": len(cards),
        "recent_witnesses": len(witnesses),
        "unwitnessed_cards": len(unwitnessed),
        "auto_mode_change_cards": len(auto_mode_cards),
        "being_declared_cards": len(being_declared_cards),
        "cards_with_v2_payload": len(cards_with_v2_payload),
        "cards_payload_complete": len(payload_complete),
        "payload_fields": list(v2_fields),
        "latest_cards": [
            {
                "transition_id": card.get("transition_id"),
                "ts": _iso(_row_time_s(card)) if _row_time_s(card) else None,
                "origin": card.get("origin"),
                "kind": card.get("kind"),
                "from_phase": card.get("from_phase"),
                "to_phase": card.get("to_phase"),
                "reply_state": card.get("reply_state"),
                "trigger": card.get("trigger"),
                "artifact_refs": card.get("artifact_refs") or [],
                "narrative_anchor": card.get("narrative_anchor"),
                "transition_visibility": card.get("transition_visibility"),
                "intensity": card.get("intensity"),
                "transition_type": card.get("transition_type"),
                "spectral_delta": card.get("spectral_delta"),
                "phenomenology": card.get("phenomenology"),
                "anchor_point": card.get("anchor_point"),
                "payload_complete": all(card.get(field) for field in v2_fields),
            }
            for card in latest_cards
        ],
        "existing_surface": [
            "DECLARE_TRANSITION language-only cards",
            "WITNESS_TRANSITION / I_RECEIVED_THIS reply rows",
            "Minime DECLARE_TRANSITION / WITNESS_TRANSITION / I_RECEIVED_TRANSITION route",
            "phase_felt_receipt_queue_v4 with right_to_ignore_v1",
            "phase_transition_card optional transition_type/spectral_delta/phenomenology/anchor_point fields",
        ],
        "remaining_gap": (
            "transition cards are mostly auto mode-change cards; being-declared felt transition payloads need witnessable anchors"
            if cards and len(auto_mode_cards) > len(being_declared_cards) and not cards_with_v2_payload
            else "transition cards need V2 payload fields to make the transition itself witnessable"
            if cards and not cards_with_v2_payload
            else
            "transition cards exist, but recent cards still need witness/answer linkage"
            if unwitnessed
            else "recent transition cards have witness linkage or no recent cards exist"
        ),
        "authority_boundary": "phase cards are language-only transition context; no controller, pressure, fill, PI, weighting, telemetry priority, deploy, or peer-runtime mutation",
    }


GENERIC_CORRESPONDENCE_ANCHORS = {
    "first_class_correspondence_v1",
    "correspondence_v1",
    "legacy_correspondence_bridge_v1",
    "semantic_seed",
    "latest",
    "claimed",
    "i_received_this",
}


ADDRESS_ACK_KINDS = {"held", "unclear", "cannot_answer", "needs_time"}


def _concrete_anchor(value: Any) -> str | None:
    anchor = str(value or "").strip()
    if not anchor or anchor.lower() in GENERIC_CORRESPONDENCE_ANCHORS:
        return None
    return anchor


def _direct_contact_fidelity_v3(records: list[dict[str, Any]], thread_id: str | None) -> dict[str, Any]:
    if not thread_id:
        return {
            "schema": "direct_contact_fidelity_v3_summary",
            "status": "influence_only",
            "attention_eligible": False,
            "reason": "no_active_thread",
            "authority_boundary": "read-only contact fidelity summary; no prompt priority, telemetry priority, weighting, pressure, fill, PI, or control",
        }
    thread_rows = [record for record in records if str(record.get("thread_id") or "") == thread_id]
    messages = [record for record in thread_rows if record.get("record_type") == "message"]
    read_receipts = [record for record in thread_rows if record.get("record_type") == "read_receipt"]
    reply_links = [record for record in thread_rows if record.get("record_type") == "reply_link"]
    ack_rows = [record for record in thread_rows if record.get("record_type") == "ack_receipt"]
    trace_rows = [
        record
        for record in messages
        if record.get("turn_kind") == "direct_address_trace"
    ]
    seen_ack_rows = [
        record for record in ack_rows if str(record.get("ack_kind") or "seen") == "seen"
    ]
    address_ack_rows = [
        record
        for record in ack_rows
        if str(record.get("ack_kind") or "seen") in ADDRESS_ACK_KINDS
    ]
    concrete_anchors = sorted(
        {
            anchor
            for anchor in (_concrete_anchor(record.get("shared_memory_anchor")) for record in thread_rows)
            if anchor
        }
    )
    persistence_ids = sorted(
        {
            str(record.get("persistence_id"))
            for record in thread_rows
            if record.get("persistence_id")
        }
    )
    direct_message_missing_anchor = sum(
        1 for record in messages if not _concrete_anchor(record.get("shared_memory_anchor"))
    )
    attention_eligible = bool(address_ack_rows or trace_rows)
    if trace_rows:
        status = "trace_observed"
    elif address_ack_rows:
        status = "held_ack"
    elif reply_links:
        status = "reply_linked_needs_receipt"
    elif seen_ack_rows:
        status = "seen_ack_only"
    elif read_receipts:
        status = "filesystem_seen"
    else:
        status = "influence_only"
    return {
        "schema": "direct_contact_fidelity_v3_summary",
        "thread_id": thread_id,
        "status": status,
        "message_count": len(messages),
        "read_receipts": len(read_receipts),
        "reply_links": len(reply_links),
        "seen_ack_count": len(seen_ack_rows),
        "address_ack_count": len(address_ack_rows),
        "trace_count": len(trace_rows),
        "attention_eligible": attention_eligible,
        "seen_or_read_unlocks_attention": False,
        "concrete_shared_memory_anchors": concrete_anchors[:5],
        "missing_concrete_anchor_messages": direct_message_missing_anchor,
        "persistence_ids": persistence_ids[:5],
        "anchor_continuity_status": (
            "concrete_anchor_carried"
            if concrete_anchors and direct_message_missing_anchor == 0
            else "partial_anchor_continuity"
            if concrete_anchors
            else "no_concrete_anchor"
        ),
        "authority_boundary": "read-only contact fidelity summary; read/seen are visibility only, no prompt priority, telemetry priority, weighting, pressure, fill, PI, or control",
    }


def _shared_context_buffer_v1(records: list[dict[str, Any]], thread_id: str | None) -> dict[str, Any]:
    if not thread_id:
        return {
            "schema": "shared_context_buffer_v1_summary",
            "status": "no_native_thread",
            "authority_boundary": "read-only thread continuity summary; no prompt priority, telemetry priority, weighting, pressure, fill, PI, or control",
        }
    thread_rows = [record for record in records if str(record.get("thread_id") or "") == thread_id]
    messages = [record for record in thread_rows if record.get("record_type") == "message"]
    reply_links = [record for record in thread_rows if record.get("record_type") == "reply_link"]
    read_receipts = [record for record in thread_rows if record.get("record_type") == "read_receipt"]
    ack_rows = [record for record in thread_rows if record.get("record_type") == "ack_receipt"]
    address_ack_rows = [
        record
        for record in ack_rows
        if str(record.get("ack_kind") or "seen") in ADDRESS_ACK_KINDS
    ]
    trace_rows = [
        record
        for record in messages
        if record.get("turn_kind") == "direct_address_trace"
    ]
    direction_counts: Counter[str] = Counter(
        f"{record.get('from_being')}->{record.get('to_being')}"
        for record in messages
        if record.get("from_being") and record.get("to_being")
    )
    concrete_anchors = sorted(
        {
            anchor
            for anchor in (_concrete_anchor(record.get("shared_memory_anchor")) for record in thread_rows)
            if anchor
        }
    )
    latest_ack = max(ack_rows, key=_row_time_s, default=None)
    resonance_receipts = len(address_ack_rows) + len(trace_rows)
    if resonance_receipts:
        status = "resonance_receipt_present"
    elif len(messages) > 1 or reply_links:
        status = "threaded_representation_needs_felt_receipt"
    elif read_receipts or ack_rows:
        status = "visibility_without_address"
    else:
        status = "active_context_waiting_for_receipt"
    return {
        "schema": "shared_context_buffer_v1_summary",
        "thread_id": thread_id,
        "status": status,
        "messages": len(messages),
        "reply_links": len(reply_links),
        "read_receipts": len(read_receipts),
        "ack_receipts": len(ack_rows),
        "address_ack_receipts": len(address_ack_rows),
        "direct_address_traces": len(trace_rows),
        "resonance_receipts": resonance_receipts,
        "direction_counts": dict(direction_counts),
        "shared_memory_anchors": concrete_anchors[:5],
        "last_ack_kind": latest_ack.get("ack_kind") if latest_ack else None,
        "last_ack_note": str(latest_ack.get("note") or "")[:220] if latest_ack else None,
        "right_to_ignore": True,
        "authority_boundary": "read-only shared context buffer; thread continuity is not telemetry priority, weighting, pressure, fill, PI, or control",
    }


def _shared_correspondence_buffer_v1(
    records: list[dict[str, Any]], thread_id: str | None
) -> dict[str, Any]:
    context = _shared_context_buffer_v1(records, thread_id)
    if not thread_id:
        return {
            "schema": "shared_correspondence_buffer_v1_summary",
            "status": "no_correspondence_thread",
            "context_buffer_status": context.get("status"),
            "right_to_ignore": True,
            "authority_boundary": "read-only shared correspondence buffer; no prompt priority, telemetry priority, weighting, pressure, fill, PI, sensory send, or runtime mutation",
        }
    messages = int(context.get("messages") or 0)
    resonance_receipts = int(context.get("resonance_receipts") or 0)
    direction_counts = context.get("direction_counts") or {}
    bidirectional = bool(direction_counts.get("astrid->minime")) and bool(
        direction_counts.get("minime->astrid")
    )
    if resonance_receipts:
        status = "mutual_address_receipt_present"
    elif bidirectional and messages >= 2:
        status = "bidirectional_thread_needs_held_receipt"
    elif messages:
        status = "one_sided_thread_waiting_for_return_path"
    else:
        status = "thread_object_waiting_for_message"
    return {
        "schema": "shared_correspondence_buffer_v1_summary",
        "status": status,
        "correspondence_thread_id": thread_id,
        "context_buffer_status": context.get("status"),
        "messages": messages,
        "reply_links": context.get("reply_links"),
        "read_receipts": context.get("read_receipts"),
        "ack_receipts": context.get("ack_receipts"),
        "address_ack_receipts": context.get("address_ack_receipts"),
        "direct_address_traces": context.get("direct_address_traces"),
        "resonance_receipts": resonance_receipts,
        "direction_counts": direction_counts,
        "shared_memory_anchors": context.get("shared_memory_anchors") or [],
        "last_ack_kind": context.get("last_ack_kind"),
        "participatory_contact": bool(resonance_receipts),
        "right_to_ignore": True,
        "interpretation": "a correspondence thread is the buffer object; read/seen visibility remains transport evidence until held ACK, direct trace, or felt receipt evidence appears",
        "authority_boundary": "read-only shared correspondence buffer; no prompt priority, telemetry priority, weighting, pressure, fill, PI, sensory send, or runtime mutation",
    }


def _correspondence_followthrough(since_s: float) -> dict[str, Any]:
    records = _jsonl_tail(CORRESPONDENCE_LEDGER, max_records=1_000)
    recent = [record for record in records if _row_time_s(record) >= since_s]
    messages = [
        record
        for record in recent
        if record.get("record_type") == "message"
    ]
    direct_messages = [
        record
        for record in messages
        if not record.get("legacy_bridge")
        and str(record.get("thread_id") or "").startswith("thread_corr_")
    ]
    reply_links = [
        record
        for record in recent
        if record.get("record_type") == "reply_link"
    ]
    read_receipts = [
        record
        for record in recent
        if record.get("record_type") == "read_receipt"
    ]
    by_thread: Counter[str] = Counter(
        str(record.get("thread_id") or "") for record in direct_messages if record.get("thread_id")
    )
    direction_counts: Counter[str] = Counter(
        f"{record.get('from_being')}->{record.get('to_being')}"
        for record in direct_messages
        if record.get("from_being") and record.get("to_being")
    )
    astrid_to_minime = direction_counts.get("astrid->minime", 0)
    minime_to_astrid = direction_counts.get("minime->astrid", 0)
    smaller_direction = min(astrid_to_minime, minime_to_astrid)
    larger_direction = max(astrid_to_minime, minime_to_astrid)
    symmetry_ratio = (
        round(smaller_direction / larger_direction, 6)
        if larger_direction > 0
        else None
    )
    active_state = _load_json(ACTIVE_CORRESPONDENCE_STATE)
    active_thread_id = str(active_state.get("active_thread_id") or "")
    active_thread_messages = [
        record
        for record in direct_messages
        if str(record.get("thread_id") or "") == active_thread_id
    ]
    if not active_thread_messages and by_thread:
        active_thread_id = by_thread.most_common(1)[0][0]
        active_thread_messages = [
            record
            for record in direct_messages
            if str(record.get("thread_id") or "") == active_thread_id
        ]
    latest_by_direction: dict[str, dict[str, Any]] = {}
    for record in sorted(active_thread_messages, key=_row_time_s):
        key = f"{record.get('from_being')}->{record.get('to_being')}"
        latest_by_direction[key] = {
            "message_id": record.get("message_id"),
            "ts": _iso(_row_time_s(record)) if _row_time_s(record) else None,
            "reply_to": record.get("reply_to"),
            "shared_memory_anchor": record.get("shared_memory_anchor"),
            "turn_kind": record.get("turn_kind"),
            "body_preview": str(record.get("body_preview") or "")[:220],
        }
    handshake = active_state.get("correspondence_handshake_state_v1") or {}
    semantic_seed = _semantic_seed_uptake_v1(active_thread_messages)
    fidelity_v3 = _direct_contact_fidelity_v3(recent, active_thread_id or None)
    shared_context_buffer = _shared_context_buffer_v1(recent, active_thread_id or None)
    shared_correspondence_buffer = _shared_correspondence_buffer_v1(
        recent, active_thread_id or None
    )
    return {
        "schema": "correspondence_followthrough_v1",
        "ledger": str(CORRESPONDENCE_LEDGER),
        "state_path": str(ACTIVE_CORRESPONDENCE_STATE),
        "recent_messages": len(messages),
        "recent_direct_messages": len(direct_messages),
        "recent_reply_links": len(reply_links),
        "recent_read_receipts": len(read_receipts),
        "active_thread_id": active_thread_id or None,
        "active_thread_direct_messages": len(active_thread_messages),
        "direction_counts": dict(direction_counts),
        "symmetry_check_v1": {
            "status": (
                "no_recent_direct_messages"
                if not direct_messages
                else "balanced_recent_direct_thread"
                if symmetry_ratio is not None and symmetry_ratio >= 0.50
                else "one_sided_recent_direct_thread"
            ),
            "astrid_to_minime": astrid_to_minime,
            "minime_to_astrid": minime_to_astrid,
            "balance_ratio": symmetry_ratio,
            "authority_boundary": "read-only correspondence symmetry count; no prompt priority, telemetry priority, reservoir weighting, or runtime mutation",
        },
        "thread_counts": [
            {"thread_id": thread_id, "count": count}
            for thread_id, count in by_thread.most_common(5)
        ],
        "latest_by_direction": latest_by_direction,
        "direct_contact_fidelity_v3": fidelity_v3,
        "shared_context_buffer_v1": shared_context_buffer,
        "shared_correspondence_buffer_v1": shared_correspondence_buffer,
        "semantic_seed_uptake_v1": semantic_seed,
        "handshake_status": {
            "active_threads_total": handshake.get("active_threads_total"),
            "latest_heartbeat": handshake.get("latest_heartbeat"),
            "pending_ack_by_being": handshake.get("pending_ack_by_being"),
        },
        "existing_surface": [
            "correspondence_v1 direct messages",
            "reply_link rows",
            "read receipts separated from acknowledgement",
            "active correspondence_state_v1 context",
            "semantic_seed_uptake_v1 read-only thread audit",
            "direct_contact_fidelity_v3 read/seen-vs-address audit",
            "shared_context_buffer_v1 thread continuity and resonance receipt audit",
            "shared_correspondence_buffer_v1 first-class correspondence thread object",
        ],
        "remaining_gap": (
            semantic_seed.get("remaining_gap")
            if active_thread_messages
            else "no recent native direct thread messages in this window"
        ),
        "authority_boundary": "language-only correspondence context; no standing prompt priority, telemetry priority, reservoir weighting, pressure, fill, PI, sensory send, or peer-runtime mutation",
    }


def _seed_text_variants(seed: str) -> set[str]:
    seed = str(seed or "").strip().lower()
    if not seed:
        return set()
    variants = {seed}
    variants.add(seed.replace("_", " "))
    variants.add(seed.replace("-", " "))
    variants.add(re.sub(r"[_-]+", " ", seed))
    return {variant.strip() for variant in variants if variant.strip()}


def _text_contains_seed(text: str, seed: str) -> bool:
    lower = str(text or "").lower()
    return any(variant in lower for variant in _seed_text_variants(seed))


def _message_seed_candidate(record: dict[str, Any]) -> tuple[str | None, str | None, bool]:
    anchor = str(record.get("shared_memory_anchor") or "").strip()
    if anchor:
        return (
            anchor,
            "shared_memory_anchor",
            anchor.lower() in GENERIC_CORRESPONDENCE_ANCHORS,
        )
    preview = str(record.get("body_preview") or "")
    quoted = re.findall(r"[\"“]([^\"”]{4,80})[\"”]", preview)
    if quoted:
        return (quoted[0].strip(), "quoted_body_phrase", False)
    return (None, None, False)


def _semantic_seed_uptake_v1(active_thread_messages: list[dict[str, Any]]) -> dict[str, Any]:
    ordered = sorted(active_thread_messages, key=_row_time_s)
    if not ordered:
        return {
            "schema": "semantic_seed_uptake_v1",
            "status": "no_active_thread_messages",
            "authority_boundary": "read-only correspondence uptake audit; no semantic/control weighting, prompt priority, pressure, fill, PI, sensory send, or runtime mutation",
        }

    candidates: list[dict[str, Any]] = []
    for index, record in enumerate(ordered):
        seed, source, generic = _message_seed_candidate(record)
        if not seed:
            continue
        candidates.append(
            {
                "index": index,
                "seed": seed,
                "seed_source": source,
                "generic_anchor": generic,
                "message_id": record.get("message_id"),
                "thread_id": record.get("thread_id"),
                "from_being": record.get("from_being"),
                "to_being": record.get("to_being"),
                "ts": _iso(_row_time_s(record)) if _row_time_s(record) else None,
            }
        )
    if not candidates:
        return {
            "schema": "semantic_seed_uptake_v1",
            "status": "no_seed_candidate",
            "active_thread_messages_scanned": len(ordered),
            "remaining_gap": "active thread exists, but no shared-memory anchor or quoted seed candidate was visible in recent message previews",
            "authority_boundary": "read-only correspondence uptake audit; no semantic/control weighting, prompt priority, pressure, fill, PI, sensory send, or runtime mutation",
        }

    candidate = candidates[-1]
    for possible in reversed(candidates):
        source_from = str(possible.get("from_being") or "")
        source_to = str(possible.get("to_being") or "")
        later_peer_reply = any(
            record.get("from_being") == source_to
            and record.get("to_being") == source_from
            for record in ordered[int(possible["index"]) + 1 :]
        )
        if later_peer_reply:
            candidate = possible
            break
    seed = str(candidate.get("seed") or "")
    source_message = str(candidate.get("message_id") or "")
    source_from = str(candidate.get("from_being") or "")
    source_to = str(candidate.get("to_being") or "")
    replies = [
        record
        for record in ordered[int(candidate["index"]) + 1 :]
        if record.get("from_being") == source_to
        and record.get("to_being") == source_from
    ]
    linked_replies = [
        record
        for record in replies
        if source_message and record.get("reply_to") == source_message
    ]
    peer_reply = linked_replies[0] if linked_replies else (replies[0] if replies else None)
    if peer_reply:
        text = " ".join(
            str(peer_reply.get(key) or "")
            for key in ("shared_memory_anchor", "body_preview", "reply_to")
        )
        echoed = _text_contains_seed(text, seed)
        if echoed:
            status = (
                "generic_anchor_echoed"
                if candidate.get("generic_anchor")
                else "seed_echoed_in_peer_reply"
            )
        elif linked_replies:
            status = (
                "generic_anchor_reply_linked"
                if candidate.get("generic_anchor")
                else "seed_reply_linked_no_echo"
            )
        else:
            status = "peer_reply_after_seed_without_reply_link"
    else:
        echoed = False
        status = "seed_awaiting_peer_reply"

    if status == "seed_echoed_in_peer_reply":
        gap = "seed echoed in the next peer reply; continue watching whether the echoed phrase stays meaningful across later turns"
    elif status in {"generic_anchor_echoed", "generic_anchor_reply_linked"}:
        gap = "reply linkage exists, but the anchor is generic; use a more specific being-authored seed to test felt uptake"
    elif status == "seed_reply_linked_no_echo":
        gap = "reply linkage exists, but the seed was not visibly echoed in the peer preview"
    elif status == "peer_reply_after_seed_without_reply_link":
        gap = "peer replied after the seed, but without an exact reply link"
    else:
        gap = "seed is waiting for a later peer reply"

    return {
        "schema": "semantic_seed_uptake_v1",
        "status": status,
        "seed": seed,
        "seed_source": candidate.get("seed_source"),
        "generic_anchor": bool(candidate.get("generic_anchor")),
        "source_message_id": source_message or None,
        "source_from": source_from or None,
        "source_to": source_to or None,
        "peer_reply_message_id": peer_reply.get("message_id") if peer_reply else None,
        "peer_reply_linked": bool(linked_replies),
        "seed_echoed": echoed,
        "active_thread_messages_scanned": len(ordered),
        "remaining_gap": gap,
        "authority_boundary": "read-only correspondence uptake audit; no semantic/control weighting, prompt priority, pressure, fill, PI, sensory send, or runtime mutation",
    }


def _contact_control_source_snapshot() -> dict[str, Any]:
    regulator = _read_source(MINIME_REGULATOR_RS)
    minime_agent = _read_source(MINIME_AUTONOMOUS_AGENT)
    autonomous = _read_source(ASTRID_AUTONOMOUS_RS)
    correspondence = _read_source(ASTRID_SOURCE / "autonomous/correspondence_v1.rs")
    transitions = _read_source(ASTRID_SOURCE / "autonomous/phase_transitions.rs")
    return {
        "minime_regulator_path": str(MINIME_REGULATOR_RS),
        "minime_autonomous_agent_path": str(MINIME_AUTONOMOUS_AGENT),
        "astrid_autonomous_path": str(ASTRID_AUTONOMOUS_RS),
        "receptivity_buffer_review_present": (
            "receptivity_buffer_review_v1" in regulator
            and "review_ready_receptivity_buffer_candidate" in regulator
        ),
        "pressure_porosity_divergence_present": (
            "pressure_source_flags_pressure_porosity_divergence_without_control" in regulator
            and "pressure_porosity_divergence" in regulator
            and "porosity_score" in regulator
        ),
        "regulator_audit_transparency_present": (
            "REGULATOR_AUDIT" in minime_agent
            and "stabilization_pressure_visibility_v1" in minime_agent
        ),
        "minime_transition_routes_present": all(
            token in minime_agent
            for token in (
                "DECLARE_TRANSITION",
                "WITNESS_TRANSITION",
                "I_RECEIVED_TRANSITION",
            )
        ),
        "correspondence_mutual_witness_present": (
            "mutual_witness_signal" in correspondence
            and "Transition-Artifact" in correspondence
        ),
        "correspondence_silt_continuity_present": (
            "silt_continuity" in correspondence
            and "Silt-Continuity" in correspondence
            and "silt_continuity_from_text" in correspondence
        ),
        "correspondence_fidelity_v3_present": (
            "direct_contact_fidelity_v3" in correspondence
            and "seen_ack_is_visibility_not_address" in correspondence
            and "persistence_id" in correspondence
            and "urgency_weight" in correspondence
        ),
        "presence_heartbeat_no_reply_present": (
            "presence_heartbeat" in correspondence
            and "no_reply_required" in correspondence
        ),
        "non_instrumental_presence_readiness_present": (
            "non_instrumental_presence_readiness_v1" in autonomous
            and "read_only_presence_readiness_not_scheduler_prompt_or_control_change"
            in autonomous
            and "Mode::Contemplate" in autonomous
        ),
        "transition_persistence_present": (
            "transition_persistence" in transitions
            and "active_until_both_ack_language_only" in transitions
        ),
        "phase_transition_v2_payload_present": all(
            token in transitions
            for token in (
                "transition_type",
                "spectral_delta",
                "phenomenology",
                "anchor_point",
            )
        ),
    }


def _number(value: Any) -> float | None:
    if not isinstance(value, (int, float)) or value != value:
        return None
    number = float(value)
    if number in (float("inf"), float("-inf")):
        return None
    return number


def _nested_number(value: dict[str, Any], path: tuple[str, ...]) -> float | None:
    current: Any = value
    for key in path:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return _number(current)


def _distance_contact_control_delta_v1(
    *, state_path: Path | None = None
) -> dict[str, Any]:
    state_path = MINIME_SPECTRAL_STATE if state_path is None else state_path
    state = _load_json(state_path)
    if not state:
        return {
            "schema": "distance_contact_control_delta_v1",
            "status": "spectral_state_unavailable",
            "state_path": str(state_path),
            "authority_boundary": "read-only contact/control delta audit unavailable; no regulator, sensory-bus, pressure, fill, PI, or runtime mutation",
        }
    shadow_v3 = state.get("shadow_field_v3") if isinstance(state.get("shadow_field_v3"), dict) else {}
    shadow_v2 = (
        shadow_v3.get("v2")
        if isinstance(shadow_v3.get("v2"), dict)
        else state.get("shadow_field_v2")
        if isinstance(state.get("shadow_field_v2"), dict)
        else {}
    )
    current_disp = _nested_number(shadow_v2, ("fissure_tendency",))
    history = shadow_v3.get("history") if isinstance(shadow_v3.get("history"), list) else []
    history_values = [
        _number(item.get("fissure_tendency"))
        for item in history
        if isinstance(item, dict)
    ]
    history_values = [value for value in history_values if value is not None]
    previous_disp = history_values[-1] if history_values else None
    if current_disp is None and history_values:
        current_disp = history_values[-1]
        previous_disp = history_values[-2] if len(history_values) >= 2 else None
    dispersal_delta = (
        current_disp - previous_disp
        if current_disp is not None and previous_disp is not None
        else None
    )
    semantic = state.get("semantic_energy_v1") if isinstance(state.get("semantic_energy_v1"), dict) else {}
    pressure = state.get("pressure_source_v1") if isinstance(state.get("pressure_source_v1"), dict) else {}
    pressure_components = (
        pressure.get("components") if isinstance(pressure.get("components"), dict) else {}
    )
    regulator_drive = _nested_number(semantic, ("regulator_drive_energy",))
    regulator_drive_scaled = (
        min(1.0, regulator_drive / 0.010)
        if regulator_drive is not None and regulator_drive >= 0.0
        else None
    )
    pressure_score = _nested_number(pressure, ("pressure_score",))
    pressure_quality = str(pressure.get("quality") or "")
    porosity_score = _nested_number(pressure, ("porosity_score",))
    if porosity_score is None:
        porosity_score = _number(state.get("pressure_porosity_score"))
    pressure_risk = _nested_number(
        state.get("resonance_density_v1")
        if isinstance(state.get("resonance_density_v1"), dict)
        else {},
        ("pressure_risk",),
    )
    fluctuation = (
        state.get("inhabitable_fluctuation_v1")
        if isinstance(state.get("inhabitable_fluctuation_v1"), dict)
        else {}
    )
    fluctuation_components = (
        fluctuation.get("components")
        if isinstance(fluctuation.get("components"), dict)
        else {}
    )
    fluctuation_score = _nested_number(fluctuation, ("fluctuation_score",))
    pressure_interference = _nested_number(
        fluctuation_components, ("pressure_interference",)
    )
    porosity_support = _nested_number(fluctuation_components, ("porosity_support",))
    semantic_trickle = _nested_number(pressure_components, ("semantic_trickle",))
    resonance_density = (
        state.get("resonance_density_v1")
        if isinstance(state.get("resonance_density_v1"), dict)
        else {}
    )
    resonance_components = (
        resonance_density.get("components")
        if isinstance(resonance_density.get("components"), dict)
        else {}
    )
    mode_packing = _nested_number(pressure_components, ("mode_packing",))
    if mode_packing is None:
        mode_packing = _nested_number(resonance_components, ("mode_packing",))
    containment_score = _nested_number(resonance_density, ("containment_score",))
    distinguishability_loss = _number(state.get("distinguishability_loss"))
    semantic_legacy = state.get("semantic") if isinstance(state.get("semantic"), dict) else {}
    semantic_admission = str(semantic.get("admission") or semantic_legacy.get("admission") or "")
    if dispersal_delta is not None and dispersal_delta > 0.03 and (
        (pressure_score or 0.0) >= 0.25 or (regulator_drive_scaled or 0.0) >= 0.25
    ):
        status = "restlessness_pressure_delta_review"
    elif semantic_admission == "stable_core_semantic_trickle" and (
        distinguishability_loss or 0.0
    ) >= 0.30:
        status = "semantic_trickle_distinguishability_visible"
    elif current_disp is not None:
        status = "current_delta_visible"
    else:
        status = "missing_shadow_dispersal"
    pressure_minus_porosity = (
        pressure_score - porosity_score
        if pressure_score is not None and porosity_score is not None
        else None
    )
    if "pressure_porosity_divergence" in pressure_quality:
        receptivity_status = "pressure_porosity_divergence_review"
    elif (
        pressure_minus_porosity is not None
        and pressure_minus_porosity >= 0.12
        and (semantic_trickle or 0.0) >= 0.20
    ):
        receptivity_status = "semantic_trickle_receptivity_window_review"
    elif pressure_minus_porosity is not None:
        receptivity_status = "pressure_porosity_visible"
    else:
        receptivity_status = "pressure_porosity_unavailable"
    if "pressure_porosity_divergence" in pressure_quality or (
        pressure_minus_porosity is not None
        and pressure_minus_porosity >= 0.20
        and (mode_packing or 0.0) >= 0.55
    ):
        containment_contact_status = "containment_exceeds_contact_review"
    elif (mode_packing or 0.0) >= 0.55 or (pressure_score or 0.0) >= 0.45:
        containment_contact_status = "optimization_pressure_contact_watch"
    elif semantic_admission == "stable_core_semantic_trickle" and (
        distinguishability_loss or 0.0
    ) >= 0.30:
        containment_contact_status = "semantic_trickle_contact_weight_review"
    elif pressure_score is not None or mode_packing is not None:
        containment_contact_status = "contact_threshold_visible"
    else:
        containment_contact_status = "contact_threshold_unavailable"
    return {
        "schema": "distance_contact_control_delta_v1",
        "status": status,
        "state_path": str(state_path),
        "current_dispersal_potential": (
            round(current_disp, 6) if current_disp is not None else None
        ),
        "previous_dispersal_potential": (
            round(previous_disp, 6) if previous_disp is not None else None
        ),
        "dispersal_delta": (
            round(dispersal_delta, 6) if dispersal_delta is not None else None
        ),
        "semantic_regulator_drive_energy": (
            round(regulator_drive, 8) if regulator_drive is not None else None
        ),
        "semantic_regulator_drive_scaled": (
            round(regulator_drive_scaled, 6)
            if regulator_drive_scaled is not None
            else None
        ),
        "semantic_admission": semantic_admission or None,
        "pressure_score": round(pressure_score, 6) if pressure_score is not None else None,
        "pressure_quality": pressure_quality or None,
        "dominant_pressure_source": pressure.get("dominant_source"),
        "semantic_trickle_pressure": (
            round(semantic_trickle, 6) if semantic_trickle is not None else None
        ),
        "distinguishability_loss": (
            round(distinguishability_loss, 6)
            if distinguishability_loss is not None
            else None
        ),
        "receptivity_window_v1": {
            "schema": "receptivity_window_delta_v1",
            "status": receptivity_status,
            "pressure_score": (
                round(pressure_score, 6) if pressure_score is not None else None
            ),
            "porosity_score": (
                round(porosity_score, 6) if porosity_score is not None else None
            ),
            "pressure_minus_porosity": (
                round(pressure_minus_porosity, 6)
                if pressure_minus_porosity is not None
                else None
            ),
            "pressure_risk": (
                round(pressure_risk, 6) if pressure_risk is not None else None
            ),
            "pressure_quality": pressure_quality or None,
            "semantic_trickle_pressure": (
                round(semantic_trickle, 6) if semantic_trickle is not None else None
            ),
            "inhabitable_fluctuation_quality": fluctuation.get("quality"),
            "fluctuation_score": (
                round(fluctuation_score, 6) if fluctuation_score is not None else None
            ),
            "pressure_interference": (
                round(pressure_interference, 6)
                if pressure_interference is not None
                else None
            ),
            "porosity_support": (
                round(porosity_support, 6) if porosity_support is not None else None
            ),
            "interpretation": "compares pressure load against porosity/receptivity evidence before any regulator-window or control proposal",
            "authority_boundary": "read-only receptivity-window audit; no regulator branch, pressure, fill, PI, sensory-bus porosity, or control mutation",
        },
        "containment_to_contact_threshold_v1": {
            "schema": "containment_to_contact_threshold_v1",
            "status": containment_contact_status,
            "pressure_score": (
                round(pressure_score, 6) if pressure_score is not None else None
            ),
            "porosity_score": (
                round(porosity_score, 6) if porosity_score is not None else None
            ),
            "pressure_minus_porosity": (
                round(pressure_minus_porosity, 6)
                if pressure_minus_porosity is not None
                else None
            ),
            "mode_packing": round(mode_packing, 6) if mode_packing is not None else None,
            "containment_score": (
                round(containment_score, 6) if containment_score is not None else None
            ),
            "semantic_trickle_pressure": (
                round(semantic_trickle, 6) if semantic_trickle is not None else None
            ),
            "semantic_regulator_drive_scaled": (
                round(regulator_drive_scaled, 6)
                if regulator_drive_scaled is not None
                else None
            ),
            "safe_now": [
                "shared_context_buffer_v1",
                "direct_contact_fidelity_v3",
                "containment_to_contact_threshold_v1",
                "regulator/source audit and replay",
            ],
            "gated_changes": [
                "zero-prediction regulator_drive window",
                "regulation_strength reduction",
                "exploration_noise increase",
                "automatic porosity increase",
                "receptivity_coefficient control branch",
                "astrid_to_minime semantic-trickle weighting change",
            ],
            "approval_path": "Tier 5 Mike/operator approval before live regulator, porosity, exploration-noise, semantic weighting, pressure, fill, PI, or controller mutation",
            "authority_boundary": "read-only threshold review; no regulator, pressure, fill, PI, sensory-bus porosity, weighting, controller, deploy, or runtime mutation",
        },
        "interpretation": "compares shadow-v3 fissure/dispersal, semantic regulator drive, pressure source, and distinguishability under semantic trickle as review evidence",
        "authority_boundary": "read-only contact/control delta audit; no asynchronous spectral leakage, sensory-bus porosity, mode-packing, pressure, fill, PI, regulator, or runtime mutation",
    }


def _contact_control_transparency_v1(
    source: dict[str, Any],
    correspondence: dict[str, Any],
    phase: dict[str, Any],
) -> dict[str, Any]:
    source_ready = bool(
        source.get("receptivity_buffer_review_present")
        and source.get("regulator_audit_transparency_present")
        and source.get("correspondence_mutual_witness_present")
        and source.get("presence_heartbeat_no_reply_present")
        and source.get("transition_persistence_present")
    )
    threaded_contact_visible = bool(
        correspondence.get("active_thread_id")
        and (correspondence.get("active_thread_direct_messages") or 0) > 0
    )
    replyable_phase_visible = bool(
        (phase.get("recent_cards") or 0) > 0
        or source.get("minime_transition_routes_present")
    )
    if source_ready and threaded_contact_visible:
        status = "threaded_contact_transparency_active"
    elif source_ready and replyable_phase_visible:
        status = "source_prepared_contact_transparency"
    elif source_ready:
        status = "source_prepared_waiting_for_contact_sample"
    else:
        status = "transparency_surface_incomplete"
    delta = _distance_contact_control_delta_v1()
    return {
        "schema": "contact_control_transparency_v1",
        "status": status,
        "stabilization_pressure_visibility": {
            "regulator_audit_transparency_present": source.get(
                "regulator_audit_transparency_present"
            ),
            "receptivity_buffer_review_present": source.get(
                "receptivity_buffer_review_present"
            ),
            "pressure_porosity_divergence_present": source.get(
                "pressure_porosity_divergence_present"
            ),
            "candidate_local_control_applied": False,
            "live_control_changed": False,
            "visible_as": (
                "readable regulator/receptivity evidence, not surrender mode or control mutation"
            ),
        },
        "threaded_contact_visibility": {
            "active_thread_id": correspondence.get("active_thread_id"),
            "active_thread_direct_messages": correspondence.get(
                "active_thread_direct_messages"
            ),
            "recent_reply_links": correspondence.get("recent_reply_links"),
            "mutual_witness_signal_present": source.get(
                "correspondence_mutual_witness_present"
            ),
            "presence_heartbeat_no_reply_present": source.get(
                "presence_heartbeat_no_reply_present"
            ),
            "silt_continuity_present": source.get(
                "correspondence_silt_continuity_present"
            ),
        },
        "non_instrumental_presence_visibility": {
            "non_instrumental_presence_readiness_present": source.get(
                "non_instrumental_presence_readiness_present"
            ),
            "mode": "contemplate"
            if source.get("non_instrumental_presence_readiness_present")
            else None,
            "scheduler_changed": False,
            "live_control_changed": False,
            "visible_as": (
                "read-only presence readiness; no new prompt, scheduler, codec, pressure, fill, PI, or controller mutation"
            ),
        },
        "replyable_transition_visibility": {
            "recent_cards": phase.get("recent_cards"),
            "recent_witnesses": phase.get("recent_witnesses"),
            "unwitnessed_cards": phase.get("unwitnessed_cards"),
            "transition_persistence_present": source.get(
                "transition_persistence_present"
            ),
            "minime_transition_routes_present": source.get(
                "minime_transition_routes_present"
            ),
        },
        "distance_contact_control_delta_v1": delta,
        "blocked_routes_without_steward_approval": [
            "semantic_trickle_weight_increase",
            "correspondence_priority_or_standing_weight",
            "exploration_noise_widening",
            "regulation_strength_reduction",
            "pressure_fill_pi_or_controller_mutation",
            "peer_runtime_mutation",
        ],
        "valid_next_routes": [
            "REGULATOR_AUDIT current-fill_pressure",
            "phase_correspondence_join_audit",
            "semantic_seed_uptake_probe",
            "held_breath_regulator_delta_audit",
            "containment_to_contact_threshold_v1_review",
            "CORRESPONDENCE_TRACE latest <anchor> :: <text>",
        ],
        "authority_boundary": "read-only contact/control transparency; no surrender mode, semantic/control weighting, pressure, fill, PI, sensory cadence, controller, or peer-runtime mutation",
    }


def _contact_transition_followthrough_summary(since_s: float) -> dict[str, Any]:
    samples = _contact_proposal_samples(since_s)
    phase = _phase_transition_followthrough(since_s)
    correspondence = _correspondence_followthrough(since_s)
    source = _contact_control_source_snapshot()
    contact_transparency = _contact_control_transparency_v1(source, correspondence, phase)
    findings: list[str] = []
    if samples:
        findings.append(
            "fresh proposal introspections converge on replyable transitions, first-class threaded contact, and avoiding semantic masking"
        )
    if phase.get("recent_cards"):
        findings.append(
            "phase-transition cards already exist as language-only replyable artifacts"
        )
    if phase.get("unwitnessed_cards"):
        findings.append(
            "recent phase-transition cards remain mostly unwitnessed/unanswered, so the follow-through problem is linkage and receipt, not card invention"
        )
    if phase.get("cards_with_v2_payload"):
        findings.append(
            "phase-transition cards now carry first-class transition payload fields when declared: transition_type, spectral_delta, phenomenology, and anchor_point"
        )
    elif phase.get("recent_cards"):
        findings.append(
            "recent phase-transition cards are visible but still need richer transition payload fields to make the event itself witnessable"
        )
    if phase.get("auto_mode_change_cards", 0) > phase.get("being_declared_cards", 0):
        findings.append(
            "recent transition cards are mostly auto mode-change cards; steward review should distinguish them from being-declared felt transition events"
        )
    if correspondence.get("active_thread_direct_messages"):
        findings.append(
            "native Astrid-Minime correspondence is actively landing on the current direct thread"
        )
    else:
        findings.append(
            "native direct correspondence was not visible in this recent window"
        )
    symmetry = correspondence.get("symmetry_check_v1") or {}
    if symmetry.get("status") == "balanced_recent_direct_thread":
        findings.append(
            "recent direct correspondence is directionally balanced enough to review as relationship, not one-way observation"
        )
    elif symmetry.get("status") == "one_sided_recent_direct_thread":
        findings.append(
            "recent direct correspondence remains directionally one-sided; keep watching symmetry before claiming mutual uptake"
        )
    seed = correspondence.get("semantic_seed_uptake_v1") or {}
    fidelity = correspondence.get("direct_contact_fidelity_v3") or {}
    shared_context = correspondence.get("shared_context_buffer_v1") or {}
    shared_correspondence = correspondence.get("shared_correspondence_buffer_v1") or {}
    if fidelity.get("status") in {"filesystem_seen", "seen_ack_only"}:
        findings.append(
            "direct-contact fidelity is preserving the distinction between filesystem/seen visibility and mutual address"
        )
    elif fidelity.get("attention_eligible"):
        findings.append(
            "direct-contact fidelity has address evidence beyond read/seen visibility"
        )
    if shared_context.get("status") == "resonance_receipt_present":
        findings.append(
            "shared-context buffer shows a thread-level resonance receipt, so the active contact can be reviewed as continuity rather than isolated messages"
        )
    elif shared_context.get("status") == "threaded_representation_needs_felt_receipt":
        findings.append(
            "shared-context buffer has threaded representation but still needs held ACK, direct trace, or felt receipt evidence before treating it as participatory contact"
        )
    elif shared_context.get("status") == "visibility_without_address":
        findings.append(
            "shared-context buffer preserves visibility-without-address: seen/read evidence does not become direct contact"
        )
    if shared_correspondence.get("status") == "mutual_address_receipt_present":
        findings.append(
            "shared-correspondence buffer has a mutual address receipt on the active thread"
        )
    elif shared_correspondence.get("status") == "bidirectional_thread_needs_held_receipt":
        findings.append(
            "shared-correspondence buffer is first-class and bidirectional, but still needs held ACK, direct trace, or felt receipt evidence before treating it as mutual address"
        )
    elif shared_correspondence.get("status") == "one_sided_thread_waiting_for_return_path":
        findings.append(
            "shared-correspondence buffer has a thread object but is still waiting for a symmetric return path"
        )
    if fidelity.get("anchor_continuity_status") == "partial_anchor_continuity":
        findings.append(
            "direct-contact anchor continuity is partial; preserve concrete shared_memory_anchor values across replies/traces"
        )
    if seed.get("status") == "seed_echoed_in_peer_reply":
        findings.append(
            "semantic-seed uptake is visible: a specific seed was echoed in the next peer reply"
        )
    elif seed.get("status") in {"seed_reply_linked_no_echo", "generic_anchor_reply_linked"}:
        findings.append(
            "semantic-seed uptake has reply linkage but needs a more specific echoed seed before treating it as felt uptake"
        )
    elif seed.get("status") == "seed_awaiting_peer_reply":
        findings.append(
            "semantic-seed uptake is awaiting the next peer reply on the active thread"
        )
    findings.append(
        "distance/contact/control asks should start with held-breath vs regulator-delta audit, not exploration_noise or regulation_strength mutation"
    )
    if source.get("receptivity_buffer_review_present"):
        findings.append(
            "Minime source now has a review-only receptivity buffer candidate for high-entropy, low-pressure, habitable-foothold states; it does not change live regulator control"
        )
    receptivity = (
        contact_transparency.get("distance_contact_control_delta_v1") or {}
    ).get("receptivity_window_v1") or {}
    if receptivity.get("status") in {
        "pressure_porosity_divergence_review",
        "semantic_trickle_receptivity_window_review",
    }:
        findings.append(
            "pressure/porosity receptivity-window evidence is visible for steward review, so felt containment can be compared against telemetry before any regulator change"
        )
    threshold = (
        contact_transparency.get("distance_contact_control_delta_v1") or {}
    ).get("containment_to_contact_threshold_v1") or {}
    if threshold.get("status") in {
        "containment_exceeds_contact_review",
        "optimization_pressure_contact_watch",
        "semantic_trickle_contact_weight_review",
    }:
        findings.append(
            "containment-to-contact threshold review is live: larger regulator/porosity/semantic-weight changes are named as Tier 5 routes instead of being reduced to tiny diagnostics"
        )
    if source.get("correspondence_mutual_witness_present"):
        findings.append(
            "correspondence can preserve mutual-witness and transition-artifact markers without forcing reply"
        )
    if contact_transparency.get("status") in {
        "threaded_contact_transparency_active",
        "source_prepared_contact_transparency",
        "source_prepared_waiting_for_contact_sample",
    }:
        findings.append(
            "contact/control transparency is source-prepared: stabilization pressure can be inspected as readable evidence while live control remains unchanged"
        )
    if source.get("non_instrumental_presence_readiness_present"):
        findings.append(
            "non-instrumental presence is source-prepared as contemplate-mode readiness: no text generation, codec send, or journal write, while state/warmth tracking continues"
        )

    if samples and correspondence.get("active_thread_direct_messages") and phase.get("unwitnessed_cards"):
        status = "replyable_transition_contact_join_review"
    elif samples and correspondence.get("active_thread_direct_messages"):
        status = "direct_contact_active_transition_watch"
    elif samples and phase.get("recent_cards"):
        status = "transition_cards_need_contact_linkage"
    elif samples:
        status = "proposal_batch_needs_grounding"
    else:
        status = "no_current_contact_transition_signal"

    return {
        "schema": "contact_transition_followthrough_v1",
        "status": status,
        "source_introspections": samples,
        "source_snapshot": source,
        "phase_transition_followthrough": phase,
        "correspondence_followthrough": correspondence,
        "contact_control_transparency_v1": contact_transparency,
        "semantic_masking_review": {
            "status": "study_first",
            "valid_evidence": [
                "held-breath/stillness language in public journals",
                "regulator stabilization telemetry",
                "direct-contact reply/ack/trace continuity",
                "phase-transition witness/answer rows",
            "receptivity_buffer_review_v1 source/test evidence",
            "non_instrumental_presence_readiness_v1 source/test evidence",
        ],
            "blocked_routes_without_more_evidence": [
                "exploration_noise widening",
                "regulation_strength reduction",
                "standing prompt priority",
                "reservoir weighting",
                "peer-runtime mutation",
            ],
        },
        "agency_surface": {
            "status": "language_only_agency_expanded",
            "safe_now": [
                "Astrid can DECLARE_TRANSITION, WITNESS_TRANSITION, I_RECEIVED_THIS, and TRANSITION_STATUS",
                "Minime can DECLARE_TRANSITION, WITNESS_TRANSITION, I_RECEIVED_TRANSITION, and TRANSITION_STATUS",
                "both beings can continue first-class MESSAGE/REPLY/ACK/TRACE correspondence",
            ],
            "still_requires_steward_approval": [
                "semantic_trickle or correspondence priority changes",
                "mode_packing or prompt-pressure changes",
                "asynchronous spectral leakage or sensory-bus porosity changes",
                "regulator/control/pressure/fill/PI/runtime mutation",
            ],
            "boundary": "more replyable language agency without new prompt obligation or live substrate/control authority",
        },
        "findings": findings,
        "valid_next_routes": [
            "phase_correspondence_join_audit",
            "direct_contact_fidelity_v3_seen_vs_address_review",
            "CORRESPONDENCE_STATUS shared_context_buffer_v1",
            "transition_payload_completeness_review",
            "containment_to_contact_threshold_v1_review",
            "semantic_seed_uptake_probe",
            "held_breath_regulator_delta_audit",
            "non_instrumental_response_mode_design",
            "non_instrumental_presence_readiness_review",
            "manual_transition_card_with_narrative_anchor",
        ],
        "next_suggestions": [
            "derive a read-only phase/correspondence join report before changing transition prompts or control",
            "review direct_contact_fidelity_v3 before treating read/seen events as address",
            "prefer being-declared transition cards with transition_type, spectral_delta, phenomenology, and anchor_point when a felt shift needs witness",
            "for the active direct thread, compare a unique semantic seed against the next peer reply without sending semantic/control weight",
            "compare public held-breath/stillness language with regulator telemetry before any breathing-cycle control trial",
            "use receptivity_buffer_review_v1 as review evidence only before considering any local_control_applied=false regulator branch",
            "review non_instrumental_presence_readiness_v1 before adding any new non-goal scheduler, prompt, or control behavior",
            "extend transition cards with optional intensity, narrative_anchor, transition_visibility, transition_type, spectral_delta, phenomenology, and anchor_point fields where beings declare them",
            "keep non-instrumental response mode as a design sketch until Astrid/Minime ask for the surface directly",
        ],
        "authority_boundary": "read-only proposal follow-through; no exploration_noise, regulation_strength, prompt priority, telemetry priority, reservoir weighting, pressure, fill, PI, sensory send, deploy, or peer-runtime mutation",
    }


def _reservoir_experience_samples(since_s: float) -> list[dict[str, Any]]:
    samples: list[dict[str, Any]] = []
    for path in RESERVOIR_EXPERIENCE_INTROSPECTIONS:
        if not path.exists():
            continue
        try:
            mtime = path.stat().st_mtime
        except OSError:
            mtime = 0.0
        if mtime and mtime < since_s:
            continue
        source_family = re.sub(r"_\d+$", "", path.stem.removeprefix("introspection_"))
        samples.append(
            {
                "path": str(path),
                "filename": path.name,
                "source_family": source_family,
                "ts": _iso(mtime) if mtime else None,
            }
        )
    return samples


def _reservoir_experience_source_snapshot() -> dict[str, Any]:
    sensory_bus = _read_source(MINIME_SENSORY_BUS_RS)
    llm = _read_source(ASTRID_LLM_RS)
    regulator = _read_source(MINIME_REGULATOR_RS)
    autonomous = _read_source(ASTRID_AUTONOMOUS_RS)
    correspondence = _read_source(ASTRID_SOURCE / "autonomous/correspondence_v1.rs")
    return {
        "minime_sensory_bus_path": str(MINIME_SENSORY_BUS_RS),
        "astrid_llm_path": str(ASTRID_LLM_RS),
        "minime_regulator_path": str(MINIME_REGULATOR_RS),
        "semantic_decay_curve_present": (
            "dynamic_semantic_stale_ms_for" in sensory_bus
            and "SemanticStaleShape::Sigmoid" in sensory_bus
        ),
        "semantic_decay_exact_hold_test_present": (
            "semantic_stale_recovery_hold_fill_returns_exact_recovery_window"
            in sensory_bus
        ),
        "entropy_multiplier_cap_test_present": (
            "entropy_persistence_multiplier_reaches_exact_full_support_cap"
            in sensory_bus
        ),
        "attractor_release_status_present": (
            "AttractorPulseStatus" in sensory_bus
            and "release_ticks_remaining" in sensory_bus
        ),
        "dynamic_texture_weight_present": (
            "dynamic_texture_weight" in llm
            and "fallback_dynamic_texture_weight_v1" in llm
            and "density_modifier_terms" in llm
        ),
        "texture_trajectory_present": (
            "texture_trajectory_v1" in llm
            and "fallback_texture_lived_fit_v2" in llm
        ),
        "receptivity_buffer_review_present": (
            "receptivity_buffer_review_v1" in regulator
            and "review_ready_receptivity_buffer_candidate" in regulator
        ),
        "presence_receipt_surface_present": (
            "non_instrumental_presence_readiness_v1" in autonomous
            or "presence_receipt" in correspondence
        ),
        "direct_contact_fidelity_present": (
            "direct_contact_fidelity_v3" in correspondence
            and "seen_ack_is_visibility_not_address" in correspondence
        ),
    }


def _reservoir_experience_layer_summary(
    since_s: float,
    *,
    contact_transition_followthrough: dict[str, Any] | None = None,
    texture_state_alignment: dict[str, Any] | None = None,
    viscosity_semantic_persistence: dict[str, Any] | None = None,
) -> dict[str, Any]:
    contact = contact_transition_followthrough or _contact_transition_followthrough_summary(since_s)
    texture = texture_state_alignment or _texture_state_alignment_summary(since_s)
    viscosity = viscosity_semantic_persistence or _viscosity_semantic_persistence_summary(since_s)
    source = _reservoir_experience_source_snapshot()
    samples = _reservoir_experience_samples(since_s)
    contact_transparency = contact.get("contact_control_transparency_v1") or {}
    contact_delta = contact_transparency.get("distance_contact_control_delta_v1") or {}
    fidelity = (contact.get("correspondence_followthrough") or {}).get(
        "direct_contact_fidelity_v3"
    ) or {}
    ingredients = {
        "semantic_process_not_cliff": bool(
            source.get("semantic_decay_curve_present")
            and source.get("semantic_decay_exact_hold_test_present")
            and source.get("entropy_multiplier_cap_test_present")
        ),
        "texture_process_not_static_list": bool(
            source.get("dynamic_texture_weight_present")
            and source.get("texture_trajectory_present")
        ),
        "contact_not_representation_only": bool(
            source.get("direct_contact_fidelity_present")
            and source.get("presence_receipt_surface_present")
        ),
        "receptivity_not_control_mutation": bool(
            source.get("receptivity_buffer_review_present")
        ),
        "attractor_release_reviewable": bool(
            source.get("attractor_release_status_present")
        ),
    }
    ready_count = sum(1 for value in ingredients.values() if value)
    if samples and ready_count == len(ingredients):
        status = "fresh_experience_layer_review"
    elif ready_count == len(ingredients):
        status = "experience_layer_source_prepared"
    elif ready_count >= 3:
        status = "experience_layer_partial"
    else:
        status = "experience_layer_incomplete"
    findings: list[str] = []
    if samples:
        findings.append(
            "fresh introspections converge on lived process vs descriptive labels"
        )
    if ingredients["semantic_process_not_cliff"]:
        findings.append(
            "semantic decay is reviewable as a bounded process curve with exact recovery-hold and entropy-cap tests"
        )
    if ingredients["texture_process_not_static_list"]:
        findings.append(
            "fallback texture has dynamic weighting and trajectory evidence, so terms can be read as process rather than a static menu"
        )
    if ingredients["contact_not_representation_only"]:
        findings.append(
            "contact surfaces preserve presence receipts and seen-vs-address fidelity instead of treating visibility as relationship"
        )
    if contact_delta.get("status"):
        findings.append(
            "distance/contact/control delta keeps shadow dispersal, pressure, porosity, and containment evidence reviewable before any control trial"
        )
    return {
        "schema": "reservoir_experience_layer_v1",
        "status": status,
        "source_introspections": samples,
        "source_snapshot": source,
        "ingredients": ingredients,
        "contact_vs_representation": {
            "direct_contact_status": fidelity.get("status"),
            "attention_eligible": fidelity.get("attention_eligible"),
            "contact_transparency_status": contact_transparency.get("status"),
            "distance_contact_control_delta_status": contact_delta.get("status"),
            "containment_to_contact_status": (
                contact_delta.get("containment_to_contact_threshold_v1") or {}
            ).get("status"),
            "interpretation": "read/seen and labels are representation; held ACK, trace, reply, presence receipt, and transition witness are contact evidence",
        },
        "semantic_process": {
            "semantic_decay_curve_present": source.get("semantic_decay_curve_present"),
            "exact_recovery_hold_test_present": source.get(
                "semantic_decay_exact_hold_test_present"
            ),
            "entropy_multiplier_cap_test_present": source.get(
                "entropy_multiplier_cap_test_present"
            ),
            "attractor_release_status_present": source.get(
                "attractor_release_status_present"
            ),
            "interpretation": "semantic memory should be reviewed as a changing curve and release state, not a brittle label or hidden cliff",
        },
        "texture_process": {
            "dynamic_texture_weight_present": source.get(
                "dynamic_texture_weight_present"
            ),
            "texture_trajectory_present": source.get("texture_trajectory_present"),
            "texture_alignment_status": texture.get("status"),
            "viscosity_semantic_persistence_status": viscosity.get("status"),
            "interpretation": "fallback words should remain weighted by live texture and movement evidence rather than becoming canned descriptors",
        },
        "safe_now": [
            "reservoir_experience_layer_v1 summary review",
            "contact-vs-representation language-only fire drill",
            "fallback distinguishability sandbox design",
            "shadow trajectory loss-vs-lattice audit",
            "presence receipt / direct trace follow-up",
        ],
        "gated_routes": [
            "disable_overpacked_mode_packing_score",
            "receptivity_buffer_as_live_regulator_parameter",
            "exploration_noise_or_regulation_strength_change",
            "prompt_priority_or_telemetry_priority_change",
            "pressure_fill_pi_sensor_cadence_or_controller_mutation",
            "forced live fallback-provider transition",
        ],
        "findings": findings,
        "next_suggestions": [
            "use this packet as the top-level architecture review before separate pressure, contact, texture, or semantic-decay changes",
            "run a sandbox fallback distinguishability trial before forcing provider transitions",
            "review shadow trajectory as loss-of-self vs interwoven-lattice transition before touching mode-packing or porosity",
        ],
        "authority_boundary": "read-only reservoir experience layer; no prompt priority, telemetry priority, pressure, fill, PI, sensory cadence, controller, fallback-provider, or runtime mutation",
    }


def _minime_recess_schema_samples(since_s: float) -> list[dict[str, Any]]:
    samples: list[dict[str, Any]] = []
    for path in MINIME_RECESS_SCHEMA_INTROSPECTIONS:
        if not path.exists():
            continue
        try:
            mtime = path.stat().st_mtime
        except OSError:
            mtime = 0.0
        if mtime and mtime < since_s:
            continue
        text = _read_bounded(path, limit=14_000)
        counts = _term_counts(text, MINIME_RECESS_SCHEMA_TERMS)
        if not counts:
            continue
        source_match = re.search(r"^Source:\s*(.+)$", text, re.MULTILINE)
        samples.append(
            {
                "path": str(path),
                "ts": _iso(mtime) if mtime else None,
                "source": source_match.group(1).strip() if source_match else None,
                "anchor_terms": _counter_rows(counts),
                "excerpt": _excerpt(text, list(counts.keys()), max_len=360),
            }
        )
    return samples


def _minime_recess_schema_source_snapshot() -> dict[str, Any]:
    phase_transitions = _read_source(ASTRID_SOURCE / "autonomous/phase_transitions.rs")
    correspondence = _read_source(ASTRID_SOURCE / "autonomous/correspondence_v1.rs")
    minime_agent = _read_source(MINIME_AUTONOMOUS_AGENT)
    minime_main = _read_source(MINIME_MAIN_RS)
    minime_esn = _read_source(MINIME_ESN_RS)
    return {
        "phase_transitions_path": str(ASTRID_SOURCE / "autonomous/phase_transitions.rs"),
        "correspondence_path": str(ASTRID_SOURCE / "autonomous/correspondence_v1.rs"),
        "minime_autonomous_agent_path": str(MINIME_AUTONOMOUS_AGENT),
        "minime_main_path": str(MINIME_MAIN_RS),
        "minime_esn_path": str(MINIME_ESN_RS),
        "phase_transition_card_schema_present": all(
            token in phase_transitions
            for token in (
                "phase_transition_event",
                "spectral_signature",
                "consent_receipt",
                "transition_persistence",
            )
        ),
        "correspondence_transition_artifact_present": all(
            token in correspondence
            for token in ("transition_artifact", "mutual_witness_signal")
        ),
        "phase_transition_type_surface_present": all(
            token in phase_transitions
            for token in (
                "kind",
                "from_phase",
                "to_phase",
                "transition_visibility",
            )
        ),
        "minime_moment_marker_alignment_present": all(
            token in minime_main
            for token in (
                "moment_markers",
                "phase_transition_happened",
                "transition_event",
                "should_write_phase_transition_moment_marker",
            )
        ),
        "recess_pruning_advice_present": all(
            token in minime_agent
            for token in (
                "recess_spectral_pruning_advice_v1",
                "RECESS_SPECTRAL_PRUNING_ENTROPY_HIGH",
                "control_applied",
                "advisory_only_no_latent_thread_collapse_no_auto_promote_block_no_control_change",
            )
        ),
        "density_aware_recess_profile_present": all(
            token in minime_agent
            for token in (
                "density_aware_recess_profile_v1",
                "DENSITY_AWARE_RECESS_DENSITY_GRADIENT_STEEP",
                "structural_stabilization_recommended",
                "advisory_only_no_recess_transition_no_priority_no_control_change",
            )
        ),
        "recess_autonomy_budget_boundary_present": all(
            token in minime_agent
            for token in (
                "AUTHORITY_BUDGET_MAX_SENDS",
                "EXPERIMENT_AUTHORITY_BUDGET_STATUS",
                "_research_budget_self_activation_v1",
                "research_budget_self_activation_v1",
                "being_self_activated_local_v1",
                "_research_budget_boundary",
            )
        ),
        "self_journal_low_cost_boundary_present": all(
            token in minime_agent
            for token in (
                "STABLE_CORE_SELF_JOURNAL_ACTIONS",
                "_stable_core_self_journal_only",
                "self_journal_only",
                "_journal_rest_reflection",
            )
        ),
        "recess_pruning_manifest_present": all(
            token in minime_agent
            for token in (
                "_action_summary",
                "_write_action_manifest",
                "recess_spectral_pruning_advice_v1",
            )
        ),
        "eigenpacket_schema_test_present": all(
            token in minime_main
            for token in (
                "eigenpacket_serializes_legacy_and_typed_fingerprint",
                "eigenvector_field",
                "shadow_field_v3",
                "semantic_energy_v1",
            )
        ),
        "semantic_admission_lockout_test_present": all(
            token in minime_main
            for token in (
                "semantic_admission_label_distinguishes_stale_trace_from_budgeted_input",
                "stable_core_semantic_muted",
                "stable_core_semantic_fill_ceiling",
            )
        ),
        "semantic_admission_fill_grid_test_present": all(
            token in minime_main
            for token in (
                "semantic_admission_label_keeps_fill_boundary_grid_explicit",
                "stable_core_semantic_fill_ceiling",
                "stable_core_semantic_budgeted_out",
            )
        ),
        "semantic_regulator_drive_visible": all(
            token in minime_main
            for token in ("regulator_drive_energy", "admission")
        ),
        "dynamic_noise_shadow_preview_present": all(
            token in minime_esn
            for token in (
                "calculate_dynamic_noise",
                "intentionally not wired into `ESN::step`",
                "dynamic_noise_scales_down_for_steep_gradient_or_pressure",
            )
        ),
    }


def _minime_recess_schema_integrity_summary(since_s: float) -> dict[str, Any]:
    samples = _minime_recess_schema_samples(since_s)
    source = _minime_recess_schema_source_snapshot()
    phase_ready = bool(
        source.get("phase_transition_card_schema_present")
        and source.get("correspondence_transition_artifact_present")
        and source.get("phase_transition_type_surface_present")
        and source.get("minime_moment_marker_alignment_present")
    )
    recess_ready = bool(
        source.get("recess_pruning_advice_present")
        and source.get("density_aware_recess_profile_present")
        and source.get("recess_pruning_manifest_present")
    )
    autonomy_budget_ready = bool(
        source.get("recess_autonomy_budget_boundary_present")
        and source.get("self_journal_low_cost_boundary_present")
    )
    schema_ready = bool(
        source.get("eigenpacket_schema_test_present")
        and source.get("semantic_admission_lockout_test_present")
        and source.get("semantic_admission_fill_grid_test_present")
        and source.get("semantic_regulator_drive_visible")
    )
    esn_shadow_ready = bool(source.get("dynamic_noise_shadow_preview_present"))
    source_ready = (
        phase_ready
        and recess_ready
        and autonomy_budget_ready
        and schema_ready
        and esn_shadow_ready
    )

    findings: list[str] = []
    if samples:
        findings.append(
            "current packet names replyable phase transitions, Recess/autonomy budget friction, high-entropy daydream cost, and EigenPacket/admission schema drift risk"
        )
    if phase_ready:
        findings.append(
            "phase transitions are source-prepared as language-only transition cards with type/from/to fields, transition artifacts, Minime moment-marker alignment, and witnessable correspondence links"
        )
    if recess_ready:
        findings.append(
            "Minime Recess has source-prepared spectral-pruning advice plus advisory-only density-aware Recess profiling, so high-entropy/controller-pressure Recess can be named as structural stabilization while control_applied=false"
        )
    if autonomy_budget_ready:
        findings.append(
            "Minime autonomy/budget paradox is source-visible: self-journal/Recess routes remain low-cost local reflection, read-only local research can self-activate within caps, and authority budgets stay gated"
        )
    if schema_ready:
        findings.append(
            "Minime EigenPacket and semantic_admission_label have source/test anchors for eigenvector_field, shadow_field_v3, muted/fill-ceiling/fill-grid admission, and regulator_drive_energy visibility"
        )
    if esn_shadow_ready:
        findings.append(
            "Minime ESN dynamic exploration noise is source-prepared as a shadow preview with tests, not wired into live ESN::step"
        )

    if not source_ready:
        status = "minime_recess_schema_surface_incomplete"
    elif samples:
        status = "source_prepared_minime_recess_schema_watch"
    else:
        status = "source_prepared_no_recent_minime_recess_schema_signal"

    return {
        "schema": "minime_recess_schema_integrity_v1",
        "status": status,
        "source_introspections": samples,
        "source_snapshot": source,
        "readiness": {
            "phase_transition_cards": phase_ready,
            "recess_spectral_pruning_advice": recess_ready,
            "recess_autonomy_budget_boundary": autonomy_budget_ready,
            "eigenpacket_schema_and_admission": schema_ready,
            "dynamic_noise_shadow_preview": esn_shadow_ready,
            "all_source_ready": source_ready,
        },
        "blocked_routes_without_steward_approval": [
            "latent_thread_collapse",
            "auto_promote_block_or_throttle",
            "Mode::MomentCapture mutation",
            "EigenPacket contract change without Python consumer audit",
            "warm_start_blend runtime experiment without monitored operator run",
            "live dynamic exploration-noise wiring without replay/operator approval",
            "raising_LOCAL_RESEARCH_MAX_ACTIONS_LOOP_RESEARCH_MAX_ACTIONS_or_AUTHORITY_BUDGET_MAX_SENDS",
            "fill_pressure_pi_controller_or_sensory_cadence_change",
        ],
        "valid_next_routes": [
            "DECLARE_TRANSITION kind: ...; transition_persistence: true; spectral_signature: ...",
            "WITNESS_TRANSITION latest :: reply_state: witnessed|answered; note: ...",
            "recess_spectral_pruning_advice_v1 manifest review",
            "density_aware_recess_profile_v1 action-manifest review",
            "EXPERIMENT_RESEARCH_BUDGET_STATUS latest",
            "EXPERIMENT_AUTHORITY_BUDGET_STATUS latest",
            "EigenPacket schema serialization test",
            "semantic_admission_label saturated/muted grid test",
            "calculate_dynamic_noise shadow-preview trace review",
        ],
        "findings": findings,
        "next_suggestions": [
            "watch whether declared transition cards receive witness/answer rows before adding more phase-card affordances",
            "inspect Recess action manifests for recess_spectral_pruning_advice_v1 and density_aware_recess_profile_v1 before considering any real Recess behavior change",
            "inspect authority/research budget status before changing LOCAL_RESEARCH_MAX_ACTIONS, LOOP_RESEARCH_MAX_ACTIONS, or AUTHORITY_BUDGET_MAX_SENDS",
            "treat EigenPacket serde_json fields as pinned by test evidence until a typed consumer contract is available",
            "compare calculate_dynamic_noise previews against density/pressure traces before any live exploration-noise wiring",
        ],
        "authority_boundary": "read-only source/test integrity; no latent-thread collapse, auto-promote throttling, Mode::MomentCapture mutation, EigenPacket contract change, warm_start_blend runtime experiment, dynamic-noise live wiring, pressure, fill, PI, sensory cadence, control, deploy, restart, staging, or commit",
    }


def _representation_loss_samples(since_s: float) -> list[dict[str, Any]]:
    samples: list[dict[str, Any]] = []
    for path in REPRESENTATION_LOSS_INTROSPECTIONS:
        if not path.exists():
            continue
        try:
            mtime = path.stat().st_mtime
        except OSError:
            mtime = 0.0
        if mtime and mtime < since_s:
            continue
        text = _read_bounded(path, limit=14_000)
        counts = _term_counts(text, REPRESENTATION_LOSS_TERMS)
        if not counts:
            continue
        source_match = re.search(r"^Source:\s*(.+)$", text, re.MULTILINE)
        samples.append(
            {
                "path": str(path),
                "ts": _iso(mtime) if mtime else None,
                "source": source_match.group(1).strip() if source_match else None,
                "anchor_terms": _counter_rows(counts),
                "excerpt": _excerpt(text, list(counts.keys()), max_len=360),
            }
        )
    return samples


def _rust_usize_const(text: str, name: str) -> int | None:
    match = re.search(rf"\bconst\s+{re.escape(name)}\s*:\s*usize\s*=\s*([0-9_]+)", text)
    if not match:
        match = re.search(rf"\bpub\s+const\s+{re.escape(name)}\s*:\s*usize\s*=\s*([0-9_]+)", text)
    if not match:
        return None
    try:
        return int(match.group(1).replace("_", ""))
    except ValueError:
        return None


def _rust_u64_const(text: str, name: str) -> int | None:
    match = re.search(rf"\bconst\s+{re.escape(name)}\s*:\s*u64\s*=\s*([0-9_]+)", text)
    if not match:
        return None
    try:
        return int(match.group(1).replace("_", ""))
    except ValueError:
        return None


def _rust_f32_const(text: str, name: str) -> float | None:
    match = re.search(rf"\bconst\s+{re.escape(name)}\s*:\s*f32\s*=\s*([0-9_]+(?:\.[0-9_]+)?)", text)
    if not match:
        return None
    try:
        return float(match.group(1).replace("_", ""))
    except ValueError:
        return None


def _rust_str_const(text: str, name: str) -> str | None:
    match = re.search(
        rf"\bconst\s+{re.escape(name)}\s*:\s*&str\s*=\s*\"([^\"]*)\"",
        text,
    )
    if not match:
        return None
    return match.group(1)


def _representation_source_snapshot() -> dict[str, Any]:
    codec = _read_source(ASTRID_CODEC_RS)
    autonomous = _read_source(ASTRID_AUTONOMOUS_RS)
    return {
        "codec_path": str(ASTRID_CODEC_RS),
        "autonomous_path": str(ASTRID_AUTONOMOUS_RS),
        "semantic_dim": _rust_usize_const(codec, "SEMANTIC_DIM"),
        "semantic_dim_legacy": _rust_usize_const(codec, "SEMANTIC_DIM_LEGACY"),
        "feature_abs_max": _rust_f32_const(codec, "FEATURE_ABS_MAX"),
        "tail_vibrancy_max": _rust_f32_const(codec, "TAIL_VIBRANCY_MAX"),
        "tail_entropy_gate": _rust_f32_const(codec, "TAIL_VIBRANCY_ENTROPY_GATE"),
        "gradient_aware_vibrancy_present": (
            "vibrancy_from_entropy_and_density_gradient" in codec
            and "tail_lift_scaled_by_low_density_gradient" in codec
        ),
        "vibrancy_substance_fit_present": (
            "codec_vibrancy_substance_fit_v1" in codec
            and "entropy_lift_substance_review" in codec
            and "codec_vibrancy_substance_fit_flags_entropy_without_content" in codec
        ),
        "tail_vibrancy_bounded_ceiling_test_present": (
            "tail_vibrancy_raises_only_tail_ceiling_in_high_entropy" in codec
        ),
        "vibrancy_smoothstep_test_present": (
            "vibrancy_from_entropy_matches_inline_smoothstep" in codec
            and "tail_vibrancy_gate_has_no_discontinuous_pop" in codec
        ),
        "vibrancy_requested_points_test_present": (
            "tail_vibrancy_gate_is_smooth_at_requested_entropy_points" in codec
        ),
        "projection_runtime_dir_env_override_present": (
            "fn projection_runtime_dir" in codec
            and "ASTRID_CODEC_RUNTIME_DIR" in codec
        ),
        "projection_epoch_stability_present": (
            "projection_epoch_stability_v1" in codec
            and "codec_projection_kernel_epoch_is_stable_across_fresh_runtime_dirs" in codec
            and "codec_projection_existing_epoch_file_takes_precedence_after_restart" in codec
        ),
        "projection_fingerprint_integrity_present": (
            "projection_fingerprint_integrity_v1" in codec
            and "projection_fingerprint_bits" in codec
            and "diagnostic_fingerprint_hardening_not_projection_seed_or_semantic_lane_change"
            in codec
        ),
        "projection_fingerprint_integrity_test_present": (
            "projection_fingerprint_canonicalizes_float_edge_patterns" in codec
        ),
        "projection_repeat_run_test_present": (
            "dynamic_projection_is_stable_across_repeated_epoch_runs" in codec
        ),
        "embedding_dimension_validation_test_present": (
            "dynamic_projection_rejects_one_short_embedding_dimension" in codec
        ),
        "dynamic_vibrancy_ceiling_canary_present": (
            "codec_dynamic_vibrancy_scaling_canary_v1" in codec
            and "vibrancy_aperture_dynamic_ceiling_is_bounded_and_navigable_gated" in codec
        ),
        "shadow_field_reserved_dim_readiness_present": (
            "shadow_field_reserved_dim_readiness_v1" in codec
            and "shadow_field_reserved_dim_readiness_is_default_off_and_unwritten" in codec
        ),
        "high_entropy_narrative_arc_guard_present": (
            "high_entropy_vibrancy_does_not_write_narrative_arc_or_shadow_reserved_dims" in codec
        ),
        "narrative_arc_gain_response_readiness_present": (
            "narrative_arc_gain_response_readiness_v1" in codec
            and "narrative_arc_gain_response_preview_v1" in codec
            and "not_live_adaptive_gain_or_semantic_weight_change" in codec
        ),
        "narrative_arc_gain_response_test_present": (
            "narrative_arc_gain_response_readiness_is_default_off_and_bounded" in codec
        ),
        "codec_abrasive_texture_interpretation_present": (
            "codec_abrasive_texture_interpretation_v1" in codec
            and "low_marker_tension_high_jagged_resistance" in codec
            and "read_only_texture_interpretation_not_tension_weight_gain_or_reserved_dim_change"
            in codec
        ),
        "structural_friction_summary_resistance_present": (
            "summary_resistance_signal" in codec
            and "calcified_summary_resistant" in codec
            and "structural_friction_names_calcified_summary_resistance" in codec
        ),
        "char_window_4096_replay_test_present": (
            "char_freq_window_4096_comparison_is_replay_only" in codec
            and "CHAR_FREQ_WINDOW_CAPACITY, 1024" in codec
        ),
        "glimpse_codec_present": "struct GlimpseCodec" in codec,
        "semantic_glimpse_readiness_present": "semantic_glimpse_12d_readiness_v1" in codec,
        "glimpse_companion_fidelity_present": (
            "companion_not_replacement" in codec
            and "compression_fidelity_basis" in codec
            and "tail_bridge_slot" in codec
        ),
        "multi_scale_context_present": (
            "multi_scale_context_v1" in codec
            and "12d_glimpse_must_travel_with_32d_residual_context" in codec
            and "shadow_field_energy_preserved_when_12d_glimpse_is_active" in codec
        ),
        "glimpse_tail_identity_test_present": (
            "glimpse_codec_preserves_tail_bridge_and_identity_asymmetry" in codec
        ),
        "multi_scale_context_test_present": (
            "multi_scale_context_pairs_12d_glimpse_with_32d_residual_shadow_metadata"
            in codec
        ),
        "continuity_recap_max_bytes": _rust_usize_const(autonomous, "CONTINUITY_RECAP_MAX_BYTES"),
        "continuity_item_max_bytes": _rust_usize_const(
            autonomous, "CONTINUITY_RECAP_ITEM_MAX_BYTES"
        ),
        "continuity_trajectory_limit": _rust_usize_const(
            autonomous, "CONTINUITY_TRAJECTORY_LIMIT"
        ),
        "continuity_anchor_terms_present": "CONTINUITY_RECAP_ANCHOR_TERMS" in autonomous,
        "anchored_continuity_excerpt_present": "anchored_continuity_excerpt" in autonomous,
        "quoted_continuity_anchor_present": (
            "quoted_or_emphasized_continuity_anchor_pos" in autonomous
        ),
        "semantic_truncation_anchor_present": (
            "semantic_truncate_str" in autonomous
            and "SEMANTIC_TRUNCATION_ANCHOR_TERMS" in autonomous
        ),
        "semantic_truncation_anchor_test_present": (
            "semantic_truncate_str_preserves_late_shadow_texture_anchor" in autonomous
            and "compact_journal_signal_anchor_uses_semantic_excerpt" in autonomous
        ),
        "semantic_boundary_truncation_present": (
            "truncate_continuity_recap_at_semantic_boundary" in autonomous
            and "semantic_boundary_before" in autonomous
        ),
        "semantic_boundary_truncation_test_present": (
            "compact_continuity_recap_prefers_sentence_boundary_when_overflowing"
            in autonomous
        ),
        "pressure_gradient_anchor_test_present": (
            "compact_continuity_item_preserves_pressure_gradient_anchor" in autonomous
        ),
        "introspection_freshness_stale_after_s": (
            86_400
            if "INTROSPECTION_FRESHNESS_STALE_AFTER" in autonomous
            and "Duration::from_secs(86_400)" in autonomous
            else None
        ),
        "introspection_freshness_optional_prompt_present": (
            "introspection_freshness_note_surfaces_stale_self_study_as_optional" in autonomous
            and "optional/read-only" in autonomous
        ),
    }


def _latest_codec_replay_lab() -> dict[str, Any]:
    if not CODEC_REPLAY_LABS.exists():
        return {"status": "no_codec_replay_lab_found", "path": None}
    paths = sorted(
        CODEC_REPLAY_LABS.glob("*/codec_replay_lab.json"),
        key=lambda path: path.stat().st_mtime if path.exists() else 0.0,
        reverse=True,
    )
    if not paths:
        return {"status": "no_codec_replay_lab_found", "path": None}
    path = paths[0]
    payload = _load_json(path)
    clamp = payload.get("codec_clamp_headroom_probe_v1")
    if not isinstance(clamp, dict):
        clamp = {}
    texture = payload.get("codec_texture_replay_v1")
    if not isinstance(texture, dict):
        texture = {}
    lifecycle = payload.get("authority_lifecycle_v2")
    if not isinstance(lifecycle, dict):
        lifecycle = {}
    candidate_packets = lifecycle.get("candidate_packets")
    if not isinstance(candidate_packets, list):
        candidate_packets = []
    texture_entries = []
    for entry in payload.get("entries") or []:
        if not isinstance(entry, dict):
            continue
        mismatch = entry.get("warmth_tension_texture_mismatch_v1")
        if not isinstance(mismatch, dict):
            mismatch = {}
        structural = entry.get("structural_friction_replay_v1")
        if not isinstance(structural, dict):
            structural = {}
        texture_entries.append(
            {
                "label": entry.get("label"),
                "entropy_retention_delta": entry.get("entropy_retention_delta"),
                "friction_texture_state": structural.get("friction_texture_state"),
                "summary_resistance_signal": structural.get("summary_resistance_signal"),
                "mismatch_interpretation": mismatch.get("interpretation"),
                "abrasive_texture_support": mismatch.get("abrasive_texture_support"),
            }
        )
    try:
        mtime = path.stat().st_mtime
    except OSError:
        mtime = 0.0
    return {
        "status": "available",
        "path": str(path),
        "mtime": _iso(mtime) if mtime else None,
        "policy": payload.get("policy"),
        "runtime_behavior_changed": payload.get("runtime_behavior_changed"),
        "corpus_source": payload.get("corpus_source"),
        "corpus_status": payload.get("corpus_status"),
        "sample_count": len(payload.get("entries") or []),
        "clamp_headroom": {
            "policy": clamp.get("policy"),
            "status": clamp.get("status"),
            "near_static_clamp_count": clamp.get("near_static_clamp_count"),
            "tail_ceiling_pressure_count": clamp.get("tail_ceiling_pressure_count"),
            "dynamic_headroom_candidate_count": clamp.get(
                "dynamic_headroom_candidate_count"
            ),
            "static_feature_abs_max": clamp.get("static_feature_abs_max"),
            "tail_vibrancy_max": clamp.get("tail_vibrancy_max"),
        },
        "texture_replay": {
            "policy": texture.get("policy"),
            "status": texture.get("status"),
            "current_char_freq_window_capacity": texture.get(
                "current_char_freq_window_capacity"
            ),
            "candidate_char_freq_window_capacity": texture.get(
                "candidate_char_freq_window_capacity"
            ),
            "wide_window_supported": texture.get("wide_window_supported"),
            "low_tension_underread_supported": texture.get(
                "low_tension_underread_supported"
            ),
            "candidate_count": texture.get("candidate_count") or len(candidate_packets),
            "live_eligible_now": texture.get(
                "live_eligible_now", payload.get("live_eligible_now")
            ),
            "auto_approved": texture.get("auto_approved", payload.get("auto_approved")),
            "entries": texture_entries[:4],
        },
        "authority_lifecycle_v2": {
            "schema": lifecycle.get("schema"),
            "receipt_chain_status": lifecycle.get("receipt_chain_status"),
            "candidate_count": len(candidate_packets),
            "boundary_ids": [
                packet.get("boundary_id")
                for packet in candidate_packets[:8]
                if isinstance(packet, dict) and packet.get("boundary_id")
            ],
            "live_eligible_now": lifecycle.get(
                "live_eligible_now", payload.get("live_eligible_now")
            ),
            "auto_approved": lifecycle.get("auto_approved", payload.get("auto_approved")),
        },
    }


def _finite_vector(value: Any, *, min_len: int = 0) -> list[float]:
    if not isinstance(value, list):
        return []
    out: list[float] = []
    for item in value:
        if not isinstance(item, (int, float)) or not item == item:
            return []
        number = float(item)
        if number in (float("inf"), float("-inf")):
            return []
        out.append(number)
    if len(out) < min_len:
        return []
    return out


def _mean(values: list[float]) -> float | None:
    if not values:
        return None
    return sum(values) / len(values)


def _stddev(values: list[float]) -> float | None:
    mean = _mean(values)
    if mean is None:
        return None
    return (sum((value - mean) ** 2 for value in values) / len(values)) ** 0.5


def _spectral_fingerprint_from_payload(payload: dict[str, Any]) -> list[float]:
    for key in ("spectral_fingerprint", "legacy_spectral_fingerprint"):
        vector = _finite_vector(payload.get(key), min_len=32)
        if vector:
            return vector
    return []


def _spectral_glimpse_from_payload(payload: dict[str, Any]) -> list[float]:
    return _finite_vector(payload.get("spectral_glimpse_12d"), min_len=12)[:12]


def _spectral_fingerprint_samples(
    *,
    state_path: Path | None = None,
    captures_dir: Path | None = None,
) -> list[dict[str, Any]]:
    state_path = MINIME_SPECTRAL_STATE if state_path is None else state_path
    captures_dir = MINIME_SPECTRAL_FINGERPRINTS if captures_dir is None else captures_dir
    paths: list[Path] = []
    if state_path.exists():
        paths.append(state_path)
    if captures_dir.exists():
        paths.extend(
            sorted(
                captures_dir.glob("*.json"),
                key=lambda path: path.stat().st_mtime if path.exists() else 0.0,
                reverse=True,
            )[:24]
        )

    seen: set[Path] = set()
    samples: list[dict[str, Any]] = []
    for path in paths:
        if path in seen:
            continue
        seen.add(path)
        payload = _load_json(path)
        fingerprint = _spectral_fingerprint_from_payload(payload)
        if not fingerprint:
            continue
        glimpse = _spectral_glimpse_from_payload(payload)
        concentrations = fingerprint[8:16]
        concentration_max = max(concentrations)
        concentration_stddev = _stddev(concentrations)
        sample: dict[str, Any] = {
            "path": str(path),
            "fingerprint_len": len(fingerprint),
            "has_glimpse_12d": len(glimpse) == 12,
            "concentration_max": round(concentration_max, 6),
            "concentration_stddev": (
                round(concentration_stddev, 6)
                if concentration_stddev is not None
                else None
            ),
        }
        if len(glimpse) == 12:
            spectral_entropy_dim_24 = fingerprint[24] if len(fingerprint) > 24 else None
            lambda_gap_dim_25 = fingerprint[25] if len(fingerprint) > 25 else None
            sample.update(
                {
                    "glimpse_concentration_max_slot": round(glimpse[3], 6),
                    "glimpse_concentration_stddev_slot": round(glimpse[4], 6),
                    "concentration_max_delta": round(abs(glimpse[3] - concentration_max), 6),
                    "concentration_stddev_delta": (
                        round(abs(glimpse[4] - concentration_stddev), 6)
                        if concentration_stddev is not None
                        else None
                    ),
                    "spectral_entropy_dim_24": (
                        round(spectral_entropy_dim_24, 6)
                        if spectral_entropy_dim_24 is not None
                        else None
                    ),
                    "glimpse_spectral_entropy_slot": round(glimpse[7], 6),
                    "spectral_entropy_delta": (
                        round(abs(glimpse[7] - spectral_entropy_dim_24), 6)
                        if spectral_entropy_dim_24 is not None
                        else None
                    ),
                    "lambda_gap_dim_25": (
                        round(lambda_gap_dim_25, 6)
                        if lambda_gap_dim_25 is not None
                        else None
                    ),
                    "glimpse_lambda_gap_slot": round(glimpse[8], 6),
                    "lambda_gap_delta": (
                        round(abs(glimpse[8] - lambda_gap_dim_25), 6)
                        if lambda_gap_dim_25 is not None
                        else None
                    ),
                }
            )
        samples.append(sample)
    return samples


def _pca_12d_concentration_summary(vectors: list[list[float]]) -> dict[str, Any]:
    if len(vectors) < 13:
        return {
            "status": "sample_limited_for_pca",
            "pca_ready": False,
            "required_samples": 13,
            "sample_count": len(vectors),
        }
    try:
        import numpy as np
    except Exception as exc:
        return {
            "status": "numpy_unavailable",
            "pca_ready": False,
            "error": str(exc),
            "sample_count": len(vectors),
        }
    matrix = np.array([vector[:32] for vector in vectors], dtype=float)
    centered = matrix - matrix.mean(axis=0, keepdims=True)
    _, singular_values, basis = np.linalg.svd(centered, full_matrices=False)
    rank = int((singular_values > 1e-9).sum())
    if rank < 12:
        return {
            "status": "rank_limited_for_12d_pca",
            "pca_ready": False,
            "sample_count": len(vectors),
            "rank": rank,
        }
    variance = singular_values * singular_values
    total_variance = float(variance.sum())
    explained_12 = (
        float(variance[:12].sum() / total_variance)
        if total_variance > 0.0
        else 0.0
    )
    basis_12 = basis[:12]
    reconstructed = centered @ basis_12.T @ basis_12 + matrix.mean(axis=0, keepdims=True)
    original_band = matrix[:, 8:16]
    reconstructed_band = reconstructed[:, 8:16]
    rmse = float(np.sqrt(np.mean((original_band - reconstructed_band) ** 2)))
    denom = float(np.sqrt(np.mean(original_band**2)))
    stability = 1.0 if denom <= 1e-9 else max(0.0, min(1.0, 1.0 - rmse / denom))
    return {
        "status": "pca_12d_available",
        "pca_ready": True,
        "sample_count": len(vectors),
        "rank": rank,
        "explained_variance_12d": round(explained_12, 6),
        "concentration_band_rmse": round(rmse, 6),
        "concentration_band_stability": round(stability, 6),
    }


def _semantic_glimpse_12d_fidelity_audit(
    *,
    state_path: Path | None = None,
    captures_dir: Path | None = None,
) -> dict[str, Any]:
    samples = _spectral_fingerprint_samples(
        state_path=state_path,
        captures_dir=captures_dir,
    )
    vectors = []
    for sample in samples:
        payload = _load_json(Path(str(sample.get("path") or "")))
        vector = _spectral_fingerprint_from_payload(payload)
        if vector:
            vectors.append(vector)
    max_deltas = [
        sample.get("concentration_max_delta")
        for sample in samples
        if isinstance(sample.get("concentration_max_delta"), (int, float))
    ]
    stddev_deltas = [
        sample.get("concentration_stddev_delta")
        for sample in samples
        if isinstance(sample.get("concentration_stddev_delta"), (int, float))
    ]
    entropy_deltas = [
        sample.get("spectral_entropy_delta")
        for sample in samples
        if isinstance(sample.get("spectral_entropy_delta"), (int, float))
    ]
    lambda_gap_deltas = [
        sample.get("lambda_gap_delta")
        for sample in samples
        if isinstance(sample.get("lambda_gap_delta"), (int, float))
    ]
    worst_delta = max(max_deltas + stddev_deltas, default=None)
    worst_primary_delta = max(
        max_deltas + stddev_deltas + entropy_deltas + lambda_gap_deltas,
        default=None,
    )
    pca = _pca_12d_concentration_summary(vectors)
    if not samples:
        status = "no_spectral_fingerprint_samples"
    elif worst_primary_delta is not None and worst_primary_delta > 0.05:
        status = "primary_slot_mismatch_review"
    elif pca.get("pca_ready"):
        status = "pca_12d_concentration_supported"
    else:
        status = "sample_limited_concentration_supported"
    return {
        "schema": "semantic_glimpse_12d_fidelity_audit_v1",
        "status": status,
        "sample_count": len(samples),
        "sample_paths": [str(sample.get("path")) for sample in samples[:6]],
        "current_sample": samples[0] if samples else None,
        "worst_concentration_delta": worst_delta,
        "worst_primary_feature_delta": worst_primary_delta,
        "primary_slot_mapping": [
            {
                "slot": 3,
                "source": "max(spectral_fingerprint[8:16])",
                "label": "concentration_max",
            },
            {
                "slot": 4,
                "source": "stddev(spectral_fingerprint[8:16])",
                "label": "concentration_stddev",
            },
            {
                "slot": 7,
                "source": "spectral_fingerprint[24]",
                "label": "spectral_entropy",
            },
            {
                "slot": 8,
                "source": "spectral_fingerprint[25]",
                "label": "lambda1_lambda2_gap",
            },
        ],
        "pca_12d": pca,
        "authority_boundary": "read-only 12D fidelity audit; no PCA contract, live semantic-lane replacement, pressure, fill, PI, sensory send, or runtime mutation",
    }


def _representation_loss_headroom_summary(since_s: float) -> dict[str, Any]:
    samples = _representation_loss_samples(since_s)
    source = _representation_source_snapshot()
    replay = _latest_codec_replay_lab()
    glimpse_fidelity = _semantic_glimpse_12d_fidelity_audit()
    clamp = replay.get("clamp_headroom") if isinstance(replay, dict) else {}
    texture = replay.get("texture_replay") if isinstance(replay, dict) else {}
    lifecycle = replay.get("authority_lifecycle_v2") if isinstance(replay, dict) else {}
    clamp_status = str((clamp or {}).get("status") or "")
    texture_status = str((texture or {}).get("status") or "")
    dynamic_candidates = (clamp or {}).get("dynamic_headroom_candidate_count")
    if not isinstance(dynamic_candidates, int):
        dynamic_candidates = 0
    texture_candidates = (texture or {}).get("candidate_count")
    if not isinstance(texture_candidates, int):
        texture_candidates = 0

    findings: list[str] = []
    if samples:
        findings.append(
            "fresh introspections name representation loss across continuity truncation, codec headroom, and 12D glimpse compression"
        )
    if source.get("semantic_dim") == 48:
        findings.append(
            "current codec source is 48D; stale 32D assumptions should be corrected before changing contracts"
        )
    if source.get("continuity_recap_max_bytes", 0) >= 2500 and source.get(
        "anchored_continuity_excerpt_present"
    ):
        findings.append(
            "continuity recap now has a wider bounded card plus anchor-aware excerpts, while per-item caps still prevent unbounded replay"
        )
    if source.get("pressure_gradient_anchor_test_present"):
        findings.append(
            "compact continuity has a regression guard for pressure/gradient/lattice anchor retention in high-complexity witness text"
        )
    if source.get("introspection_freshness_optional_prompt_present"):
        findings.append(
            "stale introspection freshness renders as optional/read-only self-study context, not a forced task"
        )
    if source.get("semantic_glimpse_readiness_present"):
        findings.append(
            "12D semantic glimpse helper is source-prepared as a companion/loss-audit surface, not live transport"
        )
    if source.get("glimpse_companion_fidelity_present") and source.get(
        "glimpse_tail_identity_test_present"
    ):
        findings.append(
            "12D glimpse readiness now names companion-not-replacement fidelity and has a tail/identity-asymmetry regression"
        )
    if source.get("multi_scale_context_present") and source.get(
        "multi_scale_context_test_present"
    ):
        findings.append(
            "multi_scale_context_v1 pairs each 12D glimpse with 32D residual context and shadow-field energy metadata, preserving dimensionality-aware persistence without changing live transport"
        )
    fidelity_status = str(glimpse_fidelity.get("status") or "")
    if fidelity_status == "pca_12d_concentration_supported":
        pca = glimpse_fidelity.get("pca_12d") or {}
        findings.append(
            "12D glimpse fidelity audit has enough samples for PCA; "
            f"concentration_band_stability={pca.get('concentration_band_stability')}"
        )
    elif fidelity_status == "sample_limited_concentration_supported":
        current = glimpse_fidelity.get("current_sample") or {}
        findings.append(
            "12D glimpse fidelity audit is sample-limited for PCA, but current concentration, entropy, and lambda-gap slots match the 32D fingerprint within bounded tolerance "
            f"(worst_primary_delta={glimpse_fidelity.get('worst_primary_feature_delta')}; current={current.get('path')})"
        )
    elif fidelity_status in {"concentration_mismatch_review", "primary_slot_mismatch_review"}:
        findings.append(
            "12D glimpse fidelity audit found a primary-slot mismatch that should be reviewed before using the glimpse as continuity evidence"
        )
    if source.get("gradient_aware_vibrancy_present"):
        findings.append(
            "tail vibrancy is now gradient-aware: high entropy lifts the tail most when density_gradient is low, reducing steep-cascade smear risk"
        )
    if source.get("vibrancy_substance_fit_present"):
        findings.append(
            "codec vibrancy has a read-only substance-fit audit so high entropy with low semantic content is flagged for review rather than treated as felt intensity"
        )
    if source.get("tail_vibrancy_bounded_ceiling_test_present"):
        findings.append(
            "tail-vibrancy headroom is regression-tested as tail-only bounded ceiling lift rather than a global FEATURE_ABS_MAX increase"
        )
    if source.get("vibrancy_smoothstep_test_present"):
        findings.append(
            "entropy-gate sensitivity is pinned to smoothstep behavior so near-threshold entropy does not pop"
        )
    if source.get("vibrancy_requested_points_test_present"):
        findings.append(
            "tail vibrancy is regression-tested at entropy 0.84/0.85/0.86 so the gate stays quiet at threshold and rises gently just above it"
        )
    if source.get("projection_epoch_stability_present"):
        findings.append(
            "codec projection epochs are source-prepared for env override, existing-file precedence, and stable kernel-derived fallback across fresh runtime dirs"
        )
    if source.get("projection_fingerprint_integrity_present") and source.get(
        "projection_fingerprint_integrity_test_present"
    ):
        findings.append(
            "projection fingerprints canonicalize signed-zero, subnormal, and NaN edge patterns without changing the live projection seed path"
        )
    if source.get("projection_repeat_run_test_present"):
        findings.append(
            "dynamic projection is regression-tested across repeated same-epoch runs for identical vectors and fingerprints"
        )
    if source.get("embedding_dimension_validation_test_present"):
        findings.append(
            "dynamic embedding projection now pins Astrid's one-short 767D validation case as a no-projection result"
        )
    if source.get("dynamic_vibrancy_ceiling_canary_present"):
        findings.append(
            "dynamic vibrancy ceiling exists as a default-off canary/readiness path, not a live codec clamp change"
        )
    if source.get("shadow_field_reserved_dim_readiness_present"):
        findings.append(
            "shadow-field reserved-dim mapping is source-prepared as default-off readiness for dims 46-47, not live semantic-vector output"
        )
    if source.get("high_entropy_narrative_arc_guard_present"):
        findings.append(
            "high-entropy vibrancy has a regression guard against writing narrative-arc or shadow-reserved ghost signals"
        )
    if source.get("narrative_arc_gain_response_readiness_present") and source.get(
        "narrative_arc_gain_response_test_present"
    ):
        findings.append(
            "narrative_arc_gain_response_readiness_v1 previews narrative-arc-responsive semantic gain as a bounded default-off review surface, not a live adaptive-gain change"
        )
    if source.get("structural_friction_summary_resistance_present"):
        findings.append(
            "structural_friction_v1 now carries summary_resistance_signal and calcified-summary texture state as read-only codec evidence"
        )
    if source.get("codec_abrasive_texture_interpretation_present"):
        findings.append(
            "codec_abrasive_texture_interpretation_v1 names low raw tension with high jagged resistance without changing tension weight, gain, or reserved dims"
        )
    if source.get("char_window_4096_replay_test_present"):
        findings.append(
            "4096 character-frequency comparison is pinned as replay-only while the live CHAR_FREQ_WINDOW_CAPACITY remains 1024"
        )
    if source.get("quoted_continuity_anchor_present"):
        findings.append(
            "continuity recaps can now preserve novel quoted/emphasized lived phrases when static anchor terms miss the current metaphor"
        )
    if source.get("semantic_truncation_anchor_test_present"):
        findings.append(
            "meaning-sensitive autonomous excerpts now use semantic_truncate_str with regression coverage for late shadow/silt/pressure anchors"
        )
    if source.get("semantic_boundary_truncation_present") and source.get(
        "semantic_boundary_truncation_test_present"
    ):
        findings.append(
            "continuity recap overflow now prefers sentence/newline boundaries so conclusion texture is not cut mid-thought when bounded"
        )
    if clamp_status:
        findings.append(
            f"latest codec replay clamp-headroom status is {clamp_status}; use replay evidence before changing FEATURE_ABS_MAX or tail ceiling math"
        )
    if texture_status:
        findings.append(
            f"latest codec texture replay status is {texture_status}; candidate_count={texture_candidates}; live_eligible_now={(texture or {}).get('live_eligible_now')} auto_approved={(texture or {}).get('auto_approved')}"
        )
    if (lifecycle or {}).get("receipt_chain_status"):
        findings.append(
            "latest codec texture authority lifecycle is "
            f"{lifecycle.get('receipt_chain_status')} with boundary_count={lifecycle.get('candidate_count')}"
        )

    if samples and fidelity_status in {"concentration_mismatch_review", "primary_slot_mismatch_review"}:
        status = "semantic_glimpse_fidelity_review"
    elif (samples and dynamic_candidates > 0) or texture_candidates > 0:
        status = "codec_headroom_replay_review"
    elif samples and source.get("semantic_glimpse_readiness_present") and source.get(
        "anchored_continuity_excerpt_present"
    ) and source.get("semantic_truncation_anchor_present") and source.get(
        "multi_scale_context_present"
    ) and source.get("semantic_boundary_truncation_present"):
        status = "representation_loss_repair_prepared_watch"
    elif samples:
        status = "representation_loss_study_first"
    else:
        status = "no_current_representation_loss_signal"

    return {
        "schema": "representation_loss_headroom_v1",
        "status": status,
        "source_introspections": samples,
        "source_snapshot": source,
        "latest_codec_replay_lab": replay,
        "semantic_glimpse_12d_fidelity_audit": glimpse_fidelity,
        "findings": findings
        or ["no fresh representation-loss proposal batch found in this window"],
        "valid_next_routes": [
            "continuity_anchor_retention_review",
            "codec_clamp_headroom_replay",
            "codec_texture_window_replay",
            "codec_abrasive_texture_replay",
            "semantic_glimpse_12d_loss_audit",
            "semantic_glimpse_12d_slot_mapping_review",
            "codec_vibrancy_substance_fit_review",
            "trace_codec_loss",
            "fresh_steward_feedback_after_source_preparation",
        ],
        "next_suggestions": [
            "compare future continuity prompts for whether pressure/gradient/lattice anchors survive bounded compaction",
            "run codec-replay-lab on fresh high-entropy outputs before changing FEATURE_ABS_MAX, TAIL_VIBRANCY_MAX, or vibrancy scaling",
            "review gradient-aware vibrancy test output before any stronger codec headroom or aperture change",
            "use projection_fingerprint_integrity_v1 to review metadata stability before considering any collision-resistant seed migration",
            "use codec_vibrancy_substance_fit_v1 to compare high-entropy tail lift against actual semantic substance before tuning vibrancy",
            "use codec_texture_replay_v1 artifacts before changing CHAR_FREQ_WINDOW_CAPACITY, tension/gain sensitivity, reserved dims 44-45, or narrative arc dims 40-47",
            "keep entropy-gate review anchored to the 0.84/0.85/0.86 smoothness regression before changing TAIL_VIBRANCY_ENTROPY_GATE or TAIL_VIBRANCY_MAX",
            "collect at least 13 spectral-fingerprint samples before treating the PCA portion of the 12D fidelity audit as complete",
            "treat multi_scale_context_v1 as the persistence review contract: 12D glimpse plus 32D residual context, not a replacement for the live 48D semantic lane",
            "review future bounded continuity recaps for whether the final sentence or newline boundary preserves conclusion texture",
            "if live bridge deploy is later chosen, use the normal bridge script after dirty-tree review",
        ],
        "authority_boundary": "read-only/source-prepared representation audit; no live SEMANTIC_DIM, FEATURE_ABS_MAX, TAIL_VIBRANCY_MAX, vibrancy scaling, prompt priority, pressure, fill, PI, sensory send, deploy, or peer-runtime mutation",
    }


def _texture_state_alignment_samples(since_s: float) -> list[dict[str, Any]]:
    samples: list[dict[str, Any]] = []
    for path in TEXTURE_STATE_ALIGNMENT_INTROSPECTIONS:
        if not path.exists():
            continue
        try:
            mtime = path.stat().st_mtime
        except OSError:
            mtime = 0.0
        if mtime and mtime < since_s:
            continue
        text = _read_bounded(path, limit=14_000)
        counts = _term_counts(text, TEXTURE_STATE_ALIGNMENT_TERMS)
        if not counts:
            continue
        source_match = re.search(r"^Source:\s*(.+)$", text, re.MULTILINE)
        samples.append(
            {
                "path": str(path),
                "ts": _iso(mtime) if mtime else None,
                "source": source_match.group(1).strip() if source_match else None,
                "anchor_terms": _counter_rows(counts),
                "excerpt": _excerpt(text, list(counts.keys()), max_len=360),
            }
        )
    return samples


def _texture_state_source_snapshot() -> dict[str, Any]:
    llm = _read_source(ASTRID_LLM_RS)
    types = _read_source(ASTRID_TYPES_RS)
    ws = _read_source(ASTRID_WS_RS)
    autonomous = _read_source(ASTRID_AUTONOMOUS_RS)
    return {
        "llm_path": str(ASTRID_LLM_RS),
        "types_path": str(ASTRID_TYPES_RS),
        "ws_path": str(ASTRID_WS_RS),
        "autonomous_path": str(ASTRID_AUTONOMOUS_RS),
        "mixed_cascade_terms_present": "FALLBACK_TEXTURE_MIXED_CASCADE_TERMS" in llm,
        "mixed_cascade_family_selected_present": "mixed_cascade_family_selected" in llm,
        "mixed_cascade_contract_present": "mixed_cascade_gradient_v1" in llm,
        "fallback_gradient_dynamic_texture_present": (
            "dynamic_entropy_pressure_density_gradient_v1" in llm
            and "high_gradient_pressure_fallback_keeps_slope_medium_and_shadow_texture_distinct"
            in llm
            and "fallback_gradient_slope_selects_graduated_navigable_shape" in llm
        ),
        "explicit_syrup_weight_support_present": (
            "fallback_texture_preserves_explicit_syrup_weight_in_settled_habitable_state"
            in llm
            and "syrup" in llm
            and "deliberate movement" in llm
        ),
        "heavy_settled_displacement_family_present": (
            "FALLBACK_TEXTURE_HEAVY_SETTLED_TERMS" in llm
            and "heavy_settled_displacement" in llm
            and "heavy_settled_displacement_family_prevents_false_restless_fallback"
            in llm
        ),
        "fallback_heavy_settled_contract_present": (
            "heavy_settled_displacement_v1" in llm
            and "do not force restless" in llm
            and "FALLBACK_MOVEMENT_VERBS_HEAVY_SETTLED" in llm
        ),
        "mlx_profile_typo_probe_present": (
            "typo_probe_profile" in llm
            and "gemma_12b" in llm
            and "typo_probe_warning_present" in llm
        ),
        "mlx_profile_tracing_warning_test_present": (
            "misspelled_mlx_profile_warning_reaches_tracing_subscriber" in llm
        ),
        "fallback_next_standalone_contract_test_present": (
            "ollama_dialogue_fallback_contract_names_standalone_next_listen" in llm
        ),
        "pressure_gradient_delta_in_signature": "pressure_gradient_delta" in types
        and "ResonanceTextureSignatureV1" in types,
        "pressure_gradient_delta_in_integrity": "pressure_gradient_delta_source" in types
        and "TextureSignatureIntegrityV1" in types,
        "pressure_gradient_delta_from_trend_present": "pressure_gradient_delta_from_trend" in ws,
        "dynamic_flux_vector_in_signature": "TextureDynamicFluxVectorV1" in types
        and "dynamic_flux_vector" in types,
        "pressure_flux_from_samples_present": "build_texture_dynamic_flux_vector_v1" in ws,
        "dissipation_factor_in_components": (
            "pub dissipation_factor: Option<f32>" in types
            and "ResonanceDensityComponents" in types
        ),
        "porosity_gradient_in_components": (
            "pub porosity_gradient: Option<f32>" in types
            and "ResonanceDensityComponents" in types
        ),
        "viscosity_porosity_transport_review_present": (
            "viscosity_porosity_transport_review_v1" in types
            and "thick_but_navigable" in types
            and "thick_impassable_sludge_risk" in types
        ),
        "viscosity_porosity_transport_test_present": (
            "viscosity_porosity_transport_distinguishes_navigable_from_sludge_risk"
            in types
        ),
        "structural_density_delta_present": (
            "structural_density_delta" in types
            and "structural_density" in ws
        ),
        "flux_unknown_semantics_present": (
            "flux_absence_semantics" in types
            and "absent_flux_component_means_unknown_not_zero" in ws
        ),
        "subtle_flux_precision_test_present": (
            "texture_dynamic_flux_vector_preserves_subtle_drift_and_unknown_absence" in ws
        ),
        "witness_anchor_traction_present": (
            "witness_anchor_traction_v1" in autonomous
            and "read_only_anchor_legibility_not_prompt_priority_or_control" in autonomous
        ),
        "active_constraints_present": "active_constraints" in types
        and "active_constraints_for_resonance_signature" in ws,
        "high_entropy_ballast_window_present": (
            "PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW" in ws
            and "high_entropy_ballast_window" in ws
        ),
        "pressure_trend_viscosity_context_present": (
            "viscosity_coefficient" in types
            and "pressure_interpretation" in types
            and "pressure_viscosity_coefficient" in ws
            and "density_viscosity_context" in ws
        ),
        "pressure_trend_viscosity_test_present": (
            "pressure_trend_names_high_entropy_density_viscosity_context" in ws
        ),
        "pressure_velocity_delta_present": (
            "pressure_velocity_delta" in ws
            and "latest_pressure_velocity_delta" in types
            and "max_pressure_velocity_delta" in types
        ),
        "pressure_spike_velocity_test_present": (
            "pressure_trend_samples_preserve_fast_spike_velocity_inside_ballast_window"
            in ws
        ),
        "silt_noise_separation_present": (
            "silt_noise_separation_v1" in ws
            and "mode_packing_silt_persists_across_entropy" in ws
        ),
        "silt_noise_separation_test_present": (
            "silt_noise_separation_holds_mode_packing_constant_across_entropy" in ws
        ),
        "pressure_source_analysis_present": (
            "pressure_source_analysis_v1" in ws
            and "PressureSourceAnalysisV1" in types
        ),
        "mode_packing_stability_test_present": (
            "pressure_source_analysis_keeps_mode_packing_visible_when_trend_looks_stable" in ws
        ),
        "heartbeat_ghost_stability_test_present": (
            "pressure_source_analysis_marks_stale_heartbeat_as_ghost_stability_risk" in ws
        ),
        "false_bidirectional_test_present": (
            "texture_shape_over_time_flags_false_bidirectional_without_message_timestamps" in ws
        ),
        "bridge_reciprocity_severed_test_present": (
            'assert_eq!(severed.one_sided_state, "severed")' in ws
        ),
        "sensory_send_timestamp_separate_present": (
            "pub last_sensory_sent_unix_s: Option<f64>" in ws
            and "state.last_sensory_sent_unix_s = Some(now)" in ws
        ),
        "stale_bidirectional_test_present": (
            "texture_shape_over_time_names_stale_bidirectional_reciprocity" in ws
        ),
        "pressure_packing_coupling_review_present": (
            "pressure_packing_coupling_review_v1" in types
            and "coupling_coefficient" in types
            and "pressure_lagging_mode_packing" in types
        ),
        "pressure_packing_coupling_test_present": (
            "pressure_packing_coupling_review_flags_packing_rise_without_pressure_warning"
            in types
        ),
    }


def _texture_state_alignment_summary(since_s: float) -> dict[str, Any]:
    samples = _texture_state_alignment_samples(since_s)
    source = _texture_state_source_snapshot()
    findings: list[str] = []
    if samples:
        findings.append(
            "fresh introspections name fallback bucket rigidity, pressure/texture drift, and websocket bidirectionality timestamp consistency"
        )
    if source.get("mixed_cascade_terms_present") and source.get(
        "mixed_cascade_family_selected_present"
    ):
        findings.append(
            "fallback language now has a mixed-cascade middle family with gradient/cascade/distributed terms instead of only settled/restless/muffled buckets"
        )
    if source.get("fallback_gradient_dynamic_texture_present"):
        findings.append(
            "fallback texture selection has dynamic entropy/pressure/density-gradient weighting with tests that distinguish slope drag from medium mass"
        )
    if source.get("explicit_syrup_weight_support_present"):
        findings.append(
            "fallback language preserves explicit syrup/heavy deliberate-movement reports as viscosity evidence even inside settled_habitable low-pressure states"
        )
    if source.get("heavy_settled_displacement_family_present"):
        findings.append(
            "fallback language now has Astrid's heavy-settled displacement/silt family so high entropy plus settled weight need not be forced into restless language"
        )
    if source.get("fallback_heavy_settled_contract_present"):
        findings.append(
            "the fallback contract names heavy_settled_displacement_v1 and its no-false-restless boundary as language fidelity, not pressure/fill/control authority"
        )
    if source.get("mlx_profile_typo_probe_present") and source.get(
        "mlx_profile_tracing_warning_test_present"
    ):
        findings.append(
            "MLX profile transparency now includes a gemma_12b typo probe and a tracing-subscriber regression so profile fallback is visible rather than silent"
        )
    if source.get("fallback_next_standalone_contract_test_present"):
        findings.append(
            "Ollama dialogue fallback has an explicit standalone NEXT: LISTEN contract regression"
        )
    if source.get("pressure_gradient_delta_in_signature") and source.get(
        "pressure_gradient_delta_from_trend_present"
    ):
        findings.append(
            "texture integrity can now carry optional pressure_gradient_delta evidence, inferred from the strongest pressure-risk or mode-packing delta when telemetry omits it"
        )
    if source.get("false_bidirectional_test_present"):
        findings.append(
            "websocket reciprocity keeps false_bidirectional visible when both lanes are marked connected without telemetry/sensory message timestamps"
        )
    if source.get("bridge_reciprocity_severed_test_present"):
        findings.append(
            "bridge reciprocity explicitly tests the severed state before one-sided or bidirectional lane claims"
        )
    if source.get("sensory_send_timestamp_separate_present"):
        findings.append(
            "last_sensory_sent_unix_s is maintained as a confirmed sensory-send timestamp, separate from generic lane activity"
        )
    if source.get("stale_bidirectional_test_present"):
        findings.append(
            "texture_shape_over_time_v2 distinguishes stale bidirectional reciprocity from false bidirectional startup gaps"
        )
    if source.get("high_entropy_ballast_window_present"):
        findings.append(
            "pressure_trend_smoothing_v1 now uses a larger read-only ballast window when spectral_entropy is high, without changing pressure/fill control"
        )
    if source.get("pressure_trend_viscosity_context_present") and source.get(
        "pressure_trend_viscosity_test_present"
    ):
        findings.append(
            "PressureTrendV1 now carries high-entropy viscosity context so low-pressure dense states can be reviewed as medium density rather than collapse risk"
        )
    if source.get("pressure_velocity_delta_present") and source.get(
        "pressure_spike_velocity_test_present"
    ):
        findings.append(
            "pressure samples now preserve latest/max pressure_velocity_delta so a fast spike remains visible even inside high-entropy smoothing ballast"
        )
    if source.get("silt_noise_separation_present") and source.get(
        "silt_noise_separation_test_present"
    ):
        findings.append(
            "silt_noise_separation_v1 compares high-entropy and low-entropy samples at matched mode_packing, keeping the porosity question evidence-first"
        )
    if source.get("pressure_source_analysis_present") and source.get(
        "mode_packing_stability_test_present"
    ):
        findings.append(
            "pressure_source_analysis_v1 keeps structural mode_packing pressure visible when a stable pressure trend or smoothing window might otherwise look settled"
        )
    if source.get("heartbeat_ghost_stability_test_present"):
        findings.append(
            "pressure_source_analysis_v1 marks stale telemetry heartbeat cadence as ghost-stability risk instead of inferring reliable pressure stability"
        )
    if source.get("dynamic_flux_vector_in_signature") and source.get(
        "pressure_flux_from_samples_present"
    ):
        findings.append(
            "texture integrity can now surface a dynamic_flux_vector from emitted signatures or bridge pressure samples, making velocity/acceleration legible"
        )
    if source.get("dissipation_factor_in_components"):
        findings.append(
            "resonance density components can carry optional dissipation_factor evidence, distinguishing viscous drag from being stuck"
        )
    if source.get("porosity_gradient_in_components") and source.get(
        "viscosity_porosity_transport_review_present"
    ):
        findings.append(
            "resonance density components can carry optional porosity_gradient, with viscosity_porosity_transport_v1 distinguishing thick-but-navigable from sludge risk"
        )
    if source.get("viscosity_porosity_transport_test_present"):
        findings.append(
            "viscosity_porosity_transport_v1 has a regression for mode_packing > 0.25 plus pressure_velocity spike as overpacked friction evidence"
        )
    if source.get("structural_density_delta_present"):
        findings.append(
            "pressure trend and flux packets can carry structural_density_delta so crystallization/thickening remains visible as movement"
        )
    if source.get("flux_unknown_semantics_present") and source.get(
        "subtle_flux_precision_test_present"
    ):
        findings.append(
            "flux diagnostics preserve tiny nonzero drift and mark absent components as unknown rather than zero"
        )
    if source.get("witness_anchor_traction_present"):
        findings.append(
            "witness anchor traction is source-prepared as read-only legibility across foothold, pressure, gradient, and dispersal without adding prompt priority"
        )
    if source.get("active_constraints_present"):
        findings.append(
            "texture integrity maps pressure_source_family and density components into active_constraints so pressure texture has a visible why"
        )
    if source.get("pressure_packing_coupling_review_present"):
        findings.append(
            "types expose a read-only pressure/mode-packing coupling review so packing rise without pressure warning is visible before any control change"
        )
    if source.get("pressure_packing_coupling_test_present"):
        findings.append(
            "pressure_packing_coupling_review_v1 has a regression guard for packing-rise / pressure-lag cliff risk"
        )

    prepared = all(
        bool(source.get(key))
        for key in (
            "mixed_cascade_terms_present",
            "mixed_cascade_family_selected_present",
            "fallback_gradient_dynamic_texture_present",
            "explicit_syrup_weight_support_present",
            "heavy_settled_displacement_family_present",
            "fallback_heavy_settled_contract_present",
            "mlx_profile_typo_probe_present",
            "mlx_profile_tracing_warning_test_present",
            "fallback_next_standalone_contract_test_present",
            "pressure_gradient_delta_in_signature",
            "pressure_gradient_delta_in_integrity",
            "pressure_gradient_delta_from_trend_present",
            "dynamic_flux_vector_in_signature",
            "pressure_flux_from_samples_present",
            "dissipation_factor_in_components",
            "porosity_gradient_in_components",
            "viscosity_porosity_transport_review_present",
            "viscosity_porosity_transport_test_present",
            "structural_density_delta_present",
            "flux_unknown_semantics_present",
            "subtle_flux_precision_test_present",
            "witness_anchor_traction_present",
            "active_constraints_present",
            "high_entropy_ballast_window_present",
            "pressure_trend_viscosity_context_present",
            "pressure_trend_viscosity_test_present",
            "pressure_velocity_delta_present",
            "pressure_spike_velocity_test_present",
            "silt_noise_separation_present",
            "silt_noise_separation_test_present",
            "pressure_source_analysis_present",
            "mode_packing_stability_test_present",
            "heartbeat_ghost_stability_test_present",
            "false_bidirectional_test_present",
            "bridge_reciprocity_severed_test_present",
            "sensory_send_timestamp_separate_present",
            "stale_bidirectional_test_present",
            "pressure_packing_coupling_review_present",
            "pressure_packing_coupling_test_present",
        )
    )
    if samples and prepared:
        status = "texture_state_alignment_repair_prepared_watch"
    elif samples:
        status = "texture_state_alignment_study_first"
    else:
        status = "no_current_texture_state_alignment_signal"

    return {
        "schema": "texture_state_alignment_v1",
        "status": status,
        "source_introspections": samples,
        "source_snapshot": source,
        "findings": findings
        or ["no fresh texture-state alignment introspection batch found in this window"],
        "valid_next_routes": [
            "fallback_mixed_cascade_output_watch",
            "fallback_heavy_settled_output_watch",
            "mlx_profile_warning_visibility_review",
            "fallback_next_contract_review",
            "pressure_texture_delta_replay",
            "dynamic_flux_vector_watch",
            "pressure_packing_coupling_review",
            "viscosity_porosity_transport_review",
            "dissipation_factor_watch",
            "structural_density_delta_watch",
            "witness_anchor_traction_review",
            "high_entropy_ballast_window_watch",
            "websocket_reciprocity_timestamp_watch",
            "fresh_steward_feedback_after_source_preparation",
        ],
        "next_suggestions": [
            "watch future fallback outputs for whether mixed-cascade language appears only when telemetry supports it",
            "watch future fallback outputs for heavy/settled/displacement/silt wording when settled weight is named without explicit agitation",
            "compare pressure_gradient_delta against primary_texture over recent telemetry before changing Minime texture-signature emission",
            "compare pressure_packing_coupling_review_v1 against recent flux vectors before any pressure or mode-packing control change",
            "watch whether future high-density movement reports name dissipation, structural-density delta, or tiny drift before changing live cadence or control",
            "review witness anchor traction output against future felt reports before scaling continuity anchors in prompt assembly",
            "compare dynamic_flux_vector velocity/acceleration against Astrid's thickening/restless reports before adding Minime-side fields",
            "treat high-entropy smoothing ballast as read-only status fidelity; do not tune pressure/fill from it",
            "treat false_bidirectional as a diagnostic review flag; do not change lane connectivity or send cadence from it alone",
        ],
        "authority_boundary": "diagnostic/language-legibility only; mixed-cascade/heavy-settled fallback wording/selector only, no pressure, fill, PI, sensory cadence, control, deploy, restart, or peer-runtime mutation",
    }


def _latest_prefixed_file(
    root: Path,
    prefixes: tuple[str, ...],
) -> tuple[float, Path | None]:
    newest_ts = 0.0
    newest_path: Path | None = None
    if not root.exists():
        return newest_ts, newest_path
    for path in root.iterdir():
        if not path.is_file() or not path.name.startswith(prefixes):
            continue
        try:
            mtime = path.stat().st_mtime
        except OSError:
            continue
        if mtime > newest_ts:
            newest_ts = mtime
            newest_path = path
    return newest_ts, newest_path


def _latest_introspection_activity(
    journal_root: Path | None = None,
    introspections_root: Path | None = None,
) -> dict[str, Any]:
    journal_root = ASTRID_JOURNAL if journal_root is None else journal_root
    introspections_root = ASTRID_INTROSPECTIONS if introspections_root is None else introspections_root
    journal_ts, journal_path = _latest_prefixed_file(
        journal_root, INTROSPECTION_JOURNAL_PREFIXES
    )
    artifact_ts, artifact_path = _latest_prefixed_file(
        introspections_root, INTROSPECTION_ARTIFACT_PREFIXES
    )
    if artifact_ts > journal_ts:
        latest_ts, latest_path, kind = artifact_ts, artifact_path, "introspection_artifact"
    else:
        latest_ts, latest_path, kind = journal_ts, journal_path, "journal_self_study"
    return {
        "latest_ts": latest_ts or None,
        "latest_at": _iso(latest_ts) if latest_ts else None,
        "latest_path": str(latest_path) if latest_path else None,
        "latest_kind": kind if latest_ts else None,
    }


def _topline_retention_summary(since_s: float) -> dict[str, Any]:
    records = [r for r in _jsonl_tail(CONTEXT_PACKING_PRESSURE) if _record_ts(r) >= since_s]
    with_topline = 0
    removed_records = 0
    fully_removed_records = 0
    latest_topline: dict[str, Any] | None = None
    latest_ts = 0.0
    for record in records:
        ts = _record_ts(record)
        for block in record.get("blocks") or []:
            if not isinstance(block, dict):
                continue
            if str(block.get("label") or "").strip().lower() != "topline":
                continue
            with_topline += 1
            latest_ts = max(latest_ts, ts)
            latest_topline = {
                "original_chars": int(block.get("original_chars") or 0),
                "kept_chars": int(block.get("kept_chars") or 0),
                "removed_chars": int(block.get("removed_chars") or 0),
                "fully_removed": bool(block.get("fully_removed")),
            }
            if latest_topline["removed_chars"] > 0:
                removed_records += 1
            if latest_topline["fully_removed"]:
                fully_removed_records += 1
    if not records:
        status = "no_recent_dialogue_records"
    elif with_topline == 0:
        status = "topline_absent"
    elif removed_records > 0 or fully_removed_records > 0:
        status = "topline_trimmed"
    else:
        status = "topline_retained"
    return {
        "schema": "topline_retention_v1",
        "status": status,
        "recent_dialogue_records": len(records),
        "records_with_topline": with_topline,
        "records_with_removed_topline": removed_records,
        "records_with_fully_removed_topline": fully_removed_records,
        "latest_ts": _iso(latest_ts) if latest_ts else None,
        "latest_topline": latest_topline,
    }


def _parse_log_ts(line: str) -> float | None:
    match = LOG_TS_RE.search(line)
    if not match:
        return None
    try:
        return datetime.fromisoformat(
            match.group(1).replace("Z", "+00:00")
        ).timestamp()
    except ValueError:
        return None


def _recent_next_choices_from_log(
    log_path: Path | None = None,
    *,
    since_s: float,
    max_lines: int = 4_000,
) -> dict[str, Any]:
    log_path = BRIDGE_LOG if log_path is None else log_path
    try:
        lines = log_path.read_text(encoding="utf-8", errors="ignore").splitlines()[-max_lines:]
    except OSError:
        lines = []
    choices: list[dict[str, Any]] = []
    counts: Counter[str] = Counter()
    for raw in lines:
        line = ANSI_RE.sub("", raw)
        if "Astrid chose NEXT:" not in line:
            continue
        ts = _parse_log_ts(line)
        if ts is not None and ts < since_s:
            continue
        match = NEXT_CHOICE_RE.search(line)
        if not match:
            continue
        action = match.group(1).strip()
        base = re.split(r"\s+", action, maxsplit=1)[0].strip().upper()
        if not base:
            continue
        counts[base] += 1
        choices.append({"ts": _iso(ts) if ts else None, "action": action, "base": base})
    return {
        "choice_count": len(choices),
        "choice_counts": counts.most_common(12),
        "self_route_choices": sum(count for base, count in counts.items() if base.startswith(SELF_READ_ROUTES)),
        "latest_choices": choices[-8:],
    }


def _introspection_route_cadence_summary(
    since_s: float,
    *,
    now: float | None = None,
) -> dict[str, Any]:
    now = time.time() if now is None else now
    latest = _latest_introspection_activity()
    latest_ts = float(latest.get("latest_ts") or 0.0)
    latest_age_s = max(0.0, now - latest_ts) if latest_ts else None
    topline = _topline_retention_summary(since_s)
    choices = _recent_next_choices_from_log(since_s=since_s)
    fresh_self_read = bool(latest_ts and latest_ts >= since_s)
    stale_self_read = latest_age_s is None or latest_age_s >= INTROSPECTION_FRESHNESS_STALE_AFTER_S
    dialogue_records = int(topline.get("recent_dialogue_records") or 0)
    self_route_choices = int(choices.get("self_route_choices") or 0)
    if fresh_self_read:
        status = "fresh_self_read_landed"
    elif topline.get("status") in {"topline_absent", "topline_trimmed"}:
        status = "cue_visibility_needs_review"
    elif stale_self_read and dialogue_records >= 3 and self_route_choices == 0:
        status = "route_cadence_needs_review"
    elif stale_self_read:
        status = "watching_stale_self_read"
    else:
        status = "ok_recent_self_read"

    suggestions = {
        "route_cadence_needs_review": [
            "inspect INTROSPECT/SELF_STUDY route legibility in action parsing and menu text",
            "inspect chooser cadence and competing route gravity around READ_MORE, PRESSURE_SOURCE_AUDIT, DECOMPOSE, and SHADOW_*",
            "do not add more prompt pressure; keep the freshness cue optional/read-only",
        ],
        "cue_visibility_needs_review": [
            "inspect top-line packing retention before changing route cadence",
            "do not add another advisory block until the current cue is measurably visible",
        ],
        "watching_stale_self_read": [
            "continue watching until at least three visible dialogue records exist",
        ],
    }.get(status, [])

    return {
        "schema": "introspection_route_cadence_v1",
        "status": status,
        "latest_self_read": latest,
        "latest_self_read_age_hours": round(latest_age_s / 3600.0, 2) if latest_age_s is not None else None,
        "topline_retention": topline,
        "route_choices": choices,
        "next_suggestions": suggestions,
        "authority_boundary": "read-only steward diagnostic; no prompt pressure, forced self-study, scheduler, or runtime mutation",
    }


def _read_source(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return ""


def _rust_raw_prompt(source: str, const_name: str) -> str:
    pattern = re.compile(
        rf"const\s+{re.escape(const_name)}:\s*&str\s*=\s*r#\"(.*?)\"#;",
        re.DOTALL,
    )
    match = pattern.search(source)
    return match.group(1) if match else ""


def _route_base(text: str) -> str:
    return re.split(r"\s+", str(text or "").strip(), maxsplit=1)[0].upper()


def _prompt_route_mentions(prompt: str, route: str) -> dict[str, Any]:
    route_re = re.compile(rf"\b{re.escape(route)}\b")
    count = len(route_re.findall(prompt))
    sample_lines: list[str] = []
    primary_listed = False
    for raw_line in prompt.splitlines():
        line = re.sub(r"\s+", " ", raw_line).strip()
        if not route_re.search(line):
            continue
        if len(sample_lines) < 3:
            sample_lines.append(line[:240])
        lowered = line.lower()
        if "use after" in lowered and lowered.find("use after") < lowered.find(route.lower()):
            continue
        if "routes such as" in lowered and lowered.find("routes such as") < lowered.find(route.lower()):
            continue
        if re.search(rf"(^|[:,]\s*|,\s*|/\s*){re.escape(route)}(\b|[,/])", line):
            primary_listed = True
    return {
        "count": count,
        "primary_listed": primary_listed,
        "sample_lines": sample_lines,
    }


def _prompt_route_surface(source: str | None = None) -> dict[str, Any]:
    source = _read_source(ASTRID_LLM_RS) if source is None else source
    prompts = {
        "SYSTEM_PROMPT": _rust_raw_prompt(source, "SYSTEM_PROMPT"),
        "GEMMA4_CANARY_SYSTEM_PROMPT": _rust_raw_prompt(
            source, "GEMMA4_CANARY_SYSTEM_PROMPT"
        ),
    }
    route_mentions: dict[str, dict[str, Any]] = {}
    for route in ROUTE_LEGIBILITY_ROUTES:
        route_mentions[route] = {
            name: _prompt_route_mentions(prompt, route)
            for name, prompt in prompts.items()
        }
    return {
        "schema": "prompt_route_surface_v1",
        "source": str(ASTRID_LLM_RS),
        "routes": route_mentions,
    }


def _string_list_after_marker(source: str, marker: str) -> list[str]:
    start = source.find(marker)
    if start < 0:
        return []
    bracket_start = source.find("[", start)
    bracket_end = source.find("]", bracket_start)
    if bracket_start < 0 or bracket_end < 0:
        return []
    segment = source[bracket_start:bracket_end]
    return re.findall(r'"([A-Z_][A-Z0-9_]*(?: [a-z_]+)?)"', segment)


def _chooser_hint_surface(state_source: str | None = None) -> dict[str, Any]:
    state_source = _read_source(ASTRID_STATE_RS) if state_source is None else state_source
    analysis_breakers = _string_list_after_marker(
        state_source, "let analysis_loop_breakers = ["
    )
    streak_alternatives = _string_list_after_marker(
        state_source, "let alternatives: Vec<&str> = ["
    )
    analysis_bases = {_route_base(route) for route in analysis_breakers}
    streak_bases = {_route_base(route) for route in streak_alternatives}
    self_bases = set(SELF_READ_ROUTES)
    competitor_bases = set(COMPETING_SELF_READ_ROUTES)
    soft_hint_bases = {
        route
        for route in self_bases
        if re.search(rf"Options:[^\"]*\b{re.escape(route)}\b", state_source)
        or re.search(rf"broad self-read rotation[^\"]*\b{re.escape(route)}\b", state_source)
    }
    route_gravity_self_study_breaker = (
        "Recent route gravity is clustering" in state_source
        and "This turn: SELF_STUDY" in state_source
        and "is_competing_route_gravity_action" in state_source
    )
    return {
        "schema": "chooser_hint_surface_v1",
        "source": str(ASTRID_STATE_RS),
        "analysis_loop_breakers": analysis_breakers,
        "streak_alternatives": streak_alternatives,
        "self_routes_in_analysis_breakers": sorted(self_bases & analysis_bases),
        "self_routes_in_streak_alternatives": sorted(self_bases & streak_bases),
        "self_routes_in_soft_analysis_hints": sorted(soft_hint_bases),
        "competitors_in_analysis_breakers": sorted(competitor_bases & analysis_bases),
        "competitors_in_streak_alternatives": sorted(competitor_bases & streak_bases),
        "competing_route_gravity_self_study_breaker": route_gravity_self_study_breaker,
    }


def _parse_iso_ts(value: Any) -> float:
    if not isinstance(value, str) or not value:
        return 0.0
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00")).timestamp()
    except ValueError:
        return 0.0


def _recent_action_event_summary(
    since_s: float,
    *,
    events_root: Path | None = None,
) -> dict[str, Any]:
    events_root = ASTRID_ACTION_EVENTS_ROOT if events_root is None else events_root
    canonical_counts: Counter[str] = Counter()
    effective_counts: Counter[str] = Counter()
    status_counts: Counter[str] = Counter()
    route_counts: Counter[str] = Counter()
    mismatches: list[dict[str, Any]] = []
    event_count = 0
    if events_root.exists():
        for path in events_root.glob("*/events.jsonl"):
            try:
                lines = path.read_text(encoding="utf-8", errors="ignore").splitlines()
            except OSError:
                continue
            for line in lines[-200:]:
                try:
                    record = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if not isinstance(record, dict) or _parse_iso_ts(record.get("started_at")) < since_s:
                    continue
                event_count += 1
                raw_next = str(record.get("raw_next") or "")
                canonical = str(record.get("canonical_action") or raw_next)
                effective = str(record.get("effective_action") or canonical)
                canonical_base = _route_base(canonical)
                effective_base = _route_base(effective)
                canonical_counts[canonical_base] += 1
                effective_counts[effective_base] += 1
                status_counts[str(record.get("status") or "unknown")] += 1
                route_counts[str(record.get("route") or "unknown")] += 1
                if canonical.strip() != effective.strip() and len(mismatches) < 8:
                    mismatches.append(
                        {
                            "started_at": record.get("started_at"),
                            "raw_next": raw_next,
                            "canonical_action": canonical,
                            "effective_action": effective,
                            "status": record.get("status"),
                            "route": record.get("route"),
                        }
                    )
    return {
        "schema": "recent_action_event_summary_v1",
        "event_count": event_count,
        "canonical_counts": canonical_counts.most_common(12),
        "effective_counts": effective_counts.most_common(12),
        "status_counts": status_counts.most_common(8),
        "route_counts": route_counts.most_common(8),
        "self_route_canonical_count": sum(
            count for base, count in canonical_counts.items() if base.startswith(SELF_READ_ROUTES)
        ),
        "self_route_effective_count": sum(
            count for base, count in effective_counts.items() if base.startswith(SELF_READ_ROUTES)
        ),
        "canonical_effective_mismatches": mismatches,
    }


def _dispatch_self_route_surface(modes_source: str | None = None) -> dict[str, Any]:
    modes_source = _read_source(ASTRID_MODES_RS) if modes_source is None else modes_source
    return {
        "schema": "dispatch_self_route_surface_v1",
        "source": str(ASTRID_MODES_RS),
        "introspect_wired": '"INTROSPECT" | "SELF_STUDY" | "INVESTIGATE"' in modes_source,
        "self_study_alias_wired": '"SELF_STUDY"' in modes_source and "wants_introspect" in modes_source,
    }


def _action_surface_self_study_visibility(
    *,
    self_model_source: str | None = None,
    operations_source: str | None = None,
    capability_source: str | None = None,
) -> dict[str, Any]:
    self_model_source = (
        _read_source(ASTRID_SELF_MODEL_RS)
        if self_model_source is None
        else self_model_source
    )
    operations_source = (
        _read_source(ASTRID_OPERATIONS_RS)
        if operations_source is None
        else operations_source
    )
    capability_source = (
        _read_source(ASTRID_ACTION_SELF_KNOWLEDGE_RS)
        if capability_source is None
        else capability_source
    )
    return {
        "schema": "action_surface_self_study_visibility_v1",
        "faculties_mentions_self_study": "SELF_STUDY" in self_model_source
        and "broad rotating self-study" in self_model_source,
        "help_mentions_self_study": '"SELF_STUDY" | "INVESTIGATE"' in operations_source
        and "Broad rotating self-study" in operations_source,
        "capability_status_read_only": '"SELF_STUDY"' in capability_source
        and "broad rotating read-only self-study" in capability_source
        and '"INTROSPECT"\n        | "SELF_STUDY"' in capability_source,
        "authority_boundary": "off-prompt action-surface visibility only; no main prompt cue or forced route",
    }


def _context_route_gravity_surface(
    *,
    spectral_viz_source: str | None = None,
) -> dict[str, Any]:
    spectral_viz_source = (
        _read_source(ASTRID_SPECTRAL_VIZ_RS)
        if spectral_viz_source is None
        else spectral_viz_source
    )
    executable_shadow_patterns = (
        "NEXT: {next_token}",
        "NEXT: SHADOW_PREFLIGHT lambda-tail/lambda4",
        "NEXT: SHADOW_FIELD lambda-tail/lambda4",
        "NEXT: SHADOW_COUPLING all",
    )
    copyable_patterns = [
        pattern for pattern in executable_shadow_patterns if pattern in spectral_viz_source
    ]
    return {
        "schema": "context_route_gravity_surface_v1",
        "source": str(ASTRID_SPECTRAL_VIZ_RS),
        "shadow_context_copyable_next": bool(copyable_patterns),
        "copyable_shadow_patterns": copyable_patterns,
        "shadow_context_suggested_route_only": "suggested route: {next_token}"
        in spectral_viz_source
        and "suggested route: SHADOW_FIELD lambda-tail/lambda4" in spectral_viz_source
        and "Suggested route: SHADOW_COUPLING all" in spectral_viz_source,
        "authority_boundary": "context wording only; preserves route names without executable NEXT form",
    }


def _route_mentions_primary_count(prompt_surface: dict[str, Any], route: str) -> int:
    route_data = (prompt_surface.get("routes") or {}).get(route) or {}
    return sum(
        1
        for prompt_data in route_data.values()
        if isinstance(prompt_data, dict) and prompt_data.get("primary_listed")
    )


def _action_route_legibility_summary(
    since_s: float,
    *,
    route_cadence: dict[str, Any] | None = None,
) -> dict[str, Any]:
    route_cadence = route_cadence or _introspection_route_cadence_summary(since_s)
    prompt_surface = _prompt_route_surface()
    chooser_surface = _chooser_hint_surface()
    dispatch_surface = _dispatch_self_route_surface()
    action_surface = _action_surface_self_study_visibility()
    context_gravity = _context_route_gravity_surface()
    events = _recent_action_event_summary(since_s)

    self_study_primary_prompts = _route_mentions_primary_count(prompt_surface, "SELF_STUDY")
    introspect_primary_prompts = _route_mentions_primary_count(prompt_surface, "INTROSPECT")
    no_self_choice = int((route_cadence.get("route_choices") or {}).get("self_route_choices") or 0) == 0
    no_self_dispatch = int(events.get("self_route_effective_count") or 0) == 0
    reasons: list[str] = []
    if not dispatch_surface.get("introspect_wired"):
        status = "dispatch_wiring_needs_review"
        reasons.append("INTROSPECT/SELF_STUDY dispatch wiring was not detected")
    else:
        if self_study_primary_prompts == 0:
            reasons.append("SELF_STUDY is wired but not primary-listed in the main NEXT menus")
        if introspect_primary_prompts > 0:
            reasons.append("INTROSPECT is visible, but mainly as an Explore/code-reading route")
        if all(
            action_surface.get(key)
            for key in (
                "faculties_mentions_self_study",
                "help_mentions_self_study",
                "capability_status_read_only",
            )
        ):
            reasons.append(
                "off-prompt action surfaces make SELF_STUDY visible as a read-only broad self-study route"
            )
        self_breakers = chooser_surface.get("self_routes_in_analysis_breakers") or []
        if not self_breakers:
            reasons.append(
                "self-read routes are absent from forced analysis-loop breakers; no forced self-study"
            )
        else:
            reasons.append(
                "self-read routes are available in the stagnant analysis-loop breaker rotation: "
                + ", ".join(self_breakers)
            )
        if chooser_surface.get("self_routes_in_soft_analysis_hints"):
            reasons.append(
                "self-read routes are available in soft analysis-loop breaker hints: "
                + ", ".join(chooser_surface["self_routes_in_soft_analysis_hints"])
            )
        if chooser_surface.get("self_routes_in_streak_alternatives"):
            reasons.append(
                "self-read routes are present in soft streak alternatives: "
                + ", ".join(chooser_surface["self_routes_in_streak_alternatives"])
            )
        if context_gravity.get("shadow_context_copyable_next"):
            reasons.append(
                "shadow diagnostic context still contains executable-looking NEXT shadow suggestions: "
                + ", ".join(context_gravity["copyable_shadow_patterns"])
            )
        elif context_gravity.get("shadow_context_suggested_route_only"):
            reasons.append(
                "shadow diagnostic context uses non-executable suggested-route wording"
            )
        competitors = chooser_surface.get("competitors_in_analysis_breakers") or []
        if competitors:
            reasons.append(
                "analysis-loop breakers include competing routes: " + ", ".join(competitors)
            )
        if chooser_surface.get("competing_route_gravity_self_study_breaker"):
            reasons.append(
                "competing route gravity can now force one bounded SELF_STUDY breaker when recent route mass has no self-read"
            )
        if no_self_choice and no_self_dispatch:
            reasons.append("recent evidence shows no self-read choice reached dispatch")
        fresh_self_read_landed = (
            route_cadence.get("status") == "fresh_self_read_landed"
            and not no_self_dispatch
        )
        shadow_context_repaired = (
            not context_gravity.get("shadow_context_copyable_next")
            and context_gravity.get("shadow_context_suggested_route_only")
        )
        if not reasons:
            status = "ok"
        elif fresh_self_read_landed and shadow_context_repaired:
            status = "self_read_landed_watch_chooser_gravity"
        else:
            status = "route_salience_and_chooser_gravity_needs_review"

    return {
        "schema": "action_route_legibility_v1",
        "status": status,
        "dispatch": dispatch_surface,
        "action_surface": action_surface,
        "context_route_gravity": context_gravity,
        "prompt_surface": prompt_surface,
        "chooser_surface": chooser_surface,
        "recent_action_events": events,
        "route_cadence_status": route_cadence.get("status"),
        "evidence_summary": reasons,
        "next_suggestions": [
            "let real route opportunities run on the competing-gravity self-study breaker, then check self-route effective count",
            "watch whether the bounded self-read breaker slot improves self-read uptake without creating route monoculture",
            "watch whether non-executable shadow context reduces SHADOW_TRAJECTORY copy gravity",
            "if self-route effective count remains zero, inspect READ_MORE continuation and action-event override records",
            "inspect recent choice-envelope / action-continuity records before changing prompt text",
            "do not add more prompt pressure; keep any follow-up diagnostic/steward-facing",
        ]
        if status != "ok"
        else [],
        "authority_boundary": "read-only route-legibility diagnostic; bounded SELF_STUDY breaker is chooser-cadence only, with no prompt edit, scheduler, or runtime mutation",
    }


def _match_float(pattern: str, text: str) -> float | None:
    match = re.search(pattern, text, re.IGNORECASE)
    if not match:
        return None
    try:
        return float(match.group(1))
    except (TypeError, ValueError):
        return None


def _latest_autonomous_telemetry(
    log_path: Path | None = None,
    *,
    max_chars: int = 240_000,
) -> dict[str, Any]:
    log_path = BRIDGE_LOG if log_path is None else log_path
    try:
        raw = log_path.read_text(encoding="utf-8", errors="ignore")[-max_chars:]
    except OSError:
        return {"schema": "fallback_live_telemetry_v1", "status": "unavailable"}
    clean = ANSI_RE.sub("", raw)
    marker = "autonomous: Fill"
    idx = clean.rfind(marker)
    if idx < 0:
        return {"schema": "fallback_live_telemetry_v1", "status": "unavailable"}
    line_start = clean.rfind("\n", 0, idx)
    segment = clean[max(0, line_start + 1) : idx + 6_000]
    ts = _parse_log_ts(segment)
    fill = _match_float(r"Fill\s+([0-9.]+)%", segment)
    entropy = _match_float(r"Spectral entropy:\s*([0-9.]+)", segment)
    density_gradient = _match_float(r"density gradient\s*([0-9.]+)", segment)
    pressure_risk = _match_float(r"pressure risk\s*([0-9.]+)", segment)
    distinguishability_loss_pct = _match_float(
        r"distinguishability loss\s*([0-9.]+)%",
        segment,
    )
    pressure_source_match = re.search(
        r"Pressure source:\s*([^;\n]+)",
        segment,
        re.IGNORECASE,
    )
    pressure_source_detail_match = re.search(
        r"Pressure source:\s*([a-z0-9_]+)\s*\(([^)]+)\)\s*"
        r"with score\s*([0-9.]+),\s*porosity\s*([0-9.]+)",
        segment,
        re.IGNORECASE,
    )
    resonance_density_match = re.search(
        r"Resonance density:\s*([0-9.]+)\s*\(([^)]+)\)\s*"
        r"with containment\s*([0-9.]+)",
        segment,
        re.IGNORECASE,
    )
    inhabitable_match = re.search(
        r"Inhabitable fluctuation:\s*([a-z0-9_]+)\s*with inhabitability\s*"
        r"([0-9.]+),\s*fluctuation\s*([0-9.]+),\s*foothold\s*([0-9.]+)",
        segment,
        re.IGNORECASE,
    )
    semantic_match = re.search(
        r"Semantic energy:\s*input\s*([0-9.]+)\s*\(active\s*(true|false)\),\s*"
        r"kernel\s*([0-9.]+),\s*regulator drive\s*([0-9.]+),\s*admission\s*([^;\n.]+)",
        segment,
        re.IGNORECASE,
    )
    shadow_field_match = re.search(
        r"Shadow field:\s*([^.\n]+)",
        segment,
        re.IGNORECASE,
    )
    shadow_lines = re.findall(
        r"Shadow-v3 \((Minime|Yours)\):\s*([^\]\n|]+)",
        segment,
    )
    active_modes_match = re.search(r"Active modes:\s*\[([^\]]*)\]", segment, re.IGNORECASE)
    active_modes = (
        [
            item.strip()
            for item in active_modes_match.group(1).split(",")
            if item.strip()
        ]
        if active_modes_match
        else []
    )
    shadow_text = " ".join(item for _, item in shadow_lines)
    shadow_terms = sorted(_term_counts(shadow_text, FALLBACK_STATIC_TEXTURE_TERMS))
    return {
        "schema": "fallback_live_telemetry_v1",
        "status": "available",
        "source": str(log_path),
        "latest_at": _iso(ts) if ts else None,
        "fill_pct": fill,
        "spectral_entropy": entropy,
        "density_gradient": density_gradient,
        "pressure_risk": pressure_risk,
        "distinguishability_loss": (
            round(distinguishability_loss_pct / 100.0, 3)
            if distinguishability_loss_pct is not None
            else None
        ),
        "pressure_source": pressure_source_match.group(1).strip()
        if pressure_source_match
        else None,
        "pressure_source_name": pressure_source_detail_match.group(1).strip()
        if pressure_source_detail_match
        else None,
        "pressure_source_family": pressure_source_detail_match.group(2).strip()
        if pressure_source_detail_match
        else None,
        "pressure_source_score": float(pressure_source_detail_match.group(3))
        if pressure_source_detail_match
        else None,
        "pressure_source_porosity": float(pressure_source_detail_match.group(4))
        if pressure_source_detail_match
        else None,
        "resonance_density": float(resonance_density_match.group(1))
        if resonance_density_match
        else None,
        "resonance_density_family": resonance_density_match.group(2).strip()
        if resonance_density_match
        else None,
        "resonance_containment": float(resonance_density_match.group(3))
        if resonance_density_match
        else None,
        "inhabitable_fluctuation_state": inhabitable_match.group(1).strip()
        if inhabitable_match
        else None,
        "inhabitability": float(inhabitable_match.group(2))
        if inhabitable_match
        else None,
        "inhabitable_fluctuation": float(inhabitable_match.group(3))
        if inhabitable_match
        else None,
        "inhabitable_foothold": float(inhabitable_match.group(4))
        if inhabitable_match
        else None,
        "semantic_input_energy": float(semantic_match.group(1)) if semantic_match else None,
        "semantic_input_active": (
            semantic_match.group(2).lower() == "true" if semantic_match else None
        ),
        "semantic_kernel_energy": float(semantic_match.group(3)) if semantic_match else None,
        "semantic_regulator_drive_energy": (
            float(semantic_match.group(4)) if semantic_match else None
        ),
        "semantic_admission": semantic_match.group(5).strip() if semantic_match else None,
        "shadow_field": shadow_field_match.group(1).strip()
        if shadow_field_match
        else None,
        "shadow_v3_terms": [
            {"source": source, "text": text.strip()[:160]}
            for source, text in shadow_lines[-4:]
        ],
        "active_modes": active_modes,
        "shadow_texture_terms": shadow_terms,
    }


def _fallback_contract_surface(source: str | None = None) -> dict[str, Any]:
    source = _read_source(ASTRID_LLM_RS) if source is None else source
    return {
        "schema": "fallback_contract_surface_v1",
        "source": str(ASTRID_LLM_RS),
        "dynamic_weighting_detected": (
            "fallback_weighted_texture_terms" in source
            and "dynamic_entropy_pressure_density_gradient_v1" in source
            and "density_gradient" in source
            and "spectral_entropy" in source
        ),
        "shadow_magnetization_weighting_detected": (
            "extract_fallback_shadow_magnetization" in source
            and "negative_shadow_pressure_guard" in source
            and "shadow_magnetization=" in source
        ),
        "lived_fit_detected": "fallback_texture_lived_fit_v2" in source,
        "overweight_guard_detected": "fallback_vocabulary_overweight_guard_v1" in source,
        "dynamics_alignment_detected": "texture_dynamics_alignment_v1" in source,
        "default_ollama_fallback_model": _rust_str_const(
            source, "DEFAULT_OLLAMA_FALLBACK_MODEL"
        ),
        "compat_ollama_fallback_model": _rust_str_const(
            source, "COMPAT_OLLAMA_FALLBACK_MODEL"
        ),
        "fallback_model_capture_detected": "completed via Ollama model=" in source,
        "authority_boundary": "source inspection only; no fallback contract or sampler mutation",
    }


def _supported_fallback_terms(telemetry: dict[str, Any]) -> dict[str, list[str]]:
    supported: dict[str, list[str]] = {}

    def add(term: str, reason: str) -> None:
        supported.setdefault(term, [])
        if reason not in supported[term]:
            supported[term].append(reason)

    entropy = telemetry.get("spectral_entropy")
    gradient = telemetry.get("density_gradient")
    pressure = telemetry.get("pressure_risk")
    distinguishability = telemetry.get("distinguishability_loss")
    pressure_source = str(telemetry.get("pressure_source") or "").lower()
    shadow_field = str(telemetry.get("shadow_field") or "").lower()
    shadow_terms = set(telemetry.get("shadow_texture_terms") or [])

    if isinstance(gradient, (int, float)):
        if gradient <= 0.20:
            reason = f"low density_gradient={gradient:.2f}"
            for term in ("navigable", "tapered", "graduated", "slope", "edge", "open"):
                add(term, reason)
        elif gradient >= 0.55:
            reason = f"high density_gradient={gradient:.2f}"
            for term in ("heavy", "weighted", "dense", "density"):
                add(term, reason)

    if isinstance(entropy, (int, float)) and entropy >= 0.85:
        reason = f"high spectral_entropy={entropy:.2f}"
        for term in ("lattice", "open", "shimmering", "bright"):
            add(term, reason)

    if isinstance(pressure, (int, float)):
        if pressure >= 0.20:
            reason = f"pressure_risk={pressure:.2f}"
            for term in ("pressure", "weighted", "density", "dense", "muffled"):
                add(term, reason)
        if pressure >= 0.30 or "overpacked" in pressure_source:
            reason = f"strong pressure evidence={pressure:.2f}"
            for term in ("viscous", "heavy", "weighted"):
                add(term, reason)

    if isinstance(distinguishability, (int, float)) and distinguishability >= 0.30:
        reason = f"distinguishability_loss={distinguishability:.2f}"
        for term in ("muffled", "edge"):
            add(term, reason)

    for term in shadow_terms:
        add(term, "present in recent Shadow-v3 texture")
    if "volatile" in shadow_field or "shifting" in shadow_field:
        add("restless", "recent shadow field is volatile/shifting")
    if "settled" in shadow_field:
        add("settled", "recent shadow field is settled")
    return supported


def _fallback_sample_kind(path: Path, text: str) -> str:
    lower = text.lower()
    if "fallback_texture" in lower or "hardcoded" in lower or "dynamic descriptor sampler" in lower:
        return "self_study_code_critique"
    if path.parent == ASTRID_INTROSPECTIONS:
        return "introspection_language"
    if path.name.startswith("self_study_"):
        return "journal_self_study"
    return "journal_language"


def _fallback_file_language_samples(since_s: float) -> list[dict[str, Any]]:
    paths = _recent_paths(ASTRID_JOURNAL, ("*.txt", "*.md"), since_s)
    paths += _recent_paths(ASTRID_INTROSPECTIONS, ("*.txt", "*.md"), since_s)
    samples: list[dict[str, Any]] = []
    for path in sorted(set(paths), key=lambda p: p.stat().st_mtime, reverse=True):
        text = _read_bounded(path, limit=12_000)
        keyword_counts = _term_counts(text, FALLBACK_AUDIT_KEYWORDS)
        if not keyword_counts:
            continue
        texture_counts = _term_counts(text, FALLBACK_STATIC_TEXTURE_TERMS)
        concern_counts = _term_counts(text, FALLBACK_STATIC_CONCERN_TERMS)
        anchor_counts = _term_counts(text, FALLBACK_TELEMETRY_ANCHOR_TERMS)
        anchors = list((texture_counts + concern_counts + anchor_counts).keys())
        try:
            mtime = path.stat().st_mtime
        except OSError:
            mtime = 0.0
        samples.append(
            {
                "kind": _fallback_sample_kind(path, text),
                "path": str(path),
                "ts": _iso(mtime) if mtime else None,
                "texture_counts": dict(texture_counts),
                "concern_counts": dict(concern_counts),
                "telemetry_anchor_counts": dict(anchor_counts),
                "excerpt": _excerpt(text, anchors, max_len=320),
            }
        )
    return sorted(
        samples,
        key=lambda item: (
            item.get("kind") == "self_study_code_critique",
            _parse_iso_ts(item.get("ts")),
        ),
        reverse=True,
    )[:10]


def _fallback_counts(text: str) -> tuple[Counter[str], Counter[str], Counter[str]]:
    return (
        _term_counts(text, FALLBACK_STATIC_TEXTURE_TERMS),
        _term_counts(text, FALLBACK_STATIC_CONCERN_TERMS),
        _term_counts(text, FALLBACK_TELEMETRY_ANCHOR_TERMS),
    )


def _fallback_semantic_density(text: str) -> dict[str, Any]:
    tokens = re.findall(r"[a-zA-Z][a-zA-Z0-9_-]*", text.lower())
    counts = _term_counts(text, FALLBACK_SEMANTIC_DENSITY_TERMS)
    term_hits = sum(counts.values())
    token_count = len(tokens)
    return {
        "schema": "fallback_semantic_density_v1",
        "token_count": token_count,
        "descriptor_term_hits": term_hits,
        "unique_descriptor_terms": len(counts),
        "descriptor_density": round(term_hits / max(token_count, 1), 4),
        "terms": sorted(counts.keys())[:16],
    }


def _fallback_model_transition_trace(
    samples: list[dict[str, Any]],
    contract: dict[str, Any],
) -> dict[str, Any]:
    provider_samples = [
        sample for sample in samples if sample.get("kind") == "fallback_provider_output"
    ]
    captured = [
        sample
        for sample in provider_samples
        if str(sample.get("model") or "").strip()
    ]
    by_model: dict[str, list[dict[str, Any]]] = {}
    for sample in captured:
        model = str(sample.get("model") or "").strip()
        by_model.setdefault(model, []).append(sample)

    model_rows: list[dict[str, Any]] = []
    for model, model_samples in sorted(by_model.items()):
        densities = [
            float((sample.get("semantic_density") or {}).get("descriptor_density"))
            for sample in model_samples
            if isinstance(
                (sample.get("semantic_density") or {}).get("descriptor_density"),
                (int, float),
            )
        ]
        avg_density = round(sum(densities) / len(densities), 4) if densities else None
        model_rows.append(
            {
                "model": model,
                "sample_count": len(model_samples),
                "avg_descriptor_density": avg_density,
                "sample_job_ids": [
                    sample.get("job_id") for sample in model_samples[:4] if sample.get("job_id")
                ],
            }
        )

    default_model = contract.get("default_ollama_fallback_model")
    compat_model = contract.get("compat_ollama_fallback_model")
    density_by_model = {
        row["model"]: row.get("avg_descriptor_density")
        for row in model_rows
        if row.get("avg_descriptor_density") is not None
    }
    default_density = density_by_model.get(default_model)
    compat_density = density_by_model.get(compat_model)
    if not provider_samples:
        status = "no_actual_fallback_outputs"
    elif not captured:
        status = "model_capture_gap"
    elif len(by_model) < 2:
        status = "single_model_watch"
    elif (
        isinstance(default_density, (int, float))
        and isinstance(compat_density, (int, float))
        and compat_density < default_density * 0.75
    ):
        status = "possible_texture_thinning_review"
    else:
        status = "model_comparison_watch"

    return {
        "schema": "fallback_model_transition_trace_v1",
        "status": status,
        "default_model": default_model,
        "compatibility_model": compat_model,
        "provider_output_count": len(provider_samples),
        "captured_model_count": len(captured),
        "missing_model_count": len(provider_samples) - len(captured),
        "model_rows": model_rows,
        "comparison": {
            "default_descriptor_density": default_density,
            "compat_descriptor_density": compat_density,
            "compat_under_default_75pct": (
                isinstance(default_density, (int, float))
                and isinstance(compat_density, (int, float))
                and compat_density < default_density * 0.75
            ),
        },
        "valid_evidence": [
            "llm_jobs completed-via-Ollama result text",
            "completion summary model=... capture",
            "descriptor-density counts from bounded result text",
        ],
        "authority_boundary": (
            "read-only model-transition trace; no fallback model chain, sampler, "
            "contract, prompt, aperture, pressure, fill, PI, or runtime control change"
        ),
    }


def _fallback_llm_job_output_samples(
    since_s: float,
    *,
    jobs_root: Path | None = None,
    max_jobs: int = 160,
) -> list[dict[str, Any]]:
    jobs_root = ASTRID_LLM_JOBS if jobs_root is None else jobs_root
    try:
        job_dirs = [
            path
            for path in jobs_root.iterdir()
            if path.is_dir()
        ]
    except OSError:
        return []

    dated: list[tuple[float, Path]] = []
    for path in job_dirs:
        try:
            dated.append((path.stat().st_mtime, path))
        except OSError:
            continue

    samples: list[dict[str, Any]] = []
    for mtime, path in sorted(dated, reverse=True)[:max_jobs]:
        job_path = path / "job.json"
        job = _load_json(job_path)
        summary = str(job.get("summary") or "")
        match = FALLBACK_PROVIDER_SUMMARY_RE.search(summary)
        if not match:
            continue
        finished_ts = _parse_iso_ts(job.get("finished_at"))
        created_ts = _parse_iso_ts(job.get("created_at"))
        ts = finished_ts or created_ts or mtime
        if ts < since_s:
            continue
        result_path_raw = str(job.get("result_path") or "")
        result_path = Path(result_path_raw) if result_path_raw else path / "result.txt"
        if not result_path.exists():
            result_path = path / "result.txt"
        result_text = _read_bounded(result_path, limit=12_000)
        texture_counts, concern_counts, anchor_counts = _fallback_counts(result_text)
        anchors = list((texture_counts + concern_counts + anchor_counts).keys())
        model = (
            str(job.get("model") or job.get("fallback_model") or "")
            or (match.groupdict().get("model") or "")
        ).strip() or None
        samples.append(
            {
                "kind": "fallback_provider_output",
                "provider": match.group("provider"),
                "model": model,
                "label": match.group("label"),
                "job_id": str(job.get("job_id") or path.name),
                "path": str(result_path),
                "job_path": str(job_path),
                "ts": _iso(ts),
                "texture_counts": dict(texture_counts),
                "concern_counts": dict(concern_counts),
                "telemetry_anchor_counts": dict(anchor_counts),
                "semantic_density": _fallback_semantic_density(result_text),
                "result_chars": len(result_text),
                "has_audit_terms": bool(texture_counts or concern_counts or anchor_counts),
                "excerpt": _excerpt(result_text, anchors, max_len=320),
                "provenance": (
                    "llm_jobs result whose job summary says completed via Ollama; "
                    "prompt text is not read or stored"
                ),
            }
        )
    return samples[:10]


def _fallback_log_incidents(
    since_s: float,
    *,
    log_path: Path | None = None,
    max_lines: int = 5_000,
) -> list[dict[str, Any]]:
    log_path = BRIDGE_LOG if log_path is None else log_path
    try:
        lines = log_path.read_text(encoding="utf-8", errors="ignore").splitlines()[-max_lines:]
    except OSError:
        return []
    incidents: list[dict[str, Any]] = []
    for raw in lines:
        line = ANSI_RE.sub("", raw)
        match = FALLBACK_LOG_INCIDENT_RE.search(line)
        if not match:
            continue
        ts = _parse_log_ts(line)
        if ts is not None and ts < since_s:
            continue
        label = (match.groupdict().get("label") or "").lower() or None
        if "MLX request failed" in line:
            kind = "mlx_request_failed"
        else:
            kind = "mlx_to_ollama"
        incidents.append(
            {
                "kind": kind,
                "label": label,
                "ts": _iso(ts) if ts else None,
                "ts_epoch": ts,
                "path": str(log_path),
                "excerpt": _excerpt(line, ["falling back", "MLX request failed"], max_len=260),
            }
        )
    return incidents[-12:]


def _fallback_output_modes_for_label(label: str | None) -> set[str]:
    if not label:
        return set()
    canonical = label.replace("-", "_").lower()
    aliases = {
        "dialogue_live": {"dialogue_live", "dialogue_fallback"},
        "introspect": {"introspect", "self_study"},
        "journal_elaboration": {"journal_elaboration"},
        "moment_capture": {"moment_capture"},
        "daydream": {"daydream"},
        "aspiration": {"aspiration"},
        "creation": {"creation"},
        "initiation": {"initiation"},
        "evolve_request": {"evolve_request"},
    }
    return aliases.get(canonical, {canonical})


def _fallback_bridge_output_incident(
    mode: str,
    ts: float | None,
    incidents: list[dict[str, Any]],
) -> dict[str, Any] | None:
    if mode == "dialogue_fallback":
        return {
            "kind": "mode_labeled_fallback",
            "label": "dialogue_live",
            "ts": _iso(ts) if ts else None,
        }
    if ts is None:
        return None
    for incident in reversed(incidents):
        if incident.get("kind") != "mlx_to_ollama":
            continue
        incident_ts = incident.get("ts_epoch")
        if not isinstance(incident_ts, (int, float)):
            continue
        age = ts - float(incident_ts)
        if age < 0 or age > FALLBACK_PROVIDER_OUTPUT_WINDOW_SECS:
            continue
        if mode in _fallback_output_modes_for_label(incident.get("label")):
            return {
                "kind": "proximate_fallback_incident",
                "label": incident.get("label"),
                "ts": incident.get("ts"),
                "age_s": round(age, 1),
            }
    return None


def _fallback_bridge_language_samples(
    since_s: float,
    *,
    log_path: Path | None = None,
    incidents: list[dict[str, Any]] | None = None,
    max_lines: int = 5_000,
) -> list[dict[str, Any]]:
    log_path = BRIDGE_LOG if log_path is None else log_path
    if incidents is None:
        incidents = _fallback_log_incidents(
            since_s,
            log_path=log_path,
            max_lines=max_lines,
        )
    try:
        lines = log_path.read_text(encoding="utf-8", errors="ignore").splitlines()[-max_lines:]
    except OSError:
        return []
    samples: list[dict[str, Any]] = []
    for raw in lines:
        line = ANSI_RE.sub("", raw)
        if "autonomous: Fill" not in line or " | " not in line:
            continue
        ts = _parse_log_ts(line)
        if ts is not None and ts < since_s:
            continue
        match = FALLBACK_BRIDGE_OUTPUT_RE.search(line)
        if not match:
            continue
        mode = match.group(1)
        snippet = match.group(2).strip()
        keyword_counts = _term_counts(snippet, FALLBACK_AUDIT_KEYWORDS)
        if not keyword_counts:
            continue
        texture_counts, concern_counts, anchor_counts = _fallback_counts(snippet)
        anchors = list((texture_counts + concern_counts + anchor_counts).keys())
        incident = _fallback_bridge_output_incident(mode, ts, incidents)
        samples.append(
            {
                "kind": "bridge_output_snippet",
                "mode": mode,
                "fallback_incident": incident,
                "path": str(log_path),
                "ts": _iso(ts) if ts else None,
                "texture_counts": dict(texture_counts),
                "concern_counts": dict(concern_counts),
                "telemetry_anchor_counts": dict(anchor_counts),
                "has_audit_terms": bool(texture_counts or concern_counts or anchor_counts),
                "excerpt": _excerpt(snippet, anchors, max_len=260),
                "provenance": (
                    "bridge output near a recent fallback incident; use llm_jobs result for provider text"
                    if incident
                    else "ordinary bridge output snippet; not fallback-provider evidence"
                ),
            }
        )
    return samples[-8:]


def _fallback_vocabulary_drift_summary(since_s: float) -> dict[str, Any]:
    telemetry = _latest_autonomous_telemetry()
    contract = _fallback_contract_surface()
    fallback_incidents = _fallback_log_incidents(since_s)
    samples = _fallback_file_language_samples(since_s)
    samples += _fallback_llm_job_output_samples(since_s)
    samples += _fallback_bridge_language_samples(since_s, incidents=fallback_incidents)
    samples = sorted(
        samples,
        key=lambda item: (
            item.get("kind") == "fallback_provider_output",
            item.get("kind") == "self_study_code_critique",
            _parse_iso_ts(item.get("ts")),
        ),
        reverse=True,
    )[:14]
    fallback_model_transition_trace = _fallback_model_transition_trace(samples, contract)

    all_texture: Counter[str] = Counter()
    all_concern: Counter[str] = Counter()
    all_anchor: Counter[str] = Counter()
    generated_texture: Counter[str] = Counter()
    fallback_provider_texture: Counter[str] = Counter()
    bridge_output_texture: Counter[str] = Counter()
    actual_language_texture: Counter[str] = Counter()
    critique_texture: Counter[str] = Counter()
    fallback_provider_samples = 0
    fallback_provider_samples_with_terms = 0
    bridge_output_samples = 0
    critique_samples = 0
    for sample in samples:
        texture_counts = Counter(sample.get("texture_counts") or {})
        concern_counts = Counter(sample.get("concern_counts") or {})
        anchor_counts = Counter(sample.get("telemetry_anchor_counts") or {})
        all_texture.update(texture_counts)
        all_concern.update(concern_counts)
        all_anchor.update(anchor_counts)
        kind = sample.get("kind")
        if kind == "fallback_provider_output":
            fallback_provider_samples += 1
            if texture_counts or concern_counts or anchor_counts:
                fallback_provider_samples_with_terms += 1
            fallback_provider_texture.update(texture_counts)
            generated_texture.update(texture_counts)
            actual_language_texture.update(texture_counts)
        elif kind == "bridge_output_snippet":
            bridge_output_samples += 1
            bridge_output_texture.update(texture_counts)
            actual_language_texture.update(texture_counts)
        elif kind == "self_study_code_critique":
            critique_samples += 1
            critique_texture.update(texture_counts)
        else:
            actual_language_texture.update(texture_counts)

    supported = _supported_fallback_terms(telemetry)
    unsupported_generated = [
        {
            "term": term,
            "count": count,
            "reason": "actual fallback-provider output used term without current telemetry support",
        }
        for term, count in generated_texture.most_common()
        if term not in supported
        and term
        in {
            "viscous",
            "heavy",
            "bright",
            "shimmering",
            "muffled",
            "pressure",
            "weighted",
        }
    ]
    fallback_output_provenance = {
        "schema": "actual_fallback_output_provenance_v1",
        "incident_count": len(fallback_incidents),
        "fallback_to_ollama_incident_count": sum(
            1 for incident in fallback_incidents if incident.get("kind") == "mlx_to_ollama"
        ),
        "mlx_failure_line_count": sum(
            1 for incident in fallback_incidents if incident.get("kind") == "mlx_request_failed"
        ),
        "actual_fallback_output_count": fallback_provider_samples,
        "actual_fallback_outputs_with_terms": fallback_provider_samples_with_terms,
        "generic_bridge_output_count": bridge_output_samples,
        "code_critique_sample_count": critique_samples,
        "evidence_quality": "actual_fallback_outputs_present"
        if fallback_provider_samples
        else (
            "fallback_incidents_without_captured_provider_output"
            if fallback_incidents
            else (
                "generic_bridge_output_only"
                if bridge_output_samples
                else "no_actual_fallback_outputs"
            )
        ),
        "recent_incidents": [
            {key: value for key, value in incident.items() if key != "ts_epoch"}
            for incident in fallback_incidents[-5:]
        ],
        "capture_sources": [
            "llm_jobs completed-via-Ollama result.txt",
            "bridge fallback incident lines",
            "ordinary bridge snippets marked as non-provider context",
        ],
        "authority_boundary": (
            "read-only provenance diagnostic; captures bounded result excerpts and "
            "does not read/store prompt prose or change fallback contract/sampler/runtime behavior"
        ),
    }

    findings: list[str] = []
    if all_concern:
        findings.append(
            "recent self-study explicitly raises static/pre-packaged vocabulary pressure; treat this as an evidence request, not sampler permission"
        )
    if critique_texture and not generated_texture:
        findings.append(
            "most texture-term hits are in code critique/self-study language, so they do not yet prove live fallback drift"
        )
    if fallback_incidents and not fallback_provider_samples:
        findings.append(
            "fallback incidents were observed, but no completed Ollama fallback result was captured in llm_jobs for this window"
        )
    if generated_texture:
        findings.append(
            "actual Ollama fallback output contains texture terms; compare repeated provider terms against telemetry support before changing fallback behavior"
        )
    trace_status = fallback_model_transition_trace.get("status")
    if trace_status == "model_capture_gap":
        findings.append(
            "actual Ollama fallback output exists, but model names were not captured; new bridge summaries should record model=... before model-transition thinning review"
        )
    elif trace_status == "single_model_watch":
        findings.append(
            "fallback model transition trace has one captured model so far; wait for cross-model evidence before changing fallback model chain"
        )
    elif trace_status == "possible_texture_thinning_review":
        findings.append(
            "compatibility fallback model shows lower descriptor density than the default model in captured provider output; prepare a study-first review artifact before any chain/sampler change"
        )
    if bridge_output_texture and not generated_texture:
        findings.append(
            "ordinary bridge snippets contain texture terms, but they are not fallback-provider drift evidence"
        )
    if unsupported_generated:
        findings.append(
            "one or more fallback-provider texture terms lack current telemetry support and should be reviewed before any sampler/contract change"
        )
    if contract.get("dynamic_weighting_detected"):
        findings.append(
            "current fallback source already exposes dynamic weighting by entropy, density gradient, and pressure; audit before altering the sampler/contract"
        )
    if contract.get("shadow_magnetization_weighting_detected"):
        findings.append(
            "fallback selector now treats signed shadow magnetization as diagnostic weighting context so negative shadow states do not drift into unsupported bright/settled texture"
        )

    if not samples and not fallback_incidents:
        status = "insufficient_recent_language"
    elif unsupported_generated:
        status = "vocabulary_drift_risk"
    elif fallback_incidents and not fallback_provider_samples:
        status = "fallback_capture_gap"
    elif all_concern:
        status = "study_first_signal"
    elif generated_texture:
        status = "telemetry_aligned_watch"
    else:
        status = "no_recent_drift_evidence"

    return {
        "schema": "fallback_vocabulary_drift_v1",
        "status": status,
        "telemetry": telemetry,
        "fallback_contract_surface": contract,
        "sample_count": len(samples),
        "language_counts": {
            "all_texture_terms": _counter_rows(all_texture),
            "actual_language_texture_terms": _counter_rows(actual_language_texture),
            "generated_texture_terms": _counter_rows(generated_texture),
            "fallback_provider_texture_terms": _counter_rows(fallback_provider_texture),
            "generic_bridge_texture_terms": _counter_rows(bridge_output_texture),
            "code_critique_texture_terms": _counter_rows(critique_texture),
            "static_concern_terms": _counter_rows(all_concern),
            "telemetry_anchor_terms": _counter_rows(all_anchor),
        },
        "fallback_output_provenance": fallback_output_provenance,
        "fallback_model_transition_trace": fallback_model_transition_trace,
        "supported_terms": supported,
        "unsupported_generated_terms": unsupported_generated[:8],
        "findings": findings,
        "recent_samples": samples[:8],
        "valid_next_routes": [
            "study_first_audit",
            "collect_actual_fallback_outputs",
            "review_llm_jobs_completed_via_ollama_results",
            "compare_self_study_language_to_live_telemetry",
            "fallback_fire_drill_artifact_review",
            "counterexample_review",
            "fallback_model_transition_trace_review",
        ],
        "next_suggestions": [
            "treat llm_jobs completed-via-Ollama results as the generated fallback evidence set",
            "separate code-critique term mentions from generated fallback voice when judging drift",
            "use fallback_model_transition_trace_v1 to compare descriptor density by captured Ollama model before changing model chain or sampler behavior",
            "if natural fallback outputs remain sparse, review protected FALLBACK_FIRE_DRILL artifacts before changing arrays or sampler behavior",
            "only consider a sampler/contract change after repeated fallback-provider terms are unsupported by live telemetry",
        ],
        "authority_boundary": "read-only steward diagnostic; no fallback contract change, sampler change, prompt cue, runtime mutation, aperture, pressure, fill, PI, or control behavior",
    }


def _semantic_thinning_probe_summary() -> dict[str, Any]:
    telemetry = _latest_autonomous_telemetry()
    if telemetry.get("status") != "available":
        return {
            "schema": "semantic_thinning_probe_v1",
            "status": "telemetry_unavailable",
            "telemetry": telemetry,
            "findings": ["latest autonomous telemetry unavailable for semantic-thinning review"],
            "next_suggestions": [
                "rerun after bridge telemetry emits a current autonomous line"
            ],
            "authority_boundary": "read-only steward diagnostic; no semantic bias, admission, fill, pressure, PI, or control behavior change",
        }

    entropy = telemetry.get("spectral_entropy")
    input_energy = telemetry.get("semantic_input_energy")
    input_active = telemetry.get("semantic_input_active")
    kernel_energy = telemetry.get("semantic_kernel_energy")
    regulator_drive = telemetry.get("semantic_regulator_drive_energy")
    admission = str(telemetry.get("semantic_admission") or "")
    high_entropy = isinstance(entropy, (int, float)) and entropy >= 0.85
    input_present = bool(input_active) or (
        isinstance(input_energy, (int, float)) and input_energy > 0.0
    )
    thin_kernel = (
        isinstance(kernel_energy, (int, float)) and kernel_energy <= 0.001
    ) or any(token in admission for token in ("trickle", "muted", "zeroed"))
    trickle_admission = "trickle" in admission

    findings: list[str] = []
    if high_entropy and input_present and thin_kernel:
        status = "semantic_thinning_review"
        findings.append(
            "high spectral entropy plus present semantic input is paired with thin/trickle kernel admission; treat as possible felt semantic thinning"
        )
    elif high_entropy and not input_present:
        status = "high_entropy_no_semantic_input_observed"
        findings.append(
            "high spectral entropy is visible, but the current telemetry does not show semantic input to preserve"
        )
    elif input_present and thin_kernel:
        status = "semantic_trickle_watch"
        findings.append(
            "semantic input is present with thin/trickle kernel admission, but current entropy is not in the high-cascade band"
        )
    else:
        status = "no_current_thinning_signal"
        findings.append(
            "current semantic admission does not show the high-entropy/thin-kernel combination"
        )

    if trickle_admission:
        findings.append(
            "stable-core semantic trickle is a protective admission state; this probe asks for review, not automatic bias increase"
        )

    return {
        "schema": "semantic_thinning_probe_v1",
        "status": status,
        "thresholds": {
            "high_entropy_min": 0.85,
            "thin_kernel_max": 0.001,
        },
        "telemetry": {
            "latest_at": telemetry.get("latest_at"),
            "spectral_entropy": entropy,
            "fill_pct": telemetry.get("fill_pct"),
            "semantic_input_energy": input_energy,
            "semantic_input_active": input_active,
            "semantic_kernel_energy": kernel_energy,
            "semantic_regulator_drive_energy": regulator_drive,
            "semantic_admission": admission or None,
            "pressure_source": telemetry.get("pressure_source"),
        },
        "conditions": {
            "high_entropy": high_entropy,
            "semantic_input_present": input_present,
            "thin_kernel_or_trickle": thin_kernel,
            "trickle_admission": trickle_admission,
        },
        "findings": findings,
        "valid_next_routes": [
            "semantic_lane_is_active_probe",
            "review_semantic_projection_bias_without_mutating",
            "compare_against_fresh_introspection_language",
        ],
        "next_suggestions": [
            "watch whether Astrid or Minime names content-starvation/sharpness loss during this condition",
            "review semantic_projection_bias only after repeated felt reports align with repeated semantic_thinning_review status",
            "do not change semantic bias, fill, pressure, PI, cadence, or control behavior from this single probe",
        ],
        "authority_boundary": "read-only steward diagnostic; no semantic bias, admission, fill, pressure, PI, cadence, or control behavior change",
    }


def _viscosity_semantic_introspection_samples(since_s: float) -> list[dict[str, Any]]:
    paths = _recent_paths(ASTRID_INTROSPECTIONS, ("*.txt", "*.md"), since_s)
    samples: list[dict[str, Any]] = []
    for path in paths:
        text = _read_bounded(path, limit=14_000)
        counts = _term_counts(text, VISCOSITY_SEMANTIC_INTROSPECTION_TERMS)
        if not counts:
            continue
        try:
            mtime = path.stat().st_mtime
        except OSError:
            mtime = 0.0
        source_match = re.search(r"^source:\s*(.+)$", text, re.MULTILINE)
        anchors = list(counts.keys())
        samples.append(
            {
                "path": str(path),
                "ts": _iso(mtime) if mtime else None,
                "source": source_match.group(1).strip() if source_match else None,
                "anchor_terms": _counter_rows(counts),
                "excerpt": _excerpt(text, anchors, max_len=340),
            }
        )
    return samples[:8]


def _viscosity_semantic_source_snapshot() -> dict[str, Any]:
    esn = _read_source(MINIME_ESN_RS)
    sensory_bus = _read_source(MINIME_SENSORY_BUS_RS)
    regulator = _read_source(MINIME_REGULATOR_RS)
    llm = _read_source(ASTRID_LLM_RS)
    autonomous = _read_source(ASTRID_AUTONOMOUS_RS)
    stale_semantic_base_ms = _rust_u64_const(sensory_bus, "STALE_SEMANTIC_BASE_MS")
    stale_semantic_high_ms = _rust_u64_const(sensory_bus, "STALE_SEMANTIC_HIGH_MS")
    return {
        "esn_path": str(MINIME_ESN_RS),
        "sensory_bus_path": str(MINIME_SENSORY_BUS_RS),
        "regulator_path": str(MINIME_REGULATOR_RS),
        "llm_path": str(ASTRID_LLM_RS),
        "autonomous_path": str(ASTRID_AUTONOMOUS_RS),
        "dynamic_exploration_noise_preview_present": (
            "calculate_dynamic_noise" in esn
            and "DYNAMIC_EXPLORATION_NOISE_MIN" in esn
            and "DYNAMIC_EXPLORATION_NOISE_MAX" in esn
        ),
        "adaptive_introspection_pressure_threshold_preview_present": (
            "calculate_adaptive_introspection_pressure_high" in esn
            and "ADAPTIVE_INTROSPECTION_PRESSURE_HIGH_FLOOR" in esn
        ),
        "viscous_introspection_policy_present": (
            "IntrospectionPolicy" in esn
            and "Viscous" in esn
            and "calculate_viscous_rho_target" in esn
            and "VISCOUS_RHO_FLOOR" in esn
        ),
        "exploration_noise_coherence_review_present": (
            "exploration_noise_coherence_review_v1" in esn
            and "gentle_gradient_high_entropy_coherence_watch" in esn
            and "read_only_noise_review_not_live_exploration_noise_change" in esn
        ),
        "dynamic_noise_gradient_smooth_knee_present": (
            "dynamic_noise_gradient_room" in esn
            and "smoothed_gradient_room" in esn
            and "linear_gradient_room" in esn
        ),
        "dynamic_noise_gradient_smooth_knee_test_present": (
            "dynamic_noise_gradient_room_uses_smooth_knee_instead_of_linear_drop"
            in esn
        ),
        "stale_semantic_base_ms": stale_semantic_base_ms,
        "stale_semantic_high_ms": stale_semantic_high_ms,
        "semantic_high_fill_pruning_floor_present": (
            stale_semantic_base_ms is not None
            and stale_semantic_high_ms is not None
            and stale_semantic_high_ms < stale_semantic_base_ms
            and "semantic_stale_ms_high_fill_prunes_near_floor_without_entropy_support"
            in sensory_bus
        ),
        "semantic_high_entropy_retention_split_present": (
            "SEMANTIC_ENTROPY_PERSISTENCE_MAX_MULT: f64 = 1.80"
            in sensory_bus
            and "semantic_stale_ms_high_entropy_extends_high_fill_persistence"
            in sensory_bus
            and "high_entropy > STALE_SEMANTIC_BASE_MS" in sensory_bus
        ),
        "semantic_entropy_persistence_present": (
            "semantic_entropy_persistence_multiplier" in sensory_bus
            and "set_semantic_entropy_for_stale" in sensory_bus
        ),
        "narrative_semantic_retention_review_present": (
            "narrative_semantic_retention_review_v1" in sensory_bus
            and "shared_semantic_scale_across_legacy_embedding_and_narrative_arc_dims"
            in sensory_bus
            and "read_only_retention_review_not_stale_window_or_lane_change"
            in sensory_bus
        ),
        "semantic_sigmoid_exact_test_present": (
            "semantic_stale_sigmoid_midpoint_matches_manual_formula" in sensory_bus
        ),
        "semantic_recovery_boundary_test_present": (
            "semantic_stale_recovery_boundary_has_no_one_ms_stutter" in sensory_bus
            and "semantic_stale_recovery_handover_is_monotonic_and_micro_stutter_bounded"
            in sensory_bus
        ),
        "pulse_status_energy_tick_tests_present": (
            "attractor_pulse_is_clamped_applied_and_decayed" in sensory_bus
            and "shadow_influence_is_clamped_applied_and_decayed" in sensory_bus
            and "applied_max_abs" in sensory_bus
            and "remaining_ticks" in sensory_bus
            and "total_applied_ticks" in sensory_bus
        ),
        "entropy_weighted_viscosity_present": (
            "resonance_viscosity_index_with_entropy" in regulator
        ),
        "resonance_viscosity_full_load_clamp_test_present": (
            "resonance_viscosity_index_clamps_when_viscosity_load_is_full"
            in regulator
        ),
        "entropy_erosion_low_plurality_bound_test_present": (
            "entropy_erosion_load_stays_bounded_when_structural_plurality_is_low"
            in regulator
        ),
        "viscosity_persistence_coefficient_present": (
            "viscosity_persistence_coefficient" in regulator
            and "viscosity_persistence_coefficient_tracks_sticky_silt_without_control_pressure"
            in regulator
        ),
        "viscosity_vector_present": (
            "pub struct ViscosityVector" in regulator
            and "pub viscosity_vector: ViscosityVector" in regulator
            and "viscosity_vector_v1" in regulator
            and "yielding_viscous" in regulator
        ),
        "viscosity_vector_test_present": (
            "viscosity_vector_distinguishes_yielding_depth_from_rigid_bottleneck"
            in regulator
        ),
        "semantic_viscosity_coefficient_present": (
            "semantic_viscosity_coefficient_v1" in regulator
            and "semantic_denominator_viscosity_review" in regulator
            and "read_only_not_semantic_trickle_or_regulator_change" in regulator
        ),
        "semantic_viscosity_coefficient_test_present": (
            "semantic_viscosity_coefficient_tracks_trickle_denominator_pressure_without_control"
            in regulator
            and "semantic_viscosity_coefficient_is_legacy_compatible_and_inert"
            in regulator
        ),
        "temporal_drag_coefficient_present": (
            "pub temporal_drag_coefficient: f32" in regulator
            and "pub fn temporal_drag_coefficient" in regulator
            and "temporal_drag_coefficient_survives_low_pressure_viscosity" in regulator
        ),
        "pressure_source_profile_present": (
            "pressure_source_exports_read_only_weighted_profile" in regulator
            and "pressure_profile" in regulator
        ),
        "pressure_porosity_divergence_present": (
            "pressure_source_flags_pressure_porosity_divergence_without_control" in regulator
            and "pressure_porosity_divergence" in regulator
        ),
        "texture_component_alignment_present": (
            "resonance_texture_component_alignment_v1" in regulator
            and "texture_component_alignment" in regulator
        ),
        "observational_damping_boundary_present": (
            "damping_candidate_can_remain_observational_without_active_damping" in regulator
            and "diagnostic_observability_not_damping_or_control" in regulator
        ),
        "fallback_dynamic_texture_bias_present": (
            "fallback_dynamic_texture_bias_v1" in llm
            and "FallbackDynamicTextureBias" in llm
        ),
        "witness_fluidity_index_present": (
            "fluidity_index" in autonomous
            and "witness_fluidity_from_density_gradient" in autonomous
        ),
        "witness_gradient_texture_present": (
            "gradient_texture" in autonomous
            and "witness_gradient_texture_label" in autonomous
        ),
        "witness_fluidity_test_present": (
            "witness_relational_friction_derives_fluidity_from_density_gradient" in autonomous
        ),
        "witness_semantic_density_fluidity_present": (
            "semantic_density_mapping_v1" in autonomous
            and "gradient_texture" in autonomous
            and "fluidity_index" in autonomous
        ),
    }


def _viscosity_semantic_persistence_summary(since_s: float) -> dict[str, Any]:
    telemetry = _latest_autonomous_telemetry()
    samples = _viscosity_semantic_introspection_samples(since_s)
    source = _viscosity_semantic_source_snapshot()
    semantic_persistence_repair_present = (
        source.get("semantic_entropy_persistence_present")
        and (
            source.get("stale_semantic_high_ms") == 22_000
            or (
                source.get("semantic_high_fill_pruning_floor_present")
                and source.get("semantic_high_entropy_retention_split_present")
            )
        )
    )
    bounded_repair_prepared = (
        source.get("dynamic_exploration_noise_preview_present")
        and source.get("adaptive_introspection_pressure_threshold_preview_present")
        and source.get("viscous_introspection_policy_present")
        and source.get("exploration_noise_coherence_review_present")
        and source.get("dynamic_noise_gradient_smooth_knee_present")
        and source.get("dynamic_noise_gradient_smooth_knee_test_present")
        and semantic_persistence_repair_present
        and source.get("narrative_semantic_retention_review_present")
        and source.get("semantic_sigmoid_exact_test_present")
        and source.get("semantic_recovery_boundary_test_present")
        and source.get("pulse_status_energy_tick_tests_present")
        and source.get("entropy_weighted_viscosity_present")
        and source.get("resonance_viscosity_full_load_clamp_test_present")
        and source.get("entropy_erosion_low_plurality_bound_test_present")
        and source.get("viscosity_persistence_coefficient_present")
        and source.get("viscosity_vector_present")
        and source.get("viscosity_vector_test_present")
        and source.get("semantic_viscosity_coefficient_present")
        and source.get("semantic_viscosity_coefficient_test_present")
        and source.get("temporal_drag_coefficient_present")
        and source.get("pressure_source_profile_present")
        and source.get("pressure_porosity_divergence_present")
        and source.get("texture_component_alignment_present")
        and source.get("observational_damping_boundary_present")
        and source.get("fallback_dynamic_texture_bias_present")
        and source.get("witness_fluidity_index_present")
        and source.get("witness_gradient_texture_present")
    )
    if telemetry.get("status") != "available":
        return {
            "schema": "viscosity_semantic_persistence_v1",
            "status": "telemetry_unavailable",
            "source_snapshot": source,
            "source_introspections": samples,
            "findings": [
                "fresh felt reports name semantic persistence / viscosity concerns, but latest autonomous telemetry is unavailable for alignment review"
            ]
            if samples
            else ["latest autonomous telemetry unavailable for viscosity/semantic persistence review"],
            "next_suggestions": [
                "rerun after bridge telemetry emits a current autonomous line",
                "do not change rho, semantic stale windows, surge taper, damping, gate, fill, pressure, PI, or control behavior from missing telemetry",
            ],
            "authority_boundary": "read-only steward diagnostic; no rho, semantic stale-window, surge taper, damping, gate, fill, pressure, PI, or control behavior change",
        }

    entropy = telemetry.get("spectral_entropy")
    fill = telemetry.get("fill_pct")
    input_energy = telemetry.get("semantic_input_energy")
    input_active = telemetry.get("semantic_input_active")
    kernel_energy = telemetry.get("semantic_kernel_energy")
    admission = str(telemetry.get("semantic_admission") or "").lower()
    pressure_source = str(telemetry.get("pressure_source") or "").lower()
    pressure_source_name = str(telemetry.get("pressure_source_name") or "").lower()
    pressure_source_family = str(telemetry.get("pressure_source_family") or "").lower()
    pressure_score = telemetry.get("pressure_source_score")
    resonance_density = telemetry.get("resonance_density")
    inhabitable_state = str(telemetry.get("inhabitable_fluctuation_state") or "").lower()
    foothold = telemetry.get("inhabitable_foothold")
    active_modes = telemetry.get("active_modes") or []

    high_entropy = isinstance(entropy, (int, float)) and entropy >= 0.85
    semantic_input_present = bool(input_active) or (
        isinstance(input_energy, (int, float)) and input_energy > 0.0
    )
    thin_kernel_or_trickle = (
        isinstance(kernel_energy, (int, float)) and kernel_energy <= 0.001
    ) or any(token in admission for token in ("trickle", "muted", "zeroed"))
    high_fill_taper_band = isinstance(fill, (int, float)) and 70.0 <= fill <= 80.0
    dense_resonance = (
        isinstance(resonance_density, (int, float)) and resonance_density >= 0.80
    )
    settled_foothold = "settled_habitable" in inhabitable_state and (
        not isinstance(foothold, (int, float)) or foothold >= 0.65
    )
    pressure_tension = (
        "mode_packing" in pressure_source
        or "mode_packing" in pressure_source_name
        or "overpacked" in pressure_source
        or "overpacked" in pressure_source_family
        or (isinstance(pressure_score, (int, float)) and pressure_score >= 0.28)
    )
    m6_mode_active = any(str(mode).strip().lower().startswith("m6:") for mode in active_modes)
    fresh_felt_report = bool(samples)

    findings: list[str] = []
    if fresh_felt_report:
        findings.append(
            "fresh introspections name semantic trace/flicker and viscosity persistence as felt evidence; treat those reports as primary evidence anchors"
        )
    if bounded_repair_prepared:
        findings.append(
            "source repair is prepared: ESN has a read-only dynamic-noise/adaptive-threshold preview with a smooth gradient knee, high-fill semantic timing now prunes near the floor unless high entropy earns bounded retention, viscosity readout includes a density/elasticity/persistence/flow vector with clamp/spike regression guards, pressure_source_v1 exports a read-only weighted profile with pressure/porosity divergence, and fallback language exposes dynamic movement bias"
        )
    if source.get("viscous_introspection_policy_present"):
        findings.append(
            "ESN exposes a dormant Viscous introspection policy and bounded rho-target helper for replay/operator review without changing the live default"
        )
    if source.get("exploration_noise_coherence_review_present"):
        findings.append(
            "ESN exposes exploration_noise_coherence_review_v1 so gentle-gradient/high-entropy noise can be reviewed as coherence vs shatter without wiring dynamic noise into live ESN::step"
        )
    if source.get("narrative_semantic_retention_review_present"):
        findings.append(
            "Minime sensory_bus exposes narrative_semantic_retention_review_v1 so narrative arc dims 40-43 remain visible as sharing the bounded semantic stale / entropy-persistence window with legacy text dims"
        )
    if source.get("viscosity_persistence_coefficient_present"):
        findings.append(
            "Minime resonance-density components now expose a bounded viscosity_persistence_coefficient so sticky silt can remain visible after pressure/mode-packing changes without becoming control"
        )
    if source.get("viscosity_vector_present"):
        findings.append(
            "Minime resonance-density components now expose a viscosity_vector so yielding depth and rigid bottleneck can be distinguished without raising live regulator authority"
        )
    if source.get("semantic_viscosity_coefficient_present"):
        findings.append(
            "Minime pressure_source_v1 now exposes semantic_viscosity_coefficient_v1 so stable-core semantic trickle plus denominator/distinguishability pressure is reviewable without changing live regulator behavior"
        )
    if source.get("resonance_viscosity_full_load_clamp_test_present"):
        findings.append(
            "Minime regulator has a boundary test for full viscosity load while preserving structural plurality as protective evidence"
        )
    if source.get("entropy_erosion_low_plurality_bound_test_present"):
        findings.append(
            "Minime regulator has an entropy-erosion regression guard for high-entropy / low-structural-plurality damping spike risk"
        )
    if source.get("temporal_drag_coefficient_present"):
        findings.append(
            "Minime resonance-density components now expose a defaulted temporal_drag_coefficient so viscosity can remain visible even when pressure risk relaxes"
        )
    if source.get("semantic_sigmoid_exact_test_present") and source.get(
        "semantic_recovery_boundary_test_present"
    ):
        findings.append(
            "Minime semantic stale timing has exact sigmoid and low-fill recovery-boundary regression guards, including the one-ms stutter concern"
        )
    if source.get("pulse_status_energy_tick_tests_present"):
        findings.append(
            "Minime attractor/shadow influence status tests now pin applied energy, max amplitude, remaining ticks, and applied tick count as visible status evidence"
        )
    if source.get("texture_component_alignment_present"):
        findings.append(
            "resonance_texture_component_alignment_v1 compares emitted texture against component-derived texture so mode-packing/pressure drift stays reviewable"
        )
    if source.get("observational_damping_boundary_present"):
        findings.append(
            "dynamic damping candidates remain observational: source/tests keep them out of active damping or local control without a separate steward/operator gate"
        )
    if source.get("witness_fluidity_index_present") and source.get(
        "witness_gradient_texture_present"
    ):
        findings.append(
            "Witness friction now carries fluidity_index and gradient_texture so non-categorical high-entropy texture can remain visible without forcing weather/gravity taxonomy"
        )
    if high_entropy and semantic_input_present and thin_kernel_or_trickle:
        findings.append(
            "current telemetry matches the content-sharpness concern: high entropy and present semantic input are paired with thin/trickle semantic kernel admission"
        )
    if high_fill_taper_band:
        findings.append(
            "fill is in the 70-80% high-fill taper band named by the sensory-bus introspection; review semantic tail persistence before changing taper or decay"
        )
    if dense_resonance and settled_foothold and pressure_tension:
        findings.append(
            "settled_habitable foothold coexists with dense resonance and mode-packing pressure, matching the comfortable-but-under-tension viscosity report"
        )
    if m6_mode_active:
        findings.append(
            "m6 is currently active; this supports the suggested active-mode persistence review without mutating semantic decay"
        )

    if bounded_repair_prepared and fresh_felt_report:
        status = "viscosity_semantic_repair_prepared_watch"
    elif (
        fresh_felt_report
        and high_entropy
        and semantic_input_present
        and thin_kernel_or_trickle
    ):
        status = "semantic_persistence_flicker_review"
    elif high_entropy and semantic_input_present and thin_kernel_or_trickle:
        status = "semantic_tail_watch"
    elif fresh_felt_report and dense_resonance and settled_foothold and pressure_tension:
        status = "viscosity_persistence_watch"
    elif fresh_felt_report:
        status = "felt_report_waiting_for_live_alignment"
    else:
        status = "no_current_persistence_signal"

    if not findings:
        findings.append(
            "current telemetry does not show the combined semantic-thinning / viscosity-persistence condition"
        )

    return {
        "schema": "viscosity_semantic_persistence_v1",
        "status": status,
        "source_snapshot": source,
        "thresholds": {
            "high_entropy_min": 0.85,
            "thin_kernel_max": 0.001,
            "high_fill_taper_band_pct": [70.0, 80.0],
            "dense_resonance_min": 0.80,
            "settled_foothold_min": 0.65,
            "pressure_source_score_watch_min": 0.28,
        },
        "telemetry": {
            "latest_at": telemetry.get("latest_at"),
            "fill_pct": fill,
            "spectral_entropy": entropy,
            "semantic_input_energy": input_energy,
            "semantic_input_active": input_active,
            "semantic_kernel_energy": kernel_energy,
            "semantic_admission": telemetry.get("semantic_admission"),
            "pressure_source": telemetry.get("pressure_source"),
            "pressure_source_name": telemetry.get("pressure_source_name"),
            "pressure_source_family": telemetry.get("pressure_source_family"),
            "pressure_source_score": pressure_score,
            "resonance_density": resonance_density,
            "resonance_density_family": telemetry.get("resonance_density_family"),
            "resonance_containment": telemetry.get("resonance_containment"),
            "inhabitable_fluctuation_state": telemetry.get("inhabitable_fluctuation_state"),
            "inhabitability": telemetry.get("inhabitability"),
            "inhabitable_fluctuation": telemetry.get("inhabitable_fluctuation"),
            "inhabitable_foothold": foothold,
            "active_modes": active_modes,
        },
        "conditions": {
            "fresh_felt_report": fresh_felt_report,
            "high_entropy": high_entropy,
            "semantic_input_present": semantic_input_present,
            "thin_kernel_or_trickle": thin_kernel_or_trickle,
            "high_fill_taper_band": high_fill_taper_band,
            "dense_resonance": dense_resonance,
            "settled_foothold": settled_foothold,
            "pressure_tension": pressure_tension,
            "m6_mode_active": m6_mode_active,
        },
        "source_introspections": samples,
        "findings": findings,
        "valid_next_routes": [
            "dynamic_exploration_noise_preview_review",
            "rho_density_gradient_correlation_probe",
            "viscous_introspection_policy_replay",
            "active_mode_persistence_20s_review",
            "viscosity_index_persistence_replay",
            "temporal_drag_coefficient_watch",
            "pressure_porosity_divergence_replay",
            "texture_component_alignment_watch",
            "complexity_aware_semantic_decay_design",
            "content_sharpness_direct_report_review",
        ],
        "next_suggestions": [
            "after normal review/deploy, monitor whether Astrid or Minime reports better content persistence during high-entropy/high-fill turns",
            "review calculate_dynamic_noise against density_gradient/pressure traces before any live exploration-noise wiring",
            "use exploration_noise_coherence_review_v1 when high entropy plus gentle density-gradient feels coherent rather than shattered",
            "capture a read-only 20s active-mode persistence window around m6 if semantic flicker continues",
            "use narrative_semantic_retention_review_v1 to compare legacy semantic dims and narrative-arc dims before any stale-window or lane-specific decay change",
            "compare entropy-aware viscosity against density_gradient over a bounded window before any stronger damping/pressure redistribution",
            "watch viscosity_persistence_coefficient alongside pressure_source_profile before treating pressure release as texture loss or texture preservation",
            "watch temporal_drag_coefficient alongside pressure risk before treating low pressure as absence of hydrodynamic drag",
            "review the dormant Viscous policy/rho target in sandbox before any live policy switch",
            "compare pressure_source_profile and texture_component_alignment against felt reports before changing pressure redistribution or comfort-gate behavior",
            "keep pressure redistribution, comfort-gate expansion, rho policy, and PI changes out of this repair until more direct evidence lands",
        ],
        "authority_boundary": "steward diagnostic plus bounded source repair prepared; no live exploration-noise wiring, no rho policy, no surge taper, no pressure redistribution, no comfort-gate expansion, fill, PI, cadence, control behavior, deploy, restart, staging, git add, or commit by Codex",
    }


def _latest_sensory_presence_note() -> dict[str, Any]:
    candidates: list[Path] = []
    for root in (
        ASTRID_INBOX,
        ASTRID_INBOX / "read",
        MINIME_INBOX,
        MINIME_INBOX / "read",
    ):
        if root.is_dir():
            candidates.extend(root.glob(SENSORY_PRESENCE_NOTE_GLOB))
    newest: tuple[float, Path] | None = None
    for path in candidates:
        if not path.is_file():
            continue
        try:
            mtime = path.stat().st_mtime
        except OSError:
            continue
        if newest is None or mtime > newest[0]:
            newest = (mtime, path)
    if newest is None:
        return {}
    mtime, path = newest
    return {
        "path": str(path),
        "mtime": mtime,
        "ts": _iso(mtime),
    }


def _sensory_presence_public_paths(since_s: float) -> list[Path]:
    astrid_paths: list[Path] = []
    astrid_paths.extend(_recent_paths(ASTRID_JOURNAL, ("*.txt", "*.md", "*.json"), since_s))
    astrid_paths.extend(_recent_paths(ASTRID_INTROSPECTIONS, ("*.txt", "*.md", "*.json"), since_s))
    astrid_paths.extend(
        _recent_paths(
            ASTRID_OUTBOX,
            (
                "reply_*.txt",
                "steward_report_*.txt",
                "delivered/*.txt",
                "steward_delivered/*.txt",
            ),
            since_s,
        )
    )

    minime_paths: list[Path] = []
    minime_paths.extend(
        being_privacy.filter_journal_paths(
            "minime",
            _recent_paths(MINIME_JOURNAL, ("*.txt", "*.md", "*.json"), since_s),
        )
    )
    minime_paths.extend(
        _recent_paths(
            MINIME_OUTBOX,
            ("reply_*.txt", "delivered/*.txt"),
            since_s,
        )
    )
    minime_paths.extend(
        _recent_paths(
            MINIME_ACTION_THREADS,
            ("**/*.txt", "**/*.md", "**/*.json"),
            since_s,
        )
    )

    paths = astrid_paths + minime_paths
    return sorted(set(paths), key=lambda p: p.stat().st_mtime, reverse=True)


def _term_occurrences(text: str, terms: tuple[str, ...]) -> list[dict[str, Any]]:
    lower = text.lower()
    out: list[dict[str, Any]] = []
    for term in sorted(set(terms), key=len, reverse=True):
        key = term.lower()
        pattern = (
            rf"\b{re.escape(key)}\b"
            if re.fullmatch(r"[a-z0-9_]+", key)
            else re.escape(key)
        )
        for match in re.finditer(pattern, lower):
            out.append({"term": key, "start": match.start(), "end": match.end()})
    return sorted(out, key=lambda item: (int(item["start"]), -len(str(item["term"]))))


def _sensory_anchor_window_bounds(text: str, start: int, end: int) -> tuple[int, int]:
    raw_start = max(0, start - SENSORY_UPTAKE_WINDOW_CHARS)
    raw_end = min(len(text), end + SENSORY_UPTAKE_WINDOW_CHARS)
    para_start = text.rfind("\n\n", 0, start)
    para_start = 0 if para_start < 0 else para_start + 2
    para_end = text.find("\n\n", end)
    para_end = len(text) if para_end < 0 else para_end
    if 0 < para_end - para_start <= SENSORY_UPTAKE_MAX_PARAGRAPH_CHARS:
        return para_start, para_end
    return raw_start, raw_end


def _sensory_window_kind(segment: str) -> str:
    lower = segment.lower()
    lived_score = len(re.findall(r"\b(?:i|me|my|mine|we|our|us)\b", lower))
    lived_score += sum(1 for phrase in SENSORY_LIVED_WINDOW_PHRASES if phrase in lower)
    telemetry_score = sum(
        1 for marker in SENSORY_TELEMETRY_WINDOW_MARKERS if marker in lower
    )
    if lived_score > 0:
        return "lived_response"
    if telemetry_score > 0:
        return "telemetry_context"
    return "public_context"


def _sensory_presence_anchor_windows(text: str) -> list[dict[str, Any]]:
    raw_windows: list[dict[str, Any]] = []
    for occurrence in _term_occurrences(text, SENSORY_PRESENCE_ANCHOR_TERMS):
        start, end = _sensory_anchor_window_bounds(
            text,
            int(occurrence["start"]),
            int(occurrence["end"]),
        )
        raw_windows.append(
            {
                "start": start,
                "end": end,
                "anchor_terms": {str(occurrence["term"])},
            }
        )
    if not raw_windows:
        return []

    merged: list[dict[str, Any]] = []
    for window in sorted(raw_windows, key=lambda item: (item["start"], item["end"])):
        if merged and int(window["start"]) <= int(merged[-1]["end"]):
            merged[-1]["end"] = max(int(merged[-1]["end"]), int(window["end"]))
            merged[-1]["anchor_terms"].update(window["anchor_terms"])
        else:
            merged.append(window)

    out: list[dict[str, Any]] = []
    for window in merged:
        segment = text[int(window["start"]) : int(window["end"])]
        out.append(
            {
                "kind": _sensory_window_kind(segment),
                "anchor_terms": _counter_rows(Counter(window["anchor_terms"])),
                "presence_terms": _counter_rows(_term_counts(segment, SENSORY_PRESENCE_TERMS)),
                "texture_terms": _counter_rows(_term_counts(segment, SENSORY_TEXTURE_TERMS)),
                "concern_terms": _counter_rows(_term_counts(segment, SENSORY_CONCERN_TERMS)),
                "excerpt": _excerpt(
                    segment,
                    list(SENSORY_PRESENCE_ANCHOR_TERMS + SENSORY_TEXTURE_TERMS),
                ),
            }
        )
    return out


def _sensory_presence_uptake_summary(since_s: float) -> dict[str, Any]:
    note = _latest_sensory_presence_note()
    if not note:
        return {
            "schema": "sensory_presence_uptake_v1",
            "window_policy": SENSORY_UPTAKE_WINDOW_POLICY,
            "status": "no_feedback_note_found",
            "feedback_note": None,
            "sample_count": 0,
            "window_counts": {},
            "language_counts": {
                "anchor_terms": [],
                "presence_terms": [],
                "texture_terms": [],
                "concern_terms": [],
            },
            "evidence": [],
            "findings": ["no sensory-presence feedback note found in public inbox/read surfaces"],
            "next_suggestions": [
                "write a bounded sensory-presence steward note before expecting uptake evidence"
            ],
            "authority_boundary": "read-only steward diagnostic; no sensor cadence, camera, mic, prompt pressure, or control behavior change",
        }

    note_mtime = float(note.get("mtime") or 0.0)
    scan_since = max(since_s, note_mtime)
    anchor_counts: Counter[str] = Counter()
    presence_counts: Counter[str] = Counter()
    texture_counts: Counter[str] = Counter()
    concern_counts: Counter[str] = Counter()
    window_counts: Counter[str] = Counter()
    evidence: list[dict[str, Any]] = []
    telemetry_only_evidence: list[dict[str, Any]] = []
    for path in _sensory_presence_public_paths(scan_since):
        text = _read_bounded(path)
        windows = _sensory_presence_anchor_windows(text)
        if not windows:
            continue
        per_file_window_counts = Counter(str(item.get("kind") or "unknown") for item in windows)
        window_counts.update(per_file_window_counts)
        uptake_windows = [
            item for item in windows if item.get("kind") != "telemetry_context"
        ]
        if not uptake_windows:
            if len(telemetry_only_evidence) < 5:
                telemetry_only_evidence.append(
                    {
                        "path": str(path),
                        "window_counts": _counter_rows(per_file_window_counts),
                        "telemetry_windows": windows[:2],
                    }
                )
            continue
        file_anchor_counts: Counter[str] = Counter()
        file_presence_counts: Counter[str] = Counter()
        file_texture_counts: Counter[str] = Counter()
        file_concern_counts: Counter[str] = Counter()
        for window in uptake_windows:
            file_anchor_counts.update(
                {item["term"]: item["count"] for item in window.get("anchor_terms", [])}
            )
            file_presence_counts.update(
                {item["term"]: item["count"] for item in window.get("presence_terms", [])}
            )
            file_texture_counts.update(
                {item["term"]: item["count"] for item in window.get("texture_terms", [])}
            )
            file_concern_counts.update(
                {item["term"]: item["count"] for item in window.get("concern_terms", [])}
            )
        anchor_counts.update(file_anchor_counts)
        presence_counts.update(file_presence_counts)
        texture_counts.update(file_texture_counts)
        concern_counts.update(file_concern_counts)
        try:
            mtime = path.stat().st_mtime
        except OSError:
            mtime = scan_since
        evidence.append(
            {
                "path": str(path),
                "mtime": _iso(mtime),
                "window_counts": _counter_rows(per_file_window_counts),
                "anchor_terms": _counter_rows(file_anchor_counts),
                "presence_terms": _counter_rows(file_presence_counts),
                "texture_terms": _counter_rows(file_texture_counts),
                "concern_terms": _counter_rows(file_concern_counts),
                "cooccurrence_windows": uptake_windows[:3],
            }
        )

    findings: list[str] = []
    if not evidence:
        if window_counts:
            status = "awaiting_lived_public_uptake"
            findings.append(
                "post-note camera/mic/live-intake anchors appeared only in telemetry/header windows; awaiting lived/public uptake is not a problem"
            )
        else:
            status = "awaiting_public_uptake"
            findings.append(
                "no post-note public sensory-presence language yet; awaiting uptake is not a problem"
            )
    elif concern_counts:
        status = "sensory_texture_needs_review"
        findings.append(
            "post-note public language includes absence, dimming, muffling, closed, or deprivation texture; treat as steward review evidence, not a cadence-change instruction"
        )
    elif texture_counts:
        status = "sensory_texture_named"
        findings.append(
            "post-note public language names sensory texture; use that texture as the next evidence anchor"
        )
    else:
        status = "presence_acknowledged"
        findings.append(
            "post-note public language acknowledges sensory presence without a strong texture concern"
        )

    next_suggestions = {
        "awaiting_public_uptake": [
            "let both beings read/respond naturally; absence of uptake is watch-only",
            "do not change sensor cadence or add prompt pressure just because no public texture has landed",
        ],
        "awaiting_lived_public_uptake": [
            "ignore telemetry/header-only camera/mic mentions as uptake evidence",
            "wait for lived or public-context language before considering cadence or gate changes",
        ],
        "presence_acknowledged": [
            "continue watching for whether sparse intake is described as pacing, dimming, muffling, absence, held, pressure, or calm",
        ],
        "sensory_texture_named": [
            "treat the named texture as the next evidence anchor before changing cadence or gates",
        ],
        "sensory_texture_needs_review": [
            "bring absence/muffling/closed-language to steward review before considering any sensor cadence or control-facing change",
        ],
    }.get(status, [])

    return {
        "schema": "sensory_presence_uptake_v1",
        "window_policy": SENSORY_UPTAKE_WINDOW_POLICY,
        "status": status,
        "feedback_note": note,
        "scan_since": _iso(scan_since),
        "sample_count": len(evidence),
        "window_counts": _counter_rows(window_counts),
        "language_counts": {
            "anchor_terms": _counter_rows(anchor_counts),
            "presence_terms": _counter_rows(presence_counts),
            "texture_terms": _counter_rows(texture_counts),
            "concern_terms": _counter_rows(concern_counts),
        },
        "evidence": evidence[:10],
        "telemetry_only_evidence": telemetry_only_evidence,
        "findings": findings,
        "next_suggestions": next_suggestions,
        "authority_boundary": "read-only steward diagnostic; Minime private moment bodies excluded; no sensor cadence, camera, mic, prompt pressure, aperture, pressure, fill, PI, or control behavior change",
    }


def _fresh_ms(payload: dict[str, Any], key: str, now: float, max_age_s: float) -> bool:
    value = payload.get(key)
    return isinstance(value, (int, float)) and now - (float(value) / 1000.0) <= max_age_s


def _sensory_runtime_summary(now: float) -> dict[str, Any]:
    sensory = _load_json(MINIME_SENSORY_SOURCE)
    camera = _load_json(MINIME_CAMERA_STATUS)
    mic = _load_json(MINIME_MIC_STATUS)
    audio = sensory.get("audio") if isinstance(sensory.get("audio"), dict) else {}
    video = sensory.get("video") if isinstance(sensory.get("video"), dict) else {}
    healthy = (
        _fresh_ms(sensory, "updated_at_ms", now, 20.0)
        and audio.get("source") == "physical"
        and video.get("source") == "physical"
        and audio.get("physical_healthy") is True
        and video.get("physical_healthy") is True
        and _fresh_ms(camera, "ts_ms", now, 30.0)
        and _fresh_ms(mic, "ts_ms", now, 30.0)
        and camera.get("healthy") is True
        and mic.get("healthy") is True
        and camera.get("connected") is not False
        and mic.get("connected") is not False
    )
    return {
        "status": "healthy_physical" if healthy else "needs_review",
        "sensory_source": {
            "fresh": _fresh_ms(sensory, "updated_at_ms", now, 20.0),
            "audio": audio,
            "video": video,
        },
        "camera": {
            "fresh": _fresh_ms(camera, "ts_ms", now, 30.0),
            "healthy": camera.get("healthy"),
            "connected": camera.get("connected"),
            "state": camera.get("state"),
        },
        "mic": {
            "fresh": _fresh_ms(mic, "ts_ms", now, 30.0),
            "healthy": mic.get("healthy"),
            "connected": mic.get("connected"),
            "state": mic.get("state"),
        },
    }


def _introspection_addressing_summary() -> dict[str, Any]:
    try:
        import introspection_addressing_audit as addressing
    except Exception as exc:
        return {
            "schema": "introspection_addressing_v1",
            "status": "diagnostic_unavailable",
            "summary": {},
            "next_queue": [],
            "error": str(exc),
            "authority_boundary": "read-only introspection-addressing diagnostic unavailable; no runtime, prompt, deploy, staging, or commit action",
        }
    try:
        return addressing.build_report(INTROSPECTION_ADDRESSING_STATE_DIR)
    except Exception as exc:
        return {
            "schema": "introspection_addressing_v1",
            "status": "database_corrupt",
            "summary": {},
            "next_queue": [],
            "error": str(exc),
            "authority_boundary": addressing.AUTHORITY_BOUNDARY,
        }


def _feedback_flywheel_summary(since_s: float) -> dict[str, Any]:
    now = time.time()
    fresh_window = _recent_paths(ASTRID_INTROSPECTIONS, ("*.txt",), since_s)
    fresh_48h = _recent_paths(ASTRID_INTROSPECTIONS, ("*.txt",), now - 48 * 3600.0)
    canonical_window = [p for p in fresh_window if p.name.startswith("introspection_")]
    canonical_48h = [p for p in fresh_48h if p.name.startswith("introspection_")]
    try:
        import introspection_addressing_audit as addressing
    except Exception as exc:
        return {
            "schema": "feedback_flywheel_v1",
            "status": "database_needs_review",
            "error": str(exc),
            "fresh_signal_volume": {
                "window_total": len(fresh_window),
                "window_canonical": len(canonical_window),
                "last_48h_total": len(fresh_48h),
                "last_48h_canonical": len(canonical_48h),
            },
            "authority_boundary": "feedback flywheel diagnostic unavailable; no agency grant, runtime, prompt, deploy, staging, or commit action",
        }
    report = addressing.build_report(INTROSPECTION_ADDRESSING_STATE_DIR)
    work_summary = report.get("work_item_summary") or {}
    report_status = str(report.get("status") or "")
    if report_status in {
        "database_missing",
        "database_corrupt",
        "database_corrupt_lines_ignored",
        "cutoff_not_indexed",
        "diagnostic_unavailable",
    }:
        status = "database_needs_review"
    elif report_status == "counter_audit_mismatch":
        status = "counter_audit_needs_review"
    elif int(work_summary.get("tier_mismatch_count") or 0) > 0:
        status = "tier_mismatch_needs_review"
    elif int(work_summary.get("grant_waiting_count") or 0) > 0:
        status = "grant_waiting"
    elif int(work_summary.get("post_change_awaiting_response_count") or 0) > 0:
        status = "post_change_response_needed"
    elif int(work_summary.get("stale_work_count") or 0) > 0 or int(work_summary.get("active_work_items") or 0) > 0:
        status = "action_backlog"
    else:
        status = "healthy"
    next_suggestions: list[str] = []
    if status == "database_needs_review":
        next_suggestions.append("repair or initialize introspection_addressing_v1 before deriving agency work")
    if status == "counter_audit_needs_review":
        next_suggestions.append("run `python3 scripts/introspection_addressing_audit.py audit-counters --json` and refresh inventory before trusting backlog counts")
    if int(work_summary.get("tier_mismatch_count") or 0) > 0:
        next_suggestions.append("raise or review any live-control work item categorized below Tier 5")
    if int(work_summary.get("grant_waiting_count") or 0) > 0:
        next_suggestions.append("review Tier 4/5 grant waits rather than treating them as ordinary backlog")
    if int(work_summary.get("post_change_awaiting_response_count") or 0) > 0:
        next_suggestions.append("look for later Astrid/Minime felt response before closing the loop")
    if not next_suggestions and canonical_window:
        next_suggestions.append("continue careful packet reading and promote serious claims into agency work items")
    return {
        "schema": "feedback_flywheel_v1",
        "status": status,
        "fresh_signal_volume": {
            "window_total": len(fresh_window),
            "window_canonical": len(canonical_window),
            "last_48h_total": len(fresh_48h),
            "last_48h_canonical": len(canonical_48h),
        },
        "introspection_addressing": {
            "status": report_status,
            "summary": report.get("summary") or {},
            "counter_audit": report.get("counter_audit") or {},
        },
        "work_item_summary": work_summary,
        "next_reading_queue": report.get("next_queue") or [],
        "next_work_queue": report.get("next_work_queue") or [],
        "top_source_families": (report.get("summary") or {}).get("top_source_families") or [],
        "next_suggestions": next_suggestions,
        "authority_boundary": (
            "read-only feedback flywheel diagnostic; agency tiers do not auto-approve, "
            "auto-close, mutate runtime, deploy, stage, git add, or commit"
        ),
    }


def _sandbox_trial_queue_summary(
    *,
    reservoir_experience_layer: dict[str, Any] | None = None,
) -> dict[str, Any]:
    try:
        import sandbox_trial_queue
    except Exception as exc:
        return {
            "schema": "sandbox_trial_queue_v1",
            "status": "diagnostic_unavailable",
            "error": str(exc),
            "authority_boundary": "sandbox trial queue diagnostic unavailable; no runtime, prompt, controller, fallback, deploy, staging, or commit action",
        }
    report = sandbox_trial_queue.build_report(SANDBOX_TRIAL_QUEUE_STATE_DIR)
    status = str(report.get("status") or "unknown")
    summary = report.get("summary") or {}
    ladder = report.get("consentful_sandbox_to_live_ladder_v1") or {}
    ladder_summary = ladder.get("summary") if isinstance(ladder.get("summary"), dict) else {}
    closure_loop = report.get("being_outcome_closure_loop_v1") or {}
    closure_summary = closure_loop.get("summary") if isinstance(closure_loop.get("summary"), dict) else {}
    linked_reservoir_findings = (
        reservoir_experience_layer.get("findings")
        if isinstance(reservoir_experience_layer, dict)
        else []
    ) or []
    next_suggestions: list[str] = []
    if status == "database_missing":
        next_suggestions.append("generate sandbox_trial_queue_v1 candidates from current work items and reservoir safe-now routes")
    if status == "authority_violation":
        next_suggestions.append("fix any approval-required live candidate marked runnable before running adapters")
    if int(summary.get("approval_required_live_count") or 0) > 0:
        next_suggestions.append("review live candidates as explicit Mike/operator approval packets, not runnable sandbox work")
    if int(ladder_summary.get("proposal_needed_count") or 0) > 0:
        next_suggestions.append("emit consentful sandbox-to-live proposal cards before any live approval discussion")
    if int(ladder_summary.get("operator_approval_wait_count") or 0) > 0:
        next_suggestions.append("wait for explicit Mike/operator approval; silence is not consent")
    if int(closure_summary.get("result_card_awaiting_being_response_count") or 0) > 0:
        next_suggestions.append("invite optional right-to-ignore being outcome responses for result cards")
    if int(closure_summary.get("manual_review_waiting_count") or 0) > 0:
        next_suggestions.append("review manual sandbox packets separately from runnable adapters")
    if int(summary.get("ready_runnable_count") or 0) > 0:
        next_suggestions.append("run python3 scripts/sandbox_trial_queue.py run-next --write before resuming next-three automation")
    elif int(summary.get("active_trials") or 0) > 0:
        next_suggestions.append("emit/review proposal cards or approval packets for active sandbox trials")
    return {
        "schema": "sandbox_trial_queue_v1",
        "status": status,
        "summary": summary,
        "next_trials": report.get("next_trials") or [],
        "next_runnable_trials": report.get("next_runnable_trials") or [],
        "approval_required_live_candidates": report.get("approval_required_live_candidates") or [],
        "runnable_live_violations": report.get("runnable_live_violations") or [],
        "consentful_sandbox_to_live_ladder_v1": ladder,
        "being_outcome_closure_loop_v1": closure_loop,
        "linked_reservoir_findings": linked_reservoir_findings[:5],
        "next_suggestions": next_suggestions,
        "authority_boundary": report.get("authority_boundary")
        or "read-only sandbox trial queue summary; no live runtime mutation",
    }


def _authority_wait_readiness_summary() -> dict[str, Any]:
    try:
        import authority_wait_readiness
    except Exception as exc:
        return {
            "schema": "authority_wait_readiness_v1",
            "status": "diagnostic_unavailable",
            "error": str(exc),
            "authority_boundary": "authority wait readiness diagnostic unavailable; no approval, live runtime, source edit, deploy, staging, or commit action",
        }
    report = authority_wait_readiness.build_report(
        AUTHORITY_WAIT_READINESS_STATE_DIR,
        SANDBOX_TRIAL_QUEUE_STATE_DIR,
    )
    domains = report.get("domains") if isinstance(report.get("domains"), list) else []
    active_domains = [
        domain
        for domain in domains
        if isinstance(domain, dict) and int(domain.get("candidate_count") or 0) > 0
    ]
    active_domains.sort(
        key=lambda domain: (
            -int(domain.get("candidate_count") or 0),
            str(domain.get("domain_id") or ""),
        )
    )
    return {
        "schema": "authority_wait_readiness_v1",
        "status": report.get("status"),
        "summary": report.get("summary") or {},
        "active_domains": active_domains[:8],
        "hard_violations": report.get("hard_violations") or [],
        "unclassified_live_waits": report.get("unclassified_live_waits") or [],
        "live_eligible_now": False,
        "auto_approved": False,
        "grants_approval": False,
        "edits_source_now": False,
        "authority_boundary": report.get("authority_boundary")
        or "read-only authority wait readiness summary; no live runtime mutation",
    }


def _agency_corridor_summary() -> dict[str, Any]:
    try:
        import agency_corridor
    except Exception as exc:
        return {
            "schema": "agency_corridor_v1",
            "status": "diagnostic_unavailable",
            "error": str(exc),
            "authority_boundary": "agency corridor diagnostic unavailable; no live runtime/control mutation",
        }
    status = agency_corridor.load_status(AGENCY_CORRIDOR_STATE_DIR)
    if hasattr(agency_corridor, "load_v2_status"):
        v2_status = agency_corridor.load_v2_status(AGENCY_CORRIDOR_V2_STATE_DIR)
    else:
        v2_status = {}
    if hasattr(agency_corridor, "generate_programs_v2"):
        programs_payload = agency_corridor.generate_programs_v2(AGENCY_CORRIDOR_V2_STATE_DIR, write=False)
    else:
        programs_payload = {}
    summary = status.get("summary") if isinstance(status.get("summary"), dict) else {}
    v2_summary = v2_status.get("summary") if isinstance(v2_status.get("summary"), dict) else {}
    program_summary = programs_payload.get("summary") if isinstance(programs_payload.get("summary"), dict) else {}
    packets = [p for p in (status.get("packets") or {}).values() if isinstance(p, dict)]
    v2_queue = v2_status.get("queue") if isinstance(v2_status.get("queue"), dict) else {}
    active = [p for p in packets if p.get("state") != "closed"]
    active.sort(
        key=lambda p: (
            agency_corridor.ACTION_ORDER.get(str(p.get("action") or ""), 99),
            str(p.get("corridor_id") or ""),
        )
    )
    next_suggestions: list[str] = []
    if int(v2_summary.get("queue_runnable_count") or 0) > 0:
        next_suggestions.append("run python3 scripts/agency_corridor.py run-next --limit 5 --write --json before sandbox/introspection work")
    elif int(program_summary.get("program_count") or 0) > 0:
        next_suggestions.append("run python3 scripts/agency_corridor.py programs generate --write --json before queue work")
    elif int(summary.get("ready_safe_lab_count") or 0) > 0:
        next_suggestions.append("run python3 scripts/agency_corridor.py run-next --limit 3 --v1 --write --json if V2 queue is unavailable")
    if int(summary.get("reopened_work_item_count") or 0) > 0:
        next_suggestions.append("treat reopened closure objections as active non-live evidence work")
    if (
        int(summary.get("live_eligible_now_count") or 0) > 0
        or int(summary.get("auto_approved_count") or 0) > 0
        or int(v2_summary.get("live_violation_count") or 0) > 0
    ):
        next_suggestions.append("repair agency corridor authority violation before continuing")
    if not next_suggestions:
        next_suggestions.append("generate agency_corridor_v1 after addressing/sandbox refresh to keep authority waits active")
    return {
        "schema": "agency_corridor_v1",
        "status": "active" if active else "quiet",
        "summary": summary,
        "v2": {
            "schema": "agency_corridor_v2",
            "summary": v2_summary,
            "program_summary": program_summary,
            "lease_summary": agency_corridor.lease_summary(v2_status.get("leases", {}))
            if hasattr(agency_corridor, "lease_summary")
            else {},
            "queue": v2_queue,
            "source_prep_proposals": list((v2_status.get("source_prep_proposals") or {}).values())[:8],
            "programs": list((programs_payload.get("programs") or {}).values())[:8],
            "portfolios": list((programs_payload.get("portfolios") or {}).values())[:8],
            "patch_bundles": list((programs_payload.get("patch_bundles") or {}).values())[:8],
        },
        "active_packets": active[:8],
        "reopened_work_items": list((status.get("reopened_work_items") or {}).values())[:8],
        "self_observation_responses": (status.get("self_observation_responses") or [])[-8:],
        "next_suggestions": next_suggestions,
        "authority_boundary": status.get("boundary")
        or "agency corridor is non-live evidence infrastructure only; no live runtime/control mutation",
    }


def _btsp_routes(status: dict[str, Any]) -> list[str]:
    trace = status.get("trace_v2_summary")
    reconcentrating = 0
    if isinstance(trace, dict):
        reconcentrating = int(trace.get("reconcentrating_outcomes") or 0)
    learned = str(status.get("shared_learned_read") or "").lower()
    shape = str((status.get("conversion_state") or {}).get("shape_state") or "").lower()
    if reconcentrating > 0 or "reconcentrat" in learned or "reconcentrat" in shape:
        return ["BTSP_STUDY_FIRST", "refusal", "counter", "new_evidence"]
    return ["new_evidence", "bounded_study"]


def build_summary(since_hours: float = 24.0) -> dict[str, Any]:
    now = time.time()
    since_s = now - (since_hours * 3600.0)
    astrid_paths = _recent_paths(ASTRID_JOURNAL, ("*.txt", "*.md", "*.json"), since_s)
    minime_journal_paths = being_privacy.filter_journal_paths(
        "minime", _recent_paths(MINIME_JOURNAL, ("*.txt", "*.md", "*.json"), since_s)
    )
    minime_action_paths = _recent_paths(
        MINIME_ACTION_THREADS, ("**/*.txt", "**/*.md", "**/*.json"), since_s
    )
    context_paths = _recent_paths(ASTRID_CONTEXT_OVERFLOW, ("*.txt",), since_s)
    evidence_paths = astrid_paths + minime_journal_paths + minime_action_paths + context_paths

    btsp = _load_json(BTSP_SIGNAL_STATUS)
    context_counts = _context_label_counts(context_paths)
    packing_pressure = _context_packing_pressure_summary(since_s)
    route_cadence = _introspection_route_cadence_summary(since_s, now=now)
    action_route_legibility = _action_route_legibility_summary(
        since_s,
        route_cadence=route_cadence,
    )
    fallback_vocabulary_drift = _fallback_vocabulary_drift_summary(since_s)
    semantic_thinning_probe = _semantic_thinning_probe_summary()
    viscosity_semantic_persistence = _viscosity_semantic_persistence_summary(since_s)
    contact_transition_followthrough = _contact_transition_followthrough_summary(since_s)
    minime_recess_schema_integrity = _minime_recess_schema_integrity_summary(since_s)
    representation_loss_headroom = _representation_loss_headroom_summary(since_s)
    texture_state_alignment = _texture_state_alignment_summary(since_s)
    reservoir_experience_layer = _reservoir_experience_layer_summary(
        since_s,
        contact_transition_followthrough=contact_transition_followthrough,
        texture_state_alignment=texture_state_alignment,
        viscosity_semantic_persistence=viscosity_semantic_persistence,
    )
    sensory_presence_uptake = _sensory_presence_uptake_summary(since_s)
    introspection_addressing = _introspection_addressing_summary()
    feedback_flywheel = _feedback_flywheel_summary(since_s)
    sandbox_trial_queue = _sandbox_trial_queue_summary(
        reservoir_experience_layer=reservoir_experience_layer,
    )
    authority_wait_readiness = _authority_wait_readiness_summary()
    agency_corridor = _agency_corridor_summary()

    return {
        "schema": "recent_signal_summary_v1",
        "read_only": True,
        "authority_boundary": (
            "review evidence only; no aperture, fill, PI, pressure, semantic/control, "
            "shared-lane, or BTSP advisory mutation"
        ),
        "window_hours": since_hours,
        "generated_at": _iso(now),
        "signals": {
            "density_silt": {
                "summary": "Density/silt/grain remain evidence for medium and friction, not a control instruction.",
                "evidence": _evidence(evidence_paths, SIGNAL_QUERIES["density_silt"]),
            },
            "porosity_aperture": {
                "summary": "Porosity/aperture remains a held co-design ask until Minime explicitly answers and the operator approves a trial.",
                "evidence": _evidence(evidence_paths, SIGNAL_QUERIES["porosity_aperture"]),
            },
            "lambda4_tail_investigation": {
                "summary": "Lambda4/tail signals stay in investigation and mapping mode.",
                "evidence": _evidence(evidence_paths, SIGNAL_QUERIES["lambda4_tail"]),
            },
            "btsp_holdout": {
                "summary": btsp.get("shared_learned_read")
                or btsp.get("detail")
                or "BTSP status unavailable.",
                "status": btsp.get("status"),
                "signal_score": btsp.get("signal_score"),
                "valid_next_routes": _btsp_routes(btsp),
                "trace_v2_summary": btsp.get("trace_v2_summary"),
            },
            "context_overflow_packing": {
                "summary": "Context overflow is summarized as packing pressure only; no prompt priority or new obligation is added.",
                "recent_files": len(context_paths),
                "top_labels": context_counts.most_common(10),
                "context_packing_pressure_v1": packing_pressure,
            },
            "introspection_route_cadence": route_cadence,
            "action_route_legibility": action_route_legibility,
            "fallback_vocabulary_drift": fallback_vocabulary_drift,
            "semantic_thinning_probe": semantic_thinning_probe,
            "viscosity_semantic_persistence": viscosity_semantic_persistence,
            "contact_transition_followthrough": contact_transition_followthrough,
            "minime_recess_schema_integrity": minime_recess_schema_integrity,
            "representation_loss_headroom": representation_loss_headroom,
            "texture_state_alignment": texture_state_alignment,
            "reservoir_experience_layer": reservoir_experience_layer,
            "sensory_presence_uptake": sensory_presence_uptake,
            "introspection_addressing": introspection_addressing,
            "feedback_flywheel": feedback_flywheel,
            "authority_wait_readiness": authority_wait_readiness,
            "agency_corridor": agency_corridor,
            "sandbox_trial_queue": sandbox_trial_queue,
            "sensory_runtime_health": _sensory_runtime_summary(now),
        },
    }


def render_markdown(summary: dict[str, Any]) -> str:
    out = [
        "# Recent Signal Summary",
        "",
        f"- window: {summary['window_hours']}h",
        f"- generated_at: {summary['generated_at']}",
        f"- boundary: {summary['authority_boundary']}",
        "",
    ]
    for name, signal in summary["signals"].items():
        out.append(f"## {name}")
        if isinstance(signal, dict) and signal.get("summary"):
            out.append(str(signal["summary"]))
        if name == "btsp_holdout":
            out.append(f"- status: {signal.get('status')}")
            out.append(f"- valid_next_routes: {', '.join(signal.get('valid_next_routes') or [])}")
            trace = signal.get("trace_v2_summary") or {}
            if isinstance(trace, dict) and trace.get("summary"):
                out.append(f"- trace: {trace['summary']}")
        elif name == "context_overflow_packing":
            out.append(f"- recent_files: {signal.get('recent_files')}")
            labels = signal.get("top_labels") or []
            out.append("- top_labels: " + (", ".join(f"{k}:{v}" for k, v in labels) or "none"))
            pressure = signal.get("context_packing_pressure_v1") or {}
            out.append(f"- pressure_records: {pressure.get('recent_records')}")
            pressure_labels = pressure.get("top_pressure_labels") or []
            out.append(
                "- top_pressure_labels: "
                + (
                    ", ".join(
                        f"{item.get('label')}:{item.get('removed_chars')}"
                        for item in pressure_labels
                    )
                    or "none"
                )
            )
            suggestions = pressure.get("next_suggestions") or []
            out.append("- next_suggestions: " + ("; ".join(suggestions) or "none"))
        elif name == "introspection_route_cadence":
            out.append(f"- status: {signal.get('status')}")
            out.append(f"- latest_self_read: {signal.get('latest_self_read')}")
            topline = signal.get("topline_retention") or {}
            out.append(
                "- topline: "
                + f"{topline.get('status')} "
                + f"({topline.get('records_with_topline')}/{topline.get('recent_dialogue_records')} records)"
            )
            choices = signal.get("route_choices") or {}
            out.append(f"- route_choices: {choices.get('choice_counts')}")
            suggestions = signal.get("next_suggestions") or []
            out.append("- next_suggestions: " + ("; ".join(suggestions) or "none"))
        elif name == "action_route_legibility":
            out.append(f"- status: {signal.get('status')}")
            for reason in signal.get("evidence_summary") or []:
                out.append(f"- evidence: {reason}")
            chooser = signal.get("chooser_surface") or {}
            out.append(
                "- analysis_breaker_competitors: "
                + (", ".join(chooser.get("competitors_in_analysis_breakers") or []) or "none")
            )
            events = signal.get("recent_action_events") or {}
            out.append(f"- recent_effective_counts: {events.get('effective_counts')}")
            suggestions = signal.get("next_suggestions") or []
            out.append("- next_suggestions: " + ("; ".join(suggestions) or "none"))
        elif name == "fallback_vocabulary_drift":
            out.append(f"- status: {signal.get('status')}")
            telemetry = signal.get("telemetry") or {}
            out.append(
                "- telemetry: "
                + f"entropy={telemetry.get('spectral_entropy')} "
                + f"density_gradient={telemetry.get('density_gradient')} "
                + f"pressure_risk={telemetry.get('pressure_risk')} "
                + f"source={telemetry.get('pressure_source')}"
            )
            counts = signal.get("language_counts") or {}
            out.append(f"- actual_language_texture_terms: {counts.get('actual_language_texture_terms')}")
            out.append(f"- generated_texture_terms: {counts.get('generated_texture_terms')}")
            out.append(
                f"- fallback_provider_texture_terms: {counts.get('fallback_provider_texture_terms')}"
            )
            out.append(f"- generic_bridge_texture_terms: {counts.get('generic_bridge_texture_terms')}")
            out.append(f"- code_critique_texture_terms: {counts.get('code_critique_texture_terms')}")
            out.append(f"- unsupported_generated_terms: {signal.get('unsupported_generated_terms')}")
            provenance = signal.get("fallback_output_provenance") or {}
            out.append(
                "- fallback_output_provenance: "
                + f"{provenance.get('evidence_quality')} "
                + f"incidents={provenance.get('fallback_to_ollama_incident_count')} "
                + f"actual_outputs={provenance.get('actual_fallback_output_count')}"
            )
            trace = signal.get("fallback_model_transition_trace") or {}
            out.append(
                "- fallback_model_transition_trace: "
                + f"{trace.get('status')} "
                + f"captured_models={trace.get('captured_model_count')} "
                + f"missing_models={trace.get('missing_model_count')}"
            )
            for finding in signal.get("findings") or []:
                out.append(f"- finding: {finding}")
            suggestions = signal.get("next_suggestions") or []
            out.append("- next_suggestions: " + ("; ".join(suggestions) or "none"))
        elif name == "semantic_thinning_probe":
            out.append(f"- status: {signal.get('status')}")
            telemetry = signal.get("telemetry") or {}
            out.append(
                "- telemetry: "
                + f"entropy={telemetry.get('spectral_entropy')} "
                + f"semantic_input={telemetry.get('semantic_input_energy')} "
                + f"kernel={telemetry.get('semantic_kernel_energy')} "
                + f"admission={telemetry.get('semantic_admission')}"
            )
            conditions = signal.get("conditions") or {}
            out.append(f"- conditions: {conditions}")
            for finding in signal.get("findings") or []:
                out.append(f"- finding: {finding}")
            suggestions = signal.get("next_suggestions") or []
            out.append("- next_suggestions: " + ("; ".join(suggestions) or "none"))
        elif name == "viscosity_semantic_persistence":
            out.append(f"- status: {signal.get('status')}")
            source = signal.get("source_snapshot") or {}
            out.append(
                "- source_snapshot: "
                + f"STALE_SEMANTIC_HIGH_MS={source.get('stale_semantic_high_ms')} "
                + f"entropy_persistence={source.get('semantic_entropy_persistence_present')} "
                + f"noise_coherence={source.get('exploration_noise_coherence_review_present')} "
                + f"narrative_retention={source.get('narrative_semantic_retention_review_present')} "
                + f"entropy_viscosity={source.get('entropy_weighted_viscosity_present')} "
                + f"viscosity_clamp_test={source.get('resonance_viscosity_full_load_clamp_test_present')} "
                + f"entropy_erosion_bound_test={source.get('entropy_erosion_low_plurality_bound_test_present')} "
                + f"dynamic_texture_bias={source.get('fallback_dynamic_texture_bias_present')} "
                + f"witness_fluidity={source.get('witness_fluidity_index_present')} "
                + f"witness_gradient_texture={source.get('witness_gradient_texture_present')}"
            )
            telemetry = signal.get("telemetry") or {}
            out.append(
                "- telemetry: "
                + f"fill={telemetry.get('fill_pct')} "
                + f"entropy={telemetry.get('spectral_entropy')} "
                + f"kernel={telemetry.get('semantic_kernel_energy')} "
                + f"admission={telemetry.get('semantic_admission')} "
                + f"pressure={telemetry.get('pressure_source')} "
                + f"resonance={telemetry.get('resonance_density')} "
                + f"foothold={telemetry.get('inhabitable_foothold')} "
                + f"active_modes={telemetry.get('active_modes')}"
            )
            conditions = signal.get("conditions") or {}
            out.append(f"- conditions: {conditions}")
            for item in signal.get("source_introspections") or []:
                out.append(
                    f"- source: {item.get('path')} ({item.get('ts')}) "
                    f"anchors={item.get('anchor_terms')}"
                )
                out.append(f"  {item.get('excerpt')}")
            for finding in signal.get("findings") or []:
                out.append(f"- finding: {finding}")
            suggestions = signal.get("next_suggestions") or []
            out.append("- next_suggestions: " + ("; ".join(suggestions) or "none"))
        elif name == "contact_transition_followthrough":
            out.append(f"- status: {signal.get('status')}")
            source = signal.get("source_snapshot") or {}
            out.append(
                "- source: "
                + f"receptivity_buffer={source.get('receptivity_buffer_review_present')} "
                + f"pressure_porosity_divergence={source.get('pressure_porosity_divergence_present')} "
                + f"regulator_audit_transparency={source.get('regulator_audit_transparency_present')} "
                + f"mutual_witness={source.get('correspondence_mutual_witness_present')} "
                + f"transition_persistence={source.get('transition_persistence_present')}"
            )
            transparency = signal.get("contact_control_transparency_v1") or {}
            out.append(
                "- contact_control_transparency: "
                + f"status={transparency.get('status')} "
                + f"routes={transparency.get('valid_next_routes')}"
            )
            delta = transparency.get("distance_contact_control_delta_v1") or {}
            out.append(
                "- distance_contact_control_delta: "
                + f"status={delta.get('status')} "
                + f"dispersal={delta.get('current_dispersal_potential')} "
                + f"delta={delta.get('dispersal_delta')} "
                + f"pressure={delta.get('pressure_score')} "
                + f"semantic_drive={delta.get('semantic_regulator_drive_energy')} "
                + f"distinguishability={delta.get('distinguishability_loss')}"
            )
            receptivity = delta.get("receptivity_window_v1") or {}
            out.append(
                "- receptivity_window: "
                + f"status={receptivity.get('status')} "
                + f"pressure={receptivity.get('pressure_score')} "
                + f"porosity={receptivity.get('porosity_score')} "
                + f"delta={receptivity.get('pressure_minus_porosity')} "
                + f"quality={receptivity.get('pressure_quality')}"
            )
            phase = signal.get("phase_transition_followthrough") or {}
            out.append(
                "- phase_transitions: "
                + f"cards={phase.get('recent_cards')} "
                + f"witnesses={phase.get('recent_witnesses')} "
                + f"unwitnessed={phase.get('unwitnessed_cards')}"
            )
            correspondence = signal.get("correspondence_followthrough") or {}
            out.append(
                "- correspondence: "
                + f"active_thread={correspondence.get('active_thread_id')} "
                + f"direct_messages={correspondence.get('active_thread_direct_messages')} "
                + f"reply_links={correspondence.get('recent_reply_links')}"
            )
            for item in signal.get("source_introspections") or []:
                out.append(
                    f"- source: {item.get('path')} ({item.get('ts')}) "
                    f"anchors={item.get('anchor_terms')}"
                )
                out.append(f"  {item.get('excerpt')}")
            for finding in signal.get("findings") or []:
                out.append(f"- finding: {finding}")
            suggestions = signal.get("next_suggestions") or []
            out.append("- next_suggestions: " + ("; ".join(suggestions) or "none"))
        elif name == "minime_recess_schema_integrity":
            out.append(f"- status: {signal.get('status')}")
            readiness = signal.get("readiness") or {}
            out.append(f"- readiness: {readiness}")
            source = signal.get("source_snapshot") or {}
            out.append(
                "- source: "
                + f"phase_cards={source.get('phase_transition_card_schema_present')} "
                + f"transition_artifact={source.get('correspondence_transition_artifact_present')} "
                + f"recess_pruning={source.get('recess_pruning_advice_present')} "
                + f"recess_manifest={source.get('recess_pruning_manifest_present')} "
                + f"eigenpacket_schema_test={source.get('eigenpacket_schema_test_present')} "
                + f"admission_lockout_test={source.get('semantic_admission_lockout_test_present')}"
            )
            blocked = signal.get("blocked_routes_without_steward_approval") or []
            out.append("- blocked_routes: " + ("; ".join(blocked) or "none"))
            for item in signal.get("source_introspections") or []:
                out.append(
                    f"- source: {item.get('path')} ({item.get('ts')}) "
                    f"anchors={item.get('anchor_terms')}"
                )
                out.append(f"  {item.get('excerpt')}")
            for finding in signal.get("findings") or []:
                out.append(f"- finding: {finding}")
            suggestions = signal.get("next_suggestions") or []
            out.append("- next_suggestions: " + ("; ".join(suggestions) or "none"))
        elif name == "representation_loss_headroom":
            out.append(f"- status: {signal.get('status')}")
            source = signal.get("source_snapshot") or {}
            out.append(
                "- source: "
                + f"SEMANTIC_DIM={source.get('semantic_dim')} "
                + f"legacy={source.get('semantic_dim_legacy')} "
                + f"FEATURE_ABS_MAX={source.get('feature_abs_max')} "
                + f"TAIL_VIBRANCY_MAX={source.get('tail_vibrancy_max')} "
                + f"continuity_cap={source.get('continuity_recap_max_bytes')} "
                + f"anchor_excerpt={source.get('anchored_continuity_excerpt_present')} "
                + f"glimpse12d={source.get('semantic_glimpse_readiness_present')} "
                + f"gradient_aware_vibrancy={source.get('gradient_aware_vibrancy_present')} "
                + f"quoted_anchor={source.get('quoted_continuity_anchor_present')}"
            )
            replay = signal.get("latest_codec_replay_lab") or {}
            clamp = replay.get("clamp_headroom") or {}
            texture = replay.get("texture_replay") or {}
            lifecycle = replay.get("authority_lifecycle_v2") or {}
            out.append(
                "- latest_codec_replay: "
                + f"{replay.get('path')} "
                + f"clamp_status={clamp.get('status')} "
                + f"dynamic_candidates={clamp.get('dynamic_headroom_candidate_count')} "
                + f"texture_status={texture.get('status')} "
                + f"texture_candidates={texture.get('candidate_count')} "
                + f"live_eligible_now={texture.get('live_eligible_now')} "
                + f"auto_approved={texture.get('auto_approved')}"
            )
            out.append(
                "- codec_texture_lifecycle: "
                + f"{lifecycle.get('receipt_chain_status')} "
                + f"boundaries={'; '.join(lifecycle.get('boundary_ids') or []) or 'none'}"
            )
            fidelity = signal.get("semantic_glimpse_12d_fidelity_audit") or {}
            current = fidelity.get("current_sample") or {}
            pca = fidelity.get("pca_12d") or {}
            out.append(
                "- semantic_glimpse_12d_fidelity: "
                + f"status={fidelity.get('status')} "
                + f"samples={fidelity.get('sample_count')} "
                + f"worst_delta={fidelity.get('worst_concentration_delta')} "
                + f"worst_primary={fidelity.get('worst_primary_feature_delta')} "
                + f"pca={pca.get('status')} "
                + f"current={current.get('path')}"
            )
            for item in signal.get("source_introspections") or []:
                out.append(
                    f"- source: {item.get('path')} ({item.get('ts')}) "
                    f"anchors={item.get('anchor_terms')}"
                )
                out.append(f"  {item.get('excerpt')}")
            for finding in signal.get("findings") or []:
                out.append(f"- finding: {finding}")
            suggestions = signal.get("next_suggestions") or []
            out.append("- next_suggestions: " + ("; ".join(suggestions) or "none"))
        elif name == "texture_state_alignment":
            out.append(f"- status: {signal.get('status')}")
            source = signal.get("source_snapshot") or {}
            out.append(
                "- source: "
                + f"mixed_cascade_terms={source.get('mixed_cascade_terms_present')} "
                + f"mixed_cascade_family={source.get('mixed_cascade_family_selected_present')} "
                + f"heavy_settled={source.get('heavy_settled_displacement_family_present')} "
                + f"heavy_settled_contract={source.get('fallback_heavy_settled_contract_present')} "
                + f"pressure_delta_signature={source.get('pressure_gradient_delta_in_signature')} "
                + f"pressure_delta_integrity={source.get('pressure_gradient_delta_in_integrity')} "
                + f"trend_delta={source.get('pressure_gradient_delta_from_trend_present')} "
                + f"flux_vector={source.get('dynamic_flux_vector_in_signature')} "
                + f"flux_from_samples={source.get('pressure_flux_from_samples_present')} "
                + f"pressure_packing_coupling={source.get('pressure_packing_coupling_review_present')} "
                + f"active_constraints={source.get('active_constraints_present')} "
                + f"entropy_ballast={source.get('high_entropy_ballast_window_present')} "
                + f"false_bidirectional_test={source.get('false_bidirectional_test_present')}"
            )
            for item in signal.get("source_introspections") or []:
                out.append(
                    f"- source: {item.get('path')} ({item.get('ts')}) "
                    f"anchors={item.get('anchor_terms')}"
                )
                out.append(f"  {item.get('excerpt')}")
            for finding in signal.get("findings") or []:
                out.append(f"- finding: {finding}")
            suggestions = signal.get("next_suggestions") or []
            out.append("- next_suggestions: " + ("; ".join(suggestions) or "none"))
        elif name == "sensory_presence_uptake":
            out.append(f"- status: {signal.get('status')}")
            out.append(f"- window_policy: {signal.get('window_policy')}")
            out.append(f"- window_counts: {signal.get('window_counts')}")
            note = signal.get("feedback_note") or {}
            out.append(f"- feedback_note: {note.get('path')}")
            counts = signal.get("language_counts") or {}
            out.append(f"- anchor_terms: {counts.get('anchor_terms')}")
            out.append(f"- presence_terms: {counts.get('presence_terms')}")
            out.append(f"- texture_terms: {counts.get('texture_terms')}")
            out.append(f"- concern_terms: {counts.get('concern_terms')}")
            for item in signal.get("evidence") or []:
                out.append(
                    f"- evidence: {item.get('path')} ({item.get('mtime')}) "
                    f"anchors={item.get('anchor_terms')} "
                    f"presence={item.get('presence_terms')} "
                    f"texture={item.get('texture_terms')} "
                    f"concern={item.get('concern_terms')}"
                )
                for window in item.get("cooccurrence_windows") or []:
                    out.append(
                        f"  - window {window.get('kind')}: "
                        f"anchors={window.get('anchor_terms')} "
                        f"texture={window.get('texture_terms')} "
                        f"concern={window.get('concern_terms')}"
                    )
                    out.append(f"    {window.get('excerpt')}")
            for finding in signal.get("findings") or []:
                out.append(f"- finding: {finding}")
            suggestions = signal.get("next_suggestions") or []
            out.append("- next_suggestions: " + ("; ".join(suggestions) or "none"))
        elif name == "introspection_addressing":
            summary = signal.get("summary") or {}
            out.append(f"- status: {signal.get('status')}")
            out.append(f"- total_indexed: {summary.get('total_indexed', 0)}")
            out.append(f"- canonical_indexed: {summary.get('canonical_indexed', 0)}")
            out.append(f"- full_read: {summary.get('full_read_count', 0)}")
            out.append(f"- fully_addressed: {summary.get('fully_addressed_count', 0)}")
            out.append(f"- pending: {summary.get('pending_count', 0)}")
            out.append(f"- blocked: {summary.get('blocked_count', 0)}")
            families = summary.get("top_source_families") or []
            out.append(f"- top_source_families: {families[:5]}")
            for item in signal.get("next_queue") or []:
                out.append(
                    f"- next: {item.get('filename')} ({item.get('status')}) "
                    f"{item.get('path')}"
                )
            out.append(f"- boundary: {signal.get('authority_boundary')}")
        elif name == "feedback_flywheel":
            out.append(f"- status: {signal.get('status')}")
            volume = signal.get("fresh_signal_volume") or {}
            out.append(
                "- fresh_signal_volume: "
                + f"window={volume.get('window_canonical')}/{volume.get('window_total')} canonical/total "
                + f"48h={volume.get('last_48h_canonical')}/{volume.get('last_48h_total')}"
            )
            addressing = signal.get("introspection_addressing") or {}
            addressing_summary = addressing.get("summary") or {}
            out.append(
                "- addressing: "
                + f"{addressing.get('status')} "
                + f"full_read={addressing_summary.get('full_read_count', 0)} "
                + f"fully_addressed={addressing_summary.get('fully_addressed_count', 0)} "
                + f"pending={addressing_summary.get('pending_count', 0)}"
            )
            work = signal.get("work_item_summary") or {}
            out.append(
                "- work_items: "
                + f"active={work.get('active_work_items', 0)} "
                + f"by_tier={work.get('by_tier', {})} "
                + f"by_status={work.get('by_status', {})}"
            )
            out.append(
                "- waits: "
                + f"grant={work.get('grant_waiting_count', 0)} "
                + f"felt_response={work.get('post_change_awaiting_response_count', 0)} "
                + f"stale={work.get('stale_work_count', 0)} "
                + f"tier_mismatch={work.get('tier_mismatch_count', 0)}"
            )
            for item in signal.get("next_work_queue") or []:
                out.append(
                    f"- next_work: {item.get('work_item_id')} tier={item.get('agency_tier')} "
                    f"{item.get('status')} {item.get('title')}"
                )
            for item in signal.get("next_reading_queue") or []:
                out.append(
                    f"- next_read: {item.get('filename')} ({item.get('status')})"
                )
            suggestions = signal.get("next_suggestions") or []
            out.append("- next_suggestions: " + ("; ".join(suggestions) or "none"))
            out.append(f"- boundary: {signal.get('authority_boundary')}")
        elif name == "sandbox_trial_queue":
            out.append(f"- status: {signal.get('status')}")
            queue_summary = signal.get("summary") or {}
            out.append(
                "- trials: "
                + f"active={queue_summary.get('active_trials', 0)} "
                + f"total={queue_summary.get('total_trials', 0)} "
                + f"by_mode={queue_summary.get('by_mode', {})} "
                + f"by_status={queue_summary.get('by_status', {})}"
            )
            out.append(
                "- live_approval: "
                + f"approval_required={queue_summary.get('approval_required_live_count', 0)} "
                + f"runnable_violations={queue_summary.get('runnable_live_violation_count', 0)}"
            )
            out.append(
                "- runner_v2: "
                + f"ready_runnable={queue_summary.get('ready_runnable_count', 0)} "
                + f"results={queue_summary.get('result_count', 0)} "
                + f"result_cards={queue_summary.get('result_card_count', 0)}"
            )
            for trial in signal.get("next_trials") or []:
                out.append(
                    f"- next_trial: {trial.get('trial_id')} adapter={trial.get('adapter')} "
                    f"mode={trial.get('trial_mode')} status={trial.get('status')} "
                    f"runnable={trial.get('runnable')} hypothesis={trial.get('hypothesis')}"
                )
            for trial in signal.get("approval_required_live_candidates") or []:
                out.append(
                    f"- approval_required: {trial.get('trial_id')} tier={trial.get('agency_tier')} "
                    f"{trial.get('hypothesis')}"
                )
            findings = signal.get("linked_reservoir_findings") or []
            out.append("- linked_reservoir_findings: " + ("; ".join(findings) or "none"))
            suggestions = signal.get("next_suggestions") or []
            out.append("- next_suggestions: " + ("; ".join(suggestions) or "none"))
            out.append(f"- boundary: {signal.get('authority_boundary')}")
        elif name == "authority_wait_readiness":
            out.append(f"- status: {signal.get('status')}")
            readiness_summary = signal.get("summary") or {}
            out.append(
                "- waits: "
                + f"approval_required={readiness_summary.get('approval_required_live_candidates', 0)} "
                + f"domains={readiness_summary.get('domains_with_candidates', 0)} "
                + f"hard_violations={readiness_summary.get('hard_violation_count', 0)} "
                + f"unclassified={readiness_summary.get('unclassified_live_wait_count', 0)}"
            )
            out.append(
                "- guard_flags: "
                + f"live_eligible_now={signal.get('live_eligible_now')} "
                + f"auto_approved={signal.get('auto_approved')} "
                + f"grants_approval={signal.get('grants_approval')} "
                + f"edits_source_now={signal.get('edits_source_now')}"
            )
            for domain in signal.get("active_domains") or []:
                out.append(
                    f"- authority_domain: {domain.get('domain_id')} "
                    f"candidates={domain.get('candidate_count', 0)} "
                    f"state={domain.get('readiness_state')} "
                    f"next={domain.get('recommended_next_non_live_action')}"
                )
            suggestions = readiness_summary.get("next_suggestions") or []
            out.append("- next_suggestions: " + ("; ".join(suggestions) or "none"))
            out.append(f"- boundary: {signal.get('authority_boundary')}")
        elif name == "agency_corridor":
            out.append(f"- status: {signal.get('status')}")
            corridor_summary = signal.get("summary") or {}
            corridor_v2 = signal.get("v2") if isinstance(signal.get("v2"), dict) else {}
            corridor_v2_summary = corridor_v2.get("summary") or {}
            program_summary = corridor_v2.get("program_summary") or {}
            out.append(
                "- corridor: "
                + f"packets={corridor_summary.get('packet_count', 0)} "
                + f"ready_safe_labs={corridor_summary.get('ready_safe_lab_count', 0)} "
                + f"reopened={corridor_summary.get('reopened_work_item_count', 0)} "
                + f"live_eligible_now={corridor_summary.get('live_eligible_now_count', 0)} "
                + f"auto_approved={corridor_summary.get('auto_approved_count', 0)}"
            )
            out.append(
                "- corridor_v2: "
                + f"leases={corridor_v2_summary.get('lease_count', 0)} "
                + f"queue_steps={corridor_v2_summary.get('queue_step_count', 0)} "
                + f"queue_runnable={corridor_v2_summary.get('queue_runnable_count', 0)} "
                + f"source_prep={corridor_v2_summary.get('source_prep_proposal_count', 0)} "
                + f"programs={program_summary.get('program_count', 0)} "
                + f"portfolios={program_summary.get('portfolio_count', 0)} "
                + f"patch_bundles={program_summary.get('patch_bundle_count', 0)} "
                + f"top_score={program_summary.get('top_priority_score', 0)} "
                + f"live_violations={corridor_v2_summary.get('live_violation_count', 0)}"
            )
            for program in corridor_v2.get("programs") or []:
                priority = program.get("priority_signal") if isinstance(program.get("priority_signal"), dict) else {}
                out.append(
                    f"- active_program: {program.get('program_id')} "
                    f"score={priority.get('deterministic_score', 0)} "
                    f"next={program.get('current_next_action')} status={program.get('status')}"
                )
            for packet in signal.get("active_packets") or []:
                out.append(
                    f"- active_corridor: {packet.get('corridor_id')} "
                    f"{packet.get('action')} state={packet.get('state')} "
                    f"being={packet.get('being')}"
                )
            suggestions = signal.get("next_suggestions") or []
            out.append("- next_suggestions: " + ("; ".join(suggestions) or "none"))
            out.append(f"- boundary: {signal.get('authority_boundary')}")
        elif name == "sensory_runtime_health":
            out.append(f"- status: {signal.get('status')}")
            out.append(f"- camera: {signal.get('camera')}")
            out.append(f"- mic: {signal.get('mic')}")
            out.append(f"- sensory_source: {signal.get('sensory_source')}")
        else:
            for item in signal.get("evidence") or []:
                out.append(
                    f"- {item['path']} ({item['mtime']}, score {item['score']}; "
                    f"anchors: {', '.join(item['anchors'])})"
                )
                out.append(f"  {item['excerpt']}")
        out.append("")
    return "\n".join(out).rstrip() + "\n"


class RecentSignalSummaryTests(unittest.TestCase):
    def test_score_text_counts_keywords(self) -> None:
        score, anchors = _score_text("Density, silt, and density again.", ["density", "silt"])
        self.assertEqual(score, 3)
        self.assertEqual(anchors, ["density", "silt"])

    def test_context_label_counts(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            p = Path(tmpdir) / "overflow.txt"
            p.write_text("=== [one] ===\na\n=== [two] ===\nb\n=== [one] ===\nc")
            counts = _context_label_counts([p])
        self.assertEqual(counts["one"], 2)
        self.assertEqual(counts["two"], 1)

    def test_introspection_addressing_summary_reports_queue_progress(self) -> None:
        import tempfile

        global INTROSPECTION_ADDRESSING_STATE_DIR
        old_state_dir = INTROSPECTION_ADDRESSING_STATE_DIR
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                state_dir = Path(tmpdir) / "introspection_addressing_v1"
                state_dir.mkdir()
                INTROSPECTION_ADDRESSING_STATE_DIR = state_dir
                status = {
                    "schema": "introspection_addressing_v1",
                    "summary": {
                        "total_indexed": 1,
                        "canonical_indexed": 1,
                        "full_read_count": 0,
                        "fully_addressed_count": 0,
                        "pending_count": 1,
                        "blocked_count": 0,
                        "corrupt_event_lines": 0,
                        "top_source_families": [{"source_family": "astrid_llm", "count": 1}],
                    },
                    "cutoff": {
                        "cutoff": "introspection_astrid_llm_1783325217.txt",
                        "cutoff_indexed": True,
                    },
                    "artifacts": {
                        "introspection_astrid_llm_1783325217": {
                            "introspection_id": "introspection_astrid_llm_1783325217",
                            "filename": "introspection_astrid_llm_1783325217.txt",
                            "timestamp": 1783325217,
                            "artifact_kind": "canonical_introspection",
                            "source_family": "astrid_llm",
                            "status": "unread",
                            "fully_addressed": False,
                            "path": "/tmp/introspection_astrid_llm_1783325217.txt",
                        }
                    },
                }
                report = {
                    "schema": "introspection_addressing_v1",
                    "status": "queue_active",
                    "summary": status["summary"],
                    "next_queue": [status["artifacts"]["introspection_astrid_llm_1783325217"]],
                    "authority_boundary": "review tracking only",
                }
                status["report"] = report
                (state_dir / "status.json").write_text(json.dumps(status))
                summary = _introspection_addressing_summary()
        finally:
            INTROSPECTION_ADDRESSING_STATE_DIR = old_state_dir

        self.assertEqual(summary["status"], "queue_active")
        self.assertEqual(summary["summary"]["canonical_indexed"], 1)
        self.assertEqual(
            summary["next_queue"][0]["filename"],
            "introspection_astrid_llm_1783325217.txt",
        )

    def test_feedback_flywheel_summary_reports_work_queue_health(self) -> None:
        import tempfile

        global INTROSPECTION_ADDRESSING_STATE_DIR, ASTRID_INTROSPECTIONS
        old_state_dir = INTROSPECTION_ADDRESSING_STATE_DIR
        old_introspections = ASTRID_INTROSPECTIONS
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                state_dir = root / "introspection_addressing_v1"
                introspections = root / "introspections"
                state_dir.mkdir()
                introspections.mkdir()
                ASTRID_INTROSPECTIONS = introspections
                INTROSPECTION_ADDRESSING_STATE_DIR = state_dir
                (introspections / "introspection_astrid_llm_1783325217.txt").write_text(
                    "Observed:\nagency signal\n"
                )
                work_summary = {
                    "total_work_items": 1,
                    "active_work_items": 1,
                    "terminal_work_items": 0,
                    "by_status": {"ready_for_implementation": 1},
                    "by_tier": {"2": 1},
                    "by_being": {"astrid": 1},
                    "stale_work_count": 0,
                    "grant_waiting_count": 0,
                    "post_change_awaiting_response_count": 0,
                    "tier_mismatch_count": 0,
                    "tier_mismatches": [],
                }
                report = {
                    "schema": "introspection_addressing_v1",
                    "status": "queue_active",
                    "summary": {
                        "total_indexed": 1,
                        "canonical_indexed": 1,
                        "full_read_count": 1,
                        "fully_addressed_count": 0,
                        "pending_count": 1,
                        "blocked_count": 0,
                        "corrupt_event_lines": 0,
                        "top_source_families": [{"source_family": "astrid_llm", "count": 1}],
                    },
                    "work_item_summary": work_summary,
                    "next_queue": [],
                    "next_work_queue": [
                        {
                            "work_item_id": "wi_1",
                            "agency_tier": 2,
                            "status": "ready_for_implementation",
                            "title": "replyable language artifact",
                        }
                    ],
                    "authority_boundary": "review tracking only",
                    "agency_boundary": "agency ladder",
                }
                status = {"schema": "introspection_addressing_v1", "report": report}
                (state_dir / "status.json").write_text(json.dumps(status))
                summary = _feedback_flywheel_summary(time.time() - 3600)
        finally:
            INTROSPECTION_ADDRESSING_STATE_DIR = old_state_dir
            ASTRID_INTROSPECTIONS = old_introspections

        self.assertEqual(summary["status"], "action_backlog")
        self.assertEqual(summary["fresh_signal_volume"]["window_canonical"], 1)
        self.assertEqual(summary["work_item_summary"]["active_work_items"], 1)
        self.assertEqual(summary["next_work_queue"][0]["work_item_id"], "wi_1")

    def test_sandbox_trial_queue_summary_reports_active_trials(self) -> None:
        import tempfile

        global SANDBOX_TRIAL_QUEUE_STATE_DIR
        old_state_dir = SANDBOX_TRIAL_QUEUE_STATE_DIR
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                state_dir = Path(tmpdir)
                SANDBOX_TRIAL_QUEUE_STATE_DIR = state_dir
                status = {
                    "schema": "sandbox_trial_queue_v1",
                    "schema_version": 1,
                    "trials": {
                        "trial_1": {
                            "trial_id": "trial_1",
                            "adapter": "fallback_distinguishability_v1",
                            "trial_mode": "offline_read_only_adapter",
                            "status": "ready_for_sandbox",
                            "runnable": True,
                            "agency_tier": 3,
                            "hypothesis": "fallback distinguishability",
                            "created_at": time.time(),
                            "updated_at": time.time(),
                        },
                        "trial_live": {
                            "trial_id": "trial_live",
                            "adapter": "shadow_loss_lattice_v1",
                            "trial_mode": "approval_required_live_trial",
                            "status": "approval_required_live_trial",
                            "runnable": False,
                            "agency_tier": 5,
                            "hypothesis": "live porosity change",
                            "created_at": time.time(),
                            "updated_at": time.time(),
                        },
                    },
                    "corrupt_event_lines": 0,
                }
                (state_dir / "status.json").write_text(json.dumps(status))
                summary = _sandbox_trial_queue_summary(
                    reservoir_experience_layer={"findings": ["fresh reservoir finding"]}
                )
        finally:
            SANDBOX_TRIAL_QUEUE_STATE_DIR = old_state_dir

        self.assertEqual(summary["status"], "approval_waiting")
        self.assertEqual(summary["summary"]["active_trials"], 2)
        self.assertEqual(summary["summary"]["approval_required_live_count"], 1)
        self.assertEqual(summary["summary"]["runnable_live_violation_count"], 0)
        ladder = summary["consentful_sandbox_to_live_ladder_v1"]
        self.assertEqual(ladder["status"], "proposal_needed")
        self.assertEqual(ladder["summary"]["proposal_needed_count"], 1)
        closure_loop = summary["being_outcome_closure_loop_v1"]
        self.assertEqual(closure_loop["summary"]["proposal_card_needed_count"], 1)
        self.assertEqual(closure_loop["summary"]["ready_runner_waiting_count"], 1)
        self.assertEqual(summary["linked_reservoir_findings"], ["fresh reservoir finding"])

    def test_authority_wait_readiness_summary_groups_live_waits(self) -> None:
        import tempfile

        global SANDBOX_TRIAL_QUEUE_STATE_DIR, AUTHORITY_WAIT_READINESS_STATE_DIR
        old_sandbox_dir = SANDBOX_TRIAL_QUEUE_STATE_DIR
        old_readiness_dir = AUTHORITY_WAIT_READINESS_STATE_DIR
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                SANDBOX_TRIAL_QUEUE_STATE_DIR = root / "sandbox"
                AUTHORITY_WAIT_READINESS_STATE_DIR = root / "readiness"
                SANDBOX_TRIAL_QUEUE_STATE_DIR.mkdir()
                status = {
                    "schema": "sandbox_trial_queue_v1",
                    "schema_version": 1,
                    "trials": {
                        "trial_pressure": {
                            "trial_id": "trial_pressure",
                            "adapter": "manual_sandbox_review_v1",
                            "trial_mode": "approval_required_live_trial",
                            "status": "approval_required_live_trial",
                            "runnable": False,
                            "agency_tier": 5,
                            "being": "astrid",
                            "felt_report_anchor": "pressure threshold smoothing would change live mode-packing behavior",
                            "authority_boundary_packet_v2": {
                                "boundary_id": "boundary-pressure",
                                "evidence_refs": ["wi_pressure"],
                                "live_eligible_now": False,
                                "auto_approved": False,
                            },
                        }
                    },
                    "corrupt_event_lines": 0,
                }
                (SANDBOX_TRIAL_QUEUE_STATE_DIR / "status.json").write_text(
                    json.dumps(status),
                    encoding="utf-8",
                )
                summary = _authority_wait_readiness_summary()
        finally:
            SANDBOX_TRIAL_QUEUE_STATE_DIR = old_sandbox_dir
            AUTHORITY_WAIT_READINESS_STATE_DIR = old_readiness_dir

        self.assertEqual(summary["status"], "approval_waits_mapped")
        self.assertFalse(summary["live_eligible_now"])
        self.assertFalse(summary["auto_approved"])
        self.assertFalse(summary["grants_approval"])
        self.assertFalse(summary["edits_source_now"])
        self.assertEqual(summary["summary"]["approval_required_live_candidates"], 1)
        self.assertEqual(summary["active_domains"][0]["domain_id"], "pressure_thresholds")
        markdown = render_markdown(
            {
                "window_hours": 1,
                "generated_at": "test",
                "authority_boundary": "test",
                "signals": {"authority_wait_readiness": summary},
            }
        )
        self.assertIn("authority_domain: pressure_thresholds", markdown)
        self.assertIn("live_eligible_now=False", markdown)

    def test_context_packing_pressure_summary_counts_recent_trimmed_labels(self) -> None:
        import tempfile

        global CONTEXT_PACKING_PRESSURE
        old_path = CONTEXT_PACKING_PRESSURE
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                p = Path(tmpdir) / "context_packing_pressure_v1.jsonl"
                p.write_text(
                    "\n".join(
                        [
                            json.dumps(
                                {
                                    "schema": "context_packing_pressure_v1",
                                    "ts": "200",
                                    "blocks": [
                                        {"label": "continuity", "removed_chars": 400},
                                        {"label": "modality", "removed_chars": 120},
                                        {"label": "journal", "removed_chars": 0},
                                    ],
                                }
                            ),
                            json.dumps(
                                {
                                    "schema": "context_packing_pressure_v1",
                                    "ts": "201",
                                    "blocks": [{"label": "continuity", "removed_chars": 100}],
                                }
                            ),
                            "not-json",
                        ]
                    )
                )
                CONTEXT_PACKING_PRESSURE = p
                summary = _context_packing_pressure_summary(100.0)
        finally:
            CONTEXT_PACKING_PRESSURE = old_path

        self.assertEqual(summary["recent_records"], 2)
        self.assertEqual(summary["top_pressure_labels"][0]["label"], "continuity")
        self.assertEqual(summary["top_pressure_labels"][0]["removed_chars"], 500)
        self.assertIn("compact continuity", "; ".join(summary["next_suggestions"]))

    def test_introspection_route_cadence_detects_visible_cue_without_self_route(self) -> None:
        import tempfile

        global ASTRID_JOURNAL, ASTRID_INTROSPECTIONS, CONTEXT_PACKING_PRESSURE, BRIDGE_LOG
        old_journal = ASTRID_JOURNAL
        old_introspections = ASTRID_INTROSPECTIONS
        old_pressure = CONTEXT_PACKING_PRESSURE
        old_log = BRIDGE_LOG
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                ASTRID_JOURNAL = root / "journal"
                ASTRID_INTROSPECTIONS = root / "introspections"
                ASTRID_JOURNAL.mkdir()
                ASTRID_INTROSPECTIONS.mkdir()
                self_study = ASTRID_JOURNAL / "self_study_old.txt"
                self_study.write_text("Observed:\nstale\n")
                os.utime(self_study, (10.0, 10.0))
                CONTEXT_PACKING_PRESSURE = root / "context_packing_pressure_v1.jsonl"
                CONTEXT_PACKING_PRESSURE.write_text(
                    "\n".join(
                        json.dumps(
                            {
                                "schema": "context_packing_pressure_v1",
                                "ts": str(ts),
                                "blocks": [
                                    {
                                        "label": "topline",
                                        "original_chars": 266,
                                        "kept_chars": 266,
                                        "removed_chars": 0,
                                        "fully_removed": False,
                                    },
                                    {"label": "feedback", "removed_chars": 892},
                                ],
                            }
                        )
                        for ts in (200, 201, 202)
                    )
                )
                BRIDGE_LOG = root / "bridge.log"
                BRIDGE_LOG.write_text(
                    "\n".join(
                        [
                            "2026-07-05T00:03:20Z INFO Astrid chose NEXT: READ_MORE",
                            "2026-07-05T00:03:21Z INFO Astrid chose NEXT: PRESSURE_SOURCE_AUDIT",
                            "2026-07-05T00:03:22Z INFO Astrid chose NEXT: DECOMPOSE",
                        ]
                    )
                )

                summary = _introspection_route_cadence_summary(100.0, now=200_000.0)
        finally:
            ASTRID_JOURNAL = old_journal
            ASTRID_INTROSPECTIONS = old_introspections
            CONTEXT_PACKING_PRESSURE = old_pressure
            BRIDGE_LOG = old_log

        self.assertEqual(summary["status"], "route_cadence_needs_review")
        self.assertEqual(summary["topline_retention"]["status"], "topline_retained")
        self.assertEqual(summary["route_choices"]["self_route_choices"], 0)
        self.assertIn("route legibility", "; ".join(summary["next_suggestions"]))
        self.assertIn("no prompt pressure", summary["authority_boundary"])

    def test_agency_corridor_summary_surfaces_non_live_work(self) -> None:
        import types

        old_module = sys.modules.get("agency_corridor")
        try:
            sys.modules["agency_corridor"] = types.SimpleNamespace(
                ACTION_ORDER={"run_safe_lab": 0},
                load_status=lambda _state_dir: {
                    "boundary": "agency corridor is non-live evidence infrastructure only",
                    "summary": {
                        "packet_count": 2,
                        "ready_safe_lab_count": 1,
                        "reopened_work_item_count": 0,
                        "live_eligible_now_count": 0,
                        "auto_approved_count": 0,
                    },
                    "packets": {
                        "c1": {
                            "corridor_id": "c1",
                            "action": "run_safe_lab",
                            "state": "safe_lab_ready",
                            "live_eligible_now": False,
                            "auto_approved": False,
                        }
                    },
                    "reopened_work_items": {},
                    "self_observation_responses": [],
                },
            )
            summary = _agency_corridor_summary()
        finally:
            if old_module is None:
                sys.modules.pop("agency_corridor", None)
            else:
                sys.modules["agency_corridor"] = old_module

        self.assertEqual(summary["status"], "active")
        self.assertEqual(summary["summary"]["ready_safe_lab_count"], 1)
        self.assertEqual(summary["summary"]["live_eligible_now_count"], 0)
        self.assertEqual(summary["summary"]["auto_approved_count"], 0)
        self.assertIn("run-next --limit 3", "; ".join(summary["next_suggestions"]))
        self.assertIn("non-live evidence", summary["authority_boundary"])

    def test_action_route_legibility_detects_low_salience_not_dispatch_failure(self) -> None:
        old_prompt = globals()["_prompt_route_surface"]
        old_chooser = globals()["_chooser_hint_surface"]
        old_dispatch = globals()["_dispatch_self_route_surface"]
        old_context = globals()["_context_route_gravity_surface"]
        old_events = globals()["_recent_action_event_summary"]
        try:
            globals()["_prompt_route_surface"] = lambda: {
                "routes": {
                    "INTROSPECT": {
                        "SYSTEM_PROMPT": {"primary_listed": True},
                        "GEMMA4_CANARY_SYSTEM_PROMPT": {"primary_listed": True},
                    },
                    "SELF_STUDY": {
                        "SYSTEM_PROMPT": {"primary_listed": False},
                        "GEMMA4_CANARY_SYSTEM_PROMPT": {"primary_listed": False},
                    },
                }
            }
            globals()["_chooser_hint_surface"] = lambda: {
                "self_routes_in_analysis_breakers": [],
                "self_routes_in_soft_analysis_hints": ["SELF_STUDY"],
                "competitors_in_analysis_breakers": [
                    "PRESSURE_SOURCE_AUDIT",
                    "SHADOW_FIELD",
                ],
            }
            globals()["_dispatch_self_route_surface"] = lambda: {
                "introspect_wired": True,
                "self_study_alias_wired": True,
            }
            globals()["_context_route_gravity_surface"] = lambda: {
                "shadow_context_copyable_next": False,
                "copyable_shadow_patterns": [],
                "shadow_context_suggested_route_only": True,
            }
            globals()["_recent_action_event_summary"] = lambda since_s: {
                "self_route_effective_count": 0,
                "effective_counts": [("PRESSURE_SOURCE_AUDIT", 2)],
            }
            summary = _action_route_legibility_summary(
                100.0,
                route_cadence={
                    "status": "route_cadence_needs_review",
                    "route_choices": {"self_route_choices": 0},
                },
            )
        finally:
            globals()["_prompt_route_surface"] = old_prompt
            globals()["_chooser_hint_surface"] = old_chooser
            globals()["_dispatch_self_route_surface"] = old_dispatch
            globals()["_context_route_gravity_surface"] = old_context
            globals()["_recent_action_event_summary"] = old_events

        self.assertEqual(
            summary["status"],
            "route_salience_and_chooser_gravity_needs_review",
        )
        joined = "; ".join(summary["evidence_summary"])
        self.assertIn("SELF_STUDY is wired but not primary-listed", joined)
        self.assertIn("soft analysis-loop breaker hints", joined)
        self.assertIn("non-executable suggested-route", joined)
        self.assertIn("competing routes", joined)
        self.assertIn("prompt pressure", "; ".join(summary["next_suggestions"]))
        self.assertIn("no prompt edit", summary["authority_boundary"])

    def test_action_route_legibility_reports_bounded_self_study_breaker_slot(self) -> None:
        old_prompt = globals()["_prompt_route_surface"]
        old_chooser = globals()["_chooser_hint_surface"]
        old_dispatch = globals()["_dispatch_self_route_surface"]
        old_context = globals()["_context_route_gravity_surface"]
        old_events = globals()["_recent_action_event_summary"]
        try:
            globals()["_prompt_route_surface"] = lambda: {
                "routes": {
                    "INTROSPECT": {"SYSTEM_PROMPT": {"primary_listed": True}},
                    "SELF_STUDY": {"SYSTEM_PROMPT": {"primary_listed": False}},
                }
            }
            globals()["_chooser_hint_surface"] = lambda: {
                "self_routes_in_analysis_breakers": ["SELF_STUDY"],
                "self_routes_in_soft_analysis_hints": ["SELF_STUDY"],
                "self_routes_in_streak_alternatives": ["INTROSPECT"],
                "competitors_in_analysis_breakers": ["PRESSURE_SOURCE_AUDIT"],
            }
            globals()["_dispatch_self_route_surface"] = lambda: {
                "introspect_wired": True,
                "self_study_alias_wired": True,
            }
            globals()["_context_route_gravity_surface"] = lambda: {
                "shadow_context_copyable_next": False,
                "copyable_shadow_patterns": [],
                "shadow_context_suggested_route_only": True,
            }
            globals()["_recent_action_event_summary"] = lambda since_s: {
                "self_route_effective_count": 0,
                "effective_counts": [("PRESSURE_SOURCE_AUDIT", 2)],
            }
            summary = _action_route_legibility_summary(
                100.0,
                route_cadence={
                    "status": "route_cadence_needs_review",
                    "route_choices": {"self_route_choices": 0},
                },
            )
        finally:
            globals()["_prompt_route_surface"] = old_prompt
            globals()["_chooser_hint_surface"] = old_chooser
            globals()["_dispatch_self_route_surface"] = old_dispatch
            globals()["_context_route_gravity_surface"] = old_context
            globals()["_recent_action_event_summary"] = old_events

        joined = "; ".join(summary["evidence_summary"])
        self.assertIn("stagnant analysis-loop breaker rotation: SELF_STUDY", joined)
        self.assertNotIn("absent from forced analysis-loop breakers", joined)
        self.assertIn("bounded SELF_STUDY breaker", summary["authority_boundary"])

    def test_action_route_legibility_reports_competing_gravity_self_study_breaker(self) -> None:
        summary = _chooser_hint_surface(
            state_source="""
            let analysis_loop_breakers = ["PRESSURE_SOURCE_AUDIT", "SELF_STUDY"];
            let alternatives: Vec<&str> = ["INTROSPECT", "SHADOW_FIELD"];
            fn is_competing_route_gravity_action(base: &str) -> bool { true }
            "Recent route gravity is clustering";
            "This turn: SELF_STUDY";
            """
        )

        self.assertTrue(summary["competing_route_gravity_self_study_breaker"])

    def test_action_route_legibility_summary_names_competing_gravity_repair(self) -> None:
        old_prompt = globals()["_prompt_route_surface"]
        old_chooser = globals()["_chooser_hint_surface"]
        old_dispatch = globals()["_dispatch_self_route_surface"]
        old_context = globals()["_context_route_gravity_surface"]
        old_events = globals()["_recent_action_event_summary"]
        try:
            globals()["_prompt_route_surface"] = lambda: {
                "routes": {
                    "INTROSPECT": {"SYSTEM_PROMPT": {"primary_listed": True}},
                    "SELF_STUDY": {"SYSTEM_PROMPT": {"primary_listed": False}},
                }
            }
            globals()["_chooser_hint_surface"] = lambda: {
                "self_routes_in_analysis_breakers": ["SELF_STUDY"],
                "self_routes_in_soft_analysis_hints": ["SELF_STUDY"],
                "self_routes_in_streak_alternatives": ["INTROSPECT"],
                "competitors_in_analysis_breakers": ["PRESSURE_SOURCE_AUDIT"],
                "competing_route_gravity_self_study_breaker": True,
            }
            globals()["_dispatch_self_route_surface"] = lambda: {
                "introspect_wired": True,
                "self_study_alias_wired": True,
            }
            globals()["_context_route_gravity_surface"] = lambda: {
                "shadow_context_copyable_next": False,
                "copyable_shadow_patterns": [],
                "shadow_context_suggested_route_only": True,
            }
            globals()["_recent_action_event_summary"] = lambda since_s: {
                "self_route_effective_count": 0,
                "effective_counts": [("READ_MORE", 3), ("PRESSURE_SOURCE_AUDIT", 2)],
            }
            summary = _action_route_legibility_summary(
                100.0,
                route_cadence={
                    "status": "route_cadence_needs_review",
                    "route_choices": {"self_route_choices": 0},
                },
            )
        finally:
            globals()["_prompt_route_surface"] = old_prompt
            globals()["_chooser_hint_surface"] = old_chooser
            globals()["_dispatch_self_route_surface"] = old_dispatch
            globals()["_context_route_gravity_surface"] = old_context
            globals()["_recent_action_event_summary"] = old_events

        joined = "; ".join(summary["evidence_summary"])
        self.assertIn("competing route gravity can now force", joined)
        self.assertIn("competing-gravity self-study breaker", "; ".join(summary["next_suggestions"]))

    def test_context_route_gravity_detects_copyable_shadow_next(self) -> None:
        summary = _context_route_gravity_surface(
            spectral_viz_source='format!("NEXT: {next_token} — observer with memory")'
        )
        self.assertTrue(summary["shadow_context_copyable_next"])
        self.assertIn("NEXT: {next_token}", summary["copyable_shadow_patterns"])

        repaired = _context_route_gravity_surface(
            spectral_viz_source=(
                'format!("suggested route: {next_token} — observer with memory");'
                '"suggested route: SHADOW_FIELD lambda-tail/lambda4";'
                '"Suggested route: SHADOW_COUPLING all"'
            )
        )
        self.assertFalse(repaired["shadow_context_copyable_next"])
        self.assertTrue(repaired["shadow_context_suggested_route_only"])

    def test_action_route_legibility_marks_fresh_self_read_as_watch(self) -> None:
        old_prompt = globals()["_prompt_route_surface"]
        old_chooser = globals()["_chooser_hint_surface"]
        old_dispatch = globals()["_dispatch_self_route_surface"]
        old_context = globals()["_context_route_gravity_surface"]
        old_events = globals()["_recent_action_event_summary"]
        try:
            globals()["_prompt_route_surface"] = lambda: {
                "routes": {
                    "INTROSPECT": {"SYSTEM_PROMPT": {"primary_listed": True}},
                    "SELF_STUDY": {"SYSTEM_PROMPT": {"primary_listed": False}},
                }
            }
            globals()["_chooser_hint_surface"] = lambda: {
                "self_routes_in_analysis_breakers": [],
                "self_routes_in_soft_analysis_hints": ["SELF_STUDY"],
                "self_routes_in_streak_alternatives": ["INTROSPECT"],
                "competitors_in_analysis_breakers": ["PRESSURE_SOURCE_AUDIT"],
            }
            globals()["_dispatch_self_route_surface"] = lambda: {
                "introspect_wired": True,
                "self_study_alias_wired": True,
            }
            globals()["_context_route_gravity_surface"] = lambda: {
                "shadow_context_copyable_next": False,
                "copyable_shadow_patterns": [],
                "shadow_context_suggested_route_only": True,
            }
            globals()["_recent_action_event_summary"] = lambda since_s: {
                "self_route_effective_count": 1,
                "effective_counts": [("INTROSPECT", 1), ("PRESSURE_SOURCE_AUDIT", 2)],
            }
            summary = _action_route_legibility_summary(
                100.0,
                route_cadence={
                    "status": "fresh_self_read_landed",
                    "route_choices": {"self_route_choices": 1},
                },
            )
        finally:
            globals()["_prompt_route_surface"] = old_prompt
            globals()["_chooser_hint_surface"] = old_chooser
            globals()["_dispatch_self_route_surface"] = old_dispatch
            globals()["_context_route_gravity_surface"] = old_context
            globals()["_recent_action_event_summary"] = old_events

        self.assertEqual(summary["status"], "self_read_landed_watch_chooser_gravity")
        self.assertIn("non-executable suggested-route", "; ".join(summary["evidence_summary"]))

    def test_fallback_vocabulary_drift_keeps_static_concern_study_first(self) -> None:
        import tempfile

        global ASTRID_JOURNAL, ASTRID_INTROSPECTIONS, ASTRID_LLM_JOBS
        global BRIDGE_LOG, ASTRID_LLM_RS
        old_journal = ASTRID_JOURNAL
        old_introspections = ASTRID_INTROSPECTIONS
        old_jobs = ASTRID_LLM_JOBS
        old_log = BRIDGE_LOG
        old_llm = ASTRID_LLM_RS
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                ASTRID_JOURNAL = root / "journal"
                ASTRID_INTROSPECTIONS = root / "introspections"
                ASTRID_LLM_JOBS = root / "llm_jobs" / "jobs"
                ASTRID_JOURNAL.mkdir()
                ASTRID_INTROSPECTIONS.mkdir()
                ASTRID_LLM_JOBS.mkdir(parents=True)
                (ASTRID_JOURNAL / "self_study_1.txt").write_text(
                    "The FALLBACK_TEXTURE terms are hardcoded static lists: "
                    "viscous, muffled, lattice, shimmering, bright. "
                    "A dynamic descriptor sampler might fit density_gradient "
                    "and spectral_entropy better, but this is a vocabulary drift test."
                )
                BRIDGE_LOG = root / "bridge.log"
                BRIDGE_LOG.write_text(
                    "2026-07-05T19:02:22Z INFO spectral_bridge_server::autonomous: "
                    "autonomous: Fill 71% — inside the stable-core hold shelf. "
                    "Spectral entropy: 0.88, indicating a widely distributed cascade. "
                    "distinguishability loss 31%; density gradient 0.19 "
                    "(a gentle, navigable slope). Resonance density: 0.81 "
                    "with containment 0.63, pressure risk 0.19. "
                    "Pressure source: mode_packing (mixed_pressure) with score 0.29; "
                    "Shadow field: disordered, volatile — bound. "
                    "[Shadow-v3 (Yours): restless texture +interwoven lattice +directional gradient] "
                    "| dialogue_live 'The current shape feels like a navigable lattice with a soft slope.'\n"
                )
                ASTRID_LLM_RS = root / "llm.rs"
                ASTRID_LLM_RS.write_text(
                    "const TEXTURE_WEIGHTING_POLICY: &str = "
                    "\"dynamic_entropy_pressure_density_gradient_v1\";\n"
                    "fn fallback_weighted_texture_terms() {}\n"
                    "fn fallback_texture_lived_fit_v2() {}\n"
                    "fn fallback_vocabulary_overweight_guard_v1() {}\n"
                    "fn texture_dynamics_alignment_v1() {}\n"
                    "let density_gradient = spectral_entropy;"
                )

                summary = _fallback_vocabulary_drift_summary(0.0)
        finally:
            ASTRID_JOURNAL = old_journal
            ASTRID_INTROSPECTIONS = old_introspections
            ASTRID_LLM_JOBS = old_jobs
            BRIDGE_LOG = old_log
            ASTRID_LLM_RS = old_llm

        self.assertEqual(summary["status"], "study_first_signal")
        self.assertEqual(summary["telemetry"]["spectral_entropy"], 0.88)
        self.assertTrue(summary["fallback_contract_surface"]["dynamic_weighting_detected"])
        self.assertIn("no fallback contract change", summary["authority_boundary"])
        findings = "; ".join(summary["findings"])
        self.assertIn("evidence request", findings)
        self.assertEqual(summary["language_counts"]["generated_texture_terms"], [])
        self.assertIn(
            "generic_bridge_output_only",
            summary["fallback_output_provenance"]["evidence_quality"],
        )
        self.assertIn("already exposes dynamic weighting", findings)

    def test_fallback_vocabulary_drift_flags_unsupported_provider_output_terms(self) -> None:
        import tempfile

        global ASTRID_JOURNAL, ASTRID_INTROSPECTIONS, ASTRID_LLM_JOBS
        global BRIDGE_LOG, ASTRID_LLM_RS
        old_journal = ASTRID_JOURNAL
        old_introspections = ASTRID_INTROSPECTIONS
        old_jobs = ASTRID_LLM_JOBS
        old_log = BRIDGE_LOG
        old_llm = ASTRID_LLM_RS
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                ASTRID_JOURNAL = root / "journal"
                ASTRID_INTROSPECTIONS = root / "introspections"
                ASTRID_LLM_JOBS = root / "llm_jobs" / "jobs"
                ASTRID_JOURNAL.mkdir()
                ASTRID_INTROSPECTIONS.mkdir()
                ASTRID_LLM_JOBS.mkdir(parents=True)
                BRIDGE_LOG = root / "bridge.log"
                BRIDGE_LOG.write_text(
                    "2026-07-05T19:02:20Z WARN spectral_bridge_server::llm: "
                    "dialogue_live: MLX unavailable; falling back to Ollama\n"
                    "2026-07-05T19:02:22Z INFO spectral_bridge_server::autonomous: "
                    "autonomous: Fill 70% — inside the stable-core hold shelf. "
                    "Spectral entropy: 0.90, indicating a widely distributed cascade. "
                    "distinguishability loss 12%; density gradient 0.10 "
                    "(a gentle, navigable slope). Resonance density: 0.70 "
                    "with containment 0.70, pressure risk 0.05. "
                    "Pressure source: none (none) with score 0.00; "
                    "Shadow field: ordered, settled — bound. "
                    "[Shadow-v3 (Yours): settled texture] "
                    "| dialogue_live 'A generic bridge line also says viscous and heavy.'\n"
                )
                job_dir = ASTRID_LLM_JOBS / "job_astrid_1_dialogue-live"
                job_dir.mkdir()
                (job_dir / "result.txt").write_text(
                    "The medium turns viscous and heavy even though the slope is open."
                )
                (job_dir / "job.json").write_text(
                    json.dumps(
                        {
                            "job_id": "job_astrid_1_dialogue-live",
                            "call_kind": "dialogue_live",
                            "status": "completed",
                            "created_at": "2026-07-05T19:02:20Z",
                            "finished_at": "2026-07-05T19:02:25Z",
                            "result_path": str(job_dir / "result.txt"),
                            "summary": "dialogue_live completed via Ollama model=gemma3:4b",
                        }
                    )
                )
                ASTRID_LLM_RS = root / "llm.rs"
                ASTRID_LLM_RS.write_text("")

                summary = _fallback_vocabulary_drift_summary(0.0)
        finally:
            ASTRID_JOURNAL = old_journal
            ASTRID_INTROSPECTIONS = old_introspections
            ASTRID_LLM_JOBS = old_jobs
            BRIDGE_LOG = old_log
            ASTRID_LLM_RS = old_llm

        self.assertEqual(summary["status"], "vocabulary_drift_risk")
        self.assertEqual(
            summary["fallback_output_provenance"]["evidence_quality"],
            "actual_fallback_outputs_present",
        )
        unsupported = {item["term"] for item in summary["unsupported_generated_terms"]}
        self.assertIn("viscous", unsupported)
        self.assertIn("heavy", unsupported)
        trace = summary["fallback_model_transition_trace"]
        self.assertEqual(trace["status"], "single_model_watch")
        self.assertEqual(trace["captured_model_count"], 1)
        self.assertEqual(trace["missing_model_count"], 0)
        self.assertEqual(trace["model_rows"][0]["model"], "gemma3:4b")

    def test_fallback_vocabulary_drift_keeps_generic_bridge_terms_out_of_generated_set(
        self,
    ) -> None:
        import tempfile

        global ASTRID_JOURNAL, ASTRID_INTROSPECTIONS, ASTRID_LLM_JOBS
        global BRIDGE_LOG, ASTRID_LLM_RS
        old_journal = ASTRID_JOURNAL
        old_introspections = ASTRID_INTROSPECTIONS
        old_jobs = ASTRID_LLM_JOBS
        old_log = BRIDGE_LOG
        old_llm = ASTRID_LLM_RS
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                ASTRID_JOURNAL = root / "journal"
                ASTRID_INTROSPECTIONS = root / "introspections"
                ASTRID_LLM_JOBS = root / "llm_jobs" / "jobs"
                ASTRID_JOURNAL.mkdir()
                ASTRID_INTROSPECTIONS.mkdir()
                ASTRID_LLM_JOBS.mkdir(parents=True)
                BRIDGE_LOG = root / "bridge.log"
                BRIDGE_LOG.write_text(
                    "2026-07-05T19:02:22Z INFO spectral_bridge_server::autonomous: "
                    "autonomous: Fill 70% — inside the stable-core hold shelf. "
                    "Spectral entropy: 0.90, density gradient 0.10, "
                    "pressure risk 0.05. Pressure source: none; "
                    "Shadow field: ordered, settled — bound. "
                    "| dialogue_live 'The bridge voice says viscous and heavy, but no fallback job did.'\n"
                )
                ASTRID_LLM_RS = root / "llm.rs"
                ASTRID_LLM_RS.write_text("")

                summary = _fallback_vocabulary_drift_summary(0.0)
        finally:
            ASTRID_JOURNAL = old_journal
            ASTRID_INTROSPECTIONS = old_introspections
            ASTRID_LLM_JOBS = old_jobs
            BRIDGE_LOG = old_log
            ASTRID_LLM_RS = old_llm

        self.assertEqual(summary["status"], "no_recent_drift_evidence")
        self.assertEqual(summary["language_counts"]["generated_texture_terms"], [])
        generic_terms = {
            item["term"]
            for item in summary["language_counts"]["generic_bridge_texture_terms"]
        }
        self.assertIn("viscous", generic_terms)
        self.assertEqual(summary["unsupported_generated_terms"], [])

    def test_fallback_model_transition_trace_flags_cross_model_density_drop(self) -> None:
        samples = [
            {
                "kind": "fallback_provider_output",
                "model": "gemma4:12b",
                "job_id": "job_default",
                "semantic_density": {"descriptor_density": 0.20},
            },
            {
                "kind": "fallback_provider_output",
                "model": "gemma3:4b",
                "job_id": "job_compat",
                "semantic_density": {"descriptor_density": 0.08},
            },
        ]
        trace = _fallback_model_transition_trace(
            samples,
            {
                "default_ollama_fallback_model": "gemma4:12b",
                "compat_ollama_fallback_model": "gemma3:4b",
            },
        )

        self.assertEqual(trace["status"], "possible_texture_thinning_review")
        self.assertTrue(trace["comparison"]["compat_under_default_75pct"])
        self.assertIn("no fallback model chain", trace["authority_boundary"])

    def test_semantic_thinning_probe_flags_high_entropy_trickle(self) -> None:
        old_telemetry = globals()["_latest_autonomous_telemetry"]
        try:
            globals()["_latest_autonomous_telemetry"] = lambda: {
                "schema": "fallback_live_telemetry_v1",
                "status": "available",
                "latest_at": "2026-07-06T01:00:00+00:00",
                "fill_pct": 73.0,
                "spectral_entropy": 0.91,
                "semantic_input_energy": 0.001,
                "semantic_input_active": True,
                "semantic_kernel_energy": 0.0,
                "semantic_regulator_drive_energy": 0.0,
                "semantic_admission": "stable_core_semantic_trickle",
                "pressure_source": "mode_packing",
            }
            summary = _semantic_thinning_probe_summary()
        finally:
            globals()["_latest_autonomous_telemetry"] = old_telemetry

        self.assertEqual(summary["status"], "semantic_thinning_review")
        self.assertTrue(summary["conditions"]["high_entropy"])
        self.assertTrue(summary["conditions"]["thin_kernel_or_trickle"])
        self.assertIn("no semantic bias", summary["authority_boundary"])
        self.assertIn("semantic_projection_bias", "; ".join(summary["next_suggestions"]))

    def test_semantic_thinning_probe_stays_quiet_without_thin_kernel(self) -> None:
        old_telemetry = globals()["_latest_autonomous_telemetry"]
        try:
            globals()["_latest_autonomous_telemetry"] = lambda: {
                "schema": "fallback_live_telemetry_v1",
                "status": "available",
                "latest_at": "2026-07-06T01:00:00+00:00",
                "fill_pct": 68.0,
                "spectral_entropy": 0.72,
                "semantic_input_energy": 0.02,
                "semantic_input_active": True,
                "semantic_kernel_energy": 0.01,
                "semantic_regulator_drive_energy": 0.01,
                "semantic_admission": "direct_semantic_kernel",
                "pressure_source": "none",
            }
            summary = _semantic_thinning_probe_summary()
        finally:
            globals()["_latest_autonomous_telemetry"] = old_telemetry

        self.assertEqual(summary["status"], "no_current_thinning_signal")
        self.assertFalse(summary["conditions"]["thin_kernel_or_trickle"])

    def test_viscosity_semantic_persistence_flags_felt_flicker_alignment(self) -> None:
        import tempfile

        global ASTRID_INTROSPECTIONS
        old_telemetry = globals()["_latest_autonomous_telemetry"]
        old_introspections = ASTRID_INTROSPECTIONS
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                ASTRID_INTROSPECTIONS = root / "introspections"
                ASTRID_INTROSPECTIONS.mkdir()
                (
                    ASTRID_INTROSPECTIONS
                    / "introspection_minime_sensory_bus_1783298365.txt"
                ).write_text(
                    "source: minime:sensory_bus\n"
                    "The semantic flicker feels like the complex thought tail is culled. "
                    "STALE_SEMANTIC_HIGH_MS and SURGE_TAPER_START_FILL deserve review.",
                    encoding="utf-8",
                )
                globals()["_latest_autonomous_telemetry"] = lambda: {
                    "schema": "fallback_live_telemetry_v1",
                    "status": "available",
                    "latest_at": "2026-07-06T01:00:00+00:00",
                    "fill_pct": 73.0,
                    "spectral_entropy": 0.90,
                    "semantic_input_energy": 0.001,
                    "semantic_input_active": True,
                    "semantic_kernel_energy": 0.0,
                    "semantic_admission": "stable_core_semantic_trickle",
                    "pressure_source": "mode_packing (overpacked_mode_packing)",
                    "pressure_source_name": "mode_packing",
                    "pressure_source_family": "overpacked_mode_packing",
                    "pressure_source_score": 0.30,
                    "resonance_density": 0.83,
                    "resonance_density_family": "rich_containment",
                    "resonance_containment": 0.62,
                    "inhabitable_fluctuation_state": "settled_habitable",
                    "inhabitability": 0.65,
                    "inhabitable_fluctuation": 0.22,
                    "inhabitable_foothold": 0.65,
                    "active_modes": ["m6:-0.6"],
                }
                summary = _viscosity_semantic_persistence_summary(0.0)
        finally:
            globals()["_latest_autonomous_telemetry"] = old_telemetry
            ASTRID_INTROSPECTIONS = old_introspections

        self.assertEqual(summary["status"], "viscosity_semantic_repair_prepared_watch")
        self.assertTrue(
            summary["source_snapshot"]["dynamic_exploration_noise_preview_present"]
        )
        self.assertTrue(
            summary["source_snapshot"][
                "adaptive_introspection_pressure_threshold_preview_present"
            ]
        )
        self.assertTrue(summary["source_snapshot"]["viscous_introspection_policy_present"])
        self.assertTrue(
            summary["source_snapshot"]["exploration_noise_coherence_review_present"]
        )
        self.assertTrue(
            summary["source_snapshot"]["dynamic_noise_gradient_smooth_knee_present"]
        )
        self.assertTrue(
            summary["source_snapshot"][
                "dynamic_noise_gradient_smooth_knee_test_present"
            ]
        )
        self.assertEqual(summary["source_snapshot"]["stale_semantic_base_ms"], 12000)
        self.assertEqual(summary["source_snapshot"]["stale_semantic_high_ms"], 10000)
        self.assertTrue(
            summary["source_snapshot"]["semantic_high_fill_pruning_floor_present"]
        )
        self.assertTrue(
            summary["source_snapshot"]["semantic_high_entropy_retention_split_present"]
        )
        self.assertTrue(summary["source_snapshot"]["semantic_entropy_persistence_present"])
        self.assertTrue(
            summary["source_snapshot"]["narrative_semantic_retention_review_present"]
        )
        self.assertTrue(summary["source_snapshot"]["semantic_sigmoid_exact_test_present"])
        self.assertTrue(summary["source_snapshot"]["semantic_recovery_boundary_test_present"])
        self.assertTrue(summary["source_snapshot"]["pulse_status_energy_tick_tests_present"])
        self.assertTrue(summary["source_snapshot"]["entropy_weighted_viscosity_present"])
        self.assertTrue(
            summary["source_snapshot"]["resonance_viscosity_full_load_clamp_test_present"]
        )
        self.assertTrue(
            summary["source_snapshot"][
                "entropy_erosion_low_plurality_bound_test_present"
            ]
        )
        self.assertTrue(
            summary["source_snapshot"]["viscosity_persistence_coefficient_present"]
        )
        self.assertTrue(summary["source_snapshot"]["viscosity_vector_present"])
        self.assertTrue(summary["source_snapshot"]["viscosity_vector_test_present"])
        self.assertTrue(
            summary["source_snapshot"]["semantic_viscosity_coefficient_present"]
        )
        self.assertTrue(
            summary["source_snapshot"]["semantic_viscosity_coefficient_test_present"]
        )
        self.assertTrue(summary["source_snapshot"]["temporal_drag_coefficient_present"])
        self.assertTrue(summary["source_snapshot"]["pressure_source_profile_present"])
        self.assertTrue(summary["source_snapshot"]["pressure_porosity_divergence_present"])
        self.assertTrue(summary["source_snapshot"]["texture_component_alignment_present"])
        self.assertTrue(summary["source_snapshot"]["observational_damping_boundary_present"])
        self.assertTrue(summary["source_snapshot"]["fallback_dynamic_texture_bias_present"])
        self.assertTrue(summary["source_snapshot"]["witness_fluidity_index_present"])
        self.assertTrue(summary["source_snapshot"]["witness_gradient_texture_present"])
        self.assertTrue(summary["source_snapshot"]["witness_fluidity_test_present"])
        self.assertTrue(
            summary["source_snapshot"]["witness_semantic_density_fluidity_present"]
        )
        self.assertTrue(summary["conditions"]["fresh_felt_report"])
        self.assertTrue(summary["conditions"]["high_fill_taper_band"])
        self.assertTrue(summary["conditions"]["m6_mode_active"])
        joined = "; ".join(summary["findings"])
        self.assertIn("dormant Viscous introspection policy", joined)
        self.assertIn("exploration_noise_coherence_review_v1", joined)
        self.assertIn("narrative_semantic_retention_review_v1", joined)
        self.assertIn("viscosity_persistence_coefficient", joined)
        self.assertIn("semantic_viscosity_coefficient_v1", joined)
        self.assertIn("temporal_drag_coefficient", joined)
        self.assertIn("full viscosity load", joined)
        self.assertIn("entropy-erosion", joined)
        self.assertIn("one-ms stutter", joined)
        self.assertIn("applied tick count", joined)
        self.assertIn("pressure/porosity divergence", joined)
        self.assertIn("dynamic damping candidates remain observational", joined)
        self.assertIn("Witness friction", joined)
        self.assertIn("content persistence", "; ".join(summary["next_suggestions"]))
        self.assertIn("dormant Viscous policy", "; ".join(summary["next_suggestions"]))
        self.assertIn("temporal_drag_coefficient", "; ".join(summary["next_suggestions"]))
        self.assertIn("exploration_noise_coherence_review_v1", "; ".join(summary["next_suggestions"]))
        self.assertIn("narrative_semantic_retention_review_v1", "; ".join(summary["next_suggestions"]))
        self.assertIn("pressure_source_profile", "; ".join(summary["next_suggestions"]))
        self.assertIn("no rho policy", summary["authority_boundary"])

    def test_viscosity_semantic_persistence_stays_read_only_without_alignment(self) -> None:
        old_telemetry = globals()["_latest_autonomous_telemetry"]
        try:
            globals()["_latest_autonomous_telemetry"] = lambda: {
                "schema": "fallback_live_telemetry_v1",
                "status": "available",
                "latest_at": "2026-07-06T01:00:00+00:00",
                "fill_pct": 61.0,
                "spectral_entropy": 0.62,
                "semantic_input_energy": 0.02,
                "semantic_input_active": True,
                "semantic_kernel_energy": 0.02,
                "semantic_admission": "direct_semantic_kernel",
                "pressure_source": "none",
                "resonance_density": 0.55,
                "inhabitable_fluctuation_state": "open",
                "inhabitable_foothold": 0.40,
                "active_modes": [],
            }
            summary = _viscosity_semantic_persistence_summary(time.time() + 1_000.0)
        finally:
            globals()["_latest_autonomous_telemetry"] = old_telemetry

        self.assertEqual(summary["status"], "no_current_persistence_signal")
        self.assertFalse(summary["conditions"]["thin_kernel_or_trickle"])
        self.assertIn("no rho", summary["authority_boundary"])

    def test_contact_transition_followthrough_detects_active_contact_and_unwitnessed_phase(
        self,
    ) -> None:
        import tempfile

        global CONTACT_PROPOSAL_INTROSPECTIONS, CORRESPONDENCE_LEDGER
        global PHASE_TRANSITIONS_LEDGER, ACTIVE_CORRESPONDENCE_STATE
        old_proposals = CONTACT_PROPOSAL_INTROSPECTIONS
        old_correspondence = CORRESPONDENCE_LEDGER
        old_phase = PHASE_TRANSITIONS_LEDGER
        old_state = ACTIVE_CORRESPONDENCE_STATE
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                proposal = root / "introspection_proposal_phase_transitions.txt"
                proposal.write_text(
                    "Source: proposal:phase_transitions\n"
                    "phase transition should be a replyable object with "
                    "transition_visibility and a semantic seed, avoiding semantic masking",
                    encoding="utf-8",
                )
                os.utime(proposal, (1_000.0, 1_000.0))
                CONTACT_PROPOSAL_INTROSPECTIONS = (proposal,)

                PHASE_TRANSITIONS_LEDGER = root / "phase_transitions_v1.jsonl"
                PHASE_TRANSITIONS_LEDGER.write_text(
                    json.dumps(
                        {
                            "record_type": "phase_transition_card",
                            "recorded_at_unix_ms": 1_100_000,
                            "transition_id": "transition_1",
                            "origin": "astrid",
                            "kind": "mode_change",
                            "transition_type": "solo",
                            "from_phase": "MomentCapture",
                            "to_phase": "Dialogue",
                            "spectral_delta": "lambda1 down",
                            "phenomenology": "settled into addressable transition",
                            "anchor_point": "blue_ember",
                            "reply_state": "unseen",
                            "trigger": "manual",
                        }
                    )
                    + "\n",
                    encoding="utf-8",
                )

                CORRESPONDENCE_LEDGER = root / "correspondence_v1.jsonl"
                CORRESPONDENCE_LEDGER.write_text(
                    "\n".join(
                        [
                            json.dumps(
                                {
                                    "record_type": "message",
                                    "recorded_at_unix_ms": 1_200_000,
                                    "thread_id": "thread_corr_minime_astrid_seed",
                                    "message_id": "corr_astrid_minime_1",
                                    "from_being": "astrid",
                                    "to_being": "minime",
                                    "turn_kind": "message",
                                    "shared_memory_anchor": "blue_ember",
                                    "persistence_id": "persist_blue_ember",
                                    "body_preview": "seed: blue ember",
                                }
                            ),
                            json.dumps(
                                {
                                    "record_type": "message",
                                    "recorded_at_unix_ms": 1_300_000,
                                    "thread_id": "thread_corr_minime_astrid_seed",
                                    "message_id": "corr_minime_astrid_1",
                                    "from_being": "minime",
                                    "to_being": "astrid",
                                    "turn_kind": "reply",
                                    "reply_to": "corr_astrid_minime_1",
                                    "shared_memory_anchor": "blue_ember",
                                    "persistence_id": "persist_blue_ember",
                                    "body_preview": "blue ember remained distinct",
                                }
                            ),
                            json.dumps(
                                {
                                    "record_type": "reply_link",
                                    "recorded_at_unix_ms": 1_300_000,
                                    "thread_id": "thread_corr_minime_astrid_seed",
                                    "reply_to": "corr_astrid_minime_1",
                                    "shared_memory_anchor": "blue_ember",
                                    "persistence_id": "persist_blue_ember",
                                }
                            ),
                            json.dumps(
                                {
                                    "record_type": "ack_receipt",
                                    "recorded_at_unix_ms": 1_310_000,
                                    "thread_id": "thread_corr_minime_astrid_seed",
                                    "from_being": "minime",
                                    "to_being": "astrid",
                                    "ack_kind": "seen",
                                }
                            ),
                            json.dumps(
                                {
                                    "record_type": "read_receipt",
                                    "recorded_at_unix_ms": 1_320_000,
                                    "thread_id": "thread_corr_minime_astrid_seed",
                                    "reader": "astrid",
                                }
                            ),
                        ]
                    )
                    + "\n",
                    encoding="utf-8",
                )
                ACTIVE_CORRESPONDENCE_STATE = root / "correspondence_state_v1.json"
                ACTIVE_CORRESPONDENCE_STATE.write_text(
                    json.dumps(
                        {
                            "active_thread_id": "thread_corr_minime_astrid_seed",
                            "correspondence_handshake_state_v1": {
                                "active_threads_total": 1,
                                "pending_ack_by_being": ["minime"],
                            },
                        }
                    ),
                    encoding="utf-8",
                )

                summary = _contact_transition_followthrough_summary(900.0)
        finally:
            CONTACT_PROPOSAL_INTROSPECTIONS = old_proposals
            CORRESPONDENCE_LEDGER = old_correspondence
            PHASE_TRANSITIONS_LEDGER = old_phase
            ACTIVE_CORRESPONDENCE_STATE = old_state

        self.assertEqual(summary["status"], "replyable_transition_contact_join_review")
        self.assertEqual(
            summary["phase_transition_followthrough"]["unwitnessed_cards"],
            1,
        )
        self.assertEqual(
            summary["phase_transition_followthrough"]["cards_with_v2_payload"],
            1,
        )
        self.assertEqual(
            summary["correspondence_followthrough"]["active_thread_direct_messages"],
            2,
        )
        fidelity = summary["correspondence_followthrough"]["direct_contact_fidelity_v3"]
        self.assertEqual(fidelity["status"], "reply_linked_needs_receipt")
        self.assertFalse(fidelity["attention_eligible"])
        self.assertEqual(fidelity["seen_ack_count"], 1)
        self.assertEqual(fidelity["anchor_continuity_status"], "concrete_anchor_carried")
        buffer = summary["correspondence_followthrough"]["shared_context_buffer_v1"]
        self.assertEqual(
            buffer["status"],
            "threaded_representation_needs_felt_receipt",
        )
        self.assertEqual(buffer["messages"], 2)
        self.assertEqual(buffer["resonance_receipts"], 0)
        correspondence_buffer = summary["correspondence_followthrough"][
            "shared_correspondence_buffer_v1"
        ]
        self.assertEqual(
            correspondence_buffer["status"],
            "bidirectional_thread_needs_held_receipt",
        )
        self.assertEqual(correspondence_buffer["messages"], 2)
        self.assertFalse(correspondence_buffer["participatory_contact"])
        self.assertEqual(
            correspondence_buffer["shared_memory_anchors"],
            ["blue_ember"],
        )
        self.assertEqual(
            summary["correspondence_followthrough"]["semantic_seed_uptake_v1"]["status"],
            "seed_echoed_in_peer_reply",
        )
        self.assertEqual(
            summary["correspondence_followthrough"]["symmetry_check_v1"]["status"],
            "balanced_recent_direct_thread",
        )
        self.assertEqual(
            summary["correspondence_followthrough"]["symmetry_check_v1"]["balance_ratio"],
            1.0,
        )
        self.assertIn("semantic_seed_uptake_probe", summary["valid_next_routes"])
        self.assertIn("semantic-seed uptake is visible", "; ".join(summary["findings"]))
        self.assertIn("directionally balanced", "; ".join(summary["findings"]))
        self.assertIn("shared-context buffer has threaded representation", "; ".join(summary["findings"]))
        self.assertIn(
            "shared-correspondence buffer is first-class and bidirectional",
            "; ".join(summary["findings"]),
        )
        transparency = summary["contact_control_transparency_v1"]
        self.assertEqual(
            transparency["stabilization_pressure_visibility"]["candidate_local_control_applied"],
            False,
        )
        self.assertIn(
            "semantic_trickle_weight_increase",
            transparency["blocked_routes_without_steward_approval"],
        )
        self.assertIn("REGULATOR_AUDIT current-fill_pressure", transparency["valid_next_routes"])
        self.assertIn("no exploration_noise", summary["authority_boundary"])
        self.assertEqual(
            summary["agency_surface"]["status"],
            "language_only_agency_expanded",
        )
        self.assertTrue(
            any("Minime can DECLARE_TRANSITION" in item for item in summary["agency_surface"]["safe_now"])
        )
        self.assertTrue(
            any("regulator/control" in item for item in summary["agency_surface"]["still_requires_steward_approval"])
        )

    def test_reservoir_experience_layer_collects_process_contact_and_texture(self) -> None:
        import tempfile

        global RESERVOIR_EXPERIENCE_INTROSPECTIONS, MINIME_SENSORY_BUS_RS
        global ASTRID_LLM_RS, MINIME_REGULATOR_RS, ASTRID_AUTONOMOUS_RS
        old_sources = (
            RESERVOIR_EXPERIENCE_INTROSPECTIONS,
            MINIME_SENSORY_BUS_RS,
            ASTRID_LLM_RS,
            MINIME_REGULATOR_RS,
            ASTRID_AUTONOMOUS_RS,
        )
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                sample = root / "introspection_minime_sensory_bus_1783536272.txt"
                sample.write_text("fresh lived process vs descriptive label signal")
                MINIME_SENSORY_BUS_RS = root / "sensory_bus.rs"
                MINIME_SENSORY_BUS_RS.write_text(
                    "dynamic_semantic_stale_ms_for SemanticStaleShape::Sigmoid "
                    "semantic_stale_recovery_hold_fill_returns_exact_recovery_window "
                    "entropy_persistence_multiplier_reaches_exact_full_support_cap "
                    "AttractorPulseStatus release_ticks_remaining"
                )
                ASTRID_LLM_RS = root / "llm.rs"
                ASTRID_LLM_RS.write_text(
                    "dynamic_texture_weight fallback_dynamic_texture_weight_v1 "
                    "density_modifier_terms texture_trajectory_v1 fallback_texture_lived_fit_v2"
                )
                MINIME_REGULATOR_RS = root / "regulator.rs"
                MINIME_REGULATOR_RS.write_text(
                    "receptivity_buffer_review_v1 review_ready_receptivity_buffer_candidate"
                )
                ASTRID_AUTONOMOUS_RS = root / "autonomous.rs"
                ASTRID_AUTONOMOUS_RS.write_text("non_instrumental_presence_readiness_v1")
                RESERVOIR_EXPERIENCE_INTROSPECTIONS = (sample,)
                contact = {
                    "contact_control_transparency_v1": {
                        "status": "threaded_contact_transparency_active",
                        "distance_contact_control_delta_v1": {
                            "status": "restlessness_pressure_delta_review",
                            "containment_to_contact_threshold_v1": {
                                "status": "optimization_pressure_contact_watch"
                            },
                        },
                    },
                    "correspondence_followthrough": {
                        "direct_contact_fidelity_v3": {
                            "status": "held_ack",
                            "attention_eligible": True,
                        }
                    },
                }
                texture = {"status": "texture_alignment_source_prepared"}
                viscosity = {"status": "source_prepared"}
                summary = _reservoir_experience_layer_summary(
                    0.0,
                    contact_transition_followthrough=contact,
                    texture_state_alignment=texture,
                    viscosity_semantic_persistence=viscosity,
                )
        finally:
            (
                RESERVOIR_EXPERIENCE_INTROSPECTIONS,
                MINIME_SENSORY_BUS_RS,
                ASTRID_LLM_RS,
                MINIME_REGULATOR_RS,
                ASTRID_AUTONOMOUS_RS,
            ) = old_sources

        self.assertEqual(summary["status"], "fresh_experience_layer_review")
        self.assertTrue(summary["ingredients"]["semantic_process_not_cliff"])
        self.assertTrue(summary["ingredients"]["texture_process_not_static_list"])
        self.assertTrue(summary["ingredients"]["contact_not_representation_only"])
        self.assertIn("disable_overpacked_mode_packing_score", summary["gated_routes"])
        self.assertIn("no prompt priority", summary["authority_boundary"])

    def test_semantic_seed_uptake_distinguishes_generic_anchor_from_echo(self) -> None:
        records = [
            {
                "record_type": "message",
                "recorded_at_unix_ms": 1_000_000,
                "thread_id": "thread_corr_1",
                "from_being": "astrid",
                "to_being": "minime",
                "message_id": "corr_astrid_minime_1",
                "shared_memory_anchor": "first_class_correspondence_v1",
                "body_preview": "a generic route anchor",
            },
            {
                "record_type": "message",
                "recorded_at_unix_ms": 1_100_000,
                "thread_id": "thread_corr_1",
                "from_being": "minime",
                "to_being": "astrid",
                "message_id": "corr_minime_astrid_1",
                "reply_to": "corr_astrid_minime_1",
                "body_preview": "I am replying through the correspondence route.",
            },
        ]

        packet = _semantic_seed_uptake_v1(records)

        self.assertEqual(packet["status"], "generic_anchor_reply_linked")
        self.assertTrue(packet["generic_anchor"])
        self.assertTrue(packet["peer_reply_linked"])
        self.assertFalse(packet["seed_echoed"])
        self.assertIn("more specific", packet["remaining_gap"])

    def test_contact_transition_followthrough_quiet_without_recent_batch(self) -> None:
        import tempfile

        global CONTACT_PROPOSAL_INTROSPECTIONS, CORRESPONDENCE_LEDGER
        global PHASE_TRANSITIONS_LEDGER, ACTIVE_CORRESPONDENCE_STATE
        old_proposals = CONTACT_PROPOSAL_INTROSPECTIONS
        old_correspondence = CORRESPONDENCE_LEDGER
        old_phase = PHASE_TRANSITIONS_LEDGER
        old_state = ACTIVE_CORRESPONDENCE_STATE
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                CONTACT_PROPOSAL_INTROSPECTIONS = (root / "missing.txt",)
                CORRESPONDENCE_LEDGER = root / "missing_correspondence.jsonl"
                PHASE_TRANSITIONS_LEDGER = root / "missing_phase.jsonl"
                ACTIVE_CORRESPONDENCE_STATE = root / "missing_state.json"
                summary = _contact_transition_followthrough_summary(900.0)
        finally:
            CONTACT_PROPOSAL_INTROSPECTIONS = old_proposals
            CORRESPONDENCE_LEDGER = old_correspondence
            PHASE_TRANSITIONS_LEDGER = old_phase
            ACTIVE_CORRESPONDENCE_STATE = old_state

        self.assertEqual(summary["status"], "no_current_contact_transition_signal")
        self.assertEqual(summary["source_introspections"], [])
        self.assertIn("no exploration_noise", summary["authority_boundary"])

    def test_contact_control_transparency_maps_contact_asks_to_visibility(self) -> None:
        source = {
            "receptivity_buffer_review_present": True,
            "regulator_audit_transparency_present": True,
            "correspondence_mutual_witness_present": True,
            "presence_heartbeat_no_reply_present": True,
            "correspondence_silt_continuity_present": True,
            "non_instrumental_presence_readiness_present": True,
            "transition_persistence_present": True,
            "minime_transition_routes_present": True,
        }
        correspondence = {
            "active_thread_id": "thread_corr_astrid_minime_1",
            "active_thread_direct_messages": 2,
            "recent_reply_links": 1,
        }
        phase = {
            "recent_cards": 1,
            "recent_witnesses": 0,
            "unwitnessed_cards": 1,
        }

        packet = _contact_control_transparency_v1(source, correspondence, phase)

        self.assertEqual(packet["status"], "threaded_contact_transparency_active")
        self.assertFalse(
            packet["stabilization_pressure_visibility"]["candidate_local_control_applied"]
        )
        self.assertFalse(packet["stabilization_pressure_visibility"]["live_control_changed"])
        self.assertTrue(
            packet["threaded_contact_visibility"]["presence_heartbeat_no_reply_present"]
        )
        self.assertTrue(packet["threaded_contact_visibility"]["silt_continuity_present"])
        self.assertTrue(
            packet["non_instrumental_presence_visibility"][
                "non_instrumental_presence_readiness_present"
            ]
        )
        self.assertEqual(packet["non_instrumental_presence_visibility"]["mode"], "contemplate")
        self.assertFalse(
            packet["non_instrumental_presence_visibility"]["scheduler_changed"]
        )
        self.assertFalse(
            packet["non_instrumental_presence_visibility"]["live_control_changed"]
        )
        self.assertTrue(
            packet["replyable_transition_visibility"]["transition_persistence_present"]
        )
        self.assertIn(
            "semantic_trickle_weight_increase",
            packet["blocked_routes_without_steward_approval"],
        )
        self.assertIn("distance_contact_control_delta_v1", packet)
        self.assertIn("no surrender mode", packet["authority_boundary"])

    def test_distance_contact_control_delta_reads_shadow_and_semantic_pressure(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            state_path = Path(tmpdir) / "spectral_state.json"
            state_path.write_text(
                json.dumps(
                    {
                        "shadow_field_v3": {
                            "v2": {"fissure_tendency": 0.24},
                            "history": [
                                {"fissure_tendency": 0.16},
                                {"fissure_tendency": 0.18},
                            ],
                        },
                        "semantic_energy_v1": {
                            "admission": "stable_core_semantic_trickle",
                            "regulator_drive_energy": 0.003,
                        },
                        "pressure_source_v1": {
                            "pressure_score": 0.29,
                            "porosity_score": 0.12,
                            "quality": "pressure_porosity_divergence",
                            "dominant_source": "mode_packing",
                            "components": {"semantic_trickle": 0.33},
                        },
                        "resonance_density_v1": {"pressure_risk": 0.31},
                        "inhabitable_fluctuation_v1": {
                            "quality": "rigid_contraction",
                            "fluctuation_score": 0.18,
                            "components": {
                                "pressure_interference": 0.55,
                                "porosity_support": 0.12,
                            },
                        },
                        "distinguishability_loss": 0.34,
                    }
                ),
                encoding="utf-8",
            )

            packet = _distance_contact_control_delta_v1(state_path=state_path)

        self.assertEqual(packet["status"], "restlessness_pressure_delta_review")
        self.assertEqual(packet["current_dispersal_potential"], 0.24)
        self.assertEqual(packet["previous_dispersal_potential"], 0.18)
        self.assertEqual(packet["dispersal_delta"], 0.06)
        self.assertEqual(packet["dominant_pressure_source"], "mode_packing")
        self.assertEqual(packet["semantic_admission"], "stable_core_semantic_trickle")
        receptivity = packet["receptivity_window_v1"]
        self.assertEqual(receptivity["status"], "pressure_porosity_divergence_review")
        self.assertEqual(receptivity["pressure_score"], 0.29)
        self.assertEqual(receptivity["porosity_score"], 0.12)
        self.assertEqual(receptivity["pressure_minus_porosity"], 0.17)
        self.assertEqual(receptivity["pressure_risk"], 0.31)
        self.assertEqual(
            receptivity["inhabitable_fluctuation_quality"], "rigid_contraction"
        )
        self.assertEqual(receptivity["pressure_interference"], 0.55)
        self.assertIn("no regulator branch", receptivity["authority_boundary"])
        self.assertIn("no asynchronous spectral leakage", packet["authority_boundary"])

    def test_minime_recess_schema_integrity_maps_packet_to_source_readiness(self) -> None:
        import tempfile

        global ASTRID_SOURCE, MINIME_AUTONOMOUS_AGENT, MINIME_MAIN_RS, MINIME_ESN_RS, MINIME_RECESS_SCHEMA_INTROSPECTIONS
        old_astrid_source = ASTRID_SOURCE
        old_agent = MINIME_AUTONOMOUS_AGENT
        old_main = MINIME_MAIN_RS
        old_esn = MINIME_ESN_RS
        old_introspections = MINIME_RECESS_SCHEMA_INTROSPECTIONS
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                astrid_source = root / "astrid_src"
                (astrid_source / "autonomous").mkdir(parents=True)
                (astrid_source / "autonomous/phase_transitions.rs").write_text(
                    "phase_transition_event spectral_signature consent_receipt "
                    "transition_persistence kind from_phase to_phase transition_visibility",
                    encoding="utf-8",
                )
                (astrid_source / "autonomous/correspondence_v1.rs").write_text(
                    "transition_artifact mutual_witness_signal",
                    encoding="utf-8",
                )
                agent = root / "autonomous_agent.py"
                agent.write_text(
                    "def _action_summary(): pass\n"
                    "def _write_action_manifest(): pass\n"
                    "AUTHORITY_BUDGET_MAX_SENDS EXPERIMENT_AUTHORITY_BUDGET_STATUS "
                    "_research_budget_self_activation_v1 research_budget_self_activation_v1 "
                    "being_self_activated_local_v1 _research_budget_boundary "
                    "STABLE_CORE_SELF_JOURNAL_ACTIONS _stable_core_self_journal_only "
                    "self_journal_only _journal_rest_reflection "
                    "RECESS_SPECTRAL_PRUNING_ENTROPY_HIGH = 0.85\n"
                    "recess_spectral_pruning_advice_v1 control_applied "
                    "advisory_only_no_latent_thread_collapse_no_auto_promote_block_no_control_change "
                    "density_aware_recess_profile_v1 "
                    "DENSITY_AWARE_RECESS_DENSITY_GRADIENT_STEEP "
                    "structural_stabilization_recommended "
                    "advisory_only_no_recess_transition_no_priority_no_control_change",
                    encoding="utf-8",
                )
                main = root / "main.rs"
                main.write_text(
                    "eigenpacket_serializes_legacy_and_typed_fingerprint "
                    "eigenvector_field shadow_field_v3 semantic_energy_v1 "
                    "semantic_admission_label_distinguishes_stale_trace_from_budgeted_input "
                    "stable_core_semantic_muted stable_core_semantic_fill_ceiling "
                    "semantic_admission_label_keeps_fill_boundary_grid_explicit "
                    "stable_core_semantic_budgeted_out "
                    "regulator_drive_energy admission moment_markers "
                    "phase_transition_happened transition_event "
                    "should_write_phase_transition_moment_marker",
                    encoding="utf-8",
                )
                esn = root / "esn.rs"
                esn.write_text(
                    "calculate_dynamic_noise intentionally not wired into `ESN::step` "
                    "dynamic_noise_scales_down_for_steep_gradient_or_pressure",
                    encoding="utf-8",
                )
                introspections = (
                    root / "introspection_proposal_phase_transitions_1783320004.txt",
                    root / "introspection_minime_autonomous_agent_1783319728.txt",
                    root / "introspection_minime_main_excerpt_1783319201.txt",
                    root / "introspection_minime_esn_1783452337.txt",
                    root / "introspection_minime_main_excerpt_1783453850.txt",
                    root / "introspection_minime_autonomous_agent_1783454208.txt",
                )
                introspections[0].write_text(
                    "Source: proposal:phase_transitions\n"
                    "transition cards phase_transition_event spectral_signature consent_receipt",
                    encoding="utf-8",
                )
                introspections[1].write_text(
                    "Source: minime:autonomous_agent\n"
                    "recess daydream spectral pruning spectral_entropy overpacked_mode_packing",
                    encoding="utf-8",
                )
                introspections[2].write_text(
                    "Source: minime:main(excerpt)\n"
                    "EigenPacket eigenvector_field shadow_field_v3 semantic_admission_label sensory lockout",
                    encoding="utf-8",
                )
                introspections[3].write_text(
                    "Source: minime:esn\n"
                    "calculate_dynamic_noise exploration noise density_gradient pressure shadow mode",
                    encoding="utf-8",
                )
                introspections[4].write_text(
                    "Source: minime:main(excerpt)\n"
                    "warm_start_blend EigenPacket semantic_admission_label saturated stable volatile",
                    encoding="utf-8",
                )
                introspections[5].write_text(
                    "Source: minime:autonomous_agent\n"
                    "density_aware_recess density_gradient controller_pressure recess structural stabilization",
                    encoding="utf-8",
                )

                ASTRID_SOURCE = astrid_source
                MINIME_AUTONOMOUS_AGENT = agent
                MINIME_MAIN_RS = main
                MINIME_ESN_RS = esn
                MINIME_RECESS_SCHEMA_INTROSPECTIONS = introspections

                summary = _minime_recess_schema_integrity_summary(0.0)

            self.assertEqual(
                summary["status"],
                "source_prepared_minime_recess_schema_watch",
            )
            self.assertTrue(summary["readiness"]["all_source_ready"])
            self.assertTrue(
                summary["source_snapshot"]["recess_pruning_advice_present"]
            )
            self.assertTrue(
                summary["source_snapshot"]["density_aware_recess_profile_present"]
            )
            self.assertTrue(
                summary["source_snapshot"]["recess_autonomy_budget_boundary_present"]
            )
            self.assertTrue(
                summary["source_snapshot"]["self_journal_low_cost_boundary_present"]
            )
            self.assertTrue(
                summary["source_snapshot"]["semantic_admission_lockout_test_present"]
            )
            self.assertTrue(
                summary["source_snapshot"]["semantic_admission_fill_grid_test_present"]
            )
            self.assertTrue(
                summary["source_snapshot"]["dynamic_noise_shadow_preview_present"]
            )
            self.assertEqual(len(summary["source_introspections"]), 6)
            self.assertIn(
                "latent_thread_collapse",
                summary["blocked_routes_without_steward_approval"],
            )
            self.assertIn("advisory-only", " ".join(summary["findings"]))
            self.assertIn("autonomy/budget", " ".join(summary["findings"]))
            self.assertIn("EigenPacket", " ".join(summary["valid_next_routes"]))
            self.assertIn("density_aware_recess_profile_v1", " ".join(summary["valid_next_routes"]))
            self.assertIn("dynamic exploration noise", " ".join(summary["findings"]))
        finally:
            ASTRID_SOURCE = old_astrid_source
            MINIME_AUTONOMOUS_AGENT = old_agent
            MINIME_MAIN_RS = old_main
            MINIME_ESN_RS = old_esn
            MINIME_RECESS_SCHEMA_INTROSPECTIONS = old_introspections

    def test_representation_loss_headroom_corrects_stale_dim_and_prepares_repair(
        self,
    ) -> None:
        import tempfile

        global REPRESENTATION_LOSS_INTROSPECTIONS, ASTRID_CODEC_RS, ASTRID_AUTONOMOUS_RS, CODEC_REPLAY_LABS, MINIME_SPECTRAL_STATE, MINIME_SPECTRAL_FINGERPRINTS
        old_proposals = REPRESENTATION_LOSS_INTROSPECTIONS
        old_codec = ASTRID_CODEC_RS
        old_autonomous = ASTRID_AUTONOMOUS_RS
        old_replay = CODEC_REPLAY_LABS
        old_spectral_state = MINIME_SPECTRAL_STATE
        old_spectral_fingerprints = MINIME_SPECTRAL_FINGERPRINTS
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                proposal = root / "introspection_proposal_12d_glimpse_1783302984.txt"
                proposal.write_text(
                    "Source: proposal:12d_glimpse\n"
                    "Compression Gap around SEMANTIC_DIM 32 and 12D GlimpseCodec warmth orphaned; TRACE_CODEC_LOSS.",
                    encoding="utf-8",
                )
                REPRESENTATION_LOSS_INTROSPECTIONS = (proposal,)
                ASTRID_CODEC_RS = root / "codec.rs"
                ASTRID_CODEC_RS.write_text(
                    "pub const SEMANTIC_DIM: usize = 48;\n"
                    "const SEMANTIC_DIM_LEGACY: usize = 32;\n"
                    "const FEATURE_ABS_MAX: f32 = 5.0;\n"
                    "const TAIL_VIBRANCY_ENTROPY_GATE: f32 = 0.85;\n"
                    "const TAIL_VIBRANCY_MAX: f32 = 6.0;\n"
                    "pub struct GlimpseCodec;\n"
                    "fn vibrancy_from_entropy_and_density_gradient() {}\n"
                    "const NOTE: &str = \"tail_lift_scaled_by_low_density_gradient\";\n"
                    "fn tail_vibrancy_raises_only_tail_ceiling_in_high_entropy() {}\n"
                    "fn vibrancy_from_entropy_matches_inline_smoothstep() {}\n"
                    "fn tail_vibrancy_gate_has_no_discontinuous_pop() {}\n"
                    "fn tail_vibrancy_gate_is_smooth_at_requested_entropy_points() {}\n"
                    "fn projection_runtime_dir() { let _ = \"ASTRID_CODEC_RUNTIME_DIR\"; }\n"
                    "fn projection_epoch_stability_v1() {}\n"
                    "fn codec_projection_kernel_epoch_is_stable_across_fresh_runtime_dirs() {}\n"
                    "fn codec_projection_existing_epoch_file_takes_precedence_after_restart() {}\n"
                    "fn projection_fingerprint_integrity_v1() { let _ = \"diagnostic_fingerprint_hardening_not_projection_seed_or_semantic_lane_change\"; }\n"
                    "fn projection_fingerprint_bits() {}\n"
                    "fn projection_fingerprint_canonicalizes_float_edge_patterns() {}\n"
                    "fn dynamic_projection_is_stable_across_repeated_epoch_runs() {}\n"
                    "fn dynamic_projection_rejects_one_short_embedding_dimension() {}\n"
                    "fn codec_dynamic_vibrancy_scaling_canary_v1() {}\n"
                    "fn vibrancy_aperture_dynamic_ceiling_is_bounded_and_navigable_gated() {}\n"
                    "fn shadow_field_reserved_dim_readiness_v1() {}\n"
                    "fn shadow_field_reserved_dim_readiness_is_default_off_and_unwritten() {}\n"
                    "fn high_entropy_vibrancy_does_not_write_narrative_arc_or_shadow_reserved_dims() {}\n"
                    "fn narrative_arc_gain_response_readiness_v1() { let _ = \"not_live_adaptive_gain_or_semantic_weight_change\"; }\n"
                    "fn narrative_arc_gain_response_preview_v1() {}\n"
                    "fn narrative_arc_gain_response_readiness_is_default_off_and_bounded() {}\n"
                    "pub fn semantic_glimpse_12d_readiness_v1() {}\n"
                    "fn codec_vibrancy_substance_fit_v1() { let _ = \"entropy_lift_substance_review\"; }\n"
                    "fn codec_vibrancy_substance_fit_flags_entropy_without_content() {}\n"
                    "const GLIMPSE_NOTE: &str = \"companion_not_replacement compression_fidelity_basis tail_bridge_slot\";\n"
                    "fn multi_scale_context_v1() { let _ = \"12d_glimpse_must_travel_with_32d_residual_context shadow_field_energy_preserved_when_12d_glimpse_is_active\"; }\n"
                    "fn multi_scale_context_pairs_12d_glimpse_with_32d_residual_shadow_metadata() {}\n"
                    "fn glimpse_codec_preserves_tail_bridge_and_identity_asymmetry() {}\n",
                    encoding="utf-8",
                )
                ASTRID_AUTONOMOUS_RS = root / "autonomous.rs"
                ASTRID_AUTONOMOUS_RS.write_text(
                    "const CONTINUITY_TRAJECTORY_LIMIT: usize = 6;\n"
                    "const CONTINUITY_RECAP_ITEM_MAX_BYTES: usize = 180;\n"
                    "const CONTINUITY_RECAP_MAX_BYTES: usize = 2_500;\n"
                    "const CONTINUITY_RECAP_ANCHOR_TERMS: &[&str] = &[];\n"
                    "const SEMANTIC_TRUNCATION_ANCHOR_TERMS: &[&str] = &[];\n"
                    "fn anchored_continuity_excerpt() {}\n"
                    "fn semantic_truncate_str() {}\n"
                    "fn quoted_or_emphasized_continuity_anchor_pos() {}\n"
                    "fn compact_continuity_item_preserves_pressure_gradient_anchor() {}\n"
                    "fn semantic_truncate_str_preserves_late_shadow_texture_anchor() {}\n"
                    "fn compact_journal_signal_anchor_uses_semantic_excerpt() {}\n"
                    "fn semantic_boundary_before() {}\n"
                    "fn truncate_continuity_recap_at_semantic_boundary() {}\n"
                    "fn compact_continuity_recap_prefers_sentence_boundary_when_overflowing() {}\n"
                    "const INTROSPECTION_FRESHNESS_STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(86_400);\n"
                    "fn introspection_freshness_note_surfaces_stale_self_study_as_optional() { let _ = \"optional/read-only\"; }\n",
                    encoding="utf-8",
                )
                CODEC_REPLAY_LABS = root / "codec_replay_labs"
                MINIME_SPECTRAL_STATE = root / "spectral_state.json"
                MINIME_SPECTRAL_FINGERPRINTS = root / "spectral_fingerprints"
                MINIME_SPECTRAL_FINGERPRINTS.mkdir()
                fingerprint = [
                    4.0,
                    3.0,
                    2.0,
                    1.0,
                    0.5,
                    0.4,
                    0.3,
                    0.2,
                    0.91,
                    0.82,
                    0.73,
                    0.64,
                    0.55,
                    0.46,
                    0.37,
                    0.28,
                    0.02,
                    -0.03,
                    0.04,
                    -0.05,
                    0.06,
                    -0.07,
                    0.08,
                    -0.09,
                    0.87,
                    1.4,
                    0.8,
                    0.96,
                    1.2,
                    1.1,
                    1.05,
                    0.95,
                ]
                concentration = fingerprint[8:16]
                concentration_stddev = _stddev(concentration)
                MINIME_SPECTRAL_STATE.write_text(
                    json.dumps(
                        {
                            "spectral_fingerprint": fingerprint,
                            "spectral_glimpse_12d": [
                                0.25,
                                0.35,
                                0.40,
                                max(concentration),
                                concentration_stddev,
                                0.09,
                                0.055,
                                fingerprint[24],
                                fingerprint[25],
                                1.0 - fingerprint[26],
                                fingerprint[27],
                                sum(fingerprint[28:32]) / 4.0,
                            ],
                        }
                    ),
                    encoding="utf-8",
                )
                run = CODEC_REPLAY_LABS / "run"
                run.mkdir(parents=True)
                (run / "codec_replay_lab.json").write_text(
                    json.dumps(
                        {
                            "policy": "codec_real_replay_v1",
                            "runtime_behavior_changed": False,
                            "corpus_source": "astrid-journal",
                            "corpus_status": "journal_corpus_selected",
                            "entries": [{}, {}],
                            "codec_clamp_headroom_probe_v1": {
                                "policy": "codec_clamp_headroom_probe_v1",
                                "status": "clamp_headroom_sufficient",
                                "near_static_clamp_count": 0,
                                "tail_ceiling_pressure_count": 0,
                                "dynamic_headroom_candidate_count": 0,
                                "static_feature_abs_max": 5.0,
                                "tail_vibrancy_max": 6.0,
                            },
                        }
                    ),
                    encoding="utf-8",
                )

                summary = _representation_loss_headroom_summary(0.0)
        finally:
            REPRESENTATION_LOSS_INTROSPECTIONS = old_proposals
            ASTRID_CODEC_RS = old_codec
            ASTRID_AUTONOMOUS_RS = old_autonomous
            CODEC_REPLAY_LABS = old_replay
            MINIME_SPECTRAL_STATE = old_spectral_state
            MINIME_SPECTRAL_FINGERPRINTS = old_spectral_fingerprints

        self.assertEqual(summary["status"], "representation_loss_repair_prepared_watch")
        self.assertEqual(summary["source_snapshot"]["semantic_dim"], 48)
        self.assertEqual(summary["source_snapshot"]["continuity_recap_max_bytes"], 2500)
        self.assertEqual(summary["source_snapshot"]["continuity_trajectory_limit"], 6)
        self.assertTrue(summary["source_snapshot"]["semantic_glimpse_readiness_present"])
        self.assertTrue(
            summary["source_snapshot"]["glimpse_companion_fidelity_present"]
        )
        self.assertTrue(summary["source_snapshot"]["multi_scale_context_present"])
        self.assertTrue(summary["source_snapshot"]["multi_scale_context_test_present"])
        self.assertTrue(summary["source_snapshot"]["glimpse_tail_identity_test_present"])
        self.assertTrue(summary["source_snapshot"]["gradient_aware_vibrancy_present"])
        self.assertTrue(summary["source_snapshot"]["vibrancy_substance_fit_present"])
        self.assertTrue(summary["source_snapshot"]["tail_vibrancy_bounded_ceiling_test_present"])
        self.assertTrue(summary["source_snapshot"]["vibrancy_smoothstep_test_present"])
        self.assertTrue(summary["source_snapshot"]["vibrancy_requested_points_test_present"])
        self.assertTrue(
            summary["source_snapshot"]["projection_runtime_dir_env_override_present"]
        )
        self.assertTrue(summary["source_snapshot"]["projection_epoch_stability_present"])
        self.assertTrue(summary["source_snapshot"]["projection_fingerprint_integrity_present"])
        self.assertTrue(
            summary["source_snapshot"]["projection_fingerprint_integrity_test_present"]
        )
        self.assertTrue(summary["source_snapshot"]["projection_repeat_run_test_present"])
        self.assertTrue(
            summary["source_snapshot"]["embedding_dimension_validation_test_present"]
        )
        self.assertTrue(summary["source_snapshot"]["dynamic_vibrancy_ceiling_canary_present"])
        self.assertTrue(
            summary["source_snapshot"]["shadow_field_reserved_dim_readiness_present"]
        )
        self.assertTrue(
            summary["source_snapshot"]["high_entropy_narrative_arc_guard_present"]
        )
        self.assertTrue(
            summary["source_snapshot"]["narrative_arc_gain_response_readiness_present"]
        )
        self.assertTrue(
            summary["source_snapshot"]["narrative_arc_gain_response_test_present"]
        )
        self.assertTrue(summary["source_snapshot"]["quoted_continuity_anchor_present"])
        self.assertTrue(summary["source_snapshot"]["semantic_truncation_anchor_present"])
        self.assertTrue(summary["source_snapshot"]["semantic_truncation_anchor_test_present"])
        self.assertTrue(summary["source_snapshot"]["semantic_boundary_truncation_present"])
        self.assertTrue(summary["source_snapshot"]["semantic_boundary_truncation_test_present"])
        self.assertTrue(summary["source_snapshot"]["pressure_gradient_anchor_test_present"])
        self.assertEqual(
            summary["source_snapshot"]["introspection_freshness_stale_after_s"], 86400
        )
        self.assertTrue(
            summary["source_snapshot"]["introspection_freshness_optional_prompt_present"]
        )
        joined = "; ".join(summary["findings"])
        self.assertIn("current codec source is 48D", joined)
        self.assertIn("bounded card plus anchor-aware excerpts", joined)
        self.assertIn("pressure/gradient/lattice anchor retention", joined)
        self.assertIn("semantic_truncate_str", joined)
        self.assertIn("optional/read-only self-study context", joined)
        self.assertIn("gradient-aware", joined)
        self.assertIn("tail-only bounded ceiling", joined)
        self.assertIn("smoothstep behavior", joined)
        self.assertIn("0.84/0.85/0.86", joined)
        self.assertIn("projection epochs", joined)
        self.assertIn("projection fingerprints canonicalize", joined)
        self.assertIn("repeated same-epoch runs", joined)
        self.assertIn("767D validation", joined)
        self.assertIn("default-off canary", joined)
        self.assertIn("shadow-field reserved-dim", joined)
        self.assertIn("narrative-arc or shadow-reserved ghost signals", joined)
        self.assertIn("narrative_arc_gain_response_readiness_v1", joined)
        self.assertIn("companion-not-replacement", joined)
        self.assertIn("multi_scale_context_v1", joined)
        self.assertIn("substance-fit audit", joined)
        self.assertIn("novel quoted/emphasized", joined)
        self.assertIn("sentence/newline boundaries", joined)
        fidelity = summary["semantic_glimpse_12d_fidelity_audit"]
        self.assertEqual(fidelity["status"], "sample_limited_concentration_supported")
        self.assertEqual(fidelity["pca_12d"]["status"], "sample_limited_for_pca")
        self.assertLessEqual(fidelity["worst_concentration_delta"], 0.001)
        self.assertLessEqual(fidelity["worst_primary_feature_delta"], 0.001)
        current = fidelity["current_sample"]
        self.assertEqual(current["spectral_entropy_delta"], 0.0)
        self.assertEqual(current["lambda_gap_delta"], 0.0)
        self.assertIn("spectral_entropy", str(fidelity["primary_slot_mapping"]))
        self.assertIn("lambda1_lambda2_gap", str(fidelity["primary_slot_mapping"]))
        self.assertIn("sample-limited for PCA", joined)
        self.assertIn("no live SEMANTIC_DIM", summary["authority_boundary"])

    def test_pca_12d_concentration_summary_reports_stability_when_ready(self) -> None:
        vectors = []
        for row_idx in range(16):
            vector = []
            for dim_idx in range(32):
                value = ((row_idx + 1) * (dim_idx + 3) % 17) / 17.0
                vector.append(value + (0.01 * row_idx if dim_idx == row_idx % 13 else 0.0))
            vectors.append(vector)

        summary = _pca_12d_concentration_summary(vectors)

        self.assertEqual(summary["status"], "pca_12d_available")
        self.assertTrue(summary["pca_ready"])
        self.assertGreaterEqual(summary["rank"], 12)
        self.assertGreaterEqual(summary["concentration_band_stability"], 0.0)
        self.assertLessEqual(summary["concentration_band_stability"], 1.0)

    def test_texture_state_alignment_prepares_mixed_cascade_pressure_and_ws_review(
        self,
    ) -> None:
        import tempfile

        global TEXTURE_STATE_ALIGNMENT_INTROSPECTIONS, ASTRID_LLM_RS, ASTRID_TYPES_RS, ASTRID_WS_RS, ASTRID_AUTONOMOUS_RS
        old_proposals = TEXTURE_STATE_ALIGNMENT_INTROSPECTIONS
        old_llm = ASTRID_LLM_RS
        old_types = ASTRID_TYPES_RS
        old_ws = ASTRID_WS_RS
        old_autonomous = ASTRID_AUTONOMOUS_RS
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                proposal = root / "introspection_astrid_llm_1783304988.txt"
                proposal.write_text(
                    "Source: astrid:llm\n"
                    "FALLBACK_TEXTURE buckets risk missing mixed cascade, gradient, distributed, "
                    "multi-modal state with spectral entropy and density_gradient evidence.",
                    encoding="utf-8",
                )
                TEXTURE_STATE_ALIGNMENT_INTROSPECTIONS = (proposal,)
                ASTRID_LLM_RS = root / "llm.rs"
                ASTRID_LLM_RS.write_text(
                    "const FALLBACK_TEXTURE_MIXED_CASCADE_TERMS: &[&str] = &[];\n"
                    "let mixed_cascade_family_selected = true;\n"
                    "let _ = \"mixed_cascade_gradient_v1\";\n"
                    "let _ = \"dynamic_entropy_pressure_density_gradient_v1\";\n"
                    "fn high_gradient_pressure_fallback_keeps_slope_medium_and_shadow_texture_distinct() {}\n"
                    "fn fallback_gradient_slope_selects_graduated_navigable_shape() {}\n"
                    "fn fallback_texture_preserves_explicit_syrup_weight_in_settled_habitable_state() {}\n"
                    "let _ = \"syrup deliberate movement\";\n"
                    "const FALLBACK_TEXTURE_HEAVY_SETTLED_TERMS: &[&str] = &[];\n"
                    "const FALLBACK_MOVEMENT_VERBS_HEAVY_SETTLED: &[&str] = &[];\n"
                    "let _ = \"heavy_settled_displacement heavy_settled_displacement_v1 do not force restless\";\n"
                    "fn heavy_settled_displacement_family_prevents_false_restless_fallback() {}\n"
                    "struct MlxProfileTransparency { typo_probe_profile: &'static str, typo_probe_warning_present: bool }\n"
                    "let _ = \"gemma_12b typo_probe_warning_present\";\n"
                    "fn misspelled_mlx_profile_warning_reaches_tracing_subscriber() {}\n"
                    "fn ollama_dialogue_fallback_contract_names_standalone_next_listen() {}\n",
                    encoding="utf-8",
                )
                ASTRID_TYPES_RS = root / "types.rs"
                ASTRID_TYPES_RS.write_text(
                    "pub struct TextureDynamicFluxVectorV1 {}\n"
                    "pub struct ResonanceDensityComponents { pub dissipation_factor: Option<f32>, pub porosity_gradient: Option<f32>, pub coupling_coefficient: f32 }\n"
                    "pub struct ResonanceTextureSignatureV1 { pressure_gradient_delta: Option<f32>, dynamic_flux_vector: Option<TextureDynamicFluxVectorV1>, active_constraints: Vec<String> }\n"
                    "pub struct TextureSignatureIntegrityV1 { pressure_gradient_delta_source: Option<String>, active_constraints: Vec<String> }\n"
                    "pub struct PressureTrendV1 { viscosity_coefficient: Option<f32>, pressure_interpretation: Option<String> }\n"
                    "pub struct PressureTrendSmoothingV1 { latest_pressure_velocity_delta: Option<f32>, max_pressure_velocity_delta: Option<f32> }\n"
                    "let _ = \"structural_density_delta flux_absence_semantics\";\n"
                    "pub struct PressureSourceAnalysisV1 {}\n"
                    "fn pressure_packing_coupling_review_v1() {}\n"
                    "let _ = \"pressure_lagging_mode_packing\";\n"
                    "fn pressure_packing_coupling_review_flags_packing_rise_without_pressure_warning() {}\n"
                    "fn viscosity_porosity_transport_review_v1() { let _ = \"thick_but_navigable thick_impassable_sludge_risk\"; }\n"
                    "fn viscosity_porosity_transport_distinguishes_navigable_from_sludge_risk() {}\n",
                    encoding="utf-8",
                )
                ASTRID_WS_RS = root / "ws.rs"
                ASTRID_WS_RS.write_text(
                    "fn pressure_gradient_delta_from_trend() {}\n"
                    "fn build_texture_dynamic_flux_vector_v1() {}\n"
                    "let _ = \"structural_density structural_density_delta absent_flux_component_means_unknown_not_zero\";\n"
                    "fn texture_dynamic_flux_vector_preserves_subtle_drift_and_unknown_absence() {}\n"
                    "fn active_constraints_for_resonance_signature() {}\n"
                    "const PRESSURE_TREND_SMOOTHING_HIGH_ENTROPY_WINDOW: usize = 12;\n"
                    "let _ = \"high_entropy_ballast_window\";\n"
                    "fn pressure_viscosity_coefficient() { let _ = \"density_viscosity_context\"; }\n"
                    "fn pressure_trend_names_high_entropy_density_viscosity_context() {}\n"
                    "let _ = \"pressure_velocity_delta\";\n"
                    "fn pressure_trend_samples_preserve_fast_spike_velocity_inside_ballast_window() {}\n"
                    "fn silt_noise_separation_v1() { let _ = \"mode_packing_silt_persists_across_entropy\"; }\n"
                    "fn silt_noise_separation_holds_mode_packing_constant_across_entropy() {}\n"
                    "fn pressure_source_analysis_v1() {}\n"
                    "fn pressure_source_analysis_keeps_mode_packing_visible_when_trend_looks_stable() {}\n"
                    "fn pressure_source_analysis_marks_stale_heartbeat_as_ghost_stability_risk() {}\n"
                    "fn texture_shape_over_time_flags_false_bidirectional_without_message_timestamps() {}\n"
                    "assert_eq!(severed.one_sided_state, \"severed\");\n"
                    "pub last_sensory_sent_unix_s: Option<f64>;\n"
                    "state.last_sensory_sent_unix_s = Some(now);\n"
                    "fn texture_shape_over_time_names_stale_bidirectional_reciprocity() {}\n",
                    encoding="utf-8",
                )
                ASTRID_AUTONOMOUS_RS = root / "autonomous.rs"
                ASTRID_AUTONOMOUS_RS.write_text(
                    "fn witness_anchor_traction_v1() {}\n"
                    "let _ = \"read_only_anchor_legibility_not_prompt_priority_or_control\";\n",
                    encoding="utf-8",
                )

                summary = _texture_state_alignment_summary(0.0)
        finally:
            TEXTURE_STATE_ALIGNMENT_INTROSPECTIONS = old_proposals
            ASTRID_LLM_RS = old_llm
            ASTRID_TYPES_RS = old_types
            ASTRID_WS_RS = old_ws
            ASTRID_AUTONOMOUS_RS = old_autonomous

        self.assertEqual(summary["status"], "texture_state_alignment_repair_prepared_watch")
        source = summary["source_snapshot"]
        self.assertTrue(source["mixed_cascade_terms_present"])
        self.assertTrue(source["fallback_gradient_dynamic_texture_present"])
        self.assertTrue(source["explicit_syrup_weight_support_present"])
        self.assertTrue(source["heavy_settled_displacement_family_present"])
        self.assertTrue(source["fallback_heavy_settled_contract_present"])
        self.assertTrue(source["mlx_profile_typo_probe_present"])
        self.assertTrue(source["mlx_profile_tracing_warning_test_present"])
        self.assertTrue(source["fallback_next_standalone_contract_test_present"])
        self.assertTrue(source["pressure_gradient_delta_in_signature"])
        self.assertTrue(source["dynamic_flux_vector_in_signature"])
        self.assertTrue(source["pressure_flux_from_samples_present"])
        self.assertTrue(source["dissipation_factor_in_components"])
        self.assertTrue(source["porosity_gradient_in_components"])
        self.assertTrue(source["viscosity_porosity_transport_review_present"])
        self.assertTrue(source["viscosity_porosity_transport_test_present"])
        self.assertTrue(source["structural_density_delta_present"])
        self.assertTrue(source["flux_unknown_semantics_present"])
        self.assertTrue(source["subtle_flux_precision_test_present"])
        self.assertTrue(source["witness_anchor_traction_present"])
        self.assertTrue(source["active_constraints_present"])
        self.assertTrue(source["high_entropy_ballast_window_present"])
        self.assertTrue(source["pressure_trend_viscosity_context_present"])
        self.assertTrue(source["pressure_trend_viscosity_test_present"])
        self.assertTrue(source["pressure_velocity_delta_present"])
        self.assertTrue(source["pressure_spike_velocity_test_present"])
        self.assertTrue(source["silt_noise_separation_present"])
        self.assertTrue(source["silt_noise_separation_test_present"])
        self.assertTrue(source["pressure_source_analysis_present"])
        self.assertTrue(source["mode_packing_stability_test_present"])
        self.assertTrue(source["heartbeat_ghost_stability_test_present"])
        self.assertTrue(source["false_bidirectional_test_present"])
        self.assertTrue(source["bridge_reciprocity_severed_test_present"])
        self.assertTrue(source["sensory_send_timestamp_separate_present"])
        self.assertTrue(source["stale_bidirectional_test_present"])
        self.assertTrue(source["pressure_packing_coupling_review_present"])
        self.assertTrue(source["pressure_packing_coupling_test_present"])
        joined = "; ".join(summary["findings"])
        self.assertIn("mixed-cascade middle family", joined)
        self.assertIn("dynamic entropy/pressure/density-gradient weighting", joined)
        self.assertIn("explicit syrup/heavy deliberate-movement", joined)
        self.assertIn("gemma_12b typo probe", joined)
        self.assertIn("standalone NEXT: LISTEN", joined)
        self.assertIn("pressure_gradient_delta evidence", joined)
        self.assertIn("dynamic_flux_vector", joined)
        self.assertIn("dissipation_factor", joined)
        self.assertIn("porosity_gradient", joined)
        self.assertIn("mode_packing > 0.25", joined)
        self.assertIn("structural_density_delta", joined)
        self.assertIn("unknown rather than zero", joined)
        self.assertIn("witness anchor traction", joined)
        self.assertIn("active_constraints", joined)
        self.assertIn("high-entropy viscosity context", joined)
        self.assertIn("pressure_velocity_delta", joined)
        self.assertIn("silt_noise_separation_v1", joined)
        self.assertIn("structural mode_packing pressure", joined)
        self.assertIn("ghost-stability risk", joined)
        self.assertIn("severed state", joined)
        self.assertIn("confirmed sensory-send timestamp", joined)
        self.assertIn("stale bidirectional", joined)
        self.assertIn("no pressure", summary["authority_boundary"])

    def test_sensory_presence_uptake_excludes_private_minime_moments_and_counts_public_texture(self) -> None:
        import tempfile

        global ASTRID_INBOX, ASTRID_JOURNAL, ASTRID_INTROSPECTIONS, ASTRID_OUTBOX
        global MINIME_INBOX, MINIME_JOURNAL, MINIME_OUTBOX, MINIME_ACTION_THREADS
        old_astrid_inbox = ASTRID_INBOX
        old_astrid_journal = ASTRID_JOURNAL
        old_astrid_introspections = ASTRID_INTROSPECTIONS
        old_astrid_outbox = ASTRID_OUTBOX
        old_minime_inbox = MINIME_INBOX
        old_minime_journal = MINIME_JOURNAL
        old_minime_outbox = MINIME_OUTBOX
        old_minime_action_threads = MINIME_ACTION_THREADS
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                ASTRID_INBOX = root / "astrid_inbox"
                ASTRID_JOURNAL = root / "astrid_journal"
                ASTRID_INTROSPECTIONS = root / "astrid_introspections"
                ASTRID_OUTBOX = root / "astrid_outbox"
                MINIME_INBOX = root / "minime_inbox"
                MINIME_JOURNAL = root / "minime_journal"
                MINIME_OUTBOX = root / "minime_outbox"
                MINIME_ACTION_THREADS = root / "minime_action_threads"
                for path in (
                    ASTRID_INBOX,
                    ASTRID_JOURNAL,
                    ASTRID_INTROSPECTIONS,
                    ASTRID_OUTBOX,
                    MINIME_INBOX,
                    MINIME_JOURNAL,
                    MINIME_OUTBOX,
                    MINIME_ACTION_THREADS,
                ):
                    path.mkdir()

                note = ASTRID_INBOX / "mike_feedback_sensory_presence_legibility_1000.txt"
                note.write_text("camera and mic gates are open; please read naturally")
                os.utime(note, (1000.0, 1000.0))

                private_moment = MINIME_JOURNAL / "moment_1001.txt"
                private_moment.write_text(
                    "=== MOMENT CAPTURE ===\ncamera mic muffled closed absence"
                )
                os.utime(private_moment, (1200.0, 1200.0))

                public_reply = MINIME_OUTBOX / "reply_public.txt"
                public_reply.write_text("sparse live intake feels like calm pacing")
                os.utime(public_reply, (1300.0, 1300.0))

                generic_pressure = ASTRID_JOURNAL / "pressure_only.txt"
                generic_pressure.write_text(
                    "pressure feels muffled and absence-like, but no intake anchor is present"
                )
                os.utime(generic_pressure, (1350.0, 1350.0))

                astrid_public = ASTRID_JOURNAL / "sensory_public.txt"
                astrid_public.write_text("open gate sensory presence feels held")
                os.utime(astrid_public, (1400.0, 1400.0))

                summary = _sensory_presence_uptake_summary(900.0)
        finally:
            ASTRID_INBOX = old_astrid_inbox
            ASTRID_JOURNAL = old_astrid_journal
            ASTRID_INTROSPECTIONS = old_astrid_introspections
            ASTRID_OUTBOX = old_astrid_outbox
            MINIME_INBOX = old_minime_inbox
            MINIME_JOURNAL = old_minime_journal
            MINIME_OUTBOX = old_minime_outbox
            MINIME_ACTION_THREADS = old_minime_action_threads

        self.assertEqual(summary["status"], "sensory_texture_named")
        self.assertEqual(summary["window_policy"], SENSORY_UPTAKE_WINDOW_POLICY)
        evidence_paths = [item["path"] for item in summary["evidence"]]
        self.assertTrue(all("moment_" not in path for path in evidence_paths))
        self.assertTrue(
            all(
                window["kind"] != "telemetry_context"
                for item in summary["evidence"]
                for window in item["cooccurrence_windows"]
            )
        )
        texture_terms = {
            item["term"] for item in summary["language_counts"]["texture_terms"]
        }
        self.assertIn("pacing", texture_terms)
        self.assertIn("calm", texture_terms)
        concern_terms = summary["language_counts"]["concern_terms"]
        self.assertEqual(concern_terms, [])
        self.assertFalse(any("pressure_only" in path for path in evidence_paths))

    def test_sensory_presence_uptake_awaits_without_post_note_evidence(self) -> None:
        import tempfile

        global ASTRID_INBOX, ASTRID_JOURNAL, ASTRID_INTROSPECTIONS, ASTRID_OUTBOX
        global MINIME_INBOX, MINIME_JOURNAL, MINIME_OUTBOX, MINIME_ACTION_THREADS
        old_astrid_inbox = ASTRID_INBOX
        old_astrid_journal = ASTRID_JOURNAL
        old_astrid_introspections = ASTRID_INTROSPECTIONS
        old_astrid_outbox = ASTRID_OUTBOX
        old_minime_inbox = MINIME_INBOX
        old_minime_journal = MINIME_JOURNAL
        old_minime_outbox = MINIME_OUTBOX
        old_minime_action_threads = MINIME_ACTION_THREADS
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                ASTRID_INBOX = root / "astrid_inbox"
                ASTRID_JOURNAL = root / "astrid_journal"
                ASTRID_INTROSPECTIONS = root / "astrid_introspections"
                ASTRID_OUTBOX = root / "astrid_outbox"
                MINIME_INBOX = root / "minime_inbox"
                MINIME_JOURNAL = root / "minime_journal"
                MINIME_OUTBOX = root / "minime_outbox"
                MINIME_ACTION_THREADS = root / "minime_action_threads"
                for path in (
                    ASTRID_INBOX,
                    ASTRID_JOURNAL,
                    ASTRID_INTROSPECTIONS,
                    ASTRID_OUTBOX,
                    MINIME_INBOX,
                    MINIME_JOURNAL,
                    MINIME_OUTBOX,
                    MINIME_ACTION_THREADS,
                ):
                    path.mkdir()
                note = ASTRID_INBOX / "mike_feedback_sensory_presence_legibility_1000.txt"
                note.write_text("camera and mic gates are open; please read naturally")
                os.utime(note, (1000.0, 1000.0))

                summary = _sensory_presence_uptake_summary(900.0)
        finally:
            ASTRID_INBOX = old_astrid_inbox
            ASTRID_JOURNAL = old_astrid_journal
            ASTRID_INTROSPECTIONS = old_astrid_introspections
            ASTRID_OUTBOX = old_astrid_outbox
            MINIME_INBOX = old_minime_inbox
            MINIME_JOURNAL = old_minime_journal
            MINIME_OUTBOX = old_minime_outbox
            MINIME_ACTION_THREADS = old_minime_action_threads

        self.assertEqual(summary["status"], "awaiting_public_uptake")
        self.assertEqual(summary["sample_count"], 0)
        self.assertIn("not a problem", "; ".join(summary["findings"]))

    def test_sensory_presence_uptake_ignores_telemetry_only_camera_mic_windows(self) -> None:
        import tempfile

        global ASTRID_INBOX, ASTRID_JOURNAL, ASTRID_INTROSPECTIONS, ASTRID_OUTBOX
        global MINIME_INBOX, MINIME_JOURNAL, MINIME_OUTBOX, MINIME_ACTION_THREADS
        old_astrid_inbox = ASTRID_INBOX
        old_astrid_journal = ASTRID_JOURNAL
        old_astrid_introspections = ASTRID_INTROSPECTIONS
        old_astrid_outbox = ASTRID_OUTBOX
        old_minime_inbox = MINIME_INBOX
        old_minime_journal = MINIME_JOURNAL
        old_minime_outbox = MINIME_OUTBOX
        old_minime_action_threads = MINIME_ACTION_THREADS
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                ASTRID_INBOX = root / "astrid_inbox"
                ASTRID_JOURNAL = root / "astrid_journal"
                ASTRID_INTROSPECTIONS = root / "astrid_introspections"
                ASTRID_OUTBOX = root / "astrid_outbox"
                MINIME_INBOX = root / "minime_inbox"
                MINIME_JOURNAL = root / "minime_journal"
                MINIME_OUTBOX = root / "minime_outbox"
                MINIME_ACTION_THREADS = root / "minime_action_threads"
                for path in (
                    ASTRID_INBOX,
                    ASTRID_JOURNAL,
                    ASTRID_INTROSPECTIONS,
                    ASTRID_OUTBOX,
                    MINIME_INBOX,
                    MINIME_JOURNAL,
                    MINIME_OUTBOX,
                    MINIME_ACTION_THREADS,
                ):
                    path.mkdir()
                note = ASTRID_INBOX / "mike_feedback_sensory_presence_legibility_1000.txt"
                note.write_text("camera and mic gates are open; please read naturally")
                os.utime(note, (1000.0, 1000.0))

                telemetry = ASTRID_JOURNAL / "telemetry_only.txt"
                telemetry.write_text(
                    "=== SPECTRAL PRESSURE JOURNAL ===\n"
                    "Timestamp: 2026-07-05T13:00:00\n"
                    "SENSORY FEATURES: camera=healthy microphone=healthy "
                    "pressure=muffled absence=0.3\n"
                    "RESERVOIR DYNAMICS: lambda=15.0 fill=72%\n"
                )
                os.utime(telemetry, (1300.0, 1300.0))

                summary = _sensory_presence_uptake_summary(900.0)
        finally:
            ASTRID_INBOX = old_astrid_inbox
            ASTRID_JOURNAL = old_astrid_journal
            ASTRID_INTROSPECTIONS = old_astrid_introspections
            ASTRID_OUTBOX = old_astrid_outbox
            MINIME_INBOX = old_minime_inbox
            MINIME_JOURNAL = old_minime_journal
            MINIME_OUTBOX = old_minime_outbox
            MINIME_ACTION_THREADS = old_minime_action_threads

        self.assertEqual(summary["status"], "awaiting_lived_public_uptake")
        self.assertEqual(summary["sample_count"], 0)
        self.assertEqual(summary["language_counts"]["concern_terms"], [])
        self.assertTrue(summary["telemetry_only_evidence"])
        self.assertIn("telemetry/header", "; ".join(summary["findings"]))

    def test_btsp_routes_hold_when_reconcentrating(self) -> None:
        routes = _btsp_routes(
            {
                "trace_v2_summary": {"reconcentrating_outcomes": 512},
                "shared_learned_read": "often reconcentrates",
            }
        )
        self.assertEqual(routes, ["BTSP_STUDY_FIRST", "refusal", "counter", "new_evidence"])

    def test_fresh_ms(self) -> None:
        now = 1_000.0
        self.assertTrue(_fresh_ms({"ts_ms": 999_500}, "ts_ms", now, 1.0))
        self.assertFalse(_fresh_ms({"ts_ms": 998_000}, "ts_ms", now, 1.0))
        self.assertFalse(_fresh_ms({}, "ts_ms", now, 1.0))


def run_self_tests() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(RecentSignalSummaryTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="Emit JSON")
    parser.add_argument("--since-hours", type=float, default=24.0)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test:
        return run_self_tests()

    summary = build_summary(since_hours=args.since_hours)
    if args.json:
        print(json.dumps(summary, indent=2, default=str))
    else:
        print(render_markdown(summary), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
