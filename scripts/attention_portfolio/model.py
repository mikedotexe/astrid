"""Immutable attention portfolio records with zero state propagation."""

from __future__ import annotations

from dataclasses import dataclass, field
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

STEWARD_SELECTION_SCOPE_V2 = "steward_work_view_not_being_attention"
CONTRACT_STATE_RELATION_V2 = "selection_does_not_change_contract_or_felt_state"
RUNTIME_RELATION_V2 = "not_consumed_by_bridge_minime_model_or_control_runtime"
AUTHORITY_RELATION_V2 = "cannot_grant_or_propagate_authority"
SOURCE_GRAPH_RELATION_V2 = "all_claims_contracts_and_evidence_remain_queryable"
UNSELECTED_RELATION_V2 = "retained_in_contract_graph_and_visible_when_urgent"

REVIEW_STATE_CLASSES_V2 = frozenset(
    {
        "contradiction_or_named_objection",
        "reopened_or_still_friction",
        "review_pending",
        "open",
        "quiet",
    }
)
RECENCY_CLASSES_V2 = frozenset({"recent_24h", "older", "unknown"})
AGE_BANDS_V2 = frozenset(
    {"under_24h", "one_to_seven_days", "seven_to_thirty_days", "over_thirty_days", "unknown"}
)


def _require_exact_keys(
    value: dict[str, Any], allowed: frozenset[str], record_name: str
) -> None:
    unexpected = sorted(set(value) - allowed)
    if unexpected:
        raise RecordValidationError(
            f"{record_name} contains unsupported fields: {', '.join(unexpected)}"
        )


def steward_unaddressed_age_band(age_ms: int) -> str:
    if age_ms < 0:
        raise RecordValidationError("steward unaddressed age cannot be negative")
    day_ms = 24 * 60 * 60 * 1000
    if age_ms < day_ms:
        return "under_24h"
    if age_ms < 7 * day_ms:
        return "one_to_seven_days"
    if age_ms < 30 * day_ms:
        return "seven_to_thirty_days"
    return "over_thirty_days"


@dataclass(frozen=True)
class BeingImportancePinV1:
    pin_id: str
    being: str
    contract_id: str
    action: str
    source_event_id: str
    source_event_sha256: str
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED: raise RecordValidationError("pins require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {"schema": "being_importance_pin_v1", "schema_version": 1,
                "pin_id": self.pin_id, "being": self.being,
                "contract_id": self.contract_id, "action": self.action,
                "source_event_id": self.source_event_id,
                "source_event_sha256": self.source_event_sha256,
                "pins_attention_only": True, "closure_propagated": False,
                "authority_propagated": False, "evidence_sufficiency_propagated": False,
                "artifact_authority_state_v1": authority_state()}

    @classmethod
    def build(cls, *, being: str, contract_id: str, action: str,
              source_event_id: str, source_event_sha256: str) -> BeingImportancePinV1:
        if being not in {"astrid", "minime"}: raise RecordValidationError("pin being must be astrid or minime")
        if action not in {"pin", "unpin"}: raise RecordValidationError("pin action must be pin or unpin")
        contract = validate_bounded_identifier(contract_id, "contract_id") or ""
        source = validate_bounded_identifier(source_event_id, "source_event_id") or ""
        source_hash = validate_sha256(source_event_sha256, "source_event_sha256") or ""
        core = {"being": being, "contract_id": contract, "action": action,
                "source_event_id": source, "source_event_sha256": source_hash}
        return cls(deterministic_id("attentionpin", core), being, contract, action,
                   source, source_hash, _TRUSTED)

    @classmethod
    def from_untrusted(cls, value: Any) -> BeingImportancePinV1:
        if not isinstance(value, dict):
            raise RecordValidationError("pin must be an object")
        validate_evidence_record(value)
        built = cls.build(**{key: value.get(key) for key in (
            "being", "contract_id", "action", "source_event_id",
            "source_event_sha256")})
        if value.get("pin_id") != built.pin_id:
            raise RecordValidationError("pin identity mismatch")
        if (
            value.get("pins_attention_only") is not True
            or value.get("closure_propagated") is not False
            or value.get("authority_propagated") is not False
            or value.get("evidence_sufficiency_propagated") is not False
        ):
            raise RecordValidationError("pin contains state propagation")
        return built


