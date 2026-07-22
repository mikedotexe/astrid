"""Register representation contracts and project mechanical transition loss."""

from __future__ import annotations

import json
from collections import Counter
from pathlib import Path
from typing import Any

try:
    from experiential_systems.common import (
        RecordValidationError, authority_state, event_payload, owner_atomic_write,
        owner_atomic_write_json, owner_atomic_write_jsonl, project_events,
        sha256_bytes, stream_payloads,
    )
    from projection_cursors import ProjectionInputCursor
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError, authority_state, event_payload, owner_atomic_write,
        owner_atomic_write_json, owner_atomic_write_jsonl, project_events,
        sha256_bytes, stream_payloads,
    )
    from scripts.projection_cursors import ProjectionInputCursor

from .model import (
    ModelTransitionReceiptV1, RepresentationContractV1,
    RepresentationLossReceiptV1, RepresentationTransitionV1,
    build_contract, build_transition,
)

STREAM = "representation_contracts"
SCHEMA = "representation_contract_domain_event_v1"


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/representation_contracts_v1"


def _record_id(record: dict[str, Any]) -> str:
    schema = record.get("schema")
    field = {
        "representation_contract_v1": "contract_id",
        "representation_transition_v1": "transition_id",
        "representation_loss_receipt_v1": "loss_receipt_id",
        "model_transition_receipt_v1": "receipt_id",
    }.get(schema)
    if field is None or not record.get(field):
        raise RecordValidationError("representation record lacks a canonical identity")
    return str(record[field])


def _source(root: Path, relative: str) -> tuple[str, str]:
    path = root / relative
    return f"repo:astrid/{relative}", sha256_bytes(path.read_bytes())


def registry(root: Path) -> list[RepresentationContractV1]:
    codec_ref, codec_hash = _source(root, "capsules/spectral-bridge/src/codec/projection.rs")
    evidence_ref, evidence_hash = _source(root, "capsules/spectral-bridge/src/codec/evidence_types.rs")
    shadow_ref, shadow_hash = _source(root, "capsules/spectral-bridge/src/astrid_shadow.rs")
    transport_ref, transport_hash = _source(root, "capsules/spectral-bridge/src/llm/provider/transport.rs")
    dialogue_ref, dialogue_hash = _source(root, "capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs")
    witness_ref, witness_hash = _source(root, "capsules/spectral-bridge/src/lived_state_witness/types.rs")
    specs = (
        ("semantic_codec_48d", "vector", 48, (), (codec_ref,), (codec_hash,)),
        ("compatibility_shadow_32d", "vector", 32, (), (shadow_ref,), (shadow_hash,)),
        ("additive_glimpse_12d", "vector", 12, (), (evidence_ref,), (evidence_hash,)),
        ("multi_scale_residual_36d", "vector", 36, (), (evidence_ref,), (evidence_hash,)),
        ("prompt_context_unpacked", "prompt_context", None, ("block_label", "original_chars"), (dialogue_ref,), (dialogue_hash,)),
        ("prompt_context_packed", "prompt_context", None, ("block_label", "kept_chars", "removed_chars"), (dialogue_ref,), (dialogue_hash,)),
        ("provider_route", "provider_route", None, ("provider_route",), (transport_ref,), (transport_hash,)),
        ("provider_fallback", "field_set", None, ("fallback_reason", "fallback_route"), (transport_ref,), (transport_hash,)),
        ("repair_ancestry", "field_set", None, ("repair_parent_call_id", "response_sha256"), (witness_ref,), (witness_hash,)),
        ("model_profile", "model_profile", None, ("model_profile",), (witness_ref,), (witness_hash,)),
        ("response_artifact", "response_artifact", None, ("response_sha256",), (witness_ref,), (witness_hash,)),
    )
    return [
        build_contract(name=name, representation_kind=kind, dimension_count=count,
                       field_names=fields, source_refs=refs, source_hashes=hashes)
        for name, kind, count, fields, refs, hashes in specs
    ]


