#!/usr/bin/env python3
"""steward_mutex.py — one advisory lock that serializes ALL steward mutation.

Two stewards mutate the same working tree AND restart the same live bridge
process: the durable loop (`com.astrid.steward-loop`, headless, fires :07/:38)
and interactive (human-steered) sessions. Concurrently they raced — CHANGELOG
"modified-since-read" collisions, redundant rebuild+restarts, ambiguity over
whose in-flight edits ride whose restart. This is the agreed fix: a single
"full mutex" both parties acquire before ANY mutation (edits, builds, restarts).

Policy:
  - Holder id is "<type>:<pid>" where type is `interactive` or `loop`.
  - A lock is STALE if its holder PID is dead OR its heartbeat is older than TTL
    (default 1800s > the loop's 25-min watchdog cap, so a legit loop cycle never
    looks stale). A stale lock is freely stolen.
  - PRIORITY: `interactive` PREEMPTS a live `loop` holder (the human present wins;
    the loop re-checks `owns` before mutating and stands down if preempted).
    `loop` NEVER preempts a live `interactive` — it stands down for the cycle.
  - Re-acquiring your own lock just refreshes the heartbeat (idempotent), so the
    interactive PreToolUse hook can call `acquire` on every tool use cheaply.

Atomicity: the lock IS a directory (`mkdir` is atomic, same gate the loop wrapper
already uses) holding `holder.json`. Steal overwrites `holder.json` in place — a
benign race only between two simultaneous *stealers*, vanishingly rare with two
low-frequency cooperating parties.

Commands (exit 0 = you hold it / done; nonzero = you do NOT hold it):
  acquire  --holder <type:pid> [--ttl S]   take/refresh/steal/preempt
  release  --holder <type:pid>             release only if you hold it
  owns     --holder <type:pid>             exit 0 iff you currently hold it
  status                                   print current holder + ages
  --self-test
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time
import unittest
from pathlib import Path

# The default lock dir is env-overridable so the hook-health canary
# (verify_mutex_hooks.py) can exercise the SAME hook commands against an
# ISOLATED throwaway lock without touching the production mutex.
LOCK_DIR_DEFAULT = Path(os.environ.get("STEWARD_MUTEX_LOCK_DIR") or "~/.astrid/run/steward_mutex.lock").expanduser()
DEFAULT_TTL = 1800.0  # 30 min; > the loop's STEWARD_LOOP_MAX_SECS (1500s) cap


def parse_holder(holder: str) -> tuple[str, int]:
    typ, _, pid = holder.partition(":")
    if typ not in ("interactive", "loop") or not pid.isdigit():
        raise ValueError(f"holder must be 'interactive:<pid>' or 'loop:<pid>', got {holder!r}")
    return typ, int(pid)


def pid_alive(pid: int) -> bool:
    if pid <= 0:
        return False
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True  # exists, owned by another user
    return True


def read_holder(lock_dir: Path) -> dict | None:
    try:
        return json.loads((lock_dir / "holder.json").read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None


def _write_holder(lock_dir: Path, holder: str, typ: str, pid: int, now: float,
                  acquired_at: float | None = None) -> None:
    data = {
        "holder": holder, "type": typ, "pid": pid,
        "acquired_at": acquired_at if acquired_at is not None else now,
        "heartbeat_at": now,
    }
    (lock_dir / "holder.json").write_text(json.dumps(data), encoding="utf-8")


def is_stale(cur: dict, now: float, ttl: float) -> bool:
    return (not pid_alive(int(cur.get("pid", -1)))) or (now - float(cur.get("heartbeat_at", 0)) > ttl)


def acquire(lock_dir: Path, holder: str, ttl: float, now: float) -> tuple[bool, str]:
    typ, pid = parse_holder(holder)
    try:
        lock_dir.mkdir(parents=True)
        _write_holder(lock_dir, holder, typ, pid, now)
        return True, "acquired (was free)"
    except FileExistsError:
        pass
    cur = read_holder(lock_dir)
    if cur is None:
        _write_holder(lock_dir, holder, typ, pid, now)
        return True, "claimed (empty/garbage lock)"
    if cur.get("holder") == holder:
        _write_holder(lock_dir, holder, typ, pid, now, cur.get("acquired_at"))
        return True, "refreshed (already mine)"
    if is_stale(cur, now, ttl):
        _write_holder(lock_dir, holder, typ, pid, now)
        return True, f"stole stale lock from {cur.get('holder')}"
    if typ == "interactive" and cur.get("type") == "loop":
        _write_holder(lock_dir, holder, typ, pid, now)
        return True, f"preempted live loop {cur.get('holder')} (interactive priority)"
    return False, f"held by live {cur.get('holder')}"


def owns(lock_dir: Path, holder: str) -> bool:
    cur = read_holder(lock_dir)
    return cur is not None and cur.get("holder") == holder


def release(lock_dir: Path, holder: str) -> bool:
    cur = read_holder(lock_dir)
    if cur is not None and cur.get("holder") != holder:
        return False  # held by someone else — never release theirs
    try:
        (lock_dir / "holder.json").unlink()
    except OSError:
        pass
    try:
        lock_dir.rmdir()
    except OSError:
        pass
    return True


def status(lock_dir: Path, now: float, ttl: float) -> dict:
    cur = read_holder(lock_dir)
    if cur is None:
        return {"held": False}
    return {
        "held": True, "holder": cur.get("holder"), "type": cur.get("type"),
        "pid": cur.get("pid"), "pid_alive": pid_alive(int(cur.get("pid", -1))),
        "age_s": round(now - float(cur.get("acquired_at", now)), 1),
        "heartbeat_age_s": round(now - float(cur.get("heartbeat_at", now)), 1),
        "stale": is_stale(cur, now, ttl),
    }


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("cmd", nargs="?", choices=["acquire", "release", "owns", "status"])
    ap.add_argument("--holder")
    ap.add_argument("--ttl", type=float, default=DEFAULT_TTL)
    ap.add_argument("--lock-dir", default=str(LOCK_DIR_DEFAULT))
    ap.add_argument("--quiet", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _run_self_test()
    if not args.cmd:
        ap.error("a command is required (acquire|release|owns|status) unless --self-test")

    lock_dir = Path(args.lock_dir)
    now = time.time()

    if args.cmd == "status":
        st = status(lock_dir, now, args.ttl)
        if not args.quiet:
            print(json.dumps(st, indent=2))
        return 0

    if not args.holder:
        ap.error(f"--holder is required for {args.cmd}")

    if args.cmd == "acquire":
        ok, why = acquire(lock_dir, args.holder, args.ttl, now)
        if not args.quiet:
            print(f"{'OK' if ok else 'BLOCKED'}: {why}")
        return 0 if ok else 1
    if args.cmd == "owns":
        held = owns(lock_dir, args.holder)
        if not args.quiet:
            print("owns" if held else "not-owner")
        return 0 if held else 1
    if args.cmd == "release":
        released = release(lock_dir, args.holder)
        if not args.quiet:
            print("released" if released else "not-owner (left intact)")
        return 0
    return 0


# ── self-test ────────────────────────────────────────────────────────────────
class StewardMutexTests(unittest.TestCase):
    def setUp(self):
        import tempfile
        self._tmp = tempfile.TemporaryDirectory()
        self.lock = Path(self._tmp.name) / "mutex.lock"

    def tearDown(self):
        self._tmp.cleanup()

    def test_acquire_free_then_refresh(self):
        now = 1000.0
        ok, _ = acquire(self.lock, f"interactive:{os.getpid()}", DEFAULT_TTL, now)
        self.assertTrue(ok)
        ok2, why = acquire(self.lock, f"interactive:{os.getpid()}", DEFAULT_TTL, now + 5)
        self.assertTrue(ok2)
        self.assertIn("refreshed", why)

    def test_loop_blocked_by_live_interactive(self):
        now = 1000.0
        acquire(self.lock, f"interactive:{os.getpid()}", DEFAULT_TTL, now)  # live (this proc)
        ok, why = acquire(self.lock, f"loop:{os.getpid()}", DEFAULT_TTL, now)
        self.assertFalse(ok)
        self.assertIn("held by live", why)

    def test_interactive_preempts_live_loop(self):
        now = 1000.0
        acquire(self.lock, f"loop:{os.getpid()}", DEFAULT_TTL, now)  # live loop
        ok, why = acquire(self.lock, f"interactive:{os.getpid()}", DEFAULT_TTL, now)
        self.assertTrue(ok)
        self.assertIn("preempted", why)

    def test_steal_dead_pid(self):
        now = 1000.0
        dead = 2_147_483_646  # almost certainly not a live pid
        acquire(self.lock, f"interactive:{dead}", DEFAULT_TTL, now)
        ok, why = acquire(self.lock, f"loop:{os.getpid()}", DEFAULT_TTL, now)
        self.assertTrue(ok)  # loop can take a DEAD interactive's stale lock
        self.assertIn("stole stale", why)

    def test_steal_ttl_expired(self):
        now = 1000.0
        acquire(self.lock, f"interactive:{os.getpid()}", DEFAULT_TTL, now)  # live but...
        # ...heartbeat now far in the past relative to a later 'now'.
        ok, why = acquire(self.lock, f"loop:{os.getpid()}", ttl=60.0, now=now + 10_000)
        self.assertTrue(ok)
        self.assertIn("stole stale", why)

    def test_owns_and_release(self):
        now = 1000.0
        me = f"interactive:{os.getpid()}"
        acquire(self.lock, me, DEFAULT_TTL, now)
        self.assertTrue(owns(self.lock, me))
        self.assertFalse(owns(self.lock, f"loop:{os.getpid()}"))
        self.assertTrue(release(self.lock, me))
        self.assertFalse(owns(self.lock, me))  # gone

    def test_release_does_not_remove_others_lock(self):
        now = 1000.0
        acquire(self.lock, f"interactive:{os.getpid()}", DEFAULT_TTL, now)
        self.assertFalse(release(self.lock, f"loop:{os.getpid()}"))  # not owner
        self.assertTrue(owns(self.lock, f"interactive:{os.getpid()}"))  # still held


def _run_self_test() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(StewardMutexTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
