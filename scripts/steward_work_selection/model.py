"""Validated records for external steward work selection.

These records describe a steward's bounded work queue. They deliberately do
not model, classify, score, or make neutrality claims about a being's felt
state or internal orientation.
"""

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

SELECTION_SCOPE = "steward_work_selection_not_being_state"
EXPERIENTIAL_RELATION = "no_claim_about_being_felt_effect"
RUNTIME_RELATION = (
    "not_consumed_by_bridge_minime_model_scheduler_or_control_runtime"
)
AUTHORITY_RELATION = "selection_grants_no_runtime_authority"
SOURCE_GRAPH_RELATION = "all_contracts_claims_cards_and_evidence_remain_queryable"
SELECTION_SOURCES = frozenset(
    {
        "source_objection_or_reopen",
        "astrid_priority_pin",
        "minime_priority_pin",
        "deterministic_steward_policy",
    }
)


def _require_exact_keys(
    value: dict[str, Any], allowed: frozenset[str], record_name: str
) -> None:
    unexpected = sorted(set(value) - allowed)
    missing = sorted(allowed - set(value))
    if unexpected or missing:
        detail = []
        if unexpected:
            detail.append(f"unsupported: {', '.join(unexpected)}")
        if missing:
            detail.append(f"missing: {', '.join(missing)}")
        raise RecordValidationError(f"{record_name} fields invalid ({'; '.join(detail)})")


