#!/usr/bin/env python3
"""Merge read-only activity timelines from independent edge appliances."""

from __future__ import annotations

import argparse
import concurrent.futures
import datetime as dt
import json
import shlex
import subprocess
import time
import unicodedata
from typing import Any

SCHEMA = "astrid_edge_fleet_activity_report_v1"
PRESETS = {
    "avado-icp": {
        "avado": {
            "ssh": "avado",
            "viewer": "/usr/libexec/astrid-edge/operator/report-edge-activity",
            "workspace": "/home/avado/.astrid/home/default/edge",
        },
        "icp": {
            "ssh": "icp",
            "viewer": "/usr/libexec/astrid-edge/operator/report-edge-activity",
            "workspace": "/home/nativeplanet/.astrid-icp/state/home/default/edge",
        },
    }
}


def remote_command(arguments: list[str]) -> str:
    return " ".join(shlex.quote(argument) for argument in arguments)


def read_host(
    appliance: str,
    profile: dict[str, str],
    args: argparse.Namespace,
) -> dict[str, Any]:
    viewer = [
        profile["viewer"],
        "--workspace",
        profile["workspace"],
        "--window-minutes",
        str(args.window_minutes),
        "--limit",
        str(args.limit),
        "--format",
        "json",
    ]
    if args.since:
        viewer.extend(["--since", args.since])
    if args.until:
        viewer.extend(["--until", args.until])
    for name in ("trace_id", "session_id", "chain_id"):
        value = getattr(args, name)
        if value:
            viewer.extend([f"--{name.replace('_', '-')}", value])
    for kind in args.kind or []:
        viewer.extend(["--kind", kind])

    local_before_ms = time.time_ns() // 1_000_000
    clock = subprocess.run(
        ["ssh", profile["ssh"], "date +%s%3N"],
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
    )
    local_after_ms = time.time_ns() // 1_000_000
    try:
        remote_ms = int(clock.stdout.strip())
        skew_ms: int | None = remote_ms - (
            local_before_ms + (local_after_ms - local_before_ms) // 2
        )
    except ValueError:
        skew_ms = None

    result = subprocess.run(
        ["ssh", profile["ssh"], remote_command(viewer)],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if result.returncode != 0:
        return {
            "appliance": appliance,
            "clock_skew_ms": skew_ms,
            "error": result.stderr.strip() or f"ssh exited {result.returncode}",
            "events": [],
        }
    try:
        report = json.loads(result.stdout)
        events = report.get("events", [])
    except json.JSONDecodeError as error:
        return {
            "appliance": appliance,
            "clock_skew_ms": skew_ms,
            "error": f"invalid remote activity JSON: {error}",
            "events": [],
        }
    values = []
    for event in events:
        if isinstance(event, dict):
            values.append({**event, "appliance": appliance})
    return {
        "appliance": appliance,
        "clock_skew_ms": skew_ms,
        "error": None,
        "events": values,
    }


def iso_time(timestamp_ms: int) -> str:
    return dt.datetime.fromtimestamp(
        timestamp_ms / 1_000, tz=dt.timezone.utc
    ).isoformat(timespec="milliseconds").replace("+00:00", "Z")


def terminal_safe_text(value: Any) -> str:
    """Neutralize controls in fleet text output while preserving JSON data."""

    return "".join(
        " "
        if unicodedata.category(character) in {"Cc", "Cf", "Cs", "Zl", "Zp"}
        else character
        for character in str(value)
    )


def short(value: Any, maximum: int = 100) -> str:
    if value in (None, ""):
        return "-"
    text = " ".join(terminal_safe_text(value).split())
    return text if len(text) <= maximum else f"{text[: maximum - 1]}…"


def text_line(event: dict[str, Any]) -> str:
    kind = str(event.get("kind", "unknown"))
    common = (
        f"{iso_time(int(event.get('timestamp_unix_ms', 0)))} "
        f"[{event.get('appliance')}] "
        f"[{str(event.get('trace_id') or 'legacy')[:8]}] {kind.upper()}"
    )
    if kind == "turn":
        detail = (
            f"status={event.get('status')} authored={str(event.get('authored')).lower()} "
            f"NEXT={short(event.get('declared_next'))}"
        )
    elif kind == "action":
        detail = (
            f"source={event.get('decision_source')} status={event.get('status')} "
            f"NEXT={short(event.get('declared_next'))} "
            f"artifact={short(event.get('artifact_path'))}"
        )
    elif kind == "chain":
        detail = (
            f"id={short(event.get('chain_id'), 44)} "
            f"step={event.get('step')}/{event.get('max_steps')} "
            f"transition={event.get('transition')}"
        )
    elif kind in {"web_request", "web_result"}:
        detail = (
            f"tool={event.get('tool_name')} status={event.get('status')} "
            f"origin={event.get('origin')} "
            f"subject={short(event.get('query') or event.get('url'))}"
        )
    else:
        detail = f"status={event.get('status')} reason={short(event.get('reason'))}"
    if event.get("trace_attribution") != "first_class":
        detail += f" attribution={event.get('trace_attribution')}"
    return terminal_safe_text(f"{common} {detail}")


def fetch(args: argparse.Namespace) -> dict[str, Any]:
    profiles = PRESETS[args.preset]
    with concurrent.futures.ThreadPoolExecutor(max_workers=len(profiles)) as pool:
        futures = [
            pool.submit(read_host, appliance, profile, args)
            for appliance, profile in profiles.items()
        ]
        hosts = [future.result() for future in futures]
    events = sorted(
        (event for host in hosts for event in host["events"]),
        key=lambda event: (
            int(event.get("timestamp_unix_ms", 0)),
            str(event.get("appliance", "")),
            str(event.get("span_id") or ""),
        ),
    )
    return {
        "schema": SCHEMA,
        "generated_at_unix_ms": time.time_ns() // 1_000_000,
        "preset": args.preset,
        "hosts": [
            {
                "appliance": host["appliance"],
                "clock_skew_ms": host["clock_skew_ms"],
                "error": host["error"],
            }
            for host in hosts
        ],
        "events": events[-args.limit :],
    }


def render(report: dict[str, Any], args: argparse.Namespace, seen: set[str]) -> None:
    fresh = []
    for event in report["events"]:
        key = json.dumps(event, sort_keys=True, separators=(",", ":"))
        if key not in seen:
            seen.add(key)
            fresh.append(event)
    if args.format == "json":
        print(json.dumps({**report, "events": fresh}, sort_keys=True), flush=True)
    elif args.format == "jsonl":
        for event in fresh:
            print(json.dumps(event, sort_keys=True), flush=True)
    else:
        if not seen or fresh:
            for host in report["hosts"]:
                print(
                    terminal_safe_text(
                        f"# {host['appliance']} clock_skew_ms={host['clock_skew_ms']} "
                        f"error={host['error'] or 'none'}"
                    )
                )
        for event in fresh:
            print(text_line(event), flush=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--preset", choices=tuple(PRESETS), default="avado-icp")
    parser.add_argument("--window-minutes", type=int, default=60)
    parser.add_argument("--since", help="ISO-8601, Unix seconds, or Unix milliseconds")
    parser.add_argument("--until", help="ISO-8601, Unix seconds, or Unix milliseconds")
    parser.add_argument("--limit", type=int, default=100)
    parser.add_argument("--trace-id")
    parser.add_argument("--session-id")
    parser.add_argument("--chain-id")
    parser.add_argument(
        "--kind",
        action="append",
        choices=(
            "turn",
            "action",
            "chain",
            "web_request",
            "web_result",
            "recovery",
        ),
    )
    parser.add_argument("--follow", action="store_true")
    parser.add_argument("--format", choices=("text", "json", "jsonl"), default="text")
    args = parser.parse_args()
    if args.window_minutes < 1 or args.limit < 1:
        parser.error("--window-minutes and --limit must be positive")

    seen: set[str] = set()
    while True:
        render(fetch(args), args, seen)
        if not args.follow:
            break
        time.sleep(5)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
