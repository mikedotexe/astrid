#!/usr/bin/env python3
"""Qualify observation upgrades with staged executables and synthetic owner state.

Never reads live drafts, changes launch selection, or activates services. The frozen
Python inventory is qualification evidence, not permission to install an overlay.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import time

from bridge_stage import verify_stage
from qualify_geometry_release import (
    AUTHORED, PRIVATE, SOURCE, Fixture, file_hashes,
    helper_selection_probe, inventory, require, sha, verify_launch_binding, write_json,
)
from reconcile_minime_launch import ASSETS, checked_copy


def freeze_adapter(source, destination):
    inputs = inventory(source)
    assets = {name: sha(source / name) for name in ASSETS}
    destination.mkdir(mode=0o700)
    for name, digest in {**inputs, **assets}.items():
        checked_copy(source / name, destination / name, digest)
    require(inputs == inventory(source) == inventory(destination), "Python source drift")
    require(all(sha(source / name) == digest == sha(destination / name)
                for name, digest in assets.items()), "adapter asset drift")
    for directory in sorted((p for p in destination.rglob("*") if p.is_dir()), reverse=True):
        directory.chmod(0o555)
    destination.chmod(0o555)
    return inputs, assets


class ObservationMigration(Fixture):
    def prepare(self, executable, action, **options):
        revision = self.call(executable, operation="preparation_revision")["revision"]
        number = getattr(self, "operation_number", 0) + 1
        self.operation_number = number
        return self.call(executable, operation="prepare_once", action=action,
                         request_id=f"migration-{number}", expected_revision=revision, **options)

    def drafts(self):
        return json.loads((self.state / "writing/drafts-v2.json").read_bytes())

    def run(self):
        self.prepare(self.old, "SELF_STUDY QUESTION NEW Synthetic observation migration?")
        opened = self.prepare(self.old, f"SELF_STUDY OPEN {SOURCE} 1")
        self.deliver(self.old, opened, f"STUDY_NOTE: {AUTHORED}")
        draft = self.prepare(self.old, "WRITE START synthetic private interest")
        self.deliver(self.old, draft, PRIVATE + "\nNEXT: WRITE CONTINUE")
        pending_draft = self.prepare(self.old, "WRITE CONTINUE")
        pending_page = self.prepare(self.old, "SELF_STUDY CONTINUE")
        legacy, old_drafts = self.checkpoint(), self.drafts()
        self.check(legacy["version"] == 6, "actual old helper creates reader schema six")
        self.check(old_drafts["schema_version"] == 3, "actual old helper creates writer schema three")
        original = file_hashes(self.state)
        shutil.copytree(self.state, self.root / "legacy-reader-copy")
        restored = self.prepare(self.new, "SELF_STUDY CONTINUE")
        self.check(restored == pending_page, "exact pending source input survives upgrade")
        current = self.checkpoint()
        self.check(current["version"] == 7, "candidate commits reader schema seven")
        for key, inquiry in legacy["questions"]["entries"].items():
            self.check(all(current["questions"]["entries"][key][field] == value
                           for field, value in inquiry.items()), "all old inquiry fields retained")
            self.check(current["questions"]["entries"][key]["observations"]["records"] == [],
                       "migration infers no observations")
        self.check(not (self.state / "activity-focus-v1.json").exists(), "no focus window created")
        writing = self.prepare(self.new, "WRITE CONTINUE")
        self.check(writing == pending_draft, "exact pending private input survives upgrade")
        self.deliver(self.new, writing, "Synthetic retained continuation.\nNEXT: WRITE CONTINUE")
        self.check(self.drafts()["schema_version"] == 4, "candidate commits writer schema four")
        self.check(self.drafts()["drafts"]["d1"]["parts"][0] ==
                   old_drafts["drafts"]["d1"]["parts"][0], "original private prose remains exact")
        for action in ("SELF_STUDY MAP", "WRITE CONTINUE"):
            before = file_hashes(self.state)
            self.call(self.old, operation="prepare", action=action, reject=True)
            self.check(file_hashes(self.state) == before, f"old helper refuses {action} without writes")
        status = self.prepare(self.new, "WRITE OBSERVE " + json.dumps(dict(
            owner=self.owner, draft="d1", operation=dict(kind="status"))))
        self.check(status["generation_requested"] is False, "status creates no generation")
        revision = status["text"].splitlines()[0].split(" revision ")[1]
        now = int(time.time() * 1000)
        write_json(self.root / "minime/workspace/runtime/esn_activation_trace_v1.json", dict(
            policy="esn_activation_trace_v1", reservoir_dim=128, sample_interval_ms=1000,
            retained_secs=180, updated_at_unix_ms=now,
            frames=[dict(t_ms=i * 1000, wall_clock_unix_ms=now - (79-i)*1000,
                         summary=dict(finite_fraction=1.0),
                         activations=[0.25 if i % 8 < 4 else -0.25] * 128) for i in range(80)]))
        before_inquiry = self.checkpoint()["questions"]
        capture = self.prepare(self.new, "WRITE OBSERVE " + json.dumps(dict(
            owner=self.owner, draft="d1", revision=revision, request_id="capture",
            expected_head="empty", operation=dict(kind="capture", seconds=180))))
        self.check(capture["generation_requested"] is False, "capture is storage only")
        self.check(before_inquiry == self.checkpoint()["questions"], "capture does not mutate inquiries")
        self.check(len(self.drafts()["drafts"]["d1"]["observations"]["records"]) == 1,
                   "exactly one voluntary attachment retained")
        self.prepare(self.new, "WRITE PARK")
        other = self.prepare(self.new, "WRITE START unrelated synthetic work")
        self.check("source_sha256" not in other["text"], "parked attachment is not ambient context")
        restored = self.prepare(self.new, "WRITE RESUME d1")
        self.check(PRIVATE in restored["text"] and "activations" not in restored["text"],
                   "return preserves prose with compact attachment only")
        self.check(file_hashes(self.root / "legacy-reader-copy") == original,
                   "legacy fixture archive untouched; no rollback restore")
        self.check(not (self.workspace / "journal").exists(), "no public journal created")
        return dict(owner=self.owner, checks=self.checks, reader_version=7, writer_version=4,
                    fixture_only=True, reader_files=file_hashes(self.state))


def qualify(old_stage, stage, minime, output, reconciliation=None):
    old_ready, ready = verify_stage(old_stage), verify_stage(stage)
    output.mkdir(parents=True, exist_ok=False, mode=0o700)
    try:
        tools = {}
        for name in ("qualify_observation_release.py", "qualify_geometry_release.py",
                     "reconcile_minime_launch.py", "bridge_stage.py"):
            source = Path(__file__).with_name(name)
            target = output / name
            shutil.copyfile(source, target)
            target.chmod(0o444)
            tools[name] = sha(target)
        inputs, assets = freeze_adapter(minime, output / "minime-source")
        launch_binding = verify_launch_binding(reconciliation, inputs) if reconciliation else None
        owners = [ObservationMigration(output / f"fixture-{owner}", owner,
                  old_stage / "helpers/astrid-source-study",
                  stage / "helpers/astrid-source-study").run() for owner in ("astrid", "minime")]
        adapter = helper_selection_probe(output / "minime-source", stage, output / "fixture-minime")
        require(inputs == inventory(minime) == inventory(output / "minime-source"),
                "Python source drift during qualification")
        require(all(sha(minime / name) == digest == sha(output / "minime-source" / name)
                    for name, digest in assets.items()), "adapter asset drift during qualification")
        require(verify_stage(old_stage) == old_ready and verify_stage(stage) == ready,
                "staged release drift during qualification")
        receipt = dict(schema="observation_paired_qualification_v1", status="offline_passed_not_activated",
                       old_bridge=old_ready, bridge=ready, owners=owners, adapter=adapter,
                       minime_source_root=str(minime), minime_snapshot=str(output / "minime-source"),
                       minime_inputs=inputs, minime_assets=assets, launch_binding=launch_binding,
                       helper_sha256=sha(stage / "helpers/astrid-source-study"),
                       qualification_tools=tools, activation_performed=False, live_eligible_now=False,
                       activation_blockers=["review exact canonical launch-source reconciliation",
                                            "cooperative preflight and explicit paired rollout approval"],
                       boundaries=["synthetic state only", "no engine/model/visual/sensory restart",
                                   "new schemas require forward-compatible rollback, never restore old state",
                                   "Python snapshot is not a launch selector"])
        write_json(output / "qualification.json", receipt)
        return receipt
    except Exception as error:
        write_json(output / "failure.json", dict(error=str(error), activation_performed=False))
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--old-bridge-stage", type=Path, required=True)
    parser.add_argument("--bridge-stage", type=Path, required=True)
    parser.add_argument("--minime-source", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--reconciliation", type=Path)
    args = parser.parse_args()
    receipt = qualify(args.old_bridge_stage.resolve(strict=True), args.bridge_stage.resolve(strict=True),
                      args.minime_source.resolve(strict=True), args.out.absolute(),
                      args.reconciliation.resolve(strict=True) if args.reconciliation else None)
    print(json.dumps(dict(status=receipt["status"], checks=sum(len(o["checks"]) for o in receipt["owners"]),
                          receipt=str(args.out / "qualification.json"), activation_performed=False)))


if __name__ == "__main__":
    main()
