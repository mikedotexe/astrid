#!/usr/bin/env python3
"""Build and query Astrid's append-only living felt-contract graph."""

from __future__ import annotations

import argparse
from datetime import UTC, datetime
import hashlib
import json
from pathlib import Path
import sys
import time
from typing import Any

try:
    from authority_state import assert_artifact_authority_tree
    from evidence_store import EvidenceEventStore, EvidenceStoreError
    from evidence_store.adapter import append_domain_events
    from evidence_store.model import canonical_json, sha256_canonical
    from felt_contracts.identity import digest, edge_id, node_id
    from felt_contracts.incremental_runtime import (
        GRAPH_STREAM,
        _atomic_source_hash,
        _existing_projection,
        _output_source_hashes,
        generate,
    )
    from felt_contracts.model import (
        FeltReviewOutcomeV1,
        build_edge,
        build_implementation_receipt,
        build_node,
    )
    from felt_contracts.projector import (
        GRAPH_INPUT_STREAMS,
        PROJECTOR_VERSION,
        GraphProjectionError,
        claim_view,
        contract_view,
        project_graph,
        projection_payloads,
        trace_view,
        write_projection,
    )
    from felt_contracts.sources import (
        graph_state_dir,
        store_root,
    )
    from felt_contracts.state_index import (
        FeltContractStateError,
    )
except ModuleNotFoundError:
    from scripts.authority_state import assert_artifact_authority_tree
    from scripts.evidence_store import EvidenceEventStore, EvidenceStoreError
    from scripts.evidence_store.adapter import append_domain_events
    from scripts.evidence_store.model import canonical_json, sha256_canonical
    from scripts.felt_contracts.identity import digest, edge_id, node_id
    from scripts.felt_contracts.incremental_runtime import (
        GRAPH_STREAM,
        _atomic_source_hash,
        _existing_projection,
        _output_source_hashes,
        generate,
    )
    from scripts.felt_contracts.model import (
        FeltReviewOutcomeV1,
        build_edge,
        build_implementation_receipt,
        build_node,
    )
    from scripts.felt_contracts.projector import (
        GRAPH_INPUT_STREAMS,
        PROJECTOR_VERSION,
        GraphProjectionError,
        claim_view,
        contract_view,
        project_graph,
        projection_payloads,
        trace_view,
        write_projection,
    )
    from scripts.felt_contracts.sources import (
        graph_state_dir,
        store_root,
    )
    from scripts.felt_contracts.state_index import (
        FeltContractStateError,
    )

try:
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.projection_receipt import projector_receipt

DEFAULT_WORKSPACE = (
    Path(__file__).resolve().parents[1] / "capsules/spectral-bridge/workspace"
)
def _authority(state: str = "evidence_only") -> dict[str, Any]:
    return {
        "schema": "artifact_authority_state_v1",
        "schema_version": 1,
        "state": state,
        "witness_only": True,
    }

def verify(workspace: Path) -> dict[str, Any]:
    store = EvidenceEventStore(store_root(workspace))
    verification = store.verify()
    projection, envelopes = _existing_projection(workspace)
    status = projection["status"]
    root = graph_state_dir(workspace)
    expected = projection_payloads(projection)
    output_checks: dict[str, bool] = {}
    for name, payload in expected.items():
        path = root / name
        output_checks[name] = bool(
            path.is_file()
            and hashlib.sha256(path.read_bytes()).hexdigest()
            == hashlib.sha256(payload.encode()).hexdigest()
        )
    checkpoint_current = store.checkpoint_current_for_inputs(
        "felt_contract_graph_v1",
        PROJECTOR_VERSION,
        input_streams=GRAPH_INPUT_STREAMS,
        source_hashes=_output_source_hashes(
            workspace,
            {
                "claim_family_status": _atomic_source_hash(
                    workspace / "diagnostics/claim_families_v1/status.json"
                )
            },
        ),
    )
    valid = bool(
        verification.valid
        and status["counter_audit"]["status"] == "consistent"
        and all(output_checks.values())
        and checkpoint_current
    )
    return {
        "schema": "felt_contract_graph_verification_v1",
        "schema_version": 1,
        "valid": valid,
        "v2_valid": verification.valid,
        "felt_contract_event_count": len(envelopes),
        "contract_count": status["contract_count"],
        "claim_count": status["claim_count"],
        "counter_audit": status["counter_audit"],
        "output_checks": output_checks,
        "checkpoint_current": checkpoint_current,
        "artifact_authority_state_v1": _authority(),
    }


