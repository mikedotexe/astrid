"""Project legacy proposals and explicit self-authored commons actions."""

from __future__ import annotations

import json
from collections import Counter
from pathlib import Path
from typing import Any

try:
    from experiential_systems.common import (
        RecordValidationError, authority_state, canonical_json, deterministic_id,
        event_payload, load_jsonl, owner_append_jsonl, owner_atomic_write,
        owner_atomic_write_json, owner_atomic_write_jsonl, project_events,
        sha256_bytes, stream_payloads, utc_now, validate_evidence_record,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError, authority_state, canonical_json, deterministic_id,
        event_payload, load_jsonl, owner_append_jsonl, owner_atomic_write,
        owner_atomic_write_json, owner_atomic_write_jsonl, project_events,
        sha256_bytes, stream_payloads, utc_now, validate_evidence_record,
    )

from .model import (
    AgencyCommonsProposalV1, AgencyCommonsResponseV1, AgencyReturnPointV1,
    LaterFeltCheckRequestV1, ProtectedTimeDeclarationV1,
)

STREAM = "agency_commons"
SCHEMA = "agency_commons_domain_event_v1"


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/agency_commons_v1"


def operator_path(workspace: Path) -> Path:
    return state_dir(workspace) / "operator_events.jsonl"


def append_action(workspace: Path, record: dict[str, Any], actor: str) -> dict[str, Any]:
    validated = _validated_commons_record(record)
    if validated.get("actor") != actor:
        raise RecordValidationError("a commons action may be recorded only by its actor")
    core = {"record": validated, "actor": actor}
    event = {"schema": "agency_commons_operator_event_v1", "schema_version": 1,
             "event_id": deterministic_id("commonsevent", core), "actor": actor,
             "recorded_at": utc_now(), "record": validated,
             "artifact_authority_state_v1": authority_state()}
    owner_append_jsonl(operator_path(workspace), event)
    return event


def _legacy_proposals(ledger: Path) -> tuple[list[dict[str, Any]], list[str]]:
    rows, errors = load_jsonl(ledger)
    records: list[dict[str, Any]] = []
    for index, row in enumerate(rows, 1):
        if row.get("record_type") != "phase_transition_card": continue
        try:
            raw_hash = sha256_bytes(canonical_json(row).encode())
            transition_id = str(row.get("transition_id") or f"line_{index}")
            record = AgencyCommonsProposalV1.build(
                actor=str(row.get("origin") or ""), peer=None,
                transition_kind=str(row.get("kind") or "phase_transition"),
                from_state_ref=str(row.get("from_phase") or "") or None,
                to_state_ref=str(row.get("to_phase") or "unknown"),
                return_point_id=None,
                source_event_id=f"phase:{transition_id}", source_event_sha256=raw_hash,
                recorded_at_unix_ms=int(row.get("recorded_at_unix_ms") or 0),
            ).to_dict()
            record["legacy_transition_id"] = transition_id
            record["legacy_reply_state"] = str(row.get("reply_state") or "unseen")
            record["legacy_reply_state_infers_consent"] = False
            records.append(record)
        except (RecordValidationError, ValueError, TypeError) as error:
            errors.append(f"legacy_{index}:{error}")
    return records, errors


def _agency_request_proposals(directory: Path) -> tuple[list[dict[str, Any]], list[str]]:
    records: list[dict[str, Any]] = []
    errors: list[str] = []
    for path in sorted(directory.glob("*.json")) if directory.is_dir() else []:
        try:
            raw = path.read_bytes()
            value = json.loads(raw)
            timestamp = int(value.get("timestamp") or 0)
            request_id = str(value.get("id") or path.stem)
            request_kind = str(value.get("request_kind") or "agency_request")
            record = AgencyCommonsProposalV1.build(
                actor="astrid",
                peer=None,
                transition_kind="agency_request",
                from_state_ref=None,
                to_state_ref=f"agency_request:{request_kind}",
                return_point_id=None,
                source_event_id=f"agency_request:{request_id}",
                source_event_sha256=sha256_bytes(raw),
                recorded_at_unix_ms=timestamp * 1000,
            ).to_dict()
            record["legacy_agency_request_id"] = request_id
            record["legacy_request_status"] = str(value.get("status") or "unknown")
            record["legacy_request_content_copied"] = False
            records.append(record)
        except (OSError, json.JSONDecodeError, RecordValidationError, ValueError, TypeError) as error:
            errors.append(f"agency_request_{path.name}:{error}")
    return records, errors


