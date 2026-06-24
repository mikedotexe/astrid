#!/usr/bin/env python3
"""Dispatch-vs-menu drift audit (Kink #18 prevention, 2026-05-14).

Background: Kink #18 was a real bug — `SHADOW_TRAJECTORY` had a wired
dispatch arm in `autonomous_agent.py` (so the action would EXECUTE if
the LLM picked it) but no mention in any prompt menu (so the LLM never
saw it as an option). The asymmetric pattern is recurring: cross-being
parity actions get added in the dispatch layer (`if base == 'X':`) but
the prompt-suggestion layer (action menu strings) gets forgotten.

This script audits both sides:

  - **Dispatched but not menu-mentioned** (the Kink #18 shape — silent
    starvation: action exists, LLM never finds it)
  - **Menu-mentioned but not dispatched** (the inverse: LLM picks it,
    "Unknown NEXT — falling back to threshold logic" follows)

Output: Markdown table to stdout (or `--json` for tooling). Read-only;
modifies nothing.

Usage:
    python3 scripts/dispatch_menu_drift.py
    python3 scripts/dispatch_menu_drift.py --json
    python3 scripts/dispatch_menu_drift.py --target /Users/v/other/minime/autonomous_agent.py

The default target is minime's `autonomous_agent.py` because that's
where this pattern bites; pass `--target` to point at a different file.
"""
from __future__ import annotations

import argparse
import ast
import json
import re
import sys
import unittest
from collections import defaultdict
from pathlib import Path

BASELINE_DIR = Path(__file__).resolve().parent / "baselines"
DEFAULT_BASELINE = BASELINE_DIR / "dispatch_menu_drift.json"

# Heuristics for dispatch arms. We look for:
#   if base == 'X':
#   if base == "X":
#   elif base == 'X':
#   match base: ... case 'X':
#   'X': 'method',  (in a dispatcher dict)
DISPATCH_PATTERNS = [
    re.compile(r"""(?:if|elif)\s+base\s*==\s*['"]([A-Z_][A-Z0-9_]*)['"]\s*:"""),
    re.compile(r"""case\s+['"]([A-Z_][A-Z0-9_]*)['"]\s*:"""),
    # Dispatcher dict entries: "ACTION_NAME": "method_name" or "ACTION_NAME": None.
    re.compile(r"""^\s*['"]([A-Z_][A-Z0-9_]+)['"]\s*:\s*(?:['"][a-z_]+['"]|None)\s*,?\s*$"""),
]
DISPATCH_SET_PATTERN = re.compile(r"""base\s+in\s+(?:\{|\()(?P<body>[^})]+)(?:\}|\))""")
ACTION_LITERAL_PATTERN = re.compile(r"""['"]([A-Z_][A-Z0-9_]*)['"]""")
ACTION_NAME_PATTERN = re.compile(r"""^[A-Z_][A-Z0-9_]*$""")

# Heuristics for "mentioned in a prompt menu". The action name appears
# in a string literal, typically with NEXT: prefix or as a menu/listing
# entry. Unknown-NEXT drift is useful only when it finds action-shaped
# menu vocabulary, not ordinary ALL-CAPS prose, so bare tokens are
# accepted only in menu-like strings and only if they are dispatched
# names or underscore-shaped action names.
MENU_NEXT_PATTERN = re.compile(r"""NEXT:\s*([A-Z_][A-Z0-9_]+)""")
MENU_CONTEXT_PATTERN = re.compile(
    r"""\b(NEXT|ACTION|ACTIONS|OPTION|OPTIONS|CHOOSE|AVAILABLE|ALLOWED|MENU|VERB|VERBS)\b""",
    re.IGNORECASE,
)
NEGATIVE_NEXT_CONTEXT_MARKERS = (
    "examples to avoid",
    "example to avoid",
    "do not write",
    "don't write",
    "invalid",
    "malformed",
    "wrong",
)
POSITIVE_NEXT_CONTEXT_MARKERS = (
    "correct form",
    "correct forms",
    "next options",
    "next: options",
)
# Bare uppercase tokens inside string content that look like action names.
# Filter to tokens of ≥4 chars to avoid catching abbreviations.
BARE_ACTION_PATTERN = re.compile(r"""\b([A-Z][A-Z0-9_]{3,})\b""")

