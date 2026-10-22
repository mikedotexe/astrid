"""Characterize unchanged capture_frame with AST extraction and a fake camera.

Run with Python -I -B. This never imports perception, OpenCV, model libraries,
or the audio helper. The extracted function's import hook accepts only fake cv2.
The assertions describe existing behavior; passing is not a resource-safety claim.
"""

import argparse
import ast
import builtins
import hashlib
import io
import json
import os
from pathlib import Path
import resource
import sys
import time
from types import SimpleNamespace
from typing import Optional
import unittest
from datetime import datetime, timezone


CANDIDATE = Path(__file__).resolve().parent
SOURCE = CANDIDATE / "perception.py"
SOURCE_SHA256 = "3b7baa2265373a39e8b46c8ad5a33781b21a86abf23d5959dc2393b53427a57a"
SOURCE_TEXT = SOURCE.read_text()
assert hashlib.sha256(SOURCE.read_bytes()).hexdigest() == SOURCE_SHA256
TREE = ast.parse(SOURCE_TEXT, filename=str(SOURCE))
FUNCTIONS = [node for node in TREE.body if isinstance(node, ast.FunctionDef) and node.name == "capture_frame"]
assert len(FUNCTIONS) == 1 and not FUNCTIONS[0].decorator_list
FUNCTION = FUNCTIONS[0]
CODE = compile(ast.Module(body=[FUNCTION], type_ignores=[]), str(SOURCE), "exec")
OBSERVATIONS = []


def observe(case):
    events = []
    release_calls = []
    import_calls = []

    class Capture:
        def isOpened(self):
            events.append("isOpened")
            if case == "open_check_exception":
                raise RuntimeError("fake open-check error")
            return case != "open_failure"

        def read(self):
            events.append("read")
            if case == "read_exception":
                raise RuntimeError("fake read error")
            return (False, None) if case == "read_false" else (True, "fake-frame")

        def release(self):
            events.append("release")
            release_calls.append(True)

    class Buffer:
        def tobytes(self):
            events.append("tobytes")
            return b"fake-jpeg"

    def construct(index):
        events.append("VideoCapture")
        assert index == 7
        return Capture()

    def encode(extension, frame):
        events.append("imencode")
        assert extension == ".jpg" and frame == "fake-frame"
        if case == "encode_exception":
            raise RuntimeError("fake encode error")
        if case == "encode_false_none":
            return False, None
        return case != "encode_false_buffer", Buffer()

    fake_cv2 = SimpleNamespace(VideoCapture=construct, imencode=encode)

    def restricted_import(name, *args, **kwargs):
        import_calls.append(name)
        if name != "cv2":
            raise AssertionError("Unexpected import in extracted function: " + name)
        return fake_cv2

    namespace = {
        "__builtins__": {**vars(builtins), "__import__": restricted_import},
        "Optional": Optional,
        "log": SimpleNamespace(error=lambda message: events.append("log.error")),
    }
    exec(CODE, namespace)
    result = None
    exception = None
    try:
        result = namespace["capture_frame"](7)
    except Exception as error:
        exception = {"type": type(error).__name__, "message": str(error)}
    row = {
        "case": case,
        "events": events,
        "release_calls": len(release_calls),
        "returned_hex": result.hex() if isinstance(result, bytes) else None,
        "exception": exception,
        "import_calls": import_calls,
        "explicit_capture_release_observed": bool(release_calls),
    }
    OBSERVATIONS.append(row)
    return row


