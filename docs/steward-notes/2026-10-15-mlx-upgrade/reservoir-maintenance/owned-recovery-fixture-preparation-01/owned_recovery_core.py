"""Load the immutable04 shared lifecycle core, without enabling production."""
from __future__ import annotations
import hashlib
import importlib
from pathlib import Path
import sys

CORE_ROOT = Path(__file__).resolve().parent.parent / 'production-candidate-04'
MODULES = ('reservoir_maintenance', 'transaction_execution', 'recovery_ledger',
           'recovery_phases', 'production_recovery_driver')


def require(value, message):
    if not value: raise ValueError(message)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def load_core(release_manifest):
    """Manifest validation is additional to coordinator's independent review gate.

    No test doubles, altered admission or module-global replacement is accepted.
    The new fixture Host remains owned_fixture even though production code is in
    the same preserved directory. This loader executes no OS control operation.
    """
    binding = release_manifest.get('core', {})
    require(binding.get('root') == str(CORE_ROOT), 'core root must be the frozen04 directory')
    require(CORE_ROOT == CORE_ROOT.resolve(), 'core root alias refused')
    actual = {str(path): sha(path) for path in sorted(CORE_ROOT.glob('*.py'))}
    require(binding.get('artifacts') == actual and actual, 'exact04 source closure differs')
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
    require(m.PRODUCTION_CONTROLLER_ENABLED is False, 'this preparatory fixture requires disabled04')
    require(all(module.m is m for module in loaded.values()), 'core modules do not share the exact controller')
    return {'m': m, **loaded}
