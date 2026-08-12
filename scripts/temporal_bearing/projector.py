"""Build exact-identity machine and owner rails without temporal inference."""

from __future__ import annotations

from collections import Counter, defaultdict
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import time
from typing import Any, Iterable

try:
    from experiential_systems.common import (
        authority_state,
        canonical_json,
        owner_atomic_write,
        owner_atomic_write_json,
        owner_atomic_write_jsonl,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        authority_state,
        canonical_json,
        owner_atomic_write,
        owner_atomic_write_json,
        owner_atomic_write_jsonl,
    )

SCHEMA = "temporal_bearing_record_v1"
STATUS_SCHEMA = "temporal_bearing_status_v1"
FRESHNESS_WINDOW_MS = 2 * 60 * 60 * 1_000
SHA256_LENGTH = 64


@dataclass(frozen=True)
class EvidenceArtifact:
    kind: str
    evidence_id: str
    source_path: str
    source_sha256: str
    timestamp_ms: int
    payload: dict[str, Any]
    exact_refs: frozenset[str]
    response_hashes: frozenset[str]


def projection_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/temporal_bearing_v1"


def _sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _canonical_sha256(value: Any) -> str:
    return _sha256_bytes(canonical_json(value).encode())


def _is_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == SHA256_LENGTH
        and all(character in "0123456789abcdef" for character in value)
    )


def _bounded_id(value: Any) -> bool:
    return (
        isinstance(value, str)
        and 1 <= len(value) <= 240
        and "/" not in value
        and "\\" not in value
        and all(character.isalnum() or character in "_.:-" for character in value)
    )


def _read_json(path: Path) -> tuple[dict[str, Any] | None, bytes | None, str | None]:
    try:
        encoded = path.read_bytes()
        value = json.loads(encoded)
    except (OSError, json.JSONDecodeError) as error:
        return None, None, str(error)
    if not isinstance(value, dict):
        return None, encoded, "root_not_object"
    return value, encoded, None


def _read_jsonl(path: Path) -> tuple[list[dict[str, Any]], list[str]]:
    rows: list[dict[str, Any]] = []
    errors: list[str] = []
    if not path.is_file():
        return rows, errors
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        return rows, [str(error)]
    for index, line in enumerate(lines, 1):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as error:
            errors.append(f"line_{index}:{error}")
            continue
        if not isinstance(value, dict):
            errors.append(f"line_{index}:not_object")
            continue
        rows.append(value)
    return rows, errors


def _all_strings(value: Any) -> set[str]:
    strings: set[str] = set()
    if isinstance(value, dict):
        for child in value.values():
            strings.update(_all_strings(child))
    elif isinstance(value, list):
        for child in value:
            strings.update(_all_strings(child))
    elif isinstance(value, str) and len(value) <= 512:
        strings.add(value)
    return strings


def _qualified_path(path: Path, workspace: Path, minime_workspace: Path) -> str:
    for owner, root in (("astrid", workspace), ("minime", minime_workspace)):
        try:
            return f"{owner}:{path.relative_to(root).as_posix()}"
        except ValueError:
            continue
    return f"external:{_sha256_bytes(str(path).encode())}"


def _source_file_sha256(path: Path) -> str | None:
    try:
        return _sha256_bytes(path.read_bytes())
    except OSError:
        return None


def _event_timestamp(value: dict[str, Any]) -> int:
    candidates = [
        value.get("recorded_at_unix_ms"),
        value.get("authored_at_unix_ms"),
        value.get("completed_at_unix_ms"),
        value.get("sampled_at_unix_ms"),
    ]
    witness = value.get("witness")
    if isinstance(witness, dict):
        candidates.append(witness.get("authored_at_unix_ms"))
    report = value.get("report_ref")
    if isinstance(report, dict) and isinstance(report.get("timestamp"), int):
        candidates.append(int(report["timestamp"]) * 1_000)
    integers = [
        int(candidate)
        for candidate in candidates
        if isinstance(candidate, int) and not isinstance(candidate, bool)
    ]
    return max(integers, default=0)


def _load_contact_evidence(
    workspace: Path,
) -> tuple[
    dict[str, dict[str, Any]],
    dict[str, dict[str, Any]],
    dict[str, dict[str, Any]],
    list[str],
]:
    roots = workspace / "diagnostics/contact_capacity_trace_v1/contact_receipts"
    contacts: dict[str, dict[str, Any]] = {}
    errors: list[str] = []
    for path in sorted(roots.glob("*.json")):
        value, _, error = _read_json(path)
        if error or value is None:
            errors.append(f"{path.name}:{error}")
            continue
        contact_id = str(value.get("contact_id") or "")
        journey_id = str(value.get("journey_id") or "")
        if not _bounded_id(contact_id) or not _bounded_id(journey_id):
            errors.append(f"{path.name}:invalid_contact_identity")
            continue
        contacts[contact_id] = value

    traces, trace_errors = _read_jsonl(
        workspace / "diagnostics/contact_capacity_trace_v2/traces.jsonl"
    )
    errors.extend(f"trace:{error}" for error in trace_errors)
    traces_by_journey = {
        str(row.get("journey_id")): row
        for row in traces
        if _bounded_id(row.get("journey_id"))
    }
    ingress, ingress_errors = _read_jsonl(
        workspace / "diagnostics/contact_capacity_trace_v2/ingress_gaps.jsonl"
    )
    errors.extend(f"ingress_gap:{error}" for error in ingress_errors)
    ingress_by_journey = {
        str(row.get("journey_id")): row
        for row in ingress
        if _bounded_id(row.get("journey_id"))
    }
    return contacts, traces_by_journey, ingress_by_journey, errors


