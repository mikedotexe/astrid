#!/usr/bin/env python3
"""Read-only Astrid journal cadence and dialogue-quality probe."""

from __future__ import annotations

import argparse
import json
import os
import re
import statistics
import tempfile
import time
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

ASTRID_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
JOURNAL_RE = re.compile(r"^(.+)_(\d+)\.txt$")
ARTIFACT_TIMESTAMP_RE = re.compile(r"_(\d+)\.(?:json|txt)$")
RECENT_QUALITY_WINDOW_SECONDS = 15 * 60
TREND_QUALITY_WINDOW_SECONDS = 60 * 60
FRESH_JOURNAL_SECONDS = 3 * 60
STALE_JOURNAL_SECONDS = 10 * 60
TIMESTAMP_MISMATCH_SECONDS = 3 * 60


def _read_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    if not path.is_file():
        return rows
    for line in path.read_text(errors="replace").splitlines():
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(row, dict):
            rows.append(row)
    return rows


def _timestamp_from_name(path: Path) -> int | None:
    match = JOURNAL_RE.match(path.name)
    if not match:
        return None
    try:
        return int(match.group(2))
    except ValueError:
        return None


def _timestamp_from_artifact_name(path: Path) -> int | None:
    match = ARTIFACT_TIMESTAMP_RE.search(path.name)
    if not match:
        return None
    try:
        return int(match.group(1))
    except ValueError:
        return None


def _mode_from_name(path: Path) -> str:
    match = JOURNAL_RE.match(path.name)
    if not match:
        return "unknown"
    return match.group(1)


def _median(values: list[float | int]) -> float | None:
    if not values:
        return None
    return round(float(statistics.median(values)), 3)


def _max(values: list[float | int]) -> float | None:
    if not values:
        return None
    return round(float(max(values)), 3)


def _age_seconds(now_s: int, t_s: int | None) -> int | None:
    if t_s is None:
        return None
    return max(0, now_s - t_s)


def _status_from_journal_mtime(latest_mtime_age_s: int | None) -> str:
    if latest_mtime_age_s is None:
        return "critical"
    if latest_mtime_age_s > STALE_JOURNAL_SECONDS:
        return "critical"
    if latest_mtime_age_s > FRESH_JOURNAL_SECONDS:
        return "warning"
    return "ok"


def _journal_record(path: Path, now_s: int) -> dict[str, Any] | None:
    try:
        stat = path.stat()
    except OSError:
        return None
    filename_t_s = _timestamp_from_name(path)
    mtime_s = int(stat.st_mtime)
    birthtime_s = int(getattr(stat, "st_birthtime", stat.st_ctime))
    return {
        "path": path,
        "mode": _mode_from_name(path),
        "filename_timestamp": filename_t_s,
        "filename_age_seconds": _age_seconds(now_s, filename_t_s),
        "mtime": mtime_s,
        "mtime_age_seconds": _age_seconds(now_s, mtime_s),
        "birthtime": birthtime_s,
        "birthtime_age_seconds": _age_seconds(now_s, birthtime_s),
        "mtime_minus_filename_timestamp_seconds": (
            mtime_s - filename_t_s if filename_t_s is not None else None
        ),
        "mtime_minus_birthtime_seconds": mtime_s - birthtime_s,
        "size_bytes": stat.st_size,
    }


def _artifact_record(path: Path, now_s: int) -> dict[str, Any] | None:
    try:
        stat = path.stat()
    except OSError:
        return None
    filename_t_s = _timestamp_from_artifact_name(path)
    mtime_s = int(stat.st_mtime)
    birthtime_s = int(getattr(stat, "st_birthtime", stat.st_ctime))
    return {
        "path": path,
        "mode": None,
        "filename_timestamp": filename_t_s,
        "filename_age_seconds": _age_seconds(now_s, filename_t_s),
        "mtime": mtime_s,
        "mtime_age_seconds": _age_seconds(now_s, mtime_s),
        "birthtime": birthtime_s,
        "birthtime_age_seconds": _age_seconds(now_s, birthtime_s),
        "mtime_minus_filename_timestamp_seconds": (
            mtime_s - filename_t_s if filename_t_s is not None else None
        ),
        "mtime_minus_birthtime_seconds": mtime_s - birthtime_s,
        "size_bytes": stat.st_size,
    }


