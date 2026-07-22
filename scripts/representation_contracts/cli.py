"""CLI for representation and model-transition contracts."""

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

from .projector import deterministic_diff, project, query, state_dir, verify

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_WORKSPACE = ROOT / "capsules/spectral-bridge/workspace"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("command", choices=("project", "verify", "report", "registry", "show", "trace", "diff", "reconcile"))
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--receipt-json", action="store_true")
    parser.add_argument("--id")
    parser.add_argument("--left-id")
    parser.add_argument("--right-id")
    args = parser.parse_args(argv)
    workspace = args.workspace.resolve()
    started = time.monotonic()
    if args.command in {"project", "reconcile"}:
        status = project(workspace, write=args.write or args.command == "reconcile")
        value = projector_receipt(
            "representation_contracts", status,
            {"status.json": state_dir(workspace) / "status.json",
             "registry.jsonl": state_dir(workspace) / "registry.jsonl",
             "transitions.jsonl": state_dir(workspace) / "transitions.jsonl",
             "report.md": state_dir(workspace) / "report.md"},
            started_monotonic=started,
        ) if args.receipt_json else status
    elif args.command == "verify": value = verify(workspace)
    elif args.command == "report":
        path = state_dir(workspace) / "status.json"
        value = json.loads(path.read_text()) if path.is_file() else {"valid": False, "error": "status_missing"}
    elif args.command == "diff":
        try:
            value = deterministic_diff(
                workspace, args.left_id or "", args.right_id or ""
            )
        except (RecordValidationError, ValueError, TypeError) as error:
            value = {"valid": False, "error": str(error)}
    else:
        records = query(workspace, args.id)
        if args.command == "registry": records = [item for item in records if item.get("schema") == "representation_contract_v1"]
        value = {"schema": "representation_contract_query_v1", "records": records}
    print(json.dumps(value, indent=2, sort_keys=True))
    return 0 if value.get("valid", True) is not False else 1
