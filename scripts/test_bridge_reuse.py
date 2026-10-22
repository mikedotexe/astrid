"""Synthetic input identity tests; no real compiler or services."""
import copy
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import bridge_reuse as reuse


class ReuseInputsTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.source = Path(temporary.name).resolve()
        self.package = self.source / "crate"
        self.package.mkdir()
        (self.package / "Cargo.toml").write_text('[package]\nname="fixture"\n')
        self.tool = str(self.source / "scripts/tool.py")
        self.snapshot = {"head": "a" * 40, "packages": [str(self.package)], "files": [
            {"path": str(self.package / "source.rs"), "sha256": "source", "mode": 420, "size": 20, "mtime_ns": 1},
            {"path": self.tool, "sha256": "tool", "mode": 420, "size": 20, "mtime_ns": 1}]}
        self.context = {"environment_sha256": "env", "versions": {"rustc": "fixture"}}

    def contract(self, snapshot=None, context=None):
        return reuse.compiled_inputs(self.source, snapshot or self.snapshot, "native-test",
                                     context or self.context, {self.tool})

    def test_mtime_and_tool_only_commit_do_not_invalidate_compilation(self):
        before = self.contract()
        changed = copy.deepcopy(self.snapshot)
        changed["head"] = "b" * 40
        changed["files"][0]["mtime_ns"] = 900
        changed["files"][1]["sha256"] = "new deployment tool"
        self.assertEqual(before, self.contract(changed))

    def test_local_build_script_binds_head(self):
        (self.package / "build.rs").write_text("fn main() {}")
        before = self.contract()
        self.snapshot["head"] = "b" * 40
        self.assertNotEqual(before, self.contract())

    def test_source_modes_toolchain_environment_and_dependencies_invalidate(self):
        before = self.contract()
        for field, value in (("sha256", "new"), ("mode", 493), ("size", 30), ("path", "/other/source.rs")):
            changed = copy.deepcopy(self.snapshot)
            changed["files"][0][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "do not match"):
                reuse.matching_inputs(before, self.contract(changed))
        for field in ("environment_sha256", "versions", "dependencies"):
            changed = {**self.context, field: "changed"}
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "context"):
                reuse.matching_inputs(before, self.contract(context=changed))

    def test_tool_inside_package_is_not_excluded(self):
        self.snapshot["files"][1]["path"] = str(self.package / "tool.rs")
        self.tool = self.snapshot["files"][1]["path"]
        self.assertEqual(len(self.contract()["files"]), 2)

    def test_missing_local_dependency_is_not_reusable(self):
        with self.assertRaisesRegex(ValueError, "local dependency missing"):
            self.contract(context={"local_packages": ["/unbound/local/dependency"]})

    def test_unqualified_custom_toolchain_can_be_recorded_but_not_reused(self):
        contract = self.contract(context={"unqualified_native_overrides": ["CC"]})
        self.assertTrue(contract["reuse_limitations"])
        with self.assertRaisesRegex(ValueError, "reuse not qualified.*CC"):
            reuse.matching_inputs(contract, contract)

    def test_custom_cargo_environment_cannot_hide_external_inputs(self):
        config = self.source / ".cargo/config.toml"
        config.parent.mkdir()
        config.write_text('[env]\nCC="/custom/compiler"\n')
        self.snapshot["files"].append({"path": str(config), "sha256": reuse.file_hash(config)})
        contract = self.contract()
        with self.assertRaisesRegex(ValueError, "custom Cargo"):
            reuse.matching_inputs(contract, contract)

    def test_other_target_configuration_does_not_block_current_target_reuse(self):
        config = self.source / ".cargo/config.toml"
        config.parent.mkdir()
        config.write_text('[target.x86_64-unknown-linux-gnu]\nrustflags=["-C", "link-args=-Wl,--allow-multiple-definition"]\n')
        self.snapshot["files"].append({"path": str(config), "sha256": reuse.file_hash(config)})
        contract = self.contract()
        self.assertEqual(contract["reuse_limitations"], [])
        reuse.matching_inputs(contract, contract)
        config.write_text('[target.native-test]\nlinker="/unbound/linker"\n')
        self.snapshot["files"][-1]["sha256"] = reuse.file_hash(config)
        self.assertTrue(self.contract()["reuse_limitations"])

    def test_dependency_bytes_modes_and_symlinks_are_bound(self):
        path = self.package / "source.rs"
        path.write_text("original")
        before = reuse.dependency_tree(self.package)
        path.write_text("different")
        self.assertNotEqual(before, reuse.dependency_tree(self.package))
        path.unlink()
        path.symlink_to(self.package / "Cargo.toml")
        with self.assertRaisesRegex(ValueError, "non-regular"):
            reuse.dependency_tree(self.package)

    def test_actual_child_environment_omits_only_shell_bookkeeping(self):
        with patch.dict(os.environ, {"PATH": "/tools", "SECRET": "not-in-receipt", "PWD": "/ephemeral",
                                    "CFLAGS": "-O2", "ASTRID_SANCTIONED_BRIDGE_STAGE": "1"}, clear=True):
            actual = reuse.build_environment()
        self.assertEqual(actual, {"PATH": "/tools", "SECRET": "not-in-receipt", "CFLAGS": "-O2"})
        self.assertNotIn("not-in-receipt", reuse.digest(actual))

    def test_fixed_build_recipes_preserve_all_three_binaries_and_flags(self):
        commands = reuse.build_commands(self.source, "native-test", self.source / "build")
        self.assertEqual(len(commands), 3)
        for command, binary in zip(commands, reuse.BINARIES.values()):
            self.assertEqual(command[:2], ["cargo", "build"])
            self.assertEqual(command[command.index("--bin") + 1], binary)
            for required in ("--release", "--locked", "--offline"):
                self.assertIn(required, command)

    def test_resolved_dependency_bytes_are_bound_and_unrelated_packages_excluded(self):
        dependency = self.source / "registry-dependency"
        dependency.mkdir()
        (dependency / "lib.rs").write_text("dependency original")
        roots = ["spectral-bridge-server", "astrid-source-study", "astrid-public-web"]
        metadata = {"packages": [
            *[{"id": name, "name": name, "source": None,
               "manifest_path": str(self.package / "Cargo.toml")} for name in roots],
            {"id": "dep", "name": "dep", "source": "registry:fixture", "manifest_path": str(dependency / "Cargo.toml")},
            {"id": "unrelated", "name": "unused", "source": "registry:fixture", "manifest_path": "/absent/Cargo.toml"}],
            "resolve": {"nodes": [*[{"id": name, "dependencies": ["dep"]} for name in roots],
                                  {"id": "dep", "dependencies": []}]}}
        def output(args, *unused):
            return json.dumps(metadata) if args[:2] == ["cargo", "metadata"] else "version fixture"
        with patch.object(reuse, "output", side_effect=output), \
             patch.object(reuse, "executable", return_value={"sha256": "fixture"}), \
             patch.object(reuse.platform, "system", return_value="Linux"):
            before = reuse.compiler_context(self.source, "fixture", {"PATH": "/tools"})
            (dependency / "lib.rs").write_text("dependency changed")
            after = reuse.compiler_context(self.source, "fixture", {"PATH": "/tools"})
        self.assertEqual(len(before["dependencies"]), 1)
        self.assertNotEqual(before["dependencies"], after["dependencies"])
        self.assertEqual(before["local_packages"], [str(self.package)])


if __name__ == "__main__":
    unittest.main()
