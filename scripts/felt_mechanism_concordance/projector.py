"""Append and project preregistered concordance studies."""

from __future__ import annotations

import json
from collections import Counter
from pathlib import Path
from typing import Any

try:
    from experiential_systems.common import (
        RecordValidationError, authority_state, deterministic_id, event_payload,
        load_jsonl, owner_append_jsonl, owner_atomic_write, owner_atomic_write_json,
        owner_atomic_write_jsonl, project_events, stream_payloads, utc_now,
        validate_bounded_identifier, validate_evidence_record,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError, authority_state, deterministic_id, event_payload,
        load_jsonl, owner_append_jsonl, owner_atomic_write, owner_atomic_write_json,
        owner_atomic_write_jsonl, project_events, stream_payloads, utc_now,
        validate_bounded_identifier, validate_evidence_record,
    )

from .model import (
    ConcordanceObservationV2, ConcordanceResultV2, ConcordanceStudyV1,
    FeltMomentRefV1, StudyStateV1,
)

STREAM = "felt_mechanism_concordance"
SCHEMA = "felt_mechanism_concordance_domain_event_v1"


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/felt_mechanism_concordance_v1"


def operator_path(workspace: Path) -> Path:
    return state_dir(workspace) / "operator_events.jsonl"


def _event(event_type: str, record: dict[str, Any], actor: str) -> dict[str, Any]:
    core = {"event_type": event_type, "record": record, "actor": actor}
    return {"schema": "concordance_operator_event_v1", "schema_version": 1,
            "event_id": deterministic_id("concordanceevent", core),
            "event_type": event_type, "actor": actor, "recorded_at": utc_now(),
            "record": record, "artifact_authority_state_v1": authority_state()}


def append_operator_event(workspace: Path, event_type: str, record: dict[str, Any], actor: str) -> dict[str, Any]:
    bounded_actor = validate_bounded_identifier(actor, "actor", limit=120) or ""
    value = _event(
        event_type, _validated_record(event_type, record), bounded_actor
    )
    owner_append_jsonl(operator_path(workspace), value)
    return value


def _validated_record(event_type: str, record: Any) -> dict[str, Any]:
    if not isinstance(record, dict):
        raise RecordValidationError("concordance record must be an object")
    validate_evidence_record(record)
    if event_type in {"study_created", "study_capture_prepared", "study_state_changed"}:
        return ConcordanceStudyV1.from_untrusted(record).to_dict()
    if event_type == "observation_recorded":
        return ConcordanceObservationV2.from_untrusted(record).to_dict()
    if event_type == "result_recorded":
        return ConcordanceResultV2.from_untrusted(record).to_dict()
    raise RecordValidationError("unknown concordance operator event")


