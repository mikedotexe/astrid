#!/usr/bin/env python3
"""Ingest bounded Causal Signal Spine receipts into Evidence Event Store V2."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from typing import Any

try:
    from evidence_store import EvidenceEventStore
    from evidence_store.adapter import append_domain_events, read_domain_events
    from evidence_store.model import canonical_json
except ModuleNotFoundError:
    from scripts.evidence_store import EvidenceEventStore
    from scripts.evidence_store.adapter import append_domain_events, read_domain_events
    from scripts.evidence_store.model import canonical_json

DEFAULT_WORKSPACE = Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace")
PROJECTOR_VERSION = 1


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/signal_spine_v1"


def authority_state() -> dict[str, Any]:
    return {
        "schema": "artifact_authority_state_v1",
        "schema_version": 1,
        "state": "evidence_only",
        "live_eligible_now": False,
        "auto_approved": False,
        "grants_approval": False,
        "edits_source_now": False,
    }


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def load_json(path: Path) -> dict[str, Any] | None:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return value if isinstance(value, dict) else None


def integer_field(value: Any, *, field: str, errors: list[str]) -> int | None:
    if isinstance(value, bool) or not isinstance(value, int):
        errors.append(f"{field}:not_integer")
        return None
    return value


def expected_source_hash(
    parents: list[Any],
    seen: dict[str, dict[str, Any]],
) -> str | None:
    parent_receipts = [seen.get(str(parent)) for parent in parents]
    if any(parent is None for parent in parent_receipts):
        return None
    parent_hashes = [
        str(parent.get("output_sha256") or "")
        for parent in parent_receipts
        if parent is not None
    ]
    if len(parent_hashes) == 1:
        return parent_hashes[0]
    if len(parent_hashes) > 1:
        return sha256_bytes(canonical_json(parent_hashes).encode())
    return None


def receipt_integrity_valid(receipt: dict[str, Any]) -> bool:
    unsigned = dict(receipt)
    expected_integrity = str(unsigned.pop("receipt_integrity_sha256", ""))
    return expected_integrity == sha256_bytes(canonical_json(unsigned).encode())


def verify_stage_provenance(
    receipt: dict[str, Any],
    *,
    prefix: str,
    errors: list[str],
    allow_observation_parent: bool = False,
) -> None:
    provenance = receipt.get("provenance_ref_v1")
    if not isinstance(provenance, dict):
        errors.append(f"{prefix}:provenance_not_object")
        return
    if provenance.get("source_id") != receipt.get("stage_id"):
        errors.append(f"{prefix}:provenance_source_id_mismatch")
    if provenance.get("canonical_sha256") != receipt.get("output_sha256"):
        errors.append(f"{prefix}:provenance_output_hash_mismatch")
    receipt_parents = receipt.get("parent_stage_ids")
    receipt_parents = receipt_parents if isinstance(receipt_parents, list) else []
    provenance_parents = provenance.get("parent_ids")
    provenance_parents = (
        provenance_parents if isinstance(provenance_parents, list) else []
    )
    if allow_observation_parent:
        if (
            len(provenance_parents) != len(receipt_parents) + 1
            or provenance_parents[: len(receipt_parents)] != receipt_parents
            or not str(provenance_parents[-1] or "")
        ):
            errors.append(f"{prefix}:provenance_parent_chain_mismatch")
    elif provenance_parents != receipt_parents:
        errors.append(f"{prefix}:provenance_parent_chain_mismatch")
    origin_by_owner = {
        "astrid_authored": "astrid_interpretation",
        "bridge_codec": "bridge_derived",
        "bridge_evidence": "bridge_derived",
        "bridge_safety": "bridge_derived",
        "bridge_dispatch": "bridge_derived",
        "minime_observed": "minime_observation",
    }
    expected_origin = origin_by_owner.get(str(receipt.get("ownership_domain") or ""))
    if expected_origin is None or provenance.get("origin") != expected_origin:
        errors.append(f"{prefix}:provenance_origin_mismatch")
    fields = provenance.get("field_paths")
    expected_field = f"signal_spine.{receipt.get('stage_kind')}"
    if not isinstance(fields, list) or expected_field not in fields:
        errors.append(f"{prefix}:provenance_field_path_mismatch")


def verify_journey(value: dict[str, Any]) -> dict[str, Any]:
    errors: list[str] = []
    journey_id = str(value.get("journey_id") or "")
    receipts = value.get("receipts")
    if not isinstance(receipts, list):
        return {"valid": False, "errors": ["receipts_not_array"]}
    seen: dict[str, dict[str, Any]] = {}
    previous_monotonic = -1
    for expected_index, receipt in enumerate(receipts):
        if not isinstance(receipt, dict):
            errors.append(f"stage_{expected_index}:not_object")
            continue
        prefix = f"stage_{expected_index}"
        stage_id = str(receipt.get("stage_id") or "")
        if receipt.get("journey_id") != journey_id:
            errors.append(f"{prefix}:journey_id_mismatch")
        stage_index = integer_field(
            receipt.get("stage_index"),
            field=f"{prefix}:stage_index",
            errors=errors,
        )
        if stage_index is not None and stage_index != expected_index:
            errors.append(f"{prefix}:stage_index_mismatch")
        if not receipt_integrity_valid(receipt):
            errors.append(f"{prefix}:receipt_integrity_mismatch")
        verify_stage_provenance(receipt, prefix=prefix, errors=errors)
        parents = receipt.get("parent_stage_ids")
        parents = parents if isinstance(parents, list) else []
        relation = str(receipt.get("relation") or "")
        if relation == "root":
            if parents:
                errors.append(f"{prefix}:root_has_parents")
        elif not parents:
            errors.append(f"{prefix}:non_root_without_parent")
        for parent in parents:
            if str(parent) not in seen:
                errors.append(f"{prefix}:unknown_or_forward_parent")
        expected_source = expected_source_hash(parents, seen)
        if (
            expected_source is not None
            and str(receipt.get("source_sha256") or "") != expected_source
        ):
            errors.append(f"{prefix}:source_parent_hash_mismatch")
        temporal = receipt.get("temporal_envelope_v1")
        temporal = temporal if isinstance(temporal, dict) else {}
        monotonic = integer_field(
            temporal.get("monotonic_time_ns"),
            field=f"{prefix}:monotonic_time_ns",
            errors=errors,
        )
        if monotonic is not None:
            if monotonic < previous_monotonic:
                errors.append(f"{prefix}:monotonic_time_regressed")
            previous_monotonic = monotonic
        arrival = integer_field(
            temporal.get("arrival_time_unix_ms"),
            field=f"{prefix}:arrival_time_unix_ms",
            errors=errors,
        )
        stage_time = integer_field(
            temporal.get("stage_time_unix_ms"),
            field=f"{prefix}:stage_time_unix_ms",
            errors=errors,
        )
        if arrival is not None and stage_time is not None and stage_time < arrival:
            errors.append(f"{prefix}:stage_time_precedes_arrival")
        if receipt.get("raw_response_prose_included") is not False:
            errors.append(f"{prefix}:raw_response_prose_marker")
        if receipt.get("live_control_authority") is not False:
            errors.append(f"{prefix}:live_control_authority_marker")
        if not stage_id or stage_id in seen:
            errors.append(f"{prefix}:missing_or_duplicate_stage_id")
        else:
            seen[stage_id] = receipt
    if value.get("lineage_valid") is not True:
        errors.append("runtime_lineage_not_valid")
    stage_count = integer_field(
        value.get("stage_count"),
        field="stage_count",
        errors=errors,
    )
    if stage_count is not None and stage_count != len(receipts):
        errors.append("stage_count_mismatch")
    if value.get("raw_response_prose_included") is not False:
        errors.append("journey_raw_response_prose_marker")
    if value.get("live_control_authority") is not False:
        errors.append("journey_live_control_authority_marker")
    return {"valid": not errors, "errors": errors}


def verify_temporal_association(value: dict[str, Any]) -> dict[str, Any]:
    errors: list[str] = []
    journey_id = str(value.get("journey_id") or "")
    receipt = value.get("receipt")
    if not isinstance(receipt, dict):
        return {"valid": False, "errors": ["receipt_not_object"]}
    if receipt.get("journey_id") != journey_id:
        errors.append("receipt_journey_id_mismatch")
    if receipt.get("stage_kind") != "minime_telemetry_window":
        errors.append("receipt_stage_kind_mismatch")
    if receipt.get("relation") != "temporal_association":
        errors.append("receipt_relation_mismatch")
    if receipt.get("effect") != "temporally_associated":
        errors.append("receipt_effect_mismatch")
    if receipt.get("ownership_domain") != "minime_observed":
        errors.append("receipt_ownership_mismatch")
    parents = receipt.get("parent_stage_ids")
    if not isinstance(parents, list) or len(parents) != 1:
        errors.append("receipt_parent_count_mismatch")
    if not receipt_integrity_valid(receipt):
        errors.append("receipt_integrity_mismatch")
    verify_stage_provenance(
        receipt,
        prefix="receipt",
        errors=errors,
        allow_observation_parent=True,
    )
    if value.get("relation") != "temporal_association_not_direct_causation":
        errors.append("association_relation_mismatch")
    if value.get("direct_causation_claimed") is not False:
        errors.append("direct_causation_marker")
    if value.get("wire_acknowledgement_available") is not False:
        errors.append("wire_acknowledgement_marker")
    if value.get("raw_response_prose_included") is not False:
        errors.append("raw_response_prose_marker")
    if value.get("live_control_authority") is not False:
        errors.append("live_control_authority_marker")
    return {"valid": not errors, "errors": errors}


def source_events(workspace: Path) -> list[dict[str, Any]]:
    root = state_dir(workspace)
    events: list[dict[str, Any]] = []
    for path in sorted((root / "journeys").glob("*.json")):
        raw = path.read_bytes()
        value = load_json(path)
        if value is None or value.get("schema") != "causal_signal_journey_v1":
            continue
        journey_id = str(value.get("journey_id") or path.stem)
        verification = verify_journey(value)
        if not verification["valid"]:
            events.append(
                {
                    "schema": "signal_spine_domain_event_v1",
                    "schema_version": 1,
                    "event_type": "signal_journey_rejected_tamper",
                    "aggregate_type": "causal_signal_journey",
                    "aggregate_id": journey_id,
                    "journey_id": journey_id,
                    "source_receipt": {
                        "kind": "rejected_signal_journey_receipt",
                        "relative_path": str(path.relative_to(workspace)),
                        "sha256": sha256_bytes(raw),
                    },
                    "verification": verification,
                    "idempotency_key": (
                        f"signal_journey_rejected:{journey_id}:{sha256_bytes(raw)}"
                    ),
                    "artifact_authority_state_v1": authority_state(),
                }
            )
            continue
        events.append(
            {
                "schema": "signal_spine_domain_event_v1",
                "schema_version": 1,
                "event_type": "signal_journey_recorded",
                "aggregate_type": "causal_signal_journey",
                "aggregate_id": journey_id,
                "journey_id": journey_id,
                "source_receipt": {
                    "kind": "owner_only_signal_journey_receipt",
                    "relative_path": str(path.relative_to(workspace)),
                    "sha256": sha256_bytes(raw),
                },
                "verification": verification,
                "journey": value,
                "idempotency_key": f"signal_journey:{journey_id}:{sha256_bytes(raw)}",
                "artifact_authority_state_v1": authority_state(),
            }
        )
    for path in sorted((root / "temporal_associations").glob("*.json")):
        raw = path.read_bytes()
        value = load_json(path)
        if value is None or value.get("schema") != "signal_temporal_association_v1":
            continue
        journey_id = str(value.get("journey_id") or path.stem)
        verification = verify_temporal_association(value)
        if not verification["valid"]:
            events.append(
                {
                    "schema": "signal_spine_domain_event_v1",
                    "schema_version": 1,
                    "event_type": "signal_temporal_association_rejected_tamper",
                    "aggregate_type": "causal_signal_journey",
                    "aggregate_id": journey_id,
                    "journey_id": journey_id,
                    "source_receipt": {
                        "kind": "rejected_temporal_association_receipt",
                        "relative_path": str(path.relative_to(workspace)),
                        "sha256": sha256_bytes(raw),
                    },
                    "verification": verification,
                    "idempotency_key": (
                        "signal_temporal_association_rejected:"
                        f"{journey_id}:{sha256_bytes(raw)}"
                    ),
                    "artifact_authority_state_v1": authority_state(),
                }
            )
            continue
        events.append(
            {
                "schema": "signal_spine_domain_event_v1",
                "schema_version": 1,
                "event_type": "signal_temporal_association_recorded",
                "aggregate_type": "causal_signal_journey",
                "aggregate_id": journey_id,
                "journey_id": journey_id,
                "source_receipt": {
                    "kind": "owner_only_temporal_association_receipt",
                    "relative_path": str(path.relative_to(workspace)),
                    "sha256": sha256_bytes(raw),
                },
                "verification": verification,
                "temporal_association": value,
                "idempotency_key": (
                    f"signal_temporal_association:{journey_id}:{sha256_bytes(raw)}"
                ),
                "artifact_authority_state_v1": authority_state(),
            }
        )
    for path in sorted((root / "capture_gaps").glob("*.json")):
        raw = path.read_bytes()
        value = load_json(path)
        if value is None or value.get("schema") != "signal_capture_gap_v1":
            continue
        journey_id = str(value.get("journey_id") or path.stem)
        events.append(
            {
                "schema": "signal_spine_domain_event_v1",
                "schema_version": 1,
                "event_type": "signal_capture_gap_recorded",
                "aggregate_type": "causal_signal_journey",
                "aggregate_id": journey_id,
                "journey_id": journey_id,
                "source_receipt": {
                    "kind": "owner_only_capture_gap_receipt",
                    "relative_path": str(path.relative_to(workspace)),
                    "sha256": sha256_bytes(raw),
                },
                "capture_gap": value,
                "idempotency_key": (
                    f"signal_capture_gap:{journey_id}:{sha256_bytes(raw)}"
                ),
                "artifact_authority_state_v1": authority_state(),
            }
        )
    return events


def project(events: list[dict[str, Any]]) -> dict[str, Any]:
    journeys: dict[str, dict[str, Any]] = {}
    associations: list[dict[str, Any]] = []
    capture_gaps: list[dict[str, Any]] = []
    rejected: dict[str, dict[str, Any]] = {}
    rejected_associations: dict[str, dict[str, Any]] = {}
    for event in events:
        journey_id = str(event.get("journey_id") or "")
        if not journey_id:
            continue
        if event.get("event_type") == "signal_journey_recorded":
            journey = event.get("journey")
            if isinstance(journey, dict):
                journeys[journey_id] = journey
        elif event.get("event_type") == "signal_temporal_association_recorded":
            association = event.get("temporal_association")
            if isinstance(association, dict):
                associations.append(association)
        elif event.get("event_type") == "signal_journey_rejected_tamper":
            rejected[journey_id] = event
        elif (
            event.get("event_type")
            == "signal_temporal_association_rejected_tamper"
        ):
            rejected_associations[
                str(event.get("source_receipt", {}).get("sha256") or journey_id)
            ] = event
        elif event.get("event_type") == "signal_capture_gap_recorded":
            gap = event.get("capture_gap")
            if isinstance(gap, dict):
                capture_gaps.append(gap)
    complete = 0
    lineage_mismatches = 0
    parity_mismatches = 0
    blocked = 0
    dispatch_failed = 0
    stage_count = 0
    for journey in journeys.values():
        receipts = journey.get("receipts")
        receipts = receipts if isinstance(receipts, list) else []
        kinds = {
            str(receipt.get("stage_kind"))
            for receipt in receipts
            if isinstance(receipt, dict)
        }
        stage_count += len(receipts)
        if journey.get("lineage_valid") is not True:
            lineage_mismatches += 1
        parity_mismatches += int(journey.get("parity_mismatch_count") or 0)
        if "blocked" in kinds:
            blocked += 1
        dispatch_succeeded = any(
            isinstance(receipt, dict)
            and receipt.get("stage_kind") == "dispatched"
            and receipt.get("effect") == "dispatched"
            for receipt in receipts
        )
        dispatch_failed += int(
            any(
                isinstance(receipt, dict)
                and receipt.get("stage_kind") == "dispatched"
                and receipt.get("effect") == "dispatch_failed"
                for receipt in receipts
            )
        )
        delivery_evidence = any(
            isinstance(receipt, dict)
            and receipt.get("stage_kind") == "delivery_evidence"
            and receipt.get("effect") == "evidence_recorded"
            for receipt in receipts
        )
        blocked_terminal = any(
            isinstance(receipt, dict)
            and receipt.get("stage_kind") == "blocked"
            and receipt.get("effect") == "blocked"
            for receipt in receipts
        )
        required = {
            "authored",
            "chunked",
            "encoded",
            "narrative",
            "feedback",
            "breathing",
            "resonance",
            "visual",
            "delta",
            "hebbian",
            "friction_review",
            "safety_review",
        }
        if required.issubset(kinds) and (
            blocked_terminal or (dispatch_succeeded and delivery_evidence)
        ):
            complete += 1
    return {
        "schema": "signal_spine_projection_v1",
        "schema_version": 1,
        "mode": "shadow",
        "projection_cutover": False,
        "journey_count": len(journeys),
        "complete_journey_count": complete,
        "stage_count": stage_count,
        "temporal_association_count": len(associations),
        "blocked_journey_count": blocked,
        "dispatch_failed_journey_count": dispatch_failed,
        "lineage_mismatch_count": lineage_mismatches,
        "parity_mismatch_count": parity_mismatches,
        "tampered_journey_count": len(rejected),
        "tampered_association_count": len(rejected_associations),
        "capture_gap_count": len(capture_gaps),
        "capture_sufficient": not capture_gaps,
        "zero_mismatch": (
            lineage_mismatches == 0
            and parity_mismatches == 0
            and not rejected
            and not rejected_associations
        ),
        "sensory_protocol_changed": False,
        "journey_id_on_wire": False,
        "temporal_association_is_not_direct_causation": True,
        "artifact_authority_state_v1": authority_state(),
        "journey_ids": sorted(journeys),
    }


def render_report(status: dict[str, Any]) -> str:
    return "\n".join(
        [
            "# Causal Signal Spine Shadow",
            "",
            f"- Journeys: {status['journey_count']}",
            f"- Complete journeys: {status['complete_journey_count']}",
            f"- Stages: {status['stage_count']}",
            f"- Later telemetry associations: {status['temporal_association_count']}",
            f"- Blocked journeys: {status['blocked_journey_count']}",
            f"- Dispatch-failed journeys: {status['dispatch_failed_journey_count']}",
            f"- Lineage mismatches: {status['lineage_mismatch_count']}",
            f"- Parity mismatches: {status['parity_mismatch_count']}",
            f"- Rejected tampered journeys: {status['tampered_journey_count']}",
            f"- Rejected tampered associations: {status['tampered_association_count']}",
            f"- Capture gaps: {status['capture_gap_count']}",
            "- Wire journey IDs: none",
            "- Telemetry relation: temporal association, not direct causation",
            "- Authority: evidence only",
            "",
        ]
    )


def atomic_write_text(path: Path, payload: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    handle = tempfile.NamedTemporaryFile(
        "w",
        encoding="utf-8",
        dir=path.parent,
        prefix=f".{path.name}.",
        delete=False,
    )
    temporary = Path(handle.name)
    try:
        with handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        directory_fd = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    finally:
        temporary.unlink(missing_ok=True)


def write_projection(workspace: Path, status: dict[str, Any]) -> dict[str, str]:
    root = state_dir(workspace)
    root.mkdir(parents=True, exist_ok=True)
    status_path = root / "projection_status.json"
    report_path = root / "report.md"
    status_payload = json.dumps(status, indent=2, sort_keys=True) + "\n"
    report_payload = render_report(status)
    atomic_write_text(status_path, status_payload)
    atomic_write_text(report_path, report_payload)
    return {
        "projection_status.json": sha256_bytes(status_payload.encode()),
        "report.md": sha256_bytes(report_payload.encode()),
    }


def run(workspace: Path, *, write: bool) -> dict[str, Any]:
    source = source_events(workspace)
    directory = state_dir(workspace)
    if write and source:
        append_domain_events(directory, "signal_spine", source)
    if write:
        events, corrupt = read_domain_events(directory, "signal_spine")
        if corrupt:
            raise RuntimeError(f"signal spine stream has {corrupt} corrupt events")
    else:
        events = source
    status = project(events)
    status["source_event_count"] = len(source)
    if write:
        hashes = write_projection(workspace, status)
        root = workspace / "diagnostics/evidence_event_store_v2"
        EvidenceEventStore(root).write_checkpoint(
            "signal_spine_v1", PROJECTOR_VERSION, hashes
        )
        status["projection_hashes"] = hashes
    return status


class SignalSpineProjectorTests(unittest.TestCase):
    def test_canonical_hash_matches_rust_fixture(self) -> None:
        fixture = {
            "zeta": {"s": "line\n", "n": 7},
            "alpha": [1, True, None, "é"],
        }
        self.assertEqual(
            sha256_bytes(canonical_json(fixture).encode()),
            "118fe7607c342d93dbffa5bcd0d0410cec4c6e7e39088935c075919d96aae129",
        )

    def test_projection_counts_complete_and_mismatch_states(self) -> None:
        receipts = [
            {
                "stage_kind": kind,
                "effect": (
                    "dispatched"
                    if kind == "dispatched"
                    else "evidence_recorded"
                    if kind == "delivery_evidence"
                    else "produced"
                ),
            }
            for kind in (
                "authored",
                "chunked",
                "encoded",
                "narrative",
                "feedback",
                "breathing",
                "resonance",
                "visual",
                "delta",
                "hebbian",
                "friction_review",
                "safety_review",
                "dispatched",
                "delivery_evidence",
            )
        ]
        status = project(
            [
                {
                    "event_type": "signal_journey_recorded",
                    "journey_id": "journey_one",
                    "journey": {
                        "receipts": receipts,
                        "lineage_valid": True,
                        "parity_mismatch_count": 0,
                    },
                },
                {
                    "event_type": "signal_temporal_association_recorded",
                    "journey_id": "journey_one",
                    "temporal_association": {
                        "relation": "temporal_association_not_direct_causation"
                    },
                },
            ]
        )
        self.assertEqual(status["complete_journey_count"], 1)
        self.assertEqual(status["temporal_association_count"], 1)
        self.assertTrue(status["zero_mismatch"])
        self.assertTrue(status["capture_sufficient"])
        self.assertFalse(status["journey_id_on_wire"])

    def test_dispatch_failure_is_not_counted_as_complete(self) -> None:
        receipts = [
            {"stage_kind": kind, "effect": "produced"}
            for kind in (
                "authored",
                "chunked",
                "encoded",
                "narrative",
                "feedback",
                "breathing",
                "resonance",
                "visual",
                "delta",
                "hebbian",
                "friction_review",
                "safety_review",
            )
        ]
        receipts.append(
            {"stage_kind": "dispatched", "effect": "dispatch_failed"}
        )
        status = project(
            [
                {
                    "event_type": "signal_journey_recorded",
                    "journey_id": "journey_failed",
                    "journey": {
                        "receipts": receipts,
                        "lineage_valid": True,
                        "parity_mismatch_count": 0,
                    },
                }
            ]
        )
        self.assertEqual(status["complete_journey_count"], 0)
        self.assertEqual(status["dispatch_failed_journey_count"], 1)

    def test_capture_gap_marks_dossier_insufficient_without_claiming_lineage_failure(
        self,
    ) -> None:
        status = project(
            [
                {
                    "event_type": "signal_capture_gap_recorded",
                    "journey_id": "journey_one",
                    "capture_gap": {
                        "reason": "asynchronous_fixture_write_failed",
                        "dossier_sufficient": False,
                    },
                }
            ]
        )
        self.assertEqual(status["capture_gap_count"], 1)
        self.assertFalse(status["capture_sufficient"])
        self.assertTrue(status["zero_mismatch"])

    def test_source_events_never_ingest_vector_fixtures(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            root = state_dir(workspace)
            (root / "journeys").mkdir(parents=True)
            (root / "fixtures").mkdir()
            (root / "journeys/journey_one.json").write_text(
                json.dumps(
                    {
                        "schema": "causal_signal_journey_v1",
                        "journey_id": "journey_one",
                        "receipts": [],
                    }
                ),
                encoding="utf-8",
            )
            (root / "fixtures/secret.json").write_text(
                json.dumps({"vector": [1.0] * 48}), encoding="utf-8"
            )
            events = source_events(workspace)
            self.assertEqual(len(events), 1)
            self.assertNotIn('"vector"', canonical_json(events[0]))

    def test_receipt_tampering_is_rejected_before_ingestion(self) -> None:
        receipt = {
            "schema": "signal_stage_receipt_v1",
            "schema_version": 1,
            "journey_id": "journey_one",
            "stage_id": "stage_one",
            "stage_index": 0,
            "stage_kind": "authored",
            "relation": "root",
            "effect": "produced",
            "ownership_domain": "astrid_authored",
            "parent_stage_ids": [],
            "source_sha256": "a" * 64,
            "output_sha256": "a" * 64,
            "provenance_ref_v1": {
                "origin": "astrid_interpretation",
                "source_id": "stage_one",
                "canonical_sha256": "a" * 64,
                "parent_ids": [],
                "field_paths": ["signal_spine.authored"],
            },
            "process_identity_v1": {},
            "temporal_envelope_v1": {
                "arrival_time_unix_ms": 10,
                "stage_time_unix_ms": 10,
                "monotonic_time_ns": 1,
            },
            "measurements": {},
            "capture_fixture_ref_v1": None,
            "raw_response_prose_included": False,
            "live_control_authority": False,
        }
        receipt["receipt_integrity_sha256"] = sha256_bytes(
            canonical_json(
                {
                    key: value
                    for key, value in receipt.items()
                    if key != "receipt_integrity_sha256"
                }
            ).encode()
        )
        journey = {
            "journey_id": "journey_one",
            "receipts": [receipt],
            "stage_count": 1,
            "lineage_valid": True,
            "raw_response_prose_included": False,
            "live_control_authority": False,
        }
        self.assertTrue(verify_journey(journey)["valid"])
        receipt["capture_fixture_ref_v1"] = {
            "capture_window_id": "capture_test",
            "fixture_sha256": "f" * 64,
            "relative_path": "captures/capture_test/fixtures/test.json",
            "vector_dimensions": 48,
        }
        capture_verification = verify_journey(journey)
        self.assertFalse(capture_verification["valid"])
        self.assertIn(
            "stage_0:receipt_integrity_mismatch",
            capture_verification["errors"],
        )
        receipt["capture_fixture_ref_v1"] = None
        receipt["output_sha256"] = "b" * 64
        verification = verify_journey(journey)
        self.assertFalse(verification["valid"])
        self.assertIn("stage_0:receipt_integrity_mismatch", verification["errors"])

    def test_temporal_association_tampering_is_rejected(self) -> None:
        receipt = {
            "journey_id": "journey_one",
            "stage_id": "stage_association",
            "stage_kind": "minime_telemetry_window",
            "relation": "temporal_association",
            "effect": "temporally_associated",
            "ownership_domain": "minime_observed",
            "parent_stage_ids": ["stage_dispatched"],
            "output_sha256": "b" * 64,
            "provenance_ref_v1": {
                "origin": "minime_observation",
                "source_id": "stage_association",
                "canonical_sha256": "b" * 64,
                "parent_ids": ["stage_dispatched", "observation_one"],
                "field_paths": ["signal_spine.minime_telemetry_window"],
            },
            "raw_response_prose_included": False,
            "live_control_authority": False,
        }
        receipt["receipt_integrity_sha256"] = sha256_bytes(
            canonical_json(receipt).encode()
        )
        association = {
            "journey_id": "journey_one",
            "relation": "temporal_association_not_direct_causation",
            "direct_causation_claimed": False,
            "wire_acknowledgement_available": False,
            "raw_response_prose_included": False,
            "live_control_authority": False,
            "receipt": receipt,
        }
        self.assertTrue(verify_temporal_association(association)["valid"])
        association["direct_causation_claimed"] = True
        verification = verify_temporal_association(association)
        self.assertFalse(verification["valid"])
        self.assertIn("direct_causation_marker", verification["errors"])


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    commands = parser.add_subparsers(dest="command")
    generate = commands.add_parser("generate")
    generate.add_argument("--write", action="store_true")
    commands.add_parser("report")
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(
            SignalSpineProjectorTests
        )
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    if args.command not in {"generate", "report"}:
        parser.print_help()
        return 2
    status = run(
        args.workspace.resolve(),
        write=args.command == "generate" and bool(args.write),
    )
    print(json.dumps(status, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
