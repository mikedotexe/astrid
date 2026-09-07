"""Selected launcher verification uses private synthetic artifacts only."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import bridge_release_launch as launch


class ReleaseLaunchTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="bridge-launch-test-")
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name).resolve()
        self.source = self.root / "source"
        (self.source / "capsules/spectral-bridge/src").mkdir(parents=True)
        self.stage = self.root / "stage"
        (self.stage / "helpers").mkdir(parents=True)
        artifacts = {}
        for name, relative in {"spectral-bridge":"spectral-bridge-server",
                               "substrate-probe-v2":"helpers/substrate_probe_v2.py",
                               "release-launcher":"helpers/launchd_spectral_bridge.sh",
                               "release-selection":"helpers/bridge_release_launch.py"}.items():
            path = self.stage / relative
            path.write_bytes(b"fixture")
            artifacts[name] = {"path":str(path), "sha256":launch.digest(path)}
        self.manifest = self.stage / "manifest.json"
        self.manifest.write_text(json.dumps({"repository":{"path":str(self.source)}, "artifacts":artifacts}))
        self.control = self.root / ".runtime/bridge-deployment"
        self.control.mkdir(parents=True)
        (self.control / "active.json").write_text(json.dumps({"schema":"bridge_release_selection_v1",
            "stage":str(self.stage), "manifest_sha256":launch.digest(self.manifest)}))

    def command(self):
        return launch.selected_command(self.root, verify_native=False)

    def test_explicit_source_and_state_paths_do_not_follow_build_location(self):
        command = self.command()
        def value(flag):
            return command[command.index(flag) + 1]
        self.assertEqual(value("--bridge-root"), str(self.source / "capsules/spectral-bridge"))
        self.assertEqual(value("--bridge-workspace"), str(self.root / "capsules/spectral-bridge/workspace"))
        self.assertEqual(value("--astrid-root"), str(self.root))
        self.assertEqual(value("--db-path"), str(self.root / "capsules/spectral-bridge/workspace/bridge.db"))
        self.assertEqual(command[-1], "--autonomous")

    def test_hold_blocks_before_reading_selection_including_dangling_symlink(self):
        (self.control / "active.json").unlink()
        hold = self.control / "hold.json"
        hold.symlink_to(self.root / "missing")
        with self.assertRaisesRegex(ValueError, "operator deployment hold"):
            self.command()

    def test_invalid_selection_does_not_fall_back_to_old_binary(self):
        (self.control / "active.json").write_text("{}")
        with self.assertRaisesRegex(ValueError, "unsupported release selection"):
            self.command()

    def test_artifact_change_or_symlink_is_refused(self):
        helper = self.stage / "helpers/bridge_release_launch.py"
        helper.write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "selected artifact changed"):
            self.command()
        helper.unlink()
        helper.symlink_to(self.stage / "helpers/substrate_probe_v2.py")
        with self.assertRaisesRegex(ValueError, "selected artifact changed"):
            self.command()

    def test_manifest_change_is_refused(self):
        self.manifest.write_text("{}")
        with self.assertRaisesRegex(ValueError, "selected manifest changed"):
            self.command()

    def test_missing_source_is_refused(self):
        (self.source / "capsules/spectral-bridge/src").rmdir()
        with self.assertRaisesRegex(ValueError, "source tree is unavailable"):
            self.command()

    def test_native_verification_is_required_and_must_match(self):
        with patch.object(launch.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, '{"verified":false}')):
            with self.assertRaisesRegex(ValueError, "native selected manifest verification failed"):
                launch.selected_command(self.root)

    def test_hold_appearing_during_verification_is_refused(self):
        def verify(args, **kwargs):
            self.assertEqual(args[-1], "--verify-deployment-manifest")
            (self.control / "hold.json").write_text("{}")
            return subprocess.CompletedProcess(args, 0, json.dumps({"verified":True, "manifest_sha256":launch.digest(self.manifest)}))
        with patch.object(launch.subprocess, "run", side_effect=verify):
            with self.assertRaisesRegex(ValueError, "hold appeared"):
                launch.selected_command(self.root)


if __name__ == "__main__":
    unittest.main()
