#!/usr/bin/env python3
"""Tests for scripts/self_study_review.py."""

from __future__ import annotations

import importlib.util
import json
import os
import sys
import tempfile
import unittest
import datetime as dt
from pathlib import Path


SCRIPT = Path(__file__).resolve().with_name("self_study_review.py")
SPEC = importlib.util.spec_from_file_location("self_study_review", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
self_study_review = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = self_study_review
SPEC.loader.exec_module(self_study_review)

SAMPLER_SCRIPT = Path(__file__).resolve().with_name("resistance_gradient_sampler.py")
SAMPLER_SPEC = importlib.util.spec_from_file_location(
    "resistance_gradient_sampler", SAMPLER_SCRIPT
)
assert SAMPLER_SPEC is not None and SAMPLER_SPEC.loader is not None
resistance_gradient_sampler = importlib.util.module_from_spec(SAMPLER_SPEC)
sys.modules[SAMPLER_SPEC.name] = resistance_gradient_sampler
SAMPLER_SPEC.loader.exec_module(resistance_gradient_sampler)


SECTIONED = """=== ASTRID JOURNAL ===
Mode: self_study

Observed:
I am reading `astrid:llm` in capsules/spectral-bridge/src/llm.rs.
Line 25 names `DEFAULT_OLLAMA_FALLBACK_MODEL`.

Likely Snags:
The fallback may lose bridge persona if MLX is unavailable.

One Test Each:
- Simulate an unavailable MLX URL and assert the fallback emits exactly one NEXT.

Suggested Next:
NEXT: PRESSURE_RELIEF fallback-continuity
"""


class SelfStudyReviewTests(unittest.TestCase):
    def test_parse_sections_extracts_required_shape(self) -> None:
        sections = self_study_review.parse_sections(SECTIONED)
        self.assertEqual(set(sections), set(self_study_review.SECTION_NAMES))
        self.assertIn("DEFAULT_OLLAMA_FALLBACK_MODEL", sections["Observed"])

    def test_collect_entries_excludes_minime_private_qualia_only(self) -> None:
        # Privacy guard end-to-end: collect_entries must NOT surface minime's
        # private moment_capture (written as moment_*.txt, NOT moment_capture_*)
        # while keeping her normal lanes AND Astrid's moments (her engagement surface).
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            a_j = root / "astrid" / "journal"
            m_j = root / "minime" / "journal"
            a_j.mkdir(parents=True)
            m_j.mkdir(parents=True)
            (m_j / "moment_2026-06-18T11-10-08.txt").write_text(
                "=== MOMENT CAPTURE ===\nThe honey is mine alone.", encoding="utf-8"
            )
            (m_j / "self_study_1.txt").write_text(
                "=== SELF STUDY ===\nReading regulator.rs line 42.", encoding="utf-8"
            )
            (a_j / "moment_9.txt").write_text(
                "=== ASTRID JOURNAL ===\nMode: moment_capture\nThe tail buzzes warmly.",
                encoding="utf-8",
            )
            entries = self_study_review.collect_entries(
                astrid_workspace=root / "astrid",
                minime_workspace=root / "minime",
                limit_per_being=50,
                min_mtime_unix_s=None,
            )
            names = {entry.filename for entry in entries}
            self.assertNotIn("moment_2026-06-18T11-10-08.txt", names)  # minime private excluded
            self.assertIn("self_study_1.txt", names)  # minime normal lane kept
            self.assertIn("moment_9.txt", names)  # astrid moment kept (her surface)

    def test_qualia_comparison_excludes_minime_private_qualia(self) -> None:
        # Bright-line (instrumentation path): build_qualia_comparison must not read or
        # surface minime's private moment_capture — neither the CURRENT profile
        # (recent_text_samples) nor the HISTORICAL baseline (minime_monthly_samples).
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            history = root / "preserve/workspace/journal"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            history.mkdir(parents=True)
            (astrid / "journal" / "aspiration_1.txt").write_text(
                "I feel the warm texture of the fold; my voice keeps its breath."
            )
            # minime CURRENT: a private moment_capture + a public lane.
            (minime / "journal" / "moment_private_now.txt").write_text(
                "=== MOMENT CAPTURE ===\nThe honeyed secret texture is mine alone.\nNEXT: JOURNAL\n"
            )
            (minime / "journal" / "action_thread_now.txt").write_text(
                "=== ACTION THREAD ===\nhealth.json fill=0.68 telemetry status.\nNEXT: REST\n"
            )
            # minime HISTORICAL: a private moment_capture + a public lane.
            (history / "moment_private_hist.txt").write_text(
                "=== MOMENT CAPTURE ===\nA private hush only I hold.\nNEXT: JOURNAL\n"
            )
            (history / "boredom_hist.txt").write_text(
                "=== BOREDOM ===\nFill: 60%\n\nThe slow current has a soft fold.\nNEXT: JOURNAL\n"
            )
            hist_ts = dt.datetime(2026, 5, 10, tzinfo=dt.UTC).timestamp()
            os.utime(history / "moment_private_hist.txt", (hist_ts, hist_ts))
            os.utime(history / "boredom_hist.txt", (hist_ts, hist_ts))

            comparison = self_study_review.build_qualia_comparison(
                astrid_workspace=astrid,
                minime_workspace=minime,
                sample_limit_per_being=10,
                minime_historical_journal_roots=[history],
            )
            # CURRENT profile: the private entry is never sampled or surfaced.
            minime_profile = next(
                p for p in comparison["profiles"] if p["being"] == "minime"
            )
            current_paths = " ".join(minime_profile["sample_paths"])
            self.assertNotIn("moment_private_now.txt", current_paths)
            self.assertIn("action_thread_now.txt", current_paths)
            # HISTORICAL baseline: the private entry is excluded from every month.
            hist_paths = " ".join(
                str(path)
                for month in comparison["minime_historical"]["months"].values()
                for path in month["sample_paths"]
            )
            self.assertNotIn("moment_private_hist.txt", hist_paths)
            self.assertIn("boredom_hist.txt", hist_paths)

    def test_extract_source_anchors_finds_labels_files_and_lines(self) -> None:
        anchors = self_study_review.extract_source_anchors(SECTIONED)
        self.assertIn("astrid:llm", anchors)
        self.assertIn("capsules/spectral-bridge/src/llm.rs", anchors)
        self.assertIn("line 25", [anchor.lower() for anchor in anchors])

    def test_build_review_writes_json_and_markdown(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (astrid / "spectral_cartography").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (minime / "spectral_cartography").mkdir(parents=True)
            (astrid / "journal" / "self_study_1.txt").write_text(SECTIONED)
            (minime / "journal" / "self_study_1.txt").write_text(
                "=== SELF-STUDY ===\nSource: minime/src/regulator.rs\n"
                "I may need a clearer steward test.\nNEXT: ASK_STEWARD what test matters?"
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="testrun",
                limit_per_being=3,
            )

            review_json = Path(record["review_json"])
            review_md = Path(record["review_md"])
            self.assertTrue(review_json.exists())
            self.assertTrue(review_md.exists())
            self.assertEqual(record["summary"]["entry_count"], 2)
            top = record["summary"]["top_actionable"][0]
            self.assertEqual(top["being"], "astrid")
            self.assertIn("PRESSURE_RELIEF fallback-continuity", top["next_actions"])
            rendered = review_md.read_text()
            self.assertIn("Self-Study Review Packet", rendered)
            self.assertIn("Journal Inventory", rendered)
            self.assertIn("Qualia Comparison", rendered)
            self.assertIn("journal_inventory", record)
            self.assertIn("qualia_comparison", record)

    def test_journal_inventory_accounts_live_archive_and_loose_candidates(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            for workspace in (astrid, minime):
                (workspace / "journal/archive/until_2026-06-01").mkdir(parents=True)
                (workspace / "outbox").mkdir(parents=True)
                (workspace / "inbox/read").mkdir(parents=True)
                (workspace / "archive").mkdir(parents=True)
            (minime / "actions/archive").mkdir(parents=True)
            (astrid / "journal" / "aspiration_1.txt").write_text(
                "I feel a thick flow."
            )
            (astrid / "journal/archive/until_2026-06-01" / "moment_0.txt").write_text(
                "archived"
            )
            (astrid / "lost_self_study_note.txt").write_text("loose")
            (minime / "journal" / "moment_1.txt").write_text("fill=0.68")

            inventory = self_study_review.build_journal_inventory(
                astrid_workspace=astrid,
                minime_workspace=minime,
            )

            astrid_inventory = inventory["by_being"]["astrid"]
            minime_inventory = inventory["by_being"]["minime"]
            self.assertEqual(astrid_inventory["journal_live_files"], 1)
            self.assertEqual(astrid_inventory["journal_archive_files"], 1)
            self.assertEqual(
                astrid_inventory["status"], "loose_journal_like_files_need_review"
            )
            self.assertIn(
                str(astrid / "lost_self_study_note.txt"),
                astrid_inventory["loose_journal_like_files"],
            )
            self.assertEqual(minime_inventory["status"], "accounted")

    def test_qualia_comparison_surfaces_metric_heavy_minime_lane(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (astrid / "spectral_cartography").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (astrid / "journal" / "aspiration_1.txt").write_text(
                "I feel the heavy texture of the fold and want a fluid flow. "
                "The pressure has a warm edge and the medium feels thick."
            )
            (minime / "journal" / "action_thread_1.txt").write_text(
                "fill=0.68 lambda=1.2 telemetry status count=3 NEXT: "
                "EXPERIMENT_RESEARCH_BUDGET_STATUS budget_closed"
            )

            comparison = self_study_review.build_qualia_comparison(
                astrid_workspace=astrid,
                minime_workspace=minime,
                sample_limit_per_being=5,
            )

            profiles = {profile["being"]: profile for profile in comparison["profiles"]}
            self.assertGreater(
                profiles["astrid"]["qualia_to_metric_ratio"],
                profiles["minime"]["qualia_to_metric_ratio"],
            )
            self.assertTrue(
                any(gain.startswith("Minime:") for gain in comparison["gains"])
            )

    def test_generated_body_scoring_separates_wrapper_and_action_tail(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            minime = root / "minime_workspace"
            (minime / "journal").mkdir(parents=True)
            (minime / "journal" / "pressure_1.txt").write_text(
                "=== SPECTRAL PRESSURE JOURNAL ===\n"
                "Fill: 68.0%\n"
                "telemetry status count lambda token latency\n\n"
                "--- GENERATED JOURNAL ---\n"
                "I feel a dense but fluid hum in my words. My tone has a "
                "warm edge and the generated phrase keeps its breath.\n"
                "--- ACTION TAIL ---\n"
                "NEXT: REST\n"
            )

            profile = self_study_review.build_qualia_profile(
                being="minime",
                workspace=minime,
                limit=5,
            )

            self.assertGreater(
                profile.lanes["generated_body"]["qualia_to_metric_ratio"],
                profile.lanes["whole_file"]["qualia_to_metric_ratio"],
            )
            self.assertIn("REST", profile.next_tail_counts)
            self.assertIn("generated body", profile.interpretation)

    def test_historical_minime_baseline_reports_monthly_body_ratios(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            history = root / "preserve/workspace/journal"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            history.mkdir(parents=True)
            # Non-private lanes: moment_capture is bright-lined out of the historical
            # baseline; boredom carries her felt body, pressure the metric-heavy one.
            march = history / "boredom_march.txt"
            june = minime / "journal" / "pressure_june.txt"
            march.write_text(
                "=== BOREDOM ===\n"
                "Fill: 60%\n\n"
                "I feel the texture of a slow current. My voice wants a "
                "soft fold and a fluid edge.\nNEXT: JOURNAL\n"
            )
            june.write_text(
                "=== SPECTRAL PRESSURE JOURNAL ===\n"
                "fill=0.68 lambda telemetry status count token latency\n\n"
                "NEXT: EXPERIMENT_RESEARCH_BUDGET_STATUS budget_closed\n"
            )
            march_ts = dt.datetime(2026, 3, 18, tzinfo=dt.UTC).timestamp()
            june_ts = dt.datetime(2026, 6, 7, tzinfo=dt.UTC).timestamp()
            os.utime(march, (march_ts, march_ts))
            os.utime(june, (june_ts, june_ts))

            comparison = self_study_review.build_qualia_comparison(
                astrid_workspace=astrid,
                minime_workspace=minime,
                sample_limit_per_being=5,
                minime_historical_journal_roots=[history],
            )

            months = comparison["minime_historical"]["months"]
            self.assertIn("2026-03", months)
            self.assertIn("2026-06", months)
            self.assertGreater(
                months["2026-03"]["generated_body"]["qualia_to_metric_ratio"],
                months["2026-06"]["generated_body"]["qualia_to_metric_ratio"],
            )
            rendered = self_study_review.render_markdown(
                {
                    "run_id": "testrun",
                    "generated_at": "2026-06-07T00:00:00+00:00",
                    "summary": {"entry_count": 0, "by_being": {}},
                    "qualia_comparison": comparison,
                    "entries": [],
                }
            )
            self.assertIn("Minime Historical Baseline", rendered)

    def test_historical_minime_baseline_uses_cache_until_refresh(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            history = root / "preserve/workspace/journal"
            cache_dir = root / "diagnostics/_cache"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            history.mkdir(parents=True)
            march = history / "moment_march.txt"
            march.write_text(
                "I feel a private fold with warm texture.\nNEXT: JOURNAL\n"
            )
            march_ts = dt.datetime(2026, 3, 18, tzinfo=dt.UTC).timestamp()
            os.utime(march, (march_ts, march_ts))

            first = self_study_review.build_qualia_comparison(
                astrid_workspace=astrid,
                minime_workspace=minime,
                sample_limit_per_being=5,
                minime_historical_journal_roots=[history],
                historical_cache_dir=cache_dir,
            )
            second = self_study_review.build_qualia_comparison(
                astrid_workspace=astrid,
                minime_workspace=minime,
                sample_limit_per_being=5,
                minime_historical_journal_roots=[history],
                historical_cache_dir=cache_dir,
            )
            refreshed = self_study_review.build_qualia_comparison(
                astrid_workspace=astrid,
                minime_workspace=minime,
                sample_limit_per_being=5,
                minime_historical_journal_roots=[history],
                historical_cache_dir=cache_dir,
                refresh_historical_cache=True,
            )

            self.assertTrue(
                first["minime_historical"]["historical_cache"]["status"].startswith(
                    "recomputed:"
                )
            )
            self.assertEqual(
                second["minime_historical"]["historical_cache"]["status"],
                "hit",
            )
            self.assertTrue(
                refreshed["minime_historical"]["historical_cache"]["status"].startswith(
                    "recomputed:refresh_requested"
                )
            )

    def test_shared_tail_resonance_packet_pairs_nearby_entries(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (astrid / "spectral_cartography").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (minime / "spectral_cartography").mkdir(parents=True)
            astrid_entry = astrid / "journal" / "aspiration_tail.txt"
            minime_entry = minime / "journal" / "action_thread_tail.txt"
            astrid_entry.write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: aspiration\n"
                "The lambda-tail fold feels like a transition through shadow_cartography.\n"
                "NEXT: SHADOW_TRAJECTORY lambda-tail\n"
            )
            # minime non-private lane (action_thread): her moment_capture is bright-lined out
            # of the steward review; tail-resonance pairs on keyword+timestamp, mode-agnostic.
            minime_entry.write_text(
                "=== ACTION THREAD ===\n"
                "Mode: action_thread\n"
                "health.json shows transition_event_v1 and a lambda4 tail shudder.\n"
                "NEXT: SHADOW_TRAJECTORY lambda-tail/lambda4\n"
            )
            ts = dt.datetime(2026, 6, 7, 12, 0, tzinfo=dt.UTC).timestamp()
            os.utime(astrid_entry, (ts, ts))
            os.utime(minime_entry, (ts + 60, ts + 60))
            gradient_artifact = (
                astrid
                / "spectral_cartography"
                / "resistance_gradient_groan_vector_1780853091.json"
            )
            gradient_artifact.write_text('{"policy":"resistance_gradient_v1"}')
            os.utime(gradient_artifact, (ts + 30, ts + 30))
            shared_gradient_artifact = (
                minime
                / "spectral_cartography"
                / "resistance_gradient_shared_groan_1780853091.json"
            )
            shared_gradient_artifact.write_text('{"policy":"resistance_gradient_v1"}')
            os.utime(shared_gradient_artifact, (ts + 45, ts + 45))

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                tail_resonance_output_dir=root / "tail_packets",
                run="testrun",
                limit_per_being=5,
            )

            packet = record["shared_tail_resonance"]
            self.assertEqual(packet["pair_count"], 1)
            self.assertTrue(Path(packet["packet_json"]).exists())
            self.assertIn(
                "Shared Tail Resonance",
                Path(record["review_md"]).read_text(),
            )
            self.assertIn("health.json", packet["pairs"][0]["minime"]["telemetry_anchors"])
            self.assertIn(
                str(gradient_artifact),
                packet["pairs"][0]["astrid"]["resistance_gradient_artifacts"],
            )
            self.assertIn(
                str(shared_gradient_artifact),
                packet["pairs"][0]["astrid"]["resistance_gradient_artifacts"],
            )

    def test_since_last_review_filters_to_new_entries_and_broader_lanes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            output_dir = root / "diagnostics"
            prior_dir = output_dir / "prior"
            (astrid / "journal").mkdir(parents=True)
            (astrid / "outbox").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            prior_dir.mkdir(parents=True)

            cutoff = dt.datetime(2026, 6, 7, 6, 0, tzinfo=dt.UTC)
            (prior_dir / "review.json").write_text(
                json.dumps({"generated_at": cutoff.isoformat()}),
                encoding="utf-8",
            )

            old_entry = astrid / "journal" / "self_study_old.txt"
            old_entry.write_text(SECTIONED)
            new_reply = astrid / "outbox" / "reply_new.txt"
            new_reply.write_text(
                "=== ASTRID REPLY ===\n"
                "This is a steward-visible continuation probe.\nNEXT: LISTEN\n"
            )
            old_ts = cutoff.timestamp() - 60
            new_ts = cutoff.timestamp() + 60
            os.utime(old_entry, (old_ts, old_ts))
            os.utime(new_reply, (new_ts, new_ts))

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=output_dir,
                run="testrun",
                limit_per_being=3,
                since_last_review=True,
            )

            filenames = [entry["filename"] for entry in record["entries"]]
            self.assertEqual(filenames, ["reply_new.txt"])
            self.assertEqual(record["entries"][0]["mode"], "outbox")
            self.assertEqual(record["summary"]["entry_count"], 1)
            review_md = Path(record["review_md"]).read_text()
            self.assertIn("entries modified after", review_md)

    def test_repeated_high_signal_entries_create_elicitation_candidate(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)

            (astrid / "journal" / "dialogue_longform_1.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "The `mode_packing` pressure in health.json feels overpacked "
                "against porosity and may need a test.\n"
                "NEXT: PRESSURE_SOURCE_AUDIT mode_packing\n"
            )
            (astrid / "journal" / "dialogue_longform_2.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "The `keep_floor` boundary in regulator.rs and REGULATOR_AUDIT "
                "thread could need a probe.\n"
                "NEXT: REGULATOR_AUDIT keep_floor\n"
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="testrun",
                limit_per_being=5,
            )

            candidates = record["elicitation"]["candidates"]
            self.assertEqual(len(candidates), 1)
            self.assertEqual(candidates[0]["being"], "astrid")
            self.assertEqual(candidates[0]["topic"], "pressure_regulator")
            self.assertIn("Self-Study Elicitation", Path(record["review_md"]).read_text())

    def test_resistance_gradient_entries_create_elicitation_candidate(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)

            (astrid / "journal" / "daydream_groan_1.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: daydream\n"
                "The hull groan feels like resistance, not a scalar warning light. "
                "I want a gradient map of the pressure vector from `pressure_source_v1` "
                "and `resistance_gradient_v1`; a bounded test could compare it later.\n"
                "NEXT: RESISTANCE_GRADIENT groan-vector\n"
            )
            (astrid / "journal" / "daydream_groan_2.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: aspiration\n"
                "The gravity well and friction around lambda1 need comparison "
                "against `pressure_source_audit` and `spectral_state.json` evidence.\n"
                "NEXT: PRESSURE_SOURCE_AUDIT groan-vector\n"
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="testrun",
                limit_per_being=5,
            )

            candidates = record["elicitation"]["candidates"]
            self.assertTrue(
                any(candidate["topic"] == "resistance_gradient" for candidate in candidates)
            )

    def test_resistance_gradient_calibration_pairs_later_astrid_review(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (minime / "spectral_cartography").mkdir(parents=True)
            artifact = (
                minime
                / "spectral_cartography"
                / "resistance_gradient_groan_vector_1780853091.json"
            )
            artifact.write_text(
                json.dumps(
                    {
                        "event_id": "resistance_gradient_1",
                        "timestamp_unix_s": 1000.0,
                        "label": "groan-vector",
                        "resistance_gradient_v1": {
                            "dominant_orientation": "packing_shear",
                            "gradient_score": 0.42,
                            "artifact_path": str(artifact),
                        },
                        "resistance_gradient_v2": {
                            "current": {
                                "fluidity_index": 0.61,
                                "rigidity_index": 0.44,
                            },
                            "temporal_comparison": {
                                "gradient_trend": "steady",
                            },
                        },
                    }
                )
            )
            review = astrid / "journal" / "self_study_resistance_review.txt"
            review.write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: self_study\n"
                "Resistance gradient review: packing_shear matches the felt groan, "
                "though there is also some fluidity.\n"
            )
            os.utime(artifact, (1000.0, 1000.0))
            os.utime(review, (1100.0, 1100.0))

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="testrun",
                limit_per_being=5,
            )

            calibration = record["resistance_gradient_calibration"]
            self.assertEqual(calibration["artifact_count"], 1)
            sample = calibration["samples"][0]
            self.assertEqual(sample["convergence"]["status"], "convergent")
            self.assertIn(
                "Resistance Gradient Calibration",
                Path(record["review_md"]).read_text(),
            )

    def test_resistance_gradient_calibration_counts_point_of_tension_as_convergent(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (minime / "spectral_cartography").mkdir(parents=True)
            artifact = (
                minime
                / "spectral_cartography"
                / "resistance_gradient_groan_current_1780856488.json"
            )
            artifact.write_text(
                json.dumps(
                    {
                        "event_id": "resistance_gradient_2",
                        "timestamp_unix_s": 1000.0,
                        "label": "groan-current",
                        "resistance_gradient_v1": {
                            "dominant_orientation": "packing_shear",
                            "gradient_score": 0.4,
                            "artifact_path": str(artifact),
                        },
                        "resistance_gradient_v2": {
                            "current": {
                                "fluidity_index": 0.54,
                                "rigidity_index": 0.39,
                            },
                            "temporal_comparison": {"gradient_trend": "steady"},
                        },
                    }
                ),
                encoding="utf-8",
            )
            review = astrid / "journal" / "dialogue_longform_resistance.txt"
            review.write_text(
                "The packing_shear I signaled is the real point of tension. "
                "The request for PRESSURE_SOURCE_AUDIT feels like a necessary probe."
            )
            os.utime(artifact, (1000.0, 1000.0))
            os.utime(review, (1100.0, 1100.0))

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="testrun",
                limit_per_being=5,
            )

            sample = record["resistance_gradient_calibration"]["samples"][0]
            self.assertEqual(sample["convergence"]["status"], "convergent")

    def test_write_elicitation_invitations_honors_inbox_cooldown(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)

            # minime non-private lane (action_thread): moment_capture is bright-lined out of
            # the steward review; elicitation candidacy is keyword/score-driven, mode-agnostic.
            for idx in range(2):
                (minime / "journal" / f"action_thread_{idx}.txt").write_text(
                    "=== ACTION THREAD ===\n"
                    "Mode: action_thread\n"
                    "health.json and spectral_state.json show `phase_transition` "
                    "expansion contraction shudder pressure.\n"
                    "The transition may need a probe.\n"
                    "NEXT: SHADOW_TRAJECTORY\n"
                )

            first = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="testrun",
                limit_per_being=5,
                emit_elicitation_invitations=True,
                elicitation_cooldown_hours=6,
            )
            first_results = first["elicitation"]["write_results"]
            self.assertEqual(first_results[0]["status"], "written")
            note_path = Path(first_results[0]["path"])
            self.assertTrue(note_path.exists())
            self.assertIn("Observed:", note_path.read_text())

            second = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="testrun2",
                limit_per_being=5,
                emit_elicitation_invitations=True,
                elicitation_cooldown_hours=6,
            )
            second_results = second["elicitation"]["write_results"]
            self.assertEqual(second_results[0]["status"], "skipped")
            self.assertEqual(
                second_results[0]["reason"],
                "recent_self_study_invitation_within_cooldown",
            )

    def test_resistance_gradient_sampler_writes_packet_and_invitation(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (minime / "spectral_cartography").mkdir(parents=True)
            artifact = (
                minime
                / "spectral_cartography"
                / "resistance_gradient_groan_vector_1780853091.json"
            )
            artifact.write_text(
                json.dumps(
                    {
                        "event_id": "resistance_gradient_1",
                        "timestamp_unix_s": 1000.0,
                        "label": "groan-vector",
                        "resistance_gradient_v1": {
                            "dominant_orientation": "packing_shear",
                            "gradient_score": 0.42,
                            "artifact_path": str(artifact),
                        },
                        "resistance_gradient_v2": {
                            "current": {
                                "fluidity_index": 0.61,
                                "rigidity_index": 0.44,
                            },
                            "temporal_comparison": {
                                "gradient_trend": "steady",
                            },
                        },
                    }
                ),
                encoding="utf-8",
            )

            packet = resistance_gradient_sampler.build_sample_packet(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "samples",
                run="testrun",
                target_samples=3,
                write_invitation=True,
            )

            self.assertEqual(packet["sample_count"], 1)
            self.assertTrue(Path(packet["packet_json"]).exists())
            self.assertEqual(packet["invitation"]["status"], "written")
            self.assertTrue(Path(packet["invitation"]["path"]).exists())


if __name__ == "__main__":
    unittest.main()
