"""Dependency-ordered projection generations with explicit receipts."""

from __future__ import annotations

from dataclasses import dataclass, field
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
from typing import Any, Callable, Iterable, Sequence
import uuid

from .config import ControlConfig
from .errors import ProjectionCancelledError, ProjectionError
from .evidence import verify_evidence
from .model import (
    atomic_write_json,
    authority_state,
    load_json,
    sha256_bytes,
    utc_now,
)
from .projection_state import (
    ProjectionJournalStore,
    bounded_receipt,
    command_identity,
    flatten_dependency_hashes,
    plan_identity,
    projection_input_identity,
)
from .projection_profile import source_first_steps
from .projection_types import (
    CommandResult,
    ProjectionCommand,
    ProjectionStep,
    command,
)


MAX_PROJECTOR_RECEIPT_BYTES = 1024 * 1024
MAX_PROJECTOR_STDERR_BYTES = 64 * 1024


@dataclass
class _GenerationRun:
    actor: str
    run_id: str
    phase: str
    full_rebuild: bool
    control: Callable[[dict[str, Any] | None], bool] | None
    generation_id: str
    started: float
    previous: dict[str, Any] | None
    plan_sha256: str
    step_receipts: list[dict[str, Any]] = field(default_factory=list)
    outputs_by_step: dict[str, dict[str, str]] = field(default_factory=dict)
    completed: set[str] = field(default_factory=set)
    reused_count: int = 0
    resume_source: str | None = None
    resume_steps: set[str] = field(default_factory=set)
    resume_rejection: str | None = None
    session_path: Path | None = None
    before: dict[str, Any] | None = None


