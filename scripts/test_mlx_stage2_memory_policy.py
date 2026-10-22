"""Model-free boundaries for the operator-directed patch-upgrade memory policy."""
from __future__ import annotations

import unittest
from unittest.mock import patch

import astrid_mlx_memory as memory
from qualify_mlx_packages import (STAGE2_MANIFESTS, STAGE2_PREPARATION,
                                  validate_memory_policy)


def samples(levels, *, swap_step=0):
    return [{"t_s": i * 5., "pressure_level": level,
             "swap_bytes": 1024. + i * swap_step}
            for i, level in enumerate(levels)]


class Stage2MemoryPolicyTests(unittest.TestCase):
    def test_pressure_and_swap_are_diagnostic_even_when_sustained(self):
        result = memory.evaluate(samples([4] * 20, swap_step=1024),
                                 policy=memory.STAGE2_DIAGNOSTIC_POLICY)
        self.assertTrue(result["automated_ok"])
        self.assertEqual(result["peak_pressure_level"], 4)
        self.assertGreater(result["retained_swap_growth_bytes"], 0)
        self.assertTrue(result["swap_is_diagnostic"])

    def test_critical_pressure_does_not_refuse_workload_admission(self):
        row = {"t_s": 0., "pressure_level": 4, "swap_bytes": 1024.}
        with patch.object(memory, "sample", return_value=row):
            monitor = memory.Monitor(policy=memory.STAGE2_DIAGNOSTIC_POLICY)
            monitor.start()
            monitor.stop()

    def test_missing_or_invalid_telemetry_is_disclosed_without_memory_veto(self):
        missing = memory.evaluate([], policy=memory.STAGE2_DIAGNOSTIC_POLICY)
        self.assertTrue(missing["automated_ok"])
        self.assertFalse(missing["telemetry_complete"])
        bad = samples([1, 1]); bad[-1]["t_s"] = 20.
        gapped = memory.evaluate(bad, policy=memory.STAGE2_DIAGNOSTIC_POLICY)
        self.assertTrue(gapped["automated_ok"])
        self.assertFalse(gapped["telemetry_complete"])

    def test_scope_is_bound_to_selected_full_campaign(self):
        for stage, manifest in STAGE2_MANIFESTS.items():
            validate_memory_policy(memory.STAGE2_DIAGNOSTIC_POLICY,
                                   preparation_hash=STAGE2_PREPARATION,
                                   baseline_hash="observed-core-identity",
                                   manifest_hash=manifest, stage=stage,
                                   qualification_target="full")
        with self.assertRaisesRegex(ValueError, "exact 0.32.3"):
            validate_memory_policy(memory.STAGE2_DIAGNOSTIC_POLICY,
                                   preparation_hash=STAGE2_PREPARATION,
                                   baseline_hash="observed-core-identity",
                                   manifest_hash="0" * 64, stage="full",
                                   qualification_target="full")

    def test_historical_policies_retain_original_behavior(self):
        old = memory.evaluate(samples([1, 4, 1]), policy=memory.SWAP_DIAGNOSTIC_POLICY)
        self.assertIn("critical_host_memory_pressure", old["failure_reasons"])
        strict = memory.evaluate(samples([1, 1, 1], swap_step=1024),
                                 policy=memory.STRICT_POLICY)
        self.assertIn("retained_swap_growth", strict["failure_reasons"])


if __name__ == "__main__":
    unittest.main()
