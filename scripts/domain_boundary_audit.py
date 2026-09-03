#!/usr/bin/env python3
"""Enforce Spectral Bridge facade, dependency, and large-file ratchets."""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import time
import tomllib
from typing import Any

try:
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.projection_receipt import projector_receipt

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

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "capsules/spectral-bridge/domain_boundaries_v1.toml"
DEFAULT_WORKSPACE = ROOT / "capsules/spectral-bridge/workspace"


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/domain_boundary_audit_v1"


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _line_count(path: Path) -> int:
    return len(path.read_text(encoding="utf-8", errors="replace").splitlines())


_FN_SIGNATURE_RE = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?"
    r"(?:(?:default|const|async|unsafe|extern(?:\s+\"[^\"]*\")?)\s+)*"
    r"fn\s+([A-Za-z0-9_]+)"
)


def _unique_fn_signature_count(path: Path) -> int:
    names: set[str] = set()
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        match = _FN_SIGNATURE_RE.match(line)
        if match:
            names.add(match.group(1))
    return len(names)


_DECISION_POINT_RE = re.compile(r"\b(?:if|match|while|for)\b|&&|\|\||\?")


def _structural_metrics(path: Path) -> dict[str, int]:
    """Report-only complexity proxies that MOVE when logic is smuggled.

    The unique-fn-signature ceiling is structurally blind on the file it most
    needs to guard: `orchestration.rs` is ~5,000 lines holding SIX fn
    declarations, so unbounded logic can land inside `spawn_autonomous_loop`
    without ever changing the count. Astrid named exactly that file, three
    reads running, as the place "logic smuggling" would hide.

    These three measures answer that. They are deliberately UNGATED — no
    manifest ceiling, no violation kind, `valid` untouched — so the numbers
    exist for review before anyone decides where a ceiling belongs.

    Approximations, not a parser: `max_fn_lines` measures the span between
    consecutive fn declarations (the last one runs to EOF), `decision_points`
    counts branch/short-circuit tokens including any inside comments and
    strings, and `max_nesting_depth` is a running brace depth.
    """
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    starts = [i for i, line in enumerate(lines) if _FN_SIGNATURE_RE.match(line)]
    spans = [b - a for a, b in zip(starts, starts[1:])] if len(starts) > 1 else []
    if starts:
        spans.append(len(lines) - starts[-1])

    depth = 0
    max_depth = 0
    decision_points = 0
    for line in lines:
        decision_points += len(_DECISION_POINT_RE.findall(line))
        for char in line:
            if char == "{":
                depth += 1
                max_depth = max(max_depth, depth)
            elif char == "}":
                depth = max(0, depth - 1)
    return {
        "max_fn_lines": max(spans) if spans else 0,
        "decision_points": decision_points,
        "max_nesting_depth": max_depth,
    }


def _is_test_path(relative: str, markers: list[str]) -> bool:
    value = f"/{relative}"
    return any(marker in value for marker in markers)


def _violation(kind: str, path: str, detail: str) -> dict[str, Any]:
    return {
        "schema": "domain_boundary_violation_v1",
        "schema_version": 1,
        "kind": kind,
        "path": path,
        "detail": detail,
        "artifact_authority_state_v1": authority_state(),
    }


