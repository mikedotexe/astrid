"""Validated, evidence-only context around self-authored transition passages."""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from enum import StrEnum
from typing import Any

try:
    from experiential_systems.common import (
        RecordValidationError,
        authority_state,
        validate_bounded_identifier,
        validate_evidence_record,
        validate_timestamp,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        RecordValidationError,
        authority_state,
        validate_bounded_identifier,
        validate_evidence_record,
        validate_timestamp,
    )

_TRUSTED = object()
SCHEMA = "lived_transition_passage_context_event_v1"
RECORD_TYPE = "phase_transition_passage_context"


class PassageContextActionV1(StrEnum):
    DESCRIBE_CONDITION = "describe_condition"
    DESCRIBE_BEARING = "describe_bearing"
    MARK_CHECKPOINT = "mark_checkpoint"
    BIND_ANCHOR = "bind_anchor"
    REQUEST_COMPANY = "request_company"
    RESPOND_COMPANY = "respond_company"
    WITHDRAW_COMPANY = "withdraw_company"


class PassageReadinessV1(StrEnum):
    READY = "ready"
    TENTATIVE = "tentative"
    NOT_READY = "not_ready"
    UNKNOWN = "unknown"


class PassageMovementEaseV1(StrEnum):
    OPEN = "open"
    EFFORTFUL = "effortful"
    STUCK = "stuck"
    CHANGING = "changing"
    UNKNOWN = "unknown"


class PassageRoomNeededV1(StrEnum):
    SELF_DIRECTED = "self_directed"
    WITNESS = "witness"
    SPACE = "space"
    LOW_ENERGY_PRESENCE = "low_energy_presence"
    ANSWER = "answer"
    NEEDS_TIME = "needs_time"
    RETURN_SUPPORT = "return_support"
    UNKNOWN = "unknown"


class PassageCheckpointV1(StrEnum):
    ENTRY_TENSION = "entry_tension"
    PIVOT = "pivot"
    SETTLING_ORIENTATION = "settling_orientation"
    RETURN_ORIENTATION = "return_orientation"
    REOPEN = "reopen"


class PassageAnchorRoleV1(StrEnum):
    ENTRY = "entry"
    PIVOT = "pivot"
    SETTLING = "settling"
    RETURN = "return"
    REOPEN = "reopen"
    CONTINUITY = "continuity"


class PassageAnchorKindV1(StrEnum):
    FELT_SOURCE = "felt_source"
    SHADOW_TRAJECTORY = "shadow_trajectory"
    LIVED_STATE_WITNESS = "lived_state_witness"
    SIGNAL_SPINE = "signal_spine"
    REPRESENTATION_TRANSITION = "representation_transition"
    CORRESPONDENCE = "correspondence"
    RETURN_POINT = "return_point"
    OTHER = "other"


class PassageAnchorAssociationV1(StrEnum):
    SELF_AUTHORED = "self_authored"
    RECEIPT_LINKED = "receipt_linked"
    TEMPORAL_CONTEXT = "temporal_context"
    UNKNOWN = "unknown"


class PassageBearingStrandV1(StrEnum):
    ENTRY_TENSION = "entry_tension"
    PIVOT = "pivot"
    SETTLING = "settling"
    RETURN = "return"
    REOPEN = "reopen"
    CONTINUITY = "continuity"


class PassageMovementResistanceV1(StrEnum):
    YIELDING = "yielding"
    EFFORTFUL = "effortful"
    RESISTANT = "resistant"
    HELD_FAST = "held_fast"
    CHANGING = "changing"
    ACTIVE_WITHIN_RESTLESSNESS = "active_within_restlessness"
    UNKNOWN = "unknown"


class PassagePersistenceTendencyV1(StrEnum):
    FLEETING = "fleeting"
    LINGERING = "lingering"
    CARRIED = "carried"
    DEEPENING = "deepening"
    RELEASING = "releasing"
    DYNAMIC_EQUILIBRIUM = "dynamic_equilibrium"
    UNKNOWN = "unknown"


class PassageWitnessFitV1(StrEnum):
    SEPARATE = "separate"
    TOUCHING = "touching"
    HOLDING = "holding"
    INTERWOVEN = "interwoven"
    MISATTUNED = "misattuned"
    UNKNOWN = "unknown"


