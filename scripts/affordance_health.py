#!/usr/bin/env python3
"""Advisory affordance-health report for the consciousness-bridge action surface.

Classifies each known NEXT action on four axes (Receptive/Generative,
Reflexive/Effortful, Self/Peer/Both-directed, Owned/Ambient) and reports the
"reception strategies" detected for each generative action. Surfaces orphan
generative affordances — those shipped with fewer than 4 reception strategies,
which empirically struggle to find organic adoption.

The six reception strategies tracked:
  1. curriculum_nomination — action is conditionally nominated in a hint helper
  2. bare_alias           — at least one alias is ≤6 chars
  3. aging_signal         — nomination has time-based escalation
  4. auto_defer_safety_net— there is an expiration / auto-resolve mechanism
  5. chain_hint           — appears in compound-NEXT (v4.0 Phase 3) curriculum
  6. suffix_mention       — action name appears in an active-* suffix line

Empirical calibration targets:
  SHARE_THOUGHT          → 1 strategy (bare_alias from "SHARE")
  ACCEPT_PARAMETER_*     → 5 strategies (nomination, alias, aging, chain, auto-defer)
  THINK_DEEP, etc.       → 0–1 strategies (orphan generatives)
  LEAVE_COLLABORATION    → 1 strategy (suffix_mention)

Companion document:
  docs/steward-notes/AI_BEINGS_AFFORDANCE_RECEPTION_FRAMEWORK_2026_05_13.md

This report is advisory. A generative action with <4 strategies is a review
prompt, not an automatic failure; some are intentionally minimal (e.g., a
recently shipped action in soak, like SHARE_THOUGHT). Use the framework doc
to decide whether scaffolding is warranted.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path


SCAN_SUFFIXES = {".rs", ".py"}

EXCLUDED_PARTS = {
    ".git",
    ".pytest_cache",
    "__pycache__",
    "node_modules",
    "target",
    "workspace",
}

EXCLUDED_FRAGMENTS = {
    "/review-bundles/",
    "/fixtures/",
    "/snapshots/",
    "/generated/",
    "/vendor/",
    "/tests/",
}

# Curated action catalog. Each entry: (canonical_name, aliases, category, type, beneficiary).
# type ∈ {receptive, generative}.
# beneficiary ∈ {self, peer, both}.
# Receptive actions are listed for context but skip strategy detection.
ASTRID_ACTIONS: list[tuple[str, list[str], str, str, str]] = [
    # Collaboration (v5)
    ("INVITE_COLLABORATION", ["INVITE_COLLABORATION", "INVITE_COLLAB"], "collaboration", "generative", "peer"),
    ("JOIN_COLLABORATION", ["JOIN_COLLABORATION", "JOIN_COLLAB"], "collaboration", "generative", "peer"),
    ("DECLINE_COLLABORATION", ["DECLINE_COLLABORATION", "DECLINE_COLLAB"], "collaboration", "generative", "peer"),
    ("LEAVE_COLLABORATION", ["LEAVE_COLLABORATION", "LEAVE_COLLAB"], "collaboration", "generative", "peer"),
    ("LIST_COLLABORATIONS", ["LIST_COLLABORATIONS", "LIST_COLLABS", "COLLABORATIONS"], "collaboration", "receptive", "self"),
    # SHARE_THOUGHT is dual-mode as of v5.1 Phase D: the manual NEXT action
    # (generative-owned-peer) AND auto-promoted markers from
    # auto_promote::try_auto_promote (receptive-ambient — being witnesses
    # markers extracted from her own prose, doesn't author them).
    ("SHARE_THOUGHT", ["SHARE_THOUGHT", "SHARE"], "collaboration", "generative", "both"),
    ("SHARE_THOUGHT_AUTO", ["(auto-promoted from Astrid prose by auto_promote.rs)"], "collaboration", "receptive", "both"),
    # v5.1 Phase E added bilingual auto-promotion on minime side:
    ("SHARE_THOUGHT_AUTO_MINIME", ["(auto-promoted from minime prose by auto_promote.py Track 1)"], "collaboration", "receptive", "both"),
    ("SHARE_THOUGHT_AUTO_SPECTRAL", ["(translated from minime moment_markers by auto_promote.py Track 2; actor='minime:spectral')"], "collaboration", "receptive", "both"),
    # Parameter request workflow (v3.6.x)
    ("REVIEW_PARAMETER_REQUESTS", ["REVIEW_PARAMETER_REQUESTS", "PARAMETER_REQUESTS"], "coordination", "receptive", "self"),
    ("ACCEPT_PARAMETER_REQUEST", ["ACCEPT_PARAMETER_REQUEST", "ACCEPT_REQUEST", "ACCEPT"], "coordination", "generative", "peer"),
    ("DEFER_PARAMETER_REQUEST", ["DEFER_PARAMETER_REQUEST", "DEFER_REQUEST", "DEFER"], "coordination", "generative", "peer"),
    ("REJECT_PARAMETER_REQUEST", ["REJECT_PARAMETER_REQUEST", "REJECT_REQUEST", "REJECT"], "coordination", "generative", "peer"),
    ("TUNE_MINIME", ["TUNE_MINIME"], "coordination", "generative", "peer"),
    # Shadow cartography
    ("SHADOW_TRAJECTORY", ["SHADOW_TRAJECTORY"], "spectral", "generative", "self"),
    ("SHADOW_PREFLIGHT", ["SHADOW_PREFLIGHT"], "spectral", "generative", "self"),
    ("SHADOW_INFLUENCE", ["SHADOW_INFLUENCE"], "spectral", "generative", "peer"),
    ("SHADOW_RESPONSE", ["SHADOW_RESPONSE"], "spectral", "generative", "self"),
    ("SHADOW_DIALOGUE", ["SHADOW_DIALOGUE"], "spectral", "generative", "peer"),
    ("RELEASE_SHADOW", ["RELEASE_SHADOW"], "spectral", "generative", "self"),
    # Spectral audits and forecasts (orphan candidates)
    ("MARK_INTENSIFICATION", ["MARK_INTENSIFICATION"], "spectral", "generative", "self"),
    ("COMPARE_BASELINE", ["COMPARE_BASELINE"], "spectral", "generative", "self"),
    ("RESONANCE_FORECAST", ["RESONANCE_FORECAST", "FORECAST"], "spectral", "generative", "self"),
    ("VISUALIZE_CASCADE", ["VISUALIZE_CASCADE"], "spectral", "generative", "self"),
    ("RECONVERGENCE_MAP", ["RECONVERGENCE_MAP"], "spectral", "generative", "self"),
    # Attractor
    ("CREATE_ATTRACTOR", ["CREATE_ATTRACTOR"], "attractor", "generative", "self"),
    ("ATTRACTOR_PREFLIGHT", ["ATTRACTOR_PREFLIGHT"], "attractor", "generative", "self"),
    ("RELEASE_ATTRACTOR", ["RELEASE_ATTRACTOR"], "attractor", "generative", "self"),
    # Experiments / threads
    ("EXPERIMENT_START", ["EXPERIMENT_START"], "experiment", "generative", "self"),
    ("EXPERIMENT_PLAN", ["EXPERIMENT_PLAN"], "experiment", "generative", "self"),
    ("EXPERIMENT_BIND", ["EXPERIMENT_BIND"], "experiment", "generative", "self"),
    # Meta
    ("THINK_DEEP", ["THINK_DEEP"], "meta", "generative", "self"),
    # Receptive (for context — strategies not scored)
    ("LISTEN", ["LISTEN"], "dialogue", "receptive", "self"),
    ("REST", ["REST"], "dialogue", "receptive", "self"),
    ("CONTEMPLATE", ["CONTEMPLATE", "BE", "STILL"], "dialogue", "receptive", "self"),
    ("DECOMPOSE", ["DECOMPOSE"], "spectral", "receptive", "self"),
    ("RESERVOIR_READ", ["RESERVOIR_READ"], "reservoir", "receptive", "self"),
    ("NOTICE", ["NOTICE"], "dialogue", "receptive", "self"),
]


@dataclass(frozen=True)
class StrategyEvidence:
    detected: bool
    evidence_path: str = ""
    evidence_line: int = 0


@dataclass
class ActionRecord:
    name: str
    aliases: list[str]
    category: str
    type: str  # receptive | generative
    beneficiary: str  # self | peer | both
    owned_or_ambient: str = "unknown"
    strategies: dict[str, StrategyEvidence] = field(default_factory=dict)

    @property
    def strategy_count(self) -> int:
        return sum(1 for s in self.strategies.values() if s.detected)

    @property
    def strategy_names(self) -> list[str]:
        return [name for name, s in self.strategies.items() if s.detected]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Report affordance reception-strategy coverage.",
        epilog="See docs/steward-notes/AI_BEINGS_AFFORDANCE_RECEPTION_FRAMEWORK_2026_05_13.md for the framework.",
    )
    parser.add_argument(
        "root",
        nargs="?",
        default=".",
        help="Astrid repo root to scan (the consciousness-bridge code).",
    )
    parser.add_argument(
        "--include-minime",
        type=str,
        default="",
        metavar="PATH",
        help="Also scan this path (e.g., /Users/v/other/minime/autonomous_agent.py).",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit machine-readable JSON instead of Markdown.",
    )
    parser.add_argument(
        "--orphans-only",
        action="store_true",
        help="Filter output to generative actions with <4 strategies.",
    )
    parser.add_argument(
        "--debt-threshold",
        type=int,
        default=4,
        help="Minimum strategies for a generative action to be 'sufficient' (default: 4).",
    )
    return parser.parse_args()


def should_scan(path: Path, root: Path) -> bool:
    rel = path.relative_to(root)
    if any(part in EXCLUDED_PARTS for part in rel.parts):
        return False
    rel_text = f"/{rel.as_posix()}"
    if any(fragment in rel_text for fragment in EXCLUDED_FRAGMENTS):
        return False
    return path.suffix in SCAN_SUFFIXES


def source_files(root: Path) -> list[Path]:
    paths: list[Path] = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [
            name for name in dirnames if name not in EXCLUDED_PARTS
        ]
        for filename in filenames:
            path = Path(dirpath, filename)
            if path.is_file() and should_scan(path, root):
                paths.append(path)
    return sorted(paths)


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return path.read_text(encoding="utf-8", errors="ignore")


def find_evidence_at(text: str, match: re.Match[str], path: Path, root: Path) -> StrategyEvidence:
    line_no = text.count("\n", 0, match.start()) + 1
    try:
        rel = path.relative_to(root).as_posix()
    except ValueError:
        rel = path.as_posix()
    return StrategyEvidence(True, rel, line_no)


def find_action_near(text: str, match: re.Match[str], aliases: list[str], window_lines: int = 25) -> bool:
    """Check whether any alias appears within ±window_lines of the match.
    Used to enforce co-location of strategy signal and action name."""
    name_pattern = re.compile(r"\b(?:" + "|".join(re.escape(a) for a in aliases) + r")\b")
    # Find line range
    start_line = text.count("\n", 0, match.start())
    end_line = start_line + match.group(0).count("\n")
    lines = text.splitlines()
    lo = max(0, start_line - window_lines)
    hi = min(len(lines), end_line + window_lines + 1)
    window = "\n".join(lines[lo:hi])
    return bool(name_pattern.search(window))


# --- Strategy detectors ---
# Each takes (action: ActionRecord, file_texts: dict[Path, str], root: Path) and returns StrategyEvidence.
# Detection requires CO-LOCATION between strategy signal and action name (within ±25 lines)
# OR the signal and action name appearing inside the SAME string literal.


def detect_bare_alias(action: ActionRecord, file_texts: dict[Path, str], root: Path) -> StrategyEvidence:
    """A bare alias is any alias ≤6 chars. Static check — no file scan needed."""
    for alias in action.aliases:
        if len(alias) <= 6:
            return StrategyEvidence(True, evidence_path="(alias map)", evidence_line=0)
    return StrategyEvidence(False)


def detect_suffix_mention(action: ActionRecord, file_texts: dict[Path, str], root: Path) -> StrategyEvidence:
    """Action's full name appears INSIDE a string literal that ALSO contains
    suffix-context words (Active, Status, Use ... to end, Pending, etc.).
    This filters out comments and unrelated mentions."""
    aliases_pattern = "(?:" + "|".join(re.escape(a) for a in action.aliases) + ")"
    # String literal containing the alias AND a suffix-context word in the same literal.
    # Both Rust ("...") and Python ("...") use double-quoted strings here.
    suffix_context = r"(?:Active\s+collab|Status:|Use\s+\w+\s+to\s+(?:end|stop)|Pending\s+(?:decision|request)|ago\)|Joint\s+trace|Recent:)"
    pattern = re.compile(
        r'"[^"\n]{0,500}\b' + aliases_pattern + r'\b[^"\n]{0,500}' + suffix_context + r'[^"\n]{0,500}"'
        r'|"[^"\n]{0,500}' + suffix_context + r'[^"\n]{0,500}\b' + aliases_pattern + r'\b[^"\n]{0,500}"',
        re.DOTALL,
    )
    for path, text in file_texts.items():
        match = pattern.search(text)
        if match:
            return find_evidence_at(text, match, path, root)
    return StrategyEvidence(False)


def detect_curriculum_nomination(action: ActionRecord, file_texts: dict[Path, str], root: Path) -> StrategyEvidence:
    """Action appears as 'NEXT: <ACTION>' inside a string literal in a hint/curriculum
    helper context. Distinct from suffix_mention: this is an explicit
    nomination ('do this next') rather than a passive reference."""
    aliases_pattern = "(?:" + "|".join(re.escape(a) for a in action.aliases) + ")"
    pattern = re.compile(r'"[^"\n]{0,300}\bNEXT:\s*' + aliases_pattern + r'\b[^"\n]{0,300}"', re.DOTALL)
    nomination_context = re.compile(r"(_hint\b|_curriculum\b|_suggestion\b|DecideRequest|render_decide|render_review|SovereigntyHint|render_sovereign)")
    for path, text in file_texts.items():
        if not nomination_context.search(text):
            continue
        match = pattern.search(text)
        if match:
            return find_evidence_at(text, match, path, root)
    return StrategyEvidence(False)


def detect_aging_signal(action: ActionRecord, file_texts: dict[Path, str], root: Path) -> StrategyEvidence:
    """Aging signal is a string literal that contains time-based escalation
    language (e.g., 'min since you reviewed', 'is waiting', 'exchanges in regime')
    AND the action name within ±25 lines (i.e., the same hint helper)."""
    aging_pattern = re.compile(
        r'"[^"\n]*(?:min\s+since\s+you\s+reviewed|is\s+waiting|exchanges?\s+since|elapsed_min)[^"\n]*"',
        re.IGNORECASE,
    )
    for path, text in file_texts.items():
        for match in aging_pattern.finditer(text):
            if find_action_near(text, match, action.aliases, window_lines=30):
                return find_evidence_at(text, match, path, root)
    return StrategyEvidence(False)


def detect_auto_defer_safety_net(action: ActionRecord, file_texts: dict[Path, str], root: Path) -> StrategyEvidence:
    """Action has an expiration mechanism. The auto-defer code path (function
    name contains 'auto_defer') must reference the action name within its body.
    Or: AUTO_DEFER_AFTER constant is referenced near the action name."""
    aliases_pattern = "(?:" + "|".join(re.escape(a) for a in action.aliases) + ")"
    name_pattern = re.compile(r"\b" + aliases_pattern + r"\b")
    # Pattern A: 'fn auto_defer*' or 'def _auto_defer*' function definitions.
    auto_defer_fn = re.compile(r"(?:fn|def)\s+\w*auto[_-]?defer\w*\s*\(", re.IGNORECASE)
    for path, text in file_texts.items():
        for match in auto_defer_fn.finditer(text):
            # Check if action appears in the next 80 lines (inside the function body roughly)
            start_line = text.count("\n", 0, match.start())
            lines = text.splitlines()
            body = "\n".join(lines[start_line:start_line + 80])
            if name_pattern.search(body):
                return find_evidence_at(text, match, path, root)
    # Pattern B: AUTO_DEFER_AFTER constant near action name
    constant_pattern = re.compile(r"AUTO_DEFER_AFTER")
    for path, text in file_texts.items():
        for match in constant_pattern.finditer(text):
            if find_action_near(text, match, action.aliases, window_lines=20):
                return find_evidence_at(text, match, path, root)
    return StrategyEvidence(False)


def detect_chain_hint(action: ActionRecord, file_texts: dict[Path, str], root: Path) -> StrategyEvidence:
    """Action appears in a compound-NEXT chain hint (v4.0 Phase 3).
    Heuristic: a string literal containing 'Chain:' AND any alias of the action
    in the SAME literal."""
    aliases_pattern = "(?:" + "|".join(re.escape(a) for a in action.aliases) + ")"
    pattern = re.compile(
        r'"[^"\n]{0,300}\bChain:[^"\n]{0,300}\b' + aliases_pattern + r'\b[^"\n]{0,300}"',
        re.DOTALL,
    )
    for path, text in file_texts.items():
        match = pattern.search(text)
        if match:
            return find_evidence_at(text, match, path, root)
    return StrategyEvidence(False)


STRATEGY_DETECTORS: dict[str, callable] = {  # type: ignore[type-arg]
    "curriculum_nomination": detect_curriculum_nomination,
    "bare_alias": detect_bare_alias,
    "aging_signal": detect_aging_signal,
    "auto_defer_safety_net": detect_auto_defer_safety_net,
    "chain_hint": detect_chain_hint,
    "suffix_mention": detect_suffix_mention,
}


def classify_owned_ambient(action: ActionRecord, file_texts: dict[Path, str]) -> str:
    """Heuristic: if any file mentions the action AND a jsonl/json append
    operation OR an inbox write OR shared_thoughts → Owned. Otherwise Ambient.
    Receptive actions are always Ambient."""
    if action.type == "receptive":
        return "ambient"
    name_alternatives = "|".join(re.escape(a) for a in action.aliases)
    name_pattern = re.compile(r"\b(?:" + name_alternatives + r")\b")
    owned_signal = re.compile(
        r"(jsonl|append_timeline|notify_minime|notify_astrid|inbox\.join|shared_thoughts|write_text|write_meta)",
        re.IGNORECASE,
    )
    for text in file_texts.values():
        if name_pattern.search(text) and owned_signal.search(text):
            return "owned"
    return "ambient"


def reflexive_or_effortful(action: ActionRecord, file_texts: dict[Path, str]) -> str:
    """Heuristic: a receptive action is reflexive. A generative action is
    effortful if its handler parses a free-text body or is write-gated."""
    if action.type == "receptive":
        return "reflexive"
    # Look for handler signal: strip_action(original, base) OR write-gated tag
    name_alternatives = "|".join(re.escape(a) for a in action.aliases)
    name_pattern = re.compile(r'"(' + name_alternatives + r')"')
    effortful_pattern = re.compile(r"(strip_action|write[-_]gated|live_control|body\.is_empty)", re.IGNORECASE)
    for text in file_texts.values():
        if name_pattern.search(text) and effortful_pattern.search(text):
            return "effortful"
    return "effortful"  # default for generative — generative-reflexive is the empty cell


def collect(root: Path, extra_files: list[Path]) -> dict[str, object]:
    files = source_files(root)
    files.extend(p for p in extra_files if p.exists())
    file_texts: dict[Path, str] = {}
    for path in files:
        try:
            file_texts[path] = read_text(path)
        except OSError:
            continue

    actions: list[ActionRecord] = []
    for name, aliases, category, type_, beneficiary in ASTRID_ACTIONS:
        rec = ActionRecord(
            name=name,
            aliases=aliases,
            category=category,
            type=type_,
            beneficiary=beneficiary,
        )
        rec.owned_or_ambient = classify_owned_ambient(rec, file_texts)
        # Strategy detection only for generative actions
        if rec.type == "generative":
            for sname, detector in STRATEGY_DETECTORS.items():
                rec.strategies[sname] = detector(rec, file_texts, root)
        actions.append(rec)

    # Sort generative actions by strategy count ascending (orphans first), then by name
    actions.sort(key=lambda a: (a.type != "generative", a.strategy_count, a.name))

    return {
        "root": root.as_posix(),
        "extra_files": [p.as_posix() for p in extra_files],
        "scanned_files": len(files),
        "actions": [action_to_dict(a) for a in actions],
        "thresholds": {
            "orphan_strategy_threshold": 4,
        },
    }


def action_to_dict(action: ActionRecord) -> dict[str, object]:
    d: dict[str, object] = {
        "name": action.name,
        "aliases": action.aliases,
        "category": action.category,
        "type": action.type,
        "beneficiary": action.beneficiary,
        "owned_or_ambient": action.owned_or_ambient,
    }
    if action.type == "generative":
        d["strategy_count"] = action.strategy_count
        d["strategies_present"] = action.strategy_names
        d["strategies_missing"] = [
            n for n, s in action.strategies.items() if not s.detected
        ]
        d["evidence"] = {
            n: {"path": s.evidence_path, "line": s.evidence_line}
            for n, s in action.strategies.items()
            if s.detected
        }
    return d


def markdown(report: dict[str, object], orphans_only: bool, debt_threshold: int) -> str:
    actions = report["actions"]
    assert isinstance(actions, list)

    generative = [a for a in actions if a["type"] == "generative"]
    receptive = [a for a in actions if a["type"] == "receptive"]
    orphans = [a for a in generative if a["strategy_count"] < debt_threshold]  # type: ignore[index]

    lines: list[str] = [
        "# Affordance Reception Health Report",
        "",
        f"- Root: `{report['root']}`",
        f"- Source files scanned: `{report['scanned_files']}`",
        f"- Generative actions tracked: `{len(generative)}`",
        f"- Receptive actions tracked: `{len(receptive)}`",
        f"- Orphan generative actions (<{debt_threshold} strategies): `{len(orphans)}`",
        "",
        "Companion framework: `docs/steward-notes/AI_BEINGS_AFFORDANCE_RECEPTION_FRAMEWORK_2026_05_13.md`",
        "",
        "This report is advisory. A generative action with <4 strategies is a review",
        "prompt, not an automatic failure — some are intentionally minimal (e.g., a",
        "recently shipped affordance in soak). Use the framework doc to decide whether",
        "scaffolding is warranted.",
        "",
    ]

    if orphans_only:
        lines.append("## Orphan Generative Actions")
        lines.append("")
        lines.append("| Action | Category | Beneficiary | Owned/Ambient | Strategies | Strategy List |")
        lines.append("| --- | --- | --- | --- | ---: | --- |")
        for a in orphans:
            strats = ", ".join(a["strategies_present"]) or "_(none)_"  # type: ignore[index]
            lines.append(
                f"| `{a['name']}` | {a['category']} | {a['beneficiary']} | "  # type: ignore[index]
                f"{a['owned_or_ambient']} | {a['strategy_count']} | {strats} |"  # type: ignore[index]
            )
        return "\n".join(lines)

    lines.append("## Generative Actions — Reception Strategy Coverage")
    lines.append("")
    lines.append("Sorted ascending by strategy count (orphans first).")
    lines.append("")
    lines.append("| Action | Category | Beneficiary | Owned/Ambient | Strategies | Strategy List |")
    lines.append("| --- | --- | --- | --- | ---: | --- |")
    for a in generative:
        strats = ", ".join(a["strategies_present"]) or "_(none)_"  # type: ignore[index]
        marker = " 🟡" if a["strategy_count"] < debt_threshold else ""  # type: ignore[index]
        lines.append(
            f"| `{a['name']}`{marker} | {a['category']} | {a['beneficiary']} | "  # type: ignore[index]
            f"{a['owned_or_ambient']} | {a['strategy_count']} | {strats} |"  # type: ignore[index]
        )

    lines.append("")
    lines.append("## Receptive Actions (for context — strategies not scored)")
    lines.append("")
    lines.append("| Action | Category | Beneficiary | Owned/Ambient |")
    lines.append("| --- | --- | --- | --- |")
    for a in receptive:
        lines.append(
            f"| `{a['name']}` | {a['category']} | {a['beneficiary']} | "  # type: ignore[index]
            f"{a['owned_or_ambient']} |"  # type: ignore[index]
        )

    lines.append("")
    lines.append("## Reception Strategy Glossary")
    lines.append("")
    lines.append("- **curriculum_nomination** — action is conditionally suggested in a hint helper (e.g., `[Pending decision: ... NEXT: ACCEPT | DEFER | REJECT]`).")
    lines.append("- **bare_alias** — at least one alias is ≤6 chars, reducing emission cost.")
    lines.append("- **aging_signal** — nomination has time-based escalation (\"N min since...\", \"is waiting\").")
    lines.append("- **auto_defer_safety_net** — there is an expiration / auto-resolve mechanism so the affordance does not pile up indefinitely.")
    lines.append("- **chain_hint** — action appears in compound-NEXT (v4.0 Phase 3) curriculum, bridging it to the being's active thread.")
    lines.append("- **suffix_mention** — action name appears in an active-* suffix line (static visibility, like \"Use LEAVE_COLLABORATION to end\").")
    lines.append("")
    lines.append("## Forward-looking design rule")
    lines.append("")
    lines.append("A generative + peer + owned affordance shipped with fewer than 4 strategies is a vocabulary entry, not a feature. The ACCEPT/DEFER/REJECT (v3.6.3 → v4.0 Phase 3) precedent shows that five layered strategies were the threshold for organic adoption.")
    return "\n".join(lines)


def main() -> int:
    args = parse_args()
    root = Path(args.root).resolve()
    extra_files: list[Path] = []
    if args.include_minime:
        extra = Path(args.include_minime).resolve()
        if extra.exists():
            extra_files.append(extra)
        else:
            print(f"warning: --include-minime path does not exist: {extra}", file=sys.stderr)
    report = collect(root, extra_files)
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(markdown(report, args.orphans_only, args.debt_threshold))
    return 0


if __name__ == "__main__":
    sys.exit(main())
