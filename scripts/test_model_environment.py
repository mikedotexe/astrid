"""Model-free tests for immutable provider selection and owned launch holds."""
from __future__ import annotations

import json
import os
import base64
import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

import model_environment as models
from qualify_model_launchd_handoff import fixture_manifest


class ModelEnvironmentTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.control = self.root / "control"
        self.manifest = fixture_manifest(self.root, "baseline", Path(sys.executable).resolve())

    def rewrite(self, change):
        value = models.read_json(self.manifest)
        change(value)
        models.atomic_json(self.manifest, value)

    def test_atomic_json_collision_preserves_temporary_and_destination(self):
        for existing in (False, True):
            with self.subTest(existing_destination=existing):
                path = self.root / f"collision-{existing}.json"
                if existing:
                    path.write_bytes(b"original destination\n")
                temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
                temporary.write_bytes(b"preexisting temporary evidence\n")
                before = temporary.stat()
                with self.assertRaises(FileExistsError):
                    models.atomic_json(path, {"replacement": True})
                self.assertEqual(temporary.read_bytes(), b"preexisting temporary evidence\n")
                self.assertEqual(temporary.stat().st_ino, before.st_ino)
                self.assertEqual(temporary.stat().st_mtime_ns, before.st_mtime_ns)
                self.assertEqual(path.exists(), existing)
                if existing:
                    self.assertEqual(path.read_bytes(), b"original destination\n")

    def test_atomic_json_failed_write_cleans_own_temporary_only(self):
        path = self.root / "failed-write.json"
        path.write_bytes(b"original destination\n")
        with self.assertRaises(TypeError):
            models.atomic_json(path, {"invalid": object()})
        self.assertEqual(path.read_bytes(), b"original destination\n")
        self.assertFalse(path.with_name(f".{path.name}.{os.getpid()}.tmp").exists())

    def test_check_is_read_only_and_binds_source_python_inventories(self):
        before = {str(p): (p.stat().st_mtime_ns, p.read_bytes())
                  for p in self.root.rglob("*") if p.is_file()}
        result = models.validate_manifest(self.manifest)
        self.assertEqual(result["release_id"], "baseline")
        after = {str(p): (p.stat().st_mtime_ns, p.read_bytes())
                 for p in self.root.rglob("*") if p.is_file()}
        self.assertEqual(before, after)

    def test_framework_runtime_executable_must_be_hash_bound(self):
        runtime = models.interpreter_paths(Path(sys.executable)) - {Path(sys.executable).resolve()}
        if not runtime:
            self.skipTest("interpreter is not a macOS framework build")
        self.rewrite(lambda m: m["artifacts"].pop(str(next(iter(runtime)))))
        with self.assertRaisesRegex(ValueError, "completely bound"):
            models.validate_manifest(self.manifest)

    def test_framework_reexec_accepts_only_exact_bound_runtime_and_remaining_argv(self):
        version = self.root / "Python.framework/Versions/3.12"
        launcher = version / "bin/python3.12"
        runtime = version / "Resources/Python.app/Contents/MacOS/Python"
        for path in (launcher, runtime):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("fixture executable never run")
        expected = [str(launcher), "server.py", "--port", "8090"]
        self.assertEqual(models.interpreter_paths(launcher), {launcher, runtime})
        self.assertTrue(models.command_matches([str(runtime), *expected[1:]], expected))
        self.assertFalse(models.command_matches([str(runtime), "other.py", *expected[2:]], expected))
        self.assertFalse(models.command_matches([str(runtime.with_name("AnotherPython")), *expected[1:]], expected))
        self.assertFalse(models.command_matches([str(runtime), *expected[1:], "--changed"], expected))

    def test_mutable_model_name_is_refused(self):
        self.rewrite(lambda m: m["argv"].__setitem__(3, "mlx-community/model"))
        with self.assertRaisesRegex(ValueError, "absolute"):
            models.validate_manifest(self.manifest)

    def test_current_runtime_state_and_absolute_qos_are_required(self):
        self.rewrite(lambda m: m["runtime_paths"].__setitem__("coupling_journal", str(self.root / "old.json")))
        with self.assertRaisesRegex(ValueError, "coupling journal"):
            models.validate_manifest(self.manifest)

    def test_unbound_source_is_refused(self):
        (self.manifest.parent / "late_import.py").write_text("# changed closure\n")
        with self.assertRaisesRegex(ValueError, "completely bound"):
            models.validate_manifest(self.manifest)

    def test_source_mutation_is_refused(self):
        (self.manifest.parent / "coupled_astrid_server.py").write_text("# changed\n")
        with self.assertRaisesRegex(ValueError, "artifact changed"):
            models.validate_manifest(self.manifest)

    def test_selected_manifest_mutation_is_refused(self):
        models.atomic_json(self.control / "active.json", models.selection(self.manifest))
        self.rewrite(lambda m: m.__setitem__("release_id", "different"))
        with self.assertRaisesRegex(ValueError, "manifest changed"):
            models.selected_release(self.control)

    def refresh_artifact(self, path):
        self.rewrite(lambda m: m["artifacts"].__setitem__(str(path), models.digest(path)))

    def test_inventory_cannot_claim_different_interpreter(self):
        inventory = self.manifest.parent / "packages.json"
        value = models.read_json(inventory)
        value["executable"] = "/different/python"
        models.atomic_json(inventory, value)
        self.refresh_artifact(inventory)
        with self.assertRaisesRegex(ValueError, "different interpreter"):
            models.validate_manifest(self.manifest)

    def test_inventory_and_manifest_versions_must_agree(self):
        self.rewrite(lambda m: m["packages"].__setitem__("mlx", "99.0"))
        with self.assertRaisesRegex(ValueError, "versions differ"):
            models.validate_manifest(self.manifest)

    def test_package_bytes_are_verified_against_record(self):
        site = next((self.manifest.parent / ".venv/lib").glob("python*/site-packages"))
        payload = site / "mlx-fixture.bin"
        payload.write_bytes(b"qualified package payload")
        encoded = base64.urlsafe_b64encode(hashlib.sha256(payload.read_bytes()).digest()).rstrip(b"=").decode()
        record = site / "mlx-0.0.0.dist-info/RECORD"
        with record.open("a") as handle:
            handle.write(f"mlx-fixture.bin,sha256={encoded},{payload.stat().st_size}\n")
        self.refresh_artifact(record)
        models.validate_manifest(self.manifest)
        payload.write_bytes(b"changed after installed inventory")
        with self.assertRaisesRegex(ValueError, "installed package content changed"):
            models.validate_manifest(self.manifest)

    def test_model_weights_must_be_bound_and_unchanged(self):
        weights = self.root / "model-placeholder/model.safetensors"
        self.rewrite(lambda m: m["artifacts"].pop(str(weights)))
        with self.assertRaisesRegex(ValueError, "model snapshot file is not bound"):
            models.validate_manifest(self.manifest)

    def test_modified_model_weight_refuses_load(self):
        (self.root / "model-placeholder/model.safetensors").write_bytes(b"different")
        with self.assertRaisesRegex(ValueError, "artifact changed"):
            models.validate_manifest(self.manifest)

    def test_foreign_hold_is_never_replaced_or_released(self):
        guard = models.create_hold(self.control, "owner", "fixture")
        with self.assertRaises(FileExistsError):
            models.create_hold(self.control, "other", "fixture")
        with self.assertRaisesRegex(RuntimeError, "ownership"):
            models.release_hold(self.control, {**guard, "transaction_id": "other"})
        self.assertEqual(models.hold_record(self.control), guard)

    def test_dangling_symlink_hold_fails_closed(self):
        self.control.mkdir()
        (self.control / "hold.json").symlink_to(self.control / "missing")
        with self.assertRaisesRegex(ValueError, "invalid record"):
            models.hold_record(self.control)

    def start_launcher(self, *options):
        command = [sys.executable, "-B", str(Path(models.__file__).resolve()), "launch",
                   "--control-root", str(self.control), *options]
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        self.addCleanup(lambda: process.poll() is None and process.terminate())
        self.addCleanup(lambda: process.stdout.close())
        self.addCleanup(lambda: process.stderr.close())
        return process

    def wait_for(self, predicate):
        deadline = time.monotonic() + 5
        while not predicate():
            if time.monotonic() > deadline:
                self.fail("fixture condition timed out")
            time.sleep(.02)

    def test_held_launcher_never_reads_release_or_imports_model(self):
        models.create_hold(self.control, "owner", "fixture")
        process = self.start_launcher("--hold-only", "owner")
        receipt = self.control / "held" / f"{process.pid}.json"
        self.wait_for(receipt.exists)
        self.assertFalse(models.read_json(receipt)["model_imported"])
        self.assertFalse((self.root / "executions").exists())
        self.assertFalse((self.control / "active.json").exists())
        process.terminate()
        process.wait(timeout=5)

    def test_one_shot_hold_cannot_execute_when_hold_removed(self):
        guard = models.create_hold(self.control, "owner", "fixture")
        models.atomic_json(self.control / "active.json", models.selection(self.manifest))
        process = self.start_launcher("--hold-only", "owner")
        self.wait_for((self.control / "held" / f"{process.pid}.json").exists)
        models.release_hold(self.control, guard)
        self.assertNotEqual(process.wait(timeout=5), 0)
        self.assertFalse((self.root / "executions").exists())

    def test_permanent_launcher_execs_same_pid_after_owned_hold_release(self):
        guard = models.create_hold(self.control, "owner", "fixture")
        models.atomic_json(self.control / "active.json", models.selection(self.manifest))
        process = self.start_launcher()
        self.wait_for((self.control / "held" / f"{process.pid}.json").exists)
        models.release_hold(self.control, guard)
        self.wait_for((self.root / "executions").exists)
        self.assertEqual((self.root / "executions").read_text().strip(), f"baseline {process.pid}")
        process.terminate()
        self.assertEqual(process.wait(timeout=5), 0)
        self.assertEqual((self.root / "drained").read_text().strip(), f"baseline {process.pid}")


if __name__ == "__main__":
    unittest.main()
