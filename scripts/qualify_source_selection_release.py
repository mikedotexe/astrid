#!/usr/bin/env python3
"""Qualify schema-13 source selection using staged binaries and synthetic state.

No live checkpoint, private writing, provider or service is used. Failed attempts
retain their fixture evidence. Passing does not grant activation authority.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

from bridge_stage import verify_stage
from qualify_geometry_release import (
    AUTHORED, PRIVATE, SOURCE, Fixture, file_hashes, invalid_state_probes,
    inventory, preparation_retry_probe, require, sha, verify_launch_binding, write_json,
)


def once(fixture, executable, request_id, action):
    revision = fixture.call(executable, operation="preparation_revision")["revision"]
    request = dict(operation="prepare_once", request_id=request_id,
                   expected_revision=revision, action=action)
    return fixture.call(executable, **request), request


def owner_migration(root, owner, old, new):
    f = Fixture(root, owner, old, new)
    f.prepare(old, "SELF_STUDY QUESTION NEW Original synthetic inquiry?")
    page = f.prepare(old, f"SELF_STUDY OPEN {SOURCE} 1")
    f.deliver(old, page, f"STUDY_NOTE: {AUTHORED}\nNEXT: REST")
    draft = f.prepare(old, "WRITE START Synthetic private work")
    f.deliver(old, draft, PRIVATE + "\nNEXT: WRITE CONTINUE")
    pending_draft = f.prepare(old, "WRITE CONTINUE")
    old_drafts = json.loads((f.state / "writing/drafts-v2.json").read_bytes())["drafts"]
    pending, request = once(f, old, "old-pending", "SELF_STUDY CONTINUE")
    before = f.checkpoint()
    f.check(before["version"] == 12, "old executable produces genuine schema twelve")
    f.check(f.call(new, **request) == pending, "old pending preparation replays byte-exact input")
    f.check(f.prepare(new, "WRITE CONTINUE") == pending_draft, "private pending input remains exact")
    f.deliver(new, pending, "Synthetic second source page.\nNEXT: REST")
    f.check(f.checkpoint()["version"] == 13, "verified delivery commits schema thirteen")
    f.check(json.loads((f.state / "writing/drafts-v2.json").read_bytes())["drafts"] == old_drafts,
            "migration preserves complete unrelated authored drafts")
    protected = file_hashes(f.state)
    f.prepare(old, "SELF_STUDY MAP", reject=True)
    f.check(file_hashes(f.state) == protected, "old executable refuses downgrade without rewriting history")
    f.prepare(new, "SELF_STUDY QUESTION NEW No saved position?")
    search = f.prepare(new, "SELF_STUDY FIND first")
    opening = f"SELF_STUDY OPEN {SOURCE} 1"
    f.check(opening in search["text"], "exact synthetic opening was actually offered")
    f.deliver(new, search, f"RETAINED_NAVIGATION_PROSE\n{opening}\nNEXT: SELF_STUDY CONTINUE")
    before_recovery = f.checkpoint()
    recovery, retry = once(f, new, "explicit-recovery", "SELF_STUDY CONTINUE")
    f.check(recovery["input_kind"] == "source_selection_needed" and recovery["continuation_decision"],
            "bookmarkless continuation is an explicit decision")
    f.check(opening in recovery["text"] and "was not selected" in recovery["text"],
            "retained conflict is an inert opening hint")
    f.check("RETAINED_NAVIGATION_PROSE" not in recovery["text"]
            and "NAVIGATION RECEIPT" not in recovery["text"], "recovery presentation is quiet")
    f.check(f.call(new, **retry) == recovery, "restarted recovery replays one exact operation")
    stable = file_hashes(f.state)
    f.call(new, reject=True, **(retry | {"action": opening}))
    f.check(file_hashes(f.state) == stable, "conflicting retry preserves native state")
    f.deliver(new, recovery, "NEXT: REST")
    for field in ("current", "bookmarks", "progress"):
        f.check(f.checkpoint()[field] == before_recovery[field], f"recovery does not change {field}")
    selected = f.prepare(new, opening)
    f.check(selected["page"] is not None, "only explicit OPEN selects a page")
    f.deliver(new, selected, "Synthetic chosen finding.\nNEXT: REST")
    f.prepare(new, "SELF_STUDY QUESTION q1")
    resumed = f.prepare(new, "SELF_STUDY CONTINUE")
    f.check(resumed["page"]["start"] == pending["page"]["end"],
            "return resumes original inquiry cursor, not another inquiry's position")
    f.check(not (f.workspace / "journal").exists(), "helper creates no automatic public journal")
    return dict(owner=owner, checks=f.checks, state_hashes=file_hashes(f.state))


def adapter_probe(snapshot, stage, root):
    helper = stage / "helpers/astrid-source-study"
    Fixture(root, "minime", helper, helper)
    write_json(root / "astrid/.runtime/bridge-deployment/active.json",
               dict(schema="bridge_release_selection_v1", stage=str(stage),
                    manifest_sha256=sha(stage / "manifest.json")))
    code = r'''
import json, pathlib, sys
from minime_autonomy.source_study import StudyClient, selected_reader
from minime_autonomy.study_feedback import StudyFeedback
root, expected = map(pathlib.Path, sys.argv[1:])
assert selected_reader(root / 'astrid') == expected
client = StudyClient(root / 'minime', root / 'owner-workspace', astrid_root=root / 'astrid')
def deliver(prompt, text):
    class Response:
        status_code = 200
        def __init__(self):
            self.text = json.dumps({'message': {'content': text}, 'done': True})
    prompt.post(lambda *_args, **_kwargs: Response(), 'synthetic://stub',
                {'messages': [{'role': 'user', 'content': str(prompt)}]}, 1)
    prompt.accepted()
client.prepare('SELF_STUDY QUESTION NEW Synthetic selected question?', request_id='question')
search = client.prepare('SELF_STUDY FIND first', request_id='search')
opening = 'SELF_STUDY OPEN astrid/crates/example/src/lib.rs 1'
assert opening in search
deliver(search, 'RETAINED_NAVIGATION_PROSE\n' + opening + '\nNEXT: SELF_STUDY CONTINUE')
for index in range(3):
    recovery = client.prepare('SELF_STUDY CONTINUE', request_id='recovery-' + str(index))
    assert recovery.output['input_kind'] == 'source_selection_needed'
    assert recovery.output['continuation_decision'] is True
    assert str(recovery) == recovery.output['text']
    assert opening in recovery and 'RETAINED_NAVIGATION_PROSE' not in recovery
    deliver(recovery, 'NEXT: SELF_STUDY CONTINUE')
selected = client.prepare(opening, request_id='selected')
assert selected.output['page'] and not selected.output.get('continuation_decision', False)
deliver(selected, 'Exact synthetic finding.\nNEXT: REST')
assert selected.receipt
print(json.dumps({'same_helper_selected': True, 'recovery_without_host_replay': True,
                  'explicit_source_delivered': True, 'synthetic_stub_only': True}))
'''
    env = {key: os.environ[key] for key in ("HOME", "PATH", "TMPDIR") if key in os.environ}
    env.update(PYTHONPATH=str(snapshot), PYTHONDONTWRITEBYTECODE="1")
    result = subprocess.run([sys.executable, "-B", "-c", code, str(root), str(helper)],
                            cwd=snapshot, env=env, capture_output=True, text=True, timeout=90)
    require(result.returncode == 0, f"frozen adapter failed: {result.stderr[-2000:]}")
    return json.loads(result.stdout)


def qualify(old, new, minime, reconciliation, out):
    stages = {"old": verify_stage(old), "new": verify_stage(new)}
    inputs = inventory(minime)
    binding = verify_launch_binding(reconciliation, inputs)
    out.mkdir(mode=0o700)
    try:
        readers = {name: stage / "helpers/astrid-source-study" for name, stage in (("old", old), ("new", new))}
        owners = [owner_migration(out / owner, owner, readers["old"], readers["new"])
                  for owner in ("astrid", "minime")]
        adapter = adapter_probe(minime, new, out / "adapter")
        invalid = invalid_state_probes(out, readers["old"], readers["new"])
        retries = preparation_retry_probe(out, readers["new"])
        require(inventory(minime) == inputs, "frozen Minime input drift")
        require(verify_launch_binding(reconciliation, inputs) == binding, "launch binding drift")
        require(stages == {"old": verify_stage(old), "new": verify_stage(new)}, "stage drift")
        receipt = dict(schema="source_selection_paired_qualification_v1", status="passed",
                       stages=stages, readers={name: sha(path) for name, path in readers.items()},
                       selected_inputs=inputs, launch_binding=binding,
                       checks=sum(len(owner["checks"]) for owner in owners), owners=owners,
                       adapter=adapter, invalid_state_probes=invalid, preparation_retries=retries,
                       qualifier_inputs={name: sha(Path(__file__).with_name(name)) for name in
                           (Path(__file__).name, "qualify_geometry_release.py", "bridge_stage.py")},
                       activation_performed=False, live_eligible_now=False,
                       scope="Synthetic state and stubbed providers; no live state or services touched")
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
    print(json.dumps(dict(status=result["status"], checks=result["checks"],
                          receipt=str(args.out / "qualification.json"), activation_performed=False)))


if __name__ == "__main__":
    main()
