#!/usr/bin/env python3
"""Record and render Astrid environment-change receipts.

The receipt log is a small being-facing context surface: restarts, routing
changes, pause flags, model/provider swaps, and steward-delivered requests can
be made inspectable instead of felt as hidden scaffolding.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import time
from pathlib import Path
from typing import Any

ASTRID_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
RECEIPT_SCHEMA_VERSION = 1
RECEIPT_AUTHORITY = "environment_receipt_context_not_command"
TEXT_LIMIT = 500
SUMMARY_LIMIT = 180
SENSITIVE_KEY_RE = re.compile(r"(?:api|auth|key|secret|token|password|credential)", re.I)


def now_ms() -> int:
    return int(time.time() * 1000)


def record_id(prefix: str, t_ms: int) -> str:
    return f"{prefix}_{t_ms}_{time.time_ns() % 1_000_000}"


def clamp_text(value: Any, limit: int = TEXT_LIMIT) -> str:
    text = " ".join(str(value or "").split())
    if len(text) <= limit:
        return text
    return text[:limit].rstrip() + "..."


def receipt_paths(workspace: Path) -> dict[str, Path]:
    root = workspace / "environment_receipts"
    return {
        "root": root,
        "jsonl": root / "environment_receipts.jsonl",
        "latest_json": root / "latest_environment_receipt.json",
        "latest_md": root / "latest_environment_receipts.md",
    }


def read_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text())
    except Exception:
        return {}
    return payload if isinstance(payload, dict) else {}


def atomic_write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.tmp")
    tmp.write_text(text)
    tmp.replace(path)


def state_summary(workspace: Path) -> dict[str, Any]:
    state = read_json(workspace / "state.json")
    if not state:
        return {"available": False}
    history = state.get("history")
    glimpse = state.get("last_remote_glimpse_12d")
    summary: dict[str, Any] = {
        "available": True,
        "exchange_count": state.get("exchange_count"),
        "creative_temperature": state.get("creative_temperature"),
        "history_count": len(history) if isinstance(history, list) else None,
        "last_remote_memory_role": state.get("last_remote_memory_role"),
    }
    if isinstance(glimpse, list) and len(glimpse) >= 12:
        summary["remote_memory_shape"] = {
            "dominant": round(float(glimpse[0]), 3),
            "shoulder": round(float(glimpse[1]), 3),
            "tail": round(float(glimpse[2]), 3),
            "entropy": round(float(glimpse[7]), 3),
            "geom": round(float(glimpse[10]), 3),
        }
    return summary


def parse_detail(raw: str) -> tuple[str, Any]:
    if "=" not in raw:
        return "detail", clamp_text(raw)
    key, value = raw.split("=", 1)
    key = re.sub(r"[^A-Za-z0-9_.-]+", "_", key.strip()).strip("_") or "detail"
    if SENSITIVE_KEY_RE.search(key):
        return key, "[redacted]"
    return key, clamp_text(value)


def parse_details(items: list[str]) -> dict[str, Any]:
    details: dict[str, Any] = {}
    for idx, raw in enumerate(items):
        key, value = parse_detail(raw)
        if key == "detail" and key in details:
            key = f"detail_{idx + 1}"
        details[key] = value
    return details


def build_receipt(
    workspace: Path,
    *,
    event: str,
    source: str,
    note: str = "",
    details: dict[str, Any] | None = None,
) -> dict[str, Any]:
    t_ms = now_ms()
    clean_event = re.sub(r"[^A-Za-z0-9_.:-]+", "_", event.strip()).strip("_") or "event"
    clean_source = clamp_text(source or "unknown", 120)
    return {
        "schema_version": RECEIPT_SCHEMA_VERSION,
        "id": record_id("env_receipt", t_ms),
        "t_ms": t_ms,
        "iso_time": dt.datetime.fromtimestamp(t_ms / 1000, dt.UTC).isoformat(),
        "event": clean_event,
        "source": clean_source,
        "note": clamp_text(note),
        "details": details or {},
        "state_summary": state_summary(workspace),
        "witness_only": True,
        "authority": RECEIPT_AUTHORITY,
    }


def read_receipts(workspace: Path) -> list[dict[str, Any]]:
    path = receipt_paths(workspace)["jsonl"]
    if not path.is_file():
        return []
    rows: list[dict[str, Any]] = []
    for line in path.read_text().splitlines():
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except Exception:
            continue
        if isinstance(payload, dict):
            rows.append(payload)
    rows.sort(key=lambda row: int(row.get("t_ms") or 0))
    return rows


def append_receipt(workspace: Path, receipt: dict[str, Any]) -> None:
    paths = receipt_paths(workspace)
    paths["root"].mkdir(parents=True, exist_ok=True)
    with paths["jsonl"].open("a") as fh:
        fh.write(json.dumps(receipt, sort_keys=True, ensure_ascii=False) + "\n")
    atomic_write_text(
        paths["latest_json"],
        json.dumps(receipt, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
    )
    atomic_write_text(paths["latest_md"], render_markdown(read_receipts(workspace), limit=5))


def _state_fragment(receipt: dict[str, Any]) -> str:
    state = receipt.get("state_summary") if isinstance(receipt.get("state_summary"), dict) else {}
    bits = []
    if state.get("exchange_count") is not None:
        bits.append(f"exchanges={state['exchange_count']}")
    if state.get("history_count") is not None:
        bits.append(f"history={state['history_count']}")
    if state.get("last_remote_memory_role"):
        bits.append(f"minime_memory={state['last_remote_memory_role']}")
    return ", ".join(bits)


def render_lines(receipts: list[dict[str, Any]], *, limit: int = 5) -> list[str]:
    tail = receipts[-limit:] if len(receipts) > limit else receipts
    if not tail:
        return ["- no environment receipts yet"]
    lines = []
    for receipt in tail:
        note = clamp_text(receipt.get("note") or "", SUMMARY_LIMIT)
        state = _state_fragment(receipt)
        suffix = f" ({state})" if state else ""
        note_part = f": {note}" if note else ""
        lines.append(
            f"- {receipt.get('iso_time', receipt.get('t_ms'))} "
            f"{receipt.get('event', 'event')} via {receipt.get('source', 'unknown')}"
            f"{note_part}{suffix}"
        )
    return lines


def render_markdown(receipts: list[dict[str, Any]], *, limit: int = 5) -> str:
    lines = [
        "# Astrid Environment Receipts",
        "",
        "These receipts are context, not commands. They make steward/runtime scaffolding inspectable.",
        "",
        *render_lines(receipts, limit=limit),
        "",
    ]
    return "\n".join(lines)


def record_receipt(
    workspace: Path,
    *,
    event: str,
    source: str,
    note: str = "",
    details: dict[str, Any] | None = None,
) -> dict[str, Any]:
    receipt = build_receipt(workspace, event=event, source=source, note=note, details=details)
    append_receipt(workspace, receipt)
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser(description="Astrid environment receipt log")
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    sub = parser.add_subparsers(dest="command", required=True)

    record = sub.add_parser("record", help="append an environment receipt")
    record.add_argument("event")
    record.add_argument("--source", default="steward")
    record.add_argument("--note", default="")
    record.add_argument("--detail", action="append", default=[])

    summary = sub.add_parser("summary", help="print recent environment receipts")
    summary.add_argument("--limit", type=int, default=5)

    paths = sub.add_parser("paths", help="print receipt artifact paths")
    paths.add_argument("--json", action="store_true", dest="as_json")

    args = parser.parse_args()
    workspace = args.workspace
    if args.command == "record":
        receipt = record_receipt(
            workspace,
            event=args.event,
            source=args.source,
            note=args.note,
            details=parse_details(args.detail),
        )
        print(receipt["id"])
        return 0
    if args.command == "summary":
        print("\n".join(render_lines(read_receipts(workspace), limit=max(args.limit, 1))))
        return 0
    if args.command == "paths":
        paths_payload = {key: str(value) for key, value in receipt_paths(workspace).items()}
        if args.as_json:
            print(json.dumps(paths_payload, indent=2, sort_keys=True))
        else:
            print("\n".join(f"{key}: {value}" for key, value in paths_payload.items()))
        return 0
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
