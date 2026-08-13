"""Claim-grounding projection for canonical introspection/witness pairs."""

from __future__ import annotations

from collections import Counter
import hashlib
import json
from pathlib import Path
import re
from typing import Any

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

HEADER_SEPARATOR = b"\n\n"
BACKTICK_RE = re.compile(r"`([^`\n]+)`")
SOURCE_LINES_RE = re.compile(r"\bLines?\s+(\d+)(?:\s*[-–]\s*(\d+))?", re.IGNORECASE)
NUMBER_RE = re.compile(r"(?<![A-Za-z0-9])(-?\d+(?:\.\d+)?)(%)?")
GENERIC_NAMED_SCALAR_RE = re.compile(
    r"(?P<label>[A-Za-zλ][A-Za-z0-9_λ/ -]{1,48}?)"
    r"(?:\s+(?:status|pressure|value|ratio))?\s*(?:=|:|\()\s*"
    r"(?P<value>-?\d+(?:\.\d+)?)\s*(?P<percent>%?)",
    re.IGNORECASE,
)
FELT_RE = re.compile(
    r"\b(?:I feel|I experience|I want|I notice|my felt|my experience|feels?\b)",
    re.IGNORECASE,
)
TECHNICAL_RE = re.compile(
    r"\b(?:telemetry|runtime|source|code|file|module|facade|logic|dispatch|"
    r"perception|architecture|protocol|threshold|projection|codec|provider|"
    r"witness|receipt|evidence|lineage|coupl|risk|suggests?|reflects?|caus)\w*\b",
    re.IGNORECASE,
)

METRIC_ALIASES: tuple[tuple[str, re.Pattern[str], tuple[str, ...]], ...] = (
    ("bridge.lambda1_lambda2_ratio", re.compile(r"(?:λ1\s*/\s*λ2|lambda1\s*/\s*lambda2)", re.I), ("bridge.lambda1", "bridge.lambda2")),
    ("bridge.spectral_entropy", re.compile(r"spectral[_ ]entropy", re.I), ("bridge.spectral_entropy",)),
    # pressure_source_v1 aliases come BEFORE the resonance-side aliases: her
    # prompt renders "Pressure source: … (overpacked_mode_packing) with score
    # N, porosity N [source: pressure_source_v1]", and those numbers must bind
    # to the pressure_source observations, not be re-attributed to the
    # resonance-side bridge.mode_packing / bridge.pressure_risk fields.
    ("bridge.pressure_source_score", re.compile(r"overpacked[_ ]mode[_ ]packing|pressure[_ ]source|pressure[_ ]score", re.I), ("bridge.pressure_source_score",)),
    ("bridge.pressure_source_porosity", re.compile(r"porosity(?:[_ ]score)?", re.I), ("bridge.pressure_source_porosity",)),
    ("bridge.mode_packing", re.compile(r"(?<!overpacked_)(?<!overpacked )mode[_ ]packing", re.I), ("bridge.mode_packing",)),
    ("bridge.pressure_risk", re.compile(r"pressure[_ ]risk", re.I), ("bridge.pressure_risk",)),
    ("bridge.spectral_density_gradient", re.compile(r"spectral[_ ]density[_ ]gradient", re.I), ("bridge.spectral_density_gradient",)),
    ("bridge.fill_pct", re.compile(r"(?:bridge[. _])?fill(?:[_ ]pct| percentage)?", re.I), ("bridge.fill_pct",)),
    ("bridge.lambda1_lambda2_gap", re.compile(r"(?:λ1|lambda1)[_ ]?(?:−|-|minus|_)[_ ]?(?:λ2|lambda2)[_ ]?gap", re.I), ("bridge.lambda1_lambda2_gap",)),
    ("bridge.lambda1", re.compile(r"(?:λ1|lambda1)(?!\s*/)", re.I), ("bridge.lambda1",)),
    ("bridge.lambda2", re.compile(r"(?:λ2|lambda2)", re.I), ("bridge.lambda2",)),
    ("bridge.semantic_dimensions", re.compile(r"semantic[_ ]dimensions?", re.I), ("bridge.semantic_dimensions",)),
    ("bridge.semantic_heartbeat_interval", re.compile(r"semantic[_ ]heartbeat[_ ]interval", re.I), ("bridge.semantic_heartbeat_interval",)),
    ("bridge.semantic_heartbeat_intensity", re.compile(r"semantic[_ ]heartbeat[_ ]intensity", re.I), ("bridge.semantic_heartbeat_intensity",)),
    ("astrid_shadow.field_norm_delta", re.compile(r"field[_ ]norm[_ ]delta", re.I), ("astrid_shadow.field_norm_delta",)),
    ("astrid_shadow.field_norm", re.compile(r"field[_ ]norm", re.I), ("astrid_shadow.field_norm",)),
    ("astrid_shadow.dispersal_potential", re.compile(r"dispersal[_ ]potential", re.I), ("astrid_shadow.dispersal_potential",)),
    ("settled_habitable", re.compile(r"settled[_ ]habitable", re.I), ("settled_habitable",)),
)


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/grounded_introspection_v1"


