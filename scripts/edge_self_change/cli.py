"""Command-line interface for the fail-closed edge self-change supervisor."""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Sequence

from .model import Config, SupervisorError, _resolved_absolute, ensure_private_dir, read_json
from .supervisor import Supervisor

def create_signing_key(path: Path) -> None:
    path = _resolved_absolute(path, "signing key")
    if path.exists() or path.is_symlink():
        raise SupervisorError("refusing to replace an existing signing key")
    ensure_private_dir(path.parent)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        os.write(descriptor, os.urandom(32))
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--config",
        type=Path,
        default=Path("/etc/astrid/edge-self-change.json"),
        help="immutable operator configuration",
    )
    parser.add_argument("--execute", action="store_true", help="perform the requested mutation")
    parser.add_argument("--now", type=int, help=argparse.SUPPRESS)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("init")
    subparsers.add_parser("status")
    subparsers.add_parser("supervise")
    subparsers.add_parser("steward")
    due = subparsers.add_parser("due")
    due.add_argument("--reason", required=True)
    pause = subparsers.add_parser("pause")
    pause.add_argument("--reason", required=True)
    resume = subparsers.add_parser("resume")
    resume.add_argument("--reason", required=True)
    resume.add_argument("--ack-rescue", action="store_true")
    candidate = subparsers.add_parser("record-candidate")
    candidate.add_argument("--manifest", type=Path, required=True)
    build = subparsers.add_parser("record-build")
    build.add_argument("--manifest", type=Path, required=True)
    stage = subparsers.add_parser("stage")
    stage.add_argument("--build-id", required=True)
    rollback = subparsers.add_parser("rollback")
    rollback.add_argument("--reason", required=True)
    rescue = subparsers.add_parser("rescue")
    rescue.add_argument("--reason", required=True)
    subparsers.add_parser("check-probation")
    subparsers.add_parser("request-synthetic")
    subparsers.add_parser("prune")
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        config = Config.from_file(args.config)
        if args.command == "init":
            result = {
                "operation": "init",
                "dry_run": not args.execute,
                "state_root": str(config.state_root),
                "releases_root": str(config.releases_root),
                "signing_key": str(config.signing_key),
                "intent_attestation_key": str(config.intent_attestation_key),
            }
            if args.execute:
                ensure_private_dir(config.state_root)
                ensure_private_dir(config.releases_root)
                key_paths = (config.signing_key, config.intent_attestation_key)
                if any(path.exists() or path.is_symlink() for path in key_paths):
                    raise SupervisorError("refusing to replace an existing supervisor key")
                created: list[Path] = []
                try:
                    for key_path in key_paths:
                        create_signing_key(key_path)
                        created.append(key_path)
                    supervisor = Supervisor(config, now=args.now)
                    supervisor.write_state(supervisor.initial_state())
                    supervisor.pipeline.project_status(result)
                except BaseException:
                    if not (config.state_root / "state.json").exists():
                        for key_path in created:
                            key_path.unlink(missing_ok=True)
                    raise
            print(json.dumps(result, indent=2, sort_keys=True))
            return 0
        supervisor = Supervisor(config, now=args.now)
        with supervisor.locked():
            if args.command == "status":
                result = supervisor.status()
            elif args.command == "supervise":
                result = supervisor.supervise(execute=args.execute)
            elif args.command == "steward":
                result = supervisor.steward(execute=args.execute)
            elif args.command == "due":
                result = supervisor.mark_due(args.reason, execute=args.execute)
            elif args.command == "pause":
                result = supervisor.set_mode("paused", args.reason, execute=args.execute)
            elif args.command == "resume":
                result = supervisor.set_mode(
                    "running",
                    args.reason,
                    execute=args.execute,
                    acknowledge_rescue=args.ack_rescue,
                )
            elif args.command == "record-candidate":
                manifest = read_json(args.manifest)
                result = supervisor.record_candidate(manifest, execute=args.execute)
            elif args.command == "record-build":
                manifest = read_json(args.manifest)
                result = supervisor.record_build(manifest, execute=args.execute)
            elif args.command == "stage":
                result = supervisor.stage(args.build_id, execute=args.execute)
            elif args.command == "rollback":
                result = supervisor.rollback(args.reason, execute=args.execute)
            elif args.command == "rescue":
                result = supervisor.rescue(args.reason, execute=args.execute)
            elif args.command == "check-probation":
                result = supervisor.check_probation(execute=args.execute)
            elif args.command == "request-synthetic":
                result = supervisor.request_synthetic(execute=args.execute)
            elif args.command == "prune":
                result = supervisor.prune(execute=args.execute)
            else:  # pragma: no cover - argparse prevents this
                raise SupervisorError("unsupported command")
            if args.execute and args.command not in {"supervise", "steward"}:
                supervisor.pipeline.project_status(result)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0
    except SupervisorError as error:
        print(json.dumps({"ok": False, "error": str(error)}, sort_keys=True), file=sys.stderr)
        return 2
