#!/usr/bin/env python3
"""Offline paired artifact and synthetic migration qualification; never activation.

Uses the old and candidate staged executables, a frozen Minime source inventory,
and newly generated owner fixtures. No live checkpoint is opened or restored.
The Python snapshot is a qualification artifact, not a launchd release selector.
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
import time

from bridge_stage import verify_stage

SOURCE = "astrid/crates/example/src/lib.rs"
AUTHORED = "Synthetic exact passage: an unresolved connection, not a conclusion."
PRIVATE = "Synthetic private passage. Keep these exact words."


def sha(path):
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("x", encoding="utf-8") as handle:
        os.chmod(path, 0o600)
        json.dump(value, handle, sort_keys=True, indent=2)
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())


def require(condition, message):
    if not condition:
        raise ValueError(message)


def inventory(root):
    # Match the agent's existing source_inputs contract without importing the agent.
    paths = set(root.glob("*.py"))
    for package in ("minime_autonomy", "mikemind"):
        paths.update((root / package).rglob("*.py"))
    paths.update(root / name for name in (
        "scripts/launchd_autonomous_agent.sh",
        "scripts/minime_rescue_investigation.py",
        "launchd/com.minime.autonomous-agent.plist",
    ))
    result = {}
    for path in sorted(paths):
        relative = path.relative_to(root)
        require(path.resolve() == path and not path.is_symlink(),
                "source symlink or path escape")
        require(path.is_file() and path.stat().st_size <= 8 * 1024 * 1024,
                "missing, nonregular or oversized source input")
        result[str(relative)] = sha(path)
    require(0 < len(result) <= 2000, "source inventory outside bound")
    return result


def freeze_python(source, destination):
    before = inventory(source)
    destination.mkdir(mode=0o700)
    for name, digest in before.items():
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source / name, target)
        target.chmod(0o444)
        require(sha(target) == digest, "Python input changed during copying")
    require(inventory(source) == before == inventory(destination),
            "Python inventory changed during snapshot")
    for directory in sorted((p for p in destination.rglob("*") if p.is_dir()), reverse=True):
        directory.chmod(0o555)
    destination.chmod(0o555)
    return before


def file_hashes(directory):
    return {str(p.relative_to(directory)): sha(p) for p in sorted(directory.rglob("*"))
            if p.is_file() and p.name != "reader.lock"}


class Fixture:
    def __init__(self, root, owner, old_reader, new_reader):
        self.root, self.owner = root, owner
        self.old, self.new = old_reader, new_reader
        self.workspace = root / "owner-workspace"
        self.state = self.workspace / "diagnostics/source_first_v3/shared_reader"
        (root / SOURCE).parent.mkdir(parents=True)
        (root / SOURCE).write_text("pub fn first() {}\npub fn second() { first(); }\n" +
                                   "// synthetic source line\n" * 500)
        (root / "minime/workspace/runtime").mkdir(parents=True)
        self.checks = []

    def check(self, condition, name):
        require(condition, f"{self.owner}: {name}")
        self.checks.append(name)

    def request(self, **operation):
        return dict(astrid_root=str(self.root / "astrid"),
                    minime_root=str(self.root / "minime"),
                    runtime_workspace=str(self.workspace), being=self.owner,
                    state_directory=str(self.state), **operation)

    def call(self, executable, *, reject=False, **operation):
        result = subprocess.run([str(executable)], input=json.dumps(self.request(**operation)),
                                capture_output=True, text=True, timeout=45, check=False)
        value = json.loads(result.stdout)
        failed = result.returncode != 0 or "error" in value
        require(failed == reject, f"unexpected helper result: {value.get('error', 'success')}")
        return value

    def prepare(self, executable, action, **options):
        return self.call(executable, operation="prepare", action=action, **options)

    def deliver(self, executable, output, text):
        page = output.get("page")
        key = "page_id" if page else "navigation_id"
        return self.call(executable, operation="delivered" if page else "navigation_delivered",
                         **{key: page["id"] if page else output[key]},
                         request_json=json.dumps({"messages": [{"role": "user", "content": output["text"]}]}),
                         response_json=json.dumps({"message": {"content": text}, "done": True, "done_reason": "stop"}))

    def checkpoint(self):
        return json.loads((self.state / "reader-v1.json").read_bytes())

    def geometry(self, operation, request_id="", expected_head=None):
        records = self.checkpoint()["questions"]["entries"]["q1"]["geometry"]["records"]
        head = records[-1]["id"] if records else "empty"
        return "SELF_STUDY GEOMETRY " + json.dumps(dict(question="q1", request_id=request_id,
                expected_head=head if expected_head is None else expected_head, operation=operation))

    def run(self):
        self.prepare(self.old, "SELF_STUDY QUESTION NEW Synthetic migration question?")
        opened = self.prepare(self.old, f"SELF_STUDY OPEN {SOURCE} 1")
        self.deliver(self.old, opened,
                     f"STUDY_NOTE: {AUTHORED}\nSTUDY_RELATION: hypothesis | {SOURCE}:1 | {SOURCE}:2 | {AUTHORED}")
        draft = self.prepare(self.old, "WRITE START synthetic continuity")
        self.deliver(self.old, draft, PRIVATE + "\nNEXT: WRITE CONTINUE")
        pending_draft = self.prepare(self.old, "WRITE CONTINUE")
        pending = self.prepare(self.old, "SELF_STUDY CONTINUE")
        legacy = self.checkpoint()
        self.check(legacy["version"] < 5, "old executable creates genuine pre-v5 checkpoint")
        self.check(json.loads((self.state / "source-findings-v1.json").read_bytes())["schema"] ==
                   "source_findings_sidecar_v2", "legacy relation compatibility floor present")
        original = file_hashes(self.state)
        shutil.copytree(self.state, self.root / "legacy-reader-copy")
        legacy_questions = legacy["questions"]
        restored = self.prepare(self.new, "SELF_STUDY CONTINUE")
        self.check(restored == pending, "pending page and exact complete input survive upgrade")
        current = self.checkpoint()
        self.check(current["version"] == 6, "new executable commits schema six")
        self.check(current["questions"]["active"] == legacy_questions["active"], "active question preserved")
        self.check(all(current["questions"]["entries"][key][field] == value
                       for key, question in legacy_questions["entries"].items()
                       for field, value in question.items()), "all legacy authored question fields preserved")
        self.check(not (self.state / "activity-focus-v1.json").exists(), "migration creates no focus window")
        self.deliver(self.new, restored, "Synthetic continuation remains unresolved.")
        writing = self.prepare(self.new, "WRITE CONTINUE")
        self.check(writing == pending_draft and PRIVATE in writing["text"],
                   "private pending delivery and exact prose preserved")
        self.deliver(self.new, writing, "Synthetic additional passage.\nNEXT: WRITE CONTINUE")
        self.check((self.state / "writing/drafts-v2.json").exists(), "native draft migration completed")
        for action in ("SELF_STUDY MAP", "WRITE CONTINUE"):
            before = file_hashes(self.state)
            self.prepare(self.old, action, reject=True)
            self.check(file_hashes(self.state) == before, f"old writer refuses {action} without changed bytes")
        self.check(file_hashes(self.root / "legacy-reader-copy") == original, "legacy archive untouched")
        self.prepare(self.new, "SELF_STUDY QUESTION q1")
        now = int(time.time() * 1000)
        frames = [dict(t_ms=1000 + i * 1000, wall_clock_unix_ms=now - 1000 + i * 1000,
                       summary=dict(finite_fraction=1.0), activations=[0.125] * 128) for i in range(2)]
        write_json(self.root / "minime/workspace/runtime/esn_activation_trace_v1.json", dict(
            policy="esn_activation_trace_v1", reservoir_dim=128, sample_interval_ms=1000,
            retained_secs=180, updated_at_unix_ms=now, frames=frames))
        action = self.geometry(dict(kind="capture", seconds=2, note="Synthetic chosen geometry."), "capture-a")
        self.prepare(self.new, action)
        history = self.checkpoint()["questions"]["entries"]["q1"]["geometry"]
        self.prepare(self.new, action)
        self.check(self.checkpoint()["questions"]["entries"]["q1"]["geometry"] == history,
                   "capture retry preserves original record")
        before = file_hashes(self.state)
        self.prepare(self.new, action.replace("Synthetic chosen geometry.", "Conflicting words."), reject=True)
        self.check(file_hashes(self.state) == before, "conflicting capture retry preserves bytes")
        self.prepare(self.new, "SELF_STUDY QUESTION PARK q1")
        unrelated = self.prepare(self.new, f"SELF_STUDY OPEN {SOURCE} 400")
        self.check("Synthetic chosen geometry." not in unrelated["text"], "parked evidence stays quiet")
        self.prepare(self.new, "SELF_STUDY QUESTION q1")
        self.prepare(self.new, self.geometry(dict(kind="export")))
        packet = next((self.state / "geometry-exports").glob("*.json"))
        self.check(PRIVATE not in packet.read_text(), "export excludes synthetic private draft")
        self.check(history == self.checkpoint()["questions"]["entries"]["q1"]["geometry"],
                   "explicit return preserves geometry history")
        before = file_hashes(self.state)
        self.prepare(self.old, "SELF_STUDY MAP", reject=True)
        self.check(file_hashes(self.state) == before, "old reader refuses geometry-bearing checkpoint unchanged")
        return dict(owner=self.owner, checks=self.checks, legacy_version=legacy["version"],
                    final_version=5, fixture_only=True, export_sha256=sha(packet),
                    reader_files=file_hashes(self.state))


def helper_selection_probe(snapshot, stage, root):
    selection = root / "astrid/.runtime/bridge-deployment/active.json"
    write_json(selection, dict(schema="bridge_release_selection_v1", stage=str(stage),
                              manifest_sha256=sha(stage / "manifest.json")))
    code = """
