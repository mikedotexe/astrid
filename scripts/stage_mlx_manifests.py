#!/usr/bin/env python3
"""Bind prepared MLX environments to immutable, reviewable release manifests.

This is a stdlib-only, read-only inspection of the installed environments and
model. It writes only a new evidence directory, never an active selection,
launchd configuration, package, or runtime journal.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import importlib.metadata
import json
import os
from pathlib import Path
import plistlib
import subprocess

import model_environment as models


MODEL_REPOSITORY = "mlx-community/gemma-4-12B-it-5bit"
LAUNCH_ENVIRONMENT = (
    "COUPLED_ASTRID_MODEL_MEMORY_MAP", "COUPLED_ASTRID_AUDIT_DIR",
    "COUPLED_ASTRID_WIDE_STRENGTH", "COUPLED_ASTRID_QUEUE_CAPACITY",
    "COUPLED_ASTRID_QOS_MODE", "COUPLED_ASTRID_QOS_RECEIPTS",
    "COUPLED_ASTRID_READINESS_STALE_SECONDS", "PYTHONPATH", "PYTHONHOME",
)


def write_new(path: Path, value: dict) -> None:
    with path.open("x", encoding="utf-8") as handle:
        os.chmod(path, 0o400)
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())


def launch_arguments(settings: dict, preparation: dict) -> list[str]:
    """Fail on drift from the launch configuration inspected for this upgrade.

    The explicit values below preserve the source defaults of the frozen
    e719cd7 release. An unknown launch configuration needs a new review.
    """
    root = Path(preparation["source_root"])
    expected = [preparation["baseline_python"], str(root / "coupled_astrid_server.py"),
                "--port", "8090", "--coupling-strength", "0.1", "--model-memory-map",
                "--model", MODEL_REPOSITORY, "--model-qos-mode", "active"]
    expected_settings = {"Label": "com.reservoir.coupled-astrid", "ProgramArguments": expected,
                         "WorkingDirectory": str(root), "KeepAlive": True, "RunAtLoad": True,
                         "StandardOutPath": str(root / "logs/coupled-astrid.log"),
                         "StandardErrorPath": str(root / "logs/coupled-astrid.log")}
    if settings != expected_settings:
        raise ValueError("original launch settings differ from the inspected provider")
    return ["--host", "127.0.0.1", "--port", "8090",
            "--coupling-strength", "0.1", "--input-dim", "32", "--n-nodes", "192",
            "--model-memory-map", "--model", preparation["model_snapshot"],
            "--wide-coupling-strength", "0.0", "--wide-coupling-rank", "16",
            "--wide-bias-cap", "4.0", "--wide-pressure-floor", "0.25",
            "--generation-queue-capacity", "4", "--model-qos-mode", "active",
            "--model-qos-receipts", str(root / "workspace/model_qos_receipts.jsonl"),
            "--readiness-stale-seconds", "5.0"]


def inspect_launch_environment() -> dict[str, str]:
    values = {}
    for name in LAUNCH_ENVIRONMENT:
        result = subprocess.run(["launchctl", "getenv", name], capture_output=True,
                                text=True, timeout=10, check=False)
        if result.returncode not in (0, 1):
            raise ValueError(f"cannot inspect launchd environment: {name}")
        if result.stdout.strip():
            values[name] = result.stdout.strip()
    if values:
        raise ValueError("launchd has provider/Python overrides; re-audit launch settings")
    return values


def source_artifacts(source: Path, expected: dict[str, str]) -> dict[str, str]:
    if source.resolve() != source or not source.is_dir():
        raise ValueError("frozen source must be an existing canonical directory")
    actual = {str(p.relative_to(source)): models.digest(p)
              for p in source.rglob("*.py") if not p.is_symlink()}
    if actual != expected or any(p.is_symlink() for p in source.rglob("*.py")):
        raise ValueError("frozen source closure differs from preparation")
    return {str(source / name): digest for name, digest in actual.items()}


def installed_artifacts(python: Path) -> dict[str, str]:
    env = python.parent.parent
    sites = list((env / "lib").glob("python*/site-packages"))
    if len(sites) != 1:
        raise ValueError("expected one environment site-packages directory")
    paths = models.interpreter_paths(python) | {env / "pyvenv.cfg"}
    for distribution in importlib.metadata.distributions(path=[str(sites[0])]):
        paths.update(Path(distribution._path) / name for name in ("METADATA", "RECORD"))
    return {str(path): models.digest(path) for path in sorted(paths)}


def build_manifest(*, name: str, release_prefix: str, preparation: dict,
                   package_inventory: Path, wheel_inventory: Path,
                   preparation_path: Path, settings_copy: Path, argv: list[str],
                   model_artifacts: dict[str, str]) -> dict:
    stage = preparation["stages"]["baseline" if name == "original" else name]
    python = Path(preparation["baseline_python"] if name == "original" else stage["python"])
    inventory = models.read_json(package_inventory)
    if (inventory.get("executable") != str(python) or inventory.get("python") != "3.12.13"
            or inventory.get("packages") != stage["packages"]):
        raise ValueError(f"package inventory does not match preparation: {name}")
    root = Path(preparation["source_root"])
    source = Path(stage["source_root"])
    artifacts = model_artifacts | source_artifacts(source, preparation["source_hashes"])
    artifacts |= installed_artifacts(python)
    for path in (package_inventory, wheel_inventory, preparation_path, settings_copy,
                 Path(stage["requirements_lock"])):
        artifacts[str(path)] = models.digest(path)
    if name == "original" and models.digest(python) != preparation["baseline_python_sha256"]:
        raise ValueError("original interpreter changed since preparation")
    return {"schema": models.SCHEMA, "release_id": f"{release_prefix}-{name}",
            "stage": name, "python": str(python), "source_root": str(source),
            "source_revision": preparation["source_revision"],
            "server_relative_path": "coupled_astrid_server.py",
            "working_directory": str(root), "argv": argv,
            "runtime_paths": {"coupling_journal": str(root / "state/coupling_journal.json"),
                              "model_qos_receipts": str(root / "workspace/model_qos_receipts.jsonl")},
            "packages": stage["packages"], "package_inventory": str(package_inventory),
            "wheel_inventory": str(wheel_inventory), "artifacts": dict(sorted(artifacts.items())),
            "qualification_status": "staged_only_not_approved_for_activation"}


def stage_manifests(preparation_path: Path, live_plist: Path, output: Path,
                    release_prefix: str, *, launch_environment=None) -> dict:
    preparation_path, live_plist, output = (p.absolute() for p in (preparation_path, live_plist, output))
    if not release_prefix or len(release_prefix) > 110:
        raise ValueError("release prefix must contain 1..110 characters")
    if os.path.lexists(output):
        raise ValueError("manifest output already exists; preserve previous evidence")
    preparation = models.read_json(preparation_path)
    if (preparation.get("schema") != "mlx_environment_preparation_v1"
            or not preparation.get("completed_at") or preparation.get("live_environment_unchanged") is not True
            or set(preparation.get("stages", {})) != {"baseline", "core", "full"}):
        raise ValueError("complete three-stage preparation is required")
    if preparation.get("source_revision") != "e719cd7f06af1c63fa3288c9689e479a47503586":
        raise ValueError("launch defaults require the inspected source revision")
    initial_plist = live_plist.read_bytes()
    settings = plistlib.loads(initial_plist)
    argv = launch_arguments(settings, preparation)
    environment = inspect_launch_environment() if launch_environment is None else launch_environment
    if environment:
        raise ValueError("launchd environment differs from inspected defaults")
    root = Path(preparation["source_root"])
    if root.resolve() != root or not root.is_dir():
        raise ValueError("runtime working directory is not canonical")
    output = output.parent.resolve() / output.name
    if output.is_relative_to(root) or not output.is_relative_to(preparation_path.parent):
        raise ValueError("manifest output must stay in preparation evidence, outside the runtime tree")
    if any(models.digest(root / name) != expected for name, expected in preparation["source_hashes"].items()):
        raise ValueError("original source changed since preparation")
    model = Path(preparation["model_snapshot"])
    if model.resolve() != model or not model.is_dir():
        raise ValueError("model snapshot must be an existing canonical directory")
    model_artifacts = {str(p): models.digest(p) for p in sorted(model.rglob("*")) if p.is_file()}
    output.mkdir(parents=True, mode=0o700)
    settings_copy = output / "original-launch-settings.json"
    write_new(settings_copy, {"plist_sha256": models.digest(live_plist), "settings": settings,
                              "launchd_provider_environment": environment})
    evidence = preparation_path.parent
    manifests = {}
    for name in ("baseline", "core", "full", "original"):
        inventory = evidence / ("live-inventory.json" if name == "original" else f"{name}-inventory.json")
        manifest = build_manifest(name=name, release_prefix=release_prefix, preparation=preparation,
                                  package_inventory=inventory, wheel_inventory=evidence / "wheel-inventory.json",
                                  preparation_path=preparation_path, settings_copy=settings_copy, argv=argv,
                                  model_artifacts=model_artifacts)
        path = output / f"{name}.json"
        write_new(path, manifest)
        checked = models.validate_manifest(path)
        manifests[name] = {"path": str(path), "sha256": checked["manifest_sha256"],
                           "release_id": checked["release_id"]}
    if live_plist.read_bytes() != initial_plist:
        raise ValueError("original launch configuration changed during staging")
    if any(models.digest(root / name) != expected for name, expected in preparation["source_hashes"].items()):
        raise ValueError("original source changed during staging")
    result = {"schema": "mlx_staged_manifests_v1", "completed_at": datetime.now(timezone.utc).isoformat(),
              "preparation_sha256": models.digest(preparation_path), "manifests": manifests,
              "production_modified": False, "activation_authorized_by_this_receipt": False}
    write_new(output / "staging.json", result)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--preparation", type=Path, required=True)
    parser.add_argument("--live-plist", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--release-prefix", required=True)
    args = parser.parse_args()
    try:
        result = stage_manifests(args.preparation, args.live_plist, args.output_dir, args.release_prefix)
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        parser.exit(1, f"MLX manifest staging refused: {error}\n")
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