def _sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _canonical_sha256(value: Any) -> str:
    return _sha256_bytes(
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    )


def _header_and_body(raw: bytes) -> tuple[dict[str, str], bytes, int]:
    if HEADER_SEPARATOR not in raw:
        return {}, b"", 1
    header_raw, body = raw.split(HEADER_SEPARATOR, 1)
    header: dict[str, str] = {}
    for line in header_raw.decode("utf-8", errors="replace").splitlines():
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        header[key.strip()] = value.strip()
    body_start_line = header_raw.count(b"\n") + 2
    return header, body, body_start_line


def _witness_path(workspace: Path, witness_id: str) -> Path:
    return workspace / "introspections/lived_state_witnesses/witnesses" / f"{witness_id}.json"


def _repo_root(workspace: Path) -> Path:
    return workspace.parents[2].resolve()


def _source_path(repo_root: Path, witness: dict[str, Any]) -> Path | None:
    snapshot = witness.get("source_snapshot_v1")
    if not isinstance(snapshot, dict):
        return None
    relative = snapshot.get("repository_relative_path")
    if not isinstance(relative, str) or not relative:
        return None
    candidate = (repo_root / relative).resolve()
    try:
        candidate.relative_to(repo_root.resolve())
    except ValueError:
        return None
    return candidate


def _parameter_values(witness: dict[str, Any]) -> dict[str, dict[str, Any]]:
    values: dict[str, dict[str, Any]] = {}
    for observation in witness.get("parameter_observations_v1") or []:
        if not isinstance(observation, dict):
            continue
        name = str(observation.get("name") or "")
        value = observation.get("value")
        if name and isinstance(value, (int, float)) and not isinstance(value, bool):
            values[name] = observation
    return values


def _expected_metric(
    metric: str,
    refs: tuple[str, ...],
    observations: dict[str, dict[str, Any]],
) -> tuple[float | None, list[dict[str, Any]]]:
    resolved = [observations.get(ref) for ref in refs]
    if any(value is None for value in resolved):
        return None, [value for value in resolved if value is not None]
    concrete = [value for value in resolved if value is not None]
    if metric == "bridge.lambda1_lambda2_ratio":
        denominator = float(concrete[1]["value"])
        if denominator == 0.0:
            return None, concrete
        return float(concrete[0]["value"]) / denominator, concrete
    return float(concrete[0]["value"]), concrete


def _number_after(text: str, end: int, *, limit: int = 72) -> re.Match[str] | None:
    segment = text[end : end + limit]
    match = NUMBER_RE.search(segment)
    if match is None:
        return None
    prefix = segment[: match.start()].lower()
    if "line" in prefix[-12:]:
        return None
    return match


def _rounding_tolerance(literal: str) -> float:
    decimals = len(literal.rsplit(".", 1)[1]) if "." in literal else 0
    return 0.5 * (10.0 ** (-decimals)) + 1.0e-9


