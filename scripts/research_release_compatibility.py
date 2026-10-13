"""Read-only deployment guard for native research transaction state.

This enforces the declared compatibility floor within the sanctioned wrappers.
It is not a way to make an unqualified candidate deployable or to repair state.
"""
import ast
import hashlib
import json
import os
from pathlib import Path
import re
import stat


def _unique_fields(items):
    result = {}
    for key, value in items:
        if key in result:
            raise RuntimeError("duplicate research compatibility field; state preserved")
        result[key] = value
    return result


def _required_version(workspace, owner, filename, version_field):
    root = Path(workspace) / "action_threads"
    if root.is_symlink():
        raise RuntimeError("invalid research compatibility directory; state preserved")
    path = root / filename
    if not path.exists() and not path.is_symlink():
        return 0
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 4096:
        raise RuntimeError("invalid research compatibility requirement; state preserved")
    with path.open("rb") as handle:
        raw = handle.read(4097)
    if len(raw) > 4096:
        raise RuntimeError("invalid research compatibility requirement; state preserved")
    value = json.loads(raw, object_pairs_hook=_unique_fields)
    if (not isinstance(value, dict)
            or set(value) != {"schema_version", "owner", version_field}
            or type(value["schema_version"]) is not int
            or value["schema_version"] != 1 or value["owner"] != owner
            or type(value[version_field]) is not int
            or value[version_field] < 1):
        raise RuntimeError("unsupported research compatibility requirement; state preserved")
    return value[version_field]


def required_version(workspace, owner):
    return _required_version(workspace, owner, "research-transactions-required.json",
                             "required_research_transaction_version")


def required_provider_input_receipt_version(workspace, owner):
    return _required_version(workspace, owner, "provider-input-receipts-required.json",
                             "required_provider_input_receipt_version")


def verify_bridge(workspace, native_report):
    for name, loader in (("research_transactions", required_version),
                         ("provider_input_receipts", required_provider_input_receipt_version)):
        required = loader(workspace, "astrid")
        if not required:
            continue
        capability = native_report.get(name, {}) if isinstance(native_report, dict) else {}
        supported = capability.get("supported_version") if isinstance(capability, dict) else None
        if (type(supported) is not int or supported < required
                or capability.get("owner") != "astrid"
                or type(capability.get("required_version")) is not int
                or capability.get("required_version") != required):
            raise RuntimeError(f"bridge {name} downgrade refused; state preserved")


def verify_minime(source, workspace):
    from minime_next_handoff import (history_path, read_history, read_history_bytes,
                                    required_admission_version, required_legacy_version)
    required = required_mailbox_version(workspace)
    if required:
        _verify_minime_capability(source, "mailbox.py", "MAILBOX_COMPATIBILITY_VERSION", required)
    required = required_legacy_version(workspace)
    if required:
        _verify_minime_capability(source, "maintenance_next_legacy.py", "LEGACY_HANDOFF_VERSION", required)
        _verify_minime_capability(source, "maintenance_next.py", "HANDOFF_VERSION", 1)
    required = required_admission_version(workspace)
    if required:
        _verify_minime_capability(source, "maintenance_next.py", "HANDOFF_VERSION", required)
    history = history_path(workspace)
    if history.exists() or history.is_symlink():
        raw = read_history_bytes(workspace)
        required = 1
        for line in raw.splitlines():
            row = json.loads(line, object_pairs_hook=_unique_fields)
            body = row.get("body") if isinstance(row, dict) else None
            version = body.get("version") if isinstance(body, dict) else None
            if type(version) is not int or version < 1:
                raise RuntimeError("unsupported NEXT handoff history; state preserved")
            required = max(required, version)
        _verify_minime_capability(source, "maintenance_next.py", "HANDOFF_VERSION", required)
        read_history(workspace)
    for module, constant, loader in (
            ("research_transactions.py", "RESEARCH_TRANSACTION_VERSION", required_version),
            ("research_provider.py", "PROVIDER_INPUT_RECEIPT_VERSION", required_provider_input_receipt_version)):
        required = loader(workspace, "minime")
        if required:
            _verify_minime_capability(source, module, constant, required)


