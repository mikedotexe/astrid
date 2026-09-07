import json
from pathlib import Path
import tempfile
import time
import unittest
from unittest.mock import patch

import bridge_drain as drain


class BridgeDrainTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.binary = self.root / "bridge"
        self.binary.write_bytes(b"fixture binary")
        self.checkpoint = self.root / "state.json"
        self.checkpoint.write_text('{"exchange_count":12}')
        self.identity = (time.strftime("%a %b %d %H:%M:%S %Y"), str(self.binary))
        self.status = {"schema": "bridge_operator_drain_v1", "pid": 1234,
                       "instance": "a" * 32, "executable": str(self.binary),
                       "executable_sha256": drain.digest(self.binary),
                       "authority": "operator_maintenance_witness_only",
                       "started_at_unix_ms": int(time.time() * 1000), "phase": "running"}
        self.status_path = self.root / "1234.json"
        self.publish()
        self.process = patch.object(drain, "process_identity", return_value=self.identity).start()
        self.kill = patch.object(drain.os, "kill").start()
        self.addCleanup(patch.stopall)

    def publish(self):
        self.status_path.write_text(json.dumps(self.status))

    def call(self, request=True):
        return drain.drain(1234, self.binary, self.root, 0.01, request)

    def test_missing_legacy_status_never_signals(self):
        self.status_path.unlink()
        with self.assertRaisesRegex(drain.DrainError, "unsupported"):
            self.call()
        self.kill.assert_not_called()

    def test_inspection_is_read_only(self):
        self.assertFalse(self.call(False)["signal_sent"])
        self.kill.assert_not_called()

    def test_pid_reuse_is_rejected(self):
        self.status["started_at_unix_ms"] -= 10000
        self.publish()
        with self.assertRaisesRegex(drain.DrainError, "stale"):
            self.call()
        self.kill.assert_not_called()

    def test_changed_executable_is_rejected(self):
        self.binary.write_bytes(b"replacement")
        with self.assertRaisesRegex(drain.DrainError, "executable changed"):
            self.call()
        self.kill.assert_not_called()

    def test_process_identity_race_is_rejected(self):
        self.process.side_effect = [self.identity, ("different start", self.identity[1])]
        with self.assertRaisesRegex(drain.DrainError, "changed during validation"):
            self.call()
        self.kill.assert_not_called()

    def test_timeout_sends_one_usr1_and_never_forces(self):
        with self.assertRaisesRegex(drain.DrainError, "timeout"):
            self.call()
        self.kill.assert_called_once_with(1234, drain.signal.SIGUSR1)

    def test_request_requires_durable_matching_checkpoint(self):
        def acknowledge(*args):
            self.status["phase"] = "drained"
            self.status["checkpoint"] = {"path": str(self.checkpoint), "sha256": drain.digest(self.checkpoint)}
            self.publish()
        self.kill.side_effect = acknowledge
        result = self.call()
        self.assertEqual(result["phase"], "drained")
        self.assertFalse(result["remote_delivery_confirmed"])
        self.assertFalse(result["restart_performed"])

    def test_bad_checkpoint_fails_closed(self):
        self.status["phase"] = "drained"
        self.status["checkpoint"] = {"path": str(self.checkpoint), "sha256": "0" * 64}
        self.publish()
        with self.assertRaisesRegex(drain.DrainError, "checkpoint changed"):
            self.call()
        self.kill.assert_not_called()

    def test_failed_status_never_signals(self):
        self.status["phase"] = "failed"
        self.publish()
        with self.assertRaisesRegex(drain.DrainError, "failed"):
            self.call()
        self.kill.assert_not_called()


if __name__ == "__main__":
    unittest.main()
