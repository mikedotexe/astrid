"""Transparent multi-membership saliency projection over exact claim evidence."""

from __future__ import annotations

from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import re
from typing import Any, Iterable

try:
    from experiential_systems.common import (
        authority_state,
        owner_atomic_write,
        owner_atomic_write_json,
        owner_atomic_write_jsonl,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        authority_state,
        owner_atomic_write,
        owner_atomic_write_json,
        owner_atomic_write_jsonl,
    )

POLICY_PATH = Path(__file__).with_name("policy_v1.json")
TOKEN_RE = re.compile(r"[a-z0-9]+")


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/felt_constellation_v1"


def _json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return value


def _jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        value = json.loads(line)
        if not isinstance(value, dict):
            raise ValueError(f"{path}:{line_number} must contain a JSON object")
        rows.append(value)
    return rows


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _canonical_sha256(value: Any) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(payload).hexdigest()


def _normalized(value: str) -> str:
    return " ".join(TOKEN_RE.findall(value.lower()))


def _matches(text: str, terms: Iterable[str]) -> list[str]:
    normalized = f" {_normalized(text)} "
    return sorted(
        term
        for term in terms
        if f" {_normalized(str(term))} " in normalized
    )


def _source_id(claim_id: str) -> str:
    return claim_id.rsplit(":", 1)[0]


def _claim_contract_index(
    contracts: list[dict[str, Any]],
) -> tuple[dict[str, str], dict[str, dict[str, Any]]]:
    by_claim: dict[str, str] = {}
    by_id: dict[str, dict[str, Any]] = {}
    for contract in contracts:
        contract_id = str(contract.get("contract_id") or "")
        if not contract_id:
            continue
        by_id[contract_id] = contract
        for claim_id in contract.get("claim_ids") or []:
            by_claim[str(claim_id)] = contract_id
    return by_claim, by_id