@dataclass(frozen=True)
class AttentionPortfolioEntryV1:
    contract_id: str
    slot_class: str
    rank: int
    felt_severity: int
    recurrence: int
    freshness: int
    unattended_duration_ms: int
    durable_queue_position: int
    pinned_by: tuple[str, ...]
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED: raise RecordValidationError("portfolio entries require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {"schema": "attention_portfolio_entry_v1", "schema_version": 1,
                "contract_id": self.contract_id, "slot_class": self.slot_class,
                "rank": self.rank, "felt_severity": self.felt_severity,
                "recurrence": self.recurrence, "freshness": self.freshness,
                "unattended_duration_ms": self.unattended_duration_ms,
                "durable_queue_position": self.durable_queue_position,
                "pinned_by": list(self.pinned_by),
                "membership_propagates_closure": False,
                "membership_propagates_authority": False,
                "membership_propagates_evidence_sufficiency": False,
                "membership_propagates_supersession": False,
                "artifact_authority_state_v1": authority_state()}

    @classmethod
    def build(cls, **values: Any) -> AttentionPortfolioEntryV1:
        contract = validate_bounded_identifier(values.get("contract_id"), "contract_id") or ""
        slot = str(values.get("slot_class") or "")
        if slot not in {"urgent", "astrid_pin", "minime_pin", "ranked"}: raise RecordValidationError("invalid slot class")
        numbers = {}
        for key in ("rank", "felt_severity", "recurrence", "freshness", "unattended_duration_ms", "durable_queue_position"):
            value = values.get(key)
            if isinstance(value, bool) or not isinstance(value, int) or value < 0:
                raise RecordValidationError(f"{key} must be a non-negative integer")
            numbers[key] = value
        if not 1 <= numbers["rank"] <= 16:
            raise RecordValidationError("rank must be between 1 and 16")
        if numbers["felt_severity"] > 6:
            raise RecordValidationError("felt_severity exceeds the bounded scale")
        if numbers["freshness"] not in {0, 1}:
            raise RecordValidationError("freshness must be zero or one")
        raw_pins = tuple(str(item) for item in values.get("pinned_by", ()))
        if len(set(raw_pins)) != len(raw_pins) or any(
            item not in {"astrid", "minime"} for item in raw_pins
        ):
            raise RecordValidationError("pinned_by must contain unique known beings")
        pins = raw_pins
        return cls(contract, slot, numbers["rank"], numbers["felt_severity"],
                   numbers["recurrence"], numbers["freshness"],
                   numbers["unattended_duration_ms"], numbers["durable_queue_position"],
                   pins, _TRUSTED)

    @classmethod
    def from_untrusted(cls, value: Any) -> AttentionPortfolioEntryV1:
        if not isinstance(value, dict):
            raise RecordValidationError("portfolio entry must be an object")
        validate_evidence_record(value)
        built = cls.build(**value)
        for key in (
            "membership_propagates_closure",
            "membership_propagates_authority",
            "membership_propagates_evidence_sufficiency",
            "membership_propagates_supersession",
        ):
            if value.get(key) is not False:
                raise RecordValidationError("portfolio entry contains state propagation")
        return built