def audit(repo_root: Path, manifest_path: Path = DEFAULT_MANIFEST) -> tuple[dict[str, Any], list[dict[str, Any]], list[dict[str, Any]]]:
    repo_root = repo_root.resolve()
    manifest_path = manifest_path.resolve()
    manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    bridge_root = repo_root / str(manifest["source_root"])
    source_root = bridge_root / "src"
    baseline_path = repo_root / str(manifest["legacy_large_file_baseline"])
    documentation_path = repo_root / str(manifest["documentation"])
    baseline = json.loads(baseline_path.read_text(encoding="utf-8"))
    baseline_files = {
        str(path): int(lines) for path, lines in (baseline.get("files") or {}).items()
    }
    threshold = int(manifest["large_file_review_threshold"])
    facade_limit = int(manifest["facade_line_limit"])
    markers = [str(value) for value in manifest.get("test_path_markers") or []]
    exceptions = {
        str(row["path"]): int(row["maximum_lines"])
        for row in manifest.get("cohesion_exceptions") or []
    }
    signature_ceilings = {
        str(row["path"]): int(row["maximum_unique_fn_signatures"])
        for row in manifest.get("cohesion_exceptions") or []
        if "maximum_unique_fn_signatures" in row
    }
    documentation = documentation_path.read_text(encoding="utf-8")
    violations: list[dict[str, Any]] = []

    for facade in manifest.get("stable_facades") or []:
        relative = str(facade["path"])
        target = str(facade["canonical_target"])
        path = bridge_root / relative
        if not path.is_file():
            violations.append(_violation("facade_missing", relative, "stable facade file is missing"))
            continue
        lines = _line_count(path)
        if lines > facade_limit:
            violations.append(_violation("facade_line_limit", relative, f"{lines}>{facade_limit}"))
        text = path.read_text(encoding="utf-8")
        if f'#[path = "{target}"]' not in text:
            violations.append(_violation("facade_target_drift", relative, f"expected canonical target {target}"))
        if "pub use" not in text:
            violations.append(_violation("facade_reexport_missing", relative, "stable facade must re-export its canonical target"))

    large_rows: list[dict[str, Any]] = []
    current_large: set[str] = set()
    for path in sorted(source_root.rglob("*.rs")):
        relative = str(path.relative_to(bridge_root))
        if _is_test_path(relative, markers):
            continue
        lines = _line_count(path)
        if lines <= threshold:
            continue
        current_large.add(relative)
        classification = "documented_cohesion_exception" if relative in exceptions else "legacy_review_debt"
        allowed = exceptions.get(relative, baseline_files.get(relative))
        large_rows.append(
            {
                "schema": "domain_boundary_large_file_v1",
                "schema_version": 1,
                "path": relative,
                "line_count": lines,
                "classification": classification,
                "maximum_lines": allowed,
                "growth_remaining": None if allowed is None else allowed - lines,
                "artifact_authority_state_v1": authority_state(),
            }
        )
        if allowed is None:
            violations.append(_violation("new_large_production_file", relative, f"{lines} lines and absent from ratchet baseline"))
        elif lines > allowed:
            violations.append(_violation("large_file_growth", relative, f"{lines}>{allowed}"))

    for relative in exceptions:
        if relative not in documentation:
            violations.append(_violation("undocumented_cohesion_exception", relative, "exception path absent from DOMAIN_BOUNDARIES.md"))
    for relative in baseline_files:
        path = bridge_root / relative
        if not path.is_file():
            continue
        lines = _line_count(path)
        if lines > threshold and relative not in current_large:
            violations.append(_violation("large_file_scan_gap", relative, "baseline file escaped current scan"))

    # Report-only complexity measures for every documented exception, whether
    # or not it carries a signature ceiling. Ungated by design (see
    # _structural_metrics): they surface the smuggling the signature count
    # cannot see, without asserting where a ceiling belongs.
    exception_structural_metrics: dict[str, dict[str, int]] = {}
    for relative in sorted(exceptions):
        path = bridge_root / relative
        if path.is_file():
            exception_structural_metrics[relative] = _structural_metrics(path)

    exception_signature_counts: dict[str, dict[str, int]] = {}
    for relative, signature_ceiling in signature_ceilings.items():
        path = bridge_root / relative
        if not path.is_file():
            continue
        signature_count = _unique_fn_signature_count(path)
        exception_signature_counts[relative] = {
            "unique_fn_signatures": signature_count,
            "maximum_unique_fn_signatures": signature_ceiling,
        }
        if signature_count > signature_ceiling:
            violations.append(
                _violation(
                    "exception_signature_growth",
                    relative,
                    f"{signature_count}>{signature_ceiling}",
                )
            )

    forbidden_match_count = 0
    for edge in manifest.get("forbidden_edges") or []:
        edge_id = str(edge["edge_id"])
        for pattern in edge.get("from_globs") or []:
            for path in sorted(bridge_root.glob(str(pattern))):
                if not path.is_file():
                    continue
                text = path.read_text(encoding="utf-8", errors="replace")
                relative = str(path.relative_to(bridge_root))
                for symbol in edge.get("forbidden_symbols") or []:
                    if str(symbol) in text:
                        forbidden_match_count += 1
                        violations.append(
                            _violation(
                                "forbidden_dependency_edge",
                                relative,
                                f"{edge_id}:{symbol}",
                            )
                        )

    kinds = Counter(str(row["kind"]) for row in violations)
    status = {
        "schema": "domain_boundary_audit_status_v1",
        "schema_version": 1,
        "valid": not violations,
        "manifest_sha256": _sha256(manifest_path),
        "baseline_sha256": _sha256(baseline_path),
        "documentation_sha256": _sha256(documentation_path),
        "stable_facade_count": len(manifest.get("stable_facades") or []),
        "documented_cohesion_exception_count": len(exceptions),
        "exception_signature_ceiling_count": len(signature_ceilings),
        "exception_signature_counts": exception_signature_counts,
        "exception_structural_metrics": exception_structural_metrics,
        "legacy_large_file_count": len(large_rows),
        "unlisted_legacy_review_debt_count": sum(row["classification"] == "legacy_review_debt" for row in large_rows),
        "resolved_large_file_debt_count": len(set(baseline_files) - current_large),
        "forbidden_edge_match_count": forbidden_match_count,
        "violation_count": len(violations),
        "violation_kind_counts": dict(sorted(kinds.items())),
        "counter_audit": {
            "status": "consistent" if not violations else "inconsistent",
            "checks": {
                "stable_facades_point_to_declared_targets": not any(row["kind"] == "facade_target_drift" for row in violations),
                "new_large_production_files_rejected": not any(row["kind"] == "new_large_production_file" for row in violations),
                "legacy_large_files_cannot_grow": not any(row["kind"] == "large_file_growth" for row in violations),
                "documented_exception_ceilings_hold": not any(row["kind"] == "large_file_growth" and row["path"] in exceptions for row in violations),
                "documented_exception_signature_ceilings_hold": not any(row["kind"] == "exception_signature_growth" for row in violations),
                "forbidden_dependency_edges_absent": forbidden_match_count == 0,
                "audit_grants_no_authority": True,
            },
        },
        "runtime_relation": "ci_and_steward_evidence_only_not_runtime_routing_or_control",
        "artifact_authority_state_v1": authority_state(),
    }
    return status, large_rows, violations


