"""Exercise the real shell wrapper with isolated paths and inert launch tools."""
from pathlib import Path
import json
import os
import subprocess
import tempfile
import unittest


class ProviderObservationLaunchTests(unittest.TestCase):
    def run_wrapper(self, *, durable=None, override=None, hold=False):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bridge = root / "capsules/spectral-bridge"
            runtime = bridge / "workspace/runtime"
            runtime.mkdir(parents=True)
            control = root / ".runtime/bridge-deployment"
            control.mkdir(parents=True)
            (control / "active.json").write_text("{}")
            if hold:
                (control / "hold.json").write_text("{}")
            if durable is not None:
                (runtime / "provider_observation.env").write_text(
                    f"export ASTRID_PROVIDER_OBSERVATION={durable}\n"
                    "export ASTRID_PROVIDER_OBSERVATION_DIR='/private/study spool'\n"
                )
            launchctl = root / "launchctl"
            launchctl.write_text(
                '#!/bin/bash\n'
                'if [ "$1" = getenv ] && [ "$2" = ASTRID_PROVIDER_OBSERVATION ]; then\n'
                '  printf "%s" "${TEST_OBSERVER_OVERRIDE:-}"\n'
                'fi\n'
            )
            launchctl.chmod(0o700)
            capture = root / "capture.py"
            capture.write_text(
                "import json,os\n"
                "print(json.dumps({key:os.environ.get(key) for key in "
                "['ASTRID_PROVIDER_OBSERVATION','ASTRID_PROVIDER_OBSERVATION_DIR',"
                "'ASTRID_GENERATION_RECORD']}))\n"
            )
            script = Path(__file__).with_name("launchd_spectral_bridge.sh").read_text()
            script = script.replace("/Users/v/other/astrid", str(root))
            script = script.replace("/bin/launchctl", str(launchctl))
            script = script.replace(str(root / "scripts/bridge_release_launch.py"), str(capture))
            wrapper = root / "wrapper.sh"
            wrapper.write_text(script)
            env = {"PATH": os.environ["PATH"], "ASTRID_GENERATION_RECORD": "on"}
            if override is not None:
                env["TEST_OBSERVER_OVERRIDE"] = override
            return subprocess.run(["/bin/bash", str(wrapper)], env=env,
                                  text=True, capture_output=True, timeout=10)

    def test_absent_config_preserves_default_off(self):
        result = self.run_wrapper()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), {
            "ASTRID_PROVIDER_OBSERVATION": None,
            "ASTRID_PROVIDER_OBSERVATION_DIR": None,
            "ASTRID_GENERATION_RECORD": "on",
        })

    def test_durable_enablement_reaches_selected_release(self):
        result = self.run_wrapper(durable="on")
        self.assertEqual(result.returncode, 0, result.stderr)
        actual = json.loads(result.stdout)
        self.assertEqual(actual["ASTRID_PROVIDER_OBSERVATION"], "on")
        self.assertEqual(actual["ASTRID_PROVIDER_OBSERVATION_DIR"], "/private/study spool")
        self.assertEqual(actual["ASTRID_GENERATION_RECORD"], "on")

    def test_live_disable_override_precedes_durable_enablement(self):
        result = self.run_wrapper(durable="on", override="off")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["ASTRID_PROVIDER_OBSERVATION"], "off")

    def test_deployment_hold_prevents_startup(self):
        result = self.run_wrapper(durable="on", hold=True)
        self.assertEqual(result.returncode, 73)
        self.assertEqual(result.stdout, "")


if __name__ == "__main__":
    unittest.main()
