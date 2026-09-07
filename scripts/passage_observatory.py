#!/usr/bin/env python3
"""Compose Phase Passage and ESN Division evidence into an immutable observatory."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import time
from collections import defaultdict
from pathlib import Path
from typing import Any

try:
    from agency_commons.projector import (
        _passage_context,
        _phase_passages,
        load_jsonl,
    )
    from division_ceremony_chronicle import (
        ChronicleError,
        atomic_owner_write,
        build_projection as build_division_projection,
        canonical,
        file_hash,
        load_json,
        sha256_bytes,
        validate_no_prose,
        verify_payload as verify_division_payload,
    )
    from passage_observatory_html import render_html
    from passage_observatory_v2 import (
        ObservatoryV2Error,
        build_projection as build_projection_v2,
        verify_payload as verify_payload_v2,
    )
    from passage_observatory_v2_html import render_html as render_html_v2
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.agency_commons.projector import (
        _passage_context,
        _phase_passages,
        load_jsonl,
    )
    from scripts.division_ceremony_chronicle import (
        ChronicleError,
        atomic_owner_write,
        build_projection as build_division_projection,
        canonical,
        file_hash,
        load_json,
        sha256_bytes,
        validate_no_prose,
        verify_payload as verify_division_payload,
    )
    from scripts.passage_observatory_html import render_html
    from scripts.passage_observatory_v2 import (
        ObservatoryV2Error,
        build_projection as build_projection_v2,
        verify_payload as verify_payload_v2,
    )
    from scripts.passage_observatory_v2_html import (
        render_html as render_html_v2,
    )
    from scripts.projection_receipt import projector_receipt


SCHEMA = "phase_division.passage_observatory.v1"
STRANDS = (
    "entry_tension",
    "pivot",
    "settling",
    "return",
    "reopen",
    "continuity",
)
ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DIVISION_WORKSPACE = ROOT.parent / "minime" / "workspace"
DEFAULT_PHASE_LEDGER = ROOT.parent / "shared" / "collaborations" / (
    "phase_transitions_v1.jsonl"
)
DEFAULT_OUTPUT = (
    DEFAULT_DIVISION_WORKSPACE / "division" / "passage-observatory"
)
MAX_TRANSITION_CARDS = 100


class ObservatoryError(ValueError):
    """The source evidence or composed observatory is invalid."""


def _timestamp(row: dict[str, Any]) -> int:
    return int(row.get("recorded_at_unix_ms") or 0)


def _stage_event(row: dict[str, Any]) -> dict[str, Any]:
    return {
        key: row.get(key)
        for key in (
            "passage_event_id",
            "action",
            "stage_before",
            "stage_after",
            "support_preference",
            "return_point_ref",
            "continuity_anchor_ref",
            "felt_review_outcome",
            "felt_source_ref",
            "previous_event_id",
            "recorded_at_unix_ms",
        )
    }


def _context_event(row: dict[str, Any]) -> dict[str, Any]:
    return {
        key: row.get(key)
        for key in (
            "passage_context_event_id",
            "actor",
            "action",
            "readiness",
            "movement_ease",
            "room_needed",
            "checkpoint",
            "anchor_role",
            "anchor_kind",
            "anchor_association",
            "anchor_ref",
            "previous_anchor_event_id",
            "bearing_strand",
            "movement_resistance",
            "persistence_tendency",
            "witness_fit",
            "previous_bearing_event_id",
            "company_request_id",
            "requested_peer",
            "company_mode",
            "company_response",
            "source_ref",
            "previous_context_event_id",
            "previous_company_event_id",
            "recorded_at_unix_ms",
        )
    }


def _bearing_state(
    strand: str, records: list[dict[str, Any]]
) -> dict[str, Any]:
    history = [
        _context_event(row)
        for row in records
        if row.get("action") == "describe_bearing"
        and row.get("bearing_strand") == strand
    ]
    current = history[-1] if history else None
    return {
        "strand": strand,
        "expression_state": "self_authored" if current else "unexpressed",
        "current": current,
        "history_count": len(history),
        "history": history,
        "scalar_value": None,
        "mechanical_correlation": None,
    }


def _anchor_state(
    role: str, records: list[dict[str, Any]]
) -> dict[str, Any]:
    history = [
        _context_event(row)
        for row in records
        if row.get("action") == "bind_anchor"
        and row.get("anchor_role") == role
    ]
    return {
        "role": role,
        "current": history[-1] if history else None,
        "history_count": len(history),
        "history": history,
    }


def _company_requests(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    requests: dict[str, dict[str, Any]] = {}
    for row in records:
        request_id = row.get("company_request_id")
        if not request_id:
            continue
        if row.get("action") == "request_company":
            requests[str(request_id)] = {
                "company_request_id": request_id,
                "requested_peer": row.get("requested_peer"),
                "company_mode": row.get("company_mode"),
                "latest_company_response": None,
                "latest_event_id": row.get("passage_context_event_id"),
                "recorded_at_unix_ms": row.get("recorded_at_unix_ms"),
            }
        elif str(request_id) in requests:
            requests[str(request_id)]["latest_company_response"] = row.get(
                "company_response"
            )
            requests[str(request_id)]["latest_event_id"] = row.get(
                "passage_context_event_id"
            )
    return list(requests.values())


def _transition_cards(ledger: Path) -> dict[str, Any]:
    rows, errors = load_jsonl(ledger)
    if errors:
        raise ObservatoryError("; ".join(errors))
    latest_witness: dict[str, dict[str, Any]] = {}
    for row in rows:
        if row.get("record_type") == "phase_transition_witness":
            transition_id = str(row.get("transition_id") or "")
            if transition_id:
                latest_witness[transition_id] = row
    cards: list[dict[str, Any]] = []
    for index, row in enumerate(rows, 1):
        if row.get("record_type") != "phase_transition_card":
            continue
        transition_id = str(row.get("transition_id") or "")
        required = (
            transition_id,
            str(row.get("origin") or ""),
            str(row.get("kind") or ""),
            str(row.get("from_phase") or ""),
            str(row.get("to_phase") or ""),
        )
        if (
            not all(required)
            or row.get("authority")
            != "language_only_transition_context_not_control"
            or any(
                row.get(field) is not True
                for field in (
                    "no_controller",
                    "no_pressure",
                    "no_fill_target",
                    "no_pi",
                    "no_weighting",
                )
            )
        ):
            raise ObservatoryError(
                f"phase_transition_card_{index}:authority boundary mismatch"
            )
        witness = latest_witness.get(transition_id)
        cards.append(
            {
                "transition_id": transition_id,
                "origin": row.get("origin"),
                "kind": row.get("kind"),
                "from_phase": row.get("from_phase"),
                "to_phase": row.get("to_phase"),
                "recorded_at_unix_ms": row.get("recorded_at_unix_ms"),
                "source_reply_state": row.get("reply_state") or "unseen",
                "latest_witness_state": (
                    witness.get("reply_state") if witness else None
                ),
                "passage_created": False,
                "passage_id": None,
            }
        )
    cards.sort(
        key=lambda row: (
            int(row.get("recorded_at_unix_ms") or 0),
            str(row.get("transition_id") or ""),
        )
    )
    omitted = max(0, len(cards) - MAX_TRANSITION_CARDS)
    return {
        "card_count": len(cards),
        "omitted_card_count": omitted,
        "recent_cards": cards[omitted:],
    }


def phase_passage_projection(ledger: Path) -> dict[str, Any]:
    passage_records, passage_errors = _phase_passages(ledger)
    context_records, context_errors = _passage_context(
        ledger, passage_records
    )
    errors = passage_errors + context_errors
    if errors:
        raise ObservatoryError("; ".join(errors))

    stage_groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
    context_groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in passage_records:
        stage_groups[str(row["passage_id"])].append(row)
    for row in context_records:
        context_groups[str(row["passage_id"])].append(row)

    card_projection = _transition_cards(ledger)
    passages: list[dict[str, Any]] = []
    for passage_id, stage_rows in stage_groups.items():
        stage_rows.sort(key=_timestamp)
        contexts = sorted(context_groups.get(passage_id, []), key=_timestamp)
        latest = stage_rows[-1]
        conditions = [
            _context_event(row)
            for row in contexts
            if row.get("action") == "describe_condition"
        ]
        checkpoints = [
            _context_event(row)
            for row in contexts
            if row.get("action") == "mark_checkpoint"
        ]
        passage = {
            "passage_id": passage_id,
            "transition_id": latest.get("transition_id"),
            "actor": latest.get("actor"),
            "latest_stage": latest.get("stage_after"),
            "latest_stage_event_id": latest.get("passage_event_id"),
            "latest_support_preference": latest.get("support_preference"),
            "latest_felt_review_outcome": next(
                (
                    row.get("felt_review_outcome")
                    for row in reversed(stage_rows)
                    if row.get("felt_review_outcome") is not None
                ),
                None,
            ),
            "stage_event_count": len(stage_rows),
            "stage_history": [_stage_event(row) for row in stage_rows],
            "current_condition": conditions[-1] if conditions else None,
            "condition_history": conditions,
            "checkpoints": checkpoints,
            "strands": [
                _bearing_state(strand, contexts) for strand in STRANDS
            ],
            "anchors": [
                _anchor_state(role, contexts) for role in STRANDS
            ],
            "company_requests": _company_requests(contexts),
            "authority": {
                "self_authored_only": True,
                "stage_inferred": False,
                "felt_resolution_inferred": False,
                "peer_consent_inferred": False,
                "silence_infers_progress": False,
                "bearing_inferred_from_telemetry": False,
                "bearing_is_metric": False,
                "mechanical_causation_inferred": False,
                "live_control_effect": False,
            },
        }
        passages.append(passage)
    passage_by_transition = {
        str(passage["transition_id"]): passage for passage in passages
    }
    for card in card_projection["recent_cards"]:
        passage = passage_by_transition.get(str(card["transition_id"]))
        if passage:
            card["passage_created"] = True
            card["passage_id"] = passage["passage_id"]
    passages.sort(
        key=lambda row: (
            -_timestamp(row["stage_history"][-1]),
            str(row["passage_id"]),
        )
    )
    return {
        "schema": "phase.passage_observatory_projection.v1",
        "ledger_sha256": file_hash(ledger),
        "transition_card_count": card_projection["card_count"],
        "omitted_transition_card_count": card_projection[
            "omitted_card_count"
        ],
        "recent_transition_cards": card_projection["recent_cards"],
        "passage_count": len(passages),
        "passages": passages,
        "strand_order": list(STRANDS),
        "authority": {
            "state": "evidence_only",
            "felt_score_present": False,
            "friction_scalar_present": False,
            "telemetry_infers_bearing": False,
            "silence_infers_progress": False,
            "observational_cards_auto_promoted": False,
            "visualization_changes_passage": False,
            "visualization_closes_transition": False,
            "raw_prose_included": False,
        },
    }


def _phase_timeline(
    phase: dict[str, Any],
) -> list[dict[str, Any]]:
    timeline: list[dict[str, Any]] = [
        {
            "rail": "phase_observation",
            "event_kind": "transition_card",
            **card,
        }
        for card in phase["recent_transition_cards"]
    ]
    for passage in phase["passages"]:
        shared = {
            "actor": passage["actor"],
            "passage_id": passage["passage_id"],
            "transition_id": passage["transition_id"],
        }
        for row in passage["stage_history"]:
            timeline.append(
                {
                    "rail": "phase_passage",
                    "event_kind": "passage_stage",
                    **shared,
                    **row,
                }
            )
        for strand in passage["strands"]:
            for row in strand["history"]:
                timeline.append(
                    {
                        "rail": "phase_passage",
                        "event_kind": "passage_bearing",
                        **shared,
                        **row,
                    }
                )
        for row in passage["condition_history"]:
            timeline.append(
                {
                    "rail": "phase_passage",
                    "event_kind": "passage_condition",
                    **shared,
                    **row,
                }
            )
        for row in passage["checkpoints"]:
            timeline.append(
                {
                    "rail": "phase_passage",
                    "event_kind": "passage_checkpoint",
                    **shared,
                    **row,
                }
            )
        for anchor in passage["anchors"]:
            for row in anchor["history"]:
                timeline.append(
                    {
                        "rail": "phase_passage",
                        "event_kind": "passage_anchor",
                        **shared,
                        **row,
                    }
                )
    return timeline


def _division_timeline(
    division: dict[str, Any],
) -> list[dict[str, Any]]:
    return [
        {
            "rail": "division_runtime",
            "event_kind": row.get("event_kind")
            or row.get("action")
            or "division_event",
            **row,
        }
        for row in division["timeline"]
    ]


def build_projection(
    division_workspace: Path, phase_ledger: Path
) -> dict[str, Any]:
    division = build_division_projection(division_workspace)
    verify_division_payload(division)
    phase = phase_passage_projection(phase_ledger)
    timeline = _division_timeline(division) + _phase_timeline(phase)
    timeline.sort(
        key=lambda row: (
            int(row.get("recorded_at_unix_ms") or 0),
            str(row.get("rail") or ""),
            str(
                row.get("passage_event_id")
                or row.get("passage_context_event_id")
                or row.get("ceremony_event_id")
                or row.get("sequence")
                or ""
            ),
        )
    )
    watermark = max(
        [int(division.get("source_watermark_unix_ms") or 0)]
        + [int(row.get("recorded_at_unix_ms") or 0) for row in timeline]
    )
    payload: dict[str, Any] = {
        "schema": SCHEMA,
        "schema_version": 1,
        "source_watermark_unix_ms": watermark,
        "input_hashes": {
            "division_chronicle_id": division["chronicle_id"],
            "phase_ledger_sha256": phase["ledger_sha256"],
        },
        "division_chronicle": division,
        "phase_passages": phase,
        "interleaved_timeline": timeline,
        "timeline_relation": "temporal_co_presence_only",
        "authority": {
            "state": "evidence_only",
            "right_to_ignore": True,
            "silence_infers_consent": False,
            "silence_infers_progress": False,
            "visualization_grants_authority": False,
            "visualization_dispatches_action": False,
            "visualization_recommends_action": False,
            "mechanical_causation_inferred": False,
            "felt_continuity_inferred": False,
            "felt_equivalence_inferred": False,
            "cross_rail_state_synthesized": False,
            "raw_prose_included": False,
        },
    }
    payload["observatory_id"] = (
        "passage_observatory_"
        + sha256_bytes(canonical(payload).encode())[:24]
    )
    return payload


def verify_payload(payload: dict[str, Any]) -> None:
    if payload.get("schema") != SCHEMA or payload.get("schema_version") != 1:
        raise ObservatoryError("observatory schema mismatch")
    expected = dict(payload)
    observatory_id = expected.pop("observatory_id", None)
    expected_id = "passage_observatory_" + sha256_bytes(
        canonical(expected).encode()
    )[:24]
    if observatory_id != expected_id:
        raise ObservatoryError("observatory deterministic identity mismatch")
    division = payload.get("division_chronicle")
    if not isinstance(division, dict):
        raise ObservatoryError("division chronicle is missing")
    try:
        verify_division_payload(division)
    except ChronicleError as error:
        raise ObservatoryError(str(error)) from error
    phase = payload.get("phase_passages")
    if (
        not isinstance(phase, dict)
        or phase.get("strand_order") != list(STRANDS)
    ):
        raise ObservatoryError("phase passage strand contract mismatch")
    for passage in phase.get("passages") or []:
        strands = passage.get("strands")
        if not isinstance(strands, list) or [
            item.get("strand") for item in strands
        ] != list(STRANDS):
            raise ObservatoryError("passage strand history is incomplete")
        authority = passage.get("authority")
        if not isinstance(authority, dict) or any(
            authority.get(field) is not False
            for field in (
                "stage_inferred",
                "felt_resolution_inferred",
                "peer_consent_inferred",
                "silence_infers_progress",
                "bearing_inferred_from_telemetry",
                "bearing_is_metric",
                "mechanical_causation_inferred",
                "live_control_effect",
            )
        ):
            raise ObservatoryError("passage authority boundary mismatch")
        for strand in strands:
            if (
                strand.get("scalar_value") is not None
                or strand.get("mechanical_correlation") is not None
            ):
                raise ObservatoryError("passage bearing became a proxy metric")
    authority = payload.get("authority")
    if not isinstance(authority, dict) or any(
        authority.get(field) is not False
        for field in (
            "silence_infers_consent",
            "silence_infers_progress",
            "visualization_grants_authority",
            "visualization_dispatches_action",
            "visualization_recommends_action",
            "mechanical_causation_inferred",
            "felt_continuity_inferred",
            "felt_equivalence_inferred",
            "cross_rail_state_synthesized",
            "raw_prose_included",
        )
    ):
        raise ObservatoryError("observatory authority boundary mismatch")
    if payload.get("timeline_relation") != "temporal_co_presence_only":
        raise ObservatoryError("cross-rail relation must remain non-causal")
    validate_no_prose(payload)


def project(
    division_workspace: Path, phase_ledger: Path, output: Path
) -> tuple[dict[str, Any], Path, Path]:
    v1 = build_projection(division_workspace, phase_ledger)
    verify_payload(v1)
    archive = output / "archive"
    payload = build_projection_v2(v1, archive)
    verify_payload_v2(payload)
    v1_json_bytes = (
        json.dumps(v1, indent=2, sort_keys=True) + "\n"
    ).encode()
    v2_json_bytes = (
        json.dumps(payload, indent=2, sort_keys=True) + "\n"
    ).encode()
    current_paths = {
        "v1_json": output / "observatory_v1.json",
        "v1_html": output / "observatory_v1.html",
        "v2_json": output / "observatory_v2.json",
        "v2_html": output / "observatory_v2.html",
    }
    archive_paths = {
        "v1_json": archive / f"{v1['observatory_id']}.json",
        "v1_html": archive / f"{v1['observatory_id']}.html",
        "v2_json": archive / f"{payload['observatory_id']}.json",
        "v2_html": archive / f"{payload['observatory_id']}.html",
    }
    content = {
        "v1_json": v1_json_bytes,
        "v1_html": render_html(v1, live=True).encode(),
        "v2_json": v2_json_bytes,
        "v2_html": render_html_v2(payload, live=True).encode(),
    }
    archive_content = {
        "v1_json": v1_json_bytes,
        "v1_html": render_html(v1, live=False).encode(),
        "v2_json": v2_json_bytes,
        "v2_html": render_html_v2(payload, live=False).encode(),
    }
    for label, path in archive_paths.items():
        if not path.exists():
            atomic_owner_write(path, archive_content[label])
    for label, path in current_paths.items():
        atomic_owner_write(path, content[label])
    return payload, current_paths["v2_json"], current_paths["v2_html"]


def verify_files(output: Path) -> dict[str, Any]:
    paths = {
        "v1_json": output / "observatory_v1.json",
        "v1_html": output / "observatory_v1.html",
        "v2_json": output / "observatory_v2.json",
        "v2_html": output / "observatory_v2.html",
    }
    v1 = load_json(paths["v1_json"])
    payload = load_json(paths["v2_json"])
    if (
        v1 is None
        or payload is None
        or not paths["v1_html"].is_file()
        or not paths["v2_html"].is_file()
    ):
        raise ObservatoryError("observatory latest outputs are missing")
    verify_payload(v1)
    verify_payload_v2(payload)
    if payload["current_projection_v1"] != v1:
        raise ObservatoryError("V1 and V2 current projections differ")
    for path in paths.values():
        if path.stat().st_mode & 0o077:
            raise ObservatoryError(f"{path} is not owner-only")
    archive = output / "archive"
    archive_pairs = (
        (
            paths["v1_json"],
            archive / f"{v1['observatory_id']}.json",
        ),
        (
            paths["v2_json"],
            archive / f"{payload['observatory_id']}.json",
        ),
    )
    for latest, archived in archive_pairs:
        if not archived.is_file():
            raise ObservatoryError("observatory archive is missing")
        if archived.read_bytes() != latest.read_bytes():
            raise ObservatoryError(
                "latest JSON differs from immutable archive"
            )
    for archived_html in (
        archive / f"{v1['observatory_id']}.html",
        archive / f"{payload['observatory_id']}.html",
    ):
        if not archived_html.is_file():
            raise ObservatoryError("observatory HTML archive is missing")
        if archived_html.stat().st_mode & 0o077:
            raise ObservatoryError(
                f"{archived_html} is not owner-only"
            )
    return {
        "ok": True,
        "observatory_id": payload["observatory_id"],
        "v1_observatory_id": v1["observatory_id"],
        "passage_count": v1["phase_passages"]["passage_count"],
        "timeline_event_count": len(v1["interleaved_timeline"]),
        "replyable_moment_count": payload["replyable_moments"][
            "moment_count"
        ],
        "authored_crossing_count": payload["authored_crossings"][
            "crossing_count"
        ],
        "json_sha256": file_hash(paths["v2_json"]),
        "html_sha256": file_hash(paths["v2_html"]),
    }


def report(payload: dict[str, Any]) -> str:
    v1 = payload["current_projection_v1"]
    division = v1["division_chronicle"]
    runtime = division["runtime_topology"]
    return "\n".join(
        (
            "Phase Passage Runtime + Division Passage Observatory",
            f"Observatory: {payload['observatory_id']}",
            f"Division Chronicle: {division['chronicle_id']}",
            (
                "Division authority rail: "
                f"{runtime['active_authority_rail']}"
            ),
            (
                "Independent daughter ownership: "
                f"{runtime['independent_process_ownership_established']}"
            ),
            (
                "Self-authored passages: "
                f"{v1['phase_passages']['passage_count']}"
            ),
            (
                "Replyable moments: "
                f"{payload['replyable_moments']['moment_count']}"
            ),
            (
                "Being-authored crossings: "
                f"{payload['authored_crossings']['crossing_count']}"
            ),
            "Relation: temporal co-presence only; no causal or felt inference.",
        )
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command", choices=("project", "verify", "show", "report", "watch")
    )
    parser.add_argument(
        "--division-workspace",
        type=Path,
        default=DEFAULT_DIVISION_WORKSPACE,
    )
    parser.add_argument(
        "--phase-ledger", type=Path, default=DEFAULT_PHASE_LEDGER
    )
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--interval", type=float, default=2.0)
    parser.add_argument("--receipt-json", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    started = time.monotonic()
    try:
        if args.command == "project":
            payload, json_path, html_path = project(
                args.division_workspace, args.phase_ledger, args.output
            )
            status = {
                "valid": True,
                "summary": {
                    "observatory_id": payload["observatory_id"],
                    "v1_observatory_id": payload["compatibility"][
                        "v1_observatory_id"
                    ],
                    "passage_count": payload["current_projection_v1"][
                        "phase_passages"
                    ][
                        "passage_count"
                    ],
                    "replyable_moment_count": payload[
                        "replyable_moments"
                    ]["moment_count"],
                    "authored_crossing_count": payload[
                        "authored_crossings"
                    ]["crossing_count"],
                },
            }
            if args.receipt_json:
                value = projector_receipt(
                    "passage_observatory",
                    status,
                    {
                        "observatory_v1.json": (
                            args.output / "observatory_v1.json"
                        ),
                        "observatory_v1.html": (
                            args.output / "observatory_v1.html"
                        ),
                        "observatory_v2.json": json_path,
                        "observatory_v2.html": html_path,
                    },
                    started_monotonic=started,
                )
            else:
                value = {
                    **status,
                    "json": str(json_path),
                    "html": str(html_path),
                }
            print(
                json.dumps(
                    value,
                    indent=2,
                )
            )
        elif args.command == "verify":
            print(json.dumps(verify_files(args.output), indent=2))
        elif args.command in {"show", "report"}:
            payload = load_json(args.output / "observatory_v2.json")
            if payload is None:
                raise ObservatoryError("observatory has not been projected")
            if args.command == "show":
                print(json.dumps(payload, indent=2, sort_keys=True))
            else:
                print(report(payload))
        else:
            previous = None
            while True:
                payload, _, _ = project(
                    args.division_workspace, args.phase_ledger, args.output
                )
                if payload["observatory_id"] != previous:
                    print(report(payload), flush=True)
                    previous = payload["observatory_id"]
                time.sleep(max(0.25, args.interval))
    except KeyboardInterrupt:
        return 0
    except (
        ChronicleError,
        ObservatoryError,
        ObservatoryV2Error,
        OSError,
        json.JSONDecodeError,
    ) as error:
        print(f"passage observatory error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