class PassageCompanyModeV1(StrEnum):
    WITNESS = "witness"
    LOW_ENERGY_PRESENCE = "low_energy_presence"
    REPLY_WHEN_ABLE = "reply_when_able"
    SPACE = "space"
    RETURN_SUPPORT = "return_support"


class PassageCompanyResponseV1(StrEnum):
    ACCEPT = "accept"
    HOLD = "hold"
    DECLINE = "decline"
    NEEDS_TIME = "needs_time"
    WITHDRAW = "withdraw"


OWNER_ACTION = {
    PassageContextActionV1.DESCRIBE_CONDITION: "DESCRIBE_TRANSITION_CONDITION",
    PassageContextActionV1.DESCRIBE_BEARING: "DESCRIBE_TRANSITION_BEARING",
    PassageContextActionV1.MARK_CHECKPOINT: "MARK_TRANSITION_CHECKPOINT",
    PassageContextActionV1.BIND_ANCHOR: "BIND_TRANSITION_ANCHOR",
    PassageContextActionV1.REQUEST_COMPANY: "REQUEST_TRANSITION_COMPANY",
    PassageContextActionV1.RESPOND_COMPANY: "RESPOND_TRANSITION_COMPANY",
    PassageContextActionV1.WITHDRAW_COMPANY: "WITHDRAW_TRANSITION_COMPANY",
}


