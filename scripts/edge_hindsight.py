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
import contextlib
import datetime as dt
import fcntl
import hashlib
import importlib.util
import json
import math
import os
import re
import sqlite3
import stat
import sys
import time
import types
import unicodedata
import uuid
from pathlib import Path
from typing import Any, Iterable, Iterator

LEGACY_CHECKPOINT_SCHEMA = "astrid_edge_hindsight_checkpoint_v1"
CHECKPOINT_SCHEMA = "astrid_edge_hindsight_checkpoint_v2"
ARTIFACT_SCHEMA = "astrid_edge_hindsight_artifact_version_v1"
FILL_SCHEMA = "astrid_edge_hindsight_fill_rollup_v1"
REPORT_SCHEMA = "astrid_edge_hindsight_report_v1"
LEGACY_STATE_SCHEMA = "astrid_edge_hindsight_collector_state_v1"
STATE_SCHEMA = "astrid_edge_hindsight_collector_state_v2"
DATABASE_SCHEMA_VERSION = 6
ARTIFACT_ATTRIBUTION_VERSION = 4
ATTRIBUTION_PROJECTION_VERSION = 5
SPECTRAL_TUNING_PROJECTION_VERSION = 2
SELF_EVOLUTION_PROJECTION_VERSION = 3
INQUIRY_PROJECTION_VERSION = 1
SEALED_WRITER_CONFIG_SCHEMA = "astrid.edge.hindsight_writer.config.v2"
MAX_SEALED_WRITER_CONFIG_BYTES = 32 * 1024
MAX_OPERATOR_REPORT_MANIFEST_BYTES = 256 * 1024
MAX_ACTIVITY_REPORT_BYTES = 16 * 1024 * 1024
MAX_TRAIN_REPORT_BYTES = 4 * 1024 * 1024
IMMUTABLE_ACTIVITY_REPORT_PATH = Path(
    "/usr/libexec/astrid-edge/operator/report_edge_activity.py"
)
IMMUTABLE_TRAIN_REPORT_PATH = Path(
    "/usr/libexec/astrid-edge/operator/astrid_train.py"
)
IMMUTABLE_OPERATOR_REPORT_MANIFEST_PATH = Path(
    "/usr/libexec/astrid-edge/operator/MANIFEST.sha256"
)
LEDGER_HASH_SCOPE = "exact_open_file_prefix_v1"
DEFAULT_BUCKET_MINUTES = 15
MAX_EXCERPT_CHARS = 480
MAX_TRACE_LABEL_CHARS = 96
INDIRECT_SHADOW_GATE_EVIDENCE = (
    "indirect_package_replay_sha256_commitment_not_independently_reinspectable"
)

ACTIVITY_LEDGERS = (
    "actions/dispatches.jsonl",
    "actions/receipts.jsonl",
    "actions/interrupted_corrections.jsonl",
    "autonomous/runs.jsonl",
    "autonomous/chains.jsonl",
    "autonomous/recoveries.jsonl",
    "autonomous/authorship_corrections.jsonl",
    "autonomous/thread_state.jsonl",
    "web/receipts.jsonl",
    "introspection/receipts.jsonl",
    "introspections/scheduled/receipts.jsonl",
    "introspection/scheduled/receipts.jsonl",
    "runtime/scheduled-introspection/admission/receipts.jsonl",
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
        return {
            **value,
            "signed_envelope": False,
            "payload_hash_valid": None,
            "signature_present_not_verified": False,
        }
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


def read_host_boot_id() -> str | None:
    """Return the canonical Linux boot ID, or fail closed when unavailable."""
    try:
        value = Path("/proc/sys/kernel/random/boot_id").read_text(
            encoding="utf-8"
        ).strip()
        return str(uuid.UUID(value))
    except (OSError, ValueError):
        return None


def canonical_bytes(value: dict[str, Any]) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def strict_json_loads(value: str | bytes) -> Any:
    """Decode standard JSON and reject values Python would make non-finite."""

    def reject_constant(constant: str) -> None:
        raise ValueError(f"non-standard JSON constant: {constant}")

    decoded = json.loads(value, parse_constant=reject_constant)
    pending = [decoded]
    while pending:
        item = pending.pop()
        if isinstance(item, float) and not math.isfinite(item):
            raise ValueError("JSON number is outside the finite numeric domain")
        if isinstance(item, dict):
            pending.extend(item.values())
        elif isinstance(item, list):
            pending.extend(item)
    return decoded


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


def validate_root_regular(
    path: Path, *, expected_mode: int, maximum_bytes: int
) -> os.stat_result:
    """Verify one immutable root-owned path and every ancestor without links."""
    if not path.is_absolute():
        raise ValueError("sealed hindsight path must be absolute")
    cursor = Path("/")
    for component in path.parts[1:]:
        cursor /= component
        metadata = cursor.lstat()
        if stat.S_ISLNK(metadata.st_mode):
            raise ValueError("sealed hindsight path contains a symlink")
        if cursor != path and (
            not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid != 0
            or stat.S_IMODE(metadata.st_mode) & 0o022
        ):
            raise ValueError("sealed hindsight ancestors are not immutable")
    metadata = path.lstat()
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_nlink != 1
        or stat.S_IMODE(metadata.st_mode) != expected_mode
        or metadata.st_size > maximum_bytes
    ):
        raise ValueError("sealed hindsight file identity is invalid")
    return metadata


def stable_root_read(path: Path, *, expected_mode: int, maximum_bytes: int) -> bytes:
    before = validate_root_regular(
        path, expected_mode=expected_mode, maximum_bytes=maximum_bytes
    )
    descriptor = os.open(
        path,
        os.O_RDONLY | os.O_CLOEXEC | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            raise ValueError("sealed hindsight file changed while opening")
        chunks: list[bytes] = []
        remaining = maximum_bytes + 1
        while remaining:
            chunk = os.read(descriptor, min(remaining, 1024 * 1024))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
    finally:
        os.close(descriptor)
    after = validate_root_regular(
        path, expected_mode=expected_mode, maximum_bytes=maximum_bytes
    )
    identity = lambda value: (
        value.st_dev,
        value.st_ino,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )
    payload = b"".join(chunks)
    if identity(before) != identity(after) or len(payload) > maximum_bytes:
        raise ValueError("sealed hindsight file changed while reading")
    return payload


def parse_sealed_writer_config(value: Any) -> dict[str, Any]:
    """Validate one exact root-authored writer binding without touching disk."""
    if not isinstance(value, dict) or set(value) != {
        "schema",
        "appliance_id",
        "workspace",
        "state_root",
        "operator_root",
        "bucket_minutes",
        "writer_path",
        "writer_sha256",
        "activity_report_path",
        "activity_report_sha256",
        "operator_report_manifest_path",
        "operator_report_manifest_sha256",
    }:
        raise ValueError("sealed hindsight configuration fields are not exact")
    appliance_id = value.get("appliance_id")
    writer_sha256 = value.get("writer_sha256")
    activity_report_sha256 = value.get("activity_report_sha256")
    operator_report_manifest_sha256 = value.get(
        "operator_report_manifest_sha256"
    )
    bucket_minutes = value.get("bucket_minutes")
    paths = {
        name: value.get(name)
        for name in (
            "workspace",
            "state_root",
            "operator_root",
            "writer_path",
            "activity_report_path",
            "operator_report_manifest_path",
        )
    }
    if (
        value.get("schema") != SEALED_WRITER_CONFIG_SCHEMA
        or not isinstance(appliance_id, str)
        or not re.fullmatch(r"[a-z0-9][a-z0-9._-]{0,63}", appliance_id)
        or type(bucket_minutes) is not int
        or bucket_minutes != DEFAULT_BUCKET_MINUTES
        or not isinstance(writer_sha256, str)
        or len(writer_sha256) != 64
        or any(character not in "0123456789abcdef" for character in writer_sha256)
        or not isinstance(activity_report_sha256, str)
        or len(activity_report_sha256) != 64
        or any(
            character not in "0123456789abcdef"
            for character in activity_report_sha256
        )
        or not isinstance(operator_report_manifest_sha256, str)
        or len(operator_report_manifest_sha256) != 64
        or any(
            character not in "0123456789abcdef"
            for character in operator_report_manifest_sha256
        )
        or any(
            not isinstance(item, str)
            or not item.startswith("/")
            or item.startswith("//")
            or "\x00" in item
            or item != os.path.normpath(item)
            for item in paths.values()
        )
    ):
        raise ValueError("sealed hindsight configuration escaped immutable bounds")
    resolved = {name: Path(item) for name, item in paths.items()}
    if resolved["workspace"] != resolved["state_root"] / "home/default/edge" or resolved[
        "operator_root"
    ] != resolved["state_root"] / "operator/hindsight":
        raise ValueError("sealed hindsight roots are not exactly derived")
    if resolved["activity_report_path"] != IMMUTABLE_ACTIVITY_REPORT_PATH:
        raise ValueError("sealed hindsight activity report path is not exact")
    if (
        resolved["operator_report_manifest_path"]
        != IMMUTABLE_OPERATOR_REPORT_MANIFEST_PATH
    ):
        raise ValueError("sealed hindsight operator manifest path is not exact")
    return {
        "appliance_id": appliance_id,
        "bucket_minutes": bucket_minutes,
        "writer_sha256": writer_sha256,
        "activity_report_sha256": activity_report_sha256,
        "operator_report_manifest_sha256": operator_report_manifest_sha256,
        **resolved,
    }


def load_sealed_writer_config(path: Path) -> dict[str, Any]:
    payload = stable_root_read(
        path,
        expected_mode=0o440,
        maximum_bytes=MAX_SEALED_WRITER_CONFIG_BYTES,
    )
    canonical_payload = payload[:-1] if payload.endswith(b"\n") else payload
    if b"\n" in canonical_payload:
        raise ValueError("sealed hindsight configuration is not one JSON line")
    value = strict_json_loads(canonical_payload)
    if canonical_bytes(value) != canonical_payload:
        raise ValueError("sealed hindsight configuration is not canonical")
    config = parse_sealed_writer_config(value)
    writer_path = config["writer_path"]
    writer = stable_root_read(
        writer_path,
        expected_mode=0o444,
        maximum_bytes=16 * 1024 * 1024,
    )
    if hashlib.sha256(writer).hexdigest() != config["writer_sha256"]:
        raise ValueError("sealed hindsight writer digest mismatch")
    if writer_path.resolve() != Path(__file__).resolve():
        raise ValueError("sealed hindsight config does not bind the running writer")
    return config


def parse_operator_report_manifest(payload: bytes) -> dict[Path, str]:
    """Parse one root-authored sha256sum inventory without path inference."""
    if not payload or not payload.endswith(b"\n"):
        raise ValueError("sealed hindsight operator manifest is not newline terminated")
    try:
        text = payload.decode("ascii")
    except UnicodeDecodeError as error:
        raise ValueError("sealed hindsight operator manifest is not ASCII") from error
    entries: dict[Path, str] = {}
    for line in text.splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  (/[^\x00-\x1f\x7f]+)", line)
        if match is None:
            raise ValueError("sealed hindsight operator manifest record is malformed")
        candidate = match.group(2)
        if candidate.startswith("//") or candidate != os.path.normpath(candidate):
            raise ValueError("sealed hindsight operator manifest path is not canonical")
        path = Path(candidate)
        if path in entries:
            raise ValueError("sealed hindsight operator manifest has duplicate paths")
        entries[path] = match.group(1)
    return entries


def verified_activity_report_sources(
    config: dict[str, Any]
) -> tuple[bytes, bytes]:
    """Return exact activity and train sources bound by one sealed manifest."""
    report_path = config["activity_report_path"]
    manifest_path = config["operator_report_manifest_path"]
    report = stable_root_read(
        report_path,
        expected_mode=0o444,
        maximum_bytes=MAX_ACTIVITY_REPORT_BYTES,
    )
    manifest = stable_root_read(
        manifest_path,
        expected_mode=0o444,
        maximum_bytes=MAX_OPERATOR_REPORT_MANIFEST_BYTES,
    )
    report_sha256 = hashlib.sha256(report).hexdigest()
    manifest_sha256 = hashlib.sha256(manifest).hexdigest()
    if report_sha256 != config["activity_report_sha256"]:
        raise ValueError("sealed hindsight activity report digest mismatch")
    if manifest_sha256 != config["operator_report_manifest_sha256"]:
        raise ValueError("sealed hindsight operator manifest digest mismatch")
    entries = parse_operator_report_manifest(manifest)
    if entries.get(report_path) != report_sha256:
        raise ValueError("sealed hindsight activity report manifest binding mismatch")
    train_path = report_path.with_name("astrid_train.py")
    if train_path != IMMUTABLE_TRAIN_REPORT_PATH:
        raise ValueError("sealed hindsight train dependency path is not exact")
    expected_train_sha256 = entries.get(train_path)
    if expected_train_sha256 is None:
        raise ValueError("sealed hindsight train dependency manifest binding is absent")
    train = stable_root_read(
        train_path,
        expected_mode=0o444,
        maximum_bytes=MAX_TRAIN_REPORT_BYTES,
    )
    if hashlib.sha256(train).hexdigest() != expected_train_sha256:
        raise ValueError("sealed hindsight train dependency digest mismatch")
    return report, train


def verified_activity_report_source(config: dict[str, Any]) -> bytes:
    """Compatibility wrapper returning the report after all dependencies verify."""
    report, _train = verified_activity_report_sources(config)
    return report


def module_from_verified_source(
    path: Path,
    payload: bytes,
    *,
    name: str = "astrid_edge_activity_for_hindsight",
    injected: dict[str, Any] | None = None,
) -> Any:
    """Execute only already-verified bytes, closing the hash/import TOCTOU gap."""
    module = types.ModuleType(name)
    module.__file__ = str(path)
    module.__package__ = ""
    module.__loader__ = None
    module.__spec__ = importlib.util.spec_from_loader(
        name, loader=None, origin=str(path)
    )
    if injected:
        module.__dict__.update(injected)
    code = compile(payload, str(path), "exec", dont_inherit=True)
    exec(code, module.__dict__)  # noqa: S102 - source is root-owned and digest-bound.
    if module.__file__ != str(path):
        raise ValueError("sealed hindsight activity module identity changed during load")
    return module


def load_verified_activity_module(config: dict[str, Any]) -> Any:
    activity_source, train_source = verified_activity_report_sources(config)
    train_module = module_from_verified_source(
        IMMUTABLE_TRAIN_REPORT_PATH,
        train_source,
        name="astrid_edge_train_for_hindsight",
    )
    return module_from_verified_source(
        config["activity_report_path"],
        activity_source,
        injected={"_SEALED_TRAIN_MODULE": train_module},
    )


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


def fsync_directory(path: Path) -> None:
    """Persist directory-entry changes made by an atomic replace/create."""
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0)
    descriptor = os.open(path, flags)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


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
        fsync_directory(path.parent)
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
    flags = os.O_RDWR | os.O_CREAT | os.O_APPEND
    flags |= getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags, 0o600)
    with os.fdopen(descriptor, "r+b") as handle:
        os.fchmod(handle.fileno(), 0o600)
        handle.seek(0)
        chain, known_hashes = inspect_chain_bytes(handle.read(), present=True)
        if not chain["valid"]:
            issues = "; ".join(chain["issues"])
            raise RuntimeError(f"refusing to append to invalid chain {path}: {issues}")
        actual_head = chain["head_sha256"]
        if (
            previous_hash is not None
            and previous_hash != actual_head
            and previous_hash not in known_hashes
        ):
            raise RuntimeError(
                f"collector state head is not an ancestor of {path}: {previous_hash}"
            )
        # A valid on-disk chain is authoritative after a crash between the
        # append fsync and collector-state replacement. Continue from its real
        # head instead of emitting a broken previous-hash link.
        previous_hash = actual_head
        handle.seek(0, os.SEEK_END)
        for source in values:
            record = {**source, "previous_record_sha256": previous_hash}
            record["record_sha256"] = digest_value(record)
            handle.write(
                json.dumps(record, sort_keys=True, separators=(",", ":")).encode()
            )
            handle.write(b"\n")
            previous_hash = str(record["record_sha256"])
            count += 1
        handle.flush()
        os.fsync(handle.fileno())
    fsync_directory(path.parent)
    return previous_hash, count