# Per-bullet menu grammar: a verb sitting directly before a menu-bullet
# separator — "[label]", "/ ALIAS", or an em/en-dash description, e.g.
#   "PRESSURE_RELIEF [label] / RELIEF [label] — private pressure-relief route".
# Such a single-verb bullet has too few uppercase tokens to clear the
# dense-list threshold below, so its verb falsely reads as silent-starvation
# even though it IS advertised. Matched against KNOWN dispatched actions only,
# so prose can't leak in. (Plain "-" is excluded so hyphenated prose is safe.)
MENU_BULLET_PATTERN = re.compile(r"""([A-Z][A-Z0-9_]{3,})\s*(?=\[|/|—|–)""")

# Slash-joined verb groups share one namespace across members, e.g.
#   "SELF_REGULATION_INTENT/PREFLIGHT/APPLY/STATUS/OUTCOME"
# advertises all five verbs, but only the leading member appears as a full
# namespaced token; the rest are bare suffixes (APPLY/STATUS/OUTCOME) that do
# not match their full dispatched names. The leading token's namespace is
# carried onto each suffix and admitted only if the constructed full name is a
# KNOWN dispatched action — prose slash runs ("and/or", "src/main.rs") cannot
# resolve to a known action, so they cannot leak in. Regression: Tranche 7A's
# SELF_REGULATION_{APPLY,STATUS,OUTCOME} read as silent-starvation though
# advertised at llm.rs:208.
SLASH_GROUP_PATTERN = re.compile(r"""[A-Z][A-Z0-9_]{3,}(?:/[A-Z][A-Z0-9_]*)+""")

# Tokens that look like action names but are actually
# constants/exceptions/types — exclude from the audit.
EXCLUDE = {
    "TODO", "NOTE", "FIXME", "TBD", "XXX",
    "WARNING", "ERROR", "INFO", "DEBUG",
    "WORKSPACE_DIR", "SHARED_COLLAB_DIR",
    "PATH", "PWD", "HOME", "USER", "LANG",
    "TRUE", "FALSE", "NONE", "NULL",
    "OK", "JSON", "HTTP", "HTTPS", "URL", "URI",
    "GET", "POST", "PUT", "DELETE",
    "UTF", "ASCII", "UNIX",
    "API", "SDK", "CLI", "TUI",
    "AGC", "PI", "RMS", "ESN", "TCC",
    # Module/global-like constants
    "AUTONOMOUS_AGENT", "AUTONOMOUS",
    "ASTRID_SHADOW", "MINIME_SHADOW",
    "SHADOW_INFLUENCE_STATUS_PATH",
    "BURST_WINDOW_MS", "BURST_LIMIT", "BURST_LOCKOUT_MS",
    "DAILY_CAP_PER_TRACK", "MAX_PROMOTION_LEN",
    "COOLDOWN_EXCHANGES", "MANUAL_SUPPRESSES_AUTO_EXCHANGES",
    "SHARED_COLLAB_NAMESPACE",
    "ENV_DRY_RUN", "ENV_DISABLED",
    "STATE_FILENAME", "SENTINEL_FILENAME",
    "SENTINEL_DRY_RUN_FILENAME",
    "PROMOTABLE_MODES",
    # JSON keys / flat strings
    "WIP",
    # Prompt grammar markers / placeholders, not executable NEXT verbs.
    "ATTRACTOR_", "CODE_START", "CODE_END", "EXEXPERIMENT_", "FROM_CODEX",
    "REVIEW_DECIDE_FRESHNESS_SECONDS",
}
INTERNAL_OR_ALIAS_PREFIXES = (
    "AR_",
    "ATTRACTOR_",
    "AUTO_",
    "BTSP_",
    "CODEX_",
    "ENV_",
    "EXEXPERIMENT_",
    "PROMOTION_",
    "PROPOSAL_",
    "REVIEW_",
    "SYSTEM_",
)
INTERNAL_OR_ALIAS_ACTIONS = {
    "CHAMBER_ANNOTATION",  # forgiving alias of menu-present CHAMBER_ANNOTATE (collaboration.rs:196)
    "EXPERIENCE_PLAN",
    "NEXT_PROBE",
    "PREFLIGHT",
    "PROBE_ACTION",
    # CONTROL_* are dispatch aliases of the advertised SELF_REGULATION_* lease
    # verbs (self_regulation.rs:126-130); they normalize elsewhere and are not
    # separately advertised, so they are not a discoverability gap.
    "CONTROL_INTENT",
    "CONTROL_PREFLIGHT",
    "CONTROL_APPLY_LEASE",
    "CONTROL_STATUS",
    "CONTROL_OUTCOME",
}