def _witness_summary(artifact: EvidenceArtifact, basis: list[str]) -> dict[str, Any]:
    value = artifact.payload
    witness = value.get("witness") if isinstance(value.get("witness"), dict) else {}
    report = value.get("report_ref") if isinstance(value.get("report_ref"), dict) else {}
    return {
        "witness_id": value.get("witness_id") or witness.get("witness_id"),
        "introspection_id": value.get("introspection_id"),
        "event_type": value.get("event_type"),
        "authored_at_unix_ms": witness.get("authored_at_unix_ms"),
        "report_sha256": report.get("sha256"),
        "evidence_completeness": value.get("evidence_completeness"),
        "source_artifact_sha256": artifact.source_sha256,
        "link_basis": basis,
        "owner_authored_lived_state_witness": True,
        "raw_prose_included": False,
    }


def _hashed_ref(value: Any) -> str | None:
    return _sha256_bytes(value.encode()) if isinstance(value, str) and value else None


def _passage_summary(artifact: EvidenceArtifact, basis: list[str]) -> dict[str, Any]:
    passage = artifact.payload
    anchors: list[dict[str, Any]] = []
    for anchor in passage.get("anchors") or []:
        if not isinstance(anchor, dict) or not isinstance(anchor.get("current"), dict):
            continue
        current = anchor["current"]
        anchors.append(
            {
                "role": anchor.get("role"),
                "passage_context_event_id": current.get("passage_context_event_id"),
                "anchor_kind": current.get("anchor_kind"),
                "anchor_association": current.get("anchor_association"),
                "anchor_ref_sha256": _hashed_ref(current.get("anchor_ref")),
                "recorded_at_unix_ms": current.get("recorded_at_unix_ms"),
            }
        )
    bearings: list[dict[str, Any]] = []
    for strand in passage.get("strands") or []:
        if not isinstance(strand, dict):
            continue
        current = strand.get("current")
        if not isinstance(current, dict):
            continue
        bearings.append(
            {
                "strand": strand.get("strand"),
                "expression_state": strand.get("expression_state"),
                "passage_context_event_id": current.get("passage_context_event_id"),
                "movement_resistance": current.get("movement_resistance"),
                "persistence_tendency": current.get("persistence_tendency"),
                "witness_fit": current.get("witness_fit"),
                "recorded_at_unix_ms": current.get("recorded_at_unix_ms"),
                "scalar_value": None,
            }
        )
    felt = passage.get("latest_felt_review_outcome")
    felt_reports = []
    if isinstance(felt, str):
        felt_reports.append(
            {
                "outcome": felt,
                "author_domain": "passage_actor_only",
                "felt_score": None,
                "closure_inferred": False,
            }
        )
    return {
        "passage_id": passage.get("passage_id"),
        "transition_id": passage.get("transition_id"),
        "actor": passage.get("actor"),
        "latest_stage": passage.get("latest_stage"),
        "latest_stage_event_id": passage.get("latest_stage_event_id"),
        "anchors": anchors,
        "categorical_bearings": bearings,
        "felt_reports": felt_reports,
        "source_observatory_sha256": artifact.source_sha256,
        "link_basis": basis,
        "passage_auto_created": False,
        "bearing_inferred_from_telemetry": False,
        "felt_closure_inferred": False,
    }


def _shadow_summary(artifact: EvidenceArtifact, basis: list[str]) -> dict[str, Any]:
    value = artifact.payload
    bearing = value.get("history_bearing_v1")
    return {
        "shadow_trajectory_id": artifact.evidence_id,
        "source_artifact": artifact.source_path,
        "source_artifact_sha256": artifact.source_sha256,
        "source_response_sha256": value.get("source_response_sha256"),
        "recorded_at_unix_ms": artifact.timestamp_ms,
        "history_bearing_v1": bearing if isinstance(bearing, dict) else None,
        "history_sample_count": len(value.get("history") or []),
        "link_basis": basis,
        "history_source_exact": True,
        "raw_history_copied_to_bearing_record": False,
        "temporal_decay_applied": False,
        "felt_causation_inferred": False,
    }


