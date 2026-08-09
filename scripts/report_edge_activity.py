#!/usr/bin/env python3
"""Read-only, identifier-based activity timeline for one edge Astrid.

Ledger decoding, exact causal joins, filters, and deterministic text/JSON
rendering stay together so every output format uses the same no-timestamp-join
policy.  A later split should expose one tested normalized-event API before
moving renderers into separate modules.
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
from typing import Any, Iterable, NamedTuple

SCHEMA = "astrid_edge_activity_report_v3"
AUTHORSHIP_ATTRIBUTION_VERSION = 7
STALE_WEB_CALL_MS = 5 * 60_000
STALE_INTROSPECTION_CALL_MS = 5 * 60_000
TRANSPORT_AUTHORSHIP_CORRECTION_REASON = (
    "legacy_transport_sentinel_reclassified_non_authored"
)
MAX_TRACE_LABEL_CHARS = 96
INTERRUPTED_ACTION_CORRECTION_SCHEMA = (
    "astrid_edge_interrupted_action_correction_v2"
)
ACTION_DISPATCH_SCHEMA = "astrid_edge_action_dispatch_v1"
MODEL_RESPONSE_PROVENANCES = frozenset(
    {
        "model_authored",
        "model_authored_with_local_safe_fallback",
        "model_authored_with_local_format_repair",
    }
)
NON_AUTHORED_STATUS_PROVENANCES = {
    "failed": "failed_non_authored",
    "interrupted": "interrupted_non_authored",
    "transport_recovery": "transport_recovery_non_authored",
}
RESPONSE_PROVENANCE_COUNTER_KEYS = (
    "executor_terminal_error",
    "failed_non_authored",
    "interrupted_non_authored",
    "invalid",
    "legacy_unspecified",
    "model_authored",
    "model_authored_with_local_format_repair",
    "model_authored_with_local_safe_fallback",
    "transport_recovery_non_authored",
)
REQUEST_HEADER_LATENCY_SOURCE_V1 = "kernel_http_host_trace_v1"
SCHEDULED_INTROSPECTION_RECEIPT_SCHEMA = (
    "astrid_edge_scheduled_introspection_v1"
)
SCHEDULED_INTROSPECTION_PROVENANCE = "model_authored_runtime_scheduled"
SCHEDULED_INTROSPECTION_ADMISSION_SCHEMA = (
    "astrid.edge.scheduled_introspection.admission.v1"
)
SCHEDULED_INTROSPECTION_LEDGER_PATHS = (
    "introspections/scheduled/receipts.jsonl",
    "introspection/scheduled/receipts.jsonl",
)
SELF_CHANGE_OPERATOR_STATUS_PATH = Path(
    "/var/lib/astrid-edge-operator/operator-status.json"
)
SELF_CHANGE_OPERATOR_STATUS_SCHEMA = (
    "astrid.edge_self_change.operator_status_envelope.v1"
)
SELF_CHANGE_OPERATOR_CORE_SCHEMA = "astrid.edge_self_change.operator_status.v3"
LEGACY_SELF_CHANGE_OPERATOR_CORE_SCHEMAS = frozenset(
    {
        "astrid.edge_self_change.operator_status.v1",
        "astrid.edge_self_change.operator_status.v2",
    }
)
SELF_CHANGE_OPERATOR_EVENT_SCHEMA = (
    "astrid.edge_self_change.operator_lifecycle_event.v1"
)
SELF_CHANGE_OPERATOR_LIFECYCLE_SCHEMA = (
    "astrid.edge_self_change.operator_lifecycle.v1"
)
SELF_CHANGE_OPERATOR_PROVENANCE = "immutable_supervisor_sanitized_projection"
SELF_CHANGE_EVENT_PROVENANCE = (
    "immutable_supervisor_signed_ledger_sanitized_metadata"
)
SELF_CHANGE_EVENT_AUTHORITY = "observation_only_not_deployment_or_astrid_authorship"
SELF_CHANGE_OPERATOR_MAX_BYTES = 256 * 1024
SELF_CHANGE_OPERATOR_MAX_EVENTS = 64
SELF_CHANGE_FACETS = frozenset(
    {
        "reflection",
        "candidate",
        "build",
        "test",
        "invariant",
        "shadow",
        "activation",
        "restart",
        "probation",
        "rollback",
        "operator",
    }
)
PATCH_EXPORT_SUMMARY_SCHEMA = (
    "astrid.edge.steward_helper.owner_patch_export_summary_envelope.v1"
)
PATCH_EXPORT_SUMMARY_CORE_SCHEMA = (
    "astrid.edge.steward_helper.owner_patch_export_summary.v1"
)
INDIRECT_SHADOW_GATE_EVIDENCE = (
    "indirect_package_replay_sha256_commitment_not_independently_reinspectable"
)
CANDIDATE_PRESENTATION_INPUT_SCHEMA = (
    "astrid.edge_candidate_presentation.input.v1"
)
CANDIDATE_PRESENTATION_CONTENT_SCHEMA = (
    "astrid.edge_candidate_presentation.content.v1"
)
CANDIDATE_PRESENTATION_INPUT_MAX_BYTES = 256 * 1024


class RunAuthorship(NamedTuple):
    status: str
    authored: bool
    fallback: bool
    correction: dict[str, Any] | None
    response_provenance: str
    local_safe_fallback_used: bool
    local_format_repair_used: bool


def normalized_uuid(value: Any) -> str | None:
    """Return a canonical, non-nil UUID or ``None``.

    Nil identifiers do not identify an event and therefore must never be
    presented as first-class causal telemetry.
    """
    try:
        parsed = uuid.UUID(str(value))
    except (TypeError, ValueError, AttributeError):
        return None
    return str(parsed) if parsed.int != 0 else None


def valid_trace_label(value: Any, *, required: bool) -> bool:
    if value is None:
        return not required
    if (
        not isinstance(value, str)
        or not value.strip()
        or len(value) > MAX_TRACE_LABEL_CHARS
    ):
        return False
    return not any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)


def normalized_trace_label(value: Any) -> str | None:
    return str(value) if valid_trace_label(value, required=False) and value else None


def valid_trace(value: dict[str, Any]) -> bool:
    trace = value.get("trace")
    if not isinstance(trace, dict) or trace.get("schema_version", 1) != 1:
        return False
    if normalized_uuid(trace.get("trace_id")) is None:
        return False
    if normalized_uuid(trace.get("span_id")) is None:
        return False
    if trace.get("parent_span_id") is not None and normalized_uuid(
        trace.get("parent_span_id")
    ) is None:
        return False
    if (
        normalized_uuid(trace.get("parent_span_id"))
        == normalized_uuid(trace.get("span_id"))
    ):
        return False
    if trace.get("turn_id") is not None and normalized_uuid(
        trace.get("turn_id")
    ) is None:
        return False
    if not valid_trace_label(trace.get("session_id"), required=True):
        return False
    if not valid_trace_label(trace.get("chain_id"), required=False):
        return False
    return True


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
    outcomes = {
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
        if (
            attempt_id is None
            or attempt_id in attempt_ids
            or normalized_uuid(request.get("request_id")) is None
            or not isinstance(request.get("outcome"), str)
            or request.get("outcome") not in outcomes
        ):
            return None
        attempt_ids.add(attempt_id)
        latency = request.get("request_header_latency_ms")
        if request.get("outcome") == "successful_headers":
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


def exact_request_header_latency(
    run: dict[str, Any],
) -> tuple[int, str, int] | None:
    """Return only a one-attempt/one-success trace-bound kernel-host latency."""
    telemetry = exact_provider_request_telemetry(run)
    if telemetry is None or telemetry[:2] != (1, 1):
        return None
    latency = run.get("request_header_latency_ms")
    request_id = normalized_uuid(run.get("provider_request_id"))
    if (
        isinstance(latency, bool)
        or not isinstance(latency, int)
        or request_id is None
    ):
        return None
    return latency, request_id, 1


def legacy_unattributed_header_latency(run: dict[str, Any]) -> int | float | None:
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
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value)
    )


def exact_response_identity(
    value: dict[str, Any], *, flat_identity: bool = False
) -> tuple[str, str, str, str] | None:
    """Bind response text to an exact turn/trace event identity."""
    response_hash = value.get("response_sha256")
    trace = value.get("trace")
    if isinstance(trace, dict):
        if trace.get("schema_version", 1) != 1:
            return None
        exact_trace_id = normalized_uuid(trace.get("trace_id"))
        exact_turn_id = normalized_uuid(trace.get("turn_id"))
    elif flat_identity:
        exact_trace_id = normalized_uuid(value.get("trace_id"))
        exact_turn_id = normalized_uuid(value.get("turn_id"))
    else:
        return None
    if not valid_response_sha256(response_hash) or exact_trace_id is None:
        return None
    if exact_turn_id is not None:
        return "turn", exact_turn_id, exact_trace_id, str(response_hash)
    return "trace", "", exact_trace_id, str(response_hash)


def interrupted_correction_identity(
    value: dict[str, Any],
) -> tuple[str, str, str, str] | None:
    if value.get("schema") != INTERRUPTED_ACTION_CORRECTION_SCHEMA:
        return None
    return exact_response_identity(value, flat_identity=True)


def action_dispatch_integrity_error(value: dict[str, Any]) -> str | None:
    if value.get("schema") != ACTION_DISPATCH_SCHEMA:
        return "unsupported_schema"
    if value.get("phase") not in {"requested", "completed"}:
        return "unsupported_phase"
    top_turn_id = normalized_uuid(value.get("turn_id"))
    if top_turn_id is None:
        return "invalid_turn_id"
    if not valid_response_sha256(value.get("response_sha256")):
        return "invalid_response_sha256"
    if not valid_trace(value):
        return "invalid_trace"
    trace = value["trace"]
    if normalized_uuid(trace.get("turn_id")) != top_turn_id:
        return "trace_turn_id_mismatch"
    return None


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


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return {}
    return value if isinstance(value, dict) else {}


def self_change_operator_status_path(workspace: Path) -> Path:
    """Return the sole root-owned production projection path.

    ``workspace`` remains in the signature for API compatibility, but never
    influences this trust decision. Tests inject a separate path explicitly
    through ``collect_events`` and opt into its non-root fixture allowance.
    """

    del workspace
    return SELF_CHANGE_OPERATOR_STATUS_PATH


def read_self_change_operator_status(
    path: Path = SELF_CHANGE_OPERATOR_STATUS_PATH,
    *,
    test_only_allow_unprivileged_owner: bool = False,
) -> dict[str, Any]:
    """Read one bounded, hash-bound immutable-supervisor operator projection."""

    if path != SELF_CHANGE_OPERATOR_STATUS_PATH and not test_only_allow_unprivileged_owner:
        return {}
    expected_uid = os.geteuid() if test_only_allow_unprivileged_owner else 0

    try:
        parent = path.parent.lstat()
        before = path.lstat()
        if (
            not stat.S_ISDIR(parent.st_mode)
            or stat.S_ISLNK(parent.st_mode)
            or parent.st_uid != expected_uid
            or parent.st_mode & 0o022
            or parent.st_mode & stat.S_ISGID == 0
            or not stat.S_ISREG(before.st_mode)
            or stat.S_ISLNK(before.st_mode)
            or before.st_nlink != 1
            or before.st_uid != expected_uid
            or before.st_gid != parent.st_gid
            or stat.S_IMODE(before.st_mode) != 0o640
            or before.st_size > SELF_CHANGE_OPERATOR_MAX_BYTES
        ):
            return {}
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        try:
            opened = os.fstat(descriptor)
            data = os.read(descriptor, SELF_CHANGE_OPERATOR_MAX_BYTES + 1)
            after = os.fstat(descriptor)
        finally:
            os.close(descriptor)
        current = path.lstat()
    except OSError:
        return {}
    identity = lambda value: (
        value.st_dev,
        value.st_ino,
        value.st_size,
        value.st_mtime_ns,
    )
    if (
        len(data) > SELF_CHANGE_OPERATOR_MAX_BYTES
        or len(data) != before.st_size
        or identity(before) != identity(opened)
        or identity(opened) != identity(after)
        or identity(after) != identity(current)
    ):
        return {}
    try:
        envelope = json.loads(data)
    except (UnicodeError, json.JSONDecodeError):
        return {}
    if not isinstance(envelope, dict) or set(envelope) != {
        "schema",
        "core",
        "core_sha256",
    }:
        return {}
    core = envelope.get("core")
    if (
        envelope.get("schema") != SELF_CHANGE_OPERATOR_STATUS_SCHEMA
        or not isinstance(core, dict)
        or core.get("provenance") != SELF_CHANGE_OPERATOR_PROVENANCE
        or core.get("authority") != "observation_only_not_deployment_authority"
        or not valid_response_sha256(envelope.get("core_sha256"))
    ):
        return {}
    try:
        encoded = json.dumps(
            core,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError):
        return {}
    if hashlib.sha256(encoded).hexdigest() != envelope["core_sha256"]:
        return {}
    if core.get("schema") in LEGACY_SELF_CHANGE_OPERATOR_CORE_SCHEMAS:
        if not _valid_legacy_self_change_operator(core):
            return {}
        return core | {"projection_core_sha256": envelope["core_sha256"]}
    if not _valid_self_change_operator_v3(core):
        return {}
    return core | {"projection_core_sha256": envelope["core_sha256"]}


def _valid_legacy_self_change_operator(core: dict[str, Any]) -> bool:
    base = {
        "schema",
        "appliance_id",
        "generated_at",
        "state_revision",
        "mode",
        "active_generation",
        "previous_generation",
        "pipeline_phase",
        "latest_transition",
        "provenance",
        "authority",
    }
    expected = (
        base
        if core.get("schema") == "astrid.edge_self_change.operator_status.v1"
        else base | {"restart_expectation"}
    )
    return (
        set(core) == expected
        and isinstance(core.get("latest_transition"), dict)
        and (
            core.get("schema") == "astrid.edge_self_change.operator_status.v1"
            or _valid_restart_expectation(core.get("restart_expectation"))
        )
    )


def _valid_self_change_operator_v3(core: dict[str, Any]) -> bool:
    if set(core) != {
        "schema",
        "appliance_id",
        "generated_at",
        "state_revision",
        "mode",
        "active_generation",
        "previous_generation",
        "pipeline_phase",
        "latest_transition",
        "restart_expectation",
        "lifecycle",
        "provenance",
        "authority",
    } or core.get("schema") != SELF_CHANGE_OPERATOR_CORE_SCHEMA:
        return False
    restart = core.get("restart_expectation")
    lifecycle = core.get("lifecycle")
    transition = core.get("latest_transition")
    if (
        any(
            not _valid_operator_label(core.get(name), required=True)
            for name in (
                "appliance_id",
                "mode",
                "active_generation",
                "previous_generation",
                "pipeline_phase",
            )
        )
        or isinstance(core.get("generated_at"), bool)
        or not isinstance(core.get("generated_at"), int)
        or core["generated_at"] < 0
        or isinstance(core.get("state_revision"), bool)
        or not isinstance(core.get("state_revision"), int)
        or core["state_revision"] < 0
        or not isinstance(transition, dict)
        or set(transition) != {"operation", "status"}
        or not _valid_operator_label(transition.get("operation"), required=True)
        or not _valid_operator_label(transition.get("status"), required=True)
        or not _valid_restart_expectation(restart)
        or not isinstance(lifecycle, dict)
        or set(lifecycle)
        != {
            "schema",
            "events",
            "included",
            "total",
            "truncated",
            "maximum_events",
            "ledger_heads",
        }
        or lifecycle.get("schema") != SELF_CHANGE_OPERATOR_LIFECYCLE_SCHEMA
        or lifecycle.get("maximum_events") != SELF_CHANGE_OPERATOR_MAX_EVENTS
        or isinstance(lifecycle.get("included"), bool)
        or not isinstance(lifecycle.get("included"), int)
        or isinstance(lifecycle.get("total"), bool)
        or not isinstance(lifecycle.get("total"), int)
        or not 0 <= lifecycle["included"] <= SELF_CHANGE_OPERATOR_MAX_EVENTS
        or lifecycle["total"] < lifecycle["included"]
        or lifecycle.get("truncated") is not (
            lifecycle["total"] > lifecycle["included"]
        )
        or not isinstance(lifecycle.get("events"), list)
        or len(lifecycle["events"]) != lifecycle["included"]
        or not _valid_ledger_heads(lifecycle.get("ledger_heads"))
    ):
        return False
    events = lifecycle["events"]
    if not all(_valid_operator_event(event) for event in events):
        return False
    identities = [
        (event["source_ledger"], event["event_id"], event["record_sha256"])
        for event in events
    ]
    ordering = [
        (event["recorded_at"], event["sequence"], event["source_ledger"])
        for event in events
    ]
    return len(identities) == len(set(identities)) and ordering == sorted(ordering)


def _valid_restart_expectation(value: Any) -> bool:
    return (
        isinstance(value, dict)
        and set(value) == {"phase", "maximum_seconds", "basis"}
        and value.get("phase") in {"none", "activation", "rollback"}
        and not isinstance(value.get("maximum_seconds"), bool)
        and isinstance(value.get("maximum_seconds"), int)
        and 0 <= value["maximum_seconds"] <= 7_200
        and (value["phase"] == "none") == (value["maximum_seconds"] == 0)
        and value.get("basis")
        == "immutable_command_profile_timeout_upper_bound"
    )


def _valid_ledger_heads(value: Any) -> bool:
    return (
        isinstance(value, dict)
        and set(value) == {"candidate", "build", "activation", "operator"}
        and all(item is None or valid_response_sha256(item) for item in value.values())
    )


def _valid_operator_event(value: Any) -> bool:
    exact = {
        "schema", "recorded_at", "source_ledger", "sequence", "event_id",
        "status", "facets", "record_sha256", "candidate_id", "candidate_sha256",
        "build_id", "generation_id", "from_generation", "trace_id", "session_id",
        "turn_id", "response_sha256", "terminal_declaration_sha256",
        "terminal_reason_sha256", "terminal_authority", "automatic_retry",
        "tests_sha256", "bundle_sha256", "manifest_sha256",
        "invariant_candidate_replay_sha256", "invariant_package_replay_sha256",
        "shadow_evidence_sha256", "shadow_status", "command_profile",
        "command_executable_sha256", "command_argv_sha256", "command_stdout_sha256",
        "command_stderr_sha256", "command_exit_code", "command_timed_out",
        "provenance", "authority", "authored", "fallback",
    }
    if not isinstance(value, dict) or set(value) != exact:
        return False
    facets = value.get("facets")
    labels = (
        "event_id", "status", "candidate_id", "build_id", "generation_id",
        "from_generation", "trace_id", "session_id", "turn_id", "command_profile",
    )
    hashes = (
        "record_sha256", "candidate_sha256", "response_sha256",
        "terminal_declaration_sha256", "terminal_reason_sha256", "tests_sha256",
        "bundle_sha256", "manifest_sha256", "invariant_candidate_replay_sha256",
        "invariant_package_replay_sha256", "shadow_evidence_sha256",
        "command_executable_sha256", "command_argv_sha256", "command_stdout_sha256",
        "command_stderr_sha256",
    )
    if (
        value.get("schema") != SELF_CHANGE_OPERATOR_EVENT_SCHEMA
        or value.get("source_ledger") not in {"candidate", "build", "activation", "operator"}
        or not isinstance(value.get("recorded_at"), int)
        or isinstance(value.get("recorded_at"), bool)
        or value["recorded_at"] < 0
        or not isinstance(value.get("sequence"), int)
        or isinstance(value.get("sequence"), bool)
        or value["sequence"] < 0
        or not isinstance(facets, list)
        or not facets
        or any(not isinstance(item, str) for item in facets)
        or facets != sorted(set(facets))
        or not set(facets).issubset(SELF_CHANGE_FACETS)
        or any(
            not _valid_operator_label(item, required=False)
            for item in (value.get(name) for name in labels)
        )
        or any(item is not None and not valid_response_sha256(item) for item in (value.get(name) for name in hashes))
        or value.get("provenance") != SELF_CHANGE_EVENT_PROVENANCE
        or value.get("authority") != SELF_CHANGE_EVENT_AUTHORITY
        or value.get("authored") is not False
        or value.get("fallback") is not False
        or not valid_response_sha256(value.get("record_sha256"))
        or not _valid_operator_label(value.get("event_id"), required=True)
        or not _valid_operator_label(value.get("status"), required=True)
    ):
        return False
    shadow_status = value.get("shadow_status")
    if shadow_status is not None and shadow_status != (
        "package_replay_hash_only_no_detailed_shadow_claim"
    ):
        return False
    if value.get("shadow_evidence_sha256") is None and shadow_status is not None:
        return False
    exit_code = value.get("command_exit_code")
    if exit_code is not None and (
        isinstance(exit_code, bool) or not isinstance(exit_code, int) or not -255 <= exit_code <= 255
    ):
        return False
    command_timed_out = value.get("command_timed_out")
    if command_timed_out is not None and type(command_timed_out) is not bool:
        return False
    terminal = value.get("status") == "scheduled_intent_terminal_rejected"
    terminal_fields_are_exact = (
        value.get("terminal_reason_sha256") is not None
        and value.get("terminal_authority")
        == "terminal_exact_candidate_rejection_no_promotion"
        and value.get("automatic_retry") is False
    )
    terminal_fields_are_absent = (
        value.get("terminal_reason_sha256") is None
        and value.get("terminal_authority") is None
        and value.get("automatic_retry") is None
    )
    return terminal_fields_are_exact if terminal else terminal_fields_are_absent


def _valid_operator_label(value: Any, *, required: bool) -> bool:
    if value is None:
        return not required
    return (
        isinstance(value, str)
        and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}", value) is not None
    )


def read_patch_export_summaries(workspace: Path) -> list[dict[str, Any]]:
    """Read only the signed export's bounded body-free companion record."""
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
            or not valid_response_sha256(envelope.get("core_sha256"))
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
        touched = core.get("touched_paths")
        counts = ("file_count", "added_lines", "removed_lines", "changed_lines")
        if (
            not isinstance(touched, list)
            or not 0 < len(touched) <= 25
            or any(
                not isinstance(value, str)
                or not value
                or len(value) > 240
                or value.startswith("/")
                or ".." in Path(value).parts
                for value in touched
            )
            or any(
                isinstance(core.get(name), bool)
                or not isinstance(core.get(name), int)
                or not 0 <= int(core[name]) <= 100_000
                for name in counts
            )
            or core["file_count"] != len(touched)
            or core["changed_lines"] > 4_000
        ):
            continue
        summaries.append(core | {"summary_path": path.name})
    return summaries


