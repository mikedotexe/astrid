#!/usr/bin/env python3
"""Prepare pinned MLX qualification environments without touching live packages.

Downloads only wheels, records their hashes, freezes the tracked Python source,
and installs three complete inventories into new Python 3.12 environments.
This tool never starts inference, contacts reservoir services, or activates a job.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys


CHANGES = {
    "baseline": {},
    "core": {"mlx": "0.32.4", "mlx-metal": "0.32.4"},
    "full": {"mlx": "0.32.4", "mlx-metal": "0.32.4", "mlx-lm": "0.32.0",
             "transformers": "5.7.0"},
}
EXPECTED = {"mlx": "0.32.0", "mlx-metal": "0.32.0", "mlx-lm": "0.31.3",
            "transformers": "5.4.0"}
INVENTORY = """import importlib.metadata as m,json,re,sys
print(json.dumps({'python':sys.version.split()[0], 'executable':sys.executable,
 'packages':{re.sub(r'[-_.]+','-',d.metadata['Name']).lower():d.version
 for d in m.distributions()}}))"""


def run(args, *, cwd=None, capture=True):
    return subprocess.run([str(a) for a in args], cwd=cwd, text=True, check=True,
                          stdout=subprocess.PIPE if capture else None).stdout


def digest(path):
    with Path(path).open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def save(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def inventories(baseline):
    if baseline["python"] != "3.12.13":
        raise ValueError("qualification requires the inspected Python 3.12.13")
    for name, version in EXPECTED.items():
        if baseline["packages"].get(name) != version:
            raise ValueError(f"baseline changed: {name}; re-audit before preparation")
    return {stage: baseline["packages"] | changes for stage, changes in CHANGES.items()}


def wheel_index(wheels):
    result = {}
    for path in sorted(wheels.glob("*.whl")):
        name, version, *_ = path.name.split("-")
        key = (re.sub(r"[-_.]+", "-", name).lower(), version)
        if key in result:
            raise ValueError(f"ambiguous wheel selection: {key}")
        result[key] = {"filename": path.name, "sha256": digest(path),
                       "size": path.stat().st_size}
    return result


def lock_text(packages, wheels):
    lines = []
    for name, version in sorted(packages.items()):
        wheel = wheels[(name, version)]
        lines.append(f"{name}=={version} --hash=sha256:{wheel['sha256']}")
    return "\n".join(lines) + "\n"


def prepare(args):
    output = args.output_root.absolute()
    evidence = args.evidence_root.absolute()
    root = args.source_root.resolve()
    python = args.baseline_python.absolute()
    baseline = json.loads(run([python, "-B", "-c", INVENTORY]))
    targets = inventories(baseline)
    if output.exists():
        raise ValueError("output root exists; preserve prior evidence and choose a fresh directory")
    if any((evidence / name).exists() for name in ("preparation.json", "live-inventory.json", "wheel-inventory.json")):
        raise ValueError("evidence already exists; choose a fresh evidence directory")
    if run(["git", "status", "--porcelain", "--untracked-files=no"], cwd=root).strip():
        raise ValueError("reservoir tracked source is dirty; freeze only a reviewed source")
    revision = run(["git", "rev-parse", "HEAD"], cwd=root).strip()
    source_paths = [Path(p) for p in run(["git", "ls-files"], cwd=root).splitlines()
                    if Path(p).suffix == ".py" and len(Path(p).parts) == 1]
    if Path("coupled_astrid_server.py") not in source_paths:
        raise ValueError("missing server source")
    output.mkdir(parents=True, mode=0o700)
    evidence.mkdir(parents=True, exist_ok=True)
    save(evidence / "live-inventory.json", baseline)
    wheels = output / "wheelhouse"
    wheels.mkdir()
    union = sorted({f"{name}=={version}" for packages in targets.values()
                    for name, version in packages.items()})
    # pip's resolver rejects multiple versions even with --no-deps. Download
    # the baseline together and each changed version separately.
    baseline_specs = [f"{n}=={v}" for n, v in sorted(targets["baseline"].items())]
    base_command = [python, "-B", "-m", "pip", "download", "--disable-pip-version-check",
                    "--no-cache-dir", "--only-binary=:all:", "--no-deps", "--dest", wheels]
    run(base_command + baseline_specs, capture=False)
    for spec in union:
        if spec not in baseline_specs:
            run(base_command + [spec], capture=False)
    indexed = wheel_index(wheels)
    save(evidence / "wheel-inventory.json", {f"{n}=={v}": row for (n, v), row in indexed.items()})
    if (run(["git", "rev-parse", "HEAD"], cwd=root).strip() != revision
            or run(["git", "status", "--porcelain", "--untracked-files=no"], cwd=root).strip()):
        raise ValueError("source revision changed during wheel download")
    source_hashes = {str(p): digest(root / p) for p in source_paths}
    record = {"schema": "mlx_environment_preparation_v1", "source_root": str(root),
              "source_revision": revision, "source_hashes": source_hashes,
              "baseline_python": str(python), "baseline_python_sha256": digest(python),
              "model_snapshot": str(args.model_snapshot.resolve()), "stages": {},
              "started_at": datetime.now(timezone.utc).isoformat()}
    save(evidence / "preparation.json", record)
    for name, packages in targets.items():
        stage = output / name
        stage.mkdir()
        source = stage / "source"
        source.mkdir()
        for path in source_paths:
            shutil.copy2(root / path, source / path)
        lock = evidence / f"{name}.requirements.lock"
        lock.write_text(lock_text(packages, indexed))
        env = stage / ".venv"
        run([python, "-B", "-m", "venv", env], capture=False)
        candidate = env / "bin/python"
        run([candidate, "-B", "-m", "pip", "install", "--disable-pip-version-check",
             "--no-index", "--find-links", wheels, "--only-binary=:all:",
             "--require-hashes", "--no-deps", "-r", lock], capture=False)
        actual = json.loads(run([candidate, "-B", "-c", INVENTORY]))
        if actual["packages"] != packages or actual["python"] != baseline["python"]:
            raise ValueError(f"installed inventory mismatch: {name}")
        run([candidate, "-B", "-m", "pip", "check"], capture=False)
        save(evidence / f"{name}-inventory.json", actual)
        record["stages"][name] = {"python": str(candidate), "source_root": str(source),
                                 "requirements_lock": str(lock), "packages": packages}
        save(evidence / "preparation.json", record)
    if json.loads(run([python, "-B", "-c", INVENTORY])) != baseline:
        raise ValueError("live inventory changed during preparation")
    if {str(p): digest(root / p) for p in source_paths} != source_hashes:
        raise ValueError("live source changed during preparation")
    if run(["git", "rev-parse", "HEAD"], cwd=root).strip() != revision:
        raise ValueError("source revision changed during preparation")
    record["completed_at"] = datetime.now(timezone.utc).isoformat()
    record["live_environment_unchanged"] = True
    save(evidence / "preparation.json", record)
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-python", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--evidence-root", type=Path, required=True)
    parser.add_argument("--model-snapshot", type=Path, required=True)
    args = parser.parse_args()
    if not (args.model_snapshot / "config.json").is_file():
        parser.error("model snapshot must already be available locally")
    try:
        record = prepare(args)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as exc:
        print(f"MLX environment preparation failed: {exc}", file=sys.stderr)
        return 1
    print(json.dumps({"completed_at": record["completed_at"], "stages": list(record["stages"])}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
