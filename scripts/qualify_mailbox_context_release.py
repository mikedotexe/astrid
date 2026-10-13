#!/usr/bin/env python3
"""Qualify mailbox/context handoff with immutable releases and synthetic inputs.

No live checkpoint, provider, service, approval, or activation is used. Failed
attempts retain their fixture evidence; passing grants no deployment authority.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

from bridge_stage import verify_stage
from qualify_eof_release import owner_preservation
from qualify_geometry_release import (
    AUTHORED, PRIVATE, SOURCE, Fixture, file_hashes, freeze_python, inventory,
    invalid_state_probes, preparation_retry_probe, require, sha,
    verify_launch_binding, write_json,
)
from qualify_private_delivery_recovery import native
from qualify_source_selection_release import once

QUALIFIER_INPUTS = (
    "qualify_mailbox_context_release.py", "qualify_eof_release.py",
    "qualify_geometry_release.py", "qualify_source_selection_release.py",
    "qualify_private_delivery_recovery.py", "bridge_stage.py",
)
ADAPTER_MODULES = (
    "autonomous_agent.py", "minime_autonomy/runtime.py", "minime_autonomy/source_study.py",
    "minime_autonomy/source_study_diagnostics.py", "minime_autonomy/mailbox.py",
    "minime_autonomy/mailbox_fs.py", "minime_autonomy/inbox_delivery.py",
    "minime_autonomy/maintenance_next.py", "minime_autonomy/deployment.py",
)


def owner_context(root, owner, old, new):
    f = Fixture(root, owner, old, new)
    f.prepare(old, "SELF_STUDY QUESTION NEW Retained mailbox-context inquiry?")
    page = f.prepare(old, f"SELF_STUDY OPEN {SOURCE} 1")
    f.deliver(old, page, f"STUDY_NOTE: {AUTHORED}\nNEXT: INTROSPECT")
    draft = f.prepare(old, "WRITE START Unrelated retained private work")
    f.deliver(old, draft, PRIVATE + "\nNEXT: WRITE PARK")
    f.prepare(old, "WRITE PARK")
    draft_path = f.state / "writing/drafts-v2.json"
    draft_bytes = draft_path.read_bytes()
    pending, request = once(f, old, "old-pending-reflection", "INTROSPECT")
    before = f.checkpoint()
    f.check(before["version"] == 13, "old helper genuinely writes schema thirteen")
    f.check("CHECK_MAILBOX" not in pending["text"] and "MAILBOX_STATUS" not in pending["text"],
            "pinned old reflection lacks the new mailbox choices")
    hashes = file_hashes(f.state)
    f.check(f.call(new, **request) == pending, "old pending reflection replays exactly")
    f.check(file_hashes(f.state) == hashes, "new helper replay preserves every old state byte")
    action = "WRITE FROM_REFLECTION " + pending["navigation_id"]
    passage = "Exact synthetic selected passage, including \u03bb."
    response = passage + "\nNEXT: " + action
    receipt = f.deliver(new, pending, response)
    response_json = json.dumps({"message": {"content": response}, "done": True, "done_reason": "stop"})
    f.check(receipt["response_sha256"] == hashlib.sha256(response_json.encode()).hexdigest(),
            "old pending delivery binds exact response bytes including authored NEXT")
    artifact = Path(receipt["artifact_path"])
    artifact_bytes = artifact.read_bytes()
    fresh, retry = once(f, new, "new-reflection", "INTROSPECT")
    f.check(all(choice in fresh["text"] for choice in ("CHECK_MAILBOX", "MAILBOX_STATUS")),
            "new reflection explicitly offers receive and status choices")
    f.check(all(text not in fresh["text"] for text in (passage, PRIVATE, AUTHORED)),
            "changed choices do not import old public or private prose")
    f.check(draft_path.read_bytes() == draft_bytes, "unrelated private state remains byte-exact")
    after = f.checkpoint()
    f.check(after["version"] == 13, "new helper keeps schema thirteen")
    for field in ("questions", "bookmarks", "notebook", "progress"):
        f.check(before[field] == after[field], f"reflection preserves {field}")
    copied = Fixture(root / "old-inspection", owner, old, new)
    shutil.copytree(f.state, copied.state)
    copied_before = file_hashes(copied.state)
    f.check(copied.call(old, **retry) == fresh, "old helper replays new pending input on a copy")
    f.check(file_hashes(copied.state) == copied_before, "old replay never rewrites new state bytes")
    carried, carry_retry = once(f, new, "chosen-old-reflection", action)
    f.check(passage in carried["text"] and PRIVATE not in carried["text"],
            "explicit return still carries only the selected old reflection")
    stable = file_hashes(f.state)
    f.check(f.call(new, **carry_retry) == carried, "selected continuation retries exactly")
    f.call(new, reject=True, **(carry_retry | {"action": "WRITE START conflicting retry"}))
    f.check(file_hashes(f.state) == stable, "conflicting retry preserves every state byte")
    f.check(artifact.read_bytes() == artifact_bytes, "original reflection artifact remains immutable")
    return dict(owner=owner, checks=f.checks, state_hashes=file_hashes(f.state),
                reflection_artifact_sha256=sha(artifact), schema_version=13)


def bridge_checkpoint_probe(old, new, root):
    f = Fixture(root, "astrid", old / "helpers/astrid-source-study", new / "helpers/astrid-source-study")
    checkpoint = dict(exchange_count=2, creative_temperature=0.7, response_length=128,
                      self_reflect_paused=False, ears_closed=False, senses_snoozed=False,
                      recent_next_choices=["INTROSPECT", "CHECK_MAILBOX"], history=[
                          dict(minime_said="Historical plain journal.", astrid_said="Exact old reply."),
                          dict(minime_said="Source revision: synthetic\nInput evidence: synthetic\n"
                               "Account: Minime\u2019s response to this input, not independently verified code facts.\n"
                               "Delivery: verified input delivery; response claims and understanding not verified\n\n"
                               + "Exact authored \u03bb. " * 40, astrid_said="Exact new reply.")])
    write_json(f.workspace / "state.json", checkpoint)
    before = file_hashes(f.workspace)
    for stage in (old, new):
        native(stage / "spectral-bridge-server", stage / "manifest.json", f,
               "--verify-deployment-inputs",
               expected="staged activation requires existing valid self-control state")
        require(file_hashes(f.workspace) == before, "native checkpoint decode changed fixture state")
    return dict(synthetic_checkpoint_sha256=sha(f.workspace / "state.json"),
                both_runtime_schemas_decoded=True, bytes_unchanged=True,
                self_control_gate_deliberately_unsatisfied=True, services_started=False)


# This is a subprocess so only the pinned snapshot supplies Minime modules.
ADAPTER_CODE = r'''
import hashlib, json, os, pathlib, sys
from types import SimpleNamespace
root, snapshot, expected = map(pathlib.Path, sys.argv[1:])
def guard(event, args):
    if event in {'socket.connect', 'socket.bind', 'os.system'}:
        raise RuntimeError('qualification forbids network and shell execution')
    if event == 'subprocess.Popen' and args[1] != [str(expected)]:
        raise RuntimeError('qualification permits only the pinned reader subprocess')
sys.addaudithook(guard)
import autonomous_agent as aa
from minime_autonomy import maintenance_next as handoff
from minime_autonomy.deployment import source_inputs
from minime_autonomy.mailbox import Mailbox
from minime_autonomy.source_study import StudyClient, SourceStudyPrompt, selected_reader
assert selected_reader(root / 'astrid') == expected
client = StudyClient(root / 'minime', root / 'owner-workspace', astrid_root=root / 'astrid')
assert client.executable == expected
workspace = client.workspace
workspace.mkdir(parents=True, exist_ok=True)
inputs = source_inputs(snapshot)
state_path = workspace / 'sovereignty_state.json'
choice = 'WRITE CONTINUE d1 "retained synthetic NEXT: REST; \u03bb"'
state = {'pending_next_action': choice, 'pending_next_selection_id': 'old-selection',
         'choice_envelope_v1': {'source': 'synthetic-old-choice', 'opaque': ['\u03bb', 7]},
         'pending_next_action_status': 'pending', 'unrelated_authored_state': 'unchanged'}
handoff.write_state(state_path, state)
state_bytes = state_path.read_bytes()
def agent(boot, pid):
    return SimpleNamespace(_sovereignty_state_path=lambda: state_path,
        _agent_boot_id=boot * 32, _agent_pid=pid, _agent_started_at='synthetic fixture',
        _agent_inputs_at_start=inputs, _pending_next_action=choice,
        _pending_next_selection_id=state['pending_next_selection_id'],
        _pending_choice_envelope_v1=state['choice_envelope_v1'])
old = agent('a', 10001)
handoff.seal(old)
replacement = agent('b', 10002)
replacement._pending_next_action = None
receipt = handoff.restore(replacement)
assert receipt['scheduling_authorized'] is False and state_path.read_bytes() == state_bytes
assert replacement._pending_next_action == choice
assert replacement._pending_next_selection_id == state['pending_next_selection_id']
assert replacement._pending_choice_envelope_v1 == state['choice_envelope_v1']

inbox = workspace / 'inbox'
inbox.mkdir()
letter = ('=== HUMAN LETTER V1 ===\nMessage-Id: human_fixture\nThread-Id: thread_fixture\n'
          'From: mike\nTo: minime\n\nSynthetic selected letter. \u03bb\r\nExact second line.')
source = inbox / 'human_letter_fixture.txt'
source.write_bytes(letter.encode())
mailbox = Mailbox(workspace)
notice = mailbox.notice()
assert notice and letter not in notice.text and source.name not in notice.text
assert source.read_bytes() == letter.encode()
reflection = client.prepare('INTROSPECT', request_id='fixture-reflection')
prompt = SourceStudyPrompt(client, reflection.output, host_feedback=notice.text)
prompt.mailbox_notice = notice
wire = []
def deliver(prompt, text):
    class Response:
        status_code = 200
        def __init__(self):
            self.text = json.dumps({'message': {'content': text}, 'done': True, 'done_reason': 'stop'})
    def post(_url, **kwargs):
        wire.append(kwargs['data'].decode())
        return Response()
    messages, _ = prompt.messages('Synthetic optional reflection.', 32000)
    prompt.post(post, 'synthetic://stub', {'messages': messages}, 1)
    prompt.accepted()
deliver(prompt, 'Synthetic reflection only.\nNEXT: INTROSPECT')
submitted = json.loads(wire[-1])['messages']
assert any(notice.text in row['content'] for row in submitted)
assert all(letter not in row['content'] for row in submitted)
assert Mailbox(workspace).notice() is None and source.exists()
assert Mailbox(workspace).status()['waiting_files'] == 1
assert state_path.read_bytes() == state_bytes and replacement._pending_next_action == choice

selected = mailbox.open('explicit-mailbox-choice', sort_key=aa.inbox_sort_key)
messages, _ = aa._adapt_ollama_messages_for_model(model='gemma4:12b',
    system_msg='Synthetic optional correspondence.', prompt=selected, num_ctx=8192, num_predict=2048)
assert any(letter in row['content'] for row in messages)
attempt = selected.inbox.prepared(messages, 'synthetic-stub')
selected.inbox.accepted(attempt, 'synthetic-stub')
text = 'UNADDRESSED_PRIVATE_SENTINEL\nINBOX_REPLY human_fixture\nExact synthetic reply.\nNEXT: INTROSPECT'
selected.validate_completion({'done': True, 'done_reason': 'stop', 'message': {'content': text}})
mailbox.complete(selected, text)
assert not source.exists() and (inbox / 'read' / source.name).read_bytes() == letter.encode()
replies = list((workspace / 'outbox/human/mike').glob('*.txt'))
assert len(replies) == 1 and 'Exact synthetic reply.' in replies[0].read_text()
assert 'UNADDRESSED_PRIVATE_SENTINEL' not in replies[0].read_text() and 'NEXT:' not in replies[0].read_text()
assert Mailbox(workspace).open('explicit-mailbox-choice', sort_key=aa.inbox_sort_key) is None
assert not list((workspace / 'journal').glob('*.txt'))
assert not (workspace / 'correspondence/inbox_delivery_v1.jsonl').stat().st_mode & 0o077
fresh = client.prepare('INTROSPECT', request_id='fixture-after-mail')
assert letter not in fresh and 'Exact synthetic reply.' not in fresh
assert state_path.read_bytes() == state_bytes and replacement._pending_next_action == choice

# A submitted but incomplete completion is preserved privately and never retried.
source.write_bytes(letter.replace('human_fixture', 'human_incomplete').encode())
pending = Mailbox(workspace).open('incomplete-mailbox-choice', sort_key=aa.inbox_sort_key)
messages, _ = aa._adapt_ollama_messages_for_model(model='gemma4:12b',
    system_msg='Synthetic optional correspondence.', prompt=pending, num_ctx=8192, num_predict=2048)
attempt = pending.inbox.prepared(messages, 'synthetic-stub')
pending.inbox.accepted(attempt, 'synthetic-stub')
try:
    pending.validate_completion({'done': True, 'done_reason': 'length', 'message': {'content': 'Partial reply.'}})
except ValueError:
    pass
else:
    raise AssertionError('incomplete completion accepted')
assert source.exists() and Mailbox(workspace).status()['reconciliation_required']
try:
    Mailbox(workspace).open('forbidden-retry', sort_key=aa.inbox_sort_key)
except ValueError:
    pass
else:
    raise AssertionError('uncertain invocation retried')
assert len(list((workspace / 'outbox/human/mike').glob('*.txt'))) == 1
assert state_path.read_bytes() == state_bytes
loaded = {}
for name, module in tuple(sys.modules.items()):
    # The public facade aliases runtime and rewrites __file__; retain both the
    # executed import origin and the compatibility facade, not just one alias.
    paths = {}
    for filename in {getattr(module, '__file__', None), getattr(getattr(module, '__spec__', None), 'origin', None)}:
        if not filename or filename in {'built-in', 'frozen'}:
            continue
        path = pathlib.Path(filename).resolve()
        if path.is_relative_to(snapshot):
            relative = str(path.relative_to(snapshot))
            assert relative in inputs
            paths[relative] = hashlib.sha256(path.read_bytes()).hexdigest()
            assert paths[relative] == inputs[relative]
        elif (name == 'autonomous_agent' or name.startswith(('minime_autonomy', 'mikemind'))
              or name.replace('.', '/') + '.py' in inputs):
            raise AssertionError('Minime module imported outside frozen source: ' + name)
    if paths:
        loaded[name] = paths
print(json.dumps({'selected_inputs': inputs, 'loaded_modules': loaded,
    'same_helper_selected': True, 'metadata_only_notice_verified_then_quiet': True,
    'one_explicit_letter_exact_reply_and_archive': True, 'uncertain_completion_no_retry': True,
    'old_pending_next_bytes_preserved': True, 'synthetic_handoff_only': True,
    'scheduling_authorized': False, 'no_automatic_journal_publication': True,
    'explicit_reply_count': len(replies), 'synthetic_stub_only': True}))
'''


def adapter_probe(snapshot, stage, root):
    helper = stage / "helpers/astrid-source-study"
    Fixture(root, "minime", helper, helper)
    write_json(root / "astrid/.runtime/bridge-deployment/active.json",
               dict(schema="bridge_release_selection_v1", stage=str(stage),
                    manifest_sha256=sha(stage / "manifest.json")))
    (root / "home").mkdir()
    env = {key: os.environ[key] for key in ("PATH", "TMPDIR") if key in os.environ}
    # Snapshot sitecustomize can shadow Homebrew's site-path setup. Reuse only
    # this interpreter's installed dependency directories, never another checkout.
    dependency_paths = [str(Path(path).resolve()) for path in sys.path
                        if path and Path(path).name in {"site-packages", "dist-packages"}
                        and Path(path).is_dir()]
    env.update(HOME=str(root / "home"),
               PYTHONPATH=os.pathsep.join([str(snapshot), *dependency_paths]), PYTHONDONTWRITEBYTECODE="1")
    log = root / "adapter.log"
    with log.open("x") as handle:
        os.chmod(log, 0o600)
        result = subprocess.run([sys.executable, "-B", "-c", ADAPTER_CODE,
                                 str(root), str(snapshot), str(helper)], cwd=snapshot, env=env,
                                stdout=subprocess.PIPE, stderr=handle, text=True, timeout=120, check=False)
        handle.flush()
        os.fsync(handle.fileno())
    (root / "adapter.stdout").write_text(result.stdout)
    require(result.returncode == 0, "frozen mailbox adapter failed; inspect retained adapter.log")
    report = json.loads(result.stdout)
    require(report["selected_inputs"] == inventory(snapshot), "runtime module inventory differs")
    loaded = {path for entry in report["loaded_modules"].values() for path in entry}
    require(set(ADAPTER_MODULES) <= loaded, "required mailbox adapter modules were not exercised")
    report["log_sha256"] = sha(log)
    report["python_executable"] = sys.executable
    report["dependency_paths"] = dependency_paths
    report["fixture_hashes"] = file_hashes(root / "owner-workspace")
    return report


def qualify(old, new, minime, reconciliation, out):
    stages = {"old": verify_stage(old), "new": verify_stage(new)}
    inputs = inventory(minime)
    require(set(ADAPTER_MODULES) <= inputs.keys(), "candidate mailbox modules missing from inventory")
    binding = verify_launch_binding(reconciliation, inputs)
    sources = {name: sha(Path(__file__).with_name(name)) for name in QUALIFIER_INPUTS}
    out.mkdir(mode=0o700)
    try:
        require(freeze_python(minime, out / "minime-source") == inputs, "snapshot input drift")
        readers = {name: stage / "helpers/astrid-source-study" for name, stage in (("old", old), ("new", new))}
        baseline = [owner_preservation(out / (owner + "-baseline"), owner, readers["old"], readers["new"])
                    for owner in ("astrid", "minime")]
        owners = [owner_context(out / owner, owner, readers["old"], readers["new"])
                  for owner in ("astrid", "minime")]
        bridge = bridge_checkpoint_probe(old, new, out / "bridge-checkpoint")
        adapter = adapter_probe(out / "minime-source", new, out / "adapter")
        invalid = invalid_state_probes(out, readers["old"], readers["new"])
        retries = preparation_retry_probe(out, readers["new"])
        require(inventory(minime) == inputs == inventory(out / "minime-source"), "frozen Minime input drift")
        require(verify_launch_binding(reconciliation, inputs) == binding, "launch binding drift")
        require(stages == {"old": verify_stage(old), "new": verify_stage(new)}, "stage drift")
        require(sources == {name: sha(Path(__file__).with_name(name)) for name in sources}, "qualifier source drift")
        receipt = dict(schema="mailbox_context_paired_qualification_v1", status="passed",
                       stages=stages, readers={name: sha(path) for name, path in readers.items()},
                       selected_inputs=inputs, launch_binding=binding, qualifier_inputs=sources,
                       baseline=baseline, owners=owners, bridge_checkpoint=bridge, adapter=adapter,
                       checks=sum(len(row["checks"]) for row in baseline + owners),
                       invalid_state_probes=invalid, preparation_retries=retries,
                       activation_performed=False, live_eligible_now=False,
                       scope="Synthetic state and stubbed providers; no live state, services or inference")
        write_json(out / "qualification.json", receipt)
        return receipt
    except Exception as error:
        write_json(out / "failure.json", dict(error=str(error), activation_performed=False,
                                             live_eligible_now=False))
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
