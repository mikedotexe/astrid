#!/usr/bin/env python3
"""Offline engine build witness. No installation, launch, signing or activation API."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import time

SCHEMA = "minime_engine_stage_v1"
BINS = ("minime", "engine_restore_inspect")
TREES = ("minime/src", "minime/shaders", "minime/tests")
FILES = ("minime/Cargo.toml", "minime/Cargo.lock", "minime/build.rs",
         "tests/fixtures/transition_afterimage_v1.json")


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def regular(path: Path) -> None:
    if not stat.S_ISREG(path.lstat().st_mode):
        raise ValueError(f"not a regular file: {path}")


def checked_path(root: Path, relative: str) -> Path:
    if root.is_symlink():
        raise ValueError("symlink in staged root")
    path = Path(relative)
    if path.is_absolute() or not path.parts or any(p in ("..", ".") for p in path.parts):
        raise ValueError("unsafe relative path")
    target = root / path
    for ancestor in (target, *target.parents):
        if ancestor == root:
            break
        if ancestor.is_symlink():
            raise ValueError("symlink in staged input")
    regular(target)
    return target


def inputs(source: Path) -> dict:
    names = set(FILES)
    for tree in TREES:
        root = source / tree
        if root.is_symlink() or not root.is_dir():
            raise ValueError(f"missing or linked input tree: {tree}")
        for path in root.rglob("*"):
            if path.is_symlink():
                raise ValueError("symlink in build inputs")
            if path.is_file():
                names.add(path.relative_to(source).as_posix())
    if len(names) > 4096:
        raise ValueError("input count exceeded")
    result = {}
    total = 0
    for name in sorted(names):
        path = checked_path(source, name)
        size = path.stat().st_size
        total += size
        if size > 16 * 1024 * 1024 or total > 128 * 1024 * 1024:
            raise ValueError("input bytes exceeded")
        result[name] = {"sha256": digest(path), "bytes": size}
    return result


def run(args: list[str], source: Path, env: dict) -> str:
    return subprocess.run(args, cwd=source, env=env, text=True, capture_output=True,
                          check=True, timeout=60).stdout.strip()


def write_new(path: Path, value: dict) -> None:
    with path.open("x") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())
    path.chmod(0o444)


def check_no_cargo_config(source: Path, env: dict) -> None:
    roots = [source / "minime", source, *source.parents,
             Path(env.get("CARGO_HOME", str(Path(env["HOME"]) / ".cargo")))]
    for root in roots:
        for path in (root / ".cargo/config", root / ".cargo/config.toml"):
            if path.exists():
                raise ValueError(f"unreviewed Cargo configuration: {path}")
    home = roots[-1]
    if (home / "config").exists() or (home / "config.toml").exists():
        raise ValueError("unreviewed Cargo home configuration")


def verify(stage: Path, expected: str) -> dict:
    if stage.is_symlink():
        raise ValueError("linked stage")
    manifest_path = checked_path(stage, "manifest.json")
    if digest(manifest_path) != expected:
        raise ValueError("manifest identity changed")
    manifest = json.loads(manifest_path.read_text())
    if (manifest.get("schema") != SCHEMA or manifest.get("activation_authorized") is not False
            or (stage / "FAILED.json").exists()):
        raise ValueError("not a completed offline build witness")
    if set(manifest["artifacts"]) != set(BINS):
        raise ValueError("incomplete artifact set")
    for name, row in manifest["inputs"].items():
        path = checked_path(stage / "source", name)
        if digest(path) != row["sha256"] or path.stat().st_size != row["bytes"]:
            raise ValueError(f"source archive changed: {name}")
    for name, row in manifest["artifacts"].items():
        if row["path"] != f"bin/{name}":
            raise ValueError("unexpected artifact path")
        if digest(checked_path(stage, row["path"])) != row["sha256"]:
            raise ValueError(f"artifact changed: {name}")
    for name, expected_hash in manifest["evidence"].items():
        if digest(checked_path(stage, name)) != expected_hash:
            raise ValueError(f"build evidence changed: {name}")
    return {"schema": SCHEMA, "stage": str(stage), "manifest_sha256": expected,
            "input_count": len(manifest["inputs"]), "artifacts": manifest["artifacts"],
            "verified": True, "activation_authorized": False}


def build(source: Path, stage: Path) -> dict:
    source = source.resolve(strict=True)
    if stage.is_symlink():
        raise ValueError("linked stage")
    stage = stage.parent.resolve(strict=True) / stage.name
    if stage == source or stage.is_relative_to(source) or source.is_relative_to(stage):
        raise ValueError("stage must be separate from source")
    env = {k: v for k, v in os.environ.items()
           if k in ("PATH", "HOME", "USER", "TMPDIR", "LANG", "RUSTUP_HOME", "CARGO_HOME")}
    check_no_cargo_config(source, env)
    before = inputs(source)
    head = run(["git", "rev-parse", "HEAD"], source, env)
    toolchain = {"rustc": run(["rustc", "-vV"], source, env),
                 "cargo": run(["cargo", "-V"], source, env)}
    hosts = [line.removeprefix("host: ") for line in toolchain["rustc"].splitlines()
             if line.startswith("host: ")]
    if len(hosts) != 1 or not hosts[0] or any(c not in "abcdefghijklmnopqrstuvwxyz0123456789_-" for c in hosts[0]):
        raise ValueError("missing or invalid rustc host target")
    host = hosts[0]
    stage.mkdir(mode=0o700, parents=False, exist_ok=False)
    try:
        shutil.copyfile(Path(__file__), stage / "staging-tool.py")
        (stage / "staging-tool.py").chmod(0o444)
        for name, row in before.items():
            target = stage / "source" / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(checked_path(source, name), target)
            if digest(target) != row["sha256"]:
                raise ValueError("source changed while archiving")
            target.chmod(0o444)
        metadata_command = ["cargo", "metadata", "--format-version", "1", "--locked", "--offline",
                            "--filter-platform", host, "--manifest-path", "minime/Cargo.toml"]
        metadata = run(metadata_command, source, env)
        write_new(stage / "dependencies.json", json.loads(metadata))
        command = ["cargo", "build", "--release", "--locked", "--offline", "--manifest-path",
                   "minime/Cargo.toml", "--target-dir", str(stage / "build")]
        for name in BINS:
            command.extend(["--bin", name])
        with (stage / "build.log").open("xb") as log:
            subprocess.run(command, cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT,
                           check=True, timeout=3600)
            log.flush()
            os.fsync(log.fileno())
        check_no_cargo_config(source, env)
        if before != inputs(source) or head != run(["git", "rev-parse", "HEAD"], source, env):
            raise ValueError("source identity changed during build")
        (stage / "bin").mkdir()
        artifacts = {}
        for name in BINS:
            artifact = stage / "bin" / name
            shutil.copyfile(stage / "build/release" / name, artifact)
            artifact.chmod(0o555)
            artifacts[name] = {"path": f"bin/{name}", "sha256": digest(artifact)}
        (stage / "build.log").chmod(0o444)
        write_new(stage / "manifest.json", {
            "schema": SCHEMA, "created_at_unix_s": time.time(), "source_root": str(source),
            "source_head": head, "source_dirty": bool(run(["git", "status", "--porcelain"], source, env)),
            "inputs": before, "toolchain": toolchain, "command": command, "features": [],
            "target": host, "metadata_command": metadata_command,
            "build_mode": "original_owned_tree_with_before_after_inventory_and_retained_source_copy",
            "artifacts": artifacts,
            "evidence": {name: digest(stage / name) for name in ("build.log", "dependencies.json", "staging-tool.py")},
            "activation_authorized": False,
            "scope": "read_only_tamper_evident_files_not_WORM_or_live_checkpoint_qualification"})
        return verify(stage, digest(stage / "manifest.json"))
    except Exception as error:
        failure = {"error_type": type(error).__name__, "activation_authorized": False}
        if isinstance(error, subprocess.CalledProcessError):
            stderr = error.stderr or ""
            if isinstance(stderr, bytes):
                stderr = stderr.decode("utf-8", errors="replace")
            failure.update({"command": error.cmd, "returncode": error.returncode,
                            "stderr": stderr[:65536], "stderr_truncated": len(stderr) > 65536})
        write_new(stage / "FAILED.json", failure)
        raise


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="operation", required=True)
    create = sub.add_parser("build")
    create.add_argument("--source", type=Path, required=True)
    create.add_argument("--stage", type=Path, required=True)
    check = sub.add_parser("verify")
    check.add_argument("--stage", type=Path, required=True)
    check.add_argument("--manifest-sha256", required=True)
    args = parser.parse_args()
    result = build(args.source, args.stage) if args.operation == "build" else verify(args.stage, args.manifest_sha256)
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
