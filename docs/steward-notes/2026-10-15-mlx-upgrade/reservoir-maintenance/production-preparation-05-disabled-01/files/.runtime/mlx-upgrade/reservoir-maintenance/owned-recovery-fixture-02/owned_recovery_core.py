"""Bind the complete05 lifecycle source; never alter its admission flags.

Disabled source can be imported by inert source tests. The native coordinator
separately requires the final enabled closure before any OS boundary is used.
"""
from __future__ import annotations
import hashlib
import importlib
import ast
from pathlib import Path
import sys

from owned_fixture_records import raw

CORE_ROOT = Path(__file__).resolve().parent.parent / 'production-candidate-05'
MODULES = ('reservoir_maintenance', 'transaction_execution', 'recovery_ledger',
           'recovery_phases', 'production_recovery_driver')


def require(value, message):
    if not value: raise ValueError(message)


def sha(path):
    return hashlib.sha256(raw(path)).hexdigest()


def source_flags():
    result = {}
    for filename, name in (('reservoir_maintenance.py', 'PRODUCTION_CONTROLLER_ENABLED'),
                           ('production_host.py', 'PRODUCTION_EXECUTION_ENABLED')):
        assignments = [node.value for node in ast.parse(raw(CORE_ROOT / filename)).body
                       if isinstance(node, ast.Assign)
                       and any(isinstance(target, ast.Name) and target.id == name for target in node.targets)]
        require(len(assignments) == 1 and isinstance(assignments[0], ast.Constant)
                and type(assignments[0].value) is bool, 'literal release flag required')
        result[name] = assignments[0].value
    return result


def validate_core(binding, *, enabled=False):
    require(binding.get('root') == str(CORE_ROOT), 'core root must be the exact05 directory')
    require(CORE_ROOT == CORE_ROOT.resolve(), 'core root alias refused')
    actual = {str(path): sha(path) for path in sorted(CORE_ROOT.glob('*.py'))}
    require(binding.get('artifacts') == actual and actual, 'exact05 source closure differs')
    flags = source_flags()
    require(binding.get('flags') == flags, 'release flags differ from actual source')
    if enabled:
        require(all(flags.values()), 'final production closure is not enabled')
    return actual


def loaded_origins(core=None):
    """Record actual loaded local modules, distinct from the complete source map."""
    result = {}
    for module in tuple(sys.modules.values()):
        filename = getattr(module, '__file__', None)
        if filename is None:
            continue
        path = Path(filename)
        if path.parent == CORE_ROOT:
            require(path.suffix == '.py' and path.resolve() == path, 'loaded core origin alias differs')
            result[str(path)] = sha(path)
    if core is not None:
        for name in MODULES:
            module = core['m' if name == 'reservoir_maintenance' else name]
            path = CORE_ROOT / (name + '.py')
            require(Path(module.__file__) == path and result.get(str(path)) == sha(path),
                    'required shared core origin missing')
    return result


def load_core(release_manifest):
    """Manifest validation is additional to coordinator's independent review gate.

    No test doubles, altered admission or module-global replacement is accepted.
    The new fixture Host remains owned_fixture even though production code is in
    the same preserved directory. This loader executes no OS control operation.
    """
    binding = release_manifest.get('core', {})
    actual = validate_core(binding)
    for name in MODULES:
        if name in sys.modules:
            require(Path(sys.modules[name].__file__).resolve() == CORE_ROOT/(name+'.py'),
                    'an alternate lifecycle module is already imported: '+name)
    original = list(sys.path)
    try:
        sys.path.insert(0, str(CORE_ROOT))
        loaded = {name: importlib.import_module(name) for name in MODULES}
    finally: sys.path[:] = original
    for name, module in loaded.items():
        path = Path(module.__file__)
        require(path == CORE_ROOT/(name+'.py') and sha(path) == actual[str(path)], 'loaded core origin differs')
    m = loaded.pop('reservoir_maintenance')
    require(m.PRODUCTION_CONTROLLER_ENABLED == binding['flags']['PRODUCTION_CONTROLLER_ENABLED'],
            'loaded controller release flag differs')
    require(all(module.m is m for module in loaded.values()), 'core modules do not share the exact controller')
    return {'m': m, **loaded}
