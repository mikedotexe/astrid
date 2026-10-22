"""Isolated maintenance evidence helpers; no service control or model imports.

Coverage and stopped-barrier records must come from the separately reviewed host
adapter. These helpers validate and bind retained evidence; they cannot establish
live ownership from a caller's Boolean. No execution CLI is provided.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import stat

COMMON = ("plan_sha256", "provider_old_identity_sha256", "reservoir_old_identity_sha256")
SHA = re.compile(r"[0-9a-f]{64}\Z")
MAX_FILE_BYTES = 256 * 1024 * 1024


def canonical_hash(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"),
                                     ensure_ascii=True, allow_nan=False).encode()).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def envelope(context, kind, evidence, *, extra_bindings=None, passed=True):
    require(isinstance(context.get("transaction_id"), str) and context["transaction_id"],
            "missing transaction identity")
    bindings = dict(context.get("bindings", {}))
    for key in COMMON:
        require(isinstance(bindings.get(key), str) and SHA.fullmatch(bindings[key]),
                "missing binding: " + key)
    bindings.update(extra_bindings or {})
    return {"schema": "reservoir_maintenance_witness_v1", "kind": kind,
            "transaction_id": context["transaction_id"], "passed": passed,
            "bindings": bindings, "evidence": evidence}


def _identity(info):
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns,
            info.st_mode, info.st_uid, info.st_nlink)


def _read(path, limit=MAX_FILE_BYTES):
    path = Path(path)
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        before = os.fstat(descriptor)
        require(stat.S_ISREG(before.st_mode) and before.st_uid == os.getuid(),
                "artifact is not an owned regular file: " + str(path))
        require(0 <= before.st_size <= limit, "artifact exceeds size bound: " + str(path))
        chunks, total = [], 0
        while True:
            chunk = os.read(descriptor, min(1024 * 1024, limit + 1 - total))
            if not chunk:
                break
            chunks.append(chunk)
            total += len(chunk)
            require(total <= limit, "artifact grew beyond bound")
        require(_identity(before) == _identity(os.fstat(descriptor)), "artifact changed while read")
        require(_identity(before) == _identity(path.lstat()), "artifact path changed while read")
        data = b"".join(chunks)
        require(len(data) == before.st_size, "short artifact read")
        return data, before
    finally:
        os.close(descriptor)


def artifact(path):
    data, _ = _read(path)
    return data, {"path": str(Path(path).absolute()), "sha256": hashlib.sha256(data).hexdigest(),
                  "bytes": len(data)}


def _coverage(context, coverage, role, references):
    require(coverage.get("schema") == "reservoir_maintenance_log_coverage_v1",
            "unsupported log coverage schema")
    require(coverage.get("complete") is True and coverage.get("old_process_absent") is True,
            "complete window through exact old exit is required")
    require(coverage.get("process_identity_sha256") ==
            context["bindings"][role + "_old_identity_sha256"], "wrong process coverage")
    for key, reference in references.items():
        span = coverage.get(key, {})
        require(all(type(span.get(field)) is int for field in
                    ("device", "inode", "start_offset", "end_offset")), "missing log span identity")
        require(span["device"] >= 0 and span["inode"] > 0 and span["start_offset"] >= 0,
                "invalid log span identity")
        require(span["end_offset"] - span["start_offset"] == reference["bytes"] and
                span.get("sha256") == reference["sha256"], "log coverage bytes do not match")


def _qos_selected(records):
    pending, active, selected, completed = set(), None, [], []
    known, outcomes = set(), {}
    recent_completed, late_waiters = None, []
    allowed = {"queued", "selected", "completed", "coalesced", "disconnected_active",
               "disconnected_queued", "stopped_pending", "queue_timeout"}
    for record in records:
        require(record.get("stream") == "model_qos" and record.get("schema_version") == 1,
                "unsupported QoS record")
        identity = record.get("receipt_id")
        canonical = {key: value for key, value in record.items() if key != "receipt_id"}
        require(identity == "model-qos-" + canonical_hash(canonical), "invalid QoS receipt hash")
        seq, event = record.get("arrival_sequence"), record.get("lifecycle")
        require(type(seq) is int and seq > 0 and event in allowed, "invalid QoS lifecycle")
        if event == "queued":
            require(seq not in known, "duplicate arrival sequence")
            known.add(seq)
            pending.add(seq)
        elif event == "selected":
            recent_completed = None
            require(active is None and seq in pending, "missing queue/overlapping generation")
            pending.remove(seq)
            active = seq
            selected.append(seq)
        elif event == "completed":
            require(active == seq, "completion does not match active generation")
            require(record.get("outcome") in ("delivered", "response_discarded_after_active_disconnect"),
                    "generation did not complete successfully")
            completed.append(seq)
            outcomes[str(seq)] = record["outcome"]
            active = None
            recent_completed = seq
        elif event in ("disconnected_queued", "stopped_pending", "queue_timeout"):
            require(seq in pending, "pending terminal event without queued work")
            pending.remove(seq)
            outcomes[str(seq)] = event
            if event in ("stopped_pending", "queue_timeout"):
                recent_completed = None  # worker-only events happen after active cleanup
        else:
            if seq == recent_completed:
                # Completion is logged before _active/_inflight cleanup. A gateway
                # thread can hold the condition lock while awaiting the receipt
                # writer lock, then append this bookkeeping after completed.
                late_waiters.append(dict(record))
            else:
                require(seq == active or (event == "coalesced" and seq in pending),
                        "waiter event without active/pending/recently-completed work")
    require(active is None and not pending, "unresolved active or pending work at exit")
    require(selected == completed, "selected/completed mismatch")
    return selected, outcomes, late_waiters


def verify_provider_drain(context, log_path, qos_path, coverage, journal_path):
    log_data, log_ref = artifact(log_path)
    qos_data, qos_ref = artifact(qos_path)
    journal_data, journal_ref = artifact(journal_path)
    _coverage(context, coverage, "provider", {"log": log_ref, "qos": qos_ref})
    require(coverage.get("start_worker_idle") is True and
            coverage.get("start_queue_empty") is True, "capture needs an idle/empty beginning anchor")
    require(log_data.endswith(b"\n") and (not qos_data or qos_data.endswith(b"\n")),
            "truncated log record")
    records = [json.loads(line) for line in qos_data.decode().splitlines()]
    selected, outcomes, late_waiters = _qos_selected(records)
    journal = json.loads(journal_data)
    require(isinstance(journal, dict) and isinstance(journal.get("entries"), list), "invalid journal")
    generated = re.compile(r"generated (\d+) tokens on (\S+) in ")
    checked = re.compile(r"push_state: checked in (\S+) \(\+(\d+) ticks, h_norms=\[")
    pending, pairs, signals, shutdowns = None, [], [], []
    for number, line in enumerate(log_data.decode().splitlines(), 1):
        if "received signal " in line:
            require("received signal 15; draining active generation" in line, "unexpected drain signal")
            signals.append(number)
        if "shutting down — saving coupling journal" in line:
            require(pending is None, "shutdown before generation check-in")
            shutdowns.append(number)
        require(not any(word in line for word in ("generation failed", "coupled model worker failed",
                    "coupling journal save failed", "HTTP gateway shutdown failed")), "provider failure in window")
        generation, acknowledgement = generated.search(line), checked.search(line)
        if generation:
            require(pending is None and not shutdowns, "overlapping generation or work after shutdown")
            pending = {"tokens": int(generation[1]), "handle": generation[2], "generated_line": number}
        if "push_state" in line and not acknowledgement:
            require("push_state: slow RPC (" in line, "missing/failed reservoir check-in")
        if acknowledgement:
            require(pending is not None and pending["handle"] == acknowledgement[1] and
                    pending["tokens"] == int(acknowledgement[2]), "unmatched reservoir acknowledgement")
            pairs.append(dict(pending, acknowledged_line=number))
            pending = None
    require(pending is None and len(signals) == len(shutdowns) == 1 and signals[0] < shutdowns[0],
            "incomplete or ambiguous drain boundary")
    require(len(pairs) == len(selected), "generation/QoS count mismatch")
    for pair, seq in zip(pairs, selected):
        pair["arrival_sequence"] = seq
    return envelope(context, "provider_drain", {
        "log": log_ref, "qos": qos_ref, "journal": journal_ref, "coverage": coverage,
        "matched_generations": pairs, "request_outcomes": outcomes, "late_waiter_bookkeeping": late_waiters,
        "method": "complete single-worker ordered generation/ack pairs plus QoS lifecycle; journal is continuity only",
        "limitation": "Capture adapter must establish complete PID-bound spans; logs have no per-line PID or push request ID."})


def verify_final_snapshot(context, log_path, initial_handles, coverage):
    data, reference = artifact(log_path)
    _coverage(context, coverage, "reservoir", {"log": reference})
    require(coverage.get("inventory_matches_start_boundary") is True, "inventory/window boundary not established")
    require(initial_handles.get("type") == "list_handles_response", "missing read-only handle inventory")
    names = [item["name"] for item in initial_handles["handles"]]
    require(all(isinstance(name, str) and name for name in names) and len(names) == len(set(names)),
            "invalid/duplicate inventory names")
    require(data.endswith(b"\n"), "truncated reservoir log")
    active, final, changes = set(names), [], []
    phase, complete, goodbye = "before", False, False
    for number, line in enumerate(data.decode().splitlines(), 1):
        create = re.search(r"created handle '([^']+)'", line)
        restore = re.search(r"restored '([^']+)' \(entity=", line)
        destroy = re.search(r"destroyed handle '([^']+)'", line)
        if create or restore or destroy:
            require(phase == "before", "handle lifecycle changed during/after final snapshot")
            name = (create or restore or destroy)[1]
            if destroy:
                require(name in active, "destroyed name absent from reconciled inventory")
                active.remove(name)
                action = "destroy"
            else:
                require(not create or name not in active, "duplicate create in lifecycle")
                active.add(name)
                action = "create" if create else "restore"
            changes.append({"line": number, "action": action, "name": name})
        if "[reservoir] shutting down..." in line:
            require(phase == "before", "duplicate shutdown boundary")
            phase = "final"
        if phase == "final":
            require("snapshot_all failed" not in line, "final per-handle snapshot failed")
            match = re.search(r"snapshot '([^']+)' → (.+)$", line)
            if match:
                require(match[1] not in final, "duplicate final snapshot")
                final.append(match[1])
            if "[reservoir] final snapshot complete" in line:
                require(bool(active), "unexpected final marker for empty inventory")
                complete, phase = True, "after"
        if "[reservoir] goodbye" in line:
            require(phase in ("final", "after") and not goodbye, "invalid goodbye boundary")
            goodbye = True
            phase = "after"
    require(goodbye and (complete or not active), "missing shutdown/final completion evidence")
    require(set(final) == active, "final snapshot set differs from reconciled inventory")
    return envelope(context, "final_snapshot", {
        "log": reference, "coverage": coverage, "initial_inventory_sha256": canonical_hash(initial_handles),
        "final_handles": final, "lifecycle_changes": changes, "full_log_coverage": True})


def _walk_error(error):
    raise error


def inventory_tree(root, *, max_file_bytes=MAX_FILE_BYTES):
    root = Path(root).absolute()
    require(root == root.resolve(strict=True), "state root contains a symlink")
    require(root.is_dir(), "state root is not a directory")
    entries, total = {}, 0
    for directory, dirs, files in os.walk(root, followlinks=False, onerror=_walk_error):
        directory = Path(directory)
        for path in [directory] + [directory / name for name in sorted(files + dirs)]:
            relative = str(path.relative_to(root))
            if relative in entries:
                continue
            info = path.lstat()
            require(info.st_uid == os.getuid() and not stat.S_ISLNK(info.st_mode),
                    "unowned path or symlink in state inventory")
            item = {"kind": "directory" if stat.S_ISDIR(info.st_mode) else "file",
                    "device": info.st_dev, "inode": info.st_ino, "mode": stat.S_IMODE(info.st_mode),
                    "mtime_ns": info.st_mtime_ns, "uid": info.st_uid}
            if not stat.S_ISDIR(info.st_mode):
                require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1, "special or hard-linked state file")
                data, stable = _read(path, max_file_bytes)
                require(_identity(info) == _identity(stable), "state file changed during inventory")
                total += len(data)
                require(total <= 2 * 1024 * 1024 * 1024, "state inventory exceeds total byte bound")
                item.update(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
            entries[relative] = item
    return {"schema": "reservoir_stopped_inventory_v1", "root": str(root),
            "entries": dict(sorted(entries.items())), "total_file_bytes": total}


def _sync_directory(path):
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _write_exclusive(path, data):
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        view = memoryview(data)
        while view:
            written = os.write(descriptor, view)
            require(written > 0, "short backup write")
            view = view[written:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def backup_stopped_tree(context, root, destination, stopped_barrier):
    require(stopped_barrier.get("schema") == "reservoir_maintenance_stopped_barrier_v1" and
            stopped_barrier.get("transaction_id") == context["transaction_id"], "wrong stopped barrier")
    require(stopped_barrier.get("bindings") == context["bindings"], "stopped barrier bindings differ")
    require(stopped_barrier.get("ownership_current") is True, "transaction ownership not established")
    for role in ("provider", "reservoir"):
        require(all(stopped_barrier.get(role, {}).get(key) is True for key in
                    ("job_absent", "old_process_absent", "listener_absent")), "writers are not proven absent")
    require(SHA.fullmatch(context["bindings"].get("final_snapshot_sha256", "")), "missing final snapshot binding")
    before = inventory_tree(root)
    root, destination = Path(root).resolve(), Path(destination).absolute()
    require(destination.parent == destination.parent.resolve(strict=True), "backup parent is not canonical")
    require(root != destination and root not in destination.parents and destination not in root.parents,
            "backup overlaps source state")
    destination.mkdir(mode=0o700, exist_ok=False)
    data_root = destination / "state"
    data_root.mkdir(mode=0o700)
    intent = {"status": "prepared_not_complete", "transaction_id": context["transaction_id"],
              "inventory": before, "barrier": stopped_barrier}
    _write_exclusive(destination / "backup-intent.json", json.dumps(intent, indent=2).encode() + b"\n")
    _sync_directory(destination)
    for name, item in before["entries"].items():
        if name == ".":
            continue
        target = data_root / name
        if item["kind"] == "directory":
            target.mkdir(mode=0o700)
        else:
            data, _ = _read(root / name)
            require(hashlib.sha256(data).hexdigest() == item["sha256"], "source changed before copy")
            _write_exclusive(target, data)
            os.utime(target, ns=(item["mtime_ns"], item["mtime_ns"]), follow_symlinks=False)
            descriptor = os.open(target, os.O_RDONLY | os.O_NOFOLLOW)
            try:
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
    for directory, _, _ in os.walk(data_root, topdown=False, onerror=_walk_error):
        _sync_directory(directory)
    require(inventory_tree(root) == before, "source state changed during backup; partial backup retained")
    copied = inventory_tree(data_root)
    require(set(copied["entries"]) == set(before["entries"]), "backup path set differs")
    for name, item in before["entries"].items():
        require(item["kind"] == copied["entries"][name]["kind"], "backup path kind differs")
        if item["kind"] == "file":
            require(all(item[key] == copied["entries"][name][key] for key in ("bytes", "sha256", "mtime_ns")),
                    "backup bytes or preserved timestamp differs")
    result = envelope(context, "backup", {"source_inventory": before, "inventory_sha256": canonical_hash(before),
                "backup_inventory": copied, "backup_root": str(data_root), "all_files": True,
                "stopped_barrier_sha256": canonical_hash(stopped_barrier)})
    _write_exclusive(destination / "backup-receipt.json", json.dumps(result, indent=2).encode() + b"\n")
    _sync_directory(destination)
    _sync_directory(destination.parent)
    return result


def prepare_restore_request(context, backup_witness, *, source_sha256, interpreter_sha256, configuration_sha256):
    require(backup_witness.get("kind") == "backup" and backup_witness.get("passed") is True and
            backup_witness.get("transaction_id") == context["transaction_id"], "missing owned backup witness")
    for key in COMMON:
        require(backup_witness["bindings"].get(key) == context["bindings"].get(key), "backup identity mismatch")
    values = {"backup_sha256": canonical_hash(backup_witness), "repair_source_sha256": source_sha256,
              "interpreter_sha256": interpreter_sha256, "configuration_sha256": configuration_sha256}
    require(all(isinstance(value, str) and SHA.fullmatch(value) for value in values.values()), "invalid restore binding")
    return envelope(context, "private_restore", {
        "status": "not_executed", "backup_root": backup_witness["evidence"]["backup_root"],
        "inventory_sha256": backup_witness["evidence"]["inventory_sha256"],
        "required": "Actual isolated restore and state/sidecar semantic comparisons remain required; this is never an acceptance receipt."},
        extra_bindings=values, passed=False)
