"""Owner-only cursors for append-only and content-addressed projector inputs."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import stat
import tempfile
from typing import Any, Iterable

CURSOR_SCHEMA = "projection_input_cursor_v1"
CURSOR_SCHEMA_VERSION = 1


class ProjectionCursorError(RuntimeError):
    """An input changed behind its settled append-only watermark."""


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _atomic_write(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    os.chmod(path.parent, 0o700)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.",
        dir=path.parent,
    )
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(value, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temporary_name, 0o600)
        os.replace(temporary_name, path)
        directory_fd = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    finally:
        if os.path.exists(temporary_name):
            os.unlink(temporary_name)


class ProjectionInputCursor:
    """Rebuildable cursor state; it grants no authority."""

    def __init__(self, path: Path, projector: str):
        self.path = Path(path)
        self.projector = projector
        self.value = self._load()

    def _empty(self) -> dict[str, Any]:
        return {
            "schema": CURSOR_SCHEMA,
            "schema_version": CURSOR_SCHEMA_VERSION,
            "projector": self.projector,
            "authoritative": False,
            "append_only": True,
            "jsonl": {},
            "files": {},
        }

    def _load(self) -> dict[str, Any]:
        if not self.path.is_file():
            return self._empty()
        mode = stat.S_IMODE(self.path.stat().st_mode)
        if mode & 0o077:
            raise ProjectionCursorError(
                f"cursor must be owner-only, found mode {mode:o}"
            )
        try:
            value = json.loads(self.path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise ProjectionCursorError(f"invalid cursor: {error}") from error
        if (
            not isinstance(value, dict)
            or value.get("schema") != CURSOR_SCHEMA
            or value.get("projector") != self.projector
        ):
            raise ProjectionCursorError("cursor schema or projector mismatch")
        return value

    def jsonl_tail(
        self,
        path: Path,
        *,
        key: str,
    ) -> tuple[list[tuple[int, str]], dict[str, Any]]:
        source = path.read_bytes() if path.is_file() else b""
        prior = (self.value.get("jsonl") or {}).get(key)
        prior = prior if isinstance(prior, dict) else {}
        offset = int(prior.get("offset") or 0)
        if len(source) < offset:
            raise ProjectionCursorError(f"{key}:append_only_source_truncated")
        prefix = source[:offset]
        expected_prefix = str(
            prior.get("settled_prefix_sha256") or sha256_bytes(b"")
        )
        if offset and sha256_bytes(prefix) != expected_prefix:
            raise ProjectionCursorError(f"{key}:settled_prefix_tampered")
        tail = source[offset:]
        if tail and not tail.endswith(b"\n"):
            raise ProjectionCursorError(f"{key}:incomplete_jsonl_tail")
        try:
            decoded = tail.decode("utf-8")
        except UnicodeDecodeError as error:
            raise ProjectionCursorError(f"{key}:invalid_utf8_tail") from error
        start_line = int(prior.get("line_count") or 0)
        rows = [
            (start_line + index, raw)
            for index, raw in enumerate(decoded.splitlines(), 1)
        ]
        next_state = {
            **prior,
            "offset": len(source),
            "line_count": start_line + len(decoded.splitlines()),
            "settled_prefix_sha256": sha256_bytes(source),
            "source_sha256": sha256_bytes(source),
            "last_raw_sha256": (
                sha256_bytes(rows[-1][1].encode("utf-8"))
                if rows
                else prior.get("last_raw_sha256")
            ),
        }
        return rows, next_state

    def changed_files(
        self,
        paths: Iterable[Path],
        *,
        root: Path,
    ) -> tuple[list[Path], dict[str, dict[str, Any]], list[str]]:
        prior_files = self.value.get("files")
        prior_files = prior_files if isinstance(prior_files, dict) else {}
        manifest: dict[str, dict[str, Any]] = {}
        changed: list[Path] = []
        for path in sorted({Path(item) for item in paths}):
            relative = path.relative_to(root).as_posix()
            raw = path.read_bytes()
            record = {
                "sha256": sha256_bytes(raw),
                "size": len(raw),
                "present": True,
            }
            manifest[relative] = record
            if prior_files.get(relative) != record:
                changed.append(path)
        removed = sorted(set(prior_files) - set(manifest))
        return changed, manifest, removed

    def jsonl_metadata(self, key: str) -> dict[str, Any]:
        value = (self.value.get("jsonl") or {}).get(key)
        return dict(value) if isinstance(value, dict) else {}

    def commit_jsonl(
        self,
        updates: dict[str, dict[str, Any]],
    ) -> None:
        value = dict(self.value)
        jsonl = dict(value.get("jsonl") or {})
        jsonl.update(updates)
        value["jsonl"] = dict(sorted(jsonl.items()))
        _atomic_write(self.path, value)
        self.value = value

    def commit_files(
        self,
        manifest: dict[str, dict[str, Any]],
    ) -> None:
        value = dict(self.value)
        value["files"] = dict(sorted(manifest.items()))
        _atomic_write(self.path, value)
        self.value = value
