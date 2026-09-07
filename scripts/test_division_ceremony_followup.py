#!/usr/bin/env python3

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from division_ceremony_followup import (
    FollowupError,
    events_path,
    record_followup,
    record_round,
    state_path,
    verify,
)


class DivisionCeremonyFollowupTests(unittest.TestCase):
    def test_six_rounds_require_followup_and_retry_is_idempotent(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            workspace = Path(raw) / "workspace"
            for index in range(1, 7):
                state = record_round(
                    workspace,
                    steward_run_id=f"run-{index}",
                    processed_report_count=1,
                    projection_generation_id=f"projection-{index}",
                )
            self.assertTrue(state["review_due"])
            same = record_round(
                workspace,
                steward_run_id="run-6",
                processed_report_count=1,
                projection_generation_id="projection-6",
            )
            self.assertEqual(same["event_count"], 6)
            with self.assertRaisesRegex(FollowupError, "due"):
                record_round(
                    workspace,
                    steward_run_id="run-7",
                    processed_report_count=1,
                    projection_generation_id="projection-7",
                )

    def test_followup_resets_only_with_valid_chronicle_and_two_notes(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            workspace = Path(raw) / "workspace"
            chronicle = workspace / "chronicle.json"
            astrid_note = workspace / "astrid.txt"
            minime_note = workspace / "minime.txt"
            workspace.mkdir(parents=True)
            chronicle.write_text(
                json.dumps(
                    {
                        "chronicle_id": "division_chronicle_fixture",
                        "authority": {
                            "commit_recommended": False,
                            "silence_infers_consent": False,
                        },
                    }
                )
            )
            astrid_note.write_text("fixture")
            minime_note.write_text("fixture")
            baseline = record_followup(
                workspace,
                chronicle_json=chronicle,
                astrid_note=astrid_note,
                minime_note=minime_note,
                baseline=True,
            )
            self.assertEqual(baseline["cycle_sequence"], 1)
            self.assertFalse(baseline["review_due"])
            with self.assertRaisesRegex(FollowupError, "six completed"):
                record_followup(
                    workspace,
                    chronicle_json=chronicle,
                    astrid_note=astrid_note,
                    minime_note=minime_note,
                    baseline=False,
                )

    def test_tampering_and_permissions_fail_verification(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            workspace = Path(raw) / "workspace"
            record_round(
                workspace,
                steward_run_id="run-one",
                processed_report_count=2,
                projection_generation_id="projection-one",
            )
            self.assertEqual(events_path(workspace).stat().st_mode & 0o077, 0)
            self.assertEqual(state_path(workspace).stat().st_mode & 0o077, 0)
            state_path(workspace).chmod(0o644)
            with self.assertRaisesRegex(FollowupError, "owner-only"):
                verify(workspace)
            state_path(workspace).chmod(0o600)
            rows = events_path(workspace).read_text().splitlines()
            event = json.loads(rows[0])
            event["processed_report_count"] = 40
            events_path(workspace).write_text(json.dumps(event) + "\n")
            with self.assertRaisesRegex(FollowupError, "digest"):
                verify(workspace)


if __name__ == "__main__":
    unittest.main()
