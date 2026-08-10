#!/usr/bin/env python3
"""Build or verify a signed, deterministic Rust 1.94.1 CPU-edge toolchain bundle."""

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
from pathlib import Path, PurePosixPath
from typing import Any

SCHEMA = "astrid.edge.self_change_toolchain_bundle.v1"
SIGNATURE_SCHEMA = "astrid.edge.self_change_toolchain_signature.v1"
BUNDLE_ROOT = "astrid-edge-toolchain"
RUST_RELEASE = "1.94.1"
RUST_COMMIT = "e408947bfd200af42db322daf0fadfe7e26d3bd1"
RUST_DATE = "2026-03-25"
LLVM_VERSION = "21.1.8"
TARGETS = {"x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"}
HEX64 = re.compile(r"[0-9a-f]{64}\Z")
MAX_FILES = 100_000
MAX_BYTES = 4 * 1024 * 1024 * 1024
MAX_MANIFEST = 64 * 1024 * 1024


class BundleError(RuntimeError):
    """Fail-closed bundle validation error."""


def canonical(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False
    ).encode("ascii")


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def safe_relative(value: str) -> str:
    if not value or value.startswith("/") or "\\" in value or "\x00" in value:
        raise BundleError("unsafe toolchain-relative path")
    path = PurePosixPath(value)
    if any(part in {"", ".", ".."} or part.startswith(".") for part in path.parts):
        raise BundleError("unsafe toolchain-relative path")
    return path.as_posix()


def stable_regular(path: Path, limit: int | None = None, owner_only: bool = False) -> bytes:
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or path.is_symlink() or before.st_nlink != 1:
        raise BundleError(f"not a regular unlinked file: {path}")
    if owner_only and before.st_mode & 0o077:
        raise BundleError(f"key is not owner-only: {path}")
    if limit is not None and before.st_size > limit:
        raise BundleError(f"file exceeds bound: {path}")
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    with os.fdopen(descriptor, "rb") as handle:
        opened = os.fstat(handle.fileno())
        data = handle.read() if limit is None else handle.read(limit + 1)
        after = os.fstat(handle.fileno())
    identity = lambda item: (item.st_dev, item.st_ino, item.st_size, item.st_mtime_ns)
    if identity(before) != identity(opened) or identity(opened) != identity(after):
        raise BundleError(f"file changed while read: {path}")
    if len(data) != before.st_size or (limit is not None and len(data) > limit):
        raise BundleError(f"file size changed or exceeded bound: {path}")
    return data


def key_material(path: Path) -> bytes:
    key = stable_regular(path, 64, owner_only=True)
    if len(key) != 32:
        raise BundleError("signing key must contain exactly 32 bytes")
    return key


