#!/usr/bin/env python3
"""Qualify retained accounts against immutable releases, using synthetic state.

This freezes no live authored state and performs no service or activation action.
The Minime snapshot must already be reconciled with the installed launch inputs.
"""
from __future__ import annotations

import argparse
import copy
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

from bridge_stage import verify_stage
from qualify_geometry_release import (
    Fixture, SOURCE, file_hashes, inventory, require, sha, write_json,
)
from qualify_writing_admission_release import qualify as preserve_writing
from qualify_writing_continuity import once


def account_view(output):
    require(output["input_kind"] == "inquiry_review", "account is not an inquiry review")
    require(output.get("page") is None and output.get("question_id") is None,
            "inspection became source delivery or attached response mutation")
    require("notebook directives in this response are not saved" in output["system_prompt"],
            "inspection lacks its detached-response contract")
    return json.JSONDecoder().raw_decode(output["text"].split("\n\n", 1)[1])[0]


def preserved_reading(state):
    result = {key: copy.deepcopy(state.get(key)) for key in (
        "questions", "notebook", "bookmarks", "progress", "last_input",
        "cursor_owner", "reading_contexts",
    )}
    # Explicit review creates its ordinary navigation offer, never a page cursor.
    for context in result["reading_contexts"].values():
        context.pop("pending_navigation", None)
    return result


def account_preservation(root, owner, old, new):
    f = Fixture(root, owner, old, new)
    f.prepare(old, "SELF_STUDY QUESTION NEW Synthetic unresolved account?")
    prose = ("Synthetic contrary evidence.\n" + "\u03bb exact authored words. " * 450
             + "An unresolved synthetic ending.")
    page = f.prepare(old, f"SELF_STUDY OPEN {SOURCE} 1")
    f.deliver(old, page, prose + "\nNEXT: REST")
    f.prepare(old, "SELF_STUDY QUESTION PARK q1")
    f.prepare(old, "SELF_STUDY QUESTION NEW Synthetic unrelated inquiry?")
    pending, retry = once(f, old, "pending-source", f"SELF_STUDY OPEN {SOURCE} 80")
    before = f.checkpoint()
    hashes = file_hashes(f.state)
    f.check(f.call(new, **retry) == pending, "old pending page replays exactly")
    f.check(file_hashes(f.state) == hashes, "old pending replay preserves every byte")
    listed = f.prepare(new, "SELF_STUDY QUESTION ACCOUNTS q1")
    rows = account_view(listed)["newest_first"]
    f.check(len(rows) == 1 and prose not in listed["text"], "explicit listing is metadata-only")
    command = rows[0]["inspect_command"]
    inspected, request = once(f, new, "inspect-old-account", command)
    selected = account_view(inspected)
    f.check(selected["complete_when_recorded"] is True, "old account completeness retained")
    f.check(page["page"]["revision"]["sha256"] in selected["historical_origin"],
            "old source revision retained as historical origin")
    stable = file_hashes(f.state)
    f.check(f.call(new, **request) == inspected, "inspection request retries exactly")
    f.call(new, reject=True, **(request | {"action": "SELF_STUDY QUESTION ACCOUNTS q2"}))
    f.check(file_hashes(f.state) == stable, "conflicting request preserves all bytes")
    assembled = ""
    for number in range(1, selected["pages"] + 1):
        view = account_view(f.prepare(new, command + f" --page {number}"))
        f.check(view["start_byte"] == len(assembled.encode()), "UTF-8 page offset exact")
        f.check(len(view["text"].encode()) <= 4_000, "account page bounded")
        assembled += view["text"]
    f.check(assembled == prose, "exact old authored account restored without normalization")
    inspect_again, cached = once(f, new, "inspect-after-pages", command)
    copied = Fixture(root / "old-helper-copy", owner, old, new)
    shutil.copytree(f.state, copied.state)
    copied_before = file_hashes(copied.state)
    f.check(copied.call(old, **cached) == inspect_again,
            "old helper replays retained new inspection offer on isolated copy")
    f.check(file_hashes(copied.state) == copied_before, "old replay rewrites no new state")
    f.deliver(new, inspect_again,
              "STUDY_NOTE: Synthetic unsaved directive.\nSTUDY_QUESTION: Do not replace.\nNEXT: REST")
    f.check(preserved_reading(f.checkpoint()) == preserved_reading(before),
            "inspection and response preserve both inquiries, notebooks and source positions")
    f.check(f.prepare(new, "SELF_STUDY CONTINUE") == pending,
            "unrelated original complete pending input restored by explicit continuation")
    f.check(f.call(new, **retry) == pending, "original pending request identity remains valid")
    response = "Synthetic original request completed after inspection.\nNEXT: REST"
    completed = f.deliver(new, pending, response)
    saved = file_hashes(f.state)
    f.check(f.deliver(new, pending, response) == completed, "original completion replays exactly")
    f.check(file_hashes(f.state) == saved, "replayed completion cannot advance or append twice")
    f.prepare(new, "SELF_STUDY QUESTION PARK q2")
    for action in ("INTROSPECT", "SELF_STUDY MAP"):
        output = f.prepare(new, action)
        f.check(prose not in output["text"], "parked account does not become ambient context")
    f.check(not (f.workspace / "journal").exists(), "inspection creates no automatic public journal")
    f.check(f.checkpoint()["version"] == 13, "reader schema remains thirteen")
    return dict(owner=owner, checks=f.checks, state_hashes=file_hashes(f.state))


