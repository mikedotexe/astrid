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
import uuid


SCHEMA = "astrid_edge_interrupted_action_correction_v2"


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return {}
    return value if isinstance(value, dict) else {}


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_bytes().splitlines(keepends=True)
    except OSError:
        return []
    values: list[dict[str, Any]] = []
    for line in lines:
        if not line.endswith(b"\n"):
            continue
        try:
            value = json.loads(line.decode("utf-8"))
        except (UnicodeError, json.JSONDecodeError):
            continue
        if isinstance(value, dict):
            values.append(value)
    return values


def normalized_uuid(value: Any) -> str | None:
    try:
        parsed = uuid.UUID(str(value))
    except (TypeError, ValueError, AttributeError):
        return None
    return str(parsed) if parsed.int != 0 else None


def trace_id(value: dict[str, Any]) -> str:
    trace = value.get("trace")
    candidate = (
        trace.get("trace_id") if isinstance(trace, dict) else value.get("trace_id")
    )
    return normalized_uuid(candidate) or ""


def turn_id(value: dict[str, Any]) -> str:
    trace = value.get("trace")
    candidate = (
        trace.get("turn_id") if isinstance(trace, dict) else value.get("turn_id")
    )
    return normalized_uuid(candidate) or ""


def span_id(value: dict[str, Any]) -> str:
    trace = value.get("trace")
    candidate = (
        trace.get("span_id") if isinstance(trace, dict) else value.get("span_id")
    )
    return normalized_uuid(candidate) or ""


def causal_response_key(value: dict[str, Any]) -> tuple[str, str, str, str] | None:
    """Return an event identity that cannot collide on repeated response text.

    A turn UUID is preferred when present, but a trace UUID is always required.
    Older traces without a turn UUID remain exactly attributable by trace UUID.
    """
    response_hash = value.get("response_sha256")
    exact_trace_id = trace_id(value)
    if (
        not isinstance(response_hash, str)
        or len(response_hash) != 64
        or any(character not in "0123456789abcdef" for character in response_hash)
        or not exact_trace_id
    ):
        return None
    exact_turn_id = turn_id(value)
    if exact_turn_id:
        return "turn", exact_turn_id, exact_trace_id, response_hash
    return "trace", "", exact_trace_id, response_hash


def attributed_correction_key(value: dict[str, Any]) -> tuple[str, str, str, str] | None:
    # v1 records are retained for audit, but cannot revoke a particular event.
    if (
        value.get("schema") != SCHEMA
        or value.get("corrected_status")
        != "revoked_interrupted_trace_non_authored"
        or value.get("authority")
        != "operator_reconciliation_non_authored_no_action_authority"
    ):
        return None
    return causal_response_key(value)


def interrupted_action_candidates(workspace: Path) -> list[dict[str, Any]]:
    recoveries = {
        trace_id(item): int(item.get("completed_at_unix_ms", 0) or 0)
        for item in read_jsonl(workspace / "autonomous/recoveries.jsonl")
        if item.get("status") == "interrupted" and trace_id(item)
    }
    authored = {
        key
        for item in read_jsonl(workspace / "autonomous/runs.jsonl")
        if item.get("status") == "authored_completed"
        if (key := causal_response_key(item)) is not None
    }
    result = []
    seen: set[tuple[str, str, str, str]] = set()
    for action in read_jsonl(workspace / "actions/receipts.jsonl"):
        action_trace = trace_id(action)
        action_key = causal_response_key(action)
        if (
            action_key is not None
            and action_trace in recoveries
            and int(action.get("recorded_at_unix_ms", 0) or 0)
            >= recoveries[action_trace]
            and action_key not in authored
            and action_key not in seen
        ):
            result.append(action)
            seen.add(action_key)
    return result


def exact_corrections(
    workspace: Path,
) -> dict[tuple[str, str, str, str], dict[str, Any]]:
    return {
        key: item
        for item in read_jsonl(workspace / "actions/interrupted_corrections.jsonl")
        if (key := attributed_correction_key(item)) is not None
    }


def exact_correction_keys(workspace: Path) -> set[tuple[str, str, str, str]]:
    return set(exact_corrections(workspace))


