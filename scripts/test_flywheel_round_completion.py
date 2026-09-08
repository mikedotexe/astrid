#!/usr/bin/env python3
"""Black-box tests for the flywheel child completion contract."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPTS = Path(__file__).resolve().parent
HELPER = SCRIPTS / "flywheel_round_completion.py"
CHILD = SCRIPTS / "flywheel_round_child.sh"


class FlywheelRoundCompletionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.marker_dir = self.root / "private-marker"
        self.marker_dir.mkdir(mode=0o700)
        self.marker = self.marker_dir / "completion.json"
        self.run_id = "run_test_123"
        self.actor = "claude-heartbeat"
        self.env = os.environ.copy()
        self.env.update(
            {
                "FLYWHEEL_ASTRID_ROOT": str(self.root),
                "FLYWHEEL_ROUND_COMPLETION_FILE": str(self.marker),
                "STEWARD_RUN_ID": self.run_id,
                "STEWARD_ACTOR": self.actor,
            }
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_helper(
        self,
        *args: str,
        env: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(HELPER), *args],
            env=env or self.env,
            text=True,
            capture_output=True,
            timeout=10,
            check=False,
        )

    def make_packet(self, count: int = 1) -> Path:
        packet = (
            self.root
            / "docs"
            / "steward-notes"
            / "claude-heartbeat_123_test_round"
        )
        claims = packet / "claims"
        summaries = packet / "summaries"
        claims.mkdir(parents=True)
        summaries.mkdir()
        (packet / "RUN_REPORT.md").write_text("# Complete round\n", encoding="utf-8")
        for index in range(count):
            (claims / f"report_{index}.json").write_text("{}\n", encoding="utf-8")
            (summaries / f"report_{index}.md").write_text("complete\n", encoding="utf-8")
        artifacts = {
            "addressing_links.json": {},
            "read_manifest.json": {
                "reports": [
                    {"path": f"report_{index}.txt", "read": "complete"}
                    for index in range(count)
                ]
            },
            "source_receipts.json": {},
            "test_results.json": {},
            "unprocessed_selected.json": [],
            "verification_receipt.json": {
                "actor": self.actor,
                "controller": {
                    "run_id": self.run_id,
                    "preprojection_status": "passed",
                    "preprojection_generation_id": "projection_test",
                },
                "addressing": {
                    "fully_addressed": True,
                    "full_read_count": count,
                    "proof_missing_claims": [],
                },
                "division": {
                    "processed_report_count": count,
                    "round_event_id": "division_event_test",
                },
                "epistemic_lint": {
                    "valid": True,
                    "issue_count": 0,
                    "history_rewritten": False,
                },
                "counters": {"status": "consistent", "mismatches": []},
                "evidence_event_store": {
                    "valid": True,
                    "corrupt_lines": 0,
                    "errors": [],
                },
                "anti_drop": {"verify": {"alarms": 0, "gaps": 0}},
            },
        }
        for name, value in artifacts.items():
            (packet / name).write_text(json.dumps(value) + "\n", encoding="utf-8")
        return packet

    def test_complete_receipt_revalidates_exact_packet(self) -> None:
        packet = self.make_packet(2)
        recorded = self.run_helper(
            "complete",
            "--run-packet",
            str(packet),
            "--processed-report-count",
            "2",
        )
        verified = self.run_helper("verify", "--marker", str(self.marker))
        payload = json.loads(self.marker.read_text(encoding="utf-8"))

        self.assertEqual(recorded.returncode, 0, recorded.stderr)
        self.assertEqual(verified.returncode, 0, verified.stderr)
        self.assertEqual(payload["outcome"], "complete")
        self.assertEqual(payload["processed_report_count"], 2)
        self.assertIn("RUN_REPORT.md", payload["packet_files"])

    def test_complete_receipt_rejects_packet_mutation(self) -> None:
        packet = self.make_packet()
        recorded = self.run_helper(
            "complete",
            "--run-packet",
            str(packet),
            "--processed-report-count",
            "1",
        )
        (packet / "RUN_REPORT.md").write_text("changed\n", encoding="utf-8")
        verified = self.run_helper("verify", "--marker", str(self.marker))

        self.assertEqual(recorded.returncode, 0, recorded.stderr)
        self.assertNotEqual(verified.returncode, 0)
        self.assertIn("changed after completion", verified.stderr)

    def test_complete_receipt_requires_matching_counts_and_run(self) -> None:
        packet = self.make_packet()
        wrong_count = self.run_helper(
            "complete",
            "--run-packet",
            str(packet),
            "--processed-report-count",
            "2",
        )
        self.assertNotEqual(wrong_count.returncode, 0)
        self.assertFalse(self.marker.exists())

        recorded = self.run_helper(
            "complete",
            "--run-packet",
            str(packet),
            "--processed-report-count",
            "1",
        )
        wrong_env = self.env.copy()
        wrong_env["STEWARD_RUN_ID"] = "run_other"
        wrong_run = self.run_helper(
            "verify",
            "--marker",
            str(self.marker),
            env=wrong_env,
        )
        self.assertEqual(recorded.returncode, 0, recorded.stderr)
        self.assertNotEqual(wrong_run.returncode, 0)
        self.assertIn("different run", wrong_run.stderr)

    def test_complete_receipt_rejects_placeholder_integrity_evidence(self) -> None:
        packet = self.make_packet()
        receipt_path = packet / "verification_receipt.json"
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        receipt["evidence_event_store"]["valid"] = False
        receipt_path.write_text(json.dumps(receipt) + "\n", encoding="utf-8")

        result = self.run_helper(
            "complete",
            "--run-packet",
            str(packet),
            "--processed-report-count",
            "1",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Event Store verification is not valid", result.stderr)
        self.assertFalse(self.marker.exists())

    def test_no_input_receipt_is_explicit_and_nonproductive(self) -> None:
        recorded = self.run_helper(
            "no-input",
            "--reason-code",
            "canonical_queue_empty",
        )
        verified = self.run_helper("verify", "--marker", str(self.marker))
        payload = json.loads(self.marker.read_text(encoding="utf-8"))

        self.assertEqual(recorded.returncode, 0, recorded.stderr)
        self.assertEqual(verified.returncode, 0, verified.stderr)
        self.assertEqual(payload["outcome"], "no_input")
        self.assertEqual(payload["processed_report_count"], 0)
        self.assertIsNone(payload["run_packet"])

    def make_fake_claude(self) -> Path:
        fake = self.root / "fake-claude.sh"
        fake.write_text(
            """#!/bin/bash
