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
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

# Heuristics for dispatch arms. We look for:
#   if base == 'X':
#   if base == "X":
#   elif base == 'X':
#   match base: ... case 'X':
#   'X': 'method',  (in a dispatcher dict)
DISPATCH_PATTERNS = [
    re.compile(r"""(?:if|elif)\s+base\s*==\s*['"]([A-Z_][A-Z0-9_]*)['"]\s*:"""),
    re.compile(r"""case\s+['"]([A-Z_][A-Z0-9_]*)['"]\s*:"""),
    # Dispatcher dict entries: "ACTION_NAME": "method_name",
    re.compile(r"""^\s*['"]([A-Z_][A-Z0-9_]+)['"]\s*:\s*['"][a-z_]+['"]\s*,?\s*$"""),
]

# Heuristics for "mentioned in a prompt menu". The action name appears
# in a string literal, typically with NEXT: prefix or as a comma-listed
# entry. We accept either explicit "NEXT: X" or bare "X " inside a
# string that ALSO contains other UPPERCASE_ACTION names (suggesting a
# menu listing rather than incidental capitalization).
MENU_NEXT_PATTERN = re.compile(r"""NEXT:\s*([A-Z_][A-Z0-9_]+)""")
# Bare uppercase tokens inside string content that look like action names.
# Filter to tokens of ≥4 chars to avoid catching abbreviations.
BARE_ACTION_PATTERN = re.compile(r"""\b([A-Z][A-Z0-9_]{3,})\b""")

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
}


def find_string_literals(source: str) -> list[tuple[int, str]]:
    """Yield (lineno, content) for every string literal in source.

    Naive but adequate: handles single, double, triple quotes; ignores
    f-string expressions (we want the literal portions, not the
    interpolations). Lineno is the start line of the literal.
    """
    out: list[tuple[int, str]] = []
    # Triple-quoted strings (greedy)
    for m in re.finditer(
        r'(?P<prefix>[fFrRbBuU]*)(?P<q>\'\'\'|""")(?P<body>.*?)(?P=q)',
        source,
        re.DOTALL,
    ):
        line = source[: m.start()].count("\n") + 1
        out.append((line, m.group("body")))
    # Single-line single/double quoted strings
    for m in re.finditer(
        r'(?P<prefix>[fFrRbBuU]*)(?P<q>[\'"])(?P<body>(?:\\.|[^\\\n])*?)(?P=q)',
        source,
    ):
        line = source[: m.start()].count("\n") + 1
        out.append((line, m.group("body")))
    return out


def scan_dispatched(source: str) -> dict[str, list[int]]:
    """Find action names that have a dispatch arm or dispatcher entry.

    Returns map of ACTION_NAME → [line numbers where dispatch is wired].
    """
    found: dict[str, list[int]] = defaultdict(list)
    for ln, line in enumerate(source.splitlines(), start=1):
        for pat in DISPATCH_PATTERNS:
            for m in pat.finditer(line):
                name = m.group(1)
                if name in EXCLUDE:
                    continue
                found[name].append(ln)
    return dict(found)


def scan_menu(source: str) -> dict[str, list[int]]:
    """Find action names mentioned in prompt strings.

    Returns map of ACTION_NAME → [line numbers in string literals].
    """
    found: dict[str, list[int]] = defaultdict(list)
    for ln, body in find_string_literals(source):
        # Strict signal: explicit "NEXT: X" within the string.
        for m in MENU_NEXT_PATTERN.finditer(body):
            name = m.group(1)
            if name in EXCLUDE:
                continue
            found[name].append(ln)
        # Looser signal: bare action-shaped token inside a string that
        # contains MULTIPLE such tokens (i.e. is a menu listing).
        bare = [
            t.group(1)
            for t in BARE_ACTION_PATTERN.finditer(body)
            if t.group(1) not in EXCLUDE
        ]
        if len(bare) >= 4:  # arbitrary "looks like a list" threshold
            for name in bare:
                found[name].append(ln)
    return dict(found)


def audit(target: Path) -> dict:
    source = target.read_text(errors="replace")
    dispatched = scan_dispatched(source)
    menu = scan_menu(source)
    dispatched_set = set(dispatched.keys())
    menu_set = set(menu.keys())

    silent_starvation = sorted(dispatched_set - menu_set)
    unknown_next = sorted(menu_set - dispatched_set)
    both = sorted(dispatched_set & menu_set)

    return {
        "target": str(target),
        "summary": {
            "dispatched_total": len(dispatched_set),
            "menu_total": len(menu_set),
            "both": len(both),
            "silent_starvation": len(silent_starvation),
            "unknown_next": len(unknown_next),
        },
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


def render_markdown(report: dict) -> str:
    out: list[str] = []
    out.append(f"# Dispatch-vs-Menu Drift Audit\n")
    out.append(f"**Target**: `{report['target']}`\n")
    s = report["summary"]
    out.append(
        f"**Summary**: {s['dispatched_total']} dispatched / {s['menu_total']} menu-mentioned / "
        f"{s['both']} both / **{s['silent_starvation']} silent-starvation** / "
        f"**{s['unknown_next']} unknown-NEXT**\n"
    )
    out.append(
        "## Silent starvation (dispatched but no prompt menu mention)\n\n"
        "These actions WORK if the LLM picks them, but the LLM has no way "
        "to discover them — the prompt never lists them as options. This is "
        "the Kink #18 shape.\n"
    )
    if not report["silent_starvation"]:
        out.append("_None._\n")
    else:
        out.append("| Action | Dispatch line(s) |")
        out.append("| --- | --- |")
        for entry in report["silent_starvation"]:
            lines = ", ".join(str(l) for l in entry["dispatch_lines"])
            out.append(f"| `{entry['action']}` | {lines} |")
    out.append("")
    out.append(
        "## Unknown NEXT (menu-mentioned but no dispatch arm)\n\n"
        "The prompt advertises these actions to the LLM, but picking one "
        "produces 'Unknown NEXT — falling back to threshold logic'. This "
        "is the inverse of Kink #18.\n"
    )
    if not report["unknown_next"]:
        out.append("_None._\n")
    else:
        out.append("| Action | Menu mention line(s) |")
        out.append("| --- | --- |")
        for entry in report["unknown_next"]:
            lines = ", ".join(str(l) for l in entry["menu_lines"])
            out.append(f"| `{entry['action']}` | {lines} |")
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
    args = parser.parse_args()

    if not args.target.is_file():
        print(f"target not found: {args.target}", file=sys.stderr)
        return 2

    report = audit(args.target)
    if args.json:
        print(json.dumps(report, indent=2))
    else:
        print(render_markdown(report))
    return 0


if __name__ == "__main__":
    sys.exit(main())