def _texture_summary(artifact: EvidenceArtifact, basis: list[str]) -> dict[str, Any]:
    value = artifact.payload
    sources = [
        row.get("source")
        for row in value.get("strands") or []
        if isinstance(row, dict) and isinstance(row.get("source"), dict)
    ]
    return {
        "inquiry_id": value.get("inquiry_id"),
        "source_artifact": artifact.source_path,
        "source_artifact_sha256": artifact.source_sha256,
        "snapshot_sha256": _canonical_sha256(value),
        "strand_count": value.get("strand_count"),
        "unordered_pair_count": value.get("unordered_pair_count"),
        "source_evidence_ids": [row.get("source_evidence_id") for row in sources],
        "source_response_interval_sha256s": [
            row.get("source_response_interval_sha256") for row in sources
        ],
        "raw_reservoir_mode_packing_state": value.get(
            "raw_reservoir_mode_packing_state"
        ),
        "shadow_dispersal_state": value.get("shadow_dispersal_state"),
        "felt_status": value.get("felt_status"),
        "link_basis": basis,
        "timestamp_proximity_match_used": False,
        "preferred_strand_selected": False,
    }


def _load_external_evidence(
    workspace: Path, minime_workspace: Path
) -> tuple[list[EvidenceArtifact], list[str], dict[str, str | None]]:
    artifacts: list[EvidenceArtifact] = []
    errors: list[str] = []
    source_hashes: dict[str, str | None] = {}

    witness_path = workspace / "diagnostics/lived_state_witness_v1/witnesses.jsonl"
    witness_rows, witness_errors = _read_jsonl(witness_path)
    errors.extend(f"witness:{error}" for error in witness_errors)
    source_hashes["lived_state_witnesses"] = _source_file_sha256(witness_path)
    witness_source_sha = source_hashes["lived_state_witnesses"] or "0" * 64
    for row in witness_rows:
        witness_id = str(row.get("witness_id") or "")
        if not _bounded_id(witness_id):
            errors.append("witness:invalid_identity")
            continue
        artifacts.append(
            EvidenceArtifact(
                "lived_state_witness",
                witness_id,
                _qualified_path(witness_path, workspace, minime_workspace),
                witness_source_sha,
                _event_timestamp(row),
                row,
                frozenset(_all_strings(row)),
                frozenset(value for value in _all_strings(row) if _is_sha256(value)),
            )
        )

    observatory_path = (
        minime_workspace / "division/passage-observatory/observatory_v1.json"
    )
    observatory, encoded, observatory_error = _read_json(observatory_path)
    source_hashes["passage_observatory"] = (
        _sha256_bytes(encoded) if encoded is not None else None
    )
    if observatory_error and observatory_path.exists():
        errors.append(f"passage_observatory:{observatory_error}")
    if observatory:
        passages = (observatory.get("phase_passages") or {}).get("passages") or []
        for passage in passages:
            if not isinstance(passage, dict) or not _bounded_id(passage.get("passage_id")):
                errors.append("passage_observatory:invalid_passage_identity")
                continue
            stage_history = passage.get("stage_history") or []
            timestamp = max(
                (
                    int(row.get("recorded_at_unix_ms") or 0)
                    for row in stage_history
                    if isinstance(row, dict)
                ),
                default=0,
            )
            artifacts.append(
                EvidenceArtifact(
                    "phase_passage",
                    str(passage["passage_id"]),
                    _qualified_path(observatory_path, workspace, minime_workspace),
                    source_hashes["passage_observatory"] or "0" * 64,
                    timestamp,
                    passage,
                    frozenset(_all_strings(passage)),
                    frozenset(
                        value for value in _all_strings(passage) if _is_sha256(value)
                    ),
                )
            )

    shadow_dir = workspace / "shadow_cartography"
    shadow_digest = hashlib.sha256()
    for path in sorted(shadow_dir.glob("trajectory_*.json")):
        value, encoded, error = _read_json(path)
        if error or value is None or encoded is None:
            errors.append(f"shadow:{path.name}:{error}")
            continue
        source_sha = _sha256_bytes(encoded)
        shadow_digest.update(path.name.encode())
        shadow_digest.update(source_sha.encode())
        response_hash = value.get("source_response_sha256")
        response_hashes = frozenset([response_hash]) if _is_sha256(response_hash) else frozenset()
        recorded = value.get("recorded_at_unix_ms")
        if not isinstance(recorded, int):
            seconds = value.get("recorded_at_unix_s")
            recorded = int(float(seconds) * 1_000) if isinstance(seconds, (int, float)) else 0
        artifacts.append(
            EvidenceArtifact(
                "shadow_trajectory",
                f"shadow_trajectory_{source_sha[:24]}",
                _qualified_path(path, workspace, minime_workspace),
                source_sha,
                recorded,
                value,
                frozenset(_all_strings(value)),
                response_hashes,
            )
        )
    source_hashes["shadow_trajectories"] = (
        shadow_digest.hexdigest() if shadow_dir.is_dir() else None
    )

    receipt_patterns = (
        workspace / "volition_v1/astrid/inquiries/receipts",
        minime_workspace / "owner_inquiry/receipts",
        minime_workspace / "inquiries/receipts",
    )
    receipt_paths: set[Path] = set()
    for root in receipt_patterns:
        receipt_paths.update(root.glob("*.json"))
    receipt_paths.update(minime_workspace.glob("volition_v1/*/inquiries/receipts/*.json"))
    texture_digest = hashlib.sha256()
    for path in sorted(receipt_paths):
        receipt, encoded, error = _read_json(path)
        if error or receipt is None or encoded is None:
            errors.append(f"owner_inquiry:{path.name}:{error}")
            continue
        source_sha = _sha256_bytes(encoded)
        snapshots = list(_find_texture_snapshots(receipt))
        for ordinal, snapshot in enumerate(snapshots):
            inquiry_id = str(snapshot.get("inquiry_id") or path.stem)
            evidence_id = f"texture_snapshot_{_canonical_sha256(snapshot)[:24]}"
            sources = [
                row.get("source")
                for row in snapshot.get("strands") or []
                if isinstance(row, dict) and isinstance(row.get("source"), dict)
            ]
            response_hashes = frozenset(
                str(source.get("source_response_interval_sha256"))
                for source in sources
                if _is_sha256(source.get("source_response_interval_sha256"))
            )
            timestamp = max(
                (
                    int(source.get("sampled_at_unix_ms") or 0)
                    for source in sources
                ),
                default=0,
            )
            refs = _all_strings(snapshot)
            refs.add(inquiry_id)
            artifacts.append(
                EvidenceArtifact(
                    "texture_dynamics",
                    evidence_id,
                    f"{_qualified_path(path, workspace, minime_workspace)}#texture:{ordinal}",
                    source_sha,
                    timestamp,
                    snapshot,
                    frozenset(refs),
                    response_hashes,
                )
            )
            texture_digest.update(path.name.encode())
            texture_digest.update(source_sha.encode())
            texture_digest.update(evidence_id.encode())
    source_hashes["texture_snapshots"] = (
        texture_digest.hexdigest() if receipt_paths else None
    )
    return artifacts, errors, source_hashes


