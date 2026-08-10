import json
import tempfile
import unittest
from pathlib import Path

from migrate_edge_transport_sentinels import migrate


class TransportSentinelMigrationTests(unittest.TestCase):
    def test_moves_only_exact_timeout_and_corrects_counters_idempotently(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            (workspace / "autonomous/turns").mkdir(parents=True)
            (workspace / "journal").mkdir()
            turn_id = 86_400_123
            state = {
                "utc_day": 1,
                "total_authored_turns": 5,
                "total_transport_recoveries": 2,
                "authored_turns_today": 3,
                "transport_recoveries_today": 1,
                "last_authored_transcript_path": f"autonomous/turns/autonomous_{turn_id}.md",
                "last_response_sha256": "old",
                "last_declared_next": "LISTEN",
            }
            (workspace / "autonomous/state.json").write_text(json.dumps(state))
            response = "Request timed out (AwaitingTools phase exceeded 120s limit)"
            (workspace / f"autonomous/turns/autonomous_{turn_id}.md").write_text(
                f"# turn\n\n## Response\n\n{response}\n\n## Transport note\n\nlocal\n"
            )
            (workspace / f"journal/signal_{turn_id}.md").write_text(
                f"# signal\n\n## Reflection\n\n{response}\n"
            )
            authored = workspace / "autonomous/turns/autonomous_86400124.md"
            authored.write_text(
                "# turn\n\n## Response\n\nI considered a timeout.\nNEXT: LISTEN\n\n"
                "## Transport note\n\nlocal\n"
            )

            self.assertEqual(migrate(workspace), 1)
            self.assertEqual(migrate(workspace), 0)
            self.assertTrue(authored.exists())
            self.assertFalse(
                (workspace / f"autonomous/turns/autonomous_{turn_id}.md").exists()
            )
            self.assertTrue(
                (
                    workspace
                    / f"autonomous/recoveries/legacy_transport_autonomous_{turn_id}.md"
                ).exists()
            )
            corrected = json.loads(
                (workspace / "autonomous/state.json").read_text(encoding="utf-8")
            )
            self.assertEqual(corrected["total_authored_turns"], 4)
            self.assertEqual(corrected["total_transport_recoveries"], 3)
            self.assertEqual(corrected["authored_turns_today"], 2)
            self.assertEqual(corrected["transport_recoveries_today"], 2)
            self.assertIsNone(corrected["last_authored_transcript_path"])
            correction = json.loads(
                (workspace / "autonomous/authorship_corrections.jsonl").read_text()
            )
            self.assertEqual(
                correction["reason"],
                "legacy_transport_sentinel_reclassified_non_authored",
            )


if __name__ == "__main__":
    unittest.main()
