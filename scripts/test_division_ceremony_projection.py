#!/usr/bin/env python3
"""Focused Agency Commons checks for ESN Division ceremony evidence."""

from __future__ import annotations

import json
import tempfile
from pathlib import Path

from agency_commons.division_ceremony import (
    _event_id,
    load_division_ceremony,
    validate_division_ceremony_event,
)
from agency_commons.projector import project
from experiential_systems.common import authority_state


def intent_event() -> dict[str, object]:
    return {
        "schema": "division.ceremony_event.v1",
        "schema_version": 1,
        "record_type": "division_ceremony_event",
        "record_id": "division_ceremony_12396f00a0031e2cb442b055",
        "ceremony_event_id": "division_ceremony_12396f00a0031e2cb442b055",
        "actor": "astrid",
        "action": "DIVISION_INTENT",
        "candidate": {
            "division_id": "divide-one",
            "parent_generation": 7,
            "plan_digest": "b" * 64,
            "selected_strategy": "input_recurrence",
        },
        "source_ref": "test:intent",
        "recorded_at_unix_ms": 1000,
        "expires_at_unix_ms": 9000,
        "previous_actor_event_id": None,
        "targets_event_id": None,
        "native_status_hash": None,
        "readiness_receipt_ref": None,
        "readiness_receipt_hash": None,
        "snapshot_refs": [],
        "current_tick": None,
        "rollback_deadline_tick": None,
        "review_outcome": None,
        "owner_language_action": "DIVISION_INTENT",
        "self_authored_only": True,
        "response_revisable": True,
        "right_to_ignore": True,
        "presence_inferred": False,
        "peer_consent_inferred": False,
        "silence_infers_consent": False,
        "native_assent_changed": False,
        "division_stage_changed": False,
        "prepare_dispatched": False,
        "commit_recommended": False,
        "commit_dispatched": False,
        "rollback_dispatched": False,
        "return_transition_dispatched": False,
        "scheduler_effect": False,
        "model_qos_effect": False,
        "substrate_effect": False,
        "dispatch_effect": False,
        "live_control_effect": False,
        "raw_prose_included": False,
        "artifact_authority_state_v1": authority_state(),
    }


def main() -> int:
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        ledger = root / "ceremony.jsonl"
        event = intent_event()
        validate_division_ceremony_event(event)
        ledger.write_text(json.dumps(event, sort_keys=True) + "\n")
        records, errors = load_division_ceremony(ledger)
        assert not errors and len(records) == 1

        held = dict(event)
        held["action"] = "DIVISION_HOLD"
        held["owner_language_action"] = "DIVISION_HOLD"
        held["recorded_at_unix_ms"] = 1001
        held["expires_at_unix_ms"] = None
        held["previous_actor_event_id"] = event["ceremony_event_id"]
        held["record_id"] = held["ceremony_event_id"] = _event_id(held)
        validate_division_ceremony_event(held)

        tampered = dict(event)
        tampered["commit_recommended"] = True
        try:
            validate_division_ceremony_event(tampered)
        except ValueError:
            pass
        else:
            raise AssertionError("authority escalation must be rejected")

        phase = root / "phase.jsonl"
        correspondence = root / "correspondence.jsonl"
        phase.write_text("")
        correspondence.write_text("")
        status = project(
            root / "workspace",
            phase,
            sovereignty_ledger=root / "sovereignty.json",
            agency_request_dir=root / "requests",
            correspondence_ledger=correspondence,
            division_ceremony_ledger=ledger,
            write=False,
        )
        assert status["valid"] is True
        assert status["explicit_division_ceremony_event_count"] == 1
        assert status["division_ceremony_changes_native_state"] is False
        assert status["division_ceremony_return_dispatches_rollback"] is False
        assert status["division_ceremony_recommends_commit"] is False
    print("division ceremony projection self-test: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