def completed_reconciliation_keys(
    operator_root: Path,
) -> set[tuple[str, str, str, str]]:
    root = operator_root / "interrupted-actions"
    try:
        metadata = root.lstat()
    except FileNotFoundError:
        return set()
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise RuntimeError(f"refusing unsafe quarantine component: {root}")
    completed: set[tuple[str, str, str, str]] = set()
    for directory in root.iterdir():
        try:
            directory_metadata = directory.lstat()
        except OSError:
            continue
        if stat.S_ISLNK(directory_metadata.st_mode) or not stat.S_ISDIR(
            directory_metadata.st_mode
        ):
            continue
        manifest_path = directory / "manifest.json"
        try:
            manifest_metadata = manifest_path.lstat()
        except OSError:
            continue
        if stat.S_ISLNK(manifest_metadata.st_mode) or not stat.S_ISREG(
            manifest_metadata.st_mode
        ):
            continue
        manifest = read_json(manifest_path)
        if (
            manifest.get("schema") != SCHEMA
            or manifest.get("authority")
            != "operator_reconciliation_non_authored_no_action_authority"
        ):
            continue
        for correction in manifest.get("corrections", []):
            if not isinstance(correction, dict):
                continue
            key = attributed_correction_key(correction)
            if key is not None:
                completed.add(key)
    return completed


def interrupted_actions(workspace: Path) -> list[dict[str, Any]]:
    corrected = exact_correction_keys(workspace)
    return [
        action
        for action in interrupted_action_candidates(workspace)
        if causal_response_key(action) not in corrected
    ]


def artifact_relative(value: Any) -> Path | None:
    text = str(value or "")
    prefix = "home://edge/"
    if not text.startswith(prefix):
        return None
    pure = PurePosixPath(text[len(prefix) :])
    if pure.is_absolute() or not pure.parts or any(part in {"", ".", ".."} for part in pure.parts):
        return None
    return Path(*pure.parts)


def require_real_directory(path: Path, *, create: bool = False) -> Path:
    """Return a strict real directory root while rejecting a symlink root."""
    if create:
        path.mkdir(parents=True, exist_ok=True, mode=0o700)
    try:
        metadata = path.lstat()
    except OSError as error:
        raise RuntimeError(f"required directory is unavailable: {path}") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise RuntimeError(f"refusing non-directory or symlink root: {path}")
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise RuntimeError(f"directory root cannot be resolved strictly: {path}") from error
    if not resolved.is_dir():
        raise RuntimeError(f"resolved root is not a directory: {resolved}")
    return resolved


def require_beneath(candidate: Path, root: Path, *, description: str) -> None:
    try:
        candidate.relative_to(root)
    except ValueError as error:
        raise RuntimeError(f"{description} escapes configured root: {candidate}") from error


def owned_regular_source(workspace: Path, relative: Path) -> Path | None:
    """Resolve a regular artifact without following any owned-tree symlink."""
    workspace_root = require_real_directory(workspace)
    current = workspace_root
    for index, part in enumerate(relative.parts):
        current = current / part
        try:
            metadata = current.lstat()
        except FileNotFoundError:
            return None
        except OSError as error:
            raise RuntimeError(f"cannot inspect owned artifact component: {current}") from error
        if stat.S_ISLNK(metadata.st_mode):
            return None
        final = index == len(relative.parts) - 1
        if final:
            if not stat.S_ISREG(metadata.st_mode):
                return None
        elif not stat.S_ISDIR(metadata.st_mode):
            return None
    try:
        resolved = current.resolve(strict=True)
    except OSError as error:
        raise RuntimeError(f"owned artifact cannot be resolved strictly: {current}") from error
    require_beneath(resolved, workspace_root, description="owned artifact")
    return resolved


def ensure_private_directory_beneath(root: Path, relative: Path) -> Path:
    """Create a private directory chain without accepting symlink components."""
    current = root
    for part in relative.parts:
        current = current / part
        try:
            metadata = current.lstat()
        except FileNotFoundError:
            current.mkdir(mode=0o700)
            fsync_directory(current.parent)
            metadata = current.lstat()
        except OSError as error:
            raise RuntimeError(f"cannot inspect quarantine component: {current}") from error
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise RuntimeError(f"refusing unsafe quarantine component: {current}")
        os.chmod(current, 0o700)
        resolved = current.resolve(strict=True)
        require_beneath(resolved, root, description="quarantine directory")
        current = resolved
    return current


def prepare_quarantine(operator_root: Path, timestamp: int) -> Path:
    operator_real = require_real_directory(operator_root, create=True)
    parent = ensure_private_directory_beneath(
        operator_real, Path("interrupted-actions")
    )
    quarantine = parent / str(timestamp)
    try:
        quarantine.mkdir(mode=0o700)
    except FileExistsError as error:
        raise RuntimeError(f"refusing pre-existing quarantine directory: {quarantine}") from error
    fsync_directory(parent)
    quarantine_real = quarantine.resolve(strict=True)
    require_beneath(quarantine_real, operator_real, description="quarantine")
    return quarantine_real