def command_output(path: Path, argument: str) -> str:
    data = stable_regular(path, 256 * 1024 * 1024)
    if not data or not path.stat().st_mode & stat.S_IXUSR:
        raise BundleError(f"toolchain executable is absent or not executable: {path}")
    try:
        result = subprocess.run(
            [str(path), argument],
            check=False,
            capture_output=True,
            timeout=15,
            env={"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"},
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise BundleError(f"cannot execute pinned toolchain binary: {error}") from error
    if result.returncode != 0 or result.stderr or len(result.stdout) > 16 * 1024:
        raise BundleError(f"pinned toolchain version command failed: {path.name}")
    return result.stdout.decode("utf-8", errors="strict").strip()


def rust_identity(root: Path, target: str) -> dict[str, str]:
    rustc = command_output(root / "bin/rustc", "-Vv")
    cargo = command_output(root / "bin/cargo", "-V")
    expected = {
        "commit-hash": RUST_COMMIT,
        "commit-date": RUST_DATE,
        "host": target,
        "release": RUST_RELEASE,
        "LLVM version": LLVM_VERSION,
    }
    lines = rustc.splitlines()
    if not lines or lines[0] != f"rustc {RUST_RELEASE} ({RUST_COMMIT[:9]} {RUST_DATE})":
        raise BundleError("rustc version line is not the reviewed 1.94.1 toolchain")
    fields: dict[str, str] = {}
    for line in lines[1:]:
        if ": " not in line:
            raise BundleError("malformed rustc verbose version")
        key, value = line.split(": ", 1)
        if key in fields:
            raise BundleError("duplicate rustc identity field")
        fields[key] = value
    if any(fields.get(key) != value for key, value in expected.items()):
        raise BundleError("rustc identity is not the reviewed 1.94.1 target")
    if not cargo.startswith("cargo 1.94.1 "):
        raise BundleError("Cargo is not the reviewed 1.94.1 release")
    wasm = root / "lib/rustlib/wasm32-wasip2"
    if not wasm.is_dir() or wasm.is_symlink():
        raise BundleError("toolchain lacks the required wasm32-wasip2 target")
    return {"rustc": lines[0], "cargo": cargo, **expected}


def inventory(root: Path) -> tuple[list[dict[str, Any]], int]:
    records: list[dict[str, Any]] = []
    total = 0
    for directory, names, files in os.walk(root, topdown=True, followlinks=False):
        names.sort()
        files.sort()
        base = Path(directory)
        for name in [*names, *files]:
            path = base / name
            info = path.lstat()
            if stat.S_ISLNK(info.st_mode) or not (stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode)):
                raise BundleError(f"toolchain contains a link or special file: {path}")
            if stat.S_ISREG(info.st_mode) and info.st_nlink != 1:
                raise BundleError(f"toolchain contains a hard link: {path}")
        for name in files:
            path = base / name
            relative = safe_relative(path.relative_to(root).as_posix())
            data = stable_regular(path)
            total += len(data)
            if len(records) >= MAX_FILES or total > MAX_BYTES:
                raise BundleError("toolchain exceeds bundle bounds")
            records.append(
                {
                    "path": relative,
                    "mode": "0755" if path.stat().st_mode & 0o111 else "0644",
                    "size": len(data),
                    "sha256": digest(data),
                }
            )
    records.sort(key=lambda item: item["path"])
    if not records:
        raise BundleError("toolchain inventory is empty")
    return records, total


def add_bytes(archive: tarfile.TarFile, name: str, data: bytes, mode: int) -> None:
    info = tarfile.TarInfo(f"{BUNDLE_ROOT}/{name}")
    info.size = len(data)
    info.mode = mode
    info.mtime = 0
    info.uid = info.gid = 0
    info.uname = info.gname = ""
    archive.addfile(info, io.BytesIO(data))


def build(args: argparse.Namespace) -> dict[str, Any]:
    if args.target not in TARGETS:
        raise BundleError("unsupported CPU-edge target")
    if args.toolchain_dir.is_symlink() or not args.toolchain_dir.is_dir():
        raise BundleError("toolchain root must be a non-symlink directory")
    root = args.toolchain_dir.resolve()
    identity = rust_identity(root, args.target)
    files, total = inventory(root)
    key = key_material(args.signing_key)
    manifest = {
        "schema": SCHEMA,
        "target": args.target,
        "identity": identity,
        "key_id": digest(key)[:16],
        "file_count": len(files),
        "uncompressed_bytes": total,
        "files": files,
    }
    manifest_bytes = canonical(manifest)
    signature = {
        "schema": SIGNATURE_SCHEMA,
        "algorithm": "hmac-sha256",
        "key_id": digest(key)[:16],
        "manifest_sha256": digest(manifest_bytes),
        "hmac_sha256": hmac.new(key, manifest_bytes, hashlib.sha256).hexdigest(),
    }
    if args.output.exists() or args.output.is_symlink():
        raise BundleError("refusing to overwrite output")
    temporary: str | None = None
    try:
        with tempfile.NamedTemporaryFile(dir=args.output.parent, prefix=".edge-toolchain-", delete=False) as raw:
            temporary = raw.name
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
                    for record in files:
                        add_bytes(
                            archive,
                            f"toolchain/{record['path']}",
                            stable_regular(root / record["path"]),
                            int(record["mode"], 8),
                        )
                    add_bytes(archive, "MANIFEST.json", manifest_bytes + b"\n", 0o600)
                    add_bytes(archive, "MANIFEST.signature.json", canonical(signature) + b"\n", 0o600)
            raw.flush()
            os.fsync(raw.fileno())
        os.chmod(temporary, 0o600)
        os.link(temporary, args.output)
        os.unlink(temporary)
        temporary = None
    finally:
        if temporary is not None:
            Path(temporary).unlink(missing_ok=True)
    return {"bundle": str(args.output), "target": args.target, "file_count": len(files)}


