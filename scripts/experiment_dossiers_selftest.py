"""Focused unit tests for experiment-dossier lifecycle semantics."""

from __future__ import annotations

from typing import Any
import unittest


def run() -> int:
    from experiment_dossiers import (
        apply_dossier_events,
        authority_state,
        intervention_signature,
        project,
        validate_transition,
    )

    class ExperimentDossierTests(unittest.TestCase):
        capture_ref = f"sha256:{'a' * 64}"

        def dossier(
            self,
            authority: str = "evidence_only",
        ) -> dict[str, Any]:
            return {
                "dossier_id": "dossier_one",
                "state": "capture-ready",
                "baseline_capture_ref": None,
                "candidate_capture_ref": None,
                "candidate_comparison_allowed": False,
                "dossier_sufficient": False,
                "artifact_authority_state_v1": authority_state(authority),
            }

        def test_candidate_is_refused_without_baseline(self) -> None:
            dossier = self.dossier()
            dossier["state"] = "baseline-captured"
            dossier["baseline_capture_ref"] = None
            with self.assertRaisesRegex(
                ValueError,
                "requires a valid baseline",
            ):
                validate_transition(
                    dossier,
                    "candidate-captured",
                    self.capture_ref,
                    None,
                )

        def test_approval_pending_candidate_requires_receipt(self) -> None:
            dossier = self.dossier("approval_pending")
            dossier["state"] = "baseline-captured"
            dossier["baseline_capture_ref"] = self.capture_ref
            with self.assertRaisesRegex(
                ValueError,
                "external approval receipt",
            ):
                validate_transition(
                    dossier,
                    "candidate-captured",
                    self.capture_ref,
                    None,
                )
            validate_transition(
                dossier,
                "candidate-captured",
                self.capture_ref,
                "operator_receipt",
            )

        def test_reprojection_preserves_advanced_state(self) -> None:
            projected = self.dossier()
            dossier = project(
                [
                    {
                        "event_type": "experiment_dossier_projected",
                        "dossier_id": "dossier_one",
                        "dossier": projected,
                    },
                    {
                        "event_type": "experiment_dossier_transitioned",
                        "dossier_id": "dossier_one",
                        "target_state": "baseline-captured",
                        "evidence_ref": self.capture_ref,
                    },
                    {
                        "event_type": "experiment_dossier_projected",
                        "dossier_id": "dossier_one",
                        "dossier": projected,
                    },
                ]
            )["dossiers"]["dossier_one"]
            self.assertEqual(dossier["state"], "baseline-captured")
            self.assertEqual(
                dossier["baseline_capture_ref"],
                self.capture_ref,
            )

        def test_invalid_replay_transition_is_a_violation(self) -> None:
            status = project(
                [
                    {
                        "event_type": "experiment_dossier_projected",
                        "dossier_id": "dossier_one",
                        "dossier": self.dossier(),
                    },
                    {
                        "event_type": "experiment_dossier_transitioned",
                        "dossier_id": "dossier_one",
                        "target_state": "candidate-captured",
                        "evidence_ref": self.capture_ref,
                    },
                ]
            )
            self.assertEqual(
                status["dossiers"]["dossier_one"]["state"],
                "capture-ready",
            )
            self.assertEqual(status["transition_violation_count"], 1)
            self.assertFalse(
                status["counter_audit"]["transition_replay_valid"]
            )

        def test_legacy_unresolved_family_is_unrouted(self) -> None:
            dossier = {
                **self.dossier(),
                "claim_family_id": "family_unresolved_old",
                "trial_refs": [{"trial_id": "trial_one"}],
            }
            status = project(
                [
                    {
                        "event_type": "experiment_dossier_projected",
                        "dossier_id": "dossier_old",
                        "dossier": dossier,
                    }
                ]
            )
            self.assertEqual(status["dossier_count"], 0)
            self.assertEqual(status["unrouted_trial_count"], 1)

        def test_projection_requires_baseline_for_comparison(self) -> None:
            status = apply_dossier_events(
                None,
                [
                    {
                        "event_type": "experiment_dossier_projected",
                        "dossier_id": "dossier_one",
                        "dossier": self.dossier(),
                    }
                ],
            )
            self.assertEqual(
                status["baseline_missing_violation_count"],
                0,
            )
            self.assertFalse(
                status["dossiers"]["dossier_one"][
                    "candidate_comparison_allowed"
                ]
            )

        def test_intervention_signature_is_stable_and_bounded(self) -> None:
            trial = {
                "adapter": "offline_replay",
                "trial_mode": "read_only",
                "proposed_intervention": "compare bounded fixtures",
                "agency_tier": 2,
            }
            self.assertEqual(
                intervention_signature(trial),
                intervention_signature(dict(trial)),
            )

    suite = unittest.defaultTestLoader.loadTestsFromTestCase(
        ExperimentDossierTests
    )
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1
