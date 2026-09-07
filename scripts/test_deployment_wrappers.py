#!/usr/bin/env python3
"""Static and syntax tests for stack deployment wrappers."""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = (
    ROOT / "scripts/build_bridge.sh",
    ROOT / "scripts/deploy_minime.sh",
    ROOT / "scripts/deploy_division_runtime.sh",
    ROOT / "scripts/restart_coupled_model.sh",
    ROOT / "scripts/capture_stack_receipt.sh",
    ROOT / "scripts/start_all.sh",
)


class DeploymentWrapperTests(unittest.TestCase):
    def activation_run(self, arguments: list[str], *, fail_preflight_repo: str = ""):
        """Exercise the system Bash without invoking deployment Python or services."""
        with tempfile.TemporaryDirectory(prefix="bridge-wrapper-") as temporary:
            fixture = Path(temporary)
            source = fixture / "source with spaces"
            scripts = source / "scripts"
            scripts.mkdir(parents=True)
            wrapper = scripts / "build_bridge.sh"
            shutil.copyfile(ROOT / "scripts/build_bridge.sh", wrapper)
            commands = fixture / "commands"
            commands.mkdir()
            calls_path = fixture / "calls.jsonl"
            mock = commands / "python3"
            mock.write_text(
                f"#!{sys.executable}\n"
                "import json, os, pathlib, sys\n"
                "args = sys.argv[1:]\n"
                "with open(os.environ['MOCK_CALLS'], 'a') as output:\n"
                "    output.write(json.dumps({'argv': args, 'sanctioned': os.environ.get('ASTRID_SANCTIONED_BRIDGE_ACTIVATION')}) + '\\n')\n"
                "if len(args) < 2 or args[0] != '-B': sys.exit(97)\n"
                "name = pathlib.Path(args[1]).name\n"
                "if name not in {'deploy_preflight.py', 'bridge_activate.py'}: sys.exit(98)\n"
                "if name == 'deploy_preflight.py' and args[args.index('--repo') + 1] == os.environ.get('MOCK_FAIL_PREFLIGHT_REPO'): sys.exit(23)\n"
            )
            mock.chmod(0o755)
            env = {
                "PATH": f"{commands}:/usr/bin:/bin",
                "MOCK_CALLS": str(calls_path),
                "MOCK_FAIL_PREFLIGHT_REPO": fail_preflight_repo,
            }
            result = subprocess.run(
                ["/bin/bash", str(wrapper), *arguments], env=env,
                capture_output=True, text=True, check=False,
            )
            calls = [json.loads(line) for line in calls_path.read_text().splitlines()] if calls_path.exists() else []
            return result, calls, str(source)

    def test_system_bash_dispatches_activation_with_and_without_recovery(self) -> None:
        for recovery in ([], ["--resume-verification", "/fixture/transaction with spaces"],
                         ["--resume-stopped-transition", "/fixture/transaction with spaces"]):
            with self.subTest(recovery=recovery):
                result, calls, source = self.activation_run([
                    "--activate-stage", "/fixture/stage with spaces", "--expected-pid", "12345",
                    "--actor", "wrapper-test", "--ack", "synthetic rollout", *recovery,
                ])
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(len(calls), 3, calls)
                for call, repository in zip(calls[:2], (source, "/Users/v/other/astrid")):
                    self.assertEqual(call, {
                        "argv": ["-B", f"{source}/scripts/deploy_preflight.py", "--repo", repository, "--ack", "synthetic rollout"],
                        "sanctioned": None,
                    })
                self.assertEqual(calls[2], {
                    "argv": ["-B", f"{source}/scripts/bridge_activate.py", "--stage-dir", "/fixture/stage with spaces",
                             "--expected-pid", "12345", "--actor", "wrapper-test", "--ack", "synthetic rollout",
                             "--legacy-stop-ack", "", "--timeout-secs", "600", *recovery],
                    "sanctioned": "1",
                })

    def test_system_bash_rejects_malformed_recovery_before_preflight(self) -> None:
        base = ["--activate-stage", "/fixture/stage", "--expected-pid", "12345", "--ack", "synthetic rollout"]
        for arguments in (
            [*base, "--resume-verification"],
            [*base, "--resume-verification", "--timeout-secs", "10"],
            ["--resume-verification", "/fixture/transaction"],
            [*base, "--resume-verification", "/fixture/transaction", "--legacy-stop-ack", "legacy"],
            [*base, "--resume-stopped-transition"],
            [*base, "--resume-stopped-transition", "--actor", "fixture"],
            [*base, "--resume-stopped-transition", "/fixture/transaction", "--legacy-stop-ack", "legacy"],
            [*base, "--resume-stopped-transition", "/fixture/transaction", "--resume-verification", "/fixture/transaction"],
            ["--resume-stopped-transition", "/fixture/transaction"],
        ):
            with self.subTest(arguments=arguments):
                result, calls, _ = self.activation_run(arguments)
                self.assertEqual(result.returncode, 64, result.stderr)
                self.assertEqual(calls, [])

    def test_system_bash_stops_when_canonical_preflight_fails(self) -> None:
        result, calls, _ = self.activation_run([
            "--activate-stage", "/fixture/stage", "--expected-pid", "12345", "--ack", "synthetic rollout",
        ], fail_preflight_repo="/Users/v/other/astrid")
        self.assertEqual(result.returncode, 23, result.stderr)
        self.assertEqual(len(calls), 2, calls)
        self.assertTrue(all(Path(call["argv"][1]).name == "deploy_preflight.py" for call in calls))

    def test_activation_is_separate_and_requires_both_preflights(self) -> None:
        wrapper = ROOT / "scripts/build_bridge.sh"
        text = wrapper.read_text()
        block = text.split('if [ -n "$ACTIVATE_STAGE" ]; then', 1)[1].split('\nfi', 1)[0]
        self.assertIn('--repo "$SCRIPT_ROOT"', block)
        self.assertIn('--repo "$ASTRID"', block)
        self.assertNotIn('--window-s', block)
        self.assertLess(block.index('deploy_preflight.py'), block.index('ASTRID_SANCTIONED_BRIDGE_ACTIVATION=1'))
        for extra in (["--restart"], ["--drain-only"], ["--no-build"], ["--stage-dir", "/unused"],
                      ["--promote-candidate", "fixture"]):
            result = subprocess.run(["bash", str(wrapper), "--activate-stage", "/unused", "--expected-pid", "12345", "--ack", "fixture", *extra], capture_output=True, text=True)
            self.assertEqual(result.returncode, 64, result.stderr)
        for extra in (["--legacy-stop-ack", "fixture"], ["--expected-pid", "12345"]):
            result = subprocess.run(["bash", str(wrapper), *extra], capture_output=True, text=True)
            self.assertEqual(result.returncode, 64, result.stderr)

    def test_stack_capture_names_actual_bridge_executable(self) -> None:
        text = (ROOT / "scripts/capture_stack_receipt.sh").read_text()
        self.assertIn('ps -p "$BRIDGE_PID" -o comm=', text)
        self.assertIn('--binary "spectral-bridge=$BRIDGE_BINARY"', text)
        self.assertNotIn('--binary "spectral-bridge=$ASTRID/capsules/spectral-bridge/target', text)

    def test_staging_is_explicit_and_cannot_restart_or_skip_preflight(self) -> None:
        wrapper = ROOT / "scripts/build_bridge.sh"
        text = wrapper.read_text()
        block = text.split('if [ -n "$STAGE_DIR" ]; then', 1)[1].split('\nfi', 1)[0]
        self.assertLess(block.index('deploy_preflight.py'), block.index('ASTRID_SANCTIONED_BRIDGE_STAGE=1'))
        self.assertNotIn('--window-s', block)
        for arguments in (["--restart"], ["--drain-only"], ["--no-build"], ["--promote-candidate", "fixture"]):
            result = subprocess.run(["bash", str(wrapper), "--stage-dir", "/unused", "--ack", "fixture", *arguments], capture_output=True, text=True)
            self.assertEqual(result.returncode, 64, result.stderr)
        result = subprocess.run(["bash", str(wrapper), "--stage-dir", "/unused"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 64)

    def test_drain_only_is_separate_and_legacy_restart_is_blocked(self) -> None:
        text = (ROOT / "scripts/build_bridge.sh").read_text()
        self.assertIn("--drain-only", text)
        self.assertIn('exec python3 -B "$SCRIPT_ROOT/scripts/bridge_drain.py"', text)
        self.assertLess(text.index('legacy forced replacement is disabled'), text.index('# 1. Preflight gate.'))
        result = subprocess.run(["bash", str(ROOT / "scripts/build_bridge.sh"), "--drain-only", "--restart"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 64)

    def test_shell_syntax(self) -> None:
        result = subprocess.run(
            ["bash", "-n", *(str(path) for path in SCRIPTS)],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_actor_default_is_neutral(self) -> None:
        for path in SCRIPTS[:5]:
            text = path.read_text()
            self.assertIn("ASTRID_DEPLOY_ACTOR:-interactive-agent", text)
            self.assertNotIn('SOURCE="claude"', text)

    def test_model_startup_waits_for_same_port_readiness(self) -> None:
        text = (ROOT / "scripts/start_all.sh").read_text()
        self.assertIn("wait_http_ready", text)
        self.assertIn("http://127.0.0.1:8090/readyz", text)
        self.assertNotIn('wait_port 8090 "coupled Astrid server"', text)

    def test_model_reload_drains_without_forced_bootout(self) -> None:
        text = (ROOT / "scripts/restart_coupled_model.sh").read_text()
        self.assertIn("graceful_model_reload.py", text)
        self.assertNotIn("launchctl bootout", text)
        self.assertNotIn("launchctl kickstart", text)
        self.assertIn('cmp -s "$PLIST" "$INSTALLED_PLIST"', text)
        self.assertIn("-iTCP:8090", text)
        self.assertIn('logit-processor=$PROCESSOR', text)
        self.assertLess(text.index('bash "$ASTRID/scripts/capture_stack_receipt.sh"'), text.index('if ! python3 "$ASTRID/scripts/graceful_model_reload.py"'))
        self.assertEqual(text.count('bash "$ASTRID/scripts/capture_stack_receipt.sh"'), 2)
        self.assertIn('--output "$CANDIDATE_MANIFEST"', text)
        self.assertLess(text.index('[ "$READYZ_OK" = true ]'), text.index('mv "$RELOAD_DIR/publish-manifest.json" "$MANIFEST"'))
        self.assertIn('--old-started-at "$OLD_STARTED_AT"', text)

    def test_stack_capture_checks_division_runtime_binding(self) -> None:
        text = (ROOT / "scripts/capture_stack_receipt.sh").read_text()
        self.assertIn("minime_runtime_binding.py", text)
        self.assertIn('PORT_OWNER_PID="$GATEWAY_PID"', text)
        self.assertIn('--process "minime-gateway=$GATEWAY_PID"', text)
        self.assertIn('--process "minime-supervisor=$SUPERVISOR_PID"', text)

    def test_wrappers_emit_checked_receipts_and_manifests(self) -> None:
        for path in SCRIPTS[:4]:
            text = path.read_text()
            self.assertIn("record-deploy", text)
            self.assertIn("environment_receipts.py", text)
            self.assertIn("--manifest", text)
        stack = SCRIPTS[4].read_text()
        self.assertIn("coupled-stack", stack)
        self.assertIn("/readyz", stack)
        self.assertIn("--process", stack)
        self.assertIn('--context-manifest "$MODEL_MANIFEST"', stack)

    def test_help_paths_are_side_effect_free(self) -> None:
        for path in SCRIPTS[:5]:
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
