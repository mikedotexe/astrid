"""Immutable, prose-free representation records."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

try:
    from experiential_systems.common import (
        RecordValidationError,
        authority_state,
        deterministic_id,
        validate_bounded_identifier,
        validate_evidence_record,
        validate_sha256,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError,
        authority_state,
        deterministic_id,
        validate_bounded_identifier,
        validate_evidence_record,
        validate_sha256,
    )

_TRUSTED = object()
CONTRACT_KINDS = frozenset(
    {"vector", "field_set", "prompt_context", "provider_route", "model_profile", "response_artifact"}
)
TRANSITION_KINDS = frozenset(
    {"projection", "aggregation", "truncation", "packing", "provider_route", "fallback", "repair", "model_transition"}
)


def _ids(values: Any, field_name: str) -> tuple[str, ...]:
    if not isinstance(values, (list, tuple)):
        raise RecordValidationError(f"{field_name} must be a list")
    return tuple(
        validate_bounded_identifier(value, field_name, limit=160) or ""
        for value in values
    )


def _dims(values: Any, field_name: str) -> tuple[int, ...]:
    if not isinstance(values, (list, tuple)):
        raise RecordValidationError(f"{field_name} must be a list")
    result: list[int] = []
    for value in values:
        if isinstance(value, bool) or not isinstance(value, int) or value < 0 or value > 4096:
            raise RecordValidationError(f"{field_name} contains an invalid dimension")
        result.append(value)
    if len(set(result)) != len(result):
        raise RecordValidationError(f"{field_name} contains duplicates")
    return tuple(result)


@dataclass(frozen=True)
class RepresentationContractV1:
    contract_id: str
    name: str
    representation_kind: str
    dimension_count: int | None
    field_names: tuple[str, ...]
    source_refs: tuple[str, ...]
    source_hashes: tuple[str, ...]
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("representation contracts require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "representation_contract_v1",
            "schema_version": 1,
            "contract_id": self.contract_id,
            "name": self.name,
            "representation_kind": self.representation_kind,
            "dimension_count": self.dimension_count,
            "field_names": list(self.field_names),
            "source_refs": list(self.source_refs),
            "source_hashes": list(self.source_hashes),
            "felt_loss_scored": False,
            "authority_effect": False,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> RepresentationContractV1:
        if not isinstance(value, dict):
            raise RecordValidationError("representation contract must be an object")
        validate_evidence_record(value)
        kind = str(value.get("representation_kind") or "")
        if kind not in CONTRACT_KINDS:
            raise RecordValidationError("unsupported representation kind")
        count = value.get("dimension_count")
        if count is not None and (isinstance(count, bool) or not isinstance(count, int) or not 1 <= count <= 4096):
            raise RecordValidationError("dimension_count is invalid")
        fields = _ids(value.get("field_names", []), "field_names")
        refs = _ids(value.get("source_refs", []), "source_refs")
        hashes = tuple(validate_sha256(item, "source_hash") or "" for item in value.get("source_hashes", []))
        if len(refs) != len(hashes) or not refs:
            raise RecordValidationError("source references and hashes must align")
        name = validate_bounded_identifier(value.get("name"), "name", limit=120) or ""
        core = {"name": name, "representation_kind": kind, "dimension_count": count, "field_names": list(fields), "source_refs": list(refs), "source_hashes": list(hashes)}
        expected = deterministic_id("repr", core)
        if value.get("contract_id") != expected:
            raise RecordValidationError("representation contract_id mismatch")
        if value.get("felt_loss_scored") is not False or value.get("authority_effect") is not False:
            raise RecordValidationError("representation contract contains a forbidden inference")
        return cls(expected, name, kind, count, fields, refs, hashes, _TRUSTED)


def build_contract(
    *, name: str, representation_kind: str, dimension_count: int | None,
    field_names: tuple[str, ...] = (), source_refs: tuple[str, ...], source_hashes: tuple[str, ...],
) -> RepresentationContractV1:
    value = {
        "name": name, "representation_kind": representation_kind,
        "dimension_count": dimension_count, "field_names": list(field_names),
        "source_refs": list(source_refs), "source_hashes": list(source_hashes),
    }
    value["contract_id"] = deterministic_id("repr", value)
    value.update(
        {
            "schema": "representation_contract_v1",
            "schema_version": 1,
            "felt_loss_scored": False,
            "authority_effect": False,
            "artifact_authority_state_v1": authority_state(),
        }
    )
    return RepresentationContractV1.from_untrusted(value)


@dataclass(frozen=True)
class RepresentationTransitionV1:
    transition_id: str
    transition_kind: str
    source_contract_id: str
    output_contract_id: str
    source_sha256: str
    output_sha256: str
    retained_dimensions: tuple[int, ...]
    dropped_dimensions: tuple[int, ...]
    retained_fields: tuple[str, ...]
    dropped_fields: tuple[str, ...]
    aggregation: str
    truncation_count: int
    timing_ms: int | None
    source_event_id: str
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("representation transitions require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "representation_transition_v1", "schema_version": 1,
            "transition_id": self.transition_id, "transition_kind": self.transition_kind,
            "source_contract_id": self.source_contract_id, "output_contract_id": self.output_contract_id,
            "source_sha256": self.source_sha256, "output_sha256": self.output_sha256,
            "retained_dimensions": list(self.retained_dimensions), "dropped_dimensions": list(self.dropped_dimensions),
            "retained_fields": list(self.retained_fields), "dropped_fields": list(self.dropped_fields),
            "aggregation": self.aggregation, "truncation_count": self.truncation_count,
            "timing_ms": self.timing_ms, "source_event_id": self.source_event_id,
            "mechanical_loss_only": True, "felt_loss_scored": False,
            "contradiction_inferred": False, "raw_payload_included": False,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> RepresentationTransitionV1:
        if not isinstance(value, dict):
            raise RecordValidationError("representation transition must be an object")
        validate_evidence_record(value)
        kind = str(value.get("transition_kind") or "")
        if kind not in TRANSITION_KINDS:
            raise RecordValidationError("unsupported transition kind")
        source = validate_bounded_identifier(value.get("source_contract_id"), "source_contract_id") or ""
        output = validate_bounded_identifier(value.get("output_contract_id"), "output_contract_id") or ""
        source_hash = validate_sha256(value.get("source_sha256"), "source_sha256") or ""
        output_hash = validate_sha256(value.get("output_sha256"), "output_sha256") or ""
        retained_dims = _dims(value.get("retained_dimensions", []), "retained_dimensions")
        dropped_dims = _dims(value.get("dropped_dimensions", []), "dropped_dimensions")
        if set(retained_dims).intersection(dropped_dims):
            raise RecordValidationError("retained and dropped dimensions overlap")
        retained_fields = _ids(value.get("retained_fields", []), "retained_fields")
        dropped_fields = _ids(value.get("dropped_fields", []), "dropped_fields")
        aggregation = validate_bounded_identifier(value.get("aggregation"), "aggregation", limit=160) or ""
        truncation = value.get("truncation_count")
        if isinstance(truncation, bool) or not isinstance(truncation, int) or truncation < 0:
            raise RecordValidationError("truncation_count is invalid")
        timing = value.get("timing_ms")
        if timing is not None and (isinstance(timing, bool) or not isinstance(timing, int) or timing < 0):
            raise RecordValidationError("timing_ms is invalid")
        source_event = validate_bounded_identifier(value.get("source_event_id"), "source_event_id") or ""
        core = {"transition_kind": kind, "source_contract_id": source, "output_contract_id": output,
                "source_sha256": source_hash, "output_sha256": output_hash,
                "retained_dimensions": list(retained_dims), "dropped_dimensions": list(dropped_dims),
                "retained_fields": list(retained_fields), "dropped_fields": list(dropped_fields),
                "aggregation": aggregation, "truncation_count": truncation,
                "timing_ms": timing, "source_event_id": source_event}
        expected = deterministic_id("transition", core)
        if value.get("transition_id") != expected:
            raise RecordValidationError("transition_id mismatch")
        if (
            value.get("mechanical_loss_only") is not True
            or value.get("felt_loss_scored") is not False
            or value.get("contradiction_inferred") is not False
            or value.get("raw_payload_included") is not False
        ):
            raise RecordValidationError("transition contains a felt or contradiction inference")
        return cls(expected, kind, source, output, source_hash, output_hash,
                   retained_dims, dropped_dims, retained_fields, dropped_fields,
                   aggregation, truncation, timing, source_event, _TRUSTED)


def build_transition(**values: Any) -> RepresentationTransitionV1:
    core = dict(values)
    core["retained_dimensions"] = list(core.get("retained_dimensions", ()))
    core["dropped_dimensions"] = list(core.get("dropped_dimensions", ()))
    core["retained_fields"] = list(core.get("retained_fields", ()))
    core["dropped_fields"] = list(core.get("dropped_fields", ()))
    value = {
        **core,
        "schema": "representation_transition_v1",
        "schema_version": 1,
        "transition_id": deterministic_id("transition", core),
        "mechanical_loss_only": True,
        "felt_loss_scored": False,
        "contradiction_inferred": False,
        "raw_payload_included": False,
        "artifact_authority_state_v1": authority_state(),
    }
    return RepresentationTransitionV1.from_untrusted(value)


@dataclass(frozen=True)
class RepresentationLossReceiptV1:
    loss_receipt_id: str
    transition_id: str
    retained_count: int
    dropped_count: int
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("loss receipts require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {"schema": "representation_loss_receipt_v1", "schema_version": 1,
                "loss_receipt_id": self.loss_receipt_id, "transition_id": self.transition_id,
                "retained_count": self.retained_count, "dropped_count": self.dropped_count,
                "mechanical_loss_only": True, "felt_loss_score": None,
                "felt_state_inferred": False, "artifact_authority_state_v1": authority_state()}

    @classmethod
    def from_transition(cls, transition: RepresentationTransitionV1) -> RepresentationLossReceiptV1:
        retained = len(transition.retained_dimensions) + len(transition.retained_fields)
        dropped = len(transition.dropped_dimensions) + len(transition.dropped_fields) + transition.truncation_count
        core = {"transition_id": transition.transition_id, "retained_count": retained, "dropped_count": dropped}
        return cls(deterministic_id("reprloss", core), transition.transition_id, retained, dropped, _TRUSTED)

    @classmethod
    def from_untrusted(cls, value: Any) -> RepresentationLossReceiptV1:
        if not isinstance(value, dict):
            raise RecordValidationError("loss receipt must be an object")
        validate_evidence_record(value)
        transition = validate_bounded_identifier(
            value.get("transition_id"), "transition_id"
        ) or ""
        retained = value.get("retained_count")
        dropped = value.get("dropped_count")
        if any(
            isinstance(count, bool) or not isinstance(count, int) or count < 0
            for count in (retained, dropped)
        ):
            raise RecordValidationError("loss receipt counts must be non-negative integers")
        core = {
            "transition_id": transition,
            "retained_count": retained,
            "dropped_count": dropped,
        }
        expected = deterministic_id("reprloss", core)
        if value.get("loss_receipt_id") != expected:
            raise RecordValidationError("loss receipt identity mismatch")
        if (
            value.get("mechanical_loss_only") is not True
            or value.get("felt_loss_score") is not None
            or value.get("felt_state_inferred") is not False
        ):
            raise RecordValidationError("loss receipt contains a felt inference")
        return cls(expected, transition, retained, dropped, _TRUSTED)


@dataclass(frozen=True)
class ModelTransitionReceiptV1:
    receipt_id: str
    request_identity_sha256: str
    response_sha256: str
    provider_route: str
    model_profile: str
    repair_parent_call_id: str | None
    fallback_reason: str | None
    timing_ms: int
    source_witness_id: str
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("model transitions require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {"schema": "model_transition_receipt_v1", "schema_version": 1,
                "receipt_id": self.receipt_id, "request_identity_sha256": self.request_identity_sha256,
                "response_sha256": self.response_sha256, "provider_route": self.provider_route,
                "model_profile": self.model_profile, "repair_parent_call_id": self.repair_parent_call_id,
                "fallback_reason": self.fallback_reason,
                "timing_ms": self.timing_ms, "source_witness_id": self.source_witness_id,
                "raw_prompt_included": False, "raw_response_included": False,
                "felt_effect_inferred": False, "artifact_authority_state_v1": authority_state()}

    @classmethod
    def build(cls, *, request_identity_sha256: str, response_sha256: str,
              provider_route: str, model_profile: str, repair_parent_call_id: str | None,
              fallback_reason: str | None, timing_ms: int,
              source_witness_id: str) -> ModelTransitionReceiptV1:
        request_hash = validate_sha256(request_identity_sha256, "request_identity_sha256") or ""
        response_hash = validate_sha256(response_sha256, "response_sha256") or ""
        route = validate_bounded_identifier(provider_route, "provider_route", limit=80) or ""
        profile = validate_bounded_identifier(model_profile, "model_profile", limit=160) or ""
        repair = validate_bounded_identifier(repair_parent_call_id, "repair_parent_call_id", optional=True)
        fallback = validate_bounded_identifier(
            fallback_reason, "fallback_reason", optional=True, limit=160
        )
        witness = validate_bounded_identifier(source_witness_id, "source_witness_id") or ""
        if isinstance(timing_ms, bool) or not isinstance(timing_ms, int) or timing_ms < 0:
            raise RecordValidationError("timing_ms is invalid")
        core = {"request_identity_sha256": request_hash, "response_sha256": response_hash,
                "provider_route": route, "model_profile": profile,
                "repair_parent_call_id": repair, "fallback_reason": fallback,
                "timing_ms": timing_ms,
                "source_witness_id": witness}
        return cls(deterministic_id("modeltransition", core), request_hash, response_hash,
                   route, profile, repair, fallback, timing_ms, witness, _TRUSTED)

    @classmethod
    def from_untrusted(cls, value: Any) -> ModelTransitionReceiptV1:
        if not isinstance(value, dict):
            raise RecordValidationError("model transition must be an object")
        validate_evidence_record(value)
        built = cls.build(
            request_identity_sha256=value.get("request_identity_sha256"),
            response_sha256=value.get("response_sha256"), provider_route=value.get("provider_route"),
            model_profile=value.get("model_profile"), repair_parent_call_id=value.get("repair_parent_call_id"),
            fallback_reason=value.get("fallback_reason"),
            timing_ms=value.get("timing_ms"), source_witness_id=value.get("source_witness_id"),
        )
        if value.get("receipt_id") != built.receipt_id:
            raise RecordValidationError("model transition receipt_id mismatch")
        if (
            value.get("felt_effect_inferred") is not False
            or value.get("raw_prompt_included") is not False
            or value.get("raw_response_included") is not False
        ):
            raise RecordValidationError(
                "model transition contains private content or a felt inference"
            )
        return built