cat >/dev/null
case "${FAKE_CLAUDE_MODE:-none}" in
  no_input)
    python3 "$FLYWHEEL_ROUND_COMPLETION_HELPER" no-input \\
      --reason-code canonical_queue_empty
    ;;
  complete)
    python3 "$FLYWHEEL_ROUND_COMPLETION_HELPER" complete \\
      --run-packet "$FAKE_RUN_PACKET" --processed-report-count 1
    ;;
  mutate)
    python3 "$FLYWHEEL_ROUND_COMPLETION_HELPER" complete \\
      --run-packet "$FAKE_RUN_PACKET" --processed-report-count 1
    printf 'changed\\n' > "$FAKE_RUN_PACKET/RUN_REPORT.md"
    ;;
  fail)
    exit 7
    ;;
esac
""",
            encoding="utf-8",
        )
        fake.chmod(0o700)
        return fake

    def run_child(
        self,
        mode: str,
        packet: Path | None = None,
    ) -> subprocess.CompletedProcess[str]:
        prompt = self.root / "prompt.txt"
        prompt.write_text("test prompt\n", encoding="utf-8")
        minime = self.root / "minime"
        minime.mkdir(exist_ok=True)
        env = self.env.copy()
        env.update(
            {
                "FLYWHEEL_MINIME_ROOT": str(minime),
                "FLYWHEEL_LOOP_PROMPT_FILE": str(prompt),
                "FLYWHEEL_CLAUDE_BIN": str(self.make_fake_claude()),
                "FLYWHEEL_ROUND_COMPLETION_HELPER": str(HELPER),
                "FAKE_CLAUDE_MODE": mode,
                "FAKE_RUN_PACKET": str(packet or ""),
            }
        )
        return subprocess.run(
            ["/bin/bash", str(CHILD)],
            env=env,
            text=True,
            capture_output=True,
            timeout=10,
            check=False,
        )

    def test_child_converts_unmarked_zero_exit_to_failure(self) -> None:
        result = self.run_child("none")
        self.assertEqual(result.returncode, 42)
        self.assertIn("without a valid completion receipt", result.stderr)

    def test_child_accepts_complete_and_no_input_receipts(self) -> None:
        complete = self.run_child("complete", self.make_packet())
        self.assertEqual(complete.returncode, 0, complete.stderr)

        self.marker.unlink(missing_ok=True)
        no_input = self.run_child("no_input")
        self.assertEqual(no_input.returncode, 0, no_input.stderr)

    def test_child_rejects_post_receipt_mutation_and_propagates_failure(self) -> None:
        mutated = self.run_child("mutate", self.make_packet())
        failed = self.run_child("fail")
        self.assertEqual(mutated.returncode, 42)
        self.assertIn("changed after completion", mutated.stderr)
        self.assertEqual(failed.returncode, 7)


if __name__ == "__main__":
    unittest.main()
