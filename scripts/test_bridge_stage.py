"""Staging transaction tests use only owned temporary files and a fake compiler."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import bridge_stage as stage


class StageTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="bridge-stage-test-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve()
        self.source = self.root / "source"
        self.package = self.source / "capsules/spectral-bridge"
        (self.package / "src").mkdir(parents=True)
        (self.package / "src/main.rs").write_text("fn main() {}")
        (self.package / "Cargo.toml").write_text("[package]\n")
        for name in stage.TOOLS:
            path = self.source / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("# fixture\n")
        self.directory = self.root / "stages/one"
        self.live = self.root / "live"
        self.live.mkdir()
        (self.live / "binary").write_bytes(b"live binary")
        (self.live / "manifest.json").write_bytes(b"live manifest")

    def compiler(self, args, **kwargs):
        self.assertEqual(args[:2], ["cargo", "build"])
        self.assertIn("--locked", args)
        self.assertIn("--offline", args)
        target = Path(args[args.index("--target-dir") + 1])
        target = target / args[args.index("--target") + 1]
        (target / "release").mkdir(parents=True, exist_ok=True)
        binary = args[args.index("--bin") + 1]
        (target / "release" / binary).write_bytes(b"synthetic binary")
        return subprocess.CompletedProcess(args, 0)

    def command(self, args, **kwargs):
        if args[:2] == ["git", "rev-parse"]:
            return "a" * 40
        if args[0] == "rustc":
            return "fixture toolchain\nhost: aarch64-apple-darwin"
        if args[0] == "cargo":
            return "fixture toolchain"
        self.assertEqual(args[-1], "--verify-deployment-manifest")
        return json.dumps({"verified": True, "manifest_sha256": stage.sha(Path(args[-2])),
                           "binary_sha256": stage.sha(Path(args[0])), "authority": stage.AUTHORITY,
                           "live_authority_granted": False, "live_eligible_now": False})

    def manifest(self, output, **kwargs):
        value = {"schema": "stack_build_manifest_v1", "authority": "build_manifest_witness_not_deploy_authority",
                 "repository": {"available": True, "head": "a" * 40},
                 "artifacts": {name: {"path": str(path), "exists": True, "sha256": stage.sha(path)}
                               for name, path in kwargs["artifacts"].items()}}
        stage.atomic_json(output, value)
        return value

    def build(self, compiler=None):
        with patch.object(stage, "local_packages", return_value=[self.package]), \
             patch.object(stage, "command", side_effect=self.command), \
             patch.object(stage, "write_build_manifest", side_effect=self.manifest), \
             patch.object(stage.subprocess, "run", side_effect=compiler or self.compiler), \
             patch.object(stage, "check_environment"):
            return stage.stage_build(self.source, self.directory, "test-operator", "isolated fixture")

    def test_build_preserves_live_files_and_verifies_versioned_artifacts(self):
        ready = self.build()
        self.assertEqual(ready["status"], "staged_verified_not_activated")
        self.assertFalse(ready["activation_performed"])
        self.assertEqual((self.live / "binary").read_bytes(), b"live binary")
        self.assertEqual((self.live / "manifest.json").read_bytes(), b"live manifest")
        self.assertEqual(stage.verify_stage(self.directory, run_binary=False), ready)
        self.assertEqual((self.directory / "helpers/substrate_probe_v2.py").read_text(), "# fixture\n")
        self.assertEqual((self.directory / "helpers/astrid-source-study").read_bytes(), b"synthetic binary")

    def test_shared_reader_tampering_is_rejected(self):
        self.build()
        reader = self.directory / "helpers/astrid-source-study"
        reader.chmod(0o700)
        reader.write_bytes(b"changed reader")
        with self.assertRaisesRegex(ValueError, "artifact changed: source-study-reader"):
            stage.verify_stage(self.directory, run_binary=False)

    def test_compiler_failure_retains_evidence_without_ready_record(self):
        def fail(args, **kwargs):
            kwargs["stdout"].write(b"synthetic compiler failure\n")
            raise subprocess.CalledProcessError(1, args)
        with self.assertRaises(subprocess.CalledProcessError):
            self.build(fail)
        self.assertFalse((self.directory / "ready.json").exists())
        self.assertIn("compiler failure", (self.directory / "build.log").read_text())
        self.assertFalse(stage.json_file(self.directory / "failure.json")["activation_performed"])

    def test_mid_build_source_change_refuses_stage(self):
        def change(args, **kwargs):
            result = self.compiler(args, **kwargs)
            (self.package / "src/main.rs").write_text("fn changed() {}")
            return result
        with self.assertRaisesRegex(ValueError, "changed during compilation"):
            self.build(change)
        self.assertFalse((self.directory / "ready.json").exists())

    def test_existing_stage_is_not_overwritten(self):
        self.build()
        before = (self.directory / "ready.json").read_bytes()
        with self.assertRaisesRegex(ValueError, "existing evidence"):
            self.build()
        self.assertEqual((self.directory / "ready.json").read_bytes(), before)

    def test_tampered_helper_is_rejected(self):
        self.build()
        helper = self.directory / "helpers/substrate_probe_v2.py"
        helper.chmod(0o600)
        helper.write_text("changed")
        with self.assertRaisesRegex(ValueError, "artifact changed"):
            stage.verify_stage(self.directory, run_binary=False)

    def test_manifest_and_source_witness_tampering_are_rejected(self):
        self.build()
        source = self.directory / "source-inputs.json"
        original = source.read_bytes()
        source.write_text("{}")
        with self.assertRaisesRegex(ValueError, "source input witness changed"):
            stage.verify_stage(self.directory, run_binary=False)
        source.write_bytes(original)
        (self.directory / "manifest.json").write_text("{}")
        with self.assertRaisesRegex(ValueError, "manifest changed"):
            stage.verify_stage(self.directory, run_binary=False)

    def test_artifact_symlink_is_rejected(self):
        self.build()
        binary = self.directory / "spectral-bridge-server"
        data = binary.read_bytes()
        binary.unlink()
        target = self.root / "same-bytes"
        target.write_bytes(data)
        binary.symlink_to(target)
        with self.assertRaisesRegex(ValueError, "path mismatch"):
            stage.verify_stage(self.directory, run_binary=False)

    def test_retained_failure_prevents_reusing_ready_record(self):
        self.build()
        stage.atomic_json(self.directory / "failure.json", {"status": "failed_not_activated"})
        with self.assertRaisesRegex(ValueError, "retained failure"):
            stage.verify_stage(self.directory, run_binary=False)

    def test_authority_or_source_reference_mismatch_is_refused(self):
        self.build()
        ready_path = self.directory / "ready.json"
        ready = stage.json_file(ready_path)
        ready["authority"] = "live_authority"
        stage.atomic_json(ready_path, ready)
        with self.assertRaisesRegex(ValueError, "authority boundary"):
            stage.verify_stage(self.directory, run_binary=False)
        ready["authority"] = stage.AUTHORITY
        manifest_path = self.directory / "manifest.json"
        manifest = stage.json_file(manifest_path)
        manifest["source_inputs"]["path"] = str(self.root / "elsewhere")
        stage.atomic_json(manifest_path, manifest)
        ready["manifest_sha256"] = stage.sha(manifest_path)
        stage.atomic_json(ready_path, ready)
        with self.assertRaisesRegex(ValueError, "source reference"):
            stage.verify_stage(self.directory, run_binary=False)

    def test_native_verification_failure_is_not_accepted(self):
        self.build()
        with patch.object(stage, "command", return_value=json.dumps({"verified": False})):
            with self.assertRaisesRegex(ValueError, "native manifest verification"):
                stage.verify_stage(self.directory)

    def test_inventory_excludes_runtime_but_detects_input_change(self):
        workspace = self.package / "workspace"
        workspace.mkdir()
        (workspace / "private.txt").write_text("not a build input")
        with patch.object(stage, "command", side_effect=self.command):
            first = stage.input_snapshot(self.source, [self.package])
            self.assertFalse(any("private.txt" in row["path"] for row in first["files"]))
            (self.package / "src/main.rs").write_text("same status, different bytes")
            self.assertNotEqual(first, stage.input_snapshot(self.source, [self.package]))

    def test_content_comparison_ignores_mtime_but_not_bytes(self):
        with patch.object(stage, "command", side_effect=self.command):
            first = stage.input_snapshot(self.source, [self.package])
            source = self.package / "src/main.rs"
            source.touch()
            touched = stage.input_snapshot(self.source, [self.package])
            self.assertNotEqual(first, touched)
            self.assertTrue(stage.same_input_contents(first, touched))
            source.write_text("different bytes")
            changed = stage.input_snapshot(self.source, [self.package])
            self.assertFalse(stage.same_input_contents(first, changed))
            self.assertTrue(stage.same_input_contents(first, changed, frozenset({str(source)})))
            changed["files"][0]["mode"] ^= 0o100
            self.assertFalse(stage.same_input_contents(first, changed, frozenset({str(source)})))

    def test_input_symlink_is_refused(self):
        (self.package / "src/external.rs").symlink_to(self.live / "binary")
        with patch.object(stage, "command", side_effect=self.command):
            with self.assertRaisesRegex(ValueError, "non-regular input"):
                stage.input_snapshot(self.source, [self.package])

    def test_unreviewed_build_override_is_refused_without_printing_value(self):
        with patch.dict(os.environ, {"RUSTC_WRAPPER": "private-value"}):
            with self.assertRaisesRegex(ValueError, "RUSTC_WRAPPER") as caught:
                stage.check_environment()
        self.assertNotIn("private-value", str(caught.exception))

    def test_standalone_staging_cli_is_refused_without_wrapper(self):
        result = subprocess.run(["python3", "-B", str(Path(stage.__file__)), "build",
                                 "--source-root", str(self.source), "--stage-dir", str(self.directory),
                                 "--actor", "test", "--ack", "fixture"],
                                env={key: value for key, value in os.environ.items() if key != "ASTRID_SANCTIONED_BRIDGE_STAGE"},
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn("build through", result.stdout)
        self.assertFalse(self.directory.exists())


if __name__ == "__main__":
    unittest.main()
