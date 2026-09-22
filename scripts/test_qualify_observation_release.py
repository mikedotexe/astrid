"""Frozen adapter inventory includes its existing non-code registry asset."""
import stat
import tempfile
import unittest
from pathlib import Path

from qualify_observation_release import freeze_adapter
from reconcile_minime_launch import ASSETS


class ObservationReleaseTests(unittest.TestCase):
    def fixture(self, root):
        source = root / "source"
        for name in ("autonomous_agent.py", "minime_autonomy/runtime.py",
                     "scripts/launchd_autonomous_agent.sh",
                     "scripts/minime_rescue_investigation.py",
                     "launchd/com.minime.autonomous-agent.plist", *ASSETS):
            path = source / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("{}" if name.endswith(".json") else "synthetic input\n")
        return source

    def test_registry_and_python_are_frozen_and_hash_bound(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            source = self.fixture(root)
            target = root / "frozen"
            inputs, assets = freeze_adapter(source, target)
            self.assertIn("minime_autonomy/runtime.py", inputs)
            self.assertEqual(set(assets), set(ASSETS))
            for name in (*inputs, *assets):
                self.assertEqual((source / name).read_bytes(), (target / name).read_bytes())
                self.assertEqual(stat.S_IMODE((target / name).stat().st_mode), 0o444)
            self.assertEqual(stat.S_IMODE(target.stat().st_mode), 0o555)

    def test_registry_symlink_is_not_followed_into_snapshot(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            source = self.fixture(root)
            registry = source / ASSETS[0]
            registry.unlink()
            unrelated = root / "unrelated.json"
            unrelated.write_text("private unrelated fixture")
            registry.symlink_to(unrelated)
            with self.assertRaisesRegex(ValueError, "symlink"):
                freeze_adapter(source, root / "frozen")
            self.assertFalse((root / "frozen" / ASSETS[0]).exists())

    def test_missing_required_asset_fails_before_snapshot_creation(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            source = self.fixture(root)
            (source / ASSETS[0]).unlink()
            with self.assertRaises(FileNotFoundError):
                freeze_adapter(source, root / "frozen")
            self.assertFalse((root / "frozen").exists())


if __name__ == "__main__":
    unittest.main()
