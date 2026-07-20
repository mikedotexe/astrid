"""Rebuildable SQLite state for incremental felt-contract projection."""

from __future__ import annotations

from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import stat
import tempfile
from typing import Any, Iterable

try:
    from evidence_store.model import EvidenceEventV2, canonical_json, sha256_canonical
except ModuleNotFoundError:
    from scripts.evidence_store.model import (
        EvidenceEventV2,
        canonical_json,
        sha256_canonical,
    )

STATE_SCHEMA = "felt_contract_derived_state_v1"
STATE_SCHEMA_VERSION = 1
_ADDRESSING_IGNORED_EVENTS = {
    "inventory_run",
    "inventory_artifact",
    "inventory_artifact_absent",
}
_CLAIM_FAMILY_RETAINED_EVENTS = {
    "claim_family_membership_assigned",
    "claim_family_membership_corrected",
    "experiment_dossier_projected",
    "experiment_dossier_trial_unrouted",
    "felt_review_packet_delivered",
    "felt_review_response_recorded",
}


class FeltContractStateError(RuntimeError):
    """The rebuildable state is missing, corrupt, or inconsistent."""


def _fsync_directory(path: Path) -> None:
    try:
        descriptor = os.open(path, os.O_RDONLY)
    except OSError:
        return
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


