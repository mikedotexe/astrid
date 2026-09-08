#!/usr/bin/env python3
"""Issue and verify nonce-scoped flywheel child completion receipts."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import tempfile
import time
from typing import Any


SCHEMA = "flywheel_round_completion_v1"
REQUIRED_PACKET_FILES = (
    "RUN_REPORT.md",
    "addressing_links.json",
    "read_manifest.json",
    "source_receipts.json",
    "test_results.json",
    "unprocessed_selected.json",
    "verification_receipt.json",
)
NO_INPUT_REASONS = {
    "canonical_queue_empty",
    "foreign_activity",
    "no_processable_canonical_input",
}


class CompletionError(RuntimeError):
    """The child did not prove an eligible terminal state."""


def _repo_root() -> Path:
    configured = os.environ.get("FLYWHEEL_ASTRID_ROOT")
    return Path(configured).resolve() if configured else Path(__file__).resolve().parents[1]


def _run_identity() -> tuple[str, str]:
    run_id = os.environ.get("STEWARD_RUN_ID", "").strip()
    actor = os.environ.get("STEWARD_ACTOR", "").strip()
    if not run_id or not actor:
        raise CompletionError(
            "STEWARD_RUN_ID and STEWARD_ACTOR must come from the controller adapter"
        )
    return run_id, actor


def _marker_path(explicit: str | None = None) -> Path:
    raw = explicit or os.environ.get("FLYWHEEL_ROUND_COMPLETION_FILE", "")
    if not raw:
        raise CompletionError("completion marker path is unavailable")
    return Path(raw)


def _require_private_directory(path: Path) -> None:
    info = path.lstat()
    if not stat.S_ISDIR(info.st_mode) or path.is_symlink():
        raise CompletionError(f"completion directory is not a real directory: {path}")
    if info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) & 0o077:
        raise CompletionError(f"completion directory is not owner-private: {path}")


def _require_regular_file(path: Path) -> None:
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or path.is_symlink():
        raise CompletionError(f"required regular file is unavailable: {path}")


def _load_json(path: Path) -> dict[str, Any] | list[Any]:
    _require_regular_file(path)
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise CompletionError(f"invalid JSON in {path}: {error}") from error
    if not isinstance(value, (dict, list)):
        raise CompletionError(f"JSON artifact must be an object or array: {path}")
    return value


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _packet_path(raw: str) -> tuple[Path, str]:
    root = _repo_root()
    candidate = Path(raw)
    if not candidate.is_absolute():
        candidate = root / candidate
    if candidate.is_symlink():
        raise CompletionError("run packet cannot be a symlink")
    try:
        packet = candidate.resolve(strict=True)
        relative = packet.relative_to((root / "docs" / "steward-notes").resolve())
    except (OSError, ValueError) as error:
        raise CompletionError("run packet must be under docs/steward-notes") from error
    if len(relative.parts) != 1 or not relative.name.startswith("claude-heartbeat_"):
        raise CompletionError("run packet must be one claude-heartbeat directory")
    if not packet.is_dir():
        raise CompletionError("run packet is not a directory")
    return packet, str(packet.relative_to(root))


def _packet_hashes(packet: Path) -> dict[str, str]:
    hashes: dict[str, str] = {}
    for path in sorted(packet.rglob("*")):
        if path.is_symlink():
            raise CompletionError(f"run packet contains a symlink: {path}")
        if path.is_file():
            _require_regular_file(path)
            hashes[str(path.relative_to(packet))] = _sha256(path)
    return hashes


def _validate_complete_packet(
    raw_packet: str,
    processed_report_count: int,
    run_id: str,
    actor: str,
) -> tuple[str, dict[str, str]]:
    if processed_report_count < 1:
        raise CompletionError("a complete productive round must process at least one report")
    packet, relative = _packet_path(raw_packet)
    for name in REQUIRED_PACKET_FILES:
        path = packet / name
        _require_regular_file(path)
        if name.endswith(".json"):
            _load_json(path)
    for name in ("claims", "summaries"):
        path = packet / name
        if path.is_symlink() or not path.is_dir():
            raise CompletionError(f"required run packet directory is unavailable: {path}")

    report = packet / "RUN_REPORT.md"
    if not report.read_text(encoding="utf-8").strip():
        raise CompletionError("RUN_REPORT.md is empty")

    manifest = _load_json(packet / "read_manifest.json")
    if not isinstance(manifest, dict):
        raise CompletionError("read_manifest.json must be an object")
    reports = manifest.get("reports")
    if not isinstance(reports, list) or len(reports) != processed_report_count:
        raise CompletionError("read manifest report count does not match the round")
    if any(not isinstance(item, dict) or item.get("read") != "complete" for item in reports):
        raise CompletionError("every processed report needs a complete read receipt")

    claim_files = list((packet / "claims").glob("*.json"))
    summary_files = list((packet / "summaries").glob("*.md"))
    if len(claim_files) < processed_report_count:
        raise CompletionError("one claims artifact is required per processed report")
    if len(summary_files) < processed_report_count:
        raise CompletionError("one summary artifact is required per processed report")

    verification = _load_json(packet / "verification_receipt.json")
    if not isinstance(verification, dict):
        raise CompletionError("verification_receipt.json must be an object")
    controller = verification.get("controller")
    addressing = verification.get("addressing")
    division = verification.get("division")
    if not isinstance(controller, dict) or controller.get("run_id") != run_id:
        raise CompletionError("verification receipt is not bound to this steward run")
    if verification.get("actor") != actor:
        raise CompletionError("verification receipt actor does not match the adapter")
    if (
        controller.get("preprojection_status") != "passed"
        or not controller.get("preprojection_generation_id")
    ):
        raise CompletionError("successful source-first preprojection evidence is absent")
    if not isinstance(addressing, dict):
        raise CompletionError("verification receipt lacks addressing evidence")
    if addressing.get("fully_addressed") is not True:
        raise CompletionError("processed reports are not fully addressed")
    if addressing.get("proof_missing_claims") != []:
        raise CompletionError("processed reports still have proof-missing claims")
    if (
        addressing.get("full_read") is not True
        and addressing.get("full_read_count") != processed_report_count
    ):
        raise CompletionError("verification receipt lacks complete read evidence")
    if not isinstance(division, dict):
        raise CompletionError("verification receipt lacks Division round evidence")
    if division.get("processed_report_count") != processed_report_count:
        raise CompletionError("Division processed count does not match the round")
    if not division.get("round_event_id"):
        raise CompletionError("Division productive-round event is absent")

    epistemic = verification.get("epistemic_lint")
    counters = verification.get("counters")
    event_store = verification.get("evidence_event_store")
    if not isinstance(epistemic, dict) or epistemic.get("valid") is not True:
        raise CompletionError("final epistemic verification is not valid")
    if epistemic.get("issue_count") != 0:
        raise CompletionError("final epistemic verification still has issues")
    if epistemic.get("history_rewritten") is not False:
        raise CompletionError("epistemic history rewrite cannot be marked complete")
    if not isinstance(counters, dict) or counters.get("status") != "consistent":
        raise CompletionError("addressing counters are not consistent")
    if counters.get("mismatches") != []:
        raise CompletionError("addressing counter mismatches remain")
    if not isinstance(event_store, dict) or event_store.get("valid") is not True:
        raise CompletionError("Evidence Event Store verification is not valid")
    if event_store.get("corrupt_lines") != 0:
        raise CompletionError("Evidence Event Store has corrupt lines")
    if event_store.get("errors") != []:
        raise CompletionError("Evidence Event Store verification has errors")

    anti_drop = verification.get("anti_drop")
    if not isinstance(anti_drop, dict):
        raise CompletionError("anti-drop verification is absent")
    anti_drop_verify = anti_drop.get("verify")
    if isinstance(anti_drop_verify, dict):
        alarms = anti_drop_verify.get("alarms")
        gaps = anti_drop_verify.get("gaps")
    else:
        alarms = anti_drop.get("alarms_after")
        gaps = anti_drop.get("gaps")
    if alarms != 0 or gaps != 0:
        raise CompletionError("anti-drop verification is not clean")

    for path in claim_files:
        _load_json(path)
    for path in summary_files:
        if not path.read_text(encoding="utf-8").strip():
            raise CompletionError(f"summary artifact is empty: {path}")

    return relative, _packet_hashes(packet)


def _write_marker(payload: dict[str, Any]) -> None:
    path = _marker_path()
    _require_private_directory(path.parent)
    if os.path.lexists(path):
        raise CompletionError("completion marker already exists")
    encoded = (json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n").encode()
    fd, temporary = tempfile.mkstemp(prefix=".completion-", dir=path.parent)
    temporary_path = Path(temporary)
    try:
        os.fchmod(fd, 0o600)
        with os.fdopen(fd, "wb") as stream:
            stream.write(encoded)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary_path, path)
    finally:
        if temporary_path.exists():
            temporary_path.unlink()


def record_complete(args: argparse.Namespace) -> dict[str, Any]:
    run_id, actor = _run_identity()
    packet, hashes = _validate_complete_packet(
        args.run_packet,
        args.processed_report_count,
        run_id,
        actor,
    )
    payload = {
        "schema": SCHEMA,
        "outcome": "complete",
        "run_id": run_id,
        "actor": actor,
        "processed_report_count": args.processed_report_count,
        "run_packet": packet,
        "packet_files": hashes,
        "created_at_unix_ns": time.time_ns(),
    }
    _write_marker(payload)
    return payload


def record_no_input(args: argparse.Namespace) -> dict[str, Any]:
    run_id, actor = _run_identity()
    if args.reason_code not in NO_INPUT_REASONS:
        raise CompletionError(f"unsupported no-input reason: {args.reason_code}")
    payload = {
        "schema": SCHEMA,
        "outcome": "no_input",
        "run_id": run_id,
        "actor": actor,
        "processed_report_count": 0,
        "run_packet": None,
        "reason_code": args.reason_code,
        "created_at_unix_ns": time.time_ns(),
    }
    _write_marker(payload)
    return payload


def verify_marker(args: argparse.Namespace) -> dict[str, Any]:
    run_id, actor = _run_identity()
    path = _marker_path(args.marker)
    _require_regular_file(path)
    info = path.stat()
    if info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) & 0o077:
        raise CompletionError("completion marker is not owner-private")
    payload = _load_json(path)
    if not isinstance(payload, dict) or payload.get("schema") != SCHEMA:
        raise CompletionError("unsupported completion marker schema")
    if payload.get("run_id") != run_id or payload.get("actor") != actor:
        raise CompletionError("completion marker belongs to a different run")
    outcome = payload.get("outcome")
    if outcome == "complete":
        count = payload.get("processed_report_count")
        packet = payload.get("run_packet")
        if isinstance(count, bool) or not isinstance(count, int) or not isinstance(packet, str):
            raise CompletionError("complete marker fields are malformed")
        _, hashes = _validate_complete_packet(packet, count, run_id, actor)
        if payload.get("packet_files") != hashes:
            raise CompletionError("run packet changed after completion was recorded")
    elif outcome == "no_input":
        if payload.get("processed_report_count") != 0 or payload.get("run_packet") is not None:
            raise CompletionError("no-input marker cannot claim productive work")
        if payload.get("reason_code") not in NO_INPUT_REASONS:
            raise CompletionError("no-input marker reason is invalid")
    else:
        raise CompletionError("completion marker outcome is invalid")
    return payload


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command", required=True)
    complete = commands.add_parser("complete")
    complete.add_argument("--run-packet", required=True)
    complete.add_argument("--processed-report-count", required=True, type=int)
    no_input = commands.add_parser("no-input")
    no_input.add_argument("--reason-code", required=True, choices=sorted(NO_INPUT_REASONS))
    verify = commands.add_parser("verify")
    verify.add_argument("--marker")
    return result


def main() -> int:
    args = parser().parse_args()
    try:
        if args.command == "complete":
            payload = record_complete(args)
        elif args.command == "no-input":
            payload = record_no_input(args)
        else:
            payload = verify_marker(args)
    except (CompletionError, OSError, UnicodeError) as error:
        print(f"flywheel completion: {error}", file=os.sys.stderr)
        return 1
    print(
        json.dumps(
            {
                "schema": payload["schema"],
                "outcome": payload["outcome"],
                "run_id": payload["run_id"],
                "processed_report_count": payload["processed_report_count"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