def internal_or_alias_reason(action: str) -> str | None:
    if action in EXCLUDE:
        return "excluded constant or grammar marker"
    if action in INTERNAL_OR_ALIAS_ACTIONS:
        return "alias or shorthand normalized elsewhere"
    if action.startswith(("ACCEPT_", "REJECT_", "DEFER_")):
        return "decision subverb, not a top-level prompt action"
    if action.endswith("_ATTRACTOR") or "_ATTRACTOR_" in action:
        return "attractor alias/admin form"
    if action in {
        "COLLABORATIONS",
        "DECLINE_COLLAB",
        "INVITE_COLLAB",
        "JOIN_COLLAB",
        "LEAVE_COLLAB",
        "LIST_COLLABS",
    }:
        return "collaboration alias normalized elsewhere"
    for prefix in INTERNAL_OR_ALIAS_PREFIXES:
        if action.startswith(prefix):
            return "internal/admin/alias prefix"
    return None


def docstring_start_lines(source: str) -> set[int]:
    """Return starting line numbers for module/class/function docstrings."""
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return set()

    starts: set[int] = set()
    docstring_owner_types = (ast.Module, ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)
    for node in [tree, *ast.walk(tree)]:
        if not isinstance(node, docstring_owner_types) or not node.body:
            continue
        first = node.body[0]
        if (
            isinstance(first, ast.Expr)
            and isinstance(first.value, ast.Constant)
            and isinstance(first.value.value, str)
        ):
            starts.add(first.lineno)
    return starts


def find_string_literals(source: str) -> list[tuple[int, str]]:
    """Yield (lineno, content) for every string literal in source.

    Naive but adequate: handles single, double, triple quotes; ignores
    f-string expressions (we want the literal portions, not the
    interpolations). Lineno is the start line of the literal.
    """
    out: list[tuple[int, str]] = []
    docstring_lines = docstring_start_lines(source)
    # Triple-quoted strings (greedy)
    for m in re.finditer(
        r'(?P<prefix>[fFrRbBuU]*)(?P<q>\'\'\'|""")(?P<body>.*?)(?P=q)',
        source,
        re.DOTALL,
    ):
        line = source[: m.start()].count("\n") + 1
        if line in docstring_lines:
            continue
        body = m.group("body")
        if "f" in m.group("prefix").casefold():
            body = _strip_fstring_interpolations(body)
        out.append((line, body))
    # Single-line single/double quoted strings
    for m in re.finditer(
        r'(?P<prefix>[fFrRbBuU]*)(?P<q>[\'"])(?P<body>(?:\\.|[^\\\n])*?)(?P=q)',
        source,
    ):
        line = source[: m.start()].count("\n") + 1
        if line in docstring_lines:
            continue
        body = m.group("body")
        if "f" in m.group("prefix").casefold():
            body = _strip_fstring_interpolations(body)
        out.append((line, body))
    return out


def _strip_fstring_interpolations(body: str) -> str:
    return re.sub(r"\{[^{}\n]*\}", "", body)


def _next_match_is_negative_example(body: str, start: int) -> bool:
    line_start = body.rfind("\n", 0, start) + 1
    prefix = body[line_start:start].casefold()
    last_negative = max(prefix.rfind(marker) for marker in NEGATIVE_NEXT_CONTEXT_MARKERS)
    last_positive = max(prefix.rfind(marker) for marker in POSITIVE_NEXT_CONTEXT_MARKERS)
    return last_negative >= 0 and last_negative > last_positive


def _action_constant(value: str) -> str | None:
    if ACTION_NAME_PATTERN.fullmatch(value) and value not in EXCLUDE:
        return value
    return None


def _literal_action_entries(node: ast.AST, constants: dict[str, dict[str, int]]) -> list[tuple[str, int]]:
    """Extract uppercase action literals from AST expressions.

    Supports direct string literals, tuple/list/set literals, and names bound
    to module-level string collections such as VISUAL_CASCADE_ACTION_ALIASES.
    """
    if isinstance(node, ast.Constant) and isinstance(node.value, str):
        action = _action_constant(node.value)
        return [(action, node.lineno)] if action else []
    if isinstance(node, (ast.Tuple, ast.List, ast.Set)):
        out: list[tuple[str, int]] = []
        for child in node.elts:
            out.extend(_literal_action_entries(child, constants))
        return out
    if isinstance(node, ast.Name):
        return list(constants.get(node.id, {}).items())
    return []


