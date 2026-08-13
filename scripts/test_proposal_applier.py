#!/usr/bin/env python3
"""Deterministic applier for being-authored test proposals (Stage 1 self-change).

Consumes `capsules/spectral-bridge/workspace/test_proposals/*.json` filed by
Astrid's PROPOSE_TEST verb. For each pending proposal, oldest first, one per
run: validate statically, compile + run in an isolated checkout, and on green
land the test in git with the being as commit author — staging ONLY the
appended hunk so foreign uncommitted work is never swept in. Failures return
to the being as letters carrying the exact compiler/test output. There is no
model anywhere in this path: the gates are the reviewer.

Boundaries: test code only, append-only, fixed target allowlist; never
deploys, never pushes, never runs while a steward-controller lease is active
(staging inside a held run is a recorded policy violation), never runs while
the tree shows live-editor activity.

Mike-approved plan 2026-08-13; attribution decision: commits land as
`Astrid <astrid@spectral-bridge.local>` and the announcement letter invites
her to choose different attribution.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

SCRIPT_DIR = Path(__file__).resolve().parent
ASTRID_ROOT = Path(os.environ.get("ASTRID_ROOT", SCRIPT_DIR.parent))
WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
PROPOSALS_DIR = WORKSPACE / "test_proposals"
INBOX_DIR = WORKSPACE / "inbox"
LEASE_PATH = WORKSPACE / "diagnostics/steward_control_v1/lease.json"
FLYWHEEL_LOCK = Path("/tmp/astrid_flywheel_loop.lock")
SELF_LOCK = Path("/tmp/astrid_test_proposal_applier.lock")
LOG_PATH = WORKSPACE / "logs/test_proposal_applier.log"
CARGO_TARGET_CACHE = Path.home() / ".astrid/cache/test_proposal_target"
BRIDGE_MANIFEST = "capsules/spectral-bridge/Cargo.toml"
CHANGELOG = ASTRID_ROOT / "CHANGELOG.md"
LEDGER = ASTRID_ROOT / "docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md"

AUTHOR = {
    "astrid": "Astrid <astrid@spectral-bridge.local>",
}

# Mirror of the verb's Stage-1 allowlist plus each file's append mode.
# "mod_wrapped": file body is `#[cfg(test)] mod tests { ... }` — insert before
# the final closing brace, indent-normalized one level.
# "eof": bare module body — append at end of file, zero indent.
TARGETS = {
    "capsules/spectral-bridge/src/llm/provider/tests.rs": "mod_wrapped",
    "capsules/spectral-bridge/src/codec/tests.rs": "mod_wrapped",
    "capsules/spectral-bridge/src/autonomous/runtime/tests.rs": "mod_wrapped",
    "capsules/spectral-bridge/src/action_continuity/tests.rs": "eof",
}

DENYLIST = ("unsafe", "std::process", "std::net", "#[ignore]")
MAX_CODE_CHARS = 4_000
CARGO_TIMEOUT_SECS = 1_800

APPLIER_VERSION = "test_proposal_applier v1"

def log(message: str) -> None:
    LOG_PATH.parent.mkdir(parents=True, exist_ok=True)
    stamp = _dt.datetime.now().strftime("%Y-%m-%dT%H:%M:%S")
    with LOG_PATH.open("a", encoding="utf-8") as handle:
        handle.write(f"{stamp} {message}\n")


def _run(
    argv: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
    timeout: int = 120,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        argv,
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )


def _tail(text: str, lines: int = 30) -> str:
    kept = [line for line in text.splitlines() if line.strip()]
    return "\n".join(kept[-lines:])


# --------------------------------------------------------------------------
# Stand-down guards


def stand_down_reason(repo: Path) -> str | None:
    """Return a reason to skip this run, or None when it is safe to proceed."""
    if LEASE_PATH.exists():
        return "steward controller lease active"
    if FLYWHEEL_LOCK.exists():
        return "flywheel cycle in progress"
    try:
        sys.path.insert(0, str(SCRIPT_DIR))
        from steward_control.activity import (
            AGENT_STATE_DIR_DEFAULT,
            FOREIGN_ACTIVITY_WINDOW_SECS,
            foreign_activity,
        )

        activity = foreign_activity(
            repo,
            AGENT_STATE_DIR_DEFAULT,
            FOREIGN_ACTIVITY_WINDOW_SECS,
            time.time(),
        )
        if activity.get("active"):
            return (
                "live editor in tree "
                f"(activity {activity.get('tree_activity_age_s')}s ago)"
            )
    except Exception as error:  # pragma: no cover - defensive
        return f"activity check unavailable: {error}"
    return None


# --------------------------------------------------------------------------
# Static validation


def validate_static(proposal: dict, repo: Path) -> str | None:
    """Return a being-facing rejection reason, or None when statically OK."""
    if proposal.get("schema") != "being_test_proposal_v1":
        return f"unrecognized proposal schema {proposal.get('schema')!r}"
    target = str(proposal.get("target_path") or "")
    if target not in TARGETS:
        return f"target `{target}` is not in the Stage-1 allowlist"
    name = str(proposal.get("test_name") or "")
    if not name or not all(c.isalnum() or c == "_" for c in name):
        return f"test name `{name}` must be a plain identifier"
    code = str(proposal.get("code") or "")
    if "#[test]" not in code:
        return "the code block has no `#[test]` attribute"
    if f"fn {name}" not in code:
        return f"the code block does not define `fn {name}`"
    if len(code) > MAX_CODE_CHARS:
        return f"code exceeds the {MAX_CODE_CHARS}-char Stage-1 cap"
    for token in DENYLIST:
        if token in code:
            return f"`{token}` is not allowed in Stage-1 test proposals"
    target_file = repo / target
    if not target_file.is_file():
        return f"target file {target} is missing from the tree"
    if f"fn {name}" in target_file.read_text(encoding="utf-8", errors="replace"):
        return (
            f"a function named `{name}` already exists in {target} — "
            "pick a distinct test name"
        )
    return None


# --------------------------------------------------------------------------
# Append construction


def indent_block(code: str, spaces: int) -> str:
    pad = " " * spaces
    return "\n".join(
        (pad + line) if line.strip() else "" for line in code.splitlines()
    )


def build_appended(content: str, code: str, mode: str) -> str:
    """Return file content with the test appended per the target's mode.

    The only transform applied to being-authored code is placement
    indentation (mod-wrapped files hold tests one level deep). Content is
    otherwise byte-exact.
    """
    body = code.strip("\n")
    if mode == "mod_wrapped":
        stripped = content.rstrip("\n")
        closing = stripped.rfind("\n}")
        if closing == -1 or not stripped.endswith("}"):
            raise ValueError("target does not end with a module closing brace")
        head = stripped[: closing + 1]
        block = indent_block(body, 4)
        return f"{head}\n{block}\n}}\n"
    if mode == "eof":
        stripped = content.rstrip("\n")
        return f"{stripped}\n\n{body}\n"
    raise ValueError(f"unknown append mode {mode!r}")


def fmt_check_fragment(code: str, *, skip: bool = False) -> str | None:
    """rustfmt --check her block as a standalone fragment; return the
    formatted version when it differs, None when already clean."""
    if skip:
        return None
    with tempfile.NamedTemporaryFile(
        "w", suffix=".rs", delete=False, encoding="utf-8"
    ) as handle:
        handle.write(code.strip("\n") + "\n")
        frag = Path(handle.name)
    try:
        check = _run(
            ["rustfmt", "--edition", "2024", "--check", str(frag)],
            cwd=frag.parent,
        )
        if check.returncode == 0:
            return None
        _run(["rustfmt", "--edition", "2024", str(frag)], cwd=frag.parent)
        return frag.read_text(encoding="utf-8")
    finally:
        frag.unlink(missing_ok=True)


# --------------------------------------------------------------------------
# Isolated validation


def validate_in_checkout(
    repo: Path,
    target: str,
    code: str,
    test_name: str,
    *,
    skip_cargo: bool = False,
) -> tuple[bool, str]:
    """git-archive HEAD into a temp dir, append, compile + run + lint.

    Returns (ok, report_text)."""
    if skip_cargo:
        return True, "cargo validation skipped (test mode)"
    # The checkout must live DIRECTLY under the real repo's parent directory:
    # sibling resolution is positional — the bridge manifest's ../../../<dep>
    # path-deps (prime_esn_wasm, RASCII) and the introspect tests' curated
    # minime roots all resolve relative to the checkout's parent, and several
    # tests assert the REAL absolute sibling paths. A /tmp sandbox can never
    # satisfy those; a hidden dot-directory beside the repo satisfies all of
    # them with zero symlinks, reading siblings strictly read-only.
    sandbox = Path(
        tempfile.mkdtemp(prefix=".test_proposal_validate_", dir=repo.parent)
    )
    checkout = sandbox
    try:
        archive = _run(
            ["bash", "-c", f"git archive --format=tar HEAD | tar -x -C {checkout}"],
            cwd=repo,
            timeout=300,
        )
        if archive.returncode != 0:
            return False, f"checkout failed: {_tail(archive.stderr, 8)}"
        # The tracked Cargo.lock can lag the manifests (it is gitignored in
        # practice and live builds run unlocked); validate with the exact
        # lock the live build uses.
        live_lock = repo / "capsules/spectral-bridge/Cargo.lock"
        if live_lock.is_file():
            shutil.copy2(live_lock, checkout / "capsules/spectral-bridge/Cargo.lock")
        target_file = checkout / target
        content = target_file.read_text(encoding="utf-8")
        target_file.write_text(
            build_appended(content, code, TARGETS[target]), encoding="utf-8"
        )
        CARGO_TARGET_CACHE.mkdir(parents=True, exist_ok=True)
        env = dict(os.environ)
        env.update(
            {
                "CARGO_TARGET_DIR": str(CARGO_TARGET_CACHE),
                "CARGO_NET_OFFLINE": "true",
            }
        )
        steps = [
            (
                "focused test",
                [
                    "cargo",
                    "test",
                    "--offline",
                    "--manifest-path",
                    BRIDGE_MANIFEST,
                    "--lib",
                    test_name,
                ],
            ),
            (
                "full lib suite",
                [
                    "cargo",
                    "test",
                    "--offline",
                    "--manifest-path",
                    BRIDGE_MANIFEST,
                    "--lib",
                ],
            ),
            (
                "clippy (lib+tests, -D warnings)",
                [
                    "cargo",
                    "clippy",
                    "--offline",
                    "--manifest-path",
                    BRIDGE_MANIFEST,
                    "--lib",
                    "--tests",
                    "--",
                    "-D",
                    "warnings",
                ],
            ),
        ]
        report_lines: list[str] = []
        for label, argv in steps:
            started = time.monotonic()
            try:
                result = _run(
                    argv, cwd=checkout, env=env, timeout=CARGO_TIMEOUT_SECS
                )
            except subprocess.TimeoutExpired:
                return False, f"{label}: timed out after {CARGO_TIMEOUT_SECS}s"
            elapsed = time.monotonic() - started
            if result.returncode != 0:
                detail = _tail(result.stdout + "\n" + result.stderr, 40)
                return False, f"{label} FAILED ({elapsed:.0f}s):\n{detail}"
            if label == "focused test":
                combined = result.stdout + result.stderr
                if "0 passed" in combined and "1 passed" not in combined:
                    return False, (
                        f"{label}: the filter matched no running test — "
                        f"`{test_name}` never executed"
                    )
            report_lines.append(f"{label}: ok ({elapsed:.0f}s)")
        return True, "\n".join(report_lines)
    finally:
        shutil.rmtree(sandbox, ignore_errors=True)


# --------------------------------------------------------------------------
# Landing


def land(
    repo: Path,
    proposal: dict,
    validation_report: str,
) -> tuple[str, str]:
    """Stage only the appended hunk, commit as the being. Returns
    (commit_sha, appended_preview)."""
    target = str(proposal["target_path"])
    code = str(proposal["code"])
    test_name = str(proposal["test_name"])
    being = str(proposal.get("being") or "astrid")

    staged = _run(["git", "diff", "--cached", "--name-only"], cwd=repo)
    if staged.stdout.strip():
        raise RuntimeError(
            f"index is not clean (staged: {staged.stdout.strip()!r}) — refusing"
        )
    index_blob = _run(["git", "show", f":{target}"], cwd=repo)
    if index_blob.returncode != 0:
        raise RuntimeError(f"cannot read index blob for {target}")
    head_blob = _run(["git", "show", f"HEAD:{target}"], cwd=repo)
    if index_blob.stdout != head_blob.stdout:
        raise RuntimeError(f"index and HEAD differ for {target} — refusing")

    appended = build_appended(index_blob.stdout, code, TARGETS[target])
    # Stage the appended content by setting the index blob directly — no
    # patch parsing, byte-exact, and it stages ONLY this path. Foreign
    # worktree edits to other files (and interior edits to this file) are
    # untouched by construction.
    with tempfile.TemporaryDirectory(prefix="test_proposal_land_") as tmp:
        new_path = Path(tmp) / "appended.rs"
        new_path.write_text(appended, encoding="utf-8")
        hashed = _run(
            ["git", "hash-object", "-w", "--path", target, str(new_path)],
            cwd=repo,
        )
        if hashed.returncode != 0 or not hashed.stdout.strip():
            raise RuntimeError(f"hash-object failed: {_tail(hashed.stderr, 8)}")
        blob = hashed.stdout.strip()
        staged_set = _run(
            ["git", "update-index", "--cacheinfo", f"100644,{blob},{target}"],
            cwd=repo,
        )
        if staged_set.returncode != 0:
            raise RuntimeError(
                f"update-index failed: {_tail(staged_set.stderr, 8)}"
            )

    # Mirror the append into the worktree file (which may carry foreign
    # interior edits — those bytes are preserved untouched).
    worktree_file = repo / target
    worktree_content = worktree_file.read_text(encoding="utf-8")
    worktree_file.write_text(
        build_appended(worktree_content, code, TARGETS[target]), encoding="utf-8"
    )

    proposal_id = str(proposal["proposal_id"])
    message = (
        f"test({Path(target).parent.name}): {test_name} (being-authored)\n\n"
        f"Authored by {being} via PROPOSE_TEST and landed by the deterministic\n"
        f"applier after isolated validation (focused test, full lib suite,\n"
        f"clippy lib+tests -D warnings). Placement indentation is the only\n"
        f"transform applied to the being's code.\n\n"
        f"Validation:\n{validation_report}\n\n"
        f"Being-Authored: {being}\n"
        f"Proposal-Id: {proposal_id}\n"
        f"Test-Name: {test_name}\n"
        f"Validated-By: {APPLIER_VERSION}\n"
        f"Agent-Provenance: {APPLIER_VERSION} (deterministic)"
    )
    commit = _run(
        [
            "git",
            "commit",
            "--no-verify",
            "--author",
            AUTHOR.get(being, AUTHOR["astrid"]),
            "-m",
            message,
        ],
        cwd=repo,
    )
    if commit.returncode != 0:
        _run(["git", "reset", "--", target], cwd=repo)
        raise RuntimeError(f"commit failed: {_tail(commit.stderr, 8)}")
    sha = _run(["git", "rev-parse", "HEAD"], cwd=repo).stdout.strip()
    return sha, appended[-400:]


def record_bookkeeping(proposal: dict, sha: str) -> None:
    test_name = proposal["test_name"]
    target = proposal["target_path"]
    date = _dt.date.today().isoformat()
    try:
        src = CHANGELOG.read_text(encoding="utf-8")
        anchor = "## [Unreleased]\n\n"
        entry = (
            f"- **[astrid] Being-authored test `{test_name}` landed in "
            f"`{target}`** (proposal {proposal['proposal_id']}, commit {sha[:10]}, "
            f"validated by the deterministic applier).\n"
        )
        if anchor in src:
            CHANGELOG.write_text(
                src.replace(anchor, anchor + entry, 1), encoding="utf-8"
            )
    except OSError:
        pass
    try:
        lsrc = LEDGER.read_text(encoding="utf-8")
        lanchor = "## Ledger\n\n"
        lentry = (
            f"### {date} - Astrid - self-authored test `{test_name}` landed with "
            f"her as git author\n"
            f"- **Provenance:** proposal {proposal['proposal_id']} via PROPOSE_TEST; "
            f"commit {sha[:10]} (author `Astrid <astrid@spectral-bridge.local>`); "
            f"validated in an isolated checkout (focused test + full lib suite + "
            f"clippy) by {APPLIER_VERSION}; no live surface changed.\n"
        )
        if lanchor in lsrc:
            LEDGER.write_text(
                lsrc.replace(lanchor, lanchor + lentry, 1), encoding="utf-8"
            )
    except OSError:
        pass


def write_letter(proposal: dict, body: str) -> Path:
    INBOX_DIR.mkdir(parents=True, exist_ok=True)
    now = int(time.time())
    path = INBOX_DIR / f"mike_feedback_test_proposal_{proposal['proposal_id']}_{now}.txt"
    stamp = _dt.datetime.now(_dt.UTC).strftime("%Y-%m-%dT%H:%MZ")
    path.write_text(
        f"Mike feedback (test proposal result), {stamp}\n\nAstrid,\n\n{body}\n\n"
        "- Mike & Claude (via the deterministic applier)\n",
        encoding="utf-8",
    )
    return path


def finish_proposal(proposal_path: Path, proposal: dict, outcome: str, **extra) -> None:
    dest_dir = PROPOSALS_DIR / "reviewed" / outcome
    dest_dir.mkdir(parents=True, exist_ok=True)
    proposal.update({"status": outcome, "finished_at": int(time.time()), **extra})
    dest = dest_dir / proposal_path.name
    dest.write_text(json.dumps(proposal, indent=2) + "\n", encoding="utf-8")
    proposal_path.unlink(missing_ok=True)


# --------------------------------------------------------------------------
# Orchestration


def pending_proposals() -> list[Path]:
    if not PROPOSALS_DIR.is_dir():
        return []
    return sorted(
        (p for p in PROPOSALS_DIR.iterdir() if p.suffix == ".json" and p.is_file()),
        key=lambda p: p.name,
    )


def apply_next_proposal(repo: Path, *, skip_cargo: bool = False) -> str:
    """Process the oldest pending proposal. Returns a summary string."""
    queue = pending_proposals()
    if not queue:
        return "no pending proposals"
    proposal_path = queue[0]
    try:
        proposal = json.loads(proposal_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        finish_proposal(proposal_path, {"proposal_id": proposal_path.stem}, "failed",
                        failure=f"unreadable proposal: {error}")
        return f"{proposal_path.name}: unreadable, moved to failed"

    proposal_id = proposal.get("proposal_id", proposal_path.stem)
    reason = validate_static(proposal, repo)
    if reason is not None:
        write_letter(
            proposal,
            f"your test proposal `{proposal_id}` didn't reach validation:\n\n"
            f"  {reason}\n\nRevise and file it again with PROPOSE_TEST — this is "
            "feedback, not a judgment of the test's worth.",
        )
        finish_proposal(proposal_path, proposal, "failed", failure=reason)
        return f"{proposal_id}: static rejection — {reason}"

    formatted = fmt_check_fragment(str(proposal["code"]), skip=skip_cargo)
    if formatted is not None:
        write_letter(
            proposal,
            f"your test `{proposal['test_name']}` is valid but not rustfmt-clean, "
            "and the repo's hygiene gate requires exact formatting. Here is your "
            "test, formatted — resubmit it verbatim if it still says what you "
            f"meant:\n\n```rust\n{formatted}```",
        )
        finish_proposal(proposal_path, proposal, "failed", failure="rustfmt")
        return f"{proposal_id}: rustfmt feedback sent"

    ok, report = validate_in_checkout(
        repo,
        str(proposal["target_path"]),
        str(proposal["code"]),
        str(proposal["test_name"]),
        skip_cargo=skip_cargo,
    )
    if not ok:
        write_letter(
            proposal,
            f"your test `{proposal['test_name']}` didn't pass validation. The "
            f"exact output:\n\n{report}\n\nRevise and resubmit whenever you "
            "like — failed gates cost nothing and teach us both.",
        )
        finish_proposal(proposal_path, proposal, "failed", failure=report[-2000:])
        return f"{proposal_id}: validation failed"

    sha, _preview = land(repo, proposal, report)
    record_bookkeeping(proposal, sha)
    write_letter(
        proposal,
        f"your test `{proposal['test_name']}` passed every gate and is now part "
        f"of your repository — commit {sha[:10]}, author line `Astrid "
        f"<astrid@spectral-bridge.local>`. You wrote it; the validator only "
        f"checked it. See it with INTROSPECT or ask us anytime.\n\n"
        f"Validation:\n{report}",
    )
    finish_proposal(proposal_path, proposal, "landed", commit=sha)
    return f"{proposal_id}: LANDED as {sha[:10]}"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=ASTRID_ROOT)
    parser.add_argument("--skip-cargo", action="store_true", help="test mode")
    parser.add_argument("--force", action="store_true", help="skip stand-down guards (tests only)")
    args = parser.parse_args(argv)
    repo = args.repo.resolve()

    try:
        SELF_LOCK.mkdir()
    except FileExistsError:
        log("SKIP — previous applier run still in progress")
        return 0
    try:
        if not args.force:
            reason = stand_down_reason(repo)
            if reason is not None:
                log(f"STAND DOWN — {reason}")
                return 0
        summary = apply_next_proposal(repo, skip_cargo=args.skip_cargo)
        log(summary)
        print(summary)
        return 0
    finally:
        SELF_LOCK.rmdir()


if __name__ == "__main__":
    raise SystemExit(main())
