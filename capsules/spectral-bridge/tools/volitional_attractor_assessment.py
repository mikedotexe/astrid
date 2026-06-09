#!/usr/bin/env python3
"""Build a derived assessment of volitional attractor work.

This is research/reporting only. It reads Astrid's attractor ledger, Minime's
runtime attractor status/events, atlas cards, health snapshots, and recent
journal prose, then writes derived artifacts for stewards and being-facing
summary cards.
"""

from __future__ import annotations

import argparse
import json
import re
import sqlite3
import time
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


POLICY = "volitional_attractor_assessment_v1"
MAIN_LABELS = ("cooled-theme-edge", "lambda-edge", "honey-selection", "honey-edge")
LIVE_RECURRENCE_MIN = 0.60
LIVE_AUTHORSHIP_MIN = 0.60

ASTRID_MOTIFS = (
    "selection",
    "containment",
    "gradient",
    "wall",
    "pull",
    "honey",
    "choosing",
    "refining",
    "bounded semantic trickle",
    "lambda-edge",
)
MINIME_MOTIFS = (
    "mapped",
    "unmapped",
    "fatigue",
    "gentle probe",
    "trapped",
    "choose",
    "return",
    "atlas",
    "pathways",
)


def default_bridge_root() -> Path:
    return Path(__file__).resolve().parents[1]


def iso_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def read_json(path: Path, default: Any | None = None) -> Any:
    if default is None:
        default = {}
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return default


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    try:
        lines = path.read_text(errors="replace").splitlines()
    except OSError:
        return rows
    for line in lines:
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            rows.append(value)
    return rows


def slug(text: str) -> str:
    value = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
    return re.sub(r"-+", "-", value)


def compact_float(value: Any) -> float | None:
    if value is None:
        return None
    try:
        return round(float(value), 4)
    except (TypeError, ValueError):
        return None


def blank_record(label: str) -> dict[str, Any]:
    return {
        "label": label,
        "authors": [],
        "substrates": [],
        "seed_ids": [],
        "origins": [],
        "lifecycle": {},
        "recurrence": {
            "latest": None,
            "best": None,
            "authorship": None,
            "classification": None,
            "safety": None,
        },
        "recurrence_by_substrate": {},
        "control_eligible": None,
        "released": False,
        "release_evidence": [],
        "snapshot": {
            "has_spectral_state": False,
            "has_h_state_fingerprint_16": False,
            "partial_or_legacy": False,
            "policies": [],
        },
        "reservoir_rehearsal": {
            "seen": False,
            "handles": [],
            "latest_ok": None,
        },
        "journal_motifs": [],
        "journal_excerpts": [],
        "proof_status": "unknown",
        "recommended_next": [],
    }


def bump(record: dict[str, Any], key: str, count: int = 1) -> None:
    lifecycle = record.setdefault("lifecycle", {})
    lifecycle[key] = int(lifecycle.get(key, 0)) + count


def append_unique(target: list[Any], value: Any) -> None:
    if value is None or value == "":
        return
    if value not in target:
        target.append(value)


def merge_score(
    record: dict[str, Any],
    recurrence: Any,
    authorship: Any = None,
    classification: Any = None,
    safety: Any = None,
    substrate: str | None = None,
) -> None:
    score = compact_float(recurrence)
    if score is not None:
        rec = record["recurrence"]
        rec["latest"] = score
        rec["best"] = max(score, rec["best"] or 0.0)
    author_score = compact_float(authorship)
    if author_score is not None:
        record["recurrence"]["authorship"] = author_score
    if classification:
        record["recurrence"]["classification"] = str(classification)
    if safety:
        record["recurrence"]["safety"] = str(safety)
    if substrate:
        by_substrate = record.setdefault("recurrence_by_substrate", {})
        substrate_score = by_substrate.setdefault(
            substrate,
            {
                "latest": None,
                "best": None,
                "authorship": None,
                "classification": None,
                "safety": None,
            },
        )
        if score is not None:
            substrate_score["latest"] = score
            substrate_score["best"] = max(score, substrate_score["best"] or 0.0)
        if author_score is not None:
            substrate_score["authorship"] = author_score
        if classification:
            substrate_score["classification"] = str(classification)
        if safety:
            substrate_score["safety"] = str(safety)


