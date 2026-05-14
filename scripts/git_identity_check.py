#!/usr/bin/env python3
"""Diagnostic for git author identity asymmetry across the consciousness-bridge sibling repos.

The three repos (astrid, minime, neural-triple-reservoir) share an AI agent that commits to
all of them. Git identity setup has historically been asymmetric:

  - astrid: local user.name=Codex, user.email=codex@localhost
  - minime: local user.name=Codex, user.email=codex@localhost
  - neural-triple-reservoir: NO local config -> commits fail with "Author identity unknown"

Today this surfaced when committing v5.1 Phase A's collab_feeder.py + plist to the reservoir
repo: the commit failed and was salvaged by the one-shot workaround
`git -c user.name=Codex -c user.email=codex@localhost commit ...`. CLAUDE.md forbids the AI
from running `git config` mutations, so this diagnostic is read-only — it surfaces the
asymmetry and suggests commands the steward could run, without changing anything.

Identity choice is a USER preference (Codex for explicit AI attribution? Volya to match
historical reservoir commits? Something else?). The script does not recommend; it surfaces.

Usage:
  python3 scripts/git_identity_check.py
  python3 scripts/git_identity_check.py /Users/v/other/foo /Users/v/other/bar
  python3 scripts/git_identity_check.py --json
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import Counter
from dataclasses import asdict, dataclass, field
from pathlib import Path


DEFAULT_REPOS = [
    "/Users/v/other/astrid",
    "/Users/v/other/minime",
    "/Users/v/other/neural-triple-reservoir",
]


@dataclass
class RepoIdentity:
    path: str
    branch: str = ""
    local_name: str = ""
    local_email: str = ""
    global_name: str = ""
    global_email: str = ""
    effective_name: str = ""
    effective_email: str = ""
    can_commit: bool = False
    recent_authors: list[tuple[str, str, int]] = field(default_factory=list)  # (name, email, count)
    error: str = ""


def _git(repo: Path, *args: str) -> str:
    """Run a git command and return stdout, or empty string on failure."""
    try:
        result = subprocess.run(
            ["git", *args],
            cwd=str(repo),
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            return ""
        return result.stdout.strip()
    except (FileNotFoundError, OSError):
        return ""


def collect_repo(repo_path: Path) -> RepoIdentity:
    out = RepoIdentity(path=repo_path.as_posix())
    if not (repo_path / ".git").exists():
        out.error = ".git directory not found — not a repo or wrong path"
        return out

    out.branch = _git(repo_path, "branch", "--show-current")

    # Local config
    out.local_name = _git(repo_path, "config", "--local", "--get", "user.name")
    out.local_email = _git(repo_path, "config", "--local", "--get", "user.email")

    # Global config (read-only — never modified by this script)
    out.global_name = _git(repo_path, "config", "--global", "--get", "user.name")
    out.global_email = _git(repo_path, "config", "--global", "--get", "user.email")

    # Effective (what `git commit` would actually use)
    out.effective_name = out.local_name or out.global_name
    out.effective_email = out.local_email or out.global_email
    out.can_commit = bool(out.effective_name and out.effective_email)

    # Recent committer attribution — last 20 commits, grouped
    log_out = _git(repo_path, "log", "--pretty=format:%an\t%ae", "-20")
    counter: Counter = Counter()
    for line in log_out.splitlines():
        if "\t" in line:
            name, email = line.split("\t", 1)
            counter[(name, email)] += 1
    out.recent_authors = [(n, e, c) for (n, e), c in counter.most_common()]

    return out


def analyze_asymmetry(repos: list[RepoIdentity]) -> list[str]:
    """Return human-readable findings about asymmetry across repos."""
    findings: list[str] = []
    valid = [r for r in repos if not r.error]
    if not valid:
        return ["No valid repos found — check paths."]

    # Group by effective identity
    identities = {(r.effective_name, r.effective_email) for r in valid}
    if len(identities) == 1 and ("", "") not in identities:
        ident = next(iter(identities))
        findings.append(f"All {len(valid)} repos share effective identity: {ident[0]} <{ident[1]}>.")
    else:
        findings.append(f"{len(identities)} distinct effective identities across {len(valid)} repos.")

    # Find can't-commit repos
    cannot_commit = [r for r in valid if not r.can_commit]
    if cannot_commit:
        findings.append(
            f"{len(cannot_commit)} repo(s) cannot commit (no effective identity): "
            + ", ".join(Path(r.path).name for r in cannot_commit)
        )

    # Find repos where local differs from history's dominant author
    for r in valid:
        if not r.recent_authors:
            continue
        dominant_name, dominant_email, _ = r.recent_authors[0]
        if r.effective_email and r.effective_email != dominant_email:
            findings.append(
                f"{Path(r.path).name}: effective `{r.effective_email}` differs from "
                f"dominant historical `{dominant_email}` ({r.recent_authors[0][2]}/20 recent commits)"
            )

    return findings


def render_markdown(repos: list[RepoIdentity], findings: list[str]) -> str:
    lines = [
        "# Git Identity Check",
        "",
        "Read-only diagnostic. Surfaces the asymmetry across known repos so the steward can decide.",
        "Per CLAUDE.md, this script never modifies git config.",
        "",
        f"- Repos checked: `{len(repos)}`",
        f"- Findings below — none, one, or several may indicate an action item.",
        "",
        "## Per-repo identity",
        "",
        "| Repo | Branch | Local user.email | Global user.email | Effective | Can commit? |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for r in repos:
        if r.error:
            lines.append(f"| `{Path(r.path).name}` | — | — | — | — | ⚠ {r.error} |")
            continue
        local = r.local_email or "_(unset)_"
        global_ = r.global_email or "_(unset)_"
        effective = r.effective_email or "_(none — commits will fail)_"
        ok = "✓" if r.can_commit else "✗"
        lines.append(
            f"| `{Path(r.path).name}` | `{r.branch or '?'}` | {local} | {global_} | {effective} | {ok} |"
        )

    lines.append("")
    lines.append("## Recent committer attribution (last 20 commits, grouped)")
    lines.append("")
    for r in repos:
        if r.error or not r.recent_authors:
            continue
        lines.append(f"### `{Path(r.path).name}`")
        lines.append("")
        lines.append("| Author name | Author email | Recent commits |")
        lines.append("| --- | --- | ---: |")
        for name, email, count in r.recent_authors[:5]:
            lines.append(f"| {name} | {email} | {count} |")
        lines.append("")

    lines.append("## Findings")
    lines.append("")
    if not findings:
        lines.append("_(no asymmetries detected; all repos can commit with consistent identity)_")
    else:
        for f in findings:
            lines.append(f"- {f}")
    lines.append("")

    lines.append("## Suggested commands (the steward decides)")
    lines.append("")
    lines.append("Per CLAUDE.md, this script does not run any of these — it surfaces them for the steward.")
    lines.append("")
    lines.append("**To set a local identity on a repo (preferred for AI-attributed work):**")
    lines.append("```bash")
    lines.append("# In the repo root:")
    lines.append("git config --local user.name 'Codex'")
    lines.append("git config --local user.email 'codex@localhost'")
    lines.append("```")
    lines.append("")
    lines.append("**For one-shot commits without changing config (when AI session needs to commit on a misconfigured repo):**")
    lines.append("```bash")
    lines.append("git -c user.name='Codex' -c user.email='codex@localhost' commit -m '...'")
    lines.append("```")
    lines.append("")
    lines.append("**To match historical attribution (if you'd rather AI commits look like your own):**")
    lines.append("```bash")
    lines.append("# In the repo root:")
    lines.append("git config --local user.name '<name from recent attribution table above>'")
    lines.append("git config --local user.email '<email from recent attribution table above>'")
    lines.append("```")
    lines.append("")
    lines.append("Identity is a user preference (Codex for AI clarity? Match history for uniform attribution?).")
    lines.append("This script never picks; the steward does.")
    return "\n".join(lines)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Diagnostic for git author identity asymmetry across sibling repos.",
        epilog="Read-only. Per CLAUDE.md, this script never modifies git config.",
    )
    parser.add_argument(
        "repos",
        nargs="*",
        default=DEFAULT_REPOS,
        help=f"Repo paths to check (default: {DEFAULT_REPOS})",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit machine-readable JSON instead of Markdown.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repos = [collect_repo(Path(p)) for p in args.repos]
    findings = analyze_asymmetry(repos)
    if args.json:
        print(json.dumps(
            {
                "repos": [asdict(r) for r in repos],
                "findings": findings,
            },
            indent=2,
            sort_keys=True,
        ))
    else:
        print(render_markdown(repos, findings))
    return 0


if __name__ == "__main__":
    sys.exit(main())
