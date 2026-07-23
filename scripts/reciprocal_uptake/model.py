"""Immutable records that keep technical context distinct from self-authored uptake."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum
import re
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
    RESONANT_PERSISTENCE = "resonant_persistence"


class ReciprocalResonanceRelationV1(StrEnum):
    EXACT_AUTHORSHIP_WITNESS = "exact_authorship_witness"
    TEMPORAL_ASSOCIATION_ONLY = "temporal_association_only"


class ReciprocalContextKindV1(StrEnum):
    DELIVERY_RECEIPT = "delivery_receipt"
    READ_RECEIPT = "read_receipt"
    REPLY_LINK = "reply_link"
    PRESENCE_HEARTBEAT = "presence_heartbeat"


_TRUSTED = object()
_LIVED_STATE_WITNESS_ID_RE = re.compile(r"^lsw_[0-9a-f]{64}$")
_PARAMETER_REF_RE = re.compile(r"^[a-z0-9_.]{1,160}$")
_REQUIRED_RESONANCE_PARAMETER_REFS = frozenset(
    {
        "bridge.lambda1",
        "bridge.lambda2",
        "bridge.lambda1_lambda2_gap",
    }
)
_BODY_HASH_SCOPE = "exact_message_bytes_not_semantic_or_experiential_equivalence"
_SPECTRAL_SHAPE_SCOPE = (
    "selected_mechanical_context_not_semantic_equivalence_uptake_inference_or_causation"
)
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
        if kind is UptakeKindV1.RESONANT_PERSISTENCE:
            raise RecordValidationError(
                "resonant persistence requires the V3 witnessed receipt contract"
            )
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
class ReciprocalResonanceSignatureV1:
    signature_id: str
    lived_state_witness_id: str
    lived_state_witness_sha256: str
    parameter_refs: tuple[str, ...]
    context_relation: str
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError(
                "trusted reciprocal resonance signatures require the internal builder"
            )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "reciprocal_resonance_signature_v1",
            "schema_version": 1,
            "signature_id": self.signature_id,
            "lived_state_witness_id": self.lived_state_witness_id,
            "lived_state_witness_sha256": self.lived_state_witness_sha256,
            "parameter_refs": list(self.parameter_refs),
            "context_relation": self.context_relation,
            "spectral_shape_scope": _SPECTRAL_SHAPE_SCOPE,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> ReciprocalResonanceSignatureV1:
        if not isinstance(value, dict):
            raise RecordValidationError("reciprocal resonance signature must be an object")
        validate_evidence_record(value)
        _require_exact_keys(
            value,
            frozenset(
                {
                    "schema",
                    "schema_version",
                    "signature_id",
                    "lived_state_witness_id",
                    "lived_state_witness_sha256",
                    "parameter_refs",
                    "context_relation",
                    "spectral_shape_scope",
                    "artifact_authority_state_v1",
                    "authority_projection_v2",
                    "auto_approved",
                    "edits_source_now",
                    "grants_approval",
                    "live_eligible_now",
                }
            ),
            "reciprocal resonance signature",
        )
        if (
            value.get("schema") != "reciprocal_resonance_signature_v1"
            or value.get("schema_version") != 1
        ):
            raise RecordValidationError("unsupported reciprocal resonance signature")
        if value.get("spectral_shape_scope") != _SPECTRAL_SHAPE_SCOPE:
            raise RecordValidationError("reciprocal resonance signature scope mismatch")
        built = build_resonance_signature(
            lived_state_witness_id=value.get("lived_state_witness_id"),
            lived_state_witness_sha256=value.get("lived_state_witness_sha256"),
            parameter_refs=value.get("parameter_refs"),
            context_relation=value.get("context_relation"),
        )
        if value.get("signature_id") != built.signature_id:
            raise RecordValidationError("reciprocal resonance signature_id mismatch")
        return built


@dataclass(frozen=True)
class ReciprocalUptakeReceiptV3:
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
    resonance_signature_v1: ReciprocalResonanceSignatureV1
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError(
                "trusted resonant uptake records require the internal builder"
            )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "reciprocal_uptake_receipt_v3",
            "schema_version": 3,
            "receipt_id": self.receipt_id,
            "uptake_kind": self.uptake_kind,
            "actor": self.actor,
            "peer": self.peer,
            "thread_id": self.thread_id,
            "message_id": self.message_id,
            "source_event_id": self.source_event_id,
            "source_event_sha256": self.source_event_sha256,
            "body_sha256": self.body_sha256,
            "body_hash_scope": _BODY_HASH_SCOPE,
            "recorded_at_unix_ms": self.recorded_at_unix_ms,
            "revises_receipt_id": self.revises_receipt_id,
            "resonance_signature_v1": self.resonance_signature_v1.to_dict(),
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(cls, value: Any) -> ReciprocalUptakeReceiptV3:
        if not isinstance(value, dict):
            raise RecordValidationError("resonant uptake receipt must be an object")
        validate_evidence_record(value)
        _require_exact_keys(
            value,
            _COMMON_KEYS
            | frozenset(
                {
                    "uptake_kind",
                    "body_hash_scope",
                    "revises_receipt_id",
                    "resonance_signature_v1",
                }
            ),
            "resonant uptake receipt",
        )
        if (
            value.get("schema") != "reciprocal_uptake_receipt_v3"
            or value.get("schema_version") != 3
        ):
            raise RecordValidationError("unsupported resonant uptake receipt schema")
        if value.get("uptake_kind") != UptakeKindV1.RESONANT_PERSISTENCE.value:
            raise RecordValidationError("V3 uptake receipt must be resonant persistence")
        if value.get("body_hash_scope") != _BODY_HASH_SCOPE:
            raise RecordValidationError("resonant uptake body hash scope mismatch")
        signature = ReciprocalResonanceSignatureV1.from_untrusted(
            value.get("resonance_signature_v1")
        )
        built = build_resonant_uptake_receipt(
            resonance_signature_v1=signature,
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
            raise RecordValidationError("resonant uptake receipt_id mismatch")
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


def build_resonance_signature(
    *,
    lived_state_witness_id: Any,
    lived_state_witness_sha256: Any,
    parameter_refs: Any,
    context_relation: Any,
) -> ReciprocalResonanceSignatureV1:
    witness_id = validate_bounded_identifier(
        lived_state_witness_id, "lived_state_witness_id", limit=80
    )
    if not _LIVED_STATE_WITNESS_ID_RE.fullmatch(witness_id):
        raise RecordValidationError("lived_state_witness_id format is invalid")
    witness_sha256 = validate_sha256(
        lived_state_witness_sha256, "lived_state_witness_sha256"
    )
    if not isinstance(parameter_refs, (list, tuple)):
        raise RecordValidationError("resonance parameter_refs must be a list")
    if not 1 <= len(parameter_refs) <= 16:
        raise RecordValidationError("resonance parameter_refs must contain 1..16 entries")
    refs: list[str] = []
    for index, raw in enumerate(parameter_refs):
        if not isinstance(raw, str) or not _PARAMETER_REF_RE.fullmatch(raw):
            raise RecordValidationError(
                f"resonance parameter_refs[{index}] is not a bounded field reference"
            )
        refs.append(raw)
    normalized = tuple(sorted(set(refs)))
    if len(normalized) != len(refs):
        raise RecordValidationError("resonance parameter_refs must be unique")
    if not _REQUIRED_RESONANCE_PARAMETER_REFS.issubset(normalized):
        raise RecordValidationError(
            "resonance signature requires lambda1, lambda2, and lambda1_lambda2_gap"
        )
    relation = ReciprocalResonanceRelationV1(str(context_relation or ""))
    core = {
        "lived_state_witness_id": witness_id,
        "lived_state_witness_sha256": witness_sha256,
        "parameter_refs": normalized,
        "context_relation": relation.value,
        "spectral_shape_scope": _SPECTRAL_SHAPE_SCOPE,
    }
    return ReciprocalResonanceSignatureV1(
        deterministic_id("resonance", core),
        witness_id,
        witness_sha256,
        normalized,
        relation.value,
        _token=_TRUSTED,
    )


def build_uptake_receipt(
    kind: UptakeKindV1,
    *,
    revises_receipt_id: str | None = None,
    **values: Any,
) -> ReciprocalUptakeReceiptV2:
    if kind is UptakeKindV1.RESONANT_PERSISTENCE:
        raise RecordValidationError(
            "resonant persistence requires build_resonant_uptake_receipt"
        )
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


def build_resonant_uptake_receipt(
    *,
    resonance_signature_v1: ReciprocalResonanceSignatureV1,
    revises_receipt_id: str | None = None,
    **values: Any,
) -> ReciprocalUptakeReceiptV3:
    if not isinstance(resonance_signature_v1, ReciprocalResonanceSignatureV1):
        raise RecordValidationError(
            "resonant persistence requires a validated resonance signature"
        )
    common = _common(**values)
    revises = validate_bounded_identifier(
        revises_receipt_id, "revises_receipt_id", optional=True
    )
    core = {
        "uptake_kind": UptakeKindV1.RESONANT_PERSISTENCE.value,
        **common,
        "body_hash_scope": _BODY_HASH_SCOPE,
        "revises_receipt_id": revises,
        "resonance_signature_v1": resonance_signature_v1.to_dict(),
    }
    return ReciprocalUptakeReceiptV3(
        deterministic_id("uptake", core),
        UptakeKindV1.RESONANT_PERSISTENCE.value,
        **common,
        revises_receipt_id=revises,
        resonance_signature_v1=resonance_signature_v1,
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


# Compatibility imports parse historical V1 and canonical V2 records. Only
# explicit resonant persistence uses the witnessed V3 contract.
ReciprocalPresenceReceiptV1 = ReciprocalPresenceReceiptV2
ReciprocalUptakeReceiptV1 = ReciprocalUptakeReceiptV2
ReciprocalContextReceiptV1 = ReciprocalContextReceiptV2
