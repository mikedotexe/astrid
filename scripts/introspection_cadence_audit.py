#!/usr/bin/env python3
"""Read-only audit for Astrid's self-authored introspection cadence."""

from __future__ import annotations

import argparse
import collections
import datetime as dt
import hashlib
import json
import math
import re
import statistics
import sys
from pathlib import Path
from typing import Any


SCHEMA_VERSION = 1
EVENT_NAMES = (
    "configured",
    "disabled",
    "rejected",
    "due",
    "deferred",
    "attempted",
    "admitted",
    "failed",
)
EVENT_LEDGER = "introspection_cadence_events_v1.jsonl"
SIGNAL_NAME = "introspection_cadence_v1"
TIMESTAMP_RE = re.compile(r"_(\d{10,})\.txt$")
SOURCE_RE = re.compile(r"^Source:\s*(.+?)\s*$", re.MULTILINE)


def default_workspace() -> Path:
    return (
        Path(__file__).resolve().parents[1]
        / "capsules"
        / "spectral-bridge"
        / "workspace"
    )


def load_json(path: Path) -> tuple[Any | None, str | None]:
    try:
        return json.loads(path.read_text(encoding="utf-8")), None
    except FileNotFoundError:
        return None, None
    except (OSError, json.JSONDecodeError) as error:
        return None, str(error)


def load_events(workspace: Path) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    ledger_path = workspace / EVENT_LEDGER
    events: list[dict[str, Any]] = []
    malformed: list[dict[str, Any]] = []
    if ledger_path.exists():
        try:
            lines = ledger_path.read_text(encoding="utf-8").splitlines()
        except OSError as error:
            return [], {
                "source": str(ledger_path),
                "source_kind": "append_only_jsonl",
                "read_error": str(error),
                "malformed_records": [],
            }
        for line_number, line in enumerate(lines, start=1):
            if not line.strip():
                continue
            try:
                value = json.loads(line)
            except json.JSONDecodeError as error:
                malformed.append({"line": line_number, "error": str(error)})
                continue
            if not isinstance(value, dict):
                malformed.append(
                    {"line": line_number, "error": "event is not a JSON object"}
                )
                continue
            events.append(value)
        return events, {
            "source": str(ledger_path),
            "source_kind": "append_only_jsonl",
            "read_error": None,
            "malformed_records": malformed,
        }

    metrics_path = workspace / "condition_metrics.json"
    metrics, error = load_json(metrics_path)
    if isinstance(metrics, dict):
        signal = metrics.get("signals", {}).get(SIGNAL_NAME, {})
        recent = signal.get("recent_events", [])
        if isinstance(recent, list):
            events = [event for event in recent if isinstance(event, dict)]
        total_count = signal.get("total_count", 0)
    else:
        total_count = 0
    return events, {
        "source": str(metrics_path),
        "source_kind": "condition_metrics_recent_fallback",
        "read_error": error,
        "malformed_records": [],
        "reported_total_count": total_count,
    }


def canonical_timestamp(path: Path) -> float:
    match = TIMESTAMP_RE.search(path.name)
    if match:
        return float(match.group(1))
    return path.stat().st_mtime


def percentile(values: list[float], fraction: float) -> float | None:
    if not values:
        return None
    index = max(0, math.ceil(fraction * len(values)) - 1)
    return sorted(values)[index]


def canonical_report_audit(workspace: Path) -> dict[str, Any]:
    introspections = workspace / "introspections"
    files = sorted(
        (
            path
            for path in introspections.glob("introspection_*.txt")
            if path.is_file() and not path.name.startswith("thin_introspection_output_")
        ),
        key=canonical_timestamp,
    )
    hashes: dict[str, list[str]] = collections.defaultdict(list)
    targets: collections.Counter[str] = collections.Counter()
    timestamps: list[float] = []
    read_errors: list[dict[str, str]] = []
    for path in files:
        timestamps.append(canonical_timestamp(path))
        try:
            content = path.read_bytes()
        except OSError as error:
            read_errors.append({"path": str(path), "error": str(error)})
            continue
        hashes[hashlib.sha256(content).hexdigest()].append(path.name)
        text = content.decode("utf-8", errors="replace")
        match = SOURCE_RE.search(text)
        targets[match.group(1).strip() if match else "unreported"] += 1

    gaps = [right - left for left, right in zip(timestamps, timestamps[1:])]
    duplicate_groups = [
        {"sha256": digest, "files": names}
        for digest, names in sorted(hashes.items())
        if len(names) > 1
    ]
    return {
        "directory": str(introspections),
        "canonical_count": len(files),
        "latest": files[-1].name if files else None,
        "latest_timestamp": (
            dt.datetime.fromtimestamp(timestamps[-1], tz=dt.timezone.utc).isoformat()
            if timestamps
            else None
        ),
        "gaps_seconds": {
            "sample_count": len(gaps),
            "median": statistics.median(gaps) if gaps else None,
            "p90": percentile(gaps, 0.9),
            "maximum": max(gaps) if gaps else None,
        },
        "duplicate_hash_group_count": len(duplicate_groups),
        "duplicate_hashes": duplicate_groups,
        "target_diversity": dict(sorted(targets.items())),
        "read_errors": read_errors,
    }


