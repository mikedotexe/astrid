#!/usr/bin/env python3
"""Rebuild pinned external Astralis capsules for CPU-edge Astrid.

The upstream capsules are external repositories. This runner makes the local
compatibility layer reviewable: it checks every patch and lockfile hash, checks
out exact commits, replays patches with ``git apply --recount``, reproduces the
historical rustfmt boundaries, and refuses a source tree whose final Git blob
IDs differ from the reviewed recipe.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


MINIMUM_PYTHON = (3, 11)


def require_supported_python(version_info: Any = sys.version_info) -> None:
    """Fail before importing ``tomllib`` on appliance Python 3.10."""

    if tuple(version_info[:2]) < MINIMUM_PYTHON:
        raise SystemExit(
            "build_astralis_cpu_edge_capsules.py is an operator-side builder "
            "and requires Python 3.11 or newer; it is never invoked by the "
            "Python 3.10 appliance runtime"
        )


require_supported_python()
import tomllib  # noqa: E402 - guarded standard-library dependency


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SPEC = REPO_ROOT / "packaging/headless/astralis-cpu-edge-capsules.toml"
DEFAULT_BASELINE_SPEC = (
    REPO_ROOT / "packaging/headless/astralis-cpu-edge-baseline-capsules.toml"
)
HEX_40 = re.compile(r"[0-9a-f]{40}\Z")
HEX_64 = re.compile(r"[0-9a-f]{64}\Z")
SDK_PACKAGES = ("astrid-sdk", "astrid-sdk-macros", "astrid-sys")


class BuildError(RuntimeError):
    """A fail-closed compatibility build error."""


@dataclass(frozen=True)
class Recipe:
    """One validated external-capsule recipe."""

    raw: dict[str, Any]

    @property
    def capsule_id(self) -> str:
        return str(self.raw["id"])

    @property
    def package(self) -> str:
        return str(self.raw["package"])


def sha256_file(path: Path) -> str:
    """Return the SHA-256 digest of one regular file."""

    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_blob_id(path: Path) -> str:
    """Compute the Git SHA-1 blob identifier for a file without invoking Git."""

    data = path.read_bytes()
    return hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()


def checked_repo_path(repo_root: Path, relative: str) -> Path:
    """Resolve an allowlisted repository-relative regular file."""

    candidate = Path(relative)
    if candidate.is_absolute() or ".." in candidate.parts:
        raise BuildError(f"repository input is not a safe relative path: {relative!r}")
    root = repo_root.resolve()
    unresolved = root / candidate
    if unresolved.is_symlink():
        raise BuildError(f"repository input must not be a symlink: {relative!r}")
    resolved = unresolved.resolve()
    if not resolved.is_relative_to(root):
        raise BuildError(f"repository input escapes the checkout: {relative!r}")
    if not resolved.is_file() or resolved.is_symlink():
        raise BuildError(f"repository input is not a regular non-symlink file: {relative!r}")
    return resolved


def load_and_verify_spec(spec_path: Path, repo_root: Path = REPO_ROOT) -> tuple[dict[str, Any], list[Recipe]]:
    """Parse and statically verify the complete pinned recipe."""

    try:
        spec = tomllib.loads(spec_path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise BuildError(f"cannot read capsule recipe {spec_path}: {error}") from error

    if spec.get("schema_version") != 1:
        raise BuildError("capsule recipe schema_version must be 1")
    if spec.get("rust_toolchain") != "1.94.1":
        raise BuildError("capsule recipe must pin rust_toolchain 1.94.1")
    if spec.get("rust_target") != "wasm32-wasip2":
        raise BuildError("capsule recipe must target wasm32-wasip2")
    source_policy = spec.get("source_policy")
    if source_policy not in {
        "reviewed_patch_replay",
        "exact_upstream_snapshot",
        "pinned_upstream_patch_replay",
    }:
        raise BuildError("capsule recipe has an unsupported source_policy")
    sdk_version = spec.get("sdk_version")
    if sdk_version not in {"0.6.0", "0.7.1"}:
        raise BuildError("capsule recipe must pin an approved exact SDK version")

    raw_capsules = spec.get("capsule")
    if not isinstance(raw_capsules, list) or not raw_capsules:
        raise BuildError("capsule recipe has no capsules")

    recipes: list[Recipe] = []
    seen_ids: set[str] = set()
    seen_packages: set[str] = set()
    for raw in raw_capsules:
        if not isinstance(raw, dict):
            raise BuildError("each capsule recipe must be a table")
        recipe = Recipe(raw)
        capsule_id = recipe.capsule_id
        package = recipe.package
        if not re.fullmatch(r"[a-z0-9][a-z0-9-]*", capsule_id):
            raise BuildError(f"invalid capsule id: {capsule_id!r}")
        if capsule_id in seen_ids or package in seen_packages:
            raise BuildError(f"duplicate capsule id or package: {capsule_id!r}")
        seen_ids.add(capsule_id)
        seen_packages.add(package)

        repository = raw.get("repository")
        if not isinstance(repository, str) or not repository.startswith(
            "https://github.com/unicity-astrid/"
        ):
            raise BuildError(f"{capsule_id}: repository is not an approved upstream URL")
        revision = raw.get("revision")
        if not isinstance(revision, str) or not HEX_40.fullmatch(revision):
            raise BuildError(f"{capsule_id}: revision must be a full 40-hex commit")

        for key in (
            "source_blob_cargo_toml",
            "source_blob_capsule_toml",
            "source_blob_lib_rs",
        ):
            value = raw.get(key)
            if not isinstance(value, str) or not HEX_40.fullmatch(value):
                raise BuildError(f"{capsule_id}: {key} must be a Git blob id")
        tests_blob = raw.get("source_blob_tests_rs")
        if tests_blob is not None and (
            not isinstance(tests_blob, str) or not HEX_40.fullmatch(tests_blob)
        ):
            raise BuildError(f"{capsule_id}: source_blob_tests_rs must be a Git blob id")

        expected_lock_hash = raw.get("lock_sha256")
        if not isinstance(expected_lock_hash, str) or not HEX_64.fullmatch(expected_lock_hash):
            raise BuildError(f"{capsule_id}: invalid lock_sha256")
        if source_policy == "reviewed_patch_replay":
            lock = checked_repo_path(repo_root, str(raw.get("lockfile", "")))
            if sha256_file(lock) != expected_lock_hash:
                raise BuildError(f"{capsule_id}: pinned Cargo.lock hash mismatch")
            verify_sdk_lock(lock, str(sdk_version), capsule_id)
            if "source_blob_cargo_lock" in raw:
                raise BuildError(
                    f"{capsule_id}: patch replay must use its tracked reviewed lockfile"
                )
        else:
            if "lockfile" in raw:
                raise BuildError(
                    f"{capsule_id}: exact upstream snapshot cannot replace Cargo.lock"
                )
            lock_blob = raw.get("source_blob_cargo_lock")
            if not isinstance(lock_blob, str) or not HEX_40.fullmatch(lock_blob):
                raise BuildError(
                    f"{capsule_id}: source_blob_cargo_lock must be a Git blob id"
                )

        steps = raw.get("steps")
        if not isinstance(steps, list):
            raise BuildError(f"{capsule_id}: patch sequence is not a list")
        if source_policy == "reviewed_patch_replay" and not steps:
            raise BuildError(f"{capsule_id}: patch sequence is empty")
        if source_policy == "exact_upstream_snapshot" and steps:
            raise BuildError(f"{capsule_id}: exact upstream snapshot cannot apply patches")
        for step in steps:
            if not isinstance(step, dict) or step.get("kind") not in {"patch", "rustfmt"}:
                raise BuildError(f"{capsule_id}: invalid build step")
            if step["kind"] == "rustfmt":
                if set(step) - {"kind", "max_width"}:
                    raise BuildError(f"{capsule_id}: unsupported rustfmt setting")
                width = step.get("max_width")
                if width is not None and width not in {80, 100}:
                    raise BuildError(f"{capsule_id}: unsupported rustfmt max_width")
                continue
            patch_path = checked_repo_path(repo_root, str(step.get("file", "")))
            expected_patch_hash = step.get("sha256")
            if not isinstance(expected_patch_hash, str) or not HEX_64.fullmatch(
                expected_patch_hash
            ):
                raise BuildError(f"{capsule_id}: patch has no valid SHA-256")
            if sha256_file(patch_path) != expected_patch_hash:
                raise BuildError(f"{capsule_id}: patch hash mismatch: {step['file']}")
            try:
                parsed_patch = subprocess.run(
                    ["git", "apply", "--numstat", "--recount", str(patch_path)],
                    check=True,
                    text=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                )
            except (OSError, subprocess.CalledProcessError) as error:
                raise BuildError(f"{capsule_id}: malformed patch: {step['file']}") from error
            if not parsed_patch.stdout.strip():
                raise BuildError(f"{capsule_id}: patch contains no file changes: {step['file']}")
            for key in ("expect_before", "expect_after"):
                expected_blob = step.get(key)
                if expected_blob is not None and (
                    not isinstance(expected_blob, str) or not HEX_40.fullmatch(expected_blob)
                ):
                    raise BuildError(f"{capsule_id}: {key} is not a Git blob id")
            if ("expect_before" in step or "expect_after" in step) and not step.get(
                "expect_path"
            ):
                raise BuildError(f"{capsule_id}: blob expectation lacks expect_path")
        recipes.append(recipe)

    if source_policy == "reviewed_patch_replay":
        verify_react_provenance_route(recipes)
    return spec, recipes


def verify_sdk_lock(lock_path: Path, sdk_version: str, capsule_id: str) -> None:
    """Require exactly one copy of each ABI-sensitive SDK package."""

    try:
        lock = tomllib.loads(lock_path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise BuildError(f"{capsule_id}: invalid Cargo.lock: {error}") from error
    packages = lock.get("package", [])
    for package_name in SDK_PACKAGES:
        versions = [
            package.get("version")
            for package in packages
            if package.get("name") == package_name
        ]
        if versions != [sdk_version]:
            raise BuildError(
                f"{capsule_id}: {package_name} must occur once at {sdk_version}, got {versions}"
            )


def verify_react_provenance_route(recipes: Iterable[Recipe]) -> None:
    """Keep the provenance patch behind its reviewed byte-exact preimage."""

    react = next((recipe for recipe in recipes if recipe.capsule_id == "react"), None)
    if react is None:
        raise BuildError("recipe must include the React compatibility capsule")
    patch_steps = [step for step in react.raw["steps"] if step["kind"] == "patch"]
    terminal = [
        step
        for step in patch_steps
        if step["file"].endswith("astralis-sdk-0.6-react-terminal-next.patch")
    ]
    if len(terminal) != 1:
        raise BuildError("React recipe must apply the terminal provenance patch exactly once")
    if terminal[0].get("expect_before") != "990e702bbf2cca49191d3d10ed040c50bb9c9dbc":
        raise BuildError("React terminal provenance preimage is not byte-exact")
    if terminal[0].get("expect_after") != react.raw["source_blob_lib_rs"]:
        raise BuildError("React terminal provenance postimage is not the reviewed source blob")


def run(command: list[str], *, cwd: Path | None = None, env: dict[str, str] | None = None) -> str:
    """Run one subprocess and return stripped stdout."""

    printable = " ".join(command)
    print(f"+ {printable}", flush=True)
    try:
        result = subprocess.run(
            command,
            cwd=cwd,
            env=env,
            check=True,
            text=True,
            stdout=subprocess.PIPE,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise BuildError(f"command failed: {printable}") from error
    if result.stdout:
        print(result.stdout, end="" if result.stdout.endswith("\n") else "\n")
    return result.stdout.strip()


def check_toolchain(expected: str) -> dict[str, str]:
    """Require the active Rust and Cargo release used by the reviewed recipe."""

    rustc = run(["rustc", "--version"])
    cargo = run(["cargo", "--version"])
    verbose_rustc = run(["rustc", "--version", "--verbose"])
    if not rustc.startswith(f"rustc {expected} "):
        raise BuildError(f"rustc must be {expected}, got {rustc!r}")
    if not cargo.startswith(f"cargo {expected} "):
        raise BuildError(f"cargo must be {expected}, got {cargo!r}")
    host_lines = [line.removeprefix("host: ") for line in verbose_rustc.splitlines() if line.startswith("host: ")]
    if len(host_lines) != 1:
        raise BuildError("rustc --version --verbose did not report one host target")
    return {"rustc": rustc, "cargo": cargo, "host": host_lines[0]}


def check_blob(source: Path, relative: str, expected: str, label: str) -> None:
    """Check one source file's reviewed Git blob ID."""

    path = source / relative
    if not path.is_file() or path.is_symlink():
        raise BuildError(f"{label}: expected regular source file {relative}")
    actual = git_blob_id(path)
    if actual != expected:
        raise BuildError(f"{label}: {relative} blob mismatch: {actual} != {expected}")


