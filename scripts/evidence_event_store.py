#!/usr/bin/env python3
"""Manage the append-only Evidence Event Store V2.

V2 remains inert until ``activate`` writes a verified atomic manifest. The
legacy JSONL files are read-only migration sources after that cutover.
"""

from __future__ import annotations

import argparse
from collections import Counter
import json
import sys
import unittest
from pathlib import Path
from typing import Any

try:
    from evidence_store import EvidenceEventStore, EvidenceStoreError
    from evidence_store.migration import LegacyEventSource, import_legacy_sources
except ModuleNotFoundError:
    from scripts.evidence_store import EvidenceEventStore, EvidenceStoreError
    from scripts.evidence_store.migration import LegacyEventSource, import_legacy_sources

DEFAULT_WORKSPACE = Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace")


def default_store_root(workspace: Path) -> Path:
    return workspace / "diagnostics/evidence_event_store_v2"


def legacy_sources(workspace: Path) -> list[LegacyEventSource]:
    diagnostics = workspace / "diagnostics"
    return [
        LegacyEventSource("addressing", diagnostics / "introspection_addressing_v1/events.jsonl"),
        LegacyEventSource("sandbox", diagnostics / "sandbox_trial_queue_v1/events.jsonl"),
        LegacyEventSource("corridor_v1", diagnostics / "agency_corridor_v1/events.jsonl"),
        LegacyEventSource("corridor_v2", diagnostics / "agency_corridor_v2/events.jsonl"),
    ]


def projection_paths(workspace: Path) -> dict[str, Path]:
    diagnostics = workspace / "diagnostics"
    result: dict[str, Path] = {}
    for prefix, directory in (
        ("addressing", diagnostics / "introspection_addressing_v1"),
        ("sandbox", diagnostics / "sandbox_trial_queue_v1"),
        ("corridor_v1", diagnostics / "agency_corridor_v1"),
        ("corridor_v2", diagnostics / "agency_corridor_v2"),
    ):
        for filename in ("status.json", "queue.md", "queue.json", "report.md"):
            path = directory / filename
            if path.is_file():
                result[f"{prefix}:{filename}"] = path
    return result


def counter_audits(workspace: Path) -> dict[str, Any]:
    diagnostics = workspace / "diagnostics"
    result: dict[str, Any] = {}
    for name, path in (
        ("addressing", diagnostics / "introspection_addressing_v1/status.json"),
        ("sandbox", diagnostics / "sandbox_trial_queue_v1/status.json"),
        ("corridor_v1", diagnostics / "agency_corridor_v1/status.json"),
        ("corridor_v2", diagnostics / "agency_corridor_v2/status.json"),
    ):
        if not path.is_file():
            result[name] = {"exists": False}
            continue
        try:
            value = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            result[name] = {"exists": True, "valid_json": False, "error": str(error)}
            continue
        audit: dict[str, Any] = {
            "exists": True,
            "valid_json": isinstance(value, dict),
            "corrupt_event_lines": (
                int(value.get("corrupt_event_lines") or 0) if isinstance(value, dict) else None
            ),
        }
        if not isinstance(value, dict):
            result[name] = audit
            continue
        summary = value.get("summary")
        if isinstance(summary, dict):
            audit["summary"] = summary
        canonical_audit = value.get("counter_audit")
        if isinstance(canonical_audit, dict):
            audit["counter_audit"] = canonical_audit
            checks = canonical_audit.get("checks")
            if isinstance(checks, dict):
                audit["consistent"] = bool(checks) and all(
                    check is True for check in checks.values()
                )
        trials = value.get("trials")
        if isinstance(trials, (dict, list)):
            trial_values = trials.values() if isinstance(trials, dict) else trials
            trial_values = list(trial_values)
            audit["trial_count"] = len(trial_values)
            audit["trial_state_counts"] = dict(
                sorted(
                    Counter(
                        str(
                            trial.get("state")
                            or trial.get("classification")
                            or trial.get("status")
                            or "unknown"
                        )
                        for trial in trial_values
                        if isinstance(trial, dict)
                    ).items()
                )
            )
        for collection in (
            "packets",
            "leases",
            "programs",
            "portfolios",
            "patch_bundles",
            "source_prep_proposals",
            "reopened_work_items",
            "self_observation_responses",
        ):
            items = value.get(collection)
            if isinstance(items, (dict, list)):
                audit[f"{collection}_count"] = len(items)
        if "consistent" not in audit:
            corrupt = audit.get("corrupt_event_lines")
            live_violations = (
                summary.get("live_violation_count") if isinstance(summary, dict) else None
            )
            audit["consistent"] = corrupt in (None, 0) and live_violations in (None, 0)
        result[name] = audit
    return result