def _static_transitions(contracts: dict[str, RepresentationContractV1]) -> list[RepresentationTransitionV1]:
    by_name = {contract.name: contract for contract in contracts.values()}
    source_hash = by_name["semantic_codec_48d"].source_hashes[0]
    return [
        build_transition(
            transition_kind="projection", source_contract_id=by_name["semantic_codec_48d"].contract_id,
            output_contract_id=by_name["compatibility_shadow_32d"].contract_id,
            source_sha256=source_hash, output_sha256=by_name["compatibility_shadow_32d"].source_hashes[0],
            retained_dimensions=tuple(range(32)), dropped_dimensions=tuple(range(32, 48)),
            retained_fields=(), dropped_fields=(), aggregation="prefix_projection_exact_dims_0_through_31",
            truncation_count=0, timing_ms=None, source_event_id="source:astrid_shadow.observe",
        ),
        build_transition(
            transition_kind="aggregation", source_contract_id=by_name["semantic_codec_48d"].contract_id,
            output_contract_id=by_name["additive_glimpse_12d"].contract_id,
            source_sha256=source_hash, output_sha256=by_name["additive_glimpse_12d"].source_hashes[0],
            retained_dimensions=(17, 24, 25, 26, 27, 31, 40),
            dropped_dimensions=tuple(index for index in range(48) if index not in {17, 24, 25, 26, 27, 31, 40}),
            retained_fields=(), dropped_fields=(), aggregation="deterministic_mean_abs_and_named_anchor_projection",
            truncation_count=0, timing_ms=None, source_event_id="source:GlimpseCodec.derive_12d",
        ),
    ]


def _packing_transitions(workspace: Path, contracts: dict[str, RepresentationContractV1], *, write: bool) -> tuple[list[RepresentationTransitionV1], dict[str, Any]]:
    path = workspace / "diagnostics/context_packing_pressure_v1.jsonl"
    cursor = ProjectionInputCursor(state_dir(workspace) / "packing_cursor_v1.json", STREAM) if write else None
    if cursor is not None:
        rows, next_cursor = cursor.jsonl_tail(path, key="context_packing")
    else:
        raw = path.read_bytes() if path.is_file() else b""
        rows = list(enumerate(raw.decode("utf-8").splitlines(), 1))
        next_cursor = {"source_sha256": sha256_bytes(raw)}
    by_name = {contract.name: contract for contract in contracts.values()}
    result: list[RepresentationTransitionV1] = []
    for line_number, raw in rows:
        if not raw.strip():
            continue
        value = json.loads(raw)
        blocks = value.get("blocks") if isinstance(value.get("blocks"), list) else []
        retained = tuple(str(block.get("label")) for block in blocks if isinstance(block, dict) and int(block.get("kept_chars") or 0) > 0)
        dropped = tuple(str(block.get("label")) for block in blocks if isinstance(block, dict) and bool(block.get("fully_removed")))
        removed = sum(int(block.get("removed_chars") or 0) for block in blocks if isinstance(block, dict))
        source_hash = sha256_bytes(raw.encode())
        output_hash = sha256_bytes(json.dumps({"total_after": value.get("total_after"), "retained": retained}, sort_keys=True).encode())
        result.append(build_transition(
            transition_kind="packing", source_contract_id=by_name["prompt_context_unpacked"].contract_id,
            output_contract_id=by_name["prompt_context_packed"].contract_id,
            source_sha256=source_hash, output_sha256=output_hash,
            retained_dimensions=(), dropped_dimensions=(), retained_fields=retained, dropped_fields=dropped,
            aggregation="priority_ordered_character_budget", truncation_count=removed,
            timing_ms=None, source_event_id=f"packing:{value.get('ts') or line_number}",
        ))
    if write and not result and path.is_file() and cursor is not None:
        cursor.commit_jsonl({"context_packing": next_cursor})
    return result, next_cursor