def apply_steps(recipe: Recipe, source: Path, repo_root: Path) -> None:
    """Apply the reviewed patch and formatting sequence."""

    manifest_path = source / "Cargo.toml"
    for step in recipe.raw["steps"]:
        if step["kind"] == "rustfmt":
            command = ["cargo", "fmt", "--manifest-path", str(manifest_path)]
            if "max_width" in step:
                command.extend(["--", "--config", f"max_width={step['max_width']}"])
            run(command, cwd=source)
            continue

        expected_path = step.get("expect_path")
        if expected_path and "expect_before" in step:
            check_blob(
                source,
                str(expected_path),
                str(step["expect_before"]),
                f"{recipe.capsule_id} patch preimage",
            )
        patch_path = checked_repo_path(repo_root, str(step["file"]))
        run(["git", "apply", "--check", "--recount", str(patch_path)], cwd=source)
        run(["git", "apply", "--recount", str(patch_path)], cwd=source)
        if expected_path and "expect_after" in step:
            check_blob(
                source,
                str(expected_path),
                str(step["expect_after"]),
                f"{recipe.capsule_id} patch postimage",
            )


def clone_pinned_source(recipe: Recipe, destination: Path, source_root: Path | None) -> None:
    """Clone one source without mutating a caller-supplied cache."""

    if destination.exists():
        raise BuildError(f"refusing to reuse build source directory: {destination}")
    origin = (
        str((source_root / recipe.capsule_id).resolve())
        if source_root is not None
        else str(recipe.raw["repository"])
    )
    if source_root is not None and not (source_root / recipe.capsule_id / ".git").is_dir():
        raise BuildError(f"missing cached Git source: {source_root / recipe.capsule_id}")
    run(["git", "clone", "--no-hardlinks", "--no-checkout", origin, str(destination)])
    run(["git", "checkout", "--detach", str(recipe.raw["revision"])], cwd=destination)
    revision = run(["git", "rev-parse", "HEAD"], cwd=destination)
    if revision != recipe.raw["revision"]:
        raise BuildError(f"{recipe.capsule_id}: checked-out revision mismatch")
    if run(["git", "status", "--porcelain"], cwd=destination):
        raise BuildError(f"{recipe.capsule_id}: upstream checkout is unexpectedly dirty")


