#!/usr/bin/env python3
"""Sealed, read-only viewer for Astrid's authored inquiry train.

The viewer reports an authored intellectual record.  It does not expose or
claim hidden provider chain-of-thought.  Its preferred source is the immutable
steward's signed, hash-chained ledger.  Owner-visible signed reflection
attestations are accepted only as an explicitly degraded, individually
attested view when the mechanical ledger cannot be read or verified.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import os
import re
import stat
import sys
import time
import unicodedata
import uuid
from pathlib import Path
from typing import Any, Iterable

REPORT_SCHEMA = "astrid_edge_inquiry_train_report_v1"
ENTRY_SCHEMA = "astrid.edge.inquiry.entry.v1"
ENTRY_ENVELOPE_SCHEMA = "astrid.edge.inquiry.entry_envelope.v1"
HEAD_SCHEMA = "astrid.edge.inquiry.head.v1"
HEAD_ENVELOPE_SCHEMA = "astrid.edge.inquiry.head_envelope.v1"
CURRENT_SCHEMA = "astrid.edge.inquiry.current.v1"
AUTHORSHIP_CORE_SCHEMA = "astrid.edge.scheduled_authorship.attestation.v2"
AUTHORSHIP_ENVELOPE_SCHEMA = (
    "astrid.edge.scheduled_authorship.attestation_envelope.v2"
)
ADMISSION_SCHEMA = "astrid.edge.inquiry.semantic_admission.v2"
LEGACY_ADMISSION_SCHEMA = "astrid.edge.scheduled_introspection.admission.v1"
ADMISSION_RECEIPT_SCHEMA = "astrid.edge.inquiry.semantic_admission_receipt.v1"
ADMISSION_RECEIPT_MAX_BYTES = 256 * 1024 * 1024
ADMISSION_RECEIPT_LINE_MAX_BYTES = 32 * 1024
VERIFY_KEY = Path("/etc/astrid/edge-scheduled-authorship.pub")
GENESIS_HASH = "0" * 64
SEGMENT_MAX_BYTES = 4 * 1024 * 1024
ENTRY_MAX_BYTES = 32 * 1024
REFLECTION_MAX_BYTES = 64 * 1024
PROJECTION_RECORD_MAX_BYTES = 512 * 1024
MAX_SEGMENTS = 4_096
MAX_ENTRIES = 100_000
HEX64 = re.compile(r"[0-9a-f]{64}\Z")
SAFE_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,95}\Z")
ENTRY_ID_DOMAIN = b"astrid.edge.inquiry.entry-id.v1\0"
ADMISSION_ID_DOMAIN = b"astrid.edge.inquiry.admission.v1\0"
STEP_ID_DOMAIN = b"astrid.edge.inquiry.step-id.v1\0"
KINDS = frozenset(
    {
        "belief_revision",
        "clean_source_review",
        "evidence_arrival",
        "integrity_violation",
        "inquiry_step",
        "legacy_reflection",
        "model_tool_request",
        "scheduled_reflection",
        "semantic_admission",
        "thread_transition",
    }
)


class TrainError(RuntimeError):
    """The protected inquiry surface failed a read or integrity invariant."""

    def __init__(self, message: str, *, path: Path | None = None) -> None:
        super().__init__(message)
        self.path = path


def canonical(value: Any) -> bytes:
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    ).encode("utf-8")


def sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def terminal_safe(value: Any) -> str:
    return "".join(
        " "
        if unicodedata.category(character) in {"Cc", "Cf", "Cs", "Zl", "Zp"}
        else character
        for character in str(value)
    )


def compact(value: Any, maximum: int = 160) -> str:
    if value in (None, ""):
        return "-"
    text = " ".join(terminal_safe(value).split())
    return text if len(text) <= maximum else f"{text[: maximum - 1]}…"


def iso_time(timestamp_ms: int) -> str:
    return dt.datetime.fromtimestamp(
        timestamp_ms / 1_000, tz=dt.timezone.utc
    ).isoformat(timespec="milliseconds").replace("+00:00", "Z")


def parse_time(value: str) -> int:
    text = value.strip()
    try:
        number = int(text)
    except ValueError:
        parsed = dt.datetime.fromisoformat(text.replace("Z", "+00:00"))
        if parsed.tzinfo is None:
            parsed = parsed.replace(tzinfo=dt.timezone.utc)
        return int(parsed.timestamp() * 1_000)
    return number if number > 10_000_000_000 else number * 1_000


def stable_regular(
    path: Path,
    maximum: int,
    *,
    private: bool = False,
) -> bytes:
    try:
        before = path.lstat()
    except OSError as error:
        raise TrainError(f"cannot inspect protected file: {error}", path=path) from error
    if (
        path.is_symlink()
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size < 0
        or before.st_size > maximum
        or before.st_mode & 0o022
        or (private and before.st_mode & 0o007)
    ):
        raise TrainError("unsafe protected file identity", path=path)
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise TrainError(f"cannot open protected file: {error}", path=path) from error
    try:
        opened = os.fstat(descriptor)
        chunks: list[bytes] = []
        remaining = maximum + 1
        while remaining:
            block = os.read(descriptor, min(1024 * 1024, remaining))
            if not block:
                break
            chunks.append(block)
            remaining -= len(block)
        after = os.fstat(descriptor)
        try:
            final_path = path.lstat()
        except OSError as error:
            raise TrainError(
                f"protected path disappeared while open: {error}", path=path
            ) from error
    finally:
        os.close(descriptor)
    identity = lambda item: (
        item.st_dev,
        item.st_ino,
        item.st_mode,
        item.st_nlink,
        item.st_uid,
        item.st_gid,
        item.st_size,
        item.st_mtime_ns,
        item.st_ctime_ns,
    )
    data = b"".join(chunks)
    if (
        identity(before) != identity(opened)
        or identity(opened) != identity(after)
        or identity(after) != identity(final_path)
        or stat.S_ISLNK(final_path.st_mode)
        or len(data) != before.st_size
        or len(data) > maximum
    ):
        raise TrainError("protected file changed or was replaced while read", path=path)
    return data


def exact_scheduled_artifact_path(value: Any, suffix: str) -> Path | None:
    """Accept only the canonical owned scheduled-artifact spelling."""
    if not isinstance(value, str):
        return None
    prefix = "introspections/scheduled/"
    if not value.startswith(prefix):
        return None
    basename = value.removeprefix(prefix)
    if not safe_id(basename) or Path(basename).suffix != suffix:
        return None
    relative = Path(value)
    if relative.parts != ("introspections", "scheduled", basename):
        return None
    return relative


def _directory_identity(value: os.stat_result) -> tuple[int, ...]:
    return (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_nlink,
        value.st_uid,
        value.st_gid,
    )


def _file_identity(value: os.stat_result) -> tuple[int, ...]:
    return (
        *_directory_identity(value),
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )


def stable_exact_workspace_artifact(
    workspace: Path,
    relative: Path,
    maximum: int,
    *,
    private: bool = False,
    required_mode: int | None = None,
    private_parent: bool = False,
) -> bytes:
    """Read one exact owned artifact through a descriptor-anchored walk."""
    if (
        relative.is_absolute()
        or len(relative.parts) < 2
        or any(not safe_id(component) for component in relative.parts)
    ):
        raise TrainError("workspace artifact path is not exact and component-safe")
    try:
        canonical_workspace = workspace.resolve(strict=True)
    except OSError as error:
        raise TrainError(
            f"cannot resolve verified workspace root: {error}", path=workspace
        ) from error
    directory_flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0)
    )
    file_flags = (
        os.O_RDONLY
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0)
    )
    descriptors: list[int] = []
    directory_links: list[tuple[int, str, tuple[int, ...]]] = []
    file_descriptor: int | None = None
    try:
        descriptors.append(os.open("/", directory_flags))
        components = (*canonical_workspace.parts[1:], *relative.parts[:-1])
        for component in components:
            parent_descriptor = descriptors[-1]
            child_descriptor = os.open(
                component, directory_flags, dir_fd=parent_descriptor
            )
            descriptors.append(child_descriptor)
            opened = os.fstat(child_descriptor)
            linked = os.stat(
                component, dir_fd=parent_descriptor, follow_symlinks=False
            )
            if (
                not stat.S_ISDIR(opened.st_mode)
                or not stat.S_ISDIR(linked.st_mode)
                or _directory_identity(opened) != _directory_identity(linked)
            ):
                raise TrainError(
                    "scheduled artifact directory identity is unsafe",
                    path=workspace / relative,
                )
            directory_links.append(
                (parent_descriptor, component, _directory_identity(opened))
            )

        parent_descriptor = descriptors[-1]
        scheduled_directory = os.fstat(parent_descriptor)
        if private_parent and scheduled_directory.st_mode & 0o077:
            raise TrainError(
                "workspace artifact parent is not owner-private",
                path=workspace / relative,
            )
        before = os.stat(
            relative.name, dir_fd=parent_descriptor, follow_symlinks=False
        )
        file_descriptor = os.open(
            relative.name, file_flags, dir_fd=parent_descriptor
        )
        opened = os.fstat(file_descriptor)
        if (
            not stat.S_ISREG(before.st_mode)
            or not stat.S_ISREG(opened.st_mode)
            or before.st_nlink != 1
            or opened.st_nlink != 1
            or before.st_size < 0
            or before.st_size > maximum
            or before.st_mode & 0o022
            or (private and before.st_mode & 0o007)
            or (
                required_mode is not None
                and before.st_mode & 0o777 != required_mode
            )
            or (
                private
                and (
                    before.st_uid != scheduled_directory.st_uid
                    or before.st_gid != scheduled_directory.st_gid
                )
            )
            or _file_identity(before) != _file_identity(opened)
        ):
            raise TrainError(
                "unsafe scheduled artifact identity", path=workspace / relative
            )
        chunks: list[bytes] = []
        remaining = maximum + 1
        while remaining:
            block = os.read(file_descriptor, min(1024 * 1024, remaining))
            if not block:
                break
            chunks.append(block)
            remaining -= len(block)
        after = os.fstat(file_descriptor)
        final_link = os.stat(
            relative.name, dir_fd=parent_descriptor, follow_symlinks=False
        )
        data = b"".join(chunks)
        if (
            _file_identity(opened) != _file_identity(after)
            or _file_identity(after) != _file_identity(final_link)
            or len(data) != opened.st_size
            or len(data) > maximum
        ):
            raise TrainError(
                "scheduled artifact changed or was replaced while read",
                path=workspace / relative,
            )
        for parent, component, identity in directory_links:
            linked = os.stat(component, dir_fd=parent, follow_symlinks=False)
            if (
                not stat.S_ISDIR(linked.st_mode)
                or _directory_identity(linked) != identity
            ):
                raise TrainError(
                    "scheduled artifact directory changed or was replaced while read",
                    path=workspace / relative,
                )
        try:
            final_workspace = workspace.resolve(strict=True)
        except OSError as error:
            raise TrainError(
                f"workspace root changed while reading scheduled artifact: {error}",
                path=workspace,
            ) from error
        if final_workspace != canonical_workspace:
            raise TrainError(
                "workspace root changed while reading scheduled artifact",
                path=workspace,
            )
        return data
    except TrainError:
        raise
    except OSError as error:
        raise TrainError(
            f"cannot read descriptor-anchored scheduled artifact: {error}",
            path=workspace / relative,
        ) from error
    finally:
        if file_descriptor is not None:
            os.close(file_descriptor)
        for descriptor in reversed(descriptors):
            os.close(descriptor)


def stable_scheduled_artifact(
    workspace: Path,
    relative_value: Any,
    suffix: str,
    maximum: int,
    *,
    private: bool = False,
) -> bytes:
    """Read one exact owned scheduled artifact without following path links."""
    relative = exact_scheduled_artifact_path(relative_value, suffix)
    if relative is None:
        raise TrainError("scheduled artifact path is not exact and basename-safe")
    return stable_exact_workspace_artifact(
        workspace, relative, maximum, private=private
    )


# Dependency-free RFC 8032 verification for stock Ubuntu Python.
_P = 2**255 - 19
_L = 2**252 + 27742317777372353535851937790883648493
_D = (-121665 * pow(121666, _P - 2, _P)) % _P
_I = pow(2, (_P - 1) // 4, _P)
_IDENTITY = (0, 1, 1, 0)


def _recover_x(y: int, sign: int) -> int | None:
    if y >= _P:
        return None
    y2 = y * y % _P
    x2 = (y2 - 1) * pow((_D * y2 + 1) % _P, _P - 2, _P) % _P
    x = pow(x2, (_P + 3) // 8, _P)
    if (x * x - x2) % _P:
        x = x * _I % _P
    if (x * x - x2) % _P:
        return None
    if x & 1 != sign:
        x = _P - x
    return None if x == 0 and sign else x


def _decode(encoded: bytes) -> tuple[int, int, int, int] | None:
    if len(encoded) != 32:
        return None
    value = int.from_bytes(encoded, "little")
    y = value & ((1 << 255) - 1)
    x = _recover_x(y, value >> 255)
    return None if x is None else (x, y, 1, x * y % _P)


def _add(
    left: tuple[int, int, int, int], right: tuple[int, int, int, int]
) -> tuple[int, int, int, int]:
    x1, y1, z1, t1 = left
    x2, y2, z2, t2 = right
    a = (y1 - x1) * (y2 - x2) % _P
    b = (y1 + x1) * (y2 + x2) % _P
    c = 2 * _D * t1 * t2 % _P
    d = 2 * z1 * z2 % _P
    e, f, g, h = b - a, d - c, d + c, b + a
    return e * f % _P, g * h % _P, f * g % _P, e * h % _P


def _double(point: tuple[int, int, int, int]) -> tuple[int, int, int, int]:
    x, y, z, _ = point
    a, b, c = x * x % _P, y * y % _P, 2 * z * z % _P
    d = -a % _P
    e = ((x + y) * (x + y) - a - b) % _P
    g, f, h = (d + b) % _P, (d + b - c) % _P, (d - b) % _P
    return e * f % _P, g * h % _P, f * g % _P, e * h % _P


def _scalar(
    point: tuple[int, int, int, int], scalar: int
) -> tuple[int, int, int, int]:
    result, current = _IDENTITY, point
    while scalar:
        if scalar & 1:
            result = _add(result, current)
        current = _double(current)
        scalar >>= 1
    return result


def _equal(
    left: tuple[int, int, int, int], right: tuple[int, int, int, int]
) -> bool:
    return (
        (left[0] * right[2] - right[0] * left[2]) % _P == 0
        and (left[1] * right[2] - right[1] * left[2]) % _P == 0
    )


_BASE_Y = 4 * pow(5, _P - 2, _P) % _P
_BASE_X = _recover_x(_BASE_Y, 0)
if _BASE_X is None:  # pragma: no cover
    raise RuntimeError("invalid Ed25519 constants")
_BASE = (_BASE_X, _BASE_Y, 1, _BASE_X * _BASE_Y % _P)


def verify_ed25519(public_key: bytes, message: bytes, signature: bytes) -> bool:
    if len(public_key) != 32 or len(signature) != 64:
        return False
    public, encoded_r = _decode(public_key), signature[:32]
    r_point = _decode(encoded_r)
    scalar = int.from_bytes(signature[32:], "little")
    if public is None or r_point is None or scalar >= _L:
        return False
    if _equal(_scalar(public, 8), _IDENTITY) or _equal(
        _scalar(r_point, 8), _IDENTITY
    ):
        return False
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public_key + message).digest(), "little"
    ) % _L
    return _equal(_scalar(_BASE, scalar), _add(r_point, _scalar(public, challenge)))


def expected_identity(workspace: Path) -> tuple[str, Path]:
    resolved = workspace.resolve(strict=False)
    text = resolved.as_posix()
    if text == "/home/avado/.astrid/home/default/edge":
        return "avado-edge", Path("/var/lib/astrid-edge-inquiry-history")
    if text in {
        "/media/data/astrid/state/home/default/edge",
        "/home/nativeplanet/.astrid-icp/state/home/default/edge",
    }:
        return "icp-edge", Path("/media/data/astrid-edge-inquiry-history")
    raise TrainError("workspace is outside the sealed AVADO/ICP profile map")


def load_key(path: Path) -> tuple[bytes, str]:
    key = stable_regular(path, 32)
    if len(key) != 32:
        raise TrainError("scheduled-authorship public key is not 32 bytes")
    return key, f"ed25519:{sha256(key)[:16]}"


def safe_id(value: Any) -> bool:
    return isinstance(value, str) and SAFE_ID.fullmatch(value) is not None


def exact_sha(value: Any) -> bool:
    return isinstance(value, str) and HEX64.fullmatch(value) is not None


def derive_id(domain: bytes, appliance: str, signed_entry_id: str) -> str:
    return hashlib.sha256(
        domain + appliance.encode() + b"\0" + signed_entry_id.encode()
    ).hexdigest()


def derive_entry_id(core: dict[str, Any]) -> str:
    fields = (
        "appliance_id",
        "trigger_kind",
        "trigger_nonce",
    )
    trace = core.get("trace") if isinstance(core.get("trace"), dict) else {}
    values = [
        *(str(core.get(field, "")) for field in fields),
        str(trace.get("trace_id", "")),
        str(trace.get("turn_id", "")),
        str(core.get("response_sha256", "")),
        str(core.get("declaration_sha256", "")),
    ]
    return "inquiry-entry-" + sha256(
        ENTRY_ID_DOMAIN + b"\0".join(value.encode() for value in values)
    )


def authored_provenance(trigger_kind: Any) -> str | None:
    return {
        "scheduled": "model_authored_runtime_scheduled",
        "evidence_integration": "model_authored_runtime_evidence_integration",
    }.get(trigger_kind)


def validate_step(step: Any) -> None:
    required = {
        "schema",
        "thread_operation",
        "thread_id",
        "parent_step_id",
        "observation",
        "interpretation",
        "uncertainty",
        "decision",
        "counterpoint",
        "next_test",
        "evidence_ids",
        "confidence",
        "belief_operation",
        "belief_id",
        "belief_claim",
    }
    if not isinstance(step, dict) or set(step) != required:
        raise TrainError("inquiry step has an unexpected shape")
    if step.get("schema") != "astrid.edge.inquiry.step.v1":
        raise TrainError("inquiry step schema is unsupported")
    operation = step.get("thread_operation")
    if operation not in {"continue", "open", "branch", "pause", "close"}:
        raise TrainError("inquiry thread operation is unsupported")
    if not safe_id(step.get("thread_id")):
        raise TrainError("inquiry thread ID is invalid")
    parent = step.get("parent_step_id")
    if (operation == "open") != (parent is None) or (parent is not None and not safe_id(parent)):
        raise TrainError("inquiry semantic parent is inconsistent")
    for field, maximum in (
        ("observation", 480),
        ("interpretation", 480),
        ("uncertainty", 320),
        ("decision", 480),
    ):
        text = step.get(field)
        if (
            not isinstance(text, str)
            or not text
            or text.strip() != text
            or len(text) > maximum
            or any(unicodedata.category(character) == "Cc" for character in text)
        ):
            raise TrainError(f"inquiry {field} is invalid")
    for field in ("counterpoint", "next_test"):
        text = step.get(field)
        if text is not None and (
            not isinstance(text, str) or not text.strip() or len(text) > 320
            or text.strip() != text
            or any(unicodedata.category(character) == "Cc" for character in text)
        ):
            raise TrainError(f"inquiry {field} is invalid")
    evidence = step.get("evidence_ids")
    if (
        not isinstance(evidence, list)
        or len(evidence) > 6
        or len(set(evidence)) != len(evidence)
        or any(not safe_id(item) for item in evidence)
    ):
        raise TrainError("inquiry evidence identifiers are invalid")
    if step.get("confidence") not in {"tentative", "moderate", "strong"}:
        raise TrainError("inquiry confidence is invalid")
    belief = step.get("belief_operation")
    if belief not in {
        None,
        "unchanged",
        "propose",
        "support",
        "weaken",
        "revise",
        "suspend",
        "resolve",
    }:
        raise TrainError("inquiry belief operation is invalid")
    belief_id, belief_claim = step.get("belief_id"), step.get("belief_claim")
    if belief in {None, "unchanged"}:
        if belief_id is not None or belief_claim is not None:
            raise TrainError("non-mutating belief operation carries belief fields")
    elif (
        not safe_id(belief_id)
        or not isinstance(belief_claim, str)
        or not belief_claim
        or belief_claim.strip() != belief_claim
        or len(belief_claim) > 480
        or any(
            unicodedata.category(character) == "Cc" for character in belief_claim
        )
    ):
        raise TrainError("mutating belief operation lacks bounded belief fields")


def inquiry_summary(step: dict[str, Any]) -> str:
    value = (
        f"Observed: {step['observation']} Interpreted: {step['interpretation']} "
        f"Uncertain: {step['uncertainty']} Decided: {step['decision']}"
    )
    compacted = " ".join(value.split())
    return compacted[:320]


def validate_entry(
    envelope: Any,
    appliance: str,
    public_key: bytes,
    key_id: str,
    prior_hash: str,
    prior_entry_id: str,
) -> tuple[dict[str, Any], str]:
    entry_fields = {
        "schema",
        "appliance_id",
        "signed_entry_id",
        "step_id",
        "admission_id",
        "recorded_at_unix_ms",
        "trigger_kind",
        "due_nonce",
        "trigger_nonce",
        "trace",
        "prompt_sha256",
        "response_sha256",
        "context_provenance_sha256",
        "reflection_path",
        "reflection_sha256",
        "declaration",
        "declaration_sha256",
        "inquiry_step",
        "inquiry_step_sha256",
        "summary",
        "summary_sha256",
        "prior_entry_sha256",
        "mechanical_predecessor",
        "semantic_parent_step_id",
        "provenance",
        "authority",
    }
    if not isinstance(envelope, dict) or set(envelope) != {
        "schema",
        "core",
        "core_sha256",
        "auth",
    }:
        raise TrainError("inquiry entry envelope shape is invalid")
    core, auth = envelope.get("core"), envelope.get("auth")
    if (
        not isinstance(core, dict)
        or set(core) != entry_fields
        or not isinstance(auth, dict)
        or set(auth) != {"algorithm", "key_id", "signature"}
    ):
        raise TrainError("inquiry entry envelope is incomplete")
    core_bytes = canonical(core)
    try:
        signature = bytes.fromhex(str(auth.get("signature", "")))
    except ValueError as error:
        raise TrainError("inquiry signature encoding is invalid") from error
    if (
        envelope.get("schema") != ENTRY_ENVELOPE_SCHEMA
        or core.get("schema") != ENTRY_SCHEMA
        or core.get("appliance_id") != appliance
        or envelope.get("core_sha256") != sha256(core_bytes)
        or auth.get("algorithm") != "ed25519"
        or auth.get("key_id") != key_id
        or not verify_ed25519(public_key, core_bytes, signature)
        or core.get("prior_entry_sha256") != prior_hash
        or core.get("mechanical_predecessor") != prior_entry_id
    ):
        raise TrainError("inquiry entry signature, identity, or predecessor failed")
    trace = core.get("trace")
    if (
        not isinstance(trace, dict)
        or set(trace)
        != {"schema_version", "trace_id", "turn_id", "span_id", "session_id"}
        or trace.get("schema_version") != 1
        or any(
            not safe_id(trace.get(field))
            for field in ("trace_id", "turn_id", "span_id", "session_id")
        )
    ):
        raise TrainError("inquiry trace context is invalid")
    step = core.get("inquiry_step")
    validate_step(step)
    signed_entry_id = core.get("signed_entry_id")
    relative = exact_scheduled_artifact_path(core.get("reflection_path"), ".md")
    declaration = core.get("declaration")
    summary = inquiry_summary(step)
    declaration_step: Any = None
    if isinstance(declaration, str) and declaration.startswith("INQUIRY_STEP: "):
        try:
            declaration_step = json.loads(declaration.removeprefix("INQUIRY_STEP: "))
        except json.JSONDecodeError:
            declaration_step = None
    if (
        not safe_id(signed_entry_id)
        or signed_entry_id != derive_entry_id(core)
        or core.get("step_id")
        != "inquiry-step-" + derive_id(STEP_ID_DOMAIN, appliance, signed_entry_id)
        or core.get("admission_id")
        != "inquiry-admission-" + derive_id(ADMISSION_ID_DOMAIN, appliance, signed_entry_id)
        or core.get("inquiry_step_sha256") != sha256(canonical(step))
        or not isinstance(declaration, str)
        or declaration_step != step
        or core.get("declaration_sha256")
        != sha256(declaration.encode())
        or core.get("reflection_sha256") != core.get("response_sha256")
        or any(
            not exact_sha(core.get(field))
            for field in (
                "prompt_sha256",
                "response_sha256",
                "context_provenance_sha256",
                "reflection_sha256",
                "declaration_sha256",
                "inquiry_step_sha256",
                "summary_sha256",
                "prior_entry_sha256",
            )
        )
        or core.get("summary") != summary
        or core.get("summary_sha256") != sha256(summary.encode())
        or core.get("semantic_parent_step_id") != step.get("parent_step_id")
        or core.get("trigger_kind") not in {"scheduled", "evidence_integration"}
        or not safe_id(core.get("due_nonce"))
        or not safe_id(core.get("trigger_nonce"))
        or not isinstance(core.get("recorded_at_unix_ms"), int)
        or isinstance(core.get("recorded_at_unix_ms"), bool)
        or core.get("recorded_at_unix_ms") <= 0
        or relative is None
        or core.get("provenance") != authored_provenance(core.get("trigger_kind"))
        or core.get("authority")
        != "signed_authored_inquiry_not_hidden_chain_of_thought_not_code_authority"
    ):
        raise TrainError("inquiry entry content bindings failed")
    entry_hash = sha256(canonical(envelope))
    return core, entry_hash


def entry_event(
    core: dict[str, Any],
    entry_hash: str,
    integrity: str,
    workspace: Path,
    full: bool,
) -> dict[str, Any]:
    step = core["inquiry_step"]
    trace = core.get("trace") if isinstance(core.get("trace"), dict) else {}
    event: dict[str, Any] = {
        "timestamp_unix_ms": int(core.get("recorded_at_unix_ms", 0) or 0),
        "kind": "inquiry_step",
        "status": "verified",
        "authored": True,
        "fallback": False,
        "authorship_class": "astrid_authored_programmatic_inquiry_step",
        "provenance_class": "astrid_authored",
        "appliance_id": core.get("appliance_id"),
        "trigger_kind": core.get("trigger_kind"),
        "due_nonce": core.get("due_nonce"),
        "trigger_nonce": core.get("trigger_nonce"),
        "signed_entry_id": core.get("signed_entry_id"),
        "step_id": core.get("step_id"),
        "admission_id": core.get("admission_id"),
        "entry_sha256": entry_hash,
        "mechanical_predecessor": core.get("mechanical_predecessor"),
        "prior_entry_sha256": core.get("prior_entry_sha256"),
        "thread_id": step.get("thread_id"),
        "parent_step_id": step.get("parent_step_id"),
        "thread_operation": step.get("thread_operation"),
        "observation": step.get("observation"),
        "interpretation": step.get("interpretation"),
        "uncertainty": step.get("uncertainty"),
        "decision": step.get("decision"),
        "counterpoint": step.get("counterpoint"),
        "next_test": step.get("next_test"),
        "evidence_ids": step.get("evidence_ids"),
        "confidence": step.get("confidence"),
        "belief_operation": step.get("belief_operation"),
        "belief_id": step.get("belief_id"),
        "belief_claim": step.get("belief_claim"),
        "summary": core.get("summary"),
        "summary_sha256": core.get("summary_sha256"),
        "trace_id": trace.get("trace_id"),
        "turn_id": trace.get("turn_id"),
        "span_id": trace.get("span_id"),
        "session_id": trace.get("session_id"),
        "response_sha256": core.get("response_sha256"),
        "reflection_path": core.get("reflection_path"),
        "reflection_sha256": core.get("reflection_sha256"),
        "declaration_sha256": core.get("declaration_sha256"),
        "train_integrity": integrity,
        "authority": core.get("authority"),
        "source_ledger": "immutable-steward/inquiry/segments",
    }
    if full:
        relative = exact_scheduled_artifact_path(core.get("reflection_path"), ".md")
        if relative is None:
            raise TrainError("reflection path escaped the owned scheduled directory")
        prose = stable_scheduled_artifact(
            workspace,
            relative.as_posix(),
            ".md",
            REFLECTION_MAX_BYTES,
            private=True,
        )
        if sha256(prose) != core.get("reflection_sha256"):
            raise TrainError("exact reflection no longer matches the signed entry")
        try:
            event["reflection_text"] = prose.decode("utf-8", errors="strict")
        except UnicodeError as error:
            raise TrainError("exact reflection is not UTF-8") from error
        event["reflection_text_authority"] = "hash_verified_owner_private_exact_prose"
    return event


def validate_head(
    root: Path,
    appliance: str,
    public_key: bytes,
    key_id: str,
    count: int,
    segment: int,
    index: int,
    entry_id: str,
    entry_hash: str,
    segment_bytes: int,
) -> None:
    path = root / "head.json"
    try:
        value = json.loads(stable_regular(path, 16 * 1024))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise TrainError("inquiry head JSON is malformed", path=path) from error
    if not isinstance(value, dict) or set(value) != {"schema", "core", "core_sha256", "auth"}:
        raise TrainError("inquiry head shape is invalid", path=path)
    core, auth = value.get("core"), value.get("auth")
    if (
        not isinstance(core, dict)
        or set(core)
        != {
            "schema",
            "appliance_id",
            "entry_count",
            "segment",
            "entry_index",
            "signed_entry_id",
            "entry_sha256",
            "segment_bytes",
        }
        or not isinstance(auth, dict)
        or set(auth) != {"algorithm", "key_id", "signature"}
    ):
        raise TrainError("inquiry head is incomplete", path=path)
    core_bytes = canonical(core)
    try:
        signature = bytes.fromhex(str(auth.get("signature", "")))
    except ValueError as error:
        raise TrainError("inquiry head signature encoding is invalid", path=path) from error
    expected = (count, segment, index, entry_id, entry_hash, segment_bytes)
    actual = (
        core.get("entry_count"),
        core.get("segment"),
        core.get("entry_index"),
        core.get("signed_entry_id"),
        core.get("entry_sha256"),
        core.get("segment_bytes"),
    )
    if (
        value.get("schema") != HEAD_ENVELOPE_SCHEMA
        or core.get("schema") != HEAD_SCHEMA
        or core.get("appliance_id") != appliance
        or value.get("core_sha256") != sha256(core_bytes)
        or auth.get("algorithm") != "ed25519"
        or auth.get("key_id") != key_id
        or not verify_ed25519(public_key, core_bytes, signature)
        or actual != expected
    ):
        raise TrainError(
            "inquiry head does not authenticate the exact ledger tail", path=path
        )


def validate_current(
    workspace: Path,
    appliance: str,
    public_key: bytes,
    key_id: str,
    tail: dict[str, Any],
    entry_hash: str,
    segment: int,
    index: int,
) -> None:
    path = workspace / "runtime/scheduled-introspection/projection/inquiry-current.json"
    try:
        value = json.loads(stable_regular(path, 48 * 1024))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise TrainError("inquiry current projection JSON is malformed", path=path) from error
    core_fields = {
        "schema",
        "appliance_id",
        "signed_entry_id",
        "step_id",
        "admission_id",
        "recorded_at_unix_ms",
        "summary",
        "summary_sha256",
        "inquiry_step",
        "inquiry_step_sha256",
        "declaration_sha256",
        "response_sha256",
        "trace",
        "trigger_kind",
        "due_nonce",
        "trigger_nonce",
        "reflection_path",
        "reflection_sha256",
        "ledger",
        "provenance",
        "authority",
    }
    if (
        not isinstance(value, dict)
        or set(value) != core_fields | {"core_sha256", "auth"}
        or not isinstance(value.get("auth"), dict)
        or set(value["auth"]) != {"algorithm", "key_id", "signature"}
    ):
        raise TrainError("inquiry current projection shape is invalid", path=path)
    core = {key: value[key] for key in core_fields}
    core_bytes = canonical(core)
    auth = value["auth"]
    try:
        signature = bytes.fromhex(str(auth.get("signature", "")))
    except ValueError as error:
        raise TrainError(
            "inquiry current signature encoding is invalid", path=path
        ) from error
    ledger = core.get("ledger")
    if (
        core.get("schema") != CURRENT_SCHEMA
        or core.get("appliance_id") != appliance
        or value.get("core_sha256") != sha256(core_bytes)
        or auth.get("algorithm") != "ed25519"
        or auth.get("key_id") != key_id
        or not verify_ed25519(public_key, core_bytes, signature)
        or not isinstance(ledger, dict)
        or ledger.get("segment") != segment
        or ledger.get("entry_index") != index
        or ledger.get("entry_sha256") != entry_hash
        or ledger.get("prior_entry_sha256") != tail.get("prior_entry_sha256")
        or core.get("signed_entry_id") != tail.get("signed_entry_id")
        or core.get("step_id") != tail.get("step_id")
        or core.get("admission_id") != tail.get("admission_id")
        or core.get("summary") != tail.get("summary")
        or core.get("summary_sha256") != tail.get("summary_sha256")
        or core.get("inquiry_step") != tail.get("inquiry_step")
        or core.get("inquiry_step_sha256") != tail.get("inquiry_step_sha256")
        or core.get("declaration_sha256") != tail.get("declaration_sha256")
        or core.get("response_sha256") != tail.get("response_sha256")
        or core.get("trace") != tail.get("trace")
        or core.get("reflection_path") != tail.get("reflection_path")
        or core.get("reflection_sha256") != tail.get("reflection_sha256")
        or core.get("recorded_at_unix_ms") != tail.get("recorded_at_unix_ms")
        or core.get("trigger_kind") != tail.get("trigger_kind")
        or core.get("due_nonce") != tail.get("due_nonce")
        or core.get("trigger_nonce") != tail.get("trigger_nonce")
        or core.get("provenance") != tail.get("provenance")
        or core.get("authority")
        != "immutable_steward_signed_bounded_inquiry_projection_observational_only"
    ):
        raise TrainError(
            "inquiry current projection does not bind the signed tail", path=path
        )


def full_chain_events(
    workspace: Path,
    root: Path,
    appliance: str,
    public_key: bytes,
    key_id: str,
    *,
    full: bool,
) -> list[dict[str, Any]]:
    try:
        root_metadata = root.lstat()
        segment_root = root / "segments"
        segment_metadata = segment_root.lstat()
    except OSError as error:
        raise TrainError(
            f"immutable inquiry ledger is unavailable: {error}", path=root
        ) from error
    if (
        root.is_symlink()
        or segment_root.is_symlink()
        or not stat.S_ISDIR(root_metadata.st_mode)
        or not stat.S_ISDIR(segment_metadata.st_mode)
        or root_metadata.st_mode & 0o022
        or segment_metadata.st_mode & 0o022
    ):
        raise TrainError(
            "immutable inquiry ledger directory identity is unsafe", path=root
        )
    try:
        inventory = sorted(segment_root.iterdir(), key=lambda item: item.name)
    except OSError as error:
        raise TrainError("cannot enumerate immutable inquiry segments", path=segment_root) from error
    if any(
        re.fullmatch(r"segment-[0-9]{20}\.jsonl", path.name) is None
        for path in inventory
    ):
        raise TrainError("immutable inquiry segment directory has an unknown entry", path=segment_root)
    paths = inventory
    if not paths or len(paths) > MAX_SEGMENTS:
        raise TrainError(
            "immutable inquiry ledger has no bounded segment inventory", path=segment_root
        )
    expected_names = [f"segment-{index:020}.jsonl" for index in range(1, len(paths) + 1)]
    if [path.name for path in paths] != expected_names:
        raise TrainError(
            "immutable inquiry ledger segment sequence has a gap", path=segment_root
        )
    events: list[dict[str, Any]] = []
    prior_hash, prior_entry_id = GENESIS_HASH, "genesis"
    last_segment = last_index = last_segment_bytes = 0
    last_core: dict[str, Any] | None = None
    for segment_number, path in enumerate(paths, start=1):
        raw = stable_regular(path, SEGMENT_MAX_BYTES)
        if not raw.endswith(b"\n"):
            raise TrainError("immutable inquiry segment has a torn tail", path=path)
        lines = raw.splitlines()
        if not lines:
            raise TrainError("immutable inquiry segment is empty")
        for entry_index, line in enumerate(lines, start=1):
            if len(line) > ENTRY_MAX_BYTES or len(events) >= MAX_ENTRIES:
                raise TrainError(
                    "immutable inquiry entry inventory exceeds its bound", path=path
                )
            try:
                value = json.loads(line)
            except (UnicodeError, json.JSONDecodeError) as error:
                raise TrainError("immutable inquiry entry is malformed", path=path) from error
            try:
                core, entry_hash = validate_entry(
                    value, appliance, public_key, key_id, prior_hash, prior_entry_id
                )
            except TrainError as error:
                raise TrainError(str(error), path=path) from error
            events.append(
                entry_event(
                    core,
                    entry_hash,
                    "full_signed_hash_chain_verified",
                    workspace,
                    full,
                )
            )
            prior_hash, prior_entry_id = entry_hash, str(core["signed_entry_id"])
            last_core = core
            last_segment, last_index = segment_number, entry_index
        last_segment_bytes = len(raw)
    validate_head(
        root,
        appliance,
        public_key,
        key_id,
        len(events),
        last_segment,
        last_index,
        prior_entry_id,
        prior_hash,
        last_segment_bytes,
    )
    if last_core is None:
        raise TrainError("immutable inquiry ledger has no signed tail", path=segment_root)
    validate_current(
        workspace,
        appliance,
        public_key,
        key_id,
        last_core,
        prior_hash,
        last_segment,
        last_index,
    )
    directory_identity = lambda item: (
        item.st_dev,
        item.st_ino,
        item.st_mode,
        item.st_uid,
        item.st_gid,
        item.st_mtime_ns,
        item.st_ctime_ns,
    )
    try:
        final_root = root.lstat()
        final_segment_root = segment_root.lstat()
    except OSError as error:
        raise TrainError("immutable inquiry directory changed during verification", path=root) from error
    if (
        directory_identity(root_metadata) != directory_identity(final_root)
        or directory_identity(segment_metadata) != directory_identity(final_segment_root)
    ):
        raise TrainError("immutable inquiry directory changed during verification", path=root)
    return events


def exact_terminal(reflection: bytes) -> tuple[dict[str, Any], str, str]:
    try:
        text = reflection.decode("utf-8", errors="strict")
    except UnicodeError as error:
        raise TrainError("attested reflection is not UTF-8") from error
    lines = text.removesuffix("\n").splitlines()
    if len(lines) < 2 or not lines[-2].startswith("INQUIRY_STEP: ") or lines[-1] not in {
        "SOURCE_REVIEW: NONE",
        "SOURCE_REVIEW: REQUEST",
    }:
        raise TrainError("attested reflection has no exact inquiry terminal")
    if sum(line.startswith("INQUIRY_STEP:") for line in lines) != 1:
        raise TrainError("attested reflection has duplicate inquiry terminals")
    declaration, source_review = lines[-2], lines[-1]
    try:
        step = json.loads(declaration.removeprefix("INQUIRY_STEP: "))
    except json.JSONDecodeError as error:
        raise TrainError("attested inquiry terminal is malformed") from error
    validate_step(step)
    return step, declaration, source_review


def terminal_step(reflection: bytes) -> dict[str, Any]:
    return exact_terminal(reflection)[0]


def preflight_attestation_inventory(
    workspace: Path,
    appliance: str,
    public_key: bytes,
    key_id: str,
    *,
    full: bool = False,
) -> list[tuple[Path, dict[str, Any], bytes]]:
    """Verify every owner-visible immutable attestation before using any of them.

    The append-only copies are a protected trust surface.  One malformed or
    invalid member therefore invalidates the inventory instead of silently
    disappearing from an otherwise clean-looking report.
    """

    root = workspace / "introspections/scheduled"
    records: list[tuple[Path, dict[str, Any], bytes]] = []
    paths = sorted(root.glob("authorship_attestation_due-*.json"))
    if len(paths) > MAX_ENTRIES:
        raise TrainError("protected attestation inventory exceeds its bound", path=root)
    for path in paths:
        try:
            envelope = json.loads(stable_regular(path, ENTRY_MAX_BYTES))
        except (UnicodeError, json.JSONDecodeError) as error:
            raise TrainError("protected attestation JSON is malformed", path=path) from error
        if not isinstance(envelope, dict) or set(envelope) != {"schema", "core", "auth"}:
            raise TrainError("protected attestation envelope shape is invalid", path=path)
        core, auth = envelope.get("core"), envelope.get("auth")
        if (
            not isinstance(core, dict)
            or not isinstance(auth, dict)
            or set(auth) != {"algorithm", "key_id", "signature"}
        ):
            raise TrainError("protected attestation is incomplete", path=path)
        unsigned = {"schema": AUTHORSHIP_ENVELOPE_SCHEMA, "core": core}
        try:
            signature = bytes.fromhex(str(auth.get("signature", "")))
        except ValueError as error:
            raise TrainError("protected attestation signature is malformed", path=path) from error
        trigger = core.get("trigger_kind")
        status = core.get("terminal_status")
        trace = core.get("trace")
        relative = exact_scheduled_artifact_path(core.get("reflection_path"), ".md")
        optional_inquiry_fields = (
            "continuity_projection_sha256",
            "inquiry_current_projection_sha256",
            "signed_entry_id",
            "step_id",
            "admission_id",
            "inquiry_step_sha256",
            "inquiry_declaration_sha256",
        )
        expected_name = (
            f"authorship_attestation_{core.get('due_nonce')}_"
            f"{core.get('response_sha256')}.json"
        )
        if (
            path.name != expected_name
            or envelope.get("schema") != AUTHORSHIP_ENVELOPE_SCHEMA
            or core.get("schema") != AUTHORSHIP_CORE_SCHEMA
            or core.get("appliance_id") != appliance
            or status not in {"model_authored_structured", "model_authored_unstructured"}
            or authored_provenance(trigger) is None
            or not safe_id(core.get("due_nonce"))
            or not safe_id(core.get("trigger_nonce"))
            or core.get("provenance") != authored_provenance(trigger)
            or core.get("authority") != "immutable_steward_signed_exact_authorship_join"
            or auth.get("algorithm") != "ed25519"
            or auth.get("key_id") != key_id
            or not verify_ed25519(public_key, canonical(unsigned), signature)
            or not isinstance(trace, dict)
            or set(trace)
            != {"schema_version", "trace_id", "turn_id", "span_id", "session_id"}
            or trace.get("schema_version") != 1
            or any(
                not safe_id(trace.get(field))
                for field in ("trace_id", "turn_id", "span_id", "session_id")
            )
            or not exact_sha(core.get("prompt_sha256"))
            or not exact_sha(core.get("response_sha256"))
            or not exact_sha(core.get("reflection_sha256"))
            or not exact_sha(core.get("reflection_metadata_sha256"))
            or not exact_sha(core.get("state_projection_sha256"))
            or not exact_sha(core.get("terminal_receipt_sha256"))
            or not exact_sha(core.get("context_provenance_sha256"))
            or core.get("reflection_sha256") != core.get("response_sha256")
            or not isinstance(core.get("completed_at_unix_ms"), int)
            or isinstance(core.get("completed_at_unix_ms"), bool)
            or int(core.get("completed_at_unix_ms", 0)) <= 0
            or relative is None
        ):
            raise TrainError("protected attestation identity or binding failed", path=path)
        assert relative is not None
        reflection = stable_scheduled_artifact(
            workspace,
            relative.as_posix(),
            ".md",
            REFLECTION_MAX_BYTES,
            private=full,
        )
        metadata_relative = relative.with_suffix(".json")
        metadata = stable_scheduled_artifact(
            workspace,
            metadata_relative.as_posix(),
            ".json",
            16 * 1024,
        )
        if (
            sha256(reflection) != core.get("reflection_sha256")
            or sha256(metadata) != core.get("reflection_metadata_sha256")
        ):
            raise TrainError("protected attestation artifact hashes failed", path=path)
        if status == "model_authored_structured":
            try:
                step, declaration, _source_review = exact_terminal(reflection)
            except TrainError as error:
                raise TrainError(str(error), path=path) from error
            if (
                any(core.get(field) is None for field in optional_inquiry_fields)
                or not safe_id(core.get("signed_entry_id"))
                or not safe_id(core.get("step_id"))
                or not safe_id(core.get("admission_id"))
                or not exact_sha(core.get("inquiry_step_sha256"))
                or not exact_sha(core.get("inquiry_declaration_sha256"))
                or not exact_sha(core.get("continuity_projection_sha256"))
                or not exact_sha(core.get("inquiry_current_projection_sha256"))
                or core.get("inquiry_step_sha256") != sha256(canonical(step))
                or core.get("inquiry_declaration_sha256")
                != sha256(declaration.encode("utf-8"))
            ):
                raise TrainError("structured attestation inquiry binding failed", path=path)
        elif any(core.get(field) is not None for field in optional_inquiry_fields):
            raise TrainError("unstructured attestation carries inquiry authority", path=path)
        records.append((path, core, reflection))
    return records


def attestation_events(
    workspace: Path,
    appliance: str,
    records: Iterable[tuple[Path, dict[str, Any], bytes]],
    *,
    full: bool,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    structured: list[dict[str, Any]] = []
    unstructured: list[dict[str, Any]] = []
    for _path, core, reflection in records:
        trace = core["trace"]
        if core["terminal_status"] == "model_authored_structured":
            step, _declaration, _source_review = exact_terminal(reflection)
            synthetic = {
                "schema": ENTRY_SCHEMA,
                "appliance_id": appliance,
                "recorded_at_unix_ms": core["completed_at_unix_ms"],
                "trigger_kind": core["trigger_kind"],
                "due_nonce": core["due_nonce"],
                "trigger_nonce": core["trigger_nonce"],
                "signed_entry_id": core["signed_entry_id"],
                "step_id": core["step_id"],
                "admission_id": core["admission_id"],
                "inquiry_step": step,
                "trace": trace,
                "response_sha256": core["response_sha256"],
                "reflection_path": core["reflection_path"],
                "reflection_sha256": core["reflection_sha256"],
                "declaration_sha256": core["inquiry_declaration_sha256"],
                "summary": inquiry_summary(step),
                "summary_sha256": sha256(inquiry_summary(step).encode()),
                "mechanical_predecessor": "unavailable_without_private_ledger",
                "prior_entry_sha256": None,
                "authority": "signed_authored_inquiry_not_hidden_chain_of_thought_not_code_authority",
            }
            event = entry_event(
                synthetic,
                "unavailable_without_private_ledger",
                "individually_attested_mechanical_chain_unavailable",
                workspace,
                False,
            )
            if full:
                event["reflection_text"] = reflection.decode("utf-8", errors="strict")
                event["reflection_text_authority"] = (
                    "hash_verified_owner_private_exact_prose"
                )
            structured.append(event)
            continue
        event: dict[str, Any] = {
            "timestamp_unix_ms": int(core["completed_at_unix_ms"]),
            "kind": "scheduled_reflection",
            "status": "model_authored_unstructured",
            "authored": True,
            "fallback": False,
            "authorship_class": "astrid_authored_programmatic_unstructured_exact_prose",
            "provenance_class": "astrid_authored",
            "appliance_id": appliance,
            "trigger_kind": core["trigger_kind"],
            "trace_id": trace.get("trace_id"),
            "turn_id": trace.get("turn_id"),
            "span_id": trace.get("span_id"),
            "session_id": trace.get("session_id"),
            "response_sha256": core["response_sha256"],
            "reflection_path": core["reflection_path"],
            "reflection_sha256": core["reflection_sha256"],
            "structured_inquiry": False,
            "continuity_admitted": False,
            "reservoir_admitted": False,
            "source_review_requested": False,
            "train_integrity": "individually_attested_no_inquiry_step_by_design",
            "source_ledger": "introspections/scheduled/authorship_attestation_due-*.json",
            "authority": "exact_signed_prose_no_inquiry_continuity_or_code_authority",
        }
        if full:
            event["reflection_text"] = reflection.decode("utf-8", errors="strict")
            event["reflection_text_authority"] = (
                "hash_verified_owner_private_exact_prose"
            )
        unstructured.append(event)
    return structured, unstructured


def individually_attested_events(
    workspace: Path,
    appliance: str,
    public_key: bytes,
    key_id: str,
    *,
    full: bool,
) -> list[dict[str, Any]]:
    records = preflight_attestation_inventory(
        workspace, appliance, public_key, key_id, full=full
    )
    events, _unstructured = attestation_events(
        workspace, appliance, records, full=full
    )
    if not events:
        raise TrainError("no valid individually attested structured reflections")
    return events


def attested_unstructured_events(
    workspace: Path,
    appliance: str,
    public_key: bytes,
    key_id: str,
    *,
    full: bool,
) -> list[dict[str, Any]]:
    """Verify exact authored prose that deliberately created no inquiry step."""
    records = preflight_attestation_inventory(
        workspace, appliance, public_key, key_id, full=full
    )
    _structured, events = attestation_events(
        workspace, appliance, records, full=full
    )
    return events


def read_json_lines(path: Path, maximum: int = 256 * 1024 * 1024) -> list[dict[str, Any]]:
    """Read one optional protected JSONL ledger without skipping bad records."""

    try:
        path.lstat()
    except FileNotFoundError:
        return []
    except OSError as error:
        raise TrainError("cannot inspect protected JSONL ledger", path=path) from error
    raw = stable_regular(path, maximum)
    if raw and not raw.endswith(b"\n"):
        raise TrainError("protected JSONL ledger has a torn tail", path=path)
    values: list[dict[str, Any]] = []
    for index, line in enumerate(raw.splitlines(), start=1):
        if not line or len(line) > PROJECTION_RECORD_MAX_BYTES:
            raise TrainError(
                f"protected JSONL record {index} has an invalid size", path=path
            )
        try:
            value = json.loads(line)
        except (UnicodeError, json.JSONDecodeError) as error:
            raise TrainError(
                f"protected JSONL record {index} is malformed", path=path
            ) from error
        if not isinstance(value, dict):
            raise TrainError(
                f"protected JSONL record {index} is not an object", path=path
            )
        values.append(value)
    return values


def read_optional_json_object(path: Path, maximum: int) -> dict[str, Any]:
    try:
        path.lstat()
    except FileNotFoundError:
        return {}
    except OSError as error:
        raise TrainError("cannot inspect protected JSON projection", path=path) from error
    try:
        value = json.loads(stable_regular(path, maximum))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise TrainError("protected JSON projection is malformed", path=path) from error
    if not isinstance(value, dict):
        raise TrainError("protected JSON projection is not an object", path=path)
    return value


def valid_uuid(value: Any) -> bool:
    try:
        parsed = uuid.UUID(str(value))
    except (ValueError, TypeError, AttributeError):
        return False
    return parsed.int != 0 and str(parsed) == str(value).lower()


def valid_action_trace(value: Any) -> bool:
    allowed = {
        "schema_version",
        "trace_id",
        "turn_id",
        "span_id",
        "parent_span_id",
        "session_id",
        "chain_id",
    }
    if (
        not isinstance(value, dict)
        or not set(value).issubset(allowed)
        or value.get("schema_version", 1) != 1
        or not valid_uuid(value.get("trace_id"))
        or not valid_uuid(value.get("span_id"))
        or (value.get("turn_id") is not None and not valid_uuid(value.get("turn_id")))
        or (
            value.get("parent_span_id") is not None
            and (
                not valid_uuid(value.get("parent_span_id"))
                or value.get("parent_span_id") == value.get("span_id")
            )
        )
    ):
        return False
    for field in ("session_id", "chain_id"):
        label = value.get(field)
        if label is not None and (
            not isinstance(label, str)
            or not label.strip()
            or len(label) > 96
            or any(unicodedata.category(character) == "Cc" for character in label)
        ):
            return False
    return True


def evidence_trace_fields(record: dict[str, Any]) -> dict[str, Any]:
    """Return exact evidence-local causal fields, or an unattributed projection."""
    trace = record.get("trace")
    if not valid_action_trace(trace):
        return {
            "trace_id": None,
            "turn_id": None,
            "span_id": None,
            "session_id": None,
            "chain_id": None,
        }
    assert isinstance(trace, dict)
    return {
        "trace_id": trace.get("trace_id"),
        "turn_id": trace.get("turn_id"),
        "span_id": trace.get("span_id"),
        "session_id": trace.get("session_id"),
        "chain_id": trace.get("chain_id"),
    }


def exact_action_receipt_key(
    response_sha256: Any, trace: Any
) -> tuple[str, bytes] | None:
    if not exact_sha(response_sha256) or not valid_action_trace(trace):
        return None
    assert isinstance(trace, dict)
    return str(response_sha256), canonical(trace)


def exact_action_receipts(
    workspace: Path,
) -> dict[tuple[str, bytes], dict[str, Any]]:
    candidates: dict[tuple[str, bytes], list[dict[str, Any]]] = {}
    for receipt in read_json_lines(workspace / "actions/receipts.jsonl"):
        trace = receipt.get("trace")
        response_hash = receipt.get("response_sha256")
        key = exact_action_receipt_key(response_hash, trace)
        if (
            receipt.get("schema") != "astrid_edge_action_receipt_v5"
            or receipt.get("decision_source") != "astrid_declared"
            or receipt.get("status") != "executed"
            or receipt.get("recovery_reason") is not None
            or key is None
            or not isinstance(receipt.get("declared_next"), str)
            or not valid_action_trace(trace)
            or not isinstance(trace, dict)
            or receipt.get("session_id") != trace.get("session_id")
            or receipt.get("authority")
            != "validated_model_next_with_optional_syntax_only_repair_owned_workspace_only"
        ):
            continue
        candidates.setdefault(key, []).append(receipt)
    return {
        digest: values[0]
        for digest, values in candidates.items()
        if len(values) == 1
    }


def parse_update_belief(declaration: Any) -> tuple[str, list[str], str, str] | None:
    if not isinstance(declaration, str) or not declaration.startswith("UPDATE_BELIEF "):
        return None
    try:
        subject, disposition, claim = (
            part.strip()
            for part in declaration.removeprefix("UPDATE_BELIEF ").split("::", 2)
        )
    except ValueError:
        return None
    match = re.fullmatch(r"([^\s]+)\s+WITH\s+([^\s]+)", subject)
    if match is None or disposition not in {
        "supported",
        "weakened",
        "revised",
        "suspended",
        "unresolved",
    }:
        return None
    belief_id, evidence = match.groups()
    evidence_ids = [item.strip() for item in evidence.split(",")]
    if (
        not safe_id(belief_id)
        or not evidence_ids
        or any(not safe_id(item) for item in evidence_ids)
        or not claim
    ):
        return None
    return belief_id, evidence_ids, disposition, claim


def thread_events(
    workspace: Path, inquiry: Iterable[dict[str, Any]] = ()
) -> list[dict[str, Any]]:
    rows = read_json_lines(workspace / "autonomous/thread_state.jsonl")
    signed_steps = {
        str(event.get("step_id")): event
        for event in inquiry
        if event.get("kind") == "inquiry_step"
        and safe_id(event.get("step_id"))
        and event.get("train_integrity")
        in {
            "full_signed_hash_chain_verified",
            "individually_attested_mechanical_chain_unavailable",
        }
    }
    action_receipts = exact_action_receipts(workspace)
    events: dict[tuple[str, str], dict[str, Any]] = {}
    for row in rows:
        if row.get("schema") != "astrid_edge_thread_state_v7":
            continue
        timestamp = int(row.get("updated_at_unix_ms", 0) or 0)
        trace = row.get("trace") if isinstance(row.get("trace"), dict) else {}
        for record in row.get("evidence_records") or []:
            if not isinstance(record, dict) or not safe_id(record.get("evidence_id")):
                continue
            evidence_id = str(record["evidence_id"])
            causal_fields = evidence_trace_fields(record)
            parent_response_sha256 = record.get("parent_response_sha256")
            event = {
                "timestamp_unix_ms": int(record.get("captured_at_unix_ms", timestamp) or timestamp),
                "kind": "evidence_arrival",
                "status": record.get("epistemic_status", "recorded"),
                "authored": False,
                "fallback": False,
                "authorship_class": "machine_or_executor_evidence_not_astrid_authorship",
                "provenance_class": "machine_evidence",
                "evidence_id": evidence_id,
                "evidence_kind": record.get("kind"),
                "reference": record.get("reference"),
                "summary": record.get("summary"),
                "source": record.get("source"),
                "sha256": record.get("sha256"),
                "parent_response_sha256": (
                    parent_response_sha256
                    if exact_sha(parent_response_sha256)
                    else None
                ),
                "eligible_for_belief_update": record.get("eligible_for_belief_update") is True,
                "thread_id": row.get("thread_id"),
                **causal_fields,
                "source_ledger": "autonomous/thread_state.jsonl",
                "authority": "typed_evidence_arrival_does_not_change_belief_automatically",
            }
            events.setdefault(("evidence", evidence_id), event)
        for belief in row.get("beliefs") or []:
            if not isinstance(belief, dict) or not safe_id(belief.get("revision_id")):
                continue
            revision_id = str(belief["revision_id"])
            response_hash = belief.get("response_sha256")
            source = str(belief.get("source") or "")
            proof = "unverified_thread_projection_not_authorship"
            authored = False
            provenance_class = "executor_outcome"
            if source in {
                "signed_scheduled_inquiry_step",
                "signed_evidence_integration_inquiry_step",
            }:
                step_id = row.get("last_admitted_inquiry_step_id")
                step = signed_steps.get(str(step_id or ""))
                expected_operation = step.get("belief_operation") if step else None
                if (
                    step is not None
                    and step.get("response_sha256") == response_hash
                    and step.get("thread_id") == belief.get("thread_id")
                    and step.get("belief_id") == belief.get("belief_id")
                    and step.get("belief_claim") == belief.get("claim")
                    and step.get("evidence_ids") == (belief.get("evidence_ids") or [])
                    and expected_operation == belief.get("operation")
                ):
                    authored = True
                    provenance_class = "astrid_authored"
                    proof = "exact_signed_inquiry_step_projection"
            else:
                row_trace = row.get("trace")
                receipt_key = exact_action_receipt_key(response_hash, row_trace)
                receipt = (
                    action_receipts.get(receipt_key)
                    if receipt_key is not None
                    else None
                )
                declaration = receipt.get("declared_next") if receipt else None
                receipt_trace = receipt.get("trace") if receipt else None
                if (
                    receipt is not None
                    and receipt_trace == row_trace
                    and source == "exact_unrepaired_update_belief_action"
                ):
                    parsed = parse_update_belief(declaration)
                    if parsed == (
                        belief.get("belief_id"),
                        belief.get("evidence_ids") or [],
                        belief.get("operation"),
                        belief.get("claim"),
                    ):
                        authored = True
                        provenance_class = "astrid_authored"
                        proof = "exact_unrepaired_action_receipt_projection"
                elif (
                    receipt is not None
                    and receipt_trace == row_trace
                    and source == "authored_propose_action"
                    and isinstance(declaration, str)
                    and declaration.startswith("PROPOSE ")
                    and declaration.removeprefix("PROPOSE ").strip()
                    == belief.get("claim")
                ):
                    authored = True
                    provenance_class = "astrid_authored"
                    proof = "exact_unrepaired_action_receipt_projection"
            events.setdefault(("belief", revision_id), {
                "timestamp_unix_ms": int(belief.get("recorded_at_unix_ms", timestamp) or timestamp),
                "kind": "belief_revision",
                "status": belief.get("operation", "recorded"),
                "authored": authored,
                "fallback": False,
                "authorship_class": (
                    "astrid_authored_explicit_belief_revision"
                    if authored
                    else "unverified_durable_belief_projection_non_authored"
                ),
                "provenance_class": provenance_class,
                "revision_id": revision_id,
                "belief_id": belief.get("belief_id"),
                "thread_id": belief.get("thread_id"),
                "operation": belief.get("operation"),
                "claim": belief.get("claim"),
                "evidence_ids": belief.get("evidence_ids") or [],
                "prior_revision_id": belief.get("prior_revision_id"),
                "response_sha256": belief.get("response_sha256"),
                "source": source,
                "source_ledger": "autonomous/thread_state.jsonl",
                "authority": proof,
            })
        event_name = str(row.get("event", ""))
        if event_name.startswith(("inquiry_", "thread_", "action_")):
            transition_id = f"{row.get('revision', 0)}:{event_name}"
            response_hash = row.get("response_sha256")
            step_id = row.get("last_admitted_inquiry_step_id")
            step = signed_steps.get(str(step_id or ""))
            receipt_key = exact_action_receipt_key(response_hash, row.get("trace"))
            receipt = (
                action_receipts.get(receipt_key)
                if receipt_key is not None
                else None
            )
            signed = (
                event_name.startswith("inquiry_step_")
                and step is not None
                and step.get("response_sha256") == response_hash
                and step.get("thread_id") == row.get("thread_id")
            )
            exact_action = (
                event_name.startswith("action_")
                and receipt is not None
                and receipt.get("trace") == row.get("trace")
                and receipt.get("declared_next") == row.get("last_action")
            )
            authored = signed or exact_action
            events.setdefault(("transition", transition_id), {
                "timestamp_unix_ms": timestamp,
                "kind": "thread_transition",
                "status": row.get("status"),
                "authored": authored,
                "fallback": False,
                "authorship_class": (
                    "astrid_authored_thread_transition"
                    if authored
                    else "unverified_durable_thread_projection_non_authored"
                ),
                "provenance_class": "astrid_authored" if authored else "executor_outcome",
                "transition_id": transition_id,
                "thread_id": row.get("thread_id"),
                "event": event_name,
                "last_step_id": row.get("last_admitted_inquiry_step_id"),
                "trace_id": trace.get("trace_id"),
                "turn_id": trace.get("turn_id"),
                "span_id": trace.get("span_id"),
                "session_id": trace.get("session_id"),
                "source_ledger": "autonomous/thread_state.jsonl",
                "authority": (
                    "exact_signed_inquiry_step_projection"
                    if signed
                    else "exact_unrepaired_action_receipt_projection"
                    if exact_action
                    else "durable_v7_projection_not_independent_authorship"
                ),
            })
    return sorted(
        events.values(),
        key=lambda item: (
            int(item.get("timestamp_unix_ms", 0)),
            str(item.get("kind", "")),
            str(item.get("revision_id") or item.get("evidence_id") or item.get("transition_id") or ""),
        ),
    )


def validate_admission(
    admission: dict[str, Any], inquiry_by_entry: dict[str, dict[str, Any]]
) -> dict[str, Any]:
    expected_fields = {
        "schema",
        "continuity_admitted",
        "admitted_at_unix_ms",
        "signed_entry_id",
        "admission_id",
        "last_response_sha256",
        "last_summary_sha256",
        "last_trace_id",
        "last_due_nonce",
        "reservoir_delivery",
        "queued_at_unix_ms",
        "terminal_at_unix_ms",
        "reservoir_generation",
        "reservoir_sequence",
        "vector_sha256",
        "source_class",
        "migrated_legacy_schema",
        "provenance",
        "authority",
    }
    signed_entry_id = admission.get("signed_entry_id")
    event = inquiry_by_entry.get(str(signed_entry_id or ""))
    status = admission.get("reservoir_delivery")
    source_class = {
        "scheduled": "scheduled_inquiry",
        "evidence_integration": "evidence_integration",
    }.get(event.get("trigger_kind") if event else None)
    queued_at = admission.get("queued_at_unix_ms")
    terminal_at = admission.get("terminal_at_unix_ms")
    generation = admission.get("reservoir_generation")
    sequence = admission.get("reservoir_sequence")
    if (
        set(admission) != expected_fields
        or admission.get("schema") != ADMISSION_SCHEMA
        or admission.get("continuity_admitted") is not True
        or event is None
        or not safe_id(signed_entry_id)
        or admission.get("admission_id") != event.get("admission_id")
        or admission.get("last_response_sha256") != event.get("response_sha256")
        or admission.get("last_summary_sha256") != event.get("summary_sha256")
        or admission.get("last_trace_id") != event.get("trace_id")
        or not safe_id(admission.get("last_due_nonce"))
        or not isinstance(admission.get("admitted_at_unix_ms"), int)
        or isinstance(admission.get("admitted_at_unix_ms"), bool)
        or int(admission.get("admitted_at_unix_ms", 0)) <= 0
        or not isinstance(queued_at, int)
        or isinstance(queued_at, bool)
        or queued_at <= 0
        or not exact_sha(admission.get("vector_sha256"))
        or admission.get("source_class") != source_class
        or admission.get("provenance")
        != authored_provenance(event.get("trigger_kind") if event else None)
        or admission.get("authority") != "verified_signed_inquiry_observational_only"
        or status not in {"queued", "acknowledged", "delivery_unknown", "failed"}
    ):
        raise TrainError("semantic admission state does not bind a signed inquiry step")
    if status == "queued":
        valid_terminal = terminal_at is None and generation is None and sequence is None
    elif status == "acknowledged":
        valid_terminal = (
            isinstance(terminal_at, int)
            and not isinstance(terminal_at, bool)
            and terminal_at > 0
            and isinstance(generation, str)
            and 0 < len(generation) <= 96
            and isinstance(sequence, int)
            and not isinstance(sequence, bool)
        )
    else:
        valid_terminal = (
            isinstance(terminal_at, int)
            and not isinstance(terminal_at, bool)
            and terminal_at > 0
            and generation is None
            and sequence is None
        )
    if not valid_terminal:
        raise TrainError("semantic admission terminal state is internally inconsistent")
    return event


def semantic_admission_receipt_history(
    workspace: Path,
    inquiry: Iterable[dict[str, Any]],
) -> tuple[list[dict[str, Any]], dict[str, dict[str, Any]]]:
    relative = Path(
        "runtime/scheduled-introspection/admission/receipts.jsonl"
    )
    try:
        raw = stable_exact_workspace_artifact(
            workspace,
            relative,
            ADMISSION_RECEIPT_MAX_BYTES,
            private=True,
            required_mode=0o600,
            private_parent=True,
        )
    except TrainError as error:
        if isinstance(error.__cause__, FileNotFoundError):
            return [], {}
        raise
    if not raw.endswith(b"\n"):
        raise TrainError("semantic admission receipt ledger has a torn tail")
    inquiry_by_entry = {
        str(event.get("signed_entry_id")): event
        for event in inquiry
        if safe_id(event.get("signed_entry_id"))
    }
    lifecycle: dict[str, tuple[str, dict[str, Any]]] = {}
    index: dict[str, dict[str, Any]] = {}
    events: list[dict[str, Any]] = []
    event_status = {
        "queued": "queued",
        "acknowledged": "acknowledged",
        "delivery_unknown": "delivery_unknown",
        "failed": "failed",
        "interrupted_before_ack": "delivery_unknown",
        "superseded_queued_delivery_unknown": "delivery_unknown",
    }
    immutable_fields = (
        "schema",
        "continuity_admitted",
        "admitted_at_unix_ms",
        "signed_entry_id",
        "admission_id",
        "last_response_sha256",
        "last_summary_sha256",
        "last_trace_id",
        "last_due_nonce",
        "queued_at_unix_ms",
        "vector_sha256",
        "source_class",
        "migrated_legacy_schema",
        "provenance",
        "authority",
    )
    for line_number, line in enumerate(raw.splitlines(), start=1):
        if not line or len(line) > ADMISSION_RECEIPT_LINE_MAX_BYTES:
            raise TrainError(
                f"semantic admission receipt line {line_number} exceeds its bound"
            )
        try:
            record = json.loads(line)
        except (UnicodeError, json.JSONDecodeError) as error:
            raise TrainError(
                f"semantic admission receipt line {line_number} is malformed"
            ) from error
        if (
            not isinstance(record, dict)
            or set(record) != {"schema", "event", "recorded_at_unix_ms", "state"}
            or record.get("schema") != ADMISSION_RECEIPT_SCHEMA
            or record.get("event") not in event_status
            or not isinstance(record.get("recorded_at_unix_ms"), int)
            or isinstance(record.get("recorded_at_unix_ms"), bool)
            or int(record.get("recorded_at_unix_ms", 0)) <= 0
            or not isinstance(record.get("state"), dict)
        ):
            raise TrainError(
                f"semantic admission receipt line {line_number} has an invalid shape"
            )
        state = record["state"]
        event = validate_admission(state, inquiry_by_entry)
        status = str(state.get("reservoir_delivery"))
        if event_status[str(record["event"])] != status:
            raise TrainError(
                f"semantic admission receipt line {line_number} event/status mismatch"
            )
        admission_id = str(state["admission_id"])
        previous = lifecycle.get(admission_id)
        if previous is None:
            if status != "queued" or record.get("event") != "queued":
                raise TrainError(
                    f"semantic admission {admission_id} lacks its queued predecessor"
                )
        else:
            previous_status, previous_state = previous
            if (
                previous_status != "queued"
                or status == "queued"
                or any(
                    previous_state.get(field) != state.get(field)
                    for field in immutable_fields
                )
            ):
                raise TrainError(
                    f"semantic admission {admission_id} lifecycle is inconsistent"
                )
        lifecycle[admission_id] = (status, state)
        index[admission_id] = {
            "record": record,
            "state": state,
            "inquiry_event": event,
        }
        events.append(
            {
                "timestamp_unix_ms": int(record["recorded_at_unix_ms"]),
                "kind": "semantic_admission",
                "status": status,
                "admission_event": record.get("event"),
                "authored": False,
                "fallback": False,
                "authorship_class": "reservoir_delivery_metadata_non_authored",
                "provenance_class": "executor_outcome",
                "signed_entry_id": state.get("signed_entry_id"),
                "admission_id": admission_id,
                "response_sha256": state.get("last_response_sha256"),
                "summary_sha256": state.get("last_summary_sha256"),
                "source_class": state.get("source_class"),
                "step_id": event.get("step_id"),
                "thread_id": event.get("thread_id"),
                "trace_id": event.get("trace_id"),
                "turn_id": event.get("turn_id"),
                "span_id": event.get("span_id"),
                "session_id": event.get("session_id"),
                "reservoir_generation": state.get("reservoir_generation"),
                "reservoir_sequence": state.get("reservoir_sequence"),
                "vector_sha256": state.get("vector_sha256"),
                "source_ledger": relative.as_posix(),
                "authority": (
                    "exact_append_only_semantic_admission_receipt_"
                    "not_astrid_authorship"
                ),
            }
        )
    return events, index


def current_admission_ambiguity_event(
    workspace: Path,
    inquiry: Iterable[dict[str, Any]],
    receipt_index: dict[str, dict[str, Any]],
) -> dict[str, Any] | None:
    relative = Path("runtime/scheduled-introspection/admission/state.json")
    try:
        raw = stable_exact_workspace_artifact(
            workspace,
            relative,
            64 * 1024,
            private=True,
            required_mode=0o600,
            private_parent=True,
        )
    except TrainError as error:
        if isinstance(error.__cause__, FileNotFoundError):
            return None
        raise
    try:
        state = json.loads(raw)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise TrainError("current semantic admission state is malformed") from error
    if not isinstance(state, dict):
        raise TrainError("current semantic admission state is not an object")
    if state.get("schema") == LEGACY_ADMISSION_SCHEMA:
        return None
    if state.get("schema") != ADMISSION_SCHEMA:
        raise TrainError("current semantic admission state has an unknown schema")
    inquiry_by_entry = {
        str(event.get("signed_entry_id")): event
        for event in inquiry
        if safe_id(event.get("signed_entry_id"))
    }
    event = validate_admission(state, inquiry_by_entry)
    admission_id = str(state["admission_id"])
    receipt = receipt_index.get(admission_id)
    if receipt is not None and receipt.get("state") == state:
        return None
    return {
        "timestamp_unix_ms": int(
            state.get("terminal_at_unix_ms")
            or state.get("queued_at_unix_ms")
            or state.get("admitted_at_unix_ms")
            or 0
        ),
        "kind": "semantic_admission",
        "status": "delivery_unknown",
        "admission_event": "current_state_ahead_of_receipt_ambiguous",
        "authored": False,
        "fallback": False,
        "authorship_class": "reservoir_delivery_metadata_non_authored",
        "provenance_class": "executor_outcome",
        "signed_entry_id": state.get("signed_entry_id"),
        "admission_id": admission_id,
        "response_sha256": state.get("last_response_sha256"),
        "summary_sha256": state.get("last_summary_sha256"),
        "source_class": state.get("source_class"),
        "step_id": event.get("step_id"),
        "thread_id": event.get("thread_id"),
        "trace_id": event.get("trace_id"),
        "turn_id": event.get("turn_id"),
        "span_id": event.get("span_id"),
        "session_id": event.get("session_id"),
        "reservoir_generation": None,
        "reservoir_sequence": None,
        "vector_sha256": state.get("vector_sha256"),
        "source_ledger": relative.as_posix(),
        "authority": (
            "current_state_without_matching_receipt_delivery_unknown_"
            "no_ack_claim"
        ),
    }


def clean_source_review_projection(value: Any) -> dict[str, Any] | None:
    expected_fields = {
        "status",
        "trace",
        "response_sha256",
        "prompt_sha256",
        "candidate_attested",
        "failure_class",
        "authority",
    }
    statuses = {
        "requested_pending_clean",
        "interrupted_by_restart_non_authored",
        "failed_non_authored",
        "completed_no_candidate",
        "candidate_attested",
    }
    authority = (
        "separate_clean_source_review_fresh_context_"
        "candidate_authority_only_when_attested"
    )
    if (
        not isinstance(value, dict)
        or set(value) != expected_fields
        or value.get("status") not in statuses
        or value.get("authority") != authority
        or not isinstance(value.get("candidate_attested"), bool)
        or value.get("candidate_attested")
        != (value.get("status") == "candidate_attested")
        or (
            value.get("failure_class") is not None
            and (
                not isinstance(value.get("failure_class"), str)
                or not str(value.get("failure_class")).strip()
                or len(str(value.get("failure_class"))) > 320
            )
        )
        or (
            value.get("status")
            in {"failed_non_authored", "interrupted_by_restart_non_authored"}
        )
        != (value.get("failure_class") is not None)
        or (
            value.get("prompt_sha256") is not None
            and not exact_sha(value.get("prompt_sha256"))
        )
        or (
            value.get("response_sha256") is not None
            and not exact_sha(value.get("response_sha256"))
        )
        or (
            value.get("status") in {"completed_no_candidate", "candidate_attested"}
            and not exact_sha(value.get("response_sha256"))
        )
    ):
        return None
    trace = value.get("trace")
    if trace is None:
        if (
            value.get("prompt_sha256") is not None
            or value.get("response_sha256") is not None
            or value.get("status") in {"completed_no_candidate", "candidate_attested"}
        ):
            return None
        trace_fields = {
            "trace_id": None,
            "turn_id": None,
            "span_id": None,
            "session_id": None,
        }
        attribution = "clean_source_review_unstarted_no_causal_identity"
    else:
        if (
            not isinstance(trace, dict)
            or set(trace)
            != {"schema_version", "trace_id", "turn_id", "span_id", "session_id"}
            or not valid_action_trace(trace)
            or not exact_sha(value.get("prompt_sha256"))
        ):
            return None
        trace_fields = {
            key: trace.get(key)
            for key in ("trace_id", "turn_id", "span_id", "session_id")
        }
        attribution = "exact_separate_clean_source_review_trace"
    return {
        **trace_fields,
        "status": value.get("status"),
        "response_sha256": value.get("response_sha256"),
        "prompt_sha256": value.get("prompt_sha256"),
        "candidate_attested": value.get("candidate_attested"),
        "failure_class": value.get("failure_class"),
        "source_review_attribution": attribution,
        "authority": authority,
    }


def derived_events(
    workspace: Path,
    inquiry: Iterable[dict[str, Any]],
    attestations: Iterable[tuple[Path, dict[str, Any], bytes]] = (),
) -> list[dict[str, Any]]:
    values: list[dict[str, Any]] = []
    inquiry = list(inquiry)
    receipts = read_json_lines(workspace / "introspections/scheduled/receipts.jsonl")
    attestation_by_receipt = {
        str(core.get("terminal_receipt_sha256")): core
        for _path, core, _reflection in attestations
        if exact_sha(core.get("terminal_receipt_sha256"))
    }
    by_step: dict[str, dict[str, Any]] = {}
    for receipt in receipts:
        try:
            receipt_hash = sha256(canonical(receipt))
        except (TypeError, ValueError, UnicodeEncodeError) as error:
            raise TrainError("scheduled receipt cannot be canonicalized") from error
        proof = attestation_by_receipt.get(receipt_hash)
        if receipt.get("schema") != "astrid_edge_scheduled_introspection_v2":
            continue
        if proof is None:
            raise TrainError("scheduled v2 receipt has no exact signed attestation")
        if (
            receipt.get("status") != proof.get("terminal_status")
            or receipt.get("response_sha256") != proof.get("response_sha256")
            or receipt.get("step_id") != proof.get("step_id")
            or receipt.get("signed_entry_id") != proof.get("signed_entry_id")
            or receipt.get("admission_id") != proof.get("admission_id")
            or receipt.get("trace") != proof.get("trace")
            or receipt.get("provenance") != proof.get("provenance")
        ):
            raise TrainError("scheduled v2 receipt does not match its signed attestation")
        if safe_id(receipt.get("step_id")):
            by_step[str(receipt["step_id"])] = receipt
    for event in inquiry:
        timestamp = int(event.get("timestamp_unix_ms", 0) or 0)
        values.append(
            {
                **{key: event.get(key) for key in ("trace_id", "turn_id", "span_id", "session_id", "thread_id", "step_id", "parent_step_id")},
                "timestamp_unix_ms": timestamp,
                "kind": "thread_transition",
                "status": event.get("thread_operation"),
                "authored": True,
                "fallback": False,
                "authorship_class": "astrid_authored_scheduled_thread_transition",
                "provenance_class": "astrid_authored",
                "source_ledger": event.get("source_ledger"),
                "authority": "signed_inquiry_semantic_parentage_not_timestamp_inference",
            }
        )
        receipt = by_step.get(str(event.get("step_id")))
        if receipt:
            for index, tool in enumerate(receipt.get("tools_used") or []):
                values.append(
                    {
                        **{key: event.get(key) for key in ("trace_id", "turn_id", "span_id", "session_id", "thread_id", "step_id")},
                        "timestamp_unix_ms": timestamp,
                        "kind": "model_tool_request",
                        "status": "used",
                        "authored": False,
                        "fallback": False,
                        "authorship_class": "model_requested_tool_machine_execution",
                        "provenance_class": "model_requested_tool",
                        "tool_name": tool,
                        "tool_index": index,
                        "source_ledger": "introspections/scheduled/receipts.jsonl",
                        "authority": "tool_use_is_not_reflection_authorship_or_code_authority",
                    }
                )
            if (
                receipt.get("source_review_relation") == "separate_clean_source_review"
                or receipt.get("source_review") is not None
            ):
                source_review = clean_source_review_projection(
                    receipt.get("source_review")
                )
                candidate_id = receipt.get("candidate_id")
                candidate_digest = receipt.get("candidate_digest")
                candidate_binding_valid = (
                    source_review is not None
                    and source_review.get("candidate_attested") is True
                    and safe_id(candidate_id)
                    and exact_sha(candidate_digest)
                )
                exact_projection = (
                    source_review is not None
                    and receipt.get("source_review_relation")
                    == "separate_clean_source_review"
                    and (
                        source_review.get("candidate_attested") is not True
                        or candidate_binding_valid
                    )
                )
                if not exact_projection:
                    source_review = {
                        "trace_id": None,
                        "turn_id": None,
                        "span_id": None,
                        "session_id": None,
                        "status": "unattributed_invalid_source_review_projection",
                        "response_sha256": None,
                        "prompt_sha256": None,
                        "candidate_attested": False,
                        "failure_class": None,
                        "source_review_attribution": (
                            "unattributed_no_clean_causal_identity"
                        ),
                        "authority": (
                            "unattributed_source_review_projection_"
                            "no_causal_or_candidate_claim"
                        ),
                    }
                assert source_review is not None
                candidate_attested = source_review["candidate_attested"] is True
                values.append(
                    {
                        "timestamp_unix_ms": timestamp,
                        "kind": "clean_source_review",
                        "status": source_review["status"],
                        "authored": False,
                        "fallback": False,
                        "authorship_class": "separate_clean_code_review_not_rich_reflection_patch_authorship",
                        "provenance_class": "separate_clean_code_review",
                        **{
                            key: source_review.get(key)
                            for key in (
                                "trace_id",
                                "turn_id",
                                "span_id",
                                "session_id",
                                "response_sha256",
                                "prompt_sha256",
                                "candidate_attested",
                                "failure_class",
                                "source_review_attribution",
                            )
                        },
                        "candidate_id": candidate_id if candidate_binding_valid else None,
                        "candidate_digest": (
                            candidate_digest if candidate_binding_valid else None
                        ),
                        "source_ledger": "introspections/scheduled/receipts.jsonl",
                        "authority": source_review["authority"],
                    }
                )
    admission_events, admission_index = semantic_admission_receipt_history(
        workspace, inquiry
    )
    values.extend(admission_events)
    ambiguity = current_admission_ambiguity_event(
        workspace, inquiry, admission_index
    )
    if ambiguity is not None:
        values.append(ambiguity)
    return values


def collect_train(
    workspace: Path,
    *,
    inquiry_root: Path | None = None,
    verify_key: Path = VERIFY_KEY,
    appliance_id: str | None = None,
    full: bool = False,
) -> dict[str, Any]:
    if appliance_id is None or inquiry_root is None:
        inferred_appliance, inferred_root = expected_identity(workspace)
        appliance_id = appliance_id or inferred_appliance
        inquiry_root = inquiry_root or inferred_root
    try:
        public_key, key_id = load_key(verify_key)
        attestations = preflight_attestation_inventory(
            workspace,
            appliance_id,
            public_key,
            key_id,
            full=full,
        )
        individually_attested, unstructured = attestation_events(
            workspace, appliance_id, attestations,
            full=full,
        )
        degraded_reason = None
        degraded_records: list[dict[str, str]] = []
        try:
            inquiry = full_chain_events(
                workspace,
                inquiry_root,
                appliance_id,
                public_key,
                key_id,
                full=full,
            )
            integrity = "full_signed_hash_chain_verified"
            structured_attestations = [
                core
                for _path, core, _reflection in attestations
                if core.get("terminal_status") == "model_authored_structured"
            ]
            if len(structured_attestations) != len(inquiry):
                raise TrainError(
                    "signed inquiry chain and protected authorship inventory counts differ",
                    path=workspace / "introspections/scheduled",
                )
            for event in inquiry:
                matches = [
                    core
                    for core in structured_attestations
                    if core.get("signed_entry_id") == event.get("signed_entry_id")
                    and core.get("step_id") == event.get("step_id")
                    and core.get("response_sha256") == event.get("response_sha256")
                    and core.get("trace", {}).get("trace_id") == event.get("trace_id")
                ]
                if len(matches) != 1:
                    raise TrainError(
                        "signed inquiry entry has no unique protected authorship attestation",
                        path=workspace / "introspections/scheduled",
                    )
        except TrainError as error:
            degraded_reason = terminal_safe(error)
            degraded_records.append(
                {
                    "path": terminal_safe(error.path or inquiry_root),
                    "reason": terminal_safe(error),
                    "classification": "full_chain_unavailable_individual_attestations_only",
                }
            )
            inquiry = individually_attested
            if not inquiry and not unstructured:
                raise error
            integrity = "degraded_individually_attested_no_full_chain_claim"
        events = [
            *inquiry,
            *unstructured,
            *derived_events(workspace, inquiry, attestations),
            *thread_events(workspace, inquiry),
        ]
        events = [
            event
            for event in events
            if int(event.get("timestamp_unix_ms", 0) or 0) > 0
        ]
        events.sort(
            key=lambda event: (
                int(event.get("timestamp_unix_ms", 0)),
                str(event.get("kind", "")),
                str(
                    event.get("step_id")
                    or event.get("evidence_id")
                    or event.get("revision_id")
                    or ""
                ),
            )
        )
        return {
            "schema": REPORT_SCHEMA,
            "generated_at_unix_ms": time.time_ns() // 1_000_000,
            "appliance_id": appliance_id,
            "workspace": str(workspace),
            "integrity": integrity,
            "degraded_reason": degraded_reason,
            "degraded_record_count": len(degraded_records),
            "degraded_records": degraded_records,
            "invalid_record_count": 0,
            "invalid_records": [],
            "key_id": key_id,
            "inquiry_step_count": len(inquiry),
            "events": events,
            "authority": "owner_private_authored_inquiry_observability_not_hidden_chain_of_thought_or_code_authority",
        }
    except TrainError as error:
        path = terminal_safe(error.path or "unknown_protected_train_path")
        reason = terminal_safe(error)
        timestamp = time.time_ns() // 1_000_000
        return {
            "schema": REPORT_SCHEMA,
            "generated_at_unix_ms": timestamp,
            "appliance_id": appliance_id,
            "workspace": str(workspace),
            "integrity": "invalid_protected_history",
            "degraded_reason": None,
            "degraded_record_count": 0,
            "degraded_records": [],
            "invalid_record_count": 1,
            "invalid_records": [
                {
                    "path": path,
                    "reason": reason,
                    "classification": "protected_train_integrity_violation",
                }
            ],
            "key_id": locals().get("key_id", "unavailable"),
            "inquiry_step_count": 0,
            "events": [
                {
                    "timestamp_unix_ms": timestamp,
                    "kind": "integrity_violation",
                    "status": "invalid_protected_history",
                    "authored": False,
                    "fallback": False,
                    "authorship_class": "integrity_failure_not_astrid_authorship",
                    "provenance_class": "operator_integrity_evidence",
                    "path": path,
                    "reason": reason,
                    "source_ledger": path,
                    "authority": "fail_closed_no_authorship_or_continuity_claim",
                }
            ],
            "authority": "invalid_protected_history_no_authorship_claim",
        }


def selected(
    report: dict[str, Any], args: argparse.Namespace, start_ms: int, end_ms: int
) -> list[dict[str, Any]]:
    kinds = set(args.kind or [])
    events = [
        event
        for event in report["events"]
        if start_ms <= int(event.get("timestamp_unix_ms", 0)) <= end_ms
        and (not args.thread_id or event.get("thread_id") == args.thread_id)
        and (not args.step_id or event.get("step_id") == args.step_id)
        and (not kinds or event.get("kind") in kinds)
    ]
    return events[-args.limit :]


def text_lines(report: dict[str, Any], events: Iterable[dict[str, Any]]) -> list[str]:
    lines = [
        f"INQUIRY_TRAIN integrity={report['integrity']} steps={report['inquiry_step_count']} authority=authored-intellectual-record-not-hidden-chain-of-thought"
    ]
    if report.get("invalid_record_count"):
        lines.append(
            f"INTEGRITY_INVALID records={report['invalid_record_count']} no_authorship_claim=true"
        )
        for record in report.get("invalid_records") or []:
            lines.append(
                "INVALID_RECORD "
                f"path={compact(record.get('path'), 180)} "
                f"reason={compact(record.get('reason'), 220)}"
            )
    if report.get("degraded_reason"):
        lines.append(
            f"DEGRADED records={report.get('degraded_record_count', 0)} "
            f"reason={compact(report['degraded_reason'], 220)}"
        )
    for event in events:
        common = f"{iso_time(int(event['timestamp_unix_ms']))} {str(event.get('kind', 'unknown')).upper()}"
        kind = event.get("kind")
        if kind == "inquiry_step":
            detail = (
                f"step={compact(event.get('step_id'), 34)} thread={compact(event.get('thread_id'), 34)} "
                f"op={event.get('thread_operation')} confidence={event.get('confidence')} "
                f"observed={compact(event.get('observation'))} interpreted={compact(event.get('interpretation'))} "
                f"uncertain={compact(event.get('uncertainty'))} decided={compact(event.get('decision'))}"
            )
            if event.get("reflection_text") is not None:
                detail += " exact_prose_json=" + json.dumps(
                    event["reflection_text"], ensure_ascii=True
                )
        elif kind == "evidence_arrival":
            detail = (
                f"evidence={compact(event.get('evidence_id'), 48)} type={event.get('evidence_kind')} "
                f"eligible={str(event.get('eligible_for_belief_update')).lower()} summary={compact(event.get('summary'))}"
            )
        elif kind == "belief_revision":
            detail = (
                f"belief={compact(event.get('belief_id'), 48)} op={event.get('operation')} "
                f"evidence={compact(','.join(event.get('evidence_ids') or []), 100)} claim={compact(event.get('claim'))}"
            )
        elif kind == "thread_transition":
            detail = f"thread={compact(event.get('thread_id'), 48)} transition={event.get('status')} parent={compact(event.get('parent_step_id'), 42)}"
        elif kind == "semantic_admission":
            detail = (
                f"admission={compact(event.get('admission_id'), 42)} status={event.get('status')} "
                f"generation={compact(event.get('reservoir_generation'), 40)} sequence={event.get('reservoir_sequence')}"
            )
        elif kind == "model_tool_request":
            detail = f"tool={event.get('tool_name')} class=model-requested-machine-execution"
        elif kind == "scheduled_reflection":
            detail = (
                "authored=true structured=false continuity=false "
                f"response={compact(event.get('response_sha256'), 28)}"
            )
            if event.get("reflection_text") is not None:
                detail += " exact_prose_json=" + json.dumps(
                    event["reflection_text"], ensure_ascii=True
                )
        elif kind == "integrity_violation":
            detail = (
                f"path={compact(event.get('path'), 120)} "
                f"reason={compact(event.get('reason'), 220)} no_authorship_claim=true"
            )
        else:
            detail = f"status={event.get('status')} authority={compact(event.get('authority'))}"
        lines.append(terminal_safe(f"{common} {detail} class={event.get('provenance_class')}"))
    return lines


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser(description="Verified owner-private Astrid inquiry train")
    value.add_argument("--workspace", type=Path)
    value.add_argument("--window-minutes", type=int, default=360)
    value.add_argument("--limit", type=int, default=100)
    value.add_argument("--thread-id")
    value.add_argument("--step-id")
    value.add_argument("--kind", action="append", choices=sorted(KINDS))
    value.add_argument("--follow", action="store_true")
    value.add_argument("--full", action="store_true")
    value.add_argument("--format", choices=("text", "json", "jsonl"), default="text")
    return value


def render(args: argparse.Namespace, seen: set[str]) -> tuple[set[str], bool]:
    workspace = args.workspace or Path.home() / ".astrid/home/default/edge"
    now = time.time_ns() // 1_000_000
    report = collect_train(workspace, full=args.full)
    events = selected(report, args, now - args.window_minutes * 60_000, now)
    fresh: list[dict[str, Any]] = []
    for event in events:
        identity = sha256(canonical(event))
        if identity not in seen:
            seen.add(identity)
            fresh.append(event)
    if args.format == "json":
        print(json.dumps({**report, "events": fresh}, sort_keys=True), flush=True)
    elif args.format == "jsonl":
        for event in fresh:
            print(json.dumps(event, sort_keys=True), flush=True)
    else:
        for line in text_lines(report, fresh):
            print(line, flush=True)
    return seen, report.get("integrity") != "invalid_protected_history"


def main() -> int:
    args = parser().parse_args()
    if args.window_minutes < 1 or args.window_minutes > 525_600:
        raise SystemExit("--window-minutes must be between 1 and 525600")
    if args.limit < 1 or args.limit > 10_000:
        raise SystemExit("--limit must be between 1 and 10000")
    if args.thread_id and not safe_id(args.thread_id):
        raise SystemExit("--thread-id is invalid")
    if args.step_id and not safe_id(args.step_id):
        raise SystemExit("--step-id is invalid")
    seen: set[str] = set()
    while True:
        try:
            seen, valid = render(args, seen)
        except (OSError, ValueError, TrainError, json.JSONDecodeError) as error:
            print(f"astrid-train: {terminal_safe(error)}", file=sys.stderr)
            return 2
        if not valid:
            return 2
        if not args.follow:
            return 0
        time.sleep(1)


if __name__ == "__main__":
    raise SystemExit(main())