@dataclass(frozen=True)
class AttentionPortfolioV1:
    portfolio_id: str
    source_contracts_sha256: str
    active_cap: int
    entries: tuple[AttentionPortfolioEntryV1, ...]
    overflow_contract_ids: tuple[str, ...]
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED: raise RecordValidationError("portfolio requires the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {"schema": "attention_portfolio_v1", "schema_version": 1,
                "portfolio_id": self.portfolio_id,
                "source_contracts_sha256": self.source_contracts_sha256,
                "active_cap": self.active_cap,
                "entries": [item.to_dict() for item in self.entries],
                "overflow_contract_ids": list(self.overflow_contract_ids),
                "every_claim_preserved_under_contract_graph": True,
                "membership_propagates_closure": False,
                "membership_propagates_authority": False,
                "membership_propagates_evidence_sufficiency": False,
                "membership_propagates_supersession": False,
                "artifact_authority_state_v1": authority_state()}

    @classmethod
    def build(cls, *, source_contracts_sha256: str,
              entries: list[AttentionPortfolioEntryV1],
              overflow_contract_ids: list[str]) -> AttentionPortfolioV1:
        source_hash = validate_sha256(source_contracts_sha256, "source_contracts_sha256") or ""
        if len(entries) > 16: raise RecordValidationError("attention portfolio exceeds 16 contracts")
        ids = [item.contract_id for item in entries]
        if len(set(ids)) != len(ids): raise RecordValidationError("attention portfolio contains duplicates")
        overflow = tuple(validate_bounded_identifier(item, "overflow_contract_id") or "" for item in overflow_contract_ids)
        if len(set(overflow)) != len(overflow):
            raise RecordValidationError("attention overflow contains duplicates")
        if set(ids).intersection(overflow):
            raise RecordValidationError("attention overflow overlaps active entries")
        core = {"source_contracts_sha256": source_hash,
                "entries": [item.to_dict() for item in entries],
                "overflow_contract_ids": list(overflow), "active_cap": 16}
        return cls(deterministic_id("attentionportfolio", core), source_hash, 16,
                   tuple(entries), overflow, _TRUSTED)

    @classmethod
    def from_untrusted(cls, value: Any) -> AttentionPortfolioV1:
        if not isinstance(value, dict):
            raise RecordValidationError("portfolio must be an object")
        validate_evidence_record(value)
        entries = [
            AttentionPortfolioEntryV1.from_untrusted(item)
            for item in value.get("entries") or []
        ]
        built = cls.build(
            source_contracts_sha256=value.get("source_contracts_sha256"),
            entries=entries,
            overflow_contract_ids=list(value.get("overflow_contract_ids") or []),
        )
        if value.get("portfolio_id") != built.portfolio_id or value.get("active_cap") != 16:
            raise RecordValidationError("portfolio identity or capacity mismatch")
        if value.get("every_claim_preserved_under_contract_graph") is not True:
            raise RecordValidationError("portfolio does not preserve the contract graph")
        for key in (
            "membership_propagates_closure",
            "membership_propagates_authority",
            "membership_propagates_evidence_sufficiency",
            "membership_propagates_supersession",
        ):
            if value.get(key) is not False:
                raise RecordValidationError("portfolio contains state propagation")
        return built