def cargo_command(
    verb: str,
    source: Path,
    *,
    offline: bool,
    target: str | None = None,
    release: bool = False,
) -> list[str]:
    """Build a strict locked Cargo command."""

    command = ["cargo", verb, "--manifest-path", str(source / "Cargo.toml"), "--locked"]
    if offline:
        command.append("--offline")
    if target:
        command.extend(["--target", target])
    if release:
        command.append("--release")
    return command


def deterministic_capsule(output: Path, capsule_toml: Path, wasm: Path) -> None:
    """Write a byte-stable owner-independent .capsule tar.gz."""

    entries = (("Capsule.toml", capsule_toml.read_bytes()), (wasm.name, wasm.read_bytes()))
    buffer = io.BytesIO()
    with gzip.GzipFile(fileobj=buffer, mode="wb", filename="", mtime=0) as gzip_file:
        with tarfile.open(fileobj=gzip_file, mode="w", format=tarfile.GNU_FORMAT) as archive:
            for name, data in entries:
                info = tarfile.TarInfo(name)
                info.size = len(data)
                info.mode = 0o644
                info.uid = 0
                info.gid = 0
                info.uname = ""
                info.gname = ""
                info.mtime = 0
                archive.addfile(info, io.BytesIO(data))
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(f".{output.name}.tmp-{os.getpid()}")
    temporary.write_bytes(buffer.getvalue())
    os.chmod(temporary, 0o644)
    os.replace(temporary, output)


