#!/usr/bin/env python3
"""Static and syntax tests for stack deployment wrappers."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = (
    ROOT / "scripts/build_bridge.sh",
    ROOT / "scripts/deploy_minime.sh",
    ROOT / "scripts/restart_coupled_model.sh",
    ROOT / "scripts/capture_stack_receipt.sh",
    ROOT / "scripts/start_all.sh",
)


class DeploymentWrapperTests(unittest.TestCase):
    def test_shell_syntax(self) -> None:
        result = subprocess.run(
            ["bash", "-n", *(str(path) for path in SCRIPTS)],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_actor_default_is_neutral(self) -> None:
        for path in SCRIPTS[:4]:
            text = path.read_text()
            self.assertIn("ASTRID_DEPLOY_ACTOR:-interactive-agent", text)
            self.assertNotIn('SOURCE="claude"', text)

    def test_model_startup_waits_for_same_port_readiness(self) -> None:
        text = (ROOT / "scripts/start_all.sh").read_text()
        self.assertIn("wait_http_ready", text)
        self.assertIn("http://127.0.0.1:8090/readyz", text)
        self.assertNotIn('wait_port 8090 "coupled Astrid server"', text)

    def test_wrappers_emit_checked_receipts_and_manifests(self) -> None:
        for path in SCRIPTS[:3]:
            text = path.read_text()
            self.assertIn("record-deploy", text)
            self.assertIn("environment_receipts.py", text)
            self.assertIn("--manifest", text)
        stack = SCRIPTS[3].read_text()
        self.assertIn("coupled-stack", stack)
        self.assertIn("/readyz", stack)
        self.assertIn("--process", stack)
        self.assertIn('--context-manifest "$MODEL_MANIFEST"', stack)

    def test_help_paths_are_side_effect_free(self) -> None:
        for path in SCRIPTS[:4]:
            result = subprocess.run(
                [str(path), "--help"],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, f"{path}: {result.stderr}")
            self.assertIn("usage:", result.stdout)


if __name__ == "__main__":
    unittest.main()
