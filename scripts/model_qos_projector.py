#!/usr/bin/env python3
"""Project bounded model scheduling receipts into Evidence Event Store V2."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
import time
import unittest
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
    from projection_cursors import ProjectionInputCursor
except ModuleNotFoundError:
    from scripts.projection_cursors import ProjectionInputCursor

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_WORKSPACE = ROOT / "capsules/spectral-bridge/workspace"
DEFAULT_RECEIPTS = (
    ROOT.parent / "neural-triple-reservoir/workspace/model_qos_receipts.jsonl"
)
ALLOWED_LIFECYCLES = {
    "queued",
    "coalesced",
    "selected",
    "completed",
    "queue_timeout",
    "disconnected_queued",
    "disconnected_active",
    "stopped_pending",
}
FORBIDDEN_KEYS = {"messages", "prompt", "prompts", "response", "responses", "content"}


def authority_state() -> dict[str, Any]:
    return ArtifactAuthorityStateV1.evidence_only().canonical_record()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def valid_sha256(value: Any, *, optional: bool = False) -> bool:
    if optional and value is None:
        return True
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value)
    )


def receipt_valid(value: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    forbidden = FORBIDDEN_KEYS.intersection(value)
    if forbidden:
        errors.append(f"private_fields_present:{','.join(sorted(forbidden))}")
    if value.get("schema_version") != 1 or value.get("stream") != "model_qos":
        errors.append("schema_or_stream_mismatch")
    if value.get("lifecycle") not in ALLOWED_LIFECYCLES:
        errors.append("unsupported_lifecycle")
    if value.get("mode") not in {"shadow", "active"}:
        errors.append("unsupported_mode")
    if value.get("class") not in {
        "interactive",
        "reflective",
        "background",
        "normal",
    }:
        errors.append("unsupported_class")
    if not valid_sha256(value.get("request_id_hash"), optional=True):
        errors.append("request_id_hash_invalid")
    if not valid_sha256(value.get("idempotency_key_hash"), optional=True):
        errors.append("idempotency_key_hash_invalid")
    if isinstance(value.get("arrival_sequence"), bool) or not isinstance(
        value.get("arrival_sequence"), int
    ):
        errors.append("arrival_sequence_invalid")

    unsigned = dict(value)
    receipt_id = str(unsigned.pop("receipt_id", ""))
    expected_id = "model-qos-" + sha256_bytes(canonical_json(unsigned).encode("ascii"))
    if receipt_id != expected_id:
        errors.append("receipt_id_mismatch")
    return errors


def bounded_payload(receipt: dict[str, Any], source_hash: str) -> dict[str, Any]:
    receipt_id = str(receipt["receipt_id"])
    return {
        "event_type": f"model_qos_{receipt['lifecycle']}",
        "aggregate": {
            "kind": "model_qos_request",
            "id": str(receipt.get("request_id_hash") or receipt_id),
        },
        "receipt_id": receipt_id,
        "recorded_at": receipt.get("recorded_at"),
        "lifecycle": receipt.get("lifecycle"),
        "mode": receipt.get("mode"),
        "class": receipt.get("class"),
        "versioned": receipt.get("versioned"),
        "arrival_sequence": receipt.get("arrival_sequence"),
        "request_id_hash": receipt.get("request_id_hash"),
        "idempotency_key_hash": receipt.get("idempotency_key_hash"),
        "queue_wait_ms": receipt.get("queue_wait_ms"),
        "actual_arrival_sequence": receipt.get("actual_arrival_sequence"),
        "hypothetical_arrival_sequence": receipt.get(
            "hypothetical_arrival_sequence"
        ),
        "would_reorder": receipt.get("would_reorder"),
        "parity_mismatch": receipt.get("parity_mismatch"),
        "outcome": receipt.get("outcome"),
        "coalesced_waiters": receipt.get("coalesced_waiters"),
        "source_receipt_sha256": source_hash,
        "raw_prompt_or_response_included": False,
        "artifact_authority_state_v1": authority_state(),
    }


def generate(workspace: Path, receipts_path: Path, *, write: bool) -> dict[str, Any]:
    cursor = ProjectionInputCursor(
        workspace / "diagnostics/model_qos_v1/ingestion_cursor_v1.json",
        "model_qos",
    )
    cursor_key = receipts_path.name
    if write:
        source_rows, next_cursor = cursor.jsonl_tail(
            receipts_path,
            key=cursor_key,
        )
        source_hash = str(next_cursor["source_sha256"])
    else:
        source_bytes = receipts_path.read_bytes() if receipts_path.is_file() else b""
        source_hash = sha256_bytes(source_bytes)
        source_rows = list(
            enumerate(source_bytes.decode("utf-8").splitlines(), 1)
        )
        next_cursor = {}
    payloads: list[dict[str, Any]] = []
    errors: list[str] = []
    seen: set[str] = set()
    for line_number, raw in source_rows:
        if not raw.strip():
            continue
        try:
            value = json.loads(raw)
        except json.JSONDecodeError:
            errors.append(f"line_{line_number}:invalid_json")
            continue
        if not isinstance(value, dict):
            errors.append(f"line_{line_number}:not_object")
            continue
        line_errors = receipt_valid(value)
        errors.extend(f"line_{line_number}:{error}" for error in line_errors)
        receipt_id = str(value.get("receipt_id") or "")
        if receipt_id in seen:
            errors.append(f"line_{line_number}:duplicate_receipt_id")
        seen.add(receipt_id)
        if not line_errors:
            payloads.append(bounded_payload(value, source_hash))

    appended = 0
    store = EvidenceEventStore(
        workspace / "diagnostics/evidence_event_store_v2"
    )
    if write and not errors:
        events = store.append_payloads(
            "model_qos",
            payloads,
            actor="model-qos-projector",
            source=ProvenanceSourceV1(
                "projection",
                "model_qos_receipts.jsonl",
            ),
            idempotency_keys=[
                f"model-qos:{payload['receipt_id']}" for payload in payloads
            ],
        )
        appended = len(events)

    total_receipt_ids = set(seen)
    if write and not errors:
        historical, corrupt = store.payloads_for_stream("model_qos")
        if corrupt:
            errors.append("model_qos_stream_corrupt")
        else:
            total_receipt_ids = {
                str(payload.get("receipt_id") or "")
                for payload in historical
                if payload.get("receipt_id")
            }
    checks = {
        "all_lines_valid": not errors,
        "all_valid_receipts_projectable": len(payloads) == len(seen),
        "private_content_absent": not any(
            "private_fields_present" in error for error in errors
        ),
    }
    result = {
        "schema": "model_qos_projection_status_v1",
        "schema_version": 1,
        "valid": all(checks.values()),
        "write": write,
        "source_present": receipts_path.is_file(),
        "source_locator": receipts_path.name,
        "source_sha256": source_hash,
        "receipt_count": len(total_receipt_ids),
        "projectable_receipt_count": len(total_receipt_ids),
        "delta_receipt_count": len(payloads),
        "appended_or_existing_count": appended,
        "counter_audit": {
            "status": "consistent" if all(checks.values()) else "inconsistent",
            "checks": checks,
        },
        "errors": errors,
        "artifact_authority_state_v1": authority_state(),
    }
    if write and result["valid"]:
        write_atomic(
            workspace / "diagnostics/model_qos_v1/status.json",
            canonical_json(result) + "\n",
        )
        cursor.commit_jsonl({cursor_key: next_cursor})
    return result


def write_atomic(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    os.chmod(path.parent, 0o700)
    descriptor, temp_name = tempfile.mkstemp(
        prefix=f".{path.name}.",
        dir=path.parent,
    )
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temp_name, 0o600)
        os.replace(temp_name, path)
    finally:
        if os.path.exists(temp_name):
            os.unlink(temp_name)


class SelfTest(unittest.TestCase):
    def test_valid_receipt_is_bounded_and_tampering_fails(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            workspace = root / "workspace"
            receipts = root / "receipts.jsonl"
            value = {
                "schema_version": 1,
                "stream": "model_qos",
                "recorded_at": "2026-01-01T00:00:00+00:00",
                "lifecycle": "selected",
                "mode": "shadow",
                "arrival_sequence": 1,
                "request_id_hash": "a" * 64,
                "idempotency_key_hash": "b" * 64,
                "class": "interactive",
                "versioned": True,
                "parity_mismatch": False,
            }
            value["receipt_id"] = "model-qos-" + sha256_bytes(
                canonical_json(value).encode("ascii")
            )
            receipts.write_text(canonical_json(value) + "\n", encoding="utf-8")
            result = generate(workspace, receipts, write=False)
            self.assertTrue(result["valid"], result)
            self.assertEqual(result["projectable_receipt_count"], 1)

            value["class"] = "background"
            receipts.write_text(canonical_json(value) + "\n", encoding="utf-8")
            result = generate(workspace, receipts, write=False)
            self.assertFalse(result["valid"])
            self.assertIn("line_1:receipt_id_mismatch", result["errors"])

    def test_private_content_is_rejected(self) -> None:
        value = {
            "schema_version": 1,
            "stream": "model_qos",
            "lifecycle": "queued",
            "mode": "shadow",
            "class": "normal",
            "arrival_sequence": 1,
            "request_id_hash": None,
            "idempotency_key_hash": None,
            "content": "must not enter evidence",
        }
        value["receipt_id"] = "model-qos-" + sha256_bytes(
            canonical_json(value).encode("ascii")
        )
        self.assertIn("private_fields_present:content", receipt_valid(value))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument(
        "--receipts",
        type=Path,
        default=Path(
            os.environ.get("ASTRID_MODEL_QOS_RECEIPTS", str(DEFAULT_RECEIPTS))
        ),
    )
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
        args.receipts.expanduser().resolve(),
        write=args.write and args.command in {"generate", "project"},
    )
    if args.command == "project":
        print(
            json.dumps(
                projector_receipt(
                    "model_qos",
                    result,
                    {
                        "status.json": (
                            workspace / "diagnostics/model_qos_v1/status.json"
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
