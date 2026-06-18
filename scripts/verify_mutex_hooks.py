#!/usr/bin/env python3
"""verify_mutex_hooks.py — keep the steward mutex's INTERACTIVE side from failing SILENTLY.

The cross-steward mutex (scripts/steward_mutex.py) has two sides. The LOOP side
(steward_loop_run.sh) is robust. The INTERACTIVE side depends on Claude Code
hooks in .claude/settings.local.json — and those can break SILENTLY two ways:
  (1) CONFIG LOSS — settings.local.json edited/rewritten, the hooks dropped;
  (2) SCHEMA DRIFT — a Claude Code upgrade changes how hooks are declared, so the
      (now-misshapen) hooks still parse as JSON but stop firing.
Either way an interactive session would quietly stop holding the mutex and the
loop would race it again — while believing it's protected. That false confidence
is the worst failure (the exact lesson of feedback_un_muffle_invariant, turned on
our own guard).

This makes both modes LOUD:
  - CONFIG CHECK  (offline, cheap): assert settings.local.json still declares the
                  3 hooks (SessionStart/PreToolUse/SessionEnd) wired to steward_mutex.py.
  - VERSION TRIPWIRE (offline, cheap): record `claude --version`; on change, ALARM
                  ("re-verify the hooks") and auto-run the canary.
  - CANARY (bold): spawn an ISOLATED `claude -p` (own throwaway lock dir via
                  STEWARD_MUTEX_LOCK_DIR; STEWARD_LOOP unset) and confirm a hook
                  actually fired — proves the mechanism against the live CC version
                  rather than just flagging the risk.

Exit 0 = healthy + unchanged. Exit 2 = ALARM (config broken / version changed /
canary FAILED) — investigate before trusting the interactive mutex. Steward-only.

  (default)    config + version checks; auto-canary ONLY when the version changed.
  --canary     force the end-to-end canary now.
  --no-canary  skip the canary even on a version change (fast / offline).
  --self-test
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path("/Users/v/other/astrid")
SETTINGS = REPO / ".claude" / "settings.local.json"
VERSION_STATE = Path("~/.astrid/run/claude_version_seen.txt").expanduser()
EXPECTED_EVENTS = ("SessionStart", "PreToolUse", "SessionEnd")


def check_hooks_config(settings_path: Path) -> list[str]:
    """Return a list of problems; empty = the 3 mutex hooks are present + wired."""
    try:
        data = json.loads(Path(settings_path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as e:
        return [f"cannot read/parse {settings_path}: {e}"]
    hooks = data.get("hooks") or {}
    problems = []
    for ev in EXPECTED_EVENTS:
        groups = hooks.get(ev) or []
        cmds = [h.get("command", "") for g in groups for h in (g.get("hooks") or [])]
        if not any("steward_mutex.py" in c for c in cmds):
            problems.append(f"hook '{ev}' missing or not wired to steward_mutex.py")
    return problems


def claude_version() -> str | None:
    try:
        out = subprocess.run(["claude", "--version"], capture_output=True, text=True, timeout=15)
    except (OSError, subprocess.SubprocessError):
        return None
    return (out.stdout.strip() or out.stderr.strip()) or None


def version_tripwire(state_path: Path, current: str | None) -> tuple[bool, str | None]:
    """Compare current `claude --version` to the last seen; persist current.
    Returns (changed, last_seen). changed is False on first-ever run (no baseline)."""
    p = Path(state_path)
    try:
        last = p.read_text(encoding="utf-8").strip() or None
    except OSError:
        last = None
    if current:
        try:
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(current, encoding="utf-8")
        except OSError:
            pass
    changed = current is not None and last is not None and current != last
    return changed, last


def run_canary(repo: Path = REPO, timeout_s: float = 90.0) -> tuple[str, str]:
    """Prove the interactive hooks fire against the LIVE Claude Code version.

    Spawns `claude -p` in an isolated env (own throwaway lock dir via
    STEWARD_MUTEX_LOCK_DIR; STEWARD_LOOP unset so the interactive hooks are NOT
    skipped) and asks it to run one bash command that, IF a PreToolUse hook
    acquired the lock, will own it and drop a sentinel file. Returns
    ('PASS'|'FAIL'|'INCONCLUSIVE', detail). Best-effort: never raises."""
    tmp = Path(tempfile.mkdtemp(prefix="mutex_canary_"))
    lock_dir = tmp / "lock"
    sentinel = tmp / "HOOKS_OK"
    env = dict(os.environ)
    env["STEWARD_MUTEX_LOCK_DIR"] = str(lock_dir)
    env.pop("STEWARD_LOOP", None)
    prompt = (
        "Run exactly this one bash command and then stop, reporting only its result:\n"
        f'python3 {repo}/scripts/steward_mutex.py owns --holder "interactive:$PPID" '
        f"&& touch {sentinel}"
    )
    try:
        subprocess.run(
            ["claude", "-p", "--dangerously-skip-permissions", prompt],
            cwd=str(repo), env=env, capture_output=True, text=True, timeout=timeout_s,
        )
    except subprocess.TimeoutExpired:
        shutil.rmtree(tmp, ignore_errors=True)
        return "INCONCLUSIVE", f"canary claude -p timed out after {timeout_s:.0f}s"
    except (OSError, subprocess.SubprocessError) as e:
        shutil.rmtree(tmp, ignore_errors=True)
        return "INCONCLUSIVE", f"canary spawn failed: {e}"
    fired = sentinel.exists()
    shutil.rmtree(tmp, ignore_errors=True)
    if fired:
        return "PASS", "PreToolUse hook fired (acquired an interactive holder in the canary lock)"
    return "FAIL", "no hook fired — the interactive mutex is OFF (config loss or CC schema drift)"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--canary", action="store_true", help="force the end-to-end canary now")
    ap.add_argument("--no-canary", action="store_true", help="skip the canary even on a version change")
    ap.add_argument("--settings", default=str(SETTINGS))
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)
    if args.self_test:
        return _run_self_test()

    alarm = False
    print("=== verify_mutex_hooks (interactive side of the steward mutex) ===")

    problems = check_hooks_config(Path(args.settings))
    if problems:
        alarm = True
        print("⚠ CONFIG: hooks NOT intact —")
        for p in problems:
            print(f"    - {p}")
        print(f"    repair (idempotent): python3 {REPO}/scripts/install_steward_hooks.py")
    else:
        print(f"✓ CONFIG: all 3 hooks present + wired to steward_mutex.py ({args.settings})")

    cur = claude_version()
    changed, last = version_tripwire(VERSION_STATE, cur)
    if cur is None:
        print("• VERSION: `claude --version` unavailable (skipping tripwire)")
    elif changed:
        alarm = True
        print(f"⚠ VERSION: Claude Code changed {last!r} → {cur!r} — hook SCHEMA may have drifted; re-verify.")
    else:
        print(f"✓ VERSION: Claude Code unchanged ({cur})")

    if args.canary or (changed and not args.no_canary):
        print("• CANARY: spawning an isolated claude -p to prove the hooks fire …")
        verdict, detail = run_canary()
        mark = {"PASS": "✓", "FAIL": "⚠", "INCONCLUSIVE": "•"}.get(verdict, "•")
        print(f"{mark} CANARY: {verdict} — {detail}")
        if verdict == "FAIL":
            alarm = True

    if alarm:
        print("RESULT: ⚠ ALARM — the interactive mutex may be OFF; investigate before trusting it.")
        return 2
    print("RESULT: ✓ healthy.")
    return 0


# ── self-test (offline; never spawns claude) ─────────────────────────────────
class VerifyMutexHooksTests(unittest.TestCase):
    def _settings(self, body: dict) -> Path:
        d = Path(tempfile.mkdtemp(prefix="vmh_"))
        p = d / "settings.local.json"
        p.write_text(json.dumps(body), encoding="utf-8")
        self.addCleanup(shutil.rmtree, d, ignore_errors=True)
        return p

    def _good(self) -> dict:
        cmd = "python3 /x/scripts/steward_mutex.py acquire --holder interactive:$PPID --quiet; true"
        return {"hooks": {ev: [{"hooks": [{"type": "command", "command": cmd}]}] for ev in EXPECTED_EVENTS}}

    def test_config_ok(self):
        self.assertEqual(check_hooks_config(self._settings(self._good())), [])

    def test_config_missing_one_hook(self):
        body = self._good()
        del body["hooks"]["PreToolUse"]
        probs = check_hooks_config(self._settings(body))
        self.assertTrue(any("PreToolUse" in p for p in probs))

    def test_config_present_but_not_wired(self):
        body = {"hooks": {ev: [{"hooks": [{"type": "command", "command": "echo hi"}]}] for ev in EXPECTED_EVENTS}}
        probs = check_hooks_config(self._settings(body))
        self.assertEqual(len(probs), len(EXPECTED_EVENTS))  # none wired to steward_mutex.py

    def test_config_unparseable(self):
        d = Path(tempfile.mkdtemp(prefix="vmh_"))
        self.addCleanup(shutil.rmtree, d, ignore_errors=True)
        p = d / "settings.local.json"
        p.write_text("{ not json", encoding="utf-8")
        self.assertTrue(check_hooks_config(p))

    def test_version_tripwire(self):
        d = Path(tempfile.mkdtemp(prefix="vmh_"))
        self.addCleanup(shutil.rmtree, d, ignore_errors=True)
        state = d / "ver.txt"
        # First run: no baseline → not 'changed', persists current.
        changed, last = version_tripwire(state, "1.0.0")
        self.assertFalse(changed)
        self.assertIsNone(last)
        # Same version → unchanged.
        changed, last = version_tripwire(state, "1.0.0")
        self.assertFalse(changed)
        # New version → changed, reports the old baseline.
        changed, last = version_tripwire(state, "1.1.0")
        self.assertTrue(changed)
        self.assertEqual(last, "1.0.0")


def _run_self_test() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(VerifyMutexHooksTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
