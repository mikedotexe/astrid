#!/usr/bin/env python3
"""Exercise next-spawn override and model holds using owned, model-free jobs."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import subprocess
import sys
import tempfile
import time
import uuid

import model_environment as environments


PREFIX = "com.codex.model-handoff-fixture."


def held_override_arguments(executable: Path, launcher: Path, control: Path, transaction: str) -> list[str]:
    # Admin-01 on macOS 27 parsed the repeated Python token as a source file.
    # The local man page instead documents argv0 after --. This script-first
    # tail is a host-specific hypothesis until this owned fixture passes.
    return ["--program", str(executable), "--", str(launcher), "launch", "--control-root", str(control),
            "--hold-only", transaction]


def run(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, capture_output=True, text=True, check=check, timeout=20)


def debug_fixture(target: str, arguments: list[str], *, sudo_debug: bool) -> tuple[int, str]:
    """Elevate only the next-spawn override for this user's owned fixture.

    sudo inherits the terminal. No password is read, piped, captured, or stored
    by Python, and the rest of the fixture retains the invoking user's identity.
    """
    expected_prefix = f"gui/{os.getuid()}/{PREFIX}"
    if not target.startswith(expected_prefix) or not re.fullmatch(r"[0-9a-f]{32}", target[len(expected_prefix):]):
        raise ValueError("privileged fixture debug cannot target another service")
    command = ["/bin/launchctl", "debug", target, *arguments]
    if not sudo_debug:
        result = run(*command, check=False)
        return result.returncode, (result.stdout + result.stderr)[-4000:]
    if os.geteuid() == 0:
        raise ValueError("run the fixture as the ordinary desktop user, not sudo Python")
    if not sys.stdin.isatty():
        raise ValueError("--sudo-debug requires an interactive Terminal; credentials are never piped")
    result = subprocess.run(["/usr/bin/sudo", "--", *command], check=False, timeout=180)
    return result.returncode, "sudo debug output displayed directly on the controlling terminal; not captured"


def wait_until(predicate, description: str, timeout: float = 20):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(0.1)
    raise RuntimeError(f"fixture timed out: {description}")


def fixture_manifest(root: Path, release_id: str, executable: Path) -> Path:
    source = root / release_id
    source.mkdir()
    server = source / "coupled_astrid_server.py"
    server.write_text("import os,signal,time\nfrom pathlib import Path\n"
                      f"Path('executions').open('a').write('{release_id} '+str(os.getpid())+'\\n')\n"
                      "stopping=False\n"
                      "def stop(*_):\n global stopping\n stopping=True\n"
                      "signal.signal(signal.SIGTERM,stop)\n"
                      "while not stopping: time.sleep(.1)\n"
                      f"Path('drained').open('a').write('{release_id} '+str(os.getpid())+'\\n')\n")
    env = source / ".venv"
    (env / "bin").mkdir(parents=True)
    python = env / "bin/python"
    python.symlink_to(executable)
    configuration = env / "pyvenv.cfg"
    configuration.write_text(f"home = {executable.parent}\ninclude-system-site-packages = false\n")
    site = env / f"lib/python{sys.version_info.major}.{sys.version_info.minor}/site-packages"
    site.mkdir(parents=True)
    package_artifacts = {}
    for name in ("mlx", "mlx-metal", "mlx-lm"):
        dist = site / (name.replace("-", "_") + "-0.0.0.dist-info")
        dist.mkdir()
        metadata = dist / "METADATA"
        metadata.write_text(f"Metadata-Version: 2.1\nName: {name}\nVersion: 0.0.0\n")
        record = dist / "RECORD"
        record.write_text(f"{dist.name}/METADATA,,\n{dist.name}/RECORD,,\n")
        package_artifacts.update({str(p): environments.digest(p) for p in (metadata, record)})
    inventory = source / "packages.json"
    inventory.write_text(json.dumps({"python": "fixture", "executable": str(python),
                                     "packages": {name: "0.0.0" for name in ("mlx", "mlx-metal", "mlx-lm")}}))
    wheels = source / "wheels.json"
    wheels.write_text('{"scope":"no_wheels_or_model_loaded"}\n')
    model = root / "model-placeholder"
    model.mkdir(exist_ok=True)
    (model / "config.json").write_text('{"fixture":true}\n')
    (model / "model.safetensors").write_bytes(b"owned fixture never loaded")
    path = source / "manifest.json"
    manifest = {"schema": environments.SCHEMA, "release_id": release_id,
                "python": str(python), "source_root": str(source),
                "server_relative_path": server.name, "working_directory": str(root),
                "argv": ["--port", "8090", "--model", str(model),
                         "--model-qos-receipts", str(root / "qos.jsonl")],
                "runtime_paths": {"coupling_journal": str(root / "state/coupling_journal.json"),
                                  "model_qos_receipts": str(root / "qos.jsonl")},
                "packages": {name: "0.0.0" for name in ("mlx", "mlx-metal", "mlx-lm")},
                "package_inventory": str(inventory), "wheel_inventory": str(wheels),
                "artifacts": {**package_artifacts, **{str(p): environments.digest(p) for p in model.iterdir()},
                              **{str(p): environments.digest(p)
                              for p in (*environments.interpreter_paths(executable), configuration, server, inventory, wheels)}}}
    environments.atomic_json(path, manifest)
    return path


def qualify(*, sudo_debug: bool = False) -> dict:
    label = PREFIX + uuid.uuid4().hex
    domain, target = f"gui/{os.getuid()}", f"gui/{os.getuid()}/{label}"
    if not label.startswith(PREFIX):
        raise ValueError("fixture cannot address a production label")
    launcher = Path(environments.__file__).resolve()
    executable = Path(sys.executable).resolve()
    result = {"schema": "model_launchd_handoff_fixture_v1", "label": label,
              "scope": "owned_stdlib_fixture_no_model_no_gpu_no_production_services",
              "sudo_debug_requested": sudo_debug,
              "passed": False, "debug_supported": False, "signal_sent": False,
              "launcher_sha256": environments.digest(launcher),
              "fixture_sha256": environments.digest(Path(__file__).resolve()),
              "activation_sha256": environments.digest(Path(__file__).with_name("model_environment_activate.py")),
              "uid": os.getuid(), "hostname": os.uname().nodename,
              "boot_session_uuid": run("sysctl", "-n", "kern.bootsessionuuid").stdout.strip(),
              "python": str(executable), "python_sha256": environments.digest(executable),
              "python_runtime_sha256": {str(path): environments.digest(path)
                                        for path in environments.interpreter_paths(executable)},
              "host_version": run("sw_vers", "-productVersion").stdout.strip()}
    with tempfile.TemporaryDirectory(prefix="model-handoff-fixture-") as temporary:
        root = Path(temporary).resolve()
        control = root / "control"
        transaction = uuid.uuid4().hex
        guard = environments.create_hold(control, transaction, "owned-host-fixture")
        baseline = fixture_manifest(root, "baseline", executable)
        candidate = fixture_manifest(root, "candidate", executable)
        environments.atomic_json(control / "active.json", environments.selection(baseline))
        original = root / "original.sh"
        original.write_text('#!/bin/bash\nset -eu\n'
                            f'trap \'sleep 1; echo "original $$" >> "{root}/drained"; exit 0\' TERM\n'
                            f'echo "original $$" >> "{root}/executions"\n'
                            'while true; do sleep 0.1; done\n')
        plist = root / f"{label}.plist"
        settings = {"Label": label, "ProgramArguments": ["/bin/bash", str(original)],
                    "RunAtLoad": True, "KeepAlive": True, "ThrottleInterval": 1,
                    "WorkingDirectory": str(root), "StandardOutPath": str(root / "stdout"),
                    "StandardErrorPath": str(root / "stderr")}
        plist.write_bytes(plistlib.dumps(settings))

        def pid():
            output = run("launchctl", "print", target, check=False).stdout
            match = re.search(r"^\s*pid = (\d+)\s*$", output, re.MULTILINE)
            return int(match[1]) if match else None

        def held():
            current = pid()
            path = control / "held" / f"{current}.json"
            if current is None or not path.is_file():
                return None
            receipt = environments.read_json(path)
            if receipt.get("hold") != guard or receipt.get("started_at") != environments.process_start(current):
                return None
            return current

        run("launchctl", "bootstrap", domain, str(plist))
        try:
            old_pid = wait_until(lambda: pid() if (root / "executions").exists() else None,
                                 "initial fixture launch")
            result["original_pid"] = old_pid
            original_start = environments.process_start(old_pid)
            arguments = held_override_arguments(executable, launcher, control, transaction)
            result["debug_arguments"] = arguments
            result["debug_argument_contract"] = "script_first_tail_requires_this_owned_host_fixture_to_pass"
            debug_code, debug_output = debug_fixture(target, arguments, sudo_debug=sudo_debug)
            result["debug_returncode"] = debug_code
            result["debug_output"] = debug_output
            if debug_code != 0:
                result["failure"] = "next_spawn_override_unsupported_no_signal_sent"
                result["root_privileges_required"] = "requires root privileges" in result["debug_output"]
                return result
            result["debug_supported"] = True
            if pid() != old_pid or environments.process_start(old_pid) != original_start:
                raise RuntimeError("fixture identity changed before signal")
            run("launchctl", "kill", "SIGTERM", target)
            result["signal_sent"] = True
            hold_pid = wait_until(held, "one-shot held replacement")
            assert hold_pid != old_pid
            assert (root / "drained").read_text().splitlines() == [f"original {old_pid}"]
            time.sleep(1)
            assert (root / "executions").read_text().splitlines() == [f"original {old_pid}"]
            result["one_shot_held_pid"] = hold_pid
            result["old_work_finished_before_replacement"] = True
            # Only this verified, model-free hold process is booted out.
            assert held() == hold_pid
            run("launchctl", "bootout", target)
            wait_until(lambda: environments.process_start(hold_pid) is None, "hold bootout")
            settings["ProgramArguments"] = [str(executable), str(launcher), "launch",
                                            "--control-root", str(control)]
            plist.write_bytes(plistlib.dumps(settings))
            run("launchctl", "bootstrap", domain, str(plist))
            permanent_pid = wait_until(held, "permanent held launcher")
            environments.atomic_json(control / "active.json", environments.selection(candidate))
            environments.release_hold(control, guard)
            wait_until(lambda: f"candidate {permanent_pid}" in (root / "executions").read_text(),
                       "candidate exec")
            assert pid() == permanent_pid
            result["same_pid_candidate_exec"] = True
            # Exercise later rollback with no next-spawn override or bootout.
            rollback_id = uuid.uuid4().hex
            guard = environments.create_hold(control, rollback_id, "owned-host-fixture")
            run("launchctl", "kill", "SIGTERM", target)
            rollback_pid = wait_until(held, "rollback held launcher")
            assert rollback_pid != permanent_pid
            assert f"candidate {permanent_pid}" in (root / "drained").read_text()
            environments.atomic_json(control / "active.json", environments.selection(baseline))
            environments.release_hold(control, guard)
            wait_until(lambda: f"baseline {rollback_pid}" in (root / "executions").read_text(),
                       "baseline rollback exec")
            assert pid() == rollback_pid
            result.update(passed=True, rollback_same_pid_exec=True,
                          no_execution_during_hold=True, rollback_pid=rollback_pid,
                          executions=(root / "executions").read_text().splitlines())
            return result
        except (OSError, ValueError, RuntimeError, AssertionError, subprocess.SubprocessError) as error:
            result["failure"] = str(error) or type(error).__name__
            if (root / "stderr").exists():
                result["stderr"] = (root / "stderr").read_text()[-4000:]
            return result
        finally:
            # These labels and processes are generated by this fixture only.
            cleanup = run("launchctl", "bootout", target, check=False)
            deadline = time.monotonic() + 10
            absent = False
            while time.monotonic() < deadline:
                absent = run("launchctl", "print", target, check=False).returncode != 0
                if absent:
                    break
                time.sleep(.1)
            result["cleanup"] = {"bootout_returncode": cleanup.returncode,
                                 "service_absent": absent}
            if not absent:
                result["passed"] = False
                result["cleanup_failure"] = "owned fixture service did not disappear within bounded cleanup"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-owned-host-fixture", action="store_true", required=True)
    parser.add_argument("--sudo-debug", action="store_true",
                        help="Use sudo only for the owned fixture's launchctl debug call; run in Terminal as your normal user")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("output must be a new evidence path")
    if os.geteuid() == 0:
        parser.error("do not sudo the whole fixture: run as your ordinary desktop user with --sudo-debug")
    if args.sudo_debug and not sys.stdin.isatty():
        parser.error("--sudo-debug requires an interactive Terminal; do not pipe a password")
    result = qualify(sudo_debug=args.sudo_debug)
    environments.atomic_json(args.output, result)
    print(json.dumps(result, indent=2))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
