"""Tiny CPU-only containment checks. Every worker is fake; no native camera use."""

import argparse
import ast
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock
from datetime import datetime, timezone


ROOT = Path(__file__).resolve().parent
HELPER_PATH = ROOT / "camera_capture.py"
SPEC = importlib.util.spec_from_file_location("isolated_camera_capture", HELPER_PATH)
HELPER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HELPER)
JPEG = b"\xff\xd8fake-jpeg\xff\xd9"
OWNED_CHILDREN = []
ACTUAL_POPEN = subprocess.Popen


class RecordingStream(io.BytesIO):
    def __init__(self, payload, *, read_error=None, close_error=False):
        super().__init__(payload)
        self.consumed = 0
        self.read_error = read_error
        self.close_error = close_error
        self.close_observed = False

    def read(self, size=-1):
        if self.read_error is not None:
            error, self.read_error = self.read_error, None
            raise error
        chunk = super().read(size)
        self.consumed += len(chunk)
        return chunk

    def close(self):
        self.close_observed = True
        super().close()
        if self.close_error:
            raise OSError("fake pipe close error")


class FakeProcess:
    def __init__(self, payload=JPEG, returncode=0, **stream_options):
        self.stdout = RecordingStream(payload, **stream_options)
        self.returncode = None
        self.exit_code = returncode
        self.wait_calls = 0

    def wait(self):
        assert self.stdout.close_observed
        self.wait_calls += 1
        self.returncode = self.exit_code
        return self.returncode


def record_child(*args, **kwargs):
    # Keep exact ownership handles, and verify each child has been reaped below.
    child = ACTUAL_POPEN(*args, **kwargs)
    OWNED_CHILDREN.append(child)
    return child


class ParentBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.logger = mock.Mock()

    def run_fake(self, child, maximum=16 * 1024 * 1024):
        with mock.patch.object(HELPER.subprocess, "Popen", return_value=child) as launch:
            value = HELPER._capture_from_command(["owned-fake-command"], logger=self.logger, maximum=maximum)
        self.assertEqual(child.wait_calls, 1)
        self.assertTrue(child.stdout.close_observed)
        self.assertIsNotNone(child.returncode)
        self.assertEqual(launch.call_args.kwargs, {
            "stdin": subprocess.DEVNULL, "stdout": subprocess.PIPE, "stderr": None,
        })
        return value

    def test_success_returns_bytes_after_reaping(self):
        self.assertEqual(self.run_fake(FakeProcess()), JPEG)

    def test_nonzero_exit_rejects_valid_jpeg(self):
        self.assertIsNone(self.run_fake(FakeProcess(returncode=3)))

    def test_malformed_and_empty_outputs_are_rejected(self):
        for payload in (b"", b"text", b"\xff\xd8missing-end", b"missing-start\xff\xd9"):
            with self.subTest(payload=repr(payload)):
                self.assertIsNone(self.run_fake(FakeProcess(payload)))

    def test_overflow_is_drained_and_discarded_before_reaping(self):
        child = FakeProcess(b"x" * 53)
        with mock.patch.object(HELPER, "READ_CHUNK_BYTES", 7):
            self.assertIsNone(self.run_fake(child, maximum=12))
        self.assertEqual(child.stdout.consumed, 53)

    def test_exact_limit_is_accepted(self):
        self.assertEqual(self.run_fake(FakeProcess(), maximum=len(JPEG)), JPEG)

    def test_read_error_still_closes_and_reaps(self):
        self.assertIsNone(self.run_fake(FakeProcess(read_error=OSError("fake read error"))))

    def test_close_error_still_reaps_and_rejects(self):
        self.assertIsNone(self.run_fake(FakeProcess(close_error=True)))

    def test_interruption_reaps_then_propagates(self):
        child = FakeProcess(read_error=KeyboardInterrupt())
        with mock.patch.object(HELPER.subprocess, "Popen", return_value=child):
            with self.assertRaises(KeyboardInterrupt):
                HELPER._capture_from_command(["owned-fake-command"], logger=self.logger)
        self.assertEqual(child.wait_calls, 1)
        self.assertTrue(child.stdout.close_observed)

    def test_spawn_error_is_static_log_without_exception_payload(self):
        with mock.patch.object(HELPER.subprocess, "Popen", side_effect=OSError("secret-payload")):
            self.assertIsNone(HELPER._capture_from_command(["owned-fake-command"], logger=self.logger))
        self.assertNotIn("secret-payload", repr(self.logger.mock_calls))

    def test_public_entry_uses_exact_interpreter_and_index(self):
        with mock.patch.object(HELPER, "_capture_from_command", return_value=JPEG) as call:
            self.assertEqual(HELPER.capture_frame(7, logger=self.logger), JPEG)
        self.assertEqual(call.call_args.args[0], [
            sys.executable, "-B", str(HELPER_PATH), "--camera-worker", "7",
        ])
        self.assertEqual(call.call_args.kwargs, {"logger": self.logger})

    def test_parent_imports_no_native_or_model_modules(self):
        self.assertFalse(any(name in sys.modules for name in ("cv2", "numpy", "mlx", "torch")))

    def test_allocation_cap_and_no_timeout_or_forced_termination(self):
        self.assertEqual(HELPER.MAX_JPEG_BYTES, 16 * 1024 * 1024)
        tree = ast.parse(HELPER_PATH.read_text())
        for node in ast.walk(tree):
            if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute):
                self.assertNotIn(node.func.attr, ("kill", "terminate", "communicate"))
                if node.func.attr == "wait":
                    self.assertEqual(node.args, [])
                    self.assertEqual(node.keywords, [])


