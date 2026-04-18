#!/usr/bin/env python3
"""
Build a steward-first BTSP latency table from recent Minime journals.

This diagnostic asks a simple question: once a cue or cue-pair appears, how
long until the next tightening / recovery / mixed / softened-only outcome is
actually observed in the journal stream, if one appears at all?
"""

from __future__ import annotations

import argparse
import json
import statistics
from collections import Counter, defaultdict
from dataclasses import dataclass
from datetime import datetime
from itertools import combinations
from pathlib import Path
from typing import Any

from btsp_onset_truth_table import (
    classify_outcome,
    detect_cues,
    first_timestamp_line,
    journal_files,
    make_excerpt,
)


OUTCOME_ORDER = [
    "tightening",
    "recovery",
    "mixed",
    "softened_only",
    "none_within_window",
]


@dataclass
class JournalEvent:
    index: int
    path: Path
    timestamp: str
    dt: datetime
    outcome: str
    cues: list[str]
    cue_tiers: dict[str, str]
    matched_aliases: dict[str, dict[str, list[str]]]
    excerpt: str


@dataclass
class SignalObservation:
    signal_type: str
    signal: str
    tier: str
    observed_at: str
    observed_file: str
    resolved_outcome: str
    latency_minutes: float | None
    resolution_mode: str
    resolved_at: str | None
    resolved_file: str | None
    excerpt: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--minime-root",
        default="/Users/v/other/minime",
        help="Path to the sibling minime checkout.",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=400,
        help="Maximum number of recent journal files to inspect.",
    )
    parser.add_argument(
        "--max-minutes",
        type=float,
        default=120.0,
        help="Maximum lookahead window in minutes for a later outcome.",
    )
    parser.add_argument(
        "--max-events-ahead",
        type=int,
        default=40,
        help="Maximum number of subsequent journal events to inspect.",
    )
    parser.add_argument(
        "--output-dir",
        default="/Users/v/other/astrid/capsules/consciousness-bridge/workspace/diagnostics/btsp_latency_table",
        help="Directory for summary.json and report.md.",
    )
    return parser.parse_args()


def parse_timestamp(text: str, path: Path) -> datetime:
    raw = first_timestamp_line(text)
    try:
        return datetime.fromisoformat(raw)
    except ValueError:
        return datetime.fromtimestamp(path.stat().st_mtime)


def load_events(minime_root: Path, limit: int) -> list[JournalEvent]:
    paths = list(reversed(journal_files(minime_root, limit)))
    events: list[JournalEvent] = []
    for idx, path in enumerate(paths):
        try:
            text = path.read_text()
        except Exception:
            continue
        cues, matched_aliases, cue_tiers = detect_cues(text)
        events.append(
            JournalEvent(
                index=idx,
                path=path,
                timestamp=first_timestamp_line(text),
                dt=parse_timestamp(text, path),
                outcome=classify_outcome(text),
                cues=cues,
                cue_tiers=cue_tiers,
                matched_aliases=matched_aliases,
                excerpt=make_excerpt(text),
            )
        )
    return events


def resolve_outcome(
    events: list[JournalEvent],
    event_index: int,
    *,
    max_minutes: float,
    max_events_ahead: int,
) -> tuple[str, float | None, str, str | None, str | None]:
    source = events[event_index]
    end_index = min(len(events), event_index + max_events_ahead + 1)
    for later in events[event_index:end_index]:
        if later.outcome == "unknown":
            continue
        latency_minutes = max(
            0.0,
            (later.dt - source.dt).total_seconds() / 60.0,
        )
        if latency_minutes > max_minutes:
            break
        resolution_mode = "same_entry" if later.index == source.index else "later_entry"
        return (
            later.outcome,
            round(latency_minutes, 2),
            resolution_mode,
            later.timestamp,
            str(later.path),
        )
    return ("none_within_window", None, "none_within_window", None, None)


def pair_tier(event: JournalEvent, cue_a: str, cue_b: str) -> str:
    return "+".join(sorted([event.cue_tiers.get(cue_a, "weak"), event.cue_tiers.get(cue_b, "weak")]))