def export_source_snapshot(source: Path, destination: Path, capsule_id: str) -> list[dict[str, Any]]:
    """Export the exact bounded source needed for later offline self-builds."""

    if destination.exists() or destination.is_symlink():
        raise BuildError(f"refusing to replace external source snapshot: {destination}")
    allowed_top = {"Cargo.toml", "Cargo.lock", "Capsule.toml", "build.rs"}
    allowed_src_suffixes = {".rs", ".md", ".json", ".toml", ".txt"}
    selected: list[Path] = []
    for candidate in sorted(source.rglob("*")):
        relative = candidate.relative_to(source)
        if any(part.startswith(".") or part in {"target", ".git"} for part in relative.parts):
            continue
        if relative.as_posix() in allowed_top or (
            relative.parts[0] == "src" and relative.suffix in allowed_src_suffixes
        ):
            selected.append(relative)
    required = {Path("Cargo.toml"), Path("Cargo.lock"), Path("Capsule.toml"), Path("src/lib.rs")}
    if not required.issubset(selected):
        raise BuildError(f"{capsule_id}: source snapshot lacks required build inputs")
    inventory: list[dict[str, Any]] = []
    for relative in selected:
        source_path = source / relative
        metadata = source_path.lstat()
        if (
            source_path.is_symlink()
            or not stat.S_ISREG(metadata.st_mode)
            or metadata.st_nlink != 1
        ):
            raise BuildError(f"{capsule_id}: source snapshot contains linked/special content")
        destination_path = destination / relative
        destination_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source_path, destination_path)
        os.chmod(destination_path, 0o644)
        inventory.append(
            {
                "path": f"{capsule_id}/{relative.as_posix()}",
                "mode": "0644",
                "size": metadata.st_size,
                "sha256": sha256_file(source_path),
            }
        )
    return inventory


