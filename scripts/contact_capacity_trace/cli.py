"""CLI for origin-aware Contact-to-Capacity Trace V2."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import time

try:
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.projection_receipt import projector_receipt

from .projector import project, projection_dir, state_dir

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_WORKSPACE = ROOT / "capsules/spectral-bridge/workspace"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("command", choices=("project", "verify", "report"))
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--receipt-json", action="store_true")
    args = parser.parse_args(argv)
    workspace = args.workspace.resolve()
    started = time.monotonic()
    if args.command in {"project", "verify"}:
        status = project(workspace, write=args.command == "project" and args.write)
        value = (
            projector_receipt(
                "contact_capacity_trace",
                status,
                {
                    "v2_status.json": projection_dir(workspace) / "status.json",
                    "v2_traces.jsonl": projection_dir(workspace) / "traces.jsonl",
                    "v2_ingress_gaps.jsonl": projection_dir(workspace) / "ingress_gaps.jsonl",
                    "v2_journey_origins.jsonl": projection_dir(workspace) / "journey_origins.jsonl",
                    "v2_capture_gaps.jsonl": projection_dir(workspace) / "capture_gaps.jsonl",
                    "v2_report.md": projection_dir(workspace) / "report.md",
                    "v1_status.json": state_dir(workspace) / "status.json",
                    "v1_traces.jsonl": state_dir(workspace) / "traces.jsonl",
                },
                started_monotonic=started,
            )
            if args.receipt_json
            else status
        )
    else:
        path = projection_dir(workspace) / "status.json"
        value = json.loads(path.read_text()) if path.is_file() else {"valid": False, "error": "status_missing"}
    print(json.dumps(value, indent=2, sort_keys=True))
    return 0 if value.get("valid", True) is not False else 1


if __name__ == "__main__":
    raise SystemExit(main())
