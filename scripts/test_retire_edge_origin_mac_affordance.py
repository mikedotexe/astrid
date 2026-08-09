#!/usr/bin/env python3

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import stat
import tempfile
import unittest
from pathlib import Path
from unittest import mock


MODULE_PATH = Path(__file__).with_name("retire_edge_origin_mac_affordance.py")
SPEC = importlib.util.spec_from_file_location("origin_mac_retirement", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class InjectedFault(RuntimeError):
    pass


class OriginMacRetirementTests(unittest.TestCase):
    def workspace(
        self,
    ) -> tuple[Path, Path, Path, tempfile.TemporaryDirectory[str]]:
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name).resolve()
        workspace = root / "home" / "default"
        operator = root / "operator"
        retirement = root / "retired-origin-mac-context"
        workspace.mkdir(parents=True)
        return workspace, operator, retirement, temporary

    def legacy(self) -> bytes:
        value = (
            Path(__file__).parents[1] / "packaging/headless/introspection-memory.md"
        ).read_bytes()
        self.assertIn(hashlib.sha256(value).hexdigest(), MODULE.KNOWN_LEGACY)
        return value

    def test_exact_known_legacy_file_is_copied_to_new_private_inode(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        legacy = self.legacy()
        digest = hashlib.sha256(legacy).hexdigest()
        source = workspace / "MEMORY.md"
        source.write_bytes(legacy)
        source_inode = source.stat().st_ino
        journal = workspace / "edge/journal/journal_1.md"
        journal.parent.mkdir(parents=True)
        journal.write_text("I once considered origin-mac but this is authored history.\n")

        receipt = MODULE.migrate(workspace, operator, retirement)

        self.assertFalse(source.exists())
        retired = retirement / f"MEMORY.md.{digest}"
        self.assertEqual(retired.read_bytes(), legacy)
        self.assertNotEqual(retired.stat().st_ino, source_inode)
        self.assertEqual(stat.S_IMODE(retired.stat().st_mode), 0o600)
        self.assertEqual(retired.stat().st_nlink, 1)
        self.assertTrue(journal.exists())
        self.assertEqual(receipt["schema"], MODULE.SCHEMA)
        self.assertFalse(receipt["historical_authored_files_modified"])
        canonical = json.loads((retirement / MODULE.RECEIPT_NAME).read_text())
        projection = json.loads((operator / MODULE.OPERATOR_RECEIPT_NAME).read_text())
        self.assertEqual(canonical, receipt)
        self.assertEqual(projection, receipt)
        self.assertTrue((retirement / MODULE.TRANSACTION_NAME).is_file())

    def test_open_source_descriptor_cannot_mutate_retired_copy(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        legacy = self.legacy()
        digest = hashlib.sha256(legacy).hexdigest()
        source = workspace / "MEMORY.md"
        source.write_bytes(legacy)
        descriptor = os.open(source, os.O_RDWR)
        try:
            MODULE.migrate(workspace, operator, retirement)
            os.lseek(descriptor, 0, os.SEEK_SET)
            os.write(descriptor, b"X" * len(legacy))
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        self.assertEqual((retirement / f"MEMORY.md.{digest}").read_bytes(), legacy)

    def test_unknown_advertising_prompt_fails_before_transaction(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        path = workspace / "AGENTS.md"
        path.write_text("Read introspections/origin-mac/surprise.txt\n")

        with self.assertRaisesRegex(MODULE.MigrationError, "unknown prompt/context"):
            MODULE.migrate(workspace, operator, retirement)

        self.assertTrue(path.exists())
        self.assertEqual(list(retirement.iterdir()), [])

    def test_preflight_failure_never_partially_retires_an_earlier_file(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        legacy = self.legacy()
        known = workspace / "AGENTS.md"
        unknown = workspace / "MEMORY.md"
        known.write_bytes(legacy)
        unknown.write_text("Read introspections/origin-mac/unknown.md\n")

        with self.assertRaisesRegex(MODULE.MigrationError, "unknown prompt/context"):
            MODULE.migrate(workspace, operator, retirement)

        self.assertEqual(known.read_bytes(), legacy)
        self.assertTrue(unknown.exists())
        self.assertEqual(list(retirement.iterdir()), [])

    def test_independent_prompt_is_preserved_but_not_permanently_bound(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        path = workspace / "AGENTS.md"
        path.write_text("This appliance has only local continuity.\n")

        receipt = MODULE.migrate(workspace, operator, retirement)

        self.assertTrue(path.exists())
        outcome = next(item for item in receipt["outcomes"] if item["path"] == "AGENTS.md")
        self.assertEqual(outcome["status"], "preserved_not_legacy")
        path.write_text("changed after the durable inventory\n")
        self.assertEqual(MODULE.migrate(workspace, operator, retirement), receipt)

    def test_previously_absent_candidate_can_appear_or_disappear_when_safe(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        receipt = MODULE.migrate(workspace, operator, retirement)
        candidate = workspace / "AGENTS.md"
        candidate.write_text("This is a new appliance-local prompt.\n")
        self.assertEqual(MODULE.migrate(workspace, operator, retirement), receipt)
        candidate.unlink()
        self.assertEqual(MODULE.migrate(workspace, operator, retirement), receipt)

    def test_later_unknown_origin_mac_reintroduction_still_fails_closed(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        MODULE.migrate(workspace, operator, retirement)
        candidate = workspace / "AGENTS.md"
        candidate.write_text("Read introspections/origin-mac/reintroduced.md\n")
        with self.assertRaisesRegex(MODULE.MigrationError, "unknown prompt/context"):
            MODULE.migrate(workspace, operator, retirement)

    def test_later_exact_known_origin_mac_reintroduction_still_fails_closed(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        MODULE.migrate(workspace, operator, retirement)
        (workspace / "AGENTS.md").write_bytes(self.legacy())
        with self.assertRaisesRegex(MODULE.MigrationError, "exact-known origin-mac"):
            MODULE.migrate(workspace, operator, retirement)

    def test_intermediate_candidate_symlink_is_rejected(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        outside = Path(temporary.name) / "outside"
        outside.mkdir()
        (outside / "MEMORY.md").write_text("not appliance-owned\n")
        (workspace / "edge").symlink_to(outside, target_is_directory=True)

        with self.assertRaisesRegex(MODULE.MigrationError, "candidate parent is linked"):
            MODULE.migrate(workspace, operator, retirement)

        self.assertTrue((outside / "MEMORY.md").exists())

    def test_hardlinked_source_is_rejected(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        source = workspace / "MEMORY.md"
        source.write_bytes(self.legacy())
        os.link(source, workspace / "alias")

        with self.assertRaisesRegex(MODULE.MigrationError, "linked or non-regular"):
            MODULE.migrate(workspace, operator, retirement)

        self.assertTrue(source.exists())

    def test_precreated_retirement_or_operator_root_cannot_forge_state(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        retirement.mkdir(mode=0o755)
        with self.assertRaisesRegex(MODULE.MigrationError, "unsafe identity or mode"):
            MODULE.migrate(workspace, operator, retirement)
        retirement.chmod(0o700)
        operator.mkdir(mode=0o700)
        with self.assertRaisesRegex(MODULE.MigrationError, "unsafe identity or mode"):
            MODULE.migrate(workspace, operator, retirement)

    def test_user_owned_ancestry_is_rejected_under_root_authority(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        retirement.mkdir(mode=0o700)
        with mock.patch.object(MODULE.os, "geteuid", return_value=0):
            with self.assertRaisesRegex(MODULE.MigrationError, "authority-controlled"):
                MODULE.migrate(workspace, operator, retirement, runtime_gid=123)

    def test_retirement_must_share_workspace_filesystem(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        original_fstat = MODULE.os.fstat
        retirement_inode: int | None = None

        def mismatched_fstat(descriptor: int):
            nonlocal retirement_inode
            result = original_fstat(descriptor)
            if retirement_inode is not None and result.st_ino == retirement_inode:
                values = list(result)
                values[2] = result.st_dev + 1
                return os.stat_result(values)
            return result

        original_ensure = MODULE.ensure_controlled_leaf_directory

        def capture(*args, **kwargs):
            nonlocal retirement_inode
            descriptor = original_ensure(*args, **kwargs)
            if args[0] == retirement:
                retirement_inode = original_fstat(descriptor).st_ino
            return descriptor

        with mock.patch.object(MODULE, "ensure_controlled_leaf_directory", capture), mock.patch.object(
            MODULE.os, "fstat", mismatched_fstat
        ):
            with self.assertRaisesRegex(MODULE.MigrationError, "workspace filesystem"):
                MODULE.migrate(workspace, operator, retirement)

    def test_second_run_verifies_exact_retirement_idempotently(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        legacy = self.legacy()
        digest = hashlib.sha256(legacy).hexdigest()
        (workspace / "MEMORY.md").write_bytes(legacy)

        first = MODULE.migrate(workspace, operator, retirement)
        second = MODULE.migrate(workspace, operator, retirement)

        self.assertEqual(first, second)
        self.assertEqual((retirement / f"MEMORY.md.{digest}").read_bytes(), legacy)

    def test_missing_operator_projection_recovers_from_canonical_receipt(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        (workspace / "MEMORY.md").write_bytes(self.legacy())
        receipt = MODULE.migrate(workspace, operator, retirement)
        (operator / MODULE.OPERATOR_RECEIPT_NAME).unlink()

        recovered = MODULE.migrate(workspace, operator, retirement)

        self.assertEqual(recovered, receipt)
        self.assertEqual(
            json.loads((operator / MODULE.OPERATOR_RECEIPT_NAME).read_text()), receipt
        )

    def test_prior_receipt_or_artifact_tampering_fails_closed(self) -> None:
        for target in ("canonical", "operator", "artifact", "missing_artifact"):
            with self.subTest(target=target):
                workspace, operator, retirement, temporary = self.workspace()
                self.addCleanup(temporary.cleanup)
                legacy = self.legacy()
                digest = hashlib.sha256(legacy).hexdigest()
                (workspace / "MEMORY.md").write_bytes(legacy)
                MODULE.migrate(workspace, operator, retirement)
                if target == "canonical":
                    path = retirement / MODULE.RECEIPT_NAME
                    path.chmod(0o600)
                    path.write_text("{}\n")
                elif target == "operator":
                    path = operator / MODULE.OPERATOR_RECEIPT_NAME
                    path.chmod(0o640)
                    path.write_text("{}\n")
                elif target == "artifact":
                    path = retirement / f"MEMORY.md.{digest}"
                    path.chmod(0o600)
                    path.write_bytes(b"tampered")
                else:
                    (retirement / f"MEMORY.md.{digest}").unlink()
                with self.assertRaises(MODULE.MigrationError):
                    MODULE.migrate(workspace, operator, retirement)

    def test_hardlinked_retired_artifact_fails_closed(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        legacy = self.legacy()
        digest = hashlib.sha256(legacy).hexdigest()
        (workspace / "MEMORY.md").write_bytes(legacy)
        MODULE.migrate(workspace, operator, retirement)
        artifact = retirement / f"MEMORY.md.{digest}"
        outside = Path(temporary.name) / "forged-source"
        outside.write_bytes(legacy)
        artifact.unlink()
        os.link(outside, artifact)

        with self.assertRaisesRegex(MODULE.MigrationError, "linked or non-regular"):
            MODULE.migrate(workspace, operator, retirement)

    def test_missing_canonical_receipt_with_prior_projection_fails(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        (workspace / "MEMORY.md").write_bytes(self.legacy())
        MODULE.migrate(workspace, operator, retirement)
        (retirement / MODULE.RECEIPT_NAME).unlink()
        with self.assertRaisesRegex(MODULE.MigrationError, "canonical retirement receipt is missing"):
            MODULE.migrate(workspace, operator, retirement)

    def test_path_swap_before_unlink_does_not_delete_replacement(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        source = workspace / "MEMORY.md"
        saved = workspace / "saved-original"
        source.write_bytes(self.legacy())

        def fault(stage: str) -> None:
            if stage == "before_source_unlink_identity_recheck":
                source.rename(saved)
                source.write_text("replacement must survive\n")

        with self.assertRaisesRegex(MODULE.MigrationError, "path changed before unlink"):
            MODULE.migrate(workspace, operator, retirement, fault=fault)

        self.assertEqual(source.read_text(), "replacement must survive\n")
        self.assertEqual(saved.read_bytes(), self.legacy())

    def test_path_swap_after_identity_recheck_restores_replacement(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        source = workspace / "MEMORY.md"
        saved = workspace / "saved-original"
        source.write_bytes(self.legacy())

        def fault(stage: str) -> None:
            if stage == "after_source_identity_recheck_before_move":
                source.rename(saved)
                source.write_text("replacement must be restored\n")

        with self.assertRaisesRegex(MODULE.MigrationError, "source disposal is not exact"):
            MODULE.migrate(workspace, operator, retirement, fault=fault)

        self.assertEqual(source.read_text(), "replacement must be restored\n")
        self.assertEqual(saved.read_bytes(), self.legacy())

    def test_symlink_swap_after_identity_recheck_is_restored_not_followed(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        source = workspace / "MEMORY.md"
        saved = workspace / "saved-original"
        outside = Path(temporary.name) / "outside"
        outside.write_text("outside must remain untouched\n")
        source.write_bytes(self.legacy())

        def fault(stage: str) -> None:
            if stage == "after_source_identity_recheck_before_move":
                source.rename(saved)
                source.symlink_to(outside)

        with self.assertRaisesRegex(MODULE.MigrationError, "source disposal is not exact"):
            MODULE.migrate(workspace, operator, retirement, fault=fault)

        self.assertTrue(source.is_symlink())
        self.assertEqual(source.resolve(), outside.resolve())
        self.assertEqual(outside.read_text(), "outside must remain untouched\n")
        self.assertEqual(saved.read_bytes(), self.legacy())

    def test_every_durable_fault_boundary_recovers_idempotently(self) -> None:
        stages = (
            "transaction_pending_fsynced",
            "transaction_published",
            "artifact_copy_fsynced",
            "artifact_published_with_staging_link",
            "source_moved_and_parent_fsynced",
            "source_disposal_unlinked",
            "canonical_receipt_pending_fsynced",
            "canonical_receipt_published",
            "operator_receipt_pending_fsynced",
            "operator_receipt_published",
        )
        for selected in stages:
            with self.subTest(stage=selected):
                workspace, operator, retirement, temporary = self.workspace()
                self.addCleanup(temporary.cleanup)
                legacy = self.legacy()
                digest = hashlib.sha256(legacy).hexdigest()
                (workspace / "MEMORY.md").write_bytes(legacy)
                seen = False

                def fault(stage: str) -> None:
                    nonlocal seen
                    if not seen and stage == selected:
                        seen = True
                        raise InjectedFault(selected)

                with self.assertRaises(InjectedFault):
                    MODULE.migrate(workspace, operator, retirement, fault=fault)
                self.assertTrue(seen)
                receipt = MODULE.migrate(workspace, operator, retirement)
                self.assertEqual(receipt["completion_status"], "durable_retirement_committed")
                self.assertEqual(
                    (retirement / f"MEMORY.md.{digest}").read_bytes(), legacy
                )
                self.assertFalse((workspace / "MEMORY.md").exists())

    def test_tampered_pending_transaction_is_never_promoted(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        (workspace / "MEMORY.md").write_bytes(self.legacy())

        def fault(stage: str) -> None:
            if stage == "transaction_pending_fsynced":
                raise InjectedFault(stage)

        with self.assertRaises(InjectedFault):
            MODULE.migrate(workspace, operator, retirement, fault=fault)
        pending = retirement / MODULE.TRANSACTION_PENDING_NAME
        pending.chmod(0o600)
        pending.write_text("{}\n")
        with self.assertRaisesRegex(MODULE.MigrationError, "hash is malformed"):
            MODULE.migrate(workspace, operator, retirement)
        self.assertTrue((workspace / "MEMORY.md").exists())

    def test_unexpected_retirement_entry_fails_closed(self) -> None:
        workspace, operator, retirement, temporary = self.workspace()
        self.addCleanup(temporary.cleanup)
        retirement.mkdir(mode=0o700)
        (retirement / "untrusted").write_text("unexpected\n")

        with self.assertRaisesRegex(MODULE.MigrationError, "without a durable transaction"):
            MODULE.migrate(workspace, operator, retirement)


if __name__ == "__main__":
    unittest.main()