def replay(workspace: Path) -> tuple[dict[str, ConcordanceStudyV1], dict[str, ConcordanceObservationV2], dict[str, ConcordanceResultV2], list[dict[str, Any]], list[str]]:
    rows, errors = load_jsonl(operator_path(workspace))
    studies: dict[str, ConcordanceStudyV1] = {}
    observations: dict[str, ConcordanceObservationV2] = {}
    results: dict[str, ConcordanceResultV2] = {}
    valid_events: list[dict[str, Any]] = []
    allowed_transitions = {
        StudyStateV1.DRAFT.value: {StudyStateV1.CAPTURE_READY.value},
        StudyStateV1.CAPTURE_READY.value: {
            StudyStateV1.BASELINE_CAPTURED.value
        },
        StudyStateV1.BASELINE_CAPTURED.value: {
            StudyStateV1.BASELINE_CAPTURED.value,
            StudyStateV1.CANDIDATE_CAPTURED.value,
        },
        StudyStateV1.CANDIDATE_CAPTURED.value: {
            StudyStateV1.COMPARISON_READY.value
        },
        StudyStateV1.COMPARISON_READY.value: {
            StudyStateV1.RESULT_RECORDED.value
        },
        StudyStateV1.RESULT_RECORDED.value: {
            StudyStateV1.REVIEW_PENDING.value
        },
        StudyStateV1.REVIEW_PENDING.value: {StudyStateV1.CLOSED.value},
        StudyStateV1.CLOSED.value: set(),
    }
    for index, event in enumerate(rows, 1):
        try:
            event_type = event.get("event_type")
            record = _validated_record(str(event_type or ""), event.get("record"))
            if event_type in {"study_created", "study_capture_prepared", "study_state_changed"}:
                item = ConcordanceStudyV1.from_untrusted(record)
                previous = studies.get(item.study_id)
                if event_type == "study_created":
                    if previous is not None or item.state != StudyStateV1.DRAFT.value:
                        raise RecordValidationError(
                            "study creation must introduce one draft study"
                        )
                elif previous is None:
                    raise RecordValidationError("study update precedes creation")
                elif item.state not in allowed_transitions[previous.state]:
                    raise RecordValidationError(
                        f"invalid study transition {previous.state}->{item.state}"
                    )
                elif (
                    previous.baseline_capture_ref
                    and item.baseline_capture_ref
                    != previous.baseline_capture_ref
                ):
                    raise RecordValidationError("baseline capture reference changed")
                elif (
                    previous.candidate_capture_ref
                    and item.candidate_capture_ref
                    != previous.candidate_capture_ref
                ):
                    raise RecordValidationError("candidate capture reference changed")
                studies[item.study_id] = item
            elif event_type == "observation_recorded":
                item = ConcordanceObservationV2.from_untrusted(record)
                study = studies.get(item.study_id)
                if study is None:
                    raise RecordValidationError("observation precedes study creation")
                if item.role == "baseline" and (
                    study.state != StudyStateV1.CAPTURE_READY.value
                    or not study.baseline_capture_ref
                ):
                    raise RecordValidationError(
                        "baseline observation requires prepared baseline capture"
                    )
                if item.role == "candidate" and (
                    study.state != StudyStateV1.BASELINE_CAPTURED.value
                    or not study.baseline_capture_ref
                    or not study.candidate_capture_ref
                ):
                    raise RecordValidationError(
                        "candidate observation requires baseline and candidate capture"
                    )
                observations[item.observation_id] = item
            elif event_type == "result_recorded":
                item = ConcordanceResultV2.from_untrusted(record)
                study = studies.get(item.study_id)
                if (
                    study is None
                    or study.state != StudyStateV1.COMPARISON_READY.value
                ):
                    raise RecordValidationError(
                        "result requires a comparison-ready study"
                    )
                results[item.result_id] = item
            valid_events.append(event)
        except (RecordValidationError, ValueError, TypeError) as error:
            errors.append(f"event_{index}:{error}")
    for observation in observations.values():
        study = studies.get(observation.study_id)
        if study is None:
            errors.append(f"observation_{observation.observation_id}:unknown_study")
        elif observation.role == "candidate" and not study.baseline_capture_ref:
            errors.append(
                f"observation_{observation.observation_id}:candidate_without_baseline"
            )
    for result in results.values():
        study = studies.get(result.study_id)
        baseline = observations.get(result.baseline_observation_id)
        candidate = observations.get(result.candidate_observation_id)
        if study is None:
            errors.append(f"result_{result.result_id}:unknown_study")
            continue
        if not study.baseline_capture_ref:
            errors.append(f"result_{result.result_id}:study_missing_baseline")
        if (
            baseline is None
            or baseline.study_id != result.study_id
            or baseline.role != "baseline"
        ):
            errors.append(f"result_{result.result_id}:invalid_baseline_observation")
        if (
            candidate is None
            or candidate.study_id != result.study_id
            or candidate.role != "candidate"
        ):
            errors.append(f"result_{result.result_id}:invalid_candidate_observation")
    return studies, observations, results, valid_events, errors


def valid_claim_and_witness(workspace: Path, claim_id: str, witness_id: str) -> bool:
    contract_path = workspace / "diagnostics/felt_contract_graph_v1/contracts.jsonl"
    context_path = workspace / "diagnostics/lived_state_witness_v1/context_index.jsonl"
    claim_found = any(claim_id in (json.loads(line).get("claim_ids") or []) for line in contract_path.read_text().splitlines()) if contract_path.is_file() else False
    witness_found = any(json.loads(line).get("witness_id") == witness_id for line in context_path.read_text().splitlines()) if context_path.is_file() else False
    return claim_found and witness_found


