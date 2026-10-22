"""Root-serialized, source-bound stdlib fake tests; no native control authority."""
import contextlib
import datetime
import hashlib
import importlib.abc
import io
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
import time
import unittest

BASE = Path(__file__).resolve().parent.parent
SCOPE = sys.argv[1]
assert SCOPE in ('fixture03', 'production06')
FIXTURE = BASE / 'owned-recovery-fixture-03'
CORE = BASE / 'production-candidate-06'
SOURCE = FIXTURE if SCOPE == 'fixture03' else CORE
PLAN = BASE / 'evidence' / (SCOPE + '-test-plan-01.json')
OUTPUT = BASE / 'evidence' / (SCOPE + '-guarded-tests-01')
SCRATCH = BASE / 'evidence' / (SCOPE + '-test-scratch-01')
FORBIDDEN = {'numpy', 'torch', 'mlx', 'mlx_lm', 'cv2', 'coremltools', 'Foundation',
             'Quartz', 'AVFoundation', 'sounddevice', 'sklearn', 'reservoir_service',
             'coupled_astrid_server'}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def inventory(root):
    return {str(p): digest(p) for p in sorted(root.rglob('*')) if p.is_file()}


plan = json.loads(PLAN.read_text())
assert plan['scope'] == SCOPE + '_stdlib_fakes_only'
assert plan['expected_tests'] > 0
assert plan['runner_sha256'] == digest(__file__)
assert os.getuid() == 501 and os.geteuid() == 501
assert str(Path(sys.executable).resolve()) in plan['python_files']
assert all(digest(p) == h for p, h in plan['python_files'].items())
before = plan['sources']
assert all(digest(p) == h for p, h in before.items())
assert plan['artifacts'] and all(before.get(p) == h for p, h in plan['artifacts'].items())
assert set(str(p) for root in (FIXTURE, CORE) for p in root.glob('*.py')) <= set(before)
assert tuple(plan['modules']) == tuple(p.stem for p in sorted(SOURCE.glob('test*.py')))
def flags_disabled():
    return all('\n' + name + ' = False\n' in path.read_text() for path, name in (
        (FIXTURE / 'owned_fixture_coordinator.py', 'FIXTURE_EXECUTION_ENABLED'),
        (CORE / 'production_host.py', 'PRODUCTION_EXECUTION_ENABLED'),
        (CORE / 'reservoir_maintenance.py', 'PRODUCTION_CONTROLLER_ENABLED')))
assert flags_disabled()
assert not (FIXTURE / 'runs').exists()
protected = plan['protected_files']
assert all(digest(p) == h for p, h in protected.items())
preserved_roots = tuple(Path(p) for p in plan['preserved_roots'])
original_before = {str(root): inventory(root) for root in preserved_roots}
assert original_before == plan['original_inventory']
assert not SCRATCH.exists()
assert not OUTPUT.with_suffix('.json').exists() and not OUTPUT.with_suffix('.txt').exists()
SCRATCH.mkdir(mode=0o700)
tempfile.tempdir = str(SCRATCH)
violations = []
executed = {}
writing_outputs = False


def reject(message):
    violations.append(message)
    raise AssertionError(message)


class ImportGuard(importlib.abc.MetaPathFinder):
    def find_spec(self, fullname, path=None, target=None):
        if fullname.split('.')[0] in FORBIDDEN:
            reject('forbidden import: ' + fullname)


def writable(path, dir_fd=None, *, follow_leaf=True):
    if isinstance(path, int):
        try:
            identity = os.fstat(path)
            if not stat.S_ISREG(identity.st_mode) or identity.st_nlink != 1:
                return False
            matches = [p for p in SCRATCH.rglob('*') if p.is_file() and not p.is_symlink()
                       and p.resolve().is_relative_to(SCRATCH)
                       and (p.stat().st_dev, p.stat().st_ino) == (identity.st_dev, identity.st_ino)]
            return len(matches) == 1
        except (OSError, ValueError):
            return False
    try:
        candidate = Path(os.fsdecode(path))
        if not candidate.is_absolute() and dir_fd not in (None, -1):
            identity = os.fstat(dir_fd)
            directories = [SCRATCH, *(p for p in SCRATCH.rglob('*') if p.is_dir() and not p.is_symlink())]
            roots = [p for p in directories if (p.stat().st_dev, p.stat().st_ino) ==
                     (identity.st_dev, identity.st_ino)]
            if len(roots) != 1:
                return False
            candidate = roots[0] / candidate
        candidate = candidate.absolute()
        if candidate.is_relative_to(SCRATCH):
            checked = candidate.resolve() if follow_leaf else candidate.parent.resolve() / candidate.name
            return checked.is_relative_to(SCRATCH)
        return False
    except (OSError, TypeError, ValueError):
        return False


