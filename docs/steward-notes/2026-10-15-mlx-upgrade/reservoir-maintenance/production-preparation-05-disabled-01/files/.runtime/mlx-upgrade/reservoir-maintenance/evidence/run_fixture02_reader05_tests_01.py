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
assert SCOPE in ('fixture', 'reader')
MODULES = ('test_owned_fixture_coordinator', 'test_owned_fixture_pin',
           'test_owned_fixture_run', 'test_owned_recovery_host') if SCOPE == 'fixture' else (
               'test_production_fixture_admission',)
EXPECTED = 62 if SCOPE == 'fixture' else 37
OUTPUT = BASE / 'evidence' / (SCOPE + '-guarded-tests-01')
assert not OUTPUT.with_suffix('.json').exists() and not OUTPUT.with_suffix('.txt').exists()
FORBIDDEN = {'numpy', 'torch', 'mlx', 'mlx_lm', 'cv2', 'coremltools', 'Foundation', 'Quartz',
             'AVFoundation', 'sounddevice', 'sklearn', 'reservoir_service', 'coupled_astrid_server'}
violations = []


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


class ImportGuard(importlib.abc.MetaPathFinder):
    def find_spec(self, fullname, path=None, target=None):
        if fullname.split('.')[0] in FORBIDDEN:
            violations.append('import:' + fullname)
            raise AssertionError('forbidden target import: ' + fullname)


def audit(event, args):
    if event.startswith(('subprocess.', 'socket.', 'ctypes.')) or event in {
            'os.system', 'os.posix_spawn', 'os.fork', 'os.forkpty', 'os.exec', 'os.kill', 'os.killpg'}:
        violations.append(event)
        raise AssertionError('forbidden external action: ' + event)


checkpoint = FIXTURE / 'source-checkpoint-03.json'
frozen = json.loads(checkpoint.read_text())['artifacts']
assert all(digest(p) == sha for p, sha in frozen.items())
reader_expected = {
    'production_fixture_admission.py': 'd86f58e54c73f09c2a48c94adde637915300c3158fa0f3c09d3ee6f9e4d6c5fe',
    'production_fixture_history.py': '2454597e2746df37e82ef6668a523d202e366a491bdda926d3e37f9731f541b1',
    'test_production_fixture_admission.py': 'fde5aab52567253224944d35f5079fb719de5bbc54e05bd9261cc2db82e64fa8'}
assert all(digest(CORE / name) == sha for name, sha in reader_expected.items())
paths = [*CORE.glob('*.py'), *FIXTURE.glob('*.py'), Path(__file__).resolve(),
         BASE / 'candidate-v2/maintenance_fixture_worker.py',
         Path('/Users/v/other/astrid/scripts/model_environment.py')]
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
receipt = {'schema': 'root_guarded_fixture02_reader05_tests_v1', 'scope': SCOPE + '_stdlib_fakes_only',
    'passed': passed, 'tests': result.testsRun, 'expected_tests': EXPECTED,
    'failures': len(result.failures), 'errors': len(result.errors), 'skipped': len(result.skipped),
    'modules': list(MODULES), 'started_at': started,
    'completed_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'elapsed_s': time.monotonic() - clock, 'source_hashes_before': before,
    'source_hashes_after': after, 'source_unchanged': before == after,
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
