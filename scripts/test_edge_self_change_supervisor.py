#!/usr/bin/env python3
"""Adversarial tests for the CPU-edge self-change supervisor."""

from __future__ import annotations

import dataclasses
import json
import os
import shutil
import stat
import sys
import tempfile
import unittest
from concurrent.futures import ThreadPoolExecutor
from unittest import mock
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import edge_self_change_supervisor as supervisor_module  # noqa: E402
from edge_self_change import model as model_module  # noqa: E402
from edge_self_change import projection as projection_module  # noqa: E402


class SupervisorFixture(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.state_root = self.root / "state"
        self.appliance_root = self.root / "appliance"
        self.releases_root = self.appliance_root / "releases"
        self.active_link = self.appliance_root / "current"
        self.key_path = self.root / "config" / "ledger.key"
        self.intent_key_path = self.root / "config" / "intent.key"
        self.profile_path = self.root / "config" / "profiles.json"
        self.operator_status_root = self.root / "operator-status"
        self.operator_status_path = self.operator_status_root / "operator-status.json"
        self.model_handoff_root = self.root / "steward" / "model-handoff"
        self.state_root.mkdir(mode=0o700)
        self.releases_root.mkdir(parents=True, mode=0o700)
        self.key_path.parent.mkdir(mode=0o700)
        self.operator_status_root.mkdir(mode=0o2750)
        self.model_handoff_root.mkdir(parents=True, mode=0o700)
        self.operator_status_root.chmod(0o2750)
        self.key_path.write_bytes(b"k" * 32)
        self.key_path.chmod(0o600)
        self.intent_key_path.write_bytes(b"i" * 32)
        self.intent_key_path.chmod(0o600)
        self.true_path = Path(shutil.which("true") or "/usr/bin/true").resolve()
        self.false_path = Path(shutil.which("false") or "/usr/bin/false").resolve()
        self.profile_executables = {
            name: self.true_path for name in supervisor_module.PROFILE_ENVELOPES
        }
        self.write_profiles()
        self.config = supervisor_module.Config(
            state_root=self.state_root,
            releases_root=self.releases_root,
            active_link=self.active_link,
            signing_key=self.key_path,
            intent_attestation_key=self.intent_key_path,
            command_profiles=self.profile_path,
            operator_status=self.operator_status_path,
            model_handoff_root=self.model_handoff_root,
            appliance_id="avado-test",
            target="x86_64-unknown-linux-gnu",
        )
        self.make_unbound_generation("g0")
        os.symlink("releases/g0", self.active_link)
        self.supervisor = supervisor_module.Supervisor(self.config, now=1_000)
        state = self.supervisor.initial_state()
        # Most unit tests exercise an operator-accepted running pipeline. The
        # dedicated bootstrap tests below retain the production paused default.
        state["mode"] = "running"
        state["paused_reason"] = None
        self.supervisor.write_state(state)
        original_invoke_profile = supervisor_module.Supervisor.invoke_profile
        self.original_invoke_profile = original_invoke_profile

        def native_transition_fixture(
            instance: supervisor_module.Supervisor,
            name: str,
            substitutions: dict[str, str],
            *,
            execute: bool,
        ) -> dict[str, object]:
            receipt = original_invoke_profile(
                instance, name, substitutions, execute=execute
            )
            if (
                execute
                and name in {"activate", "rollback"}
                and instance._profile_success(receipt)
            ):
                generation = Path(substitutions["generation_dir"]).name
                instance.switch_active_link(generation)
            if execute and name == "health" and instance._profile_success(receipt):
                probation = instance.read_state().get("probation")
                if isinstance(probation, dict):
                    elapsed = max(0, instance.now - int(probation["started_at"]))
                    complete = elapsed >= supervisor_module.PROBATION_SECONDS
                    receipt["health_result"] = {
                        "schema": "astrid.edge_rescue_helper.health.v2",
                        "active_generation_id": probation["generation_id"],
                        "status": "complete" if complete else "active",
                        "coverage_complete": complete,
                        "coverage_due_but_incomplete": False,
                        "samples": 648 if complete else 1,
                        "elapsed_seconds": elapsed,
                        "maximum_sample_gap_seconds": 5,
                        "ledger_head_sha256": "a" * 64,
                        "evidence_sha256": "b" * 64,
                    }
            if execute and name == "synthetic" and instance._profile_success(receipt):
                receipt["synthetic_result"] = {
                    "schema": "astrid.edge_rescue_helper.synthetic_lifecycle.v1",
                    "appliance_id": "avado-test",
                    "production_generation_before": "g0",
                    "synthetic_candidate_id": "synthetic-candidate",
                    "synthetic_build_id": "build-synthetic",
                    "synthetic_generation_id": "gen-synthetic",
                    "sandbox_basename": "synthetic-1-2-3",
                    "evidence_sha256": "c" * 64,
                    "production_unchanged": True,
                    "model_unloaded_and_restored": True,
                }
            if execute and name == "retention" and instance._profile_success(receipt):
                receipt["retention_result"] = {
                    "status": "healthy_nothing_eligible",
                    "active_generation": instance.read_active_generation(required=True),
                    "retained_generations": [
                        item.name
                        for item in instance.config.releases_root.iterdir()
                        if item.is_dir() and not item.is_symlink()
                    ],
                    "retired_generations": [],
                    "ledger_head_sha256": None,
                }
            return receipt

        profile_patch = mock.patch.object(
            supervisor_module.Supervisor,
            "invoke_profile",
            new=native_transition_fixture,
        )
        profile_patch.start()
        self.addCleanup(profile_patch.stop)

    def write_profiles(self, overrides: dict[str, Path] | None = None) -> None:
        executables = dict(self.profile_executables)
        executables.update(overrides or {})
        profiles = {}
        for name, executable in executables.items():
            profiles[name] = {
                "executable": str(executable),
                "executable_sha256": supervisor_module.sha256_file(executable),
                "argv": [],
                "timeout_seconds": 7_200 if name == "synthetic" else 5,
                "privilege_envelope": supervisor_module.PROFILE_ENVELOPES[name],
                "network": "deny",
                "shell": False,
                "candidate_argv": False,
                "run_as_uid": os.geteuid(),
                "run_as_gid": os.getegid(),
            }
        value = {
            "schema": supervisor_module.PROFILE_SCHEMA,
            "trusted_executable_roots": sorted(
                {str(executable.parent) for executable in executables.values()}
            ),
            "profiles": profiles,
        }
        self.profile_path.write_text(json.dumps(value))
        self.profile_path.chmod(0o600)

    def configure_profile(self, name: str, executable: Path, argv: list[str]) -> None:
        value = json.loads(self.profile_path.read_text())
        roots = set(value["trusted_executable_roots"])
        roots.add(str(executable.parent))
        value["trusted_executable_roots"] = sorted(roots)
        value["profiles"][name]["executable"] = str(executable)
        value["profiles"][name]["executable_sha256"] = supervisor_module.sha256_file(
            executable
        )
        value["profiles"][name]["argv"] = argv
        self.profile_path.write_text(json.dumps(value))
        self.profile_path.chmod(0o600)

    def make_unbound_generation(self, generation_id: str) -> Path:
        path = self.releases_root / generation_id
        path.mkdir(mode=0o755)
        path.chmod(0o555)
        return path

    def candidate_value(
        self,
        candidate_id: str = "candidate-1",
        changed_paths: list[str] | None = None,
        privilege_envelope: str = "proposal-only:no-execution:v1",
    ) -> dict[str, object]:
        return {
            "schema": supervisor_module.CANDIDATE_SCHEMA,
            "candidate_id": candidate_id,
            "base_generation": "g0",
            "proposal_sha256": "a" * 64,
            "patch_sha256": "b" * 64,
            "changed_paths": changed_paths
            or ["services/astrid-edge-runtime/src/feature.rs"],
            "created_at": 990,
            "privilege_envelope": privilege_envelope,
        }

    def build_value(
        self,
        build_id: str = "build-1",
        candidate_id: str = "candidate-1",
        generation_id: str = "g1",
        privilege_envelope: str = supervisor_module.PROFILE_ENVELOPES["build"],
    ) -> dict[str, object]:
        candidate = self.candidate_value(candidate_id=candidate_id)
        return {
            "schema": supervisor_module.BUILD_SCHEMA,
            "appliance_id": self.config.appliance_id,
            "build_id": build_id,
            "candidate_id": candidate_id,
            "candidate_sha256": supervisor_module.sha256_bytes(
                supervisor_module.canonical_bytes(
                    supervisor_module.Candidate.parse(candidate).payload()
                )
            ),
            "base_generation": candidate["base_generation"],
            "generation_id": generation_id,
            "source_revision": "abcdef1",
            "bundle_sha256": "c" * 64,
            "tests_sha256": "d" * 64,
            "target": "x86_64-unknown-linux-gnu",
            "created_at": 995,
            "privilege_envelope": privilege_envelope,
        }

    def provision_introspection_evidence(
        self, build: dict[str, object]
    ) -> tuple[Path, Path]:
        root = self.state_root / "introspection-evidence"
        build_root = root / "build-evidence"
        diff_root = root / "generation-diffs"
        for directory in (root, build_root, diff_root):
            directory.mkdir(exist_ok=True)
            directory.chmod(0o2750)
        lifecycle = {
            "status": "installed_pending_stage_verification",
            "events": [
                {
                    "phase": "generation_installed",
                    "recorded_at": 998,
                    "authority": "immutable_root_rescue_helper",
                }
            ],
        }
        common = {
            "appliance_id": "avado-test",
            "generated_at": 998,
            "build_id": build["build_id"],
            "candidate_id": build["candidate_id"],
            "candidate_sha256": build["candidate_sha256"],
            "generation_id": build["generation_id"],
            "base_generation": build["base_generation"],
            "source_id": "cpu-edge:" + "1" * 64,
            "lifecycle": lifecycle,
            "provenance": "immutable_machine_evidence_not_astrid_authorship",
            "projection_sha256": "",
        }
        build_view = {
            "schema": "astrid.edge_self_change.build_evidence_view.v1",
            **common,
            "source_revision": build["source_revision"],
            "target": build["target"],
            "bundle_sha256": build["bundle_sha256"],
            "tests_sha256": build["tests_sha256"],
            "privilege_envelope": build["privilege_envelope"],
            "gates": [
                {
                    "label": "workspace-tests",
                    "executable_sha256": "2" * 64,
                    "argv_sha256": "3" * 64,
                    "exit_code": 0,
                    "timed_out": False,
                    "duration_ms": 42,
                }
            ],
            "invariants": {
                "candidate_replay_sha256": "4" * 64,
                "package_replay_sha256": "5" * 64,
                "immutable_invariants": True,
                "offline_locked": True,
                "network_policy": "private-network-none:v1",
            },
        }
        diff_view = {
            "schema": "astrid.edge_self_change.generation_diff_view.v1",
            **common,
            "parent_source_id": "cpu-edge:" + "0" * 64,
            "files": [
                {
                    "path": "services/astrid-edge-runtime/src/feature.rs",
                    "source_sha256": "6" * 64,
                    "content_sha256": "7" * 64,
                    "changed_lines": 2,
                }
            ],
            "total_changed_lines": 2,
            "truncated": False,
        }
        build_path = build_root / f"{build['build_id']}.json"
        diff_path = diff_root / f"{build['generation_id']}.json"
        projection_module._write_projection(build_path, build_view, self.state_root)
        projection_module._write_projection(diff_path, diff_view, self.state_root)
        return build_path, diff_path

    def intent_value(
        self,
        candidate: dict[str, object] | None = None,
        intent_id: str = "intent-1",
        **overrides: object,
    ) -> dict[str, object]:
        candidate = candidate or self.candidate_value()
        value: dict[str, object] = {
            "schema": supervisor_module.INTENT_SCHEMA,
            "intent_id": intent_id,
            "appliance_id": "avado-test",
            "trace_id": "trace-1",
            "session_id": "session-1",
            "turn_id": "turn-1",
            "response_sha256": "e" * 64,
            "terminal_declaration_sha256": "f" * 64,
            "candidate_id": candidate["candidate_id"],
            "candidate_sha256": supervisor_module.sha256_bytes(
                supervisor_module.canonical_bytes(
                    supervisor_module.Candidate.parse(candidate).payload()
                )
            ),
            "base_generation": candidate["base_generation"],
            "current_generation": "g0",
            "observed_at": self.supervisor.now - 1,
            "origin": "scheduled_autonomy",
            "authorship_status": "genuinely_authored",
            "transport_status": "authored_completed",
            "declaration_provenance": "exact_terminal_model_declaration",
            "fallback": False,
            "executor_repair": False,
            "operator_harness": False,
        }
        value.update(overrides)
        return value

    def make_bound_generation(self, build_value: dict[str, object]) -> Path:
        path = self.releases_root / str(build_value["generation_id"])
        path.mkdir(mode=0o755)
        manifest = {
            "schema": supervisor_module.GENERATION_SCHEMA,
            "appliance_id": build_value["appliance_id"],
            "generation_id": build_value["generation_id"],
            "build_id": build_value["build_id"],
            "candidate_id": build_value["candidate_id"],
            "candidate_sha256": build_value["candidate_sha256"],
            "base_generation": build_value["base_generation"],
            "bundle_sha256": build_value["bundle_sha256"],
            "tests_sha256": build_value["tests_sha256"],
            "target": build_value["target"],
        }
        manifest_path = path / ".astrid-edge-generation.json"
        manifest_path.write_text(json.dumps(manifest))
        manifest_path.chmod(0o444)
        binary = path / "astrid-edge-runtime"
        binary.write_bytes(b"immutable fixture")
        binary.chmod(0o555)
        path.chmod(0o555)
        return path

    def signed_envelope(
        self,
        candidate: dict[str, object] | None = None,
        intent: dict[str, object] | None = None,
        *,
        envelope_id: str = "envelope-1",
        signing_key: Path | None = None,
        created_at: int | None = None,
    ) -> dict[str, object]:
        signer = supervisor_module.Signer(signing_key or self.intent_key_path)
        intent_envelope = self.bare_signed_intent_envelope(
            candidate,
            intent,
            envelope_id=envelope_id,
            signing_key=signing_key,
            created_at=created_at,
        )
        intent_core = intent_envelope["core"]
        assert isinstance(intent_core, dict)
        intent_value = intent_core["intent"]
        candidate_value = intent_core["candidate"]
        assert isinstance(intent_value, dict)
        assert isinstance(candidate_value, dict)
        publication = {
            "intent_envelope_id": envelope_id,
            "intent_envelope_sha256": supervisor_module.sha256_bytes(
                supervisor_module.canonical_bytes(intent_envelope)
            ),
            "intent_id": intent_value["intent_id"],
            "terminal_declaration_sha256": intent_value[
                "terminal_declaration_sha256"
            ],
            "candidate_id": candidate_value["candidate_id"],
            "candidate_sha256": intent_core["candidate_sha256"],
            "base_generation": candidate_value["base_generation"],
        }
        completion_core = {
            "schema": supervisor_module.AUTHORED_COMPLETION_SCHEMA,
            "appliance_id": intent_value["appliance_id"],
            "due_nonce": f"due-{envelope_id}",
            "trace_id": intent_value["trace_id"],
            "session_id": intent_value["session_id"],
            "turn_id": intent_value["turn_id"],
            "response_sha256": intent_value["response_sha256"],
            "transaction_sha256": "9" * 64,
            "completed_at_unix_ms": self.supervisor.now * 1_000,
            "candidate_publication": publication,
            "status": "authored_completed",
            "provenance": "model_authored_runtime_scheduled",
        }
        completion_bytes = supervisor_module.canonical_bytes(completion_core)
        authored_completion = {
            "schema": supervisor_module.AUTHORED_COMPLETION_ENVELOPE_SCHEMA,
            "core": completion_core,
            "core_sha256": supervisor_module.sha256_bytes(completion_bytes),
            "auth": {
                "algorithm": "hmac-sha256",
                "key_id": signer.key_id,
                "signature": signer.sign(completion_bytes),
            },
        }
        unsigned = {
            "schema": supervisor_module.COMPLETED_INTENT_ENVELOPE_SCHEMA,
            "intent_envelope": intent_envelope,
            "authored_completion": authored_completion,
        }
        return {
            **unsigned,
            "auth": {
                "algorithm": "hmac-sha256",
                "key_id": signer.key_id,
                "signature": signer.sign(supervisor_module.canonical_bytes(unsigned)),
            },
        }

    def bare_signed_intent_envelope(
        self,
        candidate: dict[str, object] | None = None,
        intent: dict[str, object] | None = None,
        *,
        envelope_id: str = "envelope-1",
        signing_key: Path | None = None,
        created_at: int | None = None,
    ) -> dict[str, object]:
        candidate = candidate or self.candidate_value()
        candidate_payload = supervisor_module.Candidate.parse(candidate).payload()
        intent = intent or self.intent_value(candidate_payload)
        core = {
            "envelope_id": envelope_id,
            "created_at": self.supervisor.now - 1 if created_at is None else created_at,
            "candidate_sha256": supervisor_module.sha256_bytes(
                supervisor_module.canonical_bytes(candidate_payload)
            ),
            "candidate": candidate_payload,
            "intent": intent,
        }
        signer = supervisor_module.Signer(signing_key or self.intent_key_path)
        signed = supervisor_module.canonical_bytes(
            {"schema": supervisor_module.INTENT_ENVELOPE_SCHEMA, "core": core}
        )
        return {
            "schema": supervisor_module.INTENT_ENVELOPE_SCHEMA,
            "core": core,
            "auth": {
                "algorithm": "hmac-sha256",
                "key_id": signer.key_id,
                "signature": signer.sign(signed),
            },
        }

    def resign_completed_envelope(
        self,
        envelope: dict[str, object],
        signing_key: Path | None = None,
    ) -> None:
        signer = supervisor_module.Signer(signing_key or self.intent_key_path)
        completion = envelope["authored_completion"]
        assert isinstance(completion, dict)
        completion_core = completion["core"]
        assert isinstance(completion_core, dict)
        core_bytes = supervisor_module.canonical_bytes(completion_core)
        completion["core_sha256"] = supervisor_module.sha256_bytes(core_bytes)
        completion["auth"] = {
            "algorithm": "hmac-sha256",
            "key_id": signer.key_id,
            "signature": signer.sign(core_bytes),
        }
        unsigned = {
            "schema": envelope["schema"],
            "intent_envelope": envelope["intent_envelope"],
            "authored_completion": completion,
        }
        envelope["auth"] = {
            "algorithm": "hmac-sha256",
            "key_id": signer.key_id,
            "signature": signer.sign(supervisor_module.canonical_bytes(unsigned)),
        }

    def prepare_staged_build(self) -> dict[str, object]:
        candidate = self.candidate_value()
        build = self.build_value()
        self.supervisor.record_candidate(candidate, execute=True)
        self.supervisor.record_scheduled_intent(
            self.signed_envelope(candidate, self.intent_value(candidate)), execute=True
        )
        self.supervisor.record_build(build, execute=True)
        self.make_bound_generation(build)
        self.provision_introspection_evidence(build)
        self.supervisor.stage("build-1", execute=True)
        return build

    def replace_state(self, **updates: object) -> None:
        state = self.supervisor.read_state()
        state.update(updates)
        self.supervisor.write_state(state)


class BootstrapPauseTests(SupervisorFixture):
    def pause_for_bootstrap(self) -> None:
        self.replace_state(
            mode="paused", paused_reason="bootstrap_acceptance_pending"
        )

    def write_inbox_envelope(self, envelope: dict[str, object]) -> Path:
        inbox = self.state_root / "inbox"
        inbox.mkdir(mode=0o700, exist_ok=True)
        path = inbox / "candidate-intent-envelope-1.json"
        path.write_bytes(supervisor_module.canonical_bytes(envelope) + b"\n")
        path.chmod(0o600)
        return path

    def test_production_initial_state_requires_explicit_acceptance_resume(self) -> None:
        state = supervisor_module.Supervisor(self.config, now=2_000).initial_state()
        self.assertEqual(state["mode"], "paused")
        self.assertEqual(state["paused_reason"], "bootstrap_acceptance_pending")
        self.assertEqual(state["due"]["reasons"], ["bootstrap"])

    def test_rescue_mode_requires_a_distinct_explicit_acknowledgement(self) -> None:
        self.replace_state(mode="rescue", paused_reason="ambiguous_crash_state")

        with self.assertRaisesRegex(supervisor_module.SupervisorError, "acknowledgement"):
            self.supervisor.set_mode(
                "running", "operator_review_complete", execute=True
            )

        unchanged = self.supervisor.read_state()
        self.assertEqual(unchanged["mode"], "rescue")
        resumed = self.supervisor.set_mode(
            "running",
            "operator_review_complete",
            execute=True,
            acknowledge_rescue=True,
        )
        self.assertEqual(resumed["operation"], "running")
        current = self.supervisor.read_state()
        self.assertEqual(current["mode"], "running")
        self.assertIsNone(current["paused_reason"])

    def test_paused_supervise_leaves_genuine_envelope_byte_exact_and_uningested(self) -> None:
        self.pause_for_bootstrap()
        path = self.write_inbox_envelope(self.signed_envelope())
        ready = path.parent / "candidate-ready-envelope-1.json"
        ready.write_text("non-authorizing-wakeup\n")
        ready.chmod(0o600)
        before = path.read_bytes()

        result = self.supervisor.supervise(execute=True)

        self.assertEqual(result["recovery"]["recovery"], "clean")
        for phase in ("inbox", "build", "activation"):
            self.assertEqual(result[phase]["status"], "paused_queued_untouched")
        self.assertEqual(result["inbox"]["discarded_handoff_triggers"], 1)
        self.assertEqual(path.read_bytes(), before)
        self.assertFalse(ready.exists())
        self.assertEqual(self.supervisor.candidates(), {})
        self.assertEqual(self.supervisor.scheduled_intents(), {})

    def test_direct_pipeline_mutations_fail_closed_while_paused(self) -> None:
        candidate = self.candidate_value()
        build = self.build_value()
        self.pause_for_bootstrap()
        with self.assertRaisesRegex(supervisor_module.SupervisorError, "blocked"):
            self.supervisor.record_candidate(candidate, execute=True)
        with self.assertRaisesRegex(supervisor_module.SupervisorError, "blocked"):
            self.supervisor.record_scheduled_intent(
                self.signed_envelope(candidate, self.intent_value(candidate)),
                execute=True,
            )

        self.replace_state(mode="running", paused_reason=None)
        self.supervisor.record_candidate(candidate, execute=True)
        self.supervisor.record_build(build, execute=True)
        self.make_bound_generation(build)
        self.provision_introspection_evidence(build)
        self.pause_for_bootstrap()
        with self.assertRaisesRegex(supervisor_module.SupervisorError, "blocked"):
            self.supervisor.stage("build-1", execute=True)

    def test_operator_synthetic_harness_remains_available_during_pause(self) -> None:
        self.pause_for_bootstrap()
        self.supervisor.request_synthetic(execute=True)

        result = self.supervisor.supervise(execute=True)

        self.assertEqual(result["synthetic"]["status"], "completed")
        state = self.supervisor.read_state()
        self.assertEqual(state["mode"], "paused")
        self.assertEqual(state["paused_reason"], "bootstrap_acceptance_pending")
        self.assertIsNone(state["synthetic_harness"])

    def test_paused_steward_queues_exact_attested_result_until_resume(self) -> None:
        envelope = self.signed_envelope()
        envelope_source = self.root / "config" / "intent-envelope.json"
        envelope_source.write_text(json.dumps(envelope))
        envelope_source.chmod(0o600)
        cp_path = Path(shutil.which("cp") or "/bin/cp").resolve()
        self.configure_profile(
            "model", cp_path, [str(envelope_source), "{intent_envelope}"]
        )
        self.pause_for_bootstrap()

        queued = self.supervisor.steward(execute=True)

        self.assertEqual(queued["status"], "attested_result_queued")
        self.assertIsNone(self.supervisor.read_state()["due"])
        self.assertEqual(self.supervisor.scheduled_intents(), {})
        inbox_path = self.state_root / "inbox" / "candidate-intent-envelope-1.json"
        self.assertTrue(inbox_path.exists())

        self.supervisor.set_mode(
            "running", "bootstrap_acceptance_complete", execute=True
        )
        resumed = self.supervisor.supervise(execute=True)
        self.assertEqual(resumed["inbox"]["status"], "accepted")
        self.assertFalse(inbox_path.exists())
        self.assertIn("intent-1", self.supervisor.scheduled_intents())

    def test_bootstrap_queue_survives_one_hour_but_not_more_than_one_day(self) -> None:
        self.pause_for_bootstrap()
        envelope = self.signed_envelope()
        path = self.write_inbox_envelope(envelope)
        before = path.read_bytes()
        one_hour = supervisor_module.Supervisor(self.config, now=4_600)
        one_hour.supervise(execute=True)
        self.assertEqual(path.read_bytes(), before)
        one_hour.set_mode("running", "bootstrap_acceptance_complete", execute=True)
        accepted = one_hour.pipeline.ingest_one(execute=True)
        self.assertEqual(accepted["status"], "accepted")

        second_candidate = self.candidate_value(candidate_id="candidate-stale")
        second_intent = self.intent_value(
            second_candidate,
            intent_id="intent-stale",
            trace_id="trace-stale",
            session_id="session-stale",
            turn_id="turn-stale",
            observed_at=self.supervisor.now - 1,
        )
        stale = self.signed_envelope(
            second_candidate,
            second_intent,
            envelope_id="envelope-stale",
            created_at=self.supervisor.now - 1,
        )
        expired = supervisor_module.Supervisor(
            self.config,
            now=self.supervisor.now
            + supervisor_module.INTENT_INGEST_MAX_AGE_SECONDS
            + 1,
        )
        with self.assertRaisesRegex(
            supervisor_module.IntegrityError, "not fresh"
        ):
            expired.record_scheduled_intent(stale, execute=False)


class StewardProjectionTests(SupervisorFixture):
    def test_narrow_projection_tracks_exact_candidate_lifecycle(self) -> None:
        empty = supervisor_module.steward_status(self.supervisor)
        self.assertEqual(empty["schema"], "astrid.edge_self_change.steward_status.v1")
        self.assertEqual(empty["supervisor_mode"], "running")
        self.assertIsNone(empty["candidate"])
        self.assertFalse(empty["pipeline_busy"])

        candidate = self.candidate_value()
        intent = self.intent_value(candidate)
        self.supervisor.record_scheduled_intent(
            self.signed_envelope(candidate, intent), execute=True
        )
        pending = supervisor_module.steward_status(self.supervisor)
        expected_digest = supervisor_module.sha256_bytes(
            supervisor_module.canonical_bytes(
                supervisor_module.Candidate.parse(candidate).payload()
            )
        )
        self.assertEqual(
            pending["candidate"],
            {
                "candidate_id": "candidate-1",
                "candidate_sha256": expected_digest,
                "status": "intent_pending",
            },
        )
        self.assertTrue(pending["pipeline_busy"])

        self.supervisor.ledger("build").append(
            "build_profile_started",
            {"candidate_id": "candidate-1", "intent_id": "intent-1"},
            "build-started-intent-1",
            self.supervisor.now,
        )
        building = supervisor_module.steward_status(self.supervisor)
        self.assertEqual(building["candidate"]["status"], "building")

        self.supervisor.ledger("build").append(
            "build_profile_failed",
            {"candidate_id": "candidate-1", "intent_id": "intent-1"},
            "build-failed-intent-1",
            self.supervisor.now,
        )
        rejected = supervisor_module.steward_status(self.supervisor)
        self.assertEqual(rejected["candidate"]["status"], "rejected")
        self.assertFalse(rejected["pipeline_busy"])

        self.replace_state(mode="paused", paused_reason="operator_requested")
        paused = supervisor_module.steward_status(self.supervisor)
        self.assertEqual(paused["supervisor_mode"], "paused")
        # Pause gates ingestion/build/activation, but does not masquerade as a
        # candidate transaction or suppress a due scheduled reflection.
        self.assertFalse(paused["pipeline_busy"])
        self.replace_state(mode="running", paused_reason=None)

        self.supervisor.pipeline.project_status({"operation": "test"})
        persisted = json.loads(
            (self.state_root / "steward-status.json").read_text(encoding="utf-8")
        )
        self.assertEqual(persisted, rejected)
        operator = json.loads(self.operator_status_path.read_text(encoding="utf-8"))
        self.assertEqual(
            operator["schema"],
            "astrid.edge_self_change.operator_status_envelope.v1",
        )
        self.assertEqual(
            operator["core"]["schema"],
            "astrid.edge_self_change.operator_status.v3",
        )
        self.assertEqual(
            operator["core"]["provenance"],
            "immutable_supervisor_sanitized_projection",
        )
        self.assertEqual(operator["core"]["pipeline_phase"], "due")
        lifecycle = operator["core"]["lifecycle"]
        self.assertEqual(
            lifecycle["schema"],
            "astrid.edge_self_change.operator_lifecycle.v1",
        )
        self.assertEqual(lifecycle["included"], 4)
        self.assertFalse(lifecycle["truncated"])
        self.assertEqual(
            {event["status"] for event in lifecycle["events"]},
            {
                "scheduled_intent_attested",
                "candidate_recorded",
                "build_profile_started",
                "build_profile_failed",
            },
        )
        self.assertTrue(
            all(
                event["provenance"]
                == "immutable_supervisor_signed_ledger_sanitized_metadata"
                and event["authored"] is False
                and event["fallback"] is False
                for event in lifecycle["events"]
            )
        )
        self.assertEqual(
            operator["core"]["restart_expectation"],
            {
                "phase": "none",
                "maximum_seconds": 0,
                "basis": "immutable_command_profile_timeout_upper_bound",
            },
        )
        self.assertNotIn("ledgers", operator["core"])
        self.assertNotIn("intent_attestor", operator["core"])
        self.assertEqual(stat.S_IMODE(self.operator_status_path.stat().st_mode), 0o640)

        self.replace_state(
            due=None,
            inflight={"phase": "profile_invoked"},
        )
        self.supervisor.pipeline.project_status({"operation": "activate"})
        restarting = json.loads(
            self.operator_status_path.read_text(encoding="utf-8")
        )
        self.assertEqual(
            restarting["core"]["restart_expectation"],
            {
                "phase": "activation",
                "maximum_seconds": 5,
                "basis": "immutable_command_profile_timeout_upper_bound",
            },
        )

    def test_operator_projection_is_bounded_body_free_and_target_safe(self) -> None:
        secret = {
            "prompt": "SECRET_PROMPT_BODY",
            "response": "SECRET_RESPONSE_BODY",
            "diff": "SECRET_DIFF_BODY",
            "build_log": "SECRET_BUILD_LOG",
            "private_key": "SECRET_KEY_BODY",
            "fallback_text": "SECRET_FALLBACK_BODY",
        }
        for index in range(70):
            self.supervisor.ledger("operator").append(
                "operator_observation",
                {"automatic": False, "index": index, **secret},
                f"operator-observation-{index}",
                2_000 + index,
            )
        envelope = projection_module.operator_status(
            self.supervisor, {"operation": "supervise", "status": "completed"}
        )
        encoded = supervisor_module.canonical_bytes(envelope)
        lifecycle = envelope["core"]["lifecycle"]
        self.assertEqual(lifecycle["included"], 64)
        self.assertEqual(lifecycle["total"], 70)
        self.assertTrue(lifecycle["truncated"])
        self.assertLessEqual(
            len(encoded), projection_module.MAX_OPERATOR_PROJECTION_BYTES
        )
        self.assertNotIn(b"SECRET_", encoded)
        self.assertTrue(
            all(event["record_sha256"] for event in lifecycle["events"])
        )

        projection_module.write_operator_status(self.operator_status_path, envelope)
        self.assertEqual(stat.S_IMODE(self.operator_status_path.stat().st_mode), 0o640)
        target = self.operator_status_root / "unsafe-target.json"
        target.symlink_to(self.operator_status_path)
        with self.assertRaisesRegex(RuntimeError, "target is unsafe"):
            projection_module.write_operator_status(target, envelope)

        self.operator_status_root.chmod(0o2770)
        try:
            with self.assertRaisesRegex(RuntimeError, "directory"):
                projection_module.write_operator_status(
                    self.operator_status_path, envelope
                )
        finally:
            self.operator_status_root.chmod(0o2750)


class IntrospectionEvidenceProjectionTests(SupervisorFixture):
    def recorded_build_with_projection(self) -> tuple[dict[str, object], Path, Path]:
        candidate = self.candidate_value()
        build = self.build_value()
        self.supervisor.record_candidate(candidate, execute=True)
        self.supervisor.record_build(build, execute=True)
        build_path, diff_path = self.provision_introspection_evidence(build)
        return build, build_path, diff_path

    def test_authenticated_lifecycle_is_projected_without_logs_or_source_bodies(self) -> None:
        build, build_path, diff_path = self.recorded_build_with_projection()
        self.supervisor.ledger("build").append(
            "build_profile_completed",
            {
                "candidate_id": build["candidate_id"],
                "build_id": build["build_id"],
            },
            "projection-build-completed",
            1_001,
        )
        self.supervisor.ledger("build").append(
            "stage_verified",
            {
                "build_id": build["build_id"],
                "generation_id": build["generation_id"],
            },
            "projection-stage-verified",
            1_002,
        )
        self.supervisor.ledger("activation").append(
            "probation_started",
            {
                "build_id": build["build_id"],
                "to_generation": build["generation_id"],
            },
            "projection-probation-started",
            1_003,
        )
        summary = projection_module.refresh_introspection_evidence(self.supervisor)
        self.assertEqual(summary["status"], "valid")
        self.assertEqual(
            summary["retention"],
            "metadata_retained_for_hindsight_after_generation_pruning",
        )

        build_view = projection_module._read_projection(
            build_path,
            self.state_root,
            "astrid.edge_self_change.build_evidence_view.v1",
        )
        diff_view = projection_module._read_projection(
            diff_path,
            self.state_root,
            "astrid.edge_self_change.generation_diff_view.v1",
        )
        self.assertEqual(build_view["lifecycle"]["status"], "probation")
        self.assertEqual(diff_view["lifecycle"], build_view["lifecycle"])
        self.assertNotIn("stdout", json.dumps(build_view))
        self.assertNotIn("content", diff_view["files"][0])

        self.supervisor.ledger("activation").append(
            "probation_accepted",
            {
                "build_id": build["build_id"],
                "generation_id": build["generation_id"],
            },
            "projection-probation-accepted",
            1_004,
        )
        projection_module.refresh_introspection_evidence(self.supervisor)
        accepted = projection_module._read_projection(
            build_path,
            self.state_root,
            "astrid.edge_self_change.build_evidence_view.v1",
        )
        self.assertEqual(accepted["lifecycle"]["status"], "accepted")

    def test_projection_tamper_fails_closed_before_lifecycle_rewrite(self) -> None:
        _, build_path, _ = self.recorded_build_with_projection()
        value = json.loads(build_path.read_bytes())
        value["lifecycle"]["status"] = "accepted"
        build_path.chmod(0o600)
        build_path.write_bytes(supervisor_module.canonical_bytes(value))
        build_path.chmod(0o440)
        with self.assertRaisesRegex(RuntimeError, "self-hash"):
            projection_module.refresh_introspection_evidence(self.supervisor)

    def test_retention_pruning_delegates_to_immutable_paired_gc(self) -> None:
        build, build_path, diff_path = self.recorded_build_with_projection()
        for name in ("g1", "g2", "g3", "g4"):
            path = self.make_unbound_generation(name)
            os.utime(path, (1, 1))
        later = supervisor_module.Supervisor(
            self.config,
            now=supervisor_module.RETENTION_SECONDS + 2_000,
        )
        result = later.prune(execute=True)
        self.assertIn(build["generation_id"], result["eligible"])
        self.assertEqual(result["removed"], [])
        self.assertEqual(result["status"], "healthy_nothing_eligible")
        self.assertEqual(result["command"]["profile"], "retention")
        self.assertTrue((self.releases_root / str(build["generation_id"])).exists())
        self.assertTrue(build_path.exists())
        self.assertTrue(diff_path.exists())
        self.assertEqual(
            later.ledger("operator").read()[-1]["core"]["kind"],
            "paired_retention_completed",
        )


class PathBoundaryTests(SupervisorFixture):
    def test_candidate_rejects_traversal_absolute_and_immutable_roots(self) -> None:
        for path in (
            "../escape.rs",
            "/etc/shadow",
            "packaging/systemd/astrid-edge-self-change-supervisor.service",
            "packaging/systemd/astrid-edge-self-change-probation-health.service",
            "packaging/systemd/astrid-edge-self-change-probation-health.timer",
            "packaging/systemd/astrid-edge-generation-guard.service",
            "packaging/systemd/astrid-edge-core-liveness.service",
            "packaging/systemd/astrid-edge-web-broker-runtime.service",
            "packaging/systemd/astrid-edge-web-broker-steward.service",
            "packaging/systemd/root/astrid-edge-updater@.service",
            "scripts/edge_self_change_supervisor.py",
            "scripts/edge_self_change/model.py",
            "services/astrid-edge-steward-helper/src/main.rs",
            "services/astrid-edge-rescue-helper/src/main.rs",
            "services/astrid-edge-web-broker/src/main.rs",
            "services/astrid-edge-checkpoint/src/main.rs",
            "README.md",
        ):
            with self.subTest(path=path), self.assertRaises(supervisor_module.SupervisorError):
                supervisor_module.Candidate.parse(self.candidate_value(changed_paths=[path]))

        runtime_integration = "services/astrid-edge-runtime/src/self_change/state.rs"
        parsed = supervisor_module.Candidate.parse(
            self.candidate_value(changed_paths=[runtime_integration])
        )
        self.assertEqual(parsed.changed_paths, (runtime_integration,))

    def test_all_twenty_appliance_capsules_are_inside_the_mutable_signed_surface(self) -> None:
        expected = {
            "astrid-capsule-agents",
            "astrid-capsule-cli",
            "astrid-capsule-edge-context",
            "astrid-capsule-edge-introspector",
            "astrid-capsule-edge-spectral",
            "astrid-capsule-fs",
            "astrid-capsule-http",
            "astrid-capsule-memory",
            "astrid-capsule-shell",
            "astrid-capsule-skills",
            "astrid-capsule-context-engine",
            "astrid-capsule-hook-bridge",
            "astrid-capsule-identity",
            "astrid-capsule-openai-compat",
            "astrid-capsule-prompt-builder",
            "astrid-capsule-react",
            "astrid-capsule-registry",
            "astrid-capsule-router",
            "astrid-capsule-session",
            "astrid-capsule-system",
        }
        self.assertEqual(model_module.EDGE_CAPSULES, frozenset(expected))
        for capsule in expected:
            path = f"capsules/astralis/{capsule}/src/lib.rs"
            with self.subTest(capsule=capsule):
                parsed = supervisor_module.Candidate.parse(
                    self.candidate_value(changed_paths=[path])
                )
                self.assertEqual(parsed.changed_paths, (path,))

        skill_path = (
            "capsules/astralis/astrid-capsule-system/"
            "src/skills/capsule-development/SKILL.md"
        )
        parsed = supervisor_module.Candidate.parse(
            self.candidate_value(changed_paths=[skill_path])
        )
        self.assertEqual(parsed.changed_paths, (skill_path,))
        with self.assertRaises(supervisor_module.SupervisorError):
            supervisor_module.Candidate.parse(
                self.candidate_value(
                    changed_paths=["capsules/astralis/astrid-capsule-system/src/payload.wasm"]
                )
            )

    def test_daemon_and_edge_runtime_authority_modules_remain_mutable(self) -> None:
        for path in (
            "crates/astrid-events/src/bus.rs",
            "crates/astrid-kernel/src/maintenance.rs",
            "crates/astrid-kernel/src/socket_bridge.rs",
            "services/astrid-edge-runtime/src/config.rs",
            "services/astrid-edge-runtime/src/ipc.rs",
            "services/astrid-edge-runtime/src/maintenance.rs",
        ):
            with self.subTest(path=path):
                parsed = supervisor_module.Candidate.parse(
                    self.candidate_value(changed_paths=[path])
                )
                self.assertEqual(parsed.changed_paths, (path,))

    def test_candidate_file_ceiling_is_exactly_twenty_five(self) -> None:
        accepted = [f"crates/astrid-core/src/generated_{index}.rs" for index in range(25)]
        self.assertEqual(
            len(
                supervisor_module.Candidate.parse(
                    self.candidate_value(changed_paths=accepted)
                ).changed_paths
            ),
            25,
        )
        with self.assertRaisesRegex(supervisor_module.SupervisorError, "1..25"):
            supervisor_module.Candidate.parse(
                self.candidate_value(
                    changed_paths=[*accepted, "crates/astrid-core/src/overflow.rs"]
                )
            )

    def test_signed_cpu_edge_surface_keeps_required_sources_mutable(self) -> None:
        paths = [
            "Cargo.toml",
            "Cargo.lock",
            "crates/astrid-approval/src/lib.rs",
            "crates/astrid-kernel/tests/policy.rs",
            "services/astrid-edge-runtime/src/autonomy.rs",
            "capsules/astralis/astrid-capsule-edge-context/src/lib.rs",
            "packaging/systemd/astrid-edge-runtime.service",
            "packaging/systemd/icp/astrid-edge-runtime.service",
        ]
        candidate = supervisor_module.Candidate.parse(
            self.candidate_value(changed_paths=paths)
        )
        self.assertEqual(list(candidate.changed_paths), sorted(paths))

    def test_only_six_base_systemd_fragments_enter_model_authored_surface(self) -> None:
        for name in supervisor_module.MUTABLE_UNIT_FRAGMENTS:
            for prefix in ("packaging/systemd", "packaging/systemd/icp"):
                path = f"{prefix}/{name}"
                with self.subTest(path=path):
                    parsed = supervisor_module.Candidate.parse(
                        self.candidate_value(changed_paths=[path])
                    )
                    self.assertEqual(parsed.changed_paths, (path,))

        for path in (
            "packaging/systemd/astrid-edge-generation-guard.service",
            "packaging/systemd/astrid-edge-steward.service",
            "packaging/systemd/astrid-edge-web-broker-runtime.service",
            "packaging/systemd/astrid-edge-self-change-authority.conf",
        ):
            with self.subTest(path=path), self.assertRaises(
                supervisor_module.SupervisorError
            ):
                supervisor_module.Candidate.parse(
                    self.candidate_value(changed_paths=[path])
                )

    def test_bounded_path_rejects_symlink_escape(self) -> None:
        outside = self.root / "outside"
        outside.mkdir()
        link = self.state_root / "linked"
        os.symlink(outside, link)
        with self.assertRaises(supervisor_module.SupervisorError):
            supervisor_module.validate_bounded_path(self.state_root, link / "payload", require_exists=False)

    def test_bounded_path_walks_each_nested_component_exactly_once(self) -> None:
        nested = self.state_root / "one" / "two" / "payload.json"
        nested.parent.mkdir(parents=True)
        nested.write_text("{}")
        self.assertEqual(
            supervisor_module.validate_bounded_path(self.state_root, nested), nested
        )

    def test_generation_rejects_embedded_symlink_and_mutable_file(self) -> None:
        build = self.build_value()
        generation = self.make_bound_generation(build)
        generation.chmod(0o755)
        os.symlink(self.root / "outside", generation / "escape")
        generation.chmod(0o555)
        parsed = supervisor_module.Build.parse(
            build, self.config.target, self.config.appliance_id
        )
        with self.assertRaises(supervisor_module.SupervisorError):
            self.supervisor.validate_generation(parsed)
        generation.chmod(0o755)
        (generation / "escape").unlink()
        mutable = generation / "astrid-edge-runtime"
        mutable.chmod(0o755)
        generation.chmod(0o555)
        with self.assertRaises(supervisor_module.SupervisorError):
            self.supervisor.validate_generation(parsed)

    def test_config_rejects_root_of_trust_inside_mutable_state(self) -> None:
        config_path = self.root / "bad-config.json"
        value = {
            "schema": supervisor_module.CONFIG_SCHEMA,
            "state_root": str(self.state_root),
            "releases_root": str(self.releases_root),
            "active_link": str(self.active_link),
            "signing_key": str(self.state_root / "attacker-replaceable.key"),
            "intent_attestation_key": str(self.intent_key_path),
            "command_profiles": str(self.profile_path),
            "operator_status": str(self.operator_status_path),
            "model_handoff_root": str(self.model_handoff_root),
            "appliance_id": "avado-test",
            "target": "x86_64-unknown-linux-gnu",
        }
        config_path.write_text(json.dumps(value))
        config_path.chmod(0o600)
        with self.assertRaisesRegex(supervisor_module.SupervisorError, "root-of-trust"):
            supervisor_module.Config.from_file(config_path)


class ReplayAndIntegrityTests(SupervisorFixture):
    def test_build_and_signed_state_are_bound_to_the_exact_appliance(self) -> None:
        self.supervisor.record_candidate(self.candidate_value(), execute=True)
        cross_box_build = self.build_value()
        cross_box_build["appliance_id"] = "icp-other-box"
        with self.assertRaisesRegex(
            supervisor_module.SupervisorError, "appliance identity"
        ):
            self.supervisor.record_build(cross_box_build, execute=True)
        self.assertEqual(self.supervisor.builds(), {})

        # Reusing the same supervisor key, target, state path, candidate IDs,
        # and generation IDs must not make another appliance's signed state
        # acceptable.
        other_config = dataclasses.replace(
            self.config, appliance_id="icp-other-box"
        )
        other = supervisor_module.Supervisor(other_config, now=self.supervisor.now)
        with self.assertRaisesRegex(
            supervisor_module.IntegrityError, "state payload is invalid"
        ):
            other.read_state()

    def test_candidate_and_ledger_event_replay_are_rejected(self) -> None:
        candidate = self.candidate_value()
        self.supervisor.record_candidate(candidate, execute=True)
        with self.assertRaises(supervisor_module.SupervisorError):
            self.supervisor.record_candidate(candidate, execute=True)
        ledger = self.supervisor.ledger("operator")
        ledger.append("mode_paused", {"reason": "one"}, "same-event", 1_001)
        with self.assertRaises(supervisor_module.IntegrityError):
            ledger.append("mode_paused", {"reason": "two"}, "same-event", 1_002)

    def test_scheduled_intent_replay_is_rejected(self) -> None:
        self.prepare_staged_build()
        self.supervisor.activate("build-1", "intent-1", execute=True)
        self.supervisor.rollback("operator_test", execute=True)
        self.replace_state(mode="running", paused_reason=None, previous_generation=None)
        with self.assertRaisesRegex(supervisor_module.SupervisorError, "replay"):
            self.supervisor.activate("build-1", "intent-1", execute=True)

    def test_tampered_ledger_and_state_fail_closed(self) -> None:
        self.supervisor.record_candidate(self.candidate_value(), execute=True)
        ledger_path = self.state_root / "ledgers" / "candidate.jsonl"
        data = bytearray(ledger_path.read_bytes())
        data[data.index(b"candidate-1")] = ord("X")
        ledger_path.chmod(0o600)
        ledger_path.write_bytes(data)
        with self.assertRaises(supervisor_module.IntegrityError):
            self.supervisor.ledger("candidate").read()
        state = json.loads(self.supervisor.state_path.read_text())
        state["state"]["mode"] = "rescue"
        self.supervisor.state_path.write_text(json.dumps(state))
        self.supervisor.state_path.chmod(0o600)
        with self.assertRaises(supervisor_module.IntegrityError):
            self.supervisor.read_state()

    def test_ledger_rejects_symbolic_and_hard_links(self) -> None:
        ledger = self.supervisor.ledger("operator")
        ledger_path = self.state_root / "ledgers" / "operator.jsonl"
        ledger_path.parent.mkdir(parents=True, exist_ok=True)
        outside = self.root / "outside-ledger"
        outside.write_bytes(b"")
        outside.chmod(0o600)
        ledger_path.symlink_to(outside)
        with self.assertRaises((OSError, supervisor_module.IntegrityError)):
            ledger.read()
        ledger_path.unlink()
        os.link(outside, ledger_path)
        with self.assertRaises(supervisor_module.IntegrityError):
            ledger.read()
        with self.assertRaises(supervisor_module.IntegrityError):
            ledger.append("mode_paused", {"reason": "linked"}, "linked-event", 1_001)

    def test_ledger_detects_path_replacement_during_descriptor_read(self) -> None:
        ledger = self.supervisor.ledger("operator")
        ledger.append("mode_paused", {"reason": "one"}, "event-one", 1_001)
        ledger_path = self.state_root / "ledgers" / "operator.jsonl"
        replacement = ledger_path.with_name("replacement-ledger")
        replacement.write_bytes(ledger_path.read_bytes())
        replacement.chmod(0o600)
        original_lstat = model_module.os.lstat
        matching_calls = 0

        def replace_on_second_lstat(path: os.PathLike[str] | str, *args: object, **kwargs: object):
            nonlocal matching_calls
            if Path(path) == ledger_path:
                matching_calls += 1
                if matching_calls == 2:
                    os.replace(replacement, ledger_path)
            return original_lstat(path, *args, **kwargs)

        with mock.patch.object(model_module.os, "lstat", side_effect=replace_on_second_lstat):
            with self.assertRaisesRegex(supervisor_module.IntegrityError, "replaced"):
                ledger.read()

    def test_ledger_detects_path_replacement_during_locked_append(self) -> None:
        ledger = self.supervisor.ledger("operator")
        ledger.append("mode_paused", {"reason": "one"}, "event-one", 1_001)
        ledger_path = self.state_root / "ledgers" / "operator.jsonl"
        original = ledger_path.read_bytes()
        replacement = ledger_path.with_name("replacement-during-append")
        replacement.write_bytes(original)
        replacement.chmod(0o600)
        original_lstat = model_module.os.lstat
        matching_calls = 0

        def replace_after_locked_write(
            path: os.PathLike[str] | str, *args: object, **kwargs: object
        ) -> os.stat_result:
            nonlocal matching_calls
            if Path(path) == ledger_path:
                matching_calls += 1
                if matching_calls == 2:
                    os.replace(replacement, ledger_path)
            return original_lstat(path, *args, **kwargs)

        with mock.patch.object(
            model_module.os, "lstat", side_effect=replace_after_locked_write
        ):
            with self.assertRaisesRegex(supervisor_module.IntegrityError, "replaced"):
                ledger.append("mode_paused", {"reason": "two"}, "event-two", 1_002)
        self.assertEqual(ledger_path.read_bytes(), original)

    def test_concurrent_ledger_appends_serialize_without_false_integrity_failures(self) -> None:
        ledger = self.supervisor.ledger("operator")

        def append(index: int) -> None:
            ledger.append(
                "mode_paused",
                {"reason": f"concurrent-{index}"},
                f"concurrent-event-{index}",
                1_100 + index,
            )

        with ThreadPoolExecutor(max_workers=8) as executor:
            list(executor.map(append, range(16)))
        records = ledger.read()
        self.assertEqual(len(records), 16)
        self.assertEqual(
            {record["core"]["event_id"] for record in records},
            {f"concurrent-event-{index}" for index in range(16)},
        )

    def test_ledger_rejects_an_oversized_record_before_persisting_it(self) -> None:
        ledger = self.supervisor.ledger("operator")
        with self.assertRaisesRegex(supervisor_module.IntegrityError, "line ceiling"):
            ledger.append(
                "mode_paused",
                {"reason": "x" * model_module.MAX_LEDGER_LINE_BYTES},
                "oversized-event",
                1_100,
            )
        self.assertEqual(ledger.read(), [])


class PrivilegeEnvelopeTests(SupervisorFixture):
    def test_candidate_and_build_cannot_request_elevated_authority(self) -> None:
        with self.assertRaisesRegex(supervisor_module.SupervisorError, "proposal-only"):
            supervisor_module.Candidate.parse(
                self.candidate_value(privilege_envelope="root:arbitrary-shell:v1")
            )
        with self.assertRaisesRegex(supervisor_module.SupervisorError, "offline"):
            supervisor_module.Build.parse(
                self.build_value(privilege_envelope="root-stager:release-root-only:v1"),
                self.config.target,
                self.config.appliance_id,
            )

    def test_profile_rejects_wrong_envelope_network_and_candidate_argv(self) -> None:
        value = json.loads(self.profile_path.read_text())
        value["profiles"]["model"]["privilege_envelope"] = "root:anything:v1"
        self.profile_path.write_text(json.dumps(value))
        self.profile_path.chmod(0o600)
        with self.assertRaises(supervisor_module.ProfileError):
            self.supervisor.profiles.load()
        self.write_profiles()
        value = json.loads(self.profile_path.read_text())
        value["profiles"]["model"]["network"] = "allow"
        value["profiles"]["model"]["candidate_argv"] = True
        self.profile_path.write_text(json.dumps(value))
        self.profile_path.chmod(0o600)
        with self.assertRaises(supervisor_module.ProfileError):
            self.supervisor.profiles.load()

    def test_build_has_a_long_immutable_timeout_but_synthetic_stays_bounded(self) -> None:
        value = json.loads(self.profile_path.read_text())
        value["profiles"]["build"]["timeout_seconds"] = 90_000
        self.profile_path.write_text(json.dumps(value))
        self.profile_path.chmod(0o600)
        self.assertEqual(self.supervisor.profiles.load()["build"].timeout_seconds, 90_000)

        value["profiles"]["build"]["timeout_seconds"] = 93_601
        self.profile_path.write_text(json.dumps(value))
        self.profile_path.chmod(0o600)
        with self.assertRaisesRegex(supervisor_module.ProfileError, "timeout"):
            self.supervisor.profiles.load()

        self.write_profiles()
        value = json.loads(self.profile_path.read_text())
        value["profiles"]["synthetic"]["timeout_seconds"] = 7_199
        self.profile_path.write_text(json.dumps(value))
        self.profile_path.chmod(0o600)
        with self.assertRaisesRegex(supervisor_module.ProfileError, "timeout"):
            self.supervisor.profiles.load()

    def test_profile_rejects_an_unavailable_or_root_model_identity(self) -> None:
        value = json.loads(self.profile_path.read_text())
        value["profiles"]["model"]["run_as_uid"] = 0 if os.geteuid() != 0 else 1
        value["profiles"]["model"]["run_as_gid"] = 0 if os.geteuid() != 0 else 1
        self.profile_path.write_text(json.dumps(value))
        self.profile_path.chmod(0o600)
        with self.assertRaises(supervisor_module.ProfileError):
            self.supervisor.profiles.load()

    def test_root_supervisor_requires_root_build_wrapper_but_nonroot_model(self) -> None:
        supervisor_module.validate_profile_identity("build", 0, 0, 0, 0)
        supervisor_module.validate_profile_identity("health", 0, 0, 0, 0)
        supervisor_module.validate_profile_identity("model", 1234, 1234, 0, 0)
        with self.assertRaisesRegex(supervisor_module.ProfileError, "root stager"):
            supervisor_module.validate_profile_identity("build", 1234, 1234, 0, 0)
        with self.assertRaisesRegex(supervisor_module.ProfileError, "drop"):
            supervisor_module.validate_profile_identity("model", 0, 0, 0, 0)

    def test_only_canonical_build_deferral_can_be_retried(self) -> None:
        value = {
            "schema": "astrid.edge_rescue_helper.result.v1",
            "status": "deferred_infrastructure",
            "reason": "thermal gate",
            "retry_authority": "immutable_supervisor_may_retry_after_condition_clears",
        }
        canonical = supervisor_module.canonical_bytes(value) + b"\n"
        self.assertEqual(
            supervisor_module.parse_deferred_build_result(canonical, b""),
            {"status": "deferred_infrastructure", "reason": "thermal gate"},
        )
        self.assertIsNone(
            supervisor_module.parse_deferred_build_result(
                json.dumps(value, indent=2).encode() + b"\n", b""
            )
        )
        self.assertIsNone(
            supervisor_module.parse_deferred_build_result(canonical, b"diagnostic")
        )

    def test_only_canonical_candidate_rejection_is_terminal(self) -> None:
        value = {
            "schema": "astrid.edge_rescue_helper.result.v1",
            "status": "candidate_rejected",
            "reason": "candidate profile violates immutable policy",
            "retry_authority": "identical_candidate_hash_never_retried_automatically",
        }
        canonical = supervisor_module.canonical_bytes(value) + b"\n"
        parsed = supervisor_module.parse_candidate_rejected_build_result(canonical, b"")
        self.assertEqual(
            parsed,
            {
                "status": "candidate_rejected",
                "reason_sha256": supervisor_module.sha256_bytes(
                    value["reason"].encode()
                ),
            },
        )
        self.assertIsNone(
            supervisor_module.parse_candidate_rejected_build_result(
                json.dumps(value, indent=2).encode() + b"\n", b""
            )
        )
        self.assertIsNone(
            supervisor_module.parse_candidate_rejected_build_result(
                canonical, b"untrusted diagnostic"
            )
        )

    def test_profile_rejects_script_and_mutable_profile_file(self) -> None:
        helper = self.root / "helper"
        helper.write_text("#!/bin/sh\nexit 0\n")
        helper.chmod(0o555)
        value = json.loads(self.profile_path.read_text())
        value["trusted_executable_roots"].append(str(self.root))
        value["profiles"]["model"]["executable"] = str(helper)
        value["profiles"]["model"]["executable_sha256"] = supervisor_module.sha256_file(helper)
        self.profile_path.write_text(json.dumps(value))
        self.profile_path.chmod(0o600)
        with self.assertRaisesRegex(supervisor_module.ProfileError, "native immutable helper"):
            self.supervisor.profiles.load()
        self.write_profiles()
        self.profile_path.chmod(0o666)
        with self.assertRaises(supervisor_module.SupervisorError):
            self.supervisor.profiles.load()


class CrashAndRollbackTests(SupervisorFixture):
    def test_supervisor_never_switches_for_a_helper_that_only_reports_success(self) -> None:
        self.prepare_staged_build()

        def invoke_without_transition(
            instance: supervisor_module.Supervisor,
            name: str,
            substitutions: dict[str, str],
            *,
            execute: bool,
        ) -> dict[str, object]:
            return self.original_invoke_profile(
                instance, name, substitutions, execute=execute
            )

        with mock.patch.object(
            supervisor_module.Supervisor,
            "invoke_profile",
            new=invoke_without_transition,
        ), self.assertRaisesRegex(
            supervisor_module.SupervisorError, "without switching"
        ):
            self.supervisor.activate("build-1", "intent-1", execute=True)
        self.assertEqual(self.supervisor.read_active_generation(), "g0")
        self.assertEqual(self.supervisor.read_state()["mode"], "rescue")

    def test_crash_after_link_switch_rolls_back_and_pauses(self) -> None:
        self.make_unbound_generation("g1")
        state = self.supervisor.read_state()
        state["inflight"] = {
            "phase": "profile_invoked",
            "intent_id": "intent-crash",
            "trace_id": "trace-crash",
            "response_sha256": "c" * 64,
            "build_id": "build-crash",
            "from_generation": "g0",
            "to_generation": "g1",
            "prepared_at": 999,
        }
        self.supervisor.write_state(state)
        self.supervisor.switch_active_link("g1")
        result = self.supervisor.recover_crash_state(execute=True)
        self.assertEqual(result["operation"], "rollback")
        self.assertEqual(self.supervisor.read_active_generation(), "g0")
        recovered = self.supervisor.read_state()
        self.assertEqual(recovered["mode"], "paused")
        self.assertIsNone(recovered["inflight"])

    def test_ambiguous_crash_enters_rescue_without_switching(self) -> None:
        self.make_unbound_generation("g1")
        self.make_unbound_generation("g2")
        state = self.supervisor.read_state()
        state["inflight"] = {
            "phase": "profile_invoked",
            "from_generation": "g0",
            "to_generation": "g1",
        }
        self.supervisor.write_state(state)
        self.supervisor.switch_active_link("g2")
        result = self.supervisor.recover_crash_state(execute=True)
        self.assertEqual(result["recovery"], "rescue_ambiguous_crash_state")
        self.assertEqual(self.supervisor.read_active_generation(), "g2")
        self.assertEqual(self.supervisor.read_state()["mode"], "rescue")

    def test_activation_failure_restores_old_ab_slot_and_enters_rescue(self) -> None:
        self.write_profiles({"activate": self.false_path})
        self.prepare_staged_build()
        with self.assertRaisesRegex(
            supervisor_module.SupervisorError, "previous A/B slot"
        ):
            self.supervisor.activate("build-1", "intent-1", execute=True)
        self.assertEqual(self.supervisor.read_active_generation(), "g0")
        self.assertEqual(self.supervisor.read_state()["mode"], "rescue")

    def test_operator_rollback_returns_to_previous_generation(self) -> None:
        self.prepare_staged_build()
        self.supervisor.activate("build-1", "intent-1", execute=True)
        result = self.supervisor.rollback("operator_requested", execute=True)
        self.assertEqual(result["to_generation"], "g0")
        self.assertEqual(self.supervisor.read_active_generation(), "g0")
        self.assertEqual(self.supervisor.read_state()["mode"], "paused")


class SchedulingAndProbationTests(SupervisorFixture):
    @staticmethod
    def native_health_payload(*, status: str = "complete", generation: str = "g1") -> dict[str, object]:
        complete = status == "complete"
        return {
            "schema": "astrid.edge_rescue_helper.health.v2",
            "active_generation_id": generation,
            "healthy": True,
            "available_ram_bytes": 3 * 1024 * 1024 * 1024,
            "swap_bytes": 0,
            "fill_samples": 648,
            "fill_coverage_seconds": 3_420,
            "fill_max_gap_seconds": 5.0,
            "fill_mean": 0.68,
            "fill_occupancy_65_735": 0.99,
            "probation_fill_coverage_complete": complete,
            "probation": {
                "schema": "astrid.edge_rescue_helper.probation_evaluation.v1",
                "status": status,
                "generation_id": generation,
                "started_at_unix_ms": 1,
                "elapsed_seconds": 3_600 if complete else 3_599,
                "samples": 7,
                "maximum_sample_gap_seconds": 300,
                "baseline_swap_bytes": 0,
                "current_swap_bytes": 0,
                "swap_growth_bytes": 0,
                "coverage_complete": complete,
                "coverage_due_but_incomplete": False,
                "failed": status == "failed",
                "ledger_head_sha256": "a" * 64,
            },
            "evidence_sha256": "b" * 64,
        }

    def test_due_requests_coalesce_behind_two_hour_floor(self) -> None:
        state = self.supervisor.read_state()
        state["due"] = None
        state["last_steward_started_at"] = 1_000
        self.supervisor.write_state(state)
        first = supervisor_module.Supervisor(self.config, now=1_100)
        first.mark_due("artifact_changed", execute=True)
        second = supervisor_module.Supervisor(self.config, now=1_200)
        result = second.mark_due("tool_completed", execute=True)
        self.assertEqual(result["due"]["not_before"], 8_200)
        self.assertEqual(result["due"]["coalesced_count"], 2)
        self.assertEqual(result["due"]["reasons"], ["artifact_changed", "tool_completed"])

    def test_dry_run_never_mutates_or_invokes(self) -> None:
        before = self.supervisor.state_path.read_bytes()
        result = self.supervisor.mark_due("dry_run", execute=False)
        self.assertTrue(result["dry_run"])
        self.assertEqual(self.supervisor.state_path.read_bytes(), before)
        steward = self.supervisor.steward(execute=False)
        self.assertEqual(steward["status"], "would_run")
        self.assertEqual(self.supervisor.state_path.read_bytes(), before)

    def test_probation_health_tick_is_inert_without_active_probation(self) -> None:
        before = self.supervisor.state_path.read_bytes()
        with mock.patch.object(
            self.supervisor, "invoke_profile", wraps=self.supervisor.invoke_profile
        ) as invoke_profile:
            result = self.supervisor.check_probation(execute=True)
        self.assertEqual(result, {"probation": "none", "dry_run": False})
        invoke_profile.assert_not_called()
        self.assertEqual(self.supervisor.state_path.read_bytes(), before)

    def test_regular_supervisor_delegates_active_probation_without_health_probe(self) -> None:
        self.prepare_staged_build()
        self.supervisor.activate("build-1", "intent-1", execute=True)
        with mock.patch.object(
            self.supervisor, "check_probation", wraps=self.supervisor.check_probation
        ) as check_probation:
            result = self.supervisor.supervise(execute=True)

        self.assertEqual(
            result["probation"]["probation"],
            "delegated_to_dedicated_sampler",
        )
        check_probation.assert_not_called()
        self.assertIsNotNone(self.supervisor.read_state()["probation"])

    def test_probation_cannot_accept_before_one_hour(self) -> None:
        self.prepare_staged_build()
        self.supervisor.activate("build-1", "intent-1", execute=True)
        early = supervisor_module.Supervisor(self.config, now=4_599)
        result = early.check_probation(execute=True)
        self.assertEqual(result["probation"], "healthy_waiting")
        self.assertIsNotNone(early.read_state()["probation"])
        mature = supervisor_module.Supervisor(self.config, now=4_600)
        result = mature.check_probation(execute=True)
        self.assertEqual(result["probation"], "accepted")
        self.assertIsNone(mature.read_state()["probation"])

    def test_native_health_parser_requires_complete_sample_evidence(self) -> None:
        payload = self.native_health_payload()
        canonical = supervisor_module.canonical_bytes(payload) + b"\n"
        parsed = supervisor_module.parse_native_health_result(canonical, b"")
        self.assertEqual(parsed["status"], "complete")
        payload["probation"]["samples"] = 6
        tampered = supervisor_module.canonical_bytes(payload) + b"\n"
        self.assertIsNone(supervisor_module.parse_native_health_result(tampered, b""))
        self.assertIsNone(supervisor_module.parse_native_health_result(canonical, b"diagnostic"))

    def test_native_retention_parser_requires_exact_paired_gc_receipt(self) -> None:
        payload = {
            "schema": "astrid.edge_rescue_helper.paired_retention.v1",
            "status": "retired_complete_signed_pairs",
            "active_generation": "g0",
            "retained_generations": ["g0", "g3", "g4", "g5"],
            "retired_generations": ["g1", "g2"],
            "retained_prior_minimum": 3,
            "minimum_retention_seconds": supervisor_module.RETENTION_SECONDS,
            "ledger_head_sha256": "a" * 64,
            "authority": "immutable_root_paired_generation_snapshot_gc",
        }
        canonical = supervisor_module.canonical_bytes(payload) + b"\n"
        parsed = supervisor_module.parse_native_retention_result(canonical, b"")
        self.assertEqual(parsed["retired_generations"], ["g1", "g2"])
        payload["retained_generations"] = ["g3", "g4", "g5"]
        self.assertIsNone(
            supervisor_module.parse_native_retention_result(
                supervisor_module.canonical_bytes(payload) + b"\n", b""
            )
        )


    def test_exit_zero_without_native_health_evidence_rolls_back(self) -> None:
        self.prepare_staged_build()
        self.supervisor.activate("build-1", "intent-1", execute=True)

        checker = supervisor_module.Supervisor(self.config, now=4_600)

        def forged_success(
            name: str,
            substitutions: dict[str, str],
            *,
            execute: bool,
        ) -> dict[str, object]:
            if name == "health":
                return {"exit_code": 0, "timed_out": False}
            receipt = self.original_invoke_profile(checker, name, substitutions, execute=execute)
            if execute and name == "rollback" and checker._profile_success(receipt):
                checker.switch_active_link(Path(substitutions["generation_dir"]).name)
            return receipt

        with mock.patch.object(checker, "invoke_profile", new=forged_success):
            result = checker.check_probation(execute=True)
        self.assertEqual(result["operation"], "rollback")
        self.assertEqual(checker.read_active_generation(), "g0")

    def test_native_active_status_never_accepts_at_python_deadline(self) -> None:
        self.prepare_staged_build()
        self.supervisor.activate("build-1", "intent-1", execute=True)

        def active_health(*_args: object, **_kwargs: object) -> dict[str, object]:
            return {
                "exit_code": 0,
                "timed_out": False,
                "health_result": {
                    "active_generation_id": "g1",
                    "status": "active",
                },
            }

        checker = supervisor_module.Supervisor(self.config, now=4_600)
        with mock.patch.object(checker, "invoke_profile", new=active_health):
            result = checker.check_probation(execute=True)
        self.assertEqual(result["probation"], "healthy_waiting")
        self.assertEqual(result["native_status"], "active")
        self.assertIsNotNone(checker.read_state()["probation"])

    def test_failed_health_probe_automatically_rolls_back(self) -> None:
        self.prepare_staged_build()
        self.supervisor.activate("build-1", "intent-1", execute=True)
        self.write_profiles({"health": self.false_path})
        checker = supervisor_module.Supervisor(self.config, now=1_100)
        result = checker.check_probation(execute=True)
        self.assertEqual(result["operation"], "rollback")
        self.assertEqual(checker.read_active_generation(), "g0")

    def test_retention_requires_active_plus_three_prior_and_seven_days(self) -> None:
        for index in range(1, 6):
            path = self.make_unbound_generation(f"g{index}")
            modified = 1_000 - supervisor_module.RETENTION_SECONDS - index
            os.utime(path, (modified, modified))
        result = self.supervisor.retention()
        self.assertGreaterEqual(len(result["retained"]), 4)
        self.assertEqual(result["minimum_generations"], 4)
        self.assertEqual(result["minimum_prior_generations"], 3)
        self.assertFalse(result["active_generation_counts_toward_prior_minimum"])
        self.assertNotIn("g0", result["eligible"])
        self.assertNotIn("g3", result["eligible"])
        self.assertEqual(set(result["eligible"]), {"g4", "g5"})
        policy = self.supervisor.status()["policy"]
        self.assertEqual(policy["retention_minimum_generations"], 4)
        self.assertEqual(policy["retention_minimum_prior_generations"], 3)
        self.assertFalse(policy["retention_active_counts_toward_prior_minimum"])


class SyntheticLifecycleTests(SupervisorFixture):
    def test_operator_request_runs_only_as_fixed_profile_and_clears_terminal_state(self) -> None:
        requested = self.supervisor.request_synthetic(execute=True)
        self.assertEqual(requested["status"], "queued")
        pending = self.supervisor.read_state()["synthetic_harness"]
        self.assertEqual(pending["status"], "pending")
        result = self.supervisor.supervise(execute=True)
        self.assertEqual(result["synthetic"]["status"], "completed")
        self.assertIsNone(self.supervisor.read_state()["synthetic_harness"])
        records = self.supervisor.ledger("operator").read()
        kinds = [record["core"]["kind"] for record in records]
        self.assertIn("synthetic_harness_requested", kinds)
        self.assertIn("synthetic_harness_started", kinds)
        self.assertIn("synthetic_harness_completed", kinds)
        started = next(
            record for record in records if record["core"]["kind"] == "synthetic_harness_started"
        )
        self.assertFalse(started["core"]["payload"]["caller_paths_accepted"])

    def test_interrupted_synthetic_profile_is_never_retried(self) -> None:
        self.supervisor.request_synthetic(execute=True)
        state = self.supervisor.read_state()
        state["synthetic_harness"]["status"] = "running"
        self.supervisor.write_state(state)
        restarted = supervisor_module.Supervisor(self.config, now=1_001)
        result = restarted.supervise(execute=True)
        self.assertEqual(result["synthetic"]["status"], "interrupted_no_retry")
        self.assertIsNone(restarted.read_state()["synthetic_harness"])

    def test_native_synthetic_receipt_is_canonical_nonproduction_evidence(self) -> None:
        receipts = []
        for label in (
            "build-model-stop",
            "build-model-start",
            "build-model-warmup",
        ):
            receipts.append(
                {
                    "label": label,
                    "executable_sha256": "a" * 64,
                    "argv_sha256": "b" * 64,
                    "exit_code": 0,
                    "timed_out": False,
                    "duration_ms": 1,
                }
            )
        value = {
            "schema": "astrid.edge_rescue_helper.synthetic_lifecycle.v1",
            "provenance": "operator_isolated_synthetic_machine_evidence_not_astrid_authorship",
            "appliance_id": "avado-test",
            "production_generation_before": "g0",
            "production_binding_sha256_before": "c" * 64,
            "production_binding_sha256_after": "c" * 64,
            "production_active_link_before": "releases/g0",
            "production_active_link_after": "releases/g0",
            "synthetic_candidate_id": "synthetic-candidate",
            "synthetic_build_id": "build-synthetic",
            "synthetic_generation_id": "gen-synthetic",
            "model_service_receipts": receipts,
            "candidate_source_changed": False,
            "offline_build_and_package_gates_passed": True,
            "isolated_activation_passed": True,
            "isolated_rollback_passed": True,
            "link_first_crash_recovered": True,
            "binding_first_crash_recovered": True,
            "production_intent_created": False,
            "production_generation_switched": False,
            "continuity_or_reservoir_admission": False,
            "sandbox_root": "/builder/synthetic-harness/runs/synthetic-1-2-3",
            "evidence_sha256": "",
        }
        value["evidence_sha256"] = supervisor_module.sha256_bytes(
            supervisor_module.canonical_bytes(value)
        )
        wire = supervisor_module.canonical_bytes(value) + b"\n"
        parsed = supervisor_module.parse_synthetic_lifecycle_result(wire, b"")
        self.assertIsNotNone(parsed)
        value["production_generation_switched"] = True
        tampered = supervisor_module.canonical_bytes(value) + b"\n"
        self.assertIsNone(supervisor_module.parse_synthetic_lifecycle_result(tampered, b""))


class AutonomousIntentTests(SupervisorFixture):
    def test_same_reflection_is_attested_before_build_and_activates_automatically(self) -> None:
        candidate = self.candidate_value()
        self.supervisor.record_candidate(candidate, execute=True)
        attested = self.supervisor.record_scheduled_intent(
            self.signed_envelope(candidate, self.intent_value(candidate)), execute=True
        )
        self.assertEqual(attested["operation"], "attest_scheduled_model_intent")
        self.assertEqual(self.supervisor.builds(), {})

        build = self.build_value()
        self.supervisor.record_build(build, execute=True)
        self.make_bound_generation(build)
        self.supervisor.stage("build-1", execute=True)
        result = self.supervisor.supervise(execute=True)
        self.assertEqual(result["activation"]["operation"], "activate")
        self.assertEqual(self.supervisor.read_active_generation(), "g1")
        self.assertIsNotNone(self.supervisor.read_state()["probation"])

    def test_fallback_repair_operator_transport_and_cross_box_intents_are_rejected(self) -> None:
        candidate = self.candidate_value()
        self.supervisor.record_candidate(candidate, execute=True)
        invalid_cases = (
            {"fallback": True},
            {"executor_repair": True},
            {"operator_harness": True},
            {"origin": "operator"},
            {"transport_status": "transport_recovery"},
            {"authorship_status": "executor_authored_fallback"},
            {"declaration_provenance": "formatting_repair"},
            {"appliance_id": "icp-other-box"},
        )
        for index, overrides in enumerate(invalid_cases):
            intent = self.intent_value(
                candidate,
                intent_id=f"invalid-{index}",
                trace_id=f"trace-invalid-{index}",
                **overrides,
            )
            with self.subTest(overrides=overrides), self.assertRaises(
                supervisor_module.SupervisorError
            ):
                self.supervisor.record_scheduled_intent(
                    self.signed_envelope(
                        candidate, intent, envelope_id=f"invalid-envelope-{index}"
                    ),
                    execute=True,
                )
        self.assertEqual(self.supervisor.scheduled_intents(), {})

    def test_candidate_digest_and_intent_replay_are_rejected(self) -> None:
        candidate = self.candidate_value()
        self.supervisor.record_candidate(candidate, execute=True)
        bad = self.intent_value(candidate, candidate_sha256="0" * 64)
        with self.assertRaisesRegex(
            supervisor_module.SupervisorError, "exact nested intent|exact candidate"
        ):
            self.supervisor.record_scheduled_intent(
                self.signed_envelope(candidate, bad), execute=True
            )
        intent = self.intent_value(candidate)
        envelope = self.signed_envelope(candidate, intent)
        self.supervisor.record_scheduled_intent(envelope, execute=True)
        with self.assertRaisesRegex(supervisor_module.SupervisorError, "replay"):
            self.supervisor.record_scheduled_intent(envelope, execute=True)

    def test_recovery_accepts_only_the_exact_same_attestor_envelope(self) -> None:
        candidate = self.candidate_value()
        envelope = self.signed_envelope(candidate, self.intent_value(candidate))
        self.supervisor.record_scheduled_intent(envelope, execute=True)
        recovered = self.supervisor.record_scheduled_intent(
            envelope, execute=True, allow_recovery=True
        )
        self.assertEqual(recovered["intent_id"], "intent-1")

        changed = self.intent_value(candidate, trace_id="trace-changed")
        conflicting = self.signed_envelope(candidate, changed)
        with self.assertRaisesRegex(supervisor_module.IntegrityError, "exactly match"):
            self.supervisor.record_scheduled_intent(
                conflicting, execute=True, allow_recovery=True
            )

        second = self.intent_value(
            candidate,
            intent_id="intent-2",
            trace_id="trace-2",
            session_id="session-2",
            turn_id="turn-2",
        )
        reused_envelope_id = self.signed_envelope(candidate, second)
        with self.assertRaisesRegex(supervisor_module.IntegrityError, "envelope_id replay"):
            self.supervisor.record_scheduled_intent(
                reused_envelope_id, execute=True, allow_recovery=True
            )

    def test_unattested_build_cannot_activate(self) -> None:
        candidate = self.candidate_value()
        build = self.build_value()
        self.supervisor.record_candidate(candidate, execute=True)
        self.supervisor.record_build(build, execute=True)
        self.make_bound_generation(build)
        self.supervisor.stage("build-1", execute=True)
        with self.assertRaisesRegex(supervisor_module.SupervisorError, "attested intent"):
            self.supervisor.activate("build-1", "missing-intent", execute=True)


class AttestedInboxPipelineTests(SupervisorFixture):
    def write_inbox_envelope(
        self, envelope: dict[str, object], filename: str = "candidate-intent-envelope-1.json"
    ) -> Path:
        inbox = self.state_root / "inbox"
        inbox.mkdir(mode=0o700, exist_ok=True)
        path = inbox / filename
        path.write_bytes(supervisor_module.canonical_bytes(envelope) + b"\n")
        path.chmod(0o600)
        return path

    def write_handoff_trigger(self, envelope_id: str = "envelope-1") -> Path:
        inbox = self.state_root / "inbox"
        inbox.mkdir(mode=0o700, exist_ok=True)
        path = inbox / f"candidate-ready-{envelope_id}.json"
        path.write_text("{\"authority\":\"trigger_only_no_candidate_or_deployment_authority\"}\n")
        path.chmod(0o600)
        return path

    def write_pending_handoff_trigger(self, envelope_id: str = "envelope-1") -> Path:
        inbox = self.state_root / "inbox"
        inbox.mkdir(mode=0o700, exist_ok=True)
        path = inbox / f"candidate-ready-{envelope_id}.pending"
        path.write_text(
            '{"authority":"trigger_only_no_candidate_or_deployment_authority"}\n'
        )
        path.chmod(0o600)
        return path

    def test_pending_handoff_is_never_consumed_or_treated_as_authority(self) -> None:
        pending = self.write_pending_handoff_trigger("envelope-pending")
        before = pending.read_bytes()

        result = self.supervisor.pipeline.ingest_one(execute=True)

        self.assertEqual(result["status"], "no_valid_envelope")
        self.assertEqual(result["ignored_pending_handoff_triggers"], 1)
        self.assertEqual(pending.read_bytes(), before)
        self.assertEqual(self.supervisor.candidates(), {})
        self.assertEqual(self.supervisor.scheduled_intents(), {})
        self.assertEqual(
            self.supervisor.pipeline.inbox_summary()["pending_handoff_triggers"], 1
        )
        self.assertFalse(
            any(
                record["core"]["kind"]
                in {"inbox_handoff_trigger_discarded", "inbox_rejected"}
                for record in self.supervisor.ledger("operator").read()
            )
        )

    def test_paused_pipeline_leaves_pending_root_cleanup_marker_byte_exact(self) -> None:
        self.replace_state(mode="paused", paused_reason="operator_pause")
        pending = self.write_pending_handoff_trigger("envelope-pending")
        before = pending.read_bytes()

        result = self.supervisor.pipeline.ingest_one(execute=True)

        self.assertEqual(result["status"], "paused_queued_untouched")
        self.assertEqual(result["ignored_pending_handoff_triggers"], 1)
        self.assertEqual(pending.read_bytes(), before)

    def test_non_authorizing_handoff_trigger_is_consumed_only_as_a_wakeup(self) -> None:
        envelope = self.signed_envelope()
        intent_path = self.write_inbox_envelope(envelope)
        ready = self.write_handoff_trigger()

        result = self.supervisor.pipeline.ingest_one(execute=True)

        self.assertEqual(result["status"], "accepted")
        self.assertEqual(result["discarded_handoff_triggers"], 1)
        self.assertFalse(intent_path.exists())
        self.assertFalse(ready.exists())
        self.assertIn("intent-1", self.supervisor.scheduled_intents())
        trigger_events = [
            record
            for record in self.supervisor.ledger("operator").read()
            if record["core"]["kind"] == "inbox_handoff_trigger_discarded"
        ]
        self.assertEqual(len(trigger_events), 1)
        self.assertEqual(
            trigger_events[0]["core"]["payload"]["authority"],
            "trigger_only_no_candidate_or_deployment_authority",
        )

    def test_orphan_handoff_trigger_cannot_create_or_authorize_a_candidate(self) -> None:
        ready = self.write_handoff_trigger("envelope-orphan")

        result = self.supervisor.pipeline.ingest_one(execute=True)

        self.assertEqual(result["status"], "no_valid_envelope")
        self.assertEqual(result["discarded_handoff_triggers"], 1)
        self.assertFalse(ready.exists())
        self.assertEqual(self.supervisor.candidates(), {})
        self.assertEqual(self.supervisor.scheduled_intents(), {})

    def test_bare_forged_and_ledger_signed_runtime_claims_are_rejected(self) -> None:
        candidate = self.candidate_value()
        intent = self.intent_value(candidate)
        with self.assertRaises(supervisor_module.SupervisorError):
            self.supervisor.record_scheduled_intent(intent, execute=True)

        forged = self.signed_envelope(candidate, intent)
        forged["auth"]["signature"] = "0" * 64  # type: ignore[index]
        with self.assertRaisesRegex(supervisor_module.IntegrityError, "authentication"):
            self.supervisor.record_scheduled_intent(forged, execute=True)

        wrong_root = self.signed_envelope(candidate, intent, signing_key=self.key_path)
        with self.assertRaisesRegex(supervisor_module.IntegrityError, "authentication"):
            self.supervisor.record_scheduled_intent(wrong_root, execute=True)

        runtime_path = self.write_inbox_envelope(
            intent, "candidate-intent-runtime-outbox.json"
        )
        result = self.supervisor.pipeline.ingest_one(execute=True)
        self.assertEqual(result["status"], "no_valid_envelope")
        self.assertFalse(runtime_path.exists())
        self.assertEqual(self.supervisor.scheduled_intents(), {})
        self.assertEqual(self.supervisor.pipeline.inbox_summary()["quarantined"], 1)

    def test_missing_or_mismatched_authored_completion_is_quarantined(self) -> None:
        bare = self.bare_signed_intent_envelope(envelope_id="envelope-bare-v1")
        bare_path = self.write_inbox_envelope(
            bare, "candidate-intent-envelope-bare-v1.json"
        )

        mismatched = self.signed_envelope(envelope_id="envelope-mismatch")
        completion = mismatched["authored_completion"]
        assert isinstance(completion, dict)
        completion_core = completion["core"]
        assert isinstance(completion_core, dict)
        completion_core["response_sha256"] = "8" * 64
        self.resign_completed_envelope(mismatched)
        mismatch_path = self.write_inbox_envelope(
            mismatched, "candidate-intent-envelope-mismatch.json"
        )

        result = self.supervisor.pipeline.ingest_one(execute=True)

        self.assertEqual(result["status"], "no_valid_envelope")
        self.assertFalse(bare_path.exists())
        self.assertFalse(mismatch_path.exists())
        self.assertEqual(self.supervisor.scheduled_intents(), {})
        self.assertEqual(self.supervisor.pipeline.inbox_summary()["quarantined"], 2)

    def test_intent_ingest_freshness_is_exactly_the_pipeline_lifetime(self) -> None:
        maximum_age = supervisor_module.INTENT_INGEST_MAX_AGE_SECONDS
        self.supervisor = supervisor_module.Supervisor(
            self.config, now=maximum_age + 5_000
        )

        def aged_envelope(age: int, envelope_id: str) -> dict[str, object]:
            candidate = self.candidate_value()
            intent = self.intent_value(
                candidate,
                intent_id=f"intent-{envelope_id}",
                observed_at=self.supervisor.now - age,
            )
            envelope = self.signed_envelope(
                candidate,
                intent,
                envelope_id=envelope_id,
                created_at=self.supervisor.now - age,
            )
            return envelope

        boundary = aged_envelope(maximum_age, "envelope-boundary")
        accepted = self.supervisor.record_scheduled_intent(boundary, execute=True)
        self.assertEqual(accepted["status"], "accepted")

        expired = aged_envelope(maximum_age + 1, "envelope-expired")
        with self.assertRaisesRegex(
            supervisor_module.IntegrityError, "not fresh"
        ):
            self.supervisor.record_scheduled_intent(expired, execute=True)

    def test_signed_inbox_build_stage_activate_and_status_projection(self) -> None:
        candidate = self.candidate_value()
        intent = self.intent_value(candidate)
        envelope = self.signed_envelope(candidate, intent)
        self.write_inbox_envelope(envelope)

        build = self.build_value()
        build_source = self.root / "config" / "build-manifest.json"
        build_source.write_text(json.dumps(build))
        build_source.chmod(0o600)
        cp_path = Path(shutil.which("cp") or "/bin/cp").resolve()
        self.configure_profile(
            "build", cp_path, [str(build_source), "{build_manifest}"]
        )
        (self.model_handoff_root / "envelope-1.json").write_text("{}")
        (self.model_handoff_root / "envelope-1.json").chmod(0o600)
        self.make_bound_generation(build)

        result = self.supervisor.supervise(execute=True)
        self.assertEqual(result["inbox"]["status"], "accepted")
        self.assertEqual(result["build"]["status"], "staged")
        self.assertEqual(result["activation"]["operation"], "activate")
        self.assertEqual(self.supervisor.read_active_generation(), "g1")
        processed = list((self.state_root / "inbox" / "processed").iterdir())
        self.assertEqual(len(processed), 1)
        self.assertEqual(stat.S_IMODE(processed[0].stat().st_mode), 0o600)
        status_path = self.state_root / "status.json"
        status = json.loads(status_path.read_text())
        self.assertEqual(stat.S_IMODE(status_path.stat().st_mode), 0o600)
        self.assertTrue(status["projection_only_not_authority"])
        self.assertEqual(status["last_pass"]["operation"], "supervise")
        self.assertEqual(status["active_link_generation"], "g1")

    def test_exact_candidate_rejection_is_terminal_exportable_and_not_retried(self) -> None:
        candidate = self.candidate_value()
        envelope = self.signed_envelope(candidate, self.intent_value(candidate))
        self.write_inbox_envelope(envelope)
        self.assertEqual(
            self.supervisor.pipeline.ingest_one(execute=True)["status"], "accepted"
        )
        handoff = self.model_handoff_root / "envelope-1.json"
        handoff.write_text("{}")
        handoff.chmod(0o600)
        reason_sha256 = "d" * 64
        receipt = {
            "timed_out": False,
            "exit_code": 65,
            "result_status": "candidate_rejected",
            "result_reason_sha256": reason_sha256,
        }

        with mock.patch(
            "edge_self_change.profiles.run_command_profile", return_value=receipt
        ):
            rejected = self.supervisor.pipeline.advance_one_build(execute=True)

        self.assertEqual(rejected["status"], "candidate_rejected_terminal_no_retry")
        self.assertEqual(rejected["reason_sha256"], reason_sha256)
        self.assertEqual(self.supervisor.read_state()["mode"], "running")
        self.assertEqual(
            self.supervisor.pipeline.advance_one_build(execute=True)["status"],
            "no_pending_intent",
        )
        consumed, bindings = self.supervisor.consumed_intents()
        self.assertEqual(consumed, {"intent-1"})
        self.assertEqual(
            bindings,
            {("trace-1", "e" * 64, "f" * 64)},
        )
        status = projection_module.steward_status(self.supervisor)
        self.assertFalse(status["pipeline_busy"])
        self.assertEqual(status["candidate"]["status"], "rejected")
        self.assertEqual(
            status["candidate"]["terminal_reason_sha256"], reason_sha256
        )
        rejection = [
            record
            for record in self.supervisor.ledger("activation").read()
            if record["core"]["kind"] == "scheduled_intent_terminal_rejected"
        ]
        self.assertEqual(len(rejection), 1)
        self.assertEqual(rejection[0]["core"]["payload"]["trace_id"], "trace-1")
        self.assertEqual(
            rejection[0]["core"]["payload"]["authority"],
            "terminal_exact_candidate_rejection_no_promotion",
        )
        operator = projection_module.operator_status(
            self.supervisor,
            {"operation": "supervise", "status": "candidate_rejected"},
        )
        terminal = next(
            event
            for event in operator["core"]["lifecycle"]["events"]
            if event["status"] == "scheduled_intent_terminal_rejected"
        )
        self.assertEqual(terminal["terminal_reason_sha256"], reason_sha256)
        self.assertEqual(
            terminal["terminal_authority"],
            "terminal_exact_candidate_rejection_no_promotion",
        )
        self.assertFalse(terminal["automatic_retry"])
        self.assertNotIn("command_receipt", terminal)

    def test_duplicate_partial_and_out_of_order_inputs_are_deterministic(self) -> None:
        envelope = self.signed_envelope()
        partial = self.write_inbox_envelope(
            envelope, "000-upload.partial"
        )
        invalid = self.write_inbox_envelope(
            self.build_value(), "build-before-intent.json"
        )
        valid = self.write_inbox_envelope(envelope)
        first = self.supervisor.pipeline.ingest_one(execute=True)
        self.assertEqual(first["status"], "accepted")
        self.assertEqual(first["rejected"], 1)
        self.assertEqual(first["ignored_partial"], 1)
        self.assertTrue(partial.exists())
        self.assertFalse(invalid.exists())
        self.assertFalse(valid.exists())

        duplicate = self.write_inbox_envelope(envelope)
        second = self.supervisor.pipeline.ingest_one(execute=True)
        self.assertEqual(second["status"], "accepted")
        self.assertFalse(duplicate.exists())
        self.assertEqual(len(self.supervisor.scheduled_intents()), 1)
        self.assertEqual(self.supervisor.pipeline.inbox_summary()["processed"], 1)

    def test_interrupted_build_enters_rescue_after_restart_without_retry(self) -> None:
        envelope = self.signed_envelope()
        self.supervisor.record_scheduled_intent(envelope, execute=True)
        self.supervisor.ledger("build").append(
            "build_profile_started",
            {"candidate_id": "candidate-1", "intent_id": "intent-1"},
            "build-started-intent-1",
            self.supervisor.now,
        )
        restarted = supervisor_module.Supervisor(self.config, now=1_001)
        result = restarted.supervise(execute=True)
        self.assertEqual(result["build"]["status"], "rescue")
        self.assertEqual(restarted.read_state()["mode"], "rescue")
        starts = [
            record
            for record in restarted.ledger("build").read()
            if record["core"]["kind"] == "build_profile_started"
        ]
        self.assertEqual(len(starts), 1)
        status = json.loads((self.state_root / "status.json").read_text())
        self.assertEqual(status["state"]["paused_reason"], "build_profile_interrupted_no_retry")

    def test_steward_retains_due_without_a_valid_attested_result(self) -> None:
        result = self.supervisor.steward(execute=True)
        self.assertEqual(result["status"], "no_attested_result_due_retained")
        state = self.supervisor.read_state()
        self.assertIsNotNone(state["due"])
        self.assertEqual(state["due"]["not_before"], 1_000 + supervisor_module.DUE_COALESCE_SECONDS)
        status = json.loads((self.state_root / "status.json").read_text())
        self.assertEqual(status["last_pass"]["operation"], "steward")

    def test_fifteen_minute_poll_cannot_bypass_two_hour_reflection_floor(self) -> None:
        first = self.supervisor.steward(execute=True)
        self.assertEqual(first["status"], "no_attested_result_due_retained")
        after_one_poll = supervisor_module.Supervisor(
            self.config, now=self.supervisor.now + 15 * 60
        ).steward(execute=True)
        self.assertEqual(after_one_poll["status"], "not_due")
        self.assertEqual(
            after_one_poll["due"]["not_before"],
            self.supervisor.now + supervisor_module.DUE_COALESCE_SECONDS,
        )

    def test_steward_accepts_only_the_helpers_signed_envelope_output(self) -> None:
        envelope = self.signed_envelope()
        envelope_source = self.root / "config" / "intent-envelope.json"
        envelope_source.write_text(json.dumps(envelope))
        envelope_source.chmod(0o600)
        cp_path = Path(shutil.which("cp") or "/bin/cp").resolve()
        self.configure_profile(
            "model", cp_path, [str(envelope_source), "{intent_envelope}"]
        )
        result = self.supervisor.steward(execute=True)
        self.assertEqual(result["status"], "attested_result_ingested")
        self.assertIsNone(self.supervisor.read_state()["due"])
        self.assertIn("intent-1", self.supervisor.scheduled_intents())


if __name__ == "__main__":
    unittest.main()