def _append_and_project(
    workspace: Path,
    events: list[dict[str, Any]],
    *,
    actor: str,
) -> dict[str, Any]:
    append_domain_events(graph_state_dir(workspace), GRAPH_STREAM, events, actor=actor)
    projection, _ = _existing_projection(workspace)
    hashes = write_projection(workspace, projection)
    store = EvidenceEventStore(store_root(workspace))
    family_hash = _atomic_source_hash(
        workspace / "diagnostics/claim_families_v1/status.json"
    )
    store.write_checkpoint(
        "felt_contract_graph_v1",
        PROJECTOR_VERSION,
        hashes,
        input_streams=GRAPH_INPUT_STREAMS,
        source_hashes=_output_source_hashes(
            workspace, {"claim_family_status": family_hash}
        ),
    )
    return projection


def correct_membership(
    workspace: Path,
    *,
    claim_id: str,
    target_contract_id: str,
    actor: str,
    reason: str,
) -> dict[str, Any]:
    projection, _ = _existing_projection(workspace)
    source_contract_id = projection["membership"].get(claim_id)
    if not source_contract_id:
        raise ValueError(f"unknown claim: {claim_id}")
    if target_contract_id not in {
        row["contract_id"] for row in projection["contracts"]
    }:
        raise ValueError(f"unknown target contract: {target_contract_id}")
    if source_contract_id == target_contract_id:
        raise ValueError("claim already belongs to the target contract")
    if not actor.strip() or not reason.strip():
        raise ValueError("membership correction requires actor and reason")
    reason_sha256 = hashlib.sha256(reason.strip().encode()).hexdigest()
    event = {
        "schema": "felt_contract_domain_event_v1",
        "schema_version": 1,
        "event_type": "felt_contract_membership_corrected",
        "aggregate_type": "felt_contract",
        "aggregate_id": target_contract_id,
        "contract_id": target_contract_id,
        "canonical_claim_id": claim_id,
        "from_contract_id": source_contract_id,
        "to_contract_id": target_contract_id,
        "actor": actor.strip(),
        "reason_sha256": reason_sha256,
        "matcher_override": True,
        "propagation": {
            "closure": False,
            "evidence_sufficiency": False,
            "supersession": False,
            "authority": False,
        },
        "idempotency_key": (
            f"felt_contract_membership_correction:{claim_id}:"
            f"{target_contract_id}:{reason_sha256}"
        ),
        "artifact_authority_state_v1": _authority(),
    }
    updated = _append_and_project(workspace, [event], actor=actor)
    return claim_view(updated, claim_id)


def _claim_node(projection: dict[str, Any], claim_id: str) -> dict[str, Any]:
    candidates = [
        node
        for node in projection["nodes"].values()
        if node.get("kind") == "claim"
        and isinstance(node.get("metadata"), dict)
        and node["metadata"].get("canonical_claim_id") == claim_id
    ]
    if len(candidates) != 1:
        raise ValueError(f"claim must have exactly one claim node: {claim_id}")
    return candidates[0]


