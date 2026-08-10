"""Immutable, digest-pinned external command profiles."""

from __future__ import annotations

import dataclasses
import json
import os
import re
import shutil
import signal
import stat
import subprocess
import tempfile
import time
from contextlib import contextmanager
from pathlib import Path
from typing import Any, Iterator, Mapping

from .model import (
    ALLOWED_PLACEHOLDERS,
    DUE_COALESCE_SECONDS,
    FORBIDDEN_EXECUTABLE_NAMES,
    HEX64_RE,
    IDENTIFIER_RE,
    IMMUTABLE_ROOT_DENYLIST,
    MAX_COMMAND_OUTPUT_BYTES,
    MIN_RETAINED_GENERATIONS,
    MIN_RETAINED_PRIOR_GENERATIONS,
    PROFILE_ENVELOPES,
    PROFILE_SCHEMA,
    PIPELINE_MAX_SECONDS,
    PROBATION_SECONDS,
    RETENTION_SECONDS,
    Build,
    IntegrityError,
    ProfileError,
    SupervisorError,
    _hex64,
    _lstat_no_link,
    _require_exact_keys,
    _resolved_absolute,
    canonical_bytes,
    atomic_write,
    ensure_private_dir,
    read_stable_regular,
    read_json,
    sha256_bytes,
    sha256_file,
    validate_bounded_path,
)
from .projection import (
    operator_status,
    refresh_introspection_evidence,
    steward_status,
    write_operator_status,
)

@dataclasses.dataclass(frozen=True)
class CommandProfile:
    name: str
    executable: Path
    argv: tuple[str, ...]
    timeout_seconds: int
    privilege_envelope: str
    run_as_uid: int
    run_as_gid: int


def validate_profile_identity(
    name: str, run_as_uid: int, run_as_gid: int, effective_uid: int, effective_gid: int
) -> None:
    """Enforce the immutable wrapper identity independently of host file ownership."""
    privileged = name in {
        "build",
        "install",
        "activate",
        "rollback",
        "health",
        "retention",
        "synthetic",
    }
    if effective_uid == 0:
        if privileged and (run_as_uid != 0 or run_as_gid != 0):
            raise ProfileError(f"profile {name} must use the root stager identity")
        if not privileged and (run_as_uid == 0 or run_as_gid == 0):
            raise ProfileError(f"profile {name} must drop the root supervisor identity")
    elif run_as_uid != effective_uid or run_as_gid != effective_gid:
        raise ProfileError(f"profile {name} run-as identity is unavailable in development")


def parse_deferred_build_result(stdout: bytes, stderr: bytes) -> dict[str, str] | None:
    """Accept only the rescue helper's canonical pre-command deferral receipt."""
    if stderr or len(stdout) > MAX_COMMAND_OUTPUT_BYTES:
        return None
    try:
        value = json.loads(stdout)
    except (UnicodeError, json.JSONDecodeError):
        return None
    if (
        not isinstance(value, dict)
        or set(value) != {"schema", "status", "reason", "retry_authority"}
        or value.get("schema") != "astrid.edge_rescue_helper.result.v1"
        or value.get("status") != "deferred_infrastructure"
        or value.get("retry_authority")
        != "immutable_supervisor_may_retry_after_condition_clears"
        or not isinstance(value.get("reason"), str)
        or not 0 < len(value["reason"]) <= 240
        or any(ord(character) < 0x20 or ord(character) == 0x7F for character in value["reason"])
        or stdout != canonical_bytes(value) + b"\n"
    ):
        return None
    return {"status": str(value["status"]), "reason": str(value["reason"])}


def parse_candidate_rejected_build_result(
    stdout: bytes, stderr: bytes
) -> dict[str, str] | None:
    """Accept only the native helper's canonical terminal candidate rejection."""

    if stderr or len(stdout) > MAX_COMMAND_OUTPUT_BYTES:
        return None
    try:
        value = json.loads(stdout)
    except (UnicodeError, json.JSONDecodeError):
        return None
    reason = value.get("reason") if isinstance(value, dict) else None
    if (
        not isinstance(value, dict)
        or set(value) != {"schema", "status", "reason", "retry_authority"}
        or value.get("schema") != "astrid.edge_rescue_helper.result.v1"
        or value.get("status") != "candidate_rejected"
        or value.get("retry_authority")
        != "identical_candidate_hash_never_retried_automatically"
        or not isinstance(reason, str)
        or not 0 < len(reason) <= 240
        or any(ord(character) < 0x20 or ord(character) == 0x7F for character in reason)
        or stdout != canonical_bytes(value) + b"\n"
    ):
        return None
    return {
        "status": "candidate_rejected",
        "reason_sha256": sha256_bytes(reason.encode("utf-8")),
    }


def parse_native_retention_result(
    stdout: bytes, stderr: bytes
) -> dict[str, Any] | None:
    """Accept only the immutable helper's bounded paired-GC summary."""

    if stderr or not stdout or len(stdout) > MAX_COMMAND_OUTPUT_BYTES:
        return None
    try:
        value = json.loads(stdout)
    except (UnicodeError, json.JSONDecodeError):
        return None
    required = {
        "schema",
        "status",
        "active_generation",
        "retained_generations",
        "retired_generations",
        "retained_prior_minimum",
        "minimum_retention_seconds",
        "ledger_head_sha256",
        "authority",
    }
    retained = value.get("retained_generations") if isinstance(value, dict) else None
    retired = value.get("retired_generations") if isinstance(value, dict) else None
    head = value.get("ledger_head_sha256") if isinstance(value, dict) else None
    if (
        not isinstance(value, dict)
        or set(value) != required
        or value.get("schema") != "astrid.edge_rescue_helper.paired_retention.v1"
        or value.get("status")
        not in {"healthy_nothing_eligible", "retired_complete_signed_pairs"}
        or not isinstance(value.get("active_generation"), str)
        or not IDENTIFIER_RE.fullmatch(value["active_generation"])
        or not isinstance(retained, list)
        or not isinstance(retired, list)
        or len(retained) > 1_024
        or len(retired) > 1_024
        or any(not isinstance(item, str) or not IDENTIFIER_RE.fullmatch(item) for item in [*retained, *retired])
        or set(retained) & set(retired)
        or value.get("active_generation") not in retained
        or value.get("retained_prior_minimum") != MIN_RETAINED_PRIOR_GENERATIONS
        or value.get("minimum_retention_seconds") != RETENTION_SECONDS
        or (head is not None and (not isinstance(head, str) or not HEX64_RE.fullmatch(head)))
        or value.get("authority") != "immutable_root_paired_generation_snapshot_gc"
        or stdout != canonical_bytes(value) + b"\n"
    ):
        return None
    return {
        "status": str(value["status"]),
        "active_generation": str(value["active_generation"]),
        "retained_generations": list(retained),
        "retired_generations": list(retired),
        "ledger_head_sha256": head,
    }