def build_one(
    recipe: Recipe,
    *,
    spec: dict[str, Any],
    repo_root: Path,
    source_root: Path | None,
    work_root: Path,
    output_root: Path,
    offline: bool,
    jobs: int,
    host_target: str,
    source_output_root: Path | None,
) -> dict[str, Any]:
    """Prepare, test, lint, compile, and archive one capsule."""

    source = work_root / recipe.capsule_id
    clone_pinned_source(recipe, source, source_root)
    apply_steps(recipe, source, repo_root)
    run(["cargo", "fmt", "--manifest-path", str(source / "Cargo.toml"), "--", "--check"])

    for relative, key in (
        ("Cargo.toml", "source_blob_cargo_toml"),
        ("Capsule.toml", "source_blob_capsule_toml"),
        ("src/lib.rs", "source_blob_lib_rs"),
    ):
        check_blob(source, relative, str(recipe.raw[key]), recipe.capsule_id)
    if "source_blob_tests_rs" in recipe.raw:
        check_blob(
            source,
            "src/tests.rs",
            str(recipe.raw["source_blob_tests_rs"]),
            recipe.capsule_id,
        )

    if spec["source_policy"] == "reviewed_patch_replay":
        lock_source = checked_repo_path(repo_root, str(recipe.raw["lockfile"]))
        shutil.copyfile(lock_source, source / "Cargo.lock")
    else:
        check_blob(
            source,
            "Cargo.lock",
            str(recipe.raw["source_blob_cargo_lock"]),
            recipe.capsule_id,
        )
        if sha256_file(source / "Cargo.lock") != recipe.raw["lock_sha256"]:
            raise BuildError(f"{recipe.capsule_id}: upstream Cargo.lock hash mismatch")
    verify_sdk_lock(source / "Cargo.lock", str(spec["sdk_version"]), recipe.capsule_id)

    cargo_env = os.environ.copy()
    cargo_env["CARGO_BUILD_JOBS"] = str(jobs)
    run(
        cargo_command("test", source, offline=offline, target=host_target),
        cwd=source,
        env=cargo_env,
    )
    clippy = cargo_command("clippy", source, offline=offline, target=host_target)
    clippy.extend(["--all-targets", "--", "-D", "warnings"])
    run(clippy, cwd=source, env=cargo_env)
    run(
        cargo_command(
            "build",
            source,
            offline=offline,
            target=str(spec["rust_target"]),
            release=True,
        ),
        cwd=source,
        env=cargo_env,
    )

    wasm_name = recipe.package.replace("-", "_") + ".wasm"
    wasm = source / "target" / str(spec["rust_target"]) / "release" / wasm_name
    if not wasm.is_file() or wasm.read_bytes()[:8] != b"\0asm\r\0\1\0":
        raise BuildError(f"{recipe.capsule_id}: build did not produce a WASM component")
    capsule_manifest = tomllib.loads((source / "Capsule.toml").read_text(encoding="utf-8"))
    component_files = [item.get("file") for item in capsule_manifest.get("component", [])]
    if wasm_name not in component_files:
        raise BuildError(
            f"{recipe.capsule_id}: Capsule.toml does not declare built component {wasm_name}"
        )

    archive = output_root / f"{recipe.package}.capsule"
    deterministic_capsule(archive, source / "Capsule.toml", wasm)
    source_inventory = (
        export_source_snapshot(
            source,
            source_output_root / recipe.capsule_id,
            recipe.capsule_id,
        )
        if source_output_root is not None
        else []
    )
    return {
        "id": recipe.capsule_id,
        "package": recipe.package,
        "revision": recipe.raw["revision"],
        "archive": archive.name,
        "archive_sha256": sha256_file(archive),
        "archive_bytes": archive.stat().st_size,
        "wasm_sha256": sha256_file(wasm),
        "wasm_bytes": wasm.stat().st_size,
        "lock_sha256": recipe.raw["lock_sha256"],
        "source_blobs": {
            "Cargo.toml": recipe.raw["source_blob_cargo_toml"],
            "Capsule.toml": recipe.raw["source_blob_capsule_toml"],
            "src/lib.rs": recipe.raw["source_blob_lib_rs"],
            **(
                {"src/tests.rs": recipe.raw["source_blob_tests_rs"]}
                if "source_blob_tests_rs" in recipe.raw
                else {}
            ),
            **(
                {"Cargo.lock": recipe.raw["source_blob_cargo_lock"]}
                if "source_blob_cargo_lock" in recipe.raw
                else {}
            ),
        },
        "patches": [
            {"file": step["file"], "sha256": step["sha256"]}
            for step in recipe.raw["steps"]
            if step["kind"] == "patch"
        ],
        "source_inventory": source_inventory,
    }