def resolve_artifact(workspace: Path, raw_path: str) -> Path:
    path = Path(raw_path)
    if path.is_absolute():
        return path
    candidates = (
        Path.cwd() / path,
        workspace / path,
        workspace / "introspections" / path.name,
    )
    return next((candidate for candidate in candidates if candidate.exists()), candidates[-1])


def lifecycle_audit(
    workspace: Path, events: list[dict[str, Any]], source: dict[str, Any]
) -> dict[str, Any]:
    counts = collections.Counter(str(event.get("event")) for event in events)
    unknown = sorted(name for name in counts if name not in EVENT_NAMES)
    attempts: dict[str, list[int]] = collections.defaultdict(list)
    terminals: dict[str, list[tuple[int, str]]] = collections.defaultdict(list)
    violations: list[dict[str, Any]] = []
    admitted_artifacts: list[dict[str, Any]] = []

    for index, event in enumerate(events):
        name = event.get("event")
        attempt_id = event.get("attempt_id")
        source_name = event.get("source")
        if name in {"configured", "disabled"} and source_name != "astrid":
            violations.append(
                {
                    "index": index,
                    "kind": "mutation_without_astrid_provenance",
                    "event": name,
                    "source": source_name,
                }
            )
        if name == "configured":
            interval = event.get("every_exchanges")
            if event.get("enabled") is not True or not isinstance(interval, int) or not 4 <= interval <= 256:
                violations.append(
                    {"index": index, "kind": "malformed_configured_event"}
                )
        if name == "disabled" and event.get("enabled") is not False:
            violations.append({"index": index, "kind": "malformed_disabled_event"})
        if name == "attempted" and isinstance(attempt_id, str):
            attempts[attempt_id].append(index)
        if name in {"admitted", "failed"}:
            if not isinstance(attempt_id, str) or not attempt_id:
                violations.append(
                    {"index": index, "kind": "terminal_without_attempt_id", "event": name}
                )
                continue
            terminals[attempt_id].append((index, str(name)))
        if name == "admitted":
            raw_artifact = event.get("artifact_path")
            artifact = (
                resolve_artifact(workspace, raw_artifact)
                if isinstance(raw_artifact, str) and raw_artifact
                else None
            )
            exists = artifact.is_file() if artifact else False
            canonical = bool(
                artifact
                and artifact.name.startswith("introspection_")
                and not artifact.name.startswith("thin_introspection_output_")
                and artifact.suffix == ".txt"
            )
            admitted_artifacts.append(
                {
                    "attempt_id": attempt_id,
                    "path": str(artifact) if artifact else None,
                    "exists": exists,
                    "canonical_filename": canonical,
                }
            )
            if not exists or not canonical:
                violations.append(
                    {
                        "index": index,
                        "kind": "admitted_artifact_not_canonical_or_missing",
                        "attempt_id": attempt_id,
                        "artifact_path": raw_artifact,
                    }
                )

    for attempt_id, indices in attempts.items():
        if len(indices) > 1:
            violations.append(
                {
                    "kind": "duplicate_attempted_id",
                    "attempt_id": attempt_id,
                    "indices": indices,
                }
            )
    for attempt_id, outcomes in terminals.items():
        if attempt_id not in attempts:
            violations.append(
                {
                    "kind": "terminal_without_retained_attempt",
                    "attempt_id": attempt_id,
                    "outcomes": outcomes,
                }
            )
        if len(outcomes) > 1:
            violations.append(
                {
                    "kind": "multiple_terminal_events",
                    "attempt_id": attempt_id,
                    "outcomes": outcomes,
                }
            )
        attempted_index = attempts.get(attempt_id, [None])[0]
        if attempted_index is not None and any(
            terminal_index <= attempted_index for terminal_index, _ in outcomes
        ):
            violations.append(
                {
                    "kind": "terminal_precedes_attempt",
                    "attempt_id": attempt_id,
                    "attempted_index": attempted_index,
                    "outcomes": outcomes,
                }
            )

    unresolved = sorted(set(attempts) - set(terminals))
    failed = sorted(
        attempt_id
        for attempt_id, outcomes in terminals.items()
        if outcomes and outcomes[-1][1] == "failed"
    )
    return {
        **source,
        "retained_event_count": len(events),
        "counts": {name: counts.get(name, 0) for name in EVENT_NAMES},
        "unknown_event_names": unknown,
        "unresolved_attempt_ids": unresolved,
        "failed_attempt_ids": failed,
        "admitted_artifacts": admitted_artifacts,
        "integrity_violations": violations,
    }


