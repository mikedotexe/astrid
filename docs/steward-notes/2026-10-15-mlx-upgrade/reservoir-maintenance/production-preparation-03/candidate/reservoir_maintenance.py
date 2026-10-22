"""Isolated two-role maintenance state machine; no production host or execute CLI.

Production admission remains disabled; its branch is exercised only with fakes.
The Host owns OS actions. This module never imports a service/model, opens a
socket, invokes a command, or signals a process. An unresolved mutation intent
requires read-only recovery inspection; it is never automatically replayed.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import os
from pathlib import Path
import re
import stat
from typing import Any, Protocol


ROLES = ("provider", "reservoir")
LABELS = {"provider": "com.reservoir.coupled-astrid", "reservoir": "com.reservoir.service"}
PORTS = {"provider": 8090, "reservoir": 7881}
FIXTURE_PREFIX = "com.codex.reservoir-maintenance-fixture."
REQUIRED_INPUTS = {"provider_source", "provider_settings", "provider_packages", "model_snapshot",
                   "reservoir_source", "reservoir_candidate", "reservoir_settings", "interpreter",
                   "helpers", "fixture"}
MAX_RECORD = 4 * 1024 * 1024
PRODUCTION_CONTROLLER_ENABLED = False


class Refusal(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise Refusal(message)


def canonical(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def digest(value: Any) -> str:
    return hashlib.sha256(canonical(value)).hexdigest()


def hash_value(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def identity(value: Any) -> dict:
    require(isinstance(value, dict), "process identity missing")
    require(set(value) == {"pid", "started_at", "argv"}, "unexpected process identity fields")
    require(type(value["pid"]) is int and value["pid"] > 1, "invalid process PID")
    require(isinstance(value["started_at"], str) and bool(value["started_at"].strip()), "missing process start")
    require(isinstance(value["argv"], list) and bool(value["argv"])
            and all(isinstance(v, str) and v and "\0" not in v for v in value["argv"]), "invalid argv")
    return value


def validate_plan(value: dict) -> dict:
    plan = copy.deepcopy(value)
    require(plan.get("schema") == "reservoir_maintenance_plan_v1", "unsupported plan")
    require(isinstance(plan.get("transaction_id"), str)
            and re.fullmatch(r"[0-9a-f]{32}", plan["transaction_id"]) is not None, "invalid transaction ID")
    require(isinstance(plan.get("actor"), str) and bool(plan["actor"].strip()), "actor required")
    require(type(plan.get("pause_generation")) is int and plan["pause_generation"] > 0, "pause generation required")
    require(isinstance(plan.get("host_binding"), dict) and type(plan["host_binding"].get("uid")) is int
            and plan["host_binding"]["uid"] > 0 and bool(plan["host_binding"].get("boot_uuid")), "host binding required")
    inputs = plan.get("input_hashes")
    require(isinstance(inputs, dict) and REQUIRED_INPUTS <= set(inputs)
            and all(isinstance(k, str) and hash_value(v) for k, v in inputs.items()), "complete input hashes required")
    roles = plan.get("roles")
    require(isinstance(roles, dict) and set(roles) == set(ROLES), "exactly two roles required")
    for role in ROLES:
        require(roles[role].get("label") == LABELS[role] and roles[role].get("port") == PORTS[role], "role mapping changed")
        identity(roles[role].get("identity"))
    require(roles["provider"]["identity"]["pid"] != roles["reservoir"]["identity"]["pid"], "roles share a PID")
    require(isinstance(plan.get("protected"), dict), "protected process inventory required")
    for protected in plan["protected"].values():
        identity(protected)
    timeout = plan.get("timeout_s", 1500)
    require(type(timeout) in (int, float) and math.isfinite(timeout) and 1 <= timeout <= 1800, "timeout out of bounds")
    plan["timeout_s"] = timeout
    canonical(plan)
    return plan


def read_record(path: Path) -> dict:
    require(not path.is_symlink(), "record is a symlink")
    with path.open("rb") as handle:
        raw = handle.read(MAX_RECORD + 1)
    require(len(raw) <= MAX_RECORD, "record too large")
    value = json.loads(raw)
    require(isinstance(value, dict), "record must be an object")
    return value


def sync_dir(path: Path) -> None:
    fd = os.open(path, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def write_new(path: Path, value: dict) -> None:
    payload = canonical(value) + b"\n"
    require(len(payload) <= MAX_RECORD, "event too large")
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "wb") as handle:
        handle.write(payload)
        handle.flush()
        os.fsync(handle.fileno())
    sync_dir(path.parent)


class Journal:
    """Exclusive, private, append-only event files with a checked hash chain."""

    def __init__(self, root: Path, *, plan: dict | None = None):
        self.root = root.absolute()
        require(self.root.parent.resolve() == self.root.parent, "journal parent is not canonical")
        if plan is not None:
            validated = validate_plan(plan)
            self.root.mkdir(mode=0o700)
            write_new(self.root / "plan.json", validated)
        require(self.root.is_dir() and not self.root.is_symlink(), "invalid journal directory")
        metadata = self.root.stat()
        require(metadata.st_uid == os.getuid() and not stat.S_IMODE(metadata.st_mode) & 0o077, "journal must be private and owned")
        self.plan = validate_plan(read_record(self.root / "plan.json"))
        self.plan_sha256 = digest(self.plan)
        self.events = []
        previous = self.plan_sha256
        for index, path in enumerate(sorted(self.root.glob("event-*.json")), 1):
            require(path.name == f"event-{index:05d}.json", "event sequence incomplete")
            event = read_record(path)
            require(event.get("sequence") == index and event.get("previous_sha256") == previous
                    and event.get("plan_sha256") == self.plan_sha256, "event chain mismatch")
            self.events.append(event)
            previous = digest(event)
        self.previous = previous

    def append(self, event_type: str, **data: Any) -> dict:
        event = {"sequence": len(self.events) + 1, "previous_sha256": self.previous,
                 "plan_sha256": self.plan_sha256, "type": event_type, **copy.deepcopy(data)}
        write_new(self.root / f"event-{event['sequence']:05d}.json", event)
        self.events.append(event)
        self.previous = digest(event)
        return event


class Host(Protocol):
    scope: str  # stdlib_fake, owned_fixture, or separately gated fixed production.
    def production_admission(self, plan: dict) -> dict: ...
    def inspect(self) -> dict: ...
    def inspect_role(self, role: str) -> dict: ...
    def create_hold(self, role: str, guard: dict) -> dict: ...
    def arm_next_spawn(self, role: str, guard: dict, expected: dict) -> dict: ...
    def signal_stop(self, role: str, expected: dict) -> dict: ...
    def remove_held(self, role: str, expected: dict, guard: dict) -> dict: ...
    def collect_witness(self, kind: str, context: dict) -> dict: ...
    def install_repair(self, context: dict) -> dict: ...
    def apply_provider_restart(self, context: dict) -> dict: ...
    def bootstrap_original(self, role: str, context: dict) -> dict: ...
    def release_hold(self, role: str, guard: dict) -> dict: ...
    def monotonic(self) -> float: ...
    def sleep(self, seconds: float) -> None: ...


def check_target(host: Host, role: str, view: dict, plan: dict) -> None:
    require(role in ROLES, "unknown role")
    require(host.scope in {"stdlib_fake", "owned_fixture", "production"}, "unsupported execution scope")
    if host.scope == "production":
        require(PRODUCTION_CONTROLLER_ENABLED is True, "production controller admission disabled")
        expected = f"gui/{plan['host_binding']['uid']}/{LABELS[role]}"
    else:
        expected = (f"fake:{role}" if host.scope == "stdlib_fake" else
                    f"gui/{plan['host_binding']['uid']}/{FIXTURE_PREFIX}{plan['transaction_id']}.{role}")
    require(view.get("target") == expected, "host target is outside the exact owned fixture")
    require(type(view.get("job_loaded")) is bool and isinstance(view.get("listener_pids"), list), "incomplete role inspection")
    if view.get("process") is not None:
        identity(view["process"])


def validate_production_admission(plan: dict, value: dict) -> dict:
    """Check the pure verifier's return contract; Host revalidates its raw inputs."""
    require(isinstance(value, dict) and value.get("schema") == "reservoir_production_admission_v1"
            and value.get("validated") is True and value.get("plan_sha256") == digest(plan)
            and hash_value(value.get("preparation_sha256")), "production admission binding missing")
    references = value.get("references", {})
    require(set(references) == {"execution_review", "fixture", "fixture_review", "real_matrix", "restore_qualification"},
            "production proof reference set incomplete")
    for ref in references.values():
        require(isinstance(ref, dict) and set(ref) == {"path", "sha256"}
                and isinstance(ref["path"], str) and Path(ref["path"]).is_absolute()
                and hash_value(ref["sha256"]), "invalid production proof reference")
    require(references["fixture"]["sha256"] == plan["input_hashes"]["fixture"]
            and value.get("reservoir_candidate_sha256") == plan["input_hashes"]["reservoir_candidate"]
            and value.get("provider_restart_settings_sha256") == plan["input_hashes"].get("provider_restart_settings"),
            "production proof inputs differ")
    original = plan["roles"]["provider"]["identity"]["argv"]
    model = value.get("model_snapshot")
    require(isinstance(model, str) and Path(model).is_absolute() and original.count("--model") == 1
            and original.index("--model") + 1 < len(original), "reviewed model snapshot missing")
    expected = list(original); expected[expected.index("--model") + 1] = model
    require(value.get("provider_restart_argv") == expected, "provider restart changes unrelated arguments")
    artifacts = value.get("integrated_artifacts", {})
    require(artifacts and all(Path(path).is_absolute() and hash_value(sha) for path, sha in artifacts.items()),
            "integrated helper binding missing")
    return copy.deepcopy(value)


