#!/usr/bin/env python3
"""install_steward_hooks.py — (re)install the steward mutex's INTERACTIVE side.

The cross-steward mutex (scripts/steward_mutex.py) serializes ALL steward mutation
between the durable loop and interactive (human-steered) sessions. Its interactive
side lives in .claude/settings.local.json hooks — which is machine-local + gitignored,
so a FRESH CHECKOUT (or a second machine) has NO hooks until they are re-installed, and
the interactive mutex is silently OFF until then. verify_mutex_hooks.py ALARMS when the
hooks are absent; THIS script is the one-command, correct-by-construction REPAIR it
points at (hand-writing nested hook JSON is the error-prone silent-failure surface we
are removing: a typo'd hook parses fine but never fires).

Idempotent: ensures the 3 canonical hooks (SessionStart/PreToolUse/SessionEnd, all wired
to steward_mutex.py) are present WITHOUT clobbering anything else (permissions, unrelated
hooks). Already installed -> no-op (the file is not even rewritten, so formatting/order is
preserved).

  (default)    ensure-present: append the canonical steward group to any of the 3 events
               that lacks a steward hook; leave everything else untouched.
  --force      repair schema drift: drop existing steward groups on those 3 events and
               rewrite the canonical ones (preserves UNRELATED hooks on the same events).
  --check      report only; exit 2 if any steward hook is missing (no write). Mirrors
               verify_mutex_hooks' config check so the installer can dry-run itself.
  --self-test
"""
from __future__ import annotations

import argparse
import copy
import json
import shutil
import tempfile
import unittest
from pathlib import Path

REPO = Path("/Users/v/other/astrid")  # canonical path (matches verify_mutex_hooks / launchd)
SETTINGS = REPO / ".claude" / "settings.local.json"
EXPECTED_EVENTS = ("SessionStart", "PreToolUse", "SessionEnd")

# The exact live command strings. The STEWARD_LOOP guard skips the loop's own `claude -p`
# (so it cannot preempt its own wrapper lock); the trailing `; true` guarantees exit 0
# (only a PreToolUse exit 2 would block a tool — we never want that).
_MUTEX = f"python3 {REPO}/scripts/steward_mutex.py"
_ACQUIRE = f'[ -n "${{STEWARD_LOOP:-}}" ] || {_MUTEX} acquire --holder interactive:$PPID --quiet; true'
_RELEASE = f'[ -n "${{STEWARD_LOOP:-}}" ] || {_MUTEX} release --holder interactive:$PPID --quiet; true'


def desired_groups() -> dict:
    """The canonical per-event hook group (matches .claude/settings.local.json exactly)."""

    def grp(cmd: str, timeout: int, matcher: str | None = None) -> dict:
        g: dict = {}
        if matcher is not None:
            g["matcher"] = matcher
        g["hooks"] = [{"type": "command", "command": cmd, "timeout": timeout}]
        return g

    return {
        "SessionStart": grp(_ACQUIRE, 10),
        # PreToolUse refreshes the heartbeat on every tool call (matcher "*") — this is the
        # heartbeat itself, not just a check; do NOT throttle it to the staleness boundary.
        "PreToolUse": grp(_ACQUIRE, 5, matcher="*"),
        "SessionEnd": grp(_RELEASE, 10),
    }


def _group_is_steward(group: dict) -> bool:
    return any("steward_mutex.py" in h.get("command", "") for h in (group.get("hooks") or []))


def missing_events(data: dict) -> list[str]:
    """Events whose hook list has no steward_mutex.py command."""
    hooks = data.get("hooks") or {}
    out = []
    for ev in EXPECTED_EVENTS:
        if not any(_group_is_steward(g) for g in (hooks.get(ev) or [])):
            out.append(ev)
    return out


def ensure_hooks(data: dict, *, force: bool = False) -> dict:
    """Return a copy of `data` with the 3 steward hooks present. Pure (no I/O)."""
    out = copy.deepcopy(data)
    hooks = out.setdefault("hooks", {})
    want = desired_groups()
    for ev in EXPECTED_EVENTS:
        groups = list(hooks.get(ev) or [])
        if force:
            # Drop existing steward groups (repair drift), keep unrelated hooks, re-add canonical.
            groups = [g for g in groups if not _group_is_steward(g)]
            groups.append(want[ev])
        elif not any(_group_is_steward(g) for g in groups):
            groups.append(want[ev])
        hooks[ev] = groups
    return out


