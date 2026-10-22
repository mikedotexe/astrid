"""Content identities for local bridge artifact and Rust-suite reuse.

Receipts are local build evidence, not signatures, deployment approval, or a
substitute for fresh compatibility and operational checks.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import stat
import subprocess
import tomllib

SCHEMA = "bridge_compiled_inputs_v1"
BINARIES = {
    "spectral-bridge": "spectral-bridge-server",
    "source-study-reader": "astrid-source-study",
    "public-web-helper": "astrid-public-web",
}


def build_commands(source: Path, target: str, target_dir: Path) -> list[list[str]]:
    commands = []
    for name, binary in BINARIES.items():
        manifest = source / ("capsules/spectral-bridge/Cargo.toml" if name == "spectral-bridge" else "Cargo.toml")
        package = [] if name == "spectral-bridge" else ["-p", binary]
        commands.append(["cargo", "build", "--release", "--locked", "--offline", "--manifest-path",
                         str(manifest), *package, "--bin", binary, "--target", target, "--target-dir", str(target_dir)])
    return commands


def digest(value: object) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def file_hash(path: Path) -> str:
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode):
        raise ValueError(f"non-regular reuse input: {path}")
    with path.open("rb") as handle:
        result = hashlib.file_digest(handle, "sha256").hexdigest()
    after = path.lstat()
    if (before.st_ino, before.st_size, before.st_mtime_ns, before.st_mode) != (
            after.st_ino, after.st_size, after.st_mtime_ns, after.st_mode):
        raise ValueError(f"reuse input changed while hashing: {path}")
    return result


def build_environment() -> dict[str, str]:
    # These shell bookkeeping values are removed from the actual child, not
    # merely ignored in the receipt. All remaining values bind reuse, hashed.
    return {key: value for key, value in os.environ.items() if key not in {
        "_", "SHLVL", "PWD", "OLDPWD", "ASTRID_SANCTIONED_BRIDGE_STAGE"}}


def output(args: list[str], source: Path, environment: dict[str, str]) -> str:
    return subprocess.run(args, cwd=source, env=environment, check=True,
                          capture_output=True, text=True, timeout=120).stdout.strip()


def executable(name: str, environment: dict[str, str]) -> dict:
    found = shutil.which(name, path=environment.get("PATH"))
    if not found:
        raise ValueError(f"reuse tool unavailable: {name}")
    path = Path(found).resolve(strict=True)
    return {"path": str(path), "sha256": file_hash(path)}


def dependency_tree(path: Path) -> dict:
    rows = []
    for root, directories, names in os.walk(path):
        directories[:] = sorted(name for name in directories if name not in {".git", "target"})
        if any((Path(root) / name).is_symlink() for name in directories):
            raise ValueError(f"symlinked dependency input below {root}")
        for name in sorted(names):
            item = Path(root) / name
            rows.append([str(item.relative_to(path)), file_hash(item), stat.S_IMODE(item.stat().st_mode)])
            if len(rows) > 50_000:
                raise ValueError("dependency input inventory exceeds bound")
    return {"path": str(path), "sha256": digest(rows), "file_count": len(rows)}


def compiler_context(source: Path, target: str, environment: dict[str, str]) -> dict:
    """Bind resolved third-party bytes as well as lockfiles and tool versions."""
    dependencies = {}
    local = set()
    for manifest, names in (
        (source / "capsules/spectral-bridge/Cargo.toml", {"spectral-bridge-server"}),
        (source / "Cargo.toml", {"astrid-source-study", "astrid-public-web"}),
    ):
        metadata = json.loads(output([
            "cargo", "metadata", "--manifest-path", str(manifest), "--locked", "--offline",
            "--filter-platform", target, "--format-version", "1"], source, environment))
        packages = {item["id"]: item for item in metadata["packages"]}
        nodes = {item["id"]: item for item in metadata["resolve"]["nodes"]}
        roots = [key for key, value in packages.items() if value["name"] in names and value["source"] is None]
        if {packages[key]["name"] for key in roots} != names:
            raise ValueError("compiled dependency roots unavailable")
        pending, seen = roots[:], set()
        while pending:
            key = pending.pop()
            if key in seen:
                continue
            seen.add(key)
            pending.extend(nodes[key]["dependencies"])
        for key in seen:
            item = packages[key]
            if item["source"] is not None:
                dependencies[str(Path(item["manifest_path"]).resolve().parent)] = item["source"]
            else:
                local.add(str(Path(item["manifest_path"]).resolve().parent))
    tools = {name: executable(name, environment) for name in ("cargo", "rustc", "cc", "ar")}
    versions = {name: output(args, source, environment) for name, args in {
        "rustc": ["rustc", "-Vv"], "cargo": ["cargo", "-V"], "cc": ["cc", "--version"]}.items()}
    if platform.system() == "Darwin":
        for name in ("clang", "ld"):
            path = Path(output(["xcrun", "--find", name], source, environment)).resolve(strict=True)
            tools[name] = {"path": str(path), "sha256": file_hash(path)}
        sdk = Path(output(["xcrun", "--show-sdk-path"], source, environment)).resolve(strict=True)
        tools["sdk"] = {"path": str(sdk), "settings_sha256": file_hash(sdk / "SDKSettings.json")}
    # Preserve ordinary builds with custom native toolchains, but do not claim
    # the standard-tool fingerprint covers an arbitrary wrapper or search path.
    native_keys = {"CC", "CXX", "AR", "LD", "RANLIB", "CFLAGS", "CXXFLAGS", "CPPFLAGS", "LDFLAGS",
                   "CPATH", "C_INCLUDE_PATH", "CPLUS_INCLUDE_PATH", "LIBRARY_PATH", "PKG_CONFIG_PATH",
                   "PKG_CONFIG_LIBDIR", "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER",
                   "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"}
    overrides = sorted(key for key, value in environment.items() if value and (
        key in native_keys or key.startswith(("CC_", "CXX_", "AR_", "CFLAGS_", "CXXFLAGS_"))
        or (key.startswith("CARGO_TARGET_") and key.endswith(("_LINKER", "_RUNNER", "_RUSTFLAGS")))))
    return {"environment_sha256": digest(environment), "tools": tools, "versions": versions,
            "unqualified_native_overrides": overrides,
            "local_packages": sorted(local),
            "platform": platform.platform(), "host": platform.node(), "dependencies": [
                {"source": origin, **dependency_tree(Path(path))} for path, origin in sorted(dependencies.items())]}


def compiled_inputs(source: Path, snapshot: dict, target: str, context: dict,
                    tool_paths: set[str]) -> dict:
    packages = [Path(path) for path in snapshot["packages"]]
    if not set(context.get("local_packages", [])).issubset(snapshot["packages"]):
        raise ValueError("compiled local dependency missing from source inventory")
    # Runtime/deployment tools are not compiler inputs. A tool located inside a
    # Cargo package remains conservatively included.
    files = [{key: value for key, value in row.items() if key != "mtime_ns"}
             for row in snapshot["files"]
             if row["path"] not in tool_paths or any(Path(row["path"]).is_relative_to(p) for p in packages)]
    custom_build = any(
        (p / "build.rs").exists()
        or tomllib.loads((p / "Cargo.toml").read_text()).get("package", {}).get("build") not in (None, False)
        for p in packages)
    limitations = ["native override: " + key for key in context.get("unqualified_native_overrides", [])]
    for row in files:
        path = Path(row["path"])
        cargo_home = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
        if path.name in {"config", "config.toml"} and (path.parent.name == ".cargo" or path.parent == cargo_home):
            config = tomllib.loads(path.read_text())
            build = config.get("build", {})
            if (config.get("env") or any(key in build for key in ("rustc", "rustc-wrapper", "rustc-workspace-wrapper", "rustflags"))
                    or any(any(key in value for key in ("linker", "runner", "rustflags"))
                           for selector, value in config.get("target", {}).items()
                           if isinstance(value, dict) and (selector == target or selector.startswith("cfg(")))):
                limitations.append("custom Cargo tool/environment configuration: " + str(path))
    return {"schema": SCHEMA, "source_root": str(source), "packages": snapshot["packages"],
            "files": files, "target": target, "context": context,
            "reuse_limitations": limitations,
            "local_build_script_head": snapshot["head"] if custom_build else None,
            "recipe": {"version": 1, "profile": "release", "locked": True, "offline": True,
                       "features": "default", "binaries": BINARIES,
                       "commands": build_commands(source, target, Path("<owned-build-output>"))},
            "implementation_sha256": file_hash(Path(__file__))}


def matching_inputs(expected: dict, current: dict) -> None:
    if current.get("reuse_limitations"):
        raise ValueError("reuse not qualified for " + "; ".join(current["reuse_limitations"]))
    if expected.get("schema") != SCHEMA or expected != current:
        changed = sorted(key for key in expected.keys() | current.keys() if expected.get(key) != current.get(key))
        raise ValueError("compiled inputs do not match; changed sections: " + ", ".join(changed))