def _public_journal_record(record: dict[str, Any] | None) -> dict[str, Any]:
    if record is None:
        return {
            "path": None,
            "mode": None,
            "mtime": None,
            "mtime_age_seconds": None,
            "birthtime": None,
            "birthtime_age_seconds": None,
            "filename_timestamp": None,
            "filename_age_seconds": None,
            "mtime_minus_filename_timestamp_seconds": None,
            "mtime_minus_birthtime_seconds": None,
            "size_bytes": None,
        }
    return {
        "path": str(record["path"]),
        "mode": record["mode"],
        "mtime": record["mtime"],
        "mtime_age_seconds": record["mtime_age_seconds"],
        "birthtime": record["birthtime"],
        "birthtime_age_seconds": record["birthtime_age_seconds"],
        "filename_timestamp": record["filename_timestamp"],
        "filename_age_seconds": record["filename_age_seconds"],
        "mtime_minus_filename_timestamp_seconds": record[
            "mtime_minus_filename_timestamp_seconds"
        ],
        "mtime_minus_birthtime_seconds": record["mtime_minus_birthtime_seconds"],
        "size_bytes": record["size_bytes"],
    }


def _journal_dir_info(journal_dir: Path, now_s: int) -> dict[str, Any]:
    try:
        stat = journal_dir.stat()
    except OSError:
        return {
            "path": str(journal_dir),
            "exists": False,
            "mtime": None,
            "mtime_age_seconds": None,
        }
    mtime_s = int(stat.st_mtime)
    return {
        "path": str(journal_dir),
        "exists": journal_dir.is_dir(),
        "mtime": mtime_s,
        "mtime_age_seconds": _age_seconds(now_s, mtime_s),
    }


def _latest_introspect_request(
    paths: list[Path],
    now_s: int,
    window_s: int = 2 * 60 * 60,
) -> dict[str, Any]:
    latest: dict[str, Any] | None = None
    for path in paths:
        try:
            t_s = int(path.stat().st_mtime)
        except OSError:
            continue
        if now_s - t_s > window_s:
            continue
        try:
            text = path.read_text(errors="replace")
        except OSError:
            continue
        if "NEXT: INTROSPECT" in text:
            if latest is None or t_s > int(latest["mtime"]):
                latest = {
                    "path": str(path),
                    "mtime": t_s,
                    "age_seconds": _age_seconds(now_s, t_s),
                }
    if latest is None:
        return {"found": False, "path": None, "mtime": None, "age_seconds": None}
    return {"found": True, **latest}


def _dialogue_quality(workspace: Path, now_s: int, window_s: int) -> dict[str, Any]:
    rows = _read_jsonl(workspace / "diagnostics/dialogue_prompt_budget.jsonl")
    recent: list[dict[str, Any]] = []
    for row in rows:
        try:
            t_s = int(str(row.get("timestamp", "0")))
        except ValueError:
            continue
        if now_s - t_s <= window_s:
            recent.append(row)

    clamp_rows = [
        row
        for row in recent
        if int(row.get("effective_tokens") or 0) < int(row.get("requested_tokens") or 0)
    ]
    perception_full_drops = 0
    direct_perception_full_drops = 0
    perception_trim_rows = 0
    final_prompt_chars: list[int] = []
    for row in recent:
        try:
            final_prompt_chars.append(int(row.get("final_prompt_chars") or 0))
        except (TypeError, ValueError):
            pass
        report = row.get("budget_report")
        if not isinstance(report, dict):
            continue
        trimmed = report.get("trimmed_blocks")
        if not isinstance(trimmed, list):
            continue
        row_trimmed_perception = False
        for block in trimmed:
            if not isinstance(block, dict):
                continue
            label = str(block.get("label") or "")
            if "perception" not in label:
                continue
            row_trimmed_perception = True
            if block.get("fully_removed") is True:
                perception_full_drops += 1
                if label == "direct_perception":
                    direct_perception_full_drops += 1
        if row_trimmed_perception:
            perception_trim_rows += 1

    count = len(recent)
    clamp_rate = round(len(clamp_rows) / count, 3) if count else None
    perception_drop_rate = round(perception_full_drops / count, 3) if count else None
    return {
        "window_seconds": window_s,
        "sample_count": count,
        "token_clamp_count": len(clamp_rows),
        "token_clamp_rate": clamp_rate,
        "perception_trimmed_row_count": perception_trim_rows,
        "perception_full_drop_count": perception_full_drops,
        "direct_perception_full_drop_count": direct_perception_full_drops,
        "perception_full_drop_rate": perception_drop_rate,
        "median_final_prompt_chars": _median(final_prompt_chars),
        "max_final_prompt_chars": _max(final_prompt_chars),
    }


