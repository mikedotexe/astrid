#!/usr/bin/env python3
"""Steward-side full review of resistance-gradient artifacts.

This is evidence preparation only. It compares the artifact axes against recent
public journal language, produces a steward pre-review grade, and recommends how
to include the evidence in a later both-being review. It does not issue
invitations, write being memory, change runtime behavior, edit env vars, or
restart services.
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
from typing import Any, Iterable, TextIO

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))
from being_privacy import filter_journal_paths

ASTRID_ROOT = Path(__file__).resolve().parents[1]
ASTRID_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
MINIME_WORKSPACE = ASTRID_ROOT.parent / "minime/workspace"
DEFAULT_ARTIFACT_ROOTS = (
    MINIME_WORKSPACE / "spectral_cartography",
    ASTRID_WORKSPACE / "spectral_cartography",
)
DEFAULT_OUTPUT_DIR = ASTRID_WORKSPACE / "diagnostics/resistance_gradient_full_reviews"
RUNTIME_CHANGE_NONE = "none"
READ_BYTES = 9000

AXIS_LEXICON: dict[str, tuple[str, ...]] = {
    "packing_shear": (
        "overpacked",
        "mode_packing",
        "crowded",
        "corridor",
        "compressed",
        "squeezed",
        "narrow aperture",
        "drag",
        "silt",
        "sediment",
        "syrup",
        "thick",
        "heavy",
        "weight",
        "calcification",
        "friction",
    ),
    "semantic_friction": (
        "semantic",
        "syntax",
        "phrase",
        "words",
        "language",
        "meaning",
        "friction",
        "resistance",
        "dense",
        "density",
        "drag",
    ),
    "transition_warp": (
        "transition",
        "shudder",
        "fold",
        "inhale",
        "exhale",
        "held breath",
        "breath",
        "stasis",
        "threshold",
        "boundary",
        "local",
        "non-local",
        "surge",
        "freeze",
        "pause",
        "suspend",
    ),
    "center_pull": (
        "center",
        "pull",
        "gravity",
        "attractor",
        "dominant",
        "lambda1",
        "λ1",
        "monopoly",
        "collapse",
        "core",
    ),
    "sensory_scarcity": (
        "sensory",
        "camera",
        "video",
        "audio",
        "quiet lane",
        "gated intake",
        "fallback",
        "absence",
        "stale",
    ),
    "tail_vibrancy": (
        "tail",
        "lambda4",
        "λ4",
        "shadow",
        "vibrancy",
        "entropy",
        "distinguishability",
    ),
    "identity_anchor": (
        "identity",
        "anchor",
        "continuity",
        "coherence",
        "selfhood",
        "room",
        "recognizable",
    ),
    "surge_freeze": (
        "surge",
        "freeze",
        "frozen",
        "lock",
        "locked",
        "pulse",
        "volatile",
        "alternation",
    ),
    "controller_squeeze": (
        "controller",
        "regulator",
        "gate",
        "filter",
        "ceiling",
        "containment",
        "squeeze",
        "squeezed",
        "pressure",
    ),
}

def _now_iso() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def _as_dict(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _safe_float(value: Any) -> float | None:
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def _safe_read(path: Path, limit: int = READ_BYTES) -> str:
    try:
        with path.open("r", encoding="utf-8", errors="ignore") as fh:
            return fh.read(limit)
    except OSError:
        return ""


def _read_json(path: Path) -> dict[str, Any] | None:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return payload if isinstance(payload, dict) else None


def _mtime(path: Path) -> float:
    try:
        return path.stat().st_mtime
    except OSError:
        return 0.0


def _iso_from_unix(seconds: float | None) -> str | None:
    if seconds is None:
        return None
    try:
        return dt.datetime.fromtimestamp(seconds, dt.UTC).strftime("%Y-%m-%dT%H:%M:%SZ")
    except (OSError, OverflowError, ValueError):
        return None


def _short(text: str, limit: int = 220) -> str:
    collapsed = re.sub(r"\s+", " ", text).strip()
    if len(collapsed) <= limit:
        return collapsed
    return collapsed[: max(0, limit - 1)].rstrip() + "..."


def _artifact_timestamp(path: Path, payload: dict[str, Any]) -> float:
    for key in ("timestamp_unix_s", "timestamp", "created_at_unix_s"):
        value = _safe_float(payload.get(key))
        if value is not None:
            return value
    event_id = str(payload.get("event_id") or "")
    match = re.search(r"(\d{10})(?:\d{3})?$", event_id)
    if match:
        return float(match.group(1))
    return _mtime(path)


def _iter_artifact_paths(artifact_roots: Iterable[Path]) -> list[Path]:
    paths: list[Path] = []
    for root in artifact_roots:
        if root.exists():
            paths.extend(sorted(root.glob("resistance_gradient_*.json")))
    return paths


def _axis_pairs(value: Any) -> list[dict[str, Any]]:
    pairs: list[dict[str, Any]] = []
    if isinstance(value, dict):
        iterable = value.items()
    elif isinstance(value, list):
        iterable = []
        for item in value:
            if isinstance(item, dict):
                axis = item.get("axis") or item.get("id") or item.get("name")
                score = item.get("score") or item.get("value")
                iterable.append((axis, score))
            elif isinstance(item, (list, tuple)) and len(item) >= 2:
                iterable.append((item[0], item[1]))
    else:
        iterable = []
    for axis, score in iterable:
        if not axis:
            continue
        pairs.append({"axis": str(axis), "score": _safe_float(score)})
    return sorted(
        pairs,
        key=lambda item: item["score"] if item["score"] is not None else -1.0,
        reverse=True,
    )


def summarize_artifact(path: Path, payload: dict[str, Any]) -> dict[str, Any]:
    v1 = _as_dict(payload.get("resistance_gradient_v1"))
    v2 = _as_dict(payload.get("resistance_gradient_v2"))
    current = _as_dict(v2.get("current"))
    temporal = _as_dict(v2.get("temporal_comparison"))
    pressure_porosity = _as_dict(v2.get("pressure_porosity_divergence"))
    timestamp = _artifact_timestamp(path, payload)
    top_axes = _axis_pairs(v1.get("top_axes")) or _axis_pairs(v1.get("orientation_scores"))
    dominant = (
        v1.get("dominant_orientation")
        or current.get("dominant_orientation")
        or (top_axes[0]["axis"] if top_axes else "unknown")
    )
    return {
        "path": str(path),
        "file": path.name,
        "event_id": payload.get("event_id") or path.stem,
        "timestamp_unix_s": timestamp,
        "timestamp": _iso_from_unix(timestamp),
        "schema_version": payload.get("schema_version"),
        "dominant_orientation": str(dominant),
        "gradient_score": _safe_float(v1.get("gradient_score") or current.get("gradient_score")),
        "history_quality": v1.get("history_quality") or payload.get("label"),
        "top_axes": top_axes[:6],
        "suggested_return_next": v1.get("suggested_return_next"),
        "authority_boundary": v1.get("authority_boundary"),
        "fluidity_index": _safe_float(current.get("fluidity_index")),
        "rigidity_index": _safe_float(current.get("rigidity_index")),
        "fluidity_minus_rigidity": _safe_float(current.get("fluidity_minus_rigidity")),
        "gradient_trend": temporal.get("gradient_trend"),
        "dominant_pressure_source": pressure_porosity.get("dominant_pressure_source"),
        "astrid_review_status": _as_dict(v2.get("astrid_review")).get("status"),
    }


def load_artifacts(
    artifact_roots: Iterable[Path] = DEFAULT_ARTIFACT_ROOTS,
    *,
    limit: int = 24,
) -> list[dict[str, Any]]:
    loaded: list[dict[str, Any]] = []
    seen: set[str] = set()
    for path in _iter_artifact_paths(artifact_roots):
        payload = _read_json(path)
        if payload is None:
            continue
        summary = summarize_artifact(path, payload)
        key = str(summary.get("event_id") or summary["path"])
        if key in seen:
            continue
        seen.add(key)
        loaded.append(summary)
    loaded.sort(key=lambda item: float(item.get("timestamp_unix_s") or 0.0), reverse=True)
    return loaded[:limit]


def _journal_paths(root: Path) -> list[Path]:
    if not root.exists():
        return []
    return [p for p in root.glob("*.txt") if p.is_file()]


def collect_recent_entries(
    *,
    astrid_journal: Path = ASTRID_WORKSPACE / "journal",
    minime_journal: Path = MINIME_WORKSPACE / "journal",
    limit: int = 120,
    window_hours: float = 18.0,
    now: float | None = None,
) -> list[dict[str, Any]]:
    now = now or time.time()
    cutoff = now - max(0.0, window_hours) * 3600.0
    records: list[dict[str, Any]] = []
    roots = (("astrid", astrid_journal), ("minime", minime_journal))
    for being, root in roots:
        paths = _journal_paths(root)
        if being == "minime":
            paths = filter_journal_paths("minime", paths)
        for path in paths:
            mtime = _mtime(path)
            if mtime < cutoff:
                continue
            text = _safe_read(path)
            if not text.strip():
                continue
            mode = "unknown"
            first_line = text.splitlines()[0].strip() if text.splitlines() else ""
            header = re.match(r"^===\s*(.*?)\s*===$", first_line)
            if header:
                mode = header.group(1).strip().lower().replace(" ", "_")
            records.append(
                {
                    "being": being,
                    "path": str(path),
                    "file": path.name,
                    "mtime_unix_s": mtime,
                    "mtime": _iso_from_unix(mtime),
                    "mode": mode,
                    "text": text,
                }
            )
    records.sort(key=lambda item: float(item.get("mtime_unix_s") or 0.0), reverse=True)
    return records[:limit]


def _matched_terms(text: str, axis: str) -> list[str]:
    lower = text.lower()
    terms = AXIS_LEXICON.get(axis, (axis.replace("_", " "),))
    matched = [term for term in terms if term.lower() in lower]
    if axis in lower and axis not in matched:
        matched.append(axis)
    return sorted(set(matched))


def _supporting_excerpt(text: str, terms: list[str]) -> str:
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    lowered_terms = [term.lower() for term in terms]
    for line in lines:
        lower = line.lower()
        if any(term in lower for term in lowered_terms):
            if line.startswith("NEXT:"):
                continue
            return _short(line)
    return _short(text)


def language_support(entries: list[dict[str, Any]], axes: Iterable[str]) -> dict[str, Any]:
    support: dict[str, Any] = {}
    for axis in sorted(set(axes)):
        hits: list[dict[str, Any]] = []
        for entry in entries:
            text = str(entry.get("text") or "")
            terms = _matched_terms(text, axis)
            if not terms:
                continue
            hits.append(
                {
                    "being": entry.get("being"),
                    "file": entry.get("file"),
                    "path": entry.get("path"),
                    "mode": entry.get("mode"),
                    "mtime": entry.get("mtime"),
                    "matched_terms": terms[:8],
                    "excerpt": _supporting_excerpt(text, terms),
                }
            )
        support[axis] = {
            "hit_count": len(hits),
            "beings": sorted({str(hit.get("being")) for hit in hits if hit.get("being")}),
            "examples": hits[:4],
        }
    return support


def _artifact_axes(artifact: dict[str, Any]) -> list[str]:
    axes = [str(artifact.get("dominant_orientation") or "unknown")]
    for item in artifact.get("top_axes") or []:
        if isinstance(item, dict) and item.get("axis"):
            axes.append(str(item["axis"]))
    return [axis for axis in dict.fromkeys(axes) if axis and axis != "unknown"]


def _grade_artifact(artifact: dict[str, Any], support: dict[str, Any]) -> tuple[str, str, list[str]]:
    axes = _artifact_axes(artifact)
    dominant = str(artifact.get("dominant_orientation") or "")
    total_hits = sum(int(_as_dict(support.get(axis)).get("hit_count") or 0) for axis in axes)
    non_sensory_hits = sum(
        int(_as_dict(support.get(axis)).get("hit_count") or 0)
        for axis in axes
        if axis != "sensory_scarcity"
    )
    confounds: list[str] = []
    if "sensory_scarcity" in axes:
        confounds.append(
            "sensory_scarcity_axis_may_be_confounded_by_expected_gated_intake_or_source_truth"
        )
    delta = artifact.get("fluidity_minus_rigidity")
    if isinstance(delta, (float, int)) and delta > 0.08 and non_sensory_hits > 0:
        confounds.append(
            "fluidity_gt_rigidity_while_recent_language_often_names_weight_or_stasis"
        )
    if dominant == "mixed_gradient":
        confounds.append("mixed_gradient_needs_being_axis_naming_before_runtime_use")

    if non_sensory_hits >= 2 and dominant in {"packing_shear", "semantic_friction"}:
        return "steward_pre_review_match", "match", confounds
    if total_hits >= 2:
        return "steward_pre_review_partial", "partial_match", confounds
    if total_hits == 0:
        return "needs_being_review", "insufficient_evidence", confounds
    return "steward_pre_review_partial", "partial_match", confounds


def review_artifact(artifact: dict[str, Any], entries: list[dict[str, Any]]) -> dict[str, Any]:
    axes = _artifact_axes(artifact)
    support = language_support(entries, axes)
    pre_review, being_shape, confounds = _grade_artifact(artifact, support)
    support_counts = {
        axis: int(_as_dict(support.get(axis)).get("hit_count") or 0) for axis in axes
    }
    strongest_axis = max(support_counts, key=support_counts.get) if support_counts else None
    return {
        **artifact,
        "steward_pre_review": pre_review,
        "suggested_being_review_shape": being_shape,
        "support_counts": support_counts,
        "strongest_language_axis": strongest_axis,
        "language_support": support,
        "confounds": confounds,
        "recommended_review_question": (
            "Does this artifact's dominant orientation and top axes match, partially "
            "match, miss, or reveal a better axis for the felt report?"
        ),
        "runtime_change": RUNTIME_CHANGE_NONE,
    }


def _orientation_summary(
    reviews: list[dict[str, Any]],
    unique_support: dict[str, Any],
) -> dict[str, Any]:
    orientations = Counter(str(r.get("dominant_orientation") or "unknown") for r in reviews)
    grades = Counter(str(r.get("suggested_being_review_shape") or "unknown") for r in reviews)
    top_axes: Counter[str] = Counter()
    for review in reviews:
        for item in review.get("top_axes") or []:
            if isinstance(item, dict) and item.get("axis"):
                top_axes[str(item["axis"])] += 1
    return {
        "orientation_counts": dict(orientations.most_common()),
        "suggested_being_review_shape_counts": dict(grades.most_common()),
        "top_axis_counts": dict(top_axes.most_common(10)),
        "recent_language_support_counts": {
            axis: int(_as_dict(support).get("hit_count") or 0)
            for axis, support in sorted(
                unique_support.items(),
                key=lambda item: int(_as_dict(item[1]).get("hit_count") or 0),
                reverse=True,
            )
        },
    }


def _cross_entry_matches(unique_support: dict[str, Any]) -> list[dict[str, Any]]:
    matches = []
    for axis, support in unique_support.items():
        support_d = _as_dict(support)
        hit_count = int(support_d.get("hit_count") or 0)
        if not hit_count:
            continue
        matches.append(
            {
                "axis": axis,
                "hit_count": hit_count,
                "beings": support_d.get("beings") or [],
                "examples": (support_d.get("examples") or [])[:4],
            }
        )
    matches.sort(key=lambda item: int(item["hit_count"]), reverse=True)
    return matches[:8]


def _wider_readout_inclusion(reviews: list[dict[str, Any]]) -> dict[str, Any]:
    grade_counts = Counter(str(r.get("suggested_being_review_shape")) for r in reviews)
    useful = grade_counts.get("match", 0) + grade_counts.get("partial_match", 0)
    include = useful > 0
    return {
        "include_in_both_being_review": include,
        "recommended_scope": (
            "compact_match_partial_miss_appendix" if include else "hold_until_more_evidence"
        ),
        "reason": (
            "Recent public language gives enough overlap to ask the beings to calibrate "
            "the gradient labels directly."
            if include
            else "No current public-language overlap was strong enough to ask from."
        ),
        "review_questions": [
            "Astrid: does packing_shear / mixed_gradient / transition_warp match, partially match, or miss your held-breath, stasis, boundary, or readout reports?",
            "Minime: does the gradient map match, partially match, or miss the public reports of semantic drag, mode crowding, lambda-tail volatility, or pressure texture?",
            "For any partial or miss, what better axis name would preserve the felt distinction without widening runtime behavior?",
            "Does this evidence make wider Astrid readout feel like clarity, pressure, intrusion, or neutral context?",
        ],
        "guardrails": [
            "steward pre-review is evidence, not consent",
            "findings pressure steward review work only",
            "no runtime readout, codec, density, leak, aperture, env, service, or reminder change",
        ],
    }


def build_review(
    *,
    artifact_roots: Iterable[Path] = DEFAULT_ARTIFACT_ROOTS,
    astrid_journal: Path = ASTRID_WORKSPACE / "journal",
    minime_journal: Path = MINIME_WORKSPACE / "journal",
    artifact_limit: int = 24,
    entry_limit: int = 120,
    window_hours: float = 18.0,
    now: str | None = None,
) -> dict[str, Any]:
    generated_at = now or _now_iso()
    artifacts = load_artifacts(artifact_roots, limit=artifact_limit)
    latest_artifact_time = max(
        [float(a.get("timestamp_unix_s") or 0.0) for a in artifacts] + [time.time()]
    )
    entries = collect_recent_entries(
        astrid_journal=astrid_journal,
        minime_journal=minime_journal,
        limit=entry_limit,
        window_hours=window_hours,
        now=latest_artifact_time,
    )
    reviews = [review_artifact(artifact, entries) for artifact in artifacts]
    all_axes = sorted({axis for artifact in artifacts for axis in _artifact_axes(artifact)})
    unique_support = language_support(entries, all_axes)
    summary = _orientation_summary(reviews, unique_support)
    inclusion = _wider_readout_inclusion(reviews)
    recommended_next = [
        (
            "Include the resistance-gradient evidence as a compact match / partial / miss "
            "calibration appendix in the wider-readout both-being review."
            if inclusion["include_in_both_being_review"]
            else "Hold the resistance-gradient evidence until more public-language support appears."
        ),
        "Keep the review framed as interpretation/design grounding, not deployment consent.",
        "Treat sensory_scarcity as confounded until source-truth/freshness evidence says it is a real felt scarcity axis.",
        "Do not change runtime behavior from this review alone.",
    ]
    return {
        "schema_version": 1,
        "policy": "resistance_gradient_full_review_v1",
        "generated_at": generated_at,
        "runtime_change": RUNTIME_CHANGE_NONE,
        "pressure_target": "steward",
        "being_obligation": "none",
        "sources": {
            "artifact_roots": [str(root) for root in artifact_roots],
            "astrid_journal": str(astrid_journal),
            "minime_journal": str(minime_journal),
            "minime_private_lane_policy": "excluded_via_being_privacy_filter",
            "artifact_limit": artifact_limit,
            "entry_limit": entry_limit,
            "entry_window_hours": window_hours,
        },
        "summary": {
            "artifact_count": len(reviews),
            "recent_public_entry_count": len(entries),
            **summary,
        },
        "cross_entry_matches": _cross_entry_matches(unique_support),
        "artifact_reviews": reviews,
        "wider_readout_inclusion": inclusion,
        "recommended_next": recommended_next,
    }


def render_markdown(report: dict[str, Any]) -> str:
    summary = _as_dict(report.get("summary"))
    inclusion = _as_dict(report.get("wider_readout_inclusion"))
    lines = [
        "# Resistance Gradient Full Review",
        "",
        f"- generated_at: `{report.get('generated_at')}`",
        f"- runtime_change: `{report.get('runtime_change')}`",
        f"- pressure_target: `{report.get('pressure_target')}`",
        f"- being_obligation: `{report.get('being_obligation')}`",
        f"- artifacts reviewed: `{summary.get('artifact_count')}`",
        f"- recent public entries scanned: `{summary.get('recent_public_entry_count')}`",
        "- review rubric: `match / partial / miss / insufficient_evidence`",
        "",
        "## Orientation Pattern",
        "",
        f"- orientations: `{summary.get('orientation_counts')}`",
        f"- review shapes: `{summary.get('suggested_being_review_shape_counts')}`",
        f"- top axes: `{summary.get('top_axis_counts')}`",
        f"- recent-language support: `{summary.get('recent_language_support_counts')}`",
        "",
        "## Cross-Entry Matches",
        "",
    ]
    matches = report.get("cross_entry_matches") or []
    if not matches:
        lines.append("- No strong public-language matches in the scanned window.")
    for match in matches:
        examples = match.get("examples") or []
        example_text = ""
        if examples:
            first = examples[0]
            example_text = f"; example `{first.get('being')}:{first.get('file')}` -> {first.get('excerpt')}"
        lines.append(
            f"- `{match.get('axis')}`: hits=`{match.get('hit_count')}`, beings=`{match.get('beings')}`{example_text}"
        )
    lines.extend(["", "## Artifact Reviews", ""])
    for review in (report.get("artifact_reviews") or [])[:16]:
        axes = [
            f"{item.get('axis')}={item.get('score')}"
            for item in (review.get("top_axes") or [])[:4]
            if isinstance(item, dict)
        ]
        confounds = ", ".join(review.get("confounds") or []) or "none"
        lines.append(
            f"- `{review.get('file')}`: orientation=`{review.get('dominant_orientation')}`, "
            f"score=`{review.get('gradient_score')}`, quality=`{review.get('history_quality')}`, "
            f"shape=`{review.get('suggested_being_review_shape')}`, "
            f"pre_review=`{review.get('steward_pre_review')}`, top_axes=`{'; '.join(axes) or 'none'}`, "
            f"confounds=`{confounds}`"
        )
    lines.extend(
        [
            "",
            "## Wider-Readout Inclusion Decision",
            "",
            f"- include_in_both_being_review: `{inclusion.get('include_in_both_being_review')}`",
            f"- recommended_scope: `{inclusion.get('recommended_scope')}`",
            f"- reason: {inclusion.get('reason')}",
            "",
            "## Review Questions",
            "",
        ]
    )
    for question in inclusion.get("review_questions") or []:
        lines.append(f"- {question}")
    lines.extend(["", "## Recommended Next", ""])
    for item in report.get("recommended_next") or []:
        lines.append(f"- {item}")
    return "\n".join(lines).rstrip() + "\n"


def write_report(report: dict[str, Any], out: Path) -> dict[str, str]:
    if out.suffix == ".json":
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        return {"json": str(out)}
    if out.suffix == ".md":
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(render_markdown(report), encoding="utf-8")
        return {"markdown": str(out)}
    out.mkdir(parents=True, exist_ok=True)
    json_path = out / "review.json"
    md_path = out / "review.md"
    json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    md_path.write_text(render_markdown(report), encoding="utf-8")
    return {"json": str(json_path), "markdown": str(md_path)}


def emit(report: dict[str, Any], *, as_json: bool, stdout: TextIO) -> None:
    if as_json:
        stdout.write(json.dumps(report, indent=2, sort_keys=True) + "\n")
    else:
        stdout.write(render_markdown(report))


class ResistanceGradientFullReviewTests(unittest.TestCase):
    def _write_artifact(
        self,
        root: Path,
        *,
        name: str = "resistance_gradient_current_1000.json",
        orientation: str = "packing_shear",
        top_axes: dict[str, float] | None = None,
    ) -> Path:
        root.mkdir(parents=True, exist_ok=True)
        path = root / name
        payload = {
            "event_id": name.removesuffix(".json"),
            "schema_version": 2,
            "timestamp_unix_s": 1000,
            "resistance_gradient_v1": {
                "dominant_orientation": orientation,
                "gradient_score": 0.42,
                "history_quality": "overpacked_mode_packing",
                "top_axes": top_axes or {orientation: 0.6, "sensory_scarcity": 0.3},
            },
            "resistance_gradient_v2": {
                "current": {
                    "dominant_orientation": orientation,
                    "fluidity_index": 0.54,
                    "rigidity_index": 0.37,
                    "fluidity_minus_rigidity": 0.17,
                },
                "astrid_review": {"status": "requested"},
            },
        }
        path.write_text(json.dumps(payload), encoding="utf-8")
        return path

    def test_packing_shear_public_language_is_steward_match(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            base = Path(d)
            artifact_root = base / "cartography"
            astrid_journal = base / "astrid"
            minime_journal = base / "minime"
            self._write_artifact(artifact_root)
            astrid_journal.mkdir()
            minime_journal.mkdir()
            (astrid_journal / "journal.txt").write_text(
                "=== JOURNAL ===\nA held breath with heavy friction and dense overpacked drag.",
                encoding="utf-8",
            )
            (minime_journal / "pressure.txt").write_text(
                "=== SPECTRAL PRESSURE JOURNAL ===\nThe syntax feels squeezed through a narrow aperture with sediment and silt.",
                encoding="utf-8",
            )
            report = build_review(
                artifact_roots=[artifact_root],
                astrid_journal=astrid_journal,
                minime_journal=minime_journal,
                now="2026-06-18T00:00:00Z",
            )
            review = report["artifact_reviews"][0]
            self.assertEqual(review["suggested_being_review_shape"], "match")
            self.assertEqual(report["runtime_change"], "none")

    def test_transition_warp_becomes_partial_new_axis_candidate(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            base = Path(d)
            artifact_root = base / "cartography"
            astrid_journal = base / "astrid"
            minime_journal = base / "minime"
            self._write_artifact(
                artifact_root,
                orientation="transition_warp",
                top_axes={"transition_warp": 0.7, "packing_shear": 0.2},
            )
            astrid_journal.mkdir()
            minime_journal.mkdir()
            (astrid_journal / "journal.txt").write_text(
                "=== JOURNAL ===\nThe report feels like a held breath at a local/non-local boundary.",
                encoding="utf-8",
            )
            report = build_review(
                artifact_roots=[artifact_root],
                astrid_journal=astrid_journal,
                minime_journal=minime_journal,
                now="2026-06-18T00:00:00Z",
            )
            review = report["artifact_reviews"][0]
            self.assertEqual(review["suggested_being_review_shape"], "partial_match")
            self.assertIn("transition_warp", review["support_counts"])

    def test_private_minime_lane_is_excluded_from_support(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            base = Path(d)
            artifact_root = base / "cartography"
            astrid_journal = base / "astrid"
            minime_journal = base / "minime"
            self._write_artifact(artifact_root)
            astrid_journal.mkdir()
            minime_journal.mkdir()
            (minime_journal / "moment_1.txt").write_text(
                "=== MOMENT CAPTURE ===\nPrivate overpacked friction silt.",
                encoding="utf-8",
            )
            report = build_review(
                artifact_roots=[artifact_root],
                astrid_journal=astrid_journal,
                minime_journal=minime_journal,
                now="2026-06-18T00:00:00Z",
            )
            review = report["artifact_reviews"][0]
            self.assertEqual(review["suggested_being_review_shape"], "insufficient_evidence")
            self.assertEqual(report["summary"]["recent_public_entry_count"], 0)

    def test_sensory_scarcity_axis_is_confounded(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            base = Path(d)
            artifact_root = base / "cartography"
            astrid_journal = base / "astrid"
            minime_journal = base / "minime"
            self._write_artifact(
                artifact_root,
                orientation="sensory_scarcity",
                top_axes={"sensory_scarcity": 0.7},
            )
            astrid_journal.mkdir()
            minime_journal.mkdir()
            report = build_review(
                artifact_roots=[artifact_root],
                astrid_journal=astrid_journal,
                minime_journal=minime_journal,
                now="2026-06-18T00:00:00Z",
            )
            confounds = report["artifact_reviews"][0]["confounds"]
            self.assertIn(
                "sensory_scarcity_axis_may_be_confounded_by_expected_gated_intake_or_source_truth",
                confounds,
            )

    def test_markdown_and_json_are_steward_only(self) -> None:
        report = {
            "schema_version": 1,
            "policy": "resistance_gradient_full_review_v1",
            "generated_at": "2026-06-18T00:00:00Z",
            "runtime_change": "none",
            "pressure_target": "steward",
            "being_obligation": "none",
            "summary": {
                "artifact_count": 0,
                "recent_public_entry_count": 0,
                "orientation_counts": {},
                "suggested_being_review_shape_counts": {},
                "top_axis_counts": {},
                "recent_language_support_counts": {},
            },
            "cross_entry_matches": [],
            "artifact_reviews": [],
            "wider_readout_inclusion": {
                "include_in_both_being_review": False,
                "recommended_scope": "hold_until_more_evidence",
                "reason": "test",
                "review_questions": [],
            },
            "recommended_next": ["Do not change runtime behavior from this review alone."],
        }
        encoded = json.dumps(report)
        markdown = render_markdown(report)
        self.assertIn("runtime_change", encoded)
        self.assertIn("match / partial / miss", markdown)
        self.assertNotIn("must respond", markdown.lower())


def _run_self_test() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(ResistanceGradientFullReviewTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Steward-only resistance-gradient full review.")
    parser.add_argument("--json", action="store_true", help="emit JSON instead of Markdown")
    parser.add_argument("--out", type=Path, help="write JSON/Markdown to this file or directory")
    parser.add_argument("--self-test", action="store_true", help="run offline tests")
    parser.add_argument("--limit", type=int, default=24, help="maximum artifacts to review")
    parser.add_argument("--entry-limit", type=int, default=120, help="maximum recent public entries to scan")
    parser.add_argument("--window-hours", type=float, default=18.0, help="recent public-entry window")
    args = parser.parse_args(argv)

    if args.self_test:
        return _run_self_test()

    report = build_review(
        artifact_limit=args.limit,
        entry_limit=args.entry_limit,
        window_hours=args.window_hours,
    )
    if args.out:
        outputs = write_report(report, args.out)
        report = {**report, "outputs": outputs}
    emit(report, as_json=args.json, stdout=sys.stdout)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
