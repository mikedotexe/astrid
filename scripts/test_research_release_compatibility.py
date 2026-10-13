"""Synthetic state and stubbed native inspectors; no service operations."""
import hashlib
import json
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

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


class MailboxCompatibilityTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.workspace = self.root / "workspace"
        self.source = self.root / "candidate"
        self.store = self.workspace / "correspondence/mailbox_v1"

    def candidate(self, declaration="MAILBOX_COMPATIBILITY_VERSION = 1\n"):
        module = self.source / "minime_autonomy/mailbox.py"
        module.parent.mkdir(parents=True, exist_ok=True)
        module.write_text(declaration + "raise RuntimeError('candidate must never be imported')\n")
        return module

    def mailbox(self, phase=None, *, settled=False):
        self.store.mkdir(parents=True, exist_ok=True)
        (self.store / "initialized").write_bytes(b"minime_mailbox_v1")
        state = dict(schema="minime_mailbox_v1", seen=[], completed={}, pending=None)
        if phase:
            raw = b"Private synthetic source.\r\nExact line endings.\r\n"
            digest = hashlib.sha256(raw).hexdigest()
            artifact = hashlib.sha256(b"operation").hexdigest()
            pending = dict(operation="operation", filename="synthetic.txt", source_sha256=digest,
                           artifact_id=artifact, phase=phase)
            (self.store / (digest + ".txt")).write_bytes(raw)
            if phase in {"supplied", "response_retained"}:
                pending["supplied_attempt"] = "submission_synthetic"
            if phase == "response_retained":
                raw = b'{"done": true, "done_reason": "stop", "message": {"content": "Private reply"}}'
                (self.store / (artifact + ".provider.json")).write_bytes(raw)
                pending.update(completion_validated=True, provider_receipt_sha256=hashlib.sha256(raw).hexdigest())
                raw = b"Private reply\r\n"
                (self.store / (artifact + ".response.txt")).write_bytes(raw)
                pending["response_sha256"] = hashlib.sha256(raw).hexdigest()
            if settled:
                state["completed"]["operation"] = artifact
            else:
                state["pending"] = pending
        (self.store / "state.json").write_text(json.dumps(state))
        return state

    def publication(self, *, human=False, published=True):
        relative = "inbox/human_letter_synthetic.txt" if human else "outbox/human/mike/human_reply_synthetic.txt"
        name = "human_letter_publications_v1" if human else "inbox_publications_v1"
        directory = self.workspace / "correspondence" / name / hashlib.sha256(relative.encode()).hexdigest()
        directory.mkdir(parents=True, exist_ok=True)
        metadata = {"schema": "human_letter_publication_v1" if human else "inbox_publication_v1",
                    "sha256": hashlib.sha256(b"Private synthetic bytes").hexdigest()}
        if not human:
            metadata["path"] = relative
        raw = json.dumps(metadata, sort_keys=True).encode()
        (directory / "intent.json").write_bytes(raw)
        if published:
            (directory / "published.json").write_bytes(raw)
        return directory

    def snapshot(self):
        result = {}
        for path in [self.root, *self.root.rglob("*")]:
            info = path.lstat()
            body = os.readlink(path) if path.is_symlink() else path.read_bytes() if path.is_file() else None
            result[str(path.relative_to(self.root))] = (info.st_ino, info.st_mode, info.st_size,
                                                       info.st_mtime_ns, body)
        return result

    def verify_preserved(self, *, refused=False, source=None):
        before = self.snapshot()
        real_open = os.open

        def read_only(path, flags, *args, **kwargs):
            self.assertFalse(flags & (os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND))
            return real_open(path, flags, *args, **kwargs)

        with patch.object(guard.os, "open", side_effect=read_only):
            if refused:
                with self.assertRaises(RuntimeError) as caught:
                    guard.verify_minime(source or self.source, self.workspace)
                self.assertNotIn("Private", str(caught.exception))
            else:
                guard.verify_minime(source or self.source, self.workspace)
        self.assertEqual(self.snapshot(), before)

    def test_no_history_or_empty_store_needs_no_capability_or_writes(self):
        self.verify_preserved()
        self.store.mkdir(parents=True)
        self.verify_preserved()

    def test_all_reservation_phases_require_new_candidate(self):
        for phase in ("selected", "started", "supplied", "response_retained"):
            with self.subTest(phase=phase):
                self.mailbox(phase)
                self.candidate("MAILBOX_COMPATIBILITY_VERSION = 0\n")
                self.verify_preserved(refused=True)
                self.candidate()
                self.verify_preserved()

    def test_settled_and_initialized_stores_keep_floor(self):
        for phase in (None, "response_retained"):
            with self.subTest(phase=phase):
                self.mailbox(phase, settled=True)
                self.candidate("# old candidate has no mailbox capability\n")
                self.verify_preserved(refused=True)
                self.candidate()
                self.verify_preserved()

    def test_literal_capability_is_not_imported_or_spoofed(self):
        self.mailbox("selected")
        for text in ("# MAILBOX_COMPATIBILITY_VERSION = 1\n", "MAILBOX_COMPATIBILITY_VERSION = True\n",
                     "MAILBOX_COMPATIBILITY_VERSION = int('1')\n", "MAILBOX_COMPATIBILITY_VERSION = 0\n",
                     "MAILBOX_COMPATIBILITY_VERSION = 1\nMAILBOX_COMPATIBILITY_VERSION = unknown\n",
                     "MAILBOX_COMPATIBILITY_VERSION = 1\nMAILBOX_COMPATIBILITY_VERSION = 2\n",
                     "MAILBOX_COMPATIBILITY_VERSION = (Private invalid syntax\n"):
            self.candidate(text)
            self.verify_preserved(refused=True)
        self.candidate()
        self.verify_preserved()

    def test_missing_manifest_or_initialized_marker_fails_closed(self):
        self.candidate()
        for missing in ("state.json", "initialized"):
            self.mailbox("selected")
            (self.store / missing).unlink()
            self.verify_preserved(refused=True)

    def test_orphan_artifact_is_not_a_fresh_store(self):
        self.candidate()
        self.store.mkdir(parents=True)
        (self.store / ("a" * 64 + ".txt")).write_bytes(b"Private source")
        self.verify_preserved(refused=True)

    def test_corrupt_unknown_duplicate_and_oversize_manifest(self):
        state = self.mailbox("selected")
        self.candidate()
        raw = json.dumps(state).encode()
        values = [b"{Private broken", b"[]", b"x" * 8_000_001,
                  raw.replace(b'"minime_mailbox_v1"', b'"minime_mailbox_v2"'),
                  raw[:-1] + b', "pending": null}',
                  json.dumps({**state, "pending": {**state["pending"], "phase": "unknown"}}).encode(),
                  json.dumps({**state, "seen": [False]}).encode(),
                  json.dumps({**state, "pending": {**state["pending"], "filename": "../escape"}}).encode()]
        for value in values:
            (self.store / "state.json").write_bytes(value)
            self.verify_preserved(refused=True)

    def test_retained_source_response_and_provider_are_verified(self):
        self.candidate()
        for suffix in ("source", "response", "provider"):
            state = self.mailbox("response_retained")
            pending = state["pending"]
            name = (pending["source_sha256"] + ".txt" if suffix == "source"
                    else pending["artifact_id"] + (".response.txt" if suffix == "response" else ".provider.json"))
            path = self.store / name
            path.unlink()
            self.verify_preserved(refused=True)
            path.write_bytes(b"Private changed artifact")
            self.verify_preserved(refused=True)

    def test_rejected_provider_receipt_remains_uncertain_not_repaired(self):
        self.candidate()
        state = self.mailbox("supplied")
        pending = state["pending"]
        for parsed in ({"done": True, "done_reason": "length"}, ["Private incomplete payload"]):
            raw = json.dumps(parsed).encode()
            pending.update(completion_validated=False, provider_receipt_sha256=hashlib.sha256(raw).hexdigest())
            (self.store / (pending["artifact_id"] + ".provider.json")).write_bytes(raw)
            (self.store / "state.json").write_text(json.dumps(state))
            self.verify_preserved()

    def test_mailbox_symlinks_fail_closed_without_following(self):
        self.candidate()
        self.mailbox("selected")
        for path in (self.store / "state.json", self.store / "initialized", self.store,
                     self.store.parent, self.workspace):
            with self.subTest(path=path.name):
                target = self.root / "preserved"
                path.rename(target)
                path.symlink_to(target, target_is_directory=target.is_dir())
                self.verify_preserved(refused=True)
                path.unlink()
                target.rename(path)
        source_directory = self.source / "minime_autonomy"
        source_directory.rename(self.root / "source-target")
        source_directory.symlink_to(self.root / "source-target", target_is_directory=True)
        self.verify_preserved(refused=True)

    def test_consumed_publications_and_unconfirmed_intents_keep_floor(self):
        for human in (False, True):
            for published in (False, True):
                with self.subTest(human=human, published=published):
                    self.publication(human=human, published=published)
                    self.candidate("# old candidate\n")
                    self.verify_preserved(refused=True)
                    self.candidate()
                    self.verify_preserved()
        self.assertFalse((self.workspace / "inbox").exists())
        self.assertFalse((self.workspace / "outbox").exists())

    def test_missing_corrupt_and_linked_publication_manifests(self):
        self.candidate()
        for human in (False, True):
            directory = self.publication(human=human)
            for name in ("intent.json", "published.json"):
                path = directory / name
                original = path.read_bytes()
                path.write_bytes(b"Private corrupt receipt")
                self.verify_preserved(refused=True)
                path.unlink()
                path.symlink_to(self.root / "absent")
                self.verify_preserved(refused=True)
                path.unlink()
                path.write_bytes(original)
            (directory / "intent.json").unlink()
            self.verify_preserved(refused=True)
            self.publication(human=human)

    def test_restart_and_install_reject_old_candidate_before_operations(self):
        self.mailbox("started")
        backend = object.__new__(LaunchdAgent)
        backend.root = self.root
        backend.maintenance = lambda: None
        backend.job = Mock(side_effect=AssertionError("must not inspect live service"))
        with self.assertRaisesRegex(RuntimeError, "MAILBOX_COMPATIBILITY_VERSION downgrade refused"):
            backend.validate()
        backend.job.assert_not_called()
        import paired_minime_handoff as paired
        handoff = object.__new__(paired.PairedHandoff)
        backend.validate = Mock(side_effect=AssertionError("must not perform handoff"))
        packet = dict(overlay_paths=list(paired.OVERLAY), bindings=dict(canonical_root=str(self.root)),
                      snapshot_root=str(self.source))
        before = self.snapshot()
        with patch.object(paired, "read_json", return_value=packet), \
                patch.object(paired, "qualified_inputs", return_value={}):
            with self.assertRaisesRegex(RuntimeError, "MAILBOX_COMPATIBILITY_VERSION downgrade refused"):
                handoff.install(backend, self.root / "packet.json", 123, Mock())
        backend.validate.assert_not_called()
        self.assertEqual(self.snapshot(), before)


if __name__ == "__main__":
    unittest.main()
