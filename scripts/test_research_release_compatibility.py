"""Synthetic state and stubbed native inspectors; no service operations."""
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import research_release_compatibility as guard
from bridge_activate import LaunchdBridge
from restart_minime_agent import LaunchdAgent


class CompatibilityTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def requirement(self, workspace, owner, version=1):
        path = workspace / "action_threads/research-transactions-required.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(dict(schema_version=1, owner=owner,
                                      required_research_transaction_version=version)))
        return path

    def receipt_requirement(self, workspace, owner, version=1):
        path = workspace / "action_threads/provider-input-receipts-required.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(dict(schema_version=1, owner=owner,
                                      required_provider_input_receipt_version=version)))
        return path

    def test_all_floor_combinations_and_older_candidates(self):
        for research_required, receipt_required in ((False, False), (True, False), (False, True), (True, True)):
            with self.subTest(research=research_required, receipts=receipt_required):
                workspace = self.root / f"state-{research_required}-{receipt_required}"
                source = self.root / f"source-{research_required}-{receipt_required}"
                (source / "minime_autonomy").mkdir(parents=True)
                report = {}
                if research_required:
                    self.requirement(workspace, "astrid")
                    report["research_transactions"] = dict(owner="astrid", supported_version=1, required_version=1)
                if receipt_required:
                    self.receipt_requirement(workspace, "astrid")
                    with self.assertRaisesRegex(RuntimeError, "downgrade refused"):
                        guard.verify_bridge(workspace, report)
                    report["provider_input_receipts"] = dict(owner="astrid", supported_version=1, required_version=1)
                guard.verify_bridge(workspace, report)
                if research_required:
                    self.requirement(workspace, "minime")
                    (source / "minime_autonomy/research_transactions.py").write_text("RESEARCH_TRANSACTION_VERSION = 1\n")
                if receipt_required:
                    self.receipt_requirement(workspace, "minime")
                    with self.assertRaisesRegex(RuntimeError, "downgrade refused"):
                        guard.verify_minime(source, workspace)
                    (source / "minime_autonomy/research_provider.py").write_text("PROVIDER_INPUT_RECEIPT_VERSION = 1\n")
                guard.verify_minime(source, workspace)

    def test_receipt_floor_conflicts_and_corruption_preserve_history(self):
        path = self.receipt_requirement(self.root, "astrid")
        history = path.parent / "threads/synthetic/research_provider.jsonl"
        history.parent.mkdir(parents=True)
        history.write_bytes(b'{"existing":"untouched"}\n')
        valid = json.loads(path.read_bytes())
        values = [b'{broken', b'{}', b'x' * 4097,
                  json.dumps({**valid, "owner": "minime"}).encode(),
                  json.dumps({**valid, "schema_version": 2}).encode(),
                  json.dumps({**valid, "required_provider_input_receipt_version": True}).encode(),
                  json.dumps({**valid, "required_provider_input_receipt_version": 2}).encode(),
                  b'{"schema_version":1,"owner":"astrid","required_provider_input_receipt_version":2,"required_provider_input_receipt_version":1}']
        report = dict(provider_input_receipts=dict(owner="astrid", supported_version=1, required_version=1))
        for raw in values:
            path.write_bytes(raw)
            with self.assertRaises((RuntimeError, ValueError)):
                guard.verify_bridge(self.root, report)
            self.assertEqual(path.read_bytes(), raw)
            self.assertEqual(history.read_bytes(), b'{"existing":"untouched"}\n')

    def test_receipt_floor_symlinks_preserved(self):
        path = self.receipt_requirement(self.root, "astrid")
        original = path.read_bytes()
        target = self.root / "target.json"
        target.write_bytes(original)
        path.unlink()
        path.symlink_to(target)
        with self.assertRaises(RuntimeError):
            guard.verify_bridge(self.root, {})
        self.assertEqual(target.read_bytes(), original)
        target.unlink()
        with self.assertRaises(RuntimeError):
            guard.verify_bridge(self.root, {})
        self.assertTrue(path.is_symlink())
        other = self.root / "other"
        other.mkdir()
        (other / "action_threads").symlink_to(path.parent, target_is_directory=True)
        with self.assertRaises(RuntimeError):
            guard.verify_bridge(other, {})

    def test_receipt_minime_literal_support_not_dynamic_or_spoofed(self):
        self.receipt_requirement(self.root / "workspace", "minime")
        module = self.root / "minime_autonomy/research_provider.py"
        module.parent.mkdir()
        for source in ("# PROVIDER_INPUT_RECEIPT_VERSION = 1\n", "PROVIDER_INPUT_RECEIPT_VERSION = True\n",
                       "PROVIDER_INPUT_RECEIPT_VERSION = int('1')\n", "PROVIDER_INPUT_RECEIPT_VERSION = 0\n",
                       "PROVIDER_INPUT_RECEIPT_VERSION = 1\nPROVIDER_INPUT_RECEIPT_VERSION = 1\n"):
            module.write_text(source)
            with self.assertRaisesRegex(RuntimeError, "downgrade refused"):
                guard.verify_minime(self.root, self.root / "workspace")
            self.assertEqual(module.read_text(), source)
        module.write_text("PROVIDER_INPUT_RECEIPT_VERSION = 1\nraise RuntimeError('never import')\n")
        guard.verify_minime(self.root, self.root / "workspace")
        self.receipt_requirement(self.root / "workspace", "minime", 2)
        with self.assertRaisesRegex(RuntimeError, "downgrade refused"):
            guard.verify_minime(self.root, self.root / "workspace")

    def test_sanctioned_wrappers_enforce_receipt_floor_without_research_floor(self):
        backend = object.__new__(LaunchdBridge)
        backend.root = self.root
        backend.stage = self.root / "stage"
        backend.args = lambda: []
        workspace = self.root / "capsules/spectral-bridge/workspace"
        self.receipt_requirement(workspace, "astrid")
        with patch("bridge_activate.subprocess.run", return_value=SimpleNamespace(stdout="{}")):
            with self.assertRaisesRegex(RuntimeError, "downgrade refused"):
                backend.native("--verify-deployment-inputs")
        report = dict(provider_input_receipts=dict(owner="astrid", supported_version=1, required_version=1))
        with patch("bridge_activate.subprocess.run", return_value=SimpleNamespace(stdout=json.dumps(report))):
            self.assertEqual(backend.native("--verify-deployment-inputs"), report)
        self.receipt_requirement(self.root / "workspace", "minime")
        agent = object.__new__(LaunchdAgent)
        agent.root = self.root
        agent.maintenance = lambda: None
        with self.assertRaisesRegex(RuntimeError, "downgrade refused"):
            agent.validate()

    def test_missing_requirement_is_read_only(self):
        guard.verify_bridge(self.root, {})
        guard.verify_minime(self.root, self.root)
        self.assertEqual(list(self.root.iterdir()), [])

    def test_wrong_owner_corrupt_and_future_requirements_preserved(self):
        path = self.requirement(self.root, "minime")
        original = path.read_bytes()
        with self.assertRaises(RuntimeError):
            guard.required_version(self.root, "astrid")
        self.assertEqual(path.read_bytes(), original)
        path.write_text("{broken")
        with self.assertRaises(ValueError):
            guard.required_version(self.root, "minime")
        self.assertEqual(path.read_text(), "{broken")
        self.requirement(self.root, "astrid", 2)
        with self.assertRaises(RuntimeError):
            guard.verify_bridge(self.root, {"research_transactions": dict(
                owner="astrid", supported_version=1, required_version=2)})

    def test_dangling_symlink_refuses_without_repair(self):
        path = self.requirement(self.root, "astrid")
        path.unlink()
        path.symlink_to(self.root / "missing")
        with self.assertRaises(RuntimeError):
            guard.required_version(self.root, "astrid")
        self.assertTrue(path.is_symlink())

    def test_duplicate_compatibility_fields_refuse_without_repair(self):
        path = self.requirement(self.root, "astrid")
        for field, first, second in (("owner", '"minime"', '"astrid"'),
                                     ("required_research_transaction_version", "2", "1")):
            raw = ('{"schema_version":1,"owner":"astrid",'
                   '"required_research_transaction_version":1,')
            raw += f'"{field}":{first},"{field}":{second}' + "}"
            path.write_text(raw)
            with self.assertRaisesRegex(RuntimeError, "duplicate"):
                guard.required_version(self.root, "astrid")
            self.assertEqual(path.read_text(), raw)

    def test_real_bridge_inspector_rejects_legacy_response(self):
        backend = object.__new__(LaunchdBridge)
        backend.root = self.root
        backend.stage = self.root / "stage"
        backend.args = lambda: []
        workspace = self.root / "capsules/spectral-bridge/workspace"
        self.requirement(workspace, "astrid")
        with patch("bridge_activate.subprocess.run", return_value=SimpleNamespace(stdout="{}")):
            with self.assertRaisesRegex(RuntimeError, "downgrade refused"):
                backend.native("--verify-deployment-inputs")
        report = dict(research_transactions=dict(owner="astrid", supported_version=1, required_version=1))
        with patch("bridge_activate.subprocess.run", return_value=SimpleNamespace(stdout=json.dumps(report))):
            self.assertEqual(backend.native("--verify-deployment-inputs"), report)

    def test_minime_literal_capability_not_comments_or_dynamic_code(self):
        self.requirement(self.root / "workspace", "minime")
        module = self.root / "minime_autonomy/research_transactions.py"
        module.parent.mkdir()
        for source in ("# RESEARCH_TRANSACTION_VERSION = 1\n", "RESEARCH_TRANSACTION_VERSION = True\n",
                       "RESEARCH_TRANSACTION_VERSION = int('1')\n",
                       "RESEARCH_TRANSACTION_VERSION = 1\nRESEARCH_TRANSACTION_VERSION = 0\n"):
            module.write_text(source)
            with self.assertRaises(RuntimeError):
                guard.verify_minime(self.root, self.root / "workspace")
            self.assertEqual(module.read_text(), source)
        module.write_text("RESEARCH_TRANSACTION_VERSION = 1\nraise RuntimeError('never execute candidate')\n")
        guard.verify_minime(self.root, self.root / "workspace")

    def test_actual_restart_validation_refuses_before_service_or_source_operations(self):
        self.requirement(self.root / "workspace", "minime")
        backend = object.__new__(LaunchdAgent)
        backend.root = self.root
        backend.maintenance = lambda: None
        with self.assertRaisesRegex(RuntimeError, "downgrade refused"):
            backend.validate()


if __name__ == "__main__":
    unittest.main()