def appliance_state_root(workspace: Path) -> Path:
    """Return the state root only for the canonical ``home/default/edge`` layout."""
    try:
        if (
            workspace.name == "edge"
            and workspace.parent.name == "default"
            and workspace.parent.parent.name == "home"
        ):
            return workspace.parents[2]
    except IndexError:
        pass
    return workspace


def unix_timestamp_ms(value: Any) -> int:
    """Normalize supervisor seconds while retaining existing millisecond ledgers."""
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return 0
    integer = int(value)
    if integer <= 0:
        return 0
    return integer * 1_000 if integer < 100_000_000_000 else integer


def self_change_ledger_events(
    workspace: Path,
    operator_status_path: Path = SELF_CHANGE_OPERATOR_STATUS_PATH,
    *,
    test_only_allow_unprivileged_operator_status: bool = False,
) -> list[dict[str, Any]]:
    """Project supervisor metadata without exposing patches or command output."""
    events: list[dict[str, Any]] = []
    outbox = workspace / "self-change/outbox"
    try:
        intents = sorted(outbox.glob("intent_*.json"))
    except OSError:
        intents = []
    for path in intents:
        intent = read_json(path)
        if not intent:
            continue
        authored_intent = (
            intent.get("provenance") == "exact_model_scheduled_introspection"
            and valid_trace(intent)
            and valid_response_sha256(intent.get("response_sha256"))
            and valid_response_sha256(intent.get("candidate_digest"))
        )
        event = base_event(
            intent,
            "self_change",
            unix_timestamp_ms(intent.get("recorded_at_unix_ms")),
            "self-change/outbox/intent_*.json",
        )
        event.update(
            {
                "lifecycle_kind": "intent",
                "status": "intent_only",
                "candidate_id": intent.get("candidate_id"),
                "candidate_digest": intent.get("candidate_digest"),
                "response_sha256": intent.get("response_sha256"),
                "provenance": intent.get("provenance", "legacy_unattributed"),
                "authority": intent.get("authority"),
                "artifact_path": f"self-change/outbox/{path.name}",
                "authored": authored_intent,
                "fallback": False,
                "authorship_class": (
                    SCHEDULED_INTROSPECTION_PROVENANCE
                    if authored_intent
                    else "legacy_self_change_intent_non_authored"
                ),
            }
        )
        events.append(event)

    for summary in read_patch_export_summaries(workspace):
        event = base_event(
            {},
            "self_change",
            unix_timestamp_ms(summary.get("recorded_at")),
            f"self-change/patch-outbox/{summary['summary_path']}",
        )
        event.update(
            {
                "lifecycle_kind": "patch_export",
                "status": summary.get("terminal_status"),
                "candidate_id": summary.get("candidate_id"),
                "candidate_digest": summary.get("candidate_sha256"),
                "source_id": summary.get("source_id"),
                "generation_id": summary.get("base_generation"),
                "file_count": summary.get("file_count"),
                "added_lines": summary.get("added_lines"),
                "removed_lines": summary.get("removed_lines"),
                "changed_lines": summary.get("changed_lines"),
                "touched_paths": summary.get("touched_paths"),
                "source_bodies_retained": False,
                "record_sha256": summary.get("full_export_sha256"),
                "authored": False,
                "fallback": False,
                "authorship_class": "immutable_patch_export_summary_non_authored",
                "integrity": "core_sha256_verified_signature_present_not_reverified",
                "authority": summary.get("authority"),
            }
        )
        event["trace_attribution"] = "immutable_supervisor_event"
        events.append(event)

    operator = read_self_change_operator_status(
        operator_status_path,
        test_only_allow_unprivileged_owner=(
            test_only_allow_unprivileged_operator_status
        ),
    )
    lifecycle = operator.get("lifecycle")
    projected = lifecycle.get("events") if isinstance(lifecycle, dict) else []
    for item in projected if isinstance(projected, list) else []:
        facets = list(item["facets"])
        lifecycle_kind = next(
            (
                name
                for name in (
                    "reflection", "candidate", "build", "test", "invariant",
                    "shadow", "activation", "restart", "probation", "rollback",
                    "operator",
                )
                if name in facets
            ),
            "operator",
        )
        event = base_event(
            {},
            "self_change",
            unix_timestamp_ms(item.get("recorded_at")),
            "self-change/operator-status.json",
        )
        event.update(
            {
                "lifecycle_kind": lifecycle_kind,
                "lifecycle_facets": facets,
                "status": item.get("status"),
                "sequence": item.get("sequence"),
                "event_id": item.get("event_id"),
                "record_sha256": item.get("record_sha256"),
                "projection_core_sha256": operator.get("projection_core_sha256"),
                "projected_source_ledger": item.get("source_ledger"),
                "candidate_id": item.get("candidate_id"),
                "candidate_digest": item.get("candidate_sha256"),
                "build_id": item.get("build_id"),
                "generation_id": item.get("generation_id"),
                "from_generation": item.get("from_generation"),
                "tests_sha256": item.get("tests_sha256"),
                "bundle_sha256": item.get("bundle_sha256"),
                "manifest_sha256": item.get("manifest_sha256"),
                "invariant_candidate_replay_sha256": item.get(
                    "invariant_candidate_replay_sha256"
                ),
                "invariant_package_replay_sha256": item.get(
                    "invariant_package_replay_sha256"
                ),
                "shadow_evidence_sha256": item.get("shadow_evidence_sha256"),
                "shadow_status": item.get("shadow_status"),
                "package_replay_sha256_present": (
                    item.get("invariant_package_replay_sha256") is not None
                ),
                "shadow_gate_evidence": (
                    INDIRECT_SHADOW_GATE_EVIDENCE
                    if item.get("shadow_evidence_sha256") is not None
                    else None
                ),
                "trace_id": item.get("trace_id"),
                "session_id": item.get("session_id"),
                "turn_id": item.get("turn_id"),
                "response_sha256": item.get("response_sha256"),
                "terminal_declaration_sha256": item.get(
                    "terminal_declaration_sha256"
                ),
                "terminal_reason_sha256": item.get("terminal_reason_sha256"),
                "terminal_authority": item.get("terminal_authority"),
                "automatic_retry": item.get("automatic_retry"),
                "command_profile": item.get("command_profile"),
                "command_executable_sha256": item.get("command_executable_sha256"),
                "command_argv_sha256": item.get("command_argv_sha256"),
                "command_stdout_sha256": item.get("command_stdout_sha256"),
                "command_stderr_sha256": item.get("command_stderr_sha256"),
                "command_exit_code": item.get("command_exit_code"),
                "command_timed_out": item.get("command_timed_out"),
                "authored": False,
                "fallback": False,
                "authorship_class": "immutable_supervisor_metadata_non_authored",
                "provenance": item.get("provenance"),
                "authority": item.get("authority"),
                "integrity": (
                    "operator_projection_core_sha256_verified_root_filesystem_origin_"
                    "signed_ledger_record_hash_projected"
                ),
            }
        )
        event["trace_attribution"] = "immutable_supervisor_event"
        events.append(event)
    return events


