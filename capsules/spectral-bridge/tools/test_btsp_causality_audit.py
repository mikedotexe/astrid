import unittest

import btsp_causality_audit as audit


def sample_row(cohort: str, fragile: bool, reconcentrating: bool) -> dict:
    outcome = {
        "opening_vs_reconcentration": "reconcentrating" if reconcentrating else "widening",
        "distress_or_recovery": "recovery",
    }
    return {
        "proposal_id": f"{cohort}-{fragile}-{reconcentrating}",
        "owner": "minime",
        "choice_kind": "adjacent",
        "choice_key": "SELF_STUDY",
        "choice_label": "SELF_STUDY",
        "category": "epistemic",
        "cohort": cohort,
        "fragile_recovery": fragile,
        "fingerprint": "families=grinding_family;transition=expanding->plateau;crossing=none;perturb=tightening;fill_band=under",
        "components": {
            "families": "grinding_family",
            "transition": "expanding->plateau",
            "crossing": "none",
            "perturb": "tightening",
            "fill_band": "under",
        },
        "acted_at_unix_s": 10,
        "latency_minutes": 0.5,
        "latency_bucket": "<=1m",
        "outcome": outcome,
    }


class TestBtspsCausalityAudit(unittest.TestCase):
    def test_root_dominant_when_all_fragile_lanes_reconcentrate_similarly(self):
        rows = []
        for cohort in ("heavy_inquiry", "bounded_regulation", "other_minime_adjacent"):
            rows.extend(sample_row(cohort, True, True) for _ in range(14))
        summary = audit.summarize_rows(rows)
        self.assertEqual(summary["read"], "root_dominant")
        self.assertIsNone(summary["candidate_damp_lane"])

    def test_inquiry_load_candidate_requires_gap_and_count(self):
        rows = []
        rows.extend(sample_row("heavy_inquiry", True, True) for _ in range(12))
        rows.extend(sample_row("bounded_regulation", True, False) for _ in range(12))
        rows.extend(sample_row("other_minime_adjacent", True, False) for _ in range(12))
        summary = audit.summarize_rows(rows)
        self.assertEqual(summary["read"], "inquiry_load_candidate")
        self.assertEqual(summary["candidate_damp_lane"], "minime_inquiry_heavy_lane")

    def test_mixed_when_heavy_inquiry_sample_is_too_small(self):
        rows = []
        rows.extend(sample_row("heavy_inquiry", True, True) for _ in range(6))
        rows.extend(sample_row("bounded_regulation", True, False) for _ in range(12))
        summary = audit.summarize_rows(rows)
        self.assertEqual(summary["read"], "mixed")


if __name__ == "__main__":
    unittest.main()
