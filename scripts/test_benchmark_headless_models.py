from __future__ import annotations

import os
from pathlib import Path
import tempfile
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

    def test_operator_output_helpers_tighten_existing_modes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "benchmark"
            directory.mkdir(mode=0o755)
            artifact = directory / "summary.tsv"
            artifact.write_text("prior\n")
            os.chmod(artifact, 0o644)

            benchmark.ensure_owner_only_directory(directory)
            benchmark.write_owner_only_text(artifact, "current\n")

            self.assertEqual(directory.stat().st_mode & 0o777, 0o700)
            self.assertEqual(artifact.stat().st_mode & 0o777, 0o600)


if __name__ == "__main__":
    unittest.main()
