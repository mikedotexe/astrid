"""Run the real receipt wrapper with synthetic service and receipt boundaries."""
from __future__ import annotations

import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
DRIVER = r'''
import json, os
from pathlib import Path
import sys

operation, args = Path(sys.argv[0]).name, sys.argv[1:]
if operation == "driver":
    operation, args = args[0], args[1:]
root = Path(os.environ["FIXTURE_ROOT"])
scenario = os.environ["FIXTURE_SCENARIO"]
with (root / "calls.jsonl").open("a") as stream:
    stream.write(json.dumps({"operation": operation, "args": args}) + "\n")

if operation == "id":
    assert args == ["-u"]
    print("503")
elif operation == "launchctl":
    assert args[0] == "print" and len(args) == 2
    pids = {"com.astrid.spectral-bridge": "9101", "com.minime.engine": "9102",
            "com.reservoir.coupled-astrid": "9103"}
    label = args[1].removeprefix("gui/503/")
    assert label in pids
    if not (scenario == "absent" and label == "com.reservoir.coupled-astrid"):
        print("pid = " + pids[label])
elif operation == "kill":
    assert args[0] == "-0" and args[1] in {"9101", "9102", "9103"}
elif operation == "ps":
    assert args == ["-p", "9101", "-o", "comm="]
    print(root / "bridge-executable")
elif operation == "lsof":
    assert args in (["-t", "-nP", "-iTCP:7878", "-sTCP:LISTEN"],
                    ["-t", "-nP", "-iTCP:7879", "-sTCP:LISTEN"])
    print("9102")
elif operation == "curl":
    assert args == ["--silent", "--max-time", "2", "--output", "/dev/null",
                    "--write-out", "%{http_code}", args[-1]]
    assert args[-1] in {"http://127.0.0.1:8090/livez", "http://127.0.0.1:8090/readyz"}
    print("200")
elif operation == "file":
    workspace = "/Users/v/other/astrid/capsules/spectral-bridge/workspace"
    assert args[0] in {
        str(root / "bridge-executable"), str(root / "context.json"),
        workspace + "/deployment_manifests/spectral-bridge.json",
        workspace + "/deployment_manifests/minime-engine.json",
        workspace + "/telemetry_heartbeat_delta_v1.json",
        "/Users/v/other/minime/workspace/health.json",
    }
elif operation == "python3":
    if args[:1] == ["-B"]:
        args = args[1:]
    if args[:1] == ["-c"]:
        # Only the wrapper's stdlib plist/JSON parsing runs as real Python.
        os.execv(sys.executable, [sys.executable, "-B", *args])
    name = Path(args[0]).name
    if name == "model_runtime_identity.py":
        pid = "0" if scenario == "absent" else "9103"
        assert args[1:] == ["--context-manifest", str(root / "context.json"), "--pid", pid, "--json"]
        if scenario in {"absent", "stale"}:
            print("fixture: absent PID" if scenario == "absent" else "fixture: stale context", file=sys.stderr)
            sys.exit(1)
        print(os.environ["FIXTURE_IDENTITY_OUTPUT"])
        if scenario == "resolver_failure":
            print("fixture: resolver failed after partial output", file=sys.stderr)
            sys.exit(23)
    elif name == "environment_receipts.py":
        assert args[1:5] == ["--workspace", "/Users/v/other/astrid/capsules/spectral-bridge/workspace",
                            "record-deploy", "coupled-stack"]
        status = args[args.index("--status") + 1]
        # Stand-in receipt sink: never open the supplied canonical workspace.
        with (root / "receipt.json").open("x") as stream:
            json.dump({"status": status, "args": args[1:]}, stream)
        sys.exit(0 if status == "passed" else 1)
    else:
        raise AssertionError("unexpected Python tool: " + name)
else:
    raise AssertionError("unexpected fixture operation: " + operation)
'''

BASH_ENV = r'''
function kill {
  "$FIXTURE_DRIVER" kill "$@"
}
function [ {
  if builtin [ "$#" -eq 3 ] && builtin [ "$1" = "-f" ]; then
    "$FIXTURE_DRIVER" file "$2"
  else
    builtin [ "$@"
  fi
}
'''


class CaptureStackReceiptTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory())).resolve()
        self.commands = self.root / "commands"
        self.commands.mkdir()
        self.driver = self.commands / "driver"
        self.driver.write_text(f"#!{sys.executable}\n" + DRIVER)
        self.driver.chmod(0o755)
        for name in ("id", "launchctl", "ps", "lsof", "curl", "python3"):
            (self.commands / name).symlink_to(self.driver)
        for name in ("dirname", "awk", "sed", "grep"):
            executable = shutil.which(name)
            self.assertIsNotNone(executable)
            (self.commands / name).symlink_to(executable)
        self.bash_env = self.root / "bash-env"
        self.bash_env.write_text(BASH_ENV)
        home = self.root / "home"
        plist = home / "Library/LaunchAgents/com.minime.engine.plist"
        plist.parent.mkdir(parents=True)
        plist.write_bytes(plistlib.dumps({"EnvironmentVariables": {"MINIME_DIVISION_GATEWAY_ENABLED": "false"}}))
        self.context = self.root / "context.json"
        self.context.write_text("{}\n")
        self.identity = {"python": str(self.root / "selected release/bin/python"),
                         "wrapper": str(self.root / "selected release/launcher.py")}
        self.env = {"PATH": str(self.commands), "HOME": str(home), "BASH_ENV": str(self.bash_env),
                    "FIXTURE_ROOT": str(self.root), "FIXTURE_DRIVER": str(self.driver),
                    "PYTHONDONTWRITEBYTECODE": "1", "LC_ALL": "C"}

    def run_wrapper(self, scenario, *, output=None):
        result = subprocess.run(
            ["/bin/bash", str(ROOT / "scripts/capture_stack_receipt.sh"),
             "--model-context-manifest", str(self.context), "--actor", "synthetic", "--ack", "fixture"],
            env={**self.env, "FIXTURE_SCENARIO": scenario,
                 "FIXTURE_IDENTITY_OUTPUT": json.dumps(self.identity) if output is None else output},
            capture_output=True, text=True, timeout=15,
        )
        receipt = self.root / "receipt.json"
        self.assertTrue(receipt.exists(), result.stderr)
        return result, json.loads(receipt.read_text())

    def assert_failed_identity(self, result, receipt):
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(receipt["status"], "failed")
        args = receipt["args"]
        self.assertIn("model_runtime_identity=false", args)
        self.assertIn("livez=true", args)
        self.assertIn("readyz=true", args)
        self.assertFalse(any(arg.startswith(("model-python=", "model-wrapper=")) for arg in args))
        self.assertEqual(args[args.index("--context-manifest") + 1], str(self.context))
        self.assertIn("recording failed receipt", result.stderr)

    def test_absent_model_pid_still_records_failed_identity(self):
        result, receipt = self.run_wrapper("absent")
        self.assert_failed_identity(result, receipt)
        self.assertIn("model_process=false", receipt["args"])
        self.assertNotIn("model=9103", receipt["args"])
        self.assertIn("fixture: absent PID", result.stderr)

    def test_stale_context_cannot_claim_ready_from_healthy_endpoints(self):
        result, receipt = self.run_wrapper("stale")
        self.assert_failed_identity(result, receipt)
        self.assertIn("model_process=true", receipt["args"])
        self.assertIn("fixture: stale context", result.stderr)

    def test_resolver_failure_discards_even_well_formed_partial_output(self):
        result, receipt = self.run_wrapper("resolver_failure")
        self.assert_failed_identity(result, receipt)
        self.assertIn("fixture: resolver failed after partial output", result.stderr)

    def test_valid_identity_records_only_verified_paths(self):
        result, receipt = self.run_wrapper("success")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(receipt["status"], "passed")
        self.assertIn("model_runtime_identity=true", receipt["args"])
        self.assertIn("model-python=" + self.identity["python"], receipt["args"])
        self.assertIn("model-wrapper=" + self.identity["wrapper"], receipt["args"])

    def test_malformed_success_output_still_records_failed_identity(self):
        result, receipt = self.run_wrapper("success", output="not JSON")
        self.assert_failed_identity(result, receipt)

    def test_missing_fields_still_record_failed_identity(self):
        for identity in ({}, {"python": self.identity["python"]}):
            with self.subTest(identity=identity):
                result, receipt = self.run_wrapper("success", output=json.dumps(identity))
                self.assert_failed_identity(result, receipt)
                (self.root / "receipt.json").unlink()

    def test_partial_or_invalid_paths_do_not_leak_into_receipt(self):
        for wrapper in (None, "", "relative.py", "/tmp/embedded\nnewline", 7):
            with self.subTest(wrapper=wrapper):
                result, receipt = self.run_wrapper("success", output=json.dumps({**self.identity, "wrapper": wrapper}))
                self.assert_failed_identity(result, receipt)
                (self.root / "receipt.json").unlink()


if __name__ == "__main__":
    unittest.main()
