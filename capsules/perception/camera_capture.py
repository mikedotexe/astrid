"""Capture one JPEG in a short-lived process, keeping OpenCV out of the caller.

The synchronous capture intentionally has no timeout, forced termination, or
retry. A stuck camera can still block the caller, as before. Native allocations
belong to the worker and are reclaimed when it exits; this does not identify
which native allocation caused earlier long-running perception memory growth.
An immediate or repeated interrupt during wait can still interrupt reaping;
this is not an all-signal orphan-prevention mechanism.
"""

import logging
import os
from pathlib import Path
import subprocess
import sys


MAX_JPEG_BYTES = 16 * 1024 * 1024
READ_CHUNK_BYTES = 64 * 1024
LOG = logging.getLogger(__name__)


def _jpeg_envelope_valid(data):
    return len(data) >= 4 and data.startswith(b"\xff\xd8") and data.endswith(b"\xff\xd9")


def _read_bounded(stream, maximum):
    """Retain at most maximum bytes; consume/discard an oversized stream to EOF."""
    payload = bytearray()
    oversized = False
    while True:
        chunk = stream.read(READ_CHUNK_BYTES)
        if not chunk:
            break
        if oversized:
            continue
        if len(payload) + len(chunk) > maximum:
            oversized = True
            payload.clear()
        else:
            payload.extend(chunk)
    return payload, oversized


def _capture_from_command(command, *, logger, maximum=MAX_JPEG_BYTES):
    """Internal boundary, separate from the fixed production command for tests."""
    try:
        process = subprocess.Popen(
            command,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=None,
        )
    except OSError:
        logger.error("Camera capture worker could not start")
        return None

    payload = bytearray()
    oversized = False
    failed = False
    try:
        if process.stdout is None:
            raise RuntimeError("Camera worker stdout pipe missing")
        payload, oversized = _read_bounded(process.stdout, maximum)
    except Exception:
        failed = True
        logger.error("Camera capture worker output could not be read")
    finally:
        try:
            if process.stdout is not None:
                process.stdout.close()
        except Exception:
            failed = True
            logger.error("Camera capture worker output pipe could not be closed")
        finally:
            # No timeout: return bytes only after this exact child has exited
            # and been reaped. Oversized stdout was fully drained above.
            returncode = process.wait()

    if failed:
        return None
    if returncode != 0:
        logger.error("Camera capture worker exited unsuccessfully (status %s)", returncode)
        return None
    if oversized:
        logger.error("Camera capture worker exceeded the JPEG size limit")
        return None
    if not _jpeg_envelope_valid(payload):
        logger.error("Camera capture worker returned an invalid JPEG envelope")
        return None
    return bytes(payload)


def capture_frame(camera_index=0, *, logger=None):
    """Return one bounded JPEG from one worker using this exact interpreter."""
    return _capture_from_command(
        [sys.executable, "-B", str(Path(__file__).resolve()), "--camera-worker", str(camera_index)],
        logger=LOG if logger is None else logger,
    )


def _worker_error(reason):
    # Never print image data, exception text, or a base64 representation.
    print("Camera capture failed: " + reason, file=sys.stderr, flush=True)
    return 1


def _camera_worker(camera_index, *, output=None):
    try:
        # OpenCV is imported only in this short-lived worker entry point.
        import cv2

        capture = None
        try:
            capture = cv2.VideoCapture(camera_index)
            if not capture.isOpened():
                return _worker_error("camera unavailable")
            read_ok, frame = capture.read()
            if not read_ok:
                return _worker_error("frame unavailable")
        finally:
            if capture is not None:
                capture.release()

        # Preserve the existing default backend, resolution and JPEG settings.
        encode_ok, buffer = cv2.imencode(".jpg", frame)
        if not encode_ok or buffer is None:
            return _worker_error("JPEG encoding unsuccessful")
        size = getattr(buffer, "nbytes", None)
        if type(size) is not int or not 4 <= size <= MAX_JPEG_BYTES:
            return _worker_error("JPEG size outside limit")
        payload = buffer.tobytes()
        if len(payload) > MAX_JPEG_BYTES or not _jpeg_envelope_valid(payload):
            return _worker_error("invalid JPEG output")
        sink = sys.stdout.buffer if output is None else output
        written = sink.write(payload)
        if written != len(payload):
            return _worker_error("JPEG write incomplete")
        sink.flush()
        return 0
    except ImportError:
        return _worker_error("OpenCV unavailable")
    except Exception:
        return _worker_error("capture operation unsuccessful")


def main():
    if len(sys.argv) != 3 or sys.argv[1] != "--camera-worker":
        return _worker_error("invalid worker invocation")
    try:
        camera_index = int(sys.argv[2])
    except ValueError:
        return _worker_error("invalid camera index")
    binary_fd = None
    try:
        # Reserve the original pipe exclusively for JPEG bytes. Native library
        # prints to fd1 now follow inherited stderr instead of corrupting it.
        sys.stdout.flush()
        binary_fd = os.dup(sys.stdout.fileno())
        os.set_inheritable(binary_fd, False)
        with os.fdopen(binary_fd, "wb") as output:
            binary_fd = None  # The file object owns and closes this descriptor.
            os.dup2(sys.stderr.fileno(), sys.stdout.fileno())
            return _camera_worker(camera_index, output=output)
    except OSError:
        return _worker_error("worker output channel unavailable")
    finally:
        if binary_fd is not None:
            os.close(binary_fd)


if __name__ == "__main__":
    raise SystemExit(main())
