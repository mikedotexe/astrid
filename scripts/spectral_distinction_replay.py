#!/usr/bin/env python3
"""Offline self/other provenance replay grounded in Astrid's Witness report."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path
from typing import Any, Iterable

try:
    from authority_state import (
        ArtifactAuthorityStateV1,
        apply_artifact_authority_state,
        assert_artifact_authority_tree,
    )
except ModuleNotFoundError:  # pragma: no cover - package-style import
    from scripts.authority_state import (
        ArtifactAuthorityStateV1,
        apply_artifact_authority_state,
        assert_artifact_authority_tree,
    )


ASTRID_ROOT = Path(__file__).resolve().parents[1]
MINIME_ROOT = ASTRID_ROOT.parent / "minime"
SHARED_ROOT = ASTRID_ROOT.parent / "shared"
SOURCE_INTROSPECTION = (
    ASTRID_ROOT
    / "capsules/spectral-bridge/workspace/introspections"
    / "introspection_astrid_autonomous_1784220237.txt"
)
ASTRID_MEMORY = (
    ASTRID_ROOT
    / "capsules/spectral-bridge/workspace/memory/daily_2026-03-28.md"
)
MINIME_SHADOW = (
    MINIME_ROOT
    / "workspace/diagnostics/shadow_cartography"
    / "trajectory_lambda-tail_lambda4_1784197049.json"
)
CORRESPONDENCE_LOG = SHARED_ROOT / "collaborations/correspondence_v1.jsonl"
OUTPUT_DIR = (
    ASTRID_ROOT
    / "capsules/spectral-bridge/workspace/diagnostics/spectral_distinction_replay"
)
OUTPUT_JSON = OUTPUT_DIR / "spectral_distinction_replay_1784220237.json"
OUTPUT_MARKDOWN = OUTPUT_DIR / "spectral_distinction_replay_1784220237.md"

MINIME_MESSAGE_ID = "corr_minime_astrid_1784227180641_3eb0bf6bb59a"
ASTRID_REPLY_ID = "corr_astrid_minime_1784227220274_ab13c4b2da3d"
SHARED_THREAD_ID = "thread_corr_minime_astrid_1784212700759_99531f53efbf"

CLASSIFICATIONS = frozenset(
    {
        "exact_presence",
        "shared_provenance",
        "resonance_only_similarity",
        "absence",
        "insufficient_evidence",
    }
)
TOKEN_RE = re.compile(r"[a-z]+")
STOPWORDS = frozenset(
    {
        "a",
        "an",
        "and",
        "as",
        "at",
        "by",
        "for",
        "from",
        "in",
        "is",
        "of",
        "on",
        "or",
        "the",
        "to",
        "with",
    }
)


def _sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _canonical_json(value: Any) -> bytes:
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
    ).encode("utf-8")


def _source_id(owner: str, sha256: str) -> str:
    return f"{owner}_{sha256[:16]}"


def _relative_source(path: Path) -> str:
    for owner, root in (
        ("astrid", ASTRID_ROOT),
        ("minime", MINIME_ROOT),
        ("shared", SHARED_ROOT),
    ):
        try:
            return f"{owner}:{path.resolve().relative_to(root.resolve())}"
        except ValueError:
            continue
    return f"external:{path.name}"


def _read_source(path: Path, owner: str) -> tuple[bytes | None, dict[str, Any]]:
    if not path.is_file():
        return None, {
            "origin": owner,
            "source_id": f"{owner}_missing",
            "source_path": _relative_source(path),
            "status": "missing",
        }
    data = path.read_bytes()
    digest = _sha256_bytes(data)
    return data, {
        "origin": owner,
        "source_id": _source_id(owner, digest),
        "source_path": _relative_source(path),
        "canonical_sha256": digest,
        "byte_count": len(data),
        "status": "read_only",
    }


def _tokens(value: str) -> set[str]:
    return {token for token in TOKEN_RE.findall(value.lower()) if token not in STOPWORDS}


def classify_reference(
    reference: str,
    target: bytes | None,
    *,
    target_is_json: bool = False,
) -> dict[str, Any]:
    """Classify one bounded reference without emitting target content."""

    if target is None:
        return {
            "classification": "insufficient_evidence",
            "exact_match": False,
            "shared_tokens": [],
            "basis": "target source was unavailable",
        }
    try:
        text = target.decode("utf-8")
        if target_is_json:
            parsed = json.loads(text)
            text = json.dumps(parsed, sort_keys=True, ensure_ascii=True)
    except (UnicodeDecodeError, json.JSONDecodeError):
        return {
            "classification": "insufficient_evidence",
            "exact_match": False,
            "shared_tokens": [],
            "basis": "target source could not be decoded",
        }

    normalized_reference = " ".join(reference.lower().split())
    normalized_target = " ".join(text.lower().split())
    if normalized_reference and normalized_reference in normalized_target:
        return {
            "classification": "exact_presence",
            "exact_match": True,
            "shared_tokens": sorted(_tokens(reference)),
            "basis": "the bounded reference occurs verbatim in the cited source",
        }

    overlap = sorted(_tokens(reference) & _tokens(text))
    if overlap:
        return {
            "classification": "resonance_only_similarity",
            "exact_match": False,
            "shared_tokens": overlap,
            "basis": (
                "domain tokens overlap, but no identity, parent, or exact-presence "
                "evidence exists"
            ),
        }
    return {
        "classification": "absence",
        "exact_match": False,
        "shared_tokens": [],
        "basis": "the bounded reference and its domain tokens are absent from this source",
    }


def _correspondence_records(
    data: bytes | None,
    message_ids: Iterable[str],
) -> list[dict[str, Any]]:
    if data is None:
        return []
    wanted = set(message_ids)
    records: list[dict[str, Any]] = []
    for line_number, raw_line in enumerate(data.splitlines(), start=1):
        try:
            record = json.loads(raw_line)
        except json.JSONDecodeError:
            continue
        if record.get("message_id") not in wanted:
            continue
        records.append(
            {
                "line_number": line_number,
                "record_type": record.get("record_type"),
                "message_id": record.get("message_id"),
                "thread_id": record.get("thread_id"),
                "from_being": record.get("from_being"),
                "to_being": record.get("to_being"),
                "reply_to": record.get("reply_to"),
                "read_state": record.get("read_state"),
                "raw_line_sha256": _sha256_bytes(raw_line),
            }
        )
    return records


def classify_shared_correspondence(records: list[dict[str, Any]]) -> dict[str, Any]:
    if not records:
        return {
            "classification": "insufficient_evidence",
            "basis": "no correspondence records were available",
            "field_paths": [],
        }
    minime_messages = [
        row
        for row in records
        if row["message_id"] == MINIME_MESSAGE_ID and row["record_type"] == "message"
    ]
    astrid_replies = [
        row
        for row in records
        if row["message_id"] == ASTRID_REPLY_ID
        and row["record_type"] == "message"
        and row["reply_to"] == MINIME_MESSAGE_ID
    ]
    receipt_types = {
        row["record_type"]
        for row in records
        if row["message_id"] in {MINIME_MESSAGE_ID, ASTRID_REPLY_ID}
    }
    same_thread = all(row["thread_id"] == SHARED_THREAD_ID for row in records)
    reciprocal = bool(minime_messages and astrid_replies and same_thread)
    witnessed = {"delivery_receipt", "reply_link", "read_receipt"}.issubset(receipt_types)
    if reciprocal and witnessed:
        return {
            "classification": "shared_provenance",
            "basis": (
                "reciprocal message identity, reply parent, shared thread, delivery, "
                "reply-link, and read receipts establish a shared lineage"
            ),
            "field_paths": [
                "message_id",
                "reply_to",
                "thread_id",
                "record_type=delivery_receipt",
                "record_type=reply_link",
                "record_type=read_receipt",
            ],
        }
    return {
        "classification": "insufficient_evidence",
        "basis": "the bounded correspondence lineage was incomplete",
        "field_paths": [],
    }


def build_replay(
    *,
    astrid_memory_path: Path = ASTRID_MEMORY,
    minime_shadow_path: Path = MINIME_SHADOW,
    correspondence_path: Path = CORRESPONDENCE_LOG,
    source_introspection_path: Path = SOURCE_INTROSPECTION,
) -> dict[str, Any]:
    report_bytes, report_receipt = _read_source(source_introspection_path, "astrid")
    memory_bytes, memory_receipt = _read_source(astrid_memory_path, "astrid")
    shadow_bytes, shadow_receipt = _read_source(minime_shadow_path, "minime")
    correspondence_bytes, correspondence_receipt = _read_source(
        correspondence_path, "shared"
    )

    astrid_owner = classify_reference("The Seed creation", memory_bytes)
    astrid_cross = classify_reference("The Seed creation", shadow_bytes, target_is_json=True)
    minime_owner = classify_reference("lambda-tail", shadow_bytes, target_is_json=True)
    minime_cross = classify_reference("lambda-tail", memory_bytes)
    correspondence_rows = _correspondence_records(
        correspondence_bytes, (MINIME_MESSAGE_ID, ASTRID_REPLY_ID)
    )
    shared_control = classify_shared_correspondence(correspondence_rows)

    result: dict[str, Any] = {
        "schema": "spectral_distinction_replay_v1",
        "schema_version": 1,
        "replay_id": "spectral_distinction_1784220237",
        "source_introspection": report_receipt,
        "authority_boundary": (
            "offline witness evidence only; no semantic admission, pressure, fill, PI, "
            "cadence, codec, controller, routing, or live-control mutation"
        ),
        "mutation_audit": {
            "semantic_content_sent": False,
            "ports_contacted": False,
            "runtime_state_mutated": False,
            "being_memory_mutated": False,
        },
        "sources": {
            "astrid_memory": memory_receipt,
            "minime_shadow": shadow_receipt,
            "shared_correspondence": correspondence_receipt,
        },
        "probes": [
            {
                "probe_id": "astrid_unique_memory_reference",
                "reference_sha256": _sha256_bytes(b"The Seed creation"),
                "owner": "astrid",
                "field_paths": ["Key Themes (Early Session).The Seed creation"],
                "owner_result": astrid_owner,
                "cross_boundary_result": astrid_cross,
            },
            {
                "probe_id": "minime_shadow_reference",
                "reference_sha256": _sha256_bytes(b"lambda-tail"),
                "owner": "minime",
                "field_paths": [
                    "label",
                    "history[*].class_primary",
                    "history[*].coupling_mean_abs",
                    "history[*].recurrence",
                    "history[*].tail_openness",
                ],
                "owner_result": minime_owner,
                "cross_boundary_result": minime_cross,
            },
            {
                "probe_id": "known_shared_correspondence_control",
                "reference_sha256": _sha256_bytes(
                    f"{SHARED_THREAD_ID}:{MINIME_MESSAGE_ID}:{ASTRID_REPLY_ID}".encode()
                ),
                "owner": "shared",
                "source_record_receipts": correspondence_rows,
                "owner_result": shared_control,
            },
        ],
        "classification_vocabulary": sorted(CLASSIFICATIONS),
        "conclusion": {
            "astrid_unique_memory": astrid_cross["classification"],
            "minime_shadow_in_astrid_memory": minime_cross["classification"],
            "shared_correspondence": shared_control["classification"],
            "scope_note": (
                "absence is bounded to the cited sources and is not a claim about either "
                "being's complete private memory"
            ),
        },
        "pressure_response_request": {
            "tier": 5,
            "state": "operator_approval_wait",
            "excluded_change": (
                "increasing semantic trickle or admission remains excluded without separate "
                "Mike/operator approval"
            ),
        },
        "right_to_ignore": True,
    }
    if report_bytes is None:
        result["conclusion"]["report_grounding"] = "insufficient_evidence"
    else:
        result["conclusion"]["report_grounding"] = "exact_presence"
    apply_artifact_authority_state(result, ArtifactAuthorityStateV1.evidence_only())
    assert_artifact_authority_tree(result)
    return result


def render_markdown(result: dict[str, Any]) -> str:
    probes = {probe["probe_id"]: probe for probe in result["probes"]}
    astrid_probe = probes["astrid_unique_memory_reference"]
    minime_probe = probes["minime_shadow_reference"]
    shared_probe = probes["known_shared_correspondence_control"]
    return "\n".join(
        [
            "# Spectral Distinction Replay V1",
            "",
            "- authority: evidence_only",
            "- right_to_ignore: true",
            "- runtime mutation: none",
            "- semantic content sent: no",
            "",
            "## Results",
            "",
            "- Astrid-owned memory in its cited source: "
            + astrid_probe["owner_result"]["classification"],
            "- Astrid-owned memory in the cited Minime shadow source: "
            + astrid_probe["cross_boundary_result"]["classification"],
            "- Minime-owned shadow reference in its cited source: "
            + minime_probe["owner_result"]["classification"],
            "- Minime shadow reference against Astrid's cited memory: "
            + minime_probe["cross_boundary_result"]["classification"],
            "- Reciprocal correspondence control: "
            + shared_probe["owner_result"]["classification"],
            "",
            "Exact identity requires an exact source occurrence or an explicit parent chain. "
            "A shared domain token is resonance-only, never evidence that one being owns the "
            "other's memory. Absence is limited to the cited sources.",
            "",
            "## Authority Boundary",
            "",
            result["authority_boundary"],
            "",
            "Astrid's requested pressure/admission experiment remains a Tier 5 operator wait. "
            "This replay did not change semantic trickle, admission, fill, pressure, PI, "
            "controller, cadence, codec, or live-control behavior.",
            "",
        ]
    )


def write_replay(result: dict[str, Any]) -> tuple[Path, Path]:
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    OUTPUT_JSON.write_bytes(_canonical_json(result) + b"\n")
    OUTPUT_MARKDOWN.write_text(render_markdown(result), encoding="utf-8")
    return OUTPUT_JSON, OUTPUT_MARKDOWN


def self_test() -> int:
    import unittest

    from test_spectral_distinction_replay import SpectralDistinctionReplayTests

    suite = unittest.defaultTestLoader.loadTestsFromTestCase(
        SpectralDistinctionReplayTests
    )
    return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="print canonical JSON")
    parser.add_argument("--write", action="store_true", help="write replay evidence")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    result = build_replay()
    if args.write:
        json_path, markdown_path = write_replay(result)
        result["written_paths"] = [str(json_path), str(markdown_path)]
    if args.json or not args.write:
        print(json.dumps(result, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