def event_authorship_class(event: dict[str, Any]) -> str:
    explicit = event.get("authorship_class")
    if isinstance(explicit, str) and explicit:
        return explicit
    kind = str(event.get("kind") or "")
    if event.get("trace_attribution") == "operator_harness" or kind == "operator_inquiry":
        return "operator_harness_non_authored"
    if kind == "turn":
        return (
            "model_authored_autonomy_turn"
            if event.get("authored") is True
            else "fallback_or_transport_non_authored"
        )
    if kind == "action":
        if event.get("authored") is not True:
            return "fallback_or_executor_non_authored"
        declared = str(event.get("declared_next") or "").lstrip().upper()
        return (
            "voluntary_model_authored_journal"
            if declared.startswith("JOURNAL")
            else "voluntary_model_authored_action"
        )
    if kind == "peer" and event.get("authored") is True:
        return "voluntary_model_authored_action_result"
    if kind == "thread" and event.get("authored") is True:
        return "voluntary_model_authored_continuity"
    if kind in {
        "web_request",
        "web_result",
        "introspection_request",
        "introspection_result",
        "perception",
        "study",
        "spectral_rollup",
        "spectral_activity_link",
        "spectral_receipt",
        "tuning",
        "duplication_advisory",
    }:
        if event.get("origin") == "operator_harness":
            return "operator_harness_non_authored"
        return "machine_evidence_non_authored"
    if event.get("fallback") is True:
        return "fallback_or_transport_non_authored"
    return "operational_metadata_non_authored"


