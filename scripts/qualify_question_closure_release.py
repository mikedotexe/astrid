#!/usr/bin/env python3
"""Qualify schema-13 closure recovery with immutable helpers and synthetic stores.

No service signals, live state or inference calls. Passing grants no authority.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from bridge_stage import verify_stage
from qualify_geometry_release import (
    AUTHORED, PRIVATE, SOURCE, Fixture, file_hashes, invalid_state_probes,
    inventory, preparation_retry_probe, require, sha, verify_launch_binding, write_json,
)
from qualify_source_selection_release import adapter_probe, once


def owner_preservation(root, owner, old, new):
    f = Fixture(root, owner, old, new)
    f.prepare(old, "SELF_STUDY QUESTION NEW Retained historical inquiry?")
    f.prepare(old, "SELF_STUDY QUESTION RESOLVE q1 [finding]")
    historical = f.checkpoint()["questions"]["entries"]["q1"]
    f.check(historical["finding"] == "[finding]", "old helper reproduces literal-template closure")
    f.prepare(old, "SELF_STUDY QUESTION NEW Unresolved synthetic inquiry?")
    page = f.prepare(old, f"SELF_STUDY OPEN {SOURCE} 1")
    f.deliver(old, page, f"STUDY_NOTE: {AUTHORED}\nNEXT: REST")
    draft = f.prepare(old, "WRITE START Synthetic private work")
    f.deliver(old, draft, PRIVATE + "\nNEXT: WRITE CONTINUE")
    pending_draft = f.prepare(old, "WRITE CONTINUE")
    old_drafts = json.loads((f.state / "writing/drafts-v2.json").read_bytes())["drafts"]
    pending, request = once(f, old, "old-pending", "SELF_STUDY CONTINUE")
    f.check(f.checkpoint()["version"] == 13, "old reader produces schema thirteen")
    f.check(f.call(new, **request) == pending, "old pending public input is byte-exact")
    f.check(f.prepare(new, "WRITE CONTINUE") == pending_draft, "old pending private input is byte-exact")
    f.deliver(new, pending, f"{AUTHORED}\nNEXT: SELF_STUDY QUESTION RESOLVE q2 [finding]")
    before = f.checkpoint()
    rejected, retry = once(f, new, "rejected-closure", "SELF_STUDY QUESTION RESOLVE q2 [finding]")
    f.check(rejected["input_kind"] == "recovery" and rejected["continuation_decision"],
            "literal template produces an explicit recovery decision")
    f.check("template marker" in rejected["text"] and "without a finding" in rejected["text"],
            "recovery explains the rejected marker and voluntary valid closure")
    for field in ("questions", "notebook", "current", "bookmarks", "progress"):
        f.check(f.checkpoint()[field] == before[field], f"rejection preserves {field}")
    f.check(f.call(new, **retry) == rejected, "restarted retry returns exact recovery")
    stable = file_hashes(f.state)
    f.call(new, reject=True, **(retry | {"action": "SELF_STUDY QUESTION RESOLVE q2"}))
    f.check(file_hashes(f.state) == stable, "conflicting retry cannot substitute bare closure")
    f.deliver(new, rejected, "NEXT: REST")
    review = f.prepare(new, "SELF_STUDY QUESTION REVIEW q1")
    f.check("template marker" in review["text"], "legacy marker limitation is inspectable")
    f.check(f.checkpoint()["questions"]["entries"]["q1"] == historical,
            "historical status and authored material are not rewritten")
    f.check(json.loads((f.state / "writing/drafts-v2.json").read_bytes())["drafts"] == old_drafts,
            "complete unrelated private drafts are unchanged")
    f.prepare(new, "SELF_STUDY QUESTION RESOLVE q2 I remain uncertain.")
    f.check(f.checkpoint()["questions"]["entries"]["q2"]["finding"] == "I remain uncertain.",
            "explicit uncertain finding remains available")
    f.prepare(new, "SELF_STUDY QUESTION NEW Closure needs no finding?")
    f.prepare(new, "SELF_STUDY QUESTION RESOLVE q3")
    f.check(f.checkpoint()["questions"]["entries"]["q3"]["finding"] == "",
            "explicit closure without a finding remains available")
    f.prepare(new, "SELF_STUDY QUESTION q2")
    resumed = f.prepare(new, "SELF_STUDY CONTINUE")
    f.check(resumed["page"]["start"] == pending["page"]["end"], "explicit return retains the verified cursor")
    f.check(f.checkpoint()["version"] == 13, "repair introduces no state migration")
    f.check(not (f.workspace / "journal").exists(), "helper does not publish public writing")
    return dict(owner=owner, checks=f.checks, state_hashes=file_hashes(f.state))


def qualify(old, new, minime, reconciliation, out):
    stages = {"old": verify_stage(old), "new": verify_stage(new)}
    inputs = inventory(minime)
    binding = verify_launch_binding(reconciliation, inputs)
    out.mkdir(mode=0o700)
    try:
        readers = {name: stage / "helpers/astrid-source-study" for name, stage in (("old", old), ("new", new))}
        owners = [owner_preservation(out / owner, owner, readers["old"], readers["new"])
                  for owner in ("astrid", "minime")]
        adapter = adapter_probe(minime, new, out / "adapter")
        invalid = invalid_state_probes(out, readers["old"], readers["new"])
        retries = preparation_retry_probe(out, readers["new"])
        require(inventory(minime) == inputs, "frozen Minime input drift")
        require(verify_launch_binding(reconciliation, inputs) == binding, "launch binding drift")
        require(stages == {"old": verify_stage(old), "new": verify_stage(new)}, "stage drift")
        receipt = dict(schema="question_closure_paired_qualification_v1", status="passed",
                       stages=stages, readers={name: sha(path) for name, path in readers.items()},
                       selected_inputs=inputs, launch_binding=binding, owners=owners,
                       checks=sum(len(owner["checks"]) for owner in owners), adapter=adapter,
                       invalid_state_probes=invalid, preparation_retries=retries,
                       qualifier_inputs={name: sha(Path(__file__).with_name(name)) for name in
                           (Path(__file__).name, "qualify_geometry_release.py", "qualify_source_selection_release.py", "bridge_stage.py")},
                       activation_performed=False, live_eligible_now=False,
                       rollback_limit="Schema unchanged; older helper remains data-compatible but lacks the closure repair. No rollback performed.",
                       scope="Synthetic state only; no live state, provider or service touched")
        write_json(out / "qualification.json", receipt)
        return receipt
    except Exception as error:
        write_json(out / "failure.json", dict(error=str(error), activation_performed=False))
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("old-stage", "new-stage", "minime-source", "reconciliation", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    result = qualify(args.old_stage.resolve(strict=True), args.new_stage.resolve(strict=True),
                     args.minime_source.resolve(strict=True), args.reconciliation.resolve(strict=True),
                     args.out.absolute())
    print(json.dumps(dict(status=result["status"], checks=result["checks"], receipt=str(args.out / "qualification.json"))))


if __name__ == "__main__":
    main()