def audit(event, args):
    if event == 'exec':
        filename = str(args[0].co_filename)
        if filename.startswith('/Users/v/other/'):
            if filename not in before or digest(filename) != before[filename]:
                reject('unbound local exec: ' + filename)
            executed[filename] = before[filename]
    if event.startswith(('subprocess.', 'socket.', 'ctypes.')) or event in {
            'os.system', 'os.posix_spawn', 'os.fork', 'os.forkpty', 'os.exec', 'os.kill', 'os.killpg'}:
        reject('forbidden external action: ' + event)
    if event == 'open':
        path, mode, flags = args
        writing = bool(flags & (os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND))
        output_open = (writing_outputs and isinstance(path, (str, bytes, os.PathLike))
                       and Path(os.fsdecode(path)) in (OUTPUT.with_suffix('.json'), OUTPUT.with_suffix('.txt'))
                       and Path(os.fsdecode(path)).parent.resolve() == OUTPUT.parent
                       and flags & (os.O_CREAT | os.O_EXCL) == (os.O_CREAT | os.O_EXCL)
                       and not os.path.lexists(path))
        if writing and not writable(path) and not output_open:
            reject('forbidden write: ' + str(path))
    if event in {'os.remove', 'os.rmdir', 'os.mkdir', 'os.chmod', 'os.chown', 'os.utime', 'os.truncate'}:
        fd_index = {'os.remove': 1, 'os.rmdir': 1, 'os.mkdir': 2, 'os.chmod': 2,
                    'os.chown': 3, 'os.utime': 3}.get(event)
        dir_fd = args[fd_index] if fd_index is not None and len(args) > fd_index else None
        if not writable(args[0], dir_fd, follow_leaf=event not in {'os.remove', 'os.rmdir', 'os.mkdir'}):
            reject('forbidden filesystem mutation: ' + event + ':' + str(args[0]))
    if event in {'os.rename', 'os.link'}:
        if not all(writable(p, args[i+2] if len(args) > i+2 else None, follow_leaf=event != 'os.rename')
                   for i, p in enumerate(args[:2])):
            reject('forbidden filesystem move/link: ' + event)
    if event == 'os.symlink' and not writable(args[1], args[2] if len(args) > 2 else None, follow_leaf=False):
        reject('forbidden symlink destination')


sys.dont_write_bytecode = True
sys.path.insert(0, str(SOURCE))
sys.meta_path.insert(0, ImportGuard())
sys.addaudithook(audit)
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
clock = time.monotonic()
log = io.StringIO()
with contextlib.redirect_stdout(log), contextlib.redirect_stderr(log):
    result = unittest.TextTestRunner(stream=log, verbosity=2).run(
        unittest.defaultTestLoader.loadTestsFromNames(plan['modules']))
after = {p: digest(p) for p in before}
protected_after = {p: digest(p) for p in protected}
original_after = {str(root): inventory(root) for root in preserved_roots}
flags_false = flags_disabled()
new_fixture_run_absent = not (FIXTURE / 'runs').exists()
loaded = sorted(name for name in sys.modules if name.split('.')[0] in FORBIDDEN)
passed = (result.wasSuccessful() and result.testsRun == plan['expected_tests'] and not result.skipped
          and not violations and not loaded and before == after and protected == protected_after
          and original_before == original_after and flags_false and new_fixture_run_absent)
writing_outputs = True
with OUTPUT.with_suffix('.txt').open('x') as stream:
    stream.write(log.getvalue())
receipt = {'schema': 'root_guarded_fixture03_candidate06_tests_v1', 'passed': passed,
    'scope': plan['scope'], 'tests': result.testsRun, 'expected_tests': plan['expected_tests'],
    'artifacts': plan['artifacts'],
    'failures': len(result.failures), 'errors': len(result.errors), 'skipped': len(result.skipped),
    'modules': plan['modules'], 'started_at': started,
    'completed_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'elapsed_s': time.monotonic() - clock, 'source_hashes_before': before, 'source_hashes_after': after,
    'executed_local_sources': executed, 'forbidden_actions': violations, 'forbidden_modules': loaded,
    'protected_files_unchanged': protected == protected_after,
    'original_inventory_unchanged': original_before == original_after,
    'original_inventory_before': original_before, 'original_inventory_after': original_after,
    'plan': {'path': str(PLAN), 'sha256': digest(PLAN)},
    'python': {'path': sys.executable, 'resolved': str(Path(sys.executable).resolve()),
               'sha256': digest(sys.executable)},
    'log': {'path': str(OUTPUT.with_suffix('.txt')), 'sha256': digest(OUTPUT.with_suffix('.txt'))},
    'all_flags_false': flags_false, 'new_fixture_run_absent': new_fixture_run_absent,
    'actual_os_acceptance': False, 'production_qualified': False}
with OUTPUT.with_suffix('.json').open('x') as stream:
    json.dump(receipt, stream, indent=2, sort_keys=True)
    stream.write('\n')
print(json.dumps({'passed': passed, 'tests': result.testsRun, 'failures': len(result.failures),
    'errors': len(result.errors), 'receipt': str(OUTPUT.with_suffix('.json')),
    'sha256': digest(OUTPUT.with_suffix('.json'))}))
raise SystemExit(0 if passed else 1)