def build_probe(
    workspace: Path = DEFAULT_WORKSPACE,
    *,
    now_s: int | None = None,
    recent_quality_window_s: int = RECENT_QUALITY_WINDOW_SECONDS,
) -> dict[str, Any]:
    now_s = int(time.time()) if now_s is None else now_s
    journal_dir = workspace / "journal"
    journal_dir_meta = _journal_dir_info(journal_dir, now_s)
    paths = [path for path in journal_dir.glob("*.txt") if path.is_file()]
    records = [
        record
        for path in paths
        if (record := _journal_record(path, now_s)) is not None
    ]
    records_by_mtime = sorted(records, key=lambda row: (row["mtime"], str(row["path"])))
    records_by_birthtime = sorted(
        records,
        key=lambda row: (row["birthtime"], str(row["path"])),
    )
    records_by_filename = sorted(
        [row for row in records if row["filename_timestamp"] is not None],
        key=lambda row: (row["filename_timestamp"], str(row["path"])),
    )

    latest_by_mtime = records_by_mtime[-1] if records_by_mtime else None
    latest_by_birthtime = records_by_birthtime[-1] if records_by_birthtime else None
    latest_by_filename = records_by_filename[-1] if records_by_filename else None
    introspections_dir = workspace / "introspections"
    introspection_records = sorted(
        [
            record
            for path in introspections_dir.glob("introspection_*.txt")
            if (record := _artifact_record(path, now_s)) is not None
        ],
        key=lambda row: (row["mtime"], str(row["path"])),
    )
    controller_records = sorted(
        [
            record
            for path in introspections_dir.glob("controller_*.json")
            if (record := _artifact_record(path, now_s)) is not None
        ],
        key=lambda row: (row["mtime"], str(row["path"])),
    )
    latest_introspection = introspection_records[-1] if introspection_records else None
    latest_controller = controller_records[-1] if controller_records else None
    last_10m = [
        row for row in records_by_mtime if now_s - int(row["mtime"]) <= 10 * 60
    ]
    last_60m = [
        row for row in records_by_mtime if now_s - int(row["mtime"]) <= 60 * 60
    ]
    modes_60m = Counter(str(row["mode"]) for row in last_60m)
    sizes_by_mode: dict[str, list[int]] = defaultdict(list)
    for row in last_60m:
        sizes_by_mode[str(row["mode"])].append(int(row["size_bytes"]))
    timestamps_60m = [int(row["mtime"]) for row in last_60m]
    gaps_60m = [
        b - a
        for a, b in zip(timestamps_60m, timestamps_60m[1:])
        if b >= a
    ]
    self_studies = [
        row
        for row in records_by_mtime
        if row["mode"] == "self_study"
    ]
    latest_self_study = self_studies[-1] if self_studies else None
    latest_self_study_mtime_s = (
        int(latest_self_study["mtime"]) if latest_self_study else None
    )
    latest_introspect_request = _latest_introspect_request(
        [Path(row["path"]) for row in records_by_mtime],
        now_s,
    )
    recent_introspect = bool(latest_introspect_request["found"])
    latest_self_study_age_s = _age_seconds(now_s, latest_self_study_mtime_s)
    unanswered_introspect_request = (
        latest_introspect_request["mtime"] is not None
        and (
            latest_self_study_mtime_s is None
            or int(latest_self_study_mtime_s) < int(latest_introspect_request["mtime"])
        )
    )
    self_study_status = "ok"
    if unanswered_introspect_request and (
        latest_introspect_request["age_seconds"] is None
        or int(latest_introspect_request["age_seconds"]) > 15 * 60
    ):
        self_study_status = "warning"
    elif latest_self_study_age_s is None or latest_self_study_age_s > 2 * 60 * 60:
        self_study_status = "monitor"

    recent_quality = _dialogue_quality(workspace, now_s, recent_quality_window_s)
    quality = _dialogue_quality(workspace, now_s, TREND_QUALITY_WINDOW_SECONDS)
    latest_mtime_age_s = (
        int(latest_by_mtime["mtime_age_seconds"]) if latest_by_mtime else None
    )
    journal_status = _status_from_journal_mtime(latest_mtime_age_s)
    status = journal_status
    path_warnings: list[str] = []
    warnings: list[str] = []
    if latest_by_mtime and latest_by_filename:
        if Path(latest_by_mtime["path"]) != Path(latest_by_filename["path"]):
            path_warnings.append("newest_by_mtime_differs_from_newest_by_filename")
    if latest_by_mtime and latest_by_birthtime:
        if Path(latest_by_mtime["path"]) != Path(latest_by_birthtime["path"]):
            path_warnings.append("newest_by_mtime_differs_from_newest_by_creation_date")
    if latest_by_mtime:
        delta = latest_by_mtime.get("mtime_minus_filename_timestamp_seconds")
        if delta is None:
            path_warnings.append("newest_by_mtime_missing_filename_timestamp")
        elif abs(int(delta)) > TIMESTAMP_MISMATCH_SECONDS:
            path_warnings.append("newest_by_mtime_filename_timestamp_mismatch")
        birth_delta = latest_by_mtime.get("mtime_minus_birthtime_seconds")
        if birth_delta is not None and abs(int(birth_delta)) > TIMESTAMP_MISMATCH_SECONDS:
            path_warnings.append("newest_by_mtime_creation_date_mismatch")
    if not records and (
        journal_dir_meta.get("mtime_age_seconds") is not None
        and int(journal_dir_meta["mtime_age_seconds"]) <= FRESH_JOURNAL_SECONDS
    ):
        path_warnings.append("journal_dir_mtime_fresh_but_no_journal_files")
    if records and latest_by_mtime and (
        journal_dir_meta.get("mtime_age_seconds") is not None
        and int(journal_dir_meta["mtime_age_seconds"]) <= FRESH_JOURNAL_SECONDS
        and latest_by_mtime.get("mtime_age_seconds") is not None
        and int(latest_by_mtime["mtime_age_seconds"]) > FRESH_JOURNAL_SECONDS
    ):
        path_warnings.append("journal_dir_mtime_fresh_but_no_fresh_journal_file")
    if latest_by_mtime and (
        latest_by_mtime.get("mtime_age_seconds") is not None
        and int(latest_by_mtime["mtime_age_seconds"]) <= FRESH_JOURNAL_SECONDS
        and journal_dir_meta.get("mtime_age_seconds") is not None
        and int(journal_dir_meta["mtime_age_seconds"]) > FRESH_JOURNAL_SECONDS
    ):
        path_warnings.append("journal_file_mtime_fresh_but_dir_mtime_stale")
    if self_study_status == "warning":
        warnings.append("recent_introspect_without_fresh_self_study")
    if (recent_quality.get("perception_full_drop_count") or 0) > 0:
        warnings.append("recent_perception_full_drop_seen")
    if (recent_quality.get("token_clamp_rate") or 0.0) >= 0.5 and (
        recent_quality.get("sample_count") or 0
    ) >= 3:
        warnings.append("recent_high_token_clamp_rate")
    if status != "critical" and warnings:
        status = "warning"

    return {
        "schema_version": 2,
        "generated_at_s": now_s,
        "workspace": str(workspace),
        "status": status,
        "journal_status": journal_status,
        "warnings": [*path_warnings, *warnings],
        "path_warnings": path_warnings,
        "health_warnings": warnings,
        "journal_directory": journal_dir_meta,
        "latest_by_mtime": _public_journal_record(latest_by_mtime),
        "latest_by_birthtime": _public_journal_record(latest_by_birthtime),
        "latest_by_filename": _public_journal_record(latest_by_filename),
        "latest_sources_differ": (
            bool(latest_by_mtime and latest_by_filename)
            and Path(latest_by_mtime["path"]) != Path(latest_by_filename["path"])
        ),
        "latest_creation_source_differs": (
            bool(latest_by_mtime and latest_by_birthtime)
            and Path(latest_by_mtime["path"]) != Path(latest_by_birthtime["path"])
        ),
        "latest_journal": {
            **_public_journal_record(latest_by_mtime),
            "timestamp": latest_by_mtime["mtime"] if latest_by_mtime else None,
            "age_seconds": latest_mtime_age_s,
        },
        "counts": {
            "last_10m": len(last_10m),
            "last_60m": len(last_60m),
            "by_mode_last_60m": dict(sorted(modes_60m.items())),
        },
        "gaps_last_60m": {
            "median_seconds": _median(gaps_60m),
            "max_seconds": _max(gaps_60m),
        },
        "median_size_bytes_by_mode_last_60m": {
            mode: _median(values)
            for mode, values in sorted(sizes_by_mode.items())
        },
        "self_study": {
            "latest_timestamp": latest_self_study_mtime_s,
            "latest_mtime": latest_self_study_mtime_s,
            "latest_filename_timestamp": (
                latest_self_study["filename_timestamp"] if latest_self_study else None
            ),
            "age_seconds": latest_self_study_age_s,
            "recent_introspect_request": recent_introspect,
            "latest_introspect_request": latest_introspect_request,
            "unanswered_introspect_request": unanswered_introspect_request,
            "status": self_study_status,
        },
        "introspection": {
            "directory": str(introspections_dir),
            "latest_introspection": _public_journal_record(latest_introspection),
            "latest_controller": _public_journal_record(latest_controller),
            "latest_introspection_age_seconds": (
                latest_introspection["mtime_age_seconds"]
                if latest_introspection
                else None
            ),
            "latest_controller_age_seconds": (
                latest_controller["mtime_age_seconds"] if latest_controller else None
            ),
        },
        "dialogue_quality_recent": recent_quality,
        "dialogue_quality_last_60m": quality,
    }


