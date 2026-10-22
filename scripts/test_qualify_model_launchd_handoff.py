"""CPU argument/privilege tests; never invokes sudo or launchctl.

These tests preserve the proposed script-first argument contract. Only the
owned host fixture can resolve its disagreement with the local man page.
"""
from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import qualify_model_launchd_handoff as fixture
import model_environment as environments
import model_environment_activate as activate


class DebugArgumentTests(unittest.TestCase):
    def test_program_is_not_repeated_in_script_first_tail(self):
        executable = Path("/python framework/bin/python")
        launcher = Path("/source tree/model_environment.py")
        control = Path("/runtime state/control")
        arguments = fixture.held_override_arguments(executable, launcher, control, "owned-transaction")
        self.assertEqual(arguments, ["--program", str(executable), "--", str(launcher), "launch",
                                     "--control-root", str(control), "--hold-only", "owned-transaction"])

    def test_script_first_tail_executes_as_python_script_without_model_imports(self):
        # Simulate the executable prefix observed in admin-01. This proves the
        # tail is valid Python argv, not how launchctl will supply that prefix.
        with tempfile.TemporaryDirectory(prefix="debug argv ") as temporary:
            root = Path(temporary)
            launcher = root / "argument probe.py"
            launcher.write_text("import json,sys\nprint(json.dumps({'argv':sys.argv,'mlx': 'mlx' in sys.modules}))\n")
            executable = Path(sys.executable).resolve()
            arguments = fixture.held_override_arguments(executable, launcher, root / "control state", "owned")
            tail = arguments[arguments.index("--") + 1:]
            completed = subprocess.run([str(executable), *tail], check=True, capture_output=True, text=True, timeout=10)
            observed = json.loads(completed.stdout)
            self.assertEqual(observed["argv"], [str(launcher), "launch", "--control-root",
                                                 str(root / "control state"), "--hold-only", "owned"])
            self.assertFalse(observed["mlx"])

    def test_production_override_matches_fixture_but_permanent_argv_keeps_python(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            backend = activate.LaunchdBackend(control_root=root / "control state", manifest=root / "candidate.json",
                                              baseline=root / "baseline.json", fixture_proof=root / "fixture.json",
                                              actor="fixture", ack="CPU-only test")
            backend.migrate = True
            with patch.object(environments, "assert_hold"), patch.object(activate, "run") as run:
                backend.arm_next_spawn()
            self.assertEqual(run.call_args.args[:3], ("/bin/launchctl", "debug", backend.target))
            self.assertEqual(list(run.call_args.args[3:]), fixture.held_override_arguments(
                backend.launcher_python, backend.launcher, backend.root, backend.guard["transaction_id"]))
            self.assertEqual(backend.launcher_arguments(), [str(backend.launcher_python), str(backend.launcher),
                                                            "launch", "--control-root", str(backend.root)])


class SudoDebugTests(unittest.TestCase):
    def setUp(self):
        self.target = "gui/501/" + fixture.PREFIX + "a" * 32

    def test_only_debug_is_elevated_with_inherited_terminal(self):
        with patch.object(fixture.os, "getuid", return_value=501), \
             patch.object(fixture.os, "geteuid", return_value=501), \
             patch.object(fixture.sys.stdin, "isatty", return_value=True), \
             patch.object(fixture.subprocess, "run", return_value=subprocess.CompletedProcess([], 0)) as run:
            status, output = fixture.debug_fixture(self.target, ["--program", "/bin/sleep"], sudo_debug=True)
        self.assertEqual(status, 0)
        command = run.call_args.args[0]
        self.assertEqual(command[:4], ["/usr/bin/sudo", "--", "/bin/launchctl", "debug"])
        self.assertEqual(command[4], self.target)
        self.assertEqual(run.call_args.kwargs, {"check": False, "timeout": 180})
        self.assertIn("not captured", output)

    def test_production_and_other_user_targets_are_refused_before_subprocess(self):
        for target in ("gui/501/com.reservoir.coupled-astrid", "gui/0/" + fixture.PREFIX + "a" * 32,
                       self.target + ";anything"):
            with self.subTest(target=target), patch.object(fixture.os, "getuid", return_value=501), \
                 patch.object(fixture.subprocess, "run") as run:
                with self.assertRaisesRegex(ValueError, "another service"):
                    fixture.debug_fixture(target, [], sudo_debug=True)
                run.assert_not_called()

    def test_root_python_and_password_pipes_are_refused(self):
        for effective_uid, tty in ((0, True), (501, False)):
            with self.subTest(effective_uid=effective_uid, tty=tty), \
                 patch.object(fixture.os, "getuid", return_value=501), \
                 patch.object(fixture.os, "geteuid", return_value=effective_uid), \
                 patch.object(fixture.sys.stdin, "isatty", return_value=tty), \
                 patch.object(fixture.subprocess, "run") as run:
                with self.assertRaises(ValueError):
                    fixture.debug_fixture(self.target, [], sudo_debug=True)
                run.assert_not_called()

    def test_plain_fixture_does_not_request_sudo(self):
        with patch.object(fixture.os, "getuid", return_value=501), \
             patch.object(fixture, "run", return_value=subprocess.CompletedProcess([], 1, "", "root required")) as run:
            status, output = fixture.debug_fixture(self.target, [], sudo_debug=False)
        self.assertEqual(status, 1)
        self.assertEqual(run.call_args.args[:2], ("/bin/launchctl", "debug"))
        self.assertEqual(output, "root required")


if __name__ == "__main__":
    unittest.main()