@dataclass(frozen=True)
class AttentionSelectionReceiptV1:
    receipt_id: str
    portfolio_id: str
    selected_contract_ids: tuple[str, ...]
    overflow_count: int
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED: raise RecordValidationError("selection receipts require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {"schema": "attention_selection_receipt_v1", "schema_version": 1,
                "receipt_id": self.receipt_id, "portfolio_id": self.portfolio_id,
                "selected_contract_ids": list(self.selected_contract_ids),
                "overflow_count": self.overflow_count,
                "selection_is_attention_only": True,
                "closure_propagated": False, "authority_propagated": False,
                "evidence_sufficiency_propagated": False,
                "artifact_authority_state_v1": authority_state()}

    @classmethod
    def from_portfolio(cls, portfolio: AttentionPortfolioV1) -> AttentionSelectionReceiptV1:
        selected = tuple(item.contract_id for item in portfolio.entries)
        core = {"portfolio_id": portfolio.portfolio_id,
                "selected_contract_ids": list(selected),
                "overflow_count": len(portfolio.overflow_contract_ids)}
        return cls(deterministic_id("attentionselection", core), portfolio.portfolio_id,
                   selected, len(portfolio.overflow_contract_ids), _TRUSTED)

    @classmethod
    def from_untrusted(cls, value: Any) -> AttentionSelectionReceiptV1:
        if not isinstance(value, dict):
            raise RecordValidationError("selection receipt must be an object")
        validate_evidence_record(value)
        portfolio = validate_bounded_identifier(
            value.get("portfolio_id"), "portfolio_id"
        ) or ""
        selected = tuple(
            validate_bounded_identifier(item, "selected_contract_id") or ""
            for item in value.get("selected_contract_ids") or []
        )
        if len(selected) > 16 or len(set(selected)) != len(selected):
            raise RecordValidationError("selection must contain at most 16 unique contracts")
        overflow = value.get("overflow_count")
        if isinstance(overflow, bool) or not isinstance(overflow, int) or overflow < 0:
            raise RecordValidationError("overflow_count must be non-negative")
        core = {
            "portfolio_id": portfolio,
            "selected_contract_ids": list(selected),
            "overflow_count": overflow,
        }
        expected = deterministic_id("attentionselection", core)
        if value.get("receipt_id") != expected:
            raise RecordValidationError("selection receipt identity mismatch")
        if (
            value.get("selection_is_attention_only") is not True
            or value.get("closure_propagated") is not False
            or value.get("authority_propagated") is not False
            or value.get("evidence_sufficiency_propagated") is not False
        ):
            raise RecordValidationError("selection receipt contains state propagation")
        return cls(expected, portfolio, selected, overflow, _TRUSTED)


