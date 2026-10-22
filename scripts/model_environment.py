#!/usr/bin/env python3
"""Validate frozen provider releases and launch them behind an owned hold.

This module imports only the standard library. A held process cannot import MLX,
open the model listener, or touch the shared reservoir/journal.
"""
from __future__ import annotations

import argparse
import base64
import csv
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from typing import Any


SCHEMA = "model_environment_v1"
HOLD_SCHEMA = "model_environment_hold_v1"
SELECTION_SCHEMA = "model_environment_selection_v1"


def digest(path: Path) -> str:
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def interpreter_paths(python: Path) -> set[Path]:
    """Bind both macOS framework launcher and its exact re-exec executable."""
    resolved = python.resolve(strict=True)
    paths = {resolved}
    if (resolved.parent.name == "bin" and resolved.parent.parent.parent.name == "Versions"
            and resolved.parent.parent.parent.parent.name == "Python.framework"):
        executable = resolved.parent.parent / "Resources/Python.app/Contents/MacOS/Python"
        if not executable.is_file():
            raise ValueError("framework Python runtime executable is missing")
        paths.add(executable.resolve(strict=True))
    return paths


def command_matches(actual: list[str], expected: list[str]) -> bool:
    if not actual or not expected or actual[1:] != expected[1:]:
        return False
    allowed = {expected[0], *(str(path) for path in interpreter_paths(Path(expected[0])))}
    return actual[0] in allowed


