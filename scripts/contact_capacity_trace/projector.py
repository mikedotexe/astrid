"""Project exact contact, ingress-gap, and non-contact journey lineage."""

from __future__ import annotations

from collections import Counter
import hashlib
import json
from pathlib import Path
from typing import Any

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

CONTACT_SCHEMA = "contact_input_receipt_v1"
INGRESS_GAP_SCHEMA = "contact_ingress_gap_v1"
ORIGIN_KINDS = {"contact", "ingress_gap", "autonomous", "operator", "legacy_unknown"}
RESPONSE_ORIGINS = {
    "model_authored",
    "static_fallback",
    "mirror",
    "self_study",
    "introspection",
    "witness",
    "other",
}
FORBIDDEN_KEYS = {
    "body",
    "content",
    "message",
    "prompt",
    "prose",
    "response",
    "text",
    "vector",
    "embedding",
    "features",
}


def state_dir(workspace: Path) -> Path:
    """V1 input and compatibility-output directory."""

    return workspace / "diagnostics/contact_capacity_trace_v1"


def projection_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/contact_capacity_trace_v2"


def _sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _canonical_sha256(value: Any) -> str:
    return _sha256_bytes(canonical_json(value).encode())


def _is_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value)
    )


def _is_bounded_id(value: Any, prefix: str) -> bool:
    if not isinstance(value, str) or not value.startswith(prefix):
        return False
    suffix = value.removeprefix(prefix)
    return len(suffix) == 24 and all(character in "0123456789abcdef" for character in suffix)


def _contains_forbidden_payload(value: Any) -> bool:
    if isinstance(value, list):
        return any(_contains_forbidden_payload(item) for item in value)
    if not isinstance(value, dict):
        return False
    return any(
        str(key).lower() in FORBIDDEN_KEYS or _contains_forbidden_payload(item)
        for key, item in value.items()
    )


def _validate_source_refs(value: Any, source_count: Any) -> list[str]:
    errors: list[str] = []
    if not isinstance(value, list) or not value:
        return ["source_refs_invalid"]
    if isinstance(source_count, bool) or not isinstance(source_count, int):
        errors.append("source_count_invalid")
    elif source_count != len(value):
        errors.append("source_count_mismatch")
    for index, source in enumerate(value):
        if not isinstance(source, dict):
            errors.append(f"source_ref_{index}_not_object")
            continue
        for field in ("source_ref_id_sha256", "source_bytes_sha256"):
            if not _is_sha256(source.get(field)):
                errors.append(f"source_ref_{index}_{field}_invalid")
        for field in ("source_body_sha256", "principal_identity_sha256", "thread_id_sha256"):
            raw = source.get(field)
            if raw is not None and not _is_sha256(raw):
                errors.append(f"source_ref_{index}_{field}_invalid")
        provenance_state = source.get("provenance_state")
        if provenance_state not in {"provenance_backed", "unverified"}:
            errors.append(f"source_ref_{index}_provenance_state_invalid")
        locally_validated = source.get("local_provenance_validated")
        if not isinstance(locally_validated, bool):
            errors.append(f"source_ref_{index}_local_validation_invalid")
        if source.get("cryptographic_sender_authenticated") is not False:
            errors.append(f"source_ref_{index}_sender_authentication_marker")
        if provenance_state == "provenance_backed" and locally_validated is not True:
            errors.append(f"source_ref_{index}_provenance_inconsistent")
        if provenance_state == "unverified" and locally_validated is not False:
            errors.append(f"source_ref_{index}_unverified_inconsistent")
    return errors


