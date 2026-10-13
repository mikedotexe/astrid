#!/usr/bin/env python3
"""Freeze a drain candidate and qualify unchanged paired reader/state surfaces.

No live state, signals or providers. Runtime interruption tests are separate;
this packet cannot authorize installation or turn a v1 process into v2.
"""
import argparse
from pathlib import Path

from bridge_stage import verify_stage
from qualify_eof_release import owner_preservation
from qualify_geometry_release import freeze_python, inventory, require, sha, write_json
from qualify_mailbox_context_release import adapter_probe, bridge_checkpoint_probe


def qualify(source, stage, out):
    ready = verify_stage(stage)
    scripts = Path(__file__).parent
    dependencies = ("qualify_minime_drain.py", "qualify_geometry_release.py", "qualify_eof_release.py",
                    "qualify_mailbox_context_release.py", "qualify_private_delivery_recovery.py",
                    "qualify_source_selection_release.py", "bridge_stage.py")
    qualifier_inputs = {name: sha(scripts / name) for name in dependencies}
    out.mkdir(mode=0o700)
    try:
        frozen = out / "minime-source"
        inputs = freeze_python(source, frozen)
        reader = stage / "helpers/astrid-source-study"
        owners = [owner_preservation(out / owner, owner, reader, reader)
                  for owner in ("astrid", "minime")]
        adapter = adapter_probe(frozen, stage, out / "adapter")
        checkpoint = bridge_checkpoint_probe(stage, stage, out / "bridge-checkpoint")
        require(inventory(source) == inputs == inventory(frozen), "candidate changed during qualification")
        require(verify_stage(stage) == ready, "retained stage changed during qualification")
        require(qualifier_inputs == {name: sha(scripts / name) for name in dependencies},
                "qualifier source changed during qualification")
        report = {"schema": "minime_drain_paired_qualification_v1", "status": "passed",
                  "source": str(source), "snapshot": str(frozen), "selected_inputs": inputs,
                  "stage": str(stage), "stage_identity": ready, "helper_sha256": sha(reader),
                  "qualifier_inputs": qualifier_inputs,
                  "owners": owners, "checks": sum(len(owner["checks"]) for owner in owners),
                  "adapter": adapter, "checkpoint": checkpoint,
                  "schema_migration": False, "activation_performed": False, "live_eligible_now": False,
                  "remaining_gates": ["runtime interruption and operator refusal qualification",
                      "reviewed source installation and v1 idle-only bootstrap",
                      "exact loaded v2 identity before any busy drain"]}
        write_json(out / "qualification.json", report)
        return report
    except Exception as error:
        write_json(out / "failure.json", {"error_type": type(error).__name__, "activation_performed": False})
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source", "stage", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    result = qualify(args.source.resolve(strict=True), args.stage.resolve(strict=True), args.out.absolute())
    print(f"{result['checks']} preservation checks passed; no activation performed")


if __name__ == "__main__":
    main()
