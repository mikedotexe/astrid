#!/usr/bin/env python3
"""Launch a selected stage, retaining explicit live state and staged source paths."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


def digest(path: Path) -> str:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"expected a regular release file: {path}")
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def record(path: Path) -> dict:
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 1_048_576:
        raise ValueError(f"invalid release record: {path}")
    value = json.loads(path.read_bytes())
    if not isinstance(value, dict):
        raise ValueError("release record is not an object")
    return value


def runtime_arguments(root: Path, source: Path) -> list[str]:
    workspace = root / "capsules/spectral-bridge/workspace"
    return ["--astrid-root", str(root), "--bridge-root", str(source / "capsules/spectral-bridge"),
            "--bridge-workspace", str(workspace), "--db-path", str(workspace / "bridge.db"),
            "--workspace-path", str(root.parent / "minime/workspace"),
            "--minime-root", str(root.parent / "minime"),
            "--perception-path", str(root / "capsules/perception/workspace/perceptions")]


def selected_command(root: Path, *, verify_native: bool = True) -> list[str]:
    control = root / ".runtime/bridge-deployment"
    if os.path.lexists(control / "hold.json"):
        raise ValueError("operator deployment hold: no runtime admitted")
    selected = record(control / "active.json")
    if selected.get("schema") != "bridge_release_selection_v1":
        raise ValueError("unsupported release selection")
    stage = Path(selected["stage"])
    if not stage.is_absolute() or stage.resolve() != stage:
        raise ValueError("release stage must be a canonical absolute path")
    if (stage / "failure.json").exists():
        raise ValueError("selected stage has failed evidence")
    manifest = stage / "manifest.json"
    if digest(manifest) != selected.get("manifest_sha256"):
        raise ValueError("selected manifest changed")
    value = record(manifest)
    source = Path(value["repository"]["path"])
    if not source.is_absolute() or not (source / "capsules/spectral-bridge/src").is_dir():
        raise ValueError("staged source tree is unavailable")
    artifacts = value["artifacts"]
    for name, relative in {"spectral-bridge":"spectral-bridge-server",
                           "substrate-probe-v2":"helpers/substrate_probe_v2.py",
                           "release-launcher":"helpers/launchd_spectral_bridge.sh",
                           "release-selection":"helpers/bridge_release_launch.py"}.items():
        path = stage / relative
        if path.resolve() != path or artifacts[name]["path"] != str(path) or digest(path) != artifacts[name]["sha256"]:
            raise ValueError(f"selected artifact changed: {name}")
    binary = stage / "spectral-bridge-server"
    args = [str(binary), "--deployment-manifest", str(manifest), *runtime_arguments(root, source)]
    if verify_native:
        output = subprocess.run([*args, "--verify-deployment-manifest"], capture_output=True,
                                text=True, check=True, timeout=60)
        receipt = json.loads(output.stdout)
        if receipt.get("verified") is not True or receipt.get("manifest_sha256") != selected["manifest_sha256"]:
            raise ValueError("native selected manifest verification failed")
    if os.path.lexists(control / "hold.json"):
        raise ValueError("operator hold appeared during release verification")
    return [*args, "--autonomous"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    args = parser.parse_args()
    try:
        command = selected_command(args.root.resolve(strict=True))
        os.execv(command[0], command)
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        parser.exit(1, f"bridge release not admitted: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
