#!/usr/bin/env python3
"""Project bounded steward evidence cards for later introspection windows."""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import time
from typing import Any

try:
    from authority_state import assert_artifact_authority_tree
    from evidence_store.model import canonical_json
    from experiential_systems.common import authority_state, owner_atomic_write
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.authority_state import assert_artifact_authority_tree
    from scripts.evidence_store.model import canonical_json
    from scripts.experiential_systems.common import authority_state, owner_atomic_write
    from scripts.projection_receipt import projector_receipt

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_WORKSPACE = ROOT / "capsules/spectral-bridge/workspace"
STATE_RELATIVE = Path("diagnostics/introspection_continuity_v1")
RESPONSES_RELATIVE = STATE_RELATIVE / "responses"
ADDRESSING_RELATIVE = Path("diagnostics/introspection_addressing_v1/status.json")
MAX_REPORT_BYTES = 2_000_000
MAX_CLAIMS = 24
MAX_CARD_BYTES = 256 * 1024
MAX_EXCLUSION_DETAILS = 32
MAX_RESPONSE_BYTES = 64 * 1024
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
CARD_ID_RE = re.compile(r"^icv1_[0-9a-f]{64}$")
RECEIPT_ID_RE = re.compile(r"^icrv1_[0-9a-f]{64}$")
ASSESSMENT_STATUSES = {
    "mechanical_only",
    "still_friction",
    "contradicted",
    "not_assessed",
}
SOURCE_RE = re.compile(r"^Source: (?P<label>.+?) \((?P<path>.+)\)$")
HEADER_FIELDS = {
    "Source SHA-256": "source_sha256",
    "Source read session": "read_session_id",
    "Timestamp": "captured_at_unix",
    "Lived-state witness": "lived_state_witness_id",
}
INPUT_RELATIVES = (
    ADDRESSING_RELATIVE,
    Path("diagnostics/claim_families_v1/status.json"),
    Path("diagnostics/felt_contract_graph_v1/contracts.jsonl"),
    Path("diagnostics/steward_work_selection_v1/selection.json"),
)


class ContinuityProjectionError(ValueError):
    """Canonical continuity inputs failed a strict projection contract."""


def state_dir(workspace: Path) -> Path:
    return workspace / STATE_RELATIVE


