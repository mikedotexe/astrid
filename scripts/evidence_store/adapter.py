"""Activation-aware adapter used by existing evidence projectors.

The public scripts keep their V1 files and commands. Once the verified V2
activation manifest exists, reads and appends are redirected to the canonical
multi-stream store without changing projector call sites.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any

from .model import ProvenanceSourceV1
from .store import DEFAULT_ACTOR, EvidenceEventStore, EvidenceStoreError

STORE_DIRNAME = "evidence_event_store_v2"
MODE_ENV = "ASTRID_EVIDENCE_STORE_MODE"
ROOT_ENV = "ASTRID_EVIDENCE_STORE_ROOT"
VALID_STREAMS = frozenset(
    {
        "addressing",
        "sandbox",
        "corridor_v1",
        "corridor_v2",
        "signal_spine",
        "lived_state_witness",
        "claim_families",
        "felt_contracts",
        "model_qos",
        "steward_control",
    }
)


def store_root_for_state(state_dir: Path) -> Path | None:
    override = os.environ.get(ROOT_ENV)
    if override:
        return Path(override).expanduser().resolve()
    state = Path(state_dir).resolve()
    for candidate in (state, *state.parents):
        if candidate.name == "diagnostics":
            return candidate / STORE_DIRNAME
    return None


def _activation(store: EvidenceEventStore) -> dict[str, Any]:
    if not store.activation_path.is_file():
        return {}
    try:
        value = json.loads(store.activation_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}
    return value if isinstance(value, dict) else {}


def v2_active_for_state(state_dir: Path) -> bool:
    mode = os.environ.get(MODE_ENV, "auto").strip().lower()
    if mode == "v1":
        return False
    root = store_root_for_state(state_dir)
    if root is None:
        if mode == "v2":
            raise EvidenceStoreError(
                f"{MODE_ENV}=v2 requires {ROOT_ENV} or a state directory under diagnostics"
            )
        return False
    activation = _activation(EvidenceEventStore(root))
    active = activation.get("active_store") == "v2"
    if mode == "v2" and not active:
        raise EvidenceStoreError("V2 was forced but no verified V2 activation is present")
    return active


def _validated_stream(stream: str) -> str:
    if stream not in VALID_STREAMS:
        raise EvidenceStoreError(f"unknown evidence stream: {stream!r}")
    return stream


def append_domain_events(
    state_dir: Path,
    stream: str,
    events: list[dict[str, Any]],
    *,
    actor: str = DEFAULT_ACTOR,
) -> None:
    if not events:
        return
    root = store_root_for_state(state_dir)
    if root is None:
        raise EvidenceStoreError("cannot resolve the canonical V2 store root")
    if not v2_active_for_state(state_dir):
        raise EvidenceStoreError("V2 append requested while V1 remains active")
    resolved_stream = _validated_stream(stream)
    idempotency_keys = [
        str(event["idempotency_key"]) if event.get("idempotency_key") else None
        for event in events
    ]
    EvidenceEventStore(root).append_payloads(
        resolved_stream,
        events,
        actor=actor or DEFAULT_ACTOR,
        source=ProvenanceSourceV1(
            kind="projector_runtime_append",
            locator=str(Path(state_dir) / "events.jsonl"),
        ),
        idempotency_keys=idempotency_keys,
    )


def read_domain_events(
    state_dir: Path,
    stream: str,
) -> tuple[list[dict[str, Any]], int]:
    root = store_root_for_state(state_dir)
    if root is None:
        raise EvidenceStoreError("cannot resolve the canonical V2 store root")
    if not v2_active_for_state(state_dir):
        raise EvidenceStoreError("V2 read requested while V1 remains active")
    return EvidenceEventStore(root).payloads_for_stream(_validated_stream(stream))