def _find_texture_snapshots(value: Any) -> Iterable[dict[str, Any]]:
    if isinstance(value, dict):
        if value.get("schema") == "texture_dynamics_snapshot_v1":
            yield value
        for child in value.values():
            yield from _find_texture_snapshots(child)
    elif isinstance(value, list):
        for child in value:
            yield from _find_texture_snapshots(child)


def _compact_stage(receipt: dict[str, Any] | None) -> dict[str, Any] | None:
    if not isinstance(receipt, dict):
        return None
    temporal = receipt.get("temporal_envelope_v1")
    temporal = temporal if isinstance(temporal, dict) else {}
    process = receipt.get("process_identity_v1")
    process = process if isinstance(process, dict) else {}
    return {
        "stage_id": receipt.get("stage_id"),
        "stage_index": receipt.get("stage_index"),
        "stage_kind": receipt.get("stage_kind"),
        "relation": receipt.get("relation"),
        "effect": receipt.get("effect"),
        "ownership_domain": receipt.get("ownership_domain"),
        "source_sha256": receipt.get("source_sha256"),
        "output_sha256": receipt.get("output_sha256"),
        "stage_time_unix_ms": temporal.get("stage_time_unix_ms"),
        "monotonic_time_ns": temporal.get("monotonic_time_ns"),
        "clock_scope_id": process.get("clock_scope_id"),
        "receipt_integrity_sha256": receipt.get("receipt_integrity_sha256"),
    }


def _select_terminal(receipts: list[dict[str, Any]]) -> tuple[str, dict[str, Any] | None]:
    ordered = sorted(receipts, key=lambda row: int(row.get("stage_index") or 0))
    for kind, effect, state in (
        ("delivery_evidence", "evidence_recorded", "delivered"),
        ("blocked", "blocked", "outbound_blocked"),
        ("dispatched", "dispatch_failed", "dispatch_failed"),
        ("dispatched", "dispatched", "dispatched_unconfirmed"),
    ):
        matches = [
            row
            for row in ordered
            if row.get("stage_kind") == kind and row.get("effect") == effect
        ]
        if matches:
            return state, matches[-1]
    return "response_only", None


