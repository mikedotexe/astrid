#!/usr/bin/env python3
"""Adversarial tests for the offline CPU-edge self-change source bundle."""

from __future__ import annotations

import argparse
import gzip
import importlib.util
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("build_edge_self_change_source_bundle.py")
SPEC = importlib.util.spec_from_file_location("edge_source_bundle", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
bundle = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = bundle
SPEC.loader.exec_module(bundle)
sys.path.insert(0, str(SCRIPT.parent))
from edge_self_change import model as supervisor_model  # noqa: E402


def write(path: Path, content: str | bytes, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(content, bytes):
        path.write_bytes(content)
    else:
        path.write_text(content, encoding="utf-8")
    path.chmod(mode)


def git(repo: Path, *arguments: str) -> str:
    completed = subprocess.run(
        ["git", "-C", str(repo), *arguments],
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


class SourceBundleTests(unittest.TestCase):
    def test_signed_source_and_supervisor_share_exact_twenty_capsule_surface(self) -> None:
        self.assertEqual(set(bundle.EDGE_CAPSULES), set(supervisor_model.EDGE_CAPSULES))
        self.assertEqual(len(bundle.EDGE_CAPSULES), 20)

    def test_operator_builder_rejects_python_3_10_before_tomllib_import(self) -> None:
        with self.assertRaisesRegex(SystemExit, "operator-side builder.*Python 3.11"):
            bundle.require_supported_python((3, 10, 12))
        bundle.require_supported_python((3, 11, 0))

    def test_repository_tracks_the_root_and_every_local_cpu_edge_lock(self) -> None:
        repository = SCRIPT.parent.parent
        tracked = set(
            subprocess.run(
                ["git", "ls-files", "--", "*Cargo.lock"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            ).stdout.splitlines()
        )
        required = {"Cargo.lock"}
        required.update(
            f"services/{service}/Cargo.lock" for service in bundle.EDGE_STANDALONE_SERVICES
        )
        required.update(
            f"capsules/astralis/{capsule}/Cargo.lock"
            for capsule in bundle.LOCAL_EDGE_CAPSULES
        )
        self.assertEqual(required - tracked, set())

    def test_repository_quickjs_kernel_is_reviewable_and_not_ignored(self) -> None:
        repository = SCRIPT.parent.parent
        kernel = repository / bundle.QUICKJS_KERNEL_PATH
        self.assertEqual(kernel.stat().st_size, 1_568_372)
        self.assertEqual(
            bundle.sha256_bytes(kernel.read_bytes()),
            "318c3b10c3f7dea63ba532bbe055a62b6c0d965688769d4f7bc4ca5fbfc8313f",
        )
        ignored = subprocess.run(
            ["git", "check-ignore", "--quiet", "--", bundle.QUICKJS_KERNEL_PATH],
            cwd=repository,
            check=False,
        )
        self.assertNotEqual(ignored.returncode, 0)
        self.assertEqual(
            (repository / bundle.QUICKJS_KERNEL_HASH_PATH).read_text(encoding="ascii"),
            "8c1685a206c32633d364701e6bd90b6658f1d92959f8136c82ad9a309c114862"
            "  engine.wasm\n",
        )

    def test_source_and_installer_share_four_gib_uncompressed_bound(self) -> None:
        self.assertEqual(bundle.MAX_UNCOMPRESSED_BYTES, 4 * 1024 * 1024 * 1024)
        installer = SCRIPT.with_name("install_edge_self_evolution_root.sh").read_text(
            encoding="utf-8"
        )
        match = re.search(
            r"^MAX_UNCOMPRESSED_BYTES = (\d+) \* 1024 \* 1024 \* 1024$",
            installer,
            flags=re.MULTILINE,
        )
        self.assertIsNotNone(match)
        assert match is not None
        installer_bound = int(match.group(1)) * 1024 * 1024 * 1024
        self.assertEqual(installer_bound, bundle.MAX_UNCOMPRESSED_BYTES)
        self.assertFalse(2 * 1024 * 1024 * 1024 + 1 > installer_bound)
        self.assertFalse(installer_bound > installer_bound)
        self.assertTrue(installer_bound + 1 > installer_bound)

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.package_checksum = bundle.sha256_bytes(b"serde-package")
        git(self.repo, "init", "--quiet")
        git(self.repo, "config", "user.email", "bundle-tests@example.invalid")
        git(self.repo, "config", "user.name", "Bundle Tests")
        self._write_repository_fixture()
        git(self.repo, "add", ".")
        git(self.repo, "commit", "--quiet", "-m", "fixture")

        self.lock = self.root / "Cargo.lock"
        self._write_lock(self.package_checksum)
        self.vendor = self.root / "vendor"
        self._write_vendor(self.package_checksum)
        self.rustc = self.root / "rustc-version.txt"
        write(self.rustc, self._rustc_metadata())
        self.key = self.root / "owner.hmac"
        write(self.key, b"K" * 32, 0o600)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_release_profiles_strip_and_lto_every_incremental_binary(self) -> None:
        repository = SCRIPT.parent.parent
        manifests = (
            "services/astrid-edge-runtime/Cargo.toml",
            "services/astrid-edge-steward-helper/Cargo.toml",
            "services/astrid-edge-rescue-helper/Cargo.toml",
            "services/astrid-edge-web-broker/Cargo.toml",
            "services/astrid-edge-provider-broker/Cargo.toml",
            "services/astrid-edge-presentation-broker/Cargo.toml",
            "services/astrid-edge-checkpoint/Cargo.toml",
            "capsules/astralis/astrid-capsule-edge-context/Cargo.toml",
            "capsules/astralis/astrid-capsule-edge-introspector/Cargo.toml",
            "capsules/astralis/astrid-capsule-edge-spectral/Cargo.toml",
        )
        expected = {
            "codegen-units": 1,
            "lto": "thin",
            "panic": "abort",
            "strip": "symbols",
        }
        for relative in manifests:
            manifest = tomllib.loads(
                (repository / relative).read_text(encoding="utf-8")
            )
            self.assertEqual(manifest.get("profile", {}).get("release"), expected, relative)

    def _write_repository_fixture(self) -> None:
        write(
            self.repo / "Cargo.toml",
            """[workspace]
resolver = "2"
members = ["crates/astrid-daemon"]

[workspace.package]
rust-version = "1.94"
""",
        )
        write(self.repo / ".cargo/config.toml", "[net]\noffline = true\n")
        write(
            self.repo / ".gitignore",
            "**/*.wasm\n!crates/astrid-openclaw/kernel/engine.wasm\n",
        )
        write(self.repo / bundle.QUICKJS_KERNEL_LICENSE_PATH, "js-pdk fixture license\n")
        write(self.repo / "clippy.toml", "msrv = \"1.94\"\n")
        write(self.repo / "rustfmt.toml", "edition = \"2024\"\n")
        write(
            self.repo / "crates/astrid-daemon/Cargo.toml",
            """[package]
name = "astrid-daemon"
version = "0.1.0"
edition = "2024"
""",
        )
        write(self.repo / "crates/astrid-daemon/src/main.rs", "fn main() {}\n")
        for crate in (
            "astrid-build",
            "astrid-openclaw",
            "astrid-prelude",
            "astrid-minime-protocol",
            "astrid-integration-tests",
            "astrid-test",
        ):
            write(
                self.repo / f"crates/{crate}/Cargo.toml",
                f'[package]\nname = "{crate}"\nversion = "0.1.0"\nedition = "2024"\n',
            )
            write(self.repo / f"crates/{crate}/src/lib.rs", "pub fn cpu_edge() {}\n")
        write(
            self.repo / "crates/astrid-openclaw/kernel/engine.wasm",
            b"\0asm\x01\0\0\0fixture-kernel",
        )
        write(
            self.repo / "crates/astrid-openclaw/kernel/engine.wasm.blake3",
            f"{'a' * 64}  engine.wasm\n",
        )
        write(
            self.repo / "services/astrid-edge-runtime/Cargo.toml",
            """[package]
name = "astrid-edge-runtime"
version = "0.1.0"
edition = "2024"
rust-version = "1.94"
""",
        )
        write(self.repo / "services/astrid-edge-runtime/src/main.rs", "fn main() {}\n")
        write(self.repo / "services/astrid-edge-runtime/src/actions.rs", "pub fn act() {}\n")
        for mutable_authority_path in (
            "crates/astrid-capsule/src/cpu_edge_policy.rs",
            "crates/astrid-capsule/src/engine/wasm/host/process.rs",
            "crates/astrid-capsule/src/loader.rs",
            "crates/astrid-events/src/bus.rs",
            "crates/astrid-kernel/src/maintenance.rs",
            "crates/astrid-kernel/src/socket_bridge.rs",
            "services/astrid-edge-runtime/src/config.rs",
            "services/astrid-edge-runtime/src/ipc.rs",
            "services/astrid-edge-runtime/src/maintenance.rs",
        ):
            write(self.repo / mutable_authority_path, "pub fn runtime_authority_path() {}\n")
        lock = f'''version = 4

[[package]]
name = "serde"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "{self.package_checksum}"
'''
        write(self.repo / "Cargo.lock", lock)
        write(self.repo / "services/astrid-edge-runtime/Cargo.lock", lock)
        # Runtime integration is mutable. Rescue components are present only as inspect-only
        # signed source and remain categorically ineligible for candidate edits.
        write(
            self.repo / "services/astrid-edge-runtime/src/self_change.rs",
            "pub fn mutable_runtime_integration() {}\n",
        )
        write(
            self.repo / "capsules/spectral-bridge/src/lib.rs",
            "compile_error!(\"mac bridge must stay out\");\n",
        )
        write(
            self.repo / "services/astrid-edge-steward-helper/src/main.rs",
            "compile_error!(\"immutable steward must stay out\");\n",
        )
        write(
            self.repo / "services/astrid-edge-rescue-helper/src/main.rs",
            "compile_error!(\"immutable rescue helper must stay out\");\n",
        )
        write(
            self.repo / "services/astrid-edge-web-broker/src/main.rs",
            "compile_error!(\"immutable broker must stay out\");\n",
        )
        write(
            self.repo / "services/astrid-edge-provider-broker/src/main.rs",
            "compile_error!(\"immutable provider broker must stay out\");\n",
        )
        write(
            self.repo / "services/astrid-edge-presentation-broker/src/main.rs",
            "compile_error!(\"immutable presentation broker must stay out\");\n",
        )
        write(
            self.repo / "services/astrid-edge-checkpoint/src/main.rs",
            "compile_error!(\"immutable checkpoint must stay out\");\n",
        )
        for service in bundle.EDGE_STANDALONE_SERVICES:
            write(
                self.repo / f"services/{service}/Cargo.toml",
                f'[package]\nname = "{service}"\nversion = "0.1.0"\n'
                'rust-version = "1.94"\n',
            )
            write(self.repo / f"services/{service}/Cargo.lock", lock)
        write(
            self.repo / "scripts/edge_self_change_supervisor.py",
            "raise SystemExit('immutable rescue')\n",
            0o755,
        )
        write(
            self.repo / "scripts/build_edge_self_change_source_bundle.py",
            "raise SystemExit('operator-only source bundler')\n",
            0o755,
        )
        write(
            self.repo / "scripts/test_build_edge_self_change_source_bundle.py",
            "raise SystemExit('operator-only source bundler tests')\n",
            0o644,
        )
        for capsule in bundle.EDGE_CAPSULES:
            root = self.repo / f"capsules/astralis/{capsule}"
            write(
                root / "Cargo.toml",
                f"[package]\nname = \"{capsule}\"\nversion = \"0.1.0\"\n"
                'rust-version = "1.94"\n',
            )
            write(root / "Capsule.toml", f"[package]\nname = \"{capsule}\"\n")
            write(root / "src/lib.rs", "pub fn capsule() {}\n")
            write(root / "Cargo.lock", lock)
        write(self.repo / "scripts/report_edge_appliance.py", "print('edge report')\n", 0o755)
        write(
            self.repo / "scripts/warm_ollama_model.sh",
            "#!/bin/sh\nexit 0\n",
            0o755,
        )
        write(
            self.repo / "packaging/appliances/avado-i3-16g.env",
            "ASTRID_EDGE_TEST=true\n",
        )
        write(
            self.repo / "packaging/systemd/astrid-edge-runtime.service",
            "[Service]\nExecStart=/usr/bin/astrid-edge-runtime\n",
        )
        write(
            self.repo / "packaging/systemd/astrid-edge-self-change-supervisor.service",
            "[Service]\nExecStart=/immutable/rescue\n",
        )
        for name in (
            "astrid-edge-web-broker-runtime.socket",
            "astrid-edge-web-broker-runtime.service",
            "astrid-edge-web-broker-steward.socket",
            "astrid-edge-web-broker-steward.service",
            "astrid-edge-presentation-broker.socket",
            "astrid-edge-presentation-broker@.service",
        ):
            write(
                self.repo / f"packaging/systemd/{name}",
                "[Service]\nExecStart=/immutable/web-broker\n",
            )

    def _write_lock(self, checksum: str, *, version: str = "1.0.0") -> None:
        data = f"""version = 4

[[package]]
name = "serde"
version = "{version}"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "{checksum}"
"""
        write(self.lock, data)
        write(self.repo / "Cargo.lock", data)
        git(self.repo, "add", "Cargo.lock")
        if subprocess.run(
            ["git", "diff", "--cached", "--quiet"], cwd=self.repo, check=False
        ).returncode != 0:
            git(self.repo, "commit", "--quiet", "-m", "update root lock fixture")

    def _write_vendor(self, package_checksum: str, *, unexpected: bool = False) -> None:
        package = self.vendor / "serde-1.0.0"
        manifest = b'[package]\nname = "serde"\nversion = "1.0.0"\n'
        source = b"pub fn serialize() {}\n"
        write(package / "Cargo.toml", manifest)
        write(package / "src/lib.rs", source)
        checksums = {
            "Cargo.toml": bundle.sha256_bytes(manifest),
            "src/lib.rs": bundle.sha256_bytes(source),
        }
        if unexpected:
            write(package / "surprise.rs", "pub fn surprise() {}\n")
        write(
            package / ".cargo-checksum.json",
            json.dumps({"files": checksums, "package": package_checksum}, sort_keys=True),
        )

    @staticmethod
    def _rustc_metadata(release: str = bundle.REQUIRED_RUST_RELEASE) -> str:
        return f"""rustc {release} ({bundle.REQUIRED_RUST_COMMIT[:9]} {bundle.REQUIRED_RUST_COMMIT_DATE})
binary: rustc
commit-hash: {bundle.REQUIRED_RUST_COMMIT}
commit-date: {bundle.REQUIRED_RUST_COMMIT_DATE}
host: x86_64-unknown-linux-gnu
release: {release}
LLVM version: {bundle.REQUIRED_LLVM_VERSION}
"""

    def _build_args(
        self,
        output: Path,
        *,
        signed: bool = True,
        vendor: Path | None = None,
    ) -> argparse.Namespace:
        return argparse.Namespace(
            repo=self.repo,
            vendor_dir=vendor or self.vendor,
            cargo_lock=self.lock,
            rustc_metadata=self.rustc,
            external_capsule_source_dir=[],
            output=output,
            signing_key=self.key if signed else None,
            test_only_unsigned=not signed,
        )

    def _verify_args(
        self,
        archive: Path,
        *,
        key: Path | None = None,
        unsigned: bool = False,
    ) -> argparse.Namespace:
        return argparse.Namespace(
            bundle=archive,
            signing_key=key,
            allow_test_only_unsigned=unsigned,
        )

    @staticmethod
    def _manifest(archive: Path) -> dict[str, object]:
        with tarfile.open(archive, "r:gz") as handle:
            member = handle.getmember(f"{bundle.BUNDLE_ROOT}/MANIFEST.json")
            extracted = handle.extractfile(member)
            assert extracted is not None
            return json.loads(extracted.read())

    def test_signed_bundle_is_deterministic_and_verifiable(self) -> None:
        first = self.root / "first.tar.gz"
        second = self.root / "second.tar.gz"
        result = bundle.build_bundle(self._build_args(first))
        bundle.build_bundle(self._build_args(second))
        self.assertEqual(first.read_bytes(), second.read_bytes())
        verified = bundle.verify_bundle(self._verify_args(first, key=self.key))
        self.assertEqual(result["source_id"], verified["source_id"])
        self.assertRegex(result["source_id"], r"^cpu-edge-portable:[0-9a-f]{64}$")

        manifest = self._manifest(first)
        self.assertIsNone(manifest["appliance_id"])
        self.assertEqual(
            manifest["source_authority"],
            "portable_bootstrap_non_authorizing",
        )
        paths = {record["path"] for record in manifest["files"]}  # type: ignore[index]
        records = {record["path"]: record for record in manifest["files"]}  # type: ignore[index]
        self.assertIn("source/crates/astrid-daemon/src/main.rs", paths)
        for crate in (
            "astrid-build",
            "astrid-openclaw",
            "astrid-prelude",
            "astrid-minime-protocol",
            "astrid-integration-tests",
            "astrid-test",
        ):
            path = f"source/crates/{crate}/src/lib.rs"
            self.assertIn(path, paths)
            self.assertEqual(records[path]["origin"], "mutable_core_source")
        self.assertIn("source/services/astrid-edge-runtime/src/actions.rs", paths)
        for mutable_authority_path in (
            "crates/astrid-events/src/bus.rs",
            "crates/astrid-kernel/src/maintenance.rs",
            "crates/astrid-kernel/src/socket_bridge.rs",
        ):
            packaged = f"source/{mutable_authority_path}"
            self.assertIn(packaged, paths)
            self.assertEqual(records[packaged]["origin"], "mutable_core_source")
        for mutable_authority_path in (
            "services/astrid-edge-runtime/src/config.rs",
            "services/astrid-edge-runtime/src/ipc.rs",
            "services/astrid-edge-runtime/src/maintenance.rs",
        ):
            packaged = f"source/{mutable_authority_path}"
            self.assertIn(packaged, paths)
            self.assertEqual(records[packaged]["origin"], "mutable_edge_runtime")
        self.assertIn(
            "source/capsules/astralis/astrid-capsule-edge-context/Capsule.toml",
            paths,
        )
        self.assertIn(
            "source/capsules/astralis/astrid-capsule-shell/src/lib.rs",
            paths,
        )
        self.assertIn("source/Cargo.lock", paths)
        self.assertIn("source/services/astrid-edge-runtime/Cargo.lock", paths)
        self.assertIn(f"source/{bundle.QUICKJS_KERNEL_PATH}", paths)
        self.assertEqual(
            records[f"source/{bundle.QUICKJS_KERNEL_PATH}"]["origin"],
            "build_required_immutable",
        )
        self.assertEqual(records["source/Cargo.lock"]["origin"], "mutable_build_manifest")
        self.assertEqual(
            records["source/services/astrid-edge-runtime/Cargo.lock"]["origin"],
            "mutable_build_manifest",
        )
        self.assertIn("source/scripts/warm_ollama_model.sh", paths)
        self.assertEqual(
            records["source/scripts/warm_ollama_model.sh"]["origin"],
            "build_required_runtime_script",
        )
        self.assertEqual(
            records["source/packaging/systemd/astrid-edge-runtime.service"]["origin"],
            "mutable_astrid_service_template",
        )
        for path in (
            "source/LICENSE-js-pdk",
            "source/crates/astrid-openclaw/kernel/engine.wasm",
            "source/crates/astrid-openclaw/kernel/engine.wasm.blake3",
        ):
            self.assertEqual(records[path]["origin"], "build_required_immutable")
        self.assertIn(
            "source/capsules/astralis/astrid-capsule-edge-introspector/Cargo.lock",
            paths,
        )
        for capsule in bundle.EDGE_CAPSULES:
            lock_path = f"source/capsules/astralis/{capsule}/Cargo.lock"
            self.assertIn(lock_path, paths)
            self.assertEqual(records[lock_path]["origin"], "mutable_build_manifest")
        for service in bundle.EDGE_STANDALONE_SERVICES:
            lock_path = f"source/services/{service}/Cargo.lock"
            self.assertIn(lock_path, paths)
            expected_origin = (
                "mutable_build_manifest"
                if service == "astrid-edge-runtime"
                else "inspect_only_immutable_boundary"
            )
            self.assertEqual(records[lock_path]["origin"], expected_origin)
        self.assertFalse(any("spectral-bridge" in path for path in paths))
        self.assertEqual(
            records["source/services/astrid-edge-runtime/src/self_change.rs"]["origin"],
            "mutable_edge_runtime",
        )
        for path in (
            "source/crates/astrid-capsule/src/cpu_edge_policy.rs",
            "source/crates/astrid-capsule/src/engine/wasm/host/process.rs",
            "source/crates/astrid-capsule/src/loader.rs",
        ):
            self.assertEqual(records[path]["origin"], "mutable_core_source")
        for path in (
            "source/services/astrid-edge-steward-helper/src/main.rs",
            "source/services/astrid-edge-rescue-helper/src/main.rs",
            "source/services/astrid-edge-web-broker/src/main.rs",
            "source/services/astrid-edge-provider-broker/src/main.rs",
            "source/services/astrid-edge-presentation-broker/src/main.rs",
            "source/services/astrid-edge-checkpoint/src/main.rs",
            "source/scripts/build_edge_self_change_source_bundle.py",
            "source/scripts/test_build_edge_self_change_source_bundle.py",
            "source/scripts/edge_self_change_supervisor.py",
            "source/packaging/systemd/astrid-edge-self-change-supervisor.service",
        ):
            self.assertEqual(records[path]["origin"], "inspect_only_immutable_boundary")

    def test_appliance_bound_source_identity_rejects_cross_appliance_reuse(self) -> None:
        archive = self.root / "portable.tar.gz"
        bundle.build_bundle(self._build_args(archive))
        manifest = self._manifest(archive)
        identity = bundle.appliance_bound_source_identity(
            manifest["repository_commit"],
            manifest["rustc"],
            manifest["files"],
            "avado",
        )
        identity_hash = bundle.sha256_bytes(bundle.canonical_bytes(identity))
        manifest.update(
            {
                "appliance_id": "avado",
                "source_authority": bundle.LOCAL_SOURCE_AUTHORITY,
                "source_identity_sha256": identity_hash,
                "source_id": f"cpu-edge:{identity_hash}",
            }
        )
        bundle.validate_appliance_bound_manifest(manifest, "avado")
        with self.assertRaisesRegex(bundle.BundleError, "expected appliance"):
            bundle.validate_appliance_bound_manifest(manifest, "icp")

    def test_only_exact_astrid_base_fragments_receive_mutable_unit_role(self) -> None:
        for path in (
            "packaging/systemd/ollama-cpu.service",
            "packaging/systemd/astrid-model-warmup.service",
            "packaging/systemd/astrid.service",
            "packaging/systemd/astrid-edge-runtime.service",
            "packaging/systemd/astrid-edge-hindsight.service",
            "packaging/systemd/astrid-edge-hindsight.timer",
            "packaging/systemd/icp/ollama-cpu.service",
            "packaging/systemd/icp/astrid-model-warmup.service",
            "packaging/systemd/icp/astrid.service",
            "packaging/systemd/icp/astrid-edge-runtime.service",
            "packaging/systemd/icp/astrid-edge-hindsight.service",
            "packaging/systemd/icp/astrid-edge-hindsight.timer",
        ):
            self.assertEqual(
                bundle.source_role(path), "mutable_astrid_service_template"
            )

        for path in (
            "packaging/systemd/astrid-edge-steward.service",
            "packaging/systemd/ssh.service",
            "packaging/systemd/root/astrid-edge-runtime-self-evolution.conf.in",
            "packaging/systemd/astrid.service.d/override.conf",
            "packaging/systemd/icp/nested/astrid.service",
        ):
            self.assertNotEqual(
                bundle.source_role(path), "mutable_astrid_service_template"
            )

    def test_unsigned_mode_requires_explicit_build_and_verify_flags(self) -> None:
        archive = self.root / "unsigned.tar.gz"
        result = bundle.build_bundle(self._build_args(archive, signed=False))
        self.assertEqual(result["signature_mode"], "test_only_unsigned")
        with self.assertRaisesRegex(bundle.BundleError, "explicitly allowed"):
            bundle.verify_bundle(self._verify_args(archive, key=self.key))
        verified = bundle.verify_bundle(self._verify_args(archive, unsigned=True))
        self.assertEqual(verified["signature_mode"], "test_only_unsigned")

    def test_dirty_tracked_or_untracked_source_is_rejected(self) -> None:
        write(self.repo / "services/astrid-edge-runtime/src/main.rs", "fn changed() {}\n")
        with self.assertRaisesRegex(bundle.BundleError, "repository is dirty"):
            bundle.build_bundle(self._build_args(self.root / "dirty.tar.gz"))

        git(self.repo, "reset", "--hard", "HEAD")
        write(self.repo / "untracked.txt", "not allowed\n")
        with self.assertRaisesRegex(bundle.BundleError, "repository is dirty"):
            bundle.build_bundle(self._build_args(self.root / "untracked.tar.gz"))

    def test_selected_source_symlink_is_rejected(self) -> None:
        link = self.repo / "services/astrid-edge-runtime/src/linked.rs"
        os.symlink("main.rs", link)
        git(self.repo, "add", str(link.relative_to(self.repo)))
        git(self.repo, "commit", "--quiet", "-m", "tracked symlink")
        with self.assertRaisesRegex(bundle.BundleError, "symlink, submodule, or special mode"):
            bundle.build_bundle(self._build_args(self.root / "symlink.tar.gz"))

    def test_vendor_hardlink_symlink_fifo_and_unexpected_file_are_rejected(self) -> None:
        original = self.vendor / "serde-1.0.0/src/lib.rs"
        hardlink = self.root / "hardlink-copy"
        os.link(original, hardlink)
        with self.assertRaisesRegex(bundle.BundleError, "non-hardlinked"):
            bundle.build_bundle(self._build_args(self.root / "hardlink.tar.gz"))
        hardlink.unlink()

        link = self.vendor / "serde-1.0.0/src/link.rs"
        os.symlink("lib.rs", link)
        with self.assertRaisesRegex(bundle.BundleError, "symlink"):
            bundle.build_bundle(self._build_args(self.root / "vendor-link.tar.gz"))
        link.unlink()

        fifo = self.vendor / "serde-1.0.0/fifo"
        os.mkfifo(fifo)
        with self.assertRaisesRegex(bundle.BundleError, "device or special file"):
            bundle.build_bundle(self._build_args(self.root / "fifo.tar.gz"))
        fifo.unlink()

        write(self.vendor / "serde-1.0.0/unexpected.rs", "pub fn nope() {}\n")
        with self.assertRaisesRegex(bundle.BundleError, "missing or unexpected files"):
            bundle.build_bundle(self._build_args(self.root / "unexpected.tar.gz"))

    def test_checksum_bound_vendor_github_metadata_is_accepted(self) -> None:
        files = {
            ".github/FUNDING.yml": b"github: astrid\n",
            "src/target/mod.rs": b"pub fn legitimate_target_module() {}\n",
        }
        for relative, data in files.items():
            write(self.vendor / "serde-1.0.0" / relative, data)
        checksum_path = self.vendor / "serde-1.0.0/.cargo-checksum.json"
        checksum = json.loads(checksum_path.read_text(encoding="utf-8"))
        for relative, data in files.items():
            checksum["files"][relative] = bundle.sha256_bytes(data)
        write(checksum_path, json.dumps(checksum, sort_keys=True))
        result = bundle.build_bundle(self._build_args(self.root / "github-metadata.tar.gz"))
        self.assertGreater(result["file_count"], 0)

    def test_lock_vendor_mismatch_and_bad_checksum_are_rejected(self) -> None:
        self._write_lock(self.package_checksum, version="2.0.0")
        with self.assertRaisesRegex(bundle.BundleError, "vendor.*locked package"):
            bundle.build_bundle(self._build_args(self.root / "version.tar.gz"))
        self._write_lock("f" * 64)
        with self.assertRaisesRegex(bundle.BundleError, "checksum.*Cargo.lock"):
            bundle.build_bundle(self._build_args(self.root / "checksum.tar.gz"))

    def test_every_edge_capsule_lock_is_required(self) -> None:
        missing = self.repo / "capsules/astralis/astrid-capsule-agents/Cargo.lock"
        git(self.repo, "rm", "--quiet", str(missing.relative_to(self.repo)))
        git(self.repo, "commit", "--quiet", "-m", "remove required capsule lock")
        with self.assertRaisesRegex(bundle.BundleError, "required tracked CPU-edge source.*agents"):
            bundle.build_bundle(self._build_args(self.root / "missing-capsule-lock.tar.gz"))

    def test_every_edge_capsule_manifest_is_required_and_identity_bound(self) -> None:
        manifest = self.repo / "capsules/astralis/astrid-capsule-agents/Cargo.toml"
        git(self.repo, "rm", "--quiet", str(manifest.relative_to(self.repo)))
        git(self.repo, "commit", "--quiet", "-m", "remove required capsule manifest")
        with self.assertRaisesRegex(bundle.BundleError, "required tracked CPU-edge source.*agents"):
            bundle.build_bundle(self._build_args(self.root / "missing-capsule-manifest.tar.gz"))

        write(
            manifest,
            '[package]\nname = "astrid-capsule-forged"\nversion = "0.1.0"\n',
        )
        git(self.repo, "add", str(manifest.relative_to(self.repo)))
        git(self.repo, "commit", "--quiet", "-m", "restore mismatched capsule manifest")
        with self.assertRaisesRegex(bundle.BundleError, "package identity.*agents"):
            bundle.build_bundle(self._build_args(self.root / "wrong-capsule-manifest.tar.gz"))

    def test_root_workspace_lock_is_tracked_and_matches_the_supplied_lock(self) -> None:
        write(self.lock, self.lock.read_text(encoding="utf-8").replace("1.0.0", "2.0.0"))
        with self.assertRaisesRegex(bundle.BundleError, "exact tracked root Cargo.lock"):
            bundle.build_bundle(self._build_args(self.root / "foreign-root-lock.tar.gz"))

        write(self.lock, (self.repo / "Cargo.lock").read_bytes())
        git(self.repo, "rm", "--quiet", "Cargo.lock")
        git(self.repo, "commit", "--quiet", "-m", "remove tracked root lock")
        with self.assertRaisesRegex(bundle.BundleError, "required tracked CPU-edge source.*Cargo.lock"):
            bundle.build_bundle(self._build_args(self.root / "missing-root-lock.tar.gz"))

    def test_external_capsule_source_manifest_supplies_all_mutable_capsules(self) -> None:
        external = self.root / "external-source"
        capsules: list[dict[str, object]] = []
        for package in bundle.EXTERNAL_EDGE_CAPSULES:
            short_id = package.removeprefix("astrid-capsule-")
            records: list[dict[str, object]] = []
            for relative, content in {
                "Cargo.toml": f"[package]\nname = \"{package}\"\nversion = \"0.1.0\"\n",
                "Cargo.lock": "version = 4\n",
                "Capsule.toml": f"[package]\nname = \"{package}\"\n",
                "src/lib.rs": "pub fn capsule() {}\n",
            }.items():
                data = content.encode()
                write(external / short_id / relative, data)
                records.append(
                    {
                        "path": f"{short_id}/{relative}",
                        "mode": "0644",
                        "size": len(data),
                        "sha256": bundle.sha256_bytes(data),
                    }
                )
            capsules.append(
                {
                    "id": short_id,
                    "package": package,
                    "revision": "a" * 40,
                    "files": records,
                }
            )
        manifest = {
            "schema": bundle.EXTERNAL_SOURCE_SCHEMA,
            "recipe": "fixture.toml",
            "rust_toolchain": bundle.REQUIRED_RUST_RELEASE,
            "target": "wasm32-wasip2",
            "sdk_version": "0.7.1",
            "source_policy": "exact_upstream_snapshot",
            "capsules": capsules,
        }
        write(
            external / "SOURCE-MANIFEST.json",
            json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        )
        existing = [
            bundle.Payload(
                path=f"source/capsules/astralis/{capsule}/Cargo.lock",
                origin="mutable_build_manifest",
                mode=0o644,
                sha256="0" * 64,
                size=0,
                data=b"",
            )
            for capsule in bundle.LOCAL_EDGE_CAPSULES
        ]
        imported = bundle.external_capsule_source_payloads([external], existing)
        self.assertEqual(
            {
                payload.path.split("/")[3]
                for payload in imported
                if payload.path.endswith("/Cargo.lock")
            },
            set(bundle.EXTERNAL_EDGE_CAPSULES),
        )
        write(external / "session/src/undeclared.rs", "pub fn forged() {}\n")
        with self.assertRaisesRegex(bundle.BundleError, "membership"):
            bundle.external_capsule_source_payloads([external], existing)

    def test_every_standalone_service_lock_is_required(self) -> None:
        missing = self.repo / "services/astrid-edge-provider-broker/Cargo.lock"
        git(self.repo, "rm", "--quiet", str(missing.relative_to(self.repo)))
        git(self.repo, "commit", "--quiet", "-m", "remove required service lock")
        with self.assertRaisesRegex(
            bundle.BundleError, "required tracked CPU-edge source.*provider-broker"
        ):
            bundle.build_bundle(self._build_args(self.root / "missing-service-lock.tar.gz"))

    def test_every_standalone_service_manifest_is_required(self) -> None:
        missing = self.repo / "services/astrid-edge-provider-broker/Cargo.toml"
        git(self.repo, "rm", "--quiet", str(missing.relative_to(self.repo)))
        git(self.repo, "commit", "--quiet", "-m", "remove required service manifest")
        with self.assertRaisesRegex(
            bundle.BundleError, "required tracked CPU-edge source.*provider-broker"
        ):
            bundle.build_bundle(self._build_args(self.root / "missing-service-manifest.tar.gz"))

    def test_repository_gitignore_exposes_every_required_lock(self) -> None:
        repository = SCRIPT.parent.parent
        required = [
            repository / f"capsules/astralis/{capsule}/Cargo.lock"
            for capsule in bundle.LOCAL_EDGE_CAPSULES
        ]
        required.extend(
            repository / f"services/{service}/Cargo.lock"
            for service in bundle.EDGE_STANDALONE_SERVICES
        )
        for path in required:
            self.assertTrue(path.is_file(), str(path))
            ignored = subprocess.run(
                ["git", "-C", str(repository), "check-ignore", "--quiet", "--no-index", str(path)],
                check=False,
            )
            self.assertNotEqual(ignored.returncode, 0, str(path))
            self.assertEqual(
                bundle.source_role(path.relative_to(repository).as_posix()),
                "mutable_build_manifest"
                if "astrid-edge-runtime" in path.parts
                else "inspect_only_immutable_boundary"
                if "services" in path.parts
                else "mutable_build_manifest",
                str(path),
            )
        recipe_packages: set[str] = set()
        for recipe in (
            "astralis-cpu-edge-capsules.toml",
            "astralis-cpu-edge-baseline-capsules.toml",
        ):
            document = tomllib.loads(
                (repository / "packaging/headless" / recipe).read_text(encoding="utf-8")
            )
            for capsule in document["capsule"]:
                self.assertRegex(capsule["lock_sha256"], r"^[0-9a-f]{64}$")
                recipe_packages.add(capsule["package"])
        self.assertEqual(recipe_packages, set(bundle.EXTERNAL_EDGE_CAPSULES))

    def test_tampered_edge_capsule_lock_fails_vendor_binding(self) -> None:
        changed = self.repo / "capsules/astralis/astrid-capsule-cli/Cargo.lock"
        write(
            changed,
            changed.read_text(encoding="utf-8").replace(self.package_checksum, "f" * 64),
        )
        git(self.repo, "add", str(changed.relative_to(self.repo)))
        git(self.repo, "commit", "--quiet", "-m", "tamper capsule lock")
        with self.assertRaisesRegex(bundle.BundleError, "conflicting checksums.*Cargo.lock"):
            bundle.build_bundle(self._build_args(self.root / "tampered-capsule-lock.tar.gz"))

    def test_quickjs_kernel_is_required_and_wasm_header_is_validated(self) -> None:
        kernel = self.repo / bundle.QUICKJS_KERNEL_PATH
        git(self.repo, "rm", "--quiet", str(kernel.relative_to(self.repo)))
        git(self.repo, "commit", "--quiet", "-m", "remove tracked QuickJS kernel")
        with self.assertRaisesRegex(bundle.BundleError, "tracked CPU-edge source is absent.*engine.wasm"):
            bundle.build_bundle(self._build_args(self.root / "missing-kernel.tar.gz"))
        write(kernel, b"not-wasm")
        git(self.repo, "add", "-f", str(kernel.relative_to(self.repo)))
        git(self.repo, "commit", "--quiet", "-m", "add invalid QuickJS kernel")
        with self.assertRaisesRegex(bundle.BundleError, "invalid WASM header"):
            bundle.build_bundle(self._build_args(self.root / "bad-kernel.tar.gz"))

    def test_quickjs_tracked_blake3_record_is_exact(self) -> None:
        sidecar = self.repo / bundle.QUICKJS_KERNEL_HASH_PATH
        write(sidecar, f"{'b' * 64} *engine.wasm\n")
        git(self.repo, "add", str(sidecar.relative_to(self.repo)))
        git(self.repo, "commit", "--quiet", "-m", "malform QuickJS hash record")
        with self.assertRaisesRegex(bundle.BundleError, "invalid exact record"):
            bundle.build_bundle(self._build_args(self.root / "bad-kernel-hash.tar.gz"))

    def test_exact_rust_194_metadata_and_manifest_contract_are_required(self) -> None:
        write(self.rustc, self._rustc_metadata("1.95.0"))
        with self.assertRaisesRegex(bundle.BundleError, "exact reviewed Rust 1.94"):
            bundle.build_bundle(self._build_args(self.root / "rust.tar.gz"))

        write(self.rustc, self._rustc_metadata())
        manifest = self.repo / "services/astrid-edge-runtime/Cargo.toml"
        write(
            manifest,
            '[package]\nname="astrid-edge-runtime"\nversion="0.1.0"\nrust-version="1.93"\n',
        )
        git(self.repo, "add", str(manifest.relative_to(self.repo)))
        git(self.repo, "commit", "--quiet", "-m", "wrong msrv")
        with self.assertRaisesRegex(bundle.BundleError, "rust-version 1.94"):
            bundle.build_bundle(self._build_args(self.root / "msrv.tar.gz"))

    def test_signing_key_permissions_and_wrong_key_fail_closed(self) -> None:
        archive = self.root / "signed.tar.gz"
        self.key.chmod(0o644)
        with self.assertRaisesRegex(bundle.BundleError, "owner-only"):
            bundle.build_bundle(self._build_args(archive))
        self.key.chmod(0o600)
        bundle.build_bundle(self._build_args(archive))
        wrong = self.root / "wrong.hmac"
        write(wrong, b"W" * 32, 0o600)
        with self.assertRaisesRegex(bundle.BundleError, "HMAC signature is invalid"):
            bundle.verify_bundle(self._verify_args(archive, key=wrong))

    def test_existing_output_is_never_overwritten(self) -> None:
        output = self.root / "existing.tar.gz"
        write(output, b"keep")
        with self.assertRaisesRegex(bundle.BundleError, "refusing to overwrite"):
            bundle.build_bundle(self._build_args(output))
        self.assertEqual(output.read_bytes(), b"keep")

    def test_verify_rejects_corruption_extra_members_and_links(self) -> None:
        archive = self.root / "signed.tar.gz"
        bundle.build_bundle(self._build_args(archive))

        corrupt = self.root / "corrupt.tar.gz"
        damaged = bytearray(archive.read_bytes())
        damaged[len(damaged) // 2] ^= 0x01
        corrupt.write_bytes(damaged)
        with self.assertRaises(bundle.BundleError):
            bundle.verify_bundle(self._verify_args(corrupt, key=self.key))

        extra = self.root / "extra.tar.gz"
        self._repack(archive, extra, extra_regular=True)
        with self.assertRaisesRegex(bundle.BundleError, "inventory mismatch"):
            bundle.verify_bundle(self._verify_args(extra, key=self.key))

        linked = self.root / "linked.tar.gz"
        self._repack(archive, linked, hardlink=True)
        with self.assertRaisesRegex(bundle.BundleError, "non-regular member"):
            bundle.verify_bundle(self._verify_args(linked, key=self.key))

    @staticmethod
    def _repack(source: Path, destination: Path, *, extra_regular: bool = False, hardlink: bool = False) -> None:
        with tarfile.open(source, "r:gz") as original:
            records = []
            for member in original.getmembers():
                extracted = original.extractfile(member)
                assert extracted is not None
                records.append((member.name, member.mode, extracted.read()))
        with destination.open("wb") as raw:
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as target:
                    for name, mode, data in records:
                        info = tarfile.TarInfo(name)
                        info.mode = mode
                        info.mtime = 0
                        info.size = len(data)
                        target.addfile(info, io.BytesIO(data))
                    if extra_regular:
                        data = b"unexpected\n"
                        info = tarfile.TarInfo(f"{bundle.BUNDLE_ROOT}/unexpected.txt")
                        info.mode = 0o644
                        info.mtime = 0
                        info.size = len(data)
                        target.addfile(info, io.BytesIO(data))
                    if hardlink:
                        info = tarfile.TarInfo(f"{bundle.BUNDLE_ROOT}/linked")
                        info.type = tarfile.LNKTYPE
                        info.linkname = f"{bundle.BUNDLE_ROOT}/MANIFEST.json"
                        info.mode = 0o644
                        info.mtime = 0
                        target.addfile(info)


if __name__ == "__main__":
    unittest.main()
