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

    def test_division_deploy_fail_closes_dormant_daughter_labels(self) -> None:
        text = (ROOT / "scripts/deploy_division_runtime.sh").read_text()
        self.assertIn("CHILD_LABELS=(", text)
        self.assertIn('unload_child_label "$label"', text)
        self.assertIn('launchctl print "$DOMAIN/$label"', text)
        self.assertIn("dormant daughter label remained loaded", text)
        self.assertIn('daughter_minime_unloaded=true', text)
        self.assertIn('daughter_astrid_unloaded=true', text)

    def test_self_change_promotion_is_verified_and_rollbackable(self) -> None:
        for name in ("build_bridge.sh", "deploy_minime.sh"):
            text = (ROOT / "scripts" / name).read_text()
            self.assertIn("--promote-candidate", text)
            self.assertIn("self_change_canary.py", text)
            self.assertIn("verify-promotion", text)
            self.assertIn("restore_candidate_binary", text)
            self.assertIn("artifact_sha256", text)

        wrapper = (ROOT / "scripts/run_self_change_canary.sh").read_text()
        self.assertIn("ASTRID_SANCTIONED_SELF_CHANGE_WRAPPER=1", wrapper)
        self.assertIn("verify-promotion", wrapper)


if __name__ == "__main__":
    unittest.main()
