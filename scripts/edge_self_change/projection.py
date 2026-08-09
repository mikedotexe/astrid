"""Narrow immutable projection consumed by the scheduled steward.

The projection is not an authority record.  It is derived only after the
supervisor has verified its authenticated state and ledgers, and is exposed
read-only to the separate steward namespace so a submitted draft can be
coalesced or reconciled without granting access to the supervisor root.
"""

from __future__ import annotations

import json
import os
import re
import stat
import tempfile
from pathlib import Path
from pathlib import PurePosixPath
from typing import Any

from .model import IntegrityError, canonical_bytes, sha256_bytes

SCHEMA = "astrid.edge_self_change.steward_status.v1"
OPERATOR_SCHEMA = "astrid.edge_self_change.operator_status.v3"
OPERATOR_ENVELOPE_SCHEMA = "astrid.edge_self_change.operator_status_envelope.v1"
NONTERMINAL = frozenset({"intent_pending", "building", "staged", "probation"})
BUILD_EVIDENCE_SCHEMA = "astrid.edge_self_change.build_evidence_view.v1"
GENERATION_DIFF_SCHEMA = "astrid.edge_self_change.generation_diff_view.v1"
EVIDENCE_PROVENANCE = "immutable_machine_evidence_not_astrid_authorship"
MAX_EVIDENCE_BYTES = 256 * 1024
MAX_LIFECYCLE_EVENTS = 32
MAX_OPERATOR_LIFECYCLE_EVENTS = 64
MAX_OPERATOR_PROJECTION_BYTES = 256 * 1024
OPERATOR_EVENT_SCHEMA = "astrid.edge_self_change.operator_lifecycle_event.v1"
OPERATOR_EVENT_PROVENANCE = "immutable_supervisor_signed_ledger_sanitized_metadata"
OPERATOR_EVENT_AUTHORITY = "observation_only_not_deployment_or_astrid_authorship"
OPERATOR_FACETS = frozenset(
    {
        "reflection",
        "candidate",
        "build",
        "test",
        "invariant",
        "shadow",
        "activation",
        "restart",
        "probation",
        "rollback",
        "operator",
    }
)
HEX64_RE = re.compile(r"[0-9a-f]{64}\Z")
IDENTIFIER_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}\Z")


def _hex64(value: Any) -> bool:
    return isinstance(value, str) and HEX64_RE.fullmatch(value) is not None


def _source_id(value: Any) -> bool:
    return (
        isinstance(value, str)
        and value.startswith("cpu-edge:")
        and _hex64(value.removeprefix("cpu-edge:"))
    )


def _safe_source_path(value: Any) -> bool:
    if not isinstance(value, str) or not value or len(value) > 512 or "\\" in value:
        return False
    path = PurePosixPath(value)
    return (
        not path.is_absolute()
        and all(
            part not in {"", ".", ".."} and not part.startswith(".")
            for part in path.parts
        )
    )


def _exact_projection_keys(schema: str) -> set[str]:
    common = {
        "schema",
        "appliance_id",
        "generated_at",
        "build_id",
        "candidate_id",
        "candidate_sha256",
        "generation_id",
        "base_generation",
        "source_id",
        "lifecycle",
        "provenance",
        "projection_sha256",
    }
    if schema == BUILD_EVIDENCE_SCHEMA:
        return common | {
            "source_revision",
            "target",
            "bundle_sha256",
            "tests_sha256",
            "privilege_envelope",
            "gates",
            "invariants",
        }
    if schema == GENERATION_DIFF_SCHEMA:
        return common | {
            "parent_source_id",
            "files",
            "total_changed_lines",
            "truncated",
        }
    raise IntegrityError("unsupported introspection projection schema")


def _projection_directory(root: Path, kind: str) -> Path:
    if kind not in {"build-evidence", "generation-diffs"}:
        raise IntegrityError("unsupported introspection projection kind")
    directory = root / "introspection-evidence" / kind
    for path in (directory.parent, directory):
        info = path.lstat()
        if (
            not stat.S_ISDIR(info.st_mode)
            or stat.S_ISLNK(info.st_mode)
            or info.st_uid not in {0, os.geteuid()}
            or stat.S_IMODE(info.st_mode) != 0o2750
        ):
            raise IntegrityError("introspection projection directory identity failed")
    if directory.parent.stat().st_gid != directory.stat().st_gid:
        raise IntegrityError("introspection projection directory groups differ")
    return directory