def read_json(path: Path) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 4_194_304:
        raise ValueError(f"invalid record: {path}")
    value = json.loads(path.read_bytes())
    if not isinstance(value, dict):
        raise ValueError(f"record must be an object: {path}")
    return value


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    created = False
    try:
        with temporary.open("x", encoding="utf-8") as handle:
            created = True
            os.chmod(temporary, 0o600)
            json.dump(value, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        created = False
        sync_directory(path.parent)
    finally:
        if created:
            temporary.unlink(missing_ok=True)


def sync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def process_start(pid: int) -> str | None:
    result = subprocess.run(["ps", "-o", "lstart=", "-p", str(pid)],
                            capture_output=True, text=True, timeout=10)
    return result.stdout.strip() or None if result.returncode == 0 else None


def absolute_path(value: Any, name: str, *, directory: bool = False) -> Path:
    if not isinstance(value, str) or not Path(value).is_absolute():
        raise ValueError(f"{name} must be an absolute path")
    path = Path(value)
    if directory and (not path.is_dir() or path.resolve() != path):
        raise ValueError(f"{name} must be an existing canonical directory")
    return path


def option(args: list[str], name: str) -> str:
    if args.count(name) != 1:
        raise ValueError(f"exactly one {name} is required")
    index = args.index(name) + 1
    if index >= len(args) or args[index].startswith("--"):
        raise ValueError(f"missing value for {name}")
    return args[index]


def normalized_name(name: str) -> str:
    import re
    return re.sub(r"[-_.]+", "-", name).lower()


def verify_packages(python: Path, inventory: dict[str, Any], packages: dict[str, str],
                    artifacts: dict[str, str]) -> None:
    """Verify wheel RECORD contents without importing installed packages."""
    if inventory.get("executable") != str(python):
        raise ValueError("package inventory belongs to a different interpreter path")
    recorded = inventory.get("packages")
    if not isinstance(recorded, dict):
        raise ValueError("package inventory is missing packages")
    recorded = {normalized_name(name): version for name, version in recorded.items()}
    if any(recorded.get(normalized_name(name)) != version for name, version in packages.items()):
        raise ValueError("manifest package versions differ from inventory")
    env_root = python.parent.parent
    configuration = env_root / "pyvenv.cfg"
    if str(configuration) not in artifacts:
        raise ValueError("venv configuration is not bound")
    sites = list((env_root / "lib").glob("python*/site-packages"))
    if len(sites) != 1:
        raise ValueError("expected one explicit environment site-packages directory")
    installed = {}
    for distribution in importlib.metadata.distributions(path=[str(sites[0])]):
        name = normalized_name(distribution.metadata["Name"])
        if name in installed:
            raise ValueError(f"duplicate installed distribution: {name}")
        installed[name] = distribution.version
        metadata = Path(distribution._path) / "METADATA"
        record = Path(distribution._path) / "RECORD"
        if str(metadata) not in artifacts or str(record) not in artifacts:
            raise ValueError(f"distribution metadata/RECORD is not bound: {name}")
        with record.open(newline="", encoding="utf-8") as handle:
            for row in csv.reader(handle):
                if len(row) != 3:
                    raise ValueError(f"malformed installed RECORD: {name}")
                relative, encoded_hash, _size = row
                if not encoded_hash:
                    continue  # Wheel RECORD/self and generated pycache have no content hash.
                entry = (sites[0] / relative).resolve()
                if not entry.is_relative_to(env_root.resolve()):
                    raise ValueError(f"installed RECORD escapes environment: {name}")
                algorithm, separator, expected = encoded_hash.partition("=")
                if separator != "=" or algorithm not in {"sha256", "sha384", "sha512"}:
                    raise ValueError(f"unsupported installed RECORD digest: {name}")
                with entry.open("rb") as content:
                    actual = hashlib.file_digest(content, algorithm).digest()
                if base64.urlsafe_b64encode(actual).rstrip(b"=").decode() != expected:
                    raise ValueError(f"installed package content changed: {name}: {relative}")
    if installed != recorded:
        raise ValueError("installed package versions differ from inventory")


def validate_manifest(path: Path) -> dict[str, Any]:
    """Read-only validation; never executes the release interpreter."""
    manifest = read_json(path)
    if manifest.get("schema") != SCHEMA:
        raise ValueError("unsupported model environment schema")
    release_id = manifest.get("release_id")
    if not isinstance(release_id, str) or not release_id or len(release_id) > 128:
        raise ValueError("invalid release_id")
    python = absolute_path(manifest.get("python"), "python")
    if not python.is_file() or not os.access(python, os.X_OK):
        raise ValueError("release interpreter is unavailable")
    source = absolute_path(manifest.get("source_root"), "source_root", directory=True)
    cwd = absolute_path(manifest.get("working_directory"), "working_directory", directory=True)
    relative = manifest.get("server_relative_path")
    if relative != "coupled_astrid_server.py":
        raise ValueError("unsupported provider entrypoint")
    if not (source / relative).is_file():
        raise ValueError("provider entrypoint is missing")
    args = manifest.get("argv")
    if not isinstance(args, list) or not all(isinstance(arg, str) and "\0" not in arg for arg in args):
        raise ValueError("argv must be a string array")
    model = absolute_path(option(args, "--model"), "model", directory=True)
    if not (model / "config.json").is_file() or not list(model.glob("*.safetensors")):
        raise ValueError("model snapshot must contain config and safetensors weights")
    if option(args, "--port") != "8090":
        raise ValueError("production release must retain port 8090")
    runtime = manifest.get("runtime_paths")
    if not isinstance(runtime, dict):
        raise ValueError("runtime_paths is required")
    journal = absolute_path(runtime.get("coupling_journal"), "coupling_journal")
    if journal != cwd / "state/coupling_journal.json":
        raise ValueError("working directory must preserve the explicit coupling journal")
    qos = absolute_path(runtime.get("model_qos_receipts"), "model_qos_receipts")
    if option(args, "--model-qos-receipts") != str(qos):
        raise ValueError("QoS receipts require an explicit preserved absolute path")
    packages = manifest.get("packages")
    if not isinstance(packages, dict) or any(not isinstance(packages.get(name), str)
                                            for name in ("mlx", "mlx-metal", "mlx-lm")):
        raise ValueError("MLX package version inventory is required")
    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, dict) or not artifacts:
        raise ValueError("artifact hashes are required")
    required = {str(path) for path in interpreter_paths(python)}
    for item in source.rglob("*.py"):
        if item.is_symlink():
            raise ValueError("frozen Python sources must not be symlinks")
        required.add(str(item))
    for name in ("package_inventory", "wheel_inventory"):
        required.add(str(absolute_path(manifest.get(name), name)))
    if not required <= set(artifacts):
        raise ValueError("interpreter/source/package/wheel inventory is not completely bound")
    for filename, expected in artifacts.items():
        artifact = absolute_path(filename, "artifact")
        if not isinstance(expected, str) or len(expected) != 64 or digest(artifact) != expected:
            raise ValueError(f"artifact changed: {artifact}")
    for model_file in model.rglob("*"):
        if model_file.is_file() and str(model_file) not in artifacts and str(model_file.resolve()) not in artifacts:
            raise ValueError(f"model snapshot file is not bound: {model_file}")
    verify_packages(python, read_json(Path(manifest["package_inventory"])), packages, artifacts)
    return {"manifest": manifest, "manifest_sha256": digest(path),
            "release_id": release_id, "model": str(model),
            "command": [str(python), str(source / relative), *args]}