import json, pathlib, sys
from minime_autonomy.source_study import StudyClient, selected_reader
root, expected = map(pathlib.Path, sys.argv[1:])
selected = selected_reader(root / 'astrid')
assert selected == expected
client = StudyClient(root / 'minime', root / 'owner-workspace', astrid_root=root / 'astrid')
assert client.executable == expected
output = client.prepare('SELF_STUDY GEOMETRY {"question":"q1","operation":{"kind":"status"}}').output
assert output['input_kind'] == 'geometry'
print(json.dumps({'same_helper_selected': True, 'minime_geometry_delivery_prepared': True}))
"""
    env = {key: os.environ[key] for key in ("HOME", "PATH", "TMPDIR") if key in os.environ}
    env.update(PYTHONPATH=str(snapshot), PYTHONDONTWRITEBYTECODE="1")
    result = subprocess.run([sys.executable, "-B", "-c", code, str(root),
                             str(stage / "helpers/astrid-source-study")],
                            cwd=snapshot, env=env, capture_output=True, text=True, timeout=60)
    require(result.returncode == 0, f"frozen adapter selection failed: {result.stderr[-2000:]}")
    return json.loads(result.stdout)


def invalid_state_probes(output, old_reader, new_reader):
    results = []
    for name, change in (("future_schema", lambda value: {**value, "version": 999}),
                         ("corrupt_tail", None)):
        fixture = Fixture(output / f"invalid-{name}", "astrid", old_reader, new_reader)
        fixture.prepare(old_reader, "SELF_STUDY QUESTION NEW Synthetic invalid-state fixture?")
        checkpoint = fixture.state / "reader-v1.json"
        value = fixture.checkpoint()
        checkpoint.write_bytes(json.dumps(change(value)).encode() if change else checkpoint.read_bytes() + b"{")
        before = file_hashes(fixture.state)
        fixture.prepare(new_reader, "SELF_STUDY MAP", reject=True)
        require(file_hashes(fixture.state) == before, f"{name} was overwritten")
        results.append({"case": name, "rejected_without_writes": True})
    return results


def preparation_retry_probe(output, reader):
    results = []
    for owner in ("astrid", "minime"):
        fixture = Fixture(output / f"preparation-retry-{owner}", owner, reader, reader)
        action = "SELF_STUDY QUESTION NEW Synthetic lost-response retry?"
        revision = fixture.call(reader, operation="preparation_revision")["revision"]
        request = dict(operation="prepare_once", request_id="synthetic-event-1",
                       expected_revision=revision, action=action)
        first = fixture.call(reader, **request)
        second = fixture.call(reader, **request)
        require(first == second, "lost preparation acknowledgement did not replay exact output")
        require(len(fixture.checkpoint()["questions"]["entries"]) == 1, "retry duplicated question")
        before = file_hashes(fixture.state)
        fixture.call(reader, **(request | dict(action="SELF_STUDY MAP")), reject=True)
        fixture.call(reader, **(request | dict(request_id="stale-event")), reject=True)
        require(file_hashes(fixture.state) == before, "rejected preparation changed native history")
        revision = fixture.call(reader, operation="preparation_revision")["revision"]
        fixture.call(reader, **(request | dict(request_id="synthetic-event-2", expected_revision=revision)))
        require(len(fixture.checkpoint()["questions"]["entries"]) == 2, "new authored choice was deduplicated")
        results.append(dict(owner=owner, exact_retry=True, conflicting_retry_rejected=True,
                            stale_first_admission_rejected=True, deliberate_new_choice_preserved=True))
    return dict(fixture_only=True, preparation_has_operation_id_envelope=True,
                idempotent_retry_qualified=True, owners=results)


def verify_launch_binding(reconciliation, inputs):
    receipt_bytes = reconciliation.read_bytes()
    receipt = json.loads(receipt_bytes)
    require(isinstance(receipt, dict)
            and receipt.get("schema") == "minime_launch_source_reconciliation_v1"
            and receipt.get("selected_inputs") == inputs,
            "qualified Python inputs differ from launch reconciliation")
    require(inventory(Path(receipt["bindings"]["canonical_root"])) == receipt["canonical_inputs"],
            "canonical sources changed since reconciliation")
    return dict(receipt=str(reconciliation), receipt_sha256=hashlib.sha256(receipt_bytes).hexdigest(),
                exact_selected_inputs=True, canonical_applied=False)


def qualify(old_stage, stage, minime, output, reconciliation=None):
    old_ready, ready = verify_stage(old_stage), verify_stage(stage)
    output.mkdir(parents=True, exist_ok=False, mode=0o700)
    try:
        tool = output / "qualification-tool.py"
        shutil.copyfile(Path(__file__), tool)
        tool.chmod(0o444)
        inputs = freeze_python(minime, output / "minime-source")
        launch_binding = verify_launch_binding(reconciliation, inputs) if reconciliation is not None else None
        results = []
        for owner in ("astrid", "minime"):
            fixture = Fixture(output / f"fixture-{owner}", owner,
                              old_stage / "helpers/astrid-source-study",
                              stage / "helpers/astrid-source-study")
            results.append(fixture.run())
        selection = helper_selection_probe(output / "minime-source", stage, output / "fixture-minime")
        invalid = invalid_state_probes(output, old_stage / "helpers/astrid-source-study",
                                       stage / "helpers/astrid-source-study")
        retry = preparation_retry_probe(output, stage / "helpers/astrid-source-study")
        require(inventory(output / "minime-source") == inputs == inventory(minime),
                "Python candidate changed during qualification")
        require(verify_stage(old_stage) == old_ready and verify_stage(stage) == ready,
                "bridge stage changed during qualification")
        manifest = dict(schema="geometry_paired_qualification_v1", status="offline_checks_passed_not_activatable",
                        old_bridge=old_ready, bridge=ready, minime_source_root=str(minime),
                        minime_snapshot=str(output / "minime-source"), minime_inputs=inputs,
                        helper_sha256=sha(stage / "helpers/astrid-source-study"),
                        python_executable=sys.executable, python_version=sys.version,
                        qualifier_sha256=sha(Path(__file__)), owners=results, adapter=selection,
                        invalid_state_checks=invalid,
                        preparation_retry_probe=retry,
                        launch_source_reconciliation=launch_binding,
                        activation_blockers=["source-bound review of host scheduler interruption and activity-selector migration evidence",
                                             "reviewed Python overlay still requires cooperative canonical installation" if launch_binding
                                             else "canonical Python source and launch selection reconciliation"],
                        activation_performed=False, live_eligible_now=False,
                        boundaries=["synthetic state, not copied private/live state",
                                    "Python snapshot is not a launchd release selector or dependency lock",
                                    "host scheduler handoff tests are separate evidence from this helper migration packet",
                                    "old/new writers must not overlap; no restoration over newer authored state"])
        if reconciliation is not None:
            require(verify_launch_binding(reconciliation, inputs) == launch_binding,
                    "launch reconciliation changed during qualification")
        write_json(output / "qualification.json", manifest)
        return manifest
    except Exception as error:
        write_json(output / "failure.json", dict(error=str(error), activation_performed=False))
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--old-bridge-stage", type=Path, required=True)
    parser.add_argument("--bridge-stage", type=Path, required=True)
    parser.add_argument("--minime-source", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--launch-reconciliation", type=Path)
    args = parser.parse_args()
    result = qualify(args.old_bridge_stage.resolve(strict=True), args.bridge_stage.resolve(strict=True),
                     args.minime_source.resolve(strict=True), args.out.absolute(),
                     args.launch_reconciliation.resolve(strict=True) if args.launch_reconciliation else None)
    print(json.dumps({"status": result["status"], "checks": sum(len(o["checks"]) for o in result["owners"]),
                      "receipt": str(args.out / "qualification.json"), "activation_performed": False}, indent=2))


if __name__ == "__main__":
    main()