def quarantine_destination(quarantine: Path, relative: Path) -> Path:
    parent = ensure_private_directory_beneath(quarantine, relative.parent)
    destination = parent / relative.name
    try:
        destination.lstat()
    except FileNotFoundError:
        pass
    except OSError as error:
        raise RuntimeError(f"cannot inspect quarantine destination: {destination}") from error
    else:
        raise RuntimeError(f"refusing existing quarantine destination: {destination}")
    require_beneath(parent.resolve(strict=True), quarantine, description="quarantine destination")
    return destination


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def fsync_directory(path: Path) -> None:
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0)
    descriptor = os.open(path, flags)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def validate_existing_regular(path: Path) -> tuple[int, int] | None:
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        return None
    if not stat.S_ISREG(metadata.st_mode):
        raise RuntimeError(f"refusing non-regular persistence target: {path}")
    return metadata.st_dev, metadata.st_ino


def ensure_open_identity(
    path: Path,
    descriptor: int,
    expected_identity: tuple[int, int] | None = None,
) -> None:
    opened = os.fstat(descriptor)
    current = path.lstat()
    if (
        not stat.S_ISREG(opened.st_mode)
        or not stat.S_ISREG(current.st_mode)
        or opened.st_dev != current.st_dev
        or opened.st_ino != current.st_ino
    ):
        raise RuntimeError(f"persistence path changed identity: {path}")
    if expected_identity is not None and (
        opened.st_dev,
        opened.st_ino,
    ) != expected_identity:
        raise RuntimeError(f"persistence target was replaced before open: {path}")


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    parent = require_real_directory(path.parent, create=True)
    path = parent / path.name
    existing_identity = validate_existing_regular(path)
    temporary = path.with_name(
        f".{path.name}.tmp-{os.getpid()}-{time.time_ns()}"
    )
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    flags |= getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(temporary, flags, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            ensure_open_identity(temporary, handle.fileno())
            handle.write(
                (json.dumps(value, indent=2, sort_keys=False, allow_nan=False) + "\n").encode()
            )
            handle.flush()
            os.fsync(handle.fileno())
        if validate_existing_regular(path) != existing_identity:
            raise RuntimeError(f"atomic persistence target changed identity: {path}")
        os.replace(temporary, path)
        fsync_directory(path.parent)
    finally:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


def append_owner_jsonl(path: Path, value: dict[str, Any]) -> None:
    parent = require_real_directory(path.parent, create=True)
    path = parent / path.name
    existing_identity = validate_existing_regular(path)
    flags = os.O_RDWR | os.O_APPEND
    if existing_identity is None:
        flags |= os.O_CREAT | os.O_EXCL
    flags |= getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags, 0o600)
    with os.fdopen(descriptor, "r+b") as handle:
        ensure_open_identity(path, handle.fileno(), existing_identity)
        os.fchmod(handle.fileno(), 0o600)
        handle.seek(0, os.SEEK_END)
        if handle.tell() > 0:
            handle.seek(-1, os.SEEK_END)
            if handle.read(1) != b"\n":
                raise RuntimeError(
                    f"refusing to append after a partial JSONL record: {path}"
                )
        handle.seek(0, os.SEEK_END)
        handle.write(
            (json.dumps(value, separators=(",", ":"), allow_nan=False) + "\n").encode()
        )
        handle.flush()
        os.fsync(handle.fileno())
    fsync_directory(path.parent)


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


def reconciliation_marker(
    key: tuple[str, str, str, str]
) -> dict[str, Any]:
    kind, identity, exact_trace_id, response_hash = key
    return {
        "identity_kind": "turn_id" if kind == "turn" else "trace_id",
        "turn_id": identity if kind == "turn" else None,
        "trace_id": exact_trace_id,
        "response_sha256": response_hash,
    }


def reconciled_response_keys(value: dict[str, Any]) -> set[tuple[str, str, str, str]]:
    records = value.get("reconciled_interrupted_responses")
    if not isinstance(records, list):
        return set()
    return {
        key
        for record in records
        if isinstance(record, dict)
        if (key := causal_response_key(record)) is not None
    }


