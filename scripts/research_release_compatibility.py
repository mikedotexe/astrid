"""Read-only deployment guard for native research transaction state.

This enforces the declared compatibility floor within the sanctioned wrappers.
It is not a way to make an unqualified candidate deployable or to repair state.
"""
import ast
import json
from pathlib import Path


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
    path = Path(source) / "minime_autonomy" / module
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 262144:
        raise RuntimeError(f"Minime {constant} downgrade refused; state preserved")
    # Source inventory and candidate tests bind behavior separately. Inspect a
    # literal declaration without executing unactivated candidate imports.
    declarations = [node.value.value for node in ast.parse(path.read_bytes()).body
        if isinstance(node, ast.Assign) and isinstance(node.value, ast.Constant)
        and any(isinstance(target, ast.Name) and target.id == constant
                for target in node.targets)]
    if len(declarations) != 1 or type(declarations[0]) is not int or declarations[0] < required:
        raise RuntimeError(f"Minime {constant} downgrade refused; state preserved")