def parse_time(value: Any) -> dt.datetime | None:
    if not isinstance(value, str):
        return None
    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=dt.timezone.utc)
    return parsed.astimezone(dt.timezone.utc)


def pilot_audit(
    cadence: dict[str, Any],
    events: list[dict[str, Any]],
    event_source: dict[str, Any],
    now: dt.datetime,
) -> dict[str, Any]:
    configured_indices = [
        index
        for index, event in enumerate(events)
        if event.get("event") == "configured" and event.get("source") == "astrid"
    ]
    first_index = configured_indices[0] if configured_indices else None
    window = events[first_index:] if first_index is not None else []
    starts = sum(event.get("event") == "attempted" for event in window)
    configured_at = parse_time(window[0].get("recorded_at")) if window else None
    wall_elapsed_hours = (
        max(0.0, (now - configured_at).total_seconds() / 3600.0)
        if configured_at
        else None
    )
    runtime_ms = cadence.get("pilot_runtime_ms", 0)
    runtime_ms = runtime_ms if isinstance(runtime_ms, int) and runtime_ms >= 0 else 0
    runtime_hours = runtime_ms / 3_600_000.0
    authored_off = any(event.get("event") == "disabled" for event in window[1:])
    hold_reason = cadence.get("pilot_hold_reason")
    stop_reasons: list[str] = []
    if starts >= 8:
        stop_reasons.append("eight_cadence_triggered_starts")
    if runtime_hours >= 72.0:
        stop_reasons.append("seventy_two_runtime_hours")
    if authored_off:
        stop_reasons.append("astrid_authored_off")
    if isinstance(hold_reason, str) and hold_reason and hold_reason not in stop_reasons:
        stop_reasons.append(hold_reason)
    receipt_exists = (
        first_index is not None
        and event_source.get("source_kind") == "append_only_jsonl"
    )
    return {
        "durable_configured_receipt_exists": receipt_exists,
        "configured_at": configured_at.isoformat() if configured_at else None,
        "cadence_triggered_starts": starts,
        "state_recorded_starts": cadence.get("pilot_cadence_starts", 0),
        "cumulative_runtime_hours": runtime_hours,
        "wall_elapsed_hours": wall_elapsed_hours,
        "boundaries": {"maximum_starts": 8, "maximum_runtime_hours": 72},
        "review_hold_reason": hold_reason,
        "stop_reasons": stop_reasons,
        "active": bool(cadence.get("enabled")) and receipt_exists and not stop_reasons,
    }