def _sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _json_bytes(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")


def _input_hashes(workspace: Path) -> dict[str, dict[str, Any]]:
    hashes: dict[str, dict[str, Any]] = {}
    for relative in INPUT_RELATIVES:
        path = workspace / relative
        raw = path.read_bytes() if path.is_file() else b""
        hashes[relative.as_posix()] = {
            "present": path.is_file(),
            "sha256": _sha256_bytes(raw),
        }
    response_rows: list[dict[str, str]] = []
    for path in sorted((workspace / RESPONSES_RELATIVE).glob("*.json")):
        raw = path.read_bytes()
        response_rows.append(
            {
                "path": path.relative_to(workspace).as_posix(),
                "sha256": _sha256_bytes(raw),
            }
        )
    hashes[f"{RESPONSES_RELATIVE.as_posix()}/*.json"] = {
        "present": bool(response_rows),
        "count": len(response_rows),
        "sha256": _sha256_bytes(canonical_json(response_rows).encode("utf-8")),
    }
    return hashes


def _authority_is_evidence_only(value: Any) -> bool:
    return value == {
        "schema": "artifact_authority_state_v1",
        "schema_version": 1,
        "state": "evidence_only",
        "witness_only": True,
    }


def _response_receipt(path: Path, workspace: Path) -> dict[str, Any]:
    if path.stat().st_size > MAX_RESPONSE_BYTES:
        raise ContinuityProjectionError(
            f"response receipt exceeds {MAX_RESPONSE_BYTES} bytes: {path.name}"
        )
    if path.stat().st_mode & 0o077:
        raise ContinuityProjectionError(
            f"response receipt is not owner-only: {path.name}"
        )
    raw = path.read_bytes()
    try:
        receipt = json.loads(raw)
    except json.JSONDecodeError as error:
        raise ContinuityProjectionError(
            f"response receipt is invalid JSON: {path.name}"
        ) from error
    expected_keys = {
        "schema",
        "schema_version",
        "receipt_id",
        "card_id",
        "assessment_status",
        "current_introspection",
        "response_line",
        "recorded_at_unix_ms",
        "bound",
        "linked_prior_introspection_id",
        "linked_claim_ids",
        "right_to_ignore",
        "silence_is_neutral",
        "mechanical_evidence_only",
        "felt_closure_inferred",
        "consent_inferred",
        "no_authority",
        "authority_boundary",
        "artifact_authority_state_v1",
    }
    if not isinstance(receipt, dict) or set(receipt) != expected_keys:
        raise ContinuityProjectionError(
            f"response receipt schema fields mismatch: {path.name}"
        )
    if receipt["schema"] != "introspection_continuity_response_v1" or receipt[
        "schema_version"
    ] != 1:
        raise ContinuityProjectionError(
            f"response receipt schema/version mismatch: {path.name}"
        )
    receipt_id = str(receipt["receipt_id"])
    card_id = str(receipt["card_id"])
    if RECEIPT_ID_RE.fullmatch(receipt_id) is None or path.stem != receipt_id:
        raise ContinuityProjectionError(
            f"response receipt id/path mismatch: {path.name}"
        )
    if CARD_ID_RE.fullmatch(card_id) is None:
        raise ContinuityProjectionError(
            f"response receipt card id is invalid: {path.name}"
        )
    status = str(receipt["assessment_status"])
    if status not in ASSESSMENT_STATUSES:
        raise ContinuityProjectionError(
            f"response receipt assessment is invalid: {path.name}"
        )
    current = receipt["current_introspection"]
    if not isinstance(current, dict) or set(current) != {
        "path",
        "introspection_id",
        "sha256",
    }:
        raise ContinuityProjectionError(
            f"response receipt current introspection is invalid: {path.name}"
        )
    current_path = _safe_relative(current["path"], prefix="introspections/")
    current_sha256 = str(current["sha256"])
    if SHA256_RE.fullmatch(current_sha256) is None:
        raise ContinuityProjectionError(
            f"response receipt current SHA-256 is invalid: {path.name}"
        )
    current_raw = _report_bytes(workspace, current_path, current_sha256)
    introspection_id = _bounded(
        current["introspection_id"], "response introspection_id", 240
    )
    if introspection_id != PurePosixPath(current_path).stem:
        raise ContinuityProjectionError(
            f"response receipt current path/id mismatch: {path.name}"
        )
    response_line = receipt["response_line"]
    recorded_at = receipt["recorded_at_unix_ms"]
    if (
        not isinstance(response_line, int)
        or isinstance(response_line, bool)
        or response_line <= 0
        or not isinstance(recorded_at, int)
        or isinstance(recorded_at, bool)
        or recorded_at <= 0
    ):
        raise ContinuityProjectionError(
            f"response receipt line/timestamp is invalid: {path.name}"
        )
    try:
        current_lines = current_raw.decode("utf-8", errors="strict").splitlines()
    except UnicodeDecodeError as error:
        raise ContinuityProjectionError(
            f"response receipt current introspection is not UTF-8: {path.name}"
        ) from error
    expected_response = f"Prior Evidence: {card_id} :: {status}"
    if (
        response_line > len(current_lines)
        or current_lines[response_line - 1] != expected_response
    ):
        raise ContinuityProjectionError(
            f"response receipt exact report line mismatch: {path.name}"
        )
    expected_receipt_id = "icrv1_" + _sha256_bytes(
        (
            f"{card_id}\0{current_path}\0{current_sha256}\0"
            f"{response_line}\0{status}"
        ).encode("utf-8")
    )
    if receipt_id != expected_receipt_id:
        raise ContinuityProjectionError(
            f"response receipt deterministic id mismatch: {path.name}"
        )
    linked_claim_ids = receipt["linked_claim_ids"]
    if not isinstance(linked_claim_ids, list) or len(linked_claim_ids) > MAX_CLAIMS:
        raise ContinuityProjectionError(
            f"response receipt claim binding is invalid: {path.name}"
        )
    linked_claim_ids = [
        _bounded(value, "linked claim id", 120) for value in linked_claim_ids
    ]
    if len(set(linked_claim_ids)) != len(linked_claim_ids):
        raise ContinuityProjectionError(
            f"response receipt repeats claim ids: {path.name}"
        )
    bound = receipt["bound"]
    linked_introspection_id = receipt["linked_prior_introspection_id"]
    if not isinstance(bound, bool):
        raise ContinuityProjectionError(
            f"response receipt bound marker is invalid: {path.name}"
        )
    if bound:
        linked_introspection_id = _bounded(
            linked_introspection_id, "linked prior introspection id", 240
        )
        if not linked_claim_ids:
            raise ContinuityProjectionError(
                f"bound response receipt has no claim ids: {path.name}"
            )
    elif linked_introspection_id is not None or linked_claim_ids:
        raise ContinuityProjectionError(
            f"unbound response receipt carries prior bindings: {path.name}"
        )
    if (
        receipt["right_to_ignore"] is not True
        or receipt["silence_is_neutral"] is not True
        or receipt["mechanical_evidence_only"] is not True
        or receipt["felt_closure_inferred"] is not False
        or receipt["consent_inferred"] is not False
        or receipt["no_authority"] is not True
        or receipt["authority_boundary"]
        != "response_is_evidence_only_not_felt_closure_control_approval_or_activation"
        or not _authority_is_evidence_only(receipt["artifact_authority_state_v1"])
    ):
        raise ContinuityProjectionError(
            f"response receipt authority boundary is invalid: {path.name}"
        )
    return {
        **receipt,
        "receipt_id": receipt_id,
        "card_id": card_id,
        "assessment_status": status,
        "current_introspection": {
            "path": current_path,
            "introspection_id": introspection_id,
            "sha256": current_sha256,
        },
        "linked_prior_introspection_id": linked_introspection_id,
        "linked_claim_ids": linked_claim_ids,
        "receipt_sha256": _sha256_bytes(raw),
    }


def load_response_receipts(
    workspace: Path,
) -> tuple[list[dict[str, Any]], list[dict[str, str]]]:
    receipts: list[dict[str, Any]] = []
    rejected: list[dict[str, str]] = []
    for path in sorted((workspace / RESPONSES_RELATIVE).glob("*.json")):
        try:
            receipts.append(_response_receipt(path, workspace))
        except (ContinuityProjectionError, OSError) as error:
            rejected.append({"path": path.name, "reason": str(error)})
    return receipts, rejected


def _safe_relative(value: Any, *, prefix: str | None = None) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ContinuityProjectionError("relative path must be a non-empty string")
    candidate = PurePosixPath(value.strip())
    if candidate.is_absolute() or ".." in candidate.parts:
        raise ContinuityProjectionError(f"unsafe relative path: {value!r}")
    normalized = candidate.as_posix()
    if prefix and not normalized.startswith(prefix):
        raise ContinuityProjectionError(
            f"relative path must begin with {prefix!r}: {normalized!r}"
        )
    if Path(normalized).name.startswith("moment_"):
        raise ContinuityProjectionError("private moment artifacts are never card sources")
    return normalized


def _bounded(value: Any, field: str, limit: int) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ContinuityProjectionError(f"{field} must be a non-empty string")
    result = value.strip()
    if len(result.encode("utf-8")) > limit:
        raise ContinuityProjectionError(f"{field} exceeds {limit} bytes")
    return result


def _source_locator(raw_path: str) -> str | None:
    marker = "/capsules/spectral-bridge/"
    normalized = raw_path.replace("\\", "/")
    if marker in normalized:
        suffix = normalized.split(marker, 1)[1]
        return _safe_relative(f"capsules/spectral-bridge/{suffix}")
    if not normalized.startswith("/"):
        try:
            return _safe_relative(normalized)
        except ContinuityProjectionError:
            return None
    return None


def _parse_report_header(raw: bytes) -> dict[str, Any]:
    try:
        report = raw.decode("utf-8", errors="strict")
    except UnicodeDecodeError as error:
        raise ContinuityProjectionError("canonical report is not valid UTF-8") from error
    header: dict[str, Any] = {}
    for line in report.splitlines():
        if not line.strip():
            break
        source_match = SOURCE_RE.match(line)
        if source_match:
            label = _bounded(source_match.group("label"), "source label", 240)
            locator = _source_locator(source_match.group("path"))
            identity_basis = {"label": label, "locator": locator}
            header.update(
                {
                    "source_label": label,
                    "source_locator": locator,
                    "stable_source_identity": (
                        "source_"
                        + _sha256_bytes(canonical_json(identity_basis).encode("utf-8"))
                    ),
                }
            )
            continue
        for label, field in HEADER_FIELDS.items():
            prefix = f"{label}: "
            if line.startswith(prefix):
                header[field] = line[len(prefix) :].strip()
                break
    required = (
        "source_label",
        "stable_source_identity",
        "source_sha256",
        "read_session_id",
        "captured_at_unix",
    )
    missing = [field for field in required if not header.get(field)]
    if missing:
        raise ContinuityProjectionError(
            "source-first report header incomplete: " + ",".join(missing)
        )
    for field in ("source_sha256", "read_session_id"):
        if SHA256_RE.fullmatch(str(header[field])) is None:
            raise ContinuityProjectionError(f"{field} must be a lowercase SHA-256")
    try:
        captured_at = int(header["captured_at_unix"])
    except (TypeError, ValueError) as error:
        raise ContinuityProjectionError("captured_at_unix must be an integer") from error
    if captured_at <= 0:
        raise ContinuityProjectionError("captured_at_unix must be positive")
    header["captured_at_unix"] = captured_at
    witness = header.get("lived_state_witness_id")
    if witness is not None:
        header["lived_state_witness_id"] = _bounded(
            witness, "lived_state_witness_id", 160
        )
    return header


def _evidence_locator(value: Any) -> str | None:
    if not isinstance(value, str) or not value.strip():
        return None
    normalized = value.strip().replace("\\", "/")
    marker = "/astrid/"
    if marker in normalized:
        normalized = normalized.split(marker, 1)[1]
    if normalized.startswith("/") or "://" in normalized:
        return None
    try:
        return _safe_relative(normalized)
    except ContinuityProjectionError:
        return None


def _evidence_refs(claim: dict[str, Any]) -> tuple[list[dict[str, str]], int]:
    refs: set[tuple[str, str]] = set()
    omitted = 0
    evidence = claim.get("evidence")
    if not isinstance(evidence, list):
        raise ContinuityProjectionError("claim evidence must be a list")
    for item in evidence:
        if not isinstance(item, dict) or not item.get("target"):
            continue
        kind = _bounded(item.get("kind") or "evidence", "evidence kind", 80)
        target = _evidence_locator(item["target"])
        if target is None:
            omitted += 1
            continue
        refs.add((kind, target))
    return (
        [{"kind": kind, "target": target} for kind, target in sorted(refs)],
        omitted,
    )


def _claims(artifact: dict[str, Any]) -> list[dict[str, Any]]:
    source = artifact.get("claims")
    if not isinstance(source, dict) or not source:
        raise ContinuityProjectionError("artifact must carry extracted claims")
    if len(source) > MAX_CLAIMS:
        raise ContinuityProjectionError(
            f"claim count {len(source)} exceeds bounded maximum {MAX_CLAIMS}"
        )
    claims: list[dict[str, Any]] = []
    for key, value in sorted(source.items()):
        if not isinstance(value, dict):
            raise ContinuityProjectionError(f"claim {key} must be an object")
        claim_id = _bounded(value.get("claim_id") or key, "claim_id", 120)
        evidence_refs, omitted_evidence_ref_count = _evidence_refs(value)
        claims.append(
            {
                "claim_id": claim_id,
                "summary": _bounded(value.get("summary"), "claim summary", 2_000),
                "classification": _bounded(
                    value.get("classification"), "claim classification", 240
                ),
                "disposition": _bounded(
                    value.get("disposition"), "claim disposition", 1_000
                ),
                "grounded_disposition": _bounded(
                    value.get("grounded_disposition"),
                    "grounded disposition",
                    4_000,
                ),
                "evidence_refs": evidence_refs,
                "omitted_evidence_ref_count": omitted_evidence_ref_count,
                "authority_wait": (
                    _bounded(value.get("authority"), "authority wait", 1_000)
                    if value.get("authority")
                    else None
                ),
            }
        )
    return claims


def _eligible(artifact: Any) -> bool:
    return (
        isinstance(artifact, dict)
        and artifact.get("present_on_disk") is not False
        and artifact.get("full_read") is True
        and isinstance(artifact.get("claims"), dict)
        and bool(artifact.get("claims"))
    )


def _report_bytes(workspace: Path, relative: str, expected_sha256: str) -> bytes:
    path = workspace / relative
    resolved_root = (workspace / "introspections").resolve()
    try:
        resolved = path.resolve(strict=True)
    except FileNotFoundError as error:
        raise ContinuityProjectionError(f"canonical report missing: {relative}") from error
    if not resolved.is_relative_to(resolved_root):
        raise ContinuityProjectionError("canonical report escapes introspection root")
    size = resolved.stat().st_size
    if size > MAX_REPORT_BYTES:
        raise ContinuityProjectionError(
            f"canonical report exceeds {MAX_REPORT_BYTES} bytes: {relative}"
        )
    raw = resolved.read_bytes()
    if _sha256_bytes(raw) != expected_sha256:
        raise ContinuityProjectionError(f"canonical report SHA-256 mismatch: {relative}")
    return raw


def _build_card(
    workspace: Path, introspection_id: str, artifact: dict[str, Any]
) -> dict[str, Any]:
    relative = _safe_relative(
        artifact.get("relative_path") or artifact.get("filename"),
        prefix="introspections/",
    )
    report_sha256 = _bounded(artifact.get("sha256"), "report sha256", 64)
    if SHA256_RE.fullmatch(report_sha256) is None:
        raise ContinuityProjectionError("report sha256 must be lowercase SHA-256")
    raw = _report_bytes(workspace, relative, report_sha256)
    header = _parse_report_header(raw)
    claims = _claims(artifact)
    witness = artifact.get("lived_state_witness_id") or header.get(
        "lived_state_witness_id"
    )
    if witness:
        witness = _bounded(witness, "lived_state_witness_id", 160)
    core = {
        "introspection_id": introspection_id,
        "report_sha256": report_sha256,
        "stable_source_identity": header["stable_source_identity"],
        "source_sha256": header["source_sha256"],
        "read_session_id": header["read_session_id"],
        "claims": claims,
    }
    card_id = "icv1_" + _sha256_bytes(canonical_json(core).encode("utf-8"))
    remaining_gaps = [
        {
            "claim_id": claim["claim_id"],
            "classification": claim["classification"],
            "grounded_disposition": claim["grounded_disposition"],
        }
        for claim in claims
        if "gap" in claim["classification"].lower()
    ]
    authority_waits = [
        {
            "claim_id": claim["claim_id"],
            "classification": claim["classification"],
            "authority_wait": claim["authority_wait"],
        }
        for claim in claims
        if claim["authority_wait"] is not None
        or re.search(r"tier_[45]_wait", claim["classification"].lower())
    ]
    card = {
        "schema": "introspection_continuity_card_v1",
        "schema_version": 1,
        "card_id": card_id,
        "introspection_id": introspection_id,
        "captured_at_unix": header["captured_at_unix"],
        "canonical_report": {
            "path": relative,
            "sha256": report_sha256,
            "lived_state_witness_id": witness,
        },
        "source": {
            "label": header["source_label"],
            "stable_identity": header["stable_source_identity"],
            "locator": header.get("source_locator"),
            "sha256": header["source_sha256"],
            "read_session_id": header["read_session_id"],
        },
        "claims": claims,
        "claim_count": len(claims),
        "remaining_gaps": remaining_gaps,
        "authority_waits": authority_waits,
        "right_to_ignore": True,
        "silence_is_neutral": True,
        "mechanical_evidence_only": True,
        "felt_closure_inferred": False,
        "consent_inferred": False,
        "no_authority": True,
        "authority_boundary": (
            "mechanical_evidence_not_felt_closure_control_approval_or_activation"
        ),
        "artifact_authority_state_v1": authority_state(),
    }
    assert_artifact_authority_tree(card)
    if len(_json_bytes(card)) > MAX_CARD_BYTES:
        raise ContinuityProjectionError(
            f"card exceeds {MAX_CARD_BYTES} bytes: {introspection_id}"
        )
    return card


def build_projection(workspace: Path) -> tuple[dict[str, Any], dict[str, bytes], str]:
    workspace = workspace.resolve()
    addressing_path = workspace / ADDRESSING_RELATIVE
    if not addressing_path.is_file():
        raise ContinuityProjectionError("addressing status is missing")
    status_raw = addressing_path.read_bytes()
    addressing = json.loads(status_raw)
    if not isinstance(addressing, dict) or not isinstance(
        addressing.get("artifacts"), dict
    ):
        raise ContinuityProjectionError("addressing status artifacts must be an object")

    built_cards: list[dict[str, Any]] = []
    exclusions: list[dict[str, str]] = []
    exclusion_counts: Counter[str] = Counter()
    eligible_count = 0
    for artifact_key, artifact in sorted(addressing["artifacts"].items()):
        if not _eligible(artifact):
            continue
        eligible_count += 1
        introspection_id = _bounded(
            artifact.get("introspection_id") or artifact_key,
            "introspection_id",
            240,
        )
        try:
            card = _build_card(workspace, introspection_id, artifact)
        except ContinuityProjectionError as error:
            reason = str(error)
            key = reason.split(":", 1)[0]
            exclusion_counts[key] += 1
            if len(exclusions) < MAX_EXCLUSION_DETAILS:
                exclusions.append(
                    {"introspection_id": introspection_id, "reason": reason}
                )
            continue
        built_cards.append(card)

    cards_by_id = {card["card_id"]: card for card in built_cards}
    response_receipts, rejected_responses = load_response_receipts(workspace)
    latest_response: dict[str, dict[str, Any]] = {}
    response_counts: Counter[str] = Counter()
    for receipt in response_receipts:
        if not receipt["bound"]:
            response_counts["unbound"] += 1
            continue
        card = cards_by_id.get(receipt["card_id"])
        if card is None:
            response_counts["stale_binding"] += 1
            continue
        expected_claim_ids = sorted(
            claim["claim_id"] for claim in card["claims"]
        )
        if (
            receipt["linked_prior_introspection_id"] != card["introspection_id"]
            or sorted(receipt["linked_claim_ids"]) != expected_claim_ids
        ):
            response_counts["stale_binding"] += 1
            continue
        response_counts["matching_bound"] += 1
        previous = latest_response.get(receipt["card_id"])
        receipt_order = (
            receipt["recorded_at_unix_ms"],
            receipt["response_line"],
            receipt["receipt_id"],
        )
        previous_order = (
            previous["recorded_at_unix_ms"],
            previous["response_line"],
            previous["receipt_id"],
        ) if previous else None
        if previous_order is None or receipt_order > previous_order:
            if previous is not None:
                response_counts["superseded_matching"] += 1
            latest_response[receipt["card_id"]] = receipt
        else:
            response_counts["superseded_matching"] += 1

    cards: dict[str, bytes] = {}
    index_entries: list[dict[str, Any]] = []
    applied_status_counts: Counter[str] = Counter()
    for card in built_cards:
        assessment = latest_response.get(card["card_id"])
        if assessment is not None:
            status = assessment["assessment_status"]
            applied_status_counts[status] += 1
            card["prior_evidence_assessment"] = {
                "status": status,
                "receipt_id": assessment["receipt_id"],
                "observed_in_introspection": assessment[
                    "current_introspection"
                ],
                "recorded_at_unix_ms": assessment["recorded_at_unix_ms"],
                "response_line": assessment["response_line"],
                "mechanical_evidence_only": True,
                "felt_closure_inferred": False,
                "consent_inferred": False,
                "no_authority": True,
            }
        assert_artifact_authority_tree(card)
        relative = f"cards/{card['card_id']}.json"
        encoded = _json_bytes(card)
        if len(encoded) > MAX_CARD_BYTES:
            raise ContinuityProjectionError(
                f"annotated card exceeds {MAX_CARD_BYTES} bytes: {card['card_id']}"
            )
        cards[relative] = encoded
        index_entry = {
            "card_id": card["card_id"],
            "card_path": f"{STATE_RELATIVE.as_posix()}/{relative}",
            "card_sha256": _sha256_bytes(encoded),
            "introspection_id": card["introspection_id"],
            "captured_at_unix": card["captured_at_unix"],
            "report_sha256": card["canonical_report"]["sha256"],
            "stable_source_identity": card["source"]["stable_identity"],
            "source_sha256": card["source"]["sha256"],
            "read_session_id": card["source"]["read_session_id"],
        }
        if assessment is not None:
            index_entry["assessment_status"] = assessment["assessment_status"]
            index_entry["assessment_receipt_id"] = assessment["receipt_id"]
        index_entries.append(index_entry)
    index_entries.sort(
        key=lambda item: (
            item["stable_source_identity"],
            item["captured_at_unix"],
            item["card_id"],
        )
    )
    index = {
        "schema": "introspection_continuity_index_v1",
        "schema_version": 1,
        "cards": index_entries,
        "card_count": len(index_entries),
        "right_to_ignore": True,
        "silence_is_neutral": True,
        "mechanical_evidence_only": True,
        "artifact_authority_state_v1": authority_state(),
    }
    projection_status = {
        "schema": "introspection_continuity_projection_status_v1",
        "schema_version": 1,
        "valid": True,
        "summary": {
            "addressing_artifact_count": len(addressing["artifacts"]),
            "eligible_artifact_count": eligible_count,
            "card_count": len(index_entries),
            "excluded_eligible_count": eligible_count - len(index_entries),
            "response_receipt_file_count": len(response_receipts)
            + len(rejected_responses),
            "response_receipt_valid_count": len(response_receipts),
            "response_receipt_rejected_count": len(rejected_responses),
            "response_receipt_applied_count": len(latest_response),
        },
        "response_receipts": {
            "counts": dict(sorted(response_counts.items())),
            "applied_status_counts": dict(sorted(applied_status_counts.items())),
            "rejected": rejected_responses[:MAX_EXCLUSION_DETAILS],
            "rejected_detail_truncated": len(rejected_responses)
            > MAX_EXCLUSION_DETAILS,
            "mechanical_evidence_only": True,
            "felt_closure_inferred": False,
            "consent_inferred": False,
            "no_authority": True,
            "artifact_authority_state_v1": authority_state(),
        },
        "exclusion_counts": dict(sorted(exclusion_counts.items())),
        "exclusions": exclusions,
        "exclusion_detail_truncated": (
            sum(exclusion_counts.values()) > len(exclusions)
        ),
        "input_hashes": _input_hashes(workspace),
        "right_to_ignore": True,
        "silence_is_neutral": True,
        "felt_closure_inferred": False,
        "consent_inferred": False,
        "authority_boundary": (
            "projection_is_mechanical_evidence_not_felt_review_or_live_authority"
        ),
        "artifact_authority_state_v1": authority_state(),
    }
    assert_artifact_authority_tree(index)
    assert_artifact_authority_tree(projection_status)
    report = "\n".join(
        (
            "# Introspection Continuity V1",
            "",
            f"- Cards: {len(index_entries)}",
            f"- Eligible addressing artifacts: {eligible_count}",
            f"- Excluded eligible artifacts: {eligible_count - len(index_entries)}",
            f"- Applied response assessments: {len(latest_response)}",
            f"- Rejected response receipts: {len(rejected_responses)}",
            "- Right to ignore: yes",
            "- Silence is neutral: yes",
            "- Authority: mechanical evidence only; no felt closure, approval, activation, or control",
            "",
        )
    )
    outputs = {"index.json": _json_bytes(index), "status.json": _json_bytes(projection_status)}
    outputs.update(cards)
    return projection_status, outputs, report


def project(workspace: Path, *, write: bool) -> dict[str, Any]:
    status, outputs, report = build_projection(workspace)
    if not write:
        return status
    root = state_dir(workspace)
    cards_root = root / "cards"
    cards_root.mkdir(parents=True, exist_ok=True)
    os.chmod(cards_root, 0o700)
    expected_cards = {
        Path(relative).name for relative in outputs if relative.startswith("cards/")
    }
    for stale in sorted(cards_root.glob("icv1_*.json")):
        if stale.name not in expected_cards:
            stale.unlink()
    for relative, encoded in sorted(outputs.items()):
        owner_atomic_write(root / relative, encoded)
    owner_atomic_write(root / "report.md", report)
    return status


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("command", choices=("project", "verify", "report"))
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--receipt-json", action="store_true")
    args = parser.parse_args(argv)
    started = time.monotonic()
    workspace = args.workspace.resolve()
    try:
        if args.command == "report":
            path = state_dir(workspace) / "status.json"
            result = (
                json.loads(path.read_text(encoding="utf-8"))
                if path.is_file()
                else {"valid": False, "error": "status_missing"}
            )
        else:
            result = project(
                workspace,
                write=args.command == "project" and args.write,
            )
        if args.receipt_json:
            root = state_dir(workspace)
            result = projector_receipt(
                "introspection_continuity",
                result,
                {
                    "index.json": root / "index.json",
                    "status.json": root / "status.json",
                    "report.md": root / "report.md",
                },
                started_monotonic=started,
            )
    except (ContinuityProjectionError, json.JSONDecodeError, OSError) as error:
        result = {"valid": False, "error": str(error)}
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if result.get("valid", True) is not False else 1


if __name__ == "__main__":
    raise SystemExit(main())
