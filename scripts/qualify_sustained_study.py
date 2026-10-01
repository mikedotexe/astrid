#!/usr/bin/env python3
"""Schema-11/12 helper qualification using synthetic owner stores, never live state."""
from pathlib import Path
import argparse
import json

from qualify_geometry_release import (
    Fixture, SOURCE, PRIVATE, AUTHORED, file_hashes, sha, write_json,
)


def qualify(old, new, out):
    old, new = old.resolve(strict=True), new.resolve(strict=True)
    identities = {"old": sha(old), "new": sha(new)}
    out.mkdir(mode=0o700)
    results = []
    try:
        for owner in ("astrid", "minime"):
            f = Fixture(out / owner, owner, old, new)
            f.prepare(old, "SELF_STUDY QUESTION NEW Synthetic inquiry?")
            opened = f.prepare(old, f"SELF_STUDY OPEN {SOURCE} 1")
            f.deliver(old, opened, f"STUDY_NOTE: {AUTHORED}\nNEXT: REST")
            draft = f.prepare(old, "WRITE START Synthetic private continuity")
            f.deliver(old, draft, PRIVATE + "\nNEXT: WRITE CONTINUE")
            private_pending = f.prepare(old, "WRITE CONTINUE")
            pending = f.prepare(old, "SELF_STUDY CONTINUE")
            before = f.checkpoint()
            f.check(before["version"] == 11, "old executable created schema 11")
            f.check(f.prepare(new, "SELF_STUDY CONTINUE") == pending,
                    "old source offer stays byte-exact")
            f.check(f.prepare(new, "WRITE CONTINUE") == private_pending,
                    "old private offer stays byte-exact")
            f.check(f.checkpoint()["questions"] == before["questions"],
                    "authored inquiry and note unchanged")
            f.check(f.checkpoint()["bookmarks"] == before["bookmarks"],
                    "no bookmark advance on upgrade")
            help_output = f.prepare(new, "SELF_STUDY HELP notebook")
            f.check(help_output["input_kind"] == "help" and help_output["continuation_decision"],
                    "help has reference kind and decision purpose")
            f.check("Synthetic inquiry?" not in help_output["text"],
                    "help does not resurface saved question")
            f.deliver(new, help_output, "STUDY_NOTE: Not a revision.\nNEXT: REST")
            f.check(f.checkpoint()["questions"] == before["questions"],
                    "help response cannot revise inquiries")
            f.check(f.checkpoint()["version"] == 12, "new helper persists schema 12")
            before_old = file_hashes(f.state)
            f.prepare(old, "SELF_STUDY MAP", reject=True)
            f.check(file_hashes(f.state) == before_old,
                    "old helper refuses new state without modifying bytes")
            f.deliver(new, pending, "A synthetic continuation.\nNEXT: REST")
            f.check(f.checkpoint()["bookmarks"][SOURCE]["end"] == pending["page"]["end"],
                    "retained source delivery advances exact old page")
            f.deliver(new, private_pending, "Private synthetic continuation.\nNEXT: REST")
            f.check(PRIVATE in f.prepare(new, "WRITE CONTINUE")["text"],
                    "private original prose survives accepted continuation")
            results.append({"owner": owner, "checks": f.checks})
        if identities != {"old": sha(old), "new": sha(new)}:
            raise RuntimeError("helper identity changed during qualification")
        result = {
            "status": "passed", "schema_transition": [11, 12], "owners": results,
            "checks": sum(len(r["checks"]) for r in results), "binary_sha256": identities,
            "script_sha256": sha(Path(__file__)), "activation_performed": False,
            "scope": "synthetic owner stores; no live-state copy, inference, service or control calls",
        }
        write_json(out / "qualification.json", result)
        return result
    except Exception as error:
        write_json(out / "failure.json", {"error": str(error), "activation_performed": False})
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--old-helper", type=Path, required=True)
    parser.add_argument("--new-helper", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(qualify(args.old_helper, args.new_helper, args.out), indent=2))


if __name__ == "__main__":
    main()
