#!/usr/bin/env python3
"""Same-schema paired helper qualification using synthetic state, never live writing."""
from pathlib import Path
import argparse
import json

from bridge_stage import verify_stage
from qualify_geometry_release import (
    AUTHORED, PRIVATE, SOURCE, Fixture, file_hashes, helper_selection_probe,
    inventory, require, sha, verify_launch_binding, write_json,
)


def qualify(old, new, minime, reconciliation, out):
    stages = {"old": verify_stage(old), "new": verify_stage(new)}
    inputs = inventory(minime)
    binding = verify_launch_binding(reconciliation, inputs)
    out.mkdir(mode=0o700)
    results = []
    try:
        for owner in ("astrid", "minime"):
            f = Fixture(out / owner, owner, old / "helpers/astrid-source-study",
                        new / "helpers/astrid-source-study")
            alias = f.root / "astrid/crates/astrid-kernel/src/lib.rs"
            alias.parent.mkdir(parents=True)
            alias.write_text("pub fn synthetic() {}\n")
            f.prepare(f.old, "SELF_STUDY QUESTION NEW Synthetic retained question?")
            page = f.prepare(f.old, f"SELF_STUDY OPEN {SOURCE} 1")
            f.deliver(f.old, page, f"STUDY_NOTE: {AUTHORED}\nNEXT: INTROSPECT")
            draft = f.prepare(f.old, "WRITE START Synthetic private continuation")
            f.deliver(f.old, draft, PRIVATE + "\nNEXT: WRITE CONTINUE")
            private = f.prepare(f.old, "WRITE CONTINUE")
            pending = f.prepare(f.old, "SELF_STUDY CONTINUE")
            before = f.checkpoint()
            f.check(before["version"] == 12, "old helper creates current schema twelve")
            f.check(f.prepare(f.new, "SELF_STUDY CONTINUE") == pending,
                    "pending source offer remains byte-exact")
            f.check(f.prepare(f.new, "WRITE CONTINUE") == private,
                    "pending private offer remains byte-exact")
            reflection = f.prepare(f.new, "INTROSPECT")
            f.check(reflection["input_kind"] == "reflection" and reflection.get("page") is None,
                    "bare introspection supplies reflection without a source page")
            f.deliver(f.new, reflection, "Synthetic open reflection.\nNEXT: SELF_STUDY CONTINUE")
            for verb in ("MAP", "LIST"):
                listing = f.prepare(f.new, f"SELF_STUDY {verb} astrid/crates/astrid_kernel/src")
                f.check(listing["input_kind"] == "map" and "Directory spelling" in listing["text"],
                        f"{verb} supplies the disclosed canonical directory")
                f.check("SELF_STUDY OPEN astrid/crates/astrid-kernel/src/lib.rs 1" in listing["text"],
                        f"{verb} supplies a valid concrete open command")
            after = f.checkpoint()
            f.check(after["version"] == before["version"], "no schema migration")
            for key in ("questions", "notebook", "bookmarks"):
                f.check(after[key] == before[key], f"{key} survives reflection and navigation unchanged")
            f.check(f.prepare(f.new, "SELF_STUDY CONTINUE") == pending,
                    "reflection and navigation do not replace pending source input")
            f.deliver(f.new, pending, "Synthetic source continuation.\nNEXT: REST")
            f.check(f.checkpoint()["bookmarks"][SOURCE]["end"] == pending["page"]["end"],
                    "accepted pending source delivery advances the exact page")
            f.deliver(f.new, private, "Synthetic private continuation.\nNEXT: REST")
            f.check(PRIVATE in f.prepare(f.new, "WRITE CONTINUE")["text"],
                    "private original prose survives accepted continuation")
            f.prepare(f.old, "SELF_STUDY MAP")
            f.check(f.checkpoint()["version"] == 12, "old helper retains same-schema readability")
            f.check(not (f.workspace / "journal").exists(), "fixture creates no public journal")
            results.append(dict(owner=owner, checks=f.checks, state_hashes=file_hashes(f.state)))
        selection = helper_selection_probe(minime, new, f.root)
        require(inventory(minime) == inputs, "Minime snapshot drift")
        require(verify_launch_binding(reconciliation, inputs) == binding, "launch binding drift")
        require(stages == {"old": verify_stage(old), "new": verify_stage(new)}, "stage drift")
        result = dict(schema="reflection_paired_qualification_v1", status="passed",
                      owners=results, checks=sum(len(row["checks"]) for row in results),
                      stages=stages, selected_inputs=inputs, launch_binding=binding,
                      helper_selection=selection, qualifier_sha256=sha(Path(__file__)),
                      activation_performed=False,
                      scope="Synthetic owner state; no inference, live state or service changes")
        write_json(out / "qualification.json", result)
        return result
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
                          receipt=str(args.out / "qualification.json"))))


if __name__ == "__main__":
    main()