FROZEN_TESTS = r'''
import hashlib, json, pathlib, runpy, sys
snapshot, tests, output = map(pathlib.Path, sys.argv[1:])
sys.path[:0] = [str(snapshot), str(tests)]
runpy.run_path(str(tests / 'tests/conftest.py'))
import autonomous_agent as aa
import continuity_control_plane as ccp
from minime_autonomy import runtime_actions
import pytest
def origins():
    found = {}
    for name, module in list(sys.modules.items()):
        if name in {'autonomous_agent', 'continuity_control_plane', 'minime_autonomy'} or name.startswith('minime_autonomy.'):
            origin = getattr(getattr(module, '__spec__', None), 'origin', None)
            assert origin and pathlib.Path(origin).resolve().is_relative_to(snapshot), (name, origin)
            path = pathlib.Path(origin).resolve()
            found[str(path.relative_to(snapshot))] = hashlib.sha256(path.read_bytes()).hexdigest()
    assert pathlib.Path(aa.__file__).resolve() == snapshot / 'autonomous_agent.py'
    assert aa.BASE_DIR.resolve() == snapshot
    assert runtime_actions.AutonomousAgent is aa.AutonomousAgent
    assert pathlib.Path(ccp.__file__).resolve() == snapshot / 'continuity_control_plane.py'
    from minime_autonomy import runtime as runtime_module
    assert runtime_module.retired_lexical_session_draft is ccp.retired_lexical_session_draft
    runtime = snapshot / 'minime_autonomy/runtime.py'
    assert aa.runtime_source_path().resolve() == runtime
    agent = aa.AutonomousAgent.__new__(aa.AutonomousAgent)
    entry = next(item for item in agent._introspect_source_entries()
                 if item['source'] == 'autonomous_agent.py')
    assert entry['path'].resolve() == runtime
    return found
before = origins()
layout = ('test_root_facade_preserves_public_module_identity',
          'test_source_monitoring_and_self_study_use_runtime_implementation')
result = pytest.main(['-q', '--tb=short', '-p', 'no:cacheprovider',
    '-k', ' and '.join('not ' + name for name in layout),
    '--junitxml=' + str(output / 'tests.xml'), str(tests / 'tests')])
after = origins()
(output / 'loaded-inputs.json').write_text(json.dumps(dict(
    before=before, after=after, pytest_exit_code=int(result),
    release_relative_identity_checks='passed_before_and_after_suite',
    checkout_relative_tests_replaced=layout), indent=2))
raise SystemExit(int(result))
'''


def test_inventory(root):
    return {str(p.relative_to(root)): sha(p) for p in sorted((root / "tests").rglob("*"))
            if p.is_file() and "__pycache__" not in p.parts}


