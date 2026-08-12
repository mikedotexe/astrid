"""CLI for Counterfactual Change Lab V1."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import time

try:
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.projection_receipt import projector_receipt

from .projector import (
    CORPUS_PATH,
    FIXTURE_PATH,
    capture_fixture,
    project,
    state_dir,
)

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_WORKSPACE = ROOT / "capsules/spectral-bridge/workspace"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--corpus", type=Path, default=CORPUS_PATH)
    parser.add_argument("--fixture", type=Path, default=FIXTURE_PATH)
    parser.add_argument("--endpoint", default="http://127.0.0.1:11434")
    parser.add_argument("--model", default="nomic-embed-text:latest")
    parser.add_argument("--receipt-json", action="store_true")
    parser.add_argument("command", choices=("capture-fixture", "project", "verify", "report"))
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args(argv)
    workspace = args.workspace.resolve()
    started = time.monotonic()
    if args.command == "capture-fixture":
        value = capture_fixture(
            endpoint=args.endpoint,
            model=args.model,
            corpus_path=args.corpus.resolve(),
            fixture_path=args.fixture.resolve(),
        )
    elif args.command in {"project", "verify"}:
        status = project(
            workspace,
            corpus_path=args.corpus.resolve(),
            fixture_path=args.fixture.resolve(),
            write=args.command == "project" and args.write,
        )
        value = (
            projector_receipt(
                "counterfactual_change_lab",
                status,
                {
                    "status.json": state_dir(workspace) / "status.json",
                    "campaign_manifest.json": state_dir(workspace) / "campaigns/fixed_projection_basis_epoch_comparison_v1/campaign_manifest.json",
                    "comparison.json": state_dir(workspace) / "campaigns/fixed_projection_basis_epoch_comparison_v1/comparison.json",
                    "basis_metrics.jsonl": state_dir(workspace) / "campaigns/fixed_projection_basis_epoch_comparison_v1/basis_metrics.jsonl",
                    "evidence_study_handoff.json": state_dir(workspace) / "campaigns/fixed_projection_basis_epoch_comparison_v1/evidence_study_handoff.json",
                    "report.md": state_dir(workspace) / "report.md",
                },
                started_monotonic=started,
            )
            if args.receipt_json
            else status
        )
    else:
        path = state_dir(workspace) / "status.json"
        value = json.loads(path.read_text()) if path.is_file() else {"valid": False, "error": "status_missing"}
    print(json.dumps(value, indent=2, sort_keys=True))
    return 0 if value.get("valid", True) is not False else 1


if __name__ == "__main__":
    raise SystemExit(main())
