#!/usr/bin/env python3
"""Move legacy controller timeout sentinels out of Astrid-authored history.

The append-only run ledger is retained.  A private correction ledger records
exact hashes and replacement locations so reports can classify the affected
attempts as transport recoveries without rewriting historical receipts.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import time
from pathlib import Path
from typing import Any

RESPONSE_HEADING = "## Response\n\n"
TRANSPORT_HEADING = "\n\n## Transport note\n\n"
REFLECTION_HEADING = "## Reflection\n\n"
PHASE_TIMEOUT = re.compile(
    r"^Request timed out \([A-Za-z]+ phase exceeded \d+s limit\)"
    r"(?:\n\n\[Local contract repair:.*?\nNEXT: LISTEN)?$",
    re.DOTALL,
)
DAY_MILLIS = 86_400_000


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}
    return value if isinstance(value, dict) else {}


def read_json_lines(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
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


def response_from_transcript(content: str) -> str | None:
    _prefix, separator, remainder = content.partition(RESPONSE_HEADING)
    if not separator:
        return None
    response, separator, _suffix = remainder.partition(TRANSPORT_HEADING)
    return response.strip() if separator else None


def is_transport_sentinel(response: str | None) -> bool:
    return response is not None and PHASE_TIMEOUT.fullmatch(response.strip()) is not None


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    temporary = path.with_name(f".{path.name}.transport-migration-{os.getpid()}")
    temporary.write_text(
        json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    os.chmod(temporary, 0o600)
    os.replace(temporary, path)


def append_private_jsonl(path: Path, values: list[dict[str, Any]]) -> None:
    if not values:
        return
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    try:
        with os.fdopen(descriptor, "a", encoding="utf-8") as handle:
            for value in values:
                handle.write(json.dumps(value, separators=(",", ":")) + "\n")
            handle.flush()
            os.fsync(handle.fileno())
    finally:
        try:
            os.close(descriptor)
        except OSError:
            pass


def migrate(workspace: Path) -> int:
    turns = workspace / "autonomous/turns"
    recoveries = workspace / "autonomous/recoveries"
    corrections_path = workspace / "autonomous/authorship_corrections.jsonl"
    recoveries.mkdir(parents=True, exist_ok=True)
    os.chmod(recoveries, 0o700)

    existing = {
        str(row.get("original_transcript_path"))
        for row in read_json_lines(corrections_path)
        if row.get("reason") == "legacy_transport_sentinel_reclassified_non_authored"
    }

    now = int(time.time() * 1000)
    corrections: list[dict[str, Any]] = []
    corrected_turn_ids: list[int] = []
    for transcript in sorted(turns.glob("autonomous_*.md")):
        relative = str(transcript.relative_to(workspace))
        if relative in existing or transcript.is_symlink() or not transcript.is_file():
            continue
        content = transcript.read_bytes()
        response = response_from_transcript(content.decode("utf-8", errors="replace"))
        if not is_transport_sentinel(response):
            continue
        turn_text = transcript.stem.removeprefix("autonomous_")
        try:
            turn_id = int(turn_text)
        except ValueError:
            continue
        recovery_transcript = recoveries / f"legacy_transport_{transcript.name}"
        if recovery_transcript.exists():
            raise ValueError(f"recovery target already exists: {recovery_transcript}")
        os.replace(transcript, recovery_transcript)

        journal = workspace / f"journal/signal_{turn_text}.md"
        journal_hash = None
        recovery_journal_relative = None
        if journal.is_file() and not journal.is_symlink():
            journal_bytes = journal.read_bytes()
            reflection = journal_bytes.decode("utf-8", errors="replace").partition(
                REFLECTION_HEADING
            )[2].strip()
            if is_transport_sentinel(reflection):
                recovery_journal = recoveries / f"legacy_transport_{journal.name}"
                if recovery_journal.exists():
                    raise ValueError(f"recovery target already exists: {recovery_journal}")
                os.replace(journal, recovery_journal)
                journal_hash = sha256_bytes(journal_bytes)
                recovery_journal_relative = str(recovery_journal.relative_to(workspace))

        corrected_turn_ids.append(turn_id)
        corrections.append(
            {
                "schema": "astrid_edge_authorship_correction_v2",
                "recorded_at_unix_ms": now,
                "original_transcript_path": relative,
                "recovery_transcript_path": str(
                    recovery_transcript.relative_to(workspace)
                ),
                "original_journal_path": (
                    f"journal/signal_{turn_text}.md" if journal_hash else None
                ),
                "recovery_journal_path": recovery_journal_relative,
                "transcript_sha256": sha256_bytes(content),
                "journal_sha256": journal_hash,
                "response_sha256": hashlib.sha256(
                    (response or "").encode("utf-8")
                ).hexdigest(),
                "reason": "legacy_transport_sentinel_reclassified_non_authored",
                "authority": (
                    "deterministic_provenance_correction_no_model_or_action_invocation"
                ),
            }
        )

    if not corrections:
        return 0

    state_path = workspace / "autonomous/state.json"
    state = read_json(state_path)
    count = len(corrections)
    state["total_authored_turns"] = max(
        0, int(state.get("total_authored_turns", 0) or 0) - count
    )
    state["total_transport_recoveries"] = int(
        state.get("total_transport_recoveries", 0) or 0
    ) + count
    current_day = int(state.get("utc_day", 0) or 0)
    today_count = sum(turn_id // DAY_MILLIS == current_day for turn_id in corrected_turn_ids)
    state["authored_turns_today"] = max(
        0, int(state.get("authored_turns_today", 0) or 0) - today_count
    )
    state["transport_recoveries_today"] = int(
        state.get("transport_recoveries_today", 0) or 0
    ) + today_count
    last_path = state.get("last_authored_transcript_path")
    corrected_paths = {row["original_transcript_path"] for row in corrections}
    if last_path in corrected_paths:
        state["last_authored_transcript_path"] = None
        state["last_response_sha256"] = None
        state["last_declared_next"] = None
    atomic_json(state_path, state)
    append_private_jsonl(corrections_path, corrections)
    return count


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    args = parser.parse_args()
    count = migrate(args.workspace.resolve())
    print(f"corrected={count}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
