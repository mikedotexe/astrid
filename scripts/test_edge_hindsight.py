#!/usr/bin/env python3
"""Tests for owner-only edge hindsight indexing and reporting."""

from __future__ import annotations

import hashlib
import json
import os
import sqlite3
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import edge_hindsight as hindsight

VERIFIED_TRAIN_SOURCE = b"TRAIN_MARKER = 'verified-root-train'\n"


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

    def write_self_change_ledger(
        self, name: str, values: list[tuple[str, dict[str, object]]]
    ) -> None:
        previous = "0" * 64
        records: list[dict[str, object]] = []
        for sequence, (kind, payload) in enumerate(values, 1):
            core = {
                "sequence": sequence,
                "previous_hash": previous,
                "event_id": f"{name}-{sequence}",
                "kind": kind,
                "recorded_at": 1_700_000_000 + sequence,
                "payload": payload,
            }
            record_hash = hashlib.sha256(
                json.dumps(core, sort_keys=True, separators=(",", ":")).encode()
            ).hexdigest()
            records.append(
                {
                    "schema": "astrid.edge_self_change.ledger_record.v1",
                    "ledger": name,
                    "core": core,
                    "record_hash": record_hash,
                    "auth": {
                        "algorithm": "hmac-sha256",
                        "key_id": "test-key",
                        "signature": "0" * 64,
                    },
                }
            )
            previous = record_hash
        path = self.root / "self-change/ledgers" / f"{name}.jsonl"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("".join(json.dumps(value) + "\n" for value in records))
        path.chmod(0o600)

    @staticmethod
    def operator_event(**overrides: object) -> dict[str, object]:
        value: dict[str, object] = {
            "schema": "astrid.edge_self_change.operator_lifecycle_event.v1",
            "recorded_at": 1_700_000_001,
            "source_ledger": "candidate",
            "sequence": 1,
            "event_id": "candidate-1",
            "status": "candidate_recorded",
            "facets": ["candidate"],
            "record_sha256": "1" * 64,
            "candidate_id": "candidate-one",
            "candidate_sha256": "2" * 64,
            "build_id": None,
            "generation_id": None,
            "from_generation": None,
            "trace_id": None,
            "session_id": None,
            "turn_id": None,
            "response_sha256": None,
            "terminal_declaration_sha256": None,
            "terminal_reason_sha256": None,
            "terminal_authority": None,
            "automatic_retry": None,
            "tests_sha256": None,
            "bundle_sha256": None,
            "manifest_sha256": None,
            "invariant_candidate_replay_sha256": None,
            "invariant_package_replay_sha256": None,
            "shadow_evidence_sha256": None,
            "shadow_status": None,
            "command_profile": None,
            "command_executable_sha256": None,
            "command_argv_sha256": None,
            "command_stdout_sha256": None,
            "command_stderr_sha256": None,
            "command_exit_code": None,
            "command_timed_out": None,
            "provenance": "immutable_supervisor_signed_ledger_sanitized_metadata",
            "authority": "observation_only_not_deployment_or_astrid_authorship",
            "authored": False,
            "fallback": False,
        }
        value.update(overrides)
        return value

    def write_operator_projection(
        self,
        events: list[dict[str, object]],
        *,
        total: int | None = None,
    ) -> Path:
        root = self.root / "self-change"
        root.mkdir(parents=True, exist_ok=True)
        root.chmod(0o2750)
        heads = {name: None for name in ("candidate", "build", "activation", "operator")}
        for event in events:
            heads[str(event["source_ledger"])] = event["record_sha256"]
        total = len(events) if total is None else total
        core = {
            "schema": "astrid.edge_self_change.operator_status.v3",
            "appliance_id": "avado-test",
            "generated_at": 1_700_000_010,
            "state_revision": 7,
            "mode": "running",
            "active_generation": "generation-one",
            "previous_generation": "generation-zero",
            "pipeline_phase": "idle",
            "latest_transition": {"operation": "supervise", "status": "completed"},
            "restart_expectation": {
                "phase": "none",
                "maximum_seconds": 0,
                "basis": "immutable_command_profile_timeout_upper_bound",
            },
            "lifecycle": {
                "schema": "astrid.edge_self_change.operator_lifecycle.v1",
                "events": events,
                "included": len(events),
                "total": total,
                "truncated": total > len(events),
                "maximum_events": 64,
                "ledger_heads": heads,
            },
            "provenance": "immutable_supervisor_sanitized_projection",
            "authority": "observation_only_not_deployment_authority",
        }
        encoded = json.dumps(
            core,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        ).encode("ascii")
        path = root / "operator-status.json"
        path.write_text(
            json.dumps(
                {
                    "schema": "astrid.edge_self_change.operator_status_envelope.v1",
                    "core": core,
                    "core_sha256": hashlib.sha256(encoded).hexdigest(),
                }
            ),
            encoding="utf-8",
        )
        path.chmod(0o640)
        return path

    def record_with_operator_projection(self, path: Path) -> dict[str, object]:
        activity_module = hindsight.load_activity_module()
        collect = activity_module.collect_events
        activity_module.collect_events = lambda workspace, current_ms: collect(
            workspace,
            current_ms,
            path,
            test_only_allow_unprivileged_operator_status=True,
        )
        with mock.patch.object(
            hindsight, "load_activity_module", return_value=activity_module
        ):
            return hindsight.record(self.record_args())

    def fixture(self) -> None:
        trace = {
            "schema_version": 1,
            "trace_id": "00000000-0000-4000-8000-000000000001",
            "span_id": "00000000-0000-4000-8000-000000000002",
            "turn_id": "00000000-0000-4000-8000-000000000003",
            "session_id": "session-one",
        }
        self.write_jsonl(
            "autonomous/runs.jsonl",
            [
                {
                    "completed_at_unix_ms": 1_700_000_000_100,
                    "status": "authored_completed",
                    "session_name": "session-one",
                    "response_sha256": "a" * 64,
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
                    "response_sha256": "a" * 64,
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
            "spectral/receipts.jsonl",
            [
                {
                    "schema": "astrid_edge_spectral_receipt_v1",
                    "recorded_at_unix_ms": 1_700_000_000_550,
                    "phase": "completed",
                    "status": "success",
                    "event_kind": "read_spectral_now",
                    "trace": trace,
                    "authority": (
                        "deterministic_machine_derivation_not_authorship_or_causal_proof"
                    ),
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
                            "authority_turn_id": trace["turn_id"],
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

    def sealed_writer_config(self) -> dict[str, object]:
        state_root = Path("/var/lib/astrid-edge-runtime")
        return {
            "schema": hindsight.SEALED_WRITER_CONFIG_SCHEMA,
            "appliance_id": "avado-i3",
            "workspace": str(state_root / "home/default/edge"),
            "state_root": str(state_root),
            "operator_root": str(state_root / "operator/hindsight"),
            "bucket_minutes": hindsight.DEFAULT_BUCKET_MINUTES,
            "writer_path": "/usr/libexec/astrid-edge/immutable/edge_hindsight.py",
            "writer_sha256": "a" * 64,
            "activity_report_path": str(
                hindsight.IMMUTABLE_ACTIVITY_REPORT_PATH
            ),
            "activity_report_sha256": "b" * 64,
            "operator_report_manifest_path": str(
                hindsight.IMMUTABLE_OPERATOR_REPORT_MANIFEST_PATH
            ),
            "operator_report_manifest_sha256": "c" * 64,
        }

    def test_sealed_writer_config_has_exact_root_authored_bounds(self) -> None:
        config = self.sealed_writer_config()
        parsed = hindsight.parse_sealed_writer_config(config)
        self.assertEqual(parsed["appliance_id"], "avado-i3")
        self.assertEqual(
            parsed["workspace"], Path("/var/lib/astrid-edge-runtime/home/default/edge")
        )
        self.assertEqual(
            parsed["operator_root"],
            Path("/var/lib/astrid-edge-runtime/operator/hindsight"),
        )

        invalid_values = (
            {**config, "unexpected": True},
            {**config, "schema": "astrid.edge.hindsight_writer.config.v1"},
            {**config, "appliance_id": "../peer"},
            {**config, "bucket_minutes": True},
            {**config, "bucket_minutes": 30},
            {**config, "workspace": "/var/lib/peer/home/default/edge"},
            {**config, "operator_root": "/var/lib/peer/operator/hindsight"},
            {**config, "writer_path": "relative/edge_hindsight.py"},
            {**config, "writer_path": "/usr/libexec/../bin/edge_hindsight.py"},
            {**config, "writer_sha256": "A" * 64},
            {**config, "writer_sha256": "a" * 63},
            {
                **config,
                "activity_report_path": "/var/lib/astrid-edge-runtime/bin/report_edge_activity.py",
            },
            {**config, "activity_report_sha256": "B" * 64},
            {**config, "activity_report_sha256": "b" * 63},
            {
                **config,
                "operator_report_manifest_path": "/tmp/MANIFEST.sha256",
            },
            {**config, "operator_report_manifest_sha256": "C" * 64},
            {**config, "operator_report_manifest_sha256": "c" * 63},
        )
        for invalid in invalid_values:
            with self.subTest(invalid=invalid):
                with self.assertRaises(ValueError):
                    hindsight.parse_sealed_writer_config(invalid)

    def test_record_sealed_uses_only_verified_configured_roots(self) -> None:
        configured = hindsight.parse_sealed_writer_config(self.sealed_writer_config())
        sentinel = {
            "artifacts_written": 0,
            "fill_rollups_written": 0,
            "operator_root": str(configured["operator_root"]),
        }
        with (
            mock.patch.object(
                hindsight, "load_sealed_writer_config", return_value=configured
            ) as loader,
            mock.patch.object(
                hindsight,
                "load_verified_activity_module",
                return_value=object(),
            ) as activity_loader,
            mock.patch.object(
                hindsight, "exclusive_collector_lock"
            ) as collector_lock,
            mock.patch.object(
                hindsight, "record_locked", return_value=sentinel
            ) as recorder,
        ):
            result = hindsight.record_sealed(Path("/immutable/config.json"))

        self.assertIs(result, sentinel)
        loader.assert_called_once_with(Path("/immutable/config.json"))
        activity_loader.assert_called_once_with(configured)
        collector_lock.assert_called_once_with(configured["operator_root"])
        arguments = recorder.call_args.args[0]
        self.assertEqual(arguments.workspace, configured["workspace"])
        self.assertEqual(arguments.state_root, configured["state_root"])
        self.assertEqual(arguments.operator_root, configured["operator_root"])
        self.assertEqual(arguments.bucket_minutes, hindsight.DEFAULT_BUCKET_MINUTES)
        self.assertIs(recorder.call_args.args[1], activity_loader.return_value)

    def test_sealed_writer_loader_binds_canonical_config_to_running_digest(self) -> None:
        writer_path = Path(hindsight.__file__).resolve()
        writer = writer_path.read_bytes()
        value = {
            **self.sealed_writer_config(),
            "writer_path": str(writer_path),
            "writer_sha256": hashlib.sha256(writer).hexdigest(),
        }
        payload = hindsight.canonical_bytes(value) + b"\n"
        with mock.patch.object(
            hindsight, "stable_root_read", side_effect=(payload, writer)
        ):
            loaded = hindsight.load_sealed_writer_config(Path("/immutable/config.json"))
        self.assertEqual(loaded["writer_path"], writer_path)

        value["writer_sha256"] = "0" * 64
        payload = hindsight.canonical_bytes(value)
        with (
            mock.patch.object(
                hindsight, "stable_root_read", side_effect=(payload, writer)
            ),
            self.assertRaisesRegex(ValueError, "writer digest mismatch"),
        ):
            hindsight.load_sealed_writer_config(Path("/immutable/config.json"))

        with (
            mock.patch.object(hindsight, "stable_root_read", return_value=b"{} \n"),
            self.assertRaisesRegex(ValueError, "not canonical"),
        ):
            hindsight.load_sealed_writer_config(Path("/immutable/config.json"))

    def verified_activity_fixture(
        self, source: bytes = b"MARKER = 'verified-root-report'\n"
    ) -> tuple[dict[str, object], bytes, bytes]:
        source_sha256 = hashlib.sha256(source).hexdigest()
        train_sha256 = hashlib.sha256(VERIFIED_TRAIN_SOURCE).hexdigest()
        manifest = (
            f"{train_sha256}  {hindsight.IMMUTABLE_TRAIN_REPORT_PATH}\n"
            f"{source_sha256}  {hindsight.IMMUTABLE_ACTIVITY_REPORT_PATH}\n"
        ).encode()
        config = hindsight.parse_sealed_writer_config(
            {
                **self.sealed_writer_config(),
                "activity_report_sha256": source_sha256,
                "operator_report_manifest_sha256": hashlib.sha256(
                    manifest
                ).hexdigest(),
            }
        )
        return config, source, manifest

    def test_sealed_activity_report_executes_only_manifest_bound_bytes(self) -> None:
        config, source, manifest = self.verified_activity_fixture()

        def stable_read(
            path: Path, *, expected_mode: int, maximum_bytes: int
        ) -> bytes:
            self.assertEqual(expected_mode, 0o444)
            if path == hindsight.IMMUTABLE_ACTIVITY_REPORT_PATH:
                self.assertEqual(maximum_bytes, hindsight.MAX_ACTIVITY_REPORT_BYTES)
                return source
            if path == hindsight.IMMUTABLE_TRAIN_REPORT_PATH:
                self.assertEqual(maximum_bytes, hindsight.MAX_TRAIN_REPORT_BYTES)
                return VERIFIED_TRAIN_SOURCE
            self.assertEqual(
                path, hindsight.IMMUTABLE_OPERATOR_REPORT_MANIFEST_PATH
            )
            self.assertEqual(
                maximum_bytes, hindsight.MAX_OPERATOR_REPORT_MANIFEST_BYTES
            )
            return manifest

        with mock.patch.object(
            hindsight, "stable_root_read", side_effect=stable_read
        ):
            module = hindsight.load_verified_activity_module(config)

        self.assertEqual(module.MARKER, "verified-root-report")
        self.assertEqual(
            module.__file__, str(hindsight.IMMUTABLE_ACTIVITY_REPORT_PATH)
        )
        self.assertEqual(
            module._SEALED_TRAIN_MODULE.TRAIN_MARKER, "verified-root-train"
        )

    def test_sealed_activity_report_rejects_source_or_manifest_drift(self) -> None:
        config, source, manifest = self.verified_activity_fixture()
        cases = (
            (
                b"MARKER = 'tampered'\n",
                manifest,
                "activity report digest mismatch",
            ),
            (
                source,
                manifest + b"# drift\n",
                "operator manifest digest mismatch",
            ),
            (
                source,
                (
                    f"{'0' * 64}  {hindsight.IMMUTABLE_ACTIVITY_REPORT_PATH}\n"
                ).encode(),
                "activity report manifest binding mismatch",
            ),
        )
        for observed_source, observed_manifest, message in cases:
            with self.subTest(message=message):
                adjusted = dict(config)
                if message == "activity report manifest binding mismatch":
                    adjusted["operator_report_manifest_sha256"] = hashlib.sha256(
                        observed_manifest
                    ).hexdigest()
                with (
                    mock.patch.object(
                        hindsight,
                        "stable_root_read",
                        side_effect=(observed_source, observed_manifest),
                    ),
                    self.assertRaisesRegex(ValueError, message),
                ):
                    hindsight.load_verified_activity_module(adjusted)

    def test_sealed_activity_module_rejects_loaded_identity_mutation(self) -> None:
        source = b"__file__ = '/tmp/substituted.py'\n"
        config, _, manifest = self.verified_activity_fixture(source)
        with (
            mock.patch.object(
                hindsight,
                "stable_root_read",
                side_effect=(source, manifest, VERIFIED_TRAIN_SOURCE),
            ),
            self.assertRaisesRegex(ValueError, "identity changed during load"),
        ):
            hindsight.load_verified_activity_module(config)

    def test_sealed_activity_report_rejects_unbound_or_drifting_train_dependency(self) -> None:
        config, source, manifest = self.verified_activity_fixture()
        missing_train_manifest = (
            f"{hashlib.sha256(source).hexdigest()}  "
            f"{hindsight.IMMUTABLE_ACTIVITY_REPORT_PATH}\n"
        ).encode()
        missing_config = {
            **config,
            "operator_report_manifest_sha256": hashlib.sha256(
                missing_train_manifest
            ).hexdigest(),
        }
        with (
            mock.patch.object(
                hindsight,
                "stable_root_read",
                side_effect=(source, missing_train_manifest),
            ),
            self.assertRaisesRegex(ValueError, "train dependency manifest binding is absent"),
        ):
            hindsight.load_verified_activity_module(missing_config)

        with (
            mock.patch.object(
                hindsight,
                "stable_root_read",
                side_effect=(source, manifest, b"TRAIN_MARKER = 'tampered'\n"),
            ),
            self.assertRaisesRegex(ValueError, "train dependency digest mismatch"),
        ):
            hindsight.load_verified_activity_module(config)

    def test_operator_manifest_rejects_malformed_and_duplicate_bindings(self) -> None:
        path = hindsight.IMMUTABLE_ACTIVITY_REPORT_PATH
        digest = "a" * 64
        invalid = (
            b"",
            f"{digest}  {path}".encode(),
            f"{digest} *{path}\n".encode(),
            f"{digest}  /usr/libexec/../tmp/report.py\n".encode(),
            f"{digest}  {path}\n{digest}  {path}\n".encode(),
            b"\xff\n",
        )
        for value in invalid:
            with self.subTest(value=value):
                with self.assertRaises(ValueError):
                    hindsight.parse_operator_report_manifest(value)

    def test_unsealed_activity_loader_never_falls_back_to_state_root(self) -> None:
        immutable_writer = Path(self.temporary.name) / "immutable/edge_hindsight.py"
        mutable_report = Path(self.temporary.name) / "state/bin/report_edge_activity.py"
        immutable_writer.parent.mkdir(parents=True)
        mutable_report.parent.mkdir(parents=True)
        immutable_writer.write_text("# writer\n")
        mutable_report.write_text("MUTABLE_FALLBACK = True\n")

        with mock.patch.object(hindsight, "__file__", str(immutable_writer)):
            self.assertIsNone(hindsight.load_activity_module())

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
        self.assertEqual(
            database["attribution_projection_version"],
            hindsight.ATTRIBUTION_PROJECTION_VERSION,
        )
        self.assertGreater(database["row_counts"]["activity_events"], 0)
        self.assertEqual(database["row_counts"]["spectral_rollups"], 1)
        self.assertEqual(database["row_counts"]["spectral_receipts"], 1)
        self.assertEqual(database["row_counts"]["tuning_events"], 1)
        database_view = hindsight.query_hindsight_database(
            operator / "hindsight.sqlite3",
            1_700_000_000_000,
            1_700_000_001_000,
            20,
        )
        tuning = database_view["tuning_events"][0]
        spectral_receipt = database_view["spectral_receipts"][0]
        self.assertEqual(spectral_receipt["event_kind"], "read_spectral_now")
        self.assertEqual(tuning["recorded_at_unix_ms"], 1_700_000_000_600)
        self.assertEqual(tuning["tuning_id"], "tuning-1")
        self.assertEqual(
            tuning["trace"]["trace_id"],
            "00000000-0000-4000-8000-000000000001",
        )
        self.assertEqual(
            tuning["trace"]["turn_id"],
            "00000000-0000-4000-8000-000000000003",
        )
        connection = sqlite3.connect(operator / "hindsight.sqlite3")
        try:
            activity_turn = connection.execute(
                "SELECT turn_id FROM activity_events "
                "WHERE turn_id IS NOT NULL LIMIT 1"
            ).fetchone()
            tuning_integrity = connection.execute(
                "SELECT turn_id, authority_turn_id, payload_hash_valid, "
                "signature_present_not_verified FROM tuning_events"
            ).fetchone()
        finally:
            connection.close()
        expected_turn_id = "00000000-0000-4000-8000-000000000003"
        self.assertEqual(activity_turn, (expected_turn_id,))
        self.assertEqual(
            tuning_integrity,
            (expected_turn_id, expected_turn_id, 0, 1),
        )
        self.assertEqual(
            database["row_counts"]["artifact_versions"], len(artifacts)
        )
        prefix = hindsight.checkpoint_prefix_status(self.workspace, latest)
        self.assertTrue(all("verified" in value for value in prefix.values()))
        self.assertEqual(oct((operator / "latest.json").stat().st_mode & 0o777), "0o600")

    def test_transport_correction_reclassifies_activity_and_artifacts(self) -> None:
        self.fixture()
        self.write_jsonl(
            "autonomous/authorship_corrections.jsonl",
            [
                {
                    "schema": "astrid_edge_authorship_correction_v2",
                    "recorded_at_unix_ms": 1_700_000_000_700,
                    "original_transcript_path": "autonomous/turns/turn.md",
                    "response_sha256": "a" * 64,
                    "reason": "legacy_transport_sentinel_reclassified_non_authored",
                    "authority": (
                        "deterministic_provenance_correction_no_model_or_action_invocation"
                    ),
                }
            ],
        )

        attribution = hindsight.attribution_index(self.workspace)
        self.assertFalse(
            attribution["autonomous/turns/turn.md"]["astrid_authored"]
        )
        self.assertFalse(
            attribution["workshop/drafts/draft.md"]["astrid_authored"]
        )
        self.assertEqual(
            attribution["workshop/drafts/draft.md"]["causal_attribution"],
            "exact_action_correction_join",
        )

        hindsight.record(self.record_args())
        database = hindsight.query_hindsight_database(
            self.root / "operator/hindsight/hindsight.sqlite3",
            1_700_000_000_000,
            1_700_000_001_000,
            100,
        )
        turn = next(value for value in database["activity"] if value["kind"] == "turn")
        action = next(
            value for value in database["activity"] if value["kind"] == "action"
        )
        self.assertFalse(turn["authored"])
        self.assertTrue(turn["fallback"])
        self.assertEqual(turn["status"], "transport_recovery")
        self.assertFalse(action["authored"])
        self.assertTrue(action["fallback"])

    def test_interrupted_correction_uses_exact_trace_and_ignores_v1(self) -> None:
        self.fixture()
        response_hash = "a" * 64
        second_trace = {
            "schema_version": 1,
            "trace_id": "00000000-0000-4000-8000-000000000011",
            "span_id": "00000000-0000-4000-8000-000000000012",
            "turn_id": "00000000-0000-4000-8000-000000000013",
            "session_id": "session-one",
        }
        actions = list(hindsight.json_lines(self.workspace / "actions/receipts.jsonl"))
        actions.append(
            {
                "recorded_at_unix_ms": 1_700_000_000_250,
                "artifact_path": "home://edge/workshop/drafts/same-text.md",
                "declared_next": "DRAFT same response bytes on another turn",
                "decision_source": "astrid_declared",
                "response_sha256": response_hash,
                "session_id": "session-one",
                "trace": second_trace,
            }
        )
        self.write_jsonl("actions/receipts.jsonl", actions)
        (self.workspace / "workshop/drafts/same-text.md").write_text(
            "independently authored on another turn"
        )
        self.write_jsonl(
            "actions/interrupted_corrections.jsonl",
            [
                {
                    "schema": "astrid_edge_interrupted_action_correction_v2",
                    "recorded_at_unix_ms": 1_700_000_000_700,
                    "trace_id": "00000000-0000-4000-8000-000000000001",
                    "turn_id": "00000000-0000-4000-8000-000000000003",
                    "response_sha256": response_hash,
                    "corrected_status": "revoked_interrupted_trace_non_authored",
                },
                {
                    "schema": "astrid_edge_interrupted_action_correction_v1",
                    "recorded_at_unix_ms": 1_700_000_000_701,
                    "trace_id": second_trace["trace_id"],
                    "turn_id": second_trace["turn_id"],
                    "response_sha256": response_hash,
                    "corrected_status": "revoked_interrupted_trace_non_authored",
                },
            ],
        )

        attribution = hindsight.attribution_index(self.workspace)
        self.assertFalse(
            attribution["workshop/drafts/draft.md"]["astrid_authored"]
        )
        self.assertEqual(
            attribution["workshop/drafts/draft.md"]["authority"],
            "revoked_interrupted_trace_non_authored",
        )
        self.assertTrue(
            attribution["workshop/drafts/same-text.md"]["astrid_authored"]
        )
        self.assertEqual(
            attribution["workshop/drafts/same-text.md"]["causal_attribution"],
            "exact_action_path_join",
        )

    def test_checkpoint_and_latest_bind_the_current_host_boot_id(self) -> None:
        self.fixture()
        boot_id = "00000000-0000-4000-8000-000000000099"
        with mock.patch.object(
            hindsight, "read_host_boot_id", return_value=boot_id
        ):
            hindsight.record(self.record_args())

        operator = self.root / "operator/hindsight"
        latest = hindsight.read_json(operator / "latest.json")
        checkpoint = list(hindsight.json_lines(operator / "checkpoints.jsonl"))[-1]
        self.assertEqual(latest["host_boot_id"], boot_id)
        self.assertEqual(checkpoint["host_boot_id"], boot_id)

    def test_trace_summary_carries_turn_id_and_rejects_ambiguous_identity(self) -> None:
        valid = {
            "trace": {
                "schema_version": 1,
                "trace_id": "00000000-0000-4000-8000-000000000001",
                "span_id": "00000000-0000-4000-8000-000000000002",
                "turn_id": "00000000-0000-4000-8000-000000000003",
                "session_id": "session-one",
            }
        }
        self.assertEqual(
            hindsight.trace_summary(valid)["turn_id"],
            "00000000-0000-4000-8000-000000000003",
        )
        invalid_nil_turn = {
            "trace": {
                **valid["trace"],
                "turn_id": "00000000-0000-0000-0000-000000000000",
            }
        }
        invalid_session = {
            "trace": {**valid["trace"], "session_id": "session\nspoof"}
        }
        invalid_self_parent = {
            "trace": {
                **valid["trace"],
                "parent_span_id": valid["trace"]["span_id"],
            }
        }
        invalid_blank_session = {
            "trace": {**valid["trace"], "session_id": "   "}
        }
        self.assertIsNone(hindsight.trace_summary(invalid_nil_turn))
        self.assertIsNone(hindsight.trace_summary(invalid_session))
        self.assertIsNone(hindsight.trace_summary(invalid_self_parent))
        self.assertIsNone(hindsight.trace_summary(invalid_blank_session))

    def test_projection_upgrade_removes_stale_false_authorship_rows(self) -> None:
        self.fixture()
        hindsight.record(self.record_args())
        operator = self.root / "operator/hindsight"
        database_path = operator / "hindsight.sqlite3"
        stale_payload = json.dumps(
            {
                "timestamp_unix_ms": 1_700_000_000_050,
                "kind": "turn",
                "status": "stale_false_authorship",
                "authored": True,
                "fallback": False,
                "source_ledger": "autonomous/runs.jsonl",
            },
            sort_keys=True,
            separators=(",", ":"),
        )
        connection = sqlite3.connect(database_path)
        try:
            connection.execute(
                "UPDATE metadata SET value = '1' "
                "WHERE key = 'attribution_projection_version'"
            )
            connection.execute(
                """
                INSERT INTO activity_events(
                    event_id, timestamp_unix_ms, kind, authored, fallback,
                    trace_id, session_id, chain_id, status, declared_next,
                    source_ledger, payload_json
                ) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    "stale-event",
                    1_700_000_000_050,
                    "turn",
                    1,
                    0,
                    None,
                    None,
                    None,
                    "stale_false_authorship",
                    None,
                    "autonomous/runs.jsonl",
                    stale_payload,
                ),
            )
            connection.commit()
        finally:
            connection.close()

        hindsight.sync_hindsight_database(
            operator, self.workspace, 1_700_000_001_000
        )
        view = hindsight.query_hindsight_database(
            database_path,
            1_700_000_000_000,
            1_700_000_002_000,
            100,
        )
        self.assertNotIn(
            "stale_false_authorship",
            {value.get("status") for value in view["activity"]},
        )
        connection = sqlite3.connect(database_path)
        try:
            projection = connection.execute(
                "SELECT value FROM metadata "
                "WHERE key = 'attribution_projection_version'"
            ).fetchone()
        finally:
            connection.close()
        self.assertEqual(
            projection, (str(hindsight.ATTRIBUTION_PROJECTION_VERSION),)
        )

    def test_installed_projection_adds_new_columns_in_place(self) -> None:
        database_path = Path(self.temporary.name) / "legacy.sqlite3"
        connection = sqlite3.connect(database_path)
        try:
            connection.executescript(
                """
                CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                INSERT INTO metadata(key, value) VALUES('schema_version', '3');
                CREATE TABLE activity_events (
                    event_id TEXT PRIMARY KEY,
                    timestamp_unix_ms INTEGER NOT NULL,
                    kind TEXT NOT NULL,
                    authored INTEGER,
                    fallback INTEGER,
                    trace_id TEXT,
                    session_id TEXT,
                    chain_id TEXT,
                    status TEXT,
                    declared_next TEXT,
                    source_ledger TEXT NOT NULL,
                    payload_json TEXT NOT NULL
                );
                CREATE TABLE tuning_events (
                    event_id TEXT PRIMARY KEY,
                    recorded_at_unix_ms INTEGER NOT NULL,
                    tuning_id TEXT,
                    candidate_id TEXT,
                    phase TEXT NOT NULL,
                    status TEXT,
                    parameter TEXT,
                    requested_value REAL,
                    trace_id TEXT,
                    session_id TEXT,
                    chain_id TEXT,
                    response_sha256 TEXT,
                    payload_json TEXT NOT NULL
                );
                CREATE TABLE self_change_events (
                    event_id TEXT PRIMARY KEY,
                    recorded_at_unix_ms INTEGER NOT NULL,
                    lifecycle_kind TEXT NOT NULL,
                    lifecycle_facets_json TEXT NOT NULL,
                    status TEXT,
                    candidate_id TEXT,
                    candidate_digest TEXT,
                    build_id TEXT,
                    generation_id TEXT,
                    from_generation TEXT,
                    tests_sha256 TEXT,
                    bundle_sha256 TEXT,
                    shadow_evidence_sha256 TEXT,
                    shadow_evidence_in_tests_bundle INTEGER NOT NULL,
                    manifest_sha256 TEXT,
                    record_sha256 TEXT,
                    response_sha256 TEXT,
                    terminal_declaration_sha256 TEXT,
                    trace_id TEXT,
                    session_id TEXT,
                    chain_id TEXT,
                    turn_id TEXT,
                    source_ledger TEXT NOT NULL,
                    authority TEXT,
                    integrity TEXT,
                    metadata_json TEXT NOT NULL
                );
                """
            )
            hindsight.prepare_hindsight_database(connection)
            activity_columns = {
                row[1]
                for row in connection.execute(
                    "PRAGMA table_info(activity_events)"
                ).fetchall()
            }
            tuning_columns = {
                row[1]
                for row in connection.execute(
                    "PRAGMA table_info(tuning_events)"
                ).fetchall()
            }
            self_change_columns = {
                row[1]
                for row in connection.execute(
                    "PRAGMA table_info(self_change_events)"
                ).fetchall()
            }
            schema_version = connection.execute(
                "SELECT value FROM metadata WHERE key = 'schema_version'"
            ).fetchone()
        finally:
            connection.close()

        self.assertIn("turn_id", activity_columns)
        self.assertTrue(
            {
                "turn_id",
                "authority_turn_id",
                "payload_sha256",
                "payload_hash_valid",
                "signature_present_not_verified",
            }.issubset(tuning_columns)
        )
        self.assertTrue(
            {
                "package_replay_sha256_present",
                "shadow_gate_evidence",
            }.issubset(self_change_columns)
        )
        self.assertEqual(schema_version, (str(hindsight.DATABASE_SCHEMA_VERSION),))

    def test_v6_database_projects_inquiry_evidence_beliefs_and_acknowledgments(self) -> None:
        operator = self.root / "operator/hindsight"
        operator.mkdir(parents=True)
        timestamp = 1_700_000_000_100
        common = {
            "trace_id": "00000000-0000-4000-8000-000000000001",
            "turn_id": "00000000-0000-4000-8000-000000000002",
            "session_id": "scheduled-session",
            "source_ledger": "immutable-steward/inquiry/segments",
        }
        events = [
            {
                **common,
                "timestamp_unix_ms": timestamp,
                "kind": "inquiry_step",
                "status": "verified",
                "authored": True,
                "fallback": False,
                "step_id": "inquiry-step-one",
                "signed_entry_id": "inquiry-entry-one",
                "thread_id": "thread-one",
                "parent_step_id": None,
                "thread_operation": "open",
                "confidence": "tentative",
                "belief_operation": "propose",
                "belief_id": "belief-one",
                "response_sha256": "a" * 64,
                "declaration_sha256": "b" * 64,
                "entry_sha256": "c" * 64,
                "train_integrity": "full_signed_hash_chain_verified",
            },
            {
                **common,
                "timestamp_unix_ms": timestamp + 1,
                "kind": "evidence_arrival",
                "evidence_id": "evidence-one",
                "thread_id": "thread-one",
                "evidence_kind": "verified_source",
                "status": "verified",
                "eligible_for_belief_update": True,
                "sha256": "d" * 64,
            },
            {
                **common,
                "timestamp_unix_ms": timestamp + 2,
                "kind": "belief_revision",
                "revision_id": "belief-revision-one",
                "belief_id": "belief-one",
                "thread_id": "thread-one",
                "operation": "support",
                "evidence_ids": ["evidence-one"],
                "prior_revision_id": None,
                "response_sha256": "e" * 64,
                "source": "scheduled_inquiry",
            },
            {
                **common,
                "timestamp_unix_ms": timestamp + 3,
                "kind": "thread_transition",
                "thread_id": "thread-one",
                "step_id": "inquiry-step-one",
                "parent_step_id": None,
                "status": "open",
            },
            {
                **common,
                "timestamp_unix_ms": timestamp + 4,
                "kind": "semantic_admission",
                "admission_id": "inquiry-admission-one",
                "signed_entry_id": "inquiry-entry-one",
                "status": "acknowledged",
                "source_class": "scheduled_inquiry",
                "reservoir_generation": "reservoir-one",
                "reservoir_sequence": 8,
                "vector_sha256": "f" * 64,
            },
        ]
        module = SimpleNamespace(collect_events=lambda _workspace, _now: events)
        result = hindsight.sync_hindsight_database(
            operator, self.workspace, timestamp + 10, module
        )
        self.assertEqual(result["schema_version"], 6)
        self.assertEqual(result["row_counts"]["inquiry_steps"], 1)
        self.assertEqual(result["row_counts"]["inquiry_evidence"], 1)
        self.assertEqual(result["row_counts"]["inquiry_belief_revisions"], 1)
        self.assertEqual(result["row_counts"]["semantic_admissions"], 1)
        view = hindsight.query_hindsight_database(
            operator / "hindsight.sqlite3", timestamp - 1, timestamp + 20, 20
        )
        self.assertEqual(view["inquiry_counts"]["steps"], 1)
        self.assertEqual(
            view["inquiry"]["semantic_admissions"][0]["status"],
            "acknowledged",
        )

    def test_spectral_projection_upgrade_removes_mixed_tuning_rows(self) -> None:
        self.fixture()
        hindsight.record(self.record_args())
        operator = self.root / "operator/hindsight"
        database_path = operator / "hindsight.sqlite3"
        connection = sqlite3.connect(database_path)
        try:
            connection.execute(
                "UPDATE metadata SET value = '1' "
                "WHERE key = 'spectral_tuning_projection_version'"
            )
            connection.execute(
                """
                INSERT INTO tuning_events(
                    event_id, recorded_at_unix_ms, phase, payload_json
                ) VALUES(?, ?, ?, ?)
                """,
                (
                    "legacy-spectral-misclassified",
                    1_700_000_000_575,
                    "read_spectral_now",
                    json.dumps(
                        {
                            "recorded_at_unix_ms": 1_700_000_000_575,
                            "phase": "read_spectral_now",
                            "event_kind": "spectral_observation",
                        }
                    ),
                ),
            )
            connection.commit()
        finally:
            connection.close()

        hindsight.sync_hindsight_database(
            operator, self.workspace, 1_700_000_001_000
        )
        connection = sqlite3.connect(database_path)
        try:
            tuning_rows = connection.execute(
                "SELECT event_id FROM tuning_events"
            ).fetchall()
            spectral_rows = connection.execute(
                "SELECT event_kind FROM spectral_receipts"
            ).fetchall()
            projection = connection.execute(
                "SELECT value FROM metadata "
                "WHERE key = 'spectral_tuning_projection_version'"
            ).fetchone()
        finally:
            connection.close()

        self.assertEqual(len(tuning_rows), 1)
        self.assertNotIn(("legacy-spectral-misclassified",), tuning_rows)
        self.assertEqual(spectral_rows, [("read_spectral_now",)])
        self.assertEqual(
            projection,
            (str(hindsight.SPECTRAL_TUNING_PROJECTION_VERSION),),
        )

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
        self.assertEqual(len(report["spectral"]["receipts"]), 1)
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
        self.assertIn("SPECTRAL_RECEIPT", rendered)
        self.assertEqual(rendered.count("TUNING phase="), 1)
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

    def test_current_scheduled_introspection_ledger_is_checkpointed_exactly(self) -> None:
        self.fixture()
        self.write_jsonl(
            "introspections/scheduled/receipts.jsonl",
            [
                {
                    "completed_at_unix_ms": 1_700_000_000_700,
                    "status": "authored_completed",
                }
            ],
        )
        hindsight.record(self.record_args())
        latest = hindsight.read_json(self.root / "operator/hindsight/latest.json")
        summary = latest["ledgers"]["introspections/scheduled/receipts.jsonl"]
        self.assertTrue(summary["present"])
        self.assertEqual(summary["invalid_json_lines"], 0)
        self.assertEqual(
            hindsight.checkpoint_prefix_status(self.workspace, latest)[
                "introspections/scheduled/receipts.jsonl"
            ],
            "unchanged_and_verified",
        )
        ledger = self.workspace / "introspections/scheduled/receipts.jsonl"
        ledger.write_text(
            ledger.read_text().replace("authored_completed", "altered__completed")
        )
        self.assertEqual(
            hindsight.checkpoint_prefix_status(self.workspace, latest)[
                "introspections/scheduled/receipts.jsonl"
            ],
            "checkpointed_prefix_changed",
        )

    def test_scheduled_reflection_projection_merges_ledgers_and_excludes_text(self) -> None:
        trace = {
            "schema_version": 1,
            "trace_id": "00000000-0000-4000-8000-000000000101",
            "span_id": "00000000-0000-4000-8000-000000000102",
            "turn_id": "00000000-0000-4000-8000-000000000103",
            "session_id": "scheduled-session",
        }
        receipt = {
            "schema": "astrid_edge_scheduled_introspection_v1",
            "completed_at_unix_ms": 1_700_000_000_700,
            "status": "authored_completed",
            "provenance": "model_authored_runtime_scheduled",
            "continuity_projection_written": True,
            "response_sha256": "a" * 64,
            "reflection_path": "introspections/scheduled/reflection_1700000000700.md",
            "prompt_chars": 840,
            "candidate_id": "candidate-one",
            "candidate_digest": "b" * 64,
            "introspection_tool": "read_owned_continuity",
            "introspection_result_sha256": "c" * 64,
            "trace": trace,
            "prompt": "SECRET_PROMPT_BODY_MUST_NOT_ENTER_HINDSIGHT",
            "response": "SECRET_RESPONSE_BODY_MUST_NOT_ENTER_HINDSIGHT",
            "source": "SECRET_SOURCE_BODY_MUST_NOT_ENTER_HINDSIGHT",
            "diff": "SECRET_DIFF_MUST_NOT_ENTER_HINDSIGHT",
            "build_log": "SECRET_BUILD_LOG_MUST_NOT_ENTER_HINDSIGHT",
            "test_log": "SECRET_TEST_LOG_MUST_NOT_ENTER_HINDSIGHT",
        }
        self.write_jsonl("introspections/scheduled/receipts.jsonl", [receipt])
        self.write_jsonl("introspection/scheduled/receipts.jsonl", [receipt])
        admission = self.workspace / "runtime/scheduled-introspection/admission/state.json"
        admission.parent.mkdir(parents=True, exist_ok=True)
        admission.write_text(
            json.dumps(
                {
                    "schema": "astrid.edge.scheduled_introspection.admission.v1",
                    "continuity_admitted": True,
                    "provenance": "model_authored_runtime_scheduled",
                    "authority": "runtime_verified_projection_observational_only",
                    "last_response_sha256": "a" * 64,
                    "last_trace_id": trace["trace_id"],
                }
            )
        )

        hindsight.record(self.record_args())
        database_path = self.root / "operator/hindsight/hindsight.sqlite3"
        first = hindsight.query_hindsight_database(
            database_path, 1_700_000_000_000, 1_700_000_001_000, 100
        )
        self.assertEqual(first["scheduled_reflection_count"], 1)
        self.assertEqual(len(first["scheduled_reflections"]), 1)
        projected = first["scheduled_reflections"][0]
        self.assertTrue(projected["authored"])
        self.assertTrue(projected["continuity_admitted"])
        self.assertEqual(projected["exact_duplicate_count"], 1)
        self.assertEqual(
            projected["source_ledgers"],
            [
                "introspection/scheduled/receipts.jsonl",
                "introspections/scheduled/receipts.jsonl",
            ],
        )
        forbidden = {
            "prompt",
            "response",
            "source",
            "diff",
            "build_log",
            "test_log",
        }
        self.assertTrue(forbidden.isdisjoint(projected))

        connection = sqlite3.connect(database_path)
        try:
            rows_before = connection.execute(
                "SELECT event_id, metadata_json FROM scheduled_reflections "
                "ORDER BY recorded_at_unix_ms, event_id"
            ).fetchall()
            raw_metadata = "\n".join(row[1] for row in rows_before)
        finally:
            connection.close()
        self.assertNotIn("SECRET_", raw_metadata)
        hindsight.sync_hindsight_database(
            self.root / "operator/hindsight", self.workspace, 1_700_000_001_000
        )
        connection = sqlite3.connect(database_path)
        try:
            rows_after = connection.execute(
                "SELECT event_id, metadata_json FROM scheduled_reflections "
                "ORDER BY recorded_at_unix_ms, event_id"
            ).fetchall()
        finally:
            connection.close()
        self.assertEqual(rows_after, rows_before)
        self.assertEqual(database_path.stat().st_mode & 0o777, 0o600)

    def test_self_change_lifecycle_uses_sanitized_projection_and_persists_tail(self) -> None:
        events = [
            self.operator_event(),
            self.operator_event(
                recorded_at=1_700_000_002,
                source_ledger="operator",
                event_id="reflection-1",
                status="steward_profile_completed",
                facets=["reflection"],
                record_sha256="3" * 64,
                response_sha256="4" * 64,
            ),
            self.operator_event(
                recorded_at=1_700_000_003,
                source_ledger="build",
                event_id="build-1",
                status="build_recorded",
                facets=["build", "invariant", "shadow", "test"],
                record_sha256="5" * 64,
                build_id="build-one",
                generation_id="generation-one",
                tests_sha256="6" * 64,
                bundle_sha256="7" * 64,
                invariant_candidate_replay_sha256="8" * 64,
                invariant_package_replay_sha256="9" * 64,
                shadow_evidence_sha256="9" * 64,
                shadow_status="package_replay_hash_only_no_detailed_shadow_claim",
                command_profile="build",
                command_executable_sha256="a" * 64,
                command_argv_sha256="b" * 64,
                command_stdout_sha256="c" * 64,
                command_stderr_sha256="d" * 64,
                command_exit_code=0,
                command_timed_out=False,
            ),
            self.operator_event(
                recorded_at=1_700_000_004,
                source_ledger="activation",
                event_id="activation-1",
                status="activation_completed",
                facets=["activation", "restart"],
                record_sha256="e" * 64,
                build_id="build-one",
                generation_id="generation-one",
            ),
            self.operator_event(
                recorded_at=1_700_000_005,
                source_ledger="activation",
                sequence=2,
                event_id="probation-1",
                status="probation_started",
                facets=["activation", "probation", "restart"],
                record_sha256="f" * 64,
                build_id="build-one",
                generation_id="generation-one",
            ),
            self.operator_event(
                recorded_at=1_700_000_006,
                source_ledger="activation",
                sequence=3,
                event_id="rollback-1",
                status="rollback_reconciled_after_restart",
                facets=["restart", "rollback"],
                record_sha256="0" * 64,
                generation_id="generation-zero",
                from_generation="generation-one",
            ),
            self.operator_event(
                recorded_at=1_700_000_007,
                source_ledger="activation",
                sequence=4,
                event_id="terminal-rejection-1",
                status="scheduled_intent_terminal_rejected",
                facets=["candidate"],
                record_sha256="1" * 64,
                terminal_reason_sha256="2" * 64,
                terminal_authority="terminal_exact_candidate_rejection_no_promotion",
                automatic_retry=False,
            ),
        ]
        operator_projection = self.write_operator_projection(events)
        private = self.root / "self-change/ledgers/build.jsonl"
        private.parent.mkdir(parents=True, exist_ok=True)
        private.write_text(
            "SECRET_PROMPT SECRET_RESPONSE SECRET_DIFF SECRET_BUILD_LOG",
            encoding="utf-8",
        )
        private.chmod(0o000)
        try:
            self.record_with_operator_projection(operator_projection)
        finally:
            private.chmod(0o600)
        operator = self.root / "operator/hindsight"
        database_path = operator / "hindsight.sqlite3"
        view = hindsight.query_hindsight_database(
            database_path, 1_700_000_000_000, 1_700_000_010_000, 100
        )
        facets = {
            facet
            for event in view["self_change_events"]
            for facet in event["lifecycle_facets"]
        }
        self.assertTrue(
            {
                "reflection",
                "candidate",
                "build",
                "test",
                "invariant",
                "shadow",
                "activation",
                "restart",
                "probation",
                "rollback",
            }.issubset(facets)
        )
        for event in view["self_change_events"]:
            self.assertFalse(event["authored"])
            self.assertNotIn("SECRET_", json.dumps(event, sort_keys=True))
        build_recorded = next(
            event
            for event in view["self_change_events"]
            if event["status"] == "build_recorded"
        )
        self.assertEqual(build_recorded["tests_sha256"], "6" * 64)
        self.assertEqual(build_recorded["bundle_sha256"], "7" * 64)
        self.assertTrue(build_recorded["package_replay_sha256_present"])
        self.assertEqual(
            build_recorded["shadow_gate_evidence"],
            "indirect_package_replay_sha256_commitment_not_independently_reinspectable",
        )
        self.assertEqual(build_recorded["shadow_evidence_sha256"], "9" * 64)
        self.assertEqual(
            build_recorded["invariant_candidate_replay_sha256"], "8" * 64
        )
        terminal = next(
            event
            for event in view["self_change_events"]
            if event["status"] == "scheduled_intent_terminal_rejected"
        )
        self.assertEqual(terminal["terminal_reason_sha256"], "2" * 64)
        self.assertEqual(
            terminal["terminal_authority"],
            "terminal_exact_candidate_rejection_no_promotion",
        )
        connection = sqlite3.connect(database_path)
        try:
            stored_flags = connection.execute(
                "SELECT shadow_evidence_in_tests_bundle, "
                "package_replay_sha256_present, shadow_gate_evidence "
                "FROM self_change_events WHERE status = 'build_recorded'"
            ).fetchone()
            raw_metadata = "\n".join(
                row[0]
                for row in connection.execute(
                    "SELECT metadata_json FROM self_change_events ORDER BY event_id"
                ).fetchall()
            )
        finally:
            connection.close()
        self.assertEqual(
            stored_flags,
            (
                0,
                1,
                "indirect_package_replay_sha256_commitment_not_independently_reinspectable",
            ),
        )
        self.assertNotIn("SECRET_", raw_metadata)
        latest = hindsight.read_json(operator / "latest.json")
        self.assertNotIn("self-change/ledgers/build.jsonl", latest["ledgers"])

        # A later bounded tail omits the first record. Hindsight retains the
        # already accepted stable record identity instead of re-reading the
        # private ledger or dropping history.
        operator_projection = self.write_operator_projection(events[1:], total=65)
        self.record_with_operator_projection(operator_projection)
        retained = hindsight.query_hindsight_database(
            database_path, 1_700_000_000_000, 1_700_000_010_000, 100
        )
        self.assertEqual(len(retained["self_change_events"]), len(events))

        # A later verified evidence projection may enrich the same immutable
        # signed-ledger record. It must replace that event's metadata rather
        # than create a second identity.
        enriched = [dict(event) for event in events[1:]]
        enriched_build = next(
            event for event in enriched if event["status"] == "build_recorded"
        )
        enriched_build["tests_sha256"] = "a" * 64
        enriched_build["manifest_sha256"] = "b" * 64
        original_build = next(
            event
            for event in retained["self_change_events"]
            if event["status"] == "build_recorded"
        )
        operator_projection = self.write_operator_projection(enriched, total=65)
        self.record_with_operator_projection(operator_projection)
        repeated = hindsight.query_hindsight_database(
            database_path, 1_700_000_000_000, 1_700_000_010_000, 100
        )
        self.assertEqual(len(repeated["self_change_events"]), len(events))
        self.assertEqual(
            len({event["event_id"] for event in repeated["self_change_events"]}),
            len(events),
        )
        updated_build = next(
            event
            for event in repeated["self_change_events"]
            if event["status"] == "build_recorded"
        )
        self.assertEqual(updated_build["event_id"], original_build["event_id"])
        self.assertEqual(updated_build["tests_sha256"], "a" * 64)
        self.assertEqual(updated_build["manifest_sha256"], "b" * 64)

    def test_interrupted_action_correction_is_non_authored_exact_attribution(self) -> None:
        trace = {
            "schema_version": 1,
            "trace_id": "00000000-0000-4000-8000-000000000021",
            "span_id": "00000000-0000-4000-8000-000000000022",
            "turn_id": "00000000-0000-4000-8000-000000000023",
            "session_id": "session-interrupted",
        }
        self.write_jsonl(
            "actions/receipts.jsonl",
            [
                {
                    "recorded_at_unix_ms": 200,
                    "artifact_path": "home://edge/journal/journal_200.md",
                    "declared_next": "JOURNAL stale",
                    "decision_source": "astrid_declared",
                    "response_sha256": "c" * 64,
                    "trace": trace,
                }
            ],
        )
        self.write_jsonl(
            "actions/interrupted_corrections.jsonl",
            [
                {
                    "schema": "astrid_edge_interrupted_action_correction_v2",
                    "recorded_at_unix_ms": 300,
                    "response_sha256": "c" * 64,
                    "trace_id": trace["trace_id"],
                    "turn_id": trace["turn_id"],
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

    def test_exact_prefix_snapshot_reports_concurrent_truncation(self) -> None:
        ledger = self.workspace / "runtime/fill_history.jsonl"
        first = b'{"recorded_at_unix_ms":1700000000000,"fill_pct":68.0}\n'
        second = b'{"recorded_at_unix_ms":1700000001000,"fill_pct":69.0}\n'
        ledger.write_bytes(first + second)

        summary = hindsight.ledger_summary(
            ledger, lambda: ledger.write_bytes(first)
        )

        self.assertEqual(summary["size_bytes"], len(first + second))
        self.assertEqual(summary["snapshot_unread_bytes"], len(second))

    def test_malformed_and_partial_ledgers_fail_checkpoint_and_live_report(self) -> None:
        self.fixture()
        hindsight.record(self.record_args())
        ledger = self.workspace / "actions/receipts.jsonl"
        with ledger.open("ab") as handle:
            handle.write(b"[]\n")
            handle.write(b'{"non_finite":NaN}\n')
            handle.write(b'{"unfinished":')

        summary = hindsight.ledger_summary(ledger)
        self.assertEqual(summary["invalid_json_lines"], 2)
        self.assertGreater(summary["trailing_partial_bytes"], 0)

        before_checkpoint = hindsight.build_report(
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
        self.assertFalse(before_checkpoint["integrity"]["current_ledger_syntax_valid"])
        self.assertFalse(before_checkpoint["integrity"]["overall_valid"])

        hindsight.record(self.record_args())
        latest = hindsight.read_json(self.root / "operator/hindsight/latest.json")
        self.assertEqual(latest["continuity_status"], "integrity_violation")
        self.assertEqual(latest["current_epoch_integrity_violation_count"], 1)
        violation = hindsight.read_json(
            self.root / "operator/hindsight/collector_state.json"
        )["epoch_integrity_violations"][0]
        self.assertEqual(
            violation["classification"],
            "ledger_malformed_json_or_partial_tail",
        )

    def test_transient_partial_tail_resolves_without_poisoning_epoch(self) -> None:
        self.fixture()
        hindsight.record(self.record_args())
        ledger = self.workspace / "actions/receipts.jsonl"
        with ledger.open("ab") as handle:
            handle.write(b'{"recorded_at_unix_ms":1700000000800')

        hindsight.record(self.record_args())
        deferred = hindsight.read_json(self.root / "operator/hindsight/latest.json")
        self.assertEqual(deferred["current_epoch_integrity_violation_count"], 0)
        self.assertEqual(deferred["pending_tail_observation_count"], 1)
        self.assertNotEqual(deferred["continuity_status"], "integrity_violation")

        with ledger.open("ab") as handle:
            handle.write(b",\"fill_pct\":68.0")
        hindsight.record(self.record_args())
        growing = hindsight.read_json(self.root / "operator/hindsight/latest.json")
        self.assertEqual(growing["current_epoch_integrity_violation_count"], 0)
        self.assertEqual(growing["pending_tail_observation_count"], 1)

        with ledger.open("ab") as handle:
            handle.write(b'}\n')
        hindsight.record(self.record_args())
        resolved = hindsight.read_json(self.root / "operator/hindsight/latest.json")
        state = hindsight.read_json(
            self.root / "operator/hindsight/collector_state.json"
        )
        self.assertEqual(resolved["current_epoch_integrity_violation_count"], 0)
        self.assertEqual(resolved["pending_tail_observation_count"], 0)
        self.assertEqual(resolved["continuity_status"], "verified")
        self.assertEqual(state["pending_tail_observations"], {})
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
        self.assertTrue(report["integrity"]["overall_valid"])
        self.assertEqual(
            report["integrity"]["pending_tail_observation_count"], 0
        )

    def test_stable_partial_tail_is_promoted_on_subsequent_checkpoint(self) -> None:
        self.fixture()
        hindsight.record(self.record_args())
        ledger = self.workspace / "actions/receipts.jsonl"
        with ledger.open("ab") as handle:
            handle.write(b'{"unfinished":')

        hindsight.record(self.record_args())
        first = hindsight.read_json(self.root / "operator/hindsight/latest.json")
        self.assertEqual(first["current_epoch_integrity_violation_count"], 0)
        self.assertEqual(first["pending_tail_observation_count"], 1)

        hindsight.record(self.record_args())
        confirmed = hindsight.read_json(
            self.root / "operator/hindsight/latest.json"
        )
        state = hindsight.read_json(
            self.root / "operator/hindsight/collector_state.json"
        )
        self.assertEqual(confirmed["current_epoch_integrity_violation_count"], 1)
        self.assertEqual(confirmed["pending_tail_observation_count"], 0)
        self.assertEqual(confirmed["continuity_status"], "integrity_violation")
        self.assertEqual(
            state["epoch_integrity_violations"][0]["confirmation"],
            "stable_tail_confirmed_across_subsequent_checkpoint",
        )

    def test_complete_malformed_line_fails_on_first_checkpoint(self) -> None:
        self.fixture()
        hindsight.record(self.record_args())
        ledger = self.workspace / "actions/receipts.jsonl"
        with ledger.open("ab") as handle:
            handle.write(b"not-json\n")

        hindsight.record(self.record_args())
        latest = hindsight.read_json(self.root / "operator/hindsight/latest.json")
        self.assertEqual(latest["current_epoch_integrity_violation_count"], 1)
        self.assertEqual(latest["pending_tail_observation_count"], 0)
        self.assertEqual(latest["continuity_status"], "integrity_violation")

    def test_verify_chain_rejects_malformed_and_unterminated_records(self) -> None:
        operator = self.root / "operator/hindsight"
        operator.mkdir(parents=True)
        malformed = operator / "malformed.jsonl"
        malformed.write_bytes(b"not-json\n")
        self.assertFalse(hindsight.verify_chain(malformed)["valid"])
        self.assertIn("malformed JSON", hindsight.verify_chain(malformed)["issues"][0])

        truncated = operator / "truncated.jsonl"
        record = {"value": 1, "previous_record_sha256": None}
        record["record_sha256"] = hindsight.digest_value(record)
        truncated.write_text(json.dumps(record))
        verification = hindsight.verify_chain(truncated)
        self.assertFalse(verification["valid"])
        self.assertIn("unterminated trailing record", verification["issues"][0])

    def test_append_chain_recovers_a_valid_head_after_stale_state(self) -> None:
        chain = self.root / "operator/hindsight/checkpoints.jsonl"
        first_head, _ = hindsight.append_chained(chain, [{"value": 1}], None)
        second_head, _ = hindsight.append_chained(
            chain, [{"value": 2}], first_head
        )
        final_head, _ = hindsight.append_chained(
            chain,
            [{"value": 3}],
            first_head,
        )

        self.assertNotEqual(first_head, second_head)
        self.assertNotEqual(second_head, final_head)
        verification = hindsight.verify_chain(chain)
        self.assertTrue(verification["valid"])
        self.assertEqual(verification["records"], 3)
        self.assertEqual(verification["head_sha256"], final_head)
        records = list(hindsight.json_lines(chain))
        self.assertEqual(records[2]["previous_record_sha256"], second_head)

    def test_record_recovers_all_chain_heads_after_append_before_state_crash(self) -> None:
        self.fixture()
        hindsight.record(self.record_args())
        operator = self.root / "operator/hindsight"
        stale_state = hindsight.read_json(operator / "collector_state.json")
        simulated_heads: dict[str, str | None] = {}
        for chain_name, state_field in (
            ("artifacts", "artifact_record_sha256"),
            ("fill_rollups", "fill_record_sha256"),
            ("checkpoints", "checkpoint_record_sha256"),
        ):
            simulated_heads[chain_name], _ = hindsight.append_chained(
                operator / f"{chain_name}.jsonl",
                [{"simulated_crash_boundary": chain_name}],
                stale_state[state_field],
            )

        hindsight.record(self.record_args())
        recovered_state = hindsight.read_json(operator / "collector_state.json")
        self.assertEqual(
            recovered_state["artifact_record_sha256"],
            simulated_heads["artifacts"],
        )
        self.assertEqual(
            recovered_state["fill_record_sha256"],
            simulated_heads["fill_rollups"],
        )
        checkpoint_records = list(
            hindsight.json_lines(operator / "checkpoints.jsonl")
        )
        self.assertEqual(
            checkpoint_records[-1]["previous_record_sha256"],
            simulated_heads["checkpoints"],
        )
        for chain_name in ("artifacts", "fill_rollups", "checkpoints"):
            self.assertTrue(
                hindsight.verify_chain(operator / f"{chain_name}.jsonl")["valid"]
            )

    def test_append_chain_rejects_an_unrelated_collector_head(self) -> None:
        chain = self.root / "operator/hindsight/checkpoints.jsonl"
        hindsight.append_chained(chain, [{"value": 1}], None)
        with self.assertRaisesRegex(RuntimeError, "not an ancestor"):
            hindsight.append_chained(chain, [{"value": 2}], "f" * 64)
        self.assertEqual(hindsight.verify_chain(chain)["records"], 1)

    def test_collector_lock_rejects_an_overlapping_record(self) -> None:
        self.fixture()
        operator = self.root / "operator/hindsight"
        with hindsight.exclusive_collector_lock(operator):
            with self.assertRaisesRegex(RuntimeError, "already holds"):
                hindsight.record(self.record_args())
        hindsight.record(self.record_args())

    def test_atomic_json_replace_fsyncs_the_parent_directory(self) -> None:
        destination = self.root / "operator/hindsight/latest.json"
        with mock.patch.object(hindsight, "fsync_directory") as sync:
            hindsight.owner_write_json(destination, {"valid": True})
        sync.assert_called_once_with(destination.parent)
        self.assertEqual(hindsight.read_json(destination), {"valid": True})

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

    def test_text_projection_neutralizes_controls_without_mutating_json_data(self) -> None:
        hostile = "artifact\x1b]52;c;Zm9v\x07\x1b[2J\x9b31m\u202ereversed"
        report = {
            "integrity": {
                "checkpoint_present": False,
                "checkpoint_age_seconds": None,
                "overall_valid": False,
                "checkpointed_ledger_prefixes_valid": False,
                "continuity_epoch": "test",
                "continuity_status": "missing",
                "current_epoch_integrity_violation_count": 0,
                "pending_tail_observation_count": 0,
                "legacy_race_compatible_unresolved_violation_count": 0,
                "historical_ledger_integrity_violation_count": 0,
                "chains": {},
            },
            "durable_sources": {
                "historical_query_source": "test",
                "activity_event_count_in_range": 1,
                "activity_events_returned": 1,
                "artifact_file_count_in_range": 0,
                "artifact_files_returned": 0,
                "fill_rollup_count_in_range": 0,
                "spectral_rollup_count_in_range": 0,
                "spectral_receipt_count_in_range": 0,
                "tuning_event_count_in_range": 0,
                "scheduled_reflection_count_in_range": 0,
                "self_change_event_count_in_range": 0,
                "state_database": {},
                "audit_database": {},
                "operator_hindsight_database": {},
            },
            "fill": {"summary": {"sample_count": 0}},
            "spectral": {"summary": {"rollup_count": 0}, "receipts": [], "tuning_events": []},
            "self_evolution": {"summary": {}, "scheduled_reflections": [], "lifecycle_events": []},
            "activity": [
                {
                    "timestamp_unix_ms": 1_700_000_000_000,
                    "kind": "web_result",
                    "authored": False,
                    "summary": hostile,
                }
            ],
            "artifacts": [],
            "range": {"since_unix_ms": 1, "until_unix_ms": 2},
            "workspace": hostile,
            "authority_note": hostile,
        }

        rendered = hindsight.render_text(report)

        for control in ("\x1b", "\x07", "\x9b", "\u202e"):
            self.assertNotIn(control, rendered)
        self.assertEqual(report["workspace"], hostile)


if __name__ == "__main__":
    unittest.main()
