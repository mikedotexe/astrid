import json
import tempfile
import unittest
from pathlib import Path

from migrate_edge_safe_fallback_authorship import SAFE_TAIL, migrate, sha256


class SafeFallbackAuthorshipMigrationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.workspace = Path(self.temporary.name)
        (self.workspace / "autonomous/turns").mkdir(parents=True)
        (self.workspace / "journal").mkdir()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def prepare(self, response: str) -> None:
        state = {
            "last_authored_transcript_path": "autonomous/turns/autonomous_42.md",
            "last_declared_next": "LISTEN",
            "last_response_sha256": "old",
        }
        (self.workspace / "autonomous/state.json").write_text(
            json.dumps(state), encoding="utf-8"
        )
        (self.workspace / "autonomous/turns/autonomous_42.md").write_text(
            "# turn\n\n## Response\n\n"
            f"{response}\n\n## Transport note\n\nconnected\n",
            encoding="utf-8",
        )
        (self.workspace / "journal/signal_42.md").write_text(
            f"# signal\n\n## Reflection\n\n{response}\n", encoding="utf-8"
        )

    def test_corrects_current_turn_and_records_audit_hashes(self) -> None:
        self.prepare(f"A genuine observation.\n\n{SAFE_TAIL}")
        self.assertTrue(migrate(self.workspace))

        transcript = (
            self.workspace / "autonomous/turns/autonomous_42.md"
        ).read_text(encoding="utf-8")
        journal = (self.workspace / "journal/signal_42.md").read_text(
            encoding="utf-8"
        )
        state = json.loads(
            (self.workspace / "autonomous/state.json").read_text(encoding="utf-8")
        )
        correction = json.loads(
            (self.workspace / "autonomous/authorship_corrections.jsonl")
            .read_text(encoding="utf-8")
            .strip()
        )

        self.assertNotIn(SAFE_TAIL, transcript)
        self.assertNotIn(SAFE_TAIL, journal)
        self.assertIn("executor note: legacy generic safe fallback excluded", transcript)
        self.assertIsNone(state["last_declared_next"])
        self.assertEqual(state["last_response_sha256"], sha256("A genuine observation."))
        self.assertEqual(correction["declared_next"], None)
        self.assertEqual(
            correction["reason"],
            "legacy_executor_safe_fallback_excluded_from_model_authorship",
        )
        self.assertFalse(migrate(self.workspace))

    def test_refuses_to_reclassify_transport_failure_as_authored(self) -> None:
        self.prepare(
            "Request timed out (Streaming phase exceeded 600s limit)\n\n" + SAFE_TAIL
        )
        with self.assertRaisesRegex(ValueError, "transport fallback"):
            migrate(self.workspace)


if __name__ == "__main__":
    unittest.main()