def _report(status: dict[str, Any]) -> str:
    return "\n".join(
        [
            "# Executable Domain Boundary Audit V1",
            "",
            f"- valid: {str(status['valid']).lower()}",
            f"- stable facades: {status['stable_facade_count']}",
            f"- documented cohesion exceptions: {status['documented_cohesion_exception_count']}",
            f"- exception signature ceilings: {status.get('exception_signature_ceiling_count', 0)}",
            f"- large production files under ratchet: {status['legacy_large_file_count']}",
            f"- additional legacy review debt: {status['unlisted_legacy_review_debt_count']}",
            f"- resolved large-file debt: {status['resolved_large_file_debt_count']}",
            f"- forbidden edge matches: {status['forbidden_edge_match_count']}",
            f"- violations: {status['violation_count']}",
            "",
            "Existing large files remain visible review debt. The baseline prevents new large files and growth; it does not declare the debt healthy or exempt it from future extraction.",
            "",
        ]
    )


def run(repo_root: Path, workspace: Path, manifest_path: Path, *, write: bool) -> dict[str, Any]:
    status, large_rows, violations = audit(repo_root, manifest_path)
    status["write"] = write
    if write:
        output = state_dir(workspace)
        owner_atomic_write_json(output / "status.json", status)
        owner_atomic_write_jsonl(output / "large_files.jsonl", large_rows)
        owner_atomic_write_jsonl(output / "violations.jsonl", violations)
        owner_atomic_write(output / "report.md", _report(status))
    return status


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("command", choices=("project", "verify", "report"))
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--receipt-json", action="store_true")
    args = parser.parse_args(argv)
    started = time.monotonic()
    workspace = args.workspace.resolve()
    if args.command == "report":
        path = state_dir(workspace) / "status.json"
        value = json.loads(path.read_text()) if path.is_file() else {"valid": False, "error": "status_missing"}
    else:
        status = run(
            args.repo_root.resolve(),
            workspace,
            args.manifest.resolve(),
            write=args.command == "project" and args.write,
        )
        value = (
            projector_receipt(
                "domain_boundary_audit",
                status,
                {
                    "status.json": state_dir(workspace) / "status.json",
                    "large_files.jsonl": state_dir(workspace) / "large_files.jsonl",
                    "violations.jsonl": state_dir(workspace) / "violations.jsonl",
                    "report.md": state_dir(workspace) / "report.md",
                },
                started_monotonic=started,
            )
            if args.receipt_json
            else status
        )
    print(json.dumps(value, indent=2, sort_keys=True))
    return 0 if value.get("valid", True) is not False else 1


if __name__ == "__main__":
    raise SystemExit(main())
