#!/usr/bin/env python3
"""Tests for source-first projection generation coordination."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import time
import unittest

try:
    from evidence_store import EvidenceEventStore
    from steward_control.config import ControlConfig
    from steward_control.controller import StewardController
    from steward_control.errors import ProjectionError
    from steward_control.projection import (
        CommandResult,
        ProjectionCommand,
        ProjectionCoordinator,
        ProjectionStep,
        _default_runner,
        hash_source_globs,
        source_first_steps,
    )
except ModuleNotFoundError:
    from scripts.evidence_store import EvidenceEventStore
    from scripts.steward_control.config import ControlConfig
    from scripts.steward_control.controller import StewardController
    from scripts.steward_control.errors import ProjectionError
    from scripts.steward_control.projection import (
        CommandResult,
        ProjectionCommand,
        ProjectionCoordinator,
        ProjectionStep,
        _default_runner,
        hash_source_globs,
        source_first_steps,
    )


def run_git(repo: Path, *args: str) -> None:
    subprocess.run(
        ["git", "-C", str(repo), *args],
        check=True,
        capture_output=True,
        timeout=20,
    )


class FakeCoordinator:
    def __init__(self) -> None:
        self.calls: list[tuple[str, str, str]] = []

    def run(
        self,
        *,
        actor: str,
        run_id: str,
        phase: str,
        control=None,
        full_rebuild: bool = False,
        resume_generation: str | None = None,
    ) -> dict[str, str]:
        del full_rebuild, resume_generation
        self.calls.append((actor, run_id, phase))
        if control:
            control(
                {
                    "generation_id": f"{phase}_{len(self.calls)}",
                    "completed_step_count": 1,
                }
            )
        return {
            "generation_id": f"{phase}_{len(self.calls)}",
            "status": "passed",
        }

    def plan(self) -> dict[str, object]:
        return {"steps": [], "mutates": False}


class StewardProjectionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        run_git(self.repo, "init", "-q")
        run_git(self.repo, "config", "user.email", "test@example.invalid")
        run_git(self.repo, "config", "user.name", "Projection Test")
        (self.repo / "README.md").write_text("test\n", encoding="utf-8")
        run_git(self.repo, "add", "README.md")
        run_git(self.repo, "commit", "-qm", "initial")

        self.workspace = self.repo / "workspace"
        self.workspace.mkdir()
        diagnostics = self.workspace / "diagnostics"
        self.store_root = diagnostics / "evidence_event_store_v2"
        self.state_root = diagnostics / "steward_control_v1"
        legacy = diagnostics / "legacy/events.jsonl"
        legacy.parent.mkdir(parents=True)
        legacy.write_text('{"legacy":true}\n', encoding="utf-8")
        legacy_hash = hashlib.sha256(legacy.read_bytes()).hexdigest()
        store = EvidenceEventStore(self.store_root)
        store.initialize_from_envelopes([], legacy_imported=True)
        store.activation_path.write_text(
            json.dumps({"active_store": "v2"}) + "\n",
            encoding="utf-8",
        )
        (self.store_root / "migration_receipt.json").write_text(
            json.dumps(
                {
                    "sources": [
                        {
                            "stream": "legacy",
                            "path": str(legacy),
                            "sha256": legacy_hash,
                        }
                    ]
                }
            )
            + "\n",
            encoding="utf-8",
        )
        self.config = ControlConfig(
            repo_root=self.repo,
            workspace=self.workspace,
            state_root=self.state_root,
            store_root=self.store_root,
            lease_ttl_secs=20,
            heartbeat_interval_secs=1,
            max_run_secs=5,
            projector_timeout_secs=2,
            pause_grace_secs=1,
            repositories={"astrid": self.repo},
        )
        self.source = self.workspace / "source/input.json"
        self.source.parent.mkdir()
        self.source.write_text('{"input":1}\n', encoding="utf-8")
        self.first_output = self.workspace / "outputs/first.json"
        self.second_output = self.workspace / "outputs/second.json"
        self.first_output.parent.mkdir()
        self.first_output.write_text('{"first":1}\n', encoding="utf-8")
        self.second_output.write_text('{"second":1}\n', encoding="utf-8")
        self.steps = (
            ProjectionStep(
                "first",
                (),
                (ProjectionCommand(("projector", "{workspace}")),),
                ("addressing",),
                ("source/*.json",),
                ("outputs/first.json",),
            ),
            ProjectionStep(
                "second",
                ("first",),
                (ProjectionCommand(("projector", "second")),),
                ("sandbox",),
                ("outputs/first.json",),
                ("outputs/second.json",),
            ),
        )

    def tearDown(self) -> None:
        self.temp.cleanup()

    @staticmethod
    def successful_runner(
        argv: tuple[str, ...],
        *,
        cwd: Path,
        timeout: int,
    ) -> CommandResult:
        del argv, cwd, timeout
        return CommandResult(0, b'{"schema":"fixture_projection_v1"}', b"", 1)

    def test_generation_runs_in_dependency_order_and_writes_v3_checkpoints(
        self,
    ) -> None:
        calls: list[tuple[str, ...]] = []

        def runner(
            argv: tuple[str, ...],
            *,
            cwd: Path,
            timeout: int,
        ) -> CommandResult:
            del cwd, timeout
            calls.append(tuple(argv))
            return self.successful_runner(argv, cwd=self.repo, timeout=2)

        coordinator = ProjectionCoordinator(
            self.config,
            steps=self.steps,
            runner=runner,
        )
        manifest = coordinator.run(actor="test", run_id="run", phase="manual")
        self.assertEqual([step["step_id"] for step in manifest["steps"]], [
            "first",
            "second",
        ])
        self.assertEqual(calls[0][1], str(self.workspace))
        self.assertTrue(coordinator.latest_path.is_file())
        checkpoint = json.loads(
            (
                self.store_root
                / "checkpoints/steward_first_v1.json"
            ).read_text(encoding="utf-8")
        )
        self.assertEqual(
            checkpoint["schema"],
            "evidence_event_projection_checkpoint_v3",
        )
        self.assertEqual(
            checkpoint["input_identity"]["input_streams"],
            ["addressing"],
        )

    def test_builtin_profile_declares_complete_source_first_order(self) -> None:
        self.assertEqual(
            [step.step_id for step in source_first_steps()],
            [
                "addressing",
                "sandbox",
                "corridor",
                "signal_spine",
                "lived_state_witness",
                "reciprocal_uptake",
                "representation_contracts",
                "claim_families",
                "experiment_dossiers",
                "authority_temporal",
                "model_qos",
                "felt_mechanism_concordance",
                "agency_commons",
                "felt_contracts",
                "attention_portfolio",
            ],
        )
        steps = {step.step_id: step for step in source_first_steps()}
        self.assertEqual(
            steps["lived_state_witness"].dependencies,
            ("signal_spine",),
        )
        self.assertEqual(
            steps["lived_state_witness"].input_streams,
            ("addressing", "signal_spine"),
        )
        self.assertIn(
            "diagnostics/lived_state_witness_v1/witnesses.jsonl",
            steps["lived_state_witness"].outputs,
        )
        self.assertIn(
            "diagnostics/lived_state_witness_v1/gaps.jsonl",
            steps["lived_state_witness"].outputs,
        )
        self.assertNotIn(
            "lived_state_witness",
            steps["claim_families"].dependencies,
        )
        self.assertNotIn(
            "lived_state_witness",
            steps["experiment_dossiers"].input_streams,
        )
        self.assertIn(
            "diagnostics/lived_state_witness_v1/context_index.jsonl",
            steps["experiment_dossiers"].source_globs,
        )
        self.assertEqual(
            steps["felt_mechanism_concordance"].dependencies,
            (
                "experiment_dossiers",
                "model_qos",
                "reciprocal_uptake",
                "representation_contracts",
            ),
        )
        self.assertEqual(
            steps["felt_contracts"].dependencies,
            (
                "experiment_dossiers",
                "corridor",
                "signal_spine",
                "lived_state_witness",
                "reciprocal_uptake",
                "representation_contracts",
                "authority_temporal",
                "model_qos",
                "felt_mechanism_concordance",
                "agency_commons",
            ),
        )
        self.assertEqual(
            steps["attention_portfolio"].dependencies,
            ("felt_contracts",),
        )

    def test_failure_preserves_previous_successful_manifest(self) -> None:
        coordinator = ProjectionCoordinator(
            self.config,
            steps=self.steps,
            runner=self.successful_runner,
        )
        first = coordinator.run(actor="test", run_id="one", phase="manual")
        latest_before = coordinator.latest_path.read_bytes()
        calls = 0

        def failing_runner(
            argv: tuple[str, ...],
            *,
            cwd: Path,
            timeout: int,
        ) -> CommandResult:
            nonlocal calls
            del argv, cwd, timeout
            calls += 1
            if calls == 2:
                return CommandResult(9, b"", b"failure", 1)
            return CommandResult(0, b"{}", b"", 1)

        failed = ProjectionCoordinator(
            self.config,
            steps=self.steps,
            runner=failing_runner,
        )
        with self.assertRaises(ProjectionError):
            failed.run(
                actor="test",
                run_id="two",
                phase="manual",
                full_rebuild=True,
            )
        self.assertEqual(latest_before, failed.latest_path.read_bytes())
        self.assertEqual(
            json.loads(latest_before)["generation_id"],
            first["generation_id"],
        )
        self.assertEqual(len(list(failed.failed_root.glob("*.json"))), 1)

    def test_authority_marker_rejection_stops_generation(self) -> None:
        def runner(
            argv: tuple[str, ...],
            *,
            cwd: Path,
            timeout: int,
        ) -> CommandResult:
            del argv, cwd, timeout
            return CommandResult(0, b'{"live_eligible_now":true}', b"", 1)

        coordinator = ProjectionCoordinator(
            self.config,
            steps=self.steps[:1],
            runner=runner,
        )
        with self.assertRaises(ProjectionError):
            coordinator.run(actor="test", run_id="run", phase="manual")
        self.assertFalse(coordinator.latest_path.exists())

    def test_non_json_command_output_stops_generation(self) -> None:
        def runner(
            argv: tuple[str, ...],
            *,
            cwd: Path,
            timeout: int,
        ) -> CommandResult:
            del argv, cwd, timeout
            return CommandResult(0, b"not-json", b"", 1)

        coordinator = ProjectionCoordinator(
            self.config,
            steps=self.steps[:1],
            runner=runner,
        )
        with self.assertRaises(ProjectionError):
            coordinator.run(actor="test", run_id="run", phase="manual")
        self.assertFalse(coordinator.latest_path.exists())

    def test_source_hashes_are_deterministic_and_change_with_input(self) -> None:
        first = hash_source_globs(self.workspace, ("source/*.json",))
        second = hash_source_globs(self.workspace, ("source/*.json",))
        self.assertEqual(first, second)
        self.source.write_text('{"input":2}\n', encoding="utf-8")
        self.assertNotEqual(
            first,
            hash_source_globs(self.workspace, ("source/*.json",)),
        )

    def test_no_input_generation_reuses_all_steps_and_appends_no_events(
        self,
    ) -> None:
        calls: list[tuple[str, ...]] = []

        def runner(
            argv: tuple[str, ...],
            *,
            cwd: Path,
            timeout: int,
        ) -> CommandResult:
            del cwd, timeout
            calls.append(tuple(argv))
            return CommandResult(0, b'{"schema":"bounded_fixture_v1"}', b"", 1)

        coordinator = ProjectionCoordinator(
            self.config,
            steps=self.steps,
            runner=runner,
        )
        first = coordinator.run(actor="test", run_id="one", phase="manual")
        before_count = EvidenceEventStore(self.store_root).verify().event_count
        second = coordinator.run(actor="test", run_id="two", phase="manual")
        after_count = EvidenceEventStore(self.store_root).verify().event_count
        self.assertEqual(first["executed_step_count"], 2)
        self.assertEqual(second["reused_step_count"], 2)
        self.assertEqual(len(calls), 2)
        self.assertEqual(before_count, after_count)
        self.assertTrue(
            all(step["status"] == "reused" for step in second["steps"])
        )

    def test_unrelated_stream_reuses_but_source_and_dependency_changes_do_not(
        self,
    ) -> None:
        calls: list[tuple[str, ...]] = []

        def runner(
            argv: tuple[str, ...],
            *,
            cwd: Path,
            timeout: int,
        ) -> CommandResult:
            del cwd, timeout
            calls.append(tuple(argv))
            return CommandResult(0, b'{"schema":"bounded_fixture_v1"}', b"", 1)

        coordinator = ProjectionCoordinator(
            self.config,
            steps=self.steps,
            runner=runner,
        )
        coordinator.run(actor="test", run_id="one", phase="manual")
        EvidenceEventStore(self.store_root).append_payloads(
            "unrelated",
            [{"event_type": "unrelated"}],
        )
        unrelated = coordinator.run(
            actor="test",
            run_id="two",
            phase="manual",
        )
        self.assertEqual(unrelated["reused_step_count"], 2)
        self.assertEqual(len(calls), 2)

        self.source.write_text('{"input":2}\n', encoding="utf-8")
        source_changed = coordinator.run(
            actor="test",
            run_id="three",
            phase="manual",
        )
        self.assertEqual(source_changed["executed_step_count"], 1)
        self.assertEqual(source_changed["steps"][0]["status"], "passed")
        self.assertEqual(source_changed["steps"][1]["status"], "reused")

        self.first_output.write_text('{"first":2}\n', encoding="utf-8")
        dependency_changed = coordinator.run(
            actor="test",
            run_id="four",
            phase="manual",
        )
        self.assertEqual(dependency_changed["executed_step_count"], 2)

    def test_failed_generation_journal_resumes_completed_steps(self) -> None:
        calls = 0

        def fail_second(
            argv: tuple[str, ...],
            *,
            cwd: Path,
            timeout: int,
        ) -> CommandResult:
            nonlocal calls
            del argv, cwd, timeout
            calls += 1
            if calls == 2:
                return CommandResult(9, b"", b"failure", 1)
            return CommandResult(0, b'{"schema":"fixture_v1"}', b"", 1)

        failed = ProjectionCoordinator(
            self.config,
            steps=self.steps,
            runner=fail_second,
        )
        with self.assertRaises(ProjectionError):
            failed.run(actor="test", run_id="failed", phase="manual")
        journal_paths = list((failed.root / "journals").glob("*.json"))
        self.assertEqual(len(journal_paths), 1)
        failed_generation = journal_paths[0].stem
        journal = json.loads(journal_paths[0].read_text(encoding="utf-8"))
        self.assertEqual(journal["status"], "failed")
        self.assertEqual(journal["completed_step_count"], 1)

        resumed_calls: list[tuple[str, ...]] = []

        def successful(
            argv: tuple[str, ...],
            *,
            cwd: Path,
            timeout: int,
        ) -> CommandResult:
            del cwd, timeout
            resumed_calls.append(tuple(argv))
            return CommandResult(0, b'{"schema":"fixture_v1"}', b"", 1)

        resumed = ProjectionCoordinator(
            self.config,
            steps=self.steps,
            runner=successful,
        ).run(
            actor="test",
            run_id="resumed",
            phase="manual",
            resume_generation=failed_generation,
        )
        self.assertEqual(resumed["reused_step_count"], 1)
        self.assertEqual(resumed["executed_step_count"], 1)
        self.assertEqual(len(resumed_calls), 1)
        self.assertEqual(
            resumed["resume_source_generation_id"],
            failed_generation,
        )

    def test_explain_reports_reuse_without_mutation(self) -> None:
        coordinator = ProjectionCoordinator(
            self.config,
            steps=self.steps,
            runner=self.successful_runner,
        )
        coordinator.run(actor="test", run_id="one", phase="manual")
        latest_before = coordinator.latest_path.read_bytes()
        explanation = coordinator.explain()
        self.assertEqual(
            [step["action"] for step in explanation["steps"]],
            ["reuse", "reuse"],
        )
        self.assertFalse(explanation["mutates"])
        self.assertEqual(coordinator.latest_path.read_bytes(), latest_before)

    def test_session_adapter_runs_pre_and_post_generations(self) -> None:
        fake = FakeCoordinator()
        controller = StewardController(
            self.config,
            projection_coordinator=fake,  # type: ignore[arg-type]
        )
        controller.resume(actor="test", acknowledgement="fixture")
        begun = controller.begin(actor="test")
        finished = controller.finish(
            run_id=begun["run_id"],
            lease_token=begun["lease_token"],
            outcome="success",
        )
        self.assertEqual([call[2] for call in fake.calls], ["pre", "post"])
        self.assertEqual(
            finished["receipt"]["projection_generation_id"],
            "post_2",
        )

    def test_projection_renews_past_lease_ttl_and_returns_fresh_token(self) -> None:
        class SlowCoordinator(FakeCoordinator):
            def run(
                self,
                *,
                actor: str,
                run_id: str,
                phase: str,
                control=None,
                full_rebuild: bool = False,
                resume_generation: str | None = None,
            ) -> dict[str, str]:
                del full_rebuild, resume_generation
                self.calls.append((actor, run_id, phase))
                deadline = time.monotonic() + 2.6
                while time.monotonic() < deadline:
                    if control:
                        control(
                            {
                                "generation_id": "slow_generation",
                                "step_id": "slow_step",
                                "command_id": "fixture",
                            }
                        )
                    time.sleep(0.1)
                return {
                    "generation_id": "slow_generation",
                    "status": "passed",
                }

        config = ControlConfig(
            **{
                **self.config.__dict__,
                "lease_ttl_secs": 2,
                "heartbeat_interval_secs": 1,
            }
        )
        controller = StewardController(
            config,
            projection_coordinator=SlowCoordinator(),  # type: ignore[arg-type]
        )
        controller.resume(actor="test", acknowledgement="fixture")
        begun = controller.begin(actor="test")
        self.assertGreater(
            begun["expires_at_unix"] - time.time(),
            config.heartbeat_interval_secs,
        )
        heartbeat = controller.heartbeat(
            run_id=begun["run_id"],
            lease_token=begun["lease_token"],
        )
        self.assertFalse(heartbeat["stop_requested"])
        controller.finish(
            run_id=begun["run_id"],
            lease_token=begun["lease_token"],
            outcome="success",
            project_after=False,
        )

    def test_projection_pause_interrupts_child_once_without_force_kill(self) -> None:
        controller = StewardController(self.config)
        controller.resume(actor="test", acknowledgement="fixture")
        begun = controller.begin(actor="test", project_before=False)
        started = time.monotonic()

        def pause_soon() -> None:
            time.sleep(0.25)
            controller.pause(actor="test", reason="fixture pause")

        thread = threading.Thread(target=pause_soon)
        thread.start()
        result = _default_runner(
            [
                sys.executable,
                "-c",
                (
                    "import signal,time; "
                    "signal.signal(signal.SIGINT, signal.SIG_IGN); "
                    "time.sleep(0.8)"
                ),
            ],
            cwd=self.repo,
            timeout=5,
            poll_callback=lambda progress: controller.heartbeat(
                run_id=begun["run_id"],
                lease_token=begun["lease_token"],
            )["stop_requested"],
            poll_interval_secs=0.05,
        )
        thread.join(timeout=2)
        self.assertTrue(result.cancelled)
        self.assertEqual(result.return_code, 0)
        self.assertGreater(time.monotonic() - started, 0.7)
        controller.finish(
            run_id=begun["run_id"],
            lease_token=begun["lease_token"],
            outcome="cancelled",
            project_after=False,
        )


if __name__ == "__main__":
    unittest.main()
