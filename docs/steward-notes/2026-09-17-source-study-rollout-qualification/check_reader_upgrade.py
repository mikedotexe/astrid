"""Isolated old/new executable qualification; never open live reader state."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def qualify(old, candidate):
    with tempfile.TemporaryDirectory(prefix="study-upgrade-") as temporary:
        root = Path(temporary)
        source_root = root / "astrid"
        source = source_root / "crates/example/src/lib.rs"
        source.parent.mkdir(parents=True)
        source.write_text("pub fn caller() { callee(); }\npub fn callee() {}\n")
        source_id = "astrid/crates/example/src/lib.rs"
        directory = root / "state"

        def call(binary, **operation):
            request = {"roots": {"astrid": str(source_root)},
                       "state_directory": str(directory), **operation}
            result = subprocess.run([str(binary)], input=json.dumps(request),
                                    capture_output=True, text=True, timeout=60)
            body = json.loads(result.stdout)
            return result.returncode, body

        def checked(binary, **operation):
            code, body = call(binary, **operation)
            assert code == 0 and "error" not in body, body
            return body

        def accept(binary, output, words):
            request = json.dumps({"messages": [
                {"role": "system", "content": output["system_prompt"]},
                {"role": "user", "content": output["text"]}]})
            response = json.dumps({"message": {"content": words}, "done": True})
            page = output.get("page")
            operation = ({"operation": "delivered", "page_id": page["id"]} if page
                         else {"operation": "navigation_delivered",
                               "navigation_id": output["navigation_id"]})
            return checked(binary, **operation, request_json=request, response_json=response)

        def checkpoints():
            return {name: (directory / name).read_bytes() for name in
                    ("reader-v1.json", "source-findings-v1.json")}

        # A prepared old-version page is delivered exactly, not regenerated.
        offered = checked(old, operation="prepare", action=f"SELF_STUDY OPEN {source_id} 1")
        resumed = checked(candidate, operation="prepare", action="SELF_STUDY CONTINUE")
        assert resumed == offered, "pending old input changed across upgrade"
        accept(candidate, resumed, f"STUDY_FINDING: {source_id}:1 | Existing authored finding.")
        sidecar = directory / "source-findings-v1.json"
        assert json.loads(sidecar.read_bytes())["schema"] == "source_findings_sidecar_v1"
        checked(old, operation="prepare", action="SELF_STUDY MAP")

        offered = checked(candidate, operation="prepare", action="SELF_STUDY MAP")
        accept(candidate, offered,
               f"STUDY_RELATION: flow | {source_id}:1 | {source_id}:2 | The caller invokes callee.")
        saved = checkpoints()
        assert json.loads(saved["source-findings-v1.json"])["schema"] == "source_findings_sidecar_v2"
        code, error = call(old, operation="prepare", action="SELF_STUDY MAP")
        assert code != 0 and "unsupported source findings sidecar" in error.get("error", ""), error
        assert checkpoints() == saved, "old executable changed protected checkpoints"

        offered = checked(candidate, operation="prepare", action="SELF_STUDY MAP")
        assert "Authored call/data-flow claim" in offered["text"]
        # Even after explicit removal, an old writer must not revive stale state.
        state = json.loads((directory / "reader-v1.json").read_bytes())
        finding = state["notebook"]["source_findings"]["authored"][0]
        assert finding["relation"]["other"]["line"] == 2
        accept(candidate, offered, f"STUDY_FINDING_DROP: {finding['id']}")
        dropped = checkpoints()
        code, _ = call(old, operation="prepare", action="SELF_STUDY MAP")
        assert code != 0 and checkpoints() == dropped
        checked(candidate, operation="prepare", action="SELF_STUDY MAP")

        return {"schema": "source_study_upgrade_qualification_v1", "passed": True,
                "old_reader_sha256": sha(old), "candidate_reader_sha256": sha(candidate),
                "old_pending_input_preserved": True, "ordinary_v1_readable_by_old_helper": True,
                "relation_v2_rejects_old_helper_without_checkpoint_changes": True,
                "compatibility_floor_survives_explicit_drop": True,
                "candidate_recovers_after_rejected_downgrade": True,
                "live_state_opened": False, "provider_called": False}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--old", required=True, type=Path)
    parser.add_argument("--candidate", required=True, type=Path)
    args = parser.parse_args()
    print(json.dumps(qualify(args.old.resolve(strict=True), args.candidate.resolve(strict=True)), indent=2))