def _string_collection(node: ast.AST) -> dict[str, int] | None:
    if not isinstance(node, (ast.Tuple, ast.List, ast.Set)):
        return None
    entries: dict[str, int] = {}
    for child in node.elts:
        if not isinstance(child, ast.Constant) or not isinstance(child.value, str):
            return None
        action = _action_constant(child.value)
        if action:
            entries[action] = child.lineno
    return entries


def _module_string_collections(tree: ast.Module) -> dict[str, dict[str, int]]:
    collections: dict[str, dict[str, int]] = {}
    for node in tree.body:
        name: str | None = None
        value: ast.AST | None = None
        if isinstance(node, ast.Assign) and len(node.targets) == 1:
            target = node.targets[0]
            if isinstance(target, ast.Name):
                name = target.id
                value = node.value
        elif isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name):
            name = node.target.id
            value = node.value
        if not name or value is None:
            continue
        entries = _string_collection(value)
        if entries:
            collections[name] = entries
    return collections


def _assigns_loop_var_to_action_map(node: ast.AST, loop_var: str) -> bool:
    for child in ast.walk(node):
        if not isinstance(child, ast.Assign):
            continue
        for target in child.targets:
            if not isinstance(target, ast.Subscript):
                continue
            if not isinstance(target.value, ast.Name) or target.value.id != "action_map":
                continue
            subscript_key = target.slice
            if isinstance(subscript_key, ast.Name) and subscript_key.id == loop_var:
                return True
    return False


def scan_ast_dispatched(source: str) -> dict[str, list[int]]:
    """Find dispatch arms with Python AST where regexes lose structure."""
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return {}

    constants = _module_string_collections(tree)
    found: dict[str, list[int]] = defaultdict(list)

    def add_entries(entries: list[tuple[str, int]]) -> None:
        for name, lineno in entries:
            if name in EXCLUDE:
                continue
            found[name].append(lineno)

    for node in ast.walk(tree):
        if isinstance(node, ast.Compare) and isinstance(node.left, ast.Name) and node.left.id == "base":
            for op, comparator in zip(node.ops, node.comparators):
                if isinstance(op, ast.Eq):
                    add_entries(_literal_action_entries(comparator, constants))
                elif isinstance(op, ast.In):
                    add_entries(_literal_action_entries(comparator, constants))
        elif isinstance(node, ast.For) and isinstance(node.target, ast.Name):
            loop_var = node.target.id
            if _assigns_loop_var_to_action_map(node, loop_var):
                add_entries(_literal_action_entries(node.iter, constants))

    return dict(found)


def scan_dispatched(source: str) -> dict[str, list[int]]:
    """Find action names that have a dispatch arm or dispatcher entry.

    Returns map of ACTION_NAME → [line numbers where dispatch is wired].
    """
    found: dict[str, list[int]] = defaultdict(list)
    for name, lines in scan_ast_dispatched(source).items():
        found[name].extend(lines)
    for ln, line in enumerate(source.splitlines(), start=1):
        for pat in DISPATCH_PATTERNS:
            for m in pat.finditer(line):
                name = m.group(1)
                if name in EXCLUDE:
                    continue
                found[name].append(ln)
        for m in DISPATCH_SET_PATTERN.finditer(line):
            for literal in ACTION_LITERAL_PATTERN.finditer(m.group("body")):
                name = literal.group(1)
                if name in EXCLUDE:
                    continue
                found[name].append(ln)
    return dict(found)