def _default_runner(
    argv: Sequence[str],
    *,
    cwd: Path,
    timeout: int,
    poll_callback: Callable[[dict[str, Any]], bool] | None = None,
    poll_interval_secs: float = 0.25,
    env_overrides: dict[str, str] | None = None,
) -> CommandResult:
    started = time.monotonic()
    env = dict(os.environ)
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    env.update(env_overrides or {})
    stdout_file = tempfile.TemporaryFile()
    stderr_file = tempfile.TemporaryFile()
    process = subprocess.Popen(
        list(argv),
        cwd=cwd,
        env=env,
        stdout=stdout_file,
        stderr=stderr_file,
    )
    deadline = started + timeout
    timed_out = False
    cancelled = False
    interrupted = False
    while process.poll() is None:
        progress = {
            "child_pid": process.pid,
            "command_elapsed_ms": int((time.monotonic() - started) * 1000),
        }
        if poll_callback and poll_callback(progress):
            cancelled = True
        if time.monotonic() >= deadline:
            timed_out = True
        if (cancelled or timed_out) and not interrupted:
            process.send_signal(signal.SIGINT)
            interrupted = True
        time.sleep(poll_interval_secs)
    if poll_callback:
        poll_callback(
            {
                "child_pid": process.pid,
                "command_elapsed_ms": int((time.monotonic() - started) * 1000),
                "child_exited": True,
            }
        )
    stdout_file.seek(0)
    stderr_file.seek(0)
    stdout = stdout_file.read(MAX_PROJECTOR_RECEIPT_BYTES + 1)
    stderr = stderr_file.read(MAX_PROJECTOR_STDERR_BYTES)
    output_too_large = len(stdout) > MAX_PROJECTOR_RECEIPT_BYTES
    if output_too_large:
        stdout = b""
    stdout_file.close()
    stderr_file.close()
    if timed_out:
        return CommandResult(
            return_code=124,
            stdout=stdout,
            stderr=stderr,
            duration_ms=int((time.monotonic() - started) * 1000),
            timed_out=True,
            output_too_large=output_too_large,
        )
    return CommandResult(
        return_code=process.returncode,
        stdout=stdout,
        stderr=stderr,
        duration_ms=int((time.monotonic() - started) * 1000),
        cancelled=cancelled,
        output_too_large=output_too_large,
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
    if result.cancelled:
        raise ProjectionCancelledError(
            f"{step_id} command cancelled after pause request"
        )
    if result.return_code:
        marker = "timeout" if result.timed_out else f"exit_{result.return_code}"
        raise ProjectionError(f"{step_id} command failed: {marker}")
    if result.output_too_large:
        raise ProjectionError(
            f"{step_id} command exceeded the bounded receipt limit"
        )
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
        "output_too_large": result.output_too_large,
        "stdout_sha256": sha256_bytes(result.stdout),
        "stderr_sha256": sha256_bytes(result.stderr),
        "output_schema": parsed.get("schema"),
        "projector_receipt": bounded_receipt(parsed),
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
    VERSION = 3

    def __init__(
        self,
        config: ControlConfig,
        *,
        steps: Sequence[ProjectionStep] | None = None,
        runner: Callable[..., CommandResult] | None = None,
        store: Any | None = None,
    ):
        self.config = config
        self.steps = tuple(source_first_steps() if steps is None else steps)
        self.runner = runner or _default_runner
        self._uses_default_runner = runner is None
        self.root = config.state_root / "projections"
        self.generations_root = self.root / "generations"
        self.failed_root = self.root / "failed_generations"
        self.latest_path = self.root / "latest_generation.json"
        self.journals = ProjectionJournalStore(self.root / "journals")
        if store is None:
            try:
                from evidence_store import EvidenceEventStore
            except ModuleNotFoundError:
                from scripts.evidence_store import EvidenceEventStore

            store = EvidenceEventStore(config.store_root)
        self.store = store

    def plan(self) -> dict[str, Any]:
        return {
            "schema": "projection_plan_v3",
            "schema_version": 3,
            "profile": self.config.profile,
            "coordinator_version": self.VERSION,
            "steps": [
                {
                    "step_id": step.step_id,
                    "dependencies": list(step.dependencies),
                    "commands": [
                        list(_resolve_argv(item, self.config))
                        for item in step.commands
                    ],
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

    def _resolved_commands(
        self,
        step: ProjectionStep,
    ) -> tuple[tuple[str, ...], ...]:
        return tuple(
            _resolve_argv(command_value, self.config)
            for command_value in step.commands
        )

    def _identity(
        self,
        store: Any,
        step: ProjectionStep,
        outputs_by_step: dict[str, dict[str, str]],
    ) -> dict[str, Any]:
        commands = self._resolved_commands(step)
        return projection_input_identity(
            input_streams=step.input_streams,
            input_stream_watermarks=store.stream_watermarks(
                step.input_streams
            ),
            source_hashes=hash_source_globs(
                self.config.workspace,
                step.source_globs,
            ),
            dependency_output_hashes=flatten_dependency_hashes(
                step.dependencies,
                outputs_by_step,
            ),
            command_sha256=command_identity(commands),
            config_sha256=self.config.config_sha256,
            projector_version=self.VERSION,
        )

    def _reuse_decision(
        self,
        store: Any,
        step: ProjectionStep,
        outputs_by_step: dict[str, dict[str, str]],
        *,
        full_rebuild: bool,
    ) -> tuple[bool, str, dict[str, Any], dict[str, str]]:
        identity = self._identity(store, step, outputs_by_step)
        if full_rebuild:
            return False, "full_rebuild_requested", identity, {}
        checkpoint = store.read_checkpoint(f"steward_{step.step_id}_v1")
        if not checkpoint:
            return False, "checkpoint_missing", identity, {}
        if checkpoint.get("schema") != "evidence_event_projection_checkpoint_v3":
            return False, "checkpoint_requires_v3_migration", identity, {}
        if checkpoint.get("input_identity") != identity:
            return False, "input_identity_changed", identity, {}
        try:
            output_hashes = hash_outputs(
                self.config.workspace,
                step.outputs,
            )
        except ProjectionError:
            return False, "output_missing", identity, {}
        if checkpoint.get("output_hashes") != output_hashes:
            return False, "output_hash_changed", identity, output_hashes
        audit = audit_projection_outputs(self.config.workspace, (step,))
        if not audit["valid"]:
            return False, "output_validation_failed", identity, output_hashes
        return True, "identity_and_outputs_match", identity, output_hashes

    def explain(
        self,
        *,
        full_rebuild: bool = False,
        resume_generation: str | None = None,
    ) -> dict[str, Any]:
        self._validate_graph()
        store = self.store
        evidence = verify_evidence(self.config, store=store)
        outputs_by_step: dict[str, dict[str, str]] = {}
        decisions = []
        for step in self.steps:
            reusable, reason, identity, output_hashes = self._reuse_decision(
                store,
                step,
                outputs_by_step,
                full_rebuild=full_rebuild,
            )
            decisions.append(
                {
                    "step_id": step.step_id,
                    "action": "reuse" if reusable else "execute",
                    "reason": reason,
                    "input_identity": identity,
                    "output_hashes": output_hashes,
                }
            )
            if output_hashes:
                outputs_by_step[step.step_id] = output_hashes
        resume = self.journals.load(resume_generation) if resume_generation else None
        compatible = None
        if resume_generation:
            compatible = ProjectionJournalStore.resumable(
                resume or {},
                plan_sha256=plan_identity(self.plan()),
                config_sha256=self.config.config_sha256,
            )
        return {
            "schema": "projection_explanation_v1",
            "schema_version": 1,
            "profile": self.config.profile,
            "full_rebuild": full_rebuild,
            "resume_generation": resume_generation,
            "resume_compatibility": compatible,
            "evidence_head": {
                "last_global_seq": evidence["last_global_seq"],
                "last_event_sha256": evidence["last_event_sha256"],
            },
            "steps": decisions,
            "mutates": False,
        }

    def _reuse_receipt(
        self,
        step: ProjectionStep,
        identity: dict[str, Any],
        output_hashes: dict[str, str],
        *,
        reason: str,
        resume_source: str | None,
    ) -> dict[str, Any]:
        return {
            "schema": "projection_step_receipt_v2",
            "schema_version": 2,
            "step_id": step.step_id,
            "dependencies": list(step.dependencies),
            "status": "reused",
            "reuse_reason": reason,
            "resume_source_generation_id": resume_source,
            "commands": [],
            "input_identity": identity,
            "output_hashes": output_hashes,
            "duration_ms": 0,
            "artifact_authority_state_v1": authority_state(),
            "raw_output_included": False,
        }

    def _write_journal(
        self,
        state: _GenerationRun,
        *,
        status: str = "running",
        active_step_id: str | None = None,
        command_receipts: list[dict[str, Any]] | None = None,
        reason: str | None = None,
    ) -> None:
        self.journals.write(
            generation_id=state.generation_id,
            plan_sha256=state.plan_sha256,
            config_sha256=self.config.config_sha256,
            phase=state.phase,
            run_id=state.run_id,
            actor=state.actor,
            status=status,
            step_receipts=state.step_receipts,
            active_step_id=active_step_id,
            command_receipts=command_receipts,
            resume_source_generation_id=state.resume_source,
            reason=state.resume_rejection if reason is None else reason,
        )

    @staticmethod
    def _stop_requested(
        state: _GenerationRun,
        progress: dict[str, Any],
    ) -> bool:
        return bool(state.control and state.control(progress))

    def _prepare_generation(
        self,
        state: _GenerationRun,
        *,
        resume_generation: str | None,
    ) -> None:
        self._validate_graph()
        if self._stop_requested(
            state,
            {
                "generation_id": state.generation_id,
                "status": "starting",
                "completed_step_count": 0,
                "reused_step_count": 0,
                "total_step_count": len(self.steps),
            },
        ):
            raise ProjectionCancelledError(
                "projection cancelled before verification"
            )
        state.before = verify_evidence(
            self.config,
            store=self.store,
            full_chain=False,
        )
        self.store.prepare_read_index()
        state.session_path = (
            self.root / "sessions" / f"{state.generation_id}.json"
        )
        atomic_write_json(
            state.session_path,
            {
                "schema": "evidence_projection_session_v1",
                "schema_version": 1,
                "generation_id": state.generation_id,
                "store_root": str(self.config.store_root.resolve()),
                "verified_global_seq": state.before["last_global_seq"],
                "verified_event_sha256": state.before[
                    "last_event_sha256"
                ],
                "expires_at_unix": time.time() + (4 * 60 * 60),
                "authority_source": False,
                "artifact_authority_state_v1": authority_state(),
            },
        )
        if resume_generation:
            source_journal = self.journals.load(resume_generation)
            resumable, resume_reason = ProjectionJournalStore.resumable(
                source_journal or {},
                plan_sha256=state.plan_sha256,
                config_sha256=self.config.config_sha256,
            )
            if resumable:
                expected_ids = [
                    str(item.get("step_id"))
                    for item in (source_journal or {}).get(
                        "step_receipts", []
                    )
                    if isinstance(item, dict)
                ]
                state.resume_steps = set(expected_ids)
                state.resume_source = resume_generation
            else:
                state.resume_rejection = resume_reason
        self._write_journal(state)

    def _run_step_commands(
        self,
        state: _GenerationRun,
        step: ProjectionStep,
    ) -> tuple[tuple[tuple[str, ...], ...], list[dict[str, Any]]]:
        command_receipts: list[dict[str, Any]] = []
        self._write_journal(
            state,
            active_step_id=step.step_id,
            command_receipts=command_receipts,
        )
        resolved_commands = self._resolved_commands(step)
        for resolved_argv in resolved_commands:
            command_id = (
                Path(resolved_argv[1]).name
                if len(resolved_argv) > 1
                else Path(resolved_argv[0]).name
            )
            if self._stop_requested(
                state,
                {
                    "generation_id": state.generation_id,
                    "step_id": step.step_id,
                    "command_id": command_id,
                    "completed_step_count": len(state.completed),
                    "reused_step_count": state.reused_count,
                },
            ):
                raise ProjectionCancelledError(
                    f"projection cancelled before {step.step_id} command"
                )
            if self._uses_default_runner:
                result = self.runner(
                    resolved_argv,
                    cwd=self.config.repo_root,
                    timeout=self.config.projector_timeout_secs,
                    poll_callback=state.control,
                    env_overrides={
                        "ASTRID_EVIDENCE_PROJECTION_SESSION": str(
                            state.session_path
                        )
                    },
                )
            else:
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
            self._write_journal(
                state,
                active_step_id=step.step_id,
                command_receipts=command_receipts,
            )
        return resolved_commands, command_receipts

    def _run_step(
        self,
        state: _GenerationRun,
        step: ProjectionStep,
    ) -> None:
        if self._stop_requested(
            state,
            {
                "generation_id": state.generation_id,
                "status": "running",
                "step_id": step.step_id,
                "command_id": None,
                "completed_step_count": len(state.completed),
                "reused_step_count": state.reused_count,
            },
        ):
            raise ProjectionCancelledError(
                f"projection cancelled before {step.step_id}"
            )
        if not set(step.dependencies).issubset(state.completed):
            raise ProjectionError(
                f"{step.step_id} dependencies did not complete"
            )
        reusable, reason, identity, output_hashes = self._reuse_decision(
            self.store,
            step,
            state.outputs_by_step,
            full_rebuild=state.full_rebuild,
        )
        if step.step_id in state.resume_steps and not reusable:
            state.resume_steps.clear()
            state.resume_rejection = (
                f"resume_step_invalid:{step.step_id}:{reason}"
            )
        if reusable:
            state.step_receipts.append(
                self._reuse_receipt(
                    step,
                    identity,
                    output_hashes,
                    reason=(
                        "resume_checkpoint_match"
                        if step.step_id in state.resume_steps
                        else reason
                    ),
                    resume_source=(
                        state.resume_source
                        if step.step_id in state.resume_steps
                        else None
                    ),
                )
            )
            state.outputs_by_step[step.step_id] = output_hashes
            state.completed.add(step.step_id)
            state.reused_count += 1
            self._write_journal(state)
            return

        step_started = time.monotonic()
        resolved_commands, command_receipts = self._run_step_commands(
            state,
            step,
        )
        output_hashes = hash_outputs(
            self.config.workspace,
            step.outputs,
        )
        output_audit = audit_projection_outputs(
            self.config.workspace,
            (step,),
        )
        if not output_audit["valid"]:
            raise ProjectionError(
                f"{step.step_id} output validation failed"
            )
        source_hashes = hash_source_globs(
            self.config.workspace,
            step.source_globs,
        )
        dependency_hashes = flatten_dependency_hashes(
            step.dependencies,
            state.outputs_by_step,
        )
        checkpoint = self.store.write_checkpoint(
            f"steward_{step.step_id}_v1",
            self.VERSION,
            output_hashes,
            input_streams=step.input_streams,
            source_hashes=source_hashes,
            dependency_output_hashes=dependency_hashes,
            command_sha256=command_identity(resolved_commands),
            config_sha256=self.config.config_sha256,
        )
        state.step_receipts.append(
            {
                "schema": "projection_step_receipt_v2",
                "schema_version": 2,
                "step_id": step.step_id,
                "dependencies": list(step.dependencies),
                "commands": command_receipts,
                "input_identity": self._identity(
                    self.store,
                    step,
                    state.outputs_by_step,
                ),
                "output_hashes": output_hashes,
                "output_audit": output_audit,
                "checkpoint": checkpoint.name,
                "status": "passed",
                "duration_ms": int(
                    (time.monotonic() - step_started) * 1000
                ),
                "artifact_authority_state_v1": authority_state(),
                "raw_output_included": False,
            }
        )
        state.outputs_by_step[step.step_id] = output_hashes
        state.completed.add(step.step_id)
        self._write_journal(state)

    def _publish_success(
        self,
        state: _GenerationRun,
    ) -> dict[str, Any]:
        counter_audit = audit_projection_outputs(
            self.config.workspace,
            self.steps,
        )
        if not counter_audit["valid"]:
            raise ProjectionError("projection output counter audit failed")
        after = verify_evidence(
            self.config,
            store=self.store,
            full_chain=False,
        )
        before = state.before or {}
        manifest = {
            "schema": "projection_generation_manifest_v2",
            "schema_version": 2,
            "generation_id": state.generation_id,
            "coordinator_version": self.VERSION,
            "profile": self.config.profile,
            "phase": state.phase,
            "run_id": state.run_id,
            "actor": state.actor,
            "status": "passed",
            "recorded_at": utc_now(),
            "duration_ms": int(
                (time.monotonic() - state.started) * 1000
            ),
            "previous_successful_generation_id": (
                state.previous.get("generation_id")
                if state.previous
                else None
            ),
            "resume_source_generation_id": state.resume_source,
            "resume_rejection_reason": state.resume_rejection,
            "full_rebuild": state.full_rebuild,
            "evidence_before": {
                "last_global_seq": before["last_global_seq"],
                "last_event_sha256": before["last_event_sha256"],
            },
            "evidence_after": {
                "last_global_seq": after["last_global_seq"],
                "last_event_sha256": after["last_event_sha256"],
            },
            "completed_step_count": len(state.step_receipts),
            "reused_step_count": state.reused_count,
            "executed_step_count": (
                len(state.step_receipts) - state.reused_count
            ),
            "steps": state.step_receipts,
            "counter_audit": counter_audit,
            "authority_scan_passed": True,
            "raw_output_included": False,
            "artifact_authority_state_v1": authority_state(),
        }
        atomic_write_json(
            self.generations_root / f"{state.generation_id}.json",
            manifest,
        )
        self._write_journal(state, status="passed")
        atomic_write_json(self.latest_path, manifest)
        if state.control:
            state.control(
                {
                    "generation_id": state.generation_id,
                    "status": "passed",
                    "completed_step_count": len(state.completed),
                    "reused_step_count": state.reused_count,
                }
            )
        return manifest

    def _record_failure(
        self,
        state: _GenerationRun,
        error: Exception,
    ) -> None:
        outcome = (
            "cancelled"
            if isinstance(error, ProjectionCancelledError)
            else "failed"
        )
        failed = {
            "schema": "projection_generation_manifest_v2",
            "schema_version": 2,
            "generation_id": state.generation_id,
            "coordinator_version": self.VERSION,
            "profile": self.config.profile,
            "phase": state.phase,
            "run_id": state.run_id,
            "actor": state.actor,
            "status": outcome,
            "recorded_at": utc_now(),
            "duration_ms": int(
                (time.monotonic() - state.started) * 1000
            ),
            "previous_successful_generation_id": (
                state.previous.get("generation_id")
                if state.previous
                else None
            ),
            "completed_steps": state.step_receipts,
            "error_type": type(error).__name__,
            "error_sha256": sha256_bytes(str(error).encode()),
            "raw_output_included": False,
            "artifact_authority_state_v1": authority_state(),
        }
        atomic_write_json(
            self.failed_root / f"{state.generation_id}.json",
            failed,
        )
        self._write_journal(
            state,
            status=outcome,
            reason=type(error).__name__,
        )

    def run(
        self,
        *,
        actor: str,
        run_id: str,
        phase: str,
        control: Callable[[dict[str, Any] | None], bool] | None = None,
        full_rebuild: bool = False,
        resume_generation: str | None = None,
    ) -> dict[str, Any]:
        if phase not in {"pre", "post", "manual"}:
            raise ProjectionError(f"unsupported projection phase: {phase}")
        state = _GenerationRun(
            actor=actor,
            run_id=run_id,
            phase=phase,
            full_rebuild=full_rebuild,
            control=control,
            generation_id=(
                f"projection_{time.time_ns()}_{uuid.uuid4().hex[:10]}"
            ),
            started=time.monotonic(),
            previous=load_json(self.latest_path),
            plan_sha256=plan_identity(self.plan()),
        )
        try:
            self._prepare_generation(
                state,
                resume_generation=resume_generation,
            )
            for step in self.steps:
                self._run_step(state, step)
            return self._publish_success(state)
        except Exception as error:
            self._record_failure(state, error)
            if isinstance(error, ProjectionError):
                raise
            raise ProjectionError(
                f"projection generation failed: {type(error).__name__}"
            ) from error
        finally:
            if state.session_path is not None:
                state.session_path.unlink(missing_ok=True)
