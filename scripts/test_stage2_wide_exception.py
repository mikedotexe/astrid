"""Model-free fault tests; historical diagnostic remains failed."""
from __future__ import annotations

import json
from pathlib import Path
import unittest

import stage2_wide_exception as policy


ROOT = Path("/Users/v/other/astrid")
DIAGNOSTIC = ROOT / ".runtime/mlx-upgrade/stage2-0323-01/offline-diagnostic-01"


class WideExceptionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.log = (DIAGNOSTIC / "wide-coupling.log").read_text()
        cls.audit = json.loads((DIAGNOSTIC / "embedding-gate.json").read_text())
        cls.expected = {
            "stage": "full", "qualification_target": "full", "actual_returncode": 1,
            "wide_coupling_strength": 0.0,
            "expected_source_root": cls.audit["source_root"],
            "expected_source_hashes": cls.audit["source_hashes"],
            "expected_wrapper_sha256": cls.audit["wrapper_sha256"],
            "expected_model": cls.audit["model"],
        }

    def check(self, *, log=None, audit=None, **changes):
        kwargs = {**self.expected, **changes}
        return policy.evaluate(self.log if log is None else log,
                               self.audit if audit is None else audit, **kwargs)

    def test_known_log_matches_narrow_boundary_without_rewriting_failure(self):
        result = self.check()
        self.assertTrue(result["accepted_exception"])
        self.assertFalse(result["actual_test_passed"])
        self.assertEqual(result["actual_test_exit_code"], 1)
        self.assertAlmostEqual(result["measured_k16_ms_per_token"], 2.157)
        diagnostic = json.loads((DIAGNOSTIC / "diagnostic.json").read_text())
        self.assertFalse(diagnostic["passed"])

    def test_other_functional_failure_is_rejected(self):
        log = self.log.replace("  - latency:", "  - coherence: wrong\n  - latency:")
        self.assertFalse(self.check(log=log)["accepted_exception"])

    def test_missing_section_and_bad_measurement_are_rejected(self):
        self.assertFalse(self.check(log=self.log.replace("[e] off-is-off", "[e] hidden"))
                         ["accepted_exception"])
        self.assertFalse(self.check(log=self.log.replace("2.157 ms/token", "1.157 ms/token"))
                         ["accepted_exception"])

    def test_live_wide_path_and_other_target_are_rejected(self):
        self.assertFalse(self.check(wide_coupling_strength=0.1)["accepted_exception"])
        self.assertFalse(self.check(qualification_target="core")["accepted_exception"])
        self.assertFalse(self.check(actual_returncode=0)["accepted_exception"])

    def test_changed_test_source_or_wrapper_is_rejected(self):
        hashes = dict(self.audit["source_hashes"])
        hashes["test_wide_coupling_offline.py"] = "0" * 64
        self.assertFalse(self.check(expected_source_hashes=hashes)["accepted_exception"])
        self.assertFalse(self.check(expected_wrapper_sha256="0" * 64)["accepted_exception"])

    def test_failed_loader_or_extra_policy_error_is_rejected(self):
        audit = {**self.audit, "loader_calls": 0}
        self.assertFalse(self.check(audit=audit)["accepted_exception"])
        audit = {**self.audit, "policy_errors": ["bad"]}
        self.assertFalse(self.check(audit=audit)["accepted_exception"])


if __name__ == "__main__":
    unittest.main()
