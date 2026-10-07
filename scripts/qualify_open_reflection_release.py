#!/usr/bin/env python3
"""Bind open-reflection qualification to immutable artifacts and synthetic state.

No live writing, inference, service signals or activation authority. Existing
EOF preservation and schema-13 reflection tests remain independent evidence.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

from bridge_stage import verify_stage
from qualify_eof_release import qualify as qualify_eof
from qualify_geometry_release import (
    Fixture, SOURCE, inventory, require, sha, verify_launch_binding, write_json,
)


def reflection_preservation(old, new, out):
    script = Path(__file__).with_name("test_open_reflection_preservation.py")
    env = {key: os.environ[key] for key in ("HOME", "PATH", "TMPDIR") if key in os.environ}
    env.update(QUALIFICATION_OLD_READER=str(old), ASTRID_SOURCE_STUDY_BIN=str(new),
               PYTHONDONTWRITEBYTECODE="1")
    log = out / "schema-13-preservation.log"
    with log.open("x") as handle:
        result = subprocess.run([sys.executable, "-B", str(script), "-v"],
                                cwd=script.parent, env=env, stdout=handle,
                                stderr=subprocess.STDOUT, timeout=180, check=False)
        handle.flush()
        os.fsync(handle.fileno())
    require(result.returncode == 0, "schema-13 reflection preservation failed; inspect retained log")
    return dict(status="passed", owners=["astrid", "minime"], log_sha256=sha(log),
                test_sha256=sha(script))


def adapter_probe(snapshot, stage, root):
    helper = stage / "helpers/astrid-source-study"
    Fixture(root, "minime", helper, helper)
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
    prompt.post(lambda *_a, **_kw: Response(), 'synthetic://stub',
                {'messages': [{'role': 'user', 'content': str(prompt)}]}, 1)
    prompt.accepted()
state_path = client.workspace / 'diagnostics/source_first_v3/shared_reader/reader-v1.json'
client.prepare('SELF_STUDY QUESTION NEW Synthetic verification question?', request_id='question')
page = client.prepare('SELF_STUDY OPEN astrid/crates/example/src/lib.rs 1', request_id='page')
deliver(page, 'STUDY_NOTE: Exact synthetic note.\nNEXT: REST')
before = json.loads(state_path.read_bytes())
help_page = client.prepare('SELF_STUDY HELP verification', request_id='help')
assert help_page.output['input_kind'] == 'help' and help_page.output['page'] is None
assert 'CURRENT VERIFICATION CONTEXT: q1.' in help_page
assert 'SELF_STUDY RELATE first' in help_page
assert 'sha256=' + page.output['page']['revision']['sha256'] in help_page
after = json.loads(state_path.read_bytes())
for field in ('bookmarks', 'progress', 'current'):
    assert before[field] == after[field]
source = root / 'astrid/crates/example/src/lib.rs'
source.write_text('pub fn replacement() {}\n')
stale = client.prepare('SELF_STUDY HELP verification', request_id='changed-source')
assert 'No symbol candidates derived' in stale
assert 'SELF_STUDY RELATE first' not in stale
published = client.workspace / 'journal/introspect_2026-10-07.txt'
published.parent.mkdir(parents=True, exist_ok=True)
published.write_text('=== SYNTHETIC PUBLIC ===\n\nPUBLIC_RECALL_SENTINEL\nNEXT: REST\n')
published_bytes = published.read_bytes()
reflection = client.prepare('INTROSPECT', request_id='fresh-reflection')
assert 'PUBLIC_RECALL_SENTINEL' not in reflection
passage = 'Exact selected synthetic reflection, including \u03bb.'
action = 'WRITE FROM_REFLECTION ' + reflection.output['navigation_id']
deliver(reflection, passage + '\nNEXT: ' + action)
private = client.prepare(action, request_id='chosen-continuation')
assert passage in private and 'PUBLIC_RECALL_SENTINEL' not in private
deliver(private, 'Synthetic private development.\nNEXT: WRITE PARK')
assert 'source_study_diagnostic_path' not in private.diagnostic_summary
client.prepare('WRITE PARK', request_id='park')
fresh = client.prepare('INTROSPECT', request_id='another-fresh-reflection')
for absent in (passage, 'Synthetic private development.', 'PUBLIC_RECALL_SENTINEL'):
    assert absent not in fresh
returned = client.prepare('WRITE RESUME d1', request_id='return')
assert passage in returned and 'Synthetic private development.' in returned
assert published.read_bytes() == published_bytes
assert list(published.parent.iterdir()) == [published]
print(json.dumps({'same_immutable_helper': True, 'source_bound_help': True,
                  'stale_source_explicit': True, 'fresh_reflection_omits_prior_prose': True,
                  'exact_chosen_continuation': True, 'quiet_park_and_explicit_return': True,
                  'no_automatic_publication': True, 'synthetic_stub_only': True}))
'''
    env = {key: os.environ[key] for key in ("HOME", "PATH", "TMPDIR") if key in os.environ}
    env.update(PYTHONPATH=str(snapshot), PYTHONDONTWRITEBYTECODE="1")
    result = subprocess.run([sys.executable, "-B", "-c", code, str(root), str(helper)],
                            cwd=snapshot, env=env, capture_output=True, text=True,
                            timeout=120, check=False)
    require(result.returncode == 0, f"frozen open-reflection adapter failed: {result.stderr[-2000:]}")
    return json.loads(result.stdout)


def qualify(old, new, minime, reconciliation, out):
    stages = {"old": verify_stage(old), "new": verify_stage(new)}
    inputs = inventory(minime)
    binding = verify_launch_binding(reconciliation, inputs)
    names = (Path(__file__).name, "test_open_reflection_preservation.py",
             "qualify_eof_release.py", "qualify_geometry_release.py",
             "qualify_source_selection_release.py", "qualify_reflection_continuity_release.py",
             "bridge_stage.py")
    sources = {name: sha(Path(__file__).with_name(name)) for name in names}
    out.mkdir(mode=0o700)
    try:
        baseline = qualify_eof(old, new, minime, reconciliation, out / "eof-baseline")
        reflection = reflection_preservation(old / "helpers/astrid-source-study",
                                             new / "helpers/astrid-source-study", out)
        adapter = adapter_probe(minime, new, out / "adapter")
        require(inventory(minime) == inputs, "frozen Minime input drift")
        require(verify_launch_binding(reconciliation, inputs) == binding, "launch binding drift")
        require(stages == {"old": verify_stage(old), "new": verify_stage(new)}, "stage drift")
        require(sources == {name: sha(Path(__file__).with_name(name)) for name in names},
                "qualification source drift")
        receipt = dict(schema="open_reflection_paired_qualification_v1", status="passed",
                       stages=stages, selected_inputs=inputs, launch_binding=binding,
                       baseline_checks=baseline["checks"],
                       baseline_receipt_sha256=sha(out / "eof-baseline/qualification.json"),
                       reflection_preservation=reflection, frozen_adapter=adapter,
                       qualifier_inputs=sources, activation_performed=False, live_eligible_now=False,
                       scope="Synthetic state and stubbed providers only; no live writing or services")
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
    print(json.dumps(dict(status=result["status"], receipt=str(args.out / "qualification.json"),
                          activation_performed=False)))


if __name__ == "__main__":
    main()
