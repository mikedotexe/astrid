from __future__ import annotations

import unittest

from scripts import benchmark_headless_models as benchmark


class BenchmarkHeadlessModelsTests(unittest.TestCase):
    def test_default_case_limits_remain_unchanged(self) -> None:
        self.assertEqual(
            benchmark.bounded_case_max_tokens(None),
            benchmark.CASE_MAX_TOKENS,
        )

    def test_output_cap_only_lowers_larger_case_limits(self) -> None:
        bounded = benchmark.bounded_case_max_tokens(112)
        self.assertEqual(bounded["reflection"], 112)
        self.assertEqual(bounded["artifact_action"], 96)
        self.assertEqual(bounded["tool_choice"], 64)

    def test_small_output_cap_applies_to_every_case(self) -> None:
        self.assertEqual(
            set(benchmark.bounded_case_max_tokens(32).values()),
            {32},
        )


if __name__ == "__main__":
    unittest.main()
