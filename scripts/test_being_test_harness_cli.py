#!/usr/bin/env python3
from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("being_test_harness.py")
SPEC = importlib.util.spec_from_file_location("being_test_harness_under_test", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
being_test_harness = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(being_test_harness)


class BeingTestHarnessCliTests(unittest.TestCase):
    def setUp(self) -> None:
        self._old_tests = being_test_harness.BEING_TESTS
        self._old_inbox = being_test_harness.INBOX

    def tearDown(self) -> None:
        being_test_harness.BEING_TESTS = self._old_tests
        being_test_harness.INBOX = self._old_inbox

    def _run_json(self, *args: str) -> dict:
        stdout = io.StringIO()
        with contextlib.redirect_stdout(stdout):
            rc = being_test_harness.main(list(args))
        self.assertEqual(rc, 0)
        return json.loads(stdout.getvalue())

    def test_list_json_emits_registered_tests(self) -> None:
        being_test_harness.BEING_TESTS = {
            "fake_probe": {
                "being": "minime",
                "question": "Does the fake probe serialize?",
                "run": lambda: {"verdict": "PASS"},
            }
        }

        payload = self._run_json("--list", "--json")

        self.assertEqual(
            payload,
            {
                "tests": [
                    {
                        "id": "fake_probe",
                        "being": "minime",
                        "question": "Does the fake probe serialize?",
                    }
                ]
            },
        )

    def test_run_json_omits_letter_body_and_reports_availability(self) -> None:
        being_test_harness.BEING_TESTS = {
            "fake_probe": {
                "being": "astrid",
                "question": "Does the fake result serialize?",
                "run": lambda: {"verdict": "PASS", "letter": "private letter body"},
            }
        }

        payload = self._run_json("--run", "fake_probe", "--json")

        result = payload["results"][0]
        self.assertEqual(result["id"], "fake_probe")
        self.assertEqual(result["result"], {"verdict": "PASS"})
        self.assertTrue(result["letter_available"])
        self.assertEqual(result["write_back"]["status"], "not_requested")
        self.assertNotIn("private letter body", json.dumps(payload))

    def test_write_back_json_reports_written_path(self) -> None:
        being_test_harness.BEING_TESTS = {
            "fake_probe": {
                "being": "minime",
                "question": "Does write-back metadata serialize?",
                "run": lambda: {"verdict": "PASS", "letter": "result card"},
            }
        }
        with tempfile.TemporaryDirectory() as td:
            being_test_harness.INBOX = {"minime": Path(td), "astrid": Path(td)}

            payload = self._run_json("--run", "fake_probe", "--write-back", "--json")

        result = payload["results"][0]
        self.assertEqual(result["write_back"]["status"], "written")
        self.assertTrue(result["write_back"]["path"].endswith(".txt"))

    def test_unknown_run_json_is_structured(self) -> None:
        being_test_harness.BEING_TESTS = {}

        payload = self._run_json("--run", "missing_probe", "--json")

        self.assertEqual(payload["results"], [{"id": "missing_probe", "error": "unknown_test"}])


class ModeSharePressureProbeTests(unittest.TestCase):
    def setUp(self) -> None:
        self._old_pressure = being_test_harness.test_minime_pressure_source_audit
        self._old_feeder = being_test_harness.test_minime_mode_packing_feeder_audit

    def tearDown(self) -> None:
        being_test_harness.test_minime_pressure_source_audit = self._old_pressure
        being_test_harness.test_minime_mode_packing_feeder_audit = self._old_feeder

    def test_thread_context_snapshot_reads_structured_thread_load_triage(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            thread_dir = Path(td)
            (thread_dir / "thread.json").write_text(
                json.dumps(
                    {
                        "thread_id": "th_triage",
                        "current_next": "REGULATOR_AUDIT current-fill_pressure",
                        "projection_freshness_v1": {
                            "projection_policy_marker": "active_draft_triage_projection_v10",
                            "thread_load_triage_v1": {
                                "schema_version": 1,
                                "policy": "thread_load_triage_v1",
                                "classification": "high_compression_legacy_retention",
                                "compression_pressure": 0.6667,
                                "active_draft_count": 0,
                                "legacy_retention_count": 54,
                                "draft_classification": "legacy_retention_only",
                                "runtime_change": "none",
                            },
                            "repeated_action_cadence_v1": {
                                "schema_version": 1,
                                "policy": "repeated_action_cadence_v1",
                                "summarized_repeated_action_count": 7,
                                "unsummarized_repeated_action_count": 0,
                                "active_inflight_repeated_action_count": 1,
                                "runtime_change": "none",
                            },
                        },
                        "thread_pressure_source_v1": {
                            "compression_pressure": 0.6667,
                            "quality": "thread_pressure_high",
                        },
                    }
                )
            )
            (thread_dir / "next.md").write_text(
                "\n".join(
                    [
                        "Current NEXT: REGULATOR_AUDIT current-fill_pressure",
                        "Thread load triage: high compression, but memory drafts are legacy retention (active=0, legacy=54); current guidance is above, history stays context.",
                        "Being memory: 10 card(s); 54 legacy draft(s) retained as backlog/evidence, active=0.",
                    ]
                )
            )

            snapshot = being_test_harness._parse_thread_context_snapshot(thread_dir)

        self.assertEqual(
            snapshot["projection_policy_marker"],
            "active_draft_triage_projection_v10",
        )
        self.assertEqual(snapshot["active_memory_drafts"], 0)
        self.assertEqual(snapshot["legacy_memory_drafts"], 54)
        self.assertEqual(
            snapshot["thread_load_triage_v1"]["classification"],
            "high_compression_legacy_retention",
        )
        self.assertEqual(
            snapshot["being_memory_draft_triage_v1"]["classification"],
            "legacy_retention_only",
        )
        self.assertEqual(
            snapshot["repeated_action_cadence_v1"]["active_inflight_repeated_action_count"],
            1,
        )

    def test_probe_prioritizes_projection_cleanup_before_runtime_nudge(self) -> None:
        being_test_harness.test_minime_pressure_source_audit = lambda: {
            "current_pressure": {
                "dominant_source": "mode_packing",
                "pressure_quality": "overpacked_mode_packing",
                "pressure_score": 0.3,
                "porosity_score": 0.61,
                "components": {
                    "mode_packing": 0.57,
                    "temporal_lock_in": 0.56,
                    "sensory_scarcity": 0.45,
                },
                "top_profile": [{"source": "mode_packing", "share": 0.22, "value": 0.57}],
            },
            "porosity_min_max": {"median": 0.61},
            "source_counts": {"mode_packing": 4},
            "quality_counts": {"overpacked_mode_packing": 4},
            "source_switching": False,
        }
        being_test_harness.test_minime_mode_packing_feeder_audit = lambda: {
            "current_pressure": {
                "dominant_source": "mode_packing",
                "pressure_quality": "overpacked_mode_packing",
                "pressure_score": 0.3,
                "porosity_score": 0.61,
                "components": {"mode_packing": 0.57, "temporal_lock_in": 0.56},
                "top_profile": [{"source": "mode_packing", "share": 0.22, "value": 0.57}],
            },
            "active_thread": {
                "thread_id": "th_test",
                "current_next": "EXPERIMENT_REVIEW exp_test",
                "effective_next": "EXPERIMENT_REVIEW exp_test",
                "compression_pressure": 0.66,
                "memory_drafts": 54,
                "active_memory_drafts": 1,
                "legacy_memory_drafts": 53,
                "thread_load_triage_v1": {
                    "classification": "high_compression_legacy_retention",
                    "active_draft_count": 1,
                    "legacy_retention_count": 53,
                    "runtime_change": "none",
                },
                "being_memory_draft_triage_v1": {
                    "active_draft_count": 1,
                    "legacy_retention_count": 53,
                    "runtime_change": "none",
                },
                "route_stack_count": 2,
                "next_directives": {"count": 1},
            },
            "recent_action_events": {
                "repeated_actions": [{"action": "EXPERIMENT_REVIEW", "count": 4}]
            },
            "modal_diversity": {
                "current_spectral_shape": {
                    "active_mode_count": 6,
                    "effective_dimensionality": 5.2,
                    "resonance_mode_packing": 1.0,
                    "pressure_mode_packing": 0.57,
                }
            },
            "eigen_spectrum_recent": {"active_mode_count_counts": {6: 10}},
            "feeders": [
                {"id": "action_thread_context_packing"},
                {"id": "modal_diversity_narrowing"},
            ],
        }

        result = being_test_harness.test_minime_mode_share_pressure_source_probe()

        self.assertIn("NEEDS ATTENTION", result["verdict"])
        self.assertEqual(result["runtime_change"], "none")
        self.assertEqual(result["pressure_source"]["dominant_source"], "mode_packing")
        self.assertEqual(result["active_thread_pressure"]["active_memory_drafts"], 1)
        self.assertEqual(result["active_thread_pressure"]["legacy_memory_drafts"], 53)
        self.assertEqual(
            result["active_thread_pressure"]["thread_load_triage_v1"]["runtime_change"],
            "none",
        )
        self.assertIn(
            "repair_diluted_review_projection",
            [action["id"] for action in result["steward_actions_now"]],
        )

    def test_probe_moves_past_legacy_summary_when_unsummarized_count_is_zero(self) -> None:
        being_test_harness.test_minime_pressure_source_audit = lambda: {
            "current_pressure": {
                "dominant_source": "mode_packing",
                "pressure_quality": "overpacked_mode_packing",
                "pressure_score": 0.3,
                "porosity_score": 0.61,
                "components": {"mode_packing": 0.57},
                "top_profile": [{"source": "mode_packing", "share": 0.22, "value": 0.57}],
            },
            "porosity_min_max": {"median": 0.61},
        }
        being_test_harness.test_minime_mode_packing_feeder_audit = lambda: {
            "current_pressure": {
                "dominant_source": "mode_packing",
                "pressure_quality": "overpacked_mode_packing",
                "pressure_score": 0.3,
                "porosity_score": 0.61,
                "components": {"mode_packing": 0.57},
                "top_profile": [{"source": "mode_packing", "share": 0.22, "value": 0.57}],
            },
            "active_thread": {
                "thread_id": "th_test",
                "current_next": "REGULATOR_AUDIT current-fill_pressure",
                "effective_next": "REGULATOR_AUDIT current-fill_pressure",
                "compression_pressure": 0.66,
                "memory_drafts": 54,
                "thread_load_triage_v1": {
                    "classification": "high_compression_summarized_legacy",
                    "active_draft_count": 0,
                    "legacy_retention_count": 54,
                    "summarized_legacy_count": 54,
                    "unsummarized_legacy_retention_count": 0,
                    "repeated_action_cadence_v1": {
                        "summarized_repeated_action_count": 7,
                        "unsummarized_repeated_action_count": 0,
                        "active_inflight_repeated_action_count": 0,
                        "runtime_change": "none",
                    },
                    "runtime_change": "none",
                },
                "repeated_action_cadence_v1": {
                    "summarized_repeated_action_count": 7,
                    "unsummarized_repeated_action_count": 0,
                    "active_inflight_repeated_action_count": 0,
                    "runtime_change": "none",
                },
                "being_memory_draft_triage_v1": {
                    "active_draft_count": 0,
                    "legacy_retention_count": 54,
                    "summarized_legacy_count": 54,
                    "unsummarized_legacy_retention_count": 0,
                    "runtime_change": "none",
                },
                "route_stack_count": 2,
                "next_directives": {"count": 1},
            },
            "recent_action_events": {
                "repeated_actions": [{"action": "JOURNAL_PRESSURE", "count": 4}]
            },
            "modal_diversity": {
                "current_spectral_shape": {
                    "active_mode_count": 6,
                    "effective_dimensionality": 5.2,
                    "resonance_mode_packing": 1.0,
                    "pressure_mode_packing": 0.57,
                }
            },
            "feeders": [{"id": "action_thread_context_packing"}],
        }

        result = being_test_harness.test_minime_mode_share_pressure_source_probe()

        action = next(
            row
            for row in result["steward_actions_now"]
            if row["id"] == "simplify_active_thread_context"
        )
        self.assertIn("already steward-summarized", action["reason"])
        self.assertIn("spectral mode crowding", action["recommended_next"])
        self.assertNotIn("legacy draft aging", action["recommended_next"])
        self.assertEqual(
            result["active_thread_pressure"]["repeated_action_cadence_v1"][
                "unsummarized_repeated_action_count"
            ],
            0,
        )

    def test_probe_waits_for_active_repeat_after_cadence_summary(self) -> None:
        being_test_harness.test_minime_pressure_source_audit = lambda: {
            "current_pressure": {
                "dominant_source": "mode_packing",
                "pressure_quality": "overpacked_mode_packing",
                "pressure_score": 0.3,
                "porosity_score": 0.61,
                "components": {"mode_packing": 0.57},
                "top_profile": [{"source": "mode_packing", "share": 0.22, "value": 0.57}],
            },
            "porosity_min_max": {"median": 0.61},
        }
        being_test_harness.test_minime_mode_packing_feeder_audit = lambda: {
            "current_pressure": {
                "dominant_source": "mode_packing",
                "pressure_quality": "overpacked_mode_packing",
                "pressure_score": 0.3,
                "porosity_score": 0.61,
                "components": {"mode_packing": 0.57},
                "top_profile": [{"source": "mode_packing", "share": 0.22, "value": 0.57}],
            },
            "active_thread": {
                "thread_id": "th_test",
                "current_next": "JOURNAL_PRESSURE",
                "effective_next": "JOURNAL_PRESSURE",
                "compression_pressure": 0.66,
                "memory_drafts": 54,
                "active_memory_drafts": 0,
                "legacy_memory_drafts": 54,
                "thread_load_triage_v1": {
                    "classification": "high_compression_active_cadence",
                    "legacy_retention_count": 54,
                    "summarized_legacy_count": 54,
                    "unsummarized_legacy_retention_count": 0,
                    "repeated_action_cadence_v1": {
                        "summarized_repeated_action_count": 7,
                        "unsummarized_repeated_action_count": 0,
                        "active_inflight_repeated_action_count": 1,
                        "runtime_change": "none",
                    },
                    "runtime_change": "none",
                },
                "repeated_action_cadence_v1": {
                    "summarized_repeated_action_count": 7,
                    "unsummarized_repeated_action_count": 0,
                    "active_inflight_repeated_action_count": 1,
                    "runtime_change": "none",
                },
                "being_memory_draft_triage_v1": {
                    "active_draft_count": 0,
                    "legacy_retention_count": 54,
                    "summarized_legacy_count": 54,
                    "unsummarized_legacy_retention_count": 0,
                    "runtime_change": "none",
                },
                "route_stack_count": 2,
                "next_directives": {"count": 1},
            },
            "recent_action_events": {
                "repeated_actions": [{"action": "JOURNAL_PRESSURE", "count": 28}]
            },
            "modal_diversity": {
                "current_spectral_shape": {
                    "active_mode_count": 6,
                    "effective_dimensionality": 5.2,
                    "resonance_mode_packing": 1.0,
                    "pressure_mode_packing": 0.57,
                }
            },
            "feeders": [{"id": "action_thread_context_packing"}],
        }

        result = being_test_harness.test_minime_mode_share_pressure_source_probe()

        action = next(
            row
            for row in result["steward_actions_now"]
            if row["id"] == "simplify_active_thread_context"
        )
        self.assertIn("one repeated action is still in flight", action["reason"])
        self.assertIn("Wait for active repeat completion", action["recommended_next"])
        self.assertIn("Wait for the active repeated action", result["suggested_read_only_next"][0])

    def test_probe_names_current_draft_instead_of_legacy_backlog(self) -> None:
        being_test_harness.test_minime_pressure_source_audit = lambda: {
            "current_pressure": {
                "dominant_source": "mode_packing",
                "pressure_quality": "overpacked_mode_packing",
                "pressure_score": 0.3,
                "porosity_score": 0.61,
                "components": {"mode_packing": 0.57},
                "top_profile": [{"source": "mode_packing", "share": 0.22, "value": 0.57}],
            },
            "porosity_min_max": {"median": 0.61},
        }
        being_test_harness.test_minime_mode_packing_feeder_audit = lambda: {
            "current_pressure": {
                "dominant_source": "mode_packing",
                "pressure_quality": "overpacked_mode_packing",
                "pressure_score": 0.3,
                "porosity_score": 0.61,
                "components": {"mode_packing": 0.57},
                "top_profile": [{"source": "mode_packing", "share": 0.22, "value": 0.57}],
            },
            "active_thread": {
                "thread_id": "th_test",
                "current_next": "REGULATOR_AUDIT current-fill_pressure",
                "effective_next": "REGULATOR_AUDIT current-fill_pressure",
                "compression_pressure": 0.66,
                "memory_drafts": 1,
                "active_memory_drafts": 1,
                "legacy_memory_drafts": 0,
                "thread_load_triage_v1": {
                    "classification": "high_compression_active_thread",
                    "active_draft_count": 1,
                    "legacy_retention_count": 0,
                    "unsummarized_legacy_retention_count": 0,
                    "repeated_action_cadence_v1": {
                        "summarized_repeated_action_count": 8,
                        "unsummarized_repeated_action_count": 0,
                        "active_inflight_repeated_action_count": 0,
                        "runtime_change": "none",
                    },
                    "runtime_change": "none",
                },
                "repeated_action_cadence_v1": {
                    "summarized_repeated_action_count": 8,
                    "unsummarized_repeated_action_count": 0,
                    "active_inflight_repeated_action_count": 0,
                    "runtime_change": "none",
                },
                "route_stack_count": 2,
                "next_directives": {"count": 1},
            },
            "recent_action_events": {
                "repeated_actions": [{"action": "JOURNAL_PRESSURE", "count": 4}]
            },
            "modal_diversity": {
                "current_spectral_shape": {
                    "active_mode_count": 6,
                    "effective_dimensionality": 5.2,
                    "resonance_mode_packing": 1.0,
                    "pressure_mode_packing": 0.57,
                }
            },
            "feeders": [{"id": "action_thread_context_packing"}],
        }

        result = being_test_harness.test_minime_mode_share_pressure_source_probe()

        action = next(
            row
            for row in result["steward_actions_now"]
            if row["id"] == "simplify_active_thread_context"
        )
        self.assertIn("current draft work", action["reason"])
        self.assertIn("Triage the current draft", action["recommended_next"])
        self.assertNotIn("legacy draft", action["recommended_next"])

    def test_probe_moves_past_summarized_active_draft(self) -> None:
        being_test_harness.test_minime_pressure_source_audit = lambda: {
            "current_pressure": {
                "dominant_source": "mode_packing",
                "pressure_quality": "overpacked_mode_packing",
                "pressure_score": 0.3,
                "porosity_score": 0.61,
                "components": {"mode_packing": 0.57},
                "top_profile": [{"source": "mode_packing", "share": 0.22, "value": 0.57}],
            },
            "porosity_min_max": {"median": 0.61},
        }
        being_test_harness.test_minime_mode_packing_feeder_audit = lambda: {
            "current_pressure": {
                "dominant_source": "mode_packing",
                "pressure_quality": "overpacked_mode_packing",
                "pressure_score": 0.3,
                "porosity_score": 0.61,
                "components": {"mode_packing": 0.57},
                "top_profile": [{"source": "mode_packing", "share": 0.22, "value": 0.57}],
            },
            "active_thread": {
                "thread_id": "th_test",
                "current_next": "REGULATOR_AUDIT current-fill_pressure",
                "effective_next": "REGULATOR_AUDIT current-fill_pressure",
                "compression_pressure": 0.66,
                "memory_drafts": 1,
                "active_memory_drafts": 1,
                "legacy_memory_drafts": 0,
                "thread_load_triage_v1": {
                    "classification": "high_compression_active_thread",
                    "active_draft_count": 0,
                    "total_active_draft_count": 1,
                    "summarized_active_draft_count": 1,
                    "unsummarized_active_draft_count": 0,
                    "legacy_retention_count": 0,
                    "unsummarized_legacy_retention_count": 0,
                    "repeated_action_cadence_v1": {
                        "summarized_repeated_action_count": 8,
                        "unsummarized_repeated_action_count": 0,
                        "active_inflight_repeated_action_count": 0,
                        "runtime_change": "none",
                    },
                    "runtime_change": "none",
                },
                "being_memory_draft_triage_v1": {
                    "active_draft_count": 1,
                    "summarized_active_draft_count": 1,
                    "unsummarized_active_draft_count": 0,
                    "legacy_retention_count": 0,
                    "runtime_change": "none",
                },
                "repeated_action_cadence_v1": {
                    "summarized_repeated_action_count": 8,
                    "unsummarized_repeated_action_count": 0,
                    "active_inflight_repeated_action_count": 0,
                    "runtime_change": "none",
                },
                "route_stack_count": 2,
                "next_directives": {"count": 1},
            },
            "recent_action_events": {
                "repeated_actions": [{"action": "JOURNAL_PRESSURE", "count": 4}]
            },
            "modal_diversity": {
                "current_spectral_shape": {
                    "active_mode_count": 6,
                    "effective_dimensionality": 5.2,
                    "resonance_mode_packing": 1.0,
                    "pressure_mode_packing": 0.57,
                }
            },
            "feeders": [{"id": "action_thread_context_packing"}],
        }

        result = being_test_harness.test_minime_mode_share_pressure_source_probe()

        self.assertEqual(result["active_thread_pressure"]["active_memory_drafts"], 0)
        action = next(
            row
            for row in result["steward_actions_now"]
            if row["id"] == "simplify_active_thread_context"
        )
        self.assertNotIn("current draft work", action["reason"])
        self.assertIn("spectral mode crowding", action["recommended_next"])


class SpectralModeCrowdingAuditTests(unittest.TestCase):
    def setUp(self) -> None:
        self._old_modal = being_test_harness._recent_modal_diversity_summary
        self._old_eigen = being_test_harness._recent_eigen_spectrum_summary
        self._old_pressure = being_test_harness._current_minime_pressure

    def tearDown(self) -> None:
        being_test_harness._recent_modal_diversity_summary = self._old_modal
        being_test_harness._recent_eigen_spectrum_summary = self._old_eigen
        being_test_harness._current_minime_pressure = self._old_pressure

    def test_direct_crowding_watch_is_read_only(self) -> None:
        being_test_harness._recent_modal_diversity_summary = lambda hours=3.0: {
            "hours": hours,
            "recent_moment_count": 20,
            "current_spectral_shape": {
                "active_mode_count": 6,
                "active_mode_energy_ratio": 0.91,
                "effective_dimensionality": 5.3,
                "distinguishability_loss": 0.33,
                "spectral_entropy": 0.9,
                "resonance_mode_packing": 1.0,
                "pressure_mode_packing": 0.57,
            },
            "moment_effective_dimensionality": {"median": 5.3},
            "moment_distinguishability_loss": {"median": 0.33},
            "moment_pressure_sources": {"mode_packing": 20},
            "moment_semantic_admissions": {"stable_core_semantic_trickle": 20},
        }
        being_test_harness._recent_eigen_spectrum_summary = lambda: {
            "status": "present",
            "sample_count": 180,
            "active_mode_count_counts": {"5": 90, "6": 90},
            "mode_packing_min_max": {"min": 0.47, "median": 0.57, "max": 0.58},
            "porosity_min_max": {"min": 0.61, "median": 0.63, "max": 0.66},
            "latest": {"lambda4": 2.4},
        }
        being_test_harness._current_minime_pressure = lambda: {
            "dominant_source": "mode_packing",
            "pressure_quality": "overpacked_mode_packing",
            "porosity_score": 0.63,
            "pressure_score": 0.33,
        }

        result = being_test_harness.test_minime_spectral_mode_crowding_audit()

        self.assertIn("WATCH", result["verdict"])
        self.assertEqual(result["runtime_change"], "none")
        self.assertEqual(result["production_change"], "none")
        self.assertIn(
            "high_resonance_mode_packing",
            [row["id"] for row in result["crowding_flags"]],
        )
        self.assertIn("busy-but-packed", " ".join(result["interpretation"]))
        self.assertIn("Do not change aperture", result["suggested_read_only_next"][-1])


if __name__ == "__main__":
    unittest.main()
