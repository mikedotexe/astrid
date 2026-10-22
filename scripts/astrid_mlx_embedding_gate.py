#!/usr/bin/env python3
"""Run the frozen embedding-only gate without evaluating unused model weights.

The frozen test and server compatibility shim remain unchanged. This process-local
loading policy belongs only to qualification, never to the production server.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import importlib
import importlib.util
import inspect
import json
import os
from pathlib import Path
import sys
import tempfile


SCHEMA = "astrid_mlx_embedding_gate_v1"
SOURCE_NAMES = ("test_wide_coupling_offline.py", "coupled_astrid_server.py", "wide_coupling.py")
TEST_NAME = "test_wide_coupling_offline"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def memory_snapshot(mx) -> dict[str, int]:
    result = {}
    for field, getter in (("active_bytes", "get_active_memory"),
                          ("cache_bytes", "get_cache_memory"),
                          ("peak_bytes", "get_peak_memory")):
        value = getattr(mx, getter)()
        if type(value) is not int or value < 0:
            raise ValueError(f"invalid MLX memory measurement: {getter}")
        result[field] = value
    return result


def publish(path: Path, record: dict) -> None:
    descriptor, temporary = tempfile.mkstemp(prefix=".embedding-audit-", dir=path.parent)
    try:
        with os.fdopen(descriptor, "w") as output:
            json.dump(record, output, indent=2, sort_keys=True)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
        parent = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(parent)
        finally:
            os.close(parent)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def run_gate(source_root: Path, model: Path, audit_file: Path) -> int:
    # Reserve a fresh evidence path; repeated qualifications must preserve failures.
    with audit_file.open("x"):
        pass
    record = {"schema": SCHEMA, "passed": False, "loading_policy": "lazy_embeddings_only",
              "source_root": str(source_root), "model": str(model), "loader_calls": 0,
              "load_attempts": 0, "calls": [], "memory": {}, "memory_timestamps": {},
              "pid": os.getpid(), "source_hashes": {},
              "loader_restored": False, "wrapper_sha256": digest(Path(__file__)),
              "started_at": datetime.now(timezone.utc).isoformat()}
    original_path, original_argv = list(sys.path), list(sys.argv)
    original_modules = set(sys.modules)
    mlx_lm = mx = original_load = None
    exit_code = 1
    policy_errors = []
    local_names = set()
    try:
        if not source_root.is_absolute() or source_root.resolve(strict=True) != source_root:
            raise ValueError("source root must be an existing canonical absolute directory")
        if not source_root.is_dir() or not model.is_absolute() or not model.is_dir():
            raise ValueError("qualification requires a frozen source directory and absolute model snapshot")
        for name in SOURCE_NAMES:
            path = source_root / name
            if not path.is_file() or path.is_symlink():
                raise ValueError(f"missing or symlinked frozen source: {name}")
            record["source_hashes"][name] = digest(path)
        record["test_path"] = str(source_root / SOURCE_NAMES[0])
        local_names = {path.stem for path in source_root.glob("*.py")}
        if local_names.intersection(sys.modules):
            raise ValueError("frozen source module was already imported; use a fresh qualification process")
        sys.path.insert(0, str(source_root))
        importlib.invalidate_caches()
        # Importing this wrapper itself never imports MLX or initializes a device.
        mlx_lm = importlib.import_module("mlx_lm")
        mx = importlib.import_module("mlx.core")
        original_load = mlx_lm.load
        mx.reset_peak_memory()
        record["peak_reset_before_test"] = True

        def lazy_load(*args, **kwargs):
            record["load_attempts"] += 1
            caller = inspect.currentframe().f_back
            valid_caller = (caller.f_code.co_name == "_load_mlx_runtime"
                            and Path(caller.f_code.co_filename).resolve()
                            == source_root / "coupled_astrid_server.py")
            del caller
            if record["load_attempts"] != 1 or not valid_caller:
                policy_errors.append("expected exactly one load from the frozen server compatibility shim")
                raise ValueError(policy_errors[-1])
            if len(args) != 1 or os.fspath(args[0]) != str(model):
                policy_errors.append("unexpected model or positional loader overrides")
                raise ValueError(policy_errors[-1])
            if kwargs not in ({}, {"model_config": {"model_type": "gemma4"}}):
                policy_errors.append("unexpected loader overrides; only the existing Gemma4 shim is allowed")
                raise ValueError(policy_errors[-1])
            record["memory"]["before_load"] = memory_snapshot(mx)
            record["memory_timestamps"]["before_load"] = datetime.now(timezone.utc).isoformat()
            call = {"model": str(model), "lazy": True, "original_kwargs": kwargs,
                    "caller": str(source_root / "coupled_astrid_server.py")}
            record["calls"].append(call)
            record["loader_calls"] += 1
            publish(audit_file, record)
            try:
                return original_load(*args, **kwargs, lazy=True)
            finally:
                record["memory"]["after_load"] = memory_snapshot(mx)
                record["memory_timestamps"]["after_load"] = datetime.now(timezone.utc).isoformat()

        mlx_lm.load = lazy_load
        test_path = source_root / SOURCE_NAMES[0]
        spec = importlib.util.spec_from_file_location(TEST_NAME, test_path)
        if spec is None or spec.loader is None:
            raise ValueError("could not import the exact frozen test module")
        module = importlib.util.module_from_spec(spec)
        sys.modules[TEST_NAME] = module
        sys.argv = [str(test_path), "--model", str(model)]
        spec.loader.exec_module(module)
        module.main()
        if mlx_lm.load is not lazy_load:
            raise ValueError("test changed the guarded loader")
        if policy_errors or record["load_attempts"] != 1 or record["loader_calls"] != 1:
            raise ValueError("embedding gate did not complete exactly one permitted lazy load")
        for name, expected in record["source_hashes"].items():
            if digest(source_root / name) != expected:
                raise ValueError(f"frozen source changed during qualification: {name}")
        for name in local_names.intersection(sys.modules):
            if name in original_modules:
                continue
            imported = Path(getattr(sys.modules[name], "__file__", "")).resolve()
            if imported != source_root / f"{name}.py":
                raise ValueError(f"source module escaped the frozen source root: {name}")
        record["passed"] = True
        exit_code = 0
    except BaseException as error:
        record["error"] = f"{type(error).__name__}: {error}"
        exit_code = 130 if isinstance(error, KeyboardInterrupt) else 1
    finally:
        if mlx_lm is not None and original_load is not None:
            mlx_lm.load = original_load
            record["loader_restored"] = mlx_lm.load is original_load
        if mx is not None:
            try:
                record["memory"]["after_test"] = memory_snapshot(mx)
                record["memory_timestamps"]["after_test"] = datetime.now(timezone.utc).isoformat()
            except BaseException as error:
                record["memory_error"] = f"{type(error).__name__}: {error}"
                record["passed"], exit_code = False, 1
        record["policy_errors"] = policy_errors
        sys.path[:], sys.argv[:] = original_path, original_argv
        for name in local_names.difference(original_modules):
            sys.modules.pop(name, None)
        record["completed_at"] = datetime.now(timezone.utc).isoformat()
        publish(audit_file, record)
    return exit_code


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--audit-file", type=Path, required=True)
    args = parser.parse_args()
    return run_gate(args.source_root, args.model, args.audit_file)


if __name__ == "__main__":
    raise SystemExit(main())