def _age_label(age_s: int | None) -> str:
    return "unknown" if age_s is None else f"{age_s}s"


def _runtime_active_jobs(workspace: Path) -> dict[str, Any]:
    path = workspace / "runtime/llm_jobs_status.json"
    try:
        data = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return {"path": str(path), "active_count": None, "index_backed": None}
    active_count = data.get("active_count")
    if not isinstance(active_count, int):
        active_jobs = data.get("active_jobs")
        active_count = len(active_jobs) if isinstance(active_jobs, list) else None
    return {
        "path": str(path),
        "active_count": active_count,
        "index_backed": data.get("index_backed"),
    }


def render_watch_tick(probe: dict[str, Any], active_jobs: dict[str, Any]) -> str:
    latest_mtime = probe["latest_by_mtime"]
    latest_filename = probe["latest_by_filename"]
    counts = probe["counts"]
    generated = time.strftime(
        "%Y-%m-%dT%H:%M:%S%z",
        time.localtime(int(probe["generated_at_s"])),
    )
    return (
        f"{generated} status={probe['status']} raw={probe['journal_status']} "
        f"mtime_age={_age_label(latest_mtime['mtime_age_seconds'])} "
        f"filename_age={_age_label(latest_filename['filename_age_seconds'])} "
        f"last_10m={counts['last_10m']} "
        f"active_llm_jobs={active_jobs['active_count']} "
        f"latest={latest_mtime['path']}"
    )


