"""Migrate, project, and reconcile temporal lived-state witnesses."""

from __future__ import annotations

from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import tempfile
import time
from typing import Any

try:
    from evidence_store import EvidenceEventStore
    from evidence_store.model import ProvenanceSourceV1, canonical_json
except ModuleNotFoundError:
    from scripts.evidence_store import EvidenceEventStore
    from scripts.evidence_store.model import ProvenanceSourceV1, canonical_json

from .model import (
    RECONCILIATION_OUTCOMES,
    authority_state,
    canonical_report_paths,
    deterministic_id,
    report_header,
    report_timestamp,
    sha256_bytes,
    sha256_file,
    validate_gap,
    validate_witness,
    witness_pointer,
)
from .deployments import (
    alignment as _alignment,
    deployment_before as _deployment_before,
    deployment_ref as _deployment_ref,
    environment_receipts_path as _environment_receipts_path,
    exact_deployment_match as _exact_deployment_match,
    exact_deployment_receipt as _exact_deployment_receipt,
    successful_deployments as _successful_deployments,
)
from .records import event_record, parse_source_ref

PROJECTOR_VERSION = 2
STREAM = "lived_state_witness"


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/lived_state_witness_v1"


def sidecar_dir(workspace: Path) -> Path:
    return workspace / "introspections/lived_state_witnesses"


def store_for(workspace: Path) -> EvidenceEventStore:
    return EvidenceEventStore(workspace / "diagnostics/evidence_event_store_v2")


def _load_json(path: Path) -> dict[str, Any] | None:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return value if isinstance(value, dict) else None


