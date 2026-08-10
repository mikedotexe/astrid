#!/usr/bin/env python3
"""Offline tests for the external Astralis compatibility build recipe."""

from __future__ import annotations

import hashlib
import shutil
import tarfile
import tempfile
import unittest
from pathlib import Path

from scripts import build_astralis_cpu_edge_capsules as builder


class AstralisCpuEdgeCapsuleBuildTests(unittest.TestCase):
    def test_operator_builder_rejects_python_3_10_before_tomllib_import(self) -> None:
        with self.assertRaisesRegex(SystemExit, "operator-side builder.*Python 3.11"):
            builder.require_supported_python((3, 10, 12))
        builder.require_supported_python((3, 11, 0))

    def test_tracked_recipe_and_all_sdk_locks_verify_offline(self) -> None:
        spec, recipes = builder.load_and_verify_spec(builder.DEFAULT_SPEC)
        self.assertEqual(spec["sdk_version"], "0.6.0")
        self.assertEqual(spec["source_policy"], "reviewed_patch_replay")
        self.assertEqual(
            [recipe.capsule_id for recipe in recipes],
            ["react", "prompt-builder", "openai-compat", "context-engine"],
        )
        react = recipes[0]
        terminal = [
            step
            for step in react.raw["steps"]
            if step.get("file", "").endswith("react-terminal-next.patch")
        ]
        self.assertEqual(len(terminal), 1)
        self.assertEqual(
            terminal[0]["expect_before"],
            "990e702bbf2cca49191d3d10ed040c50bb9c9dbc",
        )
        self.assertEqual(terminal[0]["expect_after"], react.raw["source_blob_lib_rs"])

    def test_every_rust_subprocess_uses_the_reviewed_toolchain(self) -> None:
        supplied = {"PATH": "/bin", "RUSTUP_TOOLCHAIN": "untrusted-override"}
        cargo = builder.command_environment(["cargo", "build"], supplied)
        rustc = builder.command_environment(["rustc", "--version"], None)
        self.assertIsNot(cargo, supplied)
        self.assertEqual(cargo["PATH"], "/bin")
        self.assertEqual(cargo["RUSTUP_TOOLCHAIN"], builder.PINNED_RUST_TOOLCHAIN)
        self.assertEqual(rustc["RUSTUP_TOOLCHAIN"], builder.PINNED_RUST_TOOLCHAIN)
        self.assertIsNone(builder.command_environment(["git", "status"], None))

    def test_exact_upstream_baseline_recipe_is_fully_pinned(self) -> None:
        spec, recipes = builder.load_and_verify_spec(builder.DEFAULT_BASELINE_SPEC)
        self.assertEqual(spec["sdk_version"], "0.7.1")
        self.assertEqual(spec["source_policy"], "pinned_upstream_patch_replay")
        self.assertEqual(
            [recipe.capsule_id for recipe in recipes],
            ["session", "identity", "router", "registry", "system", "hook-bridge"],
        )
        for recipe in recipes:
            self.assertRegex(recipe.raw["source_blob_cargo_lock"], r"^[0-9a-f]{40}$")
            self.assertNotIn("lockfile", recipe.raw)
        registry = next(recipe for recipe in recipes if recipe.capsule_id == "registry")
        self.assertEqual(len(registry.raw["steps"]), 1)
        self.assertEqual(registry.raw["steps"][0]["kind"], "patch")

    def test_recipe_rejects_tampered_patch_before_clone_or_build(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            copied = root / "packaging/headless"
            shutil.copytree(builder.REPO_ROOT / "packaging/headless", copied)
            patch = copied / "astralis-sdk-0.6-react-terminal-next.patch"
            patch.write_bytes(patch.read_bytes() + b"\n")
            with self.assertRaisesRegex(builder.BuildError, "patch hash mismatch"):
                builder.load_and_verify_spec(
                    copied / "astralis-cpu-edge-capsules.toml", root
                )

    def test_repository_inputs_reject_traversal_and_symlinks(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "inside").write_text("ok", encoding="utf-8")
            with self.assertRaises(builder.BuildError):
                builder.checked_repo_path(root, "../outside")
            (root / "link").symlink_to(root / "inside")
            with self.assertRaises(builder.BuildError):
                builder.checked_repo_path(root, "link")

    def test_git_blob_id_matches_canonical_empty_blob(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "empty"
            path.write_bytes(b"")
            self.assertEqual(
                builder.git_blob_id(path), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
            )

    def test_capsule_archive_is_byte_stable_and_metadata_neutral(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = root / "Capsule.toml"
            manifest.write_text("[package]\nname = \"fixture\"\n", encoding="utf-8")
            wasm = root / "fixture.wasm"
            wasm.write_bytes(b"\0asm\r\0\1\0fixture")
            first = root / "first.capsule"
            second = root / "second.capsule"
            builder.deterministic_capsule(first, manifest, wasm)
            builder.deterministic_capsule(second, manifest, wasm)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            self.assertEqual(
                hashlib.sha256(first.read_bytes()).hexdigest(),
                hashlib.sha256(second.read_bytes()).hexdigest(),
            )
            with tarfile.open(first, "r:gz") as archive:
                members = archive.getmembers()
                self.assertEqual([member.name for member in members], ["Capsule.toml", "fixture.wasm"])
                for member in members:
                    self.assertEqual(member.mtime, 0)
                    self.assertEqual(member.uid, 0)
                    self.assertEqual(member.gid, 0)
                    self.assertEqual(member.mode, 0o644)
                    self.assertFalse(member.issym())

    def test_external_source_snapshot_is_bounded_and_rejects_links(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            for relative, body in {
                "Cargo.toml": "[package]\nname='fixture'\n",
                "Cargo.lock": "version = 4\n",
                "Capsule.toml": "[package]\nname='fixture'\n",
                "src/lib.rs": "const NOTE: &str = include_str!(\"note.md\");\n",
                "src/note.md": "bounded resource\n",
                "README.md": "not a build input\n",
            }.items():
                path = source / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(body, encoding="utf-8")
            destination = root / "export" / "fixture"
            inventory = builder.export_source_snapshot(source, destination, "fixture")
            self.assertEqual(
                {record["path"] for record in inventory},
                {
                    "fixture/Cargo.toml",
                    "fixture/Cargo.lock",
                    "fixture/Capsule.toml",
                    "fixture/src/lib.rs",
                    "fixture/src/note.md",
                },
            )
            self.assertFalse((destination / "README.md").exists())

            linked_source = root / "linked-source"
            shutil.copytree(source, linked_source)
            (linked_source / "src/note.md").unlink()
            (linked_source / "src/note.md").symlink_to(linked_source / "src/lib.rs")
            with self.assertRaisesRegex(builder.BuildError, "linked/special"):
                builder.export_source_snapshot(
                    linked_source, root / "linked-export", "fixture"
                )

    def test_unknown_capsule_selection_fails_closed(self) -> None:
        _, recipes = builder.load_and_verify_spec(builder.DEFAULT_SPEC)
        with self.assertRaisesRegex(builder.BuildError, "unknown capsule"):
            builder.select_recipes(recipes, ["typo"])

    def test_output_surface_rejects_unmanifested_archives(self) -> None:
        _, recipes = builder.load_and_verify_spec(builder.DEFAULT_SPEC)
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            (output / "foreign.capsule").write_bytes(b"stale")
            with self.assertRaisesRegex(builder.BuildError, "outside this build manifest"):
                builder.verify_output_surface(output, [recipes[0]])


if __name__ == "__main__":
    unittest.main()
