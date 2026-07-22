"""Deterministic materialization and atomic witness projection views."""

from __future__ import annotations

from collections import Counter
import json
import os
from pathlib import Path
import tempfile
from typing import Any

try:
    from evidence_store.model import canonical_json
except ModuleNotFoundError:
    from scripts.evidence_store.model import canonical_json

from .concordance import (
    validate_concordance_cluster,
    validate_concordance_preflight,
)
from .model import RECONCILIATION_OUTCOMES, authority_state, sha256_bytes
from .temporal_clusters import validate_temporal_cluster


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/lived_state_witness_v1"


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


def _materialize(events: list[dict[str, Any]]) -> dict[str, Any]:
    witnesses: dict[str, dict[str, Any]] = {}
    auxiliary_artifacts: dict[str, dict[str, Any]] = {}
    gaps: dict[str, dict[str, Any]] = {}
    resolved_gaps: dict[str, dict[str, Any]] = {}
    orphans: dict[str, dict[str, Any]] = {}
    reconciliations: dict[str, dict[str, Any]] = {}
    temporal_clusters: dict[str, dict[str, Any]] = {}
    temporal_cluster_validation_errors: dict[str, list[str]] = {}
    concordance_clusters: dict[str, dict[str, Any]] = {}
    concordance_validation_errors: dict[str, list[str]] = {}
    concordance_preflight: dict[str, Any] | None = None
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
        elif event_type in {
            "lived_state_witness_gap_detected",
            "lived_state_writer_gap_recorded",
        }:
            gaps[str(event.get("idempotency_key") or witness_id)] = event
        elif event_type == "lived_state_witness_gap_resolved":
            resolved_key = str(
                event.get("resolved_gap_idempotency_key") or ""
            )
            if resolved_key:
                gaps.pop(resolved_key, None)
                resolved_gaps[resolved_key] = event
        elif event_type == "lived_state_witness_orphan_detected":
            orphans[witness_id] = event
        elif event_type == "lived_state_review_context_reconciled":
            reconciliations[witness_id] = event
        elif event_type == "lived_state_temporal_cluster_observed":
            cluster_id = str(event.get("cluster_id") or "")
            errors = validate_temporal_cluster(event.get("cluster"))
            if errors:
                temporal_cluster_validation_errors[cluster_id] = errors
            temporal_clusters[cluster_id] = event
        elif event_type == "lived_state_concordance_cluster_observed":
            cluster_id = str(event.get("cluster_id") or "")
            errors = validate_concordance_cluster(event.get("concordance"))
            if errors:
                concordance_validation_errors[cluster_id] = errors
            concordance_clusters[cluster_id] = event
        elif event_type == "lived_state_concordance_preflight_recorded":
            errors = validate_concordance_preflight(event.get("preflight"))
            if errors:
                concordance_validation_errors["preflight"] = errors
            concordance_preflight = event
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
        "temporal_clusters_valid": not temporal_cluster_validation_errors,
        "temporal_clusters_non_causal": all(
            event.get("cluster", {}).get("causation_established") is False
            and event.get("cluster", {}).get("direct_causation_claimed")
            is False
            for event in temporal_clusters.values()
        ),
        "concordance_valid": not concordance_validation_errors,
        "concordance_cluster_pairing_valid": all(
            isinstance(temporal_clusters.get(cluster_id), dict)
            and event.get("concordance", {}).get(
                "temporal_cluster_membership_sha256"
            )
            == temporal_clusters.get(cluster_id, {})
            .get("cluster", {})
            .get("membership_sha256")
            for cluster_id, event in concordance_clusters.items()
        ),
        "concordance_non_causal": all(
            event.get("concordance", {}).get("mechanism_established") is False
            and event.get("concordance", {}).get("causation_established") is False
            for event in concordance_clusters.values()
        )
        and (
            concordance_preflight is None
            or (
                concordance_preflight.get("preflight", {}).get(
                    "mechanism_established"
                )
                is False
                and concordance_preflight.get("preflight", {}).get(
                    "causation_established"
                )
                is False
            )
        ),
    }
    return {
        "schema": "lived_state_witness_projection_v1",
        "schema_version": 1,
        "valid": all(checks.values()),
        "witness_count": len(witnesses),
        "auxiliary_artifact_count": len(auxiliary_artifacts),
        "gap_count": len(gaps),
        "resolved_gap_count": len(resolved_gaps),
        "orphan_count": len(orphans),
        "reconciliation_count": len(reconciliations),
        "temporal_cluster_count": len(temporal_clusters),
        "high_density_temporal_cluster_count": sum(
            event.get("cluster", {}).get("density_class") == "high_density"
            for event in temporal_clusters.values()
        ),
        "clustered_temporal_association_count": sum(
            int(event.get("cluster", {}).get("association_count") or 0)
            for event in temporal_clusters.values()
        ),
        "concordance_cluster_count": len(concordance_clusters),
        "concordance_status": (
            concordance_preflight.get("preflight", {}).get("status")
            if concordance_preflight
            else "unavailable"
        ),
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
        "resolved_gaps": dict(sorted(resolved_gaps.items())),
        "orphans": dict(sorted(orphans.items())),
        "reconciliations": dict(sorted(reconciliations.items())),
        "temporal_clusters": dict(sorted(temporal_clusters.items())),
        "temporal_cluster_validation_errors": dict(
            sorted(temporal_cluster_validation_errors.items())
        ),
        "concordance_clusters": dict(sorted(concordance_clusters.items())),
        "concordance_preflight": concordance_preflight,
        "concordance_validation_errors": dict(
            sorted(concordance_validation_errors.items())
        ),
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
        f"- Resolved gap history: {status['resolved_gap_count']}",
        f"- True orphans: {status['orphan_count']}",
        f"- Reconciliations: {status['reconciliation_count']}",
        f"- Temporal density clusters: {status['temporal_cluster_count']}",
        (
            "- High-density temporal clusters: "
            f"{status['high_density_temporal_cluster_count']}"
        ),
        (
            "- Clustered temporal associations: "
            f"{status['clustered_temporal_association_count']}"
        ),
        f"- Concordance clusters: {status['concordance_cluster_count']}",
        f"- Concordance preflight: `{status['concordance_status']}`",
        "- Exact deployment links require matching source, artifact, and process evidence.",
        "- Historical deployment proximity remains temporal association only.",
        (
            "- Density weights surface repeated timing for review; they are "
            "not causal strength or felt weight."
        ),
        (
            "- Pressure and shadow comparisons require exact fresh scalar "
            "coverage; insufficient coverage produces no correlation or proxy."
        ),
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
    clusters_by_witness: dict[str, list[dict[str, Any]]] = {}
    concordance_by_cluster = status["concordance_clusters"]
    for event in status["temporal_clusters"].values():
        cluster = event.get("cluster")
        if not isinstance(cluster, dict):
            continue
        cluster_ref = {
            "cluster_id": cluster.get("cluster_id"),
            "density_class": cluster.get("density_class"),
            "temporal_density_weight": cluster.get(
                "temporal_density_weight"
            ),
            "association_count": cluster.get("association_count"),
            "window_start_unix_ms": cluster.get("window_start_unix_ms"),
            "window_end_exclusive_unix_ms": cluster.get(
                "window_end_exclusive_unix_ms"
            ),
            "causation_established": False,
            "direct_causation_claimed": False,
            "concordance": (
                concordance_by_cluster.get(str(cluster.get("cluster_id") or ""), {})
                .get("concordance")
            ),
        }
        for member in cluster.get("member_refs", []):
            if isinstance(member, dict) and isinstance(
                member.get("witness_id"), str
            ):
                clusters_by_witness.setdefault(
                    member["witness_id"], []
                ).append(cluster_ref)
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
            "temporal_cluster_refs": sorted(
                clusters_by_witness.get(witness_id, []),
                key=lambda ref: str(ref.get("cluster_id") or ""),
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
    temporal_cluster_rows = [
        canonical_json(event)
        for _, event in sorted(status["temporal_clusters"].items())
    ]
    temporal_cluster_payload = "\n".join(temporal_cluster_rows) + (
        "\n" if temporal_cluster_rows else ""
    )
    concordance_rows = [
        canonical_json(event)
        for _, event in sorted(status["concordance_clusters"].items())
    ]
    concordance_payload = "\n".join(concordance_rows) + (
        "\n" if concordance_rows else ""
    )
    concordance_preflight_payload = json.dumps(
        status["concordance_preflight"] or {},
        indent=2,
        sort_keys=True,
        ensure_ascii=False,
    ) + "\n"
    report_payload = _render_report(status)
    context_index_payload = _context_index_payload(status)
    base_hashes = {
        "witnesses.jsonl": sha256_bytes(witness_payload.encode()),
        "auxiliary_artifacts.jsonl": sha256_bytes(auxiliary_payload.encode()),
        "gaps.jsonl": sha256_bytes(gap_payload.encode()),
        "temporal_clusters.jsonl": sha256_bytes(
            temporal_cluster_payload.encode()
        ),
        "concordance_clusters.jsonl": sha256_bytes(
            concordance_payload.encode()
        ),
        "concordance_preflight.json": sha256_bytes(
            concordance_preflight_payload.encode()
        ),
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
            "resolved_gaps",
            "orphans",
            "reconciliations",
            "temporal_clusters",
            "temporal_cluster_validation_errors",
            "concordance_clusters",
            "concordance_preflight",
            "concordance_validation_errors",
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
        "temporal_clusters.jsonl": temporal_cluster_payload,
        "concordance_clusters.jsonl": concordance_payload,
        "concordance_preflight.json": concordance_preflight_payload,
        "context_index.jsonl": context_index_payload,
        "report.md": report_payload,
        "migration_receipt.json": migration_payload,
    }
    for name, payload in payloads.items():
        _atomic_write(directory / name, payload)
    return {name: sha256_bytes(payload.encode()) for name, payload in payloads.items()}