def _atomic_write(path: Path, payload: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, tmp_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(tmp_name, path)
        directory = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if os.path.exists(tmp_name):
            os.unlink(tmp_name)


def _roots(workspace: Path) -> dict[str, Path]:
    astrid = Path(__file__).resolve().parents[2]
    return {
        "astrid": astrid,
        "minime": astrid.parent / "minime",
        "model": astrid.parent / "neural-triple-reservoir",
    }


def _historical_witness_id(report_sha256: str) -> str:
    return f"lsw_{sha256_bytes(f'historical\0{report_sha256}'.encode())}"


def _sidecars(workspace: Path) -> tuple[dict[str, tuple[Path, dict[str, Any], list[str]]], list[dict[str, Any]]]:
    result: dict[str, tuple[Path, dict[str, Any], list[str]]] = {}
    gap_rows: list[dict[str, Any]] = []
    for path in sorted((sidecar_dir(workspace) / "witnesses").glob("*.json")):
        value = _load_json(path)
        errors = validate_witness(value)
        witness_id = str((value or {}).get("witness_id") or path.stem)
        if path.stem != witness_id:
            errors.append("sidecar_filename_witness_id_mismatch")
        if witness_id in result:
            errors.append("duplicate_witness_sidecar_id")
            prior_path, prior_value, prior_errors = result[witness_id]
            prior_errors.append("duplicate_witness_sidecar_id")
            result[witness_id] = (prior_path, prior_value, prior_errors)
            continue
        result[witness_id] = (path, value or {}, errors)
    for path in sorted((sidecar_dir(workspace) / "gaps").glob("*.json")):
        value = _load_json(path)
        errors = validate_gap(value)
        gap_rows.append(
            {
                "path": path,
                "value": value or {},
                "errors": errors,
            }
        )
    return result, gap_rows


def _report_ref(workspace: Path, path: Path, report_sha256: str) -> dict[str, Any]:
    return {
        "kind": "canonical_introspection_report",
        "relative_path": path.relative_to(workspace).as_posix(),
        "sha256": report_sha256,
        "timestamp": report_timestamp(path),
        "raw_prose_included": False,
    }


def _migrate_report(
    workspace: Path,
    path: Path,
    deployments: list[dict[str, Any]],
    sidecars: dict[str, tuple[Path, dict[str, Any], list[str]]],
    roots: dict[str, Path],
    referenced: set[str],
    counters: Counter[str],
) -> dict[str, Any]:
    report_sha256 = sha256_file(path)
    pointer = witness_pointer(path)
    header = report_header(path)
    authored_at_ms = (report_timestamp(path) or 0) * 1_000
    source_ref = parse_source_ref(str(header.get("source") or ""), roots)
    if pointer:
        referenced.add(pointer)
    sidecar = sidecars.get(pointer or "")
    errors: list[str] = []
    witness: dict[str, Any] | None = None
    if pointer and sidecar is None:
        errors.append("pointer_sidecar_missing")
    elif sidecar:
        _, candidate, validation_errors = sidecar
        errors.extend(validation_errors)
        if candidate.get("artifact_sha256") != report_sha256:
            errors.append("artifact_sha256_mismatch")
        if candidate.get("artifact_relative_path") != path.name:
            errors.append("artifact_relative_path_mismatch")
        if not errors:
            witness = candidate
    if pointer and errors:
        counters["gap"] += 1
        privacy_rejection = any(
            "private" in error or "prose" in error for error in errors
        )
        counters["privacy_rejection"] += int(privacy_rejection)
        return event_record(
            "lived_state_witness_gap_detected",
            pointer,
            f"lived-state-gap:{pointer}:{report_sha256}:"
            f"{sha256_bytes(canonical_json(errors).encode())}",
            introspection_id=path.stem,
            report_ref=_report_ref(workspace, path, report_sha256),
            errors=sorted(errors),
            privacy_rejection=privacy_rejection,
            report_remains_primary=True,
        )
    if witness is not None:
        alignment = _alignment(
            witness,
            int(witness.get("authored_at_unix_ms") or authored_at_ms),
            deployments,
            historical=False,
        )
        counters["exact"] += 1
        counters["exact_deployment_match"] += int(
            alignment["outcome"] == "same_deployment"
        )
        return event_record(
            "temporal_lived_state_witness_recorded",
            pointer or str(witness["witness_id"]),
            f"lived-state-witness:{witness['witness_id']}:{report_sha256}",
            introspection_id=path.stem,
            report_ref=_report_ref(workspace, path, report_sha256),
            witness=witness,
            alignment=alignment,
            evidence_completeness="exact_authorship_sidecar",
        )
    witness_id = _historical_witness_id(report_sha256)
    alignment = _alignment(None, authored_at_ms, deployments, historical=True)
    if alignment["outcome"] == "temporal_association_only":
        counters["temporal_only"] += 1
    else:
        counters["unknown"] += 1
    historical = {
        "schema": "historical_lived_state_witness_v1",
        "schema_version": 1,
        "witness_id": witness_id,
        "authored_at_unix_ms": authored_at_ms,
        "source_ref": source_ref,
        "fill_pct": _parse_fill(header.get("fill")),
        "source_window_hash_available": False,
        "process_identity_available": False,
        "model_route_available": False,
        "raw_introspection_prose_included": False,
        "private_path_included": False,
        "artifact_authority_state_v1": authority_state(),
    }
    return event_record(
        "historical_lived_state_witness_migrated",
        witness_id,
        f"lived-state-historical:{report_sha256}",
        introspection_id=path.stem,
        report_ref=_report_ref(workspace, path, report_sha256),
        witness=historical,
        alignment=alignment,
        evidence_completeness=(
            "temporal_only"
            if alignment["outcome"] == "temporal_association_only"
            else "historical_unrecoverable"
        ),
    )


def _unreferenced_sidecar_events(
    workspace: Path,
    sidecars: dict[str, tuple[Path, dict[str, Any], list[str]]],
    referenced: set[str],
    deployments: list[dict[str, Any]],
    counters: Counter[str],
) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    for witness_id, (path, value, errors) in sorted(sidecars.items()):
        if witness_id in referenced:
            continue
        errors = list(errors)
        relative = value.get("artifact_relative_path")
        artifact_path = (
            workspace / "introspections" / relative
            if isinstance(relative, str)
            and relative
            and not Path(relative).is_absolute()
            and ".." not in Path(relative).parts
            else None
        )
        if artifact_path is None or not artifact_path.is_file():
            errors.append("artifact_missing")
        else:
            artifact_sha256 = sha256_file(artifact_path)
            if artifact_sha256 != value.get("artifact_sha256"):
                errors.append("artifact_sha256_mismatch")
            if witness_pointer(artifact_path) != witness_id:
                errors.append("artifact_witness_pointer_mismatch")
        if not errors and artifact_path is not None:
            counters["auxiliary"] += 1
            alignment = _alignment(
                value,
                int(value.get("authored_at_unix_ms") or 0),
                deployments,
                historical=False,
            )
            events.append(
                event_record(
                    "lived_state_auxiliary_artifact_witness_recorded",
                    witness_id,
                    (
                        f"lived-state-auxiliary:{witness_id}:"
                        f"{sha256_file(path)}:{value['artifact_sha256']}"
                    ),
                    artifact_ref={
                        "kind": "noncanonical_lived_state_artifact",
                        "relative_path": artifact_path.relative_to(workspace).as_posix(),
                        "sha256": value["artifact_sha256"],
                        "canonical_queue_member": False,
                        "raw_prose_included": False,
                    },
                    artifact_kind=value.get("artifact_kind"),
                    witness=value,
                    alignment=alignment,
                    evidence_completeness="exact_auxiliary_sidecar",
                    felt_contract_ingestion_eligible=False,
                )
            )
            continue
        counters["orphan"] += 1
        counters["privacy_rejection"] += int(
            any("private" in error or "prose" in error for error in errors)
        )
        events.append(
            event_record(
                "lived_state_witness_orphan_detected",
                witness_id,
                f"lived-state-orphan:{witness_id}:{sha256_file(path)}",
                source_receipt={
                    "relative_path": path.relative_to(workspace).as_posix(),
                    "sha256": sha256_file(path),
                },
                validation_errors=errors,
                artifact_sha256=value.get("artifact_sha256"),
            )
        )
    return events


def _writer_gap_events(
    workspace: Path, writer_gaps: list[dict[str, Any]], counters: Counter[str]
) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    for row in writer_gaps:
        value = row["value"]
        witness_id = str(value.get("witness_id") or "")
        gap_receipt = value if not row["errors"] else None
        counters["gap"] += 1
        events.append(
            event_record(
                "lived_state_writer_gap_recorded",
                witness_id,
                f"lived-state-writer-gap:{sha256_file(row['path'])}",
                gap_receipt=gap_receipt,
                source_receipt={
                    "relative_path": row["path"].relative_to(workspace).as_posix(),
                    "sha256": sha256_file(row["path"]),
                },
                validation_errors=row["errors"],
                invalid_payload_copied=False,
            )
        )
    return events


def _migration_watermarks(
    workspace: Path,
    reports: list[Path],
    sidecar_count: int,
    writer_gap_count: int,
    deployment_count: int,
) -> dict[str, Any]:
    report_manifest = [
        {
            "name": path.name,
            "timestamp": report_timestamp(path),
            "sha256": sha256_file(path),
        }
        for path in reports
    ]
    receipt_path = _environment_receipts_path(workspace)
    return {
        "canonical_report_count": len(reports),
        "newest_canonical_timestamp": max(
            (report_timestamp(path) or 0 for path in reports), default=0
        )
        or None,
        "canonical_report_manifest_sha256": sha256_bytes(
            canonical_json(report_manifest).encode()
        ),
        "sidecar_count": sidecar_count,
        "writer_gap_count": writer_gap_count,
        "successful_deployment_receipt_count": deployment_count,
        "environment_receipts_sha256": (
            sha256_file(receipt_path) if receipt_path.is_file() else sha256_bytes(b"")
        ),
    }


def migration_events(workspace: Path) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    reports = canonical_report_paths(workspace)
    deployments = _successful_deployments(workspace)
    sidecars, writer_gaps = _sidecars(workspace)
    referenced: set[str] = set()
    counters: Counter[str] = Counter()
    roots = _roots(workspace)
    events = [
        _migrate_report(
            workspace,
            path,
            deployments,
            sidecars,
            roots,
            referenced,
            counters,
        )
        for path in reports
    ]
    events.extend(
        _unreferenced_sidecar_events(
            workspace, sidecars, referenced, deployments, counters
        )
    )
    events.extend(_writer_gap_events(workspace, writer_gaps, counters))
    source_watermarks = _migration_watermarks(
        workspace,
        reports,
        len(sidecars),
        len(writer_gaps),
        len(deployments),
    )
    return events, {
        "counts": {
            "canonical": len(reports),
            "exact": counters["exact"],
            "exact_deployment_match": counters["exact_deployment_match"],
            "temporal_only": counters["temporal_only"],
            "unknown": counters["unknown"],
            "gap": counters["gap"],
            "auxiliary": counters["auxiliary"],
            "orphan": counters["orphan"],
            "privacy_rejection": counters["privacy_rejection"],
        },
        "source_watermarks": source_watermarks,
    }
def _parse_fill(value: Any) -> float | None:
    text = str(value or "").strip().removesuffix("%")
    try:
        scalar = float(text)
    except ValueError:
        return None
    return scalar if 0.0 <= scalar <= 100.0 else None


def _materialize(events: list[dict[str, Any]]) -> dict[str, Any]:
    witnesses: dict[str, dict[str, Any]] = {}
    auxiliary_artifacts: dict[str, dict[str, Any]] = {}
    gaps: dict[str, dict[str, Any]] = {}
    orphans: dict[str, dict[str, Any]] = {}
    reconciliations: dict[str, dict[str, Any]] = {}
    migration_receipt: dict[str, Any] | None = None
    witness_event_count = 0
    for event in events:
        event_type = str(event.get("event_type") or "")
        witness_id = str(event.get("witness_id") or "")
        if event_type in {
            "temporal_lived_state_witness_recorded",
            "historical_lived_state_witness_migrated",
        }:
            witness_event_count += 1
            witnesses[witness_id] = event
        elif event_type == "lived_state_auxiliary_artifact_witness_recorded":
            auxiliary_artifacts[witness_id] = event
        elif "gap" in event_type:
            gaps[str(event.get("idempotency_key") or witness_id)] = event
        elif event_type == "lived_state_witness_orphan_detected":
            orphans[witness_id] = event
        elif event_type == "lived_state_review_context_reconciled":
            reconciliations[witness_id] = event
        elif event_type == "lived_state_witness_migration_recorded":
            migration_receipt = event
    alignment_counts = Counter(
        str(event.get("alignment", {}).get("outcome") or "unknown")
        for event in witnesses.values()
    )
    auxiliary_alignment_counts = Counter(
        str(event.get("alignment", {}).get("outcome") or "unknown")
        for event in auxiliary_artifacts.values()
    )
    checks = {
        "witness_ids_unique": len(witnesses) == witness_event_count,
        "all_authority_evidence_only": all(
            event.get("artifact_authority_state_v1", {}).get("state")
            == "evidence_only"
            and all(
                event.get(marker) is not True
                for marker in (
                    "live_eligible_now",
                    "auto_approved",
                    "grants_approval",
                    "edits_source_now",
                )
            )
            for event in events
        ),
        "no_closure_propagation": all(
            not any(
                event.get(key) is True
                for key in (
                    "felt_closed",
                    "closure_propagated",
                    "evidence_sufficient",
                    "evidence_sufficiency_propagated",
                    "authority_granted",
                    "authority_propagated",
                    "felt_resolution_propagated",
                )
            )
            for event in events
        ),
        "reconciliation_outcomes_valid": all(
            event.get("outcome") in RECONCILIATION_OUTCOMES
            for event in reconciliations.values()
        ),
    }
    return {
        "schema": "lived_state_witness_projection_v1",
        "schema_version": 1,
        "valid": all(checks.values()),
        "witness_count": len(witnesses),
        "auxiliary_artifact_count": len(auxiliary_artifacts),
        "gap_count": len(gaps),
        "orphan_count": len(orphans),
        "reconciliation_count": len(reconciliations),
        "alignment_counts": dict(sorted(alignment_counts.items())),
        "auxiliary_alignment_counts": dict(
            sorted(auxiliary_alignment_counts.items())
        ),
        "counter_audit": {
            "status": "consistent" if all(checks.values()) else "inconsistent",
            "checks": checks,
        },
        "witnesses": dict(sorted(witnesses.items())),
        "auxiliary_artifacts": dict(sorted(auxiliary_artifacts.items())),
        "gaps": dict(sorted(gaps.items())),
        "orphans": dict(sorted(orphans.items())),
        "reconciliations": dict(sorted(reconciliations.items())),
        "migration_receipt": migration_receipt,
        "artifact_authority_state_v1": authority_state(),
    }


def _render_report(status: dict[str, Any]) -> str:
    lines = [
        "# Temporal Lived-State Witness V1",
        "",
        "Evidence-only context for the conditions in which canonical introspections were authored.",
        "",
        f"- Valid: `{str(status['valid']).lower()}`",
        f"- Canonical witnesses: {status['witness_count']}",
        f"- Auxiliary artifact witnesses: {status['auxiliary_artifact_count']}",
        f"- Gaps: {status['gap_count']}",
        f"- True orphans: {status['orphan_count']}",
        f"- Reconciliations: {status['reconciliation_count']}",
        "- Exact deployment links require matching source, artifact, and process evidence.",
        "- Historical deployment proximity remains temporal association only.",
        "- Witness context never propagates closure, sufficiency, authority, or felt resolution.",
        "",
        "## Alignment",
        "",
    ]
    for outcome, count in sorted(status["alignment_counts"].items()):
        lines.append(f"- `{outcome}`: {count}")
    lines.append("")
    return "\n".join(lines)


def _context_index_payload(status: dict[str, Any]) -> str:
    gaps_by_witness: Counter[str] = Counter()
    first_gap_by_witness: dict[str, dict[str, Any]] = {}
    for event in status["gaps"].values():
        witness_id = str(event.get("witness_id") or "")
        if witness_id:
            gaps_by_witness[witness_id] += 1
            first_gap_by_witness.setdefault(witness_id, event)
    rows: list[str] = []
    witness_ids = set(status["witnesses"]) | set(first_gap_by_witness)
    for witness_id in sorted(witness_ids):
        event = status["witnesses"].get(witness_id)
        if not isinstance(event, dict):
            event = first_gap_by_witness[witness_id]
        reconciliation = status["reconciliations"].get(witness_id)
        alignment = event.get("alignment")
        if not isinstance(alignment, dict) and gaps_by_witness[witness_id]:
            alignment = {
                "outcome": "witness_gap",
                "exact_identity_match": False,
                "temporal_association_only": False,
                "direct_causation_claimed": False,
            }
        row = {
            "schema": "lived_state_context_index_v1",
            "schema_version": 1,
            "witness_id": witness_id,
            "introspection_id": event.get("introspection_id"),
            "alignment": alignment,
            "evidence_completeness": (
                event.get("evidence_completeness")
                or ("gap" if gaps_by_witness[witness_id] else None)
            ),
            "gap_count": gaps_by_witness[witness_id],
            "reconciliation_ref": (
                {
                    "outcome": reconciliation.get("outcome"),
                    "review_deployment_receipt_id": reconciliation.get(
                        "review_deployment_receipt_id"
                    ),
                    "idempotency_key_sha256": sha256_bytes(
                        str(reconciliation.get("idempotency_key") or "").encode()
                    ),
                }
                if isinstance(reconciliation, dict)
                else None
            ),
            "closure_propagated": False,
            "evidence_sufficiency_propagated": False,
            "authority_propagated": False,
            "felt_resolution_propagated": False,
            "raw_prose_included": False,
            "artifact_authority_state_v1": authority_state(),
        }
        rows.append(canonical_json(row))
    return "\n".join(rows) + ("\n" if rows else "")


def _write_outputs(workspace: Path, status: dict[str, Any], migration: dict[str, Any]) -> dict[str, str]:
    directory = state_dir(workspace)
    witness_rows = [
        canonical_json(event)
        for _, event in sorted(status["witnesses"].items())
    ]
    witness_payload = "\n".join(witness_rows) + ("\n" if witness_rows else "")
    auxiliary_rows = [
        canonical_json(event)
        for _, event in sorted(status["auxiliary_artifacts"].items())
    ]
    auxiliary_payload = "\n".join(auxiliary_rows) + (
        "\n" if auxiliary_rows else ""
    )
    gap_rows = [
        canonical_json(event) for _, event in sorted(status["gaps"].items())
    ]
    gap_payload = "\n".join(gap_rows) + ("\n" if gap_rows else "")
    report_payload = _render_report(status)
    context_index_payload = _context_index_payload(status)
    base_hashes = {
        "witnesses.jsonl": sha256_bytes(witness_payload.encode()),
        "auxiliary_artifacts.jsonl": sha256_bytes(auxiliary_payload.encode()),
        "gaps.jsonl": sha256_bytes(gap_payload.encode()),
        "context_index.jsonl": sha256_bytes(context_index_payload.encode()),
        "report.md": sha256_bytes(report_payload.encode()),
    }
    migration_receipt = {
        "schema": "lived_state_witness_migration_receipt_v1",
        "schema_version": 1,
        **migration,
        "projection_hashes": base_hashes,
        "artifact_authority_state_v1": authority_state(),
    }
    migration_payload = json.dumps(
        migration_receipt, indent=2, sort_keys=True, ensure_ascii=False
    ) + "\n"
    public_status = {
        key: value
        for key, value in status.items()
        if key
        not in {
            "witnesses",
            "auxiliary_artifacts",
            "gaps",
            "orphans",
            "reconciliations",
        }
    }
    public_status["projection_hashes"] = {
        **base_hashes,
        "migration_receipt.json": sha256_bytes(migration_payload.encode()),
    }
    status_payload = json.dumps(
        public_status, indent=2, sort_keys=True, ensure_ascii=False
    ) + "\n"
    payloads = {
        "status.json": status_payload,
        "witnesses.jsonl": witness_payload,
        "auxiliary_artifacts.jsonl": auxiliary_payload,
        "gaps.jsonl": gap_payload,
        "context_index.jsonl": context_index_payload,
        "report.md": report_payload,
        "migration_receipt.json": migration_payload,
    }
    for name, payload in payloads.items():
        _atomic_write(directory / name, payload)
    return {name: sha256_bytes(payload.encode()) for name, payload in payloads.items()}


def _source_hashes(workspace: Path, migration: dict[str, Any]) -> dict[str, str]:
    sidecars = sorted(sidecar_dir(workspace).glob("**/*.json"))
    sidecar_manifest = [
        [path.relative_to(workspace).as_posix(), sha256_file(path)] for path in sidecars
    ]
    model_receipts = (
        Path(__file__).resolve().parents[3]
        / "neural-triple-reservoir/workspace/model_qos_receipts.jsonl"
    )
    return {
        "canonical_reports": str(
            migration["source_watermarks"]["canonical_report_manifest_sha256"]
        ),
        "lived_state_sidecars": sha256_bytes(
            canonical_json(sidecar_manifest).encode()
        ),
        "environment_receipts": str(
            migration["source_watermarks"]["environment_receipts_sha256"]
        ),
        "model_receipts": (
            sha256_file(model_receipts) if model_receipts.is_file() else sha256_bytes(b"")
        ),
    }


def project(workspace: Path, *, write: bool) -> dict[str, Any]:
    candidates, migration = migration_events(workspace)
    migration_identity = sha256_bytes(
        canonical_json(
            {
                "counts": migration["counts"],
                "source_watermarks": migration["source_watermarks"],
            }
        ).encode()
    )
    migration_witness_id = f"lsw_{sha256_bytes(f'migration\0{migration_identity}'.encode())}"
    candidates.append(
        event_record(
            "lived_state_witness_migration_recorded",
            migration_witness_id,
            f"lived-state-migration:{migration_identity}",
            migration_receipt={
                "schema": "lived_state_witness_migration_event_receipt_v1",
                "schema_version": 1,
                **migration,
                "migration_identity_sha256": migration_identity,
                "raw_prose_included": False,
                "artifact_authority_state_v1": authority_state(),
            },
        )
    )
    store = store_for(workspace)
    if write:
        store.append_payloads(
            STREAM,
            candidates,
            actor="lived-state-witness-projector",
            source=ProvenanceSourceV1(
                "projection", "introspections/lived_state_witnesses"
            ),
            idempotency_keys=[str(event["idempotency_key"]) for event in candidates],
        )
        events, corrupt = store.payloads_for_stream(STREAM)
        if corrupt:
            raise RuntimeError("lived-state witness stream is corrupt")
    else:
        events = candidates
    status = _materialize(events)
    status["write"] = write
    status["migration_counters"] = migration["counts"]
    status["source_watermarks"] = migration["source_watermarks"]
    if write:
        hashes = _write_outputs(workspace, status, migration)
        store.write_checkpoint(
            "lived_state_witness_v1",
            PROJECTOR_VERSION,
            hashes,
            input_streams=("addressing", "signal_spine"),
            source_hashes=_source_hashes(workspace, migration),
            dependency_output_hashes={},
            command_sha256=sha256_bytes(b"lived_state_witness.py project --write"),
            config_sha256=sha256_bytes(b"lived_state_witness_v1_default"),
        )
        status["projection_hashes"] = hashes
    return status


def migrate(workspace: Path, *, write: bool) -> dict[str, Any]:
    status = project(workspace, write=write)
    return {
        "schema": "lived_state_witness_migration_result_v1",
        "schema_version": 1,
        "write": write,
        "valid": status["valid"],
        "migration_counters": status["migration_counters"],
        "source_watermarks": status["source_watermarks"],
        "projection_hashes": status.get("projection_hashes", {}),
        "artifact_authority_state_v1": authority_state(),
    }


def _review_outcome(
    witness_event: dict[str, Any], latest: dict[str, Any] | None, current_candidate: dict[str, Any] | None
) -> str:
    original = witness_event.get("alignment")
    original = original if isinstance(original, dict) else {}
    original_receipt = str(original.get("deployment_receipt_id") or "")
    original_witness = witness_event.get("witness")
    if not isinstance(original_witness, dict) or original_witness.get("schema") == "historical_lived_state_witness_v1":
        return (
            "temporal_association_only"
            if original_receipt
            else "historical_unrecoverable"
        )
    if latest is None:
        return "deployment_unknown"
    latest_ref = _deployment_ref(latest)
    if not latest_ref["exact_identity_available"]:
        return "deployment_unknown"
    if _exact_deployment_match(original_witness, latest):
        return "same_deployment"
    if original_receipt and original_receipt == latest_ref["deployment_receipt_id"]:
        return "same_deployment"
    candidate = original_witness.get("startup_build_candidate_v1")
    process = original_witness.get("observed_process_v1")
    candidate = candidate if isinstance(candidate, dict) else {}
    process = process if isinstance(process, dict) else {}
    if (
        candidate.get("source_identity_sha256")
        and candidate.get("source_identity_sha256")
        == latest_ref["source_identity_sha256"]
        and process.get("pid") != latest_ref["pid"]
    ):
        return "same_source_new_process"
    if (
        current_candidate
        and current_candidate.get("source_identity_sha256")
        and current_candidate.get("source_identity_sha256")
        != latest_ref["source_identity_sha256"]
    ):
        return "source_changed_not_deployed"
    return "deployed_changed"


def _current_build_candidate(workspace: Path) -> dict[str, Any] | None:
    manifest = _load_json(
        workspace / "deployment_manifests/spectral-bridge.json"
    )
    if not manifest:
        return None
    repository = manifest.get("repository")
    repository = repository if isinstance(repository, dict) else {}
    return {
        "manifest_sha256": sha256_file(
            workspace / "deployment_manifests/spectral-bridge.json"
        ),
        "source_identity_sha256": repository.get("source_identity_sha256"),
        "establishes_deployment": False,
    }


def reconcile(workspace: Path, *, write: bool) -> dict[str, Any]:
    store = store_for(workspace)
    events, corrupt = store.payloads_for_stream(STREAM)
    if corrupt:
        raise RuntimeError("lived-state witness stream is corrupt")
    state = _materialize(events)
    deployments = _successful_deployments(workspace)
    latest = deployments[-1] if deployments else None
    current_candidate = _current_build_candidate(workspace)
    current_candidate_identity = sha256_bytes(
        canonical_json(current_candidate or {}).encode()
    )
    now_ms = int(time.time() * 1_000)
    candidates: list[dict[str, Any]] = []
    counts: Counter[str] = Counter()
    for witness_id, witness_event in sorted(state["witnesses"].items()):
        outcome = _review_outcome(witness_event, latest, current_candidate)
        counts[outcome] += 1
        latest_id = str(latest.get("id") or "unknown") if latest else "unknown"
        original_witness = witness_event.get("witness")
        review_exact = bool(
            isinstance(original_witness, dict)
            and latest is not None
            and _exact_deployment_match(original_witness, latest)
        )
        original_alignment = witness_event.get("alignment")
        original_alignment = (
            original_alignment if isinstance(original_alignment, dict) else {}
        )
        candidates.append(
            event_record(
                "lived_state_review_context_reconciled",
                witness_id,
                (
                    f"lived-state-reconcile:{witness_id}:{latest_id}:"
                    f"{outcome}:{current_candidate_identity}"
                ),
                outcome=outcome,
                introspection_id=witness_event.get("introspection_id"),
                exact_identity_match=bool(
                    outcome == "same_deployment"
                    and (
                        review_exact
                        or original_alignment.get("exact_identity_match") is True
                    )
                ),
                reviewed_at_unix_ms=now_ms,
                review_deployment_receipt_id=(
                    str(latest.get("id") or "") if latest else None
                ),
                authorship_witness_unchanged=True,
                closure_propagated=False,
                evidence_sufficiency_propagated=False,
                authority_propagated=False,
                felt_resolution_propagated=False,
            )
        )
    if write:
        store.append_payloads(
            STREAM,
            candidates,
            actor="lived-state-witness-reconciler",
            source=ProvenanceSourceV1("reconciliation", "review_time_context"),
            idempotency_keys=[str(event["idempotency_key"]) for event in candidates],
        )
        project(workspace, write=True)
    return {
        "schema": "lived_state_witness_reconciliation_result_v1",
        "schema_version": 1,
        "write": write,
        "reconciled_count": len(candidates),
        "outcome_counts": dict(sorted(counts.items())),
        "latest_successful_deployment_receipt_id": (
            str(latest.get("id") or "") if latest else None
        ),
        "authorship_witnesses_mutated": False,
        "artifact_authority_state_v1": authority_state(),
    }


def verify(workspace: Path) -> dict[str, Any]:
    store = store_for(workspace)
    verification = store.verify_indexed_tail()
    status_path = state_dir(workspace) / "status.json"
    status = _load_json(status_path)
    output_errors: list[str] = []
    if status is None:
        output_errors.append("status_missing_or_invalid")
    else:
        hashes = status.get("projection_hashes")
        if isinstance(hashes, dict):
            for name, expected in hashes.items():
                path = state_dir(workspace) / str(name)
                if not path.is_file() or sha256_file(path) != expected:
                    output_errors.append(f"output_hash_mismatch:{name}")
    events, corrupt = store.payloads_for_stream(STREAM)
    materialized = _materialize(events)
    return {
        "schema": "lived_state_witness_verification_v1",
        "schema_version": 1,
        "valid": bool(
            verification.valid
            and not corrupt
            and not output_errors
            and materialized["valid"]
        ),
        "store_valid": verification.valid,
        "stream_event_count": len(events),
        "corrupt_stream_rows": corrupt,
        "output_errors": output_errors,
        "counter_audit": materialized["counter_audit"],
        "artifact_authority_state_v1": authority_state(),
    }


def report(workspace: Path) -> dict[str, Any]:
    status = _load_json(state_dir(workspace) / "status.json")
    if status is None:
        raise FileNotFoundError("lived-state witness projection status is unavailable")
    return status


def show(workspace: Path, witness_id: str) -> dict[str, Any]:
    events, corrupt = store_for(workspace).payloads_for_stream(STREAM)
    if corrupt:
        raise RuntimeError("lived-state witness stream is corrupt")
    state = _materialize(events)
    witness = state["witnesses"].get(witness_id)
    auxiliary = False
    if witness is None:
        witness = state["auxiliary_artifacts"].get(witness_id)
        auxiliary = witness is not None
    if witness is None:
        raise KeyError(f"unknown witness: {witness_id}")
    return {
        "schema": "lived_state_witness_show_v1",
        "schema_version": 1,
        "witness": witness,
        "reconciliation": (
            None if auxiliary else state["reconciliations"].get(witness_id)
        ),
        "canonical_queue_member": not auxiliary,
        "artifact_authority_state_v1": authority_state(),
    }


def diff_witnesses(workspace: Path, left: str, right: str) -> dict[str, Any]:
    left_value = show(workspace, left)["witness"]
    right_value = show(workspace, right)["witness"]
    fields = (
        "alignment",
        "evidence_completeness",
        "report_ref",
    )
    changes = {
        field: {"left": left_value.get(field), "right": right_value.get(field)}
        for field in fields
        if left_value.get(field) != right_value.get(field)
    }
    return {
        "schema": "lived_state_witness_diff_v1",
        "schema_version": 1,
        "left_witness_id": left,
        "right_witness_id": right,
        "changes": changes,
        "raw_prose_compared": False,
        "artifact_authority_state_v1": authority_state(),
    }
