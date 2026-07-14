#!/usr/bin/env python3
"""Trace Lab Spine V0 JSONL status, archive, bundle, and replay tools."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import re
import sys
import tempfile
import uuid
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_TRACE_ROOT = REPO_ROOT / "capsules" / "spectral-bridge" / "workspace" / "trace_lab"
STATE_WINDOW_SECS = 60
HASH_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
TOLERANCE = 1.0e-3
FULL_REPLAY_NOTE = (
    "Trace-envelope replay only: full reservoir replay still requires raw lane-input "
    "capture and checkpoint integration for W2 v1."
)


JsonObject = dict[str, Any]


def state_window_id(wall_time_unix_s: float) -> str:
    if not math.isfinite(wall_time_unix_s) or wall_time_unix_s < 0.0:
        wall_time_unix_s = 0.0
    seconds = int(math.floor(wall_time_unix_s))
    window_start = seconds - (seconds % STATE_WINDOW_SECS)
    return f"state_window_{window_start}"


def window_start_from_id(window_id: str) -> int | None:
    prefix = "state_window_"
    if not isinstance(window_id, str) or not window_id.startswith(prefix):
        return None
    try:
        return int(window_id[len(prefix) :])
    except ValueError:
        return None


def wall_time(value: Any) -> float | None:
    if isinstance(value, (int, float)):
        result = float(value)
        return result if math.isfinite(result) else None
    if isinstance(value, str):
        try:
            result = float(value)
        except ValueError:
            return None
        return result if math.isfinite(result) else None
    return None


def parse_time(value: str) -> float:
    try:
        result = float(value)
    except ValueError:
        normalized = value.strip()
        if normalized.endswith("Z"):
            normalized = f"{normalized[:-1]}+00:00"
        parsed = dt.datetime.fromisoformat(normalized)
        if parsed.tzinfo is None:
            parsed = parsed.replace(tzinfo=dt.timezone.utc)
        result = parsed.timestamp()
    if not math.isfinite(result):
        raise argparse.ArgumentTypeError(f"non-finite time: {value}")
    return result


def trace_root(args: argparse.Namespace) -> Path:
    return Path(args.root).expanduser().resolve()


def live_event_files(root: Path) -> list[Path]:
    events_dir = root / "live" / "events"
    if not events_dir.exists():
        return []
    return sorted(path for path in events_dir.glob("*.jsonl") if path.is_file())


def read_jsonl(path: Path) -> tuple[list[JsonObject], list[JsonObject]]:
    rows: list[JsonObject] = []
    errors: list[JsonObject] = []
    if not path.exists():
        return rows, errors
    with path.open("r", encoding="utf-8") as handle:
        for line_number, raw in enumerate(handle, start=1):
            stripped = raw.strip()
            if not stripped:
                continue
            try:
                value = json.loads(stripped)
            except json.JSONDecodeError as exc:
                errors.append(
                    {
                        "path": str(path),
                        "line": line_number,
                        "error": str(exc),
                    }
                )
                continue
            if isinstance(value, dict):
                rows.append(value)
            else:
                errors.append(
                    {
                        "path": str(path),
                        "line": line_number,
                        "error": "jsonl row is not an object",
                    }
                )
    return rows, errors


def read_live_events(root: Path) -> tuple[list[JsonObject], list[JsonObject]]:
    events: list[JsonObject] = []
    errors: list[JsonObject] = []
    for path in live_event_files(root):
        rows, row_errors = read_jsonl(path)
        for row in rows:
            row.setdefault("_source_file", str(path))
        events.extend(rows)
        errors.extend(row_errors)
    return events, errors


def read_exposure_records(root: Path) -> tuple[list[JsonObject], list[JsonObject]]:
    return read_jsonl(root / "exposure_records.jsonl")


def summarize_events(events: list[JsonObject]) -> JsonObject:
    by_lane: dict[str, int] = {}
    by_topic: dict[str, int] = {}
    by_source: dict[str, int] = {}
    missing_hash = 0
    bad_hash = 0
    missing_window = 0
    window_mismatch = 0
    walls: list[float] = []
    windows: set[str] = set()
    for event in events:
        lane = str(event.get("lane") or "(missing)")
        topic = str(event.get("topic") or "(missing)")
        source = str(event.get("source_identity") or "(missing)")
        by_lane[lane] = by_lane.get(lane, 0) + 1
        by_topic[topic] = by_topic.get(topic, 0) + 1
        by_source[source] = by_source.get(source, 0) + 1
        payload_hash = event.get("payload_hash")
        if not payload_hash:
            missing_hash += 1
        elif not HASH_RE.match(str(payload_hash)):
            bad_hash += 1
        window_id = event.get("state_window_id")
        if not window_id:
            missing_window += 1
        else:
            windows.add(str(window_id))
        wall = wall_time(event.get("wall_time_unix_s"))
        if wall is not None:
            walls.append(wall)
            if window_id and str(window_id) != state_window_id(wall):
                window_mismatch += 1
    return {
        "event_count": len(events),
        "by_lane": dict(sorted(by_lane.items())),
        "by_topic": dict(sorted(by_topic.items())),
        "by_source_identity": dict(sorted(by_source.items())),
        "missing_payload_hash": missing_hash,
        "bad_payload_hash": bad_hash,
        "missing_state_window_id": missing_window,
        "state_window_mismatch": window_mismatch,
        "state_window_count": len(windows),
        "start_wall_time_unix_s": min(walls) if walls else None,
        "end_wall_time_unix_s": max(walls) if walls else None,
    }


def collect_status(root: Path) -> JsonObject:
    events, event_errors = read_live_events(root)
    exposures, exposure_errors = read_exposure_records(root)
    summary = summarize_events(events)
    exposure_missing_hash = sum(1 for row in exposures if not row.get("prompt_hash"))
    exposure_bad_hash = sum(
        1
        for row in exposures
        if row.get("prompt_hash") and not HASH_RE.match(str(row.get("prompt_hash")))
    )
    exposure_missing_window = sum(1 for row in exposures if not row.get("state_window_id"))
    return {
        "trace_root": str(root),
        "live_event_files": [str(path) for path in live_event_files(root)],
        "live_events": summary,
        "exposure_record_count": len(exposures),
        "exposure_missing_prompt_hash": exposure_missing_hash,
        "exposure_bad_prompt_hash": exposure_bad_hash,
        "exposure_missing_state_window_id": exposure_missing_window,
        "jsonl_error_count": len(event_errors) + len(exposure_errors),
        "jsonl_errors": event_errors + exposure_errors,
    }


def format_status(status: JsonObject) -> str:
    live = status["live_events"]
    lines = [
        f"Trace Lab root: {status['trace_root']}",
        f"Live trace events: {live['event_count']}",
        f"Exposure records: {status['exposure_record_count']}",
        (
            "Missing/bad hashes: "
            f"events {live['missing_payload_hash']}/{live['bad_payload_hash']}, "
            f"exposures {status['exposure_missing_prompt_hash']}/{status['exposure_bad_prompt_hash']}"
        ),
        (
            "Missing/mismatched windows: "
            f"events {live['missing_state_window_id']}/{live['state_window_mismatch']}, "
            f"exposures {status['exposure_missing_state_window_id']}"
        ),
        f"JSONL errors: {status['jsonl_error_count']}",
        f"By lane: {json.dumps(live['by_lane'], sort_keys=True)}",
    ]
    return "\n".join(lines)


def cmd_status(args: argparse.Namespace) -> int:
    status = collect_status(trace_root(args))
    if args.json:
        print(json.dumps(status, indent=2, sort_keys=True))
    else:
        print(format_status(status))
    return 0 if status["jsonl_error_count"] == 0 else 1


def date_from_wall(value: Any) -> str:
    wall = wall_time(value)
    if wall is None:
        return "unknown-date"
    return dt.datetime.fromtimestamp(wall, tz=dt.timezone.utc).date().isoformat()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, rows: list[JsonObject]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        for row in rows:
            row = {key: value for key, value in row.items() if key != "_source_file"}
            handle.write(json.dumps(row, sort_keys=True, separators=(",", ":")) + "\n")


def cmd_archive(args: argparse.Namespace) -> int:
    root = trace_root(args)
    events, event_errors = read_live_events(root)
    exposures, exposure_errors = read_exposure_records(root)
    by_day: dict[str, list[JsonObject]] = {}
    for event in events:
        by_day.setdefault(date_from_wall(event.get("wall_time_unix_s")), []).append(event)
    written: list[str] = []
    for day, rows in sorted(by_day.items()):
        day_dir = root / "archive" / "daily" / day
        day_exposures = [
            row
            for row in exposures
            if date_from_wall(row.get("wall_time_unix_s")) == day
        ]
        summary = {
            "schema_version": 1,
            "policy": "trace_lab_daily_summary_v1",
            "date": day,
            "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
            "event_summary": summarize_events(rows),
            "exposure_record_count": len(day_exposures),
            "source_event_files": sorted(
                {str(row.get("_source_file")) for row in rows if row.get("_source_file")}
            ),
            "jsonl_error_count": len(event_errors) + len(exposure_errors),
            "full_replay_note": FULL_REPLAY_NOTE,
        }
        out = day_dir / "summary.json"
        write_json(out, summary)
        written.append(str(out))
    result = {
        "trace_root": str(root),
        "archive_summary_count": len(written),
        "written": written,
        "jsonl_error_count": len(event_errors) + len(exposure_errors),
    }
    print(json.dumps(result, indent=2, sort_keys=True) if args.json else "\n".join(written))
    return 0 if result["jsonl_error_count"] == 0 else 1


def safe_label(label: str) -> str:
    cleaned = re.sub(r"[^A-Za-z0-9_.-]+", "-", label.strip()).strip("-")
    return cleaned[:64] or "window"


def bundle_id(label: str, start: float, end: float) -> str:
    return f"bundle_{safe_label(label)}_{int(start)}_{int(end)}"


def exposure_ids_for(events: list[JsonObject]) -> set[str]:
    return {
        str(event["exposure_record_id"])
        for event in events
        if event.get("exposure_record_id")
    }


def filter_events(events: list[JsonObject], start: float, end: float) -> list[JsonObject]:
    selected: list[JsonObject] = []
    for event in events:
        wall = wall_time(event.get("wall_time_unix_s"))
        if wall is not None and start <= wall <= end:
            selected.append(event)
    return sorted(
        selected,
        key=lambda row: (
            str(row.get("stream_session_id") or ""),
            int(row.get("stream_sequence") or 0),
            wall_time(row.get("wall_time_unix_s")) or 0.0,
        ),
    )


def cmd_bundle(args: argparse.Namespace) -> int:
    root = trace_root(args)
    start = parse_time(args.start)
    end = parse_time(args.end)
    if end < start:
        raise SystemExit("--end must be greater than or equal to --start")
    events, event_errors = read_live_events(root)
    exposures, exposure_errors = read_exposure_records(root)
    selected = filter_events(events, start, end)
    exposure_ids = exposure_ids_for(selected)
    selected_exposures = [
        row for row in exposures if str(row.get("exposure_record_id")) in exposure_ids
    ]
    root_id = bundle_id(args.label, start, end)
    out_dir = root / "bundles" / root_id
    if out_dir.exists():
        out_dir = root / "bundles" / f"{root_id}_{uuid.uuid4().hex[:8]}"
    out_dir.mkdir(parents=True)
    write_jsonl(out_dir / "events.jsonl", selected)
    write_jsonl(out_dir / "exposure_records.jsonl", selected_exposures)
    manifest = {
        "schema_version": 1,
        "policy": "trace_lab_bundle_v1",
        "bundle_id": out_dir.name,
        "label": args.label,
        "start_wall_time_unix_s": start,
        "end_wall_time_unix_s": end,
        "event_count": len(selected),
        "exposure_record_count": len(selected_exposures),
        "created_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "source_trace_root": str(root),
        "full_replay_note": FULL_REPLAY_NOTE,
        "jsonl_error_count": len(event_errors) + len(exposure_errors),
    }
    write_json(out_dir / "manifest.json", manifest)
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0 if manifest["jsonl_error_count"] == 0 else 1


def resolve_bundle(root: Path, bundle: str) -> Path:
    path = Path(bundle).expanduser()
    if path.exists():
        return path.resolve()
    return (root / "bundles" / bundle).resolve()


def sha256_file(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(chunk)
    return f"sha256:{hasher.hexdigest()}"


def fill_band(fill_pct: float) -> str:
    if fill_pct >= 92.0:
        return "red"
    if fill_pct >= 85.0:
        return "orange"
    if fill_pct >= 75.0:
        return "yellow"
    return "green"


def check_close(actual: Any, expected: float, tolerance: float = TOLERANCE) -> bool:
    try:
        actual_f = float(actual)
    except (TypeError, ValueError):
        return False
    return math.isfinite(actual_f) and abs(actual_f - expected) <= tolerance


def replay_bundle(root: Path, bundle: str) -> JsonObject:
    bundle_path = resolve_bundle(root, bundle)
    events, event_errors = read_jsonl(bundle_path / "events.jsonl")
    manifest_path = bundle_path / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8")) if manifest_path.exists() else {}
    errors: list[JsonObject] = list(event_errors)
    warnings: list[JsonObject] = []
    telemetry_checks: list[JsonObject] = []
    sessions: dict[str, int] = {}
    windows: list[int] = []
    for index, event in enumerate(events):
        event_id = str(event.get("event_id") or f"row_{index + 1}")
        payload_hash = str(event.get("payload_hash") or "")
        if not HASH_RE.match(payload_hash):
            errors.append({"event_id": event_id, "error": "invalid_payload_hash"})
        payload_ref = event.get("payload_ref")
        if payload_ref:
            ref_path = Path(str(payload_ref)).expanduser()
            if ref_path.exists() and ref_path.is_file():
                actual_hash = sha256_file(ref_path)
                if actual_hash != payload_hash:
                    errors.append(
                        {
                            "event_id": event_id,
                            "error": "payload_ref_hash_mismatch",
                            "expected": payload_hash,
                            "actual": actual_hash,
                            "payload_ref": str(ref_path),
                        }
                    )
            else:
                warnings.append(
                    {
                        "event_id": event_id,
                        "warning": "payload_ref_not_available_for_hash_replay",
                        "payload_ref": str(payload_ref),
                    }
                )
        session_id = str(event.get("stream_session_id") or "")
        sequence = event.get("stream_sequence")
        if not isinstance(sequence, int):
            errors.append({"event_id": event_id, "error": "missing_stream_sequence"})
        elif session_id:
            previous = sessions.get(session_id)
            if previous is not None and sequence <= previous:
                errors.append(
                    {
                        "event_id": event_id,
                        "error": "stream_sequence_not_increasing",
                        "previous": previous,
                        "actual": sequence,
                    }
                )
            sessions[session_id] = sequence
        wall = wall_time(event.get("wall_time_unix_s"))
        if wall is None:
            errors.append({"event_id": event_id, "error": "missing_wall_time"})
        else:
            expected_window = state_window_id(wall)
            if event.get("state_window_id") != expected_window:
                errors.append(
                    {
                        "event_id": event_id,
                        "error": "state_window_mismatch",
                        "expected": expected_window,
                        "actual": event.get("state_window_id"),
                    }
                )
            window_start = window_start_from_id(expected_window)
            if window_start is not None:
                windows.append(window_start)
        compact = event.get("compact_payload")
        if isinstance(compact, dict) and compact.get("kind") == "minime_telemetry_compact_v1":
            telemetry_checks.append(validate_telemetry_compact(event_id, compact, errors))
    unique_windows = sorted(set(windows))
    gap_count = 0
    for left, right in zip(unique_windows, unique_windows[1:]):
        if right - left > STATE_WINDOW_SECS:
            gap_count += 1
            warnings.append(
                {
                    "warning": "state_window_gap",
                    "left": f"state_window_{left}",
                    "right": f"state_window_{right}",
                }
            )
    report = {
        "schema_version": 1,
        "policy": "trace_lab_replay_report_v1",
        "bundle_id": bundle_path.name,
        "bundle_path": str(bundle_path),
        "manifest": manifest,
        "event_count": len(events),
        "ok": not errors,
        "error_count": len(errors),
        "warning_count": len(warnings),
        "state_window_gap_count": gap_count,
        "telemetry_check_count": len(telemetry_checks),
        "telemetry_checks": telemetry_checks,
        "errors": errors,
        "warnings": warnings,
        "full_replay_note": FULL_REPLAY_NOTE,
    }
    replay_dir = root / "replay" / bundle_path.name
    write_json(replay_dir / "replay_report.json", report)
    write_replay_markdown(replay_dir / "replay_report.md", report)
    return report


def validate_telemetry_compact(
    event_id: str, compact: JsonObject, errors: list[JsonObject]
) -> JsonObject:
    eigenvalues = compact.get("eigenvalues")
    derived_lambda1 = None
    if isinstance(eigenvalues, list) and eigenvalues:
        try:
            derived_lambda1 = float(eigenvalues[0])
        except (TypeError, ValueError):
            derived_lambda1 = None
    if derived_lambda1 is None:
        errors.append({"event_id": event_id, "error": "telemetry_missing_lambda1_source"})
    elif not check_close(compact.get("lambda1"), derived_lambda1):
        errors.append(
            {
                "event_id": event_id,
                "error": "telemetry_lambda1_mismatch",
                "expected": derived_lambda1,
                "actual": compact.get("lambda1"),
            }
        )
    fill_ratio = compact.get("fill_ratio")
    derived_fill_pct = None
    try:
        derived_fill_pct = float(fill_ratio) * 100.0
    except (TypeError, ValueError):
        errors.append({"event_id": event_id, "error": "telemetry_missing_fill_ratio"})
    if derived_fill_pct is not None and not check_close(compact.get("fill_pct"), derived_fill_pct):
        errors.append(
            {
                "event_id": event_id,
                "error": "telemetry_fill_pct_mismatch",
                "expected": derived_fill_pct,
                "actual": compact.get("fill_pct"),
            }
        )
    derived_band = fill_band(derived_fill_pct) if derived_fill_pct is not None else None
    if derived_band is not None and compact.get("safety_level") != derived_band:
        errors.append(
            {
                "event_id": event_id,
                "error": "telemetry_fill_band_mismatch",
                "expected": derived_band,
                "actual": compact.get("safety_level"),
            }
        )
    return {
        "event_id": event_id,
        "lambda1": derived_lambda1,
        "fill_pct": derived_fill_pct,
        "fill_band": derived_band,
    }


def write_replay_markdown(path: Path, report: JsonObject) -> None:
    status = "PASS" if report["ok"] else "FAIL"
    lines = [
        f"# Trace Lab Replay: {report['bundle_id']}",
        "",
        f"Status: {status}",
        f"Events: {report['event_count']}",
        f"Errors: {report['error_count']}",
        f"Warnings: {report['warning_count']}",
        f"Telemetry checks: {report['telemetry_check_count']}",
        "",
        FULL_REPLAY_NOTE,
        "",
    ]
    if report["errors"]:
        lines.append("## Errors")
        for item in report["errors"]:
            lines.append(f"- {json.dumps(item, sort_keys=True)}")
        lines.append("")
    if report["warnings"]:
        lines.append("## Warnings")
        for item in report["warnings"]:
            lines.append(f"- {json.dumps(item, sort_keys=True)}")
        lines.append("")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines), encoding="utf-8")


def cmd_replay(args: argparse.Namespace) -> int:
    report = replay_bundle(trace_root(args), args.bundle)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["ok"] else 1


def hash_text(text: str) -> str:
    return f"sha256:{hashlib.sha256(text.encode('utf-8')).hexdigest()}"


def self_test_event(
    session: str,
    sequence: int,
    wall: float,
    lane: str,
    topic: str,
    compact: JsonObject,
    payload_hash: str,
    payload_ref: str | None = None,
) -> JsonObject:
    return {
        "schema_version": 1,
        "policy": "trace_event_v1",
        "event_id": f"{session}_{sequence}",
        "stream_session_id": session,
        "stream_sequence": sequence,
        "monotonic_time_ms": sequence * 10,
        "wall_time_unix_s": wall,
        "state_window_id": state_window_id(wall),
        "source_identity": "self-test",
        "source_class": "test",
        "lane": lane,
        "topic": topic,
        "payload_hash": payload_hash,
        "payload_ref": payload_ref,
        "compact_payload": compact,
        "runtime_build_id": "trace_lab.py:self-test",
        "authority_class": "read_only_test",
    }


def cmd_self_test(_args: argparse.Namespace) -> int:
    with tempfile.TemporaryDirectory(prefix="trace_lab_self_test_") as tmp:
        root = Path(tmp) / "trace_lab"
        base = dt.datetime(2026, 1, 1, tzinfo=dt.timezone.utc).timestamp()
        prompt_path = root / "fixtures" / "prompt.txt"
        prompt_path.parent.mkdir(parents=True)
        prompt_path.write_text("prompt fixture", encoding="utf-8")
        payload_hash = sha256_file(prompt_path)
        events = [
            self_test_event(
                "session_self_test",
                1,
                base,
                "telemetry",
                "consciousness.v1.telemetry",
                {
                    "kind": "minime_telemetry_compact_v1",
                    "t_ms": 1,
                    "lambda1": 1.25,
                    "eigenvalues": [1.25, 0.5],
                    "fill_ratio": 0.68,
                    "fill_pct": 68.0,
                    "phase": "expanding",
                    "safety_level": "green",
                },
                hash_text("{telemetry}"),
            ),
            self_test_event(
                "session_self_test",
                2,
                base + 30.0,
                "llm_prompt_exposure",
                "astrid.llm.prompt_exposure",
                {
                    "kind": "llm_prompt_exposure_compact_v1",
                    "job_id": "job_self_test",
                    "call_kind": "self_test",
                    "prompt_ref": str(prompt_path),
                    "prompt_bytes": len("prompt fixture"),
                },
                payload_hash,
                str(prompt_path),
            ),
        ]
        events[1]["exposure_record_id"] = "exposure_job_self_test"
        write_jsonl(root / "live" / "events" / "2026-01-01.jsonl", events)
        write_jsonl(
            root / "exposure_records.jsonl",
            [
                {
                    "schema_version": 1,
                    "policy": "trace_exposure_record_v1",
                    "exposure_record_id": "exposure_job_self_test",
                    "wall_time_unix_s": base + 30.0,
                    "state_window_id": state_window_id(base + 30.0),
                    "reporter_identity": "self-test",
                    "exposure_class": "llm_prompt_context",
                    "prompt_ref": str(prompt_path),
                    "prompt_hash": payload_hash,
                    "runtime_build_id": "trace_lab.py:self-test",
                    "source_refs": [],
                    "authority_class": "language_generation_context",
                }
            ],
        )
        status = collect_status(root)
        assert status["live_events"]["event_count"] == 2, status
        assert status["exposure_record_count"] == 1, status
        archive_args = argparse.Namespace(root=str(root), json=True)
        assert cmd_archive(archive_args) == 0
        bundle_args = argparse.Namespace(
            root=str(root),
            start=str(base - 1.0),
            end=str(base + 60.0),
            label="self-test",
        )
        assert cmd_bundle(bundle_args) == 0
        bundles = sorted((root / "bundles").iterdir())
        assert bundles, "bundle was not written"
        report = replay_bundle(root, bundles[0].name)
        assert report["ok"], report
        assert report["telemetry_check_count"] == 1, report
        archive_summary = root / "archive" / "daily" / "2026-01-01" / "summary.json"
        assert archive_summary.exists(), "archive summary was not written"
    print("trace_lab self-test passed")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        default=str(DEFAULT_TRACE_ROOT),
        help=f"Trace Lab root (default: {DEFAULT_TRACE_ROOT})",
    )
    sub = parser.add_subparsers(dest="command", required=True)
    status = sub.add_parser("status", help="Count live events and exposure records")
    status.add_argument("--json", action="store_true", help="Emit JSON status")
    status.set_defaults(func=cmd_status)
    archive = sub.add_parser("archive", help="Write daily JSON summaries")
    archive.add_argument("--json", action="store_true", help="Emit JSON result")
    archive.set_defaults(func=cmd_archive)
    bundle = sub.add_parser("bundle", help="Materialize a selected window")
    bundle.add_argument("--start", required=True, help="Start unix seconds or ISO time")
    bundle.add_argument("--end", required=True, help="End unix seconds or ISO time")
    bundle.add_argument("--label", required=True, help="Bundle label")
    bundle.set_defaults(func=cmd_bundle)
    replay = sub.add_parser("replay", help="Validate a bundle")
    replay.add_argument("bundle", help="Bundle id or path")
    replay.set_defaults(func=cmd_replay)
    self_test = sub.add_parser("self-test", help="Run tempdir behavioral tests")
    self_test.set_defaults(func=cmd_self_test)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BrokenPipeError:
        raise SystemExit(1)
