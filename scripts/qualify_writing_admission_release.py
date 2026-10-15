#!/usr/bin/env python3
"""Qualify final writing admission using immutable stages and synthetic state.

No live authored state, inference, installation, signals or activation authority.
Same-schema preservation uses the established paired-release probes.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

from bridge_stage import verify_stage
from qualify_eof_release import owner_preservation
from qualify_geometry_release import (
    Fixture, file_hashes, freeze_python, inventory, invalid_state_probes,
    preparation_retry_probe, require, sha, verify_launch_binding, write_json,
)
from qualify_mailbox_context_release import adapter_probe, bridge_checkpoint_probe, QUALIFIER_INPUTS
from qualify_writing_context_release import operator_probe, writing_preservation
from qualify_writing_continuity import once
from qualify_writing_paired_release import writing_probe


def admission_preservation(root, owner, old, new):
    f = Fixture(root, owner, old, new)
    start, _ = once(f, old, "start", "WRITE START Synthetic admission preservation")
    passage = "Synthetic retained passage. " * 1600
    f.deliver(old, start, passage + "\nNEXT: WRITE CONTINUE")
    pending, retry = once(f, old, "pending", "WRITE CONTINUE")
    before = file_hashes(f.state)
    size = len(pending["text"].encode("utf-8"))
    f.check(size < 48_000, "old helper prepared complete input below unchanged allowance")
    capacity = dict(prepared_input_bytes=size, system_bytes=48_000 - size + 100,
                    wrapper_bytes=128, allowance_bytes=48_000)
    request = dict(operation="writing_recovery", input_id=pending["navigation_id"],
                   final_capacity=capacity)
    recovery = f.call(new, **request)
    f.check(recovery["code"] == "final_request_too_large", "new helper identifies final refusal")
    f.check(recovery["final_capacity"] == capacity, "exact synthetic adapted counts retained")
    f.check(recovery["commands"] == ["WRITE READ d1 1", "WRITE PARK", "WRITE HELP"],
            "capacity refusal offers valid choices without unchanged retry")
    f.check(f.call(new, **request) == recovery, "recovery repeats identically without execution")
    f.check(file_hashes(f.state) == before, "capacity recovery changes no stored bytes")
    f.call(new, reject=True, **(request | {"final_capacity": capacity | {"prepared_input_bytes": size + 1}}))
    f.call(new, reject=True, **(request | {"final_capacity": capacity | {"system_bytes": 0, "wrapper_bytes": 0}}))
    f.check(file_hashes(f.state) == before, "invalid counts preserve all state")
    unknown = f.call(new, **(request | {"input_id": "writing-missing"}))
    f.check(unknown["code"] == "pending_input_unavailable", "missing identity never gains retry permission")
    f.check(f.call(new, **retry) == pending, "old pending input replays byte-exactly")
    f.check(file_hashes(f.state) == before, "replay does not retrofit shorter guidance")
    f.deliver(new, pending, "Synthetic completion across transition.\nNEXT: REST")
    fresh, _ = once(f, new, "fresh", "WRITE CONTINUE")
    f.check("Optional choices (never automatic):" in fresh["text"], "new inputs use compact choices")
    f.check("Return: WRITE RESUME d1\n" in fresh["text"], "return command has exact boundary")
    f.check(not (f.workspace / "journal").exists(), "private material never published")
    return dict(owner=owner, checks=f.checks, synthetic_state_hashes=file_hashes(f.state))


ADMISSION_PROBE = r'''
import hashlib, json, pathlib, sys
root, snapshot, old_helper, helper = map(pathlib.Path, sys.argv[1:])
def guard(event, args):
    if event in {'socket.connect', 'socket.bind', 'os.system', 'os.kill'}:
        raise RuntimeError('admission qualification forbids network, shell and signals')
    if event == 'subprocess.Popen' and args[1] not in ([str(helper)], [str(old_helper)]):
        raise RuntimeError('only pinned reader subprocesses are allowed')
sys.addaudithook(guard)
from minime_autonomy.source_study import StudyClient, WritingAdmissionRefused, selected_reader
checklist = []
def check(value, name):
    if not value:
        raise ValueError(name)
    checklist.append(name)
def hashes(path):
    return {str(p.relative_to(path)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(path.rglob('*')) if p.is_file() and p.name != 'reader.lock'}
check(selected_reader(root / 'astrid') == helper, 'frozen stage selects exact helper')
client = StudyClient(root / 'minime', root / 'owner-workspace', astrid_root=root / 'astrid')
prompt = client.prepare('WRITE START Synthetic final-lane comparison', request_id='start')
state = client.workspace / 'diagnostics/source_first_v3/shared_reader'
before = hashes(state)
draft_path = state / 'writing/drafts-v2.json'
selected_before = json.loads(draft_path.read_text())
for system in ('s' * 48_000, 's' * 47_999):
    try:
        prompt.messages(system, 48_000)
    except WritingAdmissionRefused as error:
        prompt.provider_failed(error)
    else:
        raise ValueError('oversized final request unexpectedly admitted')
recovery = prompt.final_admission_recovery()
check(recovery['code'] == 'final_request_too_large', 'frozen adapter obtains typed refusal')
check(recovery['final_capacity']['system_bytes'] == 47_999, 'smallest refusing lane retained')
check('WRITE CONTINUE' not in recovery['commands'], 'no futile continuation offered')
check(hashes(state) == before, 'final refusal and recovery preserve exact state')
check(client.prepare('WRITE CONTINUE').output == prompt.output, 'explicit retry retains input identity')
# Explicit selection updates the command binding, not the retained input or prose.
selected_before['pending']['selected_action_sha256'] = hashlib.sha256(b'WRITE CONTINUE').hexdigest()
check(json.loads(draft_path.read_text()) == selected_before,
      'explicit continuation changes only the selected command binding')
after_selection = hashes(state)
check(after_selection == before | {'writing/drafts-v2.json': after_selection['writing/drafts-v2.json']},
      'explicit continuation preserves every other state file')
before = after_selection
messages, _ = prompt.messages(prompt.output['system_prompt'], 48_000)
check(messages[1]['content'] == str(prompt), 'fitting fallback receives complete exact input')
check(prompt.final_admission_recovery() is None, 'admitted fallback suppresses capacity-only claim')
uncertain = client.prepare('WRITE CONTINUE')
try:
    uncertain.messages('s' * 48_000, 48_000)
except WritingAdmissionRefused:
    pass
uncertain.provider_failed(RuntimeError('synthetic unavailable provider'))
check(uncertain.final_admission_recovery() is None, 'unknown failure remains unknown')
client.executable = old_helper
old = client.prepare('WRITE CONTINUE')
try:
    old.messages('s' * 48_000, 48_000)
except WritingAdmissionRefused:
    pass
try:
    old.final_admission_recovery()
except RuntimeError as error:
    check('no unchanged retry' in str(error), 'old helper fails closed instead of offering invalid retry')
else:
    raise ValueError('old helper silently claimed new recovery support')
check(hashes(state) == before, 'old-helper refusal preserves all state')
client.executable = helper
unicode_prompt = client.prepare('WRITE CONTINUE')
system = '\u03bb' * 500
complete = len((system + unicode_prompt).encode('utf-8'))
try:
    unicode_prompt.messages(system, complete - 1)
except WritingAdmissionRefused:
    check(True, 'multibyte boundary refuses one byte below complete input')
else:
    raise ValueError('multibyte under-budget request admitted')
check(unicode_prompt.messages(system, complete)[0][1]['content'] == str(unicode_prompt),
      'exact multibyte boundary preserves complete input')
check(not (client.workspace / 'journal').exists(), 'private admission creates no public entry')
loaded = {}
for name, module in tuple(sys.modules.items()):
    if name.startswith('minime_autonomy') and getattr(module, '__file__', None):
        path = pathlib.Path(module.__file__).resolve()
        check(path.is_relative_to(snapshot), 'module loaded from frozen snapshot: ' + name)
        loaded[str(path.relative_to(snapshot))] = hashlib.sha256(path.read_bytes()).hexdigest()
print(json.dumps(dict(checks=checklist, loaded_inputs=loaded, provider_calls=0,
                     activation_performed=False, synthetic_only=True)))
'''


def adapter_admission_probe(snapshot, old_stage, new_stage, root):
    old, new = [stage / "helpers/astrid-source-study" for stage in (old_stage, new_stage)]
    Fixture(root, "minime", old, new)
    write_json(root / "astrid/.runtime/bridge-deployment/active.json", dict(
        schema="bridge_release_selection_v1", stage=str(new_stage),
        manifest_sha256=sha(new_stage / "manifest.json")))
    (root / "home").mkdir()
    dependencies = [str(Path(p).resolve()) for p in sys.path if p
                    and Path(p).name in {"site-packages", "dist-packages"}]
    env = {key: os.environ[key] for key in ("PATH", "TMPDIR") if key in os.environ}
    env.update(HOME=str(root / "home"), PYTHONPATH=os.pathsep.join([str(snapshot), *dependencies]),
               PYTHONDONTWRITEBYTECODE="1")
    result = subprocess.run([sys.executable, "-B", "-c", ADMISSION_PROBE,
                             str(root), str(snapshot), str(old), str(new)],
                            cwd=snapshot, env=env, capture_output=True, text=True, timeout=120)
    (root / "probe.stdout").write_text(result.stdout)
    (root / "probe.stderr").write_text(result.stderr)
    require(result.returncode == 0, "frozen admission probe failed; retained probe.stderr explains")
    report = json.loads(result.stdout)
    inputs = inventory(snapshot)
    require(all(inputs[path] == digest for path, digest in report["loaded_inputs"].items()),
            "frozen admission module identity mismatch")
    return {**report, "synthetic_state_hashes": file_hashes(root / "owner-workspace")}


def qualify(old, new, minime, reconciliation, operator, out):
    stages = {"old": verify_stage(old), "new": verify_stage(new)}
    inputs = inventory(minime)
    binding = verify_launch_binding(reconciliation, inputs)
    names = set(QUALIFIER_INPUTS) | {Path(__file__).name, "qualify_writing_context_release.py",
                                   "qualify_writing_continuity.py", "qualify_writing_paired_release.py"}
    sources = {name: sha(Path(__file__).with_name(name)) for name in sorted(names)}
    out.mkdir(mode=0o700)
    try:
        require(freeze_python(minime, out / "minime-source") == inputs, "snapshot drift")
        readers = [stage / "helpers/astrid-source-study" for stage in (old, new)]
        owners = [owner_preservation(out / (owner + "-reader"), owner, *readers)
                  for owner in ("astrid", "minime")]
        writing = [writing_preservation(out / (owner + "-writing"), owner, *readers)
                   for owner in ("astrid", "minime")]
        admission = [admission_preservation(out / (owner + "-admission"), owner, *readers)
                     for owner in ("astrid", "minime")]
        bridge = bridge_checkpoint_probe(old, new, out / "bridge-checkpoint")
        handoff = adapter_probe(out / "minime-source", new, out / "adapter-handoff")
        adapter_writing = writing_probe(out / "minime-source", new, out / "adapter-writing")
        adapter_admission = adapter_admission_probe(out / "minime-source", old, new, out / "adapter-admission")
        inspection = operator_probe(out / "operator-reader", operator, readers[1])
        invalid = invalid_state_probes(out, *readers)
        retries = preparation_retry_probe(out, readers[1])
        require(inputs == inventory(minime) == inventory(out / "minime-source"), "Python input drift")
        require(verify_launch_binding(reconciliation, inputs) == binding, "launch binding drift")
        require(stages == {"old": verify_stage(old), "new": verify_stage(new)}, "stage drift")
        require(sources == {name: sha(Path(__file__).with_name(name)) for name in sources}, "qualifier drift")
        checks = sum(len(r["checks"]) for r in owners + writing + admission +
                     [adapter_writing, adapter_admission, inspection])
        receipt = dict(schema="writing_admission_paired_qualification_v1", status="passed", checks=checks,
                       stages=stages, selected_inputs=inputs, launch_binding=binding, qualifier_inputs=sources,
                       readers=owners, writing=writing, admission=admission, bridge_checkpoint=bridge,
                       handoff=handoff, adapter_writing=adapter_writing, adapter_admission=adapter_admission,
                       operator_reader=inspection, invalid_state_probes=invalid, preparation_retries=retries,
                       activation_performed=False, live_eligible_now=False, capacity_qualification_included=False,
                       scope="Frozen artifacts, same-schema synthetic state and stubbed providers; fresh rollout approval required")
        write_json(out / "qualification.json", receipt)
        return receipt
    except Exception as error:
        write_json(out / "failure.json", dict(error=str(error), activation_performed=False))
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("old-stage", "new-stage", "minime-source", "reconciliation", "research-reader", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    result = qualify(args.old_stage.resolve(strict=True), args.new_stage.resolve(strict=True),
                     args.minime_source.resolve(strict=True), args.reconciliation.resolve(strict=True),
                     args.research_reader.resolve(strict=True), args.out.absolute())
    print(json.dumps({key: result[key] for key in ("status", "checks", "activation_performed")}))


if __name__ == "__main__":
    main()
