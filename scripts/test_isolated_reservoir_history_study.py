import json
from pathlib import Path
import socket
import sys
import unittest

import isolated_reservoir_history_study as study

sys.addaudithook(study.deny_network)
SOURCE = Path(__file__).resolve().parents[2] / "neural-triple-reservoir/triple_reservoir_coreml.py"


class HistoryTests(unittest.TestCase):
    def test_real_numpy_backend_null_controls_and_repeatability(self):
        first = study.run_study(SOURCE, (7,))
        second = study.run_study(SOURCE, (7,))
        self.assertEqual(first, second)
        trial = first["trials"][0]
        self.assertTrue(trial["controls_valid"])
        self.assertGreater(trial["metric_values"]["history_gap"], 0)
        self.assertGreater(trial["metric_values"]["different_future_input_gap"], 0)
        self.assertEqual(trial["metric_values"]["identical_history_gap"], 0)
        self.assertEqual(trial["metric_values"]["copied_state_reset_gap"], 0)
        self.assertEqual(trial["metric_values"]["execution_order_gap"], 0)
        self.assertFalse(first["being_learning_demonstrated"])
        self.assertEqual(first["plan_sha256"], study.digest(first["plan"]))
        json.dumps(first, allow_nan=False)

    def test_bounded_seeds(self):
        for seeds in ((), tuple(range(9)), (True,), (-1,)):
            with self.assertRaises(ValueError):
                study.run_study(SOURCE, seeds)

    def test_network_guard_blocks_live_connections(self):
        with socket.socket() as sock:
            with self.assertRaisesRegex(RuntimeError, "network forbidden"):
                sock.connect(("127.0.0.1", 7881))


if __name__ == "__main__":
    unittest.main()