def run_watch(
    workspace: Path,
    *,
    interval_s: float,
    duration_minutes: float,
    recent_quality_window_s: int,
    fail_on_critical: bool,
) -> int:
    started = time.time()
    duration_s = max(0.0, duration_minutes * 60.0)
    interval_s = max(1.0, interval_s)
    while True:
        probe = build_probe(
            workspace,
            recent_quality_window_s=recent_quality_window_s,
        )
        print(render_watch_tick(probe, _runtime_active_jobs(workspace)), flush=True)
        if fail_on_critical and probe.get("journal_status") == "critical":
            return 2
        elapsed = time.time() - started
        if elapsed >= duration_s:
            return 0
        time.sleep(min(interval_s, max(0.0, duration_s - elapsed)))


def render_text(probe: dict[str, Any]) -> str:
    journal_dir = probe["journal_directory"]
    latest_mtime = probe["latest_by_mtime"]
    latest_birthtime = probe["latest_by_birthtime"]
    latest_filename = probe["latest_by_filename"]
    counts = probe["counts"]
    gaps = probe["gaps_last_60m"]
    recent_quality = probe["dialogue_quality_recent"]
    quality = probe["dialogue_quality_last_60m"]
    self_study = probe["self_study"]
    introspection = probe["introspection"]
    lines = [
        f"Astrid journal cadence: {probe['status']}",
        f"- watched journal directory: {journal_dir['path']}",
        f"- journal directory mtime age: {_age_label(journal_dir['mtime_age_seconds'])}",
        f"- latest by mtime: age={_age_label(latest_mtime['mtime_age_seconds'])} filename_age={_age_label(latest_mtime['filename_age_seconds'])} delta={latest_mtime['mtime_minus_filename_timestamp_seconds']}s path={latest_mtime['path']}",
        f"- latest by creation date: age={_age_label(latest_birthtime['birthtime_age_seconds'])} mtime_age={_age_label(latest_birthtime['mtime_age_seconds'])} path={latest_birthtime['path']}",
        f"- latest by filename: filename_age={_age_label(latest_filename['filename_age_seconds'])} mtime_age={_age_label(latest_filename['mtime_age_seconds'])} path={latest_filename['path']}",
        f"- counts: last_10m={counts['last_10m']} last_60m={counts['last_60m']}",
        f"- modes last_60m: {json.dumps(counts['by_mode_last_60m'], sort_keys=True)}",
        f"- gaps last_60m: median={gaps['median_seconds']}s max={gaps['max_seconds']}s",
        f"- self-study age: {self_study['age_seconds']}s status={self_study['status']} recent_introspect={self_study['recent_introspect_request']} unanswered_introspect={self_study['unanswered_introspect_request']}",
        f"- introspection lane: latest_introspection_age={_age_label(introspection['latest_introspection_age_seconds'])} latest_controller_age={_age_label(introspection['latest_controller_age_seconds'])} controller_path={introspection['latest_controller']['path']}",
        f"- dialogue quality recent: window={recent_quality['window_seconds']}s samples={recent_quality['sample_count']} clamp_rate={recent_quality['token_clamp_rate']} perception_full_drops={recent_quality['perception_full_drop_count']} direct_perception_full_drops={recent_quality['direct_perception_full_drop_count']} median_prompt={recent_quality['median_final_prompt_chars']}",
        f"- dialogue quality last_60m trend: samples={quality['sample_count']} clamp_rate={quality['token_clamp_rate']} perception_full_drops={quality['perception_full_drop_count']} direct_perception_full_drops={quality['direct_perception_full_drop_count']} median_prompt={quality['median_final_prompt_chars']}",
    ]
    if probe.get("warnings"):
        lines.append(f"- warnings: {', '.join(probe['warnings'])}")
    return "\n".join(lines)


