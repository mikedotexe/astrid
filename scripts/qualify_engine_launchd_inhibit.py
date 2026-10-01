#!/usr/bin/env python3
"""Opt-in launchd fixture only. Never addresses a production service."""
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


def run(*args, check=True):
    return subprocess.run(args, capture_output=True, text=True, check=check, timeout=20)


def qualify():
    label = "com.codex.engine-inhibit-fixture." + uuid.uuid4().hex
    target = f"gui/{os.getuid()}/{label}"
    domain = f"gui/{os.getuid()}"
    with tempfile.TemporaryDirectory(prefix="engine-inhibit-") as temporary:
        path = Path(temporary) / (label + ".plist")
        path.write_bytes(plistlib.dumps({"Label": label, "ProgramArguments": ["/bin/sleep", "120"],
                                       "RunAtLoad": True, "KeepAlive": True, "ThrottleInterval": 1}))
        def pid():
            output = run("launchctl", "print", target, check=False).stdout
            match = re.search(r"^\s*pid = (\d+)$", output, re.MULTILINE)
            return int(match[1]) if match else None
        run("launchctl", "bootstrap", domain, str(path))
        try:
            deadline = time.monotonic() + 10
            while pid() is None and time.monotonic() < deadline:
                time.sleep(.1)
            original = pid()
            if original is None:
                raise RuntimeError("fixture did not launch")
            run("launchctl", "disable", target)
            os.kill(original, signal.SIGTERM)
            time.sleep(10)
            after = pid()
            run("launchctl", "enable", target)
            run("launchctl", "kickstart", target)
            deadline = time.monotonic() + 10
            while (pid() is None or pid() == original) and time.monotonic() < deadline:
                time.sleep(.1)
            return {"schema": "launchd_inhibit_fixture_v1", "label": label,
                    "original_pid": original, "pid_after_disabled_exit": after,
                    "pid_after_enable": pid(), "inhibited": after is None,
                    "scope": "owned_sleep_fixture_only_not_production_drain"}
        finally:
            run("launchctl", "bootout", target, check=False)
            run("launchctl", "enable", target, check=False)


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-host-fixture", action="store_true", required=True)
    parser.parse_args()
    print(json.dumps(qualify(), indent=2))