FAKE_CV2_WORKER = r'''
import json, os, runpy, sys, types
helper, case, release_path, events_path = sys.argv[1:]
events = []
class Capture:
    def isOpened(self):
        events.append('isOpened')
        if case == 'open_exception': raise RuntimeError('fake open exception')
        return case not in ('open_false', 'native_noise_error')
    def read(self):
        events.append('read')
        if case == 'read_exception': raise RuntimeError('fake read exception')
        return (False, None) if case == 'read_false' else (True, 'frame')
    def release(self):
        events.append('release')
        with open(release_path, 'a') as f: f.write('released\n')
        if case == 'release_exception': raise RuntimeError('fake release exception')
class Buffer:
    nbytes = 13
    def tobytes(self):
        events.append('tobytes')
        if case == 'malformed': return b'bad-jpeg'
        return b'\xff\xd8fake-jpeg\xff\xd9'
def video_capture(*args, **kwargs):
    assert args == (7,) and kwargs == {}, (args, kwargs)
    if case in ('native_noise', 'native_noise_error'): os.write(1, b'fake native camera diagnostic\n')
    events.append('VideoCapture')
    return Capture()
def encode(*args, **kwargs):
    assert args == ('.jpg', 'frame') and kwargs == {}, (args, kwargs)
    events.append('imencode')
    if case == 'encode_exception': raise RuntimeError('fake encode exception')
    buffer = Buffer()
    if case == 'oversize': buffer.nbytes = 100
    return (False, buffer) if case == 'encode_false' else (True, buffer)
fake = types.ModuleType('cv2')
fake.VideoCapture, fake.imencode = video_capture, encode
assert 'cv2' not in sys.modules
sys.modules['cv2'] = fake
namespace = runpy.run_path(helper)
namespace['_camera_worker'].__globals__['MAX_JPEG_BYTES'] = 32
try:
    if case in ('native_noise', 'native_noise_error'):
        actual_worker = namespace['_camera_worker']
        reserved = []
        def checking_worker(index, *, output):
            descriptor = output.fileno()
            assert not os.get_inheritable(descriptor)
            reserved.append(descriptor)
            events.append('pipe_non_inheritable')
            return actual_worker(index, output=output)
        namespace['main'].__globals__['_camera_worker'] = checking_worker
        sys.argv = [helper, '--camera-worker', '7']
        result = namespace['main']()
        assert len(reserved) == 1
        try:
            os.fstat(reserved[0])
        except OSError:
            events.append('pipe_closed')
        else:
            raise AssertionError('reserved JPEG descriptor still open')
    else:
        result = namespace['_camera_worker'](7, output=sys.stdout.buffer)
finally:
    with open(events_path, 'w') as f: json.dump(events, f)
    assert sys.modules['cv2'] is fake
    assert not any(name in sys.modules for name in ('numpy', 'mlx', 'torch'))
raise SystemExit(result)
'''


