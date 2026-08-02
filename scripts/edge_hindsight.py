#!/usr/bin/env python3
"""Durable, owner-only hindsight indexing and reporting for an edge Astrid.

The ``record`` mode is a low-frequency observer.  It indexes owned artifacts,
rolls the high-rate reservoir stream into 15-minute summaries, and records
hash-chained checkpoints of the append-only ledgers and database stores.  Its
state lives outside the edge workspace so it cannot become model continuity or
reservoir experience.

The default ``report`` mode is read-only.  It joins the existing causal
activity ledgers, artifact versions, numerical telemetry, and database health
without claiming timestamp-only attribution.

Collection, exact-prefix integrity, SQLite projection, and read-only rendering
remain in one dependency-free executable so an appliance deploys and audits one
versioned hindsight contract.  A later split should occur at the database view
API after its checkpoint transaction and authority labels are independently
versioned.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import importlib.machinery
import importlib.util
import json
import math
import os
import sqlite3
import stat
import sys
import time
from pathlib import Path
from typing import Any, Iterable, Iterator

LEGACY_CHECKPOINT_SCHEMA = "astrid_edge_hindsight_checkpoint_v1"
CHECKPOINT_SCHEMA = "astrid_edge_hindsight_checkpoint_v2"
ARTIFACT_SCHEMA = "astrid_edge_hindsight_artifact_version_v1"
FILL_SCHEMA = "astrid_edge_hindsight_fill_rollup_v1"
REPORT_SCHEMA = "astrid_edge_hindsight_report_v1"
LEGACY_STATE_SCHEMA = "astrid_edge_hindsight_collector_state_v1"
STATE_SCHEMA = "astrid_edge_hindsight_collector_state_v2"
DATABASE_SCHEMA_VERSION = 3
ARTIFACT_ATTRIBUTION_VERSION = 3
LEDGER_HASH_SCOPE = "exact_open_file_prefix_v1"
DEFAULT_BUCKET_MINUTES = 15
MAX_EXCERPT_CHARS = 480

ACTIVITY_LEDGERS = (
    "actions/receipts.jsonl",
    "actions/interrupted_corrections.jsonl",
    "autonomous/runs.jsonl",
    "autonomous/chains.jsonl",
    "autonomous/recoveries.jsonl",
    "autonomous/authorship_corrections.jsonl",
    "autonomous/thread_state.jsonl",
    "web/receipts.jsonl",
    "introspection/receipts.jsonl",
    "perception/observations.jsonl",
    "studies/receipts.jsonl",
    "spectral/rollups.jsonl",
    "spectral/receipts.jsonl",
    "tuning/receipts.jsonl",
    "research/duplication_notices.jsonl",
    "peer/receipts.jsonl",
    "runtime/fill_history.jsonl",
)

ARTIFACT_ROOTS = (
    "aspirations",
    "autonomous/recoveries",
    "autonomous/turns",
    "daydreams",
    "introspections",
    "journal",
    "measurements",
    "memories",
    "notices",
    "perception/observations",
    "plans",
    "proposals",
    "research",
    "studies/definitions",
    "studies/results",
    "tuning/evidence",
    "self",
    "peer/outbox",
    "peer/read",
    "workshop/checks",
    "workshop/drafts",
    "workshop/revisions",
)

TIMESTAMP_FIELDS = (
    "recorded_at_unix_ms",
    "completed_at_unix_ms",
    "requested_at_unix_ms",
    "started_at_unix_ms",
    "updated_at_unix_ms",
)


def spectral_metric(value: dict[str, Any], name: str) -> Any:
    metrics = value.get("metrics")
    if isinstance(metrics, dict) and name in metrics:
        return metrics.get(name)
    return value.get(name)


def flatten_signed_tuning(value: dict[str, Any]) -> dict[str, Any]:
    payload = value.get("payload")
    if not isinstance(payload, dict):
        return value
    flattened = dict(payload)
    detail = payload.get("detail")
    if isinstance(detail, dict):
        flattened.update(detail)
    flattened["signed_envelope"] = True
    flattened["payload_sha256"] = value.get("payload_sha256")
    flattened["signing_public_key"] = value.get("signing_public_key")
    flattened["signature"] = value.get("signature")
    expected_hash = value.get("payload_sha256")
    flattened["payload_hash_valid"] = expected_hash == hashlib.sha256(
        json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode()
    ).hexdigest()
    flattened["signature_present_not_verified"] = bool(
        value.get("signature") and value.get("signing_public_key")
    )
    return flattened


def now_ms() -> int:
    return time.time_ns() // 1_000_000


def canonical_bytes(value: dict[str, Any]) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def digest_value(value: dict[str, Any]) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def sha256_file(path: Path, maximum_bytes: int | None = None) -> str:
    digest = hashlib.sha256()
    remaining = maximum_bytes
    with path.open("rb") as handle:
        while remaining is None or remaining > 0:
            amount = 1024 * 1024 if remaining is None else min(1024 * 1024, remaining)
            chunk = handle.read(amount)
            if not chunk:
                break
            digest.update(chunk)
            if remaining is not None:
                remaining -= len(chunk)
    return digest.hexdigest()


def sha256_open_prefix(handle: Any, prefix_bytes: int) -> tuple[str, int]:
    """Hash at most ``prefix_bytes`` from an already-open descriptor."""
    digest = hashlib.sha256()
    remaining = max(0, prefix_bytes)
    consumed = 0
    handle.seek(0)
    while remaining > 0:
        chunk = handle.read(min(1024 * 1024, remaining))
        if not chunk:
            break
        digest.update(chunk)
        consumed += len(chunk)
        remaining -= len(chunk)
    return digest.hexdigest(), consumed


def owner_directory(path: Path) -> None:
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    path.chmod(0o700)


def owner_write_json(path: Path, value: dict[str, Any]) -> None:
    owner_directory(path.parent)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    descriptor = os.open(temporary, flags, 0o600)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(value, handle, sort_keys=True, indent=2)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        path.chmod(0o600)
    finally:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


def append_chained(
    path: Path,
    values: Iterable[dict[str, Any]],
    previous_hash: str | None,
) -> tuple[str | None, int]:
    owner_directory(path.parent)
    count = 0
    flags = os.O_WRONLY | os.O_CREAT | os.O_APPEND
    descriptor = os.open(path, flags, 0o600)
    with os.fdopen(descriptor, "a", encoding="utf-8") as handle:
        for source in values:
            record = {**source, "previous_record_sha256": previous_hash}
            record["record_sha256"] = digest_value(record)
            handle.write(json.dumps(record, sort_keys=True, separators=(",", ":")))
            handle.write("\n")
            previous_hash = str(record["record_sha256"])
            count += 1
        handle.flush()
        os.fsync(handle.fileno())
    path.chmod(0o600)
    return previous_hash, count


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}
    return value if isinstance(value, dict) else {}


def json_lines(path: Path) -> Iterator[dict[str, Any]]:
    try:
        handle = path.open("r", encoding="utf-8", errors="replace")
    except OSError:
        return
    with handle:
        for line in handle:
            try:
                value = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(value, dict):
                yield value


def normalize_relative_path(value: Any) -> str | None:
    if not isinstance(value, str) or not value or "\x00" in value:
        return None
    if value.startswith("home://edge/"):
        value = value.removeprefix("home://edge/")
    candidate = Path(value)
    if candidate.is_absolute() or any(part in {"", ".", ".."} for part in candidate.parts):
        return None
    return candidate.as_posix()


def trace_summary(value: dict[str, Any]) -> dict[str, Any] | None:
    trace = value.get("trace")
    if not isinstance(trace, dict):
        return None
    return {
        key: trace.get(key)
        for key in ("schema_version", "trace_id", "span_id", "parent_span_id", "session_id", "chain_id")
        if trace.get(key) is not None
    }


def action_authority(declared_next: Any, decision_source: Any) -> tuple[str, bool]:
    declaration = str(declared_next or "").strip()
    verb = declaration.split(maxsplit=1)[0].upper() if declaration else ""
    authored_source = str(decision_source) in {
        "astrid_declared",
        "local_format_repair_preserved_astrid_declaration",
    }
    authored_verbs = {
        "ASPIRE",
        "DAYDREAM",
        "DRAFT",
        "JOURNAL",
        "NOTICE",
        "PLAN",
        "PROPOSE",
        "REMEMBER",
        "RESEARCH",
        "REVISE",
        "SELF_STUDY",
        "SYNTHESIZE",
    }
    evidence_verbs = {
        "ADOPT_TUNING",
        "CANCEL_STUDY",
        "CANCEL_TUNING",
        "CHECK",
        "MEASURE",
        "READ",
        "READ_SOURCE",
        "SHARE",
        "STUDY",
        "TUNE_RESERVOIR",
        "VALIDATE_TUNING",
        "REVERT_TUNING",
    }
    if authored_source and verb in authored_verbs:
        return "astrid_authored_bounded_artifact", True
    if authored_source and verb in evidence_verbs:
        return "executor_generated_evidence_from_astrid_action", False
    if str(decision_source) == "local_safe_fallback":
        return "executor_fallback_not_astrid_authorship", False
    return "executor_action_outcome_unclassified_authorship", False


def attribution_index(workspace: Path) -> dict[str, dict[str, Any]]:
    index: dict[str, dict[str, Any]] = {}
    interrupted_action_corrections = {
        str(item.get("response_sha256") or ""): item
        for item in json_lines(workspace / "actions/interrupted_corrections.jsonl")
        if item.get("corrected_status") == "revoked_interrupted_trace_non_authored"
    }
    for run in json_lines(workspace / "autonomous/runs.jsonl"):
        status = str(run.get("status", ""))
        authored = status == "authored_completed"
        for field in ("transcript_path", "journal_path"):
            relative = normalize_relative_path(run.get(field))
            if relative is None:
                continue
            authority = (
                "astrid_authored_turn_record"
                if authored
                else "executor_transport_record_not_astrid_authorship"
            )
            index[relative] = {
                "causal_attribution": "exact_run_path_join",
                "authority": authority,
                "astrid_authored": authored,
                "causal_timestamp_unix_ms": int(
                    run.get("completed_at_unix_ms", run.get("started_at_unix_ms", 0)) or 0
                ),
                "response_sha256": run.get("response_sha256"),
                "session_id": run.get("session_name"),
                "trace": trace_summary(run),
            }
    for action in json_lines(workspace / "actions/receipts.jsonl"):
        relative = normalize_relative_path(action.get("artifact_path"))
        if relative is None:
            continue
        correction = interrupted_action_corrections.get(
            str(action.get("response_sha256") or "")
        )
        if correction:
            authority, authored = "revoked_interrupted_trace_non_authored", False
        else:
            authority, authored = action_authority(
                action.get("declared_next"), action.get("decision_source")
            )
        index[relative] = {
            "causal_attribution": (
                "exact_action_correction_join"
                if correction
                else "exact_action_path_join"
            ),
            "authority": authority,
            "astrid_authored": authored,
            "causal_timestamp_unix_ms": int(action.get("recorded_at_unix_ms", 0) or 0),
            "response_sha256": action.get("response_sha256"),
            "session_id": action.get("session_id"),
            "chain_id": action.get("chain_id"),
            "declared_next": action.get("declared_next"),
            "decision_source": action.get("decision_source"),
            "trace": trace_summary(action),
        }
    for observation in json_lines(workspace / "perception/observations.jsonl"):
        timestamp = int(observation.get("recorded_at_unix_ms", 0) or 0)
        if timestamp <= 0:
            continue
        relative = f"perception/observations/observation_{timestamp}.md"
        index[relative] = {
            "causal_attribution": "exact_machine_observation_timestamp_join",
            "authority": "deterministic_machine_observation_not_astrid_authorship",
            "astrid_authored": False,
            "causal_timestamp_unix_ms": timestamp,
            "source_record_sha256": observation.get("record_sha256"),
            "trace": None,
        }
    return index


def artifact_files(workspace: Path) -> Iterator[tuple[str, Path, os.stat_result]]:
    resolved_workspace = workspace.resolve()
    for root_name in ARTIFACT_ROOTS:
        root = workspace / root_name
        if not root.is_dir() or root.is_symlink():
            continue
        for directory, names, files in os.walk(root, followlinks=False):
            names[:] = [name for name in names if not name.startswith(".") and not (Path(directory) / name).is_symlink()]
            for name in sorted(files):
                if name.startswith("."):
                    continue
                path = Path(directory) / name
                try:
                    metadata = path.lstat()
                    resolved = path.resolve(strict=True)
                except OSError:
                    continue
                if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
                    continue
                try:
                    relative = resolved.relative_to(resolved_workspace).as_posix()
                except ValueError:
                    continue
                yield relative, path, metadata


def scan_artifacts(
    workspace: Path,
    state: dict[str, Any],
    observed_at: int,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    prior = state.get("artifact_inventory")
    prior = prior if isinstance(prior, dict) else {}
    force_attribution_refresh = (
        state.get("artifact_attribution_version") != ARTIFACT_ATTRIBUTION_VERSION
    )
    current: dict[str, Any] = {}
    records: list[dict[str, Any]] = []
    attribution = attribution_index(workspace)
    initial_inventory = not bool(prior)
    for relative, path, metadata in artifact_files(workspace):
        fingerprint = f"{metadata.st_size}:{metadata.st_mtime_ns}"
        previous = prior.get(relative)
        if (
            not force_attribution_refresh
            and isinstance(previous, dict)
            and previous.get("fingerprint") == fingerprint
        ):
            current[relative] = previous
            continue
        content_sha256 = sha256_file(path)
        current[relative] = {
            "fingerprint": fingerprint,
            "sha256": content_sha256,
        }
        exact = attribution.get(relative)
        if exact is None:
            exact = {
                "causal_attribution": "legacy_filesystem_discovery",
                "authority": "historical_owned_file_unclassified_authorship",
                "astrid_authored": False,
                "causal_timestamp_unix_ms": None,
                "trace": None,
            }
        records.append(
            {
                "schema": ARTIFACT_SCHEMA,
                "observed_at_unix_ms": observed_at,
                "file_mtime_unix_ms": metadata.st_mtime_ns // 1_000_000,
                "relative_path": relative,
                "size_bytes": metadata.st_size,
                "mode": f"{stat.S_IMODE(metadata.st_mode):04o}",
                "content_sha256": content_sha256,
                "initial_inventory": initial_inventory,
                **exact,
            }
        )
    removed = sorted(set(prior) - set(current))
    for relative in removed:
        previous = prior.get(relative)
        records.append(
            {
                "schema": ARTIFACT_SCHEMA,
                "observed_at_unix_ms": observed_at,
                "file_mtime_unix_ms": None,
                "relative_path": relative,
                "size_bytes": None,
                "mode": None,
                "content_sha256": previous.get("sha256") if isinstance(previous, dict) else None,
                "initial_inventory": False,
                "causal_attribution": "filesystem_removal_observed_no_causal_claim",
                "authority": "operator_observed_file_removal",
                "astrid_authored": False,
                "causal_timestamp_unix_ms": None,
                "trace": None,
            }
        )
    return sorted(records, key=lambda item: str(item["relative_path"])), current


def empty_fill_bucket(start_ms: int) -> dict[str, Any]:
    return {
        "bucket_start_unix_ms": start_ms,
        "sample_count": 0,
        "fill_sum": 0.0,
        "fill_min": None,
        "fill_max": None,
        "in_65_72": 0,
        "in_65_73_5": 0,
        "semantic_fresh": 0,
        "audio_fresh": 0,
        "aux_fresh": 0,
        "first_sample_unix_ms": None,
        "last_sample_unix_ms": None,
    }


def add_fill_sample(bucket: dict[str, Any], value: dict[str, Any]) -> None:
    try:
        fill = float(value["fill_pct"])
        timestamp = int(value["recorded_at_unix_ms"])
    except (KeyError, TypeError, ValueError):
        return
    if not math.isfinite(fill) or timestamp <= 0:
        return
    count = int(bucket["sample_count"])
    bucket["sample_count"] = count + 1
    bucket["fill_sum"] = float(bucket["fill_sum"]) + fill
    bucket["fill_min"] = fill if bucket["fill_min"] is None else min(float(bucket["fill_min"]), fill)
    bucket["fill_max"] = fill if bucket["fill_max"] is None else max(float(bucket["fill_max"]), fill)
    bucket["in_65_72"] = int(bucket["in_65_72"]) + int(65.0 <= fill <= 72.0)
    bucket["in_65_73_5"] = int(bucket["in_65_73_5"]) + int(65.0 <= fill <= 73.5)
    for field in ("semantic_fresh", "audio_fresh", "aux_fresh"):
        bucket[field] = int(bucket[field]) + int(value.get(field) is True)
    if bucket["first_sample_unix_ms"] is None:
        bucket["first_sample_unix_ms"] = timestamp
    bucket["last_sample_unix_ms"] = timestamp


def final_fill_bucket(bucket: dict[str, Any], bucket_ms: int) -> dict[str, Any] | None:
    count = int(bucket.get("sample_count", 0))
    if count <= 0:
        return None
    start = int(bucket["bucket_start_unix_ms"])
    return {
        "schema": FILL_SCHEMA,
        "bucket_start_unix_ms": start,
        "bucket_end_unix_ms": start + bucket_ms,
        "sample_count": count,
        "fill_min_pct": bucket["fill_min"],
        "fill_mean_pct": float(bucket["fill_sum"]) / count,
        "fill_max_pct": bucket["fill_max"],
        "occupancy_65_72_pct": 100.0 * int(bucket["in_65_72"]) / count,
        "occupancy_65_73_5_pct": 100.0 * int(bucket["in_65_73_5"]) / count,
        "semantic_fresh_pct": 100.0 * int(bucket["semantic_fresh"]) / count,
        "audio_fresh_pct": 100.0 * int(bucket["audio_fresh"]) / count,
        "aux_fresh_pct": 100.0 * int(bucket["aux_fresh"]) / count,
        "first_sample_unix_ms": bucket["first_sample_unix_ms"],
        "last_sample_unix_ms": bucket["last_sample_unix_ms"],
        "authority": "deterministic_rollup_of_read_only_cpu_esn_telemetry",
    }


def ingest_fill(
    workspace: Path,
    state: dict[str, Any],
    bucket_minutes: int,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    path = workspace / "runtime/fill_history.jsonl"
    bucket_ms = bucket_minutes * 60_000
    source = state.get("fill_source")
    source = source if isinstance(source, dict) else {}
    offset = int(source.get("offset", 0) or 0)
    pending = source.get("pending")
    pending = pending if isinstance(pending, dict) else None
    inode = source.get("inode")
    try:
        metadata = path.stat()
    except OSError:
        return [], {"offset": 0, "inode": None, "pending": pending}
    if inode != metadata.st_ino or metadata.st_size < offset:
        offset = 0
        pending = None
    completed: list[dict[str, Any]] = []
    with path.open("rb") as handle:
        handle.seek(offset)
        while True:
            line_start = handle.tell()
            line = handle.readline()
            if not line:
                break
            if not line.endswith(b"\n"):
                handle.seek(line_start)
                break
            offset = handle.tell()
            try:
                value = json.loads(line)
                timestamp = int(value["recorded_at_unix_ms"])
            except (json.JSONDecodeError, KeyError, TypeError, ValueError):
                continue
            start = timestamp - timestamp % bucket_ms
            if pending is None:
                pending = empty_fill_bucket(start)
            if int(pending["bucket_start_unix_ms"]) != start:
                finalized = final_fill_bucket(pending, bucket_ms)
                if finalized is not None:
                    completed.append(finalized)
                pending = empty_fill_bucket(start)
            add_fill_sample(pending, value)
    return completed, {"offset": offset, "inode": metadata.st_ino, "pending": pending}


def ledger_summary(
    path: Path,
    after_snapshot: Any | None = None,
) -> dict[str, Any]:
    """Summarize one immutable prefix of an append-only ledger.

    The writer may append while this function runs.  Capture the size from the
    already-open descriptor, then hash and parse exactly that many bytes.  A
    later append is deliberately left for the next checkpoint.  The optional
    callback exists only to make the append race deterministic in tests.
    """
    try:
        handle = path.open("rb")
    except OSError:
        return {"present": False}
    with handle:
        try:
            metadata = os.fstat(handle.fileno())
        except OSError:
            return {"present": False}
        snapshot_size = metadata.st_size
        if after_snapshot is not None:
            after_snapshot()

        digest = hashlib.sha256()
        remaining = snapshot_size
        pending = b""
        line_count = 0
        valid_json = 0
        invalid_json = 0
        first_timestamp: int | None = None
        last_timestamp: int | None = None

        def inspect_line(raw_line: bytes) -> None:
            nonlocal line_count, valid_json, invalid_json
            nonlocal first_timestamp, last_timestamp
            line_count += 1
            try:
                value = json.loads(raw_line.decode("utf-8", errors="replace"))
            except json.JSONDecodeError:
                invalid_json += 1
                return
            valid_json += 1
            if not isinstance(value, dict):
                return
            timestamp = next(
                (
                    int(value[field])
                    for field in TIMESTAMP_FIELDS
                    if isinstance(value.get(field), (int, float))
                    and int(value[field]) > 0
                ),
                None,
            )
            if timestamp is not None:
                first_timestamp = (
                    timestamp
                    if first_timestamp is None
                    else min(first_timestamp, timestamp)
                )
                last_timestamp = (
                    timestamp
                    if last_timestamp is None
                    else max(last_timestamp, timestamp)
                )

        while remaining > 0:
            chunk = handle.read(min(1024 * 1024, remaining))
            if not chunk:
                break
            remaining -= len(chunk)
            digest.update(chunk)
            pending += chunk
            parts = pending.split(b"\n")
            pending = parts.pop()
            for raw_line in parts:
                inspect_line(raw_line)
    return {
        "present": True,
        "size_bytes": snapshot_size,
        "inode": metadata.st_ino,
        "hash_scope": LEDGER_HASH_SCOPE,
        "mode": f"{stat.S_IMODE(metadata.st_mode):04o}",
        "line_count": line_count,
        "valid_json_lines": valid_json,
        "invalid_json_lines": invalid_json,
        "trailing_partial_bytes": len(pending),
        "first_timestamp_unix_ms": first_timestamp,
        "last_timestamp_unix_ms": last_timestamp,
        "sha256": digest.hexdigest(),
    }


def database_inventory(path: Path) -> dict[str, Any]:
    if not path.is_dir():
        return {"present": False, "path": str(path)}
    files: list[dict[str, Any]] = []
    total = 0
    latest = 0
    owner_only = True
    for directory, names, filenames in os.walk(path, followlinks=False):
        names[:] = [name for name in names if not (Path(directory) / name).is_symlink()]
        for name in sorted(filenames):
            candidate = Path(directory) / name
            try:
                metadata = candidate.lstat()
            except OSError:
                continue
            if not stat.S_ISREG(metadata.st_mode):
                continue
            relative = candidate.relative_to(path).as_posix()
            mode = stat.S_IMODE(metadata.st_mode)
            owner_only = owner_only and mode & 0o077 == 0
            total += metadata.st_size
            latest = max(latest, metadata.st_mtime_ns // 1_000_000)
            files.append(
                {
                    "path": relative,
                    "size_bytes": metadata.st_size,
                    "mtime_unix_ms": metadata.st_mtime_ns // 1_000_000,
                    "mode": f"{mode:04o}",
                }
            )
    inventory_hash = hashlib.sha256(
        json.dumps(files, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    return {
        "present": True,
        "path": str(path),
        "file_count": len(files),
        "size_bytes": total,
        "last_modified_unix_ms": latest or None,
        "owner_only_files": owner_only,
        "manifest_files": sum("/manifest/" in f"/{item['path']}" for item in files),
        "sstable_files": sum("/sstables/" in f"/{item['path']}" for item in files),
        "wal_files": sum("/wal/" in f"/{item['path']}" for item in files),
        "lock_present": any(item["path"] == "LOCK" for item in files),
        "inventory_sha256": inventory_hash,
    }


def audit_alert_count(state_root: Path) -> int:
    total = 0
    log_dir = state_root / "log"
    try:
        paths = sorted(log_dir.glob("astrid.*.log"))[-14:]
    except OSError:
        return 0
    needles = (
        "Audit chain integrity violation detected",
        "Audit chain verification found tampered sessions",
    )
    for path in paths:
        try:
            with path.open("r", encoding="utf-8", errors="replace") as handle:
                total += sum(any(needle in line for needle in needles) for line in handle)
        except OSError:
            continue
    return total


def activity_events(workspace: Path, current_ms: int) -> list[dict[str, Any]]:
    module = load_activity_module(infer_state_root(workspace))
    if module is None:
        return []
    return list(module.collect_events(workspace, current_ms))


def prepare_hindsight_database(connection: sqlite3.Connection) -> None:
    connection.executescript(
        """
        CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS activity_events (
            event_id TEXT PRIMARY KEY,
            timestamp_unix_ms INTEGER NOT NULL,
            kind TEXT NOT NULL,
            authored INTEGER,
            fallback INTEGER,
            trace_id TEXT,
            session_id TEXT,
            chain_id TEXT,
            status TEXT,
            declared_next TEXT,
            source_ledger TEXT NOT NULL,
            payload_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS activity_events_time
            ON activity_events(timestamp_unix_ms);
        CREATE INDEX IF NOT EXISTS activity_events_trace
            ON activity_events(trace_id, timestamp_unix_ms);
        CREATE INDEX IF NOT EXISTS activity_events_kind
            ON activity_events(kind, timestamp_unix_ms);
        CREATE TABLE IF NOT EXISTS artifact_versions (
            record_sha256 TEXT PRIMARY KEY,
            timestamp_unix_ms INTEGER NOT NULL,
            relative_path TEXT NOT NULL,
            content_sha256 TEXT,
            authority TEXT NOT NULL,
            astrid_authored INTEGER NOT NULL,
            causal_attribution TEXT NOT NULL,
            trace_id TEXT,
            payload_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS artifact_versions_time
            ON artifact_versions(timestamp_unix_ms);
        CREATE INDEX IF NOT EXISTS artifact_versions_path
            ON artifact_versions(relative_path, timestamp_unix_ms);
        CREATE TABLE IF NOT EXISTS fill_rollups (
            record_sha256 TEXT PRIMARY KEY,
            bucket_start_unix_ms INTEGER NOT NULL,
            bucket_end_unix_ms INTEGER NOT NULL,
            sample_count INTEGER NOT NULL,
            fill_min_pct REAL NOT NULL,
            fill_mean_pct REAL NOT NULL,
            fill_max_pct REAL NOT NULL,
            occupancy_65_72_pct REAL NOT NULL,
            occupancy_65_73_5_pct REAL NOT NULL,
            payload_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS fill_rollups_time
            ON fill_rollups(bucket_start_unix_ms);
        CREATE TABLE IF NOT EXISTS spectral_rollups (
            record_sha256 TEXT PRIMARY KEY,
            recorded_at_unix_ms INTEGER NOT NULL,
            substrate_kind TEXT NOT NULL,
            fill_metric TEXT NOT NULL,
            fill_pct REAL,
            spectral_entropy REAL,
            lambda1_share REAL,
            tail_share REAL,
            density_gradient REAL,
            mode_turnover REAL,
            trace_id TEXT,
            payload_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS spectral_rollups_time
            ON spectral_rollups(recorded_at_unix_ms);
        CREATE INDEX IF NOT EXISTS spectral_rollups_trace
            ON spectral_rollups(trace_id, recorded_at_unix_ms);
        CREATE TABLE IF NOT EXISTS tuning_events (
            event_id TEXT PRIMARY KEY,
            recorded_at_unix_ms INTEGER NOT NULL,
            tuning_id TEXT,
            candidate_id TEXT,
            phase TEXT NOT NULL,
            status TEXT,
            parameter TEXT,
            requested_value REAL,
            trace_id TEXT,
            session_id TEXT,
            chain_id TEXT,
            response_sha256 TEXT,
            payload_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS tuning_events_time
            ON tuning_events(recorded_at_unix_ms);
        CREATE INDEX IF NOT EXISTS tuning_events_trace
            ON tuning_events(trace_id, recorded_at_unix_ms);
        CREATE TABLE IF NOT EXISTS checkpoints (
            record_sha256 TEXT PRIMARY KEY,
            recorded_at_unix_ms INTEGER NOT NULL,
            continuity_valid INTEGER,
            historical_violation_count INTEGER NOT NULL,
            payload_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS checkpoints_time
            ON checkpoints(recorded_at_unix_ms);
        """
    )
    connection.execute(
        "INSERT OR REPLACE INTO metadata(key, value) VALUES(?, ?)",
        ("schema_version", str(DATABASE_SCHEMA_VERSION)),
    )
    connection.execute(
        "INSERT OR REPLACE INTO metadata(key, value) VALUES(?, ?)",
        (
            "authority",
            "operator_observability_index_not_astrid_memory_or_authorship",
        ),
    )


def sync_hindsight_database(
    operator_root: Path,
    workspace: Path,
    current_ms: int,
) -> dict[str, Any]:
    path = operator_root / "hindsight.sqlite3"
    connection = sqlite3.connect(path, timeout=30)
    try:
        connection.execute("PRAGMA journal_mode=WAL")
        connection.execute("PRAGMA synchronous=FULL")
        prepare_hindsight_database(connection)
        for event in activity_events(workspace, current_ms):
            payload = json.dumps(event, sort_keys=True, separators=(",", ":"))
            event_id = hashlib.sha256(payload.encode()).hexdigest()
            connection.execute(
                """
                INSERT OR IGNORE INTO activity_events(
                    event_id, timestamp_unix_ms, kind, authored, fallback,
                    trace_id, session_id, chain_id, status, declared_next,
                    source_ledger, payload_json
                ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    event_id,
                    int(event.get("timestamp_unix_ms", 0) or 0),
                    str(event.get("kind", "unknown")),
                    None
                    if event.get("authored") is None
                    else int(bool(event.get("authored"))),
                    None
                    if event.get("fallback") is None
                    else int(bool(event.get("fallback"))),
                    event.get("trace_id"),
                    event.get("session_id"),
                    event.get("chain_id"),
                    event.get("status"),
                    event.get("declared_next"),
                    str(event.get("source_ledger", "unknown")),
                    payload,
                ),
            )
        for value in json_lines(operator_root / "artifacts.jsonl"):
            trace = value.get("trace")
            trace_id = trace.get("trace_id") if isinstance(trace, dict) else None
            timestamp = int(
                value.get("causal_timestamp_unix_ms")
                or value.get("file_mtime_unix_ms")
                or value.get("observed_at_unix_ms")
                or 0
            )
            connection.execute(
                """
                INSERT OR IGNORE INTO artifact_versions(
                    record_sha256, timestamp_unix_ms, relative_path,
                    content_sha256, authority, astrid_authored,
                    causal_attribution, trace_id, payload_json
                ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    value.get("record_sha256"),
                    timestamp,
                    str(value.get("relative_path", "unknown")),
                    value.get("content_sha256"),
                    str(value.get("authority", "unknown")),
                    int(bool(value.get("astrid_authored"))),
                    str(value.get("causal_attribution", "unknown")),
                    trace_id,
                    json.dumps(value, sort_keys=True, separators=(",", ":")),
                ),
            )
        for value in json_lines(operator_root / "fill_rollups.jsonl"):
            connection.execute(
                """
                INSERT OR IGNORE INTO fill_rollups(
                    record_sha256, bucket_start_unix_ms, bucket_end_unix_ms,
                    sample_count, fill_min_pct, fill_mean_pct, fill_max_pct,
                    occupancy_65_72_pct, occupancy_65_73_5_pct, payload_json
                ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    value.get("record_sha256"),
                    int(value.get("bucket_start_unix_ms", 0) or 0),
                    int(value.get("bucket_end_unix_ms", 0) or 0),
                    int(value.get("sample_count", 0) or 0),
                    float(value.get("fill_min_pct", 0.0) or 0.0),
                    float(value.get("fill_mean_pct", 0.0) or 0.0),
                    float(value.get("fill_max_pct", 0.0) or 0.0),
                    float(value.get("occupancy_65_72_pct", 0.0) or 0.0),
                    float(value.get("occupancy_65_73_5_pct", 0.0) or 0.0),
                    json.dumps(value, sort_keys=True, separators=(",", ":")),
                ),
            )
        for value in json_lines(workspace / "spectral/rollups.jsonl"):
            payload = json.dumps(value, sort_keys=True, separators=(",", ":"))
            trace = value.get("trace")
            trace_id = trace.get("trace_id") if isinstance(trace, dict) else None
            substrate = value.get("substrate")
            substrate = substrate if isinstance(substrate, dict) else {}
            record_sha256 = value.get("record_sha256") or hashlib.sha256(
                payload.encode()
            ).hexdigest()
            connection.execute(
                """
                INSERT OR IGNORE INTO spectral_rollups(
                    record_sha256, recorded_at_unix_ms, substrate_kind,
                    fill_metric, fill_pct, spectral_entropy, lambda1_share,
                    tail_share, density_gradient, mode_turnover, trace_id,
                    payload_json
                ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    record_sha256,
                    int(value.get("recorded_at_unix_ms", 0) or 0),
                    str(
                        substrate.get("kind")
                        or value.get("substrate_kind")
                        or "legacy_unknown"
                    ),
                    str(
                        substrate.get("fill_metric")
                        or value.get("fill_metric")
                        or "legacy_unknown"
                    ),
                    spectral_metric(value, "fill_pct"),
                    spectral_metric(value, "spectral_entropy"),
                    spectral_metric(value, "lambda1_share"),
                    spectral_metric(value, "tail_share"),
                    spectral_metric(value, "density_gradient"),
                    spectral_metric(value, "mode_turnover"),
                    trace_id,
                    payload,
                ),
            )
        for relative in ("spectral/receipts.jsonl", "tuning/receipts.jsonl"):
            for raw_value in json_lines(workspace / relative):
                value = (
                    flatten_signed_tuning(raw_value)
                    if relative == "tuning/receipts.jsonl"
                    else raw_value
                )
                payload = json.dumps(
                    raw_value, sort_keys=True, separators=(",", ":")
                )
                trace = value.get("trace")
                trace = trace if isinstance(trace, dict) else {}
                event_id = value.get("record_sha256") or hashlib.sha256(
                    f"{relative}:{payload}".encode()
                ).hexdigest()
                connection.execute(
                    """
                    INSERT OR IGNORE INTO tuning_events(
                        event_id, recorded_at_unix_ms, tuning_id, candidate_id,
                        phase, status, parameter, requested_value, trace_id,
                        session_id, chain_id, response_sha256, payload_json
                    ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        event_id,
                        int(value.get("recorded_at_unix_ms", 0) or 0),
                        value.get("tuning_id") or value.get("experiment_id"),
                        value.get("candidate_id"),
                        str(value.get("phase") or value.get("kind") or "observation"),
                        value.get("status"),
                        value.get("parameter"),
                        value.get("requested_value") or value.get("value"),
                        trace.get("trace_id"),
                        trace.get("session_id") or value.get("session_id"),
                        trace.get("chain_id") or value.get("chain_id"),
                        value.get("response_sha256")
                        or value.get("parent_response_sha256"),
                        payload,
                    ),
                )
        for value in json_lines(operator_root / "checkpoints.jsonl"):
            connection.execute(
                """
                INSERT OR IGNORE INTO checkpoints(
                    record_sha256, recorded_at_unix_ms, continuity_valid,
                    historical_violation_count, payload_json
                ) VALUES(?, ?, ?, ?, ?)
                """,
                (
                    value.get("record_sha256"),
                    int(value.get("recorded_at_unix_ms", 0) or 0),
                    None
                    if value.get("continuity_from_previous_checkpoint_valid") is None
                    else int(
                        bool(value.get("continuity_from_previous_checkpoint_valid"))
                    ),
                    int(
                        value.get("historical_ledger_integrity_violation_count", 0)
                        or 0
                    ),
                    json.dumps(value, sort_keys=True, separators=(",", ":")),
                ),
            )
        connection.execute(
            "INSERT OR REPLACE INTO metadata(key, value) VALUES(?, ?)",
            ("last_sync_unix_ms", str(current_ms)),
        )
        connection.commit()
        quick_check = str(connection.execute("PRAGMA quick_check").fetchone()[0])
        counts = {
            table: int(connection.execute(f"SELECT count(*) FROM {table}").fetchone()[0])
            for table in (
                "activity_events",
                "artifact_versions",
                "fill_rollups",
                "spectral_rollups",
                "tuning_events",
                "checkpoints",
            )
        }
    finally:
        connection.close()
    for candidate in operator_root.glob("hindsight.sqlite3*"):
        if candidate.is_file():
            candidate.chmod(0o600)
    return {
        "schema_version": DATABASE_SCHEMA_VERSION,
        "path": str(path),
        "quick_check": quick_check,
        "owner_only": stat.S_IMODE(path.stat().st_mode) & 0o077 == 0,
        "size_bytes": path.stat().st_size,
        "row_counts": counts,
        "last_sync_unix_ms": current_ms,
        "authority": "operator_observability_index_not_astrid_memory_or_authorship",
    }


def hindsight_database_status(path: Path) -> dict[str, Any]:
    if not path.is_file():
        return {"present": False, "path": str(path)}
    try:
        connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True, timeout=10)
        try:
            quick_check = str(connection.execute("PRAGMA quick_check").fetchone()[0])
            counts = {
                table: int(
                    connection.execute(f"SELECT count(*) FROM {table}").fetchone()[0]
                )
                for table in (
                    "activity_events",
                    "artifact_versions",
                    "fill_rollups",
                    "spectral_rollups",
                    "tuning_events",
                    "checkpoints",
                )
            }
            metadata_rows = connection.execute(
                "SELECT key, value FROM metadata"
            ).fetchall()
            database_metadata = {
                str(key): str(value) for key, value in metadata_rows
            }
        finally:
            connection.close()
    except sqlite3.Error as error:
        return {
            "present": True,
            "path": str(path),
            "quick_check": f"error:{error}",
            "row_counts": {},
        }
    metadata = path.stat()
    return {
        "present": True,
        "path": str(path),
        "quick_check": quick_check,
        "owner_only": stat.S_IMODE(metadata.st_mode) & 0o077 == 0,
        "size_bytes": metadata.st_size,
        "row_counts": counts,
        "last_sync_unix_ms": int(database_metadata.get("last_sync_unix_ms", "0")),
    }


def query_hindsight_database(
    path: Path,
    start_ms: int,
    end_ms: int,
    limit: int,
) -> dict[str, Any]:
    connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True, timeout=10)
    try:
        activity_count = int(
            connection.execute(
                """
                SELECT count(*) FROM activity_events
                WHERE timestamp_unix_ms BETWEEN ? AND ?
                """,
                (start_ms, end_ms),
            ).fetchone()[0]
        )
        latest_activity_timestamp = int(
            connection.execute(
                "SELECT coalesce(max(timestamp_unix_ms), 0) FROM activity_events"
            ).fetchone()[0]
        )
        activity_payloads = connection.execute(
            """
            SELECT payload_json FROM activity_events
            WHERE timestamp_unix_ms BETWEEN ? AND ?
            ORDER BY timestamp_unix_ms DESC, event_id DESC
            LIMIT ?
            """,
            (start_ms, end_ms, limit),
        ).fetchall()
        artifact_payloads = connection.execute(
            """
            SELECT payload_json FROM artifact_versions
            WHERE timestamp_unix_ms BETWEEN ? AND ?
            ORDER BY timestamp_unix_ms, rowid
            """,
            (start_ms, end_ms),
        ).fetchall()
        fill_payloads = connection.execute(
            """
            SELECT payload_json FROM fill_rollups
            WHERE bucket_end_unix_ms >= ? AND bucket_start_unix_ms <= ?
            ORDER BY bucket_start_unix_ms, record_sha256
            """,
            (start_ms, end_ms),
        ).fetchall()
        spectral_payloads = connection.execute(
            """
            SELECT payload_json FROM spectral_rollups
            WHERE recorded_at_unix_ms BETWEEN ? AND ?
            ORDER BY recorded_at_unix_ms, record_sha256
            """,
            (start_ms, end_ms),
        ).fetchall()
        tuning_payloads = connection.execute(
            """
            SELECT payload_json FROM tuning_events
            WHERE recorded_at_unix_ms BETWEEN ? AND ?
            ORDER BY recorded_at_unix_ms, event_id
            LIMIT ?
            """,
            (start_ms, end_ms, limit),
        ).fetchall()
        last_sync_row = connection.execute(
            "SELECT value FROM metadata WHERE key = 'last_sync_unix_ms'"
        ).fetchone()
    finally:
        connection.close()

    def payloads(rows: list[tuple[str]]) -> list[dict[str, Any]]:
        result: list[dict[str, Any]] = []
        for (payload,) in rows:
            try:
                value = json.loads(payload)
            except json.JSONDecodeError:
                continue
            if isinstance(value, dict):
                result.append(value)
        return result

    return {
        "activity_count": activity_count,
        "activity": list(reversed(payloads(activity_payloads))),
        "artifacts": payloads(artifact_payloads),
        "fill_rollups": payloads(fill_payloads),
        "spectral_rollups": payloads(spectral_payloads),
        "tuning_events": [
            flatten_signed_tuning(value) for value in payloads(tuning_payloads)
        ],
        "last_sync_unix_ms": int(last_sync_row[0]) if last_sync_row else 0,
        "latest_activity_timestamp_unix_ms": latest_activity_timestamp,
    }


def infer_state_root(workspace: Path) -> Path:
    resolved = workspace.expanduser().resolve()
    if len(resolved.parents) >= 3 and resolved.name == "edge" and resolved.parent.name == "default" and resolved.parent.parent.name == "home":
        return resolved.parents[2]
    raise ValueError("--state-root is required when workspace is not STATE/home/default/edge")


def record(args: argparse.Namespace) -> dict[str, Any]:
    workspace = args.workspace.expanduser().resolve()
    state_root = (args.state_root or infer_state_root(workspace)).expanduser().resolve()
    operator_root = (args.operator_root or state_root / "operator/hindsight").expanduser().resolve()
    owner_directory(operator_root)
    state_path = operator_root / "collector_state.json"
    state = read_json(state_path)
    prior_state_schema = state.get("schema")
    if prior_state_schema not in {LEGACY_STATE_SCHEMA, STATE_SCHEMA}:
        state = {"schema": STATE_SCHEMA}
        prior_state_schema = None
    else:
        state["schema"] = STATE_SCHEMA
    observed_at = now_ms()
    prior_checkpoint = read_json(operator_root / "latest.json")
    migrating_from_v1 = bool(prior_checkpoint) and (
        prior_checkpoint.get("schema") != CHECKPOINT_SCHEMA
        or prior_state_schema == LEGACY_STATE_SCHEMA
    )
    continuity_epoch = state.get("continuity_epoch")
    if not isinstance(continuity_epoch, str) or not continuity_epoch:
        continuity_epoch = f"hindsight-v2-{observed_at}"
    if migrating_from_v1:
        continuity_epoch = f"hindsight-v2-{observed_at}"
        prior_ledger_continuity = {
            str(relative): "migration_baseline_no_prior_continuity_claim"
            for relative, summary in prior_checkpoint.get("ledgers", {}).items()
            if isinstance(summary, dict) and summary.get("present")
        }
        prior_ledger_continuity_valid: bool | None = None
        continuity_status = "migration_baseline_no_prior_continuity_claim"
    else:
        prior_ledger_continuity = checkpoint_prefix_status(workspace, prior_checkpoint)
        prior_ledger_continuity_valid = (
            not prior_checkpoint
            or bool(prior_ledger_continuity)
            and all(
                value in {"unchanged_and_verified", "append_only_advance_verified"}
                for value in prior_ledger_continuity.values()
            )
        )
        continuity_status = (
            "no_prior_checkpoint"
            if not prior_checkpoint
            else "verified"
            if prior_ledger_continuity_valid
            else "integrity_violation"
        )
    integrity_violations = state.get("ledger_integrity_violations")
    integrity_violations = (
        integrity_violations if isinstance(integrity_violations, list) else []
    )
    epoch_integrity_violations = state.get("epoch_integrity_violations")
    epoch_integrity_violations = (
        epoch_integrity_violations
        if isinstance(epoch_integrity_violations, list) and not migrating_from_v1
        else []
    )
    legacy_violation_count = state.get("legacy_violation_count_at_v2_migration")
    if not isinstance(legacy_violation_count, int):
        legacy_violation_count = len(integrity_violations) if migrating_from_v1 else 0
    if prior_checkpoint and prior_ledger_continuity_valid is False:
        violation = {
            "detected_at_unix_ms": observed_at,
            "prior_checkpoint_recorded_at_unix_ms": prior_checkpoint.get(
                "recorded_at_unix_ms"
            ),
            "continuity_epoch": continuity_epoch,
            "statuses": {
                key: value
                for key, value in prior_ledger_continuity.items()
                if value
                not in {"unchanged_and_verified", "append_only_advance_verified"}
            },
        }
        integrity_violations.append(violation)
        epoch_integrity_violations.append(violation)
        integrity_violations = integrity_violations[-100:]
        epoch_integrity_violations = epoch_integrity_violations[-100:]

    artifact_records, artifact_inventory = scan_artifacts(workspace, state, observed_at)
    artifact_hash, artifacts_written = append_chained(
        operator_root / "artifacts.jsonl",
        artifact_records,
        state.get("artifact_record_sha256"),
    )

    fill_records, fill_source = ingest_fill(workspace, state, args.bucket_minutes)
    fill_hash, fill_written = append_chained(
        operator_root / "fill_rollups.jsonl",
        fill_records,
        state.get("fill_record_sha256"),
    )

    ledgers = {
        relative: ledger_summary(workspace / relative)
        for relative in ACTIVITY_LEDGERS
    }
    state_database = database_inventory(state_root / "var/state.db")
    audit_database = database_inventory(state_root / "home/default/.local/audit")
    checkpoint = {
        "schema": CHECKPOINT_SCHEMA,
        "recorded_at_unix_ms": observed_at,
        "continuity_epoch": continuity_epoch,
        "continuity_status": continuity_status,
        "workspace": str(workspace),
        "collector_state_root": str(state_root),
        "ledgers": ledgers,
        "continuity_from_previous_checkpoint": prior_ledger_continuity,
        "continuity_from_previous_checkpoint_valid": prior_ledger_continuity_valid,
        "historical_ledger_integrity_violation_count": len(integrity_violations),
        "legacy_race_compatible_unresolved_violation_count": legacy_violation_count,
        "current_epoch_integrity_violation_count": len(epoch_integrity_violations),
        "artifacts_discovered": artifacts_written,
        "artifact_inventory_count": len(artifact_inventory),
        "fill_rollups_completed": fill_written,
        "fill_source_offset": fill_source.get("offset"),
        "state_database": state_database,
        "audit_database": {
            **audit_database,
            "daemon_verification_contract": "cryptographic_verify_all_on_every_daemon_boot",
            "integrity_alerts_in_retained_daemon_logs": audit_alert_count(state_root),
            "live_offline_verification": "not_attempted_database_is_locked_by_running_daemon",
        },
        "authority": "operator_observability_checkpoint_not_astrid_memory_or_authorship",
    }
    checkpoint_hash, checkpoints_written = append_chained(
        operator_root / "checkpoints.jsonl",
        [checkpoint],
        state.get("checkpoint_record_sha256"),
    )
    operator_database = sync_hindsight_database(
        operator_root, workspace, observed_at
    )
    state = {
        "schema": STATE_SCHEMA,
        "updated_at_unix_ms": observed_at,
        "continuity_epoch": continuity_epoch,
        "legacy_violation_count_at_v2_migration": legacy_violation_count,
        "bucket_minutes": args.bucket_minutes,
        "artifact_attribution_version": ARTIFACT_ATTRIBUTION_VERSION,
        "artifact_inventory": artifact_inventory,
        "artifact_record_sha256": artifact_hash,
        "fill_source": fill_source,
        "fill_record_sha256": fill_hash,
        "checkpoint_record_sha256": checkpoint_hash,
        "ledger_integrity_violations": integrity_violations,
        "epoch_integrity_violations": epoch_integrity_violations,
    }
    owner_write_json(state_path, state)
    owner_write_json(
        operator_root / "latest.json",
        {
            **checkpoint,
            "checkpoint_record_sha256": checkpoint_hash,
            "artifact_chain_head_sha256": artifact_hash,
            "fill_chain_head_sha256": fill_hash,
            "operator_hindsight_database": operator_database,
        },
    )
    return {
        "schema": "astrid_edge_hindsight_record_result_v1",
        "recorded_at_unix_ms": observed_at,
        "operator_root": str(operator_root),
        "artifacts_written": artifacts_written,
        "fill_rollups_written": fill_written,
        "checkpoints_written": checkpoints_written,
    }


def verify_chain(path: Path) -> dict[str, Any]:
    previous: str | None = None
    count = 0
    issues: list[str] = []
    for line_number, value in enumerate(json_lines(path), 1):
        claimed = value.get("record_sha256")
        payload = {key: item for key, item in value.items() if key != "record_sha256"}
        actual = digest_value(payload)
        if value.get("previous_record_sha256") != previous:
            issues.append(f"line {line_number}: previous hash mismatch")
        if claimed != actual:
            issues.append(f"line {line_number}: record hash mismatch")
        previous = str(claimed) if isinstance(claimed, str) else None
        count += 1
    return {
        "present": path.is_file(),
        "valid": path.is_file() and not issues,
        "records": count,
        "head_sha256": previous,
        "issues": issues[:20],
    }


def checkpoint_prefix_status(workspace: Path, latest: dict[str, Any]) -> dict[str, Any]:
    results: dict[str, Any] = {}
    ledgers = latest.get("ledgers")
    if not isinstance(ledgers, dict):
        return results
    for relative, prior in ledgers.items():
        if not isinstance(prior, dict) or not prior.get("present"):
            continue
        if prior.get("hash_scope") != LEDGER_HASH_SCOPE:
            results[str(relative)] = "unsupported_legacy_hash_scope"
            continue
        path = workspace / str(relative)
        try:
            prior_size = int(prior["size_bytes"])
            prior_inode = int(prior["inode"])
            handle = path.open("rb")
        except (OSError, KeyError, TypeError, ValueError):
            results[str(relative)] = "missing_after_checkpoint"
            continue
        with handle:
            try:
                metadata = os.fstat(handle.fileno())
            except OSError:
                results[str(relative)] = "unreadable_after_checkpoint"
                continue
            current_size = metadata.st_size
            if metadata.st_ino != prior_inode:
                results[str(relative)] = "replaced_after_checkpoint"
            elif current_size < prior_size:
                results[str(relative)] = "shrunk_after_checkpoint"
            else:
                prefix_hash, consumed = sha256_open_prefix(handle, prior_size)
                if consumed != prior_size:
                    results[str(relative)] = "short_read_after_checkpoint"
                elif prefix_hash != prior.get("sha256"):
                    results[str(relative)] = "checkpointed_prefix_changed"
                elif current_size == prior_size:
                    results[str(relative)] = "unchanged_and_verified"
                else:
                    results[str(relative)] = "append_only_advance_verified"
    return results


def load_activity_module(state_root: Path) -> Any:
    directory = Path(__file__).resolve().parent
    candidates = (
        directory / "report_edge_activity.py",
        directory / "report-edge-activity",
        state_root / "bin/report_edge_activity.py",
        state_root / "bin/report-edge-activity",
    )
    source = next((path for path in candidates if path.is_file()), None)
    if source is None:
        return None
    loader = importlib.machinery.SourceFileLoader("astrid_edge_activity_for_hindsight", str(source))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    if spec is None:
        return None
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


def iso_time(timestamp_ms: int | None) -> str:
    if not timestamp_ms:
        return "-"
    return dt.datetime.fromtimestamp(timestamp_ms / 1000, tz=dt.timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


def parse_time(value: str) -> int:
    text = value.strip()
    try:
        number = float(text)
    except ValueError:
        number = -1
    if number >= 0:
        return int(number if number >= 10_000_000_000 else number * 1000)
    normalized = text[:-1] + "+00:00" if text.endswith("Z") else text
    parsed = dt.datetime.fromisoformat(normalized)
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=dt.timezone.utc)
    return int(parsed.timestamp() * 1000)


def short(value: Any, maximum: int = 128) -> str:
    if value in (None, ""):
        return "-"
    text = " ".join(str(value).split())
    return text if len(text) <= maximum else f"{text[: maximum - 1]}…"


def artifact_excerpt(workspace: Path, relative: str) -> str | None:
    normalized = normalize_relative_path(relative)
    if normalized is None:
        return None
    path = workspace / normalized
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
        resolved.relative_to(workspace.resolve())
    except (OSError, ValueError):
        return None
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode) or metadata.st_size > 1024 * 1024:
        return None
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    bounded = " ".join(text.split())
    return bounded[:MAX_EXCERPT_CHARS] if bounded else None


def fill_summary(records: list[dict[str, Any]]) -> dict[str, Any]:
    count = sum(int(item.get("sample_count", 0) or 0) for item in records)
    if count <= 0:
        return {"sample_count": 0, "bucket_count": 0}
    weighted = lambda field: sum(float(item.get(field, 0.0) or 0.0) * int(item.get("sample_count", 0) or 0) for item in records) / count
    return {
        "sample_count": count,
        "bucket_count": len(records),
        "fill_min_pct": min(float(item["fill_min_pct"]) for item in records),
        "fill_mean_pct": weighted("fill_mean_pct"),
        "fill_max_pct": max(float(item["fill_max_pct"]) for item in records),
        "occupancy_65_72_pct": weighted("occupancy_65_72_pct"),
        "occupancy_65_73_5_pct": weighted("occupancy_65_73_5_pct"),
        "semantic_fresh_pct": weighted("semantic_fresh_pct"),
        "audio_fresh_pct": weighted("audio_fresh_pct"),
        "aux_fresh_pct": weighted("aux_fresh_pct"),
    }


def spectral_summary(records: list[dict[str, Any]]) -> dict[str, Any]:
    """Summarize only finite, explicitly present spectral fields."""

    result: dict[str, Any] = {"rollup_count": len(records)}
    substrates = sorted(
        {
            str((item.get("substrate") or {}).get("kind") or item.get("substrate_kind"))
            for item in records
            if (item.get("substrate") or {}).get("kind") or item.get("substrate_kind")
        }
    )
    result["substrate_kinds"] = substrates
    for field in (
        "fill_pct",
        "spectral_entropy",
        "lambda1_share",
        "tail_share",
        "density_gradient",
        "mode_turnover",
    ):
        values = [
            float(spectral_metric(item, field))
            for item in records
            if isinstance(spectral_metric(item, field), (int, float))
            and math.isfinite(float(spectral_metric(item, field)))
        ]
        if values:
            result[field] = {
                "samples": len(values),
                "min": min(values),
                "mean": sum(values) / len(values),
                "max": max(values),
            }
    return result


def build_report(args: argparse.Namespace) -> dict[str, Any]:
    workspace = args.workspace.expanduser().resolve()
    state_root = (args.state_root or infer_state_root(workspace)).expanduser().resolve()
    operator_root = (args.operator_root or state_root / "operator/hindsight").expanduser().resolve()
    current_ms = now_ms()
    end_ms = parse_time(args.until) if args.until else current_ms
    start_ms = parse_time(args.since) if args.since else end_ms - args.window_minutes * 60_000
    if start_ms >= end_ms:
        raise ValueError("hindsight range must have --since before --until")

    database_path = operator_root / "hindsight.sqlite3"
    operator_database = hindsight_database_status(database_path)
    database_view: dict[str, Any] | None = None
    if operator_database.get("quick_check") == "ok":
        database_view = query_hindsight_database(
            database_path, start_ms, end_ms, args.limit
        )

    activity_module = load_activity_module(state_root)
    if database_view is not None:
        activities = list(database_view["activity"])
        latest_indexed_activity = int(
            database_view.get("latest_activity_timestamp_unix_ms", 0) or 0
        )
        live_activities = []
        if activity_module is not None and end_ms > latest_indexed_activity:
            live_activities = [
                event
                for event in activity_module.collect_events(workspace, current_ms)
                if max(start_ms, latest_indexed_activity + 1)
                <= int(event.get("timestamp_unix_ms", 0) or 0)
                <= end_ms
            ]
        combined = {
            hashlib.sha256(canonical_bytes(event)).hexdigest(): event
            for event in [*activities, *live_activities]
        }
        activities = sorted(
            combined.values(),
            key=lambda event: (
                int(event.get("timestamp_unix_ms", 0) or 0),
                str(event.get("kind", "")),
            ),
        )
        activity_count_in_range = int(database_view["activity_count"]) + len(
            live_activities
        )
    else:
        activities = []
        if activity_module is not None:
            activities = [
                event
                for event in activity_module.collect_events(workspace, current_ms)
                if start_ms <= int(event.get("timestamp_unix_ms", 0) or 0) <= end_ms
            ]
        activity_count_in_range = len(activities)
    activities = activities[-args.limit :]

    artifacts = []
    latest_by_path: dict[str, dict[str, Any]] = {}
    artifact_values = (
        database_view["artifacts"]
        if database_view is not None
        else json_lines(operator_root / "artifacts.jsonl")
    )
    for value in artifact_values:
        timestamp = int(value.get("causal_timestamp_unix_ms") or value.get("file_mtime_unix_ms") or value.get("observed_at_unix_ms") or 0)
        if start_ms <= timestamp <= end_ms:
            item = {**value, "timestamp_unix_ms": timestamp}
            latest_by_path[str(value.get("relative_path", ""))] = item
    artifacts = sorted(latest_by_path.values(), key=lambda item: int(item["timestamp_unix_ms"]))
    artifact_count_in_range = len(artifacts)
    artifacts = artifacts[-args.limit :]
    if args.include_excerpts:
        for artifact in artifacts:
            artifact["excerpt"] = artifact_excerpt(workspace, str(artifact.get("relative_path", "")))

    if database_view is not None:
        fill_records = list(database_view["fill_rollups"])
    else:
        fill_records = [
            value
            for value in json_lines(operator_root / "fill_rollups.jsonl")
            if int(value.get("bucket_end_unix_ms", 0) or 0) >= start_ms
            and int(value.get("bucket_start_unix_ms", 0) or 0) <= end_ms
        ]
    collector_state = read_json(operator_root / "collector_state.json")
    pending = collector_state.get("fill_source", {}).get("pending") if isinstance(collector_state.get("fill_source"), dict) else None
    if isinstance(pending, dict):
        finalized = final_fill_bucket(pending, int(collector_state.get("bucket_minutes", DEFAULT_BUCKET_MINUTES)) * 60_000)
        if finalized is not None and int(finalized["bucket_end_unix_ms"]) >= start_ms and int(finalized["bucket_start_unix_ms"]) <= end_ms:
            fill_records.append({**finalized, "provisional": True})

    if database_view is not None:
        spectral_records = list(database_view["spectral_rollups"])
        tuning_events = list(database_view["tuning_events"])
    else:
        spectral_records = [
            value
            for value in json_lines(workspace / "spectral/rollups.jsonl")
            if start_ms
            <= int(value.get("recorded_at_unix_ms", 0) or 0)
            <= end_ms
        ]
        tuning_events = [
            (
                flatten_signed_tuning(value)
                if relative == "tuning/receipts.jsonl"
                else value
            )
            for relative in ("spectral/receipts.jsonl", "tuning/receipts.jsonl")
            for value in json_lines(workspace / relative)
            if start_ms
            <= int(
                (
                    value.get("payload", {}).get("recorded_at_unix_ms", 0)
                    if relative == "tuning/receipts.jsonl"
                    and isinstance(value.get("payload"), dict)
                    else value.get("recorded_at_unix_ms", 0)
                )
                or 0
            )
            <= end_ms
        ][-args.limit :]

    latest = read_json(operator_root / "latest.json")
    chains = {
        "checkpoints": verify_chain(operator_root / "checkpoints.jsonl"),
        "artifacts": verify_chain(operator_root / "artifacts.jsonl"),
        "fill_rollups": verify_chain(operator_root / "fill_rollups.jsonl"),
    }
    prefix = checkpoint_prefix_status(workspace, latest)
    prefix_ok = bool(prefix) and all(value in {"unchanged_and_verified", "append_only_advance_verified"} for value in prefix.values())
    checkpoint_at = int(latest.get("recorded_at_unix_ms", 0) or 0)
    historical_violations = int(
        latest.get("historical_ledger_integrity_violation_count", 0) or 0
    )
    legacy_violations = int(
        latest.get("legacy_race_compatible_unresolved_violation_count", 0) or 0
    )
    epoch_violations = int(
        latest.get("current_epoch_integrity_violation_count", 0) or 0
    )
    checkpoint_continuity_valid = latest.get(
        "continuity_from_previous_checkpoint_valid"
    )
    chain_integrity_valid = all(value["valid"] for value in chains.values())
    return {
        "schema": REPORT_SCHEMA,
        "generated_at_unix_ms": current_ms,
        "range": {"since_unix_ms": start_ms, "until_unix_ms": end_ms},
        "workspace": str(workspace),
        "operator_root": str(operator_root),
        "integrity": {
            "checkpoint_present": bool(latest),
            "checkpoint_age_seconds": (current_ms - checkpoint_at) // 1000 if checkpoint_at else None,
            "chains": chains,
            "checkpointed_ledger_prefixes": prefix,
            "checkpointed_ledger_prefixes_valid": prefix_ok,
            "continuity_epoch": latest.get("continuity_epoch"),
            "continuity_status": latest.get("continuity_status"),
            "checkpoint_to_checkpoint_continuity_valid": checkpoint_continuity_valid,
            "historical_ledger_integrity_violation_count": historical_violations,
            "legacy_race_compatible_unresolved_violation_count": legacy_violations,
            "current_epoch_integrity_violation_count": epoch_violations,
            "overall_valid": bool(latest)
            and chain_integrity_valid
            and prefix_ok
            and checkpoint_continuity_valid is not False
            and epoch_violations == 0,
        },
        "durable_sources": {
            "historical_query_source": (
                "owner_only_sqlite_index_plus_uncheckpointed_live_tail"
                if database_view is not None
                else "hash_chained_jsonl_fallback"
            ),
            "activity_event_count_in_range": activity_count_in_range,
            "activity_events_returned": len(activities),
            "artifact_file_count_in_range": artifact_count_in_range,
            "artifact_files_returned": len(artifacts),
            "fill_rollup_count_in_range": len(fill_records),
            "spectral_rollup_count_in_range": len(spectral_records),
            "tuning_event_count_in_range": len(tuning_events),
            "state_database": latest.get("state_database") or database_inventory(state_root / "var/state.db"),
            "audit_database": latest.get("audit_database") or database_inventory(state_root / "home/default/.local/audit"),
            "operator_hindsight_database": operator_database,
        },
        "fill": {"summary": fill_summary(fill_records), "rollups": fill_records},
        "spectral": {
            "summary": spectral_summary(spectral_records),
            "rollups": spectral_records,
            "tuning_events": tuning_events,
            "authority": "deterministic_machine_derivation_not_authorship_or_causal_proof",
        },
        "activity": activities,
        "artifacts": artifacts,
        "authority_note": "Telemetry and executor records are evidence, not Astrid-authored memory; only exact authored joins are labeled authored.",
    }


def render_text(report: dict[str, Any]) -> str:
    integrity = report["integrity"]
    sources = report["durable_sources"]
    fill = report["fill"]["summary"]
    lines = [
        "# Astrid Hindsight",
        f"Range: {iso_time(report['range']['since_unix_ms'])} to {iso_time(report['range']['until_unix_ms'])}",
        f"Workspace: {report['workspace']}",
        "",
        "## Integrity and durable coverage",
        f"Checkpoint: {'present' if integrity['checkpoint_present'] else 'missing'}; age={integrity['checkpoint_age_seconds']}s; overall_valid={str(integrity['overall_valid']).lower()}; ledger_prefixes_valid={str(integrity['checkpointed_ledger_prefixes_valid']).lower()}; epoch={integrity.get('continuity_epoch')}; status={integrity.get('continuity_status')}; current_epoch_violations={integrity['current_epoch_integrity_violation_count']}; legacy_race_compatible_unresolved={integrity['legacy_race_compatible_unresolved_violation_count']}; historical_raw={integrity['historical_ledger_integrity_violation_count']}",
        "Chains: " + ", ".join(f"{name}={'valid' if value['valid'] else 'INVALID'}({value['records']})" for name, value in integrity["chains"].items()),
        f"Query source={sources['historical_query_source']}; activity events={sources['activity_event_count_in_range']} (showing {sources['activity_events_returned']}); artifact files={sources['artifact_file_count_in_range']} (showing {sources['artifact_files_returned']}); fill rollups={sources['fill_rollup_count_in_range']}",
        f"Spectral rollups={sources.get('spectral_rollup_count_in_range', 0)}; tuning lifecycle events={sources.get('tuning_event_count_in_range', 0)}",
    ]
    state_db = sources["state_database"]
    audit_db = sources["audit_database"]
    operator_db = sources["operator_hindsight_database"]
    lines.extend(
        [
            f"Kernel state DB: present={state_db.get('present')} files={state_db.get('file_count', 0)} bytes={state_db.get('size_bytes', 0)} owner_only={state_db.get('owner_only_files')}",
            f"Cryptographic audit DB: present={audit_db.get('present')} files={audit_db.get('file_count', 0)} alerts={audit_db.get('integrity_alerts_in_retained_daemon_logs', 'unknown')} boot_contract={audit_db.get('daemon_verification_contract', 'daemon_verify_all_on_boot')}",
            f"Hindsight query DB: present={operator_db.get('present')} quick_check={operator_db.get('quick_check', 'unavailable')} owner_only={operator_db.get('owner_only')} rows={operator_db.get('row_counts', {})}",
            "",
            "## Reservoir telemetry",
        ]
    )
    if fill.get("sample_count", 0):
        lines.append(
            f"samples={fill['sample_count']} min/mean/max={fill['fill_min_pct']:.2f}/{fill['fill_mean_pct']:.2f}/{fill['fill_max_pct']:.2f}% occupancy65-72={fill['occupancy_65_72_pct']:.1f}% occupancy65-73.5={fill['occupancy_65_73_5_pct']:.1f}%"
        )
        lines.append(
            f"fresh semantic/audio/aux={fill['semantic_fresh_pct']:.1f}/{fill['audio_fresh_pct']:.1f}/{fill['aux_fresh_pct']:.1f}%"
        )
    else:
        lines.append("No indexed fill samples in this range.")
    spectral = report.get("spectral", {})
    spectral_summary_value = spectral.get("summary", {})
    lines.extend(["", "## Spectral substrate"])
    if spectral_summary_value.get("rollup_count", 0):
        lines.append(
            "substrates="
            + ",".join(spectral_summary_value.get("substrate_kinds", []))
            + f" rollups={spectral_summary_value['rollup_count']} authority={spectral.get('authority')}"
        )
        for field in (
            "spectral_entropy",
            "lambda1_share",
            "tail_share",
            "density_gradient",
            "mode_turnover",
        ):
            summary = spectral_summary_value.get(field)
            if isinstance(summary, dict):
                lines.append(
                    f"{field} n={summary['samples']} min/mean/max="
                    f"{summary['min']:.4f}/{summary['mean']:.4f}/{summary['max']:.4f}"
                )
    else:
        lines.append("No substrate-labeled spectral rollups in this range.")
    for event in spectral.get("tuning_events", []):
        lines.append(
            f"{iso_time(int(event.get('recorded_at_unix_ms', 0) or 0))} "
            f"TUNING phase={event.get('phase') or event.get('kind')} "
            f"status={event.get('status')} id={event.get('tuning_id') or event.get('experiment_id')} "
            f"candidate={event.get('candidate_id')} parameter={event.get('parameter')}"
        )
    lines.extend(["", "## Causal activity"])
    for event in report["activity"]:
        timestamp = iso_time(int(event.get("timestamp_unix_ms", 0) or 0))
        kind = str(event.get("kind", "unknown")).upper()
        authored = event.get("authored")
        detail = event.get("declared_next") or event.get("query") or event.get("url") or event.get("summary") or event.get("reason") or event.get("status")
        lines.append(
            f"{timestamp} {kind} authored={str(authored).lower() if authored is not None else '-'} trace={str(event.get('trace_id') or 'legacy')[:8]} {short(detail, 180)}"
        )
    if not report["activity"]:
        lines.append("No activity events in this range.")
    lines.extend(["", "## Files written"])
    for artifact in report["artifacts"]:
        lines.append(
            f"{iso_time(int(artifact['timestamp_unix_ms']))} authored={str(artifact.get('astrid_authored')).lower()} authority={artifact.get('authority')} path={artifact.get('relative_path')} bytes={artifact.get('size_bytes')} sha256={str(artifact.get('content_sha256') or '-')[:16]} attribution={artifact.get('causal_attribution')}"
        )
        if artifact.get("excerpt"):
            lines.append(f"  “{short(artifact['excerpt'], MAX_EXCERPT_CHARS)}”")
    if not report["artifacts"]:
        lines.append("No indexed artifact versions in this range.")
    lines.extend(
        [
            "",
            "Database note: state.db is kernel/session/security state; the audit DB is chain-verified on daemon boot. The human-readable history above comes from the private append-only edge ledgers and owned files.",
            report["authority_note"],
        ]
    )
    return "\n".join(lines)


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    subparsers = result.add_subparsers(dest="command")
    record_parser = subparsers.add_parser("record", help="append an owner-only integrity checkpoint")
    report_parser = subparsers.add_parser("report", help="render a read-only retrospective report")
    for subparser in (record_parser, report_parser):
        subparser.add_argument("--workspace", type=Path, required=True)
        subparser.add_argument("--state-root", type=Path)
        subparser.add_argument("--operator-root", type=Path)
    record_parser.add_argument("--bucket-minutes", type=int, default=DEFAULT_BUCKET_MINUTES)
    record_parser.add_argument("--format", choices=("text", "json"), default="text")
    report_parser.add_argument("--window-minutes", type=int, default=24 * 60)
    report_parser.add_argument("--since", help="ISO-8601, Unix seconds, or Unix milliseconds")
    report_parser.add_argument("--until", help="ISO-8601, Unix seconds, or Unix milliseconds")
    report_parser.add_argument("--limit", type=int, default=100)
    report_parser.add_argument("--include-excerpts", action="store_true")
    report_parser.add_argument("--format", choices=("text", "json"), default="text")
    return result


def default_workspace() -> Path | None:
    candidates = (
        Path.home() / ".astrid-icp/state/home/default/edge",
        Path.home() / ".astrid/home/default/edge",
    )
    return next((candidate for candidate in candidates if candidate.is_dir()), None)


def main() -> int:
    arguments = sys.argv[1:]
    if not arguments or arguments[0] not in {"record", "report", "-h", "--help"}:
        workspace = default_workspace()
        if workspace is None:
            raise SystemExit("no default edge workspace found; use report --workspace PATH")
        arguments = ["report", "--workspace", str(workspace), "--include-excerpts", *arguments]
    args = parser().parse_args(arguments)
    if args.command is None:
        parser().print_help()
        return 2
    if args.command == "record":
        if args.bucket_minutes < 1 or args.bucket_minutes > 24 * 60:
            raise SystemExit("--bucket-minutes must be between 1 and 1440")
        output = record(args)
        if args.format == "json":
            print(json.dumps(output, sort_keys=True))
        else:
            print(
                f"hindsight checkpoint recorded: artifacts={output['artifacts_written']} "
                f"fill_rollups={output['fill_rollups_written']} root={output['operator_root']}"
            )
        return 0
    if args.window_minutes < 1 or args.limit < 1:
        raise SystemExit("--window-minutes and --limit must be positive")
    try:
        output = build_report(args)
    except ValueError as error:
        raise SystemExit(str(error)) from error
    if args.format == "json":
        print(json.dumps(output, sort_keys=True))
    else:
        print(render_text(output))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
