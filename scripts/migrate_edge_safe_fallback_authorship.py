#!/usr/bin/env python3
"""Correct the current pre-fix edge turn without rewriting append-only receipts."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import time
from pathlib import Path

SAFE_TAIL = (
    "[Local contract repair: no valid final action was emitted; defaulting safely "
    "to LISTEN.]\nNEXT: LISTEN"
)
RESPONSE_HEADING = "## Response\n\n"
TRANSPORT_HEADING = "\n\n## Transport note\n\n"
TRANSPORT_MARKERS = (
    "Request timed out (Streaming phase exceeded",
    "HTTP stream response headers timed out",
    "HTTP stream request cancelled",
    "HTTP stream read timed out",
)


def sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def atomic_write(path: Path, content: str) -> None:
    temporary = path.with_name(f".{path.name}.authorship-migration-{os.getpid()}")
    temporary.write_text(content, encoding="utf-8")
    os.chmod(temporary, path.stat().st_mode)
    os.replace(temporary, path)


def split_transcript(content: str) -> tuple[str, str, str]:
    prefix, separator, remainder = content.partition(RESPONSE_HEADING)
    if not separator:
        raise ValueError("transcript has no Response heading")
    response, separator, suffix = remainder.partition(TRANSPORT_HEADING)
    if not separator:
        raise ValueError("transcript has no Transport note heading")
    return prefix, response.strip(), suffix


def corrected_response(response: str) -> str | None:
    if not response.endswith(SAFE_TAIL):
        return None
    authored = response[: -len(SAFE_TAIL)].rstrip()
    if not authored:
        raise ValueError("executor fallback is the only response content")
    if any(marker in authored for marker in TRANSPORT_MARKERS):
        raise ValueError("transport fallback must not be migrated as authored prose")
    return authored


def migrate(workspace: Path) -> bool:
    state_path = workspace / "autonomous/state.json"
    state = json.loads(state_path.read_text(encoding="utf-8"))
    relative_transcript = state.get("last_authored_transcript_path")
    if not isinstance(relative_transcript, str):
        return False
    transcript_path = workspace / relative_transcript
    if transcript_path.is_symlink() or not transcript_path.is_file():
        raise ValueError("current authored transcript is not an owned regular file")

    original_transcript = transcript_path.read_text(encoding="utf-8")
    prefix, response, transport_note = split_transcript(original_transcript)
    authored = corrected_response(response)
    if authored is None:
        return False

    executor_note = (
        "executor note: legacy generic safe fallback excluded from authored "
        "transcript, journal, digest, continuity, and declared Action"
    )
    corrected_transport = transport_note.rstrip()
    if executor_note not in corrected_transport:
        corrected_transport = f"{corrected_transport}\n{executor_note}".lstrip()
    corrected_transcript = (
        f"{prefix}{RESPONSE_HEADING}{authored}{TRANSPORT_HEADING}"
        f"{corrected_transport}\n"
    )

    turn_id = transcript_path.stem.removeprefix("autonomous_")
    journal_path = workspace / f"journal/signal_{turn_id}.md"
    original_journal = None
    corrected_journal = None
    if journal_path.exists():
        if journal_path.is_symlink() or not journal_path.is_file():
            raise ValueError("matching signal journal is not an owned regular file")
        original_journal = journal_path.read_text(encoding="utf-8")
        if not original_journal.rstrip().endswith(SAFE_TAIL):
            raise ValueError("matching signal journal does not contain the expected safe tail")
        corrected_journal = original_journal.rstrip()[: -len(SAFE_TAIL)].rstrip() + "\n"

    original_state = state_path.read_text(encoding="utf-8")
    state["last_declared_next"] = None
    state["last_response_sha256"] = sha256(authored)
    corrected_state = json.dumps(state, indent=2, ensure_ascii=False) + "\n"

    atomic_write(transcript_path, corrected_transcript)
    if corrected_journal is not None:
        atomic_write(journal_path, corrected_journal)
    atomic_write(state_path, corrected_state)

    recorded_at = int(time.time() * 1000)
    correction = {
        "schema": "astrid_edge_authorship_correction_v1",
        "recorded_at_unix_ms": recorded_at,
        "transcript_path": relative_transcript,
        "journal_path": (
            str(journal_path.relative_to(workspace))
            if corrected_journal is not None
            else None
        ),
        "old_transcript_sha256": sha256(original_transcript),
        "new_transcript_sha256": sha256(corrected_transcript),
        "old_journal_sha256": (
            sha256(original_journal) if original_journal is not None else None
        ),
        "new_journal_sha256": (
            sha256(corrected_journal) if corrected_journal is not None else None
        ),
        "old_state_sha256": sha256(original_state),
        "new_state_sha256": sha256(corrected_state),
        "authored_response_sha256": sha256(authored),
        "declared_next": None,
        "reason": "legacy_executor_safe_fallback_excluded_from_model_authorship",
        "authority": "deterministic_provenance_correction_no_model_or_action_invocation",
    }
    ledger = workspace / "autonomous/authorship_corrections.jsonl"
    with ledger.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(correction, separators=(",", ":")) + "\n")
        handle.flush()
        os.fsync(handle.fileno())
    return True


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    args = parser.parse_args()
    changed = migrate(args.workspace.resolve())
    print("corrected" if changed else "no_change")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
