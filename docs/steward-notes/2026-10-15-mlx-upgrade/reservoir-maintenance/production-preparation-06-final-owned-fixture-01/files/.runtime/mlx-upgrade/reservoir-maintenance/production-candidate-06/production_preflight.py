"""Fixed Astrid/reservoir preparation and read-only capture primitives.

No import probes a service, creates output, or imports numerical libraries.
The default CLI checks an existing preparation only; it cannot execute a handoff.
Loaded-source alignment is supplied as separately reviewed provenance, never
inferred merely from today's source files or an old process command line.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import re
import shlex
import stat
import subprocess
import production_native as native
import urllib.error
import urllib.request

import reservoir_maintenance as m

HERE = Path(__file__).resolve().parent
ASTRID = Path('/Users/v/other/astrid')
WORKSPACE = Path('/Users/v/other/neural-triple-reservoir')
STATE_ROOT = WORKSPACE / 'state'
HOLD_SOURCE = ASTRID / 'scripts/model_environment.py'
CONTROL_ROOT = HERE.parent / 'production-transactions'
PLISTS = {role: Path('/Users/v/Library/LaunchAgents') / (label + '.plist') for role, label in m.LABELS.items()}
SOURCES = {'provider': WORKSPACE / 'coupled_astrid_server.py', 'reservoir': WORKSPACE / 'reservoir_service.py'}
LOGS = {'provider': WORKSPACE / 'logs/coupled-astrid.log', 'reservoir': WORKSPACE / 'logs/reservoir-service.log'}
QOS = WORKSPACE / 'workspace/model_qos_receipts.jsonl'
JOURNAL = STATE_ROOT / 'coupling_journal.json'
STEWARD = ASTRID / 'capsules/spectral-bridge/workspace/diagnostics/steward_control_v1'
PROTECTED = {'bridge', 'astrid_feeder', 'minime_feeder', 'collab_feeder', 'perception', 'ollama', 'minime', 'minime_engine'}
MINIME = Path('/Users/v/other/minime')
ENGINE_INPUTS = {MINIME / name for name in (
    'minime/target/release/minime', 'scripts/launchd_minime_engine.sh',
    'scripts/minime_rescue_investigation.py', 'workspace/rescue_profile.json',
    'workspace/division/runtime-manifest.json', 'minime/shaders/av_features.metal')}
ENGINE_INPUTS.add(Path('/Users/v/Library/LaunchAgents/com.minime.engine.plist'))
RESTORE_NAMES = {'reservoir_service.py', 'persistence.py', 'dual_ai_bridge.py',
                 'triple_reservoir_coreml.py', 'lsm_reservoir.py', 'rehearsal.py'}
MAX_CAPTURE = 256 * 1024 * 1024


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def load_hold():
    spec = importlib.util.spec_from_file_location('production_hold_primitives', HOLD_SOURCE)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def reference(ref):
    m.require(isinstance(ref, dict) and isinstance(ref.get('path'), str) and Path(ref['path']).is_absolute()
              and m.hash_value(ref.get('sha256')), 'invalid artifact reference')
    path = Path(ref['path'])
    m.require(not path.is_symlink() and sha(path) == ref['sha256'], 'artifact reference changed')
    return m.read_record(path)


def stable_bytes(path, *, maximum=MAX_CAPTURE):
    path = Path(path)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        before = os.fstat(fd)
        m.require(stat.S_ISREG(before.st_mode) and before.st_uid == os.getuid()
                  and before.st_nlink == 1 and before.st_size <= maximum, 'unowned, special or oversized input')
        chunks, count = [], 0
        while chunk := os.read(fd, 1024 * 1024):
            chunks.append(chunk); count += len(chunk)
            m.require(count <= maximum, 'input grew beyond bound')
        after = os.fstat(fd)
        identity = lambda s: (s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns)
        m.require(identity(before) == identity(after) == identity(path.lstat()), 'input changed while read')
        return b''.join(chunks), before
    finally:
        os.close(fd)


def cursor(path):
    """Bind the complete retained prefix, detecting truncation and same-inode rewrites."""
    fd, info = open_log(path)
    try:
        digest, last = hash_prefix(fd, info.st_size)
        m.require(info.st_size == 0 or last == b'\n', 'capture anchor falls inside a log record')
        check_log(fd, path, info, info.st_size)
        return {'path': str(path), 'device': info.st_dev, 'inode': info.st_ino,
                'start_offset': info.st_size, 'prefix_sha256': digest}
    finally:
        os.close(fd)


def open_log(path):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    info = os.fstat(fd)
    if not (stat.S_ISREG(info.st_mode) and info.st_uid == os.getuid() and info.st_nlink == 1):
        os.close(fd)
        raise m.Refusal('log is not an owned regular file')
    return fd, info


def check_log(fd, path, original, minimum):
    for info in (os.fstat(fd), Path(path).lstat()):
        m.require((info.st_dev, info.st_ino) == (original.st_dev, original.st_ino)
                  and info.st_size >= minimum, 'log rotated or truncated during capture')


def hash_prefix(fd, length):
    digest, remaining, last = hashlib.sha256(), length, b''
    while remaining:
        chunk = os.read(fd, min(1024 * 1024, remaining))
        m.require(chunk, 'short log prefix read')
        remaining -= len(chunk); digest.update(chunk); last = chunk[-1:]
    return digest.hexdigest(), last


def capture_span(anchor, destination):
    fd, info = open_log(anchor['path'])
    try:
        start = anchor['start_offset']
        m.require((info.st_dev, info.st_ino) == (anchor['device'], anchor['inode'])
                  and start <= info.st_size <= start + MAX_CAPTURE, 'invalid or oversized captured log span')
        prefix, _ = hash_prefix(fd, start)
        m.require(prefix == anchor['prefix_sha256'], 'captured log prefix was rewritten')
        destination = Path(destination)
        out = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        digest, remaining, last = hashlib.sha256(), info.st_size - start, b''
        with os.fdopen(out, 'wb') as handle:
            while remaining:
                chunk = os.read(fd, min(1024 * 1024, remaining))
                m.require(chunk, 'short captured tail')
                remaining -= len(chunk); digest.update(chunk); last = chunk[-1:]
                handle.write(chunk)
            m.require(info.st_size == start or last == b'\n', 'capture ends inside a log record')
            handle.flush(); os.fsync(handle.fileno())
        check_log(fd, anchor['path'], info, info.st_size)
        m.sync_dir(destination.parent)
        return {'device': info.st_dev, 'inode': info.st_ino, 'start_offset': start,
                'end_offset': info.st_size, 'sha256': digest.hexdigest()}
    finally:
        os.close(fd)


def idle_health(value):
    worker = value.get('worker', {})
    return (value.get('model') == 'coupled-astrid' and value.get('ready') is True
            and not value.get('error') and worker.get('phase') == 'ready'
            and type(worker.get('queue_depth')) is int and worker['queue_depth'] == 0
            and worker.get('last_generation_error') is None
            and value.get('reservoir', {}).get('status') == 'connected')


def provider_anchor(io, expected):
    m.require(io.role('provider')['process'] == expected, 'provider changed before capture')
    before = {'log': cursor(LOGS['provider']), 'qos': cursor(QOS)}
    first, second = io.health(), io.health()
    after = {'log': cursor(LOGS['provider']), 'qos': cursor(QOS)}
    m.require(before == after and idle_health(first) and idle_health(second),
              'provider capture requires positive idle/empty health and unchanged log/QoS offsets')
    m.require(io.role('provider')['process'] == expected, 'provider changed during capture')
    return {'identity': expected, **before, 'health': [first, second],
            'start_worker_idle': True, 'start_queue_empty': True}


def reservoir_anchor(io, expected):
    m.require(io.role('reservoir')['process'] == expected, 'reservoir changed before inventory')
    before = cursor(LOGS['reservoir'])
    inventory = io.list_handles()
    fd, info = open_log(LOGS['reservoir'])
    try:
        m.require((info.st_dev, info.st_ino) == (before['device'], before['inode'])
                  and before['start_offset'] <= info.st_size <= before['start_offset'] + 1024 * 1024,
                  'reservoir log changed before inventory boundary')
        prefix, _ = hash_prefix(fd, before['start_offset'])
        m.require(prefix == before['prefix_sha256'], 'reservoir prefix changed before inventory boundary')
        remaining, chunks = info.st_size - before['start_offset'], []
        while remaining:
            chunk = os.read(fd, remaining)
            m.require(chunk, 'short inventory-boundary log span')
            remaining -= len(chunk); chunks.append(chunk)
        interval = b''.join(chunks).decode()
        m.require(not interval or interval.endswith('\n'), 'partial lifecycle record at inventory boundary')
        check_log(fd, LOGS['reservoir'], info, info.st_size)
    finally:
        os.close(fd)
    m.require(not re.search(r"(?:created handle|destroyed handle|restored) '", interval),
              'handle lifecycle crossed inventory boundary; obtain a fresh anchor')
    names = [item.get('name') for item in inventory.get('handles', [])]
    m.require(inventory.get('type') == 'list_handles_response' and all(isinstance(n, str) and n for n in names)
              and len(names) == len(set(names)), 'invalid read-only handle inventory')
    m.require(io.role('reservoir')['process'] == expected, 'reservoir changed during inventory')
    return {'identity': expected, 'log': before, 'inventory': inventory,
            'inventory_matches_start_boundary': True}


class ReadOnlySystem:
    """The only service requests here are health GET and list_handles."""
    def __init__(self, *, recorder=None):
        self.recorder=recorder

    def inventory_argv(self):
        code = ('import json; from websockets.sync.client import connect; '
                'c=connect("ws://127.0.0.1:7881",open_timeout=5,close_timeout=5,max_size=4194304); '
                'c.send(json.dumps({"type":"list_handles"})); print(c.recv(timeout=5)); c.close()')
        return [str(WORKSPACE/'.venv/bin/python'),'-B','-c',code]

    def command(self, *argv):
        m.require(Path(native.__file__).resolve()==HERE/'production_native.py','native helper loaded origin differs')
        m.require(native.readonly_argv(argv,uid=os.getuid(),labels=m.LABELS,ports=m.PORTS,
                  inventory_argv=self.inventory_argv()),'non-read-only native query refused')
        m.require(type(self.recorder) is native.Recorder,'explicit owned native recorder required before query')
        return self.recorder.run(list(argv),kind='query',timeout=20)

    def process(self, pid):
        if pid is None:
            return None
        first = self.command('/bin/ps', '-o', 'lstart=', '-p', str(pid))
        if first.returncode or not first.stdout.strip():
            m.require(first.returncode == 1 and not first.stdout.strip() and not first.stderr.strip(),
                      'process inspection failed without proven absence')
            return None
        cmd = self.command('/bin/ps', '-ww', '-o', 'command=', '-p', str(pid))
        last = self.command('/bin/ps', '-o', 'lstart=', '-p', str(pid))
        if last.returncode or not last.stdout.strip():
            m.require(last.returncode == 1 and not last.stdout.strip() and not last.stderr.strip(),
                      'process disappeared query failed without proven absence')
            return None
        m.require(cmd.returncode == 0 and first.stdout.strip() == last.stdout.strip(), 'PID changed during inspection')
        return {'pid': pid, 'started_at': first.stdout.strip(), 'argv': shlex.split(cmd.stdout.strip())}

    def job(self, role):
        m.require(role in m.ROLES, 'unknown fixed role')
        target = f'gui/{os.getuid()}/{m.LABELS[role]}'
        result = self.command('/bin/launchctl', 'print', target)
        if result.returncode:
            m.require('Could not find service' in result.stderr or 'Could not find specified service' in result.stderr,
                      'launchd inspection failed without proven absence')
            return False, None
        found = re.search(r'^\s*pid = (\d+)\s*$', result.stdout, re.MULTILINE)
        return True, int(found[1]) if found else None

    def listeners(self, role):
        result = self.command('/usr/sbin/lsof', '-nP', f'-iTCP:{m.PORTS[role]}', '-sTCP:LISTEN', '-Fp')
        m.require(result.returncode in (0, 1) and not result.stderr.strip(), 'listener inspection failed')
        return sorted({int(line[1:]) for line in result.stdout.splitlines() if line.startswith('p')})

    def all_listeners(self, pid):
        m.require(type(pid) is int and pid > 1, 'exact holder PID required')
        result = self.command('/usr/sbin/lsof', '-nP', '-a', '-p', str(pid), '-iTCP', '-sTCP:LISTEN', '-Fp')
        m.require(result.returncode in (0, 1) and not result.stderr.strip(), 'holder listener inspection failed')
        return sorted({int(line[1:]) for line in result.stdout.splitlines() if line.startswith('p')})

    def role(self, role):
        loaded, pid = self.job(role)
        process = self.process(pid)
        if pid is not None and process is None:
            loaded, newer = self.job(role)
            m.require(newer != pid, 'disappeared process is still reported by launchd')
            pid, process = newer, self.process(newer)
            m.require(pid is None or process is not None, 'replacement identity not yet stable')
        return {'target': f'gui/{os.getuid()}/{m.LABELS[role]}', 'job_loaded': loaded,
                'process': process, 'listener_pids': self.listeners(role), 'held': None}

    def host(self):
        return {'uid': os.getuid(), 'boot_uuid': self.command('/usr/sbin/sysctl', '-n', 'kern.bootsessionuuid').stdout.strip(),
                'hostname': os.uname().nodename,
                'os_version': self.command('/usr/bin/sw_vers', '-productVersion').stdout.strip()}

    def stewardship(self):
        control = m.read_record(STEWARD / 'control.json')
        lease_path = STEWARD / 'lease.json'
        lease = json.loads(lease_path.read_text()) if lease_path.exists() else None
        return {'paused': control.get('paused'), 'pause_generation': control.get('pause_generation'),
                'pause_actor': control.get('actor'), 'lease': lease}

    def health(self):
        try:
            response = urllib.request.urlopen('http://127.0.0.1:8090/healthz', timeout=5)
        except urllib.error.HTTPError as error:
            m.require(error.code == 503, 'unexpected health HTTP failure')
            response = error  # Starting/not-ready is a valid bounded observation.
        with response:
            body = response.read(1024 * 1024 + 1)
        m.require(len(body) <= 1024 * 1024, 'oversized health response')
        return json.loads(body)

    def list_handles(self):
        # Explicit capture only. No service/model imports and no write request.
        code = ('import json; from websockets.sync.client import connect; '
                'c=connect("ws://127.0.0.1:7881",open_timeout=5,close_timeout=5,max_size=4194304); '
                'c.send(json.dumps({"type":"list_handles"})); print(c.recv(timeout=5)); c.close()')
        result = self.command(str(WORKSPACE / '.venv/bin/python'), '-B', '-c', code)
        m.require(result.returncode == 0 and not result.stderr.strip() and len(result.stdout) <= 4194304,
                  'read-only handle inventory failed')
        return json.loads(result.stdout)


def validate_process_inventory(plan):
    """Eight unaffected roles and two managed roles must name ten processes."""
    m.require(set(plan['protected']) == PROTECTED, 'complete eight-process protected inventory required')
    identities = list(plan['protected'].values()) + [plan['roles'][role]['identity'] for role in m.ROLES]
    m.require(len({identity['pid'] for identity in identities}) == len(PROTECTED) + len(m.ROLES),
              'production inventory must bind ten distinct processes')


def validate_preparation(spec):
    m.require(spec.get('schema') == 'reservoir_production_preparation_v1', 'unsupported production preparation')
    plan = m.validate_plan(spec['plan'])
    validate_process_inventory(plan)
    validate_engine_inputs(spec)
    m.require(spec.get('state_root') == str(STATE_ROOT), 'canonical state root required')
    runtime_root = Path(spec['transaction_root'])
    m.require(runtime_root == CONTROL_ROOT / plan['transaction_id'], 'transaction root is not fixed and owned')
    files = spec.get('immutable_files', {})
    m.require(files and all(Path(path).is_absolute() and m.hash_value(value) for path, value in files.items()),
              'complete immutable source/config/package/model inventory required')
    required = {str(HOLD_SOURCE), str(WORKSPACE / '.venv/pyvenv.cfg'), *map(str, PLISTS.values()), *map(str, SOURCES.values())}
    m.require(required <= set(files), 'canonical originals/interpreter evidence missing')
    for path, value in files.items():
        m.require(sha(path) == value, 'immutable input changed: ' + path)
    validate_closure(spec)
    m.require(input_hashes(spec) == plan['input_hashes'], 'plan inputs do not describe actual original bytes')
    for role in m.ROLES:
        raw, _ = stable_bytes(PLISTS[role])
        settings = plistlib.loads(raw)
        m.require(settings.get('Label') == m.LABELS[role] and settings.get('WorkingDirectory') == str(WORKSPACE)
                  and settings.get('ProgramArguments', [None, None])[1] == str(SOURCES[role])
                  and settings.get('StandardOutPath') == settings.get('StandardErrorPath') == str(LOGS[role]),
                  'unexpected original launch configuration')
        preserved = Path(spec['originals'][role]['plist'])
        source_copy = Path(spec['originals'][role]['source'])
        for copied, canonical in ((preserved, PLISTS[role]), (source_copy, SOURCES[role])):
            m.require(copied.is_absolute() and copied != canonical and copied.resolve() != canonical.resolve()
                      and str(copied) in files, 'separate hash-bound original preservation required')
            retained, _ = stable_bytes(copied)
            current, _ = stable_bytes(canonical)
            m.require(retained == current, 'preserved original bytes differ')
    fixture = qualified_fixture(spec, plan)
    m.require(fixture.get('exact_release_fixture_passed') is True
              and fixture.get('host_binding') == plan['host_binding'] and fixture.get('production_qualified') is False,
              'accepted current-host owned fixture missing')
    alignment = reference(spec['runtime_alignment'])
    m.require(alignment.get('schema') == 'reservoir_original_runtime_alignment_v1'
              and alignment.get('accepted') is True and alignment.get('host_binding') == plan['host_binding']
              and alignment.get('roles') == plan['roles'] and alignment.get('input_hashes') == plan['input_hashes']
              and alignment.get('immutable_files') == files and alignment.get('reviewer') and alignment.get('evidence'),
              'separate original loaded-runtime alignment review required')
    return plan


def qualified_fixture(spec, plan):
    """Use the same complete raw-proof reader as production admission."""
    import production_fixture_admission as fixture_reader
    m.require(Path(fixture_reader.__file__).resolve(strict=True) == HERE / 'production_fixture_admission.py',
              'fixture reader loaded origin differs')
    return fixture_reader.validate_fixture(spec, plan)


def validate_engine_inputs(spec):
    """The native engine is distinct from the autonomous agent process."""
    m.require({str(path) for path in ENGINE_INPUTS} <= set(spec.get('immutable_files', {})),
              'native Minime engine binary/configuration closure missing')
    engine = m.identity(spec['plan']['protected']['minime_engine'])
    m.require(engine['argv'][0] == str(MINIME / 'minime/target/release/minime'),
              'native Minime engine command differs from preserved deployment')


def preparation_digest(spec):
    # Approval may bind preparation, but preparation cannot contain its own
    # approval file's hash in the digest being approved.
    return m.digest({key: value for key, value in spec.items() if key != 'execution_review'})


def reservoir_config(argv):
    defaults = {'--input-dim': '32', '--n-nodes': '192', '--backend': 'numpy',
                '--host': '127.0.0.1', '--port': '7881', '--state-dir': str(STATE_ROOT)}
    for key in defaults:
        if key in argv:
            m.require(argv.count(key) == 1 and argv.index(key) + 1 < len(argv), 'ambiguous reservoir option')
            defaults[key] = argv[argv.index(key) + 1]
    m.require(defaults['--host'] == '127.0.0.1' and defaults['--port'] == '7881'
              and defaults['--state-dir'] == str(STATE_ROOT) and defaults['--backend'] == 'numpy',
              'only original loopback CPU reservoir configuration is covered')
    return {'input_dim': int(defaults['--input-dim']), 'n_nodes': int(defaults['--n-nodes']),
            'backend': 'numpy', 'backend_label': 'NumPy'}


def validate_closure(spec):
    env = load_hold()
    ref = spec.get('original_provider_manifest')
    reference(ref)
    checked = env.validate_manifest(Path(ref['path']))
    manifest = checked['manifest']; files = spec['immutable_files']; groups = spec['input_groups']
    m.require(manifest['python'] == str(WORKSPACE / '.venv/bin/python')
              and manifest['working_directory'] == str(WORKSPACE), 'manifest is not the original runtime')
    required = {**manifest['artifacts'], ref['path']: ref['sha256']}
    frozen = Path(manifest['source_root'])
    names = {str(path.relative_to(frozen)) for path in frozen.rglob('*.py')}
    m.require({path.name for path in WORKSPACE.glob('*.py')} == names,
              'canonical source closure differs from preserved original source set')
    for name in names:
        m.require(not (WORKSPACE / name).is_symlink(), 'canonical Python source must not be a symlink')
        required[str(WORKSPACE / name)] = manifest['artifacts'][str(frozen / name)]
    m.require(all(files.get(path) == value for path, value in required.items()),
              'complete original source/package/model closure is not bound')
    interpreters = {str(path) for path in env.interpreter_paths(Path(manifest['python']))}
    model_root = Path(checked['model'])
    models = {str(path) for path in model_root.rglob('*') if path.is_file()}
    packages = {path for path in manifest['artifacts'] if Path(path).is_relative_to(WORKSPACE / '.venv')}
    packages.add(manifest['package_inventory'])
    m.require(set(groups['interpreter']) == interpreters and set(groups['model_snapshot']) == models
              and set(groups['provider_packages']) == packages, 'input group omits or mislabels runtime closure')
    helpers = {str(HOLD_SOURCE), str(HERE / 'production_host.py'), str(HERE / 'production_preflight.py'),
               str(HERE / 'reservoir_maintenance.py'), str(HERE / 'reservoir_maintenance_state.py'),
               str(HERE / 'restore_verifier.py'), str(HERE / 'production_admission.py'),
               str(HERE / 'production_restore_admission.py'), str(HERE / 'production_fixture_admission.py'),
               str(HERE / 'production_fixture_history.py'), str(HERE / 'production_fixture_readiness.py'),
               str(HERE / 'production_fixture_native.py'), str(HERE / 'production_native.py'),
               str(HERE / 'production_native_attempt.py'), str(HERE / 'production_entrypoint.py'),
               str(HERE / 'production_restore_runner.py'), str(HERE / 'production_recovery.py'),
               str(HERE / 'production_recovery_inspect.py'), str(HERE / 'production_retained.py'),
               str(HERE / 'transaction_execution.py'), str(HERE / 'recovery_ledger.py'), str(HERE / 'recovery_phases.py'), str(HERE / 'production_recovery_driver.py')}
    m.require(helpers <= set(groups['helpers']), 'production helper closure incomplete')
    fixture = qualified_fixture(spec, spec['plan'])
    for path, digest in fixture.get('python_executables', {}).items():
        m.require(files.get(path) == digest and sha(path) == digest, 'qualified holder interpreter changed')
    m.require(fixture.get('python_executables'), 'fixture does not bind holder interpreter')
    actual_config = reservoir_config(spec['plan']['roles']['reservoir']['identity']['argv'])
    m.require(spec.get('reservoir_config') == actual_config, 'restore config differs from original launch defaults')
    resolution = reference(spec['model_resolution'])
    m.require(resolution.get('schema') == 'original_provider_model_resolution_review_v1'
              and resolution.get('accepted') is True and resolution.get('snapshot') == checked['model']
              and resolution.get('original_argv') == spec['plan']['roles']['provider']['identity']['argv']
              and resolution.get('restart_policy') and resolution.get('reviewer') and resolution.get('evidence'),
              'explicit original model resolution/restart policy review required')
    validate_provider_restart(spec, checked['model'])
    validate_restore_inputs(spec, spec['restore_source_manifest'], spec['restore_runtime_manifest'])


def pinned_argv(argv, snapshot):
    m.require(argv.count('--model') == 1 and argv.index('--model') + 1 < len(argv), 'exact original model option required')
    result = list(argv); result[result.index('--model') + 1] = snapshot
    return result


def validate_provider_restart(spec, snapshot):
    original_argv = spec['plan']['roles']['provider']['identity']['argv']
    m.require(spec.get('provider_restart_argv') == pinned_argv(original_argv, snapshot),
              'provider restart changes more than the exact model snapshot value')
    original = plistlib.loads(PLISTS['provider'].read_bytes())
    expected = dict(original, ProgramArguments=pinned_argv(original['ProgramArguments'], snapshot))
    ref = spec['provider_restart_plist']
    m.require(Path(ref['path']).is_absolute() and sha(ref['path']) == ref['sha256'], 'restart plist missing or changed')
    actual = plistlib.loads(Path(ref['path']).read_bytes())
    m.require(actual == expected, 'restart plist changes unrelated settings')


def validate_restore_inputs(spec, source, runtime):
    files = spec['immutable_files']
    expected = {name: spec['repair_source']['sha256'] if name == 'reservoir_service.py'
                else files[str(WORKSPACE / name)] for name in RESTORE_NAMES}
    m.require(source.get('files') == expected and Path(source['root']).is_absolute(),
              'private restore source differs from original closure plus exact scheduler repair')
    for name, digest in expected.items():
        m.require(sha(Path(source['root']) / name) == digest, 'private restore source artifact changed')
    original = reference(spec['original_provider_manifest'])
    m.require(runtime.get('python') == str(WORKSPACE / '.venv/bin/python')
              and {name: value.get('version') for name, value in runtime.get('packages', {}).items()} == original['packages'],
              'private restore runtime/package inventory differs from original environment')
    required = dict(runtime['interpreter_files'])
    required[runtime['pyvenv_cfg']['path']] = runtime['pyvenv_cfg']['sha256']
    for value in runtime['packages'].values():
        required.update(value['files'])
    m.require(all(files.get(path) == digest for path, digest in required.items())
              and set(runtime['interpreter_files']) == set(spec['input_groups']['interpreter']),
              'private restore package/interpreter artifacts are not bound original bytes')


def input_hashes(spec):
    repair = spec.get('repair_source', {})
    m.require(isinstance(repair.get('path'), str) and Path(repair['path']).is_absolute()
              and sha(repair['path']) == repair.get('sha256'), 'reviewed repair source missing or changed')
    groups = spec.get('input_groups', {})
    m.require(set(groups) == {'provider_packages', 'model_snapshot', 'interpreter', 'helpers'},
              'exact package/model/interpreter/helper input groups required')
    grouped = {}
    for key, paths in groups.items():
        m.require(isinstance(paths, list) and paths and len(paths) == len(set(paths))
                  and all(path in spec['immutable_files'] for path in paths), 'unbound input group')
        grouped[key] = m.digest({path: sha(path) for path in paths})
    return {**grouped, 'provider_source': sha(SOURCES['provider']),
            'provider_settings': sha(PLISTS['provider']), 'reservoir_source': sha(SOURCES['reservoir']),
            'reservoir_candidate': repair['sha256'], 'reservoir_settings': sha(PLISTS['reservoir']),
            'provider_restart_settings': spec['provider_restart_plist']['sha256'],
            'fixture': spec['fixture']['sha256']}


def check_only(spec, io):
    plan = validate_preparation(spec)
    current = io.stewardship()
    m.require(io.host() == plan['host_binding'] and current == {'paused': True,
              'pause_generation': plan['pause_generation'], 'pause_actor': plan['actor'], 'lease': None},
              'host or stewardship changed')
    for name, identity in plan['protected'].items():
        m.require(io.process(identity['pid']) == identity, 'protected process changed: ' + name)
    env = load_hold()
    roles = {}
    for role in m.ROLES:
        view = io.role(role); expected = plan['roles'][role]['identity']
        m.require(view['job_loaded'] and view['process'] == expected and view['listener_pids'] == [expected['pid']],
                  'original process or listener changed: ' + role)
        settings = plistlib.loads(PLISTS[role].read_bytes())
        m.require(env.command_matches(expected['argv'], settings['ProgramArguments']), 'original argv/plist mismatch')
        roles[role] = view
    return {'schema': 'reservoir_production_check_v1', 'passed': True, 'mutation_performed': False,
            'service_probes_performed': False, 'execution_enabled': False,
            'plan_sha256': m.digest(plan), 'roles': roles, 'protected': plan['protected'],
            'limitations': ['loaded-runtime alignment is reviewed provenance, not inferred from disk',
                            'production controller gate, actual restore matrix and execution review remain required']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--preparation', type=Path, required=True)
    parser.add_argument('--preparation-sha256',required=True)
    parser.add_argument('--native-attempt-name',required=True)
    args = parser.parse_args()
    from production_entrypoint import run
    print(json.dumps(run(args.preparation,args.preparation_sha256,mode='check',native_attempt_name=args.native_attempt_name),indent=2))


if __name__ == '__main__':
    main()
