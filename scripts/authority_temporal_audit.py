#!/usr/bin/env python3
"""Project bounded temporal-authority lifecycle receipts into Evidence Store V2."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
import time
import unittest
from collections import Counter
from pathlib import Path
from typing import Any

try:
    from authority_state import ArtifactAuthorityStateV1
    from evidence_store import EvidenceEventStore
    from evidence_store.model import ProvenanceSourceV1, canonical_json
except ModuleNotFoundError:
    from scripts.authority_state import ArtifactAuthorityStateV1
    from scripts.evidence_store import EvidenceEventStore
    from scripts.evidence_store.model import ProvenanceSourceV1, canonical_json

try:
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.projection_receipt import projector_receipt
try:
    from projection_cursors import ProjectionCursorError, ProjectionInputCursor
except ModuleNotFoundError:
    from scripts.projection_cursors import (
        ProjectionCursorError,
        ProjectionInputCursor,
    )

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_WORKSPACE = ROOT / "capsules/spectral-bridge/workspace"
LIFECYCLE_TYPES = {
    "steward_approval",
    "budget_approval",
    "research_budget_approval",
    "dispatch_reservation",
    "dispatch_outcome",
    "execution_result",
    "blocked",
}


def evidence_only_state() -> dict[str, Any]:
    return ArtifactAuthorityStateV1.evidence_only().canonical_record()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def parse_rows(
    path: Path,
    source_rows: list[tuple[int, str]],
    *,
    previous_raw_sha256: str | None = None,
) -> tuple[list[dict[str, Any]], list[str], str | None]:
    rows: list[dict[str, Any]] = []
    errors: list[str] = []
    previous_hash = previous_raw_sha256
    for line_number, raw in source_rows:
        if not raw.strip():
            continue
        try:
            row = json.loads(raw)
        except json.JSONDecodeError:
            errors.append(f"{path}:{line_number}:invalid_json")
            previous_hash = sha256_bytes(raw.encode("utf-8"))
            continue
        if not isinstance(row, dict):
            errors.append(f"{path}:{line_number}:not_object")
            previous_hash = sha256_bytes(raw.encode("utf-8"))
            continue
        if "record_sha256" in row:
            expected_previous = previous_hash or "0" * 64
            if row.get("previous_record_sha256") != expected_previous:
                errors.append(f"{path}:{line_number}:previous_hash_mismatch")
            unsigned = dict(row)
            actual = str(unsigned.pop("record_sha256", ""))
            if sha256_bytes(canonical_json(unsigned).encode("utf-8")) != actual:
                errors.append(f"{path}:{line_number}:record_hash_mismatch")
        row["_line_number"] = line_number
        row["_raw_sha256"] = sha256_bytes(raw.encode("utf-8"))
        rows.append(row)
        previous_hash = row["_raw_sha256"]
    return rows, errors, previous_hash


def read_rows(path: Path) -> tuple[list[dict[str, Any]], list[str]]:
    source_rows = list(
        enumerate(path.read_text(encoding="utf-8").splitlines(), 1)
    )
    rows, errors, _ = parse_rows(path, source_rows)
    return rows, errors


def bounded_payload(path: Path, workspace: Path, row: dict[str, Any]) -> dict[str, Any]:
    context = row.get("authority_temporal_context_v1")
    bounded_context = {}
    if isinstance(context, dict):
        bounded_context = {
            key: context.get(key)
            for key in (
                "schema",
                "schema_version",
                "scope",
                "token_id",
                "issued_at_unix_s",
                "expires_at_unix_s",
                "remaining_budget",
                "pause_generation",
                "source_identity",
                "deployment_identity",
                "process_identity",
                "lifecycle_state",
            )
        }
    return {
        "event_type": f"authority_{row.get('record_type', 'unknown')}",
        "aggregate": {
            "kind": "authority_request",
            "id": str(row.get("request_id") or row.get("budget_id") or "unknown"),
        },
        "record_type": row.get("record_type"),
        "record_schema": row.get("record_schema"),
        "record_id": row.get("record_id"),
        "request_id": row.get("request_id"),
        "budget_id": row.get("budget_id"),
        "token_id": row.get("token_id"),
        "scope": row.get("scope"),
        "reservation_id": row.get("reservation_id"),
        "dispatch_outcome": row.get("outcome"),
        "reason": row.get("reason"),
        "authority_temporal_context_v1": bounded_context or None,
        "source_receipt": {
            "relative_path": str(path.relative_to(workspace)),
            "line_number": row["_line_number"],
            "raw_sha256": row["_raw_sha256"],
        },
        "artifact_authority_state_v1": evidence_only_state(),
    }


def generate(workspace: Path, *, write: bool) -> dict[str, Any]:
    gate_paths = sorted(workspace.glob("action_threads/threads/*/authority_gate.jsonl"))
    payloads: list[dict[str, Any]] = []
    errors: list[str] = []
    counts: Counter[str] = Counter()
    active_legacy_tokens: set[str] = set()
    cursor = ProjectionInputCursor(
        workspace / "diagnostics/authority_temporal_v1/ingestion_cursor_v1.json",
        "authority_temporal",
    )
    cursor_updates: dict[str, dict[str, Any]] = {}
    prior_keys = set((cursor.value.get("jsonl") or {}).keys())
    current_keys = {
        path.relative_to(workspace).as_posix() for path in gate_paths
    }
    if write and prior_keys - current_keys:
        errors.append(
            "authority_gate_removed_after_cursor:"
            + ",".join(sorted(prior_keys - current_keys))
        )
    for path in gate_paths:
        key = path.relative_to(workspace).as_posix()
        file_counts: Counter[str] = Counter()
        file_active_legacy_tokens: set[str] = set()
        if write:
            source_rows, next_state = cursor.jsonl_tail(path, key=key)
            prior_state = cursor.jsonl_metadata(key)
            rows, row_errors, last_raw_sha256 = parse_rows(
                path,
                source_rows,
                previous_raw_sha256=(
                    str(prior_state.get("last_raw_sha256"))
                    if prior_state.get("last_raw_sha256")
                    else None
                ),
            )
            file_counts.update(prior_state.get("record_type_counts") or {})
            file_active_legacy_tokens.update(
                str(value)
                for value in (prior_state.get("active_legacy_token_hashes") or [])
            )
        else:
            rows, row_errors = read_rows(path)
            next_state = {}
            last_raw_sha256 = None
        errors.extend(row_errors)
        for row in rows:
            record_type = str(row.get("record_type") or "")
            if record_type not in LIFECYCLE_TYPES:
                continue
            file_counts[record_type] += 1
            token = str(row.get("token_id") or row.get("budget_id") or "")
            token_hash = sha256_bytes(token.encode()) if token else ""
            if record_type in {"steward_approval", "budget_approval"}:
                if (
                    row.get("status", "active") == "active"
                    and not isinstance(row.get("authority_temporal_context_v1"), dict)
                ):
                    file_active_legacy_tokens.add(token_hash)
            if (
                record_type
                in {
                    "execution_result",
                    "blocked",
                    "dispatch_reservation",
                    "dispatch_outcome",
                }
                and row.get("outcome") != "released"
            ):
                file_active_legacy_tokens.discard(token_hash)
            if isinstance(row.get("authority_temporal_context_v1"), dict) or record_type.startswith(
                "dispatch_"
            ):
                payloads.append(bounded_payload(path, workspace, row))
        if write:
            cursor_updates[key] = {
                **next_state,
                "last_raw_sha256": last_raw_sha256,
                "record_type_counts": dict(sorted(file_counts.items())),
                "active_legacy_token_hashes": sorted(
                    file_active_legacy_tokens
                ),
            }
        counts.update(file_counts)
        active_legacy_tokens.update(file_active_legacy_tokens)

    active_legacy = len(active_legacy_tokens)
    if write and active_legacy:
        errors.append(f"active_legacy_authority_records:{active_legacy}")
    appended = 0
    total_projectable = len(payloads)
    if write and not errors:
        store = EvidenceEventStore(workspace / "diagnostics/evidence_event_store_v2")
        events = store.append_payloads(
            "authority_lifecycle",
            payloads,
            actor="authority-temporal-projector",
            source=ProvenanceSourceV1("projection", "authority_gate.jsonl"),
            idempotency_keys=[
                f"authority:{payload['source_receipt']['raw_sha256']}" for payload in payloads
            ],
        )
        appended = len(events)
        historical, corrupt = store.payloads_for_stream("authority_lifecycle")
        if corrupt:
            errors.append("authority_lifecycle_stream_corrupt")
        else:
            total_projectable = len(historical)

    result = {
        "schema": "authority_temporal_projection_status_v1",
        "schema_version": 1,
        "valid": not errors,
        "write": write,
        "gate_file_count": len(gate_paths),
        "projectable_receipt_count": total_projectable,
        "delta_projectable_receipt_count": len(payloads),
        "appended_or_existing_count": appended,
        "active_legacy_authority_count": active_legacy,
        "record_type_counts": dict(sorted(counts.items())),
        "errors": errors,
        "artifact_authority_state_v1": evidence_only_state(),
    }
    if write and not errors:
        write_atomic(
            workspace / "diagnostics/authority_temporal_v1/status.json",
            canonical_json(result) + "\n",
        )
        cursor.commit_jsonl(cursor_updates)
    return result


def write_atomic(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    os.chmod(path.parent, 0o700)
    fd, temp_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temp_name, 0o600)
        os.replace(temp_name, path)
    finally:
        if os.path.exists(temp_name):
            os.unlink(temp_name)


class SelfTest(unittest.TestCase):
    def test_hash_chain_and_bounded_projection(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            workspace = Path(raw)
            gate = workspace / "action_threads/threads/th/authority_gate.jsonl"
            gate.parent.mkdir(parents=True)
            first = {"record_schema": "authority_gate_v1", "record_type": "request"}
            first_raw = canonical_json(first)
            second = {
                "record_schema": "authority_dispatch_v1",
                "record_type": "dispatch_reservation",
                "request_id": "req",
                "token_id": "token",
                "scope": "semantic_microdose",
                "reservation_id": "dispatch_test",
                "previous_record_sha256": sha256_bytes(first_raw.encode()),
            }
            second["record_sha256"] = sha256_bytes(canonical_json(second).encode())
            gate.write_text(first_raw + "\n" + canonical_json(second) + "\n", encoding="utf-8")
            result = generate(workspace, write=False)
            self.assertTrue(result["valid"], result)
            self.assertEqual(result["projectable_receipt_count"], 1)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument(
        "command",
        nargs="?",
        choices=("generate", "project", "verify"),
        default="verify",
    )
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--receipt-json", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(SelfTest)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    started = time.monotonic()
    workspace = args.workspace.resolve()
    result = generate(
        workspace,
        write=args.write and args.command in {"generate", "project"},
    )
    if args.command == "project":
        print(
            json.dumps(
                projector_receipt(
                    "authority_temporal",
                    result,
                    {
                        "status.json": (
                            workspace
                            / "diagnostics/authority_temporal_v1/status.json"
                        )
                    },
                    started_monotonic=started,
                ),
                indent=2,
                sort_keys=True,
            )
        )
        return 0 if result["valid"] else 1
    print(json.dumps(result, indent=2, sort_keys=True) if args.json else canonical_json(result))
    return 0 if result["valid"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