def frozen_tests(snapshot, tests, astrid, helper, out):
    inputs, test_inputs = inventory(snapshot), test_inventory(tests)
    dependencies = [p for p in sys.path if p and Path(p).name in {"site-packages", "dist-packages"}]
    env = {key: os.environ[key] for key in ("PATH", "HOME", "TMPDIR") if key in os.environ}
    env.update(PYTHONDONTWRITEBYTECODE="1",
               PYTHONPATH=os.pathsep.join([str(snapshot), *dependencies]),
               ASTRID_SOURCE_ROOT=str(astrid), ASTRID_SOURCE_STUDY_BIN=str(helper))
    with (out / "tests.stdout").open("x") as stdout, (out / "tests.stderr").open("x") as stderr:
        result = subprocess.run([sys.executable, "-B", "-c", FROZEN_TESTS,
                                 str(snapshot), str(tests), str(out)],
                                cwd=out, env=env, stdout=stdout, stderr=stderr, timeout=1200)
    require(result.returncode == 0, "frozen Minime suite failed; retain tests.stdout/tests.stderr")
    loaded = json.loads((out / "loaded-inputs.json").read_bytes())
    require(all(inputs.get(p) == h for view in ("before", "after") for p, h in loaded[view].items()),
            "loaded Minime identity mismatch")
    require(inputs == inventory(snapshot) and test_inputs == test_inventory(tests),
            "frozen source or test drift")
    return dict(loaded=loaded, tests=test_inputs, results_sha256=sha(out / "tests.xml"))


def qualify(old, new, source, reconciliation, operator, tests, astrid, out,
            *, navigation_candidate_included=False):
    out.mkdir(mode=0o700)
    try:
        stages = {"old": verify_stage(old), "new": verify_stage(new)}
        qualifier = sha(Path(__file__))
        binding_sha = sha(reconciliation)
        assets = json.loads(reconciliation.read_bytes())["assets"]
        require(all(sha(source / name) == digest for name, digest in assets.items()), "asset mismatch")
        base = preserve_writing(old, new, source, reconciliation, operator, out / "preservation")
        owners = [account_preservation(out / owner, owner, old / "helpers/astrid-source-study",
                                       new / "helpers/astrid-source-study")
                  for owner in ("astrid", "minime")]
        tested = frozen_tests(source, tests, astrid, new / "helpers/astrid-source-study", out)
        require(all(sha(source / name) == digest for name, digest in assets.items()), "asset drift")
        require(base["selected_inputs"] == inventory(source), "Minime source drift")
        require(binding_sha == sha(reconciliation), "reconciliation drift")
        require(stages == {"old": verify_stage(old), "new": verify_stage(new)}, "stage drift")
        require(qualifier == sha(Path(__file__)), "qualifier drift")
        receipt = dict(schema="retained_accounts_paired_qualification_v1", status="passed",
                       stages=stages, qualifier_sha256=qualifier,
                       reconciliation_sha256=binding_sha, selected_inputs=base["selected_inputs"],
                       assets=assets, owners=owners, frozen_tests=tested,
                       preservation_sha256=sha(out / "preservation/qualification.json"),
                       checks=base["checks"] + sum(len(row["checks"]) for row in owners),
                       activation_performed=False, live_eligible_now=False,
                       capacity_qualification_included=False,
                       navigation_candidate_included=navigation_candidate_included,
                       scope="Synthetic old/new state and stubbed providers; no live authored state or services")
        write_json(out / "qualification.json", receipt)
        return receipt
    except Exception as error:
        write_json(out / "failure.json", dict(error=str(error), activation_performed=False,
                                             live_eligible_now=False))
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("old-stage", "new-stage", "minime-source", "reconciliation",
                 "research-reader", "minime-tests", "astrid-source", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    result = qualify(*(getattr(args, name).resolve(strict=True) for name in (
        "old_stage", "new_stage", "minime_source", "reconciliation",
        "research_reader", "minime_tests", "astrid_source")), args.out.absolute())
    print(json.dumps({key: result[key] for key in ("status", "checks", "activation_performed")}))


if __name__ == "__main__":
    main()