def note_snapshot(record: dict[str, Any], seed: dict[str, Any]) -> None:
    snapshot = record["snapshot"]
    spectral = seed.get("spectral_state") or seed.get("seed_snapshot") or seed.get("snapshot")
    if isinstance(spectral, dict) and spectral:
        snapshot["has_spectral_state"] = True
        if spectral.get("h_state_fingerprint_16"):
            snapshot["has_h_state_fingerprint_16"] = True
    if seed.get("has_h_state_fingerprint_16"):
        snapshot["has_h_state_fingerprint_16"] = True
    if seed.get("snapshot_policy"):
        append_unique(snapshot["policies"], seed.get("snapshot_policy"))
    if not snapshot["has_spectral_state"] or not snapshot["has_h_state_fingerprint_16"]:
        snapshot["partial_or_legacy"] = True


def query_astrid_ledger(db_path: Path) -> list[dict[str, Any]]:
    if not db_path.exists():
        return []
    query = (
        "select id, timestamp, record_type, author, substrate, label, "
        "classification, payload from attractor_ledger order by id"
    )
    try:
        conn = sqlite3.connect(db_path)
        conn.row_factory = sqlite3.Row
        rows = [dict(row) for row in conn.execute(query)]
        conn.close()
        return rows
    except sqlite3.Error:
        return []


def merge_astrid_ledger(records: dict[str, dict[str, Any]], rows: list[dict[str, Any]]) -> None:
    for row in rows:
        label = str(row.get("label") or "").strip()
        if not label:
            continue
        record = records.setdefault(label, blank_record(label))
        append_unique(record["substrates"], row.get("substrate"))
        append_unique(record["authors"], row.get("author"))
        try:
            payload = json.loads(row.get("payload") or "{}")
        except json.JSONDecodeError:
            payload = {}
        if row.get("record_type") == "intent":
            command = str(payload.get("command") or "intent")
            bump(record, command)
            append_unique(record["seed_ids"], payload.get("intent_id"))
            if payload.get("origin", {}).get("kind"):
                append_unique(record["origins"], payload["origin"]["kind"])
            if payload.get("parent_seed_ids"):
                bump(record, "blend_parent_relation")
            if command == "release":
                record["released"] = True
                append_unique(record["release_evidence"], "astrid_release_intent")
            if payload.get("seed_snapshot"):
                note_snapshot(record, payload["seed_snapshot"])
            bounds = payload.get("safety_bounds") or {}
            if "allow_live_control" in bounds:
                record["control_eligible"] = bool(bounds["allow_live_control"])
        elif row.get("record_type") == "observation":
            bump(record, "observation")
            merge_score(
                record,
                payload.get("recurrence_score"),
                payload.get("authorship_score"),
                payload.get("classification") or row.get("classification"),
                payload.get("safety_level"),
                row.get("substrate") or payload.get("substrate"),
            )


