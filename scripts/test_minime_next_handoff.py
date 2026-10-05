"""Synthetic receipts and fake reloads; no service operations or private state."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import minime_next_handoff as proof
import research_release_compatibility as guard
from restart_minime_agent import LaunchdAgent
from test_restart_minime_agent import Backend


class HandoffProofTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.inputs = {"minime_autonomy/runtime.py": "a" * 64}
        self.old_status = dict(next_handoff_contract=proof.CONTRACT, boot_id="1" * 32,
                               pid=10, started_at="old", source_inputs_at_start=self.inputs)
        self.status = dict(self.old_status, boot_id="2" * 32, pid=11, started_at="new")
        self.old = proof.process(self.old_status)
        self.new = proof.process(self.status)
        self.rows = []
        state = b'{"pending_next_action":"synthetic command"}\n'
        (self.root / "sovereignty_state.json").write_bytes(state)
        common = dict(state_sha256=hashlib.sha256(state).hexdigest(), pending_sha256="d" * 64, pending_present=True)
        seal = self.append(dict(common, kind="sealed", process=self.old))
        self.old_status["next_handoff_seal"] = dict(seal["body"], record_sha256=seal["sha256"])
        restored = self.append(dict(common, kind="restored", process=self.new,
                                    seal_sha256=seal["sha256"], scheduling_authorized=False))
        self.status["next_handoff_restore"] = dict(restored["body"], record_sha256=restored["sha256"])

    def append(self, body):
        body = dict(body, contract=proof.CONTRACT, version=1, sequence=len(self.rows) + 1,
                    previous=self.rows[-1]["sha256"] if self.rows else None)
        row = dict(body=body, sha256=proof.sha(body))
        self.rows.append(row)
        path = self.root / "runtime/next-handoff-v1.jsonl"
        path.parent.mkdir(exist_ok=True)
        path.write_bytes(b"".join(proof.canonical(r) + b"\n" for r in self.rows))
        return row

    def test_exact_durable_ack_is_not_execution_evidence(self):
        before = (self.root / "runtime/next-handoff-v1.jsonl").read_bytes()
        result = proof.verify_restore(self.root, self.old, self.status, self.inputs)
        self.assertTrue(result["pending_present"])
        self.assertFalse(result["execution_confirmed"])
        self.assertEqual((self.root / "runtime/next-handoff-v1.jsonl").read_bytes(), before)

    def test_seal_requires_exact_stopped_checkpoint_and_process(self):
        result = proof.verify_seal(self.root, self.old, self.old_status)
        self.assertFalse(result["execution_confirmed"])
        with self.assertRaisesRegex(RuntimeError, "identity mismatch"):
            proof.verify_seal(self.root, dict(self.old, boot_id="5" * 32), self.old_status)
        path = self.root / "sovereignty_state.json"
        path.write_bytes(b'{"pending_next_action":"newer choice"}\n')
        before = path.read_bytes()
        with self.assertRaisesRegex(RuntimeError, "checkpoint changed"):
            proof.verify_seal(self.root, self.old, self.old_status)
        self.assertEqual(path.read_bytes(), before)

    def test_missing_stale_or_tampered_ack_refuses(self):
        variants = []
        missing = copy.deepcopy(self.status)
        missing.pop("next_handoff_restore")
        variants.append(missing)
        for field, value in (("boot_id", "3" * 32), ("pid", 12),
                             ("source_inputs_at_start", {"runtime.py": "changed"})):
            variants.append(dict(self.status, **{field: value}))
        tampered = copy.deepcopy(self.status)
        tampered["next_handoff_restore"]["pending_sha256"] = "e" * 64
        variants.append(tampered)
        for status in variants:
            with self.subTest(status_field=list(status)), self.assertRaises(RuntimeError):
                proof.verify_restore(self.root, self.old, status, self.inputs)
        with self.assertRaises(RuntimeError):
            proof.verify_restore(self.root, dict(self.old, boot_id="4" * 32), self.status, self.inputs)

    def test_corrupt_duplicate_or_partial_history_preserved(self):
        path = self.root / "runtime/next-handoff-v1.jsonl"
        original = path.read_bytes()
        for raw in (original[:-1], original + b'{', b'{}\n',
                    b'{"body":{},"body":{}}\n', original.replace(b'"sequence":1', b'"sequence":true')):
            path.write_bytes(raw)
            with self.assertRaises((RuntimeError, ValueError)):
                proof.verify_restore(self.root, self.old, self.status, self.inputs)
            self.assertEqual(path.read_bytes(), raw)

    def test_linked_history_refuses(self):
        path = self.root / "runtime/next-handoff-v1.jsonl"
        target = self.root / "retained"
        path.rename(target)
        path.symlink_to(target)
        with self.assertRaisesRegex(RuntimeError, "linked"):
            proof.read_history(self.root)
        self.assertTrue(target.is_file())

    def test_growth_after_stat_remains_bounded_in_verifier_and_compatibility_guard(self):
        info = (self.root / "runtime/next-handoff-v1.jsonl").stat()
        earlier = SimpleNamespace(st_mode=info.st_mode, st_uid=info.st_uid,
                                  st_nlink=info.st_nlink, st_size=0)
        with patch.object(proof, "MAX_BYTES", 100), patch.object(proof.os, "fstat", return_value=earlier):
            for inspect in (lambda: proof.read_history(self.root),
                            lambda: guard.verify_minime(self.root, self.root)):
                with self.subTest(inspect=inspect), self.assertRaisesRegex(RuntimeError, "incomplete NEXT"):
                    inspect()

    def test_compatibility_floor_and_literal_not_imported(self):
        with self.assertRaisesRegex(RuntimeError, "downgrade refused"):
            guard.verify_minime(self.root, self.root)
        module = self.root / "minime_autonomy/maintenance_next.py"
        module.parent.mkdir()
        module.write_text("HANDOFF_VERSION = 1\nraise RuntimeError('not imported')\n")
        guard.verify_minime(self.root, self.root)
        module.write_text("HANDOFF_VERSION = 0\n")
        with self.assertRaisesRegex(RuntimeError, "downgrade refused"):
            guard.verify_minime(self.root, self.root)
        module.write_text("HANDOFF_VERSION = 1\n")
        path = self.root / "runtime/next-handoff-v1.jsonl"
        path.write_bytes(b'{"body":{"version":2}}\n')
        with self.assertRaisesRegex(RuntimeError, "downgrade refused"):
            guard.verify_minime(self.root, self.root)
        path.write_bytes(b'{"body":[]}\n')
        with self.assertRaisesRegex(RuntimeError, "unsupported"):
            guard.verify_minime(self.root, self.root)

    def test_legacy_absence_explicit_but_unknown_contract_refuses(self):
        self.assertIsNone(LaunchdAgent.next_handoff_identity({}))
        with self.assertRaises(RuntimeError):
            LaunchdAgent.next_handoff_identity(dict(self.status, next_handoff_contract="future"))

    def test_unsealed_admission_prevents_stop_proof_and_requires_compatible_candidate(self):
        marker = proof.admission_path(self.root)
        value = dict(version=1, contract=proof.CONTRACT, kind="unsealed_admission", process=self.old,
                     state_sha256="a" * 64, pending_sha256="b" * 64)
        marker.write_bytes(proof.canonical(value))
        with self.assertRaisesRegex(RuntimeError, "admission remains unsealed"):
            proof.verify_seal(self.root, self.old, self.old_status)
        (self.root / "runtime/next-handoff-v1.jsonl").unlink()
        with self.assertRaisesRegex(RuntimeError, "downgrade refused"):
            guard.verify_minime(self.root, self.root)
        module = self.root / "minime_autonomy/maintenance_next.py"
        module.parent.mkdir()
        module.write_text("HANDOFF_VERSION = 1\nraise RuntimeError('not imported')\n")
        guard.verify_minime(self.root, self.root)
        marker.write_bytes(proof.canonical(dict(value, version=2)))
        with self.assertRaisesRegex(RuntimeError, "downgrade refused"):
            guard.verify_minime(self.root, self.root)
        marker.write_bytes(proof.canonical(dict(value, version=True)))
        with self.assertRaisesRegex(RuntimeError, "unsupported"):
            guard.verify_minime(self.root, self.root)

    def backend(self):
        backend = Backend()
        observe = backend.observe
        backend.observe = lambda pid: dict(observe(pid), next_handoff=self.old)
        backend.verify_next_handoff = lambda old, pid, inputs: proof.verify_restore(
            self.root, old, self.status, self.inputs)
        return backend

    def test_reload_demands_actual_ack_when_old_process_advertised_contract(self):
        result = self.backend().reload()
        self.assertFalse(result["next_handoff"]["execution_confirmed"])
        self.status.pop("next_handoff_restore")
        backend = self.backend()
        with self.assertRaisesRegex(RuntimeError, "acknowledgement absent"):
            backend.reload()
        self.assertEqual(backend.signals, [10])

    def test_identity_drift_at_signal_boundary_never_signals(self):
        backend = self.backend()
        observe = backend.observe

        def changed(pid):
            result = observe(pid)
            if backend.validations == 2:
                result["next_handoff"] = None
            return result

        backend.observe = changed
        with self.assertRaisesRegex(RuntimeError, "idle boundary moved"):
            backend.reload()
        self.assertEqual(backend.signals, [])


if __name__ == "__main__":
    unittest.main()
