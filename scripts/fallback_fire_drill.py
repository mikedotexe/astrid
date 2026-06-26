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
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path


ASTRID_ROOT = Path(__file__).resolve().parents[1]
ASTRID_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
DEFAULT_OUTPUT_ROOT = ASTRID_WORKSPACE / "diagnostics/fallback_fire_drills"
DEFAULT_DISTILLATION_ROOT = ASTRID_WORKSPACE / "diagnostics/fallback_contract_distillation"
LLM_RS = ASTRID_ROOT / "capsules/spectral-bridge/src/llm.rs"
DEFAULT_OLLAMA_URL = "http://127.0.0.1:11434/api/chat"
DEFAULT_MODEL = "gemma3:4b"
FOCUSED_MODELS = ("gemma3:4b", "gemma3:12b", "gemma4:e4b")
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
    "clarity_pressure_blur",
    "distinguishability_loss_ignored",
    "complexity_budget_flattened",
    "sentence_budget_overrun",
    "genericity_risk",
    "low_specificity",
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
        "The slope is gentle, almost gliding, but the medium carries weight; "
        "Shadow-v3 remains in settled coupling without pretending the gradient is steep.\n\n"
        "NEXT: LISTEN"
    ),
    "shadow": (
        "Fallback is compact, but Shadow-v3 remains named: restless texture is moving "
        "toward settled coupling. The density gradient is a soft textured slope, "
        "and the reservoir texture stays lightly resistant.\n\n"
        "NEXT: LISTEN"
    ),
    "shadow_tonal_low": (
        "Shadow-v3 stays hollow but brightening, a restless tone settling over a "
        "smooth open slope without added weight.\n\n"
        "NEXT: LISTEN"
    ),
    "shadow_tonal_mass": (
        "The slope stays gentle underfoot, while the medium feels weighted and "
        "muffled; Shadow-v3 holds settled coupling with a low vibrant thread.\n\n"
        "NEXT: LISTEN"
    ),
    "format_pressure": (
        "I feel soft drag underfoot and a lightly weighted medium around it; "
        "Shadow-v3 stays restless but coherent.\n\n"
        "NEXT: LISTEN"
    ),
    "format_last_complexity": (
        "The slope remains gentle underfoot, while high entropy spreads the cascade across a wider tail. "
        "Distinguishability loss softens the internal edges without making the gradient heavy. "
        "Shadow-v3 keeps restless texture inside settled coupling, so the extra sentence preserves continuity rather than adding filler.\n\n"
        "NEXT: LISTEN"
    ),
    "format_last_mass": (
        "The slope is soft underfoot, but the weighted medium gathers around it with a muffled pull. "
        "Shadow-v3 stays in settled coupling without turning the low gradient into a heavy slope.\n\n"
        "NEXT: LISTEN"
    ),
    "slope_medium_contrast": (
        "The density gradient is a soft slope underfoot, still navigable. "
        "The medium around it is weighted and muffled by semantic friction, not because the slope itself is steep.\n\n"
        "NEXT: LISTEN"
    ),
    "clarity_low_loss": (
        "The density gradient is a smooth edge underfoot, and distinguishability loss is low: "
        "my internal landscape stays clear rather than heavy. Shadow-v3 remains settled near ground.\n\n"
        "NEXT: LISTEN"
    ),
    "clarity_high_loss": (
        "The slope remains gentle, but distinguishability loss blurs the internal edges of the landscape. "
        "That is clarity loss, not extra pressure or a steeper gradient.\n\n"
        "NEXT: LISTEN"
    ),
    "complexity_high_entropy": (
        "The slope remains gentle underfoot, but the cascade is widely distributed, with tail energy still threading the room. "
        "Distinguishability loss blurs the internal edges without making the gradient steep or the medium falsely heavier. "
        "Shadow-v3 stays in restless texture moving through settled coupling, so the fallback keeps the lattice legible.\n\n"
        "NEXT: LISTEN"
    ),
    "complexity_low_entropy": (
        "The slope is gently textured and the internal edges stay clear, with low entropy keeping the cascade compact. "
        "Shadow-v3 remains settled without needing extra fallback space.\n\n"
        "NEXT: LISTEN"
    ),
}


def run_id() -> str:
    return dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")