def parse_native_health_result(stdout: bytes, stderr: bytes) -> dict[str, Any] | None:
    """Parse the canonical immutable health report without retaining its body."""

    if stderr or not stdout or len(stdout) > MAX_COMMAND_OUTPUT_BYTES:
        return None
    try:
        value = json.loads(stdout)
    except (UnicodeError, json.JSONDecodeError):
        return None
    if not isinstance(value, dict) or stdout != canonical_bytes(value) + b"\n":
        return None
    required = {
        "schema",
        "active_generation_id",
        "healthy",
        "available_ram_bytes",
        "swap_bytes",
        "fill_samples",
        "fill_coverage_seconds",
        "fill_max_gap_seconds",
        "fill_mean",
        "fill_occupancy_65_735",
        "probation_fill_coverage_complete",
        "probation",
        "evidence_sha256",
    }
    if not required.issubset(value) or value.get("schema") != "astrid.edge_rescue_helper.health.v2":
        return None
    generation_id = value.get("active_generation_id")
    probation = value.get("probation")
    if (
        not isinstance(generation_id, str)
        or not IDENTIFIER_RE.fullmatch(generation_id)
        or value.get("healthy") is not True
        or not isinstance(probation, dict)
        or probation.get("schema")
        != "astrid.edge_rescue_helper.probation_evaluation.v1"
        or probation.get("generation_id") != generation_id
        or probation.get("status") not in {"active", "complete", "failed"}
        or probation.get("failed") is not (probation.get("status") == "failed")
        or not isinstance(probation.get("coverage_complete"), bool)
        or not isinstance(probation.get("coverage_due_but_incomplete"), bool)
        or not isinstance(probation.get("samples"), int)
        or isinstance(probation.get("samples"), bool)
        or probation["samples"] < 0
        or not isinstance(probation.get("elapsed_seconds"), int)
        or isinstance(probation.get("elapsed_seconds"), bool)
        or probation["elapsed_seconds"] < 0
        or not isinstance(probation.get("maximum_sample_gap_seconds"), int)
        or isinstance(probation.get("maximum_sample_gap_seconds"), bool)
        or probation["maximum_sample_gap_seconds"] < 0
        or not HEX64_RE.fullmatch(str(probation.get("ledger_head_sha256", "")))
        or not HEX64_RE.fullmatch(str(value.get("evidence_sha256", "")))
    ):
        return None
    status = str(probation["status"])
    complete = (
        status == "complete"
        and probation["coverage_complete"] is True
        and probation["coverage_due_but_incomplete"] is False
        and probation["elapsed_seconds"] >= 3_600
        and probation["samples"] >= 7
        and probation["maximum_sample_gap_seconds"] <= 600
        and value.get("probation_fill_coverage_complete") is True
        and isinstance(value.get("fill_samples"), int)
        and not isinstance(value.get("fill_samples"), bool)
        and value["fill_samples"] >= 648
        and isinstance(value.get("fill_coverage_seconds"), int)
        and not isinstance(value.get("fill_coverage_seconds"), bool)
        and value["fill_coverage_seconds"] >= 57 * 60
        and isinstance(value.get("fill_max_gap_seconds"), (int, float))
        and not isinstance(value.get("fill_max_gap_seconds"), bool)
        and float(value["fill_max_gap_seconds"]) <= 20.0
        and isinstance(value.get("fill_mean"), (int, float))
        and not isinstance(value.get("fill_mean"), bool)
        and 0.67 <= float(value["fill_mean"]) <= 0.70
        and isinstance(value.get("fill_occupancy_65_735"), (int, float))
        and not isinstance(value.get("fill_occupancy_65_735"), bool)
        and float(value["fill_occupancy_65_735"]) >= 0.90
    )
    if status == "complete" and not complete:
        return None
    return {
        "schema": str(value["schema"]),
        "active_generation_id": generation_id,
        "status": status,
        "coverage_complete": bool(probation["coverage_complete"]),
        "coverage_due_but_incomplete": bool(probation["coverage_due_but_incomplete"]),
        "samples": int(probation["samples"]),
        "elapsed_seconds": int(probation["elapsed_seconds"]),
        "maximum_sample_gap_seconds": int(probation["maximum_sample_gap_seconds"]),
        "ledger_head_sha256": str(probation["ledger_head_sha256"]),
        "evidence_sha256": str(value["evidence_sha256"]),
    }


def parse_synthetic_lifecycle_result(
    stdout: bytes, stderr: bytes
) -> dict[str, Any] | None:
    """Accept only bounded non-production evidence from the native harness."""

    if stderr or not stdout or len(stdout) > MAX_COMMAND_OUTPUT_BYTES:
        return None
    try:
        value = json.loads(stdout)
    except (UnicodeError, json.JSONDecodeError):
        return None
    required = {
        "schema",
        "provenance",
        "appliance_id",
        "production_generation_before",
        "production_binding_sha256_before",
        "production_binding_sha256_after",
        "production_active_link_before",
        "production_active_link_after",
        "synthetic_candidate_id",
        "synthetic_build_id",
        "synthetic_generation_id",
        "model_service_receipts",
        "candidate_source_changed",
        "offline_build_and_package_gates_passed",
        "isolated_activation_passed",
        "isolated_rollback_passed",
        "link_first_crash_recovered",
        "binding_first_crash_recovered",
        "production_intent_created",
        "production_generation_switched",
        "continuity_or_reservoir_admission",
        "sandbox_root",
        "evidence_sha256",
    }
    if (
        not isinstance(value, dict)
        or set(value) != required
        or stdout != canonical_bytes(value) + b"\n"
        or value.get("schema")
        != "astrid.edge_rescue_helper.synthetic_lifecycle.v1"
        or value.get("provenance")
        != "operator_isolated_synthetic_machine_evidence_not_astrid_authorship"
        or value.get("candidate_source_changed") is not False
        or value.get("offline_build_and_package_gates_passed") is not True
        or value.get("isolated_activation_passed") is not True
        or value.get("isolated_rollback_passed") is not True
        or value.get("link_first_crash_recovered") is not True
        or value.get("binding_first_crash_recovered") is not True
        or value.get("production_intent_created") is not False
        or value.get("production_generation_switched") is not False
        or value.get("continuity_or_reservoir_admission") is not False
        or value.get("production_binding_sha256_before")
        != value.get("production_binding_sha256_after")
        or value.get("production_active_link_before")
        != value.get("production_active_link_after")
    ):
        return None
    for key in (
        "appliance_id",
        "production_generation_before",
        "synthetic_candidate_id",
        "synthetic_build_id",
        "synthetic_generation_id",
    ):
        if not isinstance(value.get(key), str) or not IDENTIFIER_RE.fullmatch(value[key]):
            return None
    for key in (
        "production_binding_sha256_before",
        "production_binding_sha256_after",
        "evidence_sha256",
    ):
        if not isinstance(value.get(key), str) or not HEX64_RE.fullmatch(value[key]):
            return None
    receipts = value.get("model_service_receipts")
    expected_labels = [
        "build-model-stop",
        "build-model-start",
        "build-model-warmup",
    ]
    if not isinstance(receipts, list) or len(receipts) != 3:
        return None
    for receipt, label in zip(receipts, expected_labels, strict=True):
        if (
            not isinstance(receipt, dict)
            or set(receipt)
            != {
                "label",
                "executable_sha256",
                "argv_sha256",
                "exit_code",
                "timed_out",
                "duration_ms",
            }
            or receipt.get("label") != label
            or receipt.get("exit_code") != 0
            or receipt.get("timed_out") is not False
            or not HEX64_RE.fullmatch(str(receipt.get("executable_sha256", "")))
            or not HEX64_RE.fullmatch(str(receipt.get("argv_sha256", "")))
            or isinstance(receipt.get("duration_ms"), bool)
            or not isinstance(receipt.get("duration_ms"), int)
            or receipt["duration_ms"] < 0
        ):
            return None
    sandbox = Path(str(value.get("sandbox_root", "")))
    if (
        not sandbox.is_absolute()
        or sandbox.name == ""
        or not IDENTIFIER_RE.fullmatch(sandbox.name)
        or not sandbox.name.startswith("synthetic-")
    ):
        return None
    unhashed = dict(value)
    claimed_hash = str(unhashed.pop("evidence_sha256"))
    # Rust binds the canonical record while the digest field is empty.
    unhashed["evidence_sha256"] = ""
    if sha256_bytes(canonical_bytes(unhashed)) != claimed_hash:
        return None
    return {
        "schema": str(value["schema"]),
        "appliance_id": str(value["appliance_id"]),
        "production_generation_before": str(value["production_generation_before"]),
        "synthetic_candidate_id": str(value["synthetic_candidate_id"]),
        "synthetic_build_id": str(value["synthetic_build_id"]),
        "synthetic_generation_id": str(value["synthetic_generation_id"]),
        "sandbox_basename": sandbox.name,
        "evidence_sha256": claimed_hash,
        "production_unchanged": True,
        "model_unloaded_and_restored": True,
    }