def collect_observations(
    events: list[JournalEvent],
    *,
    max_minutes: float,
    max_events_ahead: int,
) -> list[SignalObservation]:
    observations: list[SignalObservation] = []
    for event in events:
        if not event.cues:
            continue
        for cue in sorted(event.cues):
            outcome, latency, mode, resolved_at, resolved_file = resolve_outcome(
                events,
                event.index,
                max_minutes=max_minutes,
                max_events_ahead=max_events_ahead,
            )
            observations.append(
                SignalObservation(
                    signal_type="cue",
                    signal=cue,
                    tier=event.cue_tiers.get(cue, "weak"),
                    observed_at=event.timestamp,
                    observed_file=str(event.path),
                    resolved_outcome=outcome,
                    latency_minutes=latency,
                    resolution_mode=mode,
                    resolved_at=resolved_at,
                    resolved_file=resolved_file,
                    excerpt=event.excerpt,
                )
            )
        for cue_a, cue_b in combinations(sorted(event.cues), 2):
            outcome, latency, mode, resolved_at, resolved_file = resolve_outcome(
                events,
                event.index,
                max_minutes=max_minutes,
                max_events_ahead=max_events_ahead,
            )
            observations.append(
                SignalObservation(
                    signal_type="pair",
                    signal=f"{cue_a} + {cue_b}",
                    tier=pair_tier(event, cue_a, cue_b),
                    observed_at=event.timestamp,
                    observed_file=str(event.path),
                    resolved_outcome=outcome,
                    latency_minutes=latency,
                    resolution_mode=mode,
                    resolved_at=resolved_at,
                    resolved_file=resolved_file,
                    excerpt=event.excerpt,
                )
            )
    return observations


def summarize_signal(observations: list[SignalObservation]) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str], list[SignalObservation]] = defaultdict(list)
    for observation in observations:
        grouped[(observation.signal_type, observation.signal)].append(observation)

    summaries: list[dict[str, Any]] = []
    for (signal_type, signal), rows in grouped.items():
        outcome_counts = Counter(row.resolved_outcome for row in rows)
        tier_counts = Counter(row.tier for row in rows)
        same_entry_count = sum(row.resolution_mode == "same_entry" for row in rows)
        later_entry_count = sum(row.resolution_mode == "later_entry" for row in rows)
        same_outcome_counts = Counter(
            row.resolved_outcome for row in rows if row.resolution_mode == "same_entry"
        )
        later_outcome_counts = Counter(
            row.resolved_outcome for row in rows if row.resolution_mode == "later_entry"
        )
        resolved_latencies = [
            row.latency_minutes
            for row in rows
            if row.latency_minutes is not None and row.resolved_outcome != "none_within_window"
        ]
        per_outcome_latency = {}
        for outcome in OUTCOME_ORDER:
            values = [
                row.latency_minutes
                for row in rows
                if row.resolved_outcome == outcome and row.latency_minutes is not None
            ]
            if values:
                per_outcome_latency[outcome] = round(statistics.median(values), 2)

        if (
            later_outcome_counts["tightening"] >= 2
            and later_outcome_counts["tightening"] > later_outcome_counts["recovery"]
            and later_outcome_counts["tightening"] > later_outcome_counts["mixed"]
        ):
            signal_label = "later_tightening_candidate"
        elif (
            later_outcome_counts["recovery"] >= 2
            and later_outcome_counts["recovery"] > later_outcome_counts["tightening"]
            and later_outcome_counts["recovery"] > later_outcome_counts["mixed"]
        ):
            signal_label = "later_recovery_candidate"
        elif (
            same_outcome_counts["tightening"] >= 2
            and same_outcome_counts["tightening"] >= later_outcome_counts["tightening"]
        ):
            signal_label = "same_entry_heavy_tightening"
        elif (
            same_outcome_counts["recovery"] >= 2
            and same_outcome_counts["recovery"] >= later_outcome_counts["recovery"]
        ):
            signal_label = "same_entry_heavy_recovery"
        elif (
            outcome_counts["tightening"] >= 1
            and sum(outcome_counts.values()) == outcome_counts["tightening"]
            and same_entry_count >= 1
            and later_entry_count == 0
        ):
            signal_label = "anecdotal_same_entry_tightening"
        elif outcome_counts["tightening"] >= 1 and sum(outcome_counts.values()) == outcome_counts["tightening"]:
            signal_label = "anecdotal_tightening_only"
        elif (
            outcome_counts["recovery"] >= 1
            and sum(outcome_counts.values()) == outcome_counts["recovery"]
            and same_entry_count >= 1
            and later_entry_count == 0
        ):
            signal_label = "anecdotal_same_entry_recovery"
        elif outcome_counts["recovery"] >= 1 and sum(outcome_counts.values()) == outcome_counts["recovery"]:
            signal_label = "anecdotal_recovery_only"
        elif outcome_counts["none_within_window"] == len(rows):
            signal_label = "no_observed_resolution"
        else:
            signal_label = "mixed_or_sparse"

        summaries.append(
            {
                "signal_type": signal_type,
                "signal": signal,
                "observations": len(rows),
                "tier_counts": dict(tier_counts),
                "outcomes": {key: outcome_counts.get(key, 0) for key in OUTCOME_ORDER},
                "same_outcomes": {
                    key: same_outcome_counts.get(key, 0) for key in OUTCOME_ORDER
                },
                "later_outcomes": {
                    key: later_outcome_counts.get(key, 0) for key in OUTCOME_ORDER
                },
                "same_entry_count": same_entry_count,
                "later_entry_count": later_entry_count,
                "median_latency_minutes": (
                    round(statistics.median(resolved_latencies), 2)
                    if resolved_latencies
                    else None
                ),
                "median_latency_by_outcome": per_outcome_latency,
                "signal_label": signal_label,
            }
        )
    summaries.sort(
        key=lambda item: (
            item["signal_type"] != "cue",
            item["signal_label"] not in {"repeated_tightening_candidate", "repeated_recovery_candidate"},
            -item["observations"],
            item["signal"],
        )
    )
    return summaries


