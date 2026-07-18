"""Canonical records and filesystem helpers for steward control."""

from __future__ import annotations

from contextlib import contextmanager
from datetime import UTC, datetime
import hashlib
import json
import os
from pathlib import Path
import secrets
import socket
import tempfile
import time
from typing import Any, Iterator

try:
    import fcntl
except ImportError:  # pragma: no cover - Windows compatibility
    fcntl = None  # type: ignore[assignment]

try:
    import msvcrt
except ImportError:  # pragma: no cover - POSIX compatibility
    msvcrt = None  # type: ignore[assignment]


def canonical_json(value: Any) -> str:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    )


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_canonical(value: Any) -> str:
    return sha256_bytes(canonical_json(value).encode("utf-8"))


def utc_now() -> str:
    return datetime.now(tz=UTC).isoformat()


def unix_time() -> float:
    return time.time()


def host_identity() -> str:
    return socket.gethostname() or "unknown-host"


def random_token() -> str:
    return secrets.token_urlsafe(32)


def authority_state() -> dict[str, Any]:
    try:
        from authority_state import ArtifactAuthorityStateV1
    except ModuleNotFoundError:
        from scripts.authority_state import ArtifactAuthorityStateV1

    return ArtifactAuthorityStateV1.evidence_only().canonical_record()


def ensure_owner_directory(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True, mode=0o700)
    try:
        path.chmod(0o700)
    except OSError:
        pass


def atomic_write_bytes(path: Path, payload: bytes, *, mode: int = 0o600) -> None:
    ensure_owner_directory(path.parent)
    fd, raw_tmp = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    tmp = Path(raw_tmp)
    try:
        os.fchmod(fd, mode)
        with os.fdopen(fd, "wb") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(tmp, path)
        try:
            path.chmod(mode)
        except OSError:
            pass
        if hasattr(os, "O_DIRECTORY"):
            directory_fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
            try:
                os.fsync(directory_fd)
            finally:
                os.close(directory_fd)
    finally:
        try:
            tmp.unlink()
        except OSError:
            pass


def atomic_write_json(path: Path, value: Any, *, mode: int = 0o600) -> None:
    atomic_write_bytes(
        path,
        (json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n").encode(
            "utf-8"
        ),
        mode=mode,
    )


def load_json(path: Path) -> dict[str, Any] | None:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return value if isinstance(value, dict) else None


@contextmanager
def exclusive_file_lock(path: Path) -> Iterator[None]:
    ensure_owner_directory(path.parent)
    with path.open("a+b") as handle:
        try:
            os.fchmod(handle.fileno(), 0o600)
        except OSError:
            pass
        if fcntl is not None:
            fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
        elif msvcrt is not None:  # pragma: no cover - Windows compatibility
            handle.seek(0)
            handle.write(b"\0")
            handle.flush()
            msvcrt.locking(handle.fileno(), msvcrt.LK_LOCK, 1)
        try:
            yield
        finally:
            if fcntl is not None:
                fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
            elif msvcrt is not None:  # pragma: no cover - Windows compatibility
                handle.seek(0)
                msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)
