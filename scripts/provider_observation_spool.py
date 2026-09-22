#!/usr/bin/env python3
"""Inspect bounded private observer epochs and witness a stopped epoch in place.

Never deletes evidence, changes quotas/configuration, signals processes or selects
a release. A seal is a verifiable inventory, not filesystem write protection.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import datetime as dt
import fcntl
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import uuid

MAX_FILES = 50_000
MAX_BYTES = 268_435_456
MAX_RAW_BYTES = 67_108_864
MAX_MANIFEST_BYTES = 32 * 1024 * 1024
LIMITS = {"files": MAX_FILES, "bytes": MAX_BYTES, "raw_bytes": MAX_RAW_BYTES}


def canonical(path: Path) -> Path:
    if not path.is_absolute() or path.resolve() != path:
        raise ValueError("require an absolute canonical path without symlinks")
    return path


def private(path: Path, *, directory: bool = False):
    metadata = path.lstat()
    kind = stat.S_ISDIR if directory else stat.S_ISREG
    mode = 0o700 if directory else 0o600
    if (not kind(metadata.st_mode) or stat.S_IMODE(metadata.st_mode) != mode
            or metadata.st_uid != os.getuid() or (not directory and metadata.st_nlink != 1)):
        raise ValueError("expected owned private directory or single-link regular file")
    return metadata


def identity(metadata):
    return (metadata.st_dev, metadata.st_ino, metadata.st_size,
            metadata.st_mtime_ns, metadata.st_ctime_ns)


def encoded(value):
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def sync_directory(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def publish_new(path: Path, data: bytes):
    canonical(path)
    private(path.parent, directory=True)
    temporary = path.with_name(f".{path.name}.{uuid.uuid4().hex}.pending")
    fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "wb") as handle:
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())
    # Exclusive publication: never replace an earlier witness, including on retry.
    os.link(temporary, path)
    temporary.unlink()
    sync_directory(path.parent)


def entries(root: Path):
    canonical(root)
    private(root, directory=True)
    result = []
    for lane in ("events", "raw"):
        directory = root / lane
        private(directory, directory=True)
        with os.scandir(directory) as scan:
            for item in scan:
                if len(result) >= MAX_FILES:
                    raise ValueError("inventory exceeds the observer file bound; review manually")
                path = Path(item.path)
                result.append((f"{lane}/{item.name}", private(path)))
    return sorted(result, key=lambda item: item[0])


def capacity(items):
    usage = {"files": len(items), "bytes": sum(m.st_size for _, m in items),
             "raw_bytes": sum(m.st_size for name, m in items if name.startswith("raw/"))}
    ratio = max(usage[key] / limit for key, limit in LIMITS.items())
    return {"usage": usage, "limits": LIMITS,
            "status": "full" if ratio >= 1 else "near_capacity" if ratio >= 0.8 else "available",
            "at_limit": [key for key, limit in LIMITS.items() if usage[key] >= limit],
            "usage_fraction": ratio,
            "remaining": {key: max(0, limit - usage[key]) for key, limit in LIMITS.items()},
            "scope": "on-disk snapshot; writer reservations and failed-write charges unavailable",
            "partial_files": sum(name.endswith(".tmp") for name, _ in items)}


@contextmanager
def stopped_writer(root: Path):
    canonical(root)
    private(root, directory=True)
    path = root / "writer.lock"
    before = private(path)
    fd = os.open(path, os.O_RDWR | os.O_NOFOLLOW)
    try:
        if identity(os.fstat(fd)) != identity(before):
            raise ValueError("writer lock changed")
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise ValueError("writer active; seal refused without changing evidence") from error
        yield
        if identity(private(path)) != identity(before):
            raise ValueError("writer lock changed during inventory")
    finally:
        os.close(fd)


def inventory(root: Path):
    initial = entries(root)
    if sum(m.st_size for _, m in initial) > MAX_BYTES:
        raise ValueError("inventory exceeds the observer byte bound; review manually")
    records = []
    for name, metadata in initial:
        digest = hashlib.sha256()
        fd = os.open(root / name, os.O_RDONLY | os.O_NOFOLLOW)
        with os.fdopen(fd, "rb") as handle:
            if identity(os.fstat(handle.fileno())) != identity(metadata):
                raise ValueError("evidence changed during inventory")
            remaining = metadata.st_size
            while remaining:
                chunk = handle.read(min(remaining, 1024 * 1024))
                if not chunk:
                    raise ValueError("evidence truncated during inventory")
                remaining -= len(chunk)
                digest.update(chunk)
            if handle.read(1) or identity(os.fstat(handle.fileno())) != identity(metadata):
                raise ValueError("evidence changed during inventory")
        records.append({"path": name, "bytes": metadata.st_size, "sha256": digest.hexdigest()})
    if [(n, identity(m)) for n, m in initial] != [(n, identity(m)) for n, m in entries(root)]:
        raise ValueError("epoch changed during inventory")
    return {"capacity": capacity(initial), "files": records}


def prepare(root: Path, previous: Path):
    canonical(root)
    canonical(previous)
    private(previous, directory=True)
    if root.parent != previous.parent or root == previous:
        raise ValueError("new epoch must be an unused sibling of its predecessor")
    parent = root.parent.lstat()
    if parent.st_uid != os.getuid() or stat.S_IMODE(parent.st_mode) & 0o022:
        raise ValueError("epoch parent must be owned and not group/world writable")
    root.mkdir(mode=0o700, exist_ok=False)
    for lane in ("events", "raw"):
        (root / lane).mkdir(mode=0o700)
    fd = os.open(root / "writer.lock", os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    os.fsync(fd)
    os.close(fd)
    record = {"schema": "provider_observation_epoch_v1", "root": str(root),
              "predecessor": str(previous), "limits": LIMITS,
              "created_at": dt.datetime.now(dt.UTC).isoformat(),
              "predecessor_sealed": False, "configuration_changed": False,
              "scope": "prepared directory only; activation and predecessor seal separately verified"}
    publish_new(root / "epoch.json", encoded(record))
    sync_directory(root.parent)
    return record


def seal(root: Path, manifest: Path):
    canonical(manifest)
    if manifest == root or root in manifest.parents:
        raise ValueError("seal manifest must be outside the evidence epoch")
    with stopped_writer(root):
        record = {"schema": "provider_observation_seal_v1", "root": str(root),
                  "created_at": dt.datetime.now(dt.UTC).isoformat(),
                  "scope": "stopped-writer byte inventory; not content validation or write protection",
                  **inventory(root)}
        data = encoded(record)
        if len(data) > MAX_MANIFEST_BYTES:
            raise ValueError("seal exceeds manifest bound")
        publish_new(manifest, data)
    return {"manifest": str(manifest), "sha256": hashlib.sha256(data).hexdigest(),
            "capacity": record["capacity"]}


def verify(root: Path, manifest: Path):
    canonical(manifest)
    metadata = private(manifest)
    if metadata.st_size > MAX_MANIFEST_BYTES:
        raise ValueError("seal exceeds manifest bound")
    fd = os.open(manifest, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, "rb") as handle:
        data = handle.read(MAX_MANIFEST_BYTES + 1)
    if len(data) > MAX_MANIFEST_BYTES:
        raise ValueError("seal exceeds manifest bound")
    record = json.loads(data)
    if (not isinstance(record, dict) or record.get("schema") != "provider_observation_seal_v1"
            or record.get("root") != str(root)):
        raise ValueError("seal does not identify this epoch")
    with stopped_writer(root):
        actual = inventory(root)
        if any(record.get(key) != value for key, value in actual.items()):
            raise ValueError("evidence differs from seal; preserve both and investigate")
    return {"verified": True, "manifest": str(manifest),
            "sha256": hashlib.sha256(data).hexdigest(), "capacity": actual["capacity"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    for command in ("status", "prepare", "seal", "verify-seal"):
        child = sub.add_parser(command)
        child.add_argument("--root", type=Path, required=True)
        if command == "prepare":
            child.add_argument("--previous", type=Path, required=True)
        if command in {"seal", "verify-seal"}:
            child.add_argument("--manifest", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "status":
            result = {"root": str(args.root), **capacity(entries(args.root)), "writer_quiescence_verified": False}
        elif args.command == "prepare":
            result = prepare(args.root, args.previous)
        elif args.command == "seal":
            result = seal(args.root, args.manifest)
        else:
            result = verify(args.root, args.manifest)
        print(json.dumps(result, indent=2))
        return 0
    except (OSError, ValueError, TypeError) as error:
        # Never print file bytes, raw responses or serialized evidence on error.
        print(json.dumps({"ok": False, "error": str(error)[:300]}), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
