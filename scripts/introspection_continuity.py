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
ADDRESSING_RELATIVE = Path("diagnostics/introspection_addressing_v1/status.json")
MAX_REPORT_BYTES = 2_000_000
MAX_CLAIMS = 24
MAX_CARD_BYTES = 256 * 1024
MAX_EXCLUSION_DETAILS = 32
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
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
    return hashes


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

    cards: dict[str, bytes] = {}
    index_entries: list[dict[str, Any]] = []
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
        relative = f"cards/{card['card_id']}.json"
        encoded = _json_bytes(card)
        cards[relative] = encoded
        index_entries.append(
            {
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
        )
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