def _load_activation(store: EvidenceEventStore) -> dict[str, Any]:
    if not store.activation_path.is_file():
        return {}
    try:
        value = json.loads(store.activation_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}
    return value if isinstance(value, dict) else {}


def _emit(value: Any, as_json: bool) -> None:
    if as_json:
        print(json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False))
    else:
        print(json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False))


def run_self_test() -> int:
    try:
        from test_evidence_event_store import EvidenceEventStoreTests
    except ModuleNotFoundError:
        from scripts.test_evidence_event_store import EvidenceEventStoreTests
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(EvidenceEventStoreTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--root", type=Path)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    subparsers = parser.add_subparsers(dest="command")

    subparsers.add_parser("status")
    subparsers.add_parser("verify")

    migrate = subparsers.add_parser("import-v1")
    migrate.add_argument("--write", action="store_true")

    activate = subparsers.add_parser("activate")
    activate.add_argument("--write", action="store_true")
    activate.add_argument("--actor", default="interactive-agent")
    activate.add_argument("--ack", required=True)

    export = subparsers.add_parser("export-v1")
    export.add_argument("--write", action="store_true")
    export.add_argument("--actor", default="interactive-agent")
    export.add_argument("--ack", required=True)
    export.add_argument("--output", type=Path, required=True)

    rollback = subparsers.add_parser("rollback")
    rollback.add_argument("--write", action="store_true")
    rollback.add_argument("--actor", default="interactive-agent")
    rollback.add_argument("--ack", required=True)
    rollback.add_argument("--compatibility-export", type=Path)

    checkpoint = subparsers.add_parser("checkpoint")
    checkpoint.add_argument("--write", action="store_true")
    checkpoint.add_argument("--projector", required=True)
    checkpoint.add_argument("--projector-version", type=int, required=True)
    checkpoint.add_argument("--output-hash", action="append", default=[])
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    if args.self_test:
        return run_self_test()
    if not args.command:
        parser.print_help()
        return 2
    workspace = args.workspace.resolve()
    store = EvidenceEventStore((args.root or default_store_root(workspace)).resolve())
    try:
        if args.command == "verify":
            result = store.verify().to_dict()
            _emit(result, args.json)
            return 0 if result["valid"] else 1
        if args.command == "status":
            verification = store.verify().to_dict()
            _emit(
                {
                    "schema": "evidence_event_store_status_v1",
                    "schema_version": 1,
                    "root": str(store.root),
                    "activation": _load_activation(store),
                    "verification": verification,
                },
                args.json,
            )
            return 0 if verification["valid"] else 1
        if args.command == "import-v1":
            receipt = import_legacy_sources(
                store,
                legacy_sources(workspace),
                projection_paths=projection_paths(workspace),
                counter_audits=counter_audits(workspace),
                write=args.write,
            )
            _emit(receipt, args.json)
            return 0 if receipt["verification"]["valid"] else 1
        if args.command == "activate":
            if not args.write:
                raise EvidenceStoreError("activation requires --write")
            _emit(store.activate(actor=args.actor, acknowledgement=args.ack), args.json)
            return 0
        if args.command == "export-v1":
            if not args.write:
                raise EvidenceStoreError("compatibility export requires --write")
            _emit(
                store.export_v1_compatibility(
                    args.output.resolve(), actor=args.actor, acknowledgement=args.ack
                ),
                args.json,
            )
            return 0
        if args.command == "rollback":
            if not args.write:
                raise EvidenceStoreError("rollback requires --write")
            _emit(
                store.rollback_to_v1(
                    actor=args.actor,
                    acknowledgement=args.ack,
                    compatibility_export=(
                        args.compatibility_export.resolve()
                        if args.compatibility_export
                        else None
                    ),
                ),
                args.json,
            )
            return 0
        if args.command == "checkpoint":
            hashes: dict[str, str] = {}
            for item in args.output_hash:
                if "=" not in item:
                    raise EvidenceStoreError("--output-hash values must be NAME=SHA256")
                name, digest = item.split("=", 1)
                hashes[name] = digest
            if not args.write:
                _emit(
                    {
                        "current": store.checkpoint_current(
                            args.projector, args.projector_version
                        )
                    },
                    args.json,
                )
                return 0
            path = store.write_checkpoint(args.projector, args.projector_version, hashes)
            _emit({"checkpoint": str(path), "current": True}, args.json)
            return 0
    except (EvidenceStoreError, OSError, ValueError) as error:
        _emit({"status": "failed", "error": str(error)}, True)
        return 1
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