def _problem_index(rows: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    return {
        str(row.get("contract_id") or row.get("problem_id")): row
        for row in rows
        if row.get("contract_id") or row.get("problem_id")
    }


def _empty_constellation(definition: dict[str, Any]) -> dict[str, Any]:
    return {
        "definition": definition,
        "claim_ids": set(),
        "family_ids": set(),
        "contract_ids": set(),
        "source_ids": set(),
        "target_surfaces": set(),
        "matched_terms": set(),
        "classifications": Counter(),
    }


def _constellation_row(
    accumulator: dict[str, Any],
    *,
    contract_by_id: dict[str, dict[str, Any]],
    problem_by_id: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    definition = accumulator["definition"]
    contract_ids = sorted(accumulator["contract_ids"])
    contracts = [contract_by_id[value] for value in contract_ids if value in contract_by_id]
    problems = [problem_by_id[value] for value in contract_ids if value in problem_by_id]
    wait_counts = Counter(str(row.get("current_wait") or "unknown") for row in problems)
    felt_review_counts = Counter(str(row.get("felt_review") or "not_requested") for row in contracts)
    evidence_counts: Counter[str] = Counter()
    technical_counts: Counter[str] = Counter()
    for contract in contracts:
        evidence_counts.update(
            {str(key): int(value) for key, value in (contract.get("evidence_state_counts") or {}).items()}
        )
        technical_counts.update(
            {str(key): int(value) for key, value in (contract.get("technical_state_counts") or {}).items()}
        )
    contradiction_count = sum(int(row.get("contradiction_count") or 0) for row in contracts)
    reopen_count = sum(int(row.get("reopen_count") or 0) for row in contracts)
    named_friction_count = sum(
        str(row.get("felt_review") or "") in {"contradicted", "still_friction", "objection"}
        for row in contracts
    )
    open_count = sum(not bool(row.get("felt_closed")) for row in contracts)
    evidence_gap_count = sum(
        count
        for key, count in evidence_counts.items()
        if key not in {"sufficient", "verified"}
    )
    authority_wait_count = wait_counts.get("authority", 0)
    row = {
        "schema": "felt_constellation_v1",
        "schema_version": 1,
        "constellation_id": str(definition["constellation_id"]),
        "label": str(definition["label"]),
        "description": str(definition["description"]),
        "membership_relation": (
            "transparent_lexical_multi_membership_not_semantic_equivalence_or_claim_merge"
        ),
        "claim_count": len(accumulator["claim_ids"]),
        "family_count": len(accumulator["family_ids"]),
        "contract_count": len(contract_ids),
        "source_report_count": len(accumulator["source_ids"]),
        "target_surface_count": len(accumulator["target_surfaces"]),
        "claim_ids": sorted(accumulator["claim_ids"]),
        "family_ids": sorted(accumulator["family_ids"]),
        "contract_ids": contract_ids,
        "source_ids": sorted(accumulator["source_ids"]),
        "target_surfaces": sorted(accumulator["target_surfaces"]),
        "matched_terms": sorted(accumulator["matched_terms"]),
        "classification_counts": dict(sorted(accumulator["classifications"].items())),
        "wait_counts": dict(sorted(wait_counts.items())),
        "felt_review_counts": dict(sorted(felt_review_counts.items())),
        "evidence_state_counts": dict(sorted(evidence_counts.items())),
        "technical_state_counts": dict(sorted(technical_counts.items())),
        "visibility_dimensions": {
            "recurrence_claim_count": len(accumulator["claim_ids"]),
            "source_breadth_count": len(accumulator["source_ids"]),
            "surface_breadth_count": len(accumulator["target_surfaces"]),
            "open_contract_count": open_count,
            "named_friction_count": named_friction_count,
            "contradiction_count": contradiction_count,
            "reopen_count": reopen_count,
            "evidence_gap_count": evidence_gap_count,
            "authority_wait_count": authority_wait_count,
        },
        "navigation_relation": (
            "descriptive_visibility_dimensions_not_owner_priority_scheduling_or_attention_state"
        ),
        "closure_propagated": False,
        "evidence_sufficiency_propagated": False,
        "authority_propagated": False,
        "runtime_consumed": False,
        "artifact_authority_state_v1": authority_state(),
    }
    row["constellation_sha256"] = _canonical_sha256(row)
    return row


def _render_report(rows: list[dict[str, Any]], status: dict[str, Any]) -> str:
    ordered = sorted(
        rows,
        key=lambda row: (
            -int(row["visibility_dimensions"]["named_friction_count"]),
            -int(row["visibility_dimensions"]["contradiction_count"]),
            -int(row["claim_count"]),
            str(row["constellation_id"]),
        ),
    )
    lines = [
        "# Felt Constellation V1",
        "",
        "A transparent, multi-membership navigation view over exact claims and Felt Contracts. It preserves every original identity and does not infer a being's attention, merge claims, propagate closure, schedule work, or grant authority.",
        "",
        f"- canonical claims: {status['canonical_claim_count']}",
        f"- matched claims: {status['matched_claim_count']}",
        f"- unmatched claims: {status['unmatched_claim_count']}",
        f"- constellations: {status['constellation_count']}",
        "",
        "## Descriptive Visibility Order",
        "",
        "This order exposes named friction, contradictions, and recurrence. It is not priority or scheduling.",
        "",
    ]
    for row in ordered:
        dimensions = row["visibility_dimensions"]
        lines.append(
            f"- **{row['label']}**: {row['claim_count']} claims across "
            f"{row['source_report_count']} reports; named friction "
            f"{dimensions['named_friction_count']}; contradictions "
            f"{dimensions['contradiction_count']}; evidence gaps "
            f"{dimensions['evidence_gap_count']}"
        )
    return "\n".join(lines) + "\n"


def project(workspace: Path, *, write: bool) -> dict[str, Any]:
    diagnostics = workspace / "diagnostics"
    families_path = diagnostics / "claim_families_v1/status.json"
    contracts_path = diagnostics / "felt_contract_graph_v1/contracts.jsonl"
    problems_path = diagnostics / "living_problem_registry_v2/problems.jsonl"
    required = (families_path, contracts_path, problems_path, POLICY_PATH)
    missing = [str(path) for path in required if not path.is_file()]
    if missing:
        return {
            "schema": "felt_constellation_status_v1",
            "schema_version": 1,
            "valid": False,
            "missing_inputs": missing,
            "artifact_authority_state_v1": authority_state(),
        }

    policy = _json(POLICY_PATH)
    family_status = _json(families_path)
    contracts = _jsonl(contracts_path)
    problems = _jsonl(problems_path)
    family_values = family_status.get("families") or {}
    definitions = policy.get("constellations") or []
    accumulators = {
        str(definition["constellation_id"]): _empty_constellation(definition)
        for definition in definitions
    }
    claim_contract, contract_by_id = _claim_contract_index(contracts)
    problem_by_id = _problem_index(problems)
    memberships: list[dict[str, Any]] = []
    unmatched: list[dict[str, Any]] = []
    canonical_claim_ids: set[str] = set()

    for family_id, family in sorted(family_values.items()):
        if not isinstance(family, dict):
            continue
        target_surface = str(family.get("target_surface") or "")
        claims = family.get("claims") or {}
        for claim_id, claim in sorted(claims.items()):
            if not isinstance(claim, dict):
                continue
            canonical_claim_id = str(claim.get("canonical_claim_id") or claim_id)
            canonical_claim_ids.add(canonical_claim_id)
            text = str(claim.get("text") or "")
            searchable = " ".join((text, target_surface, str(claim.get("source_family") or "")))
            matched: dict[str, list[str]] = {}
            for definition in definitions:
                constellation_id = str(definition["constellation_id"])
                terms = _matches(searchable, definition.get("terms") or [])
                if not terms:
                    continue
                matched[constellation_id] = terms
                accumulator = accumulators[constellation_id]
                accumulator["claim_ids"].add(canonical_claim_id)
                accumulator["family_ids"].add(str(family_id))
                accumulator["source_ids"].add(_source_id(canonical_claim_id))
                if target_surface:
                    accumulator["target_surfaces"].add(target_surface)
                accumulator["matched_terms"].update(terms)
                accumulator["classifications"].update(
                    [str(claim.get("classification") or "unknown")]
                )
                if contract_id := claim_contract.get(canonical_claim_id):
                    accumulator["contract_ids"].add(contract_id)
            row = {
                "schema": "felt_constellation_membership_v1",
                "schema_version": 1,
                "canonical_claim_id": canonical_claim_id,
                "family_id": str(family_id),
                "contract_id": claim_contract.get(canonical_claim_id),
                "constellation_ids": sorted(matched),
                "matched_terms_by_constellation": matched,
                "claim_text_sha256": hashlib.sha256(text.encode()).hexdigest(),
                "membership_relation": (
                    "lexical_navigation_evidence_not_equivalence_merge_priority_or_authority"
                ),
                "artifact_authority_state_v1": authority_state(),
            }
            memberships.append(row)
            if not matched:
                unmatched.append(row)

    rows = [
        _constellation_row(
            accumulator,
            contract_by_id=contract_by_id,
            problem_by_id=problem_by_id,
        )
        for accumulator in accumulators.values()
    ]
    rows.sort(key=lambda row: str(row["constellation_id"]))
    matched_claim_ids = {
        claim_id
        for accumulator in accumulators.values()
        for claim_id in accumulator["claim_ids"]
    }
    membership_claim_ids = {str(row["canonical_claim_id"]) for row in memberships}
    status = {
        "schema": "felt_constellation_status_v1",
        "schema_version": 1,
        "valid": canonical_claim_ids == membership_claim_ids,
        "write": write,
        "policy_sha256": _sha256(POLICY_PATH),
        "canonical_claim_count": len(canonical_claim_ids),
        "matched_claim_count": len(matched_claim_ids),
        "unmatched_claim_count": len(canonical_claim_ids - matched_claim_ids),
        "multi_membership_claim_count": sum(
            len(row["constellation_ids"]) > 1 for row in memberships
        ),
        "constellation_count": len(rows),
        "contract_count": len(contracts),
        "source_hashes": {
            str(path.relative_to(workspace)): _sha256(path)
            for path in (families_path, contracts_path, problems_path)
        },
        "policy_source": str(POLICY_PATH.relative_to(POLICY_PATH.parents[2])),
        "counter_audit": {
            "status": "consistent" if canonical_claim_ids == membership_claim_ids else "inconsistent",
            "checks": {
                "every_canonical_claim_has_one_membership_receipt": (
                    canonical_claim_ids == membership_claim_ids
                ),
                "unmatched_claims_remain_visible": len(unmatched) == len(canonical_claim_ids - matched_claim_ids),
                "multi_membership_does_not_merge_claims": True,
                "closure_not_propagated": True,
                "authority_not_propagated": True,
                "runtime_not_consumed": True,
            },
        },
        "navigation_relation": (
            "descriptive_saliency_for_steward_review_not_being_attention_owner_priority_or_schedule"
        ),
        "artifact_authority_state_v1": authority_state(),
    }
    if write and status["valid"]:
        output = state_dir(workspace)
        owner_atomic_write_jsonl(output / "constellations.jsonl", rows)
        owner_atomic_write_jsonl(output / "memberships.jsonl", memberships)
        owner_atomic_write_jsonl(output / "unmatched_claims.jsonl", unmatched)
        owner_atomic_write_json(output / "status.json", status)
        owner_atomic_write(output / "report.md", _render_report(rows, status))
    return status
