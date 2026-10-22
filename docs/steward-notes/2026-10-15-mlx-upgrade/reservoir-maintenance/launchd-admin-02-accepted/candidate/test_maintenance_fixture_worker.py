"""Small file/fake-clock tests; no workers, sockets or launchd jobs are started."""
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import maintenance_fixture_worker as fixture


class FixtureWorkerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.token = "a" * 32
        self.marker = {"schema": fixture.SCHEMA, "root": str(self.root),
                       "uid": os.getuid(), "token": self.token}
        (self.root / "fixture.owner.json").write_text(json.dumps(self.marker))

    def worker(self, role):
        return fixture.Worker(self.root, self.token, role, 6)

    def events(self, role):
        return [json.loads(line) for line in
                (self.root / f"{role}.events.jsonl").read_text().splitlines()]

    def test_foreign_marker_refused_before_worker_state(self):
        self.marker["token"] = "b" * 32
        (self.root / "fixture.owner.json").write_text(json.dumps(self.marker))
        with self.assertRaisesRegex(ValueError, "ownership mismatch"):
            self.worker("provider")
        self.assertEqual(list(self.root.glob("*.events.jsonl")), [])

    def test_short_drain_refused(self):
        with self.assertRaisesRegex(ValueError, "exceed five"):
            fixture.Worker(self.root, self.token, "provider", 5)

    def test_signal_only_marks_stop(self):
        worker = self.worker("reservoir")
        worker.stop()
        self.assertTrue(worker.stopping)
        self.assertFalse((self.root / "state.json").exists())

    def test_provider_needs_matching_ack_before_drained(self):
        worker = self.worker("provider")
        worker.stopping = True
        write = fixture.atomic

        def acknowledge(path, value):
            write(path, value)
            if path.name.startswith("request."):
                write(self.root / f"ack.{worker.request_id}.json",
                      {"token": self.token, "request_id": worker.request_id,
                       "ticks": 11, "ok": True})

        with patch.object(fixture, "atomic", side_effect=acknowledge), \
                patch.object(fixture.time, "sleep") as sleep:
            self.assertEqual(worker.provider(), 0)
        sleep.assert_called_once_with(6)
        phases = [x["phase"] for x in self.events("provider")]
        self.assertEqual(phases, ["idle", "generation_admitted", "stop_received",
                                  "checkin_sent", "checkin_acknowledged", "drained"])

    def test_bad_ack_never_reports_drained(self):
        worker = self.worker("provider")
        worker.stopping = True
        fixture.atomic(self.root / f"ack.{worker.request_id}.json",
                       {"token": self.token, "request_id": worker.request_id,
                        "ticks": 10, "ok": True})
        with patch.object(fixture.time, "sleep"):
            self.assertEqual(worker.provider(), 2)
        self.assertNotIn("drained", [x["phase"] for x in self.events("provider")])

    def test_final_snapshot_preserves_restored_values_after_drain(self):
        expected = {"fixture_astrid": {"ticks": 52}, "fixture_minime": {"ticks": 7}}
        fixture.atomic(self.root / "state.json", expected)
        worker = self.worker("reservoir")
        worker.stopping = True
        with patch.object(fixture.time, "sleep") as sleep:
            self.assertEqual(worker.reservoir(), 0)
        sleep.assert_called_once_with(6)
        self.assertEqual(json.loads((self.root / "state.json").read_text()), expected)
        events = self.events("reservoir")
        self.assertEqual([x["name"] for x in events if x["phase"] == "handle_saved"],
                         sorted(expected))
        self.assertEqual(events[-2]["phase"], "final_snapshot_complete")
        self.assertEqual(events[-1]["phase"], "drained")


if __name__ == "__main__":
    unittest.main()
