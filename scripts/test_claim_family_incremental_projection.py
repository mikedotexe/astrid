"""Integration tests for materialized incremental claim families."""

from __future__ import annotations

import json
from pathlib import Path
import stat
import tempfile
import unittest
from unittest import mock

from claim_families import (
    _cursor_path,
    generate,
    project,
    state_dir,
)
from evidence_store import EvidenceEventStore
from evidence_store.adapter import read_domain_events


class ClaimFamilyIncrementalProjectionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.workspace = Path(self.temporary.name) / "workspace"
        self.addressing_path = (
            self.workspace
            / "diagnostics/introspection_addressing_v1/status.json"
        )
        self.addressing_path.parent.mkdir(parents=True)
        store = EvidenceEventStore(
            self.workspace / "diagnostics/evidence_event_store_v2"
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
        self._write_claims(("intro_one",))

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _write_claims(self, introspection_ids: tuple[str, ...]) -> None:
        artifacts = {}
        for introspection_id in introspection_ids:
            artifacts[introspection_id] = {
                "introspection_id": introspection_id,
                "source_family": "astrid_codec",
                "status": "read",
                "fully_addressed": False,
                "sha256": f"source-{introspection_id}",
                "relative_path": f"introspections/{introspection_id}.txt",
                "claims": {
                    "c001": {
                        "claim_id": "c001",
                        "summary": "Preserve exact sensory JSON compatibility.",
                        "disposition": "verified",
                        "classification": "evidence",
                        "evidence": [],
                    }
                },
            }
        self.addressing_path.write_text(
            json.dumps(
                {
                    "next_queue": [
                        {"introspection_id": value}
                        for value in introspection_ids
                    ],
                    "artifacts": artifacts,
                },
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )

    def test_cached_projection_avoids_replay_and_matches_reference(self) -> None:
        first = generate(self.workspace, write=True)
        self.assertTrue(first["full_reference_replay"])
        self.assertGreater(first["generated_event_count"], 0)
        self.assertEqual(stat.S_IMODE(_cursor_path(self.workspace).stat().st_mode), 0o600)

        with mock.patch(
            "claim_families.read_domain_events",
            side_effect=AssertionError("cached family projection rescanned history"),
        ):
            unchanged = generate(self.workspace, write=True)
        self.assertFalse(unchanged["full_reference_replay"])
        self.assertEqual(unchanged["incremental_consumed_event_count"], 0)
        self.assertEqual(unchanged["generated_event_count"], 0)

        original_family = next(iter(unchanged["families"]))
        self._write_claims(("intro_one", "intro_two"))
        with mock.patch(
            "claim_families.read_domain_events",
            side_effect=AssertionError("new claim rescanned family history"),
        ):
            changed = generate(self.workspace, write=True)
        self.assertLessEqual(changed["generated_event_count"], 2)
        self.assertIn(
            "intro_one:c001",
            changed["families"][original_family]["claims"],
        )
        self.assertIn(
            "intro_two:c001",
            changed["families"][original_family]["claims"],
        )

        events, corrupt = read_domain_events(
            state_dir(self.workspace),
            "claim_families",
        )
        self.assertEqual(corrupt, 0)
        reference = project(events, None)
        persisted = json.loads(
            (state_dir(self.workspace) / "status.json").read_text(
                encoding="utf-8"
            )
        )
        for key, value in reference.items():
            self.assertEqual(persisted[key], value)


if __name__ == "__main__":
    unittest.main()