def preflight(plan: dict, host: Host) -> dict:
    """Read-only admission/inspection; no output files or OS mutations are made."""
    plan = validate_plan(plan)
    admission = None
    if host.scope == "production":
        require(PRODUCTION_CONTROLLER_ENABLED is True, "production controller admission disabled")
        admission = validate_production_admission(plan, host.production_admission(plan))
    snapshot = copy.deepcopy(host.inspect())
    require(snapshot.get("host_binding") == plan["host_binding"], "host identity changed")
    require(snapshot.get("pause_generation") == plan["pause_generation"] and snapshot.get("paused") is True
            and snapshot.get("pause_actor") == plan["actor"] and snapshot.get("lease") is None, "steward ownership changed")
    require(snapshot.get("input_hashes") == plan["input_hashes"], "input hashes changed")
    require(snapshot.get("protected") == plan["protected"], "protected process changed")
    require((admission is not None or snapshot.get("fixture_mode_authorized") is True)
            and snapshot.get("originals_preserved") is True
            and snapshot.get("rollback_software_ready") is True, "fixture/originals/rollback prerequisites missing")
    require(set(snapshot.get("roles", {})) == set(ROLES), "role inventory incomplete")
    for role in ROLES:
        view = snapshot["roles"][role]
        check_target(host, role, view, plan)
        expected = plan["roles"][role]["identity"]
        require(view["job_loaded"] and view.get("process") == expected
                and view["listener_pids"] == [expected["pid"]] and view.get("held") is None,
                f"{role} original process/listener mismatch")
    return {"schema": "reservoir_maintenance_preflight_v1", "passed": True,
            "plan_sha256": digest(plan), "snapshot_sha256": digest(snapshot), "mutation_performed": False,
            "execution_scope": host.scope, "production_qualified": False,
            **({"production_admission": admission} if admission is not None else {})}