@dataclass(frozen=True)
class OwnerPriorityPinV1:
    pin_id: str
    owner: str
    contract_id: str
    action: str
    source_event_id: str
    source_event_sha256: str
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("priority pins require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "owner_priority_pin_v1",
            "schema_version": 1,
            "pin_id": self.pin_id,
            "owner": self.owner,
            "contract_id": self.contract_id,
            "action": self.action,
            "source_event_id": self.source_event_id,
            "source_event_sha256": self.source_event_sha256,
            "selection_scope": SELECTION_SCOPE,
            "experiential_relation": EXPERIENTIAL_RELATION,
            "runtime_relation": RUNTIME_RELATION,
            "authority_relation": AUTHORITY_RELATION,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def build(
        cls,
        *,
        owner: str,
        contract_id: str,
        action: str,
        source_event_id: str,
        source_event_sha256: str,
    ) -> OwnerPriorityPinV1:
        if owner not in {"astrid", "minime"}:
            raise RecordValidationError("pin owner must be astrid or minime")
        if action not in {"pin", "unpin"}:
            raise RecordValidationError("pin action must be pin or unpin")
        contract = validate_bounded_identifier(contract_id, "contract_id") or ""
        source = validate_bounded_identifier(source_event_id, "source_event_id") or ""
        source_hash = validate_sha256(
            source_event_sha256, "source_event_sha256"
        ) or ""
        core = {
            "owner": owner,
            "contract_id": contract,
            "action": action,
            "source_event_id": source,
            "source_event_sha256": source_hash,
        }
        return cls(
            deterministic_id("ownerprioritypin", core),
            owner,
            contract,
            action,
            source,
            source_hash,
            _TRUSTED,
        )

    @classmethod
    def from_untrusted(cls, value: Any) -> OwnerPriorityPinV1:
        if not isinstance(value, dict):
            raise RecordValidationError("priority pin must be an object")
        validate_evidence_record(value)
        _require_exact_keys(
            value,
            frozenset(
                {
                    "schema",
                    "schema_version",
                    "pin_id",
                    "owner",
                    "contract_id",
                    "action",
                    "source_event_id",
                    "source_event_sha256",
                    "selection_scope",
                    "experiential_relation",
                    "runtime_relation",
                    "authority_relation",
                    "artifact_authority_state_v1",
                }
            ),
            "owner priority pin",
        )
        if value.get("schema") != "owner_priority_pin_v1" or value.get(
            "schema_version"
        ) != 1:
            raise RecordValidationError("unsupported priority pin schema")
        _validate_boundary(value)
        built = cls.build(
            owner=value.get("owner"),
            contract_id=value.get("contract_id"),
            action=value.get("action"),
            source_event_id=value.get("source_event_id"),
            source_event_sha256=value.get("source_event_sha256"),
        )
        if value.get("pin_id") != built.pin_id:
            raise RecordValidationError("priority pin identity mismatch")
        return built


@dataclass(frozen=True)
class StewardWorkSelectionEntryV1:
    contract_id: str
    selection_source: str
    source_ref: str
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("selection entries require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "steward_work_selection_entry_v1",
            "schema_version": 1,
            "contract_id": self.contract_id,
            "selection_source": self.selection_source,
            "source_ref": self.source_ref,
        }

    @classmethod
    def build(
        cls, *, contract_id: str, selection_source: str, source_ref: str
    ) -> StewardWorkSelectionEntryV1:
        contract = validate_bounded_identifier(contract_id, "contract_id") or ""
        if selection_source not in SELECTION_SOURCES:
            raise RecordValidationError("invalid steward selection source")
        source = validate_bounded_identifier(source_ref, "source_ref") or ""
        return cls(contract, selection_source, source, _TRUSTED)

    @classmethod
    def from_untrusted(cls, value: Any) -> StewardWorkSelectionEntryV1:
        if not isinstance(value, dict):
            raise RecordValidationError("selection entry must be an object")
        _require_exact_keys(
            value,
            frozenset(
                {
                    "schema",
                    "schema_version",
                    "contract_id",
                    "selection_source",
                    "source_ref",
                }
            ),
            "steward work selection entry",
        )
        if value.get("schema") != "steward_work_selection_entry_v1" or value.get(
            "schema_version"
        ) != 1:
            raise RecordValidationError("unsupported selection entry schema")
        return cls.build(
            contract_id=value.get("contract_id"),
            selection_source=value.get("selection_source"),
            source_ref=value.get("source_ref"),
        )


@dataclass(frozen=True)
class StewardWorkSelectionV1:
    selection_id: str
    policy_id: str
    source_contracts_sha256: str
    selected_entries: tuple[StewardWorkSelectionEntryV1, ...]
    visible_urgent_alert_contract_ids: tuple[str, ...]
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("work selections require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "steward_work_selection_v1",
            "schema_version": 1,
            "selection_id": self.selection_id,
            "policy_id": self.policy_id,
            "source_contracts_sha256": self.source_contracts_sha256,
            "selected_entries": [item.to_dict() for item in self.selected_entries],
            "visible_urgent_alert_contract_ids": list(
                self.visible_urgent_alert_contract_ids
            ),
            "selection_scope": SELECTION_SCOPE,
            "experiential_relation": EXPERIENTIAL_RELATION,
            "runtime_relation": RUNTIME_RELATION,
            "authority_relation": AUTHORITY_RELATION,
            "source_graph_relation": SOURCE_GRAPH_RELATION,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def build(
        cls,
        *,
        policy_id: str,
        source_contracts_sha256: str,
        selected_entries: list[StewardWorkSelectionEntryV1],
        visible_urgent_alert_contract_ids: list[str],
    ) -> StewardWorkSelectionV1:
        policy = validate_bounded_identifier(policy_id, "policy_id") or ""
        source_hash = validate_sha256(
            source_contracts_sha256, "source_contracts_sha256"
        ) or ""
        if len(selected_entries) > 16:
            raise RecordValidationError("steward work selection exceeds policy batch")
        selected_ids = [item.contract_id for item in selected_entries]
        if len(set(selected_ids)) != len(selected_ids):
            raise RecordValidationError("steward work selection contains duplicates")
        alerts = tuple(
            validate_bounded_identifier(item, "visible_urgent_alert_contract_id")
            or ""
            for item in visible_urgent_alert_contract_ids
        )
        if len(set(alerts)) != len(alerts):
            raise RecordValidationError("visible urgent alerts contain duplicates")
        if set(selected_ids).intersection(alerts):
            raise RecordValidationError("visible urgent alerts overlap selected work")
        core = {
            "policy_id": policy,
            "source_contracts_sha256": source_hash,
            "selected_entries": [item.to_dict() for item in selected_entries],
            "visible_urgent_alert_contract_ids": list(alerts),
        }
        return cls(
            deterministic_id("stewardworkselection", core),
            policy,
            source_hash,
            tuple(selected_entries),
            alerts,
            _TRUSTED,
        )

    @classmethod
    def from_untrusted(cls, value: Any) -> StewardWorkSelectionV1:
        if not isinstance(value, dict):
            raise RecordValidationError("steward work selection must be an object")
        validate_evidence_record(value)
        _require_exact_keys(
            value,
            frozenset(
                {
                    "schema",
                    "schema_version",
                    "selection_id",
                    "policy_id",
                    "source_contracts_sha256",
                    "selected_entries",
                    "visible_urgent_alert_contract_ids",
                    "selection_scope",
                    "experiential_relation",
                    "runtime_relation",
                    "authority_relation",
                    "source_graph_relation",
                    "artifact_authority_state_v1",
                }
            ),
            "steward work selection",
        )
        if value.get("schema") != "steward_work_selection_v1" or value.get(
            "schema_version"
        ) != 1:
            raise RecordValidationError("unsupported steward work selection schema")
        _validate_boundary(value)
        if value.get("source_graph_relation") != SOURCE_GRAPH_RELATION:
            raise RecordValidationError("selection does not preserve source graph access")
        entries = [
            StewardWorkSelectionEntryV1.from_untrusted(item)
            for item in value.get("selected_entries") or []
        ]
        built = cls.build(
            policy_id=value.get("policy_id"),
            source_contracts_sha256=value.get("source_contracts_sha256"),
            selected_entries=entries,
            visible_urgent_alert_contract_ids=list(
                value.get("visible_urgent_alert_contract_ids") or []
            ),
        )
        if value.get("selection_id") != built.selection_id:
            raise RecordValidationError("steward work selection identity mismatch")
        return built


@dataclass(frozen=True)
class StewardWorkSelectionReceiptV1:
    receipt_id: str
    selection_id: str
    selected_contract_ids: tuple[str, ...]
    visible_urgent_alert_contract_ids: tuple[str, ...]
    _token: object = field(repr=False, compare=False)

    def __post_init__(self) -> None:
        if self._token is not _TRUSTED:
            raise RecordValidationError("selection receipts require the internal builder")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "steward_work_selection_receipt_v1",
            "schema_version": 1,
            "receipt_id": self.receipt_id,
            "selection_id": self.selection_id,
            "selected_contract_ids": list(self.selected_contract_ids),
            "visible_urgent_alert_contract_ids": list(
                self.visible_urgent_alert_contract_ids
            ),
            "selection_scope": SELECTION_SCOPE,
            "experiential_relation": EXPERIENTIAL_RELATION,
            "runtime_relation": RUNTIME_RELATION,
            "authority_relation": AUTHORITY_RELATION,
            "artifact_authority_state_v1": authority_state(),
        }

    @classmethod
    def from_selection(
        cls, selection: StewardWorkSelectionV1
    ) -> StewardWorkSelectionReceiptV1:
        selected = tuple(item.contract_id for item in selection.selected_entries)
        alerts = selection.visible_urgent_alert_contract_ids
        core = {
            "selection_id": selection.selection_id,
            "selected_contract_ids": list(selected),
            "visible_urgent_alert_contract_ids": list(alerts),
        }
        return cls(
            deterministic_id("stewardworkselectionreceipt", core),
            selection.selection_id,
            selected,
            alerts,
            _TRUSTED,
        )

    @classmethod
    def from_untrusted(cls, value: Any) -> StewardWorkSelectionReceiptV1:
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
                    "selection_id",
                    "selected_contract_ids",
                    "visible_urgent_alert_contract_ids",
                    "selection_scope",
                    "experiential_relation",
                    "runtime_relation",
                    "authority_relation",
                    "artifact_authority_state_v1",
                }
            ),
            "steward work selection receipt",
        )
        if value.get("schema") != "steward_work_selection_receipt_v1" or value.get(
            "schema_version"
        ) != 1:
            raise RecordValidationError("unsupported selection receipt schema")
        _validate_boundary(value)
        selection_id = validate_bounded_identifier(
            value.get("selection_id"), "selection_id"
        ) or ""
        selected = tuple(
            validate_bounded_identifier(item, "selected_contract_id") or ""
            for item in value.get("selected_contract_ids") or []
        )
        alerts = tuple(
            validate_bounded_identifier(item, "visible_urgent_alert_contract_id")
            or ""
            for item in value.get("visible_urgent_alert_contract_ids") or []
        )
        if len(selected) > 16 or len(set(selected)) != len(selected):
            raise RecordValidationError("selection receipt contains invalid selected IDs")
        if len(set(alerts)) != len(alerts) or set(selected).intersection(alerts):
            raise RecordValidationError("selection receipt contains invalid alert IDs")
        core = {
            "selection_id": selection_id,
            "selected_contract_ids": list(selected),
            "visible_urgent_alert_contract_ids": list(alerts),
        }
        expected = deterministic_id("stewardworkselectionreceipt", core)
        if value.get("receipt_id") != expected:
            raise RecordValidationError("selection receipt identity mismatch")
        return cls(expected, selection_id, selected, alerts, _TRUSTED)


def _validate_boundary(value: dict[str, Any]) -> None:
    if value.get("selection_scope") != SELECTION_SCOPE:
        raise RecordValidationError("record exceeds external steward selection scope")
    if value.get("experiential_relation") != EXPERIENTIAL_RELATION:
        raise RecordValidationError("record makes an unsupported felt-effect claim")
    if value.get("runtime_relation") != RUNTIME_RELATION:
        raise RecordValidationError("record claims a runtime consumer")
    if value.get("authority_relation") != AUTHORITY_RELATION:
        raise RecordValidationError("record claims runtime authority")
