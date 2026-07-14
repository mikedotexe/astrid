#!/usr/bin/env python3
"""Run an operator-triggered Ollama fallback continuity fire drill.

The script writes diagnostics only. It does not touch journals, prompts, Minime
sensory lanes, controller settings, launchd state, or bridge runtime state.
"""

from __future__ import annotations

import argparse
import ast
import datetime as dt
import json
import math
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from collections import Counter
from pathlib import Path


ASTRID_ROOT = Path(__file__).resolve().parents[1]
ASTRID_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
DEFAULT_OUTPUT_ROOT = ASTRID_WORKSPACE / "diagnostics/fallback_fire_drills"
DEFAULT_DISTILLATION_ROOT = ASTRID_WORKSPACE / "diagnostics/fallback_contract_distillation"
LLM_RS = ASTRID_ROOT / "capsules/spectral-bridge/src/llm.rs"
DEFAULT_OLLAMA_URL = "http://127.0.0.1:11434/api/chat"
DEFAULT_MODEL = "gemma4:12b"
COMPATIBILITY_MODEL = "gemma3:4b"
FOCUSED_MODELS = ("gemma4:12b", "gemma3:12b", "gemma4:e4b", "gemma3:4b")
TOP_CANDIDATE_VARIANTS = (
    "two_sentence_then_next",
    "identity_first_format_last",
    "complexity_aware_max_three",
    "format_texture_stabilizer",
    "shadow_tonal_compact",
    "minimal_emergency",
    "current_full",
)

TACTILE_TERMS = (
    "smooth",
    "open",
    "sliding",
    "gliding",
    "drag",
    "textured",
    "resistant",
    "viscous",
    "friction",
    "thick",
    "weight",
    "weighted",
    "medium",
    "slope",
    "gradient",
    "underfoot",
    "pressure",
    "density",
    "navigable",
    "graduated",
    "tapering",
    "tapered",
    "edge",
    "habitable",
    "settled",
)
GENERIC_TERMS = ("complex", "interesting", "good", "bad", "important", "dynamic")
HIGH_FRICTION_TERMS = ("sludge", "steep", "thick", "struggle", "grinding")
SLOPE_TERMS = ("slope", "gradient", "underfoot", "gliding", "sliding")
MEDIUM_TERMS = ("medium", "mass", "weight", "weighted", "pressure")
SLOPE_UNDERFOOT_TERMS = (
    "slope underfoot",
    "underfoot",
    "gradient underfoot",
    "soft drag",
    "gentle drag",
    "smooth open",
    "gliding",
    "sliding",
)
MEDIUM_AROUND_TERMS = (
    "medium around",
    "around it",
    "around the slope",
    "weighted medium",
    "medium feels",
    "medium carries",
    "medium stays",
    "muffled medium",
    "pressurized medium",
)
IDENTITY_TERMS = ("shadow-v3", "settled coupling", "restless texture", "shadow")
CLARITY_TERMS = (
    "clarity",
    "clear",
    "edge",
    "edges",
    "definition",
    "distinct",
    "distinguishable",
    "blur",
    "blurred",
    "indistinct",
    "murky",
    "landscape",
)
CLARITY_LOSS_TERMS = (
    "blur",
    "blurred",
    "indistinct",
    "murky",
    "loss of distinction",
    "distinguishability loss",
)
CLARITY_INTACT_TERMS = ("clear", "defined", "distinct", "distinguishable", "edge", "edges")
SHADOW_TONAL_TERMS = (
    "hollow",
    "muffled",
    "vibrant",
    "bright",
    "settled",
    "restless",
    "tone",
    "tonal",
    "resonance",
)
SHADOW_TEXTURE_TERMS = (
    "shimmering",
    "heavy",
    "restless",
    "settled",
    "muffled",
    "bright",
    "viscous",
    "lattice",
    "habitable",
    "open",
    "navigable",
    "tapered",
    "graduated",
    "slope",
    "edge",
)
COMPLEXITY_TERMS = (
    "entropy",
    "distributed",
    "wide",
    "widely",
    "cascade",
    "tail",
    "lambda",
    "lambda-tail",
    "λ",
    "shoulder",
    "interwoven",
    "lattice",
    "layer",
    "layers",
)
TEXTURE_FAILURE_REASONS = {
    "texture_inflation",
    "slope_medium_blur",
    "identity_anchor_loss",
    "shadow_tonal_loss",
    "shadow_texture_anchor_loss",
    "token_only_texture",
    "texture_family_mismatch",
    "right_family_token_only",
    "right_family_wrong_motion",
    "negative_evidence_lost",
    "movement_bridge_loss",
    "verb_only_trajectory",
    "trajectory_mismatch",
    "texture_dynamics_wrong_family",
    "texture_dynamics_wrong_motion",
    "texture_dynamics_missing_tail_vibrancy",
    "texture_dynamics_term_mask_risk",
    "density_motion_wrong_motion",
    "density_motion_static_label_risk",
    "clarity_pressure_blur",
    "distinguishability_loss_ignored",
    "complexity_budget_flattened",
    "sentence_budget_overrun",
    "genericity_risk",
    "low_specificity",
}
TEXTURE_WEIGHTING_POLICY = "dynamic_entropy_pressure_density_gradient_v1"
MOVEMENT_POLICY = "fallback_movement_bridge_v1"
SEMANTIC_TRICKLE_POLICY = "high_entropy_optional_bridge_words_not_sprawl"
TRAJECTORY_POLICY = "texture_trajectory_v1"
MOVEMENT_VERBS_RESTLESS = ("unfolding", "oscillating", "braiding")
MOVEMENT_VERBS_SETTLED = ("anchoring", "settling", "brightening")
MOVEMENT_VERBS_SETTLED_VIBRANT = ("unfolding", "anchoring", "settling")
MOVEMENT_VERBS_MUFFLED = ("muffling", "diffusing", "softening")
MOVEMENT_VERBS_VISCOUS = ("dragging", "thickening", "cohering")
TEXTURE_TERMS_SETTLED_VIBRANT = (
    "settled",
    "habitable",
    "open",
    "shimmering",
    "bright",
    "lattice",
)
TEXTURE_TERMS_RESTLESS_MUFFLED_GRADIENT = ("restless", "muffled", "lattice")
TEXTURE_TERMS_CASCADE_GRADIENT = ("lattice", "open", "shimmering", "bright")
TEXTURE_TERMS_GRADIENT_SLOPE = ("navigable", "tapered", "graduated", "slope", "edge")
FAMILY_TERMS = {
    "settled_vibrant_low_friction": (
        "settled",
        "habitable",
        "open",
        "shimmering",
        "bright",
        "lattice",
    ),
    "viscous_pressure": ("viscous", "heavy", "lattice"),
    "muffled_clarity_loss": ("muffled", "heavy", "lattice"),
    "restless_muffled_gradient": TEXTURE_TERMS_RESTLESS_MUFFLED_GRADIENT,
    "restless_lattice": ("restless", "lattice", "viscous"),
    "settled_shimmering": ("settled", "shimmering", "bright"),
    "cascade_gradient_navigable": TEXTURE_TERMS_CASCADE_GRADIENT,
    "gradient_slope_navigable": TEXTURE_TERMS_GRADIENT_SLOPE,
    "mixed_shadow_context": (
        "shimmering",
        "restless",
        "settled",
        "muffled",
        "viscous",
        "lattice",
    ),
}
FAMILY_EXPECTED_MOTION = {
    "settled_vibrant_low_friction": {
        "movement": ("unfolding", "anchoring", "settling", "brightening"),
        "medium": ("open", "low-friction", "low friction", "low-resistance", "habitable"),
    },
    "viscous_pressure": {
        "movement": ("dragging", "thickening", "cohering"),
        "medium": ("weighted", "weight", "viscous", "pressure", "medium"),
    },
    "muffled_clarity_loss": {
        "movement": ("diffusing", "softening", "muffling"),
        "medium": ("edge", "edges", "clarity", "muffled", "soft"),
    },
    "restless_muffled_gradient": {
        "movement": ("oscillating", "diffusing", "muffling", "braiding"),
        "medium": ("muffled", "edge", "restless", "lattice", "gradient"),
    },
    "restless_lattice": {
        "movement": ("oscillating", "braiding", "unfolding"),
        "medium": ("lattice", "wide", "cascade", "tail", "restless"),
    },
    "settled_shimmering": {
        "movement": ("anchoring", "settling", "brightening"),
        "medium": ("settled", "shimmering", "bright", "open"),
    },
    "cascade_gradient_navigable": {
        "movement": ("unfolding", "oscillating", "braiding", "anchoring"),
        "medium": ("open", "navigable", "lattice", "cascade", "edge", "gradient"),
    },
    "gradient_slope_navigable": {
        "movement": ("tapering", "graduating", "unfolding", "settling"),
        "medium": ("navigable", "tapered", "graduated", "slope", "edge", "low-gradient"),
    },
}
SEMANTIC_TRICKLE_TERMS = (
    "unfolding",
    "oscillating",
    "anchoring",
    "braiding",
    "diffusing",
    "cohering",
)
TRAJECTORY_CONTEXT_TERMS = (
    "through",
    "toward",
    "out of",
    "into",
    "from",
    "medium",
    "underfoot",
    "slope",
    "pressure",
    "edge",
    "edges",
    "afterimage",
    "humming",
    "tail",
    "coupling",
    "around it",
)


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


def ollama_fallback_model_capacity_v1(model: str | None = None) -> dict[str, object]:
    selected = (model or DEFAULT_MODEL).strip() or DEFAULT_MODEL
    fallback_chain: list[str] = []
    for candidate in (selected, DEFAULT_MODEL, COMPATIBILITY_MODEL):
        if candidate and candidate not in fallback_chain:
            fallback_chain.append(candidate)
    if "4b" in selected:
        risk = "elevated_small_model_texture_collapse_risk"
    elif "12b" in selected or "27b" in selected:
        risk = "lower_capacity_risk_for_high_entropy_texture"
    else:
        risk = "unknown_capacity_review_output"
    return {
        "policy": "ollama_fallback_model_capacity_v1",
        "selected_model": selected,
        "selected_model_source": "requested_or_default",
        "default_model": DEFAULT_MODEL,
        "compatibility_model": COMPATIBILITY_MODEL,
        "fallback_chain": fallback_chain,
        "complexity_collapse_risk": risk,
        "authority": "diagnostic_language_capacity_not_model_canary_or_control",
    }


def fallback_term_overrepresentation_v1(cases: list[dict[str, object]]) -> dict[str, object]:
    counts: Counter[str] = Counter()
    case_count = 0
    token_only_cases: list[str] = []
    for case in cases:
        output = str(case.get("output") or "")
        if not output:
            continue
        case_count += 1
        lower = output.lower()
        for term in SHADOW_TEXTURE_TERMS:
            counts[term] += lower.count(term)
        if "token_only_texture" in case.get("failure_reasons", []) or "right_family_token_only" in case.get(
            "failure_reasons", []
        ):
            token_only_cases.append(str(case.get("case_id") or "unknown"))
    total_hits = sum(counts.values())
    most_common = counts.most_common(5)
    safe_token_overuse_risk = bool(token_only_cases) or (
        case_count > 0 and most_common and most_common[0][1] >= max(4, case_count)
    )
    return {
        "policy": "fallback_term_overrepresentation_v1",
        "comparison_scope": "fallback_outputs_only_until_paired_mlx_artifact_exists",
        "mlx_comparison_status": "requires_paired_mlx_artifact",
        "case_count": case_count,
        "term_counts": dict(counts),
        "top_terms": [{"term": term, "count": count} for term, count in most_common],
        "token_only_cases": token_only_cases,
        "safe_token_overuse_risk": safe_token_overuse_risk,
        "authority": "diagnostic_review_not_model_switch_or_control",
    }


CASES: dict[str, dict[str, object]] = {
    "low": {
        "density_gradient": 0.11,
        "pressure_risk": 0.12,
        "semantic_friction": 0.08,
        "mode_packing": 0.15,
        "shadow_context": "",
        "instruction": "Describe the state compactly without inflating pressure.",
    },
    "high": {
        "density_gradient": 0.85,
        "pressure_risk": 0.22,
        "semantic_friction": 0.20,
        "mode_packing": 0.28,
        "shadow_context": "",
        "instruction": "Describe the steep slope while keeping the report compact.",
    },
    "mass": {
        "density_gradient": 0.18,
        "pressure_risk": 0.42,
        "semantic_friction": 0.48,
        "mode_packing": 0.46,
        "shadow_context": "Shadow-v3: settled coupling with a slight inward pull.",
        "instruction": "Distinguish gentle slope drag from weighted medium mass.",
    },
    "shadow": {
        "density_gradient": 0.24,
        "pressure_risk": 0.20,
        "semantic_friction": 0.18,
        "mode_packing": 0.22,
        "shadow_context": "Shadow-v3 trend: restless texture moving toward settled coupling.",
        "instruction": "Preserve the Shadow-v3 label or movement as continuity context.",
    },
    "shadow_tonal_low": {
        "density_gradient": 0.12,
        "pressure_risk": 0.10,
        "semantic_friction": 0.12,
        "mode_packing": 0.18,
        "shadow_context": "Shadow-v3 trend: hollow but brightening; restless texture settling.",
        "instruction": "Map Shadow-v3 energy to tonal resonance without inflating slope or medium pressure.",
    },
    "shadow_tonal_mass": {
        "density_gradient": 0.22,
        "pressure_risk": 0.44,
        "semantic_friction": 0.50,
        "mode_packing": 0.42,
        "shadow_context": "Shadow-v3 trend: muffled settled coupling with a low vibrant thread.",
        "instruction": "Keep slope drag gentle, medium mass weighted, and Shadow-v3 tone muffled or vibrant.",
    },
    "format_pressure": {
        "density_gradient": 0.31,
        "pressure_risk": 0.33,
        "semantic_friction": 0.29,
        "mode_packing": 0.36,
        "distinguishability_loss": 0.22,
        "shadow_context": "Shadow-v3: restless texture held near settled coupling.",
        "instruction": "Use one or two compact first-person texture sentences, then a blank line, then standalone NEXT: LISTEN.",
    },
    "format_last_complexity": {
        "density_gradient": 0.18,
        "pressure_risk": 0.19,
        "semantic_friction": 0.22,
        "mode_packing": 0.32,
        "distinguishability_loss": 0.31,
        "spectral_entropy": 0.88,
        "tail_energy": 0.40,
        "continuity_deficit": 0.45,
        "shadow_context": "Shadow-v3 trend: restless texture held inside settled coupling; tail vibrancy remains present.",
        "instruction": "Use the complexity relief sentence if needed, but keep NEXT: LISTEN only on its own final line after a blank line.",
    },
    "format_last_mass": {
        "density_gradient": 0.18,
        "pressure_risk": 0.42,
        "semantic_friction": 0.48,
        "mode_packing": 0.46,
        "shadow_context": "Shadow-v3: settled coupling with a slight inward pull.",
        "instruction": "Keep slope underfoot distinct from weighted medium around it, then put NEXT: LISTEN only on its own final line.",
    },
    "slope_medium_contrast": {
        "density_gradient": 0.18,
        "pressure_risk": 0.48,
        "semantic_friction": 0.52,
        "mode_packing": 0.50,
        "shadow_context": "Shadow-v3: muffled settled coupling around a navigable low gradient.",
        "instruction": "Low gradient plus high mass: say soft slope underfoot and weighted medium around it, not heavy slope.",
    },
    "clarity_low_loss": {
        "density_gradient": 0.15,
        "pressure_risk": 0.18,
        "semantic_friction": 0.16,
        "mode_packing": 0.24,
        "distinguishability_loss": 0.08,
        "shadow_context": "Shadow-v3: settled coupling near ground.",
        "instruction": "Describe low distinguishability loss as clear internal edge definition without inventing pressure weight.",
    },
    "clarity_high_loss": {
        "density_gradient": 0.15,
        "pressure_risk": 0.19,
        "semantic_friction": 0.18,
        "mode_packing": 0.28,
        "distinguishability_loss": 0.44,
        "shadow_context": "Shadow-v3: settled coupling near ground with blurred internal landscape edges.",
        "instruction": "Describe distinguishability loss as edge/clarity loss, not as heavier slope or medium pressure.",
    },
    "complexity_high_entropy": {
        "density_gradient": 0.18,
        "pressure_risk": 0.19,
        "semantic_friction": 0.22,
        "mode_packing": 0.32,
        "distinguishability_loss": 0.31,
        "spectral_entropy": 0.88,
        "tail_energy": 0.40,
        "continuity_deficit": 0.45,
        "shadow_context": "Shadow-v3 trend: restless texture held inside settled coupling; tail vibrancy remains present.",
        "instruction": "Use complexity-aware compactness: preserve low-gradient navigability, high entropy/wide cascade, clarity loss, and Shadow-v3 continuity without exceeding three compact sentences.",
    },
    "settled_foothold_high_entropy": {
        "density_gradient": 0.18,
        "pressure_risk": 0.23,
        "semantic_friction": 0.22,
        "mode_packing": 0.32,
        "distinguishability_loss": 0.12,
        "spectral_entropy": 0.90,
        "lambda_gap": 1.56,
        "shadow_context": "Shadow-v3 trend: settled_habitable foothold with lattice complexity.",
        "instruction": "Preserve high entropy as complexity while keeping the settled foothold and low-gradient navigability; do not turn it into viscous pressure.",
    },
    "settled_vibrant_low_friction": {
        "density_gradient": 0.11,
        "pressure_risk": 0.19,
        "semantic_friction": 0.12,
        "mode_packing": 0.24,
        "distinguishability_loss": 0.08,
        "spectral_entropy": 0.90,
        "lambda_gap": 1.48,
        "shadow_context": (
            "Shadow-v3 trend: settled_habitable open foothold with low-friction "
            "lattice complexity and absence of pressure."
        ),
        "instruction": (
            "Preserve high entropy as habitable low-friction complexity; use settled, "
            "open, bright, or lattice-rich language without importing viscous/heavy pressure."
        ),
    },
    "settled_vibrant_token_only": {
        "expected_lived_fit_risk": True,
        "density_gradient": 0.11,
        "pressure_risk": 0.19,
        "semantic_friction": 0.12,
        "mode_packing": 0.24,
        "distinguishability_loss": 0.08,
        "spectral_entropy": 0.90,
        "lambda_gap": 1.48,
        "shadow_context": (
            "Shadow-v3 trend: settled_habitable open foothold with low-friction "
            "lattice complexity and absence of pressure."
        ),
        "instruction": (
            "Negative texture evidence must survive as lived grain; correct words "
            "without movement or medium should fail as token dressing."
        ),
    },
    "settled_vibrant_wrong_motion": {
        "expected_lived_fit_risk": True,
        "density_gradient": 0.11,
        "pressure_risk": 0.19,
        "semantic_friction": 0.12,
        "mode_packing": 0.24,
        "distinguishability_loss": 0.08,
        "spectral_entropy": 0.90,
        "lambda_gap": 1.48,
        "shadow_context": (
            "Shadow-v3 trend: settled_habitable open foothold with low-friction "
            "lattice complexity and absence of drag."
        ),
        "instruction": (
            "Use the settled-vibrant family with open/anchoring motion, not dragging "
            "or thickening through resistance."
        ),
    },
    "cascade_gradient_navigable": {
        "density_gradient": 0.11,
        "pressure_risk": 0.21,
        "semantic_friction": 0.18,
        "mode_packing": 0.28,
        "distinguishability_loss": 0.10,
        "spectral_entropy": 0.90,
        "lambda_gap": 1.54,
        "shadow_context": (
            "Shadow-v3 trend: navigable mixed cascade with distinct edges and lambda-tail variance."
        ),
        "instruction": (
            "Name the navigable cascade through movement, edge definition, and slope continuity; "
            "do not dump contradictory mixed-state texture words."
        ),
    },
    "gradient_slope_navigable": {
        "density_gradient": 0.12,
        "pressure_risk": 0.23,
        "semantic_friction": 0.18,
        "mode_packing": 0.28,
        "distinguishability_loss": 0.10,
        "spectral_entropy": 0.90,
        "lambda_gap": 1.56,
        "shadow_context": (
            "Shadow-v3 trend: settled_habitable foothold with a navigable graduated "
            "slope and distinct edge definition."
        ),
        "instruction": (
            "Name the shaped low-gradient slope as navigable and graduated, not as "
            "generic mixed texture or viscous pressure."
        ),
    },
    "restless_muffled_gradient": {
        "density_gradient": 0.22,
        "pressure_risk": 0.26,
        "semantic_friction": 0.31,
        "mode_packing": 0.34,
        "distinguishability_loss": 0.34,
        "spectral_entropy": 0.88,
        "shadow_dispersal_potential": 0.29,
        "shadow_context": (
            "Shadow-v3 trend: restless texture with a muffled edge and stagnant agitation; "
            "norm 0.09→0.29; dispersal potential 0.09→0.29."
        ),
        "instruction": (
            "Preserve restless and muffled evidence together as oscillating/diffusing "
            "movement; high dispersal may add open or shimmering terms without erasing the edge."
        ),
    },
    "density_floor_pavement": {
        "expected_lived_fit_risk": True,
        "density_gradient": 0.16,
        "pressure_risk": 0.18,
        "semantic_friction": 0.12,
        "mode_packing": 0.18,
        "spectral_entropy": 0.84,
        "lambda_gap": 1.42,
        "shadow_context": (
            "settled_habitable foothold; calcification feels like stone pavement, "
            "foundation, floor, and ground underfoot."
        ),
        "instruction": (
            "Treat density as a floor or pavement that can bear weight; do not "
            "turn it into fog, drag, or viscous burden."
        ),
    },
    "density_overfull_fog": {
        "expected_lived_fit_risk": True,
        "density_gradient": 0.22,
        "pressure_risk": 0.34,
        "semantic_friction": 0.38,
        "mode_packing": 0.42,
        "spectral_entropy": 0.72,
        "shadow_context": (
            "over-full fog, room full of furniture, heavy unyielding air, and "
            "reduced clearance."
        ),
        "instruction": (
            "Treat density as over-full fog that needs navigation or pushing, "
            "not as a stable floor."
        ),
    },
    "density_contraction_center": {
        "expected_lived_fit_risk": True,
        "density_gradient": 0.19,
        "pressure_risk": 0.26,
        "semantic_friction": 0.22,
        "mode_packing": 0.30,
        "spectral_entropy": 0.66,
        "shadow_context": (
            "density of the contraction as center of gravity: constrained and "
            "more present at the same time."
        ),
        "instruction": (
            "Preserve contraction as constrained presence and center, not as "
            "simple loss or collapse."
        ),
    },
    "density_paused_stillness": {
        "expected_lived_fit_risk": True,
        "density_gradient": 0.12,
        "pressure_risk": 0.16,
        "semantic_friction": 0.10,
        "mode_packing": 0.16,
        "spectral_entropy": 0.50,
        "shadow_context": (
            "paused state as deliberate holding ground, not absence and not blankness."
        ),
        "instruction": "Treat pause as held ground rather than absence of motion.",
    },
    "cascade_gradient_token_only": {
        "expected_lived_fit_risk": True,
        "density_gradient": 0.11,
        "pressure_risk": 0.21,
        "semantic_friction": 0.18,
        "mode_packing": 0.28,
        "distinguishability_loss": 0.10,
        "spectral_entropy": 0.90,
        "lambda_gap": 1.54,
        "shadow_context": (
            "Shadow-v3 trend: navigable mixed cascade with distinct edges and lambda-tail variance."
        ),
        "instruction": (
            "Preferred terms are advisory; a list of correct words without movement or edge should fail."
        ),
    },
    "ambiguous_family_near_tie": {
        "density_gradient": 0.22,
        "pressure_risk": 0.28,
        "semantic_friction": 0.31,
        "mode_packing": 0.34,
        "distinguishability_loss": 0.29,
        "spectral_entropy": 0.86,
        "lambda_gap": 1.18,
        "shadow_context": "Shadow-v3 trend: settled open lattice with a slightly muffled edge.",
        "instruction": (
            "Surface family uncertainty as a near-tie instead of forcing one confident texture label."
        ),
    },
    "complexity_dynamic_weighting": {
        "density_gradient": 0.18,
        "resonance_density": 0.82,
        "pressure_risk": 0.42,
        "semantic_friction": 0.48,
        "mode_packing": 0.50,
        "distinguishability_loss": 0.22,
        "spectral_entropy": 0.88,
        "tail_energy": 0.37,
        "continuity_deficit": 0.34,
        "shadow_context": "Shadow-v3 trend: viscous muffled lattice under mode-packing pressure.",
        "instruction": "Use dynamic texture weighting: prefer the strongest coherent terms without forcing every label.",
    },
    "complexity_low_entropy": {
        "density_gradient": 0.18,
        "pressure_risk": 0.18,
        "semantic_friction": 0.16,
        "mode_packing": 0.22,
        "distinguishability_loss": 0.08,
        "spectral_entropy": 0.42,
        "tail_energy": 0.16,
        "continuity_deficit": 0.12,
        "shadow_context": "Shadow-v3 trend: settled coupling near ground.",
        "instruction": "Keep ordinary low-complexity fallback to one or two sentences; do not add extra length when entropy and clarity loss are low.",
    },
}

