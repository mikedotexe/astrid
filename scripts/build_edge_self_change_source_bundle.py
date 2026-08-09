#!/usr/bin/env python3
"""Build and verify deterministic offline CPU-edge self-change source bundles.

The bundle is source and evidence only. This program never invokes Cargo, performs dependency
resolution, runs ``cargo vendor``, accesses the network, builds code, or deploys artifacts.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import hmac
import io
import json
import os
import re
import stat
import subprocess
import sys
import tarfile
import tempfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Iterable


MINIMUM_PYTHON = (3, 11)


def require_supported_python(version_info: Any = sys.version_info) -> None:
    """Fail before importing ``tomllib`` on appliance Python 3.10."""

    if tuple(version_info[:2]) < MINIMUM_PYTHON:
        raise SystemExit(
            "build_edge_self_change_source_bundle.py is an operator-side builder "
            "and requires Python 3.11 or newer; it is never invoked by the "
            "Python 3.10 appliance runtime"
        )


require_supported_python()
import tomllib  # noqa: E402 - guarded standard-library dependency

SCHEMA = "astrid.edge.self_change_source_bundle.v1"
SIGNATURE_SCHEMA = "astrid.edge.self_change_source_signature.v1"
SOURCE_ID_SCHEMA = "astrid.edge.self_change_source_identity.v1"
PORTABLE_SOURCE_AUTHORITY = "portable_bootstrap_non_authorizing"
LOCAL_SOURCE_AUTHORITY = "appliance_local_authorizing"
BUNDLE_ROOT = "astrid-edge-self-change-source"
REQUIRED_RUST_RELEASE = "1.94.1"
REQUIRED_RUST_COMMIT = "e408947bfd200af42db322daf0fadfe7e26d3bd1"
REQUIRED_RUST_COMMIT_DATE = "2026-03-25"
REQUIRED_LLVM_VERSION = "21.1.8"
MAX_FILES = 100_000
MAX_UNCOMPRESSED_BYTES = 4 * 1024 * 1024 * 1024
MAX_MANIFEST_BYTES = 32 * 1024 * 1024
MAX_QUICKJS_KERNEL_BYTES = 32 * 1024 * 1024
HEX_40 = re.compile(r"[0-9a-f]{40}\Z")
HEX_64 = re.compile(r"[0-9a-f]{64}\Z")
SAFE_COMPONENT = re.compile(r"[A-Za-z0-9][A-Za-z0-9._+-]*\Z")
SAFE_APPLIANCE_ID = re.compile(r"[a-z0-9][a-z0-9._-]{0,63}\Z")
QUICKJS_KERNEL_PATH = "crates/astrid-openclaw/kernel/engine.wasm"
QUICKJS_KERNEL_HASH_PATH = "crates/astrid-openclaw/kernel/engine.wasm.blake3"

LOCAL_EDGE_CAPSULES = (
    "astrid-capsule-agents",
    "astrid-capsule-cli",
    "astrid-capsule-edge-context",
    "astrid-capsule-edge-introspector",
    "astrid-capsule-edge-spectral",
    "astrid-capsule-fs",
    "astrid-capsule-http",
    "astrid-capsule-memory",
    "astrid-capsule-shell",
    "astrid-capsule-skills",
)
EXTERNAL_EDGE_CAPSULES = (
    "astrid-capsule-context-engine",
    "astrid-capsule-hook-bridge",
    "astrid-capsule-identity",
    "astrid-capsule-openai-compat",
    "astrid-capsule-prompt-builder",
    "astrid-capsule-react",
    "astrid-capsule-registry",
    "astrid-capsule-router",
    "astrid-capsule-session",
    "astrid-capsule-system",
)
EDGE_CAPSULES = LOCAL_EDGE_CAPSULES + EXTERNAL_EDGE_CAPSULES
EXTERNAL_SOURCE_SCHEMA = "astrid.cpu_edge.external_capsule_sources.v1"
EDGE_STANDALONE_SERVICES = (
    "astrid-edge-checkpoint",
    "astrid-edge-presentation-broker",
    "astrid-edge-provider-broker",
    "astrid-edge-rescue-helper",
    "astrid-edge-runtime",
    "astrid-edge-steward-helper",
    "astrid-edge-web-broker",
)
MUTABLE_UNIT_FRAGMENTS = frozenset(
    {
        "ollama-cpu.service",
        "astrid-model-warmup.service",
        "astrid.service",
        "astrid-edge-runtime.service",
        "astrid-edge-hindsight.service",
        "astrid-edge-hindsight.timer",
    }
)
MUTABLE_CORE_CRATES = frozenset(
    {
        "astrid-approval",
        "astrid-audit",
        "astrid-build",
        "astrid-capabilities",
        "astrid-capsule",
        "astrid-cli",
        "astrid-config",
        "astrid-core",
        "astrid-crypto",
        "astrid-daemon",
        "astrid-events",
        "astrid-guest",
        "astrid-hooks",
        "astrid-kernel",
        "astrid-mcp",
        "astrid-minime-protocol",
        "astrid-openclaw",
        "astrid-prelude",
        "astrid-spectral-core",
        "astrid-storage",
        "astrid-telemetry",
        "astrid-types",
        "astrid-integration-tests",
        "astrid-test",
        "astrid-vfs",
        "astrid-workspace",
    }
)
BUILD_FILE_SUFFIXES = frozenset(
    {".rs", ".toml", ".json", ".js", ".mjs", ".wit", ".wat", ".wasm", ".blake3"}
)
PRIVATE_COMPONENTS = frozenset(
    {
        ".ssh",
        "backups",
        "credentials",
        "home",
        "journals",
        "operator-quarantine",
        "private-keys",
        "secrets",
        "state",
        "trusted",
        "workspace",
    }
)
INSPECT_ONLY_SERVICE_PREFIXES = (
    "services/astrid-edge-steward-helper/",
    "services/astrid-edge-rescue-helper/",
    "services/astrid-edge-web-broker/",
    "services/astrid-edge-provider-broker/",
    "services/astrid-edge-presentation-broker/",
    "services/astrid-edge-checkpoint/",
)
INSPECT_ONLY_SCRIPT_NAMES = frozenset(
    {
        "build_edge_self_change_source_bundle.py",
        "build_edge_self_change_supervisor_zipapp.py",
        "build_edge_self_change_toolchain_bundle.py",
        "astrid_train.py",
        "edge_audio_feeder.py",
        "edge_hindsight.py",
        "edge_self_change_supervisor.py",
        "install_edge_self_evolution_root.sh",
        "report_edge_appliance.sh",
        "report_edge_fleet_activity.py",
        "test_build_edge_self_change_source_bundle.py",
        "test_build_edge_self_change_supervisor_zipapp.py",
        "test_build_edge_self_change_toolchain_bundle.py",
        "test_edge_builder_store.py",
        "test_edge_audio_feeder.py",
        "test_edge_state_store.py",
        "test_edge_probation_health_systemd.py",
        "test_edge_self_change_supervisor.py",
        "test_install_edge_self_evolution_root.sh",
    }
)
MUTABLE_LIVE_REPORTS = frozenset(
    {"astrid_at_a_glance.py", "report_edge_activity.py", "report_edge_appliance.py"}
)
BUILD_REQUIRED_REPORT_TESTS = frozenset(
    {
        "test_astrid_train.py",
        "test_edge_hindsight.py",
        "test_report_edge_activity.py",
        "test_report_edge_appliance.py",
    }
)

class BundleError(RuntimeError):
    """A fail-closed source bundle error."""

@dataclass(frozen=True)
class Payload:
    """One immutable archive input."""

    path: str
    origin: str
    mode: int
    sha256: str
    size: int
    source: Path | None = None
    data: bytes | None = None

    def inventory(self) -> dict[str, Any]:
        return {
            "path": self.path,
            "origin": self.origin,
            "mode": f"{self.mode:04o}",
            "size": self.size,
            "sha256": self.sha256,
        }

def canonical_bytes(value: Any) -> bytes:
    """Return the one accepted JSON representation for signed records."""

    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()

def safe_relative_path(value: str) -> PurePosixPath:
    """Validate one portable bundle-relative path."""

    if (
        not value
        or value.startswith("/")
        or "\\" in value
        or "\x00" in value
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        raise BundleError(f"unsafe relative path: {value!r}")
    raw_parts = value.split("/")
    if any(part in {"", ".", ".."} for part in raw_parts):
        raise BundleError(f"unsafe relative path: {value!r}")
    candidate = PurePosixPath(value)
    if any(part in {"", ".", ".."} for part in candidate.parts):
        raise BundleError(f"unsafe relative path: {value!r}")
    return candidate


def read_regular(path: Path, *, limit: int | None = None, owner_only: bool = False) -> bytes:
    """Read one regular, non-linked file once and reject a concurrent replacement."""

    try:
        before = path.lstat()
    except OSError as error:
        raise BundleError(f"cannot stat required file {path}: {error}") from error
    if not stat.S_ISREG(before.st_mode) or path.is_symlink() or before.st_nlink != 1:
        raise BundleError(f"input must be a regular non-symlink, non-hardlinked file: {path}")
    if owner_only and stat.S_IMODE(before.st_mode) & 0o077:
        raise BundleError(f"signing key must be owner-only: {path}")
    if limit is not None and before.st_size > limit:
        raise BundleError(f"input exceeds {limit} bytes: {path}")
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
        with os.fdopen(descriptor, "rb") as handle:
            opened = os.fstat(handle.fileno())
            data = handle.read() if limit is None else handle.read(limit + 1)
            after = os.fstat(handle.fileno())
    except OSError as error:
        raise BundleError(f"cannot read required file {path}: {error}") from error
    if limit is not None and len(data) > limit:
        raise BundleError(f"input exceeds {limit} bytes: {path}")
    stable = (
        before.st_dev,
        before.st_ino,
        before.st_size,
        before.st_mtime_ns,
        opened.st_dev,
        opened.st_ino,
        opened.st_size,
        opened.st_mtime_ns,
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
    )
    if stable[:4] != stable[4:8] or stable[4:8] != stable[8:] or len(data) != before.st_size:
        raise BundleError(f"input changed while it was read: {path}")
    return data


def run_git(repo: Path, arguments: list[str]) -> bytes:
    """Run a read-only local Git query."""

    try:
        completed = subprocess.run(
            ["git", "-C", str(repo), *arguments],
            check=False,
            capture_output=True,
            timeout=60,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise BundleError(f"Git query failed: {error}") from error
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", errors="replace")[:400]
        raise BundleError(f"Git query failed: {detail}")
    return completed.stdout


def require_clean_repository(repo: Path) -> tuple[str, str]:
    """Return exact commit and object format after rejecting every dirty path."""

    root = Path(run_git(repo, ["rev-parse", "--show-toplevel"]).decode().strip()).resolve()
    if root != repo.resolve():
        raise BundleError(f"--repo must be the Git top level: {repo}")
    dirty = run_git(repo, ["status", "--porcelain=v1", "-z", "--untracked-files=all"])
    if dirty:
        raise BundleError("repository is dirty; commit or remove every tracked/untracked change")
    commit = run_git(repo, ["rev-parse", "HEAD"]).decode("ascii").strip()
    if not HEX_40.fullmatch(commit):
        raise BundleError("repository HEAD is not a full SHA-1 commit")
    object_format = run_git(repo, ["rev-parse", "--show-object-format"]).decode().strip()
    if object_format not in {"sha1", "sha256"}:
        raise BundleError(f"unsupported Git object format: {object_format!r}")
    return commit, object_format


def denied_source_path(path: str) -> bool:
    """Return whether a tracked path belongs to an excluded authority/privacy domain."""

    parts = PurePosixPath(path).parts
    if any(part in PRIVATE_COMPONENTS for part in parts):
        return True
    if parts and parts[0] in {".git", ".github", "minime"}:
        return True
    if path.startswith(("capsules/spectral-bridge/", "capsules/introspector/")):
        return True
    if path.startswith(
        (
            "services/astrid-edge-steward-helper/",
            "services/astrid-edge-rescue-helper/",
            "services/astrid-edge-web-broker/",
            "services/astrid-edge-provider-broker/",
            "services/astrid-edge-presentation-broker/",
            "services/astrid-edge-checkpoint/",
        )
    ):
        return True
    if path.startswith(("scripts/edge_self_change/", "services/astrid-edge-self-change-")):
        return True
    if path in {
        "scripts/edge_self_change_supervisor.py",
        "scripts/test_edge_self_change_supervisor.py",
        "scripts/build_edge_self_change_source_bundle.py",
        "scripts/test_build_edge_self_change_source_bundle.py",
    }:
        return True
    if path.startswith("packaging/systemd/") and any(
        marker in path
        for marker in (
            "self-change",
            "edge-steward",
            "edge-web-broker",
            "edge-checkpoint",
            "core-liveness",
        )
    ):
        return True
    return False


def inspect_only_boundary_path(path: str) -> bool:
    """Classify reviewed rescue policy source that is readable but never candidate-mutable."""

    relative = PurePosixPath(path)
    if any(part in PRIVATE_COMPONENTS or part.startswith(".") for part in relative.parts):
        return False
    for prefix in INSPECT_ONLY_SERVICE_PREFIXES:
        if path.startswith(prefix):
            leaf = path.removeprefix(prefix)
            return leaf in {"Cargo.toml", "Cargo.lock"} or (
                leaf.startswith("src/") and relative.suffix == ".rs"
            )
    if path.startswith("scripts/edge_self_change/"):
        return relative.suffix == ".py"
    if path.startswith("scripts/"):
        return path.removeprefix("scripts/") in INSPECT_ONLY_SCRIPT_NAMES
    if path.startswith("packaging/systemd/root/"):
        return relative.suffix in {".service", ".in", ".conf"} or relative.name in {
            "astrid-edge-builder-store",
            "astrid-edge-state-store",
            "astrid-edge-self-evolution-control",
            "migrate-edge-user-services-to-system",
        }
    if path.startswith("packaging/systemd/"):
        name = relative.name
        return any(
            marker in name
            for marker in (
                "self-change",
                "edge-steward",
                "edge-web-broker",
                "edge-provider-broker",
                "edge-presentation-broker",
                "edge-checkpoint",
                "builder-store",
                "state-store",
                "audio-feeder",
                "generation-guard",
                "core-liveness",
            )
        ) and relative.suffix in {".service", ".timer", ".socket", ".conf", ".env", ".in"}
    if path == "packaging/headless/edge-audio-feeder.json.in":
        return True
    return path == "docs/cpu-edge-self-evolution.md"


def source_role(path: str) -> str | None:
    """Classify explicitly bundled tracked source, returning no ambient catch-all."""

    safe_relative_path(path)
    if inspect_only_boundary_path(path):
        return "inspect_only_immutable_boundary"
    if denied_source_path(path):
        return None
    if path in {"Cargo.toml", "Cargo.lock"}:
        return "mutable_build_manifest"
    if path == "wit/astrid-capsule.wit":
        return "build_required_immutable"
    if path in {".cargo/config.toml", "clippy.toml", "rustfmt.toml"}:
        return "build_required_manifest"
    if path.startswith("crates/"):
        parts = PurePosixPath(path).parts
        if len(parts) < 3:
            return None
        crate_name = parts[1]
        relative = PurePosixPath(*parts[2:])
        if relative.as_posix() == "Cargo.toml":
            return (
                "mutable_build_manifest"
                if crate_name in MUTABLE_CORE_CRATES
                else "build_required_immutable"
            )
        if relative.as_posix() == "build.rs":
            return (
                "mutable_core_source"
                if crate_name in MUTABLE_CORE_CRATES
                else "build_required_immutable"
            )
        if relative.suffix in BUILD_FILE_SUFFIXES:
            return (
                "mutable_core_source"
                if crate_name in MUTABLE_CORE_CRATES and relative.suffix == ".rs"
                else "build_required_immutable"
            )
        return None
    if path.startswith("services/astrid-edge-runtime/"):
        relative = path.removeprefix("services/astrid-edge-runtime/")
        if relative in {"Cargo.toml", "Cargo.lock"}:
            return "mutable_build_manifest"
        if relative.startswith("src/") and path.endswith(".rs"):
            return "mutable_edge_runtime"
        return None
    for capsule in EDGE_CAPSULES:
        prefix = f"capsules/astralis/{capsule}/"
        if not path.startswith(prefix):
            continue
        relative = path.removeprefix(prefix)
        if relative in {"Cargo.toml", "Cargo.lock"}:
            return "mutable_build_manifest"
        if relative == "Capsule.toml":
            return "mutable_capsule_manifest"
        if relative.startswith("src/") and Path(relative).suffix in {
            ".rs",
            ".md",
            ".json",
            ".toml",
            ".txt",
        }:
            return "mutable_edge_capsule"
        return None
    if path.startswith("scripts/"):
        name = path.removeprefix("scripts/")
        if name == "warm_ollama_model.sh":
            return "build_required_runtime_script"
        if name in MUTABLE_LIVE_REPORTS:
            return "mutable_edge_report"
        if name in BUILD_REQUIRED_REPORT_TESTS:
            return "build_required_immutable"
        return None
    if path.startswith("packaging/appliances/") and Path(path).suffix in {".env", ".json"}:
        return "mutable_appliance_profile"
    if path.startswith("packaging/systemd/"):
        parts = PurePosixPath(path).parts
        name = parts[-1]
        if (name.startswith("astrid") or name == "ollama-cpu.service") and Path(name).suffix in {
            ".service",
            ".timer",
            ".conf",
            ".env",
        }:
            if (
                name in MUTABLE_UNIT_FRAGMENTS
                and (len(parts) == 3 or (len(parts) == 4 and parts[2] == "icp"))
            ):
                return "mutable_astrid_service_template"
            return "build_required_service_template"
    return None


def tracked_source_payloads(repo: Path, object_format: str) -> list[Payload]:
    """Read exact indexed blobs for the explicit CPU-edge source surface."""

    output = run_git(repo, ["ls-files", "--stage", "-z"])
    payloads: list[Payload] = []
    for raw_record in output.split(b"\0"):
        if not raw_record:
            continue
        try:
            metadata, raw_path = raw_record.split(b"\t", 1)
            mode, object_id, stage = metadata.decode("ascii").split()
            path = raw_path.decode("utf-8", errors="strict")
        except (UnicodeDecodeError, ValueError) as error:
            raise BundleError("malformed or non-UTF-8 Git index entry") from error
        if stage != "0":
            raise BundleError(f"repository has an unmerged index entry: {path}")
        role = source_role(path)
        if role is None:
            continue
        if mode not in {"100644", "100755"}:
            raise BundleError(f"selected source is a symlink, submodule, or special mode: {path}")
        data = read_regular(repo / path)
        digest = hashlib.new(object_format)
        digest.update(f"blob {len(data)}\0".encode("ascii"))
        digest.update(data)
        if digest.hexdigest() != object_id:
            raise BundleError(f"selected source differs from its indexed Git blob: {path}")
        payloads.append(
            Payload(
                path=f"source/{path}",
                origin=role,
                mode=0o755 if mode == "100755" else 0o644,
                sha256=sha256_bytes(data),
                size=len(data),
                source=repo / path,
            )
        )
    required = {
        "source/Cargo.lock",
        "source/Cargo.toml",
        "source/services/astrid-edge-runtime/Cargo.toml",
        "source/services/astrid-edge-runtime/Cargo.lock",
        "source/crates/astrid-daemon/Cargo.toml",
        "source/scripts/warm_ollama_model.sh",
    }
    required.update(
        f"source/capsules/astralis/{capsule}/{leaf}"
        for capsule in LOCAL_EDGE_CAPSULES
        for leaf in ("Cargo.toml", "Cargo.lock")
    )
    required.update(
        f"source/services/{service}/{leaf}"
        for service in EDGE_STANDALONE_SERVICES
        for leaf in ("Cargo.toml", "Cargo.lock")
    )
    present = {payload.path for payload in payloads}
    missing = sorted(required - present)
    if missing:
        raise BundleError(f"required tracked CPU-edge source is absent: {missing}")
    return payloads


def external_capsule_source_payloads(
    roots: list[Path], existing: list[Payload]
) -> list[Payload]:
    """Verify deterministic recipe output and supply missing external capsule source."""

    present = {
        PurePosixPath(payload.path).parts[3]
        for payload in existing
        if payload.path.startswith("source/capsules/astralis/")
        and len(PurePosixPath(payload.path).parts) >= 5
    }
    expected = set(EXTERNAL_EDGE_CAPSULES) - present
    if not expected:
        if roots:
            raise BundleError("external capsule source roots overlap tracked source")
        return []
    payloads: list[Payload] = []
    supplied: set[str] = set()
    for root_argument in roots:
        if root_argument.is_symlink() or not root_argument.is_dir():
            raise BundleError("external capsule source root must be a non-symlink directory")
        root = root_argument.resolve()
        manifest_path = root / "SOURCE-MANIFEST.json"
        try:
            manifest = json.loads(read_regular(manifest_path, limit=8 * 1024 * 1024))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise BundleError(f"external capsule source manifest is invalid: {error}") from error
        if (
            not isinstance(manifest, dict)
            or set(manifest)
            != {
                "schema",
                "recipe",
                "rust_toolchain",
                "target",
                "sdk_version",
                "source_policy",
                "capsules",
            }
            or manifest.get("schema") != EXTERNAL_SOURCE_SCHEMA
            or manifest.get("rust_toolchain") != REQUIRED_RUST_RELEASE
            or manifest.get("target") != "wasm32-wasip2"
            or manifest.get("source_policy")
            not in {
                "reviewed_patch_replay",
                "exact_upstream_snapshot",
                "pinned_upstream_patch_replay",
            }
        ):
            raise BundleError("external capsule source manifest has an unsupported schema")
        capsules = manifest.get("capsules")
        if not isinstance(capsules, list) or not capsules:
            raise BundleError("external capsule source manifest contains no capsules")
        manifest_paths: set[str] = set()
        manifest_capsules: set[str] = set()
        root_manifest_paths: set[str] = set()
        for capsule in capsules:
            if not isinstance(capsule, dict) or set(capsule) != {
                "id",
                "package",
                "revision",
                "files",
            }:
                raise BundleError("external capsule source entry has an unexpected schema")
            short_id = capsule.get("id")
            package = capsule.get("package")
            revision = capsule.get("revision")
            if (
                not isinstance(short_id, str)
                or not isinstance(package, str)
                or package != f"astrid-capsule-{short_id}"
                or package not in expected
                or package in supplied
                or package in manifest_capsules
                or not isinstance(revision, str)
                or not HEX_40.fullmatch(revision)
            ):
                raise BundleError("external capsule source identity is invalid or duplicated")
            manifest_capsules.add(package)
            files = capsule.get("files")
            if not isinstance(files, list) or not files:
                raise BundleError("external capsule source inventory is empty")
            for record in files:
                if not isinstance(record, dict) or set(record) != {
                    "path",
                    "mode",
                    "size",
                    "sha256",
                }:
                    raise BundleError("external capsule source file has an unexpected schema")
                record_path = record.get("path")
                if not isinstance(record_path, str):
                    raise BundleError("external capsule source path is not text")
                relative = safe_relative_path(record_path)
                if relative.parts[0] != short_id or record_path in manifest_paths:
                    raise BundleError("external capsule source path has invalid ownership")
                manifest_paths.add(record_path)
                root_manifest_paths.add(record_path)
                source_relative = PurePosixPath(*relative.parts[1:]).as_posix()
                destination_relative = f"capsules/astralis/{package}/{source_relative}"
                origin = source_role(destination_relative)
                if origin not in {
                    "mutable_build_manifest",
                    "mutable_capsule_manifest",
                    "mutable_edge_capsule",
                }:
                    raise BundleError("external capsule source file is outside mutable policy")
                if (
                    record.get("mode") != "0644"
                    or not isinstance(record.get("size"), int)
                    or record["size"] < 0
                    or not isinstance(record.get("sha256"), str)
                    or not HEX_64.fullmatch(record["sha256"])
                ):
                    raise BundleError("external capsule source metadata is invalid")
                source_path = root.joinpath(*relative.parts)
                data = read_regular(source_path)
                if len(data) != record["size"] or sha256_bytes(data) != record["sha256"]:
                    raise BundleError("external capsule source content fails its manifest")
                payloads.append(
                    Payload(
                        path=f"source/{destination_relative}",
                        origin=origin,
                        mode=0o644,
                        sha256=record["sha256"],
                        size=record["size"],
                        source=source_path,
                    )
                )
            supplied.add(package)
        actual_paths: set[str] = set()
        for directory, names, files in os.walk(root, followlinks=False):
            directory_path = Path(directory)
            if directory_path.is_symlink():
                raise BundleError("external capsule source tree contains a symlink")
            for name in names:
                if (directory_path / name).is_symlink():
                    raise BundleError("external capsule source tree contains a symlink")
            for name in files:
                file_path = directory_path / name
                relative = file_path.relative_to(root).as_posix()
                if relative != "SOURCE-MANIFEST.json":
                    actual_paths.add(relative)
        if actual_paths != root_manifest_paths:
            raise BundleError("external capsule source tree membership fails its manifest")
    if supplied != expected:
        raise BundleError(
            f"external capsule source set is incomplete: {sorted(expected - supplied)}"
        )
    return payloads


def quickjs_kernel_payload(repo: Path, tracked: list[Payload]) -> Payload:
    """Bind the ignored, build-required QuickJS kernel into signed source.

    The tracked BLAKE3 sidecar is syntax-checked here. The Rust build script is
    the final cryptographic BLAKE3 verifier during every locked offline build;
    this envelope independently binds the exact WASM bytes with SHA-256.
    """

    path = repo / QUICKJS_KERNEL_PATH
    try:
        metadata = path.lstat()
    except OSError as error:
        raise BundleError("required QuickJS engine.wasm payload is absent") from error
    if path.is_symlink() or not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
        raise BundleError("QuickJS engine.wasm must be a single-linked regular file")
    data = read_regular(path, limit=MAX_QUICKJS_KERNEL_BYTES)
    if len(data) < 8 or data[:8] != b"\0asm\x01\0\0\0":
        raise BundleError("QuickJS engine.wasm has an invalid WASM header")
    sidecar = next(
        (item for item in tracked if item.path == f"source/{QUICKJS_KERNEL_HASH_PATH}"),
        None,
    )
    if sidecar is None:
        raise BundleError("tracked QuickJS engine.wasm.blake3 verifier is absent")
    try:
        sidecar_text = payload_bytes(sidecar).decode("ascii")
    except UnicodeDecodeError as error:
        raise BundleError("QuickJS engine.wasm.blake3 is not ASCII") from error
    if re.fullmatch(r"[0-9a-f]{64}  engine\.wasm\n?", sidecar_text) is None:
        raise BundleError("QuickJS engine.wasm.blake3 has an invalid exact record")
    return Payload(
        path=f"source/{QUICKJS_KERNEL_PATH}",
        origin="build_required_immutable",
        mode=0o644,
        sha256=sha256_bytes(data),
        size=len(data),
        source=path,
    )


def parse_rustc_metadata_bytes(data: bytes) -> tuple[bytes, dict[str, str]]:
    """Require and normalize the exact reviewed Rust 1.94 compiler metadata."""

    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise BundleError("rustc metadata is not UTF-8") from error
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    fields: dict[str, str] = {}
    for line in lines[1:]:
        if ": " not in line:
            raise BundleError(f"malformed rustc metadata line: {line!r}")
        key, value = line.split(": ", 1)
        if key in fields:
            raise BundleError(f"duplicate rustc metadata field: {key}")
        fields[key] = value
    expected = {
        "binary": "rustc",
        "commit-hash": REQUIRED_RUST_COMMIT,
        "commit-date": REQUIRED_RUST_COMMIT_DATE,
        "release": REQUIRED_RUST_RELEASE,
        "LLVM version": REQUIRED_LLVM_VERSION,
    }
    if any(fields.get(key) != value for key, value in expected.items()):
        raise BundleError("rustc metadata does not match the exact reviewed Rust 1.94 toolchain")
    host = fields.get("host", "")
    if not host or not SAFE_COMPONENT.fullmatch(host):
        raise BundleError("rustc metadata has an invalid host triple")
    first = f"rustc {REQUIRED_RUST_RELEASE} ({REQUIRED_RUST_COMMIT[:9]} {REQUIRED_RUST_COMMIT_DATE})"
    if not lines or lines[0] != first:
        raise BundleError("rustc version line does not match the exact reviewed toolchain")
    normalized = ("\n".join(lines) + "\n").encode("utf-8")
    return normalized, {**expected, "host": host, "version_line": first}


def parse_rustc_metadata(path: Path) -> tuple[bytes, dict[str, str]]:
    return parse_rustc_metadata_bytes(read_regular(path, limit=8 * 1024))


def parse_cargo_lock(data: bytes) -> tuple[dict[tuple[str, str], str | None], int]:
    """Parse an already-resolved lockfile and return every external package."""

    try:
        lock = tomllib.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise BundleError(f"invalid Cargo.lock: {error}") from error
    if lock.get("version") not in {3, 4}:
        raise BundleError("Cargo.lock version must be 3 or 4")
    packages = lock.get("package")
    if not isinstance(packages, list) or not packages:
        raise BundleError("Cargo.lock contains no packages")
    external: dict[tuple[str, str], str | None] = {}
    for package in packages:
        if not isinstance(package, dict):
            raise BundleError("Cargo.lock package is not a table")
        source = package.get("source")
        if source is None:
            continue
        if not isinstance(source, str) or not source.startswith(("registry+", "git+")):
            raise BundleError(f"unsupported locked dependency source: {source!r}")
        name = package.get("name")
        version = package.get("version")
        checksum = package.get("checksum")
        if not isinstance(name, str) or not isinstance(version, str):
            raise BundleError("external Cargo.lock package lacks name/version")
        if checksum is not None and (not isinstance(checksum, str) or not HEX_64.fullmatch(checksum)):
            raise BundleError(f"invalid lock checksum for {name} {version}")
        key = (name, version)
        if key in external:
            raise BundleError(f"ambiguous duplicate external package in Cargo.lock: {key}")
        external[key] = checksum
    if not external:
        raise BundleError("Cargo.lock has no external packages to match against vendor input")
    return external, int(lock["version"])


def merge_locked_packages(
    destination: dict[tuple[str, str], str | None],
    incoming: dict[tuple[str, str], str | None],
) -> None:
    """Merge standalone workspace locks without weakening checksum identity."""

    for package, checksum in incoming.items():
        existing = destination.get(package)
        if package in destination and existing != checksum:
            raise BundleError(f"conflicting checksums across Cargo.lock files: {package}")
        destination[package] = checksum


def walk_regular_tree(root: Path) -> dict[str, tuple[Path, bytes]]:
    """Read a package tree while rejecting links, devices, and race-prone hardlinks."""

    result: dict[str, tuple[Path, bytes]] = {}

    def visit(directory: Path, prefix: PurePosixPath) -> None:
        try:
            entries = sorted(os.scandir(directory), key=lambda entry: entry.name)
        except OSError as error:
            raise BundleError(f"cannot scan vendor directory {directory}: {error}") from error
        for entry in entries:
            relative = prefix / entry.name
            safe_relative_path(relative.as_posix())
            if entry.is_symlink():
                raise BundleError(f"vendor input contains a symlink: {relative}")
            if entry.is_dir(follow_symlinks=False):
                if entry.name == ".git" or (not prefix.parts and entry.name == "target"):
                    raise BundleError(f"vendor input contains VCS/build state: {relative}")
                visit(Path(entry.path), relative)
                continue
            if not entry.is_file(follow_symlinks=False):
                raise BundleError(f"vendor input contains a device or special file: {relative}")
            data = read_regular(Path(entry.path))
            result[relative.as_posix()] = (Path(entry.path), data)

    visit(root, PurePosixPath())
    return result


def vendor_payloads(
    vendor_root: Path,
    locked: dict[tuple[str, str], str | None],
) -> tuple[list[Payload], list[dict[str, Any]]]:
    """Validate Cargo checksum inventories and return exact vendor payloads."""

    if vendor_root.is_symlink() or not vendor_root.is_dir():
        raise BundleError("--vendor-dir must be a non-symlink directory")
    try:
        package_entries = sorted(os.scandir(vendor_root), key=lambda entry: entry.name)
    except OSError as error:
        raise BundleError(f"cannot scan vendor root: {error}") from error
    payloads: list[Payload] = []
    packages: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    for entry in package_entries:
        if entry.is_symlink() or not entry.is_dir(follow_symlinks=False):
            raise BundleError(f"unexpected non-directory in vendor root: {entry.name}")
        if not SAFE_COMPONENT.fullmatch(entry.name) or entry.name.startswith("."):
            raise BundleError(f"unsafe vendor package directory: {entry.name!r}")
        files = walk_regular_tree(Path(entry.path))
        manifest_entry = files.get("Cargo.toml")
        checksum_entry = files.get(".cargo-checksum.json")
        if manifest_entry is None or checksum_entry is None:
            raise BundleError(f"vendor package lacks Cargo.toml/checksum inventory: {entry.name}")
        try:
            manifest = tomllib.loads(manifest_entry[1].decode("utf-8"))
            checksum_record = json.loads(checksum_entry[1])
        except (UnicodeDecodeError, tomllib.TOMLDecodeError, json.JSONDecodeError) as error:
            raise BundleError(f"invalid vendor metadata in {entry.name}: {error}") from error
        package = manifest.get("package", {})
        name = package.get("name") if isinstance(package, dict) else None
        version = package.get("version") if isinstance(package, dict) else None
        if not isinstance(name, str) or not isinstance(version, str):
            raise BundleError(f"vendor Cargo.toml lacks package identity: {entry.name}")
        key = (name, version)
        if key not in locked or key in seen:
            raise BundleError(f"unexpected or duplicate vendor package: {name} {version}")
        seen.add(key)
        if not isinstance(checksum_record, dict) or set(checksum_record) != {"files", "package"}:
            raise BundleError(f"malformed .cargo-checksum.json: {entry.name}")
        checksum_files = checksum_record["files"]
        if not isinstance(checksum_files, dict):
            raise BundleError(f"vendor checksum file map is invalid: {entry.name}")
        actual_names = set(files) - {".cargo-checksum.json"}
        if set(checksum_files) != actual_names:
            raise BundleError(f"vendor package has missing or unexpected files: {entry.name}")
        for relative, expected_hash in checksum_files.items():
            safe_relative_path(relative)
            if not isinstance(expected_hash, str) or not HEX_64.fullmatch(expected_hash):
                raise BundleError(f"invalid vendor file checksum: {entry.name}/{relative}")
            if sha256_bytes(files[relative][1]) != expected_hash:
                raise BundleError(f"vendor file checksum mismatch: {entry.name}/{relative}")
        if checksum_record["package"] != locked[key]:
            raise BundleError(f"vendor package checksum does not match Cargo.lock: {key}")
        packages.append(
            {
                "directory": entry.name,
                "name": name,
                "version": version,
                "package_checksum": locked[key],
            }
        )
        for relative, (source, data) in files.items():
            payloads.append(
                Payload(
                    path=f"vendor/{entry.name}/{relative}",
                    origin="operator_vendored_cargo",
                    mode=0o644,
                    sha256=sha256_bytes(data),
                    size=len(data),
                    source=source,
                )
            )
    missing = sorted(set(locked) - seen)
    if missing:
        raise BundleError(f"vendor directory is missing locked packages: {missing[:10]}")
    if not payloads:
        raise BundleError("vendor directory is empty")
    return payloads, packages


def validate_package_manifests(payloads: Iterable[Payload]) -> None:
    """Bind every required package path to its exact Cargo package identity."""

    by_path = {payload.path: payload for payload in payloads}
    expected_packages = {
        **{
            f"source/services/{service}/Cargo.toml": service
            for service in EDGE_STANDALONE_SERVICES
        },
        **{
            f"source/capsules/astralis/{capsule}/Cargo.toml": capsule
            for capsule in EDGE_CAPSULES
        },
    }
    for path, expected_name in expected_packages.items():
        payload = by_path.get(path)
        if payload is None:
            raise BundleError(f"missing required package manifest: {path}")
        try:
            document = tomllib.loads(payload_bytes(payload).decode("utf-8"))
        except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
            raise BundleError(f"invalid required manifest {path}: {error}") from error
        package = document.get("package")
        if not isinstance(package, dict) or package.get("name") != expected_name:
            raise BundleError(f"manifest package identity does not match its path: {path}")

    rust_manifests = ["source/Cargo.toml"]
    rust_manifests.extend(
        f"source/services/{service}/Cargo.toml"
        for service in EDGE_STANDALONE_SERVICES
    )
    rust_manifests.extend(
        f"source/capsules/astralis/{capsule}/Cargo.toml"
        for capsule in LOCAL_EDGE_CAPSULES
    )
    for path in rust_manifests:
        payload = by_path.get(path)
        if payload is None:
            raise BundleError(f"missing required Rust-version manifest: {path}")
        try:
            document = tomllib.loads(payload_bytes(payload).decode("utf-8"))
        except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
            raise BundleError(f"invalid required manifest {path}: {error}") from error
        value = (
            document.get("workspace", {}).get("package", {}).get("rust-version")
            if path == "source/Cargo.toml"
            else document.get("package", {}).get("rust-version")
        )
        if value != "1.94":
            raise BundleError(f"manifest must declare exact rust-version 1.94: {path}")


def key_material(path: Path) -> bytes:
    key = read_regular(path, limit=4 * 1024, owner_only=True)
    if len(key) < 32:
        raise BundleError("HMAC key must contain at least 32 bytes")
    return key


def payload_bytes(payload: Payload) -> bytes:
    if payload.data is not None:
        data = payload.data
    elif payload.source is not None:
        data = read_regular(payload.source)
    else:
        raise BundleError(f"payload has no source: {payload.path}")
    if len(data) != payload.size or sha256_bytes(data) != payload.sha256:
        raise BundleError(f"payload changed after inventory: {payload.path}")
    return data


def source_identity(
    commit: str,
    rustc: dict[str, str],
    inventory: list[dict[str, Any]],
    appliance_id: str | None,
    source_authority: str,
) -> dict[str, Any]:
    return {
        "schema": SOURCE_ID_SCHEMA,
        "appliance_id": appliance_id,
        "source_authority": source_authority,
        "repository_commit": commit,
        "rustc": rustc,
        "files": inventory,
    }


def appliance_bound_source_identity(
    commit: str,
    rustc: dict[str, str],
    inventory: list[dict[str, Any]],
    appliance_id: str,
) -> dict[str, Any]:
    """Derive the local authorizing identity for one exact appliance."""

    if SAFE_APPLIANCE_ID.fullmatch(appliance_id) is None:
        raise BundleError("invalid appliance identifier for local source identity")
    return source_identity(
        commit,
        rustc,
        inventory,
        appliance_id,
        LOCAL_SOURCE_AUTHORITY,
    )


def validate_appliance_bound_manifest(
    manifest: dict[str, Any], expected_appliance_id: str
) -> None:
    """Reject a local source manifest bound to a different appliance."""

    identity = appliance_bound_source_identity(
        manifest.get("repository_commit"),
        manifest.get("rustc"),
        manifest.get("files"),
        expected_appliance_id,
    )
    identity_hash = sha256_bytes(canonical_bytes(identity))
    if (
        manifest.get("appliance_id") != expected_appliance_id
        or manifest.get("source_authority") != LOCAL_SOURCE_AUTHORITY
        or manifest.get("source_identity_sha256") != identity_hash
        or manifest.get("source_id") != f"cpu-edge:{identity_hash}"
    ):
        raise BundleError("source manifest is not bound to the expected appliance")


def add_tar_bytes(archive: tarfile.TarFile, path: str, data: bytes, mode: int) -> None:
    info = tarfile.TarInfo(f"{BUNDLE_ROOT}/{path}")
    info.size = len(data)
    info.mode = mode
    info.mtime = 0
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    archive.addfile(info, io.BytesIO(data))


def write_bundle(output: Path, payloads: list[Payload], manifest: dict[str, Any], signature: dict[str, Any]) -> None:
    """Write a byte-reproducible archive without replacing an existing target."""

    if output.exists() or output.is_symlink():
        raise BundleError(f"refusing to overwrite output: {output}")
    if not output.parent.is_dir() or output.parent.is_symlink():
        raise BundleError(f"output parent must be an existing non-symlink directory: {output.parent}")
    manifest_bytes = canonical_bytes(manifest) + b"\n"
    signature_bytes = canonical_bytes(signature) + b"\n"
    temporary_name: str | None = None
    try:
        with tempfile.NamedTemporaryFile(prefix=".edge-source-", dir=output.parent, delete=False) as raw:
            temporary_name = raw.name
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
                    for payload in payloads:
                        add_tar_bytes(archive, payload.path, payload_bytes(payload), payload.mode)
                    add_tar_bytes(archive, "MANIFEST.json", manifest_bytes, 0o600)
                    add_tar_bytes(archive, "MANIFEST.signature.json", signature_bytes, 0o600)
            raw.flush()
            os.fsync(raw.fileno())
        os.chmod(temporary_name, 0o600)
        try:
            os.link(temporary_name, output)
        except FileExistsError as error:
            raise BundleError(f"refusing to overwrite output: {output}") from error
        except OSError as error:
            raise BundleError(f"cannot publish source bundle {output}: {error}") from error
        os.unlink(temporary_name)
        temporary_name = None
    finally:
        if temporary_name is not None:
            try:
                os.unlink(temporary_name)
            except FileNotFoundError:
                pass


def build_bundle(args: argparse.Namespace) -> dict[str, Any]:
    if args.repo.is_symlink():
        raise BundleError("--repo must not be a symlink")
    repo = args.repo.resolve()
    commit, object_format = require_clean_repository(repo)
    rustc_bytes, rustc = parse_rustc_metadata(args.rustc_metadata)
    lock_bytes = read_regular(args.cargo_lock, limit=64 * 1024 * 1024)
    locked, lock_version = parse_cargo_lock(lock_bytes)

    payloads = tracked_source_payloads(repo, object_format)
    tracked_root_lock = next(
        (payload for payload in payloads if payload.path == "source/Cargo.lock"), None
    )
    if tracked_root_lock is None or payload_bytes(tracked_root_lock) != lock_bytes:
        raise BundleError(
            "--cargo-lock must be the exact tracked root Cargo.lock from the signed repository"
        )
    payloads.extend(
        external_capsule_source_payloads(args.external_capsule_source_dir, payloads)
    )
    payloads.append(quickjs_kernel_payload(repo, payloads))
    present_payload_paths = {payload.path for payload in payloads}
    missing_capsule_inputs = sorted(
        f"source/capsules/astralis/{capsule}/{leaf}"
        for capsule in EDGE_CAPSULES
        for leaf in ("Cargo.toml", "Cargo.lock")
        if f"source/capsules/astralis/{capsule}/{leaf}" not in present_payload_paths
    )
    if missing_capsule_inputs:
        raise BundleError(
            f"required CPU-edge capsule source is absent: {missing_capsule_inputs}"
        )
    for payload in payloads:
        if payload.path.endswith("/Cargo.lock"):
            dependency_lock, dependency_version = parse_cargo_lock(payload_bytes(payload))
            if dependency_version != lock_version:
                raise BundleError("CPU-edge Cargo.lock format versions do not match")
            merge_locked_packages(locked, dependency_lock)
    validate_package_manifests(payloads)
    vendor, vendor_packages = vendor_payloads(args.vendor_dir, locked)
    payloads.extend(vendor)
    payloads.append(
        Payload(
            path="rustc-version.txt",
            origin="operator_supplied_toolchain_metadata",
            mode=0o644,
            sha256=sha256_bytes(rustc_bytes),
            size=len(rustc_bytes),
            data=rustc_bytes,
        )
    )
    paths = [payload.path for payload in payloads]
    if len(paths) != len(set(paths)) or len(paths) > MAX_FILES:
        raise BundleError("bundle has duplicate paths or exceeds the file ceiling")
    payloads.sort(key=lambda payload: payload.path)
    total_bytes = sum(payload.size for payload in payloads)
    if total_bytes > MAX_UNCOMPRESSED_BYTES:
        raise BundleError("bundle exceeds the uncompressed byte ceiling")
    inventory = [payload.inventory() for payload in payloads]
    identity = source_identity(
        commit,
        rustc,
        inventory,
        None,
        PORTABLE_SOURCE_AUTHORITY,
    )
    identity_hash = sha256_bytes(canonical_bytes(identity))
    source_id = f"cpu-edge-portable:{identity_hash}"

    if args.test_only_unsigned:
        signature_mode = "test_only_unsigned"
        key_id = None
    else:
        key = key_material(args.signing_key)
        signature_mode = "hmac-sha256"
        key_id = sha256_bytes(key)[:16]
    manifest = {
        "schema": SCHEMA,
        "appliance_id": None,
        "source_authority": PORTABLE_SOURCE_AUTHORITY,
        "source_id": source_id,
        "source_identity_sha256": identity_hash,
        "repository_commit": commit,
        "git_object_format": object_format,
        "rustc": rustc,
        "cargo_lock_version": lock_version,
        "cargo_lock_sha256": sha256_bytes(lock_bytes),
        "vendor_packages": vendor_packages,
        "signature_mode": signature_mode,
        "key_id": key_id,
        "file_count": len(payloads),
        "uncompressed_bytes": total_bytes,
        "files": inventory,
    }
    manifest_bytes = canonical_bytes(manifest)
    if args.test_only_unsigned:
        signature = {
            "schema": SIGNATURE_SCHEMA,
            "mode": "test_only_unsigned",
            "manifest_sha256": sha256_bytes(manifest_bytes),
        }
    else:
        signature = {
            "schema": SIGNATURE_SCHEMA,
            "mode": "hmac-sha256",
            "key_id": key_id,
            "manifest_sha256": sha256_bytes(manifest_bytes),
            "hmac_sha256": hmac.new(key, manifest_bytes, hashlib.sha256).hexdigest(),
        }
    final_commit, final_object_format = require_clean_repository(repo)
    if (final_commit, final_object_format) != (commit, object_format):
        raise BundleError("repository identity changed while the bundle was assembled")
    write_bundle(args.output, payloads, manifest, signature)
    return {
        "bundle": str(args.output),
        "source_id": source_id,
        "file_count": len(payloads),
        "signature_mode": signature_mode,
    }


def read_member(archive: tarfile.TarFile, member: tarfile.TarInfo, limit: int) -> bytes:
    if member.size > limit:
        raise BundleError(f"archive member exceeds {limit} bytes: {member.name}")
    handle = archive.extractfile(member)
    if handle is None:
        raise BundleError(f"cannot read archive member: {member.name}")
    data = handle.read(limit + 1)
    if len(data) != member.size or len(data) > limit:
        raise BundleError(f"archive member size changed or exceeds limit: {member.name}")
    return data


def validate_inventory(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    files = manifest.get("files")
    if not isinstance(files, list) or not files or len(files) > MAX_FILES:
        raise BundleError("manifest has no valid bounded file inventory")
    normalized: list[dict[str, Any]] = []
    previous = ""
    total = 0
    for record in files:
        if not isinstance(record, dict) or set(record) != {
            "path",
            "origin",
            "mode",
            "size",
            "sha256",
        }:
            raise BundleError("manifest inventory record has an unexpected schema")
        path = record["path"]
        if not isinstance(path, str):
            raise BundleError("manifest path is not text")
        safe_relative_path(path)
        if path <= previous:
            raise BundleError("manifest inventory is not strictly sorted and unique")
        previous = path
        if path.startswith("source/"):
            relative = path.removeprefix("source/")
            expected_origin = (
                "mutable_build_manifest"
                if relative == "Cargo.lock"
                else source_role(relative)
            )
            if expected_origin is None:
                raise BundleError(f"manifest contains excluded/unexpected source: {relative}")
            if record.get("origin") != expected_origin:
                raise BundleError(f"manifest source origin disagrees with policy: {relative}")
        elif not path.startswith("vendor/") and path != "rustc-version.txt":
            raise BundleError(f"manifest contains an unexpected payload path: {path}")
        if record["mode"] not in {"0600", "0644", "0755"}:
            raise BundleError(f"manifest has an unsafe mode: {path}")
        if not isinstance(record["origin"], str) or not record["origin"]:
            raise BundleError(f"manifest has no origin: {path}")
        if not isinstance(record["size"], int) or record["size"] < 0:
            raise BundleError(f"manifest has invalid size: {path}")
        if not isinstance(record["sha256"], str) or not HEX_64.fullmatch(record["sha256"]):
            raise BundleError(f"manifest has invalid hash: {path}")
        total += record["size"]
        if total > MAX_UNCOMPRESSED_BYTES:
            raise BundleError("manifest exceeds the uncompressed byte ceiling")
        normalized.append(record)
    if manifest.get("file_count") != len(normalized) or manifest.get("uncompressed_bytes") != total:
        raise BundleError("manifest aggregate counts do not match its inventory")
    present = {record["path"] for record in normalized}
    required = {f"source/{QUICKJS_KERNEL_PATH}", f"source/{QUICKJS_KERNEL_HASH_PATH}"}
    required.update(
        f"source/capsules/astralis/{capsule}/{leaf}"
        for capsule in EDGE_CAPSULES
        for leaf in ("Cargo.toml", "Cargo.lock")
    )
    required.update(
        f"source/services/{service}/{leaf}"
        for service in EDGE_STANDALONE_SERVICES
        for leaf in ("Cargo.toml", "Cargo.lock")
    )
    missing = sorted(required - present)
    if missing:
        raise BundleError(f"manifest omits required offline build inputs: {missing}")
    return normalized


def verify_bundle(args: argparse.Namespace) -> dict[str, Any]:
    try:
        bundle_stat = args.bundle.lstat()
    except OSError as error:
        raise BundleError(f"cannot stat source bundle: {error}") from error
    if (
        args.bundle.is_symlink()
        or not stat.S_ISREG(bundle_stat.st_mode)
        or bundle_stat.st_nlink != 1
    ):
        raise BundleError("--bundle must be a regular non-symlink, non-hardlinked file")
    try:
        archive = tarfile.open(args.bundle, mode="r:gz")
    except (OSError, tarfile.TarError) as error:
        raise BundleError(f"cannot open source bundle: {error}") from error
    with archive:
        members: dict[str, tarfile.TarInfo] = {}
        for member in archive.getmembers():
            if not member.isfile() or member.issym() or member.islnk() or member.isdev():
                raise BundleError(f"archive contains a non-regular member: {member.name}")
            if member.uid != 0 or member.gid != 0 or member.mtime != 0:
                raise BundleError(f"archive member has non-deterministic ownership/time: {member.name}")
            prefix = f"{BUNDLE_ROOT}/"
            if not member.name.startswith(prefix):
                raise BundleError(f"archive member is outside the fixed bundle root: {member.name}")
            relative = member.name.removeprefix(prefix)
            safe_relative_path(relative)
            if relative in members:
                raise BundleError(f"archive contains a duplicate member: {relative}")
            members[relative] = member
        required = {"MANIFEST.json", "MANIFEST.signature.json"}
        if not required.issubset(members):
            raise BundleError("archive lacks manifest or signature")
        if any(members[name].mode != 0o600 for name in required):
            raise BundleError("manifest and signature must be owner-only in the archive")
        manifest_data = read_member(archive, members["MANIFEST.json"], MAX_MANIFEST_BYTES)
        signature_data = read_member(archive, members["MANIFEST.signature.json"], 16 * 1024)
        try:
            manifest = json.loads(manifest_data)
            signature = json.loads(signature_data)
        except json.JSONDecodeError as error:
            raise BundleError(f"invalid manifest/signature JSON: {error}") from error
        if manifest_data != canonical_bytes(manifest) + b"\n" or signature_data != canonical_bytes(signature) + b"\n":
            raise BundleError("manifest/signature is not canonically encoded")
        expected_manifest_keys = {
            "schema",
            "appliance_id",
            "source_authority",
            "source_id",
            "source_identity_sha256",
            "repository_commit",
            "git_object_format",
            "rustc",
            "cargo_lock_version",
            "cargo_lock_sha256",
            "vendor_packages",
            "signature_mode",
            "key_id",
            "file_count",
            "uncompressed_bytes",
            "files",
        }
        if (
            not isinstance(manifest, dict)
            or set(manifest) != expected_manifest_keys
            or manifest.get("schema") != SCHEMA
            or manifest.get("appliance_id") is not None
            or manifest.get("source_authority") != PORTABLE_SOURCE_AUTHORITY
        ):
            raise BundleError("unsupported source bundle manifest schema")
        if not isinstance(signature, dict) or signature.get("schema") != SIGNATURE_SCHEMA:
            raise BundleError("unsupported source bundle signature schema")
        manifest_bytes = canonical_bytes(manifest)
        if signature.get("manifest_sha256") != sha256_bytes(manifest_bytes):
            raise BundleError("manifest digest does not match signature envelope")
        mode = signature.get("mode")
        if mode == "hmac-sha256":
            if set(signature) != {
                "schema",
                "mode",
                "key_id",
                "manifest_sha256",
                "hmac_sha256",
            }:
                raise BundleError("signed envelope has unexpected fields")
            if args.signing_key is None:
                raise BundleError("signed bundle verification requires --signing-key")
            key = key_material(args.signing_key)
            expected_key_id = sha256_bytes(key)[:16]
            expected_hmac = hmac.new(key, manifest_bytes, hashlib.sha256).hexdigest()
            if (
                manifest.get("signature_mode") != mode
                or manifest.get("key_id") != expected_key_id
                or signature.get("key_id") != expected_key_id
                or not isinstance(signature.get("hmac_sha256"), str)
                or not hmac.compare_digest(signature["hmac_sha256"], expected_hmac)
            ):
                raise BundleError("source bundle HMAC signature is invalid")
        elif mode == "test_only_unsigned":
            if set(signature) != {"schema", "mode", "manifest_sha256"}:
                raise BundleError("unsigned test envelope has unexpected fields")
            if not args.allow_test_only_unsigned or manifest.get("signature_mode") != mode:
                raise BundleError("test-only unsigned bundle was not explicitly allowed")
        else:
            raise BundleError("unsupported signature mode")

        inventory = validate_inventory(manifest)
        expected_members = {record["path"] for record in inventory} | required
        if set(members) != expected_members:
            extra = sorted(set(members) - expected_members)
            missing = sorted(expected_members - set(members))
            raise BundleError(f"archive inventory mismatch; extra={extra[:5]} missing={missing[:5]}")
        rustc_member = members.get("rustc-version.txt")
        if rustc_member is None:
            raise BundleError("archive lacks exact rustc metadata")
        _, recorded_rustc = parse_rustc_metadata_bytes(
            read_member(archive, rustc_member, 8 * 1024)
        )
        if manifest.get("rustc") != recorded_rustc:
            raise BundleError("manifest rustc metadata does not match its payload")
        for record in inventory:
            member = members[record["path"]]
            if member.mode != int(record["mode"], 8) or member.size != record["size"]:
                raise BundleError(f"archive metadata mismatch: {record['path']}")
            digest = hashlib.sha256()
            handle = archive.extractfile(member)
            if handle is None:
                raise BundleError(f"cannot read archive payload: {record['path']}")
            size = 0
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > record["size"]:
                    raise BundleError(f"archive payload exceeds manifest size: {record['path']}")
                digest.update(chunk)
            if size != record["size"] or digest.hexdigest() != record["sha256"]:
                raise BundleError(f"archive payload digest mismatch: {record['path']}")
        rustc = manifest.get("rustc")
        commit = manifest.get("repository_commit")
        if not isinstance(rustc, dict) or not isinstance(commit, str) or not HEX_40.fullmatch(commit):
            raise BundleError("manifest identity metadata is invalid")
        identity = source_identity(
            commit,
            rustc,
            inventory,
            None,
            PORTABLE_SOURCE_AUTHORITY,
        )
        identity_hash = sha256_bytes(canonical_bytes(identity))
        if (
            manifest.get("source_identity_sha256") != identity_hash
            or manifest.get("source_id") != f"cpu-edge-portable:{identity_hash}"
        ):
            raise BundleError("deterministic source identity does not match manifest")
    return {
        "bundle": str(args.bundle),
        "source_id": manifest["source_id"],
        "file_count": manifest["file_count"],
        "signature_mode": mode,
    }


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    subcommands = root.add_subparsers(dest="command", required=True)
    build = subcommands.add_parser("build", help="build a deterministic offline source bundle")
    build.add_argument("--repo", type=Path, required=True)
    build.add_argument("--vendor-dir", type=Path, required=True)
    build.add_argument("--cargo-lock", type=Path, required=True)
    build.add_argument("--rustc-metadata", type=Path, required=True)
    build.add_argument(
        "--external-capsule-source-dir",
        type=Path,
        action="append",
        default=[],
        help="repeatable deterministic source output from the pinned external capsule builder",
    )
    build.add_argument("--output", type=Path, required=True)
    signing = build.add_mutually_exclusive_group(required=True)
    signing.add_argument("--signing-key", type=Path)
    signing.add_argument("--test-only-unsigned", action="store_true")

    verify = subcommands.add_parser("verify", help="verify signature, identity, and every payload")
    verify.add_argument("--bundle", type=Path, required=True)
    verification = verify.add_mutually_exclusive_group(required=True)
    verification.add_argument("--signing-key", type=Path)
    verification.add_argument("--allow-test-only-unsigned", action="store_true")
    return root


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        result = build_bundle(args) if args.command == "build" else verify_bundle(args)
    except BundleError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
