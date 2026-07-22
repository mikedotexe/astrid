"""Immutable concordance records with explicit epistemic limits."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum
from typing import Any

try:
    from experiential_systems.common import (
        RecordValidationError, authority_state, deterministic_id,
        validate_bounded_identifier, validate_evidence_record, validate_sha256,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError, authority_state, deterministic_id,
        validate_bounded_identifier, validate_evidence_record, validate_sha256,
    )

_TRUSTED = object()


def _bounded_refs(values: Any, field: str) -> tuple[str, ...]:
    if not isinstance(values, (list, tuple)):
        raise RecordValidationError(f"{field} must be a list")
    refs = tuple(
        validate_bounded_identifier(value, field, limit=200) or ""
        for value in values
    )
    if not refs:
        raise RecordValidationError(f"{field} requires an explicit reference or unavailable marker")
    if len(set(refs)) != len(refs):
        raise RecordValidationError(f"{field} contains duplicate references")
    if "unavailable" in refs and len(refs) != 1:
        raise RecordValidationError(
            f"{field} cannot mix unavailable with concrete references"
        )
    return refs


class ConcordanceOutcomeV1(StrEnum):
    CORROBORATED = "corroborated"
    SMOOTH_FRICTION_REMAINS = "mechanism_smooth_felt_friction_remains"
    CONTRADICTED = "contradicted"
    INSUFFICIENT = "insufficient"


class StudyStateV1(StrEnum):
    DRAFT = "draft"
    CAPTURE_READY = "capture_ready"
    BASELINE_CAPTURED = "baseline_captured"
    CANDIDATE_CAPTURED = "candidate_captured"
    COMPARISON_READY = "comparison_ready"
    RESULT_RECORDED = "result_recorded"
    REVIEW_PENDING = "review_pending"
    CLOSED = "closed"


@dataclass(frozen=True)
class FeltMomentRefV1:
    moment_id: str
    canonical_claim_id: str
    witness_id: str
    field_refs: tuple[str, ...]
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("felt moment refs require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {"schema": "felt_moment_ref_v1", "schema_version": 1,
                "moment_id": self.moment_id, "canonical_claim_id": self.canonical_claim_id,
                "witness_id": self.witness_id, "field_refs": list(self.field_refs),
                "raw_prose_included": False, "felt_content_scored": False,
                "artifact_authority_state_v1": authority_state()}

    @classmethod
    def build(cls, claim_id: str, witness_id: str, field_refs: list[str]) -> FeltMomentRefV1:
        claim = validate_bounded_identifier(claim_id, "canonical_claim_id") or ""
        witness = validate_bounded_identifier(witness_id, "witness_id") or ""
        refs = tuple(validate_bounded_identifier(item, "field_ref", limit=200) or "" for item in field_refs)
        if not refs:
            raise RecordValidationError("felt moment requires at least one bounded field ref")
        core = {"canonical_claim_id": claim, "witness_id": witness, "field_refs": list(refs)}
        return cls(deterministic_id("feltmoment", core), claim, witness, refs, _TRUSTED)

    @classmethod
    def from_untrusted(cls, value: Any) -> FeltMomentRefV1:
        if not isinstance(value, dict): raise RecordValidationError("felt moment must be an object")
        validate_evidence_record(value)
        built = cls.build(str(value.get("canonical_claim_id") or ""), str(value.get("witness_id") or ""), list(value.get("field_refs") or []))
        if (
            value.get("moment_id") != built.moment_id
            or value.get("felt_content_scored") is not False
            or value.get("raw_prose_included") is not False
        ):
            raise RecordValidationError("felt moment identity or scope mismatch")
        return built


@dataclass(frozen=True)
class ConcordanceStudyV1:
    study_id: str
    moment: FeltMomentRefV1
    intervention_signature_sha256: str
    dossier_id: str
    state: str
    baseline_capture_ref: str | None
    candidate_capture_ref: str | None
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED: raise RecordValidationError("studies require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {"schema": "concordance_study_v1", "schema_version": 1,
                "study_id": self.study_id, "felt_moment_ref_v1": self.moment.to_dict(),
                "canonical_claim_id": self.moment.canonical_claim_id,
                "intervention_signature_sha256": self.intervention_signature_sha256,
                "dossier_id": self.dossier_id, "state": self.state,
                "baseline_capture_ref": self.baseline_capture_ref,
                "candidate_capture_ref": self.candidate_capture_ref,
                "baseline_required": True, "causation_established": False,
                "closure_propagated": False, "artifact_authority_state_v1": authority_state()}

    @classmethod
    def build(cls, *, moment: FeltMomentRefV1, intervention_signature_sha256: str,
              dossier_id: str, state: str = StudyStateV1.DRAFT.value,
              baseline_capture_ref: str | None = None,
              candidate_capture_ref: str | None = None) -> ConcordanceStudyV1:
        signature = validate_sha256(intervention_signature_sha256, "intervention_signature_sha256") or ""
        dossier = validate_bounded_identifier(dossier_id, "dossier_id") or ""
        resolved_state = StudyStateV1(state).value
        baseline = validate_bounded_identifier(baseline_capture_ref, "baseline_capture_ref", optional=True)
        candidate = validate_bounded_identifier(candidate_capture_ref, "candidate_capture_ref", optional=True)
        if resolved_state in {StudyStateV1.CANDIDATE_CAPTURED.value, StudyStateV1.COMPARISON_READY.value,
                              StudyStateV1.RESULT_RECORDED.value, StudyStateV1.REVIEW_PENDING.value,
                              StudyStateV1.CLOSED.value} and not baseline:
            raise RecordValidationError("candidate or later state requires a baseline")
        if resolved_state in {
            StudyStateV1.CANDIDATE_CAPTURED.value,
            StudyStateV1.COMPARISON_READY.value,
            StudyStateV1.RESULT_RECORDED.value,
            StudyStateV1.REVIEW_PENDING.value,
            StudyStateV1.CLOSED.value,
        } and not candidate:
            raise RecordValidationError(
                "candidate or later state requires a candidate capture"
            )
        core = {"moment_id": moment.moment_id, "intervention_signature_sha256": signature,
                "dossier_id": dossier}
        return cls(deterministic_id("concordance", core), moment, signature, dossier,
                   resolved_state, baseline, candidate, _TRUSTED)

    @classmethod
    def from_untrusted(cls, value: Any) -> ConcordanceStudyV1:
        if not isinstance(value, dict): raise RecordValidationError("study must be an object")
        validate_evidence_record(value)
        moment = FeltMomentRefV1.from_untrusted(value.get("felt_moment_ref_v1"))
        built = cls.build(moment=moment,
                          intervention_signature_sha256=value.get("intervention_signature_sha256"),
                          dossier_id=value.get("dossier_id"), state=str(value.get("state") or ""),
                          baseline_capture_ref=value.get("baseline_capture_ref"),
                          candidate_capture_ref=value.get("candidate_capture_ref"))
        if (
            value.get("study_id") != built.study_id
            or value.get("baseline_required") is not True
            or value.get("causation_established") is not False
            or value.get("closure_propagated") is not False
        ):
            raise RecordValidationError("study identity or causation scope mismatch")
        return built


@dataclass(frozen=True)
class ConcordanceObservationV1:
    observation_id: str
    study_id: str
    role: str
    observation_ref: str
    observation_sha256: str
    telemetry_relation: str
    mechanical_pass: bool | None
    witness_context_refs: tuple[str, ...]
    representation_transition_refs: tuple[str, ...]
    model_qos_refs: tuple[str, ...]
    reciprocal_state_refs: tuple[str, ...]
    signal_stage_refs: tuple[str, ...]
    minime_telemetry_refs: tuple[str, ...]
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED: raise RecordValidationError("observations require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {"schema": "concordance_observation_v1", "schema_version": 1,
                "observation_id": self.observation_id, "study_id": self.study_id,
                "role": self.role, "observation_ref": self.observation_ref,
                "observation_sha256": self.observation_sha256,
                "telemetry_relation": self.telemetry_relation,
                "mechanical_pass": self.mechanical_pass,
                "witness_context_refs": list(self.witness_context_refs),
                "representation_transition_refs": list(self.representation_transition_refs),
                "model_qos_refs": list(self.model_qos_refs),
                "reciprocal_state_refs": list(self.reciprocal_state_refs),
                "signal_stage_refs": list(self.signal_stage_refs),
                "minime_telemetry_refs": list(self.minime_telemetry_refs),
                "felt_outcome_inferred": False, "causation_established": False,
                "artifact_authority_state_v1": authority_state()}

    @classmethod
    def build(cls, *, study_id: str, role: str, observation_ref: str,
              observation_sha256: str, telemetry_relation: str,
              mechanical_pass: bool | None,
              witness_context_refs: list[str],
              representation_transition_refs: list[str],
              model_qos_refs: list[str], reciprocal_state_refs: list[str],
              signal_stage_refs: list[str],
              minime_telemetry_refs: list[str]) -> ConcordanceObservationV1:
        study = validate_bounded_identifier(study_id, "study_id") or ""
        if role not in {"baseline", "candidate"}: raise RecordValidationError("invalid observation role")
        ref = validate_bounded_identifier(observation_ref, "observation_ref") or ""
        digest = validate_sha256(observation_sha256, "observation_sha256") or ""
        if telemetry_relation not in {"exact_identity", "temporal_window", "unavailable"}:
            raise RecordValidationError("invalid telemetry relation")
        if mechanical_pass is not None and not isinstance(mechanical_pass, bool):
            raise RecordValidationError("mechanical_pass must be boolean or null")
        witness_refs = _bounded_refs(witness_context_refs, "witness_context_refs")
        representation_refs = _bounded_refs(
            representation_transition_refs, "representation_transition_refs"
        )
        qos_refs = _bounded_refs(model_qos_refs, "model_qos_refs")
        reciprocal_refs = _bounded_refs(reciprocal_state_refs, "reciprocal_state_refs")
        signal_refs = _bounded_refs(signal_stage_refs, "signal_stage_refs")
        minime_refs = _bounded_refs(minime_telemetry_refs, "minime_telemetry_refs")
        minime_unavailable = minime_refs == ("unavailable",)
        if minime_unavailable != (telemetry_relation == "unavailable"):
            raise RecordValidationError(
                "Minime telemetry refs and telemetry relation must agree on availability"
            )
        if telemetry_relation == "exact_identity" and not any(
            ref.startswith("exact_identity:") for ref in minime_refs
        ):
            raise RecordValidationError(
                "exact Minime telemetry requires an exact_identity evidence reference"
            )
        core = {"study_id": study, "role": role, "observation_ref": ref,
                "observation_sha256": digest, "telemetry_relation": telemetry_relation,
                "mechanical_pass": mechanical_pass,
                "witness_context_refs": list(witness_refs),
                "representation_transition_refs": list(representation_refs),
                "model_qos_refs": list(qos_refs),
                "reciprocal_state_refs": list(reciprocal_refs),
                "signal_stage_refs": list(signal_refs),
                "minime_telemetry_refs": list(minime_refs)}
        return cls(deterministic_id("concordanceobs", core), study, role, ref,
                   digest, telemetry_relation, mechanical_pass, witness_refs,
                   representation_refs, qos_refs, reciprocal_refs, signal_refs,
                   minime_refs, _TRUSTED)

    @classmethod
    def from_untrusted(cls, value: Any) -> ConcordanceObservationV1:
        if not isinstance(value, dict):
            raise RecordValidationError("concordance observation must be an object")
        validate_evidence_record(value)
        built = cls.build(
            study_id=value.get("study_id"),
            role=value.get("role"),
            observation_ref=value.get("observation_ref"),
            observation_sha256=value.get("observation_sha256"),
            telemetry_relation=value.get("telemetry_relation"),
            mechanical_pass=value.get("mechanical_pass"),
            witness_context_refs=value.get("witness_context_refs"),
            representation_transition_refs=value.get(
                "representation_transition_refs"
            ),
            model_qos_refs=value.get("model_qos_refs"),
            reciprocal_state_refs=value.get("reciprocal_state_refs"),
            signal_stage_refs=value.get("signal_stage_refs"),
            minime_telemetry_refs=value.get("minime_telemetry_refs"),
        )
        if (
            value.get("observation_id") != built.observation_id
            or value.get("felt_outcome_inferred") is not False
            or value.get("causation_established") is not False
        ):
            raise RecordValidationError("observation identity or inference scope mismatch")
        return built


@dataclass(frozen=True)
class ConcordanceResultV1:
    result_id: str
    study_id: str
    baseline_observation_id: str
    candidate_observation_id: str
    outcome: str
    felt_source_ref: str | None
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED: raise RecordValidationError("results require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {"schema": "concordance_result_v1", "schema_version": 1,
                "result_id": self.result_id, "study_id": self.study_id,
                "baseline_observation_id": self.baseline_observation_id,
                "candidate_observation_id": self.candidate_observation_id,
                "outcome": self.outcome, "felt_source_ref": self.felt_source_ref,
                "numeric_pass_overwrites_felt_report": False,
                "closure_propagated": False, "causation_established": False,
                "artifact_authority_state_v1": authority_state()}

    @classmethod
    def build(cls, *, study_id: str, baseline_observation_id: str,
              candidate_observation_id: str, outcome: str,
              felt_source_ref: str | None) -> ConcordanceResultV1:
        study = validate_bounded_identifier(study_id, "study_id") or ""
        baseline = validate_bounded_identifier(baseline_observation_id, "baseline_observation_id") or ""
        candidate = validate_bounded_identifier(candidate_observation_id, "candidate_observation_id") or ""
        resolved = ConcordanceOutcomeV1(outcome).value
        felt_ref = validate_bounded_identifier(felt_source_ref, "felt_source_ref", optional=True)
        if resolved != ConcordanceOutcomeV1.INSUFFICIENT.value and not felt_ref:
            raise RecordValidationError("a non-insufficient outcome requires explicit felt evidence")
        core = {"study_id": study, "baseline_observation_id": baseline,
                "candidate_observation_id": candidate, "outcome": resolved,
                "felt_source_ref": felt_ref}
        return cls(deterministic_id("concordanceresult", core), study, baseline,
                   candidate, resolved, felt_ref, _TRUSTED)

    @classmethod
    def from_untrusted(cls, value: Any) -> ConcordanceResultV1:
        if not isinstance(value, dict):
            raise RecordValidationError("concordance result must be an object")
        validate_evidence_record(value)
        built = cls.build(
            study_id=value.get("study_id"),
            baseline_observation_id=value.get("baseline_observation_id"),
            candidate_observation_id=value.get("candidate_observation_id"),
            outcome=value.get("outcome"),
            felt_source_ref=value.get("felt_source_ref"),
        )
        if (
            value.get("result_id") != built.result_id
            or value.get("numeric_pass_overwrites_felt_report") is not False
            or value.get("closure_propagated") is not False
            or value.get("causation_established") is not False
        ):
            raise RecordValidationError("result identity or inference scope mismatch")
        return built