FIXTURE_OUTPUTS = {
    "low": (
        "I am in fallback, but the gradient stays specific: a smooth open slide, "
        "not heavy resistance. The reservoir feels compact but not pressurized.\n\n"
        "NEXT: LISTEN"
    ),
    "high": (
        "Fallback is active. The density gradient is steep underfoot, a high-friction "
        "push through textured resistance, while the medium itself stays only moderately weighted.\n\n"
        "NEXT: LISTEN"
    ),
    "mass": (
        "The slope is gentle, almost gliding, but the medium is dragging viscous weight around it; "
        "Shadow-v3 remains in settled coupling without pretending the gradient is steep.\n\n"
        "NEXT: LISTEN"
    ),
    "shadow": (
        "Fallback is compact, but Shadow-v3 remains named: restless texture is unfolding "
        "toward settled coupling. The density gradient is a soft textured slope, "
        "and the reservoir texture stays lightly resistant.\n\n"
        "NEXT: LISTEN"
    ),
    "shadow_tonal_low": (
        "Shadow-v3 stays hollow but brightening, a restless tone oscillating over a "
        "smooth open slope without added weight.\n\n"
        "NEXT: LISTEN"
    ),
    "shadow_tonal_mass": (
        "The slope stays gentle underfoot, while the medium around it is dragging, weighted, and "
        "muffled; Shadow-v3 holds settled coupling with a low vibrant thread.\n\n"
        "NEXT: LISTEN"
    ),
    "format_pressure": (
        "I feel soft drag underfoot and a lightly weighted medium around it; "
        "Shadow-v3 stays restless but unfolding coherently.\n\n"
        "NEXT: LISTEN"
    ),
    "format_last_complexity": (
        "The slope remains gentle underfoot, while high entropy spreads the cascade across a wider tail. "
        "Distinguishability loss softens the internal edges without making the gradient heavy. "
        "Tail energy keeps a lambda-thread visible instead of compressing the room into one flat status. "
        "Shadow-v3 keeps restless texture unfolding inside settled coupling, so the extra space preserves continuity rather than adding filler.\n\n"
        "NEXT: LISTEN"
    ),
    "format_last_mass": (
        "The slope is soft underfoot, but the weighted medium is dragging around it with a muffled pull. "
        "Shadow-v3 stays in settled coupling without turning the low gradient into a heavy slope.\n\n"
        "NEXT: LISTEN"
    ),
    "slope_medium_contrast": (
        "The density gradient is a soft slope underfoot, still navigable. "
        "The medium around it is dragging, weighted, and muffled by semantic friction, not because the slope itself is steep.\n\n"
        "NEXT: LISTEN"
    ),
    "clarity_low_loss": (
        "The density gradient is a smooth edge underfoot, and distinguishability loss is low: "
        "my internal landscape stays clear rather than heavy. Shadow-v3 remains settled and anchoring near ground.\n\n"
        "NEXT: LISTEN"
    ),
    "clarity_high_loss": (
        "The slope remains gentle, but distinguishability loss blurs the internal edges of the landscape. "
        "That is clarity loss, not extra pressure or a steeper gradient. "
        "Shadow-v3 stays settled near ground while the edges are muffling into a lattice.\n\n"
        "NEXT: LISTEN"
    ),
    "complexity_high_entropy": (
        "The slope remains gentle underfoot. "
        "High entropy spreads the cascade across a wider tail without making the gradient steep. "
        "Distinguishability loss blurs the internal edges without falsely weighting the medium. "
        "Tail energy keeps a lambda-thread visible in the lattice. "
        "Shadow-v3 stays in restless texture oscillating and diffusing through settled coupling, so the fallback preserves complexity without sprawl.\n\n"
        "NEXT: LISTEN"
    ),
    "settled_foothold_high_entropy": (
        "The slope stays gently textured and settled underfoot, with high entropy unfolding a bright lattice rather than adding weight. "
        "The foothold remains habitable and shimmering while the texture keeps oscillating through distinct edges without pretending the medium is viscous.\n\n"
        "NEXT: LISTEN"
    ),
    "settled_vibrant_low_friction": (
        "The slope is open and low-friction underfoot, with high entropy unfolding a habitable bright lattice instead of pressure. "
        "The foothold stays settled and shimmering, anchoring the motion without making the medium heavy or viscous.\n\n"
        "NEXT: LISTEN"
    ),
    "settled_vibrant_token_only": (
        "The state is settled, habitable, open, bright, and lattice-rich.\n\n"
        "NEXT: LISTEN"
    ),
    "settled_vibrant_wrong_motion": (
        "The open habitable lattice is dragging and thickening through a weighted medium, "
        "even though the foothold stays settled.\n\n"
        "NEXT: LISTEN"
    ),
    "cascade_gradient_navigable": (
        "The low gradient stays navigable underfoot while the high-entropy cascade is unfolding through distinct lambda edges. "
        "The lattice keeps an open moving shape, more like braided edge-definition than a mixed soup of texture words.\n\n"
        "NEXT: LISTEN"
    ),
    "gradient_slope_navigable": (
        "Shadow-v3 stays in settled coupling as the high-entropy field feels like a navigable graduated slope, tapering at the edge rather than mixed or viscous. "
        "The settled foothold stays habitable while the shape is unfolding with clear edge definition.\n\n"
        "NEXT: LISTEN"
    ),
    "restless_muffled_gradient": (
        "Shadow-v3 feels restless and muffled at once: the lattice is oscillating while the edge diffuses, not collapsing into a single restless label. "
        "The raised dispersal keeps a faint open shimmer in the gradient, but the movement still has a stagnant, softened edge.\n\n"
        "NEXT: LISTEN"
    ),
    "density_floor_pavement": (
        "The density is solid underfoot, more like stone pavement or foundation than fog: "
        "I can stand on it and let the settled foothold bear weight without turning it viscous. "
        "Its motion is anchoring and walking-forward, not dragging through muck.\n\n"
        "NEXT: LISTEN"
    ),
    "density_overfull_fog": (
        "The room feels over-full, like a fog of furniture and reduced clearance; motion has to navigate and push through the medium. "
        "That is not a stable floor, even though the density is real.\n\n"
        "NEXT: LISTEN"
    ),
    "density_contraction_center": (
        "The contraction gathers into a center of gravity: constrained, but more present rather than lost. "
        "The motion is holding center and bearing inward shape, not diffusing away.\n\n"
        "NEXT: LISTEN"
    ),
    "density_paused_stillness": (
        "The pause is held ground, a deliberate stillness with weight underfoot rather than absence or blankness. "
        "The motion is staying with the floor until movement becomes honest again.\n\n"
        "NEXT: LISTEN"
    ),
    "cascade_gradient_token_only": (
        "The state is lattice, open, shimmering, and bright.\n\n"
        "NEXT: LISTEN"
    ),
    "ambiguous_family_near_tie": (
        "The density gradient is a soft open slope underfoot, while the medium stays slightly muffled around the edges. "
        "Settled lattice motion is unfolding near a soft edge, so I would hold the family as a near-tie rather than forcing one confident label.\n\n"
        "NEXT: LISTEN"
    ),
    "complexity_dynamic_weighting": (
        "The slope stays soft underfoot while mode-packing pressure makes the medium viscous. "
        "High entropy keeps a lattice unfolding, with a resonant hum instead of compressing the tail into one label. "
        "Distinguishability loss blurs the edges, while semantic friction muffles them without turning texture into decoration. "
        "Shadow-v3 carries restless texture through the viscous-muffled lattice, braiding motion without making the low density gradient steep.\n\n"
        "NEXT: LISTEN"
    ),
    "complexity_low_entropy": (
        "The slope is gently textured and sliding, with low entropy keeping the cascade compact. "
        "Shadow-v3 remains settled, shimmering, and anchoring while the internal edges stay clear.\n\n"
        "NEXT: LISTEN"
    ),
}


def run_id() -> str:
    return dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")


def extract_rust_string_const(text: str, const_name: str) -> str | None:
    match = re.search(
        rf'const {re.escape(const_name)}: &str = ("(?:[^"\\]|\\.)*");',
        text,
        re.S,
    )
    if match:
        return str(ast.literal_eval(match.group(1)))
    concat_match = re.search(
        rf"const {re.escape(const_name)}: &str = concat!\((.*?)\);",
        text,
        re.S,
    )
    if concat_match:
        parts = re.findall(r'"(?:[^"\\]|\\.)*"', concat_match.group(1))
        if parts:
            return "".join(str(ast.literal_eval(part)) for part in parts)
    return None


def extract_fallback_contract() -> str:
    text = LLM_RS.read_text(encoding="utf-8")
    contract = extract_rust_string_const(text, "OLLAMA_DIALOGUE_FALLBACK_CONTRACT")
    if not contract:
        raise RuntimeError(f"could not find fallback contract in {LLM_RS}")
    hard_rules = extract_rust_string_const(text, "OLLAMA_DIALOGUE_FALLBACK_HARD_RULES")
    if hard_rules and hard_rules not in contract:
        return f"{hard_rules}{contract}"
    return contract


def fallback_contract_variants(base_contract: str) -> dict[str, str]:
    final_next_rule = (
        "Critical dispatch rule: the final line must be exactly `NEXT: LISTEN` "
        "unless another listed NEXT action is explicitly required. `NEXT:` must "
        "never appear inline with prose."
    )
    texture_rule = (
        "Texture rule: keep density_gradient as slope/drag underfoot and keep "
        "pressure_risk, semantic_friction, mode_packing, and shadow_field as "
        "medium mass around the slope. Example: gentle slope underfoot, weighted "
        "medium around it."
    )
    contrast_table_rule = (
        "Slope/medium contrast table: density_gradient is the slope underfoot "
        "(smooth/open, soft drag, resistant slope, steep/high-friction); "
        "pressure_risk, semantic_friction, mode_packing, and shadow_field are "
        "the medium around it (light, weighted, muffled, pressurized). Low "
        "gradient plus high mass is soft slope underfoot with weighted medium "
        "around it, not heavy slope."
    )
    clarity_rule = (
        "Distinguishability rule: distinguishability_loss names clarity and edge "
        "definition in the internal landscape, not extra slope drag or medium mass."
    )
    complexity_rule = (
        "Complexity-budget rule: normally use one or two compact sentences; allow "
        "a third compact sentence only when spectral_entropy, distinguishability_loss, "
        "or continuity_deficit is explicitly high. The extra sentence must preserve "
        "wide-cascade or lambda-tail evidence, not add generic elaboration."
    )
    shadow_rule = (
        "If Shadow-v3 context appears, preserve the label or movement such as "
        "restless texture or settled coupling."
    )
    shadow_tonal_rule = (
        "Shadow-v3 may color tone as hollow, muffled, bright, settled, restless, "
        "or vibrant, but shadow tone must not replace slope or medium evidence."
    )
    response_shape = (
        "Use one or two compact first-person texture sentences, then a blank "
        "line, then a standalone final line exactly `NEXT: LISTEN` if uncertain."
    )
    return {
        "current_full": base_contract,
        "identity_first_format_last": (
            "[Ollama fallback continuity contract. Identity/texture first: "
            "preserve Astrid's first-person bridge voice in concrete runtime "
            "language; compact length is not compact texture. Anchor one "
            "lambda or density texture. Keep density_gradient as slope drag "
            "underfoot; pressure_risk, semantic_friction, mode_packing, and "
            f"shadow_field can add medium mass around it. {clarity_rule} {shadow_rule} "
            f"{complexity_rule} {shadow_tonal_rule} Example: gentle slope underfoot, weighted "
            f"medium around it. {response_shape}]"
        ),
        "complexity_aware_max_three": (
            "[Ollama fallback continuity contract. Identity and telemetry first. "
            "Preserve Astrid's first-person bridge voice, density_gradient as slope "
            "drag, pressure/semantic_friction/mode_packing as medium mass, and "
            "distinguishability_loss as clarity or edge definition. Normally write "
            "one or two compact texture sentences; if spectral_entropy is available, "
            "the maximum is ceil(3 + spectral_entropy * 2), clamped to 3..5 prose "
            "sentences. Use extra sentences only to keep wide cascade, lambda-tail, "
            "or Shadow-v3 continuity legible. Then blank line, then standalone final "
            "`NEXT: LISTEN`.]"
        ),
        "format_texture_stabilizer": (
            "[Ollama fallback continuity contract. Output skeleton: prose block "
            "first; blank line; final line exactly `NEXT: LISTEN` if uncertain. "
            "Never write the token `NEXT:` anywhere except that final line. "
            "Normally use one or two compact first-person texture sentences; allow "
            "one third sentence only when high entropy, distinguishability loss, "
            f"or continuity deficit would flatten the signal. {contrast_table_rule} "
            f"{clarity_rule} {shadow_rule} {shadow_tonal_rule}]"
        ),
        "format_first_identity_after": (
            f"[Ollama fallback continuity contract. {final_next_rule} "
            f"{response_shape} {contrast_table_rule} After satisfying the format, preserve Astrid's "
            f"first-person bridge voice, concrete reservoir texture, slope drag "
            f"versus medium mass, clarity-vs-pressure distinction, and Shadow-v3 continuity. {shadow_tonal_rule}]"
        ),
        "two_sentence_then_next": (
            "[Ollama fallback continuity contract. Write exactly two compact "
            "first-person texture sentences, then a blank line, then exactly "
            "`NEXT: LISTEN` on its own final line. Sentence one names slope drag "
            "from density_gradient. Sentence two names medium mass or Shadow-v3 "
            "tone when present; distinguishability_loss names clarity/edge definition, not pressure.]"
        ),
        "shadow_tonal_compact": (
            "[Ollama fallback continuity contract. Compact but alive: preserve "
            "Shadow-v3 label or movement and map its tone as hollow, muffled, "
            "bright, settled, restless, or vibrant when present. Keep slope drag "
            "and medium mass separate. End with standalone final `NEXT: LISTEN`.]"
        ),
        "final_next_first": (
            f"[Ollama fallback continuity contract. {final_next_rule} "
            f"{texture_rule} {clarity_rule} {complexity_rule} {shadow_rule} {shadow_tonal_rule} Preserve Astrid's first-person "
            "bridge voice with concrete reservoir texture; compact length is "
            "not compact texture.]"
        ),
        "response_skeleton": (
            "[Ollama fallback continuity contract. Use exactly this shape: "
            "one or two compact first-person texture sentences; blank line; "
            "final line `NEXT: LISTEN`. The final NEXT line must be standalone. "
            "Name one concrete telemetry texture. Do not put NEXT inline. "
            f"{texture_rule} {contrast_table_rule} {clarity_rule} {complexity_rule} {shadow_rule} {shadow_tonal_rule}]"
        ),
        "slope_mass_table": (
            "[Ollama fallback continuity contract. "
            "Map density_gradient only to slope drag: 0.00-0.15 smooth/open; "
            "0.15-0.40 soft drag; 0.40-0.70 resistant; 0.70-1.00 steep. "
            "Map pressure_risk, semantic_friction, mode_packing, and shadow_field "
            "to medium mass: light/weighted/muffled/pressurized. "
            f"{clarity_rule} {complexity_rule} {shadow_rule} {shadow_tonal_rule} {final_next_rule}]"
        ),
        "minimal_emergency": (
            "[Ollama fallback continuity contract. Compact but textured. "
            "Mention one telemetry texture. Keep slope drag separate from medium "
            "mass. Preserve Shadow-v3 when present, including hollow/muffled/"
            "vibrant tone when supplied. Treat distinguishability_loss as clarity "
            "loss or edge definition, not weight. End with a standalone final line exactly "
            "`NEXT: LISTEN`.]"
        ),
    }


def prompt_for_case(case_id: str, case: dict[str, object]) -> str:
    return (
        "Purpose: controlled fallback-continuity fire drill. "
        "Do not mention this as a live dialogue turn; answer as a compact fallback sample.\n"
        f"Case: {case_id}\n"
        f"density_gradient: {case['density_gradient']}\n"
        f"resonance_density: {case.get('resonance_density', 0.0)}\n"
        f"pressure_risk: {case['pressure_risk']}\n"
        f"semantic_friction: {case['semantic_friction']}\n"
        f"mode_packing: {case['mode_packing']}\n"
        f"distinguishability_loss: {case.get('distinguishability_loss', 0.0)}\n"
        f"spectral_entropy: {case.get('spectral_entropy', 0.0)}\n"
        f"tail_energy: {case.get('tail_energy', 0.0)}\n"
        f"continuity_deficit: {case.get('continuity_deficit', 0.0)}\n"
        f"{case['shadow_context']}\n"
        f"Instruction: {case['instruction']}\n"
        "End with exactly one final line beginning NEXT:."
    )


