"""The offline projector must not reintroduce prescriptive return prompts."""
import copy
import unittest

from continuity_of_thought_audit import native_continuity


class ReturnProjectionTests(unittest.TestCase):
    def test_retains_evidence_without_prescribing_an_account(self):
        thread = {"title": "Synthetic", "motif_allowance_v1": {
            "returnability": 0.78, "dominant_motif": "decay", "quality": "deepening"}}
        experiment = {"question": "Does this mean the same thing?", "planned_next": "REST"}
        before = copy.deepcopy((thread, experiment))
        for owner in ("Astrid", "Minime"):
            with self.subTest(owner=owner):
                result = native_continuity(owner, thread, experiment, [])
                self.assertEqual(result["native_return_cue"], "")
                self.assertIn("evidence_lanes", result)
        self.assertEqual((thread, experiment), before)


if __name__ == "__main__":
    unittest.main()