def transport_authorship_corrections(
    workspace: Path,
) -> dict[Any, dict[str, Any]]:
    """Index exact legacy transport corrections by immutable identifiers.

    Timestamp proximity is intentionally never used.  A transcript path is
    sufficient for the run that owns that path.  A response hash is reusable
    text identity rather than event identity, so downstream records require the
    exact turn/trace identity derived from that corrected run as well.
    """
    corrections: dict[Any, dict[str, Any]] = {}
    runs_by_transcript = {
        str(run.get("transcript_path")): run
        for run in read_json_lines(workspace / "autonomous/runs.jsonl")
        if isinstance(run.get("transcript_path"), str)
        and run.get("transcript_path")
    }
    for value in read_json_lines(
        workspace / "autonomous/authorship_corrections.jsonl"
    ):
        if value.get("reason") != TRANSPORT_AUTHORSHIP_CORRECTION_REASON:
            continue
        transcript_path = value.get("original_transcript_path")
        if isinstance(transcript_path, str) and transcript_path:
            corrections[f"transcript:{transcript_path}"] = value
        response_hash = value.get("response_sha256")
        corrected_run = runs_by_transcript.get(str(transcript_path or ""))
        if (
            not isinstance(response_hash, str)
            or not response_hash
            or corrected_run is None
            or corrected_run.get("response_sha256") != response_hash
        ):
            continue
        identity = exact_response_identity(corrected_run)
        if identity is not None:
            corrections[("response_identity", *identity)] = value
    return corrections


def transport_authorship_correction(
    value: dict[str, Any],
    corrections: dict[Any, dict[str, Any]],
) -> dict[str, Any] | None:
    transcript_path = value.get("transcript_path")
    if isinstance(transcript_path, str) and transcript_path:
        correction = corrections.get(f"transcript:{transcript_path}")
        if correction is not None:
            return correction
    response_hash = value.get("response_sha256")
    if not isinstance(response_hash, str) or not response_hash:
        return None
    identity = exact_response_identity(value)
    return (
        corrections.get(("response_identity", *identity))
        if identity is not None
        else None
    )


def run_authorship(
    run: dict[str, Any],
    corrections: dict[Any, dict[str, Any]],
) -> RunAuthorship:
    """Return the correction-aware status, authorship, and fallback flags."""
    status = str(run.get("status", "unknown"))
    correction = transport_authorship_correction(run, corrections)
    if status == "authored_completed" and correction is not None:
        status = "transport_recovery"
    raw_provenance = run.get("response_provenance")
    if raw_provenance is None:
        response_provenance = (
            NON_AUTHORED_STATUS_PROVENANCES[status]
            if run.get("schema") == "astrid_edge_autonomy_run_v4"
            and status in NON_AUTHORED_STATUS_PROVENANCES
            else "legacy_unspecified"
        )
    elif isinstance(raw_provenance, str) and raw_provenance in (
        MODEL_RESPONSE_PROVENANCES | {"executor_terminal_error"}
    ):
        response_provenance = str(raw_provenance)
    else:
        response_provenance = "invalid"
    local_safe_fallback_used = (
        response_provenance == "model_authored_with_local_safe_fallback"
    )
    local_format_repair_used = (
        response_provenance == "model_authored_with_local_format_repair"
    )
    provenance_can_be_authored = response_provenance in MODEL_RESPONSE_PROVENANCES | {
        "legacy_unspecified"
    }
    authored = (
        status == "authored_completed"
        and correction is None
        and provenance_can_be_authored
    )
    fallback = (
        correction is not None
        or status in NON_AUTHORED_STATUS_PROVENANCES
        or response_provenance
        in {
            "executor_terminal_error",
            "invalid",
            *NON_AUTHORED_STATUS_PROVENANCES.values(),
        }
    )
    return RunAuthorship(
        status,
        authored,
        fallback,
        correction,
        response_provenance,
        local_safe_fallback_used,
        local_format_repair_used,
    )


def spectral_metric(value: dict[str, Any], name: str) -> Any:
    metrics = value.get("metrics")
    if isinstance(metrics, dict) and name in metrics:
        return metrics.get(name)
    return value.get(name)


def flatten_signed_tuning(value: dict[str, Any]) -> dict[str, Any]:
    """Expose the signed tuning payload without discarding its envelope.

    Tuning receipts are deliberately persisted as signed envelopes.  Activity
    reporting operates on the signed payload and its bounded ``detail`` object;
    treating envelope fields as lifecycle fields would make every traced event
    appear timestamp-less and unattributed.
    """
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
    actual_hash = hashlib.sha256(
        json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode()
    ).hexdigest()
    flattened["payload_hash_valid"] = expected_hash == actual_hash
    flattened["signature_present_not_verified"] = bool(
        value.get("signature") and value.get("signing_public_key")
    )
    return flattened


def trace_fields(value: dict[str, Any]) -> dict[str, Any]:
    trace = value.get("trace")
    if not valid_trace(value):
        return {
            "trace_id": None,
            "span_id": None,
            "parent_span_id": None,
            "turn_id": normalized_uuid(value.get("turn_id")),
            "session_id": normalized_trace_label(
                value.get("session_id") or value.get("session_name")
            ),
            "chain_id": normalized_trace_label(value.get("chain_id")),
        }
    return {
        "trace_id": normalized_uuid(trace.get("trace_id")),
        "span_id": normalized_uuid(trace.get("span_id")),
        "parent_span_id": normalized_uuid(trace.get("parent_span_id")),
        "turn_id": normalized_uuid(trace.get("turn_id"))
        or normalized_uuid(value.get("turn_id")),
        "session_id": trace.get("session_id")
        or value.get("session_id")
        or value.get("session_name"),
        "chain_id": trace.get("chain_id") or value.get("chain_id"),
    }


def bounded_results(value: dict[str, Any]) -> list[dict[str, str]]:
    summary = value.get("result_summary")
    results = summary.get("results") if isinstance(summary, dict) else None
    if not isinstance(results, list):
        return []
    bounded: list[dict[str, str]] = []
    for result in results[:3]:
        if not isinstance(result, dict):
            continue
        bounded.append(
            {
                "title": str(result.get("title", ""))[:300],
                "url": str(result.get("url", ""))[:2_048],
            }
        )
    return bounded


def base_event(
    value: dict[str, Any],
    kind: str,
    timestamp_ms: int,
    source_ledger: str,
) -> dict[str, Any]:
    return {
        "timestamp_unix_ms": timestamp_ms,
        "kind": kind,
        "source_ledger": source_ledger,
        "trace_attribution": "first_class" if valid_trace(value) else "legacy_unattributed",
        **trace_fields(value),
    }


