#!/usr/bin/env python3
"""Remove observer-duplicated operator-harness web receipts beyond a v2 prefix."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import stat
import time

CALL_PREFIX = "edge-operator-inquiry-"
CHECKPOINT_SCHEMA = "astrid_edge_hindsight_checkpoint_v2"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def latest_checkpoint(path: pathlib.Path) -> dict[str, object]:
    records = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]
    if not records or records[-1].get("schema") != CHECKPOINT_SCHEMA:
        raise RuntimeError("latest hindsight checkpoint is not v2")
    return records[-1]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=pathlib.Path)
    parser.add_argument("--hindsight-root", required=True, type=pathlib.Path)
    args = parser.parse_args()

    checkpoint = latest_checkpoint(args.hindsight_root / "checkpoints.jsonl")
    ledger = checkpoint.get("ledgers", {}).get("web/receipts.jsonl", {})
    snapshot_size = int(ledger.get("size_bytes", -1))
    expected_hash = str(ledger.get("sha256", ""))
    expected_inode = int(ledger.get("inode", -1))
    path = args.workspace / "web/receipts.jsonl"

    with path.open("r+b", buffering=0) as file:
        metadata = os.fstat(file.fileno())
        if metadata.st_ino != expected_inode:
            raise RuntimeError("ledger inode changed after the captured checkpoint")
        data = file.read()
        prefix = data[:snapshot_size]
        tail = data[snapshot_size:]
        if snapshot_size < 0 or len(prefix) != snapshot_size or sha256(prefix) != expected_hash:
            raise RuntimeError("captured hindsight prefix no longer verifies")
        if prefix and not prefix.endswith(b"\n"):
            raise RuntimeError("captured prefix ends inside a JSONL record")

        kept: list[bytes] = []
        removed: list[bytes] = []
        for line in tail.splitlines(keepends=True):
            if not line.endswith(b"\n"):
                raise RuntimeError("uncaptured ledger tail has a partial record")
            record = json.loads(line)
            call_id = str(record.get("call_id", ""))
            if call_id.startswith(CALL_PREFIX):
                removed.append(line)
            else:
                kept.append(line)
        if not removed:
            print("removed_records=0")
            return 0

        repair_root = args.hindsight_root / "isolation-repairs"
        repair_root.mkdir(mode=0o700, parents=True, exist_ok=True)
        os.chmod(repair_root, 0o700)
        timestamp = time.time_ns() // 1_000_000
        backup = repair_root / f"operator_harness_tail_{timestamp}.jsonl"
        descriptor = os.open(backup, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, "wb") as backup_file:
            backup_file.write(b"".join(removed))
            backup_file.flush()
            os.fsync(backup_file.fileno())

        replacement_tail = b"".join(kept)
        file.seek(snapshot_size)
        file.write(replacement_tail)
        file.truncate(snapshot_size + len(replacement_tail))
        file.flush()
        os.fsync(file.fileno())
        os.fchmod(file.fileno(), stat.S_IRUSR | stat.S_IWUSR)

    receipt = {
        "schema": "astrid_edge_operator_harness_isolation_repair_v1",
        "recorded_at_unix_ms": timestamp,
        "ledger": str(path),
        "captured_prefix_size": snapshot_size,
        "captured_prefix_sha256": expected_hash,
        "inode_preserved": path.stat().st_ino == expected_inode,
        "removed_records": len(removed),
        "removed_tail_sha256": sha256(b"".join(removed)),
        "kept_uncaptured_records": len(kept),
        "backup": str(backup),
        "authority": "operator_repair_removes_harness_duplicates_not_astrid_history",
    }
    receipt_path = repair_root / f"repair_{timestamp}.json"
    descriptor = os.open(receipt_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as receipt_file:
        json.dump(receipt, receipt_file, sort_keys=True, indent=2)
        receipt_file.write("\n")
        receipt_file.flush()
        os.fsync(receipt_file.fileno())
    print(json.dumps(receipt, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
