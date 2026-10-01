#!/usr/bin/env python3
"""Opt-in owned KeepAlive fixture for the exact engine launch-hold loop."""
import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import signal
import subprocess
import tempfile
import time
import uuid

from minime_engine_transition_io import HOLD_LOOP


def run(*args, check=True):
    return subprocess.run(args, capture_output=True, text=True, check=check, timeout=20)


def qualify():
    label = "com.codex.engine-hold-fixture." + uuid.uuid4().hex
    target = f"gui/{os.getuid()}/{label}"
    with tempfile.TemporaryDirectory(prefix="engine-hold-") as temporary:
        root = Path(temporary)
        hold, script, marker = root / "hold.json", root / "launch.sh", root / "executions"
        script.write_text(f'#!/bin/bash\nset -euo pipefail\nHOLD="{hold}"\n' + HOLD_LOOP +
                          f'printf "%s\\n" "$$" >> "{marker}"\nexec /bin/sleep 120\n')
        plist = root / (label + ".plist")
        plist.write_bytes(plistlib.dumps({"Label": label, "ProgramArguments": ["/bin/bash", str(script)],
                                         "RunAtLoad": True, "KeepAlive": True, "ThrottleInterval": 1}))
        def pid():
            text = run("launchctl", "print", target, check=False).stdout
            match = re.search(r"^\s*pid = (\d+)$", text, re.MULTILINE)
            return int(match[1]) if match else None
        def wait(predicate):
            deadline = time.monotonic() + 15
            while not predicate():
                if time.monotonic() > deadline:
                    raise RuntimeError("fixture timed out")
                time.sleep(.1)
        run("launchctl", "bootstrap", f"gui/{os.getuid()}", str(plist))
        try:
            wait(lambda: marker.exists())
            original = pid()
            hold.write_text(json.dumps({"owner": label}))
            os.kill(original, signal.SIGTERM)
            wait(lambda: pid() is not None and pid() != original)
            held = pid()
            time.sleep(6)
            assert marker.read_text().splitlines() == [str(original)], "fixture ran through hold"
            assert pid() == held
            hold.unlink()
            wait(lambda: len(marker.read_text().splitlines()) == 2)
            assert marker.read_text().splitlines() == [str(original), str(held)]
            assert pid() == held
            return {"schema": "launchd_hold_fixture_v1", "label": label, "passed": True,
                    "old_pid": original, "held_then_exec_pid": held,
                    "no_execution_during_hold": True, "same_pid_after_exec": True,
                    "scope": "owned_sleep_fixture_not_production_drain"}
        finally:
            run("launchctl", "bootout", target, check=False)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-host-fixture", action="store_true", required=True)
    parser.parse_args()
    print(json.dumps(qualify(), indent=2))