class FeltContractStateIndex:
    """Materialized graph state derived solely from canonical V2 events."""

    def __init__(self, root: Path):
        self.root = Path(root)
        self.path = self.root / "derived_state_v1.sqlite3"

    @staticmethod
    def _connect_path(path: Path) -> sqlite3.Connection:
        connection = sqlite3.connect(path, timeout=30)
        connection.row_factory = sqlite3.Row
        connection.execute("PRAGMA foreign_keys = ON")
        connection.execute("PRAGMA journal_mode = DELETE")
        connection.execute("PRAGMA synchronous = FULL")
        return connection

    def _connect(self) -> sqlite3.Connection:
        return self._connect_path(self.path)

    @staticmethod
    def _create_schema(connection: sqlite3.Connection) -> None:
        connection.executescript(
            """
            CREATE TABLE metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            ) WITHOUT ROWID;
            CREATE TABLE watermarks (
                stream TEXT PRIMARY KEY,
                stream_seq INTEGER NOT NULL,
                global_seq INTEGER NOT NULL,
                event_id TEXT NOT NULL,
                event_sha256 TEXT NOT NULL
            ) WITHOUT ROWID;
            CREATE TABLE source_events (
                event_id TEXT PRIMARY KEY,
                stream TEXT NOT NULL,
                stream_seq INTEGER NOT NULL,
                global_seq INTEGER NOT NULL UNIQUE,
                event_sha256 TEXT NOT NULL,
                event_type TEXT NOT NULL,
                semantic_key TEXT,
                retained INTEGER NOT NULL,
                envelope_json TEXT NOT NULL,
                UNIQUE(stream, stream_seq)
            ) WITHOUT ROWID;
            CREATE INDEX source_events_retained
                ON source_events(retained, global_seq);
            CREATE INDEX source_events_semantic
                ON source_events(semantic_key)
                WHERE semantic_key IS NOT NULL;
            CREATE TABLE graph_events (
                event_id TEXT PRIMARY KEY,
                stream_seq INTEGER NOT NULL UNIQUE,
                global_seq INTEGER NOT NULL UNIQUE,
                event_sha256 TEXT NOT NULL,
                envelope_json TEXT NOT NULL
            ) WITHOUT ROWID;
            CREATE TABLE contracts (
                contract_id TEXT PRIMARY KEY,
                record_json TEXT NOT NULL
            ) WITHOUT ROWID;
            CREATE TABLE nodes (
                node_id TEXT PRIMARY KEY,
                contract_id TEXT NOT NULL,
                record_json TEXT NOT NULL
            ) WITHOUT ROWID;
            CREATE INDEX nodes_contract ON nodes(contract_id);
            CREATE TABLE edges (
                edge_id TEXT PRIMARY KEY,
                contract_id TEXT NOT NULL,
                record_json TEXT NOT NULL
            ) WITHOUT ROWID;
            CREATE INDEX edges_contract ON edges(contract_id);
            CREATE TABLE membership (
                claim_id TEXT PRIMARY KEY,
                contract_id TEXT NOT NULL
            ) WITHOUT ROWID;
            CREATE TABLE review_budgets (
                contract_id TEXT PRIMARY KEY,
                record_json TEXT NOT NULL
            ) WITHOUT ROWID;
            """
        )
        connection.executemany(
            "INSERT INTO metadata(key, value) VALUES (?, ?)",
            (
                ("schema", STATE_SCHEMA),
                ("schema_version", str(STATE_SCHEMA_VERSION)),
                ("materialized", "false"),
            ),
        )

    def initialize(self, *, replace: bool = False) -> None:
        self.root.mkdir(parents=True, exist_ok=True)
        if self.path.exists() and not replace:
            self._validate_permissions()
            return
        descriptor, temporary_name = tempfile.mkstemp(
            prefix=f".{self.path.name}.",
            suffix=".tmp",
            dir=self.root,
        )
        os.close(descriptor)
        temporary = Path(temporary_name)
        try:
            os.chmod(temporary, 0o600)
            with closing(self._connect_path(temporary)) as connection:
                self._create_schema(connection)
                connection.commit()
            os.replace(temporary, self.path)
            os.chmod(self.path, 0o600)
            _fsync_directory(self.root)
        finally:
            temporary.unlink(missing_ok=True)

    def _validate_permissions(self) -> None:
        mode = stat.S_IMODE(self.path.stat().st_mode)
        if mode & 0o077:
            raise FeltContractStateError(
                f"derived state must be owner-only, found mode {mode:o}"
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
        values: dict[str, Any],
    ) -> None:
        connection.executemany(
            """
            INSERT INTO metadata(key, value) VALUES (?, ?)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            """,
            [
                (
                    key,
                    value
                    if isinstance(value, str)
                    else canonical_json(value),
                )
                for key, value in values.items()
            ],
        )

    @staticmethod
    def _source_retention(
        event: EvidenceEventV2,
    ) -> tuple[bool, str | None]:
        event_type = str(event.payload.get("event_type") or "")
        if (
            event.stream == "addressing"
            and event_type in _ADDRESSING_IGNORED_EVENTS
        ):
            return False, None
        if event.stream != "claim_families":
            return True, None
        if event_type not in _CLAIM_FAMILY_RETAINED_EVENTS:
            return False, None
        if event_type in {
            "claim_family_membership_assigned",
            "claim_family_membership_corrected",
        }:
            claim_id = str(event.payload.get("canonical_claim_id") or "")
            return True, f"claim_family_membership:{claim_id}" if claim_id else None
        if event_type == "experiment_dossier_projected":
            dossier_id = str(event.payload.get("dossier_id") or "")
            return True, f"experiment_dossier:{dossier_id}" if dossier_id else None
        if event_type == "experiment_dossier_trial_unrouted":
            trial_id = str(event.payload.get("trial_id") or "")
            return True, f"unrouted_trial:{trial_id}" if trial_id else None
        return True, None

    @staticmethod
    def _source_identity_json(
        *,
        event_id: str,
        stream: str,
        stream_seq: int,
        global_seq: int,
        event_sha256: str,
    ) -> str:
        return canonical_json(
            {
                "schema": "felt_contract_source_event_identity_v1",
                "schema_version": 1,
                "event_id": event_id,
                "stream": stream,
                "stream_seq": stream_seq,
                "global_seq": global_seq,
                "event_sha256": event_sha256,
                "payload_retained": False,
            }
        )

    @staticmethod
    def _insert_event(
        connection: sqlite3.Connection,
        table: str,
        event: EvidenceEventV2,
    ) -> bool:
        existing = connection.execute(
            f"SELECT event_sha256 FROM {table} WHERE event_id = ?",
            (event.event_id,),
        ).fetchone()
        if existing is not None:
            if str(existing["event_sha256"]) != event.event_sha256:
                raise FeltContractStateError(
                    f"event identity changed in {table}: {event.event_id}"
                )
            return False
        values = (
            event.event_id,
            event.stream_seq,
            event.global_seq,
            event.event_sha256,
            canonical_json(event.to_dict()),
        )
        if table == "source_events":
            retained, semantic_key = FeltContractStateIndex._source_retention(
                event
            )
            if semantic_key:
                previous_rows = connection.execute(
                    """
                    SELECT event_id, stream, stream_seq, global_seq,
                           event_sha256
                    FROM source_events
                    WHERE semantic_key = ? AND retained = 1
                    """,
                    (semantic_key,),
                ).fetchall()
                for row in previous_rows:
                    connection.execute(
                        """
                        UPDATE source_events
                        SET retained = 0, envelope_json = ?
                        WHERE event_id = ?
                        """,
                        (
                            FeltContractStateIndex._source_identity_json(
                                event_id=str(row["event_id"]),
                                stream=str(row["stream"]),
                                stream_seq=int(row["stream_seq"]),
                                global_seq=int(row["global_seq"]),
                                event_sha256=str(row["event_sha256"]),
                            ),
                            str(row["event_id"]),
                        ),
                    )
            connection.execute(
                """
                INSERT INTO source_events(
                    event_id, stream, stream_seq, global_seq,
                    event_sha256, event_type, semantic_key, retained,
                    envelope_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    event.event_id,
                    event.stream,
                    event.stream_seq,
                    event.global_seq,
                    event.event_sha256,
                    str(event.payload.get("event_type") or ""),
                    semantic_key,
                    int(retained),
                    (
                        values[-1]
                        if retained
                        else FeltContractStateIndex._source_identity_json(
                            event_id=event.event_id,
                            stream=event.stream,
                            stream_seq=event.stream_seq,
                            global_seq=event.global_seq,
                            event_sha256=event.event_sha256,
                        )
                    ),
                ),
            )
        else:
            connection.execute(
                """
                INSERT INTO graph_events(
                    event_id, stream_seq, global_seq, event_sha256, envelope_json
                ) VALUES (?, ?, ?, ?, ?)
                """,
                values,
            )
        connection.execute(
            """
            INSERT INTO watermarks(
                stream, stream_seq, global_seq, event_id, event_sha256
            ) VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(stream) DO UPDATE SET
                stream_seq = excluded.stream_seq,
                global_seq = excluded.global_seq,
                event_id = excluded.event_id,
                event_sha256 = excluded.event_sha256
            WHERE excluded.stream_seq > watermarks.stream_seq
            """,
            (
                event.stream,
                event.stream_seq,
                event.global_seq,
                event.event_id,
                event.event_sha256,
            ),
        )
        return True

    def ingest_source_events(self, events: Iterable[EvidenceEventV2]) -> int:
        inserted, _ = self.ingest_source_events_with_effects(events)
        return inserted

    def ingest_source_events_with_effects(
        self,
        events: Iterable[EvidenceEventV2],
    ) -> tuple[int, int]:
        self.initialize()
        inserted = 0
        retained_effects = 0
        with closing(self._connect()) as connection:
            connection.execute("BEGIN IMMEDIATE")
            for event in events:
                if event.stream == "felt_contracts":
                    raise FeltContractStateError(
                        "felt-contract events must use the graph event table"
                    )
                retained, _ = self._source_retention(event)
                was_inserted = self._insert_event(
                    connection,
                    "source_events",
                    event,
                )
                inserted += was_inserted
                retained_effects += int(was_inserted and retained)
            connection.commit()
        return inserted, retained_effects

    def ingest_graph_events(self, events: Iterable[EvidenceEventV2]) -> int:
        self.initialize()
        inserted = 0
        with closing(self._connect()) as connection:
            connection.execute("BEGIN IMMEDIATE")
            for event in events:
                if event.stream != "felt_contracts":
                    raise FeltContractStateError(
                        f"unexpected graph stream: {event.stream}"
                    )
                inserted += self._insert_event(connection, "graph_events", event)
            connection.commit()
        return inserted

    def watermark(self, stream: str) -> int:
        if not self.path.is_file():
            return 0
        self._validate_permissions()
        with closing(self._connect()) as connection:
            row = connection.execute(
                "SELECT stream_seq FROM watermarks WHERE stream = ?",
                (stream,),
            ).fetchone()
        return int(row["stream_seq"]) if row else 0

    def watermarks(self) -> dict[str, dict[str, Any]]:
        if not self.path.is_file():
            return {}
        self._validate_permissions()
        with closing(self._connect()) as connection:
            return {
                str(row["stream"]): {
                    "stream_seq": int(row["stream_seq"]),
                    "global_seq": int(row["global_seq"]),
                    "event_id": str(row["event_id"]),
                    "event_sha256": str(row["event_sha256"]),
                }
                for row in connection.execute(
                    "SELECT * FROM watermarks ORDER BY stream"
                )
            }

    def source_envelopes(
        self,
        streams: Iterable[str],
    ) -> list[EvidenceEventV2]:
        requested = sorted({str(stream) for stream in streams})
        if not requested:
            return []
        placeholders = ",".join("?" for _ in requested)
        with closing(self._connect()) as connection:
            rows = connection.execute(
                f"""
                SELECT envelope_json FROM source_events
                WHERE stream IN ({placeholders}) AND retained = 1
                ORDER BY global_seq
                """,
                requested,
            )
            return [
                EvidenceEventV2.from_dict(json.loads(row["envelope_json"]))
                for row in rows
            ]

    def current_membership(self) -> dict[str, str]:
        if not self.path.is_file():
            return {}
        with closing(self._connect()) as connection:
            return {
                str(row["claim_id"]): str(row["contract_id"])
                for row in connection.execute(
                    "SELECT claim_id, contract_id FROM membership"
                )
            }

    def current_contract_ids(self) -> set[str]:
        if not self.path.is_file():
            return set()
        with closing(self._connect()) as connection:
            return {
                str(row["contract_id"])
                for row in connection.execute(
                    "SELECT contract_id FROM contracts"
                )
            }

    def implementation_nodes(self) -> dict[tuple[str, str], str]:
        if not self.path.is_file():
            return {}
        result: dict[tuple[str, str], str] = {}
        with closing(self._connect()) as connection:
            rows = connection.execute(
                """
                SELECT node_id, contract_id, record_json FROM nodes
                WHERE record_json LIKE '%"kind":"implementation"%'
                """
            )
            for row in rows:
                record = json.loads(row["record_json"])
                metadata = record.get("metadata")
                if not isinstance(metadata, dict):
                    continue
                receipt_id = str(
                    metadata.get("implementation_receipt_id") or ""
                )
                if receipt_id:
                    result[(receipt_id, str(row["contract_id"]))] = str(
                        row["node_id"]
                    )
        return result

    def cached_status(self) -> dict[str, Any] | None:
        if not self.path.is_file():
            return None
        with closing(self._connect()) as connection:
            metadata = self._metadata(connection)
        if metadata.get("materialized") != "true":
            return None
        try:
            value = json.loads(metadata["status_json"])
        except (KeyError, json.JSONDecodeError) as error:
            raise FeltContractStateError(
                f"invalid materialized status: {error}"
            ) from error
        if not isinstance(value, dict):
            raise FeltContractStateError("materialized status is not an object")
        return value

    def count(self, table: str) -> int:
        if table not in {
            "source_events",
            "graph_events",
            "contracts",
            "nodes",
            "edges",
            "membership",
        }:
            raise ValueError(f"unsupported derived-state table: {table}")
        if not self.path.is_file():
            return 0
        with closing(self._connect()) as connection:
            return int(
                connection.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0]
            )

    def graph_envelopes(self) -> list[EvidenceEventV2]:
        with closing(self._connect()) as connection:
            rows = connection.execute(
                "SELECT envelope_json FROM graph_events ORDER BY stream_seq"
            )
            return [
                EvidenceEventV2.from_dict(json.loads(row["envelope_json"]))
                for row in rows
            ]

    def materialize(
        self,
        projection: dict[str, Any],
        *,
        source_hashes: dict[str, str],
        source_counters: dict[str, Any],
    ) -> str:
        self.initialize()
        projection_sha256 = sha256_canonical(projection)
        with closing(self._connect()) as connection:
            connection.execute("BEGIN IMMEDIATE")
            for table in (
                "contracts",
                "nodes",
                "edges",
                "membership",
                "review_budgets",
            ):
                connection.execute(f"DELETE FROM {table}")
            connection.executemany(
                "INSERT INTO contracts(contract_id, record_json) VALUES (?, ?)",
                [
                    (
                        str(record["contract_id"]),
                        canonical_json(record),
                    )
                    for record in projection["contracts"]
                ],
            )
            connection.executemany(
                """
                INSERT INTO nodes(node_id, contract_id, record_json)
                VALUES (?, ?, ?)
                """,
                [
                    (
                        str(node_id),
                        str(record.get("contract_id") or ""),
                        canonical_json(record),
                    )
                    for node_id, record in projection["nodes"].items()
                ],
            )
            connection.executemany(
                """
                INSERT INTO edges(edge_id, contract_id, record_json)
                VALUES (?, ?, ?)
                """,
                [
                    (
                        str(edge_id),
                        str(record.get("contract_id") or ""),
                        canonical_json(record),
                    )
                    for edge_id, record in projection["edges"].items()
                ],
            )
            connection.executemany(
                "INSERT INTO membership(claim_id, contract_id) VALUES (?, ?)",
                sorted(projection["membership"].items()),
            )
            connection.executemany(
                """
                INSERT INTO review_budgets(contract_id, record_json)
                VALUES (?, ?)
                """,
                [
                    (str(contract_id), canonical_json(record))
                    for contract_id, record in projection["review_budgets"].items()
                ],
            )
            self._set_metadata(
                connection,
                {
                    "materialized": "true",
                    "projection_sha256": projection_sha256,
                    "status_json": projection["status"],
                    "source_hashes": source_hashes,
                    "source_counters": source_counters,
                },
            )
            connection.commit()
        return projection_sha256

    def load_projection(self) -> dict[str, Any] | None:
        if not self.path.is_file():
            return None
        self._validate_permissions()
        with closing(self._connect()) as connection:
            metadata = self._metadata(connection)
            if metadata.get("materialized") != "true":
                return None
            try:
                status = json.loads(metadata["status_json"])
                contracts = [
                    json.loads(row["record_json"])
                    for row in connection.execute(
                        "SELECT record_json FROM contracts ORDER BY contract_id"
                    )
                ]
                nodes = {
                    str(row["node_id"]): json.loads(row["record_json"])
                    for row in connection.execute(
                        "SELECT node_id, record_json FROM nodes ORDER BY node_id"
                    )
                }
                edges = {
                    str(row["edge_id"]): json.loads(row["record_json"])
                    for row in connection.execute(
                        "SELECT edge_id, record_json FROM edges ORDER BY edge_id"
                    )
                }
                membership = {
                    str(row["claim_id"]): str(row["contract_id"])
                    for row in connection.execute(
                        "SELECT claim_id, contract_id FROM membership ORDER BY claim_id"
                    )
                }
                review_budgets = {
                    str(row["contract_id"]): json.loads(row["record_json"])
                    for row in connection.execute(
                        """
                        SELECT contract_id, record_json FROM review_budgets
                        ORDER BY contract_id
                        """
                    )
                }
            except (KeyError, json.JSONDecodeError) as error:
                raise FeltContractStateError(
                    f"invalid materialized projection: {error}"
                ) from error
        projection = {
            "status": status,
            "contracts": contracts,
            "nodes": nodes,
            "edges": edges,
            "membership": membership,
            "review_budgets": review_budgets,
        }
        if sha256_canonical(projection) != metadata.get("projection_sha256"):
            raise FeltContractStateError("materialized projection hash mismatch")
        return projection

    def source_metadata(self) -> tuple[dict[str, str], dict[str, Any]]:
        if not self.path.is_file():
            return {}, {}
        with closing(self._connect()) as connection:
            metadata = self._metadata(connection)
        try:
            hashes = json.loads(metadata.get("source_hashes") or "{}")
            counters = json.loads(metadata.get("source_counters") or "{}")
        except json.JSONDecodeError as error:
            raise FeltContractStateError(
                f"invalid source metadata: {error}"
            ) from error
        return hashes, counters

    def update_source_metadata(
        self,
        *,
        source_hashes: dict[str, str],
        source_counters: dict[str, Any],
    ) -> None:
        self.initialize()
        with closing(self._connect()) as connection:
            connection.execute("BEGIN IMMEDIATE")
            self._set_metadata(
                connection,
                {
                    "source_hashes": source_hashes,
                    "source_counters": source_counters,
                },
            )
            connection.commit()

    def status(self) -> dict[str, Any]:
        if not self.path.is_file():
            return {
                "schema": STATE_SCHEMA,
                "schema_version": STATE_SCHEMA_VERSION,
                "exists": False,
                "authoritative": False,
            }
        self._validate_permissions()
        with closing(self._connect()) as connection:
            metadata = self._metadata(connection)
            counts = {
                table: int(
                    connection.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0]
                )
                for table in (
                    "source_events",
                    "graph_events",
                    "contracts",
                    "nodes",
                    "edges",
                    "membership",
                )
            }
            counts["retained_source_events"] = int(
                connection.execute(
                    "SELECT COUNT(*) FROM source_events WHERE retained = 1"
                ).fetchone()[0]
            )
        return {
            "schema": STATE_SCHEMA,
            "schema_version": STATE_SCHEMA_VERSION,
            "exists": True,
            "authoritative": False,
            "owner_only": True,
            "materialized": metadata.get("materialized") == "true",
            "projection_sha256": metadata.get("projection_sha256"),
            "counts": counts,
            "watermarks": self.watermarks(),
        }

    def logical_digest(self) -> str:
        """Stable digest that excludes SQLite page layout."""

        if not self.path.is_file():
            return hashlib.sha256(b"").hexdigest()
        with closing(self._connect()) as connection:
            value = {
                "metadata": {
                    key: value
                    for key, value in self._metadata(connection).items()
                    if key not in {"projection_sha256"}
                },
                "watermarks": [
                    tuple(row)
                    for row in connection.execute(
                        """
                        SELECT stream, stream_seq, global_seq, event_id, event_sha256
                        FROM watermarks ORDER BY stream
                        """
                    )
                ],
                "source_events": [
                    tuple(row)
                    for row in connection.execute(
                        """
                        SELECT event_id, stream, stream_seq, global_seq, event_sha256
                        FROM source_events ORDER BY global_seq
                        """
                    )
                ],
                "graph_events": [
                    tuple(row)
                    for row in connection.execute(
                        """
                        SELECT event_id, stream_seq, global_seq, event_sha256
                        FROM graph_events ORDER BY stream_seq
                        """
                    )
                ],
            }
        return sha256_canonical(value)