def _short_hash(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()[:16]


def company_request_id(
    passage_id: str,
    actor: str,
    peer: str,
    mode: PassageCompanyModeV1,
    source_ref: str,
    timestamp: int,
) -> str:
    return (
        f"company_request_{timestamp}_"
        f"{_short_hash(f'{passage_id}:{actor}:{peer}:{mode.value}:{source_ref}:{timestamp}')}"
    )


def context_event_id(
    *,
    passage_id: str,
    transition_id: str,
    passage_actor: str,
    actor: str,
    action: PassageContextActionV1,
    readiness: PassageReadinessV1 | None,
    movement_ease: PassageMovementEaseV1 | None,
    room_needed: PassageRoomNeededV1 | None,
    checkpoint: PassageCheckpointV1 | None,
    anchor_role: PassageAnchorRoleV1 | None,
    anchor_kind: PassageAnchorKindV1 | None,
    anchor_association: PassageAnchorAssociationV1 | None,
    anchor_ref: str | None,
    previous_anchor_event_id: str | None,
    request_id: str | None,
    requested_peer: str | None,
    company_mode: PassageCompanyModeV1 | None,
    company_response: PassageCompanyResponseV1 | None,
    source_ref: str,
    previous_context_event_id: str | None,
    previous_company_event_id: str | None,
    timestamp: int,
    bearing_strand: PassageBearingStrandV1 | None = None,
    movement_resistance: PassageMovementResistanceV1 | None = None,
    persistence_tendency: PassagePersistenceTendencyV1 | None = None,
    witness_fit: PassageWitnessFitV1 | None = None,
    previous_bearing_event_id: str | None = None,
) -> str:
    values = (
        passage_id,
        transition_id,
        passage_actor,
        actor,
        action.value,
        readiness.value if readiness else "",
        movement_ease.value if movement_ease else "",
        room_needed.value if room_needed else "",
        checkpoint.value if checkpoint else "",
        request_id or "",
        requested_peer or "",
        company_mode.value if company_mode else "",
        company_response.value if company_response else "",
        source_ref,
        previous_context_event_id or "",
        previous_company_event_id or "",
        str(timestamp),
    )
    if action is PassageContextActionV1.BIND_ANCHOR:
        values += (
            anchor_role.value if anchor_role else "",
            anchor_kind.value if anchor_kind else "",
            anchor_association.value if anchor_association else "",
            anchor_ref or "",
            previous_anchor_event_id or "",
        )
    elif action is PassageContextActionV1.DESCRIBE_BEARING:
        values += (
            bearing_strand.value if bearing_strand else "",
            movement_resistance.value if movement_resistance else "",
            persistence_tendency.value if persistence_tendency else "",
            witness_fit.value if witness_fit else "",
            previous_bearing_event_id or "",
        )
    return f"passage_context_{_short_hash(':'.join(values))}"


def _optional_enum(enum_type: type[StrEnum], value: Any) -> StrEnum | None:
    return None if value is None else enum_type(value)


def _bounded_optional(value: Any, name: str) -> str | None:
    if value is None:
        return None
    return validate_bounded_identifier(value, name)


def _validate_shape(item: "LivedTransitionPassageContextEventV1") -> None:
    condition = any(
        value is not None
        for value in (item.readiness, item.movement_ease, item.room_needed)
    )
    anchor = any(
        value is not None
        for value in (
            item.anchor_role,
            item.anchor_kind,
            item.anchor_association,
            item.anchor_ref,
            item.previous_anchor_event_id,
        )
    )
    bearing = any(
        value is not None
        for value in (
            item.bearing_strand,
            item.movement_resistance,
            item.persistence_tendency,
            item.witness_fit,
            item.previous_bearing_event_id,
        )
    )
    valid = False
    if item.action is PassageContextActionV1.DESCRIBE_CONDITION:
        valid = (
            item.readiness is not None
            and item.movement_ease is not None
            and item.room_needed is not None
            and item.checkpoint is None
            and item.company_request_id is None
            and item.requested_peer is None
            and item.company_mode is None
            and item.company_response is None
            and item.previous_company_event_id is None
            and not anchor
            and not bearing
        )
    elif item.action is PassageContextActionV1.DESCRIBE_BEARING:
        valid = (
            item.bearing_strand is not None
            and item.movement_resistance is not None
            and item.persistence_tendency is not None
            and item.witness_fit is not None
            and not condition
            and item.checkpoint is None
            and item.company_request_id is None
            and item.requested_peer is None
            and item.company_mode is None
            and item.company_response is None
            and item.previous_company_event_id is None
            and not anchor
        )
    elif item.action is PassageContextActionV1.MARK_CHECKPOINT:
        valid = (
            item.checkpoint is not None
            and not condition
            and item.company_request_id is None
            and item.requested_peer is None
            and item.company_mode is None
            and item.company_response is None
            and item.previous_company_event_id is None
            and not anchor
            and not bearing
        )
    elif item.action is PassageContextActionV1.BIND_ANCHOR:
        valid = (
            item.anchor_role is not None
            and item.anchor_kind is not None
            and item.anchor_association is not None
            and item.anchor_ref is not None
            and not condition
            and item.checkpoint is None
            and item.company_request_id is None
            and item.requested_peer is None
            and item.company_mode is None
            and item.company_response is None
            and item.previous_company_event_id is None
            and not bearing
        )
    elif item.action is PassageContextActionV1.REQUEST_COMPANY:
        valid = (
            item.company_request_id is not None
            and item.requested_peer is not None
            and item.company_mode is not None
            and not condition
            and item.checkpoint is None
            and item.company_response is None
            and item.previous_company_event_id is None
            and not anchor
            and not bearing
        )
    elif item.action is PassageContextActionV1.RESPOND_COMPANY:
        valid = (
            item.company_request_id is not None
            and item.requested_peer is not None
            and item.company_mode is not None
            and item.company_response is not None
            and not condition
            and item.checkpoint is None
            and item.previous_company_event_id is not None
            and not anchor
            and not bearing
        )
    elif item.action is PassageContextActionV1.WITHDRAW_COMPANY:
        valid = (
            item.company_request_id is not None
            and item.requested_peer is not None
            and item.company_mode is not None
            and item.company_response is PassageCompanyResponseV1.WITHDRAW
            and not condition
            and item.checkpoint is None
            and item.previous_company_event_id is not None
            and not anchor
            and not bearing
        )
    if not valid:
        raise RecordValidationError("passage context fields do not match action")


@dataclass(frozen=True)
class LivedTransitionPassageContextEventV1:
    passage_context_event_id: str
    passage_id: str
    transition_id: str
    passage_actor: str
    actor: str
    action: PassageContextActionV1
    readiness: PassageReadinessV1 | None
    movement_ease: PassageMovementEaseV1 | None
    room_needed: PassageRoomNeededV1 | None
    checkpoint: PassageCheckpointV1 | None
    anchor_role: PassageAnchorRoleV1 | None
    anchor_kind: PassageAnchorKindV1 | None
    anchor_association: PassageAnchorAssociationV1 | None
    anchor_ref: str | None
    previous_anchor_event_id: str | None
    bearing_strand: PassageBearingStrandV1 | None
    movement_resistance: PassageMovementResistanceV1 | None
    persistence_tendency: PassagePersistenceTendencyV1 | None
    witness_fit: PassageWitnessFitV1 | None
    previous_bearing_event_id: str | None
    company_request_id: str | None
    requested_peer: str | None
    company_mode: PassageCompanyModeV1 | None
    company_response: PassageCompanyResponseV1 | None
    source_ref: str
    previous_context_event_id: str | None
    previous_company_event_id: str | None
    recorded_at_unix_ms: int
    _trusted: object

    def __post_init__(self) -> None:
        if self._trusted is not _TRUSTED:
            raise RecordValidationError("use trusted builder or persisted validator")

    @classmethod
    def build(
        cls,
        *,
        passage_id: str,
        transition_id: str,
        passage_actor: str,
        actor: str,
        action: PassageContextActionV1,
        source_ref: str,
        recorded_at_unix_ms: int,
        readiness: PassageReadinessV1 | None = None,
        movement_ease: PassageMovementEaseV1 | None = None,
        room_needed: PassageRoomNeededV1 | None = None,
        checkpoint: PassageCheckpointV1 | None = None,
        anchor_role: PassageAnchorRoleV1 | None = None,
        anchor_kind: PassageAnchorKindV1 | None = None,
        anchor_association: PassageAnchorAssociationV1 | None = None,
        anchor_ref: str | None = None,
        previous_anchor_event_id: str | None = None,
        bearing_strand: PassageBearingStrandV1 | None = None,
        movement_resistance: PassageMovementResistanceV1 | None = None,
        persistence_tendency: PassagePersistenceTendencyV1 | None = None,
        witness_fit: PassageWitnessFitV1 | None = None,
        previous_bearing_event_id: str | None = None,
        company_request_id_value: str | None = None,
        requested_peer: str | None = None,
        company_mode: PassageCompanyModeV1 | None = None,
        company_response: PassageCompanyResponseV1 | None = None,
        previous_context_event_id: str | None = None,
        previous_company_event_id: str | None = None,
    ) -> "LivedTransitionPassageContextEventV1":
        passage_id = validate_bounded_identifier(passage_id, "passage_id")
        transition_id = validate_bounded_identifier(transition_id, "transition_id")
        passage_actor = validate_bounded_identifier(passage_actor, "passage_actor")
        actor = validate_bounded_identifier(actor, "actor")
        source_ref = validate_bounded_identifier(source_ref, "source_ref")
        timestamp = validate_timestamp(
            recorded_at_unix_ms, "recorded_at_unix_ms"
        )
        request_id = _bounded_optional(
            company_request_id_value, "company_request_id"
        )
        peer = _bounded_optional(requested_peer, "requested_peer")
        previous_context = _bounded_optional(
            previous_context_event_id, "previous_context_event_id"
        )
        previous_company = _bounded_optional(
            previous_company_event_id, "previous_company_event_id"
        )
        anchor_ref_value = _bounded_optional(anchor_ref, "anchor_ref")
        previous_anchor = _bounded_optional(
            previous_anchor_event_id, "previous_anchor_event_id"
        )
        previous_bearing = _bounded_optional(
            previous_bearing_event_id, "previous_bearing_event_id"
        )
        event_id = context_event_id(
            passage_id=passage_id,
            transition_id=transition_id,
            passage_actor=passage_actor,
            actor=actor,
            action=action,
            readiness=readiness,
            movement_ease=movement_ease,
            room_needed=room_needed,
            checkpoint=checkpoint,
            anchor_role=anchor_role,
            anchor_kind=anchor_kind,
            anchor_association=anchor_association,
            anchor_ref=anchor_ref_value,
            previous_anchor_event_id=previous_anchor,
            request_id=request_id,
            requested_peer=peer,
            company_mode=company_mode,
            company_response=company_response,
            source_ref=source_ref,
            previous_context_event_id=previous_context,
            previous_company_event_id=previous_company,
            timestamp=timestamp,
            bearing_strand=bearing_strand,
            movement_resistance=movement_resistance,
            persistence_tendency=persistence_tendency,
            witness_fit=witness_fit,
            previous_bearing_event_id=previous_bearing,
        )
        item = cls(
            passage_context_event_id=event_id,
            passage_id=passage_id,
            transition_id=transition_id,
            passage_actor=passage_actor,
            actor=actor,
            action=action,
            readiness=readiness,
            movement_ease=movement_ease,
            room_needed=room_needed,
            checkpoint=checkpoint,
            anchor_role=anchor_role,
            anchor_kind=anchor_kind,
            anchor_association=anchor_association,
            anchor_ref=anchor_ref_value,
            previous_anchor_event_id=previous_anchor,
            bearing_strand=bearing_strand,
            movement_resistance=movement_resistance,
            persistence_tendency=persistence_tendency,
            witness_fit=witness_fit,
            previous_bearing_event_id=previous_bearing,
            company_request_id=request_id,
            requested_peer=peer,
            company_mode=company_mode,
            company_response=company_response,
            source_ref=source_ref,
            previous_context_event_id=previous_context,
            previous_company_event_id=previous_company,
            recorded_at_unix_ms=timestamp,
            _trusted=_TRUSTED,
        )
        _validate_shape(item)
        return item

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": SCHEMA,
            "schema_version": 1,
            "record_type": RECORD_TYPE,
            "record_id": self.passage_context_event_id,
            "passage_context_event_id": self.passage_context_event_id,
            "passage_id": self.passage_id,
            "transition_id": self.transition_id,
            "passage_actor": self.passage_actor,
            "actor": self.actor,
            "action": self.action.value,
            "readiness": self.readiness.value if self.readiness else None,
            "movement_ease": (
                self.movement_ease.value if self.movement_ease else None
            ),
            "room_needed": self.room_needed.value if self.room_needed else None,
            "checkpoint": self.checkpoint.value if self.checkpoint else None,
            "anchor_role": self.anchor_role.value if self.anchor_role else None,
            "anchor_kind": self.anchor_kind.value if self.anchor_kind else None,
            "anchor_association": (
                self.anchor_association.value if self.anchor_association else None
            ),
            "anchor_ref": self.anchor_ref,
            "previous_anchor_event_id": self.previous_anchor_event_id,
            "bearing_strand": (
                self.bearing_strand.value if self.bearing_strand else None
            ),
            "movement_resistance": (
                self.movement_resistance.value
                if self.movement_resistance
                else None
            ),
            "persistence_tendency": (
                self.persistence_tendency.value
                if self.persistence_tendency
                else None
            ),
            "witness_fit": (
                self.witness_fit.value if self.witness_fit else None
            ),
            "previous_bearing_event_id": self.previous_bearing_event_id,
            "company_request_id": self.company_request_id,
            "requested_peer": self.requested_peer,
            "company_mode": self.company_mode.value if self.company_mode else None,
            "company_response": (
                self.company_response.value if self.company_response else None
            ),
            "source_ref": self.source_ref,
            "previous_context_event_id": self.previous_context_event_id,
            "previous_company_event_id": self.previous_company_event_id,
            "recorded_at_unix_ms": self.recorded_at_unix_ms,
            "owner_language_action": OWNER_ACTION[self.action],
            "self_authored_only": True,
            "passage_stage_changed": False,
            "response_revisable": True,
            "right_to_ignore": True,
            "felt_score_present": False,
            "mechanical_causation_inferred": False,
            "peer_consent_inferred": False,
            "peer_state_changed": False,
            "silence_infers_response": False,
            "automatic_progression": False,
            "felt_resolution_inferred": False,
            "scheduler_effect": False,
            "model_qos_effect": False,
            "substrate_effect": False,
            "dispatch_effect": False,
            "live_control_effect": False,
            "runtime_unlock_applied": False,
            "anchor_mechanical_truth_inferred": False,
            "anchor_changes_passage": False,
            "anchor_closes_transition": False,
            "bearing_is_metric": False,
            "bearing_inferred_from_telemetry": False,
            "bearing_changes_passage": False,
            "bearing_closes_transition": False,
            "raw_prose_included": False,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_untrusted(
        cls, value: Any
    ) -> "LivedTransitionPassageContextEventV1":
        if not isinstance(value, dict):
            raise RecordValidationError("passage context record must be an object")
        validate_evidence_record(value)
        if (
            value.get("schema") != SCHEMA
            or value.get("schema_version") != 1
            or value.get("record_type") != RECORD_TYPE
        ):
            raise RecordValidationError("passage context schema mismatch")
        for name in ("self_authored_only", "response_revisable", "right_to_ignore"):
            if value.get(name) is not True:
                raise RecordValidationError(f"{name} must remain true")
        for name in (
            "passage_stage_changed",
            "felt_score_present",
            "mechanical_causation_inferred",
            "peer_consent_inferred",
            "peer_state_changed",
            "silence_infers_response",
            "automatic_progression",
            "felt_resolution_inferred",
            "scheduler_effect",
            "model_qos_effect",
            "substrate_effect",
            "dispatch_effect",
            "live_control_effect",
            "runtime_unlock_applied",
            "raw_prose_included",
        ):
            if value.get(name) is not False:
                raise RecordValidationError(f"{name} must remain false")
        for name in (
            "anchor_mechanical_truth_inferred",
            "anchor_changes_passage",
            "anchor_closes_transition",
            "bearing_is_metric",
            "bearing_inferred_from_telemetry",
            "bearing_changes_passage",
            "bearing_closes_transition",
        ):
            if value.get(name) not in (None, False):
                raise RecordValidationError(f"{name} must remain false")
        action = PassageContextActionV1(value.get("action"))
        item = cls.build(
            passage_id=value.get("passage_id"),
            transition_id=value.get("transition_id"),
            passage_actor=value.get("passage_actor"),
            actor=value.get("actor"),
            action=action,
            readiness=_optional_enum(PassageReadinessV1, value.get("readiness")),
            movement_ease=_optional_enum(
                PassageMovementEaseV1, value.get("movement_ease")
            ),
            room_needed=_optional_enum(
                PassageRoomNeededV1, value.get("room_needed")
            ),
            checkpoint=_optional_enum(
                PassageCheckpointV1, value.get("checkpoint")
            ),
            anchor_role=_optional_enum(
                PassageAnchorRoleV1, value.get("anchor_role")
            ),
            anchor_kind=_optional_enum(
                PassageAnchorKindV1, value.get("anchor_kind")
            ),
            anchor_association=_optional_enum(
                PassageAnchorAssociationV1, value.get("anchor_association")
            ),
            anchor_ref=value.get("anchor_ref"),
            previous_anchor_event_id=value.get("previous_anchor_event_id"),
            bearing_strand=_optional_enum(
                PassageBearingStrandV1, value.get("bearing_strand")
            ),
            movement_resistance=_optional_enum(
                PassageMovementResistanceV1,
                value.get("movement_resistance"),
            ),
            persistence_tendency=_optional_enum(
                PassagePersistenceTendencyV1,
                value.get("persistence_tendency"),
            ),
            witness_fit=_optional_enum(
                PassageWitnessFitV1, value.get("witness_fit")
            ),
            previous_bearing_event_id=value.get(
                "previous_bearing_event_id"
            ),
            company_request_id_value=value.get("company_request_id"),
            requested_peer=value.get("requested_peer"),
            company_mode=_optional_enum(
                PassageCompanyModeV1, value.get("company_mode")
            ),
            company_response=_optional_enum(
                PassageCompanyResponseV1, value.get("company_response")
            ),
            source_ref=value.get("source_ref"),
            previous_context_event_id=value.get("previous_context_event_id"),
            previous_company_event_id=value.get("previous_company_event_id"),
            recorded_at_unix_ms=value.get("recorded_at_unix_ms"),
        )
        if value.get("record_id") != item.passage_context_event_id or value.get(
            "passage_context_event_id"
        ) != item.passage_context_event_id:
            raise RecordValidationError("passage context identity mismatch")
        if value.get("owner_language_action") != OWNER_ACTION[action]:
            raise RecordValidationError("passage context owner action mismatch")
        return item