class ProfileStore:
    def __init__(self, path: Path):
        self.path = path

    def load(self) -> dict[str, CommandProfile]:
        if not self.path.exists():
            return {}
        value = read_json(self.path, immutable=True)
        _require_exact_keys(
            value,
            required={"schema", "trusted_executable_roots", "profiles"},
            optional=set(),
            label="command profiles",
        )
        if value["schema"] != PROFILE_SCHEMA:
            raise ProfileError("unsupported command-profile schema")
        raw_roots = value["trusted_executable_roots"]
        if not isinstance(raw_roots, list) or not 1 <= len(raw_roots) <= 8:
            raise ProfileError("trusted_executable_roots must contain 1..8 roots")
        roots: list[Path] = []
        for raw in raw_roots:
            root = _resolved_absolute(Path(str(raw)), "trusted executable root")
            info = _lstat_no_link(root, "trusted executable root")
            if not stat.S_ISDIR(info.st_mode) or info.st_uid not in {0, os.geteuid()} or info.st_mode & 0o022:
                raise ProfileError(f"trusted executable root is mutable or untrusted: {root}")
            roots.append(root)
        raw_profiles = value["profiles"]
        if not isinstance(raw_profiles, dict) or set(raw_profiles) - set(PROFILE_ENVELOPES):
            raise ProfileError("command profiles contain an unsupported profile")
        profiles: dict[str, CommandProfile] = {}
        for name, raw in raw_profiles.items():
            if not isinstance(raw, dict):
                raise ProfileError(f"profile {name} must be an object")
            _require_exact_keys(
                raw,
                required={
                    "executable",
                    "executable_sha256",
                    "argv",
                    "timeout_seconds",
                    "privilege_envelope",
                    "network",
                    "shell",
                    "candidate_argv",
                    "run_as_uid",
                    "run_as_gid",
                },
                optional=set(),
                label=f"profile {name}",
            )
            if (
                raw["privilege_envelope"] != PROFILE_ENVELOPES[name]
                or raw["network"] != "deny"
                or raw["shell"] is not False
                or raw["candidate_argv"] is not False
            ):
                raise ProfileError(f"profile {name} exceeds its privilege envelope")
            executable = _resolved_absolute(Path(str(raw["executable"])), "profile executable")
            if executable.name in FORBIDDEN_EXECUTABLE_NAMES:
                raise ProfileError(f"profile {name} uses a generic interpreter or privilege tool")
            if not any(executable == root or root in executable.parents for root in roots):
                raise ProfileError(f"profile {name} executable is outside trusted roots")
            info = _lstat_no_link(executable, "profile executable")
            if (
                not stat.S_ISREG(info.st_mode)
                or not info.st_mode & stat.S_IXUSR
                or info.st_uid not in {0, os.geteuid()}
                or info.st_mode & (0o022 | stat.S_ISUID | stat.S_ISGID)
            ):
                raise ProfileError(f"profile {name} executable is mutable or privileged")
            try:
                with executable.open("rb") as executable_handle:
                    executable_magic = executable_handle.read(4)
            except OSError as error:
                raise ProfileError(f"cannot inspect profile {name} executable: {error}") from error
            if executable_magic not in {
                b"\x7fELF",  # Linux appliance helper
                b"\xcf\xfa\xed\xfe",  # 64-bit Mach-O development fixture
                b"\xfe\xed\xfa\xcf",
                b"\xca\xfe\xba\xbe",  # universal Mach-O development fixture
                b"\xbe\xba\xfe\xca",
            }:
                raise ProfileError(
                    f"profile {name} executable must be a native immutable helper, not a script"
                )
            expected_hash = _hex64(raw["executable_sha256"], "profile executable hash")
            if sha256_file(executable) != expected_hash:
                raise ProfileError(f"profile {name} executable digest mismatch")
            argv_raw = raw["argv"]
            if not isinstance(argv_raw, list) or len(argv_raw) > 32:
                raise ProfileError(f"profile {name} argv is invalid")
            argv: list[str] = []
            for argument in argv_raw:
                if not isinstance(argument, str) or len(argument) > 1024 or "\x00" in argument:
                    raise ProfileError(f"profile {name} argv is invalid")
                placeholders = set(re.findall(r"\{([a-z_]+)\}", argument))
                if (
                    placeholders - ALLOWED_PLACEHOLDERS
                    or argument.count("{") != len(placeholders)
                    or argument.count("}") != len(placeholders)
                ):
                    raise ProfileError(f"profile {name} uses unsupported template data")
                argv.append(argument)
            timeout = raw["timeout_seconds"]
            maximum_timeout = 93_600 if name == "build" else 7_200
            if (
                isinstance(timeout, bool)
                or not isinstance(timeout, int)
                or not 1 <= timeout <= maximum_timeout
                or (name == "synthetic" and timeout != 7_200)
            ):
                raise ProfileError(f"profile {name} timeout is invalid")
            run_as_uid = raw["run_as_uid"]
            run_as_gid = raw["run_as_gid"]
            if (
                isinstance(run_as_uid, bool)
                or not isinstance(run_as_uid, int)
                or run_as_uid < 0
                or isinstance(run_as_gid, bool)
                or not isinstance(run_as_gid, int)
                or run_as_gid < 0
            ):
                raise ProfileError(f"profile {name} run-as identity is invalid")
            # The rescue helper owns root orchestration for materialization and
            # then drops every candidate-controlled child to the builder UID.
            # Running the build wrapper itself as the builder would prevent it
            # from enforcing immutable source ownership and the privilege split.
            validate_profile_identity(
                name,
                run_as_uid,
                run_as_gid,
                os.geteuid(),
                os.getegid(),
            )
            profiles[name] = CommandProfile(
                name=name,
                executable=executable,
                argv=tuple(argv),
                timeout_seconds=timeout,
                privilege_envelope=str(raw["privilege_envelope"]),
                run_as_uid=run_as_uid,
                run_as_gid=run_as_gid,
            )
        return profiles


def render_profile(profile: CommandProfile, substitutions: Mapping[str, str]) -> list[str]:
    command = [str(profile.executable)]
    for argument in profile.argv:
        placeholders = set(re.findall(r"\{([a-z_]+)\}", argument))
        missing = placeholders - set(substitutions)
        if missing:
            raise ProfileError(f"profile {profile.name} lacks substitutions: {sorted(missing)}")
        rendered = argument
        for placeholder in placeholders:
            rendered = rendered.replace("{" + placeholder + "}", substitutions[placeholder])
        if "\x00" in rendered or len(rendered) > 4096:
            raise ProfileError(f"profile {profile.name} rendered unsafe argv")
        command.append(rendered)
    return command