class Controller:
    def __init__(self, journal: Journal, host: Host):
        self.journal, self.host, self.plan = journal, host, journal.plan
        self.guard = {"schema": "model_environment_hold_v1", "transaction_id": self.plan["transaction_id"],
                      "actor": self.plan["actor"]}
        self.receipts: dict[str, dict] = {}
        self.repaired = False
        self.provider_pinned = False
        self.admission = None
        self.parked: set[str] = set()
        self.replacements: dict[str, dict] = {}

    def view(self, role: str) -> dict:
        view = copy.deepcopy(self.host.inspect_role(role))
        check_target(self.host, role, view, self.plan)
        return view

    def invariant(self) -> None:
        current = self.host.inspect()
        require(current.get("host_binding") == self.plan["host_binding"], "host identity changed")
        require(current.get("paused") is True and current.get("pause_generation") == self.plan["pause_generation"]
                and current.get("pause_actor") == self.plan["actor"] and current.get("lease") is None,
                "steward ownership changed")
        require(current.get("protected") == self.plan["protected"], "protected process changed")
        expected = dict(self.plan["input_hashes"])
        if self.repaired:
            expected["reservoir_source"] = expected["reservoir_candidate"]
        if self.provider_pinned:
            expected["provider_settings"] = expected["provider_restart_settings"]
        require(current.get("input_hashes") == expected, "transaction inputs changed")
        for role in self.parked:
            self.absent(role)
        for role, expected_identity in self.replacements.items():
            require(self.view(role).get("process") == expected_identity, f"{role} replacement identity changed")

    def mutate(self, operation: str, role: str | None, function, *args) -> dict:
        self.invariant()
        self.journal.append("mutation_intent", operation=operation, role=role)
        result = function(*args)
        require(isinstance(result, dict), "mutation result must be an explicit receipt")
        self.journal.append("mutation_completed", operation=operation, role=role, result=result)
        return result

    def context(self, **bindings: Any) -> dict:
        common = {"plan_sha256": self.journal.plan_sha256,
                  "provider_old_identity_sha256": digest(self.plan["roles"]["provider"]["identity"]),
                  "reservoir_old_identity_sha256": digest(self.plan["roles"]["reservoir"]["identity"])}
        common.update(bindings)
        return {"transaction_id": self.plan["transaction_id"], "bindings": common,
                "execution_scope": self.host.scope, "production_qualified": False,
                "plan": copy.deepcopy(self.plan), "receipts": copy.deepcopy(self.receipts)}

    def witness(self, kind: str, **bindings: Any) -> dict:
        self.invariant()
        context = self.context(**bindings)
        receipt = self.host.collect_witness(kind, context)
        require(isinstance(receipt, dict) and receipt.get("schema") == "reservoir_maintenance_witness_v1"
                and receipt.get("kind") == kind and receipt.get("transaction_id") == self.plan["transaction_id"]
                and receipt.get("passed") is True and isinstance(receipt.get("evidence"), dict), f"{kind} witness failed or incomplete")
        actual = receipt.get("bindings", {})
        require(all(actual.get(key) == value for key, value in context["bindings"].items()), f"{kind} witness binding mismatch")
        self.receipts[kind] = copy.deepcopy(receipt)
        self.journal.append("witness", kind=kind, receipt=receipt)
        return receipt

    def absent(self, role: str) -> None:
        view = self.view(role)
        require(view["job_loaded"] is False and view.get("process") is None and not view["listener_pids"], f"{role} is not absent")

    def held(self, role: str) -> dict | None:
        view = self.view(role)
        original = self.plan["roles"][role]["identity"]
        if view.get("process") == original:
            return None
        if (view["job_loaded"] and view.get("process") is None
                and not view["listener_pids"] and view.get("held") is None):
            return None  # launchd has reaped the original but has not assigned the next PID yet.
        held = view.get("held")
        require(view["job_loaded"] and view.get("process") is not None and not view["listener_pids"]
                and isinstance(held, dict), f"unexpected {role} replacement")
        require(held.get("guard") == self.guard and held.get("identity") == view["process"]
                and held.get("hold_only") is True and held.get("stdlib_only") is True
                and held.get("state_touched") is False, f"unowned or stateful {role} holder")
        return view["process"]

    def wait_held(self, role: str) -> dict:
        deadline = self.host.monotonic() + self.plan["timeout_s"]
        while True:
            self.invariant()
            result = self.held(role)
            if result is not None:
                return result
            require(self.host.monotonic() < deadline, f"{role} drain timed out; no escalation; hold retained")
            self.host.sleep(min(.25, max(0, deadline - self.host.monotonic())))

    def stop_into_hold(self, role: str) -> dict:
        expected = self.plan["roles"][role]["identity"]
        require(self.view(role).get("process") == expected, f"{role} changed before hold")
        result = self.mutate("create_hold", role, self.host.create_hold, role, self.guard)
        require(result.get("guard") == self.guard, "hold ownership mismatch")
        armed = self.mutate("arm_next_spawn", role, self.host.arm_next_spawn, role, self.guard, expected)
        require(armed.get("armed") is True, "next spawn override not positively armed; no signal")
        require(self.view(role).get("process") == expected, f"{role} changed before signal")
        signal_receipt = self.mutate("signal_stop", role, self.host.signal_stop, role, expected)
        require(signal_receipt.get("signal") == "SIGTERM" and signal_receipt.get("identity") == expected
                and signal_receipt.get("force_used") is False, "cooperative signal receipt mismatch")
        holder = self.wait_held(role)
        self.journal.append("held_verified", role=role, identity=holder)
        return holder

    def remove_holder(self, role: str, holder: dict) -> None:
        require(self.held(role) == holder, "held identity changed before removal")
        self.mutate("remove_held", role, self.host.remove_held, role, holder, self.guard)
        self.absent(role)
        self.parked.add(role)
        self.journal.append("job_absent", role=role)

    def ready_identity(self, role: str, expected: dict) -> None:
        view = self.view(role)
        require(view["job_loaded"] and view.get("process") == expected
                and view["listener_pids"] == [expected["pid"]] and view.get("held") is None,
                f"{role} readiness process/listener mismatch")

    def run(self) -> dict:
        require(not self.journal.events, "existing transaction requires read-only recovery inspection; never replay automatically")
        try:
            proof = preflight(self.plan, self.host)
            self.journal.append("preflight", receipt=proof)
            self.admission = proof.get("production_admission")
            provider_holder = self.stop_into_hold("provider")
            self.witness("provider_drain")
            self.remove_holder("provider", provider_holder)
            self.absent("provider")
            reservoir_holder = self.stop_into_hold("reservoir")
            self.witness("final_snapshot")
            self.remove_holder("reservoir", reservoir_holder)
            self.absent("provider")
            self.absent("reservoir")
            self.witness("backup", final_snapshot_sha256=digest(self.receipts["final_snapshot"]))
            self.witness("private_restore", backup_sha256=digest(self.receipts["backup"]),
                         repair_source_sha256=self.plan["input_hashes"]["reservoir_candidate"],
                         interpreter_sha256=self.plan["input_hashes"]["interpreter"],
                         configuration_sha256=self.plan["input_hashes"]["reservoir_settings"])
            self.absent("provider")
            self.absent("reservoir")
            installed = self.mutate("install_repair", None, self.host.install_repair, self.context())
            require(installed.get("source_sha256") == self.plan["input_hashes"]["reservoir_candidate"], "repair install mismatch")
            self.repaired = True
            self.invariant()
            self.absent("reservoir")
            started = self.mutate("bootstrap_original", "reservoir", self.host.bootstrap_original, "reservoir", self.context())
            new_reservoir = identity(started.get("identity"))
            require(new_reservoir != self.plan["roles"]["reservoir"]["identity"], "reservoir did not change identity")
            require(new_reservoir["argv"] == self.plan["roles"]["reservoir"]["identity"]["argv"], "reservoir command changed")
            self.parked.remove("reservoir")
            self.replacements["reservoir"] = new_reservoir
            self.witness("startup", private_restore_sha256=digest(self.receipts["private_restore"]),
                         reservoir_new_identity_sha256=digest(new_reservoir))
            self.ready_identity("reservoir", new_reservoir)
            self.absent("provider")
            if self.admission is not None:
                applied = self.mutate("apply_provider_restart", "provider", self.host.apply_provider_restart, self.context())
                require(applied.get("provider_settings_sha256") == self.plan["input_hashes"]["provider_restart_settings"]
                        and applied.get("provider_restart_argv") == self.admission["provider_restart_argv"],
                        "applied provider restart settings differ")
                # mutate has durably recorded completion before changing this expectation.
                self.provider_pinned = True
                self.invariant()
            started = self.mutate("bootstrap_original", "provider", self.host.bootstrap_original, "provider", self.context())
            new_provider = identity(started.get("identity"))
            require(new_provider != self.plan["roles"]["provider"]["identity"], "provider did not change identity")
            expected_argv = (self.admission["provider_restart_argv"] if self.admission is not None else
                             self.plan["roles"]["provider"]["identity"]["argv"])
            require(new_provider["argv"] == expected_argv, "original provider command/model/settings changed")
            self.parked.remove("provider")
            self.replacements["provider"] = new_provider
            self.witness("provider_ready", provider_new_identity_sha256=digest(new_provider),
                         startup_sha256=digest(self.receipts["startup"]))
            self.ready_identity("provider", new_provider)
            self.invariant()
            for role in ROLES:
                released = self.mutate("release_hold", role, self.host.release_hold, role, self.guard)
                require(released.get("released") is True, "hold release not positively confirmed")
            result = {"schema": "reservoir_maintenance_result_v1", "status": "verified",
                      "transaction_id": self.plan["transaction_id"], "plan_sha256": self.journal.plan_sha256,
                      "provider": new_provider, "reservoir": new_reservoir,
                      "force_used": False, "runtime_state_rollback": False,
                      "execution_scope": self.host.scope, "production_qualified": False,
                      "no_missed_input_claimed": False, "witness_hashes": {k: digest(v) for k, v in self.receipts.items()}}
            self.journal.append("completed", receipt=result)
            return result
        except BaseException as error:
            # Do not clear, recreate, or transfer another owner's hold in recovery.
            # No automatic rollback or second signal is attempted.
            self.journal.append("failed", error_type=type(error).__name__, error=str(error)[:1500],
                                automatic_action="none", holds_released_by_error_handler=False)
            raise


