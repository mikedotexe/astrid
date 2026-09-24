"""Synthetic schema 7-to-8 qualification using actual old/new release helpers."""
import argparse
import json
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from bridge_stage import verify_stage
from qualify_geometry_release import (
    AUTHORED, PRIVATE, SOURCE, Fixture, file_hashes, helper_selection_probe,
    inventory, invalid_state_probes, preparation_retry_probe, require, sha,
    verify_launch_binding, write_json,
)
from qualify_observation_release import freeze_adapter


class Upgrade(Fixture):
    def prepare(self, executable, action, **options):
        revision = self.call(executable, operation="preparation_revision")["revision"]
        self.number = getattr(self, "number", 0) + 1
        return self.call(executable, operation="prepare_once", action=action,
                         request_id=f"upgrade-{self.number}", expected_revision=revision, **options)

    def deliver(self, executable, output, text):
        page = output.get("page")
        return self.call(executable,
            operation="delivered" if page else "navigation_delivered",
            **({"page_id": page["id"]} if page else {"navigation_id": output["navigation_id"]}),
            request_json=json.dumps({"messages": [
                {"role": "system", "content": output["system_prompt"]},
                {"role": "user", "content": output["text"]}]}),
            response_json=json.dumps({"message": {"content": text}, "done": True,
                                      "done_reason": "stop"}))

    def run(self):
        self.prepare(self.old, "SELF_STUDY QUESTION NEW Synthetic failure-path question?")
        opened = self.prepare(self.old, f"SELF_STUDY OPEN {SOURCE} 1")
        self.deliver(self.old, opened, f"STUDY_NOTE: {AUTHORED}\n"
                     f"STUDY_RELATION: hypothesis | {SOURCE}:1 | {SOURCE}:2 | Synthetic possibility.")
        draft = self.prepare(self.old, "WRITE START synthetic private continuity")
        self.deliver(self.old, draft, PRIVATE + "\nNEXT: WRITE CONTINUE")
        pending_draft = self.prepare(self.old, "WRITE CONTINUE")
        pending_page = self.prepare(self.old, "SELF_STUDY CONTINUE")
        legacy = self.checkpoint()
        self.check(legacy["version"] == 7, "actual old helper creates schema seven")
        saved = file_hashes(self.state)
        shutil.copytree(self.state, self.root / "legacy-copy")
        restored = self.prepare(self.new, "SELF_STUDY CONTINUE")
        self.check(restored == pending_page, "complete old pending source input preserved")
        current = self.checkpoint()
        self.check(current["version"] == 8, "actual new helper commits schema eight")
        self.check(current["questions"] == legacy["questions"], "all authored inquiry fields preserved")
        self.check(current["notebook"] == legacy["notebook"], "no invented note revision history")
        self.check(not (self.state / "activity-focus-v1.json").exists(), "no focus window created")
        writing = self.prepare(self.new, "WRITE CONTINUE")
        self.check(writing == pending_draft, "complete old pending private input preserved")
        self.deliver(self.new, writing, "Synthetic continuation.\nNEXT: WRITE CONTINUE")
        before = file_hashes(self.state)
        self.call(self.old, operation="prepare", action="SELF_STUDY MAP", reject=True)
        self.check(file_hashes(self.state) == before, "old source reader refuses schema eight without writes")
        # Private drafts keep their separate, unchanged schema. Compatibility
        # there is not permission to downgrade or restore the source notebook.
        reader_bytes = (self.state / "reader-v1.json").read_bytes()
        old_writing = self.call(self.old, operation="prepare", action="WRITE CONTINUE")
        self.check(old_writing["input_kind"] == "private_writing" and
                   (self.state / "reader-v1.json").read_bytes() == reader_bytes,
                   "compatible private writer leaves upgraded source notebook untouched")
        # Deliver the old prepared page before requesting a new one: migration
        # must not retroactively rewrite material already offered to a provider.
        restored = self.prepare(self.new, "SELF_STUDY CONTINUE")
        self.deliver(self.new, restored, "Synthetic source continuation.")
        before = self.checkpoint()
        reflection = self.prepare(self.new, "INTROSPECT")
        self.check(reflection["input_kind"] == "reflection" and not reflection.get("page"),
                   "explicit reflection is not a source page")
        self.check(AUTHORED not in reflection["text"] and PRIVATE not in reflection["text"],
                   "reflection excludes saved note and private draft")
        self.deliver(self.new, reflection, "Synthetic reflection.\nSTUDY_NOTE: Not an update.\nNEXT: REST")
        after = self.checkpoint()
        self.check(all(after[k] == before[k] for k in ("notebook", "questions", "bookmarks", "progress")),
                   "reflection leaves notebook and source progress unchanged")
        page = self.prepare(self.new, f"SELF_STUDY OPEN {SOURCE} 1")
        self.check(AUTHORED not in page["text"], "new source offer omits saved note body")
        prior = self.checkpoint()["notebook"]["note"]
        revision = dict(prior=prior["response_sha256"], text="Synthetic revised account remains tentative.",
                        source=SOURCE, line=2)
        self.deliver(self.new, page, "STUDY_REVISE: " + json.dumps(revision))
        history = self.checkpoint()["notebook"]["note_history"]
        self.check(len(history) == 1 and history[0]["previous"] == prior,
                   "first new revision preserves exact migrated account")
        self.check(history[0]["counterevidence"]["page_id"] == page["page"]["id"],
                   "counterevidence bound to actual supplied source page")
        self.deliver(self.new, page, "STUDY_REVISE: " + json.dumps(revision))
        self.check(self.checkpoint()["notebook"]["note_history"] == history, "duplicate delivery is idempotent")
        note = self.prepare(self.new, "SELF_STUDY NOTE")
        self.check(AUTHORED in note["text"] and revision["text"] in note["text"],
                   "explicit note view retains original and revised accounts")
        relation = json.loads((self.state / "source-findings-v1.json").read_bytes())
        self.check(relation["schema"] == "source_findings_sidecar_v2", "relation downgrade floor retained")
        self.check(file_hashes(self.root / "legacy-copy") == saved, "legacy archive untouched")
        self.check(not (self.workspace / "journal").exists(), "helper creates no public journal")
        return dict(owner=self.owner, checks=self.checks, state_hashes=file_hashes(self.state))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("old_stage", "stage", "minime", "reconciliation", "out"):
        parser.add_argument("--" + name.replace("_", "-"), type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(mode=0o700)
    try:
        old, new = verify_stage(args.old_stage), verify_stage(args.stage)
        inputs, assets = freeze_adapter(args.minime, args.out / "minime-source")
        launch = verify_launch_binding(args.reconciliation, inputs)
        owners = [Upgrade(args.out / f"fixture-{owner}", owner,
                         args.old_stage / "helpers/astrid-source-study",
                         args.stage / "helpers/astrid-source-study").run()
                  for owner in ("astrid", "minime")]
        adapter = helper_selection_probe(args.out / "minime-source", args.stage, args.out / "fixture-minime")
        invalid = invalid_state_probes(args.out, args.old_stage / "helpers/astrid-source-study",
                                       args.stage / "helpers/astrid-source-study")
        retries = preparation_retry_probe(args.out, args.stage / "helpers/astrid-source-study")
        require(inventory(args.minime) == inputs == inventory(args.out / "minime-source"), "adapter drift")
        require(verify_stage(args.old_stage) == old and verify_stage(args.stage) == new, "release drift")
        result = dict(schema="source_counterevidence_upgrade_v1", status="offline_passed",
                      old_stage=old, stage=new, owners=owners, adapter=adapter, invalid=invalid,
                      retries=retries, launch_binding=launch, minime_inputs=inputs, assets=assets,
                      qualifier_sha256=sha(Path(__file__)), live_state_opened=False,
                      provider_called=False, activation_performed=False)
        write_json(args.out / "qualification.json", result)
        print(json.dumps(dict(status=result["status"], owner_checks=sum(len(o["checks"]) for o in owners),
                              receipt=str(args.out / "qualification.json"))))
    except Exception as error:
        write_json(args.out / "failure.json", dict(error=str(error), activation_performed=False))
        raise


if __name__ == "__main__":
    main()
