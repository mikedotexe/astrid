#!/usr/bin/env python3
"""Agent-neutral steward control plane."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
from typing import Any

try:
    from steward_control import StewardController, StewardControlError, load_config
    from steward_control.executor import run_subprocess
except ModuleNotFoundError:
    from scripts.steward_control import (
        StewardController,
        StewardControlError,
        load_config,
    )
    from scripts.steward_control.executor import run_subprocess


def emit(value: Any, as_json: bool) -> None:
    if as_json:
        print(json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False))
    else:
        print(json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False))


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--config")
    result.add_argument("--repo-root")
    result.add_argument("--workspace")
    result.add_argument("--state-root")
    result.add_argument("--store-root")
    result.add_argument("--json", action="store_true")
    result.add_argument("--self-test", action="store_true")
    commands = result.add_subparsers(dest="command")

    commands.add_parser("status")
    commands.add_parser("verify")

    pause = commands.add_parser("pause")
    pause.add_argument("--actor", default="interactive-agent")
    pause.add_argument("--reason", required=True)
    pause.add_argument("--wait-secs", type=float, default=0)

    resume = commands.add_parser("resume")
    resume.add_argument("--actor", default="interactive-agent")
    resume.add_argument("--ack", required=True)

    begin = commands.add_parser("begin")
    begin.add_argument("--actor", default="interactive-agent")
    begin.add_argument(
        "--adapter-kind",
        choices=("session", "subprocess"),
        default="session",
    )
    begin.add_argument("--holder-pid", type=int)

    heartbeat = commands.add_parser("heartbeat")
    heartbeat.add_argument("--run-id", required=True)
    heartbeat.add_argument("--lease-token", required=True)

    finish = commands.add_parser("finish")
    finish.add_argument("--run-id", required=True)
    finish.add_argument("--lease-token", required=True)
    finish.add_argument(
        "--outcome",
        choices=("success", "failed", "cancelled", "policy_violation"),
        required=True,
    )
    finish.add_argument("--exit-code", type=int)
    finish.add_argument("--summary-ref")

    commands.add_parser("reconcile")

    run = commands.add_parser("run")
    run.add_argument("--actor", default="interactive-agent")
    run.add_argument("--max-secs", type=int)
    run.add_argument("argv", nargs=argparse.REMAINDER)
    return result


def self_test() -> int:
    import unittest

    try:
        from test_steward_control import StewardControlTests
    except ModuleNotFoundError:
        from scripts.test_steward_control import StewardControlTests

    suite = unittest.defaultTestLoader.loadTestsFromTestCase(StewardControlTests)
    return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    if args.self_test:
        return self_test()
    if not args.command:
        parser().print_help()
        return 2
    config = load_config(
        config_path=args.config,
        repo_root=args.repo_root,
        workspace=args.workspace,
        state_root=args.state_root,
        store_root=args.store_root,
    )
    controller = StewardController(config)
    try:
        if args.command == "status":
            value = controller.status()
        elif args.command == "verify":
            value = controller.verify()
        elif args.command == "pause":
            value = controller.pause(
                actor=args.actor,
                reason=args.reason,
                wait_secs=args.wait_secs,
            )
        elif args.command == "resume":
            value = controller.resume(actor=args.actor, acknowledgement=args.ack)
        elif args.command == "begin":
            value = controller.begin(
                actor=args.actor,
                adapter_kind=args.adapter_kind,
                pid=args.holder_pid,
            )
        elif args.command == "heartbeat":
            value = controller.heartbeat(
                run_id=args.run_id,
                lease_token=args.lease_token,
            )
        elif args.command == "finish":
            value = controller.finish(
                run_id=args.run_id,
                lease_token=args.lease_token,
                outcome=args.outcome,
                exit_code=args.exit_code,
                summary_ref=args.summary_ref,
            )
        elif args.command == "reconcile":
            value = controller.reconcile()
        elif args.command == "run":
            command = list(args.argv)
            if command and command[0] == "--":
                command.pop(0)
            return_code, value = run_subprocess(
                controller,
                actor=args.actor,
                argv=command,
                max_secs=args.max_secs,
            )
            emit(value, args.json)
            return return_code
        else:  # pragma: no cover - argparse owns command validation
            raise AssertionError(args.command)
    except StewardControlError as error:
        emit(
            {
                "schema": "steward_control_error_v1",
                "error": type(error).__name__,
                "message": str(error),
            },
            args.json,
        )
        return error.exit_code
    except (OSError, RuntimeError, ValueError) as error:
        emit(
            {
                "schema": "steward_control_error_v1",
                "error": type(error).__name__,
                "message": str(error),
            },
            args.json,
        )
        return 5
    emit(value, args.json)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
