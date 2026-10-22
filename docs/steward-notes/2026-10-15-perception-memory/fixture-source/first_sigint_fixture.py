"""One owned asyncio-parent SIGINT with a finite fake camera worker; no hardware."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time


PARENT = r'''
import asyncio, importlib.util, json, os, pathlib, subprocess, sys, time
helper_path, work = sys.argv[1:]
work = pathlib.Path(work)
spec = importlib.util.spec_from_file_location('owned_fixture_camera', helper_path)
helper = importlib.util.module_from_spec(spec); spec.loader.exec_module(helper)
children = []
actual_popen = subprocess.Popen
def track(*args, **kwargs):
    child = actual_popen(*args, **kwargs); children.append(child)
    (work / 'parent-ready.json').write_text(json.dumps({'parent_pid':os.getpid(),'child_pid':child.pid,'at':time.time()}))
    return child
helper.subprocess.Popen = track
worker = "import json,os,pathlib,sys,time; w=pathlib.Path(sys.argv[1]); (w/'child-ready.json').write_text(json.dumps({'pid':os.getpid(),'at':time.time()})); time.sleep(0.7); sys.stdout.buffer.write(b'\\xff\\xd8fake-jpeg\\xff\\xd9'); sys.stdout.flush(); (w/'child-complete.json').write_text(json.dumps({'pid':os.getpid(),'at':time.time()}))"
result = {'parent_pid':os.getpid(),'python':sys.version,'capture_returned':False}
async def main():
    value = helper._capture_from_command([sys.executable,'-I','-B','-c',worker,str(work)], logger=helper.LOG)
    child = children[0]
    try:
        os.waitpid(child.pid, os.WNOHANG)
    except ChildProcessError:
        reaped = True
    else:
        reaped = False
    result.update({'capture_returned':True,'returned_at':time.time(),'jpeg_hex':None if value is None else value.hex(),'child_pid':child.pid,'child_returncode':child.returncode,'child_reaped':reaped})
    (work/'capture-returned.json').write_text(json.dumps(result))
    await asyncio.sleep(0)
try:
    asyncio.run(main())
except KeyboardInterrupt:
    result.update({'first_sigint_observed_as_keyboard_interrupt':True,'interrupt_observed_at':time.time()})
else:
    result['first_sigint_observed_as_keyboard_interrupt'] = False
finally:
    result['native_or_model_modules_loaded'] = [name for name in ('cv2','numpy','mlx','torch') if name in sys.modules]
    (work/'parent-result.json').write_text(json.dumps(result))
'''


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--work-dir", required=True, type=Path)
    parser.add_argument("--evidence-output", required=True, type=Path)
    args = parser.parse_args()
    assert sys.flags.isolated and sys.flags.dont_write_bytecode
    helper = Path(__file__).resolve().with_name("camera_capture.py")
    expected = "4692f30fe56090490cc648b78447e993a368a5a148ef53a0b2e38d235caae34d"
    assert hashlib.sha256(helper.read_bytes()).hexdigest() == expected
    args.work_dir.mkdir(parents=True, exist_ok=False)
    command = [sys.executable, "-I", "-B", "-c", PARENT, str(helper), str(args.work_dir)]
    started = datetime.now(timezone.utc).isoformat()
    signal_count = 0
    with (args.work_dir / "parent-output.log").open("xb") as output:
        parent = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=output, stderr=None)
        deadline = time.monotonic() + 5
        while not (args.work_dir / "child-ready.json").exists():
            if parent.poll() is not None or time.monotonic() >= deadline:
                raise RuntimeError("Owned fixture did not become ready; no signal sent")
            time.sleep(0.01)
        ready = json.loads((args.work_dir / "parent-ready.json").read_text())
        child = json.loads((args.work_dir / "child-ready.json").read_text())
        assert ready["parent_pid"] == parent.pid and ready["child_pid"] == child["pid"]
        assert parent.poll() is None and not (args.work_dir / "capture-returned.json").exists()
        sent_at = time.time()
        os.kill(parent.pid, signal.SIGINT)
        signal_count += 1
        returncode = parent.wait(timeout=5)
    result = json.loads((args.work_dir / "parent-result.json").read_text())
    worker_completed = json.loads((args.work_dir / "child-complete.json").read_text())
    checks = {
        "single_signal_only": signal_count == 1,
        "parent_exited_cleanly_after_handling_interrupt": returncode == 0,
        "signal_preceded_worker_completion": sent_at < worker_completed["at"],
        "capture_completed_after_signal": result["capture_returned"] and sent_at < result["returned_at"],
        "exact_fake_jpeg_returned": result["jpeg_hex"] == b"\xff\xd8fake-jpeg\xff\xd9".hex(),
        "child_exit_zero_and_reaped_before_cancellation": result["child_returncode"] == 0 and result["child_reaped"],
        "first_signal_deferred_to_async_cancellation_point": result["first_sigint_observed_as_keyboard_interrupt"] and result["interrupt_observed_at"] >= result["returned_at"],
        "no_native_or_model_modules_loaded": result["native_or_model_modules_loaded"] == [],
    }
    evidence = {
        "schema": "perception_camera_first_sigint_owned_fixture_v1",
        "started_at": started,
        "completed_at": datetime.now(timezone.utc).isoformat(),
        "helper": {"path": str(helper), "sha256": expected},
        "fixture": {"path": str(Path(__file__).resolve()), "sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},
        "python_executable": sys.executable,
        "python_version": sys.version,
        "parent_pid": parent.pid,
        "child_pid": child["pid"],
        "signal": "SIGINT",
        "signal_count": signal_count,
        "signal_sent_at_unix": sent_at,
        "checks": checks,
        "passed": all(checks.values()),
        "parent_result": result,
        "worker_completed": worker_completed,
        "work_directory": str(args.work_dir),
        "artifacts": {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in args.work_dir.iterdir() if p.is_file()},
        "scope": "One signal to this fixture's owned asyncio parent, one finite 0.7-second fake worker, no production signals or process changes, no camera/cv2/models.",
        "limitations": "First SIGINT only under asyncio.run; repeated/immediate KeyboardInterrupt during wait remains outside this guarantee.",
        "production_mutations": 0,
    }
    with args.evidence_output.open("x") as output:
        json.dump(evidence, output, indent=2, sort_keys=True)
        output.write("\n")
        output.flush()
        os.fsync(output.fileno())
    print(json.dumps({"passed": evidence["passed"], "checks": checks, "parent_pid": parent.pid, "child_pid": child["pid"], "evidence": str(args.evidence_output)}, indent=2))
    return 0 if evidence["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
