#!/usr/bin/env python3
"""Synthetic old/new writing preservation. Never opens live authored state.

This helper-level qualification is not a paired-stage or activation receipt.
The caller must separately qualify immutable adapter/bridge release identities.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from qualify_geometry_release import Fixture, SOURCE, file_hashes, require, sha, write_json


def once(f, helper, operation, action):
    revision = f.call(helper, operation="preparation_revision")["revision"]
    request = dict(operation="prepare_once", request_id=operation,
                   expected_revision=revision, action=action)
    return f.call(helper, **request), request


def owner_check(root, owner, old, new):
    f = Fixture(root, owner, old, new)
    f.prepare(old, "SELF_STUDY QUESTION NEW Synthetic preserved question?")
    page = f.prepare(old, f"SELF_STUDY OPEN {SOURCE} 1")
    f.deliver(old, page, "STUDY_NOTE: Synthetic exact question note.\nNEXT: REST")
    start, _ = once(f, old, "old-start", "WRITE START synthetic retained work")
    f.deliver(old, start, "Synthetic exact old passage.\nNEXT: WRITE CONTINUE")
    pending, pending_request = once(f, old, "old-pending", "WRITE CONTINUE")
    reader_before = f.checkpoint()
    before = file_hashes(f.state)
    f.check(f.call(new, **pending_request) == pending, "old pending input replays byte-equivalent")
    f.check(file_hashes(f.state) == before, "replay creates no migration writes")
    f.deliver(new, pending, "Synthetic accepted work across transition.\nNEXT: WRITE FINISH")
    draft_path = f.state / "writing/drafts-v2.json"
    saved = json.loads(draft_path.read_bytes())
    f.check(saved["schema_version"] == 5, "verified delivery migrates to writing schema five")
    f.check(saved["drafts"]["d1"]["parts"] == ["Synthetic exact old passage.", "Synthetic accepted work across transition."],
            "old and new exact prose retained")
    for n, action in enumerate(("WRITE QUESTION Optional question", "WRITE EVIDENCE synthetic evidence",
                                "WRITE STOPPING_POINT Explicit return", "WRITE PROFILE SHORT", "WRITE FINISH")):
        output, request = once(f, new, f"metadata-{n}", action)
        f.check(output["generation_requested"] is False, action + " has no generation")
        current = file_hashes(f.state)
        f.check(f.call(new, **request) == output and file_hashes(f.state) == current,
                action + " retry changes no bytes")
    error = f.prepare(new, "WRITE CONTINUE", reject=True)
    f.check(error["writing_recovery"]["commands"][0] == "WRITE RESUME d1", "finished draft offers explicit resume")
    newer = file_hashes(f.state)
    f.prepare(old, "WRITE LIST", reject=True)
    f.check(file_hashes(f.state) == newer, "older writer refuses downgrade without changing bytes")
    resumed, _ = once(f, new, "resume", "WRITE RESUME d1")
    f.check("Synthetic accepted work across transition." in resumed["text"], "explicit resume retains work")
    park, _ = once(f, new, "park", "WRITE PARK")
    f.check(not park["generation_requested"], "parking schedules no provider")
    f.deliver(new, resumed, "Synthetic late accepted output remains retained.\nNEXT: REST")
    saved = json.loads(draft_path.read_bytes())
    f.check(saved["active"] is None, "late delivery never reselects parked work")
    f.check(saved["drafts"]["d1"]["parts"][-1] == "Synthetic late accepted output remains retained.",
            "parking does not discard accepted output")
    after = f.checkpoint()
    for field in ("questions", "bookmarks", "notebook", "progress"):
        f.check(reader_before[field] == after[field], "writing preserves inquiry " + field)
    f.check(not (f.workspace / "journal").exists(), "no private prose published")
    return dict(owner=owner, checks=f.checks, synthetic_state_hashes=file_hashes(f.state))


def qualify(old, new, out):
    require(old.resolve() != new.resolve(), "distinct old and candidate helpers required")
    identities = dict(old=sha(old), new=sha(new))
    out.mkdir(mode=0o700)
    try:
        owners = [owner_check(out / owner, owner, old, new) for owner in ("astrid", "minime")]
        require(identities == dict(old=sha(old), new=sha(new)), "helper identity drift")
        receipt = dict(schema="writing_continuity_helper_qualification_v1", status="passed",
                       helpers=identities, owners=owners,
                       checks=sum(len(owner["checks"]) for owner in owners),
                       activation_performed=False, paired_release_qualified=False,
                       scope="synthetic owner stores and stubbed provider completions only")
        write_json(out / "qualification.json", receipt)
        return receipt
    except Exception as error:
        write_json(out / "failure.json", dict(error=str(error), activation_performed=False))
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("old-helper", "new-helper", "out"):
        parser.add_argument("--" + arg, type=Path, required=True)
    args = parser.parse_args()
    receipt = qualify(args.old_helper.resolve(strict=True), args.new_helper.resolve(strict=True), args.out.absolute())
    print(json.dumps({key: receipt[key] for key in ("status", "checks", "activation_performed", "paired_release_qualified")}))


if __name__ == "__main__":
    main()