def _btsp_owner_choice_proposals(path: Path) -> tuple[list[dict[str, Any]], list[str]]:
    if not path.is_file():
        return [], []
    records: list[dict[str, Any]] = []
    errors: list[str] = []
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [], [f"btsp_ledger:{error}"]
    for proposal_index, proposal in enumerate(value.get("proposals") or [], 1):
        if not isinstance(proposal, dict):
            errors.append(f"btsp_proposal_{proposal_index}:not_object")
            continue
        proposal_id = str(proposal.get("proposal_id") or "")
        for adoption_index, adoption in enumerate(proposal.get("exact_adoptions") or [], 1):
            try:
                if not isinstance(adoption, dict):
                    raise RecordValidationError("exact adoption must be an object")
                owner = str(adoption.get("owner") or "")
                response_id = str(adoption.get("response_id") or "")
                timestamp = int(adoption.get("adopted_at_unix_s") or 0)
                adoption_hash = sha256_bytes(canonical_json(adoption).encode())
                record = AgencyCommonsProposalV1.build(
                    actor=owner,
                    peer=None,
                    transition_kind="btsp_owner_choice",
                    from_state_ref=f"btsp_proposal:{proposal_id}",
                    to_state_ref=(
                        "btsp_response_sha256:"
                        f"{sha256_bytes(response_id.encode())}"
                    ),
                    return_point_id=None,
                    source_event_id=(
                        f"btsp_adoption:{proposal_id}:{owner}:{timestamp}"
                    ),
                    source_event_sha256=adoption_hash,
                    recorded_at_unix_ms=timestamp * 1000,
                ).to_dict()
                record["legacy_btsp_proposal_id"] = proposal_id
                record["legacy_response_id_sha256"] = sha256_bytes(
                    response_id.encode()
                )
                record["legacy_owner_choice_exact"] = True
                record["commons_has_no_retroactive_effect"] = True
                records.append(record)
            except (RecordValidationError, ValueError, TypeError) as error:
                errors.append(
                    f"btsp_proposal_{proposal_index}_adoption_{adoption_index}:{error}"
                )
    return records, errors