def reconcile_thread(workspace: Path, actions: list[dict[str, Any]], now_ms: int) -> bool:
    path = workspace / "autonomous/thread_state.json"
    thread = read_json(path)
    if not thread:
        return False
    response_keys = {
        key for action in actions if (key := causal_response_key(action)) is not None
    }
    history_path = workspace / "autonomous/thread_state.jsonl"
    history_keys: set[tuple[str, str, str, str]] = set()
    for snapshot in read_jsonl(history_path):
        history_keys.update(reconciled_response_keys(snapshot))
    existing_keys = reconciled_response_keys(thread) | history_keys
    pending_actions = [
        action
        for action in actions
        if causal_response_key(action) not in existing_keys
    ]
    pending_keys = {
        key
        for action in pending_actions
        if (key := causal_response_key(action)) is not None
    }
    timestamps = {
        int(action.get("recorded_at_unix_ms", 0) or 0)
        for action in pending_actions
    }
    basenames = {
        relative.name
        for action in pending_actions
        if (relative := artifact_relative(action.get("artifact_path"))) is not None
    }
    arguments = [action_argument(action) for action in pending_actions]
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
    state_changed = bool(pending_keys)
    if state_changed:
        thread["evidence_records"] = retained_evidence
        thread["provenance_hashes"] = [
            value
            for value in thread.get("provenance_hashes", [])
            if str(value) not in removed_hashes
        ]
        claims = list(thread.get("authored_claims", []))
        options = list(thread.get("next_options", []))
        for action, argument in zip(pending_actions, arguments, strict=True):
            claims = remove_one_bounded(claims, argument)
            options = remove_one_bounded(
                options, str(action.get("declared_next") or "")
            )
        thread["authored_claims"] = claims
        thread["next_options"] = options
        if causal_response_key(thread) in pending_keys:
            thread["response_sha256"] = None
            thread["last_action"] = None
            thread["latest_note"] = None
        thread["revision"] = int(thread.get("revision", 0) or 0) + 1
        thread["updated_at_unix_ms"] = now_ms
        thread["event"] = "operator_reconciled_interrupted_response_non_authored"
        markers = [
            marker
            for marker in thread.get("reconciled_interrupted_responses", [])
            if isinstance(marker, dict)
        ]
        markers.extend(reconciliation_marker(key) for key in sorted(pending_keys))
        thread["reconciled_interrupted_responses"] = markers
        atomic_json(path, thread)

    history_repaired = not response_keys.issubset(history_keys)
    if history_repaired:
        append_owner_jsonl(history_path, thread)
    return state_changed or history_repaired


def reconcile_autonomy_state(workspace: Path, actions: list[dict[str, Any]]) -> bool:
    path = workspace / "autonomous/state.json"
    state = read_json(path)
    if not state:
        return False
    existing_keys = reconciled_response_keys(state)
    pending_actions = [
        action
        for action in actions
        if causal_response_key(action) not in existing_keys
    ]
    pending_keys = {
        key
        for action in pending_actions
        if (key := causal_response_key(action)) is not None
    }
    if not pending_keys:
        return False
    active_chain = str(state.get("active_chain_id") or "")
    decrement = sum(
        1
        for action in pending_actions
        if isinstance(action.get("trace"), dict)
        and str(action["trace"].get("chain_id") or "") == active_chain
    )
    if decrement:
        state["active_chain_step"] = max(
            0, int(state.get("active_chain_step", 0) or 0) - decrement
        )
    markers = [
        marker
        for marker in state.get("reconciled_interrupted_responses", [])
        if isinstance(marker, dict)
    ]
    markers.extend(reconciliation_marker(key) for key in sorted(pending_keys))
    state["reconciled_interrupted_responses"] = markers
    atomic_json(path, state)
    return True


