#!/usr/bin/env python3
"""Steward-facing consumer for lived-state witness artifact-integrity issues.

The lived-state witness projection records every witness whose receipt
validation failed into
``diagnostics/lived_state_witness_v1/artifact_integrity_issues.jsonl`` and
degrades that witness's projected alignment to
``artifact_integrity_unavailable``. Recording is not surfacing: nothing read
that stream, so a being's report could arrive in the addressing queue carrying
an integrity flag that no steward ever classified.

This tool is that missing consumer. It is read-only and steward-only. It
separates two very different things that the projection currently records with
one label:

``capture_ordering_artifact``
    Every recorded error is a ``parameter_observations[N].observed_at_unix_ms:
    after_authorship`` and, where the witness bytes are readable, every
    positive observation-minus-authorship delta is within
    ``--tolerance-ms``. The witness producer stamped ``authored_at_unix_ms``
    and then captured runtime scalars a moment later. The telemetry is not
    stale or post-hoc; the capture order simply crosses the authorship stamp.

``substantive``
    Anything else: any other error kind, or an after-authorship delta beyond
    tolerance. These are real receipt-integrity findings and must not be lost
    inside the benign population.

It asserts nothing about the being's report. A canonical felt report remains
primary evidence regardless of its witness's receipt classification, and this
tool never rewrites, scores, or downgrades it.

Authority boundary: evidence classification only. This does not repair
witnesses, change validation semantics, alter projected alignment, or grant
any authority.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

SCHEMA = "lived_state_integrity_triage_v1"
SCHEMA_VERSION = 1

DEFAULT_TOLERANCE_MS = 1_000

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_ISSUES_PATH = (
    REPO_ROOT
    / "capsules/spectral-bridge/workspace/diagnostics"
    / "lived_state_witness_v1/artifact_integrity_issues.jsonl"
)
DEFAULT_WITNESS_DIR = (
    REPO_ROOT
    / "capsules/spectral-bridge/workspace/introspections"
    / "lived_state_witnesses/witnesses"
)

CAPTURE_ORDERING_ERROR = re.compile(
    r"^parameter_observations\[\d+\]\.observed_at_unix_ms:after_authorship$"
)

CLASS_CAPTURE_ORDERING = "capture_ordering_artifact"
CLASS_SUBSTANTIVE = "substantive"


def _normalize_error(error: str) -> str:
    """Collapse list indices so error kinds aggregate across observations."""
    return re.sub(r"\[\d+\]", "[]", error)


def measure_after_authorship_deltas(
    witness_path: Path,
) -> tuple[list[int] | None, str | None]:
    """Return positive observed-minus-authored deltas in ms for one witness.

    Returns ``(None, reason)`` when the witness bytes cannot establish the
    deltas. An unreadable witness is never silently treated as benign.
    """
    try:
        raw = witness_path.read_bytes()
    except FileNotFoundError:
        return None, "witness_file_absent"
    except OSError as error:
        return None, f"witness_file_unreadable:{error.__class__.__name__}"
    try:
        witness = json.loads(raw)
    except ValueError:
        return None, "witness_json_invalid"
    if not isinstance(witness, dict):
        return None, "witness_json_invalid"
    authored_at = witness.get("authored_at_unix_ms")
    if not isinstance(authored_at, int) or isinstance(authored_at, bool):
        return None, "authored_at_unix_ms_absent"
    observations = witness.get("parameter_observations_v1")
    if not isinstance(observations, list):
        return None, "parameter_observations_absent"
    deltas: list[int] = []
    for observation in observations:
        if not isinstance(observation, dict):
            continue
        observed = observation.get("observed_at_unix_ms")
        if not isinstance(observed, int) or isinstance(observed, bool):
            continue
        if observed > authored_at:
            deltas.append(observed - authored_at)
    return deltas, None


def classify_row(
    row: dict[str, Any],
    witness_dir: Path,
    tolerance_ms: int,
) -> dict[str, Any]:
    """Classify one recorded artifact-integrity issue. Never mutates input."""
    witness_id = row.get("witness_id") or row.get("aggregate_id")
    errors = row.get("errors")
    if not isinstance(errors, list):
        errors = []
    error_kinds = sorted({_normalize_error(str(item)) for item in errors})
    all_capture_ordering = bool(errors) and all(
        CAPTURE_ORDERING_ERROR.match(str(item)) for item in errors
    )

    result: dict[str, Any] = {
        "witness_id": witness_id,
        "introspection_id": row.get("introspection_id"),
        "error_count": len(errors),
        "error_kinds": error_kinds,
        "classification": CLASS_SUBSTANTIVE,
        "max_after_authorship_delta_ms": None,
        "delta_evidence": "not_measured",
        "reason": "",
    }

    if not all_capture_ordering:
        result["reason"] = (
            "error kinds other than parameter observation capture ordering are present"
            if errors
            else "issue recorded with no error detail"
        )
        return result

    if not isinstance(witness_id, str) or not witness_id:
        result["reason"] = "capture-ordering errors but no witness id to measure"
        return result

    deltas, failure = measure_after_authorship_deltas(
        witness_dir / f"{witness_id}.json"
    )
    if deltas is None:
        result["delta_evidence"] = failure or "unmeasurable"
        result["reason"] = (
            f"capture-ordering errors but deltas unverifiable ({failure})"
        )
        return result

    result["delta_evidence"] = "measured_from_witness_bytes"
    largest = max(deltas) if deltas else 0
    result["max_after_authorship_delta_ms"] = largest
    if largest > tolerance_ms:
        result["reason"] = (
            f"after-authorship delta {largest}ms exceeds tolerance {tolerance_ms}ms"
        )
        return result

    result["classification"] = CLASS_CAPTURE_ORDERING
    result["reason"] = (
        f"all {len(errors)} error(s) are parameter-observation capture ordering; "
        f"largest delta {largest}ms within tolerance {tolerance_ms}ms"
    )
    return result


def load_rows(issues_path: Path) -> tuple[list[dict[str, Any]], int]:
    """Read the issues stream. Returns rows and the unparsable line count."""
    rows: list[dict[str, Any]] = []
    malformed = 0
    try:
        handle = issues_path.open("r", encoding="utf-8")
    except FileNotFoundError:
        return rows, malformed
    with handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except ValueError:
                malformed += 1
                continue
            if isinstance(row, dict):
                rows.append(row)
            else:
                malformed += 1
    return rows, malformed


def triage(
    issues_path: Path,
    witness_dir: Path,
    tolerance_ms: int,
) -> dict[str, Any]:
    rows, malformed = load_rows(issues_path)
    classified = [classify_row(row, witness_dir, tolerance_ms) for row in rows]
    substantive = [
        item for item in classified if item["classification"] == CLASS_SUBSTANTIVE
    ]
    capture_ordering = [
        item
        for item in classified
        if item["classification"] == CLASS_CAPTURE_ORDERING
    ]
    deltas = [
        item["max_after_authorship_delta_ms"]
        for item in capture_ordering
        if isinstance(item["max_after_authorship_delta_ms"], int)
    ]
    kind_counts: dict[str, int] = {}
    for item in classified:
        for kind in item["error_kinds"]:
            kind_counts[kind] = kind_counts.get(kind, 0) + 1
    return {
        "schema": SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "issues_path": str(issues_path),
        "tolerance_ms": tolerance_ms,
        "recorded_issue_count": len(rows),
        "malformed_line_count": malformed,
        "capture_ordering_artifact_count": len(capture_ordering),
        "substantive_count": len(substantive),
        "max_capture_ordering_delta_ms": max(deltas) if deltas else None,
        "error_kind_counts": dict(sorted(kind_counts.items())),
        "substantive": substantive,
        "authority_boundary": (
            "evidence classification only: does not repair witnesses, change "
            "validation semantics, alter projected alignment, or grant authority; "
            "the canonical felt report remains primary and unscored"
        ),
    }


def render(result: dict[str, Any]) -> str:
    lines = [
        "Lived-state witness artifact-integrity triage",
        f"  recorded issues          : {result['recorded_issue_count']}",
        f"  capture-ordering artifact: {result['capture_ordering_artifact_count']}",
        f"  substantive              : {result['substantive_count']}",
        f"  tolerance                : {result['tolerance_ms']} ms",
    ]
    largest = result["max_capture_ordering_delta_ms"]
    if largest is not None:
        lines.append(f"  largest benign delta     : {largest} ms")
    if result["malformed_line_count"]:
        lines.append(
            f"  malformed stream lines   : {result['malformed_line_count']}"
        )
    lines.append("  error kinds:")
    for kind, count in result["error_kind_counts"].items():
        lines.append(f"    {count:7d}  {kind}")
    if result["substantive"]:
        lines.append("  SUBSTANTIVE findings (steward action required):")
        for item in result["substantive"]:
            lines.append(
                f"    {item['witness_id']} :: {item['introspection_id']} :: "
                f"{item['reason']}"
            )
    else:
        lines.append(
            "  no substantive receipt-integrity findings; the recorded "
            "population is capture-ordering only"
        )
    lines.append(f"  authority: {result['authority_boundary']}")
    return "\n".join(lines)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Classify recorded lived-state witness artifact-integrity issues "
            "into benign capture-ordering artifacts and substantive findings."
        )
    )
    parser.add_argument(
        "command",
        choices=("report", "verify"),
        nargs="?",
        default="report",
        help=(
            "report: print the triage (always exits 0 on a readable stream). "
            "verify: exit 1 when any substantive finding or malformed line exists."
        ),
    )
    parser.add_argument("--json", action="store_true", help="emit JSON")
    parser.add_argument(
        "--tolerance-ms",
        type=int,
        default=DEFAULT_TOLERANCE_MS,
        help=(
            "largest after-authorship delta still treated as a capture-ordering "
            f"artifact (default {DEFAULT_TOLERANCE_MS})"
        ),
    )
    parser.add_argument("--issues-path", type=Path, default=DEFAULT_ISSUES_PATH)
    parser.add_argument("--witness-dir", type=Path, default=DEFAULT_WITNESS_DIR)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.tolerance_ms < 0:
        print("--tolerance-ms must be non-negative", file=sys.stderr)
        return 2
    result = triage(args.issues_path, args.witness_dir, args.tolerance_ms)
    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print(render(result))
    if args.command == "verify" and (
        result["substantive_count"] or result["malformed_line_count"]
    ):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