def _verify_minime_capability(source, module, constant, required):
    from minime_next_handoff import history_path
    try:
        path = history_path(source).parent.parent / "minime_autonomy" / module
        tree = ast.parse(_mailbox_read(path, 262144))
    except (OSError, ValueError, SyntaxError, RuntimeError):
        raise RuntimeError(f"Minime {constant} downgrade refused; state preserved") from None
    # Source inventory and candidate tests bind behavior separately. Inspect a
    # literal declaration without executing unactivated candidate imports.
    declarations = [node.value.value for node in tree.body
        if isinstance(node, ast.Assign) and isinstance(node.value, ast.Constant)
        and any(isinstance(target, ast.Name) and target.id == constant
                for target in node.targets)]
    writes = [node for node in ast.walk(tree) if isinstance(node, ast.Name)
              and node.id == constant and isinstance(node.ctx, (ast.Store, ast.Del))]
    if (len(writes) != 1 or len(declarations) != 1
            or type(declarations[0]) is not int or declarations[0] < required):
        raise RuntimeError(f"Minime {constant} downgrade refused; state preserved")


def _mailbox_require(condition):
    if not condition:
        raise RuntimeError("invalid or incomplete mailbox compatibility evidence; state preserved")


def _mailbox_hash(value):
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _mailbox_path(path):
    _mailbox_require(not any(p.is_symlink() for p in (path, *path.parents)))
    return path


def _mailbox_entries(path):
    _mailbox_path(path)
    if not path.exists():
        return []
    _mailbox_require(path.is_dir())
    result = []
    with os.scandir(path) as entries:
        for entry in entries:
            _mailbox_require(len(result) < 262144 and not entry.is_symlink())
            result.append(path / entry.name)
    return result


def _mailbox_read(path, maximum):
    _mailbox_path(path)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    with os.fdopen(fd, "rb") as stream:
        before = os.fstat(stream.fileno())
        _mailbox_require(stat.S_ISREG(before.st_mode) and before.st_size <= maximum)
        raw = stream.read(maximum + 1)
        after = os.fstat(stream.fileno())
        current = _mailbox_path(path).stat()
        snapshot = lambda info: (info.st_dev, info.st_ino, info.st_size,
                                 info.st_mtime_ns, info.st_ctime_ns)
        _mailbox_require(len(raw) <= maximum and snapshot(before) == snapshot(after) == snapshot(current))
    return raw


def _mailbox_json(path, maximum, *, object_required=True):
    raw = _mailbox_read(path, maximum)
    value = json.loads(raw, object_pairs_hook=_unique_fields,
                       parse_constant=lambda _: _mailbox_require(False))
    _mailbox_require(not object_required or isinstance(value, dict))
    return value, raw


def _mailbox_manifest(directory):
    entries = _mailbox_entries(directory)
    if not entries:
        return False
    # Do not use Mailbox.status(): it creates directories/locks. Candidate source
    # is never imported here; this is the sanctioned wrapper's v1 reader.
    state, _ = _mailbox_json(directory / "state.json", 8_000_000)
    _mailbox_require(_mailbox_read(directory / "initialized", 128) == b"minime_mailbox_v1")
    _mailbox_require(set(state) in ({"schema", "seen", "completed", "pending"},
                                  {"schema", "seen", "completed", "pending", "last_selection"})
                     and state.get("schema") == "minime_mailbox_v1"
                     and isinstance(state.get("seen"), list) and len(state["seen"]) <= 65536
                     and all(_mailbox_hash(x) for x in state["seen"])
                     and isinstance(state.get("completed"), dict) and len(state["completed"]) <= 65536
                     and all(0 < len(k) <= 256 and _mailbox_hash(v) for k, v in state["completed"].items()))
    selection = state.get("last_selection")
    if selection is not None:
        _mailbox_require(isinstance(selection, dict)
                         and selection.get("outcome") in {"selected", "no_eligible"}
                         and selection.get("character_limit") in {4000, 12000}
                         and type(selection.get("inspected")) is int and 0 <= selection["inspected"] <= 20000
                         and isinstance(selection.get("rejection_counts"), dict)
                         and set(selection["rejection_counts"]) == {
                             "unreadable", "invalid_utf8", "empty", "rtf", "oversize"}
                         and all(type(n) is int and 0 <= n <= 20000
                                 for n in selection["rejection_counts"].values()))
    for path in entries:
        if path.name in {"state.json", "initialized"}:
            continue
        if re.fullmatch(r"retired-[0-9a-f]{64}", path.name):
            children = _mailbox_entries(path)
            _mailbox_require(all(p.name == "source" and p.is_file() for p in children))
        else:
            _mailbox_require(re.fullmatch(r"[0-9a-f]{64}\.(?:txt|provider\.json|response\.txt)", path.name)
                             and path.is_file() and path.stat().st_size <= 8_000_000)
    pending = state["pending"]
    if pending is None:
        return True
    _mailbox_require(isinstance(pending, dict)
                     and pending.get("phase") in {"selected", "started", "supplied", "response_retained"}
                     and all(isinstance(pending.get(k), str) for k in
                             ("operation", "filename", "source_sha256", "artifact_id"))
                     and 0 < len(pending["operation"]) <= 256
                     and pending["filename"] not in {"", ".", ".."}
                     and Path(pending["filename"]).name == pending["filename"]
                     and "\0" not in pending["filename"]
                     and all(_mailbox_hash(pending[k]) for k in ("source_sha256", "artifact_id"))
                     and pending.get("retirement") in {None, "armed"}
                     and pending.get("retirement_debt") in {None, "changed_source_retained"}
                     and type(pending.get("completion_validated", False)) is bool)
    raw = _mailbox_read(directory / (pending["source_sha256"] + ".txt"), 64000)
    _mailbox_require(hashlib.sha256(raw).hexdigest() == pending["source_sha256"])
    raw.decode("utf-8")
    if pending["phase"] in {"supplied", "response_retained"}:
        _mailbox_require(isinstance(pending.get("supplied_attempt"), str) and bool(pending["supplied_attempt"]))
    if pending.get("completion_validated") or "provider_receipt_sha256" in pending:
        provider, raw = _mailbox_json(directory / (pending["artifact_id"] + ".provider.json"),
                                      8_000_000, object_required=False)
        _mailbox_require(hashlib.sha256(raw).hexdigest() == pending.get("provider_receipt_sha256"))
        if pending.get("completion_validated"):
            _mailbox_require(isinstance(provider, dict))
            choices = provider.get("choices")
            stopped = (isinstance(choices, list) and bool(choices) and isinstance(choices[0], dict)
                       and choices[0].get("finish_reason") == "stop") if "choices" in provider else (
                           provider.get("done") is True and provider.get("done_reason") == "stop")
            _mailbox_require(stopped)
    if pending["phase"] == "response_retained":
        _mailbox_require(pending.get("completion_validated") is True)
        raw = _mailbox_read(directory / (pending["artifact_id"] + ".response.txt"), 1_048_576)
        _mailbox_require(hashlib.sha256(raw).hexdigest() == pending.get("response_sha256"))
    if pending.get("retirement"):
        _mailbox_require((directory / ("retired-" + pending["artifact_id"])).is_dir())
    return True


