"""Tiny synthetic files and stdlib-only evidence tests; no live data or services."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("maintenance_state_under_test", HERE / "reservoir_maintenance_state.py")
state = importlib.util.module_from_spec(spec)
spec.loader.exec_module(state)


def context():
    return {"transaction_id": "fake-transaction", "bindings": {
        "plan_sha256": "1" * 64, "provider_old_identity_sha256": "2" * 64,
        "reservoir_old_identity_sha256": "3" * 64, "final_snapshot_sha256": "4" * 64}}


def qos(seq, event, **fields):
    record = {"stream": "model_qos", "schema_version": 1, "arrival_sequence": seq,
              "lifecycle": event, "recorded_at": "2026-10-16T00:00:00+00:00", **fields}
    return dict(record, receipt_id="model-qos-" + state.canonical_hash(record))


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="reservoir-fake-evidence-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.ctx = context()

    def write(self, name, text):
        path = self.root / name
        path.write_text(text)
        return path

    def coverage(self, role, **paths):
        result = {"schema": "reservoir_maintenance_log_coverage_v1", "complete": True,
                  "process_identity_sha256": self.ctx["bindings"][role + "_old_identity_sha256"],
                  "old_process_absent": True, "start_worker_idle": True, "start_queue_empty": True,
                  "inventory_matches_start_boundary": True}
        for key, path in paths.items():
            _, reference = state.artifact(path)
            result[key] = {"device": 1, "inode": 2, "start_offset": 20,
                           "end_offset": 20 + reference["bytes"], "sha256": reference["sha256"]}
        return result

    def provider(self, log=None, records=None):
        log = log if log is not None else (
            "00:00:01 [coupled-astrid] received signal 15; draining active generation\n"
            "00:00:02 [coupled-astrid] generated 7 tokens on astrid in 0.1s (70 tok/s)\n"
            "00:00:02 [coupled-astrid] push_state: checked in astrid (+7 ticks, h_norms=[1, 2, 3])\n"
            "00:00:03 [coupled-astrid] shutting down — saving coupling journal\n")
        records = records if records is not None else [qos(1, "queued"), qos(1, "selected"),
                                                       qos(1, "completed", outcome="delivered")]
        log_path = self.write("provider.log", log)
        qos_path = self.write("qos.jsonl", "".join(json.dumps(item) + "\n" for item in records))
        journal_path = self.write("journal.json", json.dumps({"entries": [{"ticks": 7}]}))
        coverage = self.coverage("provider", log=log_path, qos=qos_path)
        return log_path, qos_path, coverage, journal_path

    def test_provider_pairs_work_with_explicit_pending_rejection(self):
        records = [qos(1, "queued"), qos(1, "selected"), qos(2, "queued"),
                   qos(1, "completed", outcome="response_discarded_after_active_disconnect"),
                   qos(2, "stopped_pending")]
        result = state.verify_provider_drain(self.ctx, *self.provider(records=records))
        self.assertTrue(result["passed"])
        self.assertEqual(result["evidence"]["matched_generations"][0]["tokens"], 7)
        self.assertEqual(result["evidence"]["request_outcomes"]["2"], "stopped_pending")

    def test_provider_empty_drain_requires_no_selected_work(self):
        text = ("00:00:01 [coupled-astrid] received signal 15; draining active generation\n"
                "00:00:02 [coupled-astrid] shutting down — saving coupling journal\n")
        result = state.verify_provider_drain(self.ctx, *self.provider(log=text, records=[]))
        self.assertEqual(result["evidence"]["matched_generations"], [])

    def test_missing_or_mismatched_acknowledgement_blocks(self):
        args = self.provider()
        original = args[0].read_text()
        for replacement in ("push_state failed for 'astrid': timeout", "push_state: checked in other (+7 ticks, h_norms=[1])",
                            "push_state: checked in astrid (+8 ticks, h_norms=[1])", "no acknowledgement"):
            with self.subTest(replacement=replacement):
                text = original.replace("push_state: checked in astrid (+7 ticks, h_norms=[1, 2, 3])", replacement)
                with self.assertRaises(ValueError):
                    state.verify_provider_drain(self.ctx, *self.provider(log=text))

    def test_journal_or_qos_success_does_not_replace_checkin(self):
        args = self.provider()
        text = "\n".join(line for line in args[0].read_text().splitlines()
                         if "generated" not in line and "checked in" not in line) + "\n"
        with self.assertRaisesRegex(ValueError, "count mismatch"):
            state.verify_provider_drain(self.ctx, *self.provider(log=text))

    def test_generation_error_duplicate_or_unresolved_qos_blocks(self):
        variants = [
            [qos(1, "queued"), qos(1, "selected"), qos(1, "completed", outcome="generation_error")],
            [qos(1, "queued"), qos(1, "selected"), qos(1, "selected")],
            [qos(1, "queued"), qos(1, "selected")],
            [qos(1, "selected"), qos(1, "completed", outcome="delivered")],
        ]
        for records in variants:
            with self.subTest(records=records), self.assertRaises(ValueError):
                state.verify_provider_drain(self.ctx, *self.provider(records=records))

    def test_coverage_identity_bytes_anchor_and_exit_are_required(self):
        for field, value in (("complete", False), ("old_process_absent", False),
                             ("start_worker_idle", False), ("start_queue_empty", False),
                             ("process_identity_sha256", "9" * 64)):
            args = self.provider()
            args[2][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                state.verify_provider_drain(self.ctx, *args)
        args = self.provider()
        args[2]["log"]["end_offset"] += 1
        with self.assertRaises(ValueError):
            state.verify_provider_drain(self.ctx, *args)

    def test_truncated_log_and_forged_qos_receipt_block(self):
        args = self.provider()
        with self.assertRaises(ValueError):
            state.verify_provider_drain(self.ctx, *self.provider(log=args[0].read_text().rstrip()))
        record = qos(1, "queued")
        record["arrival_sequence"] = 2
        with self.assertRaisesRegex(ValueError, "receipt hash"):
            state.verify_provider_drain(self.ctx, *self.provider(records=[record]))

    def final(self, text=None):
        text = text if text is not None else (
            "00:00:00 [reservoir] snapshot 'old' → state/old.npz\n"
            "00:00:01 [reservoir] created handle 'new' (entity=test)\n"
            "00:00:01 [reservoir] destroyed handle 'old'\n"
            "00:00:02 [reservoir] shutting down...\n"
            "00:00:03 [reservoir] snapshot 'new' → state/new.npz\n"
            "00:00:04 [reservoir] final snapshot complete\n"
            "00:00:05 [reservoir] goodbye\n")
        path = self.write("reservoir.log", text)
        inventory = {"type": "list_handles_response", "handles": [{"name": "old"}]}
        return path, inventory, self.coverage("reservoir", log=path)

    def test_final_set_reconciles_lifecycle_and_ignores_earlier_snapshot(self):
        result = state.verify_final_snapshot(self.ctx, *self.final())
        self.assertEqual(result["evidence"]["final_handles"], ["new"])
        self.assertEqual(len(result["evidence"]["lifecycle_changes"]), 2)

    def test_final_failure_missing_name_or_generic_complete_is_insufficient(self):
        text = self.final()[0].read_text()
        for replacement in ("snapshot_all failed for 'new': disk full", "", "snapshot 'wrong' → state/wrong.npz"):
            with self.subTest(replacement=replacement), self.assertRaises(ValueError):
                state.verify_final_snapshot(self.ctx, *self.final(text.replace("snapshot 'new' → state/new.npz", replacement)))

    def test_lifecycle_change_during_final_or_missing_end_blocks(self):
        text = self.final()[0].read_text()
        variants = [text.replace("final snapshot complete", "created handle 'late' (entity=test)"),
                    text.replace("[reservoir] goodbye", "[reservoir] still running")]
        for value in variants:
            with self.subTest(value=value), self.assertRaises(ValueError):
                state.verify_final_snapshot(self.ctx, *self.final(value))

    def fake_state(self):
        root = self.root / "state"
        root.mkdir()
        (root / "old.npz").write_bytes(b"fake old snapshot bytes")
        (root / "old_thermostats.json").write_text('[{"fake":1}]')
        (root / "history.txt").write_text("retain all history")
        (root / "invalid").mkdir()
        (root / "invalid" / "quarantined.npz").write_bytes(b"fake quarantined bytes")
        return root

    def barrier(self):
        return {"schema": "reservoir_maintenance_stopped_barrier_v1", "transaction_id": self.ctx["transaction_id"],
                "bindings": self.ctx["bindings"], "ownership_current": True,
                **{role: {"job_absent": True, "old_process_absent": True, "listener_absent": True}
                   for role in ("provider", "reservoir")}}

    def test_backup_preserves_all_bytes_and_timestamps(self):
        source = self.fake_state()
        before = state.inventory_tree(source)
        result = state.backup_stopped_tree(self.ctx, source, self.root / "backup", self.barrier())
        self.assertTrue(result["passed"])
        self.assertEqual(state.inventory_tree(source), before)
        for name, entry in before["entries"].items():
            copied = result["evidence"]["backup_inventory"]["entries"][name]
            if entry["kind"] == "file":
                self.assertEqual((copied["sha256"], copied["mtime_ns"]), (entry["sha256"], entry["mtime_ns"]))
        self.assertTrue((self.root / "backup/backup-receipt.json").is_file())
        self.assertIn("invalid/quarantined.npz", result["evidence"]["backup_inventory"]["entries"])

    def test_absence_and_ownership_required_before_backup_creation(self):
        source = self.fake_state()
        barrier = self.barrier()
        barrier["provider"]["job_absent"] = False
        with self.assertRaises(ValueError):
            state.backup_stopped_tree(self.ctx, source, self.root / "backup", barrier)
        self.assertFalse((self.root / "backup").exists())

    def test_backup_never_overwrites_or_nests_in_source(self):
        source = self.fake_state()
        destination = self.root / "backup"
        destination.mkdir()
        with self.assertRaises(FileExistsError):
            state.backup_stopped_tree(self.ctx, source, destination, self.barrier())
        with self.assertRaises(ValueError):
            state.backup_stopped_tree(self.ctx, source, source / "nested", self.barrier())

    def test_symlinks_hardlinks_and_oversized_files_rejected(self):
        source = self.fake_state()
        link = source / "link"
        link.symlink_to(source / "old.npz")
        with self.assertRaises(ValueError):
            state.inventory_tree(source)
        link.unlink()
        os.link(source / "old.npz", link)
        with self.assertRaises(ValueError):
            state.inventory_tree(source)
        link.unlink()
        with self.assertRaises(ValueError):
            state.inventory_tree(source, max_file_bytes=4)

    def test_special_artifact_is_rejected_without_waiting_for_a_writer(self):
        fifo = self.root / "fifo"
        os.mkfifo(fifo)
        with self.assertRaisesRegex(ValueError, "regular file"):
            state.artifact(fifo)

    def test_traversal_failure_is_not_silently_an_empty_directory(self):
        source = self.fake_state()

        def failed_walk(root, **kwargs):
            kwargs["onerror"](PermissionError("fake unreadable subtree"))
            yield root, [], []

        with patch.object(state.os, "walk", side_effect=failed_walk):
            with self.assertRaises(PermissionError):
                state.inventory_tree(source)

    def test_changed_source_keeps_partial_backup_without_success_receipt(self):
        source = self.fake_state()
        original_write = state._write_exclusive

        def write_then_change(path, data):
            original_write(path, data)
            if Path(path).name == "backup-intent.json":
                (source / "old.npz").write_bytes(b"changed fake state")

        with patch.object(state, "_write_exclusive", side_effect=write_then_change):
            with self.assertRaises(ValueError):
                state.backup_stopped_tree(self.ctx, source, self.root / "backup", self.barrier())
        self.assertTrue((self.root / "backup/backup-intent.json").exists())
        self.assertFalse((self.root / "backup/backup-receipt.json").exists())

    def test_interrupted_copy_retains_evidence_and_never_returns_success(self):
        source = self.fake_state()
        original_write = state._write_exclusive

        def interrupt(path, data):
            if Path(path).name == "old.npz":
                raise KeyboardInterrupt()
            original_write(path, data)

        with patch.object(state, "_write_exclusive", side_effect=interrupt):
            with self.assertRaises(KeyboardInterrupt):
                state.backup_stopped_tree(self.ctx, source, self.root / "backup", self.barrier())
        self.assertTrue((self.root / "backup/backup-intent.json").exists())
        self.assertFalse((self.root / "backup/backup-receipt.json").exists())

    def test_restore_interface_is_explicitly_not_acceptance(self):
        backup = state.backup_stopped_tree(self.ctx, self.fake_state(), self.root / "backup", self.barrier())
        receipt = state.prepare_restore_request(self.ctx, backup, source_sha256="5" * 64,
                    interpreter_sha256="6" * 64, configuration_sha256="7" * 64)
        self.assertIs(receipt["passed"], False)
        self.assertEqual(receipt["bindings"]["backup_sha256"], state.canonical_hash(backup))
        self.assertEqual(receipt["evidence"]["status"], "not_executed")


if __name__ == "__main__":
    unittest.main(verbosity=2)
