"""Read-only verification of runtime-authored graceful NEXT receipts.

No command payload, replay, state mutation or service control. This verifies a
particular process restored a queue, not that the choice executed successfully.
"""
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import re

CONTRACT = "minime_next_handoff_v1"
MAX_BYTES = 8 * 1024 * 1024


def require(value, reason):
    if not value:
        raise RuntimeError(reason)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False,
                      allow_nan=False).encode("utf-8")


def sha(value):
    return hashlib.sha256(canonical(value)).hexdigest()


def unique(items):
    result = {}
    for key, value in items:
        require(key not in result, "duplicate NEXT handoff key")
        result[key] = value
    return result


def history_path(workspace):
    path = Path(workspace) / "runtime/next-handoff-v1.jsonl"
    path = path.absolute()
    if sys.platform == "darwin":
        for alias in (Path("/var"), Path("/tmp")):
            if path.is_relative_to(alias) and alias.is_symlink():
                target = Path("/private") / alias.name
                require(alias.lstat().st_uid == 0 and alias.resolve() == target,
                        "unexpected system path alias")
                path = target / path.relative_to(alias)
                break
    require(not any(p.is_symlink() for p in (path, *path.parents)), "linked NEXT handoff history")
    return path


def read_owned(path, maximum):
    require(not path.is_symlink(), "linked NEXT handoff evidence")
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW), "rb") as stream:
        info = os.fstat(stream.fileno())
        require(stat.S_ISREG(info.st_mode) and info.st_uid == os.getuid()
                and info.st_nlink == 1 and info.st_size <= maximum, "invalid NEXT handoff evidence")
        raw = stream.read(maximum + 1)
    require(len(raw) <= maximum, "incomplete NEXT handoff evidence")
    return raw


def admission_path(workspace):
    return history_path(workspace).with_name("next-admission-unsealed-v1.json")


def required_admission_version(workspace):
    path = admission_path(workspace)
    if not os.path.lexists(path):
        return 0
    value = json.loads(read_owned(path, 4 * 1024 * 1024), object_pairs_hook=unique,
                       parse_constant=lambda _: require(False, "nonfinite NEXT handoff field"))
    require(isinstance(value, dict) and set(value) == {
                "contract", "kind", "version", "process", "state_sha256", "pending_sha256"}
            and value.get("contract") == CONTRACT and value.get("kind") == "unsealed_admission"
            and type(value.get("version")) is int and value["version"] > 0,
            "unsupported NEXT admission witness; state preserved")
    identity = value["process"]
    require(isinstance(identity, dict) and set(identity) == {"boot_id", "pid", "started_at", "source_sha256"}
            and type(identity["pid"]) is int and identity["pid"] > 0
            and isinstance(identity["boot_id"], str) and re.fullmatch(r"[0-9a-f]{32}", identity["boot_id"])
            and isinstance(identity["started_at"], str) and 0 < len(identity["started_at"]) < 128,
            "invalid NEXT admission identity; state preserved")
    for digest in (value["state_sha256"], value["pending_sha256"], identity["source_sha256"]):
        require(isinstance(digest, str) and re.fullmatch(r"[0-9a-f]{64}", digest),
                "invalid NEXT admission hash; state preserved")
    return value["version"]


def read_history_bytes(workspace):
    raw = read_owned(history_path(workspace), MAX_BYTES)
    require(0 < len(raw) <= MAX_BYTES and raw.endswith(b"\n"), "incomplete NEXT handoff history")
    return raw


