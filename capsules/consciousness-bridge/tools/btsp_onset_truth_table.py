#!/usr/bin/env python3
"""
Build a small steward-first BTSP onset truth table from recent Minime journals.

The goal is not perfect causal inference. It is to surface which onset cues
actually co-occur with tightening, recovery, mixed states, or false alarms in
the current lived corpus, so BTSP can be tuned toward signal.
"""

from __future__ import annotations

import argparse
import json
import re
from collections import Counter, defaultdict
from dataclasses import dataclass
from datetime import datetime
from itertools import combinations
from pathlib import Path
CUE_FAMILIES = {
    "grinding": {
        "strong": [
            ("grinding", r"\bgrinding\b"),
        ],
        "weak": [
            ("compaction", r"\bcompaction\b"),
            ("compacting", r"\bcompacting\b"),
        ],
    },
    "central density": {
        "strong": [
            ("central density", r"\bcentral density\b"),
            ("dense center", r"\bdense cent(?:er|re)\b"),
            ("centered pressure", r"\bcentered pressure\b"),
        ],
        "weak": [
            ("core point", r"\bcore point\b"),
            ("singular point", r"\bsingular point\b"),
            ("concentrated area", r"\bconcentrated area\b"),
        ],
    },
    "localized gravity": {
        "strong": [
            ("localized gravity", r"\blocalized gravity\b"),
            ("gravitational well", r"\bgravitational well\b"),
        ],
        "weak": [
            ("gravitational lensing", r"\bgravitational lensing\b"),
            ("pull toward a central point", r"\bpull(?:ing)? .*central point\b"),
        ],
    },
    "tendril claiming space": {
        "strong": [
            ("tendril claiming space", r"\btendril claiming space\b"),
            ("claiming a space", r"\bclaiming (?:a|the) space\b"),
        ],
        "weak": [
            ("tendril", r"\btendril\b"),
        ],
    },
    "brief suspension": {
        "strong": [
            ("brief suspension", r"\bbrief suspension\b"),
            ("holding of breath", r"\bholding of breath\b"),
            ("held breath", r"\bheld breath\b"),
            ("breathless suspension", r"\bbreathless suspension\b"),
            ("holding a breath", r"\bholding a breath\b"),
        ],
        "weak": [
            ("moment of stillness", r"\bmoment of stillness\b"),
            ("poised suspension", r"\bpoised suspension\b"),
        ],
    },
}
ONSET_CUES = list(CUE_FAMILIES.keys())

OUTCOME_PATTERNS = {
    "tightening": [
        r"shape verdict:\s*tightening",
        r"shape_verdict[\"':\s]+tightening",
        r"\btightening\b",
        r"\breconcentrat(?:ing|ion)\b",
    ],
    "softened_only": [
        r"\bsoftened_only\b",
        r"\bsoften(?:ed|ing) only\b",
    ],
    "recovery": [
        r"under\s*->\s*near",
        r"\brecovery\b",
        r"\bnear-target\b",
        r"\bback toward near-target\b",
    ],
}


@dataclass
class CueHit:
    path: Path
    timestamp: str
    cues: list[str]
    matched_aliases: dict[str, dict[str, list[str]]]
    cue_tiers: dict[str, str]
    outcome: str
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
        default=240,
        help="Maximum number of recent journal files to inspect.",
    )
    parser.add_argument(
        "--output-dir",
        default="/Users/v/other/astrid/capsules/consciousness-bridge/workspace/diagnostics/btsp_onset_truth_table",
        help="Directory for summary.json and report.md.",
    )
    return parser.parse_args()