def apply(workspace: Path, operator_root: Path) -> dict[str, Any]:
    candidates = interrupted_action_candidates(workspace)
    corrections_by_key = exact_corrections(workspace)
    corrected_keys = set(corrections_by_key)
    completed_keys = completed_reconciliation_keys(operator_root)
    actions = [
        action
        for action in candidates
        if causal_response_key(action) not in corrected_keys
    ]
    reconciliation_candidates = [
        action
        for action in candidates
        if causal_response_key(action) not in completed_keys
    ]
    now_ms = time.time_ns() // 1_000_000
    result: dict[str, Any] = {
        "schema": SCHEMA,
        "recorded_at_unix_ms": now_ms,
        "detected": len(actions),
        "existing_exact_corrections": len(candidates) - len(actions),
        "pending_reconciliation": len(reconciliation_candidates),
        "corrections": [],
        "authority": "operator_reconciliation_non_authored_no_action_authority",
    }
    if not actions and not reconciliation_candidates:
        return result
    quarantine: Path | None = None
    if actions:
        quarantine = prepare_quarantine(operator_root, now_ms)
        for action in actions:
            relative = artifact_relative(action.get("artifact_path"))
            moved_to = None
            artifact_hash = None
            if relative is not None:
                source = owned_regular_source(workspace, relative)
                if source is not None:
                    destination = quarantine_destination(quarantine, relative)
                    artifact_hash = file_sha256(source)
                    shutil.move(source, destination)
                    moved_metadata = destination.lstat()
                    if not stat.S_ISREG(moved_metadata.st_mode):
                        raise RuntimeError(
                            f"quarantined artifact is not regular: {destination}"
                        )
                    require_beneath(
                        destination.resolve(strict=True),
                        quarantine,
                        description="quarantined artifact",
                    )
                    os.chmod(destination, stat.S_IRUSR | stat.S_IWUSR)
                    descriptor = os.open(
                        destination,
                        os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0),
                    )
                    try:
                        ensure_open_identity(destination, descriptor)
                        os.fsync(descriptor)
                    finally:
                        os.close(descriptor)
                    fsync_directory(destination.parent)
                    fsync_directory(source.parent)
                    moved_to = str(destination)
            correction = {
                "schema": SCHEMA,
                "recorded_at_unix_ms": now_ms,
                "original_recorded_at_unix_ms": action.get("recorded_at_unix_ms"),
                "response_sha256": action.get("response_sha256"),
                "trace_id": trace_id(action),
                "turn_id": turn_id(action) or None,
                "action_span_id": span_id(action) or None,
                "identity_kind": "turn_id" if turn_id(action) else "trace_id",
                "original_status": action.get("status"),
                "corrected_status": "revoked_interrupted_trace_non_authored",
                "artifact_sha256": artifact_hash,
                "quarantined_artifact": moved_to,
                "authority": "operator_reconciliation_non_authored_no_action_authority",
            }
            append_owner_jsonl(
                workspace / "actions/interrupted_corrections.jsonl", correction
            )
            result["corrections"].append(correction)
    result["thread_reconciled"] = reconcile_thread(
        workspace, reconciliation_candidates, now_ms
    )
    result["autonomy_state_reconciled"] = reconcile_autonomy_state(
        workspace, reconciliation_candidates
    )
    result["reconciled_response_count"] = len(reconciliation_candidates)
    result["pending_reconciliation"] = 0
    result["reconciliation_complete"] = True
    if actions:
        assert quarantine is not None
        atomic_json(quarantine / "manifest.json", result)
    recovered_by_timestamp: dict[int, list[dict[str, Any]]] = {}
    for action in reconciliation_candidates:
        key = causal_response_key(action)
        correction = corrections_by_key.get(key) if key is not None else None
        if correction is None:
            continue
        timestamp = int(correction.get("recorded_at_unix_ms", 0) or 0)
        if timestamp <= 0:
            raise RuntimeError("exact correction lacks its quarantine timestamp")
        recovered_by_timestamp.setdefault(timestamp, []).append(correction)
    for timestamp, corrections in recovered_by_timestamp.items():
        recovery_directory = require_real_directory(
            operator_root / "interrupted-actions" / str(timestamp)
        )
        manifest_path = recovery_directory / "manifest.json"
        prior_manifest = read_json(manifest_path)
        merged: dict[tuple[str, str, str, str], dict[str, Any]] = {}
        for correction in prior_manifest.get("corrections", []):
            if isinstance(correction, dict):
                key = attributed_correction_key(correction)
                if key is not None:
                    merged[key] = correction
        for correction in corrections:
            key = attributed_correction_key(correction)
            if key is not None:
                merged[key] = correction
        atomic_json(
            manifest_path,
            {
                **result,
                "recorded_at_unix_ms": timestamp,
                "recovered_at_unix_ms": now_ms,
                "detected": 0,
                "corrections": list(merged.values()),
                "recovered_existing_correction": True,
            },
        )
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
        candidates = interrupted_action_candidates(args.workspace)
        corrected = exact_correction_keys(args.workspace)
        completed = completed_reconciliation_keys(args.operator_root)
        actions = interrupted_actions(args.workspace)
        result = {
            "schema": SCHEMA,
            "detected": len(actions),
            "existing_exact_corrections": sum(
                1
                for item in candidates
                if causal_response_key(item) in corrected
            ),
            "pending_reconciliation": sum(
                1
                for item in candidates
                if causal_response_key(item) not in completed
            ),
            "responses": [
                {
                    "turn_id": turn_id(item) or None,
                    "trace_id": trace_id(item),
                    "response_sha256": item.get("response_sha256"),
                }
                for item in actions
            ],
            "dry_run": True,
        }
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
