#!/usr/bin/env python3
"""Read-only terminal dashboard for a CPU Astrid appliance."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import stat
import subprocess
import sys
import time
import unicodedata
from pathlib import Path
from typing import Any

IMMUTABLE_OPERATOR_ROOT = Path("/usr/libexec/astrid-edge/operator")
ARTIFACT_READ_MAX_BYTES = 64 * 1024
ARTIFACT_SCAN_MAX_PER_DIRECTORY = 256
ARTIFACT_SCAN_MAX_TOTAL = 2_048
CANDIDATE_PRESENTATION_INPUT_SCHEMA = (
    "astrid.edge_candidate_presentation.input.v1"
)
CANDIDATE_PRESENTATION_CONTENT_SCHEMA = (
    "astrid.edge_candidate_presentation.content.v1"
)
CANDIDATE_PRESENTATION_INPUT_MAX_BYTES = 256 * 1024
_DIRECTORY_OPEN_FLAGS = (
    os.O_RDONLY
    | getattr(os, "O_CLOEXEC", 0)
    | getattr(os, "O_DIRECTORY", 0)
    | getattr(os, "O_NOFOLLOW", 0)
)
_ARTIFACT_OPEN_FLAGS = (
    os.O_RDONLY
    | getattr(os, "O_CLOEXEC", 0)
    | getattr(os, "O_NOFOLLOW", 0)
    | getattr(os, "O_NONBLOCK", 0)
)


def terminal_safe_text(value: Any) -> str:
    """Neutralize terminal controls in owner-visible text."""

    return "".join(
        " "
        if unicodedata.category(character) in {"Cc", "Cf", "Cs", "Zl", "Zp"}
        else character
        for character in str(value)
    )


def terminal_safe_value(value: Any) -> Any:
    """Sanitize only strings in dashboard-local decoded data."""

    if isinstance(value, str):
        return terminal_safe_text(value)
    if isinstance(value, list):
        return [terminal_safe_value(item) for item in value]
    if isinstance(value, dict):
        return {key: terminal_safe_value(item) for key, item in value.items()}
    return value


def candidate_presentation() -> int:
    """Render a compact candidate layout from broker-supplied facts only."""

    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--candidate-presentation", action="store_true")
    parser.add_argument("--input-stdin", action="store_true")
    parser.add_argument("--window-minutes", type=int, required=True)
    parser.add_argument("--limit", type=int, required=True)
    parser.add_argument("--format", choices=("json",), required=True)
    args = parser.parse_args()
    if not args.candidate_presentation or not args.input_stdin:
        parser.error("the active-generation presentation requires broker stdin")
    raw = sys.stdin.buffer.read(CANDIDATE_PRESENTATION_INPUT_MAX_BYTES + 1)
    if len(raw) > CANDIDATE_PRESENTATION_INPUT_MAX_BYTES:
        parser.error("broker projection exceeds its bound")
    try:
        projection = json.loads(raw)
    except (UnicodeError, json.JSONDecodeError) as error:
        parser.error(f"broker projection is invalid: {error}")
    if (
        not isinstance(projection, dict)
        or projection.get("schema") != CANDIDATE_PRESENTATION_INPUT_SCHEMA
        or not isinstance(projection.get("facts"), list)
    ):
        parser.error("broker projection has the wrong schema")
    lines = []
    for fact in projection["facts"][:64]:
        if not isinstance(fact, dict):
            continue
        key = " ".join(terminal_safe_text(fact.get("key", "")).split())
        value = " ".join(terminal_safe_text(fact.get("value", "")).split())
        if key and value:
            lines.append(f"{key}: {value}"[:240])
    sections = [
        {"heading": f"At a glance {index + 1}", "lines": lines[index:index + 16]}
        for index in range(0, len(lines), 16)
    ][:12]
    result = {
        "schema": CANDIDATE_PRESENTATION_CONTENT_SCHEMA,
        "view": "at_a_glance",
        "title": "Active-generation at-a-glance view",
        "summary": (
            f"Candidate dashboard arranged {len(lines)} sanitized immutable-report facts; "
            "the trusted dashboard above remains authoritative."
        ),
        "sections": sections,
    }
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


def workspace_for(home: Path) -> tuple[str, Path]:
    icp_workspace = home / ".astrid-icp/state/home/default/edge"
    if icp_workspace.is_dir():
        return "ICP", icp_workspace
    return "AVADO", home / ".astrid/home/default/edge"


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return {}
    return terminal_safe_value(value) if isinstance(value, dict) else {}


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
            values.append(terminal_safe_value(value))
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
    command = [str(report), "--workspace", str(workspace)]
    command.extend(("--window-minutes", str(minutes)))
    try:
        result = subprocess.run(command, capture_output=True, text=True, timeout=15)
    except (OSError, subprocess.TimeoutExpired):
        return {}
    values: dict[str, str] = {}
    for raw_line in result.stdout.splitlines():
        line = terminal_safe_text(raw_line)
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
    return [
        terminal_safe_text(line)
        for line in result.stdout.splitlines()
        if line.strip()
    ]


def compact(value: Any, fallback: str = "—", maximum: int = 180) -> str:
    if value is None or value == "":
        return fallback
    text = " ".join(terminal_safe_text(value).split())
    return text if len(text) <= maximum else text[: maximum - 1] + "…"


def percent(value: str | None) -> str:
    if value in (None, "", "unknown"):
        return "—"
    try:
        return f"{float(value):.1f}%"
    except ValueError:
        return value


def _artifact_identity(value: os.stat_result) -> tuple[int, ...]:
    """Return the fields that must remain stable across an artifact read."""

    return (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_nlink,
        value.st_uid,
        value.st_gid,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )


def _open_directory_beneath(root_descriptor: int, relative: str) -> int | None:
    """Open an allowlisted child directory without following any component."""

    if not getattr(os, "O_NOFOLLOW", 0) or not getattr(os, "O_DIRECTORY", 0):
        return None
    try:
        descriptor = os.dup(root_descriptor)
    except OSError:
        return None
    try:
        for component in Path(relative).parts:
            if component in {"", ".", ".."} or "/" in component:
                os.close(descriptor)
                return None
            child = os.open(
                component,
                _DIRECTORY_OPEN_FLAGS,
                dir_fd=descriptor,
            )
            os.close(descriptor)
            descriptor = child
        return descriptor
    except OSError:
        os.close(descriptor)
        return None


def _read_stable_artifact(
    directory_descriptor: int,
    basename: str,
    before: os.stat_result,
) -> bytes | None:
    """Read one regular, single-link artifact through its anchored directory."""

    if not getattr(os, "O_NOFOLLOW", 0):
        return None
    if (
        not basename
        or basename.startswith(".")
        or "/" in basename
        or "\x00" in basename
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size < 0
        or before.st_size > ARTIFACT_READ_MAX_BYTES
    ):
        return None
    descriptor: int | None = None
    try:
        descriptor = os.open(
            basename,
            _ARTIFACT_OPEN_FLAGS,
            dir_fd=directory_descriptor,
        )
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_nlink != 1
            or opened.st_size < 0
            or opened.st_size > ARTIFACT_READ_MAX_BYTES
            or _artifact_identity(opened) != _artifact_identity(before)
        ):
            return None

        chunks: list[bytes] = []
        size = 0
        while size <= ARTIFACT_READ_MAX_BYTES:
            remaining = ARTIFACT_READ_MAX_BYTES + 1 - size
            chunk = os.read(descriptor, min(8_192, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            size += len(chunk)
        after = os.fstat(descriptor)
        if (
            size > ARTIFACT_READ_MAX_BYTES
            or size != before.st_size
            or _artifact_identity(after) != _artifact_identity(opened)
        ):
            return None
        return b"".join(chunks)
    except OSError:
        return None
    finally:
        if descriptor is not None:
            os.close(descriptor)


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
    if maximum <= 0:
        return []
    try:
        # ICP deliberately reaches its SSD-backed workspace through a trusted
        # appliance symlink. Resolve that root once, then anchor every child
        # lookup to the opened descriptor. No artifact-controlled component is
        # resolved or followed after this point.
        resolved_workspace = workspace.resolve(strict=True)
        root_descriptor = os.open(resolved_workspace, _DIRECTORY_OPEN_FLAGS)
    except (OSError, RuntimeError):
        return []

    candidates: list[tuple[int, str, str]] = []
    scanned_total = 0
    try:
        for relative_directory in directories:
            if scanned_total >= ARTIFACT_SCAN_MAX_TOTAL:
                break
            directory_descriptor = _open_directory_beneath(
                root_descriptor, relative_directory
            )
            if directory_descriptor is None:
                continue
            try:
                with os.scandir(directory_descriptor) as iterator:
                    for index, entry in enumerate(iterator):
                        if index >= ARTIFACT_SCAN_MAX_PER_DIRECTORY:
                            break
                        scanned_total += 1
                        if scanned_total > ARTIFACT_SCAN_MAX_TOTAL:
                            break
                        basename = entry.name
                        if not basename or basename.startswith("."):
                            continue
                        try:
                            before = os.stat(
                                basename,
                                dir_fd=directory_descriptor,
                                follow_symlinks=False,
                            )
                        except OSError:
                            continue
                        body = _read_stable_artifact(
                            directory_descriptor, basename, before
                        )
                        if body is None:
                            continue
                        artifact_name = f"{relative_directory}/{basename}"
                        if (
                            relative_directory == "research"
                            and basename.startswith("source_")
                        ):
                            preview = "verified source artifact (body hidden)"
                        else:
                            text = body.decode("utf-8", errors="replace")
                            lines = [
                                " ".join(line.split())
                                for line in text.splitlines()
                            ]
                            first = next(
                                (line.lstrip("# ") for line in lines if line),
                                "empty",
                            )
                            preview = compact(first, maximum=120)
                        candidates.append(
                            (before.st_mtime_ns, artifact_name, preview)
                        )
            except OSError:
                continue
            finally:
                os.close(directory_descriptor)
    finally:
        os.close(root_descriptor)

    candidates.sort(key=lambda value: (-value[0], value[1]))
    return [
        terminal_safe_text(f"{relative} — {preview}")
        for _, relative, preview in candidates[:maximum]
    ]


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
    name, workspace = workspace_for(home)
    values = report_values(
        IMMUTABLE_OPERATOR_ROOT / "report-edge-appliance", workspace, minutes
    )
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
        "PRIVATE  voluntary-self-study={}/{}  notebook={} observations={} latest-age={}ms".format(
            values.get("introspection_window_completed_calls", "0"),
            values.get("introspection_window_requested_calls", "0"),
            values.get("perceptual_notebook_enabled", "unknown"),
            values.get("perception_window_observations", "0"),
            values.get("perception_latest_age_ms", "—"),
        )
    )
    print(
        "SCHEDULED introspection={} last={} authored={}/{} failures={} next={}".format(
            values.get("scheduled_introspection_enabled", "unknown"),
            values.get("scheduled_introspection_last_status", "none"),
            values.get("scheduled_introspection_window_authored", "0"),
            values.get("scheduled_introspection_window_receipts", "0"),
            values.get("scheduled_introspection_consecutive_failures", "0"),
            values.get("scheduled_introspection_next_due_at_unix_ms", "—"),
        )
    )
    print(
        "SELF-CHANGE enabled={} mode={} pipeline={} intent={} patch={}/{}L build={} activation={} probation={} rollback={}".format(
            values.get("self_change_enabled", "false"),
            values.get("self_change_mode", "unavailable"),
            values.get("self_change_operator_pipeline_phase", "unavailable"),
            values.get("self_change_latest_intent_candidate_id", "none"),
            values.get("self_change_latest_patch_file_count", "0"),
            values.get("self_change_latest_patch_changed_lines", "0"),
            values.get("self_change_latest_build_status", "none"),
            values.get("self_change_latest_activation_status", "none"),
            values.get("self_change_probation_status", "none"),
            values.get("self_change_latest_rollback_status", "none"),
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

    print("\nAUTHORSHIP MAP")
    print("  voluntary Actions/JOURNAL = model declaration executed through bounded Action authority")
    print("  scheduled introspection = model_authored_runtime_scheduled; runtime chose only the cadence")
    print("  machine evidence = tools, studies, perception, spectral derivations; not Astrid-authored")
    print("  fallback/recovery and operator harness = explicitly non-authored")

    print("\nSCHEDULED INTROSPECTION (owner-private verified excerpt)")
    if values.get("scheduled_introspection_state_present") != "true":
        print("  No scheduled-introspection state has been recorded yet.")
    else:
        print(
            "  status={} running={} attempts={} authored={} non-authored-window={}".format(
                values.get("scheduled_introspection_last_status", "none"),
                values.get("scheduled_introspection_running", "false"),
                values.get("scheduled_introspection_total_attempts", "0"),
                values.get("scheduled_introspection_total_authored", "0"),
                values.get(
                    "scheduled_introspection_window_non_authored_excluded", "0"
                ),
            )
        )
        print(
            "  latest path={}  response-hash={}  continuity={}/{}".format(
                compact(
                    values.get("scheduled_introspection_latest_reflection_path"),
                    maximum=120,
                ),
                compact(
                    values.get("scheduled_introspection_latest_response_sha256"),
                    maximum=20,
                ),
                values.get("scheduled_introspection_continuity_present", "false"),
                values.get("scheduled_introspection_continuity_provenance", "none"),
            )
        )
        if values.get("scheduled_introspection_continuity_integrity_valid") == "true":
            print(
                "  summary="
                + compact(
                    values.get("scheduled_introspection_continuity_summary"),
                    maximum=320,
                )
            )
            print(
                "  reflection="
                + compact(
                    values.get("scheduled_introspection_verified_reflection_excerpt"),
                    maximum=800,
                )
            )
            print(
                "  authority={} truncated={}".format(
                    values.get(
                        "scheduled_introspection_reflection_text_authority",
                        "unavailable",
                    ),
                    values.get(
                        "scheduled_introspection_verified_reflection_excerpt_truncated",
                        "false",
                    ),
                )
            )

    print("\nSELF-CHANGE SUPERVISOR (metadata only; no source, diff, or build-log bodies)")
    if values.get("self_change_state_present") != "true":
        print("  No supervisor lifecycle state is available.")
    else:
        print(
            "  mode={} revision={} active={} previous={} integrity={}".format(
                values.get("self_change_mode", "unavailable"),
                values.get("self_change_state_revision", "0"),
                values.get("self_change_active_generation", "none"),
                values.get("self_change_previous_generation", "none"),
                values.get("self_change_state_integrity", "not-reverified"),
            )
        )
        print(
            "  intents={} latest={} provenance={}  build={}/{}".format(
                values.get("self_change_intent_total", "0"),
                values.get("self_change_latest_intent_candidate_id", "none"),
                values.get("self_change_latest_intent_provenance", "none"),
                values.get("self_change_latest_build_status", "none"),
                values.get("self_change_latest_build_id", "none"),
            )
        )
        print(
            "  activation={} generation={}  probation={} checks={}  rollback={}".format(
                values.get("self_change_latest_activation_status", "none"),
                values.get("self_change_latest_activation_generation", "none"),
                values.get("self_change_probation_status", "none"),
                values.get("self_change_probation_health_checks", "0"),
                values.get("self_change_latest_rollback_status", "none"),
            )
        )
        print(
            "  expected-service-restart phase={} upper-bound={}s basis={}".format(
                values.get("self_change_expected_restart_phase", "unavailable"),
                values.get(
                    "self_change_expected_restart_maximum_seconds", "unavailable"
                ),
                values.get("self_change_expected_restart_basis", "unavailable"),
            )
        )
        print(
            "  patch-export candidate={} terminal={} files={} changed-lines={} bodies-retained={}".format(
                values.get("self_change_latest_patch_candidate_id", "none"),
                values.get("self_change_latest_patch_terminal_status", "none"),
                values.get("self_change_latest_patch_file_count", "0"),
                values.get("self_change_latest_patch_changed_lines", "0"),
                values.get(
                    "self_change_latest_patch_source_bodies_retained",
                    "not_applicable",
                ),
            )
        )
        print(
            "  touched={}".format(
                compact(
                    values.get("self_change_latest_patch_touched_paths", "none"),
                    maximum=180,
                )
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
    lines = activity(
        IMMUTABLE_OPERATOR_ROOT / "report-edge-activity", workspace, minutes, limit
    )
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
    if "--candidate-presentation" in sys.argv[1:]:
        return candidate_presentation()
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
