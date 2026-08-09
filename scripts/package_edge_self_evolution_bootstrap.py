#!/usr/bin/env python3
"""Assemble the complete CPU-edge self-evolution bootstrap archive.

This is an operator/release-side packager.  It does not resolve dependencies,
contact a network service, deploy a host, or mint appliance-local authority.
The portable HMAC key carried here provides integrity checking only for the
explicitly non-authorizing portable source/toolchain inputs; the root installer
replaces it with a fresh per-appliance key before any source identity can
authorize a candidate.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
import re
import stat
import sys
import tarfile
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any


SCHEMA = "astrid.edge.self_evolution_bootstrap.v1"
BUNDLE_PREFIX = "astrid-edge-self-evolution"
# The base CPU-edge appliance remains a native x86-64 and ARM64 target.  The
# self-evolution bootstrap is narrower: its two accepted hardware/storage/
# thermal profiles are AVADO and ICP, both x86-64.  Publishing an ARM bootstrap
# before a named ARM profile exists would create an artifact that cannot pass
# its own root installer.
TARGETS = {"x86_64-unknown-linux-gnu"}
EDGE_CAPSULES = {
    "astrid-capsule-agents",
    "astrid-capsule-cli",
    "astrid-capsule-context-engine",
    "astrid-capsule-edge-context",
    "astrid-capsule-edge-introspector",
    "astrid-capsule-edge-spectral",
    "astrid-capsule-fs",
    "astrid-capsule-hook-bridge",
    "astrid-capsule-http",
    "astrid-capsule-identity",
    "astrid-capsule-memory",
    "astrid-capsule-openai-compat",
    "astrid-capsule-prompt-builder",
    "astrid-capsule-react",
    "astrid-capsule-registry",
    "astrid-capsule-router",
    "astrid-capsule-session",
    "astrid-capsule-shell",
    "astrid-capsule-skills",
    "astrid-capsule-system",
}
SAFE_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}\Z")
HEX64 = re.compile(r"[0-9a-f]{64}\Z")
MAX_INPUT_BYTES = 4 * 1024 * 1024 * 1024
MAX_ARCHIVE_MEMBERS = 200_000
MAX_JSON_BYTES = 32 * 1024 * 1024


class PackageError(RuntimeError):
    """A fail-closed release packaging error."""


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False
    ).encode("ascii")


def stable_regular(path: Path, *, maximum: int = MAX_INPUT_BYTES, owner_only: bool = False) -> bytes:
    before = path.lstat()
    if path.is_symlink() or not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        raise PackageError(f"input must be a single-linked regular file: {path}")
    if before.st_size <= 0 or before.st_size > maximum:
        raise PackageError(f"input size is outside the release bound: {path}")
    if owner_only and before.st_mode & 0o077:
        raise PackageError(f"portable key is not owner-only: {path}")
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    with os.fdopen(descriptor, "rb") as handle:
        opened = os.fstat(handle.fileno())
        data = handle.read(maximum + 1)
        after = os.fstat(handle.fileno())
    identity = lambda item: (item.st_dev, item.st_ino, item.st_size, item.st_mtime_ns)
    if identity(before) != identity(opened) or identity(opened) != identity(after):
        raise PackageError(f"input changed while read: {path}")
    if len(data) != before.st_size or len(data) > maximum:
        raise PackageError(f"input changed size or exceeded its bound: {path}")
    return data


def safe_archive_name(name: str) -> PurePosixPath:
    path = PurePosixPath(name)
    if (
        not name
        or path.is_absolute()
        or any(part in {"", ".", ".."} for part in path.parts)
        or path.as_posix() != name
    ):
        raise PackageError(f"unsafe nested archive member: {name!r}")
    return path


def nested_json(archive: bytes, suffix: str, *, label: str) -> dict[str, Any]:
    """Read one manifest from already captured nested-archive bytes."""

    matches: list[bytes] = []
    seen: set[str] = set()
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as source:
        for index, member in enumerate(source):
            if index >= MAX_ARCHIVE_MEMBERS:
                raise PackageError(f"nested archive has too many members: {label}")
            path = safe_archive_name(member.name)
            normalized = path.as_posix()
            if normalized in seen:
                raise PackageError(f"nested archive has duplicate members: {label}")
            seen.add(normalized)
            if not (member.isdir() or member.isfile()):
                raise PackageError(f"nested archive has a link or special member: {label}")
            if member.isfile() and member.size > MAX_INPUT_BYTES:
                raise PackageError(f"nested archive member is oversized: {label}")
            if member.isfile() and normalized.endswith(suffix):
                stream = source.extractfile(member)
                data = stream.read(MAX_JSON_BYTES + 1) if stream is not None else b""
                if len(data) > MAX_JSON_BYTES:
                    raise PackageError(f"nested manifest is oversized: {label}")
                matches.append(data)
    if len(matches) != 1:
        raise PackageError(f"nested archive lacks one exact {suffix}: {label}")
    try:
        value = json.loads(matches[0])
    except (UnicodeError, json.JSONDecodeError) as error:
        raise PackageError(f"nested manifest is malformed: {label}") from error
    if not isinstance(value, dict):
        raise PackageError(f"nested manifest is not an object: {label}")
    return value


def write_exclusive_durable(path: Path, data: bytes, mode: int) -> None:
    """Create one file without following links, then durably commit its directory entry."""

    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags, mode)
    try:
        os.fchmod(descriptor, mode)
        view = memoryview(data)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise OSError("short write while creating release sidecar")
            view = view[written:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)

    directory_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
    directory = os.open(path.parent, directory_flags)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


def add_bytes(archive: tarfile.TarFile, name: str, data: bytes, mode: int) -> None:
    info = tarfile.TarInfo(name)
    info.size = len(data)
    info.mode = mode
    info.mtime = 0
    info.uid = info.gid = 0
    info.uname = info.gname = ""
    archive.addfile(info, io.BytesIO(data))


def profile_manifest() -> dict[str, Any]:
    return {
        "avado": {
            "appliance_id": "avado-edge",
            "runtime_user": "avado",
            "runtime_home": "/home/avado",
            "runtime_workspace": "/home/avado/.astrid/home/default/edge",
            "model": "qwen3.5:4b",
            "context_tokens": 4096,
            "output_tokens": 192,
            "source_authoring_output_tokens": 384,
            "header_timeout_ms": 300000,
            "total_timeout_ms": 600000,
            "audio": "physical_numeric_only",
        },
        "icp": {
            "appliance_id": "icp-edge",
            "runtime_user": "nativeplanet",
            "runtime_home": "/home/nativeplanet",
            "runtime_workspace": "/media/data/astrid/state/home/default/edge",
            "model": "qwen3:1.7b",
            "context_tokens": 2048,
            "output_tokens": 112,
            "source_authoring_output_tokens": 160,
            "header_timeout_ms": 420000,
            "total_timeout_ms": 660000,
            "audio": "explicitly_unavailable",
            "required_mount": "/media/data",
            "retained_backup": "/media/data/astrid/backups/emmc-20260729T130835Z",
        },
    }


def build(args: argparse.Namespace) -> dict[str, Any]:
    if SAFE_ID.fullmatch(args.version) is None or args.target not in TARGETS:
        raise PackageError("version or target is outside the CPU-edge release policy")
    if not args.output_dir.is_dir() or args.output_dir.is_symlink():
        raise PackageError("output directory must already be a real directory")

    inputs = {
        "cpu_edge_archive": (args.appliance_archive, 0o600),
        "initial_generation": (args.generation_archive, 0o600),
        "portable_source": (args.source_bundle, 0o600),
        "pinned_toolchain": (args.toolchain_bundle, 0o600),
        "portable_source_key": (args.source_signing_key, 0o600),
        "immutable_supervisor": (args.supervisor, 0o500),
    }
    payloads: dict[str, bytes] = {}
    for role, (path, _mode) in inputs.items():
        payloads[role] = stable_regular(
            path,
            maximum=64 if role == "portable_source_key" else MAX_INPUT_BYTES,
            owner_only=role == "portable_source_key",
        )
    if len(payloads["portable_source_key"]) != 32:
        raise PackageError("portable source key must contain exactly 32 bytes")
    if not payloads["immutable_supervisor"].startswith(b"#!"):
        raise PackageError("immutable supervisor is not an executable zipapp")

    appliance = nested_json(
        payloads["cpu_edge_archive"],
        "/BUILD-MANIFEST.json",
        label="cpu_edge_archive",
    )
    if (
        appliance.get("schema") != "astrid_cpu_edge_build_manifest_v3"
        or appliance.get("bundle_format") != "cpu-edge.3"
        or appliance.get("version") != args.version
        or appliance.get("target") != args.target
        or appliance.get("expected_loaded_capsule_count") != 20
    ):
        raise PackageError("CPU-edge archive does not match bootstrap identity")
    generation = nested_json(
        payloads["initial_generation"],
        "/.astrid-edge-generation.json",
        label="initial_generation",
    )
    if (
        generation.get("schema") != "astrid.edge_self_change.initial_generation.v1"
        or generation.get("version") != args.version
        or generation.get("target") != args.target
        or generation.get("authority")
        != "operator_packaged_initial_generation_not_model_candidate"
    ):
        raise PackageError("initial generation does not match bootstrap identity")
    toolchain = nested_json(
        payloads["pinned_toolchain"],
        "/MANIFEST.json",
        label="pinned_toolchain",
    )
    if (
        toolchain.get("schema") != "astrid.edge.self_change_toolchain_bundle.v1"
        or toolchain.get("target") != args.target
    ):
        raise PackageError("pinned toolchain does not match bootstrap target")
    source = nested_json(
        payloads["portable_source"],
        "/MANIFEST.json",
        label="portable_source",
    )
    source_files = source.get("files")
    carried_capsules = {
        PurePosixPath(record["path"]).parts[3]
        for record in source_files
        if isinstance(record, dict)
        and isinstance(record.get("path"), str)
        and len(PurePosixPath(record["path"]).parts) == 5
        and PurePosixPath(record["path"]).parts[:3] == ("source", "capsules", "astralis")
        and PurePosixPath(record["path"]).parts[4] == "Cargo.lock"
    } if isinstance(source_files, list) else set()
    if (
        source.get("schema") != "astrid.edge.self_change_source_bundle.v1"
        or source.get("source_authority") != "portable_bootstrap_non_authorizing"
        or source.get("appliance_id") is not None
        or not isinstance(source.get("rustc"), dict)
        or source["rustc"].get("host") != args.target
        or carried_capsules != EDGE_CAPSULES
    ):
        raise PackageError("portable source bundle has incorrect authority or closure")

    root_name = f"{BUNDLE_PREFIX}-{args.version}-{args.target}"
    installer = stable_regular(args.installer, maximum=2 * 1024 * 1024)
    if not installer.startswith(b"#!/usr/bin/env bash\n"):
        raise PackageError("bootstrap installer has an unexpected interpreter contract")
    payload_names = {
        "cpu_edge_archive": "payload/cpu-edge.tar.gz",
        "initial_generation": "payload/initial-generation.tar.gz",
        "portable_source": "payload/portable-source.tar.gz",
        "pinned_toolchain": "payload/pinned-toolchain.tar.gz",
        "portable_source_key": "payload/portable-source.key",
        "immutable_supervisor": "payload/edge-self-change-supervisor.pyz",
    }
    records = [
        {
            "role": role,
            "path": payload_names[role],
            "bytes": len(data),
            "sha256": sha256(data),
            "mode": f"{inputs[role][1]:04o}",
        }
        for role, data in payloads.items()
    ]
    manifest = {
        "schema": SCHEMA,
        "version": args.version,
        "target": args.target,
        "authority": "operator_release_bootstrap_not_model_authorship_or_appliance_authority",
        "portable_trust": "integrity_only_rebound_to_fresh_per_appliance_key_before_authorization",
        "ordinary_autonomy": "preserved",
        "initial_mode": "paused_bootstrap_acceptance_pending",
        "cross_appliance_or_mac_transfer": "forbidden",
        "profiles": profile_manifest(),
        "payloads": records,
    }
    manifest_bytes = json.dumps(manifest, sort_keys=True, indent=2).encode("utf-8") + b"\n"
    readme = (
        "Astrid CPU-edge self-evolution bootstrap\n\n"
        "This archive is complete and offline. It starts candidate promotion paused.\n"
        "Its SHA256SUMS and sidecar detect corruption; they do not authenticate the publisher.\n"
        "Before extraction or root execution, verify the external GitHub OIDC/Sigstore\n"
        "attestation on a trusted operator host and preserve that verification receipt.\n"
        "Use scripts/verify_edge_self_evolution_release.py --root-install from the trusted\n"
        "operator checkout. It transfers the attested archive, creates the required root-owned\n"
        "handoff over interactive sudo, and invokes this installer from those exact bytes.\n"
        "Direct sudo execution from a user-writable extraction is intentionally rejected.\n\n"
        "The installer never deletes the ICP eMMC backup and never touches Mac Astrid, "
        "Minime, or the spectral bridge.\n"
    ).encode("utf-8")
    members: dict[str, tuple[bytes, int]] = {
        "MANIFEST.json": (manifest_bytes, 0o600),
        "README.txt": (readme, 0o600),
        "install": (installer, 0o500),
    }
    for role, data in payloads.items():
        members[payload_names[role]] = (data, inputs[role][1])
    sums = "".join(
        f"{sha256(data)}  {name}\n"
        for name, (data, _mode) in sorted(members.items())
    ).encode("ascii")
    members["SHA256SUMS"] = (sums, 0o600)

    output = args.output_dir / f"{root_name}.tar.gz"
    sidecar = Path(f"{output}.sha256")
    if output.exists() or output.is_symlink() or sidecar.exists() or sidecar.is_symlink():
        raise PackageError("refusing to overwrite bootstrap output")
    temporary: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            dir=args.output_dir, prefix=".edge-self-evolution-", delete=False
        ) as raw:
            temporary = raw.name
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
                    directory = tarfile.TarInfo(root_name)
                    directory.type = tarfile.DIRTYPE
                    directory.mode = 0o700
                    directory.mtime = 0
                    directory.uid = directory.gid = 0
                    directory.uname = directory.gname = ""
                    archive.addfile(directory)
                    payload_dir = tarfile.TarInfo(f"{root_name}/payload")
                    payload_dir.type = tarfile.DIRTYPE
                    payload_dir.mode = 0o700
                    payload_dir.mtime = 0
                    payload_dir.uid = payload_dir.gid = 0
                    payload_dir.uname = payload_dir.gname = ""
                    archive.addfile(payload_dir)
                    for name, (data, mode) in sorted(members.items()):
                        add_bytes(archive, f"{root_name}/{name}", data, mode)
            raw.flush()
            os.fsync(raw.fileno())
        os.chmod(temporary, 0o600)
        os.link(temporary, output)
        os.unlink(temporary)
        temporary = None
        archive_hash = sha256(stable_regular(output))
        write_exclusive_durable(
            sidecar,
            f"{archive_hash}  {output.name}\n".encode("ascii"),
            0o600,
        )
    finally:
        if temporary is not None:
            Path(temporary).unlink(missing_ok=True)
    return {"archive": str(output), "sha256": archive_hash, "target": args.target}


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--version", required=True)
    result.add_argument("--target", required=True)
    result.add_argument("--appliance-archive", type=Path, required=True)
    result.add_argument("--generation-archive", type=Path, required=True)
    result.add_argument("--source-bundle", type=Path, required=True)
    result.add_argument("--toolchain-bundle", type=Path, required=True)
    result.add_argument("--source-signing-key", type=Path, required=True)
    result.add_argument("--supervisor", type=Path, required=True)
    result.add_argument("--installer", type=Path, required=True)
    result.add_argument("--output-dir", type=Path, required=True)
    return result


def main(argv: list[str] | None = None) -> int:
    try:
        result = build(parser().parse_args(argv))
    except (OSError, PackageError, tarfile.TarError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