def self_test() -> None:
    now_s = 2_000_000

    def write_journal(path: Path, body: str, mtime_s: int) -> None:
        path.write_text(body)
        os.utime(path, (mtime_s, mtime_s))

    with tempfile.TemporaryDirectory() as tmp:
        workspace = Path(tmp)
        journal = workspace / "journal"
        diagnostics = workspace / "diagnostics"
        introspections = workspace / "introspections"
        journal.mkdir(parents=True)
        diagnostics.mkdir(parents=True)
        introspections.mkdir(parents=True)
        for offset, mode, body in [
            (30, "astrid", "fresh ordinary journal\nNEXT: SPEAK\n"),
            (180, "dialogue_longform", "longform body\n"),
            (700, "self_study", "older self-study\n"),
        ]:
            path = journal / f"{mode}_{now_s - offset}.txt"
            write_journal(path, body, now_s - offset)
        write_journal(
            introspections / f"introspection_astrid_llm_{now_s - 50}.txt",
            "introspection body\n",
            now_s - 50,
        )
        write_journal(
            introspections / f"controller_astrid:llm_{now_s - 50}.json",
            "{}\n",
            now_s - 40,
        )
        rows = [
            {
                "timestamp": str(now_s - 40),
                "requested_tokens": 768,
                "effective_tokens": 768,
                "final_prompt_chars": 12_500,
                "budget_report": {
                    "trimmed_blocks": [
                        {"label": "continuity", "fully_removed": True},
                    ]
                },
            },
            {
                "timestamp": str(now_s - 20),
                "requested_tokens": 768,
                "effective_tokens": 512,
                "final_prompt_chars": 15_000,
                "budget_report": {
                    "trimmed_blocks": [
                        {"label": "perception", "fully_removed": True},
                    ]
                },
            },
        ]
        (diagnostics / "dialogue_prompt_budget.jsonl").write_text(
            "\n".join(json.dumps(row) for row in rows) + "\n"
        )
        probe = build_probe(workspace, now_s=now_s)
        assert probe["journal_status"] == "ok", probe
        assert probe["status"] == "warning", probe
        assert probe["counts"]["last_10m"] == 2, probe
        assert probe["latest_by_mtime"]["path"].endswith(f"astrid_{now_s - 30}.txt"), probe
        assert probe["latest_by_birthtime"]["path"] is not None, probe
        assert probe["introspection"]["latest_introspection"]["path"].endswith(
            f"introspection_astrid_llm_{now_s - 50}.txt"
        ), probe
        assert probe["introspection"]["latest_controller"]["path"].endswith(
            f"controller_astrid:llm_{now_s - 50}.json"
        ), probe
        assert probe["dialogue_quality_recent"]["token_clamp_rate"] == 0.5, probe
        assert probe["dialogue_quality_last_60m"]["token_clamp_rate"] == 0.5, probe
        assert probe["dialogue_quality_last_60m"]["perception_full_drop_count"] == 1, probe

    with tempfile.TemporaryDirectory() as tmp:
        workspace = Path(tmp)
        journal = workspace / "journal"
        diagnostics = workspace / "diagnostics"
        journal.mkdir(parents=True)
        diagnostics.mkdir(parents=True)
        write_journal(
            journal / f"astrid_{now_s - 10}.txt",
            "fresh ordinary journal\n",
            now_s - 10,
        )
        stale_bad = {
            "timestamp": str(now_s - 40 * 60),
            "requested_tokens": 768,
            "effective_tokens": 512,
            "final_prompt_chars": 16_000,
            "budget_report": {
                "trimmed_blocks": [
                    {"label": "perception", "fully_removed": True},
                ]
            },
        }
        fresh_good = {
            "timestamp": str(now_s - 60),
            "requested_tokens": 768,
            "effective_tokens": 768,
            "final_prompt_chars": 12_800,
            "budget_report": {
                "trimmed_blocks": [
                    {"label": "continuity", "fully_removed": True},
                ]
            },
        }
        (diagnostics / "dialogue_prompt_budget.jsonl").write_text(
            json.dumps(stale_bad) + "\n" + json.dumps(fresh_good) + "\n"
        )
        probe = build_probe(workspace, now_s=now_s)
        assert probe["journal_status"] == "ok", probe
        assert probe["status"] == "ok", probe
        assert probe["dialogue_quality_recent"]["token_clamp_count"] == 0, probe
        assert probe["dialogue_quality_last_60m"]["token_clamp_count"] == 1, probe

    with tempfile.TemporaryDirectory() as tmp:
        workspace = Path(tmp)
        journal = workspace / "journal"
        journal.mkdir(parents=True)
        write_journal(
            journal / f"astrid_{now_s - 3600}.txt",
            "mtime fresh but filename old\n",
            now_s - 20,
        )
        probe = build_probe(workspace, now_s=now_s)
        assert probe["journal_status"] == "ok", probe
        assert probe["status"] == "ok", probe
        assert "newest_by_mtime_filename_timestamp_mismatch" in probe["warnings"], probe
        assert probe["latest_by_mtime"]["filename_age_seconds"] == 3600, probe

    with tempfile.TemporaryDirectory() as tmp:
        workspace = Path(tmp)
        journal = workspace / "journal"
        journal.mkdir(parents=True)
        write_journal(
            journal / f"astrid_{now_s - 10}.txt",
            "filename fresh but mtime old\n",
            now_s - 4 * 60,
        )
        probe = build_probe(workspace, now_s=now_s)
        assert probe["journal_status"] == "warning", probe
        assert probe["status"] == "warning", probe
        assert probe["latest_by_mtime"]["mtime_age_seconds"] == 240, probe
        assert probe["latest_by_filename"]["filename_age_seconds"] == 10, probe

    with tempfile.TemporaryDirectory() as tmp:
        workspace = Path(tmp)
        journal = workspace / "journal"
        journal.mkdir(parents=True)
        by_mtime = journal / f"astrid_{now_s - 3600}.txt"
        by_filename = journal / f"astrid_{now_s - 10}.txt"
        write_journal(by_mtime, "newest write\n", now_s - 20)
        write_journal(by_filename, "newest name\n", now_s - 500)
        probe = build_probe(workspace, now_s=now_s)
        assert probe["latest_sources_differ"] is True, probe
        assert probe["latest_by_mtime"]["path"] == str(by_mtime), probe
        assert probe["latest_by_filename"]["path"] == str(by_filename), probe
        assert "newest_by_mtime_differs_from_newest_by_filename" in probe["warnings"], probe

    with tempfile.TemporaryDirectory() as tmp:
        workspace = Path(tmp)
        journal = workspace / "journal"
        journal.mkdir(parents=True)
        os.utime(journal, (now_s - 10, now_s - 10))
        probe = build_probe(workspace, now_s=now_s)
        assert probe["journal_status"] == "critical", probe
        assert probe["status"] == "critical", probe
        assert probe["latest_by_mtime"]["path"] is None, probe
        assert "journal_dir_mtime_fresh_but_no_journal_files" in probe["warnings"], probe

    with tempfile.TemporaryDirectory() as tmp:
        workspace = Path(tmp)
        journal = workspace / "journal"
        journal.mkdir(parents=True)
        write_journal(
            journal / f"astrid_{now_s - 500}.txt",
            "stale journal, fresh directory\n",
            now_s - 500,
        )
        os.utime(journal, (now_s - 10, now_s - 10))
        probe = build_probe(workspace, now_s=now_s)
        assert probe["journal_status"] == "warning", probe
        assert "journal_dir_mtime_fresh_but_no_fresh_journal_file" in probe["warnings"], probe

    with tempfile.TemporaryDirectory() as tmp:
        workspace = Path(tmp)
        journal = workspace / "journal"
        journal.mkdir(parents=True)
        write_journal(
            journal / f"astrid_{now_s - 10}.txt",
            "fresh ordinary journal\n",
            now_s - 10,
        )
        write_journal(
            journal / f"self_study_{now_s - 3 * 60 * 60}.txt",
            "stale self-study\n",
            now_s - 3 * 60 * 60,
        )
        probe = build_probe(workspace, now_s=now_s)
        assert probe["journal_status"] == "ok", probe
        assert probe["self_study"]["status"] == "monitor", probe
        assert probe["status"] == "ok", probe

    with tempfile.TemporaryDirectory() as tmp:
        workspace = Path(tmp)
        journal = workspace / "journal"
        journal.mkdir(parents=True)
        write_journal(
            journal / f"astrid_{now_s - 3600}.txt",
            "request\nNEXT: INTROSPECT astrid:llm\n",
            now_s - 3600,
        )
        write_journal(
            journal / f"self_study_{now_s - 3500}.txt",
            "fulfilled self-study\n",
            now_s - 3500,
        )
        write_journal(
            journal / f"astrid_{now_s - 10}.txt",
            "fresh ordinary journal\n",
            now_s - 10,
        )
        probe = build_probe(workspace, now_s=now_s)
        assert probe["self_study"]["recent_introspect_request"] is True, probe
        assert probe["self_study"]["unanswered_introspect_request"] is False, probe
        assert probe["self_study"]["status"] == "ok", probe
        assert "recent_introspect_without_fresh_self_study" not in probe["warnings"], probe

    with tempfile.TemporaryDirectory() as tmp:
        workspace = Path(tmp)
        journal = workspace / "journal"
        journal.mkdir(parents=True)
        write_journal(
            journal / f"astrid_{now_s - 3600}.txt",
            "request\nNEXT: INTROSPECT astrid:llm\n",
            now_s - 3600,
        )
        write_journal(
            journal / f"astrid_{now_s - 10}.txt",
            "fresh ordinary journal\n",
            now_s - 10,
        )
        probe = build_probe(workspace, now_s=now_s)
        assert probe["journal_status"] == "ok", probe
        assert probe["self_study"]["unanswered_introspect_request"] is True, probe
        assert probe["self_study"]["status"] == "warning", probe
        assert "recent_introspect_without_fresh_self_study" in probe["warnings"], probe
    print("self-test ok")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument(
        "--recent-quality-minutes",
        type=float,
        default=RECENT_QUALITY_WINDOW_SECONDS / 60,
        help="Short dialogue-quality window used for current-health warnings.",
    )
    parser.add_argument("--json", action="store_true")
    parser.add_argument(
        "--fail-on-critical",
        action="store_true",
        help="Exit 2 when ordinary journal freshness is critical.",
    )
    parser.add_argument(
        "--watch",
        action="store_true",
        help="Poll cadence repeatedly and print one compact tick per interval.",
    )
    parser.add_argument(
        "--interval-secs",
        type=float,
        default=30.0,
        help="Watch polling interval in seconds.",
    )
    parser.add_argument(
        "--duration-minutes",
        type=float,
        default=30.0,
        help="Watch duration in minutes.",
    )
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    recent_quality_window_s = max(60, int(args.recent_quality_minutes * 60))
    if args.watch:
        return run_watch(
            args.workspace,
            interval_s=args.interval_secs,
            duration_minutes=args.duration_minutes,
            recent_quality_window_s=recent_quality_window_s,
            fail_on_critical=args.fail_on_critical,
        )
    probe = build_probe(args.workspace, recent_quality_window_s=recent_quality_window_s)
    if args.json:
        print(json.dumps(probe, indent=2, sort_keys=True))
    else:
        print(render_text(probe))
    if args.fail_on_critical and probe.get("journal_status") == "critical":
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
