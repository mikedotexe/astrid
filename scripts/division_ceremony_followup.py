#!/usr/bin/env python3
"""Track bounded returns to the ESN Division Ceremony between introspection rounds."""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import sys
import tempfile
import time
from contextlib import contextmanager
from pathlib import Path
from typing import Any, Iterator


EVENT_SCHEMA = "division.ceremony_followup_event.v1"
STATE_SCHEMA = "division.ceremony_followup_cycle.v1"
THRESHOLD_ROUNDS = 6
MAX_EVENTS = 4096
ROOT = Path(__file__).resolve().parents[1]
DEFAULT_WORKSPACE = ROOT.parent / "minime" / "workspace"


class FollowupError(ValueError):
    """The follow-up evidence is malformed or violates the interval contract."""


def canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def file_hash(path: Path) -> str:
    if not path.is_file():
        raise FollowupError(f"required file is missing: {path}")
    return sha256_bytes(path.read_bytes())


def followup_dir(workspace: Path) -> Path:
    return workspace / "division" / "followup"


def events_path(workspace: Path) -> Path:
    return followup_dir(workspace) / "events_v1.jsonl"


def state_path(workspace: Path) -> Path:
    return followup_dir(workspace) / "cycle_v1.json"


def event_digest(event: dict[str, Any]) -> str:
    value = dict(event)
    value.pop("event_sha256", None)
    return sha256_bytes(canonical(value).encode())


def deterministic_event_id(kind: str, identity: dict[str, Any]) -> str:
    material = {"kind": kind, **identity}
    return "division_followup_event_" + sha256_bytes(canonical(material).encode())[:32]


def authority_boundary() -> dict[str, bool | str]:
    return {
        "state": "evidence_only",
        "silence_infers_consent": False,
        "followup_recommends_action": False,
        "followup_dispatches_action": False,
        "followup_grants_authority": False,
        "felt_state_inferred": False,
        "raw_prose_included": False,
    }


def validate_event(event: dict[str, Any], *, sequence: int, previous: str | None) -> None:
    if event.get("schema") != EVENT_SCHEMA or event.get("schema_version") != 1:
        raise FollowupError("follow-up event schema mismatch")
    if event.get("sequence") != sequence:
        raise FollowupError("follow-up event sequence mismatch")
    if event.get("previous_event_sha256") != previous:
        raise FollowupError("follow-up event chain mismatch")
    if event.get("event_sha256") != event_digest(event):
        raise FollowupError("follow-up event digest mismatch")
    if event.get("kind") not in {
        "introspection_round_completed",
        "division_followup_completed",
    }:
        raise FollowupError("unsupported follow-up event kind")
    if event.get("authority") != authority_boundary():
        raise FollowupError("follow-up authority boundary mismatch")
    if event["kind"] == "introspection_round_completed":
        count = event.get("processed_report_count")
        if (
            event.get("finish_outcome") != "success"
            or not isinstance(count, int)
            or not 1 <= count <= 40
            or not str(event.get("steward_run_id") or "").strip()
        ):
            raise FollowupError("invalid completed introspection round")
    else:
        if not str(event.get("chronicle_id") or "").startswith(
            "division_chronicle_"
        ):
            raise FollowupError("follow-up lacks a bounded Chronicle identity")
        for key in (
            "chronicle_json_sha256",
            "astrid_note_sha256",
            "minime_note_sha256",
        ):
            value = event.get(key)
            if not isinstance(value, str) or len(value) != 64:
                raise FollowupError(f"follow-up has invalid {key}")


def load_events(workspace: Path) -> list[dict[str, Any]]:
    path = events_path(workspace)
    if not path.is_file():
        return []
    rows: list[dict[str, Any]] = []
    previous: str | None = None
    for line_number, line in enumerate(path.read_text().splitlines(), 1):
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise FollowupError(f"invalid JSON at event line {line_number}") from error
        if not isinstance(event, dict):
            raise FollowupError(f"event line {line_number} is not an object")
        validate_event(event, sequence=len(rows) + 1, previous=previous)
        rows.append(event)
        previous = event["event_sha256"]
        if len(rows) > MAX_EVENTS:
            raise FollowupError("follow-up event limit exceeded")
    return rows


