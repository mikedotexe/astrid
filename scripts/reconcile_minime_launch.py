#!/usr/bin/env python3
"""Freeze a reviewed agent-only overlay of the actual canonical launch sources.

No canonical writes, launch selection, service signals or activation authority.
The output is an offline qualification snapshot, not a relocatable installation.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import plistlib
import shutil

from qualify_geometry_release import inventory, require, sha, write_json

OVERLAY = (
    "minime_autonomy/action_vocabulary.py",
    "minime_autonomy/activity_focus.py",
    "minime_autonomy/parsing.py",
    "minime_autonomy/runtime.py",
    "minime_autonomy/source_study.py",
    "minime_autonomy/writing.py",
    "scripts/launchd_autonomous_agent.sh",
)
PRESERVE_CANONICAL = ("visual_frame_service.py",)
ASSETS = ("minime_autonomy/envelope_registry_seed.json",)
LAUNCHER = "scripts/launchd_autonomous_agent.sh"
PLIST = "launchd/com.minime.autonomous-agent.plist"


def checked_copy(source, target, expected):
    require(source.resolve() == source and not source.is_symlink(), "symlink source refused")
    require(source.is_file() and source.stat().st_size <= 8 * 1024 * 1024, "invalid source")
    target.parent.mkdir(parents=True, exist_ok=True)
    with source.open("rb") as incoming, target.open("xb") as outgoing:
        shutil.copyfileobj(incoming, outgoing)
        outgoing.flush()
        import os
        os.fsync(outgoing.fileno())
    require(sha(target) == expected, "source changed during snapshot")
    target.chmod(0o444)


def reconcile(canonical, candidate, installed_plist, source_status, output):
    canonical, candidate = canonical.resolve(), candidate.resolve()
    require(canonical != candidate, "canonical and candidate must be distinct")
    output = output.resolve()
    require(not output.is_relative_to(canonical) and not output.is_relative_to(candidate),
            "output must be outside both input trees")
    before, proposed = inventory(canonical), inventory(candidate)
    changed = {p for p in before.keys() | proposed.keys() if before.get(p) != proposed.get(p)}
    require(changed <= set(OVERLAY + PRESERVE_CANONICAL),
            f"unreviewed source differences: {sorted(changed - set(OVERLAY + PRESERVE_CANONICAL))}")
    require(all(p in proposed for p in OVERLAY), "required candidate input missing")
    installed = plistlib.loads(installed_plist.read_bytes())
    require(plistlib.loads((canonical / PLIST).read_bytes()) == installed,
            "installed launch plist differs from canonical")
    require(installed.get("ProgramArguments") == ["/bin/bash", str(canonical / LAUNCHER)]
            and installed.get("KeepAlive") is True, "unsupported launch binding")
    # This is a source-selection observation, not an idle/drain/readiness receipt.
    status = json.loads(source_status.read_bytes())
    require(status.get("source_inputs_at_start") == before and status.get("reload_required") is False,
            "running source identity differs from canonical; review before reconciling")
    require(status.get("source_path") == str(canonical / "minime_autonomy/runtime.py"),
            "running agent is not bound to the canonical runtime")
    selected = {**before, **{p: proposed[p] for p in OVERLAY}}
    asset_hashes = {p: sha(canonical / p) for p in ASSETS}
    require(all(sha(candidate / p) == digest for p, digest in asset_hashes.items()),
            "asset differences require separate review")
    bindings = {"installed_plist_sha256": sha(installed_plist),
                "canonical_root": str(canonical), "candidate_root": str(candidate),
                "observed_pid": status.get("pid"), "source_checked_at": status.get("checked_at"),
                "launch_arguments": installed["ProgramArguments"]}
    output.mkdir(mode=0o700)
    tree = output / "source"
    tree.mkdir(mode=0o700)
    try:
        for path, digest in sorted(selected.items()):
            root = candidate if path in OVERLAY else canonical
            checked_copy(root / path, tree / path, digest)
        for path, digest in asset_hashes.items():
            checked_copy(canonical / path, tree / path, digest)
        require(inventory(tree) == selected, "selected snapshot inventory differs")
        require(inventory(canonical) == before and inventory(candidate) == proposed,
                "source drift during reconciliation")
        require(sha(installed_plist) == bindings["installed_plist_sha256"], "launch binding drift")
        require(all(sha(canonical / p) == digest == sha(candidate / p)
                    for p, digest in asset_hashes.items()), "asset drift")
        receipt = {
            "schema": "minime_launch_source_reconciliation_v1",
            "status": "source_overlay_reconciled_not_activated",
            "live_eligible_now": False, "canonical_writes": False,
            "bindings": bindings, "canonical_inputs": before, "candidate_inputs": proposed,
            "selected_inputs": selected, "assets": asset_hashes,
            "overlay_paths": list(OVERLAY), "preserved_canonical_paths": list(PRESERVE_CANONICAL),
            "snapshot_root": str(tree),
            "remaining_gates": ["qualify scheduler interruption and full paired tests",
                "one coordinator applies only reviewed agent files during cooperative drain",
                "restart wrapper must verify selected_inputs against canonical before signaling",
                "verify fresh process/helper identities, readiness and unchanged protected services"],
        }
        write_json(output / "reconciliation.json", receipt)
        for directory in sorted((p for p in tree.rglob("*") if p.is_dir()), reverse=True):
            directory.chmod(0o555)
        tree.chmod(0o555)
        return receipt
    except Exception as error:
        write_json(output / "failure.json", {"error": str(error), "live_eligible_now": False})
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--canonical", required=True, type=Path)
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--installed-plist", required=True, type=Path)
    parser.add_argument("--source-status", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    result = reconcile(args.canonical, args.candidate, args.installed_plist, args.source_status, args.out)
    print(json.dumps({"status": result["status"], "files": len(result["selected_inputs"]),
                      "receipt": str(args.out / "reconciliation.json"), "live_eligible_now": False}))


if __name__ == "__main__":
    main()