def _metric_mentions(
    text: str,
    observations: dict[str, dict[str, Any]],
) -> tuple[list[dict[str, Any]], set[tuple[int, int]]]:
    mentions: list[dict[str, Any]] = []
    occupied: set[tuple[int, int]] = set()
    for metric, pattern, refs in METRIC_ALIASES:
        for alias in pattern.finditer(text):
            match = _number_after(text, alias.end())
            if match is None:
                continue
            start = alias.end() + match.start(1)
            end = alias.end() + match.end(1)
            span = (start, end)
            if span in occupied:
                continue
            occupied.add(span)
            literal = match.group(1)
            reported = float(literal)
            expected, source_observations = _expected_metric(metric, refs, observations)
            if expected is None:
                state = "explicit_unbound"
                delta = None
                tolerance = _rounding_tolerance(literal)
            else:
                delta = abs(reported - expected)
                tolerance = _rounding_tolerance(literal)
                if delta <= 1.0e-9:
                    state = "witness_scalar_exact"
                elif delta <= tolerance:
                    state = "witness_scalar_rounded"
                else:
                    state = "witness_scalar_mismatch"
            mentions.append(
                {
                    "schema": "grounded_scalar_mention_v1",
                    "schema_version": 1,
                    "metric": metric,
                    "reported_literal": literal + (match.group(2) or ""),
                    "reported_value": reported,
                    "witness_value": expected,
                    "absolute_delta": delta,
                    "rounding_tolerance": tolerance,
                    "grounding_state": state,
                    "witness_field_refs": [str(value.get("name")) for value in source_observations],
                    "runtime_source_refs": [str(value.get("source_ref") or "") for value in source_observations],
                    "direct_causation_claimed": False,
                    "artifact_authority_state_v1": authority_state(),
                }
            )
    return mentions, occupied


