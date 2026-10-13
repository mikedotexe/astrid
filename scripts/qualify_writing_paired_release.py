#!/usr/bin/env python3
"""Qualify immutable writing releases with synthetic stores and no activation."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

from bridge_stage import verify_stage
from qualify_geometry_release import (
    Fixture, file_hashes, freeze_python, inventory, invalid_state_probes,
    preparation_retry_probe, require, sha, verify_launch_binding, write_json,
)
from qualify_mailbox_context_release import adapter_probe, bridge_checkpoint_probe, QUALIFIER_INPUTS
from qualify_writing_continuity import owner_check


WRITING_PROBE = r'''
import hashlib, json, pathlib, sys
from types import SimpleNamespace
root, snapshot, helper = map(pathlib.Path, sys.argv[1:])
def guard(event, args):
    if event in {'socket.connect', 'socket.bind', 'os.system'}:
        raise RuntimeError('writing qualification forbids network and shell execution')
    if event == 'subprocess.Popen' and args[1] != [str(helper)]:
        raise RuntimeError('only the pinned reader subprocess is allowed')
sys.addaudithook(guard)
from minime_autonomy.source_study import StudyClient, selected_reader
from minime_autonomy.journal_continuation import JournalContinuation
from minime_autonomy.deployment import source_inputs
from minime_autonomy.expressive_journal import fresh_journal_selected
assert selected_reader(root / 'astrid') == helper
client = StudyClient(root / 'minime', root / 'owner-workspace', astrid_root=root / 'astrid')
checks = []
def check(condition, name):
    assert condition, name
    checks.append(name)
def deliver(prompt, text):
    messages, _ = prompt.messages(prompt.output['system_prompt'], prompt.input_budget_bytes)
    response = SimpleNamespace(status_code=200, text=json.dumps({
        'message': {'content': text}, 'done': True, 'done_reason': 'stop'}))
    prompt.post(lambda *args, **kwargs: response, 'synthetic://stub', {'messages': messages}, 1)
    prompt.accepted()
def reject(action, code=None):
    try:
        client.prepare(action)
    except RuntimeError as error:
        if code:
            assert error.writing_recovery['code'] == code
        return
    raise AssertionError('unexpectedly accepted command')
reserved = JournalContinuation.reserve(client, 'synthetic-publication')
reject('WRITE FROM_JOURNAL ' + reserved.entry_id)
check(JournalContinuation.reserve(client, 'synthetic-publication') == reserved, 'stable reservation')
journal = client.workspace / 'journal/journal_fixture.txt'
journal.parent.mkdir(parents=True)
prose = 'Exact synthetic public passage, including \u03bb.\n\nSecond paragraph.'
response = prose + '\nNEXT: REST'
journal.write_text('=== JOURNAL ===\n\n' + response + '\n')
published = journal.read_bytes()
check(reserved.publish(journal, response)['published'], 'exact publication verified')
action = 'WRITE FROM_JOURNAL ' + reserved.entry_id
prompt = client.prepare(action, request_id='transfer')
check(prose in prompt, 'complete selected public prose supplied')
deliver(prompt, 'PRIVATE_SYNTHETIC_SENTINEL\nNEXT: WRITE CONTINUE')
checkpoint = client.workspace / 'diagnostics/source_first_v3/shared_reader/writing/drafts-v2.json'
check(json.loads(checkpoint.read_bytes())['drafts']['d1']['parts'] == [prose, 'PRIVATE_SYNTHETIC_SENTINEL'], 'exact seed and continuation retained')
for n, action in enumerate(['WRITE QUESTION Optional?', 'WRITE EVIDENCE synthetic reference',
        'WRITE STOPPING_POINT here', 'WRITE PROFILE SHORT', 'WRITE FINISH']):
    check(client.writing_storage_only(action), 'shared metadata classification ' + str(n))
    receipt = client.prepare(action, request_id='metadata-' + str(n))
    check(receipt.output['generation_requested'] is False, 'no generation ' + str(n))
    before = checkpoint.read_bytes()
    check(client.prepare(action, request_id='metadata-' + str(n)).output == receipt.output
          and checkpoint.read_bytes() == before, 'metadata exact retry ' + str(n))
reject('WRITE CONTINUE', 'draft_finished')
resumed = client.prepare('WRITE RESUME d1', request_id='resume')
check('PRIVATE_SYNTHETIC_SENTINEL' in resumed, 'explicit resume retains exact prose')
park = client.prepare('WRITE PARK', request_id='park')
check(not park.output['generation_requested'], 'parking schedules no generation')
deliver(resumed, 'Late accepted synthetic work.\nNEXT: REST')
check(json.loads(checkpoint.read_bytes())['active'] is None, 'late completion does not reselect')
unrelated = client.prepare('INTROSPECT', request_id='unrelated')
check('PRIVATE_SYNTHETIC_SENTINEL' not in unrelated, 'fresh reflection excludes private writing')
returned = client.prepare('WRITE RESUME d1', request_id='return')
check('Late accepted synthetic work.' in returned, 'deliberate return retains late output')
check(journal.read_bytes() == published and list(journal.parent.iterdir()) == [journal], 'no private publication')
check(fresh_journal_selected({'canonical_action': 'JOURNAL FRESH'})
      and not fresh_journal_selected({'canonical_action': 'JOURNAL'}), 'fresh choice remains invocation scoped')
inputs = source_inputs(snapshot)
loaded = {}
for name, module in tuple(sys.modules.items()):
    if not name.startswith('minime_autonomy'):
        continue
    path = pathlib.Path(module.__file__).resolve()
    assert path.is_relative_to(snapshot)
    relative = str(path.relative_to(snapshot))
    loaded[relative] = hashlib.sha256(path.read_bytes()).hexdigest()
    assert loaded[relative] == inputs[relative]
print(json.dumps({'checks': checks, 'loaded_inputs': loaded, 'synthetic_stub_only': True,
                  'no_private_publication': True, 'activation_performed': False}))
'''


def writing_probe(snapshot, stage, root):
    helper = stage / "helpers/astrid-source-study"
    Fixture(root, "minime", helper, helper)
    write_json(root / "astrid/.runtime/bridge-deployment/active.json", dict(
        schema="bridge_release_selection_v1", stage=str(stage),
        manifest_sha256=sha(stage / "manifest.json")))
    home = root / "home"
    home.mkdir()
    dependencies = [str(Path(path).resolve()) for path in sys.path if path
                    and Path(path).name in {"site-packages", "dist-packages"}]
    env = {key: os.environ[key] for key in ("PATH", "TMPDIR") if key in os.environ}
    env.update(HOME=str(home), PYTHONDONTWRITEBYTECODE="1",
               PYTHONPATH=os.pathsep.join([str(snapshot), *dependencies]))
    result = subprocess.run([sys.executable, "-B", "-c", WRITING_PROBE,
                             str(root), str(snapshot), str(helper)], cwd=snapshot,
                            env=env, capture_output=True, text=True, timeout=120, check=False)
    (root / "probe.stdout").write_text(result.stdout)
    (root / "probe.stderr").write_text(result.stderr)
    require(result.returncode == 0, "frozen writing adapter failed; inspect retained probe.stderr")
    report = json.loads(result.stdout)
    report["fixture_hashes"] = file_hashes(root / "owner-workspace")
    return report


def qualify(old, new, minime, reconciliation, out):
    stages = {"old": verify_stage(old), "new": verify_stage(new)}
    inputs = inventory(minime)
    binding = verify_launch_binding(reconciliation, inputs)
    names = set(QUALIFIER_INPUTS) | {Path(__file__).name, "qualify_writing_continuity.py"}
    sources = {name: sha(Path(__file__).with_name(name)) for name in sorted(names)}
    out.mkdir(mode=0o700)
    try:
        require(freeze_python(minime, out / "minime-source") == inputs, "snapshot input drift")
        readers = [stage / "helpers/astrid-source-study" for stage in (old, new)]
        owners = [owner_check(out / owner, owner, *readers) for owner in ("astrid", "minime")]
        bridge = bridge_checkpoint_probe(old, new, out / "bridge-checkpoint")
        handoff = adapter_probe(out / "minime-source", new, out / "adapter-handoff")
        writing = writing_probe(out / "minime-source", new, out / "adapter-writing")
        invalid = invalid_state_probes(out, *readers)
        retries = preparation_retry_probe(out, readers[1])
        require(inventory(minime) == inputs == inventory(out / "minime-source"), "Minime input drift")
        require(verify_launch_binding(reconciliation, inputs) == binding, "launch binding drift")
        require(stages == {"old": verify_stage(old), "new": verify_stage(new)}, "stage drift")
        require(sources == {name: sha(Path(__file__).with_name(name)) for name in sources}, "qualifier drift")
        receipt = dict(schema="writing_continuity_paired_qualification_v1", status="passed",
                       stages=stages, selected_inputs=inputs, launch_binding=binding,
                       qualifier_inputs=sources, owners=owners, bridge_checkpoint=bridge,
                       handoff=handoff, writing=writing, invalid_state_probes=invalid,
                       preparation_retries=retries, activation_performed=False,
                       live_eligible_now=False, checks=sum(len(x["checks"]) for x in owners) + len(writing["checks"]),
                       scope="Immutable candidates, synthetic state, stubbed providers; fresh rollout approval and live preflight required")
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
    print(json.dumps({key: result[key] for key in ("status", "checks", "activation_performed")}))


if __name__ == "__main__":
    main()
