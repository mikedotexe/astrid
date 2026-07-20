"""Rebuildable SQLite read index for the canonical Evidence Event Store V2."""

from __future__ import annotations

from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import stat
import time
from typing import Any, Iterator

try:
    from authority_state import (
        ArtifactAuthorityStateV1,
        assert_artifact_authority_tree,
    )
except ModuleNotFoundError:
    from scripts.authority_state import (
        ArtifactAuthorityStateV1,
        assert_artifact_authority_tree,
    )

from .model import EvidenceEventV2, GENESIS_HASH

INDEX_SCHEMA = "evidence_read_index_v1"
INDEX_SCHEMA_VERSION = 1


class EvidenceReadIndexError(RuntimeError):
    """The derived index does not faithfully describe the canonical JSONL."""


def _fsync_directory(path: Path) -> None:
    try:
        directory_fd = os.open(path, os.O_RDONLY)
    except OSError:
        return
    try:
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)


class EvidenceReadIndex:
    """Offset and identity index; never an authority or evidence source."""

    def __init__(self, root: Path, events_path: Path, head_path: Path):
        self.root = Path(root)
        self.events_path = Path(events_path)
        self.head_path = Path(head_path)
        self.path = self.root / "read_index_v1.sqlite3"

    def _connect(self, path: Path | None = None) -> sqlite3.Connection:
        target = self.path if path is None else path
        connection = sqlite3.connect(target, timeout=30)
        connection.row_factory = sqlite3.Row
        connection.execute("PRAGMA foreign_keys = ON")
        connection.execute("PRAGMA journal_mode = DELETE")
        connection.execute("PRAGMA synchronous = FULL")
        return connection

    @staticmethod
    def _create_schema(connection: sqlite3.Connection) -> None:
        connection.executescript(
            """
            CREATE TABLE metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            ) WITHOUT ROWID;
            CREATE TABLE events (
                global_seq INTEGER PRIMARY KEY,
                stream TEXT NOT NULL,
                stream_seq INTEGER NOT NULL,
                file_offset INTEGER NOT NULL,
                byte_length INTEGER NOT NULL,
                event_id TEXT NOT NULL UNIQUE,
                event_sha256 TEXT NOT NULL,
                previous_event_sha256 TEXT NOT NULL,
                idempotency_key TEXT
            );
            CREATE UNIQUE INDEX events_stream_sequence
                ON events(stream, stream_seq);
            CREATE UNIQUE INDEX events_stream_idempotency
                ON events(stream, idempotency_key)
                WHERE idempotency_key IS NOT NULL;
            """
        )

    @staticmethod
    def _metadata(connection: sqlite3.Connection) -> dict[str, str]:
        return {
            str(row["key"]): str(row["value"])
            for row in connection.execute("SELECT key, value FROM metadata")
        }

    @staticmethod
    def _set_metadata(
        connection: sqlite3.Connection,
        values: dict[str, str | int],
    ) -> None:
        connection.executemany(
            """
            INSERT INTO metadata(key, value) VALUES (?, ?)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            """,
            [(key, str(value)) for key, value in values.items()],
        )

    def _read_head(self) -> dict[str, Any]:
        if not self.head_path.is_file():
            return {
                "last_global_seq": 0,
                "last_event_sha256": GENESIS_HASH,
                "stream_sequences": {},
            }
        try:
            value = json.loads(self.head_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise EvidenceReadIndexError(f"cannot read canonical head: {error}") from error
        if not isinstance(value, dict):
            raise EvidenceReadIndexError("canonical head is not an object")
        return value

    @staticmethod
    def _event_from_line(line: bytes, offset: int) -> EvidenceEventV2:
        if not line.endswith(b"\n"):
            raise EvidenceReadIndexError(
                f"canonical event at byte {offset} has an incomplete tail"
            )
        try:
            value = json.loads(line)
            if not isinstance(value, dict):
                raise TypeError("event is not an object")
            return EvidenceEventV2.from_dict(value)
        except (json.JSONDecodeError, TypeError, ValueError) as error:
            raise EvidenceReadIndexError(
                f"invalid canonical event at byte {offset}: {error}"
            ) from error

    @staticmethod
    def _validate_event(
        event: EvidenceEventV2,
        *,
        expected_global_seq: int,
        expected_previous_hash: str,
    ) -> None:
        if event.global_seq != expected_global_seq:
            raise EvidenceReadIndexError(
                f"global sequence expected {expected_global_seq}, got {event.global_seq}"
            )
        if event.previous_event_sha256 != expected_previous_hash:
            raise EvidenceReadIndexError(
                f"event {event.global_seq} does not extend the indexed hash chain"
            )
        if event.event_sha256 != event.calculated_sha256():
            raise EvidenceReadIndexError(
                f"event {event.global_seq} has an invalid content hash"
            )

    @staticmethod
    def _insert_event(
        connection: sqlite3.Connection,
        event: EvidenceEventV2,
        offset: int,
        byte_length: int,
    ) -> None:
        try:
            connection.execute(
                """
                INSERT INTO events(
                    global_seq, stream, stream_seq, file_offset, byte_length,
                    event_id, event_sha256, previous_event_sha256, idempotency_key
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    event.global_seq,
                    event.stream,
                    event.stream_seq,
                    offset,
                    byte_length,
                    event.event_id,
                    event.event_sha256,
                    event.previous_event_sha256,
                    event.idempotency_key,
                ),
            )
        except sqlite3.IntegrityError as error:
            raise EvidenceReadIndexError(
                f"duplicate indexed event identity at global sequence {event.global_seq}"
            ) from error

    def _scan_into(
        self,
        connection: sqlite3.Connection,
        *,
        offset: int,
        global_seq: int,
        previous_hash: str,
    ) -> tuple[int, int, str]:
        if not self.events_path.is_file():
            if offset or global_seq:
                raise EvidenceReadIndexError("canonical event file disappeared")
            return 0, 0, GENESIS_HASH
        stream_sequences = {
            str(row["stream"]): int(row["stream_seq"])
            for row in connection.execute(
                """
                SELECT stream, MAX(stream_seq) AS stream_seq
                FROM events GROUP BY stream
                """
            )
        }
        with self.events_path.open("rb") as handle:
            handle.seek(offset)
            while True:
                line_offset = handle.tell()
                line = handle.readline()
                if not line:
                    break
                event = self._event_from_line(line, line_offset)
                expected_seq = global_seq + 1
                self._validate_event(
                    event,
                    expected_global_seq=expected_seq,
                    expected_previous_hash=previous_hash,
                )
                expected_stream_seq = stream_sequences.get(event.stream, 0) + 1
                if event.stream_seq != expected_stream_seq:
                    raise EvidenceReadIndexError(
                        f"stream {event.stream!r} sequence expected "
                        f"{expected_stream_seq}, got {event.stream_seq}"
                    )
                try:
                    state = str(
                        event.artifact_authority_state_v1.get("state") or ""
                    )
                    ArtifactAuthorityStateV1(state)
                    assert_artifact_authority_tree(event.payload)
                except (TypeError, ValueError) as error:
                    raise EvidenceReadIndexError(
                        f"event {event.global_seq} has invalid authority: {error}"
                    ) from error
                self._insert_event(connection, event, line_offset, len(line))
                global_seq = event.global_seq
                previous_hash = event.event_sha256
                stream_sequences[event.stream] = event.stream_seq
            offset = handle.tell()
        return offset, global_seq, previous_hash

    @staticmethod
    def _assert_streams_match_head(
        connection: sqlite3.Connection,
        head: dict[str, Any],
    ) -> None:
        indexed_streams = {
            str(row["stream"]): int(row["count"])
            for row in connection.execute(
                """
                SELECT stream, COUNT(*) AS count
                FROM events GROUP BY stream
                """
            )
        }
        head_streams = {
            str(stream): int(sequence)
            for stream, sequence in (head.get("stream_sequences") or {}).items()
        }
        if indexed_streams != head_streams:
            raise EvidenceReadIndexError(
                "indexed stream sequences do not match canonical head"
            )

    def _assert_matches_head(
        self,
        *,
        indexed_bytes: int,
        global_seq: int,
        last_hash: str,
        head: dict[str, Any],
    ) -> None:
        canonical_size = self.events_path.stat().st_size if self.events_path.exists() else 0
        if indexed_bytes != canonical_size:
            raise EvidenceReadIndexError("index does not cover the canonical byte tail")
        if global_seq != int(head.get("last_global_seq") or 0):
            raise EvidenceReadIndexError("index global sequence does not match canonical head")
        if last_hash != str(head.get("last_event_sha256") or GENESIS_HASH):
            raise EvidenceReadIndexError("index hash does not match canonical head")

    def rebuild(self) -> dict[str, Any]:
        """Build a complete replacement after the caller verifies canonical V2."""

        self.root.mkdir(parents=True, exist_ok=True)
        tmp = self.path.with_name(
            f".{self.path.name}.{os.getpid()}.{time.time_ns()}.tmp"
        )
        try:
            with closing(self._connect(tmp)) as connection:
                self._create_schema(connection)
                indexed_bytes, global_seq, last_hash = self._scan_into(
                    connection,
                    offset=0,
                    global_seq=0,
                    previous_hash=GENESIS_HASH,
                )
                head = self._read_head()
                self._assert_matches_head(
                    indexed_bytes=indexed_bytes,
                    global_seq=global_seq,
                    last_hash=last_hash,
                    head=head,
                )
                self._assert_streams_match_head(connection, head)
                self._set_metadata(
                    connection,
                    {
                        "schema": INDEX_SCHEMA,
                        "schema_version": INDEX_SCHEMA_VERSION,
                        "indexed_bytes": indexed_bytes,
                        "last_global_seq": global_seq,
                        "last_event_sha256": last_hash,
                    },
                )
                connection.commit()
            os.chmod(tmp, 0o600)
            os.replace(tmp, self.path)
            os.chmod(self.path, 0o600)
            _fsync_directory(self.root)
        finally:
            tmp.unlink(missing_ok=True)
        return self.status(include_details=False)

    def _assert_anchor(
        self,
        connection: sqlite3.Connection,
        metadata: dict[str, str],
    ) -> None:
        global_seq = int(metadata.get("last_global_seq") or 0)
        if global_seq == 0:
            return
        row = connection.execute(
            """
            SELECT file_offset, byte_length, event_id, event_sha256
            FROM events WHERE global_seq = ?
            """,
            (global_seq,),
        ).fetchone()
        if row is None:
            raise EvidenceReadIndexError("index anchor row is missing")
        with self.events_path.open("rb") as handle:
            handle.seek(int(row["file_offset"]))
            line = handle.read(int(row["byte_length"]))
        event = self._event_from_line(line, int(row["file_offset"]))
        if (
            event.event_id != row["event_id"]
            or event.event_sha256 != row["event_sha256"]
            or event.event_sha256 != metadata.get("last_event_sha256")
            or event.event_sha256 != event.calculated_sha256()
        ):
            raise EvidenceReadIndexError("canonical bytes do not match the index anchor")

    def reconcile(self) -> dict[str, Any]:
        """Validate the indexed anchor and consume a valid canonical tail."""

        if not self.path.is_file():
            return self.rebuild()
        os.chmod(self.path, 0o600)
        with closing(self._connect()) as connection:
            metadata = self._metadata(connection)
            if (
                metadata.get("schema") != INDEX_SCHEMA
                or int(metadata.get("schema_version") or 0) != INDEX_SCHEMA_VERSION
            ):
                raise EvidenceReadIndexError("unsupported read-index schema")
            indexed_bytes = int(metadata.get("indexed_bytes") or 0)
            canonical_size = (
                self.events_path.stat().st_size if self.events_path.exists() else 0
            )
            if canonical_size < indexed_bytes:
                raise EvidenceReadIndexError("canonical event file was truncated")
            self._assert_anchor(connection, metadata)
            indexed_bytes, global_seq, last_hash = self._scan_into(
                connection,
                offset=indexed_bytes,
                global_seq=int(metadata.get("last_global_seq") or 0),
                previous_hash=str(
                    metadata.get("last_event_sha256") or GENESIS_HASH
                ),
            )
            head = self._read_head()
            self._assert_matches_head(
                indexed_bytes=indexed_bytes,
                global_seq=global_seq,
                last_hash=last_hash,
                head=head,
            )
            self._assert_streams_match_head(connection, head)
            self._set_metadata(
                connection,
                {
                    "indexed_bytes": indexed_bytes,
                    "last_global_seq": global_seq,
                    "last_event_sha256": last_hash,
                },
            )
            connection.commit()
        return self.status(include_details=False)

    def status(
        self,
        *,
        include_details: bool = True,
        include_logical_digest: bool = True,
    ) -> dict[str, Any]:
        result: dict[str, Any] = {
            "schema": "evidence_read_index_status_v1",
            "schema_version": 1,
            "path": str(self.path),
            "exists": self.path.is_file(),
            "derived_only": True,
            "authority_source": False,
        }
        if not self.path.is_file():
            return result
        try:
            with closing(self._connect()) as connection:
                metadata = self._metadata(connection)
            head = self._read_head()
            file_size = self.events_path.stat().st_size if self.events_path.exists() else 0
            result.update(
                {
                    "valid_schema": metadata.get("schema") == INDEX_SCHEMA,
                    "permissions": oct(stat.S_IMODE(self.path.stat().st_mode)),
                    "event_count": int(metadata.get("last_global_seq") or 0),
                    "indexed_bytes": int(metadata.get("indexed_bytes") or 0),
                    "canonical_bytes": file_size,
                    "last_global_seq": int(metadata.get("last_global_seq") or 0),
                    "last_event_sha256": metadata.get("last_event_sha256"),
                    "matches_head": (
                        int(metadata.get("indexed_bytes") or 0) == file_size
                        and int(metadata.get("last_global_seq") or 0)
                        == int(head.get("last_global_seq") or 0)
                        and metadata.get("last_event_sha256")
                        == str(head.get("last_event_sha256") or GENESIS_HASH)
                    ),
                }
            )
            if include_details:
                with closing(self._connect()) as connection:
                    result["stream_counts"] = {
                        str(row["stream"]): int(row["count"])
                        for row in connection.execute(
                            """
                            SELECT stream, COUNT(*) AS count
                            FROM events GROUP BY stream ORDER BY stream
                            """
                        )
                    }
                    if include_logical_digest:
                        logical = hashlib.sha256()
                        for row in connection.execute(
                            """
                            SELECT global_seq, stream, stream_seq, event_id,
                                   event_sha256,
                                   COALESCE(idempotency_key, '') AS idempotency_key
                            FROM events ORDER BY global_seq
                            """
                        ):
                            logical.update(
                                (
                                    f"{row['global_seq']}\0{row['stream']}\0"
                                    f"{row['stream_seq']}\0{row['event_id']}\0"
                                    f"{row['event_sha256']}\0"
                                    f"{row['idempotency_key']}\n"
                                ).encode()
                            )
                        result["logical_sha256"] = logical.hexdigest()
        except (OSError, sqlite3.DatabaseError, EvidenceReadIndexError) as error:
            result.update({"valid_schema": False, "matches_head": False, "error": str(error)})
        return result

    def verify(self) -> dict[str, Any]:
        """Verify every indexed row against canonical bytes after V2 verification."""

        status = self.status()
        errors: list[str] = []
        if not status.get("exists"):
            errors.append("index_missing")
        elif not status.get("valid_schema"):
            errors.append("index_schema_invalid")
        elif not status.get("matches_head"):
            errors.append("index_head_mismatch")
        if errors:
            return {**status, "valid": False, "errors": errors}
        try:
            with closing(self._connect()) as connection, self.events_path.open("rb") as handle:
                expected_previous = GENESIS_HASH
                expected_global_seq = 1
                for row in connection.execute(
                    "SELECT * FROM events ORDER BY global_seq"
                ):
                    handle.seek(int(row["file_offset"]))
                    line = handle.read(int(row["byte_length"]))
                    event = self._event_from_line(line, int(row["file_offset"]))
                    self._validate_event(
                        event,
                        expected_global_seq=expected_global_seq,
                        expected_previous_hash=expected_previous,
                    )
                    if (
                        event.event_id != row["event_id"]
                        or event.stream != row["stream"]
                        or event.stream_seq != row["stream_seq"]
                        or event.event_sha256 != row["event_sha256"]
                        or event.idempotency_key != row["idempotency_key"]
                    ):
                        raise EvidenceReadIndexError(
                            f"indexed identity mismatch at {expected_global_seq}"
                        )
                    expected_global_seq += 1
                    expected_previous = event.event_sha256
        except (OSError, sqlite3.DatabaseError, EvidenceReadIndexError) as error:
            errors.append(str(error))
        return {**self.status(), "valid": not errors, "errors": errors}

    def _event_for_row(self, row: sqlite3.Row) -> EvidenceEventV2:
        with self.events_path.open("rb") as handle:
            handle.seek(int(row["file_offset"]))
            line = handle.read(int(row["byte_length"]))
        event = self._event_from_line(line, int(row["file_offset"]))
        if event.event_sha256 != row["event_sha256"]:
            raise EvidenceReadIndexError("indexed event bytes changed")
        return event

    def event_for_idempotency(
        self,
        stream: str,
        idempotency_key: str,
    ) -> EvidenceEventV2 | None:
        with closing(self._connect()) as connection:
            row = connection.execute(
                """
                SELECT * FROM events
                WHERE stream = ? AND idempotency_key = ?
                """,
                (stream, idempotency_key),
            ).fetchone()
            return None if row is None else self._event_for_row(row)

    def idempotency_keys(self, stream: str) -> set[str]:
        with closing(self._connect()) as connection:
            return {
                str(row["idempotency_key"])
                for row in connection.execute(
                    """
                    SELECT idempotency_key FROM events
                    WHERE stream = ? AND idempotency_key IS NOT NULL
                    """,
                    (stream,),
                )
            }

    def has_anchor(self, global_seq: int, event_sha256: str) -> bool:
        if global_seq == 0:
            return event_sha256 == GENESIS_HASH
        if not self.path.is_file():
            return False
        try:
            with closing(self._connect()) as connection:
                row = connection.execute(
                    "SELECT event_sha256 FROM events WHERE global_seq = ?",
                    (global_seq,),
                ).fetchone()
            return row is not None and row["event_sha256"] == event_sha256
        except sqlite3.DatabaseError:
            return False

    def iter_stream(
        self,
        stream: str,
        *,
        after_stream_seq: int = 0,
    ) -> Iterator[EvidenceEventV2]:
        with (
            closing(self._connect()) as connection,
            self.events_path.open("rb") as handle,
        ):
            rows = connection.execute(
                """
                SELECT * FROM events
                WHERE stream = ? AND stream_seq > ?
                ORDER BY stream_seq
                """,
                (stream, after_stream_seq),
            )
            for row in rows:
                handle.seek(int(row["file_offset"]))
                line = handle.read(int(row["byte_length"]))
                event = self._event_from_line(line, int(row["file_offset"]))
                if event.event_sha256 != row["event_sha256"]:
                    raise EvidenceReadIndexError("indexed event bytes changed")
                yield event

    def stream_watermarks(
        self,
        streams: list[str],
    ) -> dict[str, dict[str, Any]]:
        watermarks = {
            stream: {
                "stream_seq": 0,
                "last_global_seq": 0,
                "last_event_id": None,
                "last_event_sha256": GENESIS_HASH,
            }
            for stream in streams
        }
        if not streams:
            return watermarks
        placeholders = ",".join("?" for _ in streams)
        with closing(self._connect()) as connection:
            rows = connection.execute(
                f"""
                SELECT event_id, event_sha256, global_seq, stream, stream_seq
                FROM events
                WHERE stream IN ({placeholders})
                  AND (stream, stream_seq) IN (
                    SELECT stream, MAX(stream_seq)
                    FROM events WHERE stream IN ({placeholders})
                    GROUP BY stream
                  )
                """,
                [*streams, *streams],
            )
            for row in rows:
                watermarks[str(row["stream"])] = {
                    "stream_seq": int(row["stream_seq"]),
                    "last_global_seq": int(row["global_seq"]),
                    "last_event_id": str(row["event_id"]),
                    "last_event_sha256": str(row["event_sha256"]),
                }
        return watermarks
