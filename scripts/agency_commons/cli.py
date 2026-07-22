"""Owner-language CLI for the advisory agency commons."""

from __future__ import annotations

import argparse
import json
import os
import time
from pathlib import Path

try:
    from experiential_systems.common import RecordValidationError, sha256_bytes
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.experiential_systems.common import RecordValidationError, sha256_bytes
    from scripts.projection_receipt import projector_receipt

from .model import (
    AgencyCommonsProposalV1, AgencyCommonsResponseV1, AgencyReturnPointV1,
    LaterFeltCheckRequestV1, ProtectedTimeDeclarationV1,
)
from .projector import append_action, project, query, state_dir

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_WORKSPACE = ROOT / "capsules/spectral-bridge/workspace"
DEFAULT_PHASE_LEDGER = ROOT.parent / "shared/collaborations/phase_transitions_v1.jsonl"
DEFAULT_CORRESPONDENCE_LEDGER = ROOT.parent / "shared/collaborations/correspondence_v1.jsonl"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--phase-ledger", type=Path, default=Path(os.environ.get("ASTRID_PHASE_TRANSITION_LEDGER", DEFAULT_PHASE_LEDGER)))
    parser.add_argument("--sovereignty-ledger", type=Path)
    parser.add_argument("--agency-request-dir", type=Path)
    parser.add_argument("--correspondence-ledger", type=Path, default=Path(os.environ.get("ASTRID_CORRESPONDENCE_LEDGER", DEFAULT_CORRESPONDENCE_LEDGER)))
    parser.add_argument("--json", action="store_true")
    parser.add_argument("command", choices=("project", "verify", "report", "show", "propose", "respond", "return", "protect", "request-check"))
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--receipt-json", action="store_true")
    parser.add_argument("--actor")
    parser.add_argument("--peer")
    parser.add_argument("--id")
    parser.add_argument("--proposal-id")
    parser.add_argument("--proposal-actor")
    parser.add_argument("--response-kind", choices=("accept", "hold", "refuse", "counter", "revisit", "withdraw"))
    parser.add_argument("--counter-proposal-id")
    parser.add_argument("--transition-kind")
    parser.add_argument("--from-state-ref")
    parser.add_argument("--to-state-ref")
    parser.add_argument("--return-point-id")
    parser.add_argument("--state-ref")
    parser.add_argument("--state-sha256")
    parser.add_argument("--start-unix-ms", type=int)
    parser.add_argument("--duration-ms", type=int)
    parser.add_argument("--requested-from")
    parser.add_argument("--source-ref")
    parser.add_argument("--source-event-id")
    parser.add_argument("--source-event-sha256")
    parser.add_argument("--recorded-at-unix-ms", type=int)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    workspace = args.workspace.resolve()
    started = time.monotonic()
    common = {"actor": args.actor, "source_event_id": args.source_event_id,
              "source_event_sha256": args.source_event_sha256,
              "recorded_at_unix_ms": args.recorded_at_unix_ms}
    try:
        if args.command == "propose":
            record = AgencyCommonsProposalV1.build(peer=args.peer,
                                                   transition_kind=args.transition_kind,
                                                   from_state_ref=args.from_state_ref,
                                                   to_state_ref=args.to_state_ref,
                                                   return_point_id=args.return_point_id, **common)
            value = append_action(workspace, record.to_dict(), args.actor)
        elif args.command == "respond":
            record = AgencyCommonsResponseV1.build(proposal_id=args.proposal_id,
                                                   proposal_actor=args.proposal_actor,
                                                   response_kind=args.response_kind,
                                                   counter_proposal_id=args.counter_proposal_id, **common)
            value = append_action(workspace, record.to_dict(), args.actor)
        elif args.command == "return":
            record = AgencyReturnPointV1.build(state_ref=args.state_ref,
                                               state_sha256=args.state_sha256, **common)
            value = append_action(workspace, record.to_dict(), args.actor)
        elif args.command == "protect":
            record = ProtectedTimeDeclarationV1.build(start_unix_ms=args.start_unix_ms,
                                                       duration_ms=args.duration_ms, **common)
            value = append_action(workspace, record.to_dict(), args.actor)
        elif args.command == "request-check":
            record = LaterFeltCheckRequestV1.build(requested_from=args.requested_from,
                                                   source_ref=args.source_ref, **common)
            value = append_action(workspace, record.to_dict(), args.actor)
        elif args.command in {"project", "verify"}:
            status = project(
                workspace,
                args.phase_ledger.resolve(),
                sovereignty_ledger=(
                    args.sovereignty_ledger or workspace / "sovereignty_proposals.json"
                ).resolve(),
                agency_request_dir=(
                    args.agency_request_dir or workspace / "agency_requests"
                ).resolve(),
                correspondence_ledger=args.correspondence_ledger.resolve(),
                write=args.write and args.command == "project",
            )
            value = projector_receipt("agency_commons", status,
                                      {"status.json": state_dir(workspace) / "status.json",
                                       "records.jsonl": state_dir(workspace) / "records.jsonl",
                                       "report.md": state_dir(workspace) / "report.md"},
                                      started_monotonic=started) if args.receipt_json else status
        elif args.command == "report":
            path = state_dir(workspace) / "status.json"
            value = json.loads(path.read_text()) if path.is_file() else {"valid": False, "error": "status_missing"}
        else: value = {"schema": "agency_commons_query_v1", "records": query(workspace, args.id)}
    except (RecordValidationError, ValueError, TypeError) as error:
        value = {"valid": False, "error": str(error)}
    print(json.dumps(value, indent=2, sort_keys=True))
    return 0 if value.get("valid", True) is not False else 1