def _read_projection(path: Path, root: Path, schema: str) -> dict[str, Any]:
    directory = _projection_directory(root, path.parent.name)
    if path.parent != directory or path.suffix != ".json":
        raise IntegrityError("introspection projection path is outside its fixed directory")
    before = path.lstat()
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_uid not in {0, os.geteuid()}
        or before.st_gid != directory.stat().st_gid
        or stat.S_IMODE(before.st_mode) != 0o440
        or before.st_size > MAX_EVIDENCE_BYTES
    ):
        raise IntegrityError("introspection projection file identity failed")
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    try:
        opened = os.fstat(descriptor)
        data = b""
        while len(data) <= MAX_EVIDENCE_BYTES:
            chunk = os.read(descriptor, min(64 * 1024, MAX_EVIDENCE_BYTES + 1 - len(data)))
            if not chunk:
                break
            data += chunk
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    current = path.lstat()
    identity = lambda item: (item.st_dev, item.st_ino, item.st_size, item.st_mtime_ns)
    if (
        len(data) > MAX_EVIDENCE_BYTES
        or len(data) != before.st_size
        or identity(before) != identity(opened)
        or identity(opened) != identity(after)
        or identity(after) != identity(current)
    ):
        raise IntegrityError("introspection projection changed while being read")
    try:
        value = json.loads(data)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise IntegrityError("introspection projection is invalid JSON") from error
    if not isinstance(value, dict) or set(value) != _exact_projection_keys(schema):
        raise IntegrityError("introspection projection has unknown or missing fields")
    claimed = value.get("projection_sha256")
    if not _hex64(claimed):
        raise IntegrityError("introspection projection hash is malformed")
    unhashed = dict(value)
    unhashed["projection_sha256"] = ""
    if sha256_bytes(canonical_bytes(unhashed)) != claimed:
        raise IntegrityError("introspection projection self-hash failed")
    _validate_projection_content(value, schema)
    return value


def _validate_projection_content(value: dict[str, Any], schema: str) -> None:
    if (
        value.get("schema") != schema
        or value.get("provenance") != EVIDENCE_PROVENANCE
        or not isinstance(value.get("generated_at"), int)
        or isinstance(value.get("generated_at"), bool)
        or value["generated_at"] < 0
    ):
        raise IntegrityError("introspection projection provenance or timestamp failed")
    for key in (
        "appliance_id",
        "build_id",
        "candidate_id",
        "generation_id",
        "base_generation",
    ):
        item = value.get(key)
        if not isinstance(item, str) or IDENTIFIER_RE.fullmatch(item) is None:
            raise IntegrityError(f"introspection projection identifier failed: {key}")
    for key in ("candidate_sha256",):
        item = value.get(key)
        if not _hex64(item):
            raise IntegrityError(f"introspection projection digest failed: {key}")
    source_id = value.get("source_id")
    if not _source_id(source_id):
        raise IntegrityError("introspection projection source identity failed")
    _validate_lifecycle(value.get("lifecycle"))
    if schema == BUILD_EVIDENCE_SCHEMA:
        _validate_build_projection(value)
    else:
        _validate_diff_projection(value)


def _validate_lifecycle(value: Any) -> None:
    if not isinstance(value, dict) or set(value) != {"status", "events"}:
        raise IntegrityError("introspection lifecycle shape failed")
    events = value.get("events")
    if (
        not isinstance(value.get("status"), str)
        or not isinstance(events, list)
        or not 1 <= len(events) <= MAX_LIFECYCLE_EVENTS
    ):
        raise IntegrityError("introspection lifecycle bounds failed")
    for event in events:
        if (
            not isinstance(event, dict)
            or set(event) != {"phase", "recorded_at", "authority"}
            or not isinstance(event.get("phase"), str)
            or not isinstance(event.get("authority"), str)
            or not isinstance(event.get("recorded_at"), int)
            or isinstance(event.get("recorded_at"), bool)
            or event["recorded_at"] < 0
        ):
            raise IntegrityError("introspection lifecycle event failed")