@contextlib.contextmanager
def exclusive_collector_lock(operator_root: Path) -> Iterator[None]:
    """Serialize a complete collector checkpoint across timer/manual runs."""
    owner_directory(operator_root)
    lock_path = operator_root / "collector.lock"
    flags = os.O_RDWR | os.O_CREAT
    flags |= getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(lock_path, flags, 0o600)
    with os.fdopen(descriptor, "r+b") as handle:
        mode = os.fstat(handle.fileno()).st_mode
        if not stat.S_ISREG(mode):
            raise RuntimeError(f"collector lock is not a regular file: {lock_path}")
        os.fchmod(handle.fileno(), 0o600)
        try:
            fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RuntimeError(
                f"another edge hindsight collector already holds {lock_path}"
            ) from error
        try:
            yield
        finally:
            fcntl.flock(handle.fileno(), fcntl.LOCK_UN)


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = strict_json_loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, ValueError):
        return {}
    return value if isinstance(value, dict) else {}


def json_lines(path: Path) -> Iterator[dict[str, Any]]:
    try:
        handle = path.open("rb")
    except OSError:
        return
    with handle:
        for line in handle:
            try:
                value = strict_json_loads(line)
            except (UnicodeError, ValueError):
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


def normalized_uuid(value: Any) -> str | None:
    try:
        parsed = uuid.UUID(str(value))
    except (TypeError, ValueError, AttributeError):
        return None
    return str(parsed) if parsed.int != 0 else None


def trace_summary(value: dict[str, Any]) -> dict[str, Any] | None:
    trace = value.get("trace")
    if not isinstance(trace, dict) or trace.get("schema_version", 1) != 1:
        return None

    def identifier(name: str, *, required: bool) -> str | None:
        raw = trace.get(name)
        if raw is None:
            if required:
                raise ValueError(name)
            return None
        parsed = normalized_uuid(raw)
        if parsed is None:
            raise ValueError(name)
        return parsed

    def label(name: str, *, required: bool) -> str | None:
        raw = trace.get(name)
        if raw is None:
            if required:
                raise ValueError(name)
            return None
        if (
            not isinstance(raw, str)
            or not raw.strip()
            or len(raw) > MAX_TRACE_LABEL_CHARS
            or any(ord(character) < 0x20 or ord(character) == 0x7F for character in raw)
        ):
            raise ValueError(name)
        return raw

    try:
        result = {
            "schema_version": 1,
            "trace_id": identifier("trace_id", required=True),
            "span_id": identifier("span_id", required=True),
            "session_id": label("session_id", required=True),
        }
        for name, normalized in (
            ("parent_span_id", identifier("parent_span_id", required=False)),
            ("turn_id", identifier("turn_id", required=False)),
            ("chain_id", label("chain_id", required=False)),
        ):
            if normalized is not None:
                result[name] = normalized
        if result.get("parent_span_id") == result.get("span_id"):
            return None
    except (TypeError, ValueError, AttributeError):
        return None
    return result


def metadata_label(value: Any, maximum: int = 240) -> str | None:
    """Return bounded printable metadata, never free-form model or tool text."""
    if not isinstance(value, str) or not value or len(value) > maximum:
        return None
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in value):
        return None
    return value


def metadata_sha256(value: Any) -> str | None:
    label = metadata_label(value, 64)
    if label is None or len(label) != 64:
        return None
    return label if all(character in "0123456789abcdef" for character in label) else None


def metadata_integer(value: Any, *, maximum: int = 2**63 - 1) -> int | None:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= maximum:
        return None
    return value


def scheduled_reflection_projection(event: dict[str, Any]) -> dict[str, Any] | None:
    """Project one scheduled reflection without retaining any authored text."""
    if event.get("kind") != "scheduled_introspection":
        return None
    timestamp = metadata_integer(event.get("timestamp_unix_ms"))
    if timestamp in {None, 0}:
        return None
    source_values = event.get("source_ledgers")
    if not isinstance(source_values, list):
        source_values = [event.get("source_ledger")]
    source_ledgers = sorted(
        {
            str(value)
            for value in source_values
            if value
            in {
                "introspections/scheduled/receipts.jsonl",
                "introspection/scheduled/receipts.jsonl",
            }
        }
    )
    if not source_ledgers:
        return None
    reflection_path = normalize_relative_path(event.get("reflection_path"))
    projected = {
        "schema": "astrid_edge_hindsight_scheduled_reflection_v1",
        "timestamp_unix_ms": timestamp,
        "status": metadata_label(event.get("status"), 80),
        "authored": event.get("authored") is True,
        "fallback": event.get("fallback") is True,
        "provenance": metadata_label(event.get("provenance"), 120),
        "authorship_class": metadata_label(event.get("authorship_class"), 120),
        "continuity_projected": event.get("continuity_projected") is True,
        "continuity_admitted": event.get("continuity_admitted") is True,
        "trace_id": normalized_uuid(event.get("trace_id")),
        "span_id": normalized_uuid(event.get("span_id")),
        "parent_span_id": normalized_uuid(event.get("parent_span_id")),
        "session_id": metadata_label(event.get("session_id"), MAX_TRACE_LABEL_CHARS),
        "chain_id": metadata_label(event.get("chain_id"), MAX_TRACE_LABEL_CHARS),
        "turn_id": normalized_uuid(event.get("turn_id")),
        "response_sha256": metadata_sha256(event.get("response_sha256")),
        "reflection_path": reflection_path,
        "prompt_chars": metadata_integer(event.get("prompt_chars"), maximum=1_000_000),
        "introspection_tool": metadata_label(event.get("introspection_tool"), 80),
        "introspection_result_sha256": metadata_sha256(
            event.get("introspection_result_sha256")
        ),
        "candidate_id": metadata_label(event.get("candidate_id"), 128),
        "candidate_digest": metadata_sha256(event.get("candidate_digest")),
        "next_due_at_unix_ms": metadata_integer(event.get("next_due_at_unix_ms")),
        "source_ledgers": source_ledgers,
        "exact_duplicate_count": metadata_integer(
            event.get("exact_duplicate_count"), maximum=1_000_000
        )
        or 0,
        "authority": metadata_label(event.get("authority"), 160),
    }
    event_id = hashlib.sha256(canonical_bytes(projected)).hexdigest()
    return {**projected, "event_id": event_id}


