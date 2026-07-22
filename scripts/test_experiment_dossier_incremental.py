"""Integration tests for incremental experiment-dossier projection."""

from __future__ import annotations

import json
from pathlib import Path
import stat
import tempfile
import unittest
from unittest import mock

from evidence_store import EvidenceEventStore
from experiment_dossiers import (
    _cursor_path,
    family_state_dir,
    generate,
    project,
    state_dir,
    transition,
)
from evidence_store.adapter import read_domain_events


class ExperimentDossierIncrementalTests(unittest.TestCase):
    maxDiff = None

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.workspace = Path(self.temporary.name) / "workspace"
        diagnostics = self.workspace / "diagnostics"
        family_root = diagnostics / "claim_families_v1"
        sandbox_root = diagnostics / "sandbox_trial_queue_v1"
        family_root.mkdir(parents=True)
        sandbox_root.mkdir(parents=True)
        family_status = {
            "schema": "claim_family_status_v1",
            "schema_version": 1,
            "families": {
                "family_fixture": {
                    "claims": {
                        "introspection_astrid_100:claim_001": {
                            "claim_id": "claim_001",
                        }
                    }
                }
            },
        }
        sandbox_status = {
            "schema": "sandbox_trial_queue_status_v1",
            "schema_version": 1,
            "trials": {
                "trial_fixture": {
                    "trial_id": "trial_fixture",
                    "source_introspection_id": "introspection_astrid_100",
                    "claim_id": "claim_001",
                    "adapter": "offline_replay",
                    "trial_mode": "read_only",
                    "proposed_intervention": "compare bounded fixtures",
                    "agency_tier": 2,
                    "status": "ready",
                    "runnable": True,
                }
            },
        }
        (family_root / "status.json").write_text(
            json.dumps(family_status, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        (sandbox_root / "status.json").write_text(
            json.dumps(sandbox_status, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        store = EvidenceEventStore(
            diagnostics / "evidence_event_store_v2"
        )
        store.initialize_from_envelopes([], legacy_imported=False)
        store.activation_path.write_text(
            json.dumps(
                {
                    "schema": "evidence_store_activation_v1",
                    "schema_version": 1,
                    "active_store": "v2",
                }
            )
            + "\n",
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_unchanged_pass_uses_cursor_and_matches_full_replay(self) -> None:
        first = generate(self.workspace, write=True)
        self.assertTrue(first["full_reference_replay"])
        self.assertEqual(first["generated_event_count"], 1)
        cursor_path = _cursor_path(self.workspace)
        self.assertEqual(stat.S_IMODE(cursor_path.stat().st_mode), 0o600)

        with mock.patch(
            "experiment_dossiers.read_domain_events",
            side_effect=AssertionError("unchanged pass rescanned history"),
        ):
            second = generate(self.workspace, write=True)

        self.assertFalse(second["full_reference_replay"])
        self.assertEqual(second["incremental_consumed_event_count"], 0)
        self.assertEqual(second["generated_event_count"], 0)
        events, corrupt = read_domain_events(
            family_state_dir(self.workspace),
            "claim_families",
        )
        self.assertEqual(corrupt, 0)
        reference = project(events)
        persisted = json.loads(
            (state_dir(self.workspace) / "status.json").read_text(
                encoding="utf-8"
            )
        )
        for key, value in reference.items():
            self.assertEqual(persisted[key], value)

    def test_lived_context_refresh_never_advances_dossier_state(self) -> None:
        context_root = (
            self.workspace / "diagnostics/lived_state_witness_v1"
        )
        context_root.mkdir(parents=True)
        context_path = context_root / "context_index.jsonl"
        context = {
            "schema": "lived_state_context_index_v1",
            "witness_id": "lsw_" + "a" * 64,
            "introspection_id": "introspection_astrid_100",
            "alignment": {"outcome": "temporal_association_only"},
            "artifact_integrity_issue_count": 0,
            "gap_count": 0,
            "experiential_gap_claimed": False,
            "scalar_felt_dissimilarity_measured": False,
            "reconciliation_ref": None,
        }
        context_path.write_text(
            json.dumps(context, sort_keys=True) + "\n", encoding="utf-8"
        )
        first = generate(self.workspace, write=True)
        dossier_id = next(iter(first["dossiers"]))
        self.assertEqual(
            first["dossiers"][dossier_id]["state"], "capture-ready"
        )
        self.assertEqual(
            first["dossiers"][dossier_id]["lived_state_context_refs"][0][
                "witness_id"
            ],
            context["witness_id"],
        )
        lived_ref = first["dossiers"][dossier_id][
            "lived_state_context_refs"
        ][0]
        self.assertEqual(lived_ref["artifact_integrity_issue_count"], 0)
        self.assertFalse(lived_ref["experiential_gap_claimed"])
        self.assertFalse(lived_ref["scalar_felt_dissimilarity_measured"])
        transition(
            self.workspace,
            dossier_id,
            "baseline-captured",
            "sha256:" + "b" * 64,
            None,
        )
        context["alignment"] = {"outcome": "same_source_new_process"}
        context_path.write_text(
            json.dumps(context, sort_keys=True) + "\n", encoding="utf-8"
        )
        refreshed = generate(self.workspace, write=True)
        dossier = refreshed["dossiers"][dossier_id]
        self.assertEqual(dossier["state"], "baseline-captured")
        self.assertEqual(
            dossier["lived_state_context_refs"][0]["alignment"]["outcome"],
            "same_source_new_process",
        )
        self.assertFalse(
            dossier["lived_state_context_refs"][0][
                "state_transition_implied"
            ]
        )


if __name__ == "__main__":
    unittest.main()
