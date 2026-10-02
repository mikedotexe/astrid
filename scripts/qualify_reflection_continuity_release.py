#!/usr/bin/env python3
"""Qualify explicit reflection continuation with actual old/new staged helpers.

All prose, checkpoints and provider responses are synthetic. No service or live
writing is opened, replaced or restored; a passing receipt grants no activation.
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
from qualify_reflection_release import qualify as qualify_baseline


def once(fixture, executable, request_id, action):
    revision = fixture.call(executable, operation="preparation_revision")["revision"]
    request = dict(operation="prepare_once", request_id=request_id,
                   expected_revision=revision, action=action)
    return fixture.call(executable, **request), request


def drafts(fixture):
    return json.loads((fixture.state / "writing/drafts-v2.json").read_bytes())


def owner_continuity(root, owner, old, new):
    f = Fixture(root, owner, old, new)
    f.prepare(old, "SELF_STUDY QUESTION NEW Synthetic original inquiry?")
    page = f.prepare(old, f"SELF_STUDY OPEN {SOURCE} 1")
    f.deliver(old, page, f"STUDY_NOTE: {AUTHORED}\nNEXT: INTROSPECT")
    first = f.prepare(old, "WRITE START Synthetic existing private draft")
    f.deliver(old, first, PRIVATE + "\nNEXT: WRITE PARK")
    f.prepare(old, "WRITE PARK")
    before = f.checkpoint()
    old_draft = drafts(f)["drafts"]["d1"]

    reflection, pending_request = once(f, old, "old-pending-reflection", "INTROSPECT")
    f.check(f.call(new, **pending_request) == reflection,
            "old pending reflection input and identity replay exactly after transition")
    passage = "Synthetic reflection: an exact \u03bb passage, not an inferred private thought."
    action = f"WRITE FROM_REFLECTION {reflection['navigation_id']}"
    receipt = f.deliver(new, reflection, passage + "\nNEXT: " + action)
    original = Path(receipt["artifact_path"]).read_bytes()
    private, request = once(f, new, "carry-old-reflection", action)
    f.check(private["input_kind"] == "private_writing" and passage in private["text"],
            "explicit selection carries old verified reflection into private writing")
    f.check(PRIVATE not in private["text"], "unselected private prose is not supplied")
    f.check(drafts(f)["drafts"]["d2"]["parts"] == [passage],
            "selected passage is preserved exactly without executable NEXT")
    f.check(f.call(new, **request) == private, "lost preparation acknowledgement replays exactly")
    stable = file_hashes(f.state)
    f.call(new, reject=True, **(request | {"action": "WRITE START conflicting retry"}))
    f.check(file_hashes(f.state) == stable, "conflicting retry preserves all canonical bytes")
    f.prepare(old, action, reject=True)
    f.check(file_hashes(f.state) == stable, "old helper refuses new command without changing state")

    f.deliver(new, private, "Synthetic new development.\nNEXT: WRITE PARK")
    f.prepare(new, "WRITE PARK")
    fresh, _ = once(f, new, "fresh-reflection", "INTROSPECT")
    f.check(fresh["input_kind"] == "reflection" and "Continuity orientation:" in fresh["text"],
            "new reflection offers factual storage orientation")
    f.check(passage not in fresh["text"] and PRIVATE not in fresh["text"]
            and "Synthetic new development." not in fresh["text"],
            "fresh reflection never implicitly recalls private or previous prose")
    selected_action = f"WRITE FROM_REFLECTION {fresh['navigation_id']} 0 2"
    f.deliver(new, fresh, "\u03bb selected range, other words omitted\nNEXT: " + selected_action)
    selected, selection_request = once(f, new, "selected-range", selected_action)
    f.check(drafts(f)["drafts"]["d3"]["parts"] == ["\u03bb"],
            "explicit UTF-8 range is exact and excludes unselected passage")
    f.check(f.call(new, **selection_request) == selected,
            "restarted helper reuses prepared selected passage")
    # Existing schema supports the pending native writing record even though an
    # old executable cannot initiate the new command. Do not infer this from tags.
    f.deliver(old, selected, "Synthetic delivery by retained old helper.\nNEXT: WRITE PARK")
    f.check(drafts(f)["drafts"]["d3"]["parts"] == [
        "\u03bb", "Synthetic delivery by retained old helper."],
        "retained old helper accepts new prepared native writing without losing its seed")
    f.prepare(new, "WRITE PARK")
    returned = f.prepare(new, "WRITE RESUME d2")
    f.check(passage in returned["text"] and "Synthetic new development." in returned["text"],
            "explicit return restores exact selected and continued parts")
    revised = f.prepare(new, "WRITE REVISE qualify the explanation, not the original interest")
    f.deliver(new, revised, "Synthetic qualification, with uncertainty retained.\nNEXT: REST")
    f.check(Path(receipt["artifact_path"]).read_bytes() == original,
            "original public reflection artifact remains immutable")
    f.check(drafts(f)["drafts"]["d1"] == old_draft,
            "pre-existing unrelated private draft is byte-equivalent")
    after = f.checkpoint()
    f.check(before["version"] == after["version"] == 12, "reader schema remains twelve")
    for field in ("questions", "bookmarks", "notebook"):
        f.check(before[field] == after[field], f"{field} survives chosen continuation unchanged")
    preserved = drafts(f)
    f.prepare(old, "WRITE HELP")
    inspected = drafts(f)
    for field in ("schema_version", "drafts", "receipts", "active", "last_choice", "preview_deliveries"):
        f.check(inspected[field] == preserved[field],
                f"old helper inspection preserves writing {field}")
    f.check(all(inspected["retained_pending"].get(key) == value
                for key, value in preserved["retained_pending"].items()),
            "old helper inspection retains historical pending presentations")
    f.check(not (f.workspace / "journal").exists(), "no automatic public journal created")
    return dict(owner=owner, checks=f.checks, state_hashes=file_hashes(f.state))


def adapter_probe(snapshot, stage, root):
    fixture = Fixture(root, "minime", stage / "helpers/astrid-source-study",
                      stage / "helpers/astrid-source-study")
    write_json(root / "astrid/.runtime/bridge-deployment/active.json",
               dict(schema="bridge_release_selection_v1", stage=str(stage),
                    manifest_sha256=sha(stage / "manifest.json")))
    code = r'''
import json, pathlib, sys
from minime_autonomy.source_study import StudyClient, selected_reader
root, expected = map(pathlib.Path, sys.argv[1:])
assert selected_reader(root / 'astrid') == expected
client = StudyClient(root / 'minime', root / 'owner-workspace', astrid_root=root / 'astrid')
assert client.executable == expected
def deliver(prompt, text):
    class Response:
        status_code = 200
        def __init__(self):
            self.text = json.dumps({'message': {'content': text}, 'done': True, 'done_reason': 'stop'})
        def json(self):
            return json.loads(self.text)
    prompt.post(lambda *_args, **_kwargs: Response(), 'synthetic://stub',
                {'messages': [{'role': 'user', 'content': str(prompt)}]}, 1)
    prompt.accepted()
reflection = client.prepare('INTROSPECT', request_id='adapter-reflection')
action = 'WRITE FROM_REFLECTION ' + reflection.output['navigation_id']
passage = 'Synthetic adapter-selected public reflection.'
deliver(reflection, passage + '\nNEXT: ' + action)
private = client.prepare(action, request_id='adapter-continuation')
assert private.output['input_kind'] == 'private_writing' and passage in private
deliver(private, 'Synthetic private development.\nNEXT: WRITE PARK')
assert 'source_study_diagnostic_path' not in private.diagnostic_summary
fresh = client.prepare('INTROSPECT', request_id='adapter-fresh')
assert passage not in fresh and 'Synthetic private development.' not in fresh
state = json.loads((client.workspace / 'diagnostics/source_first_v3/shared_reader/writing/drafts-v2.json').read_bytes())
assert state['drafts']['d1']['parts'] == [passage, 'Synthetic private development.']
assert not (client.workspace / 'journal').exists()
print(json.dumps({'same_helper_selected': True, 'actual_adapter_exact_continuation': True,
                  'no_private_path_in_summary': True, 'no_automatic_public_journal': True}))
'''
    env = {key: os.environ[key] for key in ("HOME", "PATH", "TMPDIR") if key in os.environ}
    env.update(PYTHONPATH=str(snapshot), PYTHONDONTWRITEBYTECODE="1")
    result = subprocess.run([sys.executable, "-B", "-c", code, str(root), str(fixture.new)],
                            cwd=snapshot, env=env, capture_output=True, text=True, timeout=90)
    require(result.returncode == 0, f"frozen adapter continuation failed: {result.stderr[-2000:]}")
    return json.loads(result.stdout)


def qualify(old, new, minime, reconciliation, out):
    stages = {"old": verify_stage(old), "new": verify_stage(new)}
    inputs = inventory(minime)
    binding = verify_launch_binding(reconciliation, inputs)
    out.mkdir(mode=0o700)
    try:
        baseline = qualify_baseline(old, new, minime, reconciliation, out / "baseline")
        readers = {name: stage / "helpers/astrid-source-study" for name, stage in (("old", old), ("new", new))}
        owners = [owner_continuity(out / owner, owner, readers["old"], readers["new"])
                  for owner in ("astrid", "minime")]
        adapter = adapter_probe(minime, new, out / "adapter")
        invalid = invalid_state_probes(out, readers["old"], readers["new"])
        retries = preparation_retry_probe(out, readers["new"])
        require(inventory(minime) == inputs, "frozen Minime input drift")
        require(verify_launch_binding(reconciliation, inputs) == binding, "launch binding drift")
        require(stages == {"old": verify_stage(old), "new": verify_stage(new)}, "stage drift")
        receipt = dict(schema="reflection_continuity_paired_qualification_v1", status="passed",
                       stages=stages, readers={name: sha(path) for name, path in readers.items()},
                       selected_inputs=inputs, launch_binding=binding,
                       baseline_receipt_sha256=sha(out / "baseline/qualification.json"),
                       checks=baseline["checks"] + sum(len(owner["checks"]) for owner in owners),
                       owners=owners, adapter=adapter, invalid_state_probes=invalid,
                       preparation_retries=retries, qualifier_inputs={name: sha(Path(__file__).with_name(name))
                           for name in (Path(__file__).name, "qualify_reflection_release.py", "qualify_geometry_release.py", "bridge_stage.py")},
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
