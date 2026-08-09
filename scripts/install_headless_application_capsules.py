#!/usr/bin/env python3
"""Transactionally install externally built capsules on CPU-edge appliances.

This installer is intentionally separate from ``install_essential_capsules.sh``.
The latter owns the version-matched in-tree bootstrap set; this program owns
small, explicitly selected application capsules such as ReAct, Prompt Builder,
or an LLM provider rebuilt from the Astralis SDK.

Every archive is lifecycle-installed in a disposable ``ASTRID_HOME`` before a
live path is touched.  The live capsule directory, its environment file, all
referenced content-addressed objects, the service state, and the previous
generation pointer are then snapshotted under the shared CPU-edge transaction
lock.  A failed install or health check restores those exact paths and the
prior service active state.

The transaction is deliberately kept in one module: preflight, snapshot,
mutation, health verification, and rollback share one fail-closed state machine.
Splitting those phases across independently callable helpers would make partial
or out-of-order use easier during an appliance recovery.
"""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import re
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, NoReturn


CAPSULE_ID_RE = re.compile(r"^[a-z0-9][a-z0-9._-]{0,127}$")
HASH_RE = re.compile(r"^[0-9a-f]{64}$")
MAX_ARCHIVE_BYTES = 64 * 1024 * 1024
MAX_ENV_BYTES = 64 * 1024
TRANSACTION_PREFIX = "headless-application-capsules-"
PENDING_PREFIXES = (
    "headless-linux-",
    "edge-runtime-",
    "essential-capsules-",
    TRANSACTION_PREFIX,
)


class InstallError(RuntimeError):
    """An expected, operator-actionable installation failure."""


class InstallInterrupted(InstallError):
    """The process received a termination signal."""


@dataclass(frozen=True)
class ContentObject:
    kind: str
    digest: str
    preflight_sha256: str


@dataclass
class CapsuleArtifact:
    archive: Path
    verified_archive: Path
    capsule_id: str
    archive_sha256: str
    normalized_tree_sha256: str
    preflight_tree_sha256: str
    content_objects: tuple[ContentObject, ...]
    env_source: Path | None = None
    verified_env: Path | None = None
    prior_capsule_exists: bool = False
    prior_env_exists: bool = False
    prior_env_sha256: str | None = None
    prior_env_mode: int | None = None
    installed_tree_sha256: str | None = None
    installed_env_sha256: str | None = None
    installed_env_mode: int | None = None


@dataclass
class Transaction:
    astrid_home: Path
    root: Path
    generation_id: str
    service_was_active: bool = False
    service_restart_attempted: bool = False
    live_mutation_started: bool = False
    prior_service_show: str = ""
    prior_status: str = ""
    prior_current_files: dict[str, bool] = field(default_factory=dict)
    object_existed: dict[str, bool] = field(default_factory=dict)
    committed_files: list[Path] = field(default_factory=list)

    @property
    def prior(self) -> Path:
        return self.root / "prior"

    @property
    def verified(self) -> Path:
        return self.root / "verified"


def eprint(message: str) -> None:
    print(message, file=sys.stderr)