def collect_events(
    workspace: Path,
    now_ms: int,
    operator_status_path: Path = SELF_CHANGE_OPERATOR_STATUS_PATH,
    *,
    test_only_allow_unprivileged_operator_status: bool = False,
) -> list[dict[str, Any]]:
    runs = read_json_lines(workspace / "autonomous/runs.jsonl")
    transport_corrections = transport_authorship_corrections(workspace)
    action_dispatches = read_json_lines(workspace / "actions/dispatches.jsonl")
    actions = read_json_lines(workspace / "actions/receipts.jsonl")
    interrupted_correction_records = read_json_lines(
        workspace / "actions/interrupted_corrections.jsonl"
    )
    interrupted_action_corrections = {
        key: item
        for item in interrupted_correction_records
        if item.get("corrected_status") == "revoked_interrupted_trace_non_authored"
        if (key := interrupted_correction_identity(item)) is not None
    }
    chains = read_json_lines(workspace / "autonomous/chains.jsonl")
    recoveries = read_json_lines(workspace / "autonomous/recoveries.jsonl")
    session_retirements = read_json_lines(
        workspace / "autonomous/session_retirements.jsonl"
    )
    threads = read_json_lines(workspace / "autonomous/thread_state.jsonl")
    web = read_json_lines(workspace / "web/receipts.jsonl")
    introspection = read_json_lines(workspace / "introspection/receipts.jsonl")
    scheduled_introspection = scheduled_introspection_receipts(workspace)
    perception = read_json_lines(workspace / "perception/observations.jsonl")
    studies = read_json_lines(workspace / "studies/receipts.jsonl")
    spectral_rollups = read_json_lines(workspace / "spectral/rollups.jsonl")
    spectral_receipts = read_json_lines(workspace / "spectral/receipts.jsonl")
    tuning_receipts = [
        flatten_signed_tuning(value)
        for value in read_json_lines(workspace / "tuning/receipts.jsonl")
    ]
    duplicate_notices = read_json_lines(workspace / "research/duplication_notices.jsonl")
    peer = read_json_lines(workspace / "peer/receipts.jsonl")
    self_change_events = self_change_ledger_events(
        workspace,
        operator_status_path,
        test_only_allow_unprivileged_operator_status=(
            test_only_allow_unprivileged_operator_status
        ),
    )

    exact_run_candidates: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for run in runs:
        response_hash = run.get("response_sha256")
        session = run.get("session_name")
        if (
            isinstance(response_hash, str)
            and isinstance(session, str)
            and valid_trace(run)
        ):
            for key in (
                (response_hash, session),
                (response_hash, str(uuid.uuid5(uuid.NAMESPACE_URL, session))),
            ):
                exact_run_candidates.setdefault(key, []).append(run)
    exact_runs = {
        key: candidates[0]
        for key, candidates in exact_run_candidates.items()
        if len(candidates) == 1
    }

    events: list[dict[str, Any]] = []
    for run in runs:
        authorship = run_authorship(run, transport_corrections)
        exact_provider_telemetry = exact_provider_request_telemetry(run)
        exact_header_latency = exact_request_header_latency(run)
        legacy_header_latency = legacy_unattributed_header_latency(run)
        invalid_claimed_provider_metrics = (
            run.get("request_header_latency_source")
            == REQUEST_HEADER_LATENCY_SOURCE_V1
            and exact_provider_telemetry is None
        )
        event = base_event(
            run,
            "turn",
            int(run.get("completed_at_unix_ms", run.get("started_at_unix_ms", 0)) or 0),
            "autonomous/runs.jsonl",
        )
        event.update(
            {
                "status": authorship.status,
                "authored": authorship.authored,
                "fallback": authorship.fallback,
                "response_provenance": authorship.response_provenance,
                "local_safe_fallback_used": authorship.local_safe_fallback_used,
                "local_format_repair_used": authorship.local_format_repair_used,
                "trigger": run.get("trigger"),
                "declared_next": run.get("declared_next"),
                "response_sha256": run.get("response_sha256"),
                "prompt_chars": run.get("prompt_chars"),
                "elapsed_ms": run.get("elapsed_ms"),
                "provider_prompt_tokens": run.get("provider_prompt_tokens"),
                "provider_completion_tokens": run.get("provider_completion_tokens"),
                "provider_request_id": exact_header_latency[1]
                if exact_header_latency
                else None,
                "provider_request_count": exact_provider_telemetry[0]
                if exact_provider_telemetry
                else None,
                "provider_successful_header_count": exact_provider_telemetry[1]
                if exact_provider_telemetry
                else None,
                "provider_requests": exact_provider_telemetry[2]
                if exact_provider_telemetry
                else None,
                "request_header_latency_ms": exact_header_latency[0]
                if exact_header_latency
                else None,
                "request_header_latency_source": REQUEST_HEADER_LATENCY_SOURCE_V1
                if exact_provider_telemetry
                else None,
                "request_header_latency_ms_legacy_unattributed": legacy_header_latency,
                "provider_metrics_invalid_claimed_exact": invalid_claimed_provider_metrics,
                "generation_latency_ms": run.get("generation_latency_ms"),
                "full_turn_latency_ms": run.get("full_turn_latency_ms", run.get("elapsed_ms")),
                "session_generation": run.get("session_generation"),
                "session_authored_turns_before": run.get("session_authored_turns_before"),
                "artifact_path": run.get("transcript_path"),
                "journal_path": run.get("journal_path"),
                "correction_reason": authorship.correction.get("reason")
                if authorship.correction
                else None,
                "correction_authority": authorship.correction.get("authority")
                if authorship.correction
                else None,
            }
        )
        events.append(event)

    try:
        state_root = workspace.parents[2]
        harness_runs = sorted(
            (state_root / "operator/inquiry-harness").glob("run_*/result.json")
        )
    except (IndexError, OSError):
        harness_runs = []
    for result_path in harness_runs:
        result = read_json(result_path)
        if not result:
            continue
        event = base_event(
            result,
            "operator_inquiry",
            int(result.get("completed_at_unix_ms", 0) or 0),
            "operator/inquiry-harness/result.json",
        )
        candidates = read_json(result_path.parent / "search_candidates.json")
        event.update(
            {
                "status": result.get("status"),
                "question": result.get("question"),
                "candidate_count": result.get("candidate_count"),
                "candidates": candidates.get("candidates", []),
                "authored": False,
                "fallback": False,
                "trace_attribution": "operator_harness",
                "authority": result.get("authority"),
            }
        )
        events.append(event)

    for action in actions:
        interrupted_correction = interrupted_action_corrections.get(
            exact_response_identity(action)
        )
        transport_correction = transport_authorship_correction(
            action, transport_corrections
        )
        correction = interrupted_correction or transport_correction
        event = base_event(
            action,
            "action",
            int(action.get("recorded_at_unix_ms", 0) or 0),
            "actions/receipts.jsonl",
        )
        if event["trace_id"] is None:
            key = (str(action.get("response_sha256", "")), str(action.get("session_id", "")))
            matched = exact_runs.get(key)
            if matched is not None:
                event.update(trace_fields(matched))
                event["trace_attribution"] = "exact_response_session_join"
        decision_source = str(action.get("decision_source", "unknown"))
        if interrupted_correction:
            action_status = interrupted_correction.get("corrected_status")
        elif transport_correction:
            action_status = "revoked_legacy_transport_non_authored"
        else:
            action_status = action.get("status")
        event.update(
            {
                "status": action_status,
                "authored": not correction
                and decision_source
                in {
                    "astrid_declared",
                    "local_format_repair_preserved_astrid_declaration",
                },
                "fallback": bool(correction)
                or decision_source == "local_safe_fallback",
                "declared_next": action.get("declared_next"),
                "decision_source": decision_source,
                "outcome": action.get("outcome"),
                "response_sha256": action.get("response_sha256"),
                "artifact_path": action.get("artifact_path"),
                "validation_reason": action.get("validation_reason"),
                "execution_error": action.get("execution_error"),
                "unexecuted_intention": action.get("unexecuted_intention"),
                "correction_authority": correction.get("authority")
                if correction
                else None,
                "correction_reason": correction.get("reason")
                if correction
                else None,
            }
        )
        events.append(event)

    for correction in interrupted_correction_records:
        identity = interrupted_correction_identity(correction)
        exact = identity is not None
        event = {
            "timestamp_unix_ms": int(
                correction.get("recorded_at_unix_ms", 0) or 0
            ),
            "kind": "action_correction",
            "source_ledger": "actions/interrupted_corrections.jsonl",
            "trace_attribution": (
                "exact_trace_response_join" if exact else "legacy_unattributed"
            ),
            "trace_id": normalized_uuid(correction.get("trace_id")) if exact else None,
            "span_id": None,
            "parent_span_id": None,
            "turn_id": normalized_uuid(correction.get("turn_id")) if exact else None,
            "session_id": None,
            "chain_id": None,
            "status": correction.get("corrected_status"),
            "authored": False,
            "fallback": True,
            "response_sha256": correction.get("response_sha256"),
            "authority": correction.get("authority"),
            "identity_kind": correction.get("identity_kind") if exact else None,
        }
        events.append(event)

    for dispatch in action_dispatches:
        phase = str(dispatch.get("phase") or "unknown")
        integrity_error = action_dispatch_integrity_error(dispatch)
        event = base_event(
            dispatch,
            "action_dispatch",
            int(dispatch.get("recorded_at_unix_ms", 0) or 0),
            "actions/dispatches.jsonl",
        )
        event.update(
            {
                "status": phase if integrity_error is None else "invalid",
                "phase": phase,
                "response_sha256": dispatch.get("response_sha256"),
                "authored": False,
                "fallback": False,
                "authority": dispatch.get("authority"),
                "integrity_error": integrity_error,
            }
        )
        if integrity_error is not None:
            event.update(
                {
                    "trace_attribution": "invalid_untrusted_record",
                    "trace_id": None,
                    "span_id": None,
                    "parent_span_id": None,
                    "turn_id": None,
                    "session_id": None,
                    "chain_id": None,
                }
            )
        events.append(event)

    for chain in chains:
        event = base_event(
            chain,
            "chain",
            int(chain.get("recorded_at_unix_ms", 0) or 0),
            "autonomous/chains.jsonl",
        )
        event.update(
            {
                "status": chain.get("executor_status"),
                "transition": chain.get("transition"),
                "step": chain.get("step"),
                "max_steps": chain.get("max_steps"),
                "declared_next": chain.get("declared_next"),
                "response_sha256": chain.get("response_sha256"),
                "decision_source": chain.get("decision_source"),
            }
        )
        events.append(event)

    for recovery in recoveries:
        event = base_event(
            recovery,
            "recovery",
            int(
                recovery.get(
                    "completed_at_unix_ms", recovery.get("started_at_unix_ms", 0)
                )
                or 0
            ),
            "autonomous/recoveries.jsonl",
        )
        event.update(
            {
                "status": recovery.get("status"),
                "reason": recovery.get("reason"),
                "authored": False,
                "fallback": True,
            }
        )
        events.append(event)

    for retirement in session_retirements:
        if (
            retirement.get("schema")
            != "astrid_edge_operator_session_retirement_v1"
            or retirement.get("phase") != "completed"
        ):
            continue
        events.append(
            {
                "timestamp_unix_ms": int(
                    retirement.get("recorded_at_unix_ms", 0) or 0
                ),
                "kind": "session_retirement",
                "source_ledger": "autonomous/session_retirements.jsonl",
                "trace_attribution": "operator_session_retirement",
                "trace_id": None,
                "span_id": None,
                "parent_span_id": None,
                "turn_id": None,
                "session_id": None,
                "chain_id": None,
                "status": "completed",
                "authored": False,
                "fallback": False,
                "transition_id": retirement.get("transition_id"),
                "prior_session_generation": retirement.get(
                    "prior_session_generation"
                ),
                "new_session_generation": retirement.get(
                    "new_session_generation"
                ),
                "reason": retirement.get("reason"),
                "authority": retirement.get("authority"),
            }
        )

    for thread in threads:
        correction = transport_authorship_correction(thread, transport_corrections)
        event = base_event(
            thread,
            "thread",
            int(thread.get("updated_at_unix_ms", 0) or 0),
            "autonomous/thread_state.jsonl",
        )
        event.update(
            {
                "status": thread.get("status"),
                "schema": thread.get("schema"),
                "thread_id": thread.get("thread_id"),
                "focus": thread.get("focus"),
                "question": thread.get("question"),
                "hypothesis": thread.get("hypothesis"),
                "hypotheses": thread.get("hypotheses", []),
                "methods": thread.get("methods", []),
                "study_ids": thread.get("study_ids", []),
                "syntheses": thread.get("syntheses", []),
                "provenance_hashes": thread.get("provenance_hashes", []),
                "last_action": thread.get("last_action"),
                "latest_note": thread.get("latest_note"),
                "authored_claims": thread.get("authored_claims", []),
                "findings": thread.get("findings", []),
                "open_questions": thread.get("open_questions", []),
                "conclusion": thread.get("conclusion"),
                "uncertainty": thread.get("uncertainty"),
                "evidence": thread.get("evidence", []),
                "evidence_records": thread.get("evidence_records", []),
                "event": thread.get("event"),
                "authored": correction is None,
                "fallback": correction is not None,
                "correction_reason": correction.get("reason")
                if correction
                else None,
                "correction_authority": correction.get("authority")
                if correction
                else None,
            }
        )
        events.append(event)

    for receipt in studies:
        event = base_event(
            receipt,
            "study",
            int(receipt.get("recorded_at_unix_ms", 0) or 0),
            "studies/receipts.jsonl",
        )
        event.update(
            {
                "status": receipt.get("status"),
                "phase": receipt.get("phase"),
                "study_id": receipt.get("study_id"),
                "primary_metric": receipt.get("primary_metric"),
                "secondary_metric": receipt.get("secondary_metric"),
                "sample_count": receipt.get("sample_count"),
                "artifact_path": receipt.get("artifact_path"),
                "artifact_sha256": receipt.get("artifact_sha256"),
                "origin": receipt.get("origin"),
                "authored": False,
                "fallback": False,
                "authority": receipt.get("authority"),
            }
        )
        if receipt.get("origin") == "operator_harness":
            event["trace_attribution"] = "operator_harness"
        events.append(event)

    for rollup in spectral_rollups:
        event = base_event(
            rollup,
            "spectral_rollup",
            int(rollup.get("recorded_at_unix_ms", 0) or 0),
            "spectral/rollups.jsonl",
        )
        substrate = rollup.get("substrate")
        substrate = substrate if isinstance(substrate, dict) else {}
        event.update(
            {
                "status": "machine_derived",
                "substrate_kind": substrate.get("kind")
                or rollup.get("substrate_kind"),
                "fill_metric": substrate.get("fill_metric")
                or rollup.get("fill_metric"),
                "fill_pct": spectral_metric(rollup, "fill_pct"),
                "spectral_entropy": spectral_metric(rollup, "spectral_entropy"),
                "lambda1_share": spectral_metric(rollup, "lambda1_share"),
                "tail_share": spectral_metric(rollup, "tail_share"),
                "density_gradient": spectral_metric(rollup, "density_gradient"),
                "mode_turnover": spectral_metric(rollup, "mode_turnover"),
                "mode_identity_state": rollup.get("mode_identity_state"),
                "activity_ref_count": rollup.get("activity_ref_count"),
                "activity_refs_truncated": rollup.get("activity_refs_truncated"),
                "response_sha256": rollup.get("response_sha256"),
                "authored": False,
                "fallback": False,
                "authority": rollup.get("authority"),
            }
        )
        if event["trace_id"] is None:
            event["trace_attribution"] = "continuous_machine_observation"
        events.append(event)
        activity_refs = rollup.get("activity_refs")
        if isinstance(activity_refs, list):
            for reference in activity_refs[:2]:
                if not isinstance(reference, dict):
                    continue
                linked = base_event(
                    reference,
                    "spectral_activity_link",
                    int(
                        reference.get("recorded_at_unix_ms")
                        or rollup.get("recorded_at_unix_ms", 0)
                        or 0
                    ),
                    "spectral/rollups.jsonl",
                )
                linked.update(
                    {
                        "status": "temporal_rollup_context_not_exact_or_causal",
                        "activity_kind": reference.get("kind")
                        or reference.get("event_kind"),
                        "response_sha256": reference.get("response_sha256"),
                        "parent_response_sha256": reference.get(
                            "parent_response_sha256"
                        ),
                        "spectral_rollup_sha256": rollup.get("record_sha256"),
                        "authored": False,
                        "fallback": False,
                        "authority": rollup.get("authority"),
                    }
                )
                events.append(linked)

    for receipt in spectral_receipts:
        event = base_event(
            receipt,
            "spectral_receipt",
            int(receipt.get("recorded_at_unix_ms", 0) or 0),
            "spectral/receipts.jsonl",
        )
        event.update(
            {
                "status": receipt.get("status"),
                "phase": receipt.get("phase") or receipt.get("kind"),
                "event_kind": receipt.get("event_kind"),
                "snapshot_generation_id": receipt.get("snapshot_generation_id"),
                "snapshot_sequence": receipt.get("snapshot_sequence"),
                "snapshot_recorded_at_unix_ms": receipt.get(
                    "snapshot_recorded_at_unix_ms"
                ),
                "snapshot_sha256": receipt.get("snapshot_sha256"),
                "spectral_metrics": receipt.get("metrics"),
                "attribution": receipt.get("attribution"),
                "response_sha256": receipt.get("response_sha256")
                or receipt.get("parent_response_sha256"),
                "artifact_path": receipt.get("artifact_path"),
                "authored": False,
                "fallback": False,
                "authority": receipt.get("authority"),
            }
        )
        events.append(event)

    for receipt in tuning_receipts:
        event = base_event(
            receipt,
            "tuning",
            int(receipt.get("recorded_at_unix_ms", 0) or 0),
            "tuning/receipts.jsonl",
        )
        event.update(
            {
                "status": receipt.get("status"),
                "phase": receipt.get("phase"),
                "tuning_id": receipt.get("tuning_id")
                or receipt.get("experiment_id"),
                "candidate_id": receipt.get("candidate_id"),
                "parameter": receipt.get("parameter"),
                "requested_value": receipt.get("requested_value")
                or receipt.get("value"),
                "rollback_reason": receipt.get("rollback_reason"),
                "response_sha256": receipt.get("response_sha256")
                or receipt.get("parent_response_sha256"),
                "authored": False,
                "fallback": False,
                "authority": receipt.get("authority"),
                "authority_turn_id": normalized_uuid(
                    receipt.get("authority_turn_id")
                ),
                "signed_envelope": receipt.get("signed_envelope", False),
                "payload_sha256": receipt.get("payload_sha256"),
                "payload_hash_valid": receipt.get("payload_hash_valid"),
                "signature_present_not_verified": receipt.get(
                    "signature_present_not_verified"
                ),
            }
        )
        events.append(event)

    for notice in duplicate_notices:
        event = base_event(
            notice,
            "duplication_advisory",
            int(notice.get("recorded_at_unix_ms", 0) or 0),
            "research/duplication_notices.jsonl",
        )
        event.update(
            {
                "status": notice.get("new_evidence_status"),
                "current_artifact": notice.get("current_artifact"),
                "similar_artifact": notice.get("similar_artifact"),
                "score_millis": notice.get("jaccard_score_millis"),
                "authored": False,
                "fallback": False,
                "authority": notice.get("authority"),
            }
        )
        events.append(event)

    for receipt in peer:
        event = base_event(
            receipt,
            "peer",
            int(receipt.get("recorded_at_unix_ms", 0) or 0),
            "peer/receipts.jsonl",
        )
        event.update(
            {
                "status": receipt.get("phase"),
                "packet_id": receipt.get("packet_id"),
                "source_instance": receipt.get("source_instance"),
                "artifact_kind": receipt.get("artifact_kind"),
                "artifact_path": receipt.get("artifact_path"),
                "authored": receipt.get("phase") == "shared",
                "fallback": False,
                "authority": receipt.get("authority"),
            }
        )
        events.append(event)

    v2_completed: set[str] = set()
    for receipt in web:
        call_id = str(receipt.get("call_id", ""))
        phase = receipt.get("phase")
        if phase == "completed":
            v2_completed.add(call_id)

    for receipt in web:
        phase = receipt.get("phase")
        legacy = phase not in {"requested", "completed"}
        kind = "web_result" if legacy or phase == "completed" else "web_request"
        timestamp = int(
            (
                receipt.get("completed_at_unix_ms")
                if kind == "web_result"
                else receipt.get("requested_at_unix_ms")
            )
            or receipt.get("recorded_at_unix_ms", 0)
            or 0
        )
        event = base_event(receipt, kind, timestamp, "web/receipts.jsonl")
        if legacy:
            event["trace_attribution"] = "legacy_unattributed"
        call_id = str(receipt.get("call_id", ""))
        if kind == "web_request" and call_id not in v2_completed:
            requested_at = int(receipt.get("requested_at_unix_ms", timestamp) or timestamp)
            status = (
                "stale"
                if now_ms - requested_at >= STALE_WEB_CALL_MS
                else "pending"
            )
        else:
            status = receipt.get("status", "unknown")
        arguments = receipt.get("arguments")
        arguments = arguments if isinstance(arguments, dict) else {}
        summary = receipt.get("result_summary")
        summary = summary if isinstance(summary, dict) else {}
        event.update(
            {
                "call_id": call_id,
                "status": status,
                "tool_name": receipt.get("tool_name"),
                "origin": receipt.get("origin")
                or ("legacy_unattributed" if legacy else "unknown"),
                "query": arguments.get("query"),
                "url": arguments.get("url") or summary.get("url"),
                "latency_ms": receipt.get("latency_ms"),
                "result_count": summary.get("result_count"),
                "http_status": summary.get("status"),
                "results": bounded_results(receipt),
                "parent_response_sha256": receipt.get("parent_response_sha256"),
                "authored": False,
                "fallback": False,
            }
        )
        events.append(event)

    completed_introspection_ids = {
        str(receipt.get("call_id", ""))
        for receipt in introspection
        if receipt.get("phase") == "completed"
    }
    for receipt in introspection:
        phase = receipt.get("phase")
        kind = (
            "introspection_result"
            if phase == "completed"
            else "introspection_request"
        )
        timestamp = int(
            (
                receipt.get("completed_at_unix_ms")
                if phase == "completed"
                else receipt.get("requested_at_unix_ms")
            )
            or receipt.get("recorded_at_unix_ms", 0)
            or 0
        )
        event = base_event(
            receipt, kind, timestamp, "introspection/receipts.jsonl"
        )
        call_id = str(receipt.get("call_id", ""))
        status = receipt.get("status", "unknown")
        if kind == "introspection_request" and call_id not in completed_introspection_ids:
            requested_at = int(receipt.get("requested_at_unix_ms", timestamp) or timestamp)
            status = (
                "stale"
                if now_ms - requested_at >= STALE_INTROSPECTION_CALL_MS
                else "pending"
            )
        arguments = (
            receipt.get("arguments")
            if isinstance(receipt.get("arguments"), dict)
            else {}
        )
        summary = (
            receipt.get("result_summary")
            if isinstance(receipt.get("result_summary"), dict)
            else {}
        )
        matched_artifacts = []
        for match in (summary.get("matches") or [])[:8]:
            if isinstance(match, dict):
                matched_artifacts.append(
                    {
                        "kind": str(match.get("kind", ""))[:40],
                        "basename": str(match.get("basename", ""))[:128],
                    }
                )
        event.update(
            {
                "call_id": call_id,
                "status": status,
                "tool_name": receipt.get("tool_name"),
                "origin": receipt.get("origin"),
                "query": arguments.get("query"),
                "latency_ms": receipt.get("latency_ms"),
                "result_count": summary.get("match_count"),
                "matched_artifacts": matched_artifacts,
                "parent_response_sha256": receipt.get(
                    "parent_response_sha256"
                ),
                "authored": False,
                "fallback": False,
            }
        )
        events.append(event)

    scheduled_admission = read_json(
        workspace / "runtime/scheduled-introspection/admission/state.json"
    )
    for receipt, source_ledgers, occurrences in scheduled_introspection:
        if receipt.get("schema") not in {
            SCHEDULED_INTROSPECTION_RECEIPT_SCHEMA,
            None,
        }:
            continue
        status = str(receipt.get("status") or "unknown")
        provenance = str(receipt.get("provenance") or "legacy_unattributed")
        authored = (
            status == "authored_completed"
            and provenance == SCHEDULED_INTROSPECTION_PROVENANCE
            and (
                receipt.get("continuity_projection_written") is True
                or receipt.get("continuity_admitted") is True
            )
            and valid_response_sha256(receipt.get("response_sha256"))
            and valid_trace(receipt)
        )
        actually_admitted = (
            authored
            and scheduled_admission.get("schema")
            == SCHEDULED_INTROSPECTION_ADMISSION_SCHEMA
            and scheduled_admission.get("continuity_admitted") is True
            and scheduled_admission.get("provenance")
            == SCHEDULED_INTROSPECTION_PROVENANCE
            and scheduled_admission.get("authority")
            == "runtime_verified_projection_observational_only"
            and scheduled_admission.get("last_response_sha256")
            == receipt.get("response_sha256")
            and scheduled_admission.get("last_trace_id")
            == receipt.get("trace", {}).get("trace_id")
        )
        event = base_event(
            receipt,
            "scheduled_introspection",
            int(receipt.get("completed_at_unix_ms", 0) or 0),
            source_ledgers[0],
        )
        event.update(
            {
                "status": status,
                "provenance": provenance,
                "authored": authored,
                "fallback": status
                in {"transport_recovery", "failed", "interrupted"}
                or provenance
                in {
                    "local_safe_fallback",
                    "local_format_repair",
                    "non_authored_transport_or_executor_failure",
                },
                "authorship_class": (
                    SCHEDULED_INTROSPECTION_PROVENANCE
                    if authored
                    else "scheduled_introspection_non_authored_excluded"
                ),
                "prompt_chars": receipt.get("prompt_chars"),
                "response_sha256": receipt.get("response_sha256"),
                "reflection_path": receipt.get("reflection_path"),
                "continuity_projected": receipt.get(
                    "continuity_projection_written"
                )
                is True
                or receipt.get("continuity_admitted") is True,
                "continuity_admitted": actually_admitted,
                "source_ledgers": list(source_ledgers),
                "exact_duplicate_count": occurrences - 1,
                "introspection_tool": receipt.get("introspection_tool"),
                "introspection_result_sha256": receipt.get(
                    "introspection_result_sha256"
                ),
                "candidate_id": receipt.get("candidate_id"),
                "candidate_digest": receipt.get("candidate_digest"),
                "next_due_at_unix_ms": receipt.get("next_due_at_unix_ms"),
                "authority": receipt.get("authority"),
            }
        )
        events.append(event)

    for observation in perception:
        event = base_event(
            observation,
            "perception",
            int(observation.get("recorded_at_unix_ms", 0) or 0),
            "perception/observations.jsonl",
        )
        event.update(
            {
                "status": "machine_observed",
                "summary": observation.get("summary"),
                "trigger_classes": observation.get("trigger_classes") or [],
                "causal_class": observation.get(
                    "causal_class", "legacy_unclassified"
                ),
                "record_sha256": observation.get("record_sha256"),
                "authored": False,
                "fallback": False,
                "authority": observation.get("authority"),
            }
        )
        event["trace_attribution"] = "machine_observation_not_ipc_trace"
        events.append(event)

    events.extend(self_change_events)

    for event in events:
        event["authorship_class"] = event_authorship_class(event)

    return sorted(
        (event for event in events if event["timestamp_unix_ms"] > 0),
        key=lambda event: (
            int(event["timestamp_unix_ms"]),
            str(event.get("trace_id") or ""),
            str(event.get("span_id") or ""),
            str(event["kind"]),
        ),
    )


