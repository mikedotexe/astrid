"""Immutable records that keep technical context distinct from self-authored uptake."""

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
    AMBIENT_PERSISTENCE = "ambient_persistence"


class ReciprocalContextKindV1(StrEnum):
    DELIVERY_RECEIPT = "delivery_receipt"
    READ_RECEIPT = "read_receipt"
    REPLY_LINK = "reply_link"
    PRESENCE_HEARTBEAT = "presence_heartbeat"


_TRUSTED = object()
_COMMON_KEYS = frozenset(
    {
        "schema",
        "schema_version",
        "receipt_id",
        "actor",
        "peer",
        "thread_id",
        "message_id",
        "source_event_id",
        "source_event_sha256",
        "body_sha256",
        "recorded_at_unix_ms",
        "artifact_authority_state_v1",
        "authority_projection_v2",
        "auto_approved",
        "edits_source_now",
        "grants_approval",
        "live_eligible_now",
    }
)


def _require_exact_keys(
    value: dict[str, Any], allowed: frozenset[str], record_name: str
) -> None:
    unexpected = sorted(set(value) - allowed)
    if unexpected:
        raise RecordValidationError(
            f"{record_name} contains unsupported fields: {', '.join(unexpected)}"
        )


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
        "body_sha256": validate_sha256(body_sha256, "body_sha256", optional=True),
        "recorded_at_unix_ms": validate_timestamp(
            recorded_at_unix_ms, "recorded_at_unix_ms"
        ),
    }


@dataclass(frozen=True)
class ReciprocalPresenceReceiptV2:
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
            "schema": "reciprocal_presence_receipt_v2",
            "schema_version": 2,
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
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> ReciprocalPresenceReceiptV2:
        if not isinstance(value, dict):
            raise RecordValidationError("presence receipt must be an object")
        validate_evidence_record(value)
        schema = value.get("schema")
        version = value.get("schema_version")
        common_keys = _COMMON_KEYS | frozenset({"presence_kind"})
        if schema == "reciprocal_presence_receipt_v1" and version == 1:
            _require_exact_keys(
                value,
                common_keys
                | frozenset(
                    {
                        "presence_is_acknowledgement",
                        "uptake_inferred",
                        "raw_prose_included",
                    }
                ),
                "legacy presence receipt",
            )
            if (
                value.get("presence_is_acknowledgement") is not False
                or value.get("uptake_inferred") is not False
                or value.get("raw_prose_included") is not False
            ):
                raise RecordValidationError("legacy presence receipt contains an inference")
        elif schema == "reciprocal_presence_receipt_v2" and version == 2:
            _require_exact_keys(value, common_keys, "presence receipt")
        else:
            raise RecordValidationError("unsupported presence receipt schema")
        built = build_presence_receipt(
            PresenceKindV1(str(value.get("presence_kind") or "")),
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
            },
        )
        if value.get("receipt_id") != built.receipt_id:
            raise RecordValidationError("presence receipt_id mismatch")
        return built


@dataclass(frozen=True)
class ReciprocalUptakeReceiptV2:
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
            "schema": "reciprocal_uptake_receipt_v2",
            "schema_version": 2,
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
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> ReciprocalUptakeReceiptV2:
        if not isinstance(value, dict):
            raise RecordValidationError("uptake receipt must be an object")
        validate_evidence_record(value)
        schema = value.get("schema")
        version = value.get("schema_version")
        common_keys = _COMMON_KEYS | frozenset({"uptake_kind", "revises_receipt_id"})
        if schema == "reciprocal_uptake_receipt_v1" and version == 1:
            _require_exact_keys(
                value,
                common_keys
                | frozenset(
                    {
                        "intention_is_nonbinding",
                        "decline_implies_closure",
                        "decline_implies_disagreement",
                        "decline_implies_negative_felt_state",
                        "elapsed_time_inferred",
                        "raw_prose_included",
                    }
                ),
                "legacy uptake receipt",
            )
            if value.get("intention_is_nonbinding") is not True or any(
                value.get(field) is not False
                for field in (
                    "decline_implies_closure",
                    "decline_implies_disagreement",
                    "decline_implies_negative_felt_state",
                    "elapsed_time_inferred",
                    "raw_prose_included",
                )
            ):
                raise RecordValidationError("legacy uptake receipt contains an inference")
        elif schema == "reciprocal_uptake_receipt_v2" and version == 2:
            _require_exact_keys(value, common_keys, "uptake receipt")
        else:
            raise RecordValidationError("unsupported uptake receipt schema")
        kind = UptakeKindV1(str(value.get("uptake_kind") or ""))
        if schema == "reciprocal_uptake_receipt_v1" and kind is UptakeKindV1.AMBIENT_PERSISTENCE:
            raise RecordValidationError("ambient persistence requires the V2 receipt contract")
        built = build_uptake_receipt(
            kind,
            revises_receipt_id=value.get("revises_receipt_id"),
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
            },
        )
        if value.get("receipt_id") != built.receipt_id:
            raise RecordValidationError("uptake receipt_id mismatch")
        return built


