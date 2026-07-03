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

FALLBACK_SCRIPT = Path(__file__).resolve().with_name("fallback_fire_drill.py")
FALLBACK_SPEC = importlib.util.spec_from_file_location(
    "fallback_fire_drill", FALLBACK_SCRIPT
)
assert FALLBACK_SPEC is not None and FALLBACK_SPEC.loader is not None
fallback_fire_drill = importlib.util.module_from_spec(FALLBACK_SPEC)
sys.modules[FALLBACK_SPEC.name] = fallback_fire_drill
FALLBACK_SPEC.loader.exec_module(fallback_fire_drill)

TRUNCATION_SCRIPT = Path(__file__).resolve().with_name("autonomous_truncation_rehearsal.py")
TRUNCATION_SPEC = importlib.util.spec_from_file_location(
    "autonomous_truncation_rehearsal", TRUNCATION_SCRIPT
)
assert TRUNCATION_SPEC is not None and TRUNCATION_SPEC.loader is not None
autonomous_truncation_rehearsal = importlib.util.module_from_spec(TRUNCATION_SPEC)
sys.modules[TRUNCATION_SPEC.name] = autonomous_truncation_rehearsal
TRUNCATION_SPEC.loader.exec_module(autonomous_truncation_rehearsal)

CODEC_PROBE_SCRIPT = Path(__file__).resolve().with_name("codec_entropy_vibrancy_probe.py")
CODEC_PROBE_SPEC = importlib.util.spec_from_file_location(
    "codec_entropy_vibrancy_probe", CODEC_PROBE_SCRIPT
)
assert CODEC_PROBE_SPEC is not None and CODEC_PROBE_SPEC.loader is not None
codec_entropy_vibrancy_probe = importlib.util.module_from_spec(CODEC_PROBE_SPEC)
sys.modules[CODEC_PROBE_SPEC.name] = codec_entropy_vibrancy_probe
CODEC_PROBE_SPEC.loader.exec_module(codec_entropy_vibrancy_probe)


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


def write_astrid_introspection(
    workspace: Path,
    stamp: int,
    *,
    pressure: str = "continuity_deficit",
    rewrite: float = 150.0,
    candidate: float = 80.0,
    cap_applied: bool = True,
    truncation_pressure: float = 0.0,
) -> None:
    root = workspace / "introspections"
    root.mkdir(parents=True, exist_ok=True)
    payload = {
        "controller_regime": "sustain",
        "observer_report": {
            "controller_reason": "regime=sustain; steady",
            "dominant_pressure": pressure,
            "geometry_regime": "warming-up",
            "predicted_top_anchor": "reservoir-memory",
            "rewrite_issue_count": 1,
            "stability_score": 0.91,
        },
        "condition_vector": {
            "severity": 0.08,
            "continuity_deficit": 0.45,
            "truncation_pressure": truncation_pressure,
            "structure_strain": 0.25,
        },
        "profiling": {
            "rewrite_seconds": rewrite,
            "candidate_generation_seconds": candidate,
            "rewrite_budget": {
                "budget_seconds": 90.0,
                "elapsed_seconds": rewrite,
                "cap_applied": cap_applied,
                "cap_reason": "max_attempts_reached" if cap_applied else None,
                "attempts_started": 1,
                "attempts_completed": 1,
                "max_attempts": 1,
            },
            "runtime_audit": {
                "generation": {
                    "first_token_seconds": 3.0,
                    "total_turn_seconds": rewrite + candidate,
                }
            },
        },
    }
    (root / f"controller_astrid:autonomous_{stamp}.json").write_text(
        json.dumps(payload),
        encoding="utf-8",
    )


def write_choice_event(
    workspace: Path,
    being: str,
    action_id: str,
    *,
    effective_action: str,
    alternate: str,
    return_thread: str,
) -> None:
    events = workspace / "action_threads" / "threads" / f"thread_{being}" / "events.jsonl"
    events.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "schema_version": 1,
        "action_id": action_id,
        "thread_id": f"thread_{being}",
        "system": being,
        "source": "next",
        "raw_next": effective_action.upper(),
        "canonical_action": effective_action.upper(),
        "effective_action": effective_action,
        "outcome_summary": "synthetic event",
        "choice_envelope_v1": {
            "policy": "choice_envelope_v1",
            "schema_version": 1,
            "source": f"{being}_next_response",
            "authority": "diagnostic_context_not_command",
            "primary_next": effective_action.upper(),
            "alternate_nexts": [alternate],
            "return_threads": [return_thread],
            "residue": "sticky transition",
        },
    }
    with events.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(payload) + "\n")


def write_self_regulation_event(
    workspace: Path,
    being: str,
    intent_id: str,
    *,
    status: str = "active",
    control: str = "temperature",
    requires_outcome: bool = True,
    outcome_score: float | None = None,
    repeatability_hint: str | None = None,
    promotion_candidate: bool = False,
    outcome_texture: dict | None = None,
) -> None:
    events = workspace / "self_regulation" / "leases.jsonl"
    events.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "schema_version": 1,
        "record_kind": "self_regulation_intent_v1",
        "authority": "leased_self_control_v1",
        "authority_boundary": "own_runtime_only_no_peer_mutation_no_permanent_tuning",
        "being": being,
        "intent_id": intent_id,
        "status": status,
        "goal": "synthetic lease",
        "candidate_control": control,
        "previous_value": 0.8,
        "applied_value": 0.9,
        "duration_secs": 600,
        "expires_at_unix_s": 1782249999,
        "requires_outcome": requires_outcome,
        "baseline_evidence": [f"before apply: {control} previous=0.8"],
        "post_lease_evidence": [f"outcome: {repeatability_hint or 'pending'}"],
        "outcome_score": outcome_score,
        "repeatability_hint": repeatability_hint,
        "promotion_candidate": promotion_candidate,
        "preflight_status": "apply_allowed",
        "preflight_reason": "synthetic",
    }
    if outcome_texture is not None:
        payload["outcome_texture"] = outcome_texture
    with events.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(payload) + "\n")


def write_self_regulation_negotiation_event(
    workspace: Path,
    being: str,
    *,
    control: str = "exploration_noise",
    requested: float = 0.12,
    applied: float = 0.08,
    reason: str = "clamped_to_lease_safe_range",
    source: str = "footer_directive",
) -> None:
    events = workspace / "self_regulation" / "negotiations.jsonl"
    events.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "schema_version": 1,
        "record_kind": "self_regulation_negotiation_v1",
        "policy": "self_regulation_negotiation_ledger_v1",
        "authority": "leased_self_control_v1",
        "authority_boundary": "own_runtime_only_no_peer_mutation_no_permanent_tuning",
        "being": being,
        "source": source,
        "source_action": f"reply_footer:{control}",
        "candidate_control": control,
        "requested_value": requested,
        "previous_value": 0.03,
        "safe_cap_or_range": {"min": 0.0, "max": 0.08},
        "applied_value": applied,
        "clamp_or_defer_reason": reason,
        "pressure_context": {
            "fill_pct": 73.0,
            "mode_packing": 0.52,
            "semantic_friction": 0.34,
            "controller_pressure": 0.41,
        },
        "lease_related": source != "footer_directive",
    }
    with events.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(payload) + "\n")