def fail(message: str) -> NoReturn:
    raise InstallError(message)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json_hash(value: Any) -> str:
    encoded = json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def normalized_meta(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        fail(f"invalid capsule metadata {path}: {error}")
    if not isinstance(value, dict):
        fail(f"capsule metadata must be a JSON object: {path}")
    for volatile in ("installed_at", "updated_at", "source"):
        value.pop(volatile, None)
    return value


def tree_hash(root: Path, *, normalize_metadata: bool = False) -> str:
    """Hash path, type, mode, and contents without following symlinks."""

    if root.is_symlink() or not root.is_dir():
        fail(f"capsule tree must be a real directory: {root}")
    records: list[dict[str, Any]] = [
        {
            "path": ".",
            "type": "directory",
            "mode": stat.S_IMODE(root.stat().st_mode),
        }
    ]
    for path in sorted(root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()):
        relative = path.relative_to(root).as_posix()
        metadata = path.lstat()
        mode = stat.S_IMODE(metadata.st_mode)
        if stat.S_ISLNK(metadata.st_mode):
            fail(f"capsule tree contains a forbidden symlink: {path}")
        if stat.S_ISDIR(metadata.st_mode):
            records.append({"path": relative, "type": "directory", "mode": mode})
            continue
        if not stat.S_ISREG(metadata.st_mode):
            fail(f"capsule tree contains a non-regular entry: {path}")
        if normalize_metadata and relative == "meta.json":
            content_hash = canonical_json_hash(normalized_meta(path))
            content_size = None
        else:
            content_hash = sha256_file(path)
            content_size = metadata.st_size
        records.append(
            {
                "path": relative,
                "type": "file",
                "mode": mode,
                "size": content_size,
                "sha256": content_hash,
            }
        )
    return canonical_json_hash(records)


def regular_file(path: Path, label: str, *, max_bytes: int | None = None) -> None:
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        fail(f"{label} does not exist: {path}")
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        fail(f"{label} must be a regular non-symlink file: {path}")
    if max_bytes is not None and metadata.st_size > max_bytes:
        fail(f"{label} exceeds {max_bytes} bytes: {path}")


def validate_env_file(path: Path) -> dict[str, Any]:
    regular_file(path, "capsule environment", max_bytes=MAX_ENV_BYTES)
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        fail(f"invalid capsule environment JSON {path}: {error}")
    if not isinstance(value, dict):
        fail(f"capsule environment must be a JSON object: {path}")
    if not all(isinstance(key, str) and key for key in value):
        fail(f"capsule environment keys must be non-empty strings: {path}")
    return value


def run_command(
    arguments: list[str],
    *,
    env: dict[str, str] | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    try:
        result = subprocess.run(
            arguments,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
    except OSError as error:
        fail(f"cannot execute {' '.join(arguments)}: {error}")
    if check and result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()
        if len(detail) > 2_000:
            detail = detail[-2_000:]
        fail(f"command failed ({result.returncode}): {' '.join(arguments)}: {detail}")
    return result


def locate_astrid(explicit: Path | None, project_root: Path, astrid_home: Path) -> Path:
    candidates: list[Path] = []
    if explicit is not None:
        candidates.append(explicit)
    else:
        candidates.extend(
            (
                project_root / "target/release/astrid",
                project_root / "astrid",
                astrid_home / "bin/astrid",
                Path.home() / ".astrid/bin/astrid",
            )
        )
        discovered = shutil.which("astrid")
        if discovered:
            candidates.append(Path(discovered))
    for candidate in candidates:
        try:
            resolved = candidate.expanduser().resolve(strict=True)
            metadata = resolved.stat()
        except (OSError, RuntimeError):
            continue
        if stat.S_ISREG(metadata.st_mode) and os.access(resolved, os.X_OK):
            return resolved
    fail("astrid binary not found; pass --astrid-bin with an executable path")


def parse_env_assignments(raw_assignments: list[str]) -> dict[str, Path]:
    parsed: dict[str, Path] = {}
    for assignment in raw_assignments:
        capsule_id, separator, raw_path = assignment.partition("=")
        if not separator or not CAPSULE_ID_RE.fullmatch(capsule_id) or not raw_path:
            fail("--env requires CAPSULE_ID=FILE with a safe capsule identifier")
        if capsule_id in parsed:
            fail(f"duplicate --env assignment for {capsule_id}")
        path = Path(raw_path).expanduser().absolute()
        validate_env_file(path)
        parsed[capsule_id] = path
    return parsed


def check_directory_chain(home: Path, destination: Path, allowed_symlink: Path | None) -> None:
    try:
        relative = destination.relative_to(home)
    except ValueError:
        fail(f"managed path escapes HOME: {destination}")
    current = home
    for component in relative.parts:
        if component in ("", ".", ".."):
            fail(f"unsafe managed path component in {destination}")
        current = current / component
        try:
            metadata = current.lstat()
        except FileNotFoundError:
            continue
        if stat.S_ISLNK(metadata.st_mode):
            if allowed_symlink is None or current != allowed_symlink:
                fail(f"managed directory component must not be a symlink: {current}")
            continue
        if not stat.S_ISDIR(metadata.st_mode):
            fail(f"managed directory component is not a directory: {current}")


def resolve_layout(home: Path, layout: str, dry_run: bool) -> Path:
    if not home.is_absolute():
        fail("HOME must be an absolute path")
    if layout == "standard":
        astrid_home = home / ".astrid"
        check_directory_chain(home, astrid_home, None)
        return astrid_home

    link = home / ".astrid-icp"
    astrid_home = link / "state"
    if dry_run and not link.exists() and not link.is_symlink():
        print(f"+ require {link} to resolve exactly to /media/data/astrid")
        return astrid_home
    try:
        metadata = link.lstat()
    except FileNotFoundError:
        fail(f"ICP layout requires an SSD symlink at {link}")
    if not stat.S_ISLNK(metadata.st_mode):
        fail(f"ICP layout requires {link} to be a symlink")
    try:
        resolved = link.resolve(strict=True)
    except OSError as error:
        fail(f"cannot resolve ICP SSD symlink {link}: {error}")
    if resolved != Path("/media/data/astrid"):
        fail(f"{link} must resolve exactly to /media/data/astrid (found {resolved})")
    if not dry_run:
        result = run_command(["mountpoint", "-q", "/media/data"], check=False)
        if result.returncode != 0:
            fail("ICP layout requires the SSD mounted at /media/data")
    check_directory_chain(home, astrid_home, link)
    return astrid_home


def referenced_objects(preflight_home: Path, meta: dict[str, Any]) -> tuple[ContentObject, ...]:
    objects: list[ContentObject] = []
    wasm_hash = meta.get("wasm_hash")
    if wasm_hash is not None:
        if not isinstance(wasm_hash, str) or not HASH_RE.fullmatch(wasm_hash):
            fail("capsule meta.json contains an invalid wasm_hash")
        wasm_path = preflight_home / "bin" / f"{wasm_hash}.wasm"
        regular_file(wasm_path, "preflight content-addressed WASM")
        objects.append(ContentObject("wasm", wasm_hash, sha256_file(wasm_path)))
    wit_files = meta.get("wit_files", {})
    if not isinstance(wit_files, dict):
        fail("capsule meta.json wit_files must be an object")
    wit_hashes: set[str] = set()
    for digest in wit_files.values():
        if not isinstance(digest, str) or not HASH_RE.fullmatch(digest):
            fail("capsule meta.json contains an invalid WIT hash")
        wit_hashes.add(digest)
    for digest in sorted(wit_hashes):
        wit_path = preflight_home / "wit" / f"{digest}.wit"
        regular_file(wit_path, "preflight content-addressed WIT")
        objects.append(ContentObject("wit", digest, sha256_file(wit_path)))
    return tuple(objects)


def preflight_archive(
    astrid_bin: Path,
    archive: Path,
    preflight_parent: Path,
    verified_parent: Path,
    index: int,
    env_assignments: dict[str, Path] | None = None,
) -> CapsuleArtifact:
    regular_file(archive, "capsule archive", max_bytes=MAX_ARCHIVE_BYTES)
    preflight_home = preflight_parent / str(index)
    preflight_home.mkdir(parents=True, mode=0o700)
    if env_assignments:
        preflight_env_root = preflight_home / "home/default/.config/env"
        preflight_env_root.mkdir(parents=True, mode=0o700)
        for capsule_id, env_source in env_assignments.items():
            env_target = preflight_env_root / f"{capsule_id}.env.json"
            shutil.copyfile(env_source, env_target)
            os.chmod(env_target, 0o600)
    verified_archive = verified_parent / f"archive-{index}.capsule"
    shutil.copyfile(archive, verified_archive)
    os.chmod(verified_archive, 0o600)
    if sha256_file(archive) != sha256_file(verified_archive):
        fail(f"verified archive copy differs from source: {archive}")

    command_env = os.environ.copy()
    command_env["ASTRID_HOME"] = str(preflight_home)
    run_command(
        [str(astrid_bin), "capsule", "install", str(verified_archive)],
        env=command_env,
    )
    capsule_root = preflight_home / "home/default/.local/capsules"
    if not capsule_root.is_dir() or capsule_root.is_symlink():
        fail(f"isolated preflight did not create a capsule root for {archive}")
    children = list(capsule_root.iterdir())
    if len(children) != 1 or not children[0].is_dir() or children[0].is_symlink():
        fail(f"isolated preflight must install exactly one real capsule from {archive}")
    capsule_dir = children[0]
    capsule_id = capsule_dir.name
    if not CAPSULE_ID_RE.fullmatch(capsule_id):
        fail(f"archive installed an unsafe capsule identifier: {capsule_id!r}")
    regular_file(capsule_dir / "Capsule.toml", "installed Capsule.toml")
    regular_file(capsule_dir / "meta.json", "installed meta.json")
    meta = normalized_meta(capsule_dir / "meta.json")
    content_objects = referenced_objects(preflight_home, meta)
    return CapsuleArtifact(
        archive=archive,
        verified_archive=verified_archive,
        capsule_id=capsule_id,
        archive_sha256=sha256_file(verified_archive),
        normalized_tree_sha256=tree_hash(capsule_dir, normalize_metadata=True),
        preflight_tree_sha256=tree_hash(capsule_dir),
        content_objects=content_objects,
    )


def service_show() -> str:
    properties = (
        "LoadState",
        "ActiveState",
        "SubState",
        "UnitFileState",
        "MainPID",
        "NRestarts",
        "ExecMainStartTimestampMonotonic",
        "FragmentPath",
        "DropInPaths",
    )
    arguments = ["systemctl", "--user", "show", "astrid.service"]
    for property_name in properties:
        arguments.extend(("--property", property_name))
    result = run_command(arguments, check=False)
    if result.returncode != 0:
        return f"unavailable_returncode={result.returncode}\n"
    return result.stdout


def parse_properties(raw: str) -> dict[str, str]:
    result: dict[str, str] = {}
    for line in raw.splitlines():
        key, separator, value = line.partition("=")
        if separator:
            result[key] = value
    return result


def service_is_active() -> bool:
    return (
        run_command(
            ["systemctl", "--user", "is-active", "--quiet", "astrid.service"],
            check=False,
        ).returncode
        == 0
    )


def status_output(astrid_bin: Path, astrid_home: Path) -> subprocess.CompletedProcess[str]:
    command_env = os.environ.copy()
    command_env["ASTRID_HOME"] = str(astrid_home)
    return run_command(
        [str(astrid_bin), "--format", "json", "status"],
        env=command_env,
        check=False,
    )


def loaded_capsules(raw: str) -> list[str]:
    try:
        value = json.loads(raw)
        loaded = value["status"]["loaded_capsules"]
    except (json.JSONDecodeError, KeyError, TypeError) as error:
        fail(f"invalid Astrid JSON status: {error}")
    if not isinstance(loaded, list) or not all(isinstance(item, str) for item in loaded):
        fail("status.loaded_capsules must be a string array")
    if len(set(loaded)) != len(loaded):
        fail("status.loaded_capsules contains duplicates")
    return loaded


def installed_capsule_count(capsule_root: Path) -> int:
    if not capsule_root.is_dir() or capsule_root.is_symlink():
        fail(f"installed capsule root must be a real directory: {capsule_root}")
    count = 0
    for entry in capsule_root.iterdir():
        if entry.is_symlink() or not entry.is_dir():
            fail(f"installed capsule root contains an unsafe entry: {entry}")
        count += 1
    if count < 1:
        fail("installed capsule count must be positive")
    return count


def verify_service_health(
    astrid_bin: Path,
    astrid_home: Path,
    required_capsules: set[str],
    expected_total: int,
    attempts: int,
    stability_seconds: float,
    prior_service_show: str,
) -> tuple[str, str]:
    last_error = "service did not become ready"
    accepted_status = ""
    for _ in range(attempts):
        if service_is_active():
            status = status_output(astrid_bin, astrid_home)
            if status.returncode == 0:
                try:
                    loaded = loaded_capsules(status.stdout)
                except InstallError as error:
                    last_error = str(error)
                else:
                    missing = sorted(required_capsules.difference(loaded))
                    if missing:
                        last_error = "required capsules absent: " + ",".join(missing)
                    elif len(loaded) != expected_total:
                        last_error = (
                            f"expected {expected_total} loaded capsules, found {len(loaded)}"
                        )
                    else:
                        accepted_status = status.stdout
                        break
            else:
                last_error = (status.stderr or status.stdout).strip()
        time.sleep(1)
    else:
        fail(f"astrid.service failed capsule health verification: {last_error}")

    first_show = service_show()
    first = parse_properties(first_show)
    if stability_seconds > 0:
        time.sleep(stability_seconds)
    if not service_is_active():
        fail("astrid.service became inactive during the stability check")
    second_show = service_show()
    second = parse_properties(second_show)
    prior = parse_properties(prior_service_show)
    first_pid = first.get("MainPID", "")
    second_pid = second.get("MainPID", "")
    if not first_pid.isdigit() or first_pid == "0" or first_pid != second_pid:
        fail("astrid.service MainPID changed during the stability check")
    first_restarts = first.get("NRestarts", "")
    second_restarts = second.get("NRestarts", "")
    if not first_restarts.isdigit() or not second_restarts.isdigit():
        fail("astrid.service did not expose a numeric NRestarts health signal")
    if first_restarts != second_restarts:
        fail("astrid.service NRestarts changed during the stability check")
    prior_restarts = prior.get("NRestarts", "")
    installed_restarts = second_restarts
    if prior_restarts.isdigit() and installed_restarts.isdigit():
        if int(installed_restarts) > int(prior_restarts):
            fail("astrid.service recorded an unexpected restart during installation")
    prior_pid = prior.get("MainPID", "")
    if prior_pid.isdigit() and prior_pid != "0" and second_pid == prior_pid:
        fail("astrid.service MainPID did not change across the requested restart")
    return second_show, accepted_status


def safe_remove(path: Path, expected_parent: Path) -> None:
    if path.parent != expected_parent:
        fail(f"refusing rollback outside managed directory: {path}")
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        return
    if stat.S_ISLNK(metadata.st_mode) or stat.S_ISREG(metadata.st_mode):
        path.unlink()
    elif stat.S_ISDIR(metadata.st_mode):
        shutil.rmtree(path)
    else:
        fail(f"refusing rollback of non-regular managed path: {path}")


def atomic_copy(source: Path, destination: Path, mode: int) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    staged = destination.with_name(destination.name + f".new-{os.getpid()}")
    if staged.exists() or staged.is_symlink():
        safe_remove(staged, destination.parent)
    try:
        shutil.copyfile(source, staged)
        os.chmod(staged, mode)
        if sha256_file(source) != sha256_file(staged):
            fail(f"atomic copy verification failed for {destination}")
        os.replace(staged, destination)
    except BaseException:
        safe_remove(staged, destination.parent)
        raise


def write_atomic(payload: bytes, destination: Path, mode: int = 0o600) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    staged = destination.with_name(destination.name + f".new-{os.getpid()}")
    if staged.exists() or staged.is_symlink():
        safe_remove(staged, destination.parent)
    try:
        with staged.open("wb") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(staged, mode)
        os.replace(staged, destination)
    except BaseException:
        safe_remove(staged, destination.parent)
        raise


def object_path(astrid_home: Path, content_object: ContentObject) -> Path:
    if content_object.kind == "wasm":
        return astrid_home / "bin" / f"{content_object.digest}.wasm"
    if content_object.kind == "wit":
        return astrid_home / "wit" / f"{content_object.digest}.wit"
    fail(f"unsupported content object kind: {content_object.kind}")


def snapshot_service(transaction: Transaction, astrid_bin: Path) -> None:
    transaction.service_was_active = service_is_active()
    transaction.prior_service_show = service_show()
    (transaction.prior / "service.show").write_text(
        transaction.prior_service_show, encoding="utf-8"
    )
    os.chmod(transaction.prior / "service.show", 0o600)
    if transaction.service_was_active:
        status = status_output(astrid_bin, transaction.astrid_home)
        if status.returncode == 0:
            transaction.prior_status = status.stdout
    (transaction.prior / "status.json").write_text(
        transaction.prior_status, encoding="utf-8"
    )
    os.chmod(transaction.prior / "status.json", 0o600)


def snapshot_live_paths(transaction: Transaction, artifacts: list[CapsuleArtifact]) -> None:
    capsule_root = transaction.astrid_home / "home/default/.local/capsules"
    env_root = transaction.astrid_home / "home/default/.config/env"
    (transaction.prior / "capsules").mkdir(parents=True, mode=0o700)
    (transaction.prior / "env").mkdir(parents=True, mode=0o700)
    (transaction.prior / "objects").mkdir(parents=True, mode=0o700)
    for artifact in artifacts:
        target = capsule_root / artifact.capsule_id
        backup_target = target.with_suffix(".bak")
        if backup_target.exists() or backup_target.is_symlink():
            fail(f"prior capsule recovery directory requires operator review: {backup_target}")
        if target.exists() or target.is_symlink():
            if target.is_symlink() or not target.is_dir():
                fail(f"live capsule target must be a real directory: {target}")
            tree_hash(target)
            shutil.copytree(
                target,
                transaction.prior / "capsules" / artifact.capsule_id,
                symlinks=True,
                copy_function=shutil.copy2,
            )
            artifact.prior_capsule_exists = True

        env_target = env_root / f"{artifact.capsule_id}.env.json"
        if env_target.exists() or env_target.is_symlink():
            regular_file(env_target, "live capsule environment", max_bytes=MAX_ENV_BYTES)
            validate_env_file(env_target)
            shutil.copy2(env_target, transaction.prior / "env" / env_target.name)
            artifact.prior_env_exists = True
            artifact.prior_env_sha256 = sha256_file(env_target)
            artifact.prior_env_mode = stat.S_IMODE(env_target.stat().st_mode)

        for content_object in artifact.content_objects:
            key = f"{content_object.kind}:{content_object.digest}"
            if key in transaction.object_existed:
                continue
            live_object = object_path(transaction.astrid_home, content_object)
            if live_object.exists() or live_object.is_symlink():
                regular_file(live_object, "live content-addressed object")
                snapshot = transaction.prior / "objects" / (
                    f"{content_object.kind}-{content_object.digest}"
                )
                shutil.copy2(live_object, snapshot)
                transaction.object_existed[key] = True
            else:
                transaction.object_existed[key] = False


def snapshot_current_manifests(transaction: Transaction) -> None:
    manifest_root = transaction.astrid_home / "etc/install-manifests"
    for name in (
        "headless-application-capsules.current.json",
        "headless-application-capsules.current.sha256",
    ):
        source = manifest_root / name
        exists = source.exists() or source.is_symlink()
        transaction.prior_current_files[name] = exists
        if not exists:
            continue
        regular_file(source, "current application-capsule manifest")
        shutil.copy2(source, transaction.prior / name)


def verify_live_install(artifact: CapsuleArtifact, astrid_home: Path) -> None:
    capsule_dir = (
        astrid_home / "home/default/.local/capsules" / artifact.capsule_id
    )
    regular_file(capsule_dir / "Capsule.toml", "live Capsule.toml")
    regular_file(capsule_dir / "meta.json", "live meta.json")
    normalized = tree_hash(capsule_dir, normalize_metadata=True)
    if normalized != artifact.normalized_tree_sha256:
        fail(f"live capsule payload differs from isolated preflight: {artifact.capsule_id}")
    artifact.installed_tree_sha256 = tree_hash(capsule_dir)
    for content_object in artifact.content_objects:
        live_object = object_path(astrid_home, content_object)
        regular_file(live_object, "live content-addressed object")
        if sha256_file(live_object) != content_object.preflight_sha256:
            fail(
                "live content-addressed object differs from preflight: "
                f"{content_object.kind}:{content_object.digest}"
            )


def install_live(
    transaction: Transaction,
    astrid_bin: Path,
    artifacts: list[CapsuleArtifact],
) -> None:
    command_env = os.environ.copy()
    command_env["ASTRID_HOME"] = str(transaction.astrid_home)
    env_root = transaction.astrid_home / "home/default/.config/env"
    env_root.mkdir(parents=True, exist_ok=True, mode=0o700)
    transaction.live_mutation_started = True
    for artifact in artifacts:
        env_target = env_root / f"{artifact.capsule_id}.env.json"
        if artifact.verified_env is not None:
            # Lifecycle hooks and first-install prompting must see the exact
            # operator-selected environment, not a temporary empty/default
            # profile that is replaced only after installation.
            atomic_copy(artifact.verified_env, env_target, 0o600)
        run_command(
            [str(astrid_bin), "capsule", "install", str(artifact.verified_archive)],
            env=command_env,
        )
        verify_live_install(artifact, transaction.astrid_home)
        if env_target.exists() or env_target.is_symlink():
            validate_env_file(env_target)
            os.chmod(env_target, 0o600)
            artifact.installed_env_sha256 = sha256_file(env_target)
            artifact.installed_env_mode = stat.S_IMODE(env_target.stat().st_mode)
        if artifact.verified_env is not None:
            if artifact.installed_env_sha256 != sha256_file(artifact.verified_env):
                fail(
                    "capsule lifecycle changed the operator-selected environment: "
                    f"{artifact.capsule_id}"
                )
        if artifact.env_source is None and artifact.prior_env_exists:
            if artifact.installed_env_sha256 != artifact.prior_env_sha256:
                fail(
                    "capsule install changed an environment file without --env: "
                    f"{artifact.capsule_id}"
                )


def restore_service(transaction: Transaction, astrid_bin: Path) -> None:
    if not transaction.service_restart_attempted:
        return
    run_command(["systemctl", "--user", "stop", "astrid.service"], check=False)
    if transaction.service_was_active:
        result = run_command(
            ["systemctl", "--user", "start", "astrid.service"], check=False
        )
        if result.returncode != 0:
            fail("rollback could not restore the prior active astrid.service state")
        expected_loaded = (
            set(loaded_capsules(transaction.prior_status))
            if transaction.prior_status
            else None
        )
        for _ in range(10):
            if service_is_active():
                if expected_loaded is None:
                    return
                status = status_output(astrid_bin, transaction.astrid_home)
                if status.returncode == 0:
                    try:
                        restored_loaded = set(loaded_capsules(status.stdout))
                    except InstallError:
                        pass
                    else:
                        if restored_loaded == expected_loaded:
                            return
            time.sleep(1)
        fail("rollback could not verify the prior loaded-capsule service generation")
    if service_is_active():
        fail("rollback could not restore the prior inactive astrid.service state")


def rollback(
    transaction: Transaction,
    artifacts: list[CapsuleArtifact],
    astrid_bin: Path,
) -> None:
    if transaction.service_restart_attempted:
        run_command(["systemctl", "--user", "stop", "astrid.service"], check=False)
    if transaction.live_mutation_started:
        capsule_root = transaction.astrid_home / "home/default/.local/capsules"
        env_root = transaction.astrid_home / "home/default/.config/env"
        for artifact in reversed(artifacts):
            target = capsule_root / artifact.capsule_id
            safe_remove(target, capsule_root)
            safe_remove(target.with_suffix(".bak"), capsule_root)
            snapshot = transaction.prior / "capsules" / artifact.capsule_id
            if artifact.prior_capsule_exists:
                os.replace(snapshot, target)

            env_target = env_root / f"{artifact.capsule_id}.env.json"
            safe_remove(env_target, env_root)
            env_snapshot = transaction.prior / "env" / env_target.name
            if artifact.prior_env_exists:
                os.replace(env_snapshot, env_target)

        restored_objects: set[str] = set()
        for artifact in reversed(artifacts):
            for content_object in artifact.content_objects:
                key = f"{content_object.kind}:{content_object.digest}"
                if key in restored_objects:
                    continue
                restored_objects.add(key)
                live_object = object_path(transaction.astrid_home, content_object)
                live_object.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
                safe_remove(live_object, live_object.parent)
                if transaction.object_existed.get(key, False):
                    snapshot = transaction.prior / "objects" / (
                        f"{content_object.kind}-{content_object.digest}"
                    )
                    os.replace(snapshot, live_object)

    manifest_root = transaction.astrid_home / "etc/install-manifests"
    for path in reversed(transaction.committed_files):
        safe_remove(path, path.parent)
    for name, existed in transaction.prior_current_files.items():
        destination = manifest_root / name
        safe_remove(destination, manifest_root)
        if existed:
            os.replace(transaction.prior / name, destination)
    restore_service(transaction, astrid_bin)


def begin_transaction(astrid_home: Path) -> tuple[Transaction, Any]:
    transaction_parent = astrid_home / ".install-transactions"
    transaction_parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(transaction_parent, 0o700)
    lock_path = transaction_parent / "install.lock"
    lock_handle = lock_path.open("a+b")
    os.chmod(lock_path, 0o600)
    try:
        fcntl.flock(lock_handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        lock_handle.close()
        fail("another CPU-edge installer is already active")
    for entry in transaction_parent.iterdir():
        if entry.is_dir() and any(entry.name.startswith(prefix) for prefix in PENDING_PREFIXES):
            lock_handle.close()
            fail(f"pending CPU-edge transaction requires operator recovery: {entry}")

    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    generation_id = f"{TRANSACTION_PREFIX}{stamp}-{os.getpid()}"
    root = Path(tempfile.mkdtemp(prefix=generation_id + ".", dir=transaction_parent))
    os.chmod(root, 0o700)
    for directory in (root / "prior", root / "verified", root / "preflight"):
        directory.mkdir(mode=0o700)
    return Transaction(astrid_home, root, generation_id), lock_handle


def manifest_payload(
    transaction: Transaction,
    artifacts: list[CapsuleArtifact],
    astrid_bin: Path,
    layout: str,
    restart_requested: bool,
    expected_total: int | None,
    after_service_show: str,
    after_status: str,
) -> bytes:
    capsules: list[dict[str, Any]] = []
    for artifact in artifacts:
        capsules.append(
            {
                "capsule_id": artifact.capsule_id,
                "archive_sha256": artifact.archive_sha256,
                "preflight_tree_sha256": artifact.preflight_tree_sha256,
                "normalized_payload_sha256": artifact.normalized_tree_sha256,
                "installed_tree_sha256": artifact.installed_tree_sha256,
                "environment": {
                    "source": (
                        "operator_replacement"
                        if artifact.env_source is not None
                        else "preserved_or_capsule_default"
                    ),
                    "prior_present": artifact.prior_env_exists,
                    "prior_sha256": artifact.prior_env_sha256,
                    "prior_mode": artifact.prior_env_mode,
                    "installed_sha256": artifact.installed_env_sha256,
                    "installed_mode": artifact.installed_env_mode,
                },
                "content_objects": [
                    {
                        "kind": item.kind,
                        "digest": item.digest,
                        "sha256": item.preflight_sha256,
                    }
                    for item in artifact.content_objects
                ],
            }
        )
    payload = {
        "schema": "astrid_headless_application_capsule_generation_v1",
        "generation_id": transaction.generation_id,
        "created_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "authority": "operator_install_evidence_not_astrid_memory_or_action_authority",
        "layout": layout,
        "astrid_home": str(transaction.astrid_home),
        "astrid_binary": {
            "path": str(astrid_bin),
            "sha256": sha256_file(astrid_bin),
        },
        "capsules": capsules,
        "service": {
            "unit": "astrid.service",
            "restart_requested": restart_requested,
            "was_active": transaction.service_was_active,
            "expected_loaded_capsules": expected_total,
            "prior_snapshot_sha256": hashlib.sha256(
                transaction.prior_service_show.encode("utf-8")
            ).hexdigest(),
            "prior_properties": parse_properties(transaction.prior_service_show),
            "prior_status_sha256": (
                hashlib.sha256(transaction.prior_status.encode("utf-8")).hexdigest()
                if transaction.prior_status
                else None
            ),
            "prior_loaded_capsules": (
                loaded_capsules(transaction.prior_status)
                if transaction.prior_status
                else None
            ),
            "installed_snapshot_sha256": (
                hashlib.sha256(after_service_show.encode("utf-8")).hexdigest()
                if after_service_show
                else None
            ),
            "installed_properties": parse_properties(after_service_show),
            "installed_status_sha256": (
                hashlib.sha256(after_status.encode("utf-8")).hexdigest()
                if after_status
                else None
            ),
            "installed_loaded_capsules": (
                loaded_capsules(after_status) if after_status else None
            ),
        },
    }
    return (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode("utf-8")


def commit_manifest(transaction: Transaction, payload: bytes) -> tuple[Path, str]:
    manifest_root = transaction.astrid_home / "etc/install-manifests"
    history_root = manifest_root / "headless-application-capsules"
    history_root.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(history_root, 0o700)
    manifest_path = history_root / f"{transaction.generation_id}.json"
    sidecar_path = history_root / f"{transaction.generation_id}.sha256"
    digest = hashlib.sha256(payload).hexdigest()
    sidecar = f"{digest}  {manifest_path.name}\n".encode("utf-8")
    current_path = manifest_root / "headless-application-capsules.current.json"
    current_sidecar = manifest_root / "headless-application-capsules.current.sha256"

    write_atomic(payload, manifest_path)
    transaction.committed_files.append(manifest_path)
    write_atomic(sidecar, sidecar_path)
    transaction.committed_files.append(sidecar_path)
    write_atomic(payload, current_path)
    write_atomic(
        f"{digest}  {current_path.name}\n".encode("utf-8"), current_sidecar
    )
    return manifest_path, digest


def dry_run_preflight(
    astrid_bin: Path,
    archives: list[Path],
    env_assignments: dict[str, Path],
    astrid_home: Path,
    layout: str,
    restart: bool,
    expected_total: int | None,
) -> None:
    with tempfile.TemporaryDirectory(prefix="astrid-application-capsule-preflight-") as raw:
        root = Path(raw)
        verified = root / "verified"
        preflight = root / "preflight"
        verified.mkdir(mode=0o700)
        preflight.mkdir(mode=0o700)
        artifacts = [
            preflight_archive(
                astrid_bin,
                archive,
                preflight,
                verified,
                index,
                env_assignments,
            )
            for index, archive in enumerate(archives)
        ]
    ids = [artifact.capsule_id for artifact in artifacts]
    if len(set(ids)) != len(ids):
        fail("multiple archives preflighted to the same capsule identifier")
    unknown_env = sorted(set(env_assignments).difference(ids))
    if unknown_env:
        fail("--env names a capsule absent from this transaction: " + ",".join(unknown_env))
    print(f"+ acquire shared CPU-edge install lock under {astrid_home / '.install-transactions'}")
    print("+ snapshot exact capsule, environment, content-object, manifest, and service state")
    for artifact in artifacts:
        env_note = env_assignments.get(artifact.capsule_id)
        print(
            f"+ install verified {artifact.capsule_id} "
            f"archive_sha256={artifact.archive_sha256}"
        )
        if env_note is not None:
            print(f"+ install owner-only environment {artifact.capsule_id}={env_note}")
    if restart:
        print("+ restart astrid.service and verify stable PID, NRestarts, and loaded capsules")
        if expected_total is not None:
            print(f"+ require exactly {expected_total} loaded capsules")
    print(
        "+ write owner-only hashed application-capsule generation manifest under "
        f"{astrid_home / 'etc/install-manifests'}"
    )
    print(f"Dry-run preflight passed for layout={layout} capsules={','.join(ids)}")


def install(args: argparse.Namespace) -> int:
    raw_home = os.environ.get("HOME")
    if not raw_home or not Path(raw_home).is_absolute():
        fail("HOME must be set to an absolute path")
    home = Path(raw_home)
    astrid_home = resolve_layout(home, args.layout, args.dry_run)
    script_dir = Path(__file__).resolve().parent
    project_root = script_dir.parent
    astrid_bin = locate_astrid(args.astrid_bin, project_root, astrid_home)
    archives = [Path(value).expanduser().absolute() for value in args.capsule]
    for archive in archives:
        regular_file(archive, "capsule archive", max_bytes=MAX_ARCHIVE_BYTES)
    env_assignments = parse_env_assignments(args.env)

    if args.dry_run:
        dry_run_preflight(
            astrid_bin,
            archives,
            env_assignments,
            astrid_home,
            args.layout,
            args.restart,
            args.expected_total,
        )
        return 0

    allowed_symlink = home / ".astrid-icp" if args.layout == "icp-ssd" else None
    for managed in (
        astrid_home,
        astrid_home / ".install-transactions",
        astrid_home / "home/default/.local/capsules",
        astrid_home / "home/default/.config/env",
        astrid_home / "etc/install-manifests",
        astrid_home / "bin",
        astrid_home / "wit",
    ):
        check_directory_chain(home, managed, allowed_symlink)

    transaction, lock_handle = begin_transaction(astrid_home)
    artifacts: list[CapsuleArtifact] = []
    rolled_back = False
    committed = False

    def interrupted(signum: int, _frame: Any) -> NoReturn:
        raise InstallInterrupted(f"received signal {signum}")

    previous_handlers = {
        signum: signal.signal(signum, interrupted)
        for signum in (signal.SIGHUP, signal.SIGINT, signal.SIGTERM)
    }
    try:
        artifacts = [
            preflight_archive(
                astrid_bin,
                archive,
                transaction.root / "preflight",
                transaction.verified,
                index,
                env_assignments,
            )
            for index, archive in enumerate(archives)
        ]
        ids = [artifact.capsule_id for artifact in artifacts]
        if len(set(ids)) != len(ids):
            fail("multiple archives preflighted to the same capsule identifier")
        unknown_env = sorted(set(env_assignments).difference(ids))
        if unknown_env:
            fail(
                "--env names a capsule absent from this transaction: "
                + ",".join(unknown_env)
            )
        for artifact in artifacts:
            source = env_assignments.get(artifact.capsule_id)
            if source is not None:
                verified_env = transaction.verified / f"{artifact.capsule_id}.env.json"
                shutil.copyfile(source, verified_env)
                os.chmod(verified_env, 0o600)
                validate_env_file(verified_env)
                if sha256_file(source) != sha256_file(verified_env):
                    fail(f"verified environment copy differs for {artifact.capsule_id}")
                artifact.env_source = source
                artifact.verified_env = verified_env

        snapshot_service(transaction, astrid_bin)
        if args.restart:
            prior_properties = parse_properties(transaction.prior_service_show)
            if not prior_properties.get("NRestarts", "").isdigit():
                fail("cannot snapshot numeric astrid.service NRestarts before restart")
            if transaction.service_was_active:
                prior_pid = prior_properties.get("MainPID", "")
                if not prior_pid.isdigit() or prior_pid == "0":
                    fail("cannot snapshot active astrid.service MainPID before restart")
                if not transaction.prior_status:
                    fail("cannot snapshot prior loaded-capsule status before restart")
                loaded_capsules(transaction.prior_status)
        snapshot_live_paths(transaction, artifacts)
        snapshot_current_manifests(transaction)
        install_live(transaction, astrid_bin, artifacts)

        after_service_show = ""
        after_status = ""
        expected_total = args.expected_total
        if args.restart:
            if shutil.which("systemctl") is None:
                fail("--restart requires systemctl")
            transaction.service_restart_attempted = True
            run_command(["systemctl", "--user", "restart", "astrid.service"])
            if expected_total is None:
                expected_total = installed_capsule_count(
                    astrid_home / "home/default/.local/capsules"
                )
            after_service_show, after_status = verify_service_health(
                astrid_bin,
                astrid_home,
                set(ids),
                expected_total,
                args.health_attempts,
                args.health_stability_seconds,
                transaction.prior_service_show,
            )

        payload = manifest_payload(
            transaction,
            artifacts,
            astrid_bin,
            args.layout,
            args.restart,
            expected_total,
            after_service_show,
            after_status,
        )
        manifest_path, manifest_hash = commit_manifest(transaction, payload)
        committed = True
        generation_id = transaction.generation_id
        shutil.rmtree(transaction.root)
        print(
            "Committed verified headless application-capsule generation "
            f"{generation_id}"
        )
        print(f"generation_manifest={manifest_path}")
        print(f"generation_manifest_sha256={manifest_hash}")
        print("capsules=" + ",".join(ids))
        return 0
    except BaseException:
        if committed:
            eprint(
                "error: generation commit completed but transaction cleanup was "
                f"interrupted; review {transaction.root} before another install"
            )
            raise
        try:
            rollback(transaction, artifacts, astrid_bin)
            rolled_back = True
        except BaseException as rollback_error:
            eprint(
                "error: rollback was incomplete; recovery material remains at "
                f"{transaction.root}: {rollback_error}"
            )
        if rolled_back:
            shutil.rmtree(transaction.root, ignore_errors=True)
        raise
    finally:
        for signum, handler in previous_handlers.items():
            signal.signal(signum, handler)
        lock_handle.close()


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(
        description="Transactionally install selected external headless capsules."
    )
    result.add_argument(
        "--capsule",
        action="append",
        required=True,
        metavar="ARCHIVE",
        help=".capsule archive to install; repeat for an atomic set",
    )
    result.add_argument(
        "--env",
        action="append",
        default=[],
        metavar="CAPSULE_ID=FILE",
        help="owner-only environment JSON replacement; repeat as needed",
    )
    result.add_argument(
        "--astrid-bin",
        type=Path,
        help="exact astrid CLI used for isolated and live lifecycle installation",
    )
    result.add_argument(
        "--layout",
        choices=("standard", "icp-ssd"),
        default="standard",
    )
    result.add_argument(
        "--restart",
        action="store_true",
        help="restart astrid.service and require stable loaded-capsule health",
    )
    result.add_argument(
        "--expected-total",
        type=int,
        help="exact loaded capsule count; defaults to installed directory count",
    )
    result.add_argument("--health-attempts", type=int, default=20)
    result.add_argument("--health-stability-seconds", type=float, default=1.0)
    result.add_argument(
        "--dry-run",
        action="store_true",
        help="run isolated preflight and print live operations without mutation",
    )
    return result


def main() -> int:
    try:
        args = parser().parse_args()
        if args.expected_total is not None and args.expected_total < 1:
            fail("--expected-total must be positive")
        if args.expected_total is not None and not args.restart:
            fail("--expected-total requires --restart so the count is verified")
        if args.health_attempts < 1:
            fail("--health-attempts must be positive")
        if args.health_stability_seconds < 0 or args.health_stability_seconds > 60:
            fail("--health-stability-seconds must be between 0 and 60")
        return install(args)
    except (InstallError, OSError) as error:
        eprint(f"error: {error}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
