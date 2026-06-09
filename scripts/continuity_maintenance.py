#!/usr/bin/env python3
"""Append-only maintenance for Astrid/Minime continuity records.

Default mode is read-only. Use --apply to append reconciliation/triage records.
"""

from __future__ import annotations

import argparse
import copy
import json
import sys
import tempfile
import unittest
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any


ASTRID_WORKSPACE = Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace")
MINIME_WORKSPACE = Path("/Users/v/other/minime/workspace")

RUNNING_EVENT_STATUSES = {"running", "llm_running"}
TERMINAL_JOB_STATUSES = {"completed", "thin_output", "timeout", "failed", "canceled", "blocked"}
PARKING_POLICIES = {
    "astrid": {"being": "astrid", "park_hours": 72, "native_register": "astrid_motif_language"},
    "minime": {"being": "minime", "park_hours": 24, "native_register": "minime_spectral_state"},
    "custom": {"being": "custom", "park_hours": 24, "native_register": "unknown"},
}


def utc_now() -> datetime:
    return datetime.now(timezone.utc)


def iso_now() -> str:
    return utc_now().isoformat().replace("+00:00", "Z")


def parse_time(value: Any) -> datetime | None:
    if not isinstance(value, str) or not value.strip():
        return None
    text = value.strip().replace("Z", "+00:00")
    try:
        parsed = datetime.fromisoformat(text)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def read_json(path: Path, default: Any = None) -> Any:
    try:
        return json.loads(path.read_text())
    except Exception:
        return default


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    rows: list[dict[str, Any]] = []
    for line in path.read_text().splitlines():
        try:
            value = json.loads(line)
        except Exception:
            continue
        if isinstance(value, dict):
            rows.append(value)
    return rows


