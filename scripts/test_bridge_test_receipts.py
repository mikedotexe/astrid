"""Test-result reuse orchestration with fake Cargo and synthetic state."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import bridge_test_receipts as receipts


class TestReceiptsTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.source = self.root / "source"
        self.source.mkdir()
        self.package = self.source / "crate"
        self.package.mkdir()
        self.target = self.root / "owned-cache"
        self.directory = self.root / "receipt-one"
        self.identity = {"schema": "bridge_rust_test_inputs_v1", "suite": "source-study", "command": ["cargo", "test"]}
        self.executions = 0

    def execute(self, command, source, environment, stdout, stderr):
        self.executions += 1
        stdout.write(b"synthetic suite passed\n")
        stderr.write(b"synthetic compiler messages\n")
        return 0

    def run_receipt(self, donor=None, executor=None, snapshots=None):
        with patch.object(receipts.stage, "local_packages", return_value=[self.package]), \
             patch.object(receipts, "inputs", side_effect=snapshots, return_value=self.identity), \
             patch.object(receipts, "execute", side_effect=executor or self.execute), \
             patch.object(receipts.subprocess, "run", return_value=subprocess.CompletedProcess([], 0)) as audit:
            result = receipts.qualify(self.source, self.target, self.directory, "source-study", donor)
            self.assertEqual(audit.call_count, 0 if donor else 1)
            return result

    def donor(self):
        result = self.run_receipt()
        donor = self.directory
        self.directory = self.root / "receipt-two"
        return donor, result

    def test_successful_reuse_is_explicit_preserves_execution_time_and_skips_execution(self):
        donor, original = self.donor()
        original_bytes = (donor / "receipt.json").read_bytes()
        reused = self.run_receipt(donor)
        self.assertEqual(self.executions, 1)
        self.assertEqual(reused["mode"], "reused")
        self.assertEqual(reused["executed_at"], original["executed_at"])
        self.assertEqual(reused["execution_seconds"], original["execution_seconds"])
        self.assertEqual((donor / "receipt.json").read_bytes(), original_bytes)
        self.assertEqual(receipts.verify(self.directory), reused)
        self.assertFalse(reused["activation_performed"])

    def test_changed_inputs_decline_without_fallback_execution(self):
        donor, _ = self.donor()
        self.identity["new_source"] = "changed"
        with self.assertRaisesRegex(ValueError, "inputs differ"):
            self.run_receipt(donor)
        self.assertEqual(self.executions, 1)
        self.assertFalse((self.directory / "receipt.json").exists())
        self.assertTrue((self.directory / "failure.json").exists())

    def test_log_tampering_is_refused(self):
        donor, _ = self.donor()
        (donor / "stdout.log").write_text("different")
        with self.assertRaisesRegex(ValueError, "input/log mismatch"):
            self.run_receipt(donor)
        self.assertEqual(self.executions, 1)

    def test_failed_run_retains_logs_and_is_not_reusable(self):
        def failure(*args):
            self.execute(*args)
            return 17
        with self.assertRaisesRegex(ValueError, "exit code 17"):
            self.run_receipt(executor=failure)
        self.assertTrue((self.directory / "stdout.log").is_file())
        self.assertTrue((self.directory / "failure.json").is_file())
        with self.assertRaisesRegex(ValueError, "failed attempt"):
            receipts.verify(self.directory)

    def test_drift_during_tests_fails_closed(self):
        with self.assertRaisesRegex(ValueError, "inputs changed"):
            self.run_receipt(snapshots=[dict(self.identity), {**self.identity, "changed": True}])
        self.assertFalse((self.directory / "receipt.json").exists())

    def test_existing_evidence_is_not_overwritten(self):
        self.run_receipt()
        before = (self.directory / "receipt.json").read_bytes()
        with self.assertRaisesRegex(ValueError, "must be new"):
            self.run_receipt()
        self.assertEqual(before, (self.directory / "receipt.json").read_bytes())

    def test_no_builds_inside_immutable_release_or_package(self):
        (self.root / "ready.json").write_text("{}")
        with self.assertRaisesRegex(ValueError, "inside a release"):
            self.run_receipt()
        (self.root / "ready.json").unlink()
        self.target = self.package / "build"
        with self.assertRaisesRegex(ValueError, "outside package"):
            self.run_receipt()

    def test_failed_cache_audit_prevents_execution(self):
        with patch.object(receipts.stage, "local_packages", return_value=[self.package]), \
             patch.object(receipts, "inputs", return_value=self.identity), \
             patch.object(receipts, "execute") as execute, \
             patch.object(receipts.subprocess, "run", side_effect=subprocess.CalledProcessError(2, ["audit"])):
            with self.assertRaises(subprocess.CalledProcessError):
                receipts.qualify(self.source, self.target, self.directory, "source-study")
            execute.assert_not_called()

    def test_live_environment_is_not_inherited_by_tests(self):
        with patch.dict(os.environ, {"PATH": "/tools", "HOME": "/home", "ASTRID_WORKSPACE": "/live",
                                    "RESERVOIR_WS_URL": "live", "ASTRID_HISTORY_CONTEXT_MEASUREMENTS": "/live/output"}, clear=True):
            result = receipts.test_environment()
        self.assertEqual(set(result), {"PATH", "HOME", "CARGO_PROFILE_DEV_DEBUG", "CARGO_PROFILE_TEST_DEBUG", "CARGO_INCREMENTAL"})

    def test_timeout_retains_partial_output_and_terminates_owned_process(self):
        script = "import os,time; print(os.getpid(), flush=True); time.sleep(60)"
        with (self.root / "out").open("wb") as stdout, (self.root / "err").open("wb") as stderr, \
             patch.object(receipts, "TIMEOUT", 0.3):
            with self.assertRaises(subprocess.TimeoutExpired):
                receipts.execute([sys.executable, "-c", script], self.root, dict(os.environ), stdout, stderr)
        pid = int((self.root / "out").read_text().strip())
        with self.assertRaises(ProcessLookupError):
            os.kill(pid, 0)

    def test_incomplete_or_forged_success_is_rejected(self):
        donor, _ = self.donor()
        record = json.loads((donor / "receipt.json").read_text())
        record["returncode"] = 2
        (donor / "receipt.json").write_text(json.dumps(record))
        with self.assertRaisesRegex(ValueError, "completed passing"):
            self.run_receipt(donor)

    def test_symlinked_evidence_is_rejected(self):
        donor, _ = self.donor()
        output = donor / "stdout.log"
        content = output.read_bytes()
        output.unlink()
        other = self.root / "same-log"
        other.write_bytes(content)
        output.symlink_to(other)
        with self.assertRaisesRegex(ValueError, "non-regular"):
            self.run_receipt(donor)

    def test_real_input_builder_binds_test_command_not_release_profile(self):
        with patch.object(receipts.stage, "local_packages", return_value=[self.package]), \
             patch.object(receipts.stage, "input_snapshot", return_value={}), \
             patch.object(receipts.stage, "host_target", return_value="fixture-target"), \
             patch.object(receipts.stage, "compilation_inputs", return_value={"recipe": {"profile": "release"}}):
            result = receipts.inputs(self.source, self.target, "public-web", receipts.test_environment())
        self.assertNotIn("recipe", result["source_inputs"])
        self.assertIn("astrid-public-web", result["command"])
        self.assertNotIn("--release", result["command"])
        self.assertEqual(result["command"][-2:], ["--", "--test-threads=1"])


if __name__ == "__main__":
    unittest.main()
