"""Fixture-only checks; never sample or signal a live process."""
import copy
import json
import plistlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import minime_runtime_binding as binding
from environment_receipts import manifest_compatibility

REAL_TOPOLOGY = binding.topology


class RuntimeBindingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.minime = self.root / "minime"
        self.workspace = self.root / "workspace"
        self.manifests = self.workspace / "deployment_manifests"
        self.manifests.mkdir(parents=True)
        self.release = self.minime / "minime/target/release"
        (self.release / "deps").mkdir(parents=True)
        self.retained = self.release / "deps/minime-retained"
        self.retained.write_bytes(b"historical executable")
        self.retained.chmod(0o700)
        (self.release / "minime").write_bytes(b"newer disk executable")
        self.launcher = self.root / "launcher.sh"
        self.launcher.write_text("original launcher")
        self.historical = self.manifests / "minime-division-runtime.json"
        self.historical.write_text(json.dumps({
            "component": "minime-division-runtime",
            "protocol": {"version": "1.1", "revision": "fixture-rev"},
            "artifacts": {"minime-engine": {"path": str(self.release / "minime"), "sha256": binding.digest(self.retained)},
                          "gateway-launcher": binding.artifact(self.launcher)},
        }))
        self.original = self.historical.read_bytes()
        self.uuid = "56189890-33D5-3600-A0EB-5CB343B09050"
        self.processes = {label: {"label": label, "pid": 100 + i, "started_at": "fixture-start"} for i, label in enumerate(binding.LABELS)}
        self.config = {"launcher": binding.artifact(self.launcher)}
        self.patches = [
            patch.object(binding, "topology", return_value=self.config),
            patch.object(binding, "process", side_effect=lambda label: dict(self.processes[label])),
            patch.object(binding, "port_owner", return_value=101),
            patch.object(binding, "run", side_effect=self.fake_run),
        ]
        for item in self.patches:
            item.start()
            self.addCleanup(item.stop)
        self.path = binding.capture(self.minime, self.workspace, self.root / "installed")
        self.payload = json.loads(self.path.read_text())

    def fake_run(self, args, timeout=20):
        if args[0] == "sample":
            return f"Private fixture stacks: NEVER PERSIST\n  0x100 - 0x200 +minime (0) <{self.uuid}> /redacted/minime\n"
        return f"UUID: {self.uuid} (arm64) fixture\n"

    def test_capture_retains_exact_build_without_rewriting_history_or_disk(self):
        self.assertEqual(binding.validate(self.payload), [])
        self.assertEqual(self.historical.read_bytes(), self.original)
        self.assertEqual((self.release / "minime").read_bytes(), b"newer disk executable")
        self.assertFalse(self.payload["memory_bytes_attested"])
        self.assertNotIn("built_at", self.payload)
        self.assertNotIn("Private fixture stacks", self.path.read_text())
        self.assertEqual(self.path.stat().st_mode & 0o777, 0o600)
        archived = Path(self.payload["artifacts"]["retained-minime"]["path"])
        self.assertEqual(archived.read_bytes(), self.retained.read_bytes())
        self.assertEqual(archived.stat().st_mode & 0o777, 0o600)

    def test_historical_manifest_change_fails(self):
        self.historical.write_text(self.historical.read_text() + "\n")
        self.assertIn("historical build manifest changed", binding.validate(self.payload))

    def test_wrong_retained_bytes_fail(self):
        Path(self.payload["artifacts"]["retained-minime"]["path"]).write_bytes(b"wrong")
        self.assertTrue(binding.validate(self.payload))

    def test_changed_historical_launcher_fails(self):
        self.launcher.write_text("new launcher")
        self.assertTrue(binding.validate(self.payload))

    def test_wrong_mapped_uuid_fails(self):
        self.payload["processes"][0]["image_uuid"] = "unmatched"
        self.assertTrue(binding.validate(self.payload))

    def test_recycled_pid_start_fails(self):
        self.processes[binding.LABELS[0]]["started_at"] = "recycled"
        self.assertTrue(binding.validate(self.payload))

    def test_wrong_gateway_port_owner_fails(self):
        with patch.object(binding, "port_owner", return_value=100):
            self.assertTrue(binding.validate(self.payload))

    def test_missing_or_duplicate_process_fails(self):
        self.payload["processes"][2] = copy.deepcopy(self.payload["processes"][0])
        self.assertTrue(binding.validate(self.payload))

    def test_changed_configuration_fails(self):
        with patch.object(binding, "topology", return_value={}):
            self.assertTrue(binding.validate(self.payload))

    def test_scope_and_freshness_fail_closed(self):
        for age in (-1, 181):
            self.assertTrue(binding.validate(self.payload, now=self.payload["captured_at_unix_s"] + age))
        self.payload["memory_bytes_attested"] = True
        self.assertTrue(binding.validate(self.payload))

    def test_protocol_cannot_be_invented(self):
        self.payload["protocol"]["revision"] = "different"
        self.assertTrue(binding.validate(self.payload))

    def test_no_retained_artifact_is_not_current_disk_permission(self):
        self.retained.unlink()
        with self.assertRaisesRegex(ValueError, "no retained"):
            binding.retained_binary(self.minime, "0" * 64)

    def test_sample_parser_rejects_absent_or_ambiguous_image(self):
        with self.assertRaises(ValueError):
            binding.image_uuid("no image", sampled=True)
        sample = self.fake_run(["sample"])
        with self.assertRaises(ValueError):
            binding.image_uuid(sample + sample, sampled=True)

    def test_environment_receipt_checks_live_binding_not_only_artifact_hash(self):
        entry = {"path": str(self.path), "manifest": self.payload}
        self.assertTrue(manifest_compatibility(entry, pinned_revision="fixture-rev")[0])
        self.processes[binding.LABELS[1]]["pid"] = 900
        ok, reasons = manifest_compatibility(entry, pinned_revision="fixture-rev")
        self.assertFalse(ok)
        self.assertTrue(any("process identity changed" in reason for reason in reasons))

    def test_capture_refuses_mapped_mismatch_before_writing(self):
        def wrong_sample(args, timeout=20):
            result = self.fake_run(args)
            return result.replace(self.uuid, "00000000-0000-0000-0000-000000000000") if args[0] == "sample" else result
        before = set(self.manifests.iterdir())
        with patch.object(binding, "run", side_effect=wrong_sample):
            with self.assertRaisesRegex(ValueError, "mapped build mismatch"):
                binding.capture(self.minime, self.workspace, self.root / "installed")
        self.assertEqual(set(self.manifests.iterdir()), before)

    def test_topology_requires_loaded_division_and_matching_installed_plists(self):
        installed = self.root / "installed"
        installed.mkdir()
        (self.minime / "launchd").mkdir()
        runtime = self.minime / "workspace/division/runtime-manifest.json"
        runtime.parent.mkdir(parents=True)
        runtime.write_text(json.dumps({"mode": "dormant"}))
        for label in binding.LABELS:
            config = {"Label": label, "ProgramArguments": ["/bin/bash", str(self.launcher)]}
            if label == binding.LABELS[0]:
                config["EnvironmentVariables"] = {"MINIME_DIVISION_GATEWAY_ENABLED": "true", "MINIME_DIVISION_RUNTIME_MANIFEST": str(runtime)}
            data = plistlib.dumps(config)
            (self.minime / "launchd" / f"{label}.plist").write_bytes(data)
            (installed / f"{label}.plist").write_bytes(data)
        loaded = f"{self.launcher}\n MINIME_DIVISION_GATEWAY_ENABLED => true\n"
        with patch.object(binding, "run", return_value=loaded):
            self.assertEqual(len(REAL_TOPOLOGY(self.minime, installed)), 10)
            runtime.write_text(json.dumps({"mode": "active"}))
            with self.assertRaisesRegex(ValueError, "active Division"):
                REAL_TOPOLOGY(self.minime, installed)
            runtime.write_text(json.dumps({"mode": "dormant"}))
        with patch.object(binding, "run", return_value=str(self.launcher)):
            with self.assertRaisesRegex(ValueError, "already-enabled"):
                REAL_TOPOLOGY(self.minime, installed)
        (installed / f"{binding.LABELS[0]}.plist").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "configuration differs"):
            REAL_TOPOLOGY(self.minime, installed)


if __name__ == "__main__":
    unittest.main()