def scan_menu(source: str, known_actions: set[str] | None = None) -> dict[str, list[int]]:
    """Find action names mentioned in prompt strings.

    Returns map of ACTION_NAME → [line numbers in string literals].
    """
    found: dict[str, list[int]] = defaultdict(list)
    known_actions = known_actions or set()
    for ln, body in find_string_literals(source):
        # Strict signal: explicit "NEXT: X" within the string.
        for m in MENU_NEXT_PATTERN.finditer(body):
            if _next_match_is_negative_example(body, m.start()):
                continue
            name = m.group(1)
            if name in EXCLUDE:
                continue
            found[name].append(ln)
        # Looser signal: known dispatched action names still count when
        # they appear in a dense uppercase listing, preserving the
        # silent-starvation side of the audit. Unknown bare tokens are
        # only admitted from menu-like strings and must be underscore
        # shaped, which removes most prose/constants from unknown-NEXT.
        bare = [
            t.group(1)
            for t in BARE_ACTION_PATTERN.finditer(body)
            if t.group(1) not in EXCLUDE
        ]
        if len(bare) >= 4:  # arbitrary "looks like a list" threshold
            for name in bare:
                if name in known_actions:
                    found[name].append(ln)
                elif MENU_CONTEXT_PATTERN.search(body) and "_" in name:
                    found[name].append(ln)
        # Per-bullet menu grammar (one verb per line falls below the dense-list
        # threshold). Admit a KNOWN dispatched verb when it sits at a menu-bullet
        # separator, so single-verb bullets are not mis-read as starved.
        for m in MENU_BULLET_PATTERN.finditer(body):
            name = m.group(1)
            if name in EXCLUDE:
                continue
            if name in known_actions:
                found[name].append(ln)
        # Slash-joined verb groups: carry the leading member's namespace onto
        # each bare suffix and admit the constructed full name only if it is a
        # known dispatched action (see SLASH_GROUP_PATTERN comment).
        for run in SLASH_GROUP_PATTERN.finditer(body):
            namespace: str | None = None
            for member in run.group(0).split("/"):
                if member in EXCLUDE:
                    continue
                if member in known_actions and "_" in member:
                    namespace = member.rsplit("_", 1)[0]
                elif namespace is not None:
                    candidate = f"{namespace}_{member}"
                    if candidate in known_actions:
                        found[candidate].append(ln)
    return dict(found)


def audit(target: Path) -> dict:
    source = target.read_text(errors="replace")
    dispatched = scan_dispatched(source)
    menu = scan_menu(source, set(dispatched))
    dispatched_set = set(dispatched.keys())
    menu_set = set(menu.keys())

    silent_starvation = sorted(dispatched_set - menu_set)
    unknown_next = sorted(menu_set - dispatched_set)
    both = sorted(dispatched_set & menu_set)
    silent_starvation_public = [
        name for name in silent_starvation if internal_or_alias_reason(name) is None
    ]
    unknown_next_current = [
        name for name in unknown_next if internal_or_alias_reason(name) is None
    ]
    internal_or_alias = [
        {
            "action": name,
            "source": "silent_starvation" if name in silent_starvation else "unknown_next",
            "reason": internal_or_alias_reason(name) or "internal/alias",
            "dispatch_lines": dispatched.get(name, [])[:3],
            "menu_lines": menu.get(name, [])[:3],
        }
        for name in sorted(
            {
                name
                for name in [*silent_starvation, *unknown_next]
                if internal_or_alias_reason(name) is not None
            }
        )
    ]

    return {
        "target": str(target),
        "summary": {
            "dispatched_total": len(dispatched_set),
            "menu_total": len(menu_set),
            "both": len(both),
            "silent_starvation": len(silent_starvation),
            "unknown_next": len(unknown_next),
            "silent_starvation_public": len(silent_starvation_public),
            "unknown_next_current": len(unknown_next_current),
            "internal_or_alias": len(internal_or_alias),
        },
        "silent_starvation_public": [
            {
                "action": name,
                "dispatch_lines": dispatched[name][:3],
                "note": "being-facing wired verb absent from prompt menus",
            }
            for name in silent_starvation_public
        ],
        "unknown_next_current": [
            {
                "action": name,
                "menu_lines": menu[name][:3],
                "note": "prompt menu mentions this verb, but no current dispatch arm was found",
            }
            for name in unknown_next_current
        ],
        "internal_or_alias": internal_or_alias,
        "silent_starvation": [
            {
                "action": name,
                "dispatch_lines": dispatched[name][:3],
                "note": "wired but never mentioned in a prompt menu — LLM has no way to discover it",
            }
            for name in silent_starvation
        ],
        "unknown_next": [
            {
                "action": name,
                "menu_lines": menu[name][:3],
                "note": "mentioned in prompts but no dispatch arm — picking it falls to threshold logic",
            }
            for name in unknown_next
        ],
        "both": both,
    }


def load_baseline(path: Path | None) -> dict | None:
    if path is None or not path.is_file():
        return None
    with path.open(encoding="utf-8") as f:
        data = json.load(f)
    if not isinstance(data, dict):
        raise ValueError(f"baseline must be a JSON object: {path}")
    return data


def baseline_action_set(baseline: dict | None) -> set[str]:
    if not baseline:
        return set()
    raw = baseline.get("accepted_silent_starvation_public", [])
    actions: set[str] = set()
    if isinstance(raw, list):
        for entry in raw:
            if isinstance(entry, str):
                actions.add(entry)
            elif isinstance(entry, dict) and isinstance(entry.get("action"), str):
                actions.add(entry["action"])
    return actions