def _model_transitions(workspace: Path, *, write: bool) -> tuple[list[ModelTransitionReceiptV1], dict[str, dict[str, Any]], list[Path]]:
    root = workspace / "introspections/lived_state_witnesses/witnesses"
    paths = sorted(root.glob("*.json"))
    cursor = ProjectionInputCursor(state_dir(workspace) / "witness_cursor_v1.json", STREAM) if write else None
    if cursor is not None:
        changed, manifest, removed = cursor.changed_files(paths, root=root)
    else:
        changed, removed = paths, []
        manifest = {path.name: {"sha256": sha256_bytes(path.read_bytes()), "size": path.stat().st_size, "present": True} for path in paths}
    if removed:
        raise RecordValidationError("witness sidecars are append-only and cannot disappear")
    result: list[ModelTransitionReceiptV1] = []
    for path in changed:
        witness = json.loads(path.read_text(encoding="utf-8"))
        witness_id = str(witness.get("witness_id") or "")
        for route in witness.get("model_routes_v1") or []:
            if not isinstance(route, dict):
                continue
            result.append(ModelTransitionReceiptV1.build(
                request_identity_sha256=str(route.get("request_content_anchor_sha256") or route.get("qos_request_identity_sha256") or ""),
                response_sha256=str(route.get("response_sha256") or ""),
                provider_route=str(route.get("provider_route") or "unknown"),
                model_profile=str(route.get("model_profile") or "unknown"),
                repair_parent_call_id=route.get("repair_parent_call_id"),
                fallback_reason=route.get("fallback_reason"),
                timing_ms=int(route.get("duration_ms") or 0), source_witness_id=witness_id,
            ))
    return result, manifest, paths


def _all_records(workspace: Path) -> tuple[list[dict[str, Any]], int]:
    payloads, corrupt = stream_payloads(workspace, STREAM)
    records = [dict(item["record"]) for item in payloads if isinstance(item.get("record"), dict)]
    records.sort(key=lambda item: (item.get("schema", ""), _record_id(item)))
    return records, corrupt


def project(workspace: Path, *, write: bool) -> dict[str, Any]:
    root = Path(__file__).resolve().parents[2]
    contracts_list = registry(root)
    contracts = {item.contract_id: item for item in contracts_list}
    static = _static_transitions(contracts)
    packing, packing_cursor = _packing_transitions(workspace, contracts, write=write)
    model, witness_manifest, _ = _model_transitions(workspace, write=write)
    transitions = static + packing
    if write:
        events_path = (
            workspace / "diagnostics/evidence_event_store_v2/events.jsonl"
        )
        if events_path.is_file():
            existing_records, existing_corrupt = _all_records(workspace)
        else:
            existing_records, existing_corrupt = [], 0
        if existing_corrupt:
            raise RecordValidationError("representation stream is corrupt")
        transition_history = {
            item["transition_id"]: RepresentationTransitionV1.from_untrusted(item)
            for item in existing_records
            if item.get("schema") == "representation_transition_v1"
        }
        transition_history.update({item.transition_id: item for item in transitions})
        existing_loss_ids = {
            str(item["loss_receipt_id"])
            for item in existing_records
            if item.get("schema") == "representation_loss_receipt_v1"
        }
        losses = [
            loss
            for transition in transition_history.values()
            if (
                loss := RepresentationLossReceiptV1.from_transition(transition)
            ).loss_receipt_id not in existing_loss_ids
        ]
    else:
        losses = [RepresentationLossReceiptV1.from_transition(item) for item in transitions]
    records = [item.to_dict() for item in contracts_list]
    records += [item.to_dict() for item in transitions]
    records += [item.to_dict() for item in losses]
    records += [item.to_dict() for item in model]
    payloads = []
    for record in records:
        identifier = _record_id(record)
        payloads.append(event_payload(
            schema=SCHEMA, event_type=f"{record['schema']}_recorded",
            aggregate_type="representation_contract", aggregate_id=identifier,
            idempotency_key=f"{STREAM}:{identifier}", record=record,
        ))
    appended = project_events(
        workspace, STREAM, payloads, actor="representation-contract-projector",
        source_kind="source_truth_and_shadow_receipt_projection",
        source_locator_value="repo:astrid/representation-contracts",
    ) if write else 0
    if write:
        all_records, corrupt = _all_records(workspace)
    else:
        all_records, corrupt = records, 0
    counts = Counter(item.get("schema") for item in all_records)
    status = {
        "schema": "representation_contract_projection_status_v1", "schema_version": 1,
        "valid": corrupt == 0, "write": write, "record_count": len(all_records),
        "record_counts": dict(sorted(counts.items())), "appended_event_count": appended,
        "felt_loss_scored": False, "contradiction_inferred": False,
        "sensory_json_changed": False, "vector_transport_changed": False,
        "provider_behavior_changed": False, "raw_payload_included": False,
        "counter_audit": {"status": "consistent" if corrupt == 0 else "inconsistent",
                          "checks": {"stream_not_corrupt": corrupt == 0,
                                     "contract_ids_unique": len(contracts) == len(contracts_list),
                                     "mechanical_loss_only": all(item.get("felt_loss_scored") in {False, None} for item in all_records)}},
        "artifact_authority_state_v1": authority_state(),
    }
    if write and status["valid"]:
        output = state_dir(workspace)
        owner_atomic_write_jsonl(output / "registry.jsonl", [item for item in all_records if item.get("schema") == "representation_contract_v1"])
        owner_atomic_write_jsonl(output / "transitions.jsonl", [item for item in all_records if item.get("schema") != "representation_contract_v1"])
        owner_atomic_write_json(output / "status.json", status)
        owner_atomic_write(output / "report.md", "# Representation And Model Transitions\n\nMechanical contracts and loss receipts only. No felt-loss score or contradiction is inferred.\n\n" + "\n".join(f"- {key}: {value}" for key, value in sorted(counts.items())) + "\n")
        ProjectionInputCursor(state_dir(workspace) / "packing_cursor_v1.json", STREAM).commit_jsonl({"context_packing": packing_cursor})
        ProjectionInputCursor(state_dir(workspace) / "witness_cursor_v1.json", STREAM).commit_files(witness_manifest)
    return status