def _mailbox_publications(directory, schema):
    entries = _mailbox_entries(directory)
    for entry in entries:
        if entry.name == ".publication.lock":
            _mailbox_require(entry.is_file())
            continue
        _mailbox_require(_mailbox_hash(entry.name) and entry.is_dir())
        files = _mailbox_entries(entry)
        _mailbox_require({p.name for p in files} in ({"intent.json"}, {"intent.json", "published.json"}))
        metadata, raw = _mailbox_json(entry / "intent.json", 4096)
        fields = {"schema", "sha256", "path"} if schema == "inbox_publication_v1" else {"schema", "sha256"}
        _mailbox_require(set(metadata) == fields and metadata.get("schema") == schema
                         and _mailbox_hash(metadata.get("sha256")))
        if "path" in fields:
            relative = metadata["path"]
            _mailbox_require(isinstance(relative, str) and relative.startswith("outbox/")
                             and "\0" not in relative and all(p not in {"", ".", ".."} for p in relative.split("/"))
                             and hashlib.sha256(relative.encode()).hexdigest() == entry.name)
        if (entry / "published.json").exists():
            _mailbox_require(_mailbox_read(entry / "published.json", 4096) == raw)
    return bool(entries)


def required_mailbox_version(workspace):
    """Retained history keeps the floor after settlement; no repair or replay.

    A valid intent without a published marker is uncertainty, not permission to
    republish. Only a capable candidate may retain and reconcile that state.
    This check is a read-only snapshot; the wrapper still owns quiescence and
    source binding across the actual installation/handoff.
    """
    from minime_next_handoff import history_path
    try:
        # Reuse the wrapper's root-owned macOS alias handling and ancestry check.
        root = history_path(workspace).parent.parent
        directory = _mailbox_path(root / "correspondence")
        if not directory.exists():
            return 0
        _mailbox_require(directory.is_dir())
        retained = _mailbox_manifest(directory / "mailbox_v1")
        for name, schema in (("inbox_publications_v1", "inbox_publication_v1"),
                             ("human_letter_publications_v1", "human_letter_publication_v1")):
            retained = _mailbox_publications(directory / name, schema) or retained
        return int(retained)
    except (OSError, ValueError, TypeError, RecursionError):
        raise RuntimeError("invalid or incomplete mailbox compatibility evidence; state preserved") from None