def selected(
    events: Iterable[dict[str, Any]],
    args: argparse.Namespace,
    cutoff_ms: int,
    end_ms: int,
) -> list[dict[str, Any]]:
    kinds = set(args.kind or [])
    values = [
        event
        for event in events
        if int(event["timestamp_unix_ms"]) >= cutoff_ms
        and int(event["timestamp_unix_ms"]) <= end_ms
        and (not args.trace_id or event.get("trace_id") == args.trace_id)
        and (not args.session_id or event.get("session_id") == args.session_id)
        and (not args.chain_id or event.get("chain_id") == args.chain_id)
        and (not kinds or event.get("kind") in kinds)
    ]
    return values[-args.limit :] if args.limit else values


def iso_time(timestamp_ms: int) -> str:
    return dt.datetime.fromtimestamp(
        timestamp_ms / 1_000, tz=dt.timezone.utc
    ).isoformat(timespec="milliseconds").replace("+00:00", "Z")


def parse_time(value: str) -> int:
    text = value.strip()
    try:
        number = float(text)
    except ValueError:
        number = -1
    if number >= 0:
        return int(number if number >= 10_000_000_000 else number * 1_000)
    normalized = text[:-1] + "+00:00" if text.endswith("Z") else text
    parsed = dt.datetime.fromisoformat(normalized)
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=dt.timezone.utc)
    return int(parsed.timestamp() * 1_000)


