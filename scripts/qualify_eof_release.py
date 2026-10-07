#!/usr/bin/env python3
"""Qualify EOF recovery with immutable helpers, frozen adapters and synthetic state.

No live state, inference or service operations. Passing grants no authority.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

from bridge_stage import verify_stage
from qualify_geometry_release import (
    AUTHORED, PRIVATE, SOURCE, Fixture, file_hashes, invalid_state_probes,
    inventory, preparation_retry_probe, require, sha, verify_launch_binding, write_json,
)
from qualify_source_selection_release import adapter_probe, once


def owner_preservation(root, owner, old, new):
    f = Fixture(root, owner, old, new)
    f.prepare(old, "SELF_STUDY QUESTION NEW Retained synthetic question?")
    page = f.prepare(old, f"SELF_STUDY OPEN {SOURCE} 1")
    f.deliver(old, page, f"STUDY_NOTE: {AUTHORED}\nNEXT: REST")
    draft = f.prepare(old, "WRITE START Synthetic private work")
    f.deliver(old, draft, PRIVATE + "\nNEXT: WRITE CONTINUE")
    pending_draft = f.prepare(old, "WRITE CONTINUE")
    drafts = json.loads((f.state / "writing/drafts-v2.json").read_bytes())
    pending, request = once(f, old, "old-pending", "SELF_STUDY CONTINUE")
    f.check(f.checkpoint()["version"] == 13, "old reader is schema thirteen")
    f.check(f.call(new, **request) == pending, "old public preparation replays byte-exactly")
    f.check(f.prepare(new, "WRITE CONTINUE") == pending_draft, "old private preparation replays byte-exactly")
    f.deliver(new, pending, "Synthetic continuation.\nNEXT: REST")
    f.check(json.loads((f.state / "writing/drafts-v2.json").read_bytes()) == drafts,
            "unrelated drafts and pending private input remain unchanged")
    last = f.prepare(old, f"SELF_STUDY OPEN {SOURCE} 502")
    f.check(last["page"]["eof"], "old reader delivers actual end of source")
    f.deliver(old, last, "NAVIGATION_PROSE\nNEXT: SELF_STUDY CONTINUE")
    old_eof, request = once(f, old, "old-eof", "SELF_STUDY CONTINUE")
    f.check(f.call(new, **request) == old_eof, "retained old EOF input is not rewritten")
    f.deliver(new, old_eof, "NEXT: SELF_STUDY MAP")
    before = f.checkpoint()
    for index, action in enumerate(("SELF_STUDY MAP", "SELF_STUDY CONTINUE", "SELF_STUDY")):
        output, retry = once(f, new, f"new-eof-{index}", action)
        f.check(output["host_feedback_policy"] == "reader_only" and output["page"] is None,
                f"{action} supplies no source and requests no appended host history")
        f.check(all(text not in output["text"] for text in
                    ("NAVIGATION_PROSE", "NAVIGATION RECEIPT", "MAP chooses another source")),
                f"{action} omits automatic navigation replay and misleading command claim")
        f.check(f.call(new, **retry) == output, f"{action} retries byte-exactly")
        stable = file_hashes(f.state)
        f.call(new, reject=True, **(retry | {"action": f"SELF_STUDY OPEN {SOURCE} 1"}))
        f.check(file_hashes(f.state) == stable, f"{action} conflicting retry leaves state intact")
        f.deliver(new, output, "NEXT: REST")
        f.check(all(f.checkpoint()[key] == before[key] for key in ("current", "bookmarks", "progress")),
                f"{action} does not advance source coverage or position")
    f.check(AUTHORED in f.prepare(new, "SELF_STUDY NOTE")["text"], "exact authored note is explicitly retrievable")
    help_page = f.prepare(new, "SELF_STUDY HELP verification")
    f.check(help_page["input_kind"] == "help" and help_page["page"] is None,
            "explicit verification help does not supply source or execute tests")
    reopened = f.prepare(new, f"SELF_STUDY OPEN {SOURCE} 1")
    f.check(reopened["page"]["start"]["line"] == 1, "explicit OPEN deliberately rereads")
    f.deliver(new, reopened, "Synthetic rereading.\nNEXT: REST")
    f.check(f.checkpoint()["version"] == 13, "no schema migration occurs")
    # Exercise old-reader compatibility only on a copy, never over newer records.
    copied = Fixture(root / "old-compatibility", owner, old, new)
    shutil.copytree(f.state, copied.state)
    old_notes = copied.prepare(old, "SELF_STUDY NOTE")
    f.check(AUTHORED in old_notes["text"], "old helper can inspect a copy of new authored state")
    f.check(not (f.workspace / "journal").exists(), "helper does not publish writing")
    return dict(owner=owner, checks=f.checks, state_hashes=file_hashes(f.state))


def eof_adapter_probe(snapshot, stage, root):
    helper = stage / "helpers/astrid-source-study"
    Fixture(root, "minime", helper, helper)
    (root / SOURCE).write_text("pub fn first() {}\n")
    write_json(root / "astrid/.runtime/bridge-deployment/active.json",
               dict(schema="bridge_release_selection_v1", stage=str(stage),
                    manifest_sha256=sha(stage / "manifest.json")))
    code = r'''
import json, pathlib, sys
from minime_autonomy.source_study import StudyClient, selected_reader
root, expected = map(pathlib.Path, sys.argv[1:])
assert selected_reader(root / 'astrid') == expected
client = StudyClient(root / 'minime', root / 'owner-workspace', astrid_root=root / 'astrid')
def deliver(prompt, text):
    class Response:
        status_code = 200
        def __init__(self):
            self.text = json.dumps({'message': {'content': text}, 'done': True})
    prompt.post(lambda *_a, **_kw: Response(), 'synthetic://stub',
                {'messages': [{'role': 'user', 'content': str(prompt)}]}, 1)
    prompt.accepted()
client.prepare('SELF_STUDY QUESTION NEW Synthetic EOF question?', request_id='question')
page = client.prepare('SELF_STUDY OPEN astrid/crates/example/src/lib.rs 1', request_id='page')
deliver(page, 'NAVIGATION_PROSE\nSTUDY_NOTE: Exact retained note.\nNEXT: SELF_STUDY CONTINUE')
for index, action in enumerate(['SELF_STUDY CONTINUE', 'SELF_STUDY MAP'] * 2):
    output = client.prepare(action, request_id='eof-' + str(index))
    assert output.output['host_feedback_policy'] == 'reader_only'
    assert str(output) == output.output['text']
    assert 'NAVIGATION_PROSE' not in output and 'NAVIGATION RECEIPT' not in output
    assert output.output['page'] is None
    deliver(output, 'NEXT: REST')
assert 'Exact retained note.' in client.prepare('SELF_STUDY NOTE', request_id='inspect')
print(json.dumps({'same_helper_selected': True, 'quiet_eof_and_map': True,
                  'explicit_note_preserved': True, 'synthetic_stub_only': True}))
'''
    env = {key: os.environ[key] for key in ("HOME", "PATH", "TMPDIR") if key in os.environ}
    env.update(PYTHONPATH=str(snapshot), PYTHONDONTWRITEBYTECODE="1")
    result = subprocess.run([sys.executable, "-B", "-c", code, str(root), str(helper)],
                            cwd=snapshot, env=env, capture_output=True, text=True, timeout=90)
    require(result.returncode == 0, f"frozen EOF adapter failed: {result.stderr[-2000:]}")
    return json.loads(result.stdout)


def qualify(old, new, minime, reconciliation, out):
    stages = {"old": verify_stage(old), "new": verify_stage(new)}
    inputs = inventory(minime)
    binding = verify_launch_binding(reconciliation, inputs)
    out.mkdir(mode=0o700)
    try:
        readers = {name: stage / "helpers/astrid-source-study" for name, stage in (("old", old), ("new", new))}
        owners = [owner_preservation(out / owner, owner, readers["old"], readers["new"])
                  for owner in ("astrid", "minime")]
        adapters = dict(selection=adapter_probe(minime, new, out / "adapter-selection"),
                        eof=eof_adapter_probe(minime, new, out / "adapter-eof"))
        invalid = invalid_state_probes(out, readers["old"], readers["new"])
        retries = preparation_retry_probe(out, readers["new"])
        require(inventory(minime) == inputs, "frozen Minime input drift")
        require(verify_launch_binding(reconciliation, inputs) == binding, "launch binding drift")
        require(stages == {"old": verify_stage(old), "new": verify_stage(new)}, "stage drift")
        receipt = dict(schema="eof_paired_qualification_v1", status="passed",
                       stages=stages, readers={name: sha(path) for name, path in readers.items()},
                       selected_inputs=inputs, launch_binding=binding, owners=owners,
                       checks=sum(len(owner["checks"]) for owner in owners), adapters=adapters,
                       invalid_state_probes=invalid, preparation_retries=retries,
                       qualifier_inputs={name: sha(Path(__file__).with_name(name)) for name in
                           (Path(__file__).name, "qualify_geometry_release.py", "qualify_source_selection_release.py", "bridge_stage.py")},
                       activation_performed=False, live_eligible_now=False,
                       scope="Synthetic state and stubbed providers only; no live state or services touched")
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
                     args.minime_source.resolve(strict=True), args.reconciliation.resolve(strict=True), args.out.absolute())
    print(json.dumps(dict(status=result["status"], checks=result["checks"], receipt=str(args.out / "qualification.json"))))


if __name__ == "__main__":
    main()