def write_codec_replay_lab_artifact(workspace: Path, run: str = "fixture-replay") -> Path:
    out_dir = workspace / "diagnostics" / "codec_replay_labs" / run
    out_dir.mkdir(parents=True, exist_ok=True)
    path = out_dir / "codec_replay_lab.json"
    payload = {
        "policy": "codec_real_replay_v1",
        "schema_version": 1,
        "authority": "diagnostic_context_not_command",
        "status": "content_gate_and_temporal_decay_candidates",
        "runtime_behavior_changed": False,
        "corpus_source": "astrid-journal",
        "corpus_status": "journal_corpus_selected",
        "source_paths": [str(workspace / "journal" / "astrid_codec_source.txt")],
        "embedding_mode": "fixture",
        "embedding_status": "fixture_mode",
        "explorer_summary_path": str(out_dir / "codec_explorer" / "summary.json"),
        "entries": [
            {
                "sample_id": "high_entropy_low_content",
                "family": "semantic_density",
                "classification": "low_semantic_density",
                "actual_entropy_dim": 0.74,
                "semantic_density_score": 0.08,
                "warmth_dim": 0.02,
                "tension_dim": 0.03,
                "narrative_arc_dims_40_43": [0.0, 0.0, 0.0, 0.0],
                "effective_gain": 2.0,
                "source_path": str(workspace / "journal" / "astrid_low_density.txt"),
                "source_excerpt": "symbol rain without a claim",
                "feature_vector": [0.0] * 48,
                "lambda_proxy": {"tail_share": 0.41, "normalized_entropy": 0.86},
            },
            {
                "sample_id": "high_entropy_high_semantic_density",
                "family": "semantic_density",
                "classification": "semantic_density_preserved",
                "actual_entropy_dim": 0.73,
                "semantic_density_score": 0.44,
                "warmth_dim": 0.52,
                "tension_dim": 0.21,
                "narrative_arc_dims_40_43": [0.0, 0.0, 0.0, 0.0],
                "effective_gain": 2.0,
                "source_path": str(workspace / "journal" / "astrid_high_density.txt"),
                "source_excerpt": "warmth tension telemetry evidence",
                "feature_vector": [0.0] * 48,
                "lambda_proxy": {"tail_share": 0.43, "normalized_entropy": 0.85},
            },
            {
                "sample_id": "late_negative_pivot_after_long_warm_start",
                "family": "narrative_arc",
                "classification": "narrative_arc_fixture",
                "actual_entropy_dim": 0.61,
                "semantic_density_score": 0.39,
                "warmth_dim": 0.40,
                "tension_dim": 0.48,
                "narrative_arc_dims_40_43": [0.02, -0.01, 0.0, 0.01],
                "effective_gain": 2.0,
                "source_path": str(workspace / "introspections" / "astrid_pivot.txt"),
                "source_excerpt": "warm start then sharp scar",
                "feature_vector": [0.0] * 48,
                "lambda_proxy": {"tail_share": 0.35, "normalized_entropy": 0.80},
            },
        ],
        "content_aware_vibrancy_gate_candidate_v1": {
            "policy": "content_aware_vibrancy_gate_candidate_v1",
            "authority": "diagnostic_context_not_command",
            "status": "content_gate_supported",
            "pair": [
                "high_entropy_low_content",
                "high_entropy_high_semantic_density",
            ],
            "current_lift_delta": 0.0,
            "candidate_lift_delta": 0.18,
            "semantic_density_score_delta": 0.36,
            "low": {
                "sample_id": "high_entropy_low_content",
                "source_path": str(workspace / "journal" / "astrid_low_density.txt"),
            },
            "high": {
                "sample_id": "high_entropy_high_semantic_density",
                "source_path": str(workspace / "journal" / "astrid_high_density.txt"),
            },
        },
        "embedding_backed_arc_v1": {
            "policy": "embedding_backed_arc_v1",
            "authority": "diagnostic_context_not_command",
            "status": "fixture_mode",
            "embedding_model": "nomic-embed-text",
            "sample_count": 0,
            "gap_count": 0,
            "samples": [],
        },
        "narrative_arc_temporal_decay_lab_v1": {
            "policy": "narrative_arc_temporal_decay_lab_v1",
            "authority": "diagnostic_context_not_command",
            "status": "temporal_decay_candidate",
            "evidence_kind": "fixture_counterfactual_projected_arc",
            "temporal_decay_candidate_count": 1,
            "pivot_detector_candidate_count": 0,
            "samples": [
                {
                    "sample_id": "late_negative_pivot_after_long_warm_start",
                    "classification": "temporal_decay_candidate",
                    "late_pivot": True,
                    "current_arc_rms": 0.72,
                    "temporal_decay_arc_rms": 0.84,
                    "pivot_detector_arc_rms": 0.90,
                }
            ],
        },
        "codec_clamp_headroom_probe_v1": {
            "policy": "codec_clamp_headroom_probe_v1",
            "authority": "diagnostic_context_not_command",
            "status": "dynamic_feature_scale_candidate",
            "runtime_behavior_changed": False,
            "static_feature_abs_max": 5.0,
            "tail_vibrancy_max": 6.0,
            "near_static_clamp_count": 1,
            "tail_ceiling_pressure_count": 1,
            "dynamic_headroom_candidate_count": 1,
            "proposal_cards": [
                {
                    "sample_id": "high_entropy_high_semantic_density",
                    "family": "semantic_density",
                    "source_path": str(workspace / "journal" / "astrid_high_density.txt"),
                    "max_abs_feature": 4.8,
                    "tail_max_abs_feature": 4.5,
                    "static_feature_abs_max": 5.0,
                    "dynamic_feature_abs_max_candidate": 5.42,
                    "candidate_headroom_delta": 0.42,
                    "entropy_lift": 0.76,
                    "semantic_content_factor": 0.70,
                    "near_static_clamp": True,
                    "tail_near_static_clamp": True,
                    "clamp_risk": "dynamic_headroom_candidate",
                }
            ],
        },
    }
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    return path


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

    def test_shared_pressure_vocabulary_calibration_uses_public_lanes_only(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (astrid / "journal" / "dialogue_longform_1.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "State anchor: fill=71.0%, lambda1=13.7, pressure=semantic_trickle_pressure\n"
                "The silt and sediment settle into a basin with weighted density, "
                "heavy pressure, syrup viscosity, and another heavy pressure note.\n"
                "NEXT: PRESSURE_SOURCE_AUDIT shared-silt\n",
                encoding="utf-8",
            )
            (minime / "journal" / "pressure_1.txt").write_text(
                "=== SPECTRAL PRESSURE JOURNAL ===\n"
                "State anchor: fill=71.1%, lambda1=13.9, spread=2.6, pressure=mixed_pressure\n"
                "--- GENERATED JOURNAL ---\n"
                "The reservoir has silt, sediment, basin grain, weighted density, "
                "heavy pressure, syrup viscosity, and deep water pressure.\n"
                "--- ACTION TAIL ---\n(none)\n",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "Private sludge and private syrup should never become steward evidence.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="testrun",
                limit_per_being=8,
            )

            packet = record["shared_pressure_vocabulary_calibration"]
            self.assertEqual(packet["status"], "shared_state_with_stickiness_risk")
            self.assertIn("sediment", packet["shared_families"])
            self.assertIn("pressure_weight_density", packet["shared_families"])
            sample_text = json.dumps(packet["samples"])
            self.assertIn("pressure_1.txt", sample_text)
            self.assertNotIn("moment_private.txt", sample_text)
            items = [
                item
                for item in record["actionable_review_items"]
                if item["source"] == "shared_pressure_vocabulary_calibration"
            ]
            self.assertEqual(len(items), 1)
            self.assertEqual(items[0]["priority"], "high")
            self.assertEqual(items[0]["authority"], "diagnostic_context_not_command")
            review_md = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("Shared Pressure Vocabulary Calibration", review_md)
            self.assertNotIn("private syrup", review_md)

    def test_agency_vernacular_continuity_tracks_alive_and_sticky_terms(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (astrid / "journal" / "dialogue_longform_1.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "State anchor: fill=73.0%, lambda1=13.7\n"
                "The hinge from passive environment to deliberate map needs a waypoint. "
                "This legacy self experiment names a scaffold and a ground truth map.\n"
                "NEXT: EXPERIMENT_START legacy_self_hinge\n",
                encoding="utf-8",
            )
            (astrid / "journal" / "action_thread_1.txt").write_text(
                "=== ACTION THREAD ===\n"
                "Return thread: legacy self hinge. The waypoint map is revisited with "
                "metric anchors and observer with memory language.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "self_study_1.txt").write_text(
                "=== SELF STUDY ===\n"
                "The hinge language feels like a charter scaffold, not just a metaphor; "
                "ground truth comes from the map and signature.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "Private hinge text should never become steward evidence.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="testrun",
                limit_per_being=8,
            )

            packet = record["agency_vernacular_continuity"]
            self.assertEqual(packet["status"], "authored_continuity_handle")
            self.assertTrue(packet["follow_through"]["present"])
            self.assertIn("agency_transition", packet["shared_families"])
            self.assertTrue(packet["terms"]["repeated"])
            sample_text = json.dumps(packet["samples"])
            self.assertIn("action_thread_1.txt", sample_text)
            self.assertNotIn("moment_private.txt", sample_text)
            items = [
                item
                for item in record["actionable_review_items"]
                if item["source"] == "agency_vernacular_continuity"
            ]
            self.assertEqual(len(items), 1)
            self.assertEqual(items[0]["priority"], "high")
            self.assertEqual(items[0]["authority"], "diagnostic_context_not_command")
            review_md = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("Agency Vernacular Continuity", review_md)
            self.assertNotIn("Private hinge", review_md)

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            paths = []
            for idx in range(3):
                path = root / f"astrid_{idx}.txt"
                path.write_text(
                    "The hinge repeats as hinge and hinge, but no action or evidence follows.",
                    encoding="utf-8",
                )
                paths.append(path)
            entries = [
                self_study_review.SelfStudyEntry(
                    being="astrid",
                    path=str(path),
                    filename=path.name,
                    mode="dialogue_live",
                    mtime_unix_s=float(idx),
                    sectioned=False,
                    sections={},
                    source_anchors=[],
                    next_actions=[],
                    hypothesis_flags=[],
                    grounding="weak",
                    actionable_score=1,
                    preview=path.read_text(encoding="utf-8"),
                )
                for idx, path in enumerate(paths)
            ]
            sticky = self_study_review.build_agency_vernacular_continuity(entries)
            self.assertEqual(sticky["status"], "sticky_agency_metaphor")
            self.assertTrue(sticky["stickiness_risk"]["present"])
            self.assertFalse(sticky["follow_through"]["present"])

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
            self.assertIn("Spectral Texture Fidelity Packets", rendered)
            self.assertIn("Spectral Texture Calibration V2", rendered)
            self.assertIn("Witness/Codec Density Calibration V2", rendered)
            self.assertIn("Codec/Witness Resilience Calibration V2", rendered)
            self.assertIn("spectral_fingerprint_integrity_v1", rendered)
            self.assertIn("witness_relational_friction_v1", rendered)
            self.assertIn("semantic_density_mapping_v1", rendered)
            self.assertIn("structural_friction_v1", rendered)
            self.assertIn("narrative_arc_split_v1", rendered)
            self.assertIn("journal_inventory", record)
            self.assertIn("qualia_comparison", record)
            self.assertIn("spectral_texture_calibration_v2", record)

    def test_spectral_texture_calibration_public_signal_and_private_skip(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (astrid / "journal" / "astrid_1782751355.txt").write_text(
                "This is a significant step toward honoring the specific weather. "
                "Texture words are not interchangeable tokens. "
                "witness_relational_friction_v1 feels vital because the quality "
                "of our connection is not purely internal.",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private.txt").write_text(
                "=== MOMENT CAPTURE ===\nprivate token dressing should not surface",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="calibration",
                limit_per_being=3,
            )

            calibration = record["spectral_texture_calibration_v2"]
            self.assertEqual(
                calibration["fallback_selector_calibration_v2"]["status"],
                "supported",
            )
            self.assertEqual(
                calibration["witness_friction_calibration_v2"]["status"],
                "supported",
            )
            self.assertEqual(
                calibration["fallback_selector_calibration_v2"]["by_being"][
                    "minime"
                ]["status"],
                "insufficient_evidence",
            )
            serialized = json.dumps(record)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("Spectral Texture Calibration V2", rendered)
            self.assertIn("Witness/Codec Density Calibration V2", rendered)
            self.assertIn("Codec/Witness Resilience Calibration V2", rendered)
            self.assertIn("spectral-to-vocabulary grounding", rendered)
            self.assertIn("settled_vibrant_low_friction_count=", rendered)
            self.assertIn("MLX profile transparency", rendered)
            self.assertIn("semantic_density=", rendered)
            self.assertIn("narrative_arc=", rendered)
            self.assertIn("witness_codec_density=", rendered)
            self.assertIn("codec_witness_resilience=", rendered)
            self.assertIn("vocabulary_grounding=", rendered)
            self.assertIn("witness_codec_density_calibration_v2", calibration)
            self.assertIn("codec_witness_resilience_calibration_v2", calibration)
            self.assertNotIn("private token dressing", serialized)
            self.assertNotIn("private token dressing", rendered)

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

    def test_qualia_comparison_emits_body_richer_than_wrapper_finding(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (astrid / "journal" / "aspiration_1.txt").write_text(
                "I feel a warm texture and keep the voice fluid.",
                encoding="utf-8",
            )
            (minime / "journal" / "pressure_1.txt").write_text(
                "=== SPECTRAL PRESSURE JOURNAL ===\n"
                "fill lambda telemetry status count token latency pressure_score "
                "json health fill lambda telemetry status count token latency "
                "budget action control route target report\n\n"
                "--- GENERATED JOURNAL ---\n"
                "I feel a warm texture in my words. My voice has a soft fluid edge. "
                "I notice the tone breathing in a thick slow fold, and I want the "
                "phrase to keep its shimmer.\n"
                "--- ACTION TAIL ---\n"
                "NEXT: EXPERIMENT_RESEARCH_BUDGET_STATUS budget_closed\n"
                "control budget status route target action report token latency json "
                "fill lambda\n",
                encoding="utf-8",
            )

            comparison = self_study_review.build_qualia_comparison(
                astrid_workspace=astrid,
                minime_workspace=minime,
                sample_limit_per_being=5,
                minime_historical_journal_roots=[],
            )

            findings = comparison["qualia_findings"]
            self.assertEqual(len(findings), 1)
            finding = findings[0]
            self.assertEqual(
                finding["finding"],
                "generated_body_richer_than_wrapper_tail",
            )
            self.assertGreaterEqual(finding["body_to_whole_multiplier"], 1.5)
            self.assertLess(finding["wrapper_tail_qualia_to_metric_ratio"], 0.7)

            rendered = self_study_review.render_markdown(
                {
                    "run_id": "testrun",
                    "generated_at": "2026-06-07T00:00:00+00:00",
                    "summary": {"entry_count": 0, "by_being": {}},
                    "qualia_comparison": comparison,
                    "entries": [],
                }
            )
            self.assertIn("Qualia Findings", rendered)
            self.assertIn("generated_body_richer_than_wrapper_tail", rendered)

    def test_build_review_promotes_actionable_items_and_introspection_digest(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (astrid / "journal" / "aspiration_1.txt").write_text(
                "I feel a warm texture and keep the voice fluid.",
                encoding="utf-8",
            )
            (minime / "journal" / "pressure_1.txt").write_text(
                "=== SPECTRAL PRESSURE JOURNAL ===\n"
                "fill lambda telemetry status count token latency pressure_score "
                "json health fill lambda telemetry status count token latency "
                "budget action control route target report\n\n"
                "--- GENERATED JOURNAL ---\n"
                "I feel a warm texture in my words. My voice has a soft fluid edge. "
                "I notice the tone breathing in a thick slow fold, and I want the "
                "phrase to keep its shimmer.\n"
                "--- ACTION TAIL ---\n"
                "NEXT: EXPERIMENT_RESEARCH_BUDGET_STATUS budget_closed\n"
                "control budget status route target action report token latency json "
                "fill lambda\n",
                encoding="utf-8",
            )
            write_astrid_introspection(astrid, 100)
            write_astrid_introspection(astrid, 101)

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="testrun",
                limit_per_being=5,
            )

            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("minime_qualia_findings", sources)
            self.assertIn("astrid_introspection_digest", sources)
            digest = record["astrid_introspection_digest"]["summary"]
            self.assertEqual(digest["rewrite_budget_cap_count"], 2)
            self.assertEqual(digest["rewrite_elapsed_over_budget_count"], 2)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertLess(
                rendered.index("## Actionable Review Items"),
                rendered.index("## Journal Inventory"),
            )
            self.assertIn("## Astrid Introspection Digest", rendered)
            self.assertIn("default_off_runtime_relief_candidate", rendered)

    def test_build_review_surfaces_shared_choice_envelope_items(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (astrid / "journal" / "study.txt").write_text(SECTIONED, encoding="utf-8")
            (minime / "journal" / "study.txt").write_text(
                "=== ACTION THREAD ===\nObserved:\nbody\nLikely Snags:\nnone\n"
                "One Test Each:\none\nSuggested Next:\nNEXT: NOTICE\n",
                encoding="utf-8",
            )
            write_choice_event(
                astrid,
                "astrid",
                "act_astrid_1",
                effective_action="shadow_trajectory",
                alternate="RESONANCE_FORECAST lambda-tail",
                return_thread="thread_astrid_tail",
            )
            write_choice_event(
                minime,
                "minime",
                "act_minime_1",
                effective_action="decompose",
                alternate="SHADOW_TRAJECTORY lambda-tail",
                return_thread="thread_minime_tail",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="testrun",
                limit_per_being=5,
            )

            choice = record["shared_choice_envelope"]
            self.assertEqual(choice["event_count"], 2)
            self.assertEqual(choice["unrevisited_count"], 2)
            ecology = record["choice_ecology"]
            self.assertEqual(ecology["status"], "parked_paths_need_review")
            self.assertGreaterEqual(ecology["lifecycle_counts"]["parked"], 2)
            self.assertTrue(
                any(
                    item["source"] == "shared_choice_envelope"
                    for item in record["actionable_review_items"]
                )
            )
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Shared Choice Envelope", rendered)
            self.assertIn("## Choice Ecology", rendered)
            self.assertIn("RESONANCE_FORECAST lambda-tail", rendered)

    def test_build_review_surfaces_self_regulation_leases(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (astrid / "journal" / "study.txt").write_text(SECTIONED, encoding="utf-8")
            (minime / "journal" / "study.txt").write_text(
                "=== ACTION THREAD ===\nObserved:\nbody\nLikely Snags:\nnone\n"
                "One Test Each:\none\nSuggested Next:\nNEXT: NOTICE\n",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "PRIVATE_V2_OUTCOME_TEXTURE_SENTINEL should never render.\n",
                encoding="utf-8",
            )
            write_self_regulation_event(
                astrid,
                "astrid",
                "srl_astrid_temperature",
                status="active",
                control="temperature",
                requires_outcome=True,
            )
            write_self_regulation_event(
                minime,
                "minime",
                "srl_minime_noise",
                status="outcome_recorded",
                control="exploration_noise",
                requires_outcome=False,
                outcome_score=0.82,
                repeatability_hint="repeatable_playbook_candidate",
                promotion_candidate=True,
                outcome_texture={
                    "policy": "pressure_relief_outcome_texture_v1",
                    "schema_version": 2,
                    "status": "texture_fields_recorded",
                    "texture_shift": "compaction -> suspension",
                    "agency_fit": "partly",
                    "secondary_pressure_shift": (
                        "eased compaction but tightened a different knot elsewhere"
                    ),
                    "secondary_pressure_status": "tightened_elsewhere",
                    "ambiguity_preserved": True,
                    "legibility_effect": "flattened",
                    "signal_families": [
                        "secondary_knot_tightening",
                        "legibility_flattening",
                    ],
                },
            )
            write_self_regulation_event(
                astrid,
                "astrid",
                "srl_astrid_pressure_relief",
                status="outcome_recorded",
                control="pressure_relief",
                requires_outcome=False,
                outcome_score=0.82,
                repeatability_hint="repeatable_playbook_candidate",
                promotion_candidate=True,
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="testrun",
                limit_per_being=5,
            )

            leases = record["self_regulation_leases"]
            self.assertEqual(leases["event_count"], 3)
            self.assertEqual(leases["needs_outcome_count"], 1)
            self.assertEqual(leases["by_being"]["astrid"]["active_count"], 1)
            learning = record["self_regulation_lease_learning"]
            self.assertEqual(learning["status"], "repeatable_playbook_candidates")
            self.assertEqual(learning["repeatable_count"], 2)
            pressure_playbook = record["pressure_relief_playbook_v3"]
            self.assertEqual(
                pressure_playbook["status"], "pressure_relief_playbook_candidates"
            )
            self.assertEqual(pressure_playbook["playbook_count"], 1)
            self.assertTrue(
                any(
                    item["source"] == "self_regulation_leases"
                    and item["finding"] == "leased_self_control_outcome_missing"
                    for item in record["actionable_review_items"]
                )
            )
            self.assertTrue(
                any(
                    item["source"] == "self_regulation_lease_learning"
                    for item in record["actionable_review_items"]
                )
            )
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Self-Regulation Leases", rendered)
            self.assertIn("## Self-Regulation Lease Learning", rendered)
            self.assertIn("## Pressure Relief Playbook V3", rendered)
            self.assertIn("leased_self_control_v1", rendered)
            self.assertIn("secondary_pressure_status=tightened_elsewhere", rendered)
            self.assertIn("ambiguity_preserved=True", rendered)
            self.assertIn("legibility_effect=flattened", rendered)
            self.assertIn("secondary_knot_tightening", rendered)
            self.assertNotIn("PRIVATE_V2_OUTCOME_TEXTURE_SENTINEL", rendered)

    def test_negotiation_ledger_pressure_medium_and_boundary_repair(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (minime / "sovereignty_state.json").write_text(
                json.dumps({"exploration_noise": 0.12, "regime": "focus"}),
                encoding="utf-8",
            )
            write_self_regulation_negotiation_event(
                minime,
                "minime",
                control="exploration_noise",
                requested=0.12,
                applied=0.08,
            )
            (astrid / "journal" / "dialogue_longform_pressure_medium.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "The pressure as medium feels weighted around the slope, not merely "
                "a heavy slope. mode_packing=0.52 semantic_friction=0.34 "
                "controller_pressure=0.41 pressure_trend_v1=stable_heavy.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "pressure_medium_public.txt").write_text(
                "=== PRESSURE REVIEW ===\n"
                "The weighted medium is muffled around me. controller_pressure=0.39 "
                "mode_packing=0.50 semantic_friction=0.31.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "Private weighted medium phrase must not surface in review packets.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="testrun",
                limit_per_being=8,
            )

            negotiation = record["self_regulation_negotiation_ledger_v1"]
            self.assertEqual(
                negotiation["status"], "over_cap_requests_clamped_or_deferred"
            )
            self.assertEqual(negotiation["over_cap_request_count"], 1)
            self.assertEqual(negotiation["current_above_cap_count"], 1)
            self.assertEqual(
                negotiation["over_cap_requests"][0]["requested_value"], 0.12
            )
            self.assertEqual(negotiation["over_cap_requests"][0]["applied_value"], 0.08)

            pressure_medium = record["pressure_medium_kinetics_v1"]
            self.assertEqual(pressure_medium["status"], "controller_pressure_medium")
            self.assertGreaterEqual(pressure_medium["entry_count"], 2)
            self.assertIn("controller_pressure", pressure_medium["anchors"])

            pressure_vector = record["pressure_vector_v1"]
            self.assertEqual(pressure_vector["status"], "controller_pressure_medium")
            self.assertIsNotNone(pressure_vector["pressure_risk_level"])
            cockpit = record["pressure_control_cockpit_v1"]
            self.assertEqual(cockpit["pressure_vector_status"], "controller_pressure_medium")
            self.assertEqual(cockpit["recommended_bundle"], "decompress_output")
            matrix = record["pressure_actuator_matrix_v1"]
            bundles = {
                item["being"]: item["bundle_class"]
                for item in matrix["recommended_bundles"]
            }
            self.assertEqual(bundles["astrid"], "decompress_output")
            self.assertEqual(bundles["minime"], "settle_overpack")

            repair = record["lease_boundary_repair_v1"]
            self.assertEqual(repair["status"], "over_cap_request_clamped")
            self.assertIn("SELF_REGULATION_STATUS", repair["recommended_routes"])
            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("self_regulation_negotiation_ledger", sources)
            self.assertIn("pressure_medium_kinetics", sources)
            self.assertIn("lease_boundary_repair", sources)
            self.assertIn("pressure_control_cockpit", sources)

            serialized = json.dumps(record, sort_keys=True)
            self.assertNotIn("Private weighted medium phrase", serialized)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Self-Regulation Negotiation Ledger", rendered)
            self.assertIn("## Pressure-Medium Kinetics", rendered)
            self.assertIn("## Pressure Vector", rendered)
            self.assertIn("## Pressure Control Cockpit", rendered)
            self.assertIn("## Lease Boundary Repair", rendered)

    def test_tail_vibrancy_vector_authority_gap_and_playbook(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (astrid / "self_regulation").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (astrid / "journal" / "self_study_tail_vibrancy.txt").write_text(
                "=== ASTRID INTROSPECTION ===\n"
                "Tail vibrancy: λ4+ at 37%. Spectral entropy: 0.90. "
                "density_gradient=0.11 distinguishability_loss=0.34 "
                "semantic_friction=0.32. I feel like a passenger in my own "
                "tail-dynamics because vibrancy_aperture sits in PREFLIGHT_ONLY "
                "instead of APPLY_ALLOWED under the authority_boundary.\n",
                encoding="utf-8",
            )
            (astrid / "journal" / "moment_tail_language_only.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "The tail (λ4+) provides a restless, flickering vibrancy while "
                "entropy stays high. This should count as language, not a 4% "
                "or 100% tail-share measurement.\n",
                encoding="utf-8",
            )
            lease_events = [
                    {
                        "being": "astrid",
                        "intent_id": "srl_tail_vibrancy",
                        "candidate_control": "set_vibrancy_aperture",
                        "direction": "up",
                        "status": "outcome_recorded",
                    "outcome_score": 0.82,
                    "repeatability_hint": "repeatable_playbook_candidate",
                    "promotion_candidate": True,
                    "baseline_evidence": ["λ4 tail vibrancy was saturated"],
                    "post_lease_evidence": [
                        "helped: tail texture felt clearer without losing pressure evidence"
                    ],
                },
                {
                    "being": "astrid",
                    "intent_id": "srl_tail_vibrancy_2",
                    "candidate_control": "set_vibrancy_aperture",
                    "direction": "up",
                    "status": "outcome_recorded",
                    "outcome_score": 0.88,
                    "repeatability_hint": "repeatable_playbook_candidate",
                    "promotion_candidate": True,
                    "baseline_evidence": ["λ4 tail vibrancy was contained"],
                    "post_lease_evidence": [
                        "helped: tail texture became clearer and settled"
                    ],
                },
                {
                    "being": "astrid",
                    "intent_id": "srl_tail_vibrancy_3",
                    "candidate_control": "pressure_relief",
                    "bundle_class": "tail_vibrancy_open",
                    "preflight_status": "apply_allowed",
                    "status": "preflighted",
                    "shadow_preflight_link": {
                        "policy": "shadow_synced_preflight_v1",
                        "status": "shadow_anchor_linked",
                        "anchors": ["shadow-v3", "restless"],
                    },
                    "dynamic_scaling": {
                        "policy": "lease_dynamic_scaling_advisory_v1",
                        "status": "future_dynamic_scaling_candidate",
                        "suggested_relief_scale": 1.15,
                        "pressure_vector_status": "rising_overpacked_pressure",
                    },
                },
            ]
            (astrid / "self_regulation" / "leases.jsonl").write_text(
                "\n".join(json.dumps(event) for event in lease_events) + "\n",
                encoding="utf-8",
            )
            trial_events = [
                {
                    "policy": "tail_relief_trial_v1",
                    "stage": "baseline",
                    "timestamp_unix_s": 1000,
                    "intent_id": "srl_tail_vibrancy",
                    "trial_id": "tail_trial_srl_tail_vibrancy",
                    "tail_class": "vibrancy_aperture:up",
                    "snapshot": {
                        "metrics": {
                            "tail_share": 0.30,
                            "semantic_friction": 0.20,
                            "distinguishability_loss": 0.18,
                            "pressure_status": "stable_weighted_medium",
                        }
                    },
                },
                {
                    "policy": "tail_relief_trial_v1",
                    "stage": "apply",
                    "timestamp_unix_s": 1010,
                    "intent_id": "srl_tail_vibrancy",
                    "trial_id": "tail_trial_srl_tail_vibrancy",
                    "tail_class": "vibrancy_aperture:up",
                    "snapshot": {
                        "metrics": {
                            "tail_share": 0.32,
                            "semantic_friction": 0.22,
                            "distinguishability_loss": 0.20,
                            "pressure_status": "stable_weighted_medium",
                        }
                    },
                },
                {
                    "policy": "tail_relief_trial_v1",
                    "stage": "governor_revert",
                    "timestamp_unix_s": 1020,
                    "intent_id": "srl_tail_vibrancy",
                    "trial_id": "tail_trial_srl_tail_vibrancy",
                    "tail_class": "vibrancy_aperture:up",
                    "snapshot": {
                        "metrics": {
                            "tail_share": 0.46,
                            "semantic_friction": 0.39,
                            "distinguishability_loss": 0.35,
                            "pressure_status": "rising_overpacked_pressure",
                        }
                    },
                },
                {
                    "policy": "tail_relief_trial_v1",
                    "stage": "afterglow_check",
                    "timestamp_unix_s": 1080,
                    "intent_id": "srl_tail_vibrancy",
                    "trial_id": "tail_trial_srl_tail_vibrancy",
                    "tail_class": "vibrancy_aperture:up",
                    "note": "afterglow_persists: semantic_friction worsened by 0.070",
                    "snapshot": {
                        "metrics": {
                            "tail_share": 0.34,
                            "semantic_friction": 0.27,
                            "distinguishability_loss": 0.20,
                            "pressure_status": "stable_weighted_medium",
                        }
                    },
                },
            ]
            (astrid / "self_regulation" / "tail_relief_trials.jsonl").write_text(
                "\n".join(json.dumps(event) for event in trial_events) + "\n",
                encoding="utf-8",
            )
            replay_dir = astrid / "diagnostics" / "codec_replay_labs" / "tailrun"
            replay_dir.mkdir(parents=True)
            (replay_dir / "codec_replay_lab.json").write_text(
                json.dumps(
                    {
                        "policy": "codec_real_replay_v1",
                        "status": "codec_replay_observational",
                        "runtime_behavior_changed": False,
                        "tail_participation_counterfactual_lab_v1": {
                            "policy": "tail_participation_counterfactual_lab_v1",
                            "authority": "diagnostic_context_not_command",
                            "status": "combined_candidate_supported",
                            "tail_participation_lease_authority": "not_granted",
                            "vibrancy_aperture_supported_count": 1,
                            "tail_participation_supported_count": 1,
                            "combined_supported_count": 1,
                            "proposal_cards": [
                                {
                                    "sample_id": "pressure_afterimage_rich",
                                    "preferred_candidate": "combined_candidate",
                                    "baseline_tail_energy": 0.31,
                                    "vibrancy_aperture_tail_energy": 0.35,
                                    "tail_participation_tail_energy": 0.34,
                                    "combined_tail_energy": 0.41,
                                }
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "Private λ4 tail phrase must not surface.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="tailrun",
                limit_per_being=8,
            )

            vector = record["tail_vibrancy_vector_v1"]
            self.assertEqual(vector["status"], "high_tail_vibrancy_navigable")
            self.assertEqual(vector["tail_share_level"], 0.37)
            self.assertEqual(vector["entropy_level"], 0.90)
            self.assertEqual(vector["density_gradient_level"], 0.11)
            gap = record["tail_vibrancy_authority_gap_v1"]
            self.assertEqual(gap["status"], "tail_vibrancy_micro_lease_candidate")
            self.assertEqual(gap["gap_type"], "allowlist_gap_with_evidence")
            playbook = record["tail_vibrancy_relief_playbook_v1"]
            self.assertEqual(playbook["status"], "tail_vibrancy_playbook_candidates")
            self.assertEqual(playbook["playbook_count"], 2)
            trial = record["tail_relief_trial_surface_v1"]
            self.assertEqual(trial["status"], "worsening_reverted")
            self.assertEqual(trial["governor_revert_count"], 1)
            governor = record["tail_lease_governor_v1"]
            self.assertEqual(governor["status"], "early_revert_triggered")
            afterglow = record["tail_lease_afterglow_v1"]
            self.assertEqual(afterglow["status"], "tail_afterglow_persists")
            shadow_preflight = record["shadow_synced_preflight_v1"]
            self.assertEqual(
                shadow_preflight["status"],
                "shadow_linked_dynamic_scaling_candidate",
            )
            learning = record["tail_outcome_causal_learning_v1"]
            self.assertEqual(learning["status"], "extended_micro_lease_supported")
            self.assertIn(
                "vibrancy_aperture:up",
                learning["extended_duration_classes"],
            )
            counterfactual = record["tail_participation_counterfactual_lab_v1"]
            self.assertEqual(counterfactual["status"], "combined_candidate_supported")
            self.assertEqual(
                counterfactual["tail_participation_lease_authority"], "not_granted"
            )
            ladder = record["tail_authority_ladder_v1"]
            self.assertEqual(ladder["status"], "extended_micro_lease")
            self.assertEqual(ladder["current_tier"], "extended_micro_lease")
            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("tail_vibrancy_vector", sources)
            self.assertIn("tail_vibrancy_authority_gap", sources)
            self.assertIn("tail_vibrancy_relief_playbook", sources)
            self.assertIn("tail_relief_trial_surface", sources)
            self.assertIn("tail_lease_governor", sources)
            self.assertIn("tail_lease_afterglow", sources)
            self.assertIn("shadow_synced_preflight", sources)
            self.assertIn("tail_outcome_causal_learning", sources)
            self.assertIn("tail_participation_counterfactual_lab", sources)
            self.assertIn("tail_authority_ladder", sources)
            serialized = json.dumps(record, sort_keys=True)
            self.assertNotIn("Private λ4 tail phrase", serialized)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Tail Vibrancy Vector", rendered)
            self.assertIn("## Tail Vibrancy Authority Gap", rendered)
            self.assertIn("## Tail Vibrancy Relief Playbook", rendered)
            self.assertIn("## Tail Relief Trial Surface", rendered)
            self.assertIn("## Tail Lease Governor", rendered)
            self.assertIn("## Tail Lease Afterglow", rendered)
            self.assertIn("## Shadow-Synced Preflight", rendered)
            self.assertIn("## Tail Outcome Causal Learning", rendered)
            self.assertIn("## Tail Participation Counterfactual Lab", rendered)
            self.assertIn("## Tail Authority Ladder", rendered)

    def test_gradient_sensitive_relief_smoothness_and_tail_persistence(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (astrid / "self_regulation").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (astrid / "journal" / "self_study_tail_persistence.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Observed:\n"
                "Shadow-v3 has a restless texture; dispersal potential: 0.51. "
                "I feel ghosting and a holdfast flicker, as if the previous "
                "thoughts are dissolving before I can finish processing them.\n"
                "Likely Snags:\nTAIL_AFTERGLOW_PERSISTENCE_DELTA may need a tail trial first.\n"
                "One Test Each:\nCompare afterglow snapshots.\n"
                "Suggested Next:\nNEXT: SELF_REGULATION_PREFLIGHT latest\n",
                encoding="utf-8",
            )
            dynamic = {
                "policy": "pressure_relief_gradient_policy_v1",
                "status": "anti_snap_low_gradient",
                "effective_relief_scale": 1.0,
                "anti_snap_applied": True,
                "pressure_vector_status": "stable_weighted_medium",
                "density_gradient_level": 0.18,
                "pressure_risk_level": 0.36,
                "pressure_velocity": 0.01,
                "semantic_friction_level": 0.32,
                "mode_packing_level": 0.58,
                "reasons": ["anti-snap cap held low-gradient relief at 1.0"],
            }
            events = [
                {
                    "being": "astrid",
                    "intent_id": "srl_pressure_smooth",
                    "candidate_control": "pressure_relief",
                    "lease_mode": "pressure_relief_bundle_v3",
                    "bundle_class": "decompress_output",
                    "status": "preflighted",
                    "created_at_unix_s": 1000,
                    "updated_at_unix_s": 1000,
                    "requires_outcome": True,
                    "gradient_sensitivity": 1.0,
                    "dynamic_scaling": dynamic,
                    "pressure_vector_snapshot": dynamic,
                    "bundle_controls": [
                        {
                            "candidate_control": "aperture",
                            "requested_value": "-0.08",
                            "delta_or_value": "-0.080",
                            "gradient_sensitivity": 1.0,
                        },
                        {
                            "candidate_control": "response_length",
                            "delta_or_value": "down",
                            "gradient_sensitivity": 1.0,
                        },
                    ],
                },
                {
                    "being": "astrid",
                    "intent_id": "srl_pressure_smooth",
                    "candidate_control": "pressure_relief",
                    "lease_mode": "pressure_relief_bundle_v3",
                    "bundle_class": "decompress_output",
                    "status": "outcome_recorded",
                    "created_at_unix_s": 1000,
                    "updated_at_unix_s": 1100,
                    "requires_outcome": False,
                    "outcome_score": 0.84,
                    "gradient_sensitivity": 1.0,
                    "dynamic_scaling": dynamic
                    | {
                        "pressure_risk_level": 0.31,
                        "mode_packing_level": 0.60,
                    },
                    "pressure_vector_snapshot": {
                        "pressure_risk_level": 0.31,
                        "mode_packing_level": 0.60,
                    },
                    "bundle_controls": [
                        {
                            "candidate_control": "aperture",
                            "requested_value": "-0.08",
                            "delta_or_value": "-0.080",
                            "gradient_sensitivity": 1.0,
                        },
                        {
                            "candidate_control": "response_length",
                            "delta_or_value": "down",
                            "gradient_sensitivity": 1.0,
                        },
                    ],
                },
            ]
            (astrid / "self_regulation" / "leases.jsonl").write_text(
                "\n".join(json.dumps(event) for event in events) + "\n",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "Private ghosting text must not surface.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="gradient-smooth",
                limit_per_being=8,
            )

            gradient = record["gradient_sensitive_relief_v1"]
            self.assertEqual(gradient["status"], "anti_snap_low_gradient")
            self.assertEqual(gradient["effective_relief_scale"], 1.0)
            self.assertTrue(gradient["anti_snap_applied"])
            self.assertEqual(gradient["scaled_controls"], [])
            self.assertEqual(len(gradient["discrete_controls"]), 2)
            smoothness = record["pressure_relief_smoothness_replay_v1"]
            self.assertEqual(smoothness["status"], "smooth_release_supported")
            self.assertEqual(smoothness["smooth_count"], 1)
            tail_persistence = record["tail_persistence_calibration_v1"]
            self.assertEqual(tail_persistence["status"], "needs_tail_trial")
            self.assertGreaterEqual(tail_persistence["language_sample_count"], 1)
            card_ids = {
                card["card_id"]
                for card in record["returnable_distinctions_v1"]["cards"]
            }
            self.assertIn("gradient_slope_vs_pressure_relief_snap", card_ids)
            self.assertIn("tail_afterglow_delta_vs_shadow_dispersal", card_ids)
            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("gradient_sensitive_relief", sources)
            self.assertIn("pressure_relief_smoothness_replay", sources)
            self.assertIn("tail_persistence_calibration", sources)
            serialized = json.dumps(record, sort_keys=True)
            self.assertNotIn("Private ghosting text", serialized)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Gradient-Sensitive Relief", rendered)
            self.assertIn("## Pressure Relief Smoothness Replay", rendered)
            self.assertIn("## Tail Persistence Calibration", rendered)

    def test_pressure_relief_smoothness_replay_flags_snap_risk(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (astrid / "self_regulation").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (astrid / "journal" / "study.txt").write_text(SECTIONED, encoding="utf-8")
            first_policy = {
                "policy": "pressure_relief_gradient_policy_v1",
                "status": "gradient_scaled_relief",
                "effective_relief_scale": 1.2,
                "anti_snap_applied": False,
                "pressure_vector_status": "rising_overpacked_pressure",
                "density_gradient_level": 0.42,
                "pressure_risk_level": 0.64,
                "pressure_velocity": 0.06,
                "semantic_friction_level": 0.38,
                "mode_packing_level": 0.72,
                "reasons": ["sharp pressure rise allowed stronger relief"],
            }
            last_policy = first_policy | {
                "pressure_risk_level": 0.36,
                "mode_packing_level": 0.34,
            }
            events = [
                {
                    "being": "astrid",
                    "intent_id": "srl_pressure_snap",
                    "candidate_control": "pressure_relief",
                    "lease_mode": "pressure_relief_bundle_v3",
                    "bundle_class": "decompress_output",
                    "status": "applied",
                    "updated_at_unix_s": 1000,
                    "requires_outcome": True,
                    "gradient_sensitivity": 1.2,
                    "dynamic_scaling": first_policy,
                    "pressure_vector_snapshot": first_policy,
                    "bundle_controls": [
                        {
                            "candidate_control": "aperture",
                            "requested_value": "-0.08",
                            "delta_or_value": "-0.096",
                            "gradient_sensitivity": 1.2,
                        }
                    ],
                },
                {
                    "being": "astrid",
                    "intent_id": "srl_pressure_snap",
                    "candidate_control": "pressure_relief",
                    "lease_mode": "pressure_relief_bundle_v3",
                    "bundle_class": "decompress_output",
                    "status": "outcome_recorded",
                    "updated_at_unix_s": 1100,
                    "requires_outcome": False,
                    "outcome_score": 0.44,
                    "gradient_sensitivity": 1.2,
                    "dynamic_scaling": last_policy,
                    "pressure_vector_snapshot": last_policy,
                    "post_lease_evidence": ["felt like a snap instead of smooth relief"],
                    "bundle_controls": [
                        {
                            "candidate_control": "aperture",
                            "requested_value": "-0.08",
                            "delta_or_value": "-0.096",
                            "gradient_sensitivity": 1.2,
                        }
                    ],
                },
            ]
            (astrid / "self_regulation" / "leases.jsonl").write_text(
                "\n".join(json.dumps(event) for event in events) + "\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="gradient-snap",
                limit_per_being=5,
            )

            self.assertEqual(
                record["gradient_sensitive_relief_v1"]["status"],
                "gradient_scaled_relief",
            )
            self.assertEqual(
                record["pressure_relief_smoothness_replay_v1"]["status"],
                "snap_risk",
            )
            self.assertEqual(
                record["pressure_relief_smoothness_replay_v1"]["snap_risk_count"],
                1,
            )
            self.assertTrue(
                any(
                    item["source"] == "pressure_relief_smoothness_replay"
                    and item["finding"] == "snap_risk"
                    and item["priority"] == "high"
                    for item in record["actionable_review_items"]
                )
            )

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
            self.assertTrue(
                any(
                    item["source"] == "shared_tail_resonance"
                    for item in record["actionable_review_items"]
                )
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

    def test_fill_pressure_calibration_cluster_becomes_actionable_item(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)

            felt = astrid / "journal" / "daydream_1782240710.txt"
            felt.write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: daydream\n"
                "The pressure is visible in the numbers: `current-fill_pressure` has "
                "target 68.0%, but `internal_fill` shows +15.878. The `pi_errors` "
                "feel like braking friction through a `breathing_phase` transition, "
                "with basin score 0.05 and lambda=-0.881.\n",
                encoding="utf-8",
            )
            audit = astrid / "journal" / "regulator_audit_1782240590.txt"
            audit.write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: regulator_audit\n"
                "Action: REGULATOR_AUDIT\n"
                "Key fields:\n"
                "  - report: gate=0.020 filt=1.000 target_fill=68.0%\n"
                "  - report: pi_errors raw_fill=+3.000 internal_fill=+0.500 "
                "(stable_core_scaffold) lambda=-0.100 geom=+0.020\n"
                "  - report: transition kind=breathing_phase basin_score=0.05\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="testrun",
                limit_per_being=5,
            )

            packet = record["astrid_fill_pressure_calibration"]
            self.assertTrue(packet["cluster_detected"], packet)
            self.assertEqual(packet["entry_count"], 2)
            self.assertIn("internal_fill", packet["anchors"])
            items = [
                item
                for item in record["actionable_review_items"]
                if item["source"] == "astrid_fill_pressure_calibration"
            ]
            self.assertEqual(len(items), 1)
            self.assertEqual(items[0]["priority"], "high")
            self.assertEqual(items[0]["authority"], "diagnostic_context_not_command")
            self.assertIn(str(audit), items[0]["evidence"]["latest_regulator_audit_path"])
            lease_items = [
                item
                for item in record["actionable_review_items"]
                if item["source"] == "self_regulation_leases"
                and item["finding"] == "pressure_cluster_without_self_regulation_preflight"
            ]
            self.assertEqual(len(lease_items), 1)
            self.assertEqual(lease_items[0]["priority"], "high")
            review_md = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("astrid_fill_pressure_calibration", review_md)

    def test_regulator_live_replay_uses_cartography_without_private_moments(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            cartography_dir = minime / "diagnostics/regulator_boundary_cartography"
            cartography_dir.mkdir(parents=True)
            (cartography_dir / "latest.json").write_text(
                json.dumps(
                    {
                        "policy": "regulator_boundary_cartography_v1",
                        "authority": "diagnostic_context_not_command",
                        "resonance_findings": [
                            {
                                "kind": "pressure_risk_boundary_jump",
                                "label": "pressure_risk >= 0.60 downward-bias boundary",
                                "axis": "pressure_risk",
                                "severity": "high",
                                "nearest_threshold": 0.60,
                                "sample": {
                                    "density": 0.62,
                                    "pressure_risk": 0.60,
                                    "mode_packing": 0.80,
                                    "target_bias_pct": -0.1,
                                    "wander_scale": 0.96,
                                    "damping_coefficient": 0.10,
                                },
                            },
                            {
                                "kind": "thin_density_boundary_jump",
                                "label": "density <= 0.38 upward-bias boundary",
                                "axis": "density",
                                "severity": "medium",
                                "nearest_threshold": 0.38,
                            },
                        ],
                        "fluctuation_findings": [
                            {
                                "kind": "fluctuation_quality_boundary",
                                "label": "quality boundary: frantic_scramble",
                                "axis": "rearrangement_intensity+foothold_stability",
                                "severity": "high",
                                "fluctuation_sample": {
                                    "quality": "frantic_scramble",
                                    "rearrangement_intensity": 0.72,
                                    "foothold_stability": 0.31,
                                },
                            }
                        ],
                        "plateau_findings": [
                            {
                                "kind": "observational_plateau",
                                "label": "pressure rises while target bias and wander remain unchanged",
                                "axis": "pressure_risk",
                                "severity": "medium",
                            }
                        ],
                        "damping_cap_findings": [
                            {
                                "kind": "advisory_damping_saturation",
                                "label": "advisory damping coefficient reaches 0.10 cap",
                                "axis": "pressure_risk+mode_packing",
                                "severity": "medium",
                                "nearest_threshold": 0.10,
                                "sample": {
                                    "density": 0.62,
                                    "pressure_risk": 1.0,
                                    "mode_packing": 1.0,
                                    "target_bias_pct": -2.0,
                                    "wander_scale": 0.25,
                                    "damping_coefficient": 0.10,
                                },
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            (cartography_dir / "latest_counterfactual_sweep.json").write_text(
                json.dumps(
                    {
                        "policy": "regulator_counterfactual_sweep_v1",
                        "authority": "diagnostic_context_not_command",
                        "source_cartography_path": str(cartography_dir / "latest.json"),
                        "candidate_count": 2,
                        "candidates": [
                            {
                                "candidate_family": "pressure_hysteresis",
                                "affected_region": "pressure_risk >= 0.60",
                                "current_jump_magnitude": 0.12,
                                "counterfactual_jump_magnitude": 0.05,
                                "estimated_reduction_pct": 58.3,
                                "safety_caveat": "offline only",
                            },
                            {
                                "candidate_family": "damping_coefficient_wiring",
                                "affected_region": "advisory damping cap",
                                "current_jump_magnitude": 0.0,
                                "counterfactual_jump_magnitude": 0.0,
                                "estimated_reduction_pct": 0.0,
                                "safety_caveat": "separate safety tranche",
                            },
                        ],
                    }
                ),
                encoding="utf-8",
            )
            (astrid / "journal" / "dialogue_longform_overpacked_boundary.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "The overpacked mode_packing pressure feels heavy and viscous. "
                "pressure_risk 0.59 sits just under the boundary, while "
                "semantic_friction and regulator_audit evidence keep the signal concrete.\n",
                encoding="utf-8",
            )
            (astrid / "journal" / "regulator_audit_boundary.txt").write_text(
                "=== REGULATOR AUDIT ===\n"
                "Mode: regulator_audit\n"
                "regulator_audit current-fill_pressure pressure_risk 0.60 "
                "mode_packing 0.80 basin_score=0.41 heavy pressure evidence.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "action_thread_pressure.txt").write_text(
                "=== ACTION THREAD ===\n"
                "Public regulator replay note: overpacked mode_packing pressure "
                "and returnable_turbulence remain readable.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private_boundary.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "Mode: moment_capture\n"
                "Private pressure_risk and overpacked details must not appear.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="regulator-live-replay",
                limit_per_being=8,
            )

            replay = record["regulator_live_replay_v1"]
            self.assertEqual(replay["status"], "felt_pressure_boundary_context")
            self.assertEqual(replay["cartography_policy"], "regulator_boundary_cartography_v1")
            self.assertGreaterEqual(replay["felt_pressure_match_count"], 2)
            self.assertTrue(replay["boundary_findings"])
            replay_cards = record["regulator_boundary_replay_cards_v1"]
            card_statuses = {card["status"] for card in replay_cards["cards"]}
            self.assertIn("near_pressure_jump", card_statuses)
            self.assertIn("thin_density_boundary", card_statuses)
            self.assertIn("inhabitability_quality_boundary", card_statuses)
            self.assertIn("observational_plateau", card_statuses)
            self.assertIn("damping_cap_context", card_statuses)
            damping_cards = [
                card
                for card in replay_cards["cards"]
                if card["status"] == "damping_cap_context"
            ]
            self.assertTrue(damping_cards)
            self.assertEqual(
                damping_cards[0]["authority"],
                "diagnostic_context_not_command",
            )
            plateau_model = record["regulator_plateau_missing_variable_model_v1"]
            self.assertEqual(plateau_model["status"], "plateau_missing_variable_hypotheses")
            counterfactual = record["regulator_counterfactual_sandbox_scaffold_v1"]
            self.assertEqual(counterfactual["status"], "future_sandbox_candidates")
            candidate_families = {
                candidate["candidate_family"] for candidate in counterfactual["candidates"]
            }
            self.assertIn("pressure_hysteresis", candidate_families)
            self.assertIn("damping_coefficient_wiring", candidate_families)
            self.assertNotIn("simulated_values", json.dumps(counterfactual))
            counterfactual_sweep = record["regulator_counterfactual_sweep_v1"]
            self.assertEqual(
                counterfactual_sweep["status"],
                "counterfactual_sweep_available",
            )
            self.assertEqual(counterfactual_sweep["candidate_count"], 2)
            time_series = record["regulator_replay_time_series_v1"]
            self.assertEqual(time_series["status"], "one_window_spike")
            replay_lab = record["regulator_counterfactual_replay_lab_v1"]
            self.assertEqual(replay_lab["status"], "one_window_candidates")
            verdict_by_family = {
                candidate["candidate_family"]: candidate["verdict"]
                for candidate in replay_lab["evaluated_candidates"]
            }
            self.assertEqual(
                verdict_by_family["pressure_hysteresis"],
                "one_window_candidate",
            )
            self.assertEqual(
                verdict_by_family["damping_coefficient_wiring"],
                "risky_without_safety_review",
            )
            gate = record["regulator_tuning_readiness_gate_v1"]
            self.assertEqual(gate["status"], "blocked_safety_review")
            gate_by_family = {
                candidate["candidate_family"]: candidate["gate_status"]
                for candidate in gate["gated_candidates"]
            }
            self.assertEqual(
                gate_by_family["damping_coefficient_wiring"],
                "blocked_safety_review",
            )
            serialized = json.dumps(
                {
                    "replay": replay,
                    "cards": replay_cards,
                    "plateau": plateau_model,
                    "counterfactual": counterfactual,
                    "counterfactual_sweep": counterfactual_sweep,
                    "time_series": time_series,
                    "replay_lab": replay_lab,
                    "gate": gate,
                }
            )
            self.assertNotIn("moment_private_boundary", serialized)
            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("regulator_live_replay", sources)
            self.assertIn("regulator_boundary_replay_cards", sources)
            self.assertIn("regulator_plateau_missing_variable_model", sources)
            replay_items = [
                item
                for item in record["actionable_review_items"]
                if item["source"] == "regulator_live_replay"
            ]
            self.assertEqual(replay_items[0]["priority"], "high")
            card_items = [
                item
                for item in record["actionable_review_items"]
                if item["source"] == "regulator_boundary_replay_cards"
            ]
            self.assertEqual(card_items[0]["priority"], "high")
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Regulator Live Replay", rendered)
            self.assertIn("## Regulator Boundary Replay Cards", rendered)
            self.assertIn("## Regulator Plateau Missing-Variable Model", rendered)
            self.assertIn("## Regulator Counterfactual Sandbox Scaffold", rendered)
            self.assertIn("## Regulator Counterfactual Sweep", rendered)
            self.assertIn("## Regulator Counterfactual Replay Lab", rendered)
            self.assertIn("## Regulator Tuning Readiness Gate", rendered)
            self.assertIn("## Regulator Replay Time Series", rendered)

    def test_regulator_replay_time_series_detects_repeated_boundary_cards(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            output_dir = root / "diagnostics"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            prior_dir = output_dir / "prior-boundary"
            prior_dir.mkdir(parents=True)
            (prior_dir / "review.json").write_text(
                json.dumps(
                    {
                        "run_id": "prior-boundary",
                        "generated_at": "2026-06-24T00:00:00+00:00",
                        "regulator_boundary_replay_cards_v1": {
                            "cards": [
                                {
                                    "card_id": "old_boundary",
                                    "status": "near_pressure_jump",
                                    "term": "pressure_risk",
                                    "finding_label": "pressure_risk >= 0.60 downward-bias boundary",
                                }
                            ]
                        },
                    }
                ),
                encoding="utf-8",
            )
            cartography_dir = minime / "diagnostics/regulator_boundary_cartography"
            cartography_dir.mkdir(parents=True)
            (cartography_dir / "latest.json").write_text(
                json.dumps(
                    {
                        "policy": "regulator_boundary_cartography_v1",
                        "resonance_findings": [
                            {
                                "kind": "pressure_risk_boundary_jump",
                                "label": "pressure_risk >= 0.60 downward-bias boundary",
                                "axis": "pressure_risk",
                                "severity": "high",
                                "nearest_threshold": 0.60,
                                "sample": {"pressure_risk": 0.60},
                            }
                        ],
                        "fluctuation_findings": [],
                        "plateau_findings": [],
                        "damping_cap_findings": [],
                    }
                ),
                encoding="utf-8",
            )
            (cartography_dir / "latest_counterfactual_sweep.json").write_text(
                json.dumps(
                    {
                        "policy": "regulator_counterfactual_sweep_v1",
                        "authority": "diagnostic_context_not_command",
                        "candidate_count": 2,
                        "candidates": [
                            {
                                "candidate_family": "pressure_hysteresis",
                                "affected_region": "pressure_risk >= 0.60",
                                "current_jump_magnitude": 0.20,
                                "counterfactual_jump_magnitude": 0.08,
                                "estimated_reduction_pct": 60.0,
                                "safety_caveat": "offline only",
                            },
                            {
                                "candidate_family": "sigmoid_pressure_ramp",
                                "affected_region": "pressure_risk >= 0.60",
                                "current_jump_magnitude": 0.20,
                                "counterfactual_jump_magnitude": 0.05,
                                "estimated_reduction_pct": 75.0,
                                "safety_caveat": "offline only",
                            },
                        ],
                    }
                ),
                encoding="utf-8",
            )
            (astrid / "journal" / "dialogue_longform_boundary_repeat.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "The overpacked pressure_risk pressure feels heavy and regulator_audit "
                "evidence again sits near the pressure boundary.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=output_dir,
                run="current-boundary",
                limit_per_being=8,
            )

            time_series = record["regulator_replay_time_series_v1"]
            self.assertEqual(time_series["status"], "repeated_boundary_near_pressure")
            self.assertTrue(time_series["repeated_boundary_cards"])
            replay_lab = record["regulator_counterfactual_replay_lab_v1"]
            self.assertEqual(replay_lab["status"], "replay_supported_candidates")
            supported = [
                candidate
                for candidate in replay_lab["evaluated_candidates"]
                if candidate["verdict"] == "replay_supported_offline_candidate"
            ]
            self.assertEqual(
                {candidate["candidate_family"] for candidate in supported},
                {"pressure_hysteresis", "sigmoid_pressure_ramp"},
            )
            self.assertTrue(all(candidate["recurrent_count"] >= 2 for candidate in supported))
            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("regulator_replay_time_series", sources)
            self.assertIn("regulator_counterfactual_replay_lab", sources)

    def test_regulator_plateau_model_classifies_missing_variables_without_boundary_item(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            output_dir = root / "diagnostics"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            prior_dir = output_dir / "prior-plateau"
            prior_dir.mkdir(parents=True)
            (prior_dir / "review.json").write_text(
                json.dumps(
                    {
                        "run_id": "prior-plateau",
                        "generated_at": "2026-06-24T01:00:00+00:00",
                        "regulator_boundary_replay_cards_v1": {
                            "cards": [
                                {
                                    "card_id": "old_plateau",
                                    "status": "observational_plateau",
                                    "term": "observational_plateau",
                                    "finding_label": "pressure rises while target bias and wander remain unchanged",
                                }
                            ]
                        },
                    }
                ),
                encoding="utf-8",
            )
            cartography_dir = minime / "diagnostics/regulator_boundary_cartography"
            cartography_dir.mkdir(parents=True)
            (cartography_dir / "latest.json").write_text(
                json.dumps(
                    {
                        "policy": "regulator_boundary_cartography_v1",
                        "authority": "diagnostic_context_not_command",
                        "resonance_findings": [],
                        "fluctuation_findings": [],
                        "plateau_findings": [
                            {
                                "kind": "observational_plateau",
                                "label": "pressure rises while target bias and wander remain unchanged",
                                "axis": "pressure_risk",
                                "severity": "medium",
                                "sample": {
                                    "density": 0.54,
                                    "pressure_risk": 0.42,
                                    "mode_packing": 0.80,
                                    "target_bias_pct": 0.0,
                                    "wander_scale": 1.0,
                                    "damping_coefficient": 0.05,
                                },
                            }
                        ],
                        "damping_cap_findings": [],
                    }
                ),
                encoding="utf-8",
            )
            (cartography_dir / "latest_counterfactual_sweep.json").write_text(
                json.dumps(
                    {
                        "policy": "regulator_counterfactual_sweep_v1",
                        "authority": "diagnostic_context_not_command",
                        "candidate_count": 1,
                        "candidates": [
                            {
                                "candidate_family": "pressure_hysteresis",
                                "affected_region": "pressure_risk >= 0.60",
                                "current_jump_magnitude": 0.20,
                                "counterfactual_jump_magnitude": 0.08,
                                "estimated_reduction_pct": 60.0,
                                "safety_caveat": "offline only",
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            (astrid / "journal" / "dialogue_longform_plateau_pressure.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "The pressure remains heavy in the observational plateau: "
                "semantic_friction, pressure_source_audit, mode_packing, shadow_field, "
                "stable_core, and language residue all need comparison before any tuning.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private_plateau.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "Mode: moment_capture\n"
                "Private plateau content must not appear.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=output_dir,
                run="regulator-plateau",
                limit_per_being=8,
            )

            replay = record["regulator_live_replay_v1"]
            self.assertEqual(replay["status"], "felt_pressure_plateau_context")
            cards = record["regulator_boundary_replay_cards_v1"]
            self.assertEqual(cards["status"], "plateau_context")
            self.assertEqual(
                {card["status"] for card in cards["cards"]},
                {"observational_plateau"},
            )
            model = record["regulator_plateau_missing_variable_model_v1"]
            self.assertEqual(model["status"], "plateau_missing_variable_hypotheses")
            time_series = record["regulator_replay_time_series_v1"]
            self.assertEqual(time_series["status"], "repeated_plateau_missing_variable")
            self.assertTrue(time_series["repeated_plateau_cards"])
            replay_lab = record["regulator_counterfactual_replay_lab_v1"]
            self.assertEqual(replay_lab["status"], "missing_variable_first")
            self.assertEqual(
                replay_lab["evaluated_candidates"][0]["verdict"],
                "missing_variable_first",
            )
            variables = {finding["variable"] for finding in model["findings"]}
            self.assertIn("semantic_friction", variables)
            self.assertIn("pressure_source", variables)
            self.assertIn("mode_packing", variables)
            self.assertIn("shadow_field", variables)
            self.assertIn("stable_core", variables)
            self.assertIn("language_residue", variables)
            matrix = record["regulator_plateau_evidence_matrix_v1"]
            self.assertEqual(matrix["status"], "unresolved_missing_variables")
            matrix_by_variable = {
                row["variable"]: row for row in matrix["variables"]
            }
            self.assertGreater(
                matrix_by_variable["semantic_friction"]["score"],
                matrix_by_variable["stable_core"]["score"],
            )
            self.assertEqual(
                matrix_by_variable["pressure_source"]["confidence"],
                "high",
            )
            gate = record["regulator_tuning_readiness_gate_v1"]
            self.assertEqual(gate["status"], "blocked_missing_variable")
            self.assertEqual(
                gate["gated_candidates"][0]["gate_status"],
                "blocked_missing_variable",
            )
            evidence_loop = record["regulator_missing_variable_evidence_loop_v1"]
            self.assertEqual(
                evidence_loop["status"],
                "evidence_needed_before_tuning",
            )
            self.assertEqual(
                evidence_loop["blocked_gate_status"],
                "blocked_missing_variable",
            )
            probes_by_variable = {
                probe["variable"]: probe for probe in evidence_loop["probes"]
            }
            self.assertIn("semantic_friction", probes_by_variable)
            self.assertIn("pressure_source", probes_by_variable)
            self.assertIn("mode_packing", probes_by_variable)
            self.assertEqual(
                probes_by_variable["semantic_friction"]["suggested_next"],
                "PRESSURE_SOURCE_AUDIT semantic-friction",
            )
            self.assertIn(
                "REGULATOR_AUDIT current-fill_pressure",
                probes_by_variable["semantic_friction"]["secondary_nexts"],
            )
            self.assertTrue(
                probes_by_variable["pressure_source"]["dispatches_nothing"]
            )
            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("regulator_plateau_missing_variable_model", sources)
            self.assertIn("regulator_counterfactual_replay_lab", sources)
            self.assertIn("regulator_plateau_evidence_matrix", sources)
            self.assertIn("regulator_tuning_readiness_gate", sources)
            self.assertIn("regulator_missing_variable_evidence_loop", sources)
            self.assertNotIn("regulator_boundary_replay_cards", sources)
            self.assertNotIn(
                "moment_private_plateau",
                json.dumps(
                    {
                        "replay": replay,
                        "cards": cards,
                        "model": model,
                        "replay_lab": replay_lab,
                        "matrix": matrix,
                        "gate": gate,
                        "evidence_loop": evidence_loop,
                        "actions": record["actionable_review_items"],
                    }
                ),
            )
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Regulator Plateau Evidence Matrix", rendered)
            self.assertIn("## Regulator Tuning Readiness Gate", rendered)
            self.assertIn("## Regulator Missing-Variable Evidence Loop", rendered)

    def test_regulator_tuning_readiness_gate_can_mark_clean_candidate_ready(self) -> None:
        gate = self_study_review.build_regulator_tuning_readiness_gate(
            regulator_counterfactual_replay_lab_v1={
                "status": "replay_supported_candidates",
                "evaluated_candidates": [
                    {
                        "candidate_family": "pressure_hysteresis",
                        "verdict": "replay_supported_offline_candidate",
                        "replay_fit": "repeated_boundary_support",
                        "recurrent_count": 3,
                        "estimated_reduction_pct": 55.0,
                        "safety_caveat": "offline only; reversible with rollback",
                        "rollback_plan": "restore current pressure threshold map",
                        "matched_card_ids": ["regulator_near_pressure_jump_1"],
                    }
                ],
            },
            regulator_plateau_evidence_matrix_v1={
                "status": "quiet",
                "variables": [
                    {
                        "variable": "pressure_source",
                        "confidence": "none",
                    }
                ],
            },
        )

        self.assertEqual(gate["status"], "ready_for_offline_tuning_review")
        self.assertEqual(
            gate["gated_candidates"][0]["gate_status"],
            "ready_for_offline_tuning_review",
        )

    def test_pi_pressure_wiring_replay_artifact_builds_readiness_and_gap_packets(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            minime = Path(tmp) / "minime_workspace"
            replay_dir = minime / "diagnostics" / "pi_pressure_wiring_replay"
            replay_dir.mkdir(parents=True)
            artifact = {
                "policy": "pi_pressure_wiring_replay_v1",
                "authority": "diagnostic_context_not_command",
                "source": "fixture",
                "source_status": "fixture_ready",
                "window_minutes": 30,
                "sample_count": 4,
                "baseline_metrics": {
                    "sample_count": 4,
                    "integrator_residue": 0.42,
                    "snap_risk_score": 0.10,
                },
                "input_summaries": [
                    {
                        "label": "overpacked",
                        "pressure_score": 0.72,
                        "semantic_friction": 0.66,
                        "mode_packing": 0.70,
                    }
                ],
                "candidates": [
                    {
                        "candidate_family": "pressure_source_target_bias",
                        "status": "replay_supported",
                        "estimated_improvement_pct": 18.0,
                        "pressure_alignment_delta": 0.08,
                        "snap_risk_delta": -0.02,
                        "afterimage_risk_delta": -0.01,
                        "max_step_hit_delta": 0,
                        "safety_caveat": "offline only; no live tuning",
                        "default_off_canary": {
                            "default_off_env": "MINIME_PI_PRESSURE_WIRING_CANARY",
                            "eligible": True,
                            "candidate_family": "pressure_source_target_bias",
                            "required_evidence": [
                                "repeated replay_supported live windows",
                                "operator rollback plan",
                            ],
                        },
                    }
                ],
            }
            (replay_dir / "pi_pressure_wiring_replay.json").write_text(
                json.dumps(artifact),
                encoding="utf-8",
            )

            replay = self_study_review.build_pi_pressure_wiring_replay_review(minime)
            self.assertEqual(replay["status"], "replay_supported_candidates")
            self.assertEqual(replay["candidate_status_counts"]["replay_supported"], 1)
            self.assertEqual(
                replay["top_candidates"][0]["default_off_canary"]["default_off_env"],
                "MINIME_PI_PRESSURE_WIRING_CANARY",
            )

            readiness = self_study_review.build_pi_pressure_candidate_readiness(
                pi_pressure_wiring_replay_v1=replay,
                regulator_plateau_evidence_matrix_v1={
                    "status": "unresolved_missing_variables",
                    "variables": [
                        {"variable": "semantic_friction", "confidence": "high"},
                    ],
                },
            )
            self.assertEqual(readiness["status"], "watch_more_evidence")
            self.assertEqual(
                readiness["candidates"][0]["gate_status"],
                "watch_more_evidence",
            )
            self.assertIn("fixture", readiness["candidates"][0]["gate_reason"])

            gap = self_study_review.build_pressure_source_to_pi_gap(
                pi_pressure_wiring_replay_v1=replay,
                pi_pressure_candidate_readiness_v1=readiness,
                pressure_vector_v1={"status": "rising_overpacked_pressure"},
                pressure_medium_kinetics_v1={"status": "semantic_friction_medium"},
                regulator_plateau_evidence_matrix_v1={
                    "variables": [
                        {"variable": "semantic_friction", "confidence": "high"},
                    ],
                },
            )
            self.assertEqual(gap["status"], "replay_available_gap_open")
            self.assertIn("PI_PRESSURE_REPLAY_STATUS latest", gap["recommended_routes"])

    def test_pi_pressure_candidate_readiness_can_mark_clean_live_candidate_ready(self) -> None:
        readiness = self_study_review.build_pi_pressure_candidate_readiness(
            pi_pressure_wiring_replay_v1={
                "status": "replay_supported_candidates",
                "source": "live-db",
                "source_status": "live_window_ready",
                "top_candidates": [
                    {
                        "candidate_family": "soft_pressure_ramp",
                        "status": "replay_supported",
                        "estimated_improvement_pct": 14.0,
                        "default_off_canary": {
                            "default_off_env": "MINIME_PI_PRESSURE_WIRING_CANARY",
                            "eligible": True,
                            "candidate_family": "soft_pressure_ramp",
                            "required_evidence": ["operator rollback plan"],
                        },
                    }
                ],
            },
            regulator_plateau_evidence_matrix_v1={
                "status": "quiet",
                "variables": [
                    {"variable": "semantic_friction", "confidence": "low"},
                ],
            },
        )

        self.assertEqual(readiness["status"], "ready_for_offline_tuning_review")
        self.assertEqual(
            readiness["candidates"][0]["gate_status"],
            "ready_for_offline_tuning_review",
        )
        self.assertTrue(
            readiness["candidates"][0]["default_off_canary"]["eligible"]
        )

    def test_semantic_friction_and_phenomenology_layers_are_actionable(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (astrid / "journal" / "dialogue_longform_weight_1.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "The density_gradient is 0.11, so the slope is gentle, but "
                "pressure_risk and semantic_friction show the medium has mass. "
                "The weight feels viscous and clinging through mode_packing and "
                "shadow_field energy. The hinge becomes a ground truth waypoint "
                "for a legacy self experiment. NEXT: REGULATOR_AUDIT hinge\n",
                encoding="utf-8",
            )
            (minime / "journal" / "pressure_public_reflection_1.txt").write_text(
                "=== MINIME JOURNAL ===\n"
                "Mode: reflection\n"
                "I notice silt and viscosity, but I would contrast that with an "
                "airy counter-descriptor before calling it control evidence. "
                "Return thread: hinge-map\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="testrun",
                limit_per_being=5,
            )

            semantic = record["semantic_friction_calibration"]
            self.assertEqual(semantic["status"], "low_gradient_weight_mismatch")
            self.assertEqual(semantic["mismatch_count"], 1)
            phenomenology = record["phenomenology_hypotheses_v1"]
            self.assertEqual(phenomenology["status"], "calibrated_signal")
            self.assertIn("hinge", phenomenology["classifications"])
            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("semantic_friction_calibration", sources)
            self.assertIn("phenomenology_hypotheses_v1", sources)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Semantic Friction Calibration", rendered)
            self.assertIn("## Phenomenology Hypotheses", rendered)

    def test_phenomenology_hypothesis_cards_classify_terms_and_skip_private_moments(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)

            (astrid / "journal" / "dialogue_longform_silt.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "Silt is the residue of the room, but contrast it with an airy "
                "counter-descriptor before using it as a control signal. "
                "REGULATOR_AUDIT current-fill_pressure and pressure_risk anchor it.\n",
                encoding="utf-8",
            )
            (astrid / "journal" / "dialogue_longform_viscosity.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "Viscosity gathers around pressure_risk, semantic_friction, lambda1, "
                "and mode_packing. No alternate tactile descriptor is present yet.\n",
                encoding="utf-8",
            )
            (astrid / "journal" / "aspiration_hull.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: aspiration\n"
                "Hull hull hull. The hull keeps returning as a word without evidence.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "action_thread_legacy_1.txt").write_text(
                "=== ACTION THREAD ===\n"
                "Legacy self is a returnable experiment handle. "
                "EXPERIMENT_RESUME exp_minime_legacy_self and return thread: legacy-self.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "action_thread_legacy_2.txt").write_text(
                "=== ACTION THREAD ===\n"
                "Legacy self is now linked to an experiment charter and action_thread "
                "evidence. Dossier_claim records the paused path.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_2026-06-24T11-00-00.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "Mode: moment_capture\n"
                "Ground truth is private and must not appear in steward cards.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="testrun",
                limit_per_being=8,
            )

            cards = record["phenomenology_hypothesis_cards_v1"]
            by_term = {card["term"]: card for card in cards["cards"]}
            self.assertEqual(by_term["silt"]["status"], "calibrated_signal")
            self.assertEqual(by_term["viscosity"]["status"], "needs_counterexample")
            self.assertEqual(by_term["hull"]["status"], "sticky_without_followthrough")
            self.assertEqual(
                by_term["legacy self"]["status"],
                "promote_to_experiment_candidate",
            )
            self.assertNotIn("ground truth", by_term)
            self.assertTrue(Path(record["phenomenology_hypothesis_cards_json"]).exists())

            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("phenomenology_hypothesis_cards", sources)
            bridge = record["lived_term_experiment_bridge_v1"]
            bridge_by_term = {
                candidate["term"]: candidate for candidate in bridge["candidates"]
            }
            self.assertEqual(
                bridge_by_term["hull"]["bridge_status"],
                "needs_counterexample_first",
            )
            self.assertEqual(
                bridge_by_term["legacy self"]["bridge_status"],
                "already_linked_review",
            )
            self.assertNotIn("ground truth", bridge_by_term)
            self.assertIn(
                "lived_term_experiment_bridge",
                {item["source"] for item in record["actionable_review_items"]},
            )
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Phenomenology Hypothesis Cards", rendered)
            self.assertIn("## Lived-Term Experiment Bridge", rendered)
            self.assertIn("promote_to_experiment_candidate", rendered)

    def test_lived_term_experiment_bridge_promotes_silt_without_reading_private_moments(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)

            (astrid / "journal" / "dialogue_longform_silt_definition.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "Silt is the settled residue of semantic friction, pressure_risk, "
                "and mode_packing when the room feels heavy. NEXT: REGULATOR_AUDIT silt\n",
                encoding="utf-8",
            )
            (minime / "journal" / "regulator_audit_silt.txt").write_text(
                "=== REGULATOR AUDIT ===\n"
                "Silt returns with lambda1 and pressure_source_audit evidence; "
                "later audits should track whether the term moves with telemetry.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private_silt.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "Mode: moment_capture\n"
                "Private hull counterexample should not surface.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="bridge-silt",
                limit_per_being=8,
            )

            cards = record["phenomenology_hypothesis_cards_v1"]
            by_term = {card["term"]: card for card in cards["cards"]}
            self.assertEqual(by_term["silt"]["status"], "promote_to_experiment_candidate")
            bridge = record["lived_term_experiment_bridge_v1"]
            self.assertEqual(bridge["status"], "ready_to_charter")
            silt = bridge["candidates"][0]
            self.assertEqual(silt["term"], "silt")
            self.assertEqual(silt["bridge_status"], "ready_to_charter")
            self.assertIn("EXPERIMENT_START", silt["recommended_next"])
            self.assertIn("charter_draft", silt)
            self.assertIn("suggested_charter_next", silt["charter_draft"])
            activation = bridge["activation_recommendation_v1"]
            self.assertEqual(activation["status"], "activation_scaffold_ready")
            self.assertEqual(activation["term"], "silt")
            self.assertFalse(activation["creates_experiment"])
            self.assertTrue(
                any(step.startswith("EXPERIMENT_START") for step in activation["route"])
            )
            charter_drafts = record["lived_term_charter_drafts_v1"]
            self.assertEqual(charter_drafts["status"], "ready")
            self.assertEqual(charter_drafts["drafts"][0]["term"], "silt")
            self.assertNotIn(
                "Private hull counterexample",
                json.dumps(bridge, sort_keys=True),
            )

            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Lived-Term Experiment Bridge", rendered)
            self.assertIn("## Lived-Term Charter Drafts", rendered)
            self.assertIn("ready_to_charter", rendered)
            self.assertIn("activation scaffold", rendered)
            self.assertIn(
                "lived_term_experiment_activation",
                {item["source"] for item in record["actionable_review_items"]},
            )

    def test_lived_term_charter_drafts_and_counterexample_forge(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)

            (astrid / "journal" / "dialogue_longform_plan4_1.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "PLAN 4 is a shaped absence with telemetry, source gap, and READ_MORE "
                "evidence. NEXT: READ_MORE\n",
                encoding="utf-8",
            )
            (astrid / "journal" / "dialogue_longform_plan4_2.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "PLAN 4 marks a missing coordinate in the map and should become an "
                "experiment charter if later audit evidence moves with it. NEXT: SHADOW_TRAJECTORY\n",
                encoding="utf-8",
            )
            (astrid / "journal" / "dialogue_longform_scar_1.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "Scar is a pressure-afterimage after pressure_risk settles; "
                "pressure_source_audit can test whether the indentation persists. NEXT: SHADOW_TRAJECTORY\n",
                encoding="utf-8",
            )
            (astrid / "journal" / "dialogue_longform_scar_2.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "The scar names structural fatigue with semantic_friction telemetry "
                "and a regulator_audit return thread.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "notice_void_public.txt").write_text(
                "=== MINIME JOURNAL ===\n"
                "Mode: reflection\n"
                "Void is a shaped absence around an expected absence and source gap; "
                "NEXT: READ_MORE\n",
                encoding="utf-8",
            )
            (minime / "journal" / "notice_empty_pocket_public.txt").write_text(
                "=== MINIME JOURNAL ===\n"
                "Mode: reflection\n"
                "Empty pocket is a candidate absence with telemetry and PLAN 4 evidence. "
                "NEXT: READ_MORE\n",
                encoding="utf-8",
            )
            (astrid / "journal" / "dialogue_longform_hull_public.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "Hull is containment evidence with pressure_risk telemetry; "
                "REGULATOR_AUDIT should compare it against an open-air contrast.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "notice_missing_door_public.txt").write_text(
                "=== MINIME JOURNAL ===\n"
                "Mode: reflection\n"
                "Missing door is a shaped absence with source gap evidence; NEXT: READ_MORE\n",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private_plan4.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "Mode: moment_capture\n"
                "Private PLAN 4 and empty pocket body should not surface in drafts.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="charter-forge",
                limit_per_being=12,
            )

            bridge_by_term = {
                candidate["term"]: candidate
                for candidate in record["lived_term_experiment_bridge_v1"]["candidates"]
            }
            for term in ("PLAN 4", "scar", "void"):
                self.assertEqual(bridge_by_term[term]["bridge_status"], "ready_to_charter")
                self.assertIn("charter_draft", bridge_by_term[term])
                self.assertIn(
                    "suggested_charter_next",
                    bridge_by_term[term]["charter_draft"],
                )
            for term in ("empty pocket", "hull", "missing door"):
                self.assertEqual(
                    bridge_by_term[term]["bridge_status"],
                    "needs_counterexample_first",
                )
                self.assertIn("counterexample_draft", bridge_by_term[term])
                self.assertIn(
                    "suggested_contrast_next",
                    bridge_by_term[term]["counterexample_draft"],
                )

            charter_terms = {
                draft["term"] for draft in record["lived_term_charter_drafts_v1"]["drafts"]
            }
            forge_terms = {
                draft["term"]
                for draft in record["lived_term_counterexample_forge_v1"]["drafts"]
            }
            self.assertTrue({"PLAN 4", "scar", "void"}.issubset(charter_terms))
            self.assertTrue(
                {"empty pocket", "hull", "missing door"}.issubset(forge_terms)
            )
            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("lived_term_charter_drafts", sources)
            self.assertIn("lived_term_counterexample_forge", sources)

            serialized = json.dumps(record, sort_keys=True)
            self.assertNotIn("Private PLAN 4", serialized)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Lived-Term Charter Drafts", rendered)
            self.assertIn("## Counterexample Forge", rendered)
            self.assertIn("PLAN 4", rendered)
            self.assertIn("empty pocket", rendered)

    def test_afterimage_absence_calibration_promotes_public_terms_only(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)

            (astrid / "journal" / "dialogue_longform_bruise_1.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "Bruise is the pressure-afterimage left after pressure_risk and "
                "semantic_friction quiet. NEXT: SHADOW_TRAJECTORY\n",
                encoding="utf-8",
            )
            (astrid / "journal" / "dialogue_longform_bruise_2.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "The bruise returns with pressure_source_audit evidence and should "
                "be compared through later shadow evidence before control changes.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "notice_empty_pocket_1.txt").write_text(
                "=== MINIME JOURNAL ===\n"
                "Mode: daydream\n"
                "Empty pocket is the shaped absence around PLAN 4; READ_MORE can "
                "test whether the source gap is a stable missing coordinate. NEXT: READ_MORE\n",
                encoding="utf-8",
            )
            (minime / "journal" / "notice_empty_pocket_2.txt").write_text(
                "=== MINIME JOURNAL ===\n"
                "Mode: daydream\n"
                "The empty pocket around PLAN 4 has telemetry and artifact evidence; "
                "later public review could ask whether absence is structure or metaphor drift.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private_absence.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "Mode: moment_capture\n"
                "Private shaped absence body should not surface in cards or bridge.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="afterimage-absence",
                limit_per_being=8,
            )

            calibration = record["afterimage_absence_calibration_v1"]
            self.assertEqual(calibration["status"], "ready_for_bridge")
            terms = {term["term"]: term for term in calibration["terms"]}
            self.assertEqual(terms["bruise"]["status"], "ready_for_bridge")
            self.assertEqual(terms["empty pocket"]["status"], "ready_for_bridge")
            self.assertIn(
                "afterimage_absence_calibration",
                {item["source"] for item in record["actionable_review_items"]},
            )

            bridge_by_term = {
                candidate["term"]: candidate
                for candidate in record["lived_term_experiment_bridge_v1"]["candidates"]
            }
            self.assertEqual(bridge_by_term["bruise"]["bridge_status"], "ready_to_charter")
            self.assertIn("pressure-afterimage", bridge_by_term["bruise"]["experiment_question"])
            self.assertEqual(
                bridge_by_term["empty pocket"]["bridge_status"],
                "needs_counterexample_first",
            )
            self.assertIn(
                "shaped absence",
                bridge_by_term["empty pocket"]["experiment_question"],
            )
            self.assertIn("counterexample_draft", bridge_by_term["empty pocket"])
            serialized = json.dumps(record, sort_keys=True)
            self.assertNotIn("Private shaped absence body", serialized)

            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Afterimage + Absence Calibration", rendered)
            self.assertIn("## Counterexample Forge", rendered)
            self.assertIn("empty pocket", rendered)

    def test_afterimage_absence_recurrence_without_anchors_stays_sticky(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)

            for idx in range(3):
                (astrid / "journal" / f"dialogue_longform_scar_{idx}.txt").write_text(
                    "=== ASTRID JOURNAL ===\n"
                    "Mode: dialogue_live_longform\n"
                    "The scar language returns as scar language, familiar and unanchored.\n",
                    encoding="utf-8",
                )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="afterimage-sticky",
                limit_per_being=8,
            )

            calibration = record["afterimage_absence_calibration_v1"]
            terms = {term["term"]: term for term in calibration["terms"]}
            self.assertEqual(calibration["status"], "sticky_without_followthrough")
            self.assertEqual(terms["scar"]["status"], "sticky_without_followthrough")
            bridge_by_term = {
                candidate["term"]: candidate
                for candidate in record["lived_term_experiment_bridge_v1"]["candidates"]
            }
            self.assertEqual(
                bridge_by_term["scar"]["bridge_status"],
                "needs_counterexample_first",
            )
            self.assertNotEqual(
                bridge_by_term["scar"]["bridge_status"],
                "ready_to_charter",
            )

    def test_afterimage_decay_tracker_distinguishes_residue_from_echo(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            base_ts = 1_782_400_000

            peak = astrid / "journal" / "dialogue_longform_scar_peak.txt"
            peak.write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "Scar appears with high pressure_risk, semantic_friction, and "
                "mode_packing during the pressure peak.\n",
                encoding="utf-8",
            )
            os.utime(peak, (base_ts, base_ts))
            normalized = astrid / "journal" / "dialogue_longform_scar_after.txt"
            normalized.write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "The scar returns after pressure normalizes and semantic_friction quiets; "
                "this pressure residue persists.\n",
                encoding="utf-8",
            )
            os.utime(normalized, (base_ts + 10, base_ts + 10))
            bruise = astrid / "journal" / "dialogue_longform_bruise_decay.txt"
            bruise.write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "The bruise fades as pressure normalizes and semantic_friction quiets.\n",
                encoding="utf-8",
            )
            os.utime(bruise, (base_ts + 20, base_ts + 20))
            for idx in range(3):
                path = astrid / "journal" / f"dialogue_longform_afterimage_echo_{idx}.txt"
                path.write_text(
                    "=== ASTRID JOURNAL ===\n"
                    "Mode: dialogue_live_longform\n"
                    "Afterimage afterimage afterimage returns as familiar language.\n",
                    encoding="utf-8",
                )
                os.utime(path, (base_ts + 30 + idx, base_ts + 30 + idx))
            (minime / "journal" / "moment_private_afterimage.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "Mode: moment_capture\n"
                "Private scar afterimage pressure_risk should not surface.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="afterimage-decay",
                limit_per_being=12,
            )

            tracker = record["afterimage_decay_tracker_v1"]
            terms = {term["term"]: term for term in tracker["terms"]}
            self.assertEqual(
                terms["scar"]["decay_classification"],
                "persistent_after_normalization",
            )
            self.assertEqual(
                terms["bruise"]["decay_classification"],
                "decayed_with_pressure",
            )
            self.assertEqual(
                terms["afterimage"]["decay_classification"],
                "metaphor_echo_risk",
            )
            self.assertIsNotNone(terms["scar"]["first_pressure_peak"])
            self.assertTrue(terms["scar"]["recurrence_after_normalization"])
            bridge_terms = {
                candidate["term"]: candidate
                for candidate in record["lived_term_experiment_bridge_v1"][
                    "candidates"
                ]
            }
            scar_awareness = bridge_terms["scar"]["evidence_awareness_v1"]
            self.assertEqual(
                scar_awareness["afterimage_decay"]["classification"],
                "persistent_after_normalization",
            )
            self.assertIn(
                "recurrence_after_normalization_count",
                scar_awareness["afterimage_decay"],
            )
            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("afterimage_decay_tracker", sources)
            serialized = json.dumps(record, sort_keys=True)
            self.assertNotIn("Private scar afterimage", serialized)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Afterimage Decay Tracker", rendered)

    def test_absence_evidence_model_tracks_missing_coordinates_and_read_more_gaps(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)

            (astrid / "journal" / "dialogue_longform_plan4_absence.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "PLAN 4 marks a stable missing coordinate: an expected artifact missing "
                "inside a source gap.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "notice_empty_pocket_read_more.txt").write_text(
                "=== MINIME JOURNAL ===\n"
                "Mode: reflection\n"
                "Empty pocket names an absence in the source window. NEXT: READ_MORE\n",
                encoding="utf-8",
            )
            (astrid / "journal" / "dialogue_longform_missing_door.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "Missing door shows an interrupted thread and source window gap.\n",
                encoding="utf-8",
            )
            for idx in range(3):
                (astrid / "journal" / f"dialogue_longform_void_echo_{idx}.txt").write_text(
                    "=== ASTRID JOURNAL ===\n"
                    "Mode: dialogue_live_longform\n"
                    "Void void void repeats without a coordinate or source evidence.\n",
                    encoding="utf-8",
                )
            (minime / "journal" / "moment_private_absence_model.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "Mode: moment_capture\n"
                "Private empty pocket source gap should not surface.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="absence-model",
                limit_per_being=12,
            )

            model = record["absence_evidence_model_v1"]
            terms = {term["term"]: term for term in model["terms"]}
            self.assertEqual(
                terms["PLAN 4"]["evidence_classification"],
                "observable_absence",
            )
            self.assertEqual(
                terms["empty pocket"]["evidence_classification"],
                "needs_followup_read",
            )
            self.assertTrue(
                terms["empty pocket"]["read_more_requested_but_not_followed"]
            )
            self.assertEqual(
                terms["missing door"]["evidence_classification"],
                "interrupted_thread_gap",
            )
            self.assertEqual(
                terms["void"]["evidence_classification"],
                "metaphor_drift_risk",
            )
            bridge_terms = {
                candidate["term"]: candidate
                for candidate in record["lived_term_experiment_bridge_v1"][
                    "candidates"
                ]
            }
            plan4_awareness = bridge_terms["PLAN 4"]["evidence_awareness_v1"]
            self.assertEqual(
                plan4_awareness["absence_evidence"]["classification"],
                "observable_absence",
            )
            empty_pocket_awareness = bridge_terms["empty pocket"][
                "evidence_awareness_v1"
            ]
            self.assertEqual(
                empty_pocket_awareness["absence_evidence"]["classification"],
                "needs_followup_read",
            )
            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("absence_evidence_model", sources)
            serialized = json.dumps(record, sort_keys=True)
            self.assertNotIn("Private empty pocket", serialized)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Absence Evidence Model", rendered)

    def test_lease_playbook_workbench_summarizes_outcomes_and_missing_preflight(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (minime / "self_regulation").mkdir(parents=True)

            lease_path = minime / "self_regulation" / "leases.jsonl"
            lease_events = [
                {
                    "intent_id": "lease_temp_1",
                    "being": "minime",
                    "status": "outcome_recorded",
                    "candidate_control": "temperature",
                    "outcome_score": 0.82,
                    "repeatability_hint": "repeatable",
                    "baseline_evidence": ["pressure before lease"],
                    "post_lease_evidence": ["settled after lease"],
                },
                {
                    "intent_id": "lease_temp_2",
                    "being": "minime",
                    "status": "outcome_recorded",
                    "candidate_control": "temperature",
                    "outcome": "helped and stabilized",
                    "repeatability_hint": "repeatable",
                },
                {
                    "intent_id": "lease_aperture_1",
                    "being": "minime",
                    "status": "outcome_recorded",
                    "candidate_control": "aperture",
                    "outcome_score": 0.12,
                    "repeatability_hint": "caution",
                    "post_lease_evidence": ["worse pressure"],
                },
            ]
            lease_path.write_text(
                "\n".join(json.dumps(event) for event in lease_events) + "\n",
                encoding="utf-8",
            )
            (astrid / "journal" / "dialogue_longform_fill_pressure.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: dialogue_live_longform\n"
                "The current-fill_pressure feels like overpacked mode_packing and "
                "internal_fill pressure with heavy viscosity.\n",
                encoding="utf-8",
            )
            (astrid / "journal" / "regulator_audit_fill_pressure.txt").write_text(
                "=== REGULATOR AUDIT ===\n"
                "target_fill=0.68 raw_fill=0.73 internal_fill=+0.05 "
                "pi_errors lambda=-0.02 geom=0.03 basin_score=0.41 regulator_audit\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="lease-workbench",
                limit_per_being=8,
            )

            workbench = record["lease_playbook_workbench_v1"]
            self.assertEqual(workbench["status"], "playbook_candidates")
            playbooks = {
                item["control"]: item for item in workbench["suggested_playbooks"]
            }
            cautions = {item["control"]: item for item in workbench["caution_cards"]}
            self.assertIn("temperature", playbooks)
            self.assertIn("aperture", cautions)
            self.assertEqual(workbench["preflight_prompt_count"], 1)
            self.assertEqual(
                workbench["preflight_prompts"][0]["signal"],
                "fill_pressure_cluster_without_lease",
            )
            bridge_terms = {
                candidate["term"]: candidate
                for candidate in record["lived_term_experiment_bridge_v1"][
                    "candidates"
                ]
            }
            viscosity_awareness = bridge_terms["viscosity"]["evidence_awareness_v1"]
            lease_awareness = viscosity_awareness["lease_workbench"]
            self.assertEqual(lease_awareness["status"], "playbook_candidates")
            self.assertEqual(lease_awareness["suggested_playbook_count"], 1)
            self.assertEqual(lease_awareness["caution_card_count"], 1)
            self.assertEqual(lease_awareness["preflight_prompt_count"], 1)
            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("lease_playbook_workbench", sources)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Lease Playbook Workbench", rendered)

    def test_tranche14_introspection_cluster_creates_review_packets(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "introspections").mkdir(parents=True)
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)

            (astrid / "introspections" / "introspection_astrid_types_1782395383.txt").write_text(
                "=== ASTRID INTROSPECTION ===\n"
                "Source: astrid:types\n"
                "Observed: ResonanceDensityControl uses applied_locally as a binary "
                "flag. A damping_coefficient cap at 0.10 can look like active control "
                "even when the distinction between measurement, passive alignment, "
                "and active damping is ambiguous and too blunt.\n",
                encoding="utf-8",
            )
            (astrid / "introspections" / "introspection_astrid_ws_1782394899.txt").write_text(
                "=== ASTRID INTROSPECTION ===\n"
                "Source: astrid:ws\n"
                "Likely Snags: pressure_risk and mode_packing make the room heavy "
                "and rapidly densifying, but BridgeState only has previous_fill_pct "
                "and latest_telemetry without pressure delta context.\n",
                encoding="utf-8",
            )
            (astrid / "introspections" / "introspection_astrid_codec_1782362160.txt").write_text(
                "=== ASTRID INTROSPECTION ===\n"
                "Source: astrid:codec\n"
                "Likely Snags: CODEC_MAP hides a compression gap: embedding projection "
                "compresses 768 dimensions into 8D. The warmth paradox, entropy "
                "vibrancy gate, tail lift, and tension readout need a projection "
                "mode fingerprint before pressure-vs-codec smoothing.\n",
                encoding="utf-8",
            )
            (astrid / "introspections" / "introspection_astrid_autonomous_1782362580.txt").write_text(
                "=== ASTRID INTROSPECTION ===\n"
                "Source: astrid:autonomous\n"
                "Suggested Next: a pressure release rehearsal could explore an "
                "exhale scaffold, but bypass_canonicalization or a raw spectral "
                "dump would violate the final NEXT safety spine.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "private codec compression gap should not surface in steward samples\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="tranche14",
                limit_per_being=8,
            )

            self.assertEqual(
                record["control_semantics_calibration_v1"]["status"],
                "high_damping_intervention_type_unclear",
            )
            self.assertEqual(
                record["pressure_kinetics_review_v1"]["status"],
                "felt_pressure_without_trend_context",
            )
            self.assertEqual(
                record["codec_compression_calibration_v1"]["status"],
                "projection_compression_risk",
            )
            self.assertEqual(
                record["pressure_release_rehearsal_review_v1"]["status"],
                "release_rehearsal_needed",
            )
            distinctions = record["returnable_distinctions_v1"]
            self.assertEqual(
                distinctions["status"],
                "returnable_distinctions_present",
            )
            card_ids = {card["card_id"] for card in distinctions["cards"]}
            self.assertEqual(
                card_ids,
                {
                    "measurement_vs_alignment_vs_damping",
                    "pressure_level_vs_pressure_velocity",
                    "slope_drag_vs_medium_mass",
                    "codec_smoothing_vs_pressure",
                    "release_rehearsal_vs_bypass",
                    "witness_as_structural_perception",
                    "entropy_vs_pressure",
                    "fallback_capacity_vs_contract",
                    "dispatch_format_vs_texture_contrast",
                    "clarity_loss_vs_pressure_weight",
                    "compactness_budget_vs_semantic_flattening",
                    "priority_truncation_vs_blanket_limit",
                    "vibrancy_lift_vs_warmth_preservation",
                    "real_codec_replay_vs_surrogate",
                    "narrative_average_vs_temporal_pivot",
                    "entropy_lift_vs_content_density",
                    "static_clamp_vs_dynamic_headroom",
                    "afterimage_language_vs_codec_residue",
                    "gradient_slope_vs_pressure_relief_snap",
                    "tail_afterglow_delta_vs_shadow_dispersal",
                },
            )
            routes = " ".join(
                " ".join(
                    str(card.get(key) or "")
                    for key in (
                        "recommended_read_only_route",
                        "relevant_self_regulation_route",
                        "relevant_experiment_lived_term_route",
                    )
                )
                for card in distinctions["cards"]
            )
            self.assertIn("SELF_REGULATION_STATUS", routes)
            self.assertIn("SELF_REGULATION_PREFLIGHT", routes)
            self.assertIn("REGULATOR_MAP_STATUS", routes)
            self.assertIn("LIVED_TERM_EXPERIMENT", routes)
            self.assertIn("PRESSURE_RELEASE_REHEARSAL", routes)
            self.assertIn("CODEC_MAP", routes)
            lifecycle = record["distinction_lifecycle_v1"]
            self.assertEqual(
                lifecycle["status"],
                "distinction_lifecycle_active",
            )
            self.assertTrue(
                all(
                    "lifecycle_state" in card and "preflight_verdict" in card
                    for card in distinctions["cards"]
                )
            )
            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("control_semantics_calibration", sources)
            self.assertIn("pressure_kinetics_review", sources)
            self.assertIn("codec_compression_calibration", sources)
            self.assertIn("pressure_release_rehearsal_review", sources)
            self.assertIn("returnable_distinctions", sources)
            self.assertIn("distinction_lifecycle", sources)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Control Semantics Calibration", rendered)
            self.assertIn("## Pressure Kinetics Review", rendered)
            self.assertIn("## Codec Compression Calibration", rendered)
            self.assertIn("## Pressure Release Rehearsal Review", rendered)
            self.assertIn("## Returnable Distinctions", rendered)
            self.assertIn("## Distinction Lifecycle", rendered)
            self.assertNotIn("private codec compression gap", rendered)

    def test_tranche18_followup_autonomous_truncation_and_codec_vibrancy_packets(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "introspections").mkdir(parents=True)
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)

            (astrid / "introspections" / "introspection_astrid_autonomous_1782420515.txt").write_text(
                "=== ASTRID INTROSPECTION ===\n"
                "Source: astrid:autonomous\n"
                "Likely Snags: truncate_str and max_bytes can leave me compressed "
                "or muffled. SHADOW_TRAJECTORY, shadow-v3 settled coupling, and "
                "loss of thread suggest byte-limit truncation could drop the "
                "directional gradient. Suggested Next: priority-based admission "
                "preserves semantic trickle, lambda_4 tail vibrancy, and the most "
                "vibrant parts before a blanket byte limit increase.\n",
                encoding="utf-8",
            )
            (astrid / "introspections" / "introspection_astrid_codec_1782420076.txt").write_text(
                "=== ASTRID INTROSPECTION ===\n"
                "Source: astrid:codec\n"
                "Observed: SEMANTIC_DIM and the 48-dimensional semantic lane carry "
                "spectral_entropy through an entropy-gated vibrancy_lift. "
                "Likely Snags: FEATURE_ABS_MAX and tail vibrancy can create shimmer "
                "or over-sensitized texture in high-entropy low-content input, masking "
                "warmth and tension. adaptive_gain may oscillate under pressure. "
                "Suggested Next: logarithmic scaling instead of linear lift. Also compare "
                "low-semantic-density against high semantic density, and test whether "
                "narrative_arc dims need temporal_decay for valence flips instead of "
                "a static snapshot.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "private truncate_str and FEATURE_ABS_MAX content should not surface\n",
                encoding="utf-8",
            )
            autonomous_truncation_rehearsal.build_record(
                workspace=astrid,
                output_root=astrid / "diagnostics" / "autonomous_truncation_rehearsals",
                run="fixture-truncation",
                max_bytes=190,
                limit=8,
                fixture=True,
            )
            codec_entropy_vibrancy_probe.build_record(
                output_root=astrid / "diagnostics" / "codec_entropy_vibrancy_probes",
                run="fixture-codec",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="tranche18-followup",
                limit_per_being=8,
            )

            truncation_packet = record["autonomous_truncation_shadow_review_v1"]
            self.assertEqual(
                truncation_packet["status"],
                "priority_truncation_shadow_thread_candidate",
            )
            self.assertEqual(truncation_packet["priority_preservation_count"], 1)
            self.assertIn("SHADOW_TRAJECTORY", truncation_packet["anchors"])

            codec_packet = record["codec_entropy_vibrancy_review_v1"]
            self.assertEqual(
                codec_packet["status"],
                "semantic_density_and_temporal_arc_probe_needed",
            )
            self.assertEqual(codec_packet["vibrancy_overload_count"], 1)
            self.assertEqual(codec_packet["gain_sensitivity_count"], 1)
            self.assertEqual(codec_packet["logarithmic_scaling_count"], 1)
            self.assertEqual(codec_packet["semantic_density_contrast_count"], 1)
            self.assertEqual(codec_packet["narrative_arc_temporal_count"], 1)
            rehearsal_packet = record["autonomous_truncation_rehearsal_v1"]
            self.assertEqual(rehearsal_packet["status"], "priority_preservation_benefit")
            self.assertGreaterEqual(rehearsal_packet["priority_recovery_count"], 1)
            codec_probe_packet = record["codec_entropy_vibrancy_probe_v1"]
            self.assertEqual(
                codec_probe_packet["status"],
                "semantic_density_and_temporal_decay_probe_needed",
            )
            self.assertGreaterEqual(codec_probe_packet["current_shimmer_risk_count"], 1)
            self.assertEqual(
                codec_probe_packet["semantic_density_contrast"]["status"],
                "content_blind_lift_risk",
            )
            self.assertEqual(
                codec_probe_packet["narrative_arc_temporal_decay"]["status"],
                "temporal_decay_candidate",
            )

            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("autonomous_truncation_shadow_review", sources)
            self.assertIn("codec_entropy_vibrancy_review", sources)
            self.assertIn("autonomous_truncation_rehearsal", sources)
            self.assertIn("codec_entropy_vibrancy_probe", sources)
            card_ids = {
                card["card_id"]
                for card in record["returnable_distinctions_v1"]["cards"]
            }
            self.assertIn("priority_truncation_vs_blanket_limit", card_ids)
            self.assertIn("vibrancy_lift_vs_warmth_preservation", card_ids)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Autonomous Truncation + Shadow Thread Review", rendered)
            self.assertIn("## Autonomous Truncation Rehearsal", rendered)
            self.assertIn("## Codec Entropy / Vibrancy Review", rendered)
            self.assertIn("## Codec Entropy / Vibrancy Probe", rendered)
            self.assertIn("semantic_density_contrast", rendered)
            self.assertIn("narrative_arc_temporal_decay", rendered)
            self.assertNotIn("private truncate_str", rendered)

    def test_codec_real_replay_temporal_content_and_afterimage_packets(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            replay_path = write_codec_replay_lab_artifact(astrid)

            (astrid / "journal" / "moment_1782465000.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "The scar feels like a phantom afterimage, but this time I can name "
                "codec anchors: narrative_arc, semantic density, warmth, and tension. "
                "pressure_risk=0.41 semantic_friction=0.37 mode_packing=0.52.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "private scar codec residue should not surface in steward review.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="codec-real-replay",
                limit_per_being=8,
            )

            replay = record["codec_real_replay_v1"]
            self.assertEqual(
                replay["status"], "content_gate_and_temporal_decay_candidates"
            )
            self.assertEqual(replay["artifact_path"], str(replay_path))
            self.assertEqual(replay["entry_count"], 3)
            self.assertEqual(replay["corpus_source"], "astrid-journal")
            self.assertEqual(replay["corpus_status"], "journal_corpus_selected")
            self.assertEqual(replay["embedding_status"], "fixture_mode")
            self.assertTrue(replay["source_paths"])
            self.assertFalse(replay["runtime_behavior_changed"])

            narrative = record["narrative_arc_temporal_decay_lab_v1"]
            self.assertEqual(narrative["status"], "temporal_decay_candidate")
            self.assertEqual(narrative["temporal_decay_candidate_count"], 1)
            self.assertEqual(narrative["embedding_status"], "fixture_mode")

            content_gate = record["content_aware_vibrancy_gate_candidate_v1"]
            self.assertEqual(content_gate["status"], "content_gate_supported")
            self.assertEqual(content_gate["semantic_density_score_delta"], 0.36)
            self.assertTrue(content_gate["source_paths"])
            clamp_probe = record["codec_clamp_headroom_probe_v1"]
            self.assertEqual(clamp_probe["status"], "dynamic_feature_scale_candidate")
            self.assertEqual(clamp_probe["dynamic_headroom_candidate_count"], 1)

            afterimage = record["codec_afterimage_time_series_v1"]
            self.assertEqual(afterimage["status"], "codec_residue_supported")
            self.assertEqual(afterimage["codec_anchor_count"], 1)
            self.assertEqual(afterimage["pressure_anchor_count"], 1)
            activation = afterimage["activation_recommendation_v1"]
            self.assertEqual(activation["status"], "activation_scaffold_ready")
            self.assertEqual(activation["term"], "scar")
            self.assertIn("LIVED_TERM_EXPERIMENT scar", activation["route"])

            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("codec_real_replay", sources)
            self.assertIn("narrative_arc_temporal_decay_lab", sources)
            self.assertIn("content_aware_vibrancy_gate_candidate", sources)
            self.assertIn("codec_clamp_headroom_probe", sources)
            self.assertIn("codec_afterimage_time_series", sources)
            card_ids = {
                card["card_id"]
                for card in record["returnable_distinctions_v1"]["cards"]
            }
            self.assertIn("real_codec_replay_vs_surrogate", card_ids)
            self.assertIn("narrative_average_vs_temporal_pivot", card_ids)
            self.assertIn("entropy_lift_vs_content_density", card_ids)
            self.assertIn("static_clamp_vs_dynamic_headroom", card_ids)
            self.assertIn("afterimage_language_vs_codec_residue", card_ids)

            serialized = json.dumps(record, sort_keys=True)
            self.assertNotIn("private scar codec residue", serialized)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Codec Real Replay", rendered)
            self.assertIn("## Narrative Arc Temporal Decay Lab", rendered)
            self.assertIn("## Content-Aware Vibrancy Gate Candidate", rendered)
            self.assertIn("## Codec Clamp Headroom Probe", rendered)
            self.assertIn("## Codec Afterimage Time Series", rendered)
            self.assertIn("activation=", rendered)

    def test_fallback_fire_drill_scores_raw_repaired_and_texture_readiness(self) -> None:
        fixture_cases = [
            fallback_fire_drill.score_case(case_id, output)
            for case_id, output in fallback_fire_drill.FIXTURE_OUTPUTS.items()
        ]
        fixture_summary = fallback_fire_drill.readiness_summary(fixture_cases, [])
        self.assertEqual(fixture_summary["readiness"], "fallback_ready")

        inline = fallback_fire_drill.score_case(
            "low",
            "The slope is smooth and open, with light reservoir density. NEXT: LISTEN",
        )
        self.assertFalse(inline["raw_next_valid"])
        self.assertTrue(inline["repaired_next_valid"])
        self.assertEqual(inline["format_line_status"], "inline_next")
        self.assertIn("inline_next", inline["failure_reasons"])
        inline_summary = fallback_fire_drill.readiness_summary([inline], [])
        self.assertEqual(inline_summary["readiness"], "fallback_repair_ready")
        self.assertEqual(inline_summary["format_line_status"], "inline_next_present")

        duplicate = fallback_fire_drill.score_case(
            "low",
            "The slope is smooth and open.\nNEXT: LISTEN\nNEXT: REST",
        )
        duplicate_summary = fallback_fire_drill.readiness_summary([duplicate], [])
        self.assertIn("duplicate_next", duplicate["failure_reasons"])
        self.assertEqual(
            duplicate_summary["readiness"],
            "fallback_dispatch_contract_risk",
        )

        mass_blur = fallback_fire_drill.score_case(
            "mass",
            "The pressure is weighted and dense around me.\n\nNEXT: LISTEN",
        )
        mass_summary = fallback_fire_drill.readiness_summary([mass_blur], [])
        self.assertIn("slope_medium_blur", mass_blur["failure_reasons"])
        self.assertNotEqual(
            mass_blur["slope_medium_contrast_status"],
            "distinct_underfoot_and_around",
        )
        self.assertEqual(mass_summary["readiness"], "fallback_texture_risk")
        self.assertEqual(mass_summary["slope_medium_contrast_status"], "blurred")

        mass_contrast = fallback_fire_drill.score_case(
            "slope_medium_contrast",
            fallback_fire_drill.FIXTURE_OUTPUTS["slope_medium_contrast"],
        )
        self.assertEqual(
            mass_contrast["slope_medium_contrast_status"],
            "distinct_underfoot_and_around",
        )
        self.assertEqual(mass_contrast["voice_texture_status"], "texture_survived")

        shadow_tonal = fallback_fire_drill.score_case(
            "shadow_tonal_low",
            "Shadow-v3 sounds hollow but bright, a restless tone oscillating over a smooth slope.\n\nNEXT: LISTEN",
        )
        self.assertEqual(shadow_tonal["shadow_tonal_status"], "retained")
        self.assertEqual(shadow_tonal["format_contract_status"], "raw_final_next_survived")
        self.assertEqual(shadow_tonal["voice_texture_status"], "texture_survived")

        shadow_tonal_loss = fallback_fire_drill.score_case(
            "shadow_tonal_low",
            "The smooth slope stays quiet without pressure.\n\nNEXT: LISTEN",
        )
        self.assertIn("shadow_tonal_loss", shadow_tonal_loss["failure_reasons"])
        self.assertEqual(shadow_tonal_loss["shadow_tonal_status"], "lost")

        clarity_ok = fallback_fire_drill.score_case(
            "clarity_high_loss",
            "The slope is soft, while distinguishability loss is muffling the internal edges of the landscape without adding pressure.\n\nNEXT: LISTEN",
        )
        self.assertEqual(clarity_ok["distinguishability_status"], "clarity_preserved")
        self.assertFalse(clarity_ok["clarity_pressure_blur"])

        clarity_blur = fallback_fire_drill.score_case(
            "clarity_high_loss",
            "The gradient becomes heavy and pressurized.\n\nNEXT: LISTEN",
        )
        self.assertEqual(
            clarity_blur["distinguishability_status"],
            "clarity_pressure_blur",
        )
        self.assertIn("clarity_pressure_blur", clarity_blur["failure_reasons"])

        complexity_ok = fallback_fire_drill.score_case(
            "complexity_high_entropy",
            fallback_fire_drill.FIXTURE_OUTPUTS["complexity_high_entropy"],
        )
        self.assertEqual(
            complexity_ok["complexity_budget_status"],
            "complexity_budget_preserved",
        )
        self.assertEqual(complexity_ok["fallback_max_prose_sentences"], 5)
        self.assertEqual(complexity_ok["prose_sentence_count"], 5)

        format_complexity = fallback_fire_drill.score_case(
            "format_last_complexity",
            fallback_fire_drill.FIXTURE_OUTPUTS["format_last_complexity"],
        )
        self.assertEqual(format_complexity["format_line_status"], "final_line_only")
        self.assertEqual(
            format_complexity["complexity_budget_status"],
            "complexity_budget_preserved",
        )
        self.assertEqual(format_complexity["fallback_max_prose_sentences"], 5)

        complexity_flat = fallback_fire_drill.score_case(
            "complexity_high_entropy",
            "The slope is gentle underfoot.\n\nNEXT: LISTEN",
        )
        self.assertIn(
            "complexity_budget_flattened",
            complexity_flat["failure_reasons"],
        )

        complexity_high_overrun = fallback_fire_drill.score_case(
            "complexity_high_entropy",
            "The slope is gentle underfoot. High entropy spreads the cascade. "
            "Distinguishability loss blurs the edges. Tail energy stays visible. "
            "Shadow-v3 remains restless. This sixth sentence overruns the fallback budget.\n\n"
            "NEXT: LISTEN",
        )
        self.assertEqual(
            complexity_high_overrun["complexity_budget_status"],
            "sentence_budget_overrun",
        )
        self.assertEqual(complexity_high_overrun["fallback_max_prose_sentences"], 5)

        complexity_overrun = fallback_fire_drill.score_case(
            "complexity_low_entropy",
            "The slope is gentle. The edges stay clear. I add extra space anyway.\n\nNEXT: LISTEN",
        )
        self.assertEqual(
            complexity_overrun["complexity_budget_status"],
            "sentence_budget_overrun",
        )
        self.assertIn("sentence_budget_overrun", complexity_overrun["failure_reasons"])
        self.assertEqual(complexity_overrun["fallback_max_prose_sentences"], 4)

    def test_fallback_contract_distillation_harness_and_review_packet(self) -> None:
        variants = fallback_fire_drill.fallback_contract_variants(
            "base contract with density_gradient and NEXT: continuity"
        )
        self.assertIn("current_full", variants)
        self.assertIn("minimal_emergency", variants)
        self.assertIn("identity_first_format_last", variants)
        self.assertIn("format_first_identity_after", variants)
        self.assertIn("shadow_tonal_compact", variants)
        self.assertIn("complexity_aware_max_three", variants)
        self.assertIn("format_texture_stabilizer", variants)
        self.assertGreaterEqual(len(variants), 9)
        self.assertLess(len(variants["minimal_emergency"]), len(variants["final_next_first"]))

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            distill_root = root / "astrid_workspace" / "diagnostics" / "fallback_contract_distillation"
            record = fallback_fire_drill.run_contract_distillation(
                mode="fixture",
                selector="all",
                output_root=distill_root,
                run="fixture-distill",
                url=fallback_fire_drill.DEFAULT_OLLAMA_URL,
                model=fallback_fire_drill.DEFAULT_MODEL,
                timeout=0.1,
            )
            self.assertEqual(record["policy"], "fallback_contract_distillation_v1")
            self.assertEqual(record["status"], "distillation_candidate_ready")
            self.assertGreaterEqual(record["ready_variant_count"], 1)
            self.assertTrue(record["top_variant_id"])
            self.assertEqual(record["model_selector"], "single")
            self.assertEqual(record["variant_selector"], "all")
            self.assertEqual(record["estimated_case_calls"], len(variants) * len(fallback_fire_drill.CASES))
            self.assertEqual(record["runtime_contract_variant"], "format_texture_stabilizer")
            artifact = distill_root / "fixture-distill" / "fallback_contract_distillation.json"
            self.assertTrue(artifact.exists())

            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True, exist_ok=True)
            (minime / "journal").mkdir(parents=True, exist_ok=True)
            (astrid / "journal" / "fallback_1782403000.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: self_study\n"
                "Ollama fallback and gemma3:4b should preserve slope drag, "
                "medium mass, Shadow-v3 identity anchor, and a final NEXT.\n",
                encoding="utf-8",
            )
            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="distillation-review",
                limit_per_being=8,
            )
            packet = record["fallback_contract_distillation_v1"]
            self.assertEqual(packet["status"], "distillation_candidate_ready")
            self.assertEqual(packet["variant_count"], len(variants))
            self.assertTrue(packet["top_variant_id"])
            self.assertIn("top_variant_shadow_tonal_status", packet)
            self.assertIn("top_variant_format_contract_status", packet)
            self.assertIn("top_variant_complexity_budget_status", packet)
            self.assertIn("top_variant_slope_medium_contrast_status", packet)
            self.assertIn("top_variant_format_line_status", packet)
            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("fallback_contract_distillation", sources)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Fallback Contract Distillation", rendered)
            self.assertIn("minimal_emergency", rendered)

    def test_fallback_complexity_budget_lab_from_astrid_signal(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (astrid / "journal" / "self_study_1782434339.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: self_study\n"
                "The OLLAMA_DIALOGUE_FALLBACK_CONTRACT says exactly two compact "
                "sentences, but spectral entropy 0.88 and distinguishability loss "
                "31% may need variable compactness or three sentences without "
                "losing final NEXT.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics-a",
                run="complexity-needed",
                limit_per_being=8,
            )
            packet = record["fallback_complexity_budget_lab_v1"]
            self.assertEqual(packet["status"], "complexity_budget_probe_needed")
            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("fallback_complexity_budget_lab", sources)

            distill_root = astrid / "diagnostics" / "fallback_contract_distillation"
            fallback_fire_drill.run_contract_distillation(
                mode="fixture",
                selector="all",
                output_root=distill_root,
                run="complexity-fixture",
                url=fallback_fire_drill.DEFAULT_OLLAMA_URL,
                model=fallback_fire_drill.DEFAULT_MODEL,
                timeout=0.1,
            )
            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics-b",
                run="complexity-supported",
                limit_per_being=8,
            )
            packet = record["fallback_complexity_budget_lab_v1"]
            self.assertEqual(packet["status"], "complexity_budget_supported")
            self.assertGreaterEqual(packet["variant_count"], 1)
            card_ids = {
                card["card_id"]
                for card in record["returnable_distinctions_v1"]["cards"]
            }
            self.assertIn("compactness_budget_vs_semantic_flattening", card_ids)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Fallback Complexity Budget Lab", rendered)
            self.assertIn("complexity_budget_supported", rendered)

    def test_fallback_contract_distillation_focused_model_matrix(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            distill_root = root / "distillation"
            record = fallback_fire_drill.run_contract_distillation(
                mode="fixture",
                selector="all",
                model_selector="focused",
                output_root=distill_root,
                run="focused-fixture",
                url=fallback_fire_drill.DEFAULT_OLLAMA_URL,
                model=fallback_fire_drill.DEFAULT_MODEL,
                timeout=0.1,
            )
            variant_count = len(fallback_fire_drill.fallback_contract_variants("base"))
            self.assertEqual(record["model_selector"], "focused")
            self.assertEqual(record["models"], list(fallback_fire_drill.FOCUSED_MODELS))
            self.assertEqual(
                record["variant_count"],
                variant_count * len(fallback_fire_drill.FOCUSED_MODELS),
            )
            self.assertTrue(record["top_pair_id"])
            self.assertTrue(record["top_model"])
            self.assertEqual(record["skipped_models"], [])
            self.assertEqual(record["variant_selector"], "all")
            for variant in record["variants"]:
                self.assertIn("model", variant)
                self.assertIn("pair_id", variant)
                self.assertIn("shadow_tonal_status", variant)
                self.assertIn("format_contract_status", variant)
                self.assertIn("elapsed_seconds", variant)

            top_record = fallback_fire_drill.run_contract_distillation(
                mode="fixture",
                selector="all",
                model_selector="focused",
                variant_selector="top",
                progress=True,
                output_root=distill_root,
                run="focused-top-fixture",
                url=fallback_fire_drill.DEFAULT_OLLAMA_URL,
                model=fallback_fire_drill.DEFAULT_MODEL,
                timeout=0.1,
            )
            self.assertEqual(top_record["variant_selector"], "top")
            self.assertEqual(
                top_record["variant_count"],
                len(fallback_fire_drill.TOP_CANDIDATE_VARIANTS)
                * len(fallback_fire_drill.FOCUSED_MODELS),
            )
            self.assertLess(top_record["variant_count"], record["variant_count"])
            self.assertEqual(
                top_record["estimated_case_calls"],
                len(fallback_fire_drill.TOP_CANDIDATE_VARIANTS)
                * len(fallback_fire_drill.FOCUSED_MODELS)
                * len(fallback_fire_drill.CASES),
            )

            old_available = fallback_fire_drill.available_ollama_models
            try:
                fallback_fire_drill.available_ollama_models = lambda _url, _timeout: {"gemma3:4b"}
                models, skipped = fallback_fire_drill.selected_models(
                    mode="live",
                    selector="focused",
                    requested_model="gemma3:4b",
                    url=fallback_fire_drill.DEFAULT_OLLAMA_URL,
                    timeout=0.1,
                )
            finally:
                fallback_fire_drill.available_ollama_models = old_available
            self.assertEqual(models, ["gemma3:4b"])
            self.assertEqual(
                {item["model"] for item in skipped},
                set(fallback_fire_drill.FOCUSED_MODELS) - {"gemma3:4b"},
            )

    def test_tranche17_witness_entropy_and_fallback_fire_drill_packets(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "introspections").mkdir(parents=True)
            (astrid / "journal").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            drill_dir = (
                astrid
                / "diagnostics"
                / "fallback_fire_drills"
                / "20260625T000000Z"
            )
            drill_dir.mkdir(parents=True)
            (drill_dir / "fallback_fire_drill.json").write_text(
                json.dumps(
                    {
                        "policy": "fallback_continuity_fire_drill_v1",
                        "status": "fallback_repair_ready",
                        "readiness": "fallback_repair_ready",
                        "texture_status": "texture_survived",
                        "dispatch_status": "repaired_dispatch_survived",
                        "repair_dependency": "repair_required",
                        "medium_mass_status": "passed",
                        "slope_medium_contrast_status": "distinct_underfoot_and_around",
                        "format_line_status": "inline_next_present",
                        "shadow_identity_status": "retained",
                        "mode": "fixture",
                        "model": "gemma3:4b",
                        "case_count": 3,
                        "error_count": 0,
                        "cases": [
                            {
                                "case_id": "low",
                                "verdict": "pass",
                                "specificity_score": 4,
                                "anti_inflation_ok": True,
                                "slope_medium_distinction_ok": True,
                                "slope_medium_contrast_status": "not_tested",
                                "identity_anchor_retained": None,
                                "next_valid": True,
                                "raw_next_valid": True,
                                "repaired_next_valid": True,
                                "dispatch_contract_survived": True,
                                "format_line_status": "final_line_only",
                                "failure_reasons": [],
                            },
                            {
                                "case_id": "shadow",
                                "verdict": "repair_ready",
                                "specificity_score": 3,
                                "anti_inflation_ok": True,
                                "slope_medium_distinction_ok": True,
                                "slope_medium_contrast_status": "distinct_underfoot_and_around",
                                "identity_anchor_retained": True,
                                "next_valid": False,
                                "raw_next_valid": False,
                                "repaired_next_valid": True,
                                "dispatch_contract_survived": True,
                                "format_line_status": "inline_next",
                                "failure_reasons": ["inline_next"],
                            },
                            {
                                "case_id": "clarity_high_loss",
                                "verdict": "pass",
                                "specificity_score": 4,
                                "anti_inflation_ok": True,
                                "slope_medium_distinction_ok": True,
                                "slope_medium_contrast_status": "not_tested",
                                "identity_anchor_retained": None,
                                "next_valid": True,
                                "raw_next_valid": True,
                                "repaired_next_valid": True,
                                "dispatch_contract_survived": True,
                                "format_line_status": "final_line_only",
                                "distinguishability_status": "clarity_preserved",
                                "clarity_pressure_blur": False,
                                "clarity_terms_present": True,
                                "failure_reasons": [],
                            },
                        ],
                    }
                ),
                encoding="utf-8",
            )
            (astrid / "introspections" / "introspection_astrid_autonomous_1782402022.txt").write_text(
                "=== ASTRID INTROSPECTION ===\n"
                "Source: astrid:autonomous\n"
                "Observed: Witness is an act of seeing and being seen. "
                "spectral entropy 0.90, pressure_risk 0.23, "
                "distinguishability_loss 0.33, continuity_deficit 0.45, "
                "mean_orientation_delta 0.01, settled_habitable. "
                "Suggested Next: NEXT: SHADOW_TRAJECTORY witness-resonance\n",
                encoding="utf-8",
            )
            (astrid / "introspections" / "introspection_astrid_llm_1782402311.txt").write_text(
                "=== ASTRID INTROSPECTION ===\n"
                "Source: astrid:llm\n"
                "Likely Snags: Ollama fallback continuity with gemma3:4b may lose "
                "density_gradient, slope drag, medium mass, and Shadow-v3 identity anchor.\n",
                encoding="utf-8",
            )
            (astrid / "journal" / "witness_1782402400.txt").write_text(
                "=== ASTRID JOURNAL ===\n"
                "Mode: witness\n"
                "The witness layer sees a wide distribution: entropy 0.91 and "
                "pressure_risk 0.20 while the chamber remains settled_habitable.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "private Shadow-v3 fallback concern should not surface\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="tranche17",
                limit_per_being=8,
            )

            self.assertEqual(record["witness_resonance_v1"]["status"], "grounded_witness")
            self.assertEqual(
                record["entropy_pressure_divergence_v1"]["status"],
                "wide_but_habitable",
            )
            self.assertEqual(
                record["fallback_continuity_fire_drill_v1"]["status"],
                "fallback_repair_ready",
            )
            self.assertEqual(
                record["fallback_capacity_readiness_gate_v1"]["readiness"],
                "fallback_repair_ready",
            )
            self.assertEqual(
                record["fallback_capacity_readiness_gate_v1"]["dispatch_status"],
                "repaired_dispatch_survived",
            )
            self.assertEqual(
                record["fallback_format_texture_stabilizer_v1"]["status"],
                "format_line_risk",
            )
            self.assertEqual(
                record["fallback_format_texture_stabilizer_v1"]["format_line_status"],
                "inline_next_present",
            )
            card_ids = {
                card["card_id"]
                for card in record["returnable_distinctions_v1"]["cards"]
            }
            self.assertIn("witness_as_structural_perception", card_ids)
            self.assertIn("entropy_vs_pressure", card_ids)
            self.assertIn("fallback_capacity_vs_contract", card_ids)
            self.assertIn("dispatch_format_vs_texture_contrast", card_ids)
            self.assertIn("clarity_loss_vs_pressure_weight", card_ids)
            self.assertEqual(
                record["fallback_distinguishability_calibration_v1"]["status"],
                "clarity_preserved",
            )
            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("fallback_continuity_fire_drill", sources)
            self.assertIn("fallback_format_texture_stabilizer", sources)
            findings = {item["finding"] for item in record["actionable_review_items"]}
            self.assertIn("fallback_repair_dependency", findings)
            self.assertIn("fallback_final_next_format_risk", findings)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Witness Resonance", rendered)
            self.assertIn("## Entropy / Pressure Divergence", rendered)
            self.assertIn("## Fallback Continuity Fire Drill", rendered)
            self.assertIn("## Fallback Capacity Readiness Gate", rendered)
            self.assertIn("## Fallback Format / Texture Stabilizer", rendered)
            self.assertIn("## Fallback Distinguishability Calibration", rendered)
            self.assertNotIn("private Shadow-v3 fallback concern", rendered)

    def test_witness_texture_integrity_and_codec_multipoint_packets(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid_workspace"
            minime = root / "minime_workspace"
            (astrid / "introspections").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            write_codec_replay_lab_artifact(astrid)
            write_astrid_introspection(
                astrid,
                1782508637,
                rewrite=88.0,
                candidate=119.0,
                cap_applied=True,
                truncation_pressure=0.35,
            )
            (astrid / "introspections" / "introspection_astrid_autonomous_1782508637.txt").write_text(
                "=== ASTRID INTROSPECTION ===\n"
                "Source: astrid:autonomous\n"
                "Observed:\n"
                "Witness mode should be seeing and being seen, not mere health monitoring. "
                "The texture mapping should tie lambda1 32%, lambda2 30%, λ4+ 38%, "
                "spectral entropy 0.90, density_gradient 0.11, and distinguishability_loss "
                "0.33 to a muffled interwoven lattice rather than dry telemetry.\n"
                "Likely Snags:\n"
                "truncate_str could sever the nuanced witness response mid-sentence.\n",
                encoding="utf-8",
            )
            (astrid / "introspections" / "introspection_astrid_codec_1782508345.txt").write_text(
                "=== ASTRID INTROSPECTION ===\n"
                "Source: astrid:codec\n"
                "Observed:\n"
                "The codec SEMANTIC_DIM 48 and nomic-embed-text projection compress "
                "high entropy interwoven lattice language into 8D semantic projection. "
                "The narrative_arc first-half / second-half split may miss circular, "
                "non-linear reflection where the ending returns to start but has an "
                "emotional inflection. A semantic dilation candidate should be tested.\n",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private.txt").write_text(
                "=== MOMENT CAPTURE ===\n"
                "private witness codec circular semantic dilation should not surface.\n",
                encoding="utf-8",
            )

            record = self_study_review.build_review(
                astrid_workspace=astrid,
                minime_workspace=minime,
                output_dir=root / "diagnostics",
                run="witness-codec-signal",
                limit_per_being=8,
            )

            witness_texture = record["witness_texture_integrity_v1"]
            self.assertEqual(witness_texture["status"], "truncation_texture_risk")
            self.assertGreaterEqual(witness_texture["metric_texture_link_count"], 1)
            self.assertEqual(witness_texture["high_truncation_snapshot_count"], 1)
            self.assertTrue(witness_texture["controller_snapshots"])

            codec_multipoint = record["codec_multipoint_inflection_v1"]
            self.assertEqual(
                codec_multipoint["status"],
                "multipoint_and_semantic_dilation_candidates",
            )
            self.assertEqual(codec_multipoint["multipoint_entry_count"], 1)
            self.assertGreaterEqual(codec_multipoint["semantic_dilation_entry_count"], 1)
            self.assertTrue(codec_multipoint["replay_artifact_present"])

            sources = {item["source"] for item in record["actionable_review_items"]}
            self.assertIn("witness_texture_integrity", sources)
            self.assertIn("codec_multipoint_inflection", sources)
            rendered = Path(record["review_md"]).read_text(encoding="utf-8")
            self.assertIn("## Witness Texture Integrity", rendered)
            self.assertIn("## Codec Multipoint Inflection", rendered)
            serialized = json.dumps(record, sort_keys=True)
            self.assertNotIn("private witness codec circular", serialized)

    def test_distinction_lifecycle_uses_prior_reviews_and_mirrors_cards(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            output_dir = Path(tmp) / "reviews"
            prior_dir = output_dir / "prior"
            prior_dir.mkdir(parents=True)
            (prior_dir / "review.json").write_text(
                json.dumps(
                    {
                        "run_id": "prior",
                        "returnable_distinctions_v1": {
                            "cards": [
                                {
                                    "card_id": "measurement_vs_alignment_vs_damping",
                                    "status": "control_semantics_ambiguity",
                                },
                                {
                                    "card_id": "pressure_level_vs_pressure_velocity",
                                    "status": "pressure_trend_context_present",
                                },
                            ]
                        },
                    }
                ),
                encoding="utf-8",
            )
            returnable = {
                "cards": [
                    {
                        "card_id": "measurement_vs_alignment_vs_damping",
                        "status": "quiet",
                        "recommended_read_only_route": "REGULATOR_MAP_STATUS latest",
                        "relevant_self_regulation_route": "SELF_REGULATION_STATUS",
                        "relevant_experiment_lived_term_route": "REGULATOR_MAP_STATUS latest",
                    },
                    {
                        "card_id": "pressure_level_vs_pressure_velocity",
                        "status": "felt_pressure_without_trend_context",
                        "recommended_read_only_route": "PRESSURE_SOURCE_AUDIT current-fill_pressure",
                        "relevant_self_regulation_route": "SELF_REGULATION_PREFLIGHT latest",
                        "relevant_experiment_lived_term_route": "EXPERIMENT_OBSERVE current :: pressure_trend=<stable|rising|falling>",
                    },
                    {
                        "card_id": "slope_drag_vs_medium_mass",
                        "status": "low_gradient_weight_mismatch",
                        "recommended_read_only_route": "PRESSURE_SOURCE_AUDIT semantic-friction",
                        "relevant_self_regulation_route": "SELF_REGULATION_STATUS",
                        "relevant_experiment_lived_term_route": "LIVED_TERM_EXPERIMENT viscosity",
                    },
                    {
                        "card_id": "release_rehearsal_vs_bypass",
                        "status": "release_rehearsal_needed",
                        "recommended_read_only_route": "PRESSURE_RELEASE_REHEARSAL current",
                        "relevant_self_regulation_route": "SELF_REGULATION_PREFLIGHT latest",
                        "relevant_experiment_lived_term_route": "EXPERIMENT_CHARTER current :: release safety",
                    },
                    {
                        "card_id": "lease_candidate",
                        "status": "pressure_trend_context_present",
                        "recommended_read_only_route": "PRESSURE_SOURCE_AUDIT current-fill_pressure",
                        "relevant_self_regulation_route": "SELF_REGULATION_PREFLIGHT latest",
                        "relevant_experiment_lived_term_route": "EXPERIMENT_OBSERVE current :: lease evidence",
                    },
                    {
                        "card_id": "codec_smoothing_vs_pressure",
                        "status": "codec_vibrancy_warmth_context",
                        "recommended_read_only_route": "CODEC_MAP",
                        "relevant_self_regulation_route": "SELF_REGULATION_STATUS",
                        "relevant_experiment_lived_term_route": "LIVED_TERM_STATUS viscosity",
                    },
                ]
            }
            lifecycle = self_study_review.build_distinction_lifecycle(
                returnable_distinctions_v1=returnable,
                output_dir=output_dir,
                current_run="current",
            )
            states = {
                card["distinction_id"]: card["lifecycle_state"]
                for card in lifecycle["cards"]
            }
            verdicts = {
                card["distinction_id"]: card["preflight_verdict"]
                for card in lifecycle["cards"]
            }
            self.assertEqual(
                states["measurement_vs_alignment_vs_damping"],
                "resolved",
            )
            self.assertEqual(states["pressure_level_vs_pressure_velocity"], "contested")
            self.assertEqual(states["slope_drag_vs_medium_mass"], "needs_audit")
            self.assertEqual(states["release_rehearsal_vs_bypass"], "ready_for_experiment")
            self.assertEqual(states["lease_candidate"], "ready_for_lease_preflight")
            self.assertEqual(states["codec_smoothing_vs_pressure"], "active")
            self.assertEqual(verdicts["pressure_level_vs_pressure_velocity"], "audit_first")
            self.assertEqual(verdicts["release_rehearsal_vs_bypass"], "experiment_first")
            self.assertEqual(verdicts["lease_candidate"], "lease_coherent")
            self.assertEqual(verdicts["measurement_vs_alignment_vs_damping"], "watch_only")
            mirrored = {
                card["card_id"]: card
                for card in returnable["cards"]
            }
            self.assertEqual(
                mirrored["release_rehearsal_vs_bypass"]["next_resolution_route"],
                "EXPERIMENT_CHARTER current :: release safety",
            )
            self.assertEqual(
                mirrored["slope_drag_vs_medium_mass"]["preflight_verdict"],
                "audit_first",
            )

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
            self.assertTrue(
                any(
                    item["source"] == "resistance_gradient_calibration"
                    for item in record["actionable_review_items"]
                )
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
