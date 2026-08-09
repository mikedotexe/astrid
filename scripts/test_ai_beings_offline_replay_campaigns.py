#!/usr/bin/env python3
"""Tests for deterministic AI Beings offline replay campaigns."""

from __future__ import annotations

import copy
import json
import tempfile
import unittest
from pathlib import Path

try:
    import ai_beings_offline_replay_campaigns as campaigns
except ModuleNotFoundError:  # pragma: no cover - package-style import
    from scripts import ai_beings_offline_replay_campaigns as campaigns


FROZEN_TEST_TIME = 1785316300.0


class OfflineReplayCampaignTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest = campaigns.freeze_manifest(frozen_at_unix=FROZEN_TEST_TIME)
        cls.report = campaigns.build_report(cls.manifest)

    def test_freeze_covers_every_current_runnable_trial_once(self) -> None:
        campaign_trials = [
            trial["trial_id"]
            for campaign in self.manifest["campaigns"]
            for trial in campaign["trials"]
        ]
        self.assertEqual(len(campaign_trials), self.manifest["runnable_trial_count"])
        self.assertEqual(len(campaign_trials), len(set(campaign_trials)))
        self.assertEqual(
            [campaign["campaign_key"] for campaign in self.manifest["campaigns"]],
            list(campaigns.CAMPAIGN_ORDER),
        )
        self.assertEqual(self.manifest["campaign_count"], 3)

    def test_manifest_is_exact_hash_and_excludes_private_moments(self) -> None:
        verification = campaigns.verify_manifest(self.manifest)
        self.assertTrue(verification["ok"], verification["errors"])
        self.assertFalse(
            any(
                Path(source["path"]).name.startswith("moment_")
                for source in self.manifest["sources"]
            )
        )
        for campaign in self.manifest["campaigns"]:
            for row in campaign["trials"]:
                self.assertEqual(
                    row["trial_packet_sha256"],
                    campaigns.sha256_json(row["trial_packet"]),
                )

    def test_changed_source_fails_closed(self) -> None:
        changed = copy.deepcopy(self.manifest)
        changed["sources"][0]["sha256"] = "0" * 64
        changed["manifest_sha256"] = campaigns.sha256_json(
            campaigns._manifest_hash_payload(changed)
        )
        verification = campaigns.verify_manifest(
            changed,
            verify_queue_snapshot=False,
        )
        self.assertFalse(verification["ok"])
        self.assertTrue(
            any("source SHA-256 mismatch" in error for error in verification["errors"])
        )

    def test_sealed_source_survives_original_path_removal(self) -> None:
        with tempfile.TemporaryDirectory(dir=campaigns.ASTRID_ROOT) as raw:
            root = Path(raw)
            original = root / "public_source.txt"
            original.write_text("bounded public source\n", encoding="utf-8")
            manifest = copy.deepcopy(self.manifest)
            manifest["sources"] = [
                campaigns.source_record(original, "bounded_public_texture_corpus")
            ]
            manifest["source_count"] = 1
            manifest["manifest_sha256"] = campaigns.sha256_json(
                campaigns._manifest_hash_payload(manifest)
            )
            sealed = campaigns.seal_manifest_sources(
                manifest,
                snapshot_root=root / "snapshots",
            )
            original.unlink()
            verification = campaigns.verify_manifest(
                sealed,
                verify_queue_snapshot=False,
            )
            self.assertTrue(verification["ok"], verification["errors"])
            source = sealed["sources"][0]
            self.assertTrue(source["snapshot_owner_only"])
            self.assertNotEqual(source["path"], source["source_path"])

    def test_report_is_deterministic_bounded_and_non_authorizing(self) -> None:
        second = campaigns.build_report(self.manifest)
        self.assertEqual(
            json.dumps(self.report, sort_keys=True),
            json.dumps(second, sort_keys=True),
        )
        self.assertTrue(self.report["deterministic_rerun"]["match"])
        self.assertFalse(self.report["network_access"])
        self.assertFalse(self.report["live_runtime_mutation"])
        self.assertFalse(self.report["raw_prose_included"])
        self.assertFalse(self.report["felt_effect_confirmed"])
        self.assertFalse(self.report["grants_authority"])
        self.assertTrue(self.report["silence_is_neutral"])
        serialized = json.dumps(self.report, sort_keys=True)
        self.assertNotIn('"excerpt":', serialized)
        self.assertNotIn('"line":', serialized)

    def test_semantic_gain_requires_quantified_samples(self) -> None:
        semantic = self.report["campaigns"][0]
        for row in semantic["results"]:
            result = row["result"]
            if not result["quantified_samples_present"]:
                self.assertEqual(
                    result["classification"],
                    "qualitative_lattice_signal_needs_quantified_samples",
                )

    def test_write_emits_one_bounded_card_per_trial(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            paths = campaigns.write_report(self.report, Path(tmp))
            self.assertEqual(
                len(paths["trial_card_paths"]),
                self.report["trial_count"],
            )
            aggregate = json.loads(
                Path(paths["aggregate_json"]).read_text(encoding="utf-8")
            )
            self.assertEqual(aggregate["report_sha256"], self.report["report_sha256"])
            for card in paths["trial_card_paths"]:
                text = Path(card).read_text(encoding="utf-8")
                self.assertIn("right_to_ignore: `true`", text)
                self.assertIn("felt_effect_confirmed: `false`", text)
                self.assertLess(len(text), 2200)


if __name__ == "__main__":
    unittest.main()