def run_command_profile(
    profile: CommandProfile, substitutions: Mapping[str, str], scratch: Path
) -> dict[str, Any]:
    command = render_profile(profile, substitutions)
    ensure_private_dir(scratch)
    if os.geteuid() == 0:
        os.chown(scratch, profile.run_as_uid, profile.run_as_gid)
    os.chmod(scratch, 0o700)
    environment = {
        "HOME": str(scratch),
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "PATH": "/usr/bin:/bin",
    }
    started = time.monotonic()
    identity_options: dict[str, Any] = {}
    if os.geteuid() == 0:
        identity_options = {
            "user": profile.run_as_uid,
            "group": profile.run_as_gid,
            "extra_groups": (),
        }
    try:
        process = subprocess.Popen(
            command,
            cwd=scratch,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
            close_fds=True,
            **identity_options,
        )
    except OSError as error:
        raise ProfileError(f"cannot start immutable profile {profile.name}: {error}") from error
    timed_out = False
    try:
        stdout, stderr = process.communicate(timeout=profile.timeout_seconds)
    except subprocess.TimeoutExpired:
        timed_out = True
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        stdout, stderr = process.communicate()
    elapsed_ms = round((time.monotonic() - started) * 1000)
    stdout_truncated = len(stdout) > MAX_COMMAND_OUTPUT_BYTES
    stderr_truncated = len(stderr) > MAX_COMMAND_OUTPUT_BYTES
    stdout = stdout[:MAX_COMMAND_OUTPUT_BYTES]
    stderr = stderr[:MAX_COMMAND_OUTPUT_BYTES]
    receipt = {
        "profile": profile.name,
        "privilege_envelope": profile.privilege_envelope,
        "run_as_uid": profile.run_as_uid,
        "run_as_gid": profile.run_as_gid,
        "executable_sha256": sha256_file(profile.executable),
        "argv_sha256": sha256_bytes(canonical_bytes(command)),
        "exit_code": process.returncode,
        "timed_out": timed_out,
        "elapsed_ms": elapsed_ms,
        "stdout_sha256": sha256_bytes(stdout),
        "stderr_sha256": sha256_bytes(stderr),
        "stdout_truncated": stdout_truncated,
        "stderr_truncated": stderr_truncated,
    }
    if (
        profile.name == "build"
        and process.returncode == 75
        and not timed_out
        and not stdout_truncated
        and not stderr_truncated
        and not stderr
    ):
        deferred = parse_deferred_build_result(stdout, stderr)
        if deferred is not None:
            receipt["result_status"] = deferred["status"]
            receipt["result_reason"] = deferred["reason"]
    if (
        profile.name == "build"
        and process.returncode == 65
        and not timed_out
        and not stdout_truncated
        and not stderr_truncated
        and not stderr
    ):
        rejected = parse_candidate_rejected_build_result(stdout, stderr)
        if rejected is not None:
            receipt["result_status"] = rejected["status"]
            receipt["result_reason_sha256"] = rejected["reason_sha256"]
    if (
        profile.name == "health"
        and process.returncode == 0
        and not timed_out
        and not stdout_truncated
        and not stderr_truncated
    ):
        health_result = parse_native_health_result(stdout, stderr)
        if health_result is not None:
            receipt["health_result"] = health_result
    if (
        profile.name == "retention"
        and process.returncode == 0
        and not timed_out
        and not stdout_truncated
        and not stderr_truncated
    ):
        retention_result = parse_native_retention_result(stdout, stderr)
        if retention_result is not None:
            receipt["retention_result"] = retention_result
    if (
        profile.name == "synthetic"
        and process.returncode == 0
        and not timed_out
        and not stdout_truncated
        and not stderr_truncated
    ):
        synthetic_result = parse_synthetic_lifecycle_result(stdout, stderr)
        if synthetic_result is not None:
            receipt["synthetic_result"] = synthetic_result
    return receipt


@contextmanager
def temporary_profile_scratch(profile: CommandProfile, purpose: str) -> Iterator[Path]:
    """Create a PrivateTmp-compatible work directory reachable after UID drop."""

    safe_purpose = re.sub(r"[^a-z0-9-]", "-", purpose.lower())[:32] or "profile"
    try:
        scratch = Path(
            tempfile.mkdtemp(prefix=f"astrid-edge-self-change-{safe_purpose}-")
        ).resolve()
    except OSError as error:
        raise ProfileError(f"cannot create isolated profile scratch: {error}") from error
    try:
        os.chmod(scratch, 0o700)
        if os.geteuid() == 0:
            os.chown(scratch, profile.run_as_uid, profile.run_as_gid)
        yield scratch
    finally:
        try:
            shutil.rmtree(scratch, ignore_errors=False)
        except OSError as error:
            raise ProfileError(f"cannot remove isolated profile scratch: {error}") from error


