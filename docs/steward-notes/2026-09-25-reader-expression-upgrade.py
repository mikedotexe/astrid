"""Synthetic schema 9-to-10 qualification of pending work and authored history."""
import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
spec = importlib.util.spec_from_file_location("prior_upgrade", ROOT / "docs/steward-notes/2026-09-24-reader-upgrade.py")
prior = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prior)


class Upgrade(prior.Upgrade):
    def run(self):
        self.prepare(self.old, "SELF_STUDY QUESTION NEW Synthetic lifecycle question?")
        page = self.prepare(self.old, f"SELF_STUDY OPEN {prior.SOURCE} 1")
        self.deliver(self.old, page, f"STUDY_NOTE: {prior.AUTHORED}")
        revision_page = self.prepare(self.old, f"SELF_STUDY OPEN {prior.SOURCE} 1")
        original = self.checkpoint()["notebook"]["note"]
        revision = dict(prior=original["response_sha256"], text="Synthetic contrary account.", source=prior.SOURCE, line=2)
        self.deliver(self.old, revision_page, "STUDY_REVISE: " + json.dumps(revision))
        draft = self.prepare(self.old, "WRITE START synthetic private continuity")
        self.deliver(self.old, draft, prior.PRIVATE + "\nNEXT: WRITE CONTINUE")
        pending_draft = self.prepare(self.old, "WRITE CONTINUE")
        pending_page = self.prepare(self.old, "SELF_STUDY CONTINUE")
        legacy = self.checkpoint()
        self.check(legacy["version"] == 9, "actual live helper creates schema nine")
        saved = prior.file_hashes(self.state)
        shutil.copytree(self.state, self.root / "legacy-copy")
        restored = self.prepare(self.new, "SELF_STUDY CONTINUE")
        self.check(restored == pending_page, "complete old pending source input preserved")
        current = self.checkpoint()
        self.check(current["version"] == 10, "new release commits schema ten")
        for key in ("questions", "notebook", "bookmarks", "progress"):
            self.check(current[key] == legacy[key], f"migration preserves {key}")
        self.check(not (self.state / "activity-focus-v1.json").exists(), "no focus window created")
        writing = self.prepare(self.new, "WRITE CONTINUE")
        self.check(writing == pending_draft, "complete pending private input preserved")
        self.deliver(self.new, writing, "Synthetic continuation.\nNEXT: WRITE CONTINUE")
        before = prior.file_hashes(self.state)
        self.call(self.old, operation="prepare", action="SELF_STUDY MAP", reject=True)
        self.check(prior.file_hashes(self.state) == before, "old reader refuses schema ten without writes")
        self.deliver(self.new, restored, "Synthetic source continuation.")
        self.prepare(self.new, "SELF_STUDY QUESTION PARK q1")
        other = self.prepare(self.new, "SELF_STUDY QUESTION NEW Independent second question?")
        self.deliver(self.new, other, "STUDY_NOTE: Second inquiry note.")
        pending = self.prepare(self.new, f"SELF_STUDY OPEN {prior.SOURCE} 20")
        before = self.checkpoint()
        review = self.prepare(self.new, "SELF_STUDY QUESTION REVIEW q1")
        self.check(review["input_kind"] == "inquiry_review" and not review.get("page"), "detached review input kind")
        for text in (prior.AUTHORED, revision["text"], "parked", "authored_revision", "historical_source_references"):
            self.check(text in review["text"], "review retains requested authored history: " + text)
        for text in (prior.PRIVATE, "Independent second question?", "Second inquiry note.", "ACTIVE STUDY QUESTION", "RECALLED ACCOUNT"):
            self.check(text not in review["text"], "review excludes unrelated/private context: " + text)
        self.deliver(self.new, review, "STUDY_NOTE: Not a notebook update.\nSTUDY_QUESTION: Not a question replacement.\nNEXT: REST")
        after = self.checkpoint()
        for key in ("questions", "notebook", "bookmarks", "progress", "pending"):
            self.check(after[key] == before[key], f"review response cannot mutate {key}")
        self.check(self.prepare(self.new, "SELF_STUDY CONTINUE")["page"] == pending["page"], "active inquiry resumes its own exact pending page")
        self.check(prior.file_hashes(self.root / "legacy-copy") == saved, "legacy archive untouched")
        self.check(not (self.workspace / "journal").exists(), "no automatic public journal")
        # The inherited geometry/helper-selection probe explicitly targets q1.
        self.prepare(self.new, "SELF_STUDY QUESTION q1")
        self.check(self.checkpoint()["questions"]["active"] == "q1", "explicit selection restores requested inquiry after detached review")
        return dict(owner=self.owner, checks=self.checks, state_hashes=prior.file_hashes(self.state))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("old_stage", "stage", "minime", "reconciliation", "out"):
        parser.add_argument("--" + name.replace("_", "-"), type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(mode=0o700)
    try:
        old, new = prior.verify_stage(args.old_stage), prior.verify_stage(args.stage)
        inputs, assets = prior.freeze_adapter(args.minime, args.out / "minime-source")
        launch = prior.verify_launch_binding(args.reconciliation, inputs)
        owners = [Upgrade(args.out / f"fixture-{owner}", owner, args.old_stage / "helpers/astrid-source-study", args.stage / "helpers/astrid-source-study").run() for owner in ("astrid", "minime")]
        adapter = prior.helper_selection_probe(args.out / "minime-source", args.stage, args.out / "fixture-minime")
        invalid = prior.invalid_state_probes(args.out, args.old_stage / "helpers/astrid-source-study", args.stage / "helpers/astrid-source-study")
        retries = prior.preparation_retry_probe(args.out, args.stage / "helpers/astrid-source-study")
        prior.require(prior.inventory(args.minime) == inputs == prior.inventory(args.out / "minime-source"), "adapter drift")
        prior.require(prior.verify_stage(args.old_stage) == old and prior.verify_stage(args.stage) == new, "release drift")
        result = dict(schema="reader_expression_upgrade_v1", status="offline_passed", old_stage=old, stage=new, owners=owners, adapter=adapter, invalid=invalid, retries=retries, launch_binding=launch, minime_inputs=inputs, assets=assets, qualifier_sha256=prior.sha(Path(__file__)), live_state_opened=False, provider_called=False, activation_performed=False)
        prior.write_json(args.out / "qualification.json", result)
        print(json.dumps(dict(status=result["status"], owner_checks=sum(len(o["checks"]) for o in owners), receipt=str(args.out / "qualification.json"))))
    except Exception as error:
        prior.write_json(args.out / "failure.json", dict(error=str(error), activation_performed=False))
        raise


if __name__ == "__main__":
    main()
