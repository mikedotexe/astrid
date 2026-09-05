"""No live subprocesses: exercise bounded drain identity and refusal behavior."""
import plistlib
import unittest
from unittest.mock import patch

from graceful_model_reload import LaunchdModel, reload_model


class FakeModel:
    def __init__(self, *, old_exit=3, new_start=5):
        self.clock = 0
        self.old_exit = old_exit
        self.new_start = new_start
        self.signals = 0
        self.validated = False

    def validate(self):
        self.validated = True

    def pid(self):
        if self.clock < self.old_exit:
            return 100
        return 200 if self.clock >= self.new_start else None

    def identity(self, pid):
        if pid == 100 and self.clock < self.old_exit:
            return "old start"
        if pid == 200 and self.clock >= self.new_start:
            return "new start"
        return None

    def terminate(self):
        self.signals += 1

    def idle(self):
        return True

    def inputs(self):
        return {"processor": "fixture-hash"}

    def sleep(self, seconds):
        self.clock += seconds

    def reload(self, **kwargs):
        return reload_model(self, 100, now=lambda: self.clock, sleep=self.sleep, **kwargs)


class GracefulModelReloadTests(unittest.TestCase):
    def validate_config(self, *, installed=None, loaded="\targuments = {\n\t\tpython\n\t\tserver.py\n\t}\n"):
        source = {"KeepAlive": True, "ProgramArguments": ["python", "server.py"]}
        installed = source if installed is None else installed
        model = LaunchdModel()
        with patch("graceful_model_reload.Path.read_bytes", side_effect=[
            plistlib.dumps(source), plistlib.dumps(installed)
        ]), patch.object(model, "job", return_value=loaded):
            model.validate()

    def test_identical_loaded_configuration_is_accepted(self):
        self.validate_config()

    def test_changed_installed_configuration_is_refused(self):
        with self.assertRaisesRegex(RuntimeError, "identical plists"):
            self.validate_config(installed={"KeepAlive": True, "ProgramArguments": ["other"]})

    def test_missing_keepalive_is_refused(self):
        config = plistlib.dumps({"ProgramArguments": ["python", "server.py"]})
        with patch("graceful_model_reload.Path.read_bytes", return_value=config):
            with self.assertRaisesRegex(RuntimeError, "KeepAlive=true"):
                LaunchdModel().validate()

    def test_changed_or_absent_loaded_arguments_are_refused(self):
        for loaded in ("", "\targuments = {\n\t\tother\n\t}\n"):
            with self.subTest(loaded=loaded):
                with self.assertRaisesRegex(RuntimeError, "loaded launchd arguments"):
                    self.validate_config(loaded=loaded)

    def test_waits_for_actual_exit_and_replacement(self):
        model = FakeModel()
        result = model.reload()
        self.assertTrue(model.validated)
        self.assertEqual(model.signals, 1)
        self.assertEqual(model.clock, 5)
        self.assertEqual(result["old_pid"], 100)
        self.assertEqual(result["new_pid"], 200)
        self.assertTrue(result["old_process_exited"])
        self.assertFalse(result["forced_termination"])
        self.assertFalse(result["launchd_configuration_changed"])

    def test_timeout_never_escalates_signal(self):
        model = FakeModel(old_exit=100, new_start=101)
        with self.assertRaisesRegex(RuntimeError, "no forced termination"):
            model.reload(timeout_s=4)
        self.assertEqual(model.signals, 1)
        self.assertEqual(model.clock, 4)

    def test_waits_for_idle_before_signal(self):
        model = FakeModel(old_exit=5, new_start=6)
        with patch.object(model, "idle", side_effect=lambda: model.clock >= 2):
            with patch.object(model, "terminate", wraps=model.terminate) as terminate:
                result = model.reload()
        self.assertEqual(terminate.call_count, 1)
        self.assertTrue(result["idle_empty_queue_observed"])
        self.assertFalse(result["atomic_traffic_quiescence_claimed"])

    def test_no_idle_window_sends_no_signal(self):
        model = FakeModel(old_exit=100, new_start=101)
        with patch.object(model, "idle", return_value=False):
            with self.assertRaisesRegex(RuntimeError, "no signal sent"):
                model.reload(timeout_s=4)
        self.assertEqual(model.signals, 0)

    def test_changed_source_during_idle_wait_sends_no_signal(self):
        model = FakeModel()
        with patch.object(model, "inputs", side_effect=[{"processor": "old"}, {"processor": "new"}]):
            with self.assertRaisesRegex(RuntimeError, "source changed"):
                model.reload()
        self.assertEqual(model.signals, 0)

    def test_idle_readiness_requires_empty_queue_and_connected_reservoir(self):
        healthy = {"ready": True, "worker": {"phase": "ready", "queue_depth": 0}, "reservoir": {"status": "connected"}}
        for health, expected in (
            (healthy, True),
            ({**healthy, "worker": {"phase": "generating", "queue_depth": 0}}, False),
            ({**healthy, "worker": {"phase": "ready", "queue_depth": 1}}, False),
            ({**healthy, "reservoir": {"status": "degraded"}}, False),
        ):
            with patch("graceful_model_reload.urllib.request.urlopen"), patch("graceful_model_reload.json.load", return_value=health):
                self.assertEqual(LaunchdModel().idle(), expected)

    def test_missing_replacement_is_failure(self):
        model = FakeModel(old_exit=1, new_start=100)
        with self.assertRaisesRegex(RuntimeError, "timed out"):
            model.reload(timeout_s=4)
        self.assertEqual(model.signals, 1)

    def test_changed_identity_is_not_signaled(self):
        model = FakeModel()
        with patch.object(model, "pid", return_value=999):
            with self.assertRaisesRegex(RuntimeError, "identity changed"):
                model.reload()
        self.assertEqual(model.signals, 0)

    def test_identity_change_at_boundary_is_not_signaled(self):
        model = FakeModel()
        with patch.object(model, "identity", side_effect=["old start", "replacement start"]):
            with self.assertRaisesRegex(RuntimeError, "signal boundary"):
                model.reload()
        self.assertEqual(model.signals, 0)

    def test_configuration_failure_is_not_signaled(self):
        model = FakeModel()
        with patch.object(model, "validate", side_effect=RuntimeError("mismatched plist")):
            with self.assertRaisesRegex(RuntimeError, "mismatched plist"):
                model.reload()
        self.assertEqual(model.signals, 0)

    def test_bounds_checked_before_backend_calls(self):
        model = FakeModel()
        for timeout in (0, 1801):
            with self.assertRaises(ValueError):
                model.reload(timeout_s=timeout)
        self.assertFalse(model.validated)
        self.assertEqual(model.signals, 0)

    def test_launchd_signal_is_only_sigterm(self):
        model = LaunchdModel()
        with patch("graceful_model_reload.subprocess.run") as run:
            run.return_value.returncode = 0
            model.terminate()
        self.assertEqual(run.call_args.args[0], ["launchctl", "kill", "SIGTERM", model.target])


if __name__ == "__main__":
    unittest.main()
