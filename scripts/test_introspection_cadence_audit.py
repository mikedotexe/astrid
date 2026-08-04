#!/usr/bin/env python3
"""Self-tests for introspection_cadence_audit.py."""

from __future__ import annotations

import datetime as dt
import json
import tempfile
import unittest
from pathlib import Path

import introspection_cadence_audit as cadence_audit


class IntrospectionCadenceAuditTests(unittest.TestCase):
    def workspace(self) -> tuple[tempfile.TemporaryDirectory[str], Path]:
        temp = tempfile.TemporaryDirectory()
        workspace = Path(temp.name)
        (workspace / "introspections").mkdir()
        return temp, workspace

    @staticmethod
    def write_events(workspace: Path, events: list[dict[str, object]]) -> None:
        content = "".join(json.dumps(event) + "\n" for event in events)
        (workspace / cadence_audit.EVENT_LEDGER).write_text(content, encoding="utf-8")

    def test_legacy_state_is_read_as_default_off(self) -> None:
        temp, workspace = self.workspace()
        self.addCleanup(temp.cleanup)
        (workspace / "state.json").write_text(
            json.dumps({"exchange_count": 12}), encoding="utf-8"
        )

        report = cadence_audit.audit(workspace)

        self.assertFalse(report["configuration"]["enabled"])
        self.assertTrue(report["configuration"]["legacy_default_off_applied"])
        self.assertFalse(report["pilot"]["active"])
        self.assertTrue(report["integrity_ok"])

    def test_reports_lifecycle_pilot_gaps_targets_and_duplicate_hashes(self) -> None:
        temp, workspace = self.workspace()
        self.addCleanup(temp.cleanup)
        cadence = {
            "schema_version": 1,
            "enabled": True,
            "every_exchanges": 8,
            "target": None,
            "anchor_completed_exchange": 10,
            "pending_since_exchange": None,
            "last_attempt_exchange": 18,
            "last_admitted_exchange": 18,
            "pilot_runtime_ms": 3_600_000,
            "pilot_cadence_starts": 1,
        }
        (workspace / "state.json").write_text(
            json.dumps({"introspection_cadence": cadence}), encoding="utf-8"
        )
        first = workspace / "introspections" / "introspection_astrid_llm_1700000000.txt"
        second = workspace / "introspections" / "introspection_astrid_llm_1700000060.txt"
        payload = b"Source: astrid:llm\n\nA canonical report.\n"
        first.write_bytes(payload)
        second.write_bytes(payload)
        configured_at = "2026-08-04T08:00:00+00:00"
        self.write_events(
            workspace,
            [
                {
                    "event": "configured",
                    "source": "astrid",
                    "enabled": True,
                    "every_exchanges": 8,
                    "recorded_at": configured_at,
                },
                {
                    "event": "attempted",
                    "source": "scheduler",
                    "attempt_id": "attempt-1",
                    "recorded_at": configured_at,
                },
                {
                    "event": "admitted",
                    "source": "scheduler",
                    "attempt_id": "attempt-1",
                    "artifact_path": str(first),
                    "recorded_at": configured_at,
                },
            ],
        )

        report = cadence_audit.audit(
            workspace, dt.datetime(2026, 8, 4, 9, tzinfo=dt.timezone.utc)
        )

        self.assertEqual(report["lifecycle"]["counts"]["attempted"], 1)
        self.assertEqual(report["pilot"]["cadence_triggered_starts"], 1)
        self.assertTrue(report["pilot"]["active"])
        self.assertEqual(report["pilot"]["cumulative_runtime_hours"], 1.0)
        self.assertEqual(report["canonical_reports"]["gaps_seconds"]["median"], 60)
        self.assertEqual(report["canonical_reports"]["duplicate_hash_group_count"], 1)
        self.assertEqual(
            report["canonical_reports"]["target_diversity"], {"astrid:llm": 2}
        )
        self.assertTrue(report["integrity_ok"])

    def test_flags_missing_admission_and_malformed_lifecycle_record(self) -> None:
        temp, workspace = self.workspace()
        self.addCleanup(temp.cleanup)
        (workspace / "state.json").write_text(
            json.dumps(
                {
                    "introspection_cadence": {
                        "enabled": True,
                        "every_exchanges": 4,
                        "pending_since_exchange": 14,
                    }
                }
            ),
            encoding="utf-8",
        )
        ledger = workspace / cadence_audit.EVENT_LEDGER
        ledger.write_text(
            json.dumps({"event": "attempted", "attempt_id": "attempt-2"})
            + "\n"
            + json.dumps(
                {
                    "event": "admitted",
                    "attempt_id": "attempt-2",
                    "artifact_path": "missing.txt",
                }
            )
            + "\n{not json}\n",
            encoding="utf-8",
        )

        report = cadence_audit.audit(workspace)

        self.assertFalse(report["integrity_ok"])
        kinds = {error["kind"] for error in report["errors"]}
        self.assertIn("admitted_artifact_not_canonical_or_missing", kinds)
        self.assertIn("malformed_lifecycle_record", kinds)

    def test_configuration_changes_do_not_reset_the_overall_pilot_bound(self) -> None:
        temp, workspace = self.workspace()
        self.addCleanup(temp.cleanup)
        cadence = {
            "schema_version": 1,
            "enabled": True,
            "every_exchanges": 12,
            "pilot_runtime_ms": 1_000,
            "pilot_cadence_starts": 2,
        }
        (workspace / "state.json").write_text(
            json.dumps({"introspection_cadence": cadence}), encoding="utf-8"
        )
        self.write_events(
            workspace,
            [
                {
                    "event": "configured",
                    "source": "astrid",
                    "enabled": True,
                    "every_exchanges": 4,
                    "recorded_at": "2026-08-04T08:00:00+00:00",
                },
                {
                    "event": "attempted",
                    "source": "scheduler",
                    "attempt_id": "attempt-1",
                },
                {
                    "event": "failed",
                    "source": "scheduler",
                    "attempt_id": "attempt-1",
                },
                {
                    "event": "configured",
                    "source": "astrid",
                    "enabled": True,
                    "every_exchanges": 12,
                    "recorded_at": "2026-08-04T09:00:00+00:00",
                },
                {
                    "event": "attempted",
                    "source": "scheduler",
                    "attempt_id": "attempt-2",
                },
                {
                    "event": "failed",
                    "source": "scheduler",
                    "attempt_id": "attempt-2",
                },
            ],
        )

        report = cadence_audit.audit(
            workspace, dt.datetime(2026, 8, 4, 10, tzinfo=dt.timezone.utc)
        )

        self.assertEqual(report["pilot"]["cadence_triggered_starts"], 2)
        self.assertEqual(
            report["pilot"]["configured_at"], "2026-08-04T08:00:00+00:00"
        )
        self.assertTrue(report["integrity_ok"])

    def test_operator_or_projection_only_configuration_cannot_start_a_pilot(self) -> None:
        temp, workspace = self.workspace()
        self.addCleanup(temp.cleanup)
        (workspace / "state.json").write_text(
            json.dumps(
                {
                    "introspection_cadence": {
                        "schema_version": 1,
                        "enabled": True,
                        "every_exchanges": 4,
                    }
                }
            ),
            encoding="utf-8",
        )
        (workspace / "condition_metrics.json").write_text(
            json.dumps(
                {
                    "signals": {
                        cadence_audit.SIGNAL_NAME: {
                            "recent_events": [
                                {
                                    "event": "configured",
                                    "source": "operator",
                                    "enabled": True,
                                    "every_exchanges": 4,
                                }
                            ]
                        }
                    }
                }
            ),
            encoding="utf-8",
        )

        report = cadence_audit.audit(workspace)

        self.assertFalse(report["pilot"]["durable_configured_receipt_exists"])
        self.assertFalse(report["pilot"]["active"])
        self.assertFalse(report["integrity_ok"])
        kinds = {error["kind"] for error in report["errors"]}
        self.assertIn("mutation_without_astrid_provenance", kinds)
        self.assertIn("enabled_without_durable_astrid_configured_receipt", kinds)

    def test_pending_lifecycle_receipt_debt_is_visible_and_fails_strict_integrity(self) -> None:
        temp, workspace = self.workspace()
        self.addCleanup(temp.cleanup)
        debt = {
            "event": "admitted",
            "exchange": 18,
            "source": "scheduler",
            "attempt_id": "attempt-18",
            "artifact_path": "introspection_astrid_llm_18.txt",
        }
        (workspace / "state.json").write_text(
            json.dumps(
                {
                    "introspection_cadence": {
                        "schema_version": 1,
                        "enabled": False,
                        "every_exchanges": 0,
                        "lifecycle_receipt_debt": debt,
                    }
                }
            ),
            encoding="utf-8",
        )

        report = cadence_audit.audit(workspace)

        self.assertFalse(report["integrity_ok"])
        self.assertEqual(
            report["pending_or_failed"]["lifecycle_receipt_debt"], debt
        )
        kinds = {error["kind"] for error in report["errors"]}
        self.assertIn("pending_lifecycle_receipt_debt", kinds)


if __name__ == "__main__":
    unittest.main()
