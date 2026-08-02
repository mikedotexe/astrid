#!/usr/bin/env python3
"""Tests for owner-only edge hindsight indexing and reporting."""

from __future__ import annotations

import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parent))
import edge_hindsight as hindsight


class EdgeHindsightTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name) / "state"
        self.workspace = self.root / "home/default/edge"
        for relative in (
            "actions",
            "autonomous/recoveries",
            "autonomous/turns",
            "journal",
            "perception/observations",
            "runtime",
            "spectral",
            "tuning",
            "web",
            "introspection",
            "workshop/drafts",
        ):
            (self.workspace / relative).mkdir(parents=True, exist_ok=True)
        (self.root / "var/state.db/manifest").mkdir(parents=True)
        (self.root / "home/default/.local/audit/manifest").mkdir(parents=True)
        (self.root / "var/state.db/manifest/0.manifest").write_text("state")
        (self.root / "home/default/.local/audit/manifest/0.manifest").write_text("audit")
        os.chmod(self.root / "var/state.db/manifest/0.manifest", 0o600)
        os.chmod(self.root / "home/default/.local/audit/manifest/0.manifest", 0o600)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_jsonl(self, relative: str, values: list[dict[str, object]]) -> None:
        path = self.workspace / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("".join(json.dumps(value) + "\n" for value in values))
        path.chmod(0o600)

    def fixture(self) -> None:
        trace = {
            "schema_version": 1,
            "trace_id": "00000000-0000-4000-8000-000000000001",
            "span_id": "00000000-0000-4000-8000-000000000002",
            "session_id": "session-one",
        }
        self.write_jsonl(
            "autonomous/runs.jsonl",
            [
                {
                    "completed_at_unix_ms": 1_700_000_000_100,
                    "status": "authored_completed",
                    "session_name": "session-one",
                    "response_sha256": "response",
                    "transcript_path": "autonomous/turns/turn.md",
                    "journal_path": "journal/signal.md",
                    "declared_next": "DRAFT durable thought",
                    "trace": trace,
                }
            ],
        )
        self.write_jsonl(
            "actions/receipts.jsonl",
            [
                {
                    "recorded_at_unix_ms": 1_700_000_000_200,
                    "artifact_path": "home://edge/workshop/drafts/draft.md",
                    "declared_next": "DRAFT durable thought",
                    "decision_source": "astrid_declared",
                    "response_sha256": "response",
                    "session_id": "session-one",
                    "trace": trace,
                },
                {
                    "recorded_at_unix_ms": 1_700_000_000_300,
                    "artifact_path": "autonomous/recoveries/fallback.md",
                    "declared_next": "LISTEN",
                    "decision_source": "local_safe_fallback",
                    "response_sha256": "fallback",
                },
            ],
        )
        observation_timestamp = 1_700_000_000_400
        self.write_jsonl(
            "perception/observations.jsonl",
            [
                {
                    "recorded_at_unix_ms": observation_timestamp,
                    "summary": "machine observed",
                    "authority": "deterministic_machine_observation_not_astrid_authorship",
                    "record_sha256": "observation",
                }
            ],
        )
        for relative, content in (
            ("autonomous/turns/turn.md", "authored transcript"),
            ("journal/signal.md", "authored journal"),
            ("workshop/drafts/draft.md", "durable thought"),
            ("autonomous/recoveries/fallback.md", "transport fallback"),
            (
                f"perception/observations/observation_{observation_timestamp}.md",
                "machine observed",
            ),
        ):
            path = self.workspace / relative
            path.write_text(content)
            path.chmod(0o600)
        outside = Path(self.temporary.name) / "outside.md"
        outside.write_text("outside")
        (self.workspace / "journal/outside-link.md").symlink_to(outside)
        fill = []
        bucket_ms = 15 * 60_000
        start = 1_700_000_000_000 - 1_700_000_000_000 % bucket_ms
        for offset, value in ((1_000, 67.0), (2_000, 69.0), (bucket_ms + 1_000, 74.0)):
            fill.append(
                {
                    "recorded_at_unix_ms": start + offset,
                    "fill_pct": value,
                    "semantic_fresh": offset == 1_000,
                    "audio_fresh": True,
                    "aux_fresh": True,
                }
            )
        self.write_jsonl("runtime/fill_history.jsonl", fill)
        self.write_jsonl(
            "spectral/rollups.jsonl",
            [
                {
                    "schema": "astrid_edge_spectral_rollup_v1",
                    "recorded_at_unix_ms": 1_700_000_000_500,
                    "substrate": {
                        "kind": "cpu_edge_covariance_effective_rank",
                        "fill_metric": "normalized_covariance_effective_rank",
                    },
                    "metrics": {
                        "fill_pct": 68.0,
                        "spectral_entropy": 0.88,
                        "lambda1_share": 0.12,
                        "tail_share": 0.31,
                        "density_gradient": 0.04,
                        "mode_turnover": 0.2,
                    },
                    "activity_refs": [],
                    "activity_ref_count": 0,
                    "activity_refs_truncated": False,
                }
            ],
        )
        self.write_jsonl(
            "tuning/receipts.jsonl",
            [
                {
                    "payload": {
                        "schema": "astrid_edge_tuning_receipt_v1",
                        "recorded_at_unix_ms": 1_700_000_000_600,
                        "phase": "rolled_back",
                        "status": "completed",
                        "trace": trace,
                        "detail": {
                            "tuning_id": "tuning-1",
                            "parameter": "input_gain",
                            "requested_value": 1.05,
                        },
                        "authority": "signed_private_tuning_manager",
                    },
                    "payload_sha256": "b" * 64,
                    "signing_public_key": "c" * 64,
                    "signature": "d" * 128,
                }
            ],
        )

    def record_args(self) -> SimpleNamespace:
        return SimpleNamespace(
            workspace=self.workspace,
            state_root=self.root,
            operator_root=None,
            bucket_minutes=15,
        )

    def test_record_distinguishes_authorship_and_hash_chains_every_surface(self) -> None:
        self.fixture()
        result = hindsight.record(self.record_args())
        self.assertGreaterEqual(result["artifacts_written"], 5)
        operator = self.root / "operator/hindsight"
        artifacts = list(hindsight.json_lines(operator / "artifacts.jsonl"))
        by_path = {value["relative_path"]: value for value in artifacts}
        self.assertTrue(by_path["workshop/drafts/draft.md"]["astrid_authored"])
        self.assertEqual(
            by_path["workshop/drafts/draft.md"]["causal_attribution"],
            "exact_action_path_join",
        )
        self.assertFalse(
            by_path["autonomous/recoveries/fallback.md"]["astrid_authored"]
        )
        self.assertFalse(
            by_path[
                "perception/observations/observation_1700000000400.md"
            ]["astrid_authored"]
        )
        self.assertNotIn("journal/outside-link.md", by_path)
        for name in ("artifacts", "fill_rollups", "checkpoints"):
            verification = hindsight.verify_chain(operator / f"{name}.jsonl")
            self.assertTrue(verification["valid"], verification)
        latest = hindsight.read_json(operator / "latest.json")
        database = hindsight.hindsight_database_status(
            operator / "hindsight.sqlite3"
        )
        self.assertEqual(database["quick_check"], "ok")
        self.assertTrue(database["owner_only"])
        self.assertGreater(database["row_counts"]["activity_events"], 0)
        self.assertEqual(database["row_counts"]["spectral_rollups"], 1)
        self.assertEqual(database["row_counts"]["tuning_events"], 1)
        database_view = hindsight.query_hindsight_database(
            operator / "hindsight.sqlite3",
            1_700_000_000_000,
            1_700_000_001_000,
            20,
        )
        tuning = database_view["tuning_events"][0]
        self.assertEqual(tuning["recorded_at_unix_ms"], 1_700_000_000_600)
        self.assertEqual(tuning["tuning_id"], "tuning-1")
        self.assertEqual(
            tuning["trace"]["trace_id"],
            "00000000-0000-4000-8000-000000000001",
        )
        self.assertEqual(
            database["row_counts"]["artifact_versions"], len(artifacts)
        )
        prefix = hindsight.checkpoint_prefix_status(self.workspace, latest)
        self.assertTrue(all("verified" in value for value in prefix.values()))
        self.assertEqual(oct((operator / "latest.json").stat().st_mode & 0o777), "0o600")

    def test_second_record_versions_changes_and_report_reads_historical_range(self) -> None:
        self.fixture()
        hindsight.record(self.record_args())
        draft = self.workspace / "workshop/drafts/draft.md"
        draft.write_text("durable thought revised")
        draft.chmod(0o600)
        second = hindsight.record(self.record_args())
        self.assertEqual(second["artifacts_written"], 1)
        report = hindsight.build_report(
            SimpleNamespace(
                workspace=self.workspace,
                state_root=self.root,
                operator_root=None,
                since="1699999000000",
                until="1700001000000",
                window_minutes=60,
                limit=100,
                include_excerpts=True,
            )
        )
        self.assertTrue(report["integrity"]["checkpointed_ledger_prefixes_valid"])
        self.assertEqual(report["fill"]["summary"]["sample_count"], 3)
        self.assertEqual(report["fill"]["summary"]["fill_mean_pct"], 70.0)
        self.assertEqual(report["spectral"]["summary"]["rollup_count"], 1)
        self.assertEqual(
            report["spectral"]["summary"]["spectral_entropy"]["mean"], 0.88
        )
        self.assertEqual(len(report["spectral"]["tuning_events"]), 1)
        self.assertEqual(
            report["durable_sources"]["operator_hindsight_database"][
                "quick_check"
            ],
            "ok",
        )
        draft_record = next(
            value
            for value in report["artifacts"]
            if value["relative_path"] == "workshop/drafts/draft.md"
        )
        self.assertIn("revised", draft_record["excerpt"])
        rendered = hindsight.render_text(report)
        self.assertIn("Astrid Hindsight", rendered)
        self.assertIn("Astrid-authored memory", report["authority_note"])

    def test_checkpoint_detects_rewrite_of_append_only_prefix(self) -> None:
        self.fixture()
        hindsight.record(self.record_args())
        ledger = self.workspace / "actions/receipts.jsonl"
        ledger.write_text(ledger.read_text().replace("durable thought", "altered thought"))
        latest = hindsight.read_json(self.root / "operator/hindsight/latest.json")
        prefix = hindsight.checkpoint_prefix_status(self.workspace, latest)
        self.assertEqual(
            prefix["actions/receipts.jsonl"], "checkpointed_prefix_changed"
        )
        hindsight.record(self.record_args())
        latest = hindsight.read_json(self.root / "operator/hindsight/latest.json")
        self.assertFalse(latest["continuity_from_previous_checkpoint_valid"])
        self.assertEqual(latest["historical_ledger_integrity_violation_count"], 1)
        report = hindsight.build_report(
            SimpleNamespace(
                workspace=self.workspace,
                state_root=self.root,
                operator_root=None,
                since="1699999000000",
                until="1700001000000",
                window_minutes=60,
                limit=100,
                include_excerpts=False,
            )
        )
        self.assertFalse(report["integrity"]["overall_valid"])

    def test_interrupted_action_correction_is_non_authored_exact_attribution(self) -> None:
        self.write_jsonl(
            "actions/receipts.jsonl",
            [
                {
                    "recorded_at_unix_ms": 200,
                    "artifact_path": "home://edge/journal/journal_200.md",
                    "declared_next": "JOURNAL stale",
                    "decision_source": "astrid_declared",
                    "response_sha256": "interrupted",
                }
            ],
        )
        self.write_jsonl(
            "actions/interrupted_corrections.jsonl",
            [
                {
                    "recorded_at_unix_ms": 300,
                    "response_sha256": "interrupted",
                    "corrected_status": "revoked_interrupted_trace_non_authored",
                }
            ],
        )
        attribution = hindsight.attribution_index(self.workspace)[
            "journal/journal_200.md"
        ]
        self.assertFalse(attribution["astrid_authored"])
        self.assertEqual(
            attribution["causal_attribution"], "exact_action_correction_join"
        )
        self.assertEqual(
            attribution["authority"], "revoked_interrupted_trace_non_authored"
        )

    def test_exact_prefix_snapshot_ignores_concurrent_append(self) -> None:
        ledger = self.workspace / "runtime/fill_history.jsonl"
        initial = b'{"recorded_at_unix_ms":1700000000000,"fill_pct":68.0}\n'
        appended = b'{"recorded_at_unix_ms":1700000001000,"fill_pct":69.0}\n'
        ledger.write_bytes(initial)

        def append_after_snapshot() -> None:
            with ledger.open("ab") as handle:
                handle.write(appended)

        summary = hindsight.ledger_summary(ledger, append_after_snapshot)
        self.assertEqual(summary["size_bytes"], len(initial))
        self.assertEqual(summary["line_count"], 1)
        self.assertEqual(summary["hash_scope"], hindsight.LEDGER_HASH_SCOPE)
        self.assertEqual(summary["sha256"], hindsight.hashlib.sha256(initial).hexdigest())
        checkpoint = {
            "schema": hindsight.CHECKPOINT_SCHEMA,
            "ledgers": {"runtime/fill_history.jsonl": summary},
        }
        self.assertEqual(
            hindsight.checkpoint_prefix_status(self.workspace, checkpoint)[
                "runtime/fill_history.jsonl"
            ],
            "append_only_advance_verified",
        )

    def test_v1_checkpoint_opens_v2_epoch_without_erasing_legacy_alerts(self) -> None:
        self.fixture()
        operator = self.root / "operator/hindsight"
        operator.mkdir(parents=True)
        legacy_summary = hindsight.ledger_summary(
            self.workspace / "runtime/fill_history.jsonl"
        )
        legacy_summary.pop("hash_scope")
        legacy_summary.pop("inode")
        hindsight.owner_write_json(
            operator / "latest.json",
            {
                "schema": hindsight.LEGACY_CHECKPOINT_SCHEMA,
                "recorded_at_unix_ms": 1_700_000_000_000,
                "ledgers": {"runtime/fill_history.jsonl": legacy_summary},
            },
        )
        hindsight.owner_write_json(
            operator / "collector_state.json",
            {
                "schema": hindsight.LEGACY_STATE_SCHEMA,
                "ledger_integrity_violations": [
                    {
                        "detected_at_unix_ms": 1_700_000_000_000,
                        "statuses": {
                            "runtime/fill_history.jsonl": "checkpointed_prefix_changed"
                        },
                    }
                ],
            },
        )

        hindsight.record(self.record_args())
        baseline = hindsight.read_json(operator / "latest.json")
        self.assertEqual(baseline["schema"], hindsight.CHECKPOINT_SCHEMA)
        self.assertEqual(
            baseline["continuity_status"],
            "migration_baseline_no_prior_continuity_claim",
        )
        self.assertIsNone(baseline["continuity_from_previous_checkpoint_valid"])
        self.assertEqual(
            baseline["legacy_race_compatible_unresolved_violation_count"], 1
        )
        self.assertEqual(baseline["current_epoch_integrity_violation_count"], 0)

        hindsight.record(self.record_args())
        verified = hindsight.read_json(operator / "latest.json")
        self.assertEqual(verified["continuity_status"], "verified")
        self.assertTrue(verified["continuity_from_previous_checkpoint_valid"])
        self.assertEqual(verified["current_epoch_integrity_violation_count"], 0)


if __name__ == "__main__":
    unittest.main()