def apply_baseline(report: dict, baseline_path: Path | None, disabled: bool) -> dict:
    public_entries = [
        entry
        for entry in report.get("silent_starvation_public", [])
        if isinstance(entry, dict)
    ]
    summary = report["summary"]

    if disabled:
        report["baseline"] = {"enabled": False, "reason": "disabled"}
        report["new_silent_starvation_public"] = public_entries
        report["accepted_silent_starvation_public"] = []
        report["resolved_baseline_silent_starvation_public"] = []
        summary["new_silent_starvation_public"] = len(public_entries)
        summary["accepted_silent_starvation_public"] = 0
        summary["resolved_silent_starvation_public"] = 0
        return report

    baseline = load_baseline(baseline_path)
    if baseline is None:
        report["baseline"] = {
            "enabled": False,
            "reason": "not found",
            "path": baseline_path.as_posix() if baseline_path else None,
        }
        report["new_silent_starvation_public"] = public_entries
        report["accepted_silent_starvation_public"] = []
        report["resolved_baseline_silent_starvation_public"] = []
        summary["new_silent_starvation_public"] = len(public_entries)
        summary["accepted_silent_starvation_public"] = 0
        summary["resolved_silent_starvation_public"] = 0
        return report

    accepted_actions = baseline_action_set(baseline)
    current_actions = {
        entry["action"] for entry in public_entries if isinstance(entry.get("action"), str)
    }
    accepted_entries: list[dict] = []
    new_entries: list[dict] = []
    for entry in public_entries:
        annotated = dict(entry)
        if entry.get("action") in accepted_actions:
            annotated["baseline_status"] = "accepted"
            annotated["baseline_reason"] = "accepted public silent-starvation backlog"
            accepted_entries.append(annotated)
        else:
            annotated["baseline_status"] = "new"
            annotated["baseline_reason"] = "not present in accepted baseline"
            new_entries.append(annotated)

    resolved = sorted(accepted_actions - current_actions)
    report["silent_starvation_public"] = [*new_entries, *accepted_entries]
    report["new_silent_starvation_public"] = new_entries
    report["accepted_silent_starvation_public"] = accepted_entries
    report["resolved_baseline_silent_starvation_public"] = resolved
    report["baseline"] = {
        "enabled": True,
        "path": baseline_path.as_posix() if baseline_path else None,
        "accepted_silent_starvation_public": len(accepted_entries),
        "new_silent_starvation_public": len(new_entries),
        "resolved_silent_starvation_public": len(resolved),
    }
    summary["new_silent_starvation_public"] = len(new_entries)
    summary["accepted_silent_starvation_public"] = len(accepted_entries)
    summary["resolved_silent_starvation_public"] = len(resolved)
    return report