class ExistingLifecycleTests(unittest.TestCase):
    def check_case(self, case, releases, exception=None, returned=None):
        row = observe(case)
        self.assertEqual(row["import_calls"], ["cv2"])
        self.assertEqual(row["release_calls"], releases)
        self.assertEqual(row["returned_hex"], returned)
        self.assertEqual(row["exception"]["type"] if row["exception"] else None, exception)
        if releases:
            self.assertEqual(row["events"].count("release"), 1)
            if "imencode" in row["events"]:
                self.assertLess(row["events"].index("release"), row["events"].index("imencode"))

    def test_open_failure_skips_explicit_release(self):
        self.check_case("open_failure", 0)

    def test_open_check_exception_skips_explicit_release(self):
        self.check_case("open_check_exception", 0, exception="RuntimeError")

    def test_read_exception_skips_explicit_release(self):
        self.check_case("read_exception", 0, exception="RuntimeError")

    def test_read_false_releases_capture(self):
        self.check_case("read_false", 1)

    def test_encode_false_none_releases_then_raises(self):
        self.check_case("encode_false_none", 1, exception="AttributeError")

    def test_encode_false_buffer_is_incorrectly_accepted(self):
        self.check_case("encode_false_buffer", 1, returned=b"fake-jpeg".hex())

    def test_encode_exception_occurs_after_release(self):
        self.check_case("encode_exception", 1, exception="RuntimeError")

    def test_success_releases_once_before_encoding(self):
        self.check_case("success", 1, returned=b"fake-jpeg".hex())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence-output", type=Path, required=True)
    args = parser.parse_args()
    assert sys.flags.isolated and sys.flags.dont_write_bytecode
    assert not any(name in sys.modules for name in ("cv2", "numpy", "mlx", "torch", "perception"))
    started = datetime.now(timezone.utc).isoformat()
    clock = time.monotonic()
    report = io.StringIO()
    result = unittest.TextTestRunner(stream=report, verbosity=2).run(
        unittest.defaultTestLoader.loadTestsFromTestCase(ExistingLifecycleTests)
    )
    evidence = {
        "schema": "perception_capture_lifecycle_characterization_v1",
        "started_at": started,
        "completed_at": datetime.now(timezone.utc).isoformat(),
        "elapsed_s": time.monotonic() - clock,
        "python_executable": sys.executable,
        "isolated_python": bool(sys.flags.isolated),
        "python_version": sys.version,
        "invocation": [sys.executable, "-I", "-B", *sys.argv],
        "provenance": {
            "path": str(CANDIDATE / "provenance.json"),
            "sha256": hashlib.sha256((CANDIDATE / "provenance.json").read_bytes()).hexdigest(),
        },
        "source": {"path": str(SOURCE), "sha256": SOURCE_SHA256},
        "test_harness": {"path": str(Path(__file__).resolve()), "sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},
        "extraction": {
            "method": "Parse copied source as AST, compile and execute capture_frame FunctionDef only; import hook returns fake cv2.",
            "first_line": FUNCTION.lineno,
            "last_line": FUNCTION.end_lineno,
            "function_source_sha256": hashlib.sha256(ast.get_source_segment(SOURCE_TEXT, FUNCTION).encode()).hexdigest(),
        },
        "tests_run": result.testsRun,
        "characterization_tests_passed": result.wasSuccessful(),
        "test_output": report.getvalue(),
        "cases": OBSERVATIONS,
        "findings": [
            "Explicit release is skipped after a constructed capture reports not opened, raises during isOpened, or raises during read.",
            "A false read return, encoding failure or exception, and success all occur after explicit release.",
            "The encoding success flag is ignored; false plus None raises AttributeError, while false plus a buffer is accepted as JPEG bytes.",
        ],
        "limitations": [
            "Passing tests confirm existing behavior, not resource-safety acceptance.",
            "Fake objects cannot establish native camera handle retention, OpenCV destructor behavior, allocator retention, or the cause of prior heap growth.",
            "No production/candidate module import, real OpenCV, camera, microphone, network, models, stress loop, or implementation repair occurred.",
        ],
        "real_cv2_imported": "cv2" in sys.modules,
        "model_modules_imported": [name for name in ("mlx", "torch", "numpy") if name in sys.modules],
        "self_cpu_s": {"user": resource.getrusage(resource.RUSAGE_SELF).ru_utime, "system": resource.getrusage(resource.RUSAGE_SELF).ru_stime},
        "production_mutations": 0,
    }
    args.evidence_output.parent.mkdir(parents=True, exist_ok=True)
    with args.evidence_output.open("x") as output:
        json.dump(evidence, output, indent=2, sort_keys=True)
        output.write("\n")
        output.flush()
        os.fsync(output.fileno())
    print(report.getvalue())
    print(json.dumps({"evidence": str(args.evidence_output), "passed": result.wasSuccessful(), "tests": result.testsRun}))
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