def inspect_recovery(journal: Journal, host: Host) -> dict:
    """Read-only classification of interruption; never cancel a pending debug override."""
    pending = None
    for event in journal.events:
        if event["type"] == "mutation_intent":
            require(pending is None, "overlapping unresolved mutation intents")
            pending = {"operation": event["operation"], "role": event["role"], "sequence": event["sequence"]}
        elif event["type"] == "mutation_completed":
            require(pending is not None and all(pending[k] == event[k] for k in ("operation", "role")), "completion without matching intent")
            pending = None
    views = {}
    for role in ROLES:
        views[role] = host.inspect_role(role)
        check_target(host, role, views[role], journal.plan)
    armed = {event["role"] for event in journal.events
             if event["type"] == "mutation_completed" and event.get("operation") == "arm_next_spawn"}
    absent = {event["role"] for event in journal.events if event["type"] == "job_absent"}
    unresolved_debug = sorted(armed - absent)
    if pending and pending["operation"] == "arm_next_spawn" and pending["role"] not in unresolved_debug:
        unresolved_debug.append(pending["role"])
    return {"schema": "reservoir_maintenance_recovery_v1", "transaction_id": journal.plan["transaction_id"],
            "plan_sha256": journal.plan_sha256, "pending_mutation": pending,
            "possibly_pending_debug_roles": sorted(unresolved_debug), "observed_roles": views,
            "automatic_resume_allowed": False, "mutation_performed": False,
            "instruction": "Retain owned holds and receipts. Reconcile exact current identities and pending override before an explicitly reviewed recovery; never re-arm, repeat a signal, clear holds, or bootstrap from a guessed phase."}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-plan", type=Path, required=True,
                        help="Validate a saved plan without inspecting or controlling services")
    args = parser.parse_args()
    plan = validate_plan(read_record(args.check_plan))
    print(json.dumps({"plan_sha256": digest(plan), "valid": True, "execution_available": False}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