class OwnedSubprocessTests(unittest.TestCase):
    def setUp(self):
        self.logger = mock.Mock()
        self.temp = tempfile.TemporaryDirectory(prefix="astrid-camera-fake-")
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)

    def run_child(self, code, *args, maximum=32):
        command = [sys.executable, "-I", "-B", "-c", code, *map(str, args)]
        before = len(OWNED_CHILDREN)
        with mock.patch.object(HELPER.subprocess, "Popen", side_effect=record_child):
            value = HELPER._capture_from_command(command, logger=self.logger, maximum=maximum)
        self.assertEqual(len(OWNED_CHILDREN), before + 1)
        child = OWNED_CHILDREN[-1]
        self.assertIsNotNone(child.returncode)
        with self.assertRaises(ChildProcessError):
            os.waitpid(child.pid, os.WNOHANG)
        return value, child

    def test_small_real_process_success_and_nonzero_are_reaped(self):
        for status in (0, 3):
            with self.subTest(status=status):
                value, child = self.run_child(
                    "import sys;sys.stdout.buffer.write(b'\\xff\\xd8fake-jpeg\\xff\\xd9');sys.stdout.flush();sys.exit(int(sys.argv[1]))", status
                )
                self.assertEqual(child.returncode, status)
                self.assertEqual(value, JPEG if status == 0 else None)

    def test_small_real_oversized_output_is_fully_drained(self):
        finished = self.directory / "finished"
        value, child = self.run_child(
            "import pathlib,sys;sys.stdout.buffer.write(b'x'*129);sys.stdout.flush();pathlib.Path(sys.argv[1]).write_text('finished')", finished
        )
        self.assertIsNone(value)
        self.assertEqual(child.returncode, 0)
        self.assertEqual(finished.read_text(), "finished")

    def test_finite_output_larger_than_pipe_buffer_is_drained(self):
        finished = self.directory / "larger-finished"
        code = "import pathlib,sys\nchunk=b'x'*4096\nfor _ in range(64): sys.stdout.buffer.write(chunk)\nsys.stdout.flush()\npathlib.Path(sys.argv[1]).write_text('finished')"
        value, child = self.run_child(code, finished)
        self.assertIsNone(value)
        self.assertEqual(child.returncode, 0)
        self.assertEqual(finished.read_text(), "finished")

    def test_small_real_empty_and_malformed_outputs(self):
        for code in ("pass", "import sys;sys.stdout.buffer.write(b'malformed')"):
            with self.subTest(code=code):
                value, child = self.run_child(code)
                self.assertIsNone(value)
                self.assertEqual(child.returncode, 0)

    def test_fake_cv2_worker_releases_on_all_paths_and_preserves_defaults(self):
        cases = ("success", "native_noise", "native_noise_error", "open_false", "open_exception", "read_exception", "read_false",
                 "release_exception", "encode_false", "encode_exception", "oversize", "malformed")
        for case in cases:
            with self.subTest(case=case):
                release = self.directory / (case + "-release")
                events_path = self.directory / (case + "-events.json")
                value, child = self.run_child(FAKE_CV2_WORKER, HELPER_PATH, case, release, events_path)
                events = json.loads(events_path.read_text())
                self.assertEqual(release.read_text(), "released\n")
                self.assertEqual(events.count("release"), 1)
                if "imencode" in events:
                    self.assertLess(events.index("release"), events.index("imencode"))
                if case in ("success", "native_noise"):
                    self.assertEqual(value, JPEG)
                    self.assertEqual(child.returncode, 0)
                else:
                    self.assertIsNone(value)
                    self.assertEqual(child.returncode, 1)
                if case in ("encode_false", "oversize"):
                    self.assertNotIn("tobytes", events)
                if case in ("native_noise", "native_noise_error"):
                    self.assertIn("pipe_non_inheritable", events)
                    self.assertIn("pipe_closed", events)


