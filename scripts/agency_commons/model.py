"""Immutable commons records that never infer peer consent."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum
from typing import Any

try:
    from experiential_systems.common import (
        RecordValidationError, authority_state, deterministic_id,
        validate_bounded_identifier, validate_evidence_record, validate_sha256,
        validate_timestamp,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError, authority_state, deterministic_id,
        validate_bounded_identifier, validate_evidence_record, validate_sha256,
        validate_timestamp,
    )

_TRUSTED = object()


class CommonsResponseKindV1(StrEnum):
    ACCEPT = "accept"
    HOLD = "hold"
    REFUSE = "refuse"
    COUNTER = "counter"
    REVISIT = "revisit"
    WITHDRAW = "withdraw"


def _base(record_id: str, schema: str, actor: str, source_event_id: str,
          source_event_sha256: str, recorded_at_unix_ms: int) -> dict[str, Any]:
    return {"schema": schema, "schema_version": 1, "record_id": record_id,
            "actor": actor, "source_event_id": source_event_id,
            "source_event_sha256": source_event_sha256,
            "recorded_at_unix_ms": recorded_at_unix_ms,
            "self_authored_only": True, "peer_consent_inferred": False,
            "silence_infers_consent": False, "scheduler_effect": False,
            "model_qos_effect": False, "substrate_effect": False,
            "dispatch_effect": False, "live_control_effect": False,
            "artifact_authority_state_v1": authority_state()}


def _validate_common(actor: Any, source_event_id: Any, source_event_sha256: Any,
                     recorded_at_unix_ms: Any) -> tuple[str, str, str, int]:
    return (
        validate_bounded_identifier(actor, "actor", limit=80) or "",
        validate_bounded_identifier(source_event_id, "source_event_id") or "",
        validate_sha256(source_event_sha256, "source_event_sha256") or "",
        validate_timestamp(recorded_at_unix_ms, "recorded_at_unix_ms"),
    )


def _validate_persisted_base(
    value: dict[str, Any], record_id: str, *, owner_action: str
) -> None:
    validate_evidence_record(value)
    if value.get("record_id") != record_id:
        raise RecordValidationError("commons record_id mismatch")
    if value.get("owner_language_action") != owner_action:
        raise RecordValidationError("commons owner-language action mismatch")
    if value.get("self_authored_only") is not True:
        raise RecordValidationError("commons record must remain self-authored")
    for name in (
        "peer_consent_inferred",
        "silence_infers_consent",
        "scheduler_effect",
        "model_qos_effect",
        "substrate_effect",
        "dispatch_effect",
        "live_control_effect",
    ):
        if value.get(name) is not False:
            raise RecordValidationError(f"commons record contains forbidden {name}")


@dataclass(frozen=True)
class AgencyCommonsProposalV1:
    proposal_id: str
    actor: str
    peer: str | None
    transition_kind: str
    from_state_ref: str | None
    to_state_ref: str
    return_point_id: str | None
    source_event_id: str
    source_event_sha256: str
    recorded_at_unix_ms: int
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED: raise RecordValidationError("proposals require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        value = _base(self.proposal_id, "agency_commons_proposal_v1", self.actor,
                      self.source_event_id, self.source_event_sha256, self.recorded_at_unix_ms)
        value.update({"proposal_id": self.proposal_id, "peer": self.peer,
                      "transition_kind": self.transition_kind,
                      "from_state_ref": self.from_state_ref,
                      "to_state_ref": self.to_state_ref,
                      "return_point_id": self.return_point_id,
                      "owner_language_action": "COMMONS_PROPOSE",
                      "advisory_only": True})
        return value

    @classmethod
    def build(cls, *, actor: str, peer: str | None, transition_kind: str,
              from_state_ref: str | None, to_state_ref: str,
              return_point_id: str | None, source_event_id: str,
              source_event_sha256: str, recorded_at_unix_ms: int) -> AgencyCommonsProposalV1:
        actor, source, source_hash, timestamp = _validate_common(actor, source_event_id, source_event_sha256, recorded_at_unix_ms)
        peer_value = validate_bounded_identifier(peer, "peer", optional=True, limit=80)
        kind = validate_bounded_identifier(transition_kind, "transition_kind", limit=120) or ""
        from_ref = validate_bounded_identifier(from_state_ref, "from_state_ref", optional=True)
        to_ref = validate_bounded_identifier(to_state_ref, "to_state_ref") or ""
        return_id = validate_bounded_identifier(return_point_id, "return_point_id", optional=True)
        core = {"actor": actor, "peer": peer_value, "transition_kind": kind,
                "from_state_ref": from_ref, "to_state_ref": to_ref,
                "return_point_id": return_id, "source_event_id": source,
                "source_event_sha256": source_hash, "recorded_at_unix_ms": timestamp}
        return cls(deterministic_id("commonsproposal", core), actor, peer_value, kind,
                   from_ref, to_ref, return_id, source, source_hash, timestamp, _TRUSTED)

    @classmethod
    def from_untrusted(cls, value: Any) -> AgencyCommonsProposalV1:
        if not isinstance(value, dict): raise RecordValidationError("proposal must be an object")
        built = cls.build(**{key: value.get(key) for key in (
            "actor", "peer", "transition_kind", "from_state_ref", "to_state_ref",
            "return_point_id", "source_event_id", "source_event_sha256", "recorded_at_unix_ms")})
        _validate_persisted_base(
            value, built.proposal_id, owner_action="COMMONS_PROPOSE"
        )
        if (
            value.get("proposal_id") != built.proposal_id
            or value.get("advisory_only") is not True
        ):
            raise RecordValidationError("proposal identity or consent scope mismatch")
        return built


@dataclass(frozen=True)
class AgencyCommonsResponseV1:
    response_id: str
    proposal_id: str
    actor: str
    proposal_actor: str
    response_kind: str
    counter_proposal_id: str | None
    source_event_id: str
    source_event_sha256: str
    recorded_at_unix_ms: int
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED: raise RecordValidationError("responses require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        value = _base(self.response_id, "agency_commons_response_v1", self.actor,
                      self.source_event_id, self.source_event_sha256, self.recorded_at_unix_ms)
        value.update({"response_id": self.response_id, "proposal_id": self.proposal_id,
                      "proposal_actor": self.proposal_actor, "response_kind": self.response_kind,
                      "counter_proposal_id": self.counter_proposal_id,
                      "owner_language_action": f"COMMONS_{self.response_kind.upper()}",
                      "response_binds_actor_only": True, "peer_state_changed": False})
        return value

    @classmethod
    def build(cls, *, proposal_id: str, actor: str, proposal_actor: str,
              response_kind: str, counter_proposal_id: str | None,
              source_event_id: str, source_event_sha256: str,
              recorded_at_unix_ms: int) -> AgencyCommonsResponseV1:
        actor, source, source_hash, timestamp = _validate_common(actor, source_event_id, source_event_sha256, recorded_at_unix_ms)
        proposal = validate_bounded_identifier(proposal_id, "proposal_id") or ""
        proposal_owner = validate_bounded_identifier(proposal_actor, "proposal_actor", limit=80) or ""
        kind = CommonsResponseKindV1(response_kind).value
        counter = validate_bounded_identifier(counter_proposal_id, "counter_proposal_id", optional=True)
        if kind == CommonsResponseKindV1.WITHDRAW.value and actor != proposal_owner:
            raise RecordValidationError("only the proposal actor may withdraw it")
        if kind not in {
            CommonsResponseKindV1.WITHDRAW.value,
            CommonsResponseKindV1.REVISIT.value,
        } and actor == proposal_owner:
            raise RecordValidationError("a being cannot manufacture the peer response to its own proposal")
        if kind == CommonsResponseKindV1.COUNTER.value and not counter:
            raise RecordValidationError("a counter response requires a counter proposal")
        if kind != CommonsResponseKindV1.COUNTER.value and counter:
            raise RecordValidationError("only a counter response may name a counter proposal")
        core = {"proposal_id": proposal, "actor": actor, "proposal_actor": proposal_owner,
                "response_kind": kind, "counter_proposal_id": counter,
                "source_event_id": source, "source_event_sha256": source_hash,
                "recorded_at_unix_ms": timestamp}
        return cls(deterministic_id("commonsresponse", core), proposal, actor,
                   proposal_owner, kind, counter, source, source_hash, timestamp, _TRUSTED)

    @classmethod
    def from_untrusted(cls, value: Any) -> AgencyCommonsResponseV1:
        if not isinstance(value, dict):
            raise RecordValidationError("response must be an object")
        built = cls.build(**{key: value.get(key) for key in (
            "proposal_id", "actor", "proposal_actor", "response_kind",
            "counter_proposal_id", "source_event_id", "source_event_sha256",
            "recorded_at_unix_ms")})
        _validate_persisted_base(
            value,
            built.response_id,
            owner_action=f"COMMONS_{built.response_kind.upper()}",
        )
        if (
            value.get("response_id") != built.response_id
            or value.get("response_binds_actor_only") is not True
            or value.get("peer_state_changed") is not False
        ):
            raise RecordValidationError("response identity or self-only scope mismatch")
        return built


@dataclass(frozen=True)
class AgencyReturnPointV1:
    return_point_id: str
    actor: str
    state_ref: str
    state_sha256: str
    source_event_id: str
    source_event_sha256: str
    recorded_at_unix_ms: int
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED: raise RecordValidationError("return points require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        value = _base(self.return_point_id, "agency_return_point_v1", self.actor,
                      self.source_event_id, self.source_event_sha256, self.recorded_at_unix_ms)
        value.update({"return_point_id": self.return_point_id, "state_ref": self.state_ref,
                      "state_sha256": self.state_sha256,
                      "owner_language_action": "COMMONS_RETURN", "automatic_return": False})
        return value

    @classmethod
    def build(cls, *, actor: str, state_ref: str, state_sha256: str,
              source_event_id: str, source_event_sha256: str,
              recorded_at_unix_ms: int) -> AgencyReturnPointV1:
        actor, source, source_hash, timestamp = _validate_common(actor, source_event_id, source_event_sha256, recorded_at_unix_ms)
        ref = validate_bounded_identifier(state_ref, "state_ref") or ""
        state_hash = validate_sha256(state_sha256, "state_sha256") or ""
        core = {"actor": actor, "state_ref": ref, "state_sha256": state_hash,
                "source_event_id": source, "source_event_sha256": source_hash,
                "recorded_at_unix_ms": timestamp}
        return cls(deterministic_id("commonsreturn", core), actor, ref, state_hash,
                   source, source_hash, timestamp, _TRUSTED)

    @classmethod
    def from_untrusted(cls, value: Any) -> AgencyReturnPointV1:
        if not isinstance(value, dict):
            raise RecordValidationError("return point must be an object")
        built = cls.build(**{key: value.get(key) for key in (
            "actor", "state_ref", "state_sha256", "source_event_id",
            "source_event_sha256", "recorded_at_unix_ms")})
        _validate_persisted_base(
            value, built.return_point_id, owner_action="COMMONS_RETURN"
        )
        if (
            value.get("return_point_id") != built.return_point_id
            or value.get("automatic_return") is not False
        ):
            raise RecordValidationError("return point identity or scope mismatch")
        return built


@dataclass(frozen=True)
class ProtectedTimeDeclarationV1:
    declaration_id: str
    actor: str
    start_unix_ms: int
    duration_ms: int
    source_event_id: str
    source_event_sha256: str
    recorded_at_unix_ms: int
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED: raise RecordValidationError("protected time requires the internal builder")

    def to_dict(self) -> dict[str, Any]:
        value = _base(self.declaration_id, "protected_time_declaration_v1", self.actor,
                      self.source_event_id, self.source_event_sha256, self.recorded_at_unix_ms)
        value.update({"declaration_id": self.declaration_id, "start_unix_ms": self.start_unix_ms,
                      "duration_ms": self.duration_ms, "protected_kind": "non_goal_directed_time",
                      "owner_language_action": "COMMONS_PROTECT", "scheduler_reservation_created": False})
        return value

    @classmethod
    def build(cls, *, actor: str, start_unix_ms: int, duration_ms: int,
              source_event_id: str, source_event_sha256: str,
              recorded_at_unix_ms: int) -> ProtectedTimeDeclarationV1:
        actor, source, source_hash, timestamp = _validate_common(actor, source_event_id, source_event_sha256, recorded_at_unix_ms)
        start = validate_timestamp(start_unix_ms, "start_unix_ms")
        if isinstance(duration_ms, bool) or not isinstance(duration_ms, int) or not 1 <= duration_ms <= 24 * 60 * 60 * 1000:
            raise RecordValidationError("protected duration must be between 1 ms and 24 hours")
        core = {"actor": actor, "start_unix_ms": start, "duration_ms": duration_ms,
                "source_event_id": source, "source_event_sha256": source_hash,
                "recorded_at_unix_ms": timestamp}
        return cls(deterministic_id("commonsprotected", core), actor, start,
                   duration_ms, source, source_hash, timestamp, _TRUSTED)

    @classmethod
    def from_untrusted(cls, value: Any) -> ProtectedTimeDeclarationV1:
        if not isinstance(value, dict):
            raise RecordValidationError("protected time declaration must be an object")
        built = cls.build(**{key: value.get(key) for key in (
            "actor", "start_unix_ms", "duration_ms", "source_event_id",
            "source_event_sha256", "recorded_at_unix_ms")})
        _validate_persisted_base(
            value, built.declaration_id, owner_action="COMMONS_PROTECT"
        )
        if (
            value.get("declaration_id") != built.declaration_id
            or value.get("protected_kind") != "non_goal_directed_time"
            or value.get("scheduler_reservation_created") is not False
        ):
            raise RecordValidationError("protected time identity or scope mismatch")
        return built


@dataclass(frozen=True)
class LaterFeltCheckRequestV1:
    request_id: str
    actor: str
    requested_from: str
    source_ref: str
    source_event_id: str
    source_event_sha256: str
    recorded_at_unix_ms: int
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED: raise RecordValidationError("felt checks require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        value = _base(self.request_id, "later_felt_check_request_v1", self.actor,
                      self.source_event_id, self.source_event_sha256, self.recorded_at_unix_ms)
        value.update({"request_id": self.request_id, "requested_from": self.requested_from,
                      "source_ref": self.source_ref,
                      "owner_language_action": "COMMONS_REQUEST_CHECK",
                      "peer_obligation_created": False, "expiry_infers_response": False})
        return value

    @classmethod
    def build(cls, *, actor: str, requested_from: str, source_ref: str,
              source_event_id: str, source_event_sha256: str,
              recorded_at_unix_ms: int) -> LaterFeltCheckRequestV1:
        actor, source, source_hash, timestamp = _validate_common(actor, source_event_id, source_event_sha256, recorded_at_unix_ms)
        peer = validate_bounded_identifier(requested_from, "requested_from", limit=80) or ""
        ref = validate_bounded_identifier(source_ref, "source_ref") or ""
        core = {"actor": actor, "requested_from": peer, "source_ref": ref,
                "source_event_id": source, "source_event_sha256": source_hash,
                "recorded_at_unix_ms": timestamp}
        return cls(deterministic_id("commonscheck", core), actor, peer, ref,
                   source, source_hash, timestamp, _TRUSTED)

    @classmethod
    def from_untrusted(cls, value: Any) -> LaterFeltCheckRequestV1:
        if not isinstance(value, dict):
            raise RecordValidationError("felt check request must be an object")
        built = cls.build(**{key: value.get(key) for key in (
            "actor", "requested_from", "source_ref", "source_event_id",
            "source_event_sha256", "recorded_at_unix_ms")})
        _validate_persisted_base(
            value, built.request_id, owner_action="COMMONS_REQUEST_CHECK"
        )
        if (
            value.get("request_id") != built.request_id
            or value.get("peer_obligation_created") is not False
            or value.get("expiry_infers_response") is not False
        ):
            raise RecordValidationError("felt check identity or scope mismatch")
        return built