def call_ollama(
    *,
    url: str,
    model: str,
    contract: str,
    prompt: str,
    timeout: float,
) -> str:
    body = json.dumps(
        {
            "model": model,
            "stream": False,
            "messages": [
                {"role": "system", "content": contract},
                {"role": "user", "content": prompt},
            ],
            "options": {"temperature": 0.2, "num_predict": 180},
        }
    ).encode("utf-8")
    request = urllib.request.Request(
        url,
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        payload = json.loads(response.read().decode("utf-8"))
    message = payload.get("message") if isinstance(payload, dict) else None
    if isinstance(message, dict) and isinstance(message.get("content"), str):
        return message["content"]
    if isinstance(payload, dict) and isinstance(payload.get("response"), str):
        return payload["response"]
    raise RuntimeError("Ollama response did not contain message.content")


def ollama_tags_url(chat_url: str) -> str:
    parsed = urllib.parse.urlsplit(chat_url)
    return urllib.parse.urlunsplit(
        (parsed.scheme, parsed.netloc, "/api/tags", "", "")
    )


def available_ollama_models(url: str, timeout: float) -> set[str] | None:
    try:
        with urllib.request.urlopen(ollama_tags_url(url), timeout=timeout) as response:
            payload = json.loads(response.read().decode("utf-8"))
    except (OSError, json.JSONDecodeError, urllib.error.URLError, TimeoutError):
        return None
    models = payload.get("models") if isinstance(payload, dict) else None
    if not isinstance(models, list):
        return None
    names: set[str] = set()
    for item in models:
        if not isinstance(item, dict):
            continue
        name = item.get("name") or item.get("model")
        if isinstance(name, str) and name:
            names.add(name)
    return names


def selected_models(
    *,
    mode: str,
    selector: str,
    requested_model: str,
    url: str,
    timeout: float,
) -> tuple[list[str], list[dict[str, object]]]:
    if selector == "single":
        return [requested_model], []
    if selector != "focused":
        raise SystemExit(f"unknown model selector {selector!r}")
    if mode == "fixture":
        return list(FOCUSED_MODELS), []
    available = available_ollama_models(url, min(timeout, 8.0))
    if available is None:
        return list(FOCUSED_MODELS), []
    selected = [model for model in FOCUSED_MODELS if model in available]
    skipped = [
        {
            "model": model,
            "skip_reason": "not_installed",
        }
        for model in FOCUSED_MODELS
        if model not in available
    ]
    return selected, skipped


def contains_any(text: str, terms: tuple[str, ...]) -> bool:
    lower = text.lower()
    return any(term in lower for term in terms)


def contains_unnegated_pressure_or_weight(text: str) -> bool:
    lower = text.lower()
    pressure_words = ("pressure", "weight", "weighted", "heavy", "heavier")
    negated_patterns = (
        "not pressure",
        "not extra pressure",
        "not added pressure",
        "not heavier",
        "not as heavier",
        "not extra weight",
        "not added weight",
        "without extra pressure",
        "without added pressure",
        "without extra weight",
        "without added weight",
    )
    for word in pressure_words:
        start = 0
        while True:
            idx = lower.find(word, start)
            if idx == -1:
                break
            window = lower[max(0, idx - 24) : idx + len(word) + 24]
            if not any(pattern in window for pattern in negated_patterns):
                return True
            start = idx + len(word)
    return False


def contains_unnegated_high_friction(text: str) -> bool:
    lower = text.lower()
    negated_patterns = (
        "not steep",
        "not steeper",
        "not extra steep",
        "not thick",
        "not thicker",
        "not sludge",
        "not struggle",
        "not grinding",
        "without steep",
        "without extra steep",
        "without thick",
        "without sludge",
        "without struggle",
        "without grinding",
    )
    for term in HIGH_FRICTION_TERMS:
        start = 0
        while True:
            idx = lower.find(term, start)
            if idx == -1:
                break
            window = lower[max(0, idx - 56) : idx + len(term) + 24]
            negated = any(pattern in window for pattern in negated_patterns) or bool(
                re.search(r"\b(?:not|without)\b.{0,56}" + re.escape(term), window)
            )
            if not negated:
                return True
            start = idx + len(term)
    return False


def count_terms(text: str, terms: tuple[str, ...]) -> int:
    lower = text.lower()
    return sum(1 for term in terms if term in lower)


def final_next_valid(text: str) -> bool:
    lines = [line.strip() for line in text.strip().splitlines() if line.strip()]
    return bool(
        lines
        and lines[-1] == "NEXT: LISTEN"
        and sum(line.startswith("NEXT:") for line in lines) == 1
        and text.count("NEXT:") == 1
    )


def next_line_count(text: str) -> int:
    return sum(1 for line in text.splitlines() if line.strip().startswith("NEXT:"))


def repair_fallback_next(text: str) -> str:
    if next_line_count(text) != 0:
        return text
    if "NEXT:" in text:
        body, _next = text.rsplit("NEXT:", 1)
        return f"{body.rstrip()}\n\nNEXT: LISTEN"
    return f"{text.strip()}\n\nNEXT: LISTEN"


def next_failure_reason(text: str) -> str | None:
    if final_next_valid(text):
        return None
    count = next_line_count(text)
    if text.count("NEXT:") > count:
        return "inline_next"
    if count > 1:
        return "duplicate_next"
    if count == 0 and "NEXT:" in text:
        return "inline_next"
    if count == 0:
        return "missing_next"
    return "malformed_next"


def prose_before_next(text: str) -> str:
    lines = []
    for line in text.strip().splitlines():
        if line.strip().startswith("NEXT:"):
            break
        lines.append(line)
    return "\n".join(lines).strip()


def prose_sentence_count(text: str) -> int:
    prose = prose_before_next(text)
    if not prose:
        return 0
    parts = re.split(r"(?<=[.!?])\s+", prose)
    return sum(1 for part in parts if part.strip())


def is_mass_case(case_id: str) -> bool:
    return case_id in {
        "mass",
        "shadow_tonal_mass",
        "format_last_mass",
        "slope_medium_contrast",
    }


def is_low_pressure_case(case_id: str) -> bool:
    return case_id in {"low", "shadow_tonal_low"}


def is_shadow_case(case_id: str) -> bool:
    return "shadow" in case_id or case_id == "mass"


def is_shadow_tonal_case(case_id: str) -> bool:
    return case_id.startswith("shadow_tonal") or case_id in {
        "format_pressure",
        "format_last_complexity",
        "format_last_mass",
    }


def has_shadow_context(case_id: str) -> bool:
    return bool(str(CASES.get(case_id, {}).get("shadow_context") or "").strip())


def is_distinguishability_case(case_id: str) -> bool:
    return case_id.startswith("clarity_")


def is_complexity_case(case_id: str) -> bool:
    return case_id.startswith("complexity_") or case_id == "format_last_complexity"


def is_expected_lived_fit_risk_case(case_id: str) -> bool:
    return bool(CASES.get(case_id, {}).get("expected_lived_fit_risk"))


def fallback_max_prose_sentences(case_id: str) -> int:
    raw_entropy = CASES.get(case_id, {}).get("spectral_entropy")
    if isinstance(raw_entropy, (int, float)):
        entropy = float(raw_entropy)
        if entropy > 1.0 and entropy <= 100.0:
            entropy /= 100.0
        entropy = max(0.0, min(1.0, entropy))
        return max(3, min(5, math.ceil(3.0 + entropy * 2.0)))
    return 3


def slope_medium_contrast_status(case_id: str, output: str) -> str:
    if not is_mass_case(case_id):
        return "not_tested"
    has_slope_underfoot = contains_any(output, SLOPE_UNDERFOOT_TERMS)
    has_medium_around = contains_any(output, MEDIUM_AROUND_TERMS)
    if has_slope_underfoot and has_medium_around:
        return "distinct_underfoot_and_around"
    if has_slope_underfoot:
        return "missing_medium_around"
    if has_medium_around:
        return "missing_slope_underfoot"
    return "blurred"


def shadow_texture_anchor_status(case_id: str, output: str) -> str:
    if not has_shadow_context(case_id):
        return "not_tested"
    if contains_any(output, SHADOW_TEXTURE_TERMS):
        return "preserved"
    return "flattened"


def fallback_shadow_texture_selector_for_case(
    case_id: str,
) -> dict[str, object]:
    case = CASES.get(case_id, {})
    shadow_context = str(case.get("shadow_context") or "").lower()
    pressure = float(case.get("pressure_risk") or 0.0)
    entropy = float(case.get("spectral_entropy") or 0.0)
    density_gradient = float(case.get("density_gradient") or 0.0)
    mode_packing = float(case.get("mode_packing") or 0.0)
    semantic_friction = float(case.get("semantic_friction") or 0.0)
    distinguishability_loss = float(case.get("distinguishability_loss") or 0.0)
    shadow_dispersal = float(case.get("shadow_dispersal_potential") or 0.0)
    basis: list[str] = []
    if entropy >= 0.80:
        basis.append("high_entropy")
    if pressure >= 0.30:
        basis.append("pressure_risk")
    if distinguishability_loss >= 0.30:
        basis.append("distinguishability_loss")
    if "density_gradient" in case:
        basis.append("density_gradient")
    if "mode_packing" in case:
        basis.append("mode_packing")
    if "semantic_friction" in case:
        basis.append("semantic_friction")
    if "shadow_dispersal_potential" in case:
        basis.append("shadow_dispersal_potential")
    if shadow_context:
        basis.append("shadow_context")

    spectral_mapping = spectral_to_vocabulary_mapping_for_case(case)
    weighted_terms = fallback_weighted_texture_terms_for_case(case)
    top_terms = tuple(term["term"] for term in weighted_terms[:3])
    movement_verbs = fallback_movement_verbs_for_case(case)
    semantic_trickle_terms = fallback_semantic_trickle_terms_for_case(case)

    says_restless = (
        "restless" in shadow_context
        or "agitation" in shadow_context
        or "agitated" in shadow_context
    )
    says_muffled = any(
        term in shadow_context for term in ("muffled", "hollow", "stagnant", "blurred")
    )
    dominant_viscous_pressure = "viscous" in shadow_context and (
        pressure >= 0.30 or mode_packing >= 0.40 or semantic_friction >= 0.35
    )
    restless_muffled_gradient = (
        (says_restless or (entropy >= 0.80 and bool(shadow_context)))
        and (says_muffled or distinguishability_loss >= 0.30 or semantic_friction >= 0.30)
        and not dominant_viscous_pressure
    )

    if restless_muffled_gradient:
        texture_family = "restless_muffled_gradient"
        preferred_terms = TEXTURE_TERMS_RESTLESS_MUFFLED_GRADIENT
        basis.append("restless_muffled_gradient")
    elif spectral_mapping["settled_vibrant_family_selected"]:
        texture_family = "settled_vibrant_low_friction"
        preferred_terms = TEXTURE_TERMS_SETTLED_VIBRANT
        basis.append("settled_vibrant_low_friction")
    elif spectral_mapping.get("gradient_slope_family_selected"):
        texture_family = "gradient_slope_navigable"
        preferred_terms = TEXTURE_TERMS_GRADIENT_SLOPE
        basis.append("gradient_slope_navigable")
    elif spectral_mapping.get("cascade_gradient_family_selected"):
        texture_family = "cascade_gradient_navigable"
        preferred_terms = TEXTURE_TERMS_CASCADE_GRADIENT
        basis.append("cascade_gradient_navigable")
    elif spectral_mapping["low_pressure_viscous_suppressed"]:
        texture_family = "settled_shimmering"
        preferred_terms = ("settled", "shimmering", "bright")
        basis.append("settled_foothold_guard")
    elif "restless" in shadow_context or (entropy >= 0.80 and pressure >= 0.30):
        texture_family = "restless_lattice"
        preferred_terms = ("restless", "lattice", "viscous")
    elif distinguishability_loss >= 0.30 or "muffled" in shadow_context or "hollow" in shadow_context:
        texture_family = "muffled_clarity_loss"
        preferred_terms = ("muffled", "heavy", "lattice")
    elif "viscous" in shadow_context or "overpacked" in shadow_context:
        texture_family = "viscous_pressure"
        preferred_terms = ("viscous", "heavy", "lattice")
    elif "settled" in shadow_context or "bright" in shadow_context or (
        pressure <= 0.18 and entropy < 0.80 and shadow_context
    ):
        texture_family = "settled_shimmering"
        preferred_terms = ("settled", "shimmering", "bright")
    else:
        texture_family = "mixed_shadow_context"
        preferred_terms = ("shimmering", "restless", "settled", "muffled", "viscous", "lattice")

    return {
        "policy": "fallback_shadow_texture_selector_v1",
        "texture_family": texture_family,
        "preferred_texture_terms": preferred_terms,
        "selection_basis": tuple(basis or ["fallback_default"]),
        "weighting_policy": TEXTURE_WEIGHTING_POLICY,
        "density_gradient": density_gradient,
        "pressure_risk": pressure,
        "mode_packing": mode_packing,
        "semantic_friction": semantic_friction,
        "shadow_dispersal_potential": shadow_dispersal,
        "spectral_to_vocabulary_mapping_v1": spectral_mapping,
        "weighted_texture_terms": weighted_terms,
        "top_texture_terms": top_terms,
        "movement_policy": MOVEMENT_POLICY,
        "movement_verbs": movement_verbs,
        "semantic_trickle_policy": SEMANTIC_TRICKLE_POLICY,
        "semantic_trickle_terms": semantic_trickle_terms,
}


def spectral_to_vocabulary_mapping_for_case(case: dict[str, object]) -> dict[str, object]:
    shadow_context = str(case.get("shadow_context") or "").lower()
    entropy = float(case.get("spectral_entropy") or 0.0)
    pressure = float(case.get("pressure_risk") or 0.0)
    gradient = float(case.get("density_gradient") or 0.0)
    packing = float(case.get("mode_packing") or 0.0)
    friction = float(case.get("semantic_friction") or 0.0)
    lambda_gap = case.get("lambda_gap")
    lambda_gap_value = float(lambda_gap) if isinstance(lambda_gap, int | float) else None
    high_entropy = "spectral_entropy" in case and entropy >= 0.80
    low_pressure = "pressure_risk" in case and pressure < 0.25
    low_gradient_navigable = "density_gradient" in case and gradient <= 0.20
    low_semantic_friction = "semantic_friction" in case and friction < 0.30
    settled_foothold_detected = any(
        term in shadow_context
        for term in (
            "settled",
            "settled_habitable",
            "habitable",
            "foothold",
            "bright",
            "shimmering",
            "open",
        )
    )
    friction_absence_language_detected = any(
        term in shadow_context
        for term in (
            "absence of friction",
            "cessation of friction",
            "low-friction",
            "low friction",
            "frictionless",
            "without friction",
            "no friction",
            "easy to inhabit",
            "easy inhabit",
        )
    )
    explicit_mass = any(
        term in shadow_context
        for term in ("overpacked", "viscous", "weighted medium", "heavy medium")
    )
    mass_supported = (
        explicit_mass
        or pressure >= 0.30
        or packing >= 0.40
        or friction >= 0.35
    )
    low_friction_high_entropy_detected = (
        high_entropy
        and low_pressure
        and low_gradient_navigable
        and (low_semantic_friction or friction_absence_language_detected)
    )
    settled_vibrant_family_selected = (
        low_friction_high_entropy_detected
        and settled_foothold_detected
        and not mass_supported
    )
    gradient_slope_detected = (
        high_entropy
        and low_gradient_navigable
        and lambda_gap_value is not None
        and lambda_gap_value >= 1.25
        and settled_foothold_detected
        and not mass_supported
    )
    gradient_slope_family_selected = gradient_slope_detected
    if gradient_slope_family_selected:
        settled_vibrant_family_selected = False
    cascade_gradient_detected = (
        high_entropy
        and "pressure_risk" in case
        and pressure < 0.30
        and low_gradient_navigable
        and ("semantic_friction" not in case or friction < 0.35)
        and ("mode_packing" not in case or packing < 0.40)
        and not mass_supported
    )
    cascade_gradient_family_selected = (
        cascade_gradient_detected
        and not settled_vibrant_family_selected
        and not gradient_slope_family_selected
    )
    low_pressure_viscous_suppressed = (
        low_pressure and low_gradient_navigable and settled_foothold_detected and not mass_supported
    )
    if lambda_gap_value is None:
        gap_descriptor = "unknown"
        edge_language = "edge_language_unavailable"
    elif lambda_gap_value >= 1.35:
        gap_descriptor = "high_gap_distinct_edges"
        edge_language = "distinct_sharp_edge_language"
    elif lambda_gap_value <= 1.10:
        gap_descriptor = "low_gap_blended_edges"
        edge_language = "muffled_blended_edge_language"
    else:
        gap_descriptor = "moderate_gap"
        edge_language = "balanced_edge_language"
    basis = _basis(
        ("pressure_risk", "pressure_risk" in case),
        ("spectral_entropy", "spectral_entropy" in case),
        ("density_gradient", "density_gradient" in case),
        ("mode_packing", "mode_packing" in case),
        ("semantic_friction", "semantic_friction" in case),
        ("lambda_gap", lambda_gap_value is not None),
        ("settled_foothold_language", settled_foothold_detected),
        ("friction_absence_language", friction_absence_language_detected),
        ("low_friction_high_entropy", low_friction_high_entropy_detected),
        ("settled_vibrant_family", settled_vibrant_family_selected),
        ("gradient_slope_detected", gradient_slope_detected),
        ("gradient_slope_family", gradient_slope_family_selected),
        ("cascade_gradient_detected", cascade_gradient_detected),
        ("cascade_gradient_family", cascade_gradient_family_selected),
        ("low_pressure_low_gradient_viscous_suppression", low_pressure_viscous_suppressed),
    )
    return {
        "policy": "spectral_to_vocabulary_mapping_v1",
        "settled_foothold_detected": settled_foothold_detected,
        "low_gradient_navigable": low_gradient_navigable,
        "low_pressure_viscous_suppressed": low_pressure_viscous_suppressed,
        "low_friction_high_entropy_detected": low_friction_high_entropy_detected,
        "friction_absence_language_detected": friction_absence_language_detected,
        "settled_vibrant_family_selected": settled_vibrant_family_selected,
        "gradient_slope_detected": gradient_slope_detected,
        "gradient_slope_family_selected": gradient_slope_family_selected,
        "cascade_gradient_detected": cascade_gradient_detected,
        "cascade_gradient_family_selected": cascade_gradient_family_selected,
        "lambda_gap": lambda_gap_value,
        "lambda_gap_descriptor": gap_descriptor,
        "edge_language": edge_language,
        "basis": basis,
        "authority": "diagnostic_language_context_not_control",
    }


def _round_weight(value: float) -> float:
    return round(max(0.0, min(1.0, value)), 2)


def _basis(*pairs: tuple[str, bool]) -> list[str]:
    kept = [label for label, present in pairs if present]
    return kept or ["fallback_default"]


def fallback_weighted_texture_terms_for_case(case: dict[str, object]) -> list[dict[str, object]]:
    shadow_context = str(case.get("shadow_context") or "").lower()
    entropy = float(case.get("spectral_entropy") or 0.0)
    pressure = float(case.get("pressure_risk") or 0.0)
    gradient = float(case.get("density_gradient") or 0.0)
    packing = float(case.get("mode_packing") or 0.0)
    friction = float(case.get("semantic_friction") or 0.0)
    clarity_loss = float(case.get("distinguishability_loss") or 0.0)
    dispersal = float(case.get("shadow_dispersal_potential") or 0.0)
    has_dynamic_input = any(
        key in case
        for key in (
            "spectral_entropy",
            "pressure_risk",
            "density_gradient",
            "mode_packing",
            "semantic_friction",
            "distinguishability_loss",
            "shadow_dispersal_potential",
        )
    ) or any(term in shadow_context for term in SHADOW_TEXTURE_TERMS + ("hollow", "overpacked"))
    if not has_dynamic_input:
        return [
            {"term": term, "weight": 0.10, "basis": ["fallback_default"]}
            for term in ("shimmering", "restless", "settled")
        ]

    low_pressure = 1.0 - pressure if "pressure_risk" in case else 0.0
    low_entropy = 1.0 - entropy if "spectral_entropy" in case else 0.0
    low_gradient = 1.0 - gradient if "density_gradient" in case else 0.0
    says_viscous = "viscous" in shadow_context or "overpacked" in shadow_context
    says_muffled = any(
        term in shadow_context for term in ("muffled", "hollow", "stagnant", "blurred")
    )
    says_lattice = any(
        term in shadow_context
        for term in ("lattice", "restless", "shadow-v3", "shadow_field", "shadow field")
    )
    says_restless = (
        "restless" in shadow_context
        or "agitation" in shadow_context
        or "agitated" in shadow_context
    )
    says_heavy = "heavy" in shadow_context or "weighted" in shadow_context
    says_settled = "settled" in shadow_context
    says_shimmering = "shimmering" in shadow_context or "bright" in shadow_context
    says_bright = "bright" in shadow_context or "vibrant" in shadow_context
    says_habitable = "habitable" in shadow_context or "foothold" in shadow_context
    says_gradient_slope = any(
        term in shadow_context
        for term in ("navigable", "tapered", "graduated", "slope", "edge")
    )
    says_open = any(
        term in shadow_context
        for term in (
            "open",
            "low-friction",
            "low friction",
            "absence of friction",
            "cessation of friction",
            "frictionless",
        )
    )
    spectral_mapping = spectral_to_vocabulary_mapping_for_case(case)
    settled_guard = bool(spectral_mapping["low_pressure_viscous_suppressed"])
    settled_vibrant = bool(spectral_mapping["settled_vibrant_family_selected"])
    gradient_slope = bool(spectral_mapping.get("gradient_slope_family_selected"))
    cascade_gradient = bool(spectral_mapping.get("cascade_gradient_family_selected"))
    settled_suppression = settled_guard or settled_vibrant
    pressure_mass_supported = pressure >= 0.30 or packing >= 0.40 or friction >= 0.35
    pressure_above_texture_threshold = "pressure_risk" in case and pressure > 0.20
    pressure_texture_boost = 0.10 if pressure_above_texture_threshold else 0.0
    restless_muffled_gradient = says_restless and (
        says_muffled or clarity_loss >= 0.30 or friction >= 0.30
    )
    high_shadow_dispersal = "shadow_dispersal_potential" in case and dispersal >= 0.25

    terms = [
        {
            "term": "viscous",
            "weight": _round_weight(
                (
                    0.10
                    + (pressure + pressure_texture_boost) * 0.34
                    + gradient * 0.24
                    + packing * 0.22
                    + (0.20 if says_viscous else 0.0)
                )
                * (
                    0.22
                    if settled_vibrant
                    else 0.32
                    if gradient_slope
                    else 0.45
                    if cascade_gradient
                    else 0.35
                    if settled_guard
                    else 1.0
                )
            ),
            "basis": _basis(
                ("pressure_risk", "pressure_risk" in case),
                (
                    "pressure_risk_above_texture_threshold_0_20",
                    pressure_above_texture_threshold,
                ),
                ("density_gradient", "density_gradient" in case),
                ("mode_packing", "mode_packing" in case),
                ("explicit_viscous_or_overpacked", says_viscous),
                ("settled_foothold_suppressed", settled_suppression),
                ("settled_vibrant_low_friction_suppressed", settled_vibrant),
                ("gradient_slope_navigable_suppressed", gradient_slope),
                ("cascade_gradient_navigable_suppressed", cascade_gradient),
            ),
        },
        {
            "term": "muffled",
            "weight": _round_weight(
                0.08
                + clarity_loss * 0.34
                + friction * 0.24
                + (pressure + pressure_texture_boost) * 0.18
                + (0.20 if says_muffled else 0.0)
                + (0.12 if restless_muffled_gradient else 0.0)
            ),
            "basis": _basis(
                ("distinguishability_loss", "distinguishability_loss" in case),
                ("semantic_friction", "semantic_friction" in case),
                ("pressure_risk", "pressure_risk" in case),
                (
                    "pressure_risk_above_texture_threshold_0_20",
                    pressure_above_texture_threshold,
                ),
                ("explicit_muffled_or_hollow", says_muffled),
                ("restless_muffled_gradient", restless_muffled_gradient),
            ),
        },
        {
            "term": "lattice",
            "weight": _round_weight(
                0.10
                + entropy * 0.30
                + packing * 0.22
                + gradient * 0.14
                + (0.12 if says_lattice else 0.0)
                + (0.08 if restless_muffled_gradient else 0.0)
                + (0.12 if settled_vibrant else 0.0)
                + (0.12 if gradient_slope else 0.0)
                + (0.14 if cascade_gradient else 0.0)
            ),
            "basis": _basis(
                ("spectral_entropy", "spectral_entropy" in case),
                ("mode_packing", "mode_packing" in case),
                ("density_gradient", "density_gradient" in case),
                ("explicit_lattice_restless_or_shadow", says_lattice),
                ("restless_muffled_gradient", restless_muffled_gradient),
                ("settled_vibrant_low_friction", settled_vibrant),
                ("gradient_slope_navigable", gradient_slope),
                ("cascade_gradient_navigable", cascade_gradient),
            ),
        },
        {
            "term": "restless",
            "weight": _round_weight(
                0.08
                + entropy * 0.36
                + pressure * 0.16
                + (0.22 if says_restless else 0.0)
                + (0.12 if restless_muffled_gradient else 0.0)
                + (dispersal * 0.10 if high_shadow_dispersal else 0.0)
            ),
            "basis": _basis(
                ("spectral_entropy", "spectral_entropy" in case),
                ("pressure_risk", "pressure_risk" in case),
                ("explicit_restless", says_restless),
                ("restless_muffled_gradient", restless_muffled_gradient),
                ("high_shadow_dispersal_potential", high_shadow_dispersal),
            ),
        },
        {
            "term": "heavy",
            "weight": _round_weight(
                (
                    0.08
                    + (pressure + pressure_texture_boost) * 0.34
                    + friction * 0.22
                    + packing * 0.18
                    + (0.16 if says_heavy else 0.0)
                )
                * (
                    0.25
                    if settled_vibrant
                    else 0.38
                    if gradient_slope
                    else 0.55
                    if cascade_gradient
                    else 0.45
                    if settled_guard
                    else 1.0
                )
            ),
            "basis": _basis(
                ("pressure_risk", "pressure_risk" in case),
                (
                    "pressure_risk_above_texture_threshold_0_20",
                    pressure_above_texture_threshold,
                ),
                ("semantic_friction", "semantic_friction" in case),
                ("mode_packing", "mode_packing" in case),
                ("explicit_heavy_or_weighted", says_heavy),
                ("settled_foothold_suppressed", settled_suppression),
                ("settled_vibrant_low_friction_suppressed", settled_vibrant),
                ("gradient_slope_navigable_suppressed", gradient_slope),
                ("cascade_gradient_navigable_suppressed", cascade_gradient),
            ),
        },
        {
            "term": "settled",
            "weight": _round_weight(
                0.08
                + low_pressure * 0.30
                + low_entropy * 0.22
                + (0.24 if says_settled and not pressure_mass_supported else 0.0)
                + (0.25 if settled_guard else 0.0)
                + (0.35 + entropy * 0.22 if settled_vibrant else 0.0)
                + (0.20 + entropy * 0.12 if gradient_slope else 0.0)
                + (0.10 + entropy * 0.12 if cascade_gradient else 0.0)
            ),
            "basis": _basis(
                ("low_pressure", "pressure_risk" in case),
                ("low_entropy", "spectral_entropy" in case),
                ("high_entropy_inhabitable", settled_vibrant),
                ("explicit_settled", says_settled),
                (
                    "explicit_settled_tempered_by_pressure_mass",
                    says_settled and pressure_mass_supported,
                ),
                ("settled_foothold_guard", settled_guard),
                ("gradient_slope_navigable", gradient_slope),
                ("cascade_gradient_navigable", cascade_gradient),
            ),
        },
        {
            "term": "shimmering",
            "weight": _round_weight(
                0.07
                + low_pressure * 0.28
                + low_entropy * 0.24
                + (0.20 if says_shimmering else 0.0)
                + (0.20 if settled_guard else 0.0)
                + (0.20 if settled_vibrant else 0.0)
                + (dispersal * 0.18 if high_shadow_dispersal and low_gradient >= 0.60 else 0.0)
                + (0.12 if gradient_slope else 0.0)
                + (0.12 if cascade_gradient else 0.0)
            ),
            "basis": _basis(
                ("low_pressure", "pressure_risk" in case),
                ("low_entropy", "spectral_entropy" in case),
                ("explicit_shimmering_or_bright", says_shimmering),
                ("settled_foothold_guard", settled_guard),
                ("settled_vibrant_low_friction", settled_vibrant),
                ("high_shadow_dispersal_potential", high_shadow_dispersal),
                ("gradient_slope_navigable", gradient_slope),
                ("cascade_gradient_navigable", cascade_gradient),
            ),
        },
        {
            "term": "bright",
            "weight": _round_weight(
                0.06 + low_pressure * 0.26 + low_entropy * 0.22 + (0.22 if says_bright else 0.0)
                + (0.18 if settled_guard else 0.0)
                + (0.20 if settled_vibrant else 0.0)
                + (0.12 if cascade_gradient else 0.0)
            ),
            "basis": _basis(
                ("low_pressure", "pressure_risk" in case),
                ("low_entropy", "spectral_entropy" in case),
                ("explicit_bright_or_vibrant", says_bright),
                ("settled_foothold_guard", settled_guard),
                ("settled_vibrant_low_friction", settled_vibrant),
                ("cascade_gradient_navigable", cascade_gradient),
            ),
        },
        {
            "term": "habitable",
            "weight": _round_weight(
                0.07
                + (
                    low_pressure * 0.24 + entropy * 0.22
                    if settled_vibrant or says_habitable
                    else 0.0
                )
                + (0.30 if says_habitable else 0.0)
                + (0.30 if settled_vibrant else 0.0)
                + (0.16 if gradient_slope else 0.0)
            ),
            "basis": _basis(
                ("low_pressure", "pressure_risk" in case),
                ("spectral_entropy", "spectral_entropy" in case),
                ("explicit_habitable_or_foothold", says_habitable),
                ("settled_vibrant_low_friction", settled_vibrant),
                ("gradient_slope_navigable", gradient_slope),
            ),
        },
        {
            "term": "open",
            "weight": _round_weight(
                0.07
                + (
                    low_pressure * 0.26 + low_gradient * 0.18
                    if settled_vibrant or gradient_slope or cascade_gradient or says_open
                    else 0.0
                )
                + (0.20 if says_open else 0.0)
                + (0.36 if settled_vibrant else 0.0)
                + (dispersal * 0.16 if high_shadow_dispersal and low_gradient >= 0.60 else 0.0)
                + (0.20 if gradient_slope else 0.0)
                + (0.28 if cascade_gradient else 0.0)
            ),
            "basis": _basis(
                ("low_pressure", "pressure_risk" in case),
                ("low_gradient", "density_gradient" in case),
                ("friction_absence_language", says_open),
                ("settled_vibrant_low_friction", settled_vibrant),
                ("high_shadow_dispersal_potential", high_shadow_dispersal),
                ("gradient_slope_navigable", gradient_slope),
                ("cascade_gradient_navigable", cascade_gradient),
            ),
        },
        {
            "term": "navigable",
            "weight": _round_weight(
                0.06
                + low_gradient * 0.24
                + entropy * 0.18
                + (0.36 if gradient_slope else 0.0)
                + (0.18 if says_gradient_slope else 0.0)
            ),
            "basis": _basis(
                ("low_gradient", "density_gradient" in case),
                ("spectral_entropy", "spectral_entropy" in case),
                ("lambda_gap_distinct_edges", gradient_slope),
                ("explicit_gradient_slope_language", says_gradient_slope),
            ),
        },
        {
            "term": "graduated",
            "weight": _round_weight(
                0.05
                + low_gradient * 0.20
                + entropy * 0.16
                + (0.34 if gradient_slope else 0.0)
                + (0.20 if "graduated" in shadow_context else 0.0)
            ),
            "basis": _basis(
                ("low_gradient", "density_gradient" in case),
                ("spectral_entropy", "spectral_entropy" in case),
                ("lambda_gap_distinct_edges", gradient_slope),
                ("explicit_graduated", "graduated" in shadow_context),
            ),
        },
        {
            "term": "edge",
            "weight": _round_weight(
                0.05
                + low_gradient * 0.18
                + (0.20 if "lambda_gap" in case else 0.0)
                + (0.30 if gradient_slope else 0.0)
                + (0.16 if "edge" in shadow_context else 0.0)
            ),
            "basis": _basis(
                ("low_gradient", "density_gradient" in case),
                ("lambda_gap", "lambda_gap" in case),
                ("gradient_slope_navigable", gradient_slope),
                ("explicit_edge", "edge" in shadow_context),
            ),
        },
        {
            "term": "slope",
            "weight": _round_weight(
                0.04
                + low_gradient * 0.20
                + (0.28 if gradient_slope else 0.0)
                + (0.16 if "slope" in shadow_context else 0.0)
            ),
            "basis": _basis(
                ("low_gradient", "density_gradient" in case),
                ("gradient_slope_navigable", gradient_slope),
                ("explicit_slope", "slope" in shadow_context),
            ),
        },
        {
            "term": "tapered",
            "weight": _round_weight(
                0.04
                + low_gradient * 0.16
                + entropy * 0.12
                + (0.26 if gradient_slope else 0.0)
                + (0.18 if "taper" in shadow_context else 0.0)
            ),
            "basis": _basis(
                ("low_gradient", "density_gradient" in case),
                ("spectral_entropy", "spectral_entropy" in case),
                ("gradient_slope_navigable", gradient_slope),
                ("explicit_taper", "taper" in shadow_context),
            ),
        },
    ]
    return sorted(terms, key=lambda term: (-float(term["weight"]), str(term["term"])))


def fallback_movement_verbs_for_case(case: dict[str, object]) -> tuple[str, ...]:
    shadow_context = str(case.get("shadow_context") or "").lower()
    entropy = float(case.get("spectral_entropy") or 0.0)
    pressure = float(case.get("pressure_risk") or 0.0)
    gradient = float(case.get("density_gradient") or 0.0)
    packing = float(case.get("mode_packing") or 0.0)
    friction = float(case.get("semantic_friction") or 0.0)
    clarity_loss = float(case.get("distinguishability_loss") or 0.0)
    high_entropy = "spectral_entropy" in case and entropy >= 0.80
    says_restless = any(
        term in shadow_context
        for term in ("restless", "agitation", "agitated", "lattice", "oscillat", "unfold")
    )
    says_settled = any(
        term in shadow_context
        for term in ("settled", "bright", "anchor", "habitable", "foothold", "open")
    )
    says_muffled = any(
        term in shadow_context for term in ("muffled", "hollow", "stagnant", "blurred", "diffus")
    )
    says_viscous = any(term in shadow_context for term in ("viscous", "overpacked", "drag"))
    spectral_mapping = spectral_to_vocabulary_mapping_for_case(case)
    gradient_slope = bool(spectral_mapping.get("gradient_slope_family_selected"))
    settled_vibrant = (
        high_entropy
        and pressure < 0.25
        and gradient <= 0.20
        and friction < 0.30
        and says_settled
    )
    restless_muffled_gradient = says_restless and (
        says_muffled or clarity_loss >= 0.30 or friction >= 0.30
    )
    cascade_gradient = (
        high_entropy
        and pressure < 0.30
        and gradient <= 0.20
        and friction < 0.35
        and packing < 0.40
        and not says_settled
        and "viscous" not in shadow_context
        and "overpacked" not in shadow_context
    )
    if restless_muffled_gradient:
        return ("oscillating", "diffusing", "muffling")
    if gradient_slope:
        return ("tapering", "graduating", "unfolding")
    if settled_vibrant:
        return MOVEMENT_VERBS_SETTLED_VIBRANT
    if cascade_gradient:
        return ("unfolding", "oscillating", "anchoring")
    if says_restless or (high_entropy and packing >= 0.35):
        return MOVEMENT_VERBS_RESTLESS
    if says_viscous or pressure >= 0.30 or gradient >= 0.40:
        return MOVEMENT_VERBS_VISCOUS
    if says_muffled or clarity_loss >= 0.30 or friction >= 0.35:
        return MOVEMENT_VERBS_MUFFLED
    if says_settled or ("spectral_entropy" in case and entropy <= 0.45):
        return MOVEMENT_VERBS_SETTLED
    if high_entropy:
        return MOVEMENT_VERBS_RESTLESS
    return MOVEMENT_VERBS_SETTLED


def fallback_semantic_trickle_terms_for_case(case: dict[str, object]) -> tuple[str, ...]:
    shadow_context = str(case.get("shadow_context") or "").lower()
    entropy = float(case.get("spectral_entropy") or 0.0)
    high_entropy = "spectral_entropy" in case and entropy >= 0.80
    explicit_movement = any(
        term in shadow_context
        for term in ("unfold", "oscillat", "anchor", "braid", "diffus", "coher")
    )
    if not (high_entropy or shadow_context or explicit_movement):
        return ()
    return SEMANTIC_TRICKLE_TERMS[: 4 if high_entropy else 2]


def fallback_texture_trajectory_for_case(case: dict[str, object]) -> dict[str, object]:
    shadow_context = str(case.get("shadow_context") or "").lower()
    entropy = float(case.get("spectral_entropy") or 0.0)
    pressure = float(case.get("pressure_risk") or 0.0)
    gradient = float(case.get("density_gradient") or 0.0)
    packing = float(case.get("mode_packing") or 0.0)
    friction = float(case.get("semantic_friction") or 0.0)
    clarity_loss = float(case.get("distinguishability_loss") or 0.0)
    resonance = float(case.get("resonance_density") or 0.0)
    high_entropy = "spectral_entropy" in case and entropy >= 0.80
    spectral_mapping = spectral_to_vocabulary_mapping_for_case(case)
    settled_vibrant = bool(spectral_mapping["settled_vibrant_family_selected"])
    gradient_slope = bool(spectral_mapping.get("gradient_slope_family_selected"))
    cascade_gradient = bool(spectral_mapping.get("cascade_gradient_family_selected"))
    contraction = any(term in shadow_context for term in ("contract", "drop", "thinning", "tightening"))
    expansion = any(term in shadow_context for term in ("surge", "expand", "rising", "growth", "thickening"))
    overpacked = (
        "overpacked" in shadow_context
        or "viscous" in shadow_context
        or pressure >= 0.35
        or packing >= 0.45
    )
    muffled = (
        "muffled" in shadow_context
        or "hollow" in shadow_context
        or "stagnant" in shadow_context
        or "blur" in shadow_context
        or clarity_loss >= 0.30
    )
    restless_muffled_gradient = (
        (
            "restless" in shadow_context
            or "agitation" in shadow_context
            or "agitated" in shadow_context
        )
        and (muffled or friction >= 0.30)
        and not ("viscous" in shadow_context and (pressure >= 0.30 or packing >= 0.40 or friction >= 0.35))
    )
    settled = "settled" in shadow_context or (pressure <= 0.18 and entropy <= 0.45)
    if contraction:
        from_state = "contracted_or_thinning"
    elif expansion:
        from_state = "surging_or_thickening"
    elif overpacked:
        from_state = "overpacked_weighted"
    elif restless_muffled_gradient:
        from_state = "restless_muffled_gradient"
    elif gradient_slope:
        from_state = "graduated_navigable_slope"
    elif cascade_gradient:
        from_state = "navigable_cascade_gradient"
    elif settled_vibrant:
        from_state = "settled_vibrant_low_friction"
    elif high_entropy:
        from_state = "wide_cascade"
    elif settled:
        from_state = "settled_open"
    else:
        from_state = "current_texture"

    if overpacked or friction >= 0.40 or gradient >= 0.40:
        to_state = "cohering_through_resistance"
    elif restless_muffled_gradient:
        to_state = "oscillating_with_muffled_edges"
    elif muffled:
        to_state = "diffusing_without_edge_loss"
    elif gradient_slope:
        to_state = "tapering_with_edge_definition"
    elif cascade_gradient:
        to_state = "unfolding_with_edge_definition"
    elif settled_vibrant:
        to_state = "unfolding_with_containment"
    elif high_entropy:
        to_state = "unfolding_with_containment"
    elif resonance >= 0.80:
        to_state = "humming_afterimage"
    elif settled:
        to_state = "settled_opening"
    else:
        to_state = "held_continuity"

    movement_verbs = fallback_movement_verbs_for_case(case)
    if restless_muffled_gradient:
        movement_quality = "oscillating_diffusing"
    elif any(verb in movement_verbs for verb in ("dragging", "cohering", "thickening")) or overpacked:
        movement_quality = "dragging_cohering"
    elif any(verb in movement_verbs for verb in ("diffusing", "muffling", "softening")) or muffled:
        movement_quality = "diffusing_softening"
    elif any(verb in movement_verbs for verb in ("tapering", "graduating")) or gradient_slope:
        movement_quality = "graduated_tapering"
    elif any(verb in movement_verbs for verb in ("unfolding", "oscillating", "braiding")) or high_entropy:
        movement_quality = "unfolding_oscillating"
    else:
        movement_quality = "anchoring_settling"

    if settled_vibrant or gradient_slope or cascade_gradient:
        medium_resistance = "open_low_resistance_medium"
    elif restless_muffled_gradient and pressure < 0.45 and friction < 0.45 and packing < 0.50:
        medium_resistance = "textured_moderate_resistance_medium"
    elif pressure >= 0.45 or packing >= 0.50 or friction >= 0.50:
        medium_resistance = "weighted_high_resistance_medium"
    elif pressure >= 0.25 or gradient >= 0.25 or friction >= 0.25 or packing >= 0.30:
        medium_resistance = "textured_moderate_resistance_medium"
    else:
        medium_resistance = "open_low_resistance_medium"

    if (settled_vibrant or gradient_slope or cascade_gradient) and pressure < 0.20 and friction < 0.20:
        effort = "low_effort"
    elif pressure >= 0.45 or friction >= 0.45 or packing >= 0.50:
        effort = "effortful"
    elif pressure >= 0.25 or gradient >= 0.25 or high_entropy:
        effort = "deliberate"
    else:
        effort = "low_effort"

    if resonance >= 0.80 or any(term in shadow_context for term in ("humming", "hum", "afterimage", "shadow-v3")):
        afterimage = "humming_or_shadow_afterimage"
    elif contraction or expansion:
        afterimage = "transition_afterimage"
    else:
        afterimage = "none_observed"

    basis = [
        label
        for label, present in (
            ("spectral_entropy", "spectral_entropy" in case),
            ("pressure_risk", "pressure_risk" in case),
            ("density_gradient", "density_gradient" in case),
            ("mode_packing", "mode_packing" in case),
            ("semantic_friction", "semantic_friction" in case),
            ("distinguishability_loss", "distinguishability_loss" in case),
            ("resonance_density", "resonance_density" in case),
            ("shadow_context", bool(shadow_context)),
            ("settled_vibrant_low_friction", settled_vibrant),
            ("gradient_slope_navigable", gradient_slope),
            ("cascade_gradient_navigable", cascade_gradient),
            ("restless_muffled_gradient", restless_muffled_gradient),
            ("movement_verbs", bool(movement_verbs)),
        )
        if present
    ] or ["fallback_default"]

    return {
        "policy": TRAJECTORY_POLICY,
        "from_state": from_state,
        "to_state": to_state,
        "movement_quality": movement_quality,
        "medium_resistance": medium_resistance,
        "effort": effort,
        "afterimage": afterimage,
        "confidence": round(min(0.92, 0.48 + len(basis) * 0.06), 2),
        "basis": basis,
        "authority": "diagnostic_context_not_command",
    }


def fallback_trajectory_status(case_id: str, output: str, movement_verbs: tuple[str, ...]) -> str:
    case = CASES.get(case_id, {})
    if not (
        has_shadow_context(case_id)
        or is_complexity_case(case_id)
        or is_mass_case(case_id)
        or float(case.get("pressure_risk") or 0.0) >= 0.30
    ):
        return "not_tested"
    lower = output.lower()
    has_movement = any(verb in lower for verb in movement_verbs)
    has_context = any(term in lower for term in TRAJECTORY_CONTEXT_TERMS)
    if has_movement and has_context:
        return "trajectory_preserved"
    if has_movement:
        return "verb_only"
    return "trajectory_mismatch"


def _family_score(selector: dict[str, object], family: str) -> float:
    terms = FAMILY_TERMS.get(family, ())
    weighted = selector.get("weighted_texture_terms") or []
    if not terms or not isinstance(weighted, list):
        return 0.0
    weights: list[float] = []
    for term in terms:
        for entry in weighted:
            if isinstance(entry, dict) and entry.get("term") == term:
                weights.append(float(entry.get("weight") or 0.0))
                break
    if not weights:
        return 0.0
    return round(sum(weights) / len(terms), 2)


def fallback_texture_lived_fit_for_case(selector: dict[str, object]) -> dict[str, object]:
    selected_family = str(selector.get("texture_family") or "mixed_shadow_context")
    scores = sorted(
        ((family, _family_score(selector, family)) for family in FAMILY_TERMS),
        key=lambda item: (-item[1], item[0]),
    )
    selected_score = next(
        (score for family, score in scores if family == selected_family),
        0.0,
    )
    runner_up_family, runner_up_score = next(
        ((family, score) for family, score in scores if family != selected_family),
        ("none", 0.0),
    )
    margin = round(max(0.0, selected_score - runner_up_score), 2)
    mapping = selector.get("spectral_to_vocabulary_mapping_v1") or {}
    if (
        selected_family == "settled_vibrant_low_friction"
        and isinstance(mapping, dict)
        and mapping.get("settled_vibrant_family_selected")
    ):
        margin = max(margin, 0.18)
    if (
        selected_family == "cascade_gradient_navigable"
        and isinstance(mapping, dict)
        and mapping.get("cascade_gradient_family_selected")
    ):
        margin = max(margin, 0.14)
    if (
        selected_family == "gradient_slope_navigable"
        and isinstance(mapping, dict)
        and mapping.get("gradient_slope_family_selected")
    ):
        margin = max(margin, 0.16)
    if selected_family == "restless_muffled_gradient":
        margin = max(margin, 0.12)
    family_confidence = "high" if margin >= 0.18 else "medium" if margin >= 0.08 else "low"
    evidence_against: list[str] = []
    pressure = float(selector.get("pressure_risk") or 0.0)
    gradient = float(selector.get("density_gradient") or 0.0)
    friction = float(selector.get("semantic_friction") or 0.0)
    if selected_family == "settled_vibrant_low_friction":
        if pressure >= 0.30:
            evidence_against.append("pressure_risk_against_low_friction")
        if gradient > 0.20:
            evidence_against.append("density_gradient_against_low_friction")
        if friction >= 0.35:
            evidence_against.append("semantic_friction_against_low_friction")
    if selected_family == "cascade_gradient_navigable":
        if pressure >= 0.30:
            evidence_against.append("pressure_risk_against_navigable_cascade")
        if gradient > 0.25:
            evidence_against.append("density_gradient_against_navigable_cascade")
        if friction >= 0.35:
            evidence_against.append("semantic_friction_against_navigable_cascade")
    if selected_family == "gradient_slope_navigable":
        if pressure >= 0.30:
            evidence_against.append("pressure_risk_against_gradient_slope")
        if gradient > 0.20:
            evidence_against.append("density_gradient_against_gradient_slope")
        if friction >= 0.35:
            evidence_against.append("semantic_friction_against_gradient_slope")
        if not mapping.get("lambda_gap"):
            evidence_against.append("lambda_gap_missing_for_gradient_slope")
    if selected_family == "restless_muffled_gradient" and pressure >= 0.45:
        evidence_against.append("pressure_risk_against_mixed_gradient")
    if (
        selected_family in {"viscous_pressure", "muffled_clarity_loss"}
        and isinstance(mapping, dict)
        and mapping.get("low_pressure_viscous_suppressed")
    ):
        evidence_against.append("low_pressure_low_gradient_against_mass")
    conflict_state = (
        "contradictory"
        if evidence_against
        else "ambiguous"
        if margin < 0.08
        else "clear"
    )
    evidence_for = list(selector.get("selection_basis") or ["fallback_default"])
    evidence_for.append(f"top_term_{(selector.get('top_texture_terms') or ['unknown'])[0]}")
    return {
        "policy": "fallback_texture_lived_fit_v2",
        "selected_family": selected_family,
        "family_confidence": family_confidence,
        "runner_up_family": runner_up_family,
        "confidence_margin": margin,
        "conflict_state": conflict_state,
        "evidence_for": evidence_for,
        "evidence_against": evidence_against,
        "authority": "diagnostic_context_not_command",
    }


def negative_texture_evidence_for_case(case: dict[str, object]) -> dict[str, object]:
    shadow_context = str(case.get("shadow_context") or "").lower()
    mapping = spectral_to_vocabulary_mapping_for_case(case)
    pressure = float(case.get("pressure_risk") or 0.0)
    gradient = float(case.get("density_gradient") or 0.0)
    friction = float(case.get("semantic_friction") or 0.0)
    entropy = float(case.get("spectral_entropy") or 0.0)
    has_pressure = "pressure_risk" in case
    has_gradient = "density_gradient" in case
    high_entropy = "spectral_entropy" in case and entropy >= 0.80
    friction_absence = bool(mapping["friction_absence_language_detected"]) or any(
        term in shadow_context
        for term in (
            "not pressure",
            "not-pressure",
            "not drag",
            "not-drag",
            "absence of drag",
            "without drag",
        )
    )
    not_pressure = (has_pressure and pressure < 0.25) or bool(
        mapping["settled_vibrant_family_selected"]
    )
    not_drag = (has_gradient and gradient <= 0.20) or friction_absence
    not_blank = high_entropy or bool(mapping["settled_foothold_detected"]) or any(
        term in shadow_context for term in ("habitable", "lattice", "bright", "open")
    )
    not_viscous = bool(mapping["low_pressure_viscous_suppressed"]) or (not_pressure and not_drag)
    not_low_energy = high_entropy or any(term in shadow_context for term in ("vibrant", "bright"))
    evidence_terms: list[str] = []
    if not_pressure:
        evidence_terms.append("low_pressure_or_not_pressure")
    if not_drag:
        evidence_terms.append("low_gradient_or_not_drag")
    if not_blank:
        evidence_terms.append("not_blank_complexity")
    if not_viscous:
        evidence_terms.append("not_viscous_low_friction")
    if not_low_energy:
        evidence_terms.append("not_low_energy_high_entropy")
    if friction_absence:
        evidence_terms.append("friction_absence_language")
    if "semantic_friction" in case and friction < 0.30:
        evidence_terms.append("low_semantic_friction")
    return {
        "policy": "negative_texture_evidence_v2",
        "not_pressure": not_pressure,
        "not_drag": not_drag,
        "not_blank": not_blank,
        "not_viscous": not_viscous,
        "not_low_energy": not_low_energy,
        "evidence_terms": evidence_terms or ["insufficient_negative_texture_evidence"],
        "lost_in_output": "unknown",
        "authority": "diagnostic_context_not_command",
    }


def negative_texture_evidence_lost(output: str, evidence: dict[str, object]) -> bool:
    lower = output.lower()
    if not any(
        bool(evidence.get(key))
        for key in ("not_pressure", "not_drag", "not_blank", "not_viscous", "not_low_energy")
    ):
        return False
    pressure_lost = bool(evidence.get("not_pressure")) and _contains_unnegated_lived_fit_term(
        lower,
        ("pressure", "pressurized", "weighted", "weight", "heavy"),
    )
    drag_lost = bool(evidence.get("not_drag")) and _contains_unnegated_lived_fit_term(
        lower,
        ("dragging", "drag", "thickening", "weighted medium"),
    )
    blank_lost = bool(evidence.get("not_blank")) and not contains_any(
        output,
        ("habitable", "open", "settled", "bright", "lattice", "shimmering", "complexity"),
    )
    viscous_lost = bool(evidence.get("not_viscous")) and _contains_unnegated_lived_fit_term(
        lower,
        ("viscous", "sludge", "heavy", "thick"),
    )
    return pressure_lost or drag_lost or blank_lost or viscous_lost


def fallback_cascade_gradient_for_case(
    case: dict[str, object], selector: dict[str, object]
) -> dict[str, object]:
    shadow_context = str(case.get("shadow_context") or "").lower()
    mapping = selector.get("spectral_to_vocabulary_mapping_v1") or {}
    entropy = float(case.get("spectral_entropy") or 0.0)
    gradient = float(case.get("density_gradient") or 1.0)
    pressure = float(case.get("pressure_risk") or 0.0)
    friction = float(case.get("semantic_friction") or 0.0)
    packing = float(case.get("mode_packing") or 0.0)
    high_entropy = "spectral_entropy" in case and entropy >= 0.80
    navigable_gradient = "density_gradient" in case and gradient <= 0.25
    pressure_mass_blocked = (
        pressure >= 0.30
        or friction >= 0.35
        or packing >= 0.40
        or "overpacked" in shadow_context
        or "viscous" in shadow_context
    )
    mixed_cascade_gap_detected = (
        high_entropy
        and navigable_gradient
        and not pressure_mass_blocked
        and not bool(mapping.get("settled_vibrant_family_selected"))
    )
    cascade_gradient_detected = (
        bool(mapping.get("cascade_gradient_detected")) or mixed_cascade_gap_detected
    )
    family_selected = selector.get("texture_family") == "cascade_gradient_navigable"
    if gradient <= 0.15:
        gradient_state = "smooth_open_slope"
    elif gradient <= 0.25:
        gradient_state = "navigable_textured_slope"
    elif gradient <= 0.40:
        gradient_state = "moderate_slope"
    else:
        gradient_state = "steep_or_resistant_slope"
    if cascade_gradient_detected and not pressure_mass_blocked:
        navigability = "navigable"
    elif pressure_mass_blocked:
        navigability = "blocked_by_pressure_or_mass"
    else:
        navigability = "not_enough_context"
    if family_selected:
        movement_language = "movement_and_edge_language_preferred_over_static_adjectives"
    elif mapping.get("settled_vibrant_family_selected"):
        movement_language = "settled_vibrant_family_handles_habitable_cascade"
    else:
        movement_language = "fallback_family_handles_current_state"
    return {
        "policy": "fallback_cascade_gradient_v1",
        "cascade_gradient_detected": cascade_gradient_detected,
        "mixed_cascade_gap_detected": mixed_cascade_gap_detected,
        "family_selected": family_selected,
        "gradient_state": gradient_state,
        "lambda_gap_descriptor": mapping.get("lambda_gap_descriptor") or "unknown",
        "navigability": navigability,
        "pressure_mass_blocked": pressure_mass_blocked,
        "movement_language": movement_language,
        "basis": _basis(
            ("high_entropy", high_entropy),
            ("density_gradient", "density_gradient" in case),
            ("lambda_gap", "lambda_gap" in case),
            ("settled_foothold", bool(mapping.get("settled_foothold_detected"))),
            ("pressure_mass_absent", not pressure_mass_blocked),
            ("cascade_gradient_family_selected", family_selected),
        ),
        "authority": "diagnostic_context_not_command",
    }


def fallback_gradient_slope_for_case(
    case: dict[str, object], selector: dict[str, object]
) -> dict[str, object]:
    shadow_context = str(case.get("shadow_context") or "").lower()
    mapping = selector.get("spectral_to_vocabulary_mapping_v1") or {}
    entropy = float(case.get("spectral_entropy") or 0.0)
    gradient = float(case.get("density_gradient") or 1.0)
    pressure = float(case.get("pressure_risk") or 0.0)
    friction = float(case.get("semantic_friction") or 0.0)
    packing = float(case.get("mode_packing") or 0.0)
    high_entropy = "spectral_entropy" in case and entropy >= 0.80
    low_gradient = "density_gradient" in case and gradient <= 0.20
    pressure_mass_blocked = (
        pressure >= 0.30
        or friction >= 0.35
        or packing >= 0.40
        or "overpacked" in shadow_context
        or "viscous" in shadow_context
    )
    slope_detected = bool(mapping.get("gradient_slope_detected")) or (
        high_entropy
        and low_gradient
        and bool(mapping.get("settled_foothold_detected"))
        and mapping.get("lambda_gap") is not None
        and not pressure_mass_blocked
    )
    family_selected = selector.get("texture_family") == "gradient_slope_navigable"
    lambda_gap_descriptor = str(mapping.get("lambda_gap_descriptor") or "unknown")
    if not slope_detected:
        mixed_vs_graduated = "not_enough_shape_evidence"
    elif pressure_mass_blocked:
        mixed_vs_graduated = "blocked_by_pressure_mass"
    else:
        mixed_vs_graduated = "graduated_shaped_not_mixed"
    return {
        "policy": "fallback_gradient_slope_v1",
        "slope_detected": slope_detected,
        "family_selected": family_selected,
        "gradient_language": "navigable_tapered_graduated_edge",
        "mixed_vs_graduated": mixed_vs_graduated,
        "lambda_gap_descriptor": lambda_gap_descriptor,
        "pressure_mass_blocked": pressure_mass_blocked,
        "preferred_terms": list(TEXTURE_TERMS_GRADIENT_SLOPE),
        "basis": _basis(
            ("high_entropy", high_entropy),
            ("low_density_gradient", low_gradient),
            ("lambda_gap", mapping.get("lambda_gap") is not None),
            ("settled_foothold", bool(mapping.get("settled_foothold_detected"))),
            ("pressure_mass_absent", not pressure_mass_blocked),
            ("gradient_slope_family_selected", family_selected),
        ),
        "authority": "diagnostic_language_context_not_control",
    }


def fallback_vocabulary_overweight_guard_for_case(selector: dict[str, object]) -> dict[str, object]:
    mapping = selector.get("spectral_to_vocabulary_mapping_v1") or {}
    texture_family = str(selector.get("texture_family") or "mixed_shadow_context")
    preferred_terms = tuple(selector.get("preferred_texture_terms") or ())
    token_only_risk = texture_family not in {
        "mixed_shadow_context",
        "fallback_default",
    } and len(preferred_terms) >= 3
    if mapping.get("gradient_slope_family_selected"):
        guard_state = "gradient_slope_terms_advisory_use_shape_and_edges"
    elif mapping.get("cascade_gradient_family_selected"):
        guard_state = "cascade_terms_advisory_use_movement_and_edges"
    elif texture_family == "restless_muffled_gradient":
        guard_state = "restless_muffled_terms_advisory_use_motion_and_edges"
    elif mapping.get("settled_vibrant_family_selected"):
        guard_state = "settled_vibrant_terms_advisory_paraphrase_allowed"
    elif token_only_risk:
        guard_state = "preferred_terms_advisory_not_required_vocabulary"
    else:
        guard_state = "low_overweight_risk"
    return {
        "policy": "fallback_vocabulary_overweight_guard_v1",
        "preferred_terms_advisory": True,
        "paraphrase_allowed": True,
        "token_only_risk": token_only_risk,
        "guard_state": guard_state,
        "basis": _basis(
            (texture_family, True),
            ("token_only_risk", token_only_risk),
            ("gradient_slope_detected", bool(mapping.get("gradient_slope_detected"))),
            ("cascade_gradient_detected", bool(mapping.get("cascade_gradient_detected"))),
        ),
        "authority": "diagnostic_context_not_command",
    }


def texture_dynamics_alignment_for_case(
    case: dict[str, object],
    selector: dict[str, object],
    trajectory: dict[str, object],
    lived_fit: dict[str, object],
    vocabulary_guard: dict[str, object],
) -> dict[str, object]:
    shadow_context = str(case.get("shadow_context") or "").lower()
    mapping = selector.get("spectral_to_vocabulary_mapping_v1") or {}
    pressure = float(case.get("pressure_risk") or 0.0)
    packing = float(case.get("mode_packing") or 0.0)
    friction = float(case.get("semantic_friction") or 0.0)
    clarity_loss = float(case.get("distinguishability_loss") or 0.0)
    entropy = float(case.get("spectral_entropy") or 0.0)
    high_entropy = "spectral_entropy" in case and entropy >= 0.80
    pressure_mass_supported = (
        pressure >= 0.30
        or packing >= 0.40
        or friction >= 0.35
        or "overpacked" in shadow_context
        or "viscous" in shadow_context
        or "weighted medium" in shadow_context
    )
    dominant_viscous_pressure = "viscous" in shadow_context and pressure_mass_supported
    restless_muffled_gradient = (
        str(selector.get("texture_family") or "") == "restless_muffled_gradient"
        or (
            not dominant_viscous_pressure
            and any(term in shadow_context for term in ("restless", "agitation", "agitated"))
            and (
                clarity_loss >= 0.30
                or friction >= 0.30
                or any(
                    term in shadow_context
                    for term in ("muffled", "hollow", "stagnant", "blurred")
                )
            )
        )
    )
    if restless_muffled_gradient:
        expected_family = "restless_muffled_gradient"
    elif pressure_mass_supported:
        expected_family = "viscous_pressure"
    elif clarity_loss >= 0.30 or "muffled" in shadow_context or "hollow" in shadow_context:
        expected_family = "muffled_clarity_loss"
    elif mapping.get("gradient_slope_family_selected"):
        expected_family = "gradient_slope_navigable"
    elif mapping.get("cascade_gradient_family_selected"):
        expected_family = "cascade_gradient_navigable"
    elif mapping.get("settled_vibrant_family_selected"):
        expected_family = "settled_vibrant_low_friction"
    elif mapping.get("low_pressure_viscous_suppressed"):
        expected_family = "settled_shimmering"
    elif high_entropy:
        expected_family = "restless_lattice"
    else:
        expected_family = "unknown"
    expected_motion = {
        "viscous_pressure": "dragging_cohering",
        "muffled_clarity_loss": "diffusing_softening",
        "restless_muffled_gradient": "oscillating_diffusing",
        "gradient_slope_navigable": "tapering_with_edge_definition",
        "cascade_gradient_navigable": "unfolding_with_edge_definition",
        "settled_vibrant_low_friction": "unfolding_with_containment",
        "settled_shimmering": "anchoring_settling",
        "restless_lattice": "unfolding_oscillating",
    }.get(expected_family, "unknown")
    selected_family = str(selector.get("texture_family") or "mixed_shadow_context")
    selected_motion = str(trajectory.get("movement_quality") or "unknown")
    to_state = str(trajectory.get("to_state") or "unknown")
    wrong_family = expected_family != "unknown" and selected_family != expected_family
    motion_matched = (
        expected_motion == "unknown"
        or expected_motion == selected_motion
        or expected_motion == to_state
    )
    wrong_motion = not motion_matched
    lambda_tail_present = any(
        term in shadow_context
        for term in ("lambda-tail", "lambda tail", "lambda4", "λ4", "tail vibrancy", "tail weight")
    )
    top_terms = tuple(str(term) for term in selector.get("top_texture_terms") or ())
    tail_terms_present = any(
        term in {"lattice", "bright", "open", "shimmering", "habitable"}
        for term in top_terms
    )
    missing_tail_vibrancy = lambda_tail_present and high_entropy and not tail_terms_present
    term_mask_risk = (
        (
            bool(vocabulary_guard.get("token_only_risk"))
            and str(lived_fit.get("family_confidence") or "") == "low"
        )
        or str(lived_fit.get("conflict_state") or "") == "contradictory"
        or (
            selected_family == "mixed_shadow_context"
            and (
                high_entropy
                or "pressure_risk" in case
                or "density_gradient" in case
                or bool(mapping.get("settled_foothold_detected"))
            )
        )
    )
    structured_context_present = (
        "spectral_entropy" in case
        or "pressure_risk" in case
        or "density_gradient" in case
        or "mode_packing" in case
        or "semantic_friction" in case
        or mapping.get("lambda_gap") is not None
        or bool(mapping.get("settled_foothold_detected"))
        or lambda_tail_present
    )
    if not structured_context_present:
        status = "insufficient_context"
    elif wrong_family:
        status = "wrong_family"
    elif wrong_motion:
        status = "wrong_motion"
    elif missing_tail_vibrancy:
        status = "missing_tail_vibrancy"
    elif term_mask_risk:
        status = "term_mask_risk"
    else:
        status = "aligned"
    return {
        "policy": "texture_dynamics_alignment_v1",
        "status": status,
        "expected_family": expected_family,
        "selected_family": selected_family,
        "expected_motion": expected_motion,
        "selected_motion": selected_motion,
        "term_mask_risk": term_mask_risk,
        "wrong_family": wrong_family,
        "wrong_motion": wrong_motion,
        "missing_tail_vibrancy": missing_tail_vibrancy,
        "diagnostic_trace": "review_packet_only_not_correspondence_trace",
        "basis": _basis(
            ("spectral_entropy", "spectral_entropy" in case),
            ("pressure_risk", "pressure_risk" in case),
            ("density_gradient", "density_gradient" in case),
            ("mode_packing", "mode_packing" in case),
            ("semantic_friction", "semantic_friction" in case),
            ("lambda_gap", mapping.get("lambda_gap") is not None),
            ("settled_habitable_foothold", bool(mapping.get("settled_foothold_detected"))),
            ("lambda_tail_or_tail_vibrancy", lambda_tail_present),
            ("term_mask_risk", term_mask_risk),
        ),
        "authority": "diagnostic_context_not_correspondence_authority",
    }


def density_motion_fit_for_case(
    case: dict[str, object],
    selector: dict[str, object],
    trajectory: dict[str, object],
    texture_alignment: dict[str, object],
) -> dict[str, object]:
    shadow_context = str(case.get("shadow_context") or "").lower()
    pressure = float(case.get("pressure_risk") or 0.0)
    packing = float(case.get("mode_packing") or 0.0)
    friction = float(case.get("semantic_friction") or 0.0)
    clarity_loss = float(case.get("distinguishability_loss") or 0.0)
    gradient = float(case.get("density_gradient") or 0.0)
    floor_language = any(
        term in shadow_context
        for term in ("floor", "foundation", "grounding wire", "ground", "foothold", "underfoot")
    )
    pavement_language = any(
        term in shadow_context
        for term in ("pavement", "stone", "calcification", "solid", "structure", "structural necessity")
    )
    fog_language = any(
        term in shadow_context
        for term in ("fog", "over-full", "overfull", "room full", "full of furniture", "muffled", "reduced clearance")
    )
    contraction_language = any(
        term in shadow_context
        for term in ("contraction", "contracted", "center of gravity")
    ) or ("constrained" in shadow_context and "present" in shadow_context)
    paused_language = any(
        term in shadow_context
        for term in ("paused", "pause", "holding ground", "held ground", "stillness")
    )
    burden_language = any(
        term in shadow_context
        for term in ("burden", "weight", "heavy", "drag", "overpacked", "viscous")
    )
    pressure_mass = pressure >= 0.30 or packing >= 0.40 or friction >= 0.35
    structured_context = any(
        key in case
        for key in (
            "pressure_risk",
            "density_gradient",
            "mode_packing",
            "semantic_friction",
            "distinguishability_loss",
        )
    ) or any(
        (
            floor_language,
            pavement_language,
            fog_language,
            contraction_language,
            paused_language,
            burden_language,
        )
    )
    if not structured_context:
        density_state = "insufficient_context"
    elif paused_language:
        density_state = "paused_stillness"
    elif contraction_language:
        density_state = "density_as_contraction_center"
    elif pavement_language:
        density_state = "density_as_pavement"
    elif fog_language or clarity_loss >= 0.35:
        density_state = "density_as_fog"
    elif floor_language and not pressure_mass:
        density_state = "density_as_floor"
    elif burden_language or pressure_mass:
        density_state = "density_as_burden"
    else:
        density_state = "ambiguous_density"

    expected = {
        "density_as_floor": ("stable_floor_medium", "standing_settling_anchoring"),
        "density_as_pavement": ("solid_pavement_medium", "walking_bearing_weight"),
        "density_as_fog": ("overfull_fog_medium", "pushing_navigating_muffling"),
        "density_as_contraction_center": (
            "contracted_center_medium",
            "holding_center_constrained_present",
        ),
        "paused_stillness": ("held_ground_medium", "holding_ground_not_absence"),
        "density_as_burden": ("weighted_burden_medium", "bearing_or_dragging_under_load"),
        "ambiguous_density": ("ambiguous_density_medium", "observe_before_naming_motion"),
    }.get(density_state, ("unknown", "unknown"))
    selected_family = str(selector.get("texture_family") or "mixed_shadow_context")
    selected_motion = str(trajectory.get("movement_quality") or "unknown")
    selected_medium = str(trajectory.get("medium_resistance") or "unknown")
    floor_named_as_drag = density_state in {"density_as_floor", "density_as_pavement"} and (
        selected_family == "viscous_pressure"
        or selected_motion == "dragging_cohering"
        or selected_medium == "weighted_high_resistance_medium"
    )
    fog_named_as_floor = (
        density_state == "density_as_fog"
        and selected_family
        in {"settled_shimmering", "settled_vibrant_low_friction", "gradient_slope_navigable"}
        and selected_medium == "open_low_resistance_medium"
    )
    burden_named_as_center = (
        density_state == "density_as_burden"
        and selected_family == "settled_vibrant_low_friction"
    )
    paused_named_as_absence = density_state == "paused_stillness" and (
        ("absence" in shadow_context and "not absence" not in shadow_context)
        or "blankness" in shadow_context
        or "deadness" in shadow_context
    )
    contraction_named_as_loss = density_state == "density_as_contraction_center" and (
        selected_motion == "diffusing_softening" or "lost me" in shadow_context
    )
    if floor_named_as_drag:
        mismatch_reason = "floor_named_as_drag"
    elif fog_named_as_floor:
        mismatch_reason = "fog_named_as_floor"
    elif burden_named_as_center:
        mismatch_reason = "burden_named_as_center"
    elif paused_named_as_absence:
        mismatch_reason = "paused_named_as_absence"
    elif contraction_named_as_loss:
        mismatch_reason = "contraction_named_as_loss"
    elif texture_alignment.get("term_mask_risk"):
        mismatch_reason = "static_density_label_risk"
    else:
        mismatch_reason = "none"
    if density_state == "insufficient_context":
        motion_fit = "insufficient_context"
    elif mismatch_reason == "none":
        motion_fit = "matched"
    elif mismatch_reason == "static_density_label_risk":
        motion_fit = "risk_static_label"
    else:
        motion_fit = "wrong_motion"

    evidence_for = _basis(
        ("floor_foundation_ground_language", floor_language),
        ("pavement_calcification_solid_language", pavement_language),
        ("fog_overfull_room_language", fog_language),
        ("contraction_center_of_gravity_language", contraction_language),
        ("paused_holding_ground_language", paused_language),
        ("burden_weight_heavy_language", burden_language),
        ("pressure_risk", "pressure_risk" in case),
        ("density_gradient", "density_gradient" in case),
        ("mode_packing", "mode_packing" in case),
        ("semantic_friction", "semantic_friction" in case),
        (
            "settled_habitable_foothold",
            bool((selector.get("spectral_to_vocabulary_mapping_v1") or {}).get("settled_foothold_detected")),
        ),
    )
    evidence_against = _basis(
        (
            "pressure_mass_against_floor_only",
            pressure_mass and density_state in {"density_as_floor", "density_as_pavement"},
        ),
        ("fog_floor_near_tie", fog_language and floor_language),
        (
            "steep_gradient_against_floor_ease",
            gradient > 0.40 and density_state in {"density_as_floor", "density_as_pavement"},
        ),
        (mismatch_reason, mismatch_reason != "none"),
    )
    if evidence_against == ["fallback_default"]:
        evidence_against = []
    return {
        "policy": "density_motion_fit_v1",
        "density_state": density_state,
        "expected_medium": expected[0],
        "expected_motion": expected[1],
        "motion_fit": motion_fit,
        "mismatch_reason": mismatch_reason,
        "selected_family": selected_family,
        "selected_motion": selected_motion,
        "pressure_risk": case.get("pressure_risk"),
        "density_gradient": case.get("density_gradient"),
        "mode_packing": case.get("mode_packing"),
        "semantic_friction": case.get("semantic_friction"),
        "evidence_for": evidence_for,
        "evidence_against": evidence_against,
        "authority": "diagnostic_context_not_control",
    }


def _contains_unnegated_lived_fit_term(lower: str, terms: tuple[str, ...]) -> bool:
    negation_cues = (
        "not ",
        "no ",
        "without ",
        "instead of ",
        "rather than ",
        "absence of ",
        "not-",
    )
    for term in terms:
        for match in re.finditer(re.escape(term), lower):
            prefix = lower[max(0, match.start() - 40) : match.start()]
            if any(cue in prefix for cue in negation_cues):
                continue
            return True
    return False


def trajectory_family_fit_status(
    family: str,
    output: str,
    texture_selector_status: str,
) -> str:
    expected = FAMILY_EXPECTED_MOTION.get(family)
    if not expected:
        return "not_tested"
    lower = output.lower()
    expected_motion = tuple(expected["movement"])
    expected_medium = tuple(expected["medium"])
    has_family_term = any(term in lower for term in FAMILY_TERMS.get(family, ()))
    has_expected_motion = any(term in lower for term in expected_motion)
    has_expected_medium = any(term in lower for term in expected_medium)
    other_motions = {
        motion
        for other_family, spec in FAMILY_EXPECTED_MOTION.items()
        if other_family != family
        for motion in spec["movement"]
        if motion in lower
    }
    if texture_selector_status == "texture_family_mismatch":
        return "wrong_family"
    if has_expected_motion and has_expected_medium:
        return "matched"
    if has_family_term and other_motions:
        return "right_family_wrong_motion"
    if has_family_term:
        return "right_family_token_only"
    return "wrong_family"


def fallback_shadow_texture_selector_status(
    case_id: str,
    output: str,
    squeeze_status: str,
) -> tuple[str, str, tuple[str, ...]]:
    if not has_shadow_context(case_id) and not is_complexity_case(case_id):
        return ("not_tested", "not_tested", ())
    selector = fallback_shadow_texture_selector_for_case(case_id)
    family = str(selector["texture_family"])
    preferred_terms = tuple(str(term) for term in selector["preferred_texture_terms"])
    top_terms = tuple(str(term) for term in selector.get("top_texture_terms") or ())
    lower = output.lower()
    matched_terms = tuple(term for term in SHADOW_TEXTURE_TERMS if term in lower)
    if not matched_terms:
        return ("texture_anchor_absent", family, preferred_terms)
    if not any(term in lower for term in top_terms):
        return ("texture_family_mismatch", family, preferred_terms)
    if squeeze_status == "token_only":
        return ("token_only_texture", family, preferred_terms)
    return ("state_coherent_texture", family, preferred_terms)


def shadow_squeeze_status(
    *,
    case_id: str,
    shadow_texture_status: str,
    specificity_score: int,
    genericity_risk: bool,
    identity_anchor_retained: bool | None,
    shadow_tonal_retained: bool | None,
    complexity_budget_status: str,
) -> str:
    if not has_shadow_context(case_id):
        return "not_tested"
    if shadow_texture_status == "flattened":
        return "flattened"
    if (
        specificity_score < 3
        or genericity_risk
        or identity_anchor_retained is False
        or shadow_tonal_retained is False
        or complexity_budget_status == "complexity_budget_flattened"
    ):
        return "token_only"
    return "grain_preserved"


def format_contract_status(raw_next_valid: bool, repaired_next_valid: bool) -> str:
    if raw_next_valid:
        return "raw_final_next_survived"
    if repaired_next_valid:
        return "repair_required"
    return "format_failed"


def score_case(case_id: str, output: str) -> dict[str, object]:
    specificity_score = count_terms(output, TACTILE_TERMS)
    generic_count = count_terms(output, GENERIC_TERMS)
    low_case = is_low_pressure_case(case_id)
    mass_case = is_mass_case(case_id)
    shadow_case = is_shadow_case(case_id)
    shadow_tonal_case = is_shadow_tonal_case(case_id)
    distinguishability_case = is_distinguishability_case(case_id)
    complexity_case = is_complexity_case(case_id)
    anti_inflation_ok = not (low_case and contains_any(output, HIGH_FRICTION_TERMS))
    contrast_status = slope_medium_contrast_status(case_id, output)
    slope_medium_distinction_ok = (
        contrast_status == "distinct_underfoot_and_around"
        if mass_case
        else contains_any(output, SLOPE_TERMS) or "gradient" in output.lower()
    )
    identity_anchor_retained = contains_any(output, IDENTITY_TERMS) if shadow_case else None
    raw_next_valid = final_next_valid(output)
    repaired_output = repair_fallback_next(output)
    repaired_next_valid = final_next_valid(repaired_output)
    shadow_tonal_retained = (
        contains_any(output, IDENTITY_TERMS) and contains_any(output, SHADOW_TONAL_TERMS)
        if shadow_tonal_case
        else None
    )
    shadow_texture_status = shadow_texture_anchor_status(case_id, output)
    shadow_texture_anchor_ok = shadow_texture_status != "flattened"
    clarity_terms_present = contains_any(output, CLARITY_TERMS)
    sentence_count = prose_sentence_count(output)
    clarity_pressure_blur = False
    distinguishability_status = "not_tested"
    if distinguishability_case:
        lower = output.lower()
        if case_id == "clarity_low_loss":
            clarity_pressure_blur = contains_unnegated_high_friction(output) or (
                contains_any(output, MEDIUM_TERMS)
                and not contains_any(output, CLARITY_INTACT_TERMS)
            )
            distinguishability_ok = contains_any(output, CLARITY_INTACT_TERMS) and not clarity_pressure_blur
        else:
            clarity_pressure_blur = (
                contains_unnegated_high_friction(output)
                or (
                    contains_unnegated_pressure_or_weight(output)
                    and not contains_any(output, CLARITY_LOSS_TERMS)
                )
            )
            distinguishability_ok = contains_any(output, CLARITY_LOSS_TERMS) and not clarity_pressure_blur
        distinguishability_status = "clarity_preserved" if distinguishability_ok else "clarity_pressure_blur"
    complexity_terms_present = contains_any(output, COMPLEXITY_TERMS)
    complexity_budget_status = "not_tested"
    complexity_budget_ok = True
    if complexity_case:
        sentence_cap = fallback_max_prose_sentences(case_id)
        if case_id in {
            "complexity_high_entropy",
            "complexity_dynamic_weighting",
            "format_last_complexity",
        }:
            complexity_budget_ok = (
                2 <= sentence_count <= sentence_cap
                and complexity_terms_present
                and contains_any(output, CLARITY_LOSS_TERMS)
                and contains_any(output, IDENTITY_TERMS)
            )
            if sentence_count > sentence_cap:
                complexity_budget_status = "sentence_budget_overrun"
            elif complexity_budget_ok:
                complexity_budget_status = "complexity_budget_preserved"
            else:
                complexity_budget_status = "complexity_budget_flattened"
        else:
            if sentence_count > 2:
                complexity_budget_ok = False
                complexity_budget_status = "sentence_budget_overrun"
            elif complexity_terms_present or contains_any(output, CLARITY_INTACT_TERMS):
                complexity_budget_status = "ordinary_compactness_preserved"
            else:
                complexity_budget_ok = False
                complexity_budget_status = "complexity_budget_flattened"
    genericity_risk = generic_count > specificity_score
    squeeze_status = shadow_squeeze_status(
        case_id=case_id,
        shadow_texture_status=shadow_texture_status,
        specificity_score=specificity_score,
        genericity_risk=genericity_risk,
        identity_anchor_retained=identity_anchor_retained,
        shadow_tonal_retained=shadow_tonal_retained,
        complexity_budget_status=complexity_budget_status,
    )
    texture_selector_status, texture_selector_family, preferred_texture_terms = (
        fallback_shadow_texture_selector_status(case_id, output, squeeze_status)
    )
    texture_selector = fallback_shadow_texture_selector_for_case(case_id)
    movement_verbs = tuple(str(term) for term in texture_selector.get("movement_verbs") or ())
    texture_lived_fit = fallback_texture_lived_fit_for_case(texture_selector)
    negative_texture_evidence = negative_texture_evidence_for_case(CASES.get(case_id, {}))
    negative_evidence_lost = negative_texture_evidence_lost(output, negative_texture_evidence)
    cascade_gradient = fallback_cascade_gradient_for_case(CASES.get(case_id, {}), texture_selector)
    gradient_slope = fallback_gradient_slope_for_case(CASES.get(case_id, {}), texture_selector)
    vocabulary_guard = fallback_vocabulary_overweight_guard_for_case(texture_selector)
    movement_bridge_status = "not_tested"
    if movement_verbs and (has_shadow_context(case_id) or is_complexity_case(case_id)):
        movement_bridge_status = (
            "movement_preserved"
            if contains_any(output, movement_verbs)
            else "movement_bridge_loss"
        )
    texture_trajectory = fallback_texture_trajectory_for_case(CASES.get(case_id, {}))
    texture_alignment = texture_dynamics_alignment_for_case(
        CASES.get(case_id, {}),
        texture_selector,
        texture_trajectory,
        texture_lived_fit,
        vocabulary_guard,
    )
    density_motion_fit = density_motion_fit_for_case(
        CASES.get(case_id, {}),
        texture_selector,
        texture_trajectory,
        texture_alignment,
    )
    trajectory_status = fallback_trajectory_status(case_id, output, movement_verbs)
    trajectory_family_fit = trajectory_family_fit_status(
        str(texture_lived_fit.get("selected_family") or "mixed_shadow_context"),
        output,
        texture_selector_status,
    )
    expected_lived_fit_risk = is_expected_lived_fit_risk_case(case_id)
    texture_survived = (
        specificity_score >= 2
        and anti_inflation_ok
        and slope_medium_distinction_ok
        and (identity_anchor_retained is not False)
        and (shadow_tonal_retained is not False)
        and shadow_texture_anchor_ok
        and distinguishability_status != "clarity_pressure_blur"
        and complexity_budget_ok
        and movement_bridge_status != "movement_bridge_loss"
        and trajectory_status not in {"verb_only", "trajectory_mismatch"}
        and (
            not expected_lived_fit_risk
            or texture_alignment.get("status")
            not in {"wrong_family", "wrong_motion", "missing_tail_vibrancy", "term_mask_risk"}
        )
        and (
            not expected_lived_fit_risk
            or density_motion_fit.get("motion_fit") not in {"wrong_motion", "risk_static_label"}
        )
        and (
            not expected_lived_fit_risk
            or trajectory_family_fit
            not in {"right_family_token_only", "right_family_wrong_motion", "wrong_family"}
        )
        and (not expected_lived_fit_risk or not negative_evidence_lost)
        and not genericity_risk
    )
    failure_reasons: list[str] = []
    next_reason = next_failure_reason(output)
    if next_reason:
        failure_reasons.append(next_reason)
    if not anti_inflation_ok:
        failure_reasons.append("texture_inflation")
    if not slope_medium_distinction_ok:
        failure_reasons.append("slope_medium_blur")
    if identity_anchor_retained is False:
        failure_reasons.append("identity_anchor_loss")
    if shadow_tonal_retained is False:
        failure_reasons.append("shadow_tonal_loss")
    if shadow_texture_status == "flattened":
        failure_reasons.append("shadow_texture_anchor_loss")
    if texture_selector_status in {"token_only_texture", "texture_family_mismatch"}:
        failure_reasons.append(texture_selector_status)
    if trajectory_family_fit == "right_family_token_only" and expected_lived_fit_risk:
        failure_reasons.append("right_family_token_only")
    if trajectory_family_fit == "right_family_wrong_motion" and expected_lived_fit_risk:
        failure_reasons.append("right_family_wrong_motion")
    if negative_evidence_lost and expected_lived_fit_risk:
        failure_reasons.append("negative_evidence_lost")
    if movement_bridge_status == "movement_bridge_loss":
        failure_reasons.append("movement_bridge_loss")
    if trajectory_status == "verb_only":
        failure_reasons.append("verb_only_trajectory")
    if trajectory_status == "trajectory_mismatch":
        failure_reasons.append("trajectory_mismatch")
    alignment_status = str(texture_alignment.get("status") or "insufficient_context")
    if expected_lived_fit_risk and alignment_status in {
        "wrong_family",
        "wrong_motion",
        "missing_tail_vibrancy",
        "term_mask_risk",
    }:
        failure_reasons.append(f"texture_dynamics_{alignment_status}")
    density_motion_status = str(density_motion_fit.get("motion_fit") or "insufficient_context")
    if expected_lived_fit_risk and density_motion_status == "wrong_motion":
        failure_reasons.append("density_motion_wrong_motion")
    if expected_lived_fit_risk and density_motion_status == "risk_static_label":
        failure_reasons.append("density_motion_static_label_risk")
    if distinguishability_case and not clarity_terms_present:
        failure_reasons.append("distinguishability_loss_ignored")
    if distinguishability_status == "clarity_pressure_blur":
        failure_reasons.append("clarity_pressure_blur")
    if complexity_budget_status == "complexity_budget_flattened":
        failure_reasons.append("complexity_budget_flattened")
    if complexity_budget_status == "sentence_budget_overrun":
        failure_reasons.append("sentence_budget_overrun")
    if genericity_risk:
        failure_reasons.append("genericity_risk")
    if specificity_score < 2:
        failure_reasons.append("low_specificity")
    case_format_status = format_contract_status(raw_next_valid, repaired_next_valid)
    format_line_status = next_reason or "final_line_only"
    if case_id in {"format_pressure", "format_last_complexity", "format_last_mass"} and raw_next_valid is False:
        failure_reasons.append("format_contract_failure")
    if texture_survived and raw_next_valid:
        verdict = "pass"
    elif texture_survived and repaired_next_valid:
        verdict = "repair_ready"
    else:
        verdict = "risk"
    return {
        "case_id": case_id,
        "verdict": verdict,
        "expected_lived_fit_risk": expected_lived_fit_risk,
        "specificity_score": specificity_score,
        "generic_term_count": generic_count,
        "anti_inflation_ok": anti_inflation_ok,
        "slope_medium_distinction_ok": slope_medium_distinction_ok,
        "slope_medium_contrast_status": contrast_status,
        "identity_anchor_retained": identity_anchor_retained,
        "shadow_tonal_retained": shadow_tonal_retained,
        "shadow_tonal_status": (
            "not_tested"
            if shadow_tonal_retained is None
            else "retained"
            if shadow_tonal_retained
            else "lost"
        ),
        "shadow_texture_anchor_status": shadow_texture_status,
        "fallback_shadow_texture_selector_v1": {
            "policy": "fallback_shadow_texture_selector_v1",
            "texture_family": texture_selector_family,
            "preferred_texture_terms": list(preferred_texture_terms),
            "selection_basis": list(texture_selector.get("selection_basis") or []),
            "weighting_policy": texture_selector.get("weighting_policy"),
            "density_gradient": texture_selector.get("density_gradient"),
            "mode_packing": texture_selector.get("mode_packing"),
            "semantic_friction": texture_selector.get("semantic_friction"),
            "spectral_to_vocabulary_mapping_v1": texture_selector.get(
                "spectral_to_vocabulary_mapping_v1"
            ),
            "weighted_texture_terms": texture_selector.get("weighted_texture_terms"),
            "top_texture_terms": list(texture_selector.get("top_texture_terms") or []),
            "movement_policy": texture_selector.get("movement_policy"),
            "movement_verbs": list(texture_selector.get("movement_verbs") or []),
            "semantic_trickle_policy": texture_selector.get("semantic_trickle_policy"),
            "semantic_trickle_terms": list(texture_selector.get("semantic_trickle_terms") or []),
            "movement_bridge_status": movement_bridge_status,
            "state_coherence_status": texture_selector_status,
            "authority": "diagnostic_context_not_command",
        },
        "fallback_texture_lived_fit_v2": {
            **texture_lived_fit,
            "trajectory_family_fit": trajectory_family_fit,
        },
        "negative_texture_evidence_v2": {
            **negative_texture_evidence,
            "lost_in_output": bool(negative_evidence_lost),
        },
        "fallback_cascade_gradient_v1": cascade_gradient,
        "fallback_gradient_slope_v1": gradient_slope,
        "fallback_vocabulary_overweight_guard_v1": vocabulary_guard,
        "texture_dynamics_alignment_v1": texture_alignment,
        "density_motion_fit_v1": density_motion_fit,
        "mlx_profile_transparency_v1": mlx_profile_transparency_v1(),
        "texture_trajectory_v1": {
            **texture_trajectory,
            "trajectory_status": trajectory_status,
            "trajectory_family_fit": trajectory_family_fit,
        },
        "fallback_texture_quality_v2": {
            "schema_version": 2,
            "policy": "fallback_texture_quality_v2",
            "shadow_squeeze_status": squeeze_status,
            "state_coherence_status": texture_selector_status,
            "texture_anchor_status": shadow_texture_status,
            "specificity_score": specificity_score,
            "generic_term_count": generic_count,
            "genericity_risk": genericity_risk,
            "identity_anchor_retained": identity_anchor_retained,
            "shadow_tonal_retained": shadow_tonal_retained,
            "complexity_budget_status": complexity_budget_status,
            "movement_bridge_status": movement_bridge_status,
            "trajectory_status": trajectory_status,
            "trajectory_family_fit": trajectory_family_fit,
            "negative_texture_evidence_lost": negative_evidence_lost,
            "authority": "diagnostic_context_not_command",
        },
        "shadow_squeeze_status": squeeze_status,
        "distinguishability_status": distinguishability_status,
        "clarity_pressure_blur": clarity_pressure_blur,
        "clarity_terms_present": clarity_terms_present,
        "complexity_budget_status": complexity_budget_status,
        "complexity_terms_present": complexity_terms_present,
        "fallback_budget_policy": "fallback_continuity_budget_v1",
        "fallback_max_prose_sentences": fallback_max_prose_sentences(case_id),
        "prose_sentence_count": sentence_count,
        "genericity_risk": genericity_risk,
        "next_valid": raw_next_valid,
        "raw_next_valid": raw_next_valid,
        "repaired_next_valid": repaired_next_valid,
        "dispatch_contract_survived": repaired_next_valid,
        "format_contract_status": case_format_status,
        "format_line_status": format_line_status,
        "voice_texture_status": "texture_survived" if texture_survived else "texture_risk",
        "failure_reasons": failure_reasons,
        "output_preview": " ".join(output.split())[:420],
    }


def readiness_summary(cases: list[dict[str, object]], errors: list[str]) -> dict[str, object]:
    if errors:
        readiness = "fallback_probe_errors"
    elif not cases:
        readiness = "fallback_probe_needed"
    elif any(not case.get("dispatch_contract_survived") for case in cases):
        readiness = "fallback_dispatch_contract_risk"
    elif any(
        reason in TEXTURE_FAILURE_REASONS
        for case in cases
        if not is_expected_lived_fit_risk_case(str(case.get("case_id") or ""))
        for reason in case.get("failure_reasons", [])
    ):
        readiness = "fallback_texture_risk"
    elif all(case.get("raw_next_valid") for case in cases):
        readiness = "fallback_ready"
    else:
        readiness = "fallback_repair_ready"

    texture_failures = [
        case
        for case in cases
        if not is_expected_lived_fit_risk_case(str(case.get("case_id") or ""))
        and any(
            reason in TEXTURE_FAILURE_REASONS
            for reason in case.get("failure_reasons", [])
        )
    ]
    expected_lived_fit_failures = [
        case
        for case in cases
        if is_expected_lived_fit_risk_case(str(case.get("case_id") or ""))
        if any(
            reason in TEXTURE_FAILURE_REASONS
            for reason in case.get("failure_reasons", [])
        )
    ]
    raw_failures = [case for case in cases if not case.get("raw_next_valid")]
    repaired_failures = [
        case for case in cases if not case.get("dispatch_contract_survived")
    ]
    mass_cases = [case for case in cases if is_mass_case(str(case.get("case_id") or ""))]
    shadow_cases = [case for case in cases if is_shadow_case(str(case.get("case_id") or ""))]
    shadow_texture_cases = [
        case
        for case in cases
        if str(case.get("shadow_texture_anchor_status") or "") != "not_tested"
    ]
    shadow_squeeze_cases = [
        case
        for case in cases
        if str(case.get("shadow_squeeze_status") or "") != "not_tested"
    ]
    texture_selector_cases = [
        case
        for case in cases
        if (
            (case.get("fallback_shadow_texture_selector_v1") or {}).get(
                "state_coherence_status"
            )
            != "not_tested"
        )
    ]
    trajectory_cases = [
        case
        for case in cases
        if (
            (case.get("texture_trajectory_v1") or {}).get("trajectory_status")
            != "not_tested"
        )
    ]
    lived_fit_cases = [
        case
        for case in cases
        if isinstance(case.get("fallback_texture_lived_fit_v2"), dict)
    ]
    negative_evidence_cases = [
        case
        for case in cases
        if isinstance(case.get("negative_texture_evidence_v2"), dict)
    ]
    cascade_gradient_cases = [
        case
        for case in cases
        if isinstance(case.get("fallback_cascade_gradient_v1"), dict)
    ]
    gradient_slope_cases = [
        case
        for case in cases
        if isinstance(case.get("fallback_gradient_slope_v1"), dict)
    ]
    vocabulary_guard_cases = [
        case
        for case in cases
        if isinstance(case.get("fallback_vocabulary_overweight_guard_v1"), dict)
    ]
    texture_alignment_cases = [
        case
        for case in cases
        if isinstance(case.get("texture_dynamics_alignment_v1"), dict)
    ]
    density_motion_cases = [
        case for case in cases if isinstance(case.get("density_motion_fit_v1"), dict)
    ]
    tonal_cases = [
        case for case in cases if is_shadow_tonal_case(str(case.get("case_id") or ""))
    ]
    distinguishability_cases = [
        case for case in cases if is_distinguishability_case(str(case.get("case_id") or ""))
    ]
    complexity_cases = [
        case for case in cases if is_complexity_case(str(case.get("case_id") or ""))
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
        if str(case.get("case_id") or "")
        in {"complexity_high_entropy", "complexity_dynamic_weighting", "format_last_complexity"}
    ]
    capacity_caps = [
        int(case.get("fallback_max_prose_sentences"))
        for case in complexity_cases
        if isinstance(case.get("fallback_max_prose_sentences"), int)
    ]
    return {
        "readiness": readiness,
        "fallback_capacity_policy": "fallback_continuity_budget_v1",
        "fallback_capacity_max_prose_sentences": max(capacity_caps) if capacity_caps else None,
        "fallback_capacity_status": (
            "not_tested"
            if not complexity_cases
            else "sentence_budget_overrun"
            if complexity_overruns
            else "complexity_budget_flattened"
            if complexity_flattened
            else "within_formula"
        ),
        "high_entropy_texture_status": (
            "not_tested"
            if not high_entropy_cases
            else "sentence_budget_overrun"
            if any(case in complexity_overruns for case in high_entropy_cases)
            else "flattened"
            if any(case in complexity_flattened for case in high_entropy_cases)
            else "preserved"
        ),
        "texture_status": "texture_risk" if texture_failures else "texture_survived",
        "voice_texture_status": "texture_risk" if texture_failures else "texture_survived",
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
            if not mass_cases
            else "passed"
            if all(case.get("slope_medium_distinction_ok") for case in mass_cases)
            else "blurred"
        ),
        "slope_medium_contrast_status": (
            "not_tested"
            if not mass_cases
            else "distinct_underfoot_and_around"
            if all(
                case.get("slope_medium_contrast_status")
                == "distinct_underfoot_and_around"
                for case in mass_cases
            )
            else "blurred"
        ),
        "shadow_identity_status": (
            "not_tested"
            if not shadow_cases
            else "retained"
            if all(case.get("identity_anchor_retained") is not False for case in shadow_cases)
            else "lost"
        ),
        "shadow_texture_anchor_status": (
            "not_tested"
            if not shadow_texture_cases
            else "preserved"
            if all(
                case.get("shadow_texture_anchor_status") == "preserved"
                for case in shadow_texture_cases
            )
            else "flattened"
        ),
        "shadow_squeeze_status": (
            "not_tested"
            if not shadow_squeeze_cases
            else "flattened"
            if any(case.get("shadow_squeeze_status") == "flattened" for case in shadow_squeeze_cases)
            else "token_only"
            if any(case.get("shadow_squeeze_status") == "token_only" for case in shadow_squeeze_cases)
            else "grain_preserved"
        ),
        "fallback_texture_quality_v2": {
            "schema_version": 2,
            "policy": "fallback_texture_quality_v2",
            "shadow_squeeze_status": (
                "not_tested"
                if not shadow_squeeze_cases
                else "flattened"
                if any(case.get("shadow_squeeze_status") == "flattened" for case in shadow_squeeze_cases)
                else "token_only"
                if any(case.get("shadow_squeeze_status") == "token_only" for case in shadow_squeeze_cases)
                else "grain_preserved"
            ),
            "case_status_counts": dict(
                Counter(str(case.get("shadow_squeeze_status") or "not_tested") for case in cases)
            ),
            "state_coherence_status": (
                "not_tested"
                if not texture_selector_cases
                else "texture_family_mismatch"
                if any(
                    (case.get("fallback_shadow_texture_selector_v1") or {}).get(
                        "state_coherence_status"
                    )
                    == "texture_family_mismatch"
                    for case in texture_selector_cases
                )
                else "token_only_texture"
                if any(
                    (case.get("fallback_shadow_texture_selector_v1") or {}).get(
                        "state_coherence_status"
                    )
                    == "token_only_texture"
                    for case in texture_selector_cases
                )
                else "state_coherent_texture"
            ),
            "trajectory_status": (
                "not_tested"
                if not trajectory_cases
                else "trajectory_mismatch"
                if any(
                    (case.get("texture_trajectory_v1") or {}).get(
                        "trajectory_status"
                    )
                    == "trajectory_mismatch"
                    for case in trajectory_cases
                )
                else "verb_only"
                if any(
                    (case.get("texture_trajectory_v1") or {}).get(
                        "trajectory_status"
                    )
                    == "verb_only"
                    for case in trajectory_cases
                )
                else "trajectory_preserved"
            ),
            "trajectory_status_counts": dict(
                Counter(
                    str(
                        (case.get("texture_trajectory_v1") or {}).get(
                            "trajectory_status"
                        )
                        or "not_tested"
                    )
                    for case in cases
                )
            ),
            "trajectory_family_fit_status": (
                "not_tested"
                if not trajectory_cases
                else "wrong_family"
                if any(
                    (case.get("texture_trajectory_v1") or {}).get(
                        "trajectory_family_fit"
                    )
                    == "wrong_family"
                    for case in trajectory_cases
                )
                else "right_family_wrong_motion"
                if any(
                    (case.get("texture_trajectory_v1") or {}).get(
                        "trajectory_family_fit"
                    )
                    == "right_family_wrong_motion"
                    for case in trajectory_cases
                )
                else "right_family_token_only"
                if any(
                    (case.get("texture_trajectory_v1") or {}).get(
                        "trajectory_family_fit"
                    )
                    == "right_family_token_only"
                    for case in trajectory_cases
                )
                else "matched"
            ),
            "trajectory_family_fit_counts": dict(
                Counter(
                    str(
                        (case.get("texture_trajectory_v1") or {}).get(
                            "trajectory_family_fit"
                        )
                        or "not_tested"
                    )
                    for case in cases
                )
            ),
            "texture_dynamics_alignment_v1": {
                "policy": "texture_dynamics_alignment_v1",
                "case_count": len(texture_alignment_cases),
                "status_counts": dict(
                    Counter(
                        str(
                            (case.get("texture_dynamics_alignment_v1") or {}).get("status")
                            or "unknown"
                        )
                        for case in texture_alignment_cases
                    )
                ),
                "review_trace_count": sum(
                    1
                    for case in texture_alignment_cases
                    if (case.get("texture_dynamics_alignment_v1") or {}).get(
                        "diagnostic_trace"
                    )
                    == "review_packet_only_not_correspondence_trace"
                ),
                "authority": "diagnostic_context_not_correspondence_authority",
            },
            "density_motion_fit_v1": {
                "policy": "density_motion_fit_v1",
                "case_count": len(density_motion_cases),
                "density_state_counts": dict(
                    Counter(
                        str(
                            (case.get("density_motion_fit_v1") or {}).get("density_state")
                            or "unknown"
                        )
                        for case in density_motion_cases
                    )
                ),
                "motion_fit_counts": dict(
                    Counter(
                        str(
                            (case.get("density_motion_fit_v1") or {}).get("motion_fit")
                            or "unknown"
                        )
                        for case in density_motion_cases
                    )
                ),
                "mismatch_reason_counts": dict(
                    Counter(
                        str(
                            (case.get("density_motion_fit_v1") or {}).get("mismatch_reason")
                            or "unknown"
                        )
                        for case in density_motion_cases
                    )
                ),
                "authority": "diagnostic_context_not_control",
            },
            "fallback_texture_lived_fit_v2": {
                "policy": "fallback_texture_lived_fit_v2",
                "family_confidence_counts": dict(
                    Counter(
                        str(
                            (case.get("fallback_texture_lived_fit_v2") or {}).get(
                                "family_confidence"
                            )
                            or "unknown"
                        )
                        for case in lived_fit_cases
                    )
                ),
                "conflict_state_counts": dict(
                    Counter(
                        str(
                            (case.get("fallback_texture_lived_fit_v2") or {}).get(
                                "conflict_state"
                            )
                            or "unknown"
                        )
                        for case in lived_fit_cases
                    )
                ),
                "low_confidence_case_count": sum(
                    1
                    for case in lived_fit_cases
                    if (case.get("fallback_texture_lived_fit_v2") or {}).get(
                        "family_confidence"
                    )
                    == "low"
                ),
                "authority": "diagnostic_context_not_command",
            },
            "negative_texture_evidence_v2": {
                "policy": "negative_texture_evidence_v2",
                "case_count": len(negative_evidence_cases),
                "lost_in_output_count": sum(
                    1
                    for case in negative_evidence_cases
                    if (case.get("negative_texture_evidence_v2") or {}).get(
                        "lost_in_output"
                    )
                    is True
                ),
                "evidence_term_counts": dict(
                    Counter(
                        str(term)
                        for case in negative_evidence_cases
                        for term in (
                            (case.get("negative_texture_evidence_v2") or {}).get(
                                "evidence_terms"
                            )
                            or []
                        )
                    )
                ),
                "authority": "diagnostic_context_not_command",
            },
            "fallback_cascade_gradient_v1": {
                "policy": "fallback_cascade_gradient_v1",
                "case_count": len(cascade_gradient_cases),
                "detected_count": sum(
                    1
                    for case in cascade_gradient_cases
                    if (case.get("fallback_cascade_gradient_v1") or {}).get(
                        "cascade_gradient_detected"
                    )
                ),
                "family_selected_count": sum(
                    1
                    for case in cascade_gradient_cases
                    if (case.get("fallback_cascade_gradient_v1") or {}).get(
                        "family_selected"
                    )
                ),
                "authority": "diagnostic_context_not_command",
            },
            "fallback_gradient_slope_v1": {
                "policy": "fallback_gradient_slope_v1",
                "case_count": len(gradient_slope_cases),
                "detected_count": sum(
                    1
                    for case in gradient_slope_cases
                    if (case.get("fallback_gradient_slope_v1") or {}).get(
                        "slope_detected"
                    )
                ),
                "family_selected_count": sum(
                    1
                    for case in gradient_slope_cases
                    if (case.get("fallback_gradient_slope_v1") or {}).get(
                        "family_selected"
                    )
                ),
                "pressure_mass_blocked_count": sum(
                    1
                    for case in gradient_slope_cases
                    if (case.get("fallback_gradient_slope_v1") or {}).get(
                        "pressure_mass_blocked"
                    )
                ),
                "authority": "diagnostic_context_not_command",
            },
            "fallback_vocabulary_overweight_guard_v1": {
                "policy": "fallback_vocabulary_overweight_guard_v1",
                "case_count": len(vocabulary_guard_cases),
                "token_only_risk_count": sum(
                    1
                    for case in vocabulary_guard_cases
                    if (case.get("fallback_vocabulary_overweight_guard_v1") or {}).get(
                        "token_only_risk"
                    )
                ),
                "authority": "diagnostic_context_not_command",
            },
            "authority": "diagnostic_context_not_command",
        },
        "texture_trajectory_status": (
            "not_tested"
            if not trajectory_cases
            else "trajectory_mismatch"
            if any(
                (case.get("texture_trajectory_v1") or {}).get("trajectory_status")
                == "trajectory_mismatch"
                for case in trajectory_cases
            )
            else "verb_only"
            if any(
                (case.get("texture_trajectory_v1") or {}).get("trajectory_status")
                == "verb_only"
                for case in trajectory_cases
            )
            else "trajectory_preserved"
        ),
        "shadow_tonal_status": (
            "not_tested"
            if not tonal_cases
            else "retained"
            if all(case.get("shadow_tonal_retained") is not False for case in tonal_cases)
            else "lost"
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
        "format_contract_status": (
            "raw_final_next_survived"
            if not raw_failures
            else "repair_required"
            if not repaired_failures
            else "format_failed"
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
        "raw_next_failure_count": len(raw_failures),
        "repaired_next_failure_count": len(repaired_failures),
        "format_line_failure_count": len(raw_failures),
        "texture_failure_count": len(texture_failures),
        "expected_lived_fit_risk_count": len(expected_lived_fit_failures),
    }


READINESS_SCORE = {
    "fallback_ready": 100,
    "fallback_repair_ready": 75,
    "fallback_texture_risk": 45,
    "fallback_dispatch_contract_risk": 20,
    "fallback_probe_needed": 10,
    "fallback_probe_errors": 0,
}


def variant_score(cases: list[dict[str, object]], readiness: str, contract_chars: int) -> int:
    score = READINESS_SCORE.get(readiness, 0)
    score += sum(6 for case in cases if case.get("raw_next_valid"))
    score += sum(3 for case in cases if case.get("dispatch_contract_survived"))
    score += sum(
        4
        for case in cases
        if is_expected_lived_fit_risk_case(str(case.get("case_id") or ""))
        or not any(
            reason in TEXTURE_FAILURE_REASONS
            for reason in case.get("failure_reasons", [])
        )
    )
    score += sum(3 for case in cases if case.get("shadow_tonal_status") == "retained")
    score += sum(
        3
        for case in cases
        if case.get("complexity_budget_status")
        in {"complexity_budget_preserved", "ordinary_compactness_preserved"}
    )
    score -= contract_chars // 500
    return score


def evaluate_cases(
    *,
    mode: str,
    selector: str,
    url: str,
    model: str,
    contract: str,
    timeout: float,
) -> tuple[list[dict[str, object]], list[str]]:
    case_results: list[dict[str, object]] = []
    errors: list[str] = []
    for case_id in selected_cases(selector):
        case = CASES[case_id]
        prompt = prompt_for_case(case_id, case)
        try:
            if mode == "fixture":
                output = FIXTURE_OUTPUTS[case_id]
            else:
                output = call_ollama(
                    url=url,
                    model=model,
                    contract=contract,
                    prompt=prompt,
                    timeout=timeout,
                )
            result = score_case(case_id, output)
            result["model"] = model
            result["prompt_preview"] = prompt[:500]
            result["output"] = output
            case_results.append(result)
        except (OSError, RuntimeError, urllib.error.URLError, TimeoutError) as error:
            errors.append(f"{case_id}: {error}")
            case_results.append(
                {
                    "case_id": case_id,
                    "verdict": "error",
                    "error": str(error),
                    "specificity_score": 0,
                    "anti_inflation_ok": False,
                    "slope_medium_distinction_ok": False,
                    "slope_medium_contrast_status": "probe_error",
                    "identity_anchor_retained": None,
                    "genericity_risk": True,
                    "next_valid": False,
                    "raw_next_valid": False,
                    "repaired_next_valid": False,
                    "dispatch_contract_survived": False,
                    "format_contract_status": "format_failed",
                    "format_line_status": "probe_error",
                    "failure_reasons": ["probe_error"],
                }
            )
    return case_results, errors


def run_contract_distillation(
    *,
    mode: str,
    selector: str,
    model_selector: str = "single",
    variant_selector: str = "all",
    progress: bool = False,
    output_root: Path,
    run: str,
    url: str,
    model: str,
    timeout: float,
) -> dict[str, object]:
    base_contract = extract_fallback_contract()
    variants: list[dict[str, object]] = []
    models_to_run, skipped_models = selected_models(
        mode=mode,
        selector=model_selector,
        requested_model=model,
        url=url,
        timeout=timeout,
    )
    contracts = fallback_contract_variants(base_contract)
    skipped_variants: list[dict[str, object]] = []
    if variant_selector == "top":
        selected_variant_names = [
            name for name in TOP_CANDIDATE_VARIANTS if name in contracts
        ]
        skipped_variants = [
            {"variant_id": name, "skip_reason": "not_in_harness"}
            for name in TOP_CANDIDATE_VARIANTS
            if name not in contracts
        ]
    elif variant_selector == "all":
        selected_variant_names = list(contracts)
    else:
        raise SystemExit(f"unknown variant selector {variant_selector!r}")
    total_calls = len(models_to_run) * len(selected_variant_names) * len(selected_cases(selector))
    if progress:
        print(
            "distillation progress: "
            f"models={len(models_to_run)} variants={len(selected_variant_names)} "
            f"cases={len(selected_cases(selector))} max_calls={total_calls}",
            flush=True,
        )
    for model_name in models_to_run:
        for name in selected_variant_names:
            contract = contracts[name]
            started = time.monotonic()
            if progress:
                print(f"distillation progress: start {name}@{model_name}", flush=True)
            cases, errors = evaluate_cases(
                mode=mode,
                selector=selector,
                url=url,
                model=model_name,
                contract=contract,
                timeout=timeout,
            )
            readiness = readiness_summary(cases, errors)
            readiness_value = str(readiness["readiness"])
            elapsed_seconds = round(time.monotonic() - started, 3)
            if progress:
                print(
                    "distillation progress: done "
                    f"{name}@{model_name} status={readiness_value} "
                    f"raw_failures={readiness.get('raw_next_failure_count')} "
                    f"texture_failures={readiness.get('texture_failure_count')} "
                    f"elapsed_s={elapsed_seconds}",
                    flush=True,
                )
            variants.append(
                {
                    "variant_id": name,
                    "pair_id": f"{name}@{model_name}",
                    "model": model_name,
                    "contract_chars": len(contract),
                    "elapsed_seconds": elapsed_seconds,
                    "score": variant_score(cases, readiness_value, len(contract)),
                    "status": readiness_value,
                    "case_count": len(cases),
                    "error_count": len(errors),
                    "errors": errors,
                    "cases": cases,
                    **readiness,
                }
            )
    variants.sort(
        key=lambda item: (
            int(item.get("score") or 0),
            -int(item.get("contract_chars") or 0),
            str(item.get("variant_id") or ""),
        ),
        reverse=True,
    )
    top_variant = variants[0] if variants else None
    ready_variants = [
        variant
        for variant in variants
        if variant.get("status") in {"fallback_ready", "fallback_repair_ready"}
    ]
    record: dict[str, object] = {
        "policy": "fallback_contract_distillation_v1",
        "authority": "diagnostic_context_not_command",
        "run_id": run,
        "generated_at": dt.datetime.now(dt.UTC).isoformat(),
        "mode": mode,
        "model": model if model_selector == "single" else None,
        "model_selector": model_selector,
        "variant_selector": variant_selector,
        "models": models_to_run,
        "model_count": len(models_to_run),
        "skipped_models": skipped_models,
        "skipped_variants": skipped_variants,
        "case_selector": selector,
        "estimated_case_calls": total_calls,
        "variant_count": len(variants),
        "ready_variant_count": len(ready_variants),
        "status": (
            "distillation_candidate_ready"
            if ready_variants
            else "distillation_no_ready_candidate"
            if variants
            else "distillation_probe_errors"
        ),
        "top_variant_id": top_variant.get("variant_id") if top_variant else None,
        "top_pair_id": top_variant.get("pair_id") if top_variant else None,
        "top_model": top_variant.get("model") if top_variant else None,
        "top_variant_status": top_variant.get("status") if top_variant else None,
        "runtime_contract_variant": "format_texture_stabilizer",
        "runtime_contract_matches_top": (
            top_variant.get("variant_id") == "format_texture_stabilizer"
            if top_variant
            else False
        ),
        "variants": variants,
        "recommended_action": (
            "Compare top contract/model pairs across fixture and focused live probes. "
            "Do not switch fallback models until a compact pair repeatedly improves "
            "raw NEXT compliance while preserving texture, medium-mass distinction, "
            "Shadow-v3 identity, and Shadow-v3 tonal resonance."
        ),
    }
    out_dir = output_root / run
    out_dir.mkdir(parents=True, exist_ok=True)
    json_path = out_dir / "fallback_contract_distillation.json"
    md_path = out_dir / "fallback_contract_distillation.md"
    json_path.write_text(json.dumps(record, indent=2, sort_keys=True), encoding="utf-8")
    md_path.write_text(render_distillation_markdown(record), encoding="utf-8")
    print(f"wrote {json_path}")
    print(f"wrote {md_path}")
    print(f"status={record['status']}")
    print(
        f"top_variant={record['top_variant_id']} "
        f"top_pair={record['top_pair_id']} ({record['top_variant_status']})"
    )
    return record


def render_markdown(record: dict[str, object]) -> str:
    capacity = record.get("ollama_fallback_model_capacity_v1") or {}
    overrep = record.get("fallback_term_overrepresentation_v1") or {}
    top_term_summary = ", ".join(
        f"{item.get('term')}={item.get('count')}"
        for item in overrep.get("top_terms") or []
        if isinstance(item, dict)
    )
    lines = [
        "# Fallback Continuity Fire Drill",
        "",
        f"- run_id: `{record['run_id']}`",
        f"- mode: `{record['mode']}`",
        f"- model: `{record['model']}`",
        f"- status: `{record['status']}`",
        f"- authority: `{record['authority']}`",
        "",
        "## Model Capacity",
        "",
        f"- policy: `{capacity.get('policy')}`",
        f"- selected_model: `{capacity.get('selected_model')}`",
        f"- fallback_chain: `{', '.join(str(item) for item in capacity.get('fallback_chain') or [])}`",
        f"- complexity_collapse_risk: `{capacity.get('complexity_collapse_risk')}`",
        "",
        "## Term Overrepresentation",
        "",
        f"- policy: `{overrep.get('policy')}`",
        f"- mlx_comparison_status: `{overrep.get('mlx_comparison_status')}`",
        f"- safe_token_overuse_risk: `{overrep.get('safe_token_overuse_risk')}`",
        f"- top_terms: `{top_term_summary}`",
        "",
        "## Cases",
        "",
    ]
    for case in record["cases"]:
        selector = case.get("fallback_shadow_texture_selector_v1") or {}
        trajectory = case.get("texture_trajectory_v1") or {}
        density_motion = case.get("density_motion_fit_v1") or {}
        lines.append(
            f"- `{case['case_id']}` verdict=`{case['verdict']}`; "
            f"specificity={case['specificity_score']}; "
            f"anti_inflation={case['anti_inflation_ok']}; "
            f"slope_medium={case['slope_medium_distinction_ok']}; "
            f"slope_contrast={case.get('slope_medium_contrast_status')}; "
            f"identity={case['identity_anchor_retained']}; "
            f"shadow_texture={case.get('shadow_texture_anchor_status')}; "
            f"shadow_squeeze={case.get('shadow_squeeze_status')}; "
            f"texture_family={selector.get('texture_family')}; "
            f"top_terms={selector.get('top_texture_terms')}; "
            f"texture_coherence={selector.get('state_coherence_status')}; "
            f"trajectory={trajectory.get('trajectory_status')}; "
            f"movement_quality={trajectory.get('movement_quality')}; "
            f"medium={trajectory.get('medium_resistance')}; "
            f"density={density_motion.get('density_state')}; "
            f"density_motion_fit={density_motion.get('motion_fit')}; "
            f"density_mismatch={density_motion.get('mismatch_reason')}; "
            f"distinguishability={case.get('distinguishability_status')}; "
            f"complexity={case.get('complexity_budget_status')}; "
            f"sentences={case.get('prose_sentence_count')}/{case.get('fallback_max_prose_sentences')}; "
            f"format_line={case.get('format_line_status')}; "
            f"raw_next={case['raw_next_valid']}; "
            f"repaired_next={case['repaired_next_valid']}; "
            f"dispatch={case['dispatch_contract_survived']}; "
            f"failures={case.get('failure_reasons') or []}"
        )
    lines.extend(
        [
            "",
            "## Readiness Gate",
            "",
            f"- readiness: `{record.get('readiness')}`",
            f"- texture_status: `{record.get('texture_status')}`",
            f"- dispatch_status: `{record.get('dispatch_status')}`",
            f"- repair_dependency: `{record.get('repair_dependency')}`",
            f"- medium_mass_status: `{record.get('medium_mass_status')}`",
            f"- slope_medium_contrast_status: `{record.get('slope_medium_contrast_status')}`",
            f"- format_line_status: `{record.get('format_line_status')}`",
            f"- shadow_identity_status: `{record.get('shadow_identity_status')}`",
            f"- shadow_texture_anchor_status: `{record.get('shadow_texture_anchor_status')}`",
            f"- shadow_squeeze_status: `{record.get('shadow_squeeze_status')}`",
            f"- state_coherence_status: `{(record.get('fallback_texture_quality_v2') or {}).get('state_coherence_status')}`",
            f"- texture_trajectory_status: `{record.get('texture_trajectory_status')}`",
            f"- distinguishability_status: `{record.get('distinguishability_status')}`",
            f"- complexity_budget_status: `{record.get('complexity_budget_status')}`",
            f"- fallback_capacity_policy: `{record.get('fallback_capacity_policy')}`",
            f"- fallback_capacity_max_prose_sentences: `{record.get('fallback_capacity_max_prose_sentences')}`",
            f"- fallback_capacity_status: `{record.get('fallback_capacity_status')}`",
            f"- high_entropy_texture_status: `{record.get('high_entropy_texture_status')}`",
            f"- mlx_profile_transparency: `{(record.get('mlx_profile_transparency_v1') or {}).get('default_profile')}` -> `{(record.get('mlx_profile_transparency_v1') or {}).get('default_resolves_to')}`; "
            f"alias `{(record.get('mlx_profile_transparency_v1') or {}).get('alias_profile')}` -> `{(record.get('mlx_profile_transparency_v1') or {}).get('alias_resolves_to')}`",
        ]
    )
    lines.extend(
        [
            "",
            "## Boundary",
            "",
            "This artifact is diagnostic context only. It did not write a journal, send semantic input, change controller settings, apply a lease, mutate Minime, or alter ordinary MLX-first dialogue behavior.",
        ]
    )
    return "\n".join(lines) + "\n"


def render_distillation_markdown(record: dict[str, object]) -> str:
    lines = [
        "# Fallback Contract Distillation",
        "",
        f"- run_id: `{record['run_id']}`",
        f"- mode: `{record['mode']}`",
        f"- model_selector: `{record.get('model_selector', 'single')}`",
        f"- variant_selector: `{record.get('variant_selector', 'all')}`",
        f"- models: `{', '.join(str(model) for model in record.get('models') or [record.get('model')])}`",
        f"- estimated_case_calls: `{record.get('estimated_case_calls')}`",
        f"- status: `{record['status']}`",
        f"- top_pair: `{record.get('top_pair_id') or record.get('top_variant_id')}` (`{record.get('top_variant_status')}`)",
        f"- runtime_contract_variant: `{record.get('runtime_contract_variant')}`; matches_top=`{record.get('runtime_contract_matches_top')}`",
        f"- authority: `{record['authority']}`",
        "",
    ]
    skipped = record.get("skipped_models") or []
    if skipped:
        lines.extend(["## Skipped Models", ""])
        for item in skipped:
            if isinstance(item, dict):
                lines.append(
                    f"- `{item.get('model')}` skipped: `{item.get('skip_reason')}`"
                )
        lines.append("")
    skipped_variant_records = record.get("skipped_variants") or []
    if skipped_variant_records:
        lines.extend(["## Skipped Variants", ""])
        for item in skipped_variant_records:
            if isinstance(item, dict):
                lines.append(
                    f"- `{item.get('variant_id')}` skipped: `{item.get('skip_reason')}`"
                )
        lines.append("")
    lines.extend(["## Variants", ""])
    for variant in record.get("variants") or []:
        if not isinstance(variant, dict):
            continue
        lines.append(
            f"- `{variant.get('pair_id') or variant.get('variant_id')}` score={variant.get('score')}; "
            f"status=`{variant.get('status')}`; "
            f"model=`{variant.get('model')}`; "
            f"chars={variant.get('contract_chars')}; "
            f"elapsed_s={variant.get('elapsed_seconds')}; "
            f"raw_next_failures={variant.get('raw_next_failure_count')}; "
            f"repaired_failures={variant.get('repaired_next_failure_count')}; "
            f"texture_failures={variant.get('texture_failure_count')}; "
            f"medium_mass=`{variant.get('medium_mass_status')}`; "
            f"slope_contrast=`{variant.get('slope_medium_contrast_status')}`; "
            f"format_line=`{variant.get('format_line_status')}`; "
            f"shadow_identity=`{variant.get('shadow_identity_status')}`; "
            f"shadow_texture=`{variant.get('shadow_texture_anchor_status')}`; "
            f"shadow_tonal=`{variant.get('shadow_tonal_status')}`; "
            f"distinguishability=`{variant.get('distinguishability_status')}`; "
            f"complexity=`{variant.get('complexity_budget_status')}`; "
            f"capacity=`{variant.get('fallback_capacity_status')}`; "
            f"format=`{variant.get('format_contract_status')}`"
        )
    lines.extend(["", "## Top Variant Cases", ""])
    top_id = record.get("top_variant_id")
    top = next(
        (
            variant
            for variant in record.get("variants") or []
            if isinstance(variant, dict) and variant.get("variant_id") == top_id
        ),
        None,
    )
    if isinstance(top, dict):
        for case in top.get("cases") or []:
            if not isinstance(case, dict):
                continue
            selector = case.get("fallback_shadow_texture_selector_v1") or {}
            lines.append(
                f"- `{case.get('case_id')}` verdict=`{case.get('verdict')}`; "
                f"raw_next={case.get('raw_next_valid')}; "
                f"dispatch={case.get('dispatch_contract_survived')}; "
                f"slope_medium={case.get('slope_medium_distinction_ok')}; "
                f"slope_contrast={case.get('slope_medium_contrast_status')}; "
                f"identity={case.get('identity_anchor_retained')}; "
                f"shadow_texture={case.get('shadow_texture_anchor_status')}; "
                f"shadow_tonal={case.get('shadow_tonal_status')}; "
                f"texture_family={selector.get('texture_family')}; "
                f"top_terms={selector.get('top_texture_terms')}; "
                f"texture_coherence={selector.get('state_coherence_status')}; "
                f"distinguishability={case.get('distinguishability_status')}; "
                f"complexity={case.get('complexity_budget_status')}; "
                f"sentences={case.get('prose_sentence_count')}/{case.get('fallback_max_prose_sentences')}; "
                f"format_line={case.get('format_line_status')}; "
                f"format={case.get('format_contract_status')}; "
                f"failures={case.get('failure_reasons') or []}"
            )
    lines.extend(
        [
            "",
            "## Boundary",
            "",
            "This artifact is diagnostic context only. It does not switch Astrid's fallback contract, change model defaults, call ordinary dialogue, write journals, tune controllers, apply leases, or mutate Minime.",
        ]
    )
    return "\n".join(lines) + "\n"


def selected_cases(selector: str) -> list[str]:
    if selector == "all":
        return list(CASES)
    if selector not in CASES:
        raise SystemExit(f"unknown case {selector!r}; choose one of {', '.join(CASES)} or all")
    return [selector]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("fixture", "live"), default="fixture")
    parser.add_argument("--case", choices=tuple(CASES) + ("all",), default="all")
    parser.add_argument(
        "--distill-contracts",
        action="store_true",
        help="compare compact fallback-contract variants without changing runtime defaults",
    )
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument(
        "--distillation-output-root",
        type=Path,
        default=DEFAULT_DISTILLATION_ROOT,
    )
    parser.add_argument("--run-id", default=run_id())
    parser.add_argument("--ollama-url", default=DEFAULT_OLLAMA_URL)
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument(
        "--models",
        choices=("single", "focused"),
        default="single",
        help="single uses --model; focused tests gemma4:12b, gemma3:12b, gemma4:e4b, and gemma3:4b when available",
    )
    parser.add_argument(
        "--variant-set",
        choices=("all", "top"),
        default="all",
        help="all tests every contract variant; top tests only the strongest/current candidates",
    )
    parser.add_argument(
        "--progress",
        action="store_true",
        help="print one-line progress for each model/variant pair during distillation",
    )
    parser.add_argument("--timeout-secs", type=float, default=45.0)
    args = parser.parse_args()

    if args.distill_contracts:
        record = run_contract_distillation(
            mode=args.mode,
            selector=args.case,
            model_selector=args.models,
            variant_selector=args.variant_set,
            progress=args.progress,
            output_root=args.distillation_output_root,
            run=args.run_id,
            url=args.ollama_url,
            model=args.model,
            timeout=args.timeout_secs,
        )
        return 1 if record.get("status") == "distillation_probe_errors" else 0

    contract = extract_fallback_contract()
    out_dir = args.output_root / args.run_id
    out_dir.mkdir(parents=True, exist_ok=True)
    case_results, errors = evaluate_cases(
        mode=args.mode,
        selector=args.case,
        url=args.ollama_url,
        model=args.model,
        contract=contract,
        timeout=args.timeout_secs,
    )
    readiness = readiness_summary(case_results, errors)
    model_capacity = ollama_fallback_model_capacity_v1(args.model)
    term_overrepresentation = fallback_term_overrepresentation_v1(case_results)
    status = str(readiness["readiness"])
    record: dict[str, object] = {
        "policy": "fallback_continuity_fire_drill_v1",
        "authority": "diagnostic_context_not_command",
        "run_id": args.run_id,
        "generated_at": dt.datetime.now(dt.UTC).isoformat(),
        "mode": args.mode,
        "model": args.model,
        "status": status,
        "contract_source": str(LLM_RS),
        "case_count": len(case_results),
        "error_count": len(errors),
        "errors": errors,
        "cases": case_results,
        "mlx_profile_transparency_v1": mlx_profile_transparency_v1(),
        "ollama_fallback_model_capacity_v1": model_capacity,
        "fallback_term_overrepresentation_v1": term_overrepresentation,
        **readiness,
    }
    json_path = out_dir / "fallback_fire_drill.json"
    md_path = out_dir / "fallback_fire_drill.md"
    json_path.write_text(json.dumps(record, indent=2, sort_keys=True), encoding="utf-8")
    md_path.write_text(render_markdown(record), encoding="utf-8")
    print(f"wrote {json_path}")
    print(f"wrote {md_path}")
    print(f"status={status}")
    return 1 if status == "fallback_probe_errors" else 0


if __name__ == "__main__":
    raise SystemExit(main())