def project_state(events: list[dict[str, Any]]) -> dict[str, Any]:
    completed = 0
    cycle_sequence = 0
    last_round_event_id: str | None = None
    latest_followup: dict[str, Any] | None = None
    round_event_ids: list[str] = []
    for event in events:
        if event["kind"] == "introspection_round_completed":
            if completed >= THRESHOLD_ROUNDS:
                raise FollowupError("a seventh round cannot bypass a due follow-up")
            completed += 1
            last_round_event_id = event["event_id"]
            round_event_ids.append(event["event_id"])
        else:
            baseline = bool(event.get("baseline"))
            if not baseline and completed < THRESHOLD_ROUNDS:
                raise FollowupError("follow-up recorded before six completed rounds")
            if baseline and cycle_sequence != 0:
                raise FollowupError("baseline follow-up can only initialize the tracker")
            latest_followup = {
                key: event.get(key)
                for key in (
                    "event_id",
                    "recorded_at_unix_ms",
                    "chronicle_id",
                    "chronicle_json_sha256",
                    "astrid_note_sha256",
                    "minime_note_sha256",
                    "baseline",
                    "completed_rounds_observed",
                )
            }
            cycle_sequence += 1
            completed = 0
            round_event_ids = []
    return {
        "schema": STATE_SCHEMA,
        "schema_version": 1,
        "threshold_rounds": THRESHOLD_ROUNDS,
        "cycle_sequence": cycle_sequence,
        "completed_rounds_since_followup": completed,
        "rounds_remaining_before_followup": max(0, THRESHOLD_ROUNDS - completed),
        "review_due": completed >= THRESHOLD_ROUNDS,
        "last_round_event_id": last_round_event_id,
        "current_cycle_round_event_ids": round_event_ids,
        "latest_followup": latest_followup,
        "event_count": len(events),
        "event_head_sha256": events[-1]["event_sha256"] if events else None,
        "authority": authority_boundary(),
    }


