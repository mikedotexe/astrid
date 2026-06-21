#!/usr/bin/env python3
"""Tests for scripts/environment_receipts.py."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().with_name("environment_receipts.py")
SPEC = importlib.util.spec_from_file_location("environment_receipts", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
environment_receipts = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = environment_receipts
SPEC.loader.exec_module(environment_receipts)


class EnvironmentReceiptTests(unittest.TestCase):
    def test_record_writes_jsonl_latest_json_and_markdown(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            (workspace / "state.json").write_text(
                json.dumps(
                    {
                        "exchange_count": 42,
                        "creative_temperature": 0.7,
                        "history": [{}, {}],
                        "last_remote_memory_role": "latest",
                        "last_remote_glimpse_12d": [0.1, 0.2, 0.3, 0, 0, 0, 0, 0.8, 0, 0, 0.4, 0],
                    }
                )
            )

            receipt = environment_receipts.record_receipt(
                workspace,
                event="startup",
                source="unit-test",
                note="Bridge restarted cleanly.",
                details={"pid": "123"},
            )
            paths = environment_receipts.receipt_paths(workspace)

            self.assertTrue(paths["jsonl"].is_file())
            self.assertTrue(paths["latest_json"].is_file())
            self.assertTrue(paths["latest_md"].is_file())
            self.assertEqual(receipt["state_summary"]["exchange_count"], 42)
            self.assertEqual(receipt["state_summary"]["history_count"], 2)
            self.assertEqual(receipt["authority"], environment_receipts.RECEIPT_AUTHORITY)
            self.assertIn("Bridge restarted cleanly", paths["latest_md"].read_text())

    def test_summary_is_bounded_and_human_readable(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            ids = []
            for idx in range(3):
                receipt = environment_receipts.record_receipt(
                    workspace,
                    event=f"event_{idx}",
                    source="test",
                    note=f"note {idx}",
                )
                ids.append(receipt["id"])

            lines = environment_receipts.render_lines(
                environment_receipts.read_receipts(workspace),
                limit=2,
            )

            self.assertEqual(len(set(ids)), 3)
            self.assertEqual(len(lines), 2)
            self.assertIn("event_1", lines[0])
            self.assertIn("event_2", lines[1])
            self.assertNotIn("event_0", "\n".join(lines))

    def test_detail_parser_redacts_sensitive_keys(self) -> None:
        details = environment_receipts.parse_details(
            ["model=gemma", "api_key=secret-value", "plain detail"]
        )

        self.assertEqual(details["model"], "gemma")
        self.assertEqual(details["api_key"], "[redacted]")
        self.assertEqual(details["detail"], "plain detail")


if __name__ == "__main__":
    unittest.main()