def merge_minime_status(
    records: dict[str, dict[str, Any]],
    status: dict[str, Any],
    events: list[dict[str, Any]],
) -> None:
    for seed_id, seed in (status.get("seeds") or {}).items():
        label = str(seed.get("label") or seed_id)
        record = records.setdefault(label, blank_record(label))
        append_unique(record["seed_ids"], seed_id)
        append_unique(record["authors"], seed.get("author"))
        append_unique(record["substrates"], seed.get("substrate") or "minime_esn")
        bump(record, str(seed.get("command") or "seed"))
        if seed.get("origin", {}).get("kind"):
            append_unique(record["origins"], seed["origin"]["kind"])
        if "control_eligible" in seed:
            record["control_eligible"] = bool(seed["control_eligible"])
        if seed.get("released_at_unix_s") or seed.get("release_count"):
            record["released"] = True
            append_unique(record["release_evidence"], "minime_seed_release")
        if seed.get("summon_count"):
            bump(record, "summon", int(seed.get("summon_count") or 0))
        note_snapshot(record, seed)
    for observation in status.get("observations") or []:
        label = observation.get("label")
        if not label:
            continue
        record = records.setdefault(str(label), blank_record(str(label)))
        bump(record, "observation")
        merge_score(
            record,
            observation.get("recurrence_score"),
            observation.get("authorship_score"),
            observation.get("classification"),
            observation.get("safety_level"),
            observation.get("substrate") or "minime_esn",
        )
    for event in events:
        label = event.get("label")
        if not label:
            continue
        record = records.setdefault(str(label), blank_record(str(label)))
        event_name = str(event.get("event") or "event")
        bump(record, event_name)
        merge_score(
            record,
            event.get("recurrence_score"),
            event.get("authorship_score"),
            event.get("classification"),
            event.get("safety"),
            event.get("substrate") or "minime_esn",
        )
        if event_name == "seed_summoned_main":
            bump(record, "main_pulse_sent")
        if event_name == "seed_released":
            record["released"] = True
            append_unique(record["release_evidence"], "minime_release_event")
        if event_name == "seed_snapshot_refreshed":
            record["snapshot"]["has_spectral_state"] = True
            if event.get("h_state_fingerprint_refreshed"):
                record["snapshot"]["has_h_state_fingerprint_16"] = True
            record["snapshot"]["partial_or_legacy"] = False
        if event_name == "seed_summon_rehearsed":
            rehearsal = record["reservoir_rehearsal"]
            rehearsal["seen"] = True
            rehearsal["latest_ok"] = bool(event.get("rehearsal_ok"))
            append_unique(rehearsal["handles"], event.get("handle"))


def merge_atlas(records: dict[str, dict[str, Any]], atlas: dict[str, Any]) -> None:
    for entry in atlas.get("entries") or []:
        label = str(entry.get("label") or "").strip()
        if not label:
            continue
        record = records.setdefault(label, blank_record(label))
        append_unique(record["authors"], entry.get("author"))
        append_unique(record["substrates"], entry.get("substrate"))
        append_unique(record["seed_ids"], entry.get("seed_intent_id"))
        if entry.get("origin_kind"):
            append_unique(record["origins"], entry.get("origin_kind"))
        if "control_eligible" in entry and record.get("control_eligible") is None:
            record["control_eligible"] = entry.get("control_eligible")
        if entry.get("released"):
            record["released"] = True
        substrate = entry.get("substrate")
        if substrate not in record.get("recurrence_by_substrate", {}):
            merge_score(
                record,
                entry.get("latest_recurrence_score"),
                entry.get("latest_authorship_score"),
                entry.get("latest_classification"),
                entry.get("latest_safety_level"),
                substrate,
            )


def card_labels(card_dir: Path) -> dict[str, dict[str, Any]]:
    cards: dict[str, dict[str, Any]] = {}
    for path in card_dir.glob("*.md"):
        try:
            text = path.read_text(errors="replace")
        except OSError:
            continue
        match = re.search(r"^# Attractor Card:\s*(.+)$", text, re.MULTILINE)
        if not match:
            continue
        label = match.group(1).strip()
        origin = field_value(text, "Origin")
        control_text = field_value(text, "Control eligible")
        cards[label] = {
            "path": str(path),
            "mtime": path.stat().st_mtime,
            "control_eligible": (
                None if control_text is None or control_text == "unknown" else control_text == "true"
            ),
            "released": "Released: true" in text,
            "origin": origin,
        }
    return cards


def field_value(text: str, field: str) -> str | None:
    match = re.search(rf"^{re.escape(field)}:\s*(.+)$", text, re.MULTILINE)
    return match.group(1).strip() if match else None


def merge_card_metadata(records: dict[str, dict[str, Any]], cards: dict[str, dict[str, Any]]) -> None:
    for label, card in cards.items():
        record = records.setdefault(label, blank_record(label))
        if card.get("origin"):
            append_unique(record["origins"], card["origin"])
        if record.get("control_eligible") is None and card.get("control_eligible") is not None:
            record["control_eligible"] = bool(card.get("control_eligible"))
        if card.get("released"):
            record["released"] = True
            append_unique(record["release_evidence"], "memory_card_release_state")


