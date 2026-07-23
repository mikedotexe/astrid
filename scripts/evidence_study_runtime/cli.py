"""CLI for capture-first experiential evidence studies."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import time
from typing import Any

try:
    from experiential_systems.common import RecordValidationError
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.experiential_systems.common import RecordValidationError
    from scripts.projection_receipt import projector_receipt

from .config import DEFAULT_MANIFEST, DEFAULT_WORKSPACE, state_dir
from .projector import project, query
from .service import (
    assemble,
    capture_status,
    compare,
    reconcile,
    record_review,
    revise_plan,
    seed,
    start_capture,
    stop_capture,
)


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser(description=__doc__)
    value.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    value.add_argument("--json", action="store_true")
    commands = value.add_subparsers(dest="command", required=True)

    campaign = commands.add_parser("campaign")
    campaign_commands = campaign.add_subparsers(dest="campaign_command", required=True)
    campaign_seed = campaign_commands.add_parser("seed")
    campaign_seed.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    campaign_seed.add_argument("--actor", default="interactive-operator")
    campaign_show = campaign_commands.add_parser("show")
    campaign_show.add_argument("--campaign-id", required=True)
    campaign_commands.add_parser("report")

    plan = commands.add_parser("plan")
    plan.add_argument("--plan-id", required=True)
    plan.add_argument("--revision-json", type=Path)
    plan.add_argument("--actor", default="interactive-operator")

    capture = commands.add_parser("capture")
    capture_commands = capture.add_subparsers(dest="capture_command", required=True)
    capture_start = capture_commands.add_parser("start")
    capture_start.add_argument("--plan-id", required=True)
    capture_start.add_argument("--duration-minutes", type=int)
    capture_start.add_argument("--sample-limit", type=int)
    capture_start.add_argument("--signal-capture-window-ref")
    capture_start.add_argument("--extension-of-window-id")
    capture_start.add_argument("--actor", default="interactive-operator")
    capture_commands.add_parser("status")
    capture_stop = capture_commands.add_parser("stop")
    capture_stop.add_argument("--window-id", required=True)
    capture_stop.add_argument("--actor", default="interactive-operator")

    assemble_parser = commands.add_parser("assemble")
    assemble_parser.add_argument("--plan-id", required=True)
    assemble_parser.add_argument("--window-id", required=True)
    assemble_parser.add_argument("--actor", default="interactive-operator")

    compare_parser = commands.add_parser("compare")
    compare_parser.add_argument("--plan-id", required=True)
    compare_parser.add_argument("--actor", default="interactive-operator")

    review = commands.add_parser("record-review")
    review.add_argument("--campaign-id", required=True)
    review.add_argument("--study-id", required=True)
    review.add_argument("--comparison-id", required=True)
    review.add_argument(
        "--outcome",
        required=True,
        choices=(
            "no_response",
            "corroborated",
            "mechanism_smooth_felt_friction_remains",
            "contradicted",
            "insufficient",
        ),
    )
    review.add_argument("--source-ref", required=True)
    review.add_argument("--actor", default="interactive-operator")

    reconcile_parser = commands.add_parser("reconcile")
    reconcile_parser.add_argument("--actor", default="interactive-operator")

    project_parser = commands.add_parser("project")
    project_parser.add_argument("--write", action="store_true")
    project_parser.add_argument("--receipt-json", action="store_true")
    commands.add_parser("verify")
    show = commands.add_parser("show")
    show.add_argument("--id", required=True)
    commands.add_parser("self-test")
    return value


def _read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise RecordValidationError("plan revision must be a JSON object")
    return value


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    workspace = args.workspace.resolve()
    started = time.monotonic()
    try:
        if args.command == "campaign":
            if args.campaign_command == "seed":
                result = seed(
                    workspace,
                    actor=args.actor,
                    manifest_path=args.manifest.resolve(),
                )
            elif args.campaign_command == "show":
                result = query(workspace, args.campaign_id)
            else:
                status_path = state_dir(workspace) / "status.json"
                result = (
                    json.loads(status_path.read_text(encoding="utf-8"))
                    if status_path.is_file()
                    else {"valid": False, "error": "study status missing"}
                )
        elif args.command == "plan":
            if args.revision_json:
                result = revise_plan(
                    workspace,
                    args.plan_id,
                    _read_json(args.revision_json),
                    actor=args.actor,
                ).to_dict()
            else:
                result = query(workspace, args.plan_id)
        elif args.command == "capture":
            if args.capture_command == "start":
                result = start_capture(
                    workspace,
                    args.plan_id,
                    actor=args.actor,
                    duration_minutes=args.duration_minutes,
                    sample_limit=args.sample_limit,
                    signal_capture_window_ref=args.signal_capture_window_ref,
                    extension_of_window_id=args.extension_of_window_id,
                ).to_dict()
            elif args.capture_command == "stop":
                result = stop_capture(
                    workspace, args.window_id, actor=args.actor
                ).to_dict()
            else:
                result = capture_status(workspace)
        elif args.command == "assemble":
            result = assemble(
                workspace, args.plan_id, args.window_id, actor=args.actor
            )
        elif args.command == "compare":
            result = compare(workspace, args.plan_id, actor=args.actor).to_dict()
        elif args.command == "record-review":
            result = record_review(
                workspace,
                campaign_id=args.campaign_id,
                study_id=args.study_id,
                comparison_id=args.comparison_id,
                outcome=args.outcome,
                source_ref=args.source_ref,
                actor=args.actor,
            ).to_dict()
        elif args.command == "reconcile":
            result = reconcile(workspace, actor=args.actor)
        elif args.command in {"project", "verify"}:
            status = project(
                workspace,
                write=args.command == "project" and bool(args.write),
            )
            if args.command == "project" and args.receipt_json:
                root = state_dir(workspace)
                result = projector_receipt(
                    "evidence_study_runtime",
                    status,
                    {
                        "status.json": root / "status.json",
                        "campaigns.jsonl": root / "campaigns.jsonl",
                        "plans.jsonl": root / "plans.jsonl",
                        "comparisons.jsonl": root / "comparisons.jsonl",
                        "report.md": root / "report.md",
                    },
                    started_monotonic=started,
                )
            else:
                result = status
        elif args.command == "show":
            result = query(workspace, args.id)
        else:
            from .selftest import run

            result = {"valid": run() == 0}
    except (OSError, RecordValidationError, TypeError, ValueError) as error:
        result = {"valid": False, "error": str(error)}
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if result.get("valid", True) is not False else 1
