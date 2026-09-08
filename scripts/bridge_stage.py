#!/usr/bin/env python3
"""Stage a checked bridge build without touching live artifacts or services.

Only build_bridge.sh enables the build command. Verification is read-only.
This is a build witness, not approval to activate, drain, or migrate a process.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess

from environment_receipts import write_build_manifest

SCHEMA = "bridge_staged_release_v2"
AUTHORITY = "build_manifest_witness_not_deploy_authority"
EXCLUDED = {".git", ".runtime", "target", "workspace", "node_modules", ".venv", "__pycache__", ".DS_Store"}
HELPERS = {"substrate-probe-v2": "substrate_probe_v2.py", "release-launcher": "launchd_spectral_bridge.sh", "release-selection": "bridge_release_launch.py"}
TOOLS = ("scripts/build_bridge.sh", "scripts/bridge_stage.py", "scripts/bridge_activate.py", "scripts/bridge_drain.py",
         "scripts/environment_receipts.py", "scripts/deploy_preflight.py", "scripts/steward_mutex.py",
         "scripts/capture_stack_receipt.sh", "scripts/minime_runtime_binding.py",
         *(f"scripts/{name}" for name in HELPERS.values()))
ARTIFACTS = {"spectral-bridge": "spectral-bridge-server", **{name:f"helpers/{path}" for name,path in HELPERS.items()}}


def sha(path: Path) -> str:
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def encoded(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, indent=2) + "\n").encode()


def atomic_json(path: Path, value: object) -> None:
    temporary = path.with_name(path.name + ".pending")
    with temporary.open("xb") as handle:
        os.chmod(temporary, 0o600)
        handle.write(encoded(value))
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temporary, path)
    descriptor = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def json_file(path: Path) -> dict:
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 1_048_576:
        raise ValueError(f"missing, symlinked or oversized record: {path}")
    value = json.loads(path.read_bytes())
    if not isinstance(value, dict):
        raise ValueError(f"record is not an object: {path}")
    return value


def command(args: list[str], *, cwd: Path, timeout: int = 120) -> str:
    return subprocess.run(args, cwd=cwd, capture_output=True, text=True,
                          check=True, timeout=timeout).stdout.strip()


def local_packages(source: Path) -> list[Path]:
    manifest = source / "capsules/spectral-bridge/Cargo.toml"
    metadata = json.loads(command(["cargo", "metadata", "--manifest-path", str(manifest),
                                  "--locked", "--offline", "--filter-platform", host_target(source),
                                  "--format-version", "1"], cwd=source))
    return sorted({Path(item["manifest_path"]).resolve().parent for item in metadata["packages"] if item["source"] is None})


def host_target(source: Path) -> str:
    for line in command(["rustc", "-Vv"], cwd=source).splitlines():
        if line.startswith("host: "):
            return line.removeprefix("host: ").strip()
    raise ValueError("rustc did not report a native host target")


def input_snapshot(source: Path, packages: list[Path]) -> dict:
    files = {source / name for name in TOOLS}
    # Retained releases predating this operator helper have no such input.
    stopped_recovery = source / "scripts/bridge_stopped_recovery.py"
    if stopped_recovery.exists():
        files.add(stopped_recovery)
    for package in packages:
        for root, directories, names in os.walk(package):
            directories[:] = sorted(name for name in directories if name not in EXCLUDED)
            for name in directories:
                if (Path(root) / name).is_symlink():
                    raise ValueError(f"unreviewed symlink in package inputs: {Path(root) / name}")
            files.update(Path(root) / name for name in names if name not in EXCLUDED)
    # Cargo can inherit configuration from ancestor directories and CARGO_HOME.
    for parent in [source / "capsules/spectral-bridge", *(source / "capsules/spectral-bridge").parents]:
        for name in ("config", "config.toml"):
            path = parent / ".cargo" / name
            if path.exists():
                files.add(path)
    for name in ("config", "config.toml"):
        path = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo"))) / name
        if path.exists():
            files.add(path)
    if len(files) > 20_000:
        raise ValueError("package input inventory exceeded the reviewed bound")
    rows = []
    for path in sorted(files):
        before = path.lstat()
        if not stat.S_ISREG(before.st_mode):
            raise ValueError(f"non-regular input: {path}")
        digest = sha(path)
        after = path.stat()
        if (before.st_size, before.st_mtime_ns, before.st_ino) != (after.st_size, after.st_mtime_ns, after.st_ino):
            raise ValueError(f"input changed while hashing: {path}")
        rows.append({"path": str(path), "sha256": digest, "mode": stat.S_IMODE(after.st_mode),
                     "size": after.st_size, "mtime_ns": after.st_mtime_ns})
    return {"schema": "bridge_build_inputs_v1", "head": command(["git", "rev-parse", "HEAD"], cwd=source),
            "packages": [str(path) for path in packages], "files": rows,
            "rustc": command(["rustc", "-Vv"], cwd=source), "cargo": command(["cargo", "-V"], cwd=source),
            "scope": "local_package_trees_and_named_helpers_excluding_runtime_and_build_outputs"}


def same_input_contents(expected: dict, current: dict,
                        allowed_changed_paths: frozenset[str] = frozenset()) -> bool:
    """Compare build-affecting identity while retaining mtimes as stage evidence."""
    if {key:value for key, value in expected.items() if key != "files"} != {
            key:value for key, value in current.items() if key != "files"}:
        return False

    def material_rows(snapshot: dict) -> dict[str, dict]:
        files = snapshot.get("files")
        if not isinstance(files, list):
            return {}
        rows = [{key:value for key, value in row.items() if key != "mtime_ns"}
                for row in files if isinstance(row, dict) and isinstance(row.get("path"), str)]
        return {row["path"]:row for row in rows}

    expected_rows = material_rows(expected)
    current_rows = material_rows(current)
    if (len(expected_rows) != len(expected.get("files", []))
            or len(current_rows) != len(current.get("files", []))
            or expected_rows.keys() != current_rows.keys()):
        return False
    return all(
        expected_rows[path] == current_rows[path]
        or (path in allowed_changed_paths
            and expected_rows[path].get("mode") == current_rows[path].get("mode"))
        for path in expected_rows
    )


def check_environment() -> None:
    blocked = [key for key in os.environ if key in {
        "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTC", "RUSTDOCFLAGS"
    } and os.environ[key]]
    if blocked:
        raise ValueError("unreviewed build overrides: " + ", ".join(sorted(blocked)))


def verify_stage(stage: Path, *, run_binary: bool = True) -> dict:
    stage = stage.resolve(strict=True)
    if (stage / "failure.json").exists():
        raise ValueError("stage has a retained failure record; rebuild in a new directory")
    ready = json_file(stage / "ready.json")
    if ready.get("schema") not in {SCHEMA, "bridge_staged_release_v1"} or ready.get("status") != "staged_verified_not_activated":
        raise ValueError("stage has no completed build witness")
    manifest_path = stage / "manifest.json"
    if sha(manifest_path) != ready.get("manifest_sha256"):
        raise ValueError("staged manifest changed")
    manifest = json_file(manifest_path)
    source_path = stage / "source-inputs.json"
    if source_path.is_symlink() or not source_path.is_file() or sha(source_path) != ready.get("source_inputs_sha256"):
        raise ValueError("source input witness changed")
    if (ready.get("stage") != str(stage) or ready.get("authority") != AUTHORITY
            or manifest.get("schema") != "stack_build_manifest_v1" or manifest.get("authority") != AUTHORITY
            or ready.get("activation_performed") is not False
            or ready.get("live_authority_granted") is not False or ready.get("live_eligible_now") is not False
            or manifest.get("repository", {}).get("source_identity_sha256") != ready.get("source_inputs_sha256")):
        raise ValueError("stage identity or authority boundary mismatch")
    source_ref = manifest.get("source_inputs", {})
    if source_ref.get("path") != str(source_path) or source_ref.get("sha256") != ready.get("source_inputs_sha256"):
        raise ValueError("manifest source reference mismatch")
    expected_artifacts = ARTIFACTS if ready["schema"] == SCHEMA else {name:ARTIFACTS[name] for name in ("spectral-bridge", "substrate-probe-v2")}
    if set(manifest.get("artifacts", {})) != set(expected_artifacts):
        raise ValueError("unexpected staged artifact set")
    for name, relative in expected_artifacts.items():
        expected = stage / relative
        item = manifest["artifacts"][name]
        if expected.is_symlink() or not expected.is_file() or Path(item["path"]) != expected:
            raise ValueError(f"staged artifact path mismatch: {name}")
        if expected.resolve() != expected or sha(expected) != item["sha256"]:
            raise ValueError(f"staged artifact changed: {name}")
    if manifest["artifacts"]["spectral-bridge"]["sha256"] != ready.get("binary_sha256"):
        raise ValueError("stage binary witness mismatch")
    if run_binary:
        result = json.loads(command([str(stage / ARTIFACTS["spectral-bridge"]),
                                     "--deployment-manifest", str(manifest_path), "--verify-deployment-manifest"], cwd=stage, timeout=60))
        if (result.get("verified") is not True or result.get("manifest_sha256") != ready["manifest_sha256"]
                or result.get("binary_sha256") != ready["binary_sha256"]
                or result.get("authority") != AUTHORITY or result.get("live_authority_granted") is not False
                or result.get("live_eligible_now") is not False):
            raise ValueError("native manifest verification did not match the staged record")
    return ready


def stage_build(source: Path, stage: Path, actor: str, ack: str) -> dict:
    source = source.resolve(strict=True)
    stage = stage.absolute()
    check_environment()
    if not actor.strip() or not ack.strip():
        raise ValueError("staging requires an actor and explicit acknowledgement")
    if stage.exists() or stage.is_symlink():
        raise ValueError("stage must be a new directory; existing evidence is never overwritten")
    stage.parent.mkdir(parents=True, exist_ok=True)
    if stage.parent.resolve() != stage.parent:
        raise ValueError("stage parent must use its canonical path")
    stage.mkdir(mode=0o700)
    try:
        packages = local_packages(source)
        if any(stage.is_relative_to(package) for package in packages):
            raise ValueError("stage cannot be inside a package input tree")
        before = input_snapshot(source, packages)
        target = host_target(source)
        atomic_json(stage / "source-inputs.json", before)
        build = ["cargo", "build", "--release", "--locked", "--offline", "--manifest-path",
                 str(source / "capsules/spectral-bridge/Cargo.toml"), "--bin", "spectral-bridge-server",
                 "--target", target, "--target-dir", str(stage / "build")]
        with (stage / "build.log").open("xb") as log:
            subprocess.run(build, cwd=source, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=3600)
        if packages != local_packages(source) or before != input_snapshot(source, packages):
            raise ValueError("source or build environment changed during compilation; stage refused")
        binary = stage / ARTIFACTS["spectral-bridge"]
        shutil.copyfile(stage / "build" / target / "release/spectral-bridge-server", binary)
        binary.chmod(0o555)
        (stage / "helpers").mkdir(mode=0o700)
        for name, filename in HELPERS.items():
            helper = stage / ARTIFACTS[name]
            original = source / "scripts" / filename
            shutil.copyfile(original, helper)
            helper.chmod(0o444)
            expected = next(row["sha256"] for row in before["files"] if row["path"] == str(original))
            if sha(helper) != expected:
                raise ValueError(f"runtime helper changed during staging: {name}")
        for artifact in [stage / path for path in ARTIFACTS.values()]:
            with artifact.open("rb") as handle:
                os.fsync(handle.fileno())
        manifest_path = stage / "manifest.json"
        manifest = write_build_manifest(manifest_path, component="spectral-bridge", repository=source,
                                        artifacts={name: stage / relative for name, relative in ARTIFACTS.items()},
                                        actor=actor, command=" ".join(build))
        source_hash = sha(stage / "source-inputs.json")
        if manifest["repository"]["head"] != before["head"]:
            raise ValueError("repository HEAD changed during staging")
        manifest["repository"]["source_identity_sha256"] = source_hash
        manifest["source_inputs"] = {"path": str(stage / "source-inputs.json"), "sha256": source_hash,
                                     "file_count": len(before["files"]), "scope": before["scope"]}
        atomic_json(manifest_path, manifest)
        result = json.loads(command([str(binary), "--deployment-manifest", str(manifest_path),
                                     "--verify-deployment-manifest"], cwd=stage, timeout=60))
        if result.get("verified") is not True or result.get("manifest_sha256") != sha(manifest_path):
            raise ValueError("native executable did not verify its staged identity")
        ready = {"schema": SCHEMA, "status": "staged_verified_not_activated", "stage": str(stage),
                 "source_root": str(source), "native_target": target, "source_inputs_sha256": source_hash,
                 "manifest_sha256": sha(manifest_path), "binary_sha256": sha(binary),
                 "prepared_at": dt.datetime.now(dt.UTC).isoformat(), "actor": actor,
                 "acknowledgement_sha256": hashlib.sha256(ack.encode()).hexdigest(),
                 "authority": "build_manifest_witness_not_deploy_authority",
                 "live_authority_granted": False, "live_eligible_now": False, "activation_performed": False}
        atomic_json(stage / "ready.json", ready)
        return verify_stage(stage)
    except Exception as error:
        atomic_json(stage / "failure.json", {"schema": SCHEMA, "status": "failed_not_activated",
                    "error": str(error)[:2000], "activation_performed": False})
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    build = sub.add_parser("build")
    build.add_argument("--source-root", type=Path, required=True)
    build.add_argument("--stage-dir", type=Path, required=True)
    build.add_argument("--actor", required=True)
    build.add_argument("--ack", required=True)
    verify = sub.add_parser("verify")
    verify.add_argument("--stage-dir", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "build":
            if os.environ.get("ASTRID_SANCTIONED_BRIDGE_STAGE") != "1":
                raise ValueError("build through scripts/build_bridge.sh --stage-dir")
            result = stage_build(args.source_root, args.stage_dir, args.actor, args.ack)
        else:
            result = verify_stage(args.stage_dir)
        print(json.dumps({"ok": True, **result}, indent=2))
        return 0
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(json.dumps({"ok": False, "error": str(error)[:2000], "activation_performed": False}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
