"""Command-line interface for Temporal Bearing V1."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import time

try:
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.projection_receipt import projector_receipt

from .projector import project, projection_dir

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_WORKSPACE = ROOT / "capsules/spectral-bridge/workspace"
DEFAULT_MINIME_WORKSPACE = ROOT.parent / "minime/workspace"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument(
        "--minime-workspace", type=Path, default=DEFAULT_MINIME_WORKSPACE
    )
    parser.add_argument("command", choices=("project", "verify", "report"))
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--receipt-json", action="store_true")
    args = parser.parse_args(argv)

    workspace = args.workspace.resolve()
    minime_workspace = args.minime_workspace.resolve()
    started = time.monotonic()
    if args.command in {"project", "verify"}:
        status = project(
            workspace,
            minime_workspace=minime_workspace,
            write=args.command == "project" and args.write,
        )
        outputs = {
            "status.json": projection_dir(workspace) / "status.json",
            "latest.json": projection_dir(workspace) / "latest.json",
            "indexes.json": projection_dir(workspace) / "indexes.json",
            "gaps.jsonl": projection_dir(workspace) / "gaps.jsonl",
            "unlinked_evidence.jsonl": projection_dir(workspace)
            / "unlinked_evidence.jsonl",
            "report.md": projection_dir(workspace) / "report.md",
        }
        value = (
            projector_receipt(
                "temporal_bearing",
                status,
                outputs,
                started_monotonic=started,
            )
            if args.receipt_json
            else status
        )
    else:
        path = projection_dir(workspace) / "status.json"
        value = (
            json.loads(path.read_text(encoding="utf-8"))
            if path.is_file()
            else {"valid": False, "error": "status_missing"}
        )
    print(json.dumps(value, indent=2, sort_keys=True))
    return 0 if value.get("valid", True) is not False else 1


if __name__ == "__main__":
    raise SystemExit(main())
