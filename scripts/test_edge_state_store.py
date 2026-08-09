#!/usr/bin/env python3
"""Focused tests for the immutable CPU-edge runtime/rollback store."""

from __future__ import annotations

import argparse
import hashlib
import importlib.machinery
import importlib.util
import json
import os
import pathlib
import stat
import sys
import tempfile
import types
import unittest
from unittest import mock


ROOT = pathlib.Path(__file__).resolve().parents[1]
HELPER = ROOT / "packaging/systemd/root/astrid-edge-state-store"
sys.dont_write_bytecode = True
LOADER = importlib.machinery.SourceFileLoader("edge_state_store", str(HELPER))
SPEC = importlib.util.spec_from_loader(LOADER.name, LOADER)
assert SPEC is not None
store = importlib.util.module_from_spec(SPEC)
LOADER.exec_module(store)


def volume(role: str, root: pathlib.Path, *, filesystem_uuid: str) -> dict[str, object]:
    label = store.RUNTIME_LABEL if role == "runtime" else store.ROLLBACK_LABEL
    return {
        "role": role,
        "image": str(root / f"{role}.ext4"),
        "mount": str(root / f"{role}-mount"),
        "image_bytes": store.IMAGE_BYTES,
        "filesystem_uuid": filesystem_uuid,
        "filesystem_label": label,
        "image_device": 7,
        "image_inode": 11 if role == "runtime" else 12,
        "reserved_block_percent": (
            store.RUNTIME_RESERVED_PERCENT if role == "runtime" else store.ROLLBACK_RESERVED_PERCENT
        ),
        "inode_reserve_files": store.INODE_RESERVE_FILES if role == "runtime" else 0,
        "block_count": 8_388_608,
        "reserved_block_count": 1_677_721 if role == "runtime" else 0,
        "block_size": 4096,
        "mount_unit": f"tmp-{role}\x2dmount.mount",
        "required_mount_options": sorted(store.REQUIRED_OPTIONS),
    }


def config(root: pathlib.Path) -> dict[str, object]:
    runtime = volume(
        "runtime", root, filesystem_uuid="12345678-1234-4234-8234-123456789abc"
    )
    rollback = volume(
        "rollback", root, filesystem_uuid="aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
    )
    return {
        "schema": store.SCHEMA,
        "appliance_id": "fixture",
        "runtime_uid": os.getuid() or 501,
        "runtime_gid": os.getgid() or 20,
        "runtime": runtime,
        "rollback": rollback,
        "backing": {
            "target": "/",
            "source": "/dev/test",
            "fstype": "ext4",
            "uuid": "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            "device": 7,
            "runtime_image_parent_inode": 2,
            "rollback_image_parent_inode": 2,
        },
        "required_backing_mount": "",
        "required_backing_uuid": "",
        "host_reserve_bytes": store.HOST_RESERVE_BYTES,
        "store_minimum_free_bytes": store.STORE_MINIMUM_FREE_BYTES,
        "migration_journal": str(root / "migration.json"),
        "runtime_source_backup": str(root / "runtime-backup"),
        "rollback_source_backup": str(root / "rollback-backup"),
        "runtime_inventory": str(root / "runtime.inventory.json"),
        "rollback_inventory": str(root / "rollback.inventory.json"),
        "python": {
            "path": str(store.PYTHON_PATH),
            "sha256": hashlib.sha256(store.PYTHON_PATH.read_bytes()).hexdigest(),
        },
    }