def _validate_build_projection(value: dict[str, Any]) -> None:
    for key in (
        "bundle_sha256",
        "tests_sha256",
    ):
        item = value.get(key)
        if not _hex64(item):
            raise IntegrityError(f"build projection digest failed: {key}")
    for key in ("source_revision", "target", "privilege_envelope"):
        item = value.get(key)
        if not isinstance(item, str) or not item or len(item) > 256 or "\x00" in item:
            raise IntegrityError(f"build projection field failed: {key}")
    gates = value.get("gates")
    if not isinstance(gates, list) or not 1 <= len(gates) <= 128:
        raise IntegrityError("build projection gate bounds failed")
    exact_gate = {
        "label",
        "executable_sha256",
        "argv_sha256",
        "exit_code",
        "timed_out",
        "duration_ms",
    }
    for gate in gates:
        if not isinstance(gate, dict) or set(gate) != exact_gate:
            raise IntegrityError("build projection gate shape failed")
        exit_code = gate.get("exit_code")
        duration = gate.get("duration_ms")
        if (
            not isinstance(gate.get("label"), str)
            or not gate["label"]
            or len(gate["label"]) > 96
            or not _hex64(gate.get("executable_sha256"))
            or not _hex64(gate.get("argv_sha256"))
            or not isinstance(exit_code, int)
            or isinstance(exit_code, bool)
            or exit_code != 0
            or gate.get("timed_out") is not False
            or not isinstance(duration, int)
            or isinstance(duration, bool)
            or duration < 0
        ):
            raise IntegrityError("build projection gate evidence failed")
    invariants = value.get("invariants")
    if not isinstance(invariants, dict) or set(invariants) != {
        "candidate_replay_sha256",
        "package_replay_sha256",
        "immutable_invariants",
        "offline_locked",
        "network_policy",
    }:
        raise IntegrityError("build projection invariant shape failed")
    if (
        not _hex64(invariants.get("candidate_replay_sha256"))
        or not _hex64(invariants.get("package_replay_sha256"))
        or invariants.get("immutable_invariants") is not True
        or invariants.get("offline_locked") is not True
        or invariants.get("network_policy") != "private-network-none:v1"
    ):
        raise IntegrityError("build projection invariant evidence failed")


def _validate_diff_projection(value: dict[str, Any]) -> None:
    parent = value.get("parent_source_id")
    files = value.get("files")
    total = value.get("total_changed_lines")
    if (
        not _source_id(parent)
        or not isinstance(files, list)
        or not 1 <= len(files) <= 25
        or not isinstance(total, int)
        or isinstance(total, bool)
        or not 0 <= total <= 4_000
        or value.get("truncated") is not False
    ):
        raise IntegrityError("generation diff bounds failed")
    exact_file = {"path", "source_sha256", "content_sha256", "changed_lines"}
    seen: set[str] = set()
    computed_total = 0
    for item in files:
        if not isinstance(item, dict) or set(item) != exact_file:
            raise IntegrityError("generation diff file shape failed")
        path = item.get("path")
        changed_lines = item.get("changed_lines")
        if (
            not _safe_source_path(path)
            or path in seen
            or not _hex64(item.get("source_sha256"))
            or not _hex64(item.get("content_sha256"))
            or not isinstance(changed_lines, int)
            or isinstance(changed_lines, bool)
            or not 0 <= changed_lines <= 4_000
        ):
            raise IntegrityError("generation diff file evidence failed")
        seen.add(path)
        computed_total += changed_lines
    if computed_total != total:
        raise IntegrityError("generation diff changed-line total failed")


