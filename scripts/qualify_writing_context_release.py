#!/usr/bin/env python3
"""Same-schema paired writing/context qualification on synthetic state only.

No providers, live state, service controls or rollout authority. Historical
schema-upgrade qualification remains separate and unchanged.
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
from qualify_eof_release import owner_preservation
from qualify_geometry_release import (
    Fixture, file_hashes, freeze_python, inventory, invalid_state_probes,
    preparation_retry_probe, require, sha, verify_launch_binding, write_json,
)
from qualify_mailbox_context_release import (
    adapter_probe, bridge_checkpoint_probe, QUALIFIER_INPUTS,
)
from qualify_writing_continuity import once
from qualify_writing_paired_release import writing_probe


def writing_preservation(root, owner, old, new):
    f = Fixture(root, owner, old, new)
    start, _ = once(f, old, "start", "WRITE START Synthetic preserved writing")
    f.deliver(old, start, "Exact synthetic old passage.\nNEXT: WRITE CONTINUE")
    pending, retry = once(f, old, "pending", "WRITE CONTINUE")
    before = file_hashes(f.state)
    f.check(f.call(new, **retry) == pending, "old pending writing replays exactly")
    f.check(file_hashes(f.state) == before, "same-schema replay changes no bytes")
    f.deliver(new, pending, "Exact synthetic new passage.\nNEXT: WRITE FINISH")
    path = f.state / "writing/drafts-v2.json"
    state = json.loads(path.read_bytes())
    f.check(state["schema_version"] == 5, "writing schema remains five")
    f.check(state["drafts"]["d1"]["parts"] == [
        "Exact synthetic old passage.", "Exact synthetic new passage."],
        "both exact passages survive accepted work across transition")
    finish, request = once(f, new, "finish", "WRITE FINISH")
    f.check(not finish["generation_requested"], "finish is storage-only")
    stable = file_hashes(f.state)
    f.check(f.call(new, **request) == finish, "metadata retry returns exact receipt")
    f.check(file_hashes(f.state) == stable, "metadata retry changes no bytes")
    copied = Fixture(root / "old-reader-copy", owner, old, new)
    shutil.copytree(f.state, copied.state)
    copied_before = file_hashes(copied.state)
    f.check(copied.call(old, **request) == finish, "old helper replays new receipt on copy")
    f.check(file_hashes(copied.state) == copied_before, "old replay never rewrites new data")
    error = f.prepare(new, "WRITE CONTINUE", reject=True)
    f.check(error["writing_recovery"]["commands"][0] == "WRITE RESUME d1",
            "finished work still offers explicit valid resume")
    resumed, _ = once(f, new, "resume", "WRITE RESUME d1")
    parked, _ = once(f, new, "park", "WRITE PARK")
    f.check(not parked["generation_requested"], "parking is storage-only")
    f.deliver(new, resumed, "Exact synthetic late completion.\nNEXT: REST")
    state = json.loads(path.read_bytes())
    f.check(state["active"] is None, "accepted late work never reselects parked writing")
    f.check(state["drafts"]["d1"]["parts"][-1] == "Exact synthetic late completion.",
            "accepted late work is retained")
    f.check(not (f.workspace / "journal").exists(), "no private publication")
    return dict(owner=owner, checks=f.checks, state_hashes=file_hashes(f.state))


def operator_probe(root, executable, helper):
    root.mkdir(mode=0o700)
    identity = sha(executable)
    frozen = root / "writing-research"
    shutil.copyfile(executable, frozen)
    frozen.chmod(0o555)
    require(sha(frozen) == identity, "operator binary changed while freezing")
    checks = []
    for owner in ("astrid", "minime"):
        f = Fixture(root / owner, owner, helper, helper)
        prepared, _ = once(f, helper, "start", "WRITE START Synthetic inspection only")
        passage = "Exact synthetic operator passage with \u03bb."
        f.deliver(helper, prepared, passage + "\nNEXT: WRITE FINISH")
        once(f, helper, "finish", "WRITE FINISH")
        before = file_hashes(f.state)
        command = [str(frozen), "--state-directory", str(f.state), "--owner", owner]

        def run(arguments, rejected=False):
            result = subprocess.run(command + arguments, capture_output=True, text=True, timeout=30)
            require((result.returncode != 0) == rejected, "unexpected operator reader status")
            if rejected:
                require(not result.stdout and passage not in result.stderr, "rejection leaked selected prose")
                return None
            return json.loads(result.stdout)

        listing = run(["list", "--page-size", "1"])
        require(listing["total_drafts"] == 1 and passage not in json.dumps(listing),
                "operator list is not metadata-only")
        snapshot = listing["snapshot_sha256"]
        selected = run(["show", "--draft", "d1", "--revision", snapshot, "passages"])
        require(selected["parts"] == [passage], "operator selection changed exact authored passage")
        run(["show", "--draft", "d1", "--revision", "0" * 64, "passages"], rejected=True)
        require(file_hashes(f.state) == before, "operator browsing changed canonical state")
        checks.extend(owner + " " + name for name in (
            "metadata-only listing", "exact selected passage", "stale snapshot refusal", "unchanged state bytes"))
    require(identity == sha(executable) == sha(frozen), "operator binary identity drift")
    return dict(checks=checks, executable=str(frozen), binary_sha256=identity,
                synthetic_only=True, live_state_opened=False)


CUE_PROBE = r'''
import hashlib, importlib.util, json, pathlib, sys
root, old_file, snapshot, fixture_file = map(pathlib.Path, sys.argv[1:])
def guard(event, args):
    if event in {'socket.connect', 'socket.bind', 'os.system', 'subprocess.Popen', 'os.kill'}:
        raise RuntimeError('cue qualification forbids network, processes and signals')
sys.addaudithook(guard)
from minime_autonomy import afterimages as new
assert pathlib.Path(new.__file__).resolve() == snapshot / 'minime_autonomy/afterimages.py'
spec = importlib.util.spec_from_file_location('old_afterimages_fixture', old_file)
old = importlib.util.module_from_spec(spec)
spec.loader.exec_module(old)
fixture = json.loads(fixture_file.read_bytes())
now = fixture['anchor_unix_ms']
checks = []
def check(value, name):
    if not value:
        raise ValueError(name)
    checks.append(name)
for owner in ('astrid', 'minime'):
    pair = root / owner
    minime, workspace = pair / 'minime', pair / owner
    previous = old.AfterimageStore(workspace, minime, owner)
    source = previous.archive / '2026-09-07' / (fixture['id'] + '.json')
    old.atomic_json(source, fixture)
    rows = []
    for index in range(4):
        row = previous._summary(fixture)
        row['id'] = fixture['id'] if index == 0 else fixture['id'] + '_' + str(index)
        row['anchor_unix_ms'] -= index * 1000
        rows.append(row)
    old.atomic_json(previous.archive / 'recent.json', rows)
    archive = {str(p.relative_to(previous.archive)): p.read_bytes()
               for p in previous.archive.rglob('*') if p.is_file()}
    previous.set_cues(True)
    for index in range(3):
        offered = previous.prepare_cue('old-' + str(index), timestamp_ms=now)
    check(offered is not None, owner + ' old opportunity genuinely selected')
    current = new.AfterimageStore(workspace, minime, owner)
    check(current.prepare_cue('old-2', timestamp_ms=now+1000) == offered,
          owner + ' cached old opportunity replays exactly')
    for index in range(3):
        fresh = current.prepare_cue('new-' + str(index), timestamp_ms=now+1000)
    check(fresh is not None and fresh['presentation_policy'] == 'selective_afterimage_cue_v1',
          owner + ' new opportunity establishes new presentation identity')
    restarted = new.AfterimageStore(workspace, minime, owner)
    for index in range(3):
        quiet = restarted.prepare_cue('quiet-' + str(index), timestamp_ms=now+2000)
    check(quiet is None, owner + ' unchanged coverage stays quiet across IDs and restart')
    check(restarted.prepare_cue('new-2', timestamp_ms=now+2000) == fresh,
          owner + ' new opportunity retries exactly')
    check(new.read_json(current.private / 'cues.json')['enabled'] is True,
          owner + ' enabled preference retained')
    check('cadence requirement not met' in restarted.open_page(fixture['id'])['text'],
          owner + ' explicit historical detail remains retrievable')
    check(archive == {str(p.relative_to(previous.archive)): p.read_bytes()
                     for p in previous.archive.rglob('*') if p.is_file()},
          owner + ' archive byte-exact after quieting')
    note = current.keep('Synthetic deliberately retained words.', timestamp_ms=now+3000)
    for index in range(3):
        authored = current.prepare_cue('authored-' + str(index), timestamp_ms=now+4000)
    check(authored is not None and authored['id'] == note['id'],
          owner + ' new authored note remains eligible')
    state = new.read_json(current.private / 'cues.json')
    check(all(len(key) == 64 for key in state['presentation_opportunities_v1']),
          owner + ' new deduplication state contains hash keys, not prose')
    current.set_cues(False)
    check(current.prepare_cue('disabled', timestamp_ms=now+5000) is None,
          owner + ' disabled preference remains effective')
print(json.dumps(dict(checks=checks, loaded_source_sha256=hashlib.sha256(
    pathlib.Path(new.__file__).read_bytes()).hexdigest(), activation_performed=False,
    synthetic_only=True, verified_model_delivery=False)))
'''


def cue_probe(old_source, snapshot, fixture, root):
    root.mkdir(mode=0o700)
    old_file = root / "old-afterimages.py"
    frozen_fixture = root / "transition-afterimage.json"
    inputs = {str(old_source): sha(old_source), str(fixture): sha(fixture)}
    for source, target in ((old_source, old_file), (fixture, frozen_fixture)):
        with source.open("rb") as incoming, target.open("xb") as outgoing:
            shutil.copyfileobj(incoming, outgoing)
        target.chmod(0o444)
        require(sha(target) == inputs[str(source)], "cue fixture changed while copying")
    (root / "home").mkdir()
    env = {key: os.environ[key] for key in ("PATH", "TMPDIR") if key in os.environ}
    env.update(HOME=str(root / "home"), PYTHONPATH=str(snapshot), PYTHONDONTWRITEBYTECODE="1")
    result = subprocess.run([sys.executable, "-B", "-c", CUE_PROBE, str(root),
                             str(old_file), str(snapshot), str(frozen_fixture)],
                            cwd=snapshot, env=env, capture_output=True, text=True, timeout=90)
    (root / "probe.stdout").write_text(result.stdout)
    (root / "probe.stderr").write_text(result.stderr)
    require(result.returncode == 0, "frozen cue probe failed; inspect retained probe.stderr")
    require(inputs == {str(old_source): sha(old_source), str(fixture): sha(fixture)}, "cue source drift")
    report = json.loads(result.stdout)
    require(report["loaded_source_sha256"] == sha(snapshot / "minime_autonomy/afterimages.py"),
            "cue probe loaded an unqualified source")
    return {**report, "inputs": inputs, "fixture_hashes": file_hashes(root)}


def qualify(old, new, minime, old_minime, fixture, reconciliation, operator, out):
    stages = {"old": verify_stage(old), "new": verify_stage(new)}
    inputs, old_inputs = inventory(minime), inventory(old_minime)
    binding = verify_launch_binding(reconciliation, inputs)
    names = set(QUALIFIER_INPUTS) | {Path(__file__).name, "qualify_writing_continuity.py",
                                   "qualify_writing_paired_release.py"}
    sources = {name: sha(Path(__file__).with_name(name)) for name in sorted(names)}
    out.mkdir(mode=0o700)
    try:
        require(freeze_python(minime, out / "minime-source") == inputs, "snapshot input drift")
        readers = [stage / "helpers/astrid-source-study" for stage in (old, new)]
        readers_state = [owner_preservation(out / (owner + "-reader"), owner, *readers)
                         for owner in ("astrid", "minime")]
        writing_state = [writing_preservation(out / (owner + "-writing"), owner, *readers)
                         for owner in ("astrid", "minime")]
        bridge = bridge_checkpoint_probe(old, new, out / "bridge-checkpoint")
        handoff = adapter_probe(out / "minime-source", new, out / "adapter-handoff")
        writing = writing_probe(out / "minime-source", new, out / "adapter-writing")
        cues = cue_probe(old_minime / "minime_autonomy/afterimages.py", out / "minime-source",
                         fixture, out / "cue-preservation")
        inspection = operator_probe(out / "operator-reader", operator, readers[1])
        invalid = invalid_state_probes(out, *readers)
        retries = preparation_retry_probe(out, readers[1])
        require(inventory(minime) == inputs == inventory(out / "minime-source"), "Minime input drift")
        require(inventory(old_minime) == old_inputs, "old Minime source drift")
        require(verify_launch_binding(reconciliation, inputs) == binding, "launch binding drift")
        require(stages == {"old": verify_stage(old), "new": verify_stage(new)}, "stage drift")
        require(sources == {name: sha(Path(__file__).with_name(name)) for name in sources}, "qualifier drift")
        receipt = dict(schema="writing_context_paired_qualification_v1", status="passed",
                       stages=stages, selected_inputs=inputs, old_inputs=old_inputs, launch_binding=binding,
                       qualifier_inputs=sources, readers=readers_state, writing_state=writing_state,
                       bridge_checkpoint=bridge, handoff=handoff, writing=writing, cues=cues,
                       operator_reader=inspection,
                       invalid_state_probes=invalid, preparation_retries=retries,
                       checks=sum(len(x["checks"]) for x in readers_state + writing_state)
                       + len(writing["checks"]) + len(cues["checks"]) + len(inspection["checks"]),
                       activation_performed=False, live_eligible_now=False,
                       capacity_qualification_included=False,
                       scope="Immutable artifacts, same-schema synthetic state and stubbed providers; fresh rollout approval required")
        write_json(out / "qualification.json", receipt)
        return receipt
    except Exception as error:
        write_json(out / "failure.json", dict(error=str(error), activation_performed=False))
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("old-stage", "new-stage", "minime-source", "old-minime-source",
                 "cue-fixture", "reconciliation", "research-reader", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    result = qualify(args.old_stage.resolve(strict=True), args.new_stage.resolve(strict=True),
                     args.minime_source.resolve(strict=True), args.old_minime_source.resolve(strict=True),
                     args.cue_fixture.resolve(strict=True), args.reconciliation.resolve(strict=True),
                     args.research_reader.resolve(strict=True), args.out.absolute())
    print(json.dumps({key: result[key] for key in ("status", "checks", "activation_performed")}))


if __name__ == "__main__":
    main()
