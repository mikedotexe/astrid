#!/usr/bin/env python3
"""Quarantine Actions produced by autonomous responses completed after interruption.

The immutable Action/recovery ledgers remain untouched.  Exact trace and response-hash joins
identify the invalid authority transition; an owner-only correction ledger records it, affected
artifacts move out of Astrid's readable tree, and bounded current continuity is reconciled.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import stat
import time
from typing import Any


SCHEMA = "astrid_edge_interrupted_action_correction_v1"


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return {}
    return value if isinstance(value, dict) else {}


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(errors="replace").splitlines()
    except OSError:
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


def trace_id(value: dict[str, Any]) -> str:
    trace = value.get("trace")
    return str(trace.get("trace_id") or "") if isinstance(trace, dict) else ""


def interrupted_actions(workspace: Path) -> list[dict[str, Any]]:
    recoveries = {
        trace_id(item): int(item.get("completed_at_unix_ms", 0) or 0)
        for item in read_jsonl(workspace / "autonomous/recoveries.jsonl")
        if item.get("status") == "interrupted" and trace_id(item)
    }
    authored = {
        (trace_id(item), str(item.get("response_sha256") or ""))
        for item in read_jsonl(workspace / "autonomous/runs.jsonl")
        if item.get("status") == "authored_completed"
    }
    corrected = {
        str(item.get("response_sha256") or "")
        for item in read_jsonl(workspace / "actions/interrupted_corrections.jsonl")
    }
    result = []
    for action in read_jsonl(workspace / "actions/receipts.jsonl"):
        action_trace = trace_id(action)
        response_hash = str(action.get("response_sha256") or "")
        if (
            action_trace in recoveries
            and int(action.get("recorded_at_unix_ms", 0) or 0)
            >= recoveries[action_trace]
            and (action_trace, response_hash) not in authored
            and response_hash not in corrected
        ):
            result.append(action)
    return result


def artifact_relative(value: Any) -> Path | None:
    text = str(value or "")
    prefix = "home://edge/"
    if not text.startswith(prefix):
        return None
    pure = PurePosixPath(text[len(prefix) :])
    if pure.is_absolute() or not pure.parts or any(part in {"", ".", ".."} for part in pure.parts):
        return None
    return Path(*pure.parts)


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=False) + "\n")
    os.chmod(temporary, 0o600)
    os.replace(temporary, path)


def append_owner_jsonl(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    with os.fdopen(descriptor, "a", encoding="utf-8") as handle:
        handle.write(json.dumps(value, separators=(",", ":")) + "\n")
        handle.flush()
        os.fsync(handle.fileno())
    os.chmod(path, 0o600)


def action_argument(action: dict[str, Any]) -> str:
    declaration = str(action.get("declared_next") or "")
    return declaration.split(" ", 1)[1] if " " in declaration else ""


def matching_bounded(value: Any, expected: str) -> bool:
    actual = str(value or "")
    return bool(actual and expected and (actual == expected or actual.startswith(expected) or expected.startswith(actual)))


def remove_one_bounded(values: list[Any], expected: str) -> list[Any]:
    exact = [index for index, value in enumerate(values) if str(value or "") == expected]
    candidates = exact or [
        index for index, value in enumerate(values) if matching_bounded(value, expected)
    ]
    if not candidates:
        return values
    remove_index = candidates[-1]
    return [value for index, value in enumerate(values) if index != remove_index]


def reconcile_thread(workspace: Path, actions: list[dict[str, Any]], now_ms: int) -> bool:
    path = workspace / "autonomous/thread_state.json"
    thread = read_json(path)
    if not thread:
        return False
    response_hashes = {str(action.get("response_sha256") or "") for action in actions}
    timestamps = {int(action.get("recorded_at_unix_ms", 0) or 0) for action in actions}
    basenames = {
        relative.name
        for action in actions
        if (relative := artifact_relative(action.get("artifact_path"))) is not None
    }
    arguments = [action_argument(action) for action in actions]
    removed_hashes: set[str] = set()
    retained_evidence = []
    for evidence in thread.get("evidence_records", []):
        if not isinstance(evidence, dict):
            continue
        if (
            int(evidence.get("captured_at_unix_ms", 0) or 0) in timestamps
            and str(evidence.get("reference") or "") in basenames
        ):
            removed_hashes.add(str(evidence.get("sha256") or ""))
        else:
            retained_evidence.append(evidence)
    thread["evidence_records"] = retained_evidence
    thread["provenance_hashes"] = [
        value for value in thread.get("provenance_hashes", []) if str(value) not in removed_hashes
    ]
    claims = list(thread.get("authored_claims", []))
    options = list(thread.get("next_options", []))
    for action, argument in zip(actions, arguments, strict=True):
        claims = remove_one_bounded(claims, argument)
        options = remove_one_bounded(options, str(action.get("declared_next") or ""))
    thread["authored_claims"] = claims
    thread["next_options"] = options
    if str(thread.get("response_sha256") or "") in response_hashes:
        thread["response_sha256"] = None
        thread["last_action"] = None
        thread["latest_note"] = None
    thread["revision"] = int(thread.get("revision", 0) or 0) + 1
    thread["updated_at_unix_ms"] = now_ms
    thread["event"] = "operator_reconciled_interrupted_response_non_authored"
    thread["reconciled_interrupted_response_hashes"] = sorted(response_hashes)
    atomic_json(path, thread)
    append_owner_jsonl(workspace / "autonomous/thread_state.jsonl", thread)
    return True


def reconcile_autonomy_state(workspace: Path, actions: list[dict[str, Any]]) -> bool:
    path = workspace / "autonomous/state.json"
    state = read_json(path)
    if not state:
        return False
    active_chain = str(state.get("active_chain_id") or "")
    decrement = sum(
        1
        for action in actions
        if isinstance(action.get("trace"), dict)
        and str(action["trace"].get("chain_id") or "") == active_chain
    )
    if decrement:
        state["active_chain_step"] = max(
            0, int(state.get("active_chain_step", 0) or 0) - decrement
        )
    atomic_json(path, state)
    return True


def apply(workspace: Path, operator_root: Path) -> dict[str, Any]:
    actions = interrupted_actions(workspace)
    now_ms = time.time_ns() // 1_000_000
    result: dict[str, Any] = {
        "schema": SCHEMA,
        "recorded_at_unix_ms": now_ms,
        "detected": len(actions),
        "corrections": [],
        "authority": "operator_reconciliation_non_authored_no_action_authority",
    }
    if not actions:
        return result
    quarantine = operator_root / "interrupted-actions" / str(now_ms)
    quarantine.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(quarantine, 0o700)
    for action in actions:
        relative = artifact_relative(action.get("artifact_path"))
        moved_to = None
        artifact_hash = None
        if relative is not None:
            source = workspace / relative
            if source.exists() and source.is_file() and not source.is_symlink():
                destination = quarantine / relative
                destination.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
                artifact_hash = file_sha256(source)
                shutil.move(source, destination)
                os.chmod(destination, stat.S_IRUSR | stat.S_IWUSR)
                moved_to = str(destination)
        correction = {
            "schema": SCHEMA,
            "recorded_at_unix_ms": now_ms,
            "original_recorded_at_unix_ms": action.get("recorded_at_unix_ms"),
            "response_sha256": action.get("response_sha256"),
            "trace_id": trace_id(action),
            "original_status": action.get("status"),
            "corrected_status": "revoked_interrupted_trace_non_authored",
            "artifact_sha256": artifact_hash,
            "quarantined_artifact": moved_to,
            "authority": "operator_reconciliation_non_authored_no_action_authority",
        }
        append_owner_jsonl(workspace / "actions/interrupted_corrections.jsonl", correction)
        result["corrections"].append(correction)
    result["thread_reconciled"] = reconcile_thread(workspace, actions, now_ms)
    result["autonomy_state_reconciled"] = reconcile_autonomy_state(workspace, actions)
    atomic_json(quarantine / "manifest.json", result)
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path, required=True)
    parser.add_argument("--operator-root", type=Path, required=True)
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()
    if args.apply:
        result = apply(args.workspace, args.operator_root)
    else:
        actions = interrupted_actions(args.workspace)
        result = {"schema": SCHEMA, "detected": len(actions), "response_sha256": [item.get("response_sha256") for item in actions], "dry_run": True}
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
