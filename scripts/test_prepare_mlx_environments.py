"""Qualification inventory and wheel-lock integrity checks."""
import hashlib
from pathlib import Path
import tempfile
import unittest

from prepare_mlx_environments import EXPECTED, inventories, lock_text, wheel_index


class EnvironmentPreparationTests(unittest.TestCase):
    def test_only_reviewed_packages_change(self):
        baseline = {"python": "3.12.13", "packages": EXPECTED | {"numpy": "2.4.3"}}
        stages = inventories(baseline)
        self.assertEqual(stages["baseline"], baseline["packages"])
        self.assertEqual({k for k, v in stages["core"].items()
                          if v != baseline["packages"][k]}, {"mlx", "mlx-metal"})
        self.assertEqual({k for k, v in stages["full"].items()
                          if v != baseline["packages"][k]}, set(EXPECTED))

    def test_changed_baseline_is_rejected(self):
        for baseline in ({"python": "3.14.0", "packages": EXPECTED},
                         {"python": "3.12.13", "packages": EXPECTED | {"mlx": "0.32.3"}}):
            with self.assertRaises(ValueError):
                inventories(baseline)

    def test_lock_binds_selected_wheel_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            wheel = root / "mlx_lm-0.32.0-py3-none-any.whl"
            wheel.write_bytes(b"qualification wheel fixture")
            lock = lock_text({"mlx-lm": "0.32.0"}, wheel_index(root))
            self.assertEqual(lock, "mlx-lm==0.32.0 --hash=sha256:" +
                             hashlib.sha256(wheel.read_bytes()).hexdigest() + "\n")
            with self.assertRaises(KeyError):
                lock_text({"mlx-lm": "0.31.3"}, wheel_index(root))

    def test_ambiguous_platform_wheels_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for tag in ("macosx_14_0_arm64", "macosx_26_0_arm64"):
                (root / f"mlx-0.32.4-cp312-cp312-{tag}.whl").write_bytes(tag.encode())
            with self.assertRaises(ValueError):
                wheel_index(root)


if __name__ == "__main__":
    unittest.main()