def self_change_lifecycle_facets(event: dict[str, Any]) -> list[str]:
    projected = event.get("lifecycle_facets")
    allowed = {
        "reflection",
        "candidate",
        "build",
        "test",
        "tests",
        "invariant",
        "shadow",
        "activation",
        "restart",
        "probation",
        "rollback",
        "operator",
    }
    if (
        isinstance(projected, list)
        and projected
        and projected == sorted(set(projected))
        and set(projected).issubset(allowed)
    ):
        return list(projected)
    declared = str(event.get("lifecycle_kind") or "").lower()
    status = str(event.get("status") or "").lower()
    combined = f"{declared} {status}"
    facets: set[str] = set()
    if declared in {"candidate", "intent", "patch_export"} or "candidate" in combined:
        facets.add("candidate")
    if declared == "build" or "build" in combined:
        facets.add("build")
    if "test" in combined or metadata_sha256(event.get("tests_sha256")):
        facets.add("tests")
    if (
        "shadow" in combined
        or metadata_sha256(event.get("shadow_evidence_sha256"))
    ):
        facets.add("shadow")
    if declared == "activation" or "activation" in combined or "switch" in combined:
        facets.add("activation")
    if any(token in combined for token in ("restart", "crash", "recovery", "reconciled")):
        facets.add("restart")
    if declared == "probation" or "probation" in combined:
        facets.add("probation")
    if declared == "rollback" or "rollback" in combined or "rolled_back" in combined:
        facets.add("rollback")
    if not facets and declared in {"operator"}:
        facets.add(declared)
    return sorted(facets)


def self_change_projection(event: dict[str, Any]) -> dict[str, Any] | None:
    """Retain only bounded lifecycle metadata from the sanitized activity view."""
    if event.get("kind") != "self_change":
        return None
    timestamp = metadata_integer(event.get("timestamp_unix_ms"))
    source_ledger = normalize_relative_path(event.get("source_ledger"))
    if (
        timestamp in {None, 0}
        or source_ledger is None
        or not source_ledger.startswith("self-change/")
    ):
        return None
    facets = self_change_lifecycle_facets(event)
    projected = {
        "schema": "astrid_edge_hindsight_self_change_event_v2",
        "timestamp_unix_ms": timestamp,
        "lifecycle_kind": metadata_label(event.get("lifecycle_kind"), 80)
        or (facets[0] if facets else "unknown"),
        "lifecycle_facets": facets,
        "status": metadata_label(event.get("status"), 120),
        "candidate_id": metadata_label(event.get("candidate_id"), 128),
        "candidate_digest": metadata_sha256(
            event.get("candidate_digest") or event.get("candidate_sha256")
        ),
        "build_id": metadata_label(event.get("build_id"), 128),
        "generation_id": metadata_label(event.get("generation_id"), 128),
        "from_generation": metadata_label(event.get("from_generation"), 128),
        "tests_sha256": metadata_sha256(event.get("tests_sha256")),
        "bundle_sha256": metadata_sha256(event.get("bundle_sha256")),
        "shadow_evidence_sha256": metadata_sha256(
            event.get("shadow_evidence_sha256")
        ),
        "package_replay_sha256_present": (
            event.get("package_replay_sha256_present") is True
        ),
        "shadow_gate_evidence": (
            INDIRECT_SHADOW_GATE_EVIDENCE
            if event.get("package_replay_sha256_present") is True
            else None
        ),
        "manifest_sha256": metadata_sha256(event.get("manifest_sha256")),
        "invariant_candidate_replay_sha256": metadata_sha256(
            event.get("invariant_candidate_replay_sha256")
        ),
        "invariant_package_replay_sha256": metadata_sha256(
            event.get("invariant_package_replay_sha256")
        ),
        "shadow_status": metadata_label(event.get("shadow_status"), 120),
        "record_sha256": metadata_sha256(event.get("record_sha256")),
        "projection_core_sha256": metadata_sha256(
            event.get("projection_core_sha256")
        ),
        "response_sha256": metadata_sha256(event.get("response_sha256")),
        "terminal_declaration_sha256": metadata_sha256(
            event.get("terminal_declaration_sha256")
        ),
        "trace_id": normalized_uuid(event.get("trace_id")),
        "session_id": metadata_label(event.get("session_id"), MAX_TRACE_LABEL_CHARS),
        "chain_id": metadata_label(event.get("chain_id"), MAX_TRACE_LABEL_CHARS),
        "turn_id": normalized_uuid(event.get("turn_id")),
        "terminal_reason_sha256": metadata_sha256(
            event.get("terminal_reason_sha256")
        ),
        "terminal_authority": metadata_label(
            event.get("terminal_authority"), 160
        ),
        "automatic_retry": (
            event.get("automatic_retry")
            if isinstance(event.get("automatic_retry"), bool)
            else None
        ),
        "provenance": metadata_label(event.get("provenance"), 120),
        "authorship_class": metadata_label(event.get("authorship_class"), 120),
        "authority": metadata_label(event.get("authority"), 160),
        "integrity": metadata_label(event.get("integrity"), 160),
        "command_profile": metadata_label(event.get("command_profile"), 80),
        "command_executable_sha256": metadata_sha256(
            event.get("command_executable_sha256")
        ),
        "command_argv_sha256": metadata_sha256(event.get("command_argv_sha256")),
        "command_stdout_sha256": metadata_sha256(
            event.get("command_stdout_sha256")
        ),
        "command_stderr_sha256": metadata_sha256(
            event.get("command_stderr_sha256")
        ),
        "command_exit_code": metadata_integer(event.get("command_exit_code"), maximum=255),
        "command_timed_out": (
            event.get("command_timed_out")
            if isinstance(event.get("command_timed_out"), bool)
            else None
        ),
        "health_checks": metadata_integer(event.get("health_checks"), maximum=1_000_000),
        "automatic": event.get("automatic") if isinstance(event.get("automatic"), bool) else None,
        "file_count": metadata_integer(event.get("file_count"), maximum=100_000),
        "changed_lines": metadata_integer(event.get("changed_lines"), maximum=100_000),
        "source_ledger": source_ledger,
        "projected_source_ledger": metadata_label(
            event.get("projected_source_ledger"), 32
        ),
        "source_event_id": metadata_label(event.get("event_id"), 128),
        "authored": False,
    }
    if (
        projected["projected_source_ledger"] is not None
        and projected["source_event_id"] is not None
        and projected["record_sha256"] is not None
    ):
        # The operator envelope is a moving, bounded tail and may enrich an
        # already projected record with later verified build evidence.  Bind
        # identity only to immutable signed-ledger material so a restart,
        # envelope rewrite, or later enrichment updates one row instead of
        # duplicating or aliasing the lifecycle event.
        event_identity = {
            "schema": projected["schema"],
            "source_ledger": projected["source_ledger"],
            "projected_source_ledger": projected["projected_source_ledger"],
            "source_event_id": projected["source_event_id"],
            "record_sha256": projected["record_sha256"],
        }
    else:
        event_identity = {
            key: value
            for key, value in projected.items()
            if key != "projection_core_sha256"
        }
    event_id = hashlib.sha256(canonical_bytes(event_identity)).hexdigest()
    return {**projected, "event_id": event_id}


