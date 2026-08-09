#!/usr/bin/env python3
"""Dependency-free read-only report for CPU Astrid appliances.

Host probes and artifact summaries intentionally remain in one copyable script
because deployed boxes may have only the Python standard library.  A later
split should preserve a single normalized key/value report contract and must
not make report collection stateful.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import importlib.util
import json
import math
import os
import pwd
import re
import stat
import subprocess
import sys
import time
import unicodedata
import urllib.request
import uuid
from pathlib import Path
from typing import Any


SERVICE_MANAGER_MARKER = Path("/etc/astrid/edge-service-manager.json")
TRUSTED_COMMANDS = {
    "journalctl": "/usr/bin/journalctl",
    "ps": "/usr/bin/ps",
    "systemctl": "/usr/bin/systemctl",
}
SYSTEM_SERVICE_MANAGER = False


LOCAL_HEADER_PATTERN = re.compile(
    r"^(?P<timestamp>\S+)\s+.*HTTP stream response headers received "
    r".*capsule_id=astrid-capsule-openai-compat "
    r"origin=http://(?:127\.0\.0\.1|\[::1\]|localhost):\d+ "
    r"elapsed_ms=(?P<elapsed>\d+)\s*$"
)
ESSENTIAL_EDGE_CAPSULES = frozenset(
    {
        "astrid-capsule-cli",
        "astrid-capsule-fs",
        "astrid-capsule-http",
        "astrid-capsule-shell",
        "astrid-capsule-skills",
        "astrid-capsule-agents",
        "astrid-capsule-memory",
        "astrid-capsule-edge-context",
        "astrid-capsule-edge-introspector",
        "astrid-capsule-edge-spectral",
    }
)
MODEL_RESPONSE_PROVENANCES = frozenset(
    {
        "model_authored",
        "model_authored_with_local_safe_fallback",
        "model_authored_with_local_format_repair",
    }
)
NON_AUTHORED_RESPONSE_PROVENANCE_BY_STATUS = {
    "transport_recovery": "transport_recovery_non_authored",
    "failed": "failed_non_authored",
    "interrupted": "interrupted_non_authored",
}
RESPONSE_PROVENANCE_LABELS = (
    "model_authored",
    "model_authored_with_local_safe_fallback",
    "model_authored_with_local_format_repair",
    "executor_terminal_error",
    "transport_recovery_non_authored",
    "failed_non_authored",
    "interrupted_non_authored",
    "legacy_unspecified",
    "invalid",
)
REQUEST_HEADER_LATENCY_SOURCE_V1 = "kernel_http_host_trace_v1"
CURRENT_CAPSULE_MANIFEST_SCHEMA = "astrid_headless_application_capsule_generation_v1"
CURRENT_CAPSULE_MANIFEST_NAME = "headless-application-capsules.current.json"
CURRENT_CAPSULE_SIDECAR_NAME = "headless-application-capsules.current.sha256"
REACT_CAPSULE_ID = "astrid-capsule-react"
SHA256_PATTERN = re.compile(r"[0-9a-f]{64}")
SCHEDULED_INTROSPECTION_STATE_SCHEMA = (
    "astrid_edge_scheduled_introspection_state_v1"
)
SCHEDULED_INTROSPECTION_RECEIPT_SCHEMA = (
    "astrid_edge_scheduled_introspection_v1"
)
SCHEDULED_INTROSPECTION_CONTINUITY_SCHEMA = (
    "astrid_edge_scheduled_introspection_continuity_v1"
)
SCHEDULED_INTROSPECTION_PROVENANCE = "model_authored_runtime_scheduled"
SCHEDULED_INTROSPECTION_ADMISSION_SCHEMA = (
    "astrid.edge.scheduled_introspection.admission.v1"
)
SCHEDULED_AUTHORSHIP_CORE_SCHEMA = (
    "astrid.edge.scheduled_authorship.attestation.v1"
)
SCHEDULED_AUTHORSHIP_ENVELOPE_SCHEMA = (
    "astrid.edge.scheduled_authorship.attestation_envelope.v1"
)
SCHEDULED_AUTHORSHIP_VERIFY_KEY = Path(
    "/etc/astrid/edge-scheduled-authorship.pub"
)
SCHEDULED_INTROSPECTION_LEDGER_PATHS = (
    "introspections/scheduled/receipts.jsonl",
    "introspection/scheduled/receipts.jsonl",
)
SELF_CHANGE_OPERATOR_STATUS_PATH = Path(
    "/var/lib/astrid-edge-operator/operator-status.json"
)
PATCH_EXPORT_SUMMARY_SCHEMA = (
    "astrid.edge.steward_helper.owner_patch_export_summary_envelope.v1"
)
PATCH_EXPORT_SUMMARY_CORE_SCHEMA = (
    "astrid.edge.steward_helper.owner_patch_export_summary.v1"
)
CANDIDATE_PRESENTATION_INPUT_SCHEMA = (
    "astrid.edge_candidate_presentation.input.v1"
)
CANDIDATE_PRESENTATION_CONTENT_SCHEMA = (
    "astrid.edge_candidate_presentation.content.v1"
)
CANDIDATE_PRESENTATION_INPUT_MAX_BYTES = 256 * 1024


def terminal_safe_text(value: Any) -> str:
    """Neutralize controls in the line-oriented owner report surface."""

    return "".join(
        " "
        if unicodedata.category(character) in {"Cc", "Cf", "Cs", "Zl", "Zp"}
        else character
        for character in str(value)
    )


def emit(name: str, value: Any) -> None:
    if isinstance(value, bool):
        value = str(value).lower()
    safe_name = " ".join(terminal_safe_text(name).split())
    safe_value = " ".join(terminal_safe_text(value).split())
    print(f"{safe_name}={safe_value}")


def candidate_presentation() -> int:
    """Render only broker-supplied sanitized facts as untrusted JSON."""

    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--candidate-presentation", action="store_true")
    parser.add_argument("--input-stdin", action="store_true")
    parser.add_argument("--window-minutes", type=int, required=True)
    parser.add_argument("--limit", type=int, required=True)
    parser.add_argument("--format", choices=("json",), required=True)
    args = parser.parse_args()
    if not args.candidate_presentation or not args.input_stdin:
        parser.error("the active-generation presentation requires broker stdin")
    raw = sys.stdin.buffer.read(CANDIDATE_PRESENTATION_INPUT_MAX_BYTES + 1)
    if len(raw) > CANDIDATE_PRESENTATION_INPUT_MAX_BYTES:
        parser.error("broker projection exceeds its bound")
    try:
        projection = json.loads(raw)
    except (UnicodeError, json.JSONDecodeError) as error:
        parser.error(f"broker projection is invalid: {error}")
    if (
        not isinstance(projection, dict)
        or projection.get("schema") != CANDIDATE_PRESENTATION_INPUT_SCHEMA
        or not isinstance(projection.get("facts"), list)
    ):
        parser.error("broker projection has the wrong schema")
    lines = []
    for fact in projection["facts"][:128]:
        if not isinstance(fact, dict):
            continue
        key = " ".join(terminal_safe_text(fact.get("key", "")).split())
        value = " ".join(terminal_safe_text(fact.get("value", "")).split())
        if key and value:
            lines.append(f"{key}={value}"[:240])
    sections = [
        {"heading": f"Sanitized facts {index + 1}", "lines": lines[index:index + 16]}
        for index in range(0, len(lines), 16)
    ][:12]
    result = {
        "schema": CANDIDATE_PRESENTATION_CONTENT_SCHEMA,
        "view": "appliance",
        "title": "Active-generation appliance view",
        "summary": (
            f"Candidate report arranged {len(lines)} sanitized immutable-report facts; "
            "this is untrusted presentation, not health evidence."
        ),
        "sections": sections,
    }
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return {}
    return value if isinstance(value, dict) else {}


def read_self_change_operator_status(
    path: Path = SELF_CHANGE_OPERATOR_STATUS_PATH,
    *,
    test_only_allow_unprivileged_owner: bool = False,
) -> dict[str, Any]:
    """Verify the shared bounded operator projection, never private ledgers."""

    source = Path(__file__).resolve().with_name("report_edge_activity.py")
    if not source.is_file():
        return {}
    spec = importlib.util.spec_from_file_location(
        "astrid_edge_activity_projection_reader", source
    )
    if spec is None or spec.loader is None:
        return {}
    module = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(module)
        value = module.read_self_change_operator_status(
            path,
            test_only_allow_unprivileged_owner=(
                test_only_allow_unprivileged_owner
            ),
        )
    except (OSError, ValueError, AttributeError):
        return {}
    return value if isinstance(value, dict) else {}


def _unavailable_react_provenance(source: str, validation: str) -> dict[str, Any]:
    return {
        "source": source,
        "validation": validation,
        "generation_state": "unavailable",
        "generation_id": "unavailable",
        "live_content_address": "unavailable",
        "archive_sha256": "unavailable",
        "manifest_sha256": "unavailable",
    }


def react_provenance_view(astrid_root: Path) -> dict[str, Any]:
    """Resolve React provenance, preferring the hashed generic current manifest."""
    manifest_root = astrid_root / "etc/install-manifests"
    current_path = manifest_root / CURRENT_CAPSULE_MANIFEST_NAME
    sidecar_path = manifest_root / CURRENT_CAPSULE_SIDECAR_NAME
    current_present = any(
        path.exists() or path.is_symlink() for path in (current_path, sidecar_path)
    )
    if current_present:
        if any(
            path.is_symlink() or not path.is_file()
            for path in (current_path, sidecar_path)
        ):
            return _unavailable_react_provenance(
                "current_generic_manifest_invalid", "current_files_not_regular"
            )
        try:
            payload = current_path.read_bytes()
            sidecar = sidecar_path.read_text(encoding="utf-8")
        except (OSError, UnicodeError):
            return _unavailable_react_provenance(
                "current_generic_manifest_invalid", "current_files_unreadable"
            )
        if len(payload) > 4 * 1024 * 1024 or len(sidecar) > 256:
            return _unavailable_react_provenance(
                "current_generic_manifest_invalid", "current_files_oversized"
            )
        sidecar_match = re.fullmatch(
            rf"(?P<digest>[0-9a-f]{{64}})  {re.escape(CURRENT_CAPSULE_MANIFEST_NAME)}\n",
            sidecar,
        )
        payload_sha256 = hashlib.sha256(payload).hexdigest()
        if (
            sidecar_match is None
            or sidecar_match.group("digest") != payload_sha256
        ):
            return _unavailable_react_provenance(
                "current_generic_manifest_invalid", "current_sidecar_mismatch"
            )
        try:
            manifest = json.loads(payload)
        except (UnicodeError, json.JSONDecodeError):
            return _unavailable_react_provenance(
                "current_generic_manifest_invalid", "current_manifest_malformed"
            )
        if (
            not isinstance(manifest, dict)
            or manifest.get("schema") != CURRENT_CAPSULE_MANIFEST_SCHEMA
        ):
            return _unavailable_react_provenance(
                "current_generic_manifest_invalid", "current_schema_mismatch"
            )
        generation_id = manifest.get("generation_id")
        capsules = manifest.get("capsules")
        if (
            not isinstance(generation_id, str)
            or not generation_id.strip()
            or len(generation_id) > 128
            or any(ord(character) < 0x20 or ord(character) == 0x7F for character in generation_id)
            or not isinstance(capsules, list)
            or not 1 <= len(capsules) <= 128
        ):
            return _unavailable_react_provenance(
                "current_generic_manifest_invalid", "current_manifest_malformed"
            )
        capsule_ids: set[str] = set()
        react_entries: list[dict[str, Any]] = []
        for entry in capsules:
            if not isinstance(entry, dict):
                return _unavailable_react_provenance(
                    "current_generic_manifest_invalid", "current_capsule_entry_malformed"
                )
            capsule_id = entry.get("capsule_id")
            if (
                not isinstance(capsule_id, str)
                or not capsule_id
                or len(capsule_id) > 128
                or capsule_id in capsule_ids
            ):
                return _unavailable_react_provenance(
                    "current_generic_manifest_invalid", "current_capsule_entry_malformed"
                )
            capsule_ids.add(capsule_id)
            if capsule_id == REACT_CAPSULE_ID:
                react_entries.append(entry)
        if len(react_entries) != 1:
            return _unavailable_react_provenance(
                "current_generic_manifest_invalid", "current_react_entry_mismatch"
            )
        react = react_entries[0]
        archive_sha256 = react.get("archive_sha256")
        normalized_sha256 = react.get("normalized_payload_sha256")
        installed_sha256 = react.get("installed_tree_sha256")
        if not all(
            isinstance(value, str) and SHA256_PATTERN.fullmatch(value) is not None
            for value in (archive_sha256, normalized_sha256, installed_sha256)
        ):
            return _unavailable_react_provenance(
                "current_generic_manifest_invalid", "current_react_entry_malformed"
            )
        content_objects = react.get("content_objects")
        if (
            not isinstance(content_objects, list)
            or not 1 <= len(content_objects) <= 256
        ):
            return _unavailable_react_provenance(
                "current_generic_manifest_invalid", "current_react_content_objects_malformed"
            )
        wasm_objects: list[dict[str, Any]] = []
        for content_object in content_objects:
            if not isinstance(content_object, dict):
                return _unavailable_react_provenance(
                    "current_generic_manifest_invalid",
                    "current_react_content_objects_malformed",
                )
            kind = content_object.get("kind")
            digest = content_object.get("digest")
            object_sha256 = content_object.get("sha256")
            if (
                kind not in {"wasm", "wit"}
                or not isinstance(digest, str)
                or SHA256_PATTERN.fullmatch(digest) is None
                or not isinstance(object_sha256, str)
                or SHA256_PATTERN.fullmatch(object_sha256) is None
            ):
                return _unavailable_react_provenance(
                    "current_generic_manifest_invalid",
                    "current_react_content_objects_malformed",
                )
            if kind == "wasm":
                wasm_objects.append(content_object)
        if len(wasm_objects) != 1:
            return _unavailable_react_provenance(
                "current_generic_manifest_invalid", "current_react_wasm_object_mismatch"
            )
        return {
            "source": "current_generic_manifest",
            "validation": "verified",
            "generation_state": "verified_current_generic_manifest",
            "generation_id": generation_id,
            "live_content_address": wasm_objects[0]["digest"],
            "archive_sha256": archive_sha256,
            "manifest_sha256": payload_sha256,
        }

    legacy_path = manifest_root / "react-provenance-v1.json"
    if legacy_path.exists() or legacy_path.is_symlink():
        legacy = read_json(legacy_path)
        deployment = legacy.get("deployment")
        if not isinstance(deployment, dict):
            deployment = {}
        artifact = legacy.get("artifact")
        if not isinstance(artifact, dict):
            artifact = {}
        return {
            "source": "legacy_manual_manifest",
            "validation": "legacy_manual_unverified",
            "generation_state": legacy.get("deployment_state", "unavailable"),
            "generation_id": "unavailable",
            "live_content_address": deployment.get(
                "live_content_address", "unavailable"
            ),
            "archive_sha256": artifact.get("sha256", "unavailable"),
            "manifest_sha256": "unavailable",
        }
    return _unavailable_react_provenance("unavailable", "not_found")


def read_json_lines(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_text().splitlines()
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


# Minimal, dependency-free RFC 8032 verifier for owner-facing observability.
# Runtime continuity uses ed25519-dalek; this independent implementation lets
# the immutable Python report reject forged workspace presentation copies on a
# stock Ubuntu installation without installing a crypto package.
_ED25519_P = 2**255 - 19
_ED25519_L = 2**252 + 27742317777372353535851937790883648493
_ED25519_D = (-121665 * pow(121666, _ED25519_P - 2, _ED25519_P)) % _ED25519_P
_ED25519_I = pow(2, (_ED25519_P - 1) // 4, _ED25519_P)
_ED25519_IDENTITY = (0, 1, 1, 0)


def _ed25519_recover_x(y: int, sign: int) -> int | None:
    if y >= _ED25519_P:
        return None
    y_squared = y * y % _ED25519_P
    x_squared = (y_squared - 1) * pow(
        (_ED25519_D * y_squared + 1) % _ED25519_P,
        _ED25519_P - 2,
        _ED25519_P,
    ) % _ED25519_P
    x = pow(x_squared, (_ED25519_P + 3) // 8, _ED25519_P)
    if (x * x - x_squared) % _ED25519_P:
        x = x * _ED25519_I % _ED25519_P
    if (x * x - x_squared) % _ED25519_P:
        return None
    if x & 1 != sign:
        x = _ED25519_P - x
    if x == 0 and sign:
        return None
    return x


def _ed25519_decode(encoded: bytes) -> tuple[int, int, int, int] | None:
    if len(encoded) != 32:
        return None
    value = int.from_bytes(encoded, "little")
    y = value & ((1 << 255) - 1)
    x = _ed25519_recover_x(y, value >> 255)
    if x is None:
        return None
    return (x, y, 1, x * y % _ED25519_P)


def _ed25519_add(
    left: tuple[int, int, int, int], right: tuple[int, int, int, int]
) -> tuple[int, int, int, int]:
    x1, y1, z1, t1 = left
    x2, y2, z2, t2 = right
    a = (y1 - x1) * (y2 - x2) % _ED25519_P
    b = (y1 + x1) * (y2 + x2) % _ED25519_P
    c = 2 * _ED25519_D * t1 * t2 % _ED25519_P
    d = 2 * z1 * z2 % _ED25519_P
    e = b - a
    f = d - c
    g = d + c
    h = b + a
    return (
        e * f % _ED25519_P,
        g * h % _ED25519_P,
        f * g % _ED25519_P,
        e * h % _ED25519_P,
    )


def _ed25519_double(point: tuple[int, int, int, int]) -> tuple[int, int, int, int]:
    x, y, z, _t = point
    a = x * x % _ED25519_P
    b = y * y % _ED25519_P
    c = 2 * z * z % _ED25519_P
    d = -a % _ED25519_P
    e = ((x + y) * (x + y) - a - b) % _ED25519_P
    g = (d + b) % _ED25519_P
    f = (g - c) % _ED25519_P
    h = (d - b) % _ED25519_P
    return (
        e * f % _ED25519_P,
        g * h % _ED25519_P,
        f * g % _ED25519_P,
        e * h % _ED25519_P,
    )


def _ed25519_scalar(
    point: tuple[int, int, int, int], scalar: int
) -> tuple[int, int, int, int]:
    result = _ED25519_IDENTITY
    current = point
    while scalar:
        if scalar & 1:
            result = _ed25519_add(result, current)
        current = _ed25519_double(current)
        scalar >>= 1
    return result


def _ed25519_equal(
    left: tuple[int, int, int, int], right: tuple[int, int, int, int]
) -> bool:
    return (
        (left[0] * right[2] - right[0] * left[2]) % _ED25519_P == 0
        and (left[1] * right[2] - right[1] * left[2]) % _ED25519_P == 0
    )


_ED25519_BASE_Y = 4 * pow(5, _ED25519_P - 2, _ED25519_P) % _ED25519_P
_ED25519_BASE_X = _ed25519_recover_x(_ED25519_BASE_Y, 0)
if _ED25519_BASE_X is None:  # pragma: no cover - fixed field constants
    raise RuntimeError("invalid Ed25519 base point constants")
_ED25519_BASE = (
    _ED25519_BASE_X,
    _ED25519_BASE_Y,
    1,
    _ED25519_BASE_X * _ED25519_BASE_Y % _ED25519_P,
)


def verify_ed25519(public_key: bytes, message: bytes, signature: bytes) -> bool:
    if len(public_key) != 32 or len(signature) != 64:
        return False
    public = _ed25519_decode(public_key)
    encoded_r = signature[:32]
    r_point = _ed25519_decode(encoded_r)
    scalar = int.from_bytes(signature[32:], "little")
    if public is None or r_point is None or scalar >= _ED25519_L:
        return False
    if _ed25519_equal(_ed25519_scalar(public, 8), _ED25519_IDENTITY):
        return False
    if _ed25519_equal(_ed25519_scalar(r_point, 8), _ED25519_IDENTITY):
        return False
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public_key + message).digest(), "little"
    ) % _ED25519_L
    return _ed25519_equal(
        _ed25519_scalar(_ED25519_BASE, scalar),
        _ed25519_add(r_point, _ed25519_scalar(public, challenge)),
    )


def canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")


def scheduled_authorship_attestations(
    workspace: Path,
    verify_key_path: Path = SCHEDULED_AUTHORSHIP_VERIFY_KEY,
) -> tuple[list[dict[str, Any]], int, str]:
    try:
        key_metadata = verify_key_path.lstat()
        public_key = verify_key_path.read_bytes()
    except OSError:
        return [], 0, "verify_key_absent"
    if (
        verify_key_path.is_symlink()
        or not stat.S_ISREG(key_metadata.st_mode)
        or key_metadata.st_nlink != 1
        or key_metadata.st_mode & 0o022
        or len(public_key) != 32
    ):
        return [], 0, "verify_key_invalid"
    key_sha256 = hashlib.sha256(public_key).hexdigest()
    expected_key_id = f"ed25519:{key_sha256[:16]}"
    root = workspace / "introspections/scheduled"
    try:
        paths = sorted(root.glob("authorship_attestation_due-*.json"))[-256:]
    except OSError:
        return [], 0, "attestation_directory_unreadable"
    valid: list[dict[str, Any]] = []
    invalid = 0
    envelope_fields = {"schema", "core", "auth"}
    core_fields = {
        "schema",
        "appliance_id",
        "due_nonce",
        "due_at_unix_ms",
        "started_at_unix_ms",
        "completed_at_unix_ms",
        "terminal_status",
        "model",
        "prompt_sha256",
        "response_sha256",
        "reflection_path",
        "reflection_sha256",
        "reflection_metadata_sha256",
        "continuity_projection_sha256",
        "state_projection_sha256",
        "terminal_receipt_sha256",
        "context_provenance_sha256",
        "candidate_id",
        "candidate_digest",
        "trace",
        "provenance",
        "authority",
    }
    auth_fields = {"algorithm", "key_id", "signature"}
    for path in paths:
        try:
            metadata = path.lstat()
            raw = path.read_bytes()
            envelope = json.loads(raw)
            core = envelope.get("core")
            auth = envelope.get("auth")
            signature_text = auth.get("signature") if isinstance(auth, dict) else None
            signature = bytes.fromhex(signature_text) if isinstance(signature_text, str) else b""
            unsigned = {
                "schema": SCHEDULED_AUTHORSHIP_ENVELOPE_SCHEMA,
                "core": core,
            }
            due_nonce = (
                str(core.get("due_nonce") or "")
                if isinstance(core, dict)
                else ""
            )
            due_match = re.fullmatch(r"due-([0-9]{5,20})", due_nonce)
            due_at = core.get("due_at_unix_ms") if isinstance(core, dict) else None
            started_at = (
                core.get("started_at_unix_ms") if isinstance(core, dict) else None
            )
            completed_at = (
                core.get("completed_at_unix_ms") if isinstance(core, dict) else None
            )
            ordered_times = (
                due_match is not None
                and isinstance(due_at, int)
                and not isinstance(due_at, bool)
                and isinstance(started_at, int)
                and not isinstance(started_at, bool)
                and isinstance(completed_at, int)
                and not isinstance(completed_at, bool)
                and due_at == int(due_match.group(1)) * 1_000
                and started_at >= due_at
                and completed_at >= started_at
            )
            reflection_path = (
                Path(str(core.get("reflection_path") or ""))
                if isinstance(core, dict)
                else Path()
            )
            reflection_path_valid = (
                not reflection_path.is_absolute()
                and len(reflection_path.parts) == 3
                and reflection_path.parts[:2] == ("introspections", "scheduled")
                and ".." not in reflection_path.parts
                and reflection_path.name.startswith(f"reflection_{due_nonce}_")
                and reflection_path.suffix == ".md"
            )
            candidate_id = core.get("candidate_id") if isinstance(core, dict) else None
            candidate_digest = (
                core.get("candidate_digest") if isinstance(core, dict) else None
            )
            candidate_link_valid = (candidate_id is None) == (candidate_digest is None)
            if candidate_id is not None:
                candidate_link_valid = (
                    candidate_link_valid
                    and valid_trace_label(candidate_id, required=True)
                    and SHA256_PATTERN.fullmatch(str(candidate_digest or "")) is not None
                )
            if (
                path.is_symlink()
                or not stat.S_ISREG(metadata.st_mode)
                or metadata.st_nlink != 1
                or metadata.st_mode & 0o022
                or len(raw) > 32 * 1024
                or not isinstance(envelope, dict)
                or set(envelope) != envelope_fields
                or envelope.get("schema") != SCHEDULED_AUTHORSHIP_ENVELOPE_SCHEMA
                or not isinstance(core, dict)
                or set(core) != core_fields
                or core.get("schema") != SCHEDULED_AUTHORSHIP_CORE_SCHEMA
                or core.get("terminal_status") != "authored_completed"
                or core.get("provenance") != SCHEDULED_INTROSPECTION_PROVENANCE
                or core.get("authority")
                != "immutable_steward_signed_exact_authorship_join"
                or not valid_trace_label(core.get("appliance_id"), required=True)
                or not valid_trace_label(core.get("model"), required=True)
                or not ordered_times
                or not reflection_path_valid
                or not candidate_link_valid
                or not isinstance(auth, dict)
                or set(auth) != auth_fields
                or auth.get("algorithm") != "ed25519"
                or auth.get("key_id") != expected_key_id
                or not isinstance(signature_text, str)
                or not re.fullmatch(r"[0-9a-f]{128}", signature_text)
                or any(
                    SHA256_PATTERN.fullmatch(str(core.get(field) or "")) is None
                    for field in (
                        "prompt_sha256",
                        "response_sha256",
                        "reflection_sha256",
                        "reflection_metadata_sha256",
                        "continuity_projection_sha256",
                        "state_projection_sha256",
                        "terminal_receipt_sha256",
                        "context_provenance_sha256",
                    )
                )
                or core.get("reflection_sha256") != core.get("response_sha256")
                or not valid_trace(core)
                or not verify_ed25519(
                    public_key, canonical_json_bytes(unsigned), signature
                )
            ):
                invalid += 1
                continue
        except (OSError, UnicodeError, ValueError, TypeError, json.JSONDecodeError):
            invalid += 1
            continue
        valid.append(core | {"attestation_path": path.name, "key_id": expected_key_id})
    valid.sort(
        key=lambda value: (
            int(value.get("completed_at_unix_ms", 0) or 0),
            str(value.get("due_nonce") or ""),
        )
    )
    return valid, invalid, "verified" if valid else "no_valid_attestations"


def bounded_private_text(data: bytes, maximum: int) -> tuple[str, bool]:
    """Return a one-line owner-view excerpt without terminal control bytes."""
    decoded = data.decode("utf-8", errors="replace")
    safe = terminal_safe_text(decoded)
    compacted = " ".join(safe.split())
    return compacted[:maximum], len(compacted) > maximum


def scheduled_introspection_receipts(
    workspace: Path,
) -> list[tuple[dict[str, Any], tuple[str, ...], int]]:
    """Merge current and legacy ledgers without counting exact copies twice."""
    merged: dict[str, tuple[dict[str, Any], list[str], int]] = {}
    sequence = 0
    for relative in SCHEDULED_INTROSPECTION_LEDGER_PATHS:
        for receipt in read_json_lines(workspace / relative):
            sequence += 1
            try:
                identity = json.dumps(
                    receipt,
                    sort_keys=True,
                    separators=(",", ":"),
                    ensure_ascii=True,
                    allow_nan=False,
                )
            except (TypeError, ValueError):
                identity = f"noncanonical:{relative}:{sequence}"
            existing = merged.get(identity)
            if existing is None:
                merged[identity] = (receipt, [relative], 1)
                continue
            value, sources, occurrences = existing
            if relative not in sources:
                sources.append(relative)
            merged[identity] = (value, sources, occurrences + 1)
    rows = [
        (receipt, tuple(sources), occurrences)
        for receipt, sources, occurrences in merged.values()
    ]
    rows.sort(
        key=lambda row: (
            int(row[0].get("completed_at_unix_ms", 0) or 0),
            json.dumps(row[0], sort_keys=True, default=str),
        )
    )
    return rows


def read_patch_export_summaries(workspace: Path) -> list[dict[str, Any]]:
    """Read only bounded, body-free, hash-linked candidate summaries."""
    root = workspace / "self-change/patch-outbox"
    try:
        paths = sorted(root.glob("candidate-change-*.summary.json"))
    except OSError:
        return []
    summaries: list[dict[str, Any]] = []
    for path in paths:
        try:
            if path.is_symlink() or not path.is_file() or path.stat().st_size > 16 * 1024:
                continue
            envelope = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, json.JSONDecodeError):
            continue
        if not isinstance(envelope, dict) or set(envelope) != {
            "schema",
            "core",
            "core_sha256",
            "auth",
        }:
            continue
        core = envelope.get("core")
        auth = envelope.get("auth")
        if (
            envelope.get("schema") != PATCH_EXPORT_SUMMARY_SCHEMA
            or not isinstance(core, dict)
            or core.get("schema") != PATCH_EXPORT_SUMMARY_CORE_SCHEMA
            or core.get("source_bodies_retained") is not False
            or core.get("authority")
            != "reporting_summary_only_never_reingested_or_authorizing"
            or not isinstance(auth, dict)
            or auth.get("algorithm") != "hmac-sha256"
            or not SHA256_PATTERN.fullmatch(str(envelope.get("core_sha256") or ""))
        ):
            continue
        try:
            encoded = json.dumps(
                core,
                sort_keys=True,
                separators=(",", ":"),
                ensure_ascii=True,
                allow_nan=False,
            ).encode("ascii")
        except (TypeError, ValueError, UnicodeEncodeError):
            continue
        if hashlib.sha256(encoded).hexdigest() != envelope["core_sha256"]:
            continue
        paths_value = core.get("touched_paths")
        counts = ("file_count", "added_lines", "removed_lines", "changed_lines")
        if (
            not isinstance(paths_value, list)
            or not 0 < len(paths_value) <= 25
            or any(
                not isinstance(value, str)
                or not value
                or len(value) > 240
                or value.startswith("/")
                or ".." in Path(value).parts
                for value in paths_value
            )
            or any(
                isinstance(core.get(name), bool)
                or not isinstance(core.get(name), int)
                or not 0 <= int(core[name]) <= 100_000
                for name in counts
            )
            or core["file_count"] != len(paths_value)
            or core["changed_lines"] > 4_000
        ):
            continue
        summaries.append(core | {"summary_path": path.name})
    return summaries


def configured_self_change_root(
    workspace: Path, profile: dict[str, str], home: Path
) -> Path:
    configured = profile.get("ASTRID_EDGE_SELF_CHANGE_ROOT", "").strip()
    if configured:
        path = Path(configured)
        return path if path.is_absolute() else home / path
    state_root = (
        workspace.parents[2]
        if workspace.name == "edge"
        and workspace.parent.name == "default"
        and workspace.parent.parent.name == "home"
        else workspace
    )
    default = state_root / "self-change"
    legacy = workspace / "self-change"
    if any(
        (default / relative).exists()
        for relative in ("status.json", "state.json", "ledgers")
    ):
        return default
    return legacy


def unix_timestamp_ms(value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return 0
    integer = int(value)
    if integer <= 0:
        return 0
    return integer * 1_000 if integer < 100_000_000_000 else integer


def scheduled_introspection_summary(
    workspace: Path,
    cutoff_ms: int,
    now_ms: int,
    verify_key_path: Path = SCHEDULED_AUTHORSHIP_VERIFY_KEY,
) -> dict[str, Any]:
    verified_projection: dict[str, Any] = {}
    isolated = workspace / "runtime/scheduled-introspection/projection"
    state = read_json(isolated / "state.json")
    if not state:
        state = read_json(workspace / "runtime/scheduled_introspection_state.json")
    continuity = read_json(isolated / "continuity.json")
    if not continuity:
        continuity = read_json(
            workspace / "runtime/scheduled_introspection_continuity.json"
        )
    sourced_receipts = scheduled_introspection_receipts(workspace)
    receipts = [item for item, _sources, _occurrences in sourced_receipts]
    attestations, invalid_attestations, attestation_status = (
        scheduled_authorship_attestations(workspace, verify_key_path)
    )
    attested_receipt_hashes = {
        str(item["terminal_receipt_sha256"]): item for item in attestations
    }
    window = [
        item
        for item in receipts
        if int(item.get("completed_at_unix_ms", 0) or 0) >= cutoff_ms
    ]
    admission = read_json(
        workspace / "runtime/scheduled-introspection/admission/state.json"
    )

    def continuity_integrity() -> tuple[bool, str]:
        required = {
            "schema",
            "appliance_id",
            "model",
            "due_nonce",
            "recorded_at_unix_ms",
            "summary",
            "summary_sha256",
            "response_sha256",
            "prompt_sha256",
            "reflection_path",
            "trace",
            "provenance",
            "authority",
            "context_provenance",
            "context_provenance_sha256",
            "candidate_authoring_eligible",
            "reflection_lane",
            "taint_causes",
        }
        if not continuity:
            return False, "absent"
        if (
            set(continuity) != required
            or continuity.get("schema")
            != SCHEDULED_INTROSPECTION_CONTINUITY_SCHEMA
            or continuity.get("provenance")
            != SCHEDULED_INTROSPECTION_PROVENANCE
            or continuity.get("authority")
            != "bounded_continuity_projection_not_voluntary_journal"
            or not valid_trace(continuity)
            or not isinstance(continuity.get("summary"), str)
            or not 0 < len(continuity["summary"]) <= 320
            or not SHA256_PATTERN.fullmatch(
                str(continuity.get("summary_sha256") or "")
            )
            or hashlib.sha256(continuity["summary"].encode()).hexdigest()
            != continuity["summary_sha256"]
            or any(
                SHA256_PATTERN.fullmatch(str(continuity.get(field) or ""))
                is None
                for field in ("response_sha256", "prompt_sha256")
            )
        ):
            return False, "projection_invalid"
        relative = Path(str(continuity.get("reflection_path") or ""))
        if (
            relative.is_absolute()
            or len(relative.parts) != 3
            or relative.parts[:2] != ("introspections", "scheduled")
            or ".." in relative.parts
            or not relative.name.startswith("reflection_due-")
            or relative.suffix != ".md"
        ):
            return False, "reflection_path_invalid"
        reflection = workspace / relative
        sidecar = reflection.with_suffix(".json")
        try:
            if (
                reflection.is_symlink()
                or sidecar.is_symlink()
                or not reflection.is_file()
                or not sidecar.is_file()
                or reflection.stat().st_size > 64 * 1024
                or sidecar.stat().st_size > 16 * 1024
            ):
                return False, "reflection_files_invalid"
            response = reflection.read_bytes()
            metadata_bytes = sidecar.read_bytes()
            metadata = json.loads(metadata_bytes)
        except (OSError, UnicodeError, json.JSONDecodeError):
            return False, "reflection_files_unreadable"
        trace = continuity["trace"]
        expected_metadata = {
            "schema": "astrid.edge.scheduled_introspection.model_reflection.v1",
            "provenance": SCHEDULED_INTROSPECTION_PROVENANCE,
            "appliance_id": continuity["appliance_id"],
            "due_nonce": continuity["due_nonce"],
            "trace_id": trace["trace_id"],
            "session_id": trace["session_id"],
            "turn_id": trace["turn_id"],
            "model": continuity["model"],
            "prompt_sha256": continuity["prompt_sha256"],
            "response_sha256": continuity["response_sha256"],
            "exact_response_path": reflection.name,
            "context_provenance": continuity["context_provenance"],
            "context_provenance_sha256": continuity[
                "context_provenance_sha256"
            ],
            "reflection_lane": continuity["reflection_lane"],
            "taint_causes": continuity["taint_causes"],
        }
        if (
            not isinstance(metadata, dict)
            or metadata != expected_metadata
            or hashlib.sha256(response).hexdigest()
            != continuity["response_sha256"]
        ):
            return False, "reflection_hash_or_metadata_mismatch"
        try:
            continuity_sha256 = hashlib.sha256(
                canonical_json_bytes(continuity)
            ).hexdigest()
            state_sha256 = hashlib.sha256(canonical_json_bytes(state)).hexdigest()
            metadata_sha256 = hashlib.sha256(metadata_bytes).hexdigest()
        except (TypeError, ValueError, UnicodeEncodeError):
            return False, "projection_not_canonical"
        matching = [
            item
            for item in attestations
            if item.get("continuity_projection_sha256") == continuity_sha256
            and item.get("state_projection_sha256") == state_sha256
            and item.get("reflection_metadata_sha256") == metadata_sha256
            and item.get("reflection_sha256")
            == hashlib.sha256(response).hexdigest()
            and item.get("response_sha256") == continuity.get("response_sha256")
            and item.get("prompt_sha256") == continuity.get("prompt_sha256")
            and item.get("due_nonce") == continuity.get("due_nonce")
            and item.get("model") == continuity.get("model")
            and item.get("appliance_id") == continuity.get("appliance_id")
            and item.get("reflection_path") == continuity.get("reflection_path")
            and item.get("trace") == continuity.get("trace")
        ]
        if len(matching) != 1:
            return False, "immutable_authorship_attestation_join_failed"
        verified_projection["authorship_attestation"] = matching[0]
        excerpt, truncated = bounded_private_text(response, 800)
        verified_projection["reflection_excerpt"] = excerpt
        verified_projection["reflection_excerpt_truncated"] = truncated
        return True, "projection_artifact_sidecar_hash_verified"

    continuity_valid, continuity_validation = continuity_integrity()
    admission_valid = (
        continuity_valid
        and admission.get("schema") == SCHEDULED_INTROSPECTION_ADMISSION_SCHEMA
        and admission.get("continuity_admitted") is True
        and admission.get("provenance") == SCHEDULED_INTROSPECTION_PROVENANCE
        and admission.get("authority")
        == "runtime_verified_projection_observational_only"
        and admission.get("last_response_sha256")
        == continuity.get("response_sha256")
        and admission.get("last_summary_sha256")
        == continuity.get("summary_sha256")
        and admission.get("last_trace_id")
        == continuity.get("trace", {}).get("trace_id")
        and admission.get("last_due_nonce") == continuity.get("due_nonce")
    )

    def is_authored(item: dict[str, Any]) -> bool:
        try:
            receipt_sha256 = hashlib.sha256(canonical_json_bytes(item)).hexdigest()
        except (TypeError, ValueError, UnicodeEncodeError):
            return False
        attestation = attested_receipt_hashes.get(receipt_sha256)
        return (
            attestation is not None
            and item.get("schema") == SCHEDULED_INTROSPECTION_RECEIPT_SCHEMA
            and item.get("status") == "authored_completed"
            and item.get("provenance") == SCHEDULED_INTROSPECTION_PROVENANCE
            and (
                item.get("continuity_projection_written") is True
                or item.get("continuity_admitted") is True
            )
            and isinstance(item.get("response_sha256"), str)
            and SHA256_PATTERN.fullmatch(str(item["response_sha256"])) is not None
            and valid_trace(item)
            and item.get("due_nonce") == attestation.get("due_nonce")
            and item.get("response_sha256") == attestation.get("response_sha256")
            and item.get("prompt_sha256") == attestation.get("prompt_sha256")
            and item.get("trace") == attestation.get("trace")
        )

    latest = receipts[-1] if receipts else {}
    latest_sources = sourced_receipts[-1][1] if sourced_receipts else ()
    completed_at = int(state.get("last_completed_at_unix_ms", 0) or 0)
    reflections = workspace / "introspections/scheduled"
    try:
        reflection_count = sum(
            path.is_file() and not path.is_symlink()
            for path in reflections.glob("reflection_*.md")
        )
    except OSError:
        reflection_count = 0
    return {
        "state_present": bool(state),
        "state_schema": state.get("schema", "none"),
        "state_schema_supported": state.get("schema")
        == SCHEDULED_INTROSPECTION_STATE_SCHEMA,
        "running": state.get("running", False),
        "last_status": state.get("last_status", "none"),
        "last_started_at_unix_ms": state.get("last_started_at_unix_ms", 0) or 0,
        "last_completed_at_unix_ms": completed_at,
        "last_completed_age_ms": (
            max(0, now_ms - completed_at) if completed_at else "unavailable"
        ),
        "next_due_at_unix_ms": state.get("next_due_at_unix_ms", 0) or 0,
        "total_attempts": state.get("total_attempts", 0),
        "total_authored": state.get("total_authored", 0),
        "consecutive_failures": state.get("consecutive_failures", 0),
        "window_receipts": len(window),
        "window_current_ledger_records": sum(
            SCHEDULED_INTROSPECTION_LEDGER_PATHS[0] in sources
            for item, sources, _occurrences in sourced_receipts
            if int(item.get("completed_at_unix_ms", 0) or 0) >= cutoff_ms
        ),
        "window_legacy_ledger_records": sum(
            SCHEDULED_INTROSPECTION_LEDGER_PATHS[1] in sources
            for item, sources, _occurrences in sourced_receipts
            if int(item.get("completed_at_unix_ms", 0) or 0) >= cutoff_ms
        ),
        "window_exact_duplicates_merged": sum(
            occurrences - 1
            for item, _sources, occurrences in sourced_receipts
            if int(item.get("completed_at_unix_ms", 0) or 0) >= cutoff_ms
        ),
        "window_authored": sum(is_authored(item) for item in window),
        "window_non_authored_excluded": sum(not is_authored(item) for item in window),
        "window_transport_recoveries": sum(
            item.get("status") == "transport_recovery" for item in window
        ),
        "latest_receipt_status": latest.get("status", "none"),
        "latest_receipt_provenance": latest.get("provenance", "none"),
        "latest_receipt_source_ledger": latest_sources[0] if latest_sources else "none",
        "latest_receipt_source_ledgers": ",".join(latest_sources) or "none",
        "latest_reflection_path": latest.get("reflection_path")
        or state.get("last_artifact_path")
        or "none",
        "latest_response_sha256": latest.get("response_sha256")
        or state.get("last_response_sha256")
        or "none",
        "latest_candidate_id": latest.get("candidate_id") or "none",
        "latest_candidate_digest": latest.get("candidate_digest") or "none",
        "continuity_present": bool(continuity),
        "continuity_schema": continuity.get("schema", "none"),
        "continuity_schema_supported": continuity.get("schema")
        == SCHEDULED_INTROSPECTION_CONTINUITY_SCHEMA,
        "continuity_provenance": continuity.get("provenance", "none"),
        "continuity_summary": (
            bounded_private_text(str(continuity.get("summary")).encode(), 320)[0]
            if continuity_valid
            else "unavailable"
        ),
        "continuity_reflection_path": continuity.get("reflection_path") or "none",
        "continuity_integrity_valid": continuity_valid,
        "continuity_validation": continuity_validation,
        "continuity_actual_admitted": admission_valid,
        "authorship_attestation_status": attestation_status,
        "authorship_attestations_valid": len(attestations),
        "authorship_attestations_invalid": invalid_attestations,
        "authorship_attestation_key_id": (
            attestations[-1].get("key_id", "none") if attestations else "none"
        ),
        "authorship_attestation_path": (
            attestations[-1].get("attestation_path", "none")
            if attestations
            else "none"
        ),
        "continuity_admission_state_schema": admission.get("schema", "none"),
        "continuity_admitted_at_unix_ms": admission.get(
            "admitted_at_unix_ms", 0
        )
        if admission_valid
        else 0,
        "verified_reflection_excerpt": verified_projection.get(
            "reflection_excerpt", "unavailable"
        ),
        "verified_reflection_excerpt_truncated": verified_projection.get(
            "reflection_excerpt_truncated", False
        ),
        "reflection_text_authority": (
            "owner_private_hash_verified_model_authored_runtime_scheduled"
            if continuity_valid
            else "unavailable"
        ),
        "reflection_artifact_count": reflection_count,
    }


def self_change_summary(
    workspace: Path,
    root: Path,
    cutoff_ms: int,
    operator_status_path: Path = SELF_CHANGE_OPERATOR_STATUS_PATH,
    *,
    test_only_allow_unprivileged_operator_status: bool = False,
) -> dict[str, Any]:
    operator = read_self_change_operator_status(
        operator_status_path,
        test_only_allow_unprivileged_owner=(
            test_only_allow_unprivileged_operator_status
        ),
    )
    state_source = (
        "immutable_operator_projection_hash_verified"
        if operator
        else "unavailable"
    )
    pipeline_phase = str(operator.get("pipeline_phase") or "unavailable")
    intents_root = workspace / "self-change/outbox"
    try:
        intent_paths = sorted(intents_root.glob("intent_*.json"))
    except OSError:
        intent_paths = []
    intents = [value for path in intent_paths if (value := read_json(path))]
    latest_intent = intents[-1] if intents else {}
    patch_summaries = read_patch_export_summaries(workspace)
    latest_patch = patch_summaries[-1] if patch_summaries else {}
    lifecycle = operator.get("lifecycle")
    lifecycle = lifecycle if isinstance(lifecycle, dict) else {}
    events = lifecycle.get("events")
    events = events if isinstance(events, list) else []

    def latest(facet: str) -> dict[str, Any]:
        return next(
            (
                value
                for value in reversed(events)
                if facet in (value.get("facets") or [])
            ),
            {},
        )

    latest_reflection = latest("reflection")
    latest_candidate = latest("candidate")
    latest_build = latest("build")
    latest_test = latest("test")
    latest_invariant = latest("invariant")
    latest_shadow = latest("shadow")
    latest_activation = latest("activation")
    latest_restart = latest("restart")
    latest_probation = latest("probation")
    latest_rollback = latest("rollback")
    window_records = [
        value
        for value in events
        if unix_timestamp_ms(value.get("recorded_at")) >= cutoff_ms
    ]
    ledger_heads = lifecycle.get("ledger_heads")
    ledger_heads = ledger_heads if isinstance(ledger_heads, dict) else {}
    restart = operator.get("restart_expectation")
    restart = restart if isinstance(restart, dict) else {}
    return {
        "root": str(root),
        "private_root_read": False,
        "private_ledger_policy": "0600_secret_not_read_by_operator_reports",
        "status_present": False,
        "status_schema": "private_not_read",
        "operator_status_present": bool(operator),
        "operator_status_schema": operator.get("schema", "none"),
        "operator_status_generated_at_unix_s": operator.get("generated_at", 0),
        "operator_status_provenance": operator.get("provenance", "none"),
        "operator_pipeline_phase": operator.get("pipeline_phase", "unavailable"),
        "expected_restart_phase": restart.get("phase", "unavailable"),
        "expected_restart_maximum_seconds": restart.get(
            "maximum_seconds", "unavailable"
        ),
        "expected_restart_basis": restart.get("basis", "unavailable"),
        "operator_latest_transition_operation": (
            operator.get("latest_transition", {}).get("operation", "none")
            if isinstance(operator.get("latest_transition"), dict)
            else "none"
        ),
        "operator_latest_transition_status": (
            operator.get("latest_transition", {}).get("status", "none")
            if isinstance(operator.get("latest_transition"), dict)
            else "none"
        ),
        "state_present": bool(operator),
        "projection_core_sha256": operator.get("projection_core_sha256", "none"),
        "lifecycle_projection_schema": lifecycle.get("schema", "none"),
        "lifecycle_projection_included": lifecycle.get("included", 0),
        "lifecycle_projection_total": lifecycle.get("total", 0),
        "lifecycle_projection_truncated": lifecycle.get("truncated", False),
        "state_source": state_source,
        "state_integrity": (
            "narrow_projection_sha256_verified_filesystem_origin_observational"
            if state_source == "immutable_operator_projection_hash_verified"
            else "not_reverified_by_read_only_report"
        ),
        "state_schema": operator.get("schema", "none"),
        "state_revision": operator.get("state_revision", 0),
        "mode": operator.get("mode", "unavailable"),
        "paused_reason": "not_projected",
        "active_generation": operator.get("active_generation") or "none",
        "previous_generation": operator.get("previous_generation") or "none",
        "due_status": "pending" if pipeline_phase == "due" else "none",
        "due_not_before_unix_s": 0,
        "due_reasons": "not_projected",
        "inflight_status": pipeline_phase if pipeline_phase not in {"idle", "due"} else "none",
        "inflight_build_id": latest_build.get("build_id", "none"),
        "inflight_from_generation": latest_activation.get("from_generation", "none"),
        "inflight_to_generation": latest_activation.get("generation_id", "none"),
        "probation_status": "active" if pipeline_phase == "probation" else "none",
        "probation_build_id": latest_probation.get("build_id", "none"),
        "probation_generation_id": latest_probation.get("generation_id", "none"),
        "probation_not_before_unix_s": 0,
        "probation_health_checks": 0,
        "intent_total": len(intents),
        "intent_window": sum(
            unix_timestamp_ms(item.get("recorded_at_unix_ms")) >= cutoff_ms
            for item in intents
        ),
        "latest_intent_candidate_id": latest_intent.get("candidate_id", "none"),
        "latest_intent_candidate_digest": latest_intent.get("candidate_digest", "none"),
        "latest_intent_provenance": latest_intent.get("provenance", "none"),
        "patch_export_summary_total": len(patch_summaries),
        "patch_export_summary_window": sum(
            unix_timestamp_ms(item.get("recorded_at")) >= cutoff_ms
            for item in patch_summaries
        ),
        "latest_patch_candidate_id": latest_patch.get("candidate_id", "none"),
        "latest_patch_terminal_status": latest_patch.get("terminal_status", "none"),
        "latest_patch_file_count": latest_patch.get("file_count", 0),
        "latest_patch_changed_lines": latest_patch.get("changed_lines", 0),
        "latest_patch_touched_paths": ",".join(latest_patch.get("touched_paths", [])),
        "latest_patch_source_bodies_retained": latest_patch.get(
            "source_bodies_retained", "not_applicable"
        ),
        "latest_reflection_status": latest_reflection.get("status", "none"),
        "latest_reflection_response_sha256": latest_reflection.get(
            "response_sha256", "none"
        ),
        "latest_candidate_status": latest_candidate.get("status", "none"),
        "latest_candidate_id": latest_candidate.get("candidate_id", "none"),
        "latest_candidate_sha256": latest_candidate.get("candidate_sha256", "none"),
        "latest_terminal_reason_sha256": latest_candidate.get(
            "terminal_reason_sha256", "none"
        ),
        "latest_terminal_authority": latest_candidate.get(
            "terminal_authority", "none"
        ),
        "latest_build_status": latest_build.get("status", "none"),
        "latest_build_id": latest_build.get("build_id", "none"),
        "latest_tests_status": latest_test.get("status", "none"),
        "latest_tests_sha256": latest_test.get("tests_sha256", "none"),
        "latest_invariant_status": latest_invariant.get("status", "none"),
        "latest_invariant_candidate_replay_sha256": latest_invariant.get(
            "invariant_candidate_replay_sha256", "none"
        ),
        "latest_invariant_package_replay_sha256": latest_invariant.get(
            "invariant_package_replay_sha256", "none"
        ),
        "latest_shadow_status": latest_shadow.get("shadow_status", "none"),
        "latest_shadow_evidence_sha256": latest_shadow.get(
            "shadow_evidence_sha256", "none"
        ),
        "latest_activation_status": latest_activation.get("status", "none"),
        "latest_activation_generation": latest_activation.get(
            "generation_id", "none"
        ),
        "latest_restart_status": latest_restart.get("status", "none"),
        "latest_probation_status": latest_probation.get("status", "none"),
        "latest_rollback_status": latest_rollback.get("status", "none"),
        "window_lifecycle_records": len(window_records),
        **{
            f"ledger_{name}_records": sum(
                value.get("source_ledger") == name for value in events
            )
            for name in ("candidate", "build", "activation", "operator")
        },
        **{
            f"ledger_{name}_legacy_records": 0
            for name in ("candidate", "build", "activation", "operator")
        },
        **{
            f"ledger_{name}_reported_valid": (
                "projection_hash_verified"
                if operator and ledger_heads.get(name) is not None
                else "unavailable"
            )
            for name in ("candidate", "build", "activation", "operator")
        },
    }


def normalized_uuid(value: Any) -> str | None:
    try:
        parsed = uuid.UUID(str(value))
    except (TypeError, ValueError, AttributeError):
        return None
    return str(parsed) if parsed.int != 0 else None


def valid_trace_label(value: Any, *, required: bool) -> bool:
    if value is None:
        return not required
    if not isinstance(value, str) or not value.strip() or len(value) > 96:
        return False
    return not any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)


def valid_trace(value: dict[str, Any]) -> bool:
    trace = value.get("trace")
    if not isinstance(trace, dict) or trace.get("schema_version", 1) != 1:
        return False
    span_id = normalized_uuid(trace.get("span_id"))
    parent_span_id = normalized_uuid(trace.get("parent_span_id"))
    if normalized_uuid(trace.get("trace_id")) is None or span_id is None:
        return False
    if trace.get("parent_span_id") is not None and parent_span_id is None:
        return False
    if span_id == parent_span_id:
        return False
    if trace.get("turn_id") is not None and normalized_uuid(trace.get("turn_id")) is None:
        return False
    return valid_trace_label(trace.get("session_id"), required=True) and valid_trace_label(
        trace.get("chain_id"), required=False
    )


def exact_provider_request_telemetry(
    run: dict[str, Any],
) -> tuple[int, int, list[dict[str, Any]]] | None:
    """Return strict kernel-host attempts from one canonical turn."""
    request_count = run.get("provider_request_count")
    successful_count = run.get("provider_successful_header_count")
    requests = run.get("provider_requests")
    trace = run.get("trace")
    if (
        run.get("schema") != "astrid_edge_autonomy_run_v4"
        or run.get("request_header_latency_source")
        != REQUEST_HEADER_LATENCY_SOURCE_V1
        or not valid_trace(run)
        or not isinstance(trace, dict)
        or normalized_uuid(trace.get("turn_id")) is None
        or isinstance(request_count, bool)
        or not isinstance(request_count, int)
        or not 1 <= request_count <= 16
        or isinstance(successful_count, bool)
        or not isinstance(successful_count, int)
        or not 0 <= successful_count <= request_count
        or not isinstance(requests, list)
        or len(requests) != request_count
    ):
        return None
    accepted_outcomes = {
        "successful_headers",
        "non_success_status",
        "unknown_peer",
        "non_loopback_peer",
        "timeout",
        "transport_error",
        "cancelled",
    }
    attempt_ids: set[str] = set()
    observed_successes = 0
    for request in requests:
        if not isinstance(request, dict) or not {
            "attempt_id",
            "request_id",
            "outcome",
        }.issubset(request) or not set(request).issubset(
            {"attempt_id", "request_id", "outcome", "request_header_latency_ms"}
        ):
            return None
        attempt_id = normalized_uuid(request.get("attempt_id"))
        request_id = normalized_uuid(request.get("request_id"))
        outcome = request.get("outcome")
        latency = request.get("request_header_latency_ms")
        if (
            attempt_id is None
            or attempt_id in attempt_ids
            or request_id is None
            or not isinstance(outcome, str)
            or outcome not in accepted_outcomes
        ):
            return None
        attempt_ids.add(attempt_id)
        if outcome == "successful_headers":
            if (
                isinstance(latency, bool)
                or not isinstance(latency, int)
                or not 0 <= latency <= (2**64 - 1)
            ):
                return None
            observed_successes += 1
        elif latency is not None:
            return None
    if observed_successes != successful_count:
        return None
    if request_count == 1 and successful_count == 1:
        only = requests[0]
        scalar_request_id = normalized_uuid(run.get("provider_request_id"))
        scalar_latency = run.get("request_header_latency_ms")
        if (
            scalar_request_id is None
            or scalar_request_id != normalized_uuid(only.get("request_id"))
            or isinstance(scalar_latency, bool)
            or not isinstance(scalar_latency, int)
            or not 0 <= scalar_latency <= (2**64 - 1)
            or scalar_latency != only.get("request_header_latency_ms")
        ):
            return None
    elif (
        run.get("provider_request_id") is not None
        or run.get("request_header_latency_ms") is not None
    ):
        return None
    return request_count, successful_count, requests


def exact_provider_request_counts(run: dict[str, Any]) -> tuple[int, int] | None:
    telemetry = exact_provider_request_telemetry(run)
    return (telemetry[0], telemetry[1]) if telemetry is not None else None


def exact_request_header_latency(
    run: dict[str, Any],
) -> tuple[int, str, int] | None:
    """Return only a one-attempt/one-success trace-bound kernel-host latency."""
    if exact_provider_request_counts(run) != (1, 1):
        return None
    latency = run.get("request_header_latency_ms")
    request_id = normalized_uuid(run.get("provider_request_id"))
    if (
        isinstance(latency, bool)
        or not isinstance(latency, int)
        or not 0 <= latency <= (2**64 - 1)
        or request_id is None
    ):
        return None
    return latency, request_id, 1


def legacy_unattributed_header_latency(run: dict[str, Any]) -> int | float | None:
    """Return only genuinely legacy numeric latency, never a malformed exact claim."""
    latency = run.get("request_header_latency_ms")
    if (
        run.get("request_header_latency_source") is not None
        or run.get("provider_request_id") is not None
        or run.get("provider_request_count") is not None
        or run.get("provider_successful_header_count") is not None
        or run.get("provider_requests") is not None
        or isinstance(latency, bool)
        or not isinstance(latency, (int, float))
        or not math.isfinite(latency)
        or not 0 <= latency <= (2**64 - 1)
    ):
        return None
    return latency


def valid_response_sha256(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def run_response_provenance(item: dict[str, Any]) -> tuple[str, bool, bool]:
    raw = item.get("response_provenance")
    if raw is None:
        status = item.get("status")
        provenance = (
            NON_AUTHORED_RESPONSE_PROVENANCE_BY_STATUS.get(status)
            if item.get("schema") == "astrid_edge_autonomy_run_v4"
            and isinstance(status, str)
            else None
        ) or "legacy_unspecified"
    elif isinstance(raw, str) and raw in MODEL_RESPONSE_PROVENANCES | {
        "executor_terminal_error"
    }:
        provenance = str(raw)
    else:
        provenance = "invalid"
    return (
        provenance,
        provenance == "model_authored_with_local_safe_fallback",
        provenance == "model_authored_with_local_format_repair",
    )


def response_provenance_counts(runs: list[dict[str, Any]]) -> dict[str, int]:
    """Return a stable zero-filled count surface for all recognized labels."""
    counts = {label: 0 for label in RESPONSE_PROVENANCE_LABELS}
    for run in runs:
        provenance = run_response_provenance(run)[0]
        counts[provenance] += 1
    return counts


def latest_response_provenance(
    autonomy: dict[str, Any], runs: list[dict[str, Any]]
) -> tuple[str, bool, bool]:
    """Prefer the latest durable run when state cleared its provenance."""
    state_provenance = autonomy.get("last_response_provenance")
    if state_provenance is not None:
        return run_response_provenance({"response_provenance": state_provenance})
    if runs:
        return run_response_provenance(runs[-1])
    return run_response_provenance({})


DispatchCorrelationKey = tuple[str, str, str, str | None, str, str]


def dispatch_key(item: dict[str, Any]) -> DispatchCorrelationKey | None:
    """Return turn, trace, session, chain, response, and dispatch span."""
    turn_id = normalized_uuid(item.get("turn_id"))
    trace = item.get("trace")
    if (
        item.get("schema") != "astrid_edge_action_dispatch_v1"
        or item.get("phase") not in {"requested", "completed"}
        or turn_id is None
        or not valid_response_sha256(item.get("response_sha256"))
        or not valid_trace(item)
        or not isinstance(trace, dict)
        or normalized_uuid(trace.get("turn_id")) != turn_id
    ):
        return None
    return (
        turn_id,
        str(normalized_uuid(trace["trace_id"])),
        str(trace["session_id"]),
        str(trace["chain_id"]) if trace.get("chain_id") is not None else None,
        str(item["response_sha256"]),
        str(normalized_uuid(trace["span_id"])),
    )


def action_receipt_dispatch_key(item: dict[str, Any]) -> DispatchCorrelationKey | None:
    """Bind an Action child span to the exact dispatch span that parented it."""
    trace = item.get("trace")
    if (
        item.get("schema") != "astrid_edge_action_receipt_v4"
        or not valid_trace(item)
        or not isinstance(trace, dict)
        or (turn_id := normalized_uuid(trace.get("turn_id"))) is None
        or (parent_span_id := normalized_uuid(trace.get("parent_span_id"))) is None
        or item.get("session_id") != trace.get("session_id")
        or not valid_response_sha256(item.get("response_sha256"))
    ):
        return None
    return (
        turn_id,
        str(normalized_uuid(trace["trace_id"])),
        str(trace["session_id"]),
        str(trace["chain_id"]) if trace.get("chain_id") is not None else None,
        str(item["response_sha256"]),
        parent_span_id,
    )


def exact_response_identity(
    item: dict[str, Any], *, flat_identity: bool = False
) -> tuple[str, str, str, str] | None:
    trace = item.get("trace")
    if isinstance(trace, dict):
        if trace.get("schema_version", 1) != 1:
            return None
        exact_trace_id = normalized_uuid(trace.get("trace_id"))
        exact_turn_id = normalized_uuid(trace.get("turn_id"))
    elif flat_identity:
        exact_trace_id = normalized_uuid(item.get("trace_id"))
        exact_turn_id = normalized_uuid(item.get("turn_id"))
    else:
        return None
    response_hash = item.get("response_sha256")
    if exact_trace_id is None or not valid_response_sha256(response_hash):
        return None
    if exact_turn_id is not None:
        return "turn", exact_turn_id, exact_trace_id, str(response_hash)
    return "trace", "", exact_trace_id, str(response_hash)


def interrupted_correction_identity(
    item: dict[str, Any],
) -> tuple[str, str, str, str] | None:
    if item.get("schema") != "astrid_edge_interrupted_action_correction_v2":
        return None
    return exact_response_identity(item, flat_identity=True)


def summarize_action_dispatches(
    action_dispatches: list[dict[str, Any]],
    action_receipts: list[dict[str, Any]],
) -> dict[str, int]:
    valid_dispatches = [
        (item, key)
        for item in action_dispatches
        if (key := dispatch_key(item)) is not None
    ]
    requested_keys = [
        key for item, key in valid_dispatches if item.get("phase") == "requested"
    ]
    completed_keys = [
        key for item, key in valid_dispatches if item.get("phase") == "completed"
    ]
    receipt_keys = [
        key
        for item in action_receipts
        if (key := action_receipt_dispatch_key(item)) is not None
    ]
    requested_set = set(requested_keys)
    completed_set = set(completed_keys)
    receipt_set = set(receipt_keys)
    paired_dispatches = requested_set & completed_set
    return {
        "records_total": len(action_dispatches),
        "requested_total": len(requested_keys),
        "completed_total": len(completed_keys),
        "fully_correlated_total": len(paired_dispatches & receipt_set),
        "pending_total": len(requested_set - completed_set),
        "orphan_completion_total": len(completed_set - requested_set),
        "completed_without_action_receipt_total": len(
            paired_dispatches - receipt_set
        ),
        "action_receipt_without_intent_total": len(receipt_set - requested_set),
        "action_receipt_without_completion_total": len(
            (receipt_set & requested_set) - completed_set
        ),
        "duplicate_phase_total": len(requested_keys)
        - len(requested_set)
        + len(completed_keys)
        - len(completed_set),
        "duplicate_action_receipt_total": len(receipt_keys) - len(receipt_set),
        "unattributed_action_receipt_total": sum(
            action_receipt_dispatch_key(item) is None for item in action_receipts
        ),
        "malformed_total": sum(
            dispatch_key(item) is None for item in action_dispatches
        ),
    }


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


def tuning_state_view(value: dict[str, Any]) -> dict[str, Any]:
    payload = value.get("payload")
    return payload if isinstance(payload, dict) else value


def tuning_state_focus(value: dict[str, Any]) -> tuple[str, dict[str, Any]]:
    for field, status in (
        ("active_experiment", "active_trial"),
        ("active_validation", "active_validation"),
        ("standing_adoption", "standing_adoption"),
        ("suspended_adoption", "suspended_adoption"),
    ):
        candidate = value.get(field)
        if isinstance(candidate, dict):
            return status, candidate
    return "inactive", {}


def command(
    *arguments: str, environment: dict[str, str] | None = None
) -> str:
    try:
        if not arguments or arguments[0] not in TRUSTED_COMMANDS:
            return ""
        exact_arguments = (TRUSTED_COMMANDS[arguments[0]], *arguments[1:])
        command_environment = os.environ.copy()
        command_environment["PATH"] = "/usr/bin:/bin"
        for unsafe_name in ("PYTHONHOME", "PYTHONPATH", "PYTHONSTARTUP"):
            command_environment.pop(unsafe_name, None)
        if environment is not None:
            command_environment.update(environment)
        return subprocess.run(
            exact_arguments,
            check=False,
            capture_output=True,
            text=True,
            timeout=5,
            env=command_environment,
        ).stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        return ""


def system_service_manager_enabled(
    marker: Path = SERVICE_MANAGER_MARKER,
) -> bool:
    try:
        metadata = marker.stat(follow_symlinks=False)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != 0
            or metadata.st_nlink != 1
            or stat.S_IMODE(metadata.st_mode) & 0o022
        ):
            return False
        value = json.loads(marker.read_text(encoding="ascii"))
        runtime_name = pwd.getpwuid(os.getuid()).pw_name
    except (KeyError, OSError, UnicodeError, json.JSONDecodeError):
        return False
    return (
        isinstance(value, dict)
        and value.get("schema")
        in {"astrid.edge.service_manager.v1", "astrid.edge.service_manager.v2"}
        and value.get("manager") == "system"
        and value.get("runtime_user") == runtime_name
    )


def loaded_capsules_from_status(raw: str) -> list[str] | None:
    try:
        value = json.loads(raw)
        loaded = value["status"]["loaded_capsules"]
        if not isinstance(loaded, list) or not all(
            isinstance(item, str) for item in loaded
        ):
            raise ValueError("loaded_capsules is not a string array")
        if len(set(loaded)) != len(loaded):
            raise ValueError("loaded_capsules contains duplicate names")
    except (json.JSONDecodeError, KeyError, TypeError, ValueError):
        return None
    return loaded


def loaded_capsule_contract(loaded: list[str]) -> bool:
    return len(loaded) == 20 and ESSENTIAL_EDGE_CAPSULES.issubset(loaded)


def completed_session_retirements(
    rows: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    return [
        item
        for item in rows
        if item.get("schema") == "astrid_edge_operator_session_retirement_v1"
        and item.get("phase") == "completed"
    ]


def service_value(service: str, field: str) -> str:
    arguments = ["systemctl"]
    if not SYSTEM_SERVICE_MANAGER:
        arguments.append("--user")
    arguments.extend(("show", service, f"--property={field}", "--value"))
    return command(*arguments) or "unknown"


def profile_values(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    try:
        lines = path.read_text().splitlines()
    except OSError:
        return values
    for line in lines:
        if not line or line.lstrip().startswith("#") or "=" not in line:
            continue
        name, value = line.split("=", 1)
        values[name] = value.strip().strip('"')
    return values


def parse_process_profile_values(raw: bytes) -> dict[str, str]:
    """Parse only non-secret appliance profile keys from a process environment."""
    values: dict[str, str] = {}
    for entry in raw.split(b"\0"):
        if b"=" not in entry:
            continue
        raw_name, raw_value = entry.split(b"=", 1)
        name = raw_name.decode("utf-8", errors="replace")
        if not name.startswith(("ASTRID_EDGE_", "ASTRID_OLLAMA_")):
            continue
        values[name] = raw_value.decode("utf-8", errors="replace")
    return values


def process_profile_values(service: str) -> dict[str, str]:
    """Read allowlisted live edge settings from the running service process."""
    raw_pid = service_value(service, "MainPID")
    if not raw_pid.isdecimal() or int(raw_pid) <= 0:
        return {}
    try:
        raw = Path(f"/proc/{raw_pid}/environ").read_bytes()
    except OSError:
        return {}
    return parse_process_profile_values(raw)


def effective_profile_values(
    home: Path, live_values: dict[str, str] | None = None
) -> dict[str, str]:
    values = profile_values(home / ".config/astrid/edge-appliance.env")
    # The rollout authority file is loaded after the appliance profile by the
    # systemd drop-in. Mirror that precedence so reports show effective tuning
    # authority rather than the eventual profile capability.
    values.update(profile_values(home / ".config/astrid/edge-tuning-authority.env"))
    # The running process is the final authority during staged rollouts.  This
    # includes systemd Environment= and EnvironmentFile= overrides that are
    # deliberately absent from the durable appliance profile.
    if live_values is not None:
        values.update(live_values)
    return values


def summarize_fill(prefix: str, values: list[float]) -> None:
    emit(f"{prefix}_samples", len(values))
    if not values:
        return
    emit(f"{prefix}_min_pct", f"{min(values):.2f}")
    emit(f"{prefix}_mean_pct", f"{sum(values) / len(values):.2f}")
    emit(f"{prefix}_max_pct", f"{max(values):.2f}")
    emit(
        f"{prefix}_inside_65_72_pct",
        f"{100 * sum(65 <= value <= 72 for value in values) / len(values):.1f}",
    )
    emit(
        f"{prefix}_inside_65_73_5_pct",
        f"{100 * sum(65 <= value <= 73.5 for value in values) / len(values):.1f}",
    )


def percentile(values: list[int], numerator: int = 95) -> int:
    if not values:
        return 0
    ordered = sorted(values)
    rank = max(1, (len(ordered) * numerator + 99) // 100)
    return ordered[min(len(ordered), rank) - 1]


def local_provider_header_latencies(astrid_root: Path, cutoff_ms: int) -> list[int]:
    values: list[int] = []
    log_directory = astrid_root / "log"
    try:
        paths = sorted(log_directory.glob("astrid.*.log"))[-2:]
    except OSError:
        return values
    for path in paths:
        try:
            with path.open("rb") as handle:
                size = handle.seek(0, os.SEEK_END)
                start = max(0, size - 8 * 1024 * 1024)
                handle.seek(start)
                if start:
                    handle.readline()
                lines = handle.read().decode("utf-8", errors="replace").splitlines()
        except OSError:
            continue
        for line in lines:
            match = LOCAL_HEADER_PATTERN.match(line)
            if match is None:
                continue
            try:
                timestamp_ms = int(
                    dt.datetime.fromisoformat(
                        match.group("timestamp").replace("Z", "+00:00")
                    ).timestamp()
                    * 1_000
                )
            except ValueError:
                continue
            if timestamp_ms >= cutoff_ms:
                values.append(int(match.group("elapsed")))
    return values


def main() -> int:
    global SYSTEM_SERVICE_MANAGER  # noqa: PLW0603 -- one process-local report mode.
    if "--candidate-presentation" in sys.argv[1:]:
        return candidate_presentation()
    parser = argparse.ArgumentParser()
    parser.add_argument("--window-minutes", type=int, default=20)
    parser.add_argument("--workspace", type=Path)
    parser.add_argument(
        "--scheduled-authorship-verify-key",
        type=Path,
        default=SCHEDULED_AUTHORSHIP_VERIFY_KEY,
    )
    args = parser.parse_args()
    if args.window_minutes < 1:
        parser.error("--window-minutes must be positive")
    SYSTEM_SERVICE_MANAGER = system_service_manager_enabled()

    home = Path.home()
    workspace = args.workspace or home / ".astrid/home/default/edge"
    astrid_root = workspace.parents[2]
    state_path = workspace / "runtime/spectral_state.json"
    history_path = workspace / "runtime/fill_history.jsonl"
    if not state_path.is_file() or not history_path.is_file():
        parser.error(f"edge telemetry is unavailable under {workspace / 'runtime'}")

    live_profile = process_profile_values("astrid-edge-runtime.service")
    profile = effective_profile_values(home, live_profile)
    now_ms = time.time_ns() // 1_000_000
    cutoff_ms = now_ms - args.window_minutes * 60_000
    state = read_json(state_path)

    emit("report_version", 16)
    emit("instance_name", profile.get("ASTRID_EDGE_INSTANCE_NAME", "edge Astrid"))
    emit("hostname", os.uname().nodename)
    for label, service in (
        ("astrid", "astrid.service"),
        ("edge", "astrid-edge-runtime.service"),
    ):
        emit(f"{label}_service_state", service_value(service, "ActiveState"))
        emit(f"{label}_service_restarts", service_value(service, "NRestarts"))
    status_raw = command(
        str(astrid_root / "bin/astrid"),
        "--format",
        "json",
        "status",
        environment={"ASTRID_HOME": str(astrid_root)},
    )
    loaded_capsules = loaded_capsules_from_status(status_raw)
    emit(
        "astrid_loaded_capsule_count",
        len(loaded_capsules) if loaded_capsules is not None else "unavailable",
    )
    emit(
        "astrid_loaded_capsule_contract_20",
        loaded_capsule_contract(loaded_capsules)
        if loaded_capsules is not None
        else "unavailable",
    )
    emit(
        "astrid_missing_essential_capsules",
        ",".join(sorted(ESSENTIAL_EDGE_CAPSULES.difference(loaded_capsules)))
        if loaded_capsules is not None
        else "unavailable",
    )
    emit(
        "astrid_loaded_capsules",
        ",".join(sorted(loaded_capsules)) if loaded_capsules is not None else "unavailable",
    )
    react_provenance = react_provenance_view(astrid_root)
    emit("react_provenance_source", react_provenance["source"])
    emit("react_provenance_validation", react_provenance["validation"])
    emit(
        "react_provenance_generation_state",
        react_provenance["generation_state"],
    )
    emit("react_provenance_generation_id", react_provenance["generation_id"])
    emit(
        "react_provenance_live_content_address",
        react_provenance["live_content_address"],
    )
    emit(
        "react_provenance_archive_sha256",
        react_provenance["archive_sha256"],
    )
    emit("react_provenance_manifest_sha256", react_provenance["manifest_sha256"])
    emit(
        "model_warmup_service_state",
        service_value("astrid-model-warmup.service", "ActiveState"),
    )
    emit(
        "hindsight_timer_state",
        service_value("astrid-edge-hindsight.timer", "ActiveState"),
    )
    emit(
        "hindsight_last_service_result",
        service_value("astrid-edge-hindsight.service", "Result"),
    )
    emit(
        "user_linger",
        command("loginctl", "show-user", os.environ.get("USER", ""), "-p", "Linger", "--value")
        or "unknown",
    )
    emit("selected_model", profile.get("ASTRID_OLLAMA_MODEL", ""))
    emit(
        "selected_model_status",
        profile.get("ASTRID_OLLAMA_SELECTION_STATUS", ""),
    )
    emit("selected_model_context", profile.get("ASTRID_OLLAMA_CONTEXT", ""))
    emit("selected_model_max_output", profile.get("ASTRID_OLLAMA_MAX_OUTPUT", ""))
    emit("autonomy_enabled", profile.get("ASTRID_EDGE_AUTONOMY_ENABLED", ""))
    emit(
        "autonomy_interval_minutes",
        profile.get("ASTRID_EDGE_AUTONOMY_INTERVAL_MINUTES", ""),
    )
    emit(
        "autonomy_event_driven",
        profile.get("ASTRID_EDGE_AUTONOMY_EVENT_DRIVEN", ""),
    )
    emit(
        "autonomy_event_heartbeat_minutes",
        profile.get("ASTRID_EDGE_AUTONOMY_EVENT_HEARTBEAT_MINUTES", ""),
    )
    emit(
        "autonomy_follow_up_minutes",
        profile.get("ASTRID_EDGE_AUTONOMY_FOLLOW_UP_MINUTES", ""),
    )
    emit(
        "autonomy_max_chain_steps",
        profile.get("ASTRID_EDGE_AUTONOMY_MAX_CHAIN_STEPS", ""),
    )
    emit(
        "autonomy_quiet_minutes",
        profile.get("ASTRID_EDGE_AUTONOMY_QUIET_MINUTES", ""),
    )
    emit(
        "autonomy_max_turns_per_day",
        profile.get("ASTRID_EDGE_AUTONOMY_MAX_TURNS_PER_DAY", ""),
    )
    emit(
        "autonomy_initiative_profile",
        profile.get("ASTRID_EDGE_AUTONOMY_INITIATIVE_PROFILE", ""),
    )
    emit(
        "autonomy_prompt_profile",
        profile.get("ASTRID_EDGE_AUTONOMY_PROMPT_PROFILE", ""),
    )
    emit(
        "autonomy_prompt_max_chars",
        profile.get("ASTRID_EDGE_AUTONOMY_PROMPT_MAX_CHARS", ""),
    )
    emit(
        "autonomy_session_max_authored_turns",
        profile.get("ASTRID_EDGE_AUTONOMY_SESSION_MAX_AUTHORED_TURNS", ""),
    )
    emit(
        "autonomy_chain_session_max_authored_turns",
        profile.get("ASTRID_EDGE_AUTONOMY_CHAIN_SESSION_MAX_AUTHORED_TURNS", ""),
    )
    emit(
        "local_provider_response_header_timeout_seconds",
        profile.get("ASTRID_LOCAL_HTTP_RESPONSE_HEADER_TIMEOUT_SECONDS", "300"),
    )
    emit(
        "autonomy_journal_authored_turns",
        profile.get("ASTRID_EDGE_AUTONOMY_JOURNAL_AUTHORED_TURNS", ""),
    )
    emit(
        "research_action_web_search",
        profile.get("ASTRID_EDGE_RESEARCH_ACTION_WEB_SEARCH", ""),
    )
    emit(
        "scheduled_introspection_enabled",
        profile.get("ASTRID_EDGE_SCHEDULED_INTROSPECTION_ENABLED", "false"),
    )
    emit(
        "scheduled_introspection_interval_minutes",
        profile.get("ASTRID_EDGE_SCHEDULED_INTROSPECTION_INTERVAL_MINUTES", ""),
    )
    emit(
        "scheduled_introspection_initial_delay_seconds",
        profile.get(
            "ASTRID_EDGE_SCHEDULED_INTROSPECTION_INITIAL_DELAY_SECONDS", ""
        ),
    )
    emit(
        "scheduled_introspection_timeout_seconds",
        profile.get("ASTRID_EDGE_SCHEDULED_INTROSPECTION_TIMEOUT_SECONDS", ""),
    )
    emit(
        "scheduled_introspection_prompt_max_chars",
        profile.get("ASTRID_EDGE_SCHEDULED_INTROSPECTION_PROMPT_MAX_CHARS", ""),
    )
    emit(
        "self_change_enabled",
        profile.get("ASTRID_EDGE_SELF_CHANGE_ENABLED", "false"),
    )
    emit(
        "perceptual_notebook_enabled",
        profile.get("ASTRID_EDGE_PERCEPTUAL_NOTEBOOK_ENABLED", ""),
    )
    for field, default in (
        ("ASTRID_EDGE_SPECTRAL_ENABLED", "false"),
        ("ASTRID_EDGE_SPECTRAL_ROLLUP_SECONDS", "60"),
        ("ASTRID_EDGE_RESERVOIR_TUNING_ENABLED", "false"),
        ("ASTRID_EDGE_RESERVOIR_TUNING_MAX_PER_DAY", "4"),
    ):
        emit(field.lower(), profile.get(field, default))

    state_names = {
        "recorded_at_unix_ms": "current_recorded_at_unix_ms",
        "fill_pct": "current_fill_pct",
    }
    for key in (
        "recorded_at_unix_ms",
        "fill_pct",
        "target_fill_pct",
        "effective_dimensionality",
        "audio_fresh",
        "audio_source",
        "aux_fresh",
        "aux_source",
        "video_fresh",
        "video_source",
        "semantic_fresh",
    ):
        emit(state_names.get(key, key), state.get(key, "unknown"))
    emit("telemetry_age_ms", max(0, now_ms - int(state.get("recorded_at_unix_ms", 0))))
    for name, value in sorted((state.get("aux_features") or {}).items()):
        emit(f"aux_feature_{name}_available", value is not None)
        emit(f"aux_feature_{name}_value", "unavailable" if value is None else value)

    substrate = state.get("substrate") or state.get("spectral_substrate_v1") or {}
    substrate = substrate if isinstance(substrate, dict) else {}
    emit("spectral_state_schema", state.get("schema", "legacy_unavailable"))
    emit("spectral_substrate_kind", substrate.get("kind", "legacy_unknown"))
    emit("spectral_fill_metric", substrate.get("fill_metric", "legacy_unknown"))
    emit("spectral_reservoir_dim", substrate.get("reservoir_dim", "unknown"))
    emit(
        "spectral_exported_eigenvalue_count",
        substrate.get(
            "exported_eigenvalue_count",
            state.get("exported_eigenvalue_count", "unknown"),
        ),
    )
    for key in (
        "spectral_entropy",
        "lambda1_share",
        "head_share",
        "shoulder_share",
        "tail_share",
        "density_gradient",
        "largest_gap",
        "largest_gap_index",
        "mode_turnover",
        "mode_identity_state",
        "exploration_scale",
        "regulation_strength",
    ):
        emit(f"spectral_current_{key}", state.get(key, "unavailable"))

    spectral_rollups = [
        item
        for item in read_json_lines(workspace / "spectral/rollups.jsonl")
        if int(item.get("recorded_at_unix_ms", 0) or 0) >= cutoff_ms
    ]
    emit("spectral_window_rollups", len(spectral_rollups))
    latest_spectral_at = max(
        (int(item.get("recorded_at_unix_ms", 0) or 0) for item in spectral_rollups),
        default=0,
    )
    emit(
        "spectral_window_latest_age_ms",
        max(0, now_ms - latest_spectral_at) if latest_spectral_at else "unavailable",
    )
    for key in (
        "spectral_entropy",
        "lambda1_share",
        "tail_share",
        "density_gradient",
        "mode_turnover",
    ):
        values = [
            float(spectral_metric(item, key))
            for item in spectral_rollups
            if isinstance(spectral_metric(item, key), (int, float))
        ]
        emit(f"spectral_window_{key}_samples", len(values))
        if values:
            emit(f"spectral_window_{key}_min", f"{min(values):.6f}")
            emit(f"spectral_window_{key}_mean", f"{sum(values) / len(values):.6f}")
            emit(f"spectral_window_{key}_max", f"{max(values):.6f}")
    emit(
        "spectral_window_mode_identity_unstable",
        sum(
            item.get("mode_identity_state") == "unstable_near_degenerate"
            for item in spectral_rollups
        ),
    )
    emit(
        "spectral_window_complete_rollups",
        sum(
            isinstance(item.get("coverage"), dict)
            and item["coverage"].get("complete") is True
            for item in spectral_rollups
        ),
    )
    emit(
        "spectral_window_temporal_activity_contexts",
        sum(int(item.get("activity_ref_count", 0) or 0) for item in spectral_rollups),
    )
    emit(
        "spectral_window_activity_ref_truncated_rollups",
        sum(item.get("activity_refs_truncated") is True for item in spectral_rollups),
    )
    derivation_latencies = [
        float(item["spectral_derivation_p95_ms"])
        for item in spectral_rollups
        if isinstance(item.get("spectral_derivation_p95_ms"), (int, float))
    ]
    emit("spectral_derivation_rollup_p95_samples", len(derivation_latencies))
    emit(
        "spectral_derivation_p95_ms_max",
        f"{max(derivation_latencies):.3f}" if derivation_latencies else "unavailable",
    )
    spectral_paths = (
        workspace / "spectral/rollups.jsonl",
        workspace / "spectral/recent_rollups.current.jsonl",
        workspace / "spectral/recent_rollups.previous.jsonl",
        workspace / "spectral/activity_receipts.current.jsonl",
        workspace / "spectral/activity_receipts.previous.jsonl",
        workspace / "spectral/receipts.jsonl",
    )
    emit(
        "spectral_storage_bytes",
        sum(path.stat().st_size for path in spectral_paths if path.is_file()),
    )

    tuning_state_envelope = read_json(workspace / "tuning/state.json")
    tuning_state = tuning_state_view(tuning_state_envelope)
    tuning_status, tuning_focus = tuning_state_focus(tuning_state)
    tuning_receipts = [
        flatten_signed_tuning(item)
        for item in read_json_lines(workspace / "tuning/receipts.jsonl")
        if int(
            (
                item.get("payload", {}).get("recorded_at_unix_ms", 0)
                if isinstance(item.get("payload"), dict)
                else item.get("recorded_at_unix_ms", 0)
            )
            or 0
        )
        >= cutoff_ms
    ]
    emit("tuning_state_schema", tuning_state_envelope.get("schema", "none"))
    emit("tuning_state_status", tuning_status)
    emit(
        "tuning_active_id",
        tuning_focus.get("tuning_id")
        or tuning_focus.get("experiment_id")
        or tuning_focus.get("validation_id")
        or tuning_focus.get("adoption_id")
        or "none",
    )
    emit("tuning_candidate_id", tuning_focus.get("candidate_id", "none"))
    tuning_spec = tuning_focus.get("spec")
    tuning_spec = tuning_spec if isinstance(tuning_spec, dict) else {}
    emit(
        "tuning_parameter",
        tuning_focus.get("parameter") or tuning_spec.get("parameter") or "none",
    )
    emit("tuning_phase", tuning_focus.get("phase", tuning_status))
    tuning_environment = tuning_focus.get("environment")
    tuning_environment = (
        tuning_environment if isinstance(tuning_environment, dict) else {}
    )
    emit(
        "tuning_policy_hash",
        tuning_environment.get("policy_sha256") or "none",
    )
    emit(
        "tuning_signed_state_present",
        bool(
            tuning_state_envelope.get("payload_sha256")
            and tuning_state_envelope.get("signature")
            and tuning_state_envelope.get("signing_public_key")
        ),
    )
    emit(
        "tuning_signed_state_payload_hash_valid",
        flatten_signed_tuning(tuning_state_envelope).get(
            "payload_hash_valid", "not_applicable"
        )
        if tuning_state_envelope
        else "not_applicable",
    )
    emit("tuning_window_events", len(tuning_receipts))
    emit(
        "tuning_window_payload_hash_failures",
        sum(item.get("payload_hash_valid") is False for item in tuning_receipts),
    )
    emit(
        "tuning_window_signature_envelopes_present",
        sum(
            item.get("signature_present_not_verified") is True
            for item in tuning_receipts
        ),
    )
    emit(
        "tuning_window_rollbacks",
        sum(
            str(item.get("phase", "")).startswith("rolled_back")
            or item.get("rollback_reason") is not None
            for item in tuning_receipts
        ),
    )
    emit(
        "tuning_window_trace_coverage_pct",
        f"{100 * sum(isinstance(item.get('trace'), dict) for item in tuning_receipts) / len(tuning_receipts):.1f}"
        if tuning_receipts
        else "not_applicable",
    )

    latest_observation = read_json(workspace / "perception/latest.json")
    observation_rows = read_json_lines(workspace / "perception/observations.jsonl")
    observation_time = int(latest_observation.get("recorded_at_unix_ms", 0) or 0)
    emit("perception_latest_recorded_at_unix_ms", observation_time)
    emit(
        "perception_latest_age_ms",
        max(0, now_ms - observation_time) if observation_time else "unavailable",
    )
    emit(
        "perception_latest_authority",
        latest_observation.get("authority", "unavailable"),
    )
    emit(
        "perception_latest_trigger_classes",
        ",".join(latest_observation.get("trigger_classes") or []),
    )
    emit(
        "perception_latest_causal_class",
        latest_observation.get("causal_class", "legacy_unclassified"),
    )
    emit(
        "perception_latest_machine_summary",
        latest_observation.get("summary", "unavailable"),
    )
    emit(
        "perception_window_observations",
        sum(
            int(row.get("recorded_at_unix_ms", 0) or 0) >= cutoff_ms
            for row in observation_rows
        ),
    )

    warmup = read_json(workspace / "runtime/model_warmup.json")
    for key in ("status", "model", "elapsed_ms", "completed_at_unix_ms"):
        if warmup:
            emit(f"model_warmup_{key}", warmup.get(key, "unknown"))

    history = [
        item
        for item in read_json_lines(history_path)
        if int(item.get("recorded_at_unix_ms", 0)) >= cutoff_ms
    ]
    emit("fill_window_minutes", args.window_minutes)
    summarize_fill("fill_all", [float(item.get("fill_pct", 0.0)) for item in history])
    emit("fill_settled_after_seconds", 30)
    summarize_fill(
        "fill_settled",
        [
            float(item.get("fill_pct", 0.0))
            for item in history
            if int(item.get("t_ms", 0)) >= 30_000
        ],
    )
    local_header_latencies = local_provider_header_latencies(astrid_root, cutoff_ms)
    emit(
        "local_provider_header_latency_attribution",
        "exact_origin_window_unattributed_to_individual_turn",
    )
    emit("local_provider_header_latency_ms_samples", len(local_header_latencies))
    emit(
        "local_provider_header_latency_ms_p95",
        percentile(local_header_latencies),
    )
    emit(
        "local_provider_header_latency_ms_max",
        max(local_header_latencies, default=0),
    )

    recent_runs = read_json_lines(workspace / "autonomous/runs.jsonl")
    autonomy = read_json(workspace / "autonomous/state.json")
    emit("autonomy_state_schema", autonomy.get("schema", "none"))
    state_keys = (
        "last_status",
        "attempts_today",
        "authored_turns_today",
        "transport_recoveries_today",
        "consecutive_failures",
        "consecutive_action_validation_failures",
        "total_attempts",
        "total_authored_turns",
        "total_transport_recoveries",
        "ordinary_session_generation",
        "ordinary_session_authored_turns",
        "chain_session_generation",
        "chain_session_authored_turns",
        "last_session_name",
        "last_prompt_chars",
        "last_prompt_estimated_tokens",
        "last_turn_elapsed_ms",
        "last_trace_id",
        "active_chain_id",
        "active_chain_step",
        "run_receipt_pending",
        "chain_receipt_pending",
        "action_dispatch_pending",
        "operator_pause_reason",
        "operator_pause_since_unix_ms",
        "next_due_at_unix_ms",
        "last_perception_consumed_at_unix_ms",
    )
    for key in state_keys:
        optional_text = key in ("active_chain_id", "operator_pause_reason")
        value = autonomy.get(key, "none" if optional_text else 0)
        if optional_text and value is None:
            value = "none"
        emit(f"autonomy_{key}", value)
    last_response_provenance, last_safe_fallback, last_format_repair = (
        latest_response_provenance(autonomy, recent_runs)
    )
    emit("autonomy_last_response_provenance", last_response_provenance)
    emit(
        "autonomy_last_local_safe_fallback_used",
        last_safe_fallback,
    )
    emit(
        "autonomy_last_local_format_repair_used",
        last_format_repair,
    )
    last_trace = autonomy.get("last_trace")
    emit(
        "autonomy_last_turn_id",
        last_trace.get("turn_id", "none") if isinstance(last_trace, dict) else "none",
    )
    session_retirements = completed_session_retirements(
        read_json_lines(workspace / "autonomous/session_retirements.jsonl")
    )
    emit("autonomy_operator_session_retirements_total", len(session_retirements))
    emit(
        "autonomy_operator_session_retirements_window",
        sum(
            int(item.get("recorded_at_unix_ms", 0) or 0) >= cutoff_ms
            for item in session_retirements
        ),
    )
    if session_retirements:
        latest_retirement = session_retirements[-1]
        emit(
            "autonomy_latest_retired_session_generation",
            latest_retirement.get("prior_session_generation", "none"),
        )
        emit(
            "autonomy_latest_replacement_session_generation",
            latest_retirement.get("new_session_generation", "none"),
        )
        emit(
            "autonomy_latest_session_retirement_reason",
            latest_retirement.get("reason", "none"),
        )
        emit(
            "autonomy_latest_session_retirement_authority",
            latest_retirement.get("authority", "none"),
        )

    thread = read_json(workspace / "autonomous/thread_state.json")
    emit("thread_state_schema", thread.get("schema", "none"))
    emit("thread_state_revision", thread.get("revision", 0))
    emit("thread_state_id", thread.get("thread_id", "none"))
    emit("thread_state_status", thread.get("status", "none"))
    emit("thread_state_chain_id", thread.get("chain_id", "none"))
    emit("thread_state_focus", thread.get("focus", "none"))
    emit("thread_state_question", thread.get("question", "none"))
    emit("thread_state_hypothesis", thread.get("hypothesis", "none"))
    emit("thread_state_hypotheses_count", len(thread.get("hypotheses", [])))
    emit("thread_state_methods_count", len(thread.get("methods", [])))
    emit("thread_state_study_ids_count", len(thread.get("study_ids", [])))
    emit("thread_state_counterquestions_count", len(thread.get("counterquestions", [])))
    emit("thread_state_syntheses_count", len(thread.get("syntheses", [])))
    emit(
        "thread_state_unresolved_uncertainties_count",
        len(thread.get("unresolved_uncertainties", [])),
    )
    emit("thread_state_provenance_hashes_count", len(thread.get("provenance_hashes", [])))
    emit("thread_state_last_action", thread.get("last_action", "none"))
    emit("thread_state_authored_claims_count", len(thread.get("authored_claims", [])))
    emit("thread_state_findings_count", len(thread.get("findings", [])))
    emit("thread_state_open_questions_count", len(thread.get("open_questions", [])))
    emit("thread_state_conclusion", thread.get("conclusion", "none"))
    emit("thread_state_evidence_count", len(thread.get("evidence", [])))
    emit(
        "thread_state_evidence_records_count",
        len(thread.get("evidence_records", [])),
    )
    evidence_statuses = sorted(
        {
            str(item.get("epistemic_status") or "legacy_unclassified")
            for item in thread.get("evidence_records", [])
            if isinstance(item, dict)
        }
    )
    for status in evidence_statuses:
        emit(
            f"thread_state_evidence_status_{re.sub(r'[^a-zA-Z0-9]+', '_', status).strip('_')}_count",
            sum(
                str(item.get("epistemic_status") or "legacy_unclassified") == status
                for item in thread.get("evidence_records", [])
                if isinstance(item, dict)
            ),
        )
    evidence_kinds = sorted(
        {
            str(item.get("kind") or "legacy_unclassified")
            for item in thread.get("evidence_records", [])
            if isinstance(item, dict)
        }
    )
    for kind in evidence_kinds:
        emit(
            f"thread_state_evidence_kind_{re.sub(r'[^a-zA-Z0-9]+', '_', kind).strip('_')}_count",
            sum(
                str(item.get("kind") or "legacy_unclassified") == kind
                for item in thread.get("evidence_records", [])
                if isinstance(item, dict)
            ),
        )
    emit("thread_state_updated_at_unix_ms", thread.get("updated_at_unix_ms", 0))
    started = int(autonomy.get("last_started_at_unix_ms") or 0)
    age = max(0, now_ms - started) if autonomy.get("last_status") == "running" else 0
    emit("autonomy_current_turn_age_ms", age)

    statuses = {
        "authored_completed": 0,
        "transport_recovery": 0,
        "failed": 0,
        "interrupted": 0,
    }
    transport_corrections = {
        str(item.get("original_transcript_path"))
        for item in read_json_lines(
            workspace / "autonomous/authorship_corrections.jsonl"
        )
        if item.get("reason")
        == "legacy_transport_sentinel_reclassified_non_authored"
    }
    for item in recent_runs:
        if int(item.get("completed_at_unix_ms", 0)) >= cutoff_ms:
            status = str(item.get("status", ""))
            if (
                status == "authored_completed"
                and str(item.get("transcript_path")) in transport_corrections
            ):
                status = "transport_recovery"
            if status in statuses:
                statuses[status] += 1
    status_names = {
        "authored_completed": "authored",
        "transport_recovery": "transport_recoveries",
        "failed": "failed",
        "interrupted": "interrupted",
    }
    for status, count in statuses.items():
        emit(f"autonomy_window_{status_names[status]}_turns", count)
    window_runs = [
        item
        for item in recent_runs
        if int(item.get("completed_at_unix_ms", 0) or 0) >= cutoff_ms
    ]
    run_provenance = [run_response_provenance(item) for item in window_runs]
    provenance_counts = response_provenance_counts(window_runs)
    for provenance, count in provenance_counts.items():
        emit(
            f"autonomy_window_response_provenance_{provenance}_turns",
            count,
        )
    emit(
        "autonomy_window_local_safe_fallback_used_turns",
        sum(value[1] for value in run_provenance),
    )
    emit(
        "autonomy_window_local_format_repair_used_turns",
        sum(value[2] for value in run_provenance),
    )
    prompt_chars = [
        int(item.get("prompt_chars", 0) or 0)
        for item in window_runs
        if int(item.get("prompt_chars", 0) or 0) > 0
    ]
    full_turn_latencies = [
        int(item.get("full_turn_latency_ms", item.get("elapsed_ms", 0)) or 0)
        for item in window_runs
        if int(item.get("full_turn_latency_ms", item.get("elapsed_ms", 0)) or 0) > 0
    ]
    emit("autonomy_window_prompt_chars_max", max(prompt_chars, default=0))
    emit("autonomy_window_prompt_chars_p95", percentile(prompt_chars))
    emit("autonomy_window_full_turn_latency_p95_ms", percentile(full_turn_latencies))
    for field in (
        "provider_prompt_tokens",
        "provider_completion_tokens",
        "generation_latency_ms",
    ):
        values = [
            int(item[field])
            for item in window_runs
            if isinstance(item.get(field), (int, float))
        ]
        emit(f"autonomy_window_{field}_samples", len(values))
        emit(f"autonomy_window_{field}_p95", percentile(values))
    exact_header_latencies = [
        exact[0]
        for item in window_runs
        if (exact := exact_request_header_latency(item)) is not None
    ]
    legacy_header_latencies = [
        legacy
        for item in window_runs
        if (legacy := legacy_unattributed_header_latency(item)) is not None
    ]
    exact_provider_telemetry = [
        exact
        for item in window_runs
        if (exact := exact_provider_request_telemetry(item)) is not None
    ]
    invalid_claimed_provider_metrics = [
        item
        for item in window_runs
        if item.get("request_header_latency_source")
        == REQUEST_HEADER_LATENCY_SOURCE_V1
        and exact_provider_request_telemetry(item) is None
    ]
    emit(
        "autonomy_window_request_header_latency_ms_samples",
        len(exact_header_latencies),
    )
    emit(
        "autonomy_window_request_header_latency_ms_exact_samples",
        len(exact_header_latencies),
    )
    emit(
        "autonomy_window_request_header_latency_ms_legacy_unattributed_samples",
        len(legacy_header_latencies),
    )
    emit(
        "autonomy_window_request_header_latency_ms_p95",
        percentile(exact_header_latencies),
    )
    emit(
        "autonomy_window_provider_request_metrics_exact_samples",
        len(exact_provider_telemetry),
    )
    emit(
        "autonomy_window_provider_request_attempts_total",
        sum(item[0] for item in exact_provider_telemetry),
    )
    emit(
        "autonomy_window_provider_successful_headers_total",
        sum(item[1] for item in exact_provider_telemetry),
    )
    emit(
        "autonomy_window_provider_metrics_invalid_claimed_exact",
        len(invalid_claimed_provider_metrics),
    )
    emit(
        "autonomy_window_signal_journals",
        sum(
            item.get("status") == "authored_completed" and bool(item.get("journal_path"))
            and str(item.get("transcript_path")) not in transport_corrections
            for item in recent_runs
            if int(item.get("completed_at_unix_ms", 0)) >= cutoff_ms
        ),
    )

    action_receipts = read_json_lines(workspace / "actions/receipts.jsonl")
    action_dispatches = read_json_lines(workspace / "actions/dispatches.jsonl")
    dispatch_summary = summarize_action_dispatches(action_dispatches, action_receipts)
    for field, value in dispatch_summary.items():
        emit(f"action_dispatch_{field}", value)
    interrupted_correction_records = read_json_lines(
        workspace / "actions/interrupted_corrections.jsonl"
    )
    interrupted_action_corrections = {
        key: item
        for item in interrupted_correction_records
        if item.get("corrected_status") == "revoked_interrupted_trace_non_authored"
        if (key := interrupted_correction_identity(item)) is not None
    }
    emit(
        "action_interrupted_trace_corrections_total",
        len(interrupted_action_corrections),
    )
    emit(
        "action_interrupted_legacy_unattributed_corrections_total",
        sum(
            item.get("corrected_status")
            == "revoked_interrupted_trace_non_authored"
            and interrupted_correction_identity(item) is None
            for item in interrupted_correction_records
        ),
    )
    for index, item in enumerate(action_receipts[-5:], start=1):
        correction = interrupted_action_corrections.get(
            exact_response_identity(item)
        )
        for key in (
            "recorded_at_unix_ms",
            "decision_source",
            "declared_next",
            "unexecuted_intention",
            "validation_reason",
            "execution_error",
        ):
            emit(f"recent_action_{index}_{key}", item.get(key, "none"))
        emit(
            f"recent_action_{index}_status",
            correction.get("corrected_status") if correction else item.get("status", "none"),
        )
        emit(
            f"recent_action_{index}_authored",
            not bool(correction)
            and item.get("decision_source")
            in {
                "astrid_declared",
                "local_format_repair_preserved_astrid_declaration",
            },
        )
        emit(f"recent_action_{index}_artifact", item.get("artifact_path") or "none")

    artifact_directories = (
        "journal",
        "memories",
        "introspections",
        "proposals",
        "notices",
        "daydreams",
        "aspirations",
        "research",
        "measurements",
        "research/syntheses",
        "studies/definitions",
        "studies/results",
        "self",
        "peer/outbox",
        "peer/inbox",
        "peer/read",
        "plans",
        "workshop/drafts",
        "workshop/revisions",
        "workshop/checks",
        "inbox",
        "autonomous/turns",
        "autonomous/recoveries",
        "perception/observations",
    )
    for relative in artifact_directories:
        directory = workspace / relative
        count = sum(path.is_file() and not path.is_symlink() for path in directory.iterdir()) if directory.is_dir() else 0
        emit(f"artifact_count_{relative.replace('/', '_')}", count)
    journal_dir = workspace / "journal"
    emit(
        "artifact_count_journal_automatic_signals",
        sum(
            path.is_file() and not path.is_symlink()
            for path in journal_dir.glob("signal_*.md")
        ),
    )
    emit(
        "artifact_count_journal_self_declared",
        sum(
            path.is_file() and not path.is_symlink()
            for path in journal_dir.glob("journal_*.md")
        ),
    )
    research_dir = workspace / "research"
    emit(
        "artifact_count_research_sources",
        sum(
            path.is_file() and not path.is_symlink()
            for path in research_dir.glob("source_*.md")
        ),
    )
    emit(
        "artifact_count_research_readable_sources",
        sum(
            path.is_file()
            and not path.is_symlink()
            and any(
                marker in path.read_text(errors="replace")
                for marker in (
                    "Extraction: html_visible_text_v1",
                    "Extraction: html_main_abstract_visible_text_v2",
                )
            )
            for path in research_dir.glob("source_*.md")
        ),
    )
    study_registry = read_json(workspace / "studies/registry.json")
    active_study = study_registry.get("active") or {}
    emit("study_active_id", active_study.get("study_id", "none"))
    emit("study_active_samples", active_study.get("sample_count", 0))
    emit("study_active_completes_at_unix_ms", active_study.get("completes_at_unix_ms", 0))
    study_receipts = [
        item
        for item in read_json_lines(workspace / "studies/receipts.jsonl")
        if int(item.get("recorded_at_unix_ms", 0) or 0) >= cutoff_ms
    ]
    emit("study_window_starts", sum(item.get("phase") == "started" for item in study_receipts))
    emit("study_window_midpoints", sum(item.get("phase") == "midpoint" for item in study_receipts))
    emit("study_window_completions", sum(item.get("phase") == "completed" for item in study_receipts))
    emit("study_window_cancellations", sum(item.get("phase") == "cancelled" for item in study_receipts))
    duplicate_notices = [
        item
        for item in read_json_lines(workspace / "research/duplication_notices.jsonl")
        if int(item.get("recorded_at_unix_ms", 0) or 0) >= cutoff_ms
    ]
    emit("duplicate_journal_window_notices", len(duplicate_notices))
    peer_receipts = [
        item
        for item in read_json_lines(workspace / "peer/receipts.jsonl")
        if int(item.get("recorded_at_unix_ms", 0) or 0) >= cutoff_ms
    ]
    for phase in ("shared", "available_unread", "voluntarily_read"):
        emit(f"peer_window_{phase}", sum(item.get("phase") == phase for item in peer_receipts))
    profile_value = read_json(workspace / "self/profile.json")
    emit("self_profile_schema", profile_value.get("schema", "none"))
    emit("self_profile_generated_at_unix_ms", profile_value.get("generated_at_unix_ms", 0))
    inquiry_harness_root = astrid_root / "operator/inquiry-harness"
    inquiry_harness_runs = sorted(
        path
        for path in inquiry_harness_root.glob("run_*")
        if path.is_dir() and not path.is_symlink()
    )
    emit("operator_inquiry_harness_runs", len(inquiry_harness_runs))
    if inquiry_harness_runs:
        latest_harness = read_json(inquiry_harness_runs[-1] / "result.json")
        emit("operator_inquiry_harness_latest_status", latest_harness.get("status", "unknown"))
        emit(
            "operator_inquiry_harness_latest_completed_at_unix_ms",
            latest_harness.get("completed_at_unix_ms", 0),
        )
    transport_pattern = re.compile(
        r"Request timed out \([A-Za-z]+ phase exceeded \d+s limit\)"
    )
    authored_paths = [
        path
        for relative in ("journal", "autonomous/turns")
        for path in (workspace / relative).glob("*.md")
        if path.is_file() and not path.is_symlink()
    ]
    emit(
        "authorship_unreclassified_transport_sentinels",
        sum(
            bool(transport_pattern.search(path.read_text(errors="replace")))
            for path in authored_paths
        ),
    )
    emit(
        "authorship_visible_thinking_marker_files",
        sum(
            any(
                marker in path.read_text(errors="replace")
                for marker in ("<think>", "</think>")
            )
            for path in authored_paths
        ),
    )
    emit(
        "authorship_correction_records",
        len(read_json_lines(workspace / "autonomous/authorship_corrections.jsonl")),
    )

    all_web_receipts = [
        item
        for item in read_json_lines(workspace / "web/receipts.jsonl")
        if int(item.get("recorded_at_unix_ms", 0)) >= cutoff_ms
    ]
    web_requests = [
        item for item in all_web_receipts if item.get("phase") == "requested"
    ]
    web_receipts = [
        item
        for item in all_web_receipts
        if item.get("phase") == "completed" or item.get("phase") is None
    ]
    completed_call_ids = {
        str(item.get("call_id", ""))
        for item in all_web_receipts
        if item.get("phase") == "completed"
    }
    pending_web = [
        item
        for item in web_requests
        if str(item.get("call_id", "")) not in completed_call_ids
    ]
    stale_web = [
        item
        for item in pending_web
        if now_ms - int(item.get("requested_at_unix_ms", 0) or 0) >= 5 * 60_000
    ]
    emit("web_window_requested_calls", len(web_requests))
    emit("web_window_completed_calls", len(web_receipts))
    emit("web_window_pending_calls", len(pending_web))
    emit("web_window_stale_calls", len(stale_web))
    emit(
        "web_window_attributed_calls",
        sum(
            isinstance(item.get("trace"), dict)
            and item.get("origin") not in (None, "legacy_unattributed")
            for item in web_receipts
        ),
    )
    emit(
        "web_window_unattributed_calls",
        sum(
            not isinstance(item.get("trace"), dict)
            or item.get("origin") in (None, "legacy_unattributed")
            for item in web_receipts
        ),
    )
    origins = sorted(
        {
            str(item.get("origin") or "legacy_unattributed")
            for item in all_web_receipts
        }
    )
    for origin in origins:
        emit(
            f"web_window_origin_{re.sub(r'[^a-zA-Z0-9]+', '_', origin).strip('_')}_events",
            sum(
                str(item.get("origin") or "legacy_unattributed") == origin
                for item in all_web_receipts
            ),
        )
    for tool_name in ("search_web", "fetch_url"):
        matching = [item for item in web_receipts if item.get("tool_name") == tool_name]
        emit(f"web_window_{tool_name}_calls", len(matching))
        emit(
            f"web_window_{tool_name}_successes",
            sum(item.get("status") == "success" for item in matching),
        )
    for index, item in enumerate(web_receipts[-3:], start=1):
        arguments = item.get("arguments")
        arguments = arguments if isinstance(arguments, dict) else {}
        summary = item.get("result_summary")
        summary = summary if isinstance(summary, dict) else {}
        emit(f"recent_web_{index}_recorded_at_unix_ms", item.get("recorded_at_unix_ms", 0))
        emit(f"recent_web_{index}_tool_name", item.get("tool_name", "unknown"))
        emit(f"recent_web_{index}_status", item.get("status", "unknown"))
        emit(
            f"recent_web_{index}_subject",
            arguments.get("query") or arguments.get("url") or "unknown",
        )
        emit(
            f"recent_web_{index}_result_count",
            summary.get("result_count", "not_applicable"),
        )
        emit(
            f"recent_web_{index}_http_status",
            summary.get("status", "not_applicable"),
        )
        emit(f"recent_web_{index}_origin", item.get("origin", "legacy_unattributed"))
        trace = item.get("trace")
        emit(
            f"recent_web_{index}_trace_id",
            trace.get("trace_id", "unattributed") if isinstance(trace, dict) else "unattributed",
        )

    all_introspection = [
        item
        for item in read_json_lines(workspace / "introspection/receipts.jsonl")
        if int(item.get("recorded_at_unix_ms", 0) or 0) >= cutoff_ms
    ]
    introspection_requests = [
        item for item in all_introspection if item.get("phase") == "requested"
    ]
    introspection_results = [
        item for item in all_introspection if item.get("phase") == "completed"
    ]
    introspection_completed_ids = {
        str(item.get("call_id", "")) for item in introspection_results
    }
    pending_introspection = [
        item
        for item in introspection_requests
        if str(item.get("call_id", "")) not in introspection_completed_ids
    ]
    emit("introspection_window_requested_calls", len(introspection_requests))
    emit("introspection_window_completed_calls", len(introspection_results))
    emit(
        "introspection_window_successes",
        sum(item.get("status") == "success" for item in introspection_results),
    )
    emit("introspection_window_pending_calls", len(pending_introspection))
    emit(
        "introspection_window_stale_calls",
        sum(
            now_ms - int(item.get("requested_at_unix_ms", 0) or 0)
            >= 5 * 60_000
            for item in pending_introspection
        ),
    )
    if introspection_results:
        latest_introspection = introspection_results[-1]
        summary = latest_introspection.get("result_summary")
        summary = summary if isinstance(summary, dict) else {}
        emit(
            "introspection_latest_status",
            latest_introspection.get("status", "unknown"),
        )
        emit(
            "introspection_latest_origin",
            latest_introspection.get("origin", "unknown"),
        )
        emit(
            "introspection_latest_parent_response_sha256",
            latest_introspection.get("parent_response_sha256") or "none",
        )
        emit(
            "introspection_latest_match_count",
            summary.get("match_count", 0),
        )

    scheduled_summary = scheduled_introspection_summary(
        workspace, cutoff_ms, now_ms, args.scheduled_authorship_verify_key
    )
    for field, value in scheduled_summary.items():
        emit(f"scheduled_introspection_{field}", value)

    resolved_self_change_root = configured_self_change_root(
        workspace, profile, home
    )
    change_summary = self_change_summary(
        workspace, resolved_self_change_root, cutoff_ms
    )
    for field, value in change_summary.items():
        emit(f"self_change_{field}", value)

    window_actions = [
        item
        for item in action_receipts
        if int(item.get("recorded_at_unix_ms", 0)) >= cutoff_ms
    ]
    def action_provenance(item: dict[str, Any]) -> str:
        if exact_response_identity(item) in interrupted_action_corrections:
            return "revoked_interrupted_trace_non_authored"
        return str(item.get("decision_source", "unknown"))

    provenances = sorted({action_provenance(item) for item in window_actions})
    for provenance in provenances:
        emit(
            f"action_window_provenance_{re.sub(r'[^a-zA-Z0-9]+', '_', provenance).strip('_')}",
            sum(action_provenance(item) == provenance for item in window_actions),
        )

    window_chains = [
        item
        for item in read_json_lines(workspace / "autonomous/chains.jsonl")
        if int(item.get("recorded_at_unix_ms", 0)) >= cutoff_ms
    ]
    window_recoveries = [
        item
        for item in read_json_lines(workspace / "autonomous/recoveries.jsonl")
        if int(item.get("completed_at_unix_ms", 0)) >= cutoff_ms
    ]
    trace_records = [
        *[
            item
            for item in recent_runs
            if int(item.get("completed_at_unix_ms", 0)) >= cutoff_ms
        ],
        *window_actions,
        *window_chains,
        *window_recoveries,
        *all_web_receipts,
        *all_introspection,
        *[
            item
            for item, _sources, _occurrences in scheduled_introspection_receipts(workspace)
            if int(item.get("completed_at_unix_ms", 0) or 0) >= cutoff_ms
        ],
        *spectral_rollups,
        *tuning_receipts,
    ]
    traced_records = sum(isinstance(item.get("trace"), dict) for item in trace_records)
    emit("activity_window_records", len(trace_records))
    emit("activity_window_traced_records", traced_records)
    emit("activity_window_untraced_records", len(trace_records) - traced_records)
    emit(
        "activity_window_trace_coverage_pct",
        f"{100 * traced_records / len(trace_records):.1f}" if trace_records else "not_applicable",
    )

    hindsight = read_json(astrid_root / "operator/hindsight/latest.json")
    hindsight_time = int(hindsight.get("recorded_at_unix_ms", 0) or 0)
    hindsight_ledgers = hindsight.get("ledgers")
    hindsight_ledgers = hindsight_ledgers if isinstance(hindsight_ledgers, dict) else {}
    emit("hindsight_checkpoint_present", bool(hindsight))
    emit("hindsight_checkpoint_recorded_at_unix_ms", hindsight_time)
    emit(
        "hindsight_checkpoint_age_ms",
        max(0, now_ms - hindsight_time) if hindsight_time else "unavailable",
    )
    emit("hindsight_operator_root", astrid_root / "operator/hindsight")
    emit("hindsight_artifact_inventory_count", hindsight.get("artifact_inventory_count", 0))
    emit("hindsight_latest_artifacts_discovered", hindsight.get("artifacts_discovered", 0))
    emit("hindsight_latest_fill_rollups_completed", hindsight.get("fill_rollups_completed", 0))
    emit("hindsight_checkpoint_schema", hindsight.get("schema", "unavailable"))
    emit("hindsight_continuity_epoch", hindsight.get("continuity_epoch", "unavailable"))
    emit("hindsight_continuity_status", hindsight.get("continuity_status", "unavailable"))
    emit(
        "hindsight_continuity_from_previous_checkpoint_valid",
        hindsight.get("continuity_from_previous_checkpoint_valid", "unavailable"),
    )
    emit(
        "hindsight_historical_integrity_violation_count",
        hindsight.get("historical_ledger_integrity_violation_count", 0),
    )
    emit(
        "hindsight_legacy_race_compatible_unresolved_violation_count",
        hindsight.get("legacy_race_compatible_unresolved_violation_count", 0),
    )
    emit(
        "hindsight_current_epoch_integrity_violation_count",
        hindsight.get("current_epoch_integrity_violation_count", 0),
    )
    emit(
        "hindsight_checkpoint_invalid_json_lines",
        sum(
            int(value.get("invalid_json_lines", 0) or 0)
            for value in hindsight_ledgers.values()
            if isinstance(value, dict)
        ),
    )
    for database_name in ("state_database", "audit_database"):
        database = hindsight.get(database_name)
        database = database if isinstance(database, dict) else {}
        emit(f"hindsight_{database_name}_present", database.get("present", False))
        emit(f"hindsight_{database_name}_size_bytes", database.get("size_bytes", 0))
        emit(f"hindsight_{database_name}_file_count", database.get("file_count", 0))
        emit(
            f"hindsight_{database_name}_owner_only_files",
            database.get("owner_only_files", "unknown"),
        )
    audit_database = hindsight.get("audit_database")
    audit_database = audit_database if isinstance(audit_database, dict) else {}
    emit(
        "hindsight_audit_integrity_alerts_in_retained_logs",
        audit_database.get("integrity_alerts_in_retained_daemon_logs", "unknown"),
    )
    operator_database = hindsight.get("operator_hindsight_database")
    operator_database = operator_database if isinstance(operator_database, dict) else {}
    emit(
        "hindsight_query_database_quick_check",
        operator_database.get("quick_check", "unavailable"),
    )
    emit(
        "hindsight_query_database_owner_only",
        operator_database.get("owner_only", "unknown"),
    )
    for table, count in sorted((operator_database.get("row_counts") or {}).items()):
        emit(f"hindsight_query_database_{table}_rows", count)
    emit(
        "hindsight_checkpoint_authority",
        hindsight.get("authority", "unavailable"),
    )

    recent_activity: list[tuple[int, str, dict[str, Any]]] = []
    recent_activity.extend(
        (
            int(item.get("completed_at_unix_ms", 0)),
            "turn",
            item,
        )
        for item in recent_runs
        if int(item.get("completed_at_unix_ms", 0)) >= cutoff_ms
    )
    recent_activity.extend(
        (int(item.get("recorded_at_unix_ms", 0)), "action", item)
        for item in window_actions
    )
    recent_activity.extend(
        (int(item.get("recorded_at_unix_ms", 0)), "chain", item)
        for item in window_chains
    )
    recent_activity.extend(
        (int(item.get("recorded_at_unix_ms", 0)), "web", item)
        for item in all_web_receipts
    )
    recent_activity.extend(
        (int(item.get("recorded_at_unix_ms", 0)), "introspection", item)
        for item in all_introspection
    )
    recent_activity.extend(
        (
            int(item.get("completed_at_unix_ms", 0)),
            "scheduled_introspection",
            {
                **item,
                "_source_ledger": sources[0],
                "_source_ledgers": list(sources),
                "_exact_duplicate_count": occurrences - 1,
            },
        )
        for item, sources, occurrences in scheduled_introspection_receipts(workspace)
        if int(item.get("completed_at_unix_ms", 0) or 0) >= cutoff_ms
    )
    recent_activity.extend(
        (int(item.get("recorded_at_unix_ms", 0)), "perception", item)
        for item in observation_rows
        if int(item.get("recorded_at_unix_ms", 0) or 0) >= cutoff_ms
    )
    recent_activity.extend(
        (int(item.get("recorded_at_unix_ms", 0)), "spectral", item)
        for item in spectral_rollups
    )
    recent_activity.extend(
        (int(item.get("recorded_at_unix_ms", 0)), "tuning", item)
        for item in tuning_receipts
    )
    recent_activity.sort(key=lambda value: value[0])
    for index, (recorded_at, kind, item) in enumerate(recent_activity[-5:], start=1):
        trace = item.get("trace")
        emit(f"recent_activity_{index}_recorded_at_unix_ms", recorded_at)
        emit(f"recent_activity_{index}_kind", kind)
        emit(
            f"recent_activity_{index}_source_ledger",
            item.get("_source_ledger", "not_applicable"),
        )
        emit(
            f"recent_activity_{index}_trace_id",
            trace.get("trace_id", "unattributed") if isinstance(trace, dict) else "unattributed",
        )
        emit(
            f"recent_activity_{index}_detail",
            item.get("declared_next")
            or item.get("tool_name")
            or item.get("transition")
            or item.get("summary")
            or item.get("phase")
            or item.get("mode_identity_state")
            or item.get("reflection_path")
            or item.get("status")
            or "unknown",
        )

    meminfo: dict[str, int] = {}
    for line in Path("/proc/meminfo").read_text().splitlines():
        name, value, *_ = line.split()
        meminfo[name.rstrip(":")] = int(value)
    emit("memory_total_mib", round(meminfo.get("MemTotal", 0) / 1024))
    emit("memory_available_mib", round(meminfo.get("MemAvailable", 0) / 1024))
    emit(
        "swap_used_mib",
        round((meminfo.get("SwapTotal", 0) - meminfo.get("SwapFree", 0)) / 1024),
    )
    load = Path("/proc/loadavg").read_text().split()
    emit("load_1m", load[0])
    emit("load_5m", load[1])
    emit("load_15m", load[2])
    process_rows = command("ps", "-eo", "pcpu=,args=")
    llama_cpu = sum(
        float(line.split(maxsplit=1)[0])
        for line in process_rows.splitlines()
        if "llama-server " in line
    )
    emit("ollama_llama_server_cpu_pct", f"{llama_cpu:.1f}")

    try:
        with urllib.request.urlopen("http://127.0.0.1:11434/api/ps", timeout=2) as response:
            models = json.load(response).get("models", [])
    except (OSError, ValueError):
        models = []
    emit("ollama_loaded_model_count", len(models))
    emit("ollama_loaded_models", ",".join(str(item.get("name", "")) for item in models))
    emit(
        "ollama_loaded_size_mib",
        round(sum(int(item.get("size", 0)) for item in models) / 1024 / 1024),
    )

    since = f"{args.window_minutes} minutes ago"
    journal_arguments = ["journalctl"]
    if not SYSTEM_SERVICE_MANAGER:
        journal_arguments.append("--user")
    journal_arguments.extend(
        ("-u", "astrid.service", "--since", since, "--no-pager")
    )
    logs = command(*journal_arguments)
    log_path = astrid_root / f"log/astrid.{time.strftime('%Y-%m-%d', time.gmtime())}.log"
    try:
        cutoff_iso = time.strftime(
            "%Y-%m-%dT%H:%M:%S",
            time.gmtime(time.time() - args.window_minutes * 60),
        )
        file_logs = "\n".join(
            line
            for line in log_path.read_text(errors="replace").splitlines()
            if line.strip() and line.split(maxsplit=1)[0] >= cutoff_iso
        )
    except OSError:
        file_logs = ""
    logs = f"{logs}\n{file_logs}"
    emit("recent_search_web_log_mentions", logs.count("search_web"))
    emit("recent_fetch_url_log_mentions", logs.count("fetch_url"))
    header_times = [
        int(value)
        for value in re.findall(
            r"HTTP stream response headers received.*?elapsed_ms=(\d+)", logs
        )
    ]
    emit("recent_stream_header_events", len(header_times))
    emit("recent_stream_header_max_elapsed_ms", max(header_times, default=0))
    emit("recent_stream_header_p95_elapsed_ms", percentile(header_times))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
