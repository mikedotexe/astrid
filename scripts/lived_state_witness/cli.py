"""Command-line interface for temporal lived-state witnesses."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import time

try:
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.projection_receipt import projector_receipt

from . import projector

DEFAULT_WORKSPACE = Path(
    __file__
).resolve().parents[2] / "capsules/spectral-bridge/workspace"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    commands = parser.add_subparsers(dest="command")

    migrate = commands.add_parser("migrate")
    mode = migrate.add_mutually_exclusive_group(required=True)
    mode.add_argument("--dry-run", action="store_true")
    mode.add_argument("--write", action="store_true")
    migrate.add_argument("--json", action="store_true")

    project = commands.add_parser("project")
    project.add_argument("--write", action="store_true")
    project.add_argument("--receipt-json", action="store_true")
    project.add_argument("--json", action="store_true")

    for name in ("verify", "report"):
        command = commands.add_parser(name)
        command.add_argument("--json", action="store_true")

    show = commands.add_parser("show")
    show.add_argument("--witness-id", required=True)
    show.add_argument("--json", action="store_true")

    reconcile = commands.add_parser("reconcile")
    reconcile.add_argument("--write", action="store_true")
    reconcile.add_argument("--json", action="store_true")

    diff = commands.add_parser("diff")
    diff.add_argument("--left", required=True)
    diff.add_argument("--right", required=True)
    diff.add_argument("--json", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.self_test:
        from .selftest import run_self_tests

        return run_self_tests()
    if args.command is None:
        parser.print_help()
        return 2
    workspace = args.workspace.expanduser().resolve()
    started = time.monotonic()
    if args.command == "migrate":
        result = projector.migrate(workspace, write=bool(args.write))
    elif args.command == "project":
        result = projector.project(workspace, write=bool(args.write))
        if args.receipt_json:
            root = projector.state_dir(workspace)
            result = projector_receipt(
                "lived_state_witness",
                result,
                {
                    "status.json": root / "status.json",
                    "witnesses.jsonl": root / "witnesses.jsonl",
                    "auxiliary_artifacts.jsonl": root
                    / "auxiliary_artifacts.jsonl",
                    "gaps.jsonl": root / "gaps.jsonl",
                    "temporal_clusters.jsonl": root
                    / "temporal_clusters.jsonl",
                    "concordance_clusters.jsonl": root
                    / "concordance_clusters.jsonl",
                    "concordance_preflight.json": root
                    / "concordance_preflight.json",
                    "context_index.jsonl": root / "context_index.jsonl",
                    "report.md": root / "report.md",
                    "migration_receipt.json": root / "migration_receipt.json",
                },
                started_monotonic=started,
            )
    elif args.command == "verify":
        result = projector.verify(workspace)
    elif args.command == "report":
        result = projector.report(workspace)
    elif args.command == "show":
        result = projector.show(workspace, args.witness_id)
    elif args.command == "reconcile":
        result = projector.reconcile(workspace, write=bool(args.write))
    elif args.command == "diff":
        result = projector.diff_witnesses(workspace, args.left, args.right)
    else:
        parser.error(f"unsupported command: {args.command}")
    print(json.dumps(result, indent=2, sort_keys=True, ensure_ascii=False))
    return 0 if result.get("valid", True) else 1
