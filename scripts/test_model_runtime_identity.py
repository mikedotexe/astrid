"""Runtime receipt identity tests; no production processes or MLX imports."""
from __future__ import annotations

import json
from pathlib import Path
import plistlib
import tempfile
import unittest
from unittest.mock import patch

import model_environment as environments
import model_runtime_identity as identity


class RuntimeIdentityTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.runtime = self.root / "runtime"
        self.runtime.mkdir()
        self.context_path = self.root / "context.json"
        self.plist = self.root / "provider.plist"
        self.source_names = {"server": "coupled_astrid_server.py", "gateway": "coupled_http_gateway.py",
                             "logit-processor": "mlx_reservoir.py", "generation-controls": "generation_controls.py"}

    def write_context(self, artifacts):
        context = {"schema": "stack_build_manifest_v1", "component": "coupled-model",
                   "artifacts": {name: {"path": str(path), "sha256": environments.digest(path)}
                                 for name, path in artifacts.items()}}
        self.context_path.write_text(json.dumps(context))
        return context

    def setup_release(self, *, legacy=False):
        source = self.runtime if legacy else self.root / "release/source"
        source.mkdir(parents=True, exist_ok=True)
        python = self.runtime / ".venv/bin/python" if legacy else self.root / "release/.venv/bin/python"
        python.parent.mkdir(parents=True)
        python.write_bytes(b"fixture interpreter never executed")
        artifacts = {name: source / filename for name, filename in self.source_names.items()}
        for name, path in artifacts.items():
            path.write_text(f"# fixture {name}\n")
        command = [str(python), str(artifacts["server"]), "--port", "8090"]
        self.plist.write_bytes(plistlib.dumps({"Label": "com.reservoir.coupled-astrid",
                                            "WorkingDirectory": str(self.runtime), "ProgramArguments": command}))
        artifacts.update(python=python, **{"launchd-plist": self.plist})
        release = {"manifest": {"python": str(python), "source_root": str(source),
                                "working_directory": str(self.runtime)},
                   "command": command, "release_id": "fixture", "manifest_sha256": "release-hash"}
        if not legacy:
            manifest, selection, launcher = (self.root / name for name in ("manifest.json", "active.json", "launcher.py"))
            manifest.write_text("{}")
            selection.write_text(json.dumps({"schema": environments.SELECTION_SCHEMA, "manifest": str(manifest),
                                             "manifest_sha256": "release-hash", "release_id": "fixture"}))
            launcher.write_text("# fixture launcher\n")
            artifacts.update({"environment-manifest": manifest, "environment-selection": selection,
                              "environment-launcher": launcher})
        self.write_context(artifacts)
        return artifacts, release, python

    def resolve(self, command):
        return identity.resolve_interpreter(self.context_path, command, model_root=self.runtime)

    def test_selected_environment_names_its_own_interpreter_not_shared_venv(self):
        artifacts, release, python = self.setup_release()
        with patch.object(environments, "validate_manifest", return_value=release) as validate:
            self.assertEqual(self.resolve(release["command"]), python)
            validate.assert_called_once()
        self.assertNotEqual(python, self.runtime / ".venv/bin/python")
        with patch.object(environments, "validate_manifest", return_value=release):
            runtime = identity.resolve_runtime(self.context_path, release["command"], model_root=self.runtime)
        self.assertEqual(runtime["wrapper"], str(artifacts["environment-launcher"]))

    def test_new_environment_requires_exact_running_command(self):
        _, release, _ = self.setup_release()
        with patch.object(environments, "validate_manifest", return_value=release):
            with self.assertRaisesRegex(ValueError, "argv"):
                self.resolve(release["command"] + ["--changed"])

    def test_partial_environment_context_never_falls_back(self):
        artifacts, release, _ = self.setup_release()
        del artifacts["environment-selection"]
        self.write_context(artifacts)
        with self.assertRaisesRegex(ValueError, "partial environment"):
            self.resolve(release["command"])

    def test_selection_must_match_validated_environment(self):
        artifacts, release, _ = self.setup_release()
        value = environments.read_json(artifacts["environment-selection"])
        value["manifest_sha256"] = "another"
        artifacts["environment-selection"].write_text(json.dumps(value))
        self.write_context(artifacts)
        with patch.object(environments, "validate_manifest", return_value=release):
            with self.assertRaisesRegex(ValueError, "current environment selection"):
                self.resolve(release["command"])

    def test_unchanged_context_cannot_hide_modified_package_or_source(self):
        artifacts, release, _ = self.setup_release()
        with patch.object(environments, "validate_manifest", side_effect=ValueError("installed package changed")):
            with self.assertRaisesRegex(ValueError, "installed package"):
                self.resolve(release["command"])
        artifacts["gateway"].write_text("# changed\n")
        with self.assertRaisesRegex(ValueError, "artifact changed"):
            self.resolve(release["command"])

    def test_selected_context_cannot_retain_original_source_paths(self):
        artifacts, release, _ = self.setup_release()
        wrong = self.runtime / "coupled_http_gateway.py"
        wrong.write_text("# wrong legacy source\n")
        artifacts["gateway"] = wrong
        self.write_context(artifacts)
        with patch.object(environments, "validate_manifest", return_value=release):
            with self.assertRaisesRegex(ValueError, "sources/interpreter differ"):
                self.resolve(release["command"])

    def test_legacy_fallback_requires_original_source_plist_and_exact_argv(self):
        _, release, python = self.setup_release(legacy=True)
        with patch.object(environments, "validate_manifest") as validate:
            self.assertEqual(self.resolve(release["command"]), python)
            validate.assert_not_called()
        with self.assertRaisesRegex(ValueError, "argv"):
            self.resolve([str(self.root / "another/python"), *release["command"][1:]])
        runtime = identity.resolve_runtime(self.context_path, release["command"], model_root=self.runtime)
        self.assertEqual(runtime["wrapper"], str(Path(identity.__file__).resolve().with_name("restart_coupled_model.sh")))

    def test_legacy_context_cannot_identify_versioned_source(self):
        artifacts, release, _ = self.setup_release()
        for key in identity.ENVIRONMENT_KEYS:
            del artifacts[key]
        self.write_context(artifacts)
        with self.assertRaisesRegex(ValueError, "original direct"):
            self.resolve(release["command"])


if __name__ == "__main__":
    unittest.main()