def _load(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return {}


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--settings", default=str(SETTINGS))
    ap.add_argument("--force", action="store_true", help="repair drift: rewrite the canonical steward groups")
    ap.add_argument("--check", action="store_true", help="report only; exit 2 if any steward hook is missing")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)
    if args.self_test:
        return _run_self_test()

    path = Path(args.settings)
    data = _load(path)

    if args.check:
        missing = missing_events(data)
        if missing:
            print(f"⚠ steward hooks MISSING for: {', '.join(missing)}")
            print(f"  repair: python3 {REPO}/scripts/install_steward_hooks.py")
            return 2
        print(f"✓ all 3 steward hooks present + wired ({path})")
        return 0

    updated = ensure_hooks(data, force=args.force)
    if json.dumps(updated, sort_keys=True) == json.dumps(data, sort_keys=True):
        print(f"✓ already installed — no change ({path})")
        return 0

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(updated, indent=2) + "\n", encoding="utf-8")
    verb = "repaired" if args.force else "installed"
    print(f"✓ steward hooks {verb} → {path}")
    print("  (Claude Code hot-reloads settings.local.json; new sessions pick them up at SessionStart)")
    return 0


# ── self-test (offline; never touches the real settings file) ────────────────
class InstallStewardHooksTests(unittest.TestCase):
    def test_install_into_empty(self):
        out = ensure_hooks({})
        self.assertEqual(missing_events(out), [])
        # PreToolUse keeps its "*" matcher; the others have none.
        self.assertEqual(out["hooks"]["PreToolUse"][0].get("matcher"), "*")
        self.assertNotIn("matcher", out["hooks"]["SessionStart"][0])

    def test_preserves_other_settings(self):
        base = {"permissions": {"allow": ["Bash(cargo build:*)"]}}
        out = ensure_hooks(base)
        self.assertEqual(out["permissions"], base["permissions"])
        self.assertEqual(missing_events(out), [])

    def test_idempotent(self):
        once = ensure_hooks({})
        twice = ensure_hooks(once)
        self.assertEqual(json.dumps(once, sort_keys=True), json.dumps(twice, sort_keys=True))

    def test_preserves_unrelated_hook_on_same_event(self):
        other = {"hooks": {"PreToolUse": [{"matcher": "*", "hooks": [{"type": "command", "command": "echo hi"}]}]}}
        out = ensure_hooks(other)
        cmds = [h["command"] for g in out["hooks"]["PreToolUse"] for h in g["hooks"]]
        self.assertIn("echo hi", cmds)
        self.assertTrue(any("steward_mutex.py" in c for c in cmds))

    def test_force_repairs_drift_keeps_unrelated(self):
        # An OLD/changed steward command + an unrelated hook on the same event.
        drifted = {"hooks": {"SessionStart": [
            {"hooks": [{"type": "command", "command": "python3 /old/steward_mutex.py acquire --holder x"}]},
            {"hooks": [{"type": "command", "command": "echo keep-me"}]},
        ]}}
        out = ensure_hooks(drifted, force=True)
        cmds = [h["command"] for g in out["hooks"]["SessionStart"] for h in g["hooks"]]
        self.assertIn("echo keep-me", cmds)  # unrelated survives
        self.assertNotIn("python3 /old/steward_mutex.py acquire --holder x", cmds)  # drift dropped
        self.assertTrue(any(c == _ACQUIRE for c in cmds))  # canonical restored exactly

    def test_check_roundtrip_on_real_shape(self):
        # A real-shaped install passes missing_events (the --check path).
        d = Path(tempfile.mkdtemp(prefix="ish_"))
        self.addCleanup(shutil.rmtree, d, ignore_errors=True)
        p = d / "settings.local.json"
        p.write_text(json.dumps(ensure_hooks({})), encoding="utf-8")
        self.assertEqual(missing_events(_load(p)), [])


def _run_self_test() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(InstallStewardHooksTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