def read_history(workspace):
    raw = read_history_bytes(workspace)
    rows, head = {}, None
    for line in raw.splitlines():
        require(len(rows) < 8192, "NEXT handoff history limit")
        row = json.loads(line, object_pairs_hook=unique,
                         parse_constant=lambda _: require(False, "nonfinite NEXT handoff field"))
        require(isinstance(row, dict) and set(row) == {"body", "sha256"}
                and isinstance(row["body"], dict), "invalid NEXT handoff record")
        body = row["body"]
        require(body.get("contract") == CONTRACT and type(body.get("version")) is int
                and body["version"] == 1 and body.get("previous") == head
                and type(body.get("sequence")) is int and body["sequence"] == len(rows) + 1
                and body.get("kind") in {"sealed", "restored"} and sha(body) == row["sha256"],
                "NEXT handoff integrity failure")
        fields = {"contract", "version", "sequence", "previous", "kind", "process",
                  "state_sha256", "pending_sha256", "pending_present"}
        if body["kind"] == "restored":
            fields |= {"seal_sha256", "scheduling_authorized"}
        identity = body.get("process")
        require(set(body) == fields and isinstance(identity, dict)
                and set(identity) == {"boot_id", "pid", "started_at", "source_sha256"},
                "NEXT handoff record fields invalid")
        require(type(identity["pid"]) is int and identity["pid"] > 0
                and isinstance(identity["boot_id"], str) and re.fullmatch(r"[0-9a-f]{32}", identity["boot_id"])
                and isinstance(identity["started_at"], str) and 0 < len(identity["started_at"]) < 128
                and type(body["pending_present"]) is bool, "NEXT handoff record identity invalid")
        for value in (body["state_sha256"], body["pending_sha256"], identity["source_sha256"]):
            require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value),
                    "NEXT handoff record hash invalid")
        if body["kind"] == "restored":
            sealed = rows.get(body["seal_sha256"], {})
            require(sealed.get("kind") == "sealed" and body["scheduling_authorized"] is False
                    and sealed["process"]["boot_id"] != identity["boot_id"]
                    and all(body[key] == sealed[key] for key in
                            ("state_sha256", "pending_sha256", "pending_present")),
                    "NEXT acknowledgement does not match earlier seal")
        rows[row["sha256"]] = body
        head = row["sha256"]
    return rows


def process(status):
    require(status.get("next_handoff_contract") == CONTRACT, "NEXT handoff capability absent")
    value = dict(boot_id=status.get("boot_id"), pid=status.get("pid"),
                 started_at=status.get("started_at"),
                 source_sha256=sha(status.get("source_inputs_at_start")))
    require(type(value["pid"]) is int and value["pid"] > 0
            and isinstance(value["boot_id"], str) and re.fullmatch(r"[0-9a-f]{32}", value["boot_id"])
            and isinstance(value["started_at"], str) and bool(value["started_at"])
            and isinstance(status.get("source_inputs_at_start"), dict), "NEXT handoff identity absent")
    return value


def legacy_paths(workspace):
    root = history_path(workspace).parent
    return root / "next-legacy-import-v1.json", root / "next-legacy-restored-v1.json"


def read_legacy(path):
    value = json.loads(read_owned(path, 65536), object_pairs_hook=unique,
                      parse_constant=lambda _: require(False, "nonfinite legacy NEXT field"))
    require(isinstance(value, dict), "invalid legacy NEXT object")
    return value


def required_legacy_version(workspace):
    imported, acknowledgement = legacy_paths(workspace)
    if not os.path.lexists(imported):
        require(not os.path.lexists(acknowledgement), "orphan legacy NEXT acknowledgement")
        return 0
    value = read_legacy(imported)
    require(value.get("schema") == "minime_next_legacy_import_v1"
            and type(value.get("version")) is int and value["version"] > 0,
            "unsupported legacy NEXT import; state preserved")
    return value["version"]