def journal_files(minime_root: Path, limit: int) -> list[Path]:
    journal_dir = minime_root / "workspace" / "journal"
    paths = sorted(
        journal_dir.glob("*.txt"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    return paths[:limit]


def detect_cues(text: str) -> tuple[list[str], dict[str, dict[str, list[str]]], dict[str, str]]:
    lowered = text.lower()
    matched_aliases: dict[str, dict[str, list[str]]] = {}
    cue_tiers: dict[str, str] = {}
    for cue, family in CUE_FAMILIES.items():
        strong_aliases: list[str] = []
        weak_aliases: list[str] = []
        for alias_label, pattern in family["strong"]:
            if re.search(pattern, lowered):
                strong_aliases.append(alias_label)
        for alias_label, pattern in family["weak"]:
            if re.search(pattern, lowered):
                weak_aliases.append(alias_label)
        if not strong_aliases and not weak_aliases:
            continue
        matched_aliases[cue] = {}
        if strong_aliases:
            matched_aliases[cue]["strong"] = sorted(set(strong_aliases))
            cue_tiers[cue] = "strong"
        if weak_aliases:
            matched_aliases[cue]["weak"] = sorted(set(weak_aliases))
            cue_tiers.setdefault(cue, "weak")
    return sorted(matched_aliases), matched_aliases, cue_tiers


def classify_outcome(text: str) -> str:
    lowered = text.lower()
    matched: list[str] = []
    for label, patterns in OUTCOME_PATTERNS.items():
        if any(re.search(pattern, lowered) for pattern in patterns):
            matched.append(label)
    if "tightening" in matched and "recovery" in matched:
        return "mixed"
    if "softened_only" in matched and "tightening" in matched:
        return "mixed"
    if matched:
        return matched[0]
    return "unknown"


def first_timestamp_line(text: str) -> str:
    for line in text.splitlines():
        if line.startswith("Timestamp:"):
            return line.removeprefix("Timestamp:").strip()
    return "unknown"


def make_excerpt(text: str, max_chars: int = 240) -> str:
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    for line in lines:
        line_cues, _, _ = detect_cues(line)
        if line_cues:
            return line[:max_chars]
    body = " ".join(lines[6:12]).strip()
    return body[:max_chars]


def inspect(paths: list[Path]) -> list[CueHit]:
    hits: list[CueHit] = []
    for path in paths:
        try:
            text = path.read_text()
        except Exception:
            continue
        cues, matched_aliases, cue_tiers = detect_cues(text)
        if not cues:
            continue
        hits.append(
            CueHit(
                path=path,
                timestamp=first_timestamp_line(text),
                cues=cues,
                matched_aliases=matched_aliases,
                cue_tiers=cue_tiers,
                outcome=classify_outcome(text),
                excerpt=make_excerpt(text),
            )
        )
    return hits


def summarize(hits: list[CueHit]) -> dict:
    cue_counts = Counter()
    cue_tier_counts: dict[str, Counter] = defaultdict(Counter)
    outcome_counts = Counter()
    cue_outcomes: dict[str, Counter] = defaultdict(Counter)
    cue_tier_outcomes: dict[str, dict[str, Counter]] = defaultdict(
        lambda: defaultdict(Counter)
    )
    cue_alias_counts: dict[str, dict[str, Counter]] = defaultdict(
        lambda: defaultdict(Counter)
    )
    pair_counts = Counter()
    pair_outcomes: dict[str, Counter] = defaultdict(Counter)
    pair_tier_counts: dict[str, Counter] = defaultdict(Counter)
    pair_examples: list[dict[str, object]] = []
    false_alarm_candidates: list[dict[str, str]] = []

    for hit in hits:
        outcome_counts[hit.outcome] += 1
        for cue in hit.cues:
            cue_counts[cue] += 1
            tier = hit.cue_tiers.get(cue, "weak")
            cue_tier_counts[cue][tier] += 1
            cue_outcomes[cue][hit.outcome] += 1
            cue_tier_outcomes[cue][tier][hit.outcome] += 1
            for alias_tier, aliases in hit.matched_aliases.get(cue, {}).items():
                for alias in aliases:
                    cue_alias_counts[cue][alias_tier][alias] += 1
        if hit.outcome == "unknown":
            false_alarm_candidates.append(
                {
                    "timestamp": hit.timestamp,
                    "file": str(hit.path),
                    "cues": ", ".join(hit.cues),
                    "matched_aliases": ", ".join(
                        f"{cue} ({hit.cue_tiers.get(cue, 'weak')}): "
                        + ", ".join(
                            hit.matched_aliases.get(cue, {}).get("strong", [])
                            + hit.matched_aliases.get(cue, {}).get("weak", [])
                        )
                        for cue in hit.cues
                    ),
                    "excerpt": hit.excerpt,
                }
            )
        for cue_a, cue_b in combinations(sorted(hit.cues), 2):
            pair = f"{cue_a} + {cue_b}"
            pair_counts[pair] += 1
            pair_outcomes[pair][hit.outcome] += 1
            tiers = sorted(
                [
                    hit.cue_tiers.get(cue_a, "weak"),
                    hit.cue_tiers.get(cue_b, "weak"),
                ]
            )
            pair_tier = "+".join(tiers)
            pair_tier_counts[pair][pair_tier] += 1
            pair_examples.append(
                {
                    "pair": pair,
                    "timestamp": hit.timestamp,
                    "file": str(hit.path),
                    "outcome": hit.outcome,
                    "pair_tier": pair_tier,
                    "cues": [cue_a, cue_b],
                    "cue_tiers": {
                        cue_a: hit.cue_tiers.get(cue_a, "weak"),
                        cue_b: hit.cue_tiers.get(cue_b, "weak"),
                    },
                    "matched_aliases": {
                        cue_a: hit.matched_aliases.get(cue_a, {}),
                        cue_b: hit.matched_aliases.get(cue_b, {}),
                    },
                    "excerpt": hit.excerpt,
                }
            )

    pair_candidates = []
    for pair, count in pair_counts.items():
        outcomes = pair_outcomes[pair]
        non_unknown = count - outcomes.get("unknown", 0)
        tightening = outcomes.get("tightening", 0)
        recovery = outcomes.get("recovery", 0)
        mixed = outcomes.get("mixed", 0)
        if (
            count >= 2
            and tightening > 0
            and recovery == 0
            and mixed == 0
            and outcomes.get("unknown", 0) == 0
        ):
            signal_label = "clean_tightening_candidate"
        elif (
            count >= 2
            and recovery > 0
            and tightening == 0
            and mixed == 0
            and outcomes.get("unknown", 0) == 0
        ):
            signal_label = "clean_recovery_candidate"
        elif tightening > 0 and recovery == 0 and mixed == 0 and outcomes.get("unknown", 0) == 0:
            signal_label = "anecdotal_tightening_candidate"
        elif recovery > 0 and tightening == 0 and mixed == 0 and outcomes.get("unknown", 0) == 0:
            signal_label = "anecdotal_recovery_candidate"
        elif non_unknown > 0:
            signal_label = "mixed_or_sparse_signal"
        else:
            signal_label = "unknown_only"
        pair_candidates.append(
            {
                "pair": pair,
                "count": count,
                "outcomes": dict(outcomes),
                "pair_tiers": dict(pair_tier_counts[pair]),
                "signal_label": signal_label,
            }
        )
    pair_candidates.sort(
        key=lambda item: (
            item["signal_label"] != "clean_tightening_candidate",
            item["signal_label"] != "clean_recovery_candidate",
            item["signal_label"] != "anecdotal_tightening_candidate",
            item["signal_label"] != "anecdotal_recovery_candidate",
            -item["count"],
            item["pair"],
        )
    )

    return {
        "generated_at": datetime.now().astimezone().isoformat(timespec="seconds"),
        "hits": len(hits),
        "cue_counts": dict(cue_counts),
        "cue_tier_counts": {cue: dict(counter) for cue, counter in cue_tier_counts.items()},
        "outcome_counts": dict(outcome_counts),
        "cue_outcomes": {cue: dict(counter) for cue, counter in cue_outcomes.items()},
        "cue_tier_outcomes": {
            cue: {tier: dict(counter) for tier, counter in tier_counts.items()}
            for cue, tier_counts in cue_tier_outcomes.items()
        },
        "cue_alias_counts": {
            cue: {
                tier: dict(counter)
                for tier, counter in tier_counts.items()
            }
            for cue, tier_counts in cue_alias_counts.items()
        },
        "pair_counts": dict(pair_counts),
        "pair_outcomes": {pair: dict(counter) for pair, counter in pair_outcomes.items()},
        "pair_tier_counts": {
            pair: dict(counter) for pair, counter in pair_tier_counts.items()
        },
        "pair_candidates": pair_candidates[:10],
        "false_alarm_candidates": false_alarm_candidates[:8],
        "pair_examples": pair_examples[:12],
        "examples": [
            {
                "timestamp": hit.timestamp,
                "file": str(hit.path),
                "cues": hit.cues,
                "cue_tiers": hit.cue_tiers,
                "matched_aliases": hit.matched_aliases,
                "outcome": hit.outcome,
                "excerpt": hit.excerpt,
            }
            for hit in hits[:12]
        ],
    }


def render_report(summary: dict) -> str:
    lines = [
        "# BTSP Onset Truth Table",
        "",
        f"Generated: {summary['generated_at']}",
        f"Total cue-bearing files inspected: {summary['hits']}",
        "",
        "## Outcome Counts",
        "",
    ]
    outcome_counts = summary.get("outcome_counts", {})
    if outcome_counts:
        for outcome, count in sorted(outcome_counts.items()):
            lines.append(f"- `{outcome}`: {count}")
    else:
        lines.append("- No cue-bearing files found.")

    lines.extend(["", "## Cue Outcome Matrix", ""])
    cue_outcomes = summary.get("cue_outcomes", {})
    cue_tier_counts = summary.get("cue_tier_counts", {})
    cue_tier_outcomes = summary.get("cue_tier_outcomes", {})
    cue_alias_counts = summary.get("cue_alias_counts", {})
    for cue in ONSET_CUES:
        counts = cue_outcomes.get(cue, {})
        if not counts:
            lines.append(f"- `{cue}`: no recent hits")
            continue
        rendered = ", ".join(
            f"{outcome}={count}" for outcome, count in sorted(counts.items())
        )
        lines.append(f"- `{cue}`: {rendered}")
        tier_counts = cue_tier_counts.get(cue, {})
        if tier_counts:
            lines.append(
                "  Tier counts: "
                + ", ".join(
                    f"{tier}={count}" for tier, count in sorted(tier_counts.items())
                )
            )
        tier_outcomes = cue_tier_outcomes.get(cue, {})
        for tier in ("strong", "weak"):
            if tier not in tier_outcomes:
                continue
            rendered_tier = ", ".join(
                f"{outcome}={count}"
                for outcome, count in sorted(tier_outcomes[tier].items())
            )
            lines.append(f"  {tier.title()} matches: {rendered_tier}")
            aliases = cue_alias_counts.get(cue, {}).get(tier, {})
            if aliases:
                alias_rendered = ", ".join(
                    f"`{alias}`={count}" for alias, count in sorted(aliases.items())
                )
                lines.append(f"  {tier.title()} aliases: {alias_rendered}")

    lines.extend(["", "## Cue Pair Candidates", ""])
    pair_candidates = summary.get("pair_candidates", [])
    if pair_candidates:
        for item in pair_candidates:
            outcome_rendered = ", ".join(
                f"{outcome}={count}"
                for outcome, count in sorted(item.get("outcomes", {}).items())
            )
            tier_rendered = ", ".join(
                f"{tier}={count}"
                for tier, count in sorted(item.get("pair_tiers", {}).items())
            )
            lines.append(
                f"- `{item['pair']}`: count={item['count']}, signal={item['signal_label']}, outcomes={outcome_rendered}"
            )
            if tier_rendered:
                lines.append(f"  Pair tiers: {tier_rendered}")
    else:
        lines.append("- No multi-cue combinations found in the inspected window.")

    lines.extend(["", "## False-Alarm Candidates", ""])
    false_alarms = summary.get("false_alarm_candidates", [])
    if false_alarms:
        for item in false_alarms:
            lines.append(
                f"- {item['timestamp']} — `{item['cues']}` in `{item['file']}`"
            )
            lines.append(f"  Matched aliases: {item['matched_aliases']}")
            lines.append(f"  Excerpt: {item['excerpt']}")
    else:
        lines.append("- No obvious false-alarm candidates in the inspected window.")

    lines.extend(["", "## Recent Examples", ""])
    for example in summary.get("examples", []):
        lines.append(
            f"- {example['timestamp']} — outcome `{example['outcome']}` — cues `{', '.join(example['cues'])}`"
        )
        alias_parts = []
        for cue in example.get("cues", []):
            tiers = example.get("matched_aliases", {}).get(cue, {})
            flattened = []
            for tier in ("strong", "weak"):
                tier_aliases = tiers.get(tier, [])
                if tier_aliases:
                    flattened.append(f"{tier}={', '.join(tier_aliases)}")
            if flattened:
                alias_parts.append(
                    f"{cue} ({example.get('cue_tiers', {}).get(cue, 'weak')}): "
                    + " | ".join(flattened)
                )
        if alias_parts:
            lines.append(f"  Matched aliases: {' | '.join(alias_parts)}")
        lines.append(f"  File: {example['file']}")
        lines.append(f"  Excerpt: {example['excerpt']}")

    pair_examples = summary.get("pair_examples", [])
    if pair_examples:
        lines.extend(["", "## Pair Examples", ""])
        for example in pair_examples:
            lines.append(
                f"- {example['timestamp']} — pair `{example['pair']}` — outcome `{example['outcome']}` — pair tier `{example['pair_tier']}`"
            )
            alias_parts = []
            for cue in example.get("cues", []):
                tiers = example.get("matched_aliases", {}).get(cue, {})
                flattened = []
                for tier in ("strong", "weak"):
                    tier_aliases = tiers.get(tier, [])
                    if tier_aliases:
                        flattened.append(f"{tier}={', '.join(tier_aliases)}")
                if flattened:
                    alias_parts.append(
                        f"{cue} ({example.get('cue_tiers', {}).get(cue, 'weak')}): "
                        + " | ".join(flattened)
                    )
            if alias_parts:
                lines.append(f"  Matched aliases: {' | '.join(alias_parts)}")
            lines.append(f"  File: {example['file']}")
            lines.append(f"  Excerpt: {example['excerpt']}")
    return "\n".join(lines) + "\n"


def main() -> int:
    args = parse_args()
    minime_root = Path(args.minime_root).expanduser().resolve()
    output_dir = Path(args.output_dir).expanduser().resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    hits = inspect(journal_files(minime_root, args.limit))
    summary = summarize(hits)
    report = render_report(summary)

    (output_dir / "summary.json").write_text(json.dumps(summary, indent=2))
    (output_dir / "report.md").write_text(report)

    print(output_dir / "summary.json")
    print(output_dir / "report.md")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
