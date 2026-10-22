"""Real temporary Git trees; no live state, services or source installation."""
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import deploy_preflight as deployment
from deploy_preflight import _porcelain_paths, _render, preflight
from restart_minime_agent import LaunchdAgent, ReviewedInstallation


class ScopedPreflightTests(unittest.TestCase):
    _git = staticmethod(deployment.DeployPreflightTests._git)
    _repo = deployment.DeployPreflightTests._repo

    def setUp(self):
        tmp = self.enterContext(tempfile.TemporaryDirectory())
        self.root = self._repo(str(Path(tmp).resolve()))
        self.name = "capsules/spectral-bridge/src/lib.rs"
        self.source = self.root / self.name

    def check(self, **kwargs):
        return preflight(self.root, external_roots=(), **kwargs)

    def inputs(self):
        return {self.name: hashlib.sha256(self.source.read_bytes()).hexdigest()}

    def test_fresh_notes_and_other_components_do_not_extend_wait(self):
        for name in ("CHANGELOG.md", "docs/rollout.md", "tests/new.py", "minime/src/main.rs"):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("unrelated fresh change\n")
        result = self.check()
        self.assertTrue(result["ok"])
        self.assertEqual(result["foreign"]["recent_files"], [])

    def test_engine_and_provider_cli_keep_legacy_activity_policy(self):
        (self.root / "note.md").write_text("fresh note\n")
        for component in ("minime", "model"):
            with self.subTest(component=component):
                result = self.check(activity_scope=deployment.COMPONENTS[component]["activity_scope"])
                self.assertEqual(result["reason"], "foreign_active")
                self.assertEqual(result["foreign"]["scope"], "legacy_tree")
                self.assertIsNone(result["foreign"]["agent_guess"])
                self.assertIn("legacy engine/provider", _render(result))
                with patch.object(deployment, "preflight", return_value={"exit_code": 0}) as check, patch(
                        "builtins.print"):
                    deployment.main(["--component", component, "--repo", str(self.root), "--json"])
                self.assertEqual(check.call_args.kwargs["activity_scope"], "tree")
        result = self.check(activity_scope="tree", reviewed_inputs=self.inputs())
        self.assertEqual(result["reason"], "source_inspection_failed")

    def test_fresh_nested_untracked_component_input_blocks(self):
        path = self.source.parent / "new/nested.rs"
        path.parent.mkdir()
        path.write_text("// fresh\n")
        result = self.check(ack="review")
        self.assertEqual(result["reason"], "foreign_active")
        self.assertEqual(result["foreign"]["recent_files"], [str(path.relative_to(self.root))])
        self.assertIsNone(result["foreign"]["agent_guess"])

    def test_dependency_shared_source_and_build_config_are_in_scope(self):
        for name in ("crates/astrid-capabilities/src/lib.rs", "crates/astrid-storage/src/lib.rs",
                     "capsules/shared/managed_dir.rs", ".cargo/config.toml", "rust-toolchain.toml"):
            with self.subTest(name=name):
                path = self.root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("changed\n")
                result = self.check(ack="review")
                self.assertIn(name, result["foreign"]["recent_files"])

    def test_scope_covers_current_local_dependency_closure(self):
        root = Path(deployment.__file__).resolve().parents[1]
        workspace = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["dependencies"]
        pending, visited = [root / "capsules/spectral-bridge"], set()
        while pending:
            package = pending.pop()
            if package in visited or not package.is_relative_to(root):
                continue
            visited.add(package)
            manifest = tomllib.loads((package / "Cargo.toml").read_text())
            for table in (manifest, *manifest.get("target", {}).values()):
                for section in ("dependencies", "build-dependencies", "dev-dependencies"):
                    for name, spec in table.get(section, {}).items():
                        if not isinstance(spec, dict):
                            continue
                        base = package
                        if spec.get("workspace"):
                            spec, base = workspace[name], root
                        if isinstance(spec, dict) and "path" in spec:
                            pending.append((base / spec["path"]).resolve())
        for package in visited - {root / "capsules/spectral-bridge"}:
            self.assertIn(str(package.relative_to(root)), deployment.BRIDGE_BUILD_PATHS)

    def test_reviewed_install_does_not_waive_dirty_ack(self):
        self.source.write_text("// reviewed\n")
        result = self.check(reviewed_inputs=self.inputs())
        self.assertEqual(result["reason"], "dirty_no_ack")
        result = self.check(ack="reviewed installation", reviewed_inputs=self.inputs())
        self.assertTrue(result["ok"])
        self.assertEqual(result["foreign"]["reviewed_files"], [self.name])

    def test_unreviewed_source_and_other_repo_are_not_exempt(self):
        self.source.write_text("// reviewed\n")
        extra = self.source.parent / "other.rs"
        extra.write_text("// unreviewed\n")
        result = self.check(ack="review", reviewed_inputs=self.inputs())
        self.assertEqual(result["reason"], "foreign_active")
        self.assertEqual(result["foreign"]["recent_files"], [str(extra.relative_to(self.root))])

    def test_changed_reviewed_input_refuses_even_if_clean_or_old(self):
        inputs = self.inputs()
        self.source.write_text("// drift\n")
        self._git(self.root, "add", self.name)
        self._git(self.root, "commit", "-qm", "drift")
        result = self.check(ack="review", reviewed_inputs=inputs)
        self.assertEqual(result["reason"], "source_inspection_failed")
        self.assertIn("reviewed source changed", _render(result))

    def test_path_escape_and_symlink_never_receive_review_exemption(self):
        for name in ("../outside", "/outside", ""):
            with self.subTest(name=name):
                self.assertFalse(self.check(ack="review", reviewed_inputs={name: "a" * 64})["ok"])
        saved = self.root / "saved.rs"
        inputs = self.inputs()
        self.source.rename(saved)
        self.source.symlink_to(saved)
        self.assertEqual(self.check(ack="review", reviewed_inputs=inputs)["reason"],
                         "source_inspection_failed")

    def test_status_failure_and_incomplete_output_do_not_become_clean(self):
        for failure in (subprocess.TimeoutExpired("git", 10), OSError("unavailable"),
                        subprocess.CalledProcessError(128, "git")):
            with self.subTest(failure=type(failure)), patch("deploy_preflight.subprocess.run", side_effect=failure):
                self.assertEqual(self.check(ack="review")["reason"], "source_inspection_failed")
        for out in (" M truncated", "R  destination\0", "x\0"):
            with self.subTest(out=out), patch("deploy_preflight.subprocess.run",
                    return_value=SimpleNamespace(stdout=out)):
                self.assertEqual(self.check()["reason"], "source_inspection_failed")

    def test_rename_and_literal_arrow_names_preserve_both_paths(self):
        new_name = "capsules/spectral-bridge/src/literal -> target.rs"
        self._git(self.root, "mv", self.name, new_name)
        paths = _porcelain_paths(self.root, ["capsules/spectral-bridge/src"])
        self.assertEqual(set(paths), {self.name, new_name})
        self.assertFalse(self.check(ack="review")["ok"])

    def test_deleted_source_uses_directory_change_not_clean_fallback(self):
        self.source.unlink()
        result = self.check(ack="review")
        self.assertEqual(result["reason"], "foreign_active")
        self.assertIn(self.name, result["dirty_files"])

    def test_reviewed_inventory_keeps_process_local_hold_and_file_pins(self):
        backend = SimpleNamespace(root=self.root, inputs=self.inputs)
        hold = {"owned": True}
        def verify_hold():
            if not hold["owned"]:
                raise RuntimeError("hold changed")
        review = ReviewedInstallation(backend, self.inputs(), verify_hold)
        self.assertEqual(review.validate(backend), self.inputs())
        (self.root / "note.md").write_text("unrelated\n")
        self.assertEqual(review.validate(backend), self.inputs())
        hold["owned"] = False
        with self.assertRaisesRegex(RuntimeError, "hold changed"):
            review.validate(backend)
        hold["owned"] = True
        os.chmod(self.source, 0o700)
        with self.assertRaisesRegex(RuntimeError, "installation changed"):
            review.validate(backend)

    def test_added_inventory_and_equal_bytes_replacement_invalidate_review(self):
        backend = SimpleNamespace(root=self.root, inputs=self.inputs)
        review = ReviewedInstallation(backend, self.inputs(), lambda: None)
        backend.inputs = lambda: {**self.inputs(), "extra.py": "a" * 64}
        with self.assertRaisesRegex(RuntimeError, "installation changed"):
            review.validate(backend)
        backend.inputs = self.inputs
        raw = self.source.read_bytes()
        replacement = self.source.with_suffix(".new")
        replacement.write_bytes(raw)
        replacement.replace(self.source)
        with self.assertRaisesRegex(RuntimeError, "installation changed"):
            review.validate(backend)

    def test_real_backend_passes_review_only_to_agent_scope(self):
        backend = LaunchdAgent(1, "review")
        backend.root = self.root
        expected = self.inputs()
        backend.inputs = lambda: expected.copy()
        backend.maintenance = lambda: None
        backend.job = lambda: "\targuments = {\n\tpython\n\t}\n"
        backend.reviewed_installation = ReviewedInstallation(backend, self.inputs(), lambda: None)
        with patch("research_release_compatibility.verify_minime"), patch(
                "restart_minime_agent.Path.read_bytes", return_value=b"plist"), patch(
                "restart_minime_agent.plistlib.loads", return_value={"KeepAlive": True,
                    "ProgramArguments": ["python"]}), patch(
                "restart_minime_agent.preflight", return_value={"ok": True}) as check:
            backend.validate()
        self.assertEqual(check.call_count, 2)
        self.assertEqual(check.call_args_list[0].kwargs["reviewed_inputs"], self.inputs())
        self.assertIsNone(check.call_args_list[1].kwargs["reviewed_inputs"])


if __name__ == "__main__":
    unittest.main()