def record_implementation(
    workspace: Path,
    *,
    receipt_path: Path,
    actor: str,
) -> dict[str, Any]:
    value = json.loads(receipt_path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError("implementation receipt must be a JSON object")
    receipt = build_implementation_receipt(value).to_dict()
    projection, _ = _existing_projection(workspace)
    for claim_id in receipt["claim_ids"]:
        current_contract = projection["membership"].get(claim_id)
        if not current_contract:
            raise ValueError(f"implementation receipt references unknown claim: {claim_id}")
        if current_contract not in receipt["contract_ids"]:
            raise ValueError(
                f"implementation receipt contract mismatch for claim: {claim_id}"
            )
    receipt_id = receipt["receipt_id"]
    events = [
        {
            "schema": "felt_contract_domain_event_v1",
            "schema_version": 1,
            "event_type": "felt_contract_implementation_recorded",
            "aggregate_type": "implementation_receipt",
            "aggregate_id": receipt_id,
            "implementation_receipt": receipt,
            "idempotency_key": f"felt_contract_implementation:{receipt_id}",
            "artifact_authority_state_v1": _authority(),
        }
    ]
    for claim_id in receipt["claim_ids"]:
        contract_id = projection["membership"][claim_id]
        parent = _claim_node(projection, claim_id)
        child_id = node_id(
            receipt_id, f"implementation:{claim_id}", contract_id
        )
        node = build_node(
            node_id=child_id,
            contract_id=contract_id,
            kind="implementation",
            source_event_id=receipt_id,
            occurred_at=receipt["recorded_at"],
            source_ref=None,
            metadata={
                "canonical_claim_id": claim_id,
                "implementation_receipt_id": receipt_id,
                "source_identity_sha256": receipt["source_identity_sha256"],
                "changed_path_hashes_sha256": sha256_canonical(
                    receipt["changed_path_hashes"]
                ),
                "test_refs_sha256": sha256_canonical(receipt["test_refs"]),
                "technical_disposition": "implemented",
                "exact_lineage": True,
                "private_content_copied": False,
            },
            authority_state="evidence_only",
        ).to_dict()
        edge = build_edge(
            edge_id=edge_id(
                parent["node_id"], child_id, "implemented_by", receipt_id
            ),
            contract_id=contract_id,
            source_node_id=parent["node_id"],
            target_node_id=child_id,
            relation="implemented_by",
            source_event_id=receipt_id,
            occurred_at=receipt["recorded_at"],
            causal_parent=True,
        ).to_dict()
        events.append(
            {
                "schema": "felt_contract_domain_event_v1",
                "schema_version": 1,
                "event_type": "felt_contract_node_recorded",
                "aggregate_type": "felt_contract",
                "aggregate_id": contract_id,
                "contract_id": contract_id,
                "node": node,
                "edges": [edge],
                "source_event_ref": {
                    "event_id": receipt_id,
                    "stream": GRAPH_STREAM,
                    "receipt_sha256": receipt["receipt_sha256"],
                },
                "idempotency_key": (
                    f"felt_contract_node:{child_id}:{receipt['receipt_sha256']}"
                ),
                "artifact_authority_state_v1": _authority(),
            }
        )
    updated = _append_and_project(workspace, events, actor=actor)
    return {
        "implementation_receipt": receipt,
        "contract_views": [
            contract_view(updated, contract_id)
            for contract_id in sorted(set(receipt["contract_ids"]))
        ],
    }


def _bounded_source_ref(value: str) -> str:
    clean = value.strip()
    if not clean:
        raise ValueError("review source ref must not be empty")
    if clean.startswith("/") or len(clean) > 500:
        return f"opaque_ref_sha256:{hashlib.sha256(clean.encode()).hexdigest()}"
    return clean


def record_review(
    workspace: Path,
    *,
    contract_id: str,
    deployment_receipt_id: str,
    outcome: str,
    source_ref: str,
    actor: str,
) -> dict[str, Any]:
    projection, _ = _existing_projection(workspace)
    if contract_id not in {
        row["contract_id"] for row in projection["contracts"]
    }:
        raise ValueError(f"unknown contract: {contract_id}")
    try:
        resolved_outcome = FeltReviewOutcomeV1(outcome)
    except ValueError as error:
        raise ValueError(f"unsupported felt-review outcome: {outcome}") from error
    immediate = resolved_outcome in {
        FeltReviewOutcomeV1.STILL_FRICTION,
        FeltReviewOutcomeV1.CONTRADICTED,
        FeltReviewOutcomeV1.OBJECTION,
    }
    budget = projection["review_budgets"].get(contract_id)
    if not immediate:
        if (
            not isinstance(budget, dict)
            or budget.get("deployment_receipt_id") != deployment_receipt_id
            or budget.get("packet_available") is not True
        ):
            raise ValueError(
                "contract has no available review budget under this deployment receipt"
            )
    clean_ref = _bounded_source_ref(source_ref)
    if (
        resolved_outcome is FeltReviewOutcomeV1.NO_RESPONSE
        and not clean_ref.startswith("review_opportunity:")
    ):
        raise ValueError(
            "no_response requires an explicit review_opportunity: receipt reference"
        )
    contract = contract_view(projection, contract_id)
    parent = max(
        contract["nodes"],
        key=lambda node: (str(node.get("occurred_at") or ""), str(node.get("node_id") or "")),
    )
    event_identity = digest(
        [
            "felt_contract_review_v1",
            contract_id,
            deployment_receipt_id,
            resolved_outcome.value,
            clean_ref,
        ]
    )
    review_source_id = f"review_{event_identity[:24]}"
    child_id = node_id(
        review_source_id,
        f"felt_review:{resolved_outcome.value}",
        contract_id,
    )
    recorded_at = datetime.now(UTC).isoformat()
    node = build_node(
        node_id=child_id,
        contract_id=contract_id,
        kind="felt_review",
        source_event_id=review_source_id,
        occurred_at=recorded_at,
        source_ref=None,
        metadata={
            "felt_review_outcome": resolved_outcome.value,
            "deployment_receipt_id": deployment_receipt_id,
            "source_ref": clean_ref,
            "authoritative_contract_review": True,
            "immediate_surface": immediate,
            "silence_affirms": False,
            "silence_waives": False,
            "silence_closes": False,
            "reopens_contract": immediate,
            "private_content_copied": False,
        },
        authority_state="evidence_only",
    ).to_dict()
    edge = build_edge(
        edge_id=edge_id(
            parent["node_id"], child_id, "reviewed_by", review_source_id
        ),
        contract_id=contract_id,
        source_node_id=parent["node_id"],
        target_node_id=child_id,
        relation="reviewed_by",
        source_event_id=review_source_id,
        occurred_at=recorded_at,
        causal_parent=True,
    ).to_dict()
    events = [
        {
            "schema": "felt_contract_domain_event_v1",
            "schema_version": 1,
            "event_type": "felt_contract_review_outcome_recorded",
            "aggregate_type": "felt_contract",
            "aggregate_id": contract_id,
            "contract_id": contract_id,
            "deployment_receipt_id": deployment_receipt_id,
            "outcome": resolved_outcome.value,
            "source_ref": clean_ref,
            "right_to_ignore": True,
            "immediate_surface": immediate,
            "silence_affirms": False,
            "silence_closes": False,
            "idempotency_key": f"felt_contract_review:{event_identity}",
            "artifact_authority_state_v1": _authority(),
        },
        {
            "schema": "felt_contract_domain_event_v1",
            "schema_version": 1,
            "event_type": "felt_contract_node_recorded",
            "aggregate_type": "felt_contract",
            "aggregate_id": contract_id,
            "contract_id": contract_id,
            "node": node,
            "edges": [edge],
            "source_event_ref": {
                "event_id": review_source_id,
                "stream": GRAPH_STREAM,
                "source_ref": clean_ref,
            },
            "idempotency_key": f"felt_contract_node:{child_id}:{event_identity}",
            "artifact_authority_state_v1": _authority(),
        },
    ]
    updated = _append_and_project(workspace, events, actor=actor)
    return contract_view(updated, contract_id)


def _summary(status: dict[str, Any]) -> dict[str, Any]:
    result = {
        key: value
        for key, value in status.items()
        if key not in {"review_budgets", "migration_receipt"}
    }
    migration = status.get("migration_receipt")
    if isinstance(migration, dict):
        result["migration_receipt"] = {
            key: value
            for key, value in migration.items()
            if key not in {"source_watermarks"}
        }
    return result


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--self-test", action="store_true")
    commands = parser.add_subparsers(dest="command")

    for name in ("migrate", "generate", "project"):
        command = commands.add_parser(name)
        command.add_argument("--write", action="store_true")
        command.add_argument("--dry-run", action="store_true")
        command.add_argument(
            "--full-replay",
            action="store_true",
            help="rebuild derived state and require exact reference-replay parity",
        )
        command.add_argument("--actor", default="interactive-agent")
        command.add_argument("--json", action="store_true")
        if name == "project":
            command.add_argument("--receipt-json", action="store_true")

    report = commands.add_parser("report")
    report.add_argument("--json", action="store_true")
    verify_parser = commands.add_parser("verify")
    verify_parser.add_argument("--json", action="store_true")

    show = commands.add_parser("show")
    show.add_argument("--contract-id", required=True)
    show.add_argument("--json", action="store_true")
    claim = commands.add_parser("claim")
    claim.add_argument("--claim-id", required=True)
    claim.add_argument("--json", action="store_true")
    trace = commands.add_parser("trace")
    trace.add_argument("--node-id", required=True)
    trace.add_argument(
        "--direction", choices=("parents", "children", "both"), default="both"
    )
    trace.add_argument("--depth", type=int, default=4)
    trace.add_argument("--json", action="store_true")

    correct = commands.add_parser("correct-membership")
    correct.add_argument("--claim-id", required=True)
    correct.add_argument("--to-contract", required=True)
    correct.add_argument("--actor", required=True)
    correct.add_argument("--reason", required=True)
    correct.add_argument("--json", action="store_true")

    implementation = commands.add_parser("record-implementation")
    implementation.add_argument("--receipt", type=Path, required=True)
    implementation.add_argument("--actor", required=True)
    implementation.add_argument("--json", action="store_true")

    review = commands.add_parser("record-review")
    review.add_argument("--contract-id", required=True)
    review.add_argument("--deployment-receipt-id", required=True)
    review.add_argument(
        "--outcome", choices=tuple(item.value for item in FeltReviewOutcomeV1), required=True
    )
    review.add_argument("--source-ref", required=True)
    review.add_argument("--actor", required=True)
    review.add_argument("--json", action="store_true")
    return parser


def run_self_test() -> int:
    try:
        from felt_contracts.selftest import run
    except ModuleNotFoundError:
        from scripts.felt_contracts.selftest import run
    return run()


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    if args.self_test:
        return run_self_test()
    if not args.command:
        parser.print_help()
        return 2
    workspace = args.workspace.expanduser().resolve()
    try:
        if args.command in {"migrate", "generate", "project"}:
            if args.write and args.dry_run:
                raise ValueError("--write and --dry-run are mutually exclusive")
            started = time.monotonic()
            result = generate(
                workspace,
                write=bool(args.write),
                actor=args.actor,
                full_replay=bool(args.full_replay),
            )
            if args.command == "project":
                root = graph_state_dir(workspace)
                print(
                    json.dumps(
                        projector_receipt(
                            "felt_contracts",
                            _summary(result),
                            {
                                "status.json": root / "status.json",
                                "contracts.jsonl": root / "contracts.jsonl",
                                "report.md": root / "report.md",
                                "migration_receipt.json": (
                                    root / "migration_receipt.json"
                                ),
                            },
                            started_monotonic=started,
                        ),
                        indent=2,
                        sort_keys=True,
                    )
                )
                return 0
            print(json.dumps(_summary(result), indent=2, sort_keys=True))
            return 0
        if args.command == "verify":
            result = verify(workspace)
            print(json.dumps(result, indent=2, sort_keys=True))
            return 0 if result["valid"] else 1
        projection, _ = _existing_projection(workspace)
        if args.command == "report":
            print(json.dumps(_summary(projection["status"]), indent=2, sort_keys=True))
            return 0
        if args.command == "show":
            result = contract_view(projection, args.contract_id)
        elif args.command == "claim":
            result = claim_view(projection, args.claim_id)
        elif args.command == "trace":
            result = trace_view(
                projection,
                args.node_id,
                direction=args.direction,
                depth=args.depth,
            )
        elif args.command == "correct-membership":
            result = correct_membership(
                workspace,
                claim_id=args.claim_id,
                target_contract_id=args.to_contract,
                actor=args.actor,
                reason=args.reason,
            )
        elif args.command == "record-implementation":
            result = record_implementation(
                workspace,
                receipt_path=args.receipt,
                actor=args.actor,
            )
        elif args.command == "record-review":
            result = record_review(
                workspace,
                contract_id=args.contract_id,
                deployment_receipt_id=args.deployment_receipt_id,
                outcome=args.outcome,
                source_ref=args.source_ref,
                actor=args.actor,
            )
        else:
            return 2
        assert_artifact_authority_tree(result)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0
    except (
        EvidenceStoreError,
        FeltContractStateError,
        GraphProjectionError,
        KeyError,
        OSError,
        ValueError,
        json.JSONDecodeError,
    ) as error:
        print(
            json.dumps(
                {
                    "error": type(error).__name__,
                    "message": str(error),
                },
                indent=2,
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