def terminal_safe_text(value: Any) -> str:
    """Neutralize terminal controls without mutating structured event data."""

    return "".join(
        " "
        if unicodedata.category(character) in {"Cc", "Cf", "Cs", "Zl", "Zp"}
        else character
        for character in str(value)
    )


def candidate_presentation() -> int:
    """Render only broker-supplied activity summaries as untrusted JSON."""

    value = argparse.ArgumentParser(add_help=False)
    value.add_argument("--candidate-presentation", action="store_true")
    value.add_argument("--input-stdin", action="store_true")
    value.add_argument("--window-minutes", type=int, required=True)
    value.add_argument("--limit", type=int, required=True)
    value.add_argument("--format", choices=("json",), required=True)
    args = value.parse_args()
    if not args.candidate_presentation or not args.input_stdin:
        value.error("the active-generation presentation requires broker stdin")
    raw = sys.stdin.buffer.read(CANDIDATE_PRESENTATION_INPUT_MAX_BYTES + 1)
    if len(raw) > CANDIDATE_PRESENTATION_INPUT_MAX_BYTES:
        value.error("broker projection exceeds its bound")
    try:
        projection = json.loads(raw)
    except (UnicodeError, json.JSONDecodeError) as error:
        value.error(f"broker projection is invalid: {error}")
    if (
        not isinstance(projection, dict)
        or projection.get("schema") != CANDIDATE_PRESENTATION_INPUT_SCHEMA
        or not isinstance(projection.get("recent_activity"), list)
    ):
        value.error("broker projection has the wrong schema")
    lines = []
    for event in projection["recent_activity"][:64]:
        if not isinstance(event, dict):
            continue
        kind = " ".join(terminal_safe_text(event.get("kind", "")).split())
        status = " ".join(terminal_safe_text(event.get("status", "")).split())
        summary = " ".join(terminal_safe_text(event.get("summary", "")).split())
        if kind and status and summary:
            lines.append(f"{kind} [{status}] {summary}"[:240])
    sections = [
        {"heading": f"Sanitized activity {index + 1}", "lines": lines[index:index + 16]}
        for index in range(0, len(lines), 16)
    ][:12]
    result = {
        "schema": CANDIDATE_PRESENTATION_CONTENT_SCHEMA,
        "view": "activity",
        "title": "Active-generation activity view",
        "summary": (
            f"Candidate report arranged {len(lines)} sanitized immutable-report events; "
            "this is untrusted presentation, not causal or authorship evidence."
        ),
        "sections": sections,
    }
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


def short(value: Any, maximum: int = 120) -> str:
    if value in (None, ""):
        return "-"
    text = " ".join(terminal_safe_text(value).split())
    return text if len(text) <= maximum else f"{text[: maximum - 1]}…"