def verify(args: argparse.Namespace) -> dict[str, Any]:
    key = key_material(args.signing_key)
    info = args.bundle.lstat()
    if args.bundle.is_symlink() or not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
        raise BundleError("bundle is not a regular unlinked file")
    with tarfile.open(args.bundle, "r:gz") as archive:
        members: dict[str, tarfile.TarInfo] = {}
        prefix = f"{BUNDLE_ROOT}/"
        for member in archive.getmembers():
            if not member.isfile() or not member.name.startswith(prefix):
                raise BundleError("archive contains an unsafe member")
            relative = safe_relative(member.name.removeprefix(prefix))
            if relative in members or member.uid != 0 or member.gid != 0 or member.mtime != 0:
                raise BundleError("archive metadata is non-deterministic or duplicated")
            members[relative] = member
        required = {"MANIFEST.json", "MANIFEST.signature.json"}
        if not required.issubset(members):
            raise BundleError("bundle lacks manifest or signature")
        manifest_data = archive.extractfile(members["MANIFEST.json"]).read(MAX_MANIFEST + 1)  # type: ignore[union-attr]
        signature_data = archive.extractfile(members["MANIFEST.signature.json"]).read(16_385)  # type: ignore[union-attr]
        manifest = json.loads(manifest_data)
        signature = json.loads(signature_data)
        if manifest_data != canonical(manifest) + b"\n" or signature_data != canonical(signature) + b"\n":
            raise BundleError("manifest or signature is not canonical")
        manifest_bytes = canonical(manifest)
        expected = hmac.new(key, manifest_bytes, hashlib.sha256).hexdigest()
        if (
            manifest.get("schema") != SCHEMA
            or signature.get("schema") != SIGNATURE_SCHEMA
            or signature.get("algorithm") != "hmac-sha256"
            or signature.get("key_id") != digest(key)[:16]
            or manifest.get("key_id") != digest(key)[:16]
            or signature.get("manifest_sha256") != digest(manifest_bytes)
            or not hmac.compare_digest(str(signature.get("hmac_sha256", "")), expected)
        ):
            raise BundleError("toolchain bundle signature is invalid")
        files = manifest.get("files")
        if not isinstance(files, list) or len(files) != manifest.get("file_count") or len(files) > MAX_FILES:
            raise BundleError("toolchain manifest inventory is invalid")
        expected_members = required.copy()
        total = 0
        previous = ""
        for record in files:
            if not isinstance(record, dict) or set(record) != {"path", "mode", "size", "sha256"}:
                raise BundleError("toolchain inventory record is invalid")
            path = safe_relative(str(record["path"]))
            if path <= previous or record["mode"] not in {"0644", "0755"} or not HEX64.fullmatch(str(record["sha256"])):
                raise BundleError("toolchain inventory ordering or metadata is invalid")
            previous = path
            member_name = f"toolchain/{path}"
            expected_members.add(member_name)
            member = members.get(member_name)
            if member is None or member.size != record["size"] or member.mode != int(record["mode"], 8):
                raise BundleError("toolchain archive does not match inventory")
            data = archive.extractfile(member).read(record["size"] + 1)  # type: ignore[union-attr]
            if len(data) != record["size"] or digest(data) != record["sha256"]:
                raise BundleError("toolchain payload hash mismatch")
            total += len(data)
            if total > MAX_BYTES:
                raise BundleError("toolchain payload exceeds byte bound")
        if set(members) != expected_members or total != manifest.get("uncompressed_bytes"):
            raise BundleError("toolchain archive inventory is not exact")
    return {"bundle": str(args.bundle), "target": manifest["target"], "file_count": len(files)}


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    commands = root.add_subparsers(dest="command", required=True)
    build_parser = commands.add_parser("build")
    build_parser.add_argument("--toolchain-dir", type=Path, required=True)
    build_parser.add_argument("--target", required=True)
    build_parser.add_argument("--signing-key", type=Path, required=True)
    build_parser.add_argument("--output", type=Path, required=True)
    verify_parser = commands.add_parser("verify")
    verify_parser.add_argument("--bundle", type=Path, required=True)
    verify_parser.add_argument("--signing-key", type=Path, required=True)
    return root


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        result = build(args) if args.command == "build" else verify(args)
    except (BundleError, OSError, UnicodeError, json.JSONDecodeError, tarfile.TarError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