def selection(manifest_path: Path) -> dict[str, Any]:
    checked = validate_manifest(manifest_path)
    return {"schema": SELECTION_SCHEMA, "manifest": str(manifest_path.resolve()),
            "manifest_sha256": checked["manifest_sha256"], "release_id": checked["release_id"]}


def selected_release(root: Path) -> dict[str, Any]:
    selected = read_json(root / "active.json")
    if selected.get("schema") != SELECTION_SCHEMA:
        raise ValueError("unsupported model selection")
    path = absolute_path(selected.get("manifest"), "selected manifest")
    checked = validate_manifest(path)
    if checked["manifest_sha256"] != selected.get("manifest_sha256"):
        raise ValueError("selected release manifest changed")
    return checked


def hold_record(root: Path) -> dict[str, Any] | None:
    path = root / "hold.json"
    if not os.path.lexists(path):
        return None
    hold = read_json(path)
    if hold.get("schema") != HOLD_SCHEMA or not isinstance(hold.get("transaction_id"), str):
        raise ValueError("invalid model launch hold")
    return hold


def create_hold(root: Path, transaction_id: str, actor: str) -> dict[str, Any]:
    root.mkdir(parents=True, exist_ok=True, mode=0o700)
    guard = {"schema": HOLD_SCHEMA, "transaction_id": transaction_id, "actor": actor}
    path = root / "hold.json"
    with path.open("x", encoding="utf-8") as handle:
        os.chmod(path, 0o600)
        json.dump(guard, handle, sort_keys=True)
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())
    sync_directory(root)
    return guard


def assert_hold(root: Path, guard: dict[str, Any]) -> None:
    if hold_record(root) != guard:
        raise RuntimeError("model launch hold ownership changed")


def release_hold(root: Path, guard: dict[str, Any]) -> None:
    assert_hold(root, guard)
    (root / "hold.json").unlink()
    sync_directory(root)


def launch(root: Path, *, hold_only: str | None = None) -> None:
    """Wait in stdlib-only state, then exec the selected immutable provider."""
    root = root.resolve(strict=True)
    pid, started = os.getpid(), process_start(os.getpid())
    witnessed = None
    while True:
        hold = hold_record(root)
        if hold_only and (hold is None or hold.get("transaction_id") != hold_only):
            raise RuntimeError("one-shot hold launcher requires its exact owned hold")
        if hold is not None:
            if hold != witnessed:
                atomic_json(root / "held" / f"{pid}.json",
                            {"schema": "model_environment_held_v1", "pid": pid,
                             "started_at": started, "hold": hold, "hold_only": bool(hold_only),
                             "launcher_sha256": digest(Path(__file__).resolve()),
                             "model_imported": False, "listener_opened": False})
                witnessed = hold
            time.sleep(0.1)
            continue
        checked = selected_release(root)
        # Recheck after hashing, immediately before exec. The transition owner
        # establishes the hold before signaling the previous provider.
        if hold_record(root) is not None:
            continue
        manifest = checked["manifest"]
        os.chdir(manifest["working_directory"])
        child_environment = dict(os.environ)
        child_environment.pop("PYTHONPATH", None)
        child_environment.pop("PYTHONHOME", None)
        child_environment["PYTHONNOUSERSITE"] = "1"
        os.execve(checked["command"][0], checked["command"], child_environment)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="operation", required=True)
    inspect = commands.add_parser("check")
    inspect.add_argument("--manifest", type=Path, required=True)
    start = commands.add_parser("launch")
    start.add_argument("--control-root", type=Path, required=True)
    start.add_argument("--hold-only")
    args = parser.parse_args()
    try:
        if args.operation == "check":
            checked = validate_manifest(args.manifest)
            print(json.dumps({key: value for key, value in checked.items() if key != "manifest"}, indent=2))
        else:
            launch(args.control_root, hold_only=args.hold_only)
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        parser.exit(1, f"model environment refused: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
