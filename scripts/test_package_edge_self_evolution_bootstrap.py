#!/usr/bin/env python3
"""Adversarial tests for the complete CPU-edge self-evolution release bundle."""

from __future__ import annotations

import gzip
import hashlib
import importlib.util
import io
import json
import os
import stat
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock


SCRIPT = Path(__file__).with_name("package_edge_self_evolution_bootstrap.py")
SPEC = importlib.util.spec_from_file_location("edge_bootstrap_packager", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(module)


class BootstrapPackageTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.inputs = self.root / "inputs"
        self.inputs.mkdir()
        self.output = self.root / "output"
        self.output.mkdir()
        self.version = "0.9.0-test"
        self.target = "x86_64-unknown-linux-gnu"
        self.key = self.inputs / "portable.key"
        self.key.write_bytes(b"k" * 32)
        self.key.chmod(0o600)
        self.supervisor = self.inputs / "supervisor.pyz"
        self.supervisor.write_bytes(b"#!/usr/bin/env python3\nfixture\n")
        self.supervisor.chmod(0o500)
        self.installer = self.inputs / "install"
        self.installer.write_text("#!/usr/bin/env bash\nset -eu\n", encoding="utf-8")
        self.installer.chmod(0o500)
        self.appliance = self.inputs / "cpu-edge.tar.gz"
        self.generation = self.inputs / "generation.tar.gz"
        self.source = self.inputs / "source.tar.gz"
        self.toolchain = self.inputs / "toolchain.tar.gz"
        self.write_fixture_archives()

    @staticmethod
    def write_tar(path: Path, members: list[tuple[str, bytes, str]]) -> None:
        with path.open("wb") as raw:
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w") as archive:
                    for name, data, kind in members:
                        info = tarfile.TarInfo(name)
                        info.mtime = 0
                        info.uid = info.gid = 0
                        info.uname = info.gname = ""
                        if kind == "file":
                            info.size = len(data)
                            info.mode = 0o600
                            archive.addfile(info, io.BytesIO(data))
                        elif kind == "symlink":
                            info.type = tarfile.SYMTYPE
                            info.linkname = "/etc/shadow"
                            archive.addfile(info)
                        else:
                            raise AssertionError(kind)
        path.chmod(0o600)

    def write_fixture_archives(self) -> None:
        appliance_manifest = {
            "schema": "astrid_cpu_edge_build_manifest_v3",
            "bundle_format": "cpu-edge.3",
            "version": self.version,
            "target": self.target,
            "expected_loaded_capsule_count": 20,
        }
        generation_manifest = {
            "schema": "astrid.edge_self_change.initial_generation.v1",
            "version": self.version,
            "target": self.target,
            "authority": "operator_packaged_initial_generation_not_model_candidate",
        }
        source_manifest = {
            "schema": "astrid.edge.self_change_source_bundle.v1",
            "source_authority": "portable_bootstrap_non_authorizing",
            "appliance_id": None,
            "rustc": {"host": self.target},
            "files": [
                {
                    "path": f"source/capsules/astralis/{capsule}/Cargo.lock",
                    "sha256": "0" * 64,
                    "size": 1,
                    "mode": "0644",
                    "origin": "mutable_build_manifest",
                }
                for capsule in sorted(module.EDGE_CAPSULES)
            ],
        }
        toolchain_manifest = {
            "schema": "astrid.edge.self_change_toolchain_bundle.v1",
            "target": self.target,
        }
        self.write_tar(
            self.appliance,
            [("astrid-cpu-edge/BUILD-MANIFEST.json", json.dumps(appliance_manifest).encode(), "file")],
        )
        self.write_tar(
            self.generation,
            [("astrid-edge-generation/.astrid-edge-generation.json", json.dumps(generation_manifest).encode(), "file")],
        )
        self.write_tar(
            self.source,
            [("astrid-edge-self-change-source/MANIFEST.json", json.dumps(source_manifest).encode(), "file")],
        )
        self.write_tar(
            self.toolchain,
            [("astrid-edge-toolchain/MANIFEST.json", json.dumps(toolchain_manifest).encode(), "file")],
        )

    def args(self, output: Path | None = None):
        return module.parser().parse_args(
            [
                "--version", self.version,
                "--target", self.target,
                "--appliance-archive", str(self.appliance),
                "--generation-archive", str(self.generation),
                "--source-bundle", str(self.source),
                "--toolchain-bundle", str(self.toolchain),
                "--source-signing-key", str(self.key),
                "--supervisor", str(self.supervisor),
                "--installer", str(self.installer),
                "--output-dir", str(output or self.output),
            ]
        )

    def test_complete_bundle_is_deterministic_and_self_inventoried(self) -> None:
        first = module.build(self.args())
        other = self.root / "other"
        other.mkdir()
        second = module.build(self.args(other))
        first_bytes = Path(first["archive"]).read_bytes()
        self.assertEqual(first_bytes, Path(second["archive"]).read_bytes())
        self.assertEqual(first["sha256"], hashlib.sha256(first_bytes).hexdigest())
        sidecar = Path(f'{first["archive"]}.sha256')
        self.assertEqual(
            sidecar.read_text(encoding="ascii"),
            f'{first["sha256"]}  {Path(first["archive"]).name}\n',
        )

        with tarfile.open(first["archive"], "r:gz") as archive:
            names = {member.name for member in archive}
            root = f"astrid-edge-self-evolution-{self.version}-{self.target}"
            expected = {
                f"{root}/MANIFEST.json",
                f"{root}/README.txt",
                f"{root}/SHA256SUMS",
                f"{root}/install",
                f"{root}/payload/cpu-edge.tar.gz",
                f"{root}/payload/initial-generation.tar.gz",
                f"{root}/payload/portable-source.tar.gz",
                f"{root}/payload/pinned-toolchain.tar.gz",
                f"{root}/payload/portable-source.key",
                f"{root}/payload/edge-self-change-supervisor.pyz",
            }
            self.assertTrue(expected.issubset(names))
            manifest = json.load(archive.extractfile(f"{root}/MANIFEST.json"))
            self.assertEqual(manifest["schema"], module.SCHEMA)
            self.assertEqual(manifest["initial_mode"], "paused_bootstrap_acceptance_pending")
            self.assertEqual(
                manifest["portable_trust"],
                "integrity_only_rebound_to_fresh_per_appliance_key_before_authorization",
            )
            self.assertEqual(manifest["profiles"]["icp"]["output_tokens"], 112)
            self.assertEqual(
                manifest["profiles"]["icp"]["retained_backup"],
                "/media/data/astrid/backups/emmc-20260729T130835Z",
            )
            readme = archive.extractfile(f"{root}/README.txt").read().decode("utf-8")
            self.assertIn("do not authenticate the publisher", readme)
            self.assertIn("GitHub OIDC/Sigstore", readme)
            self.assertIn("--root-install", readme)
            self.assertIn("Direct sudo execution", readme)

    def test_wrong_target_and_incomplete_capsule_closure_fail(self) -> None:
        with tarfile.open(self.source, "r:gz") as archive:
            member = archive.getmember("astrid-edge-self-change-source/MANIFEST.json")
            manifest = json.load(archive.extractfile(member))
        manifest["files"].pop()
        self.write_tar(
            self.source,
            [("astrid-edge-self-change-source/MANIFEST.json", json.dumps(manifest).encode(), "file")],
        )
        with self.assertRaisesRegex(module.PackageError, "portable source"):
            module.build(self.args())

    def test_manifest_validation_uses_captured_bytes_not_replaced_path(self) -> None:
        valid_appliance = self.appliance.read_bytes()
        invalid_manifest = {
            "schema": "astrid_cpu_edge_build_manifest_v3",
            "bundle_format": "cpu-edge.3",
            "version": self.version,
            "target": self.target,
            "expected_loaded_capsule_count": 19,
        }
        self.write_tar(
            self.appliance,
            [
                (
                    "astrid-cpu-edge/BUILD-MANIFEST.json",
                    json.dumps(invalid_manifest).encode(),
                    "file",
                )
            ],
        )
        real_stable_regular = module.stable_regular
        replaced = False

        def capture_then_replace(path: Path, **kwargs) -> bytes:
            nonlocal replaced
            captured = real_stable_regular(path, **kwargs)
            if path == self.appliance and not replaced:
                self.appliance.write_bytes(valid_appliance)
                self.appliance.chmod(0o600)
                replaced = True
            return captured

        with mock.patch.object(module, "stable_regular", side_effect=capture_then_replace):
            with self.assertRaisesRegex(module.PackageError, "CPU-edge archive"):
                module.build(self.args())
        self.assertTrue(replaced)

    def test_arm_bootstrap_is_not_advertised_without_a_named_arm_appliance(self) -> None:
        arguments = [
            "--version", self.version,
            "--target", "aarch64-unknown-linux-gnu",
            "--appliance-archive", str(self.appliance),
            "--generation-archive", str(self.generation),
            "--source-bundle", str(self.source),
            "--toolchain-bundle", str(self.toolchain),
            "--source-signing-key", str(self.key),
            "--supervisor", str(self.supervisor),
            "--installer", str(self.installer),
            "--output-dir", str(self.output),
        ]
        parsed = module.parser().parse_args(arguments)
        with self.assertRaisesRegex(module.PackageError, "version or target"):
            module.build(parsed)

    def test_nested_link_duplicate_and_overwrite_fail_closed(self) -> None:
        self.write_tar(
            self.toolchain,
            [
                (
                    "astrid-edge-toolchain/MANIFEST.json",
                    json.dumps(
                        {
                            "schema": "astrid.edge.self_change_toolchain_bundle.v1",
                            "target": self.target,
                        }
                    ).encode(),
                    "file",
                ),
                ("astrid-edge-toolchain/escape", b"", "symlink"),
            ],
        )
        with self.assertRaisesRegex(module.PackageError, "link or special"):
            module.build(self.args())
        self.write_fixture_archives()
        module.build(self.args())
        with self.assertRaisesRegex(module.PackageError, "overwrite"):
            module.build(self.args())

    def test_key_permissions_and_input_links_are_rejected(self) -> None:
        self.key.chmod(0o644)
        with self.assertRaisesRegex(module.PackageError, "owner-only"):
            module.build(self.args())
        self.key.chmod(0o600)
        linked = self.inputs / "linked-source.tar.gz"
        os.link(self.source, linked)
        args = self.args()
        args.source_bundle = linked
        with self.assertRaisesRegex(module.PackageError, "single-linked"):
            module.build(args)

    def test_sidecar_creation_is_exclusive_nofollow_and_durable(self) -> None:
        sidecar = self.root / "artifact.tar.gz.sha256"
        fsync_types: list[str] = []
        real_fsync = os.fsync

        def recording_fsync(descriptor: int) -> None:
            mode = os.fstat(descriptor).st_mode
            fsync_types.append("directory" if stat.S_ISDIR(mode) else "file")
            real_fsync(descriptor)

        with mock.patch.object(module.os, "fsync", side_effect=recording_fsync):
            module.write_exclusive_durable(sidecar, b"digest  artifact.tar.gz\n", 0o600)
        self.assertEqual(sidecar.read_bytes(), b"digest  artifact.tar.gz\n")
        self.assertEqual(sidecar.stat().st_mode & 0o777, 0o600)
        self.assertIn("file", fsync_types)
        self.assertIn("directory", fsync_types)
        with self.assertRaises(FileExistsError):
            module.write_exclusive_durable(sidecar, b"replacement\n", 0o600)

        victim = self.root / "victim"
        victim.write_bytes(b"preserved\n")
        linked_sidecar = self.root / "linked.sha256"
        linked_sidecar.symlink_to(victim)
        with self.assertRaises(FileExistsError):
            module.write_exclusive_durable(linked_sidecar, b"tamper\n", 0o600)
        self.assertEqual(victim.read_bytes(), b"preserved\n")


if __name__ == "__main__":
    unittest.main()
