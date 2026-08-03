#!/usr/bin/env python3
"""Retire one contaminated CPU-edge model-session generation without a turn.

This is an operator compatibility repair, not an autonomy action.  It changes
only the ordinary session generation and leaves attempt, authorship, recovery,
failure, schedule, chain, and artifact state untouched.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import stat
import subprocess
import time
import uuid
from pathlib import Path
from typing import Any


STATE_SCHEMA = "astrid_edge_autonomy_state_v3"
RECEIPT_SCHEMA = "astrid_edge_operator_session_retirement_v1"
PRESERVED_FIELDS = (
    "attempts_today",
    "authored_turns_today",
    "transport_recoveries_today",
    "total_attempts",
    "total_authored_turns",
    "total_transport_recoveries",
    "consecutive_failures",
    "next_due_at_unix_ms",
    "last_started_at_unix_ms",
    "last_completed_at_unix_ms",
    "last_status",
    "chain_session_generation",
    "chain_session_authored_turns",
    "active_chain_id",
    "active_chain_step",
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def require_private_regular(path: Path) -> None:
    details = path.lstat()
    if not stat.S_ISREG(details.st_mode) or path.is_symlink():
        raise RuntimeError(f"required private state is not a regular file: {path}")
    if details.st_uid != os.getuid():
        raise RuntimeError(f"required private state is not owned by this user: {path}")
    if stat.S_IMODE(details.st_mode) & 0o077:
        raise RuntimeError(f"required private state is not owner-only: {path}")


def append_receipt(path: Path, receipt: dict[str, Any]) -> None:
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    flags = os.O_WRONLY | os.O_APPEND | os.O_CREAT
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, 0o600)
    try:
        os.fchmod(descriptor, 0o600)
        payload = (
            json.dumps(receipt, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
            + "\n"
        ).encode()
        with os.fdopen(descriptor, "ab", closefd=False) as stream:
            stream.write(payload)
            stream.flush()
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def read_receipts(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError:
        return []
    values: list[dict[str, Any]] = []
    for line in lines:
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            values.append(value)
    return values


def atomic_replace(path: Path, payload: bytes) -> None:
    temporary = path.with_name(f".{path.name}.retire-{uuid.uuid4()}")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(temporary, flags, 0o600)
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as stream:
            stream.write(payload)
            stream.flush()
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    os.replace(temporary, path)
    directory = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


def assert_edge_service_inactive() -> None:
    result = subprocess.run(
        ["systemctl", "--user", "is-active", "astrid-edge-runtime.service"],
        check=False,
        capture_output=True,
        text=True,
        timeout=5,
    )
    state = result.stdout.strip()
    if state not in {"inactive", "failed"}:
        raise RuntimeError(
            "astrid-edge-runtime.service must be stopped before session retirement "
            f"(observed {state or 'unknown'})"
        )


def retirement_transition(
    state: dict[str, Any],
    *,
    expected_generation: int,
    expected_session_name: str,
    expected_trace_id: str,
) -> dict[str, Any]:
    if state.get("schema") != STATE_SCHEMA:
        raise RuntimeError(f"unsupported autonomy state schema: {state.get('schema')!r}")
    if state.get("ordinary_session_generation") != expected_generation:
        raise RuntimeError("ordinary session generation changed; refusing stale repair")
    if state.get("ordinary_session_authored_turns") != 0:
        raise RuntimeError("ordinary session already has authored turns; refusing retirement")
    if state.get("last_session_name") != expected_session_name:
        raise RuntimeError("last session name does not match the contaminated session")
    if state.get("last_trace_id") != expected_trace_id:
        raise RuntimeError("last trace does not match the rejected terminal response")
    if state.get("active_chain_id") is not None:
        raise RuntimeError("an Action chain is active; refusing ordinary-session retirement")
    for pending in (
        "run_receipt_pending",
        "chain_receipt_pending",
        "action_dispatch_pending",
    ):
        if state.get(pending, False):
            raise RuntimeError(f"{pending} is set; refusing session retirement")

    updated = copy.deepcopy(state)
    updated["ordinary_session_generation"] = expected_generation + 1
    updated["ordinary_session_authored_turns"] = 0
    for field in PRESERVED_FIELDS:
        if updated.get(field) != state.get(field):
            raise AssertionError(f"retirement changed preserved field {field}")
    changed = {
        key
        for key in state.keys() | updated.keys()
        if state.get(key) != updated.get(key)
    }
    if changed != {"ordinary_session_generation"}:
        raise AssertionError(f"unexpected autonomy state changes: {sorted(changed)}")
    return updated


def retire(
    workspace: Path,
    *,
    expected_generation: int,
    expected_session_name: str,
    expected_trace_id: str,
    reason: str,
    dry_run: bool,
    require_inactive_service: bool = True,
) -> dict[str, Any]:
    if not workspace.is_absolute():
        raise RuntimeError("--workspace must be absolute")
    if not reason or len(reason) > 240 or any(ord(char) < 0x20 for char in reason):
        raise RuntimeError("--reason must be 1-240 printable characters")
    if require_inactive_service:
        assert_edge_service_inactive()

    state_path = workspace / "autonomous/state.json"
    receipt_path = workspace / "autonomous/session_retirements.jsonl"
    require_private_regular(state_path)
    state_bytes = state_path.read_bytes()
    try:
        state = json.loads(state_bytes)
    except json.JSONDecodeError as error:
        raise RuntimeError("autonomy state is not valid JSON") from error
    if not isinstance(state, dict):
        raise RuntimeError("autonomy state must be a JSON object")
    if state.get("ordinary_session_generation") == expected_generation + 1:
        matching = [
            item
            for item in read_receipts(receipt_path)
            if item.get("schema") == RECEIPT_SCHEMA
            and item.get("prior_session_generation") == expected_generation
            and item.get("new_session_generation") == expected_generation + 1
            and item.get("prior_session_name") == expected_session_name
            and item.get("rejected_trace_id") == expected_trace_id
            and item.get("state_after_sha256") == sha256(state_bytes)
        ]
        completed = next(
            (item for item in reversed(matching) if item.get("phase") == "completed"),
            None,
        )
        if completed is not None:
            return {**completed, "idempotent_replay": True}
        requested = next(
            (item for item in reversed(matching) if item.get("phase") == "requested"),
            None,
        )
        if requested is None:
            raise RuntimeError(
                "session is already advanced without a matching retirement receipt"
            )
        recovered = {
            **requested,
            "phase": "completed",
            "recovered_after_interrupted_receipt": True,
        }
        if not dry_run:
            append_receipt(receipt_path, recovered)
            require_private_regular(receipt_path)
        return recovered
    updated = retirement_transition(
        state,
        expected_generation=expected_generation,
        expected_session_name=expected_session_name,
        expected_trace_id=expected_trace_id,
    )
    updated_bytes = (json.dumps(updated, indent=2, sort_keys=True) + "\n").encode()
    transition_id = str(uuid.uuid4())
    recorded_at = time.time_ns() // 1_000_000
    base_receipt = {
        "schema": RECEIPT_SCHEMA,
        "transition_id": transition_id,
        "recorded_at_unix_ms": recorded_at,
        "reason": reason,
        "prior_session_generation": expected_generation,
        "new_session_generation": expected_generation + 1,
        "prior_session_name": expected_session_name,
        "rejected_trace_id": expected_trace_id,
        "state_before_sha256": sha256(state_bytes),
        "state_after_sha256": sha256(updated_bytes),
        "preserved_state": {field: state.get(field) for field in PRESERVED_FIELDS},
        "authority": "operator_compatibility_repair_no_turn_no_authorship_no_counter_reset",
    }
    if dry_run:
        return {**base_receipt, "phase": "dry_run"}

    append_receipt(receipt_path, {**base_receipt, "phase": "requested"})
    atomic_replace(state_path, updated_bytes)
    append_receipt(receipt_path, {**base_receipt, "phase": "completed"})
    require_private_regular(state_path)
    require_private_regular(receipt_path)
    return {**base_receipt, "phase": "completed"}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path, required=True)
    parser.add_argument("--expected-generation", type=int, required=True)
    parser.add_argument("--expected-session-name", required=True)
    parser.add_argument("--expected-trace-id", required=True)
    parser.add_argument("--reason", required=True)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    result = retire(
        args.workspace,
        expected_generation=args.expected_generation,
        expected_session_name=args.expected_session_name,
        expected_trace_id=args.expected_trace_id,
        reason=args.reason,
        dry_run=args.dry_run,
    )
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
