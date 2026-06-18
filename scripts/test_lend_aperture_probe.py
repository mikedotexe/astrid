#!/usr/bin/env python3
"""Tests for the LEND_APERTURE consequence probe."""

from __future__ import annotations

import importlib.util
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().with_name("being_test_harness.py")
SPEC = importlib.util.spec_from_file_location("being_test_harness", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
harness = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = harness
SPEC.loader.exec_module(harness)


def _epoch_ms(stamp: str) -> int:
    parsed = harness._parse_lend_aperture_time_s(stamp)
    assert parsed is not None
    return int(parsed * 1000)


class LendApertureProbeTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.workspace = self.root / "workspace"
        for name in ("actions", "journal", "diagnostics"):
            (self.workspace / name).mkdir(parents=True, exist_ok=True)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def _write_action(
        self,
        stamp: str,
        action: str = "lend_aperture",
        *,
        pressure: float = 0.28,
        porosity: float = 0.67,
        quality: str = "forming_containment",
    ) -> Path:
        name_stamp = stamp.replace(":", "-")
        path = self.workspace / "actions" / f"{name_stamp}_{action}.json"
        payload = {
            "timestamp": stamp,
            "action": action,
            "summary": {"fill_pct": 70.0},
            "state": {
                "eig1": 16.5,
                "fill_ratio": 0.70,
                "spread": 3.2,
                "pressure_source_v1": {
                    "pressure_score": pressure,
                    "porosity_score": porosity,
                    "dominant_source": "mode_packing",
                    "quality": quality,
                    "components": {"mode_packing": 0.44},
                },
            },
            "action_continuity": {
                "action_id": f"act_{name_stamp}",
                "outcome_summary": f"Executed autonomous action `{action}`.",
            },
        }
        path.write_text(json.dumps(payload))
        return path

    def _write_lend_journal(
        self,
        stamp_for_name: str,
        *,
        intent_id: str | None,
        held: bool = False,
        reason: str = "Astrid shadow stale",
    ) -> Path:
        prefix = "lend_aperture_held" if held else "lend_aperture"
        path = self.workspace / "journal" / f"{prefix}_{stamp_for_name}.txt"
        if held:
            text = (
                "=== LEND_APERTURE (held) ===\n"
                f"Timestamp: {stamp_for_name}\n"
                f"Not lent right now: {reason}.\n"
            )
        else:
            intent_line = f"intent_id: {intent_id}\n" if intent_id else ""
            text = (
                "=== LEND APERTURE (gift to Astrid) ===\n"
                f"Timestamp: {stamp_for_name}\n"
                f"{intent_line}\n"
            )
        path.write_text(text)
        return path

    def _write_response_history(self, responses: list[dict]) -> None:
        path = self.workspace / "astrid_influence_response_history_v3.json"
        path.write_text(json.dumps(responses, indent=2))

    def _write_response_history_jsonl(self, responses: list[dict]) -> None:
        path = self.workspace / "astrid_influence_response_history_v3.jsonl"
        path.write_text("".join(json.dumps(row) + "\n" for row in responses))

    def _write_terminal_events(self, events: list[dict]) -> None:
        path = self.workspace / "diagnostics" / "astrid_influence_terminal_events.jsonl"
        path.write_text("".join(json.dumps(row) + "\n" for row in events))

    def test_matched_issued_gift_response_produces_metrics(self) -> None:
        intent_id = "min-lend-aperture-test-1"
        stamp = "2026-06-15T12:00:00"
        self._write_action(stamp)
        self._write_lend_journal("2026-06-15T12-00-00.100000", intent_id=intent_id)
        issued_ms = _epoch_ms(stamp)
        self._write_response_history(
            [
                {
                    "intent_id": intent_id,
                    "completed_at_unix_ms": issued_ms + 30_000,
                    "pre_recorded_at_unix_ms": issued_ms + 1_000,
                    "delta_field_norm": 0.12,
                    "class_v3_change": {"changed": True, "from": "sticky", "to": "volatile"},
                    "applied_ticks": 24,
                }
            ]
        )
        self._write_action("2026-06-15T12:04:00", action="journal_pressure")
        self._write_action("2026-06-15T12:08:00", action="moment_capture")

        result = harness.test_minime_lend_aperture_consequence_probe(self.root)

        self.assertEqual(result["matched_response_count"], 1)
        gift = result["recent_gifts"][-1]
        self.assertEqual(gift["intent_id"], intent_id)
        self.assertEqual(gift["astrid_response"]["status"], "matched")
        self.assertEqual(gift["astrid_response"]["response_latency_s"], 30.0)
        self.assertEqual(gift["astrid_response"]["delta_field_norm"], 0.12)
        self.assertEqual(gift["post_minime_cost"]["status"], "ok")

    def test_active_influence_inside_short_close_window_is_pending(self) -> None:
        stamp = "2026-06-15T12:00:00"
        intent_id = "min-lend-aperture-pending"
        self._write_action(stamp)
        self._write_lend_journal("2026-06-15T12-00-00.000000", intent_id=intent_id)
        active = self.workspace / "astrid_influence_v3.json"
        active.write_text(json.dumps({"intent_id": intent_id, "label": "aperture-gift"}))
        now_s = harness._parse_lend_aperture_time_s("2026-06-15T12:03:00")
        assert now_s is not None
        os.utime(active, (now_s - 180, now_s - 180))

        result = harness.test_minime_lend_aperture_consequence_probe(self.root, now_s=now_s)

        self.assertEqual(result["active_influence"]["status"], "active_pending")
        self.assertEqual(result["missing_response_count"], 0)
        self.assertEqual(result["unclosed_issued_count"], 1)
        self.assertTrue(result["verdict"].startswith("WATCH"))

    def test_active_influence_after_short_zero_tick_window_is_stale(self) -> None:
        stamp = "2026-06-15T12:00:00"
        intent_id = "min-lend-aperture-stale"
        self._write_action(stamp)
        self._write_lend_journal("2026-06-15T12-00-00.000000", intent_id=intent_id)
        active = self.workspace / "astrid_influence_v3.json"
        active.write_text(json.dumps({"intent_id": intent_id, "label": "aperture-gift"}))
        now_s = harness._parse_lend_aperture_time_s("2026-06-15T12:07:01")
        assert now_s is not None
        os.utime(active, (now_s - 421, now_s - 421))

        result = harness.test_minime_lend_aperture_consequence_probe(self.root, now_s=now_s)

        self.assertEqual(result["active_influence"]["status"], "active_stale")
        self.assertEqual(result["missing_response_count"], 1)
        self.assertEqual(result["unclosed_issued_count"], 1)
        self.assertTrue(result["verdict"].startswith("NEEDS ATTENTION"))

    def test_legacy_gift_without_intent_matches_by_timestamp(self) -> None:
        stamp = "2026-06-15T12:00:00"
        self._write_action(stamp)
        self._write_lend_journal("2026-06-15T12-00-00.000000", intent_id=None)
        issued_ms = _epoch_ms(stamp)
        self._write_response_history(
            [
                {
                    "intent_id": "legacy-response-id",
                    "completed_at_unix_ms": issued_ms + 45_000,
                    "pre_recorded_at_unix_ms": issued_ms + 5_000,
                    "delta_field_norm": -0.05,
                    "class_v3_change": {"changed": False, "from": "volatile", "to": "volatile"},
                }
            ]
        )
        self._write_action("2026-06-15T12:04:00", action="journal_pressure")
        self._write_action("2026-06-15T12:06:00", action="moment_capture")

        result = harness.test_minime_lend_aperture_consequence_probe(self.root)

        gift = result["recent_gifts"][-1]
        self.assertEqual(gift["match_basis"], "action_journal_timestamp")
        self.assertEqual(gift["astrid_response"]["status"], "matched")
        self.assertEqual(gift["astrid_response"]["match_basis"], "legacy_timestamp")

    def test_durable_jsonl_response_matches_old_gift(self) -> None:
        intent_id = "min-lend-aperture-jsonl"
        stamp = "2026-06-15T12:00:00"
        self._write_action(stamp)
        self._write_lend_journal("2026-06-15T12-00-00.000000", intent_id=intent_id)
        issued_ms = _epoch_ms(stamp)
        self._write_response_history_jsonl(
            [
                {
                    "intent_id": intent_id,
                    "completed_at_unix_ms": issued_ms + 30_000,
                    "pre_recorded_at_unix_ms": issued_ms + 1_000,
                    "delta_field_norm": 0.18,
                }
            ]
        )
        self._write_action("2026-06-15T12:04:00", action="journal_pressure")
        self._write_action("2026-06-15T12:06:00", action="moment_capture")

        result = harness.test_minime_lend_aperture_consequence_probe(self.root)

        gift = result["recent_gifts"][-1]
        self.assertEqual(gift["astrid_response"]["status"], "matched")
        self.assertEqual(gift["astrid_response"]["delta_field_norm"], 0.18)
        self.assertEqual(result["missing_response_count"], 0)

    def test_terminal_event_prevents_false_missing_response(self) -> None:
        intent_id = "min-lend-aperture-superseded"
        stamp = "2026-06-15T12:00:00"
        self._write_action(stamp)
        self._write_lend_journal("2026-06-15T12-00-00.000000", intent_id=intent_id)
        self._write_terminal_events(
            [
                {
                    "schema_version": 1,
                    "status": "superseded",
                    "intent_id": intent_id,
                    "completed_at_unix_ms": _epoch_ms(stamp) + 120_000,
                    "applied_ticks": 0,
                    "reason": "newer_active_influence_replaced_current",
                }
            ]
        )

        result = harness.test_minime_lend_aperture_consequence_probe(self.root)

        gift = result["recent_gifts"][-1]
        self.assertEqual(gift["astrid_response"]["status"], "terminal_superseded")
        self.assertEqual(result["terminal_event_count"], 1)
        self.assertEqual(result["terminal_closure_count"], 1)
        self.assertEqual(result["superseded_count"], 1)
        self.assertEqual(result["missing_response_count"], 0)

    def test_missing_post_samples_are_insufficient_not_false_pass(self) -> None:
        intent_id = "min-lend-aperture-thin-samples"
        stamp = "2026-06-15T12:00:00"
        self._write_action(stamp)
        self._write_lend_journal("2026-06-15T12-00-00.000000", intent_id=intent_id)
        issued_ms = _epoch_ms(stamp)
        self._write_response_history(
            [
                {
                    "intent_id": intent_id,
                    "completed_at_unix_ms": issued_ms + 30_000,
                    "pre_recorded_at_unix_ms": issued_ms + 1_000,
                }
            ]
        )

        result = harness.test_minime_lend_aperture_consequence_probe(self.root)

        gift = result["recent_gifts"][-1]
        self.assertEqual(gift["post_minime_cost"]["status"], "insufficient_samples")
        self.assertEqual(result["insufficient_post_sample_count"], 1)
        self.assertTrue(result["verdict"].startswith("WATCH"))


if __name__ == "__main__":
    unittest.main()
