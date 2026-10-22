"""One bounded read-only release validation; never invokes a native entrypoint."""
import datetime
import hashlib
import importlib
import json
import os
from pathlib import Path
import sys
import time

ROOT = Path('/Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-maintenance')
EVIDENCE = ROOT / 'evidence'
INPUTS = EVIDENCE / 'owned-fixture02-final-source-inputs-01.json'
RELEASE = EVIDENCE / 'owned-fixture02-final-release-01.json'
OUTPUT = EVIDENCE / 'owned-fixture02-final-release-validation-01.json'
FIXTURE = ROOT / 'owned-recovery-fixture-02'


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    started = time.monotonic()
    assert sha(INPUTS) == '2ae353e1c80d5e40adabd4194b4aba9a355a3d2092105018cd23fec386595180'
    inputs = json.loads(INPUTS.read_text())
    bindings = {**inputs['fixture_artifacts'], **inputs['core_artifacts']}
    assert all(sha(path) == wanted for path, wanted in bindings.items())
    before = sha(RELEASE)
    violations = []
    executed = {}
    forbidden_modules = {'numpy', 'scipy', 'mlx', 'mlx_lm', 'torch', 'cv2',
                         'CoreML', 'coremltools', 'AVFoundation', 'Quartz', 'ctypes'}
    forbidden_events = {'subprocess.Popen', 'os.system', 'os.fork', 'os.forkpty',
                        'os.exec', 'os.posix_spawn', 'os.kill', 'os.killpg',
                        'os.remove', 'os.rmdir', 'os.rename', 'os.mkdir', 'os.link',
                        'os.symlink', 'os.truncate', 'os.chmod', 'os.chown', 'os.utime'}
    receipt = {'schema': 'owned_fixture02_read_only_release_validation_v1',
               'started_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
               'runner_sha256': sha(__file__), 'inputs_sha256': sha(INPUTS),
               'release': {'path': str(RELEASE), 'sha256': before},
               'actual_os_fixture_executed': False, 'production_qualified': False,
               'native_entrypoints_called': False}
    # Only this new receipt is opened for output before the target-import guard.
    with OUTPUT.open('x') as output:
        def refuse(event):
            violations.append(event)
            raise RuntimeError('read-only guard refused ' + event)

        def audit(event, args):
            if time.monotonic() - started > 30:
                refuse('cooperative_30_second_deadline')
            if event in forbidden_events or event.startswith(('socket.', 'ctypes.')):
                refuse(event)
            if event == 'import' and args[0].split('.')[0] in forbidden_modules:
                refuse('import:' + args[0])
            if event == 'open':
                flags = args[2]
                if isinstance(flags, int) and flags & (os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND):
                    refuse('write_open')
            if event == 'exec':
                filename = args[0].co_filename
                if filename.startswith('/Users/v/other/'):
                    if bindings.get(filename) != sha(filename):
                        refuse('unbound_local_source:' + filename)
                    executed[filename] = bindings[filename]

        sys.addaudithook(audit)
        try:
            sys.path.insert(0, str(FIXTURE))
            coordinator = importlib.import_module('owned_fixture_coordinator')
            release = coordinator.validate_release(RELEASE)
            assert coordinator.FIXTURE_EXECUTION_ENABLED is True
            assert sha(RELEASE) == before
            assert all(sha(path) == wanted for path, wanted in bindings.items())
            root = Path(release['fixture']['root'])
            result = Path(release['result_path'])
            assert not root.exists() and not root.is_symlink()
            assert not result.exists() and not result.is_symlink()
            accepted_elapsed = time.monotonic() - started
            assert accepted_elapsed <= 30, 'cooperative whole-validation deadline exceeded'
            receipt.update(passed=True, fixture_root_unused=True, result_path_unused=True,
                           final_source_files=len(bindings), executor_steps=len(release['executor_plan']),
                           acceptance_elapsed_s=accepted_elapsed)
        except BaseException as error:
            receipt.update(passed=False, error_type=type(error).__name__, error=str(error))
        receipt.update(completed_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                       elapsed_s=time.monotonic() - started, guard_violations=violations,
                       executed_local_sources=executed)
        json.dump(receipt, output, indent=2, sort_keys=True)
        output.write('\n')
    print(json.dumps({'passed': receipt['passed'], 'output': str(OUTPUT)}))
    return 0 if receipt['passed'] and not violations else 1


if __name__ == '__main__':
    raise SystemExit(main())