def _process_journeys(
    workspace: Path,
    contacts: dict[str, dict[str, Any]],
    traces_by_journey: dict[str, dict[str, Any]],
    ingress_by_journey: dict[str, dict[str, Any]],
    wanted_stage_ids: set[str],
) -> tuple[
    dict[str, dict[str, Any]],
    dict[str, set[str]],
    dict[str, str],
    list[str],
    str,
]:
    facts: dict[str, dict[str, Any]] = {}
    response_to_journeys: dict[str, set[str]] = defaultdict(set)
    matched_stage_ids: dict[str, str] = {}
    errors: list[str] = []
    digest = hashlib.sha256()
    contact_by_journey = {
        str(value.get("journey_id")): value
        for value in contacts.values()
        if _bounded_id(value.get("journey_id"))
    }
    root = workspace / "diagnostics/signal_spine_v1/journeys"
    for path in sorted(root.glob("*.json")):
        try:
            encoded = path.read_bytes()
            journey = json.loads(encoded)
        except (OSError, json.JSONDecodeError) as error:
            errors.append(f"{path.name}:{error}")
            continue
        if not isinstance(journey, dict):
            errors.append(f"{path.name}:not_object")
            continue
        journey_id = str(journey.get("journey_id") or "")
        if journey.get("schema") != "causal_signal_journey_v1" or not _bounded_id(journey_id):
            errors.append(f"{path.name}:invalid_journey")
            continue
        if journey_id in facts:
            errors.append(f"{path.name}:duplicate_journey_id")
            continue
        source_sha = _sha256_bytes(encoded)
        digest.update(path.name.encode())
        digest.update(source_sha.encode())
        receipts = [row for row in journey.get("receipts") or [] if isinstance(row, dict)]
        receipts.sort(key=lambda row: int(row.get("stage_index") or 0))
        authored = next((row for row in receipts if row.get("stage_kind") == "authored"), None)
        response_sha = authored.get("output_sha256") if authored else None
        if _is_sha256(response_sha):
            response_to_journeys[str(response_sha)].add(journey_id)
        for receipt in receipts:
            stage_id = receipt.get("stage_id")
            if isinstance(stage_id, str) and stage_id in wanted_stage_ids:
                matched_stage_ids[stage_id] = journey_id
        terminal_state, terminal = _select_terminal(receipts)
        stage_times = [
            int((row.get("temporal_envelope_v1") or {}).get("stage_time_unix_ms") or 0)
            for row in receipts
            if isinstance(row.get("temporal_envelope_v1"), dict)
        ]
        origin = journey.get("journey_origin_v1")
        if not isinstance(origin, dict):
            origin = {
                "kind": "legacy_unknown",
                "classification_basis": "origin_absent_on_historical_journey",
            }
        stage_chain = [
            [
                row.get("stage_id"),
                row.get("stage_index"),
                row.get("stage_kind"),
                row.get("receipt_integrity_sha256"),
            ]
            for row in receipts
        ]
        contact = contact_by_journey.get(journey_id)
        trace = traces_by_journey.get(journey_id)
        ingress_gap = ingress_by_journey.get(journey_id)
        facts[journey_id] = {
            "journey_id": journey_id,
            "journey_source_path": f"diagnostics/signal_spine_v1/journeys/{path.name}",
            "journey_source_sha256": source_sha,
            "journey_origin_v1": origin,
            "response_origin_v1": journey.get("response_origin_v1") or "legacy_unknown",
            "lineage_valid": journey.get("lineage_valid") is True,
            "stage_count": len(receipts),
            "stage_kind_counts": dict(
                sorted(Counter(str(row.get("stage_kind") or "unknown") for row in receipts).items())
            ),
            "stage_chain_sha256": _canonical_sha256(stage_chain),
            "first_response": _compact_stage(authored),
            "response_sha256": response_sha if _is_sha256(response_sha) else None,
            "terminal_state": terminal_state,
            "terminal_stage": _compact_stage(terminal),
            "latest_machine_time_unix_ms": max(stage_times, default=0),
            "contact": contact,
            "contact_trace": trace,
            "ingress_gap": ingress_gap,
        }
    return facts, response_to_journeys, matched_stage_ids, errors, digest.hexdigest()


def _link_artifact(
    artifact: EvidenceArtifact,
    facts: dict[str, dict[str, Any]],
    contact_to_journey: dict[str, str],
    response_to_journeys: dict[str, set[str]],
    stage_to_journey: dict[str, str],
) -> tuple[dict[str, list[str]], list[str]]:
    candidates: dict[str, set[str]] = defaultdict(set)
    ambiguity: list[str] = []
    for ref in artifact.exact_refs:
        if ref in facts:
            candidates[ref].add("exact_journey_id")
        if ref in contact_to_journey:
            candidates[contact_to_journey[ref]].add("exact_contact_id")
        if ref in stage_to_journey:
            candidates[stage_to_journey[ref]].add("exact_signal_stage_id")
    for response_hash in artifact.response_hashes:
        matched = response_to_journeys.get(response_hash, set())
        if len(matched) == 1:
            candidates[next(iter(matched))].add("exact_response_sha256")
        elif len(matched) > 1:
            ambiguity.append(f"response_sha256_ambiguous:{response_hash}")
    return (
        {journey_id: sorted(bases) for journey_id, bases in sorted(candidates.items())},
        sorted(ambiguity),
    )


def _ingress_summary(fact: dict[str, Any]) -> dict[str, Any]:
    contact = fact.get("contact")
    if isinstance(contact, dict):
        return {
            "kind": "contact",
            "contact_id": contact.get("contact_id"),
            "contact_receipt_sha256": contact.get("receipt_sha256"),
            "cutoff_unix_ms": contact.get("cutoff_unix_ms"),
            "received_at_unix_ms": contact.get("received_at_unix_ms"),
            "received_monotonic_ns": contact.get("received_monotonic_ns"),
            "clock_scope_id": contact.get("clock_scope_id"),
            "admission_outcome": contact.get("admission_outcome"),
            "raw_input_included": False,
        }
    gap = fact.get("ingress_gap")
    if isinstance(gap, dict):
        return {
            "kind": "ingress_gap",
            "ingress_gap_id": gap.get("ingress_gap_id"),
            "reason": gap.get("reason"),
            "contact_root_emitted": False,
            "verified_subset_promoted_to_contact": False,
        }
    return {
        "kind": str((fact.get("journey_origin_v1") or {}).get("kind") or "legacy_unknown"),
        "contact_root_expected": False,
    }