class StateStoreTests(unittest.TestCase):
    def test_mount_recover_verify_order_has_no_dependency_cycle(self) -> None:
        recover = (ROOT / "packaging/systemd/astrid-edge-state-store-recover.service.in").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            "Requires=@@RUNTIME_MOUNT_UNIT@@ @@ROLLBACK_MOUNT_UNIT@@\n",
            recover,
        )
        self.assertIn(
            "After=@@RUNTIME_MOUNT_UNIT@@ @@ROLLBACK_MOUNT_UNIT@@\n",
            recover,
        )
        self.assertIn("DeviceAllow=block-loop r\n", recover)
        self.assertEqual(
            [line for line in recover.splitlines() if line.startswith("DeviceAllow=")],
            ["DeviceAllow=block-loop r"],
        )
        capability = next(
            line for line in recover.splitlines() if line.startswith("CapabilityBoundingSet=")
        )
        for forbidden in ("SYS_ADMIN", "MKNOD", "SYS_RAWIO"):
            self.assertNotIn(forbidden, capability)
        self.assertIn("SystemCallFilter=~", recover)
        self.assertIn("@mount", recover)
        self.assertIn("@raw-io", recover)
        migration = (
            ROOT / "packaging/systemd/astrid-edge-state-store-migration-recover.service.in"
        ).read_text(encoding="utf-8")
        self.assertIn("Before=@@RUNTIME_MOUNT_UNIT@@ @@ROLLBACK_MOUNT_UNIT@@\n", migration)
        self.assertIn("PrivateDevices=yes\n", migration)
        self.assertIn("PrivateTmp=no\n", migration)
        self.assertNotIn("PrivateTmp=yes\n", migration)
        for name in (
            "astrid-edge-state-store-runtime.mount.in",
            "astrid-edge-state-store-rollback.mount.in",
        ):
            mount = (ROOT / "packaging/systemd/root" / name).read_text(encoding="utf-8")
            self.assertNotIn("Requires=astrid-edge-state-store-recover.service", mount)
            self.assertNotIn("After=astrid-edge-state-store-recover.service", mount)
            self.assertIn("Requires=astrid-edge-state-store-migration-recover.service\n", mount)
            self.assertIn("After=astrid-edge-state-store-migration-recover.service\n", mount)
            self.assertIn(
                "Before=astrid-edge-state-store-recover.service "
                "astrid-edge-state-store-verify.service\n",
                mount,
            )

    def test_block_identity_commands_are_read_only_inspection_forms(self) -> None:
        with mock.patch.object(
            store,
            "_run",
            return_value='{"loopdevices":[{"name":"/dev/loop7","back-file":"/state.ext4","offset":0,"sizelimit":0,"ro":false}]}',
        ) as invoked:
            store._loop_for_source("/dev/loop7")
            self.assertEqual(
                invoked.call_args.args[0],
                [
                    "/usr/sbin/losetup",
                    "--json",
                    "--output",
                    "NAME,BACK-FILE,OFFSET,SIZELIMIT,RO",
                    "/dev/loop7",
                ],
            )
        with mock.patch.object(store, "_run", return_value="TYPE=ext4\n") as invoked:
            store._blkid(pathlib.Path("/state.ext4"))
            self.assertEqual(
                invoked.call_args.args[0],
                ["/usr/sbin/blkid", "-p", "-o", "export", "/state.ext4"],
            )
        superblock = "Block count: 10\nReserved block count: 2\nBlock size: 4096\n"
        with mock.patch.object(store, "_run", return_value=superblock) as invoked:
            store._ext4_superblock(pathlib.Path("/state.ext4"))
            self.assertEqual(
                invoked.call_args.args[0],
                ["/usr/sbin/dumpe2fs", "-h", "/state.ext4"],
            )

    def test_capacity_and_mount_policy_are_fixed_not_soft_defaults(self) -> None:
        self.assertEqual(store.IMAGE_BYTES, 34_359_738_368)
        self.assertEqual(store.HOST_RESERVE_BYTES, 68_719_476_736)
        self.assertEqual(store.STORE_MINIMUM_FREE_BYTES, 4_294_967_296)
        options = store._mount_options("rw,nodev,nosuid,noexec,noatime,seclabel")
        self.assertTrue(store.REQUIRED_OPTIONS.issubset(options))
        self.assertFalse(store.FORBIDDEN_OPTIONS.intersection(options))
        for unsafe in ("exec", "suid", "dev", "relatime", "ro"):
            self.assertTrue(store.FORBIDDEN_OPTIONS.intersection({unsafe}))

    def test_full_allocation_rejects_sparse_image(self) -> None:
        self.assertFalse(store._fully_allocated(types.SimpleNamespace(st_blocks=1, st_size=4096)))
        self.assertTrue(store._fully_allocated(types.SimpleNamespace(st_blocks=8, st_size=4096)))

    def test_volume_schema_rejects_duplicate_or_wrong_identity(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            value = volume(
                "runtime", root, filesystem_uuid="12345678-1234-4234-8234-123456789abc"
            )
            with mock.patch.object(store, "_mount_unit_name", return_value=value["mount_unit"]):
                store._validate_volume(value, role="runtime", label=store.RUNTIME_LABEL)
                for key, replacement in (
                    ("image_bytes", store.IMAGE_BYTES - 1),
                    ("filesystem_label", store.ROLLBACK_LABEL),
                    ("required_mount_options", ["rw"]),
                    ("filesystem_uuid", "not-a-uuid"),
                ):
                    broken = {**value, key: replacement}
                    with self.assertRaises(store.StoreError, msg=key):
                        store._validate_volume(
                            broken, role="runtime", label=store.RUNTIME_LABEL
                        )

    def test_exact_image_rejects_symlink_inode_sparse_and_wrong_uuid(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            image = root / "runtime.ext4"
            image.write_bytes(b"fixture")
            value = volume(
                "runtime", root, filesystem_uuid="12345678-1234-4234-8234-123456789abc"
            )
            metadata = types.SimpleNamespace(
                st_dev=7,
                st_ino=11,
                st_uid=0,
                st_mode=stat.S_IFREG | 0o600,
                st_nlink=1,
                st_size=store.IMAGE_BYTES,
                st_blocks=store.IMAGE_BYTES // 512,
            )
            with (
                mock.patch.object(store, "_reject_symlink_ancestry"),
                mock.patch.object(store, "_require_regular", return_value=metadata),
                mock.patch.object(
                    store,
                    "_blkid",
                    return_value={
                        "TYPE": "ext4",
                        "LABEL": store.RUNTIME_LABEL,
                        "UUID": value["filesystem_uuid"],
                    },
                ),
                mock.patch.object(
                    store,
                    "_ext4_superblock",
                    return_value={
                        "block_count": value["block_count"],
                        "reserved_block_count": value["reserved_block_count"],
                        "block_size": value["block_size"],
                    },
                ),
            ):
                self.assertEqual(store._verify_image(value), image)
            with mock.patch.object(store, "_reject_symlink_ancestry"), mock.patch.object(
                store,
                "_require_regular",
                return_value=types.SimpleNamespace(**{**metadata.__dict__, "st_ino": 99}),
            ):
                with self.assertRaises(store.StoreError):
                    store._verify_image(value)
            target = root / "target"
            target.write_bytes(b"preserve")
            image.unlink()
            image.symlink_to(target)
            with self.assertRaises(store.StoreError):
                store._reject_symlink_ancestry(image, allow_leaf_absent=False)

    def test_exact_mount_rejects_wrong_options_loop_image_uuid_link_and_same_device(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            image = root / "runtime.ext4"
            mount = root / "runtime-mount"
            image.write_bytes(b"image")
            mount.mkdir()
            value = volume(
                "runtime", root, filesystem_uuid="12345678-1234-4234-8234-123456789abc"
            )
            observed = {
                "target": str(mount),
                "source": "/dev/loop7",
                "fstype": "ext4",
                "uuid": value["filesystem_uuid"],
                "options": "rw,nodev,nosuid,noexec,noatime",
                "maj:min": f"{os.major(mount.stat().st_dev)}:{os.minor(mount.stat().st_dev)}",
            }
            loop = {
                "name": "/dev/loop7",
                "back-file": str(image),
                "offset": 0,
                "sizelimit": 0,
                "ro": False,
            }
            with (
                mock.patch.object(store, "_reject_symlink_ancestry"),
                mock.patch.object(store, "_findmnt", return_value=observed),
                mock.patch.object(store, "_loop_for_source", return_value=loop),
                mock.patch.object(store, "_uuid_device", return_value=pathlib.Path("/dev/loop7")),
            ):
                verified_mount, verified_loop, _ = store._verify_mount(value, image)
                self.assertEqual(verified_mount, mount)
                self.assertEqual(verified_loop, "/dev/loop7")
            wrong = {**observed, "options": "rw,nodev,nosuid,noexec,relatime"}
            with mock.patch.object(store, "_reject_symlink_ancestry"), mock.patch.object(
                store, "_findmnt", return_value=wrong
            ):
                with self.assertRaises(store.StoreError):
                    store._verify_mount(value, image)
            wrong_loop = {**loop, "back-file": str(root / "other.ext4")}
            with (
                mock.patch.object(store, "_reject_symlink_ancestry"),
                mock.patch.object(store, "_findmnt", return_value=observed),
                mock.patch.object(store, "_loop_for_source", return_value=wrong_loop),
                mock.patch.object(store, "_uuid_device", return_value=pathlib.Path("/dev/loop7")),
            ):
                with self.assertRaises(store.StoreError):
                    store._verify_mount(value, image)
            with (
                mock.patch.object(store, "_reject_symlink_ancestry"),
                mock.patch.object(store, "_findmnt", return_value=observed),
                mock.patch.object(store, "_loop_for_source", return_value=loop),
                mock.patch.object(store, "_uuid_device", return_value=pathlib.Path("/dev/loop8")),
            ):
                with self.assertRaises(store.StoreError):
                    store._verify_mount(value, image)

    def test_backing_identity_pins_both_parent_inodes_and_64gib_reserve(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            value = config(root)
            identity = {
                "target": "/",
                "source": "/dev/test",
                "fstype": "ext4",
                "uuid": value["backing"]["uuid"],
                "device": 7,
                "directory_inode": 2,
            }
            with mock.patch.object(store, "_backing_identity", return_value=identity), mock.patch.object(
                store, "_free_bytes", return_value=store.HOST_RESERVE_BYTES
            ):
                store._verify_backing(value)
            tampered = {**identity, "directory_inode": 3}
            with mock.patch.object(
                store, "_backing_identity", side_effect=[identity, tampered]
            ):
                with self.assertRaises(store.StoreError):
                    store._verify_backing(value)
            with mock.patch.object(store, "_backing_identity", return_value=identity), mock.patch.object(
                store, "_free_bytes", return_value=store.HOST_RESERVE_BYTES - 1
            ):
                with self.assertRaises(store.StoreError):
                    store._verify_backing(value)

    def test_inventory_and_copy_are_deterministic_and_include_safe_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            source = root / "source"
            destination = root / "destination"
            source.mkdir()
            destination.mkdir()
            (source / "owned").mkdir()
            (source / "owned/value.txt").write_text("Astrid\n", encoding="utf-8")
            (source / "alias").symlink_to("owned/value.txt")
            first = store._inventory(source)
            store._copy_tree(source, destination)
            second = store._inventory(destination)
            self.assertEqual(first, second)
            self.assertEqual(
                [entry["kind"] for entry in first["entries"]],
                ["symlink", "directory", "file"],
            )
            store._verify_inventory(first)
            broken = {**first, "total_file_bytes": first["total_file_bytes"] + 1}
            with self.assertRaises(store.StoreError):
                store._verify_inventory(broken)

    def test_inventory_rejects_traversal_symlink_and_special_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            source = root / "source"
            source.mkdir()
            (source / "escape").symlink_to("../outside")
            with self.assertRaises(store.StoreError):
                store._inventory(source)
            (source / "escape").unlink()
            fifo = source / "pipe"
            os.mkfifo(fifo)
            with self.assertRaises(store.StoreError):
                store._inventory(source)

    def test_root_only_inode_reserve_recovers_inodes_and_detects_tamper(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            with (
                mock.patch.object(store, "INODE_RESERVE_SHARDS", 2),
                mock.patch.object(store, "INODE_RESERVE_FILES_PER_SHARD", 2),
                mock.patch.object(store, "INODE_RESERVE_FILES", 4),
                mock.patch.object(store, "ROOT_UID", os.getuid()),
                mock.patch.object(store, "ROOT_GID", os.getgid()),
            ):
                store._create_inode_reserve(root)
                store._verify_inode_reserve(root)
                reserve = root / store.INODE_RESERVE_NAME
                self.assertEqual(sum(1 for path in reserve.rglob("*") if path.is_file()), 4)
                member = reserve / "00/00"
                member.chmod(0o600)
                with self.assertRaises(store.StoreError):
                    store._verify_inode_reserve(root)
                member.chmod(0o400)
                member.unlink()
                (reserve / "01/00").unlink()
                (reserve / "01/01").unlink()
                (reserve / "01").rmdir()
                store._create_inode_reserve(root)
                store._verify_inode_reserve(root)
                (reserve / "unexpected").write_bytes(b"")
                with self.assertRaises(store.StoreError):
                    store._create_inode_reserve(root)
                (reserve / "unexpected").unlink()
                store._release_inode_reserve(root)
                self.assertFalse(reserve.exists())

    def test_boot_reserve_creation_is_separate_from_strict_verification(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            arguments = argparse.Namespace(config=str(root / "config.json"))
            value = config(root)
            with (
                mock.patch.object(store, "_require_root_linux"),
                mock.patch.object(store, "_read_config", return_value=value),
                mock.patch.object(store, "_mounted_runtime_for_boot_reserve", return_value=root),
                mock.patch.object(store, "INODE_RESERVE_SHARDS", 2),
                mock.patch.object(store, "INODE_RESERVE_FILES_PER_SHARD", 2),
                mock.patch.object(store, "INODE_RESERVE_FILES", 4),
                mock.patch.object(store, "ROOT_UID", os.getuid()),
                mock.patch.object(store, "ROOT_GID", os.getgid()),
            ):
                with self.assertRaises(FileNotFoundError):
                    store.verify_inode_reserve_at_boot(arguments)
                self.assertFalse((root / store.INODE_RESERVE_NAME).exists())
                store.recover_inode_reserve_at_boot(arguments)
                store.verify_inode_reserve_at_boot(arguments)
                self.assertTrue((root / store.INODE_RESERVE_NAME).is_dir())

    def test_sqlite_quick_check_distinguishes_plain_extension_and_corruption(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            plain = root / "plain.db"
            plain.write_text("not sqlite", encoding="ascii")
            self.assertEqual(store._sqlite_quick_checks(root), {"plain.db": "not_sqlite"})
            corrupt = root / "corrupt.sqlite3"
            corrupt.write_bytes(b"SQLite format 3\x00" + b"corrupt")
            with self.assertRaises(store.StoreError):
                store._sqlite_quick_checks(root)

    def test_recover_ready_requires_both_complete_read_only_backups(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            value = config(root)
            config_path = root / "config.json"
            store._atomic_json(config_path, value, mode=0o400)
            for role in ("runtime", "rollback"):
                pathlib.Path(value[role]["mount"]).mkdir()
                backup = pathlib.Path(value[f"{role}_source_backup"])
                backup.mkdir()
                backup.chmod(0o500)
            store._atomic_json(
                pathlib.Path(value["migration_journal"]),
                {
                    "schema": store.JOURNAL_SCHEMA,
                    "appliance_id": value["appliance_id"],
                    "phase": "ready_to_mount",
                },
            )
            with (
                mock.patch.object(store, "_require_root_linux"),
                mock.patch.object(store, "_reject_symlink_ancestry"),
                mock.patch.object(
                    store, "_require_regular", side_effect=lambda path, **_kwargs: path.lstat()
                ),
                mock.patch.object(store, "_mount_unit_name", side_effect=[
                    value["runtime"]["mount_unit"],
                    value["rollback"]["mount_unit"],
                ]),
            ):
                store.recover(argparse.Namespace(config=str(config_path)))

    def test_power_loss_recovery_restores_complete_old_tree_and_fails_boot(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            value = config(root)
            config_path = root / "config.json"
            store._atomic_json(config_path, value, mode=0o400)
            runtime_mount = pathlib.Path(value["runtime"]["mount"])
            runtime_mount.mkdir()
            runtime_backup = pathlib.Path(value["runtime_source_backup"])
            runtime_backup.mkdir()
            (runtime_backup / "state.json").write_text("complete", encoding="ascii")
            rollback_mount = pathlib.Path(value["rollback"]["mount"])
            rollback_mount.mkdir()
            rollback_backup = pathlib.Path(value["rollback_source_backup"])
            rollback_backup.mkdir()
            (rollback_backup / "snapshot.json").write_text("complete", encoding="ascii")
            store._atomic_json(
                pathlib.Path(value["migration_journal"]),
                {
                    "schema": store.JOURNAL_SCHEMA,
                    "appliance_id": value["appliance_id"],
                    "phase": "rollback_source_replaced",
                },
            )
            with (
                mock.patch.object(store, "_require_root_linux"),
                mock.patch.object(store, "_reject_symlink_ancestry"),
                mock.patch.object(
                    store, "_require_regular", side_effect=lambda path, **_kwargs: path.lstat()
                ),
                mock.patch.object(store, "_mount_unit_name", side_effect=[
                    value["runtime"]["mount_unit"],
                    value["rollback"]["mount_unit"],
                ]),
            ):
                with self.assertRaises(store.StoreError):
                    store.recover(argparse.Namespace(config=str(config_path)))
            self.assertEqual((runtime_mount / "state.json").read_text(), "complete")
            self.assertEqual((rollback_mount / "snapshot.json").read_text(), "complete")
            journal = json.loads(pathlib.Path(value["migration_journal"]).read_text())
            self.assertEqual(journal["phase"], "aborted_complete_old_state_restored")

    def test_units_express_noexec_capacity_gate_order_and_bounded_logs(self) -> None:
        runtime_mount = (
            ROOT / "packaging/systemd/root/astrid-edge-state-store-runtime.mount.in"
        ).read_text()
        rollback_mount = (
            ROOT / "packaging/systemd/root/astrid-edge-state-store-rollback.mount.in"
        ).read_text()
        verify = (
            ROOT / "packaging/systemd/astrid-edge-state-store-verify.service.in"
        ).read_text()
        bounded = (ROOT / "packaging/systemd/astrid-edge-bounded-state.conf.in").read_text()
        for body in (runtime_mount, rollback_mount):
            self.assertIn("Options=loop,rw,nodev,nosuid,noexec,noatime", body)
            self.assertIn("astrid-edge-state-store-recover.service", body)
        for unit in (
            "astrid-edge-generation-guard.service",
            "astrid-edge-self-change-supervisor.service",
            "astrid-edge-steward.service",
            "astrid.service",
            "astrid-edge-runtime.service",
        ):
            self.assertIn(unit, verify)
        self.assertIn("LimitCORE=0", bounded)
        self.assertIn("LogRateLimitIntervalSec=30s", bounded)
        self.assertIn("LogRateLimitBurst=200", bounded)
        # All directives used here predate systemd 249 on ICP.
        self.assertNotIn("ProtectProc=ptraceable", verify)


if __name__ == "__main__":
    unittest.main()
