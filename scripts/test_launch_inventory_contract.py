"""The copies of the Minime launch-input contract must agree (2026-10-01).

Three places describe which files make up Minime's launch identity: the agent's
own ``minime_autonomy.deployment.source_inputs`` (what the running process records),
``qualify_geometry_release.inventory`` (what reconciliation/qualification compares
against) and ``deploy_preflight`` (which dirty paths block a deploy). A stale copy in
the qualification tooling rejected a healthy running agent after
``launchd/autonomous-agent.env`` was added. Read-only; no live state is touched.
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import deploy_preflight  # noqa: E402
import qualify_geometry_release as qualify  # noqa: E402

MINIME = Path("/Users/v/other/minime")


class LaunchInventoryContractTests(unittest.TestCase):
    def test_preflight_dirty_list_names_every_explicit_launch_input(self):
        build_paths = deploy_preflight.COMPONENTS["minime-agent"]["build_paths"]
        for name in (*qualify.LAUNCH_INPUTS_REQUIRED, *qualify.LAUNCH_INPUTS_OPTIONAL):
            self.assertIn(name, build_paths, name)

    @unittest.skipUnless((MINIME / "minime_autonomy/deployment.py").is_file(), "minime checkout absent")
    def test_qualification_inventory_matches_the_runtime_inventory(self):
        sys.path.insert(0, str(MINIME))
        from minime_autonomy.deployment import source_inputs  # noqa: E402
        runtime = source_inputs(MINIME)
        qualification = qualify.inventory(MINIME)
        self.assertEqual(set(qualification), set(runtime))
        self.assertEqual(qualification, runtime)
        self.assertIn("launchd/autonomous-agent.env", runtime)


if __name__ == "__main__":
    unittest.main()