def audit(workspace: Path, now: dt.datetime | None = None) -> dict[str, Any]:
    now = now or dt.datetime.now(tz=dt.timezone.utc)
    state_path = workspace / "state.json"
    state, state_error = load_json(state_path)
    state = state if isinstance(state, dict) else {}
    cadence = state.get("introspection_cadence")
    legacy_default_applied = not isinstance(cadence, dict)
    cadence = cadence if isinstance(cadence, dict) else {}
    configuration = {
        "schema_version": cadence.get("schema_version", SCHEMA_VERSION),
        "enabled": bool(cadence.get("enabled", False)),
        "every_exchanges": cadence.get("every_exchanges", 0),
        "target": cadence.get("target"),
        "anchor_completed_exchange": cadence.get("anchor_completed_exchange"),
        "pending_since_exchange": cadence.get("pending_since_exchange"),
        "last_attempt_exchange": cadence.get("last_attempt_exchange"),
        "last_admitted_exchange": cadence.get("last_admitted_exchange"),
        "last_outcome": cadence.get("last_outcome"),
        "pilot_started_at_unix_ms": cadence.get("pilot_started_at_unix_ms"),
        "pilot_runtime_ms": cadence.get("pilot_runtime_ms", 0),
        "pilot_cadence_starts": cadence.get("pilot_cadence_starts", 0),
        "pilot_hold_reason": cadence.get("pilot_hold_reason"),
        "lifecycle_receipt_debt": cadence.get("lifecycle_receipt_debt"),
        "legacy_default_off_applied": legacy_default_applied,
    }
    interval = configuration["every_exchanges"]
    configuration_valid = (
        configuration["schema_version"] == SCHEMA_VERSION
        and (
            not configuration["enabled"]
            or isinstance(interval, int)
            and 4 <= interval <= 256
        )
    )
    events, event_source = load_events(workspace)
    lifecycle = lifecycle_audit(workspace, events, event_source)
    canonical = canonical_report_audit(workspace)
    pilot = pilot_audit(configuration, events, event_source, now)
    errors: list[dict[str, Any]] = []
    if state_error:
        errors.append({"kind": "state_read_error", "error": state_error})
    if not configuration_valid:
        errors.append({"kind": "invalid_enabled_configuration"})
    if configuration["lifecycle_receipt_debt"] is not None:
        errors.append(
            {
                "kind": "pending_lifecycle_receipt_debt",
                "debt": configuration["lifecycle_receipt_debt"],
            }
        )
    if lifecycle.get("read_error"):
        errors.append(
            {"kind": "lifecycle_read_error", "error": lifecycle["read_error"]}
        )
    errors.extend(lifecycle["integrity_violations"])
    errors.extend(
        {"kind": "malformed_lifecycle_record", **record}
        for record in lifecycle["malformed_records"]
    )
    latest_mutation = next(
        (
            event
            for event in reversed(events)
            if event.get("event") in {"configured", "disabled"}
        ),
        None,
    )
    if configuration["enabled"] and not pilot["durable_configured_receipt_exists"]:
        errors.append(
            {"kind": "enabled_without_durable_astrid_configured_receipt"}
        )
    if configuration["enabled"] and (
        not latest_mutation or latest_mutation.get("event") != "configured"
    ):
        errors.append({"kind": "enabled_state_without_latest_configured_transition"})
    if configuration["enabled"] and latest_mutation:
        if latest_mutation.get("every_exchanges") != configuration["every_exchanges"]:
            errors.append({"kind": "configured_interval_state_mismatch"})
    if not configuration["enabled"] and latest_mutation and latest_mutation.get("event") == "configured":
        errors.append({"kind": "disabled_state_without_latest_disabled_transition"})
    state_starts = configuration["pilot_cadence_starts"]
    if pilot["durable_configured_receipt_exists"] and state_starts != pilot["cadence_triggered_starts"]:
        errors.append(
            {
                "kind": "pilot_start_count_state_lifecycle_mismatch",
                "state": state_starts,
                "lifecycle": pilot["cadence_triggered_starts"],
            }
        )
    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at": now.isoformat(),
        "read_only": True,
        "workspace": str(workspace),
        "configuration_source": str(state_path),
        "configuration": configuration,
        "configuration_valid": configuration_valid,
        "lifecycle": lifecycle,
        "canonical_reports": canonical,
        "pilot": pilot,
        "pending_or_failed": {
            "pending_since_exchange": configuration["pending_since_exchange"],
            "lifecycle_receipt_debt": configuration["lifecycle_receipt_debt"],
            "unresolved_attempt_ids": lifecycle["unresolved_attempt_ids"],
            "failed_attempt_ids": lifecycle["failed_attempt_ids"],
        },
        "integrity_ok": not errors,
        "errors": errors,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=default_workspace())
    parser.add_argument("--json", action="store_true", help="emit JSON (the default)")
    parser.add_argument("--compact", action="store_true")
    parser.add_argument("--strict", action="store_true")
    args = parser.parse_args()
    report = audit(args.workspace.resolve())
    json.dump(report, sys.stdout, indent=None if args.compact else 2, sort_keys=True)
    sys.stdout.write("\n")
    return 0 if report["integrity_ok"] or not args.strict else 1


if __name__ == "__main__":
    raise SystemExit(main())
