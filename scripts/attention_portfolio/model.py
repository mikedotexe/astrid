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
