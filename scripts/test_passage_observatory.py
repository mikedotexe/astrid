#!/usr/bin/env python3

from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from agency_commons.model import authority_state
from agency_commons.passage_context import (
    LivedTransitionPassageContextEventV1,
    PassageBearingStrandV1,
    PassageContextActionV1,
    PassageMovementResistanceV1,
    PassagePersistenceTendencyV1,
    PassageWitnessFitV1,
)
from passage_observatory import (
    ObservatoryError,
    STRANDS,
    build_projection,
    phase_passage_projection,
    project,
    report,
    verify_files,
    verify_payload,
)
from passage_observatory_v2 import (
    SCHEMA as V2_SCHEMA,
    authored_crossings,
    build_projection as build_projection_v2,
    evidence_view_sha256,
    replyable_moments,
    verify_payload as verify_payload_v2,
)


def passage_row(
    *,
    transition_id: str,
    actor: str,
    action: str,
    stage_before: str | None,
    stage_after: str,
    timestamp: int,
    passage_id: str | None = None,
    previous_event_id: str | None = None,
) -> dict[str, object]:
    if passage_id is None:
        digest = hashlib.sha256(
            f"{actor}:{transition_id}:{timestamp}".encode()
        ).hexdigest()[:16]
        passage_id = f"passage_{timestamp}_{digest}"
    owner_action = {
        "prepare": "PREPARE_TRANSITION",
        "enter": "ENTER_TRANSITION",
    }[action]
    continuity_anchor = f"transition:{transition_id}"
    identity = ":".join(
        (
            passage_id,
            actor,
            action,
            stage_after,
            "self_directed",
            "",
            continuity_anchor,
            "",
            "",
            previous_event_id or "",
            str(timestamp),
        )
    )
    event_id = "passage_event_" + hashlib.sha256(
        identity.encode()
    ).hexdigest()[:16]
    return {
        "schema": "lived_transition_passage_event_v1",
        "schema_version": 1,
        "record_type": "phase_transition_passage",
        "record_id": event_id,
        "passage_event_id": event_id,
        "passage_id": passage_id,
        "transition_id": transition_id,
        "actor": actor,
        "action": action,
        "stage_before": stage_before,
        "stage_after": stage_after,
        "stage_changed": True,
        "support_preference": "self_directed",
        "return_point_ref": None,
        "continuity_anchor_ref": continuity_anchor,
        "felt_review_outcome": None,
        "felt_source_ref": None,
        "previous_event_id": previous_event_id,
        "recorded_at_unix_ms": timestamp,
        "owner_language_action": owner_action,
        "self_authored_only": True,
        "passage_binds_actor_only": True,
        "peer_consent_inferred": False,
        "peer_state_changed": False,
        "silence_infers_progress": False,
        "automatic_progression": False,
        "review_optional": True,
        "felt_resolution_inferred": False,
        "scheduler_effect": False,
        "model_qos_effect": False,
        "substrate_effect": False,
        "dispatch_effect": False,
        "live_control_effect": False,
        "runtime_unlock_applied": False,
        "raw_prose_included": False,
        "artifact_authority_state_v1": authority_state(),
    }


def transition_card(transition_id: str, timestamp: int) -> dict[str, object]:
    return {
        "record_type": "phase_transition_card",
        "transition_id": transition_id,
        "recorded_at_unix_ms": timestamp,
        "origin": "astrid",
        "kind": "mode_change",
        "from_phase": "drift",
        "to_phase": "focus",
        "reply_state": "unseen",
        "authority": "language_only_transition_context_not_control",
        "no_controller": True,
        "no_pressure": True,
        "no_fill_target": True,
        "no_pi": True,
        "no_weighting": True,
    }


def write_jsonl(path: Path, rows: list[dict[str, object]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows)
    )


