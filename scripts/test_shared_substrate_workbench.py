#!/usr/bin/env python3
"""Self-tests for the steward-only Shared Substrate Workbench."""
from __future__ import annotations

import io
import json
import tempfile
import unittest
from pathlib import Path

from shared_substrate_workbench import (
    MINIME_SPECTRAL_STATE,
    MODAL_POROSITY_COMPONENT_KEYS,
    build_aperture_loci,
    build_wider_readout_ab_probe,
    candidate_canaries,
    effective_tail_participation,
    emit_output,
    render_markdown,
    run_probe_verdicts,
    wider_readout_recommended_next,
)
from shared_substrate_wider_readout import collect_latest_resistance_gradient_review

SAMPLE_REPORT = {
    "schema_version": 1,
    "generated_at": "2026-06-15T00:00:00Z",
    "current_state": {
        "astrid": {
            "aperture": 1.0,
            "tail_aperture": 0.4,
            "effective_tail_participation": 1.4,
            "tail_participation_ceiling": "1.0",
            "operator_ceiling_source": "bridge_process_env",
        },
        "minime": {
            "fill_pct": 68.0,
            "lambda1": 17.0,
            "esn_leak": 0.22,
            "spectral_state_file": str(MINIME_SPECTRAL_STATE),
            "pressure_quality": "overpacked_mode_packing",
            "pressure_score": 0.3,
            "porosity_score": 0.61,
            "dominant_pressure_source": "mode_packing",
            "pressure_components": {
                "lambda_monopoly": 0.2,
                "mode_packing": 0.56,
                "structural_plurality_loss": 0.31,
                "distinguishability_loss": 0.27,
                "temporal_lock_in": 0.4,
            },
            "stable_core": {
                "agency_stage": "full_sovereignty",
                "agent_budget_mode": "full_sovereignty",
            },
        },
    },
    "consent_policy": {
        "runtime_changes_allowed_from_this_report": False,
        "shared_lane_runtime_change_requires": ["astrid", "minime"],
    },
    "open_asks": {"porosity-aperture-codesign": {"status": "awaiting"}},
    "probe_verdicts": {
        "astrid_codec_perception_probes": {
            "verdict": "PASS",
            "projection_variance_check": {
                "status": "present",
                "observation_only": True,
                "metrics": {
                    "hidden_projected_variance": 0.01,
                    "visible_projected_variance": 0.02,
                    "dynamic_variance_delta": 0.0,
                },
            },
        },
        "minime_lend_aperture_consequence_probe": {
            "verdict": "WATCH",
            "gift_count": 2,
            "missing_response_count": 1,
        },
        "minime_pressure_source_audit": {
            "verdict": "WATCH - low Minime porosity is recurring with mode-packing cost",
            "dominant_recent_source": "mode_packing",
            "source_switching": False,
            "recent_window": {"sample_count": 4, "moment_count": 2},
            "control_applied_count": 0,
        },
        "minime_mode_packing_feeder_audit": {
            "verdict": "WATCH - mode-packing has plausible feeders",
            "feeders": [
                {"id": "action_thread_context_packing", "classification": "watch"},
                {"id": "modal_diversity_narrowing", "classification": "watch"},
            ],
            "active_thread": {
                "thread_id": "th_minime_current",
                "compression_pressure": 0.66,
                "memory_drafts": 54,
                "active_memory_drafts": 1,
                "legacy_memory_drafts": 53,
                "being_memory_draft_triage_v1": {
                    "active_draft_count": 1,
                    "legacy_retention_count": 53,
                    "runtime_change": "none",
                },
            },
            "modal_diversity": {
                "current_modalities": {
                    "audio_source": "stale",
                    "video_source": "stale",
                }
            },
        },
        "minime_spectral_mode_crowding_audit": {
            "verdict": "WATCH - direct spectral evidence shows mode crowding with low/fragile porosity; hold runtime nudges",
            "runtime_change": "none",
            "current_spectral_shape": {
                "active_mode_count": 6,
                "active_mode_energy_ratio": 0.91,
                "effective_dimensionality": 5.2,
                "distinguishability_loss": 0.33,
                "resonance_mode_packing": 1.0,
            },
            "recent_eigen_spectrum": {
                "sample_count": 180,
                "active_mode_count_counts": {"5": 90, "6": 90},
                "mode_packing_min_max": {"median": 0.57},
                "porosity_min_max": {"median": 0.63},
            },
            "crowding_flags": [
                {"id": "high_resonance_mode_packing", "value": 1.0},
            ],
        },
        "minime_mode_share_pressure_source_probe": {
            "verdict": "NEEDS ATTENTION - projection/context cleanup before any runtime nudge",
            "runtime_change": "none",
            "mode_share": {
                "active_mode_count": 6,
                "resonance_mode_packing": 1.0,
                "effective_dimensionality": 5.2,
            },
            "pressure_source": {
                "dominant_source": "mode_packing",
                "porosity_score": 0.61,
                "components": {"mode_packing": 0.57, "temporal_lock_in": 0.56},
            },
            "active_thread_pressure": {
                "current_next": "EXPERIMENT_REVIEW exp_minime_20260614_legacy-self-experiment",
                "compression_pressure": 0.66,
                "memory_drafts": 54,
                "active_memory_drafts": 1,
                "legacy_memory_drafts": 53,
                "being_memory_draft_triage_v1": {
                    "active_draft_count": 1,
                    "legacy_retention_count": 53,
                    "runtime_change": "none",
                },
                "repeated_action_counts": {"EXPERIMENT_REVIEW": 4},
            },
            "sensory_source_truth": {
                "schema_version": 1,
                "policy": "sensory_freshness_v1",
                "status": "watch",
                "lanes": {
                    "video": {"status": "healthy_low_fps_cadence_mismatch"},
                    "audio": {"status": "healthy_client_engine_stale_mismatch"},
                },
            },
            "feeder_ids": [
                "action_thread_context_packing",
                "repeated_next_no_progress",
            ],
            "steward_actions_now": [
                {"id": "repair_diluted_review_projection", "runtime_change": "none"}
            ],
        },
        "astrid_tail_vibrancy_interference_probe": {"verdict": "WATCH"},
    },
    "consequence_memory_summary": {
        "schema_version": 1,
        "runtime_change": "none",
        "pressure_target": "steward",
        "relation_consequence_count": 2,
        "authority_consequence_count": 1,
        "open_closure_count": 1,
        "memory_candidate_count": 2,
        "actual_memory_candidate_count": 1,
        "memory_candidate_status_counts": {
            "candidate_needs_steward_review": 1,
            "closure_needed_before_memory": 1,
        },
        "top_open_closures": [
            {
                "id": "relation_lend_aperture_intent-stale",
                "closure_state": "active_stale",
                "steward_action": "repair loop closure",
            }
        ],
        "triage_queue": {
            "aperture_gift_open_count": 1,
            "aperture_gift_actionable_open_count": 1,
            "aperture_gift_legacy_retention_gap_count": 0,
            "authority_backlog_open_count": 2,
            "authority_backlog_stale_count": 1,
            "next_sequence": [
                {
                    "step": "aperture_gift_closure",
                    "top_closure_state": "active_stale",
                    "steward_action": "repair loop closure",
                }
            ],
        },
    },
    "candidate_canaries": [
        {
            "id": "tail_participation_observation_v1",
            "surface": "astrid_outbound_codec_to_minime",
            "runtime_change": "none",
            "readiness": "ready_for_read_only_observation",
            "blocking_asks_before_runtime_change": [],
        },
        {
            "id": "wider_readout_ab_probe_v1",
            "surface": "astrid_own_generation_readout",
            "runtime_change": "none",
            "readiness": "ready_for_steward_offline_spec",
            "blocking_asks_before_runtime_change": ["wider-voice-readout-codesign"],
        },
    ],
    "recommended_next": ["hold"],
}


