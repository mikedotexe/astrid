"""Dependency-ordered projection generations with explicit receipts."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from typing import Any, Callable, Iterable, Sequence
import uuid

from .config import ControlConfig
from .errors import ProjectionError
from .evidence import verify_evidence
from .model import (
    atomic_write_json,
    authority_state,
    load_json,
    sha256_bytes,
    utc_now,
)


@dataclass(frozen=True)
class ProjectionCommand:
    argv: tuple[str, ...]


@dataclass(frozen=True)
class ProjectionStep:
    step_id: str
    dependencies: tuple[str, ...]
    commands: tuple[ProjectionCommand, ...]
    input_streams: tuple[str, ...]
    source_globs: tuple[str, ...]
    outputs: tuple[str, ...]


@dataclass(frozen=True)
class CommandResult:
    return_code: int
    stdout: bytes
    stderr: bytes
    duration_ms: int
    timed_out: bool = False


def command(*argv: str) -> ProjectionCommand:
    return ProjectionCommand(tuple(argv))


def source_first_steps() -> tuple[ProjectionStep, ...]:
    python = sys.executable
    return (
        ProjectionStep(
            "addressing",
            (),
            (
                command(
                    python,
                    "scripts/introspection_addressing_audit.py",
                    "--workspace",
                    "{workspace}",
                    "--state-dir",
                    "{workspace}/diagnostics/introspection_addressing_v1",
                    "inventory",
                    "--cutoff",
                    "latest",
                    "--json",
                    "--write",
                ),
            ),
            ("addressing",),
            ("introspections/introspection_*.txt",),
            (
                "diagnostics/introspection_addressing_v1/status.json",
                "diagnostics/introspection_addressing_v1/queue.md",
            ),
        ),
        ProjectionStep(
            "sandbox",
            ("addressing",),
            (
                command(
                    python,
                    "scripts/sandbox_trial_queue.py",
                    "--state-dir",
                    "{workspace}/diagnostics/sandbox_trial_queue_v1",
                    "generate",
                    "--json",
                    "--write",
                ),
                command(
                    python,
                    "scripts/sandbox_trial_queue.py",
                    "--state-dir",
                    "{workspace}/diagnostics/sandbox_trial_queue_v1",
                    "report",
                    "--json",
                ),
            ),
            ("addressing", "sandbox"),
            (
                "diagnostics/introspection_addressing_v1/status.json",
                "diagnostics/sandbox_trial_queue_v1/results/*.json",
            ),
            (
                "diagnostics/sandbox_trial_queue_v1/status.json",
                "diagnostics/sandbox_trial_queue_v1/queue.md",
            ),
        ),
        ProjectionStep(
            "corridor",
            ("addressing", "sandbox"),
            (
                command(
                    python,
                    "scripts/agency_corridor.py",
                    "--state-dir",
                    "{workspace}/diagnostics/agency_corridor_v1",
                    "generate",
                    "--write",
                    "--json",
                ),
                command(
                    python,
                    "scripts/agency_corridor.py",
                    "--state-dir",
                    "{workspace}/diagnostics/agency_corridor_v1",
                    "leases",
                    "generate",
                    "--write",
                    "--json",
                ),
                command(
                    python,
                    "scripts/agency_corridor.py",
                    "--state-dir",
                    "{workspace}/diagnostics/agency_corridor_v1",
                    "leases",
                    "report",
                    "--json",
                ),
                command(
                    python,
                    "scripts/agency_corridor.py",
                    "--state-dir",
                    "{workspace}/diagnostics/agency_corridor_v1",
                    "queue",
                    "generate",
                    "--write",
                    "--json",
                ),
                command(
                    python,
                    "scripts/agency_corridor.py",
                    "--state-dir",
                    "{workspace}/diagnostics/agency_corridor_v1",
                    "queue",
                    "report",
                    "--json",
                ),
                command(
                    python,
                    "scripts/agency_corridor.py",
                    "--state-dir",
                    "{workspace}/diagnostics/agency_corridor_v1",
                    "programs",
                    "generate",
                    "--write",
                    "--json",
                ),
                command(
                    python,
                    "scripts/agency_corridor.py",
                    "--state-dir",
                    "{workspace}/diagnostics/agency_corridor_v1",
                    "programs",
                    "report",
                    "--json",
                ),
                command(
                    python,
                    "scripts/agency_corridor.py",
                    "--state-dir",
                    "{workspace}/diagnostics/agency_corridor_v1",
                    "portfolio",
                    "report",
                    "--json",
                ),
            ),
            ("addressing", "sandbox", "corridor_v1", "corridor_v2"),
            (
                "diagnostics/introspection_addressing_v1/status.json",
                "diagnostics/sandbox_trial_queue_v1/status.json",
            ),
            (
                "diagnostics/agency_corridor_v2/status.json",
                "diagnostics/agency_corridor_v2/leases.json",
                "diagnostics/agency_corridor_v2/queue.json",
                "diagnostics/agency_corridor_v2/programs.json",
                "diagnostics/agency_corridor_v2/report.md",
            ),
        ),
        ProjectionStep(
            "signal_spine",
            ("corridor",),
            (
                command(
                    python,
                    "scripts/signal_spine_projector.py",
                    "--workspace",
                    "{workspace}",
                    "--json",
                    "generate",
                    "--write",
                ),
                command(
                    python,
                    "scripts/signal_spine_projector.py",
                    "--workspace",
                    "{workspace}",
                    "--json",
                    "report",
                ),
            ),
            ("signal_spine",),
            (
                "diagnostics/signal_spine_v1/journeys/*.json",
                "diagnostics/signal_spine_v1/temporal_associations/*.json",
                "diagnostics/signal_spine_v1/capture_gaps/*.json",
            ),
            (
                "diagnostics/signal_spine_v1/projection_status.json",
                "diagnostics/signal_spine_v1/report.md",
            ),
        ),
        ProjectionStep(
            "claim_families",
            ("addressing", "signal_spine"),
            (
                command(
                    python,
                    "scripts/claim_families.py",
                    "--workspace",
                    "{workspace}",
                    "--json",
                    "generate",
                    "--write",
                ),
                command(
                    python,
                    "scripts/claim_families.py",
                    "--workspace",
                    "{workspace}",
                    "--json",
                    "report",
                ),
            ),
            ("addressing", "claim_families"),
            (
                "diagnostics/introspection_addressing_v1/status.json",
                "environment_receipts/latest_environment_receipt.json",
            ),
            (
                "diagnostics/claim_families_v1/status.json",
                "diagnostics/claim_families_v1/report.md",
                "diagnostics/claim_families_v1/migration_receipt.json",
            ),
        ),
        ProjectionStep(
            "experiment_dossiers",
            ("sandbox", "claim_families"),
            (
                command(
                    python,
                    "scripts/experiment_dossiers.py",
                    "--workspace",
                    "{workspace}",
                    "--json",
                    "generate",
                    "--write",
                ),
                command(
                    python,
                    "scripts/experiment_dossiers.py",
                    "--workspace",
                    "{workspace}",
                    "--json",
                    "report",
                ),
            ),
            ("sandbox", "claim_families"),
            (
                "diagnostics/sandbox_trial_queue_v1/status.json",
                "diagnostics/claim_families_v1/status.json",
            ),
            (
                "diagnostics/experiment_dossiers_v1/status.json",
                "diagnostics/experiment_dossiers_v1/report.md",
            ),
        ),
        ProjectionStep(
            "authority_temporal",
            ("experiment_dossiers",),
            (
                command(
                    python,
                    "scripts/authority_temporal_audit.py",
                    "--workspace",
                    "{workspace}",
                    "--json",
                    "generate",
                    "--write",
                ),
            ),
            ("authority_lifecycle",),
            ("action_threads/threads/*/authority_gate.jsonl",),
            ("diagnostics/authority_temporal_v1/status.json",),
        ),
        ProjectionStep(
            "model_qos",
            ("authority_temporal",),
            (
                command(
                    python,
                    "scripts/model_qos_projector.py",
                    "--workspace",
                    "{workspace}",
                    "--json",
                    "generate",
                    "--write",
                ),
            ),
            ("model_qos",),
            (
                "../../../../neural-triple-reservoir/workspace/model_qos_receipts.jsonl",
            ),
            ("diagnostics/model_qos_v1/status.json",),
        ),
        ProjectionStep(
            "felt_contracts",
            (
                "experiment_dossiers",
                "corridor",
                "signal_spine",
                "authority_temporal",
                "model_qos",
            ),
            (
                command(
                    python,
                    "scripts/felt_contract_graph.py",
                    "--workspace",
                    "{workspace}",
                    "generate",
                    "--write",
                    "--actor",
                    "steward-control-projector",
                    "--json",
                ),
                command(
                    python,
                    "scripts/felt_contract_graph.py",
                    "--workspace",
                    "{workspace}",
                    "report",
                    "--json",
                ),
            ),
            (
                "addressing",
                "sandbox",
                "corridor_v1",
                "corridor_v2",
                "signal_spine",
                "claim_families",
                "authority_lifecycle",
                "model_qos",
                "felt_contracts",
            ),
            (
                "diagnostics/claim_families_v1/status.json",
                "diagnostics/experiment_dossiers_v1/status.json",
                "environment_receipts/environment_receipts.jsonl",
            ),
            (
                "diagnostics/felt_contract_graph_v1/status.json",
                "diagnostics/felt_contract_graph_v1/contracts.jsonl",
                "diagnostics/felt_contract_graph_v1/report.md",
                "diagnostics/felt_contract_graph_v1/migration_receipt.json",
            ),
        ),
    )


def _default_runner(
    argv: Sequence[str],
    *,
    cwd: Path,
    timeout: int,
) -> CommandResult:
    started = time.monotonic()
    env = dict(os.environ)
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    try:
        result = subprocess.run(
            list(argv),
            cwd=cwd,
            env=env,
            capture_output=True,
            check=False,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as error:
        return CommandResult(
            return_code=124,
            stdout=error.stdout or b"",
            stderr=error.stderr or b"",
            duration_ms=int((time.monotonic() - started) * 1000),
            timed_out=True,
        )
    return CommandResult(
        return_code=result.returncode,
        stdout=result.stdout,
        stderr=result.stderr,
        duration_ms=int((time.monotonic() - started) * 1000),
    )


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def hash_source_globs(workspace: Path, globs: Iterable[str]) -> dict[str, str]:
    result: dict[str, str] = {}
    for pattern in sorted(globs):
        paths = sorted(path for path in workspace.glob(pattern) if path.is_file())
        digest = hashlib.sha256()
        for path in paths:
            relative = str(path.relative_to(workspace))
            digest.update(relative.encode("utf-8"))
            digest.update(b"\0")
            digest.update(_sha256_file(path).encode("ascii"))
            digest.update(b"\n")
        result[pattern] = digest.hexdigest()
    return result


def hash_outputs(workspace: Path, outputs: Iterable[str]) -> dict[str, str]:
    hashes: dict[str, str] = {}
    for relative in sorted(outputs):
        path = workspace / relative
        if not path.is_file():
            raise ProjectionError(f"required projection output missing: {relative}")
        hashes[relative] = _sha256_file(path)
    return hashes


def audit_projection_outputs(
    workspace: Path,
    steps: Iterable[ProjectionStep],
) -> dict[str, Any]:
    try:
        from authority_state import assert_artifact_authority_tree
    except ModuleNotFoundError:
        from scripts.authority_state import assert_artifact_authority_tree

    files: dict[str, dict[str, Any]] = {}
    valid = True
    for relative in sorted(
        {
            output
            for step in steps
            for output in step.outputs
            if output.endswith(".json")
        }
    ):
        path = workspace / relative
        record: dict[str, Any] = {"exists": path.is_file()}
        if not path.is_file():
            valid = False
            files[relative] = record
            continue
        try:
            value = json.loads(path.read_text(encoding="utf-8"))
            if not isinstance(value, dict):
                raise ValueError("projection JSON must be an object")
            assert_artifact_authority_tree(value)
        except Exception as error:
            record.update(
                {
                    "valid_json": False,
                    "error_type": type(error).__name__,
                }
            )
            valid = False
            files[relative] = record
            continue
        record["valid_json"] = True
        corrupt = int(value.get("corrupt_event_lines") or 0)
        record["corrupt_event_lines"] = corrupt
        if corrupt:
            valid = False
        counter = value.get("counter_audit")
        if isinstance(counter, dict):
            checks = counter.get("checks")
            if isinstance(checks, dict):
                consistent = bool(checks) and all(
                    check is True for check in checks.values()
                )
            else:
                boolean_checks = [
                    item for item in counter.values() if isinstance(item, bool)
                ]
                consistent = bool(boolean_checks) and all(boolean_checks)
            if counter.get("status") not in {None, "consistent"}:
                consistent = False
            record["counter_audit_present"] = True
            record["counter_audit_consistent"] = consistent
            valid = valid and consistent
        else:
            record["counter_audit_present"] = False
        files[relative] = record
    return {
        "schema": "projection_counter_audit_v1",
        "schema_version": 1,
        "valid": valid,
        "files": files,
    }


def _parse_json_output(result: CommandResult, step_id: str) -> dict[str, Any]:
    if result.return_code:
        marker = "timeout" if result.timed_out else f"exit_{result.return_code}"
        raise ProjectionError(f"{step_id} command failed: {marker}")
    try:
        value = json.loads(result.stdout)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise ProjectionError(f"{step_id} command did not emit JSON") from error
    if not isinstance(value, dict):
        raise ProjectionError(f"{step_id} command JSON must be an object")
    try:
        from authority_state import assert_artifact_authority_tree
    except ModuleNotFoundError:
        from scripts.authority_state import assert_artifact_authority_tree

    assert_artifact_authority_tree(value)
    return value


def _command_receipt(
    command_value: ProjectionCommand,
    result: CommandResult,
    parsed: dict[str, Any],
) -> dict[str, Any]:
    executable = Path(command_value.argv[0]).name if command_value.argv else "unknown"
    script = (
        Path(command_value.argv[1]).name
        if len(command_value.argv) > 1
        else executable
    )
    return {
        "schema": "projection_command_receipt_v1",
        "schema_version": 1,
        "command_id": f"{executable}:{script}",
        "argv_sha256": sha256_bytes(
            json.dumps(command_value.argv, separators=(",", ":")).encode()
        ),
        "return_code": result.return_code,
        "duration_ms": result.duration_ms,
        "timed_out": result.timed_out,
        "stdout_sha256": sha256_bytes(result.stdout),
        "stderr_sha256": sha256_bytes(result.stderr),
        "output_schema": parsed.get("schema"),
        "raw_output_included": False,
    }


def _resolve_argv(
    command_value: ProjectionCommand,
    config: ControlConfig,
) -> tuple[str, ...]:
    replacements = {
        "{repo_root}": str(config.repo_root),
        "{workspace}": str(config.workspace),
        "{state_root}": str(config.state_root),
        "{store_root}": str(config.store_root),
    }
    resolved: list[str] = []
    for raw_value in command_value.argv:
        value = raw_value
        for marker, replacement in replacements.items():
            value = value.replace(marker, replacement)
        resolved.append(value)
    return tuple(resolved)


class ProjectionCoordinator:
    VERSION = 1

    def __init__(
        self,
        config: ControlConfig,
        *,
        steps: Sequence[ProjectionStep] | None = None,
        runner: Callable[..., CommandResult] | None = None,
    ):
        self.config = config
        self.steps = tuple(source_first_steps() if steps is None else steps)
        self.runner = runner or _default_runner
        self.root = config.state_root / "projections"
        self.generations_root = self.root / "generations"
        self.failed_root = self.root / "failed_generations"
        self.latest_path = self.root / "latest_generation.json"

    def plan(self) -> dict[str, Any]:
        return {
            "schema": "projection_plan_v1",
            "schema_version": 1,
            "profile": self.config.profile,
            "steps": [
                {
                    "step_id": step.step_id,
                    "dependencies": list(step.dependencies),
                    "command_count": len(step.commands),
                    "input_streams": list(step.input_streams),
                    "source_globs": list(step.source_globs),
                    "outputs": list(step.outputs),
                }
                for step in self.steps
            ],
            "mutates": False,
        }

    def _validate_graph(self) -> None:
        seen: set[str] = set()
        for step in self.steps:
            if step.step_id in seen:
                raise ProjectionError(f"duplicate projection step: {step.step_id}")
            missing = [item for item in step.dependencies if item not in seen]
            if missing:
                raise ProjectionError(
                    f"{step.step_id} has unsatisfied dependencies: {missing}"
                )
            seen.add(step.step_id)

    def run(
        self,
        *,
        actor: str,
        run_id: str,
        phase: str,
    ) -> dict[str, Any]:
        if phase not in {"pre", "post", "manual"}:
            raise ProjectionError(f"unsupported projection phase: {phase}")
        generation_id = f"projection_{time.time_ns()}_{uuid.uuid4().hex[:10]}"
        previous = load_json(self.latest_path)
        step_receipts: list[dict[str, Any]] = []
        completed: set[str] = set()
        try:
            self._validate_graph()
            before = verify_evidence(self.config)
            for step in self.steps:
                if not set(step.dependencies).issubset(completed):
                    raise ProjectionError(
                        f"{step.step_id} dependencies did not complete"
                    )
                source_hashes = hash_source_globs(
                    self.config.workspace,
                    step.source_globs,
                )
                command_receipts = []
                for command_value in step.commands:
                    resolved_argv = _resolve_argv(command_value, self.config)
                    result = self.runner(
                        resolved_argv,
                        cwd=self.config.repo_root,
                        timeout=self.config.projector_timeout_secs,
                    )
                    parsed = _parse_json_output(result, step.step_id)
                    command_receipts.append(
                        _command_receipt(
                            ProjectionCommand(resolved_argv),
                            result,
                            parsed,
                        )
                    )
                output_hashes = hash_outputs(self.config.workspace, step.outputs)
                try:
                    from evidence_store import EvidenceEventStore
                except ModuleNotFoundError:
                    from scripts.evidence_store import EvidenceEventStore

                store = EvidenceEventStore(self.config.store_root)
                input_stream_watermarks = store.stream_watermarks(
                    step.input_streams
                )
                checkpoint = store.write_checkpoint(
                    f"steward_{step.step_id}_v1",
                    self.VERSION,
                    output_hashes,
                    input_streams=step.input_streams,
                    source_hashes=source_hashes,
                )
                step_receipts.append(
                    {
                        "schema": "projection_step_receipt_v1",
                        "schema_version": 1,
                        "step_id": step.step_id,
                        "dependencies": list(step.dependencies),
                        "commands": command_receipts,
                        "input_streams": list(step.input_streams),
                        "input_stream_watermarks": input_stream_watermarks,
                        "source_hashes": source_hashes,
                        "output_hashes": output_hashes,
                        "checkpoint": checkpoint.name,
                        "status": "passed",
                    }
                )
                completed.add(step.step_id)
            counter_audit = audit_projection_outputs(
                self.config.workspace,
                self.steps,
            )
            if not counter_audit["valid"]:
                raise ProjectionError("projection output counter audit failed")
            after = verify_evidence(self.config)
            manifest = {
                "schema": "projection_generation_manifest_v1",
                "schema_version": 1,
                "generation_id": generation_id,
                "coordinator_version": self.VERSION,
                "profile": self.config.profile,
                "phase": phase,
                "run_id": run_id,
                "actor": actor,
                "status": "passed",
                "recorded_at": utc_now(),
                "previous_successful_generation_id": (
                    previous.get("generation_id") if previous else None
                ),
                "evidence_before": {
                    "last_global_seq": before["last_global_seq"],
                    "last_event_sha256": before["last_event_sha256"],
                },
                "evidence_after": {
                    "last_global_seq": after["last_global_seq"],
                    "last_event_sha256": after["last_event_sha256"],
                },
                "steps": step_receipts,
                "counter_audit": counter_audit,
                "authority_scan_passed": True,
                "raw_output_included": False,
                "artifact_authority_state_v1": authority_state(),
            }
            generation_path = self.generations_root / f"{generation_id}.json"
            atomic_write_json(generation_path, manifest)
            atomic_write_json(self.latest_path, manifest)
            return manifest
        except Exception as error:
            failed = {
                "schema": "projection_generation_manifest_v1",
                "schema_version": 1,
                "generation_id": generation_id,
                "coordinator_version": self.VERSION,
                "profile": self.config.profile,
                "phase": phase,
                "run_id": run_id,
                "actor": actor,
                "status": "failed",
                "recorded_at": utc_now(),
                "previous_successful_generation_id": (
                    previous.get("generation_id") if previous else None
                ),
                "completed_steps": step_receipts,
                "error_type": type(error).__name__,
                "error_sha256": sha256_bytes(str(error).encode()),
                "raw_output_included": False,
                "artifact_authority_state_v1": authority_state(),
            }
            atomic_write_json(
                self.failed_root / f"{generation_id}.json",
                failed,
            )
            if isinstance(error, ProjectionError):
                raise
            raise ProjectionError(f"projection generation failed: {type(error).__name__}") from error
