#!/usr/bin/env python3
"""Focused regression for the lived-state artifact-integrity triage consumer.

The distinction under test is the one the projection currently cannot make: a
benign parameter-observation capture-ordering artifact versus a substantive
receipt-integrity finding. Collapsing the two is what let 1,129 recorded
issues sit unclassified while no steward saw them.
"""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import lived_state_integrity_triage as triage


def _witness(authored_at: int, observed_offsets: list[int]) -> dict:
    return {
        "authored_at_unix_ms": authored_at,
        "parameter_observations_v1": [
            {
                "name": f"p{index}",
                "observed_at_unix_ms": authored_at + offset,
            }
            for index, offset in enumerate(observed_offsets)
        ],
    }


def _issue_row(witness_id: str, errors: list[str], introspection_id: str) -> dict:
    return {
        "witness_id": witness_id,
        "aggregate_id": witness_id,
        "introspection_id": introspection_id,
        "errors": errors,
        "event_type": "lived_state_artifact_integrity_issue_detected",
    }


class TriageCase(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.witness_dir = self.root / "witnesses"
        self.witness_dir.mkdir()
        self.issues_path = self.root / "artifact_integrity_issues.jsonl"
        self.addCleanup(self._tmp.cleanup)

    def _write(self, rows: list[dict]) -> None:
        with self.issues_path.open("w", encoding="utf-8") as handle:
            for row in rows:
                handle.write(json.dumps(row) + "\n")

    def _write_witness(self, witness_id: str, payload: dict) -> None:
        (self.witness_dir / f"{witness_id}.json").write_text(
            json.dumps(payload), encoding="utf-8"
        )

    def _run(self, tolerance_ms: int = 1000) -> dict:
        return triage.triage(self.issues_path, self.witness_dir, tolerance_ms)

    def test_same_instant_capture_ordering_is_benign(self) -> None:
        """The live signature: every error is capture ordering, deltas ~1ms."""
        self._write_witness("lsw_a", _witness(1_000_000, [1, 1, 1]))
        self._write(
            [
                _issue_row(
                    "lsw_a",
                    [
                        "parameter_observations[0].observed_at_unix_ms:after_authorship",
                        "parameter_observations[1].observed_at_unix_ms:after_authorship",
                        "parameter_observations[2].observed_at_unix_ms:after_authorship",
                    ],
                    "introspection_a",
                )
            ]
        )
        result = self._run()
        self.assertEqual(result["capture_ordering_artifact_count"], 1)
        self.assertEqual(result["substantive_count"], 0)
        self.assertEqual(result["max_capture_ordering_delta_ms"], 1)

    def test_other_error_kind_is_substantive(self) -> None:
        """A non-capture-ordering error must never be absorbed as benign."""
        self._write_witness("lsw_b", _witness(1_000_000, [1]))
        self._write(
            [
                _issue_row(
                    "lsw_b",
                    [
                        "parameter_observations[0].observed_at_unix_ms:after_authorship",
                        "source.file_sha256:mismatch",
                    ],
                    "introspection_b",
                )
            ]
        )
        result = self._run()
        self.assertEqual(result["substantive_count"], 1)
        self.assertEqual(result["capture_ordering_artifact_count"], 0)

    def test_delta_beyond_tolerance_is_substantive(self) -> None:
        """Correct error kind but genuinely post-hoc telemetry stays visible."""
        self._write_witness("lsw_c", _witness(1_000_000, [5_000]))
        self._write(
            [
                _issue_row(
                    "lsw_c",
                    [
                        "parameter_observations[0].observed_at_unix_ms:after_authorship"
                    ],
                    "introspection_c",
                )
            ]
        )
        result = self._run()
        self.assertEqual(result["substantive_count"], 1)
        self.assertIn("exceeds tolerance", result["substantive"][0]["reason"])

    def test_unreadable_witness_is_not_assumed_benign(self) -> None:
        """No witness bytes means the deltas are unverified, so not benign."""
        self._write(
            [
                _issue_row(
                    "lsw_missing",
                    [
                        "parameter_observations[0].observed_at_unix_ms:after_authorship"
                    ],
                    "introspection_d",
                )
            ]
        )
        result = self._run()
        self.assertEqual(result["substantive_count"], 1)
        self.assertEqual(
            result["substantive"][0]["delta_evidence"], "witness_file_absent"
        )

    def test_verify_exits_nonzero_only_on_substantive(self) -> None:
        self._write_witness("lsw_e", _witness(1_000_000, [1]))
        self._write(
            [
                _issue_row(
                    "lsw_e",
                    [
                        "parameter_observations[0].observed_at_unix_ms:after_authorship"
                    ],
                    "introspection_e",
                )
            ]
        )
        argv = [
            "verify",
            "--json",
            "--issues-path",
            str(self.issues_path),
            "--witness-dir",
            str(self.witness_dir),
        ]
        self.assertEqual(triage.main(argv), 0)

        self._write_witness("lsw_f", _witness(1_000_000, [90_000]))
        self._write(
            [
                _issue_row(
                    "lsw_f",
                    [
                        "parameter_observations[0].observed_at_unix_ms:after_authorship"
                    ],
                    "introspection_f",
                )
            ]
        )
        self.assertEqual(triage.main(argv), 1)

    def test_malformed_stream_line_is_counted_not_swallowed(self) -> None:
        self.issues_path.write_text("{not json}\n", encoding="utf-8")
        result = self._run()
        self.assertEqual(result["malformed_line_count"], 1)
        self.assertEqual(result["recorded_issue_count"], 0)

    def test_empty_stream_is_clean(self) -> None:
        self._write([])
        result = self._run()
        self.assertEqual(result["recorded_issue_count"], 0)
        self.assertEqual(result["substantive_count"], 0)

    def test_tool_never_scores_the_felt_report(self) -> None:
        """Authority boundary is explicit in every result."""
        self._write([])
        result = self._run()
        self.assertIn("remains primary and unscored", result["authority_boundary"])
        self.assertIn("does not repair witnesses", result["authority_boundary"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