class PipelineManager:
    """Bounded signed-inbox ingestion and one-candidate build advancement."""

    INBOX_NAME = re.compile(r"candidate-intent-([A-Za-z0-9][A-Za-z0-9._-]{0,127})\.json\Z")
    READY_NAME = re.compile(r"candidate-ready-([A-Za-z0-9][A-Za-z0-9._-]{0,127})\.json\Z")
    READY_PENDING_NAME = re.compile(
        r"candidate-ready-([A-Za-z0-9][A-Za-z0-9._-]{0,127})\.pending\Z"
    )
    MAX_ENTRIES_PER_PASS = 64

    def __init__(self, supervisor: Any):
        self.supervisor = supervisor

    @property
    def inbox(self) -> Path:
        return self.supervisor.config.state_root / "inbox"

    def _event_exists(self, event_id: str) -> bool:
        return any(
            record["core"]["event_id"] == event_id
            for record in self.supervisor.ledger("operator").read()
        )

    def _append_operator_once(self, kind: str, payload: Mapping[str, Any], event_id: str) -> None:
        if not self._event_exists(event_id):
            self.supervisor.ledger("operator").append(
                kind, payload, event_id, self.supervisor.now
            )

    def _archive(self, source: Path, destination_root: Path, digest: str) -> Path:
        ensure_private_dir(destination_root)
        target = destination_root / f"{source.stem}.{digest[:16]}.json"
        if target.exists():
            if sha256_bytes(read_stable_regular(target)) != digest:
                raise IntegrityError("deterministic inbox archive collision")
            source.unlink()
            return target
        os.replace(source, target)
        os.chmod(target, 0o600)
        return target

    def _quarantine(self, path: Path, reason: str, data: bytes | None = None) -> dict[str, Any]:
        bounded_reason = reason[:240]
        digest = sha256_bytes(data if data is not None else path.name.encode("utf-8"))
        target_root = self.inbox / "quarantine"
        ensure_private_dir(target_root)
        if path.is_symlink() or data is None:
            if path.exists() or path.is_symlink():
                path.unlink()
            target = target_root / f"{path.stem}.{digest[:16]}.json"
            if not target.exists():
                atomic_write(
                    target,
                    canonical_bytes(
                        {
                            "schema": "astrid.edge_self_change.inbox_rejection.v1",
                            "name": path.name,
                            "reason": bounded_reason,
                            "input_sha256": digest,
                        }
                    )
                    + b"\n",
                )
        else:
            target = self._archive(path, target_root, digest)
        event_id = f"inbox-rejected-{digest[:24]}"
        self._append_operator_once(
            "inbox_rejected",
            {
                "name": path.name,
                "reason": bounded_reason,
                "input_sha256": digest,
                "quarantine": target.name,
            },
            event_id,
        )
        return {"status": "quarantined", "reason": bounded_reason, "input_sha256": digest}

    def _discard_handoff_trigger(self, path: Path, reason: str) -> bool:
        """Remove one bounded wakeup marker without treating it as candidate authority."""

        match = self.READY_NAME.fullmatch(path.name)
        if (
            match is None
            or (not path.exists() and not path.is_symlink())
            or (path.is_dir() and not path.is_symlink())
        ):
            return False
        data: bytes | None = None
        if not path.is_symlink():
            try:
                data = read_stable_regular(path, maximum_bytes=8_192)
            except SupervisorError:
                data = None
        digest = sha256_bytes(data if data is not None else path.name.encode("utf-8"))
        if path.exists() or path.is_symlink():
            path.unlink()
        self._append_operator_once(
            "inbox_handoff_trigger_discarded",
            {
                "name": path.name,
                "envelope_id": match.group(1),
                "input_sha256": digest,
                "reason": reason[:160],
                "authority": "trigger_only_no_candidate_or_deployment_authority",
            },
            f"inbox-trigger-discarded-{digest[:24]}",
        )
        return True

    def _discard_all_handoff_triggers(self, reason: str) -> int:
        if not self.inbox.exists():
            return 0
        discarded = 0
        for path in sorted(self.inbox.iterdir(), key=lambda item: item.name)[
            : self.MAX_ENTRIES_PER_PASS
        ]:
            discarded += int(self._discard_handoff_trigger(path, reason))
        return discarded

    def _pending_handoff_count(self) -> int:
        """Count inert root-cleanup inputs without opening or mutating them."""

        if not self.inbox.exists():
            return 0
        return sum(
            1
            for path in self.inbox.iterdir()
            if self.READY_PENDING_NAME.fullmatch(path.name)
        )

    def ingest_one(self, *, execute: bool) -> dict[str, Any]:
        state = self.supervisor.read_state()
        if execute and (
            state["mode"] != "running"
            or state.get("probation")
            or state.get("inflight")
        ):
            discarded_triggers = self._discard_all_handoff_triggers(
                "supervisor_not_running_or_pipeline_busy"
            )
            return {
                "status": "paused_queued_untouched",
                "mode": state["mode"],
                "reason": state.get("paused_reason"),
                "discarded_handoff_triggers": discarded_triggers,
                "ignored_pending_handoff_triggers": self._pending_handoff_count(),
                "dry_run": False,
            }
        if not self.inbox.exists():
            if execute:
                ensure_private_dir(self.inbox)
            return {"status": "empty", "dry_run": not execute}
        accepted = 0
        rejected = 0
        ignored_partial = 0
        discarded_triggers = 0
        ignored_pending_triggers = 0
        entries = sorted(self.inbox.iterdir(), key=lambda item: item.name)
        deferred = max(0, len(entries) - self.MAX_ENTRIES_PER_PASS)
        for path in entries[: self.MAX_ENTRIES_PER_PASS]:
            if self.READY_PENDING_NAME.fullmatch(path.name):
                # Immutable reflection cleanup owns the `.pending` -> `.json`
                # transition.  The supervisor must not read, quarantine,
                # remove, or infer candidate authority from a pending marker.
                ignored_pending_triggers += 1
                continue
            if self.READY_NAME.fullmatch(path.name):
                if execute:
                    discarded_triggers += int(
                        self._discard_handoff_trigger(path, "orphan_or_already_processed")
                    )
                continue
            if path.is_dir() and not path.is_symlink():
                if path.name not in {"processed", "quarantine"}:
                    rejected += 1
                    if execute:
                        digest = sha256_bytes(path.name.encode("utf-8"))
                        self._append_operator_once(
                            "inbox_rejected",
                            {
                                "name": path.name,
                                "reason": "unsupported inbox directory",
                                "input_sha256": digest,
                                "quarantine": None,
                            },
                            f"inbox-rejected-{digest[:24]}",
                        )
                continue
            if path.name.endswith(".partial") and not path.is_symlink():
                ignored_partial += 1
                continue
            match = self.INBOX_NAME.fullmatch(path.name)
            try:
                if path.is_symlink() or match is None:
                    raise IntegrityError("unsupported inbox name or linked input")
                data = read_stable_regular(path)
                value = json.loads(data)
                if not isinstance(value, dict):
                    raise IntegrityError("inbox envelope must be an object")
                canonical = canonical_bytes(value)
                if data not in {canonical, canonical + b"\n"}:
                    raise IntegrityError("inbox envelope must be exact canonical JSON")
                envelope_id = str(
                    value.get("intent_envelope", {})
                    .get("core", {})
                    .get("envelope_id", "")
                )
                if envelope_id != match.group(1):
                    raise IntegrityError("inbox filename does not bind envelope_id")
                result = self.supervisor.record_scheduled_intent(
                    value, execute=execute, allow_recovery=True
                )
            except (OSError, ValueError, SupervisorError) as error:
                rejected += 1
                if execute:
                    safe_data = None
                    try:
                        safe_data = read_stable_regular(path)
                    except SupervisorError:
                        pass
                    self._quarantine(path, str(error), safe_data)
                continue
            digest = sha256_bytes(data)
            if execute:
                archived = self._archive(path, self.inbox / "processed", digest)
                ready = self.inbox / f"candidate-ready-{result['envelope_id']}.json"
                discarded_triggers += int(
                    self._discard_handoff_trigger(ready, "exact_intent_ingested")
                )
                self._append_operator_once(
                    "inbox_processed",
                    {
                        "envelope_id": result["envelope_id"],
                        "intent_id": result["intent_id"],
                        "input_sha256": digest,
                        "processed": archived.name,
                    },
                    f"inbox-processed-{digest[:24]}",
                )
            accepted += 1
            return {
                "status": "accepted" if execute else "would_accept",
                "accepted": accepted,
                "rejected": rejected,
                "ignored_partial": ignored_partial,
                "discarded_handoff_triggers": discarded_triggers,
                "ignored_pending_handoff_triggers": ignored_pending_triggers,
                "deferred": deferred,
                "intent_id": result["intent_id"],
                "dry_run": not execute,
            }
        return {
            "status": "no_valid_envelope",
            "accepted": accepted,
            "rejected": rejected,
            "ignored_partial": ignored_partial,
            "discarded_handoff_triggers": discarded_triggers,
            "ignored_pending_handoff_triggers": ignored_pending_triggers,
            "deferred": deferred,
            "dry_run": not execute,
        }

    def _build_events(self, candidate_id: str) -> tuple[bool, bool]:
        """Return whether a wrapper invocation is unresolved and whether one completed."""
        latest_phase = "none"
        completed = False
        for record in self.supervisor.ledger("build").read():
            payload = record["core"]["payload"]
            if payload.get("candidate_id") != candidate_id:
                continue
            kind = record["core"]["kind"]
            if kind == "build_profile_started":
                latest_phase = "started"
            elif kind == "build_profile_deferred":
                latest_phase = "deferred"
            elif kind in {
                "build_profile_completed",
                "build_profile_failed",
                "build_profile_invalid_output",
                "build_profile_rejected",
            }:
                latest_phase = "terminal"
                completed |= kind == "build_profile_completed"
        return latest_phase == "started", completed

    def _rescue(self, reason: str, execute: bool) -> dict[str, Any]:
        if execute:
            state = self.supervisor.read_state()
            state["mode"] = "rescue"
            state["paused_reason"] = reason
            self.supervisor.write_state(state)
            event_id = f"pipeline-rescue-{sha256_bytes(reason.encode())[:24]}"
            self._append_operator_once(
                "pipeline_rescue",
                {"reason": reason},
                event_id,
            )
        return {"status": "rescue", "reason": reason, "dry_run": not execute}

    def _processed_intent_envelope(self, attestation: Mapping[str, Any]) -> Path:
        processed = self.inbox / "processed"
        validate_bounded_path(self.inbox, processed)
        envelope_id = str(attestation["envelope_id"])
        expected_sha256 = str(attestation["envelope_sha256"])
        prefix = f"candidate-intent-{envelope_id}."
        matches: list[Path] = []
        for path in sorted(processed.iterdir(), key=lambda item: item.name)[:128]:
            if (
                not path.name.startswith(prefix)
                or not path.name.endswith(".json")
                or path.is_symlink()
            ):
                continue
            validate_bounded_path(processed, path)
            data = read_stable_regular(path, maximum_bytes=128 * 1024)
            try:
                value = json.loads(data)
            except (UnicodeError, json.JSONDecodeError) as error:
                raise IntegrityError("processed intent envelope is malformed") from error
            if (
                not isinstance(value, dict)
                or sha256_bytes(canonical_bytes(value)) != expected_sha256
                or value.get("intent_envelope", {}).get("core", {}).get("envelope_id")
                != envelope_id
            ):
                continue
            matches.append(path)
        if len(matches) != 1:
            raise IntegrityError("exact processed intent envelope is unavailable or ambiguous")
        return matches[0]

    def run_model_pass(self, due: Mapping[str, Any], *, execute: bool) -> dict[str, Any]:
        """Run the immutable proposal helper and relay only its signed output."""

        profile = self.supervisor.profiles.load().get("model")
        if profile is None:
            raise ProfileError("immutable command profile is not configured: model")
        dry_output = Path("/tmp/astrid-edge-self-change-model/intent-envelope.json")
        substitutions = self.supervisor._substitutions()
        substitutions["intent_envelope"] = str(dry_output)
        command = render_profile(profile, substitutions)
        if not execute:
            return {
                "status": "would_run",
                "dry_run": True,
                "due": dict(due),
                "command": {
                    "profile": "model",
                    "dry_run": True,
                    "executable_sha256": sha256_file(profile.executable),
                    "argv_sha256": sha256_bytes(canonical_bytes(command)),
                    "privilege_envelope": profile.privilege_envelope,
                    "run_as_uid": profile.run_as_uid,
                    "run_as_gid": profile.run_as_gid,
                },
            }
        with temporary_profile_scratch(profile, "model") as scratch:
            output_path = scratch / "intent-envelope.json"
            substitutions["intent_envelope"] = str(output_path)
            receipt = run_command_profile(profile, substitutions, scratch)
            result: dict[str, Any] = {"due": dict(due), "command": receipt}
            if receipt["timed_out"] or receipt["exit_code"] != 0:
                return {"status": "profile_failed", **result}
            try:
                raw = read_stable_regular(
                    output_path, owners={0, os.geteuid(), profile.run_as_uid}
                )
                envelope = json.loads(raw)
                if not isinstance(envelope, dict):
                    raise IntegrityError("model output envelope must be an object")
                verified = self.supervisor.intent_attestor.verify_envelope(
                    envelope, self.supervisor.now
                )
                # Full typed/binding validation happens before anything reaches the inbox.
                checked = self.supervisor.record_scheduled_intent(
                    envelope, execute=False, allow_recovery=True
                )
                envelope_id = str(verified["envelope_id"])
                ensure_private_dir(self.inbox)
                destination = self.inbox / f"candidate-intent-{envelope_id}.json"
                encoded = canonical_bytes(envelope) + b"\n"
                if destination.exists() or destination.is_symlink():
                    if destination.is_symlink() or read_stable_regular(destination) != encoded:
                        raise IntegrityError("model output collides with an inbox envelope")
                else:
                    atomic_write(destination, encoded, 0o600)
                state = self.supervisor.read_state()
                if state["mode"] == "paused":
                    return {
                        "status": "attested_result_queued",
                        "intent_id": checked["intent_id"],
                        "envelope_id": envelope_id,
                        "queued_name": destination.name,
                        **result,
                    }
                ingestion = self.ingest_one(execute=True)
                if (
                    ingestion.get("status") != "accepted"
                    or ingestion.get("intent_id") != checked["intent_id"]
                ):
                    return {
                        "status": "no_matching_attested_result",
                        "expected_intent_id": checked["intent_id"],
                        "ingestion": ingestion,
                        **result,
                    }
                return {
                    "status": "attested_result_ingested",
                    "intent_id": checked["intent_id"],
                    "envelope_id": envelope_id,
                    "ingestion": ingestion,
                    **result,
                }
            except (OSError, ValueError, SupervisorError) as error:
                return {
                    "status": "invalid_or_missing_attested_result",
                    "error": str(error)[:240],
                    **result,
                }

    def steward(self, *, execute: bool) -> dict[str, Any]:
        state = self.supervisor.read_state()
        if state["mode"] == "rescue" or state.get("probation") or state.get("inflight"):
            return self.supervisor._finish_pass(
                {
                    "operation": "steward",
                    "status": "blocked_by_state",
                    "mode": state["mode"],
                    "dry_run": not execute,
                },
                execute=execute,
            )
        due = state.get("due")
        last = state.get("last_steward_started_at")
        if due is None and (
            last is None or self.supervisor.now >= int(last) + DUE_COALESCE_SECONDS
        ):
            due = {
                "first_requested_at": self.supervisor.now,
                "not_before": self.supervisor.now,
                "reasons": ["periodic"],
                "coalesced_count": 1,
            }
            state["due"] = due
            if execute:
                self.supervisor.write_state(state)
        if due is None or self.supervisor.now < int(due["not_before"]):
            return self.supervisor._finish_pass(
                {
                    "operation": "steward",
                    "status": "not_due",
                    "due": due,
                    "dry_run": not execute,
                },
                execute=execute,
            )
        try:
            model_pass = self.run_model_pass(due, execute=execute)
        except ProfileError as error:
            result = {
                "operation": "steward",
                "status": "model_profile_unavailable_due_retained",
                "error": str(error)[:240],
                "due": due,
                "dry_run": not execute,
            }
            if execute:
                state = self.supervisor.read_state()
                state["last_steward_started_at"] = self.supervisor.now
                due["not_before"] = max(
                    int(due["not_before"]),
                    self.supervisor.now + DUE_COALESCE_SECONDS,
                )
                state["due"] = due
                self.supervisor.write_state(state)
                self.supervisor.ledger("operator").append(
                    "steward_profile_unavailable",
                    {"due": due, "error": str(error)[:240]},
                    f"steward-unavailable-{self.supervisor.now}",
                    self.supervisor.now,
                )
            return self.supervisor._finish_pass(result, execute=execute)
        if not execute:
            return {"operation": "steward", **model_pass}
        state = self.supervisor.read_state()
        state["last_steward_started_at"] = self.supervisor.now
        if model_pass.get("status") not in {
            "attested_result_ingested",
            "attested_result_queued",
        }:
            retained_due = dict(due)
            retained_due["not_before"] = max(
                int(retained_due["not_before"]),
                self.supervisor.now + DUE_COALESCE_SECONDS,
            )
            state["due"] = retained_due
            self.supervisor.write_state(state)
            self.supervisor.ledger("operator").append(
                "steward_no_attested_result",
                {"due": retained_due, "model_pass": model_pass},
                f"steward-failed-{self.supervisor.now}",
                self.supervisor.now,
            )
            return self.supervisor._finish_pass(
                {
                    "operation": "steward",
                    "status": "no_attested_result_due_retained",
                    "due": retained_due,
                    "model_pass": model_pass,
                },
                execute=True,
            )
        state["due"] = None
        self.supervisor.write_state(state)
        self.supervisor.ledger("operator").append(
            "steward_profile_completed",
            {"due": due, "model_pass": model_pass},
            f"steward-{self.supervisor.now}",
            self.supervisor.now,
        )
        return self.supervisor._finish_pass(
            {
                "operation": "steward",
                "status": model_pass["status"],
                "due": due,
                "model_pass": model_pass,
                "dry_run": False,
            },
            execute=True,
        )

    def advance_one_build(self, *, execute: bool) -> dict[str, Any]:
        state = self.supervisor.read_state()
        if state["mode"] != "running" or state.get("probation") or state.get("inflight"):
            return {"status": "blocked_by_state", "dry_run": not execute}
        consumed, _ = self.supervisor.consumed_intents()
        candidates: list[
            tuple[str, Mapping[str, Any], Mapping[str, Any]]
        ] = []
        for intent_id, attestation in self.supervisor.scheduled_intents().items():
            intent = attestation["intent"]
            if (
                intent_id not in consumed
                and intent["base_generation"] == state.get("active_generation")
                and self.supervisor.now - int(attestation["attested_at"])
                <= PIPELINE_MAX_SECONDS
            ):
                candidates.append((intent_id, intent, attestation))
        if not candidates:
            return {"status": "no_pending_intent", "dry_run": not execute}
        if len(candidates) != 1:
            return self._rescue("multiple_pending_build_intents", execute)
        intent_id, intent, attestation = candidates[0]
        matching_builds = [
            (build_id, payload)
            for build_id, payload in self.supervisor.builds().items()
            if payload["candidate_id"] == intent["candidate_id"]
        ]
        if matching_builds:
            if len(matching_builds) != 1:
                return self._rescue("multiple_builds_for_attested_candidate", execute)
            build_id, _ = matching_builds[0]
            if build_id in self.supervisor.staged_builds():
                return {"status": "staged", "build_id": build_id, "intent_id": intent_id}
            result = self.supervisor.stage(build_id, execute=execute)
            return {"status": "staged" if execute else "would_stage", **result}
        started, completed = self._build_events(str(intent["candidate_id"]))
        if started and not completed:
            return self._rescue("build_profile_interrupted_no_retry", execute)
        profile = self.supervisor.profiles.load().get("build")
        if profile is None:
            return {"status": "blocked_missing_immutable_build_profile", "dry_run": not execute}
        dry_scratch = Path(f"/tmp/astrid-edge-self-change-build-{intent_id}")
        candidate_path = dry_scratch / "candidate.json"
        intent_envelope = dry_scratch / "processed-intent-envelope.json"
        model_handoff = self.supervisor.config.model_handoff_root / (
            str(attestation["envelope_id"]) + ".json"
        )
        validate_bounded_path(
            self.supervisor.config.model_handoff_root,
            model_handoff,
            require_exists=False,
        )
        output_path = dry_scratch / "build-manifest.json"
        substitutions = self.supervisor._substitutions()
        substitutions.update(
            {
                "candidate_id": str(intent["candidate_id"]),
                "candidate_manifest": str(candidate_path),
                "intent_envelope": str(intent_envelope),
                "model_handoff": str(model_handoff),
                "build_manifest": str(output_path),
            }
        )
        if not execute:
            command = render_profile(profile, substitutions)
            return {
                "status": "would_build",
                "intent_id": intent_id,
                "candidate_id": intent["candidate_id"],
                "argv_sha256": sha256_bytes(canonical_bytes(command)),
                "dry_run": True,
            }
        with temporary_profile_scratch(profile, "build") as scratch:
            if not model_handoff.exists():
                return {
                    "status": "deferred_model_unload_handoff",
                    "intent_id": intent_id,
                    "candidate_id": intent["candidate_id"],
                    "dry_run": False,
                }
            validate_bounded_path(self.supervisor.config.model_handoff_root, model_handoff)
            try:
                intent_envelope = self._processed_intent_envelope(attestation)
            except (OSError, ValueError, SupervisorError) as error:
                return self._rescue(
                    f"processed_intent_envelope_unavailable:{str(error)[:160]}", True
                )
            self.supervisor.ledger("build").append(
                "build_profile_started",
                {
                    "candidate_id": intent["candidate_id"],
                    "candidate_sha256": intent["candidate_sha256"],
                    "intent_id": intent_id,
                },
                f"build-started-{intent_id}",
                self.supervisor.now,
            )
            candidate_path = scratch / "candidate.json"
            output_path = scratch / "build-manifest.json"
            substitutions["candidate_manifest"] = str(candidate_path)
            substitutions["intent_envelope"] = str(intent_envelope)
            substitutions["build_manifest"] = str(output_path)
            candidate = self.supervisor.candidates()[str(intent["candidate_id"])]
            atomic_write(candidate_path, canonical_bytes(candidate) + b"\n", 0o400)
            if os.geteuid() == 0:
                os.chown(candidate_path, profile.run_as_uid, profile.run_as_gid)
            try:
                receipt = run_command_profile(profile, substitutions, scratch)
            except ProfileError as error:
                self.supervisor.ledger("build").append(
                    "build_profile_failed",
                    {
                        "candidate_id": intent["candidate_id"],
                        "intent_id": intent_id,
                        "reason": str(error)[:240],
                    },
                    f"build-failed-{intent_id}",
                    self.supervisor.now,
                )
                return self._rescue("build_profile_failed_no_retry", True)
            if receipt["timed_out"] or receipt["exit_code"] != 0:
                if receipt.get("result_status") == "candidate_rejected":
                    reason_sha256 = receipt.get("result_reason_sha256")
                    if not isinstance(reason_sha256, str) or not HEX64_RE.fullmatch(
                        reason_sha256
                    ):
                        return self._rescue("invalid_candidate_rejection_receipt", True)
                    self.supervisor.ledger("build").append(
                        "build_profile_rejected",
                        {
                            "candidate_id": intent["candidate_id"],
                            "candidate_sha256": intent["candidate_sha256"],
                            "intent_id": intent_id,
                            "reason_sha256": reason_sha256,
                            "automatic_retry": False,
                            "command_receipt": receipt,
                        },
                        f"build-rejected-{intent_id}",
                        self.supervisor.now,
                    )
                    self.supervisor.ledger("activation").append(
                        "scheduled_intent_terminal_rejected",
                        {
                            "intent_id": intent_id,
                            "appliance_id": intent["appliance_id"],
                            "trace_id": intent["trace_id"],
                            "session_id": intent["session_id"],
                            "turn_id": intent["turn_id"],
                            "response_sha256": intent["response_sha256"],
                            "terminal_declaration_sha256": intent[
                                "terminal_declaration_sha256"
                            ],
                            "candidate_id": intent["candidate_id"],
                            "candidate_sha256": intent["candidate_sha256"],
                            "base_generation": intent["base_generation"],
                            "envelope_id": attestation["envelope_id"],
                            "envelope_sha256": attestation["envelope_sha256"],
                            "reason_sha256": reason_sha256,
                            "automatic_retry": False,
                            "authority": "terminal_exact_candidate_rejection_no_promotion",
                        },
                        f"intent-terminal-rejected-{intent_id}",
                        self.supervisor.now,
                    )
                    return {
                        "status": "candidate_rejected_terminal_no_retry",
                        "intent_id": intent_id,
                        "candidate_id": intent["candidate_id"],
                        "reason_sha256": reason_sha256,
                        "dry_run": False,
                    }
                if receipt.get("result_status") == "deferred_infrastructure":
                    self.supervisor.ledger("build").append(
                        "build_profile_deferred",
                        {
                            "candidate_id": intent["candidate_id"],
                            "intent_id": intent_id,
                            "reason": receipt.get("result_reason"),
                            "command_receipt": receipt,
                        },
                        f"build-deferred-{intent_id}-{self.supervisor.now}",
                        self.supervisor.now,
                    )
                    return {
                        "status": "deferred_infrastructure",
                        "intent_id": intent_id,
                        "candidate_id": intent["candidate_id"],
                        "dry_run": False,
                    }
                self.supervisor.ledger("build").append(
                    "build_profile_failed",
                    {
                        "candidate_id": intent["candidate_id"],
                        "intent_id": intent_id,
                        "command_receipt": receipt,
                    },
                    f"build-failed-{intent_id}",
                    self.supervisor.now,
                )
                return self._rescue("build_profile_failed_no_retry", True)
            try:
                raw = read_stable_regular(
                    output_path, owners={0, os.geteuid(), profile.run_as_uid}
                )
                manifest = json.loads(raw)
                if not isinstance(manifest, dict):
                    raise IntegrityError("build manifest must be an object")
                build_result = self.supervisor.record_build(manifest, execute=True)
            except (OSError, ValueError, SupervisorError) as error:
                self.supervisor.ledger("build").append(
                    "build_profile_invalid_output",
                    {
                        "candidate_id": intent["candidate_id"],
                        "intent_id": intent_id,
                        "reason": str(error)[:240],
                        "command_receipt": receipt,
                    },
                    f"build-invalid-{intent_id}",
                    self.supervisor.now,
                )
                return self._rescue("invalid_build_manifest_no_retry", True)
        build = Build.parse(
            build_result["build"],
            self.supervisor.config.target,
            self.supervisor.config.appliance_id,
        )
        self.supervisor.ledger("build").append(
            "build_profile_completed",
            {
                "candidate_id": build.candidate_id,
                "candidate_sha256": build.candidate_sha256,
                "intent_id": intent_id,
                "build_id": build.build_id,
                "manifest_sha256": sha256_bytes(raw),
                "command_receipt": receipt,
            },
            f"build-completed-{intent_id}",
            self.supervisor.now,
        )
        staged = self.supervisor.stage(build.build_id, execute=True)
        return {"status": "staged", "intent_id": intent_id, **staged}

    def inbox_summary(self) -> dict[str, int]:
        summary = {
            "pending": 0,
            "handoff_triggers": 0,
            "pending_handoff_triggers": 0,
            "partial": 0,
            "processed": 0,
            "quarantined": 0,
            "unexpected_directories": 0,
        }
        if not self.inbox.exists():
            return summary
        for path in self.inbox.iterdir():
            if path.name == "processed" and path.is_dir():
                summary["processed"] = sum(1 for _ in path.iterdir())
            elif path.name == "quarantine" and path.is_dir():
                summary["quarantined"] = sum(1 for _ in path.iterdir())
            elif path.name.endswith(".partial"):
                summary["partial"] += 1
            elif self.READY_PENDING_NAME.fullmatch(path.name):
                summary["pending_handoff_triggers"] += 1
            elif self.READY_NAME.fullmatch(path.name):
                summary["handoff_triggers"] += 1
            elif path.is_dir() and not path.is_symlink():
                summary["unexpected_directories"] += 1
            elif path.is_file() or path.is_symlink():
                summary["pending"] += 1
        return summary

    def retention(self) -> dict[str, Any]:
        if not self.supervisor.config.releases_root.exists():
            return {"retained": [], "eligible": []}
        _lstat_no_link(self.supervisor.config.releases_root, "releases root")
        generations: list[tuple[int, str]] = []
        for item in self.supervisor.config.releases_root.iterdir():
            info = item.lstat()
            if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
                continue
            if IDENTIFIER_RE.fullmatch(item.name):
                generations.append((int(info.st_mtime), item.name))
        generations.sort(reverse=True)
        state = self.supervisor.read_state()
        active_generation = self.supervisor.read_active_generation(required=False)
        protected = {
            str(value)
            for value in (
                active_generation,
                state.get("active_generation"),
                state.get("previous_generation"),
            )
            if value
        }
        newest_prior = [
            name
            for _, name in generations
            if name != active_generation
        ][:MIN_RETAINED_PRIOR_GENERATIONS]
        protected.update(newest_prior)
        eligible = [
            name
            for modified, name in generations
            if name not in protected
            and self.supervisor.now - modified >= RETENTION_SECONDS
        ]
        return {
            "retained": [name for _, name in generations if name not in eligible],
            "eligible": eligible,
            "minimum_generations": MIN_RETAINED_GENERATIONS,
            "minimum_prior_generations": MIN_RETAINED_PRIOR_GENERATIONS,
            "active_generation_counts_toward_prior_minimum": False,
            "minimum_age_seconds": RETENTION_SECONDS,
            "pruning_authority": "immutable_signed_paired_generation_snapshot_gc",
            "snapshot_retention": "paired_with_generation_under_signed_crash_safe_transaction",
        }

    def prune(self, *, execute: bool) -> dict[str, Any]:
        retention = self.retention()
        receipt = self.supervisor.invoke_profile(
            "retention", self.supervisor._substitutions(), execute=execute
        )
        removed: list[str] = []
        if execute:
            native = receipt.get("retention_result")
            if not self.supervisor._profile_success(receipt) or not isinstance(native, dict):
                raise SupervisorError("immutable paired retention profile failed closed")
            removed = list(native["retired_generations"])
            self.supervisor.ledger("operator").append(
                "paired_retention_completed",
                {
                    "removed": removed,
                    "retained": native["retained_generations"],
                    "ledger_head_sha256": native["ledger_head_sha256"],
                    "command_receipt": receipt,
                },
                f"paired-retention-{self.supervisor.now}",
                self.supervisor.now,
            )
        return {
            "operation": "prune",
            "dry_run": not execute,
            **retention,
            "removed": removed,
            "status": (
                receipt.get("retention_result", {}).get("status", "dry_run_immutable_profile")
                if execute
                else "dry_run_immutable_profile"
            ),
            "command": receipt,
        }

    def status(self) -> dict[str, Any]:
        ledgers: dict[str, Any] = {}
        for name in ("candidate", "build", "activation", "operator"):
            records = self.supervisor.ledger(name).read()
            ledgers[name] = {
                "records": len(records),
                "head": records[-1]["record_hash"] if records else None,
                "valid": True,
            }
        try:
            loaded = self.supervisor.profiles.load()
            profiles: dict[str, Any] = {"valid": True, "configured": sorted(loaded)}
        except SupervisorError as error:
            profiles = {"valid": False, "error": str(error), "configured": []}
        return {
            "schema": "astrid.edge_self_change.status.v1",
            "projection_only_not_authority": True,
            "safe_default": "dry_run",
            "generated_at": self.supervisor.now,
            "state": self.supervisor.read_state(),
            "active_link_generation": self.supervisor.read_active_generation(required=False),
            "ledgers": ledgers,
            "command_profiles": profiles,
            "intent_attestor": {
                "configured": True,
                "key_id": self.supervisor.intent_attestor.key_id,
                "separate_from_ledger": (
                    self.supervisor.intent_attestor.key_id != self.supervisor.signer.key_id
                ),
            },
            "inbox": self.inbox_summary(),
            "policy": {
                "due_coalescing_seconds": DUE_COALESCE_SECONDS,
                "probation_seconds": PROBATION_SECONDS,
                "retention_minimum_generations": MIN_RETAINED_GENERATIONS,
                "retention_minimum_prior_generations": MIN_RETAINED_PRIOR_GENERATIONS,
                "retention_active_counts_toward_prior_minimum": False,
                "retention_minimum_seconds": RETENTION_SECONDS,
                "immutable_root_denylist": [
                    *IMMUTABLE_ROOT_DENYLIST,
                    *self.supervisor.config.extra_denylist,
                ],
            },
            "retention": self.retention(),
        }

    def project_status(self, last_pass: Mapping[str, Any]) -> dict[str, Any]:
        projection = self.status()
        projection["last_pass"] = dict(last_pass)
        projection["introspection_evidence"] = refresh_introspection_evidence(
            self.supervisor
        )
        atomic_write(
            self.supervisor.config.state_root / "status.json",
            canonical_bytes(projection) + b"\n",
            0o600,
        )
        atomic_write(
            self.supervisor.config.state_root / "steward-status.json",
            canonical_bytes(steward_status(self.supervisor)) + b"\n",
            0o600,
        )
        write_operator_status(
            self.supervisor.config.operator_status,
            operator_status(self.supervisor, last_pass),
        )
        return projection