def _validate_common_receipt(value: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    for field in (
        "input_identity_sha256",
        "process_identity_sha256",
        "admitted_context_sha256",
    ):
        if not _is_sha256(value.get(field)):
            errors.append(f"{field}_invalid")
    for field in ("cutoff_unix_ms", "received_at_unix_ms", "received_monotonic_ns"):
        raw = value.get(field)
        if isinstance(raw, bool) or not isinstance(raw, int) or raw < 0:
            errors.append(f"{field}_invalid")
    if not isinstance(value.get("clock_scope_id"), str) or not value["clock_scope_id"].strip():
        errors.append("clock_scope_id_missing")
    errors.extend(_validate_source_refs(value.get("source_refs"), value.get("source_count")))
    if value.get("local_provenance_validation_only") is not True:
        errors.append("local_provenance_marker")
    if value.get("cryptographic_sender_authentication") is not False:
        errors.append("cryptographic_sender_authentication_marker")
    if value.get("raw_input_included") is not False:
        errors.append("raw_input_marker")
    if value.get("live_control_authority") is not False:
        errors.append("live_control_authority_marker")
    if _contains_forbidden_payload(value):
        errors.append("forbidden_raw_payload")
    core = dict(value)
    recorded = str(core.pop("receipt_sha256", ""))
    if recorded != _canonical_sha256(core):
        errors.append("receipt_sha256_mismatch")
    return errors


def validate_contact_receipt(value: Any) -> list[str]:
    if not isinstance(value, dict):
        return ["receipt_not_object"]
    errors = _validate_common_receipt(value)
    if value.get("schema") != CONTACT_SCHEMA or value.get("schema_version") != 1:
        errors.append("schema_mismatch")
    if not _is_bounded_id(value.get("contact_id"), "contact_"):
        errors.append("contact_id_invalid")
    if not _is_bounded_id(value.get("journey_id"), "journey_"):
        errors.append("journey_id_invalid")
    if not _is_sha256(value.get("principal_identity_sha256")):
        errors.append("principal_identity_sha256_invalid")
    if value.get("admission_outcome") not in {"admitted", "held", "denied"}:
        errors.append("admission_outcome_invalid")
    if value.get("provenance_state") != "provenance_backed":
        errors.append("provenance_state_invalid")
    if not isinstance(value.get("admission_basis"), str) or not value["admission_basis"].strip():
        errors.append("admission_basis_missing")
    return errors


def validate_ingress_gap(value: Any) -> list[str]:
    if not isinstance(value, dict):
        return ["gap_not_object"]
    errors = _validate_common_receipt(value)
    if value.get("schema") != INGRESS_GAP_SCHEMA or value.get("schema_version") != 1:
        errors.append("schema_mismatch")
    if not _is_bounded_id(value.get("ingress_gap_id"), "ingress_gap_"):
        errors.append("ingress_gap_id_invalid")
    if not _is_bounded_id(value.get("journey_id"), "journey_"):
        errors.append("journey_id_invalid")
    attempted = value.get("attempted_contact_id")
    if attempted is not None and not _is_bounded_id(attempted, "contact_"):
        errors.append("attempted_contact_id_invalid")
    if value.get("reason") not in {
        "mixed_or_unverified_source",
        "contact_capture_failed",
    }:
        errors.append("reason_invalid")
    if value.get("admission_outcome") != "admitted":
        errors.append("admission_outcome_invalid")
    if value.get("provenance_state") not in {
        "provenance_backed",
        "mixed_or_unverified",
    }:
        errors.append("provenance_state_invalid")
    return errors


def _load_receipt_directory(
    directory: Path,
    workspace: Path,
    validator: Any,
    identity_field: str,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    rows: list[dict[str, Any]] = []
    rejected: list[dict[str, Any]] = []
    for path in sorted(directory.glob("*.json")):
        try:
            value = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            rejected.append({"path": str(path.relative_to(workspace)), "errors": [str(error)]})
            continue
        errors = validator(value)
        if isinstance(value, dict) and path.stem != value.get(identity_field):
            errors.append("filename_identity_mismatch")
        if errors:
            rejected.append({"path": str(path.relative_to(workspace)), "errors": errors})
        else:
            rows.append(value)
    return rows, rejected


def _load_contacts(workspace: Path) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    return _load_receipt_directory(
        state_dir(workspace) / "contact_receipts",
        workspace,
        validate_contact_receipt,
        "contact_id",
    )


def _load_ingress_gaps(workspace: Path) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    return _load_receipt_directory(
        state_dir(workspace) / "ingress_gaps",
        workspace,
        validate_ingress_gap,
        "ingress_gap_id",
    )


def _load_journeys(workspace: Path) -> tuple[dict[str, dict[str, Any]], list[str]]:
    rows: dict[str, dict[str, Any]] = {}
    errors: list[str] = []
    directory = workspace / "diagnostics/signal_spine_v1/journeys"
    for path in sorted(directory.glob("*.json")):
        try:
            value = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            errors.append(f"{path.name}:{error}")
            continue
        journey_id = str(value.get("journey_id") or "")
        if value.get("schema") != "causal_signal_journey_v1" or not journey_id:
            errors.append(f"{path.name}:invalid_journey_identity")
            continue
        if journey_id in rows:
            errors.append(f"{path.name}:duplicate_journey_id")
            continue
        rows[journey_id] = value
    return rows, errors


def _journey_origin(journey: dict[str, Any]) -> dict[str, Any]:
    raw = journey.get("journey_origin_v1")
    if not isinstance(raw, dict):
        return {
            "kind": "legacy_unknown",
            "input_root_expected": False,
            "classification_basis": "origin_absent_on_historical_journey",
        }
    kind = raw.get("kind")
    if kind not in ORIGIN_KINDS:
        return {
            "kind": "legacy_unknown",
            "input_root_expected": False,
            "classification_basis": "origin_present_but_invalid",
        }
    return {**raw, "classification_basis": "explicit_signal_journey_origin_v1"}


def _response_origin(journey: dict[str, Any]) -> str:
    value = journey.get("response_origin_v1")
    return str(value) if value in RESPONSE_ORIGINS else "legacy_unknown"


def _stage_summary(receipt: dict[str, Any] | None) -> dict[str, Any] | None:
    if receipt is None:
        return None
    temporal = receipt.get("temporal_envelope_v1")
    temporal = temporal if isinstance(temporal, dict) else {}
    process = receipt.get("process_identity_v1")
    process = process if isinstance(process, dict) else {}
    return {
        "stage_id": receipt.get("stage_id"),
        "stage_index": receipt.get("stage_index"),
        "stage_kind": receipt.get("stage_kind"),
        "effect": receipt.get("effect"),
        "stage_time_unix_ms": temporal.get("stage_time_unix_ms"),
        "monotonic_time_ns": temporal.get("monotonic_time_ns"),
        "clock_scope_id": process.get("clock_scope_id"),
        "process_identity_sha256": _canonical_sha256(process),
    }


def _last_stage(receipts: list[dict[str, Any]], predicate: Any) -> dict[str, Any] | None:
    matches = [row for row in receipts if predicate(row)]
    return max(matches, key=lambda row: int(row.get("stage_index") or 0), default=None)


def _terminal(receipts: list[dict[str, Any]]) -> tuple[str, dict[str, Any] | None]:
    delivery = _last_stage(
        receipts,
        lambda row: row.get("stage_kind") == "delivery_evidence"
        and row.get("effect") == "evidence_recorded",
    )
    if delivery is not None:
        return "delivered", delivery
    blocked = _last_stage(
        receipts,
        lambda row: row.get("stage_kind") == "blocked" and row.get("effect") == "blocked",
    )
    if blocked is not None:
        return "outbound_blocked", blocked
    dispatched = _last_stage(receipts, lambda row: row.get("stage_kind") == "dispatched")
    if dispatched is not None:
        if dispatched.get("effect") == "dispatch_failed":
            return "dispatch_failed", dispatched
        return "dispatched_unconfirmed", dispatched
    return "response_only", None


def _duration_ms(contact: dict[str, Any], stage: dict[str, Any] | None) -> tuple[float | None, str]:
    summary = _stage_summary(stage)
    if summary is None:
        return None, "stage_missing"
    if contact.get("process_identity_sha256") != summary["process_identity_sha256"]:
        return None, "split_process_identity"
    if contact.get("clock_scope_id") != summary.get("clock_scope_id"):
        return None, "split_or_missing_clock_scope"
    start = contact.get("received_monotonic_ns")
    end = summary.get("monotonic_time_ns")
    if not isinstance(start, int) or not isinstance(end, int) or end < start:
        return None, "monotonic_interval_unavailable"
    return (end - start) / 1_000_000.0, "same_process_monotonic"


def _capture_gap(reason: str, **fields: Any) -> dict[str, Any]:
    core = {"reason": reason, **fields}
    return {
        "schema": "contact_capacity_capture_gap_v2",
        "schema_version": 2,
        "gap_id": "contactgapv2_" + _canonical_sha256(core)[:24],
        **core,
        "existing_evidence_preserved": True,
        "timing_edge_inferred": False,
        "mutuality_inferred": False,
        "artifact_authority_state_v1": authority_state(),
    }


def _trace(
    contact: dict[str, Any],
    journey: dict[str, Any] | None,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    gaps: list[dict[str, Any]] = []
    admission = str(contact["admission_outcome"])
    receipts = [row for row in (journey or {}).get("receipts") or [] if isinstance(row, dict)]
    authored = next((row for row in receipts if row.get("stage_kind") == "authored"), None)
    terminal_state, terminal = _terminal(receipts)
    origin = _journey_origin(journey) if journey is not None else None
    response_origin = _response_origin(journey) if journey is not None else "not_applicable"
    journey_present = journey is not None

    if admission in {"held", "denied"}:
        if journey_present:
            gaps.append(
                _capture_gap(
                    "pre_prompt_decision_has_response_journey",
                    contact_id=contact["contact_id"],
                    journey_id=contact["journey_id"],
                )
            )
        terminal_state = f"{admission}_before_prompt"
        terminal = None
        response_duration, response_clock = None, "pre_prompt_no_response"
        terminal_duration, terminal_clock = None, "pre_prompt_decision"
        lineage_complete = not journey_present
    else:
        if not journey_present:
            gaps.append(
                _capture_gap(
                    "explicit_journey_missing",
                    contact_id=contact["contact_id"],
                    journey_id=contact["journey_id"],
                )
            )
        elif origin is None or origin.get("kind") != "contact":
            gaps.append(
                _capture_gap(
                    "journey_origin_is_not_contact",
                    contact_id=contact["contact_id"],
                    journey_id=contact["journey_id"],
                    observed_origin=(origin or {}).get("kind"),
                )
            )
        elif origin.get("contact_id") != contact["contact_id"]:
            gaps.append(
                _capture_gap(
                    "contact_origin_identity_mismatch",
                    contact_id=contact["contact_id"],
                    journey_id=contact["journey_id"],
                )
            )
        elif origin.get("evidence_sha256") != contact["receipt_sha256"]:
            gaps.append(
                _capture_gap(
                    "contact_origin_evidence_hash_mismatch",
                    contact_id=contact["contact_id"],
                    journey_id=contact["journey_id"],
                )
            )
        if authored is None:
            gaps.append(
                _capture_gap(
                    "first_orchestration_response_missing",
                    contact_id=contact["contact_id"],
                    journey_id=contact["journey_id"],
                )
            )
        response_duration, response_clock = _duration_ms(contact, authored)
        terminal_duration, terminal_clock = _duration_ms(contact, terminal)
        if authored is not None and response_duration is None:
            gaps.append(
                _capture_gap(
                    response_clock,
                    contact_id=contact["contact_id"],
                    journey_id=contact["journey_id"],
                )
            )
        terminal_complete = terminal_state in {
            "delivered",
            "outbound_blocked",
            "dispatch_failed",
            "dispatched_unconfirmed",
        }
        if journey_present and not terminal_complete:
            gaps.append(
                _capture_gap(
                    "terminal_response_outcome_missing",
                    contact_id=contact["contact_id"],
                    journey_id=contact["journey_id"],
                )
            )
        lineage_complete = bool(
            journey_present
            and origin is not None
            and origin.get("kind") == "contact"
            and origin.get("contact_id") == contact["contact_id"]
            and origin.get("evidence_sha256") == contact["receipt_sha256"]
            and authored is not None
            and terminal_complete
            and journey.get("lineage_valid") is True
        )

    trace_core = {
        "contact_id": contact["contact_id"],
        "journey_id": contact["journey_id"],
        "contact_receipt_sha256": contact["receipt_sha256"],
    }
    row = {
        "schema": "contact_capacity_trace_v2",
        "schema_version": 2,
        "trace_id": "contacttracev2_" + _canonical_sha256(trace_core)[:24],
        **trace_core,
        "input_identity_sha256": contact["input_identity_sha256"],
        "admitted_context_sha256": contact["admitted_context_sha256"],
        "principal_identity_sha256": contact["principal_identity_sha256"],
        "admission_outcome": admission,
        "journey_present": journey_present,
        "journey_origin_v1": origin,
        "response_origin_v1": response_origin,
        "journey_lineage_valid": (journey or {}).get("lineage_valid") is True,
        "first_orchestration_response": _stage_summary(authored),
        "terminal_state": terminal_state,
        "terminal_receipt": _stage_summary(terminal),
        "input_to_first_response_ms": response_duration,
        "input_to_first_response_clock_relation": response_clock,
        "input_to_terminal_ms": terminal_duration,
        "input_to_terminal_clock_relation": terminal_clock,
        "lineage_complete": lineage_complete,
        "causal_chain_complete": lineage_complete,
        "felt_capacity_inferred": False,
        "mutuality_inferred": False,
        "causal_effect_inferred_from_timing": False,
        "raw_input_included": False,
        "runtime_consumed": False,
        "live_control_authority": False,
        "artifact_authority_state_v1": authority_state(),
    }
    row["trace_sha256"] = _canonical_sha256(row)
    return row, gaps


def _origin_row(journey_id: str, journey: dict[str, Any]) -> dict[str, Any]:
    origin = _journey_origin(journey)
    receipts = [row for row in journey.get("receipts") or [] if isinstance(row, dict)]
    terminal_state, terminal = _terminal(receipts)
    row = {
        "schema": "contact_capacity_journey_origin_v2",
        "schema_version": 2,
        "journey_id": journey_id,
        "origin": origin,
        "response_origin_v1": _response_origin(journey),
        "journey_lineage_valid": journey.get("lineage_valid") is True,
        "terminal_state": terminal_state,
        "terminal_receipt": _stage_summary(terminal),
        "contact_root_expected": origin.get("kind") == "contact",
        "contact_root_present": False,
        "missing_contact_inferred": False,
        "felt_capacity_inferred": False,
        "artifact_authority_state_v1": authority_state(),
    }
    row["record_sha256"] = _canonical_sha256(row)
    return row


def _ingress_row(
    gap: dict[str, Any] | None,
    journey_id: str,
    journey: dict[str, Any] | None,
    origin: dict[str, Any] | None,
) -> dict[str, Any]:
    receipts = [row for row in (journey or {}).get("receipts") or [] if isinstance(row, dict)]
    terminal_state, terminal = _terminal(receipts)
    gap_id = str((gap or {}).get("ingress_gap_id") or (origin or {}).get("ingress_gap_id") or "")
    row = {
        "schema": "contact_capacity_ingress_gap_trace_v2",
        "schema_version": 2,
        "ingress_gap_id": gap_id,
        "journey_id": journey_id,
        "reason": (gap or {}).get("reason") or (origin or {}).get("ingress_gap_reason"),
        "gap_receipt_present": gap is not None,
        "journey_present": journey is not None,
        "origin_evidence_matches": bool(
            gap is not None
            and origin is not None
            and origin.get("evidence_sha256") == gap.get("receipt_sha256")
        ),
        "provenance_state": (gap or {}).get("provenance_state"),
        "attempted_contact_id": (gap or {}).get("attempted_contact_id"),
        "response_origin_v1": _response_origin(journey) if journey is not None else "not_available",
        "terminal_state": terminal_state if journey is not None else "journey_missing",
        "terminal_receipt": _stage_summary(terminal),
        "contact_root_emitted": False,
        "verified_subset_promoted_to_contact": False,
        "mutuality_inferred": False,
        "felt_capacity_inferred": False,
        "live_control_authority": False,
        "artifact_authority_state_v1": authority_state(),
    }
    row["trace_sha256"] = _canonical_sha256(row)
    return row


def _compat_trace(trace: dict[str, Any]) -> dict[str, Any]:
    row = dict(trace)
    row["schema"] = "contact_capacity_trace_v1"
    row["schema_version"] = 1
    row["trace_id"] = "contacttrace_" + _canonical_sha256(
        {
            "contact_id": row["contact_id"],
            "journey_id": row["journey_id"],
            "contact_receipt_sha256": row["contact_receipt_sha256"],
        }
    )
    row["trace_sha256"] = _canonical_sha256({key: value for key, value in row.items() if key != "trace_sha256"})
    return row


def _render_report(status: dict[str, Any]) -> str:
    return "\n".join(
        [
            "# Contact-to-Capacity Trace V2",
            "",
            "This evidence-only projection links a locally provenance-backed contact receipt to the exact Signal Spine journey identity. Local provenance validation is not cryptographic sender authentication. Timing proximity, hash similarity, and silence never create an edge.",
            "",
            f"- contact roots: {status['contact_receipt_count']}",
            f"- proven complete traces: {status['complete_trace_count']}",
            f"- ingress gaps: {status['ingress_gap_count']}",
            f"- autonomous journeys: {status['autonomous_journey_count']}",
            f"- operator journeys: {status['operator_journey_count']}",
            f"- legacy-unknown journeys: {status['legacy_unknown_journey_count']}",
            f"- true contact-root gaps: {status['true_contact_root_missing_count']}",
            f"- rejected receipts: {status['rejected_receipt_count']}",
            "",
            "Ingress gaps preserve dialogue availability without laundering a verified subset into contact or mutuality. Historical journeys with no origin field remain legacy_unknown. Felt effect, assent, closure, and control authority are never inferred.",
            "",
        ]
    )


def _render_compat_report(status: dict[str, Any]) -> str:
    return "\n".join(
        [
            "# Contact-to-Capacity Trace V1 Compatibility View",
            "",
            "Generated from the origin-aware V2 projection. Only explicit contact-origin journeys are treated as requiring contact roots; autonomous and legacy-unknown journeys are not relabeled as missing contact.",
            "",
            f"- contact roots: {status['contact_receipt_count']}",
            f"- complete explicit traces: {status['complete_trace_count']}",
            f"- true output journeys missing a contact root: {status['orphan_journey_count']}",
            f"- capture gaps: {status['capture_gap_count']}",
            "",
        ]
    )


def project(workspace: Path, *, write: bool) -> dict[str, Any]:
    contacts, rejected_contacts = _load_contacts(workspace)
    gap_receipts, rejected_gaps = _load_ingress_gaps(workspace)
    journeys, journey_errors = _load_journeys(workspace)
    errors = list(journey_errors)

    contact_ids = [str(row["contact_id"]) for row in contacts]
    contact_journeys = [str(row["journey_id"]) for row in contacts]
    gap_ids = [str(row["ingress_gap_id"]) for row in gap_receipts]
    gap_journeys = [str(row["journey_id"]) for row in gap_receipts]
    if len(contact_ids) != len(set(contact_ids)):
        errors.append("duplicate_contact_id")
    if len(contact_journeys) != len(set(contact_journeys)):
        errors.append("multiple_contact_roots_claim_same_journey")
    if len(gap_ids) != len(set(gap_ids)):
        errors.append("duplicate_ingress_gap_id")
    if len(gap_journeys) != len(set(gap_journeys)):
        errors.append("multiple_ingress_gaps_claim_same_journey")

    contacts_by_id = {str(row["contact_id"]): row for row in contacts}
    gaps_by_id = {str(row["ingress_gap_id"]): row for row in gap_receipts}
    traces: list[dict[str, Any]] = []
    capture_gaps: list[dict[str, Any]] = []
    ingress_rows: list[dict[str, Any]] = []
    origin_rows: list[dict[str, Any]] = []
    claimed_contacts: set[str] = set()
    claimed_gaps: set[str] = set()

    for contact in contacts:
        journey_id = str(contact["journey_id"])
        trace, trace_gaps = _trace(contact, journeys.get(journey_id))
        traces.append(trace)
        capture_gaps.extend(trace_gaps)
        claimed_contacts.add(str(contact["contact_id"]))

    for journey_id, journey in sorted(journeys.items()):
        origin = _journey_origin(journey)
        kind = str(origin["kind"])
        if kind == "contact":
            contact_id = str(origin.get("contact_id") or "")
            contact = contacts_by_id.get(contact_id)
            if contact is None:
                capture_gaps.append(
                    _capture_gap(
                        "contact_root_missing",
                        journey_id=journey_id,
                        contact_id=contact_id or None,
                    )
                )
            elif str(contact["journey_id"]) != journey_id:
                capture_gaps.append(
                    _capture_gap(
                        "contact_root_claims_different_journey",
                        journey_id=journey_id,
                        contact_id=contact_id,
                    )
                )
            claimed_contacts.add(contact_id)
        elif kind == "ingress_gap":
            gap_id = str(origin.get("ingress_gap_id") or "")
            gap = gaps_by_id.get(gap_id)
            ingress_rows.append(_ingress_row(gap, journey_id, journey, origin))
            claimed_gaps.add(gap_id)
            if gap is None:
                capture_gaps.append(
                    _capture_gap(
                        "ingress_gap_receipt_missing",
                        journey_id=journey_id,
                        ingress_gap_id=gap_id or None,
                    )
                )
            elif str(gap["journey_id"]) != journey_id:
                capture_gaps.append(
                    _capture_gap(
                        "ingress_gap_claims_different_journey",
                        journey_id=journey_id,
                        ingress_gap_id=gap_id,
                    )
                )
            elif origin.get("evidence_sha256") != gap.get("receipt_sha256"):
                capture_gaps.append(
                    _capture_gap(
                        "ingress_gap_evidence_hash_mismatch",
                        journey_id=journey_id,
                        ingress_gap_id=gap_id,
                    )
                )
        else:
            origin_rows.append(_origin_row(journey_id, journey))

    for gap in gap_receipts:
        gap_id = str(gap["ingress_gap_id"])
        if gap_id in claimed_gaps:
            continue
        journey_id = str(gap["journey_id"])
        ingress_rows.append(_ingress_row(gap, journey_id, journeys.get(journey_id), None))
        capture_gaps.append(
            _capture_gap(
                "ingress_gap_journey_origin_missing",
                journey_id=journey_id,
                ingress_gap_id=gap_id,
            )
        )

    origin_counts = Counter(
        str(_journey_origin(journey)["kind"])
        for journey in journeys.values()
    )
    terminal_counts = Counter(str(row["terminal_state"]) for row in traces)
    trace_ids = [str(row["trace_id"]) for row in traces]
    gap_trace_ids = [str(row["trace_sha256"]) for row in ingress_rows]
    capture_gap_ids = [str(row["gap_id"]) for row in capture_gaps]
    unique = (
        len(trace_ids) == len(set(trace_ids))
        and len(gap_trace_ids) == len(set(gap_trace_ids))
        and len(capture_gap_ids) == len(set(capture_gap_ids))
    )
    if not unique:
        errors.append("projected_identity_collision")
    rejected_receipt_count = len(rejected_contacts) + len(rejected_gaps)
    valid = not errors and rejected_receipt_count == 0
    true_missing = sum(row.get("reason") == "contact_root_missing" for row in capture_gaps)
    status = {
        "schema": "contact_capacity_trace_status_v2",
        "schema_version": 2,
        "valid": valid,
        "write": write,
        "contact_receipt_count": len(contacts),
        "rejected_contact_receipt_count": len(rejected_contacts),
        "ingress_gap_receipt_count": len(gap_receipts),
        "rejected_ingress_gap_receipt_count": len(rejected_gaps),
        "rejected_receipt_count": rejected_receipt_count,
        "journey_count": len(journeys),
        "trace_count": len(traces),
        "complete_trace_count": sum(bool(row["lineage_complete"]) for row in traces),
        "proven_contact_trace_count": sum(
            bool(row["lineage_complete"] and row["admission_outcome"] == "admitted")
            for row in traces
        ),
        "pre_prompt_terminal_count": sum(
            row["admission_outcome"] in {"held", "denied"} and row["lineage_complete"]
            for row in traces
        ),
        "ingress_gap_count": len(ingress_rows),
        "contact_journey_count": origin_counts["contact"],
        "autonomous_journey_count": origin_counts["autonomous"],
        "operator_journey_count": origin_counts["operator"],
        "legacy_unknown_journey_count": origin_counts["legacy_unknown"],
        "true_contact_root_missing_count": true_missing,
        "capture_gap_count": len(capture_gaps),
        "terminal_state_counts": dict(sorted(terminal_counts.items())),
        "rejected_contact_receipts": rejected_contacts,
        "rejected_ingress_gap_receipts": rejected_gaps,
        "errors": errors,
        "counter_audit": {
            "status": "consistent" if valid else "inconsistent",
            "checks": {
                "projected_ids_unique": unique,
                "contact_links_require_exact_ids": True,
                "ingress_gaps_never_emit_contact_roots": True,
                "autonomous_journeys_do_not_require_contact_roots": True,
                "absent_origins_are_legacy_unknown": True,
                "timing_requires_same_process_and_clock_scope": True,
                "proximity_never_creates_links": True,
                "felt_capacity_not_inferred": True,
                "projection_grants_no_authority": True,
            },
        },
        "runtime_relation": "evidence_projection_not_live_input_scheduler_model_or_control",
        "artifact_authority_state_v1": authority_state(),
    }

    compatibility_traces = [_compat_trace(row) for row in traces]
    true_orphans = [
        {
            "schema": "contact_capacity_orphan_journey_v1",
            "schema_version": 1,
            "journey_id": row.get("journey_id"),
            "contact_id": row.get("contact_id"),
            "contact_root_present": False,
            "relation": "explicit_contact_origin_with_missing_local_provenance_root",
            "artifact_authority_state_v1": authority_state(),
        }
        for row in capture_gaps
        if row.get("reason") == "contact_root_missing"
    ]
    compatibility_gaps = [
        {
            **row,
            "schema": "contact_capacity_capture_gap_v1",
            "schema_version": 1,
            "causal_chain_inferred": False,
        }
        for row in capture_gaps
    ]
    compatibility_status = {
        "schema": "contact_capacity_trace_status_v1",
        "schema_version": 1,
        "valid": valid,
        "write": write,
        "contact_receipt_count": len(contacts),
        "rejected_contact_receipt_count": len(rejected_contacts),
        "journey_count": len(journeys),
        "trace_count": len(compatibility_traces),
        "complete_trace_count": status["complete_trace_count"],
        "orphan_journey_count": len(true_orphans),
        "capture_gap_count": len(compatibility_gaps),
        "origin_aware_v2_ref": "../contact_capacity_trace_v2/status.json",
        "autonomous_journey_count": origin_counts["autonomous"],
        "legacy_unknown_journey_count": origin_counts["legacy_unknown"],
        "errors": errors,
        "artifact_authority_state_v1": authority_state(),
    }

    if write and valid:
        output = projection_dir(workspace)
        owner_atomic_write_jsonl(output / "traces.jsonl", traces)
        owner_atomic_write_jsonl(output / "ingress_gaps.jsonl", ingress_rows)
        owner_atomic_write_jsonl(output / "journey_origins.jsonl", origin_rows)
        owner_atomic_write_jsonl(output / "capture_gaps.jsonl", capture_gaps)
        owner_atomic_write_json(output / "status.json", status)
        owner_atomic_write(output / "report.md", _render_report(status))

        compatibility = state_dir(workspace)
        owner_atomic_write_jsonl(compatibility / "traces.jsonl", compatibility_traces)
        owner_atomic_write_jsonl(compatibility / "orphan_journeys.jsonl", true_orphans)
        owner_atomic_write_jsonl(compatibility / "capture_gaps.jsonl", compatibility_gaps)
        owner_atomic_write_json(compatibility / "status.json", compatibility_status)
        owner_atomic_write(compatibility / "report.md", _render_compat_report(compatibility_status))
    return status
