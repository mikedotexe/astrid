#!/usr/bin/env python3
"""Tests for source-first projection generation coordination."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
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

    def run(self, *, actor: str, run_id: str, phase: str) -> dict[str, str]:
        self.calls.append((actor, run_id, phase))
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

    def test_generation_runs_in_dependency_order_and_writes_v2_checkpoints(
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
            "evidence_event_projection_checkpoint_v2",
        )
        self.assertEqual(checkpoint["input_streams"], ["addressing"])

    def test_builtin_profile_declares_complete_source_first_order(self) -> None:
        self.assertEqual(
            [step.step_id for step in source_first_steps()],
            [
                "addressing",
                "sandbox",
                "corridor",
                "signal_spine",
                "claim_families",
                "experiment_dossiers",
                "authority_temporal",
                "model_qos",
                "felt_contracts",
            ],
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
            failed.run(actor="test", run_id="two", phase="manual")
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


if __name__ == "__main__":
    unittest.main()