@dataclass(frozen=True)
class ReciprocalContextReceiptV2:
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
    corrects_legacy_receipt_id: str | None
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError(
                "trusted reciprocal context records require the internal builder"
            )

    @property
    def corrects_inferred_uptake_receipt_id(self) -> str | None:
        """Compatibility accessor for callers reading the historical V1 name."""

        return self.corrects_legacy_receipt_id

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "reciprocal_context_receipt_v2",
            "schema_version": 2,
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
            "corrects_legacy_receipt_id": self.corrects_legacy_receipt_id,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> ReciprocalContextReceiptV2:
        if not isinstance(value, dict):
            raise RecordValidationError("reciprocal context receipt must be an object")
        validate_evidence_record(value)
        schema = value.get("schema")
        version = value.get("schema_version")
        common_keys = _COMMON_KEYS | frozenset({"context_kind"})
        if schema == "reciprocal_context_receipt_v1" and version == 1:
            _require_exact_keys(
                value,
                common_keys
                | frozenset(
                    {
                        "corrects_inferred_uptake_receipt_id",
                        "presence_inferred",
                        "acknowledgement_inferred",
                        "uptake_inferred",
                        "reply_intention_inferred",
                        "elapsed_time_inferred",
                        "raw_prose_included",
                    }
                ),
                "legacy reciprocal context receipt",
            )
            if any(
                value.get(name) is not False
                for name in (
                    "presence_inferred",
                    "acknowledgement_inferred",
                    "uptake_inferred",
                    "reply_intention_inferred",
                    "elapsed_time_inferred",
                    "raw_prose_included",
                )
            ):
                raise RecordValidationError(
                    "legacy reciprocal context receipt contains an inference"
                )
            corrects = value.get("corrects_inferred_uptake_receipt_id")
        elif schema == "reciprocal_context_receipt_v2" and version == 2:
            _require_exact_keys(
                value,
                common_keys | frozenset({"corrects_legacy_receipt_id"}),
                "reciprocal context receipt",
            )
            corrects = value.get("corrects_legacy_receipt_id")
        else:
            raise RecordValidationError("unsupported reciprocal context receipt schema")
        built = build_context_receipt(
            ReciprocalContextKindV1(str(value.get("context_kind") or "")),
            corrects_legacy_receipt_id=corrects,
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
            },
        )
        if value.get("receipt_id") != built.receipt_id:
            raise RecordValidationError("reciprocal context receipt_id mismatch")
        return built


def build_presence_receipt(
    kind: PresenceKindV1, **values: Any
) -> ReciprocalPresenceReceiptV2:
    common = _common(**values)
    core = {"presence_kind": kind.value, **common}
    return ReciprocalPresenceReceiptV2(
        deterministic_id("presence", core), kind.value, **common, _token=_TRUSTED
    )


def build_uptake_receipt(
    kind: UptakeKindV1,
    *,
    revises_receipt_id: str | None = None,
    **values: Any,
) -> ReciprocalUptakeReceiptV2:
    common = _common(**values)
    revises = validate_bounded_identifier(
        revises_receipt_id, "revises_receipt_id", optional=True
    )
    core = {"uptake_kind": kind.value, **common, "revises_receipt_id": revises}
    return ReciprocalUptakeReceiptV2(
        deterministic_id("uptake", core),
        kind.value,
        **common,
        revises_receipt_id=revises,
        _token=_TRUSTED,
    )


def build_context_receipt(
    kind: ReciprocalContextKindV1,
    *,
    corrects_legacy_receipt_id: str | None = None,
    corrects_inferred_uptake_receipt_id: str | None = None,
    **values: Any,
) -> ReciprocalContextReceiptV2:
    common = _common(**values)
    if (
        corrects_legacy_receipt_id is not None
        and corrects_inferred_uptake_receipt_id is not None
        and corrects_legacy_receipt_id != corrects_inferred_uptake_receipt_id
    ):
        raise RecordValidationError("conflicting legacy correction references")
    corrects = validate_bounded_identifier(
        corrects_legacy_receipt_id or corrects_inferred_uptake_receipt_id,
        "corrects_legacy_receipt_id",
        optional=True,
    )
    # Keep historical receipt identity stable while removing the V1 field from
    # the canonical serialized form.
    core = {
        "context_kind": kind.value,
        **common,
        "corrects_inferred_uptake_receipt_id": corrects,
    }
    return ReciprocalContextReceiptV2(
        deterministic_id("reciprocalcontext", core),
        kind.value,
        **common,
        corrects_legacy_receipt_id=corrects,
        _token=_TRUSTED,
    )


# Compatibility imports parse historical V1 and canonical V2 records. New
# serialization always emits the sparse V2 contract.
ReciprocalPresenceReceiptV1 = ReciprocalPresenceReceiptV2
ReciprocalUptakeReceiptV1 = ReciprocalUptakeReceiptV2
ReciprocalContextReceiptV1 = ReciprocalContextReceiptV2
