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
        astrid_workspace / "journal/witness_*.txt",
        astrid_workspace / "journal/moment_*.txt",
        astrid_workspace / "journal/astrid_*.txt",
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


def entry_full_text(entry: SelfStudyEntry) -> str:
    try:
        return Path(entry.path).read_text(errors="replace")
    except OSError:
        return entry.preview


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


def build_actionable_review_items(
    *,
    qualia_comparison: dict[str, object],
    shared_tail_resonance: dict[str, object],
    resistance_gradient_calibration: dict[str, object],
    astrid_introspection_digest_record: dict[str, object],
    shared_choice_envelope: dict[str, object],
    self_regulation_leases: dict[str, object],
    astrid_fill_pressure_calibration: dict[str, object],
    shared_pressure_vocabulary_calibration: dict[str, object],
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

    priority_rank = {"high": 0, "medium": 1, "low": 2}
    return sorted(
        items,
        key=lambda item: (
            priority_rank.get(str(item.get("priority")), 9),
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
    astrid_fill_pressure_calibration = build_astrid_fill_pressure_calibration(entries)
    shared_pressure_vocabulary_calibration = (
        build_shared_pressure_vocabulary_calibration(entries)
    )
    actionable_review_items = build_actionable_review_items(
        qualia_comparison=qualia_comparison,
        shared_tail_resonance=shared_tail_resonance,
        resistance_gradient_calibration=resistance_gradient_calibration,
        astrid_introspection_digest_record=astrid_introspection_digest_record,
        shared_choice_envelope=shared_choice_envelope,
        self_regulation_leases=self_regulation_leases,
        astrid_fill_pressure_calibration=astrid_fill_pressure_calibration,
        shared_pressure_vocabulary_calibration=shared_pressure_vocabulary_calibration,
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
        "self_regulation_leases": self_regulation_leases,
        "astrid_fill_pressure_calibration": astrid_fill_pressure_calibration,
        "shared_pressure_vocabulary_calibration": shared_pressure_vocabulary_calibration,
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
