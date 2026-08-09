"""A/B activation, crash recovery, probation, and operator state machine."""

from __future__ import annotations

import fcntl
import os
import stat
import time
from contextlib import contextmanager
from pathlib import Path
from typing import Any, Iterator, Mapping

from .model import (
    GENERATION_SCHEMA,
    MAX_GENERATION_ENTRIES,
    PIPELINE_MAX_SECONDS,
    PROBATION_SECONDS,
    DUE_COALESCE_SECONDS,
    STATE_SCHEMA,
    Build,
    Candidate,
    Config,
    IntegrityError,
    IntentAttestor,
    Ledger,
    ProfileError,
    ScheduledIntent,
    Signer,
    SupervisorError,
    _identifier,
    _lstat_no_link,
    _require_exact_keys,
    atomic_write,
    canonical_bytes,
    ensure_private_dir,
    read_json,
    sha256_bytes,
    sha256_file,
    validate_bounded_path,
)
from .profiles import PipelineManager, ProfileStore, render_profile
from .profiles import run_command_profile, temporary_profile_scratch
class Supervisor:
    def __init__(self, config: Config, *, now: int | None = None):
        self.config = config
        self.now = int(time.time()) if now is None else now
        self.signer = Signer(config.signing_key)
        self.intent_attestor = IntentAttestor(config.intent_attestation_key)
        if self.signer.key_id == self.intent_attestor.key_id:
            raise IntegrityError("ledger and intent attestation keys contain identical material")
        self.profiles = ProfileStore(config.command_profiles)
        self.pipeline = PipelineManager(self)

    def _finish_pass(self, result: dict[str, Any], *, execute: bool) -> dict[str, Any]:
        if execute:
            self.pipeline.project_status(result)
        return result

    @property
    def state_path(self) -> Path:
        return self.config.state_root / "state.json"
    def ledger(self, name: str) -> Ledger:
        if name not in {"candidate", "build", "activation", "operator"}:
            raise SupervisorError("unsupported ledger")
        return Ledger(self.config.state_root / "ledgers" / f"{name}.jsonl", self.signer, name)
    def initial_state(self) -> dict[str, Any]:
        active = self.read_active_generation(required=False)
        return {
            "schema": STATE_SCHEMA,
            "appliance_id": self.config.appliance_id,
            "revision": 0,
            # Installation is not authorization to evolve the live appliance.
            # The steward may produce one genuine reflection while this mode is
            # in force, but the immutable supervisor leaves its signed envelope
            # queued until the operator explicitly resumes after acceptance.
            "mode": "paused",
            "paused_reason": "bootstrap_acceptance_pending",
            "active_generation": active,
            "previous_generation": None,
            "probation": None,
            "inflight": None,
            "due": {
                "first_requested_at": self.now,
                "not_before": self.now,
                "reasons": ["bootstrap"],
                "coalesced_count": 1,
            },
            "last_steward_started_at": None,
            "synthetic_harness": None,
        }

    def require_running_pipeline(self, operation: str) -> None:
        """Reject mutable pipeline operations outside the explicit running mode."""

        state = self.read_state()
        if state["mode"] != "running" or state.get("probation") or state.get("inflight"):
            raise SupervisorError(f"{operation} is blocked by supervisor state")

    def read_state(self) -> dict[str, Any]:
        if not self.state_path.exists():
            return self.initial_state()
        envelope = read_json(self.state_path, root=self.config.state_root)
        _require_exact_keys(
            envelope,
            required={"schema", "state", "state_sha256", "auth"},
            optional=set(),
            label="state envelope",
        )
        state = envelope["state"]
        if envelope["schema"] != STATE_SCHEMA or not isinstance(state, dict):
            raise IntegrityError("unsupported state envelope")
        digest = sha256_bytes(canonical_bytes(state))
        auth = envelope["auth"]
        if (
            envelope["state_sha256"] != digest
            or not isinstance(auth, dict)
            or auth.get("algorithm") != "hmac-sha256"
            or auth.get("key_id") != self.signer.key_id
            or not self.signer.verify(canonical_bytes({"schema": STATE_SCHEMA, "state_sha256": digest}), str(auth.get("signature") or ""))
        ):
            raise IntegrityError("state authentication failed")
        if (
            state.get("schema") != STATE_SCHEMA
            or state.get("appliance_id") != self.config.appliance_id
            or state.get("mode") not in {"running", "paused", "rescue"}
        ):
            raise IntegrityError("state payload is invalid")
        return state

    def write_state(self, state: Mapping[str, Any]) -> None:
        value = dict(state)
        if value.get("appliance_id") != self.config.appliance_id:
            raise IntegrityError("state appliance identity does not match this appliance")
        value["schema"] = STATE_SCHEMA
        value["revision"] = int(value.get("revision", 0)) + 1
        digest = sha256_bytes(canonical_bytes(value))
        signed = canonical_bytes({"schema": STATE_SCHEMA, "state_sha256": digest})
        envelope = {
            "schema": STATE_SCHEMA,
            "state": value,
            "state_sha256": digest,
            "auth": {
                "algorithm": "hmac-sha256",
                "key_id": self.signer.key_id,
                "signature": self.signer.sign(signed),
            },
        }
        atomic_write(self.state_path, canonical_bytes(envelope) + b"\n")

    @contextmanager
    def locked(self) -> Iterator[None]:
        ensure_private_dir(self.config.state_root)
        lock_path = self.config.state_root / "supervisor.lock"
        descriptor = os.open(lock_path, os.O_RDWR | os.O_CREAT, 0o600)
        try:
            os.fchmod(descriptor, 0o600)
            fcntl.flock(descriptor, fcntl.LOCK_EX)
            yield
        finally:
            fcntl.flock(descriptor, fcntl.LOCK_UN)
            os.close(descriptor)

    def read_active_generation(self, *, required: bool = True) -> str | None:
        link = self.config.active_link
        validate_bounded_path(link.parent, link, require_exists=required, allow_final_symlink=True)
        if not link.exists() and not link.is_symlink():
            if required:
                raise SupervisorError("active generation link is absent")
            return None
        info = link.lstat()
        if not stat.S_ISLNK(info.st_mode):
            raise SupervisorError("active generation pointer is not a symlink")
        try:
            target = link.resolve(strict=True)
        except OSError as error:
            raise SupervisorError(f"active generation pointer is broken: {error}") from error
        validate_bounded_path(self.config.releases_root, target)
        relative = target.relative_to(self.config.releases_root)
        if len(relative.parts) != 1:
            raise SupervisorError("active generation is not a direct release child")
        return _identifier(relative.name, "active generation")

    def generation_path(self, generation_id: str) -> Path:
        generation = _identifier(generation_id, "generation_id")
        return self.config.releases_root / generation

    def validate_generation(self, build: Build) -> Path:
        path = self.generation_path(build.generation_id)
        validate_bounded_path(self.config.releases_root, path)
        info = _lstat_no_link(path, "release generation")
        if (
            not stat.S_ISDIR(info.st_mode)
            or info.st_uid not in {0, os.geteuid()}
            or info.st_mode & 0o222
        ):
            raise SupervisorError("release generation root is not immutable and operator-owned")
        entries = 0
        for root, directories, files in os.walk(path, followlinks=False):
            root_path = Path(root)
            for name in [*directories, *files]:
                entries += 1
                if entries > MAX_GENERATION_ENTRIES:
                    raise SupervisorError("release generation exceeds entry bound")
                item = root_path / name
                item_info = item.lstat()
                if stat.S_ISLNK(item_info.st_mode) or item_info.st_mode & 0o222:
                    raise SupervisorError(f"release generation contains link or mutable entry: {item}")
                if item_info.st_uid not in {0, os.geteuid()}:
                    raise SupervisorError(f"release generation contains untrusted owner: {item}")
        manifest_path = path / ".astrid-edge-generation.json"
        manifest = read_json(manifest_path, root=path, immutable=True)
        _require_exact_keys(
            manifest,
            required={
                "schema",
                "appliance_id",
                "generation_id",
                "build_id",
                "candidate_id",
                "candidate_sha256",
                "base_generation",
                "bundle_sha256",
                "tests_sha256",
                "target",
            },
            optional=set(),
            label="generation manifest",
        )
        expected = {
            "schema": GENERATION_SCHEMA,
            "appliance_id": build.appliance_id,
            "generation_id": build.generation_id,
            "build_id": build.build_id,
            "candidate_id": build.candidate_id,
            "candidate_sha256": build.candidate_sha256,
            "base_generation": build.base_generation,
            "bundle_sha256": build.bundle_sha256,
            "tests_sha256": build.tests_sha256,
            "target": build.target,
        }
        if manifest != expected:
            raise SupervisorError("generation manifest does not bind the recorded build")
        return path

    def switch_active_link(self, generation_id: str) -> None:
        target = self.generation_path(generation_id)
        validate_bounded_path(self.config.releases_root, target)
        parent = self.config.active_link.parent
        validate_bounded_path(parent, parent)
        relative_target = os.path.relpath(target, parent)
        temporary = parent / f".current.{os.getpid()}.{self.now}"
        if temporary.exists() or temporary.is_symlink():
            raise SupervisorError("temporary active-link path already exists")
        os.symlink(relative_target, temporary)
        try:
            os.replace(temporary, self.config.active_link)
            directory_fd = os.open(parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
            try:
                os.fsync(directory_fd)
            finally:
                os.close(directory_fd)
        finally:
            if temporary.is_symlink():
                temporary.unlink()

    def candidates(self) -> dict[str, dict[str, Any]]:
        result: dict[str, dict[str, Any]] = {}
        for record in self.ledger("candidate").read():
            if record["core"]["kind"] == "candidate_recorded":
                payload = record["core"]["payload"]
                result[str(payload["candidate_id"])] = payload
        return result

    def builds(self) -> dict[str, dict[str, Any]]:
        result: dict[str, dict[str, Any]] = {}
        for record in self.ledger("build").read():
            if record["core"]["kind"] == "build_recorded":
                payload = record["core"]["payload"]
                result[str(payload["build_id"])] = payload
        return result

    def staged_builds(self) -> set[str]:
        return {
            str(record["core"]["payload"]["build_id"])
            for record in self.ledger("build").read()
            if record["core"]["kind"] == "stage_verified"
        }

    def scheduled_intents(self) -> dict[str, dict[str, Any]]:
        intents: dict[str, dict[str, Any]] = {}
        for record in self.ledger("activation").read():
            if record["core"]["kind"] == "scheduled_intent_attested":
                payload = record["core"]["payload"]
                intent = payload["intent"]
                intents[str(intent["intent_id"])] = {
                    "intent": intent,
                    "attested_at": record["core"]["recorded_at"],
                    "binding_sha256": payload["binding_sha256"],
                    "envelope_id": payload["envelope_id"],
                    "envelope_sha256": payload["envelope_sha256"],
                }
        return intents

    def consumed_intents(self) -> tuple[set[str], set[tuple[str, str, str]]]:
        ids: set[str] = set()
        bindings: set[tuple[str, str, str]] = set()
        for record in self.ledger("activation").read():
            if record["core"]["kind"] not in {
                "scheduled_intent_consumed",
                "scheduled_intent_terminal_rejected",
            }:
                continue
            payload = record["core"]["payload"]
            ids.add(str(payload["intent_id"]))
            bindings.add(
                (
                    str(payload["trace_id"]),
                    str(payload["response_sha256"]),
                    str(payload["terminal_declaration_sha256"]),
                )
            )
        return ids, bindings

    def record_scheduled_intent(
        self,
        value: Mapping[str, Any],
        *,
        execute: bool,
        allow_recovery: bool = False,
    ) -> dict[str, Any]:
        envelope = self.intent_attestor.verify_envelope(value, self.now)
        candidate = Candidate.parse(envelope["candidate"], self.config.extra_denylist)
        intent = ScheduledIntent.parse(envelope["intent"], self.now, self.config.appliance_id)
        candidate_payload = candidate.payload()
        intent_payload = intent.payload()
        if envelope["candidate"] != candidate_payload or envelope["intent"] != intent_payload:
            raise IntegrityError("attestor envelope contains non-canonical typed records")
        candidate_digest = sha256_bytes(canonical_bytes(candidate_payload))
        if envelope["candidate_sha256"] != candidate_digest:
            raise IntegrityError("attestor envelope candidate digest mismatch")
        existing_candidates = self.candidates()
        if candidate.candidate_id in existing_candidates and existing_candidates[candidate.candidate_id] != candidate_payload:
            raise IntegrityError("attested candidate conflicts with an existing candidate")
        existing_intents = self.scheduled_intents()
        if intent.intent_id in existing_intents:
            existing = existing_intents[intent.intent_id]
            if not allow_recovery:
                raise SupervisorError("scheduled model intent replay detected")
            if (
                existing["intent"] != intent_payload
                or existing["envelope_id"] != envelope["envelope_id"]
                or existing["envelope_sha256"] != envelope["envelope_sha256"]
            ):
                raise IntegrityError("recovered intent does not exactly match its attestation")
        elif any(
            item["envelope_id"] == envelope["envelope_id"]
            for item in existing_intents.values()
        ):
            raise IntegrityError("intent attestor envelope_id replay detected")
        state = self.read_state()
        if (
            intent.candidate_sha256 != candidate_digest
            or intent.base_generation != candidate_payload["base_generation"]
            or intent.current_generation != state.get("active_generation")
            or intent.base_generation != intent.current_generation
        ):
            raise SupervisorError("scheduled model intent does not bind the exact candidate and base")
        result = {
            "operation": "attest_scheduled_model_intent",
            "status": "accepted",
            "intent_id": intent.intent_id,
            "envelope_id": envelope["envelope_id"],
            "envelope_sha256": envelope["envelope_sha256"],
            "binding_sha256": sha256_bytes(canonical_bytes(intent_payload)),
            "dry_run": not execute,
        }
        if execute:
            self.require_running_pipeline("scheduled intent ingestion")
            if candidate.candidate_id not in existing_candidates:
                self.ledger("candidate").append(
                    "candidate_recorded",
                    candidate_payload,
                    f"candidate-{candidate.candidate_id}",
                    self.now,
                )
            if intent.intent_id not in existing_intents:
                self.ledger("activation").append(
                    "scheduled_intent_attested",
                    {
                        "intent": intent_payload,
                        "binding_sha256": result["binding_sha256"],
                        "envelope_id": envelope["envelope_id"],
                        "envelope_sha256": envelope["envelope_sha256"],
                        "attestation_authority": "separate_root_hmac_attestor",
                    },
                    f"intent-{intent.intent_id}",
                    self.now,
                )
        return result

    def record_candidate(self, value: Mapping[str, Any], *, execute: bool) -> dict[str, Any]:
        candidate = Candidate.parse(value, self.config.extra_denylist)
        if candidate.candidate_id in self.candidates():
            raise SupervisorError("candidate replay detected")
        result = {"operation": "record_candidate", "candidate": candidate.payload(), "dry_run": not execute}
        if execute:
            self.require_running_pipeline("candidate recording")
            self.ledger("candidate").append(
                "candidate_recorded", candidate.payload(), f"candidate-{candidate.candidate_id}", self.now
            )
        return result

    def record_build(self, value: Mapping[str, Any], *, execute: bool) -> dict[str, Any]:
        build = Build.parse(value, self.config.target, self.config.appliance_id)
        candidates = self.candidates()
        if build.candidate_id not in candidates:
            raise SupervisorError("build does not reference a recorded candidate")
        if build.build_id in self.builds():
            raise SupervisorError("build replay detected")
        if any(item["candidate_id"] == build.candidate_id for item in self.builds().values()):
            raise SupervisorError("candidate already has an immutable derived build")
        candidate_sha256 = sha256_bytes(canonical_bytes(candidates[build.candidate_id]))
        if (
            build.candidate_sha256 != candidate_sha256
            or build.base_generation != candidates[build.candidate_id]["base_generation"]
            or build.base_generation != self.read_state().get("active_generation")
        ):
            raise SupervisorError("build candidate base generation is stale")
        result = {"operation": "record_build", "build": build.payload(), "dry_run": not execute}
        if execute:
            self.require_running_pipeline("build recording")
            self.ledger("build").append(
                "build_recorded", build.payload(), f"build-{build.build_id}", self.now
            )
        return result

    def _substitutions(self, build: Build | None = None) -> dict[str, str]:
        values = {
            "state_root": str(self.config.state_root),
            "releases_root": str(self.config.releases_root),
            "active_link": str(self.config.active_link),
        }
        if build is not None:
            values.update(
                {
                    "build_id": build.build_id,
                    "candidate_id": build.candidate_id,
                    "generation_dir": str(self.generation_path(build.generation_id)),
                    "previous_generation_dir": str(
                        self.generation_path(str(self.read_state().get("active_generation") or "none"))
                    ),
                    "build_manifest": str(self.config.state_root / "inbox" / f"build-{build.build_id}.json"),
                    "candidate_manifest": str(
                        self.config.state_root / "inbox" / f"candidate-{build.candidate_id}.json"
                    ),
                }
            )
        return values

    def invoke_profile(self, name: str, substitutions: Mapping[str, str], *, execute: bool) -> dict[str, Any]:
        profiles = self.profiles.load()
        if name not in profiles:
            raise ProfileError(f"immutable command profile is not configured: {name}")
        profile = profiles[name]
        command = render_profile(profile, substitutions)
        if not execute:
            return {
                "profile": name,
                "dry_run": True,
                "executable_sha256": sha256_file(profile.executable),
                "argv_sha256": sha256_bytes(canonical_bytes(command)),
                "privilege_envelope": profile.privilege_envelope,
                "run_as_uid": profile.run_as_uid,
                "run_as_gid": profile.run_as_gid,
            }
        with temporary_profile_scratch(profile, name) as scratch:
            return run_command_profile(profile, substitutions, scratch)

    def stage(self, build_id: str, *, execute: bool) -> dict[str, Any]:
        if execute:
            self.require_running_pipeline("generation staging")
        payload = self.builds().get(build_id)
        if payload is None:
            raise SupervisorError("unknown build")
        build = Build.parse(payload, self.config.target, self.config.appliance_id)
        if build_id in self.staged_builds():
            return {"operation": "stage", "build_id": build_id, "status": "already_staged"}
        build_records = self.ledger("build").read()
        stage_started = any(
            record["core"]["kind"] == "stage_profile_started"
            and record["core"]["payload"].get("build_id") == build_id
            for record in build_records
        )
        if stage_started and build_id not in self.staged_builds():
            if execute:
                state = self.read_state()
                state["mode"] = "rescue"
                state["paused_reason"] = "stage_profile_interrupted_no_retry"
                self.write_state(state)
            raise SupervisorError("stage profile was interrupted and will not be retried")
        if execute:
            self.ledger("build").append(
                "stage_profile_started",
                {"build_id": build_id, "candidate_id": build.candidate_id},
                f"stage-started-{build_id}",
                self.now,
            )
        receipt = self.invoke_profile("install", self._substitutions(build), execute=execute)
        result = {"operation": "stage", "build_id": build_id, "dry_run": not execute, "command": receipt}
        if execute:
            if receipt["timed_out"] or receipt["exit_code"] != 0:
                state = self.read_state()
                state["mode"] = "rescue"
                state["paused_reason"] = "stage_profile_failed_no_retry"
                self.write_state(state)
                raise SupervisorError("staging command failed")
            generation = self.validate_generation(build)
            self.ledger("build").append(
                "stage_verified",
                {
                    "build_id": build.build_id,
                    "generation_id": build.generation_id,
                    "generation_manifest_sha256": sha256_file(
                        generation / ".astrid-edge-generation.json"
                    ),
                    "command_receipt": receipt,
                },
                f"stage-{build.build_id}",
                self.now,
            )
        return result

    def mark_due(self, reason: str, *, execute: bool) -> dict[str, Any]:
        reason = _identifier(reason, "due reason")
        state = self.read_state()
        due = state.get("due")
        if due is None:
            last = state.get("last_steward_started_at")
            not_before = self.now if last is None else max(self.now, int(last) + DUE_COALESCE_SECONDS)
            due = {
                "first_requested_at": self.now,
                "not_before": not_before,
                "reasons": [reason],
                "coalesced_count": 1,
            }
        else:
            reasons = sorted(set([*due.get("reasons", []), reason]))[:32]
            due = {
                **due,
                "reasons": reasons,
                "coalesced_count": min(int(due.get("coalesced_count", 0)) + 1, 1_000_000),
            }
        state["due"] = due
        if execute:
            self.write_state(state)
        return {"operation": "mark_due", "due": due, "dry_run": not execute}

    def request_synthetic(self, *, execute: bool) -> dict[str, Any]:
        """Queue one operator harness; compilation occurs only in the service cgroup."""

        state = self.read_state()
        current = state.get("synthetic_harness")
        if current is not None:
            return {
                "operation": "request_synthetic",
                "status": "already_pending",
                "request": current,
                "dry_run": not execute,
            }
        request = {
            "schema": "astrid.edge_self_change.synthetic_request.v1",
            "request_id": f"synthetic-request-{self.now}-{int(state.get('revision', 0)) + 1}",
            "requested_at": self.now,
            "status": "pending",
            "authority": "operator_request_only_no_candidate_or_generation_selection",
        }
        result = {
            "operation": "request_synthetic",
            "status": "queued",
            "request": request,
            "dry_run": not execute,
        }
        if execute:
            state["synthetic_harness"] = request
            self.write_state(state)
            self.ledger("operator").append(
                "synthetic_harness_requested",
                request,
                request["request_id"],
                self.now,
            )
        return result

    def run_synthetic_pending(self, *, execute: bool) -> dict[str, Any]:
        """Run the fixed native harness profile without accepting caller paths."""

        state = self.read_state()
        request = state.get("synthetic_harness")
        if request is None:
            return {"status": "not_requested", "dry_run": not execute}
        if (
            not isinstance(request, dict)
            or request.get("schema") != "astrid.edge_self_change.synthetic_request.v1"
            or request.get("status") not in {"pending", "running"}
            or request.get("authority")
            != "operator_request_only_no_candidate_or_generation_selection"
            or not isinstance(request.get("request_id"), str)
            or not isinstance(request.get("requested_at"), int)
        ):
            raise IntegrityError("synthetic harness request is malformed")
        request_id = _identifier(str(request["request_id"]), "synthetic request id")
        if request["status"] == "running":
            result = {
                "status": "interrupted_no_retry",
                "request_id": request_id,
                "dry_run": not execute,
            }
            if execute:
                self.ledger("operator").append(
                    "synthetic_harness_interrupted",
                    {"request_id": request_id, "automatic_retry": False},
                    f"{request_id}-interrupted",
                    self.now,
                )
                state["synthetic_harness"] = None
                self.write_state(state)
            return result
        if state["mode"] == "rescue" or state.get("probation") or state.get("inflight"):
            return {
                "status": "blocked_by_state",
                "request_id": request_id,
                "dry_run": not execute,
            }
        plan = self.invoke_profile("synthetic", self._substitutions(), execute=False)
        if not execute:
            return {
                "status": "would_run",
                "request_id": request_id,
                "command": plan,
                "dry_run": True,
            }
        request = dict(request)
        request["status"] = "running"
        state["synthetic_harness"] = request
        self.write_state(state)
        self.ledger("operator").append(
            "synthetic_harness_started",
            {
                "request_id": request_id,
                "profile": "synthetic",
                "caller_paths_accepted": False,
            },
            f"{request_id}-started",
            self.now,
        )
        receipt = self.invoke_profile("synthetic", self._substitutions(), execute=True)
        success = self._profile_success(receipt) and isinstance(
            receipt.get("synthetic_result"), dict
        )
        self.ledger("operator").append(
            "synthetic_harness_completed" if success else "synthetic_harness_failed",
            {
                "request_id": request_id,
                "automatic_retry": False,
                "command_receipt": receipt,
                "machine_evidence_not_astrid_authorship": True,
            },
            f"{request_id}-terminal",
            self.now,
        )
        state = self.read_state()
        state["synthetic_harness"] = None
        self.write_state(state)
        return {
            "status": "completed" if success else "failed_no_retry",
            "request_id": request_id,
            "command": receipt,
            "dry_run": False,
        }

    def _profile_success(self, receipt: Mapping[str, Any]) -> bool:
        return not receipt.get("timed_out", False) and receipt.get("exit_code") == 0

    def activate(self, build_id: str, intent_id: str, *, execute: bool) -> dict[str, Any]:
        state = self.read_state()
        if state["mode"] != "running" or state.get("probation") or state.get("inflight"):
            raise SupervisorError("activation is blocked by supervisor state")
        payload = self.builds().get(build_id)
        if payload is None or build_id not in self.staged_builds():
            raise SupervisorError("build is not staged and verified")
        build = Build.parse(payload, self.config.target, self.config.appliance_id)
        self.validate_generation(build)
        attestation = self.scheduled_intents().get(intent_id)
        if attestation is None:
            raise SupervisorError("activation lacks an immutable-supervisor-attested intent")
        intent = ScheduledIntent.parse(
            attestation["intent"], self.now, self.config.appliance_id, require_fresh=False
        )
        if self.now - int(attestation["attested_at"]) > PIPELINE_MAX_SECONDS:
            raise SupervisorError("attested scheduled model intent exceeded the pipeline lifetime")
        candidate_payload = self.candidates().get(intent.candidate_id)
        candidate_digest = (
            sha256_bytes(canonical_bytes(candidate_payload)) if candidate_payload is not None else None
        )
        if (
            intent.candidate_id != build.candidate_id
            or intent.candidate_sha256 != candidate_digest
            or build.candidate_sha256 != intent.candidate_sha256
            or build.base_generation != intent.base_generation
            or intent.base_generation != state.get("active_generation")
            or intent.current_generation != state.get("active_generation")
        ):
            raise SupervisorError("derived build no longer matches the attested candidate and base")
        used_ids, used_bindings = self.consumed_intents()
        binding = (intent.trace_id, intent.response_sha256, intent.terminal_declaration_sha256)
        if intent.intent_id in used_ids or binding in used_bindings:
            raise SupervisorError("scheduled model intent replay detected")
        command_plan = self.invoke_profile("activate", self._substitutions(build), execute=False)
        result = {
            "operation": "activate",
            "from_generation": intent.base_generation,
            "to_generation": build.generation_id,
            "intent_id": intent.intent_id,
            "dry_run": not execute,
            "command": command_plan,
        }
        if not execute:
            return result
        # Consume before A/B mutation: a crash may strand, but never replay, an intent.
        self.ledger("activation").append(
            "scheduled_intent_consumed",
            {
                "intent_id": intent.intent_id,
                "appliance_id": intent.appliance_id,
                "trace_id": intent.trace_id,
                "session_id": intent.session_id,
                "turn_id": intent.turn_id,
                "response_sha256": intent.response_sha256,
                "terminal_declaration_sha256": intent.terminal_declaration_sha256,
                "candidate_id": intent.candidate_id,
                "candidate_sha256": intent.candidate_sha256,
                "build_id": build.build_id,
                "from_generation": intent.base_generation,
                "to_generation": build.generation_id,
            },
            f"intent-consumed-{intent.intent_id}",
            self.now,
        )
        state["inflight"] = {
            "phase": "profile_invoked",
            "intent_id": intent.intent_id,
            "trace_id": intent.trace_id,
            "response_sha256": intent.response_sha256,
            "build_id": build.build_id,
            "from_generation": intent.base_generation,
            "to_generation": build.generation_id,
            "prepared_at": self.now,
        }
        self.write_state(state)
        receipt = self.invoke_profile("activate", self._substitutions(build), execute=True)
        active_after = self.read_active_generation(required=False)
        if not self._profile_success(receipt):
            state = self.read_state()
            if active_after == intent.base_generation:
                state["inflight"] = None
            state["mode"] = "rescue"
            state["paused_reason"] = (
                "activation_helper_failed_previous_confirmed"
                if active_after == intent.base_generation
                else "activation_helper_failed_active_slot_unconfirmed"
            )
            self.write_state(state)
            self.ledger("activation").append(
                "activation_failed_previous_confirmed"
                if active_after == intent.base_generation
                else "activation_failed_active_slot_unconfirmed",
                {
                    "intent_id": intent.intent_id,
                    "trace_id": intent.trace_id,
                    "response_sha256": intent.response_sha256,
                    "build_id": build.build_id,
                    "from_generation": intent.base_generation,
                    "to_generation": build.generation_id,
                    "active_generation_after_helper": active_after,
                    "command_receipt": receipt,
                },
                f"activation-failed-{intent.intent_id}",
                self.now,
            )
            if active_after == intent.base_generation:
                raise SupervisorError(
                    "activation helper failed; native helper confirmed the previous A/B slot and rescue entered"
                )
            raise SupervisorError(
                "activation helper failed without a confirmed previous A/B slot; rescue entered"
            )
        if active_after != build.generation_id:
            state = self.read_state()
            state["mode"] = "rescue"
            state["paused_reason"] = "activation_helper_success_active_slot_mismatch"
            self.write_state(state)
            self.ledger("activation").append(
                "activation_success_active_slot_mismatch",
                {
                    "intent_id": intent.intent_id,
                    "build_id": build.build_id,
                    "expected_generation": build.generation_id,
                    "active_generation_after_helper": active_after,
                    "command_receipt": receipt,
                },
                f"activation-mismatch-{intent.intent_id}",
                self.now,
            )
            raise SupervisorError(
                "activation helper reported success without switching to the attested generation"
            )
        state = self.read_state()
        state["active_generation"] = build.generation_id
        state["previous_generation"] = intent.base_generation
        state["inflight"] = None
        state["probation"] = {
            "intent_id": intent.intent_id,
            "trace_id": intent.trace_id,
            "response_sha256": intent.response_sha256,
            "build_id": build.build_id,
            "generation_id": build.generation_id,
            "previous_generation": intent.base_generation,
            "started_at": self.now,
            "not_before": self.now + PROBATION_SECONDS,
            "health_checks": 0,
        }
        self.write_state(state)
        self.ledger("activation").append(
            "probation_started",
            {
                "intent_id": intent.intent_id,
                "trace_id": intent.trace_id,
                "session_id": intent.session_id,
                "turn_id": intent.turn_id,
                "response_sha256": intent.response_sha256,
                "terminal_declaration_sha256": intent.terminal_declaration_sha256,
                "build_id": build.build_id,
                "from_generation": intent.base_generation,
                "to_generation": build.generation_id,
                "probation_not_before": self.now + PROBATION_SECONDS,
                "command_receipt": receipt,
            },
            f"activation-{intent.intent_id}",
            self.now,
        )
        result["command"] = receipt
        result["probation_not_before"] = self.now + PROBATION_SECONDS
        return result

    def activate_pending_intent(self, *, execute: bool) -> dict[str, Any]:
        state = self.read_state()
        if state["mode"] != "running" or state.get("probation") or state.get("inflight"):
            return {"activation": "blocked_by_state", "dry_run": not execute}
        consumed, _ = self.consumed_intents()
        staged = self.staged_builds()
        ready: list[tuple[str, str]] = []
        for intent_id, attestation in self.scheduled_intents().items():
            if intent_id in consumed:
                continue
            if self.now - int(attestation["attested_at"]) > PIPELINE_MAX_SECONDS:
                continue
            intent = ScheduledIntent.parse(
                attestation["intent"], self.now, self.config.appliance_id, require_fresh=False
            )
            if intent.base_generation != state.get("active_generation"):
                continue
            builds = [
                build_id
                for build_id, payload in self.builds().items()
                if payload["candidate_id"] == intent.candidate_id and build_id in staged
            ]
            if len(builds) == 1:
                ready.append((intent_id, builds[0]))
        if not ready:
            return {"activation": "no_ready_attested_intent", "dry_run": not execute}
        if len(ready) != 1:
            if execute:
                state["mode"] = "rescue"
                state["paused_reason"] = "multiple_ready_autonomous_promotion_intents"
                self.write_state(state)
            return {
                "activation": "rescue_ambiguous_model_intents",
                "ready_intent_ids": sorted(intent_id for intent_id, _ in ready),
                "dry_run": not execute,
            }
        intent_id, build_id = ready[0]
        return self.activate(build_id, intent_id, execute=execute)

    def _rollback_to(
        self, generation: str, reason: str, *, execute: bool, automatic: bool = False
    ) -> dict[str, Any]:
        state = self.read_state()
        current = str(self.read_active_generation(required=True) or "")
        if not current or generation == current:
            raise SupervisorError("rollback requires a distinct current and target generation")
        validate_bounded_path(self.config.releases_root, self.generation_path(generation))
        substitutions = self._substitutions()
        substitutions.update(
            {
                "generation_dir": str(self.generation_path(generation)),
                "previous_generation_dir": str(self.generation_path(current)),
            }
        )
        plan = self.invoke_profile("rollback", substitutions, execute=False)
        result = {
            "operation": "rollback",
            "from_generation": current,
            "to_generation": generation,
            "reason": reason,
            "automatic": automatic,
            "dry_run": not execute,
            "command": plan,
        }
        if not execute:
            return result
        state["inflight"] = {
            "phase": "rollback_profile_invoked",
            "from_generation": current,
            "to_generation": generation,
            "reason": reason,
            "automatic": automatic,
            "prepared_at": self.now,
        }
        self.write_state(state)
        receipt = self.invoke_profile("rollback", substitutions, execute=True)
        active_after = self.read_active_generation(required=False)
        state = self.read_state()
        transition_verified = self._profile_success(receipt) and active_after == generation
        prior_confirmed = not self._profile_success(receipt) and active_after == current
        if transition_verified:
            state["active_generation"] = generation
            state["previous_generation"] = current
            state["probation"] = None
            state["inflight"] = None
            state["mode"] = "paused"
            state["paused_reason"] = reason
        else:
            if prior_confirmed:
                state["inflight"] = None
            state["mode"] = "rescue"
            state["paused_reason"] = (
                "rollback_helper_failed_prior_confirmed"
                if prior_confirmed
                else "rollback_helper_or_active_slot_mismatch"
            )
        self.write_state(state)
        event_id = f"rollback-{self.now}-{sha256_bytes(reason.encode())[:12]}"
        self.ledger("activation").append(
            "rolled_back"
            if transition_verified
            else "rollback_helper_failed_prior_confirmed"
            if prior_confirmed
            else "rollback_helper_or_active_slot_mismatch",
            {
                "from_generation": current,
                "to_generation": generation,
                "reason": reason,
                "automatic": automatic,
                "active_generation_after_helper": active_after,
                "command_receipt": receipt,
            },
            event_id,
            self.now,
        )
        result["command"] = receipt
        if not transition_verified:
            if prior_confirmed:
                raise SupervisorError(
                    "rollback helper failed; the prior active slot remains confirmed and rescue entered"
                )
            raise SupervisorError(
                "rollback helper did not establish the requested slot; rescue entered"
            )
        return result

    def rollback(self, reason: str, *, execute: bool) -> dict[str, Any]:
        if not reason or len(reason) > 240:
            raise SupervisorError("rollback reason must contain 1..240 characters")
        state = self.read_state()
        previous = state.get("previous_generation")
        if not previous:
            raise SupervisorError("no previous generation is available")
        return self._rollback_to(str(previous), reason, execute=execute)

    def recover_crash_state(self, *, execute: bool) -> dict[str, Any]:
        state = self.read_state()
        inflight = state.get("inflight")
        if not inflight:
            active = self.read_active_generation(required=False)
            if active is not None and state.get("active_generation") not in {None, active}:
                if execute:
                    state["mode"] = "rescue"
                    state["paused_reason"] = "active_link_state_mismatch"
                    self.write_state(state)
                return {"recovery": "rescue_active_link_state_mismatch", "dry_run": not execute}
            return {"recovery": "clean", "dry_run": not execute}
        phase = inflight.get("phase")
        old = _identifier(inflight.get("from_generation"), "inflight old generation")
        new = _identifier(inflight.get("to_generation"), "inflight new generation")
        active = self.read_active_generation(required=False)
        if phase in {"prepared", "profile_invoked"} and active == old:
            if execute:
                state["inflight"] = None
                state["mode"] = "paused"
                state["paused_reason"] = "activation_interrupted_before_switch"
                self.write_state(state)
                self.ledger("activation").append(
                    "crash_before_switch",
                    {"from_generation": old, "to_generation": new},
                    f"crash-before-{self.now}",
                    self.now,
                )
            return {"recovery": "paused_before_switch", "dry_run": not execute}
        if phase in {"prepared", "profile_invoked", "link_switched"} and active == new:
            return self._rollback_to(
                old, "crash_during_activation", execute=execute, automatic=True
            )
        if phase == "rollback_profile_invoked" and active == new:
            if not execute:
                return {
                    "recovery": "would_verify_completed_rollback",
                    "dry_run": True,
                }
            health = self.invoke_profile("health", self._substitutions(), execute=True)
            state = self.read_state()
            if self._profile_success(health):
                state["active_generation"] = new
                state["previous_generation"] = old
                state["probation"] = None
                state["inflight"] = None
                state["mode"] = "paused"
                state["paused_reason"] = "rollback_completed_before_supervisor_restart"
                self.write_state(state)
                self.ledger("activation").append(
                    "rollback_reconciled_after_restart",
                    {
                        "from_generation": old,
                        "to_generation": new,
                        "health_receipt": health,
                    },
                    f"rollback-reconciled-{self.now}",
                    self.now,
                )
                return {
                    "recovery": "rollback_reconciled_after_restart",
                    "dry_run": False,
                }
            state["mode"] = "rescue"
            state["paused_reason"] = "rollback_target_unhealthy_after_restart"
            self.write_state(state)
            return {
                "recovery": "rescue_rollback_target_unhealthy",
                "dry_run": False,
            }
        if execute:
            state["mode"] = "rescue"
            state["paused_reason"] = "ambiguous_activation_crash_state"
            self.write_state(state)
        return {"recovery": "rescue_ambiguous_crash_state", "dry_run": not execute}

    def check_probation(self, *, execute: bool) -> dict[str, Any]:
        state = self.read_state()
        probation = state.get("probation")
        if not probation:
            return {"probation": "none", "dry_run": not execute}
        receipt = self.invoke_profile("health", self._substitutions(), execute=execute)
        if not execute:
            return {
                "probation": "would_check",
                "not_before": probation["not_before"],
                "dry_run": True,
                "command": receipt,
            }
        if not self._profile_success(receipt):
            return self._rollback_to(
                str(probation["previous_generation"]),
                "probation_health_failed",
                execute=True,
                automatic=True,
            )
        native = receipt.get("health_result")
        expected_generation = str(probation["generation_id"])
        if (
            not isinstance(native, dict)
            or native.get("active_generation_id") != expected_generation
            or native.get("status") == "failed"
        ):
            return self._rollback_to(
                str(probation["previous_generation"]),
                "probation_native_evidence_invalid",
                execute=True,
                automatic=True,
            )
        state = self.read_state()
        probation = state["probation"]
        probation["health_checks"] = int(probation.get("health_checks", 0)) + 1
        if self.now < int(probation["not_before"]) or native.get("status") != "complete":
            state["probation"] = probation
            self.write_state(state)
            return {
                "probation": "healthy_waiting",
                "not_before": probation["not_before"],
                "health_checks": probation["health_checks"],
                "native_status": native.get("status"),
                "command": receipt,
            }
        state["probation"] = None
        self.write_state(state)
        self.ledger("activation").append(
            "probation_accepted",
            {
                "intent_id": probation["intent_id"],
                "trace_id": probation["trace_id"],
                "response_sha256": probation["response_sha256"],
                "build_id": probation["build_id"],
                "generation_id": probation["generation_id"],
                "probation_seconds": self.now - int(probation["started_at"]),
                "health_checks": probation["health_checks"],
                "command_receipt": receipt,
            },
            f"accepted-{probation['intent_id']}",
            self.now,
        )
        try:
            retention = self.prune(execute=True)
        except SupervisorError:
            state = self.read_state()
            state["mode"] = "rescue"
            state["paused_reason"] = "paired_retention_failed_after_probation_acceptance"
            self.write_state(state)
            raise
        return {
            "probation": "accepted",
            "generation_id": probation["generation_id"],
            "command": receipt,
            "retention": retention,
        }

    def set_mode(
        self, mode: str, reason: str, *, execute: bool, acknowledge_rescue: bool = False
    ) -> dict[str, Any]:
        if mode not in {"running", "paused", "rescue"} or not reason or len(reason) > 240:
            raise SupervisorError("invalid supervisor mode or reason")
        state = self.read_state()
        if mode == "running":
            if state.get("inflight") or state.get("probation"):
                raise SupervisorError("cannot resume during activation or probation")
            if state["mode"] == "rescue" and not acknowledge_rescue:
                raise SupervisorError("rescue recovery requires explicit acknowledgement")
        result = {"operation": mode, "reason": reason, "dry_run": not execute}
        if execute:
            state["mode"] = mode
            state["paused_reason"] = None if mode == "running" else reason
            self.write_state(state)
            self.ledger("operator").append(
                f"mode_{mode}",
                {"reason": reason, "acknowledge_rescue": acknowledge_rescue},
                f"mode-{mode}-{self.now}",
                self.now,
            )
        return result

    def rescue(self, reason: str, *, execute: bool) -> dict[str, Any]:
        if not reason or len(reason) > 240:
            raise SupervisorError("rescue reason must contain 1..240 characters")
        return self.set_mode("rescue", reason, execute=execute)

    def retention(self) -> dict[str, Any]:
        return self.pipeline.retention()

    def prune(self, *, execute: bool) -> dict[str, Any]:
        return self.pipeline.prune(execute=execute)

    def supervise(self, *, execute: bool) -> dict[str, Any]:
        result: dict[str, Any] = {
            "operation": "supervise",
            "dry_run": not execute,
            "recovery": {"recovery": "not_checked"},
            "inbox": {"status": "not_checked"},
            "build": {"status": "not_checked"},
            "probation": {"probation": "not_checked"},
            "activation": {"activation": "not_checked"},
            "synthetic": {"status": "not_checked"},
        }
        try:
            result["recovery"] = self.recover_crash_state(execute=execute)
            state = self.read_state()
            if (
                result["recovery"]["recovery"] == "clean"
                and state.get("synthetic_harness") is not None
            ):
                result["synthetic"] = self.run_synthetic_pending(execute=execute)
            elif result["recovery"]["recovery"] == "clean" and state.get("probation"):
                # The regular supervisor has an isolated empty network
                # namespace. Only the dedicated probation sampler joins the
                # running edge service namespace and may inspect 127.0.0.1
                # reservoir health; probing here would manufacture a false
                # failure and roll back every otherwise healthy candidate.
                result["probation"] = {
                    "probation": "delegated_to_dedicated_sampler",
                    "dry_run": not execute,
                }
            elif result["recovery"]["recovery"] == "clean" and state["mode"] == "running":
                result["inbox"] = self.pipeline.ingest_one(execute=execute)
                result["build"] = self.pipeline.advance_one_build(execute=execute)
                state = self.read_state()
                if (
                    state["mode"] == "running"
                    and not state.get("probation")
                    and not state.get("inflight")
                ):
                    result["activation"] = self.activate_pending_intent(execute=execute)
            elif result["recovery"]["recovery"] == "clean":
                paused = {
                    "status": "paused_queued_untouched",
                    "mode": state["mode"],
                    "reason": state.get("paused_reason"),
                    "dry_run": not execute,
                }
                # Even while paused, consume only already-promoted `.json`
                # wakeups so a level-triggered path unit cannot loop.  Signed
                # intent envelopes and root-owned `.pending` markers remain
                # byte-exact and uningested.
                result["inbox"] = (
                    self.pipeline.ingest_one(execute=True) if execute else dict(paused)
                )
                result["build"] = dict(paused)
                result["activation"] = dict(paused)
            result["retention"] = self.retention()
        except SupervisorError as error:
            result["status"] = "failed_closed"
            result["error"] = str(error)[:240]
            if execute:
                self.pipeline.project_status(result)
            raise
        return self._finish_pass(result, execute=execute)

    def steward(self, *, execute: bool) -> dict[str, Any]:
        return self.pipeline.steward(execute=execute)

    def status(self) -> dict[str, Any]:
        return self.pipeline.status()
