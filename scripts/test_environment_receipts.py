#!/usr/bin/env python3
"""Tests for scripts/environment_receipts.py."""

from __future__ import annotations

import importlib.util
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


SCRIPT = Path(__file__).resolve().with_name("environment_receipts.py")
SPEC = importlib.util.spec_from_file_location("environment_receipts", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
environment_receipts = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = environment_receipts
SPEC.loader.exec_module(environment_receipts)


class EnvironmentReceiptTests(unittest.TestCase):
    def test_repository_status_preserves_first_porcelain_path(self) -> None:
        completed = mock.Mock(
            returncode=0,
            stdout=" M CHANGELOG.md\n M capsules/spectral-bridge/src/rescue_policy.rs\n",
        )
        with mock.patch.object(environment_receipts.subprocess, "run", return_value=completed):
            status = environment_receipts.run_text(
                ["git", "status", "--porcelain=v1"], preserve_leading=True
            )

        self.assertEqual(status.splitlines()[0][3:], "CHANGELOG.md")
        self.assertEqual(
            status.splitlines()[1][3:],
            "capsules/spectral-bridge/src/rescue_policy.rs",
        )

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
            self.assertTrue(receipt["witness_only"])
            self.assertEqual(receipt["artifact_authority_state_v1"]["state"], "evidence_only")
            self.assertFalse(receipt["live_eligible_now"])
            self.assertFalse(receipt["auto_approved"])
            self.assertFalse(receipt["grants_approval"])
            self.assertFalse(receipt["edits_source_now"])
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

    def test_reads_v1_and_v2_and_ignores_unknown_schema(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            paths = environment_receipts.receipt_paths(workspace)
            paths["root"].mkdir(parents=True)
            rows = [
                {"id": "legacy", "t_ms": 1, "event": "v1"},
                {"id": "current", "schema_version": 2, "t_ms": 2, "event": "v2"},
                {"id": "future", "schema_version": 99, "t_ms": 3},
                {"id": "malformed", "schema_version": "not-a-number", "t_ms": 4},
            ]
            paths["jsonl"].write_text("\n".join(json.dumps(row) for row in rows) + "\n")

            loaded = environment_receipts.read_receipts(workspace)

            self.assertEqual([row["id"] for row in loaded], ["legacy", "current"])
            self.assertEqual([row["schema_version"] for row in loaded], [1, 2])

    def test_build_manifest_detects_artifact_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            artifact = root / "component.bin"
            manifest_path = root / "manifest.json"
            artifact.write_bytes(b"first")
            environment_receipts.write_build_manifest(
                manifest_path,
                component="test-component",
                repository=root,
                artifacts={"component": artifact},
                actor=environment_receipts.DEFAULT_ACTOR,
                command="test build",
                protocol_revision="a" * 40,
            )
            entry = environment_receipts.load_build_manifest(manifest_path)
            compatible, reasons = environment_receipts.manifest_compatibility(
                entry,
                pinned_revision="a" * 40,
            )
            self.assertTrue(compatible, reasons)

            artifact.write_bytes(b"second")
            compatible, reasons = environment_receipts.manifest_compatibility(
                entry,
                pinned_revision="a" * 40,
            )
            self.assertFalse(compatible)
            self.assertTrue(any("manifest mismatch" in reason for reason in reasons))

    def test_manifest_cli_keeps_subcommand_separate_from_build_command(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            artifact = root / "component.bin"
            manifest_path = root / "manifest.json"
            artifact.write_bytes(b"component")

            result = environment_receipts.subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "manifest",
                    "test-component",
                    "--output",
                    str(manifest_path),
                    "--repository",
                    str(root),
                    "--artifact",
                    f"component={artifact}",
                    "--command",
                    "cargo build --release",
                    "--protocol-revision",
                    "a" * 40,
                ],
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            payload = json.loads(manifest_path.read_text())
            self.assertEqual(payload["command"], "cargo build --release")

    def test_failed_deploy_is_recorded_and_returns_false(self) -> None:
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(
            environment_receipts,
            "protocol_identity",
            return_value={
                "name": "astrid-minime",
                "version": "1.0",
                "major": 1,
                "revision": "a" * 40,
                "revision_present_in_astrid": True,
                "compatible": True,
            },
        ):
            workspace = Path(tmp)
            receipt, ok = environment_receipts.record_deployment_receipt(
                workspace,
                component="bridge",
                requested_status="failed",
                actor=environment_receipts.DEFAULT_ACTOR,
                probes=[{"name": "build", "passed": False}],
            )

            self.assertFalse(ok)
            self.assertEqual(receipt["deployment"]["status"], "failed")
            self.assertEqual(receipt["deployment"]["actor"], "interactive-agent")
            self.assertTrue(environment_receipts.receipt_paths(workspace)["jsonl"].is_file())
            self.assertTrue(receipt["compatibility_status"]["failure_reasons"])

    def test_change_refs_are_additive_bounded_and_deduplicated(self) -> None:
        refs = environment_receipts.parse_change_refs(
            [
                "felt_contract=contract_example",
                "claim=introspection_astrid_codec_1:c001",
                "felt_contract=contract_example",
            ]
        )
        self.assertEqual(
            refs,
            [
                {
                    "kind": "claim",
                    "id": "introspection_astrid_codec_1:c001",
                },
                {"kind": "felt_contract", "id": "contract_example"},
            ],
        )
        with self.assertRaises(ValueError):
            environment_receipts.parse_change_refs(["authority_grant=forbidden"])
        with self.assertRaises(ValueError):
            environment_receipts.parse_change_refs(
                ["felt_contract=/private/contract"]
            )
        with self.assertRaises(ValueError):
            environment_receipts.parse_change_refs(
                ["work_item=contract_wrong_kind"]
            )

    def test_change_refs_remain_optional_for_existing_v2_writers(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            receipt = environment_receipts.build_receipt(
                Path(tmp),
                event="test",
                source="test",
            )
        self.assertNotIn("change_refs", receipt)

    def test_deployment_receipt_preserves_exact_change_refs(self) -> None:
        refs = [{"kind": "felt_contract", "id": "contract_example"}]
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(
            environment_receipts,
            "protocol_identity",
            return_value={"revision": "a" * 40, "compatible": True},
        ):
            receipt, ok = environment_receipts.record_deployment_receipt(
                Path(tmp),
                component="test",
                requested_status="passed",
                actor=environment_receipts.DEFAULT_ACTOR,
                change_refs=refs,
            )
        self.assertTrue(ok)
        self.assertEqual(receipt["change_refs"], refs)
        self.assertEqual(receipt["schema_version"], 2)

    def test_pid_identity_and_fresh_pid_check(self) -> None:
        identity = environment_receipts.process_identity(os.getpid())
        self.assertTrue(identity["running"])
        self.assertIsNotNone(identity["started_at"])

        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(
            environment_receipts,
            "protocol_identity",
            return_value={"revision": "a" * 40, "compatible": True},
        ):
            receipt, ok = environment_receipts.record_deployment_receipt(
                Path(tmp),
                component="test",
                requested_status="passed",
                actor=environment_receipts.DEFAULT_ACTOR,
                old_pid=os.getpid(),
                new_pid=os.getpid(),
            )
            self.assertFalse(ok)
            self.assertTrue(
                any("fresh PID" in reason for reason in receipt["compatibility_status"]["failure_reasons"])
            )

    def test_pre_restart_process_snapshot_survives_pid_reuse(self) -> None:
        live_new = {
            "pid": 222,
            "running": True,
            "started_at": "Thu Jul 16 16:28:01 2026",
            "command": "/tmp/spectral-bridge-server --autonomous",
        }
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(
            environment_receipts,
            "protocol_identity",
            return_value={"revision": "a" * 40, "compatible": True},
        ), mock.patch.object(
            environment_receipts,
            "process_identity",
            return_value=live_new,
        ) as process_identity:
            receipt, ok = environment_receipts.record_deployment_receipt(
                Path(tmp),
                component="test",
                requested_status="passed",
                actor=environment_receipts.DEFAULT_ACTOR,
                old_pid=111,
                old_started_at="Thu Jul 16 16:20:00 2026",
                old_command="/tmp/old-spectral-bridge-server --autonomous",
                old_captured_at="2026-07-16T23:27:59Z",
                new_pid=222,
            )

            self.assertTrue(ok)
            self.assertEqual(receipt["processes"]["old"]["pid"], 111)
            self.assertEqual(
                receipt["processes"]["old"]["command"],
                "/tmp/old-spectral-bridge-server --autonomous",
            )
            self.assertEqual(
                receipt["processes"]["old"]["identity_source"],
                "pre_restart_snapshot",
            )
            self.assertEqual(
                receipt["processes"]["old"]["captured_at"],
                "2026-07-16T23:27:59Z",
            )
            queried_pids = [call.args[0] for call in process_identity.call_args_list]
            self.assertIn(222, queried_pids)
            self.assertNotIn(111, queried_pids)

    def test_protocol_compatibility_uses_revision_return_code(self) -> None:
        completed = mock.Mock(returncode=0)
        with mock.patch.object(
            environment_receipts, "pinned_protocol_revision", return_value="b" * 40
        ), mock.patch.object(environment_receipts.subprocess, "run", return_value=completed):
            identity = environment_receipts.protocol_identity("1.4")
            self.assertTrue(identity["compatible"])

            unsupported = environment_receipts.protocol_identity("2.0")
            self.assertFalse(unsupported["compatible"])

    def test_protocol_1_1_is_default_and_1_0_remains_compatible(self) -> None:
        completed = mock.Mock(returncode=0)
        with mock.patch.object(
            environment_receipts, "pinned_protocol_revision", return_value="b" * 40
        ), mock.patch.object(environment_receipts.subprocess, "run", return_value=completed):
            current = environment_receipts.protocol_identity()
            legacy = environment_receipts.protocol_identity("1.0")

            self.assertEqual(current["version"], "1.1")
            self.assertTrue(current["compatible"])
            self.assertTrue(legacy["compatible"])

    def test_named_path_and_probe_parsers_are_strict(self) -> None:
        paths = environment_receipts.parse_named_paths(["bridge=./target/release/bridge"])
        self.assertIn("bridge", paths)
        self.assertEqual(environment_receipts.parse_named_pids(["bridge=123"]), {"bridge": 123})
        self.assertTrue(environment_receipts.parse_probe("readyz=ok")["passed"])
        self.assertFalse(environment_receipts.parse_probe("telemetry=false")["passed"])
        with self.assertRaises(ValueError):
            environment_receipts.parse_named_paths(["missing-separator"])
        with self.assertRaises(ValueError):
            environment_receipts.parse_probe("readyz=maybe")
        with self.assertRaises(ValueError):
            environment_receipts.parse_named_pids(["bridge=not-a-pid"])


if __name__ == "__main__":
    unittest.main()
