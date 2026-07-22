"""Immutable records that keep presence distinct from uptake."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum
from typing import Any

try:
    from experiential_systems.common import (
        RecordValidationError,
        authority_state,
        deterministic_id,
        validate_bounded_identifier,
        validate_evidence_record,
        validate_sha256,
        validate_timestamp,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError,
        authority_state,
        deterministic_id,
        validate_bounded_identifier,
        validate_evidence_record,
        validate_sha256,
        validate_timestamp,
    )


class PresenceKindV1(StrEnum):
    OFFERED = "presence_offered"
    DECLARED = "presence_declared"
    UNAVAILABLE = "presence_unavailable"


class UptakeKindV1(StrEnum):
    ATTENDED_MESSAGE = "attended_message"
    REPLY_INTENTION = "reply_intention"
    CONTINUITY_CARRIED_FORWARD = "continuity_carried_forward"
    DECLINED_ENGAGEMENT = "declined_engagement"
    NEEDS_TIME = "needs_time"
    WITHDRAWN_INTENTION = "withdrawn_intention"


class ReciprocalContextKindV1(StrEnum):
    DELIVERY_RECEIPT = "delivery_receipt"
    READ_RECEIPT = "read_receipt"
    REPLY_LINK = "reply_link"
    PRESENCE_HEARTBEAT = "presence_heartbeat"


_TRUSTED = object()


def _common(
    *,
    actor: Any,
    peer: Any,
    thread_id: Any,
    message_id: Any,
    source_event_id: Any,
    source_event_sha256: Any,
    body_sha256: Any,
    recorded_at_unix_ms: Any,
) -> dict[str, Any]:
    return {
        "actor": validate_bounded_identifier(actor, "actor", limit=80),
        "peer": validate_bounded_identifier(peer, "peer", limit=80),
        "thread_id": validate_bounded_identifier(thread_id, "thread_id"),
        "message_id": validate_bounded_identifier(
            message_id, "message_id", optional=True
        ),
        "source_event_id": validate_bounded_identifier(
            source_event_id, "source_event_id"
        ),
        "source_event_sha256": validate_sha256(
            source_event_sha256, "source_event_sha256"
        ),
        "body_sha256": validate_sha256(
            body_sha256, "body_sha256", optional=True
        ),
        "recorded_at_unix_ms": validate_timestamp(
            recorded_at_unix_ms, "recorded_at_unix_ms"
        ),
    }


@dataclass(frozen=True)
class ReciprocalPresenceReceiptV1:
    receipt_id: str
    presence_kind: str
    actor: str
    peer: str
    thread_id: str
    message_id: str | None
    source_event_id: str
    source_event_sha256: str
    body_sha256: str | None
    recorded_at_unix_ms: int
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("trusted presence records require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "reciprocal_presence_receipt_v1",
            "schema_version": 1,
            "receipt_id": self.receipt_id,
            "presence_kind": self.presence_kind,
            "actor": self.actor,
            "peer": self.peer,
            "thread_id": self.thread_id,
            "message_id": self.message_id,
            "source_event_id": self.source_event_id,
            "source_event_sha256": self.source_event_sha256,
            "body_sha256": self.body_sha256,
            "recorded_at_unix_ms": self.recorded_at_unix_ms,
            "presence_is_acknowledgement": False,
            "uptake_inferred": False,
            "raw_prose_included": False,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> ReciprocalPresenceReceiptV1:
        if not isinstance(value, dict):
            raise RecordValidationError("presence receipt must be an object")
        validate_evidence_record(value)
        kind = PresenceKindV1(str(value.get("presence_kind") or ""))
        common = _common(**{key: value.get(key) for key in (
            "actor", "peer", "thread_id", "message_id", "source_event_id",
            "source_event_sha256", "body_sha256", "recorded_at_unix_ms",
        )})
        core = {"presence_kind": kind.value, **common}
        expected = deterministic_id("presence", core)
        if value.get("receipt_id") != expected:
            raise RecordValidationError("presence receipt_id mismatch")
        if value.get("presence_is_acknowledgement") is not False:
            raise RecordValidationError("presence cannot imply acknowledgement")
        if value.get("uptake_inferred") is not False:
            raise RecordValidationError("presence cannot infer uptake")
        if value.get("raw_prose_included") is not False:
            raise RecordValidationError("presence receipt cannot contain raw prose")
        return cls(expected, kind.value, **common, _token=_TRUSTED)


@dataclass(frozen=True)
class ReciprocalUptakeReceiptV1:
    receipt_id: str
    uptake_kind: str
    actor: str
    peer: str
    thread_id: str
    message_id: str | None
    source_event_id: str
    source_event_sha256: str
    body_sha256: str | None
    recorded_at_unix_ms: int
    revises_receipt_id: str | None
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("trusted uptake records require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "reciprocal_uptake_receipt_v1",
            "schema_version": 1,
            "receipt_id": self.receipt_id,
            "uptake_kind": self.uptake_kind,
            "actor": self.actor,
            "peer": self.peer,
            "thread_id": self.thread_id,
            "message_id": self.message_id,
            "source_event_id": self.source_event_id,
            "source_event_sha256": self.source_event_sha256,
            "body_sha256": self.body_sha256,
            "recorded_at_unix_ms": self.recorded_at_unix_ms,
            "revises_receipt_id": self.revises_receipt_id,
            "intention_is_nonbinding": True,
            "decline_implies_closure": False,
            "decline_implies_disagreement": False,
            "decline_implies_negative_felt_state": False,
            "elapsed_time_inferred": False,
            "raw_prose_included": False,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> ReciprocalUptakeReceiptV1:
        if not isinstance(value, dict):
            raise RecordValidationError("uptake receipt must be an object")
        validate_evidence_record(value)
        kind = UptakeKindV1(str(value.get("uptake_kind") or ""))
        common = _common(**{key: value.get(key) for key in (
            "actor", "peer", "thread_id", "message_id", "source_event_id",
            "source_event_sha256", "body_sha256", "recorded_at_unix_ms",
        )})
        revises = validate_bounded_identifier(
            value.get("revises_receipt_id"),
            "revises_receipt_id",
            optional=True,
        )
        core = {"uptake_kind": kind.value, **common, "revises_receipt_id": revises}
        expected = deterministic_id("uptake", core)
        if value.get("receipt_id") != expected:
            raise RecordValidationError("uptake receipt_id mismatch")
        required_false = (
            "decline_implies_closure",
            "decline_implies_disagreement",
            "decline_implies_negative_felt_state",
            "elapsed_time_inferred",
            "raw_prose_included",
        )
        if any(value.get(field) is not False for field in required_false):
            raise RecordValidationError("uptake receipt contains a forbidden inference")
        if value.get("intention_is_nonbinding") is not True:
            raise RecordValidationError("reply intention must remain nonbinding")
        return cls(
            expected,
            kind.value,
            **common,
            revises_receipt_id=revises,
            _token=_TRUSTED,
        )


@dataclass(frozen=True)
class ReciprocalContextReceiptV1:
    receipt_id: str
    context_kind: str
    actor: str
    peer: str
    thread_id: str
    message_id: str | None
    source_event_id: str
    source_event_sha256: str
    body_sha256: str | None
    recorded_at_unix_ms: int
    corrects_inferred_uptake_receipt_id: str | None
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError(
                "trusted reciprocal context records require the internal builder"
            )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "reciprocal_context_receipt_v1",
            "schema_version": 1,
            "receipt_id": self.receipt_id,
            "context_kind": self.context_kind,
            "actor": self.actor,
            "peer": self.peer,
            "thread_id": self.thread_id,
            "message_id": self.message_id,
            "source_event_id": self.source_event_id,
            "source_event_sha256": self.source_event_sha256,
            "body_sha256": self.body_sha256,
            "recorded_at_unix_ms": self.recorded_at_unix_ms,
            "corrects_inferred_uptake_receipt_id": (
                self.corrects_inferred_uptake_receipt_id
            ),
            "presence_inferred": False,
            "acknowledgement_inferred": False,
            "uptake_inferred": False,
            "reply_intention_inferred": False,
            "elapsed_time_inferred": False,
            "raw_prose_included": False,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> ReciprocalContextReceiptV1:
        if not isinstance(value, dict):
            raise RecordValidationError("reciprocal context receipt must be an object")
        validate_evidence_record(value)
        kind = ReciprocalContextKindV1(str(value.get("context_kind") or ""))
        common = _common(
            **{
                key: value.get(key)
                for key in (
                    "actor",
                    "peer",
                    "thread_id",
                    "message_id",
                    "source_event_id",
                    "source_event_sha256",
                    "body_sha256",
                    "recorded_at_unix_ms",
                )
            }
        )
        corrects = validate_bounded_identifier(
            value.get("corrects_inferred_uptake_receipt_id"),
            "corrects_inferred_uptake_receipt_id",
            optional=True,
        )
        core = {
            "context_kind": kind.value,
            **common,
            "corrects_inferred_uptake_receipt_id": corrects,
        }
        expected = deterministic_id("reciprocalcontext", core)
        if value.get("receipt_id") != expected:
            raise RecordValidationError("reciprocal context receipt_id mismatch")
        required_false = (
            "presence_inferred",
            "acknowledgement_inferred",
            "uptake_inferred",
            "reply_intention_inferred",
            "elapsed_time_inferred",
            "raw_prose_included",
        )
        if any(value.get(name) is not False for name in required_false):
            raise RecordValidationError(
                "reciprocal context receipt contains a forbidden inference"
            )
        return cls(
            expected,
            kind.value,
            **common,
            corrects_inferred_uptake_receipt_id=corrects,
            _token=_TRUSTED,
        )


def build_presence_receipt(
    kind: PresenceKindV1, **values: Any
) -> ReciprocalPresenceReceiptV1:
    common = _common(**values)
    core = {"presence_kind": kind.value, **common}
    return ReciprocalPresenceReceiptV1(
        deterministic_id("presence", core), kind.value, **common, _token=_TRUSTED
    )


def build_uptake_receipt(
    kind: UptakeKindV1,
    *,
    revises_receipt_id: str | None = None,
    **values: Any,
) -> ReciprocalUptakeReceiptV1:
    common = _common(**values)
    revises = validate_bounded_identifier(
        revises_receipt_id, "revises_receipt_id", optional=True
    )
    core = {"uptake_kind": kind.value, **common, "revises_receipt_id": revises}
    return ReciprocalUptakeReceiptV1(
        deterministic_id("uptake", core),
        kind.value,
        **common,
        revises_receipt_id=revises,
        _token=_TRUSTED,
    )


def build_context_receipt(
    kind: ReciprocalContextKindV1,
    *,
    corrects_inferred_uptake_receipt_id: str | None = None,
    **values: Any,
) -> ReciprocalContextReceiptV1:
    common = _common(**values)
    corrects = validate_bounded_identifier(
        corrects_inferred_uptake_receipt_id,
        "corrects_inferred_uptake_receipt_id",
        optional=True,
    )
    core = {
        "context_kind": kind.value,
        **common,
        "corrects_inferred_uptake_receipt_id": corrects,
    }
    return ReciprocalContextReceiptV1(
        deterministic_id("reciprocalcontext", core),
        kind.value,
        **common,
        corrects_inferred_uptake_receipt_id=corrects,
        _token=_TRUSTED,
    )