def summarize_observations(observations: list[SignalObservation]) -> dict[str, Any]:
    signal_summaries = summarize_signal(observations)
    cue_summaries = [item for item in signal_summaries if item["signal_type"] == "cue"]
    pair_summaries = [item for item in signal_summaries if item["signal_type"] == "pair"]
    return {
        "generated_at": datetime.now().astimezone().isoformat(timespec="seconds"),
        "observation_count": len(observations),
        "cue_summaries": cue_summaries,
        "pair_summaries": pair_summaries,
        "recent_observations": [
            {
                "signal_type": row.signal_type,
                "signal": row.signal,
                "tier": row.tier,
                "observed_at": row.observed_at,
                "resolved_outcome": row.resolved_outcome,
                "latency_minutes": row.latency_minutes,
                "resolution_mode": row.resolution_mode,
                "resolved_at": row.resolved_at,
                "observed_file": row.observed_file,
                "resolved_file": row.resolved_file,
                "excerpt": row.excerpt,
            }
            for row in observations[-18:]
        ],
    }


def render_summary_rows(rows: list[dict[str, Any]]) -> list[str]:
    rendered: list[str] = []
    if not rows:
        return ["- No signals found."]
    for row in rows:
        outcomes = ", ".join(
            f"{label}={row['outcomes'].get(label, 0)}"
            for label in OUTCOME_ORDER
            if row["outcomes"].get(label, 0)
        ) or "none"
        later = ", ".join(
            f"{label}={row['later_outcomes'].get(label, 0)}"
            for label in OUTCOME_ORDER
            if row["later_outcomes"].get(label, 0)
        ) or "none"
        same = ", ".join(
            f"{label}={row['same_outcomes'].get(label, 0)}"
            for label in OUTCOME_ORDER
            if row["same_outcomes"].get(label, 0)
        ) or "none"
        tiers = ", ".join(
            f"{tier}={count}" for tier, count in sorted(row.get("tier_counts", {}).items())
        ) or "none"
        median = row.get("median_latency_minutes")
        latency_label = "n/a" if median is None else f"{median} min"
        rendered.append(
            f"- `{row['signal']}` [{row['signal_label']}] "
            f"obs={row['observations']}, outcomes=({outcomes}), "
            f"later=({later}), same=({same}), median_latency={latency_label}, tiers=({tiers}), "
            f"same_entry={row['same_entry_count']}, later_entry={row['later_entry_count']}"
        )
    return rendered


def render_report(summary: dict[str, Any]) -> str:
    lines = [
        "# BTSP Latency Table",
        "",
        f"Generated: {summary['generated_at']}",
        f"Total cue/pair observations: {summary['observation_count']}",
        "",
        "## Cue Latency Table",
        "",
    ]
    lines.extend(render_summary_rows(summary.get("cue_summaries", [])))
    lines.extend(["", "## Pair Latency Table", ""])
    lines.extend(render_summary_rows(summary.get("pair_summaries", [])))
    lines.extend(["", "## Recent Observations", ""])
    for row in summary.get("recent_observations", []):
        latency = "n/a" if row["latency_minutes"] is None else f"{row['latency_minutes']} min"
        lines.append(
            f"- {row['observed_at']} — {row['signal_type']} `{row['signal']}` [{row['tier']}] -> "
            f"`{row['resolved_outcome']}` in {latency} ({row['resolution_mode']})"
        )
        lines.append(f"  Source: {row['observed_file']}")
        if row.get("resolved_file"):
            lines.append(f"  Outcome file: {row['resolved_file']}")
        lines.append(f"  Excerpt: {row['excerpt']}")
    return "\n".join(lines) + "\n"


def main() -> int:
    args = parse_args()
    minime_root = Path(args.minime_root).expanduser().resolve()
    output_dir = Path(args.output_dir).expanduser().resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    events = load_events(minime_root, args.limit)
    observations = collect_observations(
        events,
        max_minutes=args.max_minutes,
        max_events_ahead=args.max_events_ahead,
    )
    summary = summarize_observations(observations)
    report = render_report(summary)

    (output_dir / "summary.json").write_text(json.dumps(summary, indent=2))
    (output_dir / "report.md").write_text(report)

    print(output_dir / "summary.json")
    print(output_dir / "report.md")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