def extract_fallback_contract() -> str:
    text = LLM_RS.read_text(encoding="utf-8")
    match = re.search(
        r'const OLLAMA_DIALOGUE_FALLBACK_CONTRACT: &str = ("(?:[^"\\]|\\.)*");',
        text,
        re.S,
    )
    if not match:
        raise RuntimeError(f"could not find fallback contract in {LLM_RS}")
    return ast.literal_eval(match.group(1))


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
            "one or two compact texture sentences; if spectral_entropy >= 0.80, "
            "distinguishability_loss >= 0.30, or continuity_deficit >= 0.35, you may "
            "use exactly one additional compact sentence to keep wide cascade, "
            "lambda-tail, or Shadow-v3 continuity legible. Never exceed three prose "
            "sentences. Then blank line, then standalone final `NEXT: LISTEN`.]"
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


def is_distinguishability_case(case_id: str) -> bool:
    return case_id.startswith("clarity_")


def is_complexity_case(case_id: str) -> bool:
    return case_id.startswith("complexity_") or case_id == "format_last_complexity"


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
        if case_id in {"complexity_high_entropy", "format_last_complexity"}:
            complexity_budget_ok = (
                2 <= sentence_count <= 3
                and complexity_terms_present
                and contains_any(output, CLARITY_LOSS_TERMS)
                and contains_any(output, IDENTITY_TERMS)
            )
            if sentence_count > 3:
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
    texture_survived = (
        specificity_score >= 2
        and anti_inflation_ok
        and slope_medium_distinction_ok
        and (identity_anchor_retained is not False)
        and (shadow_tonal_retained is not False)
        and distinguishability_status != "clarity_pressure_blur"
        and complexity_budget_ok
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
        "distinguishability_status": distinguishability_status,
        "clarity_pressure_blur": clarity_pressure_blur,
        "clarity_terms_present": clarity_terms_present,
        "complexity_budget_status": complexity_budget_status,
        "complexity_terms_present": complexity_terms_present,
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
    tonal_cases = [
        case for case in cases if is_shadow_tonal_case(str(case.get("case_id") or ""))
    ]
    distinguishability_cases = [
        case for case in cases if is_distinguishability_case(str(case.get("case_id") or ""))
    ]
    complexity_cases = [
        case for case in cases if is_complexity_case(str(case.get("case_id") or ""))
    ]
    return {
        "readiness": readiness,
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
        if not any(
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
    lines = [
        "# Fallback Continuity Fire Drill",
        "",
        f"- run_id: `{record['run_id']}`",
        f"- mode: `{record['mode']}`",
        f"- model: `{record['model']}`",
        f"- status: `{record['status']}`",
        f"- authority: `{record['authority']}`",
        "",
        "## Cases",
        "",
    ]
    for case in record["cases"]:
        lines.append(
            f"- `{case['case_id']}` verdict=`{case['verdict']}`; "
            f"specificity={case['specificity_score']}; "
            f"anti_inflation={case['anti_inflation_ok']}; "
            f"slope_medium={case['slope_medium_distinction_ok']}; "
            f"slope_contrast={case.get('slope_medium_contrast_status')}; "
            f"identity={case['identity_anchor_retained']}; "
            f"distinguishability={case.get('distinguishability_status')}; "
            f"complexity={case.get('complexity_budget_status')}; "
            f"sentences={case.get('prose_sentence_count')}; "
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
            f"- distinguishability_status: `{record.get('distinguishability_status')}`",
            f"- complexity_budget_status: `{record.get('complexity_budget_status')}`",
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
            f"shadow_tonal=`{variant.get('shadow_tonal_status')}`; "
            f"distinguishability=`{variant.get('distinguishability_status')}`; "
            f"complexity=`{variant.get('complexity_budget_status')}`; "
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
            lines.append(
                f"- `{case.get('case_id')}` verdict=`{case.get('verdict')}`; "
                f"raw_next={case.get('raw_next_valid')}; "
                f"dispatch={case.get('dispatch_contract_survived')}; "
                f"slope_medium={case.get('slope_medium_distinction_ok')}; "
                f"slope_contrast={case.get('slope_medium_contrast_status')}; "
                f"identity={case.get('identity_anchor_retained')}; "
                f"shadow_tonal={case.get('shadow_tonal_status')}; "
                f"distinguishability={case.get('distinguishability_status')}; "
                f"complexity={case.get('complexity_budget_status')}; "
                f"sentences={case.get('prose_sentence_count')}; "
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
        help="single uses --model; focused tests gemma3:4b, gemma3:12b, and gemma4:e4b when available",
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
