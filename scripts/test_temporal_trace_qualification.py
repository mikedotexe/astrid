"""Independent metric oracle and adversarial synthetic input checks."""
import copy
import math
import random
import unittest

from temporal_trace_qualification import DIM, fixtures, lag_profile, paired_curve, plan, qualify


class TemporalTraceTests(unittest.TestCase):
    def test_qualification(self):
        self.assertEqual(qualify(plan())["outcome"], "passed")

    def test_independent_distance_oracle(self):
        rng = random.Random(20260922)
        left = fixtures()["periodic"]
        right = copy.deepcopy(left)
        for row in right:
            row["activations"] = [rng.uniform(-1, 1) for _ in range(DIM)]
        before = copy.deepcopy((left, right))
        curve = paired_curve(left, right)
        for a, b, result in zip(left, right, curve["samples"]):
            expected = math.dist(a["activations"], b["activations"]) / math.sqrt(DIM)
            self.assertAlmostEqual(result["rms"], expected, places=14)
        self.assertEqual((left, right), before)
        self.assertIn("no_boot_or_node_identity_attested", curve["comparison"])

    def test_lag_counts_and_actual_elapsed_time(self):
        frames = fixtures()["drift"]
        report = lag_profile(frames, [1000, 8000, 90_000])
        a, b, missing = report["lags"]
        self.assertEqual((a["pair_count"], b["pair_count"]), (63, 56))
        self.assertAlmostEqual(a["mean_rms"], .01)
        self.assertAlmostEqual(b["mean_rms"], .08)
        self.assertEqual(b["elapsed_range_ms"], [8000, 8000])
        self.assertEqual(missing["pair_count"], 0)
        self.assertIsNone(missing["mean_rms"])
        self.assertIsNone(missing["elapsed_range_ms"])

    def test_gaps_not_filled(self):
        report = lag_profile(fixtures()["gapped"], [1000, 16000])
        self.assertEqual(report["frame_count"], 56)
        self.assertEqual(report["gap_intervals_ms"], [(123000, 132000)])
        self.assertEqual(report["lags"][0]["pair_count"], 54)
        self.assertEqual(report["lags"][0]["pairs_crossing_gap"], 0)
        self.assertGreater(report["lags"][1]["pairs_crossing_gap"], 0)

    def test_explicit_tolerance_is_not_resampling(self):
        frames = fixtures()["drift"]
        for i, row in enumerate(frames):
            row["t_ms"] += (i % 2) * 50
        self.assertEqual(lag_profile(frames, [1000])["lags"][0]["pair_count"], 0)
        row = lag_profile(frames, [1000], 50)["lags"][0]
        self.assertEqual(row["pair_count"], 63)
        self.assertEqual(row["elapsed_range_ms"], [950, 1050])

    def test_invalid_input_is_rejected_not_sanitized(self):
        original = fixtures()["periodic"]
        variants = []
        for value in (float("nan"), float("inf"), True, 1.1, "0.1", 10**400):
            bad = copy.deepcopy(original)
            bad[0]["activations"][0] = value
            variants.append(bad)
        for change in (lambda x: x[1].update(t_ms=x[0]["t_ms"]),
                       lambda x: x[0].update(t_ms=-1),
                       lambda x: x[-1].update(t_ms=500000),
                       lambda x: x[0]["activations"].pop(),
                       lambda x: x[0].update(sanitized=True)):
            bad = copy.deepcopy(original)
            change(bad)
            variants.append(bad)
        variants.extend([original[:31], original * 3])
        for bad in variants:
            with self.subTest(shape=len(bad)), self.assertRaises(ValueError):
                lag_profile(bad, [1000])

    def test_pairing_refuses_clock_alignment(self):
        left = fixtures()["periodic"]
        right = copy.deepcopy(left)
        right[10]["t_ms"] += 1
        with self.assertRaises(ValueError):
            paired_curve(left, right)

    def test_invalid_parameters_and_stale_plan(self):
        for lags, tolerance in (([], 0), ([True], 0), ([1000, 1000], 0),
                                ([1000], 1000), ([1000], -1), ([180001], 0)):
            with self.subTest(lags=lags, tolerance=tolerance), self.assertRaises(ValueError):
                lag_profile(fixtures()["periodic"], lags, tolerance)
        changed = plan()
        changed["identity_tolerance"] = .1
        with self.assertRaises(ValueError):
            qualify(changed)


if __name__ == "__main__":
    unittest.main()