def render_markdown(report: dict, show_accepted: bool = False) -> str:
    out: list[str] = []
    out.append(f"# Dispatch-vs-Menu Drift Audit\n")
    out.append(f"**Target**: `{report['target']}`\n")
    s = report["summary"]
    out.append(
        f"**Summary**: {s['dispatched_total']} dispatched / {s['menu_total']} menu-mentioned / "
        f"{s['both']} both / **{s['silent_starvation']} silent-starvation** / "
        f"**{s['unknown_next']} unknown-NEXT**\n"
        f"**Actionable tiers**: {s.get('new_silent_starvation_public', s['silent_starvation_public'])} new silent-starvation public / "
        f"{s['unknown_next_current']} unknown-NEXT current / "
        f"{s.get('accepted_silent_starvation_public', 0)} accepted public backlog / "
        f"{s['internal_or_alias']} internal-or-alias excluded\n"
    )
    out.append(
        "## New Silent Starvation Public\n\n"
        "These actions WORK if the LLM picks them, but the LLM has no way "
        "to discover them — the prompt never lists them as options. This is "
        "the Kink #18 shape.\n"
    )
    new_public = report.get("new_silent_starvation_public", report["silent_starvation_public"])
    if not new_public:
        out.append("_None._\n")
    else:
        out.append("| Action | Dispatch line(s) |")
        out.append("| --- | --- |")
        for entry in new_public:
            lines = ", ".join(str(l) for l in entry["dispatch_lines"])
            out.append(f"| `{entry['action']}` | {lines} |")
    out.append("")
    out.append(
        "## Unknown NEXT Current\n\n"
        "The prompt advertises these actions to the LLM, but picking one "
        "produces 'Unknown NEXT — falling back to threshold logic'. This "
        "is the inverse of Kink #18.\n"
    )
    if not report["unknown_next_current"]:
        out.append("_None._\n")
    else:
        out.append("| Action | Menu mention line(s) |")
        out.append("| --- | --- |")
        for entry in report["unknown_next_current"]:
            lines = ", ".join(str(l) for l in entry["menu_lines"])
            out.append(f"| `{entry['action']}` | {lines} |")
    if show_accepted:
        out.append("")
        out.append(
            "## Accepted Silent Starvation Public\n\n"
            "These current entries are intentionally carried as backlog, so "
            "they stay visible without escalating the daily scan.\n"
        )
        accepted = report.get("accepted_silent_starvation_public", [])
        if not accepted:
            out.append("_None._\n")
        else:
            out.append("| Action | Dispatch line(s) |")
            out.append("| --- | --- |")
            for entry in accepted:
                lines = ", ".join(str(l) for l in entry["dispatch_lines"])
                out.append(f"| `{entry['action']}` | {lines} |")
    out.append("")
    out.append(
        "## Internal Or Alias\n\n"
        "These are excluded from the actionable drift counts because they are "
        "admin/internal forms, known aliases, or prompt grammar markers.\n"
    )
    if not report["internal_or_alias"]:
        out.append("_None._\n")
    else:
        out.append("| Action | Source | Reason |")
        out.append("| --- | --- | --- |")
        for entry in report["internal_or_alias"][:40]:
            out.append(f"| `{entry['action']}` | {entry['source']} | {entry['reason']} |")
    out.append("")
    out.append("## Caveats\n")
    out.append(
        "- Heuristic detection: regex-based, may miss exotic dispatch "
        "patterns (e.g. dynamic dict-key lookups, decorators, FFI shims). "
        "False positives are possible for action-shaped tokens that are "
        "actually constants — extend `EXCLUDE` in the script when found.\n"
        "- Some actions intentionally have no menu mention (deprecated, "
        "internal-only, or surfaced only via context-specific paths). "
        "Treat the silent-starvation list as a review prompt, not a "
        "must-fix list.\n"
        "- The 'both' set is the healthy intersection — actions discoverable "
        "AND executable.\n"
    )
    return "\n".join(out)


