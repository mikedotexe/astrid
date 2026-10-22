"""Root-serialized stdlib fake validation; never an OS/numerical fixture."""
import contextlib
import datetime
import hashlib
import importlib.abc
import io
import json
from pathlib import Path
import sys
import time
import unittest

BASE = Path(__file__).resolve().parent.parent
FIXTURE = BASE / 'owned-recovery-fixture-02'
CORE = BASE / 'production-candidate-05'
SCOPE = sys.argv[1]
assert SCOPE == 'reader'
MODULES = tuple(p.stem for p in sorted(CORE.glob('test*.py')))
EXPECTED = 299
OUTPUT = BASE / 'evidence' / 'production05-combined-guarded-tests-02'
assert not OUTPUT.with_suffix('.json').exists() and not OUTPUT.with_suffix('.txt').exists()
FORBIDDEN = {'numpy', 'torch', 'mlx', 'mlx_lm', 'cv2', 'coremltools', 'Foundation', 'Quartz',
             'AVFoundation', 'sounddevice', 'sklearn', 'reservoir_service', 'coupled_astrid_server'}
violations = []
executed_local_sources = {}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


class ImportGuard(importlib.abc.MetaPathFinder):
    def find_spec(self, fullname, path=None, target=None):
        if fullname.split('.')[0] in FORBIDDEN:
            violations.append('import:' + fullname)
            raise AssertionError('forbidden target import: ' + fullname)


def audit(event, args):
    if event == 'exec':
        filename = str(args[0].co_filename)
        if filename.startswith('/Users/v/other/'):
            if filename not in before or digest(filename) != before[filename]:
                violations.append('unbound_local_exec:' + filename)
                raise AssertionError('unbound local Python source: ' + filename)
            executed_local_sources[filename] = before[filename]
    if event.startswith(('subprocess.', 'socket.', 'ctypes.')) or event in {
            'os.system', 'os.posix_spawn', 'os.fork', 'os.forkpty', 'os.exec', 'os.kill', 'os.killpg'}:
        violations.append(event)
        raise AssertionError('forbidden external action: ' + event)


checkpoint = FIXTURE / 'source-checkpoint-03.json'
frozen = json.loads(checkpoint.read_text())['artifacts']
assert all(digest(p) == sha for p, sha in frozen.items())
reader_expected = {
    'production_fixture_admission.py': '69f85904322db994707432d9f288df8e485607b6a1a6cc1a1ee7253a2bbd961a',
    'production_fixture_history.py': '506bd5cfeeda0dcb2315ec601aaa29e54eae58fa117310f5d60ffc66b83e8013',
    'test_production_fixture_admission.py': 'a58e4fa279710ed4f67a9c27f072bfb59d677e938874d8bbc1be8f848b8aa7d1'}
assert all(digest(CORE / name) == sha for name, sha in reader_expected.items())
paths = [*CORE.glob('*.py'), *FIXTURE.glob('*.py'), Path(__file__).resolve(),
         BASE / 'candidate-v2/maintenance_fixture_worker.py',
         BASE / 'candidate-v2/test_reservoir_maintenance.py',
         BASE / 'restore-qualification-02/child_admission.py',
         BASE.parent / 'reservoir-snapshot-candidate/matrix_host_supervision_v2.py',
         Path('/Users/v/other/astrid/scripts/model_environment.py')]
external_expected = {
    BASE / 'candidate-v2/test_reservoir_maintenance.py': '67275c384b058c2f90977d3ec8b60e29c1788d47f08cfa1f983c6dc31bfd0a15',
    BASE / 'restore-qualification-02/child_admission.py': '21b13f54390bbb058a26749d84aad625cda6df7f2df2e8917782e432107aa99d',
    BASE.parent / 'reservoir-snapshot-candidate/matrix_host_supervision_v2.py': 'fbaea5a9132b6d1ed92262d70dfadbd5f3d2cc2c93c2b4bb90662ed41e5d687d'}
assert all(digest(path) == sha for path, sha in external_expected.items())
before = {**frozen, **{str(p): digest(p) for p in paths}}
runs = FIXTURE / 'runs'
run_names_before = sorted(p.name for p in runs.iterdir()) if runs.exists() else []
sys.dont_write_bytecode = True
sys.path.insert(0, str(FIXTURE if SCOPE == 'fixture' else CORE))
sys.meta_path.insert(0, ImportGuard())
sys.addaudithook(audit)
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
clock = time.monotonic()
log = io.StringIO()
with contextlib.redirect_stdout(log), contextlib.redirect_stderr(log):
    result = unittest.TextTestRunner(stream=log, verbosity=2).run(
        unittest.defaultTestLoader.loadTestsFromNames(MODULES))
after = {path: digest(path) for path in before}
loaded = sorted(name for name in sys.modules if name.split('.')[0] in FORBIDDEN)
run_names_after = sorted(p.name for p in runs.iterdir()) if runs.exists() else []
flags_false = all('\n' + name + ' = False\n' in path.read_text() for path, name in (
    (FIXTURE / 'owned_fixture_coordinator.py', 'FIXTURE_EXECUTION_ENABLED'),
    (CORE / 'production_host.py', 'PRODUCTION_EXECUTION_ENABLED'),
    (CORE / 'reservoir_maintenance.py', 'PRODUCTION_CONTROLLER_ENABLED')))
passed = (result.wasSuccessful() and result.testsRun == EXPECTED and not violations and not loaded
          and before == after and flags_false and run_names_before == run_names_after)
with OUTPUT.with_suffix('.txt').open('x') as stream:
    stream.write(log.getvalue())
receipt = {'schema': 'root_guarded_fixture02_reader05_tests_v1', 'scope': 'production05_combined_stdlib_fakes_only',
    'passed': passed, 'tests': result.testsRun, 'expected_tests': EXPECTED,
    'failures': len(result.failures), 'errors': len(result.errors), 'skipped': len(result.skipped),
    'modules': list(MODULES), 'started_at': started,
    'completed_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'elapsed_s': time.monotonic() - clock, 'source_hashes_before': before,
    'source_hashes_after': after, 'source_unchanged': before == after, 'executed_local_sources': executed_local_sources,
    'forbidden_actions': violations, 'forbidden_modules': loaded, 'all_flags_false': flags_false,
    'fixture_run_names_before': run_names_before, 'fixture_run_names_after': run_names_after,
    'python': {'path': sys.executable, 'resolved': str(Path(sys.executable).resolve()),
               'sha256': digest(sys.executable)},
    'log': {'path': str(OUTPUT.with_suffix('.txt')), 'sha256': digest(OUTPUT.with_suffix('.txt'))},
    'actual_os_or_numerical_acceptance': False, 'production_qualified': False}
with OUTPUT.with_suffix('.json').open('x') as stream:
    json.dump(receipt, stream, indent=2, sort_keys=True)
    stream.write('\n')
print(json.dumps({'passed': passed, 'tests': result.testsRun, 'failures': len(result.failures),
    'errors': len(result.errors), 'receipt': str(OUTPUT.with_suffix('.json')),
    'sha256': digest(OUTPUT.with_suffix('.json'))}))
raise SystemExit(0 if passed else 1)
