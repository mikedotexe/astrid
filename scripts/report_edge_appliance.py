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
import json
import os
import re
import subprocess
import time
import urllib.request
import uuid
from pathlib import Path
from typing import Any


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


def emit(name: str, value: Any) -> None:
    if isinstance(value, bool):
        value = str(value).lower()
    print(f"{name}={value}")


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return {}
    return value if isinstance(value, dict) else {}


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


def valid_response_sha256(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def run_response_provenance(item: dict[str, Any]) -> tuple[str, bool, bool]:
    raw = item.get("response_provenance")
    if raw is None:
        provenance = "legacy_unspecified"
    elif raw in MODEL_RESPONSE_PROVENANCES | {"executor_terminal_error"}:
        provenance = str(raw)
    else:
        provenance = "invalid"
    return (
        provenance,
        provenance == "model_authored_with_local_safe_fallback",
        provenance == "model_authored_with_local_format_repair",
    )


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


def command(*arguments: str) -> str:
    try:
        return subprocess.run(
            arguments,
            check=False,
            capture_output=True,
            text=True,
            timeout=5,
        ).stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        return ""


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


def service_value(service: str, field: str) -> str:
    return (
        command(
            "systemctl",
            "--user",
            "show",
            service,
            f"--property={field}",
            "--value",
        )
        or "unknown"
    )


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


def effective_profile_values(home: Path) -> dict[str, str]:
    values = profile_values(home / ".config/astrid/edge-appliance.env")
    # The rollout authority file is loaded after the appliance profile by the
    # systemd drop-in. Mirror that precedence so reports show effective tuning
    # authority rather than the eventual profile capability.
    values.update(profile_values(home / ".config/astrid/edge-tuning-authority.env"))
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
    parser = argparse.ArgumentParser()
    parser.add_argument("--window-minutes", type=int, default=20)
    parser.add_argument("--workspace", type=Path)
    args = parser.parse_args()
    if args.window_minutes < 1:
        parser.error("--window-minutes must be positive")

    home = Path.home()
    workspace = args.workspace or home / ".astrid/home/default/edge"
    astrid_root = workspace.parents[2]
    state_path = workspace / "runtime/spectral_state.json"
    history_path = workspace / "runtime/fill_history.jsonl"
    if not state_path.is_file() or not history_path.is_file():
        parser.error(f"edge telemetry is unavailable under {workspace / 'runtime'}")

    profile = effective_profile_values(home)
    now_ms = time.time_ns() // 1_000_000
    cutoff_ms = now_ms - args.window_minutes * 60_000
    state = read_json(state_path)

    emit("report_version", 15)
    emit("instance_name", profile.get("ASTRID_EDGE_INSTANCE_NAME", "edge Astrid"))
    emit("hostname", os.uname().nodename)
    for label, service in (
        ("astrid", "astrid.service"),
        ("edge", "astrid-edge-runtime.service"),
    ):
        emit(f"{label}_service_state", service_value(service, "ActiveState"))
        emit(f"{label}_service_restarts", service_value(service, "NRestarts"))
    status_raw = command(str(astrid_root / "bin/astrid"), "--format", "json", "status")
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
        run_response_provenance(
            {"response_provenance": autonomy.get("last_response_provenance")}
        )
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
    recent_runs = read_json_lines(workspace / "autonomous/runs.jsonl")
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
    for provenance in (
        "model_authored",
        "model_authored_with_local_safe_fallback",
        "model_authored_with_local_format_repair",
        "executor_terminal_error",
        "legacy_unspecified",
        "invalid",
    ):
        emit(
            f"autonomy_window_response_provenance_{provenance}_turns",
            sum(value[0] == provenance for value in run_provenance),
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
        "request_header_latency_ms",
        "generation_latency_ms",
    ):
        values = [
            int(item[field])
            for item in window_runs
            if isinstance(item.get(field), (int, float))
        ]
        emit(f"autonomy_window_{field}_samples", len(values))
        emit(f"autonomy_window_{field}_p95", percentile(values))
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
    logs = command(
        "journalctl",
        "--user",
        "-u",
        "astrid.service",
        "--since",
        since,
        "--no-pager",
    )
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