def verify(workspace: Path) -> dict[str, Any]:
    errors: list[str] = []
    for filename in ("registry.jsonl", "transitions.jsonl"):
        path = state_dir(workspace) / filename
        if not path.is_file():
            errors.append(f"{filename}:missing")
            continue
        for line_number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            try:
                value = json.loads(raw)
                schema = value.get("schema")
                if schema == "representation_contract_v1": RepresentationContractV1.from_untrusted(value)
                elif schema == "representation_transition_v1": RepresentationTransitionV1.from_untrusted(value)
                elif schema == "model_transition_receipt_v1": ModelTransitionReceiptV1.from_untrusted(value)
                elif schema == "representation_loss_receipt_v1": RepresentationLossReceiptV1.from_untrusted(value)
                else: raise RecordValidationError("unknown record schema")
            except (json.JSONDecodeError, RecordValidationError, ValueError) as error:
                errors.append(f"{filename}:{line_number}:{error}")
    return {"schema": "representation_contract_verification_v1", "schema_version": 1,
            "valid": not errors, "errors": errors, "artifact_authority_state_v1": authority_state()}


def query(workspace: Path, identifier: str | None = None) -> list[dict[str, Any]]:
    records, _ = _all_records(workspace)
    if not identifier: return records
    return [item for item in records if identifier in {item.get("contract_id"), item.get("transition_id"), item.get("receipt_id"), item.get("loss_receipt_id"), item.get("source_contract_id"), item.get("output_contract_id")}]


def deterministic_diff(workspace: Path, left_id: str, right_id: str) -> dict[str, Any]:
    records, corrupt = _all_records(workspace)
    by_id = {_record_id(item): item for item in records}
    if corrupt:
        raise RecordValidationError("representation stream is corrupt")
    if left_id not in by_id or right_id not in by_id:
        raise RecordValidationError("both diff record IDs must exist")
    left = by_id[left_id]
    right = by_id[right_id]
    changed = []
    for field in sorted(set(left).union(right)):
        if left.get(field) == right.get(field):
            continue
        changed.append({
            "field": field,
            "left_value_sha256": sha256_bytes(
                json.dumps(left.get(field), sort_keys=True).encode()
            ),
            "right_value_sha256": sha256_bytes(
                json.dumps(right.get(field), sort_keys=True).encode()
            ),
        })
    return {
        "schema": "representation_contract_diff_v1",
        "schema_version": 1,
        "left_id": left_id,
        "right_id": right_id,
        "left_record_sha256": sha256_bytes(
            json.dumps(left, sort_keys=True).encode()
        ),
        "right_record_sha256": sha256_bytes(
            json.dumps(right, sort_keys=True).encode()
        ),
        "changed_fields": changed,
        "felt_loss_scored": False,
        "artifact_authority_state_v1": authority_state(),
    }