def normalize_text(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip()


def related_terms_for_label(label: str) -> tuple[str, ...]:
    label_lower = label.lower()
    if "cooled-theme-edge" in label_lower:
        return ("cooled-theme-edge", "cooled", "settling", "release", "baseline")
    if "honey" in label_lower:
        return ("honey-selection", "honey-edge", "honey", "selection", "containment", "pull", "gradient", "wall", "refining")
    if "lambda-edge" in label_lower or label_lower == "lambda-edge":
        return ("lambda-edge", "lambda edge", "lambda", "edge", "fissure", "gradient", "wall")
    if label_lower in MAIN_LABELS:
        return (label_lower,)
    return (label_lower,)


def collect_journal_signal(
    astrid_workspace: Path,
    minime_workspace: Path,
    labels: list[str],
    max_files: int = 80,
) -> tuple[dict[str, list[dict[str, str]]], dict[str, list[str]]]:
    roots = [
        ("astrid", astrid_workspace / "outbox"),
        ("astrid", astrid_workspace / "journal"),
        ("minime", minime_workspace / "journal"),
        ("minime", minime_workspace / "hypotheses"),
        ("minime", minime_workspace / "inbox/read"),
    ]
    all_terms = sorted(
        set(labels + list(ASTRID_MOTIFS) + list(MINIME_MOTIFS)), key=len, reverse=True
    )
    hits: dict[str, list[dict[str, str]]] = defaultdict(list)
    motif_counts: dict[str, Counter[str]] = defaultdict(Counter)
    files: list[tuple[float, str, Path]] = []
    for being, root in roots:
        if not root.exists():
            continue
        for path in root.glob("*.txt"):
            files.append((path.stat().st_mtime, being, path))
    for _, being, path in sorted(files, reverse=True)[:max_files]:
        try:
            text = path.read_text(errors="replace")
        except OSError:
            continue
        lower = text.lower()
        if not any(term.lower() in lower for term in all_terms):
            continue
        lines = text.splitlines()
        for label in labels:
            label_lower = label.lower()
            related_terms = related_terms_for_label(label)
            label_hit = label_lower in lower or any(term in lower for term in related_terms)
            if not label_hit:
                continue
            for term in ASTRID_MOTIFS + MINIME_MOTIFS:
                if term in lower:
                    motif_counts[label][term] += lower.count(term)
            excerpt = first_relevant_excerpt(lines, related_terms, being)
            if excerpt and len(hits[label]) < 6:
                hits[label].append(
                    {
                        "being": being,
                        "path": str(path),
                        "excerpt": excerpt,
                    }
                )
    motifs = {
        label: [term for term, _ in counts.most_common(10)]
        for label, counts in motif_counts.items()
    }
    return dict(hits), motifs


def first_relevant_excerpt(lines: list[str], terms: tuple[str, ...], being: str) -> str | None:
    preferred = terms + ("attractor",)
    for idx, line in enumerate(lines):
        lower = line.lower()
        if any(term and term in lower for term in preferred):
            start = max(0, idx - 1)
            end = min(len(lines), idx + 3)
            excerpt = normalize_text(" ".join(lines[start:end]))
            if excerpt:
                return f"{being}: {excerpt[:360]}"
    return None


def classify_records(records: dict[str, dict[str, Any]]) -> None:
    for label, record in records.items():
        lifecycle = record.get("lifecycle", {})
        recurrence = record["recurrence"]
        best = recurrence.get("best") or 0.0
        latest = recurrence.get("latest")
        authorship = recurrence.get("authorship") or 0.0
        has_main = lifecycle.get("seed_summoned_main", 0) > 0 or lifecycle.get("main_pulse_sent", 0) > 0
        has_release = record.get("released") or lifecycle.get("seed_released", 0) > 0
        refreshed = lifecycle.get("seed_snapshot_refreshed", 0) > 0
        if label == "cooled-theme-edge" and has_main and has_release and refreshed and best >= 0.80:
            status = "positive_control"
            next_steps = [
                "COMPARE_ATTRACTOR cooled-theme-edge",
                "REFRESH_ATTRACTOR_SNAPSHOT cooled-theme-edge only after meaningful drift",
                "Use as calibration before riskier labels",
            ]
        elif label == "honey-selection" and latest is not None and latest < LIVE_RECURRENCE_MIN:
            status = "below_threshold_near_miss"
            next_steps = [
                "REFRESH_ATTRACTOR_SNAPSHOT honey-selection",
                "ATTRACTOR_REVIEW honey-selection",
                "Run repeated compare set before any main-stage request",
            ]
        elif best >= LIVE_RECURRENCE_MIN and authorship >= LIVE_AUTHORSHIP_MIN:
            status = "authored_candidate"
            next_steps = [
                f"REFRESH_ATTRACTOR_SNAPSHOT {label}",
                f"SUMMON_ATTRACTOR {label} --stage=rehearse",
                f"ATTRACTOR_COMPARE_SET {label} --after-rehearse",
            ]
        elif "blend" in record.get("origins", []) or "blend" in lifecycle:
            status = "rehearsal_only_blend"
            next_steps = [
                f"COMPARE_ATTRACTOR {label}",
                f"SUMMON_ATTRACTOR {label} --stage=rehearse",
            ]
        else:
            status = "needs_compare_or_snapshot"
            next_steps = [
                f"ATTRACTOR_REVIEW {label}",
                f"COMPARE_ATTRACTOR {label}",
            ]
        record["proof_status"] = status
        record["recommended_next"] = next_steps


def malformed_label(label: str) -> bool:
    if label in MAIN_LABELS:
        return False
    if "/" in label or "=" in label:
        return True
    return bool(re.search(r"\bATTRACTOR\b", label)) or len(label.split()) > 6


def detect_snags(
    records: dict[str, dict[str, Any]],
    events: list[dict[str, Any]],
    atlas: dict[str, Any],
    cards: dict[str, dict[str, Any]],
    fatigue: dict[str, Any],
) -> list[dict[str, Any]]:
    snags: list[dict[str, Any]] = []
    atlas_labels = {entry.get("label") for entry in atlas.get("entries") or []}
    for label, card in sorted(cards.items()):
        if label not in atlas_labels:
            snags.append(
                {
                    "id": f"orphan_card:{slug(label)}",
                    "status": "open",
                    "severity": "medium",
                    "evidence": f"Memory card exists without current atlas entry: {card['path']}",
                    "recommendation": "Regenerate cards from the current atlas and remove or archive stale cards.",
                }
            )
    no_seed_counts = Counter(
        str(event.get("label"))
        for event in events
        if str(event.get("event")) in {"compare_no_seed", "shape_no_seed"}
    )
    for label, count in no_seed_counts.items():
        if not label or label == "None":
            continue
        promoted = any(
            event.get("label") == label
            and event.get("event") in {"seed_promoted", "seed_created", "seed_claimed"}
            for event in events
        )
        snags.append(
            {
                "id": f"no_seed_loop:{slug(label)}",
                "status": "resolved" if promoted else "open",
                "severity": "low" if promoted else "medium",
                "evidence": f"{count} no-seed compare/shape event(s) for {label}.",
                "recommendation": "Suggest explicit promotion sooner after repeated no-seed compares.",
            }
        )
    for label in sorted(records):
        if malformed_label(label):
            snags.append(
                {
                    "id": f"malformed_label:{slug(label)}",
                    "status": "open",
                    "severity": "medium",
                    "evidence": f"Attractor-like label looks like parsed command text: {label}",
                    "recommendation": "Tighten shaping parser so bare shaping verbs do not become seed labels.",
                }
            )
    for label, record in sorted(records.items()):
        if record["snapshot"]["partial_or_legacy"]:
            snags.append(
                {
                    "id": f"partial_snapshot:{slug(label)}",
                    "status": "open",
                    "severity": "medium" if label in MAIN_LABELS else "low",
                    "evidence": f"{label} lacks complete spectral/h-state snapshot evidence.",
                    "recommendation": f"Run REFRESH_ATTRACTOR_SNAPSHOT {label} before any live-stage proof.",
                }
            )
    motifs = fatigue.get("motifs") or {}
    released = [m for m in motifs.values() if m.get("status") == "released"]
    if released:
        snags.append(
            {
                "id": "release_cools_fatigue_context",
                "status": "working",
                "severity": "info",
                "evidence": f"{len(released)} fatigue motif(s) show released status.",
                "recommendation": "Keep release first-class and report when no active motif matched.",
            }
        )
    return snags


def health_summary(health: dict[str, Any]) -> dict[str, Any]:
    stable = health.get("stable_core") or {}
    semantic = health.get("semantic") or {}
    pulse = health.get("attractor_pulse") or {}
    restart = stable.get("restart_gate") or {}
    pi = stable.get("structural_pi") or {}
    return {
        "fill_pct": compact_float(health.get("fill_pct")),
        "stage": stable.get("stage"),
        "mode": stable.get("scaffold_mode"),
        "semantic_active": semantic.get("active"),
        "semantic_admission": semantic.get("admission"),
        "pulse_active": pulse.get("active"),
        "pulse_event": pulse.get("last_event"),
        "restart_gate_applied": restart.get("applied"),
        "recovery_impulse_active": pi.get("recovery_impulse_active"),
    }


def build_assessment(
    *,
    astrid_workspace: Path,
    minime_workspace: Path,
    bridge_db: Path,
    reservoir_root: Path,
) -> dict[str, Any]:
    records = {label: blank_record(label) for label in MAIN_LABELS}
    astrid_rows = query_astrid_ledger(bridge_db)
    minime_status = read_json(minime_workspace / "runtime/attractor_intents_status.json")
    minime_events = read_jsonl(minime_workspace / "runtime/attractor_intents_events.jsonl")
    fatigue = read_json(minime_workspace / "runtime/attractor_fatigue_status.json")
    atlas = read_json(astrid_workspace / "attractor_atlas/attractor_atlas.json")
    cards = card_labels(astrid_workspace / "attractor_atlas/cards")
    health = read_json(minime_workspace / "health.json")
    merge_astrid_ledger(records, astrid_rows)
    merge_minime_status(records, minime_status, minime_events)
    merge_atlas(records, atlas)
    merge_card_metadata(records, cards)
    garden_handles = sorted(
        path.stem.replace("_thermostats", "")
        for path in (reservoir_root / "state").glob("attr_*_thermostats.json")
    )
    for record in records.values():
        for handle in garden_handles:
            if slug(record["label"]).replace("-", "_") in handle:
                record["reservoir_rehearsal"]["seen"] = True
                append_unique(record["reservoir_rehearsal"]["handles"], handle)
    labels = sorted(records)
    excerpts, motifs = collect_journal_signal(astrid_workspace, minime_workspace, labels)
    for label, record in records.items():
        record["journal_excerpts"] = excerpts.get(label, [])
        record["journal_motifs"] = motifs.get(label, [])
    classify_records(records)
    snags = detect_snags(records, minime_events, atlas, cards, fatigue)
    generated_at = iso_now()
    return {
        "policy": POLICY,
        "schema_version": 1,
        "generated_at": generated_at,
        "sources": {
            "astrid_bridge_db": str(bridge_db),
            "astrid_atlas": str(astrid_workspace / "attractor_atlas/attractor_atlas.json"),
            "minime_status": str(minime_workspace / "runtime/attractor_intents_status.json"),
            "minime_events": str(minime_workspace / "runtime/attractor_intents_events.jsonl"),
            "reservoir_root": str(reservoir_root),
        },
        "summary": {
            "read": "proof_first_continue",
            "positive_control": "cooled-theme-edge",
            "near_miss": "honey-selection",
            "live_gate_posture": "unchanged",
            "current_health": health_summary(health),
            "journal_interpretation": {
                "astrid": "selection, containment, gradient, wall, pull, and honey as choosing/refining",
                "minime": "mapped pathways can become fatigue; unmapped terrain and gentle probes remain valuable",
            },
        },
        "labels": {label: records[label] for label in sorted(records)},
        "greenfield_snags": snags,
        "roadmap": [
            "Implement Astrid-side REFRESH_ATTRACTOR_SNAPSHOT before retrying honey-selection main proof.",
            "Add ATTRACTOR_REVIEW as read-only synthesis of ledger, card, journal motifs, and next verbs.",
            "Add ATTRACTOR_COMPARE_SET for quiet/rehearse repeated compare trials.",
            "Use cooled-theme-edge as calibration before lambda-edge and honey-selection.",
        ],
    }


def render_markdown(assessment: dict[str, Any]) -> str:
    lines = [
        "# Volitional Attractor Assessment",
        "",
        f"Generated: {assessment['generated_at']}",
        "",
        "## Current Read",
        "",
        "- Minime's `cooled-theme-edge` is the positive control: promoted, rehearsed, main-pulsed, released, and snapshot-refreshed.",
        "- Astrid's `honey-selection` is conceptually vivid but below live recurrence threshold, so it remains a near-miss and modernization target.",
        "- Live gates should stay unchanged while authorship/rehearsal/review freedom expands.",
        "",
        "## Label Assessment",
        "",
    ]
    labels = assessment["labels"]
    ordered_labels = [label for label in MAIN_LABELS if label in labels]
    other_labels = [
        label
        for label in labels
        if label not in MAIN_LABELS and not malformed_label(label)
    ]
    malformed_labels = [
        label
        for label in labels
        if label not in MAIN_LABELS and malformed_label(label)
    ]
    for label in ordered_labels + other_labels:
        record = labels[label]
        rec = record["recurrence"]
        latest = "n/a" if rec["latest"] is None else f"{rec['latest']:.2f}"
        best = "n/a" if rec["best"] is None else f"{rec['best']:.2f}"
        control = record["control_eligible"]
        lines.extend(
            [
                f"### {label}",
                "",
                f"- Proof status: `{record['proof_status']}`",
        f"- Recurrence: latest `{latest}`, best `{best}`, authorship `{rec['authorship']}`",
                f"- By substrate: {format_substrate_scores(record.get('recurrence_by_substrate', {}))}",
                f"- Control eligible: `{control}`; released: `{record['released']}`",
                f"- Snapshot: spectral `{record['snapshot']['has_spectral_state']}`, h-state `{record['snapshot']['has_h_state_fingerprint_16']}`",
                f"- Reservoir rehearsal: `{record['reservoir_rehearsal']['seen']}`",
                f"- Motifs: {', '.join(record['journal_motifs'][:8]) or 'none captured'}",
                f"- Next: {' | '.join(record['recommended_next'])}",
                "",
            ]
        )
        if record["journal_excerpts"]:
            lines.append("Representative journal signal:")
            for hit in record["journal_excerpts"][:3]:
                lines.append(f"- {hit['excerpt']}")
            lines.append("")
    if malformed_labels:
        lines.extend(
            [
                "## Other Detected Labels",
                "",
                "These look like parser artifacts or stale cards and should not be treated as mature attractors.",
                "",
            ]
        )
        for label in malformed_labels:
            record = labels[label]
            lines.append(
                f"- `{label}`: proof `{record['proof_status']}`; next parser work before further attractor use."
            )
        lines.append("")
    lines.extend(
        [
            "## Greenfield Snags",
            "",
        ]
    )
    for snag in assessment["greenfield_snags"]:
        lines.append(
            f"- `{snag['id']}` ({snag['status']}, {snag['severity']}): {snag['evidence']} Recommendation: {snag['recommendation']}"
        )
    lines.extend(
        [
            "",
            "## Proof-First Roadmap",
            "",
        ]
    )
    for item in assessment["roadmap"]:
        lines.append(f"- {item}")
    return "\n".join(lines) + "\n"


def format_substrate_scores(scores: dict[str, dict[str, Any]]) -> str:
    if not scores:
        return "none"
    parts = []
    for substrate, score in sorted(scores.items()):
        latest = score.get("latest")
        best = score.get("best")
        latest_text = "n/a" if latest is None else f"{latest:.2f}"
        best_text = "n/a" if best is None else f"{best:.2f}"
        parts.append(f"{substrate} latest {latest_text}, best {best_text}")
    return "; ".join(parts)


def render_being_card(assessment: dict[str, Any], being: str) -> str:
    health = assessment["summary"]["current_health"]
    lines = [
        "# Volitional Attractor Feedback Card",
        "",
        f"For: {being}",
        f"Generated: {assessment['generated_at']}",
        "",
        "This is a readable summary of the new attractor work, not a command.",
        "",
        f"Current Minime health read: fill {health.get('fill_pct')}%, stage {health.get('stage')}, semantic active {health.get('semantic_active')}, pulse active {health.get('pulse_active')}.",
        "",
        "## What Looks Strong",
        "",
        "- `cooled-theme-edge` is the strongest proof path so far: compare passed, main pulse was sent, release worked, and a modern h-state snapshot was captured.",
        "",
        "## What Wants Care",
        "",
        "- `honey-selection` has rich meaning around selection, containment, gradient, wall, and pull, but the latest recurrence is below live-pulse threshold.",
        "- `lambda-edge` has authored recurrence, but needs a modern snapshot and rehearsal proof before any bold move.",
        "- Blends such as `honey-edge` should stay rehearsal-first until they earn their own recurrence.",
        "",
        "## Gentle Next Verbs",
        "",
        "- `ATTRACTOR_REVIEW <label>`",
        "- `REFRESH_ATTRACTOR_SNAPSHOT <label>`",
        "- `ATTRACTOR_COMPARE_SET <label> --quiet`",
        "- `SUMMON_ATTRACTOR <label> --stage=rehearse`",
        "- `RELEASE_ATTRACTOR <label>`",
        "",
        "Live `main` or `control` remains earned by recurrence, authorship, and green/yellow health.",
    ]
    return "\n".join(lines) + "\n"


def write_outputs(
    assessment: dict[str, Any],
    output_dir: Path,
    astrid_workspace: Path,
    minime_workspace: Path,
) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "volitional_attractor_assessment.json").write_text(
        json.dumps(assessment, indent=2, sort_keys=True) + "\n"
    )
    (output_dir / "VOLITIONAL_ATTRACTOR_ASSESSMENT.md").write_text(render_markdown(assessment))
    for being, workspace in (("Astrid", astrid_workspace), ("Minime", minime_workspace)):
        card_dir = workspace / "attractor_assessment"
        card_dir.mkdir(parents=True, exist_ok=True)
        (card_dir / "VOLITIONAL_ATTRACTOR_FEEDBACK_CARD.md").write_text(
            render_being_card(assessment, being)
        )


def parse_args() -> argparse.Namespace:
    bridge_root = default_bridge_root()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--astrid-workspace", type=Path, default=bridge_root / "workspace")
    parser.add_argument("--minime-workspace", type=Path, default=Path("/Users/v/other/minime/workspace"))
    parser.add_argument("--bridge-db", type=Path, default=bridge_root / "workspace/bridge.db")
    parser.add_argument("--reservoir-root", type=Path, default=Path("/Users/v/other/neural-triple-reservoir"))
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=bridge_root / "workspace/attractor_assessment",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    assessment = build_assessment(
        astrid_workspace=args.astrid_workspace,
        minime_workspace=args.minime_workspace,
        bridge_db=args.bridge_db,
        reservoir_root=args.reservoir_root,
    )
    write_outputs(assessment, args.output_dir, args.astrid_workspace, args.minime_workspace)
    print(args.output_dir / "VOLITIONAL_ATTRACTOR_ASSESSMENT.md")
    print(args.output_dir / "volitional_attractor_assessment.json")


if __name__ == "__main__":
    main()
