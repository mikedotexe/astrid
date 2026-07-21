"""Bounded source references and evidence-only witness events."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .model import authority_state, sha256_bytes


def parse_source_ref(source: str, roots: dict[str, Path]) -> dict[str, Any]:
    text = source.strip()
    label, separator, path_text = text.rpartition(" (")
    if separator and path_text.endswith(")"):
        candidate_text = path_text[:-1]
    else:
        label = text
        candidate_text = ""
    result: dict[str, Any] = {
        "source_label_sha256": sha256_bytes(label.strip().encode()),
        "source_owner": "unknown",
        "repository_relative_path": None,
        "source_path_recoverable": False,
    }
    if candidate_text:
        candidate = Path(candidate_text).expanduser().resolve(strict=False)
        for owner, root in roots.items():
            try:
                relative = candidate.relative_to(root.resolve())
            except ValueError:
                continue
            result.update(
                {
                    "source_owner": owner,
                    "repository_relative_path": relative.as_posix(),
                    "source_path_recoverable": True,
                }
            )
            break
    return result


def event_record(
    event_type: str,
    witness_id: str,
    idempotency_key: str,
    **values: Any,
) -> dict[str, Any]:
    return {
        "schema": "lived_state_witness_domain_event_v1",
        "schema_version": 1,
        "event_type": event_type,
        "aggregate_type": "temporal_lived_state_witness",
        "aggregate_id": witness_id,
        "witness_id": witness_id,
        **values,
        "idempotency_key": idempotency_key,
        "artifact_authority_state_v1": authority_state(),
    }