def self_evolution_projections(
    events: Iterable[dict[str, Any]],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    scheduled: dict[str, dict[str, Any]] = {}
    lifecycle: dict[str, dict[str, Any]] = {}
    for event in events:
        reflection = scheduled_reflection_projection(event)
        if reflection is not None:
            scheduled[reflection["event_id"]] = reflection
        change = self_change_projection(event)
        if change is not None:
            lifecycle[change["event_id"]] = change

    def order(value: dict[str, Any]) -> tuple[int, str]:
        return int(value["timestamp_unix_ms"]), str(value["event_id"])

    return sorted(scheduled.values(), key=order), sorted(lifecycle.values(), key=order)


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


def attribution_index(
    workspace: Path, activity_module: Any | None = None
) -> dict[str, dict[str, Any]]:
    index: dict[str, dict[str, Any]] = {}
    if activity_module is None:
        activity_module = load_activity_module()
    transport_corrections = (
        activity_module.transport_authorship_corrections(workspace)
        if activity_module is not None
        else {}
    )
    interrupted_action_corrections = {}
    if activity_module is not None:
        interrupted_action_corrections = {
            key: item
            for item in json_lines(workspace / "actions/interrupted_corrections.jsonl")
            if item.get("corrected_status")
            == "revoked_interrupted_trace_non_authored"
            if (
                key := activity_module.interrupted_correction_identity(item)
            )
            is not None
        }
    for run in json_lines(workspace / "autonomous/runs.jsonl"):
        if activity_module is not None:
            run_authorship = activity_module.run_authorship(
                run, transport_corrections
            )
            authored = run_authorship.authored
            correction = run_authorship.correction
        else:
            authored = str(run.get("status", "")) == "authored_completed"
            correction = None
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
                "causal_attribution": (
                    "exact_authorship_correction_join"
                    if correction
                    else "exact_run_path_join"
                ),
                "authority": authority,
                "astrid_authored": authored,
                "causal_timestamp_unix_ms": int(
                    run.get("completed_at_unix_ms", run.get("started_at_unix_ms", 0)) or 0
                ),
                "response_sha256": run.get("response_sha256"),
                "session_id": run.get("session_name"),
                "trace": trace_summary(run),
            }
    seen_transport_corrections: set[str] = set()
    for correction in transport_corrections.values():
        correction_id = str(
            correction.get("original_transcript_path")
            or correction.get("response_sha256")
            or ""
        )
        if not correction_id or correction_id in seen_transport_corrections:
            continue
        seen_transport_corrections.add(correction_id)
        for field in ("recovery_transcript_path", "recovery_journal_path"):
            relative = normalize_relative_path(correction.get(field))
            if relative is None:
                continue
            index[relative] = {
                "causal_attribution": "exact_authorship_correction_join",
                "authority": "executor_transport_record_not_astrid_authorship",
                "astrid_authored": False,
                "causal_timestamp_unix_ms": int(
                    correction.get("recorded_at_unix_ms", 0) or 0
                ),
                "response_sha256": correction.get("response_sha256"),
                "session_id": None,
                "trace": None,
            }
    for action in json_lines(workspace / "actions/receipts.jsonl"):
        relative = normalize_relative_path(action.get("artifact_path"))
        if relative is None:
            continue
        action_identity = (
            activity_module.exact_response_identity(action)
            if activity_module is not None
            else None
        )
        correction = interrupted_action_corrections.get(action_identity)
        transport_correction = (
            activity_module.transport_authorship_correction(
                action, transport_corrections
            )
            if activity_module is not None
            else None
        )
        if correction:
            authority, authored = "revoked_interrupted_trace_non_authored", False
        elif transport_correction:
            authority, authored = "revoked_legacy_transport_non_authored", False
        else:
            authority, authored = action_authority(
                action.get("declared_next"), action.get("decision_source")
            )
        index[relative] = {
            "causal_attribution": (
                "exact_action_correction_join"
                if correction or transport_correction
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
    activity_module: Any | None = None,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    prior = state.get("artifact_inventory")
    prior = prior if isinstance(prior, dict) else {}
    force_attribution_refresh = (
        state.get("artifact_attribution_version") != ARTIFACT_ATTRIBUTION_VERSION
    )
    current: dict[str, Any] = {}
    records: list[dict[str, Any]] = []
    attribution = attribution_index(workspace, activity_module)
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
                value = strict_json_loads(line)
                timestamp = int(value["recorded_at_unix_ms"])
            except (KeyError, OverflowError, TypeError, UnicodeError, ValueError):
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
                value = strict_json_loads(raw_line)
            except (UnicodeError, ValueError):
                invalid_json += 1
                return
            if not isinstance(value, dict):
                invalid_json += 1
                return
            valid_json += 1
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
        "snapshot_unread_bytes": remaining,
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


def activity_events(
    workspace: Path, current_ms: int, activity_module: Any | None = None
) -> list[dict[str, Any]]:
    module = activity_module if activity_module is not None else load_activity_module()
    if module is None:
        return []
    return list(module.collect_events(workspace, current_ms))


def checkpoint_ledger_paths(workspace: Path) -> dict[str, Path]:
    """Resolve every append-only activity source without recording host paths."""
    return {relative: workspace / relative for relative in ACTIVITY_LEDGERS}


def ensure_database_column(
    connection: sqlite3.Connection,
    table: str,
    column: str,
    declaration: str,
) -> None:
    """Add one optional projection column without rebuilding an existing DB."""
    if not table.replace("_", "").isalnum() or not column.replace("_", "").isalnum():
        raise ValueError("invalid SQLite projection identifier")
    existing = {
        str(row[1])
        for row in connection.execute(f'PRAGMA table_info("{table}")').fetchall()
    }
    if column not in existing:
        connection.execute(
            f'ALTER TABLE "{table}" ADD COLUMN "{column}" {declaration}'
        )


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
            turn_id TEXT,
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
            turn_id TEXT,
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
            turn_id TEXT,
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
            turn_id TEXT,
            authority_turn_id TEXT,
            response_sha256 TEXT,
            payload_sha256 TEXT,
            payload_hash_valid INTEGER,
            signature_present_not_verified INTEGER,
            payload_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS tuning_events_time
            ON tuning_events(recorded_at_unix_ms);
        CREATE INDEX IF NOT EXISTS tuning_events_trace
            ON tuning_events(trace_id, recorded_at_unix_ms);
        CREATE TABLE IF NOT EXISTS spectral_receipts (
            event_id TEXT PRIMARY KEY,
            recorded_at_unix_ms INTEGER NOT NULL,
            phase TEXT NOT NULL,
            status TEXT,
            event_kind TEXT,
            trace_id TEXT,
            session_id TEXT,
            chain_id TEXT,
            turn_id TEXT,
            response_sha256 TEXT,
            payload_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS spectral_receipts_time
            ON spectral_receipts(recorded_at_unix_ms);
        CREATE INDEX IF NOT EXISTS spectral_receipts_trace
            ON spectral_receipts(trace_id, recorded_at_unix_ms);
        CREATE TABLE IF NOT EXISTS scheduled_reflections (
            event_id TEXT PRIMARY KEY,
            recorded_at_unix_ms INTEGER NOT NULL,
            status TEXT,
            authored INTEGER NOT NULL,
            fallback INTEGER NOT NULL,
            provenance TEXT,
            authorship_class TEXT,
            continuity_projected INTEGER NOT NULL,
            continuity_admitted INTEGER NOT NULL,
            trace_id TEXT,
            session_id TEXT,
            chain_id TEXT,
            turn_id TEXT,
            response_sha256 TEXT,
            reflection_path TEXT,
            candidate_id TEXT,
            candidate_digest TEXT,
            source_ledgers_json TEXT NOT NULL,
            exact_duplicate_count INTEGER NOT NULL,
            metadata_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS scheduled_reflections_time
            ON scheduled_reflections(recorded_at_unix_ms, event_id);
        CREATE INDEX IF NOT EXISTS scheduled_reflections_trace
            ON scheduled_reflections(trace_id, recorded_at_unix_ms);
        CREATE TABLE IF NOT EXISTS self_change_events (
            event_id TEXT PRIMARY KEY,
            recorded_at_unix_ms INTEGER NOT NULL,
            lifecycle_kind TEXT NOT NULL,
            lifecycle_facets_json TEXT NOT NULL,
            status TEXT,
            candidate_id TEXT,
            candidate_digest TEXT,
            build_id TEXT,
            generation_id TEXT,
            from_generation TEXT,
            tests_sha256 TEXT,
            bundle_sha256 TEXT,
            shadow_evidence_sha256 TEXT,
            shadow_evidence_in_tests_bundle INTEGER NOT NULL DEFAULT 0,
            package_replay_sha256_present INTEGER NOT NULL DEFAULT 0,
            shadow_gate_evidence TEXT,
            manifest_sha256 TEXT,
            record_sha256 TEXT,
            response_sha256 TEXT,
            terminal_declaration_sha256 TEXT,
            trace_id TEXT,
            session_id TEXT,
            chain_id TEXT,
            turn_id TEXT,
            source_ledger TEXT NOT NULL,
            authority TEXT,
            integrity TEXT,
            metadata_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS self_change_events_time
            ON self_change_events(recorded_at_unix_ms, event_id);
        CREATE INDEX IF NOT EXISTS self_change_events_candidate
            ON self_change_events(candidate_id, recorded_at_unix_ms);
        CREATE INDEX IF NOT EXISTS self_change_events_build
            ON self_change_events(build_id, recorded_at_unix_ms);
        CREATE TABLE IF NOT EXISTS inquiry_steps (
            step_id TEXT PRIMARY KEY,
            recorded_at_unix_ms INTEGER NOT NULL,
            signed_entry_id TEXT NOT NULL,
            thread_id TEXT NOT NULL,
            parent_step_id TEXT,
            thread_operation TEXT NOT NULL,
            confidence TEXT NOT NULL,
            belief_operation TEXT,
            belief_id TEXT,
            trace_id TEXT,
            session_id TEXT,
            turn_id TEXT,
            response_sha256 TEXT NOT NULL,
            declaration_sha256 TEXT NOT NULL,
            entry_sha256 TEXT,
            train_integrity TEXT NOT NULL,
            metadata_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS inquiry_steps_time
            ON inquiry_steps(recorded_at_unix_ms, step_id);
        CREATE INDEX IF NOT EXISTS inquiry_steps_thread
            ON inquiry_steps(thread_id, recorded_at_unix_ms);
        CREATE INDEX IF NOT EXISTS inquiry_steps_trace
            ON inquiry_steps(trace_id, recorded_at_unix_ms);
        CREATE TABLE IF NOT EXISTS inquiry_evidence (
            evidence_id TEXT PRIMARY KEY,
            recorded_at_unix_ms INTEGER NOT NULL,
            thread_id TEXT,
            evidence_kind TEXT,
            epistemic_status TEXT,
            eligible_for_belief_update INTEGER NOT NULL,
            evidence_sha256 TEXT,
            trace_id TEXT,
            turn_id TEXT,
            metadata_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS inquiry_evidence_time
            ON inquiry_evidence(recorded_at_unix_ms, evidence_id);
        CREATE INDEX IF NOT EXISTS inquiry_evidence_thread
            ON inquiry_evidence(thread_id, recorded_at_unix_ms);
        CREATE TABLE IF NOT EXISTS inquiry_belief_revisions (
            revision_id TEXT PRIMARY KEY,
            recorded_at_unix_ms INTEGER NOT NULL,
            belief_id TEXT NOT NULL,
            thread_id TEXT,
            operation TEXT NOT NULL,
            evidence_ids_json TEXT NOT NULL,
            prior_revision_id TEXT,
            response_sha256 TEXT,
            source TEXT,
            metadata_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS inquiry_beliefs_time
            ON inquiry_belief_revisions(recorded_at_unix_ms, revision_id);
        CREATE INDEX IF NOT EXISTS inquiry_beliefs_identity
            ON inquiry_belief_revisions(belief_id, recorded_at_unix_ms);
        CREATE TABLE IF NOT EXISTS inquiry_thread_transitions (
            event_id TEXT PRIMARY KEY,
            recorded_at_unix_ms INTEGER NOT NULL,
            thread_id TEXT,
            step_id TEXT,
            parent_step_id TEXT,
            transition TEXT,
            trace_id TEXT,
            turn_id TEXT,
            metadata_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS inquiry_transitions_time
            ON inquiry_thread_transitions(recorded_at_unix_ms, event_id);
        CREATE INDEX IF NOT EXISTS inquiry_transitions_thread
            ON inquiry_thread_transitions(thread_id, recorded_at_unix_ms);
        CREATE TABLE IF NOT EXISTS semantic_admissions (
            admission_id TEXT PRIMARY KEY,
            recorded_at_unix_ms INTEGER NOT NULL,
            signed_entry_id TEXT,
            delivery_status TEXT NOT NULL,
            source_class TEXT,
            reservoir_generation TEXT,
            reservoir_sequence INTEGER,
            vector_sha256 TEXT,
            trace_id TEXT,
            metadata_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS semantic_admissions_time
            ON semantic_admissions(recorded_at_unix_ms, admission_id);
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
    # ``CREATE TABLE IF NOT EXISTS`` does not evolve installed projections.
    # These nullable additions keep upgrades in place and avoid destructive
    # table replacement while the source ledgers remain authoritative.
    for table, column, declaration in (
        ("activity_events", "turn_id", "TEXT"),
        ("artifact_versions", "turn_id", "TEXT"),
        ("spectral_rollups", "turn_id", "TEXT"),
        ("tuning_events", "turn_id", "TEXT"),
        ("tuning_events", "authority_turn_id", "TEXT"),
        ("tuning_events", "payload_sha256", "TEXT"),
        ("tuning_events", "payload_hash_valid", "INTEGER"),
        (
            "tuning_events",
            "signature_present_not_verified",
            "INTEGER",
        ),
        (
            "self_change_events",
            "package_replay_sha256_present",
            "INTEGER NOT NULL DEFAULT 0",
        ),
        ("self_change_events", "shadow_gate_evidence", "TEXT"),
    ):
        ensure_database_column(connection, table, column, declaration)
    connection.executescript(
        """
        CREATE INDEX IF NOT EXISTS activity_events_turn
            ON activity_events(turn_id, timestamp_unix_ms);
        CREATE INDEX IF NOT EXISTS artifact_versions_turn
            ON artifact_versions(turn_id, timestamp_unix_ms);
        CREATE INDEX IF NOT EXISTS spectral_rollups_turn
            ON spectral_rollups(turn_id, recorded_at_unix_ms);
        CREATE INDEX IF NOT EXISTS tuning_events_turn
            ON tuning_events(turn_id, recorded_at_unix_ms);
        CREATE INDEX IF NOT EXISTS tuning_events_authority_turn
            ON tuning_events(authority_turn_id, recorded_at_unix_ms);
        CREATE INDEX IF NOT EXISTS spectral_receipts_turn
            ON spectral_receipts(turn_id, recorded_at_unix_ms);
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
    activity_module: Any | None = None,
) -> dict[str, Any]:
    path = operator_root / "hindsight.sqlite3"
    connection = sqlite3.connect(path, timeout=30)
    try:
        connection.execute("PRAGMA journal_mode=WAL")
        connection.execute("PRAGMA synchronous=FULL")
        prepare_hindsight_database(connection)
        prior_projection = connection.execute(
            "SELECT value FROM metadata WHERE key = ?",
            ("attribution_projection_version",),
        ).fetchone()
        if prior_projection is None or prior_projection[0] != str(
            ATTRIBUTION_PROJECTION_VERSION
        ):
            # This database is a rebuildable operator projection.  Clearing
            # attribution-bearing tables prevents pre-correction rows from
            # coexisting with their corrected event payloads indefinitely.
            connection.execute("DELETE FROM activity_events")
            connection.execute("DELETE FROM artifact_versions")
        connection.execute(
            "INSERT OR REPLACE INTO metadata(key, value) VALUES(?, ?)",
            (
                "attribution_projection_version",
                str(ATTRIBUTION_PROJECTION_VERSION),
            ),
        )
        prior_spectral_tuning_projection = connection.execute(
            "SELECT value FROM metadata WHERE key = ?",
            ("spectral_tuning_projection_version",),
        ).fetchone()
        if (
            prior_spectral_tuning_projection is None
            or prior_spectral_tuning_projection[0]
            != str(SPECTRAL_TUNING_PROJECTION_VERSION)
        ):
            # Earlier schema-v3 projections mixed spectral observation
            # receipts into ``tuning_events``.  Both are rebuildable indexes,
            # so clear just these projections and re-read their source ledgers.
            connection.execute("DELETE FROM spectral_receipts")
            connection.execute("DELETE FROM tuning_events")
        connection.execute(
            "INSERT OR REPLACE INTO metadata(key, value) VALUES(?, ?)",
            (
                "spectral_tuning_projection_version",
                str(SPECTRAL_TUNING_PROJECTION_VERSION),
            ),
        )
        observed_events = activity_events(workspace, current_ms, activity_module)
        scheduled_reflections, self_change_events = self_evolution_projections(
            observed_events
        )
        prior_self_evolution_projection = connection.execute(
            "SELECT value FROM metadata WHERE key = ?",
            ("self_evolution_projection_version",),
        ).fetchone()
        projection_migrated = (
            prior_self_evolution_projection is None
            or prior_self_evolution_projection[0]
            != str(SELF_EVOLUTION_PROJECTION_VERSION)
        )
        # Scheduled receipts remain append-only workspace ledgers and can be
        # rebuilt. The root operator projection intentionally carries only a
        # bounded lifecycle tail, so accepted lifecycle rows accumulate by
        # stable event identity instead of disappearing when that tail moves.
        connection.execute("DELETE FROM scheduled_reflections")
        if projection_migrated:
            connection.execute("DELETE FROM self_change_events")
        connection.execute(
            "INSERT OR REPLACE INTO metadata(key, value) VALUES(?, ?)",
            (
                "self_evolution_projection_version",
                str(SELF_EVOLUTION_PROJECTION_VERSION),
            ),
        )
        prior_inquiry_projection = connection.execute(
            "SELECT value FROM metadata WHERE key = ?",
            ("inquiry_projection_version",),
        ).fetchone()
        inquiry_projection_migrated = (
            prior_inquiry_projection is None
            or prior_inquiry_projection[0] != str(INQUIRY_PROJECTION_VERSION)
        )
        if inquiry_projection_migrated:
            for table in (
                "inquiry_steps",
                "inquiry_evidence",
                "inquiry_belief_revisions",
                "inquiry_thread_transitions",
                "semantic_admissions",
            ):
                connection.execute(f'DELETE FROM "{table}"')
        connection.execute(
            "INSERT OR REPLACE INTO metadata(key, value) VALUES(?, ?)",
            ("inquiry_projection_version", str(INQUIRY_PROJECTION_VERSION)),
        )
        for event in observed_events:
            payload = json.dumps(event, sort_keys=True, separators=(",", ":"))
            if event.get("kind") == "self_change" and metadata_sha256(
                event.get("record_sha256")
            ):
                activity_identity = {
                    "kind": "self_change",
                    "source_ledger": event.get("source_ledger"),
                    "projected_source_ledger": event.get("projected_source_ledger"),
                    "event_id": event.get("event_id"),
                    "record_sha256": event.get("record_sha256"),
                }
                event_id = hashlib.sha256(canonical_bytes(activity_identity)).hexdigest()
            else:
                event_id = hashlib.sha256(payload.encode()).hexdigest()
            connection.execute(
                """
                INSERT OR IGNORE INTO activity_events(
                    event_id, timestamp_unix_ms, kind, authored, fallback,
                    trace_id, session_id, chain_id, turn_id, status,
                    declared_next, source_ledger, payload_json
                ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
                    event.get("turn_id"),
                    event.get("status"),
                    event.get("declared_next"),
                    str(event.get("source_ledger", "unknown")),
                    payload,
                ),
            )
            kind = event.get("kind")
            timestamp = int(event.get("timestamp_unix_ms", 0) or 0)
            if kind == "inquiry_step" and event.get("step_id"):
                connection.execute(
                    """
                    INSERT OR REPLACE INTO inquiry_steps(
                        step_id, recorded_at_unix_ms, signed_entry_id,
                        thread_id, parent_step_id, thread_operation,
                        confidence, belief_operation, belief_id, trace_id,
                        session_id, turn_id, response_sha256,
                        declaration_sha256, entry_sha256, train_integrity,
                        metadata_json
                    ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        event.get("step_id"),
                        timestamp,
                        event.get("signed_entry_id"),
                        event.get("thread_id"),
                        event.get("parent_step_id"),
                        event.get("thread_operation"),
                        event.get("confidence"),
                        event.get("belief_operation"),
                        event.get("belief_id"),
                        event.get("trace_id"),
                        event.get("session_id"),
                        event.get("turn_id"),
                        event.get("response_sha256"),
                        event.get("declaration_sha256"),
                        event.get("entry_sha256"),
                        event.get("train_integrity", "unavailable"),
                        payload,
                    ),
                )
            elif kind == "evidence_arrival" and event.get("evidence_id"):
                connection.execute(
                    """
                    INSERT OR REPLACE INTO inquiry_evidence(
                        evidence_id, recorded_at_unix_ms, thread_id,
                        evidence_kind, epistemic_status,
                        eligible_for_belief_update, evidence_sha256,
                        trace_id, turn_id, metadata_json
                    ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        event.get("evidence_id"),
                        timestamp,
                        event.get("thread_id"),
                        event.get("evidence_kind"),
                        event.get("status"),
                        int(bool(event.get("eligible_for_belief_update"))),
                        event.get("sha256"),
                        event.get("trace_id"),
                        event.get("turn_id"),
                        payload,
                    ),
                )
            elif kind == "belief_revision" and event.get("revision_id"):
                connection.execute(
                    """
                    INSERT OR REPLACE INTO inquiry_belief_revisions(
                        revision_id, recorded_at_unix_ms, belief_id,
                        thread_id, operation, evidence_ids_json,
                        prior_revision_id, response_sha256, source,
                        metadata_json
                    ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        event.get("revision_id"),
                        timestamp,
                        event.get("belief_id"),
                        event.get("thread_id"),
                        event.get("operation"),
                        json.dumps(event.get("evidence_ids") or [], separators=(",", ":")),
                        event.get("prior_revision_id"),
                        event.get("response_sha256"),
                        event.get("source"),
                        payload,
                    ),
                )
            elif kind == "thread_transition":
                transition_identity = {
                    "thread_id": event.get("thread_id"),
                    "step_id": event.get("step_id") or event.get("last_step_id"),
                    "parent_step_id": event.get("parent_step_id"),
                    "transition": event.get("status") or event.get("event"),
                    "timestamp_unix_ms": timestamp,
                    "source_ledger": event.get("source_ledger"),
                }
                transition_id = hashlib.sha256(
                    canonical_bytes(transition_identity)
                ).hexdigest()
                connection.execute(
                    """
                    INSERT OR REPLACE INTO inquiry_thread_transitions(
                        event_id, recorded_at_unix_ms, thread_id, step_id,
                        parent_step_id, transition, trace_id, turn_id,
                        metadata_json
                    ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        transition_id,
                        timestamp,
                        event.get("thread_id"),
                        event.get("step_id") or event.get("last_step_id"),
                        event.get("parent_step_id"),
                        event.get("status") or event.get("event"),
                        event.get("trace_id"),
                        event.get("turn_id"),
                        payload,
                    ),
                )
            elif kind == "semantic_admission" and event.get("admission_id"):
                connection.execute(
                    """
                    INSERT OR REPLACE INTO semantic_admissions(
                        admission_id, recorded_at_unix_ms, signed_entry_id,
                        delivery_status, source_class, reservoir_generation,
                        reservoir_sequence, vector_sha256, trace_id,
                        metadata_json
                    ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        event.get("admission_id"),
                        timestamp,
                        event.get("signed_entry_id"),
                        event.get("status", "unknown"),
                        event.get("source_class"),
                        event.get("reservoir_generation"),
                        event.get("reservoir_sequence"),
                        event.get("vector_sha256"),
                        event.get("trace_id"),
                        payload,
                    ),
                )
        for value in scheduled_reflections:
            metadata_json = json.dumps(
                value, sort_keys=True, separators=(",", ":")
            )
            connection.execute(
                """
                INSERT INTO scheduled_reflections(
                    event_id, recorded_at_unix_ms, status, authored, fallback,
                    provenance, authorship_class, continuity_projected,
                    continuity_admitted, trace_id, session_id, chain_id,
                    turn_id, response_sha256, reflection_path, candidate_id,
                    candidate_digest, source_ledgers_json,
                    exact_duplicate_count, metadata_json
                ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    value["event_id"],
                    value["timestamp_unix_ms"],
                    value["status"],
                    int(value["authored"]),
                    int(value["fallback"]),
                    value["provenance"],
                    value["authorship_class"],
                    int(value["continuity_projected"]),
                    int(value["continuity_admitted"]),
                    value["trace_id"],
                    value["session_id"],
                    value["chain_id"],
                    value["turn_id"],
                    value["response_sha256"],
                    value["reflection_path"],
                    value["candidate_id"],
                    value["candidate_digest"],
                    json.dumps(value["source_ledgers"], separators=(",", ":")),
                    value["exact_duplicate_count"],
                    metadata_json,
                ),
            )
        for value in self_change_events:
            metadata_json = json.dumps(
                value, sort_keys=True, separators=(",", ":")
            )
            connection.execute(
                """
                INSERT OR REPLACE INTO self_change_events(
                    event_id, recorded_at_unix_ms, lifecycle_kind,
                    lifecycle_facets_json, status, candidate_id,
                    candidate_digest, build_id, generation_id, from_generation,
                    tests_sha256, bundle_sha256, shadow_evidence_sha256,
                    shadow_evidence_in_tests_bundle,
                    package_replay_sha256_present, shadow_gate_evidence,
                    manifest_sha256,
                    record_sha256, response_sha256,
                    terminal_declaration_sha256, trace_id, session_id, chain_id,
                    turn_id, source_ledger, authority, integrity, metadata_json
                ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    value["event_id"],
                    value["timestamp_unix_ms"],
                    value["lifecycle_kind"],
                    json.dumps(value["lifecycle_facets"], separators=(",", ":")),
                    value["status"],
                    value["candidate_id"],
                    value["candidate_digest"],
                    value["build_id"],
                    value["generation_id"],
                    value["from_generation"],
                    value["tests_sha256"],
                    value["bundle_sha256"],
                    value["shadow_evidence_sha256"],
                    0,
                    int(value["package_replay_sha256_present"]),
                    value["shadow_gate_evidence"],
                    value["manifest_sha256"],
                    value["record_sha256"],
                    value["response_sha256"],
                    value["terminal_declaration_sha256"],
                    value["trace_id"],
                    value["session_id"],
                    value["chain_id"],
                    value["turn_id"],
                    value["source_ledger"],
                    value["authority"],
                    value["integrity"],
                    metadata_json,
                ),
            )
        for value in json_lines(operator_root / "artifacts.jsonl"):
            trace = trace_summary(value) or {}
            trace_id = trace.get("trace_id")
            turn_id = trace.get("turn_id")
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
                    causal_attribution, trace_id, turn_id, payload_json
                ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
                    turn_id,
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
            trace = trace_summary(value) or {}
            trace_id = trace.get("trace_id")
            turn_id = trace.get("turn_id")
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
                    turn_id, payload_json
                ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
                    turn_id,
                    payload,
                ),
            )
        for value in json_lines(workspace / "spectral/receipts.jsonl"):
            payload = json.dumps(value, sort_keys=True, separators=(",", ":"))
            trace = trace_summary(value) or {}
            event_id = value.get("record_sha256") or hashlib.sha256(
                f"spectral/receipts.jsonl:{payload}".encode()
            ).hexdigest()
            connection.execute(
                """
                INSERT OR IGNORE INTO spectral_receipts(
                    event_id, recorded_at_unix_ms, phase, status, event_kind,
                    trace_id, session_id, chain_id, turn_id,
                    response_sha256, payload_json
                ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    event_id,
                    int(value.get("recorded_at_unix_ms", 0) or 0),
                    str(value.get("phase") or value.get("kind") or "observation"),
                    value.get("status"),
                    value.get("event_kind"),
                    trace.get("trace_id"),
                    trace.get("session_id") or value.get("session_id"),
                    trace.get("chain_id") or value.get("chain_id"),
                    trace.get("turn_id") or value.get("turn_id"),
                    value.get("response_sha256")
                    or value.get("parent_response_sha256"),
                    payload,
                ),
            )
        for raw_value in json_lines(workspace / "tuning/receipts.jsonl"):
            value = flatten_signed_tuning(raw_value)
            payload = json.dumps(raw_value, sort_keys=True, separators=(",", ":"))
            trace = trace_summary(value) or {}
            event_id = value.get("record_sha256") or hashlib.sha256(
                f"tuning/receipts.jsonl:{payload}".encode()
            ).hexdigest()
            connection.execute(
                """
                INSERT OR IGNORE INTO tuning_events(
                    event_id, recorded_at_unix_ms, tuning_id, candidate_id,
                    phase, status, parameter, requested_value, trace_id,
                    session_id, chain_id, turn_id, authority_turn_id,
                    response_sha256, payload_sha256, payload_hash_valid,
                    signature_present_not_verified, payload_json
                ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
                    trace.get("turn_id") or value.get("turn_id"),
                    normalized_uuid(value.get("authority_turn_id")),
                    value.get("response_sha256")
                    or value.get("parent_response_sha256"),
                    value.get("payload_sha256"),
                    None
                    if value.get("payload_hash_valid") is None
                    else int(bool(value.get("payload_hash_valid"))),
                    int(bool(value.get("signature_present_not_verified"))),
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
                "spectral_receipts",
                "tuning_events",
                "scheduled_reflections",
                "self_change_events",
                "inquiry_steps",
                "inquiry_evidence",
                "inquiry_belief_revisions",
                "inquiry_thread_transitions",
                "semantic_admissions",
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
        "attribution_projection_version": ATTRIBUTION_PROJECTION_VERSION,
        "spectral_tuning_projection_version": (
            SPECTRAL_TUNING_PROJECTION_VERSION
        ),
        "self_evolution_projection_version": SELF_EVOLUTION_PROJECTION_VERSION,
        "inquiry_projection_version": INQUIRY_PROJECTION_VERSION,
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
            available_tables = {
                str(row[0])
                for row in connection.execute(
                    "SELECT name FROM sqlite_master WHERE type = 'table'"
                ).fetchall()
            }
            counts = {
                table: (
                    int(
                        connection.execute(
                            f'SELECT count(*) FROM "{table}"'
                        ).fetchone()[0]
                    )
                    if table in available_tables
                    else 0
                )
                for table in (
                    "activity_events",
                    "artifact_versions",
                    "fill_rollups",
                    "spectral_rollups",
                    "spectral_receipts",
                    "tuning_events",
                    "scheduled_reflections",
                    "self_change_events",
                    "inquiry_steps",
                    "inquiry_evidence",
                    "inquiry_belief_revisions",
                    "inquiry_thread_transitions",
                    "semantic_admissions",
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

    def metadata_integer(name: str) -> int:
        try:
            return int(database_metadata.get(name, "0"))
        except (TypeError, ValueError):
            return 0

    return {
        "present": True,
        "path": str(path),
        "quick_check": quick_check,
        "owner_only": stat.S_IMODE(metadata.st_mode) & 0o077 == 0,
        "size_bytes": metadata.st_size,
        "row_counts": counts,
        "schema_version": metadata_integer("schema_version"),
        "projection_upgrade_required": (
            metadata_integer("schema_version") != DATABASE_SCHEMA_VERSION
            or metadata_integer("attribution_projection_version")
            != ATTRIBUTION_PROJECTION_VERSION
            or metadata_integer("spectral_tuning_projection_version")
            != SPECTRAL_TUNING_PROJECTION_VERSION
            or metadata_integer("self_evolution_projection_version")
            != SELF_EVOLUTION_PROJECTION_VERSION
            or metadata_integer("inquiry_projection_version")
            != INQUIRY_PROJECTION_VERSION
        ),
        "attribution_projection_version": metadata_integer(
            "attribution_projection_version"
        ),
        "spectral_tuning_projection_version": metadata_integer(
            "spectral_tuning_projection_version"
        ),
        "self_evolution_projection_version": metadata_integer(
            "self_evolution_projection_version"
        ),
        "inquiry_projection_version": metadata_integer(
            "inquiry_projection_version"
        ),
        "last_sync_unix_ms": metadata_integer("last_sync_unix_ms"),
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
        spectral_receipt_payloads = connection.execute(
            """
            SELECT payload_json FROM spectral_receipts
            WHERE recorded_at_unix_ms BETWEEN ? AND ?
            ORDER BY recorded_at_unix_ms, event_id
            LIMIT ?
            """,
            (start_ms, end_ms, limit),
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
        scheduled_reflection_payloads = connection.execute(
            """
            SELECT metadata_json FROM scheduled_reflections
            WHERE recorded_at_unix_ms BETWEEN ? AND ?
            ORDER BY recorded_at_unix_ms, event_id
            LIMIT ?
            """,
            (start_ms, end_ms, limit),
        ).fetchall()
        self_change_payloads = connection.execute(
            """
            SELECT metadata_json FROM self_change_events
            WHERE recorded_at_unix_ms BETWEEN ? AND ?
            ORDER BY recorded_at_unix_ms, event_id
            LIMIT ?
            """,
            (start_ms, end_ms, limit),
        ).fetchall()
        scheduled_reflection_count = int(
            connection.execute(
                "SELECT count(*) FROM scheduled_reflections "
                "WHERE recorded_at_unix_ms BETWEEN ? AND ?",
                (start_ms, end_ms),
            ).fetchone()[0]
        )
        self_change_event_count = int(
            connection.execute(
                "SELECT count(*) FROM self_change_events "
                "WHERE recorded_at_unix_ms BETWEEN ? AND ?",
                (start_ms, end_ms),
            ).fetchone()[0]
        )
        inquiry_payloads: dict[str, list[tuple[str]]] = {}
        inquiry_counts: dict[str, int] = {}
        for label, table in (
            ("steps", "inquiry_steps"),
            ("evidence", "inquiry_evidence"),
            ("belief_revisions", "inquiry_belief_revisions"),
            ("thread_transitions", "inquiry_thread_transitions"),
            ("semantic_admissions", "semantic_admissions"),
        ):
            inquiry_counts[label] = int(
                connection.execute(
                    f'SELECT count(*) FROM "{table}" WHERE recorded_at_unix_ms BETWEEN ? AND ?',
                    (start_ms, end_ms),
                ).fetchone()[0]
            )
            inquiry_payloads[label] = connection.execute(
                f'SELECT metadata_json FROM "{table}" WHERE recorded_at_unix_ms BETWEEN ? AND ? '
                "ORDER BY recorded_at_unix_ms, rowid LIMIT ?",
                (start_ms, end_ms, limit),
            ).fetchall()
        inquiry_counts["integrity_violations"] = int(
            connection.execute(
                "SELECT count(*) FROM activity_events "
                "WHERE kind = 'integrity_violation' AND timestamp_unix_ms BETWEEN ? AND ?",
                (start_ms, end_ms),
            ).fetchone()[0]
        )
        inquiry_payloads["integrity_violations"] = connection.execute(
            "SELECT payload_json FROM activity_events "
            "WHERE kind = 'integrity_violation' AND timestamp_unix_ms BETWEEN ? AND ? "
            "ORDER BY timestamp_unix_ms, event_id LIMIT ?",
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
                value = strict_json_loads(payload)
            except (UnicodeError, ValueError):
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
        "spectral_receipts": payloads(spectral_receipt_payloads),
        "tuning_events": [
            flatten_signed_tuning(value) for value in payloads(tuning_payloads)
        ],
        "scheduled_reflections": payloads(scheduled_reflection_payloads),
        "self_change_events": payloads(self_change_payloads),
        "scheduled_reflection_count": scheduled_reflection_count,
        "self_change_event_count": self_change_event_count,
        "inquiry_counts": inquiry_counts,
        "inquiry": {
            label: payloads(rows) for label, rows in inquiry_payloads.items()
        },
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
    operator_root = (
        args.operator_root or state_root / "operator/hindsight"
    ).expanduser().resolve()
    with exclusive_collector_lock(operator_root):
        return record_locked(args)


def record_sealed(config_path: Path) -> dict[str, Any]:
    """Record through one immutable writer/path/digest binding."""
    config = load_sealed_writer_config(config_path)
    activity_module = load_verified_activity_module(config)
    args = argparse.Namespace(
        workspace=config["workspace"],
        state_root=config["state_root"],
        operator_root=config["operator_root"],
        bucket_minutes=config["bucket_minutes"],
        format="json",
    )
    with exclusive_collector_lock(config["operator_root"]):
        return record_locked(args, activity_module)


def record_locked(
    args: argparse.Namespace, activity_module: Any | None = None
) -> dict[str, Any]:
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
    pending_tail_observations = state.get("pending_tail_observations")
    pending_tail_observations = (
        pending_tail_observations
        if isinstance(pending_tail_observations, dict) and not migrating_from_v1
        else {}
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

    artifact_records, artifact_inventory = scan_artifacts(
        workspace, state, observed_at, activity_module
    )
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
        relative: ledger_summary(path)
        for relative, path in checkpoint_ledger_paths(workspace).items()
    }
    existing_syntax_violations = {
        (str(value.get("ledger")), str(value.get("snapshot_sha256")))
        for value in epoch_integrity_violations
        if isinstance(value, dict)
        and value.get("classification") == "ledger_malformed_json_or_partial_tail"
    }
    for relative, summary in ledgers.items():
        invalid_lines = int(summary.get("invalid_json_lines", 0) or 0)
        trailing_bytes = int(summary.get("trailing_partial_bytes", 0) or 0)
        unread_bytes = int(summary.get("snapshot_unread_bytes", 0) or 0)
        if not summary.get("present") or not (
            invalid_lines or trailing_bytes or unread_bytes
        ):
            pending_tail_observations.pop(relative, None)
            continue
        signature = (relative, str(summary.get("sha256")))
        if signature in existing_syntax_violations:
            pending_tail_observations.pop(relative, None)
            continue
        tail_only = trailing_bytes > 0 and invalid_lines == 0 and unread_bytes == 0
        tail_observation = {
            "snapshot_sha256": summary.get("sha256"),
            "snapshot_size_bytes": summary.get("size_bytes"),
            "trailing_partial_bytes": trailing_bytes,
            "first_observed_at_unix_ms": observed_at,
        }
        if tail_only:
            prior_tail = pending_tail_observations.get(relative)
            stable_tail = (
                isinstance(prior_tail, dict)
                and prior_tail.get("snapshot_sha256")
                == tail_observation["snapshot_sha256"]
                and prior_tail.get("snapshot_size_bytes")
                == tail_observation["snapshot_size_bytes"]
                and prior_tail.get("trailing_partial_bytes")
                == tail_observation["trailing_partial_bytes"]
            )
            if not stable_tail:
                pending_tail_observations[relative] = tail_observation
                continue
        pending_tail_observations.pop(relative, None)
        violation = {
            "classification": "ledger_malformed_json_or_partial_tail",
            "detected_at_unix_ms": observed_at,
            "continuity_epoch": continuity_epoch,
            "ledger": relative,
            "snapshot_sha256": summary.get("sha256"),
            "snapshot_size_bytes": summary.get("size_bytes"),
            "invalid_json_lines": invalid_lines,
            "trailing_partial_bytes": trailing_bytes,
            "snapshot_unread_bytes": unread_bytes,
            "confirmation": (
                "stable_tail_confirmed_across_subsequent_checkpoint"
                if tail_only
                else "complete_malformed_record_or_snapshot_short_read"
            ),
        }
        integrity_violations.append(violation)
        epoch_integrity_violations.append(violation)
    integrity_violations = integrity_violations[-100:]
    epoch_integrity_violations = epoch_integrity_violations[-100:]
    if epoch_integrity_violations:
        continuity_status = "integrity_violation"
    state_database = database_inventory(state_root / "var/state.db")
    audit_database = database_inventory(state_root / "home/default/.local/audit")
    checkpoint = {
        "schema": CHECKPOINT_SCHEMA,
        "recorded_at_unix_ms": observed_at,
        "host_boot_id": read_host_boot_id(),
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
        "pending_tail_observation_count": len(pending_tail_observations),
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
        operator_root, workspace, observed_at, activity_module
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
        "pending_tail_observations": pending_tail_observations,
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


def inspect_chain_bytes(
    raw: bytes, *, present: bool
) -> tuple[dict[str, Any], set[str]]:
    previous: str | None = None
    count = 0
    issues: list[str] = []
    known_hashes: set[str] = set()
    raw_lines = raw.splitlines(keepends=True)
    for line_number, raw_line in enumerate(raw_lines, 1):
        if not raw_line.endswith(b"\n"):
            issues.append(f"line {line_number}: unterminated trailing record")
        try:
            value = strict_json_loads(raw_line)
        except (UnicodeError, ValueError):
            issues.append(f"line {line_number}: malformed JSON record")
            continue
        if not isinstance(value, dict):
            issues.append(f"line {line_number}: record is not an object")
            continue
        claimed = value.get("record_sha256")
        payload = {key: item for key, item in value.items() if key != "record_sha256"}
        actual = digest_value(payload)
        if value.get("previous_record_sha256") != previous:
            issues.append(f"line {line_number}: previous hash mismatch")
        if claimed != actual:
            issues.append(f"line {line_number}: record hash mismatch")
        previous = str(claimed) if isinstance(claimed, str) else None
        if previous is not None:
            known_hashes.add(previous)
        count += 1
    result = {
        "present": present,
        "valid": present and not issues,
        "records": count,
        "head_sha256": previous,
        "issues": issues[:20],
    }
    return result, known_hashes


def verify_chain(path: Path) -> dict[str, Any]:
    try:
        raw = path.read_bytes()
    except OSError:
        raw = b""
        present = False
    else:
        present = path.is_file()
    result, _known_hashes = inspect_chain_bytes(raw, present=present)
    return result


def checkpoint_prefix_status(workspace: Path, latest: dict[str, Any]) -> dict[str, Any]:
    results: dict[str, Any] = {}
    ledgers = latest.get("ledgers")
    if not isinstance(ledgers, dict):
        return results
    paths = checkpoint_ledger_paths(workspace)
    for relative, prior in ledgers.items():
        if not isinstance(prior, dict) or not prior.get("present"):
            continue
        if prior.get("hash_scope") != LEDGER_HASH_SCOPE:
            results[str(relative)] = "unsupported_legacy_hash_scope"
            continue
        path = paths.get(str(relative), workspace / str(relative))
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


def load_activity_module() -> Any:
    """Load only the co-installed development reporter outside sealed mode."""
    source = Path(__file__).resolve().parent / "report_edge_activity.py"
    if not source.is_file():
        return None
    try:
        payload = source.read_bytes()
    except OSError:
        return None
    return module_from_verified_source(source, payload)


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


def terminal_safe_text(value: Any) -> str:
    """Neutralize terminal controls without changing JSON report values."""

    return "".join(
        " "
        if unicodedata.category(character) in {"Cc", "Cf", "Cs", "Zl", "Zp"}
        else character
        for character in str(value)
    )


def short(value: Any, maximum: int = 128) -> str:
    if value in (None, ""):
        return "-"
    text = " ".join(terminal_safe_text(value).split())
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


def self_evolution_summary(
    scheduled: list[dict[str, Any]], lifecycle: list[dict[str, Any]]
) -> dict[str, Any]:
    statuses: dict[str, int] = {}
    facets: dict[str, int] = {}
    for value in scheduled:
        status = str(value.get("status") or "unknown")
        statuses[status] = statuses.get(status, 0) + 1
    for value in lifecycle:
        for facet in value.get("lifecycle_facets") or []:
            label = str(facet)
            facets[label] = facets.get(label, 0) + 1
    return {
        "scheduled_reflection_count": len(scheduled),
        "scheduled_authored_count": sum(
            value.get("authored") is True for value in scheduled
        ),
        "scheduled_fallback_count": sum(
            value.get("fallback") is True for value in scheduled
        ),
        "scheduled_status_counts": dict(sorted(statuses.items())),
        "self_change_event_count": len(lifecycle),
        "lifecycle_facet_counts": dict(sorted(facets.items())),
    }


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
    if (
        operator_database.get("quick_check") == "ok"
        and operator_database.get("schema_version") == DATABASE_SCHEMA_VERSION
        and operator_database.get("attribution_projection_version")
        == ATTRIBUTION_PROJECTION_VERSION
        and operator_database.get("spectral_tuning_projection_version")
        == SPECTRAL_TUNING_PROJECTION_VERSION
        and operator_database.get("self_evolution_projection_version")
        == SELF_EVOLUTION_PROJECTION_VERSION
        and operator_database.get("inquiry_projection_version")
        == INQUIRY_PROJECTION_VERSION
    ):
        database_view = query_hindsight_database(
            database_path, start_ms, end_ms, args.limit
        )

    activity_module = load_activity_module()
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
        spectral_receipts = list(database_view["spectral_receipts"])
        tuning_events = list(database_view["tuning_events"])
    else:
        spectral_records = [
            value
            for value in json_lines(workspace / "spectral/rollups.jsonl")
            if start_ms
            <= int(value.get("recorded_at_unix_ms", 0) or 0)
            <= end_ms
        ]
        spectral_receipts = [
            value
            for value in json_lines(workspace / "spectral/receipts.jsonl")
            if start_ms
            <= int(value.get("recorded_at_unix_ms", 0) or 0)
            <= end_ms
        ][-args.limit :]

    if database_view is not None:
        scheduled_reflections = list(database_view["scheduled_reflections"])
        self_change_events = list(database_view["self_change_events"])
        scheduled_reflection_count = int(
            database_view["scheduled_reflection_count"]
        )
        self_change_event_count = int(database_view["self_change_event_count"])
        inquiry_view = dict(database_view.get("inquiry") or {})
        inquiry_counts = dict(database_view.get("inquiry_counts") or {})
    else:
        observed_events = activity_events(workspace, current_ms, activity_module)
        all_scheduled, all_self_change = self_evolution_projections(observed_events)
        scheduled_reflections = [
            value
            for value in all_scheduled
            if start_ms <= int(value["timestamp_unix_ms"]) <= end_ms
        ][-args.limit :]
        self_change_events = [
            value
            for value in all_self_change
            if start_ms <= int(value["timestamp_unix_ms"]) <= end_ms
        ][-args.limit :]
        scheduled_reflection_count = len(scheduled_reflections)
        self_change_event_count = len(self_change_events)
        inquiry_view = {
            "steps": [value for value in activities if value.get("kind") == "inquiry_step"],
            "evidence": [value for value in activities if value.get("kind") == "evidence_arrival"],
            "belief_revisions": [value for value in activities if value.get("kind") == "belief_revision"],
            "thread_transitions": [value for value in activities if value.get("kind") == "thread_transition"],
            "semantic_admissions": [value for value in activities if value.get("kind") == "semantic_admission"],
            "integrity_violations": [value for value in activities if value.get("kind") == "integrity_violation"],
        }
        inquiry_counts = {key: len(value) for key, value in inquiry_view.items()}
        tuning_events = [
            flatten_signed_tuning(value)
            for value in json_lines(workspace / "tuning/receipts.jsonl")
            if start_ms
            <= int(
                (
                    value.get("payload", {}).get("recorded_at_unix_ms", 0)
                    if isinstance(value.get("payload"), dict)
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
    current_ledger_syntax = {
        relative: ledger_summary(path)
        for relative, path in checkpoint_ledger_paths(workspace).items()
    }
    current_ledger_syntax_issues = {
        relative: {
            "invalid_json_lines": int(summary.get("invalid_json_lines", 0) or 0),
            "trailing_partial_bytes": int(
                summary.get("trailing_partial_bytes", 0) or 0
            ),
            "snapshot_unread_bytes": int(
                summary.get("snapshot_unread_bytes", 0) or 0
            ),
        }
        for relative, summary in current_ledger_syntax.items()
        if summary.get("present")
        and (
            int(summary.get("invalid_json_lines", 0) or 0) > 0
            or int(summary.get("trailing_partial_bytes", 0) or 0) > 0
            or int(summary.get("snapshot_unread_bytes", 0) or 0) > 0
        )
    }
    current_ledger_syntax_valid = not current_ledger_syntax_issues
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
    pending_tail_observations = int(
        latest.get("pending_tail_observation_count", 0) or 0
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
            "current_ledger_syntax_valid": current_ledger_syntax_valid,
            "current_ledger_syntax_issues": current_ledger_syntax_issues,
            "checkpointed_ledger_prefixes": prefix,
            "checkpointed_ledger_prefixes_valid": prefix_ok,
            "continuity_epoch": latest.get("continuity_epoch"),
            "continuity_status": latest.get("continuity_status"),
            "checkpoint_to_checkpoint_continuity_valid": checkpoint_continuity_valid,
            "historical_ledger_integrity_violation_count": historical_violations,
            "legacy_race_compatible_unresolved_violation_count": legacy_violations,
            "current_epoch_integrity_violation_count": epoch_violations,
            "pending_tail_observation_count": pending_tail_observations,
            "overall_valid": bool(latest)
            and chain_integrity_valid
            and current_ledger_syntax_valid
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
            "spectral_receipt_count_in_range": len(spectral_receipts),
            "tuning_event_count_in_range": len(tuning_events),
            "scheduled_reflection_count_in_range": scheduled_reflection_count,
            "self_change_event_count_in_range": self_change_event_count,
            "inquiry_counts_in_range": inquiry_counts,
            "state_database": latest.get("state_database") or database_inventory(state_root / "var/state.db"),
            "audit_database": latest.get("audit_database") or database_inventory(state_root / "home/default/.local/audit"),
            "operator_hindsight_database": operator_database,
        },
        "fill": {"summary": fill_summary(fill_records), "rollups": fill_records},
        "spectral": {
            "summary": spectral_summary(spectral_records),
            "rollups": spectral_records,
            "receipts": spectral_receipts,
            "tuning_events": tuning_events,
            "authority": "deterministic_machine_derivation_not_authorship_or_causal_proof",
        },
        "self_evolution": {
            "summary": self_evolution_summary(
                scheduled_reflections, self_change_events
            ),
            "scheduled_reflections": scheduled_reflections,
            "lifecycle_events": self_change_events,
            "authority": (
                "metadata_hashes_and_provenance_only_no_prompt_response_source_diff_or_logs"
            ),
        },
        "inquiry": {
            **inquiry_view,
            "counts": inquiry_counts,
            "authority": (
                "signed_authored_intellectual_record_and_typed_evidence_not_hidden_chain_of_thought_or_code_authority"
            ),
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
        f"Checkpoint: {'present' if integrity['checkpoint_present'] else 'missing'}; age={integrity['checkpoint_age_seconds']}s; overall_valid={str(integrity['overall_valid']).lower()}; ledger_prefixes_valid={str(integrity['checkpointed_ledger_prefixes_valid']).lower()}; epoch={integrity.get('continuity_epoch')}; status={integrity.get('continuity_status')}; current_epoch_violations={integrity['current_epoch_integrity_violation_count']}; pending_tail_observations={integrity.get('pending_tail_observation_count', 0)}; legacy_race_compatible_unresolved={integrity['legacy_race_compatible_unresolved_violation_count']}; historical_raw={integrity['historical_ledger_integrity_violation_count']}",
        "Chains: " + ", ".join(f"{name}={'valid' if value['valid'] else 'INVALID'}({value['records']})" for name, value in integrity["chains"].items()),
        f"Query source={sources['historical_query_source']}; activity events={sources['activity_event_count_in_range']} (showing {sources['activity_events_returned']}); artifact files={sources['artifact_file_count_in_range']} (showing {sources['artifact_files_returned']}); fill rollups={sources['fill_rollup_count_in_range']}",
        f"Spectral rollups={sources.get('spectral_rollup_count_in_range', 0)}; spectral receipts={sources.get('spectral_receipt_count_in_range', 0)}; tuning lifecycle events={sources.get('tuning_event_count_in_range', 0)}",
        f"Scheduled reflections={sources.get('scheduled_reflection_count_in_range', 0)}; self-change lifecycle events={sources.get('self_change_event_count_in_range', 0)}",
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
    for event in spectral.get("receipts", []):
        lines.append(
            f"{iso_time(int(event.get('recorded_at_unix_ms', 0) or 0))} "
            f"SPECTRAL_RECEIPT phase={event.get('phase') or event.get('kind')} "
            f"status={event.get('status')} event={event.get('event_kind')} "
            f"turn={str((event.get('trace') or {}).get('turn_id') or event.get('turn_id') or '-')[:8]}"
        )
    for event in spectral.get("tuning_events", []):
        lines.append(
            f"{iso_time(int(event.get('recorded_at_unix_ms', 0) or 0))} "
            f"TUNING phase={event.get('phase') or event.get('kind')} "
            f"status={event.get('status')} id={event.get('tuning_id') or event.get('experiment_id')} "
            f"candidate={event.get('candidate_id')} parameter={event.get('parameter')}"
        )
    evolution = report.get("self_evolution", {})
    evolution_summary = evolution.get("summary", {})
    lines.extend(["", "## Scheduled reflection and self-change"])
    lines.append(
        "scheduled={} authored={} fallback={} lifecycle_facets={} authority={}".format(
            evolution_summary.get("scheduled_reflection_count", 0),
            evolution_summary.get("scheduled_authored_count", 0),
            evolution_summary.get("scheduled_fallback_count", 0),
            evolution_summary.get("lifecycle_facet_counts", {}),
            evolution.get("authority"),
        )
    )
    for event in evolution.get("scheduled_reflections", []):
        lines.append(
            f"{iso_time(int(event.get('timestamp_unix_ms', 0) or 0))} "
            f"SCHEDULED_REFLECTION status={event.get('status')} "
            f"authored={str(event.get('authored')).lower()} "
            f"continuity={str(event.get('continuity_admitted')).lower()} "
            f"candidate={event.get('candidate_id')} "
            f"response={str(event.get('response_sha256') or '-')[:16]}"
        )
    for event in evolution.get("lifecycle_events", []):
        lines.append(
            f"{iso_time(int(event.get('timestamp_unix_ms', 0) or 0))} "
            f"SELF_CHANGE facets={','.join(event.get('lifecycle_facets') or []) or '-'} "
            f"status={event.get('status')} candidate={event.get('candidate_id')} "
            f"build={event.get('build_id')} generation={event.get('generation_id')} "
            f"shadow_gate={event.get('shadow_gate_evidence') or '-'}"
        )
    inquiry = report.get("inquiry", {})
    lines.extend(["", "## Authored inquiry train"])
    lines.append(
        f"counts={inquiry.get('counts', {})} authority={inquiry.get('authority')}"
    )
    for event in inquiry.get("steps", []):
        lines.append(
            f"{iso_time(int(event.get('timestamp_unix_ms', 0) or 0))} "
            f"INQUIRY_STEP step={short(event.get('step_id'), 48)} "
            f"thread={short(event.get('thread_id'), 48)} "
            f"operation={event.get('thread_operation')} confidence={event.get('confidence')} "
            f"decision={short(event.get('decision'), 180)}"
        )
    for event in inquiry.get("evidence", []):
        lines.append(
            f"{iso_time(int(event.get('timestamp_unix_ms', 0) or 0))} "
            f"EVIDENCE id={short(event.get('evidence_id'), 48)} "
            f"kind={event.get('evidence_kind')} eligible={str(event.get('eligible_for_belief_update')).lower()}"
        )
    for event in inquiry.get("belief_revisions", []):
        lines.append(
            f"{iso_time(int(event.get('timestamp_unix_ms', 0) or 0))} "
            f"BELIEF revision={short(event.get('revision_id'), 48)} "
            f"belief={short(event.get('belief_id'), 48)} operation={event.get('operation')}"
        )
    for event in inquiry.get("semantic_admissions", []):
        lines.append(
            f"{iso_time(int(event.get('timestamp_unix_ms', 0) or 0))} "
            f"SEMANTIC_ADMISSION id={short(event.get('admission_id'), 48)} "
            f"delivery={event.get('status')} generation={short(event.get('reservoir_generation'), 40)}"
        )
    for event in inquiry.get("integrity_violations", []):
        lines.append(
            f"{iso_time(int(event.get('timestamp_unix_ms', 0) or 0))} "
            f"TRAIN_INTEGRITY_INVALID path={short(event.get('path'), 120)} "
            f"reason={short(event.get('reason'), 180)} no_authorship_claim=true"
        )
    lines.extend(["", "## Causal activity"])
    for event in report["activity"]:
        timestamp = iso_time(int(event.get("timestamp_unix_ms", 0) or 0))
        kind = str(event.get("kind", "unknown")).upper()
        authored = event.get("authored")
        detail = event.get("declared_next") or event.get("query") or event.get("url") or event.get("summary") or event.get("reason") or event.get("status")
        lines.append(
            f"{timestamp} {kind} authored={str(authored).lower() if authored is not None else '-'} trace={str(event.get('trace_id') or 'legacy')[:8]} turn={str(event.get('turn_id') or '-')[:8]} {short(detail, 180)}"
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
    return "\n".join(terminal_safe_text(line) for line in lines)


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    subparsers = result.add_subparsers(dest="command")
    record_parser = subparsers.add_parser("record", help="append an owner-only integrity checkpoint")
    sealed_parser = subparsers.add_parser(
        "record-sealed", help="append through one immutable root-owned writer binding"
    )
    sealed_parser.add_argument("--config", type=Path, required=True)
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
    if not arguments or arguments[0] not in {
        "record",
        "record-sealed",
        "report",
        "-h",
        "--help",
    }:
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
                terminal_safe_text(
                    f"hindsight checkpoint recorded: artifacts={output['artifacts_written']} "
                    f"fill_rollups={output['fill_rollups_written']} root={output['operator_root']}"
                )
            )
        return 0
    if args.command == "record-sealed":
        try:
            output = record_sealed(args.config)
        except (OSError, ValueError) as error:
            raise SystemExit(str(error)) from error
        print(
            terminal_safe_text(
                f"sealed hindsight checkpoint recorded: artifacts={output['artifacts_written']} "
                f"fill_rollups={output['fill_rollups_written']}"
            )
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
