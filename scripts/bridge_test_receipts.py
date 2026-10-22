#!/usr/bin/env python3
"""Run or explicitly reuse input-bound local Rust suite evidence.

Only source-study and public-web suites are qualified here. Bridge tests that
execute Python helpers, paired migration, handoff, packaging and live readiness
still require fresh checks. A receipt grants no deployment authority.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time

import bridge_reuse as reuse
import bridge_stage as stage

SCHEMA = "bridge_rust_test_receipt_v1"
SUITES = {"source-study": "astrid-source-study", "public-web": "astrid-public-web"}
TIMEOUT = 1800


def test_environment() -> dict[str, str]:
    # No live endpoints, service settings or opt-in fixture-output variables.
    environment = {key: value for key, value in os.environ.items() if key in {
        "PATH", "HOME", "TMPDIR", "CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "DEVELOPER_DIR", "SDKROOT",
        "MACOSX_DEPLOYMENT_TARGET"}}
    environment.update(CARGO_PROFILE_DEV_DEBUG="0", CARGO_PROFILE_TEST_DEBUG="0", CARGO_INCREMENTAL="0")
    return environment


def inputs(source: Path, target_dir: Path, suite: str, environment: dict) -> dict:
    packages = stage.local_packages(source)
    snapshot = stage.input_snapshot(source, packages)
    target = stage.host_target(source)
    compiled = stage.compilation_inputs(source, snapshot, target, environment)
    command = ["cargo", "test", "--manifest-path", str(source / "Cargo.toml"),
               "-p", SUITES[suite], "--locked", "--offline", "--target", target,
               "--target-dir", str(target_dir), "--", "--test-threads=1"]
    # These are shared source/tool identities, not a claim that cargo test used
    # the release profile from the binary-build recipe.
    compiled.pop("recipe")
    compiled["schema"] = "bridge_rust_suite_sources_v1"
    return {"schema": "bridge_rust_test_inputs_v1", "suite": suite, "source_inputs": compiled,
            "command": command, "timeout_seconds": TIMEOUT,
            "harness_sha256": reuse.file_hash(Path(__file__))}


def execute(command: list[str], source: Path, environment: dict, stdout, stderr) -> int:
    with subprocess.Popen(command, cwd=source, env=environment, stdout=stdout, stderr=stderr,
                          start_new_session=True) as child:
        try:
            return child.wait(timeout=TIMEOUT)
        except (subprocess.TimeoutExpired, KeyboardInterrupt):
            try:
                os.killpg(child.pid, signal.SIGTERM)
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
            except ProcessLookupError:
                child.wait()
            raise


def verify(directory: Path) -> dict:
    directory = directory.resolve(strict=True)
    if (directory / "failure.json").exists():
        raise ValueError("test receipt retains a failed attempt")
    receipt = stage.json_file(directory / "receipt.json")
    if (receipt.get("schema") != SCHEMA or receipt.get("status") != "passed"
            or receipt.get("returncode") != 0 or receipt.get("activation_performed") is not False
            or receipt.get("mode") not in {"executed", "reused"}):
        raise ValueError("test receipt is not a completed passing run")
    for name in ("inputs.json", "stdout.log", "stderr.log"):
        if reuse.file_hash(directory / name) != receipt.get("files", {}).get(name):
            raise ValueError(f"test receipt input/log mismatch: {name}")
    if stage.json_file(directory / "inputs.json").get("suite") != receipt.get("suite"):
        raise ValueError("test receipt suite mismatch")
    return receipt


def qualify(source: Path, target_dir: Path, directory: Path, suite: str,
            reuse_from: Path | None = None) -> dict:
    source = source.resolve(strict=True)
    target_dir = target_dir.absolute()
    directory = directory.absolute()
    if suite not in SUITES:
        raise ValueError("suite is not qualified for reuse")
    if target_dir.resolve() != target_dir or directory.parent.resolve() != directory.parent:
        raise ValueError("use canonical target and existing receipt-parent paths")
    if directory.exists() or directory.is_symlink():
        raise ValueError("receipt directory must be new; evidence is never overwritten")
    # A Cargo target or receipt must not become a source input or alter a stage.
    packages = stage.local_packages(source)
    if any(path.is_relative_to(package) or package.is_relative_to(path)
           for path in (target_dir, directory) for package in packages):
        raise ValueError("test outputs must be outside package inputs")
    for parent in (target_dir, *target_dir.parents):
        if (parent / "ready.json").exists() or (parent / "manifest.json").exists():
            raise ValueError("test builds cannot write inside a release stage")
    directory.mkdir(mode=0o700)
    started = time.monotonic()
    try:
        environment = test_environment()
        before = inputs(source, target_dir, suite, environment)
        stage.atomic_json(directory / "inputs.json", before)
        donor = None
        donor_hash = None
        executed_at = dt.datetime.now(dt.UTC).isoformat()
        execution_seconds = None
        if reuse_from is not None:
            if before.get("source_inputs", {}).get("reuse_limitations"):
                raise ValueError("test reuse is not qualified for the selected custom tool configuration")
            reuse_from = reuse_from.resolve(strict=True)
            donor = verify(reuse_from)
            donor_hash = reuse.file_hash(reuse_from / "receipt.json")
            if stage.json_file(reuse_from / "inputs.json") != before:
                raise ValueError("test inputs differ; explicit reuse refused without running tests")
            for name in ("stdout.log", "stderr.log"):
                shutil.copyfile(reuse_from / name, directory / name)
                if reuse.file_hash(directory / name) != donor["files"][name]:
                    raise ValueError("test evidence changed during copy")
            if verify(reuse_from) != donor or reuse.file_hash(reuse_from / "receipt.json") != donor_hash:
                raise ValueError("test donor changed during reuse")
            executed_at = donor["executed_at"]
            execution_seconds = donor["execution_seconds"]
            returncode = 0
        else:
            # This audit is read-only and does not authorize cache deletion.
            subprocess.run([sys.executable, "-B", str(source / "scripts/build_cache_audit.py"),
                            "--check", "--target-dir", str(target_dir)], check=True, timeout=60,
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            execution_start = time.monotonic()
            with (directory / "stdout.log").open("xb") as stdout, (directory / "stderr.log").open("xb") as stderr:
                returncode = execute(before["command"], source, environment, stdout, stderr)
            execution_seconds = time.monotonic() - execution_start
            if returncode != 0:
                raise ValueError(f"Rust suite failed with exit code {returncode}; logs retained")
        if environment != test_environment() or before != inputs(source, target_dir, suite, environment):
            raise ValueError("test inputs changed during qualification")
        for name in ("stdout.log", "stderr.log"):
            with (directory / name).open("rb") as handle:
                os.fsync(handle.fileno())
        receipt = {"schema": SCHEMA, "status": "passed", "suite": suite, "returncode": returncode,
                   "mode": "reused" if donor else "executed", "executed_at": executed_at,
                   "execution_seconds": execution_seconds, "operation_seconds": time.monotonic() - started,
                   "recorded_at": dt.datetime.now(dt.UTC).isoformat(),
                   "donor": {"path": str(reuse_from), "receipt_sha256": donor_hash} if donor else None,
                   "files": {name: reuse.file_hash(directory / name) for name in ("inputs.json", "stdout.log", "stderr.log")},
                   "activation_performed": False}
        stage.atomic_json(directory / "receipt.json", receipt)
        return verify(directory)
    except BaseException as error:
        stage.atomic_json(directory / "failure.json", {"schema": SCHEMA, "status": "failed",
            "error": type(error).__name__ + ": " + str(error)[:1000], "activation_performed": False})
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--receipt-dir", type=Path, required=True)
    parser.add_argument("--suite", choices=sorted(SUITES), required=True)
    parser.add_argument("--reuse-from", type=Path)
    args = parser.parse_args()
    try:
        print(json.dumps(qualify(args.source_root, args.target_dir, args.receipt_dir, args.suite, args.reuse_from), indent=2))
        return 0
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(json.dumps({"ok": False, "error": str(error)[:1000], "activation_performed": False}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
