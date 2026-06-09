import unittest

import volitional_attractor_assessment as assess


class TestVolitionalAttractorAssessment(unittest.TestCase):
    def test_positive_control_requires_main_release_and_refresh(self):
        records = {"cooled-theme-edge": assess.blank_record("cooled-theme-edge")}
        status = {
            "seeds": {
                "minime-1": {
                    "author": "minime",
                    "command": "promote",
                    "control_eligible": True,
                    "has_h_state_fingerprint_16": True,
                    "label": "cooled-theme-edge",
                    "released_at_unix_s": 10,
                    "spectral_state": {"fill_pct": 66.0, "h_state_fingerprint_16": [0.1] * 16},
                    "substrate": "minime_esn",
                    "summon_count": 1,
                }
            },
            "observations": [],
        }
        events = [
            {
                "event": "seed_compared",
                "label": "cooled-theme-edge",
                "recurrence_score": 0.82,
                "authorship_score": 0.72,
                "classification": "authored",
            },
            {
                "event": "seed_summoned_main",
                "label": "cooled-theme-edge",
                "recurrence_score": 0.81,
            },
            {"event": "seed_released", "label": "cooled-theme-edge", "recurrence_score": 0.91},
            {
                "event": "seed_snapshot_refreshed",
                "label": "cooled-theme-edge",
                "recurrence_score": 1.0,
                "h_state_fingerprint_refreshed": True,
            },
        ]
        assess.merge_minime_status(records, status, events)
        assess.classify_records(records)
        record = records["cooled-theme-edge"]
        self.assertEqual(record["proof_status"], "positive_control")
        self.assertTrue(record["released"])
        self.assertTrue(record["snapshot"]["has_h_state_fingerprint_16"])

    def test_honey_selection_low_compare_becomes_near_miss(self):
        records = {"honey-selection": assess.blank_record("honey-selection")}
        assess.merge_astrid_ledger(
            records,
            [
                {
                    "record_type": "observation",
                    "label": "honey-selection",
                    "substrate": "astrid_codec",
                    "payload": (
                        '{"recurrence_score":0.3694,"authorship_score":0.72,'
                        '"classification":"failed","safety_level":"green"}'
                    ),
                }
            ],
        )
        assess.classify_records(records)
        record = records["honey-selection"]
        self.assertEqual(record["proof_status"], "below_threshold_near_miss")
        self.assertIn("REFRESH_ATTRACTOR_SNAPSHOT honey-selection", record["recommended_next"])

    def test_snag_detection_finds_orphan_cards_malformed_labels_and_no_seed_loops(self):
        records = {
            "/ SPREAD_ATTRACTOR": assess.blank_record("/ SPREAD_ATTRACTOR"),
            "cooled-theme-edge": assess.blank_record("cooled-theme-edge"),
        }
        events = [
            {"event": "compare_no_seed", "label": "cooled-theme-edge"},
            {"event": "compare_no_seed", "label": "cooled-theme-edge"},
            {"event": "seed_promoted", "label": "cooled-theme-edge"},
        ]
        atlas = {"entries": [{"label": "cooled-theme-edge"}]}
        cards = {
            "cooled-theme-edge": {"path": "cooled.md", "mtime": 1},
            "old-blend": {"path": "old.md", "mtime": 1},
        }
        fatigue = {"motifs": {"x": {"status": "released"}}}
        snags = assess.detect_snags(records, events, atlas, cards, fatigue)
        snag_ids = {snag["id"] for snag in snags}
        self.assertIn("orphan_card:old-blend", snag_ids)
        self.assertIn("malformed_label:spread-attractor", snag_ids)
        loop = next(snag for snag in snags if snag["id"] == "no_seed_loop:cooled-theme-edge")
        self.assertEqual(loop["status"], "resolved")
        self.assertIn("release_cools_fatigue_context", snag_ids)


if __name__ == "__main__":
    unittest.main()