class SourceIsolationTests(unittest.TestCase):
    def test_only_capture_frame_body_changed_in_perception(self):
        before = ast.parse((ROOT / "baseline/perception.py").read_text())
        after = ast.parse((ROOT / "perception.py").read_text())
        for tree in (before, after):
            target = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == "capture_frame")
            target.body = [ast.Pass()]
        self.assertEqual(ast.dump(before), ast.dump(after))

    def test_capture_frame_delegates_index_and_logger(self):
        import builtins
        from typing import Optional
        tree = ast.parse((ROOT / "perception.py").read_text())
        target = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == "capture_frame")
        delegate = mock.Mock(return_value=JPEG)
        fake = type("FakeHelper", (), {"capture_frame": delegate})
        logger = object()
        def restricted_import(name, *args, **kwargs):
            self.assertEqual(name, "camera_capture")
            return fake
        namespace = {"__builtins__": {**vars(builtins), "__import__": restricted_import}, "Optional": Optional, "log": logger}
        exec(compile(ast.Module(body=[target], type_ignores=[]), "isolated-function", "exec"), namespace)
        self.assertEqual(namespace["capture_frame"](7), JPEG)
        delegate.assert_called_once_with(7, logger=logger)

    def test_audio_helper_unchanged_and_no_candidate_workspace_created(self):
        self.assertEqual((ROOT / "host_audio_fallback.py").read_bytes(), (ROOT / "baseline/host_audio_fallback.py").read_bytes())
        self.assertFalse((ROOT / "workspace").exists())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence-output", type=Path, required=True)
    args = parser.parse_args()
    assert sys.flags.isolated and sys.flags.dont_write_bytecode
    started = datetime.now(timezone.utc).isoformat()
    clock = time.monotonic()
    report = io.StringIO()
    result = unittest.TextTestRunner(stream=report, verbosity=2).run(
        unittest.defaultTestLoader.loadTestsFromModule(sys.modules[__name__])
    )
    evidence = {
        "schema": "perception_camera_process_containment_tests_v1",
        "started_at": started,
        "completed_at": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": time.monotonic() - clock,
        "tests_run": result.testsRun,
        "passed": result.wasSuccessful(),
        "test_output": report.getvalue(),
        "source_hashes": {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in (
            "perception.py", "host_audio_fallback.py", "camera_capture.py", "test_camera_capture_containment.py",
        )},
        "invocation": [sys.executable, "-I", "-B", *sys.argv],
        "python_version": sys.version,
        "owned_children": [{"pid": child.pid, "returncode": child.returncode} for child in OWNED_CHILDREN],
        "all_owned_children_reaped": all(child.returncode is not None for child in OWNED_CHILDREN),
        "real_cv2_imported_in_parent": "cv2" in sys.modules,
        "model_modules_imported": [name for name in ("numpy", "mlx", "torch") if name in sys.modules],
        "parent_cpu_s": {"user": resource.getrusage(resource.RUSAGE_SELF).ru_utime, "system": resource.getrusage(resource.RUSAGE_SELF).ru_stime},
        "children_cpu_s": {"user": resource.getrusage(resource.RUSAGE_CHILDREN).ru_utime, "system": resource.getrusage(resource.RUSAGE_CHILDREN).ru_stime},
        "scope": "Candidate-only mock resource tests and tiny owned stdlib processes; worker cases install fake cv2 before helper execution. No real camera, microphone, native OpenCV or model access; no stress or large-allocation tests.",
        "limitations": "No actual camera/TCC/backend timing validation. No camera timeout is introduced: a blocked camera can still block the synchronous caller indefinitely.",
        "production_mutations": 0,
    }
    args.evidence_output.parent.mkdir(parents=True, exist_ok=True)
    with args.evidence_output.open("x") as output:
        json.dump(evidence, output, indent=2, sort_keys=True)
        output.write("\n")
        output.flush()
        os.fsync(output.fileno())
    print(report.getvalue())
    print(json.dumps({"passed": result.wasSuccessful(), "tests": result.testsRun, "owned_children": len(OWNED_CHILDREN), "evidence": str(args.evidence_output)}))
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