def select_recipes(recipes: list[Recipe], selected: list[str]) -> list[Recipe]:
    """Select requested recipes while rejecting misspellings."""

    if not selected:
        return recipes
    by_id = {recipe.capsule_id: recipe for recipe in recipes}
    unknown = sorted(set(selected) - set(by_id))
    if unknown:
        raise BuildError(f"unknown capsule id(s): {', '.join(unknown)}")
    return [by_id[capsule_id] for capsule_id in selected]


def verify_output_surface(output: Path, recipes: list[Recipe]) -> None:
    """Reject stale or unrelated files that a new manifest would fail to cover."""

    if not output.exists():
        return
    if not output.is_dir() or output.is_symlink():
        raise BuildError(f"output path is not a regular directory: {output}")
    allowed = {"MANIFEST.json"} | {
        f"{recipe.package}.capsule" for recipe in recipes
    }
    unexpected = sorted(path.name for path in output.iterdir() if path.name not in allowed)
    if unexpected:
        raise BuildError(
            "output directory contains files outside this build manifest: "
            + ", ".join(unexpected)
        )


def parser() -> argparse.ArgumentParser:
    """Build the command-line parser."""

    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--spec", type=Path, default=DEFAULT_SPEC)
    result.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    result.add_argument("--capsule", action="append", default=[])
    result.add_argument("--source-root", type=Path)
    result.add_argument("--work-dir", type=Path)
    result.add_argument(
        "--source-output-dir",
        type=Path,
        help="export a deterministic, manifest-bound source snapshot for offline self-builds",
    )
    result.add_argument(
        "--output-dir", type=Path, default=REPO_ROOT / "dist/astralis-cpu-edge"
    )
    result.add_argument("--jobs", type=int, default=4)
    result.add_argument("--offline", action="store_true")
    result.add_argument(
        "--verify-only",
        action="store_true",
        help="verify tracked recipes, patches, and lockfiles without network or build work",
    )
    return result


