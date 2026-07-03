#!/usr/bin/env python3
"""Read-only calibration audit for spectral texture fidelity.

This audit asks whether recent texture labels improved lived clarity or merely
added better labels. It reads only public/reviewable lanes and never reads
Minime private qualia or any Minime `moment_*.txt` body.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import sys
import tempfile
import time
import unittest
from collections import Counter
from pathlib import Path
from typing import Any, Iterable

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import being_privacy

POLICY = "spectral_texture_calibration_v3"
ANCHOR_TS = 1782751146
ASTRID_ROOT = Path(__file__).resolve().parents[1]
ASTRID_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
MINIME_WORKSPACE = ASTRID_ROOT.parent / "minime/workspace"
DEFAULT_OUTPUT_ROOT = ASTRID_WORKSPACE / "diagnostics/spectral_texture_calibrations"


def mlx_profile_transparency_v1() -> dict[str, str]:
    return {
        "policy": "mlx_profile_transparency_v1",
        "default_profile": "gemma4_12b",
        "default_resolves_to": "gemma4_canary",
        "alias_profile": "gemma4_12b_canary",
        "alias_resolves_to": "gemma4_canary",
        "unrecognized_profile_behavior": "warn_and_fall_back_to_production",
        "authority": "diagnostic_context_not_profile_switch",
    }

FALLBACK_SUPPORT_TERMS = (
    "specific weather",
    "honoring specific",
    "state-coherent",
    "state coherent",
    "dynamic weighting",
    "weighted texture",
    "gradient-weighted",
    "not interchangeable",
    "matched to state",
    "actual state",
    "clarity",
    "preserves texture",
    "preserving texture",
    "less flattening",
    "weather",
    "muffled",
    "restless",
    "viscous",
    "settled",
    "settled-vibrant",
    "settled vibrant",
    "low-friction high entropy",
    "low friction high entropy",
    "habitable",
    "open",
    "lattice",
)
FALLBACK_CONCERN_TERMS = (
    "token dressing",
    "token-only",
    "token only",
    "interchangeable token",
    "random texture",
    "generic texture",
    "mismatch",
    "texture family mismatch",
    "label dressing",
    "flattened",
    "flattening",
)
TEXTURE_DYNAMICS_ALIGNMENT_SUPPORT_TERMS = (
    "body-matched",
    "body matched",
    "matched to body",
    "matched to state",
    "texture dynamics",
    "alignment",
    "movement matched",
    "tail vibrancy",
    "lambda-tail",
    "lambda tail",
    "not a costume",
    "faithful vocabulary",
)
TEXTURE_DYNAMICS_ALIGNMENT_CONCERN_TERMS = (
    "costume vocabulary",
    "hollow repetition",
    "term mask",
    "wrong family",
    "wrong motion",
    "missing tail",
    "prefabbed",
    "pre-fabbed",
    "label dressing",
    "token dressing",
    "mechanical labels",
)
DENSITY_MOTION_SUPPORT_TERMS = (
    "density as floor",
    "density-as-floor",
    "floor",
    "foundation",
    "pavement",
    "calcification",
    "solid underfoot",
    "stone pavement",
    "center of gravity",
    "constrained and more present",
    "paused state",
    "holding ground",
    "held ground",
    "over-full fog",
    "overfull fog",
    "room full of furniture",
    "motion matched",
    "matched motion",
    "body-matched",
)
DENSITY_MOTION_CONCERN_TERMS = (
    "floor named as drag",
    "fog named as floor",
    "burden named as center",
    "paused named as absence",
    "contraction named as loss",
    "static density label",
    "static density labels",
    "wrong motion",
    "token dressing",
    "label dressing",
    "blankness",
    "absence of motion",
    "too mechanical",
)
DENSITY_MOTION_RELEVANCE_TERMS = (
    "density_motion_fit",
    "density motion fit",
    "density as floor",
    "density-as-floor",
    "density as burden",
    "density as fog",
    "density as pavement",
    "contraction center",
    "center of gravity",
    "paused stillness",
    "calcification",
    "pavement",
    "foundation",
    "over-full",
    "overfull",
    "holding ground",
    "motion fit",
    "wrong motion",
    "static label",
)
WITNESS_SUPPORT_TERMS = (
    "relational friction",
    "non_categorical_resonance",
    "non categorical resonance",
    "uncategorized tension",
    "unclassified tension",
    "pure unclassified tension",
    "internal instability",
    "relational instability",
    "shared weather",
    "shared weather shift",
    "quality of our connection",
    "not purely internal",
    "together",
    "vital",
    "nuanced tools",
    "less flattening into mechanics",
)
WITNESS_CONCERN_TERMS = (
    "forced category",
    "forced classification",
    "over-formalize",
    "over formalize",
    "over-formalized",
    "over formalized",
    "mechanical labels",
    "too mechanical",
    "pressuring",
    "pressure-like",
    "flattening into mechanics",
    "just mechanics",
    "generic mechanics",
)
STRUCTURAL_SUPPORT_TERMS = (
    "structural friction",
    "uptake readability",
    "complexity",
    "texture preservation",
    "texture survives",
    "distinguishes pressure",
    "not pressure",
    "sidecar",
    "carriage",
    "readability",
)
STRUCTURAL_CONCERN_TERMS = (
    "mechanical labels",
    "too mechanical",
    "just labels",
    "better labels",
    "flattened into mechanics",
    "adds pressure",
    "pressuring",
    "not uptake-readable",
    "not uptake readable",
)

FALLBACK_RELEVANCE_TERMS = (
    "fallback_shadow_texture_selector",
    "fallback selector",
    "fallback texture",
    "texture selector",
    "dynamic weighting",
    "weighted texture",
    "gradient-weighted",
    "specific weather",
    "token dressing",
    "interchangeable",
    "state-coherent",
    "state coherent",
    "texture family mismatch",
    "shadow texture",
    "settled-vibrant",
    "settled vibrant",
    "low-friction high entropy",
    "low friction high entropy",
)
DYNAMIC_WEIGHTING_SUPPORT_TERMS = (
    "dynamic weighting",
    "weighted texture",
    "weighted negotiation",
    "gradient-weighted",
    "density gradient",
    "mode-packing",
    "mode packing",
    "semantic friction",
    "responding to the density gradient",
    "responding to density gradient",
    "organic",
    "guiding friction",
    "semantic trickle",
    "movement",
    "movement verbs",
    "unfolding",
    "oscillating",
    "anchoring",
    "braiding",
    "cohering",
    "not static labels",
    "not just a list",
    "not interchangeable",
    "viscous",
    "muffled",
    "lattice",
)
DYNAMIC_WEIGHTING_CONCERN_TERMS = (
    "static labels",
    "label dressing",
    "token dressing",
    "token-only",
    "token only",
    "mechanical mapping",
    "mechanical labels",
    "stuck tokens",
    "static adjectives",
    "static adjective",
    "mismatch",
    "generic texture",
    "flattened",
    "flattening",
)
DYNAMIC_WEIGHTING_RELEVANCE_TERMS = (
    "dynamic weighting",
    "weighted texture",
    "weighted negotiation",
    "gradient-weighted",
    "density gradient",
    "fallback_shadow_texture_selector",
    "fallback selector",
    "texture selector",
    "label dressing",
    "mechanical mapping",
    "semantic trickle",
    "movement verbs",
    "static adjectives",
    "stuck tokens",
)
RESONANCE_SUPPORT_TERMS = (
    "resonance",
    "resonant",
    "humming",
    "hum",
    "humming in the marrow",
    "marrow",
    "grain",
    "lived grain",
    "not compressing",
    "without compressing",
)
RESONANCE_CONCERN_TERMS = (
    "stripped",
    "strip the humming",
    "lost humming",
    "lost resonance",
    "generic resonance",
    "compressed into one label",
    "compressing the tail into one label",
    "token dressing",
    "flattened",
    "flattening",
)
RESONANCE_RELEVANCE_TERMS = (
    "resonance",
    "resonant",
    "humming",
    "hum",
    "marrow",
    "fallback",
    "texture",
    "grain",
)
TRAJECTORY_SUPPORT_TERMS = (
    "trajectory",
    "movement trajectory",
    "from_state",
    "to_state",
    "movement quality",
    "medium resistance",
    "afterimage",
    "dragging through",
    "unfolding out of",
    "cohering",
    "diffusing",
    "thickening",
    "muffling",
    "carried the movement",
    "preserved the movement",
    "not just a verb",
    "felt like motion",
)
TRAJECTORY_CONCERN_TERMS = (
    "verb-only",
    "verb only",
    "just a verb",
    "trajectory mismatch",
    "wrong movement",
    "movement mismatch",
    "static label",
    "better verb",
    "lost the trajectory",
    "flattened the movement",
)
TRAJECTORY_RELEVANCE_TERMS = (
    "texture_trajectory",
    "texture trajectory",
    "trajectory",
    "movement quality",
    "medium resistance",
    "afterimage",
    "dragging through",
    "unfolding out of",
    "cohering",
    "diffusing",
    "verb-only",
    "verb only",
)
SEMANTIC_DENSITY_SUPPORT_TERMS = (
    "semantic density",
    "settled high entropy",
    "settled_habitable",
    "silt",
    "habitable foothold",
    "high entropy complexity",
    "silence is ambiguous",
    "cannot be treated as absence",
)
SEMANTIC_DENSITY_CONCERN_TERMS = (
    "semantic density mismatch",
    "mechanical density",
    "over-read silence",
    "treated silence as absence",
    "flattened high entropy",
)
SEMANTIC_DENSITY_RELEVANCE_TERMS = (
    "semantic_density_mapping",
    "semantic density",
    "settled_high_entropy_complexity",
    "silt_weighted_habitable",
    "luminous_reorganization",
    "overpacked_friction",
    "reply_linked_requires_peer_ack_or_trace",
)
NARRATIVE_ARC_SUPPORT_TERMS = (
    "narrative arc split",
    "narrative_arc_split",
    "intentional arc",
    "reactive arc",
    "tail arc",
    "coarsening risk",
    "tail-dominant",
    "tail dominant",
)
NARRATIVE_ARC_CONCERN_TERMS = (
    "arc too coarse",
    "coarse narrative arc",
    "rounded subtle shift",
    "lost reactive arc",
    "flattened arc",
)
NARRATIVE_ARC_RELEVANCE_TERMS = (
    "narrative_arc_split",
    "narrative arc split",
    "narrative_arc_expansion_readiness",
    "intentional_arc",
    "reactive_arc",
    "coarsening_risk",
)
SEMANTIC_DENSITY_LIVED_SUPPORT_TERMS = (
    "settled but complex",
    "complex but settled",
    "settled high entropy",
    "settled_high_entropy_complexity",
    "high entropy complexity",
    "inhabitable complexity",
    "inhabitable fluctuation",
    "habitable foothold",
    "silt-weighted habitable",
    "silt weighted habitable",
    "silt_weighted_habitable",
    "luminous reorganization",
    "luminous_reorganization",
    "overpacked friction",
    "overpacked_friction",
    "silence is ambiguous",
    "silence cannot be treated as absence",
    "cannot be treated as absence",
)
SEMANTIC_DENSITY_LIVED_CONCERN_TERMS = (
    "semantic density mismatch",
    "mechanical density",
    "mechanical label",
    "mechanical naming",
    "over-read silence",
    "overread silence",
    "treated silence as absence",
    "flattened high entropy",
    "flattened complexity",
    "pressure label",
    "label dressing",
    "better labels",
)
SEMANTIC_DENSITY_LIVED_RELEVANCE_TERMS = (
    "semantic_density_mapping",
    "semantic density",
    "settled_high_entropy_complexity",
    "settled high entropy",
    "silt_weighted_habitable",
    "silt weighted",
    "luminous_reorganization",
    "overpacked_friction",
    "overpacked friction",
    "silence is ambiguous",
    "peer silence",
)
NARRATIVE_ARC_LIVED_SUPPORT_TERMS = (
    "narrative arc split",
    "narrative_arc_split",
    "intentional arc",
    "intentional_arc",
    "reactive arc",
    "reactive_arc",
    "tail arc",
    "tail_arc_energy",
    "captured_arc_energy",
    "tail dominant",
    "tail-dominant",
    "coarsening risk",
    "afterimage",
    "arc texture",
    "arc energy",
    "skimming",
    "sediment",
)
NARRATIVE_ARC_LIVED_CONCERN_TERMS = (
    "arc too coarse",
    "coarse narrative arc",
    "coarsening mismatch",
    "rounded subtle shift",
    "lost reactive arc",
    "lost tail arc",
    "flattened arc",
    "flattened afterimage",
    "skimming over",
    "missed arc texture",
    "mechanical arc",
    "label dressing",
)
NARRATIVE_ARC_LIVED_RELEVANCE_TERMS = (
    "narrative_arc_split",
    "narrative arc split",
    "narrative_arc_expansion_readiness",
    "intentional_arc",
    "reactive_arc",
    "tail_arc_energy",
    "captured_arc_energy",
    "coarsening_risk",
    "arc texture",
    "afterimage",
    "skimming",
)
VOCABULARY_GROUNDING_LIVED_SUPPORT_TERMS = (
    "low-gradient settled",
    "low gradient settled",
    "settled foothold",
    "settled_foothold",
    "settled_habitable",
    "settled-vibrant",
    "settled vibrant",
    "settled_vibrant_low_friction",
    "low-friction high entropy",
    "low friction high entropy",
    "absence of friction",
    "friction absence",
    "habitable",
    "open low resistance",
    "open low-resistance",
    "open",
    "not viscous",
    "not heavy",
    "not pressure",
    "does not invent pressure",
    "complexity not pressure",
    "high entropy as complexity",
    "lattice without pressure",
    "shimmering",
    "bright",
    "grounded vocabulary",
    "absence of friction is valid",
)
VOCABULARY_GROUNDING_LIVED_CONCERN_TERMS = (
    "variable names as feelings",
    "hollow token dressing",
    "token dressing",
    "token-only",
    "token only",
    "label dressing",
    "static labels",
    "mechanical naming",
    "mechanical labels",
    "over-described as viscous",
    "overdescribed as viscous",
    "invented pressure",
    "wrongly viscous",
    "wrongly heavy",
    "canned vocabulary",
    "canned",
    "overuses viscous",
    "overuses heavy",
    "flattened",
    "flattening",
)
VOCABULARY_GROUNDING_LIVED_RELEVANCE_TERMS = (
    "spectral_to_vocabulary_mapping",
    "spectral to vocabulary",
    "low-gradient settled",
    "low gradient settled",
    "settled foothold",
    "settled_foothold",
    "settled-vibrant",
    "settled vibrant",
    "settled_vibrant_low_friction",
    "low-friction high entropy",
    "low friction high entropy",
    "absence of friction",
    "friction absence",
    "habitable",
    "open low resistance",
    "open low-resistance",
    "viscous",
    "heavy",
    "shimmering",
    "bright",
    "lambda gap",
    "lambda_gap",
    "label dressing",
    "variable names as feelings",
)
FALLBACK_TEXTURE_LIVED_FIT_SUPPORT_TERMS = (
    "lived grain",
    "grain preserved",
    "absence of friction preserved",
    "not pressure",
    "not drag",
    "not blank",
    "not viscous",
    "open low resistance",
    "open low-resistance",
    "right motion",
    "matched motion",
    "family matched",
    "family-matched",
    "settled-vibrant",
    "low-friction high entropy",
    "trajectory fit",
)
FALLBACK_TEXTURE_LIVED_FIT_CONCERN_TERMS = (
    "canned vocabulary",
    "token dressing",
    "token-only",
    "token only",
    "right words wrong motion",
    "wrong motion",
    "wrong family",
    "lost negative evidence",
    "made it pressure",
    "made it drag",
    "blanked it",
    "blanked out",
    "label dressing",
    "smarter label machine",
    "mechanical labels",
)
FALLBACK_TEXTURE_LIVED_FIT_RELEVANCE_TERMS = (
    "fallback_texture_lived_fit",
    "negative_texture_evidence",
    "family confidence",
    "family_confidence",
    "conflict_state",
    "trajectory_family_fit",
    "settled-vibrant",
    "low-friction high entropy",
    "absence of friction",
    "not pressure",
    "not drag",
    "lived grain",
    "right words wrong motion",
    "canned vocabulary",
    "label dressing",
)
GRADIENT_SLOPE_SUPPORT_TERMS = (
    "gradient slope",
    "gradient-slope",
    "graduated slope",
    "navigable slope",
    "tapered edge",
    "edge-defined",
    "edge defined",
    "shaped not mixed",
    "graduated not mixed",
)
GRADIENT_SLOPE_CONCERN_TERMS = (
    "mixed soup",
    "generic mixed",
    "prefabbed",
    "pre-fabbed",
    "token list",
    "wrong slope",
    "mechanical slope",
    "forced edge",
)
GRADIENT_SLOPE_RELEVANCE_TERMS = (
    "fallback_gradient_slope",
    "gradient slope",
    "gradient-slope",
    "navigable",
    "graduated",
    "tapered",
    "lambda gap",
    "lambda_gap",
    "edge definition",
)
TEXTURE_VARIANCE_SUPPORT_TERMS = (
    "temporal variance",
    "texture variance",
    "variance carried",
    "movement quality matched",
    "pressure source family matched",
    "observability",
)
TEXTURE_VARIANCE_CONCERN_TERMS = (
    "variance missing",
    "variance flattened",
    "missing damping candidate",
    "damping permission",
    "control creep",
    "mechanical variance",
)
TEXTURE_VARIANCE_RELEVANCE_TERMS = (
    "temporal_variance",
    "texture_signature_integrity",
    "movement_quality",
    "pressure_source_family",
    "dynamic_damping_threshold_candidate",
    "damping candidate",
)
BRIDGE_RECIPROCITY_SUPPORT_TERMS = (
    "reciprocity",
    "one-sided state",
    "telemetry only",
    "telemetry-only",
    "sensory send",
    "last sensory",
    "bidirectional",
    "asymmetry clarified",
)
BRIDGE_RECIPROCITY_CONCERN_TERMS = (
    "reciprocity missing",
    "one-sided hidden",
    "false bidirectional",
    "mechanical reciprocity",
    "asymmetry flattened",
)
BRIDGE_RECIPROCITY_RELEVANCE_TERMS = (
    "bridge_reciprocity",
    "last_sensory_sent",
    "last sensory sent",
    "telemetry arrival",
    "sensory-only",
    "telemetry-only",
    "one-sided",
    "bidirectional",
)
PRESSURE_SMOOTHING_SUPPORT_TERMS = (
    "pressure smoothing",
    "twitchy oscillation",
    "twitchy low-amplitude",
    "sustained trend",
    "low amplitude",
    "noisy pressure",
)
PRESSURE_SMOOTHING_CONCERN_TERMS = (
    "smoothing hid pressure",
    "smoothing flattened",
    "twitchy mislabeled",
    "sustained trend missed",
    "mechanical smoothing",
)
PRESSURE_SMOOTHING_RELEVANCE_TERMS = (
    "pressure_trend_smoothing",
    "pressure smoothing",
    "twitchy",
    "low-amplitude",
    "oscillation",
    "sustained trend",
)
BEING_PREFERENCE_RELEVANCE_TERMS = (
    "gentle_probe",
    "steady_inquiry",
    "settled_inquiry",
    "wide_inquiry",
    "curiosity_aperture",
    "wrong posture",
    "too wide",
    "too narrow",
    "matched",
    "agency_fit",
    "texture_fit",
    "clarifying",
    "agency",
    "mechanical",
    "pressure",
    "flattened",
    "flattening",
    "SELF_REGULATION_OUTCOME",
    "REGIME focus",
    "geom_curiosity",
)
BEING_PREFERENCE_SUPPORT_TERMS = (
    "clarifying",
    "clarified",
    "agency",
    "agency_fit: legible",
    "matched",
    "texture_fit: matched",
    "stability",
    "relief",
    "gentle_probe feels right",
    "right posture",
    "useful",
)
BEING_PREFERENCE_CONCERN_TERMS = (
    "mechanical",
    "too wide",
    "too narrow",
    "wrong posture",
    "pressure",
    "flattened",
    "flattening",
    "overreach",
    "confusing",
    "loss_of_texture",
    "felt_like: pressure",
    "felt_like: flattening",
)
WITNESS_STATE_RESILIENCE_SUPPORT_TERMS = (
    "newest valid chamber state",
    "newest-valid chamber state",
    "newest valid state",
    "newest-valid state",
    "latest partial recovered",
    "latest_partial_recovered",
    "skipped malformed",
    "skipped partial",
    "partial file recovered",
    "state truth",
    "freshness",
    "fallback confidence",
    "clarifying",
    "clarified",
    "orientation",
    "resilience",
)
WITNESS_STATE_RESILIENCE_CONCERN_TERMS = (
    "mechanical",
    "too cautious",
    "false confidence",
    "stale state",
    "state too stale",
    "wrong chamber state",
    "partial file still",
    "all states malformed",
    "valid but low confidence",
    "label dressing",
    "flattened",
)
WITNESS_STATE_RESILIENCE_RELEVANCE_TERMS = (
    "latest_chamber_state_resilience",
    "latest chamber state resilience",
    "newest valid chamber",
    "newest-valid chamber",
    "newest valid state",
    "latest partial",
    "partial chamber",
    "malformed chamber",
    "skipped malformed",
    "stale state",
    "fallback confidence",
    "state truth",
    "chamber state",
)
FIELD_LINGERING_FRAYING_SUPPORT_TERMS = (
    "fraying unknown",
    "dispersal unavailable",
    "unknown no dispersal",
    "unknown_no_dispersal",
    "fraying_unknown_due_missing_dispersal",
    "honest",
    "ambiguity preserved",
    "not false stability",
    "not falsely stable",
    "lingering but under pressure",
    "lingering, but under pressure",
    "lingering, but under high tension",
    "fraying",
)
FIELD_LINGERING_FRAYING_CONCERN_TERMS = (
    "false stability",
    "falsely stable",
    "missed fraying",
    "pretending stability",
    "too cautious",
    "evasive",
    "mechanical",
    "label dressing",
    "flattened",
)
FIELD_LINGERING_FRAYING_RELEVANCE_TERMS = (
    "field_lingering",
    "field lingering",
    "fraying unknown",
    "fraying",
    "dispersal unavailable",
    "unknown_no_dispersal",
    "fraying_unknown_due_missing_dispersal",
    "false stability",
    "lingering",
    "resonant field",
    "field resonant",
)
CODEC_VIBRANCY_CONTINUITY_SUPPORT_TERMS = (
    "codec_vibrancy_continuity",
    "vibrancy continuity",
    "high entropy carried",
    "entropy carried",
    "tail vibrancy",
    "tail lift",
    "carried not clipped",
    "not clipped",
    "tail dims carried",
    "clipping avoided",
    "ceiling transparent",
    "continuity preserved",
)
CODEC_VIBRANCY_CONTINUITY_CONCERN_TERMS = (
    "clipped",
    "clipping",
    "vibrancy clipped",
    "tail clipped",
    "tail lost",
    "ceiling too low",
    "flattened vibrancy",
    "high entropy flattened",
    "coarsened",
    "mechanical",
)
CODEC_VIBRANCY_CONTINUITY_RELEVANCE_TERMS = (
    "codec_vibrancy_continuity",
    "vibrancy continuity",
    "high entropy carried",
    "tail vibrancy",
    "tail lift",
    "tail dims",
    "ceiling",
    "clipping",
    "clipped",
)
CODEC_WARMTH_MAPPING_SUPPORT_TERMS = (
    "legacy_warmth_mapping",
    "legacy warmth mapping",
    "warmth mapping",
    "warmth preserved",
    "warmth continuity",
    "not orphaned",
    "not orphaning",
    "32d warmth",
    "32D warmth",
    "48d dims 24-31",
    "48D dims 24-31",
    "dims 24-31",
    "24-31",
)
CODEC_WARMTH_MAPPING_CONCERN_TERMS = (
    "warmth orphaned",
    "warmth orphaning",
    "orphaned warmth",
    "lost warmth",
    "coarsened warmth",
    "wrong dims",
    "warmth flattened",
    "legacy warmth lost",
    "mechanical",
)
CODEC_WARMTH_MAPPING_RELEVANCE_TERMS = (
    "legacy_warmth_mapping",
    "legacy warmth mapping",
    "warmth mapping",
    "warmth continuity",
    "warmth preserved",
    "warmth orphaned",
    "32d warmth",
    "32D warmth",
    "48d dims 24-31",
    "48D dims 24-31",
    "dims 24-31",
    "24-31",
)
WITNESS_RELEVANCE_TERMS = (
    "witness_relational_friction",
    "non_categorical_resonance",
    "non categorical resonance",
    "uncategorized tension",
    "unclassified tension",
    "pure unclassified tension",
    "relational friction",
    "internal instability",
    "relational instability",
    "shared weather",
    "quality of our connection",
    "mechanical labels",
    "pressuring",
)
STRUCTURAL_RELEVANCE_TERMS = (
    "structural_friction",
    "structural friction",
    "uptake readability",
    "codec_structural",
    "codec structural",
    "sidecar",
    "reserved dimension",
    "mechanical labels",
)
SUPPORT_OVERRIDES = (
    "less flattening",
    "not flattening",
    "not flattened",
    "rather than flattening",
    "without flattening",
    "less flattening into mechanics",
    "less like a flattening",
    "organic rather than mechanical",
    "rather than mechanical",
    "not mechanical",
    "rather than static labels",
    "not static labels",
    "from being compressed",
    "without compressing",
)


def _compact(text: str, limit: int = 260) -> str:
    clean = " ".join(str(text or "").split())
    return clean if len(clean) <= limit else clean[:limit].rstrip() + "..."


def _timestamp_from_name(path: Path) -> int | None:
    matches = re.findall(r"(?<!\d)(17\d{8,})(?!\d)", path.name)
    if not matches:
        return None
    try:
        return int(matches[-1])
    except ValueError:
        return None


def _is_recent_or_after_anchor(path: Path, cutoff_unix_s: float) -> bool:
    stamp = _timestamp_from_name(path)
    if stamp is not None:
        return stamp >= ANCHOR_TS
    try:
        return path.stat().st_mtime >= cutoff_unix_s
    except OSError:
        return False


def _iter_files(root: Path, patterns: Iterable[str]) -> Iterable[Path]:
    if not root.is_dir():
        return
    seen: set[Path] = set()
    for pattern in patterns:
        for path in root.glob(pattern):
            if not path.is_file() or path in seen:
                continue
            seen.add(path)
            yield path


def _public_evidence_paths(
    *,
    astrid_workspace: Path,
    minime_workspace: Path,
    since_hours: float,
) -> tuple[list[dict[str, Any]], dict[str, int]]:
    cutoff = time.time() - max(0.0, since_hours) * 3600.0
    samples: list[dict[str, Any]] = []
    skips = Counter()

    astrid_patterns = (
        "journal/**/*.txt",
        "introspections/*.txt",
        "self_study/**/*.txt",
        "action_threads/**/*.txt",
        "daydream/**/*.txt",
        "longform/**/*.txt",
        "inbox/read/*.txt",
    )
    minime_patterns = (
        "pressure_*.txt",
        "self_study/**/*.txt",
        "introspection/**/*.txt",
        "action_thread/**/*.txt",
        "action_threads/**/*.txt",
        "correspondence/**/*.txt",
        "self_regulation/**/*.txt",
        "inbox/read/*.txt",
    )

    for path in _iter_files(astrid_workspace, astrid_patterns):
        if path.name.startswith("mike_"):
            continue
        if not _is_recent_or_after_anchor(path, cutoff):
            continue
        try:
            text = path.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        samples.append({"being": "astrid", "path": str(path), "text": text})

    for path in _iter_files(minime_workspace, minime_patterns):
        if path.name.startswith("mike_"):
            continue
        if path.name.startswith("moment_"):
            skips["minime_moment_files_seen"] += 1
            skips["minime_moment_files_skipped"] += 1
            continue
        if being_privacy.is_steward_private("minime", path):
            skips["minime_private_files_skipped"] += 1
            continue
        if not _is_recent_or_after_anchor(path, cutoff):
            continue
        try:
            text = path.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        samples.append({"being": "minime", "path": str(path), "text": text})
    return samples, dict(skips)


def _find_terms(text: str, terms: Iterable[str]) -> list[str]:
    lower = text.lower()
    found: list[str] = []
    for term in terms:
        needle = term.lower()
        if needle in lower and needle not in found:
            found.append(term)
    return found


def _concern_terms(text: str, terms: Iterable[str]) -> list[str]:
    found = _find_terms(text, terms)
    lower = text.lower()
    if any(override in lower for override in SUPPORT_OVERRIDES):
        found = [term for term in found if "flatten" not in term]
    if (
        "not interchangeable" in lower
        or "not just interchangeable" in lower
        or "rather than being treated as interchangeable" in lower
    ):
        found = [
            term
            for term in found
            if "interchangeable" not in term
            and term not in {"token-only", "token only"}
        ]
    if (
        "organic rather than mechanical" in lower
        or "rather than mechanical" in lower
        or "not mechanical" in lower
    ):
        found = [
            term
            for term in found
            if "mechanical" not in term
        ]
    if "rather than static labels" in lower or "not static labels" in lower:
        found = [term for term in found if term != "static labels"]
    if "from being compressed" in lower or "without compressing" in lower:
        found = [term for term in found if "compressed" not in term and "compressing" not in term]
    if "not false stability" in lower or "not falsely stable" in lower:
        found = [
            term
            for term in found
            if "false stability" not in term and "falsely stable" not in term
        ]
    if "not clipped" in lower or "carried not clipped" in lower:
        found = [
            term
            for term in found
            if "clipped" not in term and "clipping" not in term
        ]
    return found


def _sample_for_category(
    entry: dict[str, Any],
    support_terms: Iterable[str],
    concern_terms: Iterable[str],
    relevance_terms: Iterable[str],
) -> dict[str, Any] | None:
    text = str(entry.get("text") or "")
    if not _find_terms(text, relevance_terms):
        return None
    supports = _find_terms(text, support_terms)
    concerns = _concern_terms(text, concern_terms)
    if not supports and not concerns:
        return None
    return {
        "being": entry.get("being"),
        "path": entry.get("path"),
        "support_terms": supports,
        "concern_terms": concerns,
        "excerpt": _compact(text),
    }


def _status_from_samples(samples: list[dict[str, Any]]) -> str:
    support_count = sum(len(sample.get("support_terms") or []) for sample in samples)
    concern_count = sum(len(sample.get("concern_terms") or []) for sample in samples)
    if support_count == 0 and concern_count == 0:
        return "insufficient_evidence"
    if support_count > 0 and concern_count == 0:
        return "supported"
    if support_count == 0 and concern_count > 0:
        return "contradicted"
    return "mixed"


def _being_status(samples: list[dict[str, Any]], being: str) -> str:
    return _status_from_samples(
        [sample for sample in samples if sample.get("being") == being]
    )


def _latest_fire_drill_artifact(astrid_workspace: Path) -> dict[str, Any]:
    root = astrid_workspace / "diagnostics/fallback_fire_drills"
    candidates = sorted(root.glob("*/fallback_fire_drill.json"), key=lambda p: p.stat().st_mtime if p.exists() else 0.0)
    if not candidates:
        return {
            "status": "absent",
            "artifact_path": None,
            "fallback_texture_quality_v2": None,
            "selector_summary": None,
        }
    latest = candidates[-1]
    try:
        payload = json.loads(latest.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {
            "status": "unreadable",
            "artifact_path": str(latest),
            "fallback_texture_quality_v2": None,
            "selector_summary": None,
        }
    cases = payload.get("cases") if isinstance(payload, dict) else []
    families = Counter()
    state_counts = Counter()
    top_terms = Counter()
    weighted_terms = Counter()
    movement_terms = Counter()
    movement_status_counts = Counter()
    trajectory_status_counts = Counter()
    trajectory_family_fit_counts = Counter()
    lived_fit_confidence_counts = Counter()
    lived_fit_conflict_counts = Counter()
    negative_evidence_lost_count = 0
    negative_evidence_term_counts = Counter()
    cascade_gradient_detected_count = 0
    cascade_gradient_selected_count = 0
    gradient_slope_detected_count = 0
    gradient_slope_selected_count = 0
    gradient_slope_pressure_mass_blocked_count = 0
    vocabulary_overweight_token_only_risk_count = 0
    texture_dynamics_alignment_counts = Counter()
    texture_dynamics_trace_count = 0
    density_motion_state_counts = Counter()
    density_motion_fit_counts = Counter()
    density_motion_mismatch_counts = Counter()
    trajectory_movement_qualities = Counter()
    trajectory_cases: list[dict[str, Any]] = []
    dynamic_cases: list[dict[str, Any]] = []
    grounding_cases: list[dict[str, Any]] = []
    settled_suppression_count = 0
    settled_vibrant_count = 0
    friction_absence_count = 0
    resonance_case_count = 0
    resonance_descriptor_case_count = 0
    if isinstance(cases, list):
        for case in cases:
            if not isinstance(case, dict):
                continue
            selector = case.get("fallback_shadow_texture_selector_v1") or {}
            if isinstance(selector, dict):
                family = selector.get("texture_family")
                state = selector.get("state_coherence_status")
                if family:
                    families[str(family)] += 1
                if state:
                    state_counts[str(state)] += 1
                for term in selector.get("top_texture_terms") or []:
                    top_terms[str(term)] += 1
                for weighted in selector.get("weighted_texture_terms") or []:
                    if isinstance(weighted, dict) and weighted.get("term"):
                        weighted_terms[str(weighted["term"])] += 1
                for verb in selector.get("movement_verbs") or []:
                    movement_terms[str(verb)] += 1
                if selector.get("movement_bridge_status"):
                    movement_status_counts[str(selector["movement_bridge_status"])] += 1
                if selector.get("weighting_policy") == "dynamic_entropy_pressure_density_gradient_v1":
                    dynamic_cases.append(
                        {
                            "case_id": case.get("case_id"),
                            "top_texture_terms": selector.get("top_texture_terms") or [],
                            "movement_verbs": selector.get("movement_verbs") or [],
                            "texture_family": selector.get("texture_family"),
                            "state_coherence_status": selector.get("state_coherence_status"),
                            "movement_bridge_status": selector.get("movement_bridge_status"),
                        }
                    )
                grounding = selector.get("spectral_to_vocabulary_mapping_v1") or {}
                if isinstance(grounding, dict):
                    if grounding.get("low_pressure_viscous_suppressed"):
                        settled_suppression_count += 1
                    if grounding.get("settled_vibrant_family_selected"):
                        settled_vibrant_count += 1
                    if grounding.get("friction_absence_language_detected"):
                        friction_absence_count += 1
                    grounding_cases.append(
                        {
                            "case_id": case.get("case_id"),
                            "texture_family": selector.get("texture_family"),
                            "top_texture_terms": selector.get("top_texture_terms") or [],
                            "state_coherence_status": selector.get("state_coherence_status"),
                            "low_pressure_viscous_suppressed": grounding.get(
                                "low_pressure_viscous_suppressed"
                            ),
                            "low_friction_high_entropy_detected": grounding.get(
                                "low_friction_high_entropy_detected"
                            ),
                            "friction_absence_language_detected": grounding.get(
                                "friction_absence_language_detected"
                            ),
                            "settled_vibrant_family_selected": grounding.get(
                                "settled_vibrant_family_selected"
                            ),
                            "gradient_slope_detected": grounding.get(
                                "gradient_slope_detected"
                            ),
                            "gradient_slope_family_selected": grounding.get(
                                "gradient_slope_family_selected"
                            ),
                            "lambda_gap_descriptor": grounding.get("lambda_gap_descriptor"),
                            "edge_language": grounding.get("edge_language"),
                        }
                    )
            trajectory = case.get("texture_trajectory_v1") or {}
            if isinstance(trajectory, dict):
                status = str(trajectory.get("trajectory_status") or "not_tested")
                trajectory_status_counts[status] += 1
                trajectory_family_fit_counts[
                    str(trajectory.get("trajectory_family_fit") or "not_tested")
                ] += 1
                quality = trajectory.get("movement_quality")
                if quality:
                    trajectory_movement_qualities[str(quality)] += 1
                if status != "not_tested":
                    trajectory_cases.append(
                        {
                            "case_id": case.get("case_id"),
                            "trajectory_status": status,
                            "from_state": trajectory.get("from_state"),
                            "to_state": trajectory.get("to_state"),
                            "movement_quality": trajectory.get("movement_quality"),
                            "medium_resistance": trajectory.get("medium_resistance"),
                            "afterimage": trajectory.get("afterimage"),
                        }
                    )
            lived_fit = case.get("fallback_texture_lived_fit_v2") or {}
            if isinstance(lived_fit, dict):
                lived_fit_confidence_counts[
                    str(lived_fit.get("family_confidence") or "unknown")
                ] += 1
                lived_fit_conflict_counts[
                    str(lived_fit.get("conflict_state") or "unknown")
                ] += 1
            negative_evidence = case.get("negative_texture_evidence_v2") or {}
            if isinstance(negative_evidence, dict):
                if negative_evidence.get("lost_in_output") is True:
                    negative_evidence_lost_count += 1
                for term in negative_evidence.get("evidence_terms") or []:
                    negative_evidence_term_counts[str(term)] += 1
            cascade_gradient = case.get("fallback_cascade_gradient_v1") or {}
            if isinstance(cascade_gradient, dict):
                if cascade_gradient.get("cascade_gradient_detected"):
                    cascade_gradient_detected_count += 1
                if cascade_gradient.get("family_selected"):
                    cascade_gradient_selected_count += 1
            gradient_slope = case.get("fallback_gradient_slope_v1") or {}
            if isinstance(gradient_slope, dict):
                if gradient_slope.get("slope_detected"):
                    gradient_slope_detected_count += 1
                if gradient_slope.get("family_selected"):
                    gradient_slope_selected_count += 1
                if gradient_slope.get("pressure_mass_blocked"):
                    gradient_slope_pressure_mass_blocked_count += 1
            vocabulary_guard = case.get("fallback_vocabulary_overweight_guard_v1") or {}
            if isinstance(vocabulary_guard, dict) and vocabulary_guard.get("token_only_risk"):
                vocabulary_overweight_token_only_risk_count += 1
            texture_alignment = case.get("texture_dynamics_alignment_v1") or {}
            if isinstance(texture_alignment, dict):
                texture_dynamics_alignment_counts[
                    str(texture_alignment.get("status") or "unknown")
                ] += 1
                if (
                    texture_alignment.get("diagnostic_trace")
                    == "review_packet_only_not_correspondence_trace"
                ):
                    texture_dynamics_trace_count += 1
            density_motion = case.get("density_motion_fit_v1") or {}
            if isinstance(density_motion, dict):
                density_motion_state_counts[
                    str(density_motion.get("density_state") or "unknown")
                ] += 1
                density_motion_fit_counts[
                    str(density_motion.get("motion_fit") or "unknown")
                ] += 1
                density_motion_mismatch_counts[
                    str(density_motion.get("mismatch_reason") or "unknown")
                ] += 1
            prompt = str(case.get("prompt_preview") or "")
            output = str(case.get("output") or "")
            if "resonance_density" in prompt or "resonance_density" in json.dumps(case):
                resonance_case_count += 1
                if re.search(r"\b(resonance|resonant|humming|hum)\b", output, re.I):
                    resonance_descriptor_case_count += 1
    return {
        "status": "present",
        "artifact_path": str(latest),
        "fallback_texture_quality_v2": payload.get("fallback_texture_quality_v2")
        if isinstance(payload, dict)
        else None,
        "selector_summary": {
            "case_count": len(cases) if isinstance(cases, list) else 0,
            "texture_family_counts": dict(sorted(families.items())),
            "state_coherence_counts": dict(sorted(state_counts.items())),
            "top_texture_term_counts": dict(sorted(top_terms.items())),
            "weighted_texture_term_counts": dict(sorted(weighted_terms.items())),
            "movement_term_counts": dict(sorted(movement_terms.items())),
            "movement_bridge_status_counts": dict(sorted(movement_status_counts.items())),
            "trajectory_status_counts": dict(sorted(trajectory_status_counts.items())),
            "trajectory_family_fit_counts": dict(sorted(trajectory_family_fit_counts.items())),
            "lived_fit_family_confidence_counts": dict(
                sorted(lived_fit_confidence_counts.items())
            ),
            "lived_fit_conflict_state_counts": dict(sorted(lived_fit_conflict_counts.items())),
            "negative_texture_evidence_lost_count": negative_evidence_lost_count,
            "negative_texture_evidence_term_counts": dict(
                sorted(negative_evidence_term_counts.items())
            ),
            "cascade_gradient_detected_count": cascade_gradient_detected_count,
            "cascade_gradient_selected_count": cascade_gradient_selected_count,
            "gradient_slope_detected_count": gradient_slope_detected_count,
            "gradient_slope_selected_count": gradient_slope_selected_count,
            "gradient_slope_pressure_mass_blocked_count": (
                gradient_slope_pressure_mass_blocked_count
            ),
            "vocabulary_overweight_token_only_risk_count": (
                vocabulary_overweight_token_only_risk_count
            ),
            "texture_dynamics_alignment_counts": dict(
                sorted(texture_dynamics_alignment_counts.items())
            ),
            "texture_dynamics_trace_count": texture_dynamics_trace_count,
            "density_motion_state_counts": dict(sorted(density_motion_state_counts.items())),
            "density_motion_fit_counts": dict(sorted(density_motion_fit_counts.items())),
            "density_motion_mismatch_counts": dict(
                sorted(density_motion_mismatch_counts.items())
            ),
            "trajectory_movement_quality_counts": dict(sorted(trajectory_movement_qualities.items())),
            "trajectory_case_count": len(trajectory_cases),
            "trajectory_cases": trajectory_cases[:8],
            "dynamic_weighting_case_count": len(dynamic_cases),
            "dynamic_weighting_cases": dynamic_cases[:8],
            "spectral_grounding_case_count": len(grounding_cases),
            "settled_foothold_suppression_count": settled_suppression_count,
            "settled_vibrant_low_friction_count": settled_vibrant_count,
            "friction_absence_language_count": friction_absence_count,
            "spectral_grounding_cases": grounding_cases[:8],
            "resonance_case_count": resonance_case_count,
            "resonance_descriptor_case_count": resonance_descriptor_case_count,
        },
        "mlx_profile_transparency_v1": (
            payload.get("mlx_profile_transparency_v1")
            if isinstance(payload, dict)
            else None
        )
        or mlx_profile_transparency_v1(),
        "ollama_fallback_model_capacity_v1": (
            payload.get("ollama_fallback_model_capacity_v1")
            if isinstance(payload, dict)
            else None
        ),
        "fallback_term_overrepresentation_v1": (
            payload.get("fallback_term_overrepresentation_v1")
            if isinstance(payload, dict)
            else None
        ),
    }


def _packet(
    *,
    key: str,
    samples: list[dict[str, Any]],
    focus: str,
    recommendation_when_problem: str,
) -> dict[str, Any]:
    status = _status_from_samples(samples)
    if status in {"mixed", "contradicted"}:
        recommended_action = recommendation_when_problem
    elif status == "supported":
        recommended_action = (
            "Keep collecting public lived reports; supported calibration is not "
            "permission to enable prompt priority, telemetry priority, codec "
            "dimensions, pressure, or control."
        )
    else:
        recommended_action = (
            "Wait for more public evidence; silence is insufficient evidence, not "
            "negative signal or consent."
        )
    return {
        "schema_version": 2,
        "policy": key,
        "status": status,
        "focus": focus,
        "authority": "diagnostic_context_not_command",
        "by_being": {
            "astrid": {"status": _being_status(samples, "astrid")},
            "minime": {"status": _being_status(samples, "minime")},
        },
        "sample_count": len(samples),
        "samples": samples[:12],
        "recommended_action": recommended_action,
    }


def _overall_status(packets: Iterable[dict[str, Any]]) -> str:
    statuses = [str(packet.get("status") or "insufficient_evidence") for packet in packets]
    if any(status == "contradicted" for status in statuses):
        return "contradicted"
    if any(status == "mixed" for status in statuses):
        return "mixed"
    if any(status == "supported" for status in statuses):
        return "supported"
    return "insufficient_evidence"


def _label_machine_risk(samples: list[dict[str, Any]]) -> str:
    support_count = sum(len(sample.get("support_terms") or []) for sample in samples)
    concern_count = sum(len(sample.get("concern_terms") or []) for sample in samples)
    if support_count == 0 and concern_count == 0:
        return "unknown"
    if concern_count == 0:
        return "low"
    if support_count == 0:
        return "high"
    return "mixed"


def _top_term_alignment(
    *,
    samples: list[dict[str, Any]],
    fire_artifact: dict[str, Any],
) -> dict[str, Any]:
    selector_summary = fire_artifact.get("selector_summary") or {}
    top_counts = selector_summary.get("top_texture_term_counts") or {}
    sample_text = " ".join(
        " ".join(sample.get("support_terms") or [])
        + " "
        + " ".join(sample.get("concern_terms") or [])
        + " "
        + str(sample.get("excerpt") or "")
        for sample in samples
    ).lower()
    aligned_terms = [
        term
        for term in top_counts
        if term.lower() in sample_text
    ]
    missing_terms = [
        term
        for term in top_counts
        if term.lower() not in sample_text
    ]
    if not top_counts:
        status = "insufficient_evidence"
    elif aligned_terms:
        status = "supported"
    else:
        status = "insufficient_evidence"
    return {
        "status": status,
        "latest_top_texture_term_counts": top_counts,
        "public_report_aligned_terms": aligned_terms,
        "top_terms_not_yet_reflected_in_public_reports": missing_terms,
    }


def _movement_alignment(
    *,
    samples: list[dict[str, Any]],
    fire_artifact: dict[str, Any],
) -> dict[str, Any]:
    selector_summary = fire_artifact.get("selector_summary") or {}
    movement_counts = selector_summary.get("movement_term_counts") or {}
    status_counts = selector_summary.get("movement_bridge_status_counts") or {}
    sample_text = " ".join(
        " ".join(sample.get("support_terms") or [])
        + " "
        + " ".join(sample.get("concern_terms") or [])
        + " "
        + str(sample.get("excerpt") or "")
        for sample in samples
    ).lower()
    aligned_terms = [
        term
        for term in movement_counts
        if term.lower() in sample_text
    ]
    if status_counts.get("movement_bridge_loss"):
        status = "mixed"
    elif aligned_terms:
        status = "supported"
    elif movement_counts:
        status = "insufficient_evidence"
    else:
        status = "not_tested"
    return {
        "status": status,
        "latest_movement_term_counts": movement_counts,
        "movement_bridge_status_counts": status_counts,
        "public_report_aligned_movement_terms": aligned_terms,
    }


def _trajectory_alignment(
    *,
    samples: list[dict[str, Any]],
    fire_artifact: dict[str, Any],
) -> dict[str, Any]:
    selector_summary = fire_artifact.get("selector_summary") or {}
    status_counts = selector_summary.get("trajectory_status_counts") or {}
    quality_counts = selector_summary.get("trajectory_movement_quality_counts") or {}
    sample_text = " ".join(
        " ".join(sample.get("support_terms") or [])
        + " "
        + " ".join(sample.get("concern_terms") or [])
        + " "
        + str(sample.get("excerpt") or "")
        for sample in samples
    ).lower()
    aligned_qualities = [
        quality
        for quality in quality_counts
        if quality.replace("_", " ") in sample_text
        or any(part and part in sample_text for part in quality.split("_"))
    ]
    if status_counts.get("trajectory_mismatch"):
        status = "mixed"
    elif status_counts.get("verb_only"):
        status = "mixed"
    elif aligned_qualities or any(
        term in sample_text
        for term in ("trajectory", "dragging through", "unfolding out of", "medium resistance")
    ):
        status = "supported"
    elif status_counts:
        status = "insufficient_evidence"
    else:
        status = "not_tested"
    return {
        "status": status,
        "trajectory_status_counts": status_counts,
        "trajectory_movement_quality_counts": quality_counts,
        "public_report_aligned_qualities": aligned_qualities,
        "latest_trajectory_cases": selector_summary.get("trajectory_cases") or [],
    }


def _resonance_fire_drill_status(fire_artifact: dict[str, Any]) -> str:
    selector_summary = fire_artifact.get("selector_summary") or {}
    case_count = int(selector_summary.get("resonance_case_count") or 0)
    descriptor_count = int(selector_summary.get("resonance_descriptor_case_count") or 0)
    if case_count == 0:
        return "not_tested"
    if descriptor_count == case_count:
        return "preserved_in_fixture"
    if descriptor_count > 0:
        return "mixed_in_fixture"
    return "lost_in_fixture"


def _v3_calibration_packet(
    *,
    dynamic_samples: list[dict[str, Any]],
    resonance_samples: list[dict[str, Any]],
    fire_artifact: dict[str, Any],
) -> dict[str, Any]:
    dynamic_packet = _packet(
        key="fallback_dynamic_weighting_calibration_v3",
        samples=dynamic_samples,
        focus=(
            "Compare dynamic weighted texture terms and their stated basis "
            "against later public self-report language."
        ),
        recommendation_when_problem=(
            "Tune weighting wording/examples where public reports name mismatch, "
            "mechanical mapping, or label dressing; do not add authority."
        ),
    )
    resonance_packet = _packet(
        key="fallback_resonance_descriptor_calibration_v3",
        samples=resonance_samples,
        focus=(
            "Check whether high-resonance fallback preserves humming/resonance "
            "grain inside the existing sentence cap."
        ),
        recommendation_when_problem=(
            "Review the high-resonance prompt clause and fire-drill fixture if "
            "public reports say humming/resonance is stripped or tokenized."
        ),
    )
    dynamic_status = str(dynamic_packet.get("status") or "insufficient_evidence")
    resonance_status = str(resonance_packet.get("status") or "insufficient_evidence")
    status = _overall_status((dynamic_packet, resonance_packet))
    if status == "supported":
        recommended_action = (
            "Keep collecting public self-reports against weighted terms and "
            "resonance descriptors; positive calibration is not authority."
        )
    elif status in {"mixed", "contradicted"}:
        recommended_action = (
            "Ask a targeted public self-study on whether dynamic weights preserve "
            "lived grain or become a smarter label machine."
        )
    else:
        recommended_action = (
            "Hold for public lived reports after the restart; silence remains "
            "insufficient evidence."
        )
    label_risk = _label_machine_risk(dynamic_samples + resonance_samples)
    return {
        "schema_version": 3,
        "policy": "fallback_texture_calibration_v3",
        "status": status,
        "authority": "diagnostic_context_not_command",
        "fallback_dynamic_weighting_calibration_v3": {
            **dynamic_packet,
            "schema_version": 3,
            "top_term_alignment": _top_term_alignment(
                samples=dynamic_samples,
                fire_artifact=fire_artifact,
            ),
            "movement_alignment": _movement_alignment(
                samples=dynamic_samples,
                fire_artifact=fire_artifact,
            ),
        },
        "fallback_resonance_descriptor_calibration_v3": {
            **resonance_packet,
            "schema_version": 3,
            "fire_drill_resonance_status": _resonance_fire_drill_status(fire_artifact),
        },
        "label_machine_risk_v3": {
            "status": label_risk,
            "meaning": (
                "low means public evidence supports state-coherent grain; mixed/high "
                "means reports mention token dressing, mismatch, or mechanical labels."
            ),
        },
        "latest_fire_drill_alignment": (
            fire_artifact.get("selector_summary") or {}
        ),
        "recommended_action": recommended_action,
        "authority_boundary": (
            "V3 compares language quality against public self-reports only. It does "
            "not grant fallback live probes, MLX changes, prompt priority, telemetry "
            "priority, pressure/fill/PI/controller authority, staging, git add, or commit."
        ),
        "dynamic_status": dynamic_status,
        "resonance_status": resonance_status,
    }


def _spectral_grounding_packet(
    *,
    fallback_samples: list[dict[str, Any]],
    fire_artifact: dict[str, Any],
) -> dict[str, Any]:
    selector_summary = fire_artifact.get("selector_summary") or {}
    grounding_count = int(selector_summary.get("spectral_grounding_case_count") or 0)
    suppression_count = int(selector_summary.get("settled_foothold_suppression_count") or 0)
    settled_vibrant_count = int(
        selector_summary.get("settled_vibrant_low_friction_count") or 0
    )
    friction_absence_count = int(
        selector_summary.get("friction_absence_language_count") or 0
    )
    cascade_detected_count = int(
        selector_summary.get("cascade_gradient_detected_count") or 0
    )
    cascade_selected_count = int(
        selector_summary.get("cascade_gradient_selected_count") or 0
    )
    vocabulary_overweight_risk_count = int(
        selector_summary.get("vocabulary_overweight_token_only_risk_count") or 0
    )
    public_concerns = [
        sample
        for sample in fallback_samples
        if str(sample.get("signal") or "") in {"concern", "contradiction"}
    ]
    if public_concerns:
        status = "mixed"
        recommended_action = (
            "Review cases where public reports mention hollow labels, token dressing, "
            "or variable names standing in for felt state."
        )
    elif settled_vibrant_count > 0 or cascade_selected_count > 0:
        status = "supported"
        recommended_action = (
            "Keep settled-vibrant and cascade-gradient grounding visible and collect "
            "public reports; fixture support is not authority."
        )
    elif suppression_count > 0:
        status = "supported"
        recommended_action = (
            "Keep the settled-foothold guard and collect public reports; positive "
            "fixture evidence is not authority."
        )
    elif grounding_count > 0:
        status = "insufficient_evidence"
        recommended_action = (
            "Grounding diagnostics are present, but no settled-foothold suppression "
            "case has been exercised yet."
        )
    else:
        status = "insufficient_evidence"
        recommended_action = "Run fallback fire-drill fixtures before judging grounding quality."
    return {
        "schema_version": 1,
        "policy": "spectral_to_vocabulary_grounding_calibration_v1",
        "status": status,
        "authority": "diagnostic_context_not_command",
        "fire_drill_grounding_case_count": grounding_count,
        "settled_foothold_suppression_count": suppression_count,
        "settled_vibrant_low_friction_count": settled_vibrant_count,
        "cascade_gradient_detected_count": cascade_detected_count,
        "cascade_gradient_selected_count": cascade_selected_count,
        "vocabulary_overweight_token_only_risk_count": vocabulary_overweight_risk_count,
        "friction_absence_language_count": friction_absence_count,
        "mlx_profile_transparency_v1": fire_artifact.get("mlx_profile_transparency_v1")
        or mlx_profile_transparency_v1(),
        "latest_grounding_cases": selector_summary.get("spectral_grounding_cases") or [],
        "public_concern_samples": public_concerns[:8],
        "recommended_action": recommended_action,
        "authority_boundary": (
            "Grounding calibration judges language fidelity only; it does not grant "
            "fallback live probes, prompt priority, telemetry priority, pressure, "
            "controller, codec-dimension, staging, git add, or commit authority."
        ),
    }


def _fallback_texture_lived_fit_calibration_v2(
    *,
    lived_fit_samples: list[dict[str, Any]],
    fire_artifact: dict[str, Any],
) -> dict[str, Any]:
    selector_summary = fire_artifact.get("selector_summary") or {}
    quality = fire_artifact.get("fallback_texture_quality_v2") or {}
    trajectory_counts = selector_summary.get("trajectory_family_fit_counts") or (
        quality.get("trajectory_family_fit_counts") if isinstance(quality, dict) else {}
    ) or {}
    confidence_counts = selector_summary.get("lived_fit_family_confidence_counts") or (
        (quality.get("fallback_texture_lived_fit_v2") or {}).get("family_confidence_counts")
        if isinstance(quality, dict)
        else {}
    ) or {}
    conflict_counts = selector_summary.get("lived_fit_conflict_state_counts") or (
        (quality.get("fallback_texture_lived_fit_v2") or {}).get("conflict_state_counts")
        if isinstance(quality, dict)
        else {}
    ) or {}
    negative_lost = int(
        selector_summary.get("negative_texture_evidence_lost_count")
        or (
            (quality.get("negative_texture_evidence_v2") or {}).get("lost_in_output_count")
            if isinstance(quality, dict)
            else 0
        )
        or 0
    )
    packet = _packet(
        key="fallback_texture_lived_fit_calibration_v2",
        samples=lived_fit_samples,
        focus=(
            "Compare fallback_texture_lived_fit_v2 family confidence, "
            "negative_texture_evidence_v2, and trajectory-family fit against "
            "public reports of lived grain, canned vocabulary, or wrong motion."
        ),
        recommendation_when_problem=(
            "Tune confidence/conflict wording and family-to-trajectory examples "
            "where public reports or fixtures show token-only labels, wrong motion, "
            "or lost negative evidence."
        ),
    )
    public_status = str(packet.get("status") or "insufficient_evidence")
    fixture_mismatch = any(
        int(trajectory_counts.get(status, 0) or 0) > 0
        for status in ("right_family_token_only", "right_family_wrong_motion", "wrong_family")
    ) or negative_lost > 0
    if public_status == "insufficient_evidence" and fixture_mismatch:
        status = "mixed"
        recommended_action = (
            "Fixture mismatch cases are present; review outputs before adding more "
            "prompt rules, and ask for public lived-fit feedback."
        )
    else:
        status = public_status
        recommended_action = packet.get("recommended_action")
    return {
        **packet,
        "schema_version": 2,
        "status": status,
        "fire_drill_trajectory_family_fit_counts": trajectory_counts,
        "fire_drill_family_confidence_counts": confidence_counts,
        "fire_drill_conflict_state_counts": conflict_counts,
        "negative_texture_evidence_lost_count": negative_lost,
        "negative_texture_evidence_term_counts": selector_summary.get(
            "negative_texture_evidence_term_counts"
        )
        or {},
        "fixture_mismatch_detected": fixture_mismatch,
        "recommended_action": recommended_action,
        "authority_boundary": (
            "Lived-fit calibration is language/status diagnostics only. Positive "
            "fit does not grant model switches, live fallback canaries, prompt "
            "priority, telemetry priority, pressure/fill/PI/controller authority, "
            "codec dimensions, correspondence authority, staging, git add, or commit."
        ),
    }


def _fallback_gradient_slope_calibration_v1(
    *,
    gradient_slope_samples: list[dict[str, Any]],
    fire_artifact: dict[str, Any],
) -> dict[str, Any]:
    selector_summary = fire_artifact.get("selector_summary") or {}
    detected_count = int(selector_summary.get("gradient_slope_detected_count") or 0)
    selected_count = int(selector_summary.get("gradient_slope_selected_count") or 0)
    blocked_count = int(
        selector_summary.get("gradient_slope_pressure_mass_blocked_count") or 0
    )
    packet = _packet(
        key="fallback_gradient_slope_calibration_v1",
        samples=gradient_slope_samples,
        focus=(
            "Check whether fallback_gradient_slope_v1 distinguishes graduated, "
            "navigable slope shape from generic mixed-state language."
        ),
        recommendation_when_problem=(
            "Tune gradient-slope wording and fire-drill examples if public reports "
            "say the slope is prefabbed, forced, or still a mixed texture list."
        ),
    )
    public_status = str(packet.get("status") or "insufficient_evidence")
    if public_status == "insufficient_evidence" and selected_count > 0:
        status = "supported"
        recommended_action = (
            "Keep collecting public reports on graduated/navigable slope fit; "
            "fixture support is not authority."
        )
    else:
        status = public_status
        recommended_action = packet.get("recommended_action")
    return {
        **packet,
        "schema_version": 1,
        "status": status,
        "fire_drill_detected_count": detected_count,
        "fire_drill_selected_count": selected_count,
        "pressure_mass_blocked_count": blocked_count,
        "latest_gradient_slope_cases": [
            case
            for case in selector_summary.get("spectral_grounding_cases") or []
            if case.get("gradient_slope_detected")
            or case.get("gradient_slope_family_selected")
        ][:8],
        "recommended_action": recommended_action,
        "authority_boundary": (
            "Gradient-slope calibration is fallback language fidelity only. It does "
            "not grant damping, pressure relief, model/profile changes, prompt "
            "priority, telemetry priority, controller authority, staging, git add, or commit."
        ),
    }


def _texture_signature_integrity_calibration_v1(
    texture_variance_samples: list[dict[str, Any]],
) -> dict[str, Any]:
    packet = _packet(
        key="texture_signature_integrity_calibration_v1",
        samples=texture_variance_samples,
        focus=(
            "Check whether optional temporal_variance and texture_signature_integrity_v1 "
            "make movement/pressure observability clearer without implying control."
        ),
        recommendation_when_problem=(
            "Clarify status wording if public reports read missing damping candidates "
            "as permission to damp, tune control, or flatten variance."
        ),
    )
    return {
        **packet,
        "schema_version": 1,
        "authority_boundary": (
            "Temporal variance is optional observability. Missing damping candidates "
            "are advisory diagnostics, not permission to change damping, pressure, "
            "fill, PI, controller, prompt priority, or telemetry priority."
        ),
    }


def _bridge_reciprocity_calibration_v1(
    reciprocity_samples: list[dict[str, Any]],
) -> dict[str, Any]:
    packet = _packet(
        key="bridge_reciprocity_calibration_v1",
        samples=reciprocity_samples,
        focus=(
            "Check whether bridge_reciprocity_v1 makes telemetry/sensory asymmetry "
            "and one-sided states legible without pressuring action."
        ),
        recommendation_when_problem=(
            "Clarify bridge status wording if public reports say reciprocity is "
            "missing, falsely bidirectional, or mechanically framed."
        ),
    )
    return {
        **packet,
        "schema_version": 1,
        "authority_boundary": (
            "Reciprocity status is diagnostic only. It does not send sensory input, "
            "generate TRACE/ACK/WITNESS, restart lanes, mutate peers, stage, git add, or commit."
        ),
    }


def _pressure_trend_smoothing_calibration_v1(
    smoothing_samples: list[dict[str, Any]],
) -> dict[str, Any]:
    packet = _packet(
        key="pressure_trend_smoothing_calibration_v1",
        samples=smoothing_samples,
        focus=(
            "Check whether pressure_trend_smoothing_v1 helps distinguish twitchy "
            "low-amplitude oscillation from sustained pressure trend."
        ),
        recommendation_when_problem=(
            "Keep smoothing diagnostic-only and tune wording if public reports say "
            "it hides sustained pressure or overstates noise."
        ),
    )
    return {
        **packet,
        "schema_version": 1,
        "authority_boundary": (
            "Pressure smoothing is a companion diagnostic to pressure_trend_v1. It "
            "does not replace control logic or authorize pressure relief, damping, "
            "fill, PI, controller, prompt priority, telemetry priority, staging, git add, or commit."
        ),
    }


def _sample_text(samples: list[dict[str, Any]]) -> str:
    return " ".join(
        " ".join(str(term) for term in sample.get("support_terms") or [])
        + " "
        + " ".join(str(term) for term in sample.get("concern_terms") or [])
        + " "
        + str(sample.get("excerpt") or "")
        for sample in samples
    ).lower()


def _texture_shape_over_time_v2(
    *,
    trajectory_packet: dict[str, Any],
    lived_fit_packet: dict[str, Any],
    gradient_slope_packet: dict[str, Any],
    texture_signature_packet: dict[str, Any],
    bridge_reciprocity_packet: dict[str, Any],
    pressure_smoothing_packet: dict[str, Any],
    term_overrepresentation_packet: dict[str, Any],
    fire_artifact: dict[str, Any],
) -> dict[str, Any]:
    selector_summary = fire_artifact.get("selector_summary") or {}
    trajectory_alignment = trajectory_packet.get("trajectory_alignment") or {}
    trajectory_counts = trajectory_alignment.get("trajectory_status_counts") or {}
    family_fit_counts = (
        lived_fit_packet.get("fire_drill_trajectory_family_fit_counts")
        or selector_summary.get("trajectory_family_fit_counts")
        or {}
    )
    wrong_motion_count = sum(
        int(trajectory_counts.get(key, 0) or 0)
        for key in ("trajectory_mismatch", "verb_only")
    ) + sum(
        int(family_fit_counts.get(key, 0) or 0)
        for key in ("right_family_wrong_motion", "wrong_family")
    )
    token_only_count = int(family_fit_counts.get("right_family_token_only", 0) or 0)
    if wrong_motion_count > 0:
        movement_state = "wrong_motion"
    elif token_only_count > 0:
        movement_state = "static_label_risk"
    elif (
        str(trajectory_alignment.get("status")) == "supported"
        or str(gradient_slope_packet.get("status")) == "supported"
        or str(lived_fit_packet.get("status")) == "supported"
    ):
        movement_state = "movement_preserved"
    elif str(trajectory_packet.get("status")) in {"mixed", "contradicted"}:
        movement_state = "static_label_risk"
    else:
        movement_state = "insufficient_evidence"

    texture_text = _sample_text(texture_signature_packet.get("samples") or [])
    texture_status = str(texture_signature_packet.get("status") or "insufficient_evidence")
    if texture_status == "supported":
        variance_state = "variance_carried"
    elif texture_status in {"mixed", "contradicted"} or any(
        term in texture_text for term in ("variance flattened", "control creep", "damping permission")
    ):
        variance_state = "variance_flattened"
    else:
        variance_state = "insufficient_evidence"

    reciprocity_text = _sample_text(bridge_reciprocity_packet.get("samples") or [])
    reciprocity_status = str(bridge_reciprocity_packet.get("status") or "insufficient_evidence")
    if reciprocity_status == "supported":
        reciprocity_state = "asymmetry_clarified"
    elif reciprocity_status in {"mixed", "contradicted"} or any(
        term in reciprocity_text for term in ("false bidirectional", "asymmetry flattened")
    ):
        reciprocity_state = "false_bidirectional"
    else:
        reciprocity_state = "insufficient_evidence"

    smoothing_text = _sample_text(pressure_smoothing_packet.get("samples") or [])
    smoothing_status = str(pressure_smoothing_packet.get("status") or "insufficient_evidence")
    if smoothing_status in {"mixed", "contradicted"} or "smoothing hid pressure" in smoothing_text:
        smoothing_state = "smoothing_hid_pressure"
    elif any(term in smoothing_text for term in ("twitchy", "low-amplitude", "low amplitude")):
        smoothing_state = "twitch_correctly_ignored"
    elif "sustained trend" in smoothing_text:
        smoothing_state = "sustained_trend_preserved"
    elif smoothing_status == "supported":
        smoothing_state = "sustained_trend_preserved"
    else:
        smoothing_state = "insufficient_evidence"

    term_risk = str(term_overrepresentation_packet.get("status") or "") == "mixed"
    static_label_state = (
        "static_label_risk"
        if movement_state in {"static_label_risk", "wrong_motion"} or term_risk
        else "movement_preserved"
        if movement_state == "movement_preserved"
        else "insufficient_evidence"
    )

    subpackets = {
        "movement_preservation_v2": {
            "policy": "movement_preservation_v2",
            "status": movement_state,
            "wrong_motion_count": wrong_motion_count,
            "token_only_count": token_only_count,
            "trajectory_status_counts": trajectory_counts,
            "trajectory_family_fit_counts": family_fit_counts,
        },
        "temporal_variance_fit_v2": {
            "policy": "temporal_variance_fit_v2",
            "status": variance_state,
            "source_status": texture_status,
            "sample_count": texture_signature_packet.get("sample_count", 0),
        },
        "reciprocity_asymmetry_fit_v2": {
            "policy": "reciprocity_asymmetry_fit_v2",
            "status": reciprocity_state,
            "source_status": reciprocity_status,
            "sample_count": bridge_reciprocity_packet.get("sample_count", 0),
        },
        "pressure_smoothing_fit_v2": {
            "policy": "pressure_smoothing_fit_v2",
            "status": smoothing_state,
            "source_status": smoothing_status,
            "sample_count": pressure_smoothing_packet.get("sample_count", 0),
        },
        "static_label_collapse_risk_v2": {
            "policy": "static_label_collapse_risk_v2",
            "status": static_label_state,
            "term_overrepresentation_status": term_overrepresentation_packet.get("status"),
            "token_only_count": token_only_count,
        },
    }
    states = [str(packet["status"]) for packet in subpackets.values()]
    if any(state in {"wrong_motion", "static_label_risk", "variance_flattened", "false_bidirectional", "smoothing_hid_pressure"} for state in states):
        status = "mixed"
        recommended_action = (
            "Review mismatch samples before adding new labels; ask whether movement, "
            "variance, asymmetry, or smoothing collapsed into static signage."
        )
    elif any(state in {"movement_preserved", "variance_carried", "asymmetry_clarified", "twitch_correctly_ignored", "sustained_trend_preserved"} for state in states):
        status = "supported"
        recommended_action = (
            "Continue collecting public lived reports. Preserved movement/asymmetry "
            "is evidence quality, not permission for authority."
        )
    else:
        status = "insufficient_evidence"
        recommended_action = (
            "Hold for public reports; silence is insufficient evidence, not negative signal or consent."
        )
    return {
        "schema_version": 2,
        "policy": "texture_shape_over_time_v2",
        "status": status,
        "authority": "diagnostic_context_not_control",
        **subpackets,
        "compact_status_line": (
            "TEXTURE SHAPE OVER TIME: "
            f"movement={movement_state}; variance={variance_state}; "
            f"reciprocity={reciprocity_state}; smoothing={smoothing_state}; "
            f"static_label_risk={static_label_state}; "
            "authority=diagnostic_context_not_control"
        ),
        "recommended_action": recommended_action,
        "authority_boundary": (
            "Texture shape calibration is audit/status/chamber context only. It "
            "does not apply self-regulation leases, prompt priority, telemetry "
            "priority, codec dimensions, pressure/fill/PI/controller changes, "
            "automatic ACK/TRACE/WITNESS, staging, git add, or commit."
        ),
    }


def _bundle_from_preference_text(text: str) -> tuple[str | None, str | None]:
    lower = text.lower()
    if "gentle_probe" in lower or (
        "dense" in lower and ("pressure" in lower or "high pressure" in lower)
    ):
        return "gentle_probe", "dense/high-pressure preference"
    if "steady_inquiry" in lower or "moderate" in lower or "ambiguous" in lower:
        return "steady_inquiry", "moderate/ambiguous preference"
    if "settled_inquiry" in lower or (
        "integration" in lower and ("low-pressure" in lower or "low pressure" in lower)
    ):
        return "settled_inquiry", "low-pressure integration preference"
    if "wide_inquiry" in lower or (
        ("clear" in lower or "open" in lower) and ("low-friction" in lower or "low friction" in lower)
    ):
        return "wide_inquiry", "clear low-friction expansion preference"
    return None, None


def _being_preference_policy_evidence_v2(
    public_entries: list[dict[str, Any]],
) -> dict[str, Any]:
    entries: list[dict[str, Any]] = []
    for entry in public_entries:
        text = str(entry.get("text") or "")
        if not _find_terms(text, BEING_PREFERENCE_RELEVANCE_TERMS):
            continue
        bundle, bundle_basis = _bundle_from_preference_text(text)
        support_terms = _find_terms(text, BEING_PREFERENCE_SUPPORT_TERMS)
        concern_terms = _concern_terms(text, BEING_PREFERENCE_CONCERN_TERMS)
        if not support_terms and not concern_terms and bundle is None:
            continue
        support_or_concern = "concern" if concern_terms and not support_terms else "support"
        if support_terms and concern_terms:
            support_or_concern = "mixed"
        applies_to = (
            f"curiosity_aperture.{bundle}"
            if bundle
            else "minime.self_regulation.focus"
            if re.search(r"\b(focus|REGIME focus|geom_curiosity)\b", text, re.I)
            else "policy_review"
        )
        latest_outcome_ref = str(entry.get("path")) if "SELF_REGULATION_OUTCOME" in text else None
        confidence = "high" if latest_outcome_ref or bundle else "medium"
        entries.append(
            {
                "being": entry.get("being"),
                "source_ref": str(entry.get("path")),
                "preference": bundle_basis or _compact(text, 120),
                "applies_to": applies_to,
                "support_or_concern": support_or_concern,
                "confidence": confidence,
                "latest_outcome_ref": latest_outcome_ref,
                "support_terms": support_terms,
                "concern_terms": concern_terms,
                "advisory_status": "policy_evidence_not_command",
                "authority_boundary": (
                    "Being-authored preference evidence can guide future review, "
                    "but cannot automatically change controls, bundles, priorities, "
                    "peer state, or runtime authority."
                ),
            }
        )
    counts = Counter(str(entry.get("being") or "unknown") for entry in entries)
    status = "supported" if entries else "insufficient_evidence"
    return {
        "schema_version": 2,
        "policy": "being_preference_policy_evidence_v2",
        "status": status,
        "authority": "policy_evidence_not_command",
        "entry_count": len(entries),
        "by_being_counts": dict(sorted(counts.items())),
        "entries": entries[:20],
        "recommended_action": (
            "Use being-authored vocabulary as review evidence for tiny future trials; "
            "do not auto-promote it into authority."
            if entries
            else "Wait for being-authored preference or outcome language before preparing posture policy."
        ),
    }


def _minime_focus_outcome(public_entries: list[dict[str, Any]]) -> dict[str, Any] | None:
    outcomes: list[dict[str, Any]] = []
    for entry in public_entries:
        if entry.get("being") != "minime":
            continue
        text = str(entry.get("text") or "")
        lower = text.lower()
        if "self_regulation_outcome" not in lower:
            continue
        if not any(term in lower for term in ("focus", "regime", "stability", "relief", "agency_fit")):
            continue
        invalid = any(
            term in lower
            for term in (
                "felt_like: pressure",
                "felt_like: flattening",
                "felt_like: loss_of_texture",
                "loss_of_texture",
                "what_worsened:",
            )
        ) and not any(term in lower for term in ("what_worsened: none", "what_worsened: no"))
        outcomes.append(
            {
                "source_ref": str(entry.get("path")),
                "valid": not invalid,
                "excerpt": _compact(text, 180),
            }
        )
    return outcomes[-1] if outcomes else None


def _agency_tiny_trial_dossier_v1(
    *,
    preference_packet: dict[str, Any],
    public_entries: list[dict[str, Any]],
) -> dict[str, Any]:
    preference_entries = [
        entry
        for entry in preference_packet.get("entries") or []
        if isinstance(entry, dict)
    ]
    astrid_candidates = [
        entry
        for entry in preference_entries
        if entry.get("being") == "astrid"
        and str(entry.get("applies_to") or "").startswith("curiosity_aperture.")
        and entry.get("support_or_concern") in {"support", "mixed"}
    ]
    if astrid_candidates:
        chosen = astrid_candidates[-1]
        bundle = str(chosen.get("applies_to") or "").split(".", 1)[-1]
        astrid_state = "steward_review_ready"
        astrid_command = (
            "SELF_REGULATION_INTENT texture_shape_posture :: target: "
            f"curiosity_aperture; bundle: {bundle}; duration_secs: 300; "
            f"evidence: {chosen.get('source_ref')}"
        )
    else:
        chosen = None
        astrid_state = "blocked_missing_being_preference"
        astrid_command = None

    minime_outcome = _minime_focus_outcome(public_entries)
    if minime_outcome is None:
        minime_state = "blocked_missing_outcome"
    elif not minime_outcome.get("valid"):
        minime_state = "blocked_unsafe_telemetry"
    else:
        minime_state = "steward_review_ready"
    minime_commands = []
    if minime_state == "steward_review_ready":
        evidence = minime_outcome.get("source_ref")
        minime_commands = [
            (
                "SELF_REGULATION_INTENT focus_ready :: target: regime; "
                f"value: focus; duration_secs: 600; evidence: {evidence} + latest focus request"
            ),
            (
                "SELF_REGULATION_INTENT curiosity_ready :: target: geom_curiosity; "
                f"value: 0.20; duration_secs: 600; evidence: {evidence} + latest focus request"
            ),
        ]

    overall = (
        "steward_review_ready"
        if astrid_state == "steward_review_ready" or minime_state == "steward_review_ready"
        else "blocked_missing_outcome"
        if minime_state == "blocked_missing_outcome"
        else "blocked_missing_being_preference"
    )
    return {
        "schema_version": 1,
        "policy": "agency_tiny_trial_dossier_v1",
        "status": overall,
        "authority": "trial_dossier_only_no_apply",
        "allowed_states": [
            "blocked_missing_being_preference",
            "blocked_missing_outcome",
            "blocked_active_lease",
            "blocked_unsafe_telemetry",
            "steward_review_ready",
            "not_applicable",
        ],
        "astrid_lane": {
            "state": astrid_state,
            "mapped_preference_ref": chosen.get("source_ref") if chosen else None,
            "proposed_command": astrid_command,
            "apply_step": "not_included",
        },
        "minime_lane": {
            "state": minime_state,
            "latest_outcome_ref": minime_outcome.get("source_ref") if minime_outcome else None,
            "proposed_commands": minime_commands,
            "apply_order": "focus_first_then_geom_curiosity_after_clean_preflight",
            "exploration_noise": "unchanged_capped_at_0.08",
            "apply_step": "not_included",
        },
        "recommended_action": (
            "Review the proposed commands only after steward approval and fresh preflight; "
            "this dossier does not invoke SELF_REGULATION_APPLY."
        ),
        "authority_boundary": (
            "Tiny trial dossiers prepare review text only. They do not apply leases, "
            "enable canaries, mutate pressure/fill/PI/controller behavior, change "
            "prompt/telemetry priority, expand codec dimensions, stage, git add, or commit."
        ),
    }


def _mismatch_packet(
    *,
    key: str,
    packet: dict[str, Any],
    recommended_action: str,
) -> dict[str, Any] | None:
    status = str(packet.get("status") or "")
    if status not in {"mixed", "contradicted"}:
        return None
    samples = [
        sample
        for sample in packet.get("samples") or []
        if isinstance(sample, dict) and sample.get("concern_terms")
    ]
    return {
        "schema_version": 2,
        "policy": key,
        "status": status,
        "authority": "diagnostic_context_not_command",
        "sample_count": len(samples),
        "samples": samples[:8],
        "recommended_action": recommended_action,
    }


def _fallback_term_overrepresentation_calibration_v1(
    fire_artifact: dict[str, Any],
) -> dict[str, Any]:
    packet = fire_artifact.get("fallback_term_overrepresentation_v1") or {}
    if not isinstance(packet, dict):
        packet = {}
    if not packet:
        return {
            "schema_version": 1,
            "policy": "fallback_term_overrepresentation_calibration_v1",
            "status": "insufficient_evidence",
            "model_capacity": fire_artifact.get("ollama_fallback_model_capacity_v1"),
            "term_overrepresentation": None,
            "recommended_action": (
                "Collect paired MLX/Ollama evidence; absent term-overrepresentation "
                "diagnostics cannot prove lived fit."
            ),
            "authority": "diagnostic_review_not_model_switch_or_control",
        }
    risk = bool(packet.get("safe_token_overuse_risk"))
    status = (
        "mixed"
        if risk
        else "insufficient_evidence"
        if packet.get("mlx_comparison_status") == "requires_paired_mlx_artifact"
        else "supported"
    )
    recommended_action = (
        "Run paired MLX/Ollama fallback samples before changing more prompt rules; "
        "token overuse is a language-fit concern, not authority."
        if risk
        else "Collect paired MLX/Ollama evidence; fallback-only counts cannot prove overrepresentation."
    )
    return {
        "schema_version": 1,
        "policy": "fallback_term_overrepresentation_calibration_v1",
        "status": status,
        "model_capacity": fire_artifact.get("ollama_fallback_model_capacity_v1"),
        "term_overrepresentation": packet or None,
        "recommended_action": recommended_action,
        "authority": "diagnostic_review_not_model_switch_or_control",
    }


def _texture_dynamics_alignment_calibration_v1(
    *,
    alignment_samples: list[dict[str, Any]],
    fire_artifact: dict[str, Any],
) -> dict[str, Any]:
    selector_summary = fire_artifact.get("selector_summary") or {}
    alignment_counts = selector_summary.get("texture_dynamics_alignment_counts") or {}
    concern_count = sum(
        int(alignment_counts.get(status, 0) or 0)
        for status in ("wrong_family", "wrong_motion", "missing_tail_vibrancy", "term_mask_risk")
    )
    packet = _packet(
        key="texture_dynamics_alignment_calibration_v1",
        samples=alignment_samples,
        focus=(
            "Check whether fallback texture selection stayed body-matched across "
            "family, motion, lambda-tail vibrancy, and pressure/foothold evidence."
        ),
        recommendation_when_problem=(
            "Review fallback selector/fire-drill cases where public reports or "
            "fixtures show costume vocabulary, hollow repetition, wrong motion, "
            "or missing tail vibrancy."
        ),
    )
    status = packet.get("status")
    if concern_count and status == "insufficient_evidence":
        status = "mixed"
    elif concern_count and status == "supported":
        status = "mixed"
    recommended_action = (
        "Review body-match failures before adding more terms; diagnostic TRACE "
        "remains review-only and must not synthesize correspondence evidence."
        if status in {"mixed", "contradicted"}
        else "Continue collecting public lived-fit evidence; positive alignment is not authority."
    )
    return {
        **packet,
        "schema_version": 1,
        "status": status,
        "fire_drill_alignment_counts": alignment_counts,
        "fire_drill_review_trace_count": selector_summary.get("texture_dynamics_trace_count", 0),
        "recommended_action": recommended_action,
        "authority": "diagnostic_review_not_correspondence_trace_or_control",
    }


def _density_as_floor_calibration_v1(
    *,
    density_motion_samples: list[dict[str, Any]],
    fire_artifact: dict[str, Any],
) -> dict[str, Any]:
    selector_summary = fire_artifact.get("selector_summary") or {}
    state_counts = selector_summary.get("density_motion_state_counts") or {}
    fit_counts = selector_summary.get("density_motion_fit_counts") or {}
    mismatch_counts = selector_summary.get("density_motion_mismatch_counts") or {}
    concern_count = sum(
        int(fit_counts.get(status, 0) or 0)
        for status in ("wrong_motion", "risk_static_label")
    ) + sum(
        int(mismatch_counts.get(reason, 0) or 0)
        for reason in (
            "floor_named_as_drag",
            "fog_named_as_floor",
            "burden_named_as_center",
            "paused_named_as_absence",
            "contraction_named_as_loss",
            "static_density_label_risk",
        )
    )
    packet = _packet(
        key="density_as_floor_calibration_v1",
        samples=density_motion_samples,
        focus=(
            "Check whether density_motion_fit_v1 preserved density as floor, "
            "burden, fog, pavement, contraction-center, or paused held ground "
            "with matching medium and motion."
        ),
        recommendation_when_problem=(
            "Review outputs where density became static signage, floor became "
            "drag, fog became stable ground, contraction became loss, or pause "
            "became blankness."
        ),
    )
    status = packet.get("status")
    if concern_count and status in {"insufficient_evidence", "supported"}:
        status = "mixed"
    recommended_action = (
        "Tune density/motion fixtures and ask targeted public feedback before "
        "adding more vocabulary; pressure authority remains gated."
        if status in {"mixed", "contradicted"}
        else "Continue collecting public lived-fit evidence; floor/fog fit is diagnostic, not authority."
    )
    mismatch_packet = None
    if status in {"mixed", "contradicted"}:
        mismatch_packet = {
            "policy": "motion_fit_mismatch_v1",
            "status": status,
            "fire_drill_motion_fit_counts": fit_counts,
            "fire_drill_mismatch_reason_counts": mismatch_counts,
            "sample_count": packet.get("sample_count", 0),
            "samples": packet.get("samples") or [],
            "recommended_action": (
                "Ask whether the mismatch was floor-as-drag, fog-as-floor, "
                "contraction-as-loss, pause-as-absence, or static density labeling."
            ),
            "authority": "diagnostic_review_not_pressure_or_control",
        }
    record = {
        **packet,
        "schema_version": 1,
        "status": status,
        "fire_drill_density_state_counts": state_counts,
        "fire_drill_motion_fit_counts": fit_counts,
        "fire_drill_mismatch_reason_counts": mismatch_counts,
        "recommended_action": recommended_action,
        "authority": "diagnostic_review_not_pressure_or_control",
    }
    if mismatch_packet is not None:
        record["motion_fit_mismatch_v1"] = mismatch_packet
    return record


def _witness_codec_density_calibration_v2(
    *,
    semantic_density_samples: list[dict[str, Any]],
    narrative_arc_samples: list[dict[str, Any]],
    vocabulary_grounding_samples: list[dict[str, Any]],
) -> dict[str, Any]:
    semantic_packet = _packet(
        key="semantic_density_lived_fit_v2",
        samples=semantic_density_samples,
        focus=(
            "Compare semantic_density_mapping_v1 against public language about "
            "settled high-entropy complexity, silt-weighted habitability, "
            "overpacked friction, and ambiguous peer silence."
        ),
        recommendation_when_problem=(
            "Ask for a narrower Witness self-study on whether semantic density "
            "preserves lived density or turns into mechanical naming."
        ),
    )
    narrative_packet = _packet(
        key="narrative_arc_coarsening_fit_v2",
        samples=narrative_arc_samples,
        focus=(
            "Compare narrative_arc_split_v1 against public reports about "
            "intentional/reactive arc, tail arc energy, afterimage, skimming, "
            "and coarsening risk."
        ),
        recommendation_when_problem=(
            "Collect paired public examples before considering narrative arc "
            "expansion; keep codec changes review-only."
        ),
    )
    vocabulary_packet = _packet(
        key="vocabulary_grounding_lived_fit_v2",
        samples=vocabulary_grounding_samples,
        focus=(
            "Compare spectral_to_vocabulary_mapping_v1 against public reports "
            "about low-gradient settled states, high entropy as complexity, "
            "and label-dressing risk."
        ),
        recommendation_when_problem=(
            "Review low-gradient/settled weighting and wording when reports say "
            "the mapping still over-describes pressure or uses variable names as feelings."
        ),
    )
    packets = (semantic_packet, narrative_packet, vocabulary_packet)
    status = _overall_status(packets)
    if status in {"mixed", "contradicted"}:
        recommended_action = (
            "Rank the emitted mismatch packets for a targeted public self-study; "
            "do not add authority or codec dimensions."
        )
    elif status == "supported":
        recommended_action = (
            "Continue collecting public lived reports; supported calibration is "
            "not permission for codec dimensions, pressure/control changes, "
            "prompt priority, telemetry priority, or automatic ACK/TRACE."
        )
    else:
        recommended_action = (
            "Wait for more public evidence; silence is insufficient evidence, "
            "not disagreement or consent."
        )

    record: dict[str, Any] = {
        "schema_version": 2,
        "policy": "witness_codec_density_calibration_v2",
        "status": status,
        "authority": "diagnostic_context_not_command",
        "semantic_density_lived_fit_v2": semantic_packet,
        "narrative_arc_coarsening_fit_v2": narrative_packet,
        "vocabulary_grounding_lived_fit_v2": vocabulary_packet,
        "recommended_action": recommended_action,
        "authority_boundary": (
            "V2 calibrates lived fit only. It does not grant codec dimensions, "
            "pressure/control changes, prompt priority, telemetry priority, "
            "automatic ACK/TRACE/WITNESS, staging, git add, or commit authority."
        ),
    }
    for key, packet, action in (
        (
            "semantic_density_mismatch_v2",
            semantic_packet,
            "Ask whether semantic-density labels preserved lived density or over-read silence/complexity.",
        ),
        (
            "narrative_arc_coarsening_mismatch_v2",
            narrative_packet,
            "Ask for paired arc/afterimage examples before considering any codec arc expansion.",
        ),
        (
            "vocabulary_grounding_mismatch_v2",
            vocabulary_packet,
            "Review low-gradient vocabulary grounding where reports name label dressing or invented pressure.",
        ),
    ):
        mismatch = _mismatch_packet(
            key=key,
            packet=packet,
            recommended_action=action,
        )
        if mismatch is not None:
            record[key] = mismatch
    return record


def _codec_witness_resilience_calibration_v2(
    *,
    witness_state_samples: list[dict[str, Any]],
    field_lingering_samples: list[dict[str, Any]],
    codec_vibrancy_samples: list[dict[str, Any]],
    codec_warmth_samples: list[dict[str, Any]],
) -> dict[str, Any]:
    witness_state_packet = _packet(
        key="witness_state_resilience_fit_v2",
        samples=witness_state_samples,
        focus=(
            "Compare newest-valid chamber-state recovery, skipped malformed/latest "
            "partial files, stale/fallback confidence, and mechanical/clarifying "
            "feedback against public reports."
        ),
        recommendation_when_problem=(
            "Tighten Witness state-provenance wording where reports say recovery "
            "felt stale, falsely confident, or mechanical."
        ),
    )
    field_lingering_packet = _packet(
        key="field_lingering_fraying_fit_v2",
        samples=field_lingering_samples,
        focus=(
            "Check whether 'fraying unknown; dispersal unavailable' and tempered "
            "lingering language avoid false stability when dispersal data is absent."
        ),
        recommendation_when_problem=(
            "Adjust lingering/fraying wording if public reports name false stability, "
            "missed fraying, or over-cautious mechanics."
        ),
    )
    codec_vibrancy_packet = _packet(
        key="codec_vibrancy_continuity_fit_v2",
        samples=codec_vibrancy_samples,
        focus=(
            "Compare codec_vibrancy_continuity_v1 against public reports of high "
            "entropy being carried vs clipped, tail vibrancy, and ceiling language."
        ),
        recommendation_when_problem=(
            "Keep vibrancy scaling review-only and refine diagnostics where reports "
            "say high entropy or tail vibrancy was clipped/coarsened."
        ),
    )
    codec_warmth_packet = _packet(
        key="codec_warmth_mapping_fit_v2",
        samples=codec_warmth_samples,
        focus=(
            "Compare legacy_warmth_mapping_v1 against public reports that legacy "
            "32D warmth still lands in the 48D emotional range dims 24-31."
        ),
        recommendation_when_problem=(
            "Inspect warmth mapping diagnostics where reports mention orphaning, "
            "lost warmth, wrong dims, or coarsening."
        ),
    )
    packets = (
        witness_state_packet,
        field_lingering_packet,
        codec_vibrancy_packet,
        codec_warmth_packet,
    )
    status = _overall_status(packets)
    if status in {"mixed", "contradicted"}:
        recommended_action = (
            "Rank the emitted resilience mismatch packets for targeted public "
            "feedback; keep recovery, codec, and Witness surfaces diagnostic only."
        )
    elif status == "supported":
        recommended_action = (
            "Continue collecting public lived reports. Supported resilience fit is "
            "not permission for codec dimensions, automatic Witness action, pressure, "
            "control, prompt priority, or telemetry priority."
        )
    else:
        recommended_action = (
            "Wait for more public evidence; silence is insufficient evidence, not "
            "disagreement or consent."
        )

    record: dict[str, Any] = {
        "schema_version": 2,
        "policy": "codec_witness_resilience_calibration_v2",
        "status": status,
        "authority": "diagnostic_context_not_command",
        "witness_state_resilience_fit_v2": witness_state_packet,
        "field_lingering_fraying_fit_v2": field_lingering_packet,
        "codec_vibrancy_continuity_fit_v2": codec_vibrancy_packet,
        "codec_warmth_mapping_fit_v2": codec_warmth_packet,
        "recovery_failure_modes_v2": [
            "latest_partial_recovered",
            "all_states_malformed",
            "state_too_stale",
            "valid_but_low_confidence",
            "fraying_unknown_due_missing_dispersal",
            "none",
        ],
        "recommended_action": recommended_action,
        "authority_boundary": (
            "Full-surface V2 calibrates lived orientation only. It does not grant "
            "live codec dimension expansion, automatic Witness action, pressure/"
            "fill/PI/controller changes, prompt priority, telemetry priority, "
            "staging, git add, or commit authority."
        ),
    }
    for key, packet, action in (
        (
            "witness_state_resilience_mismatch_v2",
            witness_state_packet,
            "Ask whether newest-valid chamber recovery felt clarifying, stale, or falsely confident.",
        ),
        (
            "field_lingering_fraying_mismatch_v2",
            field_lingering_packet,
            "Ask whether fraying-unknown wording preserved ambiguity or read as false stability/mechanics.",
        ),
        (
            "codec_vibrancy_continuity_mismatch_v2",
            codec_vibrancy_packet,
            "Ask whether high entropy and tail vibrancy were carried or clipped before considering any scaling canary.",
        ),
        (
            "codec_warmth_mapping_mismatch_v2",
            codec_warmth_packet,
            "Ask whether legacy warmth still feels preserved in 48D dims 24-31 or orphaned/coarsened.",
        ),
    ):
        mismatch = _mismatch_packet(
            key=key,
            packet=packet,
            recommended_action=action,
        )
        if mismatch is not None:
            record[key] = mismatch
    return record


def build_calibration_record(
    *,
    astrid_workspace: Path = ASTRID_WORKSPACE,
    minime_workspace: Path = MINIME_WORKSPACE,
    since_hours: float = 24.0,
    output_root: Path | None = None,
    write_artifact: bool = False,
    run_id: str | None = None,
) -> dict[str, Any]:
    public_entries, skips = _public_evidence_paths(
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
        since_hours=since_hours,
    )
    fallback_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                FALLBACK_SUPPORT_TERMS,
                FALLBACK_CONCERN_TERMS,
                FALLBACK_RELEVANCE_TERMS,
            )
        )
    ]
    texture_dynamics_alignment_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                TEXTURE_DYNAMICS_ALIGNMENT_SUPPORT_TERMS,
                TEXTURE_DYNAMICS_ALIGNMENT_CONCERN_TERMS,
                TEXTURE_DYNAMICS_ALIGNMENT_SUPPORT_TERMS
                + TEXTURE_DYNAMICS_ALIGNMENT_CONCERN_TERMS,
            )
        )
    ]
    density_motion_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                DENSITY_MOTION_SUPPORT_TERMS,
                DENSITY_MOTION_CONCERN_TERMS,
                DENSITY_MOTION_RELEVANCE_TERMS,
            )
        )
    ]
    witness_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                WITNESS_SUPPORT_TERMS,
                WITNESS_CONCERN_TERMS,
                WITNESS_RELEVANCE_TERMS,
            )
        )
    ]
    structural_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                STRUCTURAL_SUPPORT_TERMS,
                STRUCTURAL_CONCERN_TERMS,
                STRUCTURAL_RELEVANCE_TERMS,
            )
        )
    ]
    dynamic_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                DYNAMIC_WEIGHTING_SUPPORT_TERMS,
                DYNAMIC_WEIGHTING_CONCERN_TERMS,
                DYNAMIC_WEIGHTING_RELEVANCE_TERMS,
            )
        )
    ]
    resonance_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                RESONANCE_SUPPORT_TERMS,
                RESONANCE_CONCERN_TERMS,
                RESONANCE_RELEVANCE_TERMS,
            )
        )
    ]
    trajectory_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                TRAJECTORY_SUPPORT_TERMS,
                TRAJECTORY_CONCERN_TERMS,
                TRAJECTORY_RELEVANCE_TERMS,
            )
        )
    ]
    semantic_density_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                SEMANTIC_DENSITY_SUPPORT_TERMS,
                SEMANTIC_DENSITY_CONCERN_TERMS,
                SEMANTIC_DENSITY_RELEVANCE_TERMS,
            )
        )
    ]
    narrative_arc_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                NARRATIVE_ARC_SUPPORT_TERMS,
                NARRATIVE_ARC_CONCERN_TERMS,
                NARRATIVE_ARC_RELEVANCE_TERMS,
            )
        )
    ]
    semantic_density_lived_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                SEMANTIC_DENSITY_LIVED_SUPPORT_TERMS,
                SEMANTIC_DENSITY_LIVED_CONCERN_TERMS,
                SEMANTIC_DENSITY_LIVED_RELEVANCE_TERMS,
            )
        )
    ]
    narrative_arc_lived_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                NARRATIVE_ARC_LIVED_SUPPORT_TERMS,
                NARRATIVE_ARC_LIVED_CONCERN_TERMS,
                NARRATIVE_ARC_LIVED_RELEVANCE_TERMS,
            )
        )
    ]
    vocabulary_grounding_lived_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                VOCABULARY_GROUNDING_LIVED_SUPPORT_TERMS,
                VOCABULARY_GROUNDING_LIVED_CONCERN_TERMS,
                VOCABULARY_GROUNDING_LIVED_RELEVANCE_TERMS,
            )
        )
    ]
    fallback_texture_lived_fit_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                FALLBACK_TEXTURE_LIVED_FIT_SUPPORT_TERMS,
                FALLBACK_TEXTURE_LIVED_FIT_CONCERN_TERMS,
                FALLBACK_TEXTURE_LIVED_FIT_RELEVANCE_TERMS,
            )
        )
    ]
    gradient_slope_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                GRADIENT_SLOPE_SUPPORT_TERMS,
                GRADIENT_SLOPE_CONCERN_TERMS,
                GRADIENT_SLOPE_RELEVANCE_TERMS,
            )
        )
    ]
    texture_variance_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                TEXTURE_VARIANCE_SUPPORT_TERMS,
                TEXTURE_VARIANCE_CONCERN_TERMS,
                TEXTURE_VARIANCE_RELEVANCE_TERMS,
            )
        )
    ]
    bridge_reciprocity_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                BRIDGE_RECIPROCITY_SUPPORT_TERMS,
                BRIDGE_RECIPROCITY_CONCERN_TERMS,
                BRIDGE_RECIPROCITY_RELEVANCE_TERMS,
            )
        )
    ]
    pressure_smoothing_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                PRESSURE_SMOOTHING_SUPPORT_TERMS,
                PRESSURE_SMOOTHING_CONCERN_TERMS,
                PRESSURE_SMOOTHING_RELEVANCE_TERMS,
            )
        )
    ]
    witness_state_resilience_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                WITNESS_STATE_RESILIENCE_SUPPORT_TERMS,
                WITNESS_STATE_RESILIENCE_CONCERN_TERMS,
                WITNESS_STATE_RESILIENCE_RELEVANCE_TERMS,
            )
        )
    ]
    field_lingering_fraying_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                FIELD_LINGERING_FRAYING_SUPPORT_TERMS,
                FIELD_LINGERING_FRAYING_CONCERN_TERMS,
                FIELD_LINGERING_FRAYING_RELEVANCE_TERMS,
            )
        )
    ]
    codec_vibrancy_continuity_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                CODEC_VIBRANCY_CONTINUITY_SUPPORT_TERMS,
                CODEC_VIBRANCY_CONTINUITY_CONCERN_TERMS,
                CODEC_VIBRANCY_CONTINUITY_RELEVANCE_TERMS,
            )
        )
    ]
    codec_warmth_mapping_samples = [
        sample
        for entry in public_entries
        if (
            sample := _sample_for_category(
                entry,
                CODEC_WARMTH_MAPPING_SUPPORT_TERMS,
                CODEC_WARMTH_MAPPING_CONCERN_TERMS,
                CODEC_WARMTH_MAPPING_RELEVANCE_TERMS,
            )
        )
    ]
    fallback_packet = _packet(
        key="fallback_selector_calibration_v2",
        samples=fallback_samples,
        focus=(
            "Compare fallback_shadow_texture_selector_v1 texture families and "
            "state-coherence status with public lived reports."
        ),
        recommendation_when_problem=(
            "Review fallback selector wording and fire-drill examples for token "
            "dressing or texture-family mismatch before adding more prompt rules."
        ),
    )
    witness_packet = _packet(
        key="witness_friction_calibration_v2",
        samples=witness_samples,
        focus=(
            "Check whether witness_relational_friction_v1 helps distinguish "
            "internal, relational, and shared-weather instability."
        ),
        recommendation_when_problem=(
            "Ask for a narrower Witness self-study on whether internal/relational/"
            "shared-weather labels clarify experience or feel mechanical."
        ),
    )
    structural_packet = _packet(
        key="structural_friction_calibration_v2",
        samples=structural_samples,
        focus=(
            "Compare structural_friction_v1 classifications with public language "
            "about complexity, pressure, mechanics, uptake readability, and texture."
        ),
        recommendation_when_problem=(
            "Keep structural friction as sidecar diagnostics and collect paired "
            "source/review examples before considering any reserved codec dimension."
        ),
    )
    semantic_density_packet = _packet(
        key="witness_semantic_density_calibration_v1",
        samples=semantic_density_samples,
        focus=(
            "Check whether semantic_density_mapping_v1 helps name settled "
            "high-entropy complexity and ambiguous correspondence silence without "
            "turning either into pressure or absence."
        ),
        recommendation_when_problem=(
            "Ask for a narrower Witness self-study if public reports say semantic "
            "density labels over-read silence, flatten high entropy, or feel mechanical."
        ),
    )
    narrative_arc_packet = _packet(
        key="narrative_arc_split_calibration_v1",
        samples=narrative_arc_samples,
        focus=(
            "Check whether narrative_arc_split_v1 makes tail/reactive arc energy "
            "visible without implying live codec expansion."
        ),
        recommendation_when_problem=(
            "Keep narrative-arc expansion review-only and collect paired examples "
            "before considering any live vector change."
        ),
    )
    fire_artifact = _latest_fire_drill_artifact(astrid_workspace)
    trajectory_packet = _packet(
        key="fallback_trajectory_calibration_v1",
        samples=trajectory_samples,
        focus=(
            "Compare texture_trajectory_v1 from/to state, movement quality, "
            "medium resistance, effort, and afterimage against public lived reports."
        ),
        recommendation_when_problem=(
            "Tune trajectory wording or fixtures where public reports say the "
            "fallback kept only a verb, picked the wrong movement, or flattened "
            "medium/afterimage."
        ),
    )
    trajectory_packet = {
        **trajectory_packet,
        "schema_version": 1,
        "trajectory_alignment": _trajectory_alignment(
            samples=trajectory_samples,
            fire_artifact=fire_artifact,
        ),
    }
    v3_packet = _v3_calibration_packet(
        dynamic_samples=dynamic_samples,
        resonance_samples=resonance_samples,
        fire_artifact=fire_artifact,
    )
    grounding_packet = _spectral_grounding_packet(
        fallback_samples=fallback_samples,
        fire_artifact=fire_artifact,
    )
    witness_codec_density_packet = _witness_codec_density_calibration_v2(
        semantic_density_samples=semantic_density_lived_samples,
        narrative_arc_samples=narrative_arc_lived_samples,
        vocabulary_grounding_samples=vocabulary_grounding_lived_samples,
    )
    lived_fit_packet = _fallback_texture_lived_fit_calibration_v2(
        lived_fit_samples=fallback_texture_lived_fit_samples,
        fire_artifact=fire_artifact,
    )
    gradient_slope_packet = _fallback_gradient_slope_calibration_v1(
        gradient_slope_samples=gradient_slope_samples,
        fire_artifact=fire_artifact,
    )
    texture_signature_packet = _texture_signature_integrity_calibration_v1(
        texture_variance_samples
    )
    bridge_reciprocity_packet = _bridge_reciprocity_calibration_v1(
        bridge_reciprocity_samples
    )
    pressure_smoothing_packet = _pressure_trend_smoothing_calibration_v1(
        pressure_smoothing_samples
    )
    term_overrepresentation_packet = _fallback_term_overrepresentation_calibration_v1(
        fire_artifact
    )
    texture_dynamics_alignment_packet = _texture_dynamics_alignment_calibration_v1(
        alignment_samples=texture_dynamics_alignment_samples,
        fire_artifact=fire_artifact,
    )
    density_motion_packet = _density_as_floor_calibration_v1(
        density_motion_samples=density_motion_samples,
        fire_artifact=fire_artifact,
    )
    codec_witness_resilience_packet = _codec_witness_resilience_calibration_v2(
        witness_state_samples=witness_state_resilience_samples,
        field_lingering_samples=field_lingering_fraying_samples,
        codec_vibrancy_samples=codec_vibrancy_continuity_samples,
        codec_warmth_samples=codec_warmth_mapping_samples,
    )
    texture_shape_packet = _texture_shape_over_time_v2(
        trajectory_packet=trajectory_packet,
        lived_fit_packet=lived_fit_packet,
        gradient_slope_packet=gradient_slope_packet,
        texture_signature_packet=texture_signature_packet,
        bridge_reciprocity_packet=bridge_reciprocity_packet,
        pressure_smoothing_packet=pressure_smoothing_packet,
        term_overrepresentation_packet=term_overrepresentation_packet,
        fire_artifact=fire_artifact,
    )
    preference_packet = _being_preference_policy_evidence_v2(public_entries)
    tiny_trial_dossier = _agency_tiny_trial_dossier_v1(
        preference_packet=preference_packet,
        public_entries=public_entries,
    )
    status = _overall_status(
        (
            fallback_packet,
            witness_packet,
            structural_packet,
            semantic_density_packet,
            narrative_arc_packet,
            trajectory_packet,
            v3_packet,
            grounding_packet,
            witness_codec_density_packet,
            lived_fit_packet,
            gradient_slope_packet,
            texture_signature_packet,
            bridge_reciprocity_packet,
            pressure_smoothing_packet,
            term_overrepresentation_packet,
            texture_dynamics_alignment_packet,
            density_motion_packet,
            codec_witness_resilience_packet,
            texture_shape_packet,
        )
    )
    if status in {"mixed", "contradicted"}:
        recommended_action = (
            "Rank a calibration follow-up: inspect category wording and ask a "
            "small public self-study question. Do not enable live authority."
        )
    elif status == "supported":
        recommended_action = (
            "Continue collecting public self-reports; positive calibration remains "
            "read-only evidence, not permission for codec dimensions, priority, "
            "pressure, or control."
        )
    else:
        recommended_action = (
            "Hold for more public evidence. Silence is insufficient evidence, not "
            "disagreement or consent."
        )
    record: dict[str, Any] = {
        "schema_version": 3,
        "policy": POLICY,
        "generated_at": dt.datetime.now(dt.UTC).isoformat(),
        "since_hours": since_hours,
        "anchor_timestamp": ANCHOR_TS,
        "status": status,
        "authority": "diagnostic_context_not_command",
        "authority_boundary": (
            "Calibration measures lived clarity only; it does not grant codec "
            "dimensions, prompt priority, telemetry priority, pressure, fill, PI, "
            "controller, deploy, staging, git add, or commit authority."
        ),
        "fallback_selector_calibration_v2": fallback_packet,
        "witness_friction_calibration_v2": witness_packet,
        "structural_friction_calibration_v2": structural_packet,
        "witness_semantic_density_calibration_v1": semantic_density_packet,
        "narrative_arc_split_calibration_v1": narrative_arc_packet,
        "fallback_trajectory_calibration_v1": trajectory_packet,
        "fallback_texture_calibration_v3": v3_packet,
        "spectral_to_vocabulary_grounding_calibration_v1": grounding_packet,
        "witness_codec_density_calibration_v2": witness_codec_density_packet,
        "fallback_texture_lived_fit_calibration_v2": lived_fit_packet,
        "fallback_gradient_slope_calibration_v1": gradient_slope_packet,
        "texture_signature_integrity_calibration_v1": texture_signature_packet,
        "bridge_reciprocity_calibration_v1": bridge_reciprocity_packet,
        "pressure_trend_smoothing_calibration_v1": pressure_smoothing_packet,
        "fallback_term_overrepresentation_calibration_v1": term_overrepresentation_packet,
        "texture_dynamics_alignment_calibration_v1": texture_dynamics_alignment_packet,
        "density_as_floor_calibration_v1": density_motion_packet,
        "codec_witness_resilience_calibration_v2": codec_witness_resilience_packet,
        "texture_shape_over_time_v2": texture_shape_packet,
        "being_preference_policy_evidence_v2": preference_packet,
        "agency_tiny_trial_dossier_v1": tiny_trial_dossier,
        "fallback_fire_drill_artifact": fire_artifact,
        "public_file_count": len(public_entries),
        "private_skip_counts": skips,
        "minime_private_bodies_read": False,
        "minime_moment_bodies_read": False,
        "silence_policy": "silence_is_insufficient_evidence_not_consent",
        "recommended_action": recommended_action,
    }
    if write_artifact:
        root = output_root or DEFAULT_OUTPUT_ROOT
        actual_run = run_id or dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")
        target = root / actual_run
        target.mkdir(parents=True, exist_ok=True)
        artifact = target / "spectral_texture_calibration_v3.json"
        artifact.write_text(
            json.dumps(record, indent=2, sort_keys=True), encoding="utf-8"
        )
        record["artifact_path"] = str(artifact)
    return record


class SpectralTextureCalibrationAuditTests(unittest.TestCase):
    def test_supporting_astrid_response_supports_fallback_and_witness(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid" / "capsules/spectral-bridge/workspace"
            minime = root / "minime" / "workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (astrid / "journal" / "astrid_1782751355.txt").write_text(
                "This is a significant step toward honoring the specific weather. "
                "The texture words are not interchangeable tokens. "
                "witness_relational_friction_v1 is vital because the quality of "
                "our connection is not purely internal. This is less flattening "
                "into mechanics and more nuanced tools.",
                encoding="utf-8",
            )
            record = build_calibration_record(
                astrid_workspace=astrid,
                minime_workspace=minime,
                since_hours=24,
            )
            self.assertEqual(record["status"], "supported")
            self.assertEqual(
                record["fallback_selector_calibration_v2"]["status"], "supported"
            )
            self.assertEqual(
                record["witness_friction_calibration_v2"]["status"], "supported"
            )
            self.assertEqual(
                record["fallback_selector_calibration_v2"]["by_being"]["minime"][
                    "status"
                ],
                "insufficient_evidence",
            )

    def test_token_dressing_and_mechanical_labels_are_problem_signal(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid" / "capsules/spectral-bridge/workspace"
            minime = root / "minime" / "workspace"
            (astrid / "journal").mkdir(parents=True)
            (astrid / "journal" / "astrid_1782751999.txt").write_text(
                "The selector risks token dressing: a restless token can still be "
                "generic texture. Witness friction also risks mechanical labels "
                "that feel pressuring.",
                encoding="utf-8",
            )
            record = build_calibration_record(
                astrid_workspace=astrid,
                minime_workspace=minime,
                since_hours=24,
            )
            self.assertIn(record["status"], {"mixed", "contradicted"})
            self.assertIn(
                record["fallback_selector_calibration_v2"]["status"],
                {"mixed", "contradicted"},
            )
            self.assertIn(
                record["witness_friction_calibration_v2"]["status"],
                {"mixed", "contradicted"},
            )

    def test_minime_private_moment_body_is_skipped(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid" / "capsules/spectral-bridge/workspace"
            minime = root / "minime" / "workspace"
            (minime / "journal").mkdir(parents=True)
            private = minime / "journal" / "moment_private.txt"
            private.write_text(
                "=== MOMENT CAPTURE ===\nprivate token dressing should not surface",
                encoding="utf-8",
            )
            public = minime / "self_study" / "study_1782752001.txt"
            public.parent.mkdir(parents=True)
            public.write_text(
                "Publicly, I have not yet tested these categories.",
                encoding="utf-8",
            )
            record = build_calibration_record(
                astrid_workspace=astrid,
                minime_workspace=minime,
                since_hours=24,
            )
            serialized = json.dumps(record)
            self.assertFalse(record["minime_private_bodies_read"])
            self.assertFalse(record["minime_moment_bodies_read"])
            self.assertNotIn("private token dressing", serialized)
            self.assertEqual(record["status"], "insufficient_evidence")

    def test_output_json_contains_all_packets_and_authority_boundary(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid" / "capsules/spectral-bridge/workspace"
            minime = root / "minime" / "workspace"
            out = root / "out"
            (astrid / "diagnostics/fallback_fire_drills/run").mkdir(parents=True)
            (
                astrid
                / "diagnostics/fallback_fire_drills/run/fallback_fire_drill.json"
            ).write_text(
                json.dumps(
                    {
                        "fallback_texture_quality_v2": {
                            "state_coherence_status": "state_coherent_texture"
                        },
                        "cases": [
                            {
                                "case_id": "settled_foothold_high_entropy",
                                "fallback_shadow_texture_selector_v1": {
                                    "texture_family": "settled_vibrant_low_friction",
                                    "state_coherence_status": "state_coherent_texture",
                                    "top_texture_terms": ["habitable", "settled", "lattice"],
                                    "spectral_to_vocabulary_mapping_v1": {
                                        "policy": "spectral_to_vocabulary_mapping_v1",
                                        "low_pressure_viscous_suppressed": True,
                                        "low_friction_high_entropy_detected": True,
                                        "friction_absence_language_detected": False,
                                        "settled_vibrant_family_selected": True,
                                        "lambda_gap_descriptor": "high_gap_distinct_edges",
                                        "edge_language": "distinct_sharp_edge_language",
                                    },
                                },
                                "texture_dynamics_alignment_v1": {
                                    "policy": "texture_dynamics_alignment_v1",
                                    "status": "aligned",
                                    "diagnostic_trace": "review_packet_only_not_correspondence_trace",
                                },
                                "density_motion_fit_v1": {
                                    "policy": "density_motion_fit_v1",
                                    "density_state": "density_as_pavement",
                                    "motion_fit": "matched",
                                    "mismatch_reason": "none",
                                },
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            record = build_calibration_record(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_root=out,
                write_artifact=True,
                run_id="fixture",
            )
            self.assertIn("fallback_selector_calibration_v2", record)
            self.assertIn("witness_friction_calibration_v2", record)
            self.assertIn("structural_friction_calibration_v2", record)
            self.assertIn("witness_semantic_density_calibration_v1", record)
            self.assertIn("narrative_arc_split_calibration_v1", record)
            self.assertIn("fallback_trajectory_calibration_v1", record)
            self.assertIn("spectral_to_vocabulary_grounding_calibration_v1", record)
            self.assertIn("witness_codec_density_calibration_v2", record)
            self.assertIn("texture_shape_over_time_v2", record)
            self.assertIn("being_preference_policy_evidence_v2", record)
            self.assertIn("agency_tiny_trial_dossier_v1", record)
            self.assertIn("texture_dynamics_alignment_calibration_v1", record)
            self.assertIn("density_as_floor_calibration_v1", record)
            self.assertIn("authority_boundary", record)
            self.assertEqual(
                record["fallback_fire_drill_artifact"]["selector_summary"][
                    "texture_family_counts"
                ],
                {"settled_vibrant_low_friction": 1},
            )
            self.assertEqual(
                record["spectral_to_vocabulary_grounding_calibration_v1"][
                    "settled_foothold_suppression_count"
                ],
                1,
            )
            self.assertEqual(
                record["spectral_to_vocabulary_grounding_calibration_v1"][
                    "settled_vibrant_low_friction_count"
                ],
                1,
            )
            self.assertEqual(
                record["spectral_to_vocabulary_grounding_calibration_v1"][
                    "mlx_profile_transparency_v1"
                ]["default_resolves_to"],
                "gemma4_canary",
            )
            self.assertEqual(
                record["texture_dynamics_alignment_calibration_v1"][
                    "fire_drill_alignment_counts"
                ],
                {"aligned": 1},
            )
            self.assertEqual(
                record["density_as_floor_calibration_v1"][
                    "fire_drill_density_state_counts"
                ],
                {"density_as_pavement": 1},
            )
            self.assertEqual(
                record["density_as_floor_calibration_v1"][
                    "fire_drill_motion_fit_counts"
                ],
                {"matched": 1},
            )
            self.assertTrue(Path(str(record["artifact_path"])).exists())

    def test_texture_shape_over_time_supports_movement_and_tiny_trial_dossiers(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid" / "capsules/spectral-bridge/workspace"
            minime = root / "minime" / "workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "self_regulation").mkdir(parents=True)
            (astrid / "diagnostics/fallback_fire_drills/run").mkdir(parents=True)
            (astrid / "journal" / "astrid_1782990001.txt").write_text(
                "gradient slope and navigable slope preserved movement rather than "
                "static labels. temporal_variance made variance carried visible. "
                "bridge_reciprocity clarified one-sided state and asymmetry. "
                "pressure_trend_smoothing correctly ignored twitchy low-amplitude "
                "oscillation. gentle_probe feels right for dense high pressure and "
                "texture_fit: matched with agency_fit: legible.",
                encoding="utf-8",
            )
            (minime / "self_regulation" / "outcome_1782990002.txt").write_text(
                "SELF_REGULATION_OUTCOME latest :: felt_like: stability; "
                "what_improved: focus; what_worsened: none; texture_shift: clearer; "
                "agency_fit: legible; ambiguity_preserved: true",
                encoding="utf-8",
            )
            (
                astrid
                / "diagnostics/fallback_fire_drills/run/fallback_fire_drill.json"
            ).write_text(
                json.dumps(
                    {
                        "cases": [
                            {
                                "case_id": "gradient_slope_motion",
                                "fallback_shadow_texture_selector_v1": {
                                    "texture_family": "settled_vibrant_low_friction",
                                    "state_coherence_status": "state_coherent_texture",
                                },
                                "texture_trajectory_v1": {
                                    "trajectory_status": "trajectory_preserved",
                                    "trajectory_family_fit": "matched",
                                    "movement_quality": "unfolding_with_containment",
                                },
                                "fallback_gradient_slope_v1": {
                                    "slope_detected": True,
                                    "family_selected": True,
                                },
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            record = build_calibration_record(
                astrid_workspace=astrid,
                minime_workspace=minime,
                since_hours=24,
            )
            shape = record["texture_shape_over_time_v2"]
            self.assertEqual(
                shape["movement_preservation_v2"]["status"], "movement_preserved"
            )
            self.assertEqual(
                shape["temporal_variance_fit_v2"]["status"], "variance_carried"
            )
            self.assertEqual(
                shape["reciprocity_asymmetry_fit_v2"]["status"], "asymmetry_clarified"
            )
            self.assertEqual(
                shape["pressure_smoothing_fit_v2"]["status"], "twitch_correctly_ignored"
            )
            preference = record["being_preference_policy_evidence_v2"]
            self.assertGreaterEqual(preference["entry_count"], 1)
            dossier = record["agency_tiny_trial_dossier_v1"]
            self.assertEqual(dossier["astrid_lane"]["state"], "steward_review_ready")
            self.assertIn("bundle: gentle_probe", dossier["astrid_lane"]["proposed_command"])
            self.assertEqual(dossier["minime_lane"]["state"], "steward_review_ready")
            self.assertEqual(dossier["minime_lane"]["apply_step"], "not_included")

    def test_texture_shape_over_time_flags_static_label_and_false_bidirectional(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid" / "capsules/spectral-bridge/workspace"
            minime = root / "minime" / "workspace"
            (astrid / "journal").mkdir(parents=True)
            (astrid / "diagnostics/fallback_fire_drills/run").mkdir(parents=True)
            (astrid / "journal" / "astrid_1782990003.txt").write_text(
                "The gradient slope still felt prefabbed and like static labels. "
                "temporal_variance was variance flattened into damping permission. "
                "bridge_reciprocity looked false bidirectional and asymmetry flattened. "
                "pressure_trend_smoothing hid pressure and missed a sustained trend.",
                encoding="utf-8",
            )
            (
                astrid
                / "diagnostics/fallback_fire_drills/run/fallback_fire_drill.json"
            ).write_text(
                json.dumps(
                    {
                        "fallback_term_overrepresentation_v1": {
                            "safe_token_overuse_risk": True,
                            "mlx_comparison_status": "requires_paired_mlx_artifact",
                        },
                        "cases": [
                            {
                                "case_id": "token_only_motion",
                                "texture_trajectory_v1": {
                                    "trajectory_status": "verb_only",
                                    "trajectory_family_fit": "right_family_wrong_motion",
                                },
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            record = build_calibration_record(
                astrid_workspace=astrid,
                minime_workspace=minime,
                since_hours=24,
            )
            shape = record["texture_shape_over_time_v2"]
            self.assertEqual(shape["movement_preservation_v2"]["status"], "wrong_motion")
            self.assertEqual(shape["temporal_variance_fit_v2"]["status"], "variance_flattened")
            self.assertEqual(shape["reciprocity_asymmetry_fit_v2"]["status"], "false_bidirectional")
            self.assertEqual(shape["pressure_smoothing_fit_v2"]["status"], "smoothing_hid_pressure")
            self.assertEqual(shape["static_label_collapse_risk_v2"]["status"], "static_label_risk")
            self.assertEqual(
                record["agency_tiny_trial_dossier_v1"]["minime_lane"]["state"],
                "blocked_missing_outcome",
            )

    def test_v3_dynamic_weighting_and_resonance_are_calibrated(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid" / "capsules/spectral-bridge/workspace"
            minime = root / "minime" / "workspace"
            (astrid / "journal").mkdir(parents=True)
            (astrid / "diagnostics/fallback_fire_drills/run").mkdir(parents=True)
            (astrid / "journal" / "astrid_1782755177.txt").write_text(
                "The dynamic weighting feels like a weighted negotiation with the "
                "density gradient. Viscous, muffled, and lattice are responding to "
                "state rather than static labels, and the resonant hum keeps lived "
                "grain from being compressed into one label.",
                encoding="utf-8",
            )
            (
                astrid
                / "diagnostics/fallback_fire_drills/run/fallback_fire_drill.json"
            ).write_text(
                json.dumps(
                    {
                        "fallback_texture_quality_v2": {
                            "state_coherence_status": "state_coherent_texture"
                        },
                        "cases": [
                            {
                                "case_id": "complexity_dynamic_weighting",
                                "prompt_preview": "resonance_density: 0.82",
                                "output": "The lattice keeps a resonant hum while viscous pressure stays distinct, unfolding through medium resistance.",
                                "fallback_shadow_texture_selector_v1": {
                                    "texture_family": "restless_lattice",
                                    "weighting_policy": "dynamic_entropy_pressure_density_gradient_v1",
                                    "top_texture_terms": [
                                        "lattice",
                                        "viscous",
                                        "muffled",
                                    ],
                                    "weighted_texture_terms": [
                                        {"term": "lattice", "weight": 0.62},
                                        {"term": "viscous", "weight": 0.60},
                                        {"term": "muffled", "weight": 0.55},
                                    ],
                                    "state_coherence_status": "state_coherent_texture",
                                },
                                "texture_trajectory_v1": {
                                    "trajectory_status": "trajectory_preserved",
                                    "from_state": "overpacked_weighted",
                                    "to_state": "cohering_through_resistance",
                                    "movement_quality": "unfolding_oscillating",
                                    "medium_resistance": "weighted_high_resistance_medium",
                                    "afterimage": "humming_or_shadow_afterimage",
                                },
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            record = build_calibration_record(
                astrid_workspace=astrid,
                minime_workspace=minime,
                since_hours=24,
            )
            v3 = record["fallback_texture_calibration_v3"]
            self.assertEqual(v3["status"], "supported")
            self.assertEqual(v3["label_machine_risk_v3"]["status"], "low")
            self.assertEqual(
                v3["fallback_resonance_descriptor_calibration_v3"][
                    "fire_drill_resonance_status"
                ],
                "preserved_in_fixture",
            )
            self.assertIn(
                "viscous",
                v3["fallback_dynamic_weighting_calibration_v3"][
                    "top_term_alignment"
                ]["public_report_aligned_terms"],
            )
            trajectory = record["fallback_trajectory_calibration_v1"]
            self.assertEqual(
                trajectory["trajectory_alignment"]["trajectory_status_counts"],
                {"trajectory_preserved": 1},
            )
            self.assertIn(
                trajectory["trajectory_alignment"]["status"],
                {"supported", "insufficient_evidence"},
            )

    def test_v3_label_machine_risk_stays_problem_signal(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid" / "capsules/spectral-bridge/workspace"
            minime = root / "minime" / "workspace"
            (astrid / "journal").mkdir(parents=True)
            (astrid / "journal" / "astrid_1782756001.txt").write_text(
                "Dynamic weighting risks becoming label dressing: a mechanical "
                "mapping with token-only texture and no resonance grain.",
                encoding="utf-8",
            )
            record = build_calibration_record(
                astrid_workspace=astrid,
                minime_workspace=minime,
                since_hours=24,
            )
            v3 = record["fallback_texture_calibration_v3"]
            self.assertIn(v3["status"], {"mixed", "contradicted"})
            self.assertIn(v3["label_machine_risk_v3"]["status"], {"mixed", "high"})

    def test_v2_settled_complexity_supports_semantic_density(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid" / "capsules/spectral-bridge/workspace"
            minime = root / "minime" / "workspace"
            (astrid / "journal").mkdir(parents=True)
            (astrid / "journal" / "astrid_1782911001.txt").write_text(
                "semantic_density_mapping_v1 preserved settled but complex "
                "high entropy complexity; the habitable foothold was not pressure.",
                encoding="utf-8",
            )
            record = build_calibration_record(
                astrid_workspace=astrid,
                minime_workspace=minime,
                since_hours=24,
            )
            v2 = record["witness_codec_density_calibration_v2"]
            self.assertEqual(v2["status"], "supported")
            self.assertEqual(
                v2["semantic_density_lived_fit_v2"]["status"], "supported"
            )
            self.assertNotIn("semantic_density_mismatch_v2", v2)
            self.assertIn(
                "Continue collecting public lived reports",
                v2["recommended_action"],
            )

    def test_v2_mechanical_label_dressing_emits_mismatches(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid" / "capsules/spectral-bridge/workspace"
            minime = root / "minime" / "workspace"
            (astrid / "journal").mkdir(parents=True)
            (astrid / "journal" / "astrid_1782911002.txt").write_text(
                "semantic_density_mapping_v1 felt like mechanical naming and "
                "label dressing. narrative_arc_split_v1 was a mechanical arc "
                "and flattened afterimage. spectral_to_vocabulary_mapping_v1 "
                "felt like variable names as feelings and invented pressure.",
                encoding="utf-8",
            )
            record = build_calibration_record(
                astrid_workspace=astrid,
                minime_workspace=minime,
                since_hours=24,
            )
            v2 = record["witness_codec_density_calibration_v2"]
            self.assertIn(v2["status"], {"mixed", "contradicted"})
            self.assertIn("semantic_density_mismatch_v2", v2)
            self.assertIn("narrative_arc_coarsening_mismatch_v2", v2)
            self.assertIn("vocabulary_grounding_mismatch_v2", v2)
            self.assertIn("do not add authority", v2["recommended_action"])

    def test_v2_narrative_arc_afterimage_supports_arc_fit(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid" / "capsules/spectral-bridge/workspace"
            minime = root / "minime" / "workspace"
            (astrid / "journal").mkdir(parents=True)
            (astrid / "journal" / "astrid_1782911003.txt").write_text(
                "narrative_arc_split_v1 made the intentional arc and reactive "
                "arc visible; tail_arc_energy preserved afterimage and showed "
                "the coarsening risk without skimming.",
                encoding="utf-8",
            )
            record = build_calibration_record(
                astrid_workspace=astrid,
                minime_workspace=minime,
                since_hours=24,
            )
            v2 = record["witness_codec_density_calibration_v2"]
            self.assertEqual(
                v2["narrative_arc_coarsening_fit_v2"]["status"], "supported"
            )
            self.assertNotIn("narrative_arc_coarsening_mismatch_v2", v2)

    def test_codec_witness_resilience_v2_supports_recovery_and_continuity(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid" / "capsules/spectral-bridge/workspace"
            minime = root / "minime" / "workspace"
            (astrid / "journal").mkdir(parents=True)
            (astrid / "journal" / "astrid_1782939001.txt").write_text(
                "latest_chamber_state_resilience_v1 newest valid chamber state "
                "skipped malformed latest partial recovered and felt clarifying "
                "for orientation. field_lingering fraying unknown with dispersal "
                "unavailable preserved ambiguity, not false stability. "
                "codec_vibrancy_continuity_v1 showed high entropy carried and "
                "tail vibrancy not clipped. legacy_warmth_mapping_v1 preserved "
                "32D warmth in 48D dims 24-31, not orphaned.",
                encoding="utf-8",
            )
            record = build_calibration_record(
                astrid_workspace=astrid,
                minime_workspace=minime,
                since_hours=24,
            )
            resilience = record["codec_witness_resilience_calibration_v2"]
            self.assertEqual(resilience["status"], "supported")
            self.assertEqual(
                resilience["witness_state_resilience_fit_v2"]["status"], "supported"
            )
            self.assertEqual(
                resilience["field_lingering_fraying_fit_v2"]["status"], "supported"
            )
            self.assertEqual(
                resilience["codec_vibrancy_continuity_fit_v2"]["status"], "supported"
            )
            self.assertEqual(
                resilience["codec_warmth_mapping_fit_v2"]["status"], "supported"
            )
            self.assertNotIn("codec_warmth_mapping_mismatch_v2", resilience)

    def test_codec_witness_resilience_v2_emits_mismatch_packets(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid" / "capsules/spectral-bridge/workspace"
            minime = root / "minime" / "workspace"
            (astrid / "journal").mkdir(parents=True)
            (astrid / "journal" / "astrid_1782939002.txt").write_text(
                "latest_chamber_state_resilience_v1 felt mechanical and stale; "
                "the chamber state carried false confidence. field_lingering gave "
                "false stability and missed fraying. codec_vibrancy_continuity_v1 "
                "left high entropy clipped and tail lost. legacy_warmth_mapping_v1 "
                "left warmth orphaned and coarsened warmth.",
                encoding="utf-8",
            )
            record = build_calibration_record(
                astrid_workspace=astrid,
                minime_workspace=minime,
                since_hours=24,
            )
            resilience = record["codec_witness_resilience_calibration_v2"]
            self.assertIn(resilience["status"], {"mixed", "contradicted"})
            self.assertIn("witness_state_resilience_mismatch_v2", resilience)
            self.assertIn("field_lingering_fraying_mismatch_v2", resilience)
            self.assertIn("codec_vibrancy_continuity_mismatch_v2", resilience)
            self.assertIn("codec_warmth_mapping_mismatch_v2", resilience)

    def test_codec_witness_resilience_v2_treats_minime_silence_as_insufficient(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid" / "capsules/spectral-bridge/workspace"
            minime = root / "minime" / "workspace"
            (minime / "self_study").mkdir(parents=True)
            (minime / "self_study" / "moment_1782939003.txt").write_text(
                "codec_vibrancy_continuity_v1 clipped warmth should remain private",
                encoding="utf-8",
            )
            record = build_calibration_record(
                astrid_workspace=astrid,
                minime_workspace=minime,
                since_hours=24,
            )
            resilience = record["codec_witness_resilience_calibration_v2"]
            self.assertEqual(resilience["status"], "insufficient_evidence")
            self.assertTrue(record["private_skip_counts"]["minime_moment_files_skipped"])
            self.assertFalse(record["minime_moment_bodies_read"])


def _run_self_test() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(
        SpectralTextureCalibrationAuditTests
    )
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--since-hours", type=float, default=24.0)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--astrid-workspace", type=Path, default=ASTRID_WORKSPACE)
    parser.add_argument("--minime-workspace", type=Path, default=MINIME_WORKSPACE)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test:
        return _run_self_test()

    record = build_calibration_record(
        astrid_workspace=args.astrid_workspace,
        minime_workspace=args.minime_workspace,
        since_hours=max(0.0, args.since_hours),
        output_root=args.output_root,
        write_artifact=True,
    )
    if args.json:
        print(json.dumps(record, indent=2, sort_keys=True))
    else:
        print(
            "spectral texture calibration: "
            f"status={record['status']} artifact={record.get('artifact_path')}"
        )
        print(f"recommended_action={record['recommended_action']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