def atomic_owner_write(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(path.parent, 0o700)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        os.chmod(path, 0o600)
    finally:
        if temporary.exists():
            temporary.unlink()


@contextmanager
def exclusive_lock(workspace: Path) -> Iterator[None]:
    directory = followup_dir(workspace)
    directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(directory, 0o700)
    path = directory / ".cycle.lock"
    descriptor = os.open(path, os.O_RDWR | os.O_CREAT, 0o600)
    try:
        os.chmod(path, 0o600)
        fcntl.flock(descriptor, fcntl.LOCK_EX)
        yield
    finally:
        fcntl.flock(descriptor, fcntl.LOCK_UN)
        os.close(descriptor)


def persist_state(workspace: Path, events: list[dict[str, Any]]) -> dict[str, Any]:
    state = project_state(events)
    atomic_owner_write(
        state_path(workspace),
        (json.dumps(state, indent=2, sort_keys=True) + "\n").encode(),
    )
    return state


def append_event(
    workspace: Path, kind: str, identity: dict[str, Any], fields: dict[str, Any]
) -> dict[str, Any]:
    with exclusive_lock(workspace):
        events = load_events(workspace)
        event_id = deterministic_event_id(kind, identity)
        existing = next(
            (event for event in events if event.get("event_id") == event_id), None
        )
        if existing is not None:
            return persist_state(workspace, events)
        previous = events[-1]["event_sha256"] if events else None
        event = {
            "schema": EVENT_SCHEMA,
            "schema_version": 1,
            "event_id": event_id,
            "sequence": len(events) + 1,
            "previous_event_sha256": previous,
            "kind": kind,
            "recorded_at_unix_ms": int(time.time() * 1000),
            **fields,
            "authority": authority_boundary(),
        }
        event["event_sha256"] = event_digest(event)
        validate_event(event, sequence=len(events) + 1, previous=previous)
        path = events_path(workspace)
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
        os.chmod(path, 0o600)
        with os.fdopen(descriptor, "a") as handle:
            handle.write(canonical(event) + "\n")
            handle.flush()
            os.fsync(handle.fileno())
        events.append(event)
        return persist_state(workspace, events)


def record_round(
    workspace: Path,
    *,
    steward_run_id: str,
    processed_report_count: int,
    projection_generation_id: str,
) -> dict[str, Any]:
    events = load_events(workspace)
    event_id = deterministic_event_id(
        "introspection_round_completed", {"steward_run_id": steward_run_id}
    )
    if any(event.get("event_id") == event_id for event in events):
        return persist_state(workspace, events)
    current = project_state(events)
    if current["review_due"]:
        raise FollowupError("Division follow-up is due before another round")
    return append_event(
        workspace,
        "introspection_round_completed",
        {"steward_run_id": steward_run_id},
        {
            "steward_run_id": steward_run_id,
            "processed_report_count": processed_report_count,
            "projection_generation_id": projection_generation_id,
            "finish_outcome": "success",
        },
    )


def record_followup(
    workspace: Path,
    *,
    chronicle_json: Path,
    astrid_note: Path,
    minime_note: Path,
    baseline: bool,
) -> dict[str, Any]:
    chronicle = json.loads(chronicle_json.read_text())
    if not isinstance(chronicle, dict):
        raise FollowupError("Chronicle must be a JSON object")
    chronicle_id = str(chronicle.get("chronicle_id") or "")
    authority = chronicle.get("authority")
    if (
        not chronicle_id.startswith("division_chronicle_")
        or not isinstance(authority, dict)
        or authority.get("commit_recommended") is not False
        or authority.get("silence_infers_consent") is not False
    ):
        raise FollowupError("Chronicle authority boundary is invalid")
    current = project_state(load_events(workspace))
    if baseline and current["event_count"] != 0:
        raise FollowupError("baseline follow-up requires an empty tracker")
    if not baseline and not current["review_due"]:
        raise FollowupError("six completed rounds are required before follow-up")
    hashes = {
        "chronicle_json_sha256": file_hash(chronicle_json),
        "astrid_note_sha256": file_hash(astrid_note),
        "minime_note_sha256": file_hash(minime_note),
    }
    return append_event(
        workspace,
        "division_followup_completed",
        {
            "chronicle_id": chronicle_id,
            **hashes,
            "cycle_sequence": current["cycle_sequence"],
        },
        {
            "chronicle_id": chronicle_id,
            **hashes,
            "baseline": baseline,
            "completed_rounds_observed": current[
                "completed_rounds_since_followup"
            ],
        },
    )


def verify(workspace: Path) -> dict[str, Any]:
    events = load_events(workspace)
    expected = project_state(events)
    path = state_path(workspace)
    if not path.is_file():
        if events:
            raise FollowupError("follow-up state projection is missing")
        return {"ok": True, **expected}
    persisted = json.loads(path.read_text())
    if persisted != expected:
        raise FollowupError("follow-up state differs from event replay")
    for owner_path in (events_path(workspace), path):
        if owner_path.is_file() and owner_path.stat().st_mode & 0o077:
            raise FollowupError(f"{owner_path} is not owner-only")
    return {"ok": True, **expected}


def self_test() -> None:
    with tempfile.TemporaryDirectory() as raw:
        workspace = Path(raw) / "workspace"
        for index in range(1, 7):
            state = record_round(
                workspace,
                steward_run_id=f"run-{index}",
                processed_report_count=index,
                projection_generation_id=f"generation-{index}",
            )
        assert state["review_due"] is True
        try:
            record_round(
                workspace,
                steward_run_id="run-7",
                processed_report_count=1,
                projection_generation_id="generation-7",
            )
        except FollowupError:
            pass
        else:
            raise AssertionError("seventh round bypassed due follow-up")
        chronicle = workspace / "chronicle.json"
        chronicle.write_text(
            json.dumps(
                {
                    "chronicle_id": "division_chronicle_test",
                    "authority": {
                        "commit_recommended": False,
                        "silence_infers_consent": False,
                    },
                }
            )
        )
        astrid_note = workspace / "astrid-note.txt"
        minime_note = workspace / "minime-note.txt"
        astrid_note.write_text("bounded fixture")
        minime_note.write_text("bounded fixture")
        state = record_followup(
            workspace,
            chronicle_json=chronicle,
            astrid_note=astrid_note,
            minime_note=minime_note,
            baseline=False,
        )
        assert state["completed_rounds_since_followup"] == 0
        assert state["cycle_sequence"] == 1
        assert verify(workspace)["ok"] is True
        assert record_round(
            workspace,
            steward_run_id="run-next",
            processed_report_count=1,
            projection_generation_id="generation-next",
        )["completed_rounds_since_followup"] == 1
    print("division ceremony follow-up self-test: ok")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("status")
    commands.add_parser("verify")
    commands.add_parser("self-test")
    round_parser = commands.add_parser("record-round")
    round_parser.add_argument("--steward-run-id", required=True)
    round_parser.add_argument("--processed-report-count", required=True, type=int)
    round_parser.add_argument("--projection-generation-id", required=True)
    followup_parser = commands.add_parser("record-followup")
    followup_parser.add_argument("--chronicle-json", required=True, type=Path)
    followup_parser.add_argument("--astrid-note", required=True, type=Path)
    followup_parser.add_argument("--minime-note", required=True, type=Path)
    followup_parser.add_argument("--baseline", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "status":
            print(json.dumps(project_state(load_events(args.workspace)), indent=2))
        elif args.command == "verify":
            print(json.dumps(verify(args.workspace), indent=2))
        elif args.command == "self-test":
            self_test()
        elif args.command == "record-round":
            print(
                json.dumps(
                    record_round(
                        args.workspace,
                        steward_run_id=args.steward_run_id,
                        processed_report_count=args.processed_report_count,
                        projection_generation_id=args.projection_generation_id,
                    ),
                    indent=2,
                )
            )
        else:
            print(
                json.dumps(
                    record_followup(
                        args.workspace,
                        chronicle_json=args.chronicle_json,
                        astrid_note=args.astrid_note,
                        minime_note=args.minime_note,
                        baseline=args.baseline,
                    ),
                    indent=2,
                )
            )
    except (FollowupError, OSError, json.JSONDecodeError) as error:
        print(f"division ceremony follow-up error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
