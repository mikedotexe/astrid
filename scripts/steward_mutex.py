#!/usr/bin/env python3
"""Compatibility facade for the retired advisory mutex interface.

Deployment preflight still imports ``foreign_activity`` from this path. The
activity scan remains read-only and delegates to the generic control package.
Legacy acquire, owns, and release commands are warning-only and never mutate
lock state.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
import time
import unittest

try:
    from steward_control.activity import (
        AGENT_STATE_DIR_DEFAULT,
        FOREIGN_ACTIVITY_WINDOW_SECS,
        foreign_activity,
        newest_age,
        session_liveness_age,
        tree_activity_age,
    )
    from steward_control.config import load_config
    from steward_control.controller import StewardController
except ModuleNotFoundError:
    from scripts.steward_control.activity import (
        AGENT_STATE_DIR_DEFAULT,
        FOREIGN_ACTIVITY_WINDOW_SECS,
        foreign_activity,
        newest_age,
        session_liveness_age,
        tree_activity_age,
    )
    from scripts.steward_control.config import load_config
    from scripts.steward_control.controller import StewardController

# Deprecated aliases retained because deployment preflight imports them.
CODEX_STATE_DIR_DEFAULT = AGENT_STATE_DIR_DEFAULT
FOREIGN_ACTIVITY_WINDOW_S = FOREIGN_ACTIVITY_WINDOW_SECS


def _warning(command: str) -> dict[str, object]:
    return {
        "schema": "steward_mutex_compatibility_warning_v1",
        "schema_version": 1,
        "command": command,
        "retired": True,
        "mutated": False,
        "replacement": "scripts/steward_control.py",
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        nargs="?",
        choices=("acquire", "release", "owns", "status", "foreign"),
    )
    parser.add_argument("--holder")
    parser.add_argument("--ttl", type=float, default=1800.0)
    parser.add_argument("--lock-dir")
    parser.add_argument("--quiet", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--repo", default=".")
    parser.add_argument("--codex-state-dir", default=str(AGENT_STATE_DIR_DEFAULT))
    parser.add_argument("--window-s", type=float, default=FOREIGN_ACTIVITY_WINDOW_SECS)
    parser.add_argument("--config")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(
            StewardMutexCompatibilityTests
        )
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    if args.command == "foreign":
        value = foreign_activity(
            Path(args.repo),
            Path(args.codex_state_dir),
            args.window_s,
            time.time(),
        )
        if not args.quiet:
            print(json.dumps(value, indent=2, sort_keys=True))
        return 3 if value["active"] else 0
    if args.command == "status":
        value = StewardController(
            load_config(config_path=args.config)
        ).status()
        if not args.quiet:
            print(json.dumps(value, indent=2, sort_keys=True))
        return 0
    if args.command in {"acquire", "release", "owns"}:
        value = _warning(args.command)
        if not args.quiet:
            print(json.dumps(value, indent=2, sort_keys=True), file=sys.stderr)
        return 2
    build_parser().print_help()
    return 2


class StewardMutexCompatibilityTests(unittest.TestCase):
    def test_legacy_lock_commands_are_warning_only(self) -> None:
        for command in ("acquire", "release", "owns"):
            warning = _warning(command)
            self.assertTrue(warning["retired"])
            self.assertFalse(warning["mutated"])

    def test_foreign_activity_keys_only_on_tree_evidence(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            state = root / "state"
            state.mkdir()
            heartbeat = state / "session.heartbeat"
            heartbeat.write_text("present\n", encoding="utf-8")
            now = heartbeat.stat().st_mtime + 1
            evidence = foreign_activity(
                root / "not-a-repository",
                state,
                180,
                now,
            )
            self.assertFalse(evidence["active"])
            self.assertTrue(evidence["session_state_live"])


if __name__ == "__main__":
    raise SystemExit(main())
