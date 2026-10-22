"""Owned, stdlib-only check of first-SIGINT asyncio cancellation boundaries."""

import argparse
import asyncio
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time


def event(root, name):
    with (root / "events.jsonl").open("a", encoding="utf-8") as stream:
        stream.write(json.dumps({"event": name, "pid": os.getpid(), "at": time.time()}) + "\n")
        stream.flush()
        os.fsync(stream.fileno())


def worker(root):
    event(root, "worker_ready")
    (root / "worker.pid").write_text(str(os.getpid()))
    deadline = time.monotonic() + 5
    while not (root / "release").exists():
        if time.monotonic() > deadline:
            raise RuntimeError("fixture parent did not release worker")
        time.sleep(0.01)
    event(root, "worker_complete")


async def iteration(root):
    while True:
        event(root, "iteration_start")
        handler = signal.getsignal(signal.SIGINT)
        (root / "handler.json").write_text(json.dumps({
            "function": getattr(getattr(handler, "func", None), "__qualname__", None),
            "is_default": handler is signal.default_int_handler,
        }))
        with (root / "iteration.txt").open("a", encoding="utf-8") as stream:
            stream.write("begin\n")
            stream.flush()
            os.fsync(stream.fileno())
            subprocess.run([sys.executable, "-B", __file__, "--worker", str(root)],
                           check=True, timeout=8)
            event(root, "worker_reaped")
            stream.write("complete\n")
            stream.flush()
            os.fsync(stream.fileno())
        event(root, "iteration_complete")
        await asyncio.sleep(1.0)


def child(root):
    try:
        asyncio.run(iteration(root))
    except KeyboardInterrupt:
        event(root, "keyboard_interrupt_at_runner_boundary")


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def parent(output):
    if output.exists():
        raise FileExistsError(output)
    started = time.time()
    with tempfile.TemporaryDirectory(prefix="astrid-owned-sigint-") as temporary:
        root = Path(temporary).resolve()
        command = [sys.executable, "-B", str(Path(__file__).resolve()), "--child", str(root)]
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        deadline = time.monotonic() + 6
        while not (root / "worker.pid").exists():
            if process.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError("owned fixture did not reach synchronous child wait")
            time.sleep(0.01)
        worker_pid = int((root / "worker.pid").read_text())
        event(root, "parent_sends_one_sigint")
        os.kill(process.pid, signal.SIGINT)
        (root / "release").write_text("release only this owned worker\n")
        stdout, stderr = process.communicate(timeout=12)
        events = [json.loads(line) for line in (root / "events.jsonl").read_text().splitlines()]
        body = (root / "iteration.txt").read_text()
        handler = json.loads((root / "handler.json").read_text())
        try:
            os.kill(worker_pid, 0)
            worker_absent = False
        except ProcessLookupError:
            worker_absent = True
        expected = ["iteration_start", "worker_ready", "parent_sends_one_sigint",
                    "worker_complete", "worker_reaped", "iteration_complete",
                    "keyboard_interrupt_at_runner_boundary"]
        checks = {
            "exact_event_order": [item["event"] for item in events] == expected,
            "complete_single_iteration_write": body == "begin\ncomplete\n",
            "runner_handler_installed": handler["function"] == "Runner._on_sigint" and not handler["is_default"],
            "normal_exit": process.returncode == 0 and not stderr,
            "synchronous_worker_reaped": worker_absent,
        }
        record = {
            "schema": "perception_owned_sigint_fixture_v1", "passed": all(checks.values()),
            "started_at": started, "finished_at": time.time(), "python": sys.executable,
            "python_version": sys.version, "command": command, "owned_child_pid": process.pid,
            "owned_worker_pid": worker_pid, "signal_count": 1, "signal_target": "owned child PID only",
            "fixture_sha256": sha(__file__), "python_sha256": sha(sys.executable),
            "runners_path": asyncio.runners.__file__, "runners_sha256": sha(asyncio.runners.__file__),
            "subprocess_path": subprocess.__file__, "subprocess_sha256": sha(subprocess.__file__),
            "checks": checks, "handler": handler, "events": events, "iteration_bytes": body,
            "returncode": process.returncode, "stdout": stdout, "stderr": stderr,
            "scope": "Private temporary files and two owned Python processes; no production imports or paths.",
        }
    record["temporary_directory_removed"] = not root.exists()
    with output.open("x", encoding="utf-8") as stream:
        json.dump(record, stream, indent=2)
        stream.write("\n")
    print(json.dumps({"passed": record["passed"], "output": str(output), "checks": checks}))
    return 0 if record["passed"] else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--child", type=Path)
    group.add_argument("--worker", type=Path)
    group.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.child:
        child(args.child)
    elif args.worker:
        worker(args.worker)
    else:
        sys.exit(parent(args.output))