@dataclass(frozen=True)
class AttentionPortfolioEntryV2:
    contract_id: str
    steward_slot_class: str
    selection_rank: int
    contract_review_state_class: str
    claim_recurrence_count: int
    source_signal_recency_class: str
    steward_unaddressed_age_band: str
    canonical_queue_tiebreaker: int
    pinned_by: tuple[str, ...]
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("portfolio entries require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "attention_portfolio_entry_v2",
            "schema_version": 2,
            "contract_id": self.contract_id,
            "steward_slot_class": self.steward_slot_class,
            "selection_rank": self.selection_rank,
            "contract_review_state_class": self.contract_review_state_class,
            "claim_recurrence_count": self.claim_recurrence_count,
            "source_signal_recency_class": self.source_signal_recency_class,
            "steward_unaddressed_age_band": self.steward_unaddressed_age_band,
            "canonical_queue_tiebreaker": self.canonical_queue_tiebreaker,
            "pinned_by": list(self.pinned_by),
            "selection_scope": STEWARD_SELECTION_SCOPE_V2,
            "contract_state_relation": CONTRACT_STATE_RELATION_V2,
            "runtime_relation": RUNTIME_RELATION_V2,
            "authority_relation": AUTHORITY_RELATION_V2,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def build(cls, **values: Any) -> AttentionPortfolioEntryV2:
        contract = validate_bounded_identifier(values.get("contract_id"), "contract_id") or ""
        slot = str(values.get("steward_slot_class") or "")
        if slot not in {"urgent", "astrid_pin", "minime_pin", "ranked"}:
            raise RecordValidationError("invalid steward slot class")
        rank = values.get("selection_rank")
        recurrence = values.get("claim_recurrence_count")
        queue = values.get("canonical_queue_tiebreaker")
        for key, value in (
            ("selection_rank", rank),
            ("claim_recurrence_count", recurrence),
            ("canonical_queue_tiebreaker", queue),
        ):
            if isinstance(value, bool) or not isinstance(value, int) or value < 0:
                raise RecordValidationError(f"{key} must be a non-negative integer")
        if not 1 <= rank <= 16:
            raise RecordValidationError("selection_rank must be between 1 and 16")
        review_class = str(values.get("contract_review_state_class") or "")
        if review_class not in REVIEW_STATE_CLASSES_V2:
            raise RecordValidationError("invalid contract review state class")
        recency_class = str(values.get("source_signal_recency_class") or "")
        if recency_class not in RECENCY_CLASSES_V2:
            raise RecordValidationError("invalid source signal recency class")
        age_band = str(values.get("steward_unaddressed_age_band") or "")
        if age_band not in AGE_BANDS_V2:
            raise RecordValidationError("invalid steward unaddressed age band")
        raw_pins = tuple(str(item) for item in values.get("pinned_by", ()))
        if len(set(raw_pins)) != len(raw_pins) or any(
            item not in {"astrid", "minime"} for item in raw_pins
        ):
            raise RecordValidationError("pinned_by must contain unique known beings")
        return cls(
            contract,
            slot,
            rank,
            review_class,
            recurrence,
            recency_class,
            age_band,
            queue,
            raw_pins,
            _TRUSTED,
        )

    @classmethod
    def from_untrusted(cls, value: Any) -> AttentionPortfolioEntryV2:
        if not isinstance(value, dict):
            raise RecordValidationError("portfolio entry must be an object")
        validate_evidence_record(value)
        _require_exact_keys(
            value,
            frozenset(
                {
                    "schema",
                    "schema_version",
                    "contract_id",
                    "steward_slot_class",
                    "selection_rank",
                    "contract_review_state_class",
                    "claim_recurrence_count",
                    "source_signal_recency_class",
                    "steward_unaddressed_age_band",
                    "canonical_queue_tiebreaker",
                    "pinned_by",
                    "selection_scope",
                    "contract_state_relation",
                    "runtime_relation",
                    "authority_relation",
                    "artifact_authority_state_v1",
                }
            ),
            "attention portfolio entry V2",
        )
        if value.get("schema") != "attention_portfolio_entry_v2" or value.get("schema_version") != 2:
            raise RecordValidationError("unsupported attention portfolio entry schema")
        if (
            value.get("selection_scope") != STEWARD_SELECTION_SCOPE_V2
            or value.get("contract_state_relation") != CONTRACT_STATE_RELATION_V2
            or value.get("runtime_relation") != RUNTIME_RELATION_V2
            or value.get("authority_relation") != AUTHORITY_RELATION_V2
        ):
            raise RecordValidationError("portfolio entry exceeds steward work-selection scope")
        return cls.build(**value)

    @classmethod
    def from_legacy_v1(cls, value: Any) -> AttentionPortfolioEntryV2:
        legacy = AttentionPortfolioEntryV1.from_untrusted(value)
        review_class = {
            6: "contradiction_or_named_objection",
            5: "reopened_or_still_friction",
            3: "review_pending",
            1: "open",
        }.get(legacy.felt_severity, "quiet")
        return cls.build(
            contract_id=legacy.contract_id,
            steward_slot_class=legacy.slot_class,
            selection_rank=legacy.rank,
            contract_review_state_class=review_class,
            claim_recurrence_count=legacy.recurrence,
            source_signal_recency_class="recent_24h" if legacy.freshness else "older",
            steward_unaddressed_age_band=steward_unaddressed_age_band(
                legacy.unattended_duration_ms
            ),
            canonical_queue_tiebreaker=legacy.durable_queue_position,
            pinned_by=legacy.pinned_by,
        )


@dataclass(frozen=True)
class AttentionPortfolioV2:
    portfolio_id: str
    source_contracts_sha256: str
    steward_selected_work_limit: int
    selected_entries: tuple[AttentionPortfolioEntryV2, ...]
    visible_urgent_alert_contract_ids: tuple[str, ...]
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("portfolio requires the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "attention_portfolio_v2",
            "schema_version": 2,
            "portfolio_id": self.portfolio_id,
            "source_contracts_sha256": self.source_contracts_sha256,
            "steward_selected_work_limit": self.steward_selected_work_limit,
            "selected_entries": [item.to_dict() for item in self.selected_entries],
            "visible_urgent_alert_contract_ids": list(
                self.visible_urgent_alert_contract_ids
            ),
            "selection_scope": STEWARD_SELECTION_SCOPE_V2,
            "source_graph_relation": SOURCE_GRAPH_RELATION_V2,
            "unselected_contract_relation": UNSELECTED_RELATION_V2,
            "contract_state_relation": CONTRACT_STATE_RELATION_V2,
            "runtime_relation": RUNTIME_RELATION_V2,
            "authority_relation": AUTHORITY_RELATION_V2,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def build(
        cls,
        *,
        source_contracts_sha256: str,
        selected_entries: list[AttentionPortfolioEntryV2],
        visible_urgent_alert_contract_ids: list[str],
    ) -> AttentionPortfolioV2:
        source_hash = validate_sha256(source_contracts_sha256, "source_contracts_sha256") or ""
        if len(selected_entries) > 16:
            raise RecordValidationError("steward work view exceeds 16 contracts")
        ids = [item.contract_id for item in selected_entries]
        if len(set(ids)) != len(ids):
            raise RecordValidationError("steward work view contains duplicates")
        alerts = tuple(
            validate_bounded_identifier(item, "visible_urgent_alert_contract_id") or ""
            for item in visible_urgent_alert_contract_ids
        )
        if len(set(alerts)) != len(alerts):
            raise RecordValidationError("visible urgent alert contains duplicates")
        if set(ids).intersection(alerts):
            raise RecordValidationError("visible urgent alert overlaps selected work")
        core = {
            "source_contracts_sha256": source_hash,
            "selected_entries": [item.to_dict() for item in selected_entries],
            "visible_urgent_alert_contract_ids": list(alerts),
            "steward_selected_work_limit": 16,
        }
        return cls(
            deterministic_id("attentionportfolio", core),
            source_hash,
            16,
            tuple(selected_entries),
            alerts,
            _TRUSTED,
        )

    @classmethod
    def from_untrusted(cls, value: Any) -> AttentionPortfolioV2:
        if not isinstance(value, dict):
            raise RecordValidationError("portfolio must be an object")
        validate_evidence_record(value)
        _require_exact_keys(
            value,
            frozenset(
                {
                    "schema",
                    "schema_version",
                    "portfolio_id",
                    "source_contracts_sha256",
                    "steward_selected_work_limit",
                    "selected_entries",
                    "visible_urgent_alert_contract_ids",
                    "selection_scope",
                    "source_graph_relation",
                    "unselected_contract_relation",
                    "contract_state_relation",
                    "runtime_relation",
                    "authority_relation",
                    "artifact_authority_state_v1",
                }
            ),
            "attention portfolio V2",
        )
        if value.get("schema") != "attention_portfolio_v2" or value.get("schema_version") != 2:
            raise RecordValidationError("unsupported attention portfolio schema")
        if (
            value.get("selection_scope") != STEWARD_SELECTION_SCOPE_V2
            or value.get("source_graph_relation") != SOURCE_GRAPH_RELATION_V2
            or value.get("unselected_contract_relation") != UNSELECTED_RELATION_V2
            or value.get("contract_state_relation") != CONTRACT_STATE_RELATION_V2
            or value.get("runtime_relation") != RUNTIME_RELATION_V2
            or value.get("authority_relation") != AUTHORITY_RELATION_V2
        ):
            raise RecordValidationError("portfolio exceeds steward work-selection scope")
        entries = [
            AttentionPortfolioEntryV2.from_untrusted(item)
            for item in value.get("selected_entries") or []
        ]
        built = cls.build(
            source_contracts_sha256=value.get("source_contracts_sha256"),
            selected_entries=entries,
            visible_urgent_alert_contract_ids=list(
                value.get("visible_urgent_alert_contract_ids") or []
            ),
        )
        if (
            value.get("portfolio_id") != built.portfolio_id
            or value.get("steward_selected_work_limit") != 16
        ):
            raise RecordValidationError("portfolio identity or steward work limit mismatch")
        return built

    @classmethod
    def from_legacy_v1(cls, value: Any) -> AttentionPortfolioV2:
        legacy = AttentionPortfolioV1.from_untrusted(value)
        return cls.build(
            source_contracts_sha256=legacy.source_contracts_sha256,
            selected_entries=[
                AttentionPortfolioEntryV2.from_legacy_v1(item.to_dict())
                for item in legacy.entries
            ],
            visible_urgent_alert_contract_ids=list(legacy.overflow_contract_ids),
        )


@dataclass(frozen=True)
class AttentionSelectionReceiptV2:
    receipt_id: str
    portfolio_id: str
    selected_contract_ids: tuple[str, ...]
    visible_urgent_alert_count: int
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("selection receipts require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "attention_selection_receipt_v2",
            "schema_version": 2,
            "receipt_id": self.receipt_id,
            "portfolio_id": self.portfolio_id,
            "selected_contract_ids": list(self.selected_contract_ids),
            "visible_urgent_alert_count": self.visible_urgent_alert_count,
            "selection_scope": STEWARD_SELECTION_SCOPE_V2,
            "contract_state_relation": CONTRACT_STATE_RELATION_V2,
            "runtime_relation": RUNTIME_RELATION_V2,
            "authority_relation": AUTHORITY_RELATION_V2,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_portfolio(cls, portfolio: AttentionPortfolioV2) -> AttentionSelectionReceiptV2:
        selected = tuple(item.contract_id for item in portfolio.selected_entries)
        core = {
            "portfolio_id": portfolio.portfolio_id,
            "selected_contract_ids": list(selected),
            "visible_urgent_alert_count": len(portfolio.visible_urgent_alert_contract_ids),
        }
        return cls(
            deterministic_id("attentionselection", core),
            portfolio.portfolio_id,
            selected,
            len(portfolio.visible_urgent_alert_contract_ids),
            _TRUSTED,
        )

    @classmethod
    def from_untrusted(cls, value: Any) -> AttentionSelectionReceiptV2:
        if not isinstance(value, dict):
            raise RecordValidationError("selection receipt must be an object")
        validate_evidence_record(value)
        _require_exact_keys(
            value,
            frozenset(
                {
                    "schema",
                    "schema_version",
                    "receipt_id",
                    "portfolio_id",
                    "selected_contract_ids",
                    "visible_urgent_alert_count",
                    "selection_scope",
                    "contract_state_relation",
                    "runtime_relation",
                    "authority_relation",
                    "artifact_authority_state_v1",
                }
            ),
            "attention selection receipt V2",
        )
        if value.get("schema") != "attention_selection_receipt_v2" or value.get("schema_version") != 2:
            raise RecordValidationError("unsupported attention selection receipt schema")
        if (
            value.get("selection_scope") != STEWARD_SELECTION_SCOPE_V2
            or value.get("contract_state_relation") != CONTRACT_STATE_RELATION_V2
            or value.get("runtime_relation") != RUNTIME_RELATION_V2
            or value.get("authority_relation") != AUTHORITY_RELATION_V2
        ):
            raise RecordValidationError("selection receipt exceeds steward work-selection scope")
        portfolio = validate_bounded_identifier(value.get("portfolio_id"), "portfolio_id") or ""
        selected = tuple(
            validate_bounded_identifier(item, "selected_contract_id") or ""
            for item in value.get("selected_contract_ids") or []
        )
        if len(selected) > 16 or len(set(selected)) != len(selected):
            raise RecordValidationError("selection must contain at most 16 unique contracts")
        alert_count = value.get("visible_urgent_alert_count")
        if isinstance(alert_count, bool) or not isinstance(alert_count, int) or alert_count < 0:
            raise RecordValidationError("visible urgent alert count must be non-negative")
        core = {
            "portfolio_id": portfolio,
            "selected_contract_ids": list(selected),
            "visible_urgent_alert_count": alert_count,
        }
        expected = deterministic_id("attentionselection", core)
        if value.get("receipt_id") != expected:
            raise RecordValidationError("selection receipt identity mismatch")
        return cls(expected, portfolio, selected, alert_count, _TRUSTED)
