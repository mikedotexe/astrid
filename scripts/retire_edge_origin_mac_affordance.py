#!/usr/bin/env python3
"""Durably retire exact legacy CPU-edge prompts that advertised ``origin-mac``.

The correction is a small root-owned transaction.  Source files are read and
removed relative to already-open directory descriptors and their verified
bytes are copied to a new root-owned inode.  A durable plan makes every crash
boundary recoverable; a separately published receipt is the commit record.

This migration deliberately does not search or rewrite Astrid-authored history,
databases, journals, memories, artifacts, or the operator quarantine.  Unknown
content that still advertises origin-mac fails closed for operator review.

The transaction implementation is intentionally kept in this one dependency-
free shipped bootstrap program: splitting security-critical pathname and crash
recovery helpers into an ambient import would weaken package attestation and
isolated ``python -I`` execution.  Its focused fault suite covers the resulting
larger, deliberately cohesive file.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
from collections.abc import Callable
from pathlib import Path, PurePosixPath
from typing import Any


SCHEMA = "astrid.edge.origin_mac_affordance_retirement.v2"
TRANSACTION_SCHEMA = "astrid.edge.origin_mac_affordance_retirement.transaction.v1"
TRANSACTION_NAME = "transaction.json"
TRANSACTION_PENDING_NAME = ".transaction.json.pending"
RECEIPT_NAME = "receipt.json"
RECEIPT_PENDING_NAME = ".receipt.json.pending"
OPERATOR_RECEIPT_NAME = "origin-mac-affordance-retirement.json"
KNOWN_LEGACY = {
    "29852a4aaaf9a62079caa0752f98f6a25091a3e65323d8075301fcfd9ff52266",
    "caeb4e63986d7762e80a4d7b958a2190deb9627774c00b3a835f4902ab61425f",
}
RELATIVE_CANDIDATES = (
    "AGENTS.md",
    "MEMORY.md",
    "memory.md",
    "edge/AGENTS.md",
    "edge/MEMORY.md",
    "edge/memory.md",
)
HEX64 = re.compile(r"[0-9a-f]{64}\Z")
MAXIMUM_SOURCE_BYTES = 128 * 1024
FaultHook = Callable[[str], None]


class MigrationError(RuntimeError):
    pass


def canonical_json(value: dict[str, Any]) -> bytes:
    try:
        return json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise MigrationError("transaction value is not canonical JSON") from error


def hash_bound(value: dict[str, Any], field: str) -> dict[str, Any]:
    result = dict(value)
    result[field] = hashlib.sha256(canonical_json(value)).hexdigest()
    return result


def verify_hash_bound(value: Any, field: str, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise MigrationError(f"{label} is not an object")
    claimed = value.get(field)
    if not isinstance(claimed, str) or HEX64.fullmatch(claimed) is None:
        raise MigrationError(f"{label} hash is malformed")
    core = {key: item for key, item in value.items() if key != field}
    if hashlib.sha256(canonical_json(core)).hexdigest() != claimed:
        raise MigrationError(f"{label} hash binding is invalid")
    return value


def exact_absolute(path: Path, label: str) -> None:
    raw = os.fspath(path)
    if (
        not path.is_absolute()
        or raw != os.path.normpath(raw)
        or any(part in {"", ".", ".."} for part in PurePosixPath(raw).parts[1:])
    ):
        raise MigrationError(f"{label} must be an exact absolute path")


def directory_flags() -> int:
    return (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0)
    )


def fsync_directory(descriptor: int) -> None:
    os.fsync(descriptor)


def controlled_directory(metadata: os.stat_result, authority_uid: int) -> bool:
    return (
        stat.S_ISDIR(metadata.st_mode)
        # Unit tests run the same policy below a private directory owned by the
        # invoking developer.  In production ``authority_uid`` is zero, so
        # this still reduces to root-only ancestry.
        and metadata.st_uid in {0, authority_uid}
        and metadata.st_mode & 0o022 == 0
    )


def open_absolute_directory(
    path: Path,
    *,
    label: str,
    authority_uid: int,
    require_controlled_ancestry: bool,
) -> int:
    """Open every component with O_NOFOLLOW, optionally requiring root control."""

    exact_absolute(path, label)
    descriptor = os.open("/", directory_flags())
    try:
        root_metadata = os.fstat(descriptor)
        if require_controlled_ancestry and not controlled_directory(
            root_metadata, authority_uid
        ):
            raise MigrationError(f"{label} root ancestry is not authority-controlled")
        for part in path.parts[1:]:
            try:
                next_descriptor = os.open(part, directory_flags(), dir_fd=descriptor)
            except OSError as error:
                raise MigrationError(
                    f"{label} contains an absent, linked, or non-directory component"
                ) from error
            os.close(descriptor)
            descriptor = next_descriptor
            metadata = os.fstat(descriptor)
            if require_controlled_ancestry and not controlled_directory(
                metadata, authority_uid
            ):
                raise MigrationError(f"{label} ancestry is not authority-controlled")
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def ensure_controlled_leaf_directory(
    path: Path,
    *,
    label: str,
    authority_uid: int,
    authority_gid: int,
    mode: int,
    existing_group: int | None = None,
) -> int:
    """Create only the final component below verified root-controlled ancestry."""

    exact_absolute(path, label)
    parent = path.parent
    parent_fd = open_absolute_directory(
        parent,
        label=f"{label} parent",
        authority_uid=authority_uid,
        require_controlled_ancestry=True,
    )
    try:
        try:
            os.mkdir(path.name, mode=mode, dir_fd=parent_fd)
            created = True
        except FileExistsError:
            created = False
        descriptor = os.open(path.name, directory_flags(), dir_fd=parent_fd)
        metadata = os.fstat(descriptor)
        expected_gid = authority_gid if existing_group is None else existing_group
        if created:
            os.fchown(descriptor, authority_uid, expected_gid)
            os.fchmod(descriptor, mode)
            fsync_directory(descriptor)
            fsync_directory(parent_fd)
            metadata = os.fstat(descriptor)
        if (
            not controlled_directory(metadata, authority_uid)
            or metadata.st_gid != expected_gid
            or stat.S_IMODE(metadata.st_mode) != mode
        ):
            os.close(descriptor)
            raise MigrationError(f"{label} has an unsafe identity or mode")
        return descriptor
    finally:
        os.close(parent_fd)


def open_relative_parent(root_fd: int, relative: str) -> tuple[int, str]:
    parts = PurePosixPath(relative).parts
    if (
        not parts
        or any(part in {"", ".", ".."} for part in parts)
        or str(PurePosixPath(*parts)) != relative
    ):
        raise MigrationError(f"candidate path is not canonical: {relative}")
    descriptor = os.dup(root_fd)
    try:
        for part in parts[:-1]:
            try:
                next_descriptor = os.open(part, directory_flags(), dir_fd=descriptor)
            except FileNotFoundError:
                os.close(descriptor)
                return -1, parts[-1]
            except OSError as error:
                raise MigrationError(
                    f"candidate parent is linked or non-directory: {relative}"
                ) from error
            os.close(descriptor)
            descriptor = next_descriptor
        return descriptor, parts[-1]
    except BaseException:
        if descriptor >= 0:
            os.close(descriptor)
        raise


def regular_file_metadata(
    metadata: os.stat_result,
    label: str,
    *,
    require_single_link: bool = True,
) -> None:
    if not stat.S_ISREG(metadata.st_mode) or (
        require_single_link and metadata.st_nlink != 1
    ):
        raise MigrationError(f"{label} is linked or non-regular")


def read_descriptor(descriptor: int, maximum: int, label: str) -> bytes:
    metadata = os.fstat(descriptor)
    regular_file_metadata(metadata, label)
    if metadata.st_size < 0 or metadata.st_size > maximum:
        raise MigrationError(f"{label} is oversized")
    os.lseek(descriptor, 0, os.SEEK_SET)
    chunks: list[bytes] = []
    length = 0
    while True:
        block = os.read(descriptor, min(65_536, maximum + 1 - length))
        if not block:
            break
        chunks.append(block)
        length += len(block)
        if length > maximum:
            raise MigrationError(f"{label} is oversized")
    data = b"".join(chunks)
    if len(data) != metadata.st_size:
        raise MigrationError(f"{label} changed during read")
    return data


def open_relative_regular(parent_fd: int, name: str, label: str) -> int:
    try:
        descriptor = os.open(
            name,
            os.O_RDONLY
            | getattr(os, "O_NOFOLLOW", 0)
            | getattr(os, "O_CLOEXEC", 0),
            dir_fd=parent_fd,
        )
    except OSError as error:
        raise MigrationError(f"{label} cannot be opened without following links") from error
    try:
        regular_file_metadata(os.fstat(descriptor), label)
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def relative_lstat(parent_fd: int, name: str) -> os.stat_result | None:
    try:
        return os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return None


def require_file_identity_at(
    directory_fd: int,
    name: str,
    *,
    uid: int,
    gid: int,
    mode: int,
    label: str,
) -> None:
    metadata = relative_lstat(directory_fd, name)
    if (
        metadata is None
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_uid != uid
        or metadata.st_gid != gid
        or stat.S_IMODE(metadata.st_mode) != mode
    ):
        raise MigrationError(f"{label} identity is invalid")


def read_json_file_at(
    directory_fd: int, name: str, *, label: str, maximum: int = 128 * 1024
) -> tuple[dict[str, Any], bytes]:
    descriptor = open_relative_regular(directory_fd, name, label)
    try:
        metadata = os.fstat(descriptor)
        data = read_descriptor(descriptor, maximum, label)
        if metadata.st_uid != 0 and os.geteuid() == 0:
            raise MigrationError(f"{label} is not root-owned")
        if metadata.st_mode & 0o022:
            raise MigrationError(f"{label} is writable outside its owner")
    finally:
        os.close(descriptor)
    try:
        decoded = json.loads(data)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise MigrationError(f"{label} is not valid JSON") from error
    if not isinstance(decoded, dict):
        raise MigrationError(f"{label} is not a JSON object")
    return decoded, data


def write_all(descriptor: int, data: bytes) -> None:
    view = memoryview(data)
    while view:
        written = os.write(descriptor, view)
        if written <= 0:
            raise MigrationError("durable write made no progress")
        view = view[written:]


def publish_once_at(
    directory_fd: int,
    *,
    pending_name: str,
    final_name: str,
    payload: bytes,
    uid: int,
    gid: int,
    mode: int,
    fault: FaultHook,
    fault_label: str,
) -> None:
    if relative_lstat(directory_fd, final_name) is not None:
        raise MigrationError(f"{final_name} already exists")
    if relative_lstat(directory_fd, pending_name) is not None:
        raise MigrationError(f"stale {pending_name} already exists")
    descriptor = os.open(
        pending_name,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0),
        mode,
        dir_fd=directory_fd,
    )
    try:
        os.fchown(descriptor, uid, gid)
        os.fchmod(descriptor, mode)
        write_all(descriptor, payload)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    fault(f"{fault_label}_pending_fsynced")
    os.rename(
        pending_name,
        final_name,
        src_dir_fd=directory_fd,
        dst_dir_fd=directory_fd,
    )
    fsync_directory(directory_fd)
    fault(f"{fault_label}_published")


def promote_valid_pending(
    directory_fd: int,
    *,
    pending_name: str,
    final_name: str,
    validator: Callable[[dict[str, Any]], None],
    label: str,
    uid: int,
    gid: int,
    mode: int,
) -> None:
    pending = relative_lstat(directory_fd, pending_name)
    final = relative_lstat(directory_fd, final_name)
    if pending is None:
        return
    if final is not None:
        raise MigrationError(f"both pending and committed {label} exist")
    require_file_identity_at(
        directory_fd,
        pending_name,
        uid=uid,
        gid=gid,
        mode=mode,
        label=f"pending {label}",
    )
    value, _ = read_json_file_at(directory_fd, pending_name, label=f"pending {label}")
    validator(value)
    os.rename(
        pending_name,
        final_name,
        src_dir_fd=directory_fd,
        dst_dir_fd=directory_fd,
    )
    fsync_directory(directory_fd)


def source_state(
    workspace_fd: int, relative: str, maximum: int = MAXIMUM_SOURCE_BYTES
) -> tuple[os.stat_result, bytes] | None:
    parent_fd, name = open_relative_parent(workspace_fd, relative)
    if parent_fd < 0:
        return None
    try:
        metadata = relative_lstat(parent_fd, name)
        if metadata is None:
            return None
        descriptor = open_relative_regular(parent_fd, name, f"candidate {relative}")
        try:
            opened = os.fstat(descriptor)
            if (
                opened.st_dev,
                opened.st_ino,
                opened.st_size,
                opened.st_nlink,
            ) != (
                metadata.st_dev,
                metadata.st_ino,
                metadata.st_size,
                metadata.st_nlink,
            ):
                raise MigrationError(f"candidate changed before open: {relative}")
            data = read_descriptor(descriptor, maximum, f"candidate {relative}")
            after = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
            if (
                after.st_dev,
                after.st_ino,
                after.st_size,
                after.st_nlink,
            ) != (
                opened.st_dev,
                opened.st_ino,
                opened.st_size,
                opened.st_nlink,
            ):
                raise MigrationError(f"candidate changed during read: {relative}")
            return opened, data
        finally:
            os.close(descriptor)
    finally:
        os.close(parent_fd)


def validate_transaction(
    value: dict[str, Any], workspace: Path, retirement: Path
) -> None:
    verify_hash_bound(value, "transaction_sha256", "origin-mac transaction")
    if (
        value.get("schema") != TRANSACTION_SCHEMA
        or value.get("workspace_root") != str(workspace)
        or value.get("retirement_root") != str(retirement)
        or value.get("known_legacy_sha256") != sorted(KNOWN_LEGACY)
        or value.get("scope") != "exact_prompt_context_candidates_only"
    ):
        raise MigrationError("origin-mac transaction scope or identity is invalid")
    items = value.get("items")
    outcomes = value.get("outcomes")
    if not isinstance(items, list) or not isinstance(outcomes, list):
        raise MigrationError("origin-mac transaction collections are malformed")
    seen_paths: set[str] = set()
    seen_destinations: set[str] = set()
    for item in items:
        if not isinstance(item, dict) or set(item) != {
            "path",
            "sha256",
            "size",
            "source_dev",
            "source_ino",
            "destination",
            "copy_pending",
            "source_disposal",
        }:
            raise MigrationError("origin-mac transaction item is malformed")
        relative = item.get("path")
        digest = item.get("sha256")
        destination = item.get("destination")
        pending = item.get("copy_pending")
        if (
            relative not in RELATIVE_CANDIDATES
            or relative in seen_paths
            or not isinstance(digest, str)
            or digest not in KNOWN_LEGACY
            or not isinstance(destination, str)
            or PurePosixPath(destination).name != destination
            or destination in seen_destinations
            or pending != f".copy.{destination}"
            or item.get("source_disposal") != f".dispose.{destination}"
            or not isinstance(item.get("size"), int)
            or item["size"] < 0
            or item["size"] > MAXIMUM_SOURCE_BYTES
            or not isinstance(item.get("source_dev"), int)
            or not isinstance(item.get("source_ino"), int)
        ):
            raise MigrationError("origin-mac transaction item escaped policy")
        seen_paths.add(relative)
        seen_destinations.add(destination)
    if any(
        not isinstance(item, dict)
        or item.get("path") not in RELATIVE_CANDIDATES
        or item.get("status")
        not in {
            "absent",
            "preserved_not_legacy",
            "retired_exact_known_legacy_affordance",
            "same_file_alias_handled_by_primary_candidate",
        }
        for item in outcomes
    ):
        raise MigrationError("origin-mac transaction outcomes are malformed")


def validate_retired_artifact(
    retirement_fd: int,
    item: dict[str, Any],
    *,
    authority_uid: int,
    authority_gid: int,
) -> None:
    name = str(item["destination"])
    descriptor = open_relative_regular(retirement_fd, name, f"retired artifact {name}")
    try:
        metadata = os.fstat(descriptor)
        data = read_descriptor(descriptor, MAXIMUM_SOURCE_BYTES, f"retired artifact {name}")
        if (
            metadata.st_uid != authority_uid
            or metadata.st_gid != authority_gid
            or stat.S_IMODE(metadata.st_mode) != 0o600
            or metadata.st_nlink != 1
            or metadata.st_size != item["size"]
            or hashlib.sha256(data).hexdigest() != item["sha256"]
        ):
            raise MigrationError(f"retired artifact identity or digest mismatch: {name}")
    finally:
        os.close(descriptor)


def source_matches_item(
    workspace_fd: int, item: dict[str, Any]
) -> tuple[bool, bool]:
    state = source_state(workspace_fd, str(item["path"]))
    if state is None:
        return False, False
    metadata, data = state
    exact = (
        metadata.st_dev == item["source_dev"]
        and metadata.st_ino == item["source_ino"]
        and metadata.st_size == item["size"]
        and metadata.st_nlink == 1
        and hashlib.sha256(data).hexdigest() == item["sha256"]
    )
    return True, exact


def copy_and_publish_artifact(
    workspace_fd: int,
    retirement_fd: int,
    item: dict[str, Any],
    *,
    authority_uid: int,
    authority_gid: int,
    fault: FaultHook,
) -> None:
    destination = str(item["destination"])
    pending = str(item["copy_pending"])
    destination_metadata = relative_lstat(retirement_fd, destination)
    pending_metadata = relative_lstat(retirement_fd, pending)

    if destination_metadata is not None:
        if pending_metadata is not None:
            # The only valid two-name state is the crash window between link
            # publication and removal of the private staging name.
            destination_stat = os.stat(
                destination, dir_fd=retirement_fd, follow_symlinks=False
            )
            pending_stat = os.stat(pending, dir_fd=retirement_fd, follow_symlinks=False)
            if (
                destination_stat.st_dev,
                destination_stat.st_ino,
                destination_stat.st_nlink,
                pending_stat.st_dev,
                pending_stat.st_ino,
                pending_stat.st_nlink,
            ) != (
                pending_stat.st_dev,
                pending_stat.st_ino,
                2,
                destination_stat.st_dev,
                destination_stat.st_ino,
                2,
            ):
                raise MigrationError(f"artifact staging alias is invalid: {destination}")
            os.unlink(pending, dir_fd=retirement_fd)
            fsync_directory(retirement_fd)
            validate_retired_artifact(
                retirement_fd,
                item,
                authority_uid=authority_uid,
                authority_gid=authority_gid,
            )
        else:
            validate_retired_artifact(
                retirement_fd,
                item,
                authority_uid=authority_uid,
                authority_gid=authority_gid,
            )
        return

    if pending_metadata is not None:
        # A partial write has size below the exact plan and is recoverable.  A
        # full-sized wrong value is indistinguishable from tampering and fails.
        if (
            not stat.S_ISREG(pending_metadata.st_mode)
            or pending_metadata.st_uid != authority_uid
            or pending_metadata.st_gid != authority_gid
            or stat.S_IMODE(pending_metadata.st_mode) != 0o600
            or pending_metadata.st_nlink != 1
        ):
            raise MigrationError(f"artifact staging identity is invalid: {pending}")
        descriptor = open_relative_regular(
            retirement_fd, pending, f"artifact staging {pending}"
        )
        try:
            data = read_descriptor(
                descriptor, MAXIMUM_SOURCE_BYTES, f"artifact staging {pending}"
            )
        finally:
            os.close(descriptor)
        if len(data) == item["size"]:
            if hashlib.sha256(data).hexdigest() != item["sha256"]:
                raise MigrationError(f"artifact staging digest mismatch: {pending}")
            os.link(
                pending,
                destination,
                src_dir_fd=retirement_fd,
                dst_dir_fd=retirement_fd,
                follow_symlinks=False,
            )
            fsync_directory(retirement_fd)
            os.unlink(pending, dir_fd=retirement_fd)
            fsync_directory(retirement_fd)
            validate_retired_artifact(
                retirement_fd,
                item,
                authority_uid=authority_uid,
                authority_gid=authority_gid,
            )
            return
        else:
            os.unlink(pending, dir_fd=retirement_fd)
            fsync_directory(retirement_fd)

    parent_fd, source_name = open_relative_parent(workspace_fd, str(item["path"]))
    if parent_fd < 0:
        raise MigrationError(f"source disappeared before artifact copy: {item['path']}")
    try:
        source_fd = open_relative_regular(
            parent_fd, source_name, f"candidate {item['path']}"
        )
        try:
            source_metadata = os.fstat(source_fd)
            source_data = read_descriptor(
                source_fd, MAXIMUM_SOURCE_BYTES, f"candidate {item['path']}"
            )
            if (
                source_metadata.st_dev != item["source_dev"]
                or source_metadata.st_ino != item["source_ino"]
                or source_metadata.st_size != item["size"]
                or source_metadata.st_nlink != 1
                or hashlib.sha256(source_data).hexdigest() != item["sha256"]
            ):
                raise MigrationError(f"candidate changed after durable plan: {item['path']}")
            descriptor = os.open(
                pending,
                os.O_WRONLY
                | os.O_CREAT
                | os.O_EXCL
                | getattr(os, "O_NOFOLLOW", 0)
                | getattr(os, "O_CLOEXEC", 0),
                0o600,
                dir_fd=retirement_fd,
            )
            try:
                os.fchown(descriptor, authority_uid, authority_gid)
                os.fchmod(descriptor, 0o600)
                write_all(descriptor, source_data)
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
        finally:
            os.close(source_fd)
    finally:
        os.close(parent_fd)
    fault("artifact_copy_fsynced")
    os.link(
        pending,
        destination,
        src_dir_fd=retirement_fd,
        dst_dir_fd=retirement_fd,
        follow_symlinks=False,
    )
    fsync_directory(retirement_fd)
    fault("artifact_published_with_staging_link")
    os.unlink(pending, dir_fd=retirement_fd)
    fsync_directory(retirement_fd)
    validate_retired_artifact(
        retirement_fd,
        item,
        authority_uid=authority_uid,
        authority_gid=authority_gid,
    )


def restore_disposal_if_source_absent(
    workspace_fd: int,
    retirement_fd: int,
    item: dict[str, Any],
) -> bool:
    parent_fd, name = open_relative_parent(workspace_fd, str(item["path"]))
    try:
        if parent_fd < 0 or relative_lstat(parent_fd, name) is not None:
            return False
        os.rename(
            str(item["source_disposal"]),
            name,
            src_dir_fd=retirement_fd,
            dst_dir_fd=parent_fd,
        )
        fsync_directory(retirement_fd)
        fsync_directory(parent_fd)
        return True
    finally:
        if parent_fd >= 0:
            os.close(parent_fd)


def disposal_matches_transaction(
    retirement_fd: int, item: dict[str, Any]
) -> bool:
    try:
        descriptor = open_relative_regular(
            retirement_fd,
            str(item["source_disposal"]),
            f"source disposal {item['path']}",
        )
    except MigrationError:
        return False
    try:
        metadata = os.fstat(descriptor)
        data = read_descriptor(
            descriptor, MAXIMUM_SOURCE_BYTES, f"source disposal {item['path']}"
        )
        return (
            metadata.st_dev == item["source_dev"]
            and metadata.st_ino == item["source_ino"]
            and metadata.st_size == item["size"]
            and hashlib.sha256(data).hexdigest() == item["sha256"]
        )
    except MigrationError:
        return False
    finally:
        os.close(descriptor)


def unlink_exact_source(
    workspace_fd: int,
    retirement_fd: int,
    item: dict[str, Any],
    *,
    fault: FaultHook,
) -> None:
    disposal = str(item["source_disposal"])
    disposal_metadata = relative_lstat(retirement_fd, disposal)
    if disposal_metadata is not None:
        if not disposal_matches_transaction(retirement_fd, item):
            restore_disposal_if_source_absent(
                workspace_fd, retirement_fd, item
            )
            raise MigrationError(f"source disposal is not exact: {item['path']}")
        os.unlink(disposal, dir_fd=retirement_fd)
        fsync_directory(retirement_fd)
        fault("source_disposal_unlinked")
        return

    parent_fd, name = open_relative_parent(workspace_fd, str(item["path"]))
    if parent_fd < 0:
        return
    try:
        metadata = relative_lstat(parent_fd, name)
        if metadata is None:
            return
        descriptor = open_relative_regular(parent_fd, name, f"candidate {item['path']}")
        try:
            opened = os.fstat(descriptor)
            data = read_descriptor(
                descriptor, MAXIMUM_SOURCE_BYTES, f"candidate {item['path']}"
            )
            if (
                opened.st_dev != item["source_dev"]
                or opened.st_ino != item["source_ino"]
                or opened.st_size != item["size"]
                or opened.st_nlink != 1
                or hashlib.sha256(data).hexdigest() != item["sha256"]
            ):
                raise MigrationError(f"candidate changed before unlink: {item['path']}")
            fault("before_source_unlink_identity_recheck")
            rebound = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
            if (
                rebound.st_dev,
                rebound.st_ino,
                rebound.st_size,
                rebound.st_nlink,
            ) != (
                opened.st_dev,
                opened.st_ino,
                opened.st_size,
                opened.st_nlink,
            ):
                raise MigrationError(f"candidate path changed before unlink: {item['path']}")
            fault("after_source_identity_recheck_before_move")
            # Atomically move the exact pathname into the root-controlled
            # retirement directory before deleting anything.  A concurrent
            # replacement is either detected here and restored, or remains at
            # the source name and is never selected for deletion.
            os.rename(
                name,
                disposal,
                src_dir_fd=parent_fd,
                dst_dir_fd=retirement_fd,
            )
            fsync_directory(parent_fd)
            fsync_directory(retirement_fd)
        finally:
            os.close(descriptor)
    finally:
        os.close(parent_fd)
    fault("source_moved_and_parent_fsynced")
    if not disposal_matches_transaction(retirement_fd, item):
        # Restore a raced-in replacement rather than deleting it.  This uses
        # the still root-controlled disposal name as the recovery source and
        # refuses to overwrite any newer source entry.
        restore_disposal_if_source_absent(workspace_fd, retirement_fd, item)
        raise MigrationError(f"source disposal is not exact: {item['path']}")
    os.unlink(disposal, dir_fd=retirement_fd)
    fsync_directory(retirement_fd)
    fault("source_disposal_unlinked")


def receipt_from_transaction(transaction: dict[str, Any]) -> dict[str, Any]:
    inventory = [
        {
            "path": item["path"],
            "name": item["destination"],
            "sha256": item["sha256"],
            "size": item["size"],
            "uid": 0,
            "gid": 0,
            "mode": "0600",
        }
        for item in transaction["items"]
    ]
    core: dict[str, Any] = {
        "schema": SCHEMA,
        "transaction_schema": TRANSACTION_SCHEMA,
        "transaction_sha256": transaction["transaction_sha256"],
        "workspace_root": transaction["workspace_root"],
        "retirement_root": transaction["retirement_root"],
        "scope": "exact_prompt_context_candidates_only",
        "known_legacy_sha256": sorted(KNOWN_LEGACY),
        "outcomes": transaction["outcomes"],
        "artifact_inventory": inventory,
        "completion_status": "durable_retirement_committed",
        "historical_authored_files_modified": False,
        "operator_quarantine_modified": False,
        "authority": "root_bootstrap_migration_no_authorship_or_memory_claim",
    }
    return hash_bound(core, "receipt_sha256")


def validate_receipt(
    receipt: dict[str, Any], transaction: dict[str, Any]
) -> None:
    verify_hash_bound(receipt, "receipt_sha256", "origin-mac retirement receipt")
    expected = receipt_from_transaction(transaction)
    if receipt != expected:
        raise MigrationError("origin-mac retirement receipt does not match its transaction")


def verify_transaction_completion(
    workspace_fd: int,
    retirement_fd: int,
    transaction: dict[str, Any],
    *,
    authority_uid: int,
    authority_gid: int,
) -> None:
    for item in transaction["items"]:
        validate_retired_artifact(
            retirement_fd,
            item,
            authority_uid=authority_uid,
            authority_gid=authority_gid,
        )
        present, _ = source_matches_item(workspace_fd, item)
        if present:
            raise MigrationError(f"retired source reappeared: {item['path']}")


def reevaluate_unretired_candidates(
    workspace_fd: int, transaction: dict[str, Any]
) -> None:
    """Re-scan unretired prompt slots without freezing ordinary future edits.

    Only an entry copied into ``items`` is a permanent retirement invariant.
    Historical absent/preserved outcomes describe the bootstrap that created
    the transaction; they never pin a live AGENTS/MEMORY file's later digest.
    Any later origin-mac advertisement still fails closed.  A newly appearing
    exact-known legacy file is distinguished from unknown content for explicit
    operator review; it is never silently accepted as ordinary prompt drift.
    """

    retired_paths = {str(item["path"]) for item in transaction["items"]}
    for relative in RELATIVE_CANDIDATES:
        if relative in retired_paths:
            continue
        state = source_state(workspace_fd, relative)
        if state is None:
            continue
        _, data = state
        advertises = (
            b"introspections/origin-mac" in data or b"origin-mac" in data.lower()
        )
        if not advertises:
            continue
        digest = hashlib.sha256(data).hexdigest()
        if digest in KNOWN_LEGACY:
            raise MigrationError(
                "exact-known origin-mac affordance appeared after the committed "
                f"retirement set: {relative} sha256={digest}"
            )
        raise MigrationError(
            f"unknown prompt/context still advertises origin-mac: {relative} sha256={digest}"
        )


def publish_operator_receipt(
    operator_fd: int,
    receipt: dict[str, Any],
    *,
    authority_uid: int,
    runtime_gid: int,
    fault: FaultHook,
) -> None:
    payload = canonical_json(receipt) + b"\n"
    pending_name = f".{OPERATOR_RECEIPT_NAME}.pending"
    pending = relative_lstat(operator_fd, pending_name)
    if pending is not None:
        if relative_lstat(operator_fd, OPERATOR_RECEIPT_NAME) is not None:
            raise MigrationError("operator receipt and its pending copy both exist")
        value, raw = read_json_file_at(
            operator_fd, pending_name, label="pending operator retirement receipt"
        )
        if value != receipt or raw != payload:
            raise MigrationError("pending operator retirement receipt is tampered")
        if (
            pending.st_uid != authority_uid
            or pending.st_gid != runtime_gid
            or stat.S_IMODE(pending.st_mode) != 0o640
            or pending.st_nlink != 1
        ):
            raise MigrationError("pending operator retirement receipt identity is invalid")
        os.rename(
            pending_name,
            OPERATOR_RECEIPT_NAME,
            src_dir_fd=operator_fd,
            dst_dir_fd=operator_fd,
        )
        fsync_directory(operator_fd)
    existing = relative_lstat(operator_fd, OPERATOR_RECEIPT_NAME)
    if existing is not None:
        value, raw = read_json_file_at(
            operator_fd, OPERATOR_RECEIPT_NAME, label="operator retirement receipt"
        )
        if value != receipt or raw != payload:
            raise MigrationError("prior operator retirement receipt is missing or tampered")
        metadata = os.stat(
            OPERATOR_RECEIPT_NAME, dir_fd=operator_fd, follow_symlinks=False
        )
        if (
            metadata.st_uid != authority_uid
            or metadata.st_gid != runtime_gid
            or stat.S_IMODE(metadata.st_mode) != 0o640
            or metadata.st_nlink != 1
        ):
            raise MigrationError("prior operator retirement receipt identity is invalid")
        return
    publish_once_at(
        operator_fd,
        pending_name=pending_name,
        final_name=OPERATOR_RECEIPT_NAME,
        payload=payload,
        uid=authority_uid,
        gid=runtime_gid,
        mode=0o640,
        fault=fault,
        fault_label="operator_receipt",
    )


def migrate(
    workspace_root: Path,
    operator_root: Path,
    retirement_root: Path,
    runtime_gid: int | None = None,
    *,
    fault: FaultHook | None = None,
) -> dict[str, Any]:
    """Apply or recover the exact durable correction.

    ``fault`` is an in-process test seam.  It is deliberately absent from the
    command-line interface and cannot weaken production policy.
    """

    authority_uid = os.geteuid()
    authority_gid = os.getegid()
    runtime_gid = authority_gid if runtime_gid is None else runtime_gid
    fault = (lambda _stage: None) if fault is None else fault
    if runtime_gid <= 0 and authority_uid != 0:
        raise MigrationError("runtime group must be an unprivileged appliance group")
    if authority_uid == 0 and runtime_gid <= 0:
        raise MigrationError("runtime group must be unprivileged")
    exact_absolute(workspace_root, "workspace root")
    exact_absolute(operator_root, "operator root")
    exact_absolute(retirement_root, "retirement root")
    if retirement_root == workspace_root or retirement_root.is_relative_to(workspace_root):
        raise MigrationError("retirement root must remain outside the accessible workspace")

    workspace_fd = open_absolute_directory(
        workspace_root,
        label="workspace root",
        authority_uid=authority_uid,
        require_controlled_ancestry=False,
    )
    retirement_fd = -1
    operator_fd = -1
    try:
        workspace_metadata = os.fstat(workspace_fd)
        retirement_fd = ensure_controlled_leaf_directory(
            retirement_root,
            label="retirement root",
            authority_uid=authority_uid,
            authority_gid=authority_gid,
            mode=0o700,
        )
        if os.fstat(retirement_fd).st_dev != workspace_metadata.st_dev:
            raise MigrationError("retirement root must be on the workspace filesystem")
        operator_fd = ensure_controlled_leaf_directory(
            operator_root,
            label="operator root",
            authority_uid=authority_uid,
            authority_gid=authority_gid,
            existing_group=runtime_gid,
            mode=0o2750,
        )

        def transaction_validator(value: dict[str, Any]) -> None:
            validate_transaction(value, workspace_root, retirement_root)

        promote_valid_pending(
            retirement_fd,
            pending_name=TRANSACTION_PENDING_NAME,
            final_name=TRANSACTION_NAME,
            validator=transaction_validator,
            label="origin-mac transaction",
            uid=authority_uid,
            gid=authority_gid,
            mode=0o600,
        )
        transaction_metadata = relative_lstat(retirement_fd, TRANSACTION_NAME)
        if transaction_metadata is None:
            allowed_empty = {RECEIPT_PENDING_NAME, RECEIPT_NAME}
            unexpected = set(os.listdir(retirement_fd)) - allowed_empty
            if unexpected:
                raise MigrationError(
                    "retirement root contains artifacts without a durable transaction"
                )
            if relative_lstat(retirement_fd, RECEIPT_NAME) is not None or relative_lstat(
                retirement_fd, RECEIPT_PENDING_NAME
            ) is not None:
                raise MigrationError("retirement receipt exists without its transaction")

            outcomes: list[dict[str, Any]] = []
            items: list[dict[str, Any]] = []
            seen_identities: dict[tuple[int, int], str] = {}
            for relative in RELATIVE_CANDIDATES:
                state = source_state(workspace_fd, relative)
                if state is None:
                    outcomes.append({"path": relative, "status": "absent"})
                    continue
                metadata, data = state
                identity = (metadata.st_dev, metadata.st_ino)
                if identity in seen_identities:
                    # Case-insensitive filesystems can resolve MEMORY.md and
                    # memory.md to the same singly-linked inode.  Record the
                    # alias explicitly; do not create a second retirement.
                    outcomes.append(
                        {
                            "path": relative,
                            "status": "same_file_alias_handled_by_primary_candidate",
                            "primary_path": seen_identities[identity],
                        }
                    )
                    continue
                seen_identities[identity] = relative
                digest = hashlib.sha256(data).hexdigest()
                advertises = (
                    b"introspections/origin-mac" in data
                    or b"origin-mac" in data.lower()
                )
                if digest in KNOWN_LEGACY:
                    destination = f"{relative.replace('/', '__')}.{digest}"
                    item = {
                        "path": relative,
                        "sha256": digest,
                        "size": len(data),
                        "source_dev": metadata.st_dev,
                        "source_ino": metadata.st_ino,
                        "destination": destination,
                        "copy_pending": f".copy.{destination}",
                        "source_disposal": f".dispose.{destination}",
                    }
                    items.append(item)
                    outcomes.append(
                        {
                            "path": relative,
                            "sha256": digest,
                            "status": "retired_exact_known_legacy_affordance",
                            "retired_name": destination,
                        }
                    )
                elif advertises:
                    raise MigrationError(
                        "unknown prompt/context still advertises origin-mac: "
                        f"{relative} sha256={digest}"
                    )
                else:
                    outcomes.append(
                        {
                            "path": relative,
                            "sha256": digest,
                            "status": "preserved_not_legacy",
                        }
                    )
            transaction = hash_bound(
                {
                    "schema": TRANSACTION_SCHEMA,
                    "workspace_root": str(workspace_root),
                    "retirement_root": str(retirement_root),
                    "scope": "exact_prompt_context_candidates_only",
                    "known_legacy_sha256": sorted(KNOWN_LEGACY),
                    "items": items,
                    "outcomes": outcomes,
                },
                "transaction_sha256",
            )
            validate_transaction(transaction, workspace_root, retirement_root)
            publish_once_at(
                retirement_fd,
                pending_name=TRANSACTION_PENDING_NAME,
                final_name=TRANSACTION_NAME,
                payload=canonical_json(transaction) + b"\n",
                uid=authority_uid,
                gid=authority_gid,
                mode=0o600,
                fault=fault,
                fault_label="transaction",
            )
        else:
            transaction, transaction_raw = read_json_file_at(
                retirement_fd, TRANSACTION_NAME, label="origin-mac transaction"
            )
            require_file_identity_at(
                retirement_fd,
                TRANSACTION_NAME,
                uid=authority_uid,
                gid=authority_gid,
                mode=0o600,
                label="origin-mac transaction",
            )
            validate_transaction(transaction, workspace_root, retirement_root)
            if transaction_raw != canonical_json(transaction) + b"\n":
                raise MigrationError("origin-mac transaction encoding is not exact")

        expected_entries = {TRANSACTION_NAME, RECEIPT_NAME, RECEIPT_PENDING_NAME}
        for item in transaction["items"]:
            expected_entries.add(str(item["destination"]))
            expected_entries.add(str(item["copy_pending"]))
            expected_entries.add(str(item["source_disposal"]))
        unexpected_entries = set(os.listdir(retirement_fd)) - expected_entries
        if unexpected_entries:
            raise MigrationError(
                "unexpected entry in dedicated retirement root: "
                + sorted(unexpected_entries)[0]
            )

        for item in transaction["items"]:
            copy_and_publish_artifact(
                workspace_fd,
                retirement_fd,
                item,
                authority_uid=authority_uid,
                authority_gid=authority_gid,
                fault=fault,
            )
            unlink_exact_source(
                workspace_fd, retirement_fd, item, fault=fault
            )

        reevaluate_unretired_candidates(workspace_fd, transaction)
        verify_transaction_completion(
            workspace_fd,
            retirement_fd,
            transaction,
            authority_uid=authority_uid,
            authority_gid=authority_gid,
        )
        expected_receipt = receipt_from_transaction(transaction)

        def receipt_validator(value: dict[str, Any]) -> None:
            validate_receipt(value, transaction)

        promote_valid_pending(
            retirement_fd,
            pending_name=RECEIPT_PENDING_NAME,
            final_name=RECEIPT_NAME,
            validator=receipt_validator,
            label="origin-mac receipt",
            uid=authority_uid,
            gid=authority_gid,
            mode=0o600,
        )
        if relative_lstat(retirement_fd, RECEIPT_NAME) is None:
            if relative_lstat(operator_fd, OPERATOR_RECEIPT_NAME) is not None:
                raise MigrationError(
                    "canonical retirement receipt is missing while its prior projection exists"
                )
            publish_once_at(
                retirement_fd,
                pending_name=RECEIPT_PENDING_NAME,
                final_name=RECEIPT_NAME,
                payload=canonical_json(expected_receipt) + b"\n",
                uid=authority_uid,
                gid=authority_gid,
                mode=0o600,
                fault=fault,
                fault_label="canonical_receipt",
            )
        canonical_receipt, canonical_raw = read_json_file_at(
            retirement_fd, RECEIPT_NAME, label="canonical origin-mac receipt"
        )
        require_file_identity_at(
            retirement_fd,
            RECEIPT_NAME,
            uid=authority_uid,
            gid=authority_gid,
            mode=0o600,
            label="canonical origin-mac receipt",
        )
        validate_receipt(canonical_receipt, transaction)
        if canonical_raw != canonical_json(canonical_receipt) + b"\n":
            raise MigrationError("canonical origin-mac receipt encoding is not exact")
        verify_transaction_completion(
            workspace_fd,
            retirement_fd,
            transaction,
            authority_uid=authority_uid,
            authority_gid=authority_gid,
        )
        reevaluate_unretired_candidates(workspace_fd, transaction)
        publish_operator_receipt(
            operator_fd,
            canonical_receipt,
            authority_uid=authority_uid,
            runtime_gid=runtime_gid,
            fault=fault,
        )
        return canonical_receipt
    finally:
        if operator_fd >= 0:
            os.close(operator_fd)
        if retirement_fd >= 0:
            os.close(retirement_fd)
        os.close(workspace_fd)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace-root", type=Path, required=True)
    parser.add_argument("--operator-root", type=Path, required=True)
    parser.add_argument("--retirement-root", type=Path, required=True)
    parser.add_argument("--runtime-gid", type=int, required=True)
    arguments = parser.parse_args()
    if os.geteuid() != 0:
        raise SystemExit("error: origin-mac affordance retirement requires root bootstrap")
    try:
        result = migrate(
            arguments.workspace_root,
            arguments.operator_root,
            arguments.retirement_root,
            arguments.runtime_gid,
        )
    except (MigrationError, OSError, ValueError) as error:
        raise SystemExit(f"error: {error}") from error
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
