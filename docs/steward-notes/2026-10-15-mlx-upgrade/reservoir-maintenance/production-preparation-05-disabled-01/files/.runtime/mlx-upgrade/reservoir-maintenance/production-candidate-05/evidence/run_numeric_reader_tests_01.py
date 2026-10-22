"""One root-released guarded stdlib fake test pass for numerical proof readers."""
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

ROOT = Path(__file__).resolve().parent.parent
MODULES = ('test_production_admission', 'test_production_restore_admission', 'test_production_service_admission')
FORBIDDEN = {'numpy', 'torch', 'mlx', 'mlx_lm', 'cv2', 'coremltools', 'Foundation', 'Quartz',
             'AVFoundation', 'sounddevice', 'sklearn', 'reservoir_service', 'coupled_astrid_server'}
violations = []


class ImportGuard(importlib.abc.MetaPathFinder):
    def find_spec(self, fullname, path=None, target=None):
        if fullname.split('.')[0] in FORBIDDEN:
            violations.append('import:' + fullname)
            raise AssertionError('forbidden target import: ' + fullname)


def audit(event, args):
    if event in {'subprocess.Popen', 'os.system', 'os.posix_spawn', 'os.fork', 'os.kill', 'os.killpg',
                 'socket.connect', 'socket.bind', 'socket.getaddrinfo'}:
        violations.append(event)
        raise AssertionError('forbidden external action: ' + event)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


checkpoint = ROOT / 'evidence/numerical-consumer-source-checkpoint-04.json'
frozen = json.loads(checkpoint.read_text())['artifacts']
dependencies = [ROOT / name for name in ('production_preflight.py', 'reservoir_maintenance.py',
                'reservoir_maintenance_state.py', 'restore_verifier.py')]
dependencies += [ROOT.parent / 'restore-qualification-02/child_admission.py',
                 ROOT.parent.parent / 'reservoir-snapshot-candidate/matrix_host_supervision_v2.py', Path(__file__).resolve()]
before = {**frozen, **{str(path): digest(path) for path in dependencies}}
assert all(digest(path) == sha for path, sha in before.items())
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT))
sys.meta_path.insert(0, ImportGuard()); sys.addaudithook(audit)
started = datetime.datetime.now(datetime.timezone.utc).isoformat(); clock = time.monotonic(); log = io.StringIO()
with contextlib.redirect_stdout(log), contextlib.redirect_stderr(log):
    result = unittest.TextTestRunner(stream=log, verbosity=2).run(unittest.defaultTestLoader.loadTestsFromNames(MODULES))
after = {path: digest(path) for path in before}
loaded = sorted(name for name in sys.modules if name.split('.')[0] in FORBIDDEN)
import reservoir_maintenance as controller
host_source = (ROOT / 'production_host.py').read_text()
flags_false = controller.PRODUCTION_CONTROLLER_ENABLED is False and '\nPRODUCTION_EXECUTION_ENABLED = False\n' in host_source
passed = result.wasSuccessful() and not violations and not loaded and before == after and flags_false
log_path = ROOT / 'evidence/numerical-reader-tests-01.txt'
with log_path.open('x') as stream: stream.write(log.getvalue())
receipt = {'schema': 'reservoir_production05_guarded_numeric_reader_tests_v1', 'scope': 'guarded_stdlib_fakes_only',
           'passed': passed, 'tests': result.testsRun, 'failures': len(result.failures), 'errors': len(result.errors),
           'skipped': len(result.skipped), 'modules': list(MODULES), 'started_at': started,
           'completed_at': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'elapsed_s': time.monotonic()-clock,
           'checkpoint': {'path': str(checkpoint), 'sha256': digest(checkpoint)},
           'source_hashes_before': before, 'source_hashes_after': after, 'source_unchanged': before == after,
           'forbidden_actions': violations, 'forbidden_modules': loaded, 'production_flags_false': flags_false,
           'python': {'path': sys.executable, 'resolved': str(Path(sys.executable).resolve()), 'sha256': digest(sys.executable)},
           'log': {'path': str(log_path), 'sha256': digest(log_path)}, 'actual_numerical_revalidation': False,
           'production_qualified': False}
path = ROOT / 'evidence/numerical-reader-tests-01.json'
with path.open('x') as stream: json.dump(receipt, stream, indent=2, sort_keys=True); stream.write('\n')
print(json.dumps({'passed': passed, 'tests': result.testsRun, 'failures': len(result.failures), 'errors': len(result.errors),
                  'receipt': str(path), 'sha256': digest(path)}))
raise SystemExit(0 if passed else 1)