def _write_projection(path: Path, value: dict[str, Any], root: Path) -> None:
    directory = _projection_directory(root, path.parent.name)
    if path.parent != directory:
        raise IntegrityError("introspection projection write escaped its fixed directory")
    projected = dict(value)
    projected["projection_sha256"] = ""
    projected["projection_sha256"] = sha256_bytes(canonical_bytes(projected))
    encoded = canonical_bytes(projected)
    if len(encoded) > MAX_EVIDENCE_BYTES:
        raise IntegrityError("introspection projection exceeds its byte bound")
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=directory)
    try:
        os.fchmod(descriptor, 0o440)
        with os.fdopen(descriptor, "wb", closefd=True) as handle:
            handle.write(encoded)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        directory_descriptor = os.open(directory, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(directory_descriptor)
        finally:
            os.close(directory_descriptor)
    except BaseException:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise


def _lifecycle(supervisor: Any, build_id: str, generation_id: str, initial: dict[str, Any]) -> dict[str, Any]:
    events = [dict(initial["events"][0])]
    status = "installed_pending_stage_verification"
    build_phases = {
        "build_profile_completed": "build_completed",
        "stage_verified": "stage_verified",
    }
    activation_phases = {
        "scheduled_intent_consumed": "promotion_authorized",
        "probation_started": "probation_started",
        "probation_accepted": "probation_accepted",
        "activation_failed_previous_confirmed": "activation_failed",
        "activation_failed_active_slot_unconfirmed": "activation_failed",
        "activation_success_active_slot_mismatch": "activation_failed",
    }
    for record in supervisor.ledger("build").read():
        core = record["core"]
        payload = core["payload"]
        phase = build_phases.get(core["kind"])
        if phase is None or payload.get("build_id") != build_id:
            continue
        events.append(
            {
                "phase": phase,
                "recorded_at": int(core["recorded_at"]),
                "authority": "authenticated_immutable_supervisor_ledger",
            }
        )
        status = "built" if phase == "build_completed" else "staged"
    for record in supervisor.ledger("activation").read():
        core = record["core"]
        payload = core["payload"]
        phase = activation_phases.get(core["kind"])
        if phase is not None and payload.get("build_id") == build_id:
            events.append(
                {
                    "phase": phase,
                    "recorded_at": int(core["recorded_at"]),
                    "authority": "authenticated_immutable_supervisor_ledger",
                }
            )
            status = {
                "promotion_authorized": "activation_authorized",
                "probation_started": "probation",
                "probation_accepted": "accepted",
                "activation_failed": "rejected",
            }[phase]
        if core["kind"] == "rolled_back" and payload.get("from_generation") == generation_id:
            events.append(
                {
                    "phase": "rolled_back",
                    "recorded_at": int(core["recorded_at"]),
                    "authority": "authenticated_immutable_supervisor_ledger",
                }
            )
            status = "rolled_back"
    if len(events) > MAX_LIFECYCLE_EVENTS:
        raise IntegrityError("introspection lifecycle exceeded its immutable event bound")
    return {"status": status, "events": events}


def refresh_introspection_evidence(supervisor: Any) -> dict[str, Any]:
    root = supervisor.config.state_root / "introspection-evidence"
    if not root.exists():
        return {"status": "not_provisioned", "builds": 0, "generations": 0}
    build_directory = _projection_directory(supervisor.config.state_root, "build-evidence")
    diff_directory = _projection_directory(supervisor.config.state_root, "generation-diffs")
    builds = supervisor.builds()
    refreshed = {"builds": 0, "generations": 0, "pending": 0}
    staged_builds = supervisor.staged_builds()
    for build_id, build in builds.items():
        generation_id = str(build["generation_id"])
        build_path = build_directory / f"{build_id}.json"
        diff_path = diff_directory / f"{generation_id}.json"
        if not build_path.exists() or not diff_path.exists():
            if build_id not in staged_builds:
                refreshed["pending"] += 1
                continue
            raise IntegrityError("staged build lacks root-produced introspection evidence")
        build_view = _read_projection(build_path, supervisor.config.state_root, BUILD_EVIDENCE_SCHEMA)
        diff_view = _read_projection(diff_path, supervisor.config.state_root, GENERATION_DIFF_SCHEMA)
        expected = {
            "appliance_id": supervisor.config.appliance_id,
            "build_id": build_id,
            "candidate_id": build["candidate_id"],
            "candidate_sha256": build["candidate_sha256"],
            "generation_id": generation_id,
            "base_generation": build["base_generation"],
        }
        if any(build_view.get(key) != value for key, value in expected.items()) or any(
            diff_view.get(key) != value for key, value in expected.items()
        ):
            raise IntegrityError("introspection evidence differs from authenticated build lineage")
        lifecycle = _lifecycle(supervisor, build_id, generation_id, build_view["lifecycle"])
        for path, view, kind in (
            (build_path, build_view, "builds"),
            (diff_path, diff_view, "generations"),
        ):
            view["generated_at"] = supervisor.now
            view["lifecycle"] = lifecycle
            _write_projection(path, view, supervisor.config.state_root)
            refreshed[kind] += 1
    return {
        "status": "valid",
        **refreshed,
        "retention": "metadata_retained_for_hindsight_after_generation_pruning",
    }


def _candidate_build(supervisor: Any, candidate_id: str) -> tuple[str | None, dict[str, Any] | None]:
    for build_id, payload in reversed(list(supervisor.builds().items())):
        if payload.get("candidate_id") == candidate_id:
            return build_id, payload
    return None, None


def _has_build_event(supervisor: Any, candidate_id: str, kinds: frozenset[str]) -> bool:
    return any(
        record["core"]["kind"] in kinds
        and record["core"]["payload"].get("candidate_id") == candidate_id
        for record in supervisor.ledger("build").read()
    )


def _activation_status(
    supervisor: Any,
    *,
    candidate_id: str,
    build_id: str | None,
    generation_id: str | None,
) -> str | None:
    for record in reversed(supervisor.ledger("activation").read()):
        kind = record["core"]["kind"]
        payload = record["core"]["payload"]
        if (
            kind == "scheduled_intent_terminal_rejected"
            and payload.get("candidate_id") == candidate_id
        ):
            return "rejected"
        if kind == "probation_accepted" and payload.get("build_id") == build_id:
            return "accepted"
        if kind == "activation_failed_rolled_back" and payload.get("build_id") == build_id:
            return "rejected"
        if kind == "rolled_back" and generation_id is not None:
            if payload.get("from_generation") == generation_id:
                return "rolled_back"
        if kind == "scheduled_intent_attested":
            intent = payload.get("intent")
            if isinstance(intent, dict) and intent.get("candidate_id") == candidate_id:
                return "intent_pending"
    return None


def _terminal_rejection_reason(supervisor: Any, candidate_id: str) -> str | None:
    for record in reversed(supervisor.ledger("activation").read()):
        core = record["core"]
        payload = core["payload"]
        if (
            core["kind"] == "scheduled_intent_terminal_rejected"
            and payload.get("candidate_id") == candidate_id
            and _hex64(payload.get("reason_sha256"))
        ):
            return str(payload["reason_sha256"])
    return None


def steward_status(supervisor: Any) -> dict[str, Any]:
    """Return the exact narrow lifecycle view allowed into the steward."""

    state = supervisor.read_state()
    active = supervisor.read_active_generation(required=False)
    candidates = supervisor.candidates()
    if not candidates:
        return {
            "schema": SCHEMA,
            "appliance_id": supervisor.config.appliance_id,
            "generated_at": supervisor.now,
            "current_generation": active,
            "supervisor_mode": _bounded_label(state.get("mode"), "unavailable"),
            "pipeline_busy": bool(
                state.get("inflight")
                or state.get("probation")
                or state.get("mode") == "rescue"
            ),
            "candidate": None,
        }

    candidate_id, candidate = next(reversed(candidates.items()))
    candidate_sha256 = sha256_bytes(canonical_bytes(candidate))
    build_id, build = _candidate_build(supervisor, candidate_id)
    generation_id = str(build.get("generation_id")) if build is not None else None
    status = _activation_status(
        supervisor,
        candidate_id=candidate_id,
        build_id=build_id,
        generation_id=generation_id,
    )

    probation = state.get("probation")
    if isinstance(probation, dict) and probation.get("build_id") == build_id:
        status = "probation"
    elif build_id is not None and build_id in supervisor.staged_builds() and status not in {
        "accepted",
        "rejected",
        "rolled_back",
    }:
        status = "staged"
    elif _has_build_event(
        supervisor,
        candidate_id,
        frozenset(
            {
                "build_profile_failed",
                "build_profile_invalid_output",
                "build_profile_rejected",
            }
        ),
    ):
        status = "rejected"
    elif _has_build_event(supervisor, candidate_id, frozenset({"build_profile_started"})) and not _has_build_event(
        supervisor, candidate_id, frozenset({"build_profile_completed"})
    ):
        status = "building"
    elif status is None:
        status = "intent_pending"

    if state.get("mode") == "rescue" and status in NONTERMINAL:
        status = "rejected"
    candidate_view = {
        "candidate_id": candidate_id,
        "candidate_sha256": candidate_sha256,
        "status": status,
    }
    terminal_reason_sha256 = _terminal_rejection_reason(supervisor, candidate_id)
    if terminal_reason_sha256 is not None:
        candidate_view["terminal_reason_sha256"] = terminal_reason_sha256
    value = {
        "schema": SCHEMA,
        "appliance_id": supervisor.config.appliance_id,
        "generated_at": supervisor.now,
        "current_generation": active,
        "supervisor_mode": _bounded_label(state.get("mode"), "unavailable"),
        "pipeline_busy": status in NONTERMINAL or state.get("mode") == "rescue",
        "candidate": candidate_view,
    }
    return value


def operator_status(supervisor: Any, last_pass: Any) -> dict[str, Any]:
    """Return a deliberately narrow, non-authoritative operator projection.

    The full supervisor status contains ledger heads, key identifiers, policy
    paths, and inbox details.  None of that is needed by an appliance operator
    merely checking whether self-evolution is idle, paused, building, or in
    probation.  This projection therefore carries only bounded lifecycle state
    and a self-hash; its filesystem origin, not the hash, establishes that it
    came from the immutable supervisor.
    """

    state = supervisor.read_state()
    inflight = state.get("inflight")
    probation = state.get("probation")
    due = state.get("due")
    if isinstance(probation, dict):
        pipeline_phase = "probation"
    elif isinstance(inflight, dict):
        raw_phase = inflight.get("phase")
        pipeline_phase = _bounded_label(raw_phase, "inflight")
    elif isinstance(due, dict):
        pipeline_phase = "due"
    else:
        pipeline_phase = "idle"

    profiles = supervisor.profiles.load()
    activation_profile = profiles.get("activate")
    rollback_profile = profiles.get("rollback")
    if activation_profile is None or rollback_profile is None:
        raise RuntimeError("immutable restart command profiles are absent")
    restart_phase = "none"
    restart_upper_bound_seconds = 0
    if pipeline_phase == "profile_invoked":
        restart_phase = "activation"
        restart_upper_bound_seconds = activation_profile.timeout_seconds
    elif pipeline_phase == "rollback_profile_invoked":
        restart_phase = "rollback"
        restart_upper_bound_seconds = rollback_profile.timeout_seconds

    transition_operation = "none"
    transition_status = "none"
    if isinstance(last_pass, dict):
        transition_operation = _bounded_label(last_pass.get("operation"), "supervise")
        transition_status = _bounded_label(last_pass.get("status"), "completed")
        for key in ("activation", "build", "inbox", "probation", "rollback"):
            nested = last_pass.get(key)
            if not isinstance(nested, dict):
                continue
            transition_operation = _bounded_label(
                nested.get("operation"), transition_operation
            )
            transition_status = _bounded_label(nested.get("status"), transition_status)

    lifecycle_events, lifecycle_total = _operator_lifecycle_events(supervisor)
    core = {
        "schema": OPERATOR_SCHEMA,
        "appliance_id": supervisor.config.appliance_id,
        "generated_at": supervisor.now,
        "state_revision": int(state.get("revision", 0)),
        "mode": _bounded_label(state.get("mode"), "unavailable"),
        "active_generation": _bounded_label(
            supervisor.read_active_generation(required=False), "none"
        ),
        "previous_generation": _bounded_label(
            state.get("previous_generation"), "none"
        ),
        "pipeline_phase": pipeline_phase,
        "latest_transition": {
            "operation": transition_operation,
            "status": transition_status,
        },
        "restart_expectation": {
            "phase": restart_phase,
            "maximum_seconds": restart_upper_bound_seconds,
            "basis": "immutable_command_profile_timeout_upper_bound",
        },
        "lifecycle": {
            "schema": "astrid.edge_self_change.operator_lifecycle.v1",
            "events": lifecycle_events,
            "included": len(lifecycle_events),
            "total": lifecycle_total,
            "truncated": lifecycle_total > len(lifecycle_events),
            "maximum_events": MAX_OPERATOR_LIFECYCLE_EVENTS,
            "ledger_heads": {
                name: _ledger_head(supervisor, name)
                for name in ("candidate", "build", "activation", "operator")
            },
        },
        "provenance": "immutable_supervisor_sanitized_projection",
        "authority": "observation_only_not_deployment_authority",
    }
    core_bytes = canonical_bytes(core)
    return {
        "schema": OPERATOR_ENVELOPE_SCHEMA,
        "core": core,
        "core_sha256": sha256_bytes(core_bytes),
    }


def _operator_lifecycle_events(supervisor: Any) -> tuple[list[dict[str, Any]], int]:
    """Project a bounded, body-free history from authenticated private ledgers."""

    source_records: list[tuple[str, dict[str, Any]]] = []
    for ledger_name in ("candidate", "build", "activation", "operator"):
        source_records.extend(
            (ledger_name, record) for record in supervisor.ledger(ledger_name).read()
        )
    source_records.sort(
        key=lambda item: (
            int(item[1]["core"]["recorded_at"]),
            int(item[1]["core"]["sequence"]),
            item[0],
        )
    )
    total = len(source_records)
    selected = source_records[-MAX_OPERATOR_LIFECYCLE_EVENTS:]
    build_evidence = _operator_build_evidence(supervisor)
    scheduled_intents = supervisor.scheduled_intents()
    events = [
        _operator_lifecycle_event(
            ledger_name,
            record,
            build_evidence=build_evidence,
            scheduled_intents=scheduled_intents,
        )
        for ledger_name, record in selected
    ]
    return events, total


def _operator_lifecycle_event(
    ledger_name: str,
    record: dict[str, Any],
    *,
    build_evidence: dict[str, dict[str, Any]],
    scheduled_intents: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    core = record["core"]
    payload = core["payload"]
    kind = str(core["kind"])
    model_pass = payload.get("model_pass")
    model_pass = model_pass if isinstance(model_pass, dict) else {}
    intent_id = _optional_label(payload.get("intent_id") or model_pass.get("intent_id"))
    attested = scheduled_intents.get(intent_id or "", {})
    intent = attested.get("intent") if isinstance(attested, dict) else {}
    intent = intent if isinstance(intent, dict) else {}
    command = _event_command(payload, model_pass)
    build_id = _optional_label(payload.get("build_id"))
    evidence = build_evidence.get(build_id or "", {})
    invariants = evidence.get("invariants")
    invariants = invariants if isinstance(invariants, dict) else {}
    tests_sha256 = _optional_hex(payload.get("tests_sha256") or evidence.get("tests_sha256"))
    package_replay = _optional_hex(invariants.get("package_replay_sha256"))
    facets = _operator_event_facets(ledger_name, kind, bool(evidence))
    terminal_rejection = (
        kind == "scheduled_intent_terminal_rejected"
        and _optional_hex(payload.get("reason_sha256")) is not None
        and payload.get("authority")
        == "terminal_exact_candidate_rejection_no_promotion"
        and payload.get("automatic_retry") is False
    )
    terminal_reason = (
        _optional_hex(payload.get("reason_sha256")) if terminal_rejection else None
    )
    terminal_authority = (
        "terminal_exact_candidate_rejection_no_promotion"
        if terminal_rejection
        else None
    )
    return {
        "schema": OPERATOR_EVENT_SCHEMA,
        "recorded_at": int(core["recorded_at"]),
        "source_ledger": ledger_name,
        "sequence": int(core["sequence"]),
        "event_id": _optional_label(core.get("event_id")),
        "status": _bounded_label(kind, "unknown"),
        "facets": facets,
        "record_sha256": _optional_hex(record.get("record_hash")),
        "candidate_id": _optional_label(
            payload.get("candidate_id") or intent.get("candidate_id")
        ),
        "candidate_sha256": _optional_hex(
            payload.get("candidate_sha256") or intent.get("candidate_sha256")
        ),
        "build_id": build_id,
        "generation_id": _optional_label(
            payload.get("generation_id") or payload.get("to_generation")
        ),
        "from_generation": _optional_label(payload.get("from_generation")),
        "trace_id": _optional_label(payload.get("trace_id") or intent.get("trace_id")),
        "session_id": _optional_label(
            payload.get("session_id") or intent.get("session_id")
        ),
        "turn_id": _optional_label(payload.get("turn_id") or intent.get("turn_id")),
        "response_sha256": _optional_hex(
            payload.get("response_sha256") or intent.get("response_sha256")
        ),
        "terminal_declaration_sha256": _optional_hex(
            payload.get("terminal_declaration_sha256")
            or intent.get("terminal_declaration_sha256")
        ),
        "terminal_reason_sha256": terminal_reason,
        "terminal_authority": terminal_authority,
        "automatic_retry": False if terminal_rejection else None,
        "tests_sha256": tests_sha256,
        "bundle_sha256": _optional_hex(
            payload.get("bundle_sha256") or evidence.get("bundle_sha256")
        ),
        "manifest_sha256": _optional_hex(
            payload.get("manifest_sha256")
            or payload.get("generation_manifest_sha256")
        ),
        "invariant_candidate_replay_sha256": _optional_hex(
            invariants.get("candidate_replay_sha256")
        ),
        "invariant_package_replay_sha256": package_replay,
        "shadow_evidence_sha256": package_replay,
        "shadow_status": (
            "package_replay_hash_only_no_detailed_shadow_claim"
            if package_replay is not None
            else None
        ),
        "command_profile": _optional_label(command.get("profile")),
        "command_executable_sha256": _optional_hex(command.get("executable_sha256")),
        "command_argv_sha256": _optional_hex(command.get("argv_sha256")),
        "command_stdout_sha256": _optional_hex(command.get("stdout_sha256")),
        "command_stderr_sha256": _optional_hex(command.get("stderr_sha256")),
        "command_exit_code": _optional_integer(command.get("exit_code"), minimum=-255, maximum=255),
        "command_timed_out": (
            command.get("timed_out") if isinstance(command.get("timed_out"), bool) else None
        ),
        "provenance": OPERATOR_EVENT_PROVENANCE,
        "authority": OPERATOR_EVENT_AUTHORITY,
        "authored": False,
        "fallback": False,
    }


def _operator_event_facets(ledger_name: str, kind: str, has_build_evidence: bool) -> list[str]:
    facets: set[str] = set()
    combined = f"{ledger_name} {kind}"
    if ledger_name == "operator" and kind.startswith("steward_"):
        facets.add("reflection")
    if ledger_name == "candidate" or "candidate" in combined or "intent" in kind:
        facets.add("candidate")
    if ledger_name == "build" or "build" in kind or "stage" in kind:
        facets.add("build")
    if has_build_evidence or kind in {"build_recorded", "build_profile_completed"}:
        facets.update({"test", "invariant", "shadow"})
    if "activation" in kind or kind in {"scheduled_intent_consumed", "probation_started"}:
        facets.add("activation")
    if any(token in kind for token in ("activation", "restart", "crash", "reconciled")):
        facets.add("restart")
    if "probation" in kind:
        facets.add("probation")
    if "rollback" in kind or kind == "rolled_back":
        facets.update({"rollback", "restart"})
    if not facets:
        facets.add("operator")
    return sorted(facets & OPERATOR_FACETS)


def _event_command(payload: dict[str, Any], model_pass: dict[str, Any]) -> dict[str, Any]:
    for candidate in (payload.get("command_receipt"), model_pass.get("command")):
        if isinstance(candidate, dict):
            return candidate
    return {}


def _operator_build_evidence(supervisor: Any) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for build_id in supervisor.builds():
        path = (
            supervisor.config.state_root
            / "introspection-evidence"
            / "build-evidence"
            / f"{build_id}.json"
        )
        if not path.exists():
            continue
        result[build_id] = _read_projection(
            path, supervisor.config.state_root, BUILD_EVIDENCE_SCHEMA
        )
    return result


def _ledger_head(supervisor: Any, name: str) -> str | None:
    records = supervisor.ledger(name).read()
    return _optional_hex(records[-1].get("record_hash")) if records else None


def _optional_label(value: Any) -> str | None:
    if not isinstance(value, str) or IDENTIFIER_RE.fullmatch(value) is None:
        return None
    return value


def _optional_hex(value: Any) -> str | None:
    return value if _hex64(value) else None


def _optional_integer(value: Any, *, minimum: int, maximum: int) -> int | None:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        return None
    return value


def write_operator_status(path: Path, value: dict[str, Any]) -> None:
    """Atomically replace the pre-provisioned root/operator status file.

    The installer creates a root-owned setgid directory whose group is the
    appliance operator.  The supervisor does not create, chmod, or chown that
    directory; preserving its group on rename prevents a mutable process from
    acquiring write authority while still allowing passwordless reads.
    """

    parent = path.parent
    metadata = parent.lstat()
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid not in {0, os.geteuid()}
        or metadata.st_mode & 0o022
        or metadata.st_mode & stat.S_ISGID == 0
    ):
        raise RuntimeError("operator projection directory is not immutable-supervisor controlled")
    if path.exists() or path.is_symlink():
        target = path.lstat()
        if (
            not stat.S_ISREG(target.st_mode)
            or stat.S_ISLNK(target.st_mode)
            or target.st_nlink != 1
            or target.st_uid not in {0, os.geteuid()}
            or target.st_mode & 0o022
        ):
            raise RuntimeError("operator projection target is unsafe")
    encoded = canonical_bytes(value) + b"\n"
    if len(encoded) > MAX_OPERATOR_PROJECTION_BYTES:
        raise RuntimeError("operator projection exceeds its immutable byte bound")
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=parent)
    try:
        os.fchmod(descriptor, 0o640)
        with os.fdopen(descriptor, "wb", closefd=True) as handle:
            handle.write(encoded)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        directory_fd = os.open(parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    except BaseException:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise


def _bounded_label(value: Any, default: str) -> str:
    if not isinstance(value, str) or not value:
        return default
    bounded = value[:128]
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in bounded):
        return default
    return bounded
