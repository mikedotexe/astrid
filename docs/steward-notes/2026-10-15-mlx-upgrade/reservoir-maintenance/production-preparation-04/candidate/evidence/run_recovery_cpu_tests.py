"""Guarded stdlib-only test receipt; no actual Host or numerical execution."""
import contextlib
from datetime import datetime, timezone
import hashlib
import importlib.abc
import io
import json
from pathlib import Path
import sys
import time
import unittest

HERE = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(HERE))
FORBIDDEN = ('numpy', 'mlx', 'mlx_lm', 'cv2', 'CoreML', 'Foundation', 'Quartz')
violations = []
class Imports(importlib.abc.MetaPathFinder):
    def find_spec(self, fullname, path=None, target=None):
        if fullname.split('.')[0] in FORBIDDEN:
            violations.append('import:' + fullname)
            raise AssertionError('forbidden numeric/hardware import: ' + fullname)

def audit(event, arguments):
    if event in ('subprocess.Popen', 'os.system', 'os.posix_spawn', 'os.fork', 'os.kill', 'os.killpg',
                 'socket.connect', 'socket.bind', 'socket.getaddrinfo'):
        violations.append(event)
        raise AssertionError('forbidden external action: ' + event)

sys.meta_path.insert(0, Imports()); sys.addaudithook(audit)
modules = ['test_transaction_execution', 'test_recovery_phases', 'test_production_recovery_driver',
           'test_production_controller', 'test_production_recovery', 'test_production_recovery_inspect',
           'test_production_retained']
paths = sorted(HERE.glob('*.py')) + [HERE.parent/'candidate-v2/test_reservoir_maintenance.py', Path(__file__).resolve()]
def hashes(): return {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in paths}
artifacts = hashes(); started = datetime.now(timezone.utc).isoformat(); clock = time.monotonic(); log = io.StringIO()
with contextlib.redirect_stdout(log), contextlib.redirect_stderr(log):
    result = unittest.TextTestRunner(stream=log, verbosity=2).run(unittest.defaultTestLoader.loadTestsFromNames(modules))
import reservoir_maintenance as controller
import production_host as host
unchanged = hashes() == artifacts
loaded = sorted(name for name in sys.modules if name.split('.')[0] in FORBIDDEN)
passed = result.wasSuccessful() and not violations and not loaded and unchanged
passed = passed and controller.PRODUCTION_CONTROLLER_ENABLED is False and host.PRODUCTION_EXECUTION_ENABLED is False
log_path = HERE/'evidence/recovery-cpu-tests-01.txt'
with log_path.open('x') as stream: stream.write(log.getvalue())
python = Path(sys.executable)
receipt = {'schema':'reservoir_production_recovery_cpu_tests_v2','scope':'guarded_stdlib_inert_fakes_only',
           'passed':passed,'tests_run':result.testsRun,'failures':len(result.failures),'errors':len(result.errors),
           'skipped':len(result.skipped),'started_at':started,'completed_at':datetime.now(timezone.utc).isoformat(),
           'wall_s':time.monotonic()-clock,'modules':modules,'artifacts':artifacts,
           'source_unchanged_during_suite':unchanged,'forbidden_actions':violations,'forbidden_imports_loaded':loaded,
           'python':{'path':str(python),'resolved_path':str(python.resolve()),'sha256':hashlib.sha256(python.read_bytes()).hexdigest()},
           'log':{'path':str(log_path),'sha256':hashlib.sha256(log_path.read_bytes()).hexdigest()},
           'production_enabled':False,'production_executed':False,'production_recovery_qualified':False,
           'limitations':['No real owned launchd interruption/continuation fixture or production acceptance.',
             'Numeric restore/service qualification and fresh ownership/host admission remain independently required.']}
path = HERE/'evidence/recovery-cpu-tests-01.json'
with path.open('x') as stream: json.dump(receipt,stream,sort_keys=True,indent=2);stream.write('\n')
print(json.dumps({'passed':passed,'tests_run':result.testsRun,'failures':len(result.failures),'errors':len(result.errors),
                  'receipt':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}))
raise SystemExit(0 if passed else 1)
