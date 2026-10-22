"""Read-only exact release validation, without NativeHost construction or entrypoint."""
import datetime
import hashlib
import importlib.abc
import json
import os
from pathlib import Path
import sys

E = Path(__file__).resolve().parent
N = E.parent / 'owned-fixture02-abort-01'
OUT = E / 'owned-fixture02-abort-release-validation-01.json'
RELEASE = N / 'evidence/abort-release-01.json'
CHECKPOINT = N / 'evidence/source-checkpoint-03.json'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(CHECKPOINT) == 'd9606685415a990a135d2400db912a921e3d6692bb2073f660676bed6edd4137'
assert sha(RELEASE) == 'bc7fe8c32e20f7e16a8f753315d27962fe583e6f7fe30565fbb76712f10fc02a'
artifacts = json.loads(CHECKPOINT.read_text())['artifacts']
assert all(sha(p) == value for p, value in artifacts.items())
assert not os.path.lexists(OUT)
violations = []
executed = {}
publishing = False

def reject(message):
    violations.append(message)
    raise AssertionError(message)

class Guard(importlib.abc.MetaPathFinder):
    def find_spec(self, fullname, path=None, target=None):
        if fullname.split('.')[0] in {'numpy', 'mlx', 'mlx_lm', 'torch', 'cv2', 'reservoir_service', 'coupled_astrid_server'}:
            reject('forbidden import: ' + fullname)

def audit(event, args):
    if event == 'exec':
        filename = str(args[0].co_filename)
        if filename.startswith('/Users/v/other/'):
            if filename not in artifacts or sha(filename) != artifacts[filename]:
                reject('unbound local source: ' + filename)
            executed[filename] = artifacts[filename]
    if event.startswith(('subprocess.', 'socket.', 'ctypes.')) or event in {
            'os.system', 'os.posix_spawn', 'os.fork', 'os.forkpty', 'os.exec', 'os.kill', 'os.killpg',
            'os.remove', 'os.rmdir', 'os.mkdir', 'os.rename', 'os.symlink', 'os.link',
            'os.chmod', 'os.chown', 'os.utime', 'os.truncate'}:
        reject('forbidden effect: ' + event)
    if event == 'open':
        path, mode, flags = args
        if flags & (os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND):
            if not (publishing and path == str(OUT) and OUT.parent.resolve() == E
                    and flags & (os.O_CREAT | os.O_EXCL) == (os.O_CREAT | os.O_EXCL)
                    and not os.path.lexists(OUT)):
                reject('forbidden write: ' + str(path))

sys.dont_write_bytecode = True
sys.path.insert(0, str(N))
sys.meta_path.insert(0, Guard())
sys.addaudithook(audit)
import abort_evidence as evidence
import abort_driver as driver

target, snapshot, original_release = evidence.target()
authorization, actual = driver.validate_authorization(RELEASE, target)
sources = evidence.verify_sources(target, original_release)
assert actual == artifacts and not violations
assert not os.path.lexists(target['abort_root'])
assert all(sha(p) == value for p, value in artifacts.items())
receipt = {'schema': 'owned_fixture02_abort_release_validation_v1', 'passed': True,
           'at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
           'validator': {'path': str(Path(__file__).resolve()), 'sha256': sha(__file__)},
           'release': authorization, 'artifacts': actual, 'sources': sources,
           'executed_local_sources': executed, 'forbidden_actions': violations,
           'abort_root_absent': True, 'native_host_constructed': False,
           'service_actions': False, 'production_qualified': False}
publishing = True
with OUT.open('x') as stream:
    json.dump(receipt, stream, indent=2, sort_keys=True)
    stream.write('\n')
print(json.dumps({'passed': True, 'path': str(OUT), 'sha256': sha(OUT)}))
