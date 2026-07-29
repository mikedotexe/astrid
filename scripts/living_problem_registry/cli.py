"""CLI for the derived Living Problem Registry."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import time

try:
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.projection_receipt import projector_receipt

from .projector import project, state_dir

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_WORKSPACE = ROOT / "capsules/spectral-bridge/workspace"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("command", choices=("project", "verify", "report"))
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--receipt-json", action="store_true")
    args = parser.parse_args(argv)
    workspace = args.workspace.resolve()
    started = time.monotonic()
    if args.command in {"project", "verify"}:
        status = project(
            workspace,
            write=args.write and args.command == "project",
        )
        value = (
            projector_receipt(
                "living_problem_registry",
                status,
                {
                    "status.json": state_dir(workspace) / "status.json",
                    "problems.jsonl": state_dir(workspace) / "problems.jsonl",
                    "changed_problems.jsonl": (
                        state_dir(workspace) / "changed_problems.jsonl"
                    ),
                    "queue.md": state_dir(workspace) / "queue.md",
                    "report.md": state_dir(workspace) / "report.md",
                },
                started_monotonic=started,
            )
            if args.receipt_json
            else status
        )
    else:
        path = state_dir(workspace) / "status.json"
        value = (
            json.loads(path.read_text(encoding="utf-8"))
            if path.is_file()
            else {"valid": False, "error": "status_missing"}
        )
    print(json.dumps(value, indent=2, sort_keys=True))
    return 0 if value.get("valid", True) is not False else 1
