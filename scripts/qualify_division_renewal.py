#!/usr/bin/env python3
"""Exercise the exact native dormant supervisor with synthetic state under macOS sandbox."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

from division_runtime_renewal_host import renewed_manifest, parent_authority
from minime_engine_transition_io import require, save


def qualify(binary, template, output):
    binary = binary.resolve(strict=True)
    output = output.absolute()
    require(not output.exists() and not any(p.is_symlink() for p in (output, *output.parents)), "qualification output must be new and unlinked")
    output.mkdir(mode=0o700)
    allowed = output / "fixture"
    allowed.mkdir(mode=0o700)
    profile = '(version 1)(allow default)(deny network*)(deny file-write* (require-not (subpath ' + json.dumps(str(allowed)) + ')))'
    result = {"schema": "division_renewal_native_qualification_v1", "passed": False,
              "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "live_services_touched": False, "live_authority_granted_by_record": False}
    try:
        probe = '''import socket,errno,json
out={}
try:
 with socket.socket() as s: s.connect(("127.0.0.1",9))
except OSError as e: out["network_denied"]=e.errno in (errno.EPERM,errno.EACCES)
try:
 open(%r,"w").write("forbidden")
except OSError as e: out["outside_write_denied"]=e.errno in (errno.EPERM,errno.EACCES)
print(json.dumps(out))
''' % str(output / "forbidden")
        observed = subprocess.run(["/usr/bin/sandbox-exec", "-p", profile, sys.executable, "-c", probe],
                                  capture_output=True, text=True, timeout=10, check=True)
        result["sandbox"] = json.loads(observed.stdout)
        require(result["sandbox"] == {"network_denied": True, "outside_write_denied": True}
                and not (output / "forbidden").exists(), "sandbox denial not demonstrated")
        manifest = json.loads(template.read_bytes())
        manifest.update(runtime_dir=str(allowed / "runtime"), ceremony_ledger=str(allowed / "absent-ledger.jsonl"),
            minime_root=str(allowed / "minime"), astrid_root=str(allowed / "astrid"),
            parent_process_identity="isolated-synthetic-parent", created_at_unix_ms=1, expires_at_unix_ms=2)
        path = allowed / "manifest.json"
        save(path, manifest)
        invocation = ["/usr/bin/sandbox-exec", "-p", profile, str(binary), "division", "supervisor", "--manifest", str(path)]
        expired = subprocess.run(invocation, capture_output=True, timeout=10, cwd=allowed)
        require(expired.returncode != 0 and b"runtime manifest expired" in expired.stderr
                and not (allowed / "runtime/authority.json").exists(), "expired native startup was not refused")
        result["expired_refused_without_authority_write"] = True
        hashes = []
        for iteration in range(2):
            current = renewed_manifest(manifest, int(time.time() * 1000) + iteration)
            save(path, current)
            manifest_hash = hashlib.sha256(path.read_bytes()).hexdigest()
            hashes.append(manifest_hash)
            with (allowed / f"supervisor-{iteration}.log").open("xb") as log:
                child = subprocess.Popen(invocation, stdout=log, stderr=log, cwd=allowed)
                try:
                    deadline = time.monotonic() + 10
                    while time.monotonic() < deadline:
                        require(child.poll() is None, "synthetic supervisor exited before readiness")
                        status_path = allowed / "runtime/supervisor-status.json"
                        if status_path.exists():
                            status = json.loads(status_path.read_bytes())
                            if status["pid"] == child.pid and status["manifest_sha256"] == manifest_hash:
                                require(status["mode"] == "idle_parent_authoritative" and not status["children"]
                                        and not status["matching_intents"] and not status["handoff_ready"]
                                        and status["live_authority_granted_by_record"] is False, "unexpected native authority")
                                parent_authority(json.loads((allowed / "runtime/authority.json").read_bytes()),
                                                 manifest_hash, current["division_id"])
                                break
                        time.sleep(.05)
                    else:
                        raise ValueError("synthetic supervisor readiness timed out")
                finally:
                    if child.poll() is None:
                        child.terminate()
                    child.wait(timeout=10)
        require(hashes[0] != hashes[1], "renewal identity did not change")
        require(not Path(manifest["ceremony_ledger"]).exists(), "qualification invented ceremony records")
        result.update(passed=True, native_parent_authority_rebound=True, native_manifest_hashes=hashes,
                      no_daughter_launch=True, gateway_socket_transition_tested=False)
    except BaseException as error:
        result["error"] = str(error)[:1000]
        raise
    finally:
        save(output / "qualification.json", result)
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--template", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(qualify(args.binary, args.template, args.output), sort_keys=True))
