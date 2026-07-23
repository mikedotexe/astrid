"""CLI for reciprocal presence and uptake evidence."""

from __future__ import annotations

import argparse
import json
import os
import time
from pathlib import Path

try:
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.projection_receipt import projector_receipt

from .projector import (
    project,
    select_records,
    state_dir,
    trace_records,
    verify_outputs,
)

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_WORKSPACE = ROOT / "capsules/spectral-bridge/workspace"
DEFAULT_LEDGER = ROOT.parent / "shared/collaborations/correspondence_v1.jsonl"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument(
        "--ledger",
        type=Path,
        default=Path(os.environ.get("ASTRID_CORRESPONDENCE_LEDGER", DEFAULT_LEDGER)),
    )
    parser.add_argument("--json", action="store_true")
    parser.add_argument("command", choices=("project", "verify", "report", "show", "trace", "reconcile"))
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--receipt-json", action="store_true")
    parser.add_argument("--receipt-id")
    parser.add_argument("--thread-id")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    workspace = args.workspace.expanduser().resolve()
    started = time.monotonic()
    if args.command in {"project", "reconcile"}:
        status = project(
            workspace,
            args.ledger.expanduser().resolve(),
            write=args.write or args.command == "reconcile",
        )
        if args.command == "project" and args.receipt_json:
            value = projector_receipt(
                "reciprocal_uptake",
                status,
                {
                    "status.json": state_dir(workspace) / "status.json",
                    "receipts.jsonl": state_dir(workspace) / "receipts.jsonl",
                    "current_receipts.jsonl": state_dir(workspace) / "current_receipts.jsonl",
                    "technical_context_migration_v1.json": state_dir(workspace) / "technical_context_migration_v1.json",
                    "current_view_migration_v2.json": state_dir(workspace) / "current_view_migration_v2.json",
                    "context_identity_reconciliation_v2.json": state_dir(workspace) / "context_identity_reconciliation_v2.json",
                    "report.md": state_dir(workspace) / "report.md",
                },
                started_monotonic=started,
            )
        else:
            value = status
    elif args.command == "verify":
        value = verify_outputs(workspace)
    elif args.command == "report":
        path = state_dir(workspace) / "status.json"
        value = json.loads(path.read_text(encoding="utf-8")) if path.is_file() else {"valid": False, "error": "status_missing"}
    elif args.command == "trace":
        value = {
            "schema": "reciprocal_uptake_trace_v1",
            "records": trace_records(workspace, args.receipt_id or ""),
        }
    else:
        value = {
            "schema": "reciprocal_uptake_query_v1",
            "records": select_records(
                workspace,
                receipt_id=args.receipt_id,
                thread_id=args.thread_id,
            ),
        }
    print(json.dumps(value, indent=2, sort_keys=True))
    return 0 if value.get("valid", True) is not False else 1