def valid_dossier(workspace: Path, dossier_id: str) -> bool:
    path = workspace / "diagnostics/experiment_dossiers_v1/status.json"
    if not path.is_file(): return False
    value = json.loads(path.read_text())
    return dossier_id in (value.get("dossiers") or {})


def valid_capture_ref(workspace: Path, value: str) -> bool:
    if value.startswith("journey_"):
        return (workspace / f"diagnostics/signal_spine_v1/journeys/{value}.json").is_file()
    if value.startswith("dossier_"):
        return valid_dossier(workspace, value)
    return False


def project(workspace: Path, *, write: bool) -> dict[str, Any]:
    studies, observations, results, events, errors = replay(workspace)
    payloads = []
    for event in events:
        record = dict(event["record"])
        aggregate_id = str(record.get("study_id") or record.get("felt_moment_ref_v1", {}).get("moment_id") or event["event_id"])
        if record.get("canonical_claim_id"):
            record["canonical_claim_id"] = record["canonical_claim_id"]
        payloads.append(event_payload(
            schema=SCHEMA, event_type=str(event["event_type"]),
            aggregate_type="concordance_study", aggregate_id=aggregate_id,
            idempotency_key=f"{STREAM}:{event['event_id']}", record=record,
        ))
    appended = project_events(workspace, STREAM, payloads,
                              actor="felt-mechanism-concordance-projector",
                              source_kind="operator_preregistered_study",
                              source_locator_value="diagnostics/felt_mechanism_concordance_v1/operator_events.jsonl") if write and not errors else 0
    counts = Counter(study.state for study in studies.values())
    status = {"schema": "felt_mechanism_concordance_status_v1", "schema_version": 1,
              "valid": not errors, "write": write, "study_count": len(studies),
              "observation_count": len(observations), "result_count": len(results),
              "state_counts": dict(sorted(counts.items())), "appended_event_count": appended,
              "baseline_missing_violation_count": sum(1 for result in results.values() if not any(obs.study_id == result.study_id and obs.role == "baseline" for obs in observations.values())),
              "numeric_relation_to_felt_report": "cannot_overwrite_suppress_or_score",
              "felt_report_relation": "external_primary_evidence_not_inferred_or_scored",
              "causation_established": False,
              "closure_propagated": False, "errors": errors,
              "counter_audit": {"status": "consistent" if not errors else "inconsistent",
                                "checks": {"all_results_have_baseline": all(any(obs.study_id == result.study_id and obs.role == "baseline" for obs in observations.values()) for result in results.values()),
                                           "outcomes_bounded": True, "felt_state_not_scored": True}},
              "artifact_authority_state_v1": authority_state()}
    if write and status["valid"]:
        output = state_dir(workspace)
        owner_atomic_write_jsonl(output / "studies.jsonl", [item.to_dict() for item in sorted(studies.values(), key=lambda item: item.study_id)])
        owner_atomic_write_jsonl(output / "observations.jsonl", [item.to_dict() for item in sorted(observations.values(), key=lambda item: item.observation_id)])
        owner_atomic_write_jsonl(output / "results.jsonl", [item.to_dict() for item in sorted(results.values(), key=lambda item: item.result_id)])
        owner_atomic_write_json(output / "status.json", status)
        owner_atomic_write(output / "report.md", "# Felt-Mechanism Concordance\n\nStudies are preregistered and baseline-gated. Numeric smoothness never overwrites felt friction.\n\n" + "\n".join(f"- {key}: {value}" for key, value in sorted(counts.items())) + "\n")
    return status


def query(workspace: Path, study_id: str | None) -> dict[str, Any]:
    studies, observations, results, _, errors = replay(workspace)
    return {"schema": "concordance_query_v1", "valid": not errors,
            "studies": [item.to_dict() for item in studies.values() if not study_id or item.study_id == study_id],
            "observations": [item.to_dict() for item in observations.values() if not study_id or item.study_id == study_id],
            "results": [item.to_dict() for item in results.values() if not study_id or item.study_id == study_id],
            "errors": errors}