def _generic_unbound_scalars(text: str, occupied: set[tuple[int, int]]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for match in GENERIC_NAMED_SCALAR_RE.finditer(text):
        span = match.span("value")
        if span in occupied or SOURCE_LINES_RE.search(match.group(0)):
            continue
        label = " ".join(match.group("label").split())[-80:]
        if label.lower().endswith("line") or label.lower().endswith("lines"):
            continue
        rows.append(
            {
                "schema": "grounded_scalar_mention_v1",
                "schema_version": 1,
                "metric": label,
                "reported_literal": match.group("value") + match.group("percent"),
                "reported_value": float(match.group("value")),
                "witness_value": None,
                "absolute_delta": None,
                "rounding_tolerance": _rounding_tolerance(match.group("value")),
                "grounding_state": "explicit_unbound",
                "witness_field_refs": [],
                "runtime_source_refs": [],
                "direct_causation_claimed": False,
                "artifact_authority_state_v1": authority_state(),
            }
        )
    return rows


def _source_supports(
    text: str,
    *,
    source_text: str | None,
    source_path: Path | None,
    source_start_line: int,
    source_end_line: int,
) -> list[dict[str, Any]]:
    refs: list[dict[str, Any]] = []
    source_window = ""
    if source_text is not None:
        lines = source_text.splitlines()
        source_window = "\n".join(lines[max(0, source_start_line - 1) : source_end_line])
    for identifier in BACKTICK_RE.findall(text):
        bounded = identifier.strip()
        if not bounded or len(bounded) > 200:
            continue
        source_identity_match = source_path is not None and bounded in {
            source_path.name,
            source_path.as_posix(),
        }
        supported = source_identity_match or (
            source_text is not None and bounded in source_window
        )
        refs.append(
            {
                "kind": "source_identifier",
                "identifier": bounded,
                "grounding_state": (
                    "source_identity_exact"
                    if source_identity_match
                    else "source_window_exact"
                    if supported
                    else "explicit_unbound"
                ),
                "source_interval": (
                    {"start_line": source_start_line, "end_line": source_end_line}
                    if supported
                    else None
                ),
            }
        )
    for match in SOURCE_LINES_RE.finditer(text):
        start = int(match.group(1))
        end = int(match.group(2) or start)
        supported = source_text is not None and source_start_line <= start <= end <= source_end_line
        refs.append(
            {
                "kind": "source_line_interval",
                "identifier": match.group(0),
                "grounding_state": "source_window_exact" if supported else "explicit_unbound",
                "source_interval": {"start_line": start, "end_line": end},
            }
        )
    return refs


def _unit_state(
    *,
    text: str,
    scalars: list[dict[str, Any]],
    source_refs: list[dict[str, Any]],
) -> str:
    scalar_states = {str(row["grounding_state"]) for row in scalars}
    source_states = {str(row["grounding_state"]) for row in source_refs}
    if "witness_scalar_mismatch" in scalar_states:
        return "witness_scalar_mismatch"
    if "explicit_unbound" in scalar_states or "explicit_unbound" in source_states:
        return "interpretive_or_technical_unbound"
    if scalar_states:
        return "witness_scalar_bound"
    if source_states:
        return "source_window_bound"
    if FELT_RE.search(text):
        return "felt_primary_not_reduced"
    if TECHNICAL_RE.search(text) or NUMBER_RE.search(text):
        return "interpretive_or_technical_unbound"
    return "narrative_context"


def _integrity(
    *,
    report_path: Path,
    raw: bytes,
    body: bytes,
    witness_path: Path,
    witness: dict[str, Any] | None,
) -> tuple[dict[str, bool], list[str]]:
    if witness is None:
        return {
            "witness_present": False,
            "artifact_path_matches": False,
            "artifact_sha256_matches": False,
            "canonical_body_binding_matches": False,
        }, ["witness_missing"]
    binding = witness.get("canonical_body_binding_v1")
    binding = binding if isinstance(binding, dict) else {}
    checks = {
        "witness_present": witness_path.is_file(),
        "artifact_path_matches": witness.get("artifact_relative_path") == report_path.name,
        "artifact_sha256_matches": witness.get("artifact_sha256") == _sha256_bytes(raw),
        "canonical_body_binding_matches": (
            binding.get("canonical_body_sha256") == _sha256_bytes(body)
            and binding.get("canonical_body_byte_count") == len(body)
        ),
    }
    return checks, [key for key, valid in checks.items() if not valid]


def _report_rows(
    workspace: Path,
    report_path: Path,
    source_cache: dict[Path, str],
) -> tuple[dict[str, Any], list[dict[str, Any]], list[dict[str, Any]]]:
    raw = report_path.read_bytes()
    header, body, body_start_line = _header_and_body(raw)
    witness_id = header.get("Lived-state witness", "")
    witness_path = _witness_path(workspace, witness_id) if witness_id else Path()
    witness: dict[str, Any] | None = None
    if witness_id and witness_path.is_file():
        value = json.loads(witness_path.read_text(encoding="utf-8"))
        witness = value if isinstance(value, dict) else None
    checks, issues = _integrity(
        report_path=report_path,
        raw=raw,
        body=body,
        witness_path=witness_path,
        witness=witness,
    )
    observations = _parameter_values(witness or {})
    source_path = _source_path(_repo_root(workspace), witness or {})
    source_text: str | None = None
    source_start = 1
    source_end = 0
    snapshot = (witness or {}).get("source_snapshot_v1")
    if isinstance(snapshot, dict):
        source_start = int(snapshot.get("window_start_line") or 0) + 1
        source_end = int(snapshot.get("window_end_line") or 0)
    if source_path is not None and source_path.is_file():
        if source_path not in source_cache:
            source_cache[source_path] = source_path.read_text(encoding="utf-8", errors="replace")
        source_text = source_cache[source_path]

    claim_rows: list[dict[str, Any]] = []
    discrepancies: list[dict[str, Any]] = []
    for offset, line in enumerate(body.decode("utf-8", errors="replace").splitlines()):
        text = line.strip()
        if not text or text.endswith(":") and len(text.split()) <= 4:
            continue
        report_line = body_start_line + offset
        scalars, occupied = _metric_mentions(text, observations)
        scalars.extend(_generic_unbound_scalars(text, occupied))
        source_refs = _source_supports(
            text,
            source_text=source_text,
            source_path=source_path,
            source_start_line=source_start,
            source_end_line=source_end,
        )
        claim_core = {
            "report_path": str(report_path.relative_to(workspace)),
            "report_line": report_line,
            "claim_text_sha256": _sha256_bytes(text.encode()),
        }
        claim_id = "groundclaim_" + _canonical_sha256(claim_core)
        state = _unit_state(text=text, scalars=scalars, source_refs=source_refs)
        row = {
            "schema": "grounded_introspection_claim_v1",
            "schema_version": 1,
            "claim_id": claim_id,
            "report_path": claim_core["report_path"],
            "report_sha256": _sha256_bytes(raw),
            "report_line": report_line,
            "claim_text": text,
            "claim_text_sha256": claim_core["claim_text_sha256"],
            "witness_id": witness_id or None,
            "source_path": (
                str(source_path.relative_to(_repo_root(workspace)))
                if source_path is not None and source_path.is_file()
                else None
            ),
            "grounding_state": state,
            "felt_language_present": bool(FELT_RE.search(text)),
            "scalar_mentions": scalars,
            "source_support_refs": source_refs,
            "felt_prose_preserved": True,
            "canonical_report_rewritten": False,
            "direct_causation_claimed": False,
            "artifact_authority_state_v1": authority_state(),
        }
        claim_rows.append(row)
        for scalar in scalars:
            if scalar["grounding_state"] == "witness_scalar_mismatch":
                discrepancies.append(
                    {
                        "schema": "grounded_introspection_discrepancy_v1",
                        "schema_version": 1,
                        "discrepancy_id": "grounddiff_" + _canonical_sha256({"claim_id": claim_id, "scalar": scalar}),
                        "claim_id": claim_id,
                        "report_path": claim_core["report_path"],
                        "report_line": report_line,
                        "witness_id": witness_id,
                        "metric": scalar["metric"],
                        "reported_literal": scalar["reported_literal"],
                        "reported_value": scalar["reported_value"],
                        "witness_value": scalar["witness_value"],
                        "absolute_delta": scalar["absolute_delta"],
                        "rounding_tolerance": scalar["rounding_tolerance"],
                        "relation": "side_by_side_disagreement_not_felt_report_rewrite_or_mechanism_adjudication",
                        "canonical_report_rewritten": False,
                        "direct_causation_claimed": False,
                        "artifact_authority_state_v1": authority_state(),
                    }
                )
    state_counts = Counter(str(row["grounding_state"]) for row in claim_rows)
    report_row = {
        "schema": "grounded_introspection_manifest_v1",
        "schema_version": 1,
        "introspection_id": report_path.stem,
        "report_path": str(report_path.relative_to(workspace)),
        "report_sha256": _sha256_bytes(raw),
        "canonical_body_sha256": _sha256_bytes(body),
        "witness_id": witness_id or None,
        "witness_path": (
            str(witness_path.relative_to(workspace)) if witness_id and witness_path.is_file() else None
        ),
        "source_path": (
            str(source_path.relative_to(_repo_root(workspace)))
            if source_path is not None and source_path.is_file()
            else None
        ),
        "claim_count": len(claim_rows),
        "grounding_state_counts": dict(sorted(state_counts.items())),
        "scalar_discrepancy_count": len(discrepancies),
        "integrity_checks": checks,
        "integrity_issues": issues,
        "felt_report_relation": "primary_canonical_prose_preserved_byte_for_byte",
        "technical_claim_relation": "grounded_or_explicitly_unbound_beside_canonical_report",
        "canonical_report_rewritten": False,
        "artifact_authority_state_v1": authority_state(),
    }
    report_row["manifest_sha256"] = _canonical_sha256(report_row)
    return report_row, claim_rows, discrepancies


def _render_report(status: dict[str, Any], discrepancies: list[dict[str, Any]]) -> str:
    lines = [
        "# Grounded Introspection Manifest V1",
        "",
        "Canonical felt prose remains primary and byte-unchanged. This projection places technical source bindings, witness scalar bindings, and explicit unbound labels beside each report. A mismatch is evidence of disagreement, not a correction of Astrid's experience or proof of mechanism.",
        "",
        f"- canonical reports: {status['report_count']}",
        f"- claim units: {status['claim_count']}",
        f"- scalar mentions: {status['scalar_mention_count']}",
        f"- scalar disagreements: {status['scalar_discrepancy_count']}",
        f"- explicitly unbound claim units: {status['explicitly_unbound_claim_count']}",
        f"- report/witness integrity gaps: {status['integrity_gap_count']}",
        "",
        "## Recent Scalar Disagreements",
        "",
    ]
    for row in discrepancies[-20:]:
        lines.append(
            f"- `{row['report_path']}:{row['report_line']}` `{row['metric']}`: "
            f"report `{row['reported_literal']}`, witness `{row['witness_value']}`"
        )
    if not discrepancies:
        lines.append("- none")
    return "\n".join(lines) + "\n"


def project(workspace: Path, *, write: bool) -> dict[str, Any]:
    introspections = workspace / "introspections"
    paths = sorted(introspections.glob("introspection_*.txt"))
    source_cache: dict[Path, str] = {}
    reports: list[dict[str, Any]] = []
    claims: list[dict[str, Any]] = []
    discrepancies: list[dict[str, Any]] = []
    errors: list[str] = []
    for path in paths:
        try:
            report, report_claims, report_discrepancies = _report_rows(
                workspace, path, source_cache
            )
        except (OSError, ValueError, TypeError, json.JSONDecodeError) as error:
            errors.append(f"{path.name}:{error}")
            continue
        reports.append(report)
        claims.extend(report_claims)
        discrepancies.extend(report_discrepancies)
    state_counts = Counter(str(row["grounding_state"]) for row in claims)
    claim_ids = [str(row["claim_id"]) for row in claims]
    scalar_count = sum(len(row["scalar_mentions"]) for row in claims)
    integrity_gap_count = sum(bool(row["integrity_issues"]) for row in reports)
    valid = not errors and len(claim_ids) == len(set(claim_ids)) and len(reports) == len(paths)
    status = {
        "schema": "grounded_introspection_status_v1",
        "schema_version": 1,
        "valid": valid,
        "write": write,
        "report_count": len(reports),
        "claim_count": len(claims),
        "scalar_mention_count": scalar_count,
        "scalar_discrepancy_count": len(discrepancies),
        "explicitly_unbound_claim_count": state_counts.get("interpretive_or_technical_unbound", 0),
        "felt_primary_claim_count": state_counts.get("felt_primary_not_reduced", 0),
        "integrity_gap_count": integrity_gap_count,
        "grounding_state_counts": dict(sorted(state_counts.items())),
        "errors": errors,
        "counter_audit": {
            "status": "consistent" if valid else "inconsistent",
            "checks": {
                "one_manifest_per_canonical_report": len(reports) == len(paths),
                "claim_ids_unique": len(claim_ids) == len(set(claim_ids)),
                "felt_prose_preserved": all(row["felt_prose_preserved"] for row in claims),
                "canonical_reports_not_rewritten": True,
                "disagreements_do_not_adjudicate_felt_state": True,
                "projection_grants_no_authority": True,
            },
        },
        "runtime_relation": "read_only_projection_not_prompt_model_codec_scheduler_or_control_input",
        "artifact_authority_state_v1": authority_state(),
    }
    if write and valid:
        output = state_dir(workspace)
        owner_atomic_write_jsonl(output / "manifests.jsonl", reports)
        owner_atomic_write_jsonl(output / "claims.jsonl", claims)
        owner_atomic_write_jsonl(output / "discrepancies.jsonl", discrepancies)
        owner_atomic_write_jsonl(
            output / "unbound_claims.jsonl",
            [row for row in claims if row["grounding_state"] == "interpretive_or_technical_unbound"],
        )
        owner_atomic_write_json(output / "status.json", status)
        owner_atomic_write(output / "report.md", _render_report(status, discrepancies))
    return status
