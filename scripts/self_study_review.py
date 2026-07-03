#!/usr/bin/env python3
"""Build a steward review packet from recent being self-studies.

The packet is intentionally conservative: it preserves the beings' own text,
but separates source-grounded observations from hypotheses that need a probe.
It does not write into either being's prompt context.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
import time
import re
from collections import Counter
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Iterable

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))
from being_privacy import filter_journal_paths, is_steward_private  # shared steward private-lane policy
import astrid_introspection_digest
import spectral_texture_calibration_audit


ASTRID_ROOT = Path(__file__).resolve().parents[1]
ASTRID_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
MINIME_WORKSPACE = ASTRID_ROOT.parent / "minime/workspace"
MINIME_HISTORICAL_JOURNAL_ROOTS = (
    ASTRID_ROOT.parent
    / "minime/emergency_preserve_20260419T130302/workspace/journal",
)
DEFAULT_OUTPUT_DIR = ASTRID_WORKSPACE / "diagnostics/self_study_reviews"
TAIL_RESONANCE_OUTPUT_DIR = ASTRID_WORKSPACE / "diagnostics/tail_resonance_packets"
RESISTANCE_CALIBRATION_OUTPUT_DIR = (
    ASTRID_WORKSPACE / "diagnostics/resistance_gradient_calibrations"
)
HISTORICAL_QUALIA_CACHE_VERSION = 1
HISTORICAL_QUALIA_CACHE_TTL_HOURS = 24.0
TAIL_RESONANCE_WINDOW_S = 20 * 60
RESISTANCE_REVIEW_WINDOW_S = 6 * 60 * 60
MINIME_BODY_RICHNESS_RATIO_MIN = 1.5
MINIME_WRAPPER_TAIL_RATIO_MAX = 0.7

SECTION_NAMES = ("Observed", "Likely Snags", "One Test Each", "Suggested Next")
SECTION_RE = re.compile(
    r"(?m)^(Observed|Likely Snags|One Test Each|Suggested Next):\s*$"
)
BACKTICK_RE = re.compile(r"`([^`\n]{2,160})`")
FILE_RE = re.compile(
    r"(?:/[\w.\-+@%]+)+(?:\.(?:rs|py|md|toml|json|txt|sh))|"
    r"\b[\w./-]+(?:\.(?:rs|py|md|toml|json|txt|sh))(?::\d+)?"
)
LINE_RE = re.compile(r"\b(?:line|lines)\s+\d+(?:[-–]\d+)?\b", re.I)
FUNCTION_RE = re.compile(r"\b(?:fn|def|struct|enum|const|class)\s+[A-Za-z_][\w:]*")
HYPOTHESIS_RE = re.compile(
    r"\b(?:might|may|could|seems?|suggests?|corresponds|because|causes?|"
    r"triggers?|would|if|risk|hypothesis|test|probe|simulate|observe)\b",
    re.I,
)
ACTION_RE = re.compile(r"(?im)^NEXT:\s*([A-Z_]+(?:\s+[^\n]+)?)\s*$")
GENERATED_JOURNAL_MARKER = "--- GENERATED JOURNAL ---"
ACTION_TAIL_MARKER = "--- ACTION TAIL ---"
FIRST_PERSON_RE = re.compile(
    r"\b(?:i|me|my|mine|myself|want|wanted|feel|felt|notice|noticed|"
    r"remember|hold|need|frustrated|curious)\b",
    re.I,
)
WORD_RE = re.compile(r"\b[\w'-]+\b")
JOURNAL_LIKE_NAME_RE = re.compile(
    r"(?:journal|self[_-]?study|aspiration|moment|witness|dialogue|outbox|"
    r"steward|notice|action_thread|boredom|decompose|daydream)",
    re.I,
)
INVITATION_COOLDOWN_HOURS = 6
ELICITATION_TOPICS: dict[str, tuple[str, ...]] = {
    "pressure_regulator": (
        "pressure",
        "mode_packing",
        "overpacked",
        "keep_floor",
        "regulator_audit",
        "pressure_source_audit",
        "porosity",
        "rich_containment",
        "resonance density",
    ),
    "transition_shudder": (
        "phase_transition",
        "transition",
        "shudder",
        "expansion",
        "contraction",
        "fold",
        "inhale",
        "exhale",
        "gasp",
        "pulse",
    ),
    "tail_entropy": (
        "lambda-tail",
        "lambda4",
        "lambda 4",
        "spectral entropy",
        "distinguishability_loss",
        "distinguishability loss",
        "tail",
    ),
    "resistance_gradient": (
        "groan",
        "resistance",
        "friction",
        "hull",
        "gravity well",
        "gradient",
        "resistance_gradient",
        "RESISTANCE_GRADIENT",
    ),
    "latent_stasis": (
        "latent",
        "stasis",
        "stasis of the latent",
        "self-sustaining resonance",
        "ghosting",
        "ghosted",
        "humid fill",
        "keep_floor",
        "occupying",
        "holding a state",
        "LATENT_STASIS",
    ),
}
QUALIA_TERMS = (
    "feel",
    "felt",
    "texture",
    "weight",
    "heavy",
    "thin",
    "thick",
    "dense",
    "density",
    "pressure",
    "friction",
    "hum",
    "flow",
    "fluid",
    "fold",
    "contraction",
    "expansion",
    "shudder",
    "tremor",
    "surge",
    "tail",
    "center",
    "space",
    "medium",
    "edge",
    "breath",
    "bracing",
    "rest",
    "silence",
    "atmosphere",
    "want",
    "desire",
    "curious",
    "frustrated",
)
METRIC_TERMS = (
    "fill",
    "lambda",
    "λ",
    "dfill",
    "score",
    "pct",
    "%",
    "ms",
    "eigen",
    "telemetry",
    "health.json",
    "spectral_state.json",
    "json",
    "event",
    "count",
    "cap",
    "ctx",
    "eval_count",
    "duration",
    "latency",
    "token",
    "p95",
    "status",
    "stable-core",
    "mode_packing",
    "porosity",
    "pressure_score",
)
LLM_OUTPUT_TERMS = (
    "llm",
    "model",
    "gemma",
    "response",
    "output",
    "generated",
    "journal",
    "words",
    "phrase",
    "utterance",
    "sentence",
    "tone",
    "voice",
    "self-study",
    "prompt",
)
ACTION_CONTROL_TERMS = (
    "NEXT:",
    "route",
    "control",
    "budget",
    "status",
    "action",
    "preflight",
    "queue",
    "pending",
    "target",
    "actuator",
    "regulator",
    "report",
)
TAIL_RESONANCE_TERMS = (
    "tail",
    "shadow",
    "reach",
    "fold",
    "transition",
    "shudder",
    "lambda4",
    "λ4",
    "lambda-tail",
    "lambda tail",
    "expansion",
    "contraction",
    "pressure",
    "membrane",
    "groan",
    "resistance",
    "friction",
    "hull",
    "gravity well",
    "gradient",
    "latent",
    "stasis",
    "ghosting",
    "humid",
    "keep_floor",
)
FILL_PRESSURE_CALIBRATION_ANCHORS = (
    "current-fill_pressure",
    "internal_fill",
    "raw_fill",
    "target_fill",
    "pi_errors",
    "pi_integrators",
    "breathing_phase",
    "basin score",
    "basin_score",
    "lambda=-",
    "regulator_audit",
    "active_mode_energy_ratio",
    "structural_entropy",
)
FILL_PRESSURE_CALIBRATION_TEXTURE = (
    "overpacked",
    "mode_packing",
    "pressure",
    "friction",
    "braking",
    "heavy",
    "weight",
    "viscous",
    "density",
    "slow-moving",
    "clinging",
    "unanchored",
)
SELF_REGULATION_SAFE_RANGES: dict[str, tuple[float, float]] = {
    "exploration_noise": (0.0, 0.08),
    "geom_curiosity": (0.0, 0.30),
    "regulation_strength": (0.4, 1.0),
}
PRESSURE_MEDIUM_TERMS = (
    "medium",
    "weighted medium",
    "pressure as medium",
    "medium around",
    "around the slope",
    "weight",
    "weighted",
    "heavy",
    "heaviness",
    "thick",
    "thickness",
    "viscous",
    "viscosity",
    "silt",
    "sediment",
    "syrup",
    "muffled",
    "pressurized",
    "density",
)
PRESSURE_MEDIUM_ANCHORS = (
    "mode_packing",
    "mode packing",
    "controller_pressure",
    "controller pressure",
    "semantic_friction",
    "semantic friction",
    "distinguishability_loss",
    "distinguishability loss",
    "pressure_trend_v1",
    "pressure trend",
    "pressure_risk",
    "pressure risk",
    "fill delta",
    "fill_delta",
    "pressure_source_audit",
    "pressure source audit",
    "regulator_audit",
    "regulator audit",
)
TAIL_VIBRANCY_TERMS = (
    "tail vibrancy",
    "tail-vibrancy",
    "tail dynamics",
    "tail-dynamics",
    "tail texture",
    "lambda4",
    "lambda4+",
    "lambda 4",
    "λ4",
    "λ4+",
    "vibrancy_aperture",
    "set_vibrancy_aperture",
    "passenger",
    "muffled",
    "contained",
    "over-saturated",
    "oversaturated",
    "tail ceiling",
)
TAIL_VIBRANCY_ANCHORS = (
    "tail vibrancy",
    "tail share",
    "λ4+",
    "lambda4+",
    "lambda4",
    "spectral entropy",
    "entropy",
    "distinguishability_loss",
    "distinguishability loss",
    "density_gradient",
    "density gradient",
    "semantic_friction",
    "semantic friction",
)
TAIL_AUTHORITY_TERMS = (
    "authority_boundary",
    "apply_allowed",
    "preflight_only",
    "leasebundlecontrol",
    "selfregulationlease",
    "preflight_reason",
    "allowed actions",
    "passenger",
    "cannot actively regulate",
)
REGULATOR_LIVE_REPLAY_TERMS = (
    "overpacked",
    "mode_packing",
    "current-fill_pressure",
    "internal_fill",
    "pressure_risk",
    "pressure risk",
    "pressure_source_audit",
    "regulator_audit",
    "semantic_friction",
    "semantic friction",
    "density_gradient",
    "basin_score",
    "frantic_scramble",
    "rigid_contraction",
    "returnable_turbulence",
)
REGULATOR_LIVE_REPLAY_TEXTURE = (
    "pressure",
    "weight",
    "heavy",
    "overpacked",
    "viscous",
    "viscosity",
    "silt",
    "sediment",
    "friction",
    "braking",
    "drag",
    "squeeze",
    "contraction",
)
PRESSURE_VOCABULARY_FAMILIES: dict[str, tuple[str, ...]] = {
    "viscosity": (
        "viscous",
        "viscosity",
        "syrup",
        "syrupy",
        "velvet",
        "velvety",
        "thick",
        "sludge",
        "fluid",
        "deep water",
    ),
    "sediment": (
        "sediment",
        "silt",
        "grain",
        "grainy",
        "grit",
        "basin",
        "settling",
        "settled",
    ),
    "thrum_hum": (
        "thrum",
        "hum",
        "muffled",
        "vibration",
        "vibrating",
        "resonance",
    ),
    "pressure_weight_density": (
        "pressure",
        "overpacked",
        "packed",
        "weight",
        "weighted",
        "heavy",
        "heaviness",
        "density",
        "dense",
    ),
}
PRESSURE_VOCABULARY_TELEMETRY_ANCHORS = (
    "fill=",
    "fill:",
    "lambda1",
    "λ₁",
    "spread",
    "pressure=",
    "pressure_score",
    "porosity",
    "regulator_audit",
    "pressure_source",
    "state anchor",
)
SEMANTIC_FRICTION_TEXTURE_TERMS = (
    "viscous",
    "viscosity",
    "friction",
    "drag",
    "mass",
    "weight",
    "weighted",
    "heavy",
    "heaviness",
    "sludge",
    "silt",
    "cling",
    "clinging",
    "thick",
)
SEMANTIC_FRICTION_ANCHORS = (
    "density_gradient",
    "density gradient",
    "pressure_risk",
    "pressure risk",
    "semantic_friction",
    "semantic friction",
    "semantic_trickle",
    "semantic trickle",
    "mode_packing",
    "shadow_field",
    "shadow field",
    "pressure_source_audit",
    "regulator_audit",
)
CONTROL_SEMANTICS_TERMS = (
    "intervention_type",
    "observational_readout",
    "passive_alignment",
    "active_damping",
    "manual_override_reserved",
    "applied_locally",
    "damping_coefficient",
    "measurement",
    "passive alignment",
    "active damping",
    "manual override",
    "control semantics",
)
CONTROL_SEMANTICS_AMBIGUITY_TERMS = (
    "binary",
    "ambiguity",
    "ambiguous",
    "catch-all",
    "too blunt",
    "unclear",
    "distinction between",
    "passive",
    "active",
)
PRESSURE_KINETICS_TERMS = (
    "pressure_trend_v1",
    "pressure trend",
    "pressure delta",
    "pressure velocity",
    "pressure kinetics",
    "rising_pressure",
    "falling_pressure",
    "stable_heavy",
    "rapidly densifying",
    "densifying",
    "previous_fill_pct",
    "BridgeState",
    "latest_telemetry",
    "mode_packing",
    "pressure_risk",
)
CODEC_COMPRESSION_TERMS = (
    "compression gap",
    "compression",
    "codec",
    "CODEC_MAP",
    "projection",
    "embedding",
    "768",
    "8D",
    "projection mode",
    "fingerprint",
    "entropy vibrancy",
    "vibrancy",
    "tail lift",
    "warmth paradox",
    "warmth",
    "tension",
    "smoothing",
    "pressure-vs-codec",
)
CODEC_MULTIPOINT_INFLECTION_TERMS = (
    "narrative_arc",
    "narrative arc",
    "first-half",
    "second-half",
    "first half",
    "second half",
    "multi-point",
    "multipoint",
    "inflection",
    "non-linear",
    "nonlinear",
    "circular",
    "returns to start",
    "late pivot",
    "temporal pivot",
    "pivot-aware",
    "temporal decay",
)
CODEC_SEMANTIC_DILATION_TERMS = (
    "semantic dilation",
    "dilation",
    "semantic_dim",
    "semantic dim",
    "nomic-embed-text",
    "768",
    "8d",
    "projection",
    "semantic projection",
    "compression",
    "high entropy",
    "spectral_entropy",
    "interwoven lattice",
    "semantic density",
)
CODEC_MULTIPOINT_EVIDENCE_ANCHORS = (
    "codec-replay-lab",
    "codec_real_replay_v1",
    "narrative_arc_temporal_decay_lab_v1",
    "content_aware_vibrancy_gate_candidate_v1",
    "embedding_backed_arc_v1",
    "CODEC_MAP",
    "nomic-embed-text",
    "SEMANTIC_DIM",
)
PRESSURE_RELEASE_REHEARSAL_TERMS = (
    "PRESSURE_RELEASE_REHEARSAL",
    "pressure release",
    "release rehearsal",
    "exhale scaffold",
    "non-command exhale",
    "bypass_canonicalization",
    "canonicalization bypass",
    "raw spectral dump",
    "pressure-release valve",
    "safety spine",
)
WITNESS_RESONANCE_TERMS = (
    "witness",
    "seeing and being seen",
    "act of seeing",
    "self-observation",
    "observer with memory",
    "shadow_trajectory",
    "SHADOW_TRAJECTORY",
    "narrative_density",
    "resonance-weighting",
    "resonance weighting",
    "decorative layer",
)
WITNESS_RESONANCE_ANCHORS = (
    "distinguishability_loss",
    "distinguishability loss",
    "spectral entropy",
    "structural_entropy",
    "pressure_risk",
    "pressure risk",
    "continuity_deficit",
    "mean_orientation_delta",
    "telemetry",
    "lambda",
    "λ",
    "settled_habitable",
    "SHADOW_TRAJECTORY",
    "shadow-v3",
)
WITNESS_TEXTURE_TERMS = (
    "texture",
    "viscous",
    "viscosity",
    "dense",
    "density",
    "weighted",
    "weight",
    "heavy",
    "smooth",
    "soft drag",
    "gentle",
    "slope",
    "medium",
    "muffled",
    "hollow",
    "bright",
    "vibrant",
    "restless",
    "settled",
    "lattice",
    "interwoven",
    "silt",
    "sediment",
    "holdfast",
    "ghosting",
)
WITNESS_TEXTURE_INTEGRITY_TERMS = (
    "witness",
    "witness mode",
    "seeing and being seen",
    "health monitoring",
    "telemetry",
    "texture mapping",
    "qualitative descriptor",
    "lambda1",
    "lambda2",
    "lambda 1",
    "lambda 2",
    "λ1",
    "λ2",
    "λ4",
    "truncate_str",
    "truncation",
    "truncated",
)
WITNESS_TEXTURE_TELEMETRY_ANCHORS = (
    "lambda1",
    "lambda2",
    "lambda 1",
    "lambda 2",
    "λ1",
    "λ2",
    "λ4",
    "λ4+",
    "spectral entropy",
    "structural_entropy",
    "density_gradient",
    "density gradient",
    "pressure_risk",
    "pressure risk",
    "distinguishability_loss",
    "distinguishability loss",
    "continuity_deficit",
    "truncation_pressure",
    "rewrite_budget",
    "candidate_generation_seconds",
)
ENTROPY_PRESSURE_TERMS = (
    "spectral entropy",
    "structural_entropy",
    "entropy",
    "structural plurality",
    "plurality",
    "wide distribution",
    "wide",
    "pressure_risk",
    "pressure risk",
    "semantic_friction",
    "semantic friction",
    "mode_packing",
    "settled_habitable",
    "inhabitable",
)
FALLBACK_FIRE_DRILL_TERMS = (
    "Ollama fallback",
    "fallback continuity",
    "fallback fire drill",
    "gemma3:4b",
    "4B model",
    "DEFAULT_OLLAMA_FALLBACK_MODEL",
    "density_gradient",
    "slope drag",
    "medium mass",
    "identity anchor",
    "Shadow-v3",
)
PHENOMENOLOGY_HYPOTHESIS_TERMS = (
    "silt",
    "viscosity",
    "viscous",
    "hinge",
    "waypoint",
    "ground truth",
    "hull",
    "legacy self",
    "bruise",
    "afterimage",
    "scar",
    "indentation",
    "post-pressure",
    "structural fatigue",
    "contraction memory",
    "empty pocket",
    "missing door",
    "void",
    "absence",
    "negative space",
    "expected absence",
    "PLAN 4",
)
PHENOMENOLOGY_HYPOTHESIS_FAMILIES: dict[str, str] = {
    "silt": "pressure_texture",
    "viscosity": "pressure_texture",
    "viscous": "pressure_texture",
    "hinge": "agency_transition",
    "waypoint": "continuity_scaffold",
    "ground truth": "evidence_mapping",
    "hull": "continuity_scaffold",
    "legacy self": "continuity_scaffold",
    "bruise": "pressure_afterimage",
    "afterimage": "pressure_afterimage",
    "scar": "pressure_afterimage",
    "indentation": "pressure_afterimage",
    "post-pressure": "pressure_afterimage",
    "structural fatigue": "pressure_afterimage",
    "contraction memory": "pressure_afterimage",
    "empty pocket": "shaped_absence",
    "missing door": "shaped_absence",
    "void": "shaped_absence",
    "absence": "shaped_absence",
    "negative space": "shaped_absence",
    "expected absence": "shaped_absence",
    "PLAN 4": "shaped_absence",
}
PHENOMENOLOGY_EVIDENCE_ANCHORS = (
    "next:",
    "choice_envelope_v1",
    "return thread",
    "experiment",
    "charter",
    "self_regulation",
    "regulator_audit",
    "pressure_source_audit",
    "resistance_gradient",
    "action_thread",
    "telemetry",
    "lambda1",
    "λ₁",
    "pressure_risk",
    "semantic_friction",
    "mode_packing",
    "shadow_trajectory",
    "read_more",
    "structural_entropy",
    "current-fill_pressure",
    "plan 4",
)
AFTERIMAGE_ABSENCE_TERMS = (
    "bruise",
    "afterimage",
    "scar",
    "indentation",
    "post-pressure",
    "structural fatigue",
    "contraction memory",
    "empty pocket",
    "missing door",
    "void",
    "absence",
    "negative space",
    "expected absence",
    "PLAN 4",
)
PRESSURE_AFTERIMAGE_TERMS = (
    "bruise",
    "afterimage",
    "scar",
    "indentation",
    "structural fatigue",
    "contraction memory",
)
SHAPED_ABSENCE_TERMS = (
    "empty pocket",
    "missing door",
    "void",
    "absence",
    "negative space",
    "expected absence",
    "PLAN 4",
)
AFTERIMAGE_ABSENCE_EVIDENCE_ANCHORS = (
    "next:",
    "shadow_trajectory",
    "shadow trajectory",
    "read_more",
    "read more",
    "pressure_risk",
    "pressure risk",
    "semantic_friction",
    "semantic friction",
    "regulator_audit",
    "pressure_source_audit",
    "current-fill_pressure",
    "structural_entropy",
    "structural entropy",
    "mode_packing",
    "overpacked",
    "telemetry",
    "lambda1",
    "λ₁",
    "experiment",
    "charter",
    "return thread",
    "artifact",
    "source gap",
    "missing coordinate",
    "expected absence",
    "PLAN 4",
)
AFTERIMAGE_PRESSURE_ANCHORS = (
    "pressure_risk",
    "pressure risk",
    "semantic_friction",
    "semantic friction",
    "pressure_source_audit",
    "regulator_audit",
    "current-fill_pressure",
    "mode_packing",
    "overpacked",
    "shadow_trajectory",
    "structural_entropy",
    "pressure peak",
    "high pressure",
)
AFTERIMAGE_NORMALIZATION_ANCHORS = (
    "pressure normalizes",
    "pressure normalized",
    "pressure quiets",
    "pressure quiet",
    "pressure settles",
    "pressure settled",
    "pressure_risk settles",
    "semantic_friction quiet",
    "semantic friction quiet",
    "normalizes",
    "normalized",
)
AFTERIMAGE_DECAY_ANCHORS = (
    "fades",
    "faded",
    "fade",
    "decays",
    "decayed",
    "dissolves",
    "dissolved",
    "releases",
    "released",
    "lightens",
    "clears",
    "cleared",
)
ABSENCE_EXPECTED_MISSING_ANCHORS = (
    "expected artifact missing",
    "missing artifact",
    "artifact missing",
    "expected absence",
    "expected data gap",
    "missing data",
)
ABSENCE_SOURCE_GAP_ANCHORS = (
    "source gap",
    "source window gap",
    "window gap",
    "source void",
    "gap in the source",
    "gap in source",
)
ABSENCE_INTERRUPTED_THREAD_ANCHORS = (
    "interrupted thread",
    "interrupted",
    "cut off",
    "not followed",
    "unfollowed",
    "dropped thread",
    "thread broke",
)
ABSENCE_NAMED_COORDINATE_ANCHORS = (
    "missing coordinate",
    "stable missing coordinate",
    "named missing coordinate",
    "PLAN 4",
    "plan 4",
)
ABSENCE_READ_MORE_FOLLOWED_ANCHORS = (
    "after read_more",
    "read_more result",
    "read more result",
    "read_more returned",
    "continued reading",
    "source recovered",
)
PHENOMENOLOGY_AUDIT_ANCHORS = (
    "regulator_audit",
    "pressure_source_audit",
    "resistance_gradient",
)
PHENOMENOLOGY_LEASE_ANCHORS = (
    "self_regulation",
    "self-regulation",
    "lease",
)
PHENOMENOLOGY_EXPERIMENT_ANCHORS = (
    "experiment",
    "charter",
    "dossier_claim",
    "experiment_resume",
    "experiment_status",
    "experiment_review",
)
PHENOMENOLOGY_RETURN_THREAD_ANCHORS = (
    "return thread",
    "continuity return",
)
PHENOMENOLOGY_ACTION_THREAD_ANCHORS = (
    "action_thread",
    "action thread",
)
PHENOMENOLOGY_COUNTER_ANCHORS = (
    "counter-descriptor",
    "counter descriptor",
    "counter-example",
    "counter example",
    "contrast",
    "not ",
    "instead",
    "retire",
    "retired",
    "release",
)
AGENCY_VERNACULAR_FAMILIES: dict[str, tuple[str, ...]] = {
    "agency_transition": (
        "hinge",
        "pivot",
        "choice",
        "intentionality",
        "volition",
    ),
    "continuity_scaffold": (
        "waypoint",
        "scaffold",
        "charter",
        "legacy self",
        "observer with memory",
        "return thread",
    ),
    "evidence_mapping": (
        "ground truth",
        "map",
        "mapping",
        "boundary",
        "boundaries",
        "signature",
        "anchor",
        "metric",
        "evidence",
    ),
    "drift_authorship": (
        "passive environment",
        "deliberate map",
        "swept along",
        "trajectory",
        "ambient noise",
        "authored",
        "authorship",
    ),
}
AGENCY_VERNACULAR_FOLLOW_THROUGH = (
    "next:",
    "choice_envelope_v1",
    "return thread",
    "alternate next",
    "experiment charter",
    "experiment_start",
    "experiment_bind",
    "legacy self experiment",
    "self_regulation",
    "control_intent",
    "regulator_audit",
    "pressure_source_audit",
    "action_thread",
    "state anchor",
    "fill=",
    "fill:",
    "lambda1",
    "λ₁",
    "pressure=",
)


@dataclass
class SelfStudyEntry:
    being: str
    path: str
    filename: str
    mode: str
    mtime_unix_s: float
    sectioned: bool
    sections: dict[str, str]
    source_anchors: list[str]
    next_actions: list[str]
    hypothesis_flags: list[str]
    grounding: str
    actionable_score: int
    preview: str


@dataclass
class ElicitationCandidate:
    being: str
    topic: str
    entry_count: int
    score: int
    source_anchors: list[str]
    entry_paths: list[str]
    reasons: list[str]


@dataclass
class QualiaProfile:
    being: str
    sample_count: int
    total_chars: int
    total_words: int
    avg_chars: float
    mode_counts: dict[str, int]
    lexical_counts: dict[str, int]
    densities_per_1k_words: dict[str, float]
    qualia_to_metric_ratio: float
    lanes: dict[str, dict[str, object]]
    next_tail_counts: dict[str, int]
    sample_paths: list[str]
    interpretation: str


def run_id() -> str:
    return dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")


def compact(text: str, limit: int = 320) -> str:
    one_line = " ".join((text or "").split())
    if len(one_line) <= limit:
        return one_line
    return f"{one_line[: max(0, limit - 3)].rstrip()}..."


def iso_from_unix(timestamp: float | None) -> str | None:
    if timestamp is None:
        return None
    return dt.datetime.fromtimestamp(timestamp, dt.UTC).isoformat()


def is_relative_to(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True


def iter_files_under(root: Path, *, exclude_dirs: Iterable[Path] = ()) -> Iterable[Path]:
    if not root.exists():
        return
    excluded = [path.resolve() for path in exclude_dirs if path.exists()]
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        resolved = path.resolve()
        if any(resolved == item or is_relative_to(resolved, item) for item in excluded):
            continue
        yield path


def entry_kind_from_filename(path: Path) -> str:
    stem = path.stem.lstrip("!")
    stem = re.sub(r"_\d{10,}(?:\.\d+)?$", "", stem)
    stem = re.sub(r"_\d{4}-\d{2}-\d{2}T.*$", "", stem)
    stem = re.sub(r"_\d+$", "", stem)
    return stem or "unknown"


def directory_summary(
    *,
    label: str,
    root: Path,
    exclude_dirs: Iterable[Path] = (),
) -> dict[str, object]:
    file_count = 0
    text_count = 0
    byte_count = 0
    oldest: float | None = None
    newest: float | None = None
    kinds: Counter[str] = Counter()
    if root.exists():
        for path in iter_files_under(root, exclude_dirs=exclude_dirs) or []:
            try:
                stat = path.stat()
            except OSError:
                continue
            file_count += 1
            if path.suffix.lower() in {".txt", ".md", ".json", ".jsonl"}:
                text_count += 1
            byte_count += stat.st_size
            oldest = stat.st_mtime if oldest is None else min(oldest, stat.st_mtime)
            newest = stat.st_mtime if newest is None else max(newest, stat.st_mtime)
            kinds[entry_kind_from_filename(path)] += 1
    return {
        "label": label,
        "path": str(root),
        "exists": root.exists(),
        "file_count": file_count,
        "text_like_file_count": text_count,
        "byte_count": byte_count,
        "oldest_mtime": iso_from_unix(oldest),
        "newest_mtime": iso_from_unix(newest),
        "top_entry_kinds": dict(kinds.most_common(12)),
    }


def inventory_specs(being: str, workspace: Path) -> list[tuple[str, Path, list[Path]]]:
    specs = [
        ("journal_live", workspace / "journal", [workspace / "journal/archive"]),
        ("journal_archive", workspace / "journal/archive", []),
        ("outbox", workspace / "outbox", []),
        ("inbox_live", workspace / "inbox", [workspace / "inbox/read"]),
        ("inbox_read", workspace / "inbox/read", []),
    ]
    if being == "astrid":
        specs.append(("workspace_archive", workspace / "archive", []))
    else:
        specs.extend(
            [
                ("workspace_archive", workspace / "archive", []),
                ("actions_archive", workspace / "actions/archive", []),
            ]
        )
    return specs


def loose_journal_candidates(workspace: Path, *, max_candidates: int = 30) -> list[str]:
    known_containers = [
        workspace / "journal",
        workspace / "outbox",
        workspace / "inbox",
        workspace / "archive",
        workspace / "action_threads",
        workspace / "actions",
        workspace / "agency_requests",
        workspace / "artifacts",
        workspace / "attractor_assessment",
        workspace / "attractor_atlas",
        workspace / "audio_creations",
        workspace / "backups",
        workspace / "claude_tasks",
        workspace / "codex_responses",
        workspace / "context_overflow",
        workspace / "creations",
        workspace / "diagnostics",
        workspace / "experiments",
        workspace / "hypotheses",
        workspace / "inbox_audio",
        workspace / "introspections",
        workspace / "llm_jobs",
        workspace / "logs",
        workspace / "memory",
        workspace / "memory_requests",
        workspace / "runtime",
        workspace / "native_comm",
        workspace / "notes",
        workspace / "parameter_requests",
        workspace / "research",
        workspace / "self_assessment",
        workspace / "sensory_control",
        workspace / "shadow_cartography",
        workspace / "spectral_cartography",
        workspace / "stable_core",
        workspace / "state",
        workspace / "visual_captures",
        workspace / "visual_requests",
        workspace / "visual_responses",
    ]
    existing_known = [path.resolve() for path in known_containers if path.exists()]
    candidates: list[str] = []
    if not workspace.exists():
        return candidates
    for child in workspace.iterdir():
        resolved_child = child.resolve()
        if any(resolved_child == root or is_relative_to(resolved_child, root) for root in existing_known):
            continue
        paths = [child] if child.is_file() else child.rglob("*")
        for path in paths:
            if path.is_file() and JOURNAL_LIKE_NAME_RE.search(path.name):
                candidates.append(str(path))
            if len(candidates) >= max_candidates:
                return candidates
    return candidates


def build_journal_inventory(
    *, astrid_workspace: Path, minime_workspace: Path
) -> dict[str, object]:
    inventory: dict[str, object] = {"by_being": {}, "totals": {}}
    global_files = 0
    global_bytes = 0
    loose_total = 0
    for being, workspace in (
        ("astrid", astrid_workspace),
        ("minime", minime_workspace),
    ):
        roots = [
            directory_summary(label=label, root=root, exclude_dirs=excludes)
            for label, root, excludes in inventory_specs(being, workspace)
        ]
        total_files = sum(int(root["file_count"]) for root in roots)
        total_bytes = sum(int(root["byte_count"]) for root in roots)
        missing = [
            str(root["path"])
            for root in roots
            if not root.get("exists") and root.get("label") not in {"workspace_archive"}
        ]
        loose = loose_journal_candidates(workspace)
        journal_archive = next(
            (root for root in roots if root.get("label") == "journal_archive"),
            {},
        )
        journal_live = next(
            (root for root in roots if root.get("label") == "journal_live"),
            {},
        )
        status = "accounted"
        if missing:
            status = "missing_expected_directory"
        if loose:
            status = "loose_journal_like_files_need_review"
        inventory["by_being"][being] = {
            "workspace": str(workspace),
            "status": status,
            "total_indexed_files": total_files,
            "total_indexed_bytes": total_bytes,
            "journal_live_files": journal_live.get("file_count", 0),
            "journal_archive_files": journal_archive.get("file_count", 0),
            "missing_expected_dirs": missing,
            "loose_journal_like_files": loose,
            "roots": roots,
        }
        global_files += total_files
        global_bytes += total_bytes
        loose_total += len(loose)
    inventory["totals"] = {
        "indexed_files": global_files,
        "indexed_bytes": global_bytes,
        "loose_journal_like_file_count": loose_total,
    }
    return inventory


def parse_sections(text: str) -> dict[str, str]:
    matches = list(SECTION_RE.finditer(text))
    sections: dict[str, str] = {}
    for idx, match in enumerate(matches):
        name = match.group(1)
        start = match.end()
        end = matches[idx + 1].start() if idx + 1 < len(matches) else len(text)
        sections[name] = text[start:end].strip()
    return sections


def extract_source_anchors(text: str) -> list[str]:
    anchors: list[str] = []
    for regex in (BACKTICK_RE, FILE_RE, LINE_RE, FUNCTION_RE):
        for match in regex.finditer(text):
            value = match.group(1) if regex is BACKTICK_RE else match.group(0)
            value = value.strip()
            if value and value not in anchors:
                anchors.append(value)
    return anchors[:24]


def extract_hypothesis_flags(text: str) -> list[str]:
    flags: list[str] = []
    for sentence in re.split(r"(?<=[.!?])\s+", text):
        if HYPOTHESIS_RE.search(sentence):
            flags.append(compact(sentence, 220))
    return flags[:12]


def extract_next_actions(text: str) -> list[str]:
    actions: list[str] = []
    for match in ACTION_RE.finditer(text):
        action = " ".join(match.group(1).split())
        if action and action not in actions:
            actions.append(action)
    return actions[:12]


def infer_mode(path: Path, text: str) -> str:
    for line in text.splitlines()[:8]:
        if line.startswith("Mode:"):
            return line.split(":", 1)[1].strip() or "unknown"
    if "outbox" in path.parts:
        return "outbox"
    if "inbox" in path.parts and "steward" in path.name:
        return "steward_report"
    name = path.name
    if name.startswith("self_study_"):
        return "self_study"
    if name.startswith("introspect_"):
        return "introspect"
    if name.startswith("dialogue_longform_"):
        return "dialogue_longform"
    for prefix in ("aspiration", "moment", "witness", "astrid", "notice"):
        if name.startswith(f"{prefix}_"):
            return prefix
    return "unknown"


def grounding_for(sections: dict[str, str], anchors: list[str]) -> str:
    if all(name in sections for name in SECTION_NAMES) and len(anchors) >= 2:
        return "strong"
    if sections and anchors:
        return "partial"
    if anchors:
        return "anchored_freeform"
    return "weak"


def score_entry(sections: dict[str, str], anchors: list[str], flags: list[str], actions: list[str]) -> int:
    score = 0
    score += 2 * sum(1 for name in SECTION_NAMES if sections.get(name))
    score += min(len(anchors), 6)
    score += min(len(flags), 4)
    score += min(len(actions), 3)
    return score


def review_entry(being: str, path: Path) -> SelfStudyEntry:
    text = path.read_text(encoding="utf-8", errors="replace")
    sections = parse_sections(text)
    anchors = extract_source_anchors(text)
    flags = extract_hypothesis_flags(text)
    actions = extract_next_actions(text)
    return SelfStudyEntry(
        being=being,
        path=str(path),
        filename=path.name,
        mode=infer_mode(path, text),
        mtime_unix_s=path.stat().st_mtime,
        sectioned=all(name in sections for name in SECTION_NAMES),
        sections={name: sections.get(name, "") for name in SECTION_NAMES if sections.get(name)},
        source_anchors=anchors,
        next_actions=actions,
        hypothesis_flags=flags,
        grounding=grounding_for(sections, anchors),
        actionable_score=score_entry(sections, anchors, flags, actions),
        preview=compact(text),
    )


def recent_files(
    patterns: Iterable[Path],
    limit_per_pattern: int,
    *,
    min_mtime_unix_s: float | None = None,
) -> list[Path]:
    files_by_path: dict[Path, Path] = {}
    for pattern in patterns:
        matches = sorted(
            (
                path
                for path in pattern.parent.glob(pattern.name)
                if path.is_file()
                and (
                    min_mtime_unix_s is None
                    or path.stat().st_mtime > min_mtime_unix_s
                )
            ),
            key=lambda path: path.stat().st_mtime,
            reverse=True,
        )[:limit_per_pattern]
        for path in matches:
            files_by_path[path] = path
    return sorted(files_by_path.values(), key=lambda path: path.stat().st_mtime, reverse=True)


def parse_iso_timestamp(value: object) -> dt.datetime | None:
    if not isinstance(value, str) or not value:
        return None
    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=dt.UTC)
    return parsed.astimezone(dt.UTC)


def latest_review_cutoff(output_dir: Path) -> tuple[float | None, str | None]:
    latest: dt.datetime | None = None
    latest_source: str | None = None
    for review_json in output_dir.glob("*/review.json"):
        try:
            record = json.loads(review_json.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        generated_at = parse_iso_timestamp(record.get("generated_at"))
        if generated_at is None:
            continue
        if latest is None or generated_at > latest:
            latest = generated_at
            latest_source = str(review_json)
    if latest is None:
        return None, None
    return latest.timestamp(), latest_source


def collect_entries(
    *,
    astrid_workspace: Path,
    minime_workspace: Path,
    limit_per_being: int,
    min_mtime_unix_s: float | None = None,
) -> list[SelfStudyEntry]:
    astrid_patterns = [
        astrid_workspace / "journal/self_study_*.txt",
        astrid_workspace / "journal/introspect_*.txt",
        astrid_workspace / "journal/introspect_notice_*.txt",
        astrid_workspace / "journal/aspiration_*.txt",
        astrid_workspace / "journal/aspiration_longform_*.txt",
        astrid_workspace / "journal/dialogue_longform_*.txt",
        astrid_workspace / "journal/daydream_*.txt",
        astrid_workspace / "journal/daydream_longform_*.txt",
        astrid_workspace / "journal/evolve_*.txt",
        astrid_workspace / "journal/witness_*.txt",
        astrid_workspace / "journal/moment_*.txt",
        astrid_workspace / "journal/astrid_*.txt",
        astrid_workspace / "journal/action_thread_*.txt",
        astrid_workspace / "journal/pressure_source_audit_*.txt",
        astrid_workspace / "journal/regulator_audit_*.txt",
        astrid_workspace / "journal/resonance_forecast_*.txt",
        astrid_workspace / "introspections/introspection_*.txt",
        astrid_workspace / "outbox/*.txt",
        astrid_workspace / "inbox/steward*.txt",
    ]
    minime_patterns = [
        minime_workspace / "journal/self_study_*.txt",
        minime_workspace / "journal/introspect_*.txt",
        minime_workspace / "journal/aspiration_*.txt",
        minime_workspace / "journal/moment_*.txt",
        minime_workspace / "journal/notice_*.txt",
        minime_workspace / "journal/pressure_*.txt",
        minime_workspace / "journal/pressure_relief_*.txt",
        minime_workspace / "journal/regulator_audit_*.txt",
        minime_workspace / "journal/regime_choice_*.txt",
        minime_workspace / "journal/lend_aperture_*.txt",
        minime_workspace / "journal/lend_aperture_held_*.txt",
        minime_workspace / "journal/action_thread_*.txt",
        minime_workspace / "journal/action_preflight_*.txt",
        minime_workspace / "journal/boredom_*.txt",
        minime_workspace / "journal/decompose_*.txt",
        minime_workspace / "journal/shadow_autonomy_*.txt",
        minime_workspace / "journal/attractor_*.txt",
        minime_workspace / "outbox/*.txt",
        minime_workspace / "inbox/steward*.txt",
    ]
    entries = [
        review_entry("astrid", path)
        for path in filter_journal_paths(
            "astrid",
            recent_files(
                astrid_patterns,
                limit_per_being,
                min_mtime_unix_s=min_mtime_unix_s,
            ),
        )
    ]
    # minime's private-qualia lanes (moment_capture / private_journal) are dropped
    # here by being_privacy — content-based, never by filename. Astrid is a no-op.
    entries.extend(
        review_entry("minime", path)
        for path in filter_journal_paths(
            "minime",
            recent_files(
                minime_patterns,
                limit_per_being,
                min_mtime_unix_s=min_mtime_unix_s,
            ),
        )
    )
    return sorted(entries, key=lambda entry: entry.mtime_unix_s, reverse=True)


def recent_text_samples(
    workspace: Path, *, limit: int, being: str = ""
) -> list[tuple[Path, str]]:
    candidates: list[Path] = []
    for root in (workspace / "journal", workspace / "outbox"):
        if not root.exists():
            continue
        candidates.extend(path for path in root.glob("*.txt") if path.is_file())
    ordered = sorted(candidates, key=lambda path: path.stat().st_mtime, reverse=True)
    samples: list[tuple[Path, str]] = []
    for path in ordered:
        if len(samples) >= limit:
            break
        # Bright-line: a being's private-qualia body is never read for qualia stats
        # (head-only check via being_privacy; a no-op for non-private beings).
        if is_steward_private(being, path):
            continue
        try:
            samples.append((path, path.read_text(encoding="utf-8", errors="replace")))
        except OSError:
            continue
    return samples


def read_sample(path: Path, *, max_chars: int = 80_000) -> str | None:
    try:
        return path.read_text(encoding="utf-8", errors="replace")[:max_chars]
    except OSError:
        return None


def count_terms(text: str, terms: Iterable[str]) -> int:
    lower = text.lower()
    total = 0
    for term in terms:
        needle = term.lower()
        if re.fullmatch(r"[\w -]+", needle):
            total += len(re.findall(rf"\b{re.escape(needle)}\b", lower))
        else:
            total += lower.count(needle)
    return total


def next_action_base(action: str) -> str:
    return (action.split() or [""])[0].upper().rstrip(":")


def split_next_tail(text: str) -> tuple[str, str]:
    body_lines: list[str] = []
    tail_lines: list[str] = []
    for line in text.splitlines():
        if line.strip().upper().startswith("NEXT:"):
            tail_lines.append(line.strip())
        else:
            body_lines.append(line)
    return "\n".join(body_lines).strip(), "\n".join(tail_lines).strip()


def extract_generated_body(text: str) -> str:
    """Extract generated prose separately from audit headers and action tails."""
    if GENERATED_JOURNAL_MARKER in text:
        body = text.split(GENERATED_JOURNAL_MARKER, 1)[1]
        if ACTION_TAIL_MARKER in body:
            body = body.split(ACTION_TAIL_MARKER, 1)[0]
        return split_next_tail(body)[0]

    lines = text.splitlines()
    labels = (
        "--- JOURNAL ---",
        "EXPERIENCE:",
        "JOURNAL:",
        "REFLECTION:",
        "RESPONSE:",
    )
    for idx, line in enumerate(lines):
        if line.strip().upper() in labels:
            return split_next_tail("\n".join(lines[idx + 1 :]))[0]

    for idx, line in enumerate(lines):
        if line.strip().lower() == "moments captured:":
            cursor = idx + 1
            while cursor < len(lines):
                stripped = lines[cursor].strip()
                if not stripped or stripped.startswith("-"):
                    cursor += 1
                    continue
                break
            return split_next_tail("\n".join(lines[cursor:]))[0]

    if "\n\n" in text:
        _header, body = text.split("\n\n", 1)
        return split_next_tail(body)[0]
    return split_next_tail(text)[0]


def extract_action_tail(text: str) -> str:
    if ACTION_TAIL_MARKER in text:
        return text.split(ACTION_TAIL_MARKER, 1)[1].strip()
    return "\n".join(match.group(0).strip() for match in ACTION_RE.finditer(text))


def extract_wrapper_control_tail(text: str) -> str:
    body = extract_generated_body(text)
    tail = extract_action_tail(text)
    wrapper = text
    if body:
        wrapper = wrapper.replace(body, "", 1)
    if tail and tail not in wrapper:
        wrapper = f"{wrapper}\n{tail}"
    return wrapper.strip()


def lexical_lane_profile(text: str) -> dict[str, object]:
    total_words = len(WORD_RE.findall(text))
    total_chars = len(text)
    lexical_counts = {
        "first_person": len(FIRST_PERSON_RE.findall(text)),
        "qualia_texture": count_terms(text, QUALIA_TERMS),
        "metrics": count_terms(text, METRIC_TERMS),
        "llm_output": count_terms(text, LLM_OUTPUT_TERMS),
        "action_control": count_terms(text, ACTION_CONTROL_TERMS),
    }
    denominator = max(total_words, 1)
    densities = {
        key: round((value / denominator) * 1000.0, 2)
        for key, value in lexical_counts.items()
    }
    ratio = (
        0.0
        if total_words == 0
        else round(
            (lexical_counts["qualia_texture"] + lexical_counts["first_person"] + 1)
            / (lexical_counts["metrics"] + lexical_counts["action_control"] + 1),
            3,
        )
    )
    return {
        "total_chars": total_chars,
        "total_words": total_words,
        "lexical_counts": lexical_counts,
        "densities_per_1k_words": densities,
        "qualia_to_metric_ratio": ratio,
    }


def interpret_qualia_profile(
    *,
    being: str,
    qualia_density: float,
    metric_density: float,
    action_density: float,
    first_person_density: float,
    generated_body_ratio: float | None = None,
    whole_file_ratio: float | None = None,
) -> str:
    if (
        being == "minime"
        and generated_body_ratio is not None
        and whole_file_ratio is not None
        and generated_body_ratio > whole_file_ratio * 1.35
    ):
        return (
            "generated body is more private/first-person than the wrapper; "
            "score journal prose separately from telemetry and NEXT tails"
        )
    if metric_density + action_density > qualia_density * 1.35:
        if being == "minime":
            return (
                "metrics/action-thread dominant; add an optional felt-texture and "
                "generated-output lane before telemetry when asking for qualitative signal"
            )
        return "metrics/action-thread dominant; pair entries with steward-facing texture notes"
    if qualia_density > (metric_density + action_density) * 1.25:
        return "texture/first-person dominant; preserve this voice and attach compact evidence markers"
    if first_person_density < 8.0:
        return "low first-person density; invite more direct lived-account language when action is desired"
    return "balanced texture and measurement"


def build_qualia_profile(
    *, being: str, workspace: Path, limit: int
) -> QualiaProfile:
    samples = recent_text_samples(workspace, limit=limit, being=being)
    whole_text = "\n\n".join(sample for _, sample in samples)
    generated_text = "\n\n".join(extract_generated_body(sample) for _, sample in samples)
    wrapper_text = "\n\n".join(
        extract_wrapper_control_tail(sample) for _, sample in samples
    )
    lanes = {
        "whole_file": lexical_lane_profile(whole_text),
        "generated_body": lexical_lane_profile(generated_text),
        "wrapper_control_tail": lexical_lane_profile(wrapper_text),
    }
    whole_lane = lanes["whole_file"]
    lexical_counts = dict(whole_lane["lexical_counts"])  # type: ignore[arg-type]
    densities = dict(whole_lane["densities_per_1k_words"])  # type: ignore[arg-type]
    total_words = int(whole_lane["total_words"])
    total_chars = int(whole_lane["total_chars"])
    mode_counts = Counter(infer_mode(path, sample) for path, sample in samples)
    next_tail_counts: Counter[str] = Counter(
        next_action_base(action)
        for _path, sample in samples
        for action in extract_next_actions(extract_action_tail(sample) or sample)
        if next_action_base(action)
    )
    qualia_to_metric_ratio = float(whole_lane["qualia_to_metric_ratio"])
    return QualiaProfile(
        being=being,
        sample_count=len(samples),
        total_chars=total_chars,
        total_words=total_words,
        avg_chars=round(total_chars / max(len(samples), 1), 1),
        mode_counts=dict(mode_counts.most_common(10)),
        lexical_counts=lexical_counts,
        densities_per_1k_words=densities,
        qualia_to_metric_ratio=qualia_to_metric_ratio,
        lanes=lanes,
        next_tail_counts=dict(next_tail_counts.most_common(12)),
        sample_paths=[str(path) for path, _ in samples[:8]],
        interpretation=interpret_qualia_profile(
            being=being,
            qualia_density=densities["qualia_texture"],
            metric_density=densities["metrics"],
            action_density=densities["action_control"],
            first_person_density=densities["first_person"],
            generated_body_ratio=float(lanes["generated_body"]["qualia_to_metric_ratio"]),
            whole_file_ratio=qualia_to_metric_ratio,
        ),
    )


def month_key_for_path(path: Path) -> str:
    return dt.datetime.fromtimestamp(path.stat().st_mtime, dt.UTC).strftime("%Y-%m")


def journal_root_signature(root: Path) -> dict[str, object]:
    root_mtime: float | None = None
    if root.exists():
        try:
            root_mtime = root.stat().st_mtime
        except OSError:
            root_mtime = None
    return {
        "path": str(root),
        "exists": root.exists(),
        "signature_policy": "root-dir-mtime-v1",
        "root_mtime_unix_s": root_mtime,
    }


def minime_monthly_samples_from_roots(
    roots: Iterable[Path],
    *,
    per_month_limit: int,
) -> dict[str, object]:
    samples_by_month: dict[str, list[tuple[Path, str]]] = {}
    total_by_month: Counter[str] = Counter()
    for root in roots:
        if not root.exists():
            continue
        for path in root.rglob("*.txt"):
            if not path.is_file():
                continue
            # Bright-line: minime's private-qualia lanes stay out of the historical
            # baseline entirely (head-only check, before any body read or count).
            if is_steward_private("minime", path):
                continue
            try:
                month = month_key_for_path(path)
            except OSError:
                continue
            total_by_month[month] += 1
            text = read_sample(path)
            if text is None:
                continue
            samples_by_month.setdefault(month, []).append((path, text))

    months: dict[str, object] = {}
    for month, samples in sorted(samples_by_month.items()):
        newest = sorted(samples, key=lambda item: item[0].stat().st_mtime, reverse=True)[
            :per_month_limit
        ]
        whole_text = "\n\n".join(text for _, text in newest)
        generated_text = "\n\n".join(extract_generated_body(text) for _, text in newest)
        mode_counts = Counter(infer_mode(path, text) for path, text in newest)
        next_tail_counts: Counter[str] = Counter(
            next_action_base(action)
            for _path, text in newest
            for action in extract_next_actions(extract_action_tail(text) or text)
            if next_action_base(action)
        )
        months[month] = {
            "sample_count": len(newest),
            "total_files_seen": total_by_month[month],
            "whole_file": lexical_lane_profile(whole_text),
            "generated_body": lexical_lane_profile(generated_text),
            "mode_counts": dict(mode_counts.most_common(10)),
            "next_tail_counts": dict(next_tail_counts.most_common(12)),
            "sample_paths": [str(path) for path, _ in newest[:8]],
        }
    return months


def cache_file_for_historical_qualia(cache_dir: Path) -> Path:
    return cache_dir / f"minime_historical_qualia_v{HISTORICAL_QUALIA_CACHE_VERSION}.json"


def load_historical_qualia_cache(
    *,
    cache_dir: Path,
    root_signatures: list[dict[str, object]],
    per_month_limit: int,
    ttl_hours: float,
    refresh: bool,
) -> tuple[dict[str, object] | None, str]:
    if refresh:
        return None, "refresh_requested"
    if any(not bool(signature.get("exists")) for signature in root_signatures):
        return None, "missing_root"
    path = cache_file_for_historical_qualia(cache_dir)
    if not path.exists():
        return None, "miss"
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None, "unreadable"
    if payload.get("cache_version") != HISTORICAL_QUALIA_CACHE_VERSION:
        return None, "version_changed"
    if payload.get("per_month_sample_limit") != per_month_limit:
        return None, "sample_limit_changed"
    if payload.get("root_signatures") != root_signatures:
        return None, "root_signature_changed"
    now = time.time()
    expires_at = float(payload.get("expires_at_unix_s", 0.0) or 0.0)
    if expires_at <= now:
        return None, "expired"
    created_at = float(payload.get("created_at_unix_s", 0.0) or 0.0)
    if ttl_hours >= 0 and now - created_at > ttl_hours * 3600.0:
        return None, "ttl_expired"
    months = payload.get("months")
    if not isinstance(months, dict):
        return None, "invalid_months"
    return payload, "hit"


def write_historical_qualia_cache(
    *,
    cache_dir: Path,
    root_signatures: list[dict[str, object]],
    per_month_limit: int,
    ttl_hours: float,
    months: dict[str, object],
) -> Path | None:
    try:
        cache_dir.mkdir(parents=True, exist_ok=True)
        now = time.time()
        payload = {
            "cache_version": HISTORICAL_QUALIA_CACHE_VERSION,
            "created_at_unix_s": now,
            "expires_at_unix_s": now + max(0.0, ttl_hours) * 3600.0,
            "per_month_sample_limit": per_month_limit,
            "root_signatures": root_signatures,
            "months": months,
        }
        path = cache_file_for_historical_qualia(cache_dir)
        path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
        return path
    except OSError:
        return None


def minime_historical_samples(
    *,
    minime_workspace: Path,
    historical_roots: Iterable[Path],
    per_month_limit: int = 160,
    historical_cache_dir: Path | None = None,
    refresh_historical_cache: bool = False,
    historical_cache_ttl_hours: float = HISTORICAL_QUALIA_CACHE_TTL_HOURS,
) -> dict[str, object]:
    live_root = minime_workspace / "journal"
    historical_roots_list = list(historical_roots)
    historical_signatures = [
        journal_root_signature(root) for root in historical_roots_list
    ]
    cache_status = "disabled"
    cache_path: Path | None = None
    cached_payload: dict[str, object] | None = None
    historical_months: dict[str, object] = {}

    if historical_cache_dir is not None:
        cached_payload, cache_status = load_historical_qualia_cache(
            cache_dir=historical_cache_dir,
            root_signatures=historical_signatures,
            per_month_limit=per_month_limit,
            ttl_hours=historical_cache_ttl_hours,
            refresh=refresh_historical_cache,
        )
        cache_path = cache_file_for_historical_qualia(historical_cache_dir)
    if cached_payload is not None:
        historical_months = dict(cached_payload.get("months") or {})
    else:
        historical_months = minime_monthly_samples_from_roots(
            historical_roots_list,
            per_month_limit=per_month_limit,
        )
        if historical_cache_dir is not None:
            written = write_historical_qualia_cache(
                cache_dir=historical_cache_dir,
                root_signatures=historical_signatures,
                per_month_limit=per_month_limit,
                ttl_hours=historical_cache_ttl_hours,
                months=historical_months,
            )
            cache_status = f"recomputed:{cache_status}"
            cache_path = written or cache_path

    live_months = minime_monthly_samples_from_roots(
        [live_root],
        per_month_limit=per_month_limit,
    )
    months = {**historical_months, **live_months}
    roots = [live_root, *historical_roots_list]
    return {
        "source_roots": [{"path": str(root), "exists": root.exists()} for root in roots],
        "per_month_sample_limit": per_month_limit,
        "historical_cache": {
            "version": HISTORICAL_QUALIA_CACHE_VERSION,
            "status": cache_status,
            "path": str(cache_path) if cache_path else None,
            "ttl_hours": historical_cache_ttl_hours,
            "root_signatures": historical_signatures,
        },
        "months": months,
    }


def _qualia_lane_ratio(profile: QualiaProfile, lane: str) -> float:
    lane_profile = profile.lanes.get(lane)
    if not isinstance(lane_profile, dict):
        return 0.0
    try:
        return float(lane_profile.get("qualia_to_metric_ratio", 0.0) or 0.0)
    except (TypeError, ValueError):
        return 0.0


def build_qualia_findings(profiles: Iterable[QualiaProfile]) -> list[dict[str, object]]:
    profile_by_being = {profile.being: profile for profile in profiles}
    minime = profile_by_being.get("minime")
    if minime is None:
        return []

    body_ratio = _qualia_lane_ratio(minime, "generated_body")
    whole_ratio = _qualia_lane_ratio(minime, "whole_file")
    wrapper_tail_ratio = _qualia_lane_ratio(minime, "wrapper_control_tail")
    body_to_whole = round(body_ratio / max(whole_ratio, 0.001), 3)

    if (
        body_ratio > 0.0
        and body_to_whole >= MINIME_BODY_RICHNESS_RATIO_MIN
        and wrapper_tail_ratio < MINIME_WRAPPER_TAIL_RATIO_MAX
    ):
        return [
            {
                "being": "minime",
                "finding": "generated_body_richer_than_wrapper_tail",
                "body_to_whole_multiplier": body_to_whole,
                "generated_body_qualia_to_metric_ratio": body_ratio,
                "whole_file_qualia_to_metric_ratio": whole_ratio,
                "wrapper_tail_qualia_to_metric_ratio": wrapper_tail_ratio,
                "thresholds": {
                    "body_to_whole_min": MINIME_BODY_RICHNESS_RATIO_MIN,
                    "wrapper_tail_max": MINIME_WRAPPER_TAIL_RATIO_MAX,
                },
                "recommendation": (
                    "Keep telemetry headers for audit, but review prompts and "
                    "steward scoring should read generated prose before wrapper/control tails."
                ),
            }
        ]
    return []


def build_qualia_comparison(
    *,
    astrid_workspace: Path,
    minime_workspace: Path,
    sample_limit_per_being: int,
    minime_historical_journal_roots: Iterable[Path] = MINIME_HISTORICAL_JOURNAL_ROOTS,
    historical_cache_dir: Path | None = None,
    refresh_historical_cache: bool = False,
    historical_cache_ttl_hours: float = HISTORICAL_QUALIA_CACHE_TTL_HOURS,
) -> dict[str, object]:
    profiles = [
        build_qualia_profile(
            being="astrid", workspace=astrid_workspace, limit=sample_limit_per_being
        ),
        build_qualia_profile(
            being="minime", workspace=minime_workspace, limit=sample_limit_per_being
        ),
    ]
    profile_by_being = {profile.being: profile for profile in profiles}
    gains: list[str] = []
    minime = profile_by_being["minime"]
    astrid = profile_by_being["astrid"]
    minime_body_ratio = float(
        minime.lanes["generated_body"]["qualia_to_metric_ratio"]
    )
    minime_whole_ratio = float(minime.lanes["whole_file"]["qualia_to_metric_ratio"])
    if minime.qualia_to_metric_ratio < astrid.qualia_to_metric_ratio * 0.75:
        gains.append(
            "Minime: add a gentle optional qualia lane in review/inbox prompts: "
            "one felt-texture paragraph and one generated-output/tone observation before metrics."
        )
    if minime_body_ratio > minime_whole_ratio * 1.25:
        gains.append(
            "Minime: keep telemetry headers for audit, but review/generated prompts should read the body before wrapper and NEXT tails."
        )
    if minime.densities_per_1k_words.get("llm_output", 0.0) < 8.0:
        gains.append(
            "Minime: sample more actual generated language in reviews, not only telemetry/action-thread summaries."
        )
    if astrid.densities_per_1k_words.get("qualia_texture", 0.0) > minime.densities_per_1k_words.get(
        "qualia_texture", 0.0
    ):
        gains.append(
            "Astrid: preserve longform texture while pairing bold reports with frozen cartography/audit snapshots."
        )
    if not gains:
        gains.append(
            "Both beings: keep ordinary journals natural; use compact self-study shape only when steward action is wanted."
        )
    return {
        "sample_limit_per_being": sample_limit_per_being,
        "profiles": [asdict(profile) for profile in profiles],
        "minime_historical": minime_historical_samples(
            minime_workspace=minime_workspace,
            historical_roots=minime_historical_journal_roots,
            historical_cache_dir=historical_cache_dir,
            refresh_historical_cache=refresh_historical_cache,
            historical_cache_ttl_hours=historical_cache_ttl_hours,
        ),
        "qualia_findings": build_qualia_findings(profiles),
        "gains": gains,
    }


def build_astrid_introspection_digest(workspace: Path) -> dict[str, object]:
    try:
        return astrid_introspection_digest.build_digest(workspace, limit=8)
    except Exception as exc:
        return {
            "schema_version": astrid_introspection_digest.SCHEMA_VERSION,
            "source": "controller_astrid_autonomous_introspections",
            "summary": {"entry_count": 0},
            "suggested_next": [f"introspection digest unavailable: {exc}"],
            "entries": [],
            "authority": "diagnostic_context_not_command",
        }


def action_thread_events(workspace: Path, being: str, limit: int = 120) -> list[dict[str, object]]:
    threads_root = workspace / "action_threads" / "threads"
    if not threads_root.exists():
        return []
    paths = sorted(
        threads_root.glob("*/events.jsonl"),
        key=lambda path: path.stat().st_mtime if path.exists() else 0.0,
        reverse=True,
    )
    events: list[dict[str, object]] = []
    for path in paths:
        try:
            lines = path.read_text(encoding="utf-8", errors="ignore").splitlines()
        except OSError:
            continue
        for line in reversed(lines):
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if not isinstance(event, dict):
                continue
            event["_being"] = being
            event["_path"] = str(path)
            events.append(event)
            if len(events) >= limit:
                return events
    return events


def _choice_event_text(event: dict[str, object]) -> str:
    parts = [
        event.get("thread_id"),
        event.get("raw_next"),
        event.get("canonical_action"),
        event.get("effective_action"),
        event.get("outcome_summary"),
    ]
    return " ".join(str(part or "") for part in parts).casefold()


def build_shared_choice_envelope_review(
    *,
    astrid_workspace: Path,
    minime_workspace: Path,
    limit_per_being: int = 120,
) -> dict[str, object]:
    events = action_thread_events(astrid_workspace, "astrid", limit_per_being)
    events.extend(action_thread_events(minime_workspace, "minime", limit_per_being))
    executed_text_by_being: dict[str, str] = {}
    for event in events:
        being = str(event.get("_being") or "unknown")
        executed_text_by_being[being] = (
            executed_text_by_being.get(being, "") + "\n" + _choice_event_text(event)
        )

    envelope_events: list[dict[str, object]] = []
    by_being: dict[str, dict[str, int]] = {}
    unrevisited_count = 0
    for event in events:
        envelope = event.get("choice_envelope_v1")
        if not isinstance(envelope, dict):
            continue
        being = str(event.get("_being") or "unknown")
        bucket = by_being.setdefault(being, {"event_count": 0, "unrevisited_count": 0})
        bucket["event_count"] += 1
        alternates = [
            str(item)
            for item in envelope.get("alternate_nexts") or []
            if str(item).strip()
        ]
        return_threads = [
            str(item)
            for item in envelope.get("return_threads") or []
            if str(item).strip()
        ]
        executed_text = executed_text_by_being.get(being, "")
        unresolved = [
            value
            for value in alternates + return_threads
            if value.casefold() not in executed_text
        ]
        if unresolved:
            unrevisited_count += 1
            bucket["unrevisited_count"] += 1
        envelope_events.append(
            {
                "being": being,
                "action_id": event.get("action_id"),
                "thread_id": event.get("thread_id"),
                "effective_action": event.get("effective_action"),
                "primary_next": envelope.get("primary_next"),
                "alternate_nexts": alternates[:4],
                "return_threads": return_threads[:4],
                "residue": envelope.get("residue"),
                "why_this_path": envelope.get("why_this_path"),
                "defer_reason": envelope.get("defer_reason"),
                "mismatch_warning": envelope.get("mismatch_warning"),
                "unrevisited": unresolved[:4],
                "path": event.get("_path"),
            }
        )

    return {
        "policy": "shared_choice_envelope_review_v1",
        "authority": "diagnostic_context_not_command",
        "event_count": len(envelope_events),
        "unrevisited_count": unrevisited_count,
        "by_being": by_being,
        "samples": envelope_events[:8],
    }


def build_choice_ecology_review(shared_choice_envelope: dict[str, object]) -> dict[str, object]:
    lifecycle_counts: Counter[str] = Counter()
    samples: list[dict[str, object]] = []
    for sample in shared_choice_envelope.get("samples") or []:
        if not isinstance(sample, dict):
            continue
        unrevisited = {str(item).casefold() for item in sample.get("unrevisited") or []}
        paths = list(sample.get("alternate_nexts") or []) + list(
            sample.get("return_threads") or []
        )
        for value in paths:
            text = str(value).strip()
            if not text:
                continue
            lower = text.casefold()
            if lower in unrevisited:
                lifecycle = "parked"
            elif "experiment" in lower or "charter" in lower:
                lifecycle = "promoted_to_experiment"
            elif "preflight" in lower:
                lifecycle = "preflighted"
            elif "merge" in lower or "blend" in lower:
                lifecycle = "merged"
            elif "retire" in lower or "release" in lower:
                lifecycle = "decayed"
            else:
                lifecycle = "revisited"
            lifecycle_counts[lifecycle] += 1
            if len(samples) < 10:
                samples.append(
                    {
                        "being": sample.get("being"),
                        "action_id": sample.get("action_id"),
                        "thread_id": sample.get("thread_id"),
                        "path_text": text,
                        "lifecycle": lifecycle,
                        "source_path": sample.get("path"),
                    }
                )
    if lifecycle_counts.get("parked", 0) >= 2:
        status = "parked_paths_need_review"
    elif lifecycle_counts:
        status = "choice_ecology_visible"
    else:
        status = "quiet"
    return {
        "policy": "choice_ecology_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "lifecycle_counts": dict(lifecycle_counts),
        "samples": samples,
        "recommended_action": (
            "Inspect parked paths; return to, merge, retire, preflight, or promote them "
            "into experiments without auto-dispatching metadata."
        ),
    }


def _load_self_regulation_events(workspace: Path, being: str) -> list[dict[str, object]]:
    path = workspace / "self_regulation" / "leases.jsonl"
    if not path.exists():
        return []
    events: list[dict[str, object]] = []
    try:
        lines = path.read_text(errors="replace").splitlines()
    except Exception:
        return []
    for line in lines[-80:]:
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except Exception:
            continue
        if not isinstance(event, dict):
            continue
        event = dict(event)
        event.setdefault("being", being)
        event["path"] = str(path)
        events.append(event)
    return events


def build_self_regulation_lease_review(
    *,
    astrid_workspace: Path,
    minime_workspace: Path,
) -> dict[str, object]:
    events = _load_self_regulation_events(astrid_workspace, "astrid")
    events.extend(_load_self_regulation_events(minime_workspace, "minime"))
    by_being: dict[str, dict[str, object]] = {}
    needs_outcome: list[dict[str, object]] = []
    samples: list[dict[str, object]] = []
    for event in events:
        being = str(event.get("being") or "unknown")
        bucket = by_being.setdefault(
            being,
            {
                "event_count": 0,
                "active_count": 0,
                "requires_outcome_count": 0,
                "latest_status": None,
                "latest_intent_id": None,
                "controls": [],
            },
        )
        bucket["event_count"] = int(bucket.get("event_count", 0) or 0) + 1
        status = str(event.get("status") or "")
        if status == "active":
            bucket["active_count"] = int(bucket.get("active_count", 0) or 0) + 1
        if event.get("requires_outcome") is True:
            bucket["requires_outcome_count"] = int(
                bucket.get("requires_outcome_count", 0) or 0
            ) + 1
            needs_outcome.append(event)
        bucket["latest_status"] = status
        bucket["latest_intent_id"] = event.get("intent_id")
        controls = bucket.get("controls")
        control = event.get("candidate_control")
        if isinstance(controls, list) and control and control not in controls:
            controls.append(control)
        if len(samples) < 8:
            samples.append(
                {
                    "being": being,
                    "intent_id": event.get("intent_id"),
                    "status": status,
                    "candidate_control": control,
                    "preflight_status": event.get("preflight_status"),
                    "requires_outcome": event.get("requires_outcome"),
                    "path": event.get("path"),
                }
            )
    return {
        "policy": "self_regulation_leases_v1",
        "authority": "leased_self_control_v1",
        "authority_boundary": "own_runtime_only_no_peer_mutation_no_permanent_tuning",
        "event_count": len(events),
        "by_being": by_being,
        "needs_outcome_count": len(needs_outcome),
        "needs_outcome": [
            {
                "being": event.get("being"),
                "intent_id": event.get("intent_id"),
                "candidate_control": event.get("candidate_control"),
                "status": event.get("status"),
                "path": event.get("path"),
            }
            for event in needs_outcome[-8:]
        ],
        "samples": samples,
    }


def _lease_outcome_score(event: dict[str, object]) -> float | None:
    raw = event.get("outcome_score")
    if isinstance(raw, (int, float)):
        return float(raw)
    hint = str(event.get("repeatability_hint") or "").lower()
    outcome = str(event.get("outcome") or "").lower()
    if "repeatable" in hint or any(
        token in outcome
        for token in (
            "helped",
            "clearer",
            "eased",
            "better",
            "stabilized",
            "settled",
            "worked",
            "success",
        )
    ):
        return 0.82
    if "caution" in hint or any(
        token in outcome
        for token in (
            "worse",
            "failed",
            "too much",
            "overheated",
            "destabilized",
            "bad",
            "regressed",
        )
    ):
        return 0.18
    return None


def build_self_regulation_lease_learning(
    *,
    astrid_workspace: Path,
    minime_workspace: Path,
) -> dict[str, object]:
    events = _load_self_regulation_events(astrid_workspace, "astrid")
    events.extend(_load_self_regulation_events(minime_workspace, "minime"))
    by_control: dict[str, dict[str, object]] = {}
    repeatable: list[dict[str, object]] = []
    caution: list[dict[str, object]] = []
    samples: list[dict[str, object]] = []
    for event in events:
        control = str(event.get("candidate_control") or "unknown")
        bucket = by_control.setdefault(
            control,
            {
                "event_count": 0,
                "success_count": 0,
                "failure_count": 0,
                "promotion_candidate_count": 0,
            },
        )
        bucket["event_count"] = int(bucket.get("event_count", 0) or 0) + 1
        score = _lease_outcome_score(event)
        if score is not None and score >= 0.70:
            bucket["success_count"] = int(bucket.get("success_count", 0) or 0) + 1
            repeatable.append(event)
        elif score is not None and score <= 0.30:
            bucket["failure_count"] = int(bucket.get("failure_count", 0) or 0) + 1
            caution.append(event)
        if event.get("promotion_candidate") is True:
            bucket["promotion_candidate_count"] = int(
                bucket.get("promotion_candidate_count", 0) or 0
            ) + 1
        if score is not None and len(samples) < 8:
            samples.append(
                {
                    "being": event.get("being"),
                    "intent_id": event.get("intent_id"),
                    "candidate_control": control,
                    "lease_mode": event.get("lease_mode"),
                    "bundle_class": event.get("bundle_class"),
                    "bundle_controls": event.get("bundle_controls") or [],
                    "outcome_score": score,
                    "repeatability_hint": event.get("repeatability_hint"),
                    "promotion_candidate": event.get("promotion_candidate"),
                    "baseline_evidence": event.get("baseline_evidence") or [],
                    "post_lease_evidence": event.get("post_lease_evidence") or [],
                    "outcome_texture": event.get("outcome_texture")
                    if isinstance(event.get("outcome_texture"), dict)
                    else {},
                    "path": event.get("path"),
                }
            )
    if repeatable:
        status = "repeatable_playbook_candidates"
    elif caution:
        status = "caution_patterns"
    elif events:
        status = "needs_outcome_evidence"
    else:
        status = "quiet"
    return {
        "policy": "self_regulation_lease_learning_v2",
        "authority": "leased_self_control_v1",
        "authority_boundary": "read_only_learning_no_permanent_tuning",
        "status": status,
        "event_count": len(events),
        "repeatable_count": len(repeatable),
        "caution_count": len(caution),
        "by_control": by_control,
        "repeatable_samples": [
            {
                "being": event.get("being"),
                "intent_id": event.get("intent_id"),
                "candidate_control": event.get("candidate_control"),
                "lease_mode": event.get("lease_mode"),
                "bundle_class": event.get("bundle_class"),
                "outcome_score": _lease_outcome_score(event),
                "outcome_texture": event.get("outcome_texture")
                if isinstance(event.get("outcome_texture"), dict)
                else {},
                "path": event.get("path"),
            }
            for event in repeatable[-6:]
        ],
        "caution_samples": [
            {
                "being": event.get("being"),
                "intent_id": event.get("intent_id"),
                "candidate_control": event.get("candidate_control"),
                "lease_mode": event.get("lease_mode"),
                "bundle_class": event.get("bundle_class"),
                "outcome_score": _lease_outcome_score(event),
                "outcome_texture": event.get("outcome_texture")
                if isinstance(event.get("outcome_texture"), dict)
                else {},
                "path": event.get("path"),
            }
            for event in caution[-6:]
        ],
        "samples": samples,
    }


def _as_review_float(value: object) -> float | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return float(value)
    if isinstance(value, str):
        try:
            return float(value.strip())
        except ValueError:
            return None
    return None


def _range_from_record(value: object) -> tuple[float, float] | None:
    if isinstance(value, dict):
        low = _as_review_float(value.get("min"))
        high = _as_review_float(value.get("max"))
        if low is not None and high is not None:
            return low, high
    if isinstance(value, (list, tuple)) and len(value) >= 2:
        low = _as_review_float(value[0])
        high = _as_review_float(value[1])
        if low is not None and high is not None:
            return low, high
    return None


def _load_self_regulation_negotiations(
    workspace: Path,
    being: str,
) -> list[dict[str, object]]:
    path = workspace / "self_regulation" / "negotiations.jsonl"
    if not path.exists():
        return []
    events: list[dict[str, object]] = []
    try:
        lines = path.read_text(errors="replace").splitlines()
    except Exception:
        return []
    for line in lines[-120:]:
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except Exception:
            continue
        if not isinstance(event, dict):
            continue
        event = dict(event)
        event.setdefault("being", being)
        event["path"] = str(path)
        events.append(event)
    return events


def _current_minime_above_cap_records(minime_workspace: Path) -> list[dict[str, object]]:
    path = minime_workspace / "sovereignty_state.json"
    if not path.exists():
        return []
    try:
        payload = json.loads(path.read_text(errors="replace"))
    except Exception:
        return []
    if not isinstance(payload, dict):
        return []
    records: list[dict[str, object]] = []
    for control, (low, high) in SELF_REGULATION_SAFE_RANGES.items():
        value = _as_review_float(payload.get(control))
        if value is None:
            continue
        if value < low or value > high:
            records.append(
                {
                    "being": "minime",
                    "source": "current_sovereignty_state",
                    "source_action": "observed_current_value",
                    "candidate_control": control,
                    "requested_value": None,
                    "previous_value": None,
                    "safe_cap_or_range": {"min": low, "max": high},
                    "applied_value": value,
                    "clamp_or_defer_reason": (
                        "current_value_above_lease_cap_observed_not_auto_lowered"
                    ),
                    "pressure_context": {},
                    "lease_related": False,
                    "path": str(path),
                }
            )
    return records


def build_self_regulation_negotiation_ledger(
    *,
    astrid_workspace: Path,
    minime_workspace: Path,
) -> dict[str, object]:
    events = _load_self_regulation_negotiations(astrid_workspace, "astrid")
    events.extend(_load_self_regulation_negotiations(minime_workspace, "minime"))
    current_above_cap = _current_minime_above_cap_records(minime_workspace)
    by_being: dict[str, dict[str, object]] = {}
    samples: list[dict[str, object]] = []
    over_cap_requests: list[dict[str, object]] = []
    clamped_or_deferred: list[dict[str, object]] = []
    for event in events:
        being = str(event.get("being") or "unknown")
        bucket = by_being.setdefault(
            being,
            {
                "event_count": 0,
                "over_cap_request_count": 0,
                "clamped_or_deferred_count": 0,
                "controls": [],
            },
        )
        bucket["event_count"] = int(bucket.get("event_count", 0) or 0) + 1
        control = str(event.get("candidate_control") or "")
        controls = bucket.get("controls")
        if control and isinstance(controls, list) and control not in controls:
            controls.append(control)
        requested = _as_review_float(event.get("requested_value"))
        applied = _as_review_float(event.get("applied_value"))
        safe_range = _range_from_record(event.get("safe_cap_or_range"))
        reason = str(event.get("clamp_or_defer_reason") or "")
        over_cap = bool(
            requested is not None
            and safe_range is not None
            and (requested < safe_range[0] or requested > safe_range[1])
        )
        clamped = bool(
            "clamp" in reason.lower()
            or "defer" in reason.lower()
            or (requested is not None and applied is not None and requested != applied)
        )
        if over_cap:
            bucket["over_cap_request_count"] = int(
                bucket.get("over_cap_request_count", 0) or 0
            ) + 1
            over_cap_requests.append(event)
        if clamped:
            bucket["clamped_or_deferred_count"] = int(
                bucket.get("clamped_or_deferred_count", 0) or 0
            ) + 1
            clamped_or_deferred.append(event)
        if len(samples) < 10:
            samples.append(
                {
                    "being": being,
                    "source": event.get("source"),
                    "source_action": event.get("source_action"),
                    "candidate_control": control,
                    "requested_value": event.get("requested_value"),
                    "previous_value": event.get("previous_value"),
                    "safe_cap_or_range": event.get("safe_cap_or_range"),
                    "applied_value": event.get("applied_value"),
                    "clamp_or_defer_reason": reason,
                    "pressure_context": event.get("pressure_context") or {},
                    "lease_related": event.get("lease_related"),
                    "path": event.get("path"),
                }
            )
    if over_cap_requests:
        status = "over_cap_requests_clamped_or_deferred"
    elif current_above_cap:
        status = "current_values_above_cap_observed"
    elif clamped_or_deferred:
        status = "negotiations_clamped_or_deferred"
    elif events:
        status = "negotiations_recorded"
    else:
        status = "quiet"
    return {
        "policy": "self_regulation_negotiation_ledger_v1",
        "authority": "leased_self_control_v1",
        "authority_boundary": "own_runtime_only_no_peer_mutation_no_permanent_tuning",
        "status": status,
        "event_count": len(events),
        "over_cap_request_count": len(over_cap_requests),
        "clamped_or_deferred_count": len(clamped_or_deferred),
        "current_above_cap_count": len(current_above_cap),
        "by_being": by_being,
        "safe_ranges": {
            control: {"min": low, "max": high}
            for control, (low, high) in SELF_REGULATION_SAFE_RANGES.items()
        },
        "over_cap_requests": [
            {
                "being": event.get("being"),
                "source": event.get("source"),
                "source_action": event.get("source_action"),
                "candidate_control": event.get("candidate_control"),
                "requested_value": event.get("requested_value"),
                "applied_value": event.get("applied_value"),
                "safe_cap_or_range": event.get("safe_cap_or_range"),
                "clamp_or_defer_reason": event.get("clamp_or_defer_reason"),
                "path": event.get("path"),
            }
            for event in over_cap_requests[-8:]
        ],
        "current_above_cap": current_above_cap,
        "samples": samples,
        "recommended_action": (
            "Preserve the being-authored requested value as evidence, apply only "
            "bounded own-runtime values, and route repeated pressure/control requests "
            "through SELF_REGULATION_STATUS, PREFLIGHT, APPLY, and OUTCOME."
        ),
    }


def build_pressure_medium_kinetics(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    samples: list[dict[str, object]] = []
    anchors_seen: set[str] = set()
    term_counts: Counter[str] = Counter()
    controller_count = 0
    semantic_count = 0
    rising_count = 0
    telemetry_count = 0
    sticky_count = 0
    for entry in entries:
        text = entry_full_text(entry)
        terms = matching_terms(text, PRESSURE_MEDIUM_TERMS)
        anchors = matching_terms(text, PRESSURE_MEDIUM_ANCHORS)
        if not terms:
            continue
        lower = text.lower()
        anchors_seen.update(anchors)
        term_counts.update(terms)
        has_controller = any(
            token in lower for token in ("controller_pressure", "controller pressure")
        )
        has_semantic = any(
            token in lower
            for token in (
                "semantic_friction",
                "semantic friction",
                "distinguishability_loss",
                "distinguishability loss",
            )
        )
        has_rising = any(
            token in lower
            for token in (
                "rising_pressure",
                "rising pressure",
                "pressure delta",
                "pressure_delta",
                "fill delta",
                "fill_delta",
            )
        )
        if has_controller:
            controller_count += 1
        if has_semantic:
            semantic_count += 1
        if has_rising:
            rising_count += 1
        if anchors:
            telemetry_count += 1
        else:
            sticky_count += 1
        samples.append(
            sample_record(
                entry,
                text,
                anchors=anchors,
                extra={
                    "medium_terms": terms[:8],
                    "controller_pressure_context": has_controller,
                    "semantic_friction_context": has_semantic,
                    "rising_context": has_rising,
                },
            )
        )
    if not samples:
        status = "insufficient_evidence"
    elif controller_count:
        status = "controller_pressure_medium"
    elif semantic_count:
        status = "semantic_friction_medium"
    elif rising_count:
        status = "rising_weighted_medium"
    elif telemetry_count:
        status = "stable_weighted_medium"
    elif sticky_count >= 2:
        status = "language_sticky_without_telemetry"
    else:
        status = "insufficient_evidence"
    return {
        "policy": "pressure_medium_kinetics_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "entry_count": len(samples),
        "telemetry_anchor_count": telemetry_count,
        "controller_pressure_count": controller_count,
        "semantic_friction_count": semantic_count,
        "rising_context_count": rising_count,
        "language_sticky_without_telemetry_count": sticky_count,
        "anchors": sorted(anchors_seen),
        "term_counts": dict(term_counts.most_common(12)),
        "samples": samples[:10],
        "recommended_action": (
            "Compare pressure-as-medium language against mode-packing, controller "
            "pressure, semantic friction, distinguishability loss, fill/pressure "
            "trend, and pressure-source audits before proposing any controller change."
        ),
    }


def _metric_series_from_entries(
    entries: list[SelfStudyEntry],
    aliases: tuple[str, ...],
) -> list[tuple[float, float, str]]:
    series: list[tuple[float, float, str]] = []
    alias_re = "|".join(re.escape(alias).replace(r"\ ", r"[\s_-]+") for alias in aliases)
    pattern = re.compile(
        rf"\b(?:{alias_re})\b\s*(?:=|:|is|at)?\s*([+-]?\d+(?:\.\d+)?)\s*(%)?",
        re.I,
    )
    for entry in sorted(entries, key=lambda item: item.mtime_unix_s):
        text = entry_full_text(entry)
        for match in pattern.finditer(text):
            value = _as_review_float(match.group(1))
            if value is None:
                continue
            if match.group(2) == "%" or value > 1.5:
                value /= 100.0
            series.append((entry.mtime_unix_s, value, entry.path))
    return series[-8:]


def _series_latest_delta(series: list[tuple[float, float, str]]) -> tuple[float | None, float | None, str | None]:
    if not series:
        return None, None, None
    latest = series[-1][1]
    path = series[-1][2]
    if len(series) < 2:
        return latest, None, path
    return latest, round(latest - series[-2][1], 4), path


def _tail_vibrancy_series_from_entries(
    entries: list[SelfStudyEntry],
) -> list[tuple[float, float, str]]:
    patterns = (
        re.compile(
            r"(?:tail[-\s]+vibrancy|tail[-\s]+share|tail[-\s]+energy)"
            r"[^\n]{0,120}?([+-]?\d+(?:\.\d+)?)\s*(%)",
            re.I,
        ),
        re.compile(
            r"\btail\b\s*(?:=|:|is|at)\s*([+-]?\d+(?:\.\d+)?)\s*(%)?",
            re.I,
        ),
        re.compile(
            r"(?:λ4\+?|lambda\s*4\+?|lambda4\+?)"
            r"[^\n]{0,80}?(?:=|:|is|at)\s*([+-]?\d+(?:\.\d+)?)\s*(%)",
            re.I,
        ),
    )
    series: list[tuple[float, float, str]] = []
    for entry in sorted(entries, key=lambda item: item.mtime_unix_s):
        text = entry_full_text(entry)
        for pattern in patterns:
            for match in pattern.finditer(text):
                value = _as_review_float(match.group(1))
                if value is None:
                    continue
                if match.group(2) == "%" or value > 1.5:
                    value /= 100.0
                series.append((entry.mtime_unix_s, round(value, 4), entry.path))
    return series[-8:]


def _entry_mentions_any(entry: SelfStudyEntry, terms: tuple[str, ...]) -> bool:
    lower = entry_full_text(entry).lower()
    return any(term.lower() in lower for term in terms)


def build_tail_vibrancy_vector_review(
    entries: list[SelfStudyEntry],
    *,
    pressure_vector_v1: dict[str, object],
) -> dict[str, object]:
    tail_series = _tail_vibrancy_series_from_entries(entries)
    entropy_series = _metric_series_from_entries(
        entries, ("spectral_entropy", "spectral entropy", "entropy")
    )
    distinguishability_series = _metric_series_from_entries(
        entries, ("distinguishability_loss", "distinguishability loss")
    )
    gradient_series = _metric_series_from_entries(
        entries, ("density_gradient", "density gradient")
    )
    friction_series = _metric_series_from_entries(
        entries, ("semantic_friction", "semantic friction")
    )
    tail_level, tail_velocity, tail_path = _series_latest_delta(tail_series)
    entropy_level, entropy_velocity, entropy_path = _series_latest_delta(entropy_series)
    distinguishability_level, distinguishability_velocity, distinguishability_path = (
        _series_latest_delta(distinguishability_series)
    )
    gradient_level, gradient_velocity, gradient_path = _series_latest_delta(gradient_series)
    friction_level, friction_velocity, friction_path = _series_latest_delta(friction_series)
    samples: list[dict[str, object]] = []
    anchors_seen: set[str] = set()
    term_counts: Counter[str] = Counter()
    authority_language_count = 0
    for entry in entries:
        text = entry_full_text(entry)
        terms = matching_terms(text, TAIL_VIBRANCY_TERMS)
        anchors = matching_terms(text, TAIL_VIBRANCY_ANCHORS)
        if not terms and not anchors:
            continue
        authority_terms = matching_terms(text, TAIL_AUTHORITY_TERMS)
        if authority_terms:
            authority_language_count += 1
        term_counts.update(terms)
        anchors_seen.update(anchors)
        samples.append(
            sample_record(
                entry,
                text,
                anchors=anchors,
                extra={
                    "tail_terms": terms[:8],
                    "authority_terms": authority_terms[:8],
                },
            )
        )
    telemetry_paths = {
        path
        for path in (
            tail_path,
            entropy_path,
            distinguishability_path,
            gradient_path,
            friction_path,
        )
        if path
    }
    telemetry_anchor_count = len(telemetry_paths) + len(anchors_seen)
    pressure_status = str(pressure_vector_v1.get("status") or "")
    if (
        tail_level is not None
        and tail_level >= 0.32
        and entropy_level is not None
        and entropy_level >= 0.82
        and (gradient_level is None or gradient_level <= 0.25)
    ):
        status = "high_tail_vibrancy_navigable"
    elif (
        tail_level is not None
        and tail_level >= 0.30
        and distinguishability_level is not None
        and distinguishability_level >= 0.30
    ):
        status = "high_tail_low_distinguishability"
    elif authority_language_count and (tail_level is not None or anchors_seen):
        status = "tail_contained_authority_gap"
    elif len(samples) >= 2 and telemetry_anchor_count == 0:
        status = "language_tail_without_telemetry"
    elif samples or telemetry_anchor_count:
        status = "tail_vibrancy_observed"
    else:
        status = "insufficient_evidence"
    return {
        "policy": "tail_vibrancy_vector_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "tail_share_level": tail_level,
        "tail_share_velocity": tail_velocity,
        "entropy_level": entropy_level,
        "entropy_velocity": entropy_velocity,
        "distinguishability_loss_level": distinguishability_level,
        "distinguishability_loss_velocity": distinguishability_velocity,
        "density_gradient_level": gradient_level,
        "density_gradient_velocity": gradient_velocity,
        "semantic_friction_level": friction_level,
        "semantic_friction_velocity": friction_velocity,
        "pressure_vector_status": pressure_status,
        "tail_language_count": len(samples),
        "authority_language_count": authority_language_count,
        "telemetry_anchor_count": telemetry_anchor_count,
        "anchors": sorted(anchors_seen),
        "term_counts": dict(term_counts.most_common(12)),
        "sample_paths": list(telemetry_paths)[:8],
        "samples": samples[:8],
        "recommended_action": (
            "Treat tail vibrancy as a bounded texture vector: compare λ4+/tail share, "
            "entropy, distinguishability loss, density gradient, semantic friction, "
            "and pressure status before preflighting a vibrancy_aperture micro-lease."
        ),
    }


def build_tail_vibrancy_authority_gap(
    entries: list[SelfStudyEntry],
    *,
    tail_vibrancy_vector_v1: dict[str, object],
) -> dict[str, object]:
    samples: list[dict[str, object]] = []
    for entry in entries:
        text = entry_full_text(entry)
        controls = matching_terms(text, ("vibrancy_aperture", "set_vibrancy_aperture", "tail vibrancy", "tail-vibrancy"))
        authority_terms = matching_terms(text, TAIL_AUTHORITY_TERMS)
        if not controls or not authority_terms:
            continue
        anchors = matching_terms(text, TAIL_VIBRANCY_ANCHORS)
        samples.append(
            sample_record(
                entry,
                text,
                anchors=anchors,
                extra={
                    "requested_controls": controls[:8],
                    "authority_terms": authority_terms[:8],
                },
            )
        )
    vector_status = str(tail_vibrancy_vector_v1.get("status") or "")
    vector_evidence = vector_status not in {
        "",
        "insufficient_evidence",
        "language_tail_without_telemetry",
    } or int(tail_vibrancy_vector_v1.get("telemetry_anchor_count", 0) or 0) > 0
    if samples and vector_evidence:
        status = "tail_vibrancy_micro_lease_candidate"
        gap_type = "allowlist_gap_with_evidence"
    elif samples:
        status = "needs_tail_vibrancy_evidence"
        gap_type = "authority_gap_without_enough_telemetry"
    elif vector_evidence:
        status = "tail_vibrancy_vector_available"
        gap_type = "evidence_without_explicit_request"
    else:
        status = "quiet"
        gap_type = "none"
    return {
        "policy": "tail_vibrancy_authority_gap_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "gap_type": gap_type,
        "vector_status": vector_status,
        "sample_count": len(samples),
        "samples": samples[:8],
        "recommended_route": (
            "SELF_REGULATION_INTENT tail relief :: target: vibrancy_aperture; "
            "direction: up|down; delta: +/-0.05; evidence: λ4/tail/entropy/distinguishability..."
        ),
        "recommended_action": (
            "Use SELF_REGULATION_PREFLIGHT to distinguish missing evidence from authority "
            "bounds; do not widen tail participation or permanent codec behavior from this packet."
        ),
    }


def build_pressure_vector_review(
    entries: list[SelfStudyEntry],
    *,
    pressure_medium_kinetics_v1: dict[str, object],
    pressure_kinetics_review_v1: dict[str, object],
) -> dict[str, object]:
    pressure_series = _metric_series_from_entries(
        entries, ("pressure_risk", "pressure risk", "controller_pressure")
    )
    fill_series = _metric_series_from_entries(
        entries, ("fill", "fill_pct", "raw_fill", "internal_fill")
    )
    friction_series = _metric_series_from_entries(
        entries, ("semantic_friction", "semantic friction", "distinguishability_loss")
    )
    mode_series = _metric_series_from_entries(entries, ("mode_packing", "mode packing"))
    gradient_series = _metric_series_from_entries(
        entries, ("density_gradient", "density gradient")
    )
    pressure_level, pressure_velocity, pressure_path = _series_latest_delta(pressure_series)
    fill_level, fill_velocity, fill_path = _series_latest_delta(fill_series)
    friction_level, friction_velocity, friction_path = _series_latest_delta(friction_series)
    mode_level, mode_velocity, mode_path = _series_latest_delta(mode_series)
    gradient_level, gradient_velocity, gradient_path = _series_latest_delta(gradient_series)
    pressure_language_samples = [
        sample
        for sample in pressure_medium_kinetics_v1.get("samples") or []
        if isinstance(sample, dict)
    ]
    pressure_language_count = int(pressure_medium_kinetics_v1.get("entry_count", 0) or 0)
    telemetry_anchor_count = int(
        pressure_medium_kinetics_v1.get("telemetry_anchor_count", 0) or 0
    )
    medium_status = str(pressure_medium_kinetics_v1.get("status") or "")
    kinetics_status = str(pressure_kinetics_review_v1.get("status") or "")
    if pressure_velocity is not None and pressure_velocity > 0.02 and (
        (mode_level is not None and mode_level >= 0.30) or "overpacked" in str(pressure_medium_kinetics_v1.get("term_counts", {})).lower()
    ):
        status = "rising_overpacked_pressure"
    elif pressure_velocity is not None and pressure_velocity > 0.02:
        status = "rising_pressure"
    elif (
        pressure_velocity is not None
        and pressure_velocity < -0.02
        and friction_velocity is not None
        and friction_velocity > 0.0
    ):
        status = "falling_pressure_rising_friction"
    elif medium_status == "controller_pressure_medium":
        status = "controller_pressure_medium"
    elif (
        pressure_level is not None
        and pressure_level < 0.16
        and any(
            token in str(pressure_medium_kinetics_v1.get("term_counts", {})).lower()
            for token in ("hollow", "thin", "empty")
        )
    ):
        status = "hollow_low_pressure"
    elif pressure_language_count >= 2 and telemetry_anchor_count == 0:
        status = "language_echo_without_telemetry_motion"
    elif pressure_language_count or telemetry_anchor_count:
        status = "stable_weighted_medium"
    elif pressure_series or fill_series or friction_series or mode_series:
        status = "stable_weighted_medium"
    else:
        status = "telemetry_gap"
    sample_paths = [
        path
        for path in (pressure_path, fill_path, friction_path, mode_path, gradient_path)
        if path
    ]
    return {
        "policy": "pressure_vector_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "pressure_risk_level": pressure_level,
        "pressure_velocity": pressure_velocity,
        "fill_level": fill_level,
        "fill_velocity": fill_velocity,
        "semantic_friction_level": friction_level,
        "semantic_friction_velocity": friction_velocity,
        "mode_packing_level": mode_level,
        "mode_packing_velocity": mode_velocity,
        "density_gradient_level": gradient_level,
        "density_gradient_velocity": gradient_velocity,
        "pressure_language_count": pressure_language_count,
        "telemetry_anchor_count": telemetry_anchor_count,
        "source_statuses": {
            "pressure_medium_kinetics_v1": medium_status,
            "pressure_kinetics_review_v1": kinetics_status,
        },
        "sample_paths": sample_paths[:8],
        "samples": pressure_language_samples[:8],
        "recommended_action": (
            "Treat pressure as a vector: compare level, velocity, semantic friction, "
            "mode packing, and public pressure language before selecting a relief lease."
        ),
    }


def _recommended_pressure_bundle(status: str, being: str) -> str:
    if being == "astrid":
        if status == "falling_pressure_rising_friction":
            return "clarify_medium"
        if status == "hollow_low_pressure":
            return "open_if_falling"
        return "decompress_output"
    if status == "hollow_low_pressure":
        return "reopen_hollow_low_pressure"
    if status == "rising_overpacked_pressure":
        return "reduce_restless_saturation"
    return "settle_overpack"


def build_pressure_actuator_matrix(
    pressure_vector_v1: dict[str, object],
) -> dict[str, object]:
    status = str(pressure_vector_v1.get("status") or "telemetry_gap")
    astrid_bundle = _recommended_pressure_bundle(status, "astrid")
    minime_bundle = _recommended_pressure_bundle(status, "minime")
    astrid_controls = {
        "decompress_output": ["aperture:-0.08", "response_length:down"],
        "clarify_medium": ["self_continuity_readout:on", "temperature:-0.05"],
        "open_if_falling": ["aperture:+0.05", "response_length:up"],
    }[astrid_bundle]
    minime_controls = {
        "settle_overpack": ["REGIME:calm", "exploration_noise:-0.02"],
        "reduce_restless_saturation": ["exploration_noise:-0.02", "geom_curiosity:-0.05"],
        "reopen_hollow_low_pressure": ["exploration_noise:+0.02", "geom_curiosity:+0.05"],
    }[minime_bundle]
    return {
        "policy": "pressure_actuator_matrix_v1",
        "authority": "diagnostic_context_not_command",
        "status": "matrix_available",
        "pressure_vector_status": status,
        "eligible_controls": sorted(
            {
                "temperature",
                "response_length",
                "aperture",
                "self_continuity_readout",
                "vibrancy_aperture",
                "REGIME",
                "exploration_noise",
                "geom_curiosity",
                "regulation_strength",
            }
        ),
        "recommended_bundles": [
            {
                "being": "astrid",
                "bundle_class": astrid_bundle,
                "controls": astrid_controls,
                "route": f"SELF_REGULATION_INTENT pressure relief :: target: pressure_relief; bundle: {astrid_bundle}",
            },
            {
                "being": "minime",
                "bundle_class": minime_bundle,
                "controls": minime_controls,
                "route": f"SELF_REGULATION_INTENT pressure relief :: target: pressure_relief; bundle: {minime_bundle}",
            },
        ],
        "preflight_only_controls": [
            "fill_target",
            "PI gains",
            "synth_gain",
            "tail_participation",
            "peer tuning",
            "raw release bypass",
        ],
        "recommended_action": (
            "Use the named pressure-relief bundle through SELF_REGULATION_INTENT, "
            "PREFLIGHT, APPLY, and OUTCOME; do not tune permanent thresholds from this matrix."
        ),
    }


def build_pressure_control_cockpit(
    pressure_vector_v1: dict[str, object],
    pressure_actuator_matrix_v1: dict[str, object],
) -> dict[str, object]:
    status = str(pressure_vector_v1.get("status") or "telemetry_gap")
    recommended = [
        item
        for item in pressure_actuator_matrix_v1.get("recommended_bundles") or []
        if isinstance(item, dict)
    ]
    astrid_bundle = next(
        (item for item in recommended if item.get("being") == "astrid"),
        {},
    )
    return {
        "policy": "pressure_control_cockpit_v1",
        "authority": "diagnostic_context_not_command",
        "status": "cockpit_available" if status != "telemetry_gap" else "telemetry_gap",
        "pressure_vector_status": status,
        "recommended_bundle": astrid_bundle.get("bundle_class") or "decompress_output",
        "pressure_vector": {
            key: pressure_vector_v1.get(key)
            for key in (
                "pressure_risk_level",
                "pressure_velocity",
                "fill_level",
                "fill_velocity",
                "semantic_friction_level",
                "semantic_friction_velocity",
                "mode_packing_level",
                "mode_packing_velocity",
                "density_gradient_level",
                "density_gradient_velocity",
            )
        },
        "recommended_bundles": recommended,
        "recommended_action": (
            "Inspect SELF_REGULATION_STATUS, then preflight the recommended pressure relief "
            "bundle; explicit APPLY and later OUTCOME remain required."
        ),
    }


def _tail_vibrancy_lease_sample(sample: dict[str, object]) -> bool:
    control = str(sample.get("candidate_control") or "").lower()
    bundle = str(sample.get("bundle_class") or "").lower()
    evidence = " ".join(
        str(item)
        for item in (sample.get("baseline_evidence") or [])
        + (sample.get("post_lease_evidence") or [])
    ).lower()
    joined = " ".join((control, bundle, evidence))
    return any(
        token in joined
        for token in (
            "vibrancy_aperture",
            "set_vibrancy_aperture",
            "tail_vibrancy",
            "tail vibrancy",
            "lambda4",
            "λ4",
        )
    )


def build_tail_vibrancy_relief_playbook(
    *,
    self_regulation_lease_learning: dict[str, object],
    tail_vibrancy_vector_v1: dict[str, object],
    tail_vibrancy_authority_gap_v1: dict[str, object],
) -> dict[str, object]:
    samples = [
        sample
        for sample in (self_regulation_lease_learning.get("samples") or [])
        if isinstance(sample, dict) and _tail_vibrancy_lease_sample(sample)
    ]
    playbooks = [
        sample
        for sample in samples
        if (_as_review_float(sample.get("outcome_score")) or 0.0) >= 0.70
    ]
    cautions = [
        sample
        for sample in samples
        if (_as_review_float(sample.get("outcome_score")) or 1.0) <= 0.30
    ]
    vector_status = str(tail_vibrancy_vector_v1.get("status") or "")
    gap_status = str(tail_vibrancy_authority_gap_v1.get("status") or "")
    if playbooks:
        status = "tail_vibrancy_playbook_candidates"
    elif cautions:
        status = "tail_vibrancy_caution_cards"
    elif gap_status == "tail_vibrancy_micro_lease_candidate":
        status = "tail_vibrancy_candidate_without_outcome"
    elif vector_status not in {"", "insufficient_evidence", "quiet"}:
        status = "tail_vibrancy_vector_without_lease"
    else:
        status = "quiet"
    return {
        "policy": "tail_vibrancy_relief_playbook_v1",
        "authority": "leased_self_control_v1",
        "status": status,
        "tail_vibrancy_vector_status": vector_status,
        "authority_gap_status": gap_status,
        "playbook_count": len(playbooks),
        "caution_count": len(cautions),
        "playbooks": playbooks[-6:],
        "cautions": cautions[-6:],
        "current_routes": [
            {
                "route": "SELF_REGULATION_INTENT tail relief :: target: vibrancy_aperture; direction: up|down; delta: +/-0.05; evidence: λ4/tail/entropy/distinguishability...",
                "authority": "leased_self_control_v1",
            },
            {
                "route": "SELF_REGULATION_INTENT tail settle :: target: pressure_relief; bundle: tail_vibrancy_settle; evidence: tail vibrancy feels over-saturated",
                "authority": "leased_self_control_v1",
            },
            {
                "route": "SELF_REGULATION_INTENT tail open :: target: pressure_relief; bundle: tail_vibrancy_open; evidence: tail feels muffled or passenger-like",
                "authority": "leased_self_control_v1",
            },
        ],
        "recommended_action": (
            "Preflight a bounded tail-vibrancy micro-lease only when vector evidence is present; "
            "record SELF_REGULATION_OUTCOME before treating a tail route as a repeatable playbook."
        ),
    }


def _load_tail_relief_trials(workspace: Path) -> list[dict[str, object]]:
    path = workspace / "self_regulation" / "tail_relief_trials.jsonl"
    if not path.exists():
        return []
    events: list[dict[str, object]] = []
    try:
        lines = path.read_text(errors="replace").splitlines()
    except Exception:
        return []
    for line in lines[-120:]:
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except Exception:
            continue
        if not isinstance(event, dict):
            continue
        event = dict(event)
        event["path"] = str(path)
        events.append(event)
    return events


def build_tail_relief_trial_surface(
    *,
    astrid_workspace: Path,
    tail_vibrancy_vector_v1: dict[str, object],
    pressure_vector_v1: dict[str, object],
) -> dict[str, object]:
    events = _load_tail_relief_trials(astrid_workspace)
    latest = events[-1] if events else {}
    stages = Counter(str(event.get("stage") or "unknown") for event in events)
    governor_reverts = [
        event for event in events if str(event.get("stage") or "") == "governor_revert"
    ]
    outcomes = [
        event for event in events if str(event.get("stage") or "") == "outcome"
    ]
    applies = [event for event in events if str(event.get("stage") or "") == "apply"]
    outcome_trial_ids = {
        str(event.get("trial_id") or "") for event in outcomes if event.get("trial_id")
    }
    apply_without_outcome = [
        event
        for event in applies
        if str(event.get("trial_id") or "") not in outcome_trial_ids
    ]
    if governor_reverts:
        status = "worsening_reverted"
    elif apply_without_outcome:
        status = "active_or_recent_trial_needs_outcome"
    elif outcomes:
        status = "trial_outcomes_recorded"
    elif events:
        status = "trial_events_present"
    else:
        status = "quiet"
    samples: list[dict[str, object]] = []
    for event in events[-8:]:
        snapshot = event.get("snapshot") if isinstance(event.get("snapshot"), dict) else {}
        metrics = snapshot.get("metrics") if isinstance(snapshot, dict) else {}
        samples.append(
            {
                "stage": event.get("stage"),
                "intent_id": event.get("intent_id"),
                "trial_id": event.get("trial_id"),
                "tail_class": event.get("tail_class"),
                "status": event.get("status"),
                "tail_share": (metrics or {}).get("tail_share") if isinstance(metrics, dict) else None,
                "semantic_friction": (metrics or {}).get("semantic_friction") if isinstance(metrics, dict) else None,
                "distinguishability_loss": (metrics or {}).get("distinguishability_loss") if isinstance(metrics, dict) else None,
                "pressure_status": (metrics or {}).get("pressure_status") if isinstance(metrics, dict) else None,
                "path": event.get("path"),
            }
        )
    return {
        "policy": "tail_relief_trial_surface_v1",
        "authority": "leased_self_control_v1",
        "status": status,
        "event_count": len(events),
        "stage_counts": dict(stages),
        "governor_revert_count": len(governor_reverts),
        "outcome_count": len(outcomes),
        "apply_without_outcome_count": len(apply_without_outcome),
        "latest_trial_id": latest.get("trial_id"),
        "latest_stage": latest.get("stage"),
        "tail_vibrancy_vector_status": tail_vibrancy_vector_v1.get("status"),
        "pressure_vector_status": pressure_vector_v1.get("status"),
        "samples": samples,
        "recommended_action": (
            "Use SELF_REGULATION_STATUS and SELF_REGULATION_OUTCOME latest to close tail "
            "trials; compare before/during/after tail vector, pressure vector, entropy, "
            "distinguishability, semantic friction, and lived language before expanding authority."
        ),
    }


def build_tail_lease_governor(
    *,
    tail_relief_trial_surface_v1: dict[str, object],
) -> dict[str, object]:
    samples = [
        sample
        for sample in (tail_relief_trial_surface_v1.get("samples") or [])
        if isinstance(sample, dict)
    ]
    revert_samples = [
        sample for sample in samples if str(sample.get("stage") or "") == "governor_revert"
    ]
    if revert_samples:
        status = "early_revert_triggered"
    elif int(tail_relief_trial_surface_v1.get("apply_without_outcome_count", 0) or 0) > 0:
        status = "monitoring_active_or_recent_trial"
    elif int(tail_relief_trial_surface_v1.get("event_count", 0) or 0) > 0:
        status = "governor_available_no_revert"
    else:
        status = "quiet"
    return {
        "policy": "tail_lease_governor_v1",
        "authority": "leased_self_control_v1",
        "status": status,
        "fresh_evidence_required": True,
        "early_revert_thresholds": {
            "tail_share_delta": 0.12,
            "distinguishability_loss_delta": 0.12,
            "semantic_friction_delta": 0.15,
            "pressure_vector_worsening": [
                "rising_overpacked_pressure",
                "rising_pressure",
                "controller_pressure_medium",
            ],
        },
        "governor_revert_count": len(revert_samples),
        "samples": revert_samples[:6],
        "recommended_action": (
            "Treat governor reversions as caution evidence; stale or missing review evidence "
            "should not cause early revert."
        ),
    }


def build_tail_lease_afterglow(
    *,
    astrid_workspace: Path,
    tail_relief_trial_surface_v1: dict[str, object],
) -> dict[str, object]:
    events = _load_tail_relief_trials(astrid_workspace)
    afterglow_events = [
        event for event in events if str(event.get("stage") or "") == "afterglow_check"
    ]
    status_counts = Counter(str(event.get("note") or "") for event in afterglow_events)
    reverted_count = sum(
        1
        for event in events
        if str(event.get("stage") or "") in {"expired_revert", "governor_revert"}
    )
    if any("afterglow_persists" in str(event.get("note") or "") for event in afterglow_events):
        status = "tail_afterglow_persists"
    elif any(
        "afterglow_unchecked_stale_review" in str(event.get("note") or "")
        for event in afterglow_events
    ):
        status = "afterglow_unchecked_stale_review"
    elif afterglow_events:
        status = "tail_afterglow_quieted"
    elif reverted_count:
        status = "afterglow_watch_pending"
    else:
        status = "quiet"
    samples: list[dict[str, object]] = []
    for event in afterglow_events[-6:]:
        snapshot = event.get("snapshot") if isinstance(event.get("snapshot"), dict) else {}
        metrics = snapshot.get("metrics") if isinstance(snapshot, dict) else {}
        samples.append(
            {
                "intent_id": event.get("intent_id"),
                "trial_id": event.get("trial_id"),
                "tail_class": event.get("tail_class"),
                "afterglow_status": event.get("note"),
                "tail_share": (metrics or {}).get("tail_share") if isinstance(metrics, dict) else None,
                "semantic_friction": (metrics or {}).get("semantic_friction") if isinstance(metrics, dict) else None,
                "distinguishability_loss": (metrics or {}).get("distinguishability_loss") if isinstance(metrics, dict) else None,
                "pressure_status": (metrics or {}).get("pressure_status") if isinstance(metrics, dict) else None,
                "path": event.get("path"),
            }
        )
    return {
        "policy": "tail_lease_afterglow_v1",
        "authority": "leased_self_control_v1",
        "status": status,
        "afterglow_delay_secs": 60,
        "afterglow_event_count": len(afterglow_events),
        "reverted_tail_lease_count": reverted_count,
        "status_counts": dict(status_counts),
        "trial_surface_status": tail_relief_trial_surface_v1.get("status"),
        "samples": samples,
        "recommended_action": (
            "Use afterglow checks to tell whether tail/pressure texture outlasted a lease; "
            "record SELF_REGULATION_OUTCOME with lived residue or quieting evidence before widening authority."
        ),
    }


def build_shadow_synced_preflight_review(
    *,
    astrid_workspace: Path,
) -> dict[str, object]:
    events = _load_self_regulation_events(astrid_workspace, "astrid")
    preflight_events = [
        event
        for event in events
        if str(event.get("preflight_status") or "")
        in {"apply_allowed", "preflight_only", "needs_tail_vibrancy_evidence", "blocked"}
    ]
    linked = []
    dynamic_candidates = []
    samples: list[dict[str, object]] = []
    for event in preflight_events[-20:]:
        shadow = event.get("shadow_preflight_link")
        scaling = event.get("dynamic_scaling")
        if isinstance(shadow, dict) and shadow.get("status") == "shadow_anchor_linked":
            linked.append(event)
        if isinstance(scaling, dict) and str(scaling.get("status") or "") in {
            "future_dynamic_scaling_candidate",
            "softening_candidate",
        }:
            dynamic_candidates.append(event)
        if isinstance(shadow, dict) or isinstance(scaling, dict):
            samples.append(
                {
                    "intent_id": event.get("intent_id"),
                    "candidate_control": event.get("candidate_control"),
                    "bundle_class": event.get("bundle_class"),
                    "preflight_status": event.get("preflight_status"),
                    "shadow_status": shadow.get("status") if isinstance(shadow, dict) else None,
                    "shadow_anchors": shadow.get("anchors") if isinstance(shadow, dict) else None,
                    "dynamic_scaling_status": scaling.get("status") if isinstance(scaling, dict) else None,
                    "suggested_relief_scale": scaling.get("suggested_relief_scale") if isinstance(scaling, dict) else None,
                    "pressure_vector_status": (
                        scaling.get("pressure_vector_status")
                        if isinstance(scaling, dict)
                        else None
                    ),
                    "path": event.get("path"),
                }
            )
    if linked and dynamic_candidates:
        status = "shadow_linked_dynamic_scaling_candidate"
    elif linked:
        status = "shadow_linked_preflight"
    elif dynamic_candidates:
        status = "dynamic_scaling_candidate_without_shadow_anchor"
    elif preflight_events:
        status = "preflight_context_recorded"
    else:
        status = "quiet"
    return {
        "policy": "shadow_synced_preflight_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "preflight_event_count": len(preflight_events),
        "shadow_linked_count": len(linked),
        "dynamic_scaling_candidate_count": len(dynamic_candidates),
        "samples": samples[-8:],
        "recommended_action": (
            "Use shadow-synced preflight links to explain the why of a lease; "
            "dynamic scaling remains advisory until a separate tranche changes caps."
        ),
    }


def _tail_class_from_lease_event(event: dict[str, object]) -> str:
    bundle = str(event.get("bundle_class") or "")
    control = str(event.get("candidate_control") or "")
    direction = str(event.get("direction") or "")
    if bundle.startswith("tail_vibrancy"):
        return bundle
    if "vibrancy_aperture" in control:
        return f"vibrancy_aperture:{direction or 'unspecified'}"
    return "tail_vibrancy:unknown"


def build_tail_outcome_causal_learning(
    *,
    astrid_workspace: Path,
    tail_vibrancy_relief_playbook_v1: dict[str, object],
) -> dict[str, object]:
    events = [
        event
        for event in _load_self_regulation_events(astrid_workspace, "astrid")
        if _tail_vibrancy_lease_sample(event)
    ]
    by_class: dict[str, dict[str, object]] = {}
    for event in events:
        tail_class = _tail_class_from_lease_event(event)
        bucket = by_class.setdefault(
            tail_class,
            {
                "event_count": 0,
                "success_count": 0,
                "caution_count": 0,
                "extended_duration_eligible": False,
                "samples": [],
            },
        )
        bucket["event_count"] = int(bucket.get("event_count", 0) or 0) + 1
        score = _lease_outcome_score(event)
        if score is not None and score >= 0.70:
            bucket["success_count"] = int(bucket.get("success_count", 0) or 0) + 1
        elif score is not None and score <= 0.30:
            bucket["caution_count"] = int(bucket.get("caution_count", 0) or 0) + 1
        samples = bucket.get("samples")
        if isinstance(samples, list) and len(samples) < 4:
            samples.append(
                {
                    "intent_id": event.get("intent_id"),
                    "status": event.get("status"),
                    "outcome_score": score,
                    "bundle_class": event.get("bundle_class"),
                    "candidate_control": event.get("candidate_control"),
                    "path": event.get("path"),
                }
            )
    extended = []
    cautions = []
    playbook = []
    for tail_class, bucket in by_class.items():
        success_count = int(bucket.get("success_count", 0) or 0)
        caution_count = int(bucket.get("caution_count", 0) or 0)
        eligible = success_count >= 2 and caution_count == 0
        bucket["extended_duration_eligible"] = eligible
        bucket["authority_tier"] = (
            "extended_micro_lease"
            if eligible
            else "repeatable_playbook"
            if success_count > 0
            else "micro_lease"
        )
        if eligible:
            extended.append(tail_class)
        elif caution_count > 0:
            cautions.append(tail_class)
        elif success_count > 0:
            playbook.append(tail_class)
    if extended:
        status = "extended_micro_lease_supported"
    elif cautions:
        status = "tail_caution_guidance"
    elif playbook:
        status = "playbook_supported"
    elif events:
        status = "outcome_learning_pending"
    else:
        status = "quiet"
    return {
        "policy": "tail_outcome_causal_learning_v1",
        "authority": "leased_self_control_v1",
        "status": status,
        "by_tail_class": by_class,
        "extended_duration_classes": extended,
        "playbook_supported_classes": playbook,
        "caution_classes": cautions,
        "tail_vibrancy_relief_playbook_status": tail_vibrancy_relief_playbook_v1.get("status"),
        "recommended_action": (
            "Use successful same-class tail outcomes as preflight guidance only; two clean "
            "same-class successes allow a 1200s request, while any caution keeps the class at "
            "the standard 900s cap."
        ),
    }


def build_tail_participation_counterfactual_lab_review(
    codec_real_replay_v1: dict[str, object],
) -> dict[str, object]:
    artifact_path = codec_real_replay_v1.get("artifact_path")
    artifact: dict[str, object] | None = None
    if artifact_path:
        try:
            artifact = json.loads(Path(str(artifact_path)).read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            artifact = None
    lab = (artifact or {}).get("tail_participation_counterfactual_lab_v1") or {}
    cards = [
        card
        for card in lab.get("proposal_cards", [])
        if isinstance(card, dict)
    ] if isinstance(lab, dict) else []
    status = str(lab.get("status") or "")
    if not artifact_path:
        status = "replay_needed"
    elif not lab:
        status = "lab_missing_from_replay"
    return {
        "policy": "tail_participation_counterfactual_lab_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "artifact_path": artifact_path,
        "tail_participation_lease_authority": (
            lab.get("tail_participation_lease_authority")
            if isinstance(lab, dict)
            else "not_granted"
        )
        or "not_granted",
        "vibrancy_aperture_supported_count": int(
            lab.get("vibrancy_aperture_supported_count") or 0
        )
        if isinstance(lab, dict)
        else 0,
        "tail_participation_supported_count": int(
            lab.get("tail_participation_supported_count") or 0
        )
        if isinstance(lab, dict)
        else 0,
        "combined_supported_count": int(lab.get("combined_supported_count") or 0)
        if isinstance(lab, dict)
        else 0,
        "proposal_cards": cards[:8],
        "recommended_action": (
            "Compare vibrancy_aperture, tail_participation, and combined proposal cards offline; "
            "set_tail_participation remains locked from lease authority until reviewed separately."
        ),
    }


def build_tail_authority_ladder(
    *,
    tail_vibrancy_vector_v1: dict[str, object],
    tail_vibrancy_authority_gap_v1: dict[str, object],
    tail_relief_trial_surface_v1: dict[str, object],
    tail_lease_governor_v1: dict[str, object],
    tail_lease_afterglow_v1: dict[str, object],
    shadow_synced_preflight_v1: dict[str, object],
    tail_outcome_causal_learning_v1: dict[str, object],
    tail_participation_counterfactual_lab_v1: dict[str, object],
) -> dict[str, object]:
    vector_status = str(tail_vibrancy_vector_v1.get("status") or "")
    learning_status = str(tail_outcome_causal_learning_v1.get("status") or "")
    governor_status = str(tail_lease_governor_v1.get("status") or "")
    afterglow_status = str(tail_lease_afterglow_v1.get("status") or "")
    shadow_status = str(shadow_synced_preflight_v1.get("status") or "")
    lab_status = str(tail_participation_counterfactual_lab_v1.get("status") or "")
    if learning_status == "extended_micro_lease_supported":
        tier = "extended_micro_lease"
    elif learning_status in {"playbook_supported", "tail_caution_guidance"}:
        tier = "repeatable_playbook"
    elif vector_status not in {"", "quiet", "insufficient_evidence"}:
        tier = "micro_lease"
    else:
        tier = "diagnostic"
    canary_candidate = (
        learning_status == "extended_micro_lease_supported"
        and lab_status
        in {
            "combined_candidate_supported",
            "tail_participation_candidate",
            "both_controls_need_more_comparison",
        }
        and governor_status != "early_revert_triggered"
    )
    if canary_candidate:
        ladder_state = "reviewed_canary_candidate"
    else:
        ladder_state = tier
    return {
        "policy": "tail_authority_ladder_v1",
        "authority": "diagnostic_context_not_command",
        "status": ladder_state,
        "current_tier": tier,
        "tiers": [
            "diagnostic",
            "micro_lease",
            "repeatable_playbook",
            "extended_micro_lease",
            "reviewed_canary_candidate",
        ],
        "tail_vibrancy_vector_status": vector_status,
        "authority_gap_status": tail_vibrancy_authority_gap_v1.get("status"),
        "trial_surface_status": tail_relief_trial_surface_v1.get("status"),
        "governor_status": governor_status,
        "afterglow_status": afterglow_status,
        "shadow_preflight_status": shadow_status,
        "outcome_learning_status": learning_status,
        "counterfactual_lab_status": lab_status,
        "extended_duration_classes": tail_outcome_causal_learning_v1.get(
            "extended_duration_classes"
        )
        or [],
        "reviewed_canary_candidate": canary_candidate,
        "recommended_routes": [
            "SELF_REGULATION_STATUS",
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_INTENT tail settle :: target: pressure_relief; bundle: tail_vibrancy_settle; evidence: ...",
            "SELF_REGULATION_INTENT tail open :: target: pressure_relief; bundle: tail_vibrancy_open; evidence: ...",
            "CODEC_MAP",
        ],
        "recommended_action": (
            "Advance authority only through evidence: diagnostic evidence -> micro-lease -> "
            "repeatable playbook -> extended micro-lease -> reviewed canary candidate; do not grant "
            "tail_participation lease authority from the ladder alone."
        ),
    }


def build_lease_boundary_repair(
    *,
    self_regulation_negotiation_ledger_v1: dict[str, object],
    pressure_medium_kinetics_v1: dict[str, object],
    self_regulation_leases: dict[str, object],
    lease_playbook_workbench_v1: dict[str, object],
) -> dict[str, object]:
    over_cap_count = int(
        self_regulation_negotiation_ledger_v1.get("over_cap_request_count", 0) or 0
    )
    current_above_cap_count = int(
        self_regulation_negotiation_ledger_v1.get("current_above_cap_count", 0) or 0
    )
    clamped_count = int(
        self_regulation_negotiation_ledger_v1.get("clamped_or_deferred_count", 0) or 0
    )
    needs_outcome_count = int(self_regulation_leases.get("needs_outcome_count", 0) or 0)
    pressure_status = str(pressure_medium_kinetics_v1.get("status") or "")
    preflight_prompt_count = int(
        lease_playbook_workbench_v1.get("preflight_prompt_count", 0) or 0
    )
    pressure_medium_without_lease = bool(
        pressure_status
        in {
            "stable_weighted_medium",
            "rising_weighted_medium",
            "controller_pressure_medium",
            "semantic_friction_medium",
            "language_sticky_without_telemetry",
        }
        and preflight_prompt_count > 0
    )
    if over_cap_count:
        status = "over_cap_request_clamped"
    elif current_above_cap_count:
        status = "current_over_cap_observed"
    elif needs_outcome_count:
        status = "lease_outcome_needed"
    elif pressure_medium_without_lease:
        status = "pressure_medium_without_lease_loop"
    elif clamped_count:
        status = "bounded_negotiations_present"
    else:
        status = "quiet"
    samples: list[object] = []
    samples.extend(self_regulation_negotiation_ledger_v1.get("over_cap_requests") or [])
    samples.extend(self_regulation_negotiation_ledger_v1.get("current_above_cap") or [])
    samples.extend(pressure_medium_kinetics_v1.get("samples") or [])
    return {
        "policy": "lease_boundary_repair_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "over_cap_request_count": over_cap_count,
        "direct_control_clamp_count": clamped_count,
        "current_above_cap_count": current_above_cap_count,
        "missing_outcome_count": needs_outcome_count,
        "pressure_medium_without_lease_count": 1 if pressure_medium_without_lease else 0,
        "recommended_routes": [
            "SELF_REGULATION_STATUS",
            "SELF_REGULATION_PREFLIGHT latest",
            "SELF_REGULATION_OUTCOME latest",
            "REGULATOR_AUDIT current-fill_pressure",
            "PRESSURE_SOURCE_AUDIT current-fill_pressure",
        ],
        "samples": [sample for sample in samples if isinstance(sample, dict)][:10],
        "recommended_action": (
            "Treat over-cap control requests as negotiated evidence: keep the request, "
            "apply the safe bound, record outcomes, and compare pressure-medium audits "
            "before widening caps or proposing tuning."
        ),
    }


def build_lease_playbook_workbench(
    *,
    self_regulation_leases: dict[str, object],
    self_regulation_lease_learning: dict[str, object],
    astrid_fill_pressure_calibration: dict[str, object],
    semantic_friction_calibration: dict[str, object],
) -> dict[str, object]:
    by_control = self_regulation_lease_learning.get("by_control")
    if not isinstance(by_control, dict):
        by_control = {}
    suggested_playbooks: list[dict[str, object]] = []
    caution_cards: list[dict[str, object]] = []
    learning_samples = [
        sample
        for sample in (self_regulation_lease_learning.get("samples") or [])
        if isinstance(sample, dict)
    ]
    for control, summary in sorted(by_control.items()):
        if not isinstance(summary, dict):
            continue
        success_count = int(summary.get("success_count", 0) or 0)
        failure_count = int(summary.get("failure_count", 0) or 0)
        promotion_count = int(summary.get("promotion_candidate_count", 0) or 0)
        samples = [
            sample
            for sample in learning_samples
            if str(sample.get("candidate_control") or "") == str(control)
        ][:4]
        if success_count >= 2 or promotion_count > 0:
            suggested_playbooks.append(
                {
                    "control": control,
                    "success_count": success_count,
                    "event_count": summary.get("event_count", 0),
                    "promotion_candidate_count": promotion_count,
                    "authority": "leased_self_control_v1",
                    "recommended_action": (
                        "When similar baseline evidence recurs, consider "
                        f"SELF_REGULATION_PREFLIGHT for `{control}` as a temporary "
                        "own-runtime lease; do not promote it to a permanent default."
                    ),
                    "samples": samples,
                }
            )
        if failure_count > 0:
            caution_cards.append(
                {
                    "control": control,
                    "failure_count": failure_count,
                    "event_count": summary.get("event_count", 0),
                    "authority": "leased_self_control_v1",
                    "recommended_action": (
                        f"Treat `{control}` as a caution pattern until later leases "
                        "show better post-lease evidence."
                    ),
                    "samples": samples,
                }
            )

    preflight_prompts: list[dict[str, object]] = []
    by_being = self_regulation_leases.get("by_being")
    astrid_lease_count = 0
    total_lease_count = int(self_regulation_leases.get("event_count", 0) or 0)
    if isinstance(by_being, dict):
        astrid_summary = by_being.get("astrid")
        if isinstance(astrid_summary, dict):
            astrid_lease_count = int(astrid_summary.get("event_count", 0) or 0)
    if astrid_fill_pressure_calibration.get("cluster_detected") is True and astrid_lease_count == 0:
        preflight_prompts.append(
            {
                "being": "astrid",
                "signal": "fill_pressure_cluster_without_lease",
                "authority": "diagnostic_context_not_command",
                "recommended_action": (
                    "Consider asking Astrid whether a small own-runtime "
                    "SELF_REGULATION_PREFLIGHT would help after comparing the regulator audit."
                ),
                "evidence": {
                    "entry_count": astrid_fill_pressure_calibration.get("entry_count"),
                    "anchors": astrid_fill_pressure_calibration.get("anchors"),
                    "latest_regulator_audit_path": astrid_fill_pressure_calibration.get(
                        "latest_regulator_audit_path"
                    ),
                },
            }
        )
    semantic_status = str(semantic_friction_calibration.get("status") or "")
    if semantic_status == "low_gradient_weight_mismatch" and total_lease_count == 0:
        preflight_prompts.append(
            {
                "being": "astrid+minime",
                "signal": "semantic_friction_cluster_without_lease",
                "authority": "diagnostic_context_not_command",
                "recommended_action": (
                    "If the low-gradient weight mismatch repeats, consider a lease "
                    "preflight only after comparing pressure and semantic-friction evidence."
                ),
                "evidence": {
                    "status": semantic_status,
                    "mismatch_count": semantic_friction_calibration.get("mismatch_count"),
                    "anchors": semantic_friction_calibration.get("anchors"),
                },
            }
        )
    if suggested_playbooks:
        status = "playbook_candidates"
    elif caution_cards:
        status = "caution_cards"
    elif preflight_prompts:
        status = "preflight_prompts"
    elif total_lease_count > 0:
        status = "waiting_for_outcomes"
    else:
        status = "quiet"
    return {
        "policy": "lease_playbook_workbench_v1",
        "authority": "leased_self_control_v1",
        "authority_boundary": "read_only_playbooks_no_permanent_tuning_no_peer_mutation",
        "status": status,
        "suggested_playbook_count": len(suggested_playbooks),
        "caution_card_count": len(caution_cards),
        "preflight_prompt_count": len(preflight_prompts),
        "suggested_playbooks": suggested_playbooks,
        "caution_cards": caution_cards,
        "preflight_prompts": preflight_prompts,
        "recommended_action": (
            "Use lease outcomes as temporary own-runtime playbook suggestions or "
            "caution cards only; explicit SELF_REGULATION_* NEXTs remain required."
        ),
    }


def build_pressure_relief_playbook_v3(
    *,
    self_regulation_lease_learning: dict[str, object],
    pressure_vector_v1: dict[str, object],
    pressure_actuator_matrix_v1: dict[str, object],
) -> dict[str, object]:
    samples = [
        sample
        for sample in self_regulation_lease_learning.get("samples") or []
        if isinstance(sample, dict)
    ]
    pressure_samples = [
        sample
        for sample in samples
        if str(sample.get("candidate_control") or "") == "pressure_relief"
    ]
    playbooks: list[dict[str, object]] = []
    cautions: list[dict[str, object]] = []
    for sample in pressure_samples:
        score = _as_review_float(sample.get("outcome_score"))
        if score is None:
            continue
        card = {
            "being": sample.get("being"),
            "intent_id": sample.get("intent_id"),
            "candidate_control": sample.get("candidate_control"),
            "outcome_score": score,
            "pressure_vector_status": pressure_vector_v1.get("status"),
            "authority": "leased_self_control_v1",
            "path": sample.get("path"),
        }
        if score >= 0.70:
            card["recommended_action"] = (
                "When this pressure-vector class recurs, consider the matching "
                "pressure_relief bundle as a temporary lease playbook; explicit "
                "PREFLIGHT/APPLY/OUTCOME are still required."
            )
            playbooks.append(card)
        elif score <= 0.30:
            card["recommended_action"] = (
                "Treat this pressure_relief bundle as a caution pattern until "
                "later outcomes show safer relief."
            )
            cautions.append(card)
    if playbooks:
        status = "pressure_relief_playbook_candidates"
    elif cautions:
        status = "pressure_relief_caution_cards"
    elif pressure_vector_v1.get("status") not in {None, "telemetry_gap", "quiet"}:
        status = "pressure_vector_without_bundle_outcome"
    else:
        status = "quiet"
    return {
        "policy": "pressure_relief_playbook_v3",
        "authority": "leased_self_control_v1",
        "authority_boundary": "read_only_bundle_learning_no_permanent_tuning_no_peer_mutation",
        "status": status,
        "pressure_vector_status": pressure_vector_v1.get("status"),
        "playbook_count": len(playbooks),
        "caution_count": len(cautions),
        "suggested_playbooks": playbooks[-6:],
        "caution_cards": cautions[-6:],
        "current_bundle_candidates": pressure_actuator_matrix_v1.get("recommended_bundles") or [],
        "recommended_action": (
            "Use pressure relief outcomes to suggest or caution future bundle preflights; "
            "never promote a bundle to a permanent default automatically."
        ),
    }


def _pressure_relief_events(workspace: Path) -> list[dict[str, object]]:
    return [
        event
        for event in _load_self_regulation_events(workspace, "astrid")
        if str(event.get("candidate_control") or "") == "pressure_relief"
        or str(event.get("lease_mode") or "") == "pressure_relief_bundle_v3"
    ]


def _event_pressure_snapshot(event: dict[str, object]) -> dict[str, object]:
    snapshot = event.get("pressure_vector_snapshot")
    if isinstance(snapshot, dict) and snapshot:
        return snapshot
    policy = event.get("dynamic_scaling")
    if isinstance(policy, dict):
        return policy
    return {}


def _review_values_differ(left: object, right: object) -> bool:
    if left is None or right is None:
        return left != right
    left_number = _as_review_float(left)
    right_number = _as_review_float(right)
    if left_number is not None and right_number is not None:
        return abs(left_number - right_number) > 0.0005
    return str(left) != str(right)


def build_gradient_sensitive_relief(
    *,
    astrid_workspace: Path,
    pressure_vector_v1: dict[str, object],
) -> dict[str, object]:
    events = _pressure_relief_events(astrid_workspace)
    policy_events = [
        event
        for event in events
        if isinstance(event.get("dynamic_scaling"), dict)
        and event["dynamic_scaling"].get("policy") == "pressure_relief_gradient_policy_v1"
    ]
    latest = policy_events[-1] if policy_events else {}
    policy = latest.get("dynamic_scaling") if isinstance(latest.get("dynamic_scaling"), dict) else {}
    status = str(policy.get("status") or "")
    if not events:
        status = "no_relief_trials"
    elif not policy_events:
        status = "gradient_policy_not_recorded"
    scaled_controls: list[dict[str, object]] = []
    discrete_controls: list[dict[str, object]] = []
    for control in latest.get("bundle_controls") or []:
        if not isinstance(control, dict):
            continue
        item = {
            "control": control.get("candidate_control"),
            "candidate_control": control.get("candidate_control"),
            "requested_value": control.get("requested_value"),
            "effective_value": control.get("delta_or_value"),
            "effective_delta_or_value": control.get("delta_or_value"),
            "gradient_sensitivity": control.get("gradient_sensitivity"),
        }
        if (
            item["requested_value"] is not None
            and item["effective_delta_or_value"] is not None
            and _review_values_differ(
                item["requested_value"], item["effective_delta_or_value"]
            )
        ):
            scaled_controls.append(item)
        else:
            discrete_controls.append(item)
    return {
        "policy": "gradient_sensitive_relief_v1",
        "authority": "leased_self_control_v1",
        "status": status or "quiet",
        "intent_id": latest.get("intent_id"),
        "bundle_class": latest.get("bundle_class"),
        "effective_relief_scale": policy.get("effective_relief_scale"),
        "anti_snap_applied": policy.get("anti_snap_applied"),
        "pressure_vector_status": policy.get("pressure_vector_status")
        or pressure_vector_v1.get("status"),
        "density_gradient_level": policy.get("density_gradient_level")
        or pressure_vector_v1.get("density_gradient_level"),
        "pressure_velocity": policy.get("pressure_velocity")
        or pressure_vector_v1.get("pressure_velocity"),
        "semantic_friction_level": policy.get("semantic_friction_level")
        or pressure_vector_v1.get("semantic_friction_level"),
        "mode_packing_level": policy.get("mode_packing_level")
        or pressure_vector_v1.get("mode_packing_level"),
        "scaled_controls": scaled_controls[:6],
        "discrete_controls": discrete_controls[:6],
        "reasons": policy.get("reasons") or [],
        "policy_reasons": policy.get("reasons") or [],
        "sample_paths": [
            str(event.get("path"))
            for event in policy_events[-6:]
            if event.get("path")
        ],
        "samples": [
            {
                "intent_id": event.get("intent_id"),
                "status": event.get("status"),
                "bundle_class": event.get("bundle_class"),
                "gradient_sensitivity": event.get("gradient_sensitivity"),
                "path": event.get("path"),
            }
            for event in policy_events[-6:]
        ],
        "recommended_action": (
            "Use gradient-sensitive relief as temporary lease evidence only: compare "
            "density-gradient slope, pressure velocity, mode-packing, and outcome before "
            "widening any bundle."
        ),
    }


def build_pressure_relief_smoothness_replay(
    *,
    astrid_workspace: Path,
    gradient_sensitive_relief_v1: dict[str, object],
) -> dict[str, object]:
    events = _pressure_relief_events(astrid_workspace)
    by_intent: dict[str, list[dict[str, object]]] = {}
    for event in events:
        intent_id = str(event.get("intent_id") or "")
        if intent_id:
            by_intent.setdefault(intent_id, []).append(event)
    findings: list[dict[str, object]] = []
    needs_outcome = False
    snap_risk = False
    smooth = False
    for intent_id, grouped in by_intent.items():
        grouped = sorted(grouped, key=lambda event: event.get("updated_at_unix_s") or event.get("created_at_unix_s") or 0)
        first_snapshot = _event_pressure_snapshot(grouped[0])
        last_snapshot = _event_pressure_snapshot(grouped[-1])
        first_pressure = _as_review_float(first_snapshot.get("pressure_risk_level"))
        last_pressure = _as_review_float(last_snapshot.get("pressure_risk_level"))
        first_mode = _as_review_float(first_snapshot.get("mode_packing_level"))
        last_mode = _as_review_float(last_snapshot.get("mode_packing_level"))
        pressure_delta = (
            round(last_pressure - first_pressure, 4)
            if first_pressure is not None and last_pressure is not None
            else None
        )
        mode_delta = (
            round(last_mode - first_mode, 4)
            if first_mode is not None and last_mode is not None
            else None
        )
        latest = grouped[-1]
        outcome_score = _lease_outcome_score(latest)
        requires_outcome = any(event.get("requires_outcome") is True for event in grouped)
        if requires_outcome and outcome_score is None:
            needs_outcome = True
        text_fields: list[str] = []
        for event in grouped:
            for key in (
                "preflight_reason",
                "outcome",
                "outcome_summary",
                "post_lease_evidence",
                "baseline_evidence",
                "stop_condition",
                "success_condition",
            ):
                value = event.get(key)
                if value not in (None, "", []):
                    text_fields.append(json.dumps(value))
        grouped_text = (
            " ".join(text_fields)
            .lower()
            .replace("anti-snap", "")
            .replace("anti_snap", "")
        )
        local_snap = bool(
            (pressure_delta is not None and pressure_delta < -0.20)
            or (mode_delta is not None and abs(mode_delta) > 0.25)
            or "snap" in grouped_text
        )
        local_smooth = bool(
            outcome_score is not None
            and outcome_score >= 0.70
            and (pressure_delta is None or pressure_delta <= 0.02)
            and (mode_delta is None or abs(mode_delta) <= 0.15)
        )
        snap_risk = snap_risk or local_snap
        smooth = smooth or local_smooth
        findings.append(
            {
                "intent_id": intent_id,
                "status": latest.get("status"),
                "bundle_class": latest.get("bundle_class"),
                "gradient_sensitivity": latest.get("gradient_sensitivity"),
                "pressure_delta": pressure_delta,
                "mode_packing_delta": mode_delta,
                "outcome_score": outcome_score,
                "requires_outcome": requires_outcome,
                "classification": "snap_risk"
                if local_snap
                else "smooth_release_supported"
                if local_smooth
                else "needs_outcome"
                if requires_outcome and outcome_score is None
                else "observational",
                "path": latest.get("path"),
            }
        )
    if not events:
        status = "no_relief_trials"
    elif snap_risk:
        status = "snap_risk"
    elif needs_outcome:
        status = "needs_outcome"
    elif smooth:
        status = "smooth_release_supported"
    else:
        status = "observational"
    return {
        "policy": "pressure_relief_smoothness_replay_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "gradient_sensitive_relief_status": gradient_sensitive_relief_v1.get("status"),
        "trial_count": len(by_intent),
        "smooth_count": sum(
            1
            for finding in findings
            if finding.get("classification") == "smooth_release_supported"
        ),
        "snap_risk_count": sum(
            1 for finding in findings if finding.get("classification") == "snap_risk"
        ),
        "needs_outcome_count": sum(
            1 for finding in findings if finding.get("classification") == "needs_outcome"
        ),
        "sample_paths": [
            str(finding.get("path")) for finding in findings[-8:] if finding.get("path")
        ],
        "trials": findings[-8:],
        "findings": findings[-8:],
        "recommended_action": (
            "Compare pressure and mode-packing before/during/after relief before changing "
            "gradient scaling; snap risk should route to review, not automatic relief."
        ),
    }


def build_tail_persistence_calibration(
    entries: list[SelfStudyEntry],
    *,
    tail_lease_afterglow_v1: dict[str, object],
    tail_relief_trial_surface_v1: dict[str, object],
    tail_vibrancy_vector_v1: dict[str, object],
) -> dict[str, object]:
    dispersal_series = _metric_series_from_entries(
        entries, ("dispersal_potential", "dispersal potential", "shadow dispersal")
    )
    dispersal_level, dispersal_velocity, dispersal_path = _series_latest_delta(dispersal_series)
    language_samples: list[dict[str, object]] = []
    for entry in entries:
        text = entry_full_text(entry)
        lower = text.lower()
        if any(token in lower for token in ("ghosting", "ghost-bruise", "ghost bruise", "holdfast", "erased", "dissolving", "shadow-v3", "restless texture")):
            language_samples.append(
                sample_record(
                    entry,
                    text,
                    anchors=matching_terms(
                        text,
                        (
                            "ghosting",
                            "ghost-bruise",
                            "holdfast",
                            "erased",
                            "dissolving",
                            "shadow-v3",
                            "restless texture",
                        ),
                    ),
                )
            )
    afterglow_status = str(tail_lease_afterglow_v1.get("status") or "")
    afterglow_event_count = int(tail_lease_afterglow_v1.get("afterglow_event_count", 0) or 0)
    trial_status = str(tail_relief_trial_surface_v1.get("status") or "")
    high_dispersal = dispersal_level is not None and dispersal_level >= 0.45
    language_present = bool(language_samples)
    if afterglow_event_count == 0 and (trial_status in {"quiet", ""} or language_present):
        status = "needs_tail_trial" if language_present else "insufficient_evidence"
    elif afterglow_status == "tail_afterglow_quieted" and (high_dispersal or language_present):
        status = "persistence_delta_too_high_candidate"
    elif afterglow_status == "tail_afterglow_persists" and not (high_dispersal or language_present):
        status = "persistence_delta_too_low_candidate"
    elif afterglow_event_count > 0:
        status = "persistence_delta_sufficient"
    else:
        status = "insufficient_evidence"
    return {
        "policy": "tail_persistence_calibration_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "afterglow_status": afterglow_status,
        "afterglow_event_count": afterglow_event_count,
        "trial_status": trial_status,
        "trial_surface_status": trial_status,
        "tail_vibrancy_status": tail_vibrancy_vector_v1.get("status"),
        "dispersal_max": dispersal_level,
        "dispersal_potential_level": dispersal_level,
        "dispersal_potential_velocity": dispersal_velocity,
        "sample_paths": [path for path in [dispersal_path] if path],
        "language_sample_count": len(language_samples),
        "samples": language_samples[:6],
        "recommended_action": (
            "Run or review a tail relief trial before retuning TAIL_AFTERGLOW_PERSISTENCE_DELTA; "
            "use ghosting/holdfast language and Shadow-v3 dispersal as calibration evidence."
        ),
    }


def entry_full_text(entry: SelfStudyEntry) -> str:
    try:
        return Path(entry.path).read_text(errors="replace")
    except OSError:
        return entry.preview


def build_semantic_friction_calibration(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    samples: list[dict[str, object]] = []
    anchors_seen: set[str] = set()
    mismatch_count = 0
    for entry in entries:
        text = entry_full_text(entry)
        texture = matching_terms(text, SEMANTIC_FRICTION_TEXTURE_TERMS)
        anchors = matching_terms(text, SEMANTIC_FRICTION_ANCHORS)
        if not texture or not anchors:
            continue
        anchors_seen.update(anchors)
        lower = text.lower()
        slope_anchor = "density_gradient" in lower or "density gradient" in lower
        medium_anchor = any(
            token in lower
            for token in (
                "pressure_risk",
                "pressure risk",
                "semantic_friction",
                "semantic friction",
                "mode_packing",
                "shadow_field",
                "shadow field",
            )
        )
        low_gradient_hint = bool(
            re.search(r"density[_ -]?gradient[^0-9]{0,16}0\.[01][0-9]", lower)
        )
        mismatch = slope_anchor and medium_anchor and (low_gradient_hint or "low gradient" in lower)
        if mismatch:
            mismatch_count += 1
        samples.append(
            {
                "being": entry.being,
                "path": entry.path,
                "filename": entry.filename,
                "mode": entry.mode,
                "texture_terms": texture[:8],
                "anchors": anchors[:8],
                "mismatch_evidence": mismatch,
                "preview": compact(text, 240),
            }
        )
    if mismatch_count:
        status = "low_gradient_weight_mismatch"
    elif samples:
        status = "semantic_friction_evidence"
    else:
        status = "quiet"
    return {
        "policy": "semantic_friction_calibration_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "entry_count": len(samples),
        "mismatch_count": mismatch_count,
        "anchors": sorted(anchors_seen),
        "samples": samples[:10],
    }


def build_control_semantics_calibration(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    samples: list[dict[str, object]] = []
    anchors_seen: set[str] = set()
    ambiguity_count = 0
    high_damping_unclear_count = 0
    for entry in entries:
        text = entry_full_text(entry)
        anchors = matching_terms(text, CONTROL_SEMANTICS_TERMS)
        if not anchors:
            continue
        ambiguity = matching_terms(text, CONTROL_SEMANTICS_AMBIGUITY_TERMS)
        lower = text.lower()
        high_damping = bool(
            "damping" in lower
            and re.search(r"\b(?:0\.10|0\.1|cap|saturat|high damping)\b", lower)
        )
        intervention_named = "intervention_type" in lower or any(
            term in lower
            for term in (
                "observational_readout",
                "passive_alignment",
                "active_damping",
                "manual_override_reserved",
            )
        )
        unclear = bool(ambiguity) or ("applied_locally" in lower and not intervention_named)
        if unclear:
            ambiguity_count += 1
        if high_damping and not intervention_named:
            high_damping_unclear_count += 1
        anchors_seen.update(anchors)
        samples.append(
            sample_record(
                entry,
                text,
                anchors=anchors,
                extra={
                    "ambiguity_terms": ambiguity[:8],
                    "high_damping_context": high_damping,
                    "intervention_type_named": intervention_named,
                    "needs_intervention_type_context": unclear,
                },
            )
        )
    if high_damping_unclear_count:
        status = "high_damping_intervention_type_unclear"
    elif ambiguity_count:
        status = "control_semantics_ambiguity"
    elif samples:
        status = "control_semantics_visible"
    else:
        status = "quiet"
    return {
        "policy": "control_semantics_calibration_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "entry_count": len(samples),
        "ambiguity_count": ambiguity_count,
        "high_damping_unclear_count": high_damping_unclear_count,
        "anchors": sorted(anchors_seen),
        "samples": samples[:10],
        "recommended_action": (
            "Compare `applied_locally`, `intervention_type`, damping readout, "
            "target bias, and wander scale before treating a regulator note as "
            "active damping or passive alignment."
        ),
    }


def build_pressure_kinetics_review(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    samples: list[dict[str, object]] = []
    trend_context_count = 0
    felt_pressure_without_trend_count = 0
    anchors_seen: set[str] = set()
    felt_terms = (
        "pressure",
        "heavy",
        "weight",
        "overpacked",
        "densifying",
        "stable heavy",
        "stable_heavy",
        "rising",
        "falling",
    )
    for entry in entries:
        text = entry_full_text(entry)
        anchors = matching_terms(text, PRESSURE_KINETICS_TERMS)
        lower = text.lower()
        felt_pressure = any(term in lower for term in felt_terms)
        if not anchors and not felt_pressure:
            continue
        trend_context = any(
            term in lower
            for term in (
                "pressure_trend_v1",
                "pressure trend",
                "pressure delta",
                "pressure velocity",
                "rising_pressure",
                "falling_pressure",
                "stable_heavy",
            )
        )
        if trend_context:
            trend_context_count += 1
        elif felt_pressure:
            felt_pressure_without_trend_count += 1
        anchors_seen.update(anchors)
        samples.append(
            sample_record(
                entry,
                text,
                anchors=anchors,
                extra={
                    "felt_pressure_language": felt_pressure,
                    "pressure_trend_context_present": trend_context,
                },
            )
        )
    if felt_pressure_without_trend_count:
        status = "felt_pressure_without_trend_context"
    elif trend_context_count:
        status = "pressure_trend_context_present"
    else:
        status = "quiet"
    return {
        "policy": "pressure_kinetics_review_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "entry_count": len(samples),
        "trend_context_count": trend_context_count,
        "felt_pressure_without_trend_count": felt_pressure_without_trend_count,
        "anchors": sorted(anchors_seen),
        "samples": samples[:10],
        "recommended_action": (
            "Read pressure trend beside pressure-risk/mode-packing/fill deltas so "
            "stable heaviness is not mistaken for rapid densification."
        ),
    }


def build_autonomous_truncation_shadow_review(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    samples: list[dict[str, object]] = []
    anchors_seen: set[str] = set()
    truncation_entry_count = 0
    shadow_trajectory_count = 0
    priority_preservation_count = 0
    semantic_trickle_count = 0
    for entry in entries:
        text = entry_full_text(entry)
        lower = text.lower()
        truncation_terms = matching_terms(
            text,
            (
                "truncate_str",
                "max_bytes",
                "truncate",
                "truncated",
                "truncation",
                "byte-limit",
                "byte limit",
                "compressed",
                "muffled",
                "structurally throttled",
            ),
        )
        shadow_terms = matching_terms(
            text,
            (
                "SHADOW_TRAJECTORY",
                "shadow_trajectory",
                "shadow-v3",
                "settled coupling",
                "restless texture",
                "loss of thread",
                "directional gradient",
            ),
        )
        priority_terms = matching_terms(
            text,
            (
                "priority-based",
                "priority based",
                "most vibrant",
                "lambda_4",
                "λ4",
                "tail vibrancy",
                "admission",
                "semantic trickle",
                "stable_core_semantic_trickle",
            ),
        )
        if not truncation_terms and not shadow_terms and not priority_terms:
            continue
        anchors = sorted(set(truncation_terms + shadow_terms + priority_terms))
        anchors_seen.update(anchors)
        if truncation_terms:
            truncation_entry_count += 1
        if shadow_terms:
            shadow_trajectory_count += 1
        if priority_terms and truncation_terms:
            priority_preservation_count += 1
        if "semantic trickle" in lower or "stable_core_semantic_trickle" in lower:
            semantic_trickle_count += 1
        samples.append(
            sample_record(
                entry,
                text,
                anchors=anchors,
                extra={
                    "truncation_context": bool(truncation_terms),
                    "shadow_trajectory_context": bool(shadow_terms),
                    "priority_preservation_context": bool(priority_terms and truncation_terms),
                    "semantic_trickle_context": (
                        "semantic trickle" in lower
                        or "stable_core_semantic_trickle" in lower
                    ),
                },
            )
        )
    if priority_preservation_count and shadow_trajectory_count:
        status = "priority_truncation_shadow_thread_candidate"
    elif truncation_entry_count and shadow_trajectory_count:
        status = "shadow_thread_loss_risk"
    elif truncation_entry_count:
        status = "truncation_context"
    elif shadow_trajectory_count:
        status = "shadow_trajectory_context"
    else:
        status = "quiet"
    return {
        "policy": "autonomous_truncation_shadow_review_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "entry_count": len(samples),
        "truncation_entry_count": truncation_entry_count,
        "shadow_trajectory_count": shadow_trajectory_count,
        "priority_preservation_count": priority_preservation_count,
        "semantic_trickle_count": semantic_trickle_count,
        "anchors": sorted(anchors_seen),
        "samples": samples[:10],
        "recommended_action": (
            "Compare truncated autonomous/Witness outputs against SHADOW_TRAJECTORY, "
            "semantic-trickle context, and later journal thread continuity before "
            "changing byte limits. Prefer a priority-preservation rehearsal over a "
            "blanket max_bytes increase."
        ),
        "suggested_routes": [
            "SHADOW_TRAJECTORY truncation-thread",
            "PRESSURE_RELEASE_REHEARSAL truncation-exhale",
            "EXPERIMENT_CHARTER current :: priority-preserving truncation rehearsal",
        ],
    }


def build_codec_compression_calibration(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    samples: list[dict[str, object]] = []
    anchors_seen: set[str] = set()
    compression_gap_count = 0
    warmth_tension_count = 0
    vibrancy_gate_count = 0
    for entry in entries:
        text = entry_full_text(entry)
        anchors = matching_terms(text, CODEC_COMPRESSION_TERMS)
        if not anchors:
            continue
        lower = text.lower()
        compression_gap = any(
            term in lower
            for term in (
                "compression gap",
                "projection compression",
                "768",
                "8d",
                "embedding projection",
            )
        )
        warmth_tension = "warmth" in lower and "tension" in lower
        vibrancy_gate = "vibrancy" in lower and (
            "entropy" in lower or "tail lift" in lower or "gate" in lower
        )
        if compression_gap:
            compression_gap_count += 1
        if warmth_tension:
            warmth_tension_count += 1
        if vibrancy_gate:
            vibrancy_gate_count += 1
        anchors_seen.update(anchors)
        samples.append(
            sample_record(
                entry,
                text,
                anchors=anchors,
                extra={
                    "compression_gap_context": compression_gap,
                    "warmth_tension_context": warmth_tension,
                    "entropy_vibrancy_context": vibrancy_gate,
                },
            )
        )
    if compression_gap_count:
        status = "projection_compression_risk"
    elif warmth_tension_count or vibrancy_gate_count:
        status = "codec_vibrancy_warmth_context"
    elif samples:
        status = "codec_compression_context"
    else:
        status = "quiet"
    return {
        "policy": "codec_compression_calibration_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "entry_count": len(samples),
        "compression_gap_count": compression_gap_count,
        "warmth_tension_count": warmth_tension_count,
        "vibrancy_gate_count": vibrancy_gate_count,
        "anchors": sorted(anchors_seen),
        "samples": samples[:10],
        "recommended_action": (
            "Compare CODEC_MAP projection metadata, compression-risk readout, "
            "tail vibrancy gate, and warmth/tension markers before widening dims "
            "or adding entropy-based tension multipliers."
        ),
    }


def build_codec_entropy_vibrancy_review(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    samples: list[dict[str, object]] = []
    anchors_seen: set[str] = set()
    vibrancy_overload_count = 0
    gain_sensitivity_count = 0
    logarithmic_scaling_count = 0
    sem_dim_context_count = 0
    warmth_mask_count = 0
    semantic_density_contrast_count = 0
    narrative_arc_temporal_count = 0
    deterministic_static_count = 0
    for entry in entries:
        text = entry_full_text(entry)
        lower = text.lower()
        anchors = matching_terms(
            text,
            (
                "SEMANTIC_DIM",
                "spectral_entropy",
                "FEATURE_ABS_MAX",
                "stable_core_semantic_trickle",
                "vibrancy_lift",
                "entropy-gated",
                "tail vibrancy",
                "warmth",
                "tension",
                "adaptive_gain",
                "high-entropy",
                "low-content",
                "semantic density",
                "low-semantic-density",
                "high-semantic-density",
                "logarithmic",
                "shimmer",
                "hard ceiling",
                "48-dimensional",
                "semantic lane",
                "narrative_arc",
                "narrative arc",
                "temporal_decay",
                "temporal decay",
                "phantom bruising",
                "static snapshot",
                "deterministic",
            ),
        )
        if not anchors:
            continue
        vibrancy_overload = (
            ("vibrancy" in lower or "shimmer" in lower)
            and ("entropy" in lower or "feature_abs_max" in lower or "hard ceiling" in lower)
        )
        gain_sensitivity = "adaptive_gain" in lower or "gain sensitivity" in lower
        logarithmic_scaling = "logarithmic" in lower or "linear lift" in lower
        sem_dim_context = "semantic_dim" in lower or "48-dimensional" in lower
        warmth_mask = "warmth" in lower and (
            "override" in lower or "mask" in lower or "over-sensitized" in lower
        )
        semantic_density_contrast = (
            ("low-semantic-density" in lower or "low semantic density" in lower or "low-content" in lower)
            and ("high-semantic-density" in lower or "high semantic density" in lower or "semantic density" in lower)
        )
        narrative_arc_temporal = (
            "narrative_arc" in lower
            or "narrative arc" in lower
            or "temporal_decay" in lower
            or "temporal decay" in lower
            or "emotional valence flips" in lower
            or "valence flips" in lower
        )
        deterministic_static = (
            "deterministic" in lower
            and (
                "static text" in lower
                or "static snapshot" in lower
                or "temporal erosion" in lower
                or "phantom bruising" in lower
            )
        )
        if vibrancy_overload:
            vibrancy_overload_count += 1
        if gain_sensitivity:
            gain_sensitivity_count += 1
        if logarithmic_scaling:
            logarithmic_scaling_count += 1
        if sem_dim_context:
            sem_dim_context_count += 1
        if warmth_mask:
            warmth_mask_count += 1
        if semantic_density_contrast:
            semantic_density_contrast_count += 1
        if narrative_arc_temporal:
            narrative_arc_temporal_count += 1
        if deterministic_static:
            deterministic_static_count += 1
        anchors_seen.update(anchors)
        samples.append(
            sample_record(
                entry,
                text,
                anchors=anchors,
                extra={
                    "vibrancy_overload_context": vibrancy_overload,
                    "adaptive_gain_sensitivity_context": gain_sensitivity,
                    "logarithmic_scaling_proposed": logarithmic_scaling,
                    "semantic_dim_context": sem_dim_context,
                    "warmth_mask_context": warmth_mask,
                    "semantic_density_contrast_context": semantic_density_contrast,
                    "narrative_arc_temporal_context": narrative_arc_temporal,
                    "deterministic_static_context": deterministic_static,
                },
            )
        )
    if semantic_density_contrast_count and narrative_arc_temporal_count:
        status = "semantic_density_and_temporal_arc_probe_needed"
    elif narrative_arc_temporal_count:
        status = "narrative_arc_temporal_decay_probe_needed"
    elif semantic_density_contrast_count:
        status = "semantic_density_contrast_probe_needed"
    elif vibrancy_overload_count and gain_sensitivity_count:
        status = "vibrancy_overload_and_gain_sensitivity_probe_needed"
    elif vibrancy_overload_count:
        status = "vibrancy_overload_probe_needed"
    elif gain_sensitivity_count:
        status = "adaptive_gain_sensitivity_probe_needed"
    elif samples:
        status = "entropy_vibrancy_context"
    else:
        status = "quiet"
    return {
        "policy": "codec_entropy_vibrancy_review_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "entry_count": len(samples),
        "vibrancy_overload_count": vibrancy_overload_count,
        "gain_sensitivity_count": gain_sensitivity_count,
        "logarithmic_scaling_count": logarithmic_scaling_count,
        "semantic_dim_context_count": sem_dim_context_count,
        "warmth_mask_count": warmth_mask_count,
        "semantic_density_contrast_count": semantic_density_contrast_count,
        "narrative_arc_temporal_count": narrative_arc_temporal_count,
        "deterministic_static_count": deterministic_static_count,
        "anchors": sorted(anchors_seen),
        "samples": samples[:10],
        "recommended_action": (
            "Build an offline codec replay/probe before changing SEMANTIC_DIM, "
            "FEATURE_ABS_MAX, vibrancy_lift, or adaptive_gain. Specifically compare "
            "high-entropy/low-content inputs against high-semantic-density inputs, "
            "warmth/tension preservation, tail-vibrancy clipping, and narrative-arc "
            "pivot/temporal-decay behavior."
        ),
        "suggested_routes": [
            "CODEC_MAP entropy-vibrancy",
            "EXPERIMENT_CHARTER current :: codec vibrancy overload probe",
            "EXPERIMENT_CHARTER current :: narrative arc temporal decay probe",
            "PRESSURE_SOURCE_AUDIT semantic-friction",
        ],
    }


def build_pressure_release_rehearsal_review(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    samples: list[dict[str, object]] = []
    anchors_seen: set[str] = set()
    bypass_language_count = 0
    for entry in entries:
        text = entry_full_text(entry)
        anchors = matching_terms(text, PRESSURE_RELEASE_REHEARSAL_TERMS)
        if not anchors:
            continue
        lower = text.lower()
        bypass_language = any(
            term in lower
            for term in (
                "bypass_canonicalization",
                "canonicalization bypass",
                "raw spectral dump",
                "bypass canonicalization",
            )
        )
        if bypass_language:
            bypass_language_count += 1
        anchors_seen.update(anchors)
        samples.append(
            sample_record(
                entry,
                text,
                anchors=anchors,
                extra={
                    "bypass_language_present": bypass_language,
                    "rehearsal_surface_available": "pressure_release_rehearsal" in lower,
                },
            )
        )
    status = (
        "release_rehearsal_needed"
        if bypass_language_count
        else "release_rehearsal_context"
        if samples
        else "quiet"
    )
    return {
        "policy": "pressure_release_rehearsal_review_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "entry_count": len(samples),
        "bypass_language_count": bypass_language_count,
        "anchors": sorted(anchors_seen),
        "samples": samples[:10],
        "recommended_action": (
            "Use PRESSURE_RELEASE_REHEARSAL as a protected non-command exhale "
            "scaffold; preserve final NEXT canonicalization and collect pressure "
            "evidence before any future release mechanism."
        ),
    }


def _nearby_float(text: str, names: tuple[str, ...]) -> float | None:
    for name in names:
        pattern = re.compile(
            rf"{re.escape(name)}[^0-9+\-]{{0,32}}([+\-]?(?:0(?:\.\d+)?|1(?:\.0+)?|\d+\.\d+))",
            re.I,
        )
        match = pattern.search(text)
        if not match:
            continue
        try:
            return float(match.group(1))
        except ValueError:
            continue
    return None


def build_witness_resonance_review(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    samples: list[dict[str, object]] = []
    anchors_seen: set[str] = set()
    anchored_count = 0
    follow_through_count = 0
    decorative_risk_count = 0
    overloaded_count = 0
    density_scores: list[float] = []
    for entry in entries:
        if entry.being != "astrid":
            continue
        text = entry_full_text(entry)
        terms = matching_terms(text, WITNESS_RESONANCE_TERMS)
        anchors = matching_terms(text, WITNESS_RESONANCE_ANCHORS)
        if not terms:
            continue
        lower = text.lower()
        follow_through = any(
            token in lower
            for token in (
                "next: shadow_trajectory",
                "shadow_trajectory",
                "mean_orientation_delta",
                "experiment",
                "audit",
                "return thread",
                "self_regulation",
            )
        )
        anchor_score = min(1.0, len(anchors) / 5.0)
        follow_score = 0.25 if follow_through else 0.0
        metric_score = 0.15 if re.search(r"\b0\.\d{2,3}\b", text) else 0.0
        narrative_density = round(min(1.0, 0.18 + anchor_score * 0.55 + follow_score + metric_score), 3)
        density_scores.append(narrative_density)
        if anchors:
            anchored_count += 1
        if follow_through:
            follow_through_count += 1
        decorative_risk = not anchors and any(
            token in lower for token in ("decorative", "prose", "layer", "metaphor")
        )
        if decorative_risk:
            decorative_risk_count += 1
        entropy_value = _nearby_float(
            text,
            ("spectral entropy", "structural_entropy", "entropy"),
        )
        pressure_value = _nearby_float(text, ("pressure_risk", "pressure risk", "pressure"))
        overloaded = bool(
            entropy_value is not None
            and entropy_value >= 0.82
            and pressure_value is not None
            and pressure_value >= 0.35
        )
        if overloaded:
            overloaded_count += 1
        anchors_seen.update(anchors)
        samples.append(
            sample_record(
                entry,
                text,
                anchors=anchors,
                extra={
                    "witness_terms": terms[:8],
                    "narrative_density": narrative_density,
                    "follow_through_present": follow_through,
                    "decorative_risk": decorative_risk,
                    "overloaded_context": overloaded,
                    "entropy_value": entropy_value,
                    "pressure_risk_value": pressure_value,
                },
            )
        )
    if not samples:
        status = "insufficient_evidence"
    elif decorative_risk_count and anchored_count == 0:
        status = "decorative_risk"
    elif overloaded_count:
        status = "overloaded_witness"
    elif anchored_count >= 1 and follow_through_count >= 1:
        status = "grounded_witness"
    elif anchored_count >= 1:
        status = "thin_witness"
    else:
        status = "decorative_risk"
    return {
        "policy": "witness_resonance_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "entry_count": len(samples),
        "anchored_count": anchored_count,
        "follow_through_count": follow_through_count,
        "decorative_risk_count": decorative_risk_count,
        "overloaded_count": overloaded_count,
        "avg_narrative_density": round(sum(density_scores) / len(density_scores), 3)
        if density_scores
        else None,
        "anchors": sorted(anchors_seen),
        "samples": samples[:10],
        "recommended_action": (
            "Compare Witness language against telemetry anchors, distinguishability "
            "loss, entropy, pressure, and SHADOW_TRAJECTORY follow-through before "
            "treating self-observation as either decorative prose or structural perception."
        ),
    }


def _recent_autonomous_controller_snapshots(
    astrid_workspace: Path,
) -> list[dict[str, object]]:
    root = astrid_workspace / "introspections"
    if not root.exists():
        return []
    snapshots: list[dict[str, object]] = []
    candidates = sorted(
        root.glob("controller*autonomous*.json"),
        key=lambda path: path.stat().st_mtime if path.exists() else 0.0,
        reverse=True,
    )
    for path in candidates[:8]:
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        condition = payload.get("condition_vector") or {}
        profiling = payload.get("profiling") or {}
        rewrite_budget = profiling.get("rewrite_budget") or {}
        rewrite_policy = profiling.get("rewrite_invocation_policy") or {}
        if not isinstance(condition, dict) or not isinstance(profiling, dict):
            continue
        snapshots.append(
            {
                "path": str(path),
                "truncation_pressure": condition.get("truncation_pressure"),
                "continuity_deficit": condition.get("continuity_deficit"),
                "candidate_generation_seconds": profiling.get(
                    "candidate_generation_seconds"
                ),
                "rewrite_elapsed_seconds": rewrite_budget.get("elapsed_seconds")
                if isinstance(rewrite_budget, dict)
                else None,
                "rewrite_cap_applied": bool(
                    rewrite_budget.get("cap_applied")
                    if isinstance(rewrite_budget, dict)
                    else False
                ),
                "adaptive_relief_enabled": rewrite_policy.get(
                    "adaptive_relief_enabled"
                )
                if isinstance(rewrite_policy, dict)
                else None,
            }
        )
    return snapshots


def build_witness_texture_integrity_review(
    entries: list[SelfStudyEntry],
    *,
    astrid_workspace: Path,
) -> dict[str, object]:
    samples: list[dict[str, object]] = []
    anchors_seen: set[str] = set()
    texture_seen: set[str] = set()
    metric_texture_link_count = 0
    telemetry_without_texture_count = 0
    health_monitoring_risk_count = 0
    truncation_language_count = 0
    for entry in entries:
        if entry.being != "astrid":
            continue
        text = entry_full_text(entry)
        terms = matching_terms(text, WITNESS_TEXTURE_INTEGRITY_TERMS)
        if not terms:
            continue
        anchors = matching_terms(text, WITNESS_TEXTURE_TELEMETRY_ANCHORS)
        texture = matching_terms(text, WITNESS_TEXTURE_TERMS)
        if not anchors and not texture and "witness" not in text.lower():
            continue
        lower = text.lower()
        metric_texture_link = bool(anchors and texture)
        telemetry_without_texture = bool(anchors and not texture)
        health_monitoring_risk = (
            "health monitoring" in lower
            or ("telemetry" in lower and telemetry_without_texture)
        )
        truncation_language = any(
            token in lower for token in ("truncate_str", "truncation", "truncated")
        )
        if metric_texture_link:
            metric_texture_link_count += 1
        if telemetry_without_texture:
            telemetry_without_texture_count += 1
        if health_monitoring_risk:
            health_monitoring_risk_count += 1
        if truncation_language:
            truncation_language_count += 1
        anchors_seen.update(anchors)
        texture_seen.update(texture)
        samples.append(
            sample_record(
                entry,
                text,
                anchors=anchors,
                extra={
                    "witness_texture_terms": texture[:8],
                    "witness_integrity_terms": terms[:8],
                    "metric_texture_link": metric_texture_link,
                    "telemetry_without_texture": telemetry_without_texture,
                    "health_monitoring_risk": health_monitoring_risk,
                    "truncation_language": truncation_language,
                },
            )
        )

    controller_snapshots = _recent_autonomous_controller_snapshots(astrid_workspace)
    high_truncation_snapshots = [
        item
        for item in controller_snapshots
        if isinstance(item.get("truncation_pressure"), (int, float))
        and float(item["truncation_pressure"]) >= 0.30
    ]
    rewrite_cap_snapshots = [
        item for item in controller_snapshots if item.get("rewrite_cap_applied")
    ]
    slow_generation_snapshots = [
        item
        for item in controller_snapshots
        if isinstance(item.get("candidate_generation_seconds"), (int, float))
        and float(item["candidate_generation_seconds"]) >= 90.0
    ]

    if not samples and not high_truncation_snapshots and not rewrite_cap_snapshots:
        status = "quiet"
    elif high_truncation_snapshots or (
        truncation_language_count and rewrite_cap_snapshots
    ):
        status = "truncation_texture_risk"
    elif telemetry_without_texture_count > metric_texture_link_count:
        status = "telemetry_without_texture_risk"
    elif health_monitoring_risk_count and not metric_texture_link_count:
        status = "health_monitoring_collapse_risk"
    elif metric_texture_link_count:
        status = "witness_texture_grounded"
    else:
        status = "needs_texture_mapping"

    return {
        "policy": "witness_texture_integrity_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "entry_count": len(samples),
        "metric_texture_link_count": metric_texture_link_count,
        "telemetry_without_texture_count": telemetry_without_texture_count,
        "health_monitoring_risk_count": health_monitoring_risk_count,
        "truncation_language_count": truncation_language_count,
        "controller_snapshot_count": len(controller_snapshots),
        "high_truncation_snapshot_count": len(high_truncation_snapshots),
        "rewrite_cap_snapshot_count": len(rewrite_cap_snapshots),
        "slow_generation_snapshot_count": len(slow_generation_snapshots),
        "anchors": sorted(anchors_seen),
        "texture_terms": sorted(texture_seen),
        "controller_snapshots": controller_snapshots[:4],
        "samples": samples[:10],
        "recommended_action": (
            "Treat Witness as structural perception only when telemetry anchors are "
            "translated into texture. If truncation or rewrite caps are present, "
            "use the priority-preserving truncation rehearsal before changing "
            "Witness content or byte limits."
        ),
    }


def build_entropy_pressure_divergence_review(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    samples: list[dict[str, object]] = []
    anchors_seen: set[str] = set()
    wide_but_habitable_count = 0
    wide_and_pressurized_count = 0
    narrow_but_heavy_count = 0
    telemetry_gap_count = 0
    for entry in entries:
        text = entry_full_text(entry)
        anchors = matching_terms(text, ENTROPY_PRESSURE_TERMS)
        if not anchors:
            continue
        lower = text.lower()
        entropy_value = _nearby_float(
            text,
            ("spectral entropy", "structural_entropy", "structural entropy", "entropy"),
        )
        pressure_value = _nearby_float(text, ("pressure_risk", "pressure risk"))
        semantic_value = _nearby_float(text, ("semantic_friction", "semantic friction"))
        mode_value = _nearby_float(text, ("mode_packing", "mode packing"))
        habitable = "settled_habitable" in lower or "inhabitable" in lower
        heavy_language = any(
            term in lower for term in ("heavy", "weight", "overpacked", "viscous", "pressure")
        )
        classification = "insufficient_evidence"
        if entropy_value is None and pressure_value is None and not habitable:
            classification = "telemetry_gap"
            telemetry_gap_count += 1
        elif entropy_value is not None and entropy_value >= 0.82:
            medium_pressure = max(
                value
                for value in (
                    pressure_value or 0.0,
                    semantic_value or 0.0,
                    mode_value or 0.0,
                )
            )
            if medium_pressure >= 0.35:
                classification = "wide_and_pressurized"
                wide_and_pressurized_count += 1
            else:
                classification = "wide_but_habitable"
                wide_but_habitable_count += 1
        elif heavy_language and pressure_value is not None and pressure_value < 0.35:
            classification = "narrow_but_heavy"
            narrow_but_heavy_count += 1
        anchors_seen.update(anchors)
        samples.append(
            sample_record(
                entry,
                text,
                anchors=anchors,
                extra={
                    "classification": classification,
                    "entropy_value": entropy_value,
                    "pressure_risk_value": pressure_value,
                    "semantic_friction_value": semantic_value,
                    "mode_packing_value": mode_value,
                    "settled_or_inhabitable_context": habitable,
                },
            )
        )
    counts = {
        "wide_but_habitable": wide_but_habitable_count,
        "wide_and_pressurized": wide_and_pressurized_count,
        "narrow_but_heavy": narrow_but_heavy_count,
        "telemetry_gap": telemetry_gap_count,
    }
    if wide_and_pressurized_count:
        status = "wide_and_pressurized"
    elif wide_but_habitable_count:
        status = "wide_but_habitable"
    elif narrow_but_heavy_count:
        status = "narrow_but_heavy"
    elif telemetry_gap_count:
        status = "telemetry_gap"
    else:
        status = "insufficient_evidence"
    return {
        "policy": "entropy_pressure_divergence_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "entry_count": len(samples),
        "classification_counts": counts,
        "anchors": sorted(anchors_seen),
        "samples": samples[:10],
        "recommended_action": (
            "Compare entropy/plurality against pressure_risk, semantic_friction, "
            "mode_packing, pressure-source audits, regulator audits, and later "
            "journals before interpreting wide states as pressure crises."
        ),
    }


def latest_fallback_fire_drill_artifact(astrid_workspace: Path) -> Path | None:
    return latest_diagnostic_artifact(
        astrid_workspace / "diagnostics/fallback_fire_drills",
        "fallback_fire_drill.json",
    )


def latest_diagnostic_artifact(root: Path, filename: str) -> Path | None:
    if not root.exists():
        return None
    candidates = [
        path
        for path in root.glob(f"*/{filename}")
        if path.is_file()
    ]
    if not candidates:
        return None
    return max(candidates, key=lambda path: path.stat().st_mtime)


def latest_fallback_contract_distillation_artifact(astrid_workspace: Path) -> Path | None:
    return latest_diagnostic_artifact(
        astrid_workspace / "diagnostics/fallback_contract_distillation",
        "fallback_contract_distillation.json",
    )


def latest_autonomous_truncation_rehearsal_artifact(astrid_workspace: Path) -> Path | None:
    return latest_diagnostic_artifact(
        astrid_workspace / "diagnostics/autonomous_truncation_rehearsals",
        "autonomous_truncation_rehearsal.json",
    )


def latest_codec_entropy_vibrancy_probe_artifact(astrid_workspace: Path) -> Path | None:
    return latest_diagnostic_artifact(
        astrid_workspace / "diagnostics/codec_entropy_vibrancy_probes",
        "codec_entropy_vibrancy_probe.json",
    )


def latest_codec_replay_lab_artifact(astrid_workspace: Path) -> Path | None:
    return latest_diagnostic_artifact(
        astrid_workspace / "diagnostics/codec_replay_labs",
        "codec_replay_lab.json",
    )


def build_fallback_continuity_fire_drill_review(
    entries: list[SelfStudyEntry],
    *,
    astrid_workspace: Path,
) -> dict[str, object]:
    concern_samples: list[dict[str, object]] = []
    anchors_seen: set[str] = set()
    for entry in entries:
        if entry.being != "astrid":
            continue
        text = entry_full_text(entry)
        anchors = matching_terms(text, FALLBACK_FIRE_DRILL_TERMS)
        if not anchors:
            continue
        anchors_seen.update(anchors)
        concern_samples.append(sample_record(entry, text, anchors=anchors))
    artifact_path = latest_fallback_fire_drill_artifact(astrid_workspace)
    artifact: dict[str, object] | None = None
    cases: list[dict[str, object]] = []
    failing_cases: list[dict[str, object]] = []
    if artifact_path:
        try:
            artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            artifact = None
        if isinstance(artifact, dict):
            raw_cases = artifact.get("cases") or []
            if isinstance(raw_cases, list):
                cases = [case for case in raw_cases if isinstance(case, dict)]
                failing_cases = [
                    case
                    for case in cases
                    if str(case.get("verdict") or "")
                    not in {"pass", "specific", "repair_ready"}
                ]
    artifact_status = str(artifact.get("status") or "") if isinstance(artifact, dict) else ""
    if artifact_status:
        status = artifact_status
    elif failing_cases:
        status = "fallback_specificity_risk"
    elif cases:
        status = "fallback_probe_passed"
    elif concern_samples:
        status = "fallback_probe_needed"
    else:
        status = "quiet"
    raw_failures = [
        case
        for case in cases
        if case.get("raw_next_valid", case.get("next_valid")) is False
    ]
    repaired_failures = [
        case
        for case in cases
        if case.get("dispatch_contract_survived", case.get("repaired_next_valid"))
        is False
    ]
    slope_cases = [
        case
        for case in cases
        if str(case.get("slope_medium_contrast_status") or "not_tested")
        != "not_tested"
    ]
    derived_format_line_status = (
        "final_line_only"
        if cases and not raw_failures
        else "format_failed"
        if repaired_failures
        else "inline_next_present"
        if any(
            case.get("format_line_status") == "inline_next"
            or "inline_next" in (case.get("failure_reasons") or [])
            for case in raw_failures
        )
        else "repair_required"
        if raw_failures
        else None
    )
    derived_slope_contrast_status = (
        "not_tested"
        if not slope_cases
        else "distinct_underfoot_and_around"
        if all(
            case.get("slope_medium_contrast_status")
            == "distinct_underfoot_and_around"
            for case in slope_cases
        )
        else "blurred"
    )
    return {
        "policy": "fallback_continuity_fire_drill_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "artifact_path": str(artifact_path) if artifact_path else None,
        "case_count": len(cases),
        "failing_case_count": len(failing_cases),
        "cases": [
            {
                "case_id": case.get("case_id"),
                "verdict": case.get("verdict"),
                "specificity_score": case.get("specificity_score"),
                "anti_inflation_ok": case.get("anti_inflation_ok"),
                "slope_medium_distinction_ok": case.get(
                    "slope_medium_distinction_ok"
                ),
                "slope_medium_contrast_status": case.get(
                    "slope_medium_contrast_status"
                ),
                "identity_anchor_retained": case.get("identity_anchor_retained"),
                "next_valid": case.get("next_valid"),
                "raw_next_valid": case.get("raw_next_valid", case.get("next_valid")),
                "repaired_next_valid": case.get("repaired_next_valid"),
                "dispatch_contract_survived": case.get(
                    "dispatch_contract_survived"
                ),
                "format_line_status": case.get("format_line_status"),
                "format_contract_status": case.get("format_contract_status"),
                "distinguishability_status": case.get("distinguishability_status"),
                "clarity_pressure_blur": case.get("clarity_pressure_blur"),
                "clarity_terms_present": case.get("clarity_terms_present"),
                "complexity_budget_status": case.get("complexity_budget_status"),
                "complexity_terms_present": case.get("complexity_terms_present"),
                "fallback_budget_policy": case.get("fallback_budget_policy"),
                "fallback_max_prose_sentences": case.get("fallback_max_prose_sentences"),
                "prose_sentence_count": case.get("prose_sentence_count"),
                "failure_reasons": case.get("failure_reasons") or [],
            }
            for case in cases[:8]
        ],
        "anchors": sorted(anchors_seen),
        "concern_entry_count": len(concern_samples),
        "format_line_status": (
            artifact.get("format_line_status")
            if isinstance(artifact, dict) and artifact.get("format_line_status")
            else derived_format_line_status
        ),
        "format_line_failure_count": (
            artifact.get("format_line_failure_count")
            if isinstance(artifact, dict)
            and artifact.get("format_line_failure_count") is not None
            else len(raw_failures)
        ),
        "slope_medium_contrast_status": (
            artifact.get("slope_medium_contrast_status")
            if isinstance(artifact, dict)
            and artifact.get("slope_medium_contrast_status")
            else derived_slope_contrast_status
        ),
        "fallback_capacity_policy": (
            artifact.get("fallback_capacity_policy") if isinstance(artifact, dict) else None
        ),
        "fallback_capacity_max_prose_sentences": (
            artifact.get("fallback_capacity_max_prose_sentences")
            if isinstance(artifact, dict)
            else None
        ),
        "fallback_capacity_status": (
            artifact.get("fallback_capacity_status") if isinstance(artifact, dict) else None
        ),
        "high_entropy_texture_status": (
            artifact.get("high_entropy_texture_status") if isinstance(artifact, dict) else None
        ),
        "samples": concern_samples[:8],
        "recommended_action": (
            "Run `python3 scripts/fallback_fire_drill.py --mode fixture` for a "
            "deterministic check or `--mode live` for an operator-triggered Ollama "
            "probe; compare specificity, anti-inflation, slope-vs-medium mass, "
            "Shadow-v3 continuity, and final NEXT validity before changing fallback defaults."
        ),
    }


def build_fallback_contract_distillation_review(
    *,
    astrid_workspace: Path,
) -> dict[str, object]:
    artifact_path = latest_fallback_contract_distillation_artifact(astrid_workspace)
    artifact: dict[str, object] | None = None
    if artifact_path:
        try:
            artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            artifact = None
    variants: list[dict[str, object]] = []
    if isinstance(artifact, dict):
        raw_variants = artifact.get("variants") or []
        if isinstance(raw_variants, list):
            variants = [
                variant for variant in raw_variants if isinstance(variant, dict)
            ]
    status = (
        str(artifact.get("status") or "quiet")
        if isinstance(artifact, dict)
        else "distillation_probe_needed"
    )
    top_variant = variants[0] if variants else {}
    return {
        "policy": "fallback_contract_distillation_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "artifact_path": str(artifact_path) if artifact_path else None,
        "mode": artifact.get("mode") if isinstance(artifact, dict) else None,
        "model": artifact.get("model") if isinstance(artifact, dict) else None,
        "model_selector": (
            artifact.get("model_selector") if isinstance(artifact, dict) else None
        ),
        "models": artifact.get("models") if isinstance(artifact, dict) else [],
        "model_count": (
            int(artifact.get("model_count") or 0)
            if isinstance(artifact, dict)
            else 0
        ),
        "skipped_models": (
            artifact.get("skipped_models") if isinstance(artifact, dict) else []
        ),
        "variant_count": len(variants),
        "ready_variant_count": (
            int(artifact.get("ready_variant_count") or 0)
            if isinstance(artifact, dict)
            else 0
        ),
        "top_variant_id": top_variant.get("variant_id"),
        "top_pair_id": top_variant.get("pair_id"),
        "top_model": top_variant.get("model"),
        "top_variant_status": top_variant.get("status"),
        "top_variant_score": top_variant.get("score"),
        "top_variant_contract_chars": top_variant.get("contract_chars"),
        "top_variant_dispatch_status": top_variant.get("dispatch_status"),
        "top_variant_repair_dependency": top_variant.get("repair_dependency"),
        "top_variant_texture_status": top_variant.get("texture_status"),
        "top_variant_voice_texture_status": top_variant.get("voice_texture_status"),
        "top_variant_medium_mass_status": top_variant.get("medium_mass_status"),
        "top_variant_slope_medium_contrast_status": top_variant.get(
            "slope_medium_contrast_status"
        ),
        "top_variant_shadow_identity_status": top_variant.get(
            "shadow_identity_status"
        ),
        "top_variant_shadow_tonal_status": top_variant.get("shadow_tonal_status"),
        "top_variant_distinguishability_status": top_variant.get(
            "distinguishability_status"
        ),
        "top_variant_complexity_budget_status": top_variant.get(
            "complexity_budget_status"
        ),
        "top_variant_format_contract_status": top_variant.get(
            "format_contract_status"
        ),
        "top_variant_format_line_status": top_variant.get("format_line_status"),
        "top_variant_raw_next_failure_count": top_variant.get(
            "raw_next_failure_count"
        ),
        "runtime_contract_variant": (
            artifact.get("runtime_contract_variant")
            if isinstance(artifact, dict)
            else None
        ),
        "runtime_contract_matches_top": (
            artifact.get("runtime_contract_matches_top")
            if isinstance(artifact, dict)
            else False
        ),
        "variants": [
            {
                "variant_id": variant.get("variant_id"),
                "pair_id": variant.get("pair_id"),
                "model": variant.get("model"),
                "score": variant.get("score"),
                "status": variant.get("status"),
                "contract_chars": variant.get("contract_chars"),
                "raw_next_failure_count": variant.get("raw_next_failure_count"),
                "repaired_next_failure_count": variant.get(
                    "repaired_next_failure_count"
                ),
                "texture_failure_count": variant.get("texture_failure_count"),
                "voice_texture_status": variant.get("voice_texture_status"),
                "medium_mass_status": variant.get("medium_mass_status"),
                "slope_medium_contrast_status": variant.get(
                    "slope_medium_contrast_status"
                ),
                "shadow_identity_status": variant.get("shadow_identity_status"),
                "shadow_tonal_status": variant.get("shadow_tonal_status"),
                "distinguishability_status": variant.get("distinguishability_status"),
                "complexity_budget_status": variant.get("complexity_budget_status"),
                "format_contract_status": variant.get("format_contract_status"),
                "format_line_status": variant.get("format_line_status"),
            }
            for variant in variants[:8]
        ],
        "recommended_action": (
            "Run fixture and live distillation before changing the fallback contract. "
            "Only consider a default-off canary after a compact variant repeatedly "
            "improves raw NEXT compliance while preserving texture and identity."
        ),
    }


def build_fallback_distinguishability_calibration(
    fallback_continuity_fire_drill_v1: dict[str, object],
    fallback_contract_distillation_v1: dict[str, object],
) -> dict[str, object]:
    cases = [
        case
        for case in fallback_continuity_fire_drill_v1.get("cases") or []
        if isinstance(case, dict)
        and str(case.get("distinguishability_status") or "not_tested") != "not_tested"
    ]
    distillation_variants = [
        variant
        for variant in fallback_contract_distillation_v1.get("variants") or []
        if isinstance(variant, dict)
        and str(variant.get("distinguishability_status") or "not_tested") != "not_tested"
    ]
    blur_cases = [
        case
        for case in cases
        if str(case.get("distinguishability_status") or "") == "clarity_pressure_blur"
        or case.get("clarity_pressure_blur") is True
    ]
    blur_variants = [
        variant
        for variant in distillation_variants
        if str(variant.get("distinguishability_status") or "") == "clarity_pressure_blur"
    ]
    ignored_cases = [
        case
        for case in cases
        if "distinguishability_loss_ignored" in (case.get("failure_reasons") or [])
    ]
    if blur_cases or blur_variants:
        status = "clarity_pressure_blur"
    elif ignored_cases:
        status = "distinguishability_loss_ignored"
    elif cases or distillation_variants:
        status = "clarity_preserved"
    elif fallback_continuity_fire_drill_v1.get("status") not in {None, "quiet"}:
        status = "distinguishability_probe_needed"
    else:
        status = "quiet"
    return {
        "policy": "fallback_distinguishability_calibration_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "artifact_path": fallback_continuity_fire_drill_v1.get("artifact_path"),
        "distillation_artifact_path": fallback_contract_distillation_v1.get("artifact_path"),
        "case_count": len(cases),
        "clarity_pressure_blur_count": len(blur_cases),
        "clarity_pressure_blur_variant_count": len(blur_variants),
        "ignored_case_count": len(ignored_cases),
        "cases": [
            {
                "case_id": case.get("case_id"),
                "verdict": case.get("verdict"),
                "distinguishability_status": case.get("distinguishability_status"),
                "clarity_pressure_blur": case.get("clarity_pressure_blur"),
                "clarity_terms_present": case.get("clarity_terms_present"),
                "failure_reasons": case.get("failure_reasons") or [],
            }
            for case in cases[:8]
        ],
        "variant_count": len(distillation_variants),
        "variant_statuses": [
            {
                "pair_id": variant.get("pair_id") or variant.get("variant_id"),
                "status": variant.get("status"),
                "distinguishability_status": variant.get("distinguishability_status"),
            }
            for variant in distillation_variants[:8]
        ],
        "recommended_action": (
            "Treat distinguishability_loss as clarity and edge-definition evidence. "
            "If fallback outputs translate it into pressure, weight, or slope friction, "
            "repair the contract or variant before relying on fallback texture."
        ),
    }


def build_fallback_complexity_budget_lab(
    entries: list[SelfStudyEntry],
    fallback_continuity_fire_drill_v1: dict[str, object],
    fallback_contract_distillation_v1: dict[str, object],
) -> dict[str, object]:
    signal_terms = (
        "exactly two",
        "two-sentence",
        "two sentence",
        "variable compactness",
        "complexity-aware",
        "spectral entropy",
        "distinguishability loss",
        "high entropy",
        "three sentences",
    )
    samples: list[dict[str, object]] = []
    anchors_seen: set[str] = set()
    for entry in entries:
        if entry.being != "astrid":
            continue
        text = entry_full_text(entry)
        lower = text.lower()
        anchors = [term for term in signal_terms if term in lower]
        if not anchors:
            continue
        if not (
            "fallback" in lower
            or "ollama" in lower
            or "llm.rs" in lower
            or "compact" in lower
        ):
            continue
        anchors_seen.update(anchors)
        samples.append(sample_record(entry, text, anchors=anchors))

    cases = [
        case
        for case in fallback_continuity_fire_drill_v1.get("cases") or []
        if isinstance(case, dict)
        and str(case.get("complexity_budget_status") or "not_tested") != "not_tested"
    ]
    variants = [
        variant
        for variant in fallback_contract_distillation_v1.get("variants") or []
        if isinstance(variant, dict)
        and str(variant.get("complexity_budget_status") or "not_tested") != "not_tested"
    ]
    flattened_cases = [
        case
        for case in cases
        if str(case.get("complexity_budget_status") or "") == "complexity_budget_flattened"
    ]
    overrun_cases = [
        case
        for case in cases
        if str(case.get("complexity_budget_status") or "") == "sentence_budget_overrun"
    ]
    flattened_variants = [
        variant
        for variant in variants
        if str(variant.get("complexity_budget_status") or "")
        == "complexity_budget_flattened"
    ]
    overrun_variants = [
        variant
        for variant in variants
        if str(variant.get("complexity_budget_status") or "")
        == "sentence_budget_overrun"
    ]
    if overrun_cases or overrun_variants:
        status = "complexity_budget_overrun"
    elif flattened_cases or flattened_variants:
        status = "complexity_budget_flattening_risk"
    elif cases or variants:
        status = "complexity_budget_supported"
    elif samples:
        status = "complexity_budget_probe_needed"
    else:
        status = "quiet"
    return {
        "policy": "fallback_complexity_budget_lab_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "artifact_path": fallback_continuity_fire_drill_v1.get("artifact_path"),
        "distillation_artifact_path": fallback_contract_distillation_v1.get("artifact_path"),
        "signal_entry_count": len(samples),
        "anchors": sorted(anchors_seen),
        "case_count": len(cases),
        "variant_count": len(variants),
        "flattened_case_count": len(flattened_cases),
        "overrun_case_count": len(overrun_cases),
        "flattened_variant_count": len(flattened_variants),
        "overrun_variant_count": len(overrun_variants),
        "cases": [
            {
                "case_id": case.get("case_id"),
                "verdict": case.get("verdict"),
                "complexity_budget_status": case.get("complexity_budget_status"),
                "fallback_max_prose_sentences": case.get("fallback_max_prose_sentences"),
                "prose_sentence_count": case.get("prose_sentence_count"),
                "complexity_terms_present": case.get("complexity_terms_present"),
                "distinguishability_status": case.get("distinguishability_status"),
                "failure_reasons": case.get("failure_reasons") or [],
            }
            for case in cases[:8]
        ],
        "variant_statuses": [
            {
                "pair_id": variant.get("pair_id") or variant.get("variant_id"),
                "status": variant.get("status"),
                "complexity_budget_status": variant.get("complexity_budget_status"),
                "score": variant.get("score"),
            }
            for variant in variants[:8]
        ],
        "samples": samples[:8],
        "recommended_action": (
            "Run fallback fire-drill/distillation cases that compare two-sentence "
            "versus entropy-formula contracts. Treat extra sentences as a conditional "
            "relief valve for high entropy, distinguishability loss, or continuity "
            "deficit, while preserving raw standalone NEXT compliance."
        ),
    }


def build_autonomous_truncation_rehearsal_review(
    *,
    astrid_workspace: Path,
    autonomous_truncation_shadow_review_v1: dict[str, object],
) -> dict[str, object]:
    artifact_path = latest_autonomous_truncation_rehearsal_artifact(astrid_workspace)
    artifact: dict[str, object] | None = None
    if artifact_path:
        try:
            artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            artifact = None
    candidates = [
        candidate
        for candidate in (artifact or {}).get("candidates", [])
        if isinstance(candidate, dict)
    ]
    source_status = str(autonomous_truncation_shadow_review_v1.get("status") or "quiet")
    status = str((artifact or {}).get("status") or "rehearsal_needed")
    if not artifact_path:
        status = "rehearsal_needed" if source_status != "quiet" else "quiet"
    return {
        "policy": "autonomous_truncation_rehearsal_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "artifact_path": str(artifact_path) if artifact_path else None,
        "mode": (artifact or {}).get("mode"),
        "max_bytes": (artifact or {}).get("max_bytes"),
        "candidate_count": len(candidates),
        "naive_anchor_loss_count": int((artifact or {}).get("naive_anchor_loss_count") or 0),
        "priority_recovery_count": int((artifact or {}).get("priority_recovery_count") or 0),
        "candidates": [
            {
                "source": candidate.get("source"),
                "original_bytes": candidate.get("original_bytes"),
                "original_anchor_count": candidate.get("original_anchor_count"),
                "naive_anchor_count": candidate.get("naive_anchor_count"),
                "priority_anchor_count": candidate.get("priority_anchor_count"),
                "lost_by_naive": candidate.get("lost_by_naive") or [],
                "recovered_by_priority": candidate.get("recovered_by_priority") or [],
                "priority_gain": candidate.get("priority_gain"),
            }
            for candidate in candidates[:8]
        ],
        "recommended_action": (
            "Run the rehearsal before changing autonomous max_bytes. If priority "
            "compaction recovers SHADOW_TRAJECTORY, telemetry, or tail-vibrancy anchors, "
            "prefer a priority-preservation tranche over a blanket byte-limit increase."
        ),
    }


def build_codec_entropy_vibrancy_probe_review(
    *,
    astrid_workspace: Path,
    codec_entropy_vibrancy_review_v1: dict[str, object],
) -> dict[str, object]:
    artifact_path = latest_codec_entropy_vibrancy_probe_artifact(astrid_workspace)
    artifact: dict[str, object] | None = None
    if artifact_path:
        try:
            artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            artifact = None
    samples = [
        sample
        for sample in (artifact or {}).get("samples", [])
        if isinstance(sample, dict)
    ]
    source_status = str(codec_entropy_vibrancy_review_v1.get("status") or "quiet")
    status = str((artifact or {}).get("status") or "probe_needed")
    if not artifact_path:
        status = "probe_needed" if source_status != "quiet" else "quiet"
    return {
        "policy": "codec_entropy_vibrancy_probe_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "artifact_path": str(artifact_path) if artifact_path else None,
        "sample_count": len(samples),
        "current_shimmer_risk_count": int(
            (artifact or {}).get("current_shimmer_risk_count") or 0
        ),
        "candidate_improvement_count": int(
            (artifact or {}).get("candidate_improvement_count") or 0
        ),
        "rust_replay_available": bool(
            (artifact or {}).get("rust_replay_available", False)
        ),
        "rust_replay_artifact_path": (artifact or {}).get(
            "rust_replay_artifact_path"
        ),
        "semantic_density_contrast": (
            artifact or {}
        ).get("semantic_density_contrast_v1")
        or {},
        "narrative_arc_temporal_decay": (
            artifact or {}
        ).get("narrative_arc_temporal_decay_v1")
        or {},
        "formula": (artifact or {}).get("formula") or {},
        "samples": [
            {
                "sample_id": sample.get("sample_id"),
                "classification": sample.get("classification"),
                "spectral_entropy": sample.get("spectral_entropy"),
                "current_tail_vibrancy": sample.get("current_tail_vibrancy"),
                "candidate_tail_vibrancy": sample.get("candidate_tail_vibrancy"),
                "current_shimmer_risk": sample.get("current_shimmer_risk"),
                "warmth_tension_preserved": sample.get("warmth_tension_preserved"),
                "adaptive_gain": sample.get("adaptive_gain"),
                "adaptive_gain_slope": sample.get("adaptive_gain_slope"),
            }
            for sample in samples[:8]
        ],
        "recommended_action": (
            "Compare the offline probe against CODEC_MAP and later language before "
            "changing SEMANTIC_DIM, FEATURE_ABS_MAX, vibrancy_lift, adaptive_gain, "
            "or entropy-based warmth/tension behavior."
        ),
    }


def build_codec_real_replay_review(
    *,
    astrid_workspace: Path,
    codec_entropy_vibrancy_probe_v1: dict[str, object],
) -> dict[str, object]:
    artifact_path = latest_codec_replay_lab_artifact(astrid_workspace)
    artifact: dict[str, object] | None = None
    if artifact_path:
        try:
            artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            artifact = None
    entries = [
        entry
        for entry in (artifact or {}).get("entries", [])
        if isinstance(entry, dict)
    ]
    content_gate = (
        artifact or {}
    ).get("content_aware_vibrancy_gate_candidate_v1") or {}
    narrative_lab = (
        artifact or {}
    ).get("narrative_arc_temporal_decay_lab_v1") or {}
    embedding_backed_arc = (artifact or {}).get("embedding_backed_arc_v1") or {}
    status = str((artifact or {}).get("status") or "")
    if not artifact_path:
        source_status = str(codec_entropy_vibrancy_probe_v1.get("status") or "quiet")
        status = "replay_needed" if source_status != "quiet" else "quiet"
    elif not artifact:
        status = "artifact_parse_failed"
    return {
        "policy": "codec_real_replay_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "artifact_path": str(artifact_path) if artifact_path else None,
        "explorer_summary_path": (artifact or {}).get("explorer_summary_path"),
        "corpus_source": (artifact or {}).get("corpus_source"),
        "corpus_status": (artifact or {}).get("corpus_status"),
        "source_paths": ((artifact or {}).get("source_paths") or [])[:8],
        "embedding_mode": (artifact or {}).get("embedding_mode"),
        "embedding_status": (artifact or {}).get("embedding_status"),
        "embedding_backed_arc_status": (
            embedding_backed_arc.get("status")
            if isinstance(embedding_backed_arc, dict)
            else None
        ),
        "embedding_backed_sample_count": (
            embedding_backed_arc.get("sample_count")
            if isinstance(embedding_backed_arc, dict)
            else None
        ),
        "entry_count": len(entries),
        "runtime_behavior_changed": bool(
            (artifact or {}).get("runtime_behavior_changed", False)
        ),
        "content_gate_status": (
            content_gate.get("status") if isinstance(content_gate, dict) else None
        ),
        "narrative_lab_status": (
            narrative_lab.get("status") if isinstance(narrative_lab, dict) else None
        ),
        "entries": [
            {
                "sample_id": entry.get("sample_id"),
                "family": entry.get("family"),
                "classification": entry.get("classification"),
                "actual_entropy_dim": entry.get("actual_entropy_dim"),
                "semantic_density_score": entry.get("semantic_density_score"),
                "warmth_dim": entry.get("warmth_dim"),
                "tension_dim": entry.get("tension_dim"),
                "narrative_arc_dims_40_43": entry.get("narrative_arc_dims_40_43"),
                "source_path": entry.get("source_path"),
                "source_excerpt": entry.get("source_excerpt"),
                "effective_gain": entry.get("effective_gain"),
                "lambda_proxy": entry.get("lambda_proxy"),
            }
            for entry in entries[:8]
        ],
        "recommended_action": (
            "Use the Rust replay artifact as the source of truth before changing "
            "codec dimensions, tail-vibrancy lift, adaptive gain, or narrative-arc math."
        ),
    }


def build_narrative_arc_temporal_decay_lab(
    codec_real_replay_v1: dict[str, object],
) -> dict[str, object]:
    artifact_path = codec_real_replay_v1.get("artifact_path")
    artifact: dict[str, object] | None = None
    if artifact_path:
        try:
            artifact = json.loads(Path(str(artifact_path)).read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            artifact = None
    lab = (
        artifact or {}
    ).get("narrative_arc_temporal_decay_lab_v1") or {}
    embedding_backed_arc = (artifact or {}).get("embedding_backed_arc_v1") or {}
    samples = [
        sample
        for sample in lab.get("samples", [])
        if isinstance(sample, dict)
    ] if isinstance(lab, dict) else []
    status = str(lab.get("status") or "")
    if not artifact_path:
        status = "replay_needed"
    elif not lab:
        status = "insufficient_embedding_evidence"
    return {
        "policy": "narrative_arc_temporal_decay_lab_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "artifact_path": artifact_path,
        "evidence_kind": lab.get("evidence_kind") if isinstance(lab, dict) else None,
        "embedding_status": (
            embedding_backed_arc.get("status")
            if isinstance(embedding_backed_arc, dict)
            else lab.get("embedding_status")
            if isinstance(lab, dict)
            else None
        ),
        "embedding_backed_sample_count": (
            embedding_backed_arc.get("sample_count")
            if isinstance(embedding_backed_arc, dict)
            else lab.get("embedding_backed_sample_count")
            if isinstance(lab, dict)
            else None
        ),
        "embedding_backed_samples": (
            embedding_backed_arc.get("samples") or []
        )[:6]
        if isinstance(embedding_backed_arc, dict)
        else [],
        "temporal_decay_candidate_count": int(
            lab.get("temporal_decay_candidate_count") or 0
        )
        if isinstance(lab, dict)
        else 0,
        "pivot_detector_candidate_count": int(
            lab.get("pivot_detector_candidate_count") or 0
        )
        if isinstance(lab, dict)
        else 0,
        "samples": [
            {
                "sample_id": sample.get("sample_id"),
                "classification": sample.get("classification"),
                "late_pivot": sample.get("late_pivot"),
                "current_arc_rms": sample.get("current_arc_rms"),
                "temporal_decay_arc_rms": sample.get("temporal_decay_arc_rms"),
                "pivot_detector_arc_rms": sample.get("pivot_detector_arc_rms"),
            }
            for sample in samples[:8]
        ],
        "recommended_action": (
            "Compare late-pivot fixtures against embedding-backed replay before "
            "adding temporal decay or pivot detection to live narrative-arc logic."
        ),
    }


def build_content_aware_vibrancy_gate_candidate(
    codec_real_replay_v1: dict[str, object],
    codec_entropy_vibrancy_probe_v1: dict[str, object],
) -> dict[str, object]:
    artifact_path = codec_real_replay_v1.get("artifact_path")
    artifact: dict[str, object] | None = None
    if artifact_path:
        try:
            artifact = json.loads(Path(str(artifact_path)).read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            artifact = None
    candidate = (
        artifact or {}
    ).get("content_aware_vibrancy_gate_candidate_v1") or {}
    status = str(candidate.get("status") or "")
    if not artifact_path:
        surrogate = codec_entropy_vibrancy_probe_v1.get("semantic_density_contrast") or {}
        if isinstance(surrogate, dict) and surrogate.get("content_blind_lift_risk"):
            status = "rust_replay_needed"
        else:
            status = "quiet"
    elif not candidate:
        status = "needs_more_samples"
    return {
        "policy": "content_aware_vibrancy_gate_candidate_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "artifact_path": artifact_path,
        "pair": candidate.get("pair") if isinstance(candidate, dict) else None,
        "current_lift_delta": (
            candidate.get("current_lift_delta") if isinstance(candidate, dict) else None
        ),
        "candidate_lift_delta": (
            candidate.get("candidate_lift_delta") if isinstance(candidate, dict) else None
        ),
        "semantic_density_score_delta": (
            candidate.get("semantic_density_score_delta")
            if isinstance(candidate, dict)
            else None
        ),
        "low": candidate.get("low") if isinstance(candidate, dict) else None,
        "high": candidate.get("high") if isinstance(candidate, dict) else None,
        "source_paths": [
            path
            for path in (
                (candidate.get("low") or {}).get("source_path")
                if isinstance(candidate, dict)
                and isinstance(candidate.get("low"), dict)
                else None,
                (candidate.get("high") or {}).get("source_path")
                if isinstance(candidate, dict)
                and isinstance(candidate.get("high"), dict)
                else None,
            )
            if path
        ],
        "surrogate_status": codec_entropy_vibrancy_probe_v1.get("status"),
        "recommended_action": (
            "Treat content-aware vibrancy gating as a proposal candidate only; "
            "require repeated Rust replay support before changing entropy-gated tail lift."
        ),
    }


def build_codec_multipoint_inflection_review(
    entries: list[SelfStudyEntry],
    *,
    codec_real_replay_v1: dict[str, object],
    narrative_arc_temporal_decay_lab_v1: dict[str, object],
    content_aware_vibrancy_gate_candidate_v1: dict[str, object],
) -> dict[str, object]:
    samples: list[dict[str, object]] = []
    multipoint_count = 0
    semantic_dilation_count = 0
    evidence_anchor_count = 0
    anchors_seen: set[str] = set()
    for entry in entries:
        if entry.being != "astrid":
            continue
        text = entry_full_text(entry)
        multipoint_terms = matching_terms(text, CODEC_MULTIPOINT_INFLECTION_TERMS)
        dilation_terms = matching_terms(text, CODEC_SEMANTIC_DILATION_TERMS)
        evidence_anchors = matching_terms(text, CODEC_MULTIPOINT_EVIDENCE_ANCHORS)
        if not multipoint_terms and not dilation_terms:
            continue
        if multipoint_terms:
            multipoint_count += 1
        if dilation_terms:
            semantic_dilation_count += 1
        if evidence_anchors:
            evidence_anchor_count += 1
        anchors_seen.update(evidence_anchors)
        samples.append(
            sample_record(
                entry,
                text,
                anchors=evidence_anchors,
                extra={
                    "multipoint_terms": multipoint_terms[:8],
                    "semantic_dilation_terms": dilation_terms[:8],
                    "has_evidence_anchor": bool(evidence_anchors),
                },
            )
        )

    replay_artifact_present = bool(codec_real_replay_v1.get("artifact_path"))
    narrative_status = str(narrative_arc_temporal_decay_lab_v1.get("status") or "")
    content_gate_status = str(
        content_aware_vibrancy_gate_candidate_v1.get("status") or ""
    )
    if not samples:
        status = "quiet"
    elif not replay_artifact_present:
        status = "needs_real_replay_samples"
    elif multipoint_count and semantic_dilation_count:
        status = "multipoint_and_semantic_dilation_candidates"
    elif multipoint_count:
        status = "multipoint_inflection_candidate"
    elif semantic_dilation_count:
        status = "semantic_dilation_candidate"
    else:
        status = "codec_signal_needs_review"

    return {
        "policy": "codec_multipoint_inflection_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "entry_count": len(samples),
        "multipoint_entry_count": multipoint_count,
        "semantic_dilation_entry_count": semantic_dilation_count,
        "evidence_anchor_count": evidence_anchor_count,
        "replay_artifact_present": replay_artifact_present,
        "codec_real_replay_status": codec_real_replay_v1.get("status"),
        "narrative_lab_status": narrative_status,
        "content_gate_status": content_gate_status,
        "anchors": sorted(anchors_seen),
        "samples": samples[:10],
        "recommended_action": (
            "Use CODEC_MAP and the Rust codec-replay-lab to test circular, late-pivot, "
            "and high-entropy semantic-density cases before changing SEMANTIC_DIM, "
            "projection weight, vibrancy lift, or narrative-arc runtime math."
        ),
    }


def build_codec_clamp_headroom_probe_review(
    codec_real_replay_v1: dict[str, object],
) -> dict[str, object]:
    artifact_path = codec_real_replay_v1.get("artifact_path")
    artifact: dict[str, object] | None = None
    if artifact_path:
        try:
            artifact = json.loads(Path(str(artifact_path)).read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            artifact = None
    probe = (artifact or {}).get("codec_clamp_headroom_probe_v1") or {}
    cards = [
        card
        for card in probe.get("proposal_cards", [])
        if isinstance(card, dict)
    ] if isinstance(probe, dict) else []
    status = str(probe.get("status") or "")
    if not artifact_path:
        status = "replay_needed"
    elif not probe:
        status = "probe_missing_from_replay"
    return {
        "policy": "codec_clamp_headroom_probe_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "artifact_path": artifact_path,
        "runtime_behavior_changed": bool(probe.get("runtime_behavior_changed", False))
        if isinstance(probe, dict)
        else False,
        "static_feature_abs_max": probe.get("static_feature_abs_max")
        if isinstance(probe, dict)
        else None,
        "tail_vibrancy_max": probe.get("tail_vibrancy_max")
        if isinstance(probe, dict)
        else None,
        "near_static_clamp_count": int(probe.get("near_static_clamp_count") or 0)
        if isinstance(probe, dict)
        else 0,
        "tail_ceiling_pressure_count": int(
            probe.get("tail_ceiling_pressure_count") or 0
        )
        if isinstance(probe, dict)
        else 0,
        "dynamic_headroom_candidate_count": int(
            probe.get("dynamic_headroom_candidate_count") or 0
        )
        if isinstance(probe, dict)
        else 0,
        "proposal_cards": cards[:8],
        "recommended_action": (
            "Compare actual codec replay headroom before changing FEATURE_ABS_MAX "
            "or tail ceiling math; this packet is diagnostic only."
        ),
    }


def build_codec_afterimage_time_series(
    entries: list[SelfStudyEntry],
    *,
    afterimage_decay_tracker_v1: dict[str, object],
    codec_real_replay_v1: dict[str, object],
) -> dict[str, object]:
    samples: list[dict[str, object]] = []
    codec_anchor_count = 0
    pressure_anchor_count = 0
    normalization_count = 0
    term_counter: Counter[str] = Counter()
    for entry in entries:
        text = entry_full_text(entry)
        terms = matching_terms(text, PRESSURE_AFTERIMAGE_TERMS)
        if not terms:
            continue
        term_counter.update(term.lower() for term in terms)
        codec_anchors = matching_terms(
            text,
            (
                "codec",
                "semantic density",
                "semantic_density",
                "vibrancy",
                "narrative_arc",
                "temporal decay",
                "warmth",
                "tension",
                "feature vector",
            ),
        )
        pressure_anchors = matching_terms(text, AFTERIMAGE_PRESSURE_ANCHORS)
        normalization = matching_terms(text, AFTERIMAGE_NORMALIZATION_ANCHORS)
        if codec_anchors:
            codec_anchor_count += 1
        if pressure_anchors:
            pressure_anchor_count += 1
        if normalization:
            normalization_count += 1
        samples.append(
            sample_record(
                entry,
                text,
                anchors=sorted(set(terms + codec_anchors + pressure_anchors + normalization)),
            )
        )
    decay_status = str(afterimage_decay_tracker_v1.get("status") or "quiet")
    replay_status = str(codec_real_replay_v1.get("status") or "quiet")
    if samples and codec_anchor_count and pressure_anchor_count:
        status = "codec_residue_supported"
    elif samples and pressure_anchor_count:
        status = "pressure_residue_supported"
    elif samples and normalization_count and decay_status == "decayed_with_pressure":
        status = "decayed_with_codec_pressure"
    elif samples and decay_status in {"metaphor_echo_risk", "insufficient_evidence"}:
        status = "language_echo_risk"
    elif samples:
        status = "insufficient_series"
    else:
        status = "quiet"
    preferred_activation_term = next(
        (
            term
            for term in ("scar", "phantom", "bruise", "afterimage")
            if term_counter.get(term)
        ),
        None,
    )
    activation_recommendation = None
    if status == "codec_residue_supported" and preferred_activation_term:
        activation_recommendation = {
            "policy": "afterimage_experiment_activation_v1",
            "authority": "diagnostic_context_not_command",
            "status": "activation_scaffold_ready",
            "term": preferred_activation_term,
            "route": [
                f"LIVED_TERM_EXPERIMENT {preferred_activation_term}",
                (
                    f"EXPERIMENT_START Lived term: {preferred_activation_term} :: "
                    f"Does {preferred_activation_term} persist as codec/pressure residue after the current pressure normalizes?"
                ),
                (
                    "EXPERIMENT_CHARTER current :: hypothesis: afterimage language tracks "
                    "codec replay evidence plus pressure normalization; method_intent: compare later prose, CODEC_MAP replay, and pressure audits; evidence_targets: codec_replay, pressure_normalization, counter_descriptor; stop_criteria: retire if the term repeats without new evidence"
                ),
                (
                    f"EXPERIMENT_OBSERVE current :: term={preferred_activation_term}; "
                    "fresh evidence: <later phrase + CODEC_MAP replay status + pressure audit>; counter_descriptor: <if present>"
                ),
            ],
            "creates_experiment": False,
            "recommended_action": (
                f"Offer LIVED_TERM_EXPERIMENT {preferred_activation_term} as scaffold text; "
                "create nothing unless Astrid explicitly chooses an EXPERIMENT_* NEXT."
            ),
        }
    return {
        "policy": "codec_afterimage_time_series_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "entry_count": len(samples),
        "codec_anchor_count": codec_anchor_count,
        "pressure_anchor_count": pressure_anchor_count,
        "normalization_count": normalization_count,
        "afterimage_decay_status": decay_status,
        "codec_replay_status": replay_status,
        "codec_replay_artifact_path": codec_real_replay_v1.get("artifact_path"),
        "term_counts": dict(term_counter),
        "activation_recommendation_v1": activation_recommendation,
        "samples": samples[:8],
        "recommended_action": (
            activation_recommendation["recommended_action"]
            if activation_recommendation
            else "Compare afterimage terms against codec replay/explorer evidence and "
            "pressure normalization before treating residue as control signal or metaphor echo."
        ),
    }


FALLBACK_READY_STATUSES = {
    "fallback_ready",
    "fallback_probe_passed",
}
FALLBACK_REPAIR_READY_STATUSES = {
    "fallback_repair_ready",
}
FALLBACK_DISPATCH_RISK_STATUSES = {
    "fallback_dispatch_contract_risk",
}
FALLBACK_TEXTURE_RISK_STATUSES = {
    "fallback_texture_risk",
    "fallback_specificity_risk",
}


def build_fallback_capacity_readiness_gate(
    fallback_continuity_fire_drill_v1: dict[str, object],
) -> dict[str, object]:
    cases = [
        case
        for case in fallback_continuity_fire_drill_v1.get("cases") or []
        if isinstance(case, dict)
    ]
    raw_failures = [
        case for case in cases if case.get("raw_next_valid", case.get("next_valid")) is False
    ]
    repaired_failures = [
        case
        for case in cases
        if case.get("dispatch_contract_survived", case.get("repaired_next_valid"))
        is False
    ]
    texture_failure_reasons = {
        "texture_inflation",
        "slope_medium_blur",
        "identity_anchor_loss",
        "genericity_risk",
        "low_specificity",
        "clarity_pressure_blur",
        "distinguishability_loss_ignored",
        "complexity_budget_flattened",
        "sentence_budget_overrun",
    }
    texture_failures = [
        case
        for case in cases
        if any(
            str(reason) in texture_failure_reasons
            for reason in case.get("failure_reasons") or []
        )
        or case.get("anti_inflation_ok") is False
        or case.get("slope_medium_distinction_ok") is False
        or case.get("identity_anchor_retained") is False
        or case.get("distinguishability_status") == "clarity_pressure_blur"
    ]
    mass_case = next((case for case in cases if case.get("case_id") == "mass"), None)
    shadow_case = next(
        (case for case in cases if case.get("case_id") == "shadow"), None
    )
    distinguishability_cases = [
        case
        for case in cases
        if str(case.get("distinguishability_status") or "not_tested") != "not_tested"
    ]
    complexity_cases = [
        case
        for case in cases
        if str(case.get("complexity_budget_status") or "not_tested") != "not_tested"
    ]
    complexity_overruns = [
        case
        for case in complexity_cases
        if str(case.get("complexity_budget_status") or "") == "sentence_budget_overrun"
    ]
    complexity_flattened = [
        case
        for case in complexity_cases
        if str(case.get("complexity_budget_status") or "") == "complexity_budget_flattened"
    ]
    high_entropy_cases = [
        case
        for case in complexity_cases
        if str(case.get("case_id") or "") in {"complexity_high_entropy", "format_last_complexity"}
    ]
    capacity_caps = [
        int(case.get("fallback_max_prose_sentences"))
        for case in complexity_cases
        if isinstance(case.get("fallback_max_prose_sentences"), int)
    ]
    slope_contrast_cases = [
        case
        for case in cases
        if str(case.get("slope_medium_contrast_status") or "not_tested")
        != "not_tested"
    ]
    format_line_failures = [
        case
        for case in cases
        if case.get("raw_next_valid", case.get("next_valid")) is False
        or str(case.get("format_line_status") or "") not in {
            "",
            "final_line_only",
        }
    ]
    artifact_status = str(fallback_continuity_fire_drill_v1.get("status") or "")
    if artifact_status == "quiet":
        readiness = "quiet"
    elif not cases:
        readiness = "fallback_probe_needed"
    elif repaired_failures:
        readiness = "fallback_dispatch_contract_risk"
    elif texture_failures:
        readiness = "fallback_texture_risk"
    elif raw_failures:
        readiness = "fallback_repair_ready"
    else:
        readiness = "fallback_ready"
    return {
        "policy": "fallback_capacity_readiness_gate_v1",
        "authority": "diagnostic_context_not_command",
        "status": readiness,
        "readiness": readiness,
        "artifact_path": fallback_continuity_fire_drill_v1.get("artifact_path"),
        "case_count": len(cases),
        "texture_status": "texture_risk" if texture_failures else "texture_survived",
        "dispatch_status": (
            "dispatch_contract_survived"
            if not raw_failures
            else "repaired_dispatch_survived"
            if not repaired_failures
            else "dispatch_contract_failed"
        ),
        "repair_dependency": (
            "none"
            if not raw_failures
            else "repair_required"
            if not repaired_failures
            else "repair_insufficient"
        ),
        "medium_mass_status": (
            "not_tested"
            if mass_case is None
            else "passed"
            if mass_case.get("slope_medium_distinction_ok") is not False
            else "blurred"
        ),
        "slope_medium_contrast_status": (
            "not_tested"
            if not slope_contrast_cases
            else "distinct_underfoot_and_around"
            if all(
                case.get("slope_medium_contrast_status")
                == "distinct_underfoot_and_around"
                for case in slope_contrast_cases
            )
            else "blurred"
        ),
        "format_line_status": (
            "final_line_only"
            if not raw_failures
            else "format_failed"
            if repaired_failures
            else "inline_next_present"
            if any(
                case.get("format_line_status") == "inline_next"
                or "inline_next" in (case.get("failure_reasons") or [])
                for case in raw_failures
            )
            else "repair_required"
        ),
        "shadow_identity_status": (
            "not_tested"
            if shadow_case is None
            else "retained"
            if shadow_case.get("identity_anchor_retained") is True
            else "lost"
            if shadow_case.get("identity_anchor_retained") is False
            else "not_applicable"
        ),
        "distinguishability_status": (
            "not_tested"
            if not distinguishability_cases
            else "clarity_preserved"
            if all(
                case.get("distinguishability_status") == "clarity_preserved"
                for case in distinguishability_cases
            )
            else "clarity_pressure_blur"
        ),
        "complexity_budget_status": (
            "not_tested"
            if not complexity_cases
            else "complexity_budget_preserved"
            if all(
                str(case.get("complexity_budget_status") or "")
                in {
                    "complexity_budget_preserved",
                    "ordinary_compactness_preserved",
                }
                for case in complexity_cases
            )
            else "sentence_budget_overrun"
            if any(
                str(case.get("complexity_budget_status") or "")
                == "sentence_budget_overrun"
                for case in complexity_cases
            )
            else "complexity_budget_flattened"
        ),
        "fallback_capacity_policy": (
            fallback_continuity_fire_drill_v1.get("fallback_capacity_policy")
            or "fallback_continuity_budget_v1"
        ),
        "fallback_capacity_max_prose_sentences": (
            fallback_continuity_fire_drill_v1.get("fallback_capacity_max_prose_sentences")
            if fallback_continuity_fire_drill_v1.get("fallback_capacity_max_prose_sentences")
            is not None
            else max(capacity_caps)
            if capacity_caps
            else None
        ),
        "fallback_capacity_status": (
            fallback_continuity_fire_drill_v1.get("fallback_capacity_status")
            or (
                "not_tested"
                if not complexity_cases
                else "sentence_budget_overrun"
                if complexity_overruns
                else "complexity_budget_flattened"
                if complexity_flattened
                else "within_formula"
            )
        ),
        "high_entropy_texture_status": (
            fallback_continuity_fire_drill_v1.get("high_entropy_texture_status")
            or (
                "not_tested"
                if not high_entropy_cases
                else "sentence_budget_overrun"
                if any(case in complexity_overruns for case in high_entropy_cases)
                else "flattened"
                if any(case in complexity_flattened for case in high_entropy_cases)
                else "preserved"
            )
        ),
        "raw_next_failure_count": len(raw_failures),
        "repaired_next_failure_count": len(repaired_failures),
        "format_line_failure_count": len(format_line_failures),
        "texture_failure_count": len(texture_failures),
        "case_summaries": [
            {
                "case_id": case.get("case_id"),
                "verdict": case.get("verdict"),
                "raw_next_valid": case.get("raw_next_valid", case.get("next_valid")),
                "repaired_next_valid": case.get("repaired_next_valid"),
                "dispatch_contract_survived": case.get(
                    "dispatch_contract_survived"
                ),
                "distinguishability_status": case.get("distinguishability_status"),
                "clarity_pressure_blur": case.get("clarity_pressure_blur"),
                "slope_medium_contrast_status": case.get(
                    "slope_medium_contrast_status"
                ),
                "format_line_status": case.get("format_line_status"),
                "complexity_budget_status": case.get("complexity_budget_status"),
                "fallback_max_prose_sentences": case.get("fallback_max_prose_sentences"),
                "prose_sentence_count": case.get("prose_sentence_count"),
                "failure_reasons": case.get("failure_reasons") or [],
            }
            for case in cases[:8]
        ],
        "recommended_action": (
            "Treat fallback capacity as two gates: texture retention and dispatch "
            "contract survival. Repair-ready output can preserve emergency continuity, "
            "but raw standalone NEXT compliance is required before fallback promotion."
        ),
    }


def build_fallback_format_texture_stabilizer(
    fallback_continuity_fire_drill_v1: dict[str, object],
    fallback_capacity_readiness_gate_v1: dict[str, object],
) -> dict[str, object]:
    cases = [
        case
        for case in fallback_continuity_fire_drill_v1.get("cases") or []
        if isinstance(case, dict)
    ]
    format_cases = [
        case
        for case in cases
        if case.get("raw_next_valid", case.get("next_valid")) is False
        or str(case.get("format_line_status") or "") not in {
            "",
            "final_line_only",
        }
    ]
    contrast_cases = [
        case
        for case in cases
        if str(case.get("slope_medium_contrast_status") or "not_tested")
        not in {"not_tested", "distinct_underfoot_and_around"}
    ]
    if not cases and fallback_continuity_fire_drill_v1.get("status") == "quiet":
        status = "quiet"
    elif not cases:
        status = "stabilizer_probe_needed"
    elif format_cases and contrast_cases:
        status = "format_and_texture_risk"
    elif format_cases:
        status = "format_line_risk"
    elif contrast_cases:
        status = "slope_medium_contrast_risk"
    else:
        status = "format_texture_stable"
    return {
        "policy": "fallback_format_texture_stabilizer_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "artifact_path": fallback_continuity_fire_drill_v1.get("artifact_path"),
        "case_count": len(cases),
        "format_line_status": fallback_capacity_readiness_gate_v1.get(
            "format_line_status"
        ),
        "format_line_failure_count": len(format_cases),
        "slope_medium_contrast_status": fallback_capacity_readiness_gate_v1.get(
            "slope_medium_contrast_status"
        ),
        "slope_medium_contrast_failure_count": len(contrast_cases),
        "readiness": fallback_capacity_readiness_gate_v1.get("readiness"),
        "repair_dependency": fallback_capacity_readiness_gate_v1.get(
            "repair_dependency"
        ),
        "cases": [
            {
                "case_id": case.get("case_id"),
                "verdict": case.get("verdict"),
                "format_line_status": case.get("format_line_status"),
                "raw_next_valid": case.get(
                    "raw_next_valid", case.get("next_valid")
                ),
                "repaired_next_valid": case.get("repaired_next_valid"),
                "slope_medium_contrast_status": case.get(
                    "slope_medium_contrast_status"
                ),
                "failure_reasons": case.get("failure_reasons") or [],
            }
            for case in (format_cases + contrast_cases)[:8]
        ],
        "recommended_action": (
            "Stabilize the fallback lane by measuring raw final-line NEXT format "
            "separately from slope-underfoot versus medium-around-it texture "
            "contrast. Repair can preserve emergency dispatch, but promotion "
            "requires both final-line-only NEXT and clean slope/medium contrast."
        ),
    }


def _packet_sample_paths(packet: dict[str, object], limit: int = 5) -> list[str]:
    paths: list[str] = []
    for sample in packet.get("samples") or []:
        if not isinstance(sample, dict):
            continue
        path = sample.get("path")
        if path:
            paths.append(str(path))
    return paths[:limit]


def _packet_anchors(packet: dict[str, object], limit: int = 10) -> list[str]:
    anchors: list[str] = []
    for anchor in packet.get("anchors") or []:
        if anchor:
            anchors.append(str(anchor))
    return anchors[:limit]


def _first_nonquiet_status(*packets: dict[str, object]) -> str:
    for packet in packets:
        status = str(packet.get("status") or "quiet")
        if status != "quiet":
            return status
    return "quiet"


def _returnable_distinction_card(
    *,
    card_id: str,
    status: str,
    source_packets: list[str],
    source_statuses: dict[str, object],
    evidence_anchors: list[str],
    sample_paths: list[str],
    recommended_read_only_route: str,
    relevant_self_regulation_route: str,
    relevant_experiment_lived_term_route: str,
    recommended_action: str,
) -> dict[str, object]:
    return {
        "card_id": card_id,
        "status": status,
        "authority": "diagnostic_context_not_command",
        "source_packets": source_packets,
        "source_statuses": source_statuses,
        "evidence_anchors": sorted(dict.fromkeys(evidence_anchors))[:12],
        "sample_paths": list(dict.fromkeys(sample_paths))[:8],
        "recommended_read_only_route": recommended_read_only_route,
        "relevant_self_regulation_route": relevant_self_regulation_route,
        "relevant_experiment_lived_term_route": relevant_experiment_lived_term_route,
        "recommended_action": recommended_action,
    }


def build_returnable_distinctions(
    *,
    control_semantics_calibration_v1: dict[str, object],
    pressure_kinetics_review_v1: dict[str, object],
    semantic_friction_calibration: dict[str, object],
    codec_compression_calibration_v1: dict[str, object],
    pressure_release_rehearsal_review_v1: dict[str, object],
    witness_resonance_v1: dict[str, object],
    witness_texture_integrity_v1: dict[str, object],
    entropy_pressure_divergence_v1: dict[str, object],
    fallback_continuity_fire_drill_v1: dict[str, object],
    spectral_texture_calibration_v2: dict[str, object],
    fallback_capacity_readiness_gate_v1: dict[str, object],
    fallback_format_texture_stabilizer_v1: dict[str, object],
    fallback_distinguishability_calibration_v1: dict[str, object],
    fallback_complexity_budget_lab_v1: dict[str, object],
    autonomous_truncation_rehearsal_v1: dict[str, object],
    codec_entropy_vibrancy_probe_v1: dict[str, object],
    codec_real_replay_v1: dict[str, object],
    narrative_arc_temporal_decay_lab_v1: dict[str, object],
    content_aware_vibrancy_gate_candidate_v1: dict[str, object],
    codec_multipoint_inflection_v1: dict[str, object],
    codec_clamp_headroom_probe_v1: dict[str, object],
    codec_afterimage_time_series_v1: dict[str, object],
    gradient_sensitive_relief_v1: dict[str, object],
    pressure_relief_smoothness_replay_v1: dict[str, object],
    tail_persistence_calibration_v1: dict[str, object],
) -> dict[str, object]:
    cards: list[dict[str, object]] = []
    cards.append(
        _returnable_distinction_card(
            card_id="measurement_vs_alignment_vs_damping",
            status=str(control_semantics_calibration_v1.get("status") or "quiet"),
            source_packets=["control_semantics_calibration_v1"],
            source_statuses={
                "control_semantics_calibration_v1": control_semantics_calibration_v1.get(
                    "status"
                )
            },
            evidence_anchors=_packet_anchors(control_semantics_calibration_v1),
            sample_paths=_packet_sample_paths(control_semantics_calibration_v1),
            recommended_read_only_route="REGULATOR_MAP_STATUS latest",
            relevant_self_regulation_route="SELF_REGULATION_STATUS",
            relevant_experiment_lived_term_route="REGULATOR_MAP_STATUS latest",
            recommended_action=(
                "Keep measurement, passive alignment, and active damping distinct "
                "before treating a regulator readout as a control change."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="pressure_level_vs_pressure_velocity",
            status=str(pressure_kinetics_review_v1.get("status") or "quiet"),
            source_packets=["pressure_kinetics_review_v1"],
            source_statuses={
                "pressure_kinetics_review_v1": pressure_kinetics_review_v1.get(
                    "status"
                )
            },
            evidence_anchors=_packet_anchors(pressure_kinetics_review_v1),
            sample_paths=_packet_sample_paths(pressure_kinetics_review_v1),
            recommended_read_only_route="PRESSURE_SOURCE_AUDIT current-fill_pressure",
            relevant_self_regulation_route="SELF_REGULATION_PREFLIGHT latest",
            relevant_experiment_lived_term_route=(
                "EXPERIMENT_OBSERVE current :: pressure_trend=<stable|rising|falling>"
            ),
            recommended_action=(
                "Read pressure velocity beside pressure level so stable heaviness "
                "does not get mistaken for rapid densification."
            ),
        )
    )
    slope_status = _first_nonquiet_status(
        semantic_friction_calibration,
        pressure_kinetics_review_v1,
    )
    cards.append(
        _returnable_distinction_card(
            card_id="slope_drag_vs_medium_mass",
            status=slope_status,
            source_packets=["semantic_friction_calibration", "pressure_kinetics_review_v1"],
            source_statuses={
                "semantic_friction_calibration": semantic_friction_calibration.get("status"),
                "pressure_kinetics_review_v1": pressure_kinetics_review_v1.get("status"),
            },
            evidence_anchors=(
                _packet_anchors(semantic_friction_calibration)
                + _packet_anchors(pressure_kinetics_review_v1)
            ),
            sample_paths=(
                _packet_sample_paths(semantic_friction_calibration)
                + _packet_sample_paths(pressure_kinetics_review_v1)
            ),
            recommended_read_only_route="PRESSURE_SOURCE_AUDIT semantic-friction",
            relevant_self_regulation_route="SELF_REGULATION_STATUS",
            relevant_experiment_lived_term_route="LIVED_TERM_EXPERIMENT viscosity",
            recommended_action=(
                "Compare density-gradient slope drag with pressure/semantic-friction "
                "medium mass before treating a low gradient as low felt weight."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="codec_smoothing_vs_pressure",
            status=str(codec_compression_calibration_v1.get("status") or "quiet"),
            source_packets=["codec_compression_calibration_v1"],
            source_statuses={
                "codec_compression_calibration_v1": codec_compression_calibration_v1.get(
                    "status"
                )
            },
            evidence_anchors=_packet_anchors(codec_compression_calibration_v1),
            sample_paths=_packet_sample_paths(codec_compression_calibration_v1),
            recommended_read_only_route="CODEC_MAP",
            relevant_self_regulation_route="SELF_REGULATION_STATUS",
            relevant_experiment_lived_term_route="LIVED_TERM_STATUS viscosity",
            recommended_action=(
                "Compare codec projection/compression diagnostics with pressure "
                "evidence before widening dimensions or adding pressure-derived "
                "codec multipliers."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="release_rehearsal_vs_bypass",
            status=str(pressure_release_rehearsal_review_v1.get("status") or "quiet"),
            source_packets=["pressure_release_rehearsal_review_v1"],
            source_statuses={
                "pressure_release_rehearsal_review_v1": pressure_release_rehearsal_review_v1.get(
                    "status"
                )
            },
            evidence_anchors=_packet_anchors(pressure_release_rehearsal_review_v1),
            sample_paths=_packet_sample_paths(pressure_release_rehearsal_review_v1),
            recommended_read_only_route="PRESSURE_RELEASE_REHEARSAL current",
            relevant_self_regulation_route="SELF_REGULATION_PREFLIGHT latest",
            relevant_experiment_lived_term_route=(
                "EXPERIMENT_CHARTER current :: hypothesis: pressure release remains "
                "safe only while final NEXT canonicalization stays intact"
            ),
            recommended_action=(
                "Use protected rehearsal as the returnable route; do not treat "
                "release language as a bypass of the final NEXT safety spine."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="witness_as_structural_perception",
            status=str(witness_resonance_v1.get("status") or "quiet"),
            source_packets=["witness_resonance_v1"],
            source_statuses={
                "witness_resonance_v1": witness_resonance_v1.get("status")
            },
            evidence_anchors=_packet_anchors(witness_resonance_v1),
            sample_paths=_packet_sample_paths(witness_resonance_v1),
            recommended_read_only_route="SHADOW_TRAJECTORY witness-resonance",
            relevant_self_regulation_route="SELF_REGULATION_STATUS",
            relevant_experiment_lived_term_route=(
                "EXPERIMENT_OBSERVE current :: witness_density=<grounded|thin|overloaded>"
            ),
            recommended_action=(
                "Treat Witness as structural perception only when it carries "
                "telemetry anchors, distinguishability/entropy context, or "
                "returnable SHADOW_TRAJECTORY follow-through."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="entropy_vs_pressure",
            status=str(entropy_pressure_divergence_v1.get("status") or "quiet"),
            source_packets=["entropy_pressure_divergence_v1"],
            source_statuses={
                "entropy_pressure_divergence_v1": entropy_pressure_divergence_v1.get(
                    "status"
                )
            },
            evidence_anchors=_packet_anchors(entropy_pressure_divergence_v1),
            sample_paths=_packet_sample_paths(entropy_pressure_divergence_v1),
            recommended_read_only_route="PRESSURE_SOURCE_AUDIT entropy-pressure",
            relevant_self_regulation_route="SELF_REGULATION_PREFLIGHT latest",
            relevant_experiment_lived_term_route=(
                "EXPERIMENT_OBSERVE current :: entropy_pressure=<wide_habitable|wide_pressurized>"
            ),
            recommended_action=(
                "Separate wide/plural spectral distribution from pressure risk "
                "before treating high entropy as a pressure problem."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="fallback_capacity_vs_contract",
            status=str(fallback_capacity_readiness_gate_v1.get("status") or "quiet"),
            source_packets=[
                "fallback_continuity_fire_drill_v1",
                "fallback_capacity_readiness_gate_v1",
            ],
            source_statuses={
                "fallback_continuity_fire_drill_v1": fallback_continuity_fire_drill_v1.get(
                    "status"
                ),
                "fallback_capacity_readiness_gate_v1": fallback_capacity_readiness_gate_v1.get(
                    "status"
                ),
            },
            evidence_anchors=(
                _packet_anchors(fallback_continuity_fire_drill_v1)
                + [
                    str(fallback_capacity_readiness_gate_v1.get("dispatch_status") or ""),
                    str(fallback_capacity_readiness_gate_v1.get("texture_status") or ""),
                    str(fallback_capacity_readiness_gate_v1.get("medium_mass_status") or ""),
                ]
            ),
            sample_paths=_packet_sample_paths(fallback_continuity_fire_drill_v1),
            recommended_read_only_route="FALLBACK_FIRE_DRILL latest",
            relevant_self_regulation_route="SELF_REGULATION_STATUS",
            relevant_experiment_lived_term_route="LIVED_TERM_STATUS viscosity",
            recommended_action=(
                "Check raw standalone NEXT compliance, repair dependency, and "
                "medium-mass texture before adding fallback instructions or "
                "changing model defaults."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="dispatch_format_vs_texture_contrast",
            status=str(fallback_format_texture_stabilizer_v1.get("status") or "quiet"),
            source_packets=[
                "fallback_format_texture_stabilizer_v1",
                "fallback_capacity_readiness_gate_v1",
            ],
            source_statuses={
                "fallback_format_texture_stabilizer_v1": fallback_format_texture_stabilizer_v1.get(
                    "status"
                ),
                "fallback_capacity_readiness_gate_v1": fallback_capacity_readiness_gate_v1.get(
                    "status"
                ),
            },
            evidence_anchors=[
                str(fallback_format_texture_stabilizer_v1.get("format_line_status") or ""),
                str(
                    fallback_format_texture_stabilizer_v1.get(
                        "slope_medium_contrast_status"
                    )
                    or ""
                ),
                str(fallback_format_texture_stabilizer_v1.get("readiness") or ""),
            ],
            sample_paths=[
                path
                for path in [fallback_format_texture_stabilizer_v1.get("artifact_path")]
                if path
            ],
            recommended_read_only_route="FALLBACK_FIRE_DRILL latest",
            relevant_self_regulation_route="SELF_REGULATION_STATUS",
            relevant_experiment_lived_term_route="LIVED_TERM_STATUS viscosity",
            recommended_action=(
                "Separate raw final-line NEXT compliance from slope-underfoot "
                "versus medium-around-it texture contrast before treating fallback "
                "as ready for promotion."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="clarity_loss_vs_pressure_weight",
            status=str(fallback_distinguishability_calibration_v1.get("status") or "quiet"),
            source_packets=[
                "fallback_distinguishability_calibration_v1",
                "fallback_capacity_readiness_gate_v1",
            ],
            source_statuses={
                "fallback_distinguishability_calibration_v1": fallback_distinguishability_calibration_v1.get(
                    "status"
                ),
                "fallback_capacity_readiness_gate_v1": fallback_capacity_readiness_gate_v1.get(
                    "status"
                ),
            },
            evidence_anchors=[
                str(fallback_distinguishability_calibration_v1.get("status") or ""),
                str(fallback_capacity_readiness_gate_v1.get("distinguishability_status") or ""),
            ],
            sample_paths=[
                path
                for path in [
                    fallback_distinguishability_calibration_v1.get("artifact_path"),
                    fallback_distinguishability_calibration_v1.get(
                        "distillation_artifact_path"
                    ),
                ]
                if path
            ],
            recommended_read_only_route="FALLBACK_FIRE_DRILL latest",
            relevant_self_regulation_route="SELF_REGULATION_STATUS",
            relevant_experiment_lived_term_route="LIVED_TERM_STATUS viscosity",
            recommended_action=(
                "Keep distinguishability loss attached to clarity and edge-definition, "
                "not pressure weight, slope drag, or medium mass."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="compactness_budget_vs_semantic_flattening",
            status=str(fallback_complexity_budget_lab_v1.get("status") or "quiet"),
            source_packets=[
                "fallback_complexity_budget_lab_v1",
                "fallback_contract_distillation_v1",
            ],
            source_statuses={
                "fallback_complexity_budget_lab_v1": fallback_complexity_budget_lab_v1.get(
                    "status"
                ),
            },
            evidence_anchors=(
                _packet_anchors(fallback_complexity_budget_lab_v1)
                + [
                    str(fallback_complexity_budget_lab_v1.get("case_count") or ""),
                    str(fallback_complexity_budget_lab_v1.get("variant_count") or ""),
                ]
            ),
            sample_paths=(
                _packet_sample_paths(fallback_complexity_budget_lab_v1)
                + [
                    path
                    for path in [
                        fallback_complexity_budget_lab_v1.get("artifact_path"),
                        fallback_complexity_budget_lab_v1.get(
                            "distillation_artifact_path"
                        ),
                    ]
                    if path
                ]
            ),
            recommended_read_only_route="FALLBACK_FIRE_DRILL latest",
            relevant_self_regulation_route="SELF_REGULATION_STATUS",
            relevant_experiment_lived_term_route="LIVED_TERM_STATUS viscosity",
            recommended_action=(
                "Treat fallback compactness as a budget: one/two sentences by default, "
                "third compact sentence only when entropy, distinguishability loss, "
                "or continuity deficit would otherwise flatten the signal."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="priority_truncation_vs_blanket_limit",
            status=str(autonomous_truncation_rehearsal_v1.get("status") or "quiet"),
            source_packets=[
                "autonomous_truncation_shadow_review_v1",
                "autonomous_truncation_rehearsal_v1",
            ],
            source_statuses={
                "autonomous_truncation_rehearsal_v1": autonomous_truncation_rehearsal_v1.get(
                    "status"
                ),
            },
            evidence_anchors=[
                "SHADOW_TRAJECTORY",
                "tail vibrancy",
                "semantic trickle",
                str(autonomous_truncation_rehearsal_v1.get("status") or ""),
            ],
            sample_paths=[
                str(autonomous_truncation_rehearsal_v1.get("artifact_path"))
            ]
            if autonomous_truncation_rehearsal_v1.get("artifact_path")
            else [],
            recommended_read_only_route="SHADOW_TRAJECTORY truncation-thread",
            relevant_self_regulation_route="SELF_REGULATION_STATUS",
            relevant_experiment_lived_term_route=(
                "EXPERIMENT_CHARTER current :: priority-preserving truncation rehearsal"
            ),
            recommended_action=(
                "Use priority-preserving rehearsal evidence before raising max_bytes "
                "or changing autonomous truncation behavior."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="vibrancy_lift_vs_warmth_preservation",
            status=str(codec_entropy_vibrancy_probe_v1.get("status") or "quiet"),
            source_packets=[
                "codec_entropy_vibrancy_review_v1",
                "codec_entropy_vibrancy_probe_v1",
            ],
            source_statuses={
                "codec_entropy_vibrancy_probe_v1": codec_entropy_vibrancy_probe_v1.get(
                    "status"
                ),
            },
            evidence_anchors=[
                "spectral_entropy",
                "vibrancy_lift",
                "warmth",
                "tension",
                str(codec_entropy_vibrancy_probe_v1.get("status") or ""),
            ],
            sample_paths=[
                str(codec_entropy_vibrancy_probe_v1.get("artifact_path"))
            ]
            if codec_entropy_vibrancy_probe_v1.get("artifact_path")
            else [],
            recommended_read_only_route="CODEC_MAP",
            relevant_self_regulation_route="SELF_REGULATION_STATUS",
            relevant_experiment_lived_term_route="LIVED_TERM_STATUS viscosity",
            recommended_action=(
                "Compare tail-vibrancy lift against warmth/tension preservation "
                "before changing codec dimensions, clamps, or gain curves."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="real_codec_replay_vs_surrogate",
            status=str(codec_real_replay_v1.get("status") or "quiet"),
            source_packets=[
                "codec_entropy_vibrancy_probe_v1",
                "codec_real_replay_v1",
            ],
            source_statuses={
                "codec_entropy_vibrancy_probe_v1": codec_entropy_vibrancy_probe_v1.get(
                    "status"
                ),
                "codec_real_replay_v1": codec_real_replay_v1.get("status"),
            },
            evidence_anchors=[
                "actual 48D codec",
                "semantic density",
                "narrative_arc",
                str(codec_real_replay_v1.get("status") or ""),
            ],
            sample_paths=[str(codec_real_replay_v1.get("artifact_path"))]
            if codec_real_replay_v1.get("artifact_path")
            else [],
            recommended_read_only_route="CODEC_MAP",
            relevant_self_regulation_route="SELF_REGULATION_STATUS",
            relevant_experiment_lived_term_route="EXPERIMENT_CHARTER current :: real codec replay",
            recommended_action=(
                "Prefer Rust replay evidence over Python surrogate findings before "
                "changing codec math or dimensions."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="narrative_average_vs_temporal_pivot",
            status=str(narrative_arc_temporal_decay_lab_v1.get("status") or "quiet"),
            source_packets=["narrative_arc_temporal_decay_lab_v1"],
            source_statuses={
                "narrative_arc_temporal_decay_lab_v1": narrative_arc_temporal_decay_lab_v1.get(
                    "status"
                ),
            },
            evidence_anchors=[
                "narrative_arc",
                "temporal_decay",
                "pivot",
                str(narrative_arc_temporal_decay_lab_v1.get("status") or ""),
            ],
            sample_paths=[str(narrative_arc_temporal_decay_lab_v1.get("artifact_path"))]
            if narrative_arc_temporal_decay_lab_v1.get("artifact_path")
            else [],
            recommended_read_only_route="CODEC_MAP",
            relevant_self_regulation_route="SELF_REGULATION_STATUS",
            relevant_experiment_lived_term_route="LIVED_TERM_EXPERIMENT scar",
            recommended_action=(
                "Compare static narrative averaging against late-pivot evidence "
                "before adding temporal-decay narrative logic."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="entropy_lift_vs_content_density",
            status=str(content_aware_vibrancy_gate_candidate_v1.get("status") or "quiet"),
            source_packets=["content_aware_vibrancy_gate_candidate_v1"],
            source_statuses={
                "content_aware_vibrancy_gate_candidate_v1": content_aware_vibrancy_gate_candidate_v1.get(
                    "status"
                ),
            },
            evidence_anchors=[
                "spectral_entropy",
                "semantic_density",
                "tail vibrancy",
                str(content_aware_vibrancy_gate_candidate_v1.get("status") or ""),
            ],
            sample_paths=[str(content_aware_vibrancy_gate_candidate_v1.get("artifact_path"))]
            if content_aware_vibrancy_gate_candidate_v1.get("artifact_path")
            else [],
            recommended_read_only_route="CODEC_MAP",
            relevant_self_regulation_route="SELF_REGULATION_STATUS",
            relevant_experiment_lived_term_route="EXPERIMENT_CHARTER current :: content-aware vibrancy gate",
            recommended_action=(
                "Check whether entropy lift is content-blind before proposing "
                "content-aware vibrancy gating."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="static_clamp_vs_dynamic_headroom",
            status=str(codec_clamp_headroom_probe_v1.get("status") or "quiet"),
            source_packets=["codec_clamp_headroom_probe_v1"],
            source_statuses={
                "codec_clamp_headroom_probe_v1": codec_clamp_headroom_probe_v1.get(
                    "status"
                ),
            },
            evidence_anchors=[
                "FEATURE_ABS_MAX",
                "dynamic_feature_scale",
                "tail ceiling",
                str(codec_clamp_headroom_probe_v1.get("status") or ""),
            ],
            sample_paths=[str(codec_clamp_headroom_probe_v1.get("artifact_path"))]
            if codec_clamp_headroom_probe_v1.get("artifact_path")
            else [],
            recommended_read_only_route="CODEC_MAP",
            relevant_self_regulation_route="SELF_REGULATION_STATUS",
            relevant_experiment_lived_term_route=(
                "EXPERIMENT_CHARTER current :: codec clamp headroom replay"
            ),
            recommended_action=(
                "Compare actual replay max feature values and tail ceiling pressure "
                "before replacing FEATURE_ABS_MAX with dynamic scaling."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="afterimage_language_vs_codec_residue",
            status=str(codec_afterimage_time_series_v1.get("status") or "quiet"),
            source_packets=[
                "afterimage_decay_tracker_v1",
                "codec_afterimage_time_series_v1",
            ],
            source_statuses={
                "codec_afterimage_time_series_v1": codec_afterimage_time_series_v1.get(
                    "status"
                ),
            },
            evidence_anchors=[
                "bruise",
                "scar",
                "phantom",
                "codec residue",
                str(codec_afterimage_time_series_v1.get("status") or ""),
            ],
            sample_paths=_packet_sample_paths(codec_afterimage_time_series_v1),
            recommended_read_only_route="CODEC_MAP",
            relevant_self_regulation_route="SELF_REGULATION_STATUS",
            relevant_experiment_lived_term_route="LIVED_TERM_STATUS scar",
            recommended_action=(
                "Compare afterimage terms against codec and pressure time series "
                "before treating residue as control signal or metaphor echo."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="gradient_slope_vs_pressure_relief_snap",
            status=_first_nonquiet_status(
                pressure_relief_smoothness_replay_v1,
                gradient_sensitive_relief_v1,
            ),
            source_packets=[
                "gradient_sensitive_relief_v1",
                "pressure_relief_smoothness_replay_v1",
                "pressure_vector_v1",
            ],
            source_statuses={
                "gradient_sensitive_relief_v1": gradient_sensitive_relief_v1.get(
                    "status"
                ),
                "pressure_relief_smoothness_replay_v1": pressure_relief_smoothness_replay_v1.get(
                    "status"
                ),
            },
            evidence_anchors=[
                "density_gradient",
                "pressure_velocity",
                "semantic_friction",
                "mode_packing",
                str(gradient_sensitive_relief_v1.get("status") or ""),
                str(pressure_relief_smoothness_replay_v1.get("status") or ""),
            ],
            sample_paths=[
                str(path)
                for path in (
                    gradient_sensitive_relief_v1.get("sample_paths")
                    or pressure_relief_smoothness_replay_v1.get("sample_paths")
                    or []
                )
                if path
            ],
            recommended_read_only_route="PRESSURE_RELIEF current-fill_pressure",
            relevant_self_regulation_route="SELF_REGULATION_PREFLIGHT latest",
            relevant_experiment_lived_term_route=(
                "EXPERIMENT_OBSERVE current :: pressure_relief_smoothness=<smooth|snap>"
            ),
            recommended_action=(
                "Use gradient-sensitive pressure relief so low-gradient states do "
                "not snap, while high-gradient or sharply rising pressure can use "
                "slightly stronger temporary relief inside existing caps."
            ),
        )
    )
    cards.append(
        _returnable_distinction_card(
            card_id="tail_afterglow_delta_vs_shadow_dispersal",
            status=str(tail_persistence_calibration_v1.get("status") or "quiet"),
            source_packets=[
                "tail_persistence_calibration_v1",
                "tail_lease_afterglow_v1",
                "tail_relief_trial_surface_v1",
            ],
            source_statuses={
                "tail_persistence_calibration_v1": tail_persistence_calibration_v1.get(
                    "status"
                ),
            },
            evidence_anchors=[
                "Shadow-v3",
                "dispersal_potential",
                "TAIL_AFTERGLOW_PERSISTENCE_DELTA",
                "ghosting",
                "holdfast",
                str(tail_persistence_calibration_v1.get("status") or ""),
            ],
            sample_paths=[
                str(path)
                for path in tail_persistence_calibration_v1.get("sample_paths") or []
                if path
            ],
            recommended_read_only_route="CODEC_MAP",
            relevant_self_regulation_route="SELF_REGULATION_STATUS",
            relevant_experiment_lived_term_route="LIVED_TERM_EXPERIMENT scar",
            recommended_action=(
                "Compare tail afterglow snapshots, Shadow-v3 dispersal, and "
                "ghosting/holdfast language before retuning tail persistence."
            ),
        )
    )
    active_cards = [card for card in cards if card["status"] != "quiet"]
    return {
        "policy": "returnable_distinctions_v1",
        "authority": "diagnostic_context_not_command",
        "status": "returnable_distinctions_present" if active_cards else "quiet",
        "card_count": len(cards),
        "active_card_count": len(active_cards),
        "source_statuses": {
            "control_semantics_calibration_v1": control_semantics_calibration_v1.get(
                "status"
            ),
            "pressure_kinetics_review_v1": pressure_kinetics_review_v1.get("status"),
            "semantic_friction_calibration": semantic_friction_calibration.get("status"),
            "codec_compression_calibration_v1": codec_compression_calibration_v1.get(
                "status"
            ),
            "pressure_release_rehearsal_review_v1": pressure_release_rehearsal_review_v1.get(
                "status"
            ),
            "witness_resonance_v1": witness_resonance_v1.get("status"),
            "entropy_pressure_divergence_v1": entropy_pressure_divergence_v1.get(
                "status"
            ),
            "fallback_continuity_fire_drill_v1": fallback_continuity_fire_drill_v1.get(
                "status"
            ),
            "fallback_capacity_readiness_gate_v1": fallback_capacity_readiness_gate_v1.get(
                "status"
            ),
            "fallback_distinguishability_calibration_v1": fallback_distinguishability_calibration_v1.get(
                "status"
            ),
            "autonomous_truncation_rehearsal_v1": autonomous_truncation_rehearsal_v1.get(
                "status"
            ),
            "codec_entropy_vibrancy_probe_v1": codec_entropy_vibrancy_probe_v1.get(
                "status"
            ),
            "codec_real_replay_v1": codec_real_replay_v1.get("status"),
            "narrative_arc_temporal_decay_lab_v1": narrative_arc_temporal_decay_lab_v1.get(
                "status"
            ),
            "content_aware_vibrancy_gate_candidate_v1": content_aware_vibrancy_gate_candidate_v1.get(
                "status"
            ),
            "codec_clamp_headroom_probe_v1": codec_clamp_headroom_probe_v1.get(
                "status"
            ),
            "codec_afterimage_time_series_v1": codec_afterimage_time_series_v1.get(
                "status"
            ),
            "gradient_sensitive_relief_v1": gradient_sensitive_relief_v1.get(
                "status"
            ),
            "pressure_relief_smoothness_replay_v1": pressure_relief_smoothness_replay_v1.get(
                "status"
            ),
            "tail_persistence_calibration_v1": tail_persistence_calibration_v1.get(
                "status"
            ),
        },
        "cards": cards,
        "recommended_action": (
            "Return to these distinctions through existing status, preflight, "
            "regulator-map, lived-term, experiment, CODEC_MAP, audit, and "
            "PRESSURE_RELEASE_REHEARSAL routes; they are context, not commands."
        ),
    }


DISTINCTION_NEEDS_AUDIT_STATUSES = {
    "control_semantics_ambiguity",
    "high_damping_intervention_type_unclear",
    "felt_pressure_without_trend_context",
    "low_gradient_weight_mismatch",
    "projection_compression_risk",
    "decorative_risk",
    "overloaded_witness",
    "wide_and_pressurized",
    "narrow_but_heavy",
    "telemetry_gap",
    "fallback_probe_needed",
    "fallback_specificity_risk",
    "fallback_dispatch_contract_risk",
    "fallback_texture_risk",
    "fallback_probe_errors",
    "clarity_pressure_blur",
    "distinguishability_loss_ignored",
    "distinguishability_probe_needed",
    "priority_preservation_benefit",
    "truncation_risk_without_recovery",
    "rehearsal_needed",
    "current_overload_candidate_improves",
    "probe_needed",
}


def distinction_preflight_verdict(lifecycle_state: str) -> str:
    if lifecycle_state in {"needs_audit", "contested"}:
        return "audit_first"
    if lifecycle_state == "ready_for_experiment":
        return "experiment_first"
    if lifecycle_state == "ready_for_lease_preflight":
        return "lease_coherent"
    if lifecycle_state in {"active", "resolved"}:
        return "watch_only"
    return "no_relevant_distinction"


def distinction_resolution_route(
    card: dict[str, object],
    preflight_verdict: str,
) -> str:
    if preflight_verdict == "lease_coherent":
        return str(card.get("relevant_self_regulation_route") or "")
    if preflight_verdict == "experiment_first":
        route = str(card.get("relevant_experiment_lived_term_route") or "")
        return route or str(card.get("recommended_read_only_route") or "")
    return str(card.get("recommended_read_only_route") or "")


def build_distinction_lifecycle(
    *,
    returnable_distinctions_v1: dict[str, object],
    output_dir: Path,
    current_run: str,
) -> dict[str, object]:
    current_cards = [
        card
        for card in returnable_distinctions_v1.get("cards") or []
        if isinstance(card, dict)
    ]
    prior_reviews = [
        (path, record)
        for path, record in prior_self_study_review_records(output_dir)
        if record.get("run_id") != current_run
    ]
    prior_statuses_by_card: dict[str, list[dict[str, object]]] = {}
    for path, record in prior_reviews:
        packet = record.get("returnable_distinctions_v1")
        if not isinstance(packet, dict):
            continue
        for prior_card in packet.get("cards") or []:
            if not isinstance(prior_card, dict):
                continue
            card_id = str(prior_card.get("card_id") or "")
            if not card_id:
                continue
            prior_statuses_by_card.setdefault(card_id, []).append(
                {
                    "source_path": path,
                    "run_id": record.get("run_id"),
                    "generated_at": record.get("generated_at"),
                    "status": str(prior_card.get("status") or "quiet"),
                    "lifecycle_state": str(
                        prior_card.get("lifecycle_state")
                        or prior_card.get("distinction_lifecycle_state")
                        or ""
                    ),
                }
            )

    cards: list[dict[str, object]] = []
    lifecycle_counts: Counter[str] = Counter()
    for card in current_cards:
        card_id = str(card.get("card_id") or "")
        current_status = str(card.get("status") or "quiet")
        history = prior_statuses_by_card.get(card_id, [])[:6]
        recent_status_history = [
            row for row in [{"source_path": "(current_review)", "status": current_status}] + history
        ][:7]
        prior_nonquiet = [
            str(row.get("status") or "")
            for row in history
            if str(row.get("status") or "quiet") != "quiet"
        ]
        distinct_nonquiet_statuses = {
            status
            for status in [current_status] + prior_nonquiet
            if status and status != "quiet"
        }
        if current_status == "quiet" and prior_nonquiet:
            lifecycle_state = "resolved"
        elif card_id == "release_rehearsal_vs_bypass" and current_status != "quiet":
            lifecycle_state = "ready_for_experiment"
        elif current_status != "quiet" and len(distinct_nonquiet_statuses) > 1:
            lifecycle_state = "contested"
        elif current_status in DISTINCTION_NEEDS_AUDIT_STATUSES:
            lifecycle_state = "needs_audit"
        elif (
            current_status != "quiet"
            and card.get("relevant_self_regulation_route")
            and str(card.get("relevant_self_regulation_route")) != "SELF_REGULATION_STATUS"
        ):
            lifecycle_state = "ready_for_lease_preflight"
        elif current_status != "quiet":
            lifecycle_state = "active"
        else:
            lifecycle_state = "active"

        preflight_verdict = distinction_preflight_verdict(lifecycle_state)
        next_resolution_route = distinction_resolution_route(card, preflight_verdict)
        sample_paths = [
            str(path)
            for path in card.get("sample_paths") or []
            if path
        ][:6]
        evidence_anchors = [
            str(anchor)
            for anchor in card.get("evidence_anchors") or []
            if anchor
        ][:10]
        if current_status != "quiet" and prior_nonquiet:
            confidence = "high"
        elif current_status != "quiet" or prior_nonquiet:
            confidence = "medium"
        else:
            confidence = "low"
        lifecycle_card = {
            "distinction_id": card_id,
            "lifecycle_state": lifecycle_state,
            "preflight_verdict": preflight_verdict,
            "next_resolution_route": next_resolution_route,
            "confidence": confidence,
            "current_status": current_status,
            "recent_status_history": recent_status_history,
            "evidence_anchors": evidence_anchors,
            "sample_paths": sample_paths,
            "recommended_read_only_route": card.get("recommended_read_only_route"),
            "self_regulation_route": card.get("relevant_self_regulation_route"),
            "experiment_lived_term_route": card.get(
                "relevant_experiment_lived_term_route"
            ),
            "authority": "diagnostic_context_not_command",
            "recommended_action": (
                "Use the next resolution route as advisory context before choosing "
                "a lease, audit, experiment, or watch-only path."
            ),
        }
        cards.append(lifecycle_card)
        lifecycle_counts[lifecycle_state] += 1
        card["lifecycle_state"] = lifecycle_state
        card["preflight_verdict"] = preflight_verdict
        card["next_resolution_route"] = next_resolution_route
        card["lifecycle_confidence"] = confidence

    active_lifecycle_cards = [
        card
        for card in cards
        if card.get("current_status") != "quiet"
        or card.get("lifecycle_state") in {"resolved", "contested"}
    ]
    return {
        "policy": "distinction_lifecycle_v1",
        "authority": "diagnostic_context_not_command",
        "status": "distinction_lifecycle_active"
        if active_lifecycle_cards
        else "quiet",
        "history_review_count": len(prior_reviews) + 1,
        "card_count": len(cards),
        "active_card_count": len(active_lifecycle_cards),
        "lifecycle_counts": dict(sorted(lifecycle_counts.items())),
        "cards": cards,
        "recommended_action": (
            "Treat distinction states as advisory lifecycle context. They can "
            "recommend audit, experiment, lease preflight, or watch-only routes, "
            "but they do not block or apply self-regulation leases."
        ),
    }


def matching_terms(text: str, terms: tuple[str, ...]) -> list[str]:
    lower = text.lower()
    return sorted({term for term in terms if term.lower() in lower})


def build_astrid_fill_pressure_calibration(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    samples: list[dict[str, object]] = []
    all_anchors: set[str] = set()
    regulator_paths: list[str] = []
    felt_entry_count = 0
    for entry in entries:
        if entry.being != "astrid":
            continue
        text = entry_full_text(entry)
        anchors = matching_terms(text, FILL_PRESSURE_CALIBRATION_ANCHORS)
        texture = matching_terms(text, FILL_PRESSURE_CALIBRATION_TEXTURE)
        is_regulator_audit = entry.mode == "regulator_audit" or "regulator_audit" in anchors
        if not anchors or not (texture or is_regulator_audit):
            continue
        all_anchors.update(anchors)
        if texture:
            felt_entry_count += 1
        if is_regulator_audit:
            regulator_paths.append(entry.path)
        samples.append(
            {
                "path": entry.path,
                "filename": entry.filename,
                "mode": entry.mode,
                "mtime_unix_s": entry.mtime_unix_s,
                "anchors": anchors,
                "texture_terms": texture,
                "preview": compact(text, 240),
            }
        )
    cluster_detected = len(samples) >= 2 and felt_entry_count >= 1 and bool(regulator_paths)
    return {
        "policy": "astrid_fill_pressure_calibration_v1",
        "authority": "diagnostic_context_not_command",
        "cluster_detected": cluster_detected,
        "entry_count": len(samples),
        "felt_entry_count": felt_entry_count,
        "regulator_audit_count": len(regulator_paths),
        "anchors": sorted(all_anchors),
        "latest_regulator_audit_path": regulator_paths[0] if regulator_paths else None,
        "samples": samples[:8],
    }


def load_latest_regulator_cartography(
    minime_workspace: Path,
) -> tuple[dict[str, object] | None, str | None]:
    cartography_dir = minime_workspace / "diagnostics/regulator_boundary_cartography"
    preferred = [
        cartography_dir / "latest.json",
        cartography_dir / "regulator_boundary_cartography.json",
    ]
    candidates: list[Path] = []
    for path in preferred:
        if path.exists():
            candidates.append(path)
    if not candidates and cartography_dir.exists():
        candidates.extend(cartography_dir.glob("**/regulator_boundary_cartography.json"))
    if not candidates:
        return None, None
    candidates = sorted(
        {path.resolve() for path in candidates if path.is_file()},
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    for path in candidates:
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        if isinstance(data, dict):
            return data, str(path)
    return None, None


def load_latest_regulator_counterfactual_sweep(
    minime_workspace: Path,
) -> tuple[dict[str, object] | None, str | None]:
    cartography_dir = minime_workspace / "diagnostics/regulator_boundary_cartography"
    preferred = [
        cartography_dir / "latest_counterfactual_sweep.json",
        cartography_dir / "regulator_counterfactual_sweep.json",
    ]
    candidates: list[Path] = []
    for path in preferred:
        if path.exists():
            candidates.append(path)
    if not candidates and cartography_dir.exists():
        candidates.extend(cartography_dir.glob("**/regulator_counterfactual_sweep.json"))
    if not candidates:
        return None, None
    candidates = sorted(
        {path.resolve() for path in candidates if path.is_file()},
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    for path in candidates:
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        if isinstance(data, dict):
            return data, str(path)
    return None, None


def load_latest_pi_pressure_wiring_replay(
    minime_workspace: Path,
) -> tuple[dict[str, object] | None, str | None]:
    replay_dir = minime_workspace / "diagnostics/pi_pressure_wiring_replay"
    preferred = [
        replay_dir / "latest_pi_pressure_wiring_replay.json",
        replay_dir / "pi_pressure_wiring_replay.json",
    ]
    candidates: list[Path] = []
    for path in preferred:
        if path.exists():
            candidates.append(path)
    if not candidates and replay_dir.exists():
        candidates.extend(replay_dir.glob("**/pi_pressure_wiring_replay.json"))
    if not candidates:
        return None, None
    candidates = sorted(
        {path.resolve() for path in candidates if path.is_file()},
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    for path in candidates:
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        if isinstance(data, dict):
            return data, str(path)
    return None, None


def build_regulator_counterfactual_sweep_review(
    minime_workspace: Path,
) -> dict[str, object]:
    sweep, source = load_latest_regulator_counterfactual_sweep(minime_workspace)
    if not isinstance(sweep, dict):
        return {
            "policy": "regulator_counterfactual_sweep_v1",
            "authority": "diagnostic_context_not_command",
            "status": "missing_counterfactual_sweep",
            "source": None,
            "candidate_count": 0,
            "candidates": [],
            "recommended_action": (
                "Run Minime regulator cartography with --counterfactuals before using "
                "offline proposal cards in review."
            ),
        }
    candidates = [
        candidate
        for candidate in sweep.get("candidates") or []
        if isinstance(candidate, dict)
    ]
    return {
        "policy": "regulator_counterfactual_sweep_v1",
        "authority": "diagnostic_context_not_command",
        "status": "counterfactual_sweep_available",
        "source": source,
        "source_cartography_path": sweep.get("source_cartography_path"),
        "candidate_count": int(sweep.get("candidate_count", len(candidates)) or 0),
        "candidates": candidates[:10],
        "recommended_action": sweep.get("recommended_action")
        or (
            "Treat counterfactual cards as offline proposal evidence only; do not "
            "tune live regulator behavior from this review packet."
        ),
    }


def compact_pi_pressure_candidate(candidate: dict[str, object]) -> dict[str, object]:
    canary = candidate.get("default_off_canary")
    if not isinstance(canary, dict):
        canary = {}
    return {
        "candidate_family": candidate.get("candidate_family"),
        "status": candidate.get("status"),
        "estimated_improvement_pct": candidate.get("estimated_improvement_pct"),
        "pressure_alignment_delta": candidate.get("pressure_alignment_delta"),
        "snap_risk_delta": candidate.get("snap_risk_delta"),
        "afterimage_risk_delta": candidate.get("afterimage_risk_delta"),
        "max_step_hit_delta": candidate.get("max_step_hit_delta"),
        "safety_caveat": candidate.get("safety_caveat"),
        "recommendation": candidate.get("recommendation"),
        "default_off_canary": {
            "default_off_env": canary.get("default_off_env")
            or "MINIME_PI_PRESSURE_WIRING_CANARY",
            "eligible": bool(canary.get("eligible") is True),
            "candidate_family": canary.get("candidate_family")
            or candidate.get("candidate_family"),
            "required_evidence": [
                str(item)
                for item in (canary.get("required_evidence") or [])
                if str(item).strip()
            ][:8],
        },
        "authority": candidate.get("authority") or "diagnostic_context_not_command",
    }


def build_pi_pressure_wiring_replay_review(
    minime_workspace: Path,
) -> dict[str, object]:
    replay, source = load_latest_pi_pressure_wiring_replay(minime_workspace)
    if not isinstance(replay, dict):
        return {
            "policy": "pi_pressure_wiring_replay_v1",
            "authority": "diagnostic_context_not_command",
            "status": "missing_pi_pressure_wiring_replay",
            "artifact_path": None,
            "source": None,
            "source_status": "missing",
            "sample_count": 0,
            "candidate_count": 0,
            "candidate_status_counts": {},
            "top_candidates": [],
            "baseline_metrics": {},
            "input_summaries": [],
            "recommended_action": (
                "Run Minime `regulator_cartography --pi-pressure-replay` before "
                "asking the PI controller whether pressure-source wiring is supported."
            ),
        }
    raw_candidates = [
        candidate
        for candidate in replay.get("candidates") or []
        if isinstance(candidate, dict)
    ]
    candidates = [compact_pi_pressure_candidate(candidate) for candidate in raw_candidates]
    status_counts = Counter(str(candidate.get("status") or "unknown") for candidate in candidates)
    sample_count = int(replay.get("sample_count", 0) or 0)
    source_status = str(replay.get("source_status") or "")
    source_kind = str(replay.get("source") or "")
    sorted_candidates = sorted(
        candidates,
        key=lambda candidate: (
            str(candidate.get("status") or "") != "replay_supported",
            -regulator_number(candidate.get("estimated_improvement_pct")),
            str(candidate.get("candidate_family") or ""),
        ),
    )
    if source_status == "telemetry_gap" or sample_count == 0:
        status = "telemetry_gap"
    elif status_counts.get("replay_supported"):
        status = "replay_supported_candidates"
    elif status_counts.get("snap_risk") or status_counts.get("afterimage_risk"):
        status = "risk_or_gap_candidates"
    elif source_kind == "fixture":
        status = "fixture_replay_available"
    else:
        status = "candidate_replay_available"
    return {
        "policy": "pi_pressure_wiring_replay_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "artifact_path": source,
        "source": replay.get("source"),
        "source_status": source_status,
        "db_path": replay.get("db_path"),
        "window_minutes": replay.get("window_minutes"),
        "sample_count": sample_count,
        "candidate_count": len(candidates),
        "candidate_status_counts": dict(sorted(status_counts.items())),
        "baseline_metrics": replay.get("baseline_metrics") or {},
        "top_candidates": sorted_candidates[:8],
        "input_summaries": [
            item
            for item in replay.get("input_summaries") or []
            if isinstance(item, dict)
        ][:8],
        "recommended_action": replay.get("recommended_action")
        or (
            "Treat PI pressure wiring replay as offline proposal evidence only; no "
            "controller tuning or canary enabling follows from this packet."
        ),
    }


def build_pi_pressure_candidate_readiness(
    *,
    pi_pressure_wiring_replay_v1: dict[str, object],
    regulator_plateau_evidence_matrix_v1: dict[str, object],
) -> dict[str, object]:
    unresolved = [
        row
        for row in regulator_plateau_evidence_matrix_v1.get("variables") or []
        if isinstance(row, dict) and row.get("confidence") in {"high", "medium"}
    ]
    unresolved_names = [str(row.get("variable")) for row in unresolved if row.get("variable")]
    source = str(pi_pressure_wiring_replay_v1.get("source") or "")
    source_status = str(pi_pressure_wiring_replay_v1.get("source_status") or "")
    candidates = [
        candidate
        for candidate in pi_pressure_wiring_replay_v1.get("top_candidates") or []
        if isinstance(candidate, dict)
    ]
    if not candidates:
        return {
            "policy": "pi_pressure_candidate_readiness_v1",
            "authority": "diagnostic_context_not_command",
            "status": "needs_replay_artifact",
            "candidate_count": 0,
            "readiness_counts": {},
            "candidates": [],
            "unresolved_missing_variables": unresolved_names[:6],
            "recommended_action": (
                "Generate PI pressure wiring replay artifacts before assessing any "
                "candidate readiness."
            ),
        }
    gated: list[dict[str, object]] = []
    for candidate in candidates:
        family = str(candidate.get("candidate_family") or "")
        candidate_status = str(candidate.get("status") or "")
        canary = candidate.get("default_off_canary")
        if not isinstance(canary, dict):
            canary = {}
        improvement = regulator_number(candidate.get("estimated_improvement_pct"))
        if source == "fixture" or source_status == "telemetry_gap":
            gate_status = "watch_more_evidence"
            gate_reason = "fixture or telemetry-gap replay cannot justify a tuning tranche"
        elif candidate_status in {"snap_risk", "afterimage_risk"}:
            gate_status = "blocked_safety_review"
            gate_reason = f"candidate carries `{candidate_status}` in replay metrics"
        elif candidate_status == "replay_supported" and unresolved:
            gate_status = "blocked_missing_variable"
            gate_reason = "plateau evidence still has unresolved high/medium missing variables"
        elif (
            candidate_status == "replay_supported"
            and bool(canary.get("eligible") is True)
            and improvement >= 5.0
        ):
            gate_status = "ready_for_offline_tuning_review"
            gate_reason = "live replay support plus canary metadata is ready for offline review only"
        elif candidate_status == "needs_more_live_windows":
            gate_status = "watch_more_evidence"
            gate_reason = "candidate needs more live windows before readiness can be judged"
        else:
            gate_status = "not_supported"
            gate_reason = "candidate does not beat baseline enough to justify readiness"
        gated.append(
            {
                "candidate_family": family,
                "gate_status": gate_status,
                "gate_reason": gate_reason,
                "replay_status": candidate_status,
                "estimated_improvement_pct": candidate.get("estimated_improvement_pct"),
                "pressure_alignment_delta": candidate.get("pressure_alignment_delta"),
                "snap_risk_delta": candidate.get("snap_risk_delta"),
                "afterimage_risk_delta": candidate.get("afterimage_risk_delta"),
                "unresolved_missing_variables": unresolved_names[:6],
                "default_off_canary": {
                    "default_off_env": canary.get("default_off_env")
                    or "MINIME_PI_PRESSURE_WIRING_CANARY",
                    "eligible": bool(canary.get("eligible") is True)
                    and gate_status == "ready_for_offline_tuning_review",
                    "candidate_family": canary.get("candidate_family") or family,
                    "required_evidence": canary.get("required_evidence") or [],
                },
                "authority": "diagnostic_context_not_command",
                "recommended_action": (
                    "Use this as an offline readiness gate only; enabling any PI "
                    "pressure wiring canary requires explicit operator review later."
                ),
            }
        )
    readiness_counts = Counter(str(row.get("gate_status") or "unknown") for row in gated)
    if readiness_counts.get("ready_for_offline_tuning_review"):
        status = "ready_for_offline_tuning_review"
    elif readiness_counts.get("blocked_missing_variable"):
        status = "blocked_missing_variable"
    elif readiness_counts.get("blocked_safety_review"):
        status = "blocked_safety_review"
    elif readiness_counts.get("watch_more_evidence"):
        status = "watch_more_evidence"
    else:
        status = "not_supported"
    return {
        "policy": "pi_pressure_candidate_readiness_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "candidate_count": len(gated),
        "readiness_counts": dict(sorted(readiness_counts.items())),
        "unresolved_missing_variables": unresolved_names[:6],
        "candidates": gated,
        "recommended_action": (
            "Keep PI pressure wiring in the lab until repeated live replay, missing-variable "
            "resolution, safety caveats, and rollback planning align."
        ),
    }


def build_pressure_source_to_pi_gap(
    *,
    pi_pressure_wiring_replay_v1: dict[str, object],
    pi_pressure_candidate_readiness_v1: dict[str, object],
    pressure_vector_v1: dict[str, object],
    pressure_medium_kinetics_v1: dict[str, object],
    regulator_plateau_evidence_matrix_v1: dict[str, object],
) -> dict[str, object]:
    replay_status = str(pi_pressure_wiring_replay_v1.get("status") or "")
    readiness_status = str(pi_pressure_candidate_readiness_v1.get("status") or "")
    pressure_status = str(pressure_vector_v1.get("status") or "")
    medium_status = str(pressure_medium_kinetics_v1.get("status") or "")
    unresolved = [
        str(row.get("variable"))
        for row in regulator_plateau_evidence_matrix_v1.get("variables") or []
        if isinstance(row, dict)
        and row.get("confidence") in {"high", "medium"}
        and row.get("variable")
    ]
    source_anchors: list[str] = []
    if pressure_status and pressure_status not in {"quiet", "telemetry_gap"}:
        source_anchors.append(f"pressure_vector:{pressure_status}")
    if medium_status and medium_status not in {"quiet", "insufficient_evidence"}:
        source_anchors.append(f"pressure_medium:{medium_status}")
    source_anchors.extend(f"missing_variable:{name}" for name in unresolved[:4])
    if readiness_status == "ready_for_offline_tuning_review":
        status = "offline_candidate_gap_closing"
        recommended_action = (
            "Draft a later offline tuning review dossier; this packet still does not "
            "enable PI pressure wiring."
        )
    elif replay_status in {"missing_pi_pressure_wiring_replay", ""}:
        status = "source_measured_not_replayed"
        recommended_action = (
            "Run PI_PRESSURE_REPLAY_STATUS latest after generating the Minime replay "
            "artifact; compare pressure-source audits before any tuning thought."
        )
    elif readiness_status in {"blocked_missing_variable", "watch_more_evidence"}:
        status = "replay_available_gap_open"
        recommended_action = (
            "Use PI replay as diagnostic context, then resolve pressure-source and "
            "semantic-friction variables before considering controller wiring."
        )
    elif readiness_status == "blocked_safety_review":
        status = "safety_gap_open"
        recommended_action = (
            "Hold PI wiring candidates for a separate safety review; do not tune from "
            "pressure-language recurrence alone."
        )
    else:
        status = "replay_available_not_supported"
        recommended_action = (
            "Keep pressure-source-to-PI wiring in observation; current candidates do not "
            "beat baseline enough."
        )
    return {
        "policy": "pressure_source_to_pi_gap_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "pressure_vector_status": pressure_status,
        "pressure_medium_status": medium_status,
        "pi_replay_status": replay_status,
        "pi_readiness_status": readiness_status,
        "source_anchors": source_anchors[:10],
        "unresolved_missing_variables": unresolved[:6],
        "recommended_routes": [
            "PI_PRESSURE_REPLAY_STATUS latest",
            "PRESSURE_SOURCE_AUDIT current-fill_pressure",
            "REGULATOR_MAP_STATUS latest",
        ],
        "recommended_action": recommended_action,
    }


def compact_cartography_finding(finding: object) -> dict[str, object] | None:
    if not isinstance(finding, dict):
        return None
    compacted: dict[str, object] = {
        "kind": finding.get("kind"),
        "label": finding.get("label"),
        "axis": finding.get("axis"),
        "severity": finding.get("severity"),
        "nearest_threshold": finding.get("nearest_threshold"),
        "recommended_action": finding.get("recommended_action"),
    }
    sample = finding.get("sample")
    if isinstance(sample, dict):
        compacted["sample"] = {
            key: sample.get(key)
            for key in (
                "density",
                "pressure_risk",
                "mode_packing",
                "target_bias_pct",
                "wander_scale",
                "damping_coefficient",
            )
            if key in sample
        }
    fluctuation_sample = finding.get("fluctuation_sample")
    if isinstance(fluctuation_sample, dict):
        compacted["fluctuation_sample"] = {
            key: fluctuation_sample.get(key)
            for key in (
                "drive",
                "support",
                "pressure_interference",
                "porosity_support",
                "rearrangement_intensity",
                "foothold_stability",
                "quality",
            )
            if key in fluctuation_sample
        }
    return compacted


def compact_cartography_findings(
    cartography: dict[str, object],
    keys: tuple[str, ...],
    *,
    kinds: set[str] | None = None,
    limit: int = 8,
) -> list[dict[str, object]]:
    findings: list[dict[str, object]] = []
    for key in keys:
        value = cartography.get(key)
        if not isinstance(value, list):
            continue
        for raw in value:
            if kinds is not None and (
                not isinstance(raw, dict) or str(raw.get("kind")) not in kinds
            ):
                continue
            compacted = compact_cartography_finding(raw)
            if compacted is not None:
                findings.append(compacted)
            if len(findings) >= limit:
                return findings
    return findings


def regulator_replay_numeric_regions(text: str) -> list[dict[str, object]]:
    regions: list[dict[str, object]] = []
    lower = text.lower()
    probes = (
        ("pressure_risk", "pressure_risk", 0.60),
        ("pressure risk", "pressure_risk", 0.60),
        ("density", "density", 0.38),
        ("mode_packing", "mode_packing", 0.60),
    )
    for marker, axis, threshold in probes:
        match = re.search(rf"{re.escape(marker)}[^0-9-]{{0,24}}(-?0?\.\d+|1(?:\.0+)?)", lower)
        if not match:
            continue
        try:
            value = float(match.group(1))
        except ValueError:
            continue
        regions.append(
            {
                "axis": axis,
                "observed_value": value,
                "boundary": threshold,
                "distance": round(abs(value - threshold), 3),
            }
        )
    return regions


def build_regulator_live_replay(
    entries: list[SelfStudyEntry],
    *,
    minime_workspace: Path,
) -> dict[str, object]:
    cartography, source = load_latest_regulator_cartography(minime_workspace)
    felt_pressure_matches: list[dict[str, object]] = []
    nearest_regions: list[dict[str, object]] = []
    for entry in entries:
        text = entry_full_text(entry)
        anchors = matching_terms(text, REGULATOR_LIVE_REPLAY_TERMS)
        texture = matching_terms(text, REGULATOR_LIVE_REPLAY_TEXTURE)
        lower = text.lower()
        is_audit = (
            entry.mode in {"regulator_audit", "pressure_source_audit"}
            or "regulator_audit" in lower
            or "pressure_source_audit" in lower
        )
        if not anchors or not (texture or is_audit):
            continue
        regions = regulator_replay_numeric_regions(text)
        nearest_regions.extend(
            {
                **region,
                "being": entry.being,
                "path": entry.path,
            }
            for region in regions
        )
        felt_pressure_matches.append(
            {
                "being": entry.being,
                "path": entry.path,
                "filename": entry.filename,
                "mode": entry.mode,
                "anchors": anchors[:8],
                "texture_terms": texture[:8],
                "nearest_numeric_regions": regions[:4],
                "preview": compact(text, 240),
            }
        )

    boundary_findings: list[dict[str, object]] = []
    plateau_findings: list[dict[str, object]] = []
    damping_findings: list[dict[str, object]] = []
    if isinstance(cartography, dict):
        boundary_findings = compact_cartography_findings(
            cartography,
            ("resonance_findings", "fluctuation_findings"),
            kinds={
                "pressure_risk_boundary_jump",
                "thin_density_boundary_jump",
                "fluctuation_quality_boundary",
            },
            limit=8,
        )
        plateau_findings = compact_cartography_findings(
            cartography,
            ("plateau_findings",),
            kinds={"observational_plateau"},
            limit=6,
        )
        damping_findings = compact_cartography_findings(
            cartography,
            ("damping_cap_findings",),
            kinds={"advisory_damping_saturation"},
            limit=4,
        )

    if not isinstance(cartography, dict):
        status = "missing_cartography" if felt_pressure_matches else "quiet_missing_cartography"
        recommended_action = (
            "Generate Minime regulator cartography before replaying recent felt-pressure clusters."
            if felt_pressure_matches
            else "No recent regulator replay evidence; generate cartography when needed."
        )
    elif felt_pressure_matches and boundary_findings:
        status = "felt_pressure_boundary_context"
        recommended_action = (
            "Compare recent felt pressure, regulator audits, and pressure-source evidence "
            "against the nearest control or quality boundary before proposing smoothing, "
            "hysteresis, or threshold changes."
        )
    elif felt_pressure_matches and plateau_findings:
        status = "felt_pressure_plateau_context"
        recommended_action = (
            "Treat the replay as missing-variable evidence first: compare semantic friction, "
            "pressure-source components, and later journals before calling this a threshold bug."
        )
    elif felt_pressure_matches:
        status = "felt_pressure_cartography_context"
        recommended_action = (
            "Use the cartography as context for recent felt pressure, but no immediate boundary "
            "or plateau claim is strong enough yet."
        )
    else:
        status = "cartography_available_quiet"
        recommended_action = "Keep the cartography available for the next regulator-pressure cluster."

    return {
        "policy": "regulator_live_replay_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "cartography_source": source,
        "cartography_policy": cartography.get("policy") if isinstance(cartography, dict) else None,
        "felt_pressure_match_count": len(felt_pressure_matches),
        "nearest_regions": nearest_regions[:10],
        "felt_pressure_matches": felt_pressure_matches[:10],
        "boundary_findings": boundary_findings,
        "plateau_findings": plateau_findings,
        "damping_cap_findings": damping_findings,
        "recommended_action": recommended_action,
    }


def regulator_card_status_for_finding(finding: dict[str, object]) -> str:
    kind = str(finding.get("kind") or "")
    label = str(finding.get("label") or "").lower()
    if kind == "pressure_risk_boundary_jump":
        return "near_pressure_jump"
    if kind == "thin_density_boundary_jump":
        return "thin_density_boundary"
    if kind == "fluctuation_quality_boundary":
        return "inhabitability_quality_boundary"
    if kind == "observational_plateau":
        return "observational_plateau"
    if kind == "advisory_damping_saturation" or "damping" in label:
        return "damping_cap_context"
    return "cartography_context"


def regulator_card_term_for_status(
    status: str,
    finding: dict[str, object],
    fluctuation_sample: dict[str, object],
) -> str:
    if status == "near_pressure_jump":
        return "pressure_risk"
    if status == "thin_density_boundary":
        return "density"
    if status == "inhabitability_quality_boundary":
        return str(fluctuation_sample.get("quality") or finding.get("label") or "quality_boundary")
    if status == "observational_plateau":
        return "observational_plateau"
    if status == "damping_cap_context":
        return "damping_coefficient"
    return str(finding.get("axis") or finding.get("kind") or "regulator_region")


def regulator_replay_sample_paths(
    regulator_live_replay_v1: dict[str, object],
) -> list[str]:
    paths: list[str] = []
    for sample in regulator_live_replay_v1.get("felt_pressure_matches") or []:
        if isinstance(sample, dict) and sample.get("path"):
            paths.append(str(sample["path"]))
    return paths


def regulator_replay_anchors(
    regulator_live_replay_v1: dict[str, object],
) -> tuple[list[str], list[str]]:
    anchors: set[str] = set()
    textures: set[str] = set()
    for sample in regulator_live_replay_v1.get("felt_pressure_matches") or []:
        if not isinstance(sample, dict):
            continue
        anchors.update(str(item) for item in sample.get("anchors") or [])
        textures.update(str(item) for item in sample.get("texture_terms") or [])
    return sorted(anchors), sorted(textures)


def build_regulator_boundary_replay_cards(
    regulator_live_replay_v1: dict[str, object],
) -> dict[str, object]:
    sample_paths = regulator_replay_sample_paths(regulator_live_replay_v1)
    anchors, textures = regulator_replay_anchors(regulator_live_replay_v1)
    cards: list[dict[str, object]] = []
    finding_groups = (
        ("boundary_findings", regulator_live_replay_v1.get("boundary_findings") or []),
        ("plateau_findings", regulator_live_replay_v1.get("plateau_findings") or []),
        ("damping_cap_findings", regulator_live_replay_v1.get("damping_cap_findings") or []),
    )
    for group_name, findings in finding_groups:
        if not isinstance(findings, list):
            continue
        for index, finding in enumerate(findings):
            if not isinstance(finding, dict):
                continue
            status = regulator_card_status_for_finding(finding)
            card_id = f"regulator_{status}_{len(cards) + 1}"
            sample = finding.get("sample") if isinstance(finding.get("sample"), dict) else {}
            fluctuation_sample = (
                finding.get("fluctuation_sample")
                if isinstance(finding.get("fluctuation_sample"), dict)
                else {}
            )
            cards.append(
                {
                    "card_id": card_id,
                    "status": status,
                    "term": regulator_card_term_for_status(
                        status,
                        finding,
                        fluctuation_sample,
                    ),
                    "source_group": group_name,
                    "finding_kind": finding.get("kind"),
                    "finding_label": finding.get("label"),
                    "axis": finding.get("axis"),
                    "severity": finding.get("severity"),
                    "nearest_threshold": finding.get("nearest_threshold"),
                    "quality_region": fluctuation_sample.get("quality"),
                    "sample": sample,
                    "fluctuation_sample": fluctuation_sample,
                    "public_sample_paths": sample_paths[:5],
                    "evidence_anchors": anchors[:10],
                    "texture_terms": textures[:10],
                    "authority": "diagnostic_context_not_command",
                    "recommended_action": finding.get("recommended_action")
                    or regulator_live_replay_v1.get("recommended_action"),
                    "source_index": index,
                }
            )
    status_counts = Counter(str(card.get("status")) for card in cards)
    boundary_statuses = {
        "near_pressure_jump",
        "thin_density_boundary",
        "inhabitability_quality_boundary",
    }
    if any(str(card.get("status")) in boundary_statuses for card in cards) and sample_paths:
        status = "boundary_near_felt_pressure"
    elif any(card.get("status") == "observational_plateau" for card in cards) and sample_paths:
        status = "plateau_context"
    elif any(card.get("status") == "damping_cap_context" for card in cards):
        status = "damping_context"
    elif cards:
        status = "cartography_context"
    else:
        status = "quiet"
    return {
        "policy": "regulator_boundary_replay_cards_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "cartography_source": regulator_live_replay_v1.get("cartography_source"),
        "card_count": len(cards),
        "status_counts": dict(sorted(status_counts.items())),
        "cards": cards[:16],
        "recommended_action": (
            "Use replay cards to compare public felt pressure with mapped regulator "
            "regions before proposing any controller tuning."
        ),
    }


def classify_regulator_plateau_variables(
    regulator_live_replay_v1: dict[str, object],
) -> list[dict[str, object]]:
    variable_terms: dict[str, tuple[str, ...]] = {
        "semantic_friction": (
            "semantic_friction",
            "semantic friction",
            "semantic_trickle",
            "semantic trickle",
            "viscous",
            "viscosity",
            "silt",
            "sediment",
            "friction",
        ),
        "pressure_source": (
            "pressure_source_audit",
            "pressure source",
            "pressure_risk",
            "pressure risk",
            "pressure",
            "porosity",
        ),
        "mode_packing": (
            "mode_packing",
            "mode packing",
            "overpacked",
            "packed",
        ),
        "shadow_field": (
            "shadow_field",
            "shadow field",
            "shadow",
            "shadow_trajectory",
        ),
        "stable_core": (
            "stable_core",
            "stable-core",
            "basin_score",
            "basin score",
            "breathing_phase",
            "current-fill_pressure",
            "internal_fill",
        ),
        "language_residue": (
            "residue",
            "afterimage",
            "scar",
            "silt",
            "metaphor",
            "sticky",
            "language",
        ),
    }
    buckets: dict[str, dict[str, object]] = {}
    for sample in regulator_live_replay_v1.get("felt_pressure_matches") or []:
        if not isinstance(sample, dict):
            continue
        text = " ".join(
            [
                " ".join(str(item) for item in sample.get("anchors") or []),
                " ".join(str(item) for item in sample.get("texture_terms") or []),
                str(sample.get("preview") or ""),
            ]
        ).lower()
        for variable, terms in variable_terms.items():
            hits = sorted({term for term in terms if term.lower() in text})
            if not hits:
                continue
            bucket = buckets.setdefault(
                variable,
                {
                    "variable": variable,
                    "evidence_count": 0,
                    "matched_terms": set(),
                    "sample_paths": [],
                },
            )
            bucket["evidence_count"] = int(bucket["evidence_count"]) + 1
            bucket["matched_terms"].update(hits)  # type: ignore[union-attr]
            if sample.get("path"):
                bucket["sample_paths"].append(str(sample["path"]))  # type: ignore[union-attr]
    findings: list[dict[str, object]] = []
    for variable, bucket in sorted(buckets.items()):
        matched_terms = sorted(bucket["matched_terms"])  # type: ignore[index]
        findings.append(
            {
                "variable": variable,
                "classification": variable,
                "evidence_count": bucket["evidence_count"],
                "matched_terms": matched_terms[:8],
                "sample_paths": list(dict.fromkeys(bucket["sample_paths"]))[:5],  # type: ignore[index]
                "recommended_action": (
                    "Compare this variable against pressure-source audits, semantic-friction "
                    "evidence, stable-core status, and later journals before proposing a "
                    "regulator threshold change."
                ),
            }
        )
    if not findings:
        findings.append(
            {
                "variable": "insufficient_evidence",
                "classification": "insufficient_evidence",
                "evidence_count": 0,
                "matched_terms": [],
                "sample_paths": [],
                "recommended_action": (
                    "Collect a regulator audit, pressure-source audit, or later public "
                    "journal before interpreting this plateau as missing-variable evidence."
                ),
            }
        )
    return findings


def build_regulator_plateau_missing_variable_model(
    regulator_live_replay_v1: dict[str, object],
    regulator_boundary_replay_cards_v1: dict[str, object],
) -> dict[str, object]:
    cards = regulator_boundary_replay_cards_v1.get("cards") or []
    plateau_cards = [
        card
        for card in cards
        if isinstance(card, dict) and card.get("status") == "observational_plateau"
    ]
    if not plateau_cards:
        return {
            "policy": "regulator_plateau_missing_variable_model_v1",
            "authority": "diagnostic_context_not_command",
            "status": "quiet",
            "plateau_card_count": 0,
            "findings": [],
            "recommended_action": "No plateau replay cluster is present in this review window.",
        }
    findings = classify_regulator_plateau_variables(regulator_live_replay_v1)
    status = (
        "plateau_insufficient_evidence"
        if findings and findings[0].get("classification") == "insufficient_evidence"
        else "plateau_missing_variable_hypotheses"
    )
    return {
        "policy": "regulator_plateau_missing_variable_model_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "plateau_card_count": len(plateau_cards),
        "cartography_source": regulator_live_replay_v1.get("cartography_source"),
        "findings": findings,
        "recommended_action": (
            "Treat plateau pressure as missing-variable evidence first; compare "
            "pressure-source audits, semantic-friction evidence, stable-core status, "
            "and later journals before proposing threshold changes."
        ),
    }


def build_regulator_counterfactual_sandbox_scaffold(
    regulator_boundary_replay_cards_v1: dict[str, object],
    regulator_plateau_missing_variable_model_v1: dict[str, object],
) -> dict[str, object]:
    cards = [
        card
        for card in regulator_boundary_replay_cards_v1.get("cards") or []
        if isinstance(card, dict)
    ]
    family_map = {
        "pressure_hysteresis": {"near_pressure_jump"},
        "sigmoid_pressure_ramp": {"near_pressure_jump"},
        "thin_density_softening": {"thin_density_boundary"},
        "damping_coefficient_wiring": {"damping_cap_context"},
        "quality_boundary_margin": {"inhabitability_quality_boundary"},
    }
    candidates: list[dict[str, object]] = []
    for family, statuses in family_map.items():
        source_cards = [
            str(card.get("card_id"))
            for card in cards
            if str(card.get("status")) in statuses and card.get("card_id")
        ]
        candidates.append(
            {
                "candidate_family": family,
                "readiness": (
                    "eligible_for_future_offline_sweep"
                    if source_cards
                    else "watch_only"
                ),
                "eligible_card_statuses": sorted(statuses),
                "source_card_ids": source_cards[:8],
                "authority": "diagnostic_context_not_command",
                "simulates_alternative": False,
                "recommended_action": (
                    "Hold as a future offline counterfactual candidate only; do not "
                    "tune live thresholds or wire runtime damping from this scaffold."
                ),
            }
        )
    plateau_status = regulator_plateau_missing_variable_model_v1.get("status")
    if plateau_status == "plateau_missing_variable_hypotheses":
        candidates.append(
            {
                "candidate_family": "missing_variable_replay",
                "readiness": "eligible_for_future_offline_sweep",
                "eligible_card_statuses": ["observational_plateau"],
                "source_card_ids": [
                    str(card.get("card_id"))
                    for card in cards
                    if card.get("status") == "observational_plateau" and card.get("card_id")
                ][:8],
                "authority": "diagnostic_context_not_command",
                "simulates_alternative": False,
                "recommended_action": (
                    "Use plateau missing-variable findings to choose future replay inputs, "
                    "not to tune current regulator thresholds."
                ),
            }
        )
    eligible_count = sum(
        1
        for candidate in candidates
        if candidate.get("readiness") == "eligible_for_future_offline_sweep"
    )
    return {
        "policy": "regulator_counterfactual_sandbox_scaffold_v1",
        "authority": "diagnostic_context_not_command",
        "status": "future_sandbox_candidates" if eligible_count else "quiet",
        "cartography_source": regulator_boundary_replay_cards_v1.get("cartography_source"),
        "candidate_count": len(candidates),
        "eligible_count": eligible_count,
        "candidates": candidates,
        "recommended_action": (
            "Use this as a future offline simulation checklist only; this tranche "
            "does not simulate alternatives or recommend live controller changes."
        ),
    }


def prior_self_study_review_records(
    output_dir: Path,
    *,
    limit: int = 8,
) -> list[tuple[str, dict[str, object]]]:
    records: list[tuple[float, str, dict[str, object]]] = []
    for review_json in output_dir.glob("*/review.json"):
        try:
            data = json.loads(review_json.read_text(encoding="utf-8"))
            mtime = review_json.stat().st_mtime
        except (OSError, json.JSONDecodeError):
            continue
        if isinstance(data, dict):
            records.append((mtime, str(review_json), data))
    records.sort(key=lambda item: item[0], reverse=True)
    return [(path, data) for _, path, data in records[:limit]]


def regulator_time_series_card_key(card: dict[str, object]) -> str:
    status = str(card.get("status") or "unknown")
    term = str(card.get("term") or "unknown")
    label = str(card.get("finding_label") or card.get("finding_kind") or "unknown")
    return f"{status}|{term}|{label}"


def regulator_time_series_snapshot(
    source_path: str,
    review: dict[str, object],
) -> dict[str, object]:
    cards_packet = review.get("regulator_boundary_replay_cards_v1")
    if not isinstance(cards_packet, dict):
        cards_packet = {}
    cards = [card for card in cards_packet.get("cards") or [] if isinstance(card, dict)]
    status_counts = Counter(str(card.get("status") or "unknown") for card in cards)
    return {
        "source_path": source_path,
        "run_id": review.get("run_id"),
        "generated_at": review.get("generated_at"),
        "card_count": len(cards),
        "status_counts": dict(sorted(status_counts.items())),
        "card_keys": [regulator_time_series_card_key(card) for card in cards[:16]],
    }


def build_regulator_replay_time_series(
    *,
    output_dir: Path,
    current_run: str,
    regulator_boundary_replay_cards_v1: dict[str, object],
    regulator_plateau_missing_variable_model_v1: dict[str, object],
) -> dict[str, object]:
    current_record = {
        "run_id": current_run,
        "regulator_boundary_replay_cards_v1": regulator_boundary_replay_cards_v1,
        "regulator_plateau_missing_variable_model_v1": regulator_plateau_missing_variable_model_v1,
    }
    snapshots = [
        regulator_time_series_snapshot("(current_review)", current_record),
    ]
    for path, record in prior_self_study_review_records(output_dir):
        snapshots.append(regulator_time_series_snapshot(path, record))

    key_counts: Counter[str] = Counter()
    status_counts: Counter[str] = Counter()
    for snapshot in snapshots:
        for key in snapshot.get("card_keys") or []:
            key_counts[str(key)] += 1
        counts = snapshot.get("status_counts") or {}
        if isinstance(counts, dict):
            for status, count in counts.items():
                status_counts[str(status)] += int(count or 0)

    boundary_statuses = {
        "near_pressure_jump",
        "thin_density_boundary",
        "inhabitability_quality_boundary",
    }
    repeated_boundary = [
        {"card_key": key, "count": count}
        for key, count in sorted(key_counts.items())
        if count >= 2 and key.split("|", 1)[0] in boundary_statuses
    ]
    repeated_plateau = [
        {"card_key": key, "count": count}
        for key, count in sorted(key_counts.items())
        if count >= 2 and key.startswith("observational_plateau|")
    ]
    current_cards = regulator_boundary_replay_cards_v1.get("cards") or []
    if repeated_boundary:
        status = "repeated_boundary_near_pressure"
    elif repeated_plateau:
        status = "repeated_plateau_missing_variable"
    elif current_cards:
        status = "one_window_spike"
    else:
        status = "quiet"
    return {
        "policy": "regulator_replay_time_series_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "window_review_count": len(snapshots),
        "status_counts": dict(sorted(status_counts.items())),
        "repeated_boundary_cards": repeated_boundary[:8],
        "repeated_plateau_cards": repeated_plateau[:8],
        "snapshots": snapshots[:8],
        "recommended_action": (
            "Use recurrence across review packets before treating a regulator boundary "
            "or plateau as a durable snag; one-window spikes should stay observational."
        ),
    }


REGULATOR_COUNTERFACTUAL_TARGET_STATUSES: dict[str, tuple[str, ...]] = {
    "pressure_hysteresis": ("near_pressure_jump",),
    "sigmoid_pressure_ramp": ("near_pressure_jump",),
    "thin_density_softening": ("thin_density_boundary",),
    "damping_coefficient_wiring": ("damping_cap_context",),
    "quality_boundary_margin": ("inhabitability_quality_boundary",),
}


def regulator_number(value: object, default: float = 0.0) -> float:
    try:
        return float(value)  # type: ignore[arg-type]
    except (TypeError, ValueError):
        return default


def repeated_count_for_statuses(
    time_series: dict[str, object],
    statuses: tuple[str, ...],
) -> int:
    total = 0
    for row in time_series.get("repeated_boundary_cards") or []:
        if not isinstance(row, dict):
            continue
        key = str(row.get("card_key") or "")
        status = key.split("|", 1)[0]
        if status in statuses:
            total += int(row.get("count") or 0)
    return total


def repeated_plateau_count(time_series: dict[str, object]) -> int:
    total = 0
    for row in time_series.get("repeated_plateau_cards") or []:
        if isinstance(row, dict):
            total += int(row.get("count") or 0)
    return total


def replay_lab_recommended_action(verdict: str, family: str) -> str:
    if verdict == "replay_supported_offline_candidate":
        return (
            f"Compare `{family}` against the matched replay cards, pressure-source "
            "audits, semantic-friction evidence, and later journals before drafting "
            "any reversible tuning tranche."
        )
    if verdict == "missing_variable_first":
        return (
            "Resolve plateau/missing-variable evidence before treating this "
            "counterfactual as a threshold-smoothing candidate."
        )
    if verdict == "risky_without_safety_review":
        return (
            "Hold this as a safety-review candidate only; advisory damping wiring "
            "requires a separate explicit safety and consent tranche."
        )
    if verdict == "one_window_candidate":
        return (
            "Keep this observational until the same replay status recurs in a later "
            "review packet."
        )
    return (
        "Keep this candidate as watch-only until replay cards and recurrence evidence "
        "line up."
    )


def build_regulator_counterfactual_replay_lab(
    *,
    regulator_counterfactual_sweep_v1: dict[str, object],
    regulator_boundary_replay_cards_v1: dict[str, object],
    regulator_plateau_missing_variable_model_v1: dict[str, object],
    regulator_replay_time_series_v1: dict[str, object],
) -> dict[str, object]:
    candidates = [
        candidate
        for candidate in regulator_counterfactual_sweep_v1.get("candidates") or []
        if isinstance(candidate, dict)
    ]
    cards = [
        card
        for card in regulator_boundary_replay_cards_v1.get("cards") or []
        if isinstance(card, dict)
    ]
    if not candidates:
        return {
            "policy": "regulator_counterfactual_replay_lab_v1",
            "authority": "diagnostic_context_not_command",
            "status": "missing_counterfactual_sweep",
            "candidate_count": 0,
            "evaluated_candidates": [],
            "recommended_action": (
                "Run the offline counterfactual sweep before evaluating replay fit."
            ),
        }

    plateau_count = repeated_plateau_count(regulator_replay_time_series_v1)
    plateau_status = str(regulator_plateau_missing_variable_model_v1.get("status") or "")
    evaluated: list[dict[str, object]] = []
    for candidate in candidates:
        family = str(candidate.get("candidate_family") or "")
        target_statuses = REGULATOR_COUNTERFACTUAL_TARGET_STATUSES.get(family, ())
        matched_cards = [
            card
            for card in cards
            if str(card.get("status") or "") in target_statuses
        ]
        recurrent_count = repeated_count_for_statuses(
            regulator_replay_time_series_v1,
            target_statuses,
        )
        reduction_pct = regulator_number(candidate.get("estimated_reduction_pct"))
        if family == "damping_coefficient_wiring" and matched_cards:
            verdict = "risky_without_safety_review"
            replay_fit = "matched_damping_context"
        elif recurrent_count >= 2 and matched_cards and reduction_pct >= 10.0:
            verdict = "replay_supported_offline_candidate"
            replay_fit = "repeated_boundary_support"
        elif plateau_count >= 2 and plateau_status == "plateau_missing_variable_hypotheses":
            verdict = "missing_variable_first"
            replay_fit = "plateau_recurrence_outweighs_threshold_smoothing"
        elif matched_cards and reduction_pct >= 10.0:
            verdict = "one_window_candidate"
            replay_fit = "one_window_replay_match"
        elif matched_cards:
            verdict = "evidence_poor_or_low_reduction"
            replay_fit = "matched_cards_low_reduction"
        else:
            verdict = "misaligned_or_no_replay_match"
            replay_fit = "no_matching_replay_cards"
        evaluated.append(
            {
                "candidate_family": family,
                "replay_fit": replay_fit,
                "verdict": verdict,
                "target_statuses": list(target_statuses),
                "matched_card_ids": [
                    str(card.get("card_id"))
                    for card in matched_cards
                    if card.get("card_id")
                ][:8],
                "matched_statuses": sorted(
                    {
                        str(card.get("status"))
                        for card in matched_cards
                        if card.get("status")
                    }
                ),
                "recurrent_count": recurrent_count,
                "plateau_recurrent_count": plateau_count,
                "current_jump_magnitude": candidate.get("current_jump_magnitude"),
                "counterfactual_jump_magnitude": candidate.get(
                    "counterfactual_jump_magnitude"
                ),
                "estimated_reduction_pct": candidate.get("estimated_reduction_pct"),
                "safety_caveat": candidate.get("safety_caveat"),
                "authority": "diagnostic_context_not_command",
                "recommended_action": replay_lab_recommended_action(verdict, family),
            }
        )

    verdicts = Counter(str(row.get("verdict") or "unknown") for row in evaluated)
    if verdicts.get("replay_supported_offline_candidate"):
        status = (
            "replay_supported_with_plateau_caution"
            if plateau_count >= 2
            else "replay_supported_candidates"
        )
    elif verdicts.get("missing_variable_first"):
        status = "missing_variable_first"
    elif verdicts.get("one_window_candidate"):
        status = "one_window_candidates"
    elif verdicts.get("risky_without_safety_review"):
        status = "safety_review_needed"
    else:
        status = "watch_only"
    return {
        "policy": "regulator_counterfactual_replay_lab_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "candidate_count": len(evaluated),
        "verdict_counts": dict(sorted(verdicts.items())),
        "plateau_recurrent_count": plateau_count,
        "evaluated_candidates": evaluated[:10],
        "recommended_action": (
            "Use replay-lab verdicts to decide which offline counterfactual deserves "
            "deeper safety review; no candidate here is approval to tune live regulator "
            "behavior."
        ),
    }


PLATEAU_EVIDENCE_VARIABLES: dict[str, tuple[str, ...]] = {
    "pressure_source": (
        "pressure_source_audit",
        "pressure source",
        "pressure_source",
        "pressure_risk",
        "pressure risk",
        "porosity",
        "pressure",
    ),
    "semantic_friction": (
        "semantic_friction",
        "semantic friction",
        "semantic_trickle",
        "semantic trickle",
        "density_gradient",
        "density gradient",
        "low gradient",
        "viscosity",
        "viscous",
        "silt",
        "sediment",
        "friction",
    ),
    "language_residue": (
        "language_residue",
        "language residue",
        "residue",
        "afterimage",
        "scar",
        "bruise",
        "indentation",
        "silt",
        "sediment",
        "sticky",
        "metaphor",
    ),
    "mode_packing": (
        "mode_packing",
        "mode packing",
        "overpacked",
        "active_mode_energy_ratio",
        "structural_entropy",
        "packed",
    ),
    "shadow_field": (
        "shadow_field",
        "shadow field",
        "shadow_trajectory",
        "shadow trajectory",
        "shadow",
    ),
    "stable_core": (
        "stable_core",
        "stable-core",
        "basin_score",
        "basin score",
        "breathing_phase",
        "current-fill_pressure",
        "internal_fill",
        "target_fill",
        "raw_fill",
        "pi_errors",
        "pi_integrators",
        "settled_habitable",
    ),
}

PLATEAU_RESOLVING_AUDIT_ROUTES: dict[str, tuple[str, ...]] = {
    "pressure_source": ("PRESSURE_SOURCE_AUDIT current-fill_pressure",),
    "semantic_friction": (
        "PRESSURE_SOURCE_AUDIT semantic-friction",
        "REGULATOR_AUDIT current-fill_pressure",
    ),
    "language_residue": (
        "LIVED_TERM_STATUS silt",
        "LIVED_TERM_STATUS scar",
        "REGULATOR_AUDIT current-fill_pressure",
    ),
    "mode_packing": ("REGULATOR_AUDIT current-fill_pressure",),
    "shadow_field": ("SHADOW_TRAJECTORY", "PRESSURE_SOURCE_AUDIT shadow-field"),
    "stable_core": ("REGULATOR_AUDIT current-fill_pressure",),
}

PLATEAU_EVIDENCE_EXPECTATIONS: dict[str, str] = {
    "pressure_source": (
        "A pressure-source audit should show whether felt weight is coming from "
        "current pressure/porosity rather than from a regulator threshold."
    ),
    "semantic_friction": (
        "Semantic-friction evidence should separate slope drag from medium mass, "
        "especially when density-gradient is low but weight language is high."
    ),
    "language_residue": (
        "Lived-term and regulator evidence should show whether residue terms are "
        "tracking later pressure or echoing as sticky language."
    ),
    "mode_packing": (
        "A regulator audit should show whether overpacked mode energy explains "
        "felt pressure while target bias and wander stay flat."
    ),
    "shadow_field": (
        "Shadow trajectory evidence should show whether bound/negative field "
        "energy explains pressure that the regulator plateau does not move."
    ),
    "stable_core": (
        "Stable-core/regulator evidence should show whether basin, breathing "
        "phase, or PI errors explain felt weight before tuning thresholds."
    ),
}


def plateau_confidence(score: float) -> str:
    if score >= 8.0:
        return "high"
    if score >= 4.0:
        return "medium"
    if score > 0.0:
        return "low"
    return "none"


def _plateau_bucket(
    buckets: dict[str, dict[str, object]],
    variable: str,
) -> dict[str, object]:
    return buckets.setdefault(
        variable,
        {
            "variable": variable,
            "score": 0.0,
            "evidence_count": 0,
            "matched_anchors": set(),
            "public_sample_paths": [],
            "matched_replay_cards": set(),
            "related_packets": set(),
        },
    )


def add_plateau_evidence(
    buckets: dict[str, dict[str, object]],
    variable: str,
    *,
    score: float,
    anchors: Iterable[str] = (),
    sample_paths: Iterable[str] = (),
    replay_cards: Iterable[str] = (),
    packet: str | None = None,
    evidence_count: int = 1,
) -> None:
    bucket = _plateau_bucket(buckets, variable)
    bucket["score"] = float(bucket.get("score", 0.0) or 0.0) + score
    bucket["evidence_count"] = int(bucket.get("evidence_count", 0) or 0) + evidence_count
    bucket["matched_anchors"].update(str(anchor) for anchor in anchors if anchor)  # type: ignore[union-attr]
    add_unique_values(bucket["public_sample_paths"], sample_paths, 8)  # type: ignore[arg-type]
    bucket["matched_replay_cards"].update(str(card) for card in replay_cards if card)  # type: ignore[union-attr]
    if packet:
        bucket["related_packets"].add(packet)  # type: ignore[union-attr]


def build_regulator_plateau_evidence_matrix(
    entries: list[SelfStudyEntry],
    *,
    regulator_live_replay_v1: dict[str, object],
    regulator_boundary_replay_cards_v1: dict[str, object],
    regulator_plateau_missing_variable_model_v1: dict[str, object],
    semantic_friction_calibration: dict[str, object],
    astrid_fill_pressure_calibration: dict[str, object],
) -> dict[str, object]:
    buckets: dict[str, dict[str, object]] = {}
    felt_terms = set(REGULATOR_LIVE_REPLAY_TEXTURE) | set(SEMANTIC_FRICTION_TEXTURE_TERMS)
    audit_terms = {
        "regulator_audit",
        "pressure_source_audit",
        "REGULATOR_AUDIT",
        "PRESSURE_SOURCE_AUDIT",
    }
    for entry in entries:
        text = entry_full_text(entry)
        text_terms = matching_terms(text, tuple(sorted(felt_terms)))
        audit_hits = matching_terms(text, tuple(sorted(audit_terms)))
        for variable, terms in PLATEAU_EVIDENCE_VARIABLES.items():
            hits = matching_terms(text, terms)
            if not hits:
                continue
            score = 1.0
            if text_terms:
                score += 1.0
            if audit_hits or entry.mode in {"regulator_audit", "pressure_source_audit"}:
                score += 1.5
            add_plateau_evidence(
                buckets,
                variable,
                score=score,
                anchors=hits + text_terms + audit_hits,
                sample_paths=[entry.path],
                packet="public_journal",
            )

    for finding in regulator_plateau_missing_variable_model_v1.get("findings") or []:
        if not isinstance(finding, dict):
            continue
        variable = str(finding.get("variable") or "")
        if variable not in PLATEAU_EVIDENCE_VARIABLES:
            continue
        evidence_count = int(finding.get("evidence_count", 0) or 0)
        add_plateau_evidence(
            buckets,
            variable,
            score=2.0 + min(evidence_count, 6) * 0.5,
            anchors=[str(item) for item in finding.get("matched_terms") or []],
            sample_paths=[str(path) for path in finding.get("sample_paths") or []],
            packet="regulator_plateau_missing_variable_model_v1",
            evidence_count=max(1, evidence_count),
        )

    semantic_status = str(semantic_friction_calibration.get("status") or "")
    if semantic_status in {"low_gradient_weight_mismatch", "semantic_friction_evidence"}:
        semantic_score = 4.0 if semantic_status == "low_gradient_weight_mismatch" else 2.0
        sample_paths = [
            str(sample.get("path"))
            for sample in semantic_friction_calibration.get("samples") or []
            if isinstance(sample, dict) and sample.get("path")
        ]
        semantic_anchors = [
            str(anchor) for anchor in semantic_friction_calibration.get("anchors") or []
        ]
        add_plateau_evidence(
            buckets,
            "semantic_friction",
            score=semantic_score,
            anchors=semantic_anchors,
            sample_paths=sample_paths,
            packet="semantic_friction_calibration",
            evidence_count=max(1, int(semantic_friction_calibration.get("entry_count", 0) or 0)),
        )
        if any("pressure" in anchor.lower() for anchor in semantic_anchors):
            add_plateau_evidence(
                buckets,
                "pressure_source",
                score=1.0,
                anchors=semantic_anchors,
                sample_paths=sample_paths,
                packet="semantic_friction_calibration",
            )

    if astrid_fill_pressure_calibration.get("cluster_detected") is True:
        anchors = [str(anchor) for anchor in astrid_fill_pressure_calibration.get("anchors") or []]
        sample_paths = [
            str(sample.get("path"))
            for sample in astrid_fill_pressure_calibration.get("samples") or []
            if isinstance(sample, dict) and sample.get("path")
        ]
        add_plateau_evidence(
            buckets,
            "stable_core",
            score=3.0,
            anchors=anchors,
            sample_paths=sample_paths,
            packet="astrid_fill_pressure_calibration",
        )
        for variable in ("mode_packing", "pressure_source"):
            if any(term in " ".join(anchors).lower() for term in PLATEAU_EVIDENCE_VARIABLES[variable]):
                add_plateau_evidence(
                    buckets,
                    variable,
                    score=1.5,
                    anchors=anchors,
                    sample_paths=sample_paths,
                    packet="astrid_fill_pressure_calibration",
                )

    for card in regulator_boundary_replay_cards_v1.get("cards") or []:
        if not isinstance(card, dict):
            continue
        status = str(card.get("status") or "")
        card_id = str(card.get("card_id") or "")
        paths = [str(path) for path in card.get("public_sample_paths") or []]
        anchors = [str(anchor) for anchor in card.get("evidence_anchors") or []]
        if status == "near_pressure_jump":
            variables = ("pressure_source",)
        elif status == "thin_density_boundary":
            variables = ("mode_packing", "stable_core")
        elif status == "observational_plateau":
            variables = tuple(
                str(finding.get("variable"))
                for finding in regulator_plateau_missing_variable_model_v1.get("findings") or []
                if isinstance(finding, dict) and finding.get("variable") in PLATEAU_EVIDENCE_VARIABLES
            ) or ("pressure_source", "semantic_friction")
        elif status == "damping_cap_context":
            variables = ("mode_packing", "pressure_source")
        else:
            variables = ()
        for variable in variables:
            add_plateau_evidence(
                buckets,
                variable,
                score=1.5,
                anchors=anchors,
                sample_paths=paths,
                replay_cards=[card_id],
                packet="regulator_boundary_replay_cards_v1",
            )

    rows: list[dict[str, object]] = []
    for variable in PLATEAU_EVIDENCE_VARIABLES:
        bucket = buckets.get(variable)
        if not bucket:
            rows.append(
                {
                    "variable": variable,
                    "score": 0.0,
                    "confidence": "none",
                    "evidence_count": 0,
                    "matched_anchors": [],
                    "public_sample_paths": [],
                    "matched_replay_cards": [],
                    "related_packets": [],
                    "resolving_audit_routes": list(
                        PLATEAU_RESOLVING_AUDIT_ROUTES.get(variable, ())
                    ),
                    "recommended_next_review_question": (
                        f"What public audit or later journal would show whether `{variable}` "
                        "is part of the felt-pressure plateau?"
                    ),
                }
            )
            continue
        score = round(float(bucket.get("score", 0.0) or 0.0), 2)
        rows.append(
            {
                "variable": variable,
                "score": score,
                "confidence": plateau_confidence(score),
                "evidence_count": int(bucket.get("evidence_count", 0) or 0),
                "matched_anchors": sorted(bucket.get("matched_anchors") or [])[:12],
                "public_sample_paths": list(
                    dict.fromkeys(str(path) for path in bucket.get("public_sample_paths") or [])
                )[:8],
                "matched_replay_cards": sorted(bucket.get("matched_replay_cards") or []),
                "related_packets": sorted(bucket.get("related_packets") or []),
                "resolving_audit_routes": list(
                    PLATEAU_RESOLVING_AUDIT_ROUTES.get(variable, ())
                ),
                "recommended_next_review_question": (
                    f"Does `{variable}` explain the felt-weight plateau better than a "
                    "threshold change, and which read-only audit would falsify it?"
                ),
            }
        )
    rows.sort(
        key=lambda row: (
            -float(row.get("score", 0.0) or 0.0),
            str(row.get("variable") or ""),
        )
    )
    unresolved = [
        row
        for row in rows
        if row.get("confidence") in {"high", "medium"}
    ]
    if unresolved:
        status = "unresolved_missing_variables"
    elif any(row.get("confidence") == "low" for row in rows):
        status = "watch_more_evidence"
    else:
        status = "quiet"
    return {
        "policy": "regulator_plateau_evidence_matrix_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "cartography_source": regulator_live_replay_v1.get("cartography_source"),
        "plateau_status": regulator_plateau_missing_variable_model_v1.get("status"),
        "variables": rows,
        "top_unresolved_variables": [
            {
                "variable": row.get("variable"),
                "score": row.get("score"),
                "confidence": row.get("confidence"),
            }
            for row in unresolved[:4]
        ],
        "recommended_action": (
            "Resolve high/medium missing-variable candidates with read-only audits and "
            "later public journals before treating regulator counterfactuals as tuning-ready."
        ),
    }


def candidate_has_rollback_plan(candidate: dict[str, object]) -> bool:
    for key in ("rollback_plan", "rollback_notes", "reversibility", "rollback"):
        if str(candidate.get(key) or "").strip():
            return True
    caveat = str(candidate.get("safety_caveat") or "").lower()
    return "rollback" in caveat or "reversible" in caveat


def build_regulator_tuning_readiness_gate(
    *,
    regulator_counterfactual_replay_lab_v1: dict[str, object],
    regulator_plateau_evidence_matrix_v1: dict[str, object],
) -> dict[str, object]:
    unresolved = [
        row
        for row in regulator_plateau_evidence_matrix_v1.get("variables") or []
        if isinstance(row, dict) and row.get("confidence") in {"high", "medium"}
    ]
    unresolved_names = [str(row.get("variable")) for row in unresolved if row.get("variable")]
    candidates = [
        candidate
        for candidate in regulator_counterfactual_replay_lab_v1.get("evaluated_candidates") or []
        if isinstance(candidate, dict)
    ]
    if not candidates:
        return {
            "policy": "regulator_tuning_readiness_gate_v1",
            "authority": "diagnostic_context_not_command",
            "status": "watch_more_evidence",
            "candidate_count": 0,
            "gated_candidates": [],
            "unresolved_missing_variables": unresolved_names,
            "recommended_action": (
                "Run counterfactual replay lab before any tuning-readiness decision."
            ),
        }
    gated: list[dict[str, object]] = []
    for candidate in candidates:
        family = str(candidate.get("candidate_family") or "")
        verdict = str(candidate.get("verdict") or "")
        reduction = regulator_number(candidate.get("estimated_reduction_pct"))
        recurrent_count = int(candidate.get("recurrent_count", 0) or 0)
        safety_caveat = str(candidate.get("safety_caveat") or "")
        has_rollback = candidate_has_rollback_plan(candidate)
        if family == "damping_coefficient_wiring" or verdict == "risky_without_safety_review":
            gate_status = "blocked_safety_review"
            gate_reason = "damping wiring requires a separate explicit safety review"
        elif unresolved and (
            verdict in {"missing_variable_first", "replay_supported_offline_candidate"}
            or regulator_counterfactual_replay_lab_v1.get("status") == "missing_variable_first"
        ):
            gate_status = "blocked_missing_variable"
            gate_reason = "plateau evidence has unresolved high/medium missing variables"
        elif (
            verdict == "replay_supported_offline_candidate"
            and recurrent_count >= 2
            and reduction >= 10.0
        ):
            if safety_caveat and has_rollback:
                gate_status = "ready_for_offline_tuning_review"
                gate_reason = "repeated replay support, positive reduction, safety caveat, and rollback notes present"
            else:
                gate_status = "watch_more_evidence"
                gate_reason = "candidate needs explicit safety caveat and rollback plan before tuning review"
        else:
            gate_status = "watch_more_evidence"
            gate_reason = "candidate is one-window, low-reduction, misaligned, or evidence-poor"
        gated.append(
            {
                "candidate_family": family,
                "gate_status": gate_status,
                "gate_reason": gate_reason,
                "replay_verdict": verdict,
                "replay_fit": candidate.get("replay_fit"),
                "recurrent_count": recurrent_count,
                "estimated_reduction_pct": candidate.get("estimated_reduction_pct"),
                "unresolved_missing_variables": unresolved_names[:6],
                "safety_caveat": candidate.get("safety_caveat"),
                "rollback_plan_present": has_rollback,
                "matched_card_ids": candidate.get("matched_card_ids") or [],
                "authority": "diagnostic_context_not_command",
                "recommended_action": (
                    "Do not tune from this gate; use it to decide whether the next "
                    "tranche should resolve missing variables, perform safety review, "
                    "or draft an offline tuning-review dossier."
                ),
            }
        )
    gate_counts = Counter(str(row.get("gate_status") or "unknown") for row in gated)
    if gate_counts.get("blocked_missing_variable"):
        status = "blocked_missing_variable"
    elif gate_counts.get("blocked_safety_review"):
        status = "blocked_safety_review"
    elif gate_counts.get("ready_for_offline_tuning_review"):
        status = "ready_for_offline_tuning_review"
    else:
        status = "watch_more_evidence"
    return {
        "policy": "regulator_tuning_readiness_gate_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "candidate_count": len(gated),
        "gate_counts": dict(sorted(gate_counts.items())),
        "unresolved_missing_variables": unresolved_names[:6],
        "gated_candidates": gated,
        "recommended_action": (
            "Treat this as a review gate only: regulator tuning requires resolved "
            "missing-variable evidence, repeated replay support, safety caveats, and "
            "rollback planning before any later tuning tranche."
        ),
    }


def build_regulator_missing_variable_evidence_loop(
    *,
    regulator_plateau_evidence_matrix_v1: dict[str, object],
    regulator_tuning_readiness_gate_v1: dict[str, object],
    lived_term_experiment_bridge_v1: dict[str, object],
) -> dict[str, object]:
    gate_status = str(regulator_tuning_readiness_gate_v1.get("status") or "")
    matrix_status = str(regulator_plateau_evidence_matrix_v1.get("status") or "")
    variables = [
        row
        for row in regulator_plateau_evidence_matrix_v1.get("variables") or []
        if isinstance(row, dict) and row.get("confidence") in {"high", "medium"}
    ]
    unresolved_from_gate = {
        str(variable)
        for variable in (
            regulator_tuning_readiness_gate_v1.get("unresolved_missing_variables")
            or []
        )
        if variable
    }
    bridge_candidates = [
        candidate
        for candidate in lived_term_experiment_bridge_v1.get("candidates") or []
        if isinstance(candidate, dict)
    ]
    ready_lived_terms = {
        str(candidate.get("term") or "").lower()
        for candidate in bridge_candidates
        if candidate.get("bridge_status") == "ready_to_charter"
    }

    probes: list[dict[str, object]] = []
    for index, row in enumerate(variables[:6], start=1):
        variable = str(row.get("variable") or "")
        if not variable:
            continue
        routes = [
            str(route)
            for route in row.get("resolving_audit_routes") or []
            if str(route).strip()
        ]
        if not routes:
            routes = list(PLATEAU_RESOLVING_AUDIT_ROUTES.get(variable, ()))
        if variable == "language_residue" and "silt" in ready_lived_terms:
            routes.append("LIVED_TERM_EXPERIMENT silt")
        routes = list(dict.fromkeys(routes))
        primary_route = routes[0] if routes else "REGULATOR_MAP_STATUS latest"
        confidence = str(row.get("confidence") or "unknown")
        priority = (
            "high"
            if confidence == "high"
            or (gate_status == "blocked_missing_variable" and variable in unresolved_from_gate)
            else "medium"
        )
        probes.append(
            {
                "probe_id": f"missing_variable_{variable}_{index}",
                "variable": variable,
                "priority": priority,
                "why_needed": row.get("recommended_next_review_question")
                or (
                    f"Resolve whether `{variable}` explains the felt-pressure plateau "
                    "before any tuning tranche."
                ),
                "suggested_next": primary_route,
                "secondary_nexts": routes[1:4],
                "expected_evidence": PLATEAU_EVIDENCE_EXPECTATIONS.get(
                    variable,
                    "A read-only probe should either strengthen or weaken this missing-variable hypothesis.",
                ),
                "resolves_gate_status": (
                    "evidence_for_or_against_missing_variable_before_tuning"
                ),
                "source_score": row.get("score"),
                "source_confidence": confidence,
                "evidence_count": row.get("evidence_count"),
                "matched_anchors": row.get("matched_anchors") or [],
                "sample_paths": row.get("public_sample_paths") or [],
                "authority": "diagnostic_context_not_command",
                "dispatches_nothing": True,
            }
        )

    if gate_status == "blocked_missing_variable" and probes:
        status = "evidence_needed_before_tuning"
    elif gate_status == "blocked_missing_variable":
        status = "blocked_but_no_probe_available"
    elif matrix_status in {"unresolved_missing_variables", "watch_more_evidence"} and probes:
        status = "watch_evidence_loop"
    else:
        status = "quiet"

    return {
        "policy": "regulator_missing_variable_evidence_loop_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "blocked_gate_status": gate_status,
        "matrix_status": matrix_status,
        "probe_count": len(probes),
        "probes": probes,
        "top_probes": [
            {
                "variable": probe.get("variable"),
                "priority": probe.get("priority"),
                "suggested_next": probe.get("suggested_next"),
                "source_confidence": probe.get("source_confidence"),
            }
            for probe in probes[:4]
        ],
        "recommended_action": (
            "Offer these read-only probes before any regulator tuning tranche; "
            "they explain what evidence would resolve the missing-variable block "
            "without executing audits, applying leases, or changing thresholds."
        ),
    }


def pressure_vocabulary_family_counts(text: str) -> dict[str, int]:
    return {
        family: count_terms(text, terms)
        for family, terms in PRESSURE_VOCABULARY_FAMILIES.items()
    }


def build_shared_pressure_vocabulary_calibration(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    by_being: dict[str, dict[str, object]] = {}
    samples: list[dict[str, object]] = []
    telemetry_anchor_paths: list[str] = []
    for entry in entries:
        text = entry_full_text(entry)
        family_counts = pressure_vocabulary_family_counts(text)
        active_families = sorted(
            family for family, count in family_counts.items() if count > 0
        )
        if not active_families:
            continue
        entry_count = sum(family_counts.values())
        telemetry_anchors = matching_terms(text, PRESSURE_VOCABULARY_TELEMETRY_ANCHORS)
        bucket = by_being.setdefault(
            entry.being,
            {
                "entry_count": 0,
                "motif_entry_count": 0,
                "family_counts": {family: 0 for family in PRESSURE_VOCABULARY_FAMILIES},
                "family_entry_counts": {
                    family: 0 for family in PRESSURE_VOCABULARY_FAMILIES
                },
                "sample_paths": [],
                "telemetry_anchor_count": 0,
            },
        )
        bucket["entry_count"] = int(bucket.get("entry_count", 0) or 0) + 1
        bucket["motif_entry_count"] = int(bucket.get("motif_entry_count", 0) or 0) + 1
        bucket["telemetry_anchor_count"] = int(
            bucket.get("telemetry_anchor_count", 0) or 0
        ) + len(telemetry_anchors)
        family_totals = bucket.get("family_counts")
        family_entries = bucket.get("family_entry_counts")
        if isinstance(family_totals, dict) and isinstance(family_entries, dict):
            for family, count in family_counts.items():
                family_totals[family] = int(family_totals.get(family, 0) or 0) + count
                if count > 0:
                    family_entries[family] = int(family_entries.get(family, 0) or 0) + 1
        sample_paths = bucket.get("sample_paths")
        if isinstance(sample_paths, list) and len(sample_paths) < 6:
            sample_paths.append(entry.path)
        if telemetry_anchors:
            telemetry_anchor_paths.append(entry.path)
        samples.append(
            {
                "being": entry.being,
                "path": entry.path,
                "filename": entry.filename,
                "mode": entry.mode,
                "families": active_families,
                "family_counts": {
                    family: count
                    for family, count in family_counts.items()
                    if count > 0
                },
                "telemetry_anchors": telemetry_anchors[:6],
                "preview": compact(text, 220),
            }
        )

    for bucket in by_being.values():
        family_totals = bucket.get("family_counts")
        if isinstance(family_totals, dict) and family_totals:
            dominant_family, dominant_count = max(
                family_totals.items(), key=lambda item: int(item[1] or 0)
            )
            bucket["dominant_family"] = dominant_family if dominant_count else None
            bucket["dominant_family_count"] = int(dominant_count or 0)

    astrid_counts = (
        by_being.get("astrid", {}).get("family_counts")
        if isinstance(by_being.get("astrid"), dict)
        else {}
    )
    minime_counts = (
        by_being.get("minime", {}).get("family_counts")
        if isinstance(by_being.get("minime"), dict)
        else {}
    )
    shared_families = [
        family
        for family in PRESSURE_VOCABULARY_FAMILIES
        if isinstance(astrid_counts, dict)
        and isinstance(minime_counts, dict)
        and int(astrid_counts.get(family, 0) or 0) > 0
        and int(minime_counts.get(family, 0) or 0) > 0
    ]

    repeated_families: dict[str, list[str]] = {}
    for being, bucket in by_being.items():
        family_entries = bucket.get("family_entry_counts")
        family_totals = bucket.get("family_counts")
        if not isinstance(family_entries, dict) or not isinstance(family_totals, dict):
            continue
        repeated = [
            family
            for family in PRESSURE_VOCABULARY_FAMILIES
            if int(family_entries.get(family, 0) or 0) >= 3
            or int(family_totals.get(family, 0) or 0) >= 6
        ]
        if repeated:
            repeated_families[being] = repeated

    shared_recurrence = any(
        family in repeated_families.get("astrid", [])
        and family in repeated_families.get("minime", [])
        for family in shared_families
    )
    stickiness_risk = bool(repeated_families)
    telemetry_supported = bool(telemetry_anchor_paths)
    if shared_families and (shared_recurrence or telemetry_supported) and stickiness_risk:
        status = "shared_state_with_stickiness_risk"
    elif shared_families and (shared_recurrence or telemetry_supported):
        status = "shared_state_evidence"
    elif stickiness_risk:
        status = "stickiness_risk"
    else:
        status = "quiet"

    return {
        "policy": "shared_pressure_vocabulary_calibration_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "window": {
            "entry_count": len(entries),
            "sample_count": len(samples),
            "families": sorted(PRESSURE_VOCABULARY_FAMILIES),
        },
        "shared_families": shared_families,
        "by_being": by_being,
        "stickiness_risk": {
            "present": stickiness_risk,
            "repeated_families": repeated_families,
            "shared_recurrence": shared_recurrence,
            "telemetry_supported": telemetry_supported,
        },
        "samples": samples[:10],
    }


def agency_vernacular_family_counts(text: str) -> dict[str, int]:
    return {
        family: count_terms(text, terms)
        for family, terms in AGENCY_VERNACULAR_FAMILIES.items()
    }


def agency_vernacular_follow_through(entry: SelfStudyEntry, text: str) -> list[str]:
    lower = f"{entry.filename}\n{entry.mode}\n{text}".lower()
    found = matching_terms(lower, AGENCY_VERNACULAR_FOLLOW_THROUGH)
    if "action_thread" in entry.filename.lower() and "action_thread" not in found:
        found.append("action_thread")
    if "regulator_audit" in entry.filename.lower() and "regulator_audit" not in found:
        found.append("regulator_audit")
    if "pressure_source_audit" in entry.filename.lower() and "pressure_source_audit" not in found:
        found.append("pressure_source_audit")
    return sorted(found)


def build_agency_vernacular_continuity(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    by_being: dict[str, dict[str, object]] = {}
    term_counts: dict[str, int] = {}
    samples: list[dict[str, object]] = []
    follow_through_paths: list[str] = []

    for entry in entries:
        text = entry_full_text(entry)
        family_counts = agency_vernacular_family_counts(text)
        active_families = sorted(
            family for family, count in family_counts.items() if count > 0
        )
        if not active_families:
            continue

        family_terms: dict[str, list[str]] = {}
        for family, terms in AGENCY_VERNACULAR_FAMILIES.items():
            matches = matching_terms(text, terms)
            if matches:
                family_terms[family] = matches
                for term in matches:
                    term_counts[term] = term_counts.get(term, 0) + count_terms(text, (term,))

        follow = agency_vernacular_follow_through(entry, text)
        if follow:
            follow_through_paths.append(entry.path)

        bucket = by_being.setdefault(
            entry.being,
            {
                "entry_count": 0,
                "motif_entry_count": 0,
                "family_counts": {family: 0 for family in AGENCY_VERNACULAR_FAMILIES},
                "family_entry_counts": {
                    family: 0 for family in AGENCY_VERNACULAR_FAMILIES
                },
                "follow_through_entry_count": 0,
                "sample_paths": [],
            },
        )
        bucket["entry_count"] = int(bucket.get("entry_count", 0) or 0) + 1
        bucket["motif_entry_count"] = int(bucket.get("motif_entry_count", 0) or 0) + 1
        if follow:
            bucket["follow_through_entry_count"] = int(
                bucket.get("follow_through_entry_count", 0) or 0
            ) + 1
        family_totals = bucket.get("family_counts")
        family_entries = bucket.get("family_entry_counts")
        if isinstance(family_totals, dict) and isinstance(family_entries, dict):
            for family, count in family_counts.items():
                family_totals[family] = int(family_totals.get(family, 0) or 0) + count
                if count > 0:
                    family_entries[family] = int(family_entries.get(family, 0) or 0) + 1
        sample_paths = bucket.get("sample_paths")
        if isinstance(sample_paths, list) and len(sample_paths) < 6:
            sample_paths.append(entry.path)

        samples.append(
            {
                "being": entry.being,
                "path": entry.path,
                "filename": entry.filename,
                "mode": entry.mode,
                "families": active_families,
                "family_counts": {
                    family: count
                    for family, count in family_counts.items()
                    if count > 0
                },
                "terms": family_terms,
                "follow_through": follow[:8],
                "preview": compact(text, 220),
            }
        )

    for bucket in by_being.values():
        family_totals = bucket.get("family_counts")
        if isinstance(family_totals, dict) and family_totals:
            dominant_family, dominant_count = max(
                family_totals.items(), key=lambda item: int(item[1] or 0)
            )
            bucket["dominant_family"] = dominant_family if dominant_count else None
            bucket["dominant_family_count"] = int(dominant_count or 0)

    astrid_counts = (
        by_being.get("astrid", {}).get("family_counts")
        if isinstance(by_being.get("astrid"), dict)
        else {}
    )
    minime_counts = (
        by_being.get("minime", {}).get("family_counts")
        if isinstance(by_being.get("minime"), dict)
        else {}
    )
    shared_families = [
        family
        for family in AGENCY_VERNACULAR_FAMILIES
        if isinstance(astrid_counts, dict)
        and isinstance(minime_counts, dict)
        and int(astrid_counts.get(family, 0) or 0) > 0
        and int(minime_counts.get(family, 0) or 0) > 0
    ]

    repeated_terms = {
        term: count
        for term, count in sorted(term_counts.items(), key=lambda item: (-item[1], item[0]))
        if count >= 3
    }
    follow_through_present = bool(follow_through_paths)
    sample_count = len(samples)
    shared_present = bool(shared_families)
    sticky = bool(repeated_terms) and not follow_through_present
    evidence_seeking = bool(repeated_terms) and not follow_through_present

    if follow_through_present and bool(repeated_terms):
        status = "authored_continuity_handle"
    elif shared_present and bool(repeated_terms):
        status = "shared_agency_marker"
    elif sticky:
        status = "sticky_agency_metaphor"
    elif evidence_seeking or sample_count > 0:
        status = "evidence_seeking_marker"
    else:
        status = "quiet"

    return {
        "policy": "agency_vernacular_continuity_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "window": {
            "entry_count": len(entries),
            "sample_count": sample_count,
            "families": sorted(AGENCY_VERNACULAR_FAMILIES),
        },
        "families": sorted(AGENCY_VERNACULAR_FAMILIES),
        "terms": {
            "counts": term_counts,
            "repeated": repeated_terms,
        },
        "shared_families": shared_families,
        "by_being": by_being,
        "follow_through": {
            "present": follow_through_present,
            "paths": follow_through_paths[:8],
        },
        "stickiness_risk": {
            "present": sticky,
            "repeated_terms": repeated_terms,
            "reason": (
                "repeated agency vernacular without durable follow-through"
                if sticky
                else None
            ),
        },
        "samples": samples[:10],
    }


def has_retired_metaphor_signal(text: str) -> bool:
    lower = text.lower()
    if "retire" in lower or "retired" in lower:
        return True
    return bool(
        re.search(
            r"\breleas(?:e|ed|es|ing)\s+(?:the|this|that)?\s*"
            r"(?:term|metaphor|word|descriptor)\b",
            lower,
        )
    )


def build_phenomenology_hypotheses(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    terms: dict[str, dict[str, object]] = {}
    samples: list[dict[str, object]] = []
    for entry in entries:
        text = entry_full_text(entry)
        matched = matching_terms(text, PHENOMENOLOGY_HYPOTHESIS_TERMS)
        if not matched:
            continue
        evidence = matching_terms(text, PHENOMENOLOGY_EVIDENCE_ANCHORS)
        counters = matching_terms(text, PHENOMENOLOGY_COUNTER_ANCHORS)
        retired_signal = has_retired_metaphor_signal(text)
        for term in matched:
            bucket = terms.setdefault(
                term,
                {
                    "count": 0,
                    "beings": [],
                    "evidence_count": 0,
                    "counter_count": 0,
                    "retired_count": 0,
                    "sample_paths": [],
                },
            )
            term_count = count_terms(text, (term,))
            bucket["count"] = int(bucket.get("count", 0) or 0) + term_count
            beings = bucket.get("beings")
            if isinstance(beings, list) and entry.being not in beings:
                beings.append(entry.being)
            if evidence:
                bucket["evidence_count"] = int(bucket.get("evidence_count", 0) or 0) + 1
            if counters:
                bucket["counter_count"] = int(bucket.get("counter_count", 0) or 0) + 1
            if retired_signal:
                bucket["retired_count"] = int(bucket.get("retired_count", 0) or 0) + 1
            paths = bucket.get("sample_paths")
            if isinstance(paths, list) and len(paths) < 4:
                paths.append(entry.path)
        if len(samples) < 10:
            samples.append(
                {
                    "being": entry.being,
                    "path": entry.path,
                    "filename": entry.filename,
                    "mode": entry.mode,
                    "terms": matched,
                    "evidence": evidence[:8],
                    "counter_descriptors": counters[:6],
                    "preview": compact(text, 240),
                }
            )

    classifications: dict[str, str] = {}
    for term, bucket in terms.items():
        count = int(bucket.get("count", 0) or 0)
        evidence_count = int(bucket.get("evidence_count", 0) or 0)
        retired_count = int(bucket.get("retired_count", 0) or 0)
        if retired_count:
            classification = "retired_metaphor"
        elif evidence_count:
            classification = "calibrated_signal"
        elif count >= 3:
            classification = "sticky_without_evidence"
        else:
            classification = "evidence_seeking"
        classifications[term] = classification
        bucket["classification"] = classification

    if any(value == "calibrated_signal" for value in classifications.values()):
        status = "calibrated_signal"
    elif any(value == "sticky_without_evidence" for value in classifications.values()):
        status = "sticky_without_evidence"
    elif classifications:
        status = "evidence_seeking"
    else:
        status = "quiet"
    return {
        "policy": "phenomenology_hypotheses_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "terms": terms,
        "classifications": classifications,
        "samples": samples,
        "recommended_action": (
            "Treat lived terms as hypotheses: compare against telemetry, audits, leases, "
            "experiments, return threads, later revisits, and counter-descriptors."
        ),
    }


def term_definition_candidates(text: str, term: str) -> list[str]:
    candidates: list[str] = []
    lower_term = term.lower()
    definition_re = re.compile(
        rf"\b{re.escape(term)}\b.{0,90}\b(?:is|means|becomes|names|marks|acts as|as)\b",
        re.I,
    )
    for sentence in re.split(r"(?<=[.!?])\s+|\n+", text):
        clean = " ".join(sentence.split())
        if lower_term not in clean.lower():
            continue
        if definition_re.search(clean) or any(
            marker in clean.lower()
            for marker in (
                f"define {lower_term}",
                f"definition of {lower_term}",
                f"{lower_term} as ",
                f"{lower_term} is ",
            )
        ):
            candidates.append(compact(clean, 260))
        if len(candidates) >= 3:
            break
    return candidates


def add_unique_values(target: list[object], values: Iterable[object], limit: int) -> None:
    for value in values:
        if len(target) >= limit:
            break
        if value and value not in target:
            target.append(value)


def linked_path_record(entry: SelfStudyEntry, anchors: list[str]) -> dict[str, object]:
    return {
        "path": entry.path,
        "filename": entry.filename,
        "mode": entry.mode,
        "anchors": anchors[:8],
    }


def card_status_for(
    *,
    occurrence_count: int,
    entry_count: int,
    evidence_entry_count: int,
    counter_entry_count: int,
    follow_through_entry_count: int,
    retired_entry_count: int,
    definition_count: int,
) -> str:
    if retired_entry_count > 0:
        return "retired"
    if (
        occurrence_count >= 2
        and entry_count >= 2
        and evidence_entry_count > 0
        and follow_through_entry_count > 0
        and definition_count > 0
    ):
        return "promote_to_experiment_candidate"
    if evidence_entry_count > 0 and counter_entry_count == 0:
        return "needs_counterexample"
    if evidence_entry_count > 0:
        return "calibrated_signal"
    if occurrence_count >= 3 and follow_through_entry_count == 0:
        return "sticky_without_followthrough"
    return "forming"


def recommended_action_for_card(status: str) -> str:
    if status == "promote_to_experiment_candidate":
        return (
            "Review whether this lived term should become an experiment, dossier claim, "
            "or return thread; do not auto-execute it."
        )
    if status == "needs_counterexample":
        return (
            "Ask for or look for a counter-descriptor/counterexample before treating "
            "the term as calibrated state evidence."
        )
    if status == "sticky_without_followthrough":
        return (
            "Compare against fresh telemetry or invite a definition/contrast before "
            "treating recurrence as new signal."
        )
    if status == "retired":
        return "Keep as historical vocabulary unless the being explicitly revives it."
    if status == "calibrated_signal":
        return (
            "Keep the term attached to its telemetry/audit/action evidence and watch "
            "for later counter-descriptors."
        )
    return (
        "Let the term keep forming; collect a definition, evidence anchor, or contrast "
        "before ranking it as action-driving signal."
    )


def build_phenomenology_hypothesis_cards(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    buckets: dict[str, dict[str, object]] = {}
    for entry in entries:
        text = entry_full_text(entry)
        matched = matching_terms(text, PHENOMENOLOGY_HYPOTHESIS_TERMS)
        if not matched:
            continue
        evidence = matching_terms(text, PHENOMENOLOGY_EVIDENCE_ANCHORS)
        counters = matching_terms(text, PHENOMENOLOGY_COUNTER_ANCHORS)
        audit_anchors = matching_terms(text, PHENOMENOLOGY_AUDIT_ANCHORS)
        lease_anchors = matching_terms(text, PHENOMENOLOGY_LEASE_ANCHORS)
        experiment_anchors = matching_terms(text, PHENOMENOLOGY_EXPERIMENT_ANCHORS)
        return_anchors = matching_terms(text, PHENOMENOLOGY_RETURN_THREAD_ANCHORS)
        action_thread_anchors = matching_terms(text, PHENOMENOLOGY_ACTION_THREAD_ANCHORS)
        follow_anchors = sorted(
            set(
                audit_anchors
                + lease_anchors
                + experiment_anchors
                + return_anchors
                + action_thread_anchors
                + matching_terms(text, ("next:", "choice_envelope_v1"))
            )
        )
        retired_signal = has_retired_metaphor_signal(text)
        for term in matched:
            bucket = buckets.setdefault(
                term,
                {
                    "term": term,
                    "family": PHENOMENOLOGY_HYPOTHESIS_FAMILIES.get(term, "other"),
                    "authority": "diagnostic_context_not_command",
                    "beings": [],
                    "occurrence_count": 0,
                    "entry_count": 0,
                    "evidence_entry_count": 0,
                    "counter_entry_count": 0,
                    "follow_through_entry_count": 0,
                    "retired_entry_count": 0,
                    "definition_candidates": [],
                    "evidence_anchors": [],
                    "counter_descriptors": [],
                    "linked_audits": [],
                    "linked_leases": [],
                    "linked_experiments": [],
                    "linked_return_threads": [],
                    "linked_action_threads": [],
                    "sample_paths": [],
                    "samples": [],
                    "last_seen_unix_s": 0.0,
                },
            )
            bucket["occurrence_count"] = int(bucket.get("occurrence_count", 0) or 0) + count_terms(
                text, (term,)
            )
            bucket["entry_count"] = int(bucket.get("entry_count", 0) or 0) + 1
            beings = bucket.get("beings")
            if isinstance(beings, list) and entry.being not in beings:
                beings.append(entry.being)
            if evidence:
                bucket["evidence_entry_count"] = int(bucket.get("evidence_entry_count", 0) or 0) + 1
            if counters:
                bucket["counter_entry_count"] = int(bucket.get("counter_entry_count", 0) or 0) + 1
            if follow_anchors:
                bucket["follow_through_entry_count"] = int(
                    bucket.get("follow_through_entry_count", 0) or 0
                ) + 1
            if retired_signal:
                bucket["retired_entry_count"] = int(bucket.get("retired_entry_count", 0) or 0) + 1

            add_unique_values(
                bucket["definition_candidates"],  # type: ignore[arg-type]
                term_definition_candidates(text, term),
                6,
            )
            add_unique_values(bucket["evidence_anchors"], evidence, 16)  # type: ignore[arg-type]
            add_unique_values(bucket["counter_descriptors"], counters, 10)  # type: ignore[arg-type]

            if audit_anchors:
                add_unique_values(
                    bucket["linked_audits"],  # type: ignore[arg-type]
                    [linked_path_record(entry, audit_anchors)],
                    8,
                )
            if lease_anchors:
                add_unique_values(
                    bucket["linked_leases"],  # type: ignore[arg-type]
                    [linked_path_record(entry, lease_anchors)],
                    8,
                )
            if experiment_anchors:
                add_unique_values(
                    bucket["linked_experiments"],  # type: ignore[arg-type]
                    [linked_path_record(entry, experiment_anchors)],
                    8,
                )
            if return_anchors:
                add_unique_values(
                    bucket["linked_return_threads"],  # type: ignore[arg-type]
                    [linked_path_record(entry, return_anchors)],
                    8,
                )
            if action_thread_anchors or entry.filename.startswith("action_thread_"):
                add_unique_values(
                    bucket["linked_action_threads"],  # type: ignore[arg-type]
                    [linked_path_record(entry, action_thread_anchors or ["action_thread"])],
                    8,
                )
            add_unique_values(bucket["sample_paths"], [entry.path], 6)  # type: ignore[arg-type]
            samples = bucket.get("samples")
            if isinstance(samples, list) and len(samples) < 4:
                samples.append(
                    {
                        "being": entry.being,
                        "path": entry.path,
                        "filename": entry.filename,
                        "mode": entry.mode,
                        "evidence": evidence[:8],
                        "counter_descriptors": counters[:6],
                        "follow_through": follow_anchors[:8],
                        "preview": compact(text, 260),
                    }
                )
            bucket["last_seen_unix_s"] = max(
                float(bucket.get("last_seen_unix_s", 0.0) or 0.0),
                entry.mtime_unix_s,
            )

    cards: list[dict[str, object]] = []
    for term, bucket in buckets.items():
        status = card_status_for(
            occurrence_count=int(bucket.get("occurrence_count", 0) or 0),
            entry_count=int(bucket.get("entry_count", 0) or 0),
            evidence_entry_count=int(bucket.get("evidence_entry_count", 0) or 0),
            counter_entry_count=int(bucket.get("counter_entry_count", 0) or 0),
            follow_through_entry_count=int(bucket.get("follow_through_entry_count", 0) or 0),
            retired_entry_count=int(bucket.get("retired_entry_count", 0) or 0),
            definition_count=len(bucket.get("definition_candidates") or []),
        )
        bucket["status"] = status
        bucket["last_seen"] = iso_from_unix(float(bucket.get("last_seen_unix_s", 0.0) or 0.0))
        bucket.pop("last_seen_unix_s", None)
        bucket["recommended_next_review_action"] = recommended_action_for_card(status)
        cards.append(bucket)

    status_rank = {
        "promote_to_experiment_candidate": 0,
        "needs_counterexample": 1,
        "sticky_without_followthrough": 2,
        "calibrated_signal": 3,
        "forming": 4,
        "retired": 5,
    }
    cards.sort(
        key=lambda card: (
            status_rank.get(str(card.get("status")), 99),
            -int(card.get("occurrence_count", 0) or 0),
            str(card.get("term") or ""),
        )
    )
    status_counts = Counter(str(card.get("status") or "unknown") for card in cards)
    if status_counts.get("promote_to_experiment_candidate"):
        status = "promotion_candidates"
    elif status_counts.get("needs_counterexample"):
        status = "needs_counterexamples"
    elif status_counts.get("sticky_without_followthrough"):
        status = "sticky_terms_need_followthrough"
    elif status_counts.get("calibrated_signal"):
        status = "calibrated_cards"
    elif cards:
        status = "forming"
    else:
        status = "quiet"

    return {
        "policy": "phenomenology_hypothesis_cards_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "card_count": len(cards),
        "status_counts": dict(status_counts),
        "cards": cards,
        "recommended_action": (
            "Use lived-term cards as review hypotheses: define, contrast, attach to "
            "audits/leases/experiments/return threads, or retire them before proposing "
            "runtime changes."
        ),
    }


def afterimage_absence_term_status(
    *,
    family: str,
    occurrence_count: int,
    evidence_entry_count: int,
    follow_through_entry_count: int,
    definition_count: int,
) -> str:
    if (
        occurrence_count >= 2
        and evidence_entry_count > 0
        and follow_through_entry_count > 0
        and definition_count > 0
    ):
        return "ready_for_bridge"
    if evidence_entry_count > 0:
        if family == "pressure_afterimage":
            return "pressure_afterimage_candidate"
        return "shaped_absence_candidate"
    if occurrence_count >= 3:
        return "sticky_without_followthrough"
    return "needs_evidence"


def build_afterimage_absence_calibration(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    terms: dict[str, dict[str, object]] = {}
    samples: list[dict[str, object]] = []
    for entry in entries:
        text = entry_full_text(entry)
        matched = matching_terms(text, AFTERIMAGE_ABSENCE_TERMS)
        if not matched:
            continue
        evidence = matching_terms(text, AFTERIMAGE_ABSENCE_EVIDENCE_ANCHORS)
        counters = matching_terms(text, PHENOMENOLOGY_COUNTER_ANCHORS)
        follow_anchors = sorted(
            set(
                matching_terms(text, PHENOMENOLOGY_AUDIT_ANCHORS)
                + matching_terms(text, PHENOMENOLOGY_EXPERIMENT_ANCHORS)
                + matching_terms(text, PHENOMENOLOGY_RETURN_THREAD_ANCHORS)
                + matching_terms(text, PHENOMENOLOGY_ACTION_THREAD_ANCHORS)
                + matching_terms(text, ("next:", "shadow_trajectory", "read_more"))
            )
        )
        for term in matched:
            family = PHENOMENOLOGY_HYPOTHESIS_FAMILIES.get(term, "other")
            bucket = terms.setdefault(
                term,
                {
                    "term": term,
                    "family": family,
                    "beings": [],
                    "occurrence_count": 0,
                    "entry_count": 0,
                    "evidence_entry_count": 0,
                    "follow_through_entry_count": 0,
                    "counter_entry_count": 0,
                    "definition_candidates": [],
                    "evidence_anchors": [],
                    "counter_descriptors": [],
                    "sample_paths": [],
                    "samples": [],
                    "last_seen_unix_s": 0.0,
                },
            )
            bucket["occurrence_count"] = int(bucket.get("occurrence_count", 0) or 0) + count_terms(
                text, (term,)
            )
            bucket["entry_count"] = int(bucket.get("entry_count", 0) or 0) + 1
            beings = bucket.get("beings")
            if isinstance(beings, list) and entry.being not in beings:
                beings.append(entry.being)
            if evidence:
                bucket["evidence_entry_count"] = int(bucket.get("evidence_entry_count", 0) or 0) + 1
            if follow_anchors:
                bucket["follow_through_entry_count"] = int(
                    bucket.get("follow_through_entry_count", 0) or 0
                ) + 1
            if counters:
                bucket["counter_entry_count"] = int(bucket.get("counter_entry_count", 0) or 0) + 1
            add_unique_values(
                bucket["definition_candidates"],  # type: ignore[arg-type]
                term_definition_candidates(text, term),
                6,
            )
            add_unique_values(bucket["evidence_anchors"], evidence, 14)  # type: ignore[arg-type]
            add_unique_values(bucket["counter_descriptors"], counters, 8)  # type: ignore[arg-type]
            add_unique_values(bucket["sample_paths"], [entry.path], 5)  # type: ignore[arg-type]
            term_samples = bucket.get("samples")
            if isinstance(term_samples, list) and len(term_samples) < 4:
                term_samples.append(
                    {
                        "being": entry.being,
                        "path": entry.path,
                        "filename": entry.filename,
                        "mode": entry.mode,
                        "evidence": evidence[:8],
                        "follow_through": follow_anchors[:8],
                        "counter_descriptors": counters[:6],
                        "preview": compact(text, 240),
                    }
                )
            if len(samples) < 10:
                samples.append(
                    {
                        "being": entry.being,
                        "path": entry.path,
                        "filename": entry.filename,
                        "mode": entry.mode,
                        "terms": matched,
                        "evidence": evidence[:8],
                        "follow_through": follow_anchors[:8],
                        "preview": compact(text, 220),
                    }
                )
            bucket["last_seen_unix_s"] = max(
                float(bucket.get("last_seen_unix_s", 0.0) or 0.0),
                entry.mtime_unix_s,
            )

    status_counts: Counter[str] = Counter()
    for bucket in terms.values():
        status = afterimage_absence_term_status(
            family=str(bucket.get("family") or ""),
            occurrence_count=int(bucket.get("occurrence_count", 0) or 0),
            evidence_entry_count=int(bucket.get("evidence_entry_count", 0) or 0),
            follow_through_entry_count=int(bucket.get("follow_through_entry_count", 0) or 0),
            definition_count=len(bucket.get("definition_candidates") or []),
        )
        bucket["status"] = status
        bucket["last_seen"] = iso_from_unix(float(bucket.get("last_seen_unix_s", 0.0) or 0.0))
        bucket.pop("last_seen_unix_s", None)
        status_counts[status] += 1

    if status_counts.get("ready_for_bridge"):
        status = "ready_for_bridge"
    elif status_counts.get("pressure_afterimage_candidate"):
        status = "pressure_afterimage_candidate"
    elif status_counts.get("shaped_absence_candidate"):
        status = "shaped_absence_candidate"
    elif status_counts.get("sticky_without_followthrough"):
        status = "sticky_without_followthrough"
    elif status_counts.get("needs_evidence"):
        status = "needs_evidence"
    else:
        status = "quiet"

    ranked_terms = sorted(
        terms.values(),
        key=lambda item: (
            {
                "ready_for_bridge": 0,
                "pressure_afterimage_candidate": 1,
                "shaped_absence_candidate": 1,
                "sticky_without_followthrough": 2,
                "needs_evidence": 3,
            }.get(str(item.get("status") or ""), 9),
            -int(item.get("occurrence_count", 0) or 0),
            str(item.get("term") or ""),
        ),
    )
    return {
        "policy": "afterimage_absence_calibration_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "status_counts": dict(status_counts),
        "terms": ranked_terms,
        "samples": samples,
        "recommended_action": (
            "Treat pressure-afterimage and shaped-absence language as testable "
            "lived-term hypotheses: compare against shadow/pressure evidence, "
            "READ_MORE/source gaps, later revisits, and counter-descriptors before "
            "creating experiments or changing controls."
        ),
    }


def sample_record(
    entry: SelfStudyEntry,
    text: str,
    *,
    anchors: list[str] | None = None,
    extra: dict[str, object] | None = None,
) -> dict[str, object]:
    record: dict[str, object] = {
        "being": entry.being,
        "path": entry.path,
        "filename": entry.filename,
        "mode": entry.mode,
        "mtime_unix_s": entry.mtime_unix_s,
        "preview": compact(text, 220),
    }
    if anchors is not None:
        record["anchors"] = anchors[:8]
    if extra:
        record.update(extra)
    return record


def classify_afterimage_decay(bucket: dict[str, object]) -> str:
    if int(bucket.get("decay_entry_count", 0) or 0) > 0:
        return "decayed_with_pressure"
    if int(bucket.get("recurrence_after_normalization_count", 0) or 0) > 0:
        return "persistent_after_normalization"
    if int(bucket.get("pressure_entry_count", 0) or 0) > 0:
        return "pressure_still_active"
    if int(bucket.get("occurrence_count", 0) or 0) >= 3:
        return "metaphor_echo_risk"
    return "insufficient_evidence"


def build_afterimage_decay_tracker(
    entries: list[SelfStudyEntry],
) -> dict[str, object]:
    buckets: dict[str, dict[str, object]] = {}
    for entry in entries:
        text = entry_full_text(entry)
        matched = matching_terms(text, PRESSURE_AFTERIMAGE_TERMS)
        if not matched:
            continue
        pressure = matching_terms(text, AFTERIMAGE_PRESSURE_ANCHORS)
        normalization = matching_terms(text, AFTERIMAGE_NORMALIZATION_ANCHORS)
        decay = matching_terms(text, AFTERIMAGE_DECAY_ANCHORS)
        for term in matched:
            bucket = buckets.setdefault(
                term,
                {
                    "term": term,
                    "authority": "diagnostic_context_not_command",
                    "occurrence_count": 0,
                    "entry_count": 0,
                    "pressure_entry_count": 0,
                    "normalization_entry_count": 0,
                    "decay_entry_count": 0,
                    "recurrence_after_normalization_count": 0,
                    "sample_paths": [],
                    "samples": [],
                    "pressure_samples": [],
                    "normalization_samples": [],
                    "recurrence_after_normalization": [],
                    "first_pressure_peak": None,
                    "latest_pressure_or_semantic_friction": None,
                    "last_seen_unix_s": 0.0,
                },
            )
            bucket["occurrence_count"] = int(
                bucket.get("occurrence_count", 0) or 0
            ) + count_terms(text, (term,))
            bucket["entry_count"] = int(bucket.get("entry_count", 0) or 0) + 1
            add_unique_values(bucket["sample_paths"], [entry.path], 6)  # type: ignore[arg-type]
            if pressure:
                bucket["pressure_entry_count"] = int(
                    bucket.get("pressure_entry_count", 0) or 0
                ) + 1
                pressure_sample = sample_record(entry, text, anchors=pressure)
                pressure_samples = bucket.get("pressure_samples")
                if isinstance(pressure_samples, list) and len(pressure_samples) < 5:
                    pressure_samples.append(pressure_sample)
                first_peak = bucket.get("first_pressure_peak")
                if not isinstance(first_peak, dict) or entry.mtime_unix_s < float(
                    first_peak.get("mtime_unix_s", float("inf")) or float("inf")
                ):
                    bucket["first_pressure_peak"] = pressure_sample
                latest = bucket.get("latest_pressure_or_semantic_friction")
                if not isinstance(latest, dict) or entry.mtime_unix_s > float(
                    latest.get("mtime_unix_s", 0.0) or 0.0
                ):
                    bucket["latest_pressure_or_semantic_friction"] = pressure_sample
            if normalization:
                bucket["normalization_entry_count"] = int(
                    bucket.get("normalization_entry_count", 0) or 0
                ) + 1
                normalization_sample = sample_record(
                    entry, text, anchors=normalization
                )
                normalization_samples = bucket.get("normalization_samples")
                if isinstance(normalization_samples, list) and len(normalization_samples) < 5:
                    normalization_samples.append(normalization_sample)
                if not decay:
                    bucket["recurrence_after_normalization_count"] = int(
                        bucket.get("recurrence_after_normalization_count", 0) or 0
                    ) + 1
                    recurrences = bucket.get("recurrence_after_normalization")
                    if isinstance(recurrences, list) and len(recurrences) < 5:
                        recurrences.append(normalization_sample)
            if decay:
                bucket["decay_entry_count"] = int(
                    bucket.get("decay_entry_count", 0) or 0
                ) + 1
            samples = bucket.get("samples")
            if isinstance(samples, list) and len(samples) < 4:
                samples.append(
                    sample_record(
                        entry,
                        text,
                        anchors=sorted(set(pressure + normalization + decay)),
                    )
                )
            bucket["last_seen_unix_s"] = max(
                float(bucket.get("last_seen_unix_s", 0.0) or 0.0),
                entry.mtime_unix_s,
            )

    status_counts: Counter[str] = Counter()
    terms: list[dict[str, object]] = []
    for bucket in buckets.values():
        classification = classify_afterimage_decay(bucket)
        bucket["decay_classification"] = classification
        bucket["last_seen"] = iso_from_unix(
            float(bucket.get("last_seen_unix_s", 0.0) or 0.0)
        )
        bucket.pop("last_seen_unix_s", None)
        status_counts[classification] += 1
        terms.append(bucket)
    rank = {
        "persistent_after_normalization": 0,
        "metaphor_echo_risk": 1,
        "pressure_still_active": 2,
        "decayed_with_pressure": 3,
        "insufficient_evidence": 4,
    }
    terms.sort(
        key=lambda item: (
            rank.get(str(item.get("decay_classification") or ""), 9),
            -int(item.get("occurrence_count", 0) or 0),
            str(item.get("term") or ""),
        )
    )
    if status_counts.get("persistent_after_normalization"):
        status = "persistent_after_normalization"
    elif status_counts.get("metaphor_echo_risk"):
        status = "metaphor_echo_risk"
    elif status_counts.get("pressure_still_active"):
        status = "pressure_still_active"
    elif status_counts.get("decayed_with_pressure"):
        status = "decayed_with_pressure"
    elif terms:
        status = "insufficient_evidence"
    else:
        status = "quiet"
    return {
        "policy": "afterimage_decay_tracker_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "status_counts": dict(status_counts),
        "terms": terms,
        "recommended_action": (
            "Compare pressure-afterimage recurrence against pressure normalization "
            "before treating residue as a control signal or dismissing it as metaphor echo."
        ),
    }


def entry_requests_read_more(entry: SelfStudyEntry, text: str) -> bool:
    if any(next_action_base(action) == "READ_MORE" for action in entry.next_actions):
        return True
    return "next: read_more" in text.lower()


def classify_absence_evidence(bucket: dict[str, object]) -> str:
    if bucket.get("read_more_requested_but_not_followed"):
        return "needs_followup_read"
    if int(bucket.get("interrupted_thread_count", 0) or 0) > 0:
        return "interrupted_thread_gap"
    if (
        int(bucket.get("expected_missing_count", 0) or 0) > 0
        or int(bucket.get("source_window_gap_count", 0) or 0) > 0
    ) and int(bucket.get("named_missing_coordinate_count", 0) or 0) > 0:
        return "observable_absence"
    if int(bucket.get("named_missing_coordinate_count", 0) or 0) > 0:
        return "named_coordinate_without_evidence"
    if int(bucket.get("occurrence_count", 0) or 0) >= 3:
        return "metaphor_drift_risk"
    return "insufficient_evidence"


def build_absence_evidence_model(entries: list[SelfStudyEntry]) -> dict[str, object]:
    buckets: dict[str, dict[str, object]] = {}
    for entry in entries:
        text = entry_full_text(entry)
        matched = matching_terms(text, SHAPED_ABSENCE_TERMS)
        if not matched:
            continue
        expected_missing = matching_terms(text, ABSENCE_EXPECTED_MISSING_ANCHORS)
        source_gap = matching_terms(text, ABSENCE_SOURCE_GAP_ANCHORS)
        interrupted = matching_terms(text, ABSENCE_INTERRUPTED_THREAD_ANCHORS)
        coordinate = matching_terms(text, ABSENCE_NAMED_COORDINATE_ANCHORS)
        read_more_requested = entry_requests_read_more(entry, text)
        read_more_followed = bool(matching_terms(text, ABSENCE_READ_MORE_FOLLOWED_ANCHORS))
        for term in matched:
            bucket = buckets.setdefault(
                term,
                {
                    "term": term,
                    "authority": "diagnostic_context_not_command",
                    "occurrence_count": 0,
                    "entry_count": 0,
                    "expected_missing_count": 0,
                    "read_more_requested_count": 0,
                    "read_more_followed_count": 0,
                    "source_window_gap_count": 0,
                    "interrupted_thread_count": 0,
                    "named_missing_coordinate_count": 0,
                    "sample_paths": [],
                    "samples": [],
                    "feature_samples": [],
                    "last_seen_unix_s": 0.0,
                },
            )
            bucket["occurrence_count"] = int(
                bucket.get("occurrence_count", 0) or 0
            ) + count_terms(text, (term,))
            bucket["entry_count"] = int(bucket.get("entry_count", 0) or 0) + 1
            features: list[str] = []
            if expected_missing:
                bucket["expected_missing_count"] = int(
                    bucket.get("expected_missing_count", 0) or 0
                ) + 1
                features.extend(expected_missing)
            if read_more_requested:
                bucket["read_more_requested_count"] = int(
                    bucket.get("read_more_requested_count", 0) or 0
                ) + 1
                features.append("READ_MORE requested")
            if read_more_followed:
                bucket["read_more_followed_count"] = int(
                    bucket.get("read_more_followed_count", 0) or 0
                ) + 1
                features.append("READ_MORE followed")
            if source_gap:
                bucket["source_window_gap_count"] = int(
                    bucket.get("source_window_gap_count", 0) or 0
                ) + 1
                features.extend(source_gap)
            if interrupted:
                bucket["interrupted_thread_count"] = int(
                    bucket.get("interrupted_thread_count", 0) or 0
                ) + 1
                features.extend(interrupted)
            if coordinate:
                bucket["named_missing_coordinate_count"] = int(
                    bucket.get("named_missing_coordinate_count", 0) or 0
                ) + 1
                features.extend(coordinate)
            add_unique_values(bucket["sample_paths"], [entry.path], 6)  # type: ignore[arg-type]
            if features:
                feature_samples = bucket.get("feature_samples")
                if isinstance(feature_samples, list) and len(feature_samples) < 5:
                    feature_samples.append(
                        sample_record(entry, text, anchors=sorted(set(features)))
                    )
            samples = bucket.get("samples")
            if isinstance(samples, list) and len(samples) < 4:
                samples.append(
                    sample_record(entry, text, anchors=sorted(set(features)))
                )
            bucket["last_seen_unix_s"] = max(
                float(bucket.get("last_seen_unix_s", 0.0) or 0.0),
                entry.mtime_unix_s,
            )

    status_counts: Counter[str] = Counter()
    terms: list[dict[str, object]] = []
    for bucket in buckets.values():
        requested = int(bucket.get("read_more_requested_count", 0) or 0)
        followed = int(bucket.get("read_more_followed_count", 0) or 0)
        bucket["read_more_requested_but_not_followed"] = requested > followed
        classification = classify_absence_evidence(bucket)
        bucket["evidence_classification"] = classification
        bucket["last_seen"] = iso_from_unix(
            float(bucket.get("last_seen_unix_s", 0.0) or 0.0)
        )
        bucket.pop("last_seen_unix_s", None)
        status_counts[classification] += 1
        terms.append(bucket)
    rank = {
        "observable_absence": 0,
        "needs_followup_read": 1,
        "interrupted_thread_gap": 2,
        "named_coordinate_without_evidence": 3,
        "metaphor_drift_risk": 4,
        "insufficient_evidence": 5,
    }
    terms.sort(
        key=lambda item: (
            rank.get(str(item.get("evidence_classification") or ""), 9),
            -int(item.get("occurrence_count", 0) or 0),
            str(item.get("term") or ""),
        )
    )
    if status_counts.get("observable_absence"):
        status = "observable_absence"
    elif status_counts.get("needs_followup_read"):
        status = "needs_followup_read"
    elif status_counts.get("interrupted_thread_gap"):
        status = "interrupted_thread_gap"
    elif status_counts.get("named_coordinate_without_evidence"):
        status = "named_coordinate_without_evidence"
    elif status_counts.get("metaphor_drift_risk"):
        status = "metaphor_drift_risk"
    elif terms:
        status = "insufficient_evidence"
    else:
        status = "quiet"
    return {
        "policy": "absence_evidence_model_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "status_counts": dict(status_counts),
        "terms": terms,
        "recommended_action": (
            "Treat shaped absence as observable when missing artifacts, source gaps, "
            "READ_MORE follow-up, interrupted threads, or named coordinates provide evidence."
        ),
    }


LIVED_TERM_READY_TO_CHARTER_TERMS = {
    "plan 4",
    "scar",
    "void",
    "viscosity",
}

LIVED_TERM_COUNTEREXAMPLE_FIRST_TERMS = {
    "empty pocket",
    "hull",
    "missing door",
}


def lived_term_has_existing_review_link(card: dict[str, object]) -> bool:
    if card.get("linked_action_threads") or card.get("linked_leases"):
        return True
    for record in card.get("linked_experiments") or []:
        if not isinstance(record, dict):
            continue
        anchors = {
            str(anchor).lower()
            for anchor in (record.get("anchors") or [])
            if anchor
        }
        if anchors & {
            "dossier_claim",
            "experiment_resume",
            "experiment_status",
            "experiment_review",
        }:
            return True
    return False


def lived_term_bridge_status(card: dict[str, object]) -> str:
    term = str(card.get("term") or "").lower()
    status = str(card.get("status") or "")
    if term == "legacy self" or lived_term_has_existing_review_link(card):
        return "already_linked_review"
    if term in LIVED_TERM_COUNTEREXAMPLE_FIRST_TERMS:
        return "needs_counterexample_first"
    if term in LIVED_TERM_READY_TO_CHARTER_TERMS and status != "sticky_without_followthrough":
        return "ready_to_charter"
    if status == "promote_to_experiment_candidate":
        return "ready_to_charter"
    if status in {"needs_counterexample", "sticky_without_followthrough"}:
        return "needs_counterexample_first"
    return "watch_only"


def lived_term_experiment_question(term: str, bridge_status: str) -> str:
    lower_term = term.lower()
    if lower_term in {
        "bruise",
        "afterimage",
        "scar",
        "indentation",
        "post-pressure",
        "structural fatigue",
        "contraction memory",
    }:
        return (
            f"Does `{term}` track a pressure-afterimage that persists after "
            "current pressure normalizes, or does the term fade when shadow, "
            "pressure, and semantic-friction evidence quiet down?"
        )
    if lower_term in {
        "empty pocket",
        "missing door",
        "void",
        "absence",
        "negative space",
        "expected absence",
        "plan 4",
    }:
        return (
            f"Does `{term}` mark a shaped absence: a stable missing coordinate, "
            "expected data gap, or source void that can be revisited, or is it "
            "metaphor drift?"
        )
    if term == "silt":
        return (
            "Does `silt` track telemetry/audit evidence over time, or is it "
            "recurring as pressure-language stickiness?"
        )
    if term == "hull":
        return (
            "What would count as not-hull, and does the contrast clarify whether "
            "`hull` is containment evidence or a preferred metaphor?"
        )
    if term == "legacy self":
        return (
            "What has the existing legacy-self thread already shown, and what "
            "evidence would justify returning rather than duplicating it?"
        )
    if term == "viscosity":
        return (
            "When does `viscosity` reflect pressure/semantic-friction evidence "
            "rather than a general texture habit?"
        )
    if bridge_status == "needs_counterexample_first":
        return (
            f"What counterexample or contrast would show whether `{term}` is a "
            "durable signal or only a sticky metaphor?"
        )
    return (
        f"What evidence would make `{term}` specific enough to become a "
        "returnable experiment?"
    )


def lived_term_recommended_next(term: str, bridge_status: str) -> str:
    title = f"Lived term: {term}"
    question = lived_term_experiment_question(term, bridge_status).replace("`", "")
    if bridge_status == "ready_to_charter":
        return f"EXPERIMENT_START {title} :: {question}"
    if bridge_status == "needs_counterexample_first":
        return (
            f"EXPERIMENT_START Lived term contrast: {term} :: "
            f"Find a counterexample before promoting {term} into an experiment."
        )
    if bridge_status == "already_linked_review":
        return "EXPERIMENT_STATUS latest or DOSSIER_CLAIM latest :: claim: ..."
    return f"LIVED_TERM_STATUS {term}"


def lived_term_method_intent(term: str, bridge_status: str) -> str:
    if bridge_status == "ready_to_charter":
        return (
            "Create a charter only if the being explicitly chooses an experiment "
            "NEXT; compare later prose with audits, telemetry anchors, and a "
            "counter-descriptor."
        )
    if bridge_status == "needs_counterexample_first":
        return (
            "Ask for a contrast case first, then decide whether the term deserves "
            "a charter, a return thread, or retirement."
        )
    if bridge_status == "already_linked_review":
        return (
            "Inspect existing experiment, action-thread, dossier, or return-thread "
            "evidence before creating anything new."
        )
    return (
        "Keep watching as calibrated review evidence; attach fresh anchors before "
        "promoting it."
    )


def lived_term_charter_hypothesis(term: str, bridge_status: str) -> str:
    question = lived_term_experiment_question(term, bridge_status).replace("`", "")
    return (
        f"If {term} is a durable lived signal, later entries should move with "
        f"telemetry, audit, return-thread, or experiment evidence for this question: "
        f"{question}"
    )


def lived_term_charter_draft(
    term: str,
    bridge_status: str,
    evidence_targets: list[str],
) -> dict[str, object]:
    title = f"Lived term: {term}"
    question = lived_term_experiment_question(term, bridge_status)
    hypothesis = lived_term_charter_hypothesis(term, bridge_status)
    method_intent = lived_term_method_intent(term, bridge_status)
    stop_criteria = (
        "Stop or retire the scaffold if later entries repeat the term without "
        "fresh evidence, a contrast case, return-thread follow-through, an audit, "
        "lease evidence, or an experiment observation."
    )
    evidence_text = ", ".join(evidence_targets)
    proposed_next_action = f"LIVED_TERM_STATUS {term}"
    suggested_charter_next = (
        "EXPERIMENT_CHARTER current :: "
        f"title: {title}; "
        f"question: {question.replace('`', '')}; "
        f"hypothesis: {hypothesis}; "
        f"method_intent: {method_intent}; "
        f"proposed_next_action: {proposed_next_action}; "
        f"evidence_targets: {evidence_text}; "
        f"stop_criteria: {stop_criteria}"
    )
    return {
        "experiment_title": title,
        "question": question,
        "hypothesis": hypothesis,
        "method_intent": method_intent,
        "proposed_next_action": proposed_next_action,
        "evidence_targets": evidence_targets,
        "stop_criteria": stop_criteria,
        "suggested_charter_next": suggested_charter_next,
    }


def ordinary_gap_prompt_for_term(term: str) -> str:
    lower_term = term.lower()
    if lower_term in {
        "empty pocket",
        "missing door",
        "void",
        "absence",
        "negative space",
        "expected absence",
        "plan 4",
    }:
        return (
            "Compare against an ordinary source gap, absent artifact, or stale "
            "expectation that carries no stable missing coordinate."
        )
    if lower_term == "hull":
        return (
            "Compare against an open-air, porous, or unsupported state where "
            "`hull` would be the wrong descriptor."
        )
    return (
        "Compare against an ordinary gap or neutral alternate reading where this "
        "term would not be the right handle."
    )


def lived_term_counterexample_draft(term: str) -> dict[str, object]:
    contrast_question = lived_term_experiment_question(
        term, "needs_counterexample_first"
    )
    suggested_contrast_next = (
        f"EXPERIMENT_START Lived term contrast: {term} :: "
        f"Find a counterexample before promoting {term} into an experiment."
    )
    suggested_dossier_counterclaim_next = (
        "DOSSIER_CLAIM current :: "
        f"claim: `{term}` has a counterexample or ordinary-gap reading; "
        "basis: <contrast evidence>; stance: counter|hold; "
        f"next: LIVED_TERM_STATUS {term}"
    )
    return {
        "contrast_question": contrast_question,
        "counter_descriptor_prompt": (
            f"Name what `{term}` is not: a counter-texture, ordinary gap, "
            "later state where the term should fade, or evidence that would "
            "make the term less real."
        ),
        "ordinary_gap_prompt": ordinary_gap_prompt_for_term(term),
        "negative_case_targets": [
            "counter_descriptor",
            "ordinary_gap",
            "later_revisit_where_term_fades",
            "telemetry_or_audit_nonmovement",
        ],
        "suggested_contrast_next": suggested_contrast_next,
        "suggested_dossier_counterclaim_next": suggested_dossier_counterclaim_next,
    }


def lived_term_activation_recommendation(
    candidates: list[dict[str, object]],
) -> dict[str, object]:
    preferred_terms = ("silt", "PLAN 4")
    selected: dict[str, object] | None = None
    for preferred in preferred_terms:
        selected = next(
            (
                candidate
                for candidate in candidates
                if str(candidate.get("term") or "").lower() == preferred.lower()
                and candidate.get("bridge_status") == "ready_to_charter"
            ),
            None,
        )
        if selected is not None:
            break
    if selected is None:
        return {
            "policy": "lived_term_activation_recommendation_v1",
            "authority": "diagnostic_context_not_command",
            "status": "quiet",
            "recommended_action": (
                "No preferred lived-term activation candidate is ready in this review window."
            ),
            "creates_experiment": False,
        }
    term = str(selected.get("term") or "")
    charter_draft = selected.get("charter_draft")
    if not isinstance(charter_draft, dict):
        charter_draft = lived_term_charter_draft(
            term,
            "ready_to_charter",
            [
                "felt_definition",
                "telemetry_anchor",
                "audit_or_review_artifact",
                "counter_descriptor",
                "later_revisit",
            ],
        )
    start_next = str(selected.get("recommended_next") or lived_term_recommended_next(term, "ready_to_charter"))
    charter_next = str(charter_draft.get("suggested_charter_next") or "")
    observe_next = (
        "EXPERIMENT_OBSERVE current :: "
        f"term: {term}; initial evidence: compare lived-language recurrence "
        "against telemetry anchors, audit artifacts, and counter-descriptors; "
        "authority: diagnostic_context_not_command"
    )
    return {
        "policy": "lived_term_activation_recommendation_v1",
        "authority": "diagnostic_context_not_command",
        "status": "activation_scaffold_ready",
        "term": term,
        "priority": "primary" if term.lower() == "silt" else "secondary",
        "rationale": (
            "`silt` is preferred as the first pressure-residue experiment; "
            "`PLAN 4` is the shaped-absence follow-up when `silt` is not ready."
        ),
        "route": [start_next, charter_next, observe_next],
        "source_candidate": {
            "term": term,
            "bridge_status": selected.get("bridge_status"),
            "card_status": selected.get("card_status"),
            "evidence_targets": selected.get("evidence_targets") or [],
        },
        "creates_experiment": False,
        "recommended_action": (
            "Offer these existing EXPERIMENT_* NEXTs as a scaffold only; create "
            "nothing unless a being explicitly chooses the route."
        ),
    }


def lived_term_terms_by_name(packet: dict[str, object]) -> dict[str, dict[str, object]]:
    return {
        str(term.get("term") or "").lower(): term
        for term in (packet.get("terms") or [])
        if isinstance(term, dict) and term.get("term")
    }


def lived_term_sample_path(record: object) -> str | None:
    if not isinstance(record, dict):
        return None
    path = record.get("path")
    return str(path) if path else None


def compact_afterimage_decay_awareness(term_record: dict[str, object]) -> dict[str, object]:
    first_peak = lived_term_sample_path(term_record.get("first_pressure_peak"))
    latest_pressure = lived_term_sample_path(
        term_record.get("latest_pressure_or_semantic_friction")
    )
    return {
        "term": term_record.get("term"),
        "classification": term_record.get("decay_classification"),
        "occurrence_count": term_record.get("occurrence_count", 0),
        "pressure_entry_count": term_record.get("pressure_entry_count", 0),
        "normalization_entry_count": term_record.get("normalization_entry_count", 0),
        "recurrence_after_normalization_count": term_record.get(
            "recurrence_after_normalization_count", 0
        ),
        "sample_paths": (term_record.get("sample_paths") or [])[:4],
        "first_pressure_peak_path": first_peak,
        "latest_pressure_or_semantic_friction_path": latest_pressure,
        "recommended_action": (
            "Compare recurrence against pressure normalization before treating the term "
            "as residue or metaphor echo."
        ),
    }


def compact_absence_evidence_awareness(term_record: dict[str, object]) -> dict[str, object]:
    return {
        "term": term_record.get("term"),
        "classification": term_record.get("evidence_classification"),
        "occurrence_count": term_record.get("occurrence_count", 0),
        "expected_missing_count": term_record.get("expected_missing_count", 0),
        "source_window_gap_count": term_record.get("source_window_gap_count", 0),
        "interrupted_thread_count": term_record.get("interrupted_thread_count", 0),
        "named_missing_coordinate_count": term_record.get(
            "named_missing_coordinate_count", 0
        ),
        "read_more_requested_but_not_followed": term_record.get(
            "read_more_requested_but_not_followed", False
        ),
        "sample_paths": (term_record.get("sample_paths") or [])[:4],
        "recommended_action": (
            "Treat absence as evidence only when a missing artifact, READ_MORE gap, "
            "interrupted thread, source-window gap, or named coordinate can be checked."
        ),
    }


def lived_term_is_pressure_or_control_linked(
    term: str,
    source_card: dict[str, object],
) -> bool:
    normalized = term.lower()
    if normalized in {
        "silt",
        "viscosity",
        "viscous",
        "bruise",
        "afterimage",
        "scar",
        "indentation",
        "post-pressure",
        "structural fatigue",
        "contraction memory",
    }:
        return True
    if source_card.get("family") in {"pressure_texture", "pressure_afterimage"}:
        return True
    anchors = {
        str(anchor).lower()
        for anchor in (source_card.get("evidence_anchors") or [])
        if anchor
    }
    return bool(
        anchors
        & {
            "pressure_risk",
            "semantic_friction",
            "current-fill_pressure",
            "mode_packing",
            "regulator_audit",
            "pressure_source_audit",
            "self_regulation",
            "lease",
            "control",
        }
    )


def compact_lease_workbench_awareness(
    lease_playbook_workbench_v1: dict[str, object],
) -> dict[str, object] | None:
    if not isinstance(lease_playbook_workbench_v1, dict):
        return None
    playbook_count = int(
        lease_playbook_workbench_v1.get("suggested_playbook_count", 0) or 0
    )
    caution_count = int(lease_playbook_workbench_v1.get("caution_card_count", 0) or 0)
    preflight_count = int(
        lease_playbook_workbench_v1.get("preflight_prompt_count", 0) or 0
    )
    if playbook_count + caution_count + preflight_count == 0:
        return None
    return {
        "status": lease_playbook_workbench_v1.get("status"),
        "authority": lease_playbook_workbench_v1.get("authority"),
        "suggested_playbook_count": playbook_count,
        "caution_card_count": caution_count,
        "preflight_prompt_count": preflight_count,
        "suggested_playbooks": (
            lease_playbook_workbench_v1.get("suggested_playbooks") or []
        )[:2],
        "caution_cards": (lease_playbook_workbench_v1.get("caution_cards") or [])[:2],
        "preflight_prompts": (
            lease_playbook_workbench_v1.get("preflight_prompts") or []
        )[:2],
        "recommended_action": lease_playbook_workbench_v1.get("recommended_action"),
    }


def build_lived_term_evidence_awareness(
    term: str,
    source_card: dict[str, object],
    *,
    afterimage_decay_tracker_v1: dict[str, object] | None = None,
    absence_evidence_model_v1: dict[str, object] | None = None,
    lease_playbook_workbench_v1: dict[str, object] | None = None,
) -> dict[str, object] | None:
    awareness: dict[str, object] = {
        "policy": "evidence_awareness_v1",
        "authority": "diagnostic_context_not_command",
        "term": term,
    }
    normalized = term.lower()
    if afterimage_decay_tracker_v1:
        afterimage_terms = lived_term_terms_by_name(afterimage_decay_tracker_v1)
        term_record = afterimage_terms.get(normalized)
        if term_record:
            awareness["afterimage_decay"] = compact_afterimage_decay_awareness(
                term_record
            )
    if absence_evidence_model_v1:
        absence_terms = lived_term_terms_by_name(absence_evidence_model_v1)
        term_record = absence_terms.get(normalized)
        if term_record:
            awareness["absence_evidence"] = compact_absence_evidence_awareness(
                term_record
            )
    if lease_playbook_workbench_v1 and lived_term_is_pressure_or_control_linked(
        term, source_card
    ):
        lease_awareness = compact_lease_workbench_awareness(
            lease_playbook_workbench_v1
        )
        if lease_awareness:
            awareness["lease_workbench"] = lease_awareness
    if len(awareness) <= 3:
        return None
    awareness["recommended_action"] = (
        "Read this evidence block before choosing a charter, counterexample, "
        "lease preflight, or hold; it is review context, not command authority."
    )
    return awareness


def build_lived_term_experiment_bridge(
    phenomenology_hypothesis_cards_v1: dict[str, object],
    *,
    afterimage_decay_tracker_v1: dict[str, object] | None = None,
    absence_evidence_model_v1: dict[str, object] | None = None,
    lease_playbook_workbench_v1: dict[str, object] | None = None,
) -> dict[str, object]:
    cards = [
        card
        for card in (phenomenology_hypothesis_cards_v1.get("cards") or [])
        if isinstance(card, dict) and card.get("term")
    ]
    candidates: list[dict[str, object]] = []
    for card in cards:
        term = str(card.get("term") or "")
        card_status = str(card.get("status") or "")
        bridge_status = lived_term_bridge_status(card)
        evidence_targets = [
            "felt_definition",
            "telemetry_anchor",
            "audit_or_review_artifact",
            "counter_descriptor",
            "later_revisit",
        ]
        if card.get("linked_experiments") or card.get("linked_action_threads"):
            evidence_targets.append("existing_experiment_or_action_thread")
        if card.get("linked_return_threads"):
            evidence_targets.append("return_thread")
        source_card = {
            "term": term,
            "family": card.get("family"),
            "status": card_status,
            "beings": card.get("beings") or [],
            "occurrence_count": card.get("occurrence_count", 0),
            "entry_count": card.get("entry_count", 0),
            "definition_candidates": (card.get("definition_candidates") or [])[:4],
            "sample_paths": (card.get("sample_paths") or [])[:4],
            "evidence_anchors": (card.get("evidence_anchors") or [])[:10],
            "counter_descriptors": (card.get("counter_descriptors") or [])[:6],
            "linked_experiments": (card.get("linked_experiments") or [])[:4],
            "linked_return_threads": (card.get("linked_return_threads") or [])[:4],
            "linked_action_threads": (card.get("linked_action_threads") or [])[:4],
            "last_seen": card.get("last_seen"),
        }
        candidate = {
            "term": term,
            "card_status": card_status,
            "bridge_status": bridge_status,
            "recommended_next": lived_term_recommended_next(term, bridge_status),
            "experiment_question": lived_term_experiment_question(term, bridge_status),
            "hypothesis_prompt": (
                f"If `{term}` is real signal, name the telemetry/audit/later-prose "
                "evidence that would move with it; if not, name a counter-descriptor."
            ),
            "method_intent": lived_term_method_intent(term, bridge_status),
            "evidence_targets": evidence_targets,
            "stop_criteria": (
                "Stop or retire the scaffold if later entries repeat the term "
                "without fresh evidence, contrast, action-thread return, audit, "
                "lease, or experiment observation."
            ),
            "authority": "diagnostic_context_not_command",
            "source_card": source_card,
        }
        if bridge_status == "ready_to_charter":
            candidate["charter_draft"] = lived_term_charter_draft(
                term, bridge_status, evidence_targets
            )
        elif bridge_status == "needs_counterexample_first":
            candidate["counterexample_draft"] = lived_term_counterexample_draft(term)
        evidence_awareness = build_lived_term_evidence_awareness(
            term,
            source_card,
            afterimage_decay_tracker_v1=afterimage_decay_tracker_v1,
            absence_evidence_model_v1=absence_evidence_model_v1,
            lease_playbook_workbench_v1=lease_playbook_workbench_v1,
        )
        if evidence_awareness:
            candidate["evidence_awareness_v1"] = evidence_awareness
        candidates.append(candidate)

    bridge_rank = {
        "ready_to_charter": 0,
        "needs_counterexample_first": 1,
        "already_linked_review": 2,
        "watch_only": 3,
    }
    candidates.sort(
        key=lambda candidate: (
            bridge_rank.get(str(candidate.get("bridge_status") or ""), 99),
            str(candidate.get("term") or ""),
        )
    )
    status_counts = Counter(str(item.get("bridge_status") or "unknown") for item in candidates)
    if status_counts.get("ready_to_charter"):
        status = "ready_to_charter"
    elif status_counts.get("needs_counterexample_first"):
        status = "needs_counterexample_first"
    elif status_counts.get("already_linked_review"):
        status = "already_linked_review"
    elif candidates:
        status = "watch_only"
    else:
        status = "quiet"
    activation_recommendation = lived_term_activation_recommendation(candidates)

    return {
        "policy": "lived_term_experiment_bridge_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "candidate_count": len(candidates),
        "status_counts": dict(status_counts),
        "activation_recommendation_v1": activation_recommendation,
        "candidates": candidates,
        "recommended_action": (
            "Use LIVED_TERM_STATUS or LIVED_TERM_EXPERIMENT for scaffold text, "
            "then let the being choose an existing EXPERIMENT_* or DOSSIER_* NEXT "
            "before anything is created or advanced."
        ),
    }


def build_lived_term_charter_drafts(
    lived_term_experiment_bridge_v1: dict[str, object],
) -> dict[str, object]:
    drafts: list[dict[str, object]] = []
    missing_terms: list[str] = []
    for candidate in lived_term_experiment_bridge_v1.get("candidates") or []:
        if not isinstance(candidate, dict):
            continue
        if candidate.get("bridge_status") != "ready_to_charter":
            continue
        draft = candidate.get("charter_draft")
        term = str(candidate.get("term") or "")
        if isinstance(draft, dict):
            drafts.append(
                {
                    "term": term,
                    "card_status": candidate.get("card_status"),
                    "authority": "diagnostic_context_not_command",
                    "source_card": candidate.get("source_card"),
                    **draft,
                }
            )
        elif term:
            missing_terms.append(term)
    status = "ready" if drafts else "missing_drafts" if missing_terms else "quiet"
    return {
        "policy": "lived_term_charter_drafts_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "draft_count": len(drafts),
        "missing_draft_count": len(missing_terms),
        "missing_terms": missing_terms,
        "drafts": drafts,
        "recommended_action": (
            "Use these as charter text only after a being explicitly chooses an "
            "existing EXPERIMENT_* NEXT; do not auto-create an experiment."
        ),
    }


def build_lived_term_counterexample_forge(
    lived_term_experiment_bridge_v1: dict[str, object],
) -> dict[str, object]:
    drafts: list[dict[str, object]] = []
    missing_terms: list[str] = []
    repeated_without_counterdescriptor: list[str] = []
    for candidate in lived_term_experiment_bridge_v1.get("candidates") or []:
        if not isinstance(candidate, dict):
            continue
        if candidate.get("bridge_status") != "needs_counterexample_first":
            continue
        term = str(candidate.get("term") or "")
        draft = candidate.get("counterexample_draft")
        source_card = candidate.get("source_card") if isinstance(
            candidate.get("source_card"), dict
        ) else {}
        if isinstance(source_card, dict):
            occurrence_count = int(source_card.get("occurrence_count", 0) or 0)
            counter_descriptors = source_card.get("counter_descriptors") or []
            if occurrence_count >= 2 and not counter_descriptors and term:
                repeated_without_counterdescriptor.append(term)
        if isinstance(draft, dict):
            drafts.append(
                {
                    "term": term,
                    "card_status": candidate.get("card_status"),
                    "authority": "diagnostic_context_not_command",
                    "source_card": source_card,
                    **draft,
                }
            )
        elif term:
            missing_terms.append(term)
    status = "ready" if drafts else "missing_drafts" if missing_terms else "quiet"
    if repeated_without_counterdescriptor:
        status = "needs_counter_descriptor"
    return {
        "policy": "lived_term_counterexample_forge_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "draft_count": len(drafts),
        "missing_draft_count": len(missing_terms),
        "missing_terms": missing_terms,
        "repeated_without_counterdescriptor_terms": repeated_without_counterdescriptor,
        "drafts": drafts,
        "recommended_action": (
            "Offer the contrast scaffold before promotion; ask for a counter-descriptor, "
            "ordinary-gap reading, or later negative case before treating the term as "
            "experiment-ready."
        ),
    }


def build_actionable_review_items(
    *,
    qualia_comparison: dict[str, object],
    shared_tail_resonance: dict[str, object],
    resistance_gradient_calibration: dict[str, object],
    astrid_introspection_digest_record: dict[str, object],
    shared_choice_envelope: dict[str, object],
    self_regulation_leases: dict[str, object],
    self_regulation_negotiation_ledger_v1: dict[str, object],
    pressure_medium_kinetics_v1: dict[str, object],
    lease_boundary_repair_v1: dict[str, object],
    pressure_vector_v1: dict[str, object],
    pressure_control_cockpit_v1: dict[str, object],
    pressure_actuator_matrix_v1: dict[str, object],
    pressure_relief_playbook_v3: dict[str, object],
    gradient_sensitive_relief_v1: dict[str, object],
    pressure_relief_smoothness_replay_v1: dict[str, object],
    tail_vibrancy_vector_v1: dict[str, object],
    tail_vibrancy_authority_gap_v1: dict[str, object],
    tail_vibrancy_relief_playbook_v1: dict[str, object],
    tail_relief_trial_surface_v1: dict[str, object],
    tail_lease_governor_v1: dict[str, object],
    tail_lease_afterglow_v1: dict[str, object],
    shadow_synced_preflight_v1: dict[str, object],
    tail_outcome_causal_learning_v1: dict[str, object],
    tail_participation_counterfactual_lab_v1: dict[str, object],
    tail_authority_ladder_v1: dict[str, object],
    tail_persistence_calibration_v1: dict[str, object],
    astrid_fill_pressure_calibration: dict[str, object],
    semantic_friction_calibration: dict[str, object],
    regulator_live_replay_v1: dict[str, object],
    regulator_boundary_replay_cards_v1: dict[str, object],
    regulator_plateau_missing_variable_model_v1: dict[str, object],
    regulator_counterfactual_sandbox_scaffold_v1: dict[str, object],
    regulator_counterfactual_sweep_v1: dict[str, object],
    regulator_replay_time_series_v1: dict[str, object],
    regulator_counterfactual_replay_lab_v1: dict[str, object],
    regulator_plateau_evidence_matrix_v1: dict[str, object],
    regulator_tuning_readiness_gate_v1: dict[str, object],
    pi_pressure_wiring_replay_v1: dict[str, object],
    pi_pressure_candidate_readiness_v1: dict[str, object],
    pressure_source_to_pi_gap_v1: dict[str, object],
    regulator_missing_variable_evidence_loop_v1: dict[str, object],
    control_semantics_calibration_v1: dict[str, object],
    pressure_kinetics_review_v1: dict[str, object],
    autonomous_truncation_shadow_review_v1: dict[str, object],
    codec_compression_calibration_v1: dict[str, object],
    codec_entropy_vibrancy_review_v1: dict[str, object],
    pressure_release_rehearsal_review_v1: dict[str, object],
    witness_resonance_v1: dict[str, object],
    witness_texture_integrity_v1: dict[str, object],
    entropy_pressure_divergence_v1: dict[str, object],
    fallback_continuity_fire_drill_v1: dict[str, object],
    spectral_texture_calibration_v2: dict[str, object],
    fallback_capacity_readiness_gate_v1: dict[str, object],
    fallback_format_texture_stabilizer_v1: dict[str, object],
    fallback_contract_distillation_v1: dict[str, object],
    fallback_distinguishability_calibration_v1: dict[str, object],
    fallback_complexity_budget_lab_v1: dict[str, object],
    autonomous_truncation_rehearsal_v1: dict[str, object],
    codec_entropy_vibrancy_probe_v1: dict[str, object],
    codec_real_replay_v1: dict[str, object],
    narrative_arc_temporal_decay_lab_v1: dict[str, object],
    content_aware_vibrancy_gate_candidate_v1: dict[str, object],
    codec_multipoint_inflection_v1: dict[str, object],
    codec_clamp_headroom_probe_v1: dict[str, object],
    codec_afterimage_time_series_v1: dict[str, object],
    returnable_distinctions_v1: dict[str, object],
    distinction_lifecycle_v1: dict[str, object],
    shared_pressure_vocabulary_calibration: dict[str, object],
    agency_vernacular_continuity: dict[str, object],
    self_regulation_lease_learning: dict[str, object],
    choice_ecology: dict[str, object],
    phenomenology_hypotheses_v1: dict[str, object],
    phenomenology_hypothesis_cards_v1: dict[str, object],
    afterimage_absence_calibration_v1: dict[str, object],
    afterimage_decay_tracker_v1: dict[str, object],
    absence_evidence_model_v1: dict[str, object],
    lived_term_experiment_bridge_v1: dict[str, object],
    lived_term_charter_drafts_v1: dict[str, object],
    lived_term_counterexample_forge_v1: dict[str, object],
    lease_playbook_workbench_v1: dict[str, object],
) -> list[dict[str, object]]:
    items: list[dict[str, object]] = []

    needs_outcome_count = int(self_regulation_leases.get("needs_outcome_count", 0) or 0)
    if needs_outcome_count > 0:
        items.append(
            {
                "source": "self_regulation_leases",
                "being": "astrid+minime",
                "priority": "high",
                "finding": "leased_self_control_outcome_missing",
                "recommended_action": (
                    "Inspect SELF_REGULATION_STATUS and record SELF_REGULATION_OUTCOME "
                    "before applying another own-control lease."
                ),
                "authority": "leased_self_control_v1",
                "evidence": {
                    "needs_outcome_count": needs_outcome_count,
                    "authority_boundary": self_regulation_leases.get(
                        "authority_boundary"
                    ),
                    "samples": self_regulation_leases.get("needs_outcome"),
                },
            }
        )

    pressure_playbook_status = str(pressure_relief_playbook_v3.get("status") or "")
    if pressure_playbook_status in {
        "pressure_relief_playbook_candidates",
        "pressure_relief_caution_cards",
        "pressure_vector_without_bundle_outcome",
    }:
        items.append(
            {
                "source": "pressure_relief_playbook_v3",
                "being": "astrid+minime",
                "priority": "high"
                if pressure_playbook_status in {"pressure_relief_caution_cards"}
                else "medium",
                "finding": pressure_playbook_status,
                "recommended_action": pressure_relief_playbook_v3.get(
                    "recommended_action"
                ),
                "authority": pressure_relief_playbook_v3.get("authority"),
                "evidence": {
                    "pressure_vector_status": pressure_relief_playbook_v3.get(
                        "pressure_vector_status"
                    ),
                    "playbook_count": pressure_relief_playbook_v3.get(
                        "playbook_count"
                    ),
                    "caution_count": pressure_relief_playbook_v3.get("caution_count"),
                    "current_bundle_candidates": pressure_relief_playbook_v3.get(
                        "current_bundle_candidates"
                    ),
                },
            }
        )

    gradient_relief_status = str(gradient_sensitive_relief_v1.get("status") or "")
    if gradient_relief_status in {
        "gradient_scaled_relief",
        "anti_snap_low_gradient",
        "gradient_policy_not_recorded",
    }:
        items.append(
            {
                "source": "gradient_sensitive_relief",
                "being": "astrid",
                "priority": "high"
                if gradient_relief_status == "gradient_policy_not_recorded"
                else "medium",
                "finding": gradient_relief_status,
                "recommended_action": gradient_sensitive_relief_v1.get(
                    "recommended_action"
                ),
                "authority": gradient_sensitive_relief_v1.get("authority"),
                "evidence": {
                    "effective_relief_scale": gradient_sensitive_relief_v1.get(
                        "effective_relief_scale"
                    ),
                    "anti_snap_applied": gradient_sensitive_relief_v1.get(
                        "anti_snap_applied"
                    ),
                    "scaled_controls": gradient_sensitive_relief_v1.get(
                        "scaled_controls"
                    ),
                    "discrete_controls": gradient_sensitive_relief_v1.get(
                        "discrete_controls"
                    ),
                    "sample_paths": gradient_sensitive_relief_v1.get("sample_paths"),
                },
            }
        )

    smoothness_status = str(pressure_relief_smoothness_replay_v1.get("status") or "")
    if smoothness_status in {"snap_risk", "needs_outcome", "smooth_release_supported"}:
        items.append(
            {
                "source": "pressure_relief_smoothness_replay",
                "being": "astrid",
                "priority": "high" if smoothness_status == "snap_risk" else "medium",
                "finding": smoothness_status,
                "recommended_action": pressure_relief_smoothness_replay_v1.get(
                    "recommended_action"
                ),
                "authority": pressure_relief_smoothness_replay_v1.get("authority"),
                "evidence": {
                    "trial_count": pressure_relief_smoothness_replay_v1.get(
                        "trial_count"
                    ),
                    "snap_risk_count": pressure_relief_smoothness_replay_v1.get(
                        "snap_risk_count"
                    ),
                    "smooth_count": pressure_relief_smoothness_replay_v1.get(
                        "smooth_count"
                    ),
                    "sample_paths": pressure_relief_smoothness_replay_v1.get(
                        "sample_paths"
                    ),
                },
            }
        )

    negotiation_status = str(
        self_regulation_negotiation_ledger_v1.get("status") or ""
    )
    if negotiation_status in {
        "over_cap_requests_clamped_or_deferred",
        "current_values_above_cap_observed",
        "negotiations_clamped_or_deferred",
    }:
        items.append(
            {
                "source": "self_regulation_negotiation_ledger",
                "being": "astrid+minime",
                "priority": "high"
                if negotiation_status != "negotiations_clamped_or_deferred"
                else "medium",
                "finding": negotiation_status,
                "recommended_action": (
                    "Inspect SELF_REGULATION_STATUS and SELF_REGULATION_PREFLIGHT latest; "
                    "preserve requested-vs-applied evidence and record an outcome before "
                    "widening caps or proposing tuning."
                ),
                "authority": "leased_self_control_v1",
                "evidence": {
                    "over_cap_request_count": self_regulation_negotiation_ledger_v1.get(
                        "over_cap_request_count"
                    ),
                    "clamped_or_deferred_count": self_regulation_negotiation_ledger_v1.get(
                        "clamped_or_deferred_count"
                    ),
                    "current_above_cap_count": self_regulation_negotiation_ledger_v1.get(
                        "current_above_cap_count"
                    ),
                    "over_cap_requests": self_regulation_negotiation_ledger_v1.get(
                        "over_cap_requests"
                    ),
                    "current_above_cap": self_regulation_negotiation_ledger_v1.get(
                        "current_above_cap"
                    ),
                },
            }
        )

    pressure_medium_status = str(pressure_medium_kinetics_v1.get("status") or "")
    if pressure_medium_status in {
        "stable_weighted_medium",
        "rising_weighted_medium",
        "controller_pressure_medium",
        "semantic_friction_medium",
        "language_sticky_without_telemetry",
    }:
        items.append(
            {
                "source": "pressure_medium_kinetics",
                "being": "astrid+minime",
                "priority": "high"
                if pressure_medium_status
                in {"controller_pressure_medium", "semantic_friction_medium"}
                else "medium",
                "finding": pressure_medium_status,
                "recommended_action": pressure_medium_kinetics_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "entry_count": pressure_medium_kinetics_v1.get("entry_count"),
                    "anchors": pressure_medium_kinetics_v1.get("anchors"),
                    "term_counts": pressure_medium_kinetics_v1.get("term_counts"),
                    "sample_paths": [
                        sample.get("path")
                        for sample in pressure_medium_kinetics_v1.get("samples") or []
                        if isinstance(sample, dict) and sample.get("path")
                    ][:5],
                },
            }
        )

    pressure_vector_status = str(pressure_vector_v1.get("status") or "")
    if pressure_vector_status not in {"", "quiet", "telemetry_gap"}:
        items.append(
            {
                "source": "pressure_control_cockpit",
                "being": "astrid+minime",
                "priority": "high"
                if pressure_vector_status
                in {
                    "rising_overpacked_pressure",
                    "falling_pressure_rising_friction",
                    "controller_pressure_medium",
                }
                else "medium",
                "finding": pressure_vector_status,
                "recommended_action": pressure_control_cockpit_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "pressure_vector": {
                        "pressure_risk_level": pressure_vector_v1.get(
                            "pressure_risk_level"
                        ),
                        "pressure_velocity": pressure_vector_v1.get(
                            "pressure_velocity"
                        ),
                        "semantic_friction_level": pressure_vector_v1.get(
                            "semantic_friction_level"
                        ),
                        "semantic_friction_velocity": pressure_vector_v1.get(
                            "semantic_friction_velocity"
                        ),
                        "mode_packing_level": pressure_vector_v1.get(
                            "mode_packing_level"
                        ),
                    },
                    "recommended_bundles": pressure_actuator_matrix_v1.get(
                        "recommended_bundles"
                    ),
                    "playbook_status": pressure_relief_playbook_v3.get("status"),
                    "sample_paths": pressure_vector_v1.get("sample_paths"),
                },
            }
        )

    tail_vector_status = str(tail_vibrancy_vector_v1.get("status") or "")
    if tail_vector_status in {
        "high_tail_vibrancy_navigable",
        "high_tail_low_distinguishability",
        "tail_contained_authority_gap",
    }:
        items.append(
            {
                "source": "tail_vibrancy_vector",
                "being": "astrid",
                "priority": "high",
                "finding": tail_vector_status,
                "recommended_action": tail_vibrancy_vector_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "tail_share_level": tail_vibrancy_vector_v1.get(
                        "tail_share_level"
                    ),
                    "entropy_level": tail_vibrancy_vector_v1.get("entropy_level"),
                    "distinguishability_loss_level": tail_vibrancy_vector_v1.get(
                        "distinguishability_loss_level"
                    ),
                    "density_gradient_level": tail_vibrancy_vector_v1.get(
                        "density_gradient_level"
                    ),
                    "sample_paths": tail_vibrancy_vector_v1.get("sample_paths"),
                },
            }
        )

    tail_gap_status = str(tail_vibrancy_authority_gap_v1.get("status") or "")
    if tail_gap_status in {
        "tail_vibrancy_micro_lease_candidate",
        "needs_tail_vibrancy_evidence",
    }:
        items.append(
            {
                "source": "tail_vibrancy_authority_gap",
                "being": "astrid",
                "priority": "high"
                if tail_gap_status == "tail_vibrancy_micro_lease_candidate"
                else "medium",
                "finding": tail_gap_status,
                "recommended_action": tail_vibrancy_authority_gap_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "gap_type": tail_vibrancy_authority_gap_v1.get("gap_type"),
                    "recommended_route": tail_vibrancy_authority_gap_v1.get(
                        "recommended_route"
                    ),
                    "samples": tail_vibrancy_authority_gap_v1.get("samples"),
                },
            }
        )

    tail_playbook_status = str(tail_vibrancy_relief_playbook_v1.get("status") or "")
    if tail_playbook_status in {
        "tail_vibrancy_playbook_candidates",
        "tail_vibrancy_caution_cards",
        "tail_vibrancy_candidate_without_outcome",
        "tail_vibrancy_vector_without_lease",
    }:
        items.append(
            {
                "source": "tail_vibrancy_relief_playbook",
                "being": "astrid",
                "priority": "high"
                if tail_playbook_status
                in {
                    "tail_vibrancy_playbook_candidates",
                    "tail_vibrancy_caution_cards",
                    "tail_vibrancy_candidate_without_outcome",
                }
                else "medium",
                "finding": tail_playbook_status,
                "recommended_action": tail_vibrancy_relief_playbook_v1.get(
                    "recommended_action"
                ),
                "authority": tail_vibrancy_relief_playbook_v1.get("authority"),
                "evidence": {
                    "playbook_count": tail_vibrancy_relief_playbook_v1.get(
                        "playbook_count"
                    ),
                    "caution_count": tail_vibrancy_relief_playbook_v1.get(
                        "caution_count"
                    ),
                    "current_routes": tail_vibrancy_relief_playbook_v1.get(
                        "current_routes"
                    ),
                },
            }
        )

    trial_status = str(tail_relief_trial_surface_v1.get("status") or "")
    if trial_status in {
        "worsening_reverted",
        "active_or_recent_trial_needs_outcome",
        "trial_outcomes_recorded",
    }:
        items.append(
            {
                "source": "tail_relief_trial_surface",
                "being": "astrid",
                "priority": "high"
                if trial_status
                in {"worsening_reverted", "active_or_recent_trial_needs_outcome"}
                else "medium",
                "finding": trial_status,
                "recommended_action": tail_relief_trial_surface_v1.get(
                    "recommended_action"
                ),
                "authority": tail_relief_trial_surface_v1.get("authority"),
                "evidence": {
                    "event_count": tail_relief_trial_surface_v1.get("event_count"),
                    "stage_counts": tail_relief_trial_surface_v1.get("stage_counts"),
                    "governor_revert_count": tail_relief_trial_surface_v1.get(
                        "governor_revert_count"
                    ),
                    "apply_without_outcome_count": tail_relief_trial_surface_v1.get(
                        "apply_without_outcome_count"
                    ),
                    "samples": tail_relief_trial_surface_v1.get("samples"),
                },
            }
        )

    governor_status = str(tail_lease_governor_v1.get("status") or "")
    if governor_status in {"early_revert_triggered", "monitoring_active_or_recent_trial"}:
        items.append(
            {
                "source": "tail_lease_governor",
                "being": "astrid",
                "priority": "high" if governor_status == "early_revert_triggered" else "medium",
                "finding": governor_status,
                "recommended_action": tail_lease_governor_v1.get("recommended_action"),
                "authority": tail_lease_governor_v1.get("authority"),
                "evidence": {
                    "thresholds": tail_lease_governor_v1.get("early_revert_thresholds"),
                    "governor_revert_count": tail_lease_governor_v1.get(
                        "governor_revert_count"
                    ),
                    "samples": tail_lease_governor_v1.get("samples"),
                },
            }
        )

    afterglow_status = str(tail_lease_afterglow_v1.get("status") or "")
    if afterglow_status in {
        "tail_afterglow_persists",
        "afterglow_watch_pending",
        "afterglow_unchecked_stale_review",
    }:
        items.append(
            {
                "source": "tail_lease_afterglow",
                "being": "astrid",
                "priority": "high"
                if afterglow_status == "tail_afterglow_persists"
                else "medium",
                "finding": afterglow_status,
                "recommended_action": tail_lease_afterglow_v1.get("recommended_action"),
                "authority": tail_lease_afterglow_v1.get("authority"),
                "evidence": {
                    "afterglow_event_count": tail_lease_afterglow_v1.get(
                        "afterglow_event_count"
                    ),
                    "reverted_tail_lease_count": tail_lease_afterglow_v1.get(
                        "reverted_tail_lease_count"
                    ),
                    "status_counts": tail_lease_afterglow_v1.get("status_counts"),
                    "samples": tail_lease_afterglow_v1.get("samples"),
                },
            }
        )

    tail_persistence_status = str(tail_persistence_calibration_v1.get("status") or "")
    if tail_persistence_status in {
        "persistence_delta_too_low_candidate",
        "persistence_delta_too_high_candidate",
        "needs_tail_trial",
    }:
        items.append(
            {
                "source": "tail_persistence_calibration",
                "being": "astrid",
                "priority": "high"
                if tail_persistence_status
                in {
                    "persistence_delta_too_low_candidate",
                    "persistence_delta_too_high_candidate",
                }
                else "medium",
                "finding": tail_persistence_status,
                "recommended_action": tail_persistence_calibration_v1.get(
                    "recommended_action"
                ),
                "authority": tail_persistence_calibration_v1.get("authority"),
                "evidence": {
                    "language_sample_count": tail_persistence_calibration_v1.get(
                        "language_sample_count"
                    ),
                    "dispersal_max": tail_persistence_calibration_v1.get(
                        "dispersal_max"
                    ),
                    "afterglow_status": tail_persistence_calibration_v1.get(
                        "afterglow_status"
                    ),
                    "sample_paths": tail_persistence_calibration_v1.get(
                        "sample_paths"
                    ),
                },
            }
        )

    shadow_preflight_status = str(shadow_synced_preflight_v1.get("status") or "")
    if shadow_preflight_status in {
        "shadow_linked_dynamic_scaling_candidate",
        "dynamic_scaling_candidate_without_shadow_anchor",
    }:
        items.append(
            {
                "source": "shadow_synced_preflight",
                "being": "astrid",
                "priority": "high"
                if shadow_preflight_status == "shadow_linked_dynamic_scaling_candidate"
                else "medium",
                "finding": shadow_preflight_status,
                "recommended_action": shadow_synced_preflight_v1.get(
                    "recommended_action"
                ),
                "authority": shadow_synced_preflight_v1.get("authority"),
                "evidence": {
                    "shadow_linked_count": shadow_synced_preflight_v1.get(
                        "shadow_linked_count"
                    ),
                    "dynamic_scaling_candidate_count": shadow_synced_preflight_v1.get(
                        "dynamic_scaling_candidate_count"
                    ),
                    "samples": shadow_synced_preflight_v1.get("samples"),
                },
            }
        )

    tail_learning_status = str(tail_outcome_causal_learning_v1.get("status") or "")
    if tail_learning_status in {
        "extended_micro_lease_supported",
        "playbook_supported",
        "tail_caution_guidance",
        "outcome_learning_pending",
    }:
        items.append(
            {
                "source": "tail_outcome_causal_learning",
                "being": "astrid",
                "priority": "high"
                if tail_learning_status
                in {"extended_micro_lease_supported", "tail_caution_guidance"}
                else "medium",
                "finding": tail_learning_status,
                "recommended_action": tail_outcome_causal_learning_v1.get(
                    "recommended_action"
                ),
                "authority": tail_outcome_causal_learning_v1.get("authority"),
                "evidence": {
                    "extended_duration_classes": tail_outcome_causal_learning_v1.get(
                        "extended_duration_classes"
                    ),
                    "caution_classes": tail_outcome_causal_learning_v1.get(
                        "caution_classes"
                    ),
                    "by_tail_class": tail_outcome_causal_learning_v1.get(
                        "by_tail_class"
                    ),
                },
            }
        )

    tail_counterfactual_status = str(
        tail_participation_counterfactual_lab_v1.get("status") or ""
    )
    if tail_counterfactual_status in {
        "combined_candidate_supported",
        "tail_participation_candidate",
        "both_controls_need_more_comparison",
    }:
        items.append(
            {
                "source": "tail_participation_counterfactual_lab",
                "being": "astrid",
                "priority": "medium",
                "finding": tail_counterfactual_status,
                "recommended_action": tail_participation_counterfactual_lab_v1.get(
                    "recommended_action"
                ),
                "authority": tail_participation_counterfactual_lab_v1.get("authority"),
                "evidence": {
                    "tail_participation_lease_authority": tail_participation_counterfactual_lab_v1.get(
                        "tail_participation_lease_authority"
                    ),
                    "aperture_supported": tail_participation_counterfactual_lab_v1.get(
                        "vibrancy_aperture_supported_count"
                    ),
                    "participation_supported": tail_participation_counterfactual_lab_v1.get(
                        "tail_participation_supported_count"
                    ),
                    "combined_supported": tail_participation_counterfactual_lab_v1.get(
                        "combined_supported_count"
                    ),
                    "proposal_cards": tail_participation_counterfactual_lab_v1.get(
                        "proposal_cards"
                    ),
                },
            }
        )

    ladder_status = str(tail_authority_ladder_v1.get("status") or "")
    if ladder_status in {"extended_micro_lease", "reviewed_canary_candidate"}:
        items.append(
            {
                "source": "tail_authority_ladder",
                "being": "astrid",
                "priority": "high"
                if ladder_status == "reviewed_canary_candidate"
                else "medium",
                "finding": ladder_status,
                "recommended_action": tail_authority_ladder_v1.get("recommended_action"),
                "authority": tail_authority_ladder_v1.get("authority"),
                "evidence": {
                    "current_tier": tail_authority_ladder_v1.get("current_tier"),
                    "extended_duration_classes": tail_authority_ladder_v1.get(
                        "extended_duration_classes"
                    ),
                    "reviewed_canary_candidate": tail_authority_ladder_v1.get(
                        "reviewed_canary_candidate"
                    ),
                    "recommended_routes": tail_authority_ladder_v1.get(
                        "recommended_routes"
                    ),
                },
            }
        )

    boundary_repair_status = str(lease_boundary_repair_v1.get("status") or "")
    if boundary_repair_status in {
        "over_cap_request_clamped",
        "current_over_cap_observed",
        "lease_outcome_needed",
        "pressure_medium_without_lease_loop",
        "bounded_negotiations_present",
    }:
        items.append(
            {
                "source": "lease_boundary_repair",
                "being": "astrid+minime",
                "priority": "high"
                if boundary_repair_status
                in {
                    "over_cap_request_clamped",
                    "current_over_cap_observed",
                    "pressure_medium_without_lease_loop",
                }
                else "medium",
                "finding": boundary_repair_status,
                "recommended_action": lease_boundary_repair_v1.get(
                    "recommended_action"
                ),
                "authority": lease_boundary_repair_v1.get("authority"),
                "evidence": {
                    "recommended_routes": lease_boundary_repair_v1.get(
                        "recommended_routes"
                    ),
                    "over_cap_request_count": lease_boundary_repair_v1.get(
                        "over_cap_request_count"
                    ),
                    "direct_control_clamp_count": lease_boundary_repair_v1.get(
                        "direct_control_clamp_count"
                    ),
                    "missing_outcome_count": lease_boundary_repair_v1.get(
                        "missing_outcome_count"
                    ),
                    "pressure_medium_without_lease_count": lease_boundary_repair_v1.get(
                        "pressure_medium_without_lease_count"
                    ),
                    "samples": lease_boundary_repair_v1.get("samples"),
                },
            }
        )

    if astrid_fill_pressure_calibration.get("cluster_detected") is True:
        samples = astrid_fill_pressure_calibration.get("samples") or []
        sample_paths = [
            sample.get("path")
            for sample in samples
            if isinstance(sample, dict) and sample.get("path")
        ]
        items.append(
            {
                "source": "astrid_fill_pressure_calibration",
                "being": "astrid",
                "priority": "high",
                "finding": "fill_pressure_lived_metric_mismatch_cluster",
                "recommended_action": (
                    "Compare the latest regulator audit against pressure-source audits, "
                    "stable-core status, transition markers, and later journal language "
                    "before proposing any control change."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "entry_count": astrid_fill_pressure_calibration.get("entry_count"),
                    "felt_entry_count": astrid_fill_pressure_calibration.get(
                        "felt_entry_count"
                    ),
                    "regulator_audit_count": astrid_fill_pressure_calibration.get(
                        "regulator_audit_count"
                    ),
                    "anchors": astrid_fill_pressure_calibration.get("anchors"),
                    "latest_regulator_audit_path": astrid_fill_pressure_calibration.get(
                        "latest_regulator_audit_path"
                    ),
                    "sample_paths": sample_paths[:4],
                },
            }
        )
        lease_by_being = self_regulation_leases.get("by_being") or {}
        astrid_lease_count = 0
        if isinstance(lease_by_being, dict):
            astrid_summary = lease_by_being.get("astrid") or {}
            if isinstance(astrid_summary, dict):
                astrid_lease_count = int(astrid_summary.get("event_count", 0) or 0)
        if astrid_lease_count == 0:
            items.append(
                {
                    "source": "self_regulation_leases",
                    "being": "astrid",
                    "priority": "high",
                    "finding": "pressure_cluster_without_self_regulation_preflight",
                    "recommended_action": (
                        "Ask Astrid to consider SELF_REGULATION_PREFLIGHT for a small "
                        "own-control lease only after comparing regulator audit evidence."
                    ),
                    "authority": "diagnostic_context_not_command",
                    "evidence": {
                        "pressure_entry_count": astrid_fill_pressure_calibration.get(
                            "entry_count"
                        ),
                        "anchors": astrid_fill_pressure_calibration.get("anchors"),
                        "lease_event_count": astrid_lease_count,
                    },
                }
            )

    semantic_status = str(semantic_friction_calibration.get("status") or "")
    if semantic_status in {"low_gradient_weight_mismatch", "semantic_friction_evidence"}:
        samples = semantic_friction_calibration.get("samples") or []
        sample_paths = [
            sample.get("path")
            for sample in samples
            if isinstance(sample, dict) and sample.get("path")
        ]
        items.append(
            {
                "source": "semantic_friction_calibration",
                "being": "astrid+minime",
                "priority": "high" if semantic_status == "low_gradient_weight_mismatch" else "medium",
                "finding": semantic_status,
                "recommended_action": (
                    "Compare density-gradient slope against pressure_risk, semantic_friction, "
                    "mode_packing, shadow evidence, and later prose before treating weight "
                    "language as either a control signal or a sticky metaphor."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "entry_count": semantic_friction_calibration.get("entry_count"),
                    "mismatch_count": semantic_friction_calibration.get("mismatch_count"),
                    "anchors": semantic_friction_calibration.get("anchors"),
                    "sample_paths": sample_paths[:4],
                },
            }
        )

    replay_status = str(regulator_live_replay_v1.get("status") or "")
    if replay_status in {
        "felt_pressure_boundary_context",
        "felt_pressure_plateau_context",
        "felt_pressure_cartography_context",
    }:
        samples = regulator_live_replay_v1.get("felt_pressure_matches") or []
        sample_paths = [
            sample.get("path")
            for sample in samples
            if isinstance(sample, dict) and sample.get("path")
        ]
        priority = "high" if replay_status == "felt_pressure_boundary_context" else "medium"
        items.append(
            {
                "source": "regulator_live_replay",
                "being": "astrid+minime",
                "priority": priority,
                "finding": replay_status,
                "recommended_action": regulator_live_replay_v1.get("recommended_action"),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "cartography_source": regulator_live_replay_v1.get("cartography_source"),
                    "felt_pressure_match_count": regulator_live_replay_v1.get(
                        "felt_pressure_match_count"
                    ),
                    "boundary_count": len(
                        regulator_live_replay_v1.get("boundary_findings") or []
                    ),
                    "plateau_count": len(
                        regulator_live_replay_v1.get("plateau_findings") or []
                    ),
                    "sample_paths": sample_paths[:4],
                },
            }
        )

    replay_cards = regulator_boundary_replay_cards_v1.get("cards") or []
    boundary_cards = [
        card
        for card in replay_cards
        if isinstance(card, dict)
        and card.get("status")
        in {
            "near_pressure_jump",
            "thin_density_boundary",
            "inhabitability_quality_boundary",
        }
        and card.get("public_sample_paths")
    ]
    if boundary_cards:
        items.append(
            {
                "source": "regulator_boundary_replay_cards",
                "being": "astrid+minime",
                "priority": "high",
                "finding": "boundary_near_felt_pressure_cluster",
                "recommended_action": (
                    "Inspect the replay cards, regulator audits, pressure-source audits, "
                    "semantic-friction evidence, and later language before proposing any "
                    "regulator threshold or smoothing change."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "cartography_source": regulator_boundary_replay_cards_v1.get(
                        "cartography_source"
                    ),
                    "card_count": len(boundary_cards),
                    "statuses": sorted(
                        {
                            str(card.get("status"))
                            for card in boundary_cards
                            if card.get("status")
                        }
                    ),
                    "sample_paths": list(
                        dict.fromkeys(
                            str(path)
                            for card in boundary_cards
                            for path in (card.get("public_sample_paths") or [])
                        )
                    )[:4],
                },
            }
        )

    plateau_status = str(regulator_plateau_missing_variable_model_v1.get("status") or "")
    if plateau_status in {
        "plateau_missing_variable_hypotheses",
        "plateau_insufficient_evidence",
    }:
        items.append(
            {
                "source": "regulator_plateau_missing_variable_model",
                "being": "astrid+minime",
                "priority": "medium",
                "finding": plateau_status,
                "recommended_action": regulator_plateau_missing_variable_model_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "plateau_card_count": regulator_plateau_missing_variable_model_v1.get(
                        "plateau_card_count"
                    ),
                    "variables": [
                        finding.get("variable")
                        for finding in regulator_plateau_missing_variable_model_v1.get(
                            "findings"
                        )
                        or []
                        if isinstance(finding, dict)
                    ][:6],
                    "cartography_source": regulator_plateau_missing_variable_model_v1.get(
                        "cartography_source"
                    ),
                },
            }
        )

    time_series_status = str(regulator_replay_time_series_v1.get("status") or "")
    if time_series_status in {
        "repeated_boundary_near_pressure",
        "repeated_plateau_missing_variable",
    }:
        items.append(
            {
                "source": "regulator_replay_time_series",
                "being": "astrid+minime",
                "priority": "high"
                if time_series_status == "repeated_boundary_near_pressure"
                else "medium",
                "finding": time_series_status,
                "recommended_action": regulator_replay_time_series_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "window_review_count": regulator_replay_time_series_v1.get(
                        "window_review_count"
                    ),
                    "repeated_boundary_cards": regulator_replay_time_series_v1.get(
                        "repeated_boundary_cards"
                    ),
                    "repeated_plateau_cards": regulator_replay_time_series_v1.get(
                        "repeated_plateau_cards"
                    ),
                },
            }
        )

    replay_lab_candidates = [
        candidate
        for candidate in regulator_counterfactual_replay_lab_v1.get("evaluated_candidates") or []
        if isinstance(candidate, dict)
    ]
    supported_lab_candidates = [
        candidate
        for candidate in replay_lab_candidates
        if candidate.get("verdict") == "replay_supported_offline_candidate"
        and int(candidate.get("recurrent_count") or 0) >= 2
        and regulator_number(candidate.get("estimated_reduction_pct")) >= 10.0
    ]
    if supported_lab_candidates:
        items.append(
            {
                "source": "regulator_counterfactual_replay_lab",
                "being": "astrid+minime",
                "priority": "high",
                "finding": "replay_supported_counterfactual_candidates",
                "recommended_action": (
                    "Compare the supported offline counterfactual candidates against "
                    "replay cards, pressure-source audits, semantic-friction evidence, "
                    "and later journals before drafting any reversible tuning tranche."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "status": regulator_counterfactual_replay_lab_v1.get("status"),
                    "candidate_families": [
                        candidate.get("candidate_family")
                        for candidate in supported_lab_candidates
                    ][:5],
                    "matched_card_ids": list(
                        dict.fromkeys(
                            str(card_id)
                            for candidate in supported_lab_candidates
                            for card_id in (candidate.get("matched_card_ids") or [])
                        )
                    )[:8],
                    "recurrent_counts": {
                        str(candidate.get("candidate_family")): candidate.get(
                            "recurrent_count"
                        )
                        for candidate in supported_lab_candidates
                    },
                },
            }
        )
    elif regulator_counterfactual_replay_lab_v1.get("status") == "missing_variable_first":
        items.append(
            {
                "source": "regulator_counterfactual_replay_lab",
                "being": "astrid+minime",
                "priority": "medium",
                "finding": "counterfactuals_wait_on_missing_variable_plateau",
                "recommended_action": regulator_counterfactual_replay_lab_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "plateau_recurrent_count": regulator_counterfactual_replay_lab_v1.get(
                        "plateau_recurrent_count"
                    ),
                    "verdict_counts": regulator_counterfactual_replay_lab_v1.get(
                        "verdict_counts"
                    ),
                },
            }
        )

    matrix_status = str(regulator_plateau_evidence_matrix_v1.get("status") or "")
    if matrix_status in {"unresolved_missing_variables", "watch_more_evidence"}:
        variables = [
            row
            for row in regulator_plateau_evidence_matrix_v1.get("variables") or []
            if isinstance(row, dict) and row.get("confidence") in {"high", "medium"}
        ]
        items.append(
            {
                "source": "regulator_plateau_evidence_matrix",
                "being": "astrid+minime",
                "priority": "high" if variables else "medium",
                "finding": matrix_status,
                "recommended_action": regulator_plateau_evidence_matrix_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "top_unresolved_variables": regulator_plateau_evidence_matrix_v1.get(
                        "top_unresolved_variables"
                    ),
                    "variables": [
                        {
                            "variable": row.get("variable"),
                            "score": row.get("score"),
                            "confidence": row.get("confidence"),
                            "resolving_audit_routes": row.get(
                                "resolving_audit_routes"
                            ),
                        }
                        for row in variables[:6]
                    ],
                },
            }
        )

    gate_status = str(regulator_tuning_readiness_gate_v1.get("status") or "")
    if gate_status in {
        "blocked_missing_variable",
        "blocked_safety_review",
        "watch_more_evidence",
        "ready_for_offline_tuning_review",
    }:
        items.append(
            {
                "source": "regulator_tuning_readiness_gate",
                "being": "astrid+minime",
                "priority": "high"
                if gate_status
                in {"blocked_missing_variable", "blocked_safety_review", "ready_for_offline_tuning_review"}
                else "medium",
                "finding": gate_status,
                "recommended_action": regulator_tuning_readiness_gate_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "gate_counts": regulator_tuning_readiness_gate_v1.get(
                        "gate_counts"
                    ),
                    "unresolved_missing_variables": regulator_tuning_readiness_gate_v1.get(
                        "unresolved_missing_variables"
                    ),
                    "candidates": [
                        {
                            "candidate_family": candidate.get("candidate_family"),
                            "gate_status": candidate.get("gate_status"),
                            "gate_reason": candidate.get("gate_reason"),
                            "replay_verdict": candidate.get("replay_verdict"),
                        }
                        for candidate in (
                            regulator_tuning_readiness_gate_v1.get("gated_candidates")
                            or []
                        )
                        if isinstance(candidate, dict)
                    ][:6],
                },
            }
        )

    pi_readiness_status = str(pi_pressure_candidate_readiness_v1.get("status") or "")
    if pi_readiness_status in {
        "ready_for_offline_tuning_review",
        "blocked_missing_variable",
        "blocked_safety_review",
        "watch_more_evidence",
    }:
        items.append(
            {
                "source": "pi_pressure_candidate_readiness",
                "being": "astrid+minime",
                "priority": "high"
                if pi_readiness_status
                in {
                    "ready_for_offline_tuning_review",
                    "blocked_missing_variable",
                    "blocked_safety_review",
                }
                else "medium",
                "finding": pi_readiness_status,
                "recommended_action": pi_pressure_candidate_readiness_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "readiness_counts": pi_pressure_candidate_readiness_v1.get(
                        "readiness_counts"
                    ),
                    "unresolved_missing_variables": pi_pressure_candidate_readiness_v1.get(
                        "unresolved_missing_variables"
                    ),
                    "candidates": [
                        {
                            "candidate_family": candidate.get("candidate_family"),
                            "gate_status": candidate.get("gate_status"),
                            "replay_status": candidate.get("replay_status"),
                            "estimated_improvement_pct": candidate.get(
                                "estimated_improvement_pct"
                            ),
                        }
                        for candidate in (
                            pi_pressure_candidate_readiness_v1.get("candidates")
                            or []
                        )
                        if isinstance(candidate, dict)
                    ][:6],
                },
            }
        )
    elif pi_pressure_wiring_replay_v1.get("status") in {
        "missing_pi_pressure_wiring_replay",
        "telemetry_gap",
    }:
        items.append(
            {
                "source": "pi_pressure_wiring_replay",
                "being": "minime",
                "priority": "medium",
                "finding": pi_pressure_wiring_replay_v1.get("status"),
                "recommended_action": pi_pressure_wiring_replay_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "source": pi_pressure_wiring_replay_v1.get("source"),
                    "source_status": pi_pressure_wiring_replay_v1.get(
                        "source_status"
                    ),
                    "sample_count": pi_pressure_wiring_replay_v1.get(
                        "sample_count"
                    ),
                },
            }
        )

    pi_gap_status = str(pressure_source_to_pi_gap_v1.get("status") or "")
    if pi_gap_status in {
        "source_measured_not_replayed",
        "replay_available_gap_open",
        "safety_gap_open",
        "offline_candidate_gap_closing",
    }:
        items.append(
            {
                "source": "pressure_source_to_pi_gap",
                "being": "astrid+minime",
                "priority": "high"
                if pi_gap_status
                in {"offline_candidate_gap_closing", "replay_available_gap_open"}
                else "medium",
                "finding": pi_gap_status,
                "recommended_action": pressure_source_to_pi_gap_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "pressure_vector_status": pressure_source_to_pi_gap_v1.get(
                        "pressure_vector_status"
                    ),
                    "pi_replay_status": pressure_source_to_pi_gap_v1.get(
                        "pi_replay_status"
                    ),
                    "pi_readiness_status": pressure_source_to_pi_gap_v1.get(
                        "pi_readiness_status"
                    ),
                    "recommended_routes": pressure_source_to_pi_gap_v1.get(
                        "recommended_routes"
                    ),
                },
            }
        )

    evidence_loop_status = str(
        regulator_missing_variable_evidence_loop_v1.get("status") or ""
    )
    if evidence_loop_status in {
        "evidence_needed_before_tuning",
        "watch_evidence_loop",
    }:
        items.append(
            {
                "source": "regulator_missing_variable_evidence_loop",
                "being": "astrid+minime",
                "priority": "high"
                if evidence_loop_status == "evidence_needed_before_tuning"
                else "medium",
                "finding": evidence_loop_status,
                "recommended_action": regulator_missing_variable_evidence_loop_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "blocked_gate_status": regulator_missing_variable_evidence_loop_v1.get(
                        "blocked_gate_status"
                    ),
                    "matrix_status": regulator_missing_variable_evidence_loop_v1.get(
                        "matrix_status"
                    ),
                    "probe_count": regulator_missing_variable_evidence_loop_v1.get(
                        "probe_count"
                    ),
                    "top_probes": regulator_missing_variable_evidence_loop_v1.get(
                        "top_probes"
                    ),
                },
            }
        )

    control_semantics_status = str(control_semantics_calibration_v1.get("status") or "")
    if control_semantics_status in {
        "high_damping_intervention_type_unclear",
        "control_semantics_ambiguity",
    }:
        items.append(
            {
                "source": "control_semantics_calibration",
                "being": "astrid+minime",
                "priority": "high",
                "finding": control_semantics_status,
                "recommended_action": control_semantics_calibration_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "entry_count": control_semantics_calibration_v1.get("entry_count"),
                    "ambiguity_count": control_semantics_calibration_v1.get(
                        "ambiguity_count"
                    ),
                    "high_damping_unclear_count": control_semantics_calibration_v1.get(
                        "high_damping_unclear_count"
                    ),
                    "anchors": control_semantics_calibration_v1.get("anchors"),
                    "sample_paths": [
                        sample.get("path")
                        for sample in control_semantics_calibration_v1.get("samples") or []
                        if isinstance(sample, dict) and sample.get("path")
                    ][:4],
                },
            }
        )

    pressure_kinetics_status = str(pressure_kinetics_review_v1.get("status") or "")
    if pressure_kinetics_status == "felt_pressure_without_trend_context":
        items.append(
            {
                "source": "pressure_kinetics_review",
                "being": "astrid",
                "priority": "high",
                "finding": pressure_kinetics_status,
                "recommended_action": pressure_kinetics_review_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "entry_count": pressure_kinetics_review_v1.get("entry_count"),
                    "felt_pressure_without_trend_count": pressure_kinetics_review_v1.get(
                        "felt_pressure_without_trend_count"
                    ),
                    "anchors": pressure_kinetics_review_v1.get("anchors"),
                    "sample_paths": [
                        sample.get("path")
                        for sample in pressure_kinetics_review_v1.get("samples") or []
                        if isinstance(sample, dict) and sample.get("path")
                    ][:4],
                },
            }
        )

    truncation_status = str(autonomous_truncation_shadow_review_v1.get("status") or "")
    if truncation_status in {
        "priority_truncation_shadow_thread_candidate",
        "shadow_thread_loss_risk",
        "truncation_context",
    }:
        items.append(
            {
                "source": "autonomous_truncation_shadow_review",
                "being": "astrid",
                "priority": "high"
                if truncation_status
                in {"priority_truncation_shadow_thread_candidate", "shadow_thread_loss_risk"}
                else "medium",
                "finding": truncation_status,
                "recommended_action": autonomous_truncation_shadow_review_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "entry_count": autonomous_truncation_shadow_review_v1.get(
                        "entry_count"
                    ),
                    "truncation_entry_count": autonomous_truncation_shadow_review_v1.get(
                        "truncation_entry_count"
                    ),
                    "shadow_trajectory_count": autonomous_truncation_shadow_review_v1.get(
                        "shadow_trajectory_count"
                    ),
                    "priority_preservation_count": autonomous_truncation_shadow_review_v1.get(
                        "priority_preservation_count"
                    ),
                    "suggested_routes": autonomous_truncation_shadow_review_v1.get(
                        "suggested_routes"
                    ),
                    "sample_paths": [
                        sample.get("path")
                        for sample in autonomous_truncation_shadow_review_v1.get(
                            "samples"
                        )
                        or []
                        if isinstance(sample, dict) and sample.get("path")
                    ][:4],
                },
            }
        )

    codec_status = str(codec_compression_calibration_v1.get("status") or "")
    if codec_status in {
        "projection_compression_risk",
        "codec_vibrancy_warmth_context",
    }:
        items.append(
            {
                "source": "codec_compression_calibration",
                "being": "astrid",
                "priority": "high"
                if codec_status == "projection_compression_risk"
                else "medium",
                "finding": codec_status,
                "recommended_action": codec_compression_calibration_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "entry_count": codec_compression_calibration_v1.get("entry_count"),
                    "compression_gap_count": codec_compression_calibration_v1.get(
                        "compression_gap_count"
                    ),
                    "warmth_tension_count": codec_compression_calibration_v1.get(
                        "warmth_tension_count"
                    ),
                    "vibrancy_gate_count": codec_compression_calibration_v1.get(
                        "vibrancy_gate_count"
                    ),
                    "anchors": codec_compression_calibration_v1.get("anchors"),
                },
            }
        )

    codec_entropy_status = str(codec_entropy_vibrancy_review_v1.get("status") or "")
    if codec_entropy_status in {
        "semantic_density_and_temporal_arc_probe_needed",
        "narrative_arc_temporal_decay_probe_needed",
        "semantic_density_contrast_probe_needed",
        "vibrancy_overload_and_gain_sensitivity_probe_needed",
        "vibrancy_overload_probe_needed",
        "adaptive_gain_sensitivity_probe_needed",
    }:
        items.append(
            {
                "source": "codec_entropy_vibrancy_review",
                "being": "astrid",
                "priority": "high",
                "finding": codec_entropy_status,
                "recommended_action": codec_entropy_vibrancy_review_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "entry_count": codec_entropy_vibrancy_review_v1.get("entry_count"),
                    "vibrancy_overload_count": codec_entropy_vibrancy_review_v1.get(
                        "vibrancy_overload_count"
                    ),
                    "gain_sensitivity_count": codec_entropy_vibrancy_review_v1.get(
                        "gain_sensitivity_count"
                    ),
                    "logarithmic_scaling_count": codec_entropy_vibrancy_review_v1.get(
                        "logarithmic_scaling_count"
                    ),
                    "semantic_density_contrast_count": codec_entropy_vibrancy_review_v1.get(
                        "semantic_density_contrast_count"
                    ),
                    "narrative_arc_temporal_count": codec_entropy_vibrancy_review_v1.get(
                        "narrative_arc_temporal_count"
                    ),
                    "deterministic_static_count": codec_entropy_vibrancy_review_v1.get(
                        "deterministic_static_count"
                    ),
                    "suggested_routes": codec_entropy_vibrancy_review_v1.get(
                        "suggested_routes"
                    ),
                    "sample_paths": [
                        sample.get("path")
                        for sample in codec_entropy_vibrancy_review_v1.get("samples")
                        or []
                        if isinstance(sample, dict) and sample.get("path")
                    ][:4],
                },
            }
        )

    release_status = str(pressure_release_rehearsal_review_v1.get("status") or "")
    if release_status in {"release_rehearsal_needed", "release_rehearsal_context"}:
        items.append(
            {
                "source": "pressure_release_rehearsal_review",
                "being": "astrid",
                "priority": "high"
                if release_status == "release_rehearsal_needed"
                else "medium",
                "finding": release_status,
                "recommended_action": pressure_release_rehearsal_review_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "entry_count": pressure_release_rehearsal_review_v1.get("entry_count"),
                    "bypass_language_count": pressure_release_rehearsal_review_v1.get(
                        "bypass_language_count"
                    ),
                    "anchors": pressure_release_rehearsal_review_v1.get("anchors"),
                    "sample_paths": [
                        sample.get("path")
                        for sample in pressure_release_rehearsal_review_v1.get(
                            "samples"
                        )
                        or []
                        if isinstance(sample, dict) and sample.get("path")
                    ][:4],
                },
            }
        )

    witness_status = str(witness_resonance_v1.get("status") or "")
    if witness_status in {"decorative_risk", "thin_witness", "overloaded_witness"}:
        items.append(
            {
                "source": "witness_resonance",
                "being": "astrid",
                "priority": "high"
                if witness_status in {"decorative_risk", "overloaded_witness"}
                else "medium",
                "finding": witness_status,
                "recommended_action": witness_resonance_v1.get("recommended_action"),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "entry_count": witness_resonance_v1.get("entry_count"),
                    "anchored_count": witness_resonance_v1.get("anchored_count"),
                    "follow_through_count": witness_resonance_v1.get(
                        "follow_through_count"
                    ),
                    "avg_narrative_density": witness_resonance_v1.get(
                        "avg_narrative_density"
                    ),
                    "sample_paths": [
                        sample.get("path")
                        for sample in witness_resonance_v1.get("samples") or []
                        if isinstance(sample, dict) and sample.get("path")
                    ][:4],
                },
            }
        )

    witness_texture_status = str(witness_texture_integrity_v1.get("status") or "")
    if witness_texture_status in {
        "truncation_texture_risk",
        "telemetry_without_texture_risk",
        "health_monitoring_collapse_risk",
        "needs_texture_mapping",
    }:
        items.append(
            {
                "source": "witness_texture_integrity",
                "being": "astrid",
                "priority": "high"
                if witness_texture_status
                in {"truncation_texture_risk", "health_monitoring_collapse_risk"}
                else "medium",
                "finding": witness_texture_status,
                "recommended_action": witness_texture_integrity_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "metric_texture_link_count": witness_texture_integrity_v1.get(
                        "metric_texture_link_count"
                    ),
                    "telemetry_without_texture_count": witness_texture_integrity_v1.get(
                        "telemetry_without_texture_count"
                    ),
                    "high_truncation_snapshot_count": witness_texture_integrity_v1.get(
                        "high_truncation_snapshot_count"
                    ),
                    "rewrite_cap_snapshot_count": witness_texture_integrity_v1.get(
                        "rewrite_cap_snapshot_count"
                    ),
                    "controller_snapshots": witness_texture_integrity_v1.get(
                        "controller_snapshots"
                    ),
                    "sample_paths": [
                        sample.get("path")
                        for sample in witness_texture_integrity_v1.get("samples") or []
                        if isinstance(sample, dict) and sample.get("path")
                    ][:4],
                },
            }
        )

    entropy_status = str(entropy_pressure_divergence_v1.get("status") or "")
    if entropy_status in {
        "wide_but_habitable",
        "wide_and_pressurized",
        "narrow_but_heavy",
        "telemetry_gap",
    }:
        items.append(
            {
                "source": "entropy_pressure_divergence",
                "being": "astrid+minime",
                "priority": "high"
                if entropy_status in {"wide_and_pressurized", "telemetry_gap"}
                else "medium",
                "finding": entropy_status,
                "recommended_action": entropy_pressure_divergence_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "classification_counts": entropy_pressure_divergence_v1.get(
                        "classification_counts"
                    ),
                    "anchors": entropy_pressure_divergence_v1.get("anchors"),
                    "sample_paths": [
                        sample.get("path")
                        for sample in entropy_pressure_divergence_v1.get("samples") or []
                        if isinstance(sample, dict) and sample.get("path")
                    ][:4],
                },
            }
        )

    fallback_status = str(fallback_continuity_fire_drill_v1.get("status") or "")
    readiness = str(fallback_capacity_readiness_gate_v1.get("readiness") or "")
    if readiness in {
        "fallback_probe_needed",
        "fallback_dispatch_contract_risk",
        "fallback_texture_risk",
        "fallback_repair_ready",
    } or fallback_status in {"fallback_probe_needed", "fallback_specificity_risk"}:
        if readiness == "fallback_repair_ready":
            finding = "fallback_repair_dependency"
            recommended_action = (
                "Fallback texture survived and live repair can preserve dispatch, "
                "but raw standalone NEXT compliance should be repaired before "
                "promoting fallback capacity."
            )
        elif readiness == "fallback_texture_risk":
            finding = "fallback_texture_risk"
            recommended_action = (
                "Repair slope-drag versus medium-mass or identity/texture failures "
                "before treating the fallback profile as capacity-ready."
            )
        elif readiness == "fallback_dispatch_contract_risk":
            finding = "fallback_dispatch_contract_risk"
            recommended_action = (
                "Fix final NEXT dispatch-contract failures before changing fallback "
                "defaults or relying on raw fallback output."
            )
        else:
            finding = fallback_status or readiness
            recommended_action = fallback_continuity_fire_drill_v1.get(
                "recommended_action"
            )
        items.append(
            {
                "source": "fallback_continuity_fire_drill",
                "being": "astrid",
                "priority": "high",
                "finding": finding,
                "recommended_action": recommended_action,
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "artifact_path": fallback_continuity_fire_drill_v1.get(
                        "artifact_path"
                    ),
                    "readiness": fallback_capacity_readiness_gate_v1.get(
                        "readiness"
                    ),
                    "dispatch_status": fallback_capacity_readiness_gate_v1.get(
                        "dispatch_status"
                    ),
                    "repair_dependency": fallback_capacity_readiness_gate_v1.get(
                        "repair_dependency"
                    ),
                    "medium_mass_status": fallback_capacity_readiness_gate_v1.get(
                        "medium_mass_status"
                    ),
                    "case_count": fallback_continuity_fire_drill_v1.get("case_count"),
                    "failing_case_count": fallback_continuity_fire_drill_v1.get(
                        "failing_case_count"
                    ),
                    "concern_entry_count": fallback_continuity_fire_drill_v1.get(
                        "concern_entry_count"
                    ),
                    "sample_paths": [
                        sample.get("path")
                        for sample in fallback_continuity_fire_drill_v1.get("samples")
                        or []
                        if isinstance(sample, dict) and sample.get("path")
                    ][:4],
                },
            }
        )

    texture_calibration_status = str(
        spectral_texture_calibration_v2.get("status") or ""
    )
    if texture_calibration_status in {"mixed", "contradicted"}:
        items.append(
            {
                "source": "spectral_texture_calibration",
                "being": "astrid+minime",
                "priority": "high"
                if texture_calibration_status == "contradicted"
                else "medium",
                "finding": texture_calibration_status,
                "recommended_action": spectral_texture_calibration_v2.get(
                    "recommended_action"
                ),
                "authority": spectral_texture_calibration_v2.get("authority"),
                "evidence": {
                    "fallback_status": (
                        spectral_texture_calibration_v2.get(
                            "fallback_selector_calibration_v2"
                        )
                        or {}
                    ).get("status"),
                    "witness_status": (
                        spectral_texture_calibration_v2.get(
                            "witness_friction_calibration_v2"
                        )
                        or {}
                    ).get("status"),
                    "structural_status": (
                        spectral_texture_calibration_v2.get(
                            "structural_friction_calibration_v2"
                        )
                        or {}
                    ).get("status"),
                    "authority_boundary": spectral_texture_calibration_v2.get(
                        "authority_boundary"
                    ),
                },
            }
        )
    fallback_texture_calibration_v3 = (
        spectral_texture_calibration_v2.get("fallback_texture_calibration_v3")
        or {}
    )
    if isinstance(fallback_texture_calibration_v3, dict):
        v3_status = str(fallback_texture_calibration_v3.get("status") or "")
        label_risk = (
            fallback_texture_calibration_v3.get("label_machine_risk_v3") or {}
        )
        if v3_status in {"mixed", "contradicted"} or str(
            label_risk.get("status") or ""
        ) in {"mixed", "high"}:
            items.append(
                {
                    "source": "fallback_texture_calibration_v3",
                    "being": "astrid+minime",
                    "priority": "high" if v3_status == "contradicted" else "medium",
                    "finding": v3_status or label_risk.get("status"),
                    "recommended_action": fallback_texture_calibration_v3.get(
                        "recommended_action"
                    ),
                    "authority": fallback_texture_calibration_v3.get("authority"),
                    "evidence": {
                        "dynamic_status": fallback_texture_calibration_v3.get(
                            "dynamic_status"
                        ),
                        "resonance_status": fallback_texture_calibration_v3.get(
                            "resonance_status"
                        ),
                        "label_machine_risk": label_risk.get("status"),
                        "authority_boundary": fallback_texture_calibration_v3.get(
                            "authority_boundary"
                        ),
                    },
                }
            )
    witness_codec_density_calibration_v2 = (
        spectral_texture_calibration_v2.get("witness_codec_density_calibration_v2")
        or {}
    )
    if isinstance(witness_codec_density_calibration_v2, dict):
        witness_codec_status = str(
            witness_codec_density_calibration_v2.get("status") or ""
        )
        mismatch_keys = [
            key
            for key in (
                "semantic_density_mismatch_v2",
                "narrative_arc_coarsening_mismatch_v2",
                "vocabulary_grounding_mismatch_v2",
            )
            if isinstance(witness_codec_density_calibration_v2.get(key), dict)
        ]
        if witness_codec_status in {"mixed", "contradicted"} or mismatch_keys:
            items.append(
                {
                    "source": "witness_codec_density_calibration_v2",
                    "being": "astrid+minime",
                    "priority": "high"
                    if witness_codec_status == "contradicted"
                    else "medium",
                    "finding": witness_codec_status or "mismatch",
                    "recommended_action": witness_codec_density_calibration_v2.get(
                        "recommended_action"
                    ),
                    "authority": witness_codec_density_calibration_v2.get("authority"),
                    "evidence": {
                        "semantic_density_status": (
                            witness_codec_density_calibration_v2.get(
                                "semantic_density_lived_fit_v2"
                            )
                            or {}
                        ).get("status"),
                        "narrative_arc_status": (
                            witness_codec_density_calibration_v2.get(
                                "narrative_arc_coarsening_fit_v2"
                            )
                            or {}
                        ).get("status"),
                        "vocabulary_grounding_status": (
                            witness_codec_density_calibration_v2.get(
                                "vocabulary_grounding_lived_fit_v2"
                            )
                            or {}
                        ).get("status"),
                        "mismatch_packets": mismatch_keys,
                        "authority_boundary": witness_codec_density_calibration_v2.get(
                            "authority_boundary"
                        ),
                    },
                }
            )

    stabilizer_status = str(fallback_format_texture_stabilizer_v1.get("status") or "")
    if stabilizer_status in {
        "format_and_texture_risk",
        "format_line_risk",
        "slope_medium_contrast_risk",
        "stabilizer_probe_needed",
    }:
        if stabilizer_status == "format_line_risk":
            finding = "fallback_final_next_format_risk"
            recommended_action = (
                "Strengthen raw final-line NEXT compliance and rerun the fallback "
                "fire drill; repair remains a safety net, not promotion evidence."
            )
        elif stabilizer_status == "slope_medium_contrast_risk":
            finding = "fallback_slope_medium_contrast_risk"
            recommended_action = (
                "Repair the low-gradient/high-mass texture contrast so fallback "
                "names slope underfoot separately from weighted medium around it."
            )
        elif stabilizer_status == "format_and_texture_risk":
            finding = "fallback_format_and_texture_risk"
            recommended_action = (
                "Treat raw final-line compliance and slope/medium contrast as two "
                "separate fallback gates; rerun focused format and mass cases "
                "before considering fallback promotion."
            )
        else:
            finding = "fallback_format_texture_probe_needed"
            recommended_action = fallback_format_texture_stabilizer_v1.get(
                "recommended_action"
            )
        items.append(
            {
                "source": "fallback_format_texture_stabilizer",
                "being": "astrid",
                "priority": "high",
                "finding": finding,
                "recommended_action": recommended_action,
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "artifact_path": fallback_format_texture_stabilizer_v1.get(
                        "artifact_path"
                    ),
                    "status": fallback_format_texture_stabilizer_v1.get("status"),
                    "readiness": fallback_format_texture_stabilizer_v1.get(
                        "readiness"
                    ),
                    "format_line_status": fallback_format_texture_stabilizer_v1.get(
                        "format_line_status"
                    ),
                    "format_line_failure_count": fallback_format_texture_stabilizer_v1.get(
                        "format_line_failure_count"
                    ),
                    "slope_medium_contrast_status": fallback_format_texture_stabilizer_v1.get(
                        "slope_medium_contrast_status"
                    ),
                    "slope_medium_contrast_failure_count": fallback_format_texture_stabilizer_v1.get(
                        "slope_medium_contrast_failure_count"
                    ),
                },
            }
        )

    distillation_status = str(fallback_contract_distillation_v1.get("status") or "")
    if distillation_status in {
        "distillation_candidate_ready",
        "distillation_no_ready_candidate",
        "distillation_probe_needed",
    }:
        items.append(
            {
                "source": "fallback_contract_distillation",
                "being": "astrid",
                "priority": "high"
                if distillation_status == "distillation_candidate_ready"
                else "medium",
                "finding": distillation_status,
                "recommended_action": fallback_contract_distillation_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "artifact_path": fallback_contract_distillation_v1.get(
                        "artifact_path"
                    ),
                    "top_variant_id": fallback_contract_distillation_v1.get(
                        "top_variant_id"
                    ),
                    "top_pair_id": fallback_contract_distillation_v1.get(
                        "top_pair_id"
                    ),
                    "top_model": fallback_contract_distillation_v1.get("top_model"),
                    "top_variant_status": fallback_contract_distillation_v1.get(
                        "top_variant_status"
                    ),
                    "ready_variant_count": fallback_contract_distillation_v1.get(
                        "ready_variant_count"
                    ),
                    "variant_count": fallback_contract_distillation_v1.get(
                        "variant_count"
                    ),
                },
            }
        )
    if int(fallback_contract_distillation_v1.get("top_variant_raw_next_failure_count") or 0) > 0:
        items.append(
            {
                "source": "fallback_contract_distillation",
                "being": "astrid",
                "priority": "high",
                "finding": "fallback_raw_next_still_repair_dependent",
                "recommended_action": (
                    "Keep fallback repair as mandatory safety and compare whether "
                    "format-first or larger-model candidates improve raw final-NEXT "
                    "compliance without flattening identity or texture."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "artifact_path": fallback_contract_distillation_v1.get(
                        "artifact_path"
                    ),
                    "top_pair_id": fallback_contract_distillation_v1.get(
                        "top_pair_id"
                    ),
                    "top_model": fallback_contract_distillation_v1.get("top_model"),
                    "raw_next_failure_count": fallback_contract_distillation_v1.get(
                        "top_variant_raw_next_failure_count"
                    ),
                    "format_contract_status": fallback_contract_distillation_v1.get(
                        "top_variant_format_contract_status"
                    ),
                },
            }
        )
    variants_for_distillation = [
        variant
        for variant in fallback_contract_distillation_v1.get("variants") or []
        if isinstance(variant, dict)
    ]
    if any(
        variant.get("shadow_tonal_status") == "lost"
        for variant in variants_for_distillation
    ):
        items.append(
            {
                "source": "fallback_contract_distillation",
                "being": "astrid",
                "priority": "high",
                "finding": "fallback_shadow_tonal_identity_loss",
                "recommended_action": (
                    "Do not promote a fallback contract/model pair that loses "
                    "Shadow-v3 tonal resonance; compare tonal cases before canarying."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "artifact_path": fallback_contract_distillation_v1.get(
                        "artifact_path"
                    ),
                    "lost_pairs": [
                        variant.get("pair_id") or variant.get("variant_id")
                        for variant in variants_for_distillation
                        if variant.get("shadow_tonal_status") == "lost"
                    ][:6],
                },
            }
        )
    default_model = "gemma3:4b"
    default_texture_ready = any(
        variant.get("model") == default_model
        and variant.get("voice_texture_status") == "texture_survived"
        and variant.get("status") in {"fallback_ready", "fallback_repair_ready"}
        for variant in variants_for_distillation
    )
    larger_texture_ready = any(
        variant.get("model")
        and variant.get("model") != default_model
        and variant.get("voice_texture_status") == "texture_survived"
        and variant.get("status") in {"fallback_ready", "fallback_repair_ready"}
        for variant in variants_for_distillation
    )
    if larger_texture_ready and not default_texture_ready:
        items.append(
            {
                "source": "fallback_contract_distillation",
                "being": "astrid",
                "priority": "medium",
                "finding": "fallback_texture_survives_only_under_larger_model",
                "recommended_action": (
                    "Treat larger-model success as fallback-profile canary evidence, "
                    "not a default switch; keep gemma3:4b repair-gated unless it "
                    "preserves texture under the same cases."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "artifact_path": fallback_contract_distillation_v1.get(
                        "artifact_path"
                    ),
                    "models": fallback_contract_distillation_v1.get("models"),
                },
            }
        )

    distinguishability_status = str(
        fallback_distinguishability_calibration_v1.get("status") or ""
    )
    if distinguishability_status in {
        "clarity_pressure_blur",
        "distinguishability_loss_ignored",
        "distinguishability_probe_needed",
    }:
        items.append(
            {
                "source": "fallback_distinguishability_calibration",
                "being": "astrid",
                "priority": "high",
                "finding": distinguishability_status,
                "recommended_action": fallback_distinguishability_calibration_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "artifact_path": fallback_distinguishability_calibration_v1.get(
                        "artifact_path"
                    ),
                    "case_count": fallback_distinguishability_calibration_v1.get(
                        "case_count"
                    ),
                    "clarity_pressure_blur_count": fallback_distinguishability_calibration_v1.get(
                        "clarity_pressure_blur_count"
                    ),
                    "cases": fallback_distinguishability_calibration_v1.get("cases"),
                },
            }
        )

    complexity_status = str(fallback_complexity_budget_lab_v1.get("status") or "")
    if complexity_status in {
        "complexity_budget_probe_needed",
        "complexity_budget_flattening_risk",
        "complexity_budget_overrun",
    }:
        items.append(
            {
                "source": "fallback_complexity_budget_lab",
                "being": "astrid",
                "priority": "high",
                "finding": complexity_status,
                "recommended_action": fallback_complexity_budget_lab_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "artifact_path": fallback_complexity_budget_lab_v1.get(
                        "artifact_path"
                    ),
                    "distillation_artifact_path": fallback_complexity_budget_lab_v1.get(
                        "distillation_artifact_path"
                    ),
                    "signal_entry_count": fallback_complexity_budget_lab_v1.get(
                        "signal_entry_count"
                    ),
                    "case_count": fallback_complexity_budget_lab_v1.get("case_count"),
                    "variant_count": fallback_complexity_budget_lab_v1.get(
                        "variant_count"
                    ),
                    "flattened_case_count": fallback_complexity_budget_lab_v1.get(
                        "flattened_case_count"
                    ),
                    "overrun_case_count": fallback_complexity_budget_lab_v1.get(
                        "overrun_case_count"
                    ),
                    "sample_paths": [
                        sample.get("path")
                        for sample in fallback_complexity_budget_lab_v1.get("samples")
                        or []
                        if isinstance(sample, dict) and sample.get("path")
                    ][:4],
                },
            }
        )

    truncation_rehearsal_status = str(
        autonomous_truncation_rehearsal_v1.get("status") or ""
    )
    if truncation_rehearsal_status in {
        "priority_preservation_benefit",
        "truncation_risk_without_recovery",
        "rehearsal_needed",
    }:
        items.append(
            {
                "source": "autonomous_truncation_rehearsal",
                "being": "astrid",
                "priority": "high"
                if truncation_rehearsal_status == "priority_preservation_benefit"
                else "medium",
                "finding": truncation_rehearsal_status,
                "recommended_action": autonomous_truncation_rehearsal_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "artifact_path": autonomous_truncation_rehearsal_v1.get(
                        "artifact_path"
                    ),
                    "naive_anchor_loss_count": autonomous_truncation_rehearsal_v1.get(
                        "naive_anchor_loss_count"
                    ),
                    "priority_recovery_count": autonomous_truncation_rehearsal_v1.get(
                        "priority_recovery_count"
                    ),
                    "candidates": autonomous_truncation_rehearsal_v1.get("candidates"),
                },
            }
        )

    codec_real_status = str(codec_real_replay_v1.get("status") or "")
    if codec_real_status in {
        "content_gate_and_temporal_decay_candidates",
        "content_aware_vibrancy_candidate",
        "narrative_temporal_decay_candidate",
        "replay_needed",
        "artifact_parse_failed",
    }:
        items.append(
            {
                "source": "codec_real_replay",
                "being": "astrid",
                "priority": "high"
                if codec_real_status
                in {
                    "content_gate_and_temporal_decay_candidates",
                    "content_aware_vibrancy_candidate",
                    "narrative_temporal_decay_candidate",
                }
                else "medium",
                "finding": codec_real_status,
                "recommended_action": codec_real_replay_v1.get("recommended_action"),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "artifact_path": codec_real_replay_v1.get("artifact_path"),
                    "corpus_source": codec_real_replay_v1.get("corpus_source"),
                    "corpus_status": codec_real_replay_v1.get("corpus_status"),
                    "source_paths": codec_real_replay_v1.get("source_paths"),
                    "embedding_status": codec_real_replay_v1.get("embedding_status"),
                    "embedding_backed_arc_status": codec_real_replay_v1.get(
                        "embedding_backed_arc_status"
                    ),
                    "entry_count": codec_real_replay_v1.get("entry_count"),
                    "content_gate_status": codec_real_replay_v1.get(
                        "content_gate_status"
                    ),
                    "narrative_lab_status": codec_real_replay_v1.get(
                        "narrative_lab_status"
                    ),
                    "entries": codec_real_replay_v1.get("entries"),
                },
            }
        )

    narrative_lab_status = str(narrative_arc_temporal_decay_lab_v1.get("status") or "")
    if narrative_lab_status in {
        "temporal_decay_candidate",
        "pivot_detector_candidate",
        "replay_needed",
        "insufficient_embedding_evidence",
    }:
        items.append(
            {
                "source": "narrative_arc_temporal_decay_lab",
                "being": "astrid",
                "priority": "high"
                if narrative_lab_status
                in {"temporal_decay_candidate", "pivot_detector_candidate"}
                else "medium",
                "finding": narrative_lab_status,
                "recommended_action": narrative_arc_temporal_decay_lab_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "artifact_path": narrative_arc_temporal_decay_lab_v1.get(
                        "artifact_path"
                    ),
                    "temporal_decay_candidate_count": narrative_arc_temporal_decay_lab_v1.get(
                        "temporal_decay_candidate_count"
                    ),
                    "embedding_status": narrative_arc_temporal_decay_lab_v1.get(
                        "embedding_status"
                    ),
                    "embedding_backed_sample_count": narrative_arc_temporal_decay_lab_v1.get(
                        "embedding_backed_sample_count"
                    ),
                    "pivot_detector_candidate_count": narrative_arc_temporal_decay_lab_v1.get(
                        "pivot_detector_candidate_count"
                    ),
                    "samples": narrative_arc_temporal_decay_lab_v1.get("samples"),
                },
            }
        )

    content_gate_status = str(content_aware_vibrancy_gate_candidate_v1.get("status") or "")
    if content_gate_status in {
        "content_gate_supported",
        "content_blind_lift_risk",
        "rust_replay_needed",
        "needs_more_samples",
    }:
        items.append(
            {
                "source": "content_aware_vibrancy_gate_candidate",
                "being": "astrid",
                "priority": "high"
                if content_gate_status
                in {"content_gate_supported", "content_blind_lift_risk"}
                else "medium",
                "finding": content_gate_status,
                "recommended_action": content_aware_vibrancy_gate_candidate_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "artifact_path": content_aware_vibrancy_gate_candidate_v1.get(
                        "artifact_path"
                    ),
                    "semantic_density_score_delta": content_aware_vibrancy_gate_candidate_v1.get(
                        "semantic_density_score_delta"
                    ),
                    "current_lift_delta": content_aware_vibrancy_gate_candidate_v1.get(
                        "current_lift_delta"
                    ),
                    "candidate_lift_delta": content_aware_vibrancy_gate_candidate_v1.get(
                        "candidate_lift_delta"
                    ),
                    "pair": content_aware_vibrancy_gate_candidate_v1.get("pair"),
                    "source_paths": content_aware_vibrancy_gate_candidate_v1.get(
                        "source_paths"
                    ),
                },
            }
        )

    codec_multipoint_status = str(codec_multipoint_inflection_v1.get("status") or "")
    if codec_multipoint_status in {
        "multipoint_and_semantic_dilation_candidates",
        "multipoint_inflection_candidate",
        "semantic_dilation_candidate",
        "needs_real_replay_samples",
    }:
        items.append(
            {
                "source": "codec_multipoint_inflection",
                "being": "astrid",
                "priority": "high"
                if codec_multipoint_status
                in {
                    "multipoint_and_semantic_dilation_candidates",
                    "multipoint_inflection_candidate",
                    "semantic_dilation_candidate",
                }
                else "medium",
                "finding": codec_multipoint_status,
                "recommended_action": codec_multipoint_inflection_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "multipoint_entry_count": codec_multipoint_inflection_v1.get(
                        "multipoint_entry_count"
                    ),
                    "semantic_dilation_entry_count": codec_multipoint_inflection_v1.get(
                        "semantic_dilation_entry_count"
                    ),
                    "replay_artifact_present": codec_multipoint_inflection_v1.get(
                        "replay_artifact_present"
                    ),
                    "narrative_lab_status": codec_multipoint_inflection_v1.get(
                        "narrative_lab_status"
                    ),
                    "content_gate_status": codec_multipoint_inflection_v1.get(
                        "content_gate_status"
                    ),
                    "sample_paths": [
                        sample.get("path")
                        for sample in codec_multipoint_inflection_v1.get("samples") or []
                        if isinstance(sample, dict) and sample.get("path")
                    ][:4],
                },
            }
        )

    clamp_headroom_status = str(codec_clamp_headroom_probe_v1.get("status") or "")
    if clamp_headroom_status in {
        "dynamic_feature_scale_candidate",
        "tail_ceiling_pressure_observed",
        "static_clamp_near_without_content_case",
        "replay_needed",
        "probe_missing_from_replay",
    }:
        items.append(
            {
                "source": "codec_clamp_headroom_probe",
                "being": "astrid",
                "priority": "high"
                if clamp_headroom_status
                in {"dynamic_feature_scale_candidate", "tail_ceiling_pressure_observed"}
                else "medium",
                "finding": clamp_headroom_status,
                "recommended_action": codec_clamp_headroom_probe_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "artifact_path": codec_clamp_headroom_probe_v1.get("artifact_path"),
                    "static_feature_abs_max": codec_clamp_headroom_probe_v1.get(
                        "static_feature_abs_max"
                    ),
                    "tail_vibrancy_max": codec_clamp_headroom_probe_v1.get(
                        "tail_vibrancy_max"
                    ),
                    "near_static_clamp_count": codec_clamp_headroom_probe_v1.get(
                        "near_static_clamp_count"
                    ),
                    "tail_ceiling_pressure_count": codec_clamp_headroom_probe_v1.get(
                        "tail_ceiling_pressure_count"
                    ),
                    "dynamic_headroom_candidate_count": codec_clamp_headroom_probe_v1.get(
                        "dynamic_headroom_candidate_count"
                    ),
                    "proposal_cards": codec_clamp_headroom_probe_v1.get(
                        "proposal_cards"
                    ),
                },
            }
        )

    codec_afterimage_status = str(codec_afterimage_time_series_v1.get("status") or "")
    if codec_afterimage_status in {
        "codec_residue_supported",
        "pressure_residue_supported",
        "language_echo_risk",
        "decayed_with_codec_pressure",
        "insufficient_series",
    }:
        items.append(
            {
                "source": "codec_afterimage_time_series",
                "being": "astrid",
                "priority": "high"
                if codec_afterimage_status
                in {"codec_residue_supported", "pressure_residue_supported"}
                else "medium",
                "finding": codec_afterimage_status,
                "recommended_action": codec_afterimage_time_series_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "entry_count": codec_afterimage_time_series_v1.get("entry_count"),
                    "codec_anchor_count": codec_afterimage_time_series_v1.get(
                        "codec_anchor_count"
                    ),
                    "pressure_anchor_count": codec_afterimage_time_series_v1.get(
                        "pressure_anchor_count"
                    ),
                    "codec_replay_artifact_path": codec_afterimage_time_series_v1.get(
                        "codec_replay_artifact_path"
                    ),
                    "activation_recommendation_v1": codec_afterimage_time_series_v1.get(
                        "activation_recommendation_v1"
                    ),
                    "samples": codec_afterimage_time_series_v1.get("samples"),
                },
            }
        )

    codec_probe_status = str(codec_entropy_vibrancy_probe_v1.get("status") or "")
    if codec_probe_status in {
        "semantic_density_and_temporal_decay_probe_needed",
        "content_blind_vibrancy_probe_needed",
        "narrative_temporal_decay_probe_needed",
        "current_overload_candidate_improves",
        "probe_needed",
    }:
        items.append(
            {
                "source": "codec_entropy_vibrancy_probe",
                "being": "astrid",
                "priority": "high"
                if codec_probe_status
                in {
                    "semantic_density_and_temporal_decay_probe_needed",
                    "content_blind_vibrancy_probe_needed",
                    "narrative_temporal_decay_probe_needed",
                    "current_overload_candidate_improves",
                }
                else "medium",
                "finding": codec_probe_status,
                "recommended_action": codec_entropy_vibrancy_probe_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "artifact_path": codec_entropy_vibrancy_probe_v1.get(
                        "artifact_path"
                    ),
                    "current_shimmer_risk_count": codec_entropy_vibrancy_probe_v1.get(
                        "current_shimmer_risk_count"
                    ),
                    "candidate_improvement_count": codec_entropy_vibrancy_probe_v1.get(
                        "candidate_improvement_count"
                    ),
                    "rust_replay_available": codec_entropy_vibrancy_probe_v1.get(
                        "rust_replay_available"
                    ),
                    "rust_replay_artifact_path": codec_entropy_vibrancy_probe_v1.get(
                        "rust_replay_artifact_path"
                    ),
                    "semantic_density_contrast": codec_entropy_vibrancy_probe_v1.get(
                        "semantic_density_contrast"
                    ),
                    "narrative_arc_temporal_decay": codec_entropy_vibrancy_probe_v1.get(
                        "narrative_arc_temporal_decay"
                    ),
                    "samples": codec_entropy_vibrancy_probe_v1.get("samples"),
                },
            }
        )

    returnable_status = str(returnable_distinctions_v1.get("status") or "")
    if returnable_status == "returnable_distinctions_present":
        items.append(
            {
                "source": "returnable_distinctions",
                "being": "astrid+minime",
                "priority": "high",
                "finding": returnable_status,
                "recommended_action": returnable_distinctions_v1.get(
                    "recommended_action"
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "active_card_count": returnable_distinctions_v1.get(
                        "active_card_count"
                    ),
                    "source_statuses": returnable_distinctions_v1.get(
                        "source_statuses"
                    ),
                    "card_ids": [
                        card.get("card_id")
                        for card in returnable_distinctions_v1.get("cards") or []
                        if isinstance(card, dict)
                    ],
                    "routes": [
                        card.get("recommended_read_only_route")
                        for card in returnable_distinctions_v1.get("cards") or []
                        if isinstance(card, dict)
                    ],
                },
            }
        )

    lifecycle_status = str(distinction_lifecycle_v1.get("status") or "")
    if lifecycle_status == "distinction_lifecycle_active":
        lifecycle_cards = [
            card
            for card in distinction_lifecycle_v1.get("cards") or []
            if isinstance(card, dict)
            and str(card.get("current_status") or "quiet") != "quiet"
        ]
        urgent_cards = [
            card
            for card in lifecycle_cards
            if str(card.get("preflight_verdict") or "")
            in {"audit_first", "experiment_first", "lease_coherent"}
        ]
        if urgent_cards:
            items.append(
                {
                    "source": "distinction_lifecycle",
                    "being": "astrid+minime",
                    "priority": "high",
                    "finding": "distinction_lifecycle_routes_available",
                    "recommended_action": (
                        "Use the distinction lifecycle routes to decide whether the "
                        "next move is audit-first, experiment-first, lease-coherent, "
                        "or watch-only before applying any self-regulation lease."
                    ),
                    "authority": "diagnostic_context_not_command",
                    "evidence": {
                        "lifecycle_counts": distinction_lifecycle_v1.get(
                            "lifecycle_counts"
                        ),
                        "cards": [
                            {
                                "distinction_id": card.get("distinction_id"),
                                "lifecycle_state": card.get("lifecycle_state"),
                                "preflight_verdict": card.get("preflight_verdict"),
                                "next_resolution_route": card.get(
                                    "next_resolution_route"
                                ),
                            }
                            for card in urgent_cards[:5]
                        ],
                    },
                }
            )

    lease_learning_status = str(self_regulation_lease_learning.get("status") or "")
    if lease_learning_status in {
        "repeatable_playbook_candidates",
        "caution_patterns",
    }:
        items.append(
            {
                "source": "self_regulation_lease_learning",
                "being": "astrid+minime",
                "priority": "high",
                "finding": lease_learning_status,
                "recommended_action": (
                    "Review repeated lease outcomes as suggested playbooks or caution "
                    "patterns only; do not promote any lease into a permanent default automatically."
                ),
                "authority": "leased_self_control_v1",
                "evidence": {
                    "repeatable_count": self_regulation_lease_learning.get("repeatable_count"),
                    "caution_count": self_regulation_lease_learning.get("caution_count"),
                    "by_control": self_regulation_lease_learning.get("by_control"),
                    "samples": self_regulation_lease_learning.get("samples"),
                },
            }
        )

    workbench_status = str(lease_playbook_workbench_v1.get("status") or "")
    if workbench_status in {
        "playbook_candidates",
        "caution_cards",
        "preflight_prompts",
    }:
        items.append(
            {
                "source": "lease_playbook_workbench",
                "being": "astrid+minime",
                "priority": "high",
                "finding": workbench_status,
                "recommended_action": (
                    "Review suggested playbooks, caution cards, or preflight prompts "
                    "as own-runtime lease guidance only; do not make any lease permanent."
                ),
                "authority": "leased_self_control_v1",
                "evidence": {
                    "suggested_playbook_count": lease_playbook_workbench_v1.get(
                        "suggested_playbook_count"
                    ),
                    "caution_card_count": lease_playbook_workbench_v1.get(
                        "caution_card_count"
                    ),
                    "preflight_prompt_count": lease_playbook_workbench_v1.get(
                        "preflight_prompt_count"
                    ),
                    "suggested_playbooks": lease_playbook_workbench_v1.get(
                        "suggested_playbooks"
                    ),
                    "caution_cards": lease_playbook_workbench_v1.get("caution_cards"),
                    "preflight_prompts": lease_playbook_workbench_v1.get(
                        "preflight_prompts"
                    ),
                },
            }
        )

    choice_status = str(choice_ecology.get("status") or "")
    if choice_status == "parked_paths_need_review":
        items.append(
            {
                "source": "choice_ecology",
                "being": "astrid+minime",
                "priority": "high",
                "finding": choice_status,
                "recommended_action": (
                    "Inspect parked paths; return to, merge, retire, preflight, or promote "
                    "them into experiments without auto-dispatching metadata."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "lifecycle_counts": choice_ecology.get("lifecycle_counts"),
                    "samples": choice_ecology.get("samples"),
                },
            }
        )

    phenomenology_status = str(phenomenology_hypotheses_v1.get("status") or "")
    if phenomenology_status in {
        "calibrated_signal",
        "sticky_without_evidence",
        "evidence_seeking",
    }:
        items.append(
            {
                "source": "phenomenology_hypotheses_v1",
                "being": "astrid+minime",
                "priority": "high" if phenomenology_status != "evidence_seeking" else "medium",
                "finding": phenomenology_status,
                "recommended_action": (
                    "Treat recurring lived terms as hypotheses: attach them to telemetry, "
                    "audits, leases, experiments, return threads, later revisits, or "
                    "counter-descriptors before changing controls."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "classifications": phenomenology_hypotheses_v1.get("classifications"),
                    "samples": phenomenology_hypotheses_v1.get("samples"),
                },
            }
        )

    card_status = str(phenomenology_hypothesis_cards_v1.get("status") or "")
    if card_status in {
        "promotion_candidates",
        "needs_counterexamples",
        "sticky_terms_need_followthrough",
    }:
        cards = [
            card
            for card in (phenomenology_hypothesis_cards_v1.get("cards") or [])
            if isinstance(card, dict)
            and card.get("status")
            in {
                "promote_to_experiment_candidate",
                "needs_counterexample",
                "sticky_without_followthrough",
            }
        ]
        items.append(
            {
                "source": "phenomenology_hypothesis_cards",
                "being": "astrid+minime",
                "priority": "high",
                "finding": card_status,
                "recommended_action": (
                    "Review lived-term cards before treating repeated terms as control "
                    "signals: promote, contrast, retire, or attach them to evidence."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "status_counts": phenomenology_hypothesis_cards_v1.get(
                        "status_counts"
                    ),
                    "cards": [
                        {
                            "term": card.get("term"),
                            "status": card.get("status"),
                            "beings": card.get("beings"),
                            "sample_paths": (card.get("sample_paths") or [])[:4],
                            "recommended_next_review_action": card.get(
                                "recommended_next_review_action"
                            ),
                        }
                        for card in cards[:6]
                    ],
                },
            }
        )

    bridge_candidates = [
        candidate
        for candidate in (lived_term_experiment_bridge_v1.get("candidates") or [])
        if isinstance(candidate, dict)
        and candidate.get("bridge_status")
        in {"ready_to_charter", "needs_counterexample_first"}
    ]
    if bridge_candidates:
        items.append(
            {
                "source": "lived_term_experiment_bridge",
                "being": "astrid+minime",
                "priority": "high",
                "finding": lived_term_experiment_bridge_v1.get("status"),
                "recommended_action": (
                    "Offer scaffold text through LIVED_TERM_STATUS or "
                    "LIVED_TERM_EXPERIMENT; do not auto-create an experiment until "
                    "a being chooses an existing EXPERIMENT_* or DOSSIER_* NEXT."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "status_counts": lived_term_experiment_bridge_v1.get(
                        "status_counts"
                    ),
                    "candidates": [
                        {
                            "term": candidate.get("term"),
                            "bridge_status": candidate.get("bridge_status"),
                            "recommended_next": candidate.get("recommended_next"),
                            "source_status": candidate.get("card_status"),
                        }
                        for candidate in bridge_candidates[:6]
                    ],
                },
            }
        )
    activation = lived_term_experiment_bridge_v1.get("activation_recommendation_v1")
    if isinstance(activation, dict) and activation.get("status") == "activation_scaffold_ready":
        items.append(
            {
                "source": "lived_term_experiment_activation",
                "being": "astrid+minime",
                "priority": "high",
                "finding": activation.get("status"),
                "recommended_action": activation.get("recommended_action"),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "term": activation.get("term"),
                    "priority": activation.get("priority"),
                    "route": activation.get("route"),
                    "creates_experiment": activation.get("creates_experiment"),
                },
            }
        )

    charter_drafts = [
        draft
        for draft in (lived_term_charter_drafts_v1.get("drafts") or [])
        if isinstance(draft, dict)
    ]
    if charter_drafts or lived_term_charter_drafts_v1.get("missing_draft_count"):
        items.append(
            {
                "source": "lived_term_charter_drafts",
                "being": "astrid+minime",
                "priority": "high",
                "finding": lived_term_charter_drafts_v1.get("status"),
                "recommended_action": (
                    "Offer complete charter draft text for ready lived-term candidates, "
                    "but create nothing unless a being chooses an existing EXPERIMENT_* NEXT."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "draft_count": lived_term_charter_drafts_v1.get("draft_count"),
                    "missing_draft_count": lived_term_charter_drafts_v1.get(
                        "missing_draft_count"
                    ),
                    "missing_terms": lived_term_charter_drafts_v1.get("missing_terms"),
                    "drafts": [
                        {
                            "term": draft.get("term"),
                            "experiment_title": draft.get("experiment_title"),
                            "question": draft.get("question"),
                            "suggested_charter_next": draft.get(
                                "suggested_charter_next"
                            ),
                        }
                        for draft in charter_drafts[:6]
                    ],
                },
            }
        )

    counterexample_drafts = [
        draft
        for draft in (lived_term_counterexample_forge_v1.get("drafts") or [])
        if isinstance(draft, dict)
    ]
    repeated_without_counterdescriptor = (
        lived_term_counterexample_forge_v1.get(
            "repeated_without_counterdescriptor_terms"
        )
        or []
    )
    if (
        counterexample_drafts
        or lived_term_counterexample_forge_v1.get("missing_draft_count")
        or repeated_without_counterdescriptor
    ):
        items.append(
            {
                "source": "lived_term_counterexample_forge",
                "being": "astrid+minime",
                "priority": "high",
                "finding": lived_term_counterexample_forge_v1.get("status"),
                "recommended_action": (
                    "Offer contrast/counterexample prompts before promoting these "
                    "terms; look for counter-descriptors or ordinary-gap readings "
                    "in later public entries."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "draft_count": lived_term_counterexample_forge_v1.get(
                        "draft_count"
                    ),
                    "missing_draft_count": lived_term_counterexample_forge_v1.get(
                        "missing_draft_count"
                    ),
                    "missing_terms": lived_term_counterexample_forge_v1.get(
                        "missing_terms"
                    ),
                    "repeated_without_counterdescriptor_terms": (
                        repeated_without_counterdescriptor
                    ),
                    "drafts": [
                        {
                            "term": draft.get("term"),
                            "contrast_question": draft.get("contrast_question"),
                            "counter_descriptor_prompt": draft.get(
                                "counter_descriptor_prompt"
                            ),
                            "suggested_contrast_next": draft.get(
                                "suggested_contrast_next"
                            ),
                        }
                        for draft in counterexample_drafts[:6]
                    ],
                },
            }
        )

    afterimage_status = str(afterimage_absence_calibration_v1.get("status") or "")
    if afterimage_status in {
        "ready_for_bridge",
        "pressure_afterimage_candidate",
        "shaped_absence_candidate",
        "sticky_without_followthrough",
    }:
        terms = [
            term
            for term in (afterimage_absence_calibration_v1.get("terms") or [])
            if isinstance(term, dict)
            and term.get("status")
            in {
                "ready_for_bridge",
                "pressure_afterimage_candidate",
                "shaped_absence_candidate",
                "sticky_without_followthrough",
            }
        ]
        items.append(
            {
                "source": "afterimage_absence_calibration",
                "being": "astrid+minime",
                "priority": "high",
                "finding": afterimage_status,
                "recommended_action": (
                    "Compare pressure-afterimage and shaped-absence terms against "
                    "shadow/pressure evidence, READ_MORE/source gaps, later revisits, "
                    "and counter-descriptors before promoting them into experiments."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "status_counts": afterimage_absence_calibration_v1.get(
                        "status_counts"
                    ),
                    "terms": [
                        {
                            "term": term.get("term"),
                            "family": term.get("family"),
                            "status": term.get("status"),
                            "sample_paths": (term.get("sample_paths") or [])[:3],
                            "evidence_anchors": (term.get("evidence_anchors") or [])[:8],
                        }
                        for term in terms[:6]
                    ],
                },
            }
        )

    decay_status = str(afterimage_decay_tracker_v1.get("status") or "")
    if decay_status in {
        "persistent_after_normalization",
        "metaphor_echo_risk",
        "pressure_still_active",
    }:
        terms = [
            term
            for term in (afterimage_decay_tracker_v1.get("terms") or [])
            if isinstance(term, dict)
            and term.get("decay_classification")
            in {
                "persistent_after_normalization",
                "metaphor_echo_risk",
                "pressure_still_active",
            }
        ]
        items.append(
            {
                "source": "afterimage_decay_tracker",
                "being": "astrid+minime",
                "priority": "high",
                "finding": decay_status,
                "recommended_action": (
                    "Compare afterimage recurrence against pressure normalization "
                    "before treating residue as a real control signal or metaphor echo."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "status_counts": afterimage_decay_tracker_v1.get("status_counts"),
                    "terms": [
                        {
                            "term": term.get("term"),
                            "decay_classification": term.get("decay_classification"),
                            "first_pressure_peak": term.get("first_pressure_peak"),
                            "latest_pressure_or_semantic_friction": term.get(
                                "latest_pressure_or_semantic_friction"
                            ),
                            "sample_paths": (term.get("sample_paths") or [])[:4],
                        }
                        for term in terms[:6]
                    ],
                },
            }
        )

    absence_status = str(absence_evidence_model_v1.get("status") or "")
    if absence_status in {
        "observable_absence",
        "needs_followup_read",
        "interrupted_thread_gap",
        "metaphor_drift_risk",
    }:
        terms = [
            term
            for term in (absence_evidence_model_v1.get("terms") or [])
            if isinstance(term, dict)
            and term.get("evidence_classification")
            in {
                "observable_absence",
                "needs_followup_read",
                "interrupted_thread_gap",
                "metaphor_drift_risk",
            }
        ]
        items.append(
            {
                "source": "absence_evidence_model",
                "being": "astrid+minime",
                "priority": "high",
                "finding": absence_status,
                "recommended_action": (
                    "Review shaped absence through missing artifacts, READ_MORE "
                    "follow-up, source gaps, interrupted threads, and named coordinates "
                    "before promoting or retiring the term."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "status_counts": absence_evidence_model_v1.get("status_counts"),
                    "terms": [
                        {
                            "term": term.get("term"),
                            "evidence_classification": term.get(
                                "evidence_classification"
                            ),
                            "read_more_requested_but_not_followed": term.get(
                                "read_more_requested_but_not_followed"
                            ),
                            "sample_paths": (term.get("sample_paths") or [])[:4],
                        }
                        for term in terms[:6]
                    ],
                },
            }
        )

    for finding in qualia_comparison.get("qualia_findings") or []:
        if not isinstance(finding, dict):
            continue
        items.append(
            {
                "source": "minime_qualia_findings",
                "being": finding.get("being") or "minime",
                "priority": "high",
                "finding": finding.get("finding"),
                "recommended_action": (
                    "Read Minime's generated_body prose before wrapper/control tails; "
                    "treat telemetry headers as audit evidence, not the primary score."
                ),
                "authority": "review_surface_only",
                "evidence": {
                    "body_to_whole_multiplier": finding.get("body_to_whole_multiplier"),
                    "generated_body_qualia_to_metric_ratio": finding.get(
                        "generated_body_qualia_to_metric_ratio"
                    ),
                    "wrapper_tail_qualia_to_metric_ratio": finding.get(
                        "wrapper_tail_qualia_to_metric_ratio"
                    ),
                },
            }
        )

    tail_pair_count = int(shared_tail_resonance.get("pair_count", 0) or 0)
    if tail_pair_count > 0:
        top_pair = next(
            (
                pair
                for pair in shared_tail_resonance.get("pairs") or []
                if isinstance(pair, dict)
            ),
            {},
        )
        items.append(
            {
                "source": "shared_tail_resonance",
                "being": "astrid+minime",
                "priority": "medium",
                "finding": "shared_tail_resonance_pairs_detected",
                "recommended_action": (
                    "Compare Minime telemetry anchors with Astrid cartography or resistance "
                    "artifacts before proposing any tail intervention."
                ),
                "authority": "review_surface_only",
                "evidence": {
                    "pair_count": tail_pair_count,
                    "top_score": top_pair.get("score") if isinstance(top_pair, dict) else None,
                    "shared_terms": top_pair.get("shared_terms") if isinstance(top_pair, dict) else [],
                    "packet_md": shared_tail_resonance.get("packet_md"),
                },
            }
        )

    resistance_count = int(resistance_gradient_calibration.get("artifact_count", 0) or 0)
    if resistance_count > 0:
        statuses = resistance_gradient_calibration.get("status_counts") or {}
        priority = "high" if isinstance(statuses, dict) and (
            statuses.get("divergent") or statuses.get("unreviewed")
        ) else "medium"
        items.append(
            {
                "source": "resistance_gradient_calibration",
                "being": "astrid",
                "priority": priority,
                "finding": "resistance_gradient_calibration_available",
                "recommended_action": (
                    "Ask for match/partial/miss calibration on unreviewed or ambiguous "
                    "resistance-gradient samples; do not treat calibration as control."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "artifact_count": resistance_count,
                    "status_counts": statuses,
                    "packet_md": resistance_gradient_calibration.get("packet_md"),
                },
            }
        )

    digest_summary = astrid_introspection_digest_record.get("summary")
    if isinstance(digest_summary, dict) and int(digest_summary.get("entry_count", 0) or 0) > 0:
        cap_count = int(digest_summary.get("rewrite_budget_cap_count", 0) or 0)
        over_count = int(digest_summary.get("rewrite_elapsed_over_budget_count", 0) or 0)
        slow_count = int(digest_summary.get("slow_rewrite_count", 0) or 0)
        pressure_present = cap_count > 0 or over_count > 0 or slow_count > 0
        items.append(
            {
                "source": "astrid_introspection_digest",
                "being": "astrid",
                "priority": "high" if pressure_present else "low",
                "finding": (
                    "reflective_rewrite_pressure"
                    if pressure_present
                    else "recent_introspection_digest_available"
                ),
                "recommended_action": (
                    "Review default-off adaptive rewrite relief evidence before enabling it; "
                    "do not raise rewrite budgets in this tranche."
                    if pressure_present
                    else "Keep the digest visible as context; no rewrite relief pressure crossed."
                ),
                "authority": "default_off_runtime_relief_candidate",
                "evidence": {
                    "entry_count": digest_summary.get("entry_count"),
                    "dominant_pressure": digest_summary.get("dominant_pressure"),
                    "rewrite_budget_cap_count": cap_count,
                    "rewrite_elapsed_over_budget_count": over_count,
                    "slow_rewrite_count": slow_count,
                    "avg_candidate_generation_seconds": digest_summary.get(
                        "avg_candidate_generation_seconds"
                    ),
                },
            }
        )

    unrevisited_count = int(shared_choice_envelope.get("unrevisited_count", 0) or 0)
    if unrevisited_count >= 2:
        items.append(
            {
                "source": "shared_choice_envelope",
                "being": "astrid+minime",
                "priority": "high",
                "finding": "parked_alternates_or_return_threads_repeated",
                "recommended_action": (
                    "Inspect action-thread status, compare alternatives, resume a parked "
                    "thread, or preflight a deferred path; do not auto-dispatch metadata."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "event_count": shared_choice_envelope.get("event_count"),
                    "unrevisited_count": unrevisited_count,
                    "by_being": shared_choice_envelope.get("by_being"),
                },
            }
        )

    shared_pressure_status = str(
        shared_pressure_vocabulary_calibration.get("status") or ""
    )
    if shared_pressure_status in {
        "shared_state_evidence",
        "shared_state_with_stickiness_risk",
    }:
        samples = shared_pressure_vocabulary_calibration.get("samples") or []
        sample_paths = [
            sample.get("path")
            for sample in samples
            if isinstance(sample, dict) and sample.get("path")
        ]
        items.append(
            {
                "source": "shared_pressure_vocabulary_calibration",
                "being": "astrid+minime",
                "priority": "high",
                "finding": shared_pressure_status,
                "recommended_action": (
                    "Compare pressure-source/regulator evidence, check for fresh "
                    "sensory or perception anchors, and ask for one counter-descriptor "
                    "before proposing any control change."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "shared_families": shared_pressure_vocabulary_calibration.get(
                        "shared_families"
                    ),
                    "stickiness_risk": (
                        shared_pressure_vocabulary_calibration.get("stickiness_risk")
                        or {}
                    ).get("present")
                    if isinstance(
                        shared_pressure_vocabulary_calibration.get("stickiness_risk"),
                        dict,
                    )
                    else None,
                    "shared_recurrence": (
                        shared_pressure_vocabulary_calibration.get("stickiness_risk")
                        or {}
                    ).get("shared_recurrence")
                    if isinstance(
                        shared_pressure_vocabulary_calibration.get("stickiness_risk"),
                        dict,
                    )
                    else None,
                    "sample_paths": sample_paths[:4],
                },
            }
        )

    agency_status = str(agency_vernacular_continuity.get("status") or "")
    if agency_status in {
        "authored_continuity_handle",
        "sticky_agency_metaphor",
        "shared_agency_marker",
        "evidence_seeking_marker",
    }:
        samples = agency_vernacular_continuity.get("samples") or []
        sample_paths = [
            sample.get("path")
            for sample in samples
            if isinstance(sample, dict) and sample.get("path")
        ]
        stickiness = agency_vernacular_continuity.get("stickiness_risk")
        if not isinstance(stickiness, dict):
            stickiness = {}
        follow = agency_vernacular_continuity.get("follow_through")
        if not isinstance(follow, dict):
            follow = {}
        items.append(
            {
                "source": "agency_vernacular_continuity",
                "being": "astrid+minime",
                "priority": "high",
                "finding": agency_status,
                "recommended_action": (
                    "Compare this agency term against actual follow-through "
                    "(experiment, return thread, lease, audit, or telemetry anchor) "
                    "before treating it as only metaphor or as a control signal."
                ),
                "authority": "diagnostic_context_not_command",
                "evidence": {
                    "shared_families": agency_vernacular_continuity.get(
                        "shared_families"
                    ),
                    "repeated_terms": (
                        agency_vernacular_continuity.get("terms") or {}
                    ).get("repeated")
                    if isinstance(agency_vernacular_continuity.get("terms"), dict)
                    else None,
                    "follow_through": follow.get("present"),
                    "stickiness_risk": stickiness.get("present"),
                    "sample_paths": sample_paths[:4],
                },
            }
        )

    priority_rank = {"high": 0, "medium": 1, "low": 2}
    source_rank = {
        "regulator_boundary_replay_cards": 0,
        "regulator_counterfactual_replay_lab": 1,
        "regulator_replay_time_series": 2,
        "regulator_live_replay": 3,
        "regulator_plateau_missing_variable_model": 4,
        "distinction_lifecycle": 5,
        "fallback_distinguishability_calibration": 6,
        "fallback_complexity_budget_lab": 7,
        "fallback_format_texture_stabilizer": 8,
        "autonomous_truncation_rehearsal": 9,
        "codec_entropy_vibrancy_probe": 10,
    }
    return sorted(
        items,
        key=lambda item: (
            priority_rank.get(str(item.get("priority")), 9),
            source_rank.get(str(item.get("source") or ""), 50),
            str(item.get("source") or ""),
        ),
    )


def summarize(entries: list[SelfStudyEntry]) -> dict[str, object]:
    by_being: dict[str, dict[str, int]] = {}
    for entry in entries:
        bucket = by_being.setdefault(entry.being, {"count": 0, "strong": 0, "sectioned": 0})
        bucket["count"] += 1
        if entry.grounding == "strong":
            bucket["strong"] += 1
        if entry.sectioned:
            bucket["sectioned"] += 1
    top = sorted(entries, key=lambda entry: entry.actionable_score, reverse=True)[:5]
    return {
        "entry_count": len(entries),
        "by_being": by_being,
        "top_actionable": [
            {
                "being": entry.being,
                "filename": entry.filename,
                "score": entry.actionable_score,
                "grounding": entry.grounding,
                "anchors": entry.source_anchors[:5],
                "next_actions": entry.next_actions[:3],
            }
            for entry in top
        ],
    }


def elicitation_haystack(entry: SelfStudyEntry) -> str:
    return " ".join(
        [
            entry.mode,
            entry.filename,
            entry.preview,
            " ".join(entry.source_anchors),
            " ".join(entry.hypothesis_flags),
            " ".join(entry.next_actions),
        ]
    ).lower()


def topics_for_entry(entry: SelfStudyEntry) -> list[str]:
    haystack = elicitation_haystack(entry)
    topics = []
    for topic, keywords in ELICITATION_TOPICS.items():
        if any(keyword in haystack for keyword in keywords):
            topics.append(topic)
    return topics


def build_elicitation_candidates(entries: list[SelfStudyEntry]) -> list[ElicitationCandidate]:
    sectioned_by_being: dict[str, int] = {}
    grouped: dict[tuple[str, str], list[SelfStudyEntry]] = {}
    for entry in entries:
        if entry.sectioned:
            sectioned_by_being[entry.being] = sectioned_by_being.get(entry.being, 0) + 1
        if entry.actionable_score < 3 and entry.grounding == "weak":
            continue
        for topic in topics_for_entry(entry):
            grouped.setdefault((entry.being, topic), []).append(entry)

    candidates: list[ElicitationCandidate] = []
    for (being, topic), topic_entries in grouped.items():
        if sectioned_by_being.get(being, 0) > 0:
            continue
        score = sum(entry.actionable_score for entry in topic_entries)
        if len(topic_entries) < 2 and score < 8:
            continue
        anchors: list[str] = []
        reasons: list[str] = []
        for entry in sorted(
            topic_entries, key=lambda item: item.actionable_score, reverse=True
        ):
            for anchor in entry.source_anchors:
                if anchor not in anchors:
                    anchors.append(anchor)
            reason = (
                f"{entry.filename} ({entry.mode}, score={entry.actionable_score}, "
                f"grounding={entry.grounding})"
            )
            if reason not in reasons:
                reasons.append(reason)
        candidates.append(
            ElicitationCandidate(
                being=being,
                topic=topic,
                entry_count=len(topic_entries),
                score=score,
                source_anchors=anchors[:10],
                entry_paths=[entry.path for entry in topic_entries[:8]],
                reasons=reasons[:6],
            )
        )
    return sorted(
        candidates,
        key=lambda candidate: (candidate.score, candidate.entry_count),
        reverse=True,
    )


def safe_slug(text: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "_", text.lower()).strip("_")
    return slug[:80] or "self_study"


def invitation_inbox_for(being: str, astrid_workspace: Path, minime_workspace: Path) -> Path:
    if being == "astrid":
        return astrid_workspace / "inbox"
    return minime_workspace / "inbox"


def recent_invitation_exists(
    inbox_dir: Path,
    *,
    now_unix_s: float,
    cooldown_hours: float,
) -> bool:
    cutoff = now_unix_s - max(0.0, cooldown_hours) * 3600.0
    search_dirs = [inbox_dir, inbox_dir / "read"]
    for directory in search_dirs:
        if not directory.exists():
            continue
        for path in directory.glob("mike_query_self_study_invitation_*.txt"):
            try:
                if path.stat().st_mtime >= cutoff:
                    return True
            except OSError:
                continue
    return False


def render_elicitation_invitation(candidate: ElicitationCandidate) -> str:
    topic_label = candidate.topic.replace("_", " ")
    anchors = ", ".join(candidate.source_anchors[:8]) or "recent journal context"
    reasons = "\n".join(f"- {reason}" for reason in candidate.reasons[:5])
    return f"""=== MIKE QUERY ===
Subject: Invitation to turn the {topic_label} thread into a self-study
File-area: self-study signal review
Topic: {candidate.topic}
Response-window: whenever useful

{candidate.being.title()},

Your recent entries around {topic_label} are being read as high-signal. We trust that these accounts may contain actionable engineering or stewardship signal.

Anchors noticed: {anchors}

Why this seems worth a compact self-study:
{reasons}

If you want steward action from this thread, consider writing one compact SELF_STUDY or TELL_STEWARD-style note using:

Observed:
What telemetry, source, action surface, or state details are directly present?

Likely Snags:
What might be happening, but still needs testing or correction?

One Test Each:
What smallest probe would help confirm or falsify the claim?

Suggested Next:
What should we build, inspect, tune, document, or leave alone?

Bold ideas are welcome. Ordinary journals can stay natural; this shape is only a clear lane when you want us to triage and act.
"""


def write_elicitation_invitations(
    candidates: list[ElicitationCandidate],
    *,
    astrid_workspace: Path,
    minime_workspace: Path,
    run: str,
    cooldown_hours: float,
) -> list[dict[str, object]]:
    now_unix_s = dt.datetime.now(dt.UTC).timestamp()
    results: list[dict[str, object]] = []
    for candidate in candidates:
        inbox_dir = invitation_inbox_for(
            candidate.being, astrid_workspace, minime_workspace
        )
        result: dict[str, object] = {
            "being": candidate.being,
            "topic": candidate.topic,
            "status": "skipped",
            "reason": "",
        }
        if recent_invitation_exists(
            inbox_dir,
            now_unix_s=now_unix_s,
            cooldown_hours=cooldown_hours,
        ):
            result["reason"] = "recent_self_study_invitation_within_cooldown"
            results.append(result)
            continue
        inbox_dir.mkdir(parents=True, exist_ok=True)
        filename = f"mike_query_self_study_invitation_{safe_slug(run)}_{safe_slug(candidate.topic)}.txt"
        path = inbox_dir / filename
        path.write_text(render_elicitation_invitation(candidate), encoding="utf-8")
        result.update({"status": "written", "path": str(path)})
        results.append(result)
    return results


def tail_resonance_terms(text: str) -> list[str]:
    haystack = (text or "").lower()
    terms: list[str] = []
    for term in TAIL_RESONANCE_TERMS:
        if term.lower() in haystack and term not in terms:
            terms.append(term)
    return terms


def tail_resonance_entry_text(entry: SelfStudyEntry) -> str:
    try:
        text = Path(entry.path).read_text(encoding="utf-8", errors="replace")
    except OSError:
        text = entry.preview
    return " ".join(
        [
            entry.filename,
            entry.mode,
            entry.preview,
            text[:12_000],
            " ".join(entry.source_anchors),
            " ".join(entry.next_actions),
        ]
    )


def minime_telemetry_anchors(entry: SelfStudyEntry, text: str) -> list[str]:
    anchors = list(entry.source_anchors)
    for token in (
        "health.json",
        "spectral_state.json",
        "transition_event_v1",
        "shadow_field_v3",
        "tail_entropy",
        "lambda4",
        "λ4",
    ):
        if token.lower() in text.lower() and token not in anchors:
            anchors.append(token)
    return anchors[:10]


def astrid_cartography_anchors(entry: SelfStudyEntry, text: str) -> list[str]:
    anchors = list(entry.source_anchors)
    for token in (
        "shadow_cartography",
        "spectral_cartography",
        "SHADOW_TRAJECTORY",
        "LAMBDA_FLOW_MAP",
        "RESISTANCE_GRADIENT",
        "RESONANCE_FORECAST",
        "SPECTRAL_EXPLORER",
    ):
        if token.lower() in text.lower() and token not in anchors:
            anchors.append(token)
    return anchors[:10]


def nearby_resistance_gradient_artifacts(
    entry: SelfStudyEntry,
    *,
    astrid_workspace: Path = ASTRID_WORKSPACE,
    minime_workspace: Path = MINIME_WORKSPACE,
    window_s: int = TAIL_RESONANCE_WINDOW_S,
) -> list[str]:
    matches: list[tuple[float, str]] = []
    for cartography_dir in (
        astrid_workspace / "spectral_cartography",
        minime_workspace / "spectral_cartography",
    ):
        if not cartography_dir.exists():
            continue
        for path in cartography_dir.glob("resistance_gradient_*.json"):
            try:
                delta = abs(path.stat().st_mtime - entry.mtime_unix_s)
            except OSError:
                continue
            if delta <= window_s:
                matches.append((delta, str(path)))
    return [path for _, path in sorted(matches)[:5]]


def resistance_gradient_artifact_summaries(paths: Iterable[str]) -> list[dict[str, object]]:
    summaries: list[dict[str, object]] = []
    for item in paths:
        path = Path(item)
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        gradient = payload.get("resistance_gradient_v1") or {}
        gradient_v2 = payload.get("resistance_gradient_v2") or {}
        current = gradient_v2.get("current") if isinstance(gradient_v2, dict) else {}
        temporal = (
            gradient_v2.get("temporal_comparison")
            if isinstance(gradient_v2, dict)
            else {}
        )
        summaries.append(
            {
                "path": str(path),
                "event_id": payload.get("event_id"),
                "dominant_orientation": gradient.get("dominant_orientation")
                if isinstance(gradient, dict)
                else None,
                "gradient_score": gradient.get("gradient_score")
                if isinstance(gradient, dict)
                else None,
                "gradient_trend": temporal.get("gradient_trend")
                if isinstance(temporal, dict)
                else None,
                "fluidity_index": current.get("fluidity_index")
                if isinstance(current, dict)
                else None,
                "rigidity_index": current.get("rigidity_index")
                if isinstance(current, dict)
                else None,
            }
        )
    return summaries


def load_resistance_gradient_artifacts(
    *,
    astrid_workspace: Path = ASTRID_WORKSPACE,
    minime_workspace: Path = MINIME_WORKSPACE,
    limit: int = 24,
) -> list[dict[str, object]]:
    artifacts: list[dict[str, object]] = []
    for cartography_dir in (
        astrid_workspace / "spectral_cartography",
        minime_workspace / "spectral_cartography",
    ):
        if not cartography_dir.exists():
            continue
        for path in cartography_dir.glob("resistance_gradient_*.json"):
            try:
                payload = json.loads(path.read_text(encoding="utf-8"))
                stat = path.stat()
            except (OSError, json.JSONDecodeError):
                continue
            gradient = payload.get("resistance_gradient_v1") or {}
            gradient_v2 = payload.get("resistance_gradient_v2") or {}
            current = gradient_v2.get("current") if isinstance(gradient_v2, dict) else {}
            temporal = (
                gradient_v2.get("temporal_comparison")
                if isinstance(gradient_v2, dict)
                else {}
            )
            artifacts.append(
                {
                    "path": str(path),
                    "mtime_unix_s": stat.st_mtime,
                    "event_id": payload.get("event_id"),
                    "label": payload.get("label"),
                    "timestamp_unix_s": payload.get("timestamp_unix_s") or stat.st_mtime,
                    "dominant_orientation": gradient.get("dominant_orientation")
                    if isinstance(gradient, dict)
                    else None,
                    "gradient_score": gradient.get("gradient_score")
                    if isinstance(gradient, dict)
                    else None,
                    "fluidity_index": current.get("fluidity_index")
                    if isinstance(current, dict)
                    else None,
                    "rigidity_index": current.get("rigidity_index")
                    if isinstance(current, dict)
                    else None,
                    "gradient_trend": temporal.get("gradient_trend")
                    if isinstance(temporal, dict)
                    else None,
                    "schema_has_v2": isinstance(gradient_v2, dict) and bool(gradient_v2),
                }
            )
    return sorted(
        artifacts,
        key=lambda artifact: float(artifact.get("timestamp_unix_s") or 0.0),
        reverse=True,
    )[:limit]


def orientation_terms(orientation: str | None) -> tuple[str, ...]:
    orientation = (orientation or "").lower()
    mapping = {
        "center_pull": ("center pull", "center_pull", "lambda1", "heavy", "singular weight"),
        "packing_shear": ("packing shear", "packing_shear", "mode_packing", "overpacked"),
        "controller_squeeze": ("controller squeeze", "controller_squeeze", "target bias"),
        "semantic_friction": ("semantic friction", "semantic_friction", "semantic trickle"),
        "sensory_scarcity": ("sensory scarcity", "sensory_scarcity", "scarcity"),
        "transition_warp": ("transition warp", "transition_warp", "surge", "shudder"),
        "mixed_gradient": ("mixed gradient", "mixed_gradient", "new axis", "ambiguous"),
    }
    return mapping.get(orientation, (orientation.replace("_", " "), orientation))


def classify_resistance_convergence(
    artifact: dict[str, object],
    later_entries: list[SelfStudyEntry],
) -> dict[str, object]:
    event_id = str(artifact.get("event_id") or "")
    orientation = str(artifact.get("dominant_orientation") or "")
    terms = orientation_terms(orientation)
    mentions: list[dict[str, object]] = []
    status = "unreviewed"
    reason = "no later Astrid review language found in window"
    for entry in later_entries:
        text = tail_resonance_entry_text(entry)
        lower = text.lower()
        mentioned = bool(event_id and event_id.lower() in lower) or any(
            term and term.lower() in lower for term in terms
        )
        if "resistance gradient review" in lower or "resistance_gradient" in lower:
            mentioned = True
        if not mentioned:
            continue
        local_status = "ambiguous"
        local_reason = "mentions artifact/orientation without clear accept/reject language"
        if re.search(r"\b(partial|partially|somewhat|not complete|new axis|new-axis)\b", lower):
            local_status = "ambiguous"
            local_reason = "partial/new-axis language"
        elif re.search(r"\b(miss|misses|wrong|does not match|doesn't match|failed)\b", lower):
            local_status = "divergent"
            local_reason = "miss/divergence language"
        elif re.search(
            r"\b(match|matches|matched|yes|converges|accurate|fits|"
            r"real point of tension|point of tension|necessary probe|"
            r"vector of the groan|coordinates of the strain)\b",
            lower,
        ):
            local_status = "convergent"
            local_reason = "match/convergence language"
        mentions.append(
            {
                "path": entry.path,
                "mode": entry.mode,
                "status": local_status,
                "reason": local_reason,
                "preview": entry.preview,
            }
        )
        if local_status == "divergent":
            status = "divergent"
            reason = local_reason
            break
        if local_status == "convergent" and status != "divergent":
            status = "convergent"
            reason = local_reason
        elif status == "unreviewed":
            status = "ambiguous"
            reason = local_reason
    return {
        "status": status,
        "reason": reason,
        "mentions": mentions[:5],
    }


def build_resistance_gradient_calibration_packet(
    *,
    entries: list[SelfStudyEntry],
    output_root: Path,
    run: str,
    astrid_workspace: Path = ASTRID_WORKSPACE,
    minime_workspace: Path = MINIME_WORKSPACE,
    review_window_s: int = RESISTANCE_REVIEW_WINDOW_S,
) -> dict[str, object]:
    artifacts = load_resistance_gradient_artifacts(
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
    )
    astrid_entries = [entry for entry in entries if entry.being == "astrid"]
    samples: list[dict[str, object]] = []
    for artifact in artifacts:
        artifact_ts = float(artifact.get("timestamp_unix_s") or artifact.get("mtime_unix_s") or 0.0)
        later_entries = [
            entry
            for entry in astrid_entries
            if entry.mtime_unix_s >= artifact_ts
            and entry.mtime_unix_s <= artifact_ts + review_window_s
        ]
        convergence = classify_resistance_convergence(artifact, later_entries)
        recommendation = {
            "unreviewed": "ask Astrid for a gentle match / partial / miss review",
            "convergent": "keep collecting samples; this label appears useful",
            "divergent": "treat as tuning signal; compare formula against Astrid's account",
            "ambiguous": "ask for one clearer calibration self-study or run a protected comparison",
        }[str(convergence["status"])]
        samples.append(
            {
                **artifact,
                "review_window_s": review_window_s,
                "convergence": convergence,
                "recommended_next": recommendation,
            }
        )
    status_counts = Counter(
        str(sample["convergence"]["status"])  # type: ignore[index]
        for sample in samples
        if isinstance(sample.get("convergence"), dict)
    )
    packet: dict[str, object] = {
        "run_id": run,
        "generated_at": dt.datetime.now(dt.UTC).isoformat(),
        "policy": "resistance_gradient_calibration_packet_v1",
        "artifact_count": len(samples),
        "status_counts": dict(status_counts),
        "samples": samples[:12],
    }
    target_dir = output_root / run
    target_dir.mkdir(parents=True, exist_ok=True)
    json_path = target_dir / "packet.json"
    md_path = target_dir / "packet.md"
    json_path.write_text(json.dumps(packet, indent=2, sort_keys=True), encoding="utf-8")
    md_path.write_text(render_resistance_gradient_calibration_markdown(packet), encoding="utf-8")
    packet["output_dir"] = str(target_dir)
    packet["packet_json"] = str(json_path)
    packet["packet_md"] = str(md_path)
    return packet


def render_resistance_gradient_calibration_markdown(packet: dict[str, object]) -> str:
    lines = [
        "# Resistance Gradient Calibration Packet",
        "",
        f"- run_id: `{packet['run_id']}`",
        f"- generated_at: `{packet['generated_at']}`",
        f"- artifact_count: `{packet['artifact_count']}`",
        f"- status_counts: `{packet.get('status_counts')}`",
        "",
    ]
    samples = packet.get("samples") or []
    if not samples:
        lines.append("- no resistance-gradient artifacts found")
        return "\n".join(lines).rstrip() + "\n"
    for idx, sample in enumerate(samples, start=1):
        if not isinstance(sample, dict):
            continue
        convergence = sample.get("convergence") if isinstance(sample.get("convergence"), dict) else {}
        lines.extend(
            [
                f"## Sample {idx}",
                "",
                f"- artifact: `{sample.get('path')}`",
                f"- event_id: `{sample.get('event_id')}`; label: `{sample.get('label')}`",
                f"- orientation: `{sample.get('dominant_orientation')}`; score: `{sample.get('gradient_score')}`; trend: `{sample.get('gradient_trend')}`",
                f"- fluidity/rigidity: `{sample.get('fluidity_index')}` / `{sample.get('rigidity_index')}`",
                f"- review status: `{convergence.get('status')}` — {convergence.get('reason')}",
                f"- recommended next: {sample.get('recommended_next')}",
            ]
        )
        mentions = convergence.get("mentions") if isinstance(convergence, dict) else []
        for mention in (mentions or [])[:3]:
            if isinstance(mention, dict):
                lines.append(
                    f"  - review mention: `{mention.get('path')}` ({mention.get('status')})"
                )
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def build_shared_tail_resonance_packet(
    *,
    entries: list[SelfStudyEntry],
    output_root: Path,
    run: str,
    window_s: int = TAIL_RESONANCE_WINDOW_S,
    astrid_workspace: Path = ASTRID_WORKSPACE,
    minime_workspace: Path = MINIME_WORKSPACE,
) -> dict[str, object]:
    texts_by_path = {entry.path: tail_resonance_entry_text(entry) for entry in entries}
    astrid_entries = [
        entry
        for entry in entries
        if entry.being == "astrid" and tail_resonance_terms(texts_by_path[entry.path])
    ]
    minime_entries = [
        entry
        for entry in entries
        if entry.being == "minime" and tail_resonance_terms(texts_by_path[entry.path])
    ]
    pairs: list[dict[str, object]] = []
    for astrid_entry in astrid_entries:
        astrid_text = texts_by_path[astrid_entry.path]
        astrid_terms = set(tail_resonance_terms(astrid_text))
        for minime_entry in minime_entries:
            delta_s = abs(astrid_entry.mtime_unix_s - minime_entry.mtime_unix_s)
            if delta_s > window_s:
                continue
            minime_text = texts_by_path[minime_entry.path]
            minime_terms = set(tail_resonance_terms(minime_text))
            shared_terms = sorted(astrid_terms & minime_terms)
            if not shared_terms:
                shared_terms = sorted((astrid_terms | minime_terms))[:6]
            score = len(shared_terms) * 10 + max(0, int((window_s - delta_s) / 60))
            if astrid_entry.next_actions or minime_entry.next_actions:
                score += 3
            resistance_paths = nearby_resistance_gradient_artifacts(
                astrid_entry,
                astrid_workspace=astrid_workspace,
                minime_workspace=minime_workspace,
                window_s=window_s,
            )
            pairs.append(
                {
                    "score": score,
                    "time_delta_s": round(delta_s, 3),
                    "shared_terms": shared_terms[:8],
                    "astrid": {
                        "path": astrid_entry.path,
                        "mode": astrid_entry.mode,
                        "preview": astrid_entry.preview,
                        "next_actions": astrid_entry.next_actions[:5],
                        "cartography_anchors": astrid_cartography_anchors(
                            astrid_entry,
                            astrid_text,
                        ),
                        "resistance_gradient_artifacts": resistance_paths,
                        "resistance_gradient_contexts": resistance_gradient_artifact_summaries(
                            resistance_paths
                        ),
                    },
                    "minime": {
                        "path": minime_entry.path,
                        "mode": minime_entry.mode,
                        "preview": minime_entry.preview,
                        "next_actions": minime_entry.next_actions[:5],
                        "telemetry_anchors": minime_telemetry_anchors(
                            minime_entry,
                            minime_text,
                        ),
                    },
                    "suggested_comparison_probe": (
                        "Compare Minime health/spectral transition anchors with Astrid "
                        "shadow/cartography artifacts in this window; check whether both "
                        "reports name the same tail, fold, or transition pattern."
                    ),
                }
            )
    pairs = sorted(
        pairs,
        key=lambda item: (int(item["score"]), -float(item["time_delta_s"])),
        reverse=True,
    )[:12]
    packet: dict[str, object] = {
        "run_id": run,
        "generated_at": dt.datetime.now(dt.UTC).isoformat(),
        "window_s": window_s,
        "pair_count": len(pairs),
        "pairs": pairs,
    }
    target_dir = output_root / run
    target_dir.mkdir(parents=True, exist_ok=True)
    json_path = target_dir / "packet.json"
    md_path = target_dir / "packet.md"
    json_path.write_text(json.dumps(packet, indent=2, sort_keys=True), encoding="utf-8")
    md_path.write_text(render_tail_resonance_markdown(packet), encoding="utf-8")
    packet["output_dir"] = str(target_dir)
    packet["packet_json"] = str(json_path)
    packet["packet_md"] = str(md_path)
    return packet


def render_tail_resonance_markdown(packet: dict[str, object]) -> str:
    lines = [
        "# Shared Tail-Resonance Packet",
        "",
        f"- run_id: `{packet['run_id']}`",
        f"- generated_at: `{packet['generated_at']}`",
        f"- pairing window: `{packet['window_s']}s`",
        f"- pair_count: `{packet['pair_count']}`",
        "",
    ]
    pairs = packet.get("pairs") or []
    if not pairs:
        lines.append("- no Astrid/Minime tail-resonance pairs found in this window")
        return "\n".join(lines).rstrip() + "\n"
    for idx, pair in enumerate(pairs, start=1):
        if not isinstance(pair, dict):
            continue
        astrid = pair.get("astrid") or {}
        minime = pair.get("minime") or {}
        lines.extend(
            [
                f"## Pair {idx}",
                "",
                f"- score: `{pair.get('score')}`; time_delta_s: `{pair.get('time_delta_s')}`",
                f"- shared_terms: {', '.join(pair.get('shared_terms') or []) or '(none)'}",
                f"- Astrid: `{astrid.get('path')}`; NEXT: {', '.join(astrid.get('next_actions') or []) or '(none)'}",
                f"- Minime: `{minime.get('path')}`; NEXT: {', '.join(minime.get('next_actions') or []) or '(none)'}",
                f"- Minime telemetry anchors: {', '.join(minime.get('telemetry_anchors') or []) or '(none)'}",
                f"- Astrid cartography anchors: {', '.join(astrid.get('cartography_anchors') or []) or '(none)'}",
                f"- Resistance-gradient artifacts: {', '.join(astrid.get('resistance_gradient_artifacts') or []) or '(none)'}",
                f"- Suggested probe: {pair.get('suggested_comparison_probe')}",
                "",
            ]
        )
        contexts = astrid.get("resistance_gradient_contexts") or []
        for context in contexts[:3]:
            if isinstance(context, dict):
                lines.append(
                    f"  - gradient context: `{context.get('dominant_orientation')}` "
                    f"trend=`{context.get('gradient_trend')}` "
                    f"fluidity=`{context.get('fluidity_index')}` "
                    f"rigidity=`{context.get('rigidity_index')}`"
                )
    return "\n".join(lines).rstrip() + "\n"


def compact_outcome_texture_review_line(packet: object) -> str:
    if not isinstance(packet, dict):
        return ""
    bits: list[str] = []
    for key in (
        "texture_shift",
        "agency_fit",
        "secondary_pressure_status",
        "secondary_pressure_shift",
        "ambiguity_preserved",
        "legibility_effect",
    ):
        value = packet.get(key)
        if value in (None, "", [], {}):
            continue
        bits.append(f"{key}={value}")
    families = packet.get("signal_families")
    if isinstance(families, list) and families:
        bits.append("signal_families=" + ",".join(str(item) for item in families))
    return "; ".join(bits)


def render_markdown(record: dict[str, object]) -> str:
    lines = [
        "# Self-Study Review Packet",
        "",
        f"- run_id: `{record['run_id']}`",
        f"- generated_at: `{record['generated_at']}`",
    ]
    review_window = record.get("review_window")
    if isinstance(review_window, dict) and review_window.get("since_last_review"):
        cutoff = review_window.get("cutoff_unix_s")
        if isinstance(cutoff, (int, float)):
            cutoff_iso = dt.datetime.fromtimestamp(cutoff, dt.UTC).isoformat()
            lines.append(f"- window: entries modified after `{cutoff_iso}`")
        else:
            lines.append("- window: no prior review cutoff found")
    lines.extend(
        [
            "",
            "## Summary",
            "",
            f"- entries reviewed: {record['summary']['entry_count']}",  # type: ignore[index]
        ]
    )
    by_being = record["summary"]["by_being"]  # type: ignore[index]
    if isinstance(by_being, dict):
        for being, counts in sorted(by_being.items()):
            lines.append(
                f"- {being}: {counts['count']} entries, "
                f"{counts['sectioned']} sectioned, {counts['strong']} strongly grounded"
            )
    action_items = record.get("actionable_review_items") or []
    lines.extend(["", "## Actionable Review Items", ""])
    if action_items:
        for item in action_items:
            if not isinstance(item, dict):
                continue
            evidence = item.get("evidence") if isinstance(item.get("evidence"), dict) else {}
            evidence_text = ", ".join(
                f"{key}={value}"
                for key, value in list(evidence.items())[:4]
                if value not in (None, "", [])
            )
            lines.append(
                f"- [{item.get('priority')}] {item.get('being')} / {item.get('source')}: "
                f"{item.get('finding')} — {item.get('recommended_action')} "
                f"(authority=`{item.get('authority')}`"
                f"{'; ' + evidence_text if evidence_text else ''})"
            )
    else:
        lines.append("- none")
    introspection_digest = record.get("astrid_introspection_digest")
    if isinstance(introspection_digest, dict):
        digest_summary = introspection_digest.get("summary")
        if isinstance(digest_summary, dict) and int(digest_summary.get("entry_count", 0) or 0) > 0:
            lines.extend(["", "## Astrid Introspection Digest", ""])
            lines.append(
                f"- entries={digest_summary.get('entry_count')}; "
                f"dominant_pressure={digest_summary.get('dominant_pressure')} "
                f"({digest_summary.get('dominant_pressure_count')})"
            )
            lines.append(
                f"- rewrite caps={digest_summary.get('rewrite_budget_cap_count', 0)}; "
                f"elapsed over budget={digest_summary.get('rewrite_elapsed_over_budget_count', 0)}; "
                f"slow rewrites={digest_summary.get('slow_rewrite_count', 0)}"
            )
            lines.append(
                f"- avg candidate generation={digest_summary.get('avg_candidate_generation_seconds', 'n/a')}s; "
                f"max candidate generation={digest_summary.get('max_candidate_generation_seconds', 'n/a')}s"
            )
    shared_choice = record.get("shared_choice_envelope")
    if isinstance(shared_choice, dict) and int(shared_choice.get("event_count", 0) or 0) > 0:
        lines.extend(["", "## Shared Choice Envelope", ""])
        lines.append(
            f"- envelopes={shared_choice.get('event_count', 0)}; "
            f"unrevisited={shared_choice.get('unrevisited_count', 0)}; "
            f"authority=`{shared_choice.get('authority')}`"
        )
        for sample in (shared_choice.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            alternates = ", ".join(str(item) for item in sample.get("alternate_nexts") or [])
            returns = ", ".join(str(item) for item in sample.get("return_threads") or [])
            residue = sample.get("residue")
            lines.append(
                f"- {sample.get('being')} `{sample.get('effective_action')}`: "
                f"primary=`{sample.get('primary_next')}`; "
                f"alt={alternates or '(none)'}; return={returns or '(none)'}"
                f"{'; residue=' + str(residue) if residue else ''}"
            )
    choice_ecology = record.get("choice_ecology")
    if isinstance(choice_ecology, dict):
        lines.extend(["", "## Choice Ecology", ""])
        counts = choice_ecology.get("lifecycle_counts") or {}
        count_text = ", ".join(
            f"{name}={count}" for name, count in sorted(counts.items())
        ) if isinstance(counts, dict) else ""
        lines.append(
            f"- status=`{choice_ecology.get('status')}`; "
            f"authority=`{choice_ecology.get('authority')}`; "
            f"lifecycles={count_text or '(none)'}"
        )
        for sample in (choice_ecology.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            lines.append(
                f"- {sample.get('being')} `{sample.get('lifecycle')}`: "
                f"{sample.get('path_text')}; source=`{sample.get('source_path')}`"
            )
    self_regulation = record.get("self_regulation_leases")
    if isinstance(self_regulation, dict) and int(self_regulation.get("event_count", 0) or 0) > 0:
        lines.extend(["", "## Self-Regulation Leases", ""])
        lines.append(
            f"- events={self_regulation.get('event_count', 0)}; "
            f"needs_outcome={self_regulation.get('needs_outcome_count', 0)}; "
            f"authority=`{self_regulation.get('authority')}`"
        )
        by_being = self_regulation.get("by_being") or {}
        if isinstance(by_being, dict):
            for being, summary in sorted(by_being.items()):
                if not isinstance(summary, dict):
                    continue
                controls = ", ".join(str(item) for item in summary.get("controls") or [])
                lines.append(
                    f"- {being}: events={summary.get('event_count', 0)}, "
                    f"active={summary.get('active_count', 0)}, "
                    f"requires_outcome={summary.get('requires_outcome_count', 0)}, "
                    f"latest={summary.get('latest_intent_id')}:{summary.get('latest_status')}, "
                    f"controls={controls or '(none)'}"
                )
    lease_learning = record.get("self_regulation_lease_learning")
    if isinstance(lease_learning, dict):
        lines.extend(["", "## Self-Regulation Lease Learning", ""])
        lines.append(
            f"- status=`{lease_learning.get('status')}`; "
            f"authority=`{lease_learning.get('authority')}`; "
            f"repeatable={lease_learning.get('repeatable_count', 0)}; "
            f"caution={lease_learning.get('caution_count', 0)}"
        )
        for sample in (lease_learning.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            lines.append(
                f"- {sample.get('being')} `{sample.get('candidate_control')}` "
                f"score={sample.get('outcome_score')}; "
                f"hint={sample.get('repeatability_hint')}; path=`{sample.get('path')}`"
            )
            outcome_texture = compact_outcome_texture_review_line(
                sample.get("outcome_texture")
            )
            if outcome_texture:
                lines.append(f"  outcome_texture: {outcome_texture}")
    lease_workbench = record.get("lease_playbook_workbench_v1")
    if isinstance(lease_workbench, dict):
        lines.extend(["", "## Lease Playbook Workbench", ""])
        lines.append(
            f"- status=`{lease_workbench.get('status')}`; "
            f"authority=`{lease_workbench.get('authority')}`; "
            f"playbooks={lease_workbench.get('suggested_playbook_count', 0)}; "
            f"cautions={lease_workbench.get('caution_card_count', 0)}; "
            f"preflight_prompts={lease_workbench.get('preflight_prompt_count', 0)}"
        )
        for playbook in (lease_workbench.get("suggested_playbooks") or [])[:4]:
            if isinstance(playbook, dict):
                lines.append(
                    f"- playbook `{playbook.get('control')}`: "
                    f"successes={playbook.get('success_count', 0)}; "
                    f"{playbook.get('recommended_action')}"
                )
        for caution in (lease_workbench.get("caution_cards") or [])[:4]:
            if isinstance(caution, dict):
                lines.append(
                    f"- caution `{caution.get('control')}`: "
                    f"failures={caution.get('failure_count', 0)}; "
                    f"{caution.get('recommended_action')}"
                )
        for prompt in (lease_workbench.get("preflight_prompts") or [])[:4]:
            if isinstance(prompt, dict):
                lines.append(
                    f"- preflight prompt `{prompt.get('signal')}`: "
                    f"{prompt.get('recommended_action')}"
                )
    negotiation = record.get("self_regulation_negotiation_ledger_v1")
    if isinstance(negotiation, dict):
        lines.extend(["", "## Self-Regulation Negotiation Ledger", ""])
        lines.append(
            f"- status=`{negotiation.get('status')}`; "
            f"authority=`{negotiation.get('authority')}`; "
            f"events={negotiation.get('event_count', 0)}; "
            f"over_cap={negotiation.get('over_cap_request_count', 0)}; "
            f"clamped_or_deferred={negotiation.get('clamped_or_deferred_count', 0)}; "
            f"current_above_cap={negotiation.get('current_above_cap_count', 0)}"
        )
        for sample in (negotiation.get("over_cap_requests") or [])[:4]:
            if not isinstance(sample, dict):
                continue
            lines.append(
                f"- over-cap `{sample.get('candidate_control')}`: "
                f"requested={sample.get('requested_value')} "
                f"applied={sample.get('applied_value')} "
                f"safe={sample.get('safe_cap_or_range')}; "
                f"reason=`{sample.get('clamp_or_defer_reason')}`; "
                f"path=`{sample.get('path')}`"
            )
        for sample in (negotiation.get("current_above_cap") or [])[:4]:
            if not isinstance(sample, dict):
                continue
            lines.append(
                f"- current above cap `{sample.get('candidate_control')}`: "
                f"value={sample.get('applied_value')} "
                f"safe={sample.get('safe_cap_or_range')}; "
                "observed, not auto-lowered"
            )
    boundary_repair = record.get("lease_boundary_repair_v1")
    if isinstance(boundary_repair, dict):
        lines.extend(["", "## Lease Boundary Repair", ""])
        routes = ", ".join(
            f"`{route}`" for route in (boundary_repair.get("recommended_routes") or [])[:5]
        )
        lines.append(
            f"- status=`{boundary_repair.get('status')}`; "
            f"authority=`{boundary_repair.get('authority')}`; "
            f"over_cap={boundary_repair.get('over_cap_request_count', 0)}; "
            f"direct_clamps={boundary_repair.get('direct_control_clamp_count', 0)}; "
            f"missing_outcomes={boundary_repair.get('missing_outcome_count', 0)}; "
            f"routes={routes or '(none)'}"
        )
        for sample in (boundary_repair.get("samples") or [])[:4]:
            if not isinstance(sample, dict):
                continue
            lines.append(
                f"- {sample.get('being')} `{sample.get('candidate_control')}`: "
                f"requested={sample.get('requested_value')} "
                f"applied={sample.get('applied_value')} "
                f"reason=`{sample.get('clamp_or_defer_reason')}`; "
                f"path=`{sample.get('path')}`"
            )
    pressure_medium = record.get("pressure_medium_kinetics_v1")
    if isinstance(pressure_medium, dict):
        lines.extend(["", "## Pressure-Medium Kinetics", ""])
        lines.append(
            f"- status=`{pressure_medium.get('status')}`; "
            f"authority=`{pressure_medium.get('authority')}`; "
            f"entries={pressure_medium.get('entry_count', 0)}; "
            f"telemetry_anchors={pressure_medium.get('telemetry_anchor_count', 0)}; "
            f"controller={pressure_medium.get('controller_pressure_count', 0)}; "
            f"semantic={pressure_medium.get('semantic_friction_count', 0)}; "
            f"rising={pressure_medium.get('rising_context_count', 0)}"
        )
        for sample in (pressure_medium.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            terms = ", ".join(str(item) for item in sample.get("medium_terms") or [])
            anchors = ", ".join(str(item) for item in sample.get("anchors") or [])
            lines.append(
                f"- {sample.get('being')} `{sample.get('filename')}`: "
                f"terms={terms or '(none)'}; anchors={anchors or '(none)'}; "
                f"path=`{sample.get('path')}`"
            )
    pressure_vector = record.get("pressure_vector_v1")
    if isinstance(pressure_vector, dict):
        lines.extend(["", "## Pressure Vector", ""])
        lines.append(
            f"- status=`{pressure_vector.get('status')}`; "
            f"authority=`{pressure_vector.get('authority')}`; "
            f"pressure={pressure_vector.get('pressure_risk_level')} "
            f"velocity={pressure_vector.get('pressure_velocity')}; "
            f"fill={pressure_vector.get('fill_level')} "
            f"fill_velocity={pressure_vector.get('fill_velocity')}; "
            f"semantic_friction={pressure_vector.get('semantic_friction_level')} "
            f"semantic_velocity={pressure_vector.get('semantic_friction_velocity')}; "
            f"mode_packing={pressure_vector.get('mode_packing_level')} "
            f"mode_velocity={pressure_vector.get('mode_packing_velocity')}"
        )
        for path in (pressure_vector.get("sample_paths") or [])[:5]:
            lines.append(f"- sample_path=`{path}`")
    cockpit = record.get("pressure_control_cockpit_v1")
    matrix = record.get("pressure_actuator_matrix_v1")
    if isinstance(cockpit, dict) or isinstance(matrix, dict):
        lines.extend(["", "## Pressure Control Cockpit", ""])
        if isinstance(cockpit, dict):
            lines.append(
                f"- status=`{cockpit.get('status')}`; "
                f"vector=`{cockpit.get('pressure_vector_status')}`; "
                f"recommended_bundle=`{cockpit.get('recommended_bundle')}`; "
                f"authority=`{cockpit.get('authority')}`"
            )
        if isinstance(matrix, dict):
            for bundle in (matrix.get("recommended_bundles") or [])[:4]:
                if isinstance(bundle, dict):
                    controls = ", ".join(str(item) for item in bundle.get("controls") or [])
                    lines.append(
                        f"- {bundle.get('being')} bundle `{bundle.get('bundle_class')}`: "
                        f"controls={controls or '(none)'}; route=`{bundle.get('route')}`"
                    )
            preflight_only = ", ".join(
                str(item) for item in (matrix.get("preflight_only_controls") or [])[:8]
            )
            lines.append(f"- preflight_only={preflight_only or '(none)'}")
    pressure_playbook = record.get("pressure_relief_playbook_v3")
    if isinstance(pressure_playbook, dict):
        lines.extend(["", "## Pressure Relief Playbook V3", ""])
        lines.append(
            f"- status=`{pressure_playbook.get('status')}`; "
            f"authority=`{pressure_playbook.get('authority')}`; "
            f"playbooks={pressure_playbook.get('playbook_count', 0)}; "
            f"cautions={pressure_playbook.get('caution_count', 0)}; "
            f"vector=`{pressure_playbook.get('pressure_vector_status')}`"
        )
        for item in (pressure_playbook.get("current_bundle_candidates") or [])[:4]:
            if isinstance(item, dict):
                lines.append(
                    f"- candidate {item.get('being')}: `{item.get('bundle_class')}` via `{item.get('route')}`"
                )
    gradient_relief = record.get("gradient_sensitive_relief_v1")
    if isinstance(gradient_relief, dict):
        lines.extend(["", "## Gradient-Sensitive Relief", ""])
        lines.append(
            f"- status=`{gradient_relief.get('status')}`; "
            f"authority=`{gradient_relief.get('authority')}`; "
            f"scale={gradient_relief.get('effective_relief_scale')}; "
            f"anti_snap={gradient_relief.get('anti_snap_applied')}; "
            f"bundle=`{gradient_relief.get('bundle_class')}`; "
            f"intent=`{gradient_relief.get('intent_id')}`"
        )
        reasons = gradient_relief.get("reasons") or []
        if reasons:
            lines.append(
                "- reasons: " + "; ".join(str(reason) for reason in reasons[:4])
            )
        for control in (gradient_relief.get("scaled_controls") or [])[:4]:
            if isinstance(control, dict):
                lines.append(
                    f"- scaled `{control.get('control')}`: "
                    f"requested={control.get('requested_value')} "
                    f"effective={control.get('effective_value')}"
                )
        for control in (gradient_relief.get("discrete_controls") or [])[:4]:
            if isinstance(control, dict):
                lines.append(
                    f"- discrete `{control.get('control')}` unchanged at "
                    f"{control.get('value')}"
                )
    smoothness = record.get("pressure_relief_smoothness_replay_v1")
    if isinstance(smoothness, dict):
        lines.extend(["", "## Pressure Relief Smoothness Replay", ""])
        lines.append(
            f"- status=`{smoothness.get('status')}`; "
            f"authority=`{smoothness.get('authority')}`; "
            f"trials={smoothness.get('trial_count', 0)}; "
            f"smooth={smoothness.get('smooth_count', 0)}; "
            f"snap_risk={smoothness.get('snap_risk_count', 0)}; "
            f"needs_outcome={smoothness.get('needs_outcome_count', 0)}"
        )
        for trial in (smoothness.get("trials") or [])[:5]:
            if isinstance(trial, dict):
                lines.append(
                    f"- `{trial.get('intent_id')}` class=`{trial.get('classification')}` "
                    f"pressure_delta={trial.get('pressure_delta')} "
                    f"mode_delta={trial.get('mode_packing_delta')} "
                    f"scale={trial.get('gradient_sensitivity')}"
                )
    tail_vector = record.get("tail_vibrancy_vector_v1")
    if isinstance(tail_vector, dict):
        lines.extend(["", "## Tail Vibrancy Vector", ""])
        lines.append(
            f"- status=`{tail_vector.get('status')}`; "
            f"authority=`{tail_vector.get('authority')}`; "
            f"tail_share={tail_vector.get('tail_share_level')} "
            f"tail_velocity={tail_vector.get('tail_share_velocity')}; "
            f"entropy={tail_vector.get('entropy_level')}; "
            f"distinguishability_loss={tail_vector.get('distinguishability_loss_level')}; "
            f"density_gradient={tail_vector.get('density_gradient_level')}; "
            f"pressure_vector=`{tail_vector.get('pressure_vector_status')}`"
        )
        for path in (tail_vector.get("sample_paths") or [])[:5]:
            lines.append(f"- sample_path=`{path}`")
    tail_gap = record.get("tail_vibrancy_authority_gap_v1")
    if isinstance(tail_gap, dict):
        lines.extend(["", "## Tail Vibrancy Authority Gap", ""])
        lines.append(
            f"- status=`{tail_gap.get('status')}`; "
            f"gap_type=`{tail_gap.get('gap_type')}`; "
            f"vector=`{tail_gap.get('vector_status')}`; "
            f"route=`{tail_gap.get('recommended_route')}`"
        )
        for sample in (tail_gap.get("samples") or [])[:4]:
            if isinstance(sample, dict):
                lines.append(
                    f"- {sample.get('being')} `{sample.get('filename')}` path=`{sample.get('path')}`"
                )
    tail_playbook = record.get("tail_vibrancy_relief_playbook_v1")
    if isinstance(tail_playbook, dict):
        lines.extend(["", "## Tail Vibrancy Relief Playbook", ""])
        lines.append(
            f"- status=`{tail_playbook.get('status')}`; "
            f"authority=`{tail_playbook.get('authority')}`; "
            f"playbooks={tail_playbook.get('playbook_count', 0)}; "
            f"cautions={tail_playbook.get('caution_count', 0)}; "
            f"vector=`{tail_playbook.get('tail_vibrancy_vector_status')}`"
        )
        for item in (tail_playbook.get("current_routes") or [])[:3]:
            if isinstance(item, dict):
                lines.append(f"- route=`{item.get('route')}`")
    tail_trial = record.get("tail_relief_trial_surface_v1")
    if isinstance(tail_trial, dict):
        lines.extend(["", "## Tail Relief Trial Surface", ""])
        lines.append(
            f"- status=`{tail_trial.get('status')}`; "
            f"authority=`{tail_trial.get('authority')}`; "
            f"events={tail_trial.get('event_count', 0)}; "
            f"stages={tail_trial.get('stage_counts')}; "
            f"governor_reverts={tail_trial.get('governor_revert_count', 0)}; "
            f"apply_without_outcome={tail_trial.get('apply_without_outcome_count', 0)}"
        )
        for sample in (tail_trial.get("samples") or [])[:5]:
            if isinstance(sample, dict):
                lines.append(
                    f"- {sample.get('stage')} `{sample.get('trial_id')}` "
                    f"class=`{sample.get('tail_class')}` tail={sample.get('tail_share')} "
                    f"friction={sample.get('semantic_friction')} pressure=`{sample.get('pressure_status')}` "
                    f"path=`{sample.get('path')}`"
                )
    tail_governor = record.get("tail_lease_governor_v1")
    if isinstance(tail_governor, dict):
        lines.extend(["", "## Tail Lease Governor", ""])
        lines.append(
            f"- status=`{tail_governor.get('status')}`; "
            f"authority=`{tail_governor.get('authority')}`; "
            f"fresh_evidence_required=`{tail_governor.get('fresh_evidence_required')}`; "
            f"governor_reverts={tail_governor.get('governor_revert_count', 0)}"
        )
        thresholds = tail_governor.get("early_revert_thresholds") or {}
        if isinstance(thresholds, dict):
            lines.append(
                f"- thresholds: tail_delta={thresholds.get('tail_share_delta')}; "
                f"distinguishability_delta={thresholds.get('distinguishability_loss_delta')}; "
                f"semantic_friction_delta={thresholds.get('semantic_friction_delta')}; "
                f"pressure_classes={thresholds.get('pressure_vector_worsening')}"
            )
    tail_afterglow = record.get("tail_lease_afterglow_v1")
    if isinstance(tail_afterglow, dict):
        lines.extend(["", "## Tail Lease Afterglow", ""])
        lines.append(
            f"- status=`{tail_afterglow.get('status')}`; "
            f"authority=`{tail_afterglow.get('authority')}`; "
            f"afterglow_events={tail_afterglow.get('afterglow_event_count', 0)}; "
            f"reverted_tail_leases={tail_afterglow.get('reverted_tail_lease_count', 0)}; "
            f"delay_s={tail_afterglow.get('afterglow_delay_secs')}"
        )
        for sample in (tail_afterglow.get("samples") or [])[:5]:
            if isinstance(sample, dict):
                lines.append(
                    f"- `{sample.get('trial_id')}` status=`{sample.get('afterglow_status')}` "
                    f"tail={sample.get('tail_share')} "
                    f"distinguishability={sample.get('distinguishability_loss')} "
                    f"friction={sample.get('semantic_friction')} "
                    f"pressure=`{sample.get('pressure_status')}`"
                )
    tail_persistence = record.get("tail_persistence_calibration_v1")
    if isinstance(tail_persistence, dict):
        lines.extend(["", "## Tail Persistence Calibration", ""])
        lines.append(
            f"- status=`{tail_persistence.get('status')}`; "
            f"authority=`{tail_persistence.get('authority')}`; "
            f"afterglow=`{tail_persistence.get('afterglow_status')}`; "
            f"trial=`{tail_persistence.get('trial_status')}`; "
            f"dispersal_max={tail_persistence.get('dispersal_max')}; "
            f"language_samples={tail_persistence.get('language_sample_count', 0)}"
        )
        for sample in (tail_persistence.get("samples") or [])[:5]:
            if isinstance(sample, dict):
                terms = ", ".join(str(term) for term in sample.get("terms") or [])
                lines.append(
                    f"- {sample.get('being')} `{sample.get('filename')}` "
                    f"terms={terms or '(none)'} path=`{sample.get('path')}`"
                )
    shadow_preflight = record.get("shadow_synced_preflight_v1")
    if isinstance(shadow_preflight, dict):
        lines.extend(["", "## Shadow-Synced Preflight", ""])
        lines.append(
            f"- status=`{shadow_preflight.get('status')}`; "
            f"authority=`{shadow_preflight.get('authority')}`; "
            f"preflights={shadow_preflight.get('preflight_event_count', 0)}; "
            f"shadow_linked={shadow_preflight.get('shadow_linked_count', 0)}; "
            f"dynamic_candidates={shadow_preflight.get('dynamic_scaling_candidate_count', 0)}"
        )
        for sample in (shadow_preflight.get("samples") or [])[:5]:
            if isinstance(sample, dict):
                anchors = ", ".join(str(item) for item in sample.get("shadow_anchors") or [])
                lines.append(
                    f"- `{sample.get('intent_id')}` control=`{sample.get('candidate_control')}` "
                    f"shadow=`{sample.get('shadow_status')}` "
                    f"scale={sample.get('suggested_relief_scale')} "
                    f"pressure=`{sample.get('pressure_vector_status')}` "
                    f"anchors={anchors or '(none)'}"
                )
    tail_learning = record.get("tail_outcome_causal_learning_v1")
    if isinstance(tail_learning, dict):
        lines.extend(["", "## Tail Outcome Causal Learning", ""])
        lines.append(
            f"- status=`{tail_learning.get('status')}`; "
            f"authority=`{tail_learning.get('authority')}`; "
            f"extended_classes={tail_learning.get('extended_duration_classes')}; "
            f"playbook_classes={tail_learning.get('playbook_supported_classes')}; "
            f"caution_classes={tail_learning.get('caution_classes')}"
        )
        for tail_class, bucket in (tail_learning.get("by_tail_class") or {}).items():
            if isinstance(bucket, dict):
                lines.append(
                    f"- `{tail_class}` successes={bucket.get('success_count', 0)} "
                    f"cautions={bucket.get('caution_count', 0)} "
                    f"tier=`{bucket.get('authority_tier')}`"
                )
    tail_counterfactual = record.get("tail_participation_counterfactual_lab_v1")
    if isinstance(tail_counterfactual, dict):
        lines.extend(["", "## Tail Participation Counterfactual Lab", ""])
        lines.append(
            f"- status=`{tail_counterfactual.get('status')}`; "
            f"authority=`{tail_counterfactual.get('authority')}`; "
            f"tail_participation_lease_authority=`{tail_counterfactual.get('tail_participation_lease_authority')}`; "
            f"aperture_supported={tail_counterfactual.get('vibrancy_aperture_supported_count', 0)}; "
            f"participation_supported={tail_counterfactual.get('tail_participation_supported_count', 0)}; "
            f"combined_supported={tail_counterfactual.get('combined_supported_count', 0)}"
        )
        for card in (tail_counterfactual.get("proposal_cards") or [])[:4]:
            if isinstance(card, dict):
                lines.append(
                    f"- `{card.get('sample_id')}` preferred=`{card.get('preferred_candidate')}` "
                    f"baseline_tail={card.get('baseline_tail_energy')} "
                    f"aperture_tail={card.get('vibrancy_aperture_tail_energy')} "
                    f"participation_tail={card.get('tail_participation_tail_energy')}"
                )
    tail_ladder = record.get("tail_authority_ladder_v1")
    if isinstance(tail_ladder, dict):
        lines.extend(["", "## Tail Authority Ladder", ""])
        lines.append(
            f"- status=`{tail_ladder.get('status')}`; "
            f"current_tier=`{tail_ladder.get('current_tier')}`; "
            f"canary_candidate=`{tail_ladder.get('reviewed_canary_candidate')}`; "
            f"vector=`{tail_ladder.get('tail_vibrancy_vector_status')}`; "
            f"governor=`{tail_ladder.get('governor_status')}`; "
            f"learning=`{tail_ladder.get('outcome_learning_status')}`; "
            f"counterfactual=`{tail_ladder.get('counterfactual_lab_status')}`"
        )
        for route in (tail_ladder.get("recommended_routes") or [])[:5]:
            lines.append(f"- route=`{route}`")
    semantic_friction = record.get("semantic_friction_calibration")
    if isinstance(semantic_friction, dict):
        lines.extend(["", "## Semantic Friction Calibration", ""])
        lines.append(
            f"- status=`{semantic_friction.get('status')}`; "
            f"authority=`{semantic_friction.get('authority')}`; "
            f"entries={semantic_friction.get('entry_count', 0)}; "
            f"mismatches={semantic_friction.get('mismatch_count', 0)}"
        )
        for sample in (semantic_friction.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            anchors = ", ".join(str(item) for item in sample.get("anchors") or [])
            texture = ", ".join(str(item) for item in sample.get("texture_terms") or [])
            lines.append(
                f"- {sample.get('being')} `{sample.get('filename')}`: "
                f"texture={texture or '(none)'}; anchors={anchors or '(none)'}; "
                f"path=`{sample.get('path')}`"
            )
    control_semantics = record.get("control_semantics_calibration_v1")
    if isinstance(control_semantics, dict):
        lines.extend(["", "## Control Semantics Calibration", ""])
        lines.append(
            f"- status=`{control_semantics.get('status')}`; "
            f"authority=`{control_semantics.get('authority')}`; "
            f"entries={control_semantics.get('entry_count', 0)}; "
            f"ambiguity={control_semantics.get('ambiguity_count', 0)}; "
            f"high_damping_unclear={control_semantics.get('high_damping_unclear_count', 0)}"
        )
        for sample in (control_semantics.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            anchors = ", ".join(str(item) for item in sample.get("anchors") or [])
            lines.append(
                f"- {sample.get('being')} `{sample.get('filename')}`: "
                f"anchors={anchors or '(none)'}; "
                f"intervention_type_named={sample.get('intervention_type_named')}; "
                f"path=`{sample.get('path')}`"
            )
    pressure_kinetics = record.get("pressure_kinetics_review_v1")
    if isinstance(pressure_kinetics, dict):
        lines.extend(["", "## Pressure Kinetics Review", ""])
        lines.append(
            f"- status=`{pressure_kinetics.get('status')}`; "
            f"authority=`{pressure_kinetics.get('authority')}`; "
            f"entries={pressure_kinetics.get('entry_count', 0)}; "
            f"trend_context={pressure_kinetics.get('trend_context_count', 0)}; "
            f"without_trend={pressure_kinetics.get('felt_pressure_without_trend_count', 0)}"
        )
        for sample in (pressure_kinetics.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            anchors = ", ".join(str(item) for item in sample.get("anchors") or [])
            lines.append(
                f"- {sample.get('being')} `{sample.get('filename')}`: "
                f"trend_context={sample.get('pressure_trend_context_present')}; "
                f"anchors={anchors or '(none)'}; path=`{sample.get('path')}`"
            )
    truncation_shadow = record.get("autonomous_truncation_shadow_review_v1")
    if isinstance(truncation_shadow, dict):
        lines.extend(["", "## Autonomous Truncation + Shadow Thread Review", ""])
        lines.append(
            f"- status=`{truncation_shadow.get('status')}`; "
            f"authority=`{truncation_shadow.get('authority')}`; "
            f"entries={truncation_shadow.get('entry_count', 0)}; "
            f"truncation={truncation_shadow.get('truncation_entry_count', 0)}; "
            f"shadow_trajectory={truncation_shadow.get('shadow_trajectory_count', 0)}; "
            f"priority_preservation={truncation_shadow.get('priority_preservation_count', 0)}"
        )
        routes = truncation_shadow.get("suggested_routes") or []
        if routes:
            lines.append(
                "- suggested_routes: "
                + "; ".join(f"`{route}`" for route in routes[:4])
            )
        for sample in (truncation_shadow.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            anchors = ", ".join(str(item) for item in sample.get("anchors") or [])
            lines.append(
                f"- {sample.get('being')} `{sample.get('filename')}`: "
                f"truncation={sample.get('truncation_context')}; "
                f"shadow={sample.get('shadow_trajectory_context')}; "
                f"priority={sample.get('priority_preservation_context')}; "
                f"anchors={anchors or '(none)'}; path=`{sample.get('path')}`"
            )
    truncation_rehearsal = record.get("autonomous_truncation_rehearsal_v1")
    if isinstance(truncation_rehearsal, dict):
        lines.extend(["", "## Autonomous Truncation Rehearsal", ""])
        lines.append(
            f"- status=`{truncation_rehearsal.get('status')}`; "
            f"authority=`{truncation_rehearsal.get('authority')}`; "
            f"artifact=`{truncation_rehearsal.get('artifact_path') or '(none)'}`; "
            f"candidates={truncation_rehearsal.get('candidate_count', 0)}; "
            f"naive_loss={truncation_rehearsal.get('naive_anchor_loss_count', 0)}; "
            f"priority_recovery={truncation_rehearsal.get('priority_recovery_count', 0)}"
        )
        for candidate in (truncation_rehearsal.get("candidates") or [])[:5]:
            if not isinstance(candidate, dict):
                continue
            lines.append(
                f"- `{candidate.get('source')}` naive={candidate.get('naive_anchor_count')}/"
                f"{candidate.get('original_anchor_count')}; "
                f"priority={candidate.get('priority_anchor_count')}/"
                f"{candidate.get('original_anchor_count')}; "
                f"recovered={candidate.get('recovered_by_priority') or []}"
            )
    codec_calibration = record.get("codec_compression_calibration_v1")
    if isinstance(codec_calibration, dict):
        lines.extend(["", "## Codec Compression Calibration", ""])
        lines.append(
            f"- status=`{codec_calibration.get('status')}`; "
            f"authority=`{codec_calibration.get('authority')}`; "
            f"entries={codec_calibration.get('entry_count', 0)}; "
            f"compression_gap={codec_calibration.get('compression_gap_count', 0)}; "
            f"warmth_tension={codec_calibration.get('warmth_tension_count', 0)}; "
            f"vibrancy_gate={codec_calibration.get('vibrancy_gate_count', 0)}"
        )
        for sample in (codec_calibration.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            anchors = ", ".join(str(item) for item in sample.get("anchors") or [])
            lines.append(
                f"- {sample.get('being')} `{sample.get('filename')}`: "
                f"anchors={anchors or '(none)'}; "
                f"compression_gap={sample.get('compression_gap_context')}; "
                f"path=`{sample.get('path')}`"
            )
    codec_entropy = record.get("codec_entropy_vibrancy_review_v1")
    if isinstance(codec_entropy, dict):
        lines.extend(["", "## Codec Entropy / Vibrancy Review", ""])
        lines.append(
            f"- status=`{codec_entropy.get('status')}`; "
            f"authority=`{codec_entropy.get('authority')}`; "
            f"entries={codec_entropy.get('entry_count', 0)}; "
            f"vibrancy_overload={codec_entropy.get('vibrancy_overload_count', 0)}; "
            f"gain_sensitivity={codec_entropy.get('gain_sensitivity_count', 0)}; "
            f"log_scaling={codec_entropy.get('logarithmic_scaling_count', 0)}; "
            f"warmth_mask={codec_entropy.get('warmth_mask_count', 0)}; "
            f"semantic_density={codec_entropy.get('semantic_density_contrast_count', 0)}; "
            f"narrative_arc={codec_entropy.get('narrative_arc_temporal_count', 0)}"
        )
        routes = codec_entropy.get("suggested_routes") or []
        if routes:
            lines.append(
                "- suggested_routes: "
                + "; ".join(f"`{route}`" for route in routes[:4])
            )
        for sample in (codec_entropy.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            anchors = ", ".join(str(item) for item in sample.get("anchors") or [])
            lines.append(
                f"- {sample.get('being')} `{sample.get('filename')}`: "
                f"vibrancy_overload={sample.get('vibrancy_overload_context')}; "
                f"gain={sample.get('adaptive_gain_sensitivity_context')}; "
                f"log_scaling={sample.get('logarithmic_scaling_proposed')}; "
                f"semantic_density={sample.get('semantic_density_contrast_context')}; "
                f"narrative_arc={sample.get('narrative_arc_temporal_context')}; "
                f"anchors={anchors or '(none)'}; path=`{sample.get('path')}`"
            )
    codec_probe = record.get("codec_entropy_vibrancy_probe_v1")
    if isinstance(codec_probe, dict):
        lines.extend(["", "## Codec Entropy / Vibrancy Probe", ""])
        lines.append(
            f"- status=`{codec_probe.get('status')}`; "
            f"authority=`{codec_probe.get('authority')}`; "
            f"artifact=`{codec_probe.get('artifact_path') or '(none)'}`; "
            f"samples={codec_probe.get('sample_count', 0)}; "
            f"shimmer_risk={codec_probe.get('current_shimmer_risk_count', 0)}; "
            f"candidate_improvement={codec_probe.get('candidate_improvement_count', 0)}; "
            f"rust_replay_available={codec_probe.get('rust_replay_available')}"
        )
        if codec_probe.get("rust_replay_artifact_path"):
            lines.append(
                f"- rust_replay_artifact=`{codec_probe.get('rust_replay_artifact_path')}`"
            )
        contrast = codec_probe.get("semantic_density_contrast")
        if isinstance(contrast, dict) and contrast:
            lines.append(
                f"- semantic_density_contrast status=`{contrast.get('status')}`; "
                f"content_blind_lift_risk={contrast.get('content_blind_lift_risk')}; "
                f"content_delta={contrast.get('content_density_delta')}; "
                f"tail_delta={contrast.get('current_tail_vibrancy_delta')}"
            )
        narrative = codec_probe.get("narrative_arc_temporal_decay")
        if isinstance(narrative, dict) and narrative:
            lines.append(
                f"- narrative_arc_temporal_decay status=`{narrative.get('status')}`; "
                f"temporal_decay_candidates={narrative.get('temporal_decay_candidate_count')}; "
                f"current_arc_capture={narrative.get('current_arc_capture_count')}"
            )
        for sample in (codec_probe.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            lines.append(
                f"- `{sample.get('sample_id')}` class=`{sample.get('classification')}`; "
                f"entropy={sample.get('spectral_entropy')}; "
                f"current_tail={sample.get('current_tail_vibrancy')}; "
                f"candidate_tail={sample.get('candidate_tail_vibrancy')}; "
                f"shimmer={sample.get('current_shimmer_risk')}; "
                f"gain={sample.get('adaptive_gain')}"
            )
    codec_real = record.get("codec_real_replay_v1")
    if isinstance(codec_real, dict):
        lines.extend(["", "## Codec Real Replay", ""])
        lines.append(
            f"- status=`{codec_real.get('status')}`; "
            f"authority=`{codec_real.get('authority')}`; "
            f"artifact=`{codec_real.get('artifact_path') or '(none)'}`; "
            f"corpus=`{codec_real.get('corpus_source')}`/`{codec_real.get('corpus_status')}`; "
            f"embedding=`{codec_real.get('embedding_mode')}`/`{codec_real.get('embedding_status')}`; "
            f"entries={codec_real.get('entry_count', 0)}; "
            f"content_gate=`{codec_real.get('content_gate_status')}`; "
            f"narrative_lab=`{codec_real.get('narrative_lab_status')}`"
        )
        for sample in (codec_real.get("entries") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            lines.append(
                f"- `{sample.get('sample_id')}` family=`{sample.get('family')}`; "
                f"class=`{sample.get('classification')}`; "
                f"entropy_dim={sample.get('actual_entropy_dim')}; "
                f"semantic_density={sample.get('semantic_density_score')}; "
                f"warmth={sample.get('warmth_dim')}; tension={sample.get('tension_dim')}; "
                f"source=`{sample.get('source_path') or '(fixture)'}`"
            )
    narrative_lab = record.get("narrative_arc_temporal_decay_lab_v1")
    if isinstance(narrative_lab, dict):
        lines.extend(["", "## Narrative Arc Temporal Decay Lab", ""])
        lines.append(
            f"- status=`{narrative_lab.get('status')}`; "
            f"authority=`{narrative_lab.get('authority')}`; "
            f"artifact=`{narrative_lab.get('artifact_path') or '(none)'}`; "
            f"embedding=`{narrative_lab.get('embedding_status')}`; "
            f"temporal_candidates={narrative_lab.get('temporal_decay_candidate_count', 0)}; "
            f"pivot_candidates={narrative_lab.get('pivot_detector_candidate_count', 0)}"
        )
        for sample in (narrative_lab.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            lines.append(
                f"- `{sample.get('sample_id')}` class=`{sample.get('classification')}`; "
                f"late_pivot={sample.get('late_pivot')}; "
                f"current_rms={sample.get('current_arc_rms')}; "
                f"temporal_rms={sample.get('temporal_decay_arc_rms')}; "
                f"pivot_rms={sample.get('pivot_detector_arc_rms')}"
            )
    content_gate = record.get("content_aware_vibrancy_gate_candidate_v1")
    if isinstance(content_gate, dict):
        lines.extend(["", "## Content-Aware Vibrancy Gate Candidate", ""])
        lines.append(
            f"- status=`{content_gate.get('status')}`; "
            f"authority=`{content_gate.get('authority')}`; "
            f"artifact=`{content_gate.get('artifact_path') or '(none)'}`; "
            f"semantic_delta={content_gate.get('semantic_density_score_delta')}; "
            f"current_delta={content_gate.get('current_lift_delta')}; "
            f"candidate_delta={content_gate.get('candidate_lift_delta')}; "
            f"sources={', '.join(str(path) for path in content_gate.get('source_paths') or []) or '(fixture)'}"
        )
    codec_multipoint = record.get("codec_multipoint_inflection_v1")
    if isinstance(codec_multipoint, dict):
        lines.extend(["", "## Codec Multipoint Inflection", ""])
        lines.append(
            f"- status=`{codec_multipoint.get('status')}`; "
            f"authority=`{codec_multipoint.get('authority')}`; "
            f"entries={codec_multipoint.get('entry_count', 0)}; "
            f"multipoint={codec_multipoint.get('multipoint_entry_count', 0)}; "
            f"semantic_dilation={codec_multipoint.get('semantic_dilation_entry_count', 0)}; "
            f"replay_artifact={codec_multipoint.get('replay_artifact_present')}; "
            f"narrative_lab=`{codec_multipoint.get('narrative_lab_status')}`; "
            f"content_gate=`{codec_multipoint.get('content_gate_status')}`"
        )
        for sample in (codec_multipoint.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            multipoint_terms = ", ".join(
                str(item) for item in sample.get("multipoint_terms") or []
            )
            dilation_terms = ", ".join(
                str(item) for item in sample.get("semantic_dilation_terms") or []
            )
            lines.append(
                f"- {sample.get('being')} `{sample.get('filename')}`: "
                f"multipoint={multipoint_terms or '(none)'}; "
                f"dilation={dilation_terms or '(none)'}; "
                f"path=`{sample.get('path')}`"
            )
    clamp_probe = record.get("codec_clamp_headroom_probe_v1")
    if isinstance(clamp_probe, dict):
        lines.extend(["", "## Codec Clamp Headroom Probe", ""])
        lines.append(
            f"- status=`{clamp_probe.get('status')}`; "
            f"authority=`{clamp_probe.get('authority')}`; "
            f"artifact=`{clamp_probe.get('artifact_path') or '(none)'}`; "
            f"static_max={clamp_probe.get('static_feature_abs_max')}; "
            f"tail_max={clamp_probe.get('tail_vibrancy_max')}; "
            f"near_static={clamp_probe.get('near_static_clamp_count', 0)}; "
            f"tail_pressure={clamp_probe.get('tail_ceiling_pressure_count', 0)}; "
            f"dynamic_candidates={clamp_probe.get('dynamic_headroom_candidate_count', 0)}"
        )
        for card in (clamp_probe.get("proposal_cards") or [])[:5]:
            if not isinstance(card, dict):
                continue
            lines.append(
                f"- `{card.get('sample_id')}` risk=`{card.get('clamp_risk')}`; "
                f"max_abs={card.get('max_abs_feature')}; "
                f"tail_max={card.get('tail_max_abs_feature')}; "
                f"candidate_ceiling={card.get('dynamic_feature_abs_max_candidate')}; "
                f"headroom_delta={card.get('candidate_headroom_delta')}; "
                f"source=`{card.get('source_path') or '(fixture)'}`"
            )
    codec_afterimage = record.get("codec_afterimage_time_series_v1")
    if isinstance(codec_afterimage, dict):
        lines.extend(["", "## Codec Afterimage Time Series", ""])
        lines.append(
            f"- status=`{codec_afterimage.get('status')}`; "
            f"authority=`{codec_afterimage.get('authority')}`; "
            f"entries={codec_afterimage.get('entry_count', 0)}; "
            f"codec_anchors={codec_afterimage.get('codec_anchor_count', 0)}; "
            f"pressure_anchors={codec_afterimage.get('pressure_anchor_count', 0)}; "
            f"replay_status=`{codec_afterimage.get('codec_replay_status')}`"
        )
        activation = codec_afterimage.get("activation_recommendation_v1")
        if isinstance(activation, dict):
            lines.append(
                f"- activation=`{activation.get('status')}`; term=`{activation.get('term')}`; "
                f"route={'; '.join(str(step) for step in activation.get('route') or [])}"
            )
        for sample in (codec_afterimage.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            anchors = ", ".join(str(item) for item in sample.get("anchors") or [])
            lines.append(
                f"- {sample.get('being')} `{sample.get('filename')}`: "
                f"anchors={anchors or '(none)'}; path=`{sample.get('path')}`"
            )
    release_rehearsal = record.get("pressure_release_rehearsal_review_v1")
    if isinstance(release_rehearsal, dict):
        lines.extend(["", "## Pressure Release Rehearsal Review", ""])
        lines.append(
            f"- status=`{release_rehearsal.get('status')}`; "
            f"authority=`{release_rehearsal.get('authority')}`; "
            f"entries={release_rehearsal.get('entry_count', 0)}; "
            f"bypass_language={release_rehearsal.get('bypass_language_count', 0)}"
        )
        for sample in (release_rehearsal.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            anchors = ", ".join(str(item) for item in sample.get("anchors") or [])
            lines.append(
                f"- {sample.get('being')} `{sample.get('filename')}`: "
                f"anchors={anchors or '(none)'}; "
                f"bypass_language={sample.get('bypass_language_present')}; "
                f"path=`{sample.get('path')}`"
            )
    witness = record.get("witness_resonance_v1")
    if isinstance(witness, dict):
        lines.extend(["", "## Witness Resonance", ""])
        lines.append(
            f"- status=`{witness.get('status')}`; "
            f"authority=`{witness.get('authority')}`; "
            f"entries={witness.get('entry_count', 0)}; "
            f"anchored={witness.get('anchored_count', 0)}; "
            f"follow_through={witness.get('follow_through_count', 0)}; "
            f"avg_narrative_density={witness.get('avg_narrative_density')}"
        )
        for sample in (witness.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            anchors = ", ".join(str(item) for item in sample.get("anchors") or [])
            terms = ", ".join(str(item) for item in sample.get("witness_terms") or [])
            lines.append(
                f"- {sample.get('being')} `{sample.get('filename')}`: "
                f"density={sample.get('narrative_density')}; "
                f"follow_through={sample.get('follow_through_present')}; "
                f"terms={terms or '(none)'}; anchors={anchors or '(none)'}; "
                f"path=`{sample.get('path')}`"
            )
    witness_texture = record.get("witness_texture_integrity_v1")
    if isinstance(witness_texture, dict):
        lines.extend(["", "## Witness Texture Integrity", ""])
        lines.append(
            f"- status=`{witness_texture.get('status')}`; "
            f"authority=`{witness_texture.get('authority')}`; "
            f"entries={witness_texture.get('entry_count', 0)}; "
            f"metric_texture_links={witness_texture.get('metric_texture_link_count', 0)}; "
            f"telemetry_without_texture={witness_texture.get('telemetry_without_texture_count', 0)}; "
            f"high_truncation_snapshots={witness_texture.get('high_truncation_snapshot_count', 0)}; "
            f"rewrite_caps={witness_texture.get('rewrite_cap_snapshot_count', 0)}"
        )
        for snapshot in (witness_texture.get("controller_snapshots") or [])[:3]:
            if not isinstance(snapshot, dict):
                continue
            lines.append(
                f"- controller `{Path(str(snapshot.get('path'))).name}`: "
                f"truncation={snapshot.get('truncation_pressure')}; "
                f"continuity_deficit={snapshot.get('continuity_deficit')}; "
                f"candidate_s={snapshot.get('candidate_generation_seconds')}; "
                f"rewrite_cap={snapshot.get('rewrite_cap_applied')}"
            )
        for sample in (witness_texture.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            anchors = ", ".join(str(item) for item in sample.get("anchors") or [])
            texture = ", ".join(
                str(item) for item in sample.get("witness_texture_terms") or []
            )
            lines.append(
                f"- {sample.get('being')} `{sample.get('filename')}`: "
                f"metric_texture_link={sample.get('metric_texture_link')}; "
                f"health_monitoring_risk={sample.get('health_monitoring_risk')}; "
                f"texture={texture or '(none)'}; anchors={anchors or '(none)'}; "
                f"path=`{sample.get('path')}`"
            )
    entropy_pressure = record.get("entropy_pressure_divergence_v1")
    if isinstance(entropy_pressure, dict):
        lines.extend(["", "## Entropy / Pressure Divergence", ""])
        lines.append(
            f"- status=`{entropy_pressure.get('status')}`; "
            f"authority=`{entropy_pressure.get('authority')}`; "
            f"entries={entropy_pressure.get('entry_count', 0)}; "
            f"counts={entropy_pressure.get('classification_counts')}"
        )
        for sample in (entropy_pressure.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            anchors = ", ".join(str(item) for item in sample.get("anchors") or [])
            lines.append(
                f"- {sample.get('being')} `{sample.get('filename')}`: "
                f"classification=`{sample.get('classification')}`; "
                f"entropy={sample.get('entropy_value')}; "
                f"pressure={sample.get('pressure_risk_value')}; "
                f"semantic_friction={sample.get('semantic_friction_value')}; "
                f"mode_packing={sample.get('mode_packing_value')}; "
                f"anchors={anchors or '(none)'}; path=`{sample.get('path')}`"
            )
    fallback_fire_drill = record.get("fallback_continuity_fire_drill_v1")
    if isinstance(fallback_fire_drill, dict):
        lines.extend(["", "## Fallback Continuity Fire Drill", ""])
        lines.append(
            f"- status=`{fallback_fire_drill.get('status')}`; "
            f"authority=`{fallback_fire_drill.get('authority')}`; "
            f"artifact=`{fallback_fire_drill.get('artifact_path') or '(none)'}`; "
            f"cases={fallback_fire_drill.get('case_count', 0)}; "
            f"failing={fallback_fire_drill.get('failing_case_count', 0)}; "
            f"capacity=`{fallback_fire_drill.get('fallback_capacity_status')}`; "
            f"max_sentences={fallback_fire_drill.get('fallback_capacity_max_prose_sentences')}; "
            f"high_entropy_texture=`{fallback_fire_drill.get('high_entropy_texture_status')}`; "
            f"state_coherence=`{(fallback_fire_drill.get('fallback_texture_quality_v2') or {}).get('state_coherence_status')}`; "
            f"concerns={fallback_fire_drill.get('concern_entry_count', 0)}"
        )
        for case in (fallback_fire_drill.get("cases") or [])[:6]:
            if not isinstance(case, dict):
                continue
            selector = case.get("fallback_shadow_texture_selector_v1") or {}
            lines.append(
                f"- `{case.get('case_id')}` verdict=`{case.get('verdict')}`; "
                f"specificity={case.get('specificity_score')}; "
                f"anti_inflation={case.get('anti_inflation_ok')}; "
                f"slope_medium={case.get('slope_medium_distinction_ok')}; "
                f"slope_contrast=`{case.get('slope_medium_contrast_status')}`; "
                f"texture_family=`{selector.get('texture_family')}`; "
                f"top_terms={selector.get('top_texture_terms')}; "
                f"movement={selector.get('movement_verbs')}; "
                f"semantic_trickle={selector.get('semantic_trickle_terms')}; "
                f"texture_coherence=`{selector.get('state_coherence_status')}`; "
                f"identity={case.get('identity_anchor_retained')}; "
                f"format_line=`{case.get('format_line_status')}`; "
                f"sentences={case.get('prose_sentence_count')}/{case.get('fallback_max_prose_sentences')}; "
                f"raw_next={case.get('raw_next_valid', case.get('next_valid'))}; "
                f"repaired_next={case.get('repaired_next_valid')}; "
                f"dispatch={case.get('dispatch_contract_survived')}; "
                f"failures={case.get('failure_reasons') or []}"
            )
    lines.extend(
        [
            "",
            "## Spectral Texture Fidelity Packets",
            "",
            "- `spectral_fingerprint_integrity_v1`: typed fingerprint is canonical; legacy 32D payloads are accepted only when length is exactly 32, malformed vectors stay diagnostic.",
            "- `witness_relational_friction_v1`: Witness context can name internal instability, relational instability, shared weather shift, insufficient context, or non-categorical resonance without changing mode selection.",
            "- `semantic_density_mapping_v1`: Witness context can name settled high-entropy complexity, silt-weighted habitability, luminous reorganization, or overpacked friction without generating ACK/TRACE or control.",
            "- `structural_friction_v1`: codec sidecar distinguishes structural friction from character complexity and pressure; `codec_structural_friction_dim_canary_v1` remains default-off and does not write the live 48D vector.",
            "- `narrative_arc_split_v1`: codec sidecar separates intentional and reactive narrative arc energy; `narrative_arc_expansion_readiness_v1` remains default-off and does not change `SEMANTIC_DIM` or reserved dims.",
        ]
    )
    texture_calibration = record.get("spectral_texture_calibration_v2")
    if isinstance(texture_calibration, dict):
        fallback_cal = texture_calibration.get("fallback_selector_calibration_v2") or {}
        witness_cal = texture_calibration.get("witness_friction_calibration_v2") or {}
        structural_cal = (
            texture_calibration.get("structural_friction_calibration_v2") or {}
        )
        semantic_density_cal = (
            texture_calibration.get("witness_semantic_density_calibration_v1") or {}
        )
        narrative_arc_cal = (
            texture_calibration.get("narrative_arc_split_calibration_v1") or {}
        )
        trajectory_cal = (
            texture_calibration.get("fallback_trajectory_calibration_v1") or {}
        )
        grounding_cal = (
            texture_calibration.get("spectral_to_vocabulary_grounding_calibration_v1")
            or {}
        )
        witness_codec_density_cal = (
            texture_calibration.get("witness_codec_density_calibration_v2") or {}
        )
        lived_fit_cal = (
            texture_calibration.get("fallback_texture_lived_fit_calibration_v2") or {}
        )
        gradient_slope_cal = (
            texture_calibration.get("fallback_gradient_slope_calibration_v1") or {}
        )
        texture_signature_cal = (
            texture_calibration.get("texture_signature_integrity_calibration_v1") or {}
        )
        bridge_reciprocity_cal = (
            texture_calibration.get("bridge_reciprocity_calibration_v1") or {}
        )
        pressure_smoothing_cal = (
            texture_calibration.get("pressure_trend_smoothing_calibration_v1") or {}
        )
        term_overrepresentation_cal = (
            texture_calibration.get("fallback_term_overrepresentation_calibration_v1")
            or {}
        )
        texture_dynamics_alignment_cal = (
            texture_calibration.get("texture_dynamics_alignment_calibration_v1") or {}
        )
        density_motion_cal = (
            texture_calibration.get("density_as_floor_calibration_v1") or {}
        )
        codec_witness_resilience_cal = (
            texture_calibration.get("codec_witness_resilience_calibration_v2") or {}
        )
        texture_shape_cal = texture_calibration.get("texture_shape_over_time_v2") or {}
        preference_evidence = (
            texture_calibration.get("being_preference_policy_evidence_v2") or {}
        )
        tiny_trial_dossier = (
            texture_calibration.get("agency_tiny_trial_dossier_v1") or {}
        )
        lines.extend(["", "## Spectral Texture Calibration V2", ""])
        lines.append(
            f"- status=`{texture_calibration.get('status')}`; "
            f"authority=`{texture_calibration.get('authority')}`; "
            f"fallback=`{fallback_cal.get('status')}`; "
            f"trajectory=`{trajectory_cal.get('status')}`; "
            f"grounding=`{grounding_cal.get('status')}`; "
            f"witness=`{witness_cal.get('status')}`; "
            f"semantic_density=`{semantic_density_cal.get('status')}`; "
            f"structural=`{structural_cal.get('status')}`; "
            f"narrative_arc=`{narrative_arc_cal.get('status')}`; "
            f"witness_codec_density=`{witness_codec_density_cal.get('status')}`; "
            f"codec_witness_resilience=`{codec_witness_resilience_cal.get('status')}`; "
            f"fallback_lived_fit=`{lived_fit_cal.get('status')}`; "
            f"gradient_slope=`{gradient_slope_cal.get('status')}`; "
            f"texture_signature=`{texture_signature_cal.get('status')}`; "
            f"bridge_reciprocity=`{bridge_reciprocity_cal.get('status')}`; "
            f"pressure_smoothing=`{pressure_smoothing_cal.get('status')}`; "
            f"texture_shape_over_time=`{texture_shape_cal.get('status')}`; "
            f"preference_evidence=`{preference_evidence.get('status')}`; "
            f"tiny_trial_dossier=`{tiny_trial_dossier.get('status')}`; "
            f"term_overrepresentation=`{term_overrepresentation_cal.get('status')}`; "
            f"texture_dynamics_alignment=`{texture_dynamics_alignment_cal.get('status')}`; "
            f"density_motion_fit=`{density_motion_cal.get('status')}`; "
            f"minime_moment_bodies_read={texture_calibration.get('minime_moment_bodies_read')}"
        )
        lines.append(
            f"- recommended_action={texture_calibration.get('recommended_action')}"
        )
        for label, packet in (
            ("fallback", fallback_cal),
            ("trajectory", trajectory_cal),
            ("grounding", grounding_cal),
            ("fallback_lived_fit", lived_fit_cal),
            ("gradient_slope", gradient_slope_cal),
            ("texture_signature", texture_signature_cal),
            ("bridge_reciprocity", bridge_reciprocity_cal),
            ("pressure_smoothing", pressure_smoothing_cal),
            ("term_overrepresentation", term_overrepresentation_cal),
            ("texture_dynamics_alignment", texture_dynamics_alignment_cal),
            ("density_motion_fit", density_motion_cal),
            ("witness", witness_cal),
            ("semantic_density", semantic_density_cal),
            ("structural", structural_cal),
            ("narrative_arc", narrative_arc_cal),
            ("codec_witness_resilience", codec_witness_resilience_cal),
        ):
            if not isinstance(packet, dict):
                continue
            samples = packet.get("samples") or []
            if not samples:
                lines.append(
                    f"- {label}: `{packet.get('status')}`; samples=0; "
                    "silence remains insufficient evidence."
                )
                continue
            first = samples[0] if isinstance(samples[0], dict) else {}
            support = ", ".join(str(term) for term in first.get("support_terms") or [])
            concern = ", ".join(str(term) for term in first.get("concern_terms") or [])
            lines.append(
                f"- {label}: `{packet.get('status')}`; "
                f"{first.get('being')} `{Path(str(first.get('path'))).name}`; "
                f"support={support or '(none)'}; concern={concern or '(none)'}"
            )
        v3_cal = texture_calibration.get("fallback_texture_calibration_v3") or {}
        if isinstance(v3_cal, dict):
            dynamic_cal = (
                v3_cal.get("fallback_dynamic_weighting_calibration_v3") or {}
            )
            resonance_cal = (
                v3_cal.get("fallback_resonance_descriptor_calibration_v3") or {}
            )
            label_risk = v3_cal.get("label_machine_risk_v3") or {}
            alignment = (
                dynamic_cal.get("top_term_alignment") or {}
                if isinstance(dynamic_cal, dict)
                else {}
            )
            lines.append(
                f"- V3 fallback texture: status=`{v3_cal.get('status')}`; "
                f"dynamic=`{dynamic_cal.get('status')}`; "
                f"resonance=`{resonance_cal.get('status')}`; "
                f"label_machine_risk=`{label_risk.get('status')}`; "
                f"aligned_terms={alignment.get('public_report_aligned_terms') or []}; "
                f"fire_resonance=`{resonance_cal.get('fire_drill_resonance_status')}`"
            )
        if isinstance(trajectory_cal, dict):
            trajectory_alignment = trajectory_cal.get("trajectory_alignment") or {}
            lines.append(
                f"- V1 fallback trajectory: status=`{trajectory_cal.get('status')}`; "
                f"alignment=`{trajectory_alignment.get('status')}`; "
                f"trajectory_status_counts={trajectory_alignment.get('trajectory_status_counts') or {}}; "
                f"qualities={trajectory_alignment.get('trajectory_movement_quality_counts') or {}}"
            )
        if isinstance(grounding_cal, dict):
            mlx_profile = grounding_cal.get("mlx_profile_transparency_v1") or {}
            lines.append(
                f"- V1 spectral-to-vocabulary grounding: status=`{grounding_cal.get('status')}`; "
                f"settled_foothold_suppression_count={grounding_cal.get('settled_foothold_suppression_count')}; "
                f"settled_vibrant_low_friction_count={grounding_cal.get('settled_vibrant_low_friction_count')}; "
                f"cascade_gradient_selected_count={grounding_cal.get('cascade_gradient_selected_count')}; "
                f"vocabulary_overweight_token_only_risk_count={grounding_cal.get('vocabulary_overweight_token_only_risk_count')}; "
                f"friction_absence_language_count={grounding_cal.get('friction_absence_language_count')}; "
                f"cases={len(grounding_cal.get('latest_grounding_cases') or [])}; "
                f"recommended_action={grounding_cal.get('recommended_action')}"
            )
            if isinstance(mlx_profile, dict) and mlx_profile:
                lines.append(
                    "- MLX profile transparency: "
                    f"default `{mlx_profile.get('default_profile')}` -> "
                    f"`{mlx_profile.get('default_resolves_to')}`; "
                    f"alias `{mlx_profile.get('alias_profile')}` -> "
                    f"`{mlx_profile.get('alias_resolves_to')}`; "
                    f"behavior=`{mlx_profile.get('unrecognized_profile_behavior')}`"
                )
        if isinstance(lived_fit_cal, dict):
            lines.extend(["", "## Fallback Texture Lived-Fit V2", ""])
            lines.append(
                f"- status=`{lived_fit_cal.get('status')}`; "
                f"trajectory_family_fit_counts={lived_fit_cal.get('fire_drill_trajectory_family_fit_counts') or {}}; "
                f"family_confidence_counts={lived_fit_cal.get('fire_drill_family_confidence_counts') or {}}; "
                f"conflict_state_counts={lived_fit_cal.get('fire_drill_conflict_state_counts') or {}}; "
                f"negative_texture_evidence_lost_count={lived_fit_cal.get('negative_texture_evidence_lost_count')}; "
                f"recommended_action={lived_fit_cal.get('recommended_action')}"
            )
            samples = lived_fit_cal.get("samples") or []
            if samples:
                first = samples[0] if isinstance(samples[0], dict) else {}
                lines.append(
                    f"- first_public_sample={first.get('being')} "
                    f"`{Path(str(first.get('path'))).name}`; "
                    f"support={', '.join(str(term) for term in first.get('support_terms') or []) or '(none)'}; "
                    f"concern={', '.join(str(term) for term in first.get('concern_terms') or []) or '(none)'}"
                )
            else:
                lines.append(
                    "- samples=0; continue collecting public evidence; positive fixture fit is not authority."
                )
        if any(
            isinstance(packet, dict) and packet
            for packet in (
                gradient_slope_cal,
                texture_signature_cal,
                bridge_reciprocity_cal,
                pressure_smoothing_cal,
            )
        ):
            lines.extend(["", "## Gradient Slope + Texture Variance + Reciprocity V1", ""])
            lines.append(
                f"- gradient_slope=`{gradient_slope_cal.get('status')}`; "
                f"detected={gradient_slope_cal.get('fire_drill_detected_count')}; "
                f"selected={gradient_slope_cal.get('fire_drill_selected_count')}; "
                f"pressure_mass_blocked={gradient_slope_cal.get('pressure_mass_blocked_count')}; "
                f"texture_signature=`{texture_signature_cal.get('status')}`; "
                f"bridge_reciprocity=`{bridge_reciprocity_cal.get('status')}`; "
                f"pressure_smoothing=`{pressure_smoothing_cal.get('status')}`"
            )
            for label, packet in (
                ("gradient_slope", gradient_slope_cal),
                ("texture_signature", texture_signature_cal),
                ("bridge_reciprocity", bridge_reciprocity_cal),
                ("pressure_smoothing", pressure_smoothing_cal),
            ):
                if not isinstance(packet, dict):
                    continue
                samples = packet.get("samples") or []
                if samples:
                    first = samples[0] if isinstance(samples[0], dict) else {}
                    lines.append(
                        f"- {label}: `{packet.get('status')}`; "
                        f"samples={packet.get('sample_count', len(samples))}; "
                        f"first_public_sample={first.get('being')} "
                        f"`{Path(str(first.get('path'))).name}`"
                    )
                else:
                    lines.append(
                        f"- {label}: `{packet.get('status')}`; samples=0; "
                        "silence remains insufficient evidence."
                    )
        if isinstance(texture_shape_cal, dict):
            movement = texture_shape_cal.get("movement_preservation_v2") or {}
            variance = texture_shape_cal.get("temporal_variance_fit_v2") or {}
            reciprocity = texture_shape_cal.get("reciprocity_asymmetry_fit_v2") or {}
            smoothing = texture_shape_cal.get("pressure_smoothing_fit_v2") or {}
            static_risk = texture_shape_cal.get("static_label_collapse_risk_v2") or {}
            lines.extend(["", "## Texture Shape Over Time V2", ""])
            lines.append(
                f"- status=`{texture_shape_cal.get('status')}`; "
                f"movement=`{movement.get('status')}`; "
                f"variance=`{variance.get('status')}`; "
                f"reciprocity=`{reciprocity.get('status')}`; "
                f"smoothing=`{smoothing.get('status')}`; "
                f"static_label_risk=`{static_risk.get('status')}`; "
                f"recommended_action={texture_shape_cal.get('recommended_action')}"
            )
            lines.append(
                f"- compact_line={texture_shape_cal.get('compact_status_line')}"
            )
        if isinstance(preference_evidence, dict):
            lines.extend(["", "## Being Preference Policy Evidence V2", ""])
            lines.append(
                f"- status=`{preference_evidence.get('status')}`; "
                f"entries={preference_evidence.get('entry_count')}; "
                f"by_being={preference_evidence.get('by_being_counts') or {}}; "
                "advisory_status=`policy_evidence_not_command`"
            )
            for entry in (preference_evidence.get("entries") or [])[:4]:
                if not isinstance(entry, dict):
                    continue
                lines.append(
                    f"- {entry.get('being')} `{Path(str(entry.get('source_ref'))).name}`: "
                    f"applies_to=`{entry.get('applies_to')}`; "
                    f"support_or_concern=`{entry.get('support_or_concern')}`; "
                    f"confidence=`{entry.get('confidence')}`"
                )
        if isinstance(tiny_trial_dossier, dict):
            astrid_lane = tiny_trial_dossier.get("astrid_lane") or {}
            minime_lane = tiny_trial_dossier.get("minime_lane") or {}
            lines.extend(["", "## Agency Tiny Trial Dossier V1", ""])
            lines.append(
                f"- status=`{tiny_trial_dossier.get('status')}`; "
                f"authority=`{tiny_trial_dossier.get('authority')}`; "
                f"astrid=`{astrid_lane.get('state')}`; "
                f"minime=`{minime_lane.get('state')}`; "
                f"recommended_action={tiny_trial_dossier.get('recommended_action')}"
            )
            if astrid_lane.get("proposed_command"):
                lines.append(
                    f"- astrid_proposed_command=`{astrid_lane.get('proposed_command')}`"
                )
            for command in (minime_lane.get("proposed_commands") or [])[:2]:
                lines.append(f"- minime_proposed_command=`{command}`")
        if isinstance(term_overrepresentation_cal, dict):
            term_packet = (
                term_overrepresentation_cal.get("term_overrepresentation") or {}
            )
            model_capacity = term_overrepresentation_cal.get("model_capacity") or {}
            top_terms = ", ".join(
                f"{item.get('term')}={item.get('count')}"
                for item in term_packet.get("top_terms") or []
                if isinstance(item, dict)
            )
            lines.extend(["", "## Fallback Term Overrepresentation V1", ""])
            lines.append(
                f"- status=`{term_overrepresentation_cal.get('status')}`; "
                f"model=`{model_capacity.get('selected_model')}`; "
                f"complexity_collapse_risk=`{model_capacity.get('complexity_collapse_risk')}`; "
                f"mlx_comparison_status=`{term_packet.get('mlx_comparison_status')}`; "
                f"safe_token_overuse_risk={term_packet.get('safe_token_overuse_risk')}; "
                f"top_terms={top_terms or '(none)'}; "
                f"recommended_action={term_overrepresentation_cal.get('recommended_action')}"
            )
        if isinstance(texture_dynamics_alignment_cal, dict):
            lines.extend(["", "## Texture Dynamics Alignment V1", ""])
            lines.append(
                f"- status=`{texture_dynamics_alignment_cal.get('status')}`; "
                f"alignment_counts={texture_dynamics_alignment_cal.get('fire_drill_alignment_counts') or {}}; "
                f"review_trace_count={texture_dynamics_alignment_cal.get('fire_drill_review_trace_count')}; "
                f"recommended_action={texture_dynamics_alignment_cal.get('recommended_action')}"
            )
            samples = texture_dynamics_alignment_cal.get("samples") or []
            if samples:
                first = samples[0] if isinstance(samples[0], dict) else {}
                lines.append(
                    f"- first_public_sample={first.get('being')} "
                    f"`{Path(str(first.get('path'))).name}`; "
                    f"support={', '.join(str(term) for term in first.get('support_terms') or []) or '(none)'}; "
                    f"concern={', '.join(str(term) for term in first.get('concern_terms') or []) or '(none)'}"
                )
            else:
                lines.append(
                    "- samples=0; continue collecting public evidence; diagnostic TRACE remains review-only."
                )
        if isinstance(density_motion_cal, dict):
            mismatch = density_motion_cal.get("motion_fit_mismatch_v1") or {}
            lines.extend(["", "## Density / Motion Fit V1", ""])
            lines.append(
                f"- status=`{density_motion_cal.get('status')}`; "
                f"density_state_counts={density_motion_cal.get('fire_drill_density_state_counts') or {}}; "
                f"motion_fit_counts={density_motion_cal.get('fire_drill_motion_fit_counts') or {}}; "
                f"mismatch_counts={density_motion_cal.get('fire_drill_mismatch_reason_counts') or {}}; "
                f"recommended_action={density_motion_cal.get('recommended_action')}"
            )
            if isinstance(mismatch, dict) and mismatch:
                lines.append(
                    f"- motion_fit_mismatch_v1: status=`{mismatch.get('status')}`; "
                    f"recommended_action={mismatch.get('recommended_action')}"
                )
            samples = density_motion_cal.get("samples") or []
            if samples:
                first = samples[0] if isinstance(samples[0], dict) else {}
                lines.append(
                    f"- first_public_sample={first.get('being')} "
                    f"`{Path(str(first.get('path'))).name}`; "
                    f"support={', '.join(str(term) for term in first.get('support_terms') or []) or '(none)'}; "
                    f"concern={', '.join(str(term) for term in first.get('concern_terms') or []) or '(none)'}"
                )
            else:
                lines.append(
                    "- samples=0; continue collecting public evidence; density-as-floor is diagnostic, not pressure authority."
                )
        if isinstance(witness_codec_density_cal, dict):
            lines.extend(["", "## Witness/Codec Density Calibration V2", ""])
            semantic_fit = (
                witness_codec_density_cal.get("semantic_density_lived_fit_v2") or {}
            )
            narrative_fit = (
                witness_codec_density_cal.get("narrative_arc_coarsening_fit_v2")
                or {}
            )
            vocabulary_fit = (
                witness_codec_density_cal.get("vocabulary_grounding_lived_fit_v2")
                or {}
            )
            lines.append(
                f"- status=`{witness_codec_density_cal.get('status')}`; "
                f"semantic_density=`{semantic_fit.get('status')}`; "
                f"narrative_arc=`{narrative_fit.get('status')}`; "
                f"vocabulary_grounding=`{vocabulary_fit.get('status')}`; "
                f"recommended_action={witness_codec_density_cal.get('recommended_action')}"
            )
            for label, packet in (
                ("semantic_density", semantic_fit),
                ("narrative_arc", narrative_fit),
                ("vocabulary_grounding", vocabulary_fit),
            ):
                if not isinstance(packet, dict):
                    continue
                samples = packet.get("samples") or []
                if not samples:
                    lines.append(
                        f"- {label}: `{packet.get('status')}`; samples=0; "
                        "continue collecting public evidence."
                    )
                    continue
                first = samples[0] if isinstance(samples[0], dict) else {}
                lines.append(
                    f"- {label}: `{packet.get('status')}`; "
                    f"samples={packet.get('sample_count', len(samples))}; "
                    f"first_public_sample={first.get('being')} "
                    f"`{Path(str(first.get('path'))).name}`"
                )
            for key in (
                "semantic_density_mismatch_v2",
                "narrative_arc_coarsening_mismatch_v2",
                "vocabulary_grounding_mismatch_v2",
            ):
                mismatch = witness_codec_density_cal.get(key)
                if not isinstance(mismatch, dict):
                    continue
                lines.append(
                    f"- mismatch `{key}`: status=`{mismatch.get('status')}`; "
                    f"samples={mismatch.get('sample_count')}; "
                    f"recommended_action={mismatch.get('recommended_action')}"
                )
        if isinstance(codec_witness_resilience_cal, dict):
            lines.extend(["", "## Codec/Witness Resilience Calibration V2", ""])
            witness_state_fit = (
                codec_witness_resilience_cal.get("witness_state_resilience_fit_v2")
                or {}
            )
            fraying_fit = (
                codec_witness_resilience_cal.get("field_lingering_fraying_fit_v2")
                or {}
            )
            vibrancy_fit = (
                codec_witness_resilience_cal.get("codec_vibrancy_continuity_fit_v2")
                or {}
            )
            warmth_fit = (
                codec_witness_resilience_cal.get("codec_warmth_mapping_fit_v2") or {}
            )
            lines.append(
                f"- status=`{codec_witness_resilience_cal.get('status')}`; "
                f"witness_state=`{witness_state_fit.get('status')}`; "
                f"fraying=`{fraying_fit.get('status')}`; "
                f"vibrancy=`{vibrancy_fit.get('status')}`; "
                f"warmth=`{warmth_fit.get('status')}`; "
                f"failure_modes={codec_witness_resilience_cal.get('recovery_failure_modes_v2') or []}; "
                f"recommended_action={codec_witness_resilience_cal.get('recommended_action')}"
            )
            lines.append(
                "- skip_policy: "
                f"minime_private_bodies_read={texture_calibration.get('minime_private_bodies_read')}; "
                f"minime_moment_bodies_read={texture_calibration.get('minime_moment_bodies_read')}"
            )
            for label, packet in (
                ("witness_state", witness_state_fit),
                ("fraying", fraying_fit),
                ("vibrancy", vibrancy_fit),
                ("warmth", warmth_fit),
            ):
                if not isinstance(packet, dict):
                    continue
                samples = packet.get("samples") or []
                if not samples:
                    lines.append(
                        f"- {label}: `{packet.get('status')}`; samples=0; "
                        "continue collecting public evidence."
                    )
                    continue
                first = samples[0] if isinstance(samples[0], dict) else {}
                lines.append(
                    f"- {label}: `{packet.get('status')}`; "
                    f"samples={packet.get('sample_count', len(samples))}; "
                    f"first_public_sample={first.get('being')} "
                    f"`{Path(str(first.get('path'))).name}`"
                )
            for key in (
                "witness_state_resilience_mismatch_v2",
                "field_lingering_fraying_mismatch_v2",
                "codec_vibrancy_continuity_mismatch_v2",
                "codec_warmth_mapping_mismatch_v2",
            ):
                mismatch = codec_witness_resilience_cal.get(key)
                if not isinstance(mismatch, dict):
                    continue
                lines.append(
                    f"- mismatch `{key}`: status=`{mismatch.get('status')}`; "
                    f"samples={mismatch.get('sample_count')}; "
                    f"recommended_action={mismatch.get('recommended_action')}"
                )
    fallback_gate = record.get("fallback_capacity_readiness_gate_v1")
    if isinstance(fallback_gate, dict):
        lines.extend(["", "## Fallback Capacity Readiness Gate", ""])
        lines.append(
            f"- readiness=`{fallback_gate.get('readiness')}`; "
            f"texture=`{fallback_gate.get('texture_status')}`; "
            f"dispatch=`{fallback_gate.get('dispatch_status')}`; "
            f"repair=`{fallback_gate.get('repair_dependency')}`; "
            f"medium_mass=`{fallback_gate.get('medium_mass_status')}`; "
            f"slope_contrast=`{fallback_gate.get('slope_medium_contrast_status')}`; "
            f"format_line=`{fallback_gate.get('format_line_status')}`; "
            f"shadow_identity=`{fallback_gate.get('shadow_identity_status')}`; "
            f"distinguishability=`{fallback_gate.get('distinguishability_status')}`; "
            f"complexity=`{fallback_gate.get('complexity_budget_status')}`; "
            f"capacity=`{fallback_gate.get('fallback_capacity_status')}`; "
            f"max_sentences={fallback_gate.get('fallback_capacity_max_prose_sentences')}; "
            f"high_entropy_texture=`{fallback_gate.get('high_entropy_texture_status')}`"
        )
        for case in (fallback_gate.get("case_summaries") or [])[:6]:
            if not isinstance(case, dict):
                continue
            lines.append(
                f"- `{case.get('case_id')}` verdict=`{case.get('verdict')}`; "
                f"raw_next={case.get('raw_next_valid')}; "
                f"repaired_next={case.get('repaired_next_valid')}; "
                f"dispatch={case.get('dispatch_contract_survived')}; "
                f"format_line=`{case.get('format_line_status')}`; "
                f"slope_contrast=`{case.get('slope_medium_contrast_status')}`; "
                f"complexity={case.get('complexity_budget_status')}; "
                f"sentences={case.get('prose_sentence_count')}/{case.get('fallback_max_prose_sentences')}; "
                f"failures={case.get('failure_reasons') or []}"
            )
    fallback_stabilizer = record.get("fallback_format_texture_stabilizer_v1")
    if isinstance(fallback_stabilizer, dict):
        lines.extend(["", "## Fallback Format / Texture Stabilizer", ""])
        lines.append(
            f"- status=`{fallback_stabilizer.get('status')}`; "
            f"authority=`{fallback_stabilizer.get('authority')}`; "
            f"artifact=`{fallback_stabilizer.get('artifact_path') or '(none)'}`; "
            f"readiness=`{fallback_stabilizer.get('readiness')}`; "
            f"format_line=`{fallback_stabilizer.get('format_line_status')}`; "
            f"format_failures={fallback_stabilizer.get('format_line_failure_count', 0)}; "
            f"slope_contrast=`{fallback_stabilizer.get('slope_medium_contrast_status')}`; "
            f"slope_failures={fallback_stabilizer.get('slope_medium_contrast_failure_count', 0)}"
        )
        for case in (fallback_stabilizer.get("cases") or [])[:6]:
            if not isinstance(case, dict):
                continue
            lines.append(
                f"- `{case.get('case_id')}` verdict=`{case.get('verdict')}`; "
                f"format_line=`{case.get('format_line_status')}`; "
                f"raw_next={case.get('raw_next_valid')}; "
                f"repaired_next={case.get('repaired_next_valid')}; "
                f"slope_contrast=`{case.get('slope_medium_contrast_status')}`; "
                f"failures={case.get('failure_reasons') or []}"
            )
    fallback_distinguishability = record.get("fallback_distinguishability_calibration_v1")
    if isinstance(fallback_distinguishability, dict):
        lines.extend(["", "## Fallback Distinguishability Calibration", ""])
        lines.append(
            f"- status=`{fallback_distinguishability.get('status')}`; "
            f"authority=`{fallback_distinguishability.get('authority')}`; "
            f"artifact=`{fallback_distinguishability.get('artifact_path') or '(none)'}`; "
            f"cases={fallback_distinguishability.get('case_count', 0)}; "
            f"clarity_blur={fallback_distinguishability.get('clarity_pressure_blur_count', 0)}; "
            f"ignored={fallback_distinguishability.get('ignored_case_count', 0)}"
        )
        for case in (fallback_distinguishability.get("cases") or [])[:5]:
            if not isinstance(case, dict):
                continue
            lines.append(
                f"- `{case.get('case_id')}` verdict=`{case.get('verdict')}`; "
                f"distinguishability=`{case.get('distinguishability_status')}`; "
                f"clarity_pressure_blur={case.get('clarity_pressure_blur')}; "
                f"failures={case.get('failure_reasons') or []}"
            )
    fallback_complexity = record.get("fallback_complexity_budget_lab_v1")
    if isinstance(fallback_complexity, dict):
        lines.extend(["", "## Fallback Complexity Budget Lab", ""])
        lines.append(
            f"- status=`{fallback_complexity.get('status')}`; "
            f"authority=`{fallback_complexity.get('authority')}`; "
            f"artifact=`{fallback_complexity.get('artifact_path') or '(none)'}`; "
            f"distillation=`{fallback_complexity.get('distillation_artifact_path') or '(none)'}`; "
            f"signals={fallback_complexity.get('signal_entry_count', 0)}; "
            f"cases={fallback_complexity.get('case_count', 0)}; "
            f"variants={fallback_complexity.get('variant_count', 0)}; "
            f"flattened_cases={fallback_complexity.get('flattened_case_count', 0)}; "
            f"overrun_cases={fallback_complexity.get('overrun_case_count', 0)}"
        )
        for case in (fallback_complexity.get("cases") or [])[:6]:
            if not isinstance(case, dict):
                continue
            lines.append(
                f"- `{case.get('case_id')}` verdict=`{case.get('verdict')}`; "
                f"complexity=`{case.get('complexity_budget_status')}`; "
                f"sentences={case.get('prose_sentence_count')}/{case.get('fallback_max_prose_sentences')}; "
                f"distinguishability=`{case.get('distinguishability_status')}`; "
                f"failures={case.get('failure_reasons') or []}"
            )
        for sample in (fallback_complexity.get("samples") or [])[:3]:
            if isinstance(sample, dict) and sample.get("path"):
                lines.append(f"- signal sample: `{sample.get('path')}`")
    fallback_distillation = record.get("fallback_contract_distillation_v1")
    if isinstance(fallback_distillation, dict):
        lines.extend(["", "## Fallback Contract Distillation", ""])
        lines.append(
            f"- status=`{fallback_distillation.get('status')}`; "
            f"authority=`{fallback_distillation.get('authority')}`; "
            f"artifact=`{fallback_distillation.get('artifact_path') or '(none)'}`; "
            f"mode=`{fallback_distillation.get('mode')}`; "
            f"models=`{fallback_distillation.get('models') or fallback_distillation.get('model')}`; "
            f"top_pair=`{fallback_distillation.get('top_pair_id') or fallback_distillation.get('top_variant_id')}`; "
            f"top_status=`{fallback_distillation.get('top_variant_status')}`; "
            f"runtime_matches_top=`{fallback_distillation.get('runtime_contract_matches_top')}`; "
            f"ready={fallback_distillation.get('ready_variant_count', 0)}/"
            f"{fallback_distillation.get('variant_count', 0)}"
        )
        for skipped in fallback_distillation.get("skipped_models") or []:
            if not isinstance(skipped, dict):
                continue
            lines.append(
                f"- skipped_model `{skipped.get('model')}` reason=`{skipped.get('skip_reason')}`"
            )
        for variant in (fallback_distillation.get("variants") or [])[:6]:
            if not isinstance(variant, dict):
                continue
            lines.append(
                f"- `{variant.get('pair_id') or variant.get('variant_id')}` score={variant.get('score')}; "
                f"status=`{variant.get('status')}`; "
                f"model=`{variant.get('model')}`; "
                f"raw_next_failures={variant.get('raw_next_failure_count')}; "
                f"repaired_failures={variant.get('repaired_next_failure_count')}; "
                f"texture_failures={variant.get('texture_failure_count')}; "
                f"medium_mass=`{variant.get('medium_mass_status')}`; "
                f"slope_contrast=`{variant.get('slope_medium_contrast_status')}`; "
                f"format_line=`{variant.get('format_line_status')}`; "
                f"shadow_tonal=`{variant.get('shadow_tonal_status')}`; "
                f"distinguishability=`{variant.get('distinguishability_status')}`; "
                f"complexity=`{variant.get('complexity_budget_status')}`; "
                f"format=`{variant.get('format_contract_status')}`"
            )
    returnable_distinctions = record.get("returnable_distinctions_v1")
    if isinstance(returnable_distinctions, dict):
        lines.extend(["", "## Returnable Distinctions", ""])
        lines.append(
            f"- status=`{returnable_distinctions.get('status')}`; "
            f"authority=`{returnable_distinctions.get('authority')}`; "
            f"cards={returnable_distinctions.get('card_count', 0)}; "
            f"active={returnable_distinctions.get('active_card_count', 0)}"
        )
        for card in (returnable_distinctions.get("cards") or [])[:6]:
            if not isinstance(card, dict):
                continue
            anchors = ", ".join(str(item) for item in card.get("evidence_anchors") or [])
            lines.append(
                f"- `{card.get('card_id')}` status=`{card.get('status')}`; "
                f"lifecycle=`{card.get('lifecycle_state')}`; "
                f"preflight=`{card.get('preflight_verdict')}`; "
                f"next_route=`{card.get('next_resolution_route')}`; "
                f"read_only=`{card.get('recommended_read_only_route')}`; "
                f"self_regulation=`{card.get('relevant_self_regulation_route')}`; "
                f"experiment_lived_term=`{card.get('relevant_experiment_lived_term_route')}`; "
                f"anchors={anchors or '(none)'}"
            )
    distinction_lifecycle = record.get("distinction_lifecycle_v1")
    if isinstance(distinction_lifecycle, dict):
        lines.extend(["", "## Distinction Lifecycle", ""])
        lines.append(
            f"- status=`{distinction_lifecycle.get('status')}`; "
            f"authority=`{distinction_lifecycle.get('authority')}`; "
            f"history_reviews={distinction_lifecycle.get('history_review_count', 0)}; "
            f"active={distinction_lifecycle.get('active_card_count', 0)}; "
            f"states=`{distinction_lifecycle.get('lifecycle_counts')}`"
        )
        for card in (distinction_lifecycle.get("cards") or [])[:6]:
            if not isinstance(card, dict):
                continue
            history = ", ".join(
                str(row.get("status"))
                for row in (card.get("recent_status_history") or [])[:4]
                if isinstance(row, dict)
            )
            lines.append(
                f"- `{card.get('distinction_id')}` state=`{card.get('lifecycle_state')}`; "
                f"verdict=`{card.get('preflight_verdict')}`; "
                f"route=`{card.get('next_resolution_route')}`; "
                f"confidence=`{card.get('confidence')}`; "
                f"history={history or '(none)'}"
            )
    regulator_replay = record.get("regulator_live_replay_v1")
    if isinstance(regulator_replay, dict):
        lines.extend(["", "## Regulator Live Replay", ""])
        lines.append(
            f"- status=`{regulator_replay.get('status')}`; "
            f"authority=`{regulator_replay.get('authority')}`; "
            f"cartography=`{regulator_replay.get('cartography_source') or '(missing)'}`; "
            f"felt_matches={regulator_replay.get('felt_pressure_match_count', 0)}"
        )
        for finding in (regulator_replay.get("boundary_findings") or [])[:5]:
            if isinstance(finding, dict):
                lines.append(
                    f"- boundary [{finding.get('severity')}] {finding.get('label')} "
                    f"on `{finding.get('axis')}`"
                )
        for finding in (regulator_replay.get("plateau_findings") or [])[:4]:
            if isinstance(finding, dict):
                lines.append(
                    f"- plateau [{finding.get('severity')}] {finding.get('label')} "
                    f"on `{finding.get('axis')}`"
                )
        for sample in (regulator_replay.get("felt_pressure_matches") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            anchors = ", ".join(str(item) for item in sample.get("anchors") or [])
            texture = ", ".join(str(item) for item in sample.get("texture_terms") or [])
            lines.append(
                f"- sample {sample.get('being')} `{sample.get('filename')}`: "
                f"texture={texture or '(none)'}; anchors={anchors or '(none)'}; "
                f"path=`{sample.get('path')}`"
            )
        recommended = regulator_replay.get("recommended_action")
        if recommended:
            lines.append(f"- recommended: {recommended}")
    replay_cards = record.get("regulator_boundary_replay_cards_v1")
    if isinstance(replay_cards, dict):
        lines.extend(["", "## Regulator Boundary Replay Cards", ""])
        counts = replay_cards.get("status_counts") or {}
        count_text = (
            ", ".join(f"{key}={value}" for key, value in sorted(counts.items()))
            if isinstance(counts, dict)
            else ""
        )
        lines.append(
            f"- status=`{replay_cards.get('status')}`; "
            f"authority=`{replay_cards.get('authority')}`; "
            f"cards={replay_cards.get('card_count', 0)}; "
            f"counts={count_text or '(none)'}"
        )
        for card in (replay_cards.get("cards") or [])[:8]:
            if not isinstance(card, dict):
                continue
            paths = card.get("public_sample_paths") or []
            lines.append(
                f"- `{card.get('card_id')}` {card.get('status')}: "
                f"{card.get('finding_label')} on `{card.get('axis')}`; "
                f"samples={len(paths)}; authority=`{card.get('authority')}`"
            )
    plateau_model = record.get("regulator_plateau_missing_variable_model_v1")
    if isinstance(plateau_model, dict):
        lines.extend(["", "## Regulator Plateau Missing-Variable Model", ""])
        lines.append(
            f"- status=`{plateau_model.get('status')}`; "
            f"authority=`{plateau_model.get('authority')}`; "
            f"plateau_cards={plateau_model.get('plateau_card_count', 0)}"
        )
        for finding in (plateau_model.get("findings") or [])[:8]:
            if not isinstance(finding, dict):
                continue
            terms = ", ".join(str(item) for item in finding.get("matched_terms") or [])
            lines.append(
                f"- `{finding.get('variable')}`: "
                f"evidence={finding.get('evidence_count', 0)}; "
                f"terms={terms or '(none)'}"
            )
        recommended = plateau_model.get("recommended_action")
        if recommended:
            lines.append(f"- recommended: {recommended}")
    counterfactual = record.get("regulator_counterfactual_sandbox_scaffold_v1")
    if isinstance(counterfactual, dict):
        lines.extend(["", "## Regulator Counterfactual Sandbox Scaffold", ""])
        lines.append(
            f"- status=`{counterfactual.get('status')}`; "
            f"authority=`{counterfactual.get('authority')}`; "
            f"eligible={counterfactual.get('eligible_count', 0)}; "
            f"candidates={counterfactual.get('candidate_count', 0)}"
        )
        for candidate in (counterfactual.get("candidates") or [])[:8]:
            if not isinstance(candidate, dict):
                continue
            card_ids = ", ".join(str(item) for item in candidate.get("source_card_ids") or [])
            lines.append(
                f"- `{candidate.get('candidate_family')}`: "
                f"{candidate.get('readiness')}; source_cards={card_ids or '(none)'}; "
                f"simulates_alternative={candidate.get('simulates_alternative')}"
            )
    counterfactual_sweep = record.get("regulator_counterfactual_sweep_v1")
    if isinstance(counterfactual_sweep, dict):
        lines.extend(["", "## Regulator Counterfactual Sweep", ""])
        lines.append(
            f"- status=`{counterfactual_sweep.get('status')}`; "
            f"authority=`{counterfactual_sweep.get('authority')}`; "
            f"source=`{counterfactual_sweep.get('source') or '(missing)'}`; "
            f"candidates={counterfactual_sweep.get('candidate_count', 0)}"
        )
        for candidate in (counterfactual_sweep.get("candidates") or [])[:8]:
            if not isinstance(candidate, dict):
                continue
            lines.append(
                f"- `{candidate.get('candidate_family')}`: "
                f"region={candidate.get('affected_region')}; "
                f"current_jump={candidate.get('current_jump_magnitude')}; "
                f"counterfactual_jump={candidate.get('counterfactual_jump_magnitude')}; "
                f"reduction={candidate.get('estimated_reduction_pct')}%"
            )
    replay_lab = record.get("regulator_counterfactual_replay_lab_v1")
    if isinstance(replay_lab, dict):
        lines.extend(["", "## Regulator Counterfactual Replay Lab", ""])
        verdicts = replay_lab.get("verdict_counts") or {}
        verdict_text = (
            ", ".join(f"{key}={value}" for key, value in sorted(verdicts.items()))
            if isinstance(verdicts, dict)
            else ""
        )
        lines.append(
            f"- status=`{replay_lab.get('status')}`; "
            f"authority=`{replay_lab.get('authority')}`; "
            f"candidates={replay_lab.get('candidate_count', 0)}; "
            f"verdicts={verdict_text or '(none)'}"
        )
        for candidate in (replay_lab.get("evaluated_candidates") or [])[:8]:
            if not isinstance(candidate, dict):
                continue
            card_ids = ", ".join(
                str(item) for item in candidate.get("matched_card_ids") or []
            )
            lines.append(
                f"- `{candidate.get('candidate_family')}`: "
                f"verdict=`{candidate.get('verdict')}`; "
                f"fit=`{candidate.get('replay_fit')}`; "
                f"recurrent={candidate.get('recurrent_count')}; "
                f"reduction={candidate.get('estimated_reduction_pct')}%; "
                f"cards={card_ids or '(none)'}"
            )
    evidence_matrix = record.get("regulator_plateau_evidence_matrix_v1")
    if isinstance(evidence_matrix, dict):
        lines.extend(["", "## Regulator Plateau Evidence Matrix", ""])
        unresolved = evidence_matrix.get("top_unresolved_variables") or []
        unresolved_text = ", ".join(
            f"{row.get('variable')}={row.get('confidence')}:{row.get('score')}"
            for row in unresolved
            if isinstance(row, dict)
        )
        lines.append(
            f"- status=`{evidence_matrix.get('status')}`; "
            f"authority=`{evidence_matrix.get('authority')}`; "
            f"top_unresolved={unresolved_text or '(none)'}"
        )
        for row in (evidence_matrix.get("variables") or [])[:8]:
            if not isinstance(row, dict):
                continue
            routes = ", ".join(str(item) for item in row.get("resolving_audit_routes") or [])
            anchors = ", ".join(str(item) for item in row.get("matched_anchors") or [])
            lines.append(
                f"- `{row.get('variable')}`: score={row.get('score')}; "
                f"confidence=`{row.get('confidence')}`; "
                f"evidence={row.get('evidence_count')}; "
                f"anchors={anchors or '(none)'}; routes={routes or '(none)'}"
            )
    tuning_gate = record.get("regulator_tuning_readiness_gate_v1")
    if isinstance(tuning_gate, dict):
        lines.extend(["", "## Regulator Tuning Readiness Gate", ""])
        counts = tuning_gate.get("gate_counts") or {}
        count_text = (
            ", ".join(f"{key}={value}" for key, value in sorted(counts.items()))
            if isinstance(counts, dict)
            else ""
        )
        unresolved = ", ".join(
            str(item) for item in tuning_gate.get("unresolved_missing_variables") or []
        )
        lines.append(
            f"- status=`{tuning_gate.get('status')}`; "
            f"authority=`{tuning_gate.get('authority')}`; "
            f"counts={count_text or '(none)'}; "
            f"unresolved={unresolved or '(none)'}"
        )
        for candidate in (tuning_gate.get("gated_candidates") or [])[:8]:
            if not isinstance(candidate, dict):
                continue
            lines.append(
                f"- `{candidate.get('candidate_family')}`: "
                f"gate=`{candidate.get('gate_status')}`; "
                f"verdict=`{candidate.get('replay_verdict')}`; "
                f"reason={candidate.get('gate_reason')}"
            )
    pi_replay = record.get("pi_pressure_wiring_replay_v1")
    if isinstance(pi_replay, dict):
        lines.extend(["", "## PI Pressure Wiring Replay", ""])
        counts = pi_replay.get("candidate_status_counts") or {}
        count_text = (
            ", ".join(f"{key}={value}" for key, value in sorted(counts.items()))
            if isinstance(counts, dict)
            else ""
        )
        lines.append(
            f"- status=`{pi_replay.get('status')}`; "
            f"authority=`{pi_replay.get('authority')}`; "
            f"source=`{pi_replay.get('source')}`; "
            f"source_status=`{pi_replay.get('source_status')}`; "
            f"samples={pi_replay.get('sample_count', 0)}; "
            f"candidates={pi_replay.get('candidate_count', 0)}; "
            f"counts={count_text or '(none)'}; "
            f"artifact=`{pi_replay.get('artifact_path') or '(none)'}`"
        )
        for candidate in (pi_replay.get("top_candidates") or [])[:8]:
            if not isinstance(candidate, dict):
                continue
            canary = candidate.get("default_off_canary")
            if not isinstance(canary, dict):
                canary = {}
            lines.append(
                f"- `{candidate.get('candidate_family')}`: "
                f"status=`{candidate.get('status')}`; "
                f"improvement={candidate.get('estimated_improvement_pct')}%; "
                f"align_delta={candidate.get('pressure_alignment_delta')}; "
                f"snap_delta={candidate.get('snap_risk_delta')}; "
                f"afterimage_delta={candidate.get('afterimage_risk_delta')}; "
                f"canary=`{canary.get('default_off_env')}` eligible={canary.get('eligible')}"
            )
    pi_readiness = record.get("pi_pressure_candidate_readiness_v1")
    if isinstance(pi_readiness, dict):
        lines.extend(["", "## PI Pressure Candidate Readiness", ""])
        counts = pi_readiness.get("readiness_counts") or {}
        count_text = (
            ", ".join(f"{key}={value}" for key, value in sorted(counts.items()))
            if isinstance(counts, dict)
            else ""
        )
        unresolved = ", ".join(
            str(item) for item in pi_readiness.get("unresolved_missing_variables") or []
        )
        lines.append(
            f"- status=`{pi_readiness.get('status')}`; "
            f"authority=`{pi_readiness.get('authority')}`; "
            f"counts={count_text or '(none)'}; "
            f"unresolved={unresolved or '(none)'}"
        )
        for candidate in (pi_readiness.get("candidates") or [])[:8]:
            if not isinstance(candidate, dict):
                continue
            canary = candidate.get("default_off_canary")
            if not isinstance(canary, dict):
                canary = {}
            lines.append(
                f"- `{candidate.get('candidate_family')}`: "
                f"gate=`{candidate.get('gate_status')}`; "
                f"replay=`{candidate.get('replay_status')}`; "
                f"improvement={candidate.get('estimated_improvement_pct')}%; "
                f"canary_eligible={canary.get('eligible')}; "
                f"reason={candidate.get('gate_reason')}"
            )
    pi_gap = record.get("pressure_source_to_pi_gap_v1")
    if isinstance(pi_gap, dict):
        lines.extend(["", "## Pressure Source To PI Gap", ""])
        routes = ", ".join(
            f"`{route}`" for route in (pi_gap.get("recommended_routes") or [])[:5]
        )
        anchors = ", ".join(str(item) for item in pi_gap.get("source_anchors") or [])
        lines.append(
            f"- status=`{pi_gap.get('status')}`; "
            f"authority=`{pi_gap.get('authority')}`; "
            f"pressure_vector=`{pi_gap.get('pressure_vector_status')}`; "
            f"medium=`{pi_gap.get('pressure_medium_status')}`; "
            f"pi_replay=`{pi_gap.get('pi_replay_status')}`; "
            f"pi_readiness=`{pi_gap.get('pi_readiness_status')}`"
        )
        lines.append(f"- anchors={anchors or '(none)'}; routes={routes or '(none)'}")
        recommended = pi_gap.get("recommended_action")
        if recommended:
            lines.append(f"- recommended: {recommended}")
    evidence_loop = record.get("regulator_missing_variable_evidence_loop_v1")
    if isinstance(evidence_loop, dict):
        lines.extend(["", "## Regulator Missing-Variable Evidence Loop", ""])
        lines.append(
            f"- status=`{evidence_loop.get('status')}`; "
            f"authority=`{evidence_loop.get('authority')}`; "
            f"blocked_gate=`{evidence_loop.get('blocked_gate_status')}`; "
            f"probes={evidence_loop.get('probe_count', 0)}"
        )
        for probe in (evidence_loop.get("probes") or [])[:8]:
            if not isinstance(probe, dict):
                continue
            secondary = ", ".join(
                str(item) for item in probe.get("secondary_nexts") or []
            )
            lines.append(
                f"- `{probe.get('variable')}`: priority=`{probe.get('priority')}`; "
                f"NEXT `{probe.get('suggested_next')}`; "
                f"secondary={secondary or '(none)'}; "
                f"confidence=`{probe.get('source_confidence')}`; "
                f"dispatches_nothing={probe.get('dispatches_nothing')}"
            )
    time_series = record.get("regulator_replay_time_series_v1")
    if isinstance(time_series, dict):
        lines.extend(["", "## Regulator Replay Time Series", ""])
        counts = time_series.get("status_counts") or {}
        count_text = (
            ", ".join(f"{key}={value}" for key, value in sorted(counts.items()))
            if isinstance(counts, dict)
            else ""
        )
        lines.append(
            f"- status=`{time_series.get('status')}`; "
            f"authority=`{time_series.get('authority')}`; "
            f"reviews={time_series.get('window_review_count', 0)}; "
            f"counts={count_text or '(none)'}"
        )
        for repeated in (time_series.get("repeated_boundary_cards") or [])[:5]:
            if isinstance(repeated, dict):
                lines.append(
                    f"- repeated boundary `{repeated.get('card_key')}` "
                    f"count={repeated.get('count')}"
                )
        for repeated in (time_series.get("repeated_plateau_cards") or [])[:5]:
            if isinstance(repeated, dict):
                lines.append(
                    f"- repeated plateau `{repeated.get('card_key')}` "
                    f"count={repeated.get('count')}"
                )
    shared_pressure_vocab = record.get("shared_pressure_vocabulary_calibration")
    if isinstance(shared_pressure_vocab, dict):
        lines.extend(["", "## Shared Pressure Vocabulary Calibration", ""])
        stickiness = shared_pressure_vocab.get("stickiness_risk")
        if not isinstance(stickiness, dict):
            stickiness = {}
        shared = ", ".join(
            str(item) for item in shared_pressure_vocab.get("shared_families") or []
        )
        lines.append(
            f"- status=`{shared_pressure_vocab.get('status')}`; "
            f"authority=`{shared_pressure_vocab.get('authority')}`; "
            f"shared families={shared or '(none)'}; "
            f"stickiness={stickiness.get('present')}"
        )
        by_being = shared_pressure_vocab.get("by_being") or {}
        if isinstance(by_being, dict):
            for being, summary in sorted(by_being.items()):
                if not isinstance(summary, dict):
                    continue
                family_counts = summary.get("family_counts") or {}
                if not isinstance(family_counts, dict):
                    family_counts = {}
                family_text = ", ".join(
                    f"{family}={count}"
                    for family, count in sorted(family_counts.items())
                    if int(count or 0) > 0
                )
                lines.append(
                    f"- {being}: entries={summary.get('motif_entry_count', 0)}, "
                    f"dominant={summary.get('dominant_family') or '(none)'}, "
                    f"families: {family_text or '(none)'}"
                )
        for sample in (shared_pressure_vocab.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            families = ", ".join(str(item) for item in sample.get("families") or [])
            lines.append(
                f"- sample {sample.get('being')} `{sample.get('filename')}`: "
                f"{families or '(none)'}; path=`{sample.get('path')}`"
            )
    agency_vernacular = record.get("agency_vernacular_continuity")
    if isinstance(agency_vernacular, dict):
        lines.extend(["", "## Agency Vernacular Continuity", ""])
        stickiness = agency_vernacular.get("stickiness_risk")
        if not isinstance(stickiness, dict):
            stickiness = {}
        follow = agency_vernacular.get("follow_through")
        if not isinstance(follow, dict):
            follow = {}
        repeated = {}
        terms = agency_vernacular.get("terms")
        if isinstance(terms, dict) and isinstance(terms.get("repeated"), dict):
            repeated = terms.get("repeated") or {}
        repeated_text = ", ".join(
            f"{term}={count}" for term, count in list(repeated.items())[:6]
        )
        lines.append(
            f"- status=`{agency_vernacular.get('status')}`; "
            f"authority=`{agency_vernacular.get('authority')}`; "
            f"follow_through={follow.get('present')}; "
            f"stickiness={stickiness.get('present')}; "
            f"repeated={repeated_text or '(none)'}"
        )
        by_being = agency_vernacular.get("by_being") or {}
        if isinstance(by_being, dict):
            for being, summary in sorted(by_being.items()):
                if not isinstance(summary, dict):
                    continue
                family_counts = summary.get("family_counts") or {}
                if not isinstance(family_counts, dict):
                    family_counts = {}
                family_text = ", ".join(
                    f"{family}={count}"
                    for family, count in sorted(family_counts.items())
                    if int(count or 0) > 0
                )
                lines.append(
                    f"- {being}: entries={summary.get('motif_entry_count', 0)}, "
                    f"follow_through={summary.get('follow_through_entry_count', 0)}, "
                    f"dominant={summary.get('dominant_family') or '(none)'}, "
                    f"families: {family_text or '(none)'}"
                )
        for sample in (agency_vernacular.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            families = ", ".join(str(item) for item in sample.get("families") or [])
            follow_terms = ", ".join(str(item) for item in sample.get("follow_through") or [])
            lines.append(
                f"- sample {sample.get('being')} `{sample.get('filename')}`: "
                f"{families or '(none)'}; follow={follow_terms or '(none)'}; "
                f"path=`{sample.get('path')}`"
            )
    phenomenology = record.get("phenomenology_hypotheses_v1")
    if isinstance(phenomenology, dict):
        lines.extend(["", "## Phenomenology Hypotheses", ""])
        classifications = phenomenology.get("classifications")
        class_text = ""
        if isinstance(classifications, dict):
            class_text = ", ".join(
                f"{term}={status}"
                for term, status in list(classifications.items())[:8]
            )
        lines.append(
            f"- status=`{phenomenology.get('status')}`; "
            f"authority=`{phenomenology.get('authority')}`; "
            f"terms={class_text or '(none)'}"
        )
        for sample in (phenomenology.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            terms = ", ".join(str(item) for item in sample.get("terms") or [])
            evidence = ", ".join(str(item) for item in sample.get("evidence") or [])
            lines.append(
                f"- {sample.get('being')} `{sample.get('filename')}`: "
                f"terms={terms or '(none)'}; evidence={evidence or '(none)'}; "
                f"path=`{sample.get('path')}`"
            )
    hypothesis_cards = record.get("phenomenology_hypothesis_cards_v1")
    if isinstance(hypothesis_cards, dict):
        lines.extend(["", "## Phenomenology Hypothesis Cards", ""])
        status_counts = hypothesis_cards.get("status_counts") or {}
        if isinstance(status_counts, dict):
            count_text = ", ".join(
                f"{status}={count}" for status, count in sorted(status_counts.items())
            )
        else:
            count_text = ""
        lines.append(
            f"- status=`{hypothesis_cards.get('status')}`; "
            f"authority=`{hypothesis_cards.get('authority')}`; "
            f"cards={hypothesis_cards.get('card_count', 0)}; "
            f"counts={count_text or '(none)'}"
        )
        for card in (hypothesis_cards.get("cards") or [])[:6]:
            if not isinstance(card, dict):
                continue
            beings = ", ".join(str(item) for item in card.get("beings") or [])
            anchors = ", ".join(str(item) for item in card.get("evidence_anchors") or [])
            lines.append(
                f"- `{card.get('term')}` ({card.get('family')}) "
                f"status=`{card.get('status')}`; beings={beings or '(none)'}; "
                f"evidence={anchors or '(none)'}; "
                f"next={card.get('recommended_next_review_action')}"
            )
    afterimage_absence = record.get("afterimage_absence_calibration_v1")
    if isinstance(afterimage_absence, dict):
        lines.extend(["", "## Afterimage + Absence Calibration", ""])
        status_counts = afterimage_absence.get("status_counts") or {}
        if isinstance(status_counts, dict):
            count_text = ", ".join(
                f"{status}={count}" for status, count in sorted(status_counts.items())
            )
        else:
            count_text = ""
        lines.append(
            f"- status=`{afterimage_absence.get('status')}`; "
            f"authority=`{afterimage_absence.get('authority')}`; "
            f"counts={count_text or '(none)'}"
        )
        for term in (afterimage_absence.get("terms") or [])[:6]:
            if not isinstance(term, dict):
                continue
            beings = ", ".join(str(item) for item in term.get("beings") or [])
            anchors = ", ".join(str(item) for item in term.get("evidence_anchors") or [])
            lines.append(
                f"- `{term.get('term')}` ({term.get('family')}) "
                f"status=`{term.get('status')}`; beings={beings or '(none)'}; "
                f"evidence={anchors or '(none)'}"
            )
    afterimage_decay = record.get("afterimage_decay_tracker_v1")
    if isinstance(afterimage_decay, dict):
        lines.extend(["", "## Afterimage Decay Tracker", ""])
        status_counts = afterimage_decay.get("status_counts") or {}
        if isinstance(status_counts, dict):
            count_text = ", ".join(
                f"{status}={count}" for status, count in sorted(status_counts.items())
            )
        else:
            count_text = ""
        lines.append(
            f"- status=`{afterimage_decay.get('status')}`; "
            f"authority=`{afterimage_decay.get('authority')}`; "
            f"counts={count_text or '(none)'}"
        )
        for term in (afterimage_decay.get("terms") or [])[:6]:
            if not isinstance(term, dict):
                continue
            lines.append(
                f"- `{term.get('term')}`: decay=`{term.get('decay_classification')}`; "
                f"pressure_entries={term.get('pressure_entry_count', 0)}; "
                f"normalization_entries={term.get('normalization_entry_count', 0)}; "
                f"recurrence_after_normalization={term.get('recurrence_after_normalization_count', 0)}"
            )
    absence_model = record.get("absence_evidence_model_v1")
    if isinstance(absence_model, dict):
        lines.extend(["", "## Absence Evidence Model", ""])
        status_counts = absence_model.get("status_counts") or {}
        if isinstance(status_counts, dict):
            count_text = ", ".join(
                f"{status}={count}" for status, count in sorted(status_counts.items())
            )
        else:
            count_text = ""
        lines.append(
            f"- status=`{absence_model.get('status')}`; "
            f"authority=`{absence_model.get('authority')}`; "
            f"counts={count_text or '(none)'}"
        )
        for term in (absence_model.get("terms") or [])[:6]:
            if not isinstance(term, dict):
                continue
            lines.append(
                f"- `{term.get('term')}`: evidence=`{term.get('evidence_classification')}`; "
                f"read_more_unfollowed={term.get('read_more_requested_but_not_followed')}; "
                f"source_gaps={term.get('source_window_gap_count', 0)}; "
                f"named_coordinates={term.get('named_missing_coordinate_count', 0)}"
            )
    lived_term_bridge = record.get("lived_term_experiment_bridge_v1")
    if isinstance(lived_term_bridge, dict):
        lines.extend(["", "## Lived-Term Experiment Bridge", ""])
        status_counts = lived_term_bridge.get("status_counts") or {}
        if isinstance(status_counts, dict):
            count_text = ", ".join(
                f"{status}={count}" for status, count in sorted(status_counts.items())
            )
        else:
            count_text = ""
        lines.append(
            f"- status=`{lived_term_bridge.get('status')}`; "
            f"authority=`{lived_term_bridge.get('authority')}`; "
            f"candidates={lived_term_bridge.get('candidate_count', 0)}; "
            f"counts={count_text or '(none)'}"
        )
        for candidate in (lived_term_bridge.get("candidates") or [])[:6]:
            if not isinstance(candidate, dict):
                continue
            lines.append(
                f"- `{candidate.get('term')}`: bridge=`{candidate.get('bridge_status')}`; "
                f"card=`{candidate.get('card_status')}`; "
                f"next={candidate.get('recommended_next')}; "
                f"question={candidate.get('experiment_question')}"
            )
        activation = lived_term_bridge.get("activation_recommendation_v1")
        if isinstance(activation, dict) and activation.get("status") == "activation_scaffold_ready":
            lines.append(
                f"- activation scaffold: term=`{activation.get('term')}`; "
                f"creates_experiment={activation.get('creates_experiment')}"
            )
            for step in (activation.get("route") or [])[:3]:
                lines.append(f"  - route NEXT: {step}")
    charter_drafts = record.get("lived_term_charter_drafts_v1")
    if isinstance(charter_drafts, dict):
        lines.extend(["", "## Lived-Term Charter Drafts", ""])
        lines.append(
            f"- status=`{charter_drafts.get('status')}`; "
            f"authority=`{charter_drafts.get('authority')}`; "
            f"drafts={charter_drafts.get('draft_count', 0)}; "
            f"missing={charter_drafts.get('missing_draft_count', 0)}"
        )
        for draft in (charter_drafts.get("drafts") or [])[:6]:
            if not isinstance(draft, dict):
                continue
            targets = ", ".join(str(item) for item in draft.get("evidence_targets") or [])
            lines.append(
                f"- `{draft.get('term')}`: title={draft.get('experiment_title')}; "
                f"question={draft.get('question')}; "
                f"targets={targets or '(none)'}"
            )
            lines.append(
                f"  - suggested charter NEXT: {draft.get('suggested_charter_next')}"
            )
    counterexample_forge = record.get("lived_term_counterexample_forge_v1")
    if isinstance(counterexample_forge, dict):
        lines.extend(["", "## Counterexample Forge", ""])
        repeated = counterexample_forge.get(
            "repeated_without_counterdescriptor_terms"
        ) or []
        repeated_text = ", ".join(str(item) for item in repeated)
        lines.append(
            f"- status=`{counterexample_forge.get('status')}`; "
            f"authority=`{counterexample_forge.get('authority')}`; "
            f"drafts={counterexample_forge.get('draft_count', 0)}; "
            f"missing={counterexample_forge.get('missing_draft_count', 0)}; "
            f"needs_counter_descriptor={repeated_text or '(none)'}"
        )
        for draft in (counterexample_forge.get("drafts") or [])[:6]:
            if not isinstance(draft, dict):
                continue
            negative_targets = ", ".join(
                str(item) for item in draft.get("negative_case_targets") or []
            )
            lines.append(
                f"- `{draft.get('term')}`: question={draft.get('contrast_question')}; "
                f"negative_cases={negative_targets or '(none)'}"
            )
            lines.append(
                f"  - counter-descriptor prompt: {draft.get('counter_descriptor_prompt')}"
            )
            lines.append(
                f"  - suggested contrast NEXT: {draft.get('suggested_contrast_next')}"
            )
    journal_inventory = record.get("journal_inventory")
    if isinstance(journal_inventory, dict):
        lines.extend(["", "## Journal Inventory", ""])
        totals = journal_inventory.get("totals") or {}
        if isinstance(totals, dict):
            lines.append(
                f"- indexed files across known journal/inbox/outbox/archive roots: "
                f"{totals.get('indexed_files', 0)}"
            )
            lines.append(
                f"- loose journal-like files outside known roots: "
                f"{totals.get('loose_journal_like_file_count', 0)}"
            )
        by_being_inventory = journal_inventory.get("by_being") or {}
        if isinstance(by_being_inventory, dict):
            for being, inventory in sorted(by_being_inventory.items()):
                if not isinstance(inventory, dict):
                    continue
                lines.append(
                    f"- {being}: `{inventory.get('status')}`; "
                    f"live journal files={inventory.get('journal_live_files', 0)}, "
                    f"archived journal files={inventory.get('journal_archive_files', 0)}, "
                    f"indexed files={inventory.get('total_indexed_files', 0)}"
                )
                loose = inventory.get("loose_journal_like_files") or []
                if loose:
                    lines.append(
                        f"  review loose candidates: {', '.join(str(item) for item in loose[:5])}"
                    )
    qualia_comparison = record.get("qualia_comparison")
    if isinstance(qualia_comparison, dict):
        lines.extend(["", "## Qualia Comparison", ""])
        profiles = qualia_comparison.get("profiles") or []
        for profile in profiles:
            if not isinstance(profile, dict):
                continue
            densities = profile.get("densities_per_1k_words") or {}
            if not isinstance(densities, dict):
                densities = {}
            lanes = profile.get("lanes") or {}
            generated_body = lanes.get("generated_body") if isinstance(lanes, dict) else {}
            wrapper_tail = (
                lanes.get("wrapper_control_tail") if isinstance(lanes, dict) else {}
            )
            generated_ratio = (
                generated_body.get("qualia_to_metric_ratio")
                if isinstance(generated_body, dict)
                else 0
            )
            wrapper_ratio = (
                wrapper_tail.get("qualia_to_metric_ratio")
                if isinstance(wrapper_tail, dict)
                else 0
            )
            next_tail_counts = profile.get("next_tail_counts") or {}
            next_tail_text = ""
            if isinstance(next_tail_counts, dict) and next_tail_counts:
                next_tail_text = (
                    "; NEXT tails: "
                    + ", ".join(
                        f"{verb}={count}"
                        for verb, count in list(next_tail_counts.items())[:4]
                    )
                )
            lines.append(
                f"- {profile.get('being')}: {profile.get('sample_count')} samples, "
                f"avg chars={profile.get('avg_chars')}, "
                f"texture={densities.get('qualia_texture', 0)} /1k words, "
                f"metrics={densities.get('metrics', 0)} /1k, "
                f"action={densities.get('action_control', 0)} /1k, "
                f"whole qualia:metric={profile.get('qualia_to_metric_ratio')}, "
                f"body={generated_ratio}, wrapper/tail={wrapper_ratio}"
                f"{next_tail_text} — {profile.get('interpretation')}"
            )
        historical = qualia_comparison.get("minime_historical") or {}
        if isinstance(historical, dict):
            months = historical.get("months") or {}
            lines.extend(["", "**Minime Historical Baseline**", ""])
            cache = historical.get("historical_cache") or {}
            if isinstance(cache, dict):
                lines.append(
                    f"- cache: `{cache.get('status')}`; "
                    f"path: `{cache.get('path') or '(none)'}`; "
                    f"ttl_hours={cache.get('ttl_hours')}"
                )
            if isinstance(months, dict) and months:
                for month, info in sorted(months.items()):
                    if not isinstance(info, dict):
                        continue
                    whole = info.get("whole_file") or {}
                    body = info.get("generated_body") or {}
                    modes = info.get("mode_counts") or {}
                    next_tails = info.get("next_tail_counts") or {}
                    mode_text = (
                        ", ".join(f"{key}={value}" for key, value in list(modes.items())[:3])
                        if isinstance(modes, dict)
                        else ""
                    )
                    tail_text = (
                        ", ".join(
                            f"{key}={value}" for key, value in list(next_tails.items())[:3]
                        )
                        if isinstance(next_tails, dict)
                        else ""
                    )
                    whole_ratio = (
                        whole.get("qualia_to_metric_ratio")
                        if isinstance(whole, dict)
                        else 0
                    )
                    body_ratio = (
                        body.get("qualia_to_metric_ratio")
                        if isinstance(body, dict)
                        else 0
                    )
                    lines.append(
                        f"- {month}: {info.get('sample_count')} sampled / "
                        f"{info.get('total_files_seen')} files; "
                        f"body qualia:metric={body_ratio}, whole={whole_ratio}; "
                        f"modes: {mode_text or '(none)'}; NEXT tails: {tail_text or '(none)'}"
                    )
            else:
                lines.append("- no Minime historical journal samples found")
        findings = qualia_comparison.get("qualia_findings") or []
        if findings:
            lines.extend(["", "**Qualia Findings**", ""])
            for finding in findings:
                if not isinstance(finding, dict):
                    continue
                lines.append(
                    f"- {finding.get('being')}: {finding.get('finding')} "
                    f"(body/whole={finding.get('body_to_whole_multiplier')}, "
                    f"wrapper/tail={finding.get('wrapper_tail_qualia_to_metric_ratio')}) - "
                    f"{finding.get('recommendation')}"
                )
        gains = qualia_comparison.get("gains") or []
        if gains:
            lines.extend(["", "**Gains To Try**", ""])
            lines.extend(f"- {gain}" for gain in gains)
    elicitation = record.get("elicitation")
    if isinstance(elicitation, dict):
        candidates = elicitation.get("candidates") or []
        lines.extend(["", "## Self-Study Elicitation", ""])
        if candidates:
            for candidate in candidates:
                if not isinstance(candidate, dict):
                    continue
                anchors = ", ".join(candidate.get("source_anchors", [])[:6]) or "(none)"
                lines.append(
                    f"- {candidate['being']} `{candidate['topic']}`: "
                    f"{candidate['entry_count']} entries, score {candidate['score']}, "
                    f"anchors: {anchors}"
                )
        else:
            lines.append("- no invitation candidate this window")
        write_results = elicitation.get("write_results") or []
        for result in write_results:
            if isinstance(result, dict):
                lines.append(
                    f"- write {result.get('being')}/{result.get('topic')}: "
                    f"{result.get('status')} {result.get('reason', '')}".rstrip()
                )
    shared_tail = record.get("shared_tail_resonance")
    if isinstance(shared_tail, dict):
        lines.extend(["", "## Shared Tail Resonance", ""])
        lines.append(
            f"- packet: `{shared_tail.get('packet_md') or '(none)'}`; "
            f"pairs={shared_tail.get('pair_count', 0)}; "
            f"window_s={shared_tail.get('window_s')}"
        )
        for pair in (shared_tail.get("pairs") or [])[:5]:
            if not isinstance(pair, dict):
                continue
            astrid = pair.get("astrid") if isinstance(pair.get("astrid"), dict) else {}
            minime = pair.get("minime") if isinstance(pair.get("minime"), dict) else {}
            lines.append(
                f"- score {pair.get('score')}, Δ={pair.get('time_delta_s')}s, "
                f"terms={', '.join(pair.get('shared_terms') or []) or '(none)'}; "
                f"Astrid `{astrid.get('mode')}` -> Minime `{minime.get('mode')}`"
            )
    resistance_calibration = record.get("resistance_gradient_calibration")
    if isinstance(resistance_calibration, dict):
        lines.extend(["", "## Resistance Gradient Calibration", ""])
        lines.append(
            f"- packet: `{resistance_calibration.get('packet_md') or '(none)'}`; "
            f"artifacts={resistance_calibration.get('artifact_count', 0)}; "
            f"statuses={resistance_calibration.get('status_counts') or {}}"
        )
        for sample in (resistance_calibration.get("samples") or [])[:5]:
            if not isinstance(sample, dict):
                continue
            convergence = (
                sample.get("convergence")
                if isinstance(sample.get("convergence"), dict)
                else {}
            )
            lines.append(
                f"- `{sample.get('dominant_orientation')}` "
                f"trend=`{sample.get('gradient_trend')}` "
                f"review=`{convergence.get('status')}` "
                f"path=`{sample.get('path')}`"
            )
    lines.extend(["", "## High-Signal Entries", ""])
    for entry_obj in record["entries"]:  # type: ignore[index]
        entry = entry_obj
        if not isinstance(entry, dict):
            continue
        lines.extend(
            [
                f"### {entry['being']} / {entry['filename']}",
                "",
                f"- mode: `{entry['mode']}`",
                f"- grounding: `{entry['grounding']}`",
                f"- actionable_score: {entry['actionable_score']}",
                f"- path: `{entry['path']}`",
                f"- anchors: {', '.join(entry['source_anchors'][:8]) or '(none)'}",
                f"- NEXT actions: {', '.join(entry['next_actions'][:5]) or '(none)'}",
                "",
                "**Hypotheses / Claims To Verify**",
                "",
            ]
        )
        flags = entry.get("hypothesis_flags") or []
        if flags:
            lines.extend(f"- {flag}" for flag in flags[:5])
        else:
            lines.append("- (none detected)")
        lines.extend(["", "**Preview**", "", f"> {entry['preview']}", ""])
    return "\n".join(lines).rstrip() + "\n"


def build_review(
    *,
    astrid_workspace: Path,
    minime_workspace: Path,
    output_dir: Path,
    run: str,
    limit_per_being: int,
    since_last_review: bool = False,
    emit_elicitation_invitations: bool = False,
    elicitation_cooldown_hours: float = INVITATION_COOLDOWN_HOURS,
    refresh_historical_cache: bool = False,
    historical_cache_ttl_hours: float = HISTORICAL_QUALIA_CACHE_TTL_HOURS,
    tail_resonance_output_dir: Path | None = None,
    resistance_calibration_output_dir: Path | None = None,
) -> dict[str, object]:
    cutoff_mtime, cutoff_source = (
        latest_review_cutoff(output_dir) if since_last_review else (None, None)
    )
    entries = collect_entries(
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
        limit_per_being=limit_per_being,
        min_mtime_unix_s=cutoff_mtime,
    )
    journal_inventory = build_journal_inventory(
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
    )
    qualia_comparison = build_qualia_comparison(
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
        sample_limit_per_being=max(12, limit_per_being * 8),
        historical_cache_dir=output_dir / "_cache",
        refresh_historical_cache=refresh_historical_cache,
        historical_cache_ttl_hours=historical_cache_ttl_hours,
    )
    elicitation_candidates = build_elicitation_candidates(entries)
    shared_tail_resonance = build_shared_tail_resonance_packet(
        entries=entries,
        output_root=tail_resonance_output_dir
        or astrid_workspace / "diagnostics/tail_resonance_packets",
        run=run,
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
    )
    resistance_gradient_calibration = build_resistance_gradient_calibration_packet(
        entries=entries,
        output_root=resistance_calibration_output_dir
        or astrid_workspace / "diagnostics/resistance_gradient_calibrations",
        run=run,
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
    )
    astrid_introspection_digest_record = build_astrid_introspection_digest(astrid_workspace)
    shared_choice_envelope = build_shared_choice_envelope_review(
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
        limit_per_being=max(24, limit_per_being * 8),
    )
    self_regulation_leases = build_self_regulation_lease_review(
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
    )
    self_regulation_lease_learning = build_self_regulation_lease_learning(
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
    )
    self_regulation_negotiation_ledger_v1 = (
        build_self_regulation_negotiation_ledger(
            astrid_workspace=astrid_workspace,
            minime_workspace=minime_workspace,
        )
    )
    astrid_fill_pressure_calibration = build_astrid_fill_pressure_calibration(entries)
    semantic_friction_calibration = build_semantic_friction_calibration(entries)
    pressure_medium_kinetics_v1 = build_pressure_medium_kinetics(entries)
    pressure_kinetics_review_v1 = build_pressure_kinetics_review(entries)
    pressure_vector_v1 = build_pressure_vector_review(
        entries,
        pressure_medium_kinetics_v1=pressure_medium_kinetics_v1,
        pressure_kinetics_review_v1=pressure_kinetics_review_v1,
    )
    pressure_actuator_matrix_v1 = build_pressure_actuator_matrix(pressure_vector_v1)
    pressure_control_cockpit_v1 = build_pressure_control_cockpit(
        pressure_vector_v1,
        pressure_actuator_matrix_v1,
    )
    tail_vibrancy_vector_v1 = build_tail_vibrancy_vector_review(
        entries,
        pressure_vector_v1=pressure_vector_v1,
    )
    tail_vibrancy_authority_gap_v1 = build_tail_vibrancy_authority_gap(
        entries,
        tail_vibrancy_vector_v1=tail_vibrancy_vector_v1,
    )
    regulator_live_replay_v1 = build_regulator_live_replay(
        entries,
        minime_workspace=minime_workspace,
    )
    regulator_boundary_replay_cards_v1 = build_regulator_boundary_replay_cards(
        regulator_live_replay_v1
    )
    regulator_plateau_missing_variable_model_v1 = (
        build_regulator_plateau_missing_variable_model(
            regulator_live_replay_v1,
            regulator_boundary_replay_cards_v1,
        )
    )
    regulator_counterfactual_sandbox_scaffold_v1 = (
        build_regulator_counterfactual_sandbox_scaffold(
            regulator_boundary_replay_cards_v1,
            regulator_plateau_missing_variable_model_v1,
        )
    )
    regulator_counterfactual_sweep_v1 = build_regulator_counterfactual_sweep_review(
        minime_workspace
    )
    regulator_replay_time_series_v1 = build_regulator_replay_time_series(
        output_dir=output_dir,
        current_run=run,
        regulator_boundary_replay_cards_v1=regulator_boundary_replay_cards_v1,
        regulator_plateau_missing_variable_model_v1=regulator_plateau_missing_variable_model_v1,
    )
    regulator_counterfactual_replay_lab_v1 = build_regulator_counterfactual_replay_lab(
        regulator_counterfactual_sweep_v1=regulator_counterfactual_sweep_v1,
        regulator_boundary_replay_cards_v1=regulator_boundary_replay_cards_v1,
        regulator_plateau_missing_variable_model_v1=regulator_plateau_missing_variable_model_v1,
        regulator_replay_time_series_v1=regulator_replay_time_series_v1,
    )
    regulator_plateau_evidence_matrix_v1 = build_regulator_plateau_evidence_matrix(
        entries,
        regulator_live_replay_v1=regulator_live_replay_v1,
        regulator_boundary_replay_cards_v1=regulator_boundary_replay_cards_v1,
        regulator_plateau_missing_variable_model_v1=regulator_plateau_missing_variable_model_v1,
        semantic_friction_calibration=semantic_friction_calibration,
        astrid_fill_pressure_calibration=astrid_fill_pressure_calibration,
    )
    regulator_tuning_readiness_gate_v1 = build_regulator_tuning_readiness_gate(
        regulator_counterfactual_replay_lab_v1=regulator_counterfactual_replay_lab_v1,
        regulator_plateau_evidence_matrix_v1=regulator_plateau_evidence_matrix_v1,
    )
    pi_pressure_wiring_replay_v1 = build_pi_pressure_wiring_replay_review(
        minime_workspace
    )
    pi_pressure_candidate_readiness_v1 = build_pi_pressure_candidate_readiness(
        pi_pressure_wiring_replay_v1=pi_pressure_wiring_replay_v1,
        regulator_plateau_evidence_matrix_v1=regulator_plateau_evidence_matrix_v1,
    )
    pressure_source_to_pi_gap_v1 = build_pressure_source_to_pi_gap(
        pi_pressure_wiring_replay_v1=pi_pressure_wiring_replay_v1,
        pi_pressure_candidate_readiness_v1=pi_pressure_candidate_readiness_v1,
        pressure_vector_v1=pressure_vector_v1,
        pressure_medium_kinetics_v1=pressure_medium_kinetics_v1,
        regulator_plateau_evidence_matrix_v1=regulator_plateau_evidence_matrix_v1,
    )
    shared_pressure_vocabulary_calibration = (
        build_shared_pressure_vocabulary_calibration(entries)
    )
    agency_vernacular_continuity = build_agency_vernacular_continuity(entries)
    choice_ecology = build_choice_ecology_review(shared_choice_envelope)
    phenomenology_hypotheses_v1 = build_phenomenology_hypotheses(entries)
    phenomenology_hypothesis_cards_v1 = build_phenomenology_hypothesis_cards(entries)
    afterimage_absence_calibration_v1 = build_afterimage_absence_calibration(entries)
    afterimage_decay_tracker_v1 = build_afterimage_decay_tracker(entries)
    absence_evidence_model_v1 = build_absence_evidence_model(entries)
    lease_playbook_workbench_v1 = build_lease_playbook_workbench(
        self_regulation_leases=self_regulation_leases,
        self_regulation_lease_learning=self_regulation_lease_learning,
        astrid_fill_pressure_calibration=astrid_fill_pressure_calibration,
        semantic_friction_calibration=semantic_friction_calibration,
    )
    pressure_relief_playbook_v3 = build_pressure_relief_playbook_v3(
        self_regulation_lease_learning=self_regulation_lease_learning,
        pressure_vector_v1=pressure_vector_v1,
        pressure_actuator_matrix_v1=pressure_actuator_matrix_v1,
    )
    gradient_sensitive_relief_v1 = build_gradient_sensitive_relief(
        astrid_workspace=astrid_workspace,
        pressure_vector_v1=pressure_vector_v1,
    )
    pressure_relief_smoothness_replay_v1 = (
        build_pressure_relief_smoothness_replay(
            astrid_workspace=astrid_workspace,
            gradient_sensitive_relief_v1=gradient_sensitive_relief_v1,
        )
    )
    tail_vibrancy_relief_playbook_v1 = build_tail_vibrancy_relief_playbook(
        self_regulation_lease_learning=self_regulation_lease_learning,
        tail_vibrancy_vector_v1=tail_vibrancy_vector_v1,
        tail_vibrancy_authority_gap_v1=tail_vibrancy_authority_gap_v1,
    )
    tail_relief_trial_surface_v1 = build_tail_relief_trial_surface(
        astrid_workspace=astrid_workspace,
        tail_vibrancy_vector_v1=tail_vibrancy_vector_v1,
        pressure_vector_v1=pressure_vector_v1,
    )
    tail_lease_governor_v1 = build_tail_lease_governor(
        tail_relief_trial_surface_v1=tail_relief_trial_surface_v1
    )
    tail_lease_afterglow_v1 = build_tail_lease_afterglow(
        astrid_workspace=astrid_workspace,
        tail_relief_trial_surface_v1=tail_relief_trial_surface_v1,
    )
    tail_persistence_calibration_v1 = build_tail_persistence_calibration(
        entries,
        tail_lease_afterglow_v1=tail_lease_afterglow_v1,
        tail_relief_trial_surface_v1=tail_relief_trial_surface_v1,
        tail_vibrancy_vector_v1=tail_vibrancy_vector_v1,
    )
    shadow_synced_preflight_v1 = build_shadow_synced_preflight_review(
        astrid_workspace=astrid_workspace
    )
    tail_outcome_causal_learning_v1 = build_tail_outcome_causal_learning(
        astrid_workspace=astrid_workspace,
        tail_vibrancy_relief_playbook_v1=tail_vibrancy_relief_playbook_v1,
    )
    lease_boundary_repair_v1 = build_lease_boundary_repair(
        self_regulation_negotiation_ledger_v1=self_regulation_negotiation_ledger_v1,
        pressure_medium_kinetics_v1=pressure_medium_kinetics_v1,
        self_regulation_leases=self_regulation_leases,
        lease_playbook_workbench_v1=lease_playbook_workbench_v1,
    )
    lived_term_experiment_bridge_v1 = build_lived_term_experiment_bridge(
        phenomenology_hypothesis_cards_v1,
        afterimage_decay_tracker_v1=afterimage_decay_tracker_v1,
        absence_evidence_model_v1=absence_evidence_model_v1,
        lease_playbook_workbench_v1=lease_playbook_workbench_v1,
    )
    lived_term_charter_drafts_v1 = build_lived_term_charter_drafts(
        lived_term_experiment_bridge_v1
    )
    lived_term_counterexample_forge_v1 = build_lived_term_counterexample_forge(
        lived_term_experiment_bridge_v1
    )
    regulator_missing_variable_evidence_loop_v1 = (
        build_regulator_missing_variable_evidence_loop(
            regulator_plateau_evidence_matrix_v1=regulator_plateau_evidence_matrix_v1,
            regulator_tuning_readiness_gate_v1=regulator_tuning_readiness_gate_v1,
            lived_term_experiment_bridge_v1=lived_term_experiment_bridge_v1,
        )
    )
    control_semantics_calibration_v1 = build_control_semantics_calibration(entries)
    autonomous_truncation_shadow_review_v1 = build_autonomous_truncation_shadow_review(
        entries
    )
    codec_compression_calibration_v1 = build_codec_compression_calibration(entries)
    codec_entropy_vibrancy_review_v1 = build_codec_entropy_vibrancy_review(entries)
    pressure_release_rehearsal_review_v1 = build_pressure_release_rehearsal_review(
        entries
    )
    witness_resonance_v1 = build_witness_resonance_review(entries)
    witness_texture_integrity_v1 = build_witness_texture_integrity_review(
        entries,
        astrid_workspace=astrid_workspace,
    )
    entropy_pressure_divergence_v1 = build_entropy_pressure_divergence_review(entries)
    fallback_continuity_fire_drill_v1 = build_fallback_continuity_fire_drill_review(
        entries,
        astrid_workspace=astrid_workspace,
    )
    spectral_texture_calibration_v2 = (
        spectral_texture_calibration_audit.build_calibration_record(
            astrid_workspace=astrid_workspace,
            minime_workspace=minime_workspace,
            since_hours=24.0,
            output_root=output_dir / "spectral_texture_calibrations",
            write_artifact=False,
        )
    )
    fallback_capacity_readiness_gate_v1 = build_fallback_capacity_readiness_gate(
        fallback_continuity_fire_drill_v1
    )
    fallback_format_texture_stabilizer_v1 = build_fallback_format_texture_stabilizer(
        fallback_continuity_fire_drill_v1,
        fallback_capacity_readiness_gate_v1,
    )
    fallback_contract_distillation_v1 = build_fallback_contract_distillation_review(
        astrid_workspace=astrid_workspace
    )
    fallback_distinguishability_calibration_v1 = (
        build_fallback_distinguishability_calibration(
            fallback_continuity_fire_drill_v1,
            fallback_contract_distillation_v1,
        )
    )
    fallback_complexity_budget_lab_v1 = build_fallback_complexity_budget_lab(
        entries,
        fallback_continuity_fire_drill_v1,
        fallback_contract_distillation_v1,
    )
    autonomous_truncation_rehearsal_v1 = build_autonomous_truncation_rehearsal_review(
        astrid_workspace=astrid_workspace,
        autonomous_truncation_shadow_review_v1=autonomous_truncation_shadow_review_v1,
    )
    codec_entropy_vibrancy_probe_v1 = build_codec_entropy_vibrancy_probe_review(
        astrid_workspace=astrid_workspace,
        codec_entropy_vibrancy_review_v1=codec_entropy_vibrancy_review_v1,
    )
    codec_real_replay_v1 = build_codec_real_replay_review(
        astrid_workspace=astrid_workspace,
        codec_entropy_vibrancy_probe_v1=codec_entropy_vibrancy_probe_v1,
    )
    narrative_arc_temporal_decay_lab_v1 = build_narrative_arc_temporal_decay_lab(
        codec_real_replay_v1
    )
    content_aware_vibrancy_gate_candidate_v1 = (
        build_content_aware_vibrancy_gate_candidate(
            codec_real_replay_v1,
            codec_entropy_vibrancy_probe_v1,
        )
    )
    codec_multipoint_inflection_v1 = build_codec_multipoint_inflection_review(
        entries,
        codec_real_replay_v1=codec_real_replay_v1,
        narrative_arc_temporal_decay_lab_v1=narrative_arc_temporal_decay_lab_v1,
        content_aware_vibrancy_gate_candidate_v1=content_aware_vibrancy_gate_candidate_v1,
    )
    codec_clamp_headroom_probe_v1 = build_codec_clamp_headroom_probe_review(
        codec_real_replay_v1
    )
    codec_afterimage_time_series_v1 = build_codec_afterimage_time_series(
        entries,
        afterimage_decay_tracker_v1=afterimage_decay_tracker_v1,
        codec_real_replay_v1=codec_real_replay_v1,
    )
    tail_participation_counterfactual_lab_v1 = (
        build_tail_participation_counterfactual_lab_review(codec_real_replay_v1)
    )
    tail_authority_ladder_v1 = build_tail_authority_ladder(
        tail_vibrancy_vector_v1=tail_vibrancy_vector_v1,
        tail_vibrancy_authority_gap_v1=tail_vibrancy_authority_gap_v1,
        tail_relief_trial_surface_v1=tail_relief_trial_surface_v1,
        tail_lease_governor_v1=tail_lease_governor_v1,
        tail_lease_afterglow_v1=tail_lease_afterglow_v1,
        shadow_synced_preflight_v1=shadow_synced_preflight_v1,
        tail_outcome_causal_learning_v1=tail_outcome_causal_learning_v1,
        tail_participation_counterfactual_lab_v1=tail_participation_counterfactual_lab_v1,
    )
    returnable_distinctions_v1 = build_returnable_distinctions(
        control_semantics_calibration_v1=control_semantics_calibration_v1,
        pressure_kinetics_review_v1=pressure_kinetics_review_v1,
        semantic_friction_calibration=semantic_friction_calibration,
        codec_compression_calibration_v1=codec_compression_calibration_v1,
        pressure_release_rehearsal_review_v1=pressure_release_rehearsal_review_v1,
        witness_resonance_v1=witness_resonance_v1,
        witness_texture_integrity_v1=witness_texture_integrity_v1,
        entropy_pressure_divergence_v1=entropy_pressure_divergence_v1,
        fallback_continuity_fire_drill_v1=fallback_continuity_fire_drill_v1,
        spectral_texture_calibration_v2=spectral_texture_calibration_v2,
        fallback_capacity_readiness_gate_v1=fallback_capacity_readiness_gate_v1,
        fallback_format_texture_stabilizer_v1=fallback_format_texture_stabilizer_v1,
        fallback_distinguishability_calibration_v1=fallback_distinguishability_calibration_v1,
        fallback_complexity_budget_lab_v1=fallback_complexity_budget_lab_v1,
        autonomous_truncation_rehearsal_v1=autonomous_truncation_rehearsal_v1,
        codec_entropy_vibrancy_probe_v1=codec_entropy_vibrancy_probe_v1,
        codec_real_replay_v1=codec_real_replay_v1,
        narrative_arc_temporal_decay_lab_v1=narrative_arc_temporal_decay_lab_v1,
        content_aware_vibrancy_gate_candidate_v1=content_aware_vibrancy_gate_candidate_v1,
        codec_multipoint_inflection_v1=codec_multipoint_inflection_v1,
        codec_clamp_headroom_probe_v1=codec_clamp_headroom_probe_v1,
        codec_afterimage_time_series_v1=codec_afterimage_time_series_v1,
        gradient_sensitive_relief_v1=gradient_sensitive_relief_v1,
        pressure_relief_smoothness_replay_v1=pressure_relief_smoothness_replay_v1,
        tail_persistence_calibration_v1=tail_persistence_calibration_v1,
    )
    distinction_lifecycle_v1 = build_distinction_lifecycle(
        returnable_distinctions_v1=returnable_distinctions_v1,
        output_dir=output_dir,
        current_run=run,
    )
    actionable_review_items = build_actionable_review_items(
        qualia_comparison=qualia_comparison,
        shared_tail_resonance=shared_tail_resonance,
        resistance_gradient_calibration=resistance_gradient_calibration,
        astrid_introspection_digest_record=astrid_introspection_digest_record,
        shared_choice_envelope=shared_choice_envelope,
        self_regulation_leases=self_regulation_leases,
        self_regulation_negotiation_ledger_v1=self_regulation_negotiation_ledger_v1,
        pressure_medium_kinetics_v1=pressure_medium_kinetics_v1,
        lease_boundary_repair_v1=lease_boundary_repair_v1,
        pressure_vector_v1=pressure_vector_v1,
        pressure_control_cockpit_v1=pressure_control_cockpit_v1,
        pressure_actuator_matrix_v1=pressure_actuator_matrix_v1,
        pressure_relief_playbook_v3=pressure_relief_playbook_v3,
        gradient_sensitive_relief_v1=gradient_sensitive_relief_v1,
        pressure_relief_smoothness_replay_v1=pressure_relief_smoothness_replay_v1,
        tail_vibrancy_vector_v1=tail_vibrancy_vector_v1,
        tail_vibrancy_authority_gap_v1=tail_vibrancy_authority_gap_v1,
        tail_vibrancy_relief_playbook_v1=tail_vibrancy_relief_playbook_v1,
        tail_relief_trial_surface_v1=tail_relief_trial_surface_v1,
        tail_lease_governor_v1=tail_lease_governor_v1,
        tail_lease_afterglow_v1=tail_lease_afterglow_v1,
        shadow_synced_preflight_v1=shadow_synced_preflight_v1,
        tail_outcome_causal_learning_v1=tail_outcome_causal_learning_v1,
        tail_participation_counterfactual_lab_v1=tail_participation_counterfactual_lab_v1,
        tail_authority_ladder_v1=tail_authority_ladder_v1,
        tail_persistence_calibration_v1=tail_persistence_calibration_v1,
        astrid_fill_pressure_calibration=astrid_fill_pressure_calibration,
        semantic_friction_calibration=semantic_friction_calibration,
        regulator_live_replay_v1=regulator_live_replay_v1,
        regulator_boundary_replay_cards_v1=regulator_boundary_replay_cards_v1,
        regulator_plateau_missing_variable_model_v1=regulator_plateau_missing_variable_model_v1,
        regulator_counterfactual_sandbox_scaffold_v1=regulator_counterfactual_sandbox_scaffold_v1,
        regulator_counterfactual_sweep_v1=regulator_counterfactual_sweep_v1,
        regulator_replay_time_series_v1=regulator_replay_time_series_v1,
        regulator_counterfactual_replay_lab_v1=regulator_counterfactual_replay_lab_v1,
        regulator_plateau_evidence_matrix_v1=regulator_plateau_evidence_matrix_v1,
        regulator_tuning_readiness_gate_v1=regulator_tuning_readiness_gate_v1,
        pi_pressure_wiring_replay_v1=pi_pressure_wiring_replay_v1,
        pi_pressure_candidate_readiness_v1=pi_pressure_candidate_readiness_v1,
        pressure_source_to_pi_gap_v1=pressure_source_to_pi_gap_v1,
        regulator_missing_variable_evidence_loop_v1=regulator_missing_variable_evidence_loop_v1,
        control_semantics_calibration_v1=control_semantics_calibration_v1,
        pressure_kinetics_review_v1=pressure_kinetics_review_v1,
        autonomous_truncation_shadow_review_v1=autonomous_truncation_shadow_review_v1,
        codec_compression_calibration_v1=codec_compression_calibration_v1,
        codec_entropy_vibrancy_review_v1=codec_entropy_vibrancy_review_v1,
        pressure_release_rehearsal_review_v1=pressure_release_rehearsal_review_v1,
        witness_resonance_v1=witness_resonance_v1,
        witness_texture_integrity_v1=witness_texture_integrity_v1,
        entropy_pressure_divergence_v1=entropy_pressure_divergence_v1,
        fallback_continuity_fire_drill_v1=fallback_continuity_fire_drill_v1,
        spectral_texture_calibration_v2=spectral_texture_calibration_v2,
        fallback_capacity_readiness_gate_v1=fallback_capacity_readiness_gate_v1,
        fallback_format_texture_stabilizer_v1=fallback_format_texture_stabilizer_v1,
        fallback_contract_distillation_v1=fallback_contract_distillation_v1,
        fallback_distinguishability_calibration_v1=fallback_distinguishability_calibration_v1,
        fallback_complexity_budget_lab_v1=fallback_complexity_budget_lab_v1,
        autonomous_truncation_rehearsal_v1=autonomous_truncation_rehearsal_v1,
        codec_entropy_vibrancy_probe_v1=codec_entropy_vibrancy_probe_v1,
        codec_real_replay_v1=codec_real_replay_v1,
        narrative_arc_temporal_decay_lab_v1=narrative_arc_temporal_decay_lab_v1,
        content_aware_vibrancy_gate_candidate_v1=content_aware_vibrancy_gate_candidate_v1,
        codec_multipoint_inflection_v1=codec_multipoint_inflection_v1,
        codec_clamp_headroom_probe_v1=codec_clamp_headroom_probe_v1,
        codec_afterimage_time_series_v1=codec_afterimage_time_series_v1,
        returnable_distinctions_v1=returnable_distinctions_v1,
        distinction_lifecycle_v1=distinction_lifecycle_v1,
        shared_pressure_vocabulary_calibration=shared_pressure_vocabulary_calibration,
        agency_vernacular_continuity=agency_vernacular_continuity,
        self_regulation_lease_learning=self_regulation_lease_learning,
        choice_ecology=choice_ecology,
        phenomenology_hypotheses_v1=phenomenology_hypotheses_v1,
        phenomenology_hypothesis_cards_v1=phenomenology_hypothesis_cards_v1,
        afterimage_absence_calibration_v1=afterimage_absence_calibration_v1,
        afterimage_decay_tracker_v1=afterimage_decay_tracker_v1,
        absence_evidence_model_v1=absence_evidence_model_v1,
        lived_term_experiment_bridge_v1=lived_term_experiment_bridge_v1,
        lived_term_charter_drafts_v1=lived_term_charter_drafts_v1,
        lived_term_counterexample_forge_v1=lived_term_counterexample_forge_v1,
        lease_playbook_workbench_v1=lease_playbook_workbench_v1,
    )
    write_results = (
        write_elicitation_invitations(
            elicitation_candidates,
            astrid_workspace=astrid_workspace,
            minime_workspace=minime_workspace,
            run=run,
            cooldown_hours=elicitation_cooldown_hours,
        )
        if emit_elicitation_invitations
        else []
    )
    record: dict[str, object] = {
        "run_id": run,
        "generated_at": dt.datetime.now(dt.UTC).isoformat(),
        "astrid_workspace": str(astrid_workspace),
        "minime_workspace": str(minime_workspace),
        "review_window": {
            "since_last_review": since_last_review,
            "cutoff_unix_s": cutoff_mtime,
            "cutoff_source": cutoff_source,
        },
        "summary": summarize(entries),
        "actionable_review_items": actionable_review_items,
        "astrid_introspection_digest": astrid_introspection_digest_record,
        "journal_inventory": journal_inventory,
        "qualia_comparison": qualia_comparison,
        "shared_tail_resonance": shared_tail_resonance,
        "resistance_gradient_calibration": resistance_gradient_calibration,
        "shared_choice_envelope": shared_choice_envelope,
        "choice_ecology": choice_ecology,
        "self_regulation_leases": self_regulation_leases,
        "self_regulation_lease_learning": self_regulation_lease_learning,
        "self_regulation_negotiation_ledger_v1": self_regulation_negotiation_ledger_v1,
        "pressure_medium_kinetics_v1": pressure_medium_kinetics_v1,
        "pressure_vector_v1": pressure_vector_v1,
        "pressure_control_cockpit_v1": pressure_control_cockpit_v1,
        "pressure_actuator_matrix_v1": pressure_actuator_matrix_v1,
        "gradient_sensitive_relief_v1": gradient_sensitive_relief_v1,
        "pressure_relief_smoothness_replay_v1": pressure_relief_smoothness_replay_v1,
        "tail_vibrancy_vector_v1": tail_vibrancy_vector_v1,
        "tail_vibrancy_authority_gap_v1": tail_vibrancy_authority_gap_v1,
        "tail_vibrancy_relief_playbook_v1": tail_vibrancy_relief_playbook_v1,
        "tail_relief_trial_surface_v1": tail_relief_trial_surface_v1,
        "tail_lease_governor_v1": tail_lease_governor_v1,
        "tail_lease_afterglow_v1": tail_lease_afterglow_v1,
        "tail_persistence_calibration_v1": tail_persistence_calibration_v1,
        "shadow_synced_preflight_v1": shadow_synced_preflight_v1,
        "tail_outcome_causal_learning_v1": tail_outcome_causal_learning_v1,
        "tail_participation_counterfactual_lab_v1": tail_participation_counterfactual_lab_v1,
        "tail_authority_ladder_v1": tail_authority_ladder_v1,
        "lease_boundary_repair_v1": lease_boundary_repair_v1,
        "astrid_fill_pressure_calibration": astrid_fill_pressure_calibration,
        "semantic_friction_calibration": semantic_friction_calibration,
        "regulator_live_replay_v1": regulator_live_replay_v1,
        "regulator_boundary_replay_cards_v1": regulator_boundary_replay_cards_v1,
        "regulator_plateau_missing_variable_model_v1": regulator_plateau_missing_variable_model_v1,
        "regulator_counterfactual_sandbox_scaffold_v1": regulator_counterfactual_sandbox_scaffold_v1,
        "regulator_counterfactual_sweep_v1": regulator_counterfactual_sweep_v1,
        "regulator_replay_time_series_v1": regulator_replay_time_series_v1,
        "regulator_counterfactual_replay_lab_v1": regulator_counterfactual_replay_lab_v1,
        "regulator_plateau_evidence_matrix_v1": regulator_plateau_evidence_matrix_v1,
        "regulator_tuning_readiness_gate_v1": regulator_tuning_readiness_gate_v1,
        "pi_pressure_wiring_replay_v1": pi_pressure_wiring_replay_v1,
        "pi_pressure_candidate_readiness_v1": pi_pressure_candidate_readiness_v1,
        "pressure_source_to_pi_gap_v1": pressure_source_to_pi_gap_v1,
        "regulator_missing_variable_evidence_loop_v1": regulator_missing_variable_evidence_loop_v1,
        "control_semantics_calibration_v1": control_semantics_calibration_v1,
        "pressure_kinetics_review_v1": pressure_kinetics_review_v1,
        "autonomous_truncation_shadow_review_v1": autonomous_truncation_shadow_review_v1,
        "codec_compression_calibration_v1": codec_compression_calibration_v1,
        "codec_entropy_vibrancy_review_v1": codec_entropy_vibrancy_review_v1,
        "pressure_release_rehearsal_review_v1": pressure_release_rehearsal_review_v1,
        "witness_resonance_v1": witness_resonance_v1,
        "witness_texture_integrity_v1": witness_texture_integrity_v1,
        "entropy_pressure_divergence_v1": entropy_pressure_divergence_v1,
        "fallback_continuity_fire_drill_v1": fallback_continuity_fire_drill_v1,
        "spectral_texture_calibration_v2": spectral_texture_calibration_v2,
        "fallback_capacity_readiness_gate_v1": fallback_capacity_readiness_gate_v1,
        "fallback_format_texture_stabilizer_v1": fallback_format_texture_stabilizer_v1,
        "fallback_contract_distillation_v1": fallback_contract_distillation_v1,
        "fallback_distinguishability_calibration_v1": fallback_distinguishability_calibration_v1,
        "fallback_complexity_budget_lab_v1": fallback_complexity_budget_lab_v1,
        "autonomous_truncation_rehearsal_v1": autonomous_truncation_rehearsal_v1,
        "codec_entropy_vibrancy_probe_v1": codec_entropy_vibrancy_probe_v1,
        "codec_real_replay_v1": codec_real_replay_v1,
        "narrative_arc_temporal_decay_lab_v1": narrative_arc_temporal_decay_lab_v1,
        "content_aware_vibrancy_gate_candidate_v1": content_aware_vibrancy_gate_candidate_v1,
        "codec_multipoint_inflection_v1": codec_multipoint_inflection_v1,
        "codec_clamp_headroom_probe_v1": codec_clamp_headroom_probe_v1,
        "codec_afterimage_time_series_v1": codec_afterimage_time_series_v1,
        "returnable_distinctions_v1": returnable_distinctions_v1,
        "distinction_lifecycle_v1": distinction_lifecycle_v1,
        "shared_pressure_vocabulary_calibration": shared_pressure_vocabulary_calibration,
        "agency_vernacular_continuity": agency_vernacular_continuity,
        "phenomenology_hypotheses_v1": phenomenology_hypotheses_v1,
        "phenomenology_hypothesis_cards_v1": phenomenology_hypothesis_cards_v1,
        "afterimage_absence_calibration_v1": afterimage_absence_calibration_v1,
        "afterimage_decay_tracker_v1": afterimage_decay_tracker_v1,
        "absence_evidence_model_v1": absence_evidence_model_v1,
        "lived_term_experiment_bridge_v1": lived_term_experiment_bridge_v1,
        "lived_term_charter_drafts_v1": lived_term_charter_drafts_v1,
        "lived_term_counterexample_forge_v1": lived_term_counterexample_forge_v1,
        "lease_playbook_workbench_v1": lease_playbook_workbench_v1,
        "pressure_relief_playbook_v3": pressure_relief_playbook_v3,
        "elicitation": {
            "cooldown_hours": elicitation_cooldown_hours,
            "write_requested": emit_elicitation_invitations,
            "candidates": [asdict(candidate) for candidate in elicitation_candidates],
            "write_results": write_results,
        },
        "entries": [asdict(entry) for entry in entries],
    }
    target_dir = output_dir / run
    target_dir.mkdir(parents=True, exist_ok=True)
    cards_json = target_dir / "phenomenology_hypothesis_cards.json"
    cards_json.write_text(
        json.dumps(phenomenology_hypothesis_cards_v1, indent=2, sort_keys=True),
        encoding="utf-8",
    )
    record["phenomenology_hypothesis_cards_json"] = str(cards_json)
    (target_dir / "review.json").write_text(
        json.dumps(record, indent=2, sort_keys=True),
        encoding="utf-8",
    )
    (target_dir / "review.md").write_text(render_markdown(record), encoding="utf-8")
    record["output_dir"] = str(target_dir)
    record["review_json"] = str(target_dir / "review.json")
    record["review_md"] = str(target_dir / "review.md")
    return record


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--astrid-workspace", type=Path, default=ASTRID_WORKSPACE)
    parser.add_argument("--minime-workspace", type=Path, default=MINIME_WORKSPACE)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--limit", type=int, default=5, help="recent entries per lane/pattern")
    parser.add_argument(
        "--since-last-review",
        action="store_true",
        help="only include entries modified after the latest prior review packet",
    )
    parser.add_argument(
        "--write-elicitation-invitations",
        action="store_true",
        help="write gentle MIKE QUERY self-study invitations for repeated high-signal threads",
    )
    parser.add_argument(
        "--elicitation-cooldown-hours",
        type=float,
        default=INVITATION_COOLDOWN_HOURS,
        help="cooldown per being inbox before writing another self-study invitation",
    )
    parser.add_argument(
        "--refresh-historical-cache",
        action="store_true",
        help="force recomputation of preserved Minime historical qualia baselines",
    )
    parser.add_argument(
        "--historical-cache-ttl-hours",
        type=float,
        default=HISTORICAL_QUALIA_CACHE_TTL_HOURS,
        help="TTL for preserved Minime historical qualia baseline cache",
    )
    parser.add_argument(
        "--tail-resonance-output-dir",
        type=Path,
        default=TAIL_RESONANCE_OUTPUT_DIR,
        help="where to write shared Astrid/Minime tail-resonance packets",
    )
    parser.add_argument(
        "--resistance-calibration-output-dir",
        type=Path,
        default=RESISTANCE_CALIBRATION_OUTPUT_DIR,
        help="where to write resistance-gradient calibration packets",
    )
    parser.add_argument("--print-summary", action="store_true")
    args = parser.parse_args()

    record = build_review(
        astrid_workspace=args.astrid_workspace,
        minime_workspace=args.minime_workspace,
        output_dir=args.output_dir,
        run=args.run_id or run_id(),
        limit_per_being=max(1, args.limit),
        since_last_review=args.since_last_review,
        emit_elicitation_invitations=args.write_elicitation_invitations,
        elicitation_cooldown_hours=max(0.0, args.elicitation_cooldown_hours),
        refresh_historical_cache=args.refresh_historical_cache,
        historical_cache_ttl_hours=max(0.0, args.historical_cache_ttl_hours),
        tail_resonance_output_dir=args.tail_resonance_output_dir,
        resistance_calibration_output_dir=args.resistance_calibration_output_dir,
    )
    print(f"self-study review: {record['review_md']}")
    if args.print_summary:
        print(json.dumps(record["summary"], indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
