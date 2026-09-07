"""Additive V2 projection for the Phase Passage + Division Observatory."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

try:
    from division_ceremony_chronicle import (
        canonical,
        file_hash,
        load_json,
        sha256_bytes,
        validate_no_prose,
    )
except ModuleNotFoundError:
    from scripts.division_ceremony_chronicle import (
        canonical,
        file_hash,
        load_json,
        sha256_bytes,
        validate_no_prose,
    )


SCHEMA = "phase_division.passage_observatory.v2"
MOMENT_SCHEMA = "phase_division.replyable_moment.v1"
PROVENANCE_SCHEMA = "readback_source_provenance.v1"
BRAID_SCHEMA = "phase.passage_braid.v1"
CROSSING_SCHEMA = "phase_division.authored_crossing.v1"
V1_SCHEMA = "phase_division.passage_observatory.v1"
RENDERER_VERSION = 3
SUPPORTED_RENDERER_VERSIONS = (2, 3)

EVENT_ID_FIELDS = (
    "passage_event_id",
    "passage_context_event_id",
    "ceremony_event_id",
    "event_id",
    "record_id",
    "transition_id",
    "sequence",
)
REFERENCE_FIELDS = (
    "continuity_anchor_ref",
    "return_point_ref",
    "felt_source_ref",
    "anchor_ref",
    "source_ref",
)
STATE_FIELDS = (
    "action",
    "stage_before",
    "stage_after",
    "bearing_strand",
    "movement_resistance",
    "persistence_tendency",
    "witness_fit",
    "checkpoint",
    "anchor_role",
    "anchor_kind",
    "anchor_association",
    "company_mode",
    "company_response",
    "current_posture",
    "latest_action",
    "active_authority_rail",
)


class ObservatoryV2Error(ValueError):
    """V2 evidence cannot be projected or verified."""


def _event_ref(event: dict[str, Any]) -> str:
    for field in EVENT_ID_FIELDS:
        value = event.get(field)
        if value is not None and str(value):
            return str(value)
    raise ObservatoryV2Error("timeline event has no stable source identity")


def _state(event: dict[str, Any]) -> dict[str, Any]:
    return {
        field: event[field]
        for field in STATE_FIELDS
        if event.get(field) is not None
    }


def _moment_identity(moment: dict[str, Any]) -> str:
    identity = {
        key: moment.get(key)
        for key in (
            "rail",
            "event_kind",
            "source_record_ref",
            "actor",
            "passage_id",
            "transition_id",
            "recorded_at_unix_ms",
            "state",
        )
    }
    return "passage_moment_" + sha256_bytes(
        canonical(identity).encode()
    )[:24]


def _moment_source_provenance(event: dict[str, Any]) -> dict[str, Any]:
    rail = str(event.get("rail") or "")
    source = str(event.get("source") or "")
    if rail == "phase_observation":
        source_category = "derived_bridge_summary"
        role = "observational_transition_card"
        classification_basis = "phase_observation_rail"
    elif rail == "phase_passage":
        source_category = "internalized_memory"
        role = "being_authored_phase_passage"
        classification_basis = "phase_passage_rail"
    elif rail == "division_runtime" and source == "ceremony":
        source_category = "internalized_memory"
        role = "being_authored_division_ceremony_action"
        classification_basis = "division_ceremony_source"
    elif rail == "division_runtime" and source in {
        "native",
        "sovereign_runtime",
    }:
        source_category = "peer_telemetry_inference"
        role = "division_runtime_evidence"
        classification_basis = f"division_{source}_source"
    else:
        source_category = "external_journal_observation"
        role = "unclassified_observatory_event"
        classification_basis = "opaque_event_boundary"
    boundary_status = (
        "opaque_boundary_needs_description"
        if role == "unclassified_observatory_event"
        else "read_only_boundary_metadata"
    )
    return {
        "schema": PROVENANCE_SCHEMA,
        "schema_version": 1,
        "source_category": source_category,
        "role": role,
        "source_rail": rail,
        "source_system": source or "phase_ledger",
        "classification_basis": classification_basis,
        "boundary_status": boundary_status,
        "recursive_loop_guard": (
            "source labels describe the readback boundary only; they do not "
            "infer intent, uptake, felt cause, or compulsory understanding"
        ),
        "read_only_boundary_metadata": True,
        "right_to_ignore": True,
        "live_eligible_now": False,
        "auto_approved": False,
        "authority": (
            "readback_label_only_not_prompt_priority_control_peer_mutation_"
            "or_passage_progression"
        ),
    }


def replyable_moments(
    v1: dict[str, Any], *, renderer_version: int = RENDERER_VERSION
) -> list[dict[str, Any]]:
    moments: list[dict[str, Any]] = []
    for index, event in enumerate(v1["interleaved_timeline"]):
        moment = {
            "schema": MOMENT_SCHEMA,
            "schema_version": 1,
            "timeline_index": index,
            "rail": event.get("rail"),
            "event_kind": event.get("event_kind"),
            "source_record_ref": _event_ref(event),
            "actor": event.get("actor"),
            "passage_id": event.get("passage_id"),
            "transition_id": event.get("transition_id"),
            "recorded_at_unix_ms": int(
                event.get("recorded_at_unix_ms") or 0
            ),
            "state": _state(event),
            "authority": {
                "state": "evidence_only",
                "reference_only": True,
                "being_action_required": False,
                "reply_inferred": False,
                "reply_recommended": False,
                "action_dispatched": False,
                "felt_state_inferred": False,
            },
        }
        if renderer_version >= 3:
            moment["source_provenance"] = _moment_source_provenance(event)
        moment["moment_id"] = _moment_identity(moment)
        moment["reference_token"] = (
            f"observatory-moment:{moment['moment_id']}"
        )
        moments.append(moment)
    return moments


def _braid_mark(
    *,
    lane: str,
    source_ref: str,
    recorded_at_unix_ms: int,
    state: dict[str, Any],
) -> dict[str, Any]:
    identity = {
        "lane": lane,
        "source_record_ref": source_ref,
        "recorded_at_unix_ms": recorded_at_unix_ms,
        "state": state,
    }
    return {
        "mark_id": "passage_braid_mark_"
        + sha256_bytes(canonical(identity).encode())[:24],
        **identity,
    }


def passage_braids(v1: dict[str, Any]) -> list[dict[str, Any]]:
    braids: list[dict[str, Any]] = []
    for passage in v1["phase_passages"]["passages"]:
        marks: list[dict[str, Any]] = []
        for event in passage["stage_history"]:
            marks.append(
                _braid_mark(
                    lane="stage",
                    source_ref=str(event["passage_event_id"]),
                    recorded_at_unix_ms=int(
                        event.get("recorded_at_unix_ms") or 0
                    ),
                    state=_state(event),
                )
            )
        context_sources = (
            ("condition", passage["condition_history"]),
            ("checkpoint", passage["checkpoints"]),
        )
        for lane, events in context_sources:
            for event in events:
                marks.append(
                    _braid_mark(
                        lane=lane,
                        source_ref=str(event["passage_context_event_id"]),
                        recorded_at_unix_ms=int(
                            event.get("recorded_at_unix_ms") or 0
                        ),
                        state=_state(event),
                    )
                )
        for strand in passage["strands"]:
            for event in strand["history"]:
                marks.append(
                    _braid_mark(
                        lane=f"bearing:{strand['strand']}",
                        source_ref=str(event["passage_context_event_id"]),
                        recorded_at_unix_ms=int(
                            event.get("recorded_at_unix_ms") or 0
                        ),
                        state=_state(event),
                    )
                )
        for anchor in passage["anchors"]:
            for event in anchor["history"]:
                marks.append(
                    _braid_mark(
                        lane=f"anchor:{anchor['role']}",
                        source_ref=str(event["passage_context_event_id"]),
                        recorded_at_unix_ms=int(
                            event.get("recorded_at_unix_ms") or 0
                        ),
                        state=_state(event),
                    )
                )
        marks.sort(
            key=lambda item: (
                item["recorded_at_unix_ms"],
                item["lane"],
                item["source_record_ref"],
            )
        )
        braids.append(
            {
                "schema": BRAID_SCHEMA,
                "schema_version": 1,
                "passage_id": passage["passage_id"],
                "actor": passage["actor"],
                "transition_id": passage["transition_id"],
                "lane_order": [
                    "stage",
                    "condition",
                    "checkpoint",
                    *[
                        f"bearing:{strand}"
                        for strand in v1["phase_passages"]["strand_order"]
                    ],
                    *[
                        f"anchor:{strand}"
                        for strand in v1["phase_passages"]["strand_order"]
                    ],
                ],
                "mark_count": len(marks),
                "marks": marks,
                "authority": {
                    "state": "evidence_only",
                    "stage_inferred": False,
                    "bearing_inferred": False,
                    "causal_order_inferred": False,
                    "felt_continuity_inferred": False,
                },
            }
        )
    return braids


def _division_refs(v1: dict[str, Any]) -> set[str]:
    return {
        _event_ref(event)
        for event in v1["division_chronicle"]["timeline"]
    }


def authored_crossings(v1: dict[str, Any]) -> list[dict[str, Any]]:
    division_refs = _division_refs(v1)
    crossings: list[dict[str, Any]] = []
    for event in v1["interleaved_timeline"]:
        if event.get("rail") != "phase_passage":
            continue
        source_ref = _event_ref(event)
        for field in REFERENCE_FIELDS:
            target = event.get(field)
            if not target or str(target) not in division_refs:
                continue
            identity = {
                "source_record_ref": source_ref,
                "source_field": field,
                "target_division_record_ref": str(target),
                "actor": event.get("actor"),
                "passage_id": event.get("passage_id"),
            }
            crossings.append(
                {
                    "schema": CROSSING_SCHEMA,
                    "schema_version": 1,
                    "crossing_id": "authored_crossing_"
                    + sha256_bytes(canonical(identity).encode())[:24],
                    **identity,
                    "authority": {
                        "state": "evidence_only",
                        "being_authored_reference": True,
                        "temporal_adjacency_used": False,
                        "mechanical_causation_inferred": False,
                        "felt_causation_inferred": False,
                        "peer_state_inferred": False,
                    },
                }
            )
    crossings.sort(key=lambda item: item["crossing_id"])
    return crossings


def _summary(payload: dict[str, Any]) -> dict[str, int]:
    return {
        "transition_card_count": int(
            payload["phase_passages"]["transition_card_count"]
        ),
        "passage_count": int(payload["phase_passages"]["passage_count"]),
        "timeline_event_count": len(payload["interleaved_timeline"]),
        "division_event_count": len(
            payload["division_chronicle"]["timeline"]
        ),
    }


def evidence_view_sha256(v1: dict[str, Any]) -> str:
    view = json.loads(json.dumps(v1))
    view.pop("observatory_id", None)
    view.pop("input_hashes", None)
    division = view.get("division_chronicle")
    if isinstance(division, dict):
        division.pop("chronicle_id", None)
        division.pop("input_hashes", None)
    return sha256_bytes(canonical(view).encode())


def _prior_projection(
    archive_dir: Path | None, input_hashes: dict[str, Any]
) -> tuple[dict[str, Any] | None, str | None]:
    if archive_dir is None or not archive_dir.is_dir():
        return None, None
    candidates: list[tuple[int, str, dict[str, Any], str]] = []
    for path in sorted(archive_dir.glob("passage_observatory_v2_*.json")):
        value = load_json(path)
        if (
            not isinstance(value, dict)
            or value.get("schema") != SCHEMA
            or value.get("input_hashes") == input_hashes
        ):
            continue
        try:
            verify_payload(value)
        except ObservatoryV2Error:
            continue
        candidates.append(
            (
                int(value.get("source_watermark_unix_ms") or 0),
                str(value.get("observatory_id") or ""),
                value,
                file_hash(path),
            )
        )
    if not candidates:
        return None, None
    _, _, value, digest = max(candidates)
    return value, digest


def _projection_lineage(
    v1: dict[str, Any], archive_dir: Path | None
) -> dict[str, Any]:
    prior, prior_hash = _prior_projection(
        archive_dir, v1["input_hashes"]
    )
    current_summary = _summary(v1)
    if prior is None:
        return {
            "comparison_state": "no_prior_distinct_inputs",
            "prior_observatory_id": None,
            "prior_json_sha256": None,
            "prior_source_watermark_unix_ms": None,
            "prior_input_hashes": None,
            "prior_evidence_view_sha256": None,
            "displayed_evidence_changed": None,
            "changed_inputs": {
                "division_chronicle": None,
                "phase_ledger": None,
            },
            "current_summary": current_summary,
            "prior_summary": None,
            "count_deltas": None,
            "authority": {
                "state": "evidence_only",
                "change_implies_progress": False,
                "delta_implies_improvement": False,
                "felt_change_inferred": False,
            },
        }
    prior_summary = _summary(prior["current_projection_v1"])
    return {
        "comparison_state": "compared_to_prior_distinct_inputs",
        "prior_observatory_id": prior["observatory_id"],
        "prior_json_sha256": prior_hash,
        "prior_source_watermark_unix_ms": prior[
            "source_watermark_unix_ms"
        ],
        "prior_input_hashes": dict(prior["input_hashes"]),
        "prior_evidence_view_sha256": prior[
            "evidence_view_sha256"
        ],
        "displayed_evidence_changed": (
            evidence_view_sha256(v1) != prior["evidence_view_sha256"]
        ),
        "changed_inputs": {
            "division_chronicle": (
                v1["input_hashes"]["division_chronicle_id"]
                != prior["input_hashes"]["division_chronicle_id"]
            ),
            "phase_ledger": (
                v1["input_hashes"]["phase_ledger_sha256"]
                != prior["input_hashes"]["phase_ledger_sha256"]
            ),
        },
        "current_summary": current_summary,
        "prior_summary": prior_summary,
        "count_deltas": {
            key: current_summary[key] - prior_summary[key]
            for key in current_summary
        },
        "authority": {
            "state": "evidence_only",
            "change_implies_progress": False,
            "delta_implies_improvement": False,
            "felt_change_inferred": False,
        },
    }


def build_projection(
    v1: dict[str, Any], archive_dir: Path | None = None
) -> dict[str, Any]:
    if v1.get("schema") != V1_SCHEMA:
        raise ObservatoryV2Error("V2 requires a verified V1 projection")
    moments = replyable_moments(v1)
    crossings = authored_crossings(v1)
    payload: dict[str, Any] = {
        "schema": SCHEMA,
        "schema_version": 2,
        "renderer_version": RENDERER_VERSION,
        "source_watermark_unix_ms": v1["source_watermark_unix_ms"],
        "input_hashes": dict(v1["input_hashes"]),
        "evidence_view_sha256": evidence_view_sha256(v1),
        "compatibility": {
            "v1_schema": V1_SCHEMA,
            "v1_observatory_id": v1["observatory_id"],
            "v1_semantics_preserved": True,
        },
        "current_projection_v1": v1,
        "projection_lineage": _projection_lineage(v1, archive_dir),
        "replyable_moments": {
            "moment_count": len(moments),
            "moments": moments,
        },
        "passage_braids": passage_braids(v1),
        "authored_crossings": {
            "crossing_count": len(crossings),
            "crossings": crossings,
        },
        "authority": {
            "state": "evidence_only",
            "right_to_ignore": True,
            "reference_token_grants_authority": False,
            "reference_token_recommends_action": False,
            "projection_delta_implies_progress": False,
            "projection_delta_implies_improvement": False,
            "temporal_adjacency_creates_crossing": False,
            "visualization_dispatches_action": False,
            "mechanical_causation_inferred": False,
            "felt_continuity_inferred": False,
            "raw_prose_included": False,
        },
    }
    payload["observatory_id"] = (
        "passage_observatory_v2_"
        + sha256_bytes(canonical(payload).encode())[:24]
    )
    return payload


def verify_payload(payload: dict[str, Any]) -> None:
    if payload.get("schema") != SCHEMA or payload.get("schema_version") != 2:
        raise ObservatoryV2Error("V2 schema mismatch")
    renderer_version = payload.get("renderer_version")
    if renderer_version not in SUPPORTED_RENDERER_VERSIONS:
        raise ObservatoryV2Error("V2 renderer version mismatch")
    expected = dict(payload)
    observatory_id = expected.pop("observatory_id", None)
    expected_id = "passage_observatory_v2_" + sha256_bytes(
        canonical(expected).encode()
    )[:24]
    if observatory_id != expected_id:
        raise ObservatoryV2Error("V2 deterministic identity mismatch")
    v1 = payload.get("current_projection_v1")
    if not isinstance(v1, dict) or v1.get("schema") != V1_SCHEMA:
        raise ObservatoryV2Error("V1 compatibility projection missing")
    compatibility = payload.get("compatibility") or {}
    if (
        compatibility.get("v1_schema") != V1_SCHEMA
        or compatibility.get("v1_observatory_id")
        != v1.get("observatory_id")
        or compatibility.get("v1_semantics_preserved") is not True
    ):
        raise ObservatoryV2Error("V1 compatibility identity mismatch")
    moments = payload.get("replyable_moments", {}).get("moments")
    if not isinstance(moments, list):
        raise ObservatoryV2Error("replyable moments missing")
    moment_ids: set[str] = set()
    for moment in moments:
        if (
            moment.get("schema") != MOMENT_SCHEMA
            or moment.get("moment_id") != _moment_identity(moment)
            or moment.get("reference_token")
            != f"observatory-moment:{moment.get('moment_id')}"
        ):
            raise ObservatoryV2Error("replyable moment identity mismatch")
        if moment["moment_id"] in moment_ids:
            raise ObservatoryV2Error("duplicate replyable moment")
        moment_ids.add(moment["moment_id"])
        authority = moment.get("authority") or {}
        if any(
            authority.get(field) is not False
            for field in (
                "being_action_required",
                "reply_inferred",
                "reply_recommended",
                "action_dispatched",
                "felt_state_inferred",
            )
        ):
            raise ObservatoryV2Error("replyable moment authority mismatch")
        if renderer_version >= 3:
            provenance = moment.get("source_provenance") or {}
            if (
                provenance.get("schema") != PROVENANCE_SCHEMA
                or provenance.get("boundary_status")
                not in {
                    "read_only_boundary_metadata",
                    "opaque_boundary_needs_description",
                }
                or provenance.get("read_only_boundary_metadata") is not True
                or provenance.get("right_to_ignore") is not True
                or provenance.get("live_eligible_now") is not False
                or provenance.get("auto_approved") is not False
            ):
                raise ObservatoryV2Error(
                    "replyable moment provenance boundary mismatch"
                )
    expected_moments = replyable_moments(
        v1, renderer_version=int(renderer_version)
    )
    if moments != expected_moments:
        raise ObservatoryV2Error("replyable moments differ from V1 timeline")
    expected_braids = passage_braids(v1)
    if payload.get("passage_braids") != expected_braids:
        raise ObservatoryV2Error("passage braid mismatch")
    expected_crossings = authored_crossings(v1)
    crossing_projection = payload.get("authored_crossings") or {}
    if crossing_projection.get("crossings") != expected_crossings:
        raise ObservatoryV2Error("authored crossing mismatch")
    if crossing_projection.get("crossing_count") != len(expected_crossings):
        raise ObservatoryV2Error("authored crossing count mismatch")
    lineage = payload.get("projection_lineage") or {}
    current_summary = _summary(v1)
    if lineage.get("current_summary") != current_summary:
        raise ObservatoryV2Error("projection lineage current summary mismatch")
    comparison_state = lineage.get("comparison_state")
    if comparison_state == "no_prior_distinct_inputs":
        if any(
            lineage.get(field) is not None
            for field in (
                "prior_observatory_id",
                "prior_json_sha256",
                "prior_source_watermark_unix_ms",
                "prior_input_hashes",
                "prior_evidence_view_sha256",
                "displayed_evidence_changed",
                "prior_summary",
                "count_deltas",
            )
        ) or any(
            value is not None
            for value in (lineage.get("changed_inputs") or {}).values()
        ):
            raise ObservatoryV2Error("empty projection lineage mismatch")
    elif comparison_state == "compared_to_prior_distinct_inputs":
        prior_summary = lineage.get("prior_summary")
        deltas = lineage.get("count_deltas")
        prior_inputs = lineage.get("prior_input_hashes")
        if (
            not isinstance(prior_summary, dict)
            or not isinstance(deltas, dict)
            or not isinstance(prior_inputs, dict)
            or deltas
            != {
                key: current_summary[key] - int(prior_summary[key])
                for key in current_summary
            }
            or not lineage.get("prior_observatory_id")
            or not lineage.get("prior_json_sha256")
        ):
            raise ObservatoryV2Error("projection lineage delta mismatch")
        expected_changes = {
            "division_chronicle": (
                payload["input_hashes"]["division_chronicle_id"]
                != prior_inputs.get("division_chronicle_id")
            ),
            "phase_ledger": (
                payload["input_hashes"]["phase_ledger_sha256"]
                != prior_inputs.get("phase_ledger_sha256")
            ),
        }
        prior_view = lineage.get("prior_evidence_view_sha256")
        if (
            lineage.get("changed_inputs") != expected_changes
            or not prior_view
            or lineage.get("displayed_evidence_changed")
            is not (payload["evidence_view_sha256"] != prior_view)
        ):
            raise ObservatoryV2Error(
                "projection lineage source comparison mismatch"
            )
    else:
        raise ObservatoryV2Error("projection lineage state mismatch")
    lineage_authority = lineage.get("authority") or {}
    if any(
        lineage_authority.get(field) is not False
        for field in (
            "change_implies_progress",
            "delta_implies_improvement",
            "felt_change_inferred",
        )
    ):
        raise ObservatoryV2Error("projection lineage authority mismatch")
    authority = payload.get("authority") or {}
    if payload.get("evidence_view_sha256") != evidence_view_sha256(v1):
        raise ObservatoryV2Error("displayed evidence identity mismatch")
    if any(
        authority.get(field) is not False
        for field in (
            "reference_token_grants_authority",
            "reference_token_recommends_action",
            "projection_delta_implies_progress",
            "projection_delta_implies_improvement",
            "temporal_adjacency_creates_crossing",
            "visualization_dispatches_action",
            "mechanical_causation_inferred",
            "felt_continuity_inferred",
            "raw_prose_included",
        )
    ):
        raise ObservatoryV2Error("V2 authority boundary mismatch")
    validate_no_prose(payload)