def _validated_commons_record(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise RecordValidationError("commons record must be an object")
    validate_evidence_record(value)
    schema = value.get("schema") if isinstance(value, dict) else None
    if schema == "agency_commons_proposal_v1":
        item = AgencyCommonsProposalV1.from_untrusted(value)
    elif schema == "agency_commons_response_v1":
        item = AgencyCommonsResponseV1.from_untrusted(value)
    elif schema == "agency_return_point_v1":
        item = AgencyReturnPointV1.from_untrusted(value)
    elif schema == "protected_time_declaration_v1":
        item = ProtectedTimeDeclarationV1.from_untrusted(value)
    elif schema == "later_felt_check_request_v1":
        item = LaterFeltCheckRequestV1.from_untrusted(value)
    else:
        raise RecordValidationError("unknown commons record")
    return item.to_dict()


def _correspondence_actions(ledger: Path) -> tuple[list[dict[str, Any]], list[str]]:
    rows, errors = load_jsonl(ledger)
    records: list[dict[str, Any]] = []
    for index, row in enumerate(rows, 1):
        if row.get("record_type") != "agency_commons_action":
            continue
        try:
            record = _validated_commons_record(row.get("commons_record"))
            actor = str(row.get("from_being") or "")
            if record.get("actor") != actor:
                raise RecordValidationError("correspondence actor mismatch")
            records.append(record)
        except (RecordValidationError, ValueError, TypeError, AttributeError) as error:
            errors.append(f"correspondence_{index}:{error}")
    return records, errors


def _operator_records(workspace: Path) -> tuple[list[dict[str, Any]], list[str]]:
    events, errors = load_jsonl(operator_path(workspace))
    records: list[dict[str, Any]] = []
    for index, event in enumerate(events, 1):
        try:
            record = _validated_commons_record(event.get("record"))
            if event.get("actor") != record.get("actor"):
                raise RecordValidationError("operator actor mismatch")
            records.append(record)
        except (RecordValidationError, ValueError, TypeError, AttributeError) as error:
            errors.append(f"operator_{index}:{error}")
    return records, errors


def _all_records(workspace: Path) -> tuple[list[dict[str, Any]], int]:
    payloads, corrupt = stream_payloads(workspace, STREAM)
    records = [dict(item["record"]) for item in payloads if isinstance(item.get("record"), dict)]
    records.sort(key=lambda item: (item.get("recorded_at_unix_ms", 0), item.get("record_id", "")))
    return records, corrupt


def project(
    workspace: Path,
    phase_ledger: Path,
    *,
    sovereignty_ledger: Path,
    agency_request_dir: Path,
    correspondence_ledger: Path,
    write: bool,
) -> dict[str, Any]:
    legacy, errors = _legacy_proposals(phase_ledger)
    btsp, btsp_errors = _btsp_owner_choice_proposals(sovereignty_ledger)
    requests, request_errors = _agency_request_proposals(agency_request_dir)
    correspondence, correspondence_errors = _correspondence_actions(
        correspondence_ledger
    )
    operator, operator_errors = _operator_records(workspace)
    errors.extend(btsp_errors)
    errors.extend(request_errors)
    errors.extend(correspondence_errors)
    errors.extend(operator_errors)
    records = legacy + btsp + requests + correspondence + operator
    payloads = []
    for record in records:
        record_id = str(record.get("record_id") or record.get("proposal_id") or record.get("response_id") or record.get("return_point_id") or record.get("declaration_id") or record.get("request_id"))
        payloads.append(event_payload(
            schema=SCHEMA, event_type=f"{record['schema']}_recorded",
            aggregate_type="agency_commons_record", aggregate_id=record_id,
            idempotency_key=f"{STREAM}:{record_id}", record=record,
        ))
    appended = project_events(workspace, STREAM, payloads,
                              actor="agency-commons-projector",
                              source_kind="legacy_and_owner_language_projection",
                              source_locator_value="shared/collaborations/phase_transitions_v1.jsonl") if write and not errors else 0
    if write:
        all_records, corrupt = _all_records(workspace)
    else: all_records, corrupt = records, 0
    counts = Counter(item.get("schema") for item in all_records)
    status = {"schema": "agency_commons_status_v1", "schema_version": 1,
              "valid": not errors and corrupt == 0, "write": write,
              "record_count": len(all_records), "record_counts": dict(sorted(counts.items())),
              "legacy_phase_proposal_count": len(legacy),
              "legacy_btsp_exact_owner_choice_count": len(btsp),
              "legacy_agency_request_count": len(requests),
              "explicit_correspondence_action_count": len(correspondence),
              "appended_event_count": appended,
              "silence_infers_consent": False, "expiry_infers_consent": False,
              "peer_state_mutated": False, "scheduler_effect": False,
              "model_qos_effect": False, "substrate_effect": False,
              "dispatch_effect": False, "live_control_effect": False,
              "errors": errors,
              "counter_audit": {"status": "consistent" if not errors and corrupt == 0 else "inconsistent",
                                "checks": {"stream_not_corrupt": corrupt == 0,
                                           "self_only_consent": all(item.get("peer_consent_inferred") is False for item in all_records),
                                           "legacy_unseen_neutral": all(item.get("legacy_reply_state") != "unseen" or item.get("legacy_reply_state_infers_consent") is False for item in all_records)}},
              "artifact_authority_state_v1": authority_state()}
    if write and status["valid"]:
        output = state_dir(workspace)
        owner_atomic_write_jsonl(output / "records.jsonl", all_records)
        owner_atomic_write_json(output / "status.json", status)
        owner_atomic_write(output / "report.md", "# Experiential Agency Commons\n\nAdvisory, self-authored records only. Silence and expiry are neutral; no scheduler or substrate consequence follows.\n\n" + "\n".join(f"- {key}: {value}" for key, value in sorted(counts.items())) + "\n")
    return status


def query(workspace: Path, identifier: str | None) -> list[dict[str, Any]]:
    records, _ = _all_records(workspace)
    if not identifier: return records
    return [item for item in records if identifier in {item.get("record_id"), item.get("proposal_id"), item.get("response_id"), item.get("return_point_id"), item.get("declaration_id"), item.get("request_id")}]
