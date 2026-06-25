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
                    "outcome_score": score,
                    "repeatability_hint": event.get("repeatability_hint"),
                    "promotion_candidate": event.get("promotion_candidate"),
                    "baseline_evidence": event.get("baseline_evidence") or [],
                    "post_lease_evidence": event.get("post_lease_evidence") or [],
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
                "outcome_score": _lease_outcome_score(event),
                "path": event.get("path"),
            }
            for event in repeatable[-6:]
        ],
        "caution_samples": [
            {
                "being": event.get("being"),
                "intent_id": event.get("intent_id"),
                "candidate_control": event.get("candidate_control"),
                "outcome_score": _lease_outcome_score(event),
                "path": event.get("path"),
            }
            for event in caution[-6:]
        ],
        "samples": samples,
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
    regulator_missing_variable_evidence_loop_v1: dict[str, object],
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
    astrid_fill_pressure_calibration = build_astrid_fill_pressure_calibration(entries)
    semantic_friction_calibration = build_semantic_friction_calibration(entries)
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
    actionable_review_items = build_actionable_review_items(
        qualia_comparison=qualia_comparison,
        shared_tail_resonance=shared_tail_resonance,
        resistance_gradient_calibration=resistance_gradient_calibration,
        astrid_introspection_digest_record=astrid_introspection_digest_record,
        shared_choice_envelope=shared_choice_envelope,
        self_regulation_leases=self_regulation_leases,
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
        regulator_missing_variable_evidence_loop_v1=regulator_missing_variable_evidence_loop_v1,
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
        "regulator_missing_variable_evidence_loop_v1": regulator_missing_variable_evidence_loop_v1,
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
