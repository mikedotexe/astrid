"""Run explicitly selected offline inspector, never an engine or live checkpoint."""
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest


@unittest.skipUnless(os.environ.get("MINIME_RESTORE_INSPECT_BIN"), "explicit staged inspector required")
class RestoreInspectionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.binary = Path(os.environ["MINIME_RESTORE_INSPECT_BIN"]).resolve(strict=True)
        self.context = self.root / "context.json"
        self.scaffold = self.root / "scaffold.bin"
        self.metadata = self.root / "scaffold.json"
        self.state = {"integ_fill": 1, "integ_lam": 2, "integ_geom": 3,
                      "gate": 0.4, "filt": 0.2, "fill_rate_v1": {"available": True}}
        self.context.write_text(json.dumps(self.state))
        self.scaffold.write_bytes(b"".join(struct.pack("<f", float(i == j))
                                         for i in range(512) for j in range(512)))
        self.metadata.write_text(json.dumps({"source": "synthetic", "matrix_dim": 512, "trace": 512}))

    def inspect(self, *extra):
        files = [self.context, self.scaffold, self.metadata]
        before = [hashlib.sha256(path.read_bytes()).hexdigest() for path in files]
        result = subprocess.run([
            "/usr/bin/sandbox-exec", "-p", "(version 1)(allow default)(deny network*)(deny file-write*)",
            str(self.binary), "--context", str(self.context), "--scaffold", str(self.scaffold),
            "--scaffold-metadata", str(self.metadata), *extra,
        ], capture_output=True, text=True, timeout=10, cwd=self.root)
        self.assertEqual(before, [hashlib.sha256(path.read_bytes()).hexdigest() for path in files])
        self.assertEqual({path.name for path in self.root.iterdir()}, {path.name for path in files})
        return result

    def test_legacy_pi_only_and_unprimed_process_clock(self):
        result = self.inspect()
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertTrue(report["passed"])
        self.assertEqual(report["context"]["resume_mode"], "pi_only_resume")
        self.assertTrue(report["new_process_rate_unprimed"])
        self.assertFalse(report["covariance_checkpoint_loaded"])
        self.assertFalse(report["grants_restart_authority"])

    def test_requested_but_missing_adaptive_state_not_passed(self):
        report = json.loads(self.inspect("--restore-adaptive-target").stdout)
        self.assertFalse(report["passed"])
        self.assertEqual(report["context"]["state"], "partial")

    def test_complete_adaptive_restore(self):
        self.state.update({"fill_ema": 68, "adaptive_target": 68})
        self.context.write_text(json.dumps(self.state))
        report = json.loads(self.inspect("--restore-adaptive-target").stdout)
        self.assertTrue(report["passed"])
        self.assertEqual(report["context"]["resume_mode"], "restored_resume")

    def test_malformed_context_not_passed(self):
        self.context.write_text("{partial")
        report = json.loads(self.inspect().stdout)
        self.assertFalse(report["passed"])
        self.assertEqual(report["context"]["state"], "parse_error")

    def test_float_overflow_not_passed(self):
        self.state["integ_fill"] = 1e100
        self.context.write_text(json.dumps(self.state))
        self.assertFalse(json.loads(self.inspect().stdout)["passed"])

    def test_missing_and_nonfinite_matrix_rejected(self):
        for data in (b"truncated", struct.pack("<f", float("nan")) * (512 * 512)):
            self.scaffold.write_bytes(data)
            self.assertNotEqual(self.inspect().returncode, 0)

    def test_metadata_fallback_not_silently_qualified(self):
        for data in ("invalid", json.dumps({"source": "wrong", "matrix_dim": 2, "trace": 2})):
            self.metadata.write_text(data)
            self.assertNotEqual(self.inspect().returncode, 0)

    def test_oversized_input_rejected(self):
        self.context.write_bytes(b" " * 2_097_153)
        self.assertNotEqual(self.inspect().returncode, 0)


if __name__ == "__main__":
    unittest.main()