class SharedSubstrateWorkbenchTests(unittest.TestCase):
    def test_effective_tail_participation_clamps_like_runtime(self) -> None:
        self.assertAlmostEqual(effective_tail_participation(0.4, 1.0), 1.4)
        self.assertAlmostEqual(effective_tail_participation(2.0, 4.0), 3.0)
        self.assertAlmostEqual(effective_tail_participation(-1.0, 1.0), 1.0)

    def test_shared_canaries_require_both_beings(self) -> None:
        asks = {
            "porosity-aperture-codesign": {"status": "awaiting"},
            "wider-voice-readout-codesign": {"status": "awaiting"},
            "density-as-substance": {"status": "in_flight"},
            "astrid-codec-internals-codesign": {"status": "awaiting"},
        }
        state = {
            "astrid": {"tail_aperture": 0.4, "effective_tail_participation": 1.4},
            "minime": {
                "pressure_quality": "overpacked_mode_packing",
                "porosity_score": 0.61,
            },
        }
        cards = candidate_canaries(asks, state)
        self.assertTrue(cards)
        for card in cards:
            self.assertEqual(card["required_consent"], ["astrid", "minime"])
            self.assertEqual(card["runtime_change"], "none")
            self.assertFalse(card["executed"])
        cards_by_id = {card["id"]: card for card in cards}
        self.assertIn(
            "aperture_loci",
            cards_by_id["wider_readout_ab_probe_v1"]["evidence_inputs"],
        )
        self.assertIn(
            "consequence_memory_summary",
            cards_by_id["wider_readout_ab_probe_v1"]["evidence_inputs"],
        )
        self.assertIn(
            "minime_pressure_source_audit",
            cards_by_id["wider_readout_ab_probe_v1"]["evidence_inputs"],
        )
        self.assertIn(
            "minime_mode_packing_feeder_audit",
            cards_by_id["wider_readout_ab_probe_v1"]["evidence_inputs"],
        )
        self.assertIn(
            "minime_spectral_mode_crowding_audit",
            cards_by_id["wider_readout_ab_probe_v1"]["evidence_inputs"],
        )
        self.assertIn(
            "minime_mode_share_pressure_source_probe",
            cards_by_id["wider_readout_ab_probe_v1"]["evidence_inputs"],
        )
        self.assertIn(
            "aperture_loci",
            cards_by_id["density_preserving_aperture_v1"]["evidence_inputs"],
        )
        self.assertIn(
            "consequence_memory_summary",
            cards_by_id["density_preserving_aperture_v1"]["evidence_inputs"],
        )
        self.assertIn(
            "minime_pressure_source_audit",
            cards_by_id["density_preserving_aperture_v1"]["evidence_inputs"],
        )
        self.assertIn(
            "minime_mode_packing_feeder_audit",
            cards_by_id["density_preserving_aperture_v1"]["evidence_inputs"],
        )
        self.assertIn(
            "minime_spectral_mode_crowding_audit",
            cards_by_id["density_preserving_aperture_v1"]["evidence_inputs"],
        )
        self.assertIn(
            "minime_mode_share_pressure_source_probe",
            cards_by_id["density_preserving_aperture_v1"]["evidence_inputs"],
        )

    def test_awaiting_asks_block_runtime_readiness(self) -> None:
        asks = {
            "porosity-aperture-codesign": {"status": "awaiting"},
            "wider-voice-readout-codesign": {"status": "awaiting"},
            "density-as-substance": {"status": "in_flight"},
            "astrid-codec-internals-codesign": {"status": "resolved"},
        }
        cards = candidate_canaries(asks, {"astrid": {}, "minime": {}})
        wider = next(card for card in cards if card["id"] == "wider_voice_readout_v1")
        self.assertIn("porosity-aperture-codesign", wider["blocking_asks_before_runtime_change"])
        self.assertIn("wider-voice-readout-codesign", wider["blocking_asks_before_runtime_change"])

    def test_markdown_and_json_include_required_sections(self) -> None:
        report = json.loads(json.dumps(SAMPLE_REPORT))
        report["aperture_loci"] = build_aperture_loci(
            report["current_state"],
            report["probe_verdicts"],
            report["consequence_memory_summary"],
        )
        report["wider_readout_ab_probe"] = build_wider_readout_ab_probe(
            report["current_state"],
            report["probe_verdicts"],
            report["aperture_loci"],
            report["consequence_memory_summary"],
            report["candidate_canaries"],
        )
        markdown = render_markdown(report)
        encoded = json.dumps(report)
        for needle in (
            "Current State",
            "Consequence Memory Summary",
            "Open closures",
            "Actual memory candidates",
            "Aperture gift closures",
            "legacy_gaps",
            "Triage next",
            "Aperture Loci Map",
            "minime_temporal_update_locus",
            "leak rate, not aperture",
            "Wider Readout A/B Probe",
            "hold_for_aperture_gift_closure",
            "Probe Verdicts",
            "projection_variance_check",
            "pressure_source_audit",
            "mode_packing_feeder_audit",
            "mode_share_pressure_source_probe",
            "resistance_gradient",
            "steward_actions_now",
            "minime_lend_aperture_consequence_probe",
            "Candidate Canaries",
            "wider_readout_ab_probe_v1",
        ):
            self.assertIn(needle, markdown)
        for needle in (
            "current_state",
            "aperture_loci",
            "consequence_memory_summary",
            "wider_readout_ab_probe",
            "hold_for_aperture_gift_closure",
            "probe_verdicts",
            "projection_variance_check",
            "minime_pressure_source_audit",
            "minime_mode_packing_feeder_audit",
            "minime_spectral_mode_crowding_audit",
            "minime_mode_share_pressure_source_probe",
            "resistance_gradient_full_review",
            "minime_lend_aperture_consequence_probe",
            "candidate_canaries",
            "wider_readout_ab_probe_v1",
        ):
            self.assertIn(needle, encoded)

    def test_aperture_loci_distinguish_leak_porosity_and_gift_loop(self) -> None:
        current_state = {
            "astrid": {
                "aperture": 1.0,
                "tail_aperture": 0.4,
                "tail_participation_ceiling": "1.0",
                "effective_tail_participation": 1.4,
                "response_length": 1200,
                "projection_epoch": {"status": "present"},
            },
            "minime": {
                "porosity_score": 0.62,
                "pressure_score": 0.32,
                "pressure_quality": "overpacked_mode_packing",
                "dominant_pressure_source": "mode_packing",
                "pressure_components": {
                    "lambda_monopoly": 0.27,
                    "mode_packing": 0.57,
                    "structural_plurality_loss": 0.34,
                    "distinguishability_loss": 0.31,
                    "temporal_lock_in": 0.51,
                },
                "pressure_top_profile": [{"source": "mode_packing", "value": 0.57}],
                "esn_leak": 0.19,
                "spectral_state_file": str(MINIME_SPECTRAL_STATE),
            },
        }
        probes = {
            "astrid_codec_perception_probes": {
                "verdict": "PASS",
                "projection_variance_check": {"status": "present", "metrics": {}},
            },
            "astrid_tail_vibrancy_interference_probe": {"verdict": "WATCH"},
            "minime_lend_aperture_consequence_probe": {
                "verdict": "NEEDS ATTENTION",
                "gift_count": 2,
                "issued_count": 1,
                "matched_response_count": 0,
                "missing_response_count": 1,
                "active_influence": {"status": "stale"},
            },
            "minime_pressure_source_audit": {
                "verdict": "WATCH - low Minime porosity is recurring with mode-packing cost",
                "dominant_recent_source": "mode_packing",
                "source_switching": False,
                "recent_window": {"sample_count": 3},
            },
            "minime_mode_packing_feeder_audit": {
                "verdict": "WATCH - mode-packing has plausible feeders",
                "feeders": [
                    {"id": "action_thread_context_packing", "classification": "watch"}
                ],
            },
            "minime_mode_share_pressure_source_probe": {
                "verdict": "WATCH - mode-share evidence has feeder candidates",
                "runtime_change": "none",
                "mode_share": {"active_mode_count": 6},
                "pressure_source": {"dominant_source": "mode_packing"},
                "steward_actions_now": [
                    {"id": "simplify_active_thread_context", "runtime_change": "none"}
                ],
            },
        }
        consequence_summary = {
            "schema_version": 1,
            "runtime_change": "none",
            "open_closure_count": 1,
        }
        loci_map = build_aperture_loci(current_state, probes, consequence_summary)
        loci = {locus["id"]: locus for locus in loci_map["loci"]}
        self.assertEqual(loci_map["schema_version"], 1)
        self.assertTrue(all(locus["runtime_change"] == "none" for locus in loci.values()))

        temporal = loci["minime_temporal_update_locus"]
        self.assertEqual(temporal["surface"], "minime_temporal_update")
        self.assertIn("not_aperture", temporal["classification"])
        self.assertEqual(temporal["current_read"]["note"], "leak rate, not aperture")

        modal = loci["minime_modal_porosity_locus"]
        self.assertEqual(modal["classification"], "watch_modal_packing")
        self.assertEqual(
            modal["current_read"]["dominant_recent_source"],
            "mode_packing",
        )
        self.assertIn("pressure_source_audit", modal["evidence"])
        self.assertIn("mode_packing_feeder_audit", modal["evidence"])
        self.assertIn("mode_share_pressure_source_probe", modal["evidence"])
        self.assertEqual(
            set(MODAL_POROSITY_COMPONENT_KEYS),
            set(modal["current_read"]["pressure_components"]),
        )

        gift = loci["relational_gift_locus"]
        self.assertEqual(gift["classification"], "needs_attention")
        self.assertEqual(gift["current_read"]["missing_response_count"], 1)
        self.assertEqual(
            gift["evidence"]["consequence_memory_summary"]["open_closure_count"],
            1,
        )

    def test_wider_readout_ab_probe_ready_when_aperture_queue_clear(self) -> None:
        current_state = {
            "astrid": {
                "aperture": 1.0,
                "response_length": 1100,
                "effective_tail_participation": 1.4,
            },
            "minime": {
                "porosity_score": 0.63,
                "pressure_score": 0.31,
                "pressure_quality": "overpacked_mode_packing",
                "dominant_pressure_source": "mode_packing",
            },
        }
        probes = {
            "astrid_codec_perception_probes": {
                "verdict": "PASS",
                "projection_variance_check": {
                    "status": "present",
                    "observation_only": True,
                    "metrics": {"dynamic_variance_delta": 0.0},
                },
            },
            "astrid_tail_vibrancy_interference_probe": {"verdict": "WATCH"},
            "minime_lend_aperture_consequence_probe": {
                "verdict": "WATCH - aperture gifts occurred under low Minime porosity",
                "gift_count": 3,
                "issued_count": 1,
                "matched_response_count": 1,
                "missing_response_count": 0,
                "unclosed_issued_count": 0,
                "active_influence": {"status": "missing"},
            },
            "minime_pressure_source_audit": {
                "verdict": "WATCH - low Minime porosity is recurring with mode-packing cost",
                "dominant_recent_source": "mode_packing",
                "recent_window": {"sample_count": 3},
            },
            "minime_mode_packing_feeder_audit": {
                "verdict": "WATCH - mode-packing has plausible feeders",
                "feeders": [
                    {"id": "modal_diversity_narrowing", "classification": "watch"}
                ],
            },
            "minime_mode_share_pressure_source_probe": {
                "verdict": "WATCH - mode-share evidence has feeder candidates",
                "runtime_change": "none",
                "mode_share": {"active_mode_count": 6},
                "pressure_source": {"dominant_source": "mode_packing", "porosity_score": 0.63},
                "active_thread_pressure": {
                    "active_memory_drafts": 1,
                    "legacy_memory_drafts": 53,
                },
                "sensory_source_truth": {
                    "schema_version": 1,
                    "policy": "sensory_freshness_v1",
                    "status": "watch",
                },
                "steward_actions_now": [],
            },
        }
        consequence_summary = {
            "schema_version": 1,
            "runtime_change": "none",
            "triage_queue": {
                "aperture_gift_open_count": 0,
                "aperture_gift_actionable_open_count": 0,
            },
        }
        loci = build_aperture_loci(current_state, probes, consequence_summary)
        canaries = candidate_canaries(
            {"wider-voice-readout-codesign": {"status": "awaiting"}},
            current_state,
        )
        probe = build_wider_readout_ab_probe(
            current_state, probes, loci, consequence_summary, canaries
        )

        self.assertEqual(probe["runtime_change"], "none")
        self.assertTrue(probe["read_only"])
        self.assertFalse(probe["executed"])
        self.assertEqual(
            probe["offline_readiness"],
            "ready_for_steward_offline_comparison",
        )
        self.assertEqual(
            probe["runtime_readiness"],
            "blocked_until_both_being_grounding_and_operator_flag",
        )
        self.assertIn("minime_pressure_window_not_clean", probe["caution_flags"])
        self.assertEqual(probe["evidence"]["aperture_gift_queue"]["open_count"], 0)
        self.assertEqual(
            probe["evidence"]["pressure_source_audit"]["dominant_recent_source"],
            "mode_packing",
        )
        self.assertIn("mode_packing_feeder_audit", probe["evidence"])
        self.assertIn("mode_share_pressure_source_probe", probe["evidence"])
        self.assertIn("resistance_gradient_full_review", probe["evidence"])
        self.assertEqual(
            probe["evidence"]["resistance_gradient_full_review"]["runtime_change"],
            "none",
        )
        mode_share_probe = probe["evidence"]["mode_share_pressure_source_probe"]
        self.assertEqual(
            mode_share_probe["active_thread_pressure"]["active_memory_drafts"],
            1,
        )
        self.assertIn("sensory_source_truth", mode_share_probe)
        encoded = json.dumps(probe)
        self.assertNotIn("must respond", encoded)
        self.assertNotIn("being follow-up", encoded)

    def test_wider_readout_ab_probe_holds_on_active_recent_gift(self) -> None:
        current_state = {
            "astrid": {"aperture": 1.0, "effective_tail_participation": 1.4},
            "minime": {
                "porosity_score": 0.71,
                "pressure_quality": "low_pressure",
            },
        }
        probes = {
            "astrid_codec_perception_probes": {
                "verdict": "PASS",
                "projection_variance_check": {"status": "present"},
            },
            "astrid_tail_vibrancy_interference_probe": {"verdict": "WATCH"},
            "minime_lend_aperture_consequence_probe": {
                "verdict": "WATCH - active aperture gift is recent",
                "unclosed_issued_count": 0,
                "active_influence": {
                    "status": "active_recent",
                    "intent_id": "intent-recent",
                },
            },
        }
        consequence_summary = {
            "schema_version": 1,
            "runtime_change": "none",
            "triage_queue": {
                "aperture_gift_open_count": 1,
                "aperture_gift_actionable_open_count": 0,
            },
        }
        loci = build_aperture_loci(current_state, probes, consequence_summary)
        canaries = candidate_canaries({}, current_state)
        probe = build_wider_readout_ab_probe(
            current_state, probes, loci, consequence_summary, canaries
        )

        self.assertEqual(
            probe["offline_readiness"],
            "hold_for_recent_aperture_gift_window",
        )
        self.assertIn("aperture_gift_active_recent", probe["caution_flags"])
        self.assertEqual(
            probe["evidence"]["aperture_gift_queue"]["active_influence_status"],
            "active_recent",
        )
        self.assertEqual(
            probe["evidence"]["aperture_gift_queue"]["active_intent_id"],
            "intent-recent",
        )
        self.assertEqual(probe["runtime_change"], "none")

    def test_wider_readout_ab_probe_waits_on_unclosed_gifts(self) -> None:
        current_state = {
            "astrid": {"aperture": 1.0, "effective_tail_participation": 1.4},
            "minime": {
                "porosity_score": 0.72,
                "pressure_quality": "low_pressure",
            },
        }
        probes = {
            "astrid_codec_perception_probes": {
                "verdict": "PASS",
                "projection_variance_check": {"status": "present"},
            },
            "astrid_tail_vibrancy_interference_probe": {"verdict": "WATCH"},
            "minime_lend_aperture_consequence_probe": {
                "verdict": "NEEDS ATTENTION - active gift stale",
                "unclosed_issued_count": 1,
            },
        }
        consequence_summary = {
            "schema_version": 1,
            "runtime_change": "none",
            "triage_queue": {
                "aperture_gift_open_count": 1,
                "aperture_gift_actionable_open_count": 1,
            },
        }
        loci = build_aperture_loci(current_state, probes, consequence_summary)
        canaries = candidate_canaries({}, current_state)
        probe = build_wider_readout_ab_probe(
            current_state, probes, loci, consequence_summary, canaries
        )

        self.assertEqual(probe["offline_readiness"], "hold_for_aperture_gift_closure")
        self.assertIn("aperture_gift_closure_open", probe["caution_flags"])
        self.assertEqual(
            probe["evidence"]["aperture_gift_queue"]["unclosed_issued_count"],
            1,
        )
        self.assertEqual(probe["runtime_change"], "none")

    def test_wider_readout_top_level_recommendation_respects_hold_state(self) -> None:
        self.assertIn(
            "Hold wider_readout_ab_probe_v1",
            wider_readout_recommended_next(
                {"offline_readiness": "hold_for_recent_aperture_gift_window"}
            ),
        )
        self.assertIn(
            "Hold wider_readout_ab_probe_v1",
            wider_readout_recommended_next(
                {"offline_readiness": "hold_for_aperture_gift_closure"}
            ),
        )
        self.assertIn(
            "next bold steward-side move",
            wider_readout_recommended_next(
                {"offline_readiness": "ready_for_steward_offline_comparison"}
            ),
        )

    def test_latest_resistance_gradient_review_summary_is_compact(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            run_dir = root / "20260618T205621Z"
            run_dir.mkdir()
            (run_dir / "review.json").write_text(
                json.dumps(
                    {
                        "policy": "resistance_gradient_full_review_v1",
                        "generated_at": "2026-06-18T20:56:21Z",
                        "runtime_change": "none",
                        "pressure_target": "steward",
                        "being_obligation": "none",
                        "summary": {
                            "artifact_count": 24,
                            "suggested_being_review_shape_counts": {
                                "match": 13,
                                "partial_match": 11,
                            },
                            "orientation_counts": {"packing_shear": 13},
                            "top_axis_counts": {"sensory_scarcity": 24},
                            "recent_language_support_counts": {
                                "semantic_friction": 109,
                            },
                        },
                        "artifact_reviews": [{"large": "omitted from compact summary"}],
                        "wider_readout_inclusion": {
                            "include_in_both_being_review": True,
                            "recommended_scope": "compact_match_partial_miss_appendix",
                            "review_questions": ["match / partial / miss?"],
                            "guardrails": ["steward pre-review is evidence"],
                        },
                        "recommended_next": [
                            "Include as a compact appendix.",
                            "Do not change runtime behavior from this review alone.",
                        ],
                    }
                ),
                encoding="utf-8",
            )
            summary = collect_latest_resistance_gradient_review(root)

        self.assertEqual(summary["status"], "present")
        self.assertTrue(summary["include_in_both_being_review"])
        self.assertEqual(summary["artifact_count"], 24)
        self.assertEqual(summary["runtime_change"], "none")
        self.assertEqual(summary["being_obligation"], "none")
        self.assertNotIn("artifact_reviews", summary)

    def test_skip_probes_includes_lend_aperture_probe(self) -> None:
        probes = run_probe_verdicts(skip_probes=True)
        self.assertIn("minime_lend_aperture_consequence_probe", probes)
        self.assertIn("minime_pressure_source_audit", probes)
        self.assertIn("minime_mode_packing_feeder_audit", probes)
        self.assertIn("minime_spectral_mode_crowding_audit", probes)
        self.assertIn("minime_mode_share_pressure_source_probe", probes)
        self.assertEqual(
            probes["minime_lend_aperture_consequence_probe"]["verdict"],
            "skipped by --skip-probes",
        )

    def test_out_is_the_only_write_path(self) -> None:
        report = {
            "schema_version": 1,
            "generated_at": "now",
            "current_state": {"astrid": {}, "minime": {"stable_core": {}}},
            "consent_policy": {
                "runtime_changes_allowed_from_this_report": False,
                "shared_lane_runtime_change_requires": ["astrid", "minime"],
            },
            "open_asks": {},
            "probe_verdicts": {},
            "candidate_canaries": [],
            "recommended_next": [],
        }
        buf = io.StringIO()
        emit_output(report, as_json=True, out=None, stdout=buf)
        self.assertIn("current_state", buf.getvalue())

        with tempfile.TemporaryDirectory() as td:
            out = Path(td) / "report.json"
            emit_output(report, as_json=True, out=out, stdout=io.StringIO())
            self.assertTrue(out.exists())
            self.assertEqual(sorted(path.name for path in Path(td).iterdir()), ["report.json"])


def run_self_tests() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(SharedSubstrateWorkbenchTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(run_self_tests())