def text_lines(events: list[dict[str, Any]]) -> list[str]:
    spans = {
        str(event["span_id"]): event
        for event in events
        if event.get("span_id") is not None
    }

    def depth(event: dict[str, Any]) -> int:
        value = event.get("parent_span_id")
        seen: set[str] = set()
        result = 0
        while value and str(value) in spans and str(value) not in seen and result < 8:
            seen.add(str(value))
            result += 1
            value = spans[str(value)].get("parent_span_id")
        if result == 0 and event.get("parent_span_id"):
            return {
                "action": 1,
                "action_correction": 1,
                "action_dispatch": 1,
                "chain": 2,
                "web_request": 2,
                "web_result": 3,
                "recovery": 1,
                "session_retirement": 1,
                "thread": 1,
                "introspection_request": 2,
                "introspection_result": 3,
                "scheduled_introspection": 1,
                "self_change": 1,
                "perception": 1,
                "study": 1,
                "duplication_advisory": 1,
                "peer": 1,
                "spectral_rollup": 1,
                "spectral_receipt": 2,
                "tuning": 2,
            }.get(str(event.get("kind")), 1)
        return result

    lines: list[str] = []
    for event in events:
        prefix = "  " * depth(event)
        trace = str(
            event.get("trace_id")
            or (
                "machine"
                if event.get("kind") == "perception"
                else "operator"
                if event.get("trace_attribution")
                in {"operator_harness", "operator_session_retirement"}
                else "legacy"
            )
        )[:8]
        common = (
            f"{iso_time(int(event['timestamp_unix_ms']))} "
            f"[{trace}] {event['kind'].upper()} "
            f"turn={str(event.get('turn_id') or '-')[:8]}"
        )
        kind = event["kind"]
        if kind == "turn":
            detail = (
                f"status={event.get('status')} authored={str(event.get('authored')).lower()} "
                f"provenance={event.get('response_provenance')} "
                f"local_safe_fallback={str(event.get('local_safe_fallback_used')).lower()} "
                f"local_format_repair={str(event.get('local_format_repair_used')).lower()} "
                f"session={short(event.get('session_id'), 48)} "
                f"NEXT={short(event.get('declared_next'))}"
            )
        elif kind == "action":
            detail = (
                f"status={event.get('status')} source={event.get('decision_source')} "
                f"authored={str(event.get('authored')).lower()} "
                f"NEXT={short(event.get('declared_next'))} "
                f"artifact={short(event.get('artifact_path'), 90)}"
            )
            if event.get("execution_error"):
                detail += f" error={short(event.get('execution_error'), 100)}"
        elif kind == "action_dispatch":
            detail = (
                f"phase={event.get('phase')} status={event.get('status')} authored=false "
                f"response={short(event.get('response_sha256'), 16)} "
                "authority=executor-idempotency"
            )
            if event.get("integrity_error"):
                detail += f" integrity={event.get('integrity_error')}"
        elif kind == "action_correction":
            detail = (
                f"status={event.get('status')} authored=false "
                f"response={short(event.get('response_sha256'), 16)} "
                f"identity={event.get('identity_kind') or 'legacy-unattributed'}"
            )
        elif kind == "session_retirement":
            detail = (
                "authored=false counters-preserved=true "
                f"generation={event.get('prior_session_generation')}"
                f"->{event.get('new_session_generation')} "
                f"reason={short(event.get('reason'), 100)} "
                f"authority={short(event.get('authority'), 100)}"
            )
        elif kind == "chain":
            detail = (
                f"id={short(event.get('chain_id'), 48)} "
                f"step={event.get('step')}/{event.get('max_steps')} "
                f"transition={event.get('transition')}"
            )
        elif kind in {"web_request", "web_result"}:
            subject = event.get("query") or event.get("url")
            detail = (
                f"call={short(event.get('call_id'), 44)} tool={event.get('tool_name')} "
                f"status={event.get('status')} origin={event.get('origin')} "
                f"subject={short(subject)}"
            )
            results = event.get("results") or []
            if results:
                detail += " results=" + " | ".join(
                    f"{short(result.get('title'), 42)} ({short(result.get('url'), 62)})"
                    for result in results
                )
        elif kind in {"introspection_request", "introspection_result"}:
            detail = (
                f"call={short(event.get('call_id'), 44)} "
                f"status={event.get('status')} origin={event.get('origin')} "
                f"query={short(event.get('query'))} "
                f"matches={event.get('result_count', '-')}"
            )
        elif kind == "scheduled_introspection":
            detail = (
                f"status={event.get('status')} authored={str(event.get('authored')).lower()} "
                f"provenance={event.get('provenance')} "
                f"continuity={str(event.get('continuity_admitted')).lower()} "
                f"reflection={short(event.get('reflection_path'), 100)} "
                f"candidate={short(event.get('candidate_id'), 54)} "
                f"ledgers={short(','.join(event.get('source_ledgers') or []), 110)} "
                f"duplicates={event.get('exact_duplicate_count', 0)}"
            )
        elif kind == "self_change":
            detail = (
                f"lifecycle={event.get('lifecycle_kind')} status={event.get('status')} "
                f"facets={short(','.join(event.get('lifecycle_facets') or []), 90)} "
                f"candidate={short(event.get('candidate_id'), 54)} "
                f"build={short(event.get('build_id'), 54)} "
                f"generation={short(event.get('generation_id'), 54)} "
                f"authority={short(event.get('authority') or event.get('integrity'), 90)}"
            )
            if event.get("shadow_gate_evidence"):
                detail += (
                    " shadow_gate="
                    f"{short(event.get('shadow_gate_evidence'), 90)}"
                )
            if event.get("terminal_reason_sha256"):
                detail += (
                    " terminal_reason_sha256="
                    f"{short(event.get('terminal_reason_sha256'), 20)}"
                )
            if event.get("lifecycle_kind") == "patch_export":
                detail += (
                    f" files={event.get('file_count')} changed_lines={event.get('changed_lines')}"
                    f" paths={short(','.join(event.get('touched_paths') or []), 120)}"
                )
        elif kind == "perception":
            detail = (
                "machine-observed authored=false "
                f"triggers={short(','.join(event.get('trigger_classes') or []), 90)} "
                f"causal={event.get('causal_class')} "
                f"summary={short(event.get('summary'))}"
            )
        elif kind == "thread":
            detail = (
                f"id={short(event.get('thread_id'), 48)} status={event.get('status')} "
                f"event={event.get('event')} question={short(event.get('question'))} "
                f"focus={short(event.get('focus'))} "
                f"epistemic={'v6_spectral_typed' if event.get('schema') == 'astrid_edge_thread_state_v6' else 'v5_retained_typed' if event.get('schema') == 'astrid_edge_thread_state_v5' else 'v4_inquiry' if event.get('schema') == 'astrid_edge_thread_state_v4' else 'v3_typed' if event.get('schema') == 'astrid_edge_thread_state_v3' else 'legacy_unclassified'} "
                f"claims={len(event.get('authored_claims') or [])} "
                f"findings={len(event.get('findings') or [])} "
                f"open={len(event.get('open_questions') or [])} "
                f"conclusion={short(event.get('conclusion'))} "
                f"evidence={short(' | '.join(event.get('evidence') or []))}"
            )
        elif kind == "study":
            detail = (
                f"id={short(event.get('study_id'), 54)} phase={event.get('phase')} "
                f"status={event.get('status')} metrics={event.get('primary_metric')}+{event.get('secondary_metric') or '-'} "
                f"samples={event.get('sample_count')} origin={event.get('origin') or '-'} "
                f"artifact={short(event.get('artifact_path'), 90)} authored=false"
            )
        elif kind == "spectral_rollup":
            detail = (
                f"machine-derived authored=false substrate={event.get('substrate_kind')} "
                f"fill_metric={event.get('fill_metric')} fill={event.get('fill_pct')} "
                f"entropy={event.get('spectral_entropy')} turnover={event.get('mode_turnover')} "
                f"identity={event.get('mode_identity_state')}"
            )
        elif kind == "spectral_receipt":
            detail = (
                f"phase={event.get('phase')} status={event.get('status')} "
                f"event={event.get('event_kind')} artifact={short(event.get('artifact_path'), 90)} "
                "authored=false non-causal=true"
            )
        elif kind == "tuning":
            detail = (
                f"id={short(event.get('tuning_id'), 54)} candidate={short(event.get('candidate_id'), 54)} "
                f"phase={event.get('phase')} status={event.get('status')} "
                f"parameter={event.get('parameter')} value={event.get('requested_value')} "
                f"rollback={event.get('rollback_reason')} authored=false "
                f"authority_turn={short(event.get('authority_turn_id'), 36)} "
                f"payload_hash_valid={event.get('payload_hash_valid')} "
                f"signature_present={event.get('signature_present_not_verified')}"
            )
        elif kind == "duplication_advisory":
            detail = (
                f"current={event.get('current_artifact')} similar={event.get('similar_artifact')} "
                f"score={event.get('score_millis')} advisory-only=true"
            )
        elif kind == "peer":
            detail = (
                f"packet={short(event.get('packet_id'), 54)} phase={event.get('status')} "
                f"source={short(event.get('source_instance'), 40)} kind={event.get('artifact_kind')}"
            )
        elif kind == "operator_inquiry":
            detail = (
                f"status={event.get('status')} candidates={event.get('candidate_count')} "
                f"question={short(event.get('question'))} authority=operator-not-Astrid"
            )
        else:
            detail = f"status={event.get('status')} reason={short(event.get('reason'))}"
        if event.get("correction_reason"):
            detail += f" correction={short(event.get('correction_reason'), 80)}"
        attribution = event.get("trace_attribution")
        if attribution != "first_class":
            detail += f" attribution={attribution}"
        detail += f" class={event.get('authorship_class')}"
        lines.append(f"{prefix}{common} {detail}")
    return [terminal_safe_text(line) for line in lines]


def response_provenance_counts(events: Iterable[dict[str, Any]]) -> dict[str, int]:
    """Return fixed-key counts for the turn provenance labels in this output."""
    counts = {key: 0 for key in RESPONSE_PROVENANCE_COUNTER_KEYS}
    for event in events:
        if event.get("kind") != "turn":
            continue
        provenance = event.get("response_provenance")
        key = (
            provenance
            if isinstance(provenance, str) and provenance in counts
            else "invalid"
        )
        counts[key] += 1
    return counts


def response_provenance_summary_line(events: Iterable[dict[str, Any]]) -> str:
    counts = response_provenance_counts(events)
    values = " ".join(f"{key}={counts[key]}" for key in RESPONSE_PROVENANCE_COUNTER_KEYS)
    return f"RESPONSE_PROVENANCE_COUNTS {values}"


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser()
    value.add_argument("--workspace", type=Path)
    value.add_argument("--window-minutes", type=int, default=60)
    value.add_argument("--since", help="ISO-8601, Unix seconds, or Unix milliseconds")
    value.add_argument("--until", help="ISO-8601, Unix seconds, or Unix milliseconds")
    value.add_argument("--limit", type=int, default=100)
    value.add_argument("--trace-id")
    value.add_argument("--session-id")
    value.add_argument("--chain-id")
    value.add_argument(
        "--kind",
        action="append",
        choices=(
            "turn",
            "action",
            "action_correction",
            "action_dispatch",
            "chain",
            "web_request",
            "web_result",
            "recovery",
            "session_retirement",
            "thread",
            "introspection_request",
            "introspection_result",
            "scheduled_introspection",
            "self_change",
            "perception",
            "study",
            "operator_inquiry",
            "duplication_advisory",
            "peer",
            "spectral_rollup",
            "spectral_receipt",
            "tuning",
        ),
    )
    value.add_argument("--follow", action="store_true")
    value.add_argument("--format", choices=("text", "json", "jsonl"), default="text")
    return value


def render(args: argparse.Namespace, already_seen: set[str]) -> set[str]:
    workspace = args.workspace or Path.home() / ".astrid/home/default/edge"
    now_ms = time.time_ns() // 1_000_000
    end_ms = parse_time(args.until) if args.until else now_ms
    cutoff_ms = parse_time(args.since) if args.since else end_ms - args.window_minutes * 60_000
    if cutoff_ms >= end_ms:
        raise SystemExit("activity range must have --since before --until")
    events = selected(collect_events(workspace, now_ms), args, cutoff_ms, end_ms)
    fresh = []
    for event in events:
        key = json.dumps(event, sort_keys=True, separators=(",", ":"))
        if key not in already_seen:
            fresh.append(event)
            already_seen.add(key)
    if args.format == "json":
        provenance_counts = response_provenance_counts(fresh)
        print(
            json.dumps(
                {
                    "schema": SCHEMA,
                    "authorship_attribution_version": AUTHORSHIP_ATTRIBUTION_VERSION,
                    "generated_at_unix_ms": now_ms,
                    "workspace": str(workspace),
                    "response_provenance_counts": provenance_counts,
                    "events": fresh,
                },
                sort_keys=True,
            ),
            flush=True,
        )
    elif args.format == "jsonl":
        for event in fresh:
            print(json.dumps(event, sort_keys=True), flush=True)
    else:
        if fresh or not args.follow:
            print(response_provenance_summary_line(fresh), flush=True)
        for line in text_lines(fresh):
            print(line, flush=True)
    return already_seen


def main() -> int:
    if "--candidate-presentation" in sys.argv[1:]:
        return candidate_presentation()
    args = parser().parse_args()
    if args.window_minutes < 1:
        raise SystemExit("--window-minutes must be positive")
    if args.limit < 1:
        raise SystemExit("--limit must be positive")
    seen: set[str] = set()
    seen = render(args, seen)
    while args.follow:
        time.sleep(1)
        seen = render(args, seen)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
