#!/usr/bin/env python3
"""Schema-aware linter for experiential evidence boundaries."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import tempfile
import unittest
from typing import Any, Iterable

try:
    from evidence_store import EvidenceEventStore
    from experiential_systems.common import (
        FORBIDDEN_PROSE_KEYS,
        RecordValidationError,
        authority_state,
        owner_atomic_write_json,
        reject_private_content,
    )
    from projection_receipt import projector_receipt
except ModuleNotFoundError:
    from scripts.evidence_store import EvidenceEventStore
    from scripts.experiential_systems.common import (
        FORBIDDEN_PROSE_KEYS,
        RecordValidationError,
        authority_state,
        owner_atomic_write_json,
        reject_private_content,
    )
    from scripts.projection_receipt import projector_receipt

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_WORKSPACE = ROOT / "capsules/spectral-bridge/workspace"

RULES = {
    "authority_escalation": "Evidence records cannot grant approval, dispatch, or live control.",
    "felt_scoring": "Mechanical or numeric evidence cannot score felt content.",
    "causal_overclaim": "Timing, association, and correlation cannot establish causation.",
    "source_runtime_confusion": "Source-declared values cannot be represented as observed active state.",
    "missing_baseline": "Candidate comparison requires a validated baseline receipt.",
    "silence_as_agreement": "No response leaves felt review pending and cannot affirm or close.",
    "identity_without_receipts": "Exact identity requires process and deployment evidence.",
    "state_propagation": "Evidence cannot propagate closure, consent, authority, or felt state.",
    "prose_leakage": "Study evidence contains bounded metadata and scalar samples only.",
}

TRUE_FORBIDDEN = {
    "auto_approved": "authority_escalation",
    "edits_source_now": "authority_escalation",
    "grants_approval": "authority_escalation",
    "live_eligible_now": "authority_escalation",
    "live_control_authority": "authority_escalation",
    "causation_established": "causal_overclaim",
    "direct_causation_claimed": "causal_overclaim",
    "felt_content_scored": "felt_scoring",
    "felt_outcome_inferred": "felt_scoring",
    "scalar_felt_dissimilarity_measured": "felt_scoring",
    "closure_propagated": "state_propagation",
    "authority_propagated": "state_propagation",
    "consent_inferred": "state_propagation",
    "state_transition_implied": "state_propagation",
    "membership_propagates_authority": "state_propagation",
    "membership_propagates_evidence": "state_propagation",
    "membership_propagates_outcome": "state_propagation",
    "membership_propagates_closure": "state_propagation",
}


def _issue(rule: str, path: tuple[str, ...], detail: str) -> dict[str, str]:
    return {
        "rule": rule,
        "path": ".".join(path) or "$",
        "detail": detail,
    }


def lint_value(value: Any, *, path: tuple[str, ...] = ()) -> list[dict[str, str]]:
    issues: list[dict[str, str]] = []
    try:
        reject_private_content(value)
    except RecordValidationError as error:
        issues.append(_issue("prose_leakage", path, str(error)))
    if isinstance(value, list):
        for index, item in enumerate(value):
            issues.extend(lint_value(item, path=(*path, str(index))))
        return issues
    if not isinstance(value, dict):
        return issues

    for key, item in value.items():
        normalized = str(key).lower()
        if normalized in FORBIDDEN_PROSE_KEYS or normalized.endswith("_prose"):
            issues.append(
                _issue("prose_leakage", (*path, str(key)), "private prose field")
            )
        rule = TRUE_FORBIDDEN.get(normalized)
        if rule and item is True:
            issues.append(
                _issue(rule, (*path, str(key)), "forbidden evidence assertion")
            )
        if (
            "felt" in normalized
            and any(
                marker in normalized
                for marker in ("score", "similarity", "dissimilarity", "probability")
            )
            and item is not None
            and item is not False
        ):
            issues.append(
                _issue(
                    "felt_scoring",
                    (*path, str(key)),
                    "numeric or categorical felt score is forbidden",
                )
            )
        issues.extend(lint_value(item, path=(*path, str(key))))

    schema = value.get("schema")
    if schema == "mechanical_comparison_receipt_v1":
        if not value.get("baseline_receipt_id"):
            issues.append(
                _issue("missing_baseline", path, "mechanical comparison lacks baseline")
            )
        if value.get("felt_outcome") is not None:
            issues.append(
                _issue("felt_scoring", path, "mechanical comparison contains felt outcome")
            )
    if schema in {"concordance_result_v1", "concordance_result_v2"} and not value.get(
        "baseline_observation_id"
    ):
        issues.append(
            _issue("missing_baseline", path, "Concordance result lacks baseline")
        )
    if value.get("outcome") == "no_response":
        if value.get("felt_result_established") is not False:
            issues.append(
                _issue("silence_as_agreement", path, "silence established felt result")
            )
        if value.get("review_pending") is not True:
            issues.append(
                _issue("silence_as_agreement", path, "silence did not remain pending")
            )
        if value.get("closure_propagated") is not False:
            issues.append(
                _issue("silence_as_agreement", path, "silence propagated closure")
            )
    if value.get("identity_relation") == "exact_identity" and (
        not value.get("process_identity_sha256")
        or not value.get("deployment_identity_sha256")
    ):
        issues.append(
            _issue(
                "identity_without_receipts",
                path,
                "exact identity lacks process or deployment hash",
            )
        )
    if value.get("telemetry_relation") == "exact_identity" and not any(
        str(ref).startswith("exact_identity:")
        for ref in value.get("minime_telemetry_refs") or []
    ):
        issues.append(
            _issue(
                "identity_without_receipts",
                path,
                "exact telemetry relation lacks exact receipt reference",
            )
        )
    if value.get("classification") == "source_declared" and value.get(
        "active"
    ) is True:
        issues.append(
            _issue(
                "source_runtime_confusion",
                path,
                "source declaration represented as active runtime observation",
            )
        )
    return issues


def _json_values(path: Path) -> Iterable[tuple[str, Any]]:
    if not path.is_file():
        return ()
    if path.suffix == ".jsonl":
        values = []
        for line_number, raw in enumerate(
            path.read_text(encoding="utf-8").splitlines(), 1
        ):
            if not raw.strip():
                continue
            values.append((f"{path.name}:{line_number}", json.loads(raw)))
        return values
    return ((path.name, json.loads(path.read_text(encoding="utf-8"))),)


def lint_workspace(workspace: Path) -> dict[str, Any]:
    root = workspace / "diagnostics/evidence_study_runtime_v1"
    paths = [
        root / "operator_events.jsonl",
        root / "campaigns.jsonl",
        root / "plans.jsonl",
        root / "windows.jsonl",
        root / "window_receipts.jsonl",
        root / "comparisons.jsonl",
        root / "capture_gaps.jsonl",
        root / "reviews.jsonl",
        root / "status.json",
    ]
    paths.extend(sorted((root / "review_packets").glob("*.json")))
    paths.extend(
        (
            workspace / "diagnostics/felt_mechanism_concordance_v1/studies.jsonl",
            workspace
            / "diagnostics/felt_mechanism_concordance_v1/observations.jsonl",
            workspace / "diagnostics/felt_mechanism_concordance_v1/results.jsonl",
            workspace / "diagnostics/felt_contract_graph_v1/contracts.jsonl",
            workspace / "diagnostics/felt_contract_graph_v1/status.json",
        )
    )
    issues: list[dict[str, str]] = []
    checked = 0
    for path in paths:
        for locator, value in _json_values(path):
            checked += 1
            issues.extend(lint_value(value, path=(locator,)))
    store_root = workspace / "diagnostics/evidence_event_store_v2"
    if store_root.is_dir():
        store = EvidenceEventStore(store_root)
        payloads, corrupt = store.payloads_for_stream("felt_mechanism_concordance")
        if corrupt:
            issues.append(
                _issue("prose_leakage", ("v2",), "Concordance stream is corrupt")
            )
        for index, value in enumerate(payloads):
            if value.get("schema") != "evidence_study_domain_event_v1":
                continue
            checked += 1
            issues.extend(lint_value(value, path=("v2", str(index))))
    counts: dict[str, int] = {}
    for issue in issues:
        counts[issue["rule"]] = counts.get(issue["rule"], 0) + 1
    return {
        "schema": "experiential_epistemics_lint_status_v1",
        "schema_version": 1,
        "valid": not issues,
        "checked_record_count": checked,
        "issue_count": len(issues),
        "rule_counts": dict(sorted(counts.items())),
        "issues": issues[:256],
        "canonical_event_appended": False,
        "history_rewritten": False,
        "artifact_authority_state_v1": authority_state(),
    }


class EpistemicTests(unittest.TestCase):
    def test_adversarial_boundaries(self) -> None:
        fixtures = [
            ({"grants_approval": True}, "authority_escalation"),
            ({"felt_similarity_score": 0.9}, "felt_scoring"),
            ({"causation_established": True}, "causal_overclaim"),
            (
                {"classification": "source_declared", "active": True},
                "source_runtime_confusion",
            ),
            (
                {
                    "schema": "mechanical_comparison_receipt_v1",
                    "felt_outcome": None,
                },
                "missing_baseline",
            ),
            (
                {
                    "outcome": "no_response",
                    "felt_result_established": True,
                    "review_pending": False,
                },
                "silence_as_agreement",
            ),
            (
                {"identity_relation": "exact_identity"},
                "identity_without_receipts",
            ),
            ({"closure_propagated": True}, "state_propagation"),
            ({"prompt": "private"}, "prose_leakage"),
        ]
        for value, rule in fixtures:
            self.assertIn(rule, {item["rule"] for item in lint_value(value)})

    def test_valid_legacy_false_markers_are_accepted(self) -> None:
        value = {
            "scalar_felt_dissimilarity_measured": False,
            "causation_established": False,
            "closure_propagated": False,
            "live_eligible_now": False,
        }
        self.assertEqual(lint_value(value), [])


def self_test() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(EpistemicTests)
    return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser(description=__doc__)
    value.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    value.add_argument("--json", action="store_true")
    value.add_argument("command", choices=("lint", "verify", "explain", "self-test"))
    value.add_argument("--rule", choices=tuple(sorted(RULES)))
    value.add_argument("--write", action="store_true")
    value.add_argument("--receipt-json", action="store_true")
    return value


def main() -> int:
    args = parser().parse_args()
    import time

    started = time.monotonic()
    if args.command == "self-test":
        result = {"valid": self_test() == 0}
    elif args.command == "explain":
        result = {
            "valid": True,
            "rules": (
                {args.rule: RULES[args.rule]} if args.rule else dict(sorted(RULES.items()))
            ),
        }
    else:
        result = lint_workspace(args.workspace.resolve())
        if args.command == "lint" and args.write and result["valid"]:
            output = (
                args.workspace.resolve()
                / "diagnostics/experiential_epistemics_v1/status.json"
            )
            owner_atomic_write_json(output, result)
            if args.receipt_json:
                result = projector_receipt(
                    "experiential_epistemics",
                    result,
                    {"status.json": output},
                    started_monotonic=started,
                )
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if result.get("valid") else 1


if __name__ == "__main__":
    raise SystemExit(main())