def _clock_summary(fact: dict[str, Any]) -> dict[str, Any]:
    trace = fact.get("contact_trace")
    if not isinstance(trace, dict):
        return {
            "state": "not_applicable_without_contact_root",
            "same_process_duration_available": False,
        }
    first_relation = trace.get("input_to_first_response_clock_relation")
    terminal_relation = trace.get("input_to_terminal_clock_relation")
    valid = first_relation in {"same_process_monotonic", "pre_prompt_no_response"} and (
        terminal_relation in {"same_process_monotonic", "pre_prompt_decision"}
        or trace.get("terminal_state") == "response_only"
    )
    return {
        "state": "valid_same_process_monotonic" if valid else "gap",
        "same_process_duration_available": valid,
        "input_to_first_response_ms": trace.get("input_to_first_response_ms"),
        "input_to_first_response_relation": first_relation,
        "input_to_terminal_ms": trace.get("input_to_terminal_ms"),
        "input_to_terminal_relation": terminal_relation,
        "wall_clock_causation_inferred": False,
    }


def _record(
    fact: dict[str, Any],
    attachments: dict[str, list[dict[str, Any]]],
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    journey_id = str(fact["journey_id"])
    witnesses = attachments.get("lived_state_witness", [])
    passages = attachments.get("phase_passage", [])
    shadow = attachments.get("shadow_trajectory", [])
    texture = attachments.get("texture_dynamics", [])
    passage_ids = sorted(
        str(row["passage_id"])
        for row in passages
        if _bounded_id(row.get("passage_id"))
    )
    contact = fact.get("contact")
    contact_id = str(contact.get("contact_id")) if isinstance(contact, dict) else None
    identity = {
        "journey_id": journey_id,
        "contact_id": contact_id,
        "passage_ids": passage_ids,
        "journey_source_sha256": fact["journey_source_sha256"],
    }
    machine = {
        "ingress": _ingress_summary(fact),
        "first_response": fact.get("first_response"),
        "terminal_state": fact.get("terminal_state"),
        "dispatch_or_delivery": fact.get("terminal_stage"),
        "clock_validity": _clock_summary(fact),
        "signal_spine": {
            "journey_artifact": fact["journey_source_path"],
            "journey_artifact_sha256": fact["journey_source_sha256"],
            "stage_count": fact["stage_count"],
            "stage_kind_counts": fact["stage_kind_counts"],
            "stage_chain_sha256": fact["stage_chain_sha256"],
            "lineage_valid": fact["lineage_valid"],
            "complete_stage_rows_copied": False,
            "complete_stage_rows_available_at_exact_artifact_ref": True,
        },
        "shadow_history_evidence": shadow,
        "texture_dynamics_snapshots": texture,
    }
    owner = {
        "lived_state_witness_state": (
            "exact_witnesses_present" if witnesses else "owner_unreported_for_exact_lineage"
        ),
        "lived_state_witnesses": witnesses,
        "phase_passage_state": (
            "exact_passages_present" if passages else "owner_unreported_for_exact_lineage"
        ),
        "phase_passages": passages,
        "felt_weight_scalar": None,
        "felt_closure_inferred": False,
        "silence_interpreted": False,
    }
    row = {
        "schema": SCHEMA,
        "schema_version": 1,
        "record_id": f"temporal_bearing_{_canonical_sha256(identity)[:24]}",
        **identity,
        "journey_origin_v1": fact.get("journey_origin_v1"),
        "response_origin_v1": fact.get("response_origin_v1"),
        "latest_machine_time_unix_ms": fact.get("latest_machine_time_unix_ms"),
        "machine_rail": machine,
        "owner_rail": owner,
        "exact_identity_edges_only": True,
        "timestamp_proximity_edges_created": False,
        "passage_or_anchor_auto_created": False,
        "temporal_decay_applied": False,
        "felt_state_inferred": False,
        "closure_inferred": False,
        "live_control_authority": False,
        "artifact_authority_state_v1": authority_state(),
    }
    row["record_sha256"] = _canonical_sha256(row)
    gaps: list[dict[str, Any]] = []
    if row["machine_rail"]["first_response"] is None:
        gaps.append(_gap(journey_id, contact_id, "first_response_missing"))
    if row["machine_rail"]["terminal_state"] == "response_only":
        gaps.append(_gap(journey_id, contact_id, "terminal_response_outcome_missing"))
    if row["machine_rail"]["clock_validity"]["state"] == "gap":
        gaps.append(_gap(journey_id, contact_id, "same_process_clock_invalid_or_split"))
    return row, gaps


def _gap(journey_id: str, contact_id: str | None, reason: str) -> dict[str, Any]:
    identity = {"journey_id": journey_id, "contact_id": contact_id, "reason": reason}
    return {
        "schema": "temporal_bearing_gap_v1",
        "schema_version": 1,
        "gap_id": f"temporal_gap_{_canonical_sha256(identity)[:24]}",
        **identity,
        "existing_evidence_preserved": True,
        "felt_state_inferred": False,
        "artifact_authority_state_v1": authority_state(),
    }


def _unlinked(
    artifact: EvidenceArtifact, ambiguity: list[str]
) -> dict[str, Any]:
    return {
        "schema": "temporal_bearing_unlinked_evidence_v1",
        "schema_version": 1,
        "evidence_kind": artifact.kind,
        "evidence_id": artifact.evidence_id,
        "source_artifact": artifact.source_path,
        "source_artifact_sha256": artifact.source_sha256,
        "evidence_time_unix_ms": artifact.timestamp_ms,
        "reason": (
            "ambiguous_exact_response_hash" if ambiguity else "no_exact_lineage_identity"
        ),
        "ambiguity": ambiguity,
        "timestamp_proximity_considered": False,
        "timestamp_proximity_edge_created": False,
        "evidence_discarded": False,
        "artifact_authority_state_v1": authority_state(),
    }


def _validate_record(row: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if row.get("schema") != SCHEMA or row.get("schema_version") != 1:
        errors.append("schema")
    if not _bounded_id(row.get("journey_id")):
        errors.append("journey_id")
    if row.get("timestamp_proximity_edges_created") is not False:
        errors.append("timestamp_proximity")
    if row.get("passage_or_anchor_auto_created") is not False:
        errors.append("passage_auto_creation")
    if row.get("temporal_decay_applied") is not False:
        errors.append("temporal_decay")
    if row.get("felt_state_inferred") is not False or row.get("closure_inferred") is not False:
        errors.append("felt_inference")
    if row.get("live_control_authority") is not False:
        errors.append("control_authority")
    owner = row.get("owner_rail")
    if not isinstance(owner, dict) or owner.get("felt_weight_scalar") is not None:
        errors.append("felt_scalar")
    recorded = row.get("record_sha256")
    core = dict(row)
    core.pop("record_sha256", None)
    if recorded != _canonical_sha256(core):
        errors.append("record_sha256")
    return errors


def _write_json_if_changed(path: Path, value: Any) -> None:
    encoded = (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()
    try:
        if path.read_bytes() == encoded:
            return
    except OSError:
        pass
    owner_atomic_write(path, encoded)


def _render_report(status: dict[str, Any]) -> str:
    summary = status["summary"]
    return "\n".join(
        [
            "# Temporal Bearing V1",
            "",
            "This owner navigator presents separate machine and owner rails. Exact IDs, exact Signal stage IDs, and exact response hashes may form links. Timestamp proximity never forms a link.",
            "",
            f"- journeys indexed: {summary['journey_count']}",
            f"- contact records: {summary['contact_record_count']}",
            f"- exact lived-state bindings: {summary['lived_state_binding_count']}",
            f"- exact Phase Passage bindings: {summary['passage_binding_count']}",
            f"- exact Shadow bindings: {summary['shadow_binding_count']}",
            f"- exact Texture bindings: {summary['texture_binding_count']}",
            f"- unlinked evidence: {summary['unlinked_evidence_count']}",
            f"- machine gaps: {summary['gap_count']}",
            "",
            "Categorical bearing and felt reports remain owner-authored. Missing owner evidence remains unreported. The projection never creates a passage or anchor, scores felt weight, infers closure, applies temporal decay, or grants control authority.",
            "",
        ]
    )


def project(
    workspace: Path,
    *,
    minime_workspace: Path,
    write: bool,
    now_unix_ms: int | None = None,
) -> dict[str, Any]:
    workspace = workspace.resolve()
    minime_workspace = minime_workspace.resolve()
    now_ms = int(time.time() * 1_000) if now_unix_ms is None else now_unix_ms
    contacts, traces, ingress, contact_errors = _load_contact_evidence(workspace)
    artifacts, external_errors, external_hashes = _load_external_evidence(
        workspace, minime_workspace
    )
    wanted_stage_ids = {
        value
        for artifact in artifacts
        for value in artifact.exact_refs
        if value.startswith("stage_")
    }
    facts, response_index, stage_index, journey_errors, journey_hash = _process_journeys(
        workspace,
        contacts,
        traces,
        ingress,
        wanted_stage_ids,
    )
    contact_to_journey = {
        contact_id: str(value.get("journey_id"))
        for contact_id, value in contacts.items()
        if str(value.get("journey_id")) in facts
    }
    attachments: dict[str, dict[str, list[dict[str, Any]]]] = defaultdict(
        lambda: defaultdict(list)
    )
    unlinked: list[dict[str, Any]] = []
    for artifact in artifacts:
        links, ambiguity = _link_artifact(
            artifact, facts, contact_to_journey, response_index, stage_index
        )
        if not links:
            unlinked.append(_unlinked(artifact, ambiguity))
            continue
        for journey_id, basis in links.items():
            if artifact.kind == "lived_state_witness":
                summary = _witness_summary(artifact, basis)
            elif artifact.kind == "phase_passage":
                summary = _passage_summary(artifact, basis)
            elif artifact.kind == "shadow_trajectory":
                summary = _shadow_summary(artifact, basis)
            else:
                summary = _texture_summary(artifact, basis)
            attachments[journey_id][artifact.kind].append(summary)

    records: dict[str, dict[str, Any]] = {}
    gaps: list[dict[str, Any]] = []
    validation_errors: list[str] = []
    for journey_id, fact in sorted(facts.items()):
        row, row_gaps = _record(fact, attachments.get(journey_id, {}))
        errors = _validate_record(row)
        validation_errors.extend(f"{journey_id}:{error}" for error in errors)
        records[journey_id] = row
        gaps.extend(row_gaps)

    latest = max(
        records.values(),
        key=lambda row: (
            int(row.get("latest_machine_time_unix_ms") or 0),
            str(row.get("journey_id") or ""),
        ),
        default=None,
    )
    contacts_index: dict[str, list[str]] = defaultdict(list)
    passages_index: dict[str, list[str]] = defaultdict(list)
    for journey_id, row in records.items():
        contact_id = row.get("contact_id")
        if isinstance(contact_id, str):
            contacts_index[contact_id].append(journey_id)
        for passage_id in row.get("passage_ids") or []:
            passages_index[str(passage_id)].append(journey_id)
    indexes = {
        "schema": "temporal_bearing_indexes_v1",
        "schema_version": 1,
        "latest_journey_id": latest.get("journey_id") if latest else None,
        "journeys": {
            journey_id: f"by_journey/{journey_id}.json" for journey_id in records
        },
        "contacts": {key: sorted(value) for key, value in sorted(contacts_index.items())},
        "passages": {key: sorted(value) for key, value in sorted(passages_index.items())},
        "exact_identity_only": True,
        "timestamp_proximity_indexed": False,
    }
    errors = contact_errors + external_errors + journey_errors + validation_errors
    summary = {
        "journey_count": len(records),
        "contact_record_count": sum(row.get("contact_id") is not None for row in records.values()),
        "ingress_gap_record_count": sum(
            (row.get("journey_origin_v1") or {}).get("kind") == "ingress_gap"
            for row in records.values()
        ),
        "autonomous_record_count": sum(
            (row.get("journey_origin_v1") or {}).get("kind") == "autonomous"
            for row in records.values()
        ),
        "legacy_unknown_record_count": sum(
            (row.get("journey_origin_v1") or {}).get("kind") == "legacy_unknown"
            for row in records.values()
        ),
        "lived_state_binding_count": sum(
            len(row["owner_rail"]["lived_state_witnesses"]) for row in records.values()
        ),
        "passage_binding_count": sum(
            len(row["owner_rail"]["phase_passages"]) for row in records.values()
        ),
        "shadow_binding_count": sum(
            len(row["machine_rail"]["shadow_history_evidence"])
            for row in records.values()
        ),
        "texture_binding_count": sum(
            len(row["machine_rail"]["texture_dynamics_snapshots"])
            for row in records.values()
        ),
        "owner_unreported_record_count": sum(
            row["owner_rail"]["lived_state_witness_state"]
            == "owner_unreported_for_exact_lineage"
            and row["owner_rail"]["phase_passage_state"]
            == "owner_unreported_for_exact_lineage"
            for row in records.values()
        ),
        "unlinked_evidence_count": len(unlinked),
        "gap_count": len(gaps),
    }
    source_hashes = {
        "signal_journeys_aggregate_sha256": journey_hash,
        "contact_traces_sha256": _source_file_sha256(
            workspace / "diagnostics/contact_capacity_trace_v2/traces.jsonl"
        ),
        **external_hashes,
    }
    status = {
        "schema": STATUS_SCHEMA,
        "schema_version": 1,
        "valid": not errors,
        "write": write,
        "projection_generated_at_unix_ms": now_ms,
        "stale_after_unix_ms": now_ms + FRESHNESS_WINDOW_MS,
        "freshness_window_ms": FRESHNESS_WINDOW_MS,
        "latest_journey_id": latest.get("journey_id") if latest else None,
        "summary": summary,
        "input_hashes": source_hashes,
        "errors": errors,
        "counter_audit": {
            "status": "consistent" if not errors else "inconsistent",
            "checks": {
                "exact_identity_edges_only": True,
                "timestamp_proximity_never_creates_edges": True,
                "owner_and_machine_rails_separate": True,
                "missing_owner_evidence_remains_unreported": True,
                "passages_and_anchors_not_auto_created": True,
                "felt_weight_not_scalar_scored": True,
                "closure_not_inferred": True,
                "temporal_decay_not_applied": True,
                "projection_grants_no_control_authority": True,
            },
        },
        "artifact_authority_state_v1": authority_state(),
    }

    if write and status["valid"]:
        output = projection_dir(workspace)
        for journey_id, row in records.items():
            _write_json_if_changed(output / "by_journey" / f"{journey_id}.json", row)
        latest_payload = {
            "schema": "temporal_bearing_latest_v1",
            "schema_version": 1,
            "journey_id": latest.get("journey_id") if latest else None,
            "record": latest,
            "empty": latest is None,
        }
        _write_json_if_changed(output / "latest.json", latest_payload)
        _write_json_if_changed(output / "indexes.json", indexes)
        owner_atomic_write_jsonl(output / "gaps.jsonl", gaps)
        owner_atomic_write_jsonl(output / "unlinked_evidence.jsonl", unlinked)
        owner_atomic_write_json(output / "status.json", status)
        owner_atomic_write(output / "report.md", _render_report(status).encode())
    return status
