import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import minime_engine_stage as stage


class StageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def fixture(self):
        for folder in ("source/minime/src", "bin"):
            (self.root / folder).mkdir(parents=True)
        source = self.root / "source/minime/src/main.rs"
        source.write_text("fn main() {}")
        artifacts = {}
        for name in stage.BINS:
            path = self.root / "bin" / name
            path.write_bytes(b"synthetic executable")
            artifacts[name] = {"path": f"bin/{name}", "sha256": stage.digest(path)}
        (self.root / "build.log").write_text("synthetic build")
        stage.write_new(self.root / "manifest.json", {
            "schema": stage.SCHEMA, "activation_authorized": False,
            "inputs": {"minime/src/main.rs": {"sha256": stage.digest(source), "bytes": source.stat().st_size}},
            "artifacts": artifacts, "evidence": {"build.log": stage.digest(self.root / "build.log")}})
        return stage.digest(self.root / "manifest.json")

    def test_complete_witness_remains_non_authorizing(self):
        result = stage.verify(self.root, self.fixture())
        self.assertTrue(result["verified"])
        self.assertFalse(result["activation_authorized"])

    def test_all_material_tampering_is_rejected(self):
        expected = self.fixture()
        for name in ("source/minime/src/main.rs", "bin/minime", "bin/engine_restore_inspect", "build.log"):
            with self.subTest(name=name):
                path = self.root / name
                original = path.read_bytes()
                path.write_bytes(b"tampered")
                with self.assertRaises(ValueError):
                    stage.verify(self.root, expected)
                path.write_bytes(original)

    def test_manifest_change_rejected(self):
        expected = self.fixture()
        path = self.root / "manifest.json"
        path.chmod(0o600)
        path.write_text("{}")
        with self.assertRaises(ValueError):
            stage.verify(self.root, expected)

    def test_failed_stage_rejected(self):
        expected = self.fixture()
        (self.root / "FAILED.json").write_text("{}")
        with self.assertRaises(ValueError):
            stage.verify(self.root, expected)

    def test_path_escape_and_symlink_rejected(self):
        (self.root / "linked").symlink_to("/etc/passwd")
        for name in ("../escape", "/etc/passwd", "linked"):
            with self.subTest(name=name), self.assertRaises(ValueError):
                stage.checked_path(self.root, name)

    def test_source_root_symlink_rejected(self):
        expected = self.fixture()
        (self.root / "source").rename(self.root / "retained-source")
        (self.root / "source").symlink_to(self.root / "retained-source", target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symlink"):
            stage.verify(self.root, expected)

    def test_cargo_config_refused(self):
        (self.root / ".cargo").mkdir()
        (self.root / ".cargo/config.toml").write_text("[build]")
        with self.assertRaises(ValueError):
            stage.check_no_cargo_config(self.root, {"HOME": str(self.root)})

    def test_input_inventory_tracks_new_source(self):
        for tree in stage.TREES:
            (self.root / tree).mkdir(parents=True)
        for name in stage.FILES:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("fixture")
        first = stage.inputs(self.root)
        (self.root / "minime/src/new.rs").write_text("new")
        self.assertNotEqual(first, stage.inputs(self.root))

    def test_build_freezes_exact_artifacts_and_refuses_reuse(self):
        self.test_input_inventory_tracks_new_source()
        destination = self.root.parent / (self.root.name + "-stage")
        self.addCleanup(shutil.rmtree, destination, True)
        def command(args, *_):
            if args == ["rustc", "-vV"]:
                return "host: aarch64-apple-darwin"
            if args[:2] == ["git", "status"]:
                return " M minime/src/new.rs"
            if args[:2] == ["cargo", "metadata"]:
                self.assertIn("--offline", args)
                self.assertIn("--locked", args)
                self.assertEqual(args[args.index("--filter-platform") + 1], "aarch64-apple-darwin")
            return "{}" if args[:2] == ["cargo", "metadata"] else "fixture identity"
        def compile_fake(*_, **__):
            folder = destination / "build/release"
            folder.mkdir(parents=True)
            for name in stage.BINS:
                (folder / name).write_bytes(b"compiled fixture")
        with patch.object(stage, "run", command), patch.object(stage.subprocess, "run", compile_fake):
            result = stage.build(self.root, destination)
            self.assertTrue(result["verified"])
            self.assertFalse(result["activation_authorized"])
            with self.assertRaises(FileExistsError):
                stage.build(self.root, destination)

    def test_source_drift_retains_failed_stage(self):
        self.test_input_inventory_tracks_new_source()
        destination = self.root.parent / (self.root.name + "-failed")
        self.addCleanup(shutil.rmtree, destination, True)
        def command(args, *_):
            if args == ["rustc", "-vV"]:
                return "host: aarch64-apple-darwin"
            return "{}" if args[:2] == ["cargo", "metadata"] else "fixture identity"
        def change_source(*_, **__):
            (self.root / "minime/src/new.rs").write_text("concurrent change")
        with patch.object(stage, "run", command), patch.object(stage.subprocess, "run", change_source):
            with self.assertRaisesRegex(ValueError, "source identity changed"):
                stage.build(self.root, destination)
        self.assertTrue((destination / "FAILED.json").is_file())
        self.assertFalse((destination / "manifest.json").exists())

    def test_metadata_failure_retains_bounded_diagnostics(self):
        self.test_input_inventory_tracks_new_source()
        destination = self.root.parent / (self.root.name + "-metadata-failed")
        self.addCleanup(shutil.rmtree, destination, True)
        def command(args, *_):
            if args == ["rustc", "-vV"]:
                return "host: aarch64-apple-darwin"
            if args[:2] == ["cargo", "metadata"]:
                raise subprocess.CalledProcessError(101, args, stderr="uncached " + "x" * 70000)
            return "fixture identity"
        with patch.object(stage, "run", command):
            with self.assertRaises(subprocess.CalledProcessError):
                stage.build(self.root, destination)
        failure = json.loads((destination / "FAILED.json").read_text())
        self.assertEqual(failure["returncode"], 101)
        self.assertEqual(len(failure["stderr"]), 65536)
        self.assertTrue(failure["stderr_truncated"])
        self.assertFalse((destination / "manifest.json").exists())


if __name__ == "__main__":
    unittest.main()