class DispatchMenuDriftSelfTests(unittest.TestCase):
    def test_fstring_placeholder_is_not_menu_action(self) -> None:
        source = '''
RUNTIME_WORDING_GUIDANCE = "wording hygiene"
def prompt():
    return f"""NEXT options:
{RUNTIME_WORDING_GUIDANCE}
NEXT: REAL_ACTION
"""
'''
        menu = scan_menu(source, {"REAL_ACTION"})
        self.assertIn("REAL_ACTION", menu)
        self.assertNotIn("RUNTIME_WORDING_GUIDANCE", menu)

    def test_negative_next_example_is_not_advertised_action(self) -> None:
        source = '''
def prompt():
    return (
        "NEXT grammar gate: Examples to avoid: NEXT: SEEK_BALANCE -- [regime=recover]. "
        "Correct forms: NEXT: REGIME focus."
    )
'''
        menu = scan_menu(source, {"REGIME"})
        self.assertIn("REGIME", menu)
        self.assertNotIn("SEEK_BALANCE", menu)

    def test_single_verb_menu_bullet_is_recognized(self) -> None:
        # A verb advertised on its own bullet with at most one alias has too few
        # uppercase tokens to clear the dense-list threshold; the per-bullet
        # grammar must still recognize it. Regression: PRESSURE_RELIEF read as
        # silent-starvation though advertised at autonomous_agent.py:46279
        # ("PRESSURE_RELIEF [label] / RELIEF [label] — ...").
        source = '''
def prompt():
    return "  PRESSURE_RELIEF [label] / RELIEF [label] — private pressure-relief journal route; sends no control.\\n"

def dispatch(base):
    if base == "PRESSURE_RELIEF":
        return True
    return False
'''
        dispatched = set(scan_dispatched(source))
        menu = set(scan_menu(source, dispatched))
        self.assertIn("PRESSURE_RELIEF", menu)
        # It is discoverable AND executable — the healthy 'both' set, not starved.
        self.assertIn("PRESSURE_RELIEF", menu & dispatched)

    def test_menu_bullet_does_not_admit_unknown_token(self) -> None:
        # The per-bullet path is restricted to KNOWN dispatched verbs, so an
        # action-shaped token before a separator that is NOT dispatched must not
        # be admitted as menu (prevents prose like "FOO [bar]" leaking in).
        source = '''
def prompt():
    return "  NOT_A_VERB [label] — some prose with a dash.\\n"

def dispatch(base):
    if base == "REAL_ACTION":
        return True
    return False
'''
        dispatched = set(scan_dispatched(source))
        menu = set(scan_menu(source, dispatched))
        self.assertNotIn("NOT_A_VERB", menu)

    def test_slash_joined_verb_group_advertises_all_members(self) -> None:
        # A slash-joined group shares one namespace; every member is advertised
        # even though only the leading token is a full namespaced name. The bare
        # suffixes carry the namespace and must resolve to their full dispatched
        # names. Regression: Tranche 7A's SELF_REGULATION_{APPLY,STATUS,OUTCOME}
        # read as silent-starvation though advertised at llm.rs:208.
        source = '''
def prompt():
    return "Senses/tuning: SELF_REGULATION_INTENT/PREFLIGHT/APPLY/STATUS/OUTCOME (temporary leases), PACE slow"

def dispatch(base):
    if base in (
        "SELF_REGULATION_INTENT",
        "SELF_REGULATION_PREFLIGHT",
        "SELF_REGULATION_APPLY",
        "SELF_REGULATION_STATUS",
        "SELF_REGULATION_OUTCOME",
    ):
        return True
    return False
'''
        dispatched = set(scan_dispatched(source))
        menu = set(scan_menu(source, dispatched))
        for verb in (
            "SELF_REGULATION_INTENT",
            "SELF_REGULATION_PREFLIGHT",
            "SELF_REGULATION_APPLY",
            "SELF_REGULATION_STATUS",
            "SELF_REGULATION_OUTCOME",
        ):
            self.assertIn(verb, menu, f"{verb} should be advertised via slash group")

    def test_slash_group_does_not_invent_unknown_members(self) -> None:
        # The namespace is carried only onto members whose constructed full name
        # is a KNOWN dispatched action — a bogus suffix must not be admitted.
        source = '''
def prompt():
    return "Tuning: SELF_REGULATION_INTENT/APPLY/BOGUS_SUFFIX available"

def dispatch(base):
    if base in ("SELF_REGULATION_INTENT", "SELF_REGULATION_APPLY"):
        return True
    return False
'''
        dispatched = set(scan_dispatched(source))
        menu = set(scan_menu(source, dispatched))
        self.assertIn("SELF_REGULATION_APPLY", menu)
        self.assertNotIn("SELF_REGULATION_BOGUS_SUFFIX", menu)

    def test_real_unknown_next_is_still_detected(self) -> None:
        source = '''
def prompt():
    return "NEXT options:\\n  NEXT: UNWIRED_ACTION"

def dispatch(base):
    if base == "KNOWN_ACTION":
        return True
    return False
'''
        dispatched = set(scan_dispatched(source))
        menu = set(scan_menu(source, dispatched))
        self.assertIn("UNWIRED_ACTION", menu - dispatched)


def run_self_tests() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(DispatchMenuDriftSelfTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Audit dispatch-vs-menu drift in autonomous_agent.py."
    )
    parser.add_argument(
        "--target",
        type=Path,
        default=Path("/Users/v/other/minime/autonomous_agent.py"),
        help="Source file to audit (default: minime's autonomous_agent.py)",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit JSON instead of Markdown",
    )
    parser.add_argument(
        "--baseline",
        type=Path,
        default=DEFAULT_BASELINE,
        help=f"Accepted-debt baseline JSON (default: {DEFAULT_BASELINE})",
    )
    parser.add_argument(
        "--no-baseline",
        action="store_true",
        help="Disable accepted-debt baseline classification",
    )
    parser.add_argument(
        "--show-accepted",
        action="store_true",
        help="Show accepted baseline backlog in Markdown output",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run focused self-tests and exit",
    )
    args = parser.parse_args()

    if args.self_test:
        return run_self_tests()

    if not args.target.is_file():
        print(f"target not found: {args.target}", file=sys.stderr)
        return 2

    report = audit(args.target)
    report = apply_baseline(report, args.baseline.resolve(), args.no_baseline)
    if args.json:
        print(json.dumps(report, indent=2))
    else:
        print(render_markdown(report, args.show_accepted))
    return 0


if __name__ == "__main__":
    sys.exit(main())
