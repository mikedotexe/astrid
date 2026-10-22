#!/usr/bin/env python3
"""Resolve the running model interpreter from a checked deployment context.

This read-only helper prevents stack receipts from naming the old shared venv
after a versioned release is selected. It never imports MLX or changes a service.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import plistlib
import shlex
import subprocess

import model_environment as environments


MODEL_ROOT = Path("/Users/v/other/neural-triple-reservoir")
ENVIRONMENT_KEYS = {"environment-manifest", "environment-selection", "environment-launcher"}


def checked_artifacts(context: dict) -> dict[str, Path]:
    if context.get("schema") != "stack_build_manifest_v1" or context.get("component") != "coupled-model":
        raise ValueError("model context must be a coupled-model build manifest")
    artifacts = context.get("artifacts", {})
    required = {"python", "server", "gateway", "logit-processor", "generation-controls", "launchd-plist"}
    if not required <= set(artifacts):
        raise ValueError("model context is missing runtime artifacts")
    checked = {}
    for name, artifact in artifacts.items():
        path = Path(artifact.get("path", ""))
        if not path.is_absolute() or environments.digest(path) != artifact.get("sha256"):
            raise ValueError(f"model context artifact changed: {name}")
        checked[name] = path
    return checked


def resolve_runtime(context_path: Path, command: list[str], *, model_root: Path = MODEL_ROOT) -> dict[str, str]:
    artifacts = checked_artifacts(environments.read_json(context_path))
    present = ENVIRONMENT_KEYS & set(artifacts)
    if present:
        if present != ENVIRONMENT_KEYS:
            raise ValueError("partial environment context cannot fall back to the legacy interpreter")
        manifest_path = artifacts["environment-manifest"]
        release = environments.validate_manifest(manifest_path)
        selected = environments.read_json(artifacts["environment-selection"])
        if (selected.get("schema") != environments.SELECTION_SCHEMA
                or selected.get("manifest") != str(manifest_path)
                or selected.get("manifest_sha256") != release["manifest_sha256"]
                or selected.get("release_id") != release["release_id"]):
            raise ValueError("model context does not match the current environment selection")
        manifest = release["manifest"]
        source = Path(manifest["source_root"])
        expected_command = release["command"]
        python = Path(manifest["python"])
        wrapper = artifacts["environment-launcher"]
        if manifest["working_directory"] != str(model_root):
            raise ValueError("selected model changed canonical runtime directory")
    else:
        # This compatibility path applies only to the exact original direct
        # launch configuration, never to a partially described new environment.
        source = model_root
        python = model_root / ".venv/bin/python"
        wrapper = Path(__file__).resolve().with_name("restart_coupled_model.sh")
        settings = plistlib.loads(artifacts["launchd-plist"].read_bytes())
        expected_command = settings.get("ProgramArguments", [])
        if (settings.get("Label") != "com.reservoir.coupled-astrid"
                or settings.get("WorkingDirectory") != str(model_root)
                or expected_command[:2] != [str(python), str(source / "coupled_astrid_server.py")]):
            raise ValueError("legacy fallback requires the original direct provider launch")
    expected_sources = {"server": "coupled_astrid_server.py", "gateway": "coupled_http_gateway.py",
                        "logit-processor": "mlx_reservoir.py", "generation-controls": "generation_controls.py"}
    if (artifacts["python"].resolve() != python.resolve()
            or any(artifacts[name].resolve() != (source / relative).resolve()
                   for name, relative in expected_sources.items())):
        raise ValueError("model context sources/interpreter differ from the selected runtime")
    if not environments.command_matches(command, expected_command):
        raise ValueError("running model argv does not match the checked model context")
    return {"python": str(python), "wrapper": str(wrapper)}


def resolve_interpreter(context_path: Path, command: list[str], *, model_root: Path = MODEL_ROOT) -> Path:
    return Path(resolve_runtime(context_path, command, model_root=model_root)["python"])


def command_for(pid: int) -> list[str]:
    result = subprocess.run(["ps", "-o", "command=", "-p", str(pid)],
                            capture_output=True, text=True, check=True, timeout=10)
    return shlex.split(result.stdout.strip())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--context-manifest", type=Path, required=True)
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--json", action="store_true", help="Include the validated model wrapper identity")
    args = parser.parse_args()
    try:
        if args.pid <= 1:
            raise ValueError("an exact running provider PID is required")
        started_at = environments.process_start(args.pid)
        if not started_at:
            raise ValueError("model process is absent")
        command = command_for(args.pid)
        runtime = resolve_runtime(args.context_manifest, command)
        if environments.process_start(args.pid) != started_at or command_for(args.pid) != command:
            raise ValueError("model process changed during context validation")
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"model runtime identity refused: {error}\n")
    print(json.dumps(runtime) if args.json else runtime["python"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
