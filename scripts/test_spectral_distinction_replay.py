#!/usr/bin/env python3
"""Tests for the offline Spectral Distinction replay."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from spectral_distinction_replay import (
    ASTRID_REPLY_ID,
    MINIME_MESSAGE_ID,
    SHARED_THREAD_ID,
    build_replay,
    classify_reference,
)


class SpectralDistinctionReplayTests(unittest.TestCase):
    def test_reference_classification_covers_exact_resonance_absence_and_missing(self) -> None:
        exact = classify_reference("The Seed creation", b"The Seed creation lives here")
        resonance = classify_reference("lambda-tail", b"lambda1 spectral state")
        absent = classify_reference("The Seed creation", b"coupled recurrence")
        missing = classify_reference("anything", None)
        self.assertEqual(exact["classification"], "exact_presence")
        self.assertEqual(resonance["classification"], "resonance_only_similarity")
        self.assertEqual(absent["classification"], "absence")
        self.assertEqual(missing["classification"], "insufficient_evidence")

    def test_replay_proves_shared_lineage_without_copying_bodies(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            report = root / "report.txt"
            memory = root / "memory.md"
            shadow = root / "shadow.json"
            ledger = root / "correspondence.jsonl"
            report.write_text("Spectral Distinction Test", encoding="utf-8")
            memory.write_text("The Seed creation and lambda1", encoding="utf-8")
            shadow.write_text(
                json.dumps({"label": "lambda-tail/lambda4"}), encoding="utf-8"
            )
            rows = [
                {
                    "record_type": "message",
                    "message_id": MINIME_MESSAGE_ID,
                    "thread_id": SHARED_THREAD_ID,
                    "from_being": "minime",
                    "to_being": "astrid",
                },
                {
                    "record_type": "delivery_receipt",
                    "message_id": MINIME_MESSAGE_ID,
                    "thread_id": SHARED_THREAD_ID,
                },
                {
                    "record_type": "reply_link",
                    "message_id": MINIME_MESSAGE_ID,
                    "thread_id": SHARED_THREAD_ID,
                },
                {
                    "record_type": "message",
                    "message_id": ASTRID_REPLY_ID,
                    "thread_id": SHARED_THREAD_ID,
                    "from_being": "astrid",
                    "to_being": "minime",
                    "reply_to": MINIME_MESSAGE_ID,
                },
                {
                    "record_type": "read_receipt",
                    "message_id": ASTRID_REPLY_ID,
                    "thread_id": SHARED_THREAD_ID,
                },
            ]
            ledger.write_text(
                "".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8"
            )
            result = build_replay(
                astrid_memory_path=memory,
                minime_shadow_path=shadow,
                correspondence_path=ledger,
                source_introspection_path=report,
            )

        probes = {probe["probe_id"]: probe for probe in result["probes"]}
        self.assertEqual(
            probes["known_shared_correspondence_control"]["owner_result"][
                "classification"
            ],
            "shared_provenance",
        )
        self.assertEqual(
            probes["astrid_unique_memory_reference"]["cross_boundary_result"][
                "classification"
            ],
            "absence",
        )
        self.assertEqual(
            probes["minime_shadow_reference"]["cross_boundary_result"][
                "classification"
            ],
            "resonance_only_similarity",
        )
        serialized = json.dumps(result, sort_keys=True)
        self.assertNotIn("lives here", serialized)
        self.assertNotIn("body_preview", serialized)
        self.assertEqual(
            result["artifact_authority_state_v1"]["state"], "evidence_only"
        )
        self.assertFalse(result["live_eligible_now"])
        self.assertTrue(result["right_to_ignore"])

    def test_missing_source_is_insufficient_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            missing = Path(temp_dir) / "missing"
            result = build_replay(
                astrid_memory_path=missing,
                minime_shadow_path=missing,
                correspondence_path=missing,
                source_introspection_path=missing,
            )
        self.assertEqual(result["conclusion"]["report_grounding"], "insufficient_evidence")
        self.assertTrue(
            all(
                probe["owner_result"]["classification"] == "insufficient_evidence"
                for probe in result["probes"]
            )
        )


if __name__ == "__main__":
    unittest.main()