def append_jsonl(path: Path, row: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a") as handle:
        handle.write(json.dumps(row, sort_keys=True) + "\n")


def base_action(action: Any) -> str:
    text = str(action or "").strip()
    if not text:
        return ""
    return text.split(None, 1)[0].strip("`*[](){}<>").strip(".,;:").upper()


def workspace_label(workspace: Path) -> str:
    text = str(workspace).casefold()
    if "astrid" in text:
        return "astrid"
    if "minime" in text:
        return "minime"
    return "custom"


def parking_policy_for_workspace(
    workspace: Path,
    override_hours: int | None = None,
) -> dict[str, Any]:
    label = workspace_label(workspace)
    policy = dict(PARKING_POLICIES.get(label, PARKING_POLICIES["custom"]))
    if override_hours is not None:
        policy["park_hours"] = override_hours
        policy["override"] = True
    else:
        policy["override"] = False
    return policy


def meaningful_text(value: Any) -> bool:
    text = str(value or "").strip()
    if not text:
        return False
    lowered = text.casefold()
    if lowered in {"<structured prose>", "<felt note>", "<reason>", "<note>", "...", "current"}:
        return False
    return not (text.startswith("<") and text.endswith(">"))


def meaningful_list(value: Any) -> bool:
    return isinstance(value, list) and any(meaningful_text(item) for item in value)


def valid_charter(charter: Any) -> bool:
    if not isinstance(charter, dict):
        return False
    return bool(
        meaningful_text(charter.get("hypothesis"))
        or meaningful_text(charter.get("method_intent"))
        or meaningful_text(charter.get("proposed_next_action"))
        or meaningful_list(charter.get("evidence_targets"))
        or meaningful_list(charter.get("stop_criteria"))
    )


def evidence_meaningful(evidence: Any) -> bool:
    if not isinstance(evidence, dict):
        return False
    felt = evidence.get("felt_observations")
    if isinstance(felt, list):
        for item in felt:
            if isinstance(item, dict) and (
                meaningful_text(item.get("note"))
                or meaningful_text(item.get("felt"))
                or meaningful_text(item.get("summary"))
            ):
                return True
    telemetry = evidence.get("telemetry_snapshots")
    if isinstance(telemetry, list) and telemetry:
        return True
    artifacts = evidence.get("artifact_refs")
    return isinstance(artifacts, list) and bool(artifacts)


def latest_by_id(rows: list[dict[str, Any]], key: str) -> dict[str, dict[str, Any]]:
    latest: dict[str, dict[str, Any]] = {}
    for row in rows:
        identifier = row.get(key)
        if isinstance(identifier, str) and identifier:
            latest[identifier] = row
    return latest


def action_key(event: dict[str, Any]) -> str:
    return str(event.get("action_id") or (
        f"{event.get('started_at', '')}:"
        f"{event.get('canonical_action', '')}:"
        f"{event.get('effective_action', '')}"
    ))


def has_later_terminal_event(events: list[dict[str, Any]], index: int) -> bool:
    key = action_key(events[index])
    for event in events[index + 1:]:
        if action_key(event) == key and event.get("status") not in RUNNING_EVENT_STATUSES:
            return True
    return False


def load_terminal_jobs(workspace: Path) -> dict[str, dict[str, Any]]:
    jobs: dict[str, dict[str, Any]] = {}
    for path in (workspace / "llm_jobs" / "jobs").glob("*/job.json"):
        job = read_json(path, {})
        if not isinstance(job, dict) or job.get("status") not in TERMINAL_JOB_STATUSES:
            continue
        action_id = job.get("action_id")
        if isinstance(action_id, str) and action_id:
            jobs[action_id] = job
    return jobs


@dataclass
class MaintenanceChange:
    kind: str
    workspace: str
    path: str
    summary: str
    payload: dict[str, Any] = field(default_factory=dict)


@dataclass
class MaintenanceReport:
    dry_run: bool
    changes: list[MaintenanceChange] = field(default_factory=list)

    def add(self, change: MaintenanceChange) -> None:
        self.changes.append(change)

    def as_dict(self) -> dict[str, Any]:
        return {
            "schema_version": 1,
            "dry_run": self.dry_run,
            "change_count": len(self.changes),
            "changes": [
                {
                    "kind": change.kind,
                    "workspace": change.workspace,
                    "path": change.path,
                    "summary": change.summary,
                    "payload": change.payload,
                }
                for change in self.changes
            ],
        }


def reconcile_stale_events(
    workspace: Path,
    report: MaintenanceReport,
    *,
    apply: bool,
    stale_minutes: int,
    limit: int | None,
    now: datetime,
) -> None:
    terminal_jobs = load_terminal_jobs(workspace)
    cutoff = now - timedelta(minutes=stale_minutes)
    for events_path in sorted((workspace / "action_threads" / "threads").glob("*/events.jsonl")):
        events = read_jsonl(events_path)
        for idx, event in enumerate(events):
            if limit is not None and len(report.changes) >= limit:
                return
            if event.get("status") not in RUNNING_EVENT_STATUSES:
                continue
            started = parse_time(event.get("started_at") or event.get("created_at"))
            if started and started > cutoff:
                continue
            if has_later_terminal_event(events, idx):
                continue
            action_id = event.get("action_id")
            job = terminal_jobs.get(action_id) if isinstance(action_id, str) else None
            status = str(job.get("status")) if job else "stale_reconciled"
            summary = (
                str(job.get("summary") or status.replace("_", " "))
                if job
                else "Stale running action reconciled after no terminal event was found."
            )
            reconciled = copy.deepcopy(event)
            reconciled["status"] = status
            reconciled["ended_at"] = iso_now()
            reconciled["outcome_summary"] = summary
            if not reconciled.get("post_state"):
                reconciled["post_state"] = reconciled.get("pre_state") or {}
            reconciled["source"] = "continuity_maintenance"
            reconciled["continuity_reconciliation_v1"] = {
                "schema_version": 1,
                "reconciled_at": reconciled["ended_at"],
                "reason": "stale_running_action_without_terminal_event",
                "original_status": event.get("status"),
                "matched_job_id": job.get("job_id") if job else None,
                "matched_job_status": job.get("status") if job else None,
                "mode": "apply" if apply else "dry_run",
            }
            change = MaintenanceChange(
                kind="stale_event_reconciliation",
                workspace=str(workspace),
                path=str(events_path),
                summary=f"{action_id or '(no action_id)'} {event.get('status')} -> {status}",
                payload=reconciled,
            )
            report.add(change)
            if apply:
                append_jsonl(events_path, reconciled)


def park_stale_experiments(
    workspace: Path,
    report: MaintenanceReport,
    *,
    apply: bool,
    policy: dict[str, Any],
    limit: int | None,
    now: datetime,
    include_current: bool = False,
) -> None:
    park_hours = int(policy.get("park_hours", 24))
    cutoff = now - timedelta(hours=park_hours)
    for thread_dir in sorted((workspace / "action_threads" / "threads").glob("*")):
        if limit is not None and len(report.changes) >= limit:
            return
        thread = read_json(thread_dir / "thread.json", {})
        if not isinstance(thread, dict):
            continue
        current_id = thread.get("active_experiment_id")
        experiments_path = thread_dir / "experiments.jsonl"
        experiments = latest_by_id(read_jsonl(experiments_path), "experiment_id")
        for experiment in sorted(experiments.values(), key=lambda item: item.get("updated_at") or ""):
            if limit is not None and len(report.changes) >= limit:
                return
            experiment_id = experiment.get("experiment_id")
            if not include_current and experiment_id == current_id:
                continue
            if experiment.get("status", "active") != "active":
                continue
            if base_action(experiment.get("planned_next")) != "EXPERIMENT_PLAN":
                continue
            if valid_charter(experiment.get("charter_v1")):
                continue
            if evidence_meaningful(experiment.get("evidence_v1")):
                continue
            updated_at = parse_time(experiment.get("updated_at") or experiment.get("created_at"))
            if updated_at is None or updated_at > cutoff:
                continue
            parked = copy.deepcopy(experiment)
            parked["status"] = "paused"
            parked["planned_next"] = f"EXPERIMENT_RESUME {experiment_id}"
            parked["updated_at"] = iso_now()
            parked["experiment_triage_v1"] = {
                "schema_version": 1,
                "triaged_at": parked["updated_at"],
                "reason": "non_current_plan_stage_charterless_evidenceless",
                "prior_status": experiment.get("status"),
                "prior_planned_next": experiment.get("planned_next"),
                "mode": "apply" if apply else "dry_run",
                "being_policy_v1": {
                    "schema_version": 1,
                    "being": policy.get("being", "custom"),
                    "park_hours": park_hours,
                    "native_register": policy.get("native_register", "unknown"),
                    "reason": "per_being_cadence_policy",
                    "override": bool(policy.get("override")),
                },
            }
            parked["being_policy_v1"] = {
                "schema_version": 1,
                "being": policy.get("being", "custom"),
                "park_hours": park_hours,
                "native_register": policy.get("native_register", "unknown"),
                "reason": "per_being_cadence_policy",
                "override": bool(policy.get("override")),
            }
            change = MaintenanceChange(
                kind="experiment_parking",
                workspace=str(workspace),
                path=str(experiments_path),
                summary=f"{experiment_id} active -> paused",
                payload=parked,
            )
            report.add(change)
            if apply:
                append_jsonl(experiments_path, parked)


def workspace_paths(selected: str | None) -> list[Path]:
    if selected == "astrid":
        return [ASTRID_WORKSPACE]
    if selected == "minime":
        return [MINIME_WORKSPACE]
    if selected:
        return [Path(selected)]
    return [ASTRID_WORKSPACE, MINIME_WORKSPACE]


def run_maintenance(
    *,
    apply: bool,
    selected_workspace: str | None = None,
    stale_minutes: int = 45,
    park_hours: int | None = None,
    limit: int | None = None,
    include_current: bool = False,
) -> MaintenanceReport:
    report = MaintenanceReport(dry_run=not apply)
    now = utc_now()
    for workspace in workspace_paths(selected_workspace):
        policy = parking_policy_for_workspace(workspace, park_hours)
        reconcile_stale_events(
            workspace,
            report,
            apply=apply,
            stale_minutes=stale_minutes,
            limit=limit,
            now=now,
        )
        if limit is not None and len(report.changes) >= limit:
            break
        park_stale_experiments(
            workspace,
            report,
            apply=apply,
            policy=policy,
            limit=limit,
            now=now,
            include_current=include_current,
        )
    return report


def render_markdown(report: MaintenanceReport) -> str:
    lines = ["# Continuity Maintenance", ""]
    lines.append(f"- Mode: {'dry-run' if report.dry_run else 'apply'}")
    lines.append(f"- Planned/appended changes: {len(report.changes)}")
    if not report.changes:
        lines.append("- No eligible stale events or parkable experiments found.")
        return "\n".join(lines) + "\n"
    lines.append("")
    for idx, change in enumerate(report.changes, start=1):
        lines.append(f"{idx}. `{change.kind}` {change.summary}")
        lines.append(f"   path: `{change.path}`")
        policy = change.payload.get("being_policy_v1") or (
            change.payload.get("experiment_triage_v1", {}).get("being_policy_v1")
            if isinstance(change.payload.get("experiment_triage_v1"), dict)
            else None
        )
        if isinstance(policy, dict):
            lines.append(
                "   being_policy_v1: "
                f"being={policy.get('being')} park_hours={policy.get('park_hours')} "
                f"native_register={policy.get('native_register')} override={policy.get('override')}"
            )
    return "\n".join(lines) + "\n"


class ContinuityMaintenanceTests(unittest.TestCase):
    def test_apply_reconciles_stale_event_with_terminal_job(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp) / "workspace"
            thread_dir = workspace / "action_threads" / "threads" / "th_test"
            thread_dir.mkdir(parents=True)
            event = {
                "schema_version": 1,
                "action_id": "act_1",
                "thread_id": "th_test",
                "status": "llm_running",
                "started_at": "2026-01-01T00:00:00Z",
                "canonical_action": "EXAMINE x",
                "effective_action": "EXAMINE x",
                "pre_state": {"fill_pct": 68.0},
                "post_state": {},
                "outcome_summary": "running",
            }
            append_jsonl(thread_dir / "events.jsonl", event)
            job_dir = workspace / "llm_jobs" / "jobs" / "job_1"
            job_dir.mkdir(parents=True)
            (job_dir / "job.json").write_text(json.dumps({
                "job_id": "job_1",
                "action_id": "act_1",
                "status": "completed",
                "summary": "Finished cleanly.",
            }))

            report = run_maintenance(
                apply=True,
                selected_workspace=str(workspace),
                stale_minutes=1,
            )

            self.assertEqual(len(report.changes), 1)
            rows = read_jsonl(thread_dir / "events.jsonl")
            self.assertEqual(rows[-1]["action_id"], "act_1")
            self.assertEqual(rows[-1]["status"], "completed")
            self.assertEqual(rows[-1]["continuity_reconciliation_v1"]["matched_job_id"], "job_1")

    def test_apply_parks_stale_non_current_experiment(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp) / "workspace"
            thread_dir = workspace / "action_threads" / "threads" / "th_test"
            thread_dir.mkdir(parents=True)
            (thread_dir / "thread.json").write_text(json.dumps({
                "thread_id": "th_test",
                "active_experiment_id": "exp_current",
            }))
            append_jsonl(thread_dir / "experiments.jsonl", {
                "schema_version": 1,
                "thread_id": "th_test",
                "experiment_id": "exp_old",
                "status": "active",
                "planned_next": "EXPERIMENT_PLAN exp_old",
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z",
                "charter_v1": None,
                "evidence_v1": None,
            })

            report = run_maintenance(
                apply=True,
                selected_workspace=str(workspace),
                park_hours=1,
            )

            self.assertEqual(len(report.changes), 1)
            rows = read_jsonl(thread_dir / "experiments.jsonl")
            self.assertEqual(rows[-1]["experiment_id"], "exp_old")
            self.assertEqual(rows[-1]["status"], "paused")
            self.assertEqual(rows[-1]["planned_next"], "EXPERIMENT_RESUME exp_old")
            self.assertIn("experiment_triage_v1", rows[-1])
            self.assertIn("being_policy_v1", rows[-1])

    def test_default_parking_policy_differs_by_being(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            old_stamp = (utc_now() - timedelta(hours=48)).isoformat().replace("+00:00", "Z")
            for name in ("astrid", "minime"):
                workspace = Path(tmp) / name / "workspace"
                thread_dir = workspace / "action_threads" / "threads" / "th_test"
                thread_dir.mkdir(parents=True)
                (thread_dir / "thread.json").write_text(json.dumps({
                    "thread_id": "th_test",
                    "active_experiment_id": "exp_current",
                }))
                append_jsonl(thread_dir / "experiments.jsonl", {
                    "schema_version": 1,
                    "thread_id": "th_test",
                    "experiment_id": f"exp_old_{name}",
                    "status": "active",
                    "planned_next": "EXPERIMENT_PLAN current",
                    "created_at": old_stamp,
                    "updated_at": old_stamp,
                    "charter_v1": None,
                    "evidence_v1": None,
                })

            astrid_report = run_maintenance(
                apply=False,
                selected_workspace=str(Path(tmp) / "astrid" / "workspace"),
            )
            minime_report = run_maintenance(
                apply=False,
                selected_workspace=str(Path(tmp) / "minime" / "workspace"),
            )

            self.assertEqual(len(astrid_report.changes), 0)
            self.assertEqual(len(minime_report.changes), 1)
            policy = minime_report.changes[0].payload["being_policy_v1"]
            self.assertEqual(policy["being"], "minime")
            self.assertEqual(policy["park_hours"], 24)

    def test_dry_run_does_not_append(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp) / "workspace"
            thread_dir = workspace / "action_threads" / "threads" / "th_test"
            thread_dir.mkdir(parents=True)
            append_jsonl(thread_dir / "events.jsonl", {
                "action_id": "act_1",
                "thread_id": "th_test",
                "status": "running",
                "started_at": "2026-01-01T00:00:00Z",
            })
            before = (thread_dir / "events.jsonl").read_text()

            report = run_maintenance(
                apply=False,
                selected_workspace=str(workspace),
                stale_minutes=1,
            )

            self.assertEqual(len(report.changes), 1)
            self.assertEqual((thread_dir / "events.jsonl").read_text(), before)


def run_self_tests() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(ContinuityMaintenanceTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apply", action="store_true", help="append reconciliation/triage records")
    parser.add_argument("--dry-run", action="store_true", help="force read-only mode")
    parser.add_argument("--json", action="store_true", help="emit JSON")
    parser.add_argument("--workspace", help="astrid, minime, or an explicit workspace path")
    parser.add_argument("--limit", type=int, help="maximum number of records to append/report")
    parser.add_argument("--stale-minutes", type=int, default=45)
    parser.add_argument("--park-hours", type=int)
    parser.add_argument("--include-current", action="store_true", help="allow current active experiment parking")
    parser.add_argument("--self-test", action="store_true", help="run fixture tests")
    args = parser.parse_args(argv)
    if args.self_test:
        return run_self_tests()
    apply = bool(args.apply and not args.dry_run)
    report = run_maintenance(
        apply=apply,
        selected_workspace=args.workspace,
        stale_minutes=args.stale_minutes,
        park_hours=args.park_hours,
        limit=args.limit,
        include_current=args.include_current,
    )
    if args.json:
        print(json.dumps(report.as_dict(), indent=2, sort_keys=True))
    else:
        print(render_markdown(report), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