def main(argv: list[str] | None = None) -> int:
    """Run the compatibility pipeline."""

    args = parser().parse_args(argv)
    try:
        repo_root = args.repo_root.resolve()
        spec, recipes = load_and_verify_spec(args.spec.resolve(), repo_root)
        recipes = select_recipes(recipes, args.capsule)
        print(f"verified {len(recipes)} pinned external capsule recipe(s)")
        if args.verify_only:
            return 0
        if args.jobs < 1:
            raise BuildError("--jobs must be positive")
        toolchain = check_toolchain(str(spec["rust_toolchain"]))

        output = args.output_dir.resolve()
        verify_output_surface(output, recipes)
        output.mkdir(parents=True, exist_ok=True)
        source_root = args.source_root.resolve() if args.source_root else None
        source_output_root = (
            args.source_output_dir.resolve() if args.source_output_dir else None
        )
        if source_output_root is not None:
            if source_output_root.exists() or source_output_root.is_symlink():
                raise BuildError("--source-output-dir must not already exist")
            source_output_root.mkdir(parents=True)
        if args.work_dir:
            work_root = args.work_dir.resolve()
            work_root.mkdir(parents=True, exist_ok=True)
            temporary: tempfile.TemporaryDirectory[str] | None = None
        else:
            temporary = tempfile.TemporaryDirectory(prefix="astralis-cpu-edge-")
            work_root = Path(temporary.name)
        try:
            built = [
                build_one(
                    recipe,
                    spec=spec,
                    repo_root=repo_root,
                    source_root=source_root,
                    work_root=work_root,
                    output_root=output,
                    offline=args.offline,
                    jobs=args.jobs,
                    host_target=toolchain["host"],
                    source_output_root=source_output_root,
                )
                for recipe in recipes
            ]
        finally:
            if temporary is not None:
                temporary.cleanup()

        manifest = {
            "schema_version": 1,
            "recipe": str(args.spec.resolve().relative_to(repo_root)),
            "rust_toolchain": toolchain,
            "target": spec["rust_target"],
            "sdk_version": spec["sdk_version"],
            "source_policy": spec["source_policy"],
            "capsules": built,
        }
        manifest_bytes = (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()
        manifest_path = output / "MANIFEST.json"
        temporary_manifest = output / f".MANIFEST.json.tmp-{os.getpid()}"
        temporary_manifest.write_bytes(manifest_bytes)
        os.chmod(temporary_manifest, 0o644)
        os.replace(temporary_manifest, manifest_path)
        if source_output_root is not None:
            source_manifest = {
                "schema": "astrid.cpu_edge.external_capsule_sources.v1",
                "recipe": str(args.spec.resolve().relative_to(repo_root)),
                "rust_toolchain": spec["rust_toolchain"],
                "target": spec["rust_target"],
                "sdk_version": spec["sdk_version"],
                "source_policy": spec["source_policy"],
                "capsules": [
                    {
                        "id": item["id"],
                        "package": item["package"],
                        "revision": item["revision"],
                        "files": item["source_inventory"],
                    }
                    for item in built
                ],
            }
            source_manifest_path = source_output_root / "SOURCE-MANIFEST.json"
            source_manifest_path.write_bytes(
                (json.dumps(source_manifest, indent=2, sort_keys=True) + "\n").encode()
            )
            os.chmod(source_manifest_path, 0o644)
        print(f"wrote {manifest_path}")
        return 0
    except BuildError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
