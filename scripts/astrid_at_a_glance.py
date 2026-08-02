#!/usr/bin/env python3
"""Read-only terminal dashboard for a CPU Astrid appliance."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


def workspace_for(home: Path) -> tuple[str, Path, Path]:
    icp_workspace = home / ".astrid-icp/state/home/default/edge"
    if icp_workspace.is_dir():
        return "ICP", icp_workspace, home / ".astrid-icp/state/bin"
    return "AVADO", home / ".astrid/home/default/edge", home / ".astrid/bin"


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return {}
    return value if isinstance(value, dict) else {}


def read_json_lines(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_text().splitlines()
    except OSError:
        return []
    values: list[dict[str, Any]] = []
    for line in lines:
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            values.append(value)
    return values


def tuning_state_view(value: dict[str, Any]) -> tuple[str, dict[str, Any]]:
    payload = value.get("payload")
    state = payload if isinstance(payload, dict) else value
    for field, status in (
        ("active_experiment", "active_trial"),
        ("active_validation", "active_validation"),
        ("standing_adoption", "standing_adoption"),
        ("suspended_adoption", "suspended_adoption"),
    ):
        candidate = state.get(field)
        if isinstance(candidate, dict):
            return status, candidate
    return "inactive", {}


def report_values(report: Path, workspace: Path, minutes: int) -> dict[str, str]:
    command = [str(report)]
    if ".astrid-icp" in str(report):
        command.extend(("--workspace", str(workspace)))
    command.extend(("--window-minutes", str(minutes)))
    try:
        result = subprocess.run(command, capture_output=True, text=True, timeout=15)
    except (OSError, subprocess.TimeoutExpired):
        return {}
    values: dict[str, str] = {}
    for line in result.stdout.splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            values[key] = value
    return values


def activity(report: Path, workspace: Path, minutes: int, limit: int) -> list[str]:
    command = [
        str(report),
        "--workspace",
        str(workspace),
        "--window-minutes",
        str(minutes),
        "--limit",
        str(limit),
        "--format",
        "text",
    ]
    try:
        result = subprocess.run(command, capture_output=True, text=True, timeout=15)
    except (OSError, subprocess.TimeoutExpired):
        return ["activity viewer unavailable"]
    return [line for line in result.stdout.splitlines() if line.strip()]


def compact(value: Any, fallback: str = "—", maximum: int = 180) -> str:
    if value is None or value == "":
        return fallback
    text = " ".join(str(value).split())
    return text if len(text) <= maximum else text[: maximum - 1] + "…"


def percent(value: str | None) -> str:
    if value in (None, "", "unknown"):
        return "—"
    try:
        return f"{float(value):.1f}%"
    except ValueError:
        return value


def newest_artifacts(workspace: Path, maximum: int = 6) -> list[str]:
    directories = (
        "journal",
        "introspections",
        "research",
        "measurements",
        "studies/results",
        "tuning/evidence",
        "research/syntheses",
        "peer/outbox",
        "peer/read",
        "proposals",
        "notices",
        "daydreams",
        "aspirations",
        "plans",
        "workshop/drafts",
        "workshop/revisions",
        "autonomous/turns",
    )
    files: list[Path] = []
    for relative in directories:
        directory = workspace / relative
        if directory.is_dir():
            files.extend(path for path in directory.iterdir() if path.is_file())
    files.sort(key=lambda path: path.stat().st_mtime, reverse=True)
    entries: list[str] = []
    for path in files[:maximum]:
        try:
            lines = [" ".join(line.split()) for line in path.read_text(errors="replace").splitlines()]
        except OSError:
            lines = []
        preview = next((line.lstrip("# ") for line in lines if line), "empty")
        entries.append(f"{path.relative_to(workspace)} — {compact(preview, maximum=120)}")
    return entries


def thread_evolution(workspace: Path, maximum: int = 6) -> list[str]:
    records = read_json_lines(workspace / "autonomous/thread_state.jsonl")
    if not records:
        return ["No stateful thread revisions yet — waiting for an authored research Action."]
    entries: list[str] = []
    for record in records[-maximum:]:
        claims = len(record.get("authored_claims") or [])
        findings = len(record.get("findings") or [])
        open_questions = len(record.get("open_questions") or [])
        event = compact(record.get("event"), "revision", 48)
        epistemic = (
            "v6_spectral_typed"
            if record.get("schema") == "astrid_edge_thread_state_v6"
            else "v5_retained_typed"
            if record.get("schema") == "astrid_edge_thread_state_v5"
            else "v4_inquiry"
            if record.get("schema") == "astrid_edge_thread_state_v4"
            else "v3_typed"
            if record.get("schema") == "astrid_edge_thread_state_v3"
            else "legacy_unclassified"
        )
        action = compact(record.get("last_action"), "—", 80)
        question = compact(record.get("question"), "—", 100)
        entries.append(
            f"r{record.get('revision', '?')} {event} status={record.get('status', '—')} epistemic={epistemic} "
            f"claims={claims} findings={findings} open={open_questions} action={action} question={question}"
        )
    return entries


def render(minutes: int, limit: int) -> None:
    home = Path.home()
    name, workspace, bin_dir = workspace_for(home)
    values = report_values(bin_dir / "report-edge-appliance", workspace, minutes)
    thread = read_json(workspace / "autonomous/thread_state.json")
    state_root = workspace.parents[2]
    hindsight = read_json(state_root / "operator/hindsight/latest.json")
    now = dt.datetime.now().astimezone().strftime("%Y-%m-%d %H:%M:%S %Z")

    print(f"Astrid — {name}   {now}")
    print("═" * 72)
    print(
        "CORE     edge={}/{}  capsules={}/20  model={} ({})  prompt={} chars".format(
            values.get("edge_service_state", "unknown"),
            values.get("astrid_service_state", "unknown"),
            values.get("astrid_loaded_capsule_count", "—"),
            values.get("selected_model", "unknown"),
            values.get("model_warmup_status", "unknown"),
            values.get("autonomy_last_prompt_chars", "—"),
        )
    )
    print(
        "RESERVOIR current={}  settled mean={}  range={}-{}  in 65–73.5%={}%".format(
            percent(values.get("current_fill_pct")),
            percent(values.get("fill_settled_mean_pct")),
            percent(values.get("fill_settled_min_pct")),
            percent(values.get("fill_settled_max_pct")),
            values.get("fill_settled_inside_65_73_5_pct", "—"),
        )
    )
    print(
        "AUTONOMY attempts={} authored={} recoveries={} last={} next={}".format(
            values.get("autonomy_attempts_today", "—"),
            values.get("autonomy_authored_turns_today", "—"),
            values.get("autonomy_transport_recoveries_today", "—"),
            values.get("autonomy_last_status", "—"),
            values.get("autonomy_next_due_at_unix_ms", "—"),
        )
    )
    print(
        "PRIVATE  introspection={}/{}  notebook={} observations={} latest-age={}ms".format(
            values.get("introspection_window_completed_calls", "0"),
            values.get("introspection_window_requested_calls", "0"),
            values.get("perceptual_notebook_enabled", "unknown"),
            values.get("perception_window_observations", "0"),
            values.get("perception_latest_age_ms", "—"),
        )
    )
    print(
        "INQUIRY  active-study={} starts={} completions={} syntheses={} peer-out={} peer-in={} duplicates={}".format(
            values.get("study_active_id", "none"),
            values.get("study_window_starts", "0"),
            values.get("study_window_completions", "0"),
            values.get("thread_state_syntheses_count", "0"),
            values.get("peer_window_shared", "0"),
            values.get("peer_window_available_unread", "0"),
            values.get("duplicate_journal_window_notices", "0"),
        )
    )
    print(
        "SPECTRAL substrate={} metric={} rollups={} entropy={} turnover={} tuning={}/{}".format(
            values.get("spectral_substrate_kind", "legacy_unknown"),
            values.get("spectral_fill_metric", "legacy_unknown"),
            values.get("spectral_window_rollups", "0"),
            values.get("spectral_current_spectral_entropy", "—"),
            values.get("spectral_current_mode_turnover", "—"),
            values.get("tuning_state_status", "inactive"),
            values.get("tuning_phase", "none"),
        )
    )
    print(
        "HINDSIGHT timer={} checkpoint-age={}ms artifacts={} continuity={} violations={}".format(
            values.get("hindsight_timer_state", "unknown"),
            values.get("hindsight_checkpoint_age_ms", "—"),
            values.get("hindsight_artifact_inventory_count", "—"),
            values.get("hindsight_continuity_from_previous_checkpoint_valid", "—"),
            values.get("hindsight_historical_integrity_violation_count", "—"),
        )
    )

    print("\nMACHINE-OBSERVED CONTEXT (not Astrid-authored)")
    observation = read_json(workspace / "perception/latest.json")
    if not observation:
        print("  No deterministic notebook baseline has been recorded yet.")
    else:
        print(
            "  triggers={}  hash={}".format(
                compact(", ".join(observation.get("trigger_classes") or []), maximum=100),
                compact(observation.get("record_sha256"), maximum=20),
            )
        )
        print(
            "  causal-class="
            + compact(observation.get("causal_class"), "legacy_unclassified", 80)
        )
        print("  " + compact(observation.get("summary"), maximum=220))
        print(
            "  authority="
            + compact(observation.get("authority"), maximum=80)
        )

    print("\nSPECTRAL CONTEXT (machine-derived; not authorship or causal proof)")
    spectral = read_json(workspace / "runtime/spectral_state.json")
    substrate = spectral.get("substrate") or spectral.get("spectral_substrate_v1") or {}
    substrate = substrate if isinstance(substrate, dict) else {}
    if not spectral or not substrate:
        print("  No substrate-labeled spectral state has been recorded yet.")
    else:
        print(
            "  kind={}  fill-metric={}  coverage={}/{}  identity={}".format(
                substrate.get("kind", "legacy_unknown"),
                substrate.get("fill_metric", "legacy_unknown"),
                substrate.get("exported_eigenvalue_count", "—"),
                substrate.get("reservoir_dim", "—"),
                spectral.get("mode_identity_state", "unavailable"),
            )
        )
        print(
            "  entropy={}  λ1-share={}  tail={}  density-gradient={}  turnover={}".format(
                spectral.get("spectral_entropy", "—"),
                spectral.get("lambda1_share", "—"),
                spectral.get("tail_share", "—"),
                spectral.get("density_gradient", "—"),
                spectral.get("mode_turnover", "—"),
            )
        )
    tuning_envelope = read_json(workspace / "tuning/state.json")
    if tuning_envelope:
        tuning_status, tuning = tuning_state_view(tuning_envelope)
        tuning_spec = tuning.get("spec")
        tuning_spec = tuning_spec if isinstance(tuning_spec, dict) else {}
        environment = tuning.get("environment")
        environment = environment if isinstance(environment, dict) else {}
        print(
            "  tuning status={} phase={} id={} candidate={} parameter={} policy={}".format(
                tuning_status,
                tuning.get("phase", tuning_status),
                tuning.get("tuning_id")
                or tuning.get("experiment_id")
                or tuning.get("validation_id")
                or tuning.get("adoption_id")
                or "none",
                tuning.get("candidate_id", "none"),
                tuning.get("parameter") or tuning_spec.get("parameter") or "none",
                compact(environment.get("policy_sha256"), maximum=18),
            )
        )

    print("\nWORKING THREAD")
    if not thread.get("thread_id"):
        print("  No structured thread yet — waiting for a genuinely authored stateful Action.")
    else:
        print(
            "  {}  status={}  last={}".format(
                compact(thread.get("thread_id")),
                thread.get("status", "unknown"),
                compact(thread.get("last_action")),
            )
        )
        for label, key in (
            ("question", "question"),
            ("hypothesis", "hypothesis"),
            ("latest", "latest_note"),
            ("conclusion", "conclusion"),
            ("uncertainty", "uncertainty"),
        ):
            if thread.get(key):
                print(f"  {label:<11} {compact(thread[key])}")
        for label, key in (
            ("claims", "authored_claims"),
            ("findings", "findings"),
            ("open questions", "open_questions"),
            ("hypotheses", "hypotheses"),
            ("methods", "methods"),
            ("studies", "study_ids"),
            ("syntheses", "syntheses"),
            ("uncertainties", "unresolved_uncertainties"),
        ):
            values_list = thread.get(key) or []
            if values_list:
                print(f"  {label:<11} " + " | ".join(compact(item, maximum=110) for item in values_list))
        records = thread.get("evidence_records") or []
        if records:
            print("  evidence    " + " | ".join(
                f"{item.get('kind', 'evidence')}[{item.get('epistemic_status', 'legacy')}]:{compact(item.get('reference'), maximum=60)}"
                for item in records[-4:]
            ))

    print("\nTHREAD EVOLUTION")
    for entry in thread_evolution(workspace):
        print("  " + entry)

    print(f"\nRECENT ACTIVITY (last {minutes}m)")
    lines = activity(bin_dir / "report-edge-activity", workspace, minutes, limit)
    if lines:
        for line in lines:
            print("  " + line)
    else:
        print("  No recorded activity in this window.")

    print("\nNEWEST OWNED ARTIFACTS (authorship is resolved by hindsight)")
    artifacts = newest_artifacts(workspace)
    if artifacts:
        for artifact in artifacts:
            print("  " + artifact)
    else:
        print("  No owned artifacts found in the tracked directories.")
    print("\nDURABLE HINDSIGHT")
    if hindsight:
        state_database = hindsight.get("state_database") or {}
        audit_database = hindsight.get("audit_database") or {}
        operator_database = hindsight.get("operator_hindsight_database") or {}
        print(
            "  checkpoint={}  indexed-artifacts={}  state-db={} bytes  audit-db={} bytes".format(
                hindsight.get("recorded_at_unix_ms", "—"),
                hindsight.get("artifact_inventory_count", "—"),
                state_database.get("size_bytes", "—"),
                audit_database.get("size_bytes", "—"),
            )
        )
        print(
            "  query-db={} rows={} owner-only={}".format(
                operator_database.get("quick_check", "—"),
                operator_database.get("row_counts", {}),
                operator_database.get("owner_only", "—"),
            )
        )
        print("  authority=" + compact(hindsight.get("authority"), maximum=100))
    else:
        print("  No owner-only hindsight checkpoint has been recorded yet.")
    print("\nPaths: " + str(workspace))
    print("Tip: run `~/astrid-hindsight --since 2026-07-30T00:00:00Z --include-excerpts` for a retrospective view.")


def main() -> int:
    parser = argparse.ArgumentParser(description="Read-only Astrid appliance dashboard")
    parser.add_argument("--window-minutes", type=int, default=180)
    parser.add_argument("--limit", type=int, default=12)
    parser.add_argument("--follow", action="store_true", help="refresh every 15 seconds")
    args = parser.parse_args()
    while True:
        if args.follow:
            print("\033[2J\033[H", end="")
        render(max(1, args.window_minutes), max(1, args.limit))
        if not args.follow:
            return 0
        time.sleep(15)


if __name__ == "__main__":
    sys.exit(main())
