"""CLI for external steward work selection."""

from __future__ import annotations

import argparse
import json
import time
from pathlib import Path

try:
    from experiential_systems.common import RecordValidationError
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.experiential_systems.common import RecordValidationError
    from scripts.projection_receipt import projector_receipt

from .model import OwnerPriorityPinV1
from .projector import append_pin, project, query, state_dir

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_WORKSPACE = ROOT / "capsules/spectral-bridge/workspace"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--json", action="store_true")
    parser.add_argument(
        "command", choices=("project", "verify", "report", "show", "pin", "unpin")
    )
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--receipt-json", action="store_true")
    parser.add_argument("--owner", choices=("astrid", "minime"))
    parser.add_argument("--actor")
    parser.add_argument("--contract-id")
    parser.add_argument("--source-event-id")
    parser.add_argument("--source-event-sha256")
    args = parser.parse_args(argv)
    workspace = args.workspace.resolve()
    started = time.monotonic()
    try:
        if args.command in {"pin", "unpin"}:
            pin = OwnerPriorityPinV1.build(
                owner=args.owner,
                contract_id=args.contract_id,
                action=args.command,
                source_event_id=args.source_event_id,
                source_event_sha256=args.source_event_sha256,
            )
            value = append_pin(workspace, pin, args.actor or "")
        elif args.command in {"project", "verify"}:
            status = project(
                workspace, write=args.write and args.command == "project"
            )
            value = (
                projector_receipt(
                    "steward_work_selection",
                    status,
                    {
                        "status.json": state_dir(workspace) / "status.json",
                        "selection.json": state_dir(workspace) / "selection.json",
                        "report.md": state_dir(workspace) / "report.md",
                    },
                    started_monotonic=started,
                )
                if args.receipt_json
                else status
            )
        elif args.command == "report":
            path = state_dir(workspace) / "status.json"
            value = (
                json.loads(path.read_text())
                if path.is_file()
                else {"valid": False, "error": "status_missing"}
            )
        else:
            value = query(workspace, args.contract_id)
    except (RecordValidationError, ValueError, TypeError, StopIteration) as error:
        value = {"valid": False, "error": str(error)}
    print(json.dumps(value, indent=2, sort_keys=True))
    return 0 if value.get("valid", True) is not False else 1