def verify_legacy_restore(workspace, expected_import, status, expected_inputs):
    imported_path, ack_path = legacy_paths(workspace)
    imported = read_legacy(imported_path)
    require(imported == expected_import and imported.get("schema") == "minime_next_legacy_import_v1"
            and imported.get("version") == 1 and imported.get("runtime_seal_available") is False
            and imported.get("execution_confirmed") is False,
            "legacy NEXT import binding mismatch")
    current = process(status)
    require(status.get("source_inputs_at_start") == expected_inputs
            and current["source_sha256"] == imported["expected_source_sha256"]
            and current["pid"] != imported["old_process"]["pid"],
            "legacy NEXT replacement identity mismatch")
    expected = {"schema": "minime_next_legacy_restored_v1", "version": 1, "kind": "legacy_restored",
                "import_sha256": hashlib.sha256(read_owned(imported_path, 65536)).hexdigest(),
                "process": current, **{key: imported[key] for key in
                    ("state_sha256", "pending_sha256", "pending_present")},
                "runtime_seal_available": False, "scheduling_authorized": False,
                "execution_confirmed": False}
    require(read_legacy(ack_path) == expected == status.get("next_handoff_restore"),
            "legacy NEXT restoration acknowledgement absent or changed")
    return {"contract": "minime_next_legacy_import_v1", "import_sha256": expected["import_sha256"],
            "restore_sha256": hashlib.sha256(read_owned(ack_path, 65536)).hexdigest(),
            "old_process": imported["old_process"], "new_process": current,
            "pending_present": imported["pending_present"], "runtime_seal_available": False,
            "execution_confirmed": False}


def verify_seal(workspace, old_process, status):
    require(not os.path.lexists(admission_path(workspace)), "NEXT admission remains unsealed")
    require(process(status) == old_process, "NEXT stopped process identity mismatch")
    receipt = status.get("next_handoff_seal")
    require(isinstance(receipt, dict) and receipt.get("kind") == "sealed"
            and receipt.get("process") == old_process, "NEXT drain seal absent")
    rows = read_history(workspace)
    digest = receipt.get("record_sha256")
    require(isinstance(digest, str) and digest in rows
            and rows[digest] == {key: value for key, value in receipt.items() if key != "record_sha256"},
            "NEXT drain seal not durable")
    path = history_path(workspace).parent.parent / "sovereignty_state.json"
    require(not path.is_symlink(), "linked NEXT checkpoint")
    raw = b"{}"
    if path.exists():
        with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW), "rb") as stream:
            info = os.fstat(stream.fileno())
            require(stat.S_ISREG(info.st_mode) and info.st_uid == os.getuid()
                    and info.st_nlink == 1 and info.st_size <= 4 * 1024 * 1024,
                    "invalid NEXT checkpoint")
            raw = stream.read(4 * 1024 * 1024 + 1)
    require(hashlib.sha256(raw).hexdigest() == receipt.get("state_sha256"),
            "stopped NEXT checkpoint changed")
    return {"contract": CONTRACT, "seal_sha256": digest, "old_process": old_process,
            "pending_present": receipt["pending_present"], "execution_confirmed": False}


def verify_restore(workspace, old_process, status, expected_inputs):
    current = process(status)
    require(current["boot_id"] != old_process.get("boot_id")
            and status.get("source_inputs_at_start") == expected_inputs,
            "NEXT handoff replacement identity mismatch")
    receipt = status.get("next_handoff_restore")
    require(isinstance(receipt, dict) and receipt.get("kind") == "restored"
            and receipt.get("process") == current and receipt.get("scheduling_authorized") is False,
            "NEXT restoration acknowledgement absent")
    rows = read_history(workspace)
    digest = receipt.get("record_sha256")
    require(isinstance(digest, str) and digest in rows
            and rows[digest] == {key: value for key, value in receipt.items() if key != "record_sha256"},
            "NEXT restoration receipt not durable")
    sealed = rows.get(receipt.get("seal_sha256"), {})
    require(sealed.get("kind") == "sealed" and sealed.get("process") == old_process
            and all(sealed.get(key) == receipt.get(key) for key in
                    ("state_sha256", "pending_sha256", "pending_present")),
            "NEXT restoration does not match stopped process")
    return {"contract": CONTRACT, "seal_sha256": receipt["seal_sha256"],
            "restore_sha256": digest, "old_process": old_process, "new_process": current,
            "pending_present": receipt["pending_present"], "execution_confirmed": False}
