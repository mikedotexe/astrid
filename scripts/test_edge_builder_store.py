#!/usr/bin/env python3
"""Focused tests for the immutable CPU-edge builder-store helper."""

from __future__ import annotations

import argparse
import importlib.machinery
import importlib.util
import json
import os
import pathlib
import sys
import tempfile
import types
import unittest
from unittest import mock


ROOT = pathlib.Path(__file__).resolve().parents[1]
HELPER = ROOT / "packaging/systemd/root/astrid-edge-builder-store"
sys.dont_write_bytecode = True
LOADER = importlib.machinery.SourceFileLoader("edge_builder_store", str(HELPER))
SPEC = importlib.util.spec_from_loader(LOADER.name, LOADER)
assert SPEC is not None
store = importlib.util.module_from_spec(SPEC)
LOADER.exec_module(store)


class BuilderStoreTests(unittest.TestCase):
    def test_fixed_size_and_mount_policy_are_not_soft_defaults(self) -> None:
        self.assertEqual(store.IMAGE_BYTES, 64 * 1024 * 1024 * 1024)
        self.assertEqual(store.HOST_RESERVE_BYTES, 64 * 1024 * 1024 * 1024)
        self.assertEqual(store.STORE_MINIMUM_FREE_BYTES, 8 * 1024 * 1024 * 1024)
        options = store._mount_options("rw,nodev,nosuid,noatime,seclabel")
        self.assertTrue(store.REQUIRED_OPTIONS.issubset(options))
        self.assertFalse(store.FORBIDDEN_OPTIONS.intersection(options))
        self.assertTrue(store.FORBIDDEN_OPTIONS.intersection(store._mount_options("rw,relatime")))

    def test_full_allocation_rejects_sparse_extent_accounting(self) -> None:
        sparse = types.SimpleNamespace(st_blocks=1, st_size=4096)
        allocated = types.SimpleNamespace(st_blocks=8, st_size=4096)
        self.assertFalse(store._fully_allocated(sparse))
        self.assertTrue(store._fully_allocated(allocated))

    def test_backing_identity_and_host_reserve_are_rechecked_exactly(self) -> None:
        identity = {
            "target": "/media/data",
            "source": "/dev/sda1",
            "fstype": "ext4",
            "uuid": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            "device": 9,
        }
        config = {
            "backing": identity,
            "required_backing_mount": "/media/data",
            "required_backing_uuid": identity["uuid"],
        }
        image = pathlib.Path("/media/data/builder.ext4")
        with mock.patch.object(store, "_backing_identity", return_value=identity), mock.patch.object(
            store, "_free_bytes", return_value=store.HOST_RESERVE_BYTES
        ):
            store._verify_backing(config, image)
        with mock.patch.object(
            store, "_backing_identity", return_value={**identity, "source": "/dev/sdb1"}
        ):
            with self.assertRaises(store.StoreError):
                store._verify_backing(config, image)
        with mock.patch.object(store, "_backing_identity", return_value=identity), mock.patch.object(
            store, "_free_bytes", return_value=store.HOST_RESERVE_BYTES - 1
        ):
            with self.assertRaises(store.StoreError):
                store._verify_backing(config, image)

    def test_exact_mount_rejects_wrong_uuid_options_and_backing_image(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            image = root / "builder.ext4"
            mount = root / "mount"
            image.write_bytes(b"image")
            mount.mkdir()
            config = {"mount": str(mount), "filesystem_uuid": "12345678-1234-4234-8234-123456789abc"}
            good_mount = {
                "target": str(mount),
                "source": "/dev/loop7",
                "fstype": "ext4",
                "uuid": config["filesystem_uuid"],
                "options": "rw,nodev,nosuid,noatime",
            }
            good_loop = json.dumps(
                {
                    "loopdevices": [
                        {
                            "name": "/dev/loop7",
                            "back-file": str(image),
                            "offset": 0,
                            "sizelimit": 0,
                            "ro": False,
                        }
                    ]
                }
            )
            with mock.patch.object(store, "_reject_symlink_ancestry"), mock.patch.object(
                store, "_findmnt", return_value=good_mount
            ), mock.patch.object(store, "_run", return_value=good_loop):
                self.assertEqual(store._verify_mount(config, image), mount)
            for replacement in (
                {**good_mount, "uuid": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"},
                {**good_mount, "options": "rw,nodev,nosuid,relatime"},
            ):
                with mock.patch.object(store, "_reject_symlink_ancestry"), mock.patch.object(
                    store, "_findmnt", return_value=replacement
                ):
                    with self.assertRaises(store.StoreError):
                        store._verify_mount(config, image)
            bad_loop = good_loop.replace(str(image), str(root / "other.ext4"))
            with mock.patch.object(store, "_reject_symlink_ancestry"), mock.patch.object(
                store, "_findmnt", return_value=good_mount
            ), mock.patch.object(store, "_run", return_value=bad_loop):
                with self.assertRaises(store.StoreError):
                    store._verify_mount(config, image)

    def test_power_loss_pending_cleanup_never_follows_a_link(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            target = root / "target"
            target.write_bytes(b"preserve")
            pending = root / ".builder.pending"
            pending.symlink_to(target)
            with self.assertRaises(store.StoreError):
                store._remove_recoverable_pending(pending)
            self.assertEqual(target.read_bytes(), b"preserve")

    def test_orphaned_exact_image_reconstructs_config_without_reformatting(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            image = root / "builder.ext4"
            mount = root / "mount"
            config_path = root / "builder.json"
            image.write_bytes(b"orphan")
            mount.mkdir()
            arguments = argparse.Namespace(
                config=str(config_path),
                image=str(image),
                mount=str(mount),
                builder_uid=1200,
                builder_gid=1200,
                required_backing_mount=None,
                required_backing_uuid=None,
            )
            metadata = types.SimpleNamespace(
                st_dev=9,
                st_ino=11,
                st_uid=0,
                st_gid=0,
                st_mode=0o100600,
                st_nlink=1,
                st_size=store.IMAGE_BYTES,
                st_blocks=store.IMAGE_BYTES // 512,
            )
            written: list[dict[str, object]] = []
            with (
                mock.patch.object(store, "_require_root_linux"),
                mock.patch.object(store, "_reject_symlink_ancestry"),
                mock.patch.object(store, "_require_secure_directory"),
                mock.patch.object(store, "_require_regular", return_value=metadata),
                mock.patch.object(store, "_remove_recoverable_pending", return_value=False),
                mock.patch.object(
                    store,
                    "_backing_identity",
                    return_value={
                        "target": "/",
                        "source": "/dev/test",
                        "fstype": "ext4",
                        "uuid": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                        "device": 9,
                    },
                ),
                mock.patch.object(
                    store,
                    "_blkid",
                    return_value={
                        "TYPE": "ext4",
                        "LABEL": store.FILESYSTEM_LABEL,
                        "UUID": "12345678-1234-4234-8234-123456789abc",
                    },
                ),
                mock.patch.object(store, "_mount_unit_name", return_value="tmp-mount.mount"),
                mock.patch.object(store, "_atomic_json", side_effect=lambda _path, value: written.append(value)),
                mock.patch.object(store, "_run") as run,
            ):
                store.initialize(arguments)
            self.assertEqual(len(written), 1)
            self.assertEqual(written[0]["image_inode"], 11)
            self.assertEqual(written[0]["mount_unit"], "tmp-mount.mount")
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