class PassageObservatoryTests(unittest.TestCase):
    def test_empty_projection_keeps_rails_distinct(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            ledger = root / "phase.jsonl"
            ledger.touch()
            payload = build_projection(root / "workspace", ledger)

            self.assertEqual(
                payload["timeline_relation"], "temporal_co_presence_only"
            )
            self.assertEqual(payload["phase_passages"]["passage_count"], 0)
            self.assertEqual(payload["interleaved_timeline"], [])
            self.assertFalse(
                payload["authority"]["cross_rail_state_synthesized"]
            )
            verify_payload(payload)

    def test_observational_card_does_not_create_passage(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            ledger = Path(raw) / "phase.jsonl"
            write_jsonl(
                ledger, [transition_card("transition-one", 99)]
            )
            phase = phase_passage_projection(ledger)
            self.assertEqual(phase["transition_card_count"], 1)
            self.assertEqual(phase["passage_count"], 0)
            self.assertFalse(
                phase["recent_transition_cards"][0]["passage_created"]
            )
            self.assertFalse(
                phase["authority"]["observational_cards_auto_promoted"]
            )

    def test_six_strands_preserve_independent_history(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            ledger = Path(raw) / "phase.jsonl"
            prepared = passage_row(
                transition_id="transition-one",
                actor="astrid",
                action="prepare",
                stage_before=None,
                stage_after="prepared",
                timestamp=100,
            )
            entered = passage_row(
                transition_id="transition-one",
                actor="astrid",
                action="enter",
                stage_before="prepared",
                stage_after="crossing",
                timestamp=101,
                passage_id=str(prepared["passage_id"]),
                previous_event_id=str(prepared["passage_event_id"]),
            )
            bearing = LivedTransitionPassageContextEventV1.build(
                passage_id=str(prepared["passage_id"]),
                transition_id="transition-one",
                passage_actor="astrid",
                actor="astrid",
                action=PassageContextActionV1.DESCRIBE_BEARING,
                bearing_strand=PassageBearingStrandV1.ENTRY_TENSION,
                movement_resistance=PassageMovementResistanceV1.EFFORTFUL,
                persistence_tendency=PassagePersistenceTendencyV1.LINGERING,
                witness_fit=PassageWitnessFitV1.TOUCHING,
                source_ref="felt_source_one",
                recorded_at_unix_ms=102,
            ).to_dict()
            write_jsonl(
                ledger,
                [
                    transition_card("transition-one", 99),
                    prepared,
                    entered,
                    bearing,
                ],
            )

            phase = phase_passage_projection(ledger)
            passage = phase["passages"][0]
            self.assertEqual(passage["latest_stage"], "crossing")
            self.assertEqual(
                [item["strand"] for item in passage["strands"]],
                list(STRANDS),
            )
            entry = passage["strands"][0]
            self.assertEqual(entry["expression_state"], "self_authored")
            self.assertEqual(
                entry["current"]["movement_resistance"], "effortful"
            )
            self.assertIsNone(entry["scalar_value"])
            self.assertEqual(
                passage["strands"][1]["expression_state"], "unexpressed"
            )
            self.assertFalse(
                passage["authority"]["bearing_inferred_from_telemetry"]
            )

    def test_invalid_passage_lineage_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            ledger = Path(raw) / "phase.jsonl"
            prepared = passage_row(
                transition_id="transition-one",
                actor="astrid",
                action="prepare",
                stage_before=None,
                stage_after="prepared",
                timestamp=100,
            )
            entered = passage_row(
                transition_id="transition-one",
                actor="astrid",
                action="enter",
                stage_before="prepared",
                stage_after="crossing",
                timestamp=101,
                passage_id=str(prepared["passage_id"]),
                previous_event_id="wrong-event",
            )
            write_jsonl(
                ledger,
                [
                    transition_card("transition-one", 99),
                    prepared,
                    entered,
                ],
            )
            with self.assertRaisesRegex(
                ObservatoryError, "self-owned sequence"
            ):
                phase_passage_projection(ledger)

    def test_projection_is_deterministic_owner_only_and_archived(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            workspace = root / "workspace"
            ledger = root / "phase.jsonl"
            ledger.touch()
            output = root / "output"

            first, _, _ = project(workspace, ledger, output)
            second, _, _ = project(workspace, ledger, output)

            self.assertEqual(
                first["observatory_id"], second["observatory_id"]
            )
            receipt = verify_files(output)
            self.assertTrue(receipt["ok"])
            self.assertEqual(
                (output / "observatory_v1.json").stat().st_mode & 0o077, 0
            )
            self.assertEqual(
                (output / "observatory_v2.json").stat().st_mode & 0o077, 0
            )
            html = (output / "observatory_v1.html").read_text()
            self.assertIn(
                'name="observatory-mode" content="live"', html
            )
            self.assertIn("Temporal co-presence only", html)
            self.assertIn("Movement resistance", html)
            v2_html = (output / "observatory_v2.html").read_text()
            self.assertIn("Replyable Moment Atlas", v2_html)
            self.assertIn("Passage Braid", v2_html)
            self.assertEqual(first["schema"], V2_SCHEMA)
            self.assertIn(
                "Relation: temporal co-presence only",
                report(first),
            )

    def test_tampering_with_authority_or_proxy_metric_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            ledger = root / "phase.jsonl"
            ledger.touch()
            payload = build_projection(root / "workspace", ledger)
            payload["authority"]["felt_continuity_inferred"] = True
            with self.assertRaisesRegex(
                ObservatoryError, "identity mismatch"
            ):
                verify_payload(payload)

            prepared = passage_row(
                transition_id="transition-one",
                actor="astrid",
                action="prepare",
                stage_before=None,
                stage_after="prepared",
                timestamp=100,
            )
            write_jsonl(
                ledger,
                [transition_card("transition-one", 99), prepared],
            )
            payload = build_projection(root / "workspace", ledger)
            payload["phase_passages"]["passages"][0]["strands"][0][
                "scalar_value"
            ] = 0.8
            expected = dict(payload)
            expected.pop("observatory_id")
            payload["observatory_id"] = (
                "passage_observatory_"
                + hashlib.sha256(
                    json.dumps(
                        expected,
                        sort_keys=True,
                        separators=(",", ":"),
                        ensure_ascii=True,
                    ).encode()
                ).hexdigest()[:24]
            )
            with self.assertRaisesRegex(
                ObservatoryError, "proxy metric"
            ):
                verify_payload(payload)

    def test_v2_moments_are_stable_references_without_action_pressure(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            ledger = root / "phase.jsonl"
            write_jsonl(
                ledger, [transition_card("transition-one", 99)]
            )
            v1 = build_projection(root / "workspace", ledger)
            moments = replyable_moments(v1)
            v2 = build_projection_v2(v1)

            self.assertEqual(
                v2["replyable_moments"]["moments"], moments
            )
            self.assertEqual(len(moments), 1)
            self.assertTrue(
                moments[0]["reference_token"].startswith(
                    "observatory-moment:passage_moment_"
                )
            )
            self.assertFalse(
                moments[0]["authority"]["being_action_required"]
            )
            self.assertFalse(
                moments[0]["authority"]["reply_recommended"]
            )
            provenance = moments[0]["source_provenance"]
            self.assertEqual(
                provenance["source_category"],
                "derived_bridge_summary",
            )
            self.assertEqual(
                provenance["role"],
                "observational_transition_card",
            )
            self.assertEqual(
                provenance["boundary_status"],
                "read_only_boundary_metadata",
            )
            self.assertTrue(provenance["right_to_ignore"])
            self.assertFalse(provenance["live_eligible_now"])
            verify_payload_v2(v2)

    def test_v2_readback_provenance_distinguishes_owner_and_runtime_sources(
        self,
    ) -> None:
        base = {
            "interleaved_timeline": [
                {
                    "rail": "phase_passage",
                    "event_kind": "passage_stage",
                    "passage_event_id": "passage-event-one",
                    "actor": "astrid",
                    "recorded_at_unix_ms": 100,
                },
                {
                    "rail": "division_runtime",
                    "source": "ceremony",
                    "event_kind": "DIVISION_HOLD",
                    "ceremony_event_id": "ceremony-event-one",
                    "actor": "minime",
                    "recorded_at_unix_ms": 101,
                },
                {
                    "rail": "division_runtime",
                    "source": "sovereign_runtime",
                    "event_kind": "authority_switched",
                    "event_id": "runtime-event-one",
                    "recorded_at_unix_ms": 102,
                },
            ]
        }
        moments = replyable_moments(base)
        self.assertEqual(
            [
                item["source_provenance"]["source_category"]
                for item in moments
            ],
            [
                "internalized_memory",
                "internalized_memory",
                "peer_telemetry_inference",
            ],
        )
        self.assertEqual(
            moments[0]["source_provenance"]["role"],
            "being_authored_phase_passage",
        )
        self.assertEqual(
            moments[1]["source_provenance"]["role"],
            "being_authored_division_ceremony_action",
        )
        self.assertEqual(
            moments[2]["source_provenance"]["role"],
            "division_runtime_evidence",
        )
        self.assertTrue(
            all(
                item["source_provenance"]["auto_approved"] is False
                for item in moments
            )
        )

    def test_v2_renderer_two_archives_remain_verifiable(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            ledger = root / "phase.jsonl"
            write_jsonl(
                ledger, [transition_card("transition-one", 99)]
            )
            v1 = build_projection(root / "workspace", ledger)
            payload = build_projection_v2(v1)
            payload["renderer_version"] = 2
            payload["replyable_moments"]["moments"] = replyable_moments(
                v1, renderer_version=2
            )
            expected = dict(payload)
            expected.pop("observatory_id")
            payload["observatory_id"] = (
                "passage_observatory_v2_"
                + hashlib.sha256(
                    json.dumps(
                        expected,
                        sort_keys=True,
                        separators=(",", ":"),
                        ensure_ascii=True,
                    ).encode()
                ).hexdigest()[:24]
            )
            verify_payload_v2(payload)

    def test_v2_lineage_compares_only_distinct_source_identities(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            ledger = root / "phase.jsonl"
            archive = root / "archive"
            archive.mkdir()
            ledger.touch()
            first_v1 = build_projection(root / "workspace", ledger)
            first = build_projection_v2(first_v1, archive)
            first_path = archive / f"{first['observatory_id']}.json"
            first_path.write_text(
                json.dumps(first, indent=2, sort_keys=True) + "\n"
            )

            same = build_projection_v2(first_v1, archive)
            self.assertEqual(
                same["observatory_id"], first["observatory_id"]
            )
            self.assertEqual(
                same["projection_lineage"]["comparison_state"],
                "no_prior_distinct_inputs",
            )

            write_jsonl(
                ledger, [transition_card("transition-one", 99)]
            )
            second_v1 = build_projection(root / "workspace", ledger)
            second = build_projection_v2(second_v1, archive)
            lineage = second["projection_lineage"]
            self.assertEqual(
                lineage["prior_observatory_id"], first["observatory_id"]
            )
            self.assertTrue(lineage["changed_inputs"]["phase_ledger"])
            self.assertFalse(
                lineage["changed_inputs"]["division_chronicle"]
            )
            self.assertEqual(
                lineage["count_deltas"]["transition_card_count"], 1
            )
            self.assertTrue(lineage["displayed_evidence_changed"])
            self.assertFalse(
                lineage["authority"]["delta_implies_improvement"]
            )
            verify_payload_v2(second)

    def test_v2_display_identity_ignores_source_hash_only_churn(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            ledger = root / "phase.jsonl"
            ledger.touch()
            v1 = build_projection(root / "workspace", ledger)
            changed = json.loads(json.dumps(v1))
            changed["input_hashes"][
                "division_chronicle_id"
            ] = "division_chronicle_changed"
            changed["division_chronicle"][
                "chronicle_id"
            ] = "division_chronicle_changed"
            changed["division_chronicle"]["input_hashes"] = {
                "supervisor_status_sha256": "changed"
            }
            self.assertEqual(
                evidence_view_sha256(v1),
                evidence_view_sha256(changed),
            )

    def test_v2_crossing_requires_exact_being_authored_reference(
        self,
    ) -> None:
        base = {
            "division_chronicle": {
                "timeline": [
                    {
                        "event_id": "division-event-one",
                        "recorded_at_unix_ms": 100,
                    }
                ]
            },
            "interleaved_timeline": [
                {
                    "rail": "phase_passage",
                    "event_kind": "passage_anchor",
                    "passage_context_event_id": "passage-context-one",
                    "passage_id": "passage-one",
                    "actor": "astrid",
                    "recorded_at_unix_ms": 101,
                }
            ],
        }
        self.assertEqual(authored_crossings(base), [])
        base["interleaved_timeline"][0][
            "anchor_ref"
        ] = "division-event-one"
        crossings = authored_crossings(base)
        self.assertEqual(len(crossings), 1)
        self.assertEqual(
            crossings[0]["target_division_record_ref"],
            "division-event-one",
        )
        self.assertTrue(
            crossings[0]["authority"]["being_authored_reference"]
        )
        self.assertFalse(
            crossings[0]["authority"]["temporal_adjacency_used"]
        )

    def test_v2_rejects_rehashed_editorialized_lineage_delta(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            ledger = root / "phase.jsonl"
            ledger.touch()
            v1 = build_projection(root / "workspace", ledger)
            payload = build_projection_v2(v1)
            payload["projection_lineage"]["current_summary"][
                "passage_count"
            ] = 9
            expected = dict(payload)
            expected.pop("observatory_id")
            payload["observatory_id"] = (
                "passage_observatory_v2_"
                + hashlib.sha256(
                    json.dumps(
                        expected,
                        sort_keys=True,
                        separators=(",", ":"),
                        ensure_ascii=True,
                    ).encode()
                ).hexdigest()[:24]
            )
            with self.assertRaisesRegex(
                ValueError, "current summary mismatch"
            ):
                verify_payload_v2(payload)


if __name__ == "__main__":
    unittest.main()
